//! The reads behind a history resume (T-42a-015, T-42a-016): the entry's units in reading order, and the unit a
//! resume opens. The order is the entry's own sort in ascending reading order, as `GetNextChapters` reads it; the
//! rule on top is [`crate::updates::history::next_units_from`], which starts at the first unit when the history row's
//! unit is gone (EC-42-H-05).

use std::sync::Arc;

use kuro_data::entry::EntryRepo;
use kuro_data::item::{Item, ItemRepo};
use kuro_db::handle::Db;
use kuro_domain::entries::flags::EntryFlags;
use kuro_domain::history::repo::HistoryRepo;
use kuro_domain::history::view::RemoveAction;
use kuro_domain::items::facades::get_items_by_entry;
use kuro_domain::items::sorter::item_cmp;
use kuro_platform::kind::Kind;

use crate::updates::history::{Resume, next_units_from, resume};

/// A chapter or episode as a resume needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Unit {
    pub id: i64,
    pub name: String,
    pub done: bool,
}

/// The units of one entry in ascending reading order. An unknown entry gives an empty list and a log line.
pub fn ordered_units(db: &Arc<Db>, kind: Kind, entry_id: i64) -> Vec<Unit> {
    let entry = match EntryRepo::new(db.clone()).by_id(entry_id) {
        Ok(e) if e.kind == kind => e,
        _ => {
            tracing::error!(entry_id, %kind, "cannot read the entry for resume");
            return Vec::new();
        }
    };
    let mut items: Vec<Item> = get_items_by_entry(&ItemRepo::new(db.clone()), entry_id, true);
    items.sort_by(item_cmp(entry.sorting(), false));
    items.into_iter().map(|i| Unit { id: i.id, name: i.name, done: i.done }).collect()
}

/// What a click on a history row of `entry_id` (whose row names `from_item`) does.
pub fn resume_row(db: &Arc<Db>, kind: Kind, entry_id: i64, from_item: i64) -> Resume<Unit> {
    let units = ordered_units(db, kind, entry_id);
    resume(next_units_from(&units, |u| u.id, |u| u.done, from_item, false), kind)
}

/// The write of a confirmed remove dialog: one row reset, or every row of the entry (the checkbox).
pub fn apply_remove(db: &Arc<Db>, kind: Kind, action: RemoveAction) -> Result<(), String> {
    let repo = HistoryRepo::new(db.clone());
    match action {
        RemoveAction::ResetOne { history_id } => repo.reset_one(kind, history_id),
        RemoveAction::ResetEntry { entry_id } => repo.reset_entry(kind, entry_id),
    }
    .map_err(|e| e.to_string())
}

/// The title of an entry, for the reader or player page. `None` when the entry is gone.
pub fn entry_title(db: &Arc<Db>, kind: Kind, id: i64) -> Option<String> {
    EntryRepo::new(db.clone()).by_id(id).ok().filter(|e| e.kind == kind).map(|e| e.title)
}

#[cfg(test)]
mod tests {
    use kuro_data::entry::Entry;
    use super::*;

    /// A manga of `n` chapters whose reading order is the insert order; the first `read` of them are read.
    fn manga_with(n: i64, read: i64) -> (tempfile::TempDir, Arc<Db>, i64, Vec<i64>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
        let db = Arc::new(Db::new(&path, writer).expect("manga db handle"));
        let manga = EntryRepo::new(db.clone())
            .insert(&Entry { favorite: true, source: 1, url: "m".into(), title: "M".into(), ..Entry::create(Kind::Manga) })
            .expect("insert manga");
        let chapters: Vec<Item> = (0..n)
            .map(|i| Item {
                entry_id: manga,
                url: format!("c{i}"),
                name: format!("Chapter {i}"),
                number: i as f64,
                source_order: n - i,
                done: i < read,
                ..Item::create(Kind::Manga)
            })
            .collect();
        let ids = ItemRepo::new(db.clone()).add_all(&chapters).expect("add").into_iter().map(|c| c.id).collect();
        (dir, db, manga, ids)
    }

    #[test]
    fn history_resume_opens_the_first_unread_unit_after_a_read_one() {
        let (_dir, db, manga, ids) = manga_with(4, 2);
        let Resume::Open(unit) = resume_row(&db, Kind::Manga, manga, ids[1]) else { panic!("a unit follows") };
        assert_eq!(unit.id, ids[2]);
    }

    #[test]
    fn history_resume_with_a_deleted_unit_starts_at_the_first_unit() {
        let (_dir, db, manga, _ids) = manga_with(3, 0);
        let Resume::Open(unit) = resume_row(&db, Kind::Manga, manga, 9_999) else { panic!("units exist") };
        assert_eq!(unit.name, "Chapter 0");
    }

    #[test]
    fn history_resume_after_the_last_read_unit_is_not_found() {
        let (_dir, db, manga, ids) = manga_with(2, 2);
        assert!(matches!(resume_row(&db, Kind::Manga, manga, ids[1]), Resume::NotFound { .. }));
    }

    #[test]
    fn history_remove_resets_one_row_or_the_whole_entry() {
        let (_dir, db, manga, ids) = manga_with(3, 0);
        let repo = HistoryRepo::new(db.clone());
        repo.upsert(Kind::Manga, ids[0], 1_000, 5).expect("record");
        repo.upsert(Kind::Manga, ids[1], 2_000, 5).expect("record");
        let rows = repo.by_entry(Kind::Manga, manga).expect("rows");
        let first = rows.iter().find(|r| r.item_id == ids[0]).expect("row").id;
        apply_remove(&db, Kind::Manga, RemoveAction::ResetOne { history_id: first }).expect("reset one");
        let live: Vec<i64> = repo.by_entry(Kind::Manga, manga).expect("rows").iter().filter(|r| r.at > 0).map(|r| r.item_id).collect();
        assert_eq!(live, vec![ids[1]], "the other row stays");
        apply_remove(&db, Kind::Manga, RemoveAction::ResetEntry { entry_id: manga }).expect("reset entry");
        assert!(repo.by_entry(Kind::Manga, manga).expect("rows").iter().all(|r| r.at == 0));
    }

}
