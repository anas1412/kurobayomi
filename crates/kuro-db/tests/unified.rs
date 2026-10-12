//! The unified database (docs/design/unified-db.md section 6, the kuro-db part): one file, both kinds.

use std::path::Path;

use kuro_db::codec::UpdateStrategy;
use kuro_db::flags::{self, chapter, manga_item_flags_from_unified, manga_item_flags_to_unified};
use kuro_db::schema_unified::{self, APPLICATION_ID, SCHEMA_VERSION, db_path, legacy_files, open_file};
use kuro_db::statements_anime::{self as sa, EpisodeUpdate};
use kuro_db::statements_manga::{
    self as sm, ChapterPatch, ChapterRow, MangaPatch, MangaRow, categories, chapters, history, mangas,
    mangas_categories, views,
};
use kuro_db::{DB_TABLES, Db, DbError};
use kuro_platform::paths::Paths;
use kuro_testkit::{AnimeBuilder, EpisodeBuilder, HistoryBuilder, TempDir, TempPaths};
use rusqlite::Connection;

fn manga(source: i64, url: &str, title: &str) -> MangaRow {
    MangaRow {
        id: 0,
        source,
        url: url.into(),
        artist: None,
        author: None,
        description: None,
        genre: None,
        title: title.into(),
        status: 1,
        thumbnail_url: None,
        favorite: true,
        last_update: None,
        next_update: None,
        initialized: true,
        viewer: 0,
        chapter_flags: 0,
        cover_last_modified: 0,
        date_added: 1_000,
        update_strategy: UpdateStrategy::AlwaysUpdate,
        calculate_interval: 0,
        last_modified_at: 0,
        favorite_modified_at: None,
        version: 0,
        is_syncing: false,
    }
}

fn chapter_row(manga_id: i64, url: &str) -> ChapterRow {
    ChapterRow {
        id: 0,
        manga_id,
        url: url.into(),
        name: format!("Chapter {url}"),
        scanlator: None,
        read: false,
        bookmark: false,
        last_page_read: 0,
        chapter_number: 1.0,
        source_order: 0,
        date_fetch: 2_000,
        date_upload: 1_500,
        last_modified_at: 0,
        version: 0,
        is_syncing: false,
    }
}

fn insert_anime(c: &Connection, source: i64, url: &str, title: &str) -> i64 {
    AnimeBuilder::new().source(source).url(url).title(title).favorite(true).insert(c)
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
    std::fs::create_dir_all(empty.data_dir()).unwrap();
    std::fs::write(empty.data_dir().join("manga.db"), b"").unwrap();
    assert!(legacy_files(&empty.data_dir()).is_empty());
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
    let manga_id = mangas::insert(&c, &manga(0, "X", "Local manga")).unwrap();
    let anime_id = insert_anime(&c, 0, "X", "Local anime");
    assert_ne!(manga_id, anime_id, "entry ids are unique across kinds");
    assert!(mangas::insert(&c, &manga(0, "X", "Again")).is_err(), "UNIQUE (kind, source, url)");
    assert!(sa::insert_anime(&c, &AnimeBuilder::new().source(0).url("X").build()).is_err());

    let kinds: Vec<(String, i64)> = c
        .prepare("SELECT kind, _id FROM libraryView ORDER BY kind")
        .unwrap()
        .query_map([], |r| Ok((r.get(0).unwrap(), r.get(1).unwrap())))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(kinds, [("anime".to_owned(), anime_id), ("manga".to_owned(), manga_id)]);

    assert_eq!(mangas::get_by_url_and_source(&c, "X", 0).unwrap().map(|m| m.id), Some(manga_id));
    assert_eq!(sa::anime_by_url_and_source(&c, "X", 0).unwrap().map(|a| a.id), Some(anime_id));
    assert!(matches!(mangas::get_by_id(&c, anime_id), Err(DbError::NotFound)), "a wrong-kind id is not found");
    assert!(sa::anime_by_id(&c, manga_id).unwrap().is_none());

    sm::sources::upsert(&c, &sm::StubSourceRow { id: 0, lang: "en".into(), name: "Local".into() }).unwrap();
    sa::upsert_stub_source(&c, 0, "ja", "Local anime").unwrap();
    assert_eq!(scalar(&c, "SELECT count(*) FROM stub_sources WHERE id = 0"), 2);
    assert_eq!(sm::sources::find_one(&c, 0).unwrap().unwrap().lang, "en");
    assert_eq!(sa::stub_source(&c, 0).unwrap().unwrap().lang, "ja");

    assert_eq!(mangas::get_source_id_with_favorite_count(&c).unwrap(), [(0, 1)]);
    assert_eq!(sa::source_favorite_counts(&c).unwrap(), [(0, 1)]);
    assert_eq!(mangas::get_favorites(&c).unwrap().len(), 1);
    assert_eq!(sa::favorite_animes(&c).unwrap().len(), 1);
}

