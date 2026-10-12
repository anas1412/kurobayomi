//! Repository tests against real files: fixtures, the trigger matrix (T-05a-024), the transaction and
//! legacy-adapter behaviour (T-05a-028), duplicates and upcoming queries (T-05a-019, T-05a-020).

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Local;
use kuro_db::codecs::{GenreList, UpdateStrategy};
use kuro_db::handle::Db;
use kuro_db::schema_anime::{FetchType, Memo};
use kuro_db::statements_anime::{AnimeRow, EpisodeRow, EpisodeUpdate};
use kuro_db::statements_manga::{ChapterPatch, ChapterRow, categories, chapters, excluded_scanlators};
use rusqlite::params;

use crate::DataError;
use crate::anime::{Anime, AnimeRepo, AnimeUpdate, UPCOMING_STATUSES as ANIME_STATUSES};
use crate::manga::{Manga, MangaRepo, MangaUpdate, UPCOMING_STATUSES as MANGA_STATUSES, library_by_category};
use crate::{compat, time};

pub(crate) use crate::support::{anime_db, manga_db};

fn new_manga(url: &str, title: &str, favorite: bool) -> Manga {
    Manga { url: url.to_owned(), title: title.to_owned(), favorite, source: 1, status: 1, ..Manga::create() }
}

fn new_anime(url: &str, title: &str, favorite: bool) -> Anime {
    Anime { url: url.to_owned(), title: title.to_owned(), favorite, source: 1, status: 1, ..Anime::create() }
}

fn chapter(manga_id: i64, url: &str, scanlator: Option<&str>, read: bool, last_page_read: i64) -> ChapterRow {
    ChapterRow {
        id: -1,
        manga_id,
        url: url.to_owned(),
        name: url.to_owned(),
        scanlator: scanlator.map(str::to_owned),
        read,
        bookmark: false,
        last_page_read,
        chapter_number: 1.0,
        source_order: 0,
        date_fetch: 0,
        date_upload: 0,
        last_modified_at: 0,
        version: 0,
        is_syncing: false,
    }
}

fn episode(anime_id: i64, url: &str, seen: bool) -> EpisodeRow {
    EpisodeRow {
        anime_id,
        url: url.to_owned(),
        name: url.to_owned(),
        seen,
        episode_number: 1.0,
        ..Default::default()
    }
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

fn manga_version(db: &Arc<Db>, id: i64) -> i64 {
    MangaRepo::new(Arc::clone(db)).by_id(id).unwrap().version
}

fn anime_version(db: &Arc<Db>, id: i64) -> i64 {
    AnimeRepo::new(Arc::clone(db)).by_id(id).unwrap().version
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).expect("clock").as_secs() as i64
}

mod manga_repo {
    use super::*;

