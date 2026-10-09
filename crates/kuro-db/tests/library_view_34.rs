//! T-03a-024: migration 34 keeps the library view rows identical to the baseline view (TC-L08, `EXCEPT` both
//! ways returns 0 rows), and the benchmark timings hold. The benchmark is `#[ignore]`d because it measures time;
//! run it with `cargo test -p kuro-db --test library_view_34 -- --ignored`.

use std::time::Instant;

use kuro_db::schema::{BASELINE_SQL, MANGA_MIGRATIONS};
use rusqlite::Connection;

fn baseline() -> Connection {
    let c = Connection::open_in_memory().unwrap();
    c.pragma_update(None, "foreign_keys", "ON").unwrap();
    c.execute_batch(BASELINE_SQL).unwrap();
    c
}

/// Keeps the baseline view's rows readable as `libraryViewV33`, then applies migration 34.
fn migrate_keeping_old_view(c: &Connection) {
    let old_sql: String = c.query_row("SELECT sql FROM sqlite_master WHERE name = 'libraryView'", [], |r| r.get(0)).unwrap();
    c.execute_batch(&old_sql.replacen("libraryView", "libraryViewV33", 1)).unwrap();
    c.execute_batch(MANGA_MIGRATIONS[0].1).unwrap();
}

/// Rows in one view but not the other, both ways.
fn except_both_ways(c: &Connection) -> (i64, i64) {
    let count = |sql: &str| c.query_row(&format!("SELECT count(*) FROM ({sql})"), [], |r| r.get::<_, i64>(0)).unwrap();
    (
        count("SELECT * FROM libraryViewV33 EXCEPT SELECT * FROM libraryView"),
        count("SELECT * FROM libraryView EXCEPT SELECT * FROM libraryViewV33"),
    )
}

fn add_manga(c: &Connection, id: i64, favorite: bool) {
    c.execute(
        "INSERT INTO mangas(_id, source, url, title, status, favorite, initialized, viewer, chapter_flags, cover_last_modified, date_added)
         VALUES (?1, 1, ?2, ?3, 0, ?4, 1, 0, 0, 0, 0)",
        rusqlite::params![id, format!("/m/{id}"), format!("Manga {id}"), favorite],
    )
    .unwrap();
}

fn add_chapter(c: &Connection, id: i64, manga: i64, scanlator: Option<&str>, read: bool, bookmark: bool) {
    c.execute(
        "INSERT INTO chapters(_id, manga_id, url, name, scanlator, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 0, ?1, ?1, ?8, ?9)",
        rusqlite::params![id, manga, format!("/m/{manga}/{id}"), format!("Ch {id}"), scanlator, read, bookmark, 1000 + id, 500 + id],
    )
    .unwrap();
}

/// The TC-L01 to TC-L07 shapes in one database: counters with history, two categories, no links, NULL and empty
/// scanlators with exclusions, everything excluded, and a non-favourite.
#[test]
fn tc_l08_old_and_new_library_view_rows_are_identical_on_the_tc_l_fixtures() {
    let c = baseline();
    c.execute("INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'A', 0, 0), (2, 'B', 1, 0)", []).unwrap();
    // TC-L01: three chapters, two read, one bookmarked, history on one
    add_manga(&c, 1, true);
    add_chapter(&c, 11, 1, None, true, true);
    add_chapter(&c, 12, 1, None, true, false);
    add_chapter(&c, 13, 1, None, false, false);
    c.execute("INSERT INTO history(chapter_id, last_read, time_read) VALUES (11, 5000, 10)", []).unwrap();
    // TC-L02: two categories, TC-L03: manga with no links
    add_manga(&c, 2, true);
    c.execute("INSERT INTO mangas_categories(manga_id, category_id) VALUES (2, 1), (2, 2)", []).unwrap();
    add_manga(&c, 3, true);
    // TC-L04 and TC-L05: NULL and empty scanlators with exclusions present
    add_manga(&c, 4, true);
    add_chapter(&c, 41, 4, None, false, false);
    add_chapter(&c, 42, 4, Some(""), false, false);
    add_chapter(&c, 43, 4, Some("Group"), false, false);
    c.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (4, ''), (4, 'Group')", []).unwrap();
    // TC-L06: everything excluded
    add_manga(&c, 5, true);
    add_chapter(&c, 51, 5, Some("Group"), true, false);
    c.execute("INSERT INTO excluded_scanlators(manga_id, scanlator) VALUES (5, 'Group')", []).unwrap();
    // TC-L07: a non-favourite with chapters
    add_manga(&c, 6, false);
    add_chapter(&c, 61, 6, None, false, false);
    c.execute("UPDATE mangas SET favorite = 0 WHERE _id = 6", []).unwrap();

    migrate_keeping_old_view(&c);
    let rows: i64 = c.query_row("SELECT count(*) FROM libraryView", [], |r| r.get(0)).unwrap();
    assert_eq!(rows, 6, "manga 2 has two category rows; the non-favourite is absent");
    assert_eq!(except_both_ways(&c), (0, 0));
}

