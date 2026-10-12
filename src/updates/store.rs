//! The Updates feed against a real (temporary) manga database (T-42b-044): the edge cases E-U1 to E-U5 of the feed query,
//! on the views of `kuro-db`, the fixture `tests/fixtures/updates_upcoming.sql`, and E-U33 (the downloads of newly read
//! rows go, grouped per entry) with a fake download module. The file holds tests only; the query and the repository
//! are in `kuro_domain::updates`. The calendar cases E-C1 to E-C5 belong to `src/upcoming`, which does not exist yet.

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use kuro_data::entry::{Entry, EntryRepo};
    use kuro_data::item::{Item, ItemRepo};
    use kuro_db::handle::Db;
    use kuro_platform::kind::Kind;
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
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
        let db = Arc::new(Db::new(&path, writer).expect("manga db handle"));
        Fixture { _dir: dir, db }
    }

    /// A favourite manga added at `ADDED`, unless `favorite` is false.
    fn manga(db: &Arc<Db>, title: &str, favorite: bool) -> i64 {
        EntryRepo::new(db.clone())
            .insert(&Entry { favorite, date_added: ADDED, source: 1, url: title.into(), title: title.into(), ..Entry::create(Kind::Manga) })
            .expect("insert manga")
    }

    fn chapter(manga_id: i64, name: &str, date_fetch: i64, date_upload: i64) -> Item {
        Item { entry_id: manga_id, url: name.into(), name: name.into(), date_fetch, date_upload, ..Item::create(Kind::Manga) }
    }

    fn add(db: &Arc<Db>, chapters: &[Item]) -> Vec<i64> {
        ItemRepo::new(db.clone()).add_all(chapters).expect("add chapters").iter().map(|c| c.id).collect()
    }

    fn feed(db: &Arc<Db>) -> Vec<i64> {
        UpdatesRepo::new(db.clone()).recent(Kind::Manga, AFTER).expect("feed").iter().map(|r| r.item_id).collect()
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
    fn updates_with_one_fetch_date_come_back_newest_id_first() {
        // the order is the SQL `datefetch DESC, itemId DESC`; there is no re-sort in Rust
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let ids = add(&f.db, &[chapter(m, "a", ADDED + 1, AFTER + 10), chapter(m, "b", ADDED + 1, AFTER + 10), chapter(m, "c", ADDED + 2, AFTER + 10)]);
        assert_eq!(feed(&f.db), vec![ids[2], ids[1], ids[0]]);
    }

    #[test]
    fn updates_limit_500_keeps_at_most_500_rows() {
        // E-U5 (count): more than 500 qualifying rows give 500 rows.
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let rows: Vec<Item> = (0..520).map(|i| chapter(m, &format!("c{i}"), ADDED + 1 + i, AFTER + 1 + i)).collect();
        add(&f.db, &rows);
        assert_eq!(feed(&f.db).len(), 500);
    }

    #[test]
    fn updates_limit_500_by_fetch_order() {
        // E-U5 (order): the 500 most recently fetched rows are kept, not 500 arbitrary ones.
        let f = fixture();
        let m = manga(&f.db, "M", true);
        let rows: Vec<Item> = (0..520).map(|i| chapter(m, &format!("c{i}"), ADDED + 1 + i, AFTER + 1 + i)).collect();
        let ids = add(&f.db, &rows);
        let kept = feed(&f.db);
        assert_eq!(kept.len(), 500);
        assert!(!kept.contains(&ids[0]), "the oldest fetch is outside the newest 500");
        assert!(kept.contains(&ids[519]));
    }

    #[test]
    fn updates_fixture_file_gives_the_edge_cases_e_u1_to_e_u3() {
        // E-U1 (unknown upload date), E-U2 (backfill) and E-U3 (not a favourite) against the shared fixture file.
        let f = fixture();
        f.db.write(|tx| {
            tx.execute_batch(include_str!("../../tests/fixtures/updates_upcoming.sql"))?;
            Ok(())
        })
        .expect("load the fixture");
        assert_eq!(feed(&f.db), vec![1], "only C1 is an update");
    }

    #[test]
    fn updates_remove_after_read_only_newly_read() {
        // E-U33: marking read removes the downloads of the newly read chapters, grouped per manga; a chapter that
        // was already read is not touched.
        use crate::updates::bulk::{self, BulkAction};
        use crate::updates::download_actions::UnitTarget;
        use crate::updates::download_exec::remove_read_downloads;
        use crate::updates::download_host::DownloadBackend as _;
        use crate::updates::download_host::fake::{Call, Fake};
        use crate::updates::download_state::DownloadState;
        use crate::updates::feed::{self, rederive};
        use crate::updates::selection::Selection;

        let f = fixture();
        let (a, b) = (manga(&f.db, "A", true), manga(&f.db, "B", true));
        let ids = add(
            &f.db,
            &[
                chapter(a, "d1", ADDED + 1, AFTER + 10),
                chapter(a, "d2", ADDED + 2, AFTER + 10),
                chapter(a, "d3", ADDED + 3, AFTER + 10),
                chapter(b, "d4", ADDED + 4, AFTER + 10),
            ],
        );
        // d2 is already read; d1, d2 and d4 have files on disk (the fake matches `c<id>`, so name them by id)
        bulk::apply(&f.db, Kind::Manga, &[ids[1]], BulkAction::MarkRead(true)).expect("pre-read");
        f.db.write(|tx| {
            for (id, name) in ids.iter().zip(["c1", "c2", "c3", "c4"]) {
                tx.execute("UPDATE items SET name = ?1 WHERE _id = ?2", (name, id))?;
            }
            Ok(())
        })
        .expect("rename");
        let fake = Fake::installed();
        *fake.disk.lock().unwrap() = vec![1, 2, 4];

        let rows = UpdatesRepo::new(f.db.clone()).recent(Kind::Manga, AFTER).expect("feed");
        let mut items = feed::replace_rows(rows, &[], &mut Selection::default());
        rederive(&mut items, |_| None, &mut Default::default(), |key| fake.on_disk(Kind::Manga, key));
        for item in &mut items {
            item.selected = true;
        }
        let unit_of = |id: i64| items.iter().find(|r| r.row.item_id == id).expect("row");
        assert_eq!(unit_of(ids[0]).download.state, DownloadState::Downloaded);
        assert_eq!(unit_of(ids[2]).download.state, DownloadState::NotDownloaded);

        let changed = bulk::targets(&feed::selected_flags(&items), BulkAction::MarkRead(true));
        assert!(!changed.contains(&ids[1]), "the already read chapter is not written");
        bulk::apply(&f.db, Kind::Manga, &changed, BulkAction::MarkRead(true)).expect("mark read");
        let newly_read: Vec<UnitTarget> = items
            .iter()
            .filter(|r| changed.contains(&r.row.item_id))
            .map(|r| UnitTarget { entry_id: r.row.entry_id, unit_id: r.row.item_id, state: r.download.state })
            .collect();

        remove_read_downloads(&fake, Kind::Manga, true, &newly_read);
        let mut calls = fake.calls();
        calls.sort_by_key(|c| match c {
            Call::Delete { entry_id, .. } => *entry_id,
            _ => i64::MAX,
        });
        assert_eq!(
            calls,
            vec![Call::Delete { entry_id: a, unit_ids: vec![ids[0]] }, Call::Delete { entry_id: b, unit_ids: vec![ids[3]] }],
            "grouped per manga; d2 was already read and d3 has no files"
        );
        let all_read: Vec<bool> = UpdatesRepo::new(f.db.clone())
            .recent(Kind::Manga, AFTER)
            .expect("feed")
            .iter()
            .map(|r| r.done)
            .collect();
        assert!(all_read.iter().all(|read| *read), "every selected chapter is now read");
    }
}
