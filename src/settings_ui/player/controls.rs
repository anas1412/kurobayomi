//! Internal player page rows: progress and preserve position (T-48a-015), Controls and Hosters (T-48a-013) and the
//! Display group (T-48a-014). Intro skip, PiP and external player rows belong to tasks 016 to 018.
// Not reachable from `route.rs` yet: the Internal player page is still the spec page.
#![allow(dead_code)]
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::bind;
use crate::pref_model::model::{ItemCommon, Preference, PreferenceItem};
use crate::settings_ui::rows::{group, info, int_choice, item, slider, switch, switch_sub};
use crate::settings_ui::search::SearchableSettings;

/// Progress values (fraction of the episode) offered for "mark as seen".
pub const PROGRESS_VALUES: [f32; 7] = [1.00, 0.95, 0.90, 0.85, 0.80, 0.75, 0.70];

/// Hide-controls time options in milliseconds, 500 to 5000 in steps of 500.
pub fn hide_time_options() -> Vec<(i32, String)> {
    (1..=10).map(|step| (step * 500, format!("{} ms", step * 500))).collect()
}

fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}

/// The progress list. A stored value outside the list is kept, shown as `Custom (N%)` and listed first.
pub fn progress_row(pref: &Pref<f32>) -> PreferenceItem {
    let stored = pref.get();
    let in_list = PROGRESS_VALUES.iter().any(|value| (value - stored).abs() < 1e-6);
    let mut entries: Vec<(String, String)> = Vec::new();
    if !in_list {
        entries.push((format!("{stored}"), format!("Custom ({})", percent(stored))));
    }
    entries.extend(PROGRESS_VALUES.iter().map(|value| (format!("{value}"), percent(*value))));
    let value = PROGRESS_VALUES.iter().copied().find(|v| (v - stored).abs() < 1e-6).unwrap_or(stored);
    let value = format!("{value}");
    let pref = pref.clone();
    let on_change = bind::veto(move |key: String| {
        let result = key
            .parse::<f32>()
            .map_err(|err| err.to_string())
            .and_then(|value| pref.set(value).map_err(|err| err.to_string()));
        async move { result.map(|()| true) }
    });
    PreferenceItem::BasicList {
        common: ItemCommon::bound("At what point to mark the episode as seen"),
        value,
        entries,
        on_change,
    }
}

/// The internal player page, without the rows of tasks 016 to 018.
pub fn internal_rows(prefs: &Prefs) -> Vec<Preference> {
    let p = &prefs.player;
    let hide_times = hide_time_options();
    let hide_options: Vec<(i32, &str)> = hide_times.iter().map(|(code, label)| (*code, label.as_str())).collect();
    let opacity = format!("{}%", p.panel_opacity.get());
    vec![
        item(progress_row(&p.progress_preference)),
        item(switch("Preserve watch position on seen episodes", &p.preserve_watching_position, true)),
        group(
            "Controls",
            vec![
                switch("Allow gesture in panels", &p.allow_gestures, true),
                switch("Show loading circle", &p.show_loading_circle, true),
                switch_sub("Show current chapter indicator", "Only appears if the video has chapters", &p.show_current_chapter, true),
                switch("Remember and switch to the last used brightness", &p.remember_player_brightness, true),
            ],
        ),
        group(
            "Hosters",
            vec![
                switch("Display failed hosters", &p.show_failed_hosters, true),
                switch("Display empty hosters", &p.show_empty_hosters, true),
            ],
        ),
        group(
            "Display",
            vec![
                switch("Fullscreen", &p.player_fullscreen, true),
                switch("Hide player controls when opening the player", &p.hide_controls, true),
                switch("Display volume as percentage", &p.display_vol_per, true),
                switch("Reduce player animation", &p.reduce_motion, true),
                int_choice("Hide player controls time", &p.player_time_to_disappear, &hide_options, true),
                slider("Panel opacity", &p.panel_opacity, 0..=100, Some(opacity.as_str()), true),
            ],
        ),
    ]
}

/// Screen-search contract for the internal player page.
pub struct InternalScreen;

