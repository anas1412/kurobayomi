//! Subtitles page rows (task T-48a-029): preferred languages, whitelist and blacklist. The values are read by the track
//! selector of the player (area 45); this page only stores them.
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::player::language::{language_row, text_row};

pub const LANGUAGE_INFO: &str = "Subtitle language(s) to be selected by default on a video with multiple subtitles, Two- or three-letter languages codes work. Multiple values can be delimited by a comma.";
pub const WHITELIST_INFO: &str = "Whitelist for subtitles. If a whitelist is defined, the first subtitle that contains a whitelisted word will be used. Multiple values can be delimited by a comma.";
pub const BLACKLIST_INFO: &str = "Blacklist for subtitles. If a blacklist is defined, all subtitles that contains a blacklisted word will be filtered out. Multiple values can be delimited by a comma.";

/// The three rows of the Subtitles page, loose as the spec page has them.
pub fn page_rows(prefs: &Prefs) -> Vec<PreferenceItem> {
    let store = prefs.store();
    let language = Pref::new(store.clone(), "pref_subtitle_lang", String::new());
    let whitelist = Pref::new(store.clone(), "pref_subtitle_whitelist", String::new());
    let blacklist = Pref::new(store.clone(), "pref_subtitle_blacklist", String::new());
    vec![
        language_row("Preferred languages", &language, LANGUAGE_INFO),
        text_row("Whitelist", &whitelist, WHITELIST_INFO),
        text_row("Blacklist", &blacklist, BLACKLIST_INFO),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_rows_come_in_spec_order_and_an_empty_default_is_allowed() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let titles: Vec<&str> = rows.iter().map(PreferenceItem::title).collect();
        assert_eq!(titles, ["Preferred languages", "Whitelist", "Blacklist"]);
        let PreferenceItem::EditText { validate, .. } = &rows[1] else { panic!("an edit row") };
        assert!(validate(""), "an empty whitelist is valid");
    }

    #[test]
    fn a_language_row_refuses_an_unknown_code_when_checked() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let PreferenceItem::EditText { validate, error_message, .. } = &rows[0] else { panic!("an edit row") };
        assert!(validate("en,fr"));
        assert!(!validate("en, qq"));
        assert_eq!(error_message.as_ref().map(|m| m("qq")), Some("Invalid code: \"qq\"".to_owned()));
    }
}
