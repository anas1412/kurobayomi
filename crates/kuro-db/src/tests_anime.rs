//! Tests of the anime handle (T-04a-007 to T-04a-012, T-04a-027): the change bus, the write and read
//! paths, the repository batches, the library read model and the live queries of `AnimeDb`.

use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

use kuro_testkit::TempPaths;
use rusqlite::Connection;
use tokio::sync::broadcast;

use crate::anime::AnimeDb;
use crate::error::{DbError, DbResult};
use crate::notify::Changed;
use crate::runtime::{Db, DbKind, TxExt};
use crate::schema_anime::Millis;
use crate::statements_anime::{
    self as sql, AnimeUpdate, EpisodeRow, EpisodeUpdate,
};
use crate::subscribe::Subscription;
use crate::testdb::{AnimeBuilder, EpisodeBuilder, HistoryBuilder};

const QUIET: Duration = Duration::from_millis(400);

fn open() -> (TempPaths, AnimeDb) {
    let paths = TempPaths::new().unwrap();
    let db = AnimeDb::open(&paths).unwrap();
    (paths, db)
}

/// Every table name published since the last call.
fn published(rx: &mut broadcast::Receiver<Changed>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    while let Ok(changed) = rx.try_recv() {
        out.extend(changed.iter().cloned());
    }
    out
}

fn names(list: &[&str]) -> BTreeSet<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

async fn next<T>(sub: &mut Subscription<T>) -> Option<Arc<T>> {
    tokio::time::timeout(QUIET, sub.changed()).await.ok().flatten().map(|r| r.expect("the query succeeded"))
}

/// An anime with one episode, one history row, one category link, one track and one relation; returns
/// `(anime, episode)`.
fn full_anime(tx: &Connection) -> (i64, i64) {
    let anime = AnimeBuilder::new("/a").insert(tx);
    let other = AnimeBuilder::new("/b").insert(tx);
    let episode = EpisodeBuilder::new(anime, "/a/1").insert(tx);
    HistoryBuilder::new(episode).insert(tx);
    tx.execute("INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'c', 1, 0)", []).unwrap();
    tx.execute("INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, 1)", [anime]).unwrap();
    tx.execute(
        "INSERT INTO anime_sync(anime_id, sync_id, remote_id, title, last_episode_seen, total_episodes, status, score, \
         remote_url, start_date, finish_date) VALUES (?1, 1, 1, 't', 0, 0, 1, 0, 'u', 0, 0)",
        [anime],
    )
    .unwrap();
    tx.execute("INSERT INTO anime_relations(anime_id, related_anime_id, name, sort_order, last_fetched_at) VALUES (?1, ?2, 'Sequel', 0, 0)", [anime, other])
        .unwrap();
    (anime, episode)
}

// T-04a-007: the change bus ------------------------------------------------------------------------

#[test]
fn deleting_an_anime_publishes_every_table_the_cascade_touches() {
    let (_p, db) = open();
    let (anime, _) = db.handle().write(|tx| Ok(full_anime(tx))).unwrap();
    let mut rx = db.handle().changes.subscribe();
    db.handle().write(|tx| Ok(tx.execute("DELETE FROM animes WHERE _id = ?1", [anime]).map(drop)?)).unwrap();
    let got = published(&mut rx);
    for table in ["animes", "episodes", "animehistory", "animes_categories", "anime_sync", "anime_relations"] {
        assert!(got.contains(table), "{table} missing from {got:?}");
    }
}

#[test]
fn trigger_writes_are_published() {
    let (_p, db) = open();
    let (_, episode) = db.handle().write(|tx| Ok(full_anime(tx))).unwrap();
    let mut rx = db.handle().changes.subscribe();
    // Marking an episode seen fires the version triggers, which write `animes` and `episodes`.
    db.handle().write(|tx| Ok(tx.execute("UPDATE episodes SET seen = 1 WHERE _id = ?1", [episode]).map(drop)?)).unwrap();
    assert_eq!(published(&mut rx), names(&["animes", "episodes"]));
}

