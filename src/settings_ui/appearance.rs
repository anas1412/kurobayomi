//! Theme mode (task T-47a-011), the pure-black visibility rule (T-47a-014 gate) and the start screen (T-48a-005).
// Not reachable from `route.rs` yet: the Theme mode chips and the dark-only pure-black rule wait for the theme area; only the start screen and pure-black rows are routed.
#![allow(dead_code)]
use chrono::{Local, NaiveDate};
use kuro_prefs::datefmt;
use kuro_prefs::prefs::Prefs;
use kuro_prefs::ui::{StartScreen, ThemeMode};

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::rows::{enum_choice, group, switch, switch_sub, text_choice, REQUIRES_RESTART};

/// XDG portal `color-scheme` value for "prefer dark" (0 = no preference, 2 = light).
pub const PORTAL_PREFER_DARK: u32 = 1;

/// Whether the dark theme is in effect. LIGHT and DARK force it; SYSTEM follows the portal, and "no preference" or
/// a missing portal means light (the GTK default).
pub fn effective_dark(mode: ThemeMode, portal_color_scheme: Option<u32>) -> bool {
    match mode {
        ThemeMode::LIGHT => false,
        ThemeMode::DARK => true,
        ThemeMode::SYSTEM => portal_color_scheme == Some(PORTAL_PREFER_DARK),
    }
}

/// The pure-black switch is only offered while the dark theme is in effect.
pub fn amoled_visible(mode: ThemeMode, portal_color_scheme: Option<u32>) -> bool {
    effective_dark(mode, portal_color_scheme)
}

/// Theme mode list: System, Light, Dark. Stored by constant name.
pub fn theme_mode_row(prefs: &Prefs) -> PreferenceItem {
    enum_choice(
        "Theme",
        &prefs.ui.theme_mode,
        &[(ThemeMode::SYSTEM, "System"), (ThemeMode::LIGHT, "Light"), (ThemeMode::DARK, "Dark")],
        true,
    )
}

/// The five start screens in list order, with their titles.
pub const START_SCREENS: [(StartScreen, &str); 5] = [
    (StartScreen::ANIME, "Anime"),
    (StartScreen::MANGA, "Manga"),
    (StartScreen::UPDATES, "Updates"),
    (StartScreen::HISTORY, "History"),
    (StartScreen::BROWSE, "Browse"),
];

/// The start screen list. It applies at the next launch, so its subtitle says so (task T-47a-010).
pub fn start_screen_row(prefs: &Prefs) -> PreferenceItem {
    let mut row = enum_choice("Start Screen", &prefs.ui.start_screen, &START_SCREENS, true);
    if let PreferenceItem::BasicList { common, .. } = &mut row {
        common.subtitle = Some(REQUIRES_RESTART.to_owned());
    }
    row
}

/// The `app_date_format` patterns in list order; the empty one is the short date.
pub const DATE_PATTERNS: [&str; 6] = ["", "MM/dd/yy", "dd/MM/yy", "yyyy-MM-dd", "dd MMM yyyy", "MMM dd, yyyy"];

/// Each pattern as `<pattern or Default> (<today formatted>)`, the stored value being the pattern (task T-47a-016).
pub fn date_format_options(today: NaiveDate) -> Vec<(String, String)> {
    DATE_PATTERNS
        .iter()
        .map(|pattern| {
            let name = if pattern.is_empty() { "Default" } else { pattern };
            (pattern.to_string(), format!("{name} ({})", datefmt::format(pattern, today)))
        })
        .collect()
}

/// The date format list. A stored pattern outside [`DATE_PATTERNS`] shows the default.
pub fn date_format_row(prefs: &Prefs, today: NaiveDate) -> PreferenceItem {
    text_choice("Date format", &prefs.ui.date_format, &date_format_options(today), true)
}

/// The subtitle of the relative timestamps switch: `"Today" instead of "<today formatted>"`.
pub fn relative_subtitle(today: NaiveDate) -> String {
    format!("\"Today\" instead of \"{}\"", datefmt::format("", today))
}

/// The relative timestamps switch (`relative_time_v2`).
pub fn relative_time_row(prefs: &Prefs, today: NaiveDate) -> PreferenceItem {
    switch_sub("Relative timestamps", &relative_subtitle(today), &prefs.ui.relative_time, true)
}

