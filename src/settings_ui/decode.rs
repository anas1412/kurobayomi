//! The reader decode rows of Advanced settings (task T-47a-081): the custom hardware bitmap threshold. The threshold choices
//! come from the device's texture limit (the `custom_texture_limit_options` rule of `kuro-platform`). The legacy long strip
//! decoder switch (T-47a-082) is dropped (T-44a-061): the tiled decoder is the only path, and a stored value is ignored.
//!
//! Gap: the reader does not read the threshold yet (the reader's decode path is another lane's module). The device
//! limit comes from `device::device_texture_limit`, which `startup::probe_texture_limit` feeds at start-up; the row is not
//! hidden when no probe was recorded because `kuro-platform` does not say whether one was (it is not in this lane).
use kuro_platform::device::{custom_texture_limit_options, device_texture_limit};
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::{group, int_choice};

pub const THRESHOLD_TITLE: &str = "Custom hardware bitmap threshold";
pub const THRESHOLD_KEY: &str = "pref_hardware_bitmap_threshold";
pub const THRESHOLD_DEFAULT: i32 = 2048;
pub const THRESHOLD_HINT: &str = "If reader loads a blank image incrementally reduce the threshold.";
/// The device limit at or below which the threshold has one choice, so the row is disabled.
const FLOOR: i32 = 2048;

/// The threshold choices for a device limit `device`: the device limit first, labelled as the default, then the
/// multiples of 1024 below it down to 2048.
pub fn threshold_options(device: i32) -> Vec<(i32, String)> {
    custom_texture_limit_options(device)
        .into_iter()
        .enumerate()
        .map(|(i, value)| (value, if i == 0 { format!("Default ({value})") } else { value.to_string() }))
        .collect()
}

/// The row is usable only when the device limit leaves a choice above the floor.
pub fn threshold_enabled(device: i32) -> bool {
    device > FLOOR
}

/// The threshold row: the choices of the device, the stored value, and the hint with the selected label.
pub fn threshold_row(prefs: &Prefs) -> PreferenceItem {
    let device = device_texture_limit();
    let owned = threshold_options(device);
    let options: Vec<(i32, &str)> = owned.iter().map(|(value, label)| (*value, label.as_str())).collect();
    let pref = prefs.store().pref(THRESHOLD_KEY, THRESHOLD_DEFAULT);
    let mut row = int_choice(THRESHOLD_TITLE, &pref, &options, threshold_enabled(device));
    if let PreferenceItem::BasicList { common, .. } = &mut row {
        common.subtitle = Some(format!("{THRESHOLD_HINT}\nSelected: %s"));
    }
    row
}

/// The Reader group of the Advanced page: the threshold only (T-44a-061 drops the legacy decoder switch).
pub fn reader_group(prefs: &Prefs) -> crate::pref_model::model::Preference {
    group("Reader", vec![threshold_row(prefs)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_choice_is_the_device_limit_labelled_as_default() {
        let options = threshold_options(4096);
        assert_eq!(options[0], (4096, "Default (4096)".to_owned()));
        assert_eq!(options[1], (3072, "3072".to_owned()));
        assert_eq!(options[2], (2048, "2048".to_owned()));
        assert_eq!(options.len(), 3);
    }

    #[test]
    fn a_device_limit_that_is_a_multiple_of_1024_is_not_repeated() {
        let values: Vec<i32> = threshold_options(8192).into_iter().map(|(v, _)| v).collect();
        assert_eq!(values, [8192, 7168, 6144, 5120, 4096, 3072, 2048]);
    }

    #[test]
    fn the_floor_device_has_one_choice_and_a_disabled_row() {
        assert_eq!(threshold_options(2048).len(), 1);
        assert!(!threshold_enabled(2048));
        assert!(threshold_enabled(4096));
    }

    #[test]
    fn the_stored_default_is_one_of_the_choices_of_any_device_at_or_above_the_floor() {
        for device in [2048, 2560, 3000, 4096, 16384] {
            let values: Vec<i32> = threshold_options(device).into_iter().map(|(v, _)| v).collect();
            assert!(values.contains(&THRESHOLD_DEFAULT), "device {device}: {values:?}");
        }
    }

    #[test]
    fn the_reader_group_has_only_the_threshold_row() {
        let prefs = Prefs::in_memory();
        let crate::pref_model::model::Preference::Group(reader) = reader_group(&prefs) else { panic!("a group") };
        assert_eq!(reader.items.len(), 1, "the legacy long strip switch is dropped (T-44a-061)");
        assert_eq!(reader.items[0].title(), THRESHOLD_TITLE);
    }
}
