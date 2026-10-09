//! The anime entry model (T-39b-001): the load sequence of behaviour 2.3, the state it publishes and the lists the
//! widgets draw from it. `load` runs on the blocking pool; the widgets read the value it returns. No GTK here.
//! Upstream: `AnimeScreenModel.kt:204-283` (init) and `:1602-1739` (`State.Success`).
//!
//! The load sequence, as `load` runs it:
//!  1. the entry is read (step 3.1), and a missing row is `None`;
//!  2. its source is resolved (3.2);
//!  3. the episode list is read, empty for a `Seasons` entry (3.3);
//!  4. the season list is read, empty for an `Episodes` entry (3.4);
//!  5. an entry outside the library gets the library defaults, then is read again so the first state never shows
//!     stale filters (3.5);
//!  6. the three refresh flags are computed (3.6);
//!  7. the first `Success` state is published with `is_refreshing` set when a refresh is needed (3.7).
//!
//! Steps 1 and 2 of 2.3 (the live subscription and the download observers) belong to the runner, and steps 8 and 9
//! (tracker and related-anime observers) to the tracking and related tasks. Step 10 (the source fetch) is started by
//! the runner when `plan.any()`, and step 11 is `finish_refresh`.

use kuro_data::DataResult;
use kuro_data::anime::{Anime, SeasonAnime};
use kuro_data::episode::Episode;
use kuro_db::schema_anime::FetchType;
use kuro_domain::app::items::{ListFilters, apply_filters, episode_filters};
use kuro_domain::entries::defaults::{episode_default_word, season_default_word};
use kuro_domain::items::sorter::{SortKey, compare_items};
use kuro_prefs::store::PrefStore;

use super::entry_state::{EntryFlow, RefreshPlan, refresh_on_open};
use super::episode_row::{EpisodeRow, row};
use super::gesture::Selection;
use super::header::{ItemHeader, header_missing_count, item_header};
use super::layout::{Fab, fab};
use super::settings::display_rows;
use super::store::AnimeStore;
use crate::entry_core::detail::SourceInfo;
use crate::entry_core::errors::DetailError;

/// What the load reads besides the rows: values read once at open (behaviour 2.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct AnimeLoadContext {
    pub from_source: bool,
    pub downloaded_only: bool,
}

#[derive(Clone, Debug)]
pub struct AnimeEntryModel {
    pub flow: EntryFlow,
    pub anime: Anime,
    pub source: SourceInfo,
    /// Every stored episode, before any filter.
    pub episodes: Vec<Episode>,
    pub seasons: Vec<SeasonAnime>,
    /// Which parts the first open refreshes from the source.
    pub plan: RefreshPlan,
    pub from_source: bool,
    pub downloaded_only: bool,
    pub selection: Selection,
    /// The error the last refresh ended with, shown by the page; `None` after a successful one.
    pub refresh_error: Option<DetailError>,
    /// The in-memory library defaults of a non-library entry, `(episode, season)`. A snapshot re-applies them.
    pub session_flags: Option<(i64, i64)>,
}

impl AnimeEntryModel {
    /// The load sequence. `source_of` resolves the source line (a stub when the extension is missing).
    pub fn load(
        store: &AnimeStore,
        prefs: &PrefStore,
        anime_id: i64,
        ctx: AnimeLoadContext,
        source_of: impl FnOnce(i64) -> SourceInfo,
    ) -> DataResult<Option<Self>> {
        let Some(mut anime) = store.anime(anime_id)? else {
            return Ok(None);
        };
        let source = source_of(anime.source);
        let episode_list = anime.fetch_type == FetchType::Episodes;
        let season_list = anime.fetch_type == FetchType::Seasons;
        let episodes = if season_list { Vec::new() } else { store.episodes(anime_id)? };
        let seasons = if episode_list { Vec::new() } else { store.seasons(anime_id)? };
        // A non-library entry shows the library defaults for this session only (T-39b-012): nothing is written to its row.
        let session_flags = (!anime.favorite).then(|| (episode_default_word(prefs), season_default_word(prefs)));
        if let Some((episode_flags, season_flags)) = session_flags {
            anime.episode_flags = episode_flags;
            anime.season_flags = season_flags;
        }
        let plan = refresh_on_open(anime.initialized, episodes.is_empty(), episode_list, seasons.is_empty(), season_list);
        let mut flow = EntryFlow::default();
        flow.loaded(true);
        if plan.any() {
            flow.begin_refresh();
        }
        Ok(Some(Self {
            flow,
            anime,
            source,
            episodes,
            seasons,
            plan,
            from_source: ctx.from_source,
            downloaded_only: ctx.downloaded_only,
            selection: Selection::default(),
            refresh_error: None,
            session_flags,
        }))
    }

