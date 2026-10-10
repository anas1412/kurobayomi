//! Statistics payloads (T-42a-039) and the cards the Statistics page shows (T-42a-043). Pure: the aggregation queries
//! live with the data layer. This file holds the types, the tracker mean rule (EC-42-S-08 to S-10) and the card layout
//! of p1-ui-parity 4.2.

use crate::updates::copy;
use crate::updates::format::duration_string;

/// The overview section. `total_duration_ms` is the read duration (manga) or the watched duration (anime).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Overview {
    pub library_count: i64,
    pub completed_count: i64,
    pub total_duration_ms: i64,
}

/// The entries section: how many titles are in the global update, started, or local.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Titles {
    pub global_update_count: i64,
    pub started_count: i64,
    pub local_count: i64,
}

/// The chapters section (manga) or the episodes section (anime).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub total: i64,
    pub read: i64,
    pub downloaded: i64,
}

/// The trackers section. `mean_score` is `None` where the Kotlin value is NaN (no scored track).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TrackerStats {
    pub tracked_count: i64,
    pub mean_score: Option<f64>,
    pub tracker_count: i64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MangaStats {
    pub overview: Overview,
    pub titles: Titles,
    pub chapters: Counts,
    pub trackers: TrackerStats,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AnimeStats {
    pub overview: Overview,
    pub titles: Titles,
    pub episodes: Counts,
    pub trackers: TrackerStats,
}

/// What the Statistics page holds. `Loading` shows a spinner until the aggregation is done.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum StatsState {
    Loading,
    Manga(MangaStats),
    Anime(AnimeStats),
}

/// One value tile: the value text and its caption. The icon is the GTK symbolic name, if the tile has one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tile {
    pub value: String,
    pub caption: &'static str,
    pub icon: Option<&'static str>,
}

/// One section card: its heading and a row of equal tiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub title: &'static str,
    pub tiles: Vec<Tile>,
}

/// The tracker mean rule (EC-42-S-08, S-09, S-10). `entries` holds, for each library entry, the scores of its
/// logged-in tracker links (0 means unscored). An entry counts as tracked when it has at least one link. The mean is
/// the average of the per-entry means over the entries that have a positive score, so each entry weighs 1.
pub fn tracker_stats(entries: &[Vec<f64>], logged_in_trackers: i64) -> TrackerStats {
    let tracked_count = entries.iter().filter(|links| !links.is_empty()).count() as i64;
    let entry_means: Vec<f64> = entries
        .iter()
        .filter_map(|links| {
            let scored: Vec<f64> = links.iter().copied().filter(|score| *score > 0.0).collect();
            (!scored.is_empty()).then(|| scored.iter().sum::<f64>() / scored.len() as f64)
        })
        .collect();
    let mean_score = (!entry_means.is_empty()).then(|| entry_means.iter().sum::<f64>() / entry_means.len() as f64);
    TrackerStats { tracked_count, mean_score, tracker_count: logged_in_trackers }
}

/// The mean as the page shows it: `x.xx ★` with a point decimal, or "N/A" when there is no score.
pub fn mean_score_text(mean: Option<f64>) -> String {
    match mean {
        Some(value) => format!("{value:.2} ★"),
        None => copy::NOT_AVAILABLE.to_owned(),
    }
}

fn tile(value: String, caption: &'static str, icon: Option<&'static str>) -> Tile {
    Tile { value, caption, icon }
}

fn overview_card(overview: Overview, duration_caption: &'static str) -> Card {
    Card {
        title: copy::STATS_OVERVIEW,
        tiles: vec![
            tile(overview.library_count.to_string(), copy::STATS_IN_LIBRARY, Some("user-bookmarks-symbolic")),
            tile(duration_string(overview.total_duration_ms, copy::NONE), duration_caption, Some("document-open-recent-symbolic")),
            tile(overview.completed_count.to_string(), copy::STATS_COMPLETED, Some("emblem-ok-symbolic")),
        ],
    }
}

fn titles_card(titles: Titles) -> Card {
    Card {
        title: copy::STATS_ENTRIES,
        tiles: vec![
            tile(titles.global_update_count.to_string(), copy::STATS_IN_GLOBAL_UPDATE, None),
            tile(titles.started_count.to_string(), copy::STATS_STARTED, None),
            tile(titles.local_count.to_string(), copy::STATS_LOCAL, None),
        ],
    }
}

fn counts_card(title: &'static str, counts: Counts, read_caption: &'static str) -> Card {
    Card {
        title,
        tiles: vec![
            tile(counts.total.to_string(), copy::STATS_TOTAL, None),
            tile(counts.read.to_string(), read_caption, None),
            tile(counts.downloaded.to_string(), copy::STATS_DOWNLOADED, None),
        ],
    }
}

