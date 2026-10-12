//! Bulk actions on a selection: mark read or viewed (T-37a-027), change categories (T-37a-030) and the continue
//! button's visibility (T-37a-033). The rules are pure; each write goes through the storage layer that owns it.

use std::sync::Arc;
use std::time::{Duration, Instant};

use kuro_data::anime::AnimeRepo;
use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::episode::{EpisodeRepo, EpisodeUpdate};
use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use kuro_domain::app::status::{chapter_status_update, chapters_to_change, episode_status_update, episodes_to_change};
use kuro_domain::category::bulk;
use kuro_domain::category::service::CategoryService;
use kuro_domain::error::DomainError;
use kuro_platform::kind::Kind;
use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::search::EmptyPage;

use crate::updates::download_actions::UnitTarget;
use crate::updates::download_exec;
use crate::updates::download_host::DownloadBackend;
use crate::updates::download_state::{self, DiskKey};

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

/// A chapter or episode a mark-read changed: what the download check and the download removal need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangedItem {
    pub id: i64,
    pub name: String,
    pub scanlator: Option<String>,
}

/// Marks the chapters or episodes of one entry whose `url` is in `urls` read or unread, the ones an update notice names
/// (T-26a-034), and returns the ones that changed; urls the entry does not hold are ignored.
pub fn mark_items_by_url_changed(
    db: &Arc<Db>,
    kind: Kind,
    entry_id: i64,
    urls: &[String],
    read: bool,
) -> Result<Vec<ChangedItem>, DomainError> {
    match kind {
        Kind::Manga => {
            let repo = ChapterRepo::new(db.clone());
            let named: Vec<_> = repo.by_manga_id(entry_id, false)?.into_iter().filter(|c| urls.contains(&c.url)).collect();
            let changed = chapters_to_change(read, &named);
            let items = changed.iter().map(|c| ChangedItem { id: c.id, name: c.name.clone(), scanlator: c.scanlator.clone() }).collect();
            let updates: Vec<ChapterUpdate> = changed.iter().map(|c| chapter_status_update(read, c)).collect();
            repo.update_all(&updates)?;
            Ok(items)
        }
        Kind::Anime => {
            let repo = EpisodeRepo::new(db.clone());
            let named: Vec<_> = repo.by_anime_id(entry_id)?.into_iter().filter(|e| urls.contains(&e.url)).collect();
            let changed = episodes_to_change(read, &named);
            let items = changed.iter().map(|e| ChangedItem { id: e.id, name: e.name.clone(), scanlator: e.scanlator.clone() }).collect();
            let updates: Vec<EpisodeUpdate> = changed.iter().map(|e| episode_status_update(read, e)).collect();
            repo.update_all(&updates)?;
            Ok(items)
        }
    }
}

/// The title and source id of an entry, for the download disk check. `None` when the entry is gone.
fn entry_facts(db: &Arc<Db>, kind: Kind, entry_id: i64) -> Option<(String, i64)> {
    match kind {
        Kind::Manga => MangaRepo::new(db.clone()).by_id(entry_id).ok().map(|m| (m.title, m.source)),
        Kind::Anime => AnimeRepo::new(db.clone()).by_id(entry_id).ok().map(|a| (a.title, a.source)),
    }
}

/// The notice's "Mark as read" (T-26a-093): marks the named items read and, when "remove after marked as read" is on,
/// deletes the downloads of the items that changed. Returns how many items changed.
pub fn mark_notified_read(
    db: &Arc<Db>,
    backend: &dyn DownloadBackend,
    kind: Kind,
    entry_id: i64,
    urls: &[String],
    remove_downloads: bool,
) -> Result<usize, DomainError> {
    let changed = mark_items_by_url_changed(db, kind, entry_id, urls, true)?;
    if remove_downloads && !changed.is_empty() {
        if let Some((title, source_id)) = entry_facts(db, kind, entry_id) {
            let targets: Vec<UnitTarget> = changed
                .iter()
                .map(|item| {
                    let key = DiskKey {
                        source_id,
                        entry_title: title.clone(),
                        unit_name: item.name.clone(),
                        scanlator: item.scanlator.clone(),
                    };
                    UnitTarget { entry_id, unit_id: item.id, state: download_state::derive(None, backend.on_disk(kind, &key)).state }
                })
                .collect();
            download_exec::remove_read_downloads(backend, kind, true, &targets);
        }
    }
    Ok(changed.len())
}

