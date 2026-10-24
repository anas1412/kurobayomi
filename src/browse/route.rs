//! Browse tab shell and route table (T-40a-001, T-40a-034, T-40a-050, T-40b-030). The pages keep Aniyomi's order;
//! manga pages are 1 and 3. Routes replace the string screen ids of the manga browse branch.

use kuro_domain::source::browse::BrowseQuery;
use kuro_platform::kind::Kind;

/// Which catalogue a page or search belongs to: the app's one entry kind. Kept for callers outside the browse lane.
pub use kuro_platform::kind::Kind as BrowseKind;

/// The page ids of the Browse tab, in tab order. The tab shows only the two source pages (anime and manga); the migrate
/// pages have no place on the tab either.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowsePage {
    AnimeSources = 0,
    MangaSources = 1,
    AnimeMigrate = 2,
    MangaMigrate = 3,
}

impl BrowsePage {
    /// Tab order of the pages (B§1).
    pub const ALL: [BrowsePage; 4] = [BrowsePage::AnimeSources, BrowsePage::MangaSources, BrowsePage::AnimeMigrate, BrowsePage::MangaMigrate];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn kind(self) -> Kind {
        match self {
            BrowsePage::AnimeSources | BrowsePage::AnimeMigrate => Kind::Anime,
            BrowsePage::MangaSources | BrowsePage::MangaMigrate => Kind::Manga,
        }
    }

    #[cfg(test)]
    /// The tab label.
    pub fn title(self) -> String {
        use kuro_i18n::MsgKey as K;
        kuro_i18n::tr(match self {
            BrowsePage::AnimeSources => K::label_anime_sources,
            BrowsePage::MangaSources => K::label_manga_sources,
            BrowsePage::AnimeMigrate => K::label_migration_anime,
            BrowsePage::MangaMigrate => K::label_migration_manga,
        })
    }
}

#[allow(dead_code)] // kept for the shape of the type; not built or read yet
/// The browse routes. `kind` says which catalogue the page belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowseRoute {
    Sources,
    MigrateSources,
    /// One source's listing. `migrating` selects the migrate search screen instead of the browse screen.
    BrowseSource { kind: Kind, source_id: i64, query: BrowseQuery, migrating: bool },
    /// Global search. `source_id` limits the search to one source (migrate), `query` is empty for a blank search.
    GlobalSearch { kind: Kind, source_id: Option<i64>, query: String },
    /// The favourites of one source of `kind` that a migration can start from.
    MigrateEntries { kind: Kind, source_id: i64 },
}

impl BrowseRoute {
    /// The screen id the screen registry knows this route by.
    pub fn screen_id(&self) -> &'static str {
        match self {
            BrowseRoute::Sources => "sources",
            BrowseRoute::MigrateSources => "migrate_sources",
            BrowseRoute::BrowseSource { migrating: false, .. } => "browse_source",
            BrowseRoute::BrowseSource { migrating: true, .. } => "migrate_source_search",
            BrowseRoute::GlobalSearch { .. } => "global_search",
            BrowseRoute::MigrateEntries { .. } => "migrate_entries",
        }
    }
}

/// A blank search still goes out as `""` (B-EC-09); a search is always a new listing (B-EC-08).
pub fn search_listing(query: &str) -> BrowseQuery {
    BrowseQuery::Search(query.to_owned())
}

#[cfg(test)]
/// Search box debounce (B§5.1).
pub const SEARCH_DEBOUNCE_MS: u64 = 250;

#[cfg(test)]
/// True once the search text has been still for the debounce period.
pub fn search_due(last_edit_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_edit_ms) >= SEARCH_DEBOUNCE_MS
}

#[cfg(test)]
/// Milliseconds until the debounce is due (0 once it is), measured from the last edit.
pub fn search_wait_ms(last_edit_ms: u64, now_ms: u64) -> u64 {
    SEARCH_DEBOUNCE_MS.saturating_sub(now_ms.saturating_sub(last_edit_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wait_runs_down_to_zero_exactly_when_the_search_is_due() {
        assert_eq!(search_wait_ms(1_000, 1_000), 250);
        assert_eq!(search_wait_ms(1_000, 1_100), 150);
        assert_eq!(search_wait_ms(1_000, 1_250), 0);
        for now in [1_000, 1_100, 1_249, 1_250, 2_000] {
            assert_eq!(search_wait_ms(1_000, now) == 0, search_due(1_000, now));
        }
    }

    #[test]
    fn four_pages_in_tab_order_and_manga_pages_are_1_and_3() {
        assert_eq!(BrowsePage::ALL.len(), 4);
        for (i, page) in BrowsePage::ALL.iter().enumerate() {
            assert_eq!(page.index(), i);
        }
        let manga: Vec<usize> = BrowsePage::ALL.iter().filter(|p| p.kind() == Kind::Manga).map(|p| p.index()).collect();
        assert_eq!(manga, [1, 3]);
    }

    #[test]
    fn every_tab_has_its_own_title() {
        let titles: std::collections::BTreeSet<String> = BrowsePage::ALL.iter().map(|p| p.title()).collect();
        assert_eq!(titles.len(), 4);
    }

    #[test]
    fn debounce_fires_at_250_ms_and_not_before() {
        assert!(!search_due(1_000, 1_249));
        assert!(search_due(1_000, 1_250));
        assert!(search_due(1_000, 2_000));
    }

    #[test]
    fn routes_map_to_their_screen_ids() {
        let browse = BrowseRoute::BrowseSource { kind: Kind::Manga, source_id: 7, query: BrowseQuery::Popular, migrating: false };
        let migrate = BrowseRoute::BrowseSource { kind: Kind::Manga, source_id: 7, query: BrowseQuery::Popular, migrating: true };
        assert_eq!(browse.screen_id(), "browse_source");
        assert_eq!(migrate.screen_id(), "migrate_source_search");
        assert_eq!(BrowseRoute::MigrateEntries { kind: Kind::Manga, source_id: 1 }.screen_id(), "migrate_entries");
        let anime = BrowseRoute::BrowseSource { kind: Kind::Anime, source_id: 1, query: BrowseQuery::Popular, migrating: false };
        assert_eq!(anime.screen_id(), "browse_source", "the anime listing shares the screen");
        assert_eq!(BrowseRoute::Sources.screen_id(), "sources");
    }
}
