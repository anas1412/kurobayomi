//! Playback speed, presets and the speed cycle (area 45 `PlayerViewModel.playbackSpeed` and
//! `PlayerPreferences.speedPresets`, behaviour 8.6; area 46 `BottomLeftPlayerControls.kt`,
//! T-45b-038 and T-46a-008). Pure: the caller writes the chosen speed to mpv and reads the new value
//! back through the observer.

use crate::numfmt::preset_string;
use crate::player_prefs::keys;
use crate::pref_write::Write;

/// `pref_player_speed` default (`PlayerViewModel.playbackSpeed`).
pub const DEFAULT_SPEED: f32 = 1.0;

/// `default_speed_presets` when the stored set is absent (`PlayerPreferences.kt:78`).
pub const DEFAULT_PRESETS: [f32; 12] = [
    0.25, 0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0, 3.5, 4.0,
];

/// Presets from the stored string set. Values that do not parse are dropped and the rest sorted.
/// An absent or empty set gives `DEFAULT_PRESETS`.
pub fn presets(stored: Option<&[String]>) -> Vec<f32> {
    let mut values: Vec<f32> = stored
        .unwrap_or(&[])
        .iter()
        .filter_map(|s| s.trim().parse::<f32>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .collect();
    if values.is_empty() {
        return DEFAULT_PRESETS.to_vec();
    }
    values.sort_by(|a, b| a.total_cmp(b));
    values.dedup();
    values
}

/// The speed after one tap on the speed button: `+0.25`, and back to `0.25` once the speed reaches
/// `2.0` (`BottomLeftPlayerControls.kt:69`).
pub fn next_speed(current: f32) -> f32 {
    if current >= 2.0 { 0.25 } else { current + 0.25 }
}

/// Speed button text, `%.2fx`.
pub fn speed_label(speed: f32) -> String {
    format!("{speed:.2}x")
}

/// Write-through of the default speed (D-13): stores the exact float, as `preset_string` prints it.
pub fn default_speed_write(speed: f32) -> Write {
    Write::pref(keys::PREF_PLAYER_SPEED, preset_string(speed))
}

/// The slider range (`PlaybackSpeedSheet`, E67): 0.01 to 6.00, continuous.
pub const SPEED_MIN: f32 = 0.01;
pub const SPEED_MAX: f32 = 6.0;

/// Keyboard step on the desktop: 0.05, or 0.01 with Shift (p2-ui-parity section 3.2).
pub fn keyboard_step(shift: bool) -> f32 {
    if shift { 0.01 } else { 0.05 }
}

/// A speed moved by `steps` keyboard steps, kept in the slider range.
pub fn step_speed(current: f32, steps: i32, shift: bool) -> f32 {
    (current + keyboard_step(shift) * steps as f32).clamp(SPEED_MIN, SPEED_MAX)
}

/// Adds `speed` to the stored set (`Add`). The value is stored as `preset_string`. A speed whose
/// float value is already in the set leaves the set unchanged (P2).
pub fn add_preset(stored: &[String], speed: f32) -> Vec<String> {
    let mut out: Vec<String> = stored.to_vec();
    if !stored.iter().any(|s| parse_preset(s) == Some(speed)) {
        out.push(preset_string(speed));
    }
    out
}

/// One stored preset as a float. Blank, non-numeric, non-finite and non-positive text is `None`.
fn parse_preset(text: &str) -> Option<f32> {
    text.trim()
        .parse::<f32>()
        .ok()
        .filter(|v| v.is_finite() && *v > 0.0)
}

/// Removes every stored entry whose float value equals `speed` (E62: matched by value, so a stored
/// `"1.50"` is removed by `1.5`). Entries that do not parse stay.
pub fn remove_preset(stored: &[String], speed: f32) -> Vec<String> {
    stored
        .iter()
        .filter(|s| !matches!(s.trim().parse::<f32>(), Ok(v) if v == speed))
        .cloned()
        .collect()
}

/// The write that restores the 12 default presets (the reset button deletes the stored set).
pub fn reset_presets_write() -> Write {
    Write::PrefDelete {
        key: keys::DEFAULT_SPEED_PRESETS,
    }
}

/// The write that stores a whole preset set (after `add_preset` or `remove_preset`).
pub fn presets_write(values: Vec<String>) -> Write {
    Write::PrefSet {
        key: keys::DEFAULT_SPEED_PRESETS,
        values,
    }
}

/// Reset speed (E69): deletes the stored default and returns speed `1.0` for the caller to apply.
pub fn reset_speed() -> (f32, Write) {
    (
        DEFAULT_SPEED,
        Write::PrefDelete {
            key: keys::PREF_PLAYER_SPEED,
        },
    )
}

/// Pitch correction switch (E68): the preference and mpv are both written at once.
pub fn pitch_correction_writes(enabled: bool) -> Vec<Write> {
    vec![
        Write::pref(
            keys::PREF_AUDIO_PITCH_CORRECTION,
            if enabled { "true" } else { "false" },
        ),
        Write::mpv("audio-pitch-correction", if enabled { "yes" } else { "no" }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_speed_is_stored_as_the_exact_float() {
        assert_eq!(
            default_speed_write(1.0),
            Write::pref("pref_player_speed", "1.0")
        );
        assert_eq!(
            default_speed_write(1.25),
            Write::pref("pref_player_speed", "1.25")
        );
    }

    #[test]
    fn default_presets_have_twelve_values_in_order() {
        assert_eq!(DEFAULT_PRESETS.len(), 12);
        assert_eq!(presets(None), DEFAULT_PRESETS.to_vec());
        assert_eq!(presets(Some(&[])), DEFAULT_PRESETS.to_vec());
    }

    #[test]
    fn stored_presets_are_parsed_sorted_and_bad_values_dropped() {
        let stored = [
            "2.0".to_owned(),
            "x".to_owned(),
            "0.5".to_owned(),
            " 1.0 ".to_owned(),
        ];
        assert_eq!(presets(Some(&stored)), vec![0.5, 1.0, 2.0]);
    }

    #[test]
    fn speed_cycle_wraps_at_two() {
        assert_eq!(next_speed(1.1), 1.35);
        assert_eq!(next_speed(0.25), 0.5);
        assert_eq!(next_speed(1.0), 1.25);
        assert_eq!(next_speed(1.75), 2.0);
        assert_eq!(next_speed(2.0), 0.25);
        assert_eq!(next_speed(3.0), 0.25);
    }

    #[test]
    fn label_has_two_decimals() {
        assert_eq!(speed_label(1.5), "1.50x");
        assert_eq!(speed_label(DEFAULT_SPEED), "1.00x");
    }
}

#[cfg(test)]
mod preset_edit_tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn p1_parse_sorts_merges_and_skips_invalid_values() {
        assert_eq!(
            presets(Some(&strings(&["1.0", "0.25", "x", "1.0"]))),
            vec![0.25, 1.0]
        );
    }

    #[test]
    fn p2_add_is_a_no_op_for_an_existing_value_and_appends_a_new_one() {
        assert_eq!(add_preset(&strings(&["1.0"]), 1.0), strings(&["1.0"]));
        assert_eq!(
            add_preset(&strings(&["1.0"]), 1.5),
            strings(&["1.0", "1.5"])
        );
    }

    #[test]
    fn add_to_an_empty_stored_set_keeps_only_the_new_value() {
        assert_eq!(add_preset(&[], 1.0), strings(&["1.0"]));
    }

    #[test]
    fn p3_remove_matches_by_float_value() {
        assert_eq!(
            remove_preset(&strings(&["1.0", "1.5"]), 1.5),
            strings(&["1.0"])
        );
    }

    #[test]
    fn p4_remove_matches_a_non_canonical_stored_string() {
        assert_eq!(
            remove_preset(&strings(&["1.50"]), 1.5),
            Vec::<String>::new()
        );
    }

    #[test]
    fn e61_invalid_stored_strings_do_not_block_the_sheet() {
        assert_eq!(presets(Some(&strings(&["abc", "", " 2.0"]))), vec![2.0]);
    }

    #[test]
    fn make_default_stores_the_exact_float_not_the_rounded_one() {
        assert_eq!(
            default_speed_write(1.2345),
            Write::pref("pref_player_speed", "1.2345")
        );
    }

    #[test]
    fn reset_speed_returns_one_and_deletes_the_stored_default() {
        assert_eq!(
            reset_speed(),
            (
                1.0,
                Write::PrefDelete {
                    key: "pref_player_speed"
                }
            )
        );
    }

    #[test]
    fn reset_presets_deletes_the_stored_set_so_the_twelve_defaults_return() {
        assert_eq!(
            reset_presets_write(),
            Write::PrefDelete {
                key: "default_speed_presets"
            }
        );
        assert_eq!(presets(None).len(), 12);
    }

    #[test]
    fn presets_write_stores_the_whole_set() {
        assert_eq!(
            presets_write(strings(&["1.0"])),
            Write::PrefSet {
                key: "default_speed_presets",
                values: strings(&["1.0"]),
            }
        );
    }

    #[test]
    fn pitch_correction_writes_the_pref_and_mpv_together() {
        assert_eq!(
            pitch_correction_writes(false),
            vec![
                Write::pref("pref_audio_pitch_correction", "false"),
                Write::mpv("audio-pitch-correction", "no"),
            ]
        );
    }

    #[test]
    fn keyboard_steps_are_005_and_001_with_shift_and_stay_in_range() {
        assert_eq!(keyboard_step(false), 0.05);
        assert_eq!(keyboard_step(true), 0.01);
        assert_eq!(step_speed(1.0, 1, false), 1.05);
        assert_eq!(step_speed(1.0, -1, true), 0.99);
        assert_eq!(step_speed(0.02, -50, false), SPEED_MIN);
        assert_eq!(step_speed(5.99, 10, false), SPEED_MAX);
    }
}
