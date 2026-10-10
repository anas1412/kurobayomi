//! MangaUpdates score table and index mapping (T-32a-003, T-32a-026; `MangaUpdates.kt:29-39, 69-73`).

use super::error::MuError;

/// The label of the unrated entry, index 0.
pub const UNRATED: &str = "-";

/// The 92 picker labels in order: `-`, `1.0` to `1.9`, ..., `9.9`, `10.0` (B-2.2).
pub fn labels() -> Vec<String> {
    let mut out = vec![UNRATED.to_owned()];
    for whole in 1..=9 {
        for tenth in 0..=9 {
            out.push(format!("{whole}.{tenth}"));
        }
    }
    out.push("10.0".to_owned());
    out
}

/// The stored score of picker `index` (`indexToScore`). Index 0 is 0.0. An index past the table is
/// `ScoreOutOfRange` instead of a panic (fix D-MU-7).
pub fn index_to_score(labels: &[String], index: usize) -> Result<f64, MuError> {
    if index == 0 {
        return Ok(0.0);
    }
    let label = labels.get(index).ok_or(MuError::ScoreOutOfRange(index))?;
    label
        .parse::<f64>()
        .map_err(|_| MuError::ScoreOutOfRange(index))
}

/// The score text shown for a track. An unrated track shows `-` (fix D-MU-8); other scores use the shortest form
/// (`7.5`, `10.0`).
pub fn display_score(score: f64) -> String {
    if score == 0.0 {
        UNRATED.to_owned()
    } else {
        format!("{score:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_32a_003_the_table_has_92_entries_in_source_order() {
        let t = labels();
        assert_eq!(t.len(), 92);
        assert_eq!(t[0], "-");
        assert_eq!(t[1], "1.0");
        assert_eq!(t[2], "1.1");
        assert_eq!(t[10], "1.9");
        assert_eq!(t[11], "2.0");
        assert_eq!(t[90], "9.9");
        assert_eq!(t[91], "10.0");
    }

    #[test]
    fn t_32a_003_index_to_score_follows_the_table() {
        let t = labels();
        assert_eq!(index_to_score(&t, 0), Ok(0.0));
        assert_eq!(index_to_score(&t, 1), Ok(1.0));
        assert_eq!(index_to_score(&t, 2), Ok(1.1));
        assert_eq!(index_to_score(&t, 11), Ok(2.0));
        assert_eq!(index_to_score(&t, 91), Ok(10.0));
    }

    #[test]
    fn t_32a_003_out_of_range_index_is_an_error_not_a_panic() {
        let t = labels();
        assert_eq!(index_to_score(&t, 92), Err(MuError::ScoreOutOfRange(92)));
        assert_eq!(index_to_score(&[], 0), Ok(0.0));
    }

    #[test]
    fn t_32a_026_display_is_dash_for_zero_and_shortest_text_otherwise() {
        assert_eq!(display_score(0.0), "-");
        assert_eq!(display_score(7.5), "7.5");
        assert_eq!(display_score(10.0), "10.0");
        assert_eq!(display_score(7.0), "7.0");
    }

    #[test]
    fn t_32a_026_every_picker_label_round_trips_through_its_score() {
        let t = labels();
        for (i, label) in t.iter().enumerate().skip(1) {
            let score = index_to_score(&t, i).unwrap();
            assert_eq!(&format!("{score:?}"), label, "index {i}");
        }
    }
}