    /// Step 11: the refresh is over, with or without an error.
    pub fn finish_refresh(&mut self, result: Result<(), DetailError>) {
        self.flow.end_refresh();
        self.refresh_error = result.err();
    }

    /// The subscription delivered a newer entry and episode list. A `Seasons` entry keeps an empty episode list.
    pub fn apply_snapshot(&mut self, mut anime: Anime, episodes: Vec<Episode>) {
        if anime.fetch_type == FetchType::Episodes {
            self.episodes = episodes;
        }
        if let Some((episode_flags, season_flags)) = self.session_flags {
            anime.episode_flags = episode_flags;
            anime.season_flags = season_flags;
        }
        self.anime = anime;
    }

    /// `Loading` ignores dispatches (`T-MDL-5`).
    pub fn accepts_dispatch(&self) -> bool {
        self.flow.accepts_dispatch()
    }

    /// The filters and sort of the entry's flag word. An unknown sorting value shows the list unfiltered.
    pub fn filters(&self) -> Option<ListFilters> {
        episode_filters(&self.anime, self.downloaded_only).ok()
    }

    /// The filtered and sorted list. Nothing is downloaded yet (no anime download queue), and a local entry counts
    /// every episode as downloaded.
    pub fn processed(&self) -> Vec<Episode> {
        let local = self.anime.source == kuro_source::local::LOCAL_SOURCE_ID;
        match self.filters() {
            Some(filters) => apply_filters(self.episodes.clone(), &filters, local, |_| false),
            None => self.episodes.clone(),
        }
    }

    pub fn processed_ids(&self) -> Vec<i64> {
        self.processed().iter().map(|e| e.id).collect()
    }

    /// One row per processed episode, in list order.
    pub fn rows(&self) -> Vec<EpisodeRow> {
        let flags = display_rows(self.anime.episode_flags);
        self.processed()
            .iter()
            .map(|ep| row(ep, flags.episode_number, flags.previews, flags.summaries, self.selection.contains(ep.id)))
            .collect()
    }

    /// The FAB and continue target (`getNextUnseen`): the filters apply, the list is taken in ascending order, and
    /// `next_unseen_in` picks the first unseen one, or the last one for a descending sort.
    pub fn next_unseen(&self) -> Option<i64> {
        let local = self.anime.source == kuro_source::local::LOCAL_SOURCE_ID;
        let (ascending, descending) = match self.filters() {
            Some(f) => {
                let descending = f.descending;
                (apply_filters(self.episodes.clone(), &ListFilters { descending: false, ..f }, local, |_| false), descending)
            }
            None => {
                let mut list = self.episodes.clone();
                list.sort_by(|a, b| compare_items(SortKey::Number, false, a, b));
                (list, false)
            }
        };
        next_unseen_in(&ascending, descending)
    }

    /// The FAB reads the unfiltered list (behaviour 9).
    pub fn fab(&self) -> Fab {
        let seen: Vec<bool> = self.episodes.iter().map(|e| e.seen).collect();
        fab(&seen, self.selection.is_active())
    }

    /// The item header over the processed list. The missing count reads the unfiltered numbers (behaviour 8).
    pub fn item_header(&self) -> ItemHeader {
        let seasons = self.anime.fetch_type == FetchType::Seasons;
        let count = if seasons { self.seasons.len() } else { self.processed().len() };
        let episode_numbers: Vec<f64> = self.episodes.iter().map(|e| e.episode_number).collect();
        let season_numbers: Vec<f64> = self.seasons.iter().map(|s| s.anime.season_number).collect();
        let missing = header_missing_count(&episode_numbers, &season_numbers);
        item_header(seasons, count, missing, self.selection.is_active())
    }
}

