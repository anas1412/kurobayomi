//! Repository tests against real files: the trigger matrix (T-05a-024), the transaction behaviour (T-05a-028),
//! duplicates and upcoming queries (T-05a-019, T-05a-020), item reads and keys (T-05a-021, T-05a-031), and the live
//! queries. Both kinds run through the one `EntryRepo` and `ItemRepo`.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use kuro_db::codecs::UpdateStrategy;
use kuro_db::handle::Db;
use kuro_db::codec::{FetchType, Memo};
use kuro_platform::kind::Kind;
use rusqlite::params;

use crate::DataError;
use crate::entry::{Entry, EntryRepo, EntryUpdate};
use crate::item::{Item, ItemRepo, ItemUpdate};
use crate::support::{entry as new_entry, insert_category, insert_entry, insert_item, item};
use crate::time;

pub(crate) use crate::support::test_db;

fn new_manga(url: &str, title: &str, favorite: bool) -> Entry {
    new_entry(Kind::Manga, url, title, favorite)
}

fn new_anime(url: &str, title: &str, favorite: bool) -> Entry {
    new_entry(Kind::Anime, url, title, favorite)
}

/// Stores one item and returns its id.
fn put(db: &Arc<Db>, it: Item) -> i64 {
    ItemRepo::new(Arc::clone(db)).add_all(&[it]).expect("add item")[0].id
}

/// Makes every update of `table` row `id` abort, so a write in a transaction fails halfway.
fn abort_updates_of(db: &Db, table: &str, id: i64) {
    db.write(|tx| {
        tx.execute_batch(&format!(
            "CREATE TRIGGER abort_{table}_{id} BEFORE UPDATE ON {table} WHEN old._id = {id} \
             BEGIN SELECT RAISE(ABORT, 'injected'); END;"
        ))?;
        Ok(())
    })
    .expect("install trigger");
}

fn version(db: &Arc<Db>, id: i64) -> i64 {
    EntryRepo::new(Arc::clone(db)).by_id(id).unwrap().version
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs() as i64
}

mod entry_repo {
    use super::*;