    #[test]
    fn insert_returns_a_new_id_and_ignores_the_input_id() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let mut m = new_manga("u1", "One", true);
        m.id = 99;
        m.last_modified_at = 123;
        let id = repo.insert(&m).unwrap();
        assert_ne!(id, 99);
        let back = repo.by_id(id).unwrap();
        assert_eq!(back, Manga { id, last_modified_at: 0, ..m });
    }

    #[test]
    fn by_id_missing_is_not_found() {
        let (_d, db) = manga_db();
        assert!(matches!(MangaRepo::new(db).by_id(404), Err(DataError::NotFound)));
    }

    /// T-43b-011, E-20b: the reading mode is bits 0..2 and the orientation bits 3..5 of `mangas.viewer`. Writing one
    /// group keeps the other, and the column value round-trips through the database unchanged.
    #[test]
    fn viewer_bits_write_keeps_the_other_group_and_the_column_round_trips() {
        const MODE: i64 = 0x7;
        const ORIENTATION: i64 = 0x38;
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let id = repo.insert(&Manga { viewer_flags: 0b101_010, ..new_manga("v", "V", true) }).unwrap();
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b101_010, "stored as given");

        assert!(repo.set_viewer_bits(id, 0b011, MODE).unwrap());
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b101_011, "mode replaced, orientation bits kept");

        // A value wider than the mask cannot leak into the other group.
        assert!(repo.set_viewer_bits(id, 0b111_100, MODE).unwrap());
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b101_100);

        assert!(repo.set_viewer_bits(id, 0b010 << 3, ORIENTATION).unwrap());
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b010_100, "orientation replaced, mode kept");

        // An unrelated update leaves the column alone.
        repo.update(&MangaUpdate { title: Some("W".into()), ..MangaUpdate::new(id) }).unwrap();
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0b010_100);
        assert!(!repo.set_viewer_bits(id + 1, 1, MODE).unwrap(), "no such manga");
    }

    #[test]
    fn by_url_and_source_returns_the_lowest_id_on_duplicates() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let first = repo.insert(&new_manga("dup", "A", false)).unwrap();
        repo.insert(&new_manga("dup", "B", false)).unwrap();
        let found = repo.by_url_and_source("dup", 1).unwrap().unwrap();
        assert_eq!(found.id, first);
        assert!(repo.by_url_and_source("missing", 1).unwrap().is_none());
    }

    #[test]
    fn favorites_and_read_not_in_library_follow_their_sql() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let fav = repo.insert(&new_manga("f", "Fav", true)).unwrap();
        let read = repo.insert(&new_manga("r", "Read", false)).unwrap();
        let progress = repo.insert(&new_manga("p", "Progress", false)).unwrap();
        repo.insert(&new_manga("n", "None", false)).unwrap();
        db.write(|tx| {
            chapters::insert(tx, &chapter(read, "r1", None, true, 0))?;
            chapters::insert(tx, &chapter(progress, "p1", None, false, 5))?;
            Ok(())
        })
        .unwrap();
        let favs: Vec<i64> = repo.favorites().unwrap().iter().map(|m| m.id).collect();
        assert_eq!(favs, vec![fav]);
        let mut not_in_library: Vec<i64> = repo.read_not_in_library().unwrap().iter().map(|m| m.id).collect();
        not_in_library.sort_unstable();
        assert_eq!(not_in_library, vec![read, progress]);
    }

    #[test]
    fn update_keeps_none_fields_and_a_missing_id_is_not_an_error() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let mut m = new_manga("u", "A", false);
        m.artist = Some("Artist".into());
        let id = repo.insert(&m).unwrap();
        repo.update(&MangaUpdate { title: Some("B".into()), ..MangaUpdate::new(id) }).unwrap();
        let back = repo.by_id(id).unwrap();
        assert_eq!(back.title, "B");
        assert_eq!(back.artist.as_deref(), Some("Artist"));
        repo.update(&MangaUpdate::new(9_999)).unwrap();
    }

    #[test]
    fn update_all_rolls_back_every_row_when_one_fails() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let a = repo.insert(&new_manga("a", "A", false)).unwrap();
        let b = repo.insert(&new_manga("b", "B", false)).unwrap();
        abort_updates_of(&db, "mangas", b);
        let result = repo.update_all(&[
            MangaUpdate { title: Some("X".into()), ..MangaUpdate::new(a) },
            MangaUpdate { title: Some("Y".into()), ..MangaUpdate::new(b) },
        ]);
        assert!(result.is_err());
        assert_eq!(repo.by_id(a).unwrap().title, "A");
    }

    #[test]
    fn reset_viewer_flags_zeroes_every_row() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let mut m = new_manga("u", "A", false);
        m.viewer_flags = 5;
        let id = repo.insert(&m).unwrap();
        repo.reset_viewer_flags().unwrap();
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0);
    }

    #[test]
    fn library_has_one_row_per_category_link_and_unlinked_favourites_sit_at_zero() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let (cat_a, cat_b) = db
            .write(|tx| Ok((categories::insert(tx, "A", 1, 0)?, categories::insert(tx, "B", 2, 0)?)))
            .unwrap();
        let both = repo.insert(&new_manga("both", "Both", true)).unwrap();
        let loose = repo.insert(&new_manga("loose", "Loose", true)).unwrap();
        let hidden = repo.insert(&new_manga("hidden", "Hidden", false)).unwrap();
        repo.set_categories(both, &[cat_a, cat_b]).unwrap();
        repo.set_categories(hidden, &[cat_a]).unwrap();

        let rows = repo.library().unwrap();
        assert_eq!(rows.iter().filter(|r| r.id() == both).count(), 2);
        assert_eq!(rows.iter().filter(|r| r.id() == loose).map(|r| r.category).collect::<Vec<_>>(), vec![0]);
        assert!(rows.iter().all(|r| r.id() != hidden));

        let grouped = library_by_category(rows);
        assert_eq!(grouped.keys().copied().collect::<Vec<_>>(), vec![0, cat_a, cat_b]);
    }

    #[test]
    fn library_counts_ignore_chapters_of_excluded_scanlators() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("m", "M", true)).unwrap();
        db.write(|tx| {
            chapters::insert(tx, &chapter(id, "c1", Some("S1"), true, 0))?;
            chapters::insert(tx, &chapter(id, "c2", Some("S2"), false, 0))?;
            excluded_scanlators::insert(tx, id, "S2")?;
            Ok(())
        })
        .unwrap();
        let row = repo.library().unwrap().into_iter().find(|r| r.id() == id).unwrap();
        assert_eq!((row.total_chapters, row.read_count, row.unread_count()), (1, 1, 0));
        assert!(row.has_started());
    }

    #[test]
    fn set_categories_bumps_version_once_per_link_and_rolls_back_on_a_missing_category() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let (a, b, c) = db
            .write(|tx| {
                Ok((
                    categories::insert(tx, "A", 1, 0)?,
                    categories::insert(tx, "B", 2, 0)?,
                    categories::insert(tx, "C", 3, 0)?,
                ))
            })
            .unwrap();
        let id = repo.insert(&new_manga("m", "M", true)).unwrap();
        repo.set_categories(id, &[a, b, c]).unwrap();
        assert_eq!(manga_version(&db, id), 3);
        repo.set_categories(id, &[]).unwrap();
        assert_eq!(manga_version(&db, id), 3);
        repo.set_categories(id, &[a]).unwrap();
        assert_eq!(manga_version(&db, id), 4);
        assert!(repo.set_categories(id, &[b, 999]).is_err());
        let linked: Vec<i64> = repo.library().unwrap().iter().map(|r| r.category).collect();
        assert_eq!(linked, vec![a]);
    }

    #[test]
    fn duplicates_match_unicode_titles_and_skip_self_and_non_favourites() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let first = repo.insert(&new_manga("a", "Éclair", true)).unwrap();
        let second = repo.insert(&new_manga("b", "éCLAIR", true)).unwrap();
        repo.insert(&new_manga("c", "éclair", false)).unwrap();
        let found: Vec<i64> = repo.duplicates(second, "éCLAIR").unwrap().iter().map(|m| m.id).collect();
        assert_eq!(found, vec![first]);
        assert!(repo.duplicates(first, "éclair").unwrap().iter().all(|m| m.id != first));
    }

    #[test]
    fn upcoming_lists_favourites_from_local_midnight_soonest_first() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let now = Local::now().timestamp_millis();
        let start = time::start_of_local_day_millis(now);
        let with = |url: &str, fav: bool, status: i64, next: i64| {
            let mut m = new_manga(url, url, fav);
            m.status = status;
            m.next_update = next;
            m
        };
        let later = repo.insert(&with("later", true, 1, start + 3_600_000)).unwrap();
        let soon = repo.insert(&with("soon", true, 4, start + 1_000)).unwrap();
        repo.insert(&with("wrong-status", true, 2, start + 1_000)).unwrap();
        repo.insert(&with("yesterday", true, 1, start - 1)).unwrap();
        repo.insert(&with("not-favourite", false, 1, start + 5)).unwrap();
        let ids: Vec<i64> = repo.upcoming(&MANGA_STATUSES, now).unwrap().iter().map(|m| m.id).collect();
        assert_eq!(ids, vec![soon, later]);
    }

    #[test]
    fn delete_non_library_by_sources_never_deletes_a_favourite() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let fav = repo.insert(&new_manga("f", "F", true)).unwrap();
        let non = repo.insert(&new_manga("n", "N", false)).unwrap();
        repo.delete_non_library_by_sources(&[1]).unwrap();
        assert!(repo.by_id(fav).is_ok());
        assert!(matches!(repo.by_id(non), Err(DataError::NotFound)));
    }

    #[test]
    fn clear_non_library_removes_only_non_library_rows_and_an_empty_list_is_a_no_op() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let fav = repo.insert(&new_manga("f", "F", true)).unwrap();
        let non = repo.insert(&new_manga("n", "N", false)).unwrap();
        assert_eq!(repo.clear_non_library_by_sources(&[]).unwrap(), 0);
        assert!(repo.by_id(non).is_ok(), "an empty list deletes nothing");
        repo.clear_non_library_by_sources(&[1]).unwrap();
        assert!(repo.by_id(fav).is_ok(), "library entries are untouched");
        assert!(matches!(repo.by_id(non), Err(DataError::NotFound)));
    }

    #[test]
    fn source_counts_split_library_and_non_library_rows() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        repo.insert(&new_manga("a", "A", true)).unwrap();
        repo.insert(&new_manga("b", "B", true)).unwrap();
        repo.insert(&Manga { source: 2, ..new_manga("c", "C", false) }).unwrap();
        assert_eq!(repo.favorite_counts_by_source().unwrap(), vec![(1, 2)]);
        assert_eq!(repo.non_library_counts_by_source().unwrap(), vec![(2, 1)]);
        assert_eq!(repo.all_source_and_url().unwrap().len(), 3);
    }
}

mod triggers {
    use super::*;

    #[test]
    fn version_bumps_follow_the_ec05_matrix() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("u", "T", true)).unwrap();
        let step = |u: MangaUpdate| {
            repo.update(&u).unwrap();
            manga_version(&db, id)
        };
        // NULL to text does not bump: the comparison of NULL is NULL.
        assert_eq!(step(MangaUpdate { description: Some("a".into()), ..MangaUpdate::new(id) }), 0);
        assert_eq!(step(MangaUpdate { description: Some("b".into()), ..MangaUpdate::new(id) }), 1);
        assert_eq!(step(MangaUpdate { description: Some("b".into()), ..MangaUpdate::new(id) }), 1);
        assert_eq!(step(MangaUpdate { url: Some("u2".into()), ..MangaUpdate::new(id) }), 2);
        assert_eq!(step(MangaUpdate { favorite: Some(false), ..MangaUpdate::new(id) }), 3);
        assert_eq!(step(MangaUpdate { title: Some("T2".into()), ..MangaUpdate::new(id) }), 3);
    }

    #[test]
    fn a_title_only_update_stamps_favorite_modified_at_and_timestamps_are_seconds() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let id = repo.insert(&new_manga("u", "T", true)).unwrap();
        repo.update(&MangaUpdate { title: Some("New".into()), ..MangaUpdate::new(id) }).unwrap();
        let row = repo.by_id(id).unwrap();
        let now = now_secs();
        let stamped = row.favorite_modified_at.expect("favorite_modified_at set");
        assert!((now - stamped).abs() <= 2, "favorite_modified_at {stamped} is not near {now}");
        assert!((now - row.last_modified_at).abs() <= 2);
        assert!(row.last_modified_at < 10_000_000_000, "timestamps are epoch seconds (EC-25)");
    }

    #[test]
    fn reading_a_chapter_bumps_the_chapter_and_its_manga() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let manga_id = repo.insert(&new_manga("m", "M", true)).unwrap();
        let chapter_id = db.write(|tx| Ok(chapters::insert(tx, &chapter(manga_id, "c", None, false, 0))?)).unwrap();
        db.write(|tx| {
            chapters::update(tx, &ChapterPatch { id: chapter_id, read: Some(true), ..Default::default() })?;
            Ok(())
        })
        .unwrap();
        let chapter_version: i64 = db
            .read(|c| Ok(chapters::get_by_id(c, chapter_id)?.version))
            .unwrap();
        assert_eq!(chapter_version, 1);
        assert_eq!(manga_version(&db, manga_id), 1);
    }
}

