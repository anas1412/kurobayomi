//! Golden and integration tests of the domain layer (T-10a-036). They use only the public API, `kuro-testkit` and a
//! migrated database in a temporary directory. Test names follow `p1-edge-cases-and-tests.md` section 4.

use kuro_platform::kind::Kind;
use std::sync::{Arc, Mutex};

use kuro_data::chapter::{Chapter, ChapterRepo};
use kuro_data::episode::Episode;
use kuro_data::manga::{Manga, MangaRepo};
use kuro_db::Db;
use kuro_db::sql::{chapters, excluded_scanlators};
use kuro_domain::app::status::{ChapterDownloads, SetReadStatus, SetSeenResult, SetSeenStatus, SetStatusResult, EpisodeDownloads};
use kuro_domain::app::update::{CoverCache, UpdateManga};
use kuro_domain::items::chapter_sync::{ChapterSyncOptions, NoRenames, SyncChapters};
use kuro_domain::items::source_sync::MangaSyncLocks;
use kuro_domain::track::delayed::{DelayedTrackingStore, DelayedTrackingWorker, QUEUE_FILE, ReplayOutcome, TrackReplay};
use kuro_prefs::store::PrefStore;
use kuro_source::model::SChapter;
use kuro_testkit::{InMemorySettings, TempDir, TestDb};

const NOW_MS: i64 = 1_000_000;

/// A migrated manga database and a manga row titled `title`.
fn manga_fixture(title: &str) -> (TempDir, Arc<Db>, i64) {
    let dir = TempDir::new().expect("temp dir");
    let db = TestDb::manga_handle(&dir);
    let id = MangaRepo::new(db.clone()).insert(&Manga { title: title.into(), ..Manga::create() }).expect("insert manga");
    (dir, db, id)
}

fn chapter_row(manga_id: i64, url: &str, name: &str, number: f64) -> kuro_db::statements_manga::ChapterRow {
    let c = Chapter { manga_id, url: url.into(), name: name.into(), chapter_number: number, ..Chapter::create() };
    kuro_db::statements_manga::ChapterRow {
        id: 0,
        manga_id,
        url: c.url,
        name: c.name,
        scanlator: c.scanlator,
        read: c.read,
        bookmark: c.bookmark,
        last_page_read: c.last_page_read,
        chapter_number: c.chapter_number,
        source_order: c.source_order,
        date_fetch: c.date_fetch,
        date_upload: c.date_upload,
        last_modified_at: 0,
        version: 1,
        is_syncing: false,
    }
}

fn stored_chapters(db: &Arc<Db>, manga_id: i64) -> Vec<Chapter> {
    ChapterRepo::new(db.clone()).by_manga_id(manga_id, false).expect("read chapters")
}

fn sync(db: &Arc<Db>, locks: &Arc<MangaSyncLocks>) -> SyncChapters {
    SyncChapters::new(db.clone(), locks.clone(), Arc::new(NoRenames))
}

fn options(now_ms: i64) -> ChapterSyncOptions {
    ChapterSyncOptions { now_ms, manual: false, window: (0, 0), mark_duplicate_as_read: true, is_local: false }
}

fn source_chapter(url: &str, name: &str, date_upload: i64) -> SChapter {
    SChapter { url: url.into(), name: name.into(), date_upload, chapter_number: -1.0, scanlator: None }
}

fn no_hook(_: &mut SChapter) {}

fn manga_row(db: &Arc<Db>, id: i64) -> Manga {
    MangaRepo::new(db.clone()).by_id(id).expect("read manga")
}

mod sync_chapters {
    use super::*;

