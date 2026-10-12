//! Bulk actions of the Updates selection (T-42a-030): the rows an action touches are the selected rows whose flag
//! differs from the target (`filterNot` on the snapshot, EC-42-U-12), written in one batch update. Mark read and
//! unread follow `SetReadStatus` / `SetSeenStatus`: unread also resets the progress.

use std::sync::Arc;

use kuro_data::chapter::{ChapterRepo, ChapterUpdate};
use kuro_data::episode::{EpisodeRepo, EpisodeUpdate};
use kuro_db::handle::Db;
use kuro_platform::kind::Kind;

use crate::updates::selection::{Selectable, Selection};

/// The flags one update row carries. Fillermark only exists for anime; manga rows pass `false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitFlags {
    pub id: i64,
    pub read: bool,
    pub bookmark: bool,
    pub fillermark: bool,
    /// `last_page_read` (manga) or `last_second_seen` (anime): unread also applies to a started, unfinished unit.
    pub progress: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BulkAction {
    /// Mark read (`true`) or unread (`false`). Manga `read`, anime `seen`.
    MarkRead(bool),
    Bookmark(bool),
    Fillermark(bool),
}

/// The one batch write of `action` over `ids`. Fillermark exists for anime only, so on manga it writes nothing. An
/// empty id list writes nothing and succeeds (every row already had the target value).
pub fn apply(db: &Arc<Db>, kind: Kind, ids: &[i64], action: BulkAction) -> Result<(), String> {
    if ids.is_empty() {
        return Ok(());
    }
    match kind {
        Kind::Manga => {
            let updates: Vec<ChapterUpdate> = ids
                .iter()
                .filter_map(|&id| {
                    let base = ChapterUpdate::new(id);
                    match action {
                        BulkAction::MarkRead(read) => {
                            Some(ChapterUpdate { read: Some(read), last_page_read: (!read).then_some(0), ..base })
                        }
                        BulkAction::Bookmark(bookmark) => Some(ChapterUpdate { bookmark: Some(bookmark), ..base }),
                        BulkAction::Fillermark(_) => None,
                    }
                })
                .collect();
            if updates.is_empty() {
                return Ok(());
            }
            ChapterRepo::new(db.clone()).update_all(&updates).map_err(|e| e.to_string())
        }
        Kind::Anime => {
            let updates: Vec<EpisodeUpdate> = ids
                .iter()
                .map(|&id| {
                    let base = EpisodeUpdate::new(id);
                    match action {
                        BulkAction::MarkRead(seen) => {
                            EpisodeUpdate { seen: Some(seen), last_second_seen: (!seen).then_some(0), ..base }
                        }
                        BulkAction::Bookmark(bookmark) => EpisodeUpdate { bookmark: Some(bookmark), ..base },
                        BulkAction::Fillermark(fillermark) => EpisodeUpdate { fillermark: Some(fillermark), ..base },
                    }
                })
                .collect();
            EpisodeRepo::new(db.clone()).update_all(&updates).map_err(|e| e.to_string())
        }
    }
}

/// What the selection does when the write ends. Bookmark and fillermark clear it on every outcome; mark read clears it
/// on success only (EC-42-U-11). A failure returns the toast text.
pub fn after_bulk<T: Selectable>(
    action: BulkAction,
    result: Result<(), String>,
    selection: &mut Selection,
    items: &mut [T],
) -> Option<String> {
    match (&result, action) {
        (Err(_), BulkAction::MarkRead(_)) => {}
        _ => selection.toggle_all(items, false),
    }
    result.err().map(|message| format!("Could not update the selection: {message}"))
}

