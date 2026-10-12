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

/// Title of the backup group on Data and storage. The restore link of the empty library highlights this exact title.
pub fn label_backup() -> String {
    tr(MsgKey::label_backup)
}

/// Kurobayomi-only words (T-47ba-074). Not in the Kotlin tree, so they are plain English constants until the string
/// table (area 51) takes them.
pub const NOT_INCLUDED: &str = "Not included";
pub const INCLUDED: &str = "Included";
pub const EXCLUDED: &str = "Excluded";
pub const LOG_OUT_OF_TRACKER: &str = "Log out of this tracker?";
pub const COULD_NOT_SAVE: &str = "Could not save this setting";

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
    }

    #[test]
    fn the_kurobayomi_only_words_are_the_proposal_text() {
        assert_eq!([NOT_INCLUDED, INCLUDED, EXCLUDED], ["Not included", "Included", "Excluded"]);
        assert_eq!(LOG_OUT_OF_TRACKER, "Log out of this tracker?");
        assert_eq!(COULD_NOT_SAVE, "Could not save this setting");
    }
}
