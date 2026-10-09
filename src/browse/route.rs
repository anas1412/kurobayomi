//! Browse tab shell and route table (T-40a-001, T-40a-034, T-40a-050, T-40b-030). The six pages keep Aniyomi's order;
//! manga pages are 1, 3 and 5. Routes replace the string screen ids of the manga browse branch.

use kuro_domain::source::browse::BrowseQuery;

/// Which catalogue a page or search belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowseKind {
    Anime,
    Manga,
}

/// The six pages of the Browse tab, in tab order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowsePage {
    AnimeSources = 0,
    MangaSources = 1,
    AnimeExtensions = 2,
    MangaExtensions = 3,
    AnimeMigrate = 4,
    MangaMigrate = 5,
}

impl BrowsePage {
    /// Tab order of the six pages (B§1).
    pub const ALL: [BrowsePage; 6] = [
        BrowsePage::AnimeSources,
        BrowsePage::MangaSources,
        BrowsePage::AnimeExtensions,
        BrowsePage::MangaExtensions,
        BrowsePage::AnimeMigrate,
        BrowsePage::MangaMigrate,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn kind(self) -> BrowseKind {
        match self {
            BrowsePage::AnimeSources | BrowsePage::AnimeExtensions | BrowsePage::AnimeMigrate => BrowseKind::Anime,
            BrowsePage::MangaSources | BrowsePage::MangaExtensions | BrowsePage::MangaMigrate => BrowseKind::Manga,
        }
    }

    pub fn is_extensions(self) -> bool {
        matches!(self, BrowsePage::AnimeExtensions | BrowsePage::MangaExtensions)
    }

    /// The tab label.
    pub fn title(self) -> String {
        use kuro_i18n::MsgKey as K;
        kuro_i18n::tr(match self {
            BrowsePage::AnimeSources => K::label_anime_sources,
            BrowsePage::MangaSources => K::label_manga_sources,
            BrowsePage::AnimeExtensions => K::label_anime_extensions,
            BrowsePage::MangaExtensions => K::label_manga_extensions,
            BrowsePage::AnimeMigrate => K::label_migration_anime,
            BrowsePage::MangaMigrate => K::label_migration_manga,
        })
    }

    /// The extensions page of a kind: `show_extension()` selects manga (3), `show_anime_extension()` anime (2).
    pub fn extensions_of(kind: BrowseKind) -> BrowsePage {
        match kind {
            BrowseKind::Anime => BrowsePage::AnimeExtensions,
            BrowseKind::Manga => BrowsePage::MangaExtensions,
        }
    }
}

/// The manga browse routes. `kind` is carried where a search can belong to either catalogue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BrowseRoute {
    Sources,
    Extensions,
    MigrateSources,
    /// One source's listing. `migrating` selects the migrate search screen instead of the browse screen.
    BrowseSource { source_id: i64, query: BrowseQuery, migrating: bool },
    /// Global search. `source_id` limits the search to one source (migrate), `query` is empty for a blank search.
    GlobalSearch { kind: BrowseKind, source_id: Option<i64>, query: String },
    MigrateEntries { source_id: i64 },
}

impl BrowseRoute {
    /// The screen id the screen registry knows this route by.
    pub fn screen_id(&self) -> &'static str {
        match self {
            BrowseRoute::Sources => "sources",
            BrowseRoute::Extensions => "extensions",
            BrowseRoute::MigrateSources => "migrate_sources",
            BrowseRoute::BrowseSource { migrating: false, .. } => "browse_source",
            BrowseRoute::BrowseSource { migrating: true, .. } => "migrate_source_search",
            BrowseRoute::GlobalSearch { .. } => "global_search",
            BrowseRoute::MigrateEntries { .. } => "migrate_entries",
        }
    }
}

/// Re-selecting the Browse tab opens global search for the visible kind (D-UI-1), with an empty query.
pub fn reselect_route(visible: BrowsePage) -> BrowseRoute {
    BrowseRoute::GlobalSearch { kind: visible.kind(), source_id: None, query: String::new() }
}

/// A blank search still goes out as `""` (B-EC-09); a search is always a new listing (B-EC-08).
pub fn search_listing(query: &str) -> BrowseQuery {
    BrowseQuery::Search(query.to_owned())
}

