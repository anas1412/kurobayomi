//! Chapter gap calculation (T-44a-003, B§5.2, E-04, E-05). Upstream `MissingItemsTest` vectors are the tests.

/// The number of chapters missing between `higher` and `lower`: `floor(higher) - floor(lower) - 1`.
///
/// Returns 0 when either chapter is absent or has an unrecognised number (`< 0`). The result can be negative
/// (`10.5` and `10.0` give -1); callers test `> 0`.
pub fn calculate_chapter_gap(higher: Option<f64>, lower: Option<f64>) -> i32 {
    let (Some(higher), Some(lower)) = (higher, lower) else { return 0 };
    if higher < 0.0 || lower < 0.0 {
        return 0;
    }
    higher.floor() as i32 - lower.floor() as i32 - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_vectors() {
        assert_eq!(calculate_chapter_gap(Some(10.0), Some(9.0)), 0);
        assert_eq!(calculate_chapter_gap(Some(10.0), Some(8.0)), 1);
        assert_eq!(calculate_chapter_gap(Some(10.0), Some(8.5)), 1);
        assert_eq!(calculate_chapter_gap(Some(10.0), Some(1.1)), 8);
        assert_eq!(calculate_chapter_gap(Some(-1.0), Some(10.0)), 0);
        assert_eq!(calculate_chapter_gap(Some(99.0), Some(-1.0)), 0);
    }

    #[test]
    fn t027_the_plan_vectors_including_the_half_chapter_case() {
        assert_eq!(calculate_chapter_gap(Some(5.0), Some(5.5)), -1);
        assert!(calculate_chapter_gap(Some(5.0), Some(5.5)) <= 0, "callers hide values not above 0 (E-29)");
    }

    #[test]
    fn absent_chapters_give_no_gap() {
        assert_eq!(calculate_chapter_gap(None, Some(3.0)), 0);
        assert_eq!(calculate_chapter_gap(Some(3.0), None), 0);
    }

    #[test]
    fn equal_integer_parts_give_minus_one_which_is_not_a_gap() {
        assert_eq!(calculate_chapter_gap(Some(10.5), Some(10.0)), -1);
        assert!(calculate_chapter_gap(Some(10.5), Some(10.0)) <= 0);
    }
}
