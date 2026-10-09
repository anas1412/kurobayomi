//! Row builders shared by the settings pages. Each one binds a row to a typed preference; a value the list does not
//! offer shows the preference default (the stored value is kept).
use std::fmt::Debug;
use std::ops::RangeInclusive;

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
