//! Missing-item counts, gaps and the separators that the entry screens insert between rows
//! (`MissingChapters.kt`, `MissingEpisodes.kt`, `CollectionUtils.insertSeparators`).
//!
//! Only exactly `-1.0` is ignored by `missing_items_count`; `-2.0` is counted as the integer `-2`, as in
//! the Kotlin source. Consequence: `[-2.0, 3.0]` gives 4 (the jump from -2 to 3), not 2.

use kuro_data::chapter::Chapter;
use kuro_data::episode::Episode;

/// `missingChaptersCount` / `missingEntriesCount`: the number of integer slots missing between the
/// distinct truncated numbers, counting from 0.
pub fn missing_items_count(numbers: &[f64]) -> i32 {
    let mut items: Vec<i32> = numbers
        .iter()
        .filter(|n| **n != -1.0)
        .map(|n| *n as i32)
        .collect();
    items.sort_unstable();
    items.dedup();

    let mut missing = 0;
    let mut previous = 0;
    for current in items {
        if current > previous + 1 {
            missing += current - previous - 1;
        }
        previous = current;
    }
    missing
}

/// `calculateChapterGap(higherNumber, lowerNumber)` and the episode twin: 0 when either is negative,
/// otherwise `floor(higher) - floor(lower) - 1`, which can be negative for an unsorted pair.
pub fn gap(higher: f64, lower: f64) -> i32 {
    if higher < 0.0 || lower < 0.0 {
        return 0;
    }
    higher.floor() as i32 - lower.floor() as i32 - 1
}

/// The fields the gap and separator code reads from a chapter or an episode.
pub trait Numbered {
    fn item_id(&self) -> i64;
    fn number(&self) -> f64;
}

impl Numbered for Chapter {
    fn item_id(&self) -> i64 {
        self.id
    }
    fn number(&self) -> f64 {
        self.chapter_number
    }
}

impl Numbered for Episode {
    fn item_id(&self) -> i64 {
        self.id
    }
    fn number(&self) -> f64 {
        self.episode_number
    }
}

/// `calculateChapterGap(higher: Chapter?, lower: Chapter?)` and `calculateEpisodeGap`: 0 when either side is
/// missing or has an unrecognised number.
pub fn item_gap<T: Numbered>(higher: Option<&T>, lower: Option<&T>) -> i32 {
    match (higher, lower) {
        (Some(h), Some(l)) if h.number() >= 0.0 && l.number() >= 0.0 => gap(h.number(), l.number()),
        _ => 0,
    }
}

/// A row of a list with the missing-count separators inserted.
#[derive(Clone, Debug, PartialEq)]
pub enum ListRow<T> {
    Item(T),
    /// `EpisodeList.MissingCount` / `ChapterList.MissingCount`. The text (`Missing %1$s chapter(s)` or
    /// `Missing %1$s item(s)`) comes from the i18n catalogue, keyed by the entry kind.
    MissingCount {
        id: String,
        count: i32,
    },
}

/// The separator between `before` and `after` in list order, following `AnimeScreenModel.kt:1636-1659`.
/// `descending` says which neighbour is the higher number. A separator is produced only when the count is
/// above zero. At the list end the count is `floor(higher) - 1`, clamped at zero.
fn separator<T: Numbered>(
    before: Option<&T>,
    after: Option<&T>,
    descending: bool,
) -> Option<ListRow<T>> {
    let (lower, higher) = if descending {
        (after, before)
    } else {
        (before, after)
    };
    let higher = higher?;
    let count = match lower {
        None => (higher.number().floor() as i32 - 1).max(0),
        Some(_) => item_gap(Some(higher), lower),
    };
    if count <= 0 {
        return None;
    }
    let lower_id = lower.map_or_else(|| "null".to_owned(), |l| l.item_id().to_string());
    Some(ListRow::MissingCount {
        id: format!("{lower_id}-{}", higher.item_id()),
        count,
    })
}

