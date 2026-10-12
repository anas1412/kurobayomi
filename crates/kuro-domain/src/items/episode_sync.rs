//! `SyncEpisodesWithSource` (T-10a-009): the episode twin of [`crate::items::chapter_sync`]. The planner is the same
//! ([`crate::items::sync_plan`]); the differences are the duplicate-seen rule (`mark_duplicate_seen_episode_seen`), the
//! merge of the fillermark and the preview URL, no scanlator exclusion, and the anime tables. The write is one transaction.

use std::sync::Arc;

use chrono::{DateTime, Local};
use kuro_data::anime::Anime;
use kuro_data::episode::{Episode, map_episode};
use kuro_db::Db;
use kuro_db::schema_anime::Memo;
use kuro_db::statements_anime::{AnimeUpdate as DbAnimeUpdate, EpisodeRow, EpisodeUpdate as DbEpisodeUpdate};
use kuro_source::anime::SEpisode;

use crate::entries::fetch_interval::{ItemDates, Stored, to_update};
use crate::error::NoEpisodes;
use crate::items::recognition::parse_episode_number;
use crate::items::should_update::should_update_db_episode;
use crate::items::source_sync::MangaSyncLocks;
use crate::items::model::widen_source_number;
use kuro_data::sanitize::sanitize_item_name;
use crate::items::sync_plan::{PlanInput, SyncRow, plan_sync};

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
    rule.contains(kuro_prefs::library::MARK_DUPLICATE_EPISODE_SEEN_NEW)
}

/// `Episode.copyFromSEpisode`: the source fields. The scanlator is NOT trimmed (only blank becomes `None`).
fn episode_from_source(anime: &Anime, source: &SEpisode, position: usize) -> Episode {
    Episode {
        anime_id: anime.id,
        name: sanitize_item_name(&source.name, &anime.title),
        url: source.url.clone(),
        date_upload: source.date_upload,
        episode_number: widen_source_number(source.episode_number),
        fillermark: source.fillermark,
        scanlator: blank_to_none(source.scanlator.as_deref()),
        summary: blank_to_none(source.summary.as_deref()),
        preview_url: blank_to_none(source.preview_url.as_deref()),
        memo: source.memo.clone(),
        source_order: i64::try_from(position).unwrap_or(i64::MAX),
        ..Episode::create()
    }
}

fn blank_to_none(text: Option<&str>) -> Option<String> {
    text.filter(|t| !t.trim().is_empty()).map(str::to_owned)
}

impl SyncRow for Episode {
    fn id(&self) -> i64 {
        self.id
    }
    fn url(&self) -> &str {
        &self.url
    }
    fn number(&self) -> f64 {
        self.episode_number
    }
    fn read(&self) -> bool {
        self.seen
    }
    fn bookmark(&self) -> bool {
        self.bookmark
    }
    fn date_fetch(&self) -> i64 {
        self.date_fetch
    }
    fn date_upload(&self) -> i64 {
        self.date_upload
    }
    fn set_source_order(&mut self, order: i64) {
        self.source_order = order;
    }
    fn set_date_upload(&mut self, date: i64) {
        self.date_upload = date;
    }
    fn set_date_fetch(&mut self, date: i64) {
        self.date_fetch = date;
    }
    fn set_read(&mut self, seen: bool) {
        self.seen = seen;
    }
    fn set_bookmark(&mut self, bookmark: bool) {
        self.bookmark = bookmark;
    }
    fn needs_update(db: &Self, src: &Self) -> bool {
        should_update_db_episode(db, src)
    }
    /// `toChange` with the episode rules: name, number, scanlator, summary, source order and memo from the source; the
    /// upload date only when the source has one; a fillermark is never cleared by the source (only set while stored false);
    /// the preview URL is filled only while stored blank.
    fn merged(db: &Self, src: &Self) -> Self {
        let mut out = Episode {
            name: src.name.clone(),
            episode_number: src.episode_number,
            scanlator: src.scanlator.clone(),
            summary: src.summary.clone(),
            source_order: src.source_order,
            memo: src.memo.clone(),
            ..db.clone()
        };
        if src.date_upload != 0 {
            out.date_upload = src.date_upload;
        }
        if !out.fillermark {
            out.fillermark = src.fillermark;
        }
        if out.preview_url.as_deref().is_none_or(|p| p.trim().is_empty()) {
            out.preview_url = src.preview_url.clone();
        }
        out
    }
}

/// The row the insert statement takes.
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

/// The columns a refresh owns. Seen, bookmark and the seconds are not written (race 1 of area 10).
fn episode_patch(e: &Episode) -> DbEpisodeUpdate {
    DbEpisodeUpdate {
        id: e.id,
        name: Some(e.name.clone()),
        scanlator: e.scanlator.clone(),
        episode_number: Some(e.episode_number),
        source_order: Some(e.source_order),
        date_upload: Some(e.date_upload),
        summary: e.summary.clone(),
        preview_url: e.preview_url.clone(),
        fillermark: Some(e.fillermark),
        memo: Some(Memo(e.memo.clone())),
        ..Default::default()
    }
}

