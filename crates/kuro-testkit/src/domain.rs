//! Domain test fixtures (T-07a-034): entries built from their `create()` defaults, item upload dates, and a
//! clock that does not move. Every builder only overrides the fields the caller names.

use kuro_data::anime::Anime;
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;
use kuro_data::manga::Manga;

/// A manga with the `create()` defaults.
pub fn manga() -> Manga {
    Manga::create()
}

/// An anime with the `create()` defaults.
pub fn anime() -> Anime {
    Anime::create()
}

/// A chapter with number `number` and the `create()` defaults otherwise.
pub fn chapter(number: f64) -> Chapter {
    Chapter { chapter_number: number, ..Chapter::create() }
}

/// An episode with number `number` and the `create()` defaults otherwise.
pub fn episode(number: f64) -> Episode {
    Episode { episode_number: number, ..Episode::create() }
}

/// A season (an anime with a parent) with the given title and season number.
pub fn season(title: &str, number: f64) -> Anime {
    Anime { title: title.to_owned(), season_number: number, ..Anime::create() }
}

/// A clock that always reads the same instant, for tests of time-based rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FixedClock {
    now_ms: i64,
}

impl FixedClock {
    pub const fn at(now_ms: i64) -> Self {
        Self { now_ms }
    }

    /// Epoch milliseconds of the fixed instant.
    pub const fn now_ms(self) -> i64 {
        self.now_ms
    }
}

/// Upload dates for a series of items: `count` dates starting at `start_ms`, spaced by a fixed step.
pub struct ItemDates;

impl ItemDates {
    const DAY_MS: i64 = 86_400_000;
    const HOUR_MS: i64 = 3_600_000;

    /// Dates every `n` days, newest last.
    pub fn every_n_days(start_ms: i64, n: i64, count: usize) -> Vec<i64> {
        (0..count as i64).map(|i| start_ms + i * n * Self::DAY_MS).collect()
    }

    /// Dates every `n` hours, newest last.
    pub fn every_n_hours(start_ms: i64, n: i64, count: usize) -> Vec<i64> {
        (0..count as i64).map(|i| start_ms + i * n * Self::HOUR_MS).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builders_keep_the_create_defaults_and_the_named_fields() {
        assert_eq!(chapter(3.5).chapter_number, 3.5);
        assert_eq!(episode(2.0).episode_number, 2.0);
        let s = season("Season 2", 2.0);
        assert_eq!((s.title.as_str(), s.season_number), ("Season 2", 2.0));
        assert_eq!(manga().title, Manga::create().title);
        assert_eq!(anime().url, Anime::create().url);
    }

    #[test]
    fn item_dates_step_by_the_given_unit() {
        let days = ItemDates::every_n_days(0, 2, 3);
        assert_eq!(days, vec![0, 2 * 86_400_000, 4 * 86_400_000]);
        let hours = ItemDates::every_n_hours(1000, 6, 2);
        assert_eq!(hours, vec![1000, 1000 + 6 * 3_600_000]);
        assert_eq!(FixedClock::at(42).now_ms(), 42);
    }
}