    #[test]
    fn insert_returns_a_new_id_and_ignores_the_input_id() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        for kind in Kind::ALL {
            let mut e = new_entry(kind, "u1", "One", true);
            e.id = 99;
            e.last_modified_at = 123;
            let id = repo.insert(&e).unwrap();
            assert_ne!(id, 99);
            assert_eq!(repo.by_id(id).unwrap(), Entry { id, last_modified_at: 0, ..e });
        }
    }

    #[test]
    fn by_id_missing_is_not_found() {
        let (_d, db) = test_db();
        assert!(matches!(EntryRepo::new(db).by_id(404), Err(DataError::NotFound)));
    }

    /// T-43b-011, E-20b: the reading mode is bits 0..2 and the orientation bits 3..5 of `entries.viewer`. Writing one
    /// group keeps the other, and the column value round-trips through the database unchanged.
    #[test]
    fn viewer_bits_write_keeps_the_other_group_and_the_column_round_trips() {
        const MODE: i64 = 0x7;
        const ORIENTATION: i64 = 0x38;
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry { viewer_flags: 0b101_010, ..new_manga("v", "V", true) }).unwrap();
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b101_010, "stored as given");

        assert!(repo.set_viewer_bits(id, 0b011, MODE).unwrap());
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b101_011, "mode replaced, orientation bits kept");

        // A value wider than the mask cannot leak into the other group.
        assert!(repo.set_viewer_bits(id, 0b111_100, MODE).unwrap());
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b101_100);

        assert!(repo.set_viewer_bits(id, 0b010 << 3, ORIENTATION).unwrap());
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b010_100, "orientation replaced, mode kept");

        // An unrelated update leaves the column alone.
        repo.update(&EntryUpdate { title: Some("W".into()), ..EntryUpdate::new(id) }).unwrap();
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b010_100);
        assert!(!repo.set_viewer_bits(id + 1, 1, MODE).unwrap(), "no such entry");
    }

    #[test]
    fn watched_not_in_library_lists_non_favourites_with_progress() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let fav = repo.insert(&new_entry(kind, "f", "Fav", true)).unwrap();
            let done = repo.insert(&new_entry(kind, "r", "Done", false)).unwrap();
            let progress = repo.insert(&new_entry(kind, "p", "Progress", false)).unwrap();
            repo.insert(&new_entry(kind, "n", "None", false)).unwrap();
            put(&db, Item { done: true, ..item(kind, done, "r1", None) });
            put(&db, Item { progress: 5, ..item(kind, progress, "p1", None) });
            put(&db, Item { done: true, ..item(kind, fav, "f1", None) });
            assert_eq!(repo.favorites(kind).unwrap().iter().map(|e| e.id).collect::<Vec<_>>(), vec![fav], "{kind}");
            let mut watched: Vec<i64> = repo.watched_not_in_library(kind).unwrap().iter().map(|e| e.id).collect();
            watched.sort_unstable();
            assert_eq!(watched, vec![done, progress], "{kind}");
        }
    }

    #[test]
    fn update_keeps_none_fields_and_a_missing_id_is_not_an_error() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let mut e = new_manga("u", "A", false);
        e.artist = Some("Artist".into());
        let id = repo.insert(&e).unwrap();
        repo.update(&EntryUpdate { title: Some("B".into()), ..EntryUpdate::new(id) }).unwrap();
        let back = repo.by_id(id).unwrap();
        assert_eq!(back.title, "B");
        assert_eq!(back.artist.as_deref(), Some("Artist"));
        repo.update(&EntryUpdate::new(9_999)).unwrap();
    }

    #[test]
    fn update_sets_parent_id_but_never_clears_it() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&new_anime("u", "T", false)).unwrap();
        repo.update(&EntryUpdate { parent_id: Some(7), ..EntryUpdate::new(id) }).unwrap();
        repo.update(&EntryUpdate { title: Some("New".into()), ..EntryUpdate::new(id) }).unwrap();
        let back = repo.by_id(id).unwrap();
        assert_eq!((back.parent_id, back.title.as_str()), (Some(7), "New"));
    }

    #[test]
    fn update_all_rolls_back_every_row_when_one_fails() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let a = repo.insert(&new_entry(kind, "a", "A", false)).unwrap();
            let b = repo.insert(&new_entry(kind, "b", "B", false)).unwrap();
            abort_updates_of(&db, "entries", b);
            let result = repo.update_all(&[
                EntryUpdate { title: Some("X".into()), ..EntryUpdate::new(a) },
                EntryUpdate { title: Some("Y".into()), ..EntryUpdate::new(b) },
            ]);
            assert!(result.is_err());
            assert_eq!(repo.by_id(a).unwrap().title, "A", "{kind}");
        }
    }

    #[test]
    fn reset_viewer_flags_zeroes_the_rows_of_the_kind_only() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let m = repo.insert(&Entry { viewer_flags: 5, ..new_manga("u", "A", false) }).unwrap();
        let a = repo.insert(&Entry { viewer_flags: 9, ..new_anime("a", "A", false) }).unwrap();
        repo.reset_viewer_flags(Kind::Manga).unwrap();
        assert_eq!(repo.by_id(m).unwrap().viewer_flags, 0);
        assert_eq!(repo.by_id(a).unwrap().viewer_flags, 9);
        repo.reset_viewer_flags(Kind::Anime).unwrap();
        assert_eq!(repo.by_id(a).unwrap().viewer_flags, 0);
    }

    #[test]
    fn library_has_one_row_per_category_link_and_unlinked_favourites_sit_at_zero() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let (cat_a, cat_b) = (insert_category(&db, kind, "A", 1), insert_category(&db, kind, "B", 2));
            let both = repo.insert(&new_entry(kind, "both", "Both", true)).unwrap();
            let loose = repo.insert(&new_entry(kind, "loose", "Loose", true)).unwrap();
            let hidden = repo.insert(&new_entry(kind, "hidden", "Hidden", false)).unwrap();
            repo.set_categories(both, &[cat_a, cat_b]).unwrap();
            repo.set_categories(hidden, &[cat_a]).unwrap();

            let rows = repo.library(kind).unwrap();
            assert_eq!(rows.iter().filter(|r| r.id() == both).count(), 2, "{kind}");
            assert_eq!(rows.iter().filter(|r| r.id() == loose).map(|r| r.category).collect::<Vec<_>>(), vec![0]);
            assert!(rows.iter().all(|r| r.id() != hidden));
            let categories: std::collections::BTreeSet<i64> = rows.iter().map(|r| r.category).collect();
            assert_eq!(categories.into_iter().collect::<Vec<_>>(), vec![0, cat_a, cat_b]);
        }
    }

    #[test]
    fn library_counts_ignore_items_of_excluded_scanlators() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let id = repo.insert(&new_manga("m", "M", true)).unwrap();
        put(&db, Item { done: true, ..item(Kind::Manga, id, "c1", Some("S1")) });
        put(&db, item(Kind::Manga, id, "c2", Some("S2")));
        repo.set_excluded_scanlators(id, &["S2".to_owned()].into()).unwrap();
        let row = repo.library(Kind::Manga).unwrap().into_iter().find(|r| r.id() == id).unwrap();
        assert_eq!((row.total, row.done, row.pending()), (1, 1, 0));
        assert!(row.has_started());
    }

    #[test]
    fn set_categories_dedupes_bumps_once_per_link_and_rolls_back_on_a_missing_category() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let (a, b, c) = (insert_category(&db, kind, "A", 1), insert_category(&db, kind, "B", 2), insert_category(&db, kind, "C", 3));
            let id = repo.insert(&new_entry(kind, "m", "M", true)).unwrap();
            repo.set_categories(id, &[a, a, b, c]).unwrap();
            assert_eq!(version(&db, id), 3, "{kind}: one bump per distinct link");
            repo.set_categories(id, &[]).unwrap();
            assert_eq!(version(&db, id), 3);
            repo.set_categories(id, &[a]).unwrap();
            assert_eq!(version(&db, id), 4);
            assert!(repo.set_categories(id, &[b, 999]).is_err());
            let linked: Vec<i64> = repo.library(kind).unwrap().iter().map(|r| r.category).collect();
            assert_eq!(linked, vec![a]);
        }
    }

    #[test]
    fn duplicates_match_unicode_titles_and_skip_self_and_non_favourites() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        for kind in Kind::ALL {
            let first = repo.insert(&new_entry(kind, "a", "Éclair", true)).unwrap();
            let second = repo.insert(&new_entry(kind, "b", "éCLAIR", true)).unwrap();
            repo.insert(&new_entry(kind, "c", "éclair", false)).unwrap();
            let found: Vec<i64> = repo.duplicates(kind, second, "éCLAIR").unwrap().iter().map(|m| m.id).collect();
            assert_eq!(found, vec![first], "{kind}");
            assert!(repo.duplicates(kind, first, "éclair").unwrap().iter().all(|m| m.id != first));
        }
    }

    #[test]
    fn upcoming_lists_favourites_from_local_midnight_soonest_first() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let now = Local::now().timestamp_millis();
        let start = time::start_of_local_day_millis(now);
        for kind in Kind::ALL {
            let with = |url: &str, fav: bool, status: i64, next: i64| Entry { status, next_update: next, ..new_entry(kind, url, url, fav) };
            let statuses = kind.upcoming_statuses();
            let last = *statuses.last().unwrap();
            let later = repo.insert(&with("later", true, statuses[0], start + 3_600_000)).unwrap();
            let soon = repo.insert(&with("soon", true, last, start + 1_000)).unwrap();
            repo.insert(&with("wrong-status", true, 2, start + 1_000)).unwrap();
            repo.insert(&with("yesterday", true, statuses[0], start - 1)).unwrap();
            repo.insert(&with("not-favourite", false, statuses[0], start + 5)).unwrap();
            let ids: Vec<i64> = repo.upcoming(kind, now).unwrap().iter().map(|m| m.id).collect();
            assert_eq!(ids, vec![soon, later], "{kind}");
        }
    }

    #[test]
    fn clear_non_library_never_deletes_a_favourite_and_an_empty_list_is_a_no_op() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let fav = repo.insert(&new_manga("f", "F", true)).unwrap();
        let non = repo.insert(&new_manga("n", "N", false)).unwrap();
        assert_eq!(repo.clear_non_library(Kind::Manga, &[]).unwrap(), 0);
        assert!(repo.by_id(non).is_ok(), "an empty list deletes nothing");
        repo.clear_non_library(Kind::Manga, &[1]).unwrap();
        assert!(repo.by_id(fav).is_ok(), "library entries are untouched");
        assert!(matches!(repo.by_id(non), Err(DataError::NotFound)));
    }

    #[test]
    fn source_counts_split_library_and_non_library_rows() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        repo.insert(&new_manga("a", "A", true)).unwrap();
        repo.insert(&new_manga("b", "B", true)).unwrap();
        repo.insert(&Entry { source: 2, ..new_manga("c", "C", false) }).unwrap();
        assert_eq!(repo.favorite_counts_by_source(Kind::Manga).unwrap(), vec![(1, 2)]);
        assert_eq!(repo.non_library_counts_by_source(Kind::Manga).unwrap(), vec![(2, 1)]);
        assert_eq!(repo.all_source_and_url(Kind::Manga).unwrap().len(), 3);
    }

    fn full_anime() -> Entry {
        Entry {
            genre: Some(vec!["Action".into(), "Drama".into()]),
            artist: Some("Artist".into()),
            author: Some("Author".into()),
            description: Some("Desc".into()),
            thumbnail_url: Some("thumb".into()),
            last_update: 10,
            next_update: 20,
            fetch_interval: 7,
            date_added: 30,
            viewer_flags: 2,
            item_flags: 5,
            cover_last_modified: 40,
            update_strategy: UpdateStrategy::OnlyFetchOnce,
            initialized: true,
            version: 5,
            background_last_modified: Some(9),
            background_url: Some("bg".into()),
            fetch_type: Some(FetchType::Seasons),
            parent_id: Some(42),
            season_flags: Some(3),
            season_number: Some(2.5),
            season_source_order: Some(4),
            memo: Some(Memo(serde_json::json!({"k": "v", "n": 1}).as_object().cloned().unwrap())),
            ..new_anime("full", "Full", true)
        }
    }

    #[test]
    fn insert_round_trips_every_column_except_the_ones_the_database_sets() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let expected = full_anime();
        let id = repo.insert(&expected).unwrap();
        assert_eq!(repo.by_id(id).unwrap(), Entry { id, ..expected });
    }

    #[test]
    fn library_stats_switch_by_fetch_type_and_a_seasons_parent_counts_its_children() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let parent = repo.insert(&Entry { fetch_type: Some(FetchType::Seasons), ..new_anime("parent", "Parent", true) }).unwrap();
        let season = |url: &str| Entry { parent_id: Some(parent), fetch_type: Some(FetchType::Episodes), ..new_anime(url, url, false) };
        repo.insert(&season("s0")).unwrap();
        let all_seen = repo.insert(&season("s5")).unwrap();
        let partly = repo.insert(&season("s53")).unwrap();
        let single = repo.insert(&new_anime("single", "Single", true)).unwrap();
        for i in 0..5 {
            put(&db, Item { done: true, ..item(Kind::Anime, all_seen, &format!("a{i}"), None) });
            put(&db, Item { done: i < 3, ..item(Kind::Anime, partly, &format!("p{i}"), None) });
        }
        put(&db, Item { done: true, ..item(Kind::Anime, single, "e1", None) });
        put(&db, item(Kind::Anime, single, "e2", None));
        put(&db, item(Kind::Anime, single, "e3", None));

        let rows = repo.library(Kind::Anime).unwrap();
        let parent_row = rows.iter().find(|r| r.id() == parent).unwrap();
        assert_eq!((parent_row.total, parent_row.done), (3, 2));
        let single_row = rows.iter().find(|r| r.id() == single).unwrap();
        assert_eq!((single_row.total, single_row.done, single_row.pending()), (3, 1, 2));
        assert_eq!(rows.len(), 2, "non-favourite children are not library rows");

        let seasons = repo.seasons_by_parent(parent).unwrap();
        assert_eq!(seasons.len(), 3);
        let partly_row = seasons.iter().find(|s| s.entry.id == partly).unwrap();
        assert_eq!((partly_row.total, partly_row.done), (5, 3));
        assert!(!partly_row.is_done());
        assert_eq!(repo.children_by_parent(parent).unwrap().len(), 3);
    }

    #[test]
    fn season_bookmark_and_fillermark_counts_sum_over_children() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let parent = repo.insert(&Entry { fetch_type: Some(FetchType::Seasons), ..new_anime("p", "P", true) }).unwrap();
        let c1 = repo.insert(&Entry { parent_id: Some(parent), ..new_anime("c1", "C1", false) }).unwrap();
        let c2 = repo.insert(&Entry { parent_id: Some(parent), ..new_anime("c2", "C2", false) }).unwrap();
        put(&db, Item { bookmark: true, ..item(Kind::Anime, c1, "b", None) });
        put(&db, Item { fillermark: true, ..item(Kind::Anime, c2, "f", None) });
        let row = repo.library(Kind::Anime).unwrap().into_iter().find(|r| r.id() == parent).unwrap();
        assert_eq!((row.bookmarks, row.fillermarks), (1, 1));
        assert!(row.has_bookmarks());
    }

    #[test]
    fn deletable_parents_are_non_favourite_top_level_rows() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let fav = repo.insert(&new_anime("fav", "F", true)).unwrap();
        let top = repo.insert(&new_anime("top", "T", false)).unwrap();
        repo.insert(&Entry { parent_id: Some(top), ..new_anime("child", "C", false) }).unwrap();
        let ids: Vec<i64> = repo.deletable_parents(Kind::Anime).unwrap().iter().map(|d| d.entry_id).collect();
        assert_eq!(ids, vec![top]);
        assert!(!ids.contains(&fav));
    }

    #[test]
    fn remove_parent_ids_clears_the_child_link_and_a_deleted_parent_leaves_it_dangling() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let parent = repo.insert(&new_anime("p", "P", false)).unwrap();
        let child = repo.insert(&Entry { parent_id: Some(parent), ..new_anime("c", "C", true) }).unwrap();
        repo.clear_non_library_ids(&[parent], &[]).unwrap();
        assert_eq!(repo.by_id(child).unwrap().parent_id, Some(parent));
        repo.remove_parent_ids(&[child]).unwrap();
        assert_eq!(repo.by_id(child).unwrap().parent_id, None);
    }

    #[test]
    fn remove_parent_ids_clears_links_across_more_than_one_chunk() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let parent = repo.insert(&new_anime("p", "P", false)).unwrap();
        let child_id = repo.insert(&Entry { parent_id: Some(parent), ..new_anime("c", "C", false) }).unwrap();
        // More than one chunk of ids, with the real child last so it sits in the second chunk.
        let mut ids: Vec<i64> = (100_000..101_200).collect();
        ids.push(child_id);
        repo.remove_parent_ids(&ids).unwrap();
        assert_eq!(repo.by_id(child_id).unwrap().parent_id, None);
    }

    #[test]
    fn clear_non_library_ids_never_deletes_a_favourite_and_cascades_to_items_and_links() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let fav = repo.insert(&new_anime("fav", "F", true)).unwrap();
        let non = repo.insert(&new_anime("non", "N", false)).unwrap();
        let cat = insert_category(&db, Kind::Anime, "C", 1);
        put(&db, item(Kind::Anime, non, "e", None));
        db.write(|tx| {
            tx.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'anime', ?2)", params![non, cat])?;
            Ok(())
        })
        .unwrap();
        repo.clear_non_library_ids(&[fav, non], &[]).unwrap();
        assert!(repo.by_id(fav).is_ok());
        assert!(matches!(repo.by_id(non), Err(DataError::NotFound)));
        let (items, links): (i64, i64) = db
            .read(|c| {
                Ok((
                    c.query_row("SELECT count(*) FROM items WHERE entry_id = ?1", [non], |r| r.get(0))?,
                    c.query_row("SELECT count(*) FROM entry_categories WHERE entry_id = ?1", [non], |r| r.get(0))?,
                ))
            })
            .unwrap();
        assert_eq!((items, links), (0, 0));
    }

    #[test]
    fn a_failing_delete_propagates_its_error() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let id = repo.insert(&new_manga("u", "T", false)).unwrap();
        db.write(|tx| {
            tx.execute_batch("CREATE TRIGGER no_delete BEFORE DELETE ON entries BEGIN SELECT RAISE(ABORT, 'injected'); END;")?;
            Ok(())
        })
        .unwrap();
        assert!(repo.clear_non_library(Kind::Manga, &[1]).is_err());
        assert!(repo.by_id(id).is_ok());
    }

    #[test]
    fn a_fetch_type_the_view_does_not_know_reads_zero_stats_instead_of_failing_the_query() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let parent = repo.insert(&new_anime("parent", "Parent", true)).unwrap();
        let child = repo.insert(&Entry { parent_id: Some(parent), ..new_anime("child", "Child", false) }).unwrap();
        put(&db, Item { done: true, ..item(Kind::Anime, parent, "e1", None) });
        db.write(|tx| Ok(tx.execute("UPDATE entries SET fetch_type = 7 WHERE _id IN (?1, ?2)", (parent, child))?)).unwrap();

        let library = repo.library(Kind::Anime).expect("library() succeeds for fetch_type 7");
        let row = library.iter().find(|r| r.id() == parent).unwrap();
        // The unified libraryView rolls up only fetch_type 0 as a container; any other value counts its own items.
        assert_eq!((row.total, row.done, row.bookmarks, row.fillermarks), (1, 1, 0, 0));

        let seasons = repo.seasons_by_parent(parent).expect("seasons_by_parent succeeds for fetch_type 7");
        assert_eq!(seasons.len(), 1);
        assert_eq!((seasons[0].total, seasons[0].done, seasons[0].fillermarks), (0, 0, 0));
    }

    #[test]
    fn a_favourite_without_items_has_zero_aggregates_and_last_activity_comes_from_history() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let bare = repo.insert(&new_manga("bare", "Bare", true)).unwrap();
        let read = repo.insert(&new_manga("read", "Read", true)).unwrap();
        let chapter_id = put(&db, Item { done: true, ..item(Kind::Manga, read, "c1", None) });
        crate::history::HistoryRepo::new(db.clone()).upsert(chapter_id, 4_242, 10).unwrap();

        let rows = repo.library(Kind::Manga).unwrap();
        let bare_row = rows.iter().find(|r| r.id() == bare).unwrap();
        assert_eq!((bare_row.total, bare_row.done, bare_row.bookmarks), (0, 0, 0));
        assert_eq!((bare_row.latest_upload, bare_row.item_fetched_at, bare_row.last_activity), (0, 0, 0));
        let read_row = rows.iter().find(|r| r.id() == read).unwrap();
        assert_eq!(read_row.last_activity, 4_242);
        assert_eq!(read_row.total, 1);
    }

    #[test]
    fn the_related_query_maps_each_row_with_its_group() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let a = repo.insert(&new_anime("a", "A", true)).unwrap();
        let b = repo.insert(&new_anime("b", "B", false)).unwrap();
        let c = repo.insert(&new_anime("c", "C", false)).unwrap();
        db.write(|tx| {
            tx.execute(
                "INSERT INTO anime_relations(entry_id, related_id, name, sort_order, last_fetched_at) \
                 VALUES (?1, ?2, 'Sequels', 0, 1), (?1, ?3, 'Spin-offs', 1, 1)",
                (a, b, c),
            )?;
            Ok(())
        })
        .unwrap();

        let groups = crate::anime_relation::AnimeRelationRepo::new(db.clone()).related(a).unwrap();
        let names: Vec<(String, i64)> = groups.into_iter().flat_map(|g| g.anime.into_iter().map(move |anime| (g.name.clone(), anime.id))).collect();
        assert_eq!(names, vec![("Sequels".to_owned(), b), ("Spin-offs".to_owned(), c)]);
    }

    #[test]
    fn the_create_factory_matches_the_documented_defaults() {
        let a = Entry::create(Kind::Anime);
        assert_eq!((a.id, a.source, a.fetch_type(), a.season_number()), (-1, -1, FetchType::Episodes, -1.0));
        assert!(a.memo().is_empty());
        let m = Entry::create(Kind::Manga);
        assert_eq!((m.id, m.source, m.version, m.update_strategy), (-1, -1, 0, UpdateStrategy::AlwaysUpdate));
    }

    fn set(items: &[&str]) -> std::collections::BTreeSet<String> {
        items.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn excluded_set_makes_the_stored_set_equal_to_the_wanted_one() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry::create(Kind::Manga)).expect("insert");
        repo.set_excluded_scanlators(id, &set(&["A", "B"])).expect("set");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["A", "B"]));
        repo.set_excluded_scanlators(id, &set(&["B", "C"])).expect("replace");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["B", "C"]), "A removed, C added");
        repo.set_excluded_scanlators(id, &set(&[])).expect("clear");
        assert!(repo.excluded_scanlators(id).expect("get").is_empty());
        repo.set_excluded_scanlators(id, &set(&["G1", "G2"])).expect("both");
        repo.set_excluded_scanlators(id, &set(&["G1"])).expect("one");
        assert_eq!(repo.excluded_scanlators(id).expect("get"), set(&["G1"]));
    }

    #[test]
    fn the_excluded_list_is_published_after_each_set() {
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_time().build().expect("runtime");
        let _guard = rt.enter();
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let id = repo.insert(&Entry::create(Kind::Manga)).expect("insert");
        let mut sub = repo.subscribe_excluded_scanlators(id);
        rt.block_on(async {
            let first = tokio::time::timeout(std::time::Duration::from_secs(5), sub.changed()).await.expect("first").expect("open");
            assert!(first.expect("query").is_empty());
        });
        repo.set_excluded_scanlators(id, &set(&["X"])).expect("set");
        rt.block_on(async {
            let next = tokio::time::timeout(std::time::Duration::from_secs(5), sub.changed()).await.expect("after set").expect("open");
            assert_eq!(*next.expect("query"), set(&["X"]));
        });
    }

    #[test]
    fn available_scanlators_are_distinct_and_drop_blank_names() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let id = repo.insert(&Entry::create(Kind::Manga)).expect("insert");
        for (url, scanlator) in [("c1", Some("G1")), ("c2", Some("G1")), ("c3", Some("  ")), ("c4", None), ("c5", Some("G2"))] {
            put(&db, item(Kind::Manga, id, url, scanlator));
        }
        assert_eq!(repo.available_scanlators(id).expect("available"), set(&["G1", "G2"]));
    }
}

