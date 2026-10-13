//! Season tiles (T-39a-032, T-39a-033) and the season grid columns (T-39a-034). Upstream: `AnimeSeasonListItem.kt`,
//! `AnimeSeasonItem.kt` (the `-1` defaults that hide a badge) and `AnimeScreen.kt` (grid cells). The rules are pure; the
//! grid widget is `season_grid`.

use kuro_data::entry::{Entry, SeasonEntry};
use kuro_data::item::Item;
use kuro_db::codec::FetchType;
use kuro_domain::entries::flags::EntryFlags;
use kuro_domain::entries::season_display::SeasonDisplayMode;
use kuro_i18n::{Arg, MsgKey, trf};

use super::format::format_episode_number;

/// The width one grid column needs when the size is automatic (`max(1, width / 128)`).
pub const AUTO_COLUMN_WIDTH: i32 = 128;

/// The largest column count the size setting allows (the slider runs 0..=10).
pub const MAX_GRID_SIZE: i64 = 10;

/// The title a season shows. Number mode is "Season {n}" from the season number, otherwise the source title.
pub fn tile_title(number_mode: bool, season_number: f64, source_title: &str) -> String {
    if number_mode {
        trf(MsgKey::display_mode_season, &[("arg1", Arg::Str(&format_episode_number(season_number)))])
    } else {
        source_title.to_owned()
    }
}

/// The columns of the grid. List mode is one full-width column. Size 0 is automatic, `max(1, width / 128)`, and any
/// other size is that many columns (clamped to 0..=10).
pub fn grid_columns(width: i32, size: i64, list_mode: bool) -> u32 {
    if list_mode {
        return 1;
    }
    match size.clamp(0, MAX_GRID_SIZE) {
        0 => (width / AUTO_COLUMN_WIDTH).max(1) as u32,
        n => n as u32,
    }
}

/// Which of the parent entry's season overlays are on (season flags bits 22-26, set means on, T-F5).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overlays {
    pub downloaded: bool,
    pub unseen: bool,
    pub local: bool,
    pub language: bool,
    pub continue_watching: bool,
}

impl Overlays {
    pub fn of(parent: &Entry) -> Self {
        let season = parent.season();
        Self {
            downloaded: season.downloaded_overlay(),
            unseen: season.unseen_overlay(),
            local: season.local_overlay(),
            language: season.lang_overlay(),
            continue_watching: season.continue_overlay(),
        }
    }
}

/// One season as the list shows it, with the overlays already applied (`AnimeSeasonItem`). A count of `-1` means hidden.
#[derive(Clone, Debug, PartialEq)]
pub struct SeasonItem {
    pub id: i64,
    /// The source title; the number-mode title is built from `season_number` when shown.
    pub source_title: String,
    pub season_number: f64,
    pub download_count: i64,
    pub unseen_count: i64,
    pub is_local: bool,
    pub source_language: String,
    pub show_continue: bool,
}

/// The inputs of one season that are not part of the stored season row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeasonFacts {
    pub download_count: i64,
    pub is_local: bool,
    pub source_language: String,
    /// The season fetches episodes (not a nested list); the continue overlay needs this.
    pub fetches_episodes: bool,
}

/// What the runner reads for one season besides its row: its download count and its source language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeasonExtra {
    pub id: i64,
    pub download_count: i64,
    pub source_language: String,
}

/// The facts of one season: the runner's extras when they have arrived, else none (a season with no extras shows
/// no download count and no language, which the overlays then hide). Only a season that lists episodes can continue.
pub fn season_facts(season: &SeasonEntry, extra: Option<&SeasonExtra>, is_local: bool) -> SeasonFacts {
    SeasonFacts {
        download_count: extra.map_or(0, |e| e.download_count),
        is_local,
        source_language: extra.map(|e| e.source_language.clone()).unwrap_or_default(),
        fetches_episodes: season.entry.fetch_type() == FetchType::Episodes,
    }
}

/// `applySeasonFilters` overlay mapping: each overlay gates its own fact, and the continue overlay needs an unseen
/// episode and an episode-fetching season.
pub fn season_item(season: &SeasonEntry, overlays: Overlays, facts: &SeasonFacts) -> SeasonItem {
    let unseen_count = season.pending();
    SeasonItem {
        id: season.entry.id,
        source_title: season.entry.title.clone(),
        season_number: season.entry.season_number(),
        download_count: if overlays.downloaded { facts.download_count } else { -1 },
        unseen_count: if overlays.unseen { unseen_count } else { -1 },
        is_local: overlays.local && facts.is_local,
        source_language: if overlays.language { facts.source_language.clone() } else { String::new() },
        show_continue: overlays.continue_watching && unseen_count > 0 && facts.fetches_episodes,
    }
}

/// What the continue button of a season plays: the unseen episode with the lowest number, the same one the page's FAB
/// picks for an entry with no filters (`getNextUnseen`).
pub fn continue_episode(episodes: &[Item]) -> Option<&Item> {
    episodes.iter().filter(|e| !e.done).min_by(|a, b| a.number.total_cmp(&b.number))
}