mod anime_repo {
    use super::*;

    fn full_anime() -> Anime {
        Anime {
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
            episode_flags: 5,
            cover_last_modified: 40,
            update_strategy: UpdateStrategy::OnlyFetchOnce,
            initialized: true,
            version: 5,
            background_last_modified: 9,
            background_url: Some("bg".into()),
            fetch_type: FetchType::Seasons,
            parent_id: Some(42),
            season_flags: 3,
            season_number: 2.5,
            season_source_order: 4,
            memo: serde_json::json!({"k": "v", "n": 1}).as_object().cloned().unwrap(),
            ..new_anime("full", "Full", true)
        }
    }

    #[test]
    fn insert_round_trips_every_column_except_the_ones_the_database_sets() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let expected = full_anime();
        let id = repo.insert(&expected).unwrap();
        assert_eq!(repo.by_id(id).unwrap(), Anime { id, ..expected });
    }

    #[test]
    fn update_sets_parent_id_but_never_clears_it_and_keeps_none_fields() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let id = repo.insert(&new_anime("u", "T", false)).unwrap();
        repo.update(&AnimeUpdate { parent_id: Some(7), ..AnimeUpdate::new(id) }).unwrap();
        repo.update(&AnimeUpdate { title: Some("New".into()), ..AnimeUpdate::new(id) }).unwrap();
        let back = repo.by_id(id).unwrap();
        assert_eq!((back.parent_id, back.title.as_str()), (Some(7), "New"));
    }

    #[test]
    fn a_title_only_update_stamps_favorite_modified_at() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let id = repo.insert(&new_anime("u", "T", true)).unwrap();
        repo.update(&AnimeUpdate { title: Some("N".into()), ..AnimeUpdate::new(id) }).unwrap();
        assert!(repo.by_id(id).unwrap().favorite_modified_at.is_some());
    }

    #[test]
    fn library_stats_switch_by_fetch_type_and_a_seasons_parent_counts_its_children() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let parent = repo
            .insert(&Anime { fetch_type: FetchType::Seasons, ..new_anime("parent", "Parent", true) })
            .unwrap();
        let season = |url: &str| Anime {
            parent_id: Some(parent),
            fetch_type: FetchType::Episodes,
            ..new_anime(url, url, false)
        };
        repo.insert(&season("s0")).unwrap();
        let all_seen = repo.insert(&season("s5")).unwrap();
        let partly = repo.insert(&season("s53")).unwrap();
        let single = repo.insert(&new_anime("single", "Single", true)).unwrap();
        db.write(|tx| {
            for i in 0..5 {
                kuro_db::statements_anime::insert_episode(tx, &episode(all_seen, &format!("a{i}"), true))?;
                kuro_db::statements_anime::insert_episode(tx, &episode(partly, &format!("p{i}"), i < 3))?;
            }
            kuro_db::statements_anime::insert_episode(tx, &episode(single, "e1", true))?;
            kuro_db::statements_anime::insert_episode(tx, &episode(single, "e2", false))?;
            kuro_db::statements_anime::insert_episode(tx, &episode(single, "e3", false))?;
            Ok(())
        })
        .unwrap();

        let rows = repo.library().unwrap();
        let parent_row = rows.iter().find(|r| r.id() == parent).unwrap();
        assert_eq!((parent_row.total_count, parent_row.seen_count), (3, 2));
        let single_row = rows.iter().find(|r| r.id() == single).unwrap();
        assert_eq!((single_row.total_count, single_row.seen_count, single_row.unseen_count()), (3, 1, 2));
        assert_eq!(rows.len(), 2, "non-favourite children are not library rows");

        let seasons = repo.seasons_by_parent(parent).unwrap();
        assert_eq!(seasons.len(), 3);
        let partly_row = seasons.iter().find(|s| s.id() == partly).unwrap();
        assert_eq!((partly_row.total_count, partly_row.seen_count), (5, 3));
        assert!(!partly_row.seen());
        assert_eq!(repo.children_by_parent(parent).unwrap().len(), 3);
    }

    #[test]
    fn deletable_parents_are_non_favourite_top_level_rows() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let fav = repo.insert(&new_anime("fav", "F", true)).unwrap();
        let top = repo.insert(&new_anime("top", "T", false)).unwrap();
        repo.insert(&Anime { parent_id: Some(top), ..new_anime("child", "C", false) }).unwrap();
        let ids: Vec<i64> = repo.deletable_parents().unwrap().iter().map(|d| d.anime_id).collect();
        assert_eq!(ids, vec![top]);
        assert!(!ids.contains(&fav));
    }

    #[test]
    fn remove_parent_ids_clears_the_child_link_and_a_deleted_parent_leaves_it_dangling() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let parent = repo.insert(&new_anime("p", "P", false)).unwrap();
        let child = repo.insert(&Anime { parent_id: Some(parent), ..new_anime("c", "C", true) }).unwrap();
        repo.delete_non_library_by_ids(&[parent]).unwrap();
        assert_eq!(repo.by_id(child).unwrap().parent_id, Some(parent));
        repo.remove_parent_ids(&[child]).unwrap();
        assert_eq!(repo.by_id(child).unwrap().parent_id, None);
    }

    #[test]
    fn delete_non_library_by_ids_never_deletes_a_favourite_and_cascades_to_episodes_and_links() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let fav = repo.insert(&new_anime("fav", "F", true)).unwrap();
        let non = repo.insert(&new_anime("non", "N", false)).unwrap();
        let cat = db.write(|tx| Ok(kuro_db::statements_anime::insert_category(tx, "C", 1, 0)?)).unwrap();
        db.write(|tx| {
            kuro_db::statements_anime::insert_episode(tx, &episode(non, "e", false))?;
            tx.execute("INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, ?2)", params![non, cat])?;
            Ok(())
        })
        .unwrap();
        repo.delete_non_library_by_ids(&[fav, non]).unwrap();
        assert!(repo.by_id(fav).is_ok());
        assert!(matches!(repo.by_id(non), Err(DataError::NotFound)));
        let (episodes, links): (i64, i64) = db
            .read(|c| {
                Ok((
                    c.query_row("SELECT count(*) FROM episodes WHERE anime_id = ?1", [non], |r| r.get(0))?,
                    c.query_row("SELECT count(*) FROM animes_categories WHERE anime_id = ?1", [non], |r| r.get(0))?,
                ))
            })
            .unwrap();
        assert_eq!((episodes, links), (0, 0));
    }

    #[test]
    fn duplicates_match_unicode_titles_for_anime() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let first = repo.insert(&new_anime("a", "Éclair", true)).unwrap();
        let second = repo.insert(&new_anime("b", "éCLAIR", true)).unwrap();
        let found: Vec<i64> = repo.duplicates(second, "éCLAIR").unwrap().iter().map(|a| a.id).collect();
        assert_eq!(found, vec![first]);
    }

    #[test]
    fn upcoming_uses_the_anime_status_list_and_orders_soonest_first() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let now = Local::now().timestamp_millis();
        let start = time::start_of_local_day_millis(now);
        let with = |url: &str, status: i64, next: i64| Anime {
            status,
            next_update: next,
            ..new_anime(url, url, true)
        };
        let seven = repo.insert(&with("seven", 7, start + 9_000)).unwrap();
        let one = repo.insert(&with("one", 1, start + 1_000)).unwrap();
        repo.insert(&with("two", 2, start + 1_000)).unwrap();
        let ids: Vec<i64> = repo.upcoming(&ANIME_STATUSES, now).unwrap().iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![one, seven]);
    }

    #[test]
    fn set_categories_dedupes_bumps_once_per_link_and_rolls_back_on_a_missing_category() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let (a, b) = db
            .write(|tx| Ok((kuro_db::statements_anime::insert_category(tx, "A", 1, 0)?, kuro_db::statements_anime::insert_category(tx, "B", 2, 0)?)))
            .unwrap();
        let id = repo.insert(&new_anime("x", "X", true)).unwrap();
        repo.set_categories(id, &[a, a, b]).unwrap();
        assert_eq!(anime_version(&db, id), 2);
        repo.set_categories(id, &[a]).unwrap();
        assert_eq!(anime_version(&db, id), 3);
        assert!(repo.set_categories(id, &[b, 999]).is_err());
        let linked: Vec<i64> = repo.library().unwrap().iter().map(|r| r.category).collect();
        assert_eq!(linked, vec![a]);
    }

    #[test]
    fn update_all_is_one_transaction_for_anime() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let a = repo.insert(&new_anime("a", "A", false)).unwrap();
        let b = repo.insert(&new_anime("b", "B", false)).unwrap();
        abort_updates_of(&db, "animes", b);
        let result = repo.update_all(&[
            AnimeUpdate { title: Some("X".into()), ..AnimeUpdate::new(a) },
            AnimeUpdate { title: Some("Y".into()), ..AnimeUpdate::new(b) },
        ]);
        assert!(result.is_err());
        assert_eq!(repo.by_id(a).unwrap().title, "A");
    }

    #[test]
    fn episode_seen_bumps_both_versions_and_fillermark_bumps_neither() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let anime_id = repo.insert(&new_anime("a", "A", true)).unwrap();
        let ep = db.write(|tx| Ok(kuro_db::statements_anime::insert_episode(tx, &episode(anime_id, "e", false))?)).unwrap();
        let ep_version = |db: &Db| -> i64 {
            db.read(|c| Ok(kuro_db::statements_anime::episode_by_id(c, ep)?.map(|e| e.version).unwrap_or(-1)))
                .unwrap()
        };
        db.write(|tx| {
            kuro_db::statements_anime::update_episode(tx, &EpisodeUpdate { id: ep, fillermark: Some(true), ..Default::default() })?;
            Ok(())
        })
        .unwrap();
        assert_eq!((ep_version(&db), anime_version(&db, anime_id)), (0, 0));
        db.write(|tx| {
            kuro_db::statements_anime::update_episode(tx, &EpisodeUpdate { id: ep, seen: Some(true), ..Default::default() })?;
            Ok(())
        })
        .unwrap();
        assert_eq!((ep_version(&db), anime_version(&db, anime_id)), (1, 1));
    }

    #[test]
    fn reset_viewer_flags_zeroes_anime_rows() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let id = repo.insert(&Anime { viewer_flags: 9, ..new_anime("a", "A", false) }).unwrap();
        repo.reset_viewer_flags().unwrap();
        assert_eq!(repo.by_id(id).unwrap().viewer_flags, 0);
    }

    #[test]
    fn anime_row_defaults_match_the_create_factory() {
        let row = AnimeRow::default();
        assert_eq!(row.memo, Memo::default());
        let a = Anime::create();
        assert_eq!((a.id, a.source, a.fetch_type, a.season_number), (-1, -1, FetchType::Episodes, -1.0));
        assert_eq!(a.memo, serde_json::Map::new());
        let m = Manga::create();
        assert_eq!((m.id, m.source, m.version, m.update_strategy), (-1, -1, 0, UpdateStrategy::AlwaysUpdate));
        assert_eq!(GenreList::default().0.len(), 0);
    }
}

