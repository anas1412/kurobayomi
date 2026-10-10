//! The anime migration engine (`migrateAnimeInternal` of `MigrateAnimeDialog.kt`; T-41a-034). Pure over the
//! [`AnimeMigrateSteps`] seam, in the order of behavior 12. The episode fetch runs first and outside the transaction. The
//! episode, category and track writes and the two favourite flips run in one transaction. The download delete and the
//! custom cover and background copies run after the commit, so a failed transaction leaves the files alone.
//!
//! Decision (T-41a-034): the favourite flips stay inside the transaction, ahead of the file steps. The acceptance asks for
//! one transaction with rollback and file work only after the commit, which cannot also keep the Kotlin order of the flips.
//!
//! Deviations from the Kotlin order, all forced by the acceptance: the file steps (5, 7, 8) run after the commit, so they
//! come after the favourite flips (6, 9), which sit inside the transaction. A file step that fails after the commit is a
//! warning, as in the manga engine. `last_second_seen` and `fillermark` are copied with the matched episode (E20).

use kuro_data::anime::{Anime, AnimeUpdate};
use kuro_data::episode::{Episode, EpisodeUpdate};
use kuro_data::track::AnimeTrack;

use crate::browse::migrate_engine::{MigratePlan, StepFuture};
use crate::browse::migrate_flags::{CATEGORIES, CUSTOM_BACKGROUND, CUSTOM_COVER, DELETE_DOWNLOADED, EPISODES};

/// Why an anime migration did not run to the end. Nothing was written for the first three.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnimeMigrateError {
    /// The new anime's source is not installed (behavior 11.3 returns silently; the dialog shows this instead).
    NewSourceMissing,
    /// The episode fetch of the new anime failed (step 1). Nothing was written.
    FetchEpisodes(String),
    /// A database step failed: the transaction rolled back.
    Step(String),
}

impl std::fmt::Display for AnimeMigrateError {
    /// The reason the dialog shows after "Migration failed: ".
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnimeMigrateError::NewSourceMissing => f.write_str("the source of the new anime is not installed"),
            AnimeMigrateError::FetchEpisodes(reason) => write!(f, "the episodes could not be fetched ({reason})"),
            AnimeMigrateError::Step(reason) => f.write_str(reason),
        }
    }
}

/// What a finished migration reports besides success.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnimeMigrateOutcome {
    /// Tracker ids of the bindings an enhanced tracker refused for the new source (T32). They were not moved.
    pub dropped_tracks: Vec<i64>,
    /// File steps that failed after the commit. The migration still stands.
    pub warnings: Vec<String>,
}

/// The database steps. They all run inside the one transaction `AnimeMigrateSteps::transact` opens.
pub trait AnimeMigrateTx {
    fn episodes(&mut self, anime_id: i64) -> Result<Vec<Episode>, String>;
    fn update_episodes(&mut self, updates: &[EpisodeUpdate]) -> Result<(), String>;
    fn category_ids(&mut self, anime_id: i64) -> Result<Vec<i64>, String>;
    fn set_categories(&mut self, anime_id: i64, ids: &[i64]) -> Result<(), String>;
    fn tracks(&mut self, anime_id: i64) -> Result<Vec<AnimeTrack>, String>;
    fn insert_tracks(&mut self, tracks: &[AnimeTrack]) -> Result<(), String>;
    fn update_anime(&mut self, update: &AnimeUpdate) -> Result<(), String>;
}

/// The one body of work of a transaction.
pub type AnimeTxWork<'a> = &'a mut dyn FnMut(&mut dyn AnimeMigrateTx) -> Result<(), String>;