/// The language badge of a season: a folder for a local season, else the uppercase source language when one is set.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LanguageBadge {
    None,
    Local,
    Text(String),
}

pub fn language_badge(is_local: bool, source_language: &str) -> LanguageBadge {
    if is_local {
        LanguageBadge::Local
    } else if source_language.is_empty() {
        LanguageBadge::None
    } else {
        LanguageBadge::Text(source_language.to_uppercase())
    }
}

/// A count badge shows only above zero (`DownloadsBadge` and `UnviewedBadge`), so the `-1` default is hidden too.
pub fn count_badge(count: i64) -> Option<i64> {
    (count > 0).then_some(count)
}

/// The three badges of a tile. Grid tiles put downloads and unseen on the cover's start edge and the language on its end
/// edge; list rows put all of them in one slot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badges {
    pub downloads: Option<i64>,
    pub unseen: Option<i64>,
    pub language: LanguageBadge,
}

/// The text of the language badge: "Local" for a local season, else the uppercase code.
fn language_text(language: &LanguageBadge) -> Option<String> {
    match language {
        LanguageBadge::None => None,
        LanguageBadge::Local => Some("Local".to_owned()),
        LanguageBadge::Text(code) => Some(code.clone()),
    }
}

/// The badges of a grid tile by cover edge: downloads and unseen on the start edge, the language on the end edge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EdgeBadges {
    pub start: Vec<String>,
    pub end: Option<String>,
}

pub fn edge_badges(badges: &Badges) -> EdgeBadges {
    EdgeBadges {
        start: [badges.downloads, badges.unseen].into_iter().flatten().map(|n| n.to_string()).collect(),
        end: language_text(&badges.language),
    }
}

/// The badges of a list row, all in one slot in order: downloads, unseen, language.
pub fn list_badges(badges: &Badges) -> Vec<String> {
    let edge = edge_badges(badges);
    edge.start.into_iter().chain(edge.end).collect()
}

/// What one season tile draws in a display mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    /// The title under the cover, or `None` when the mode hides it (cover-only).
    pub title: Option<String>,
    pub badges: Badges,
    /// The continue button shows only when the list has a continue callback and the season says so (T-G4).
    pub continue_button: bool,
}