/// `insertSeparators`: every item, with a `MissingCount` row after a pair that has a gap, and one before
/// the first item and after the last when the gap rule applies there.
pub fn insert_missing_separators<T: Numbered>(list: Vec<T>, descending: bool) -> Vec<ListRow<T>> {
    let mut out = Vec::with_capacity(list.len() * 2 + 1);
    let mut items = list.into_iter();
    let Some(mut before) = items.next() else {
        return out;
    };
    if let Some(sep) = separator(None, Some(&before), descending) {
        out.push(sep);
    }
    loop {
        let after = items.next();
        let sep = separator(Some(&before), after.as_ref(), descending);
        out.push(ListRow::Item(before));
        out.extend(sep);
        match after {
            Some(next) => before = next,
            None => break,
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(id: i64, number: f64) -> Chapter {
        Chapter {
            id,
            chapter_number: number,
            ..Chapter::create()
        }
    }

    fn episode(id: i64, number: f64) -> Episode {
        Episode {
            id,
            episode_number: number,
            ..Episode::create()
        }
    }

    #[test]
    fn upstream_missing_items_vectors() {
        assert_eq!(missing_items_count(&[]), 0);
        assert_eq!(missing_items_count(&[-1.0, -1.0, -1.0]), 0);
        assert_eq!(missing_items_count(&[1.0, 1.0, 1.1, 1.5, 1.6, 1.99]), 0);
        assert_eq!(
            missing_items_count(&[-1.0, 1.0, 2.0, 2.2, 4.0, 6.0, 10.0, 10.0]),
            5
        );
    }

    #[test]
    fn extra_missing_items_vectors() {
        assert_eq!(missing_items_count(&[0.0]), 0);
        assert_eq!(missing_items_count(&[5.0]), 4);
        assert_eq!(
            missing_items_count(&[-2.0, 3.0]),
            4,
            "Kotlin counts the jump from -2"
        );
    }

    #[test]
    fn gap_arithmetic_and_negative_guards() {
        assert_eq!(gap(5.0, 7.0), -3);
        assert_eq!(gap(10.0, 8.5), 1);
        assert_eq!(gap(10.0, 1.1), 8);
        assert_eq!(gap(-1.0, 10.0), 0);
        assert_eq!(item_gap(Some(&chapter(1, 10.0)), Some(&chapter(2, 1.1))), 8);
        assert_eq!(
            item_gap(Some(&chapter(1, -1.0)), Some(&chapter(2, 10.0))),
            0
        );
        assert_eq!(item_gap(None::<&Episode>, Some(&episode(2, 3.0))), 0);
        assert_eq!(
            item_gap(Some(&episode(1, 99.0)), Some(&episode(2, -1.0))),
            0
        );
    }

    #[test]
    fn separators_between_neighbours_and_at_the_end() {
        // Descending list 10, 8, 7: a gap of 1 between 10 and 8, none between 8 and 7, and the list end
        // at 7 gives floor(7) - 1 = 6 missing below it.
        let rows = insert_missing_separators(
            vec![chapter(1, 10.0), chapter(2, 8.0), chapter(3, 7.0)],
            true,
        );
        assert_eq!(
            rows,
            vec![
                ListRow::Item(chapter(1, 10.0)),
                ListRow::MissingCount {
                    id: "2-1".into(),
                    count: 1
                },
                ListRow::Item(chapter(2, 8.0)),
                ListRow::Item(chapter(3, 7.0)),
                ListRow::MissingCount {
                    id: "null-3".into(),
                    count: 6
                },
            ]
        );
    }

    #[test]
    fn last_row_separator_counts_down_to_one() {
        // Descending list ending at 5: the last row gets floor(5) - 1 = 4 missing, with a "null" lower id.
        let rows = insert_missing_separators(vec![episode(7, 5.0)], true);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows[1],
            ListRow::MissingCount {
                id: "null-7".into(),
                count: 4
            }
        );
    }

    #[test]
    fn ascending_list_puts_the_head_separator_first() {
        let rows = insert_missing_separators(vec![episode(3, 4.0)], false);
        assert_eq!(
            rows,
            vec![
                ListRow::MissingCount {
                    id: "null-3".into(),
                    count: 3
                },
                ListRow::Item(episode(3, 4.0))
            ]
        );
    }

    #[test]
    fn no_separator_for_unrecognised_or_empty_lists() {
        assert!(insert_missing_separators(Vec::<Chapter>::new(), true).is_empty());
        let rows = insert_missing_separators(vec![chapter(1, -1.0), chapter(2, -1.0)], true);
        assert_eq!(
            rows,
            vec![
                ListRow::Item(chapter(1, -1.0)),
                ListRow::Item(chapter(2, -1.0))
            ]
        );
    }
}