    #[tokio::test]
    async fn worked_example() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("fixtures/sync/worked_example.json")).expect("fixture");
        let (_dir, db, id) = manga_fixture(fixture["manga_title"].as_str().expect("title"));
        for row in fixture["stored"].as_array().expect("stored") {
            let mut r = chapter_row(id, row["url"].as_str().expect("url"), row["name"].as_str().expect("name"), row["number"].as_f64().expect("number"));
            r.read = row["read"].as_bool().expect("read");
            r.bookmark = row["bookmark"].as_bool().expect("bookmark");
            r.date_fetch = row["date_fetch"].as_i64().expect("date_fetch");
            r.source_order = row["source_order"].as_i64().expect("source_order");
            db.write(|tx| Ok(chapters::insert(tx, &r)?)).expect("insert");
        }
        let raw: Vec<SChapter> = fixture["source"]
            .as_array()
            .expect("source")
            .iter()
            .map(|c| source_chapter(c["url"].as_str().expect("url"), c["name"].as_str().expect("name"), c["date_upload"].as_i64().expect("date")))
            .collect();
        let locks = Arc::new(MangaSyncLocks::default());
        let manga = manga_row(&db, id);
        let returned = sync(&db, &locks).run(raw, &manga, options(fixture["now_ms"].as_i64().expect("now")), &no_hook).await.expect("sync");
        let urls: Vec<&str> = returned.iter().map(|c| c.url.as_str()).collect();
        let expected: Vec<&str> = fixture["expected_returned_urls"].as_array().expect("urls").iter().map(|u| u.as_str().expect("url")).collect();
        assert_eq!(urls, expected, "only /c5 and /c4 are reported as new");

        let rows = stored_chapters(&db, id);
        for (url, want) in fixture["expected_date_fetch"].as_object().expect("dates") {
            let row = rows.iter().find(|r| &r.url == url).expect("row");
            assert_eq!(row.date_fetch, want.as_i64().expect("date"), "dateFetch of {url}");
        }
        assert!(rows.iter().all(|r| r.url != "/c3"), "the removed chapter is deleted");
        let c3b = rows.iter().find(|r| r.url == "/c3b").expect("c3b");
        assert!(c3b.read && c3b.bookmark, "c3b inherits read and bookmark from the removed twin");
    }

    #[tokio::test]
    async fn empty_source_errors() {
        let (_dir, db, id) = manga_fixture("Foo");
        let manga = manga_row(&db, id);
        let err = sync(&db, &Arc::new(MangaSyncLocks::default())).run(Vec::new(), &manga, options(NOW_MS), &no_hook).await;
        assert!(err.is_err(), "EC-01");
        assert!(stored_chapters(&db, id).is_empty(), "nothing written");
    }

    #[tokio::test]
    async fn local_empty_ok() {
        let (_dir, db, id) = manga_fixture("Foo");
        let manga = manga_row(&db, id);
        let local = ChapterSyncOptions { is_local: true, ..options(NOW_MS) };
        let out = sync(&db, &Arc::new(MangaSyncLocks::default())).run(Vec::new(), &manga, local, &no_hook).await.expect("ok");
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn no_change_no_write() {
        let (_dir, db, id) = manga_fixture("Foo");
        let locks = Arc::new(MangaSyncLocks::default());
        let raw = vec![source_chapter("/a", "Ch 1", 10), source_chapter("/b", "Ch 2", 20)];
        let first = manga_row(&db, id);
        sync(&db, &locks).run(raw.clone(), &first, options(NOW_MS), &no_hook).await.expect("first");
        let after_first = manga_row(&db, id);
        let second = sync(&db, &locks).run(raw, &after_first, options(NOW_MS + 60_000), &no_hook).await.expect("second");
        assert!(second.is_empty(), "EC-09");
        assert_eq!(manga_row(&db, id).last_update, after_first.last_update, "no last_update bump on a no-change refresh");
    }

    #[tokio::test]
    async fn duplicate_urls_first_wins() {
        let (_dir, db, id) = manga_fixture("Foo");
        let manga = manga_row(&db, id);
        let raw = vec![source_chapter("/x", "Ch 1", 0), source_chapter("/x", "Ch 1 again", 0), source_chapter("/y", "Ch 2", 0)];
        let out = sync(&db, &Arc::new(MangaSyncLocks::default())).run(raw, &manga, options(NOW_MS), &no_hook).await.expect("sync");
        assert_eq!(out.len(), 2, "EC-03");
        assert_eq!(out[0].name, "Ch 1", "the first occurrence wins");
    }

    #[tokio::test]
    async fn excluded_scanlators_filter() {
        let (_dir, db, id) = manga_fixture("Foo");
        db.write(|tx| Ok(excluded_scanlators::insert(tx, id, "Bad")?)).expect("exclude");
        let manga = manga_row(&db, id);
        let raw = vec![
            SChapter { scanlator: Some("Bad".into()), ..source_chapter("/a", "Ch 1", 0) },
            SChapter { scanlator: Some("Good".into()), ..source_chapter("/b", "Ch 2", 0) },
        ];
        let out = sync(&db, &Arc::new(MangaSyncLocks::default())).run(raw, &manga, options(NOW_MS), &no_hook).await.expect("sync");
        assert_eq!(out.iter().map(|c| c.url.as_str()).collect::<Vec<_>>(), ["/b"], "EC-12: not reported as new");
        assert_eq!(stored_chapters(&db, id).len(), 2, "but both rows are inserted");
    }

    #[tokio::test]
    async fn f32_number_not_requeued() {
        let (_dir, db, id) = manga_fixture("Foo");
        let locks = Arc::new(MangaSyncLocks::default());
        let raw = vec![SChapter { chapter_number: 12.1_f32, ..source_chapter("/c12", "Ch 12.1", 0) }];
        let manga = manga_row(&db, id);
        sync(&db, &locks).run(raw.clone(), &manga, options(NOW_MS), &no_hook).await.expect("first");
        assert_eq!(stored_chapters(&db, id)[0].chapter_number, 12.1_f64, "EC-CH-07: the decimal form, not 12.100000762939453");
        let after = manga_row(&db, id);
        let second = sync(&db, &locks).run(raw, &after, options(NOW_MS), &no_hook).await.expect("second");
        assert!(second.is_empty(), "an identical second sync reports nothing");
        assert_eq!(manga_row(&db, id).last_update, after.last_update, "and writes nothing");
    }

    #[tokio::test]
    async fn scanlator_never_cleared() {
        let (_dir, db, id) = manga_fixture("Foo");
        let locks = Arc::new(MangaSyncLocks::default());
        let first = vec![SChapter { scanlator: Some("Team".into()), ..source_chapter("/c", "Ch 1", 0) }];
        let manga = manga_row(&db, id);
        sync(&db, &locks).run(first, &manga, options(NOW_MS), &no_hook).await.expect("first");
        let manga = manga_row(&db, id);
        sync(&db, &locks).run(vec![source_chapter("/c", "Ch 1", 0)], &manga, options(NOW_MS), &no_hook).await.expect("second");
        assert_eq!(stored_chapters(&db, id)[0].scanlator.as_deref(), Some("Team"), "EC-08");
    }
}