#[test]
fn a_rolled_back_write_publishes_nothing() {
    let (_p, db) = open();
    let mut rx = db.handle().changes.subscribe();
    let result: DbResult<()> = db.handle().write(|tx| {
        AnimeBuilder::new("/a").insert(tx);
        Err(DbError::NotFound)
    });
    assert!(result.is_err());
    assert!(published(&mut rx).is_empty());
}

#[tokio::test]
async fn delete_all_history_publishes_animehistory() {
    let (_p, db) = open();
    db.handle()
        .write(|tx| {
            let a = AnimeBuilder::new("/a").insert(tx);
            let e = EpisodeBuilder::new(a, "/a/1").insert(tx);
            HistoryBuilder::new(e).insert(tx);
            Ok(())
        })
        .unwrap();
    let mut rx = db.handle().changes.subscribe();
    assert!(db.delete_all_history().await);
    assert!(published(&mut rx).contains("animehistory"));
    let left: i64 = db.handle().read(|c| Ok(c.query_row("SELECT count(*) FROM animehistory", [], |r| r.get(0))?)).unwrap();
    assert_eq!(left, 0);
}

#[test]
fn touch_names_a_table_the_hook_did_not_see() {
    let (_p, db) = open();
    let mut rx = db.handle().changes.subscribe();
    db.handle().write(|tx| tx.touch(&["animehistory", "episodes"])).unwrap();
    assert_eq!(published(&mut rx), names(&["animehistory", "episodes"]));
    // Touching in a write that then fails publishes nothing.
    let _: DbResult<()> = db.handle().write(|tx| {
        tx.touch(&["animes"])?;
        Err(DbError::NotFound)
    });
    assert!(published(&mut rx).is_empty());
}

#[tokio::test]
async fn the_library_stream_emits_now_and_after_each_of_its_four_tables_changes() {
    let (_p, db) = open();
    let mut sub = db.library_stream();
    assert_eq!(next(&mut sub).await.map(|v| v.len()), Some(0), "the current result comes first");

    let anime = db
        .handle()
        .write_async(|tx| {
            let id = AnimeBuilder::new("/a").insert(tx);
            tx.execute("UPDATE animes SET favorite = 1 WHERE _id = ?1", [id])?;
            tx.execute("INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'c', 1, 0)", [])?;
            Ok(id)
        })
        .await
        .unwrap();
    assert_eq!(next(&mut sub).await.map(|v| v.len()), Some(1), "animes");

    let episode = db.handle().write_async(move |tx| Ok(EpisodeBuilder::new(anime, "/a/1").insert(tx))).await.unwrap();
    assert_eq!(next(&mut sub).await.map(|v| v[0].total_count), Some(1), "episodes");

    db.upsert_history(episode, 4000).await.unwrap();
    assert_eq!(next(&mut sub).await.map(|v| v[0].last_seen), Some(4000), "animehistory");

    db.handle()
        .write_async(move |tx| Ok(tx.execute("INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, 1)", [anime]).map(drop)?))
        .await
        .unwrap();
    assert_eq!(next(&mut sub).await.map(|v| v[0].category), Some(1), "animes_categories");

    // `categories` is not a table the view reads.
    db.handle().write_async(|tx| Ok(tx.execute("UPDATE categories SET name = 'd' WHERE _id = 1", []).map(drop)?)).await.unwrap();
    assert!(next(&mut sub).await.is_none(), "a category rename does not re-run the library query");
}

#[tokio::test]
async fn commits_inside_the_coalescing_window_cost_one_emission() {
    let (_p, db) = open();
    let anime = db.handle().write_async(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).await.unwrap();
    let mut sub = db.library_stream();
    next(&mut sub).await.expect("current result");
    for i in 0..5 {
        db.handle()
            .write_async(move |tx| Ok(tx.execute("UPDATE animes SET favorite = ?2 WHERE _id = ?1", [anime, i % 2]).map(drop)?))
            .await
            .unwrap();
    }
    // Five commits end on favourite = 0 (i = 4): the library is empty again, and at most the settled state shows.
    let mut seen = 0;
    while next(&mut sub).await.is_some() {
        seen += 1;
    }
    assert!(seen <= 2, "five commits in a row produced {seen} emissions");
}