mod triggers {
    use super::*;

    #[test]
    fn version_bumps_follow_the_ec05_matrix() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let id = repo.insert(&new_entry(kind, "u", "T", true)).unwrap();
            let step = |u: EntryUpdate| {
                repo.update(&u).unwrap();
                version(&db, id)
            };
            // NULL to text does not bump: the comparison of NULL is NULL.
            assert_eq!(step(EntryUpdate { description: Some("a".into()), ..EntryUpdate::new(id) }), 0);
            assert_eq!(step(EntryUpdate { description: Some("b".into()), ..EntryUpdate::new(id) }), 1);
            assert_eq!(step(EntryUpdate { description: Some("b".into()), ..EntryUpdate::new(id) }), 1);
            assert_eq!(step(EntryUpdate { url: Some("u2".into()), ..EntryUpdate::new(id) }), 2);
            assert_eq!(step(EntryUpdate { favorite: Some(false), ..EntryUpdate::new(id) }), 3);
            assert_eq!(step(EntryUpdate { title: Some("T2".into()), ..EntryUpdate::new(id) }), 3, "{kind}");
        }
    }

    #[test]
    fn a_title_only_update_stamps_favorite_modified_at_and_timestamps_are_seconds() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        for kind in Kind::ALL {
            let id = repo.insert(&new_entry(kind, "u", "T", true)).unwrap();
            repo.update(&EntryUpdate { title: Some("New".into()), ..EntryUpdate::new(id) }).unwrap();
            let row = repo.by_id(id).unwrap();
            let now = now_secs();
            let stamped = row.favorite_modified_at.expect("favorite_modified_at set");
            assert!((now - stamped).abs() <= 2, "favorite_modified_at {stamped} is not near {now}");
            assert!((now - row.last_modified_at).abs() <= 2);
            assert!(row.last_modified_at < 10_000_000_000, "timestamps are epoch seconds (EC-25)");
        }
    }

    #[test]
    fn marking_an_item_done_bumps_the_item_and_its_entry_and_a_fillermark_bumps_neither() {
        let (_d, db) = test_db();
        let items = ItemRepo::new(db.clone());
        for kind in Kind::ALL {
            let entry_id = insert_entry(&db, kind, "m", true);
            let item_id = put(&db, Item { version: 0, ..item(kind, entry_id, "c", None) });
            let item_version = || items.by_id(item_id).unwrap().unwrap().version;
            if kind.is_anime() {
                items.update(&ItemUpdate { fillermark: Some(true), ..ItemUpdate::new(item_id) }).unwrap();
                assert_eq!((item_version(), version(&db, entry_id)), (0, 0), "a fillermark bumps neither");
            }
            items.update(&ItemUpdate { done: Some(true), ..ItemUpdate::new(item_id) }).unwrap();
            assert_eq!((item_version(), version(&db, entry_id)), (1, 1), "{kind}");
        }
    }
}

