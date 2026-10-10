//! Bulk actions on a selection: mark read or viewed (T-37a-027), change categories (T-37a-030) and the continue
//! button's visibility (T-37a-033). The rules are pure; each write goes through the storage layer that owns it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use kuro_data::anime::AnimeRepo;
use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::episode::{EpisodeRepo, EpisodeUpdate};
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use kuro_domain::app::items::{apply_filters, chapter_filters, episode_filters};
use kuro_domain::app::status::{chapter_status_update, chapters_to_change, episode_status_update, episodes_to_change};
use kuro_domain::category::bulk;
use kuro_domain::category::service::CategoryService;
use kuro_domain::error::DomainError;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::active::continue_target;
use kuro_domain::library::model::LibraryRow;

/// The category ids an entry keeps after a change. `all_links` must hold every link, hidden ones included, so
/// a hidden category the user cannot see in the dialog is not dropped (E-07).
pub fn next_membership(all_links: &[i64], include: &[i64], exclude: &[i64]) -> Vec<i64> {
    bulk::apply(all_links, include, exclude)
}

/// Applies a dialog result to every selected entry. Returns how many entries failed to save; the others still
/// change. Each entry is one write, which is one transaction in the repository.
pub async fn set_membership(
    categories: &CategoryService,
    entries: &[i64],
    include: &[i64],
    exclude: &[i64],
) -> Result<usize, DomainError> {
    let mut failed = 0;
    for entry in entries {
        let links: Vec<i64> = categories.repo().by_entry(*entry, false)?.iter().map(|c| c.id).collect();
        let next = next_membership(&links, include, exclude);
        if let Err(e) = categories.set_entry_categories(*entry, &next).await {
            tracing::error!(error = %e, entry, "saving categories failed");
            failed += 1;
        }
    }
    Ok(failed)
}

/// Marks every chapter of a manga read or unread. Returns the number of chapters that changed.
/// Removing downloads after a chapter is read is not done here (no download manager is wired into the library yet).
pub fn mark_manga(db: &Arc<Db>, manga_id: i64, read: bool) -> Result<usize, DomainError> {
    let repo = ChapterRepo::new(db.clone());
    let chapters = repo.by_manga_id(manga_id, false)?;
    let updates: Vec<ChapterUpdate> =
        chapters_to_change(read, &chapters).iter().map(|c| chapter_status_update(read, c)).collect();
    repo.update_all(&updates)?;
    Ok(updates.len())
}

/// Marks every episode of an anime seen or unseen. Returns the number of episodes that changed.
pub fn mark_anime(db: &Arc<Db>, anime_id: i64, seen: bool) -> Result<usize, DomainError> {
    let repo = EpisodeRepo::new(db.clone());
    let episodes = repo.by_anime_id(anime_id)?;
    let updates: Vec<EpisodeUpdate> =
        episodes_to_change(seen, &episodes).iter().map(|e| episode_status_update(seen, e)).collect();
    repo.update_all(&updates)?;
    Ok(updates.len())
}

/// Marks every selected entry and returns how many entries were written. A failed entry is logged and the rest
/// still change (T-37a-027).
pub fn mark_many(db: &Arc<Db>, kind: EntryKind, ids: &[i64], read: bool) -> usize {
    ids.iter()
        .filter(|id| {
            let done = match kind {
                EntryKind::Manga => mark_manga(db, **id, read).map(drop),
                EntryKind::Anime => mark_anime(db, **id, read).map(drop),
            };
            if let Err(e) = &done {
                tracing::error!(error = %e, entry = **id, "marking the entry failed");
            }
            done.is_ok()
        })
        .count()
}

/// The toast after a bulk write when some entries failed (T2-BULK-02); `None` when every write succeeded.
pub fn partial_failure_toast(ok: usize, total: usize) -> Option<String> {
    (ok < total).then(|| format!("{ok} of {total} updated"))
}