impl SearchableSettings for InternalScreen {
    fn title(&self) -> &'static str {
        "Internal player"
    }

    fn preferences(&self, prefs: &Prefs) -> Vec<Preference> {
        internal_rows(prefs)
    }
}

/// The text of the info row under the gesture options (desktop key names).
pub fn info_text_for_custom() -> &'static str {
    "When a tap gesture is set to \"Custom\", it can be bound through input.conf. The key names are kurobayomi-double-tap-left, kurobayomi-double-tap-center and kurobayomi-double-tap-right."
}

pub fn info_row() -> PreferenceItem {
    info(info_text_for_custom())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn common_of(row: &PreferenceItem) -> &ItemCommon {
        match row {
            PreferenceItem::Switch { common, .. }
            | PreferenceItem::BasicList { common, .. }
            | PreferenceItem::Slider { common, .. } => common,
            _ => panic!("row has no common part"),
        }
    }

    #[test]
    fn hide_time_offers_ten_steps_from_500_to_5000() {
        let options = hide_time_options();
        assert_eq!(options.len(), 10);
        assert_eq!(options[0], (500, "500 ms".to_owned()));
        assert_eq!(options[7], (4000, "4000 ms".to_owned()));
    }

    #[test]
    fn progress_default_is_85_percent_and_lists_the_seven_values() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { value, entries, .. } = progress_row(&prefs.player.progress_preference) else {
            panic!("list")
        };
        assert_eq!(value, "0.85");
        assert_eq!(entries.len(), 7);
        assert_eq!(entries[0].1, "100%");
        assert_eq!(entries[6].1, "70%");
    }

    #[test]
    fn a_stored_progress_outside_the_list_is_kept_as_custom() {
        let prefs = Prefs::in_memory();
        prefs.player.progress_preference.set(0.6).unwrap();
        let PreferenceItem::BasicList { value, entries, .. } = progress_row(&prefs.player.progress_preference) else {
            panic!("list")
        };
        assert_eq!(entries.len(), 8);
        assert_eq!(entries[0].1, "Custom (60%)");
        assert_eq!(value, "0.6");
        assert_eq!(prefs.player.progress_preference.get(), 0.6);
    }

    #[test]
    fn controls_and_hosters_defaults_match_the_spec() {
        let prefs = Prefs::in_memory();
        let tree = internal_rows(&prefs);
        let controls = match &tree[2] {
            Preference::Group(g) => g,
            _ => panic!("group"),
        };
        assert_eq!(controls.title, "Controls");
        assert_eq!(controls.items.len(), 4);
        assert!(!prefs.player.allow_gestures.get());
        assert!(prefs.player.show_loading_circle.get());
        let subtitle = common_of(&controls.items[2]).subtitle.as_deref();
        assert_eq!(subtitle, Some("Only appears if the video has chapters"));
        let hosters = match &tree[3] {
            Preference::Group(g) => g,
            _ => panic!("group"),
        };
        assert_eq!(hosters.items.len(), 2);
        assert!(!prefs.player.show_failed_hosters.get() && !prefs.player.show_empty_hosters.get());
    }

    #[test]
    fn display_group_has_no_cutout_or_status_bar_rows_and_shows_the_opacity() {
        let prefs = Prefs::in_memory();
        let tree = internal_rows(&prefs);
        let Preference::Group(display) = &tree[4] else { panic!("group") };
        let titles: Vec<&str> = display.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles.len(), 6);
        assert!(!titles.iter().any(|t| t.contains("status bar") || t.contains("cutout")));
        let PreferenceItem::Slider { common, value, .. } = &display.items[5] else { panic!("slider") };
        assert_eq!(*value, 60);
        assert_eq!(common.subtitle.as_deref(), Some("60%"));
    }

    #[test]
    fn info_text_names_the_three_double_tap_key_names() {
        assert!(info_text_for_custom().contains("kurobayomi-double-tap-left"));
        assert!(info_text_for_custom().contains("kurobayomi-double-tap-right"));
    }
}
