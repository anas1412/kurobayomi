//! Row builders shared by the settings pages. Each one binds a row to a typed preference; a value the list does not
//! offer shows the preference default (the stored value is kept).
// Not reachable from `route.rs` yet: text rows are used by pages that are not routed yet.
#![allow(dead_code)]
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::ops::RangeInclusive;
use std::rc::Rc;

use kuro_prefs::pref::Pref;
use kuro_prefs::values::PrefType;

use crate::pref_model::bind::{self, Veto};
use crate::pref_model::model::{slider_steps, ItemCommon, Preference, PreferenceGroup, PreferenceItem};

pub fn group(title: &str, items: Vec<PreferenceItem>) -> Preference {
    Preference::Group(PreferenceGroup::new(title, items))
}

pub fn item(item: PreferenceItem) -> Preference {
    Preference::Item(item)
}

/// Drops the rows that are not enabled (Android hides them instead of greying them out).
pub fn visible(items: Vec<PreferenceItem>) -> Vec<PreferenceItem> {
    items.into_iter().filter(PreferenceItem::enabled).collect()
}

fn common(title: &str, subtitle: Option<&str>, enabled: bool) -> ItemCommon {
    let mut common = ItemCommon::new(title);
    common.subtitle = subtitle.map(str::to_owned);
    common.enabled = enabled;
    common
}

pub fn text(title: &str, subtitle: Option<&str>, enabled: bool) -> PreferenceItem {
    PreferenceItem::Text { common: common(title, subtitle, enabled), on_click: None }
}

pub fn info(text: &str) -> PreferenceItem {
    PreferenceItem::Info { text: text.to_owned(), enabled: true }
}

/// A single-line text field. The dialog OK needs a changed, non-blank (unless `can_be_blank`) and valid text; a text
/// that fails `validate` is refused with its message, and a blank one that is not allowed is not applied.
pub fn edit_text(
    title: &str,
    pref: &Pref<String>,
    can_be_blank: bool,
    validate: fn(&str) -> Result<(), String>,
    enabled: bool,
) -> PreferenceItem {
    PreferenceItem::EditText {
        common: common(title, Some("%s"), enabled),
        pref: pref.clone(),
        dialog_subtitle: None,
        can_be_blank,
        validate: Rc::new(move |text: &str| validate(text).is_ok()),
        error_message: Some(Rc::new(move |text: &str| validate(text).err().unwrap_or_default())),
        veto: text_veto(pref, can_be_blank, validate),
    }
}

/// A multi-line text field (no validation; blank allowed only when `can_be_blank`).
pub fn multi_line_edit(title: &str, pref: &Pref<String>, can_be_blank: bool, enabled: bool) -> PreferenceItem {
    PreferenceItem::MultiLineEdit {
        common: common(title, Some("%s"), enabled),
        pref: pref.clone(),
        can_be_blank,
        veto: text_veto(pref, can_be_blank, accept_any),
    }
}

fn accept_any(_: &str) -> Result<(), String> {
    Ok(())
}

/// The write of a text row: a blank text that is not allowed is not applied (`Ok(false)`), an invalid one is an error.
fn text_veto(pref: &Pref<String>, can_be_blank: bool, validate: fn(&str) -> Result<(), String>) -> Veto<String> {
    let pref = pref.clone();
    bind::veto(move |text: String| {
        let result = if !can_be_blank && text.trim().is_empty() {
            Ok(false)
        } else {
            validate(&text).and_then(|()| pref.set(text).map(|()| true).map_err(|err| err.to_string()))
        };
        async move { result }
    })
}

pub fn switch(title: &str, pref: &Pref<bool>, enabled: bool) -> PreferenceItem {
    switch_with(title, None, pref, enabled, bind::accept())
}

pub fn switch_sub(title: &str, subtitle: &str, pref: &Pref<bool>, enabled: bool) -> PreferenceItem {
    switch_with(title, Some(subtitle), pref, enabled, bind::accept())
}

pub fn switch_with(title: &str, subtitle: Option<&str>, pref: &Pref<bool>, enabled: bool, veto: Veto<bool>) -> PreferenceItem {
    PreferenceItem::Switch { common: common(title, subtitle, enabled), pref: pref.clone(), veto }
}

/// A switch that, when turned on, turns `partner` off first (Aniyomi split and rotate are exclusive).
pub fn exclusive_switch(title: &str, pref: &Pref<bool>, partner: &Pref<bool>, enabled: bool) -> PreferenceItem {
    let partner = partner.clone();
    let veto = bind::veto(move |on: bool| {
        let result = if on { partner.set(false).map_err(|err| err.to_string()) } else { Ok(()) };
        async move { result.map(|()| true) }
    });
    switch_with(title, None, pref, enabled, veto)
}