/// Whether the bulk download action shows: it is hidden when any selected entry is local (T2-BULK-03).
pub fn download_visible<'a>(mut selected: impl Iterator<Item = &'a LibraryRow>) -> bool {
    !selected.any(LibraryRow::is_local)
}

/// How many items a bulk download covers (`DownloadAction`): the next N, or every unseen one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadAmount {
    Next(usize),
    Unviewed,
}

/// The item ids to queue for one entry. `unseen` is its next-items list in reading order. Items that are already
/// queued or downloaded (`skip`) are dropped first and the amount is taken after, so "Next 5" picks the next five
/// that are not on disk, which may lie further along the list (E-36, T2-BULK-01, T2-BULK-04).
pub fn plan_download(unseen: &[i64], amount: DownloadAmount, skip: impl Fn(i64) -> bool) -> Vec<i64> {
    let pending = unseen.iter().copied().filter(|id| !skip(*id));
    match amount {
        DownloadAmount::Next(n) => pending.take(n).collect(),
        DownloadAmount::Unviewed => pending.collect(),
    }
}

/// The two checkboxes of the remove dialog: take the entries out of the library, delete their downloads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DeleteChoice {
    pub from_library: bool,
    pub downloads: bool,
}

impl DeleteChoice {
    /// OK needs at least one box checked (E-37).
    pub fn confirmable(self) -> bool {
        self.from_library || self.downloads
    }
}

/// What a confirmed remove dialog does.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeletePlan {
    /// Entries whose favorite flag goes off.
    pub unfavorite: Vec<i64>,
    /// Entries whose downloads go through the download manager.
    pub delete_downloads: Vec<i64>,
}

/// The "Downloaded" checkbox is not offered when the selection holds a local entry (E-28, T2-DEL-04).
pub fn downloads_checkbox_offered<'a>(mut selected: impl Iterator<Item = &'a LibraryRow>) -> bool {
    !selected.any(LibraryRow::is_local)
}

/// Plans a confirmed remove for `entries` as `(id, is_local)` pairs. The two choices are independent: downloads can go
/// while the entry stays in the library, and the other way round. A local entry's folder is user data and is never
/// deleted (E-28, T2-DEL-01, T2-DEL-02, T2-DEL-04).
pub fn delete_plan(entries: &[(i64, bool)], choice: DeleteChoice) -> DeletePlan {
    let mut seen = std::collections::BTreeSet::new();
    let unique: Vec<(i64, bool)> = entries.iter().copied().filter(|(id, _)| seen.insert(*id)).collect();
    DeletePlan {
        unfavorite: if choice.from_library { unique.iter().map(|(id, _)| *id).collect() } else { Vec::new() },
        delete_downloads: if choice.downloads { unique.iter().filter(|(_, local)| !local).map(|(id, _)| *id).collect() } else { Vec::new() },
    }
}

/// What an update request covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateScope {
    Library,
    Category,
}

/// Decision (T-26a-113): the library's toasts go through the app's one toast layer (`crate::toast::notify`), which
/// shows each message for `ToastLength::Short`, 2 s, and replaces a visible one. The 4 s length and the
/// `toast(overlay, text)` signature of the task need a new length in `kuro_platform::toast` and a changed helper in
/// `src/toast.rs`; neither file belongs to the library lane, so the library keeps the shared layer.
/// The toast an update request shows: "Updating library" or "Updating category", or the already-running notice
/// when an update is in progress (T2-UPD-01, T2-UPD-02).
pub fn update_toast(scope: UpdateScope, already_running: bool) -> String {
    use kuro_i18n::{MsgKey, tr};
    tr(match (already_running, scope) {
        (true, _) => MsgKey::update_already_running,
        (false, UpdateScope::Library) => MsgKey::updating_library,
        (false, UpdateScope::Category) => MsgKey::updating_category,
    })
}