#[test]
fn categories_are_per_kind_and_links_cannot_cross() {
    let c = schema_unified::open_in_memory().unwrap();
    let manga_cat = categories::insert(&c, "Reading", 0, 0).unwrap();
    let anime_cat = sa::insert_category(&c, "Watching", 0, 0).unwrap();
    assert_eq!((manga_cat, anime_cat), (1, 1), "ids are per kind");
    let second = categories::insert(&c, "Second", 1, 0).unwrap();
    assert_eq!(second, 2);
    assert_eq!(categories::get_categories(&c).unwrap().len(), 3, "system row plus two");
    assert_eq!(sa::categories(&c).unwrap().len(), 2, "system row plus one");

    let m = mangas::insert(&c, &manga(1, "/m", "M")).unwrap();
    let a = insert_anime(&c, 1, "/a", "A");
    // an anime entry cannot join a manga category: the foreign key on (kind, category_id) refuses it
    assert!(c.execute("INSERT INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'anime', 2)", [a]).is_err());
    // and an anime entry id under kind 'manga' fails on the entry side
    assert!(mangas_categories::insert(&c, a, manga_cat).is_err());
    assert!(sa::set_anime_categories(&c, m, &[anime_cat]).is_err(), "a manga entry is no anime");
    mangas_categories::insert(&c, m, manga_cat).unwrap();
    mangas_categories::insert(&c, m, manga_cat).unwrap();
    assert_eq!(scalar(&c, "SELECT count(*) FROM entry_categories"), 1, "a repeated membership is ignored");
    sa::set_anime_categories(&c, a, &[anime_cat]).unwrap();

    let err = sa::delete_category(&c, 0).unwrap_err();
    assert!(err.to_string().contains("System category can't be deleted"), "{err}");
    assert!(categories::delete(&c, 0).is_err());

    let version = |id: i64| scalar(&c, &format!("SELECT version FROM entries WHERE _id = {id}"));
    let before = version(m);
    assert_eq!(categories::delete(&c, manga_cat).unwrap(), 1);
    assert_eq!(scalar(&c, &format!("SELECT count(*) FROM entry_categories WHERE entry_id = {m}")), 0, "its links go");
    assert_eq!(version(m), before, "deleting a category does not bump the version");
    assert_eq!(sa::categories_by_anime_id(&c, a).unwrap().len(), 1, "the anime category is untouched");
}

#[test]
fn history_search_folds_non_ascii_for_both_kinds() {
    let c = schema_unified::open_in_memory().unwrap();
    let m = mangas::insert(&c, &manga(1, "/m", "Éclair 100%")).unwrap();
    let ch = chapters::insert(&c, &chapter_row(m, "/m/1")).unwrap();
    history::upsert(&c, ch, 500, 1).unwrap();
    let a = insert_anime(&c, 1, "/a", "Привет_мир");
    let ep = EpisodeBuilder::new(a).url("/a/1").insert(&c);
    HistoryBuilder::new(ep).seen_at(600).insert(&c);

    let manga_hits = |q: &str| views::history(&c, q).unwrap().iter().map(|r| r.manga_id).collect::<Vec<_>>();
    let anime_hits = |q: &str| sa::history(&c, q).unwrap().iter().map(|r| r.anime_id).collect::<Vec<_>>();
    for q in ["éclair", "ÉCLAIR", "Éclair"] {
        assert_eq!(manga_hits(q), [m], "{q}");
    }
    for q in ["привет", "ПРИВЕТ"] {
        assert_eq!(anime_hits(q), [a], "{q}");
    }
    assert_eq!(manga_hits("100%"), [m], "% is literal");
    assert!(manga_hits("a%").is_empty());
    assert_eq!(anime_hits("т_м"), [a], "_ is literal");
    assert!(anime_hits("т%м").is_empty());
    assert!(manga_hits("привет").is_empty(), "an anime title is not a manga hit");
    assert!(anime_hits("éclair").is_empty(), "a manga title is not an anime hit");
    assert_eq!(manga_hits("").len(), 1);
    assert_eq!(anime_hits("").len(), 1);
}