mod adapter {
    use super::*;

    #[test]
    fn update_wrappers_return_false_on_a_failed_write_and_true_otherwise() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("u", "T", false)).unwrap();
        assert!(compat::update_manga(&repo, &MangaUpdate { title: Some("A".into()), ..MangaUpdate::new(id) }));
        abort_updates_of(&db, "mangas", id);
        assert!(!compat::update_manga(&repo, &MangaUpdate { title: Some("B".into()), ..MangaUpdate::new(id) }));
        assert!(!compat::update_all_manga(&repo, &[MangaUpdate { title: Some("C".into()), ..MangaUpdate::new(id) }]));
        assert!(!compat::reset_manga_viewer_flags(&repo));
        assert_eq!(repo.by_id(id).unwrap().title, "A");
    }

    #[test]
    fn read_and_remove_methods_propagate_their_errors() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("u", "T", false)).unwrap();
        assert!(matches!(repo.by_id(404), Err(DataError::NotFound)));
        db.write(|tx| {
            tx.execute_batch("CREATE TRIGGER no_delete BEFORE DELETE ON mangas BEGIN SELECT RAISE(ABORT, 'injected'); END;")?;
            Ok(())
        })
        .unwrap();
        assert!(repo.delete_non_library_by_sources(&[1]).is_err());
        assert!(repo.by_id(id).is_ok());
    }

    #[test]
    fn anime_wrappers_follow_the_kotlin_shapes() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let id = repo.insert(&new_anime("u", "T", false)).unwrap();
        abort_updates_of(&db, "animes", id);
        assert!(!compat::update_anime(&repo, &AnimeUpdate { title: Some("X".into()), ..AnimeUpdate::new(id) }));
        assert!(!compat::update_all_anime(&repo, &[AnimeUpdate::new(id)]));
        assert!(!compat::reset_anime_viewer_flags(&repo));
        compat::remove_anime_parent_ids(&repo, &[id]);
        assert_eq!(repo.by_id(id).unwrap().title, "T");
    }
}

mod views_and_round_trip {
    use super::*;
    use rusqlite::types::Value;

    fn column_count(db: &Db, view: &str) -> i64 {
        db.read(|c| {
            Ok(c.query_row(&format!("SELECT count(*) FROM pragma_table_info('{view}')"), [], |r| r.get(0))?)
        })
        .unwrap()
    }

    /// Every column of the `mangas` row `id`, by name, as stored.
    fn manga_columns(db: &Db, id: i64) -> Vec<(String, Value)> {
        db.read(|c| {
            let mut st = c.prepare("SELECT * FROM mangas WHERE _id = ?1")?;
            let names: Vec<String> = st.column_names().iter().map(|n| (*n).to_owned()).collect();
            let mut rows = st.query([id])?;
            let row = rows.next()?.expect("row exists");
            let mut out = Vec::with_capacity(names.len());
            for (i, name) in names.into_iter().enumerate() {
                out.push((name, row.get::<_, Value>(i)?));
            }
            Ok(out)
        })
        .unwrap()
    }

    #[test]
    fn library_views_have_the_column_counts_the_mappers_expect() {
        let (_m, manga) = manga_db();
        let (_a, anime) = anime_db();
        assert_eq!(column_count(&manga, "libraryView"), 31);
        assert_eq!(column_count(&anime, "animelibView"), 40);
        assert_eq!(column_count(&anime, "animeseasonsView"), 39);
    }