mod sync_episodes {
    use super::*;
    use kuro_domain::items::episode_sync::{EpisodeSyncOptions, NoEpisodeRenames, SyncEpisodes};
    use kuro_data::anime::{Anime, AnimeRepo};
    use kuro_data::episode::EpisodeRepo;
    use kuro_db::statements_anime::{EpisodeRow, insert_episode};
    use kuro_source::anime::SEpisode;

    #[tokio::test]
    async fn fillermark_sticky_and_preview_fill() {
        let dir = TempDir::new().expect("temp dir");
        let db = TestDb::anime_handle(&dir);
        let anime_id = AnimeRepo::new(db.clone()).insert(&Anime::create()).expect("anime");
        let anime = AnimeRepo::new(db.clone()).by_id(anime_id).expect("read");
        db.write(|tx| {
            let row = EpisodeRow {
                id: 0,
                anime_id,
                url: "/e1".into(),
                name: "Ep 1".into(),
                scanlator: None,
                seen: false,
                bookmark: false,
                last_second_seen: 0,
                total_seconds: 0,
                episode_number: 1.0,
                source_order: 0,
                date_fetch: 0,
                date_upload: 0,
                last_modified_at: 0,
                version: 1,
                is_syncing: 0,
                summary: None,
                preview_url: None,
                fillermark: true,
                memo: kuro_db::schema_anime::Memo(serde_json::Map::new()),
            };
            Ok(insert_episode(tx, &row)?)
        })
        .expect("insert");
        let sync = SyncEpisodes::new(db.clone(), Arc::new(MangaSyncLocks::default()), Arc::new(NoEpisodeRenames));
        let raw = vec![SEpisode {
            url: "/e1".into(),
            name: "Ep 1".into(),
            episode_number: 1.0,
            fillermark: false,
            scanlator: None,
            date_upload: 0,
            summary: None,
            preview_url: Some("preview".into()),
            memo: serde_json::Map::new(),
        }];
        let opts = EpisodeSyncOptions { now_ms: NOW_MS, manual: false, window: (0, 0), mark_duplicate_as_seen: false, is_local: false };
        sync.run(raw, &anime, opts).await.expect("sync");
        let row = EpisodeRepo::new(db.clone()).by_anime_id(anime_id).expect("read").remove(0);
        assert!(row.fillermark, "a stored fillermark is never cleared by the source");
        assert_eq!(row.preview_url.as_deref(), Some("preview"), "a blank preview is filled");
    }
}