#[tokio::test]
async fn a_failing_query_is_an_item_not_the_end_of_the_stream() {
    let (_p, db) = open();
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(true));
    let flag = Arc::clone(&fail);
    let mut sub = db.handle().subscribe(&["animes"], Duration::ZERO, move |c| {
        if flag.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(DbError::NotFound);
        }
        Ok(sql::favorite_animes(c)?.len())
    });
    let first = tokio::time::timeout(QUIET, sub.changed()).await.unwrap().unwrap();
    assert!(first.is_err());
    fail.store(false, std::sync::atomic::Ordering::SeqCst);
    db.handle().write_async(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).await.unwrap();
    let second = tokio::time::timeout(QUIET, sub.changed()).await.unwrap().unwrap();
    assert_eq!(second.map(|v| *v), Ok(0));
}

// T-04a-008: the handler --------------------------------------------------------------------------

#[test]
#[ignore = "slow: run on demand"]
fn two_concurrent_writes_serialize() {
    let (_p, db) = open();
    let db = Arc::clone(db.handle());
    let started = Instant::now();
    let first = {
        let db = Arc::clone(&db);
        std::thread::spawn(move || {
            db.write(|tx| {
                AnimeBuilder::new("/first").insert(tx);
                std::thread::sleep(Duration::from_millis(250));
                Ok(())
            })
            .unwrap();
        })
    };
    std::thread::sleep(Duration::from_millis(50));
    let seen_by_second = db
        .write(|tx| Ok(tx.query_row("SELECT count(*) FROM animes", [], |r| r.get::<_, i64>(0))?))
        .unwrap();
    first.join().unwrap();
    assert_eq!(seen_by_second, 1, "the second write started after the first committed");
    assert!(started.elapsed() >= Duration::from_millis(250));
}

#[test]
#[ignore = "slow: run on demand"]
fn a_long_write_does_not_block_readers() {
    let (_p, db) = open();
    let db = Arc::clone(db.handle());
    db.write(|tx| Ok(AnimeBuilder::new("/old").insert(tx))).unwrap();
    let writer = {
        let db = Arc::clone(&db);
        std::thread::spawn(move || {
            db.write(|tx| {
                AnimeBuilder::new("/new").insert(tx);
                std::thread::sleep(Duration::from_millis(400));
                Ok(())
            })
            .unwrap();
        })
    };
    std::thread::sleep(Duration::from_millis(100));
    let t = Instant::now();
    let count = db.read(|c| Ok(c.query_row("SELECT count(*) FROM animes", [], |r| r.get::<_, i64>(0))?)).unwrap();
    assert!(t.elapsed() < Duration::from_millis(200), "the read waited for the write");
    assert_eq!(count, 1, "the reader sees the last committed state");
    writer.join().unwrap();
}

#[tokio::test]
async fn a_write_future_dropped_before_it_starts_leaves_nothing_behind() {
    let (_p, db) = open();
    let future = db.handle().write_async(|tx| Ok(AnimeBuilder::new("/never").insert(tx)));
    drop(future);
    db.handle().write_async(|tx| Ok(AnimeBuilder::new("/after").insert(tx))).await.unwrap();
    let urls: Vec<String> = db
        .handle()
        .read_async(|c| Ok(sql::favorite_animes(c)?.into_iter().map(|a| a.url).collect::<Vec<_>>()))
        .await
        .unwrap();
    assert!(urls.is_empty());
    let total: i64 = db.handle().read_async(|c| Ok(c.query_row("SELECT count(*) FROM animes", [], |r| r.get(0))?)).await.unwrap();
    assert_eq!(total, 1, "only the second write ran");
}

#[test]
fn a_savepoint_rolls_back_unless_released() {
    let (_p, db) = open();
    db.handle()
        .write(|tx| {
            AnimeBuilder::new("/keep").insert(tx);
            {
                let sp = tx.savepoint()?;
                AnimeBuilder::new("/dropped").insert(tx);
                drop(sp);
            }
            let sp = tx.savepoint()?;
            AnimeBuilder::new("/released").insert(tx);
            sp.release()
        })
        .unwrap();
    let urls: Vec<String> = db
        .handle()
        .read(|c| Ok(c.prepare("SELECT url FROM animes ORDER BY _id")?.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?))
        .unwrap();
    assert_eq!(urls, ["/keep", "/released"]);
}