mod subscriptions {
    use std::time::Duration;

    use kuro_db::subscribe::Subscription;

    use super::*;

    /// The next publication, or `None` when none comes within half a second.
    async fn next<T>(sub: &mut Subscription<T>) -> Option<Arc<T>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
    }

    /// The rows of the next library publication. A published error fails the test.
    async fn library_rows<T: Clone>(sub: &mut Subscription<crate::LibraryState<T>>) -> Vec<T> {
        let published = next(sub).await.expect("a library publication");
        match &*published {
            Ok(rows) => rows.clone(),
            Err(e) => panic!("library query failed: {e}"),
        }
    }

    fn title(published: &Option<Entry>) -> Option<&str> {
        published.as_ref().map(|m| m.title.as_str())
    }

    #[tokio::test]
    async fn by_id_publishes_each_change_and_none_once_deleted() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let id = repo.insert(&new_entry(kind, "u", "First", true)).unwrap();
            let mut sub = repo.subscribe_by_id(id);
            assert_eq!(title(&next(&mut sub).await.unwrap()), Some("First"));
            repo.update(&EntryUpdate { title: Some("Second".into()), ..EntryUpdate::new(id) }).unwrap();
            assert_eq!(title(&next(&mut sub).await.expect("the update publishes")), Some("Second"));
            db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [id])?)).unwrap();
            assert!(next(&mut sub).await.expect("the delete publishes").is_none(), "{kind}");
        }
    }

    #[tokio::test]
    async fn by_url_and_source_publishes_the_row_and_its_changes() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let first = repo.insert(&new_entry(kind, "dup", "First", true)).unwrap();
            let mut sub = repo.subscribe_by_url_and_source(kind, "dup", 1);
            assert_eq!(title(&next(&mut sub).await.unwrap()), Some("First"));
            repo.update(&EntryUpdate { title: Some("Renamed".into()), ..EntryUpdate::new(first) }).unwrap();
            assert_eq!(title(&next(&mut sub).await.expect("the update publishes")), Some("Renamed"), "{kind}");
        }
    }

    #[tokio::test]
    async fn favorites_by_source_publishes_when_a_favourite_joins_and_not_for_other_rows() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let mut sub = repo.subscribe_favorites_by_source(kind, 1);
            assert!(next(&mut sub).await.unwrap().is_empty());
            repo.insert(&new_entry(kind, "hidden", "Hidden", false)).unwrap();
            assert!(next(&mut sub).await.is_none(), "a non-favourite leaves the result unchanged");
            repo.insert(&new_entry(kind, "fav", "Fav", true)).unwrap();
            assert_eq!(next(&mut sub).await.expect("a favourite publishes").len(), 1, "{kind}");
        }
    }

    #[tokio::test]
    async fn library_publishes_a_done_flip_and_a_trigger_write_reaches_the_parent() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        for kind in Kind::ALL {
            let id = repo.insert(&new_entry(kind, "m", "M", true)).unwrap();
            let mut library = repo.subscribe_library(kind);
            assert_eq!(library_rows(&mut library).await[0].total, 0);
            let item_id = put(&db, item(kind, id, "c1", None));
            assert_eq!(library_rows(&mut library).await[0].total, 1);
            let mut by_id = repo.subscribe_by_id(id);
            let before = next(&mut by_id).await.unwrap().as_ref().as_ref().unwrap().version;

            db.write(|tx| Ok(tx.execute("UPDATE items SET done = 1 WHERE _id = ?1", [item_id])?)).unwrap();
            let rows = library_rows(&mut library).await;
            assert_eq!((rows[0].done, rows[0].pending()), (1, 0));
            let after = next(&mut by_id).await.expect("the trigger's parent write publishes");
            assert!(after.as_ref().as_ref().unwrap().version > before, "{kind}: the item trigger bumped the entry");
        }
    }

    #[tokio::test]
    async fn deleting_an_entry_publishes_an_empty_item_list() {
        let (_d, db) = test_db();
        let id = insert_entry(&db, Kind::Manga, "m", true);
        let items = ItemRepo::new(db.clone());
        items.add_all(&[item(Kind::Manga, id, "c1", None), item(Kind::Manga, id, "c2", None)]).unwrap();
        let mut sub = items.subscribe_by_entry_id(id, false);
        assert_eq!(next(&mut sub).await.unwrap().len(), 2);
        db.write(|tx| Ok(tx.execute("DELETE FROM entries WHERE _id = ?1", [id])?)).unwrap();
        assert!(next(&mut sub).await.expect("the cascade publishes").is_empty());
    }

    #[tokio::test]
    async fn seasons_of_a_parent_publish_after_an_item_is_done_and_after_a_child_is_renamed() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        let parent = repo.insert(&new_anime("p", "P", true)).unwrap();
        let child = repo.insert(&Entry { parent_id: Some(parent), ..new_anime("c", "C", false) }).unwrap();
        let mut sub = repo.subscribe_seasons_by_parent(parent);
        let first = next(&mut sub).await.expect("a season publication");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].total, 0);
        let item_id = put(&db, item(Kind::Anime, child, "e1", None));
        let after_insert = next(&mut sub).await.expect("a publication after the item insert");
        assert_eq!(after_insert[0].total, 1);
        db.write(|tx| Ok(tx.execute("UPDATE items SET done = 1 WHERE _id = ?1", [item_id])?)).unwrap();
        assert_eq!(next(&mut sub).await.expect("a publication after the done flag").first().map(|s| s.done), Some(1));
        repo.update(&EntryUpdate { title: Some("Renamed".into()), ..EntryUpdate::new(child) }).unwrap();
        let renamed = next(&mut sub).await.expect("a publication after the child rename");
        assert_eq!(renamed[0].entry.title, "Renamed");
    }

    #[tokio::test]
    async fn non_library_counts_publish_again_when_an_entry_changes() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        let mut sub = repo.subscribe_non_library_counts(Kind::Manga);
        let first = next(&mut sub).await.expect("an initial publication");
        assert!(first.is_empty());
        let id = repo.insert(&Entry { source: 2, ..new_manga("c", "C", false) }).unwrap();
        let after_insert = next(&mut sub).await.expect("a publication after an insert");
        assert_eq!(*after_insert, vec![(2, 1)]);
        repo.update(&EntryUpdate { favorite: Some(true), ..EntryUpdate::new(id) }).unwrap();
        let after_favourite = next(&mut sub).await.expect("a publication after a favourite flip");
        assert!(after_favourite.is_empty());
    }
}