/// A list of `i32` codes (Aniyomi stores these as ints).
pub fn int_choice(title: &str, pref: &Pref<i32>, options: &[(i32, &str)], enabled: bool) -> PreferenceItem {
    let stored = pref.get();
    let current = if options.iter().any(|(code, _)| *code == stored) { stored } else { pref.default_value() };
    let entries = options.iter().map(|(code, label)| (code.to_string(), (*label).to_owned())).collect();
    let pref = pref.clone();
    let on_change = bind::veto(move |key: String| {
        let result = key
            .parse::<i32>()
            .map_err(|err| err.to_string())
            .and_then(|code| pref.set(code).map_err(|err| err.to_string()));
        async move { result.map(|()| true) }
    });
    PreferenceItem::BasicList { common: common(title, Some("%s"), enabled), value: current.to_string(), entries, on_change }
}

/// The subtitle of a row whose change applies at the next launch (task T-47a-010).
pub const REQUIRES_RESTART: &str = "Requires app restart to take effect";

/// A list of text values stored as the text itself (`app_date_format`). A stored text the list does not offer shows
/// the preference default, as the other lists do.
pub fn text_choice(title: &str, pref: &Pref<String>, options: &[(String, String)], enabled: bool) -> PreferenceItem {
    let stored = pref.get();
    let current = if options.iter().any(|(value, _)| *value == stored) { stored } else { pref.default_value() };
    let pref = pref.clone();
    let on_change = bind::veto(move |value: String| {
        let result = pref.set(value).map(|()| true).map_err(|err| err.to_string());
        async move { result }
    });
    PreferenceItem::BasicList { common: common(title, Some("%s"), enabled), value: current, entries: options.to_vec(), on_change }
}

/// A list of a Rust enum stored by its constant name (`pref_enum!` types).
pub fn enum_choice<T>(title: &str, pref: &Pref<T>, options: &[(T, &str)], enabled: bool) -> PreferenceItem
where
    T: PrefType + Copy + Debug + PartialEq + 'static,
{
    let stored = pref.get();
    let current = if options.iter().any(|(value, _)| *value == stored) { stored } else { pref.default_value() };
    let entries = options.iter().map(|(value, label)| (format!("{value:?}"), (*label).to_owned())).collect();
    let values: Vec<T> = options.iter().map(|(value, _)| *value).collect();
    let pref = pref.clone();
    let on_change = bind::veto(move |key: String| {
        let result = values
            .iter()
            .find(|value| format!("{value:?}") == key)
            .copied()
            .ok_or_else(|| format!("unknown option {key}"))
            .and_then(|value| pref.set(value).map_err(|err| err.to_string()));
        async move { result.map(|()| true) }
    });
    PreferenceItem::BasicList {
        common: common(title, Some("%s"), enabled),
        value: format!("{current:?}"),
        entries,
        on_change,
    }
}

/// A multi-choice list whose selected keys are stored as a set of strings. `options` are `(key, label)` pairs; the
/// subtitle lists the selected labels.
pub fn multi_choice(title: &str, pref: &Pref<BTreeSet<String>>, options: &[(&str, &str)], enabled: bool) -> PreferenceItem {
    let entries = options.iter().map(|(key, label)| ((*key).to_owned(), (*label).to_owned())).collect();
    let writer = pref.clone();
    let veto = bind::veto(move |selected: BTreeSet<String>| {
        let result = writer.set(selected).map(|()| true).map_err(|err| err.to_string());
        async move { result }
    });
    PreferenceItem::MultiSelect { common: common(title, Some("%s"), enabled), pref: pref.clone(), entries, veto }
}