    #[test]
    fn updating_one_manga_title_leaves_every_other_column_unchanged() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let mut m = new_manga("u", "Old", true);
        m.description = Some("d".into());
        m.genre = Some(vec!["Action".into(), "Drama".into()]);
        m.last_update = 7;
        let id = repo.insert(&m).unwrap();
        let before = manga_columns(&db, id);
        repo.update(&MangaUpdate { title: Some("New".into()), ..MangaUpdate::new(id) }).unwrap();
        let after = manga_columns(&db, id);
        for ((name, was), (_, now)) in before.iter().zip(after.iter()) {
            if ["title", "last_modified_at", "favorite_modified_at"].contains(&name.as_str()) {
                continue;
            }
            assert_eq!(was, now, "column {name} changed");
        }
    }

    #[test]
    fn watched_not_in_library_lists_non_favourites_with_progress() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let watched = repo.insert(&new_anime("w", "W", false)).unwrap();
        repo.insert(&new_anime("idle", "I", false)).unwrap();
        repo.insert(&new_anime("fav", "F", true)).unwrap();
        db.write(|tx| {
            kuro_db::statements_anime::insert_episode(tx, &episode(watched, "e", true))?;
            Ok(())
        })
        .unwrap();
        let ids: Vec<i64> = repo.watched_not_in_library().unwrap().iter().map(|a| a.id).collect();
        assert_eq!(ids, vec![watched]);
    }

    #[test]
    fn season_bookmark_and_fillermark_counts_sum_over_children() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let parent = repo.insert(&Anime { fetch_type: FetchType::Seasons, ..new_anime("p", "P", true) }).unwrap();
        let c1 = repo.insert(&Anime { parent_id: Some(parent), ..new_anime("c1", "C1", false) }).unwrap();
        let c2 = repo.insert(&Anime { parent_id: Some(parent), ..new_anime("c2", "C2", false) }).unwrap();
        db.write(|tx| {
            let mut bookmarked = episode(c1, "b", false);
            bookmarked.bookmark = true;
            let mut filler = episode(c2, "f", false);
            filler.fillermark = true;
            kuro_db::statements_anime::insert_episode(tx, &bookmarked)?;
            kuro_db::statements_anime::insert_episode(tx, &filler)?;
            Ok(())
        })
        .unwrap();
        let row = repo.library().unwrap().into_iter().find(|r| r.id() == parent).unwrap();
        assert_eq!((row.bookmark_count, row.fillermark_count), (1, 1));
        assert!(row.has_bookmarks());
    }

    #[test]
    // Timing contract for release builds only: run `cargo test --release -p kuro-data -- --ignored`.
    // A debug build takes about 1.1 s here and fails the bound, so the debug profile is not timed.
    #[ignore = "benchmark: 5 000 manga and 200 000 chapters, run with cargo test --release -- --ignored"]
    fn library_query_on_a_large_fixture_stays_under_500_ms() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let ids: Vec<i64> = (0..5_000)
            .map(|m| repo.insert(&new_manga(&format!("u{m}"), &format!("Title {m}"), true)).unwrap())
            .collect();
        db.write(|tx| {
            for (m, id) in ids.iter().enumerate() {
                for c in 0..40 {
                    chapters::insert(tx, &chapter(*id, &format!("c{m}-{c}"), None, c % 3 == 0, 0))?;
                }
            }
            Ok(())
        })
        .unwrap();
        let started = std::time::Instant::now();
        let rows = repo.library().unwrap();
        let elapsed = started.elapsed();
        assert_eq!(rows.len(), 5_000);
        assert!(elapsed.as_millis() < 500, "library() took {elapsed:?}");
    }
}

mod library_aggregates {
    use super::*;
    use kuro_db::statements_manga::history;

    #[test]
    fn a_favourite_without_chapters_has_zero_aggregates_and_last_read_comes_from_history() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let bare = repo.insert(&new_manga("bare", "Bare", true)).unwrap();
        let read = repo.insert(&new_manga("read", "Read", true)).unwrap();
        let chapter_id = db.write(|tx| Ok(chapters::insert(tx, &chapter(read, "c1", None, true, 0))?)).unwrap();
        db.write(|tx| Ok(history::upsert(tx, chapter_id, 4_242, 10)?)).unwrap();

        let rows = repo.library().unwrap();
        let bare_row = rows.iter().find(|r| r.id() == bare).unwrap();
        assert_eq!(
            (bare_row.total_chapters, bare_row.read_count, bare_row.bookmark_count),
            (0, 0, 0)
        );
        assert_eq!(
            (bare_row.latest_upload, bare_row.chapter_fetched_at, bare_row.last_read),
            (0, 0, 0)
        );
        let read_row = rows.iter().find(|r| r.id() == read).unwrap();
        assert_eq!(read_row.last_read, 4_242);
        assert_eq!(read_row.total_chapters, 1);
    }

    #[test]
    fn an_unknown_fetch_type_reads_zero_stats_instead_of_failing_the_query() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let parent = repo.insert(&new_anime("parent", "Parent", true)).unwrap();
        let child = repo
            .insert(&Anime { parent_id: Some(parent), ..new_anime("child", "Child", false) })
            .unwrap();
        db.write(|tx| {
            kuro_db::statements_anime::insert_episode(tx, &episode(parent, "e1", true))?;
            Ok(tx.execute("UPDATE animes SET fetch_type = 7 WHERE _id IN (?1, ?2)", (parent, child))?)
        })
        .unwrap();

        let library = repo.library().expect("library() succeeds for fetch_type 7");
        let row = library.iter().find(|r| r.id() == parent).unwrap();
        assert_eq!(
            (row.total_count, row.seen_count, row.bookmark_count, row.fillermark_count),
            (0, 0, 0, 0)
        );
        assert_eq!((row.latest_upload, row.episode_fetched_at, row.last_seen), (0, 0, 0));

        let seasons = repo.seasons_by_parent(parent).expect("seasons_by_parent succeeds for fetch_type 7");
        assert_eq!(seasons.len(), 1);
        assert_eq!(
            (seasons[0].total_count, seasons[0].seen_count, seasons[0].fillermark_count),
            (0, 0, 0)
        );
    }

    #[test]
    fn the_related_query_returns_33_columns_and_maps_each_row_with_its_group() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let a = repo.insert(&new_anime("a", "A", true)).unwrap();
        let b = repo.insert(&new_anime("b", "B", false)).unwrap();
        let c = repo.insert(&new_anime("c", "C", false)).unwrap();
        db.write(|tx| {
            tx.execute(
                "INSERT INTO anime_relations(anime_id, related_anime_id, name, sort_order, last_fetched_at) \
                 VALUES (?1, ?2, 'Sequels', 0, 1), (?1, ?3, 'Spin-offs', 1, 1)",
                (a, b, c),
            )?;
            Ok(())
        })
        .unwrap();

        let rows = db
            .read(|conn| {
                let mut st = conn.prepare(kuro_db::statements_anime::RELATED_BY_ANIME_ID)?;
                assert_eq!(st.column_count(), 33);
                let mapped = st
                    .query_map(rusqlite::named_params! {":animeId": a}, crate::anime::map_related_anime)?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                Ok(mapped)
            })
            .unwrap();
        let names: Vec<(String, i64)> = rows.into_iter().map(|(name, anime)| (name, anime.id)).collect();
        assert_eq!(names, vec![("Sequels".to_owned(), b), ("Spin-offs".to_owned(), c)]);
    }
}

mod subscriptions {
    use std::sync::Arc;
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

    fn manga_title(published: &Option<Manga>) -> Option<&str> {
        published.as_ref().map(|m| m.title.as_str())
    }

    fn manga_version(published: &Option<Manga>) -> Option<i64> {
        published.as_ref().map(|m| m.version)
    }

    fn anime_title(published: &Option<Anime>) -> Option<&str> {
        published.as_ref().map(|a| a.title.as_str())
    }

