//! Kitsu score labels and conversions (T-30a-003, `KitsuUtils.kt`). The stored score is on the 0 to 10 scale in
//! half steps. The wire rating is the same value on a 20-point integer scale.

/// The 20 labels in index order. Index 0 is "0" (no score).
pub const SCORE_LIST: [&str; 20] = [
    "0", "1", "1.5", "2", "2.5", "3", "3.5", "4", "4.5", "5", "5.5", "6", "6.5", "7", "7.5", "8",
    "8.5", "9", "9.5", "10",
];

/// The stored score of a label index (`index_to_score`). Index 0 is 0.0, index n is (n + 1) / 2.
pub fn index_to_score(index: usize) -> f64 {
    if index == 0 {
        0.0
    } else {
        (index as f64 + 1.0) / 2.0
    }
}

/// The stored score of a label. An unknown label is 0.0.
pub fn label_to_score(label: &str) -> f64 {
    SCORE_LIST
        .iter()
        .position(|l| *l == label)
        .map_or(0.0, index_to_score)
}

/// The wire rating (`ratingTwenty`) of a stored score. 0 or less is `None` (JSON null).
pub fn to_wire(score: f64) -> Option<String> {
    let rating = (score * 2.0).round() as i64;
    (rating > 0).then(|| rating.to_string())
}

/// The stored score of a wire rating. A missing rating is 0.0.
pub fn from_rating(rating: Option<i64>) -> f64 {
    rating.map_or(0.0, |r| r as f64 / 2.0)
}

/// The text the score cell shows. Rust's `f64` display gives "7" for 7.0 and "7.5" for 7.5.
pub fn display_score(score: f64) -> String {
    format!("{score}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_30a_003_the_score_list_has_twenty_labels() {
        assert_eq!(SCORE_LIST.len(), 20);
        assert_eq!(SCORE_LIST[1], "1");
        assert_eq!(SCORE_LIST[2], "1.5");
        assert_eq!(SCORE_LIST[19], "10");
    }

    #[test]
    fn t_30a_003_index_maps_to_half_steps() {
        assert_eq!(index_to_score(0), 0.0);
        assert_eq!(index_to_score(1), 1.0);
        assert_eq!(index_to_score(2), 1.5);
        assert_eq!(index_to_score(19), 10.0);
        assert_eq!(label_to_score("nope"), 0.0);
        assert_eq!(label_to_score("7.5"), 7.5);
    }

    #[test]
    fn t_30a_003_wire_score_uses_the_twenty_point_scale() {
        assert_eq!(to_wire(0.0), None);
        assert_eq!(to_wire(0.5).as_deref(), Some("1"));
        assert_eq!(to_wire(1.0).as_deref(), Some("2"));
        assert_eq!(to_wire(10.0).as_deref(), Some("20"));
        assert_eq!(from_rating(None), 0.0);
        assert_eq!(from_rating(Some(15)), 7.5);
    }

    #[test]
    fn t_30a_003_display_drops_the_decimal_for_whole_scores() {
        assert_eq!(display_score(0.0), "0");
        assert_eq!(display_score(7.0), "7");
        assert_eq!(display_score(7.5), "7.5");
        assert_eq!(display_score(10.0), "10");
    }
}