mod library_retry {
    use std::cell::Cell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use kuro_db::subscribe::Subscription;
    use rusqlite::Connection;

    use super::*;
    use crate::retrying;

    const ENTRY_TABLES: &[&str] = &["entries"];

    /// A transient lock as SQLite reports it (`SQLITE_BUSY`).
    fn busy() -> DataError {
        DataError::Sqlite(rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(5), None))
    }

    async fn next_published<T>(sub: &mut Subscription<T>) -> Option<Result<Arc<T>, String>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten()
    }

    async fn next<T>(sub: &mut Subscription<T>) -> Option<Arc<T>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
    }

    #[test]
    fn a_transient_lock_is_retried_until_the_query_succeeds() {
        let calls = Cell::new(0);
        let query = retrying(20, Duration::ZERO, |_c: &Connection| {
            calls.set(calls.get() + 1);
            if calls.get() <= 3 { Err(busy()) } else { Ok(7_i64) }
        });
        let c = Connection::open_in_memory().expect("memory db");
        assert_eq!(query(&c).expect("no hard error"), Ok(7));
        assert_eq!(calls.get(), 4, "three retries, then the value");
    }

    #[test]
    fn after_twenty_retries_the_error_value_is_returned() {
        let calls = Cell::new(0);
        let query = retrying(20, Duration::ZERO, |_c: &Connection| {
            calls.set(calls.get() + 1);
            Err::<i64, _>(busy())
        });
        let c = Connection::open_in_memory().expect("memory db");
        let value = query(&c).expect("no hard error");
        assert!(value.is_err(), "the error text is published");
        assert_eq!(calls.get(), 21, "one attempt and twenty retries");
    }

    #[test]
    fn other_errors_are_not_retried() {
        let calls = Cell::new(0);
        let query = retrying(20, Duration::ZERO, |_c: &Connection| {
            calls.set(calls.get() + 1);
            Err::<i64, _>(DataError::Corrupt("injected".into()))
        });
        let c = Connection::open_in_memory().expect("memory db");
        assert!(matches!(query(&c), Err(DataError::Corrupt(_))));
        assert_eq!(calls.get(), 1);
    }

    #[tokio::test]
    async fn a_non_transient_failure_is_published_and_the_next_success_publishes() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db.clone());
        repo.insert(&new_manga("a", "A", true)).expect("insert");
        let fail = Arc::new(AtomicBool::new(false));
        let injected = Arc::clone(&fail);
        let query = retrying(20, Duration::ZERO, move |c: &Connection| {
            if injected.load(Ordering::SeqCst) {
                return Err(DataError::Corrupt("injected".into()));
            }
            Ok(c.query_row("SELECT count(*) FROM entries WHERE kind = 'manga'", [], |r| r.get::<_, i64>(0))?)
        });
        let mut sub = db.subscribe(ENTRY_TABLES, Duration::ZERO, query);
        assert_eq!(*next(&mut sub).await.expect("first publication"), Ok(1));

        fail.store(true, Ordering::SeqCst);
        repo.insert(&new_manga("b", "B", true)).expect("insert while the query fails");
        let failed = next_published(&mut sub).await.expect("the failure is published");
        assert!(failed.unwrap_err().contains("injected"), "the error text reaches the subscriber");

        fail.store(false, Ordering::SeqCst);
        repo.insert(&new_manga("c", "C", true)).expect("insert");
        assert_eq!(*next(&mut sub).await.expect("recovered publication"), Ok(3));
    }

    #[tokio::test]
    async fn an_exhausted_lock_publishes_the_error_value() {
        let (_d, db) = test_db();
        let query = retrying(20, Duration::ZERO, |_c: &Connection| Err::<i64, _>(busy()));
        let mut sub = db.subscribe(ENTRY_TABLES, Duration::ZERO, query);
        let published = next(&mut sub).await.expect("first publication");
        assert!(published.is_err(), "the subscription surfaces the error value");
    }
}

