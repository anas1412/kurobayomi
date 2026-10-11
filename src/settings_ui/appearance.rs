//! Theme mode (task T-47a-011), the App theme strip (T-47ba-041), the pure-black visibility rule (T-47a-014 gate) and
//! the start screen (T-48a-005).
//!
//! Theme mode, the App theme strip and the pure-black switch change the look through one path, `theme::apply`
//! (`update_settings`, `picker::change_settings_in`): the palette, the window marks and the GTK dark preference are
//! set together, with no restart.
// Some helpers (the start tab, the row builders used only by the search index) are kept for the index and tests.
use std::rc::Rc;

use chrono::{Local, NaiveDate};
use kuro_prefs::prefs::Prefs;
use kuro_prefs::ui::{NavStyle, StartScreen, ThemeMode};

use crate::pref_model::bind;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::pref_model::widgets::{app_theme, theme_mode};
use crate::settings_ui::rows::{enum_choice, group, switch_sub, switch_with, text_choice, REQUIRES_RESTART};
use crate::theme::apply;

/// The pure-black switch is only offered while the dark theme is in effect.
pub fn amoled_visible(mode: ThemeMode, system_dark: bool) -> bool {
    crate::theme::is_dark(mode, system_dark)
}

/// The Theme mode row: System, Light and Dark as linked toggle buttons (`widgets::theme_mode`). A click stores the
/// mode and applies it through `theme::apply`; a click on the selected segment writes nothing.
pub fn theme_mode_row(prefs: &Prefs) -> PreferenceItem {
    let (pref, title) = (prefs.ui.theme_mode.clone(), "Theme mode");
    PreferenceItem::Custom {
        title: title.to_owned(),
        build: Rc::new(move || {
            let built = theme_mode::row(&crate::pref_model::model::ItemCommon::new(title), pref.clone());
            custom_content(built.row)
        }),
    }
}

/// The App theme row: the strip of preview cards (`widgets::app_theme`).
pub fn app_theme_row(prefs: &Prefs) -> PreferenceItem {
    let store = prefs.store().clone();
    PreferenceItem::Custom {
        title: "App theme".to_owned(),
        build: Rc::new(move || gtk::prelude::Cast::upcast(app_theme::picker_strip(&store))),
    }
}

/// The content of a built list row, taken out of it so it can be a `Custom` row's widget.
fn custom_content(row: gtk::ListBoxRow) -> gtk::Widget {
    use gtk::prelude::*;
    let content = row.child().expect("the row content");
    row.set_child(None::<&gtk::Widget>);
    content
}

/// The navigation styles in list order: the tab each one moves into More.
pub const NAV_STYLES: [(NavStyle, &str); 4] = [
    (NavStyle::MOVE_MANGA_TO_MORE, "Move Manga to the More tab"),
    (NavStyle::MOVE_UPDATES_TO_MORE, "Move Updates to the More tab"),
    (NavStyle::MOVE_HISTORY_TO_MORE, "Move History to the More tab"),
    (NavStyle::MOVE_BROWSE_TO_MORE, "Move Browse to the More tab"),
];

