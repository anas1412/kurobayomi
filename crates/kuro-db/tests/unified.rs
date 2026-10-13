//! The unified database (docs/design/unified-db.md section 6, the kuro-db part): one file, both kinds.

use std::path::Path;

use kuro_db::codec::FetchType;
use kuro_db::schema_unified::{self, APPLICATION_ID, SCHEMA_VERSION, db_path, legacy_files, open_file};
use kuro_db::tables::{ItemPatch, categories, entries, history, items, library, stub_sources};
use kuro_db::{DB_TABLES, Db, DbError};
use kuro_platform::kind::Kind;
use kuro_platform::paths::Paths;
use kuro_testkit::{TempDir, TempPaths};
use rusqlite::Connection;

mod common;
use common::{entry, item_row};

fn add(c: &Connection, kind: Kind, source: i64, url: &str, title: &str) -> i64 {
    entries::insert(c, &entry(kind, source, url, title)).unwrap()
}

fn scalar(c: &Connection, sql: &str) -> i64 {
    c.query_row(sql, [], |r| r.get(0)).unwrap()
}

fn file_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn a_fresh_profile_creates_one_file_with_every_table_and_view() {
    let paths = TempPaths::new().unwrap();
    let db = Db::open(&paths).unwrap();
    let data = paths.data_dir();
    assert!(db_path(&data).is_file());
    assert!(!data.join("manga.db").exists() && !data.join("anime.db").exists());
    assert_eq!(db_path(&data).file_name().unwrap(), "kurobayomi.db");
    db.read(|c| {
        assert_eq!(scalar(c, "PRAGMA user_version"), SCHEMA_VERSION);
        assert_eq!(scalar(c, "PRAGMA application_id"), APPLICATION_ID);
        assert_eq!(APPLICATION_ID, 0x4B55_5232);
        for table in DB_TABLES {
            let n: i64 = c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'table' AND name = ?1", [table], |r| r.get(0))?;
            assert_eq!(n, 1, "table {table}");
        }
        for view in ["historyView", "updatesView", "libraryView"] {
            let n: i64 = c.query_row("SELECT count(*) FROM sqlite_master WHERE type = 'view' AND name = ?1", [view], |r| r.get(0))?;
            assert_eq!(n, 1, "view {view}");
        }
        assert_eq!(scalar(c, "SELECT count(*) FROM sqlite_master WHERE type = 'view'"), 3);
        let system: Vec<(String, i64)> = c
            .prepare("SELECT kind, flags FROM categories WHERE _id = 0 ORDER BY kind")?
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .map(Result::unwrap)
            .collect();
        assert_eq!(system, [("anime".to_owned(), 64), ("manga".to_owned(), 64)]);
        Ok(())
    })
    .unwrap();
}

#[test]
fn old_files_are_untouched_and_listed_by_legacy_files() {
    let paths = TempPaths::new().unwrap();
    let data = paths.data_dir();
    std::fs::create_dir_all(&data).unwrap();
    // an Aniyomi-shaped file (SQLite, application_id 0) and a junk file, each with a -wal sibling
    {
        let c = Connection::open(data.join("anime.db")).unwrap();
        c.execute_batch("CREATE TABLE animes(_id INTEGER PRIMARY KEY, title TEXT); INSERT INTO animes VALUES (1, 'x');").unwrap();
        c.pragma_update(None, "user_version", 139).unwrap();
    }
    std::fs::write(data.join("manga.db"), b"not a database at all ".repeat(100)).unwrap();
    std::fs::write(data.join("manga.db-wal"), b"wal bytes").unwrap();
    let snapshot = |name: &str| (std::fs::read(data.join(name)).unwrap(), std::fs::metadata(data.join(name)).unwrap().modified().unwrap());
    let before: Vec<_> = ["manga.db", "manga.db-wal", "anime.db"].iter().map(|n| snapshot(n)).collect();

    assert_eq!(legacy_files(&data), [data.join("manga.db"), data.join("anime.db")]);
    let db = Db::open(&paths).unwrap();
    db.close().unwrap();
    drop(db);

    let after: Vec<_> = ["manga.db", "manga.db-wal", "anime.db"].iter().map(|n| snapshot(n)).collect();
    assert_eq!(before, after, "bytes and mtimes of the old files are unchanged");
    assert_eq!(legacy_files(&data), [data.join("manga.db"), data.join("anime.db")], "still listed after the open");
    let names = file_names(&data);
    assert!(names.contains(&"kurobayomi.db".to_owned()));
    assert!(!names.iter().any(|n| n.contains("corrupt")), "{names:?}");

    // an empty or missing old file is not listed
    let empty = TempPaths::new().unwrap();
    let folder = empty.data_dir();
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join(kuro_db::schema_unified::LEGACY_FILES[0]), b"").unwrap();
    assert!(legacy_files(&folder).is_empty());
}