#[test]
fn chapter_and_episode_progress_persist_across_reopen() {
    let dir = TempDir::new().unwrap();
    let path = db_path(dir.path());
    let (m, ch, a, ep) = {
        let c = open_file(&path).unwrap();
        let m = mangas::insert(&c, &manga(1, "/m", "M")).unwrap();
        let ch = chapters::insert(&c, &chapter_row(m, "/m/1")).unwrap();
        let a = insert_anime(&c, 1, "/a", "A");
        let ep = EpisodeBuilder::new(a).url("/a/1").with(|e| e.version = 0).insert(&c);
        chapters::update(&c, &ChapterPatch { id: ch, read: Some(true), last_page_read: Some(7), ..Default::default() }).unwrap();
        sa::update_episode(&c, &EpisodeUpdate { id: ep, seen: Some(true), last_second_seen: Some(30), ..Default::default() }).unwrap();
        history::upsert(&c, ch, 500, 40).unwrap();
        HistoryBuilder::new(ep).seen_at(600).insert(&c);
        (m, ch, a, ep)
    };
    assert_ne!(ch, ep, "item ids are unique across kinds");

    let c = open_file(&path).unwrap();
    let chapter = chapters::get_by_id(&c, ch).unwrap();
    assert_eq!((chapter.read, chapter.last_page_read, chapter.version), (true, 7, 1));
    let episode = sa::episode_by_id(&c, ep).unwrap().unwrap();
    assert_eq!((episode.seen, episode.last_second_seen, episode.version), (true, 30, 1));
    assert_eq!(scalar(&c, &format!("SELECT version FROM entries WHERE _id = {m}")), 1, "the manga version followed its chapter");
    assert_eq!(scalar(&c, &format!("SELECT version FROM entries WHERE _id = {a}")), 1, "the anime version followed its episode");
    let h = history::get_by_manga_id(&c, m).unwrap();
    assert_eq!((h[0].last_read, h[0].time_read), (500, 40), "the chapter keeps its duration");
    assert_eq!(scalar(&c, &format!("SELECT duration_ms FROM history WHERE item_id = {ep}")), 0, "an episode writes none");

    let lib = views::library(&c).unwrap();
    assert_eq!((lib[0].total_count, lib[0].read_count, lib[0].last_read), (1, 1, 500));
    let alib = sa::library(&c).unwrap();
    assert_eq!((alib[0].total_count, alib[0].seen_count, alib[0].last_seen), (1, 1, 600));
}