mod update_manga {
    use super::*;

    #[derive(Default)]
    struct Covers {
        deleted: Mutex<Vec<bool>>,
    }

    impl CoverCache for Covers {
        fn has_custom_cover(&self, _manga: &Manga) -> bool {
            false
        }
        fn delete_from_cache(&self, _manga: &Manga, delete_custom: bool) {
            self.deleted.lock().expect("lock").push(delete_custom);
        }
    }

    #[test]
    fn title_not_overwritten_for_favorite() {
        let (_dir, db, id) = manga_fixture("Kept");
        let repo = MangaRepo::new(db.clone());
        repo.update(&kuro_data::manga::MangaUpdate { favorite: Some(true), ..kuro_data::manga::MangaUpdate::new(id) }).expect("favorite");
        let interactor = UpdateManga::new(repo.clone(), ChapterRepo::new(db));
        let local = repo.by_id(id).expect("read");
        let remote = kuro_source::model::SManga { title: "Remote".into(), ..kuro_source::model::SManga::new("/m", "M") };
        let covers = Covers::default();
        let now = chrono::Local::now();
        interactor.update_from_source(&local, &remote, false, &covers, &now).expect("update");
        assert_eq!(repo.by_id(id).expect("read").title, "Kept", "EC-18");
    }

    #[test]
    fn favorite_sets_date_added() {
        let (_dir, db, id) = manga_fixture("Foo");
        let repo = MangaRepo::new(db.clone());
        let interactor = UpdateManga::new(repo.clone(), ChapterRepo::new(db));
        let now = chrono::DateTime::from_timestamp_millis(1_700_000_000_000).expect("instant").with_timezone(&chrono::Local);
        interactor.update_favorite(id, true, &now).expect("on");
        assert_eq!(repo.by_id(id).expect("read").date_added, 1_700_000_000_000, "EC-25");
        interactor.update_favorite(id, false, &now).expect("off");
        assert_eq!(repo.by_id(id).expect("read").date_added, 0);
    }
}

mod set_read_status {
    use super::*;

    #[derive(Default)]
    struct Downloads {
        calls: Mutex<usize>,
    }

    impl ChapterDownloads for Downloads {
        fn delete_chapters(&self, _manga: &Manga, _chapters: &[Chapter]) {
            *self.calls.lock().expect("lock") += 1;
        }
    }

    fn pref(on: bool) -> kuro_prefs::pref::Pref<bool> {
        let settings = InMemorySettings::new();
        let store: PrefStore = PrefStore::clone(&settings);
        let pref = store.pref("pref_remove_after_marked_as_read_key", false);
        pref.set(on);
        pref
    }

    #[tokio::test]
    async fn results() {
        let (_dir, db, id) = manga_fixture("Foo");
        db.write(|tx| Ok(chapters::insert(tx, &chapter_row(id, "/a", "Ch 1", 1.0))?)).expect("insert");
        let chapters_repo = ChapterRepo::new(db.clone());
        let mangas = MangaRepo::new(db.clone());
        let downloads = Arc::new(Downloads::default());
        let set = SetReadStatus::new(chapters_repo.clone(), mangas, pref(true), downloads.clone());
        let rows = stored_chapters(&db, id);
        assert_eq!(set.run(true, rows.clone()).await, SetStatusResult::Success);
        assert_eq!(*downloads.calls.lock().expect("lock"), 1, "downloads deleted once for the manga");
        let read_rows = stored_chapters(&db, id);
        assert_eq!(set.run(true, read_rows).await, SetStatusResult::NoItems, "already read: nothing to do");
    }
}