    #[tokio::test]
    async fn manga_by_id_publishes_each_change_and_none_once_deleted() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("u", "First", true)).unwrap();
        let mut sub = repo.subscribe_by_id(id);
        assert_eq!(manga_title(&next(&mut sub).await.unwrap()), Some("First"));
        repo.update(&MangaUpdate { title: Some("Second".into()), ..MangaUpdate::new(id) }).unwrap();
        assert_eq!(manga_title(&next(&mut sub).await.expect("the update publishes")), Some("Second"));
        db.write(|tx| Ok(tx.execute("DELETE FROM mangas WHERE _id = ?1", [id])?)).unwrap();
        assert!(next(&mut sub).await.expect("the delete publishes").is_none());
    }

    #[tokio::test]
    async fn manga_by_url_and_source_publishes_the_first_duplicate_and_its_changes() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let first = repo.insert(&new_manga("dup", "First", true)).unwrap();
        repo.insert(&new_manga("dup", "Second", true)).unwrap();
        let mut sub = repo.subscribe_by_url_and_source("dup", 1);
        assert_eq!(manga_title(&next(&mut sub).await.unwrap()), Some("First"));
        repo.update(&MangaUpdate { title: Some("Renamed".into()), ..MangaUpdate::new(first) }).unwrap();
        assert_eq!(manga_title(&next(&mut sub).await.expect("the update publishes")), Some("Renamed"));
    }

    #[tokio::test]
    async fn manga_favorites_by_source_publishes_when_a_favourite_joins_and_not_for_other_rows() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let mut sub = repo.subscribe_favorites_by_source(1);
        assert!(next(&mut sub).await.unwrap().is_empty());
        repo.insert(&new_manga("hidden", "Hidden", false)).unwrap();
        assert!(next(&mut sub).await.is_none(), "a non-favourite leaves the result unchanged");
        repo.insert(&new_manga("fav", "Fav", true)).unwrap();
        assert_eq!(next(&mut sub).await.expect("a favourite publishes").len(), 1);
    }

    #[tokio::test]
    async fn manga_library_publishes_a_read_and_a_trigger_write_reaches_the_parent() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("m", "M", true)).unwrap();
        let chapter_id = db.write(|tx| Ok(chapters::insert(tx, &chapter(id, "c1", None, false, 0))?)).unwrap();
        let mut library = repo.subscribe_library();
        assert_eq!(library_rows(&mut library).await[0].unread_count(), 1);
        let mut by_id = repo.subscribe_by_id(id);
        let before = manga_version(&next(&mut by_id).await.unwrap()).unwrap();

        db.write(|tx| Ok(tx.execute("UPDATE chapters SET read = 1 WHERE _id = ?1", [chapter_id])?)).unwrap();
        let rows = library_rows(&mut library).await;
        assert_eq!((rows[0].read_count, rows[0].unread_count()), (1, 0));
        let after = manga_version(&next(&mut by_id).await.expect("the trigger's parent write publishes"));
        assert!(after.unwrap() > before, "update_chapter_and_manga_version bumped the manga");
    }

    #[tokio::test]
    async fn deleting_a_manga_publishes_an_empty_chapter_list() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let id = repo.insert(&new_manga("m", "M", true)).unwrap();
        db.write(|tx| {
            chapters::insert(tx, &chapter(id, "c1", None, false, 0))?;
            chapters::insert(tx, &chapter(id, "c2", None, false, 0))?;
            Ok(())
        })
        .unwrap();
        let mut list = db.subscribe(&["chapters"], Duration::ZERO, |c| {
            Ok(c.query_row("SELECT count(*) FROM chapters", [], |r| r.get::<_, i64>(0))?)
        });
        assert_eq!(*next(&mut list).await.unwrap(), 2);
        db.write(|tx| Ok(tx.execute("DELETE FROM mangas WHERE _id = ?1", [id])?)).unwrap();
        assert_eq!(*next(&mut list).await.expect("the cascade publishes"), 0);
    }

    #[tokio::test]
    async fn anime_by_id_publishes_each_change_and_none_once_deleted() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let id = repo.insert(&new_anime("u", "First", true)).unwrap();
        let mut sub = repo.subscribe_by_id(id);
        assert_eq!(anime_title(&next(&mut sub).await.unwrap()), Some("First"));
        repo.update(&AnimeUpdate { title: Some("Second".into()), ..AnimeUpdate::new(id) }).unwrap();
        assert_eq!(anime_title(&next(&mut sub).await.expect("the update publishes")), Some("Second"));
        db.write(|tx| Ok(tx.execute("DELETE FROM animes WHERE _id = ?1", [id])?)).unwrap();
        assert!(next(&mut sub).await.expect("the delete publishes").is_none());
    }

    #[tokio::test]
    async fn anime_by_url_and_source_publishes_the_lowest_id_on_duplicates() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let first = repo.insert(&new_anime("dup", "First", true)).unwrap();
        repo.insert(&new_anime("dup", "Second", true)).unwrap();
        let mut sub = repo.subscribe_by_url_and_source("dup", 1);
        assert_eq!(anime_title(&next(&mut sub).await.unwrap()), Some("First"));
        repo.update(&AnimeUpdate { title: Some("Renamed".into()), ..AnimeUpdate::new(first) }).unwrap();
        assert_eq!(anime_title(&next(&mut sub).await.expect("the update publishes")), Some("Renamed"));
    }

    #[tokio::test]
    async fn anime_favorites_by_source_publishes_favourites_only() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let mut sub = repo.subscribe_favorites_by_source(1);
        assert!(next(&mut sub).await.unwrap().is_empty());
        repo.insert(&new_anime("hidden", "Hidden", false)).unwrap();
        assert!(next(&mut sub).await.is_none(), "a non-favourite leaves the result unchanged");
        repo.insert(&new_anime("fav", "Fav", true)).unwrap();
        assert_eq!(next(&mut sub).await.expect("a favourite publishes").len(), 1);
    }

    #[tokio::test]
    async fn anime_library_publishes_when_an_episode_is_seen() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let id = repo.insert(&new_anime("a", "A", true)).unwrap();
        let mut sub = repo.subscribe_library();
        assert_eq!(library_rows(&mut sub).await[0].total_count, 0);
        let episode_id = db.write(|tx| {
            Ok(kuro_db::statements_anime::insert_episode(tx, &episode(id, "e1", false))?)
        })
        .unwrap();
        assert_eq!(library_rows(&mut sub).await[0].total_count, 1);
        db.write(|tx| Ok(tx.execute("UPDATE episodes SET seen = 1 WHERE _id = ?1", [episode_id])?)).unwrap();
        assert_eq!(library_rows(&mut sub).await[0].seen_count, 1);
    }

    #[tokio::test]
    async fn seasons_of_a_parent_publish_after_an_episode_is_seen_and_after_a_child_is_renamed() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let parent = repo.insert(&new_anime("p", "P", true)).unwrap();
        let child = repo.insert(&Anime { parent_id: Some(parent), ..new_anime("c", "C", false) }).unwrap();
        let mut sub = repo.subscribe_seasons_by_parent(parent);
        let first = next(&mut sub).await.expect("a season publication");
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].total_count, 0);
        let episode_id = db
            .write(|tx| Ok(kuro_db::statements_anime::insert_episode(tx, &episode(child, "e1", false))?))
            .unwrap();
        let after_episode = next(&mut sub).await.expect("a publication after the episode insert");
        assert_eq!(after_episode[0].total_count, 1);
        db.write(|tx| Ok(tx.execute("UPDATE episodes SET seen = 1 WHERE _id = ?1", [episode_id])?)).unwrap();
        assert_eq!(next(&mut sub).await.expect("a publication after the seen flag").first().map(|s| s.seen_count), Some(1));
        repo.update(&AnimeUpdate { title: Some("Renamed".into()), ..AnimeUpdate::new(child) }).unwrap();
        let renamed = next(&mut sub).await.expect("a publication after the child rename");
        assert_eq!(renamed[0].anime.title, "Renamed");
    }

    #[tokio::test]
    async fn non_library_counts_publish_again_when_a_manga_changes() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let mut sub = repo.subscribe_non_library_counts();
        let first = next(&mut sub).await.expect("an initial publication");
        assert!(first.is_empty());
        let id = repo.insert(&Manga { source: 2, ..new_manga("c", "C", false) }).unwrap();
        let after_insert = next(&mut sub).await.expect("a publication after an insert");
        assert_eq!(*after_insert, vec![(2, 1)]);
        repo.update(&MangaUpdate { favorite: Some(true), ..MangaUpdate::new(id) }).unwrap();
        let after_favourite = next(&mut sub).await.expect("a publication after a favourite flip");
        assert!(after_favourite.is_empty());
    }
}

mod anime_reads {
    use super::*;

    #[test]
    fn by_url_and_source_takes_the_lowest_id_and_favorites_follow_their_sql() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let first = repo.insert(&new_anime("dup", "First", true)).unwrap();
        repo.insert(&new_anime("dup", "Second", true)).unwrap();
        let other = repo.insert(&Anime { source: 2, ..new_anime("other", "Other", true) }).unwrap();
        repo.insert(&new_anime("hidden", "Hidden", false)).unwrap();