/// An integer slider. `subtitle` is shown as the value text.
pub fn slider(title: &str, pref: &Pref<i32>, range: RangeInclusive<i32>, subtitle: Option<&str>, enabled: bool) -> PreferenceItem {
    let steps = slider_steps(*range.start(), *range.end());
    let stored = pref.get();
    let pref = pref.clone();
    let on_change = bind::veto(move |value: i32| {
        let result = pref.set(value).map_err(|err| err.to_string());
        async move { result.map(|()| true) }
    });
    PreferenceItem::Slider { common: common(title, subtitle, enabled), value: stored, range, steps, on_change }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::player::Debanding;
    use kuro_prefs::prefs::Prefs;

    #[test]
    fn visible_drops_disabled_rows_and_keeps_the_rest() {
        let prefs = Prefs::in_memory();
        let rows = vec![
            switch("On", &prefs.reader.fullscreen, true),
            switch("Off", &prefs.reader.fullscreen, false),
        ];
        let kept = visible(rows);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].title(), "On");
    }

    #[test]
    fn int_choice_shows_the_stored_code_or_the_default_for_an_unlisted_one() {
        let prefs = Prefs::in_memory();
        prefs.reader.reader_theme.set(3).unwrap();
        let PreferenceItem::BasicList { value, .. } = int_choice("Background", &prefs.reader.reader_theme, &[(1, "Black"), (3, "Auto")], true) else {
            panic!("expected a list");
        };
        assert_eq!(value, "3");
        prefs.reader.reader_theme.set(9).unwrap();
        let PreferenceItem::BasicList { value, .. } = int_choice("Background", &prefs.reader.reader_theme, &[(1, "Black"), (3, "Auto")], true) else {
            panic!("expected a list");
        };
        assert_eq!(value, "1");
        // the unlisted stored value is not rewritten by reading it
        assert_eq!(prefs.reader.reader_theme.get(), 9);
    }

    #[test]
    fn list_choice_writes_the_selected_entry_through_its_veto() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::BasicList { on_change, .. } = int_choice("Background", &prefs.reader.reader_theme, &[(0, "White"), (2, "Gray")], true) else {
            panic!("expected a list");
        };
        let accepted = futures_block(on_change("2".to_owned()));
        assert_eq!(accepted, Ok(true));
        assert_eq!(prefs.reader.reader_theme.get(), 2);
    }

    #[test]
    fn enum_choice_stores_the_variant_name_and_rejects_unknown_keys() {
        let prefs = Prefs::in_memory();
        let options = [(Debanding::None, "None"), (Debanding::GPU, "GPU")];
        let PreferenceItem::BasicList { on_change, entries, .. } = enum_choice("Debanding", &prefs.decoder.video_debanding, &options, true) else {
            panic!("expected a list");
        };
        assert_eq!(entries[1], ("GPU".to_owned(), "GPU".to_owned()));
        assert!(futures_block(on_change("NOPE".to_owned())).is_err());
    }

    #[test]
    fn exclusive_switch_turns_the_partner_off_when_turned_on() {
        let prefs = Prefs::in_memory();
        prefs.reader.dual_page_rotate_to_fit.set(true).unwrap();
        let PreferenceItem::Switch { veto, .. } = exclusive_switch("Split", &prefs.reader.dual_page_split_paged, &prefs.reader.dual_page_rotate_to_fit, true) else {
            panic!("expected a switch");
        };
        assert_eq!(futures_block(veto(true)), Ok(true));
        assert!(!prefs.reader.dual_page_rotate_to_fit.get());
    }

    #[test]
    fn edit_text_applies_a_valid_text_and_refuses_blank_or_invalid_ones() {
        fn short(text: &str) -> Result<(), String> {
            if text.len() <= 3 { Ok(()) } else { Err("too long".to_owned()) }
        }
        let prefs = Prefs::in_memory();
        let store = Pref::new(prefs.store().clone(), "test_edit_text", "start".to_owned());
        let PreferenceItem::EditText { validate, error_message, veto, .. } = edit_text("Name", &store, false, short, true) else {
            panic!("expected an edit row");
        };
        assert!(validate("ab") && !validate("abcd"));
        assert_eq!(error_message.map(|msg| msg("abcd")), Some("too long".to_owned()));
        assert_eq!(futures_block(veto("   ".to_owned())), Ok(false));
        assert_eq!(store.get(), "start");
        assert_eq!(futures_block(veto("abcd".to_owned())), Err("too long".to_owned()));
        assert_eq!(futures_block(veto("ab".to_owned())), Ok(true));
        assert_eq!(store.get(), "ab");
    }

    #[test]
    fn a_blank_text_is_allowed_only_when_the_row_says_so() {
        let prefs = Prefs::in_memory();
        let store = Pref::new(prefs.store().clone(), "test_blank_text", "x".to_owned());
        let PreferenceItem::MultiLineEdit { veto, .. } = multi_line_edit("Notes", &store, true, true) else {
            panic!("expected a multi-line row");
        };
        assert_eq!(futures_block(veto(String::new())), Ok(true));
        assert_eq!(store.get(), "");
        let PreferenceItem::MultiLineEdit { veto, .. } = multi_line_edit("Notes", &store, false, true) else {
            panic!("expected a multi-line row");
        };
        assert_eq!(futures_block(veto(" ".to_owned())), Ok(false));
        assert_eq!(store.get(), "");
    }

    /// Runs a veto future to completion without a GTK main loop (the futures resolve immediately).
    fn futures_block<F: std::future::Future>(future: F) -> F::Output {
        let waker = std::task::Waker::noop();
        let mut context = std::task::Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut context) {
            std::task::Poll::Ready(value) => value,
            std::task::Poll::Pending => panic!("veto future is not immediately ready"),
        }
    }
}