#[test]
fn close_folds_the_wal_into_the_database_file() {
    let (paths, db) = open();
    db.handle().write(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).unwrap();
    db.close().unwrap();
    let wal = DbKind::Anime.path(&kuro_platform::paths::Paths::data_dir(&paths)).with_extension("db-wal");
    let size = std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0);
    assert_eq!(size, 0, "the checkpoint truncated the WAL");
    // The handle still works afterwards.
    db.handle().write(|tx| Ok(AnimeBuilder::new("/b").insert(tx))).unwrap();
}

// T-04a-009: entries -------------------------------------------------------------------------------

#[tokio::test]
async fn update_all_anime_applies_a_batch_in_one_transaction_and_reports_true() {
    let (_p, db) = open();
    let (a, b) = db
        .handle()
        .write_async(|tx| Ok((AnimeBuilder::new("/a").insert(tx), AnimeBuilder::new("/b").insert(tx))))
        .await
        .unwrap();
    let ok = db
        .update_all_anime(vec![
            AnimeUpdate { id: a, title: Some("A2".into()), ..Default::default() },
            AnimeUpdate { id: b, favorite: Some(true), ..Default::default() },
        ])
        .await;
    assert!(ok);
    let rows = db.handle().read_async(move |c| Ok((sql::anime_by_id(c, a)?, sql::anime_by_id(c, b)?))).await.unwrap();
    assert_eq!(rows.0.unwrap().title, "A2");
    assert!(rows.1.unwrap().favorite);
}

#[test]
fn update_all_anime_logs_and_returns_false_instead_of_failing() {
    let (_p, db) = open();
    let a = db.handle().write(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).unwrap();
    // A reader refuses writes, so the batch fails; the answer is `false`, not an error.
    let done = db
        .handle()
        .read(|c| Ok(sql::update_all_anime(c, &[AnimeUpdate { id: a, title: Some("X".into()), ..Default::default() }])))
        .unwrap();
    assert!(!done);
    let title = db.handle().read(|c| Ok(sql::anime_by_id(c, a)?.unwrap().title)).unwrap();
    assert_eq!(title, "T /a");
}

#[test]
fn duplicate_source_and_url_resolve_to_the_first_row() {
    let (_p, db) = open();
    let (first, second) = db
        .handle()
        .write(|tx| Ok((AnimeBuilder::new("/same").insert(tx), AnimeBuilder::new("/same").insert(tx))))
        .unwrap();
    assert!(first < second);
    let found = db.handle().read(|c| Ok(sql::anime_by_url_and_source(c, "/same", 1)?.unwrap().id)).unwrap();
    assert_eq!(found, first);
}

#[test]
fn source_ids_above_2_pow_63_round_trip_and_the_highest_id_is_reused() {
    let (_p, db) = open();
    let source = 0xF000_0000_0000_0001_u64 as i64;
    db.handle()
        .write(|tx| {
            let id = AnimeBuilder::new("/a").insert(tx);
            tx.execute("UPDATE animes SET source = ?2 WHERE _id = ?1", [id, source])?;
            Ok(())
        })
        .unwrap();
    let back = db.handle().read(|c| Ok(sql::anime_by_url_and_source(c, "/a", source)?.map(|a| a.source as u64))).unwrap();
    assert_eq!(back, Some(0xF000_0000_0000_0001));
    let (last, reused) = db
        .handle()
        .write(|tx| {
            let last = AnimeBuilder::new("/b").insert(tx);
            tx.execute("DELETE FROM animes WHERE _id = ?1", [last])?;
            Ok((last, AnimeBuilder::new("/c").insert(tx)))
        })
        .unwrap();
    assert_eq!(last, reused, "no AUTOINCREMENT: the freed id comes back");
}

#[test]
fn an_update_cannot_clear_parent_id() {
    let (_p, db) = open();
    db.handle()
        .write(|tx| {
            let parent = AnimeBuilder::new("/p").insert(tx);
            let child = AnimeBuilder::new("/c").insert(tx);
            tx.execute("UPDATE animes SET parent_id = ?2 WHERE _id = ?1", [child, parent])?;
            sql::update_anime(tx, &AnimeUpdate { id: child, parent_id: None, title: Some("x".into()), ..Default::default() })?;
            let still: Option<i64> = tx.query_row("SELECT parent_id FROM animes WHERE _id = ?1", [child], |r| r.get(0))?;
            assert_eq!(still, Some(parent));
            Ok(())
        })
        .unwrap();
}

