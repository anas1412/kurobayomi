//! `[ui]` preferences (Aniyomi `UiPreferences`), the UI enums they select, and the theme choice.
//!
//! The enums are stored by their Kotlin constant names. Titles are string-resource keys, which
//! the i18n area resolves; the GTK application is not touched here.

use crate::pref_enum;
use crate::registry::define_prefs;

pref_enum! {
    /// `pref_theme_mode_key`.
    pub enum ThemeMode { LIGHT, DARK, SYSTEM }
}

pref_enum! {
    /// `pref_app_theme`. The three deprecated themes stay valid stored values but are not offered.
    pub enum AppTheme {
        DEFAULT, MONET, CLOUDFLARE, COTTONCANDY, DOOM, GREEN_APPLE, LAVENDER, MATRIX,
        MIDNIGHT_DUSK, MOCHA, SAPPHIRE, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE,
        TIDAL_WAVE, YINYANG, YOTSUBA, MONOCHROME, DARK_BLUE, HOT_PINK, BLUE,
    }
}

impl AppTheme {
    /// The string-resource key of the picker title, or `None` for a deprecated theme.
    pub fn title_key(self) -> Option<&'static str> {
        Some(match self {
            Self::DEFAULT => "label_default",
            Self::MONET => "theme_monet",
            Self::CLOUDFLARE => "theme_cloudflare",
            Self::COTTONCANDY => "theme_cottoncandy",
            Self::DOOM => "theme_doom",
            Self::GREEN_APPLE => "theme_greenapple",
            Self::LAVENDER => "theme_lavender",
            Self::MATRIX => "theme_matrix",
            Self::MIDNIGHT_DUSK => "theme_midnightdusk",
            Self::MOCHA => "theme_mocha",
            Self::SAPPHIRE => "theme_sapphire",
            Self::NORD => "theme_nord",
            Self::STRAWBERRY_DAIQUIRI => "theme_strawberrydaiquiri",
            Self::TAKO => "theme_tako",
            Self::TEALTURQUOISE => "theme_tealturquoise",
            Self::TIDAL_WAVE => "theme_tidalwave",
            Self::YINYANG => "theme_yinyang",
            Self::YOTSUBA => "theme_yotsuba",
            Self::MONOCHROME => "theme_monochrome",
            Self::DARK_BLUE | Self::HOT_PINK | Self::BLUE => return None,
        })
    }
}

/// A top-level destination (Kotlin `Tab`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    AnimeLibrary,
    MangaLibrary,
    Updates,
    History,
    Browse,
    More,
}

pref_enum! {
    /// `bottom_rail_nav_style`: which destination moves into More.
    pub enum NavStyle {
        MOVE_MANGA_TO_MORE, MOVE_UPDATES_TO_MORE, MOVE_HISTORY_TO_MORE, MOVE_BROWSE_TO_MORE,
    }
}

impl NavStyle {
    /// The destination this style moves into More.
    pub fn more_tab(self) -> Tab {
        match self {
            Self::MOVE_MANGA_TO_MORE => Tab::MangaLibrary,
            Self::MOVE_UPDATES_TO_MORE => Tab::Updates,
            Self::MOVE_HISTORY_TO_MORE => Tab::History,
            Self::MOVE_BROWSE_TO_MORE => Tab::Browse,
        }
    }

    /// The visible navigation tabs: all six destinations minus [`NavStyle::more_tab`].
    pub fn tabs(self) -> Vec<Tab> {
        [
            Tab::AnimeLibrary,
            Tab::MangaLibrary,
            Tab::Updates,
            Tab::History,
            Tab::Browse,
            Tab::More,
        ]
        .into_iter()
        .filter(|tab| *tab != self.more_tab())
        .collect()
    }
}

pref_enum! {
    /// `start_screen`: the tab shown at launch.
    pub enum StartScreen { ANIME, MANGA, UPDATES, HISTORY, BROWSE }
}

impl StartScreen {
    /// The destination this screen opens.
    pub fn tab(self) -> Tab {
        match self {
            Self::ANIME => Tab::AnimeLibrary,
            Self::MANGA => Tab::MangaLibrary,
            Self::UPDATES => Tab::Updates,
            Self::HISTORY => Tab::History,
            Self::BROWSE => Tab::Browse,
        }
    }
}

