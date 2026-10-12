//! `SyncEpisodesWithSource` (T-10a-009): the anime names of the shared item sync ([`crate::items::sync`]). Everything the
//! refresh does is in [`SyncItems`]; this file turns the source episode, the legacy `Anime` and `Episode` shapes and the
//! old option and error types into it and back, and carries no logic of its own. Phase E deletes it.

use std::sync::Arc;

use kuro_data::anime::Anime;
use kuro_data::entry::Entry;
use kuro_data::episode::Episode;
use kuro_data::item::Item;
use kuro_db::Db;
use kuro_platform::kind::Kind;
use kuro_source::anime::SEpisode;

use crate::error::NoEpisodes;
use crate::items::source_sync::MangaSyncLocks;
use crate::items::sync::{
    ItemRenames, SyncItems, SyncItemsError, SyncOptions, mark_duplicate_done_from_pref, source_item_from_episode,
};

/// The download folder rename of a changed episode (`downloadManager.renameEpisode`). Same contract as the chapter port:
/// the implementation logs its own failures and never fails the sync (EC-11).
pub trait EpisodeRenames: Send + Sync {
    fn rename(&self, anime: &Anime, stored: &Episode, prepared: &Episode);
}

/// A download port that does nothing.
pub struct NoEpisodeRenames;

impl EpisodeRenames for NoEpisodeRenames {
    fn rename(&self, _anime: &Anime, _stored: &Episode, _prepared: &Episode) {}
}

/// Why an episode refresh failed.
#[derive(Debug, thiserror::Error)]
pub enum EpisodeSyncError {
    /// A non-local source returned no episodes. Nothing was written.
    #[error(transparent)]
    NoEpisodes(#[from] NoEpisodes),
    #[error("database error: {0}")]
    Db(#[from] kuro_db::DbError),
}

/// The options of one refresh, as [`crate::items::chapter_sync::ChapterSyncOptions`], with the seen preference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EpisodeSyncOptions {
    /// The refresh time, epoch milliseconds.
    pub now_ms: i64,
    pub manual: bool,
    pub window: (i64, i64),
    pub mark_duplicate_as_seen: bool,
    pub is_local: bool,
}

/// The duplicate rule from its multi-select preference: on when the set holds `"new_episode"` (`MARK_DUPLICATE_EPISODE_SEEN_NEW`). The `"existing"`
/// value is a separate rule and does not turn this one on.
pub fn mark_duplicate_as_seen_from_pref(rule: &std::collections::BTreeSet<String>) -> bool {
    mark_duplicate_done_from_pref(Kind::Anime, rule)
}

/// An [`EpisodeRenames`] as the shared [`ItemRenames`].
struct RenameAdapter(Arc<dyn EpisodeRenames>);

impl ItemRenames for RenameAdapter {
    fn rename(&self, e: &Entry, stored: &Item, prepared: &Item) {
        self.0.rename(&Anime::from(e.clone()), &Episode::from(stored.clone()), &Episode::from(prepared.clone()));
    }
}

/// The episode sync of one anime. Clones share the database handle and the per-entry locks.
#[derive(Clone)]
pub struct SyncEpisodes {
    sync: SyncItems,
}

impl SyncEpisodes {
    pub fn new(db: Arc<Db>, locks: Arc<MangaSyncLocks>, renames: Arc<dyn EpisodeRenames>) -> Self {
        Self { sync: SyncItems::new(db, locks, Arc::new(RenameAdapter(renames))) }
    }