#[test]
fn a_foreign_file_at_the_new_path_is_refused_untouched() {
    let dir = TempDir::new().unwrap();
    let path = db_path(dir.path());
    {
        let c = Connection::open(&path).unwrap();
        c.execute_batch("CREATE TABLE mangas(_id INTEGER PRIMARY KEY);").unwrap();
        c.pragma_update(None, "user_version", 1).unwrap();
    }
    let before = std::fs::read(&path).unwrap();
    assert!(matches!(open_file(&path), Err(DbError::ForeignFile { found: 1 })));
    assert_eq!(std::fs::read(&path).unwrap(), before);
}

#[test]
fn a_local_manga_and_a_local_anime_with_the_same_url_both_appear() {
    let c = schema_unified::open_in_memory().unwrap();
    let manga_id = add(&c, Kind::Manga, 0, "X", "Local manga");
    let anime_id = add(&c, Kind::Anime, 0, "X", "Local anime");
    assert_ne!(manga_id, anime_id, "entry ids are unique across kinds");
    for kind in Kind::ALL {
        assert!(entries::insert(&c, &entry(kind, 0, "X", "Again")).is_err(), "UNIQUE (kind, source, url) for {kind}");
    }

    let kinds: Vec<(String, i64)> = c
        .prepare("SELECT kind, _id FROM libraryView ORDER BY kind")
        .unwrap()
        .query_map([], |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(kinds, [("anime".to_owned(), anime_id), ("manga".to_owned(), manga_id)]);

    assert_eq!(entries::by_url_and_source(&c, Kind::Manga, "X", 0).unwrap().map(|m| m.id), Some(manga_id));
    assert_eq!(entries::by_url_and_source(&c, Kind::Anime, "X", 0).unwrap().map(|a| a.id), Some(anime_id));

    stub_sources::upsert(&c, Kind::Manga, 0, "en", "Local").unwrap();
    stub_sources::upsert(&c, Kind::Anime, 0, "ja", "Local anime").unwrap();
    assert_eq!(scalar(&c, "SELECT count(*) FROM stub_sources WHERE id = 0"), 2);
    assert_eq!(stub_sources::find_one(&c, Kind::Manga, 0).unwrap().unwrap().lang, "en");
    assert_eq!(stub_sources::find_one(&c, Kind::Anime, 0).unwrap().unwrap().lang, "ja");

    for kind in Kind::ALL {
        assert_eq!(entries::favorite_counts(&c, kind).unwrap(), [(0, 1)], "{kind}");
        assert_eq!(entries::favorites(&c, kind).unwrap().len(), 1, "{kind}");
    }
}

#[test]
fn categories_are_per_kind_and_links_cannot_cross() {
    let c = schema_unified::open_in_memory().unwrap();
    let manga_cat = categories::insert(&c, Kind::Manga, "Reading", 0, 0).unwrap();
    let anime_cat = categories::insert(&c, Kind::Anime, "Watching", 0, 0).unwrap();
    assert_eq!((manga_cat, anime_cat), (1, 1), "ids are per kind");
    let second = categories::insert(&c, Kind::Manga, "Second", 1, 0).unwrap();
    assert_eq!(second, 2);
    assert_eq!(categories::all(&c, Kind::Manga).unwrap().len(), 3, "system row plus two");
    assert_eq!(categories::all(&c, Kind::Anime).unwrap().len(), 2, "system row plus one");

    let m = add(&c, Kind::Manga, 1, "/m", "M");
    let a = add(&c, Kind::Anime, 1, "/a", "A");
    // an anime entry cannot join a manga category: the foreign key on (kind, category_id) refuses it
    assert!(c.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'anime', 2)", [a]).is_err());
    // and an anime entry id under kind 'manga' fails on the entry side
    assert!(c.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'manga', ?2)", [a, manga_cat]).is_err());
    let link = |entry_id: i64, ids: &[i64]| {
        let tx = c.unchecked_transaction().unwrap();
        entries::set_categories(&tx, entry_id, ids).unwrap();
        tx.commit().unwrap();
    };
    link(m, &[manga_cat, manga_cat]);
    assert_eq!(scalar(&c, "SELECT count(*) FROM entry_categories"), 1, "a repeated membership is ignored");
    link(a, &[anime_cat]);

    let err = categories::delete(&c, Kind::Anime, 0).unwrap_err();
    assert!(err.to_string().contains("System category can't be deleted"), "{err}");
    assert!(categories::delete(&c, Kind::Manga, 0).is_err());

    let version = |id: i64| scalar(&c, &format!("SELECT version FROM entries WHERE _id = {id}"));
    let before = version(m);
    assert_eq!(categories::delete(&c, Kind::Manga, manga_cat).unwrap(), 1);
    assert_eq!(scalar(&c, &format!("SELECT count(*) FROM entry_categories WHERE entry_id = {m}")), 0, "its links go");
    assert_eq!(version(m), before, "deleting a category does not bump the version");
    assert_eq!(categories::by_entry(&c, Kind::Anime, a).unwrap().len(), 1, "the anime category is untouched");
}

#[test]
fn history_search_folds_non_ascii_for_both_kinds() {
    let c = schema_unified::open_in_memory().unwrap();
    let m = add(&c, Kind::Manga, 1, "/m", "Éclair 100%");
    let ch = items::insert(&c, &item_row(m, Kind::Manga, "/m/1", 1.0)).unwrap();
    history::upsert(&c, ch, 500, 1).unwrap();
    let a = add(&c, Kind::Anime, 1, "/a", "Привет_мир");
    let ep = items::insert(&c, &item_row(a, Kind::Anime, "/a/1", 1.0)).unwrap();
    history::upsert(&c, ep, 600, 0).unwrap();

    let hits = |kind: Kind, q: &str| history::list(&c, kind, q).unwrap().iter().map(|r| r.entry_id).collect::<Vec<_>>();
    for q in ["éclair", "ÉCLAIR", "Éclair"] {
        assert_eq!(hits(Kind::Manga, q), [m], "{q}");
    }
    for q in ["привет", "ПРИВЕТ"] {
        assert_eq!(hits(Kind::Anime, q), [a], "{q}");
    }
    assert_eq!(hits(Kind::Manga, "100%"), [m], "% is literal");
    assert!(hits(Kind::Manga, "a%").is_empty());
    assert_eq!(hits(Kind::Anime, "т_м"), [a], "_ is literal");
    assert!(hits(Kind::Anime, "т%м").is_empty());
    assert!(hits(Kind::Manga, "привет").is_empty(), "an anime title is not a manga hit");
    assert!(hits(Kind::Anime, "éclair").is_empty(), "a manga title is not an anime hit");
    assert_eq!(hits(Kind::Manga, "").len(), 1);
    assert_eq!(hits(Kind::Anime, "").len(), 1);
}

#[test]
fn chapter_and_episode_progress_persist_across_reopen() {
    let dir = TempDir::new().unwrap();
    let path = db_path(dir.path());
    let (m, ch, a, ep) = {
        let c = open_file(&path).unwrap();
        let m = add(&c, Kind::Manga, 1, "/m", "M");
        let ch = items::insert(&c, &item_row(m, Kind::Manga, "/m/1", 1.0)).unwrap();
        let a = add(&c, Kind::Anime, 1, "/a", "A");
        let ep = items::insert(&c, &item_row(a, Kind::Anime, "/a/1", 1.0)).unwrap();
        items::update(&c, &ItemPatch { id: ch, done: Some(true), progress: Some(7), ..Default::default() }).unwrap();
        items::update(&c, &ItemPatch { id: ep, done: Some(true), progress: Some(30), ..Default::default() }).unwrap();
        history::upsert(&c, ch, 500, 40).unwrap();
        history::upsert(&c, ep, 600, 0).unwrap();
        (m, ch, a, ep)
    };
    assert_ne!(ch, ep, "item ids are unique across kinds");

    let c = open_file(&path).unwrap();
    let chapter = items::by_id(&c, ch).unwrap();
    assert_eq!((chapter.done, chapter.progress, chapter.version, chapter.kind), (true, 7, 1, Kind::Manga));
    let episode = items::by_id(&c, ep).unwrap();
    assert_eq!((episode.done, episode.progress, episode.version, episode.kind), (true, 30, 1, Kind::Anime));
    assert_eq!(scalar(&c, &format!("SELECT version FROM entries WHERE _id = {m}")), 1, "the manga version followed its chapter");
    assert_eq!(scalar(&c, &format!("SELECT version FROM entries WHERE _id = {a}")), 1, "the anime version followed its episode");
    let h = history::by_entry(&c, m).unwrap();
    assert_eq!((h[0].last_at, h[0].duration_ms), (500, 40), "the chapter keeps its duration");
    assert_eq!(scalar(&c, &format!("SELECT duration_ms FROM history WHERE item_id = {ep}")), 0, "an episode writes none");

    let lib = library::library(&c, Kind::Manga).unwrap();
    assert_eq!((lib[0].total, lib[0].done, lib[0].last_activity), (1, 1, 500));
    let alib = library::library(&c, Kind::Anime).unwrap();
    assert_eq!((alib[0].total, alib[0].done, alib[0].last_activity), (1, 1, 600));
}

#[test]
fn seasons_of_matches_the_library_rollup_and_also_works_for_a_browsed_parent() {
    let c = schema_unified::open_in_memory().unwrap();
    let parent = entries::insert(&c, &kuro_db::tables::EntryRow { fetch_type: Some(FetchType::Seasons), ..entry(Kind::Anime, 1, "/p", "P") }).unwrap();
    let mut children = Vec::new();
    for (url, seen) in [("/s1", [true, true]), ("/s2", [true, false]), ("/s3", [false, false])] {
        let id = entries::insert(&c, &kuro_db::tables::EntryRow { parent_id: Some(parent), favorite: false, ..entry(Kind::Anime, 1, url, url) }).unwrap();
        for (i, done) in seen.into_iter().enumerate() {
            let row = kuro_db::tables::ItemRow { done, date_upload: 10 + i as i64, ..item_row(id, Kind::Anime, &format!("{url}/{i}"), 1.0) };
            let ep = items::insert(&c, &row).unwrap();
            history::upsert(&c, ep, 100 + i as i64, 0).unwrap();
        }
        children.push(id);
    }
    let from_library = library::library(&c, Kind::Anime).unwrap();
    let row = from_library.iter().find(|r| r.entry.id == parent).expect("the favourite parent is in the library");
    let seasons = entries::seasons_of(&c, parent).unwrap();
    assert_eq!(seasons.iter().map(|s| s.entry.id).collect::<Vec<_>>(), children);
    let fully_done = seasons.iter().filter(|s| s.total == s.done).count() as i64;
    let max = |f: fn(&kuro_db::tables::SeasonRow) -> i64| seasons.iter().map(f).max().unwrap();
    assert_eq!(
        (row.total, row.done, row.latest_upload, row.item_fetched_at, row.last_activity),
        (seasons.len() as i64, fully_done, max(|s| s.latest_upload), max(|s| s.fetched_at), max(|s| s.last_activity))
    );

    // a browsed (non-favourite) parent has seasons too, which the library prefilter would not aggregate
    c.execute("UPDATE entries SET favorite = 0 WHERE _id = ?1", [parent]).unwrap();
    assert!(library::library(&c, Kind::Anime).unwrap().iter().all(|r| r.entry.id != parent));
    let browsed = entries::seasons_of(&c, parent).unwrap();
    assert_eq!(browsed.len(), 3);
    assert_eq!(browsed[0].total, 2);
}