mod set_seen_status {
    use super::*;

    #[derive(Default)]
    struct EpisodeDl;

    impl EpisodeDownloads for EpisodeDl {
        fn delete_episodes(&self, _anime: &kuro_data::anime::Anime, _episodes: &[Episode]) {}
    }

    #[tokio::test]
    async fn last_second_reset() {
        let dir = TempDir::new().expect("temp dir");
        let db = TestDb::anime_handle(&dir);
        let animes = kuro_data::anime::AnimeRepo::new(db.clone());
        let episodes = kuro_data::episode::EpisodeRepo::new(db.clone());
        let anime_id = animes.insert(&kuro_data::anime::Anime::create()).expect("anime");
        db.write(|tx| {
            Ok(kuro_db::statements_anime::insert_episode(
                tx,
                &kuro_db::statements_anime::EpisodeRow {
                    id: 0,
                    anime_id,
                    url: "/e".into(),
                    name: "Ep".into(),
                    scanlator: None,
                    seen: true,
                    bookmark: false,
                    last_second_seen: 0,
                    total_seconds: 500,
                    episode_number: 1.0,
                    source_order: 0,
                    date_fetch: 0,
                    date_upload: 0,
                    last_modified_at: 0,
                    version: 1,
                    is_syncing: 0,
                    summary: None,
                    preview_url: None,
                    fillermark: false,
                    memo: kuro_db::schema_anime::Memo(serde_json::Map::new()),
                },
            )?)
        })
        .expect("insert");
        let set = SetSeenStatus::new(episodes.clone(), animes, pref(false), Arc::new(EpisodeDl));
        db.write(|tx| Ok(tx.execute("UPDATE episodes SET last_second_seen = 40 WHERE anime_id = ?1", [anime_id])?)).expect("progress");
        let rows = episodes.by_anime_id(anime_id).expect("read");
        assert_eq!(set.run(false, rows).await, SetSeenResult::Success);
        let row = episodes.by_anime_id(anime_id).expect("read").remove(0);
        assert!(!row.seen);
        assert_eq!(row.last_second_seen, 0, "marking unseen resets the position");
        assert_eq!(row.total_seconds, 500, "total_seconds is untouched");
    }

    fn pref(on: bool) -> kuro_prefs::pref::Pref<bool> {
        let settings = InMemorySettings::new();
        let store: PrefStore = PrefStore::clone(&settings);
        let pref = store.pref("pref_remove_after_marked_as_read_key", false);
        pref.set(on);
        pref
    }
}

mod domain_end_to_end {
    use super::*;

    /// Sync new chapters, mark one read, then refresh again: the read state survives and the second refresh reports
    /// nothing. The track and offline-queue stages are in [`sync_mark_read_track_offline_then_replay`].
    #[tokio::test]
    async fn sync_then_mark_read_then_refresh() {
        let (_dir, db, id) = manga_fixture("Foo");
        let locks = Arc::new(MangaSyncLocks::default());
        let raw = vec![source_chapter("/c2", "Ch 2", 0), source_chapter("/c1", "Ch 1", 0)];
        let manga = manga_row(&db, id);
        let added = sync(&db, &locks).run(raw.clone(), &manga, options(NOW_MS), &no_hook).await.expect("first sync");
        assert_eq!(added.len(), 2);

        let read = stored_chapters(&db, id).into_iter().filter(|c| c.chapter_number == 1.0).collect::<Vec<_>>();
        let set = SetReadStatus::new(
            ChapterRepo::new(db.clone()),
            MangaRepo::new(db.clone()),
            {
                let settings = InMemorySettings::new();
                let store: PrefStore = PrefStore::clone(&settings);
                let p = store.pref("pref_remove_after_marked_as_read_key", false);
                p.set(false);
                p
            },
            Arc::new(NoDownloads),
        );
        assert_eq!(set.run(true, read).await, SetStatusResult::Success);

        let after = manga_row(&db, id);
        let again = sync(&db, &locks).run(raw, &after, options(NOW_MS + 1), &no_hook).await.expect("second sync");
        assert!(again.is_empty(), "nothing new on the second refresh");
        let c1 = stored_chapters(&db, id).into_iter().find(|c| c.url == "/c1").expect("c1");
        assert!(c1.read, "the read state survives the refresh");
    }