/// A stub source never starts a listing (B-EC-07).
pub fn start_listing(stub: bool, query: BrowseQuery) -> Option<BrowseQuery> {
    (!stub).then_some(query)
}

/// The tab-switch channel: bounded at one with drop-oldest (E-40 R-40-08). A new request replaces an unread one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageRequest {
    pending: Option<BrowsePage>,
}

impl PageRequest {
    pub fn send(&mut self, page: BrowsePage) {
        self.pending = Some(page);
    }

    pub fn take(&mut self) -> Option<BrowsePage> {
        self.pending.take()
    }
}

/// Search box debounce (B§5.1).
pub const SEARCH_DEBOUNCE_MS: u64 = 250;

/// True once the search text has been still for the debounce period.
pub fn search_due(last_edit_ms: u64, now_ms: u64) -> bool {
    now_ms.saturating_sub(last_edit_ms) >= SEARCH_DEBOUNCE_MS
}

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
    fn six_pages_in_tab_order_and_manga_pages_are_1_3_5() {
        assert_eq!(BrowsePage::ALL.len(), 6);
        for (i, page) in BrowsePage::ALL.iter().enumerate() {
            assert_eq!(page.index(), i);
        }
        let manga: Vec<usize> = BrowsePage::ALL.iter().filter(|p| p.kind() == BrowseKind::Manga).map(|p| p.index()).collect();
        assert_eq!(manga, [1, 3, 5]);
    }

    #[test]
    fn every_tab_has_its_own_title_and_only_two_are_extension_pages() {
        let titles: std::collections::BTreeSet<String> = BrowsePage::ALL.iter().map(|p| p.title()).collect();
        assert_eq!(titles.len(), 6);
        let extensions: Vec<usize> = BrowsePage::ALL.iter().filter(|p| p.is_extensions()).map(|p| p.index()).collect();
        assert_eq!(extensions, [2, 3]);
    }

    #[test]
    fn extension_requests_select_the_page_of_their_kind() {
        assert_eq!(BrowsePage::extensions_of(BrowseKind::Manga).index(), 3);
        assert_eq!(BrowsePage::extensions_of(BrowseKind::Anime).index(), 2);
    }

    #[test]
    fn the_request_channel_keeps_only_the_newest_unread_page() {
        let mut channel = PageRequest::default();
        channel.send(BrowsePage::AnimeExtensions);
        channel.send(BrowsePage::MangaExtensions);
        assert_eq!(channel.take(), Some(BrowsePage::MangaExtensions));
        assert_eq!(channel.take(), None);
    }

    #[test]
    fn reselect_opens_global_search_for_the_visible_kind() {
        assert_eq!(
            reselect_route(BrowsePage::MangaSources),
            BrowseRoute::GlobalSearch { kind: BrowseKind::Manga, source_id: None, query: String::new() }
        );
        assert_eq!(
            reselect_route(BrowsePage::AnimeMigrate),
            BrowseRoute::GlobalSearch { kind: BrowseKind::Anime, source_id: None, query: String::new() }
        );
    }

    #[test]
    fn debounce_fires_at_250_ms_and_not_before() {
        assert!(!search_due(1_000, 1_249));
        assert!(search_due(1_000, 1_250));
        assert!(search_due(1_000, 2_000));
    }

    #[test]
    fn blank_search_sends_empty_text_and_stubs_never_list() {
        assert_eq!(search_listing(""), BrowseQuery::Search(String::new()));
        assert_eq!(start_listing(true, BrowseQuery::Popular), None);
        assert_eq!(start_listing(false, BrowseQuery::Latest), Some(BrowseQuery::Latest));
    }

    #[test]
    fn routes_map_to_their_screen_ids() {
        let browse = BrowseRoute::BrowseSource { source_id: 7, query: BrowseQuery::Popular, migrating: false };
        let migrate = BrowseRoute::BrowseSource { source_id: 7, query: BrowseQuery::Popular, migrating: true };
        assert_eq!(browse.screen_id(), "browse_source");
        assert_eq!(migrate.screen_id(), "migrate_source_search");
        assert_eq!(BrowseRoute::MigrateEntries { source_id: 1 }.screen_id(), "migrate_entries");
        assert_eq!(BrowseRoute::Sources.screen_id(), "sources");
    }
}
