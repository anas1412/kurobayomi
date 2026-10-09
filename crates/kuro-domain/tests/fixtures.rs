//! The committed SQL fixtures loaded into a migrated manga database, checked against the repositories (T-08a-040).
//! The expected values come from the fixture comments, not from running the code.

use std::sync::Arc;

use kuro_data::manga::MangaRepo;
use kuro_db::handle::Db;
use kuro_domain::category::repo::CategoryRepo;
use kuro_domain::history::model::HistoryWithRelations;
use kuro_domain::history::repo::HistoryRepo;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::repo::LibraryRepo;
use kuro_domain::updates::model::UpdateRow;
use kuro_domain::updates::repo::UpdatesRepo;
use tempfile::TempDir;

const LIBRARY_SMALL: &str = include_str!("library_small.sql");
const HISTORY_BASIC: &str = include_str!("history_basic.sql");
const UPDATES_WINDOW: &str = include_str!("updates_window.sql");

/// A migrated manga database with one fixture loaded. Keep the `TempDir` alive for the test.
fn fixture(sql: &str) -> (TempDir, Arc<Db>) {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("tachiyomi.db");
    let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
    let db = Arc::new(Db::new(&path, writer).expect("manga handle"));
    db.write(|tx| Ok(tx.execute_batch(sql)?)).expect("the fixture loads");
    (dir, db)
}

#[test]
fn library_small_has_three_favourites_and_hides_the_hidden_category() {
    let (_d, db) = fixture(LIBRARY_SMALL);
    let favourites = MangaRepo::new(db.clone()).favorites().expect("favourites");
    assert_eq!(favourites.len(), 3, "entry 4 is not a favourite");

    let library = LibraryRepo::new(db.clone()).all(EntryKind::Manga).expect("library rows");
    let mut ids: Vec<i64> = library.iter().map(|r| r.id).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids, vec![1, 2, 3]);
    let default_page: Vec<i64> = library.iter().filter(|r| r.category == 0).map(|r| r.id).collect();
    assert_eq!(default_page, vec![3], "Gamma has no category row, so it is in Default");

    let categories = CategoryRepo::new(db.clone(), EntryKind::Manga);
    let names = |rows: Vec<kuro_domain::category::model::Category>| rows.into_iter().map(|c| c.name).collect::<Vec<_>>();
    assert_eq!(names(categories.by_entry(2, false).expect("all")), ["Reading", "Hold"]);
    assert_eq!(names(categories.by_entry(2, true).expect("visible")), ["Reading"]);
}

#[test]
fn history_basic_lists_the_newest_row_per_entry_and_a_reset_reveals_the_previous_one() {
    let (_d, db) = fixture(HISTORY_BASIC);
    let repo = HistoryRepo::new(db.clone());
    let items = |rows: Vec<HistoryWithRelations>| rows.iter().map(|r| r.item_id).collect::<Vec<_>>();
    assert_eq!(items(repo.list(EntryKind::Manga, "").expect("list")), vec![2, 3], "[M.c2, N.n1]");

    repo.reset_one(EntryKind::Manga, 2).expect("reset the M c2 row");
    assert_eq!(items(repo.list(EntryKind::Manga, "").expect("list")), vec![3, 1], "[N.n1, M.c1]");
}

#[test]
fn updates_window_keeps_favourite_chapters_fetched_after_their_entry_was_added() {
    let (_d, db) = fixture(UPDATES_WINDOW);
    let repo = UpdatesRepo::new(db.clone());
    let names = |rows: Vec<UpdateRow>| rows.into_iter().map(|r| r.item_name).collect::<Vec<_>>();
    assert_eq!(names(repo.recent(EntryKind::Manga, 100).expect("feed")), ["N 2", "M 2"]);
    assert_eq!(names(repo.recent(EntryKind::Manga, 300).expect("feed")), ["N 2"]);
}