/// The toast when the continue button finds nothing to open (E-25).
pub const NO_NEXT_ITEM: &str = "Next chapter not found";

/// The item the continue button opens: the first pending chapter or episode of the entry's own list, after the
/// entry's filters and sort (the last pending one when sorted descending). `is_downloaded` takes the item id.
pub fn next_item(
    db: &Arc<Db>,
    kind: EntryKind,
    row: &LibraryRow,
    downloaded_only: bool,
    is_downloaded: impl Fn(i64) -> bool,
) -> Result<Option<i64>, DomainError> {
    match kind {
        EntryKind::Manga => {
            let filters = chapter_filters(&MangaRepo::new(db.clone()).by_id(row.id)?, downloaded_only)?;
            let list = ChapterRepo::new(db.clone()).by_manga_id(row.id, true)?;
            let list = apply_filters(list, &filters, row.is_local(), |c| is_downloaded(c.id));
            Ok(continue_target(&list, filters.descending, |c| !c.read).map(|c| c.id))
        }
        EntryKind::Anime => {
            let filters = episode_filters(&AnimeRepo::new(db.clone()).by_id(row.id)?, downloaded_only)?;
            let list = EpisodeRepo::new(db.clone()).by_anime_id(row.id)?;
            let list = apply_filters(list, &filters, row.is_local(), |e| is_downloaded(e.id));
            Ok(continue_target(&list, filters.descending, |e| !e.seen).map(|e| e.id))
        }
    }
}

/// Minimum time between two continue clicks on the same entry (R-11).
pub const CONTINUE_DEBOUNCE: Duration = Duration::from_millis(300);

/// Whether a continue click on an entry goes through, given the time of its previous click.
pub fn continue_click_allowed(last: Option<Instant>, now: Instant) -> bool {
    last.is_none_or(|t| now.duration_since(t) >= CONTINUE_DEBOUNCE)
}