mod item_repo {
    use std::collections::HashSet;
    use std::time::Duration;

    use super::*;
    use crate::item::ItemKey;

    #[test]
    fn create_defaults_match_the_kotlin_factory() {
        for kind in Kind::ALL {
            let i = Item::create(kind);
            assert_eq!((i.id, i.entry_id, i.date_upload, i.version, i.number), (-1, -1, -1, 1, -1.0), "{kind}");
        }
    }

    #[test]
    fn add_all_returns_new_ids_in_input_order_and_binds_every_column() {
        let (_dir, db) = test_db();
        let a = insert_entry(&db, Kind::Anime, "a", false);
        let repo = ItemRepo::new(db.clone());
        let mut first = item(Kind::Anime, a, "e1", None);
        first.summary = Some("s".into());
        first.preview_url = Some("p".into());
        first.fillermark = true;
        first.memo = Some(Memo(serde_json::json!({"k": 2}).as_object().cloned().unwrap()));
        first.total_seconds = Some(1400);
        let added = repo.add_all(&[first.clone(), item(Kind::Anime, a, "e2", None), item(Kind::Anime, a, "e3", None)]).unwrap();
        assert_eq!(added.iter().map(|i| i.url.as_str()).collect::<Vec<_>>(), ["e1", "e2", "e3"]);
        assert!(added.windows(2).all(|w| w[0].id < w[1].id));
        let stored = repo.by_id(added[0].id).unwrap().unwrap();
        assert_eq!(stored, Item { id: added[0].id, ..first });
        assert_eq!(repo.by_entry_id(a, false).unwrap().len(), 3);
    }

    #[test]
    fn add_all_with_a_failing_row_leaves_the_table_unchanged() {
        let (_dir, db) = test_db();
        let repo = ItemRepo::new(db.clone());
        for kind in Kind::ALL {
            let e = insert_entry(&db, kind, "m", false);
            // The second row points at an entry that does not exist: a foreign key failure.
            assert!(repo.add_all(&[item(kind, e, "ok", None), item(kind, 999, "bad", None)]).is_err());
            assert!(repo.by_entry_id(e, false).unwrap().is_empty(), "{kind}");
        }
    }

