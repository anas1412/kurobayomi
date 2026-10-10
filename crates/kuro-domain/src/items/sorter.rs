//! Chapter, episode and season comparators (`ChapterSorter.kt`, `EpisodeSorter.kt`, `SeasonSorter.kt`) and
//! the collator used for alphabetical order (`SortUtil.kt`). Table: `p1-topic-sorting-and-filtering.md`.
//!
//! Numbers compare with `f64::total_cmp` (the same total order as `Double.compare`). Sorts are stable.
//! An unknown sorting value falls back to SOURCE with a warning; the Kotlin code throws there.

use std::cmp::Ordering;
use std::sync::LazyLock;

use crate::entries::anime::{AnimeExt, episode_flags, season_flags};
use crate::entries::manga::chapter_flags;
use icu_collator::options::{CollatorOptions, Strength};
use icu_collator::{Collator, CollatorBorrowed, CollatorPreferences};
use icu_locale::Locale;
use kuro_data::anime::{Anime, SeasonAnime};
use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;

/// The four item sort keys shared by chapters and episodes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    Source,
    Number,
    UploadDate,
    Alphabet,
}

/// The fields the item comparator reads.
pub trait SortItem {
    fn source_order(&self) -> i64;
    fn number(&self) -> f64;
    fn date_upload(&self) -> i64;
    fn name(&self) -> &str;
}

