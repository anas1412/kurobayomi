//! Chapter number text: `DecimalFormat("#.###")` with the default HALF_EVEN rounding, on the exact binary value.
//! Upstream: `app/src/main/java/eu/kanade/presentation/util/ItemNumberFormatter.kt`.

/// Formats a chapter number the way the upstream `#.###` pattern does: at most three decimals, trailing
/// zeros dropped, the minus sign kept for negative values (including values that round to zero).
/// `NaN` is `NaN`, infinities are `∞` / `-∞`, as Java's `DecimalFormatSymbols` prints them.
pub fn format_chapter_number(n: f64) -> String {
    if n.is_nan() {
        return "NaN".to_owned();
    }
    if n.is_infinite() {
        return if n < 0.0 { "-∞" } else { "∞" }.to_owned();
    }
    // Rust rounds exact ties to even, the same as HALF_EVEN, so the three decimals match Java.
    let fixed = format!("{n:.3}");
    let trimmed = fixed.trim_end_matches('0').trim_end_matches('.');
    trimmed.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_and_fractional_values() {
        assert_eq!(format_chapter_number(1.0), "1");
        assert_eq!(format_chapter_number(1.5), "1.5");
        assert_eq!(format_chapter_number(-1.0), "-1");
        assert_eq!(format_chapter_number(10.25), "10.25");
        assert_eq!(format_chapter_number(0.0), "0");
    }

    #[test]
    fn half_even_on_exact_ties() {
        // 0.0625 and 0.1875 are exact binary values, so the third decimal is a true tie.
        assert_eq!(format_chapter_number(0.0625), "0.062");
        assert_eq!(format_chapter_number(0.1875), "0.188");
    }

    #[test]
    fn signed_zero_and_small_negatives_keep_the_sign() {
        assert_eq!(format_chapter_number(-0.0), "-0");
        assert_eq!(format_chapter_number(-0.0001), "-0");
    }

    #[test]
    fn non_finite_values_match_java_symbols() {
        assert_eq!(format_chapter_number(f64::NAN), "NaN");
        assert_eq!(format_chapter_number(f64::INFINITY), "∞");
        assert_eq!(format_chapter_number(f64::NEG_INFINITY), "-∞");
    }

    #[test]
    fn three_decimals_at_most() {
        assert_eq!(format_chapter_number(2.0004), "2");
        assert_eq!(format_chapter_number(2.0016), "2.002");
    }
}