    /// `await(rawSourceEpisodes, anime, manualFetch, fetchWindow)`. Returns the NEW episodes that should notify and
    /// auto-download. Unlike chapters there is no scanlator exclusion and no source hook.
    pub async fn run(
        &self,
        raw: Vec<SEpisode>,
        anime: &Anime,
        opts: EpisodeSyncOptions,
    ) -> Result<Vec<Episode>, EpisodeSyncError> {
        let options = SyncOptions {
            now_ms: opts.now_ms,
            manual: opts.manual,
            window: opts.window,
            mark_duplicate_done: opts.mark_duplicate_as_seen,
            is_local: opts.is_local,
        };
        let items = raw.iter().map(source_item_from_episode).collect();
        match self.sync.run(items, &Entry::from(anime), options, None).await {
            Ok(new) => Ok(new.into_iter().map(Episode::from).collect()),
            Err(SyncItemsError::NoItems) => Err(NoEpisodes.into()),
            Err(SyncItemsError::Db(e)) => Err(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::anime::Anime;
    use kuro_data::episode::{Episode, map_episode};
    use kuro_db::Db;
    use kuro_db::schema_anime::Memo;
    use kuro_db::statements_anime::{AnimeUpdate as DbAnimeUpdate, EpisodeRow};

    /// The row the legacy insert statement takes.
    fn episode_row(e: &Episode) -> EpisodeRow {
        EpisodeRow {
            id: e.id,
            anime_id: e.anime_id,
            url: e.url.clone(),
            name: e.name.clone(),
            scanlator: e.scanlator.clone(),
            seen: e.seen,
            bookmark: e.bookmark,
            last_second_seen: e.last_second_seen,
            total_seconds: e.total_seconds,
            episode_number: e.episode_number,
            source_order: e.source_order,
            date_fetch: e.date_fetch,
            date_upload: e.date_upload,
            last_modified_at: 0,
            version: e.version,
            is_syncing: 0,
            summary: e.summary.clone(),
            preview_url: e.preview_url.clone(),
            fillermark: e.fillermark,
            memo: Memo(e.memo.clone()),
        }
    }

    fn empty_anime_update(id: i64) -> DbAnimeUpdate {
        DbAnimeUpdate { id, ..Default::default() }
    }

    #[test]
    fn the_duplicate_preference_set_maps_to_the_flag() {
        let set = |items: &[&str]| items.iter().map(|s| (*s).to_owned()).collect::<std::collections::BTreeSet<String>>();
        assert!(mark_duplicate_as_seen_from_pref(&set(&["new_episode"])), "on");
        assert!(mark_duplicate_as_seen_from_pref(&set(&["new_episode", "existing_episode"])));
        assert!(!mark_duplicate_as_seen_from_pref(&set(&[])), "off");
        assert!(!mark_duplicate_as_seen_from_pref(&set(&["existing_episode"])), "existing is another rule");
    }
    use crate::testdb::test_db;
    use kuro_db::statements_anime::{episodes_by_anime_id, insert_episode};


    fn fixture() -> (tempfile::TempDir, Arc<Db>, SyncEpisodes, Anime) {
        let (dir, db) = test_db();
        let anime_id = kuro_data::anime::AnimeRepo::new(db.clone()).insert(&Anime::create()).expect("insert anime");
        let anime = kuro_data::anime::AnimeRepo::new(db.clone()).by_id(anime_id).expect("read");
        let sync = SyncEpisodes::new(db.clone(), Arc::new(MangaSyncLocks::default()), Arc::new(NoEpisodeRenames));
        (dir, db, sync, anime)
    }

    fn src(url: &str, name: &str) -> SEpisode {
        SEpisode {
            url: url.into(),
            name: name.into(),
            episode_number: -1.0,
            fillermark: false,
            scanlator: None,
            date_upload: 0,
            summary: None,
            preview_url: None,
            memo: serde_json::Map::new(),
        }
    }

    fn opts(mark: bool) -> EpisodeSyncOptions {
        EpisodeSyncOptions { now_ms: 1_000_000, manual: false, window: (0, 0), mark_duplicate_as_seen: mark, is_local: false }
    }

    #[tokio::test]
    async fn an_empty_remote_list_is_no_episodes() {
        let (_dir, _db, sync, anime) = fixture();
        let err = sync.run(Vec::new(), &anime, opts(false)).await.expect_err("error");
        assert!(matches!(err, EpisodeSyncError::NoEpisodes(_)));
    }

    /// `sync_episodes::fillermark_sticky_and_preview_fill`: a user fillermark is never cleared by the source, and a
    /// stored preview URL is kept while a blank one is filled.
    #[tokio::test]
    async fn fillermark_is_sticky_and_the_preview_is_filled_only_when_blank() {
        let (_dir, db, sync, anime) = fixture();
        let filler = Episode { anime_id: anime.id, url: "/e1".into(), name: "Ep 1".into(), episode_number: 1.0, fillermark: true, preview_url: None, ..Episode::create() };
        let keep = Episode { anime_id: anime.id, url: "/e2".into(), name: "Ep 2".into(), episode_number: 2.0, preview_url: Some("keep".into()), ..Episode::create() };
        for e in [&filler, &keep] {
            db.write(|tx| Ok(insert_episode(tx, &episode_row(e))?)).expect("insert");
        }
        let raw = vec![
            SEpisode { preview_url: Some("new".into()), episode_number: 1.0, ..src("/e1", "Ep 1") },
            SEpisode { preview_url: Some("new".into()), episode_number: 2.0, ..src("/e2", "Ep 2") },
        ];
        sync.run(raw, &anime, opts(false)).await.expect("sync");
        let rows = db.read(|c| Ok(episodes_by_anime_id(c, anime.id)?)).expect("read");
        let by = |u: &str| map_episode(rows.iter().find(|r| r.url == u).expect("row"));
        assert!(by("/e1").fillermark, "a stored fillermark stays");
        assert_eq!(by("/e1").preview_url.as_deref(), Some("new"), "a blank preview is filled");
        assert_eq!(by("/e2").preview_url.as_deref(), Some("keep"), "a stored preview is kept");
    }

    #[tokio::test]
    async fn a_source_fillermark_is_taken_only_while_the_stored_one_is_false() {
        let (_dir, db, sync, anime) = fixture();
        let plain = Episode { anime_id: anime.id, url: "/e1".into(), name: "Ep 1".into(), episode_number: 1.0, ..Episode::create() };
        db.write(|tx| Ok(insert_episode(tx, &episode_row(&plain))?)).expect("insert");
        let raw = vec![SEpisode { fillermark: true, episode_number: 1.0, ..src("/e1", "Ep 1") }];
        sync.run(raw, &anime, opts(false)).await.expect("sync");
        let row = map_episode(&db.read(|c| Ok(episodes_by_anime_id(c, anime.id)?)).expect("read")[0]);
        assert!(row.fillermark, "false takes the source value true");
    }

    #[tokio::test]
    async fn new_episodes_are_inserted_and_returned_with_their_ids() {
        let (_dir, db, sync, anime) = fixture();
        let raw = vec![src("/a", "Ep 1"), src("/b", "Ep 2")];
        let returned = sync.run(raw, &anime, opts(false)).await.expect("sync");
        assert_eq!(returned.len(), 2);
        assert!(returned.iter().all(|e| e.id > 0));
        assert_eq!(db.read(|c| Ok(episodes_by_anime_id(c, anime.id)?)).expect("read").len(), 2);
    }

    #[tokio::test]
    async fn a_duplicate_is_marked_seen_when_the_rule_is_on() {
        let (_dir, db, sync, anime) = fixture();
        let seen = Episode { anime_id: anime.id, url: "/old".into(), name: "Ep 1".into(), episode_number: 1.0, seen: true, ..Episode::create() };
        db.write(|tx| Ok(insert_episode(tx, &episode_row(&seen))?)).expect("insert");
        let raw = vec![src("/old", "Ep 1"), SEpisode { episode_number: 1.0, ..src("/alt", "Ep 1") }];
        let on = sync.run(raw.clone(), &anime, opts(true)).await.expect("sync");
        assert!(on.is_empty(), "the duplicate is marked seen, so it is not reported as new");
        assert!(db.read(|c| Ok(episodes_by_anime_id(c, anime.id)?)).expect("read").iter().any(|r| r.url == "/alt" && r.seen));
    }

    #[tokio::test]
    async fn the_anime_row_gets_last_update_when_episodes_change() {
        let (_dir, db, sync, anime) = fixture();
        sync.run(vec![src("/a", "Ep 1")], &anime, opts(false)).await.expect("sync");
        let row = kuro_data::anime::AnimeRepo::new(db.clone()).by_id(anime.id).expect("read");
        assert!(row.last_update > 0, "last_update is now");
    }

    /// The delete, insert and the rest run in one transaction: a failing insert rolls back the delete.
    #[tokio::test]
    async fn a_failing_insert_rolls_back_the_delete() {
        let (_dir, db, sync, anime) = fixture();
        let gone = Episode { anime_id: anime.id, url: "/gone".into(), name: "Ep 1".into(), episode_number: 1.0, ..Episode::create() };
        db.write(|tx| Ok(insert_episode(tx, &episode_row(&gone))?)).expect("insert");
        db.write(|tx| {
            tx.execute_batch("CREATE TRIGGER boom BEFORE INSERT ON items WHEN NEW.url = '/boom' BEGIN SELECT RAISE(ABORT, 'boom'); END;")?;
            Ok(())
        })
        .expect("trigger");
        let urls = |db: &Db| -> Vec<String> { db.read(|c| Ok(episodes_by_anime_id(c, anime.id)?)).expect("read").into_iter().map(|r| r.url).collect() };
        let err = sync.run(vec![src("/boom", "Ep 2")], &anime, opts(false)).await.expect_err("the insert fails");
        assert!(matches!(err, EpisodeSyncError::Db(_)));
        assert_eq!(urls(&db), ["/gone"], "the removed row is still stored");
        assert_eq!(read_anime(&db, anime.id).last_update, anime.last_update, "no last_update write survived either");
        // Control: without the failing insert the same plan does delete the row.
        db.write(|tx| {
            tx.execute_batch("DROP TRIGGER boom;")?;
            Ok(())
        })
        .expect("drop");
        sync.run(vec![src("/boom", "Ep 2")], &anime, opts(false)).await.expect("sync");
        assert_eq!(urls(&db), ["/boom"]);
    }

    /// RC-08: `run` takes the per-entry lock, so a held lock stops it before any write.
    #[tokio::test]
    async fn run_waits_for_the_per_entry_lock() {
        let (_dir, db, _sync, anime) = fixture();
        let locks = Arc::new(MangaSyncLocks::default());
        let sync = SyncEpisodes::new(db.clone(), locks.clone(), Arc::new(NoEpisodeRenames));
        let held = locks.lock(anime.id).await;
        let blocked = tokio::time::timeout(std::time::Duration::from_millis(50), sync.run(vec![src("/a", "Ep 1")], &anime, opts(false))).await;
        assert!(blocked.is_err(), "the sync waits while the lock is held");
        assert!(db.read(|c| Ok(episodes_by_anime_id(c, anime.id)?)).expect("read").is_empty(), "nothing was written");
        drop(held);
        assert_eq!(sync.run(vec![src("/a", "Ep 1")], &anime, opts(false)).await.expect("sync").len(), 1);
    }

    const SENTINEL_NEXT: i64 = 7_777_777;
    const WINDOW: (i64, i64) = (1_000_000, 2_000_000);

    fn set_interval(db: &Arc<Db>, id: i64, interval: i64, next_update: i64) {
        let patch = DbAnimeUpdate { next_update: Some(next_update), fetch_interval: Some(interval), ..empty_anime_update(id) };
        db.write(|tx| Ok(kuro_db::statements_anime::update_anime(tx, &patch)?)).expect("patch");
    }

    fn read_anime(db: &Arc<Db>, id: i64) -> Anime {
        kuro_data::anime::AnimeRepo::new(db.clone()).by_id(id).expect("read")
    }

    async fn second_sync_row(interval: i64, next_update: i64, manual: bool) -> (Anime, Anime) {
        let (_dir, db, sync, anime) = fixture();
        let raw = vec![src("/a", "Ep 1"), src("/b", "Ep 2")];
        sync.run(raw.clone(), &anime, EpisodeSyncOptions { window: WINDOW, ..opts(false) }).await.expect("first");
        set_interval(&db, anime.id, interval, next_update);
        let before = read_anime(&db, anime.id);
        let again = EpisodeSyncOptions { window: WINDOW, manual, ..opts(false) };
        assert!(sync.run(raw, &before, again).await.expect("second").is_empty());
        (before, read_anime(&db, anime.id))
    }

    /// S-11 for episodes.
    #[tokio::test]
    async fn s11_no_change_and_no_trigger_recomputes_nothing() {
        let (before, after) = second_sync_row(7, SENTINEL_NEXT, false).await;
        assert_eq!(after.next_update, SENTINEL_NEXT);
        assert_eq!(after.fetch_interval, 7);
        assert_eq!(after.last_update, before.last_update);
    }

    /// S-12 for episodes.
    #[tokio::test]
    async fn s12_no_change_recomputes_on_manual_interval_zero_or_overdue() {
        for (interval, next_update, manual, why) in [(7, SENTINEL_NEXT, true, "manual"), (0, SENTINEL_NEXT, false, "interval 0"), (7, 5, false, "next update before the window start")] {
            let (before, after) = second_sync_row(interval, next_update, manual).await;
            assert!(after.next_update != next_update || after.fetch_interval != i32::try_from(interval).unwrap(), "{why}: recomputed");
            assert_eq!(after.last_update, before.last_update, "{why}: last_update is not touched");
        }
    }

    /// S-13 for episodes.
    #[tokio::test]
    async fn s13_changes_recompute_and_set_last_update_to_now() {
        let (_dir, db, sync, anime) = fixture();
        set_interval(&db, anime.id, 7, SENTINEL_NEXT);
        let before = read_anime(&db, anime.id);
        let o = EpisodeSyncOptions { window: WINDOW, ..opts(false) };
        assert_eq!(sync.run(vec![src("/a", "Ep 1")], &before, o).await.expect("sync").len(), 1);
        let after = read_anime(&db, anime.id);
        assert_eq!(after.last_update, 1_000_000);
        assert_ne!(after.next_update, SENTINEL_NEXT, "recomputed");
    }
}