#[test]
fn the_manga_chapter_flags_round_trip_through_the_unified_layout() {
    let c = schema_unified::open_in_memory().unwrap();
    let stored = |id: i64| scalar(&c, &format!("SELECT item_flags FROM entries WHERE _id = {id}"));
    for (i, sorting) in [chapter::SORTING_SOURCE, chapter::SORTING_NUMBER, chapter::SORTING_UPLOAD_DATE, chapter::SORTING_ALPHABET]
        .into_iter()
        .enumerate()
    {
        let value = sorting | chapter::SORT_ASC | chapter::SHOW_UNREAD | chapter::SHOW_BOOKMARKED | chapter::DISPLAY_NUMBER;
        let id = mangas::insert(&c, &MangaRow { chapter_flags: value, ..manga(1, &format!("/m{i}"), "M") }).unwrap();
        assert_eq!(mangas::get_by_id(&c, id).unwrap().chapter_flags, value, "the manga layout reads back");
        assert_eq!(stored(id), manga_item_flags_to_unified(value), "the unified layout is stored");
        assert_eq!(stored(id) & 0x600, sorting << 1, "sorting sits at the anime bits");
        // a patch goes through the same translation
        let patched = chapter::SORTING_ALPHABET | chapter::SORT_DESC;
        mangas::update(&c, &MangaPatch { id, chapter_flags: Some(patched), ..Default::default() }).unwrap();
        assert_eq!(mangas::get_by_id(&c, id).unwrap().chapter_flags, patched);
        assert_eq!(stored(id), 0x600);
    }

    // the masked writer of design 3.4, translating both the flag and the mask
    let id = mangas::insert(&c, &MangaRow { chapter_flags: chapter::SORTING_NUMBER | chapter::SHOW_READ, ..manga(1, "/masked", "M") }).unwrap();
    let (flag, mask) = (chapter::SORTING_UPLOAD_DATE, chapter::SORTING_MASK);
    c.execute(
        "UPDATE entries SET item_flags = (item_flags & ~((?2 & ~768) | ((?2 & 768) << 1))) | (((?1 & ~768) | ((?1 & 768) << 1)) & ((?2 & ~768) | ((?2 & 768) << 1))) WHERE _id = ?3 AND kind = 'manga'",
        rusqlite::params![flag, mask, id],
    )
    .unwrap();
    assert_eq!(mangas::get_by_id(&c, id).unwrap().chapter_flags, flags::set_flag(chapter::SORTING_NUMBER | chapter::SHOW_READ, flag, mask));

    // anime flags are not translated
    let a = AnimeBuilder::new().url("/a").with(|r| r.episode_flags = 0x600 | 0x80 | 0x1000).insert(&c);
    assert_eq!(sa::anime_by_id(&c, a).unwrap().unwrap().episode_flags, 0x600 | 0x80 | 0x1000);
    assert_eq!(stored(a), 0x600 | 0x80 | 0x1000);

    // the SQL expressions and the Rust functions agree over every manga-legal value
    let legal = [0x1_i64, 0x2, 0x4, 0x8, 0x10, 0x20, 0x40, 0x100, 0x200, 0x10_0000];
    for mask in 0..(1_i64 << legal.len()) {
        let x = legal.iter().enumerate().filter(|(i, _)| mask & (1 << i) != 0).fold(0, |acc, (_, bit)| acc | bit);
        let (up, down): (i64, i64) = c
            .query_row(
                "SELECT ((?1 & ~768) | ((?1 & 768) << 1)), ((?2 & ~1536) | ((?2 & 1536) >> 1))",
                rusqlite::params![x, manga_item_flags_to_unified(x)],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(up, manga_item_flags_to_unified(x), "{x:#x}");
        assert_eq!(down, x, "{x:#x}");
        assert_eq!(manga_item_flags_from_unified(up), x);
    }
}

#[test]
fn seasons_of_matches_the_library_rollup_and_also_works_for_a_browsed_parent() {
    let c = schema_unified::open_in_memory().unwrap();
    let parent = AnimeBuilder::new().url("/p").favorite(true).with(|a| a.fetch_type = kuro_db::schema_anime::FetchType::Seasons).insert(&c);
    let mut children = Vec::new();
    for (url, seen) in [("/s1", [true, true]), ("/s2", [true, false]), ("/s3", [false, false])] {
        let id = AnimeBuilder::new().url(url).with(|a| a.parent_id = Some(parent)).insert(&c);
        for (i, done) in seen.into_iter().enumerate() {
            let ep = EpisodeBuilder::new(id).url(&format!("{url}/{i}")).seen(done).with(|e| e.date_upload = 10 + i as i64).insert(&c);
            HistoryBuilder::new(ep).seen_at(100 + i as i64).insert(&c);
        }
        children.push(id);
    }
    let from_library = sa::library(&c).unwrap();
    let row = from_library.iter().find(|r| r.anime.id == parent).expect("the favourite parent is in the library");
    let seasons = sa::seasons_of(&c, parent).unwrap();
    assert_eq!(seasons.iter().map(|s| s.anime.id).collect::<Vec<_>>(), children);
    let fully_done = seasons.iter().filter(|s| s.total_count == s.seen_count).count() as i64;
    let max = |f: fn(&sa::SeasonAnimeRow) -> i64| seasons.iter().map(f).max().unwrap();
    assert_eq!(
        (row.total_count, row.seen_count, row.latest_upload, row.episode_fetched_at, row.last_seen),
        (seasons.len() as i64, fully_done, max(|s| s.latest_upload), max(|s| s.episode_fetched_at), max(|s| s.last_seen))
    );

    // a browsed (non-favourite) parent has seasons too, which the library prefilter would not aggregate
    c.execute("UPDATE entries SET favorite = 0 WHERE _id = ?1", [parent]).unwrap();
    assert!(sa::library(&c).unwrap().iter().all(|r| r.anime.id != parent));
    let browsed = sa::seasons_of(&c, parent).unwrap();
    assert_eq!(browsed.len(), 3);
    assert_eq!(browsed[0].total_count, 2);
}
