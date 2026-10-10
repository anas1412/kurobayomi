//! The season filter, sort and overlay pipeline (T-39b-008). Upstream: `AnimeScreenModel.applySeasonFilters`
//! (AnimeScreenModel.kt:1701-1737) and `SeasonSorter.kt` (here `sort_seasons`). Pure: `process_seasons` gives the cards
//! the grid shows, in order. The filters read the true counts; the overlays only decide what a card shows.

use std::collections::HashMap;

use kuro_data::anime::{Anime, SeasonAnime};
use kuro_domain::app::items::downloaded_filter;
use kuro_domain::entries::anime::{AnimeExt, season_flags};
use kuro_domain::entries::manga::STATUS_COMPLETED;
use kuro_domain::items::sorter::sort_seasons;
use kuro_prefs::tristate::TriState;

use super::season_tile::{Overlays, SeasonFacts, SeasonItem, season_item};

/// The six season filters of the anime. The downloaded filter takes the downloaded-only override (T-39b-054).
pub struct SeasonFilters {
    pub unseen: TriState,
    pub downloaded: TriState,
    pub started: TriState,
    pub completed: TriState,
    pub bookmarked: TriState,
    pub fillermarked: TriState,
}

impl SeasonFilters {
    pub fn of(anime: &Anime, downloaded_only: bool) -> Self {
        Self {
            unseen: anime.season_unseen_filter(),
            downloaded: downloaded_filter(
                anime.season_downloaded_filter_raw(),
                season_flags::SHOW_DOWNLOADED,
                season_flags::SHOW_NOT_DOWNLOADED,
                downloaded_only,
            ),
            started: anime.season_started_filter(),
            completed: anime.season_completed_filter(),
            bookmarked: anime.season_bookmarked_filter(),
            fillermarked: anime.season_fillermarked_filter(),
        }
    }

    /// Whether a season passes every filter.
    pub fn keeps(&self, season: &SeasonAnime, facts: &SeasonFacts) -> bool {
        self.unseen.apply_filter(|| !season.seen())
            && self.started.apply_filter(|| season.has_started())
            && self.completed.apply_filter(|| season.anime.status == STATUS_COMPLETED)
            && self.bookmarked.apply_filter(|| season.has_bookmarks())
            && self.fillermarked.apply_filter(|| season.has_fillermarks())
            && self.downloaded.apply_filter(|| facts.download_count > 0 || facts.is_local)
    }
}

/// The cards of the season grid: the seasons that pass the filters, sorted by the season sort (the alphabetical tie-break
/// is always ascending), each with the overlays of the parent applied. `seasons` pairs each season with its facts.
pub fn process_seasons(anime: &Anime, seasons: &[(SeasonAnime, SeasonFacts)], downloaded_only: bool) -> Vec<SeasonItem> {
    let overlays = Overlays::of(anime);
    let filters = SeasonFilters::of(anime, downloaded_only);
    let facts: HashMap<i64, &SeasonFacts> = seasons.iter().map(|(s, f)| (s.id(), f)).collect();
    let mut kept: Vec<SeasonAnime> =
        seasons.iter().filter(|(s, f)| filters.keeps(s, f)).map(|(s, _)| s.clone()).collect();
    sort_seasons(&mut kept, anime);
    kept.iter().map(|s| season_item(s, overlays, facts[&s.id()])).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn season(id: i64, title: &str, total: i64, seen: i64, bookmarks: i64) -> (SeasonAnime, SeasonFacts) {
        let mut anime = Anime::create();
        anime.id = id;
        anime.title = title.into();
        (
            SeasonAnime {
                anime,
                total_count: total,
                seen_count: seen,
                bookmark_count: bookmarks,
                fillermark_count: 0,
                latest_upload: 0,
                fetched_at: 0,
                last_seen: 0,
            },
            SeasonFacts { download_count: 0, is_local: false, source_language: String::new(), fetches_episodes: true },
        )
    }

    fn parent(flags: i64) -> Anime {
        let mut anime = Anime::create();
        anime.season_flags = flags;
        anime
    }

    #[test]
    fn the_unseen_filter_keeps_seasons_with_an_unseen_episode_and_a_zero_total_is_seen() {
        let seasons = vec![season(1, "a", 3, 3, 0), season(2, "b", 3, 1, 0), season(3, "c", 0, 0, 0)];
        let kept: Vec<i64> = process_seasons(&parent(season_flags::SHOW_UNSEEN), &seasons, false).iter().map(|s| s.id).collect();
        assert_eq!(kept, vec![2], "a fully seen season and a season with no episodes are both seen");
    }

    #[test]
    fn the_downloaded_filter_reads_downloaded_only_as_enabled_is() {
        let mut seasons = vec![season(1, "a", 1, 0, 0), season(2, "b", 1, 0, 0)];
        seasons[1].1.download_count = 2;
        let flags = parent(0);
        assert_eq!(process_seasons(&flags, &seasons, false).len(), 2, "no filter keeps both");
        let only: Vec<i64> = process_seasons(&flags, &seasons, true).iter().map(|s| s.id).collect();
        assert_eq!(only, vec![2], "downloaded-only keeps the season with downloads");
    }

    #[test]
    fn the_alphabetical_tie_break_is_ascending_even_when_the_sort_is_reversed() {
        let mut seasons = vec![season(1, "b", 1, 0, 0), season(2, "a", 1, 0, 0)];
        for (s, _) in &mut seasons {
            s.anime.season_source_order = 0;
        }
        // the default sort is descending: the title still decides ascending
        let titles: Vec<String> = process_seasons(&parent(season_flags::SORT_DESC), &seasons, false)
            .iter()
            .map(|s| s.source_title.clone())
            .collect();
        assert_eq!(titles, vec!["a".to_owned(), "b".to_owned()]);
    }

    #[test]
    fn the_unseen_count_is_the_true_count_and_hides_when_the_overlay_is_off() {
        let seasons = vec![season(1, "a", 5, 2, 0)];
        let shown = process_seasons(&parent(season_flags::OVERLAY_UNSEEN_MASK), &seasons, false);
        assert_eq!(shown[0].unseen_count, 3);
        let hidden = process_seasons(&parent(0), &seasons, false);
        assert_eq!(hidden[0].unseen_count, -1);
    }

    #[test]
    fn the_bookmarked_filter_reads_the_bookmark_count() {
        let seasons = vec![season(1, "a", 2, 0, 0), season(2, "b", 2, 0, 1)];
        let flags = parent(season_flags::SHOW_BOOKMARKED);
        let kept: Vec<i64> = process_seasons(&flags, &seasons, false).iter().map(|s| s.id).collect();
        assert_eq!(kept, vec![2]);
    }
}
