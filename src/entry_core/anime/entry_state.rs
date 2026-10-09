//! First-open refresh decision and load state of the anime entry (T-39b-001). Upstream: `AnimeScreenModel.kt` (init and
//! `needRefresh*`), behaviour 2.3. The load sequence itself is `model::AnimeEntryModel::load`; this module holds the
//! decisions and the state flags that the sequence sets.

/// The load state of the page. Dispatches are ignored while `Loading`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LoadState {
    Loading,
    Success,
    Error,
}

/// Which parts of the entry refresh from the source on open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RefreshPlan {
    pub info: bool,
    pub episodes: bool,
    pub seasons: bool,
}

impl RefreshPlan {
    /// Some part of the entry must refresh from the source (`isRefreshingData` on the first state).
    pub fn any(self) -> bool {
        self.info || self.episodes || self.seasons
    }
}

/// `needRefreshInfo = !initialized`, `needRefreshEpisode = episodes.isEmpty() && Episodes`,
/// `needRefreshSeason = seasons.isEmpty() && Seasons`. `episode_list` and `season_list` say whether the entry's fetch
/// type is `Episodes` or `Seasons`.
pub fn refresh_on_open(
    initialized: bool,
    episodes_empty: bool,
    episode_list: bool,
    seasons_empty: bool,
    season_list: bool,
) -> RefreshPlan {
    RefreshPlan {
        info: !initialized,
        episodes: episodes_empty && episode_list,
        seasons: seasons_empty && season_list,
    }
}

/// The load and refresh flags of one open page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntryFlow {
    pub load: LoadState,
    /// True from the start of a refresh until it completes. It also goes false after an error.
    pub refreshing: bool,
}

impl Default for EntryFlow {
    fn default() -> Self {
        Self {
            load: LoadState::Loading,
            refreshing: false,
        }
    }
}

impl EntryFlow {
    /// `Loading` ignores dispatches (`T-MDL-5`).
    pub fn accepts_dispatch(&self) -> bool {
        self.load != LoadState::Loading
    }

    /// The load finished, with or without data.
    pub fn loaded(&mut self, ok: bool) {
        self.load = if ok { LoadState::Success } else { LoadState::Error };
    }

    pub fn begin_refresh(&mut self) {
        self.refreshing = true;
    }

    /// Ends the refresh on success and on error alike (`T-MDL-3`).
    pub fn end_refresh(&mut self) {
        self.refreshing = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_refreshes_only_before_the_first_open() {
        assert!(refresh_on_open(false, false, false, false, false).info);
        assert!(!refresh_on_open(true, false, false, false, false).info);
    }

    #[test]
    fn lists_refresh_only_when_empty_and_of_their_fetch_type() {
        let plan = refresh_on_open(true, true, true, true, false);
        assert!(plan.episodes);
        assert!(!plan.seasons);
        let plan = refresh_on_open(true, false, true, true, true);
        assert!(!plan.episodes);
        assert!(plan.seasons);
    }

    #[test]
    fn a_plan_is_active_when_any_part_refreshes() {
        assert!(!refresh_on_open(true, false, true, false, false).any());
        assert!(refresh_on_open(false, false, true, false, false).any());
        assert!(refresh_on_open(true, true, true, false, false).any());
        assert!(refresh_on_open(true, false, false, true, true).any());
    }

    #[test]
    fn loading_ignores_dispatches_until_loaded() {
        let mut flow = EntryFlow::default();
        assert!(!flow.accepts_dispatch());
        flow.loaded(true);
        assert!(flow.accepts_dispatch());
        assert_eq!(flow.load, LoadState::Success);
    }

    #[test]
    fn refreshing_ends_on_error_too() {
        let mut flow = EntryFlow::default();
        flow.begin_refresh();
        assert!(flow.refreshing);
        flow.loaded(false);
        flow.end_refresh();
        assert!(!flow.refreshing);
        assert_eq!(flow.load, LoadState::Error);
    }
}