impl SortItem for Chapter {
    fn source_order(&self) -> i64 {
        self.source_order
    }
    fn number(&self) -> f64 {
        self.chapter_number
    }
    fn date_upload(&self) -> i64 {
        self.date_upload
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl SortItem for Episode {
    fn source_order(&self) -> i64 {
        self.source_order
    }
    fn number(&self) -> f64 {
        self.episode_number
    }
    fn date_upload(&self) -> i64 {
        self.date_upload
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// The user's locale for collation: `LC_COLLATE`, then `LANG`, converted to a BCP 47 tag. Anything that
/// does not parse (including `C` and `POSIX`) is the root locale.
fn system_locale() -> Locale {
    let raw = ["LC_COLLATE", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    let base = raw
        .split(['.', '@'])
        .next()
        .unwrap_or_default()
        .replace('_', "-");
    Locale::try_from_str(&base).unwrap_or(Locale::UNKNOWN)
}

/// The primary-strength collator, built once. `a`, `A` and `á` compare equal; digits compare one character
/// at a time, so `Chapter 10` sorts before `Chapter 2`.
static COLLATOR: LazyLock<CollatorBorrowed<'static>> = LazyLock::new(|| {
    let mut options = CollatorOptions::default();
    options.strength = Some(Strength::Primary);
    let prefs = CollatorPreferences::from(&system_locale());
    // The compiled data covers every locale, so the root collator is the fallback when the lookup fails.
    Collator::try_new(prefs, options)
        .or_else(|_| Collator::try_new(CollatorPreferences::default(), options))
        .expect("compiled collation data is built in")
});

/// `compareToWithCollator`.
pub fn collate(a: &str, b: &str) -> Ordering {
    COLLATOR.compare(a, b)
}

/// The key of a chapter sorting value (`chapter_flags & SORTING_MASK`). Unknown values are `Source`.
pub fn chapter_key(sorting: i64) -> SortKey {
    match sorting {
        chapter_flags::SORTING_SOURCE => SortKey::Source,
        chapter_flags::SORTING_NUMBER => SortKey::Number,
        chapter_flags::SORTING_UPLOAD_DATE => SortKey::UploadDate,
        chapter_flags::SORTING_ALPHABET => SortKey::Alphabet,
        other => {
            tracing::warn!(
                sorting = other,
                "unknown chapter sorting, using source order"
            );
            SortKey::Source
        }
    }
}

/// The key of an episode sorting value (`episode_flags & SORTING_MASK`). Unknown values are `Source`.
pub fn episode_key(sorting: i64) -> SortKey {
    match sorting {
        episode_flags::SORTING_SOURCE => SortKey::Source,
        episode_flags::SORTING_NUMBER => SortKey::Number,
        episode_flags::SORTING_UPLOAD_DATE => SortKey::UploadDate,
        episode_flags::SORTING_ALPHABET => SortKey::Alphabet,
        other => {
            tracing::warn!(
                sorting = other,
                "unknown episode sorting, using source order"
            );
            SortKey::Source
        }
    }
}

/// Compares two items by `key`. Number, upload date and name are ascending when not descending. Source
/// order is the inverse: a source lists newest first, so descending means ascending `source_order`.
pub fn compare_items<T: SortItem>(key: SortKey, descending: bool, a: &T, b: &T) -> Ordering {
    let ascending = match key {
        SortKey::Source => a.source_order().cmp(&b.source_order()),
        SortKey::Number => a.number().total_cmp(&b.number()),
        SortKey::UploadDate => a.date_upload().cmp(&b.date_upload()),
        SortKey::Alphabet => collate(a.name(), b.name()),
    };
    let reversed = match key {
        SortKey::Source => !descending,
        _ => descending,
    };
    if reversed {
        ascending.reverse()
    } else {
        ascending
    }
}

/// `getChapterSort(manga, sortDescending)`. `sorting` is `Manga::sorting()`.
pub fn chapter_cmp(sorting: i64, descending: bool) -> impl Fn(&Chapter, &Chapter) -> Ordering {
    let key = chapter_key(sorting);
    move |a, b| compare_items(key, descending, a, b)
}

/// `getEpisodeSort(anime, sortDescending)`. `sorting` is `Anime::sorting()`.
pub fn episode_cmp(sorting: i64, descending: bool) -> impl Fn(&Episode, &Episode) -> Ordering {
    let key = episode_key(sorting);
    move |a, b| compare_items(key, descending, a, b)
}

/// `getSeasonSortComparator(anime)`: ascending by the season sort code. Unknown codes (0xE000) fall back
/// to SOURCE with a warning.
pub fn season_cmp(anime: &Anime) -> impl Fn(&SeasonAnime, &SeasonAnime) -> Ordering {
    let sorting = anime.season_sorting();
    move |a: &SeasonAnime, b: &SeasonAnime| match sorting {
        season_flags::SORT_SEASON => a.anime.season_number.total_cmp(&b.anime.season_number),
        season_flags::SORT_UPLOAD => a.latest_upload.cmp(&b.latest_upload),
        season_flags::SORT_ALPHABET => {
            collate(&a.anime.title.to_lowercase(), &b.anime.title.to_lowercase())
        }
        season_flags::SORT_COUNT => a.unseen_count().cmp(&b.unseen_count()),
        season_flags::SORT_LAST_SEEN => a.last_seen.cmp(&b.last_seen),
        season_flags::SORT_FETCHED => a.fetched_at.cmp(&b.fetched_at),
        season_flags::SORT_SOURCE => a
            .anime
            .season_source_order
            .cmp(&b.anime.season_source_order),
        other => {
            tracing::warn!(
                sorting = other,
                "unknown season sorting, using source order"
            );
            a.anime
                .season_source_order
                .cmp(&b.anime.season_source_order)
        }
    }
}

/// `sort_seasons`: the key comparator, reversed when `season_sort_descending()`, then the alphabetical title
/// as a tie-break that is never reversed.
pub fn sort_seasons(list: &mut [SeasonAnime], anime: &Anime) {
    let by_key = season_cmp(anime);
    let descending = anime.season_sort_descending();
    list.sort_by(|a, b| {
        let ord = by_key(a, b);
        let ord = if descending { ord.reverse() } else { ord };
        ord.then_with(|| collate(&a.anime.title.to_lowercase(), &b.anime.title.to_lowercase()))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::anime::Anime;

    fn chapter(name: &str, source_order: i64, number: f64, upload: i64) -> Chapter {
        Chapter {
            name: name.into(),
            source_order,
            chapter_number: number,
            date_upload: upload,
            ..Chapter::create()
        }
    }

    fn names<T: SortItem>(items: &[T]) -> Vec<String> {
        items.iter().map(|i| i.name().to_owned()).collect()
    }

    fn sorted(mut items: Vec<Chapter>, sorting: i64, descending: bool) -> Vec<String> {
        items.sort_by(chapter_cmp(sorting, descending));
        names(&items)
    }

    #[test]
    fn source_row_is_inverted_against_the_other_keys() {
        let items = vec![
            chapter("s0", 0, 0.0, 0),
            chapter("s1", 1, 0.0, 0),
            chapter("s2", 2, 0.0, 0),
        ];
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_SOURCE, true),
            ["s0", "s1", "s2"]
        );
        assert_eq!(
            sorted(items, chapter_flags::SORTING_SOURCE, false),
            ["s2", "s1", "s0"]
        );
    }

    #[test]
    fn numbers_use_total_order_with_unrecognised_first_ascending() {
        let items = vec![
            chapter("five", 0, 5.0, 0),
            chapter("unknown", 1, -1.0, 0),
            chapter("zero", 2, 0.0, 0),
            chapter("two-and-half", 3, 2.5, 0),
        ];
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_NUMBER, false),
            ["unknown", "zero", "two-and-half", "five"]
        );
        assert_eq!(
            sorted(items, chapter_flags::SORTING_NUMBER, true),
            ["five", "two-and-half", "zero", "unknown"]
        );
        assert_eq!((-0.0f64).total_cmp(&0.0), Ordering::Less);
    }

    #[test]
    fn upload_date_follows_the_same_direction_rule() {
        let items = vec![chapter("old", 0, 0.0, 10), chapter("new", 1, 0.0, 20)];
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_UPLOAD_DATE, true),
            ["new", "old"]
        );
        assert_eq!(
            sorted(items, chapter_flags::SORTING_UPLOAD_DATE, false),
            ["old", "new"]
        );
    }

    #[test]
    fn sorts_are_stable_for_equal_keys() {
        let items = vec![
            chapter("first", 0, 3.0, 0),
            chapter("second", 1, 3.0, 0),
            chapter("third", 2, 3.0, 0),
        ];
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_NUMBER, false),
            ["first", "second", "third"]
        );
        assert_eq!(
            sorted(items, chapter_flags::SORTING_NUMBER, true),
            ["first", "second", "third"]
        );
    }

    #[test]
    fn collator_is_primary_strength_and_not_natural_order() {
        assert_eq!(collate("a", "A"), Ordering::Equal);
        assert_eq!(collate("á", "a"), Ordering::Equal);
        assert_eq!(collate("Chapter 10", "Chapter 2"), Ordering::Less);
        assert_eq!(collate("b", "A"), Ordering::Greater);
        assert_eq!(collate("alpha", "Beta"), Ordering::Less, "T-17a-045: alphabetical, not by code point");
    }

    #[test]
    fn primary_equal_names_keep_input_order() {
        let items = vec![
            chapter("Chapter 2A", 0, 0.0, 0),
            chapter("chapter 2a", 1, 0.0, 0),
        ];
        assert_eq!(
            sorted(items, chapter_flags::SORTING_ALPHABET, false),
            ["Chapter 2A", "chapter 2a"]
        );
    }

    #[test]
    fn descending_false_is_playback_order_for_every_sorting() {
        // Source: the highest source_order is the oldest item, so ascending playback lists it first.
        let items = vec![
            chapter("n", 0, 2.0, 30),
            chapter("o", 1, 1.0, 10),
            chapter("p", 2, 3.0, 20),
        ];
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_SOURCE, false),
            ["p", "o", "n"]
        );
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_NUMBER, false),
            ["o", "n", "p"]
        );
        assert_eq!(
            sorted(items.clone(), chapter_flags::SORTING_UPLOAD_DATE, false),
            ["o", "p", "n"]
        );
        assert_eq!(
            sorted(items, chapter_flags::SORTING_ALPHABET, false),
            ["n", "o", "p"]
        );
    }

    #[test]
    fn unknown_sorting_falls_back_to_source_without_panicking() {
        let items = vec![chapter("s0", 0, 9.0, 0), chapter("s1", 1, 1.0, 0)];
        assert_eq!(chapter_key(0x400), SortKey::Source);
        assert_eq!(
            sorted(items.clone(), 0x400, true),
            sorted(items, chapter_flags::SORTING_SOURCE, true)
        );
    }

    #[test]
    fn episode_comparator_uses_the_episode_masks() {
        let ep = |name: &str, n: f64| Episode {
            name: name.into(),
            episode_number: n,
            ..Episode::create()
        };
        let mut items = [ep("b", 2.0), ep("a", 1.0)];
        items.sort_by(episode_cmp(episode_flags::SORTING_NUMBER, false));
        assert_eq!(
            items.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            episode_key(episode_flags::SORTING_ALPHABET),
            SortKey::Alphabet
        );
        assert_eq!(episode_key(0x800), SortKey::Source);
    }

    fn season(title: &str, source_order: i64, number: f64) -> SeasonAnime {
        SeasonAnime {
            anime: Anime {
                title: title.into(),
                season_source_order: source_order,
                season_number: number,
                ..Anime::create()
            },
            total_count: 1,
            seen_count: 0,
            bookmark_count: 0,
            fillermark_count: 0,
            latest_upload: 0,
            fetched_at: 0,
            last_seen: 0,
        }
    }

    fn titles(list: &[SeasonAnime]) -> Vec<String> {
        list.iter().map(|s| s.anime.title.clone()).collect()
    }

    fn parent(flags: i64) -> Anime {
        Anime {
            season_flags: flags,
            ..Anime::create()
        }
    }

    #[test]
    fn default_word_zero_lists_highest_source_order_first() {
        let mut list = vec![
            season("a", 0, 1.0),
            season("b", 1, 2.0),
            season("c", 2, 3.0),
        ];
        sort_seasons(&mut list, &parent(0));
        assert_eq!(titles(&list), ["c", "b", "a"]);
    }

    #[test]
    fn ascending_flag_reverses_back_to_source_order() {
        let mut list = vec![
            season("a", 0, 1.0),
            season("b", 1, 2.0),
            season("c", 2, 3.0),
        ];
        sort_seasons(&mut list, &parent(season_flags::SORT_ASC));
        assert_eq!(titles(&list), ["a", "b", "c"]);
    }

    #[test]
    fn alphabetical_tie_break_is_not_reversed() {
        let tied = || vec![season("Zeta", 0, 2.0), season("Alpha", 1, 2.0)];
        let mut asc = tied();
        sort_seasons(
            &mut asc,
            &parent(season_flags::SORT_ASC | season_flags::SORT_SEASON),
        );
        assert_eq!(titles(&asc), ["Alpha", "Zeta"]);
        let mut desc = tied();
        sort_seasons(&mut desc, &parent(season_flags::SORT_SEASON));
        assert_eq!(
            titles(&desc),
            ["Alpha", "Zeta"],
            "descending reverses the key, not the name tie-break"
        );
    }

    #[test]
    fn code_0xe000_falls_back_to_source() {
        let items = vec![season("a", 0, 1.0), season("b", 1, 2.0)];
        let flagged = parent(0xE000);
        let cmp = season_cmp(&flagged);
        let mut sorted_items = items.clone();
        sorted_items.sort_by(&cmp);
        assert_eq!(titles(&sorted_items), ["a", "b"]);
    }
}