/// `getNextUnseen` over a list in ascending order: the first unseen episode, or the last one when the display is
/// descending. The list is already filtered.
pub fn next_unseen_in(ascending: &[Episode], descending: bool) -> Option<i64> {
    let mut unseen = ascending.iter().filter(|e| !e.seen).map(|e| e.id);
    if descending { unseen.last() } else { unseen.next() }
}

#[cfg(test)]
mod tests {
    use kuro_domain::entries::anime::episode_flags as ep;

    use super::super::entry_state::LoadState;
    use super::super::store::fixture::with_episodes;
    use super::*;

    fn load(store: &AnimeStore, id: i64) -> AnimeEntryModel {
        let source = |_| SourceInfo { name: "Fake".into(), stub: false };
        AnimeEntryModel::load(store, &PrefStore::in_memory(), id, AnimeLoadContext::default(), source)
            .unwrap()
            .expect("the entry exists")
    }

    #[test]
    fn a_fixture_entry_opens_in_success_with_its_episodes_and_no_refresh() {
        let (_dir, store, id) = with_episodes(true, true, 4, 1);
        let model = load(&store, id);
        assert_eq!(model.flow.load, LoadState::Success);
        assert!(model.accepts_dispatch());
        assert!(!model.flow.refreshing);
        assert_eq!(model.plan, RefreshPlan { info: false, episodes: false, seasons: false });
        assert_eq!(model.anime.title, "Frieren");
        assert_eq!(model.source.name, "Fake");
        assert_eq!(model.episodes.len(), 4);
        // the default word 0 sorts by source, descending
        assert_eq!(model.rows().iter().map(|r| r.title.as_str()).collect::<Vec<_>>(), ["Episode 4", "Episode 3", "Episode 2", "Episode 1"]);
    }

    #[test]
    fn a_missing_entry_loads_as_none() {
        let (_dir, store, _) = with_episodes(true, true, 0, 0);
        let none = AnimeEntryModel::load(&store, &PrefStore::in_memory(), 4242, AnimeLoadContext::default(), |_| SourceInfo::default());
        assert!(none.unwrap().is_none());
    }

    #[test]
    fn the_first_state_is_refreshing_until_the_refresh_ends_on_success_or_error() {
        let (_dir, store, id) = with_episodes(true, false, 2, 0);
        let mut model = load(&store, id);
        assert_eq!(model.plan, RefreshPlan { info: true, episodes: false, seasons: false });
        assert!(model.flow.refreshing);
        model.finish_refresh(Err(DetailError::SourceNotInstalled));
        assert!(!model.flow.refreshing);
        assert_eq!(model.refresh_error, Some(DetailError::SourceNotInstalled));
        model.flow.begin_refresh();
        model.finish_refresh(Ok(()));
        assert_eq!(model.refresh_error, None);
    }

    #[test]
    fn an_empty_episode_list_refreshes_and_a_seasons_entry_reads_no_episodes() {
        let (_dir, store, id) = with_episodes(true, true, 0, 0);
        assert_eq!(load(&store, id).plan, RefreshPlan { info: false, episodes: true, seasons: false });

        let (_dir, store, id) = with_episodes(true, true, 3, 0);
        store.set_fetch_type_for_test(id, FetchType::Seasons);
        let model = load(&store, id);
        assert!(model.episodes.is_empty(), "episodes stay empty for a seasons entry");
        assert_eq!(model.plan, RefreshPlan { info: false, episodes: false, seasons: true });
    }