        assert_eq!(repo.by_url_and_source("dup", 1).unwrap().map(|a| a.id), Some(first));
        assert_eq!(repo.by_url_and_source("missing", 1).unwrap(), None);
        assert_eq!(repo.favorites().unwrap().len(), 3, "the non-favourite is not a favourite");
        assert_eq!(repo.favorites_by_source(2).unwrap().iter().map(|a| a.id).collect::<Vec<_>>(), vec![other]);
    }
}

mod library_retry {
    use std::cell::Cell;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    use kuro_db::subscribe::Subscription;
    use rusqlite::Connection;

    use super::*;
    use crate::retrying;

    const MANGA_TABLES: &[&str] = &["mangas"];

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
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        repo.insert(&new_manga("a", "A", true)).expect("insert");
        let fail = Arc::new(AtomicBool::new(false));
        let injected = Arc::clone(&fail);
        let query = retrying(20, Duration::ZERO, move |c: &Connection| {
            if injected.load(Ordering::SeqCst) {
                return Err(DataError::Corrupt("injected".into()));
            }
            Ok(c.query_row("SELECT count(*) FROM mangas", [], |r| r.get::<_, i64>(0))?)
        });
        let mut sub = db.subscribe(MANGA_TABLES, Duration::ZERO, query);
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
        let (_d, db) = manga_db();
        let query = retrying(20, Duration::ZERO, |_c: &Connection| Err::<i64, _>(busy()));
        let mut sub = db.subscribe(MANGA_TABLES, Duration::ZERO, query);
        let published = next(&mut sub).await.expect("first publication");
        assert!(published.is_err(), "the subscription surfaces the error value");
    }
}

mod item_repos {
    use std::time::Duration;

    use kuro_db::statements_anime::insert_episode;
    use kuro_db::statements_manga::excluded_scanlators;

    use super::*;
    use crate::chapter::{ChapterRepo, ChapterUpdate};
    use crate::episode::{EpisodeRepo, EpisodeUpdate};

    #[test]
    fn chapter_reads_by_id_url_and_scanlator_filter() {
        let (_d, db) = manga_db();
        let manga_id = MangaRepo::new(db.clone()).insert(&new_manga("m", "M", true)).unwrap();
        db.write(|tx| {
            chapters::insert(tx, &chapter(manga_id, "c1", Some("S1"), false, 0))?;
            chapters::insert(tx, &chapter(manga_id, "c2", Some("S2"), false, 0))?;
            excluded_scanlators::insert(tx, manga_id, "S2")?;
            Ok(())
        })
        .unwrap();
        let repo = ChapterRepo::new(db.clone());
        assert_eq!(repo.by_manga_id(manga_id, false).unwrap().len(), 2);
        assert_eq!(repo.by_manga_id(manga_id, true).unwrap().len(), 1, "excluded scanlator dropped");
        let c1 = repo.by_url_and_manga_id("c1", manga_id).unwrap().expect("by url");
        assert_eq!(repo.by_id(c1.id).unwrap(), Some(c1));
        assert_eq!(repo.by_id(9_999).unwrap(), None, "a missing row is None, not an error");
        assert_eq!(repo.by_url_and_manga_id("c1", manga_id + 1).unwrap(), None);
    }

    #[test]
    fn chapter_update_writes_the_set_fields() {
        let (_d, db) = manga_db();
        let manga_id = MangaRepo::new(db.clone()).insert(&new_manga("m", "M", true)).unwrap();
        let id = db.write(|tx| Ok(chapters::insert(tx, &chapter(manga_id, "c", None, false, 0))?)).unwrap();
        let repo = ChapterRepo::new(db);
        repo.update(&ChapterUpdate { read: Some(true), last_page_read: Some(4), ..ChapterUpdate::new(id) }).unwrap();
        let after = repo.by_id(id).unwrap().expect("row");
        assert_eq!((after.read, after.last_page_read), (true, 4));
    }

    #[test]
    fn chapter_update_all_rolls_back_on_failure() {
        let (_d, db) = manga_db();
        let manga_id = MangaRepo::new(db.clone()).insert(&new_manga("m", "M", true)).unwrap();
        let (a, b) = db
            .write(|tx| Ok((chapters::insert(tx, &chapter(manga_id, "a", None, false, 0))?, chapters::insert(tx, &chapter(manga_id, "b", None, false, 0))?)))
            .unwrap();
        abort_updates_of(&db, "chapters", b);
        let repo = ChapterRepo::new(db);
        let result = repo.update_all(&[
            ChapterUpdate { read: Some(true), ..ChapterUpdate::new(a) },
            ChapterUpdate { read: Some(true), ..ChapterUpdate::new(b) },
        ]);
        assert!(result.is_err());
        assert!(!repo.by_id(a).unwrap().expect("row a").read, "the first update rolled back");
    }

    #[tokio::test]
    async fn chapter_subscription_publishes_a_read_toggle() {
        let (_d, db) = manga_db();
        let manga_id = MangaRepo::new(db.clone()).insert(&new_manga("m", "M", true)).unwrap();
        let id = db.write(|tx| Ok(chapters::insert(tx, &chapter(manga_id, "c", None, false, 0))?)).unwrap();
        let repo = ChapterRepo::new(db);
        let mut sub = repo.subscribe_by_manga_id(manga_id, false);
        assert!(!next(&mut sub).await.expect("first publication")[0].read);
        repo.update(&ChapterUpdate { read: Some(true), ..ChapterUpdate::new(id) }).unwrap();
        assert!(next(&mut sub).await.expect("the toggle publishes")[0].read);
    }

    #[test]
    fn episode_reads_by_id_url_and_anime() {
        let (_d, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone()).insert(&new_anime("a", "A", true)).unwrap();
        let id = db.write(|tx| Ok(insert_episode(tx, &episode(anime_id, "e1", false))?)).unwrap();
        let repo = EpisodeRepo::new(db);
        let e1 = repo.by_id(id).unwrap().expect("by id");
        assert_eq!(repo.by_url_and_anime_id("e1", anime_id).unwrap(), Some(e1.clone()));
        assert_eq!(repo.by_anime_id(anime_id).unwrap(), vec![e1]);
        assert_eq!(repo.by_id(9_999).unwrap(), None);
    }

    #[test]
    fn episode_update_writes_summary_memo_and_seen() {
        let (_d, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone()).insert(&new_anime("a", "A", true)).unwrap();
        let id = db.write(|tx| Ok(insert_episode(tx, &episode(anime_id, "e1", false))?)).unwrap();
        let repo = EpisodeRepo::new(db);
        let mut memo = serde_json::Map::new();
        memo.insert("k".into(), serde_json::json!(1));
        repo.update(&EpisodeUpdate {
            seen: Some(true),
            summary: Some("s".into()),
            memo: Some(memo.clone()),
            ..EpisodeUpdate::new(id)
        })
        .unwrap();
        let after = repo.by_id(id).unwrap().expect("row");
        assert_eq!((after.seen, after.summary.as_deref(), after.memo), (true, Some("s"), memo));
    }

    /// Every value from -1.0 to 10000.0 in 0.5 steps is stored in the REAL column and reads back as the same f32 (T-16a-007).
    #[test]
    fn episode_number_survives_a_sqlite_real_round_trip_as_f32() {
        let (_d, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone()).insert(&new_anime("a", "A", true)).unwrap();
        let numbers: Vec<f32> = (-2..=20_000).map(|half| half as f32 * 0.5).collect();
        db.write(|tx| {
            for (i, number) in numbers.iter().enumerate() {
                let mut row = episode(anime_id, &format!("e{i}"), false);
                row.episode_number = f64::from(*number);
                insert_episode(tx, &row)?;
            }
            Ok(())
        })
        .unwrap();
        let mut read: Vec<f32> =
            EpisodeRepo::new(db).by_anime_id(anime_id).unwrap().iter().map(|e| e.episode_number as f32).collect();
        read.sort_by(f32::total_cmp);
        assert_eq!(read, numbers);
    }