/// The nav destination a start screen opens: its tab id in the app window and, for the two libraries, the page of the
/// Anime | Manga switcher inside it.
pub fn nav_target(screen: StartScreen) -> (&'static str, Option<&'static str>) {
    match screen {
        StartScreen::ANIME => ("library", Some("anime")),
        StartScreen::MANGA => ("library", Some("manga")),
        StartScreen::UPDATES => ("updates", None),
        StartScreen::HISTORY => ("history", None),
        StartScreen::BROWSE => ("browse", None),
    }
}

/// Selects the tab `prefs.ui.start_screen` names (task T-48a-005). A tab the window does not have is left alone.
pub fn open_start_tab(tabs: &gtk::Stack, prefs: &Prefs) {
    let (tab, kind) = nav_target(prefs.ui.start_screen.get());
    let Some(page) = tabs.child_by_name(tab) else { return };
    tabs.set_visible_child(&page);
    if let Some(kind) = kind {
        crate::settings_ui::view::show_kind(&page, kind);
    }
}

/// The rows of the Appearance page this module binds to the typed preferences: the pure-black switch and the start
/// screen list. The rest of the page stays as the spec builds it.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    let today = Local::now().date_naive();
    vec![
        group("Theme", vec![switch("Pure black dark mode", &prefs.ui.theme_dark_amoled, true)]),
        group(
            "Display",
            vec![start_screen_row(prefs), date_format_row(prefs, today), relative_time_row(prefs, today)],
        ),
    ]
}