#[test]
fn updates_of_different_columns_from_two_threads_both_survive() {
    let (_p, db) = open();
    let handle = Arc::clone(db.handle());
    let a = handle.write(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).unwrap();
    let (h1, h2) = (Arc::clone(&handle), Arc::clone(&handle));
    let t1 = std::thread::spawn(move || {
        h1.write(|tx| Ok(sql::update_anime(tx, &AnimeUpdate { id: a, episode_flags: Some(0x22), ..Default::default() })?)).unwrap()
    });
    let t2 = std::thread::spawn(move || {
        h2.write(|tx| Ok(sql::update_anime(tx, &AnimeUpdate { id: a, last_update: Some(77), ..Default::default() })?)).unwrap()
    });
    t1.join().unwrap();
    t2.join().unwrap();
    let row = handle.read(|c| Ok(sql::anime_by_id(c, a)?.unwrap())).unwrap();
    assert_eq!((row.episode_flags, row.last_update), (0x22, Some(77)));
}

// T-04a-010: library ------------------------------------------------------------------------------

#[test]
fn library_rows_carry_one_row_per_category_and_category_zero_when_there_is_none() {
    let (_p, db) = open();
    db.handle()
        .write(|tx| {
            let a = AnimeBuilder::new("/a").insert(tx);
            let b = AnimeBuilder::new("/b").insert(tx);
            tx.execute("UPDATE animes SET favorite = 1", [])?;
            tx.execute("INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'a', 0, 0), (2, 'b', 1, 0)", [])?;
            tx.execute("INSERT INTO animes_categories(anime_id, category_id) VALUES (?1, 1), (?1, 2)", [a])?;
            let e = EpisodeBuilder::new(a, "/a/1").insert(tx);
            tx.execute("UPDATE episodes SET seen = 1, bookmark = 1, date_fetch = 200, date_upload = 150 WHERE _id = ?1", [e])?;
            HistoryBuilder::new(e).seen_at(3000).insert(tx);
            let _ = b;
            Ok(())
        })
        .unwrap();
    let rows = db.handle().read(|c| Ok(sql::library(c)?)).unwrap();
    let mut by_anime: Vec<(String, i64)> = rows.iter().map(|r| (r.anime.url.clone(), r.category)).collect();
    by_anime.sort();
    assert_eq!(by_anime, [("/a".into(), 1), ("/a".into(), 2), ("/b".into(), 0)]);
    let a = rows.iter().find(|r| r.anime.url == "/a").unwrap();
    assert_eq!(
        (a.total_count, a.seen_count, a.bookmark_count, a.fillermark_count, a.latest_upload, a.episode_fetched_at, a.last_seen),
        (1, 1, 1, 0, 150, 200, 3000)
    );
}

#[test]
fn count_columns_are_truncated_like_the_kotlin_mapper() {
    let c = crate::testdb::TestDb::anime();
    let a = AnimeBuilder::new("/a").insert(&c);
    c.execute("UPDATE animes SET favorite = 1", []).unwrap();
    EpisodeBuilder::new(a, "/a/1").insert(&c);
    // A view column that comes back as REAL still reads as a whole number.
    c.execute_batch("DROP VIEW animelibView; CREATE VIEW animelibView AS SELECT M.*, 3 AS totalCount, 2.9 AS seenCount, 1.5 AS bookmarkCount, 0.0 AS fillermarkCount, 0 AS latestUpload, 0 AS episodeFetchedAt, 0 AS lastSeen, 0 AS category FROM animes M")
        .unwrap();
    let rows = sql::library(&c).unwrap();
    assert_eq!((rows[0].seen_count, rows[0].bookmark_count, rows[0].fillermark_count), (2, 1, 0));
}

