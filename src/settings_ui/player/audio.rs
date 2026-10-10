//! Audio page rows (task T-48a-031): preferred languages, pitch correction, audio channels and the volume boost cap.
//! The channel names are the stored ones of the Kotlin `AudioChannels` (`Auto` maps to mpv `auto-safe` and the reverse
//! is the deliberate fix of E-AU-01, applied where mpv options are built).
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::player::language::language_row;
use crate::settings_ui::rows::{slider, switch_sub, text_choice};

pub const LANGUAGE_INFO: &str = "Audio language(s) to be selected by default on a video with multiple audio streams, Two- or three-letter languages codes work. Multiple values can be delimited by a comma.";
pub const PITCH_SUBTITLE: &str = "Prevents the audio from becoming high-pitched at faster speeds and low-pitched at slower speeds";
pub const BOOST_RANGE: std::ops::RangeInclusive<i32> = 0..=200;
pub const BOOST_DEFAULT: i32 = 30;

/// Audio channel options: the stored name and the label.
pub fn channel_options() -> Vec<(String, String)> {
    [("Auto", "Auto"), ("AutoSafe", "Auto-safe"), ("Mono", "Mono"), ("Stereo", "Stereo"), ("ReverseStereo", "Reverse stereo")]
        .iter()
        .map(|(name, label)| ((*name).to_owned(), (*label).to_owned()))
        .collect()
}

/// The rows of the Audio page, loose as the spec page has them.
pub fn page_rows(prefs: &Prefs) -> Vec<PreferenceItem> {
    let store = prefs.store();
    let language = Pref::new(store.clone(), "pref_audio_lang", String::new());
    let pitch = Pref::new(store.clone(), "pref_audio_pitch_correction", true);
    let channels = Pref::new(store.clone(), "pref_audio_config", "AutoSafe".to_owned());
    let boost = Pref::new(store.clone(), "pref_audio_volume_boost_cap", BOOST_DEFAULT);
    let shown = boost.get().to_string();
    vec![
        language_row("Preferred languages", &language, LANGUAGE_INFO),
        switch_sub("Enable audio pitch correction", PITCH_SUBTITLE, &pitch, true),
        text_choice("Audio channels", &channels, &channel_options(), true),
        slider("Volume boost cap", &boost, BOOST_RANGE, Some(shown.as_str()), true),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_pitch_on_auto_safe_and_a_boost_cap_of_30() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let titles: Vec<&str> = rows.iter().map(PreferenceItem::title).collect();
        assert_eq!(titles, ["Preferred languages", "Enable audio pitch correction", "Audio channels", "Volume boost cap"]);
        let PreferenceItem::BasicList { value, entries, .. } = &rows[2] else { panic!("a list") };
        assert_eq!(value, "AutoSafe");
        assert_eq!(entries.len(), 5);
        let PreferenceItem::Slider { value, range, .. } = &rows[3] else { panic!("a slider") };
        assert_eq!(*value, 30);
        assert_eq!((*range.start(), *range.end()), (0, 200));
    }

    #[test]
    fn the_channel_list_uses_the_stored_names_and_the_spec_labels() {
        let labels: Vec<String> = channel_options().into_iter().map(|(_, label)| label).collect();
        assert_eq!(labels, ["Auto", "Auto-safe", "Mono", "Stereo", "Reverse stereo"]);
        assert_eq!(channel_options()[4].0, "ReverseStereo");
    }
}
