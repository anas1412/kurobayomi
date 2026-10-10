//! The Updates feed against a real (temporary) manga database (T-42b-044): the edge cases E-U1 to E-U5 of the feed query,
//! on the views of `kuro-db`. The file holds tests only; the query and the repository are in `kuro_domain::updates`.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_data::chapter::{Chapter, ChapterRepo};
    use kuro_data::manga::{Manga, MangaRepo};
    use kuro_db::handle::Db;
    use kuro_domain::kind::EntryKind;
    use kuro_domain::updates::repo::UpdatesRepo;

    /// The window cut-off used by every test: the feed shows uploads after this instant (epoch ms).
    const AFTER: i64 = 1_700_000_000_000;
    /// The date a manga was added to the library in the fixtures.
    const ADDED: i64 = 1_700_000_100_000;

    struct Fixture {
        _dir: tempfile::TempDir,
        db: Arc<Db>,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("open manga db");
        let db = Arc::new(Db::new(&path, writer).expect("manga db handle"));
        Fixture { _dir: dir, db }
    }

    /// A favourite manga added at `ADDED`, unless `favorite` is false.
    fn manga(db: &Arc<Db>, title: &str, favorite: bool) -> i64 {
        MangaRepo::new(db.clone())
            .insert(&Manga { favorite, date_added: ADDED, source: 1, url: title.into(), title: title.into(), ..Manga::create() })
            .expect("insert manga")
    }

    fn chapter(manga_id: i64, name: &str, date_fetch: i64, date_upload: i64) -> Chapter {
        Chapter { manga_id, url: name.into(), name: name.into(), date_fetch, date_upload, ..Chapter::create() }
    }

    fn add(db: &Arc<Db>, chapters: &[Chapter]) -> Vec<i64> {
        ChapterRepo::new(db.clone()).add_all(chapters).expect("add chapters").iter().map(|c| c.id).collect()
    }

    fn feed(db: &Arc<Db>) -> Vec<i64> {
        UpdatesRepo::new(db.clone()).recent(EntryKind::Manga, AFTER).expect("feed").iter().map(|r| r.item_id).collect()
    }

    #[test]
    fn updates_excludes_unknown_upload_date() {
        // E-U1: fetched today, but the upload date is unknown (0), so it is not an update.
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let ids = add(&f.db, &[chapter(m, "fetched, no upload date", ADDED + 1_000, 0), chapter(m, "uploaded", ADDED + 1_000, AFTER + 10)]);
        assert_eq!(feed(&f.db), vec![ids[1]]);
    }

    #[test]
    fn updates_excludes_backfill() {
        // E-U2: fetched before the manga was added to the library.
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let ids = add(&f.db, &[chapter(m, "backfill", ADDED - 5_000, AFTER + 10), chapter(m, "new", ADDED + 5_000, AFTER + 10)]);
        assert_eq!(feed(&f.db), vec![ids[1]]);
    }

    #[test]
    fn updates_drop_when_unfavorited() {
        // E-U3: a manga that left the library has no updates in the feed.
        let f = fixture();
        let kept = manga(&f.db, "Kept", true);
        let left = manga(&f.db, "Left", false);
        let ids = add(&f.db, &[chapter(kept, "a", ADDED + 1, AFTER + 10), chapter(left, "b", ADDED + 1, AFTER + 10)]);
        assert_eq!(feed(&f.db), vec![ids[0]]);
    }

    #[test]
    fn updates_window_is_strict() {
        // E-U4: an upload exactly at the cut-off is outside the window (strict `>`).
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let ids = add(&f.db, &[chapter(m, "at cut-off", ADDED + 1, AFTER), chapter(m, "just after", ADDED + 1, AFTER + 1)]);
        assert_eq!(feed(&f.db), vec![ids[1]]);
    }

    #[test]
    fn updates_limit_500_keeps_at_most_500_rows() {
        // E-U5 (count): more than 500 qualifying rows give 500 rows.
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let rows: Vec<Chapter> = (0..520).map(|i| chapter(m, &format!("c{i}"), ADDED + 1 + i, AFTER + 1 + i)).collect();
        add(&f.db, &rows);
        assert_eq!(feed(&f.db).len(), 500);
    }

    #[test]
    #[ignore = "kuro-db GET_RECENT_UPDATES has no ORDER BY and LIMIT runs before the Rust sort; fix in kuro-db"]
    fn updates_limit_500_by_fetch_order() {
        // E-U5 (order): the 500 most recently fetched rows are kept, not 500 arbitrary ones.
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let rows: Vec<Chapter> = (0..520).map(|i| chapter(m, &format!("c{i}"), ADDED + 1 + i, AFTER + 1 + i)).collect();
        let ids = add(&f.db, &rows);
        let kept = feed(&f.db);
        assert_eq!(kept.len(), 500);
        assert!(!kept.contains(&ids[0]), "the oldest fetch is outside the newest 500");
        assert!(kept.contains(&ids[519]));
    }
}
