//! Number text helpers for the sheets and panels (area 46 `PlaybackSpeedSheet.kt` and the colour
//! and time labels, T-46b-003). Pure.

use crate::time::pretty_time;

/// Fixed decimals, as `%.Nf` prints them (`to_fixed(1.2345, 2) == "1.23"`).
pub fn to_fixed(value: f64, digits: usize) -> String {
    format!("{value:.digits$}")
}

/// A speed preset as stored in the preference: the shortest float text with at least one decimal,
/// as Kotlin's `Float.toString` prints it (`1.0`, `0.25`, `3.0`).
pub fn preset_string(speed: f32) -> String {
    format!("{speed:?}")
}

/// Rounds to three decimals, ties to even (the Kotlin `round` contract).
pub fn round3_half_even(value: f64) -> f64 {
    (value * 1000.0).round_ties_even() / 1000.0
}

/// An elapsed-time label: `mm:ss`, or `h:mm:ss` from one hour on. The same rule as `pretty_time`
/// (T-46b-003, N6 and N7).
pub fn format_elapsed(seconds: i64) -> String {
    pretty_time(seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_decimals_round_the_way_printf_does() {
        assert_eq!(to_fixed(1.2345, 2), "1.23");
        assert_eq!(to_fixed(1.75, 2), "1.75");
        assert_eq!(to_fixed(0.25, 2), "0.25");
    }

    #[test]
    fn presets_keep_one_decimal_at_least() {
        assert_eq!(preset_string(1.0), "1.0");
        assert_eq!(preset_string(0.25), "0.25");
        assert_eq!(preset_string(3.0), "3.0");
    }

    #[test]
    fn ties_round_to_even() {
        assert_eq!(2.5_f64.round_ties_even(), 2.0);
        assert_eq!(3.5_f64.round_ties_even(), 4.0);
        assert_eq!(round3_half_even(1.2345), 1.234);
        assert_eq!(round3_half_even(0.5), 0.5);
    }

    #[test]
    fn elapsed_labels_follow_the_time_rule() {
        assert_eq!(format_elapsed(0), "00:00");
        assert_eq!(format_elapsed(65), "01:05");
        assert_eq!(format_elapsed(3599), "59:59");
        assert_eq!(format_elapsed(3600), "1:00:00");
        assert_eq!(format_elapsed(3723), "1:02:03");
    }
}