/// The ids of the chapters or episodes of an entry whose url is in `urls`. Empty when the entry is gone.
pub fn item_ids_by_url(db: &Arc<Db>, kind: Kind, entry_id: i64, urls: &[String]) -> Vec<i64> {
    match kind {
        Kind::Manga => ChapterRepo::new(db.clone())
            .by_manga_id(entry_id, false)
            .map(|rows| rows.into_iter().filter(|c| urls.contains(&c.url)).map(|c| c.id).collect()),
        Kind::Anime => EpisodeRepo::new(db.clone())
            .by_anime_id(entry_id)
            .map(|rows| rows.into_iter().filter(|e| urls.contains(&e.url)).map(|e| e.id).collect()),
    }
    .unwrap_or_default()
}

/// The notice's "Download" (T-26a-093): queues the named items with autostart on, so the queue starts at once. An entry
/// that is gone, or items that match nothing, queue nothing and say nothing. Returns how many items were named.
pub fn download_notified(
    db: &Arc<Db>,
    backend: &dyn DownloadBackend,
    kind: Kind,
    entry_id: i64,
    urls: &[String],
    external: bool,
) -> usize {
    let ids = item_ids_by_url(db, kind, entry_id, urls);
    if ids.is_empty() {
        return 0;
    }
    backend.enqueue(kind, entry_id, &ids, external);
    backend.start_queue(kind);
    ids.len()
}