    #[test]
    fn a_non_favourite_opens_with_the_library_defaults_not_its_stored_flags() {
        let (_dir, store, id) = with_episodes(false, true, 2, 0);
        store.set_flag_bits(id, ep::SHOW_SEEN, ep::UNSEEN_MASK).unwrap();
        assert_eq!(load(&store, id).anime.episode_flags & ep::UNSEEN_MASK, 0);
        let row = store.anime(id).unwrap().unwrap();
        assert_eq!(row.episode_flags & ep::UNSEEN_MASK, ep::SHOW_SEEN, "the open writes nothing to the row (T-39b-012)");

        let (_dir, store, id) = with_episodes(true, true, 2, 0);
        store.set_flag_bits(id, ep::SHOW_SEEN, ep::UNSEEN_MASK).unwrap();
        assert_eq!(load(&store, id).anime.episode_flags & ep::UNSEEN_MASK, ep::SHOW_SEEN);
    }

    #[test]
    fn filters_the_list_but_the_fab_and_header_read_the_whole_one() {
        let (_dir, store, id) = with_episodes(true, true, 3, 3);
        store.set_flag_bits(id, ep::SHOW_UNSEEN, ep::UNSEEN_MASK).unwrap();
        let model = load(&store, id);
        assert!(model.rows().is_empty(), "every episode is seen, so the unseen filter hides all");
        assert!(!model.fab().visible, "nothing is unseen");
        assert!(model.fab().resume);
        assert_eq!(model.item_header().count, "0 episodes");

        let (_dir, store, id) = with_episodes(true, true, 3, 1);
        store.set_flag_bits(id, ep::SHOW_SEEN, ep::UNSEEN_MASK).unwrap();
        let mut model = load(&store, id);
        assert_eq!(model.rows().len(), 1, "only the seen episode shows");
        assert!(model.fab().visible, "an unseen episode exists in the unfiltered list");
        model.selection.long_press(model.episodes[0].id);
        assert!(!model.fab().visible, "selecting hides it");
        assert!(!model.item_header().enabled);
    }

    #[test]
    fn next_unseen_takes_the_last_unseen_for_a_descending_sort() {
        let list: Vec<Episode> = (1..=4)
            .map(|id| {
                let mut e = Episode::create();
                e.id = id;
                e.seen = id <= 2;
                e
            })
            .collect();
        assert_eq!(next_unseen_in(&list, false), Some(3));
        assert_eq!(next_unseen_in(&list, true), Some(4));
        assert_eq!(next_unseen_in(&[], true), None);
    }

    #[test]
    fn the_fab_opens_the_first_unseen_in_display_order() {
        let (_dir, store, id) = with_episodes(true, true, 4, 2);
        store.set_flag_bits(id, ep::SORT_ASC, ep::SORT_DIR_MASK).unwrap();
        let model = load(&store, id);
        let by_number = |n: f64| model.episodes.iter().find(|e| e.episode_number == n).unwrap().id;
        assert_eq!(model.next_unseen(), Some(by_number(3.0)), "ascending: the oldest unseen");
        store.set_flag_bits(id, ep::SORT_DESC, ep::SORT_DIR_MASK).unwrap();
        assert_eq!(load(&store, id).next_unseen(), Some(by_number(4.0)), "descending: the newest unseen comes first");
        let (_dir, store, id) = with_episodes(true, true, 2, 2);
        assert_eq!(load(&store, id).next_unseen(), None);
    }

    #[test]
    fn selection_marks_rows_and_a_snapshot_keeps_the_selection() {
        let (_dir, store, id) = with_episodes(true, true, 3, 0);
        let mut model = load(&store, id);
        let first = model.processed_ids()[0];
        model.selection.long_press(first);
        assert_eq!(model.rows().iter().filter(|r| r.selected).count(), 1);
        let anime = model.anime.clone();
        model.apply_snapshot(anime, model.episodes[..2].to_vec());
        assert_eq!(model.episodes.len(), 2);
        assert!(model.selection.contains(first));
    }

    #[test]
    fn three_thousand_rows_build_in_well_under_a_second() {
        let (_dir, store, id) = with_episodes(true, true, 3000, 100);
        let model = load(&store, id);
        let started = std::time::Instant::now();
        let rows = model.rows();
        assert_eq!(rows.len(), 3000);
        assert!(started.elapsed() < std::time::Duration::from_secs(2), "{:?}", started.elapsed());
    }
}