/// Whether the continue button shows on an entry. It depends on the raw unseen count, not the unread badge, so
/// turning the badge off does not hide the button (E-02, the fix in T-37a-033).
pub fn continue_visible(button_on: bool, row: &LibraryRow) -> bool {
    button_on && row.pending() > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::chapter::{Chapter, ChapterRepo};
    use kuro_data::manga::{Manga, MangaRepo};
    use std::collections::BTreeSet;

    #[test]
    fn t2_cont_03_an_excluded_scanlator_is_never_the_next_chapter() {
        // The only unread chapter belongs to an excluded scanlator, so there is nothing to continue with.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
        let db = Arc::new(Db::new(&path, writer).expect("manga db handle"));
        let manga_id = MangaRepo::new(db.clone())
            .insert(&Manga { favorite: true, source: 7, url: "/m".into(), title: "t".into(), ..Manga::create() })
            .expect("insert manga");
        ChapterRepo::new(db.clone())
            .add_all(&[
                Chapter { manga_id, url: "/1".into(), name: "1".into(), chapter_number: 1.0, read: true, ..Chapter::create() },
                Chapter {
                    manga_id,
                    url: "/2".into(),
                    name: "2".into(),
                    chapter_number: 2.0,
                    scanlator: Some("bad".into()),
                    ..Chapter::create()
                },
            ])
            .expect("add chapters");
        MangaRepo::new(db.clone())
            .set_excluded_scanlators(manga_id, &BTreeSet::from(["bad".to_string()]))
            .expect("exclude scanlator");
        let entry = LibraryRow { id: manga_id, ..row(2, 1) };
        assert_eq!(next_item(&db, EntryKind::Manga, &entry, false, |_| false).expect("next item"), None);
    }

    fn row(total: i64, done: i64) -> LibraryRow {
        LibraryRow {
            id: 1,
            source: 7,
            title: "t".into(),
            category: 0,
            total,
            done,
            bookmarks: 0,
            fillermarks: 0,
            latest_upload: 0,
            item_fetched_at: 0,
            last_activity: 0,
            last_update: 0,
            date_added: 0,
            status: 0,
            fetch_interval: 0,
            viewer_flags: 0,
            always_update: true,
            next_update: 0,
        }
    }

    fn db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
        (dir, Arc::new(Db::new(&path, writer).expect("manga db handle")))
    }

    #[test]
    fn membership_is_computed_from_every_link_so_hidden_categories_survive() {
        // category 9 is hidden: the dialog never shows it, but the entry keeps it
        let next = next_membership(&[1, 9], &[3], &[1]);
        assert_eq!(next, vec![3, 9]);
        assert_eq!(next_membership(&[1, 2], &[], &[]), vec![1, 2], "a mixed row left alone changes nothing");
        assert_eq!(next_membership(&[2], &[0], &[]), vec![2], "Default is never added");
    }

    #[test]
    fn continue_follows_the_raw_unseen_count_not_the_badge() {
        assert!(continue_visible(true, &row(3, 1)));
        assert!(!continue_visible(true, &row(3, 3)));
        assert!(!continue_visible(false, &row(3, 1)));
    }

    #[test]
    fn continue_clicks_within_300_ms_of_each_other_are_dropped() {
        let t0 = Instant::now();
        assert!(continue_click_allowed(None, t0));
        assert!(!continue_click_allowed(Some(t0), t0 + Duration::from_millis(299)));
        assert!(continue_click_allowed(Some(t0), t0 + Duration::from_millis(300)));
    }

    fn manga_with_chapters(db: &Arc<Db>, flags: i64, chapters: &[(i64, bool, bool)]) -> LibraryRow {
        let id = MangaRepo::new(db.clone())
            .insert(&Manga {
                favorite: true,
                source: 7,
                url: "m".into(),
                title: "m".into(),
                chapter_flags: flags,
                ..Manga::create()
            })
            .expect("insert");
        db.write(|tx| {
            for (n, read, bookmark) in chapters {
                tx.execute(
                    "INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) \
                     VALUES (?1, ?2, 'c', NULL, ?3, ?4, 0, ?5, ?5, 0, 0)",
                    (id, format!("c{n}"), read, bookmark, *n as f64),
                )?;
            }
            Ok(())
        })
        .expect("chapters");
        LibraryRow { id, ..row(0, 0) }
    }

    fn chapter_number_of(db: &Arc<Db>, manga: i64, chapter: i64) -> f64 {
        let list = ChapterRepo::new(db.clone()).by_manga_id(manga, false).expect("chapters");
        list.iter().find(|c| c.id == chapter).expect("chapter").chapter_number
    }

    #[test]
    fn t2_cont_01_continue_opens_the_first_unread_ascending_and_the_last_unread_descending() {
        use kuro_db::flags::chapter::{SORTING_NUMBER, SORT_ASC, SORT_DESC};
        let chapters = [(1, true, false), (2, false, false), (3, false, false)];
        for (dir, want) in [(SORT_ASC, 2.0), (SORT_DESC, 2.0)] {
            let (_dir, db) = db();
            let row = manga_with_chapters(&db, SORTING_NUMBER | dir, &chapters);
            let id = next_item(&db, EntryKind::Manga, &row, false, |_| false).expect("target").expect("some");
            assert_eq!(chapter_number_of(&db, row.id, id), want);
        }
    }

    #[test]
    fn t2_cont_02_continue_applies_the_entry_filters_and_is_none_when_nothing_is_left() {
        use kuro_db::flags::chapter::{SHOW_BOOKMARKED, SORTING_NUMBER, SORT_ASC};
        let (_dir, db1) = db();
        let row = manga_with_chapters(
            &db1,
            SORTING_NUMBER | SORT_ASC | SHOW_BOOKMARKED,
            &[(1, true, false), (2, false, false), (3, false, true)],
        );
        let id = next_item(&db1, EntryKind::Manga, &row, false, |_| false).expect("target").expect("some");
        assert_eq!(chapter_number_of(&db1, row.id, id), 3.0, "the unbookmarked chapter 2 is filtered out");
        let (_dir2, db2) = db();
        let row = manga_with_chapters(&db2, SORTING_NUMBER | SORT_ASC, &[(1, true, false)]);
        assert_eq!(next_item(&db2, EntryKind::Manga, &row, false, |_| false).expect("target"), None);
        assert_eq!(NO_NEXT_ITEM, "Next chapter not found", "the toast text of E-25");
    }

    #[test]
    fn t2_cont_03_a_chapter_of_an_excluded_scanlator_is_never_chosen() {
        use kuro_db::flags::chapter::{SORTING_NUMBER, SORT_ASC};
        let (_dir, db) = db();
        let row = manga_with_chapters(&db, SORTING_NUMBER | SORT_ASC, &[(1, true, false), (2, false, false), (3, false, false)]);
        db.write(|tx| {
            tx.execute("UPDATE chapters SET scanlator = 'Bad' WHERE manga_id = ?1 AND chapter_number = 2", [row.id])?;
            tx.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (?1, 'Bad')", [row.id])?;
            Ok(())
        })
        .expect("exclude a scanlator");
        let id = next_item(&db, EntryKind::Manga, &row, false, |_| false).expect("target").expect("some");
        assert_eq!(chapter_number_of(&db, row.id, id), 3.0);
    }

    #[test]
    fn t2_bulk_02_partial_failures_report_n_of_m_and_full_success_stays_quiet() {
        assert_eq!(partial_failure_toast(2, 3).as_deref(), Some("2 of 3 updated"));
        assert_eq!(partial_failure_toast(3, 3), None);
    }

    #[test]
    fn mark_many_counts_the_entries_written() {
        let (_dir, db) = db();
        let row = manga_with_chapters(&db, 0, &[(1, false, false)]);
        assert_eq!(mark_many(&db, EntryKind::Manga, &[row.id, row.id], true), 2);
    }

    #[test]
    fn mark_manga_read_changes_only_unread_chapters_and_is_idempotent() {
        let (_dir, db) = db();
        let id = MangaRepo::new(db.clone())
            .insert(&Manga { favorite: true, source: 7, url: "m".into(), title: "m".into(), ..Manga::create() })
            .expect("insert");
        db.write(|tx| {
            for (n, read) in [(1, true), (2, false), (3, false)] {
                tx.execute(
                    "INSERT INTO chapters(manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload) \
                     VALUES (?1, ?2, 'c', NULL, ?3, 0, 0, ?4, ?4, 0, 0)",
                    (id, format!("c{n}"), read, n as f64),
                )?;
            }
            Ok(())
        })
        .expect("chapters");
        assert_eq!(mark_manga(&db, id, true).expect("mark read"), 2);
        assert_eq!(mark_manga(&db, id, true).expect("mark read again"), 0);
        assert_eq!(mark_manga(&db, id, false).expect("mark unread"), 3);
    }

    #[test]
    fn t2_bulk_03_the_download_action_hides_when_any_selected_entry_is_local() {
        let mut local = row(3, 0);
        local.source = 0;
        assert!(download_visible([row(3, 0), row(3, 1)].iter()));
        assert!(!download_visible([row(3, 0), local].iter()));
        assert!(download_visible([].iter()));
    }

    #[test]
    fn t2_upd_01_global_and_category_update_messages() {
        assert_eq!(update_toast(UpdateScope::Library, false), "Updating library");
        assert_eq!(update_toast(UpdateScope::Category, false), "Updating category");
    }

    #[test]
    fn t2_upd_02_a_running_update_says_so() {
        assert_eq!(update_toast(UpdateScope::Library, true), "An update is already running");
        assert_eq!(update_toast(UpdateScope::Category, true), "An update is already running");
    }
}