/// The episode sync of one anime. Clones share the database handle and the per-entry locks.
#[derive(Clone)]
pub struct SyncEpisodes {
    db: Arc<Db>,
    locks: Arc<MangaSyncLocks>,
    renames: Arc<dyn EpisodeRenames>,
}

impl SyncEpisodes {
    pub fn new(db: Arc<Db>, locks: Arc<MangaSyncLocks>, renames: Arc<dyn EpisodeRenames>) -> Self {
        Self { db, locks, renames }
    }

    /// `await(rawSourceEpisodes, anime, manualFetch, fetchWindow)`. Returns the NEW episodes that should notify and
    /// auto-download. Unlike chapters there is no scanlator exclusion (EC: section 5).
    pub async fn run(
        &self,
        raw: Vec<SEpisode>,
        anime: &Anime,
        opts: EpisodeSyncOptions,
    ) -> Result<Vec<Episode>, EpisodeSyncError> {
        if raw.is_empty() && !opts.is_local {
            return Err(NoEpisodes.into());
        }
        let now_ms = opts.now_ms;
        let now = DateTime::from_timestamp_millis(now_ms).map(|utc| utc.with_timezone(&Local)).unwrap_or_else(Local::now);
        let _guard = self.locks.lock(anime.id).await;

        let stored = self.db.read(|c| {
            Ok(kuro_db::statements_anime::episodes_by_anime_id(c, anime.id)?.iter().map(map_episode).collect::<Vec<_>>())
        })?;

        let source: Vec<Episode> = raw.iter().enumerate().map(|(i, s)| episode_from_source(anime, s, i)).collect();
        // No source hook exists for anime (kuro-source has no prepare_new_episode), so the row is only number-recognised.
        let prepare_row = |row: Episode| -> Episode {
            let number = parse_episode_number(&anime.title, &row.name, Some(row.episode_number));
            Episode { episode_number: number, ..row }
        };
        let plan = plan_sync(PlanInput {
            source,
            db: &stored,
            now_ms,
            added_ms: if anime.favorite { anime.date_added } else { 0 },
            mark_duplicate_as_read: opts.mark_duplicate_as_seen,
            prepare: &prepare_row,
        });

        for (db_row, prepared) in &plan.renames {
            self.renames.rename(anime, db_row, prepared);
        }

        let window = opts.window;
        let fetch_needed = opts.manual || anime.fetch_interval == 0 || anime.next_update < window.0;
        let changed = !plan.is_empty();
        let inserted = self.db.write(|tx| {
            if changed && !plan.removed_ids.is_empty() {
                kuro_db::statements_anime::remove_episodes_with_ids(tx, &plan.removed_ids)?;
            }
            let mut inserted: Vec<Episode> = Vec::new();
            if changed {
                for added in &plan.added {
                    let id = kuro_db::statements_anime::insert_episode(tx, &episode_row(added))?;
                    inserted.push(Episode { id, ..added.clone() });
                }
            }
            for updated in &plan.updated {
                kuro_db::statements_anime::update_episode(tx, &episode_patch(updated))?;
            }
            if changed || fetch_needed {
                let items: Vec<ItemDates> = if anime.fetch_interval < 0 {
                    Vec::new()
                } else {
                    kuro_db::statements_anime::episodes_by_anime_id(tx, anime.id)?
                        .iter()
                        .map(map_episode)
                        .map(|e| ItemDates::from(&e))
                        .collect()
                };
                let stored_interval = Stored {
                    fetch_interval: anime.fetch_interval,
                    next_update: anime.next_update,
                    last_update: anime.last_update,
                };
                let out = to_update(stored_interval, &items, &now, window, now_ms);
                let patch = DbAnimeUpdate {
                    next_update: Some(out.next_update),
                    fetch_interval: Some(i64::from(out.fetch_interval)),
                    ..empty_anime_update(anime.id)
                };
                kuro_db::statements_anime::update_anime(tx, &patch)?;
            }
            if changed {
                let patch = DbAnimeUpdate { last_update: Some(now_ms), ..empty_anime_update(anime.id) };
                kuro_db::statements_anime::update_anime(tx, &patch)?;
            }
            Ok(inserted)
        })?;

        Ok(inserted.into_iter().filter(|e| !plan.changed_or_duplicate_urls.contains(&e.url)).collect())
    }
}

fn empty_anime_update(id: i64) -> DbAnimeUpdate {
    DbAnimeUpdate { id, ..Default::default() }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_merge_never_clears_a_fillermark() {
        let db = Episode { fillermark: true, ..Episode::create() };
        let src = Episode { fillermark: false, ..Episode::create() };
        assert!(Episode::merged(&db, &src).fillermark);
    }

    #[test]
    fn the_scanlator_is_not_trimmed() {
        let anime = Anime { title: "A".into(), ..Anime::create() };
        let ep = episode_from_source(&anime, &SEpisode { scanlator: Some("  Team  ".into()), ..src("/x", "Ep") }, 0);
        assert_eq!(ep.scanlator.as_deref(), Some("  Team  "), "episodes do not trim (unlike chapters)");
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