pref_enum! {
    /// `tablet_ui_mode`. Kept for backups; it has no effect on desktop.
    pub enum TabletUiMode { AUTOMATIC, ALWAYS, LANDSCAPE, NEVER }
}

/// What the GTK layer must do for a theme mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThemeChoice {
    /// Set `gtk-application-prefer-dark-theme` to false.
    Light,
    /// Set `gtk-application-prefer-dark-theme` to true.
    Dark,
    /// Leave the choice to the desktop colour scheme, and follow its changes.
    FollowSystem,
}

/// The theme chosen by `mode`. Only the choice is returned; GTK is not called.
pub fn apply_theme(mode: ThemeMode) -> ThemeChoice {
    match mode {
        ThemeMode::LIGHT => ThemeChoice::Light,
        ThemeMode::DARK => ThemeChoice::Dark,
        ThemeMode::SYSTEM => ThemeChoice::FollowSystem,
    }
}

define_prefs! {
    /// `[ui]` section: the eight keys of Aniyomi `UiPreferences`.
    pub struct UiPrefs in "ui" {
        theme_mode: ThemeMode = "pref_theme_mode_key" => ThemeMode::SYSTEM;
        app_theme: AppTheme = "pref_app_theme" => AppTheme::DEFAULT;
        theme_dark_amoled: bool = "pref_theme_dark_amoled_key" => false;
        relative_time: bool = "relative_time_v2" => true;
        date_format: String = "app_date_format" => String::new();
        tablet_ui_mode: TabletUiMode = "tablet_ui_mode" => TabletUiMode::AUTOMATIC;
        start_screen: StartScreen = "start_screen" => StartScreen::ANIME;
        nav_style: NavStyle = "bottom_rail_nav_style" => NavStyle::MOVE_HISTORY_TO_MORE;
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::paths::Paths;

    use super::*;
    use crate::toml_backend::config_file;
    use crate::settings_load::open;
    use crate::store::PrefStore;
    use crate::testutil::{TestDir, TestPaths, assert_section_registered, read_toml};
    use crate::values::PrefValue;

    #[test]
    fn keys_are_the_aniyomi_keys_in_their_ui_fields() {
        assert_eq!(
            UiPrefs::FIELDS,
            &[
                ("theme_mode", "pref_theme_mode_key"),
                ("app_theme", "pref_app_theme"),
                ("theme_dark_amoled", "pref_theme_dark_amoled_key"),
                ("relative_time", "relative_time_v2"),
                ("date_format", "app_date_format"),
                ("tablet_ui_mode", "tablet_ui_mode"),
                ("start_screen", "start_screen"),
                ("nav_style", "bottom_rail_nav_style"),
            ]
        );
        assert_eq!(UiPrefs::TABLE, "ui");
    }

    #[test]
    fn every_field_is_registered_under_the_ui_table() {
        let ui = UiPrefs::new(PrefStore::in_memory());
        assert_section_registered(UiPrefs::TABLE, UiPrefs::FIELDS, &ui.defaults());
    }

    #[test]
    fn defaults_are_the_plan_defaults() {
        let ui = UiPrefs::new(PrefStore::in_memory());
        assert_eq!(ui.theme_mode.get(), ThemeMode::SYSTEM);
        assert_eq!(ui.app_theme.get(), AppTheme::DEFAULT);
        assert!(!ui.theme_dark_amoled.get());
        assert!(ui.relative_time.get());
        assert_eq!(ui.date_format.get(), "");
        assert_eq!(ui.tablet_ui_mode.get(), TabletUiMode::AUTOMATIC);
        assert_eq!(ui.start_screen.get(), StartScreen::ANIME);
        assert_eq!(ui.nav_style.get(), NavStyle::MOVE_HISTORY_TO_MORE);
    }

    #[test]
    fn enums_are_stored_by_their_constant_names() {
        let store = PrefStore::in_memory();
        let ui = UiPrefs::new(store.clone());
        ui.theme_mode.set(ThemeMode::DARK).unwrap();
        ui.nav_style.set(NavStyle::MOVE_BROWSE_TO_MORE).unwrap();
        assert_eq!(
            store.get_raw("pref_theme_mode_key"),
            Some(PrefValue::Str("DARK".into()))
        );
        assert_eq!(
            store.get_raw("bottom_rail_nav_style"),
            Some(PrefValue::Str("MOVE_BROWSE_TO_MORE".into()))
        );
    }

    #[test]
    fn an_unknown_stored_name_gives_the_default_and_stays_stored() {
        let store = PrefStore::in_memory();
        store.set_raw("start_screen", PrefValue::Str("NOPE".into()));
        let ui = UiPrefs::new(store.clone());
        assert_eq!(ui.start_screen.get(), StartScreen::ANIME);
        assert!(ui.start_screen.is_set());
    }

    #[test]
    fn app_theme_has_the_22_names_and_the_three_deprecated_have_no_title() {
        assert_eq!(AppTheme::ALL.len(), 22);
        assert_eq!(AppTheme::ALL[0], AppTheme::DEFAULT);
        assert_eq!(AppTheme::ALL[18], AppTheme::MONOCHROME);
        assert_eq!(AppTheme::ALL[21], AppTheme::BLUE);
        assert_eq!(AppTheme::DEFAULT.title_key(), Some("label_default"));
        assert_eq!(AppTheme::MONOCHROME.title_key(), Some("theme_monochrome"));
        for deprecated in [AppTheme::DARK_BLUE, AppTheme::HOT_PINK, AppTheme::BLUE] {
            assert_eq!(deprecated.title_key(), None);
        }
    }

    #[test]
    fn nav_style_shows_six_destinations_minus_the_moved_one() {
        let tabs = NavStyle::MOVE_HISTORY_TO_MORE.tabs();
        assert_eq!(
            tabs,
            vec![
                Tab::AnimeLibrary,
                Tab::MangaLibrary,
                Tab::Updates,
                Tab::Browse,
                Tab::More,
            ]
        );
        assert_eq!(
            NavStyle::MOVE_MANGA_TO_MORE.tabs(),
            vec![
                Tab::AnimeLibrary,
                Tab::Updates,
                Tab::History,
                Tab::Browse,
                Tab::More,
            ]
        );
    }

    #[test]
    fn start_screens_open_their_tabs() {
        assert_eq!(StartScreen::ANIME.tab(), Tab::AnimeLibrary);
        assert_eq!(StartScreen::MANGA.tab(), Tab::MangaLibrary);
        assert_eq!(StartScreen::UPDATES.tab(), Tab::Updates);
        assert_eq!(StartScreen::HISTORY.tab(), Tab::History);
        assert_eq!(StartScreen::BROWSE.tab(), Tab::Browse);
    }

    #[test]
    fn theme_modes_choose_the_gtk_setting() {
        assert_eq!(apply_theme(ThemeMode::LIGHT), ThemeChoice::Light);
        assert_eq!(apply_theme(ThemeMode::DARK), ThemeChoice::Dark);
        assert_eq!(apply_theme(ThemeMode::SYSTEM), ThemeChoice::FollowSystem);
    }

    #[test]
    fn ui_values_round_trip_through_config_toml_in_the_ui_table() {
        let paths = TestPaths(TestDir::new("ui-round-trip"));
        {
            let store = open(&paths).unwrap();
            let ui = UiPrefs::new(store.clone());
            ui.theme_mode.set(ThemeMode::LIGHT).unwrap();
            ui.date_format.set("yyyy-MM-dd".into()).unwrap();
            ui.relative_time.set(false).unwrap();
            store.flush().unwrap();
        }
        let store = open(&paths).unwrap();
        let ui = UiPrefs::new(store);
        assert_eq!(ui.theme_mode.get(), ThemeMode::LIGHT);
        assert_eq!(ui.date_format.get(), "yyyy-MM-dd");
        assert!(!ui.relative_time.get());
        let doc = read_toml(&config_file(&paths.config_dir()));
        assert_eq!(doc["ui"]["theme_mode"].as_str(), Some("LIGHT"));
        assert_eq!(doc["ui"]["date_format"].as_str(), Some("yyyy-MM-dd"));
        assert_eq!(doc["ui"]["relative_time"].as_bool(), Some(false));
    }
}