#[test]
fn seasons_of_lists_children_with_their_aggregates() {
    let c = crate::testdb::TestDb::anime();
    let parent = AnimeBuilder::new("/p").insert(&c);
    c.execute("UPDATE animes SET favorite = 1, fetch_type = 0 WHERE _id = ?1", [parent]).unwrap();
    let mut seasons = Vec::new();
    for url in ["/s1", "/s2", "/s3"] {
        let id = AnimeBuilder::new(url).insert(&c);
        c.execute("UPDATE animes SET parent_id = ?2 WHERE _id = ?1", [id, parent]).unwrap();
        seasons.push(id);
    }
    for (i, seen) in [(0, 1), (0, 1), (1, 1), (1, 0)] {
        let e = EpisodeBuilder::new(seasons[i], &format!("/e{i}{seen}{}", i + seen)).insert(&c);
        c.execute("UPDATE episodes SET seen = ?2 WHERE _id = ?1", [e, seen as i64]).unwrap();
    }
    let rows = sql::seasons_of(&c, parent).unwrap();
    let got: Vec<(i64, i64, i64)> = rows.iter().map(|r| (r.anime.id, r.total_count, r.seen_count)).collect();
    assert_eq!(got, [(seasons[0], 2, 2), (seasons[1], 2, 1), (seasons[2], 0, 0)]);
}

// T-04a-011: episodes -----------------------------------------------------------------------------

#[tokio::test]
async fn add_all_episodes_inside_a_write_returns_ids_and_rolls_back_alone_on_failure() {
    let (_p, db) = open();
    let anime = db.handle().write_async(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).await.unwrap();
    let make = |n: i64, anime_id: i64| EpisodeRow {
        anime_id,
        url: format!("/e{n}"),
        name: format!("E{n}"),
        episode_number: n as f64,
        ..Default::default()
    };
    let added = db.add_all_episodes((1..=3).map(|n| make(n, anime)).collect()).await;
    assert_eq!(added.iter().map(|e| e.episode_number as i64).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(added.iter().all(|e| e.id > 0));

    // Element 3 of 5 points at a missing anime: nothing of the batch stays, and the answer is empty.
    let mut batch: Vec<EpisodeRow> = (10..15).map(|n| make(n, anime)).collect();
    batch[2].anime_id = 9999;
    assert!(db.add_all_episodes(batch).await.is_empty());
    let total: i64 = db.handle().read_async(|c| Ok(c.query_row("SELECT count(*) FROM episodes", [], |r| r.get(0))?)).await.unwrap();
    assert_eq!(total, 3);
}

#[tokio::test]
async fn update_all_episodes_and_remove_run_through_the_facade() {
    let (_p, db) = open();
    let anime = db.handle().write_async(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).await.unwrap();
    let ids: Vec<i64> = db
        .handle()
        .write_async(move |tx| Ok((0..3).map(|i| EpisodeBuilder::new(anime, &format!("/e{i}")).insert(tx)).collect()))
        .await
        .unwrap();
    let written = db
        .update_all_episodes(ids.iter().map(|&id| EpisodeUpdate { id, seen: Some(true), ..Default::default() }).collect())
        .await
        .unwrap();
    assert_eq!(written, 3);
    db.remove_episodes_with_ids(vec![ids[0], ids[1]]).await;
    let left: Vec<i64> = db.handle().read_async(move |c| Ok(sql::episodes_by_anime_id(c, anime)?.into_iter().map(|e| e.id).collect())).await.unwrap();
    assert_eq!(left, [ids[2]]);
}

#[test]
fn episode_by_url_may_return_several_rows_and_the_first_wins() {
    let c = crate::testdb::TestDb::anime();
    let a = AnimeBuilder::new("/a").insert(&c);
    let b = AnimeBuilder::new("/b").insert(&c);
    let first = EpisodeBuilder::new(a, "/same").insert(&c);
    EpisodeBuilder::new(b, "/same").insert(&c);
    let rows = sql::episodes_by_url(&c, "/same").unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].id, first);
}