    #[test]
    fn remove_with_ids_handles_1200_ids_in_one_call() {
        let (_dir, db) = test_db();
        let repo = ItemRepo::new(db.clone());
        for kind in Kind::ALL {
            let e = insert_entry(&db, kind, "m", false);
            let items: Vec<Item> = (0..1200).map(|i| item(kind, e, &format!("u{i}"), None)).collect();
            let ids: Vec<i64> = repo.add_all(&items).unwrap().iter().map(|c| c.id).collect();
            repo.remove_with_ids(&ids).unwrap();
            assert!(repo.by_entry_id(e, false).unwrap().is_empty(), "{kind}");
        }
    }

    #[test]
    fn by_entry_id_applies_the_scanlator_filter_exactly() {
        let (_dir, db) = test_db();
        let m = insert_entry(&db, Kind::Manga, "m", false);
        let repo = ItemRepo::new(db.clone());
        repo.add_all(&[item(Kind::Manga, m, "a", Some("Group A")), item(Kind::Manga, m, "b", Some("Group B")), item(Kind::Manga, m, "c", None)])
            .unwrap();
        EntryRepo::new(db).set_excluded_scanlators(m, &["Group B".to_owned()].into()).unwrap();
        let unfiltered: Vec<String> = repo.by_entry_id(m, false).unwrap().into_iter().map(|c| c.url).collect();
        let filtered: Vec<String> = repo.by_entry_id(m, true).unwrap().into_iter().map(|c| c.url).collect();
        assert_eq!(unfiltered.len(), 3);
        assert_eq!(filtered, ["a", "c"]);
    }

    /// T-05a-021: an item without a scanlator is never excluded, even when a blank name is listed.
    #[test]
    fn an_item_without_a_scanlator_is_never_excluded() {
        let (_dir, db) = test_db();
        let m = insert_entry(&db, Kind::Manga, "m", false);
        let repo = ItemRepo::new(db.clone());
        repo.add_all(&[item(Kind::Manga, m, "c", None)]).unwrap();
        db.write(|tx| Ok(kuro_db::tables::excluded::insert(tx, m, "")?)).unwrap();
        let filtered: Vec<String> = repo.by_entry_id(m, true).unwrap().into_iter().map(|c| c.url).collect();
        assert_eq!(filtered, ["c"]);
    }

    /// EC-12: duplicate exclusion rows inserted by hand multiply the rows of a non-excluded item when the filter flag
    /// is 0, because the join runs regardless of the flag. With the flag set, the row is kept once.
    #[test]
    fn duplicate_exclusion_rows_multiply_items_only_when_the_filter_flag_is_zero() {
        let (_dir, db) = test_db();
        let m = insert_entry(&db, Kind::Manga, "m", false);
        let repo = ItemRepo::new(db.clone());
        repo.add_all(&[item(Kind::Manga, m, "a", Some("Group A"))]).unwrap();
        db.write(|tx| {
            for group in ["Group B", "Group B", "Group A", "Group A"] {
                kuro_db::tables::excluded::insert(tx, m, group)?;
            }
            Ok(())
        })
        .unwrap();
        // Group A is excluded twice: the flag-set query drops it, the flag-clear query still multiplies it.
        assert_eq!(repo.by_entry_id(m, false).unwrap().len(), 2);
        assert!(repo.by_entry_id(m, true).unwrap().is_empty());
    }

    #[test]
    fn scanlators_map_null_to_empty_and_keep_duplicates() {
        let (_dir, db) = test_db();
        let m = insert_entry(&db, Kind::Manga, "m", false);
        let repo = ItemRepo::new(db.clone());
        repo.add_all(&[item(Kind::Manga, m, "a", Some("G")), item(Kind::Manga, m, "b", Some("G")), item(Kind::Manga, m, "c", None)]).unwrap();
        let mut names = repo.scanlators(m).unwrap();
        names.sort();
        assert_eq!(names, ["", "G", "G"]);
    }

    #[test]
    fn bookmarked_returns_only_bookmarked_items_and_anime_has_no_scanlator_filter() {
        let (_dir, db) = test_db();
        let repo = ItemRepo::new(db.clone());
        for kind in Kind::ALL {
            let e = insert_entry(&db, kind, "m", false);
            let added = repo.add_all(&[item(kind, e, "a", Some("G")), item(kind, e, "b", None)]).unwrap();
            repo.update(&ItemUpdate { bookmark: Some(true), ..ItemUpdate::new(added[1].id) }).unwrap();
            assert_eq!(repo.bookmarked(e).unwrap().iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["b"], "{kind}");
        }
    }

    #[test]
    fn a_duplicate_url_returns_the_lowest_id() {
        let (_dir, db) = test_db();
        let m = insert_entry(&db, Kind::Manga, "m", false);
        let first = insert_item(&db, Kind::Manga, m, "dup");
        let second = insert_item(&db, Kind::Manga, m, "dup");
        assert!(first < second);
        assert_eq!(ItemRepo::new(db).by_url_and_entry_id("dup", m).unwrap().unwrap().id, first);
    }

    #[test]
    fn a_corrupt_memo_does_not_fail_the_list_query() {
        let (_dir, db) = test_db();
        let a = insert_entry(&db, Kind::Anime, "a", false);
        let id = insert_item(&db, Kind::Anime, a, "e");
        db.write(|tx| Ok(tx.execute("UPDATE items SET memo = CAST('not json' AS BLOB) WHERE _id = ?1", [id]).map(drop)?)).unwrap();
        let listed = ItemRepo::new(db.clone()).by_entry_id(a, false).unwrap();
        assert_eq!(listed.len(), 1);
        assert!(listed[0].memo().is_empty());
    }

    #[test]
    fn reads_by_id_url_and_scanlator_filter() {
        let (_d, db) = test_db();
        let manga_id = insert_entry(&db, Kind::Manga, "m", true);
        put(&db, item(Kind::Manga, manga_id, "c1", Some("S1")));
        put(&db, item(Kind::Manga, manga_id, "c2", Some("S2")));
        EntryRepo::new(db.clone()).set_excluded_scanlators(manga_id, &["S2".to_owned()].into()).unwrap();
        let repo = ItemRepo::new(db.clone());
        assert_eq!(repo.by_entry_id(manga_id, false).unwrap().len(), 2);
        assert_eq!(repo.by_entry_id(manga_id, true).unwrap().len(), 1, "excluded scanlator dropped");
        let c1 = repo.by_url_and_entry_id("c1", manga_id).unwrap().expect("by url");
        assert_eq!(repo.by_id(c1.id).unwrap(), Some(c1));
        assert_eq!(repo.by_id(9_999).unwrap(), None, "a missing row is None, not an error");
        assert_eq!(repo.by_url_and_entry_id("c1", manga_id + 1).unwrap(), None);
    }