/// The tile of one season in one display mode. `number_mode` is the season display title mode, `has_continue` says the
/// page can continue watching (the callback exists).
pub fn tile(item: &SeasonItem, mode: SeasonDisplayMode, number_mode: bool, has_continue: bool) -> Tile {
    let title = match mode {
        SeasonDisplayMode::CoverOnlyGrid => None,
        _ => Some(tile_title(number_mode, item.season_number, &item.source_title)),
    };
    Tile {
        title,
        badges: Badges {
            downloads: count_badge(item.download_count),
            unseen: count_badge(item.unseen_count),
            language: language_badge(item.is_local, &item.source_language),
        },
        continue_button: has_continue && item.show_continue,
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;

    use super::*;

    fn season(id: i64, number: f64, title: &str, total: i64, seen: i64) -> SeasonEntry {
        SeasonEntry {
            entry: Entry { id, title: title.into(), season_number: Some(number), ..Entry::create(Kind::Anime) },
            total,
            done: seen,
            bookmarks: 0,
            fillermarks: 0,
            latest_upload: 0,
            fetched_at: 0,
            last_activity: 0,
        }
    }

    fn facts() -> SeasonFacts {
        SeasonFacts { download_count: 3, is_local: false, source_language: "en".into(), fetches_episodes: true }
    }

    fn all_on() -> Overlays {
        Overlays { downloaded: true, unseen: true, local: true, language: true, continue_watching: true }
    }

    #[test]
    fn number_mode_names_the_season_and_source_mode_keeps_the_title() {
        assert_eq!(tile_title(true, 2.0, "Source"), trf(MsgKey::display_mode_season, &[("arg1", Arg::Str("2"))]));
        assert_eq!(tile_title(false, 2.0, "Source"), "Source");
    }

    #[test]
    fn grid_size_zero_is_width_over_128_and_a_set_size_is_that_many() {
        assert_eq!(grid_columns(500, 0, false), 3, "T-G1: width 500 gives 3");
        assert_eq!(grid_columns(100, 0, false), 1, "T-G1: width 100 gives 1");
        assert_eq!(grid_columns(500, 4, false), 4, "a set size is that many");
        assert_eq!(grid_columns(500, 99, false), 10, "a stored size past ten is clamped");
        assert_eq!(grid_columns(500, 4, true), 1, "T-G2: list mode spans the full width");
    }

    #[test]
    fn overlays_off_hide_their_facts() {
        let item = season_item(&season(1, 1.0, "S1", 12, 4), Overlays::default(), &facts());
        assert_eq!(item.download_count, -1);
        assert_eq!(item.unseen_count, -1);
        assert!(!item.is_local);
        assert_eq!(item.source_language, "");
        assert!(!item.show_continue);
    }

    #[test]
    fn overlays_on_show_the_facts_and_continue_needs_unseen_and_episodes() {
        let item = season_item(&season(1, 1.0, "S1", 12, 4), all_on(), &facts());
        assert_eq!((item.download_count, item.unseen_count), (3, 8));
        assert_eq!(item.source_language, "en");
        assert!(item.show_continue);
        let done = season_item(&season(1, 1.0, "S1", 12, 12), all_on(), &facts());
        assert!(!done.show_continue, "no unseen episode, no continue");
        let nested = season_item(&season(1, 1.0, "S1", 12, 4), all_on(), &SeasonFacts { fetches_episodes: false, ..facts() });
        assert!(!nested.show_continue, "only episode-fetching seasons continue");
    }

    #[test]
    fn the_facts_of_a_season_come_from_its_extras() {
        let extra = SeasonExtra { id: 1, download_count: 4, source_language: "ja".into() };
        let one = season(1, 1.0, "S1", 12, 4);
        let facts = season_facts(&one, Some(&extra), false);
        assert_eq!((facts.download_count, facts.source_language.as_str(), facts.is_local), (4, "ja", false));
        let none = season_facts(&one, None, true);
        let nested = SeasonEntry { entry: Entry { fetch_type: Some(FetchType::Seasons), ..one.entry.clone() }, ..one.clone() };
        assert!(facts.fetches_episodes && !season_facts(&nested, None, false).fetches_episodes);
        assert_eq!((none.download_count, none.source_language.as_str(), none.is_local), (0, "", true));
        let item = season_item(&one, all_on(), &facts);
        assert_eq!((item.download_count, item.source_language.as_str()), (4, "ja"), "the overlays show what the runner read");
    }

    #[test]
    fn continue_plays_the_lowest_numbered_unseen_episode() {
        let ep = |id: i64, number: f64, seen: bool| Item { id, number, done: seen, ..Item::create(Kind::Anime) };
        let list = [ep(1, 3.0, false), ep(2, 1.0, true), ep(3, 2.0, false), ep(4, 4.0, false)];
        assert_eq!(continue_episode(&list).map(|e| e.id), Some(3));
        assert_eq!(continue_episode(&[ep(1, 1.0, true)]), None);
        assert_eq!(continue_episode(&[]), None);
    }

    #[test]
    fn language_badge_is_folder_for_local_else_uppercase_code() {
        assert_eq!(language_badge(true, "en"), LanguageBadge::Local);
        assert_eq!(language_badge(false, "ja"), LanguageBadge::Text("JA".into()));
        assert_eq!(language_badge(false, ""), LanguageBadge::None);
    }

    #[test]
    fn a_grid_tile_puts_the_counts_on_the_start_edge_and_the_language_on_the_end_edge() {
        let badges = Badges { downloads: Some(3), unseen: Some(8), language: LanguageBadge::Text("JA".into()) };
        assert_eq!(edge_badges(&badges), EdgeBadges { start: vec!["3".into(), "8".into()], end: Some("JA".into()) });
        assert_eq!(list_badges(&badges), vec!["3", "8", "JA"], "a list row has one slot");
        let local = Badges { downloads: None, unseen: Some(1), language: LanguageBadge::Local };
        assert_eq!(edge_badges(&local), EdgeBadges { start: vec!["1".into()], end: Some("Local".into()) });
        let none = Badges { downloads: None, unseen: None, language: LanguageBadge::None };
        assert_eq!(edge_badges(&none), EdgeBadges { start: vec![], end: None });
        assert!(list_badges(&none).is_empty());
    }

    #[test]
    fn count_badges_hide_at_zero_and_the_minus_one_default() {
        assert_eq!(count_badge(-1), None);
        assert_eq!(count_badge(0), None);
        assert_eq!(count_badge(5), Some(5));
    }

    #[test]
    fn cover_only_hides_the_title_and_the_other_modes_show_it() {
        let item = season_item(&season(1, 1.0, "S1", 12, 4), all_on(), &facts());
        assert_eq!(tile(&item, SeasonDisplayMode::CoverOnlyGrid, false, true).title, None);
        for mode in [SeasonDisplayMode::CompactGrid, SeasonDisplayMode::ComfortableGrid, SeasonDisplayMode::List] {
            assert_eq!(tile(&item, mode, false, true).title.as_deref(), Some("S1"));
        }
    }

    #[test]
    fn continue_button_needs_the_callback_and_the_season_flag() {
        let item = season_item(&season(1, 1.0, "S1", 12, 4), all_on(), &facts());
        assert!(tile(&item, SeasonDisplayMode::CompactGrid, false, true).continue_button);
        assert!(!tile(&item, SeasonDisplayMode::CompactGrid, false, false).continue_button, "T-G4: no callback, no button");
        let hidden = season_item(&season(1, 1.0, "S1", 12, 12), all_on(), &facts());
        assert!(!tile(&hidden, SeasonDisplayMode::List, false, true).continue_button);
    }
}