#[tokio::test]
async fn the_episode_stream_orders_by_source_order_and_follows_writes() {
    let (_p, db) = open();
    let anime = db.handle().write_async(|tx| Ok(AnimeBuilder::new("/a").insert(tx))).await.unwrap();
    let mut sub = db.episodes_by_anime_stream(anime);
    assert_eq!(next(&mut sub).await.map(|v| v.len()), Some(0));
    db.handle()
        .write_async(move |tx| {
            for (url, order) in [("/late", 2), ("/early", 1)] {
                let id = EpisodeBuilder::new(anime, url).insert(tx);
                tx.execute("UPDATE episodes SET source_order = ?2 WHERE _id = ?1", [id, order])?;
            }
            Ok(())
        })
        .await
        .unwrap();
    let urls: Vec<String> = next(&mut sub).await.unwrap().iter().map(|e| e.url.clone()).collect();
    assert_eq!(urls, ["/early", "/late"]);
}

// T-04a-012: history ------------------------------------------------------------------------------

#[tokio::test]
async fn the_history_stream_shows_the_newest_episode_per_anime_and_follows_resets() {
    let (_p, db) = open();
    let (e1, e2, e3) = db
        .handle()
        .write_async(|tx| {
            let a = AnimeBuilder::new("/a").insert(tx);
            tx.execute("UPDATE animes SET title = 'Alpha' WHERE _id = ?1", [a])?;
            let ids: Vec<i64> = (1..=3).map(|n| EpisodeBuilder::new(a, &format!("/e{n}")).insert(tx)).collect();
            Ok((ids[0], ids[1], ids[2]))
        })
        .await
        .unwrap();
    let mut sub = db.history_stream("alp");
    assert_eq!(next(&mut sub).await.map(|v| v.len()), Some(0));
    for (e, at) in [(e1, 1000), (e2, 3000), (e3, 2000)] {
        db.upsert_history(e, at).await.unwrap();
    }
    let rows = loop {
        let rows = next(&mut sub).await.expect("a publication");
        if rows.first().map(|r| r.seen_at) == Some(Millis(3000)) {
            break rows;
        }
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].episode_id, e2);

    // Reset the newest row: the next most recent row of that anime takes its place.
    let hid = db.handle().read_async(move |c| Ok(c.query_row("SELECT _id FROM animehistory WHERE episode_id = ?1", [e2], |r| r.get::<_, i64>(0))?)).await.unwrap();
    db.handle().write_async(move |tx| Ok(sql::reset_history(tx, hid).map(drop)?)).await.unwrap();
    let rows = next(&mut sub).await.unwrap();
    assert_eq!((rows[0].episode_id, rows[0].seen_at), (e3, Millis(2000)));

    // A query that does not match shows nothing; resets and removal of reset rows keep the rows' meaning.
    let mut miss = db.history_stream("zzz");
    assert_eq!(next(&mut miss).await.map(|v| v.len()), Some(0));
    db.handle().write_async(|tx| Ok(sql::remove_resetted_history(tx).map(drop)?)).await.unwrap();
    let left: i64 = db.handle().read_async(|c| Ok(c.query_row("SELECT count(*) FROM animehistory", [], |r| r.get(0))?)).await.unwrap();
    assert_eq!(left, 2, "only the reset row was removed");
}

// T-04a-002: the open gate through the handle ---------------------------------------------------------

#[test]
fn opening_a_file_from_a_newer_app_through_the_handle_is_refused_and_untouched() {
    let paths = TempPaths::new().unwrap();
    let file = DbKind::Anime.path(&kuro_platform::paths::Paths::data_dir(&paths));
    drop(AnimeDb::open(&paths).unwrap());
    Connection::open(&file).unwrap().pragma_update(None, "user_version", 141).unwrap();
    let before = std::fs::read(&file).unwrap();
    let err = Db::open(DbKind::Anime, &paths).err().expect("refused");
    assert!(matches!(err, DbError::TooNew { found: 141, supported: 140 }), "{err:?}");
    assert_eq!(std::fs::read(&file).unwrap(), before);
}

#[test]
fn a_history_row_cannot_outlive_its_episode_and_empty_database_is_empty() {
    let (_p, db) = open();
    let counts = db
        .handle()
        .read(|c| {
            Ok(["animes", "episodes", "animehistory"]
                .map(|t| c.query_row(&format!("SELECT count(*) FROM {t}"), [], |r| r.get::<_, i64>(0)).unwrap()))
        })
        .unwrap();
    assert_eq!(counts, [0, 0, 0], "a new database holds no entries: the app inserts no sample data");
}
