//! Language picker data and the Settings row subtitle (plan 51-i18n T-51a-011, T-51a-013,
//! T-51a-021). The screen itself is GTK code in the app; this module only builds its rows.

use crate::names;
use crate::{MsgKey, tr};

/// One row of the language list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PickerRow {
    /// The `app_language` value: a tag, or `""` for Default.
    pub value: String,
    /// The native name (or `Default`).
    pub primary: String,
    /// The English name. Empty for Default.
    pub secondary: String,
    pub checked: bool,
}

/// The effective saved choice: the saved tag when it is offered, otherwise `""` (Default). A
/// saved tag that is no longer offered is ignored (test L11).
fn selected<'a>(saved: &'a str, offered: &[&str]) -> &'a str {
    if !saved.is_empty() && offered.contains(&saved) {
        saved
    } else {
        ""
    }
}

/// The picker rows: Default first, then the offered tags sorted by native name in code-point
/// order (T-51a-011). `saved` is the `app_language` preference.
pub fn rows(saved: &str, offered: &[&str]) -> Vec<PickerRow> {
    let chosen = selected(saved, offered);
    let mut languages: Vec<PickerRow> = offered
        .iter()
        .filter(|tag| !tag.is_empty())
        .map(|tag| PickerRow {
            value: tag.to_string(),
            primary: names::native_name(tag),
            secondary: names::english_name(tag),
            checked: *tag == chosen,
        })
        .collect();
    languages.sort_by(|a, b| a.primary.cmp(&b.primary));
    let mut out = Vec::with_capacity(languages.len() + 1);
    out.push(PickerRow {
        value: String::new(),
        primary: tr(MsgKey::label_default),
        secondary: String::new(),
        checked: chosen.is_empty(),
    });
    out.extend(languages);
    out
}

/// The Settings row subtitle (T-51a-021): the saved language's native name, or Default.
pub fn settings_subtitle(saved: &str, offered: &[&str]) -> String {
    match selected(saved, offered) {
        "" => tr(MsgKey::label_default),
        tag => names::native_name(tag),
    }
}

/// The inline note shown after a language change (T-51a-013, T-51a-015). The change takes
/// effect on restart.
pub fn restart_note() -> String {
    tr(MsgKey::requires_app_restart)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFFERED: &[&str] = &["en", "de", "ar", "zh-TW", "ka-GE", "tr"];

    #[test]
    fn default_row_comes_first_with_its_label() {
        // D6
        let rows = rows("", OFFERED);
        assert_eq!(rows[0].value, "");
        assert_eq!(rows[0].primary, "Default");
        assert!(rows[0].checked);
        assert_eq!(rows.len(), OFFERED.len() + 1);
    }

    #[test]
    fn rows_after_default_sort_by_native_name() {
        // Code-point order on the native names: ASCII before Arabic and Georgian.
        let rows = rows("", OFFERED);
        let natives: Vec<&str> = rows[1..].iter().map(|r| r.primary.as_str()).collect();
        let mut sorted = natives.clone();
        sorted.sort();
        assert_eq!(natives, sorted);
        // "Deutsch" sorts before "English".
        assert_eq!(rows[1].value, "de");
    }

    #[test]
    fn each_row_has_native_and_english_names() {
        let rows = rows("", OFFERED);
        let tr_row = rows.iter().find(|r| r.value == "tr").expect("tr row");
        assert_eq!(tr_row.primary, "Türkçe");
        assert_eq!(tr_row.secondary, "Turkish");
    }

    #[test]
    fn saved_choice_is_checked() {
        let rows = rows("de", OFFERED);
        let checked: Vec<&str> = rows
            .iter()
            .filter(|r| r.checked)
            .map(|r| r.value.as_str())
            .collect();
        assert_eq!(checked, ["de"]);
    }

    #[test]
    fn unknown_saved_tag_shows_default_checked() {
        // L11: a saved tag that is not offered is treated as not set.
        let rows = rows("xx", OFFERED);
        assert!(rows[0].checked);
        assert!(rows[1..].iter().all(|r| !r.checked));
    }

    #[test]
    fn settings_subtitle_is_native_name_or_default() {
        // T-51a-021
        assert_eq!(settings_subtitle("", OFFERED), "Default");
        assert_eq!(settings_subtitle("zh-TW", OFFERED), "繁體中文");
        assert_eq!(settings_subtitle("xx", OFFERED), "Default");
    }

    #[test]
    fn restart_note_is_the_catalog_text() {
        assert_eq!(restart_note(), "Requires app restart to take effect");
    }
}