/// Marks every selected entry and returns how many entries were written. A failed entry is logged and the rest
/// still change (T-37a-027).
pub fn mark_many(db: &Arc<Db>, kind: Kind, ids: &[i64], read: bool) -> usize {
    ids.iter()
        .filter(|id| {
            let done = match kind {
                Kind::Manga => mark_manga(db, **id, read).map(drop),
                Kind::Anime => mark_anime(db, **id, read).map(drop),
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

/// The Download menu of the selection bar for `kind`: next 1, 5, 10 or 25 items, then every unviewed one (T-37a-028).
pub fn download_menu(kind: Kind) -> Vec<(DownloadAmount, String)> {
    let (one, many, unviewed) = match kind {
        Kind::Manga => ("chapter", "chapters", "Unread"),
        Kind::Anime => ("episode", "episodes", "Unwatched"),
    };
    vec![
        (DownloadAmount::Next(1), format!("Next {one}")),
        (DownloadAmount::Next(5), format!("Next 5 {many}")),
        (DownloadAmount::Next(10), format!("Next 10 {many}")),
        (DownloadAmount::Next(25), format!("Next 25 {many}")),
        (DownloadAmount::Unviewed, unviewed.to_owned()),
    ]
}

/// One entry of a cover's right-click menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverAction {
    Open,
    MarkAllViewed,
    DownloadUnviewed,
    ChangeCategories,
    Remove,
}

/// A cover's right-click menu for `kind`, in order, as (action, label, section). A local entry has nothing to
/// download, as in the selection bar (T2-BULK-03).
pub fn cover_menu(kind: Kind, local: bool) -> Vec<(CoverAction, String, u8)> {
    let (mark, download) = match kind {
        Kind::Manga => ("Mark all as read", "Download unread"),
        Kind::Anime => ("Mark all as watched", "Download unwatched"),
    };
    let mut menu = vec![(CoverAction::Open, "Open".to_owned(), 0), (CoverAction::MarkAllViewed, mark.to_owned(), 1)];
    if !local {
        menu.push((CoverAction::DownloadUnviewed, download.to_owned(), 1));
    }
    menu.push((CoverAction::ChangeCategories, "Change categories".to_owned(), 2));
    menu.push((CoverAction::Remove, kuro_i18n::tr(kuro_i18n::MsgKey::remove_from_library), 3));
    menu
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

// Decision (T-37a-062, supersedes T-26a-113): the library's toasts go through the shared layer (`crate::toast::notify_for`)
// for `crate::library::TOAST_TIME` (4 s, U-1.8).

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

/// What the empty library offers (T-37a-053), shown only when it has no entries, no search and no filter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmptyLibraryAction {
    /// No source repository is set up yet: the welcome view (`view::welcome`) leads with adding one.
    AddRepository,
    /// A repository exists: the library is simply empty, so the button leads to Browse.
    Browse,
}

/// The action on the empty library page, `None` on any other empty page (a search, a filter or an empty category).
pub fn empty_library_action(page: EmptyPage, has_repositories: bool) -> Option<EmptyLibraryAction> {
    (page == EmptyPage::LibraryEmpty).then_some(if has_repositories { EmptyLibraryAction::Browse } else { EmptyLibraryAction::AddRepository })
}

/// The toast when the continue button finds nothing to open (E-25).
pub const NO_NEXT_ITEM: &str = "Next chapter not found";

/// The item the continue button opens: the first pending chapter or episode of the entry's own list, after the
/// entry's filters and sort (the last pending one when sorted descending). `is_downloaded` takes the item id.
pub fn next_item(
    db: &Arc<Db>,
    kind: Kind,
    row: &LibraryRow,
    downloaded_only: bool,
    is_downloaded: impl Fn(i64) -> bool,
) -> Result<Option<i64>, DomainError> {
    // the one next-unread rule, shared with the player's "resume next" (`crate::selection`)
    match kind {
        Kind::Manga => {
            let manga = MangaRepo::new(db.clone()).by_id(row.id)?;
            let list = ChapterRepo::new(db.clone()).by_manga_id(row.id, true)?;
            Ok(crate::selection::next_unread(&manga, &list, downloaded_only, |c| is_downloaded(c.id)).map(|c| c.id))
        }
        Kind::Anime => {
            let anime = AnimeRepo::new(db.clone()).by_id(row.id)?;
            let list = EpisodeRepo::new(db.clone()).by_anime_id(row.id)?;
            Ok(crate::selection::next_unseen(&anime, &list, downloaded_only, |e| is_downloaded(e.id)).map(|e| e.id))
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

    #[test]
    fn the_cover_menu_lists_its_actions_and_skips_download_for_a_local_entry() {
        use CoverAction::{ChangeCategories, DownloadUnviewed, MarkAllViewed, Open, Remove};
        let actions = |kind, local| cover_menu(kind, local).into_iter().map(|(a, _, _)| a).collect::<Vec<_>>();
        assert_eq!(actions(Kind::Manga, false), [Open, MarkAllViewed, DownloadUnviewed, ChangeCategories, Remove]);
        assert_eq!(actions(Kind::Anime, true), [Open, MarkAllViewed, ChangeCategories, Remove]);
        assert_eq!(cover_menu(Kind::Anime, false)[1].1, "Mark all as watched");
        assert_eq!(cover_menu(Kind::Manga, false)[4].1, "Remove from library");
    }
    use kuro_data::chapter::{Chapter, ChapterRepo};
    use kuro_data::manga::{Manga, MangaRepo};
    use std::collections::BTreeSet;

    #[test]
    fn t2_cont_03_an_excluded_scanlator_is_never_the_next_chapter() {
        // The only unread chapter belongs to an excluded scanlator, so there is nothing to continue with.
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
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
        assert_eq!(next_item(&db, Kind::Manga, &entry, false, |_| false).expect("next item"), None);
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
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
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
                    "INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, number, source_order, date_fetch, date_upload) \
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
            let id = next_item(&db, Kind::Manga, &row, false, |_| false).expect("target").expect("some");
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
        let id = next_item(&db1, Kind::Manga, &row, false, |_| false).expect("target").expect("some");
        assert_eq!(chapter_number_of(&db1, row.id, id), 3.0, "the unbookmarked chapter 2 is filtered out");
        let (_dir2, db2) = db();
        let row = manga_with_chapters(&db2, SORTING_NUMBER | SORT_ASC, &[(1, true, false)]);
        assert_eq!(next_item(&db2, Kind::Manga, &row, false, |_| false).expect("target"), None);
        assert_eq!(NO_NEXT_ITEM, "Next chapter not found", "the toast text of E-25");
    }

    #[test]
    fn t2_cont_03_a_chapter_of_an_excluded_scanlator_is_never_chosen() {
        use kuro_db::flags::chapter::{SORTING_NUMBER, SORT_ASC};
        let (_dir, db) = db();
        let row = manga_with_chapters(&db, SORTING_NUMBER | SORT_ASC, &[(1, true, false), (2, false, false), (3, false, false)]);
        db.write(|tx| {
            tx.execute("UPDATE items SET scanlator = 'Bad' WHERE entry_id = ?1 AND number = 2", [row.id])?;
            tx.execute("INSERT INTO excluded_scanlators(entry_id, scanlator) VALUES (?1, 'Bad')", [row.id])?;
            Ok(())
        })
        .expect("exclude a scanlator");
        let id = next_item(&db, Kind::Manga, &row, false, |_| false).expect("target").expect("some");
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
        assert_eq!(mark_many(&db, Kind::Manga, &[row.id, row.id], true), 2);
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
                    "INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, number, source_order, date_fetch, date_upload) \
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

    fn notified_manga(db: &Arc<Db>) -> (i64, Vec<(String, i64)>) {
        let id = MangaRepo::new(db.clone())
            .insert(&Manga { favorite: true, source: 7, url: "m".into(), title: "m".into(), ..Manga::create() })
            .expect("insert");
        db.write(|tx| {
            for n in 1..=3 {
                tx.execute(
                    "INSERT INTO items(entry_id, url, name, scanlator, done, bookmark, progress, number, source_order, date_fetch, date_upload) \
                     VALUES (?1, ?2, ?3, NULL, 0, 0, 0, ?4, ?4, 0, 0)",
                    (id, format!("u{n}"), format!("c{n}"), n as f64),
                )?;
            }
            Ok(())
        })
        .expect("chapters");
        let ids = ChapterRepo::new(db.clone()).by_manga_id(id, false).expect("rows").into_iter().map(|c| (c.url, c.id)).collect();
        (id, ids)
    }

    fn id_of(ids: &[(String, i64)], url: &str) -> i64 {
        ids.iter().find(|(u, _)| u == url).map(|(_, id)| *id).expect("chapter id")
    }

    #[test]
    fn t_26a_093_mark_read_from_a_notice_removes_the_downloads_of_the_changed_items_when_the_option_is_on() {
        use crate::updates::download_host::fake::{Call, Fake};
        let (_dir, db) = db();
        let (id, ids) = notified_manga(&db);
        let backend = Fake::installed();
        backend.disk.lock().unwrap().extend([1, 2]);
        let urls = ["u1".to_owned(), "u2".to_owned(), "u3".to_owned()];
        // option off: the items are marked, nothing is deleted
        assert_eq!(mark_notified_read(&db, &backend, Kind::Manga, id, &urls[..1], false).expect("mark"), 1);
        assert!(backend.calls().is_empty(), "remove-after-read is off");
        // option on: only the changed, downloaded item goes (c1 was already read, c3 is not on disk)
        assert_eq!(mark_notified_read(&db, &backend, Kind::Manga, id, &urls, true).expect("mark"), 2);
        assert_eq!(backend.calls(), vec![Call::Delete { entry_id: id, unit_ids: vec![id_of(&ids, "u2")] }]);
        // nothing changes the second time, so nothing is deleted again
        assert_eq!(mark_notified_read(&db, &backend, Kind::Manga, id, &urls, true).expect("mark"), 0);
        assert_eq!(backend.calls().len(), 1);
    }

    #[test]
    fn t_26a_093_the_download_button_queues_the_named_items_with_autostart_and_a_missing_entry_is_a_no_op() {
        use crate::updates::download_host::fake::{Call, Fake};
        let (_dir, db) = db();
        let (id, ids) = notified_manga(&db);
        let backend = Fake::installed();
        let urls = ["u2".to_owned(), "u3".to_owned(), "gone".to_owned()];
        assert_eq!(download_notified(&db, &backend, Kind::Manga, id, &urls, false), 2);
        assert_eq!(
            backend.calls(),
            vec![
                Call::Enqueue { entry_id: id, unit_ids: vec![id_of(&ids, "u2"), id_of(&ids, "u3")], external: false },
                Call::StartQueue,
            ],
            "autostart: the queue starts right after the enqueue"
        );
        let other = Fake::installed();
        assert_eq!(download_notified(&db, &other, Kind::Manga, 9_999, &urls, false), 0);
        assert!(other.calls().is_empty(), "a missing entry is a silent no-op");
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
    fn t_37a_053_an_empty_library_offers_a_repository_first_and_browse_once_one_exists() {
        use kuro_domain::library::search::empty_page;
        let library_empty = empty_page(false, false, true);
        assert_eq!(empty_library_action(library_empty, false), Some(EmptyLibraryAction::AddRepository));
        assert_eq!(empty_library_action(library_empty, true), Some(EmptyLibraryAction::Browse));
    }

    #[test]
    fn t_37a_053_the_empty_library_action_never_shows_for_a_search_a_filter_or_an_empty_category() {
        use kuro_domain::library::search::empty_page;
        assert_eq!(empty_library_action(empty_page(true, false, true), false), None, "a search leaves no room for it");
        assert_eq!(empty_library_action(empty_page(false, true, true), true), None, "an active filter does not");
        assert_eq!(empty_library_action(empty_page(false, false, false), false), None, "an empty category is not an empty library");
    }

    #[test]
    fn t2_upd_02_a_running_update_says_so() {
        assert_eq!(update_toast(UpdateScope::Library, true), "An update is already running");
        assert_eq!(update_toast(UpdateScope::Category, true), "An update is already running");
    }
}