/// The pure-black switch, offered only while the dark theme is in effect (none otherwise).
pub fn amoled_row(prefs: &Prefs, mode: ThemeMode, portal_color_scheme: Option<u32>) -> Option<PreferenceItem> {
    amoled_visible(mode, portal_color_scheme).then(|| switch("Pure black dark mode", &prefs.ui.theme_dark_amoled, true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_ignore_the_portal() {
        assert!(!effective_dark(ThemeMode::LIGHT, Some(PORTAL_PREFER_DARK)));
        assert!(effective_dark(ThemeMode::DARK, Some(2)));
        assert!(effective_dark(ThemeMode::DARK, None));
    }

    #[test]
    fn system_follows_the_portal_and_falls_back_to_light() {
        assert!(effective_dark(ThemeMode::SYSTEM, Some(1)));
        assert!(!effective_dark(ThemeMode::SYSTEM, Some(2)));
        assert!(!effective_dark(ThemeMode::SYSTEM, Some(0)));
        assert!(!effective_dark(ThemeMode::SYSTEM, None));
    }

    #[test]
    fn pure_black_is_offered_only_in_dark() {
        assert!(!amoled_visible(ThemeMode::LIGHT, Some(1)));
        assert!(amoled_visible(ThemeMode::SYSTEM, Some(1)));
    }

    #[test]
    fn the_pure_black_row_is_absent_outside_dark_and_stores_its_switch() {
        let prefs = Prefs::in_memory();
        assert!(amoled_row(&prefs, ThemeMode::LIGHT, Some(1)).is_none());
        assert!(amoled_row(&prefs, ThemeMode::SYSTEM, Some(2)).is_none());
        let Some(row) = amoled_row(&prefs, ThemeMode::DARK, None) else { panic!("visible in dark") };
        assert_eq!(row.title(), "Pure black dark mode");
    }

    #[test]
    fn theme_row_defaults_to_system_and_offers_three_modes() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { value, entries, .. } = theme_mode_row(&prefs) else { panic!("list") };
        assert_eq!(value, "SYSTEM");
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn each_start_screen_maps_to_a_nav_tab_and_the_libraries_pick_their_kind() {
        assert_eq!(nav_target(StartScreen::ANIME), ("library", Some("anime")));
        assert_eq!(nav_target(StartScreen::MANGA), ("library", Some("manga")));
        assert_eq!(nav_target(StartScreen::UPDATES), ("updates", None));
        assert_eq!(nav_target(StartScreen::HISTORY), ("history", None));
        assert_eq!(nav_target(StartScreen::BROWSE), ("browse", None));
        crate::ui::with_spec(|spec| {
            for (screen, _) in START_SCREENS {
                let (tab, _) = nav_target(screen);
                assert!(spec.nav.iter().any(|n| n.id == tab), "{tab} is a nav destination");
            }
        });
        let tabs: Vec<_> = START_SCREENS.iter().map(|(s, _)| s.tab()).collect();
        assert_eq!(tabs.len(), 5, "StartScreen::tab names a destination for every start screen");
    }

    #[test]
    fn the_page_rows_have_the_spec_titles_and_the_start_screen_reads_its_preference() {
        let prefs = Prefs::in_memory();
        prefs.ui.start_screen.set(StartScreen::UPDATES).unwrap();
        let tree = page_rows(&prefs);
        let Preference::Group(display) = &tree[1] else { panic!("group") };
        assert_eq!(display.title, "Display");
        let PreferenceItem::BasicList { common, value, .. } = &display.items[0] else { panic!("list") };
        assert_eq!((common.title.as_str(), value.as_str()), ("Start Screen", "UPDATES"));
        crate::ui::with_spec(|spec| {
            let screen = spec.screen("settings_appearance").expect("screen");
            let has = |group: &str, title: &str| {
                screen.groups.iter().any(|g| {
                    g.title.as_deref() == Some(group) && g.items.iter().any(|i| crate::pref_model::spec_convert::item_title(i) == title)
                })
            };
            assert!(has("Theme", "Pure black dark mode") && has("Display", "Start Screen"));
        });
    }

    #[test]
    fn start_screen_labels_match_the_spec_and_default_is_anime() {
        let labels: Vec<&str> = START_SCREENS.iter().map(|(_, l)| *l).collect();
        assert_eq!(labels, ["Anime", "Manga", "Updates", "History", "Browse"]);
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { value, .. } = start_screen_row(&prefs) else { panic!("list") };
        assert_eq!(value, "ANIME");
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 8).expect("date")
    }

    #[test]
    fn date_labels_show_the_pattern_or_default_and_today_in_that_pattern() {
        let labels: Vec<String> = date_format_options(day()).into_iter().map(|(_, label)| label).collect();
        assert_eq!(labels[0], "Default (10/08/26)");
        assert_eq!(labels[1], "MM/dd/yy (10/08/26)");
        assert_eq!(labels[2], "dd/MM/yy (08/10/26)");
        assert_eq!(labels[3], "yyyy-MM-dd (2026-10-08)");
        assert_eq!(labels[4], "dd MMM yyyy (08 Oct 2026)");
        assert_eq!(labels[5], "MMM dd, yyyy (Oct 08, 2026)");
    }

    #[test]
    fn the_date_list_stores_the_pattern_and_defaults_to_the_short_date() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { value, entries, .. } = date_format_row(&prefs, day()) else { panic!("list") };
        assert_eq!(value, "", "the empty pattern is the default");
        assert_eq!(entries.len(), DATE_PATTERNS.len());
        prefs.ui.date_format.set("yyyy-MM-dd".into()).unwrap();
        let PreferenceItem::BasicList { value, .. } = date_format_row(&prefs, day()) else { panic!("list") };
        assert_eq!(value, "yyyy-MM-dd");
        prefs.ui.date_format.set("not-a-pattern".into()).unwrap();
        let PreferenceItem::BasicList { value, .. } = date_format_row(&prefs, day()) else { panic!("list") };
        assert_eq!(value, "", "a stored text the list does not offer shows the default");
    }

    #[test]
    fn relative_subtitle_names_today_as_the_short_date_and_the_switch_is_on_by_default() {
        assert_eq!(relative_subtitle(day()), "\"Today\" instead of \"10/08/26\"");
        let prefs = Prefs::in_memory();
        let row = relative_time_row(&prefs, day());
        assert_eq!(row.title(), "Relative timestamps");
        let PreferenceItem::Switch { common, pref, .. } = row else { panic!("switch") };
        assert_eq!(common.subtitle.as_deref(), Some("\"Today\" instead of \"10/08/26\""));
        assert!(pref.get());
    }

    #[test]
    fn only_the_start_screen_says_it_needs_a_restart_and_pure_black_applies_live() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { common, .. } = start_screen_row(&prefs) else { panic!("list") };
        assert_eq!(common.subtitle.as_deref(), Some(REQUIRES_RESTART));
        let PreferenceItem::Switch { common, .. } = amoled_row(&prefs, ThemeMode::DARK, None).expect("dark") else {
            panic!("switch")
        };
        assert_eq!(common.subtitle, None, "pure black is applied live");
    }

    #[test]
    fn the_display_group_holds_the_start_screen_and_both_timestamp_rows() {
        let prefs = Prefs::in_memory();
        let tree = page_rows(&prefs);
        let Preference::Group(display) = &tree[1] else { panic!("group") };
        let titles: Vec<&str> = display.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Start Screen", "Date format", "Relative timestamps"]);
    }
}