    /// A tracker the test switches online: `replay` pushes the progress and records it.
    struct FakeTracker {
        online: tokio::sync::watch::Sender<bool>,
        pushed: Mutex<Vec<(Kind, i64, f32)>>,
    }

    #[async_trait::async_trait]
    impl TrackReplay for FakeTracker {
        async fn wait_online(&self) {
            let mut rx = self.online.subscribe();
            let _ = rx.wait_for(|up| *up).await;
        }

        async fn replay(&self, kind: Kind, track_id: i64, progress: f32) -> ReplayOutcome {
            self.pushed.lock().expect("lock").push((kind, track_id, progress));
            ReplayOutcome::Done
        }
    }

    /// `domain_end_to_end` of the plan: sync (new chapters) -> mark read -> track update while offline (queued) -> go
    /// online -> the worker replays. Asserts the final database, queue file and tracker state.
    #[tokio::test]
    async fn sync_mark_read_track_offline_then_replay() {
        let (dir, db, id) = manga_fixture("Foo");
        let locks = Arc::new(MangaSyncLocks::default());
        let raw = vec![source_chapter("/c2", "Ch 2", 0), source_chapter("/c1", "Ch 1", 0)];
        let manga = manga_row(&db, id);
        sync(&db, &locks).run(raw, &manga, options(NOW_MS), &no_hook).await.expect("sync");

        let read: Vec<Chapter> = stored_chapters(&db, id).into_iter().filter(|c| c.chapter_number == 1.0).collect();
        let progress = read.iter().map(|c| c.chapter_number).fold(0.0, f64::max);
        let pref = {
            let settings = InMemorySettings::new();
            let store: PrefStore = PrefStore::clone(&settings);
            let p = store.pref("pref_remove_after_marked_as_read_key", false);
            p.set(false);
            p
        };
        let set = SetReadStatus::new(ChapterRepo::new(db.clone()), MangaRepo::new(db.clone()), pref, Arc::new(NoDownloads));
        assert_eq!(set.run(true, read).await, SetStatusResult::Success);

        // The tracker is offline: the push cannot happen, so the progress is queued under the track row id.
        let (up, _keep) = tokio::sync::watch::channel(false);
        let tracker = Arc::new(FakeTracker { online: up, pushed: Mutex::default() });
        let queue_path = dir.path().join(QUEUE_FILE);
        let store = Arc::new(DelayedTrackingStore::new(queue_path.clone()));
        store.add(Kind::Manga, 7, progress).expect("queue");
        let worker = DelayedTrackingWorker::new(store.clone(), tracker.clone());
        worker.schedule();
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(tracker.pushed.lock().expect("lock").is_empty(), "offline: nothing is pushed");
        assert_eq!(store.items(Kind::Manga), vec![(7, 1.0_f32)], "and the progress waits in the queue");

        // Online again: the worker replays the item and empties the queue.
        tracker.online.send_replace(true);
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while worker.is_running() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the worker finishes");

        assert_eq!(*tracker.pushed.lock().expect("lock"), vec![(Kind::Manga, 7, 1.0_f32)], "tracker state");
        assert!(store.is_empty(), "queue state");
        assert!(DelayedTrackingStore::new(queue_path).is_empty(), "the queue file is empty too");
        let c1 = stored_chapters(&db, id).into_iter().find(|c| c.url == "/c1").expect("c1");
        let c2 = stored_chapters(&db, id).into_iter().find(|c| c.url == "/c2").expect("c2");
        assert!(c1.read && !c2.read, "database state");
    }

    struct NoDownloads;

    impl ChapterDownloads for NoDownloads {
        fn delete_chapters(&self, _manga: &Manga, _chapters: &[Chapter]) {}
    }
}