/// 5000 manga with 500 favourites and 20 chapters each (100 000 chapters).
fn benchmark_db() -> Connection {
    let c = baseline();
    let tx = c.unchecked_transaction().unwrap();
    {
        let mut manga = tx
            .prepare(
                "INSERT INTO mangas(_id, source, url, title, status, favorite, initialized, viewer, chapter_flags, cover_last_modified, date_added)
                 VALUES (?1, 1, ?2, ?3, 0, ?4, 1, 0, 0, 0, 0)",
            )
            .unwrap();
        let mut chapter = tx
            .prepare(
                "INSERT INTO chapters(_id, manga_id, url, name, read, bookmark, last_page_read, chapter_number, source_order, date_fetch, date_upload)
                 VALUES (?1, ?2, ?3, ?4, ?5, 0, 0, ?6, ?6, ?7, ?7)",
            )
            .unwrap();
        for id in 1..=5000_i64 {
            manga.execute(rusqlite::params![id, format!("/m/{id}"), format!("Manga {id}"), id % 10 == 0]).unwrap();
            for n in 1..=20_i64 {
                let cid = (id - 1) * 20 + n;
                chapter.execute(rusqlite::params![cid, id, format!("/m/{id}/{n}"), format!("Ch {n}"), n % 2, n, 1000 + n]).unwrap();
            }
        }
    }
    tx.commit().unwrap();
    c
}

#[test]
#[ignore = "timing benchmark: run with --ignored"]
fn benchmark_url_lookup_and_library_query_stay_within_the_budget() {
    let c = benchmark_db();
    migrate_keeping_old_view(&c);
    assert_eq!(except_both_ways(&c), (0, 0), "the benchmark rows are identical too");

    let mut lookup = c.prepare("SELECT _id FROM chapters WHERE url = ?1").unwrap();
    let plan: String = c
        .query_row("EXPLAIN QUERY PLAN SELECT _id FROM chapters WHERE url = '/m/7/3'", [], |r| r.get(3))
        .unwrap();
    assert!(plan.contains("chapters_url_index"), "{plan}");
    let started = Instant::now();
    for i in 0..1000_i64 {
        let url = format!("/m/{}/{}", i % 5000 + 1, i % 20 + 1);
        let found: Option<i64> = lookup.query_row([url], |r| r.get(0)).ok();
        assert!(found.is_some());
    }
    let per_lookup_ms = started.elapsed().as_secs_f64() * 1000.0 / 1000.0;
    assert!(per_lookup_ms < 1.0, "url lookup took {per_lookup_ms} ms");

    let started = Instant::now();
    let rows: i64 = c.query_row("SELECT count(*) FROM (SELECT * FROM libraryView)", [], |r| r.get(0)).unwrap();
    let library_ms = started.elapsed().as_secs_f64() * 1000.0;
    assert_eq!(rows, 500);
    assert!(library_ms < 100.0, "library query took {library_ms} ms");
}
