//! Upcoming feeds (area 09): the status codes an entry carries, and the sets the upcoming list reads (the sets are
//! `Kind::upcoming_statuses`, the one definition).

/// Entry status codes (`SManga` and `SAnime` constants).
pub const UNKNOWN: i64 = 0;
pub const ONGOING: i64 = 1;
pub const COMPLETED: i64 = crate::entries::flags::STATUS_COMPLETED;
pub const LICENSED: i64 = 3;
pub const PUBLISHING_FINISHED: i64 = 4;
pub const CANCELLED: i64 = 5;
pub const ON_HIATUS: i64 = 6;
/// Anime only: a title that has not aired yet.
pub const UPCOMING: i64 = 7;

/// Manga statuses shown on the upcoming list (`GetUpcomingManga`): `Kind::Manga.upcoming_statuses()`.
pub const UPCOMING_MANGA_STATUSES: [i64; 2] = {
    let s = kuro_platform::kind::Kind::Manga.upcoming_statuses();
    [s[0], s[1]]
};

/// Anime statuses shown on the upcoming list (`GetUpcomingAnime`), which adds [`UPCOMING`]: `Kind::Anime.upcoming_statuses()`.
pub const UPCOMING_ANIME_STATUSES: [i64; 3] = {
    let s = kuro_platform::kind::Kind::Anime.upcoming_statuses();
    [s[0], s[1], s[2]]
};

#[cfg(test)]
mod tests {
    use super::*;

    /// TC-UP-01: the manga set is `[1, 4]`, the anime set is `[1, 4, 7]`, and statuses 2 and 5 are excluded.
    #[test]
    fn tc_up_01_the_sets_match_the_upstream_lists() {
        assert_eq!(UPCOMING_MANGA_STATUSES, [1, 4]);
        assert_eq!(UPCOMING_ANIME_STATUSES, [1, 4, 7]);
        for excluded in [COMPLETED, CANCELLED] {
            assert!(!UPCOMING_MANGA_STATUSES.contains(&excluded));
            assert!(!UPCOMING_ANIME_STATUSES.contains(&excluded));
        }
    }

    #[test]
    fn the_status_codes_are_the_upstream_numbers() {
        let codes = [UNKNOWN, ONGOING, COMPLETED, LICENSED, PUBLISHING_FINISHED, CANCELLED, ON_HIATUS, UPCOMING];
        assert_eq!(codes, [0, 1, 2, 3, 4, 5, 6, 7]);
    }
}
