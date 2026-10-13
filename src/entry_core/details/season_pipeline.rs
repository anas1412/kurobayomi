//! The season filter, sort and overlay pipeline (T-39b-008). Upstream: `AnimeScreenModel.applySeasonFilters`
//! (AnimeScreenModel.kt:1701-1737) and `SeasonSorter.kt` (here `sort_seasons`). Pure: `process_seasons` gives the cards
//! the grid shows, in order. The filters read the true counts; the overlays only decide what a card shows.

use std::collections::HashMap;

use kuro_data::entry::{Entry, SeasonEntry};
use kuro_domain::app::items::downloaded_filter;
use kuro_db::flags::season as season_flags;
use kuro_domain::entries::flags::{EntryFlags, STATUS_COMPLETED};
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
    pub fn of(entry: &Entry, downloaded_only: bool) -> Self {
        let season = entry.season();
        Self {
            unseen: season.unseen_filter(),
            downloaded: downloaded_filter(
                season.downloaded_filter_raw(),
                season_flags::SHOW_DOWNLOADED,
                season_flags::SHOW_NOT_DOWNLOADED,
                downloaded_only,
            ),
            started: season.started_filter(),
            completed: season.completed_filter(),
            bookmarked: season.bookmarked_filter(),
            fillermarked: season.fillermarked_filter(),
        }
    }

    /// Whether a season passes every filter.
    pub fn keeps(&self, season: &SeasonEntry, facts: &SeasonFacts) -> bool {
        self.unseen.apply_filter(|| !season.is_done())
            && self.started.apply_filter(|| season.done > 0)
            && self.completed.apply_filter(|| season.entry.status == STATUS_COMPLETED)
            && self.bookmarked.apply_filter(|| season.bookmarks > 0)
            && self.fillermarked.apply_filter(|| season.fillermarks > 0)
            && self.downloaded.apply_filter(|| facts.download_count > 0 || facts.is_local)
    }
}

/// The cards of the season grid: the seasons that pass the filters, sorted by the season sort (the alphabetical tie-break
/// is always ascending), each with the overlays of the parent applied. `seasons` pairs each season with its facts.
pub fn process_seasons(entry: &Entry, seasons: &[(SeasonEntry, SeasonFacts)], downloaded_only: bool) -> Vec<SeasonItem> {
    let overlays = Overlays::of(entry);
    let filters = SeasonFilters::of(entry, downloaded_only);
    let facts: HashMap<i64, &SeasonFacts> = seasons.iter().map(|(s, f)| (s.entry.id, f)).collect();
    let mut kept: Vec<SeasonEntry> =
        seasons.iter().filter(|(s, f)| filters.keeps(s, f)).map(|(s, _)| s.clone()).collect();
    sort_seasons(&mut kept, entry);
    kept.iter().map(|s| season_item(s, overlays, facts[&s.entry.id])).collect()
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;

    use super::*;

    fn season(id: i64, title: &str, total: i64, seen: i64, bookmarks: i64) -> (SeasonEntry, SeasonFacts) {
        let mut entry = Entry::create(Kind::Anime);
        entry.id = id;
        entry.title = title.into();
        (
            SeasonEntry {
                entry: entry,
                total,
                done: seen,
                bookmarks,
                fillermarks: 0,
                latest_upload: 0,
                fetched_at: 0,
                last_activity: 0,
            },
            SeasonFacts { download_count: 0, is_local: false, source_language: String::new(), fetches_episodes: true },
        )
    }

    fn parent(flags: i64) -> Entry {
        let mut entry = Entry::create(Kind::Anime);
        entry.season_flags = Some(flags);
        entry
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
            s.entry.season_source_order = Some(0);
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