/// What the engine needs of the app.
pub trait AnimeMigrateSteps {
    /// Whether the source of this id is installed.
    fn source_present(&self, source_id: i64) -> bool;
    /// `awaitEpisodesUpdate` of the new anime with episodes fetched: the list is fetched and stored.
    fn fetch_episodes(&self, new: &Anime) -> StepFuture<Result<(), String>>;
    /// Runs `work` in one transaction: it commits when `work` returns `Ok` and rolls back otherwise.
    fn transact(&self, work: AnimeTxWork<'_>) -> Result<(), String>;
    /// The tracker decision for one binding already moved to `new`: `None` drops it (`EnhancedAnimeTracker.migrateTrack`).
    /// A plain tracker keeps the binding and its remote id.
    fn move_track(&self, track: AnimeTrack, old: &Anime, new: &Anime) -> Option<AnimeTrack>;
    fn now_ms(&self) -> i64;
    fn delete_downloads(&self, old: &Anime) -> Result<(), String>;
    fn old_has_custom_cover(&self, old: &Anime) -> bool;
    fn copy_custom_cover(&self, old: &Anime, new: &Anime) -> Result<(), String>;
    fn old_has_custom_background(&self, old: &Anime) -> bool;
    fn copy_custom_background(&self, old: &Anime, new: &Anime) -> Result<(), String>;
}

fn has(plan: MigratePlan, bit: u32) -> bool {
    (plan.flags as u32) & bit != 0
}

fn recognized(episode: &Episode) -> bool {
    episode.episode_number >= 0.0
}

/// The episode rows of the new anime with the old anime's state carried over (step 2). A recognised number takes the date
/// fetched, bookmark, last second seen and filler mark of the old episode with the same number. It is seen when the old
/// anime had seen an episode at or above its number (the max-seen rule). `seen` is never taken from the matched episode.
/// Unrecognised episodes keep their state. Every row is written, as the Kotlin code does.
fn episode_updates(prev: &[Episode], current: &[Episode]) -> Vec<EpisodeUpdate> {
    let max_seen = prev.iter().filter(|e| e.seen).map(|e| e.episode_number).reduce(f64::max);
    current
        .iter()
        .map(|episode| {
            let mut state = episode.clone();
            if recognized(episode) {
                if let Some(old) = prev.iter().find(|p| recognized(p) && p.episode_number == episode.episode_number) {
                    state.date_fetch = old.date_fetch;
                    state.bookmark = old.bookmark;
                    state.last_second_seen = old.last_second_seen;
                    state.fillermark = old.fillermark;
                }
                if max_seen.is_some_and(|max| episode.episode_number <= max) {
                    state.seen = true;
                }
            }
            EpisodeUpdate {
                id: state.id,
                seen: Some(state.seen),
                bookmark: Some(state.bookmark),
                fillermark: Some(state.fillermark),
                last_second_seen: Some(state.last_second_seen),
                date_fetch: Some(state.date_fetch),
                ..EpisodeUpdate::default()
            }
        })
        .collect()
}

/// The database half of the migration (steps 2, 3, 4, 6 and 9).
fn database_steps(
    tx: &mut dyn AnimeMigrateTx,
    steps: &dyn AnimeMigrateSteps,
    old: &Anime,
    new: &Anime,
    plan: MigratePlan,
    now_ms: i64,
    dropped: &mut Vec<i64>,
) -> Result<(), String> {
    if has(plan, EPISODES) {
        let (prev, current) = (tx.episodes(old.id)?, tx.episodes(new.id)?);
        tx.update_episodes(&episode_updates(&prev, &current))?;
    }
    if has(plan, CATEGORIES) {
        let ids = tx.category_ids(old.id)?;
        tx.set_categories(new.id, &ids)?;
    }
    // the tracks move whatever the flags say
    let mut moved = Vec::new();
    for track in tx.tracks(old.id)? {
        let tracker_id = track.tracker_id;
        let bound = AnimeTrack { id: -1, anime_id: new.id, ..track };
        match steps.move_track(bound, old, new) {
            Some(track) => moved.push(track),
            None => dropped.push(tracker_id),
        }
    }
    if !moved.is_empty() {
        tx.insert_tracks(&moved)?;
    }
    if plan.replace {
        tx.update_anime(&AnimeUpdate { favorite: Some(false), date_added: Some(0), ..AnimeUpdate::new(old.id) })?;
    }
    tx.update_anime(&AnimeUpdate {
        favorite: Some(true),
        episode_flags: Some(old.episode_flags),
        viewer_flags: Some(old.viewer_flags),
        date_added: Some(if plan.replace { old.date_added } else { now_ms }),
        ..AnimeUpdate::new(new.id)
    })
}

/// Moves `old` to `new`. A missing new source and a failed episode fetch return before any write.
pub async fn migrate_anime(
    steps: &dyn AnimeMigrateSteps,
    old: &Anime,
    new: &Anime,
    plan: MigratePlan,
) -> Result<AnimeMigrateOutcome, AnimeMigrateError> {
    if !steps.source_present(new.source) {
        return Err(AnimeMigrateError::NewSourceMissing);
    }
    steps.fetch_episodes(new).await.map_err(AnimeMigrateError::FetchEpisodes)?;

    let now_ms = steps.now_ms();
    let mut dropped = Vec::new();
    steps
        .transact(&mut |tx| database_steps(tx, steps, old, new, plan, now_ms, &mut dropped))
        .map_err(AnimeMigrateError::Step)?;

    let mut outcome = AnimeMigrateOutcome { dropped_tracks: dropped, warnings: Vec::new() };
    if has(plan, DELETE_DOWNLOADED) && steps.source_present(old.source) {
        if let Err(error) = steps.delete_downloads(old) {
            tracing::warn!(%error, "the downloads of the old anime were not deleted");
            outcome.warnings.push(error);
        }
    }
    if has(plan, CUSTOM_COVER) && steps.old_has_custom_cover(old) {
        if let Err(error) = steps.copy_custom_cover(old, new) {
            tracing::warn!(%error, "the custom cover was not copied");
            outcome.warnings.push(error);
        }
    }
    if has(plan, CUSTOM_BACKGROUND) && steps.old_has_custom_background(old) {
        if let Err(error) = steps.copy_custom_background(old, new) {
            tracing::warn!(%error, "the custom background was not copied");
            outcome.warnings.push(error);
        }
    }
    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// A scripted app. `log` records what the engine did, in order.
    struct Fake {
        log: RefCell<Vec<String>>,
        installed: Vec<i64>,
        fetch_fails: Option<String>,
        transact_fails: bool,
        downloads_fail: bool,
        old_cover: bool,
        old_background: bool,
        refused_tracker: Option<i64>,
        old_episodes: Vec<Episode>,
        new_episodes: Vec<Episode>,
        old_categories: Vec<i64>,
        old_tracks: Vec<AnimeTrack>,
    }

    impl Fake {
        fn new() -> Self {
            Self {
                log: RefCell::default(),
                installed: vec![1, 2],
                fetch_fails: None,
                transact_fails: false,
                downloads_fail: false,
                old_cover: true,
                old_background: true,
                refused_tracker: None,
                old_episodes: Vec::new(),
                new_episodes: Vec::new(),
                old_categories: vec![5, 6],
                old_tracks: Vec::new(),
            }
        }

        fn note(&self, line: impl Into<String>) {
            self.log.borrow_mut().push(line.into());
        }

        fn lines(&self) -> Vec<String> {
            self.log.borrow().clone()
        }
    }

    struct Tx<'a> {
        fake: &'a Fake,
        writes: RefCell<Vec<String>>,
    }