    #[tokio::test]
    async fn episode_subscription_publishes_a_new_episode() {
        let (_d, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone()).insert(&new_anime("a", "A", true)).unwrap();
        let repo = EpisodeRepo::new(db.clone());
        let mut sub = repo.subscribe_by_anime_id(anime_id);
        assert!(next(&mut sub).await.expect("first publication").is_empty());
        db.write(|tx| Ok(insert_episode(tx, &episode(anime_id, "e1", false))?)).unwrap();
        assert_eq!(next(&mut sub).await.expect("the insert publishes").len(), 1);
    }

    async fn next<T>(sub: &mut kuro_db::subscribe::Subscription<T>) -> Option<std::sync::Arc<T>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
    }
}

/// Keys (T-06a-034), chapter and episode lookups by url (T-05a-031), the deletable-parents subscription
/// (T-05a-017), chunked parent clearing (T-05a-017) and the upcoming subscriptions with an injected clock
/// (T-05a-020).
mod keys_lookups_and_clock {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::time::Duration;

    use chrono::TimeZone as _;
    use kuro_db::statements_anime::insert_episode;
    use kuro_db::subscribe::Subscription;

    use super::*;
    use crate::chapter::{ChapterKey, ChapterRepo};
    use crate::episode::{EpisodeKey, EpisodeRepo};

    async fn next<T>(sub: &mut Subscription<T>) -> Option<Arc<T>> {
        tokio::time::timeout(Duration::from_millis(500), sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
    }

    fn manga_titles(published: Option<Arc<Vec<Manga>>>) -> Vec<String> {
        published.expect("a publication").iter().map(|m| m.title.clone()).collect()
    }

    #[test]
    fn chapter_key_is_the_id_and_url_pair() {
        let (_d, db) = manga_db();
        let manga_id = MangaRepo::new(db.clone()).insert(&new_manga("m", "M", true)).unwrap();
        db.write(|tx| Ok(chapters::insert(tx, &chapter(manga_id, "c1", None, false, 0))?)).unwrap();
        let c = ChapterRepo::new(db).by_url_and_manga_id("c1", manga_id).unwrap().expect("row");
        let mut set = HashSet::new();
        set.insert(ChapterKey::from(&c));
        assert!(set.contains(&ChapterKey::from(&c)), "the same row is the same key");
        let mut other_url = c.clone();
        other_url.url = "c2".into();
        assert_ne!(ChapterKey::from(&other_url), ChapterKey::from(&c));
        let mut other_id = c.clone();
        other_id.id += 1;
        assert_ne!(ChapterKey::from(&other_id), ChapterKey::from(&c));
    }

    #[test]
    fn episode_key_is_the_id_and_url_pair() {
        let (_d, db) = anime_db();
        let anime_id = AnimeRepo::new(db.clone()).insert(&new_anime("a", "A", true)).unwrap();
        db.write(|tx| Ok(insert_episode(tx, &episode(anime_id, "e1", false))?)).unwrap();
        let e = EpisodeRepo::new(db).by_url_and_anime_id("e1", anime_id).unwrap().expect("row");
        let mut set = HashSet::new();
        set.insert(EpisodeKey::from(&e));
        assert!(set.contains(&EpisodeKey::from(&e)));
        let mut other_url = e.clone();
        other_url.url = "e2".into();
        assert_ne!(EpisodeKey::from(&other_url), EpisodeKey::from(&e));
    }

    #[test]
    fn chapter_by_url_lists_every_match_in_any_manga() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db.clone());
        let a = repo.insert(&new_manga("a", "A", true)).unwrap();
        let b = repo.insert(&new_manga("b", "B", false)).unwrap();
        db.write(|tx| {
            chapters::insert(tx, &chapter(a, "shared", None, false, 0))?;
            chapters::insert(tx, &chapter(b, "shared", None, false, 0))?;
            chapters::insert(tx, &chapter(a, "only-a", None, false, 0))?;
            Ok(())
        })
        .unwrap();
        let mut mangas: Vec<i64> = ChapterRepo::new(db.clone()).by_url("shared").unwrap().iter().map(|c| c.manga_id).collect();
        mangas.sort_unstable();
        assert_eq!(mangas, vec![a.min(b), a.max(b)]);
        assert!(ChapterRepo::new(db).by_url("missing").unwrap().is_empty());
    }

    #[test]
    fn episode_by_url_lists_every_match_in_any_anime() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db.clone());
        let a = repo.insert(&new_anime("a", "A", true)).unwrap();
        let b = repo.insert(&new_anime("b", "B", false)).unwrap();
        db.write(|tx| {
            insert_episode(tx, &episode(a, "shared", false))?;
            insert_episode(tx, &episode(b, "shared", false))?;
            Ok(())
        })
        .unwrap();
        let mut animes: Vec<i64> = EpisodeRepo::new(db.clone()).by_url("shared").unwrap().iter().map(|e| e.anime_id).collect();
        animes.sort_unstable();
        assert_eq!(animes, vec![a.min(b), a.max(b)]);
        assert!(EpisodeRepo::new(db).by_url("missing").unwrap().is_empty());
    }

    #[test]
    fn remove_parent_ids_clears_links_across_more_than_one_chunk() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let parent = repo.insert(&new_anime("p", "P", false)).unwrap();
        let mut child = new_anime("c", "C", false);
        child.parent_id = Some(parent);
        let child_id = repo.insert(&child).unwrap();
        // More than one chunk of ids, with the real child last so it sits in the second chunk.
        let mut ids: Vec<i64> = (100_000..101_200).collect();
        ids.push(child_id);
        repo.remove_parent_ids(&ids).unwrap();
        assert_eq!(repo.by_id(child_id).unwrap().parent_id, None);
    }

    #[tokio::test]
    async fn deletable_parents_subscription_publishes_each_new_candidate() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        repo.insert(&new_anime("fav", "Fav", true)).unwrap();
        let mut sub = repo.subscribe_deletable_parents();
        assert_eq!(next(&mut sub).await.expect("first publication").len(), 0, "favourites are never candidates");
        repo.insert(&new_anime("gone", "Gone", false)).unwrap();
        assert_eq!(next(&mut sub).await.expect("the insert publishes").len(), 1);
    }

    #[tokio::test]
    async fn manga_upcoming_subscription_recomputes_the_cut_off_on_every_run() {
        let (_d, db) = manga_db();
        let repo = MangaRepo::new(db);
        let day1 = chrono::Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap().timestamp_millis();
        let start1 = time::start_of_local_day_millis(day1);
        let mut due = new_manga("due", "Due", true);
        due.status = 1;
        due.next_update = start1 + 3_600_000;
        let id = repo.insert(&due).unwrap();
        let clock = Arc::new(AtomicI64::new(day1));
        let ticks = Arc::clone(&clock);
        let mut sub = repo.subscribe_upcoming_with_clock(&MANGA_STATUSES, move || ticks.load(Ordering::SeqCst));
        assert_eq!(manga_titles(next(&mut sub).await), vec!["Due"]);
        // The next day starts after the entry's time. A write re-runs the query, which reads the new clock.
        clock.store(day1 + 86_400_000, Ordering::SeqCst);
        repo.update(&MangaUpdate { title: Some("Due again".into()), ..MangaUpdate::new(id) }).unwrap();
        assert_eq!(manga_titles(next(&mut sub).await), Vec::<String>::new(), "the cut-off moved to the new day");
    }

    #[tokio::test]
    async fn anime_upcoming_subscription_uses_the_injected_clock() {
        let (_d, db) = anime_db();
        let repo = AnimeRepo::new(db);
        let day1 = chrono::Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).unwrap().timestamp_millis();
        let mut due = new_anime("due", "Due", true);
        due.status = 1;
        due.next_update = time::start_of_local_day_millis(day1) + 3_600_000;
        repo.insert(&due).unwrap();
        let mut sub = repo.subscribe_upcoming_with_clock(&ANIME_STATUSES, move || day1);
        let shown = next(&mut sub).await.expect("first publication");
        assert_eq!(shown.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(), vec!["Due"]);
    }
}
