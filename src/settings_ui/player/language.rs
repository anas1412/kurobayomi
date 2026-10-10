//! Text rows of the Subtitles and Audio pages (tasks T-48a-029 and T-48a-031). A language list is checked by the
//! language validator before it is stored; a whitelist or blacklist is stored as typed. Both open an edit dialog that
//! shows an explanatory line under the title. They are plain `EditText` rows, so the page's edit dialog runs the
//! validator and keeps OK off until the text changed.
use std::rc::Rc;

use kuro_prefs::pref::Pref;

use crate::pref_model::bind::{self, Veto};
use crate::pref_model::model::{ItemCommon, PreferenceItem};
use crate::settings_ui::lang;

/// A preferred-language row: the list is checked with [`lang::validate_text`] and an invalid one shows its message.
pub fn language_row(title: &str, pref: &Pref<String>, dialog_info: &str) -> PreferenceItem {
    edit_info_row(title, pref, dialog_info, lang::validate_text)
}

/// A free text row (whitelist, blacklist): any text is stored, an empty one too.
pub fn text_row(title: &str, pref: &Pref<String>, dialog_info: &str) -> PreferenceItem {
    edit_info_row(title, pref, dialog_info, accept_any)
}

fn accept_any(_: &str) -> Result<(), String> {
    Ok(())
}

fn edit_info_row(title: &str, pref: &Pref<String>, dialog_info: &str, validate: fn(&str) -> Result<(), String>) -> PreferenceItem {
    PreferenceItem::EditText {
        common: ItemCommon::bound(title),
        pref: pref.clone(),
        dialog_subtitle: Some(dialog_info.to_owned()),
        can_be_blank: true,
        validate: Rc::new(move |text: &str| validate(text).is_ok()),
        error_message: Some(Rc::new(move |text: &str| validate(text).err().unwrap_or_default())),
        veto: text_veto(pref, validate),
    }
}

/// The write of a text row: a text that fails `validate` is refused with its message and nothing is stored.
fn text_veto(pref: &Pref<String>, validate: fn(&str) -> Result<(), String>) -> Veto<String> {
    let pref = pref.clone();
    bind::veto(move |text: String| {
        let result = validate(&text).and_then(|()| pref.set(text).map(|()| true).map_err(|err| err.to_string()));
        async move { result }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::prefs::Prefs;

    fn run<F: std::future::Future>(future: F) -> F::Output {
        let mut context = std::task::Context::from_waker(std::task::Waker::noop());
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("the veto is not immediately ready"),
        }
    }

    fn veto_of(item: &PreferenceItem) -> &Veto<String> {
        match item {
            PreferenceItem::EditText { veto, .. } => veto,
            _ => panic!("an edit row with a dialog line"),
        }
    }

    #[test]
    fn a_valid_language_list_is_stored_and_an_invalid_one_names_the_code() {
        let prefs = Prefs::in_memory();
        let pref = Pref::new(prefs.store().clone(), "pref_subtitle_lang", String::new());
        let row = language_row("Preferred languages", &pref, "info");
        assert_eq!(run(veto_of(&row)("en, ja".to_owned())), Ok(true));
        assert_eq!(pref.get(), "en, ja");
        assert_eq!(run(veto_of(&row)("xx".to_owned())), Err("Invalid code: \"xx\"".to_owned()));
        assert_eq!(pref.get(), "en, ja", "a refused list is not stored");
    }

    #[test]
    fn the_dialog_line_and_the_validator_are_wired_to_the_row() {
        let prefs = Prefs::in_memory();
        let pref = Pref::new(prefs.store().clone(), "pref_subtitle_lang", String::new());
        let PreferenceItem::EditText { dialog_subtitle, validate, error_message, .. } = language_row("Preferred languages", &pref, "Two-letter codes") else {
            panic!("an edit row with a dialog line");
        };
        assert_eq!(dialog_subtitle.as_deref(), Some("Two-letter codes"));
        assert!(validate("eng") && !validate("qq"));
        assert_eq!(error_message.map(|msg| msg("qq")), Some("Invalid code: \"qq\"".to_owned()));
    }

    #[test]
    fn a_text_row_stores_anything_including_an_empty_text() {
        let prefs = Prefs::in_memory();
        let pref = Pref::new(prefs.store().clone(), "pref_subtitle_whitelist", "old".to_owned());
        let row = text_row("Whitelist", &pref, "info");
        assert_eq!(run(veto_of(&row)(String::new())), Ok(true));
        assert_eq!(pref.get(), "");
        assert_eq!(run(veto_of(&row)("anything, at all".to_owned())), Ok(true));
        assert_eq!(pref.get(), "anything, at all");
    }
}
