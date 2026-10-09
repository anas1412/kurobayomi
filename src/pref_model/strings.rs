//! The shared words of the settings widgets, read from the string table (T-47ba-073). Each is a function because the
//! text follows the active language.

use kuro_i18n::{MsgKey, tr};

pub fn ok() -> String {
    tr(MsgKey::action_ok)
}

pub fn cancel() -> String {
    tr(MsgKey::action_cancel)
}

/// The empty label of a multi-select.
pub fn none() -> String {
    tr(MsgKey::none)
}

/// The tooltip of a signed-in tracker row.
pub fn logged_in() -> String {
    tr(MsgKey::login_success)
}

/// Title of the backup group on Data and storage. The onboarding restore highlights this exact title (T-E68).
pub fn label_backup() -> String {
    tr(MsgKey::label_backup)
}

/// The Back button's tooltip.
pub fn back() -> String {
    tr(MsgKey::action_webview_back)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_words_come_from_the_string_table() {
        assert_eq!(ok(), "OK");
        assert_eq!(cancel(), "Cancel");
        assert_eq!(none(), "None");
        assert_eq!(logged_in(), "Logged in");
        assert_eq!(label_backup(), "Backup and restore");
        assert_eq!(back(), "Back");
    }
}