    impl AnimeMigrateTx for Tx<'_> {
        fn episodes(&mut self, anime_id: i64) -> Result<Vec<Episode>, String> {
            Ok(if anime_id == 10 { self.fake.old_episodes.clone() } else { self.fake.new_episodes.clone() })
        }
        fn update_episodes(&mut self, updates: &[EpisodeUpdate]) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("episodes {}", updates.len()));
            Ok(())
        }
        fn category_ids(&mut self, _: i64) -> Result<Vec<i64>, String> {
            Ok(self.fake.old_categories.clone())
        }
        fn set_categories(&mut self, anime_id: i64, ids: &[i64]) -> Result<(), String> {
            self.writes.borrow_mut().push(format!("categories {anime_id} {ids:?}"));
            Ok(())
        }
        fn tracks(&mut self, _: i64) -> Result<Vec<AnimeTrack>, String> {
            Ok(self.fake.old_tracks.clone())
        }
        fn insert_tracks(&mut self, tracks: &[AnimeTrack]) -> Result<(), String> {
            let rows: Vec<_> = tracks.iter().map(|t| (t.anime_id, t.tracker_id, t.remote_id)).collect();
            self.writes.borrow_mut().push(format!("tracks {rows:?}"));
            Ok(())
        }
        fn update_anime(&mut self, update: &AnimeUpdate) -> Result<(), String> {
            self.writes.borrow_mut().push(format!(
                "anime {} favorite={:?} episode_flags={:?} viewer_flags={:?} date_added={:?}",
                update.id, update.favorite, update.episode_flags, update.viewer_flags, update.date_added
            ));
            Ok(())
        }
    }

    impl AnimeMigrateSteps for Fake {
        fn source_present(&self, source_id: i64) -> bool {
            self.installed.contains(&source_id)
        }
        fn fetch_episodes(&self, _: &Anime) -> StepFuture<Result<(), String>> {
            self.note("fetch");
            let answer = self.fetch_fails.clone().map_or(Ok(()), Err);
            Box::pin(async move { answer })
        }
        fn transact(&self, work: AnimeTxWork<'_>) -> Result<(), String> {
            let mut tx = Tx { fake: self, writes: RefCell::default() };
            let result = work(&mut tx).and_then(|()| if self.transact_fails { Err("disk full".to_owned()) } else { Ok(()) });
            // a failed transaction rolls everything back: only a commit keeps the writes
            if result.is_ok() {
                for write in tx.writes.take() {
                    self.note(format!("write {write}"));
                }
                self.note("commit");
            } else {
                self.note("rollback");
            }
            result
        }
        fn move_track(&self, track: AnimeTrack, _: &Anime, _: &Anime) -> Option<AnimeTrack> {
            (self.refused_tracker != Some(track.tracker_id)).then_some(track)
        }
        fn now_ms(&self) -> i64 {
            777
        }
        fn delete_downloads(&self, _: &Anime) -> Result<(), String> {
            self.note("delete downloads");
            if self.downloads_fail { Err("busy".to_owned()) } else { Ok(()) }
        }
        fn old_has_custom_cover(&self, _: &Anime) -> bool {
            self.old_cover
        }
        fn copy_custom_cover(&self, _: &Anime, _: &Anime) -> Result<(), String> {
            self.note("copy cover");
            Ok(())
        }
        fn old_has_custom_background(&self, _: &Anime) -> bool {
            self.old_background
        }
        fn copy_custom_background(&self, _: &Anime, _: &Anime) -> Result<(), String> {
            self.note("copy background");
            Ok(())
        }
    }

    fn anime(id: i64, source: i64) -> Anime {
        Anime { id, source, episode_flags: 11, viewer_flags: 22, date_added: 100, ..Anime::create() }
    }

    fn episode(id: i64, number: f64, seen: bool) -> Episode {
        Episode { id, episode_number: number, seen, ..Episode::create() }
    }

    fn track(tracker_id: i64, remote_id: i64) -> AnimeTrack {
        AnimeTrack {
            id: 90 + tracker_id,
            anime_id: 10,
            tracker_id,
            remote_id,
            library_id: None,
            title: String::new(),
            last_episode_seen: 0.0,
            total_episodes: 0,
            status: 0,
            score: 0.0,
            remote_url: String::new(),
            start_date: 0,
            finish_date: 0,
            private: false,
        }
    }

    fn run(fake: &Fake, replace: bool, flags: i32) -> Result<AnimeMigrateOutcome, AnimeMigrateError> {
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("runtime");
        runtime.block_on(migrate_anime(fake, &anime(10, 1), &anime(20, 2), MigratePlan { replace, flags }))
    }

    #[test]
    fn a_missing_new_source_returns_before_anything_is_fetched_or_written() {
        let fake = Fake { installed: vec![1], ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(AnimeMigrateError::NewSourceMissing));
        assert!(fake.lines().is_empty(), "{:?}", fake.lines());
    }

    #[test]
    fn a_failed_episode_fetch_stops_before_any_write() {
        let fake = Fake { fetch_fails: Some("offline".to_owned()), ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(AnimeMigrateError::FetchEpisodes("offline".to_owned())));
        assert_eq!(fake.lines(), ["fetch"], "nothing was written");
    }

    #[test]
    fn each_error_has_the_reason_the_dialog_shows() {
        assert_eq!(AnimeMigrateError::NewSourceMissing.to_string(), "the source of the new anime is not installed");
        assert_eq!(AnimeMigrateError::FetchEpisodes("timeout".into()).to_string(), "the episodes could not be fetched (timeout)");
        assert_eq!(AnimeMigrateError::Step("disk full".into()).to_string(), "disk full");
    }

    #[test]
    fn the_steps_run_in_behavior_order_and_the_file_steps_follow_the_commit() {
        let fake = Fake {
            old_episodes: vec![episode(1, 1.0, true)],
            new_episodes: vec![episode(2, 1.0, false)],
            old_tracks: vec![track(4, 77)],
            ..Fake::new()
        };
        assert_eq!(run(&fake, true, i32::MAX), Ok(AnimeMigrateOutcome::default()));
        assert_eq!(
            fake.lines(),
            [
                "fetch",
                "write episodes 1",
                "write categories 20 [5, 6]",
                "write tracks [(20, 4, 77)]",
                "write anime 10 favorite=Some(false) episode_flags=None viewer_flags=None date_added=Some(0)",
                "write anime 20 favorite=Some(true) episode_flags=Some(11) viewer_flags=Some(22) date_added=Some(100)",
                "commit",
                "delete downloads",
                "copy cover",
                "copy background",
            ],
            "behavior 12: one transaction, then the file steps"
        );
    }

    #[test]
    fn zero_flags_still_fetch_move_the_tracks_and_flip_the_favourite_but_write_no_episodes_or_categories() {
        let fake = Fake { old_episodes: vec![episode(1, 1.0, true)], old_tracks: vec![track(4, 77)], ..Fake::new() };
        run(&fake, false, 0).expect("copy with no options");
        let lines = fake.lines();
        assert_eq!(lines.first().map(String::as_str), Some("fetch"), "{lines:?}");
        assert!(lines.contains(&"write tracks [(20, 4, 77)]".to_owned()), "T35: tracks still move");
        assert!(lines.contains(&"write anime 20 favorite=Some(true) episode_flags=Some(11) viewer_flags=Some(22) date_added=Some(777)".to_owned()), "copy stamps now");
        assert!(!lines.iter().any(|l| l.contains("episodes") || l.contains("categories") || l.starts_with("write anime 10")), "{lines:?}");
        assert!(!lines.iter().any(|l| l.contains("delete") || l.contains("copy")), "no file steps without their flags");
    }

    #[test]
    fn a_failed_transaction_rolls_back_and_runs_no_file_step() {
        let fake = Fake { transact_fails: true, ..Fake::new() };
        assert_eq!(run(&fake, true, i32::MAX), Err(AnimeMigrateError::Step("disk full".to_owned())));
        assert_eq!(fake.lines(), ["fetch", "rollback"], "T30: all or nothing, the files are untouched");
    }

    #[test]
    fn an_enhanced_tracker_refusal_drops_the_binding_and_the_outcome_lists_it() {
        let fake = Fake { refused_tracker: Some(3), old_tracks: vec![track(3, 11), track(4, 77)], ..Fake::new() };
        let outcome = run(&fake, true, 0).expect("migrates");
        assert_eq!(outcome.dropped_tracks, [3], "T32: the refused binding is listed");
        assert!(fake.lines().contains(&"write tracks [(20, 4, 77)]".to_owned()), "the plain tracker keeps its remote id: {:?}", fake.lines());
    }

    #[test]
    fn the_seen_rule_and_the_matched_state_carry_over_from_the_old_episodes() {
        let prev = [
            Episode { seen: true, bookmark: true, date_fetch: 500, last_second_seen: 90, fillermark: true, ..episode(1, 1.0, true) },
            episode(2, 2.0, false),
            episode(3, 5.0, true),
        ];
        let current = [episode(11, 1.0, false), episode(12, 2.0, false), episode(13, 4.0, false), episode(14, -1.0, false)];
        let updates = episode_updates(&prev, &current);
        let row = |id: i64| updates.iter().find(|u| u.id == id).expect("row");

        let first = row(11);
        assert_eq!((first.seen, first.bookmark, first.date_fetch), (Some(true), Some(true), Some(500)), "matched by number, seen by max-seen");
        assert_eq!((first.last_second_seen, first.fillermark), (Some(90), Some(true)), "E20: copied with the match");
        assert_eq!(row(12).seen, Some(true), "2.0 is under the max seen 5.0");
        assert_eq!(row(13).seen, Some(true), "4.0 is under 5.0 even with no matching episode");
        let unrecognised = row(14);
        assert_eq!((unrecognised.seen, unrecognised.bookmark, unrecognised.date_fetch), (Some(false), Some(false), Some(0)), "unrecognised numbers are copied unchanged");
    }

    #[test]
    fn with_nothing_seen_no_episode_is_marked_seen() {
        let updates = episode_updates(&[episode(1, 1.0, false)], &[episode(2, 1.0, false)]);
        assert_eq!(updates[0].seen, Some(false), "no max-seen when nothing was seen");
    }

    #[test]
    fn the_date_added_rule_keeps_the_old_one_only_in_replace_mode() {
        let replace = Fake::new();
        run(&replace, true, 0).expect("migrates");
        assert!(replace.lines().contains(&"write anime 20 favorite=Some(true) episode_flags=Some(11) viewer_flags=Some(22) date_added=Some(100)".to_owned()));
        let copy = Fake::new();
        run(&copy, false, 0).expect("copies");
        assert!(!copy.lines().iter().any(|l| l.starts_with("write anime 10")), "copy never unfavourites the old anime");
    }

    #[test]
    fn a_failing_file_step_is_a_warning_and_the_next_file_steps_still_run() {
        let fake = Fake { downloads_fail: true, ..Fake::new() };
        let outcome = run(&fake, true, i32::MAX).expect("the migration stands");
        assert_eq!(outcome.warnings, ["busy"]);
        assert!(fake.lines().contains(&"copy cover".to_owned()) && fake.lines().contains(&"copy background".to_owned()));
    }

    #[test]
    fn covers_and_backgrounds_are_copied_only_when_the_flag_is_set_and_the_old_anime_has_one() {
        let no_cover = Fake { old_cover: false, ..Fake::new() };
        run(&no_cover, true, i32::MAX).expect("migrates");
        assert!(!no_cover.lines().contains(&"copy cover".to_owned()));
        assert!(no_cover.lines().contains(&"copy background".to_owned()));
        let unflagged = Fake::new();
        run(&unflagged, true, 0).expect("migrates");
        assert!(!unflagged.lines().iter().any(|l| l.starts_with("copy") || l.starts_with("delete")));
    }
}