/// The ids of the rows the action changes: the ones whose flag is not already the target.
pub fn targets(selected: &[UnitFlags], action: BulkAction) -> Vec<i64> {
    selected
        .iter()
        .filter(|u| match action {
            BulkAction::MarkRead(true) => !u.read,
            BulkAction::MarkRead(false) => u.read || u.progress > 0,
            BulkAction::Bookmark(target) => u.bookmark != target,
            BulkAction::Fillermark(target) => u.fillermark != target,
        })
        .map(|u| u.id)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(id: i64, read: bool, bookmark: bool, fillermark: bool) -> UnitFlags {
        UnitFlags { id, read, bookmark, fillermark, progress: 0 }
    }

    #[test]
    fn bulk_bookmark_targets_only_rows_that_differ() {
        let rows = [unit(1, false, true, false), unit(2, false, false, false), unit(3, true, false, true)];
        assert_eq!(targets(&rows, BulkAction::Bookmark(true)), vec![2, 3]);
        assert_eq!(targets(&rows, BulkAction::Bookmark(false)), vec![1]);
    }

    #[test]
    fn bulk_fillermark_targets_only_rows_that_differ() {
        let rows = [unit(1, false, false, true), unit(2, false, false, false)];
        assert_eq!(targets(&rows, BulkAction::Fillermark(true)), vec![2]);
        assert_eq!(targets(&rows, BulkAction::Fillermark(false)), vec![1]);
    }

    #[test]
    fn bulk_mark_read_and_unread_targets() {
        let rows = [unit(1, true, false, false), unit(2, false, false, false)];
        assert_eq!(targets(&rows, BulkAction::MarkRead(true)), vec![2]);
        assert_eq!(targets(&rows, BulkAction::MarkRead(false)), vec![1]);
        assert!(targets(&[], BulkAction::MarkRead(true)).is_empty());
    }

    #[test]
    fn bulk_unread_also_targets_a_started_unit() {
        let started = UnitFlags { progress: 12, ..unit(1, false, false, false) };
        assert_eq!(targets(&[started, unit(2, false, false, false)], BulkAction::MarkRead(false)), vec![1]);
    }

    #[derive(Clone)]
    struct Row {
        id: i64,
        selected: bool,
    }

    impl Selectable for Row {
        fn id(&self) -> i64 {
            self.id
        }
        fn selected(&self) -> bool {
            self.selected
        }
        fn set_selected(&mut self, selected: bool) {
            self.selected = selected;
        }
    }

    fn selected_rows() -> Vec<Row> {
        vec![Row { id: 1, selected: true }, Row { id: 2, selected: true }]
    }

    #[test]
    fn bulk_bookmark_clears_the_selection_on_every_outcome() {
        for result in [Ok(()), Err("disk full".to_owned())] {
            let mut rows = selected_rows();
            let toast = after_bulk(BulkAction::Bookmark(true), result.clone(), &mut Selection::default(), &mut rows);
            assert!(rows.iter().all(|r| !r.selected));
            assert_eq!(toast.is_some(), result.is_err());
        }
    }

    #[test]
    fn bulk_fillermark_clears_the_selection_on_failure_too() {
        let mut rows = selected_rows();
        assert!(after_bulk(BulkAction::Fillermark(true), Err("x".into()), &mut Selection::default(), &mut rows).is_some());
        assert!(rows.iter().all(|r| !r.selected));
    }

    #[test]
    fn bulk_mark_read_keeps_the_selection_on_failure() {
        let mut rows = selected_rows();
        assert!(after_bulk(BulkAction::MarkRead(true), Err("x".into()), &mut Selection::default(), &mut rows).is_some());
        assert!(rows.iter().all(|r| r.selected));
        assert_eq!(after_bulk(BulkAction::MarkRead(true), Ok(()), &mut Selection::default(), &mut rows), None);
        assert!(rows.iter().all(|r| !r.selected));
    }

    #[test]
    fn bulk_apply_writes_one_batch_and_unread_resets_progress() {
        use kuro_data::chapter::Chapter;
        use kuro_data::manga::{Manga, MangaRepo};
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
        let db = Arc::new(Db::new(&path, writer).expect("manga db handle"));
        let manga = MangaRepo::new(db.clone())
            .insert(&Manga { favorite: true, source: 1, url: "m".into(), title: "M".into(), ..Manga::create() })
            .expect("insert manga");
        let chapter = |url: &str| Chapter { manga_id: manga, url: url.into(), name: url.into(), read: true, last_page_read: 9, ..Chapter::create() };
        let added = ChapterRepo::new(db.clone()).add_all(&[chapter("a"), chapter("b")]).expect("add chapters");
        let ids: Vec<i64> = added.iter().map(|c| c.id).collect();

        apply(&db, Kind::Manga, &ids, BulkAction::Bookmark(true)).expect("bookmark");
        apply(&db, Kind::Manga, &ids[..1], BulkAction::MarkRead(false)).expect("unread");
        assert_eq!(apply(&db, Kind::Manga, &[], BulkAction::Bookmark(true)), Ok(()));
        let repo = ChapterRepo::new(db);
        let first = repo.by_id(ids[0]).expect("read").expect("stored");
        let second = repo.by_id(ids[1]).expect("read").expect("stored");
        assert!(first.bookmark && second.bookmark);
        assert!(!first.read && first.last_page_read == 0);
        assert!(second.read && second.last_page_read == 9);
    }
}
