//! "Use as default for all titles" on the anime page (T-39a-009): which flag word it saves, and the message once the
//! library took it. Upstream: `EpisodeSettingsDialog.kt`, `SeasonSettingsDialog.kt` and p1-ui-parity 9.3. The default
//! keys are written and every anime in the library takes the same settings (the runner does that, `ApplyDefaultsToLibrary`).

use kuro_i18n::{MsgKey, tr};

use crate::entry_core::message::UserMessage;

/// Which list the options belong to: the episode list or the season grid.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    Episode,
    Season,
}

/// The snackbar once the library took the settings: the saved text and how many entries took them (E-DEF-3).
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
        let UserMessage::Plain(text) = applied_message(Scope::Season, 3) else { panic!("a plain message") };
        assert_eq!(text, format!("{} (3)", tr(MsgKey::season_settings_updated)));
        let UserMessage::Plain(text) = applied_message(Scope::Episode, 0) else { panic!("a plain message") };
        assert!(text.starts_with(&tr(MsgKey::episode_settings_updated)) && text.ends_with("(0)"));
    }
}