    #[test]
    fn update_writes_the_set_fields_for_both_kinds() {
        let (_d, db) = test_db();
        let repo = ItemRepo::new(db.clone());
        let manga = put(&db, item(Kind::Manga, insert_entry(&db, Kind::Manga, "m", true), "c", None));
        repo.update(&ItemUpdate { done: Some(true), progress: Some(4), ..ItemUpdate::new(manga) }).unwrap();
        let after = repo.by_id(manga).unwrap().expect("row");
        assert_eq!((after.done, after.progress), (true, 4));

        let anime = put(&db, item(Kind::Anime, insert_entry(&db, Kind::Anime, "a", true), "e1", None));
        let mut memo = serde_json::Map::new();
        memo.insert("k".into(), serde_json::json!(1));
        repo.update(&ItemUpdate { done: Some(true), summary: Some("s".into()), memo: Some(Memo(memo.clone())), ..ItemUpdate::new(anime) }).unwrap();
        let after = repo.by_id(anime).unwrap().expect("row");
        assert_eq!((after.done, after.summary.as_deref(), after.memo()), (true, Some("s"), memo));
    }

    #[test]
    fn update_all_rolls_back_on_failure() {
        let (_d, db) = test_db();
        let entry_id = insert_entry(&db, Kind::Manga, "m", true);
        let (a, b) = (put(&db, item(Kind::Manga, entry_id, "a", None)), put(&db, item(Kind::Manga, entry_id, "b", None)));
        abort_updates_of(&db, "items", b);
        let repo = ItemRepo::new(db);
        let result = repo.update_all(&[
            ItemUpdate { done: Some(true), ..ItemUpdate::new(a) },
            ItemUpdate { done: Some(true), ..ItemUpdate::new(b) },
        ]);
        assert!(result.is_err());
        assert!(!repo.by_id(a).unwrap().expect("row a").done, "the first update rolled back");
    }

    /// Every value from -1.0 to 10000.0 in 0.5 steps is stored in the REAL column and reads back as the same f32 (T-16a-007).
    #[test]
    fn the_item_number_survives_a_sqlite_real_round_trip_as_f32() {
        let (_d, db) = test_db();
        let anime_id = insert_entry(&db, Kind::Anime, "a", true);
        let numbers: Vec<f32> = (-2..=20_000).map(|half| half as f32 * 0.5).collect();
        let items: Vec<Item> =
            numbers.iter().enumerate().map(|(i, n)| Item { number: f64::from(*n), ..item(Kind::Anime, anime_id, &format!("e{i}"), None) }).collect();
        ItemRepo::new(db.clone()).add_all(&items).unwrap();
        let mut read: Vec<f32> = ItemRepo::new(db).by_entry_id(anime_id, false).unwrap().iter().map(|e| e.number as f32).collect();
        read.sort_by(f32::total_cmp);
        assert_eq!(read, numbers);
    }

    #[test]
    fn the_key_is_the_id_and_url_pair() {
        let (_d, db) = test_db();
        for kind in Kind::ALL {
            let entry_id = insert_entry(&db, kind, "m", true);
            insert_item(&db, kind, entry_id, "c1");
            let c = ItemRepo::new(db.clone()).by_url_and_entry_id("c1", entry_id).unwrap().expect("row");
            let mut set = HashSet::new();
            set.insert(ItemKey::from(&c));
            assert!(set.contains(&ItemKey::from(&c)), "the same row is the same key");
            let mut other_url = c.clone();
            other_url.url = "c2".into();
            assert_ne!(ItemKey::from(&other_url), ItemKey::from(&c));
            let mut other_id = c.clone();
            other_id.id += 1;
            assert_ne!(ItemKey::from(&other_id), ItemKey::from(&c));
        }
    }

    #[test]
    fn by_url_lists_every_match_of_the_kind_in_any_entry() {
        let (_d, db) = test_db();
        let repo = ItemRepo::new(db.clone());
        for kind in Kind::ALL {
            let a = insert_entry(&db, kind, "a", true);
            let b = insert_entry(&db, kind, "b", false);
            insert_item(&db, kind, a, "shared");
            insert_item(&db, kind, b, "shared");
            insert_item(&db, kind, a, "only-a");
            let mut entries: Vec<i64> = repo.by_url(kind, "shared").unwrap().iter().map(|c| c.entry_id).collect();
            entries.sort_unstable();
            assert_eq!(entries, vec![a.min(b), a.max(b)], "{kind}");
            assert!(repo.by_url(kind, "missing").unwrap().is_empty());
        }
    }

    async fn next<T>(sub: &mut kuro_db::subscribe::Subscription<T>) -> Option<Arc<T>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
    }

    #[tokio::test]
    async fn the_item_subscription_publishes_a_done_toggle_and_a_new_item() {
        let (_d, db) = test_db();
        let repo = ItemRepo::new(db.clone());
        for kind in Kind::ALL {
            let entry_id = insert_entry(&db, kind, "m", true);
            let mut sub = repo.subscribe_by_entry_id(entry_id, false);
            assert!(next(&mut sub).await.expect("first publication").is_empty());
            let id = put(&db, item(kind, entry_id, "c", None));
            assert!(!next(&mut sub).await.expect("the insert publishes")[0].done);
            repo.update(&ItemUpdate { done: Some(true), ..ItemUpdate::new(id) }).unwrap();
            assert!(next(&mut sub).await.expect("the toggle publishes")[0].done, "{kind}");
        }
    }
}

mod keys_and_clock {
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::time::Duration;

    use chrono::TimeZone as _;
    use kuro_db::subscribe::Subscription;

    use super::*;

    async fn next<T>(sub: &mut Subscription<T>) -> Option<Arc<T>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
    }

    fn titles(published: Option<Arc<Vec<Entry>>>) -> Vec<String> {
        published.expect("a publication").iter().map(|m| m.title.clone()).collect()
    }

    #[tokio::test]
    async fn deletable_parents_subscription_publishes_each_new_candidate() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        repo.insert(&new_anime("fav", "Fav", true)).unwrap();
        let mut sub = repo.subscribe_deletable_parents(Kind::Anime);
        assert_eq!(next(&mut sub).await.expect("first publication").len(), 0, "favourites are never candidates");
        repo.insert(&new_anime("gone", "Gone", false)).unwrap();
        assert_eq!(next(&mut sub).await.expect("the insert publishes").len(), 1);
    }

    #[tokio::test]
    async fn the_upcoming_subscription_recomputes_the_cut_off_on_every_run() {
        let (_d, db) = test_db();
        let repo = EntryRepo::new(db);
        for kind in Kind::ALL {
            let day1 = chrono::Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap().timestamp_millis();
            let start1 = time::start_of_local_day_millis(day1);
            let due = Entry { status: 1, next_update: start1 + 3_600_000, ..new_entry(kind, "due", "Due", true) };
            let id = repo.insert(&due).unwrap();
            let clock = Arc::new(AtomicI64::new(day1));
            let ticks = Arc::clone(&clock);
            let mut sub = repo.subscribe_upcoming_with_clock(kind, Arc::new(move || ticks.load(Ordering::SeqCst)));
            assert_eq!(titles(next(&mut sub).await), vec!["Due"]);
            // The next day starts after the entry's time. A write re-runs the query, which reads the new clock.
            clock.store(day1 + 86_400_000, Ordering::SeqCst);
            repo.update(&EntryUpdate { title: Some("Due again".into()), ..EntryUpdate::new(id) }).unwrap();
            assert_eq!(titles(next(&mut sub).await), Vec::<String>::new(), "{kind}: the cut-off moved to the new day");
        }
    }
}