fn trackers_card(trackers: TrackerStats) -> Card {
    Card {
        title: copy::STATS_TRACKERS,
        tiles: vec![
            tile(trackers.tracked_count.to_string(), copy::STATS_TRACKED, None),
            tile(mean_score_text(trackers.mean_score), copy::STATS_MEAN_SCORE, None),
            tile(trackers.tracker_count.to_string(), copy::STATS_USED, None),
        ],
    }
}

/// The four cards in page order (Overview, Entries, Chapters or Episodes, Trackers). `Loading` has no cards.
pub fn cards(state: &StatsState) -> Vec<Card> {
    match state {
        StatsState::Loading => Vec::new(),
        StatsState::Manga(s) => vec![
            overview_card(s.overview, copy::STATS_READ_DURATION),
            titles_card(s.titles),
            counts_card(copy::STATS_CHAPTERS, s.chapters, copy::STATS_READ),
            trackers_card(s.trackers),
        ],
        StatsState::Anime(s) => vec![
            overview_card(s.overview, copy::STATS_WATCHED_DURATION),
            titles_card(s.titles),
            counts_card(copy::STATS_EPISODES, s.episodes, copy::STATS_WATCHED),
            trackers_card(s.trackers),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manga() -> MangaStats {
        MangaStats {
            overview: Overview { library_count: 3, completed_count: 1, total_duration_ms: 0 },
            titles: Titles { global_update_count: 2, started_count: 2, local_count: 0 },
            chapters: Counts { total: 40, read: 12, downloaded: 5 },
            trackers: TrackerStats { tracked_count: 2, mean_score: Some(7.5), tracker_count: 1 },
        }
    }

    #[test]
    fn stats_state_has_loading_manga_and_anime() {
        assert_eq!(cards(&StatsState::Loading), Vec::new());
        assert_eq!(cards(&StatsState::Manga(manga())).len(), 4);
        assert_eq!(cards(&StatsState::Anime(AnimeStats::default())).len(), 4);
    }

    #[test]
    fn stats_cards_are_in_page_order_with_per_kind_titles() {
        let manga_titles: Vec<&str> = cards(&StatsState::Manga(manga())).iter().map(|c| c.title).collect();
        assert_eq!(manga_titles, ["Overview", "Entries", "Chapters", "Trackers"]);
        let anime_titles: Vec<&str> = cards(&StatsState::Anime(AnimeStats::default())).iter().map(|c| c.title).collect();
        assert_eq!(anime_titles[2], "Episodes");
    }

    #[test]
    fn stats_manga_values_and_captions_match_the_table() {
        let cards = cards(&StatsState::Manga(manga()));
        let overview: Vec<(&str, &str)> = cards[0].tiles.iter().map(|t| (t.value.as_str(), t.caption)).collect();
        assert_eq!(overview, [("3", "In library"), ("None", "Read duration"), ("1", "Completed entries")]);
        let chapters: Vec<(&str, &str)> = cards[2].tiles.iter().map(|t| (t.value.as_str(), t.caption)).collect();
        assert_eq!(chapters, [("40", "Total"), ("12", "Read"), ("5", "Downloaded")]);
        assert_eq!(cards[3].tiles[1].value, "7.50 ★");
    }

    #[test]
    fn stats_anime_duration_and_watched_captions() {
        let anime = AnimeStats {
            overview: Overview { library_count: 1, completed_count: 0, total_duration_ms: 90_000_000 },
            ..AnimeStats::default()
        };
        let cards = cards(&StatsState::Anime(anime));
        assert_eq!(cards[0].tiles[1].value, "1d 1h");
        assert_eq!(cards[0].tiles[1].caption, "Watched duration");
        assert_eq!(cards[2].tiles[1].caption, "Watched");
    }

    #[test]
    fn stats_tracker_mean_is_mean_of_entry_means() {
        // Entry A tracks 8 and 10 (mean 9), entry B tracks 6: overall 7.5.
        let stats = tracker_stats(&[vec![8.0, 10.0], vec![6.0]], 2);
        assert_eq!(stats.mean_score, Some(7.5));
        assert_eq!(stats.tracked_count, 2);
        assert_eq!(stats.tracker_count, 2);
    }

    #[test]
    fn stats_tracker_zero_score_is_excluded_but_entry_is_tracked() {
        let stats = tracker_stats(&[vec![0.0], vec![7.0]], 1);
        assert_eq!(stats.tracked_count, 2);
        assert_eq!(stats.mean_score, Some(7.0));
    }

    #[test]
    fn stats_no_scored_tracks_gives_none_and_na() {
        let stats = tracker_stats(&[vec![0.0], vec![]], 1);
        assert_eq!(stats.tracked_count, 1);
        assert_eq!(stats.mean_score, None);
        assert_eq!(mean_score_text(stats.mean_score), "N/A");
        assert_eq!(mean_score_text(Some(7.4567)), "7.46 ★");
    }

    #[test]
    fn stats_zero_duration_shows_none_in_the_overview() {
        assert_eq!(cards(&StatsState::Manga(manga()))[0].tiles[1].value, "None");
    }
}