/// The navigation style list (`bottom_rail_nav_style`).
pub fn nav_style_row(prefs: &Prefs) -> PreferenceItem {
    enum_choice("Navigation Style", &prefs.ui.nav_style, &NAV_STYLES, true)
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

/// Each pattern as `<pattern or Default> (<today formatted>)`, the stored value being the pattern (task T-47a-016).
/// Month names and the Default short date follow the UI locale (`kuro_i18n::date`), not a fixed en-US form.
pub fn date_format_options(today: NaiveDate) -> Vec<(String, String)> {
    kuro_i18n::date::previews(today, kuro_i18n::current_locale().tag)
        .into_iter()
        .map(|preview| (preview.pref.to_owned(), preview.label))
        .collect()
}

/// The date format list. A stored pattern outside the offered formats shows the default.
pub fn date_format_row(prefs: &Prefs, today: NaiveDate) -> PreferenceItem {
    text_choice("Date format", &prefs.ui.date_format, &date_format_options(today), true)
}

/// The subtitle of the relative timestamps switch: `"Today" instead of "<today formatted>"`.
pub fn relative_subtitle(today: NaiveDate) -> String {
    format!("\"Today\" instead of \"{}\"", kuro_i18n::date::DateFormat::LocaleShort.format(today, kuro_i18n::current_locale().tag))
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

/// The rows of the Appearance page this module binds to the typed preferences: Theme mode, the App theme strip, the
/// pure-black switch (only while the dark theme is in effect) and the Display rows. The rest of the page stays as the
/// spec builds it.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    let today = Local::now().date_naive();
    let mode = prefs.ui.theme_mode.get();
    vec![
        group(
            "Theme",
            vec![theme_mode_row(prefs), app_theme_row(prefs), amoled_row(prefs, mode, apply::system_is_dark())],
        ),
        group(
            "Display",
            vec![start_screen_row(prefs), nav_style_row(prefs), date_format_row(prefs, today), relative_time_row(prefs, today)],
        ),
    ]
}

/// What the Appearance page depends on besides the stored values: the mode and whether the desktop is dark. The page
/// is rebuilt when it changes, so the pure-black row appears and goes with the mode.
pub fn rule_state(prefs: &Prefs) -> (ThemeMode, bool) {
    (prefs.ui.theme_mode.get(), apply::system_is_dark())
}

/// The pure-black switch. It is enabled only while the dark theme is in effect; a disabled row is dropped by the page
/// merge together with its spec twin, so the switch is not shown otherwise. Turning it on or off applies the scheme's
/// AMOLED overlay through `theme::apply`, the same path as the mode.
pub fn amoled_row(prefs: &Prefs, mode: ThemeMode, system_dark: bool) -> PreferenceItem {
    let veto = bind::veto(move |on: bool| {
        apply::update_settings(|settings| settings.amoled = on);
        async { Ok(true) }
    });
    switch_with("Pure black dark mode", None, &prefs.ui.theme_dark_amoled, amoled_visible(mode, system_dark), veto)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_and_dark_ignore_the_desktop_and_system_follows_it() {
        assert!(!amoled_visible(ThemeMode::LIGHT, true));
        assert!(amoled_visible(ThemeMode::DARK, false));
        assert!(amoled_visible(ThemeMode::SYSTEM, true));
        assert!(!amoled_visible(ThemeMode::SYSTEM, false));
    }

    #[test]
    fn the_pure_black_row_is_absent_outside_dark_and_stores_its_switch() {
        let prefs = Prefs::in_memory();
        assert!(!amoled_row(&prefs, ThemeMode::LIGHT, true).enabled());
        assert!(!amoled_row(&prefs, ThemeMode::SYSTEM, false).enabled());
        let row = amoled_row(&prefs, ThemeMode::DARK, false);
        assert!(row.enabled());
        assert_eq!(row.title(), "Pure black dark mode");
        let PreferenceItem::Switch { pref, .. } = row else { panic!("switch") };
        assert_eq!(pref.key(), "pref_theme_dark_amoled_key");
    }

    // T-47ba-040: the segmented widget is on the page, with the App theme strip below it and pure black last.
    #[test]
    fn the_theme_group_holds_mode_strip_and_pure_black_in_that_order() {
        let prefs = Prefs::in_memory();
        let titles = |prefs: &Prefs| -> Vec<String> {
            let tree = page_rows(prefs);
            let Preference::Group(theme) = &tree[0] else { panic!("group") };
            theme.items.iter().filter(|i| i.enabled()).map(|i| i.title().to_owned()).collect()
        };
        prefs.ui.theme_mode.set(ThemeMode::LIGHT).unwrap();
        assert_eq!(titles(&prefs), ["Theme mode", "App theme"], "pure black waits for dark");
        prefs.ui.theme_mode.set(ThemeMode::DARK).unwrap();
        assert_eq!(titles(&prefs), ["Theme mode", "App theme", "Pure black dark mode"]);
        let tree = page_rows(&prefs);
        let Preference::Group(theme) = &tree[0] else { panic!("group") };
        assert!(matches!(theme.items[0], PreferenceItem::Custom { .. }) && matches!(theme.items[1], PreferenceItem::Custom { .. }));
    }

    #[test]
    fn the_page_rule_state_follows_the_stored_mode() {
        let prefs = Prefs::in_memory();
        let before = rule_state(&prefs);
        prefs.ui.theme_mode.set(ThemeMode::DARK).unwrap();
        assert_ne!(before, rule_state(&prefs));
    }

    // T-47a-011, T-47ba-041: with the merged spec page, Theme mode and App theme (spec chips are skipped) both show,
    // in their place before Pure black.
    #[test]
    fn the_merged_page_shows_mode_strip_and_pure_black_in_the_theme_group() {
        let prefs = Prefs::in_memory();
        prefs.ui.theme_mode.set(ThemeMode::DARK).unwrap();
        let tree = crate::settings_ui::route::tree_with(
            "settings_appearance",
            &prefs,
            &[],
            &[],
            crate::settings_ui::browse::RepoCounts::default(),
        );
        let Some(Preference::Group(theme)) = tree.iter().find(|e| matches!(e, Preference::Group(g) if g.title == "Theme")) else {
            panic!("the Theme group")
        };
        let titles: Vec<&str> = theme.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Theme mode", "App theme", "Pure black dark mode"]);
        prefs.ui.theme_mode.set(ThemeMode::LIGHT).unwrap();
        let tree = crate::settings_ui::route::tree_with("settings_appearance", &prefs, &[], &[], crate::settings_ui::browse::RepoCounts::default());
        let Some(Preference::Group(theme)) = tree.iter().find(|e| matches!(e, Preference::Group(g) if g.title == "Theme")) else {
            panic!("the Theme group")
        };
        let titles: Vec<&str> = theme.items.iter().map(|i| i.title()).collect();
        assert_eq!(titles, ["Theme mode", "App theme"], "in Light the pure-black row and its spec twin are both gone");
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
        assert_eq!(labels[0], "Default (10/8/26)");
        assert_eq!(labels[1], "MM/dd/yy (10/08/26)");
        assert_eq!(labels[2], "dd/MM/yy (08/10/26)");
        assert_eq!(labels[3], "yyyy-MM-dd (2026-10-08)");
        assert_eq!(labels[4], "dd MMM yyyy (08 Oct 2026)");
        assert_eq!(labels[5], "MMM dd, yyyy (Oct 08, 2026)");
    }

    #[test]
    fn relative_subtitle_names_today_as_the_short_date_and_the_switch_is_on_by_default() {
        assert_eq!(relative_subtitle(day()), "\"Today\" instead of \"10/8/26\"");
        let prefs = Prefs::in_memory();
        let row = relative_time_row(&prefs, day());
        assert_eq!(row.title(), "Relative timestamps");
        let PreferenceItem::Switch { common, pref, .. } = row else { panic!("switch") };
        assert_eq!(common.subtitle.as_deref(), Some("\"Today\" instead of \"10/8/26\""));
        assert!(pref.get());
    }

    #[test]
    fn only_the_start_screen_says_it_needs_a_restart_and_pure_black_applies_live() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { common, .. } = start_screen_row(&prefs) else { panic!("list") };
        assert_eq!(common.subtitle.as_deref(), Some(REQUIRES_RESTART));
        let PreferenceItem::Switch { common, .. } = amoled_row(&prefs, ThemeMode::DARK, false) else {
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
        assert_eq!(titles, ["Start Screen", "Navigation Style", "Date format", "Relative timestamps"]);
    }
}

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn the_navigation_style_list_offers_the_four_styles_and_stores_the_pick() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { entries, value, .. } = nav_style_row(&prefs) else { panic!("list") };
        assert_eq!(value, "MOVE_HISTORY_TO_MORE");
        assert_eq!(entries[0].1, "Move Manga to the More tab");
        assert_eq!(entries.len(), 4);
    }
}
