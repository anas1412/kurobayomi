//! The "Set as default" confirmation (T-39a-009): the title, body and library checkbox of the episode and the season
//! windows, and the writes a confirm sends. Upstream: `EpisodeSettingsDialog.kt`, `SeasonSettingsDialog.kt` and
//! p1-ui-parity 9.3. With the checkbox off only the default keys are written; with it on, the same settings also go to
//! every anime in the library (the library engine does that, see `settings::applied_to_library_message`).

use kuro_i18n::{MsgKey, tr};

use crate::entry_core::message::UserMessage;

/// Which window asks: the episode settings or the season settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Episode,
    Season,
}

/// The headline of the confirmation.
pub fn title(scope: Scope) -> MsgKey {
    match scope {
        Scope::Episode => MsgKey::episode_settings,
        Scope::Season => MsgKey::season_settings,
    }
}

/// The body text. Both windows use the settings sentence of the catalogue.
pub const BODY: MsgKey = MsgKey::confirm_set_chapter_settings;

/// The checkbox text. Both windows use the library sentence of the episode settings.
pub const LIBRARY_CHECKBOX: MsgKey = MsgKey::also_set_episode_settings_for_library;

/// What a confirm writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Confirm {
    /// The default keys are always written.
    pub write_defaults: bool,
    /// Every anime in the library takes the same settings too.
    pub apply_to_library: bool,
}

pub fn confirm(library_checkbox: bool) -> Confirm {
    Confirm { write_defaults: true, apply_to_library: library_checkbox }
}

/// The snackbar after a save with the library checkbox off: once per successful save.
pub fn saved_message(scope: Scope) -> UserMessage {
    match scope {
        Scope::Episode => UserMessage::EpisodeSettingsUpdated,
        Scope::Season => UserMessage::SeasonSettingsUpdated,
    }
}

/// The snackbar after a save with the checkbox on: the saved text and how many entries took the settings (E-DEF-3).
pub fn applied_message(scope: Scope, count: usize) -> UserMessage {
    let saved = tr(match scope {
        Scope::Episode => MsgKey::episode_settings_updated,
        Scope::Season => MsgKey::season_settings_updated,
    });
    UserMessage::Plain(format!("{saved} ({count})"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_scope_has_its_own_saved_text_and_the_library_count_follows_it() {
        assert_eq!(saved_message(Scope::Episode), UserMessage::EpisodeSettingsUpdated);
        assert_eq!(saved_message(Scope::Season), UserMessage::SeasonSettingsUpdated);
        let UserMessage::Plain(text) = applied_message(Scope::Season, 3) else { panic!("a plain message") };
        assert_eq!(text, format!("{} (3)", tr(MsgKey::season_settings_updated)));
        let UserMessage::Plain(text) = applied_message(Scope::Episode, 0) else { panic!("a plain message") };
        assert!(text.starts_with(&tr(MsgKey::episode_settings_updated)) && text.ends_with("(0)"));
    }

    #[test]
    fn the_titles_name_the_window() {
        assert_eq!(title(Scope::Episode), MsgKey::episode_settings);
        assert_eq!(title(Scope::Season), MsgKey::season_settings);
    }

    #[test]
    fn the_checkbox_off_writes_only_the_defaults() {
        assert_eq!(confirm(false), Confirm { write_defaults: true, apply_to_library: false });
    }

    #[test]
    fn the_checkbox_on_also_applies_to_the_library() {
        assert_eq!(confirm(true), Confirm { write_defaults: true, apply_to_library: true });
    }
}
