//! Puts the typed rows of this module into a page built from the spec. The spec rows keep their place and their
//! text; a row that has a typed twin (same group title, same row title) is replaced by it, so it reads and writes the
//! real preference. A twin that is disabled drops the spec row (the reader hides rows its rules turn off). A typed row
//! with no twin is kept when it is enabled, placed before the next typed row that has one, so a row the spec does not
//! build (Theme mode is a `chips` item the spec converter skips) still shows in its place. Typed groups with no spec
//! group are dropped.
use std::collections::VecDeque;

use crate::pref_model::model::{Preference, PreferenceItem};

/// Merges one level of a page. `same` says whether a spec entry and a typed entry are twins; `keep` says whether an
/// unmatched typed entry is shown; `combine` builds the shown entry from a matched pair (`None` drops the spec entry).
fn merge_level<T>(
    spec: Vec<T>,
    typed: Vec<T>,
    same: impl Fn(&T, &T) -> bool,
    keep: impl Fn(&T) -> bool,
    combine: impl Fn(T, T) -> Option<T>,
) -> Vec<T> {
    let mut claimed_by: Vec<Option<usize>> = vec![None; spec.len()];
    let mut matched = vec![false; typed.len()];
    for (k, entry) in typed.iter().enumerate() {
        if let Some(j) = spec.iter().enumerate().position(|(j, s)| claimed_by[j].is_none() && same(s, entry)) {
            claimed_by[j] = Some(k);
            matched[k] = true;
        }
    }
    let mut typed: Vec<Option<T>> = typed.into_iter().map(Some).collect();
    let mut pending: VecDeque<usize> = (0..typed.len()).filter(|&k| !matched[k]).collect();
    let mut out = Vec::new();
    let emit_pending = |pending: &mut VecDeque<usize>, typed: &mut Vec<Option<T>>, before: Option<usize>, out: &mut Vec<T>| {
        while let Some(&p) = pending.front() {
            if before.is_some_and(|k| p >= k) {
                break;
            }
            pending.pop_front();
            if let Some(entry) = typed[p].take().filter(|e| keep(e)) {
                out.push(entry);
            }
        }
    };
    for (j, entry) in spec.into_iter().enumerate() {
        match claimed_by[j] {
            Some(k) => {
                emit_pending(&mut pending, &mut typed, Some(k), &mut out);
                if let Some(twin) = typed[k].take() {
                    out.extend(combine(entry, twin));
                }
            }
            None => out.push(entry),
        }
    }
    emit_pending(&mut pending, &mut typed, None, &mut out);
    out
}

fn merge_items(spec: Vec<PreferenceItem>, typed: Vec<PreferenceItem>) -> Vec<PreferenceItem> {
    merge_level(spec, typed, |s, t| s.title() == t.title(), PreferenceItem::enabled, |_, twin| twin.enabled().then_some(twin))
}

/// `spec` is the spec-built page; `typed` the typed rows, disabled ones included. Rows and groups without a twin stay
/// (see the module comment for which unmatched typed rows are kept).
pub fn merge(spec: Vec<Preference>, typed: Vec<Preference>) -> Vec<Preference> {
    merge_level(
        spec,
        typed,
        |s, t| match (s, t) {
            (Preference::Item(a), Preference::Item(b)) => a.title() == b.title(),
            (Preference::Group(a), Preference::Group(b)) => a.title == b.title,
            _ => false,
        },
        |t| matches!(t, Preference::Item(item) if item.enabled()),
        |entry, twin| match (entry, twin) {
            (Preference::Group(mut group), Preference::Group(typed)) => {
                group.items = merge_items(group.items, typed.items);
                Some(Preference::Group(group))
            }
            (Preference::Item(_), Preference::Item(item)) => item.enabled().then_some(Preference::Item(item)),
            _ => None,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings_ui::rows::{group, item, switch};
    use kuro_prefs::prefs::Prefs;

    fn titles(tree: &[Preference]) -> Vec<String> {
        tree.iter()
            .flat_map(|entry| match entry {
                Preference::Item(i) => vec![i.title().to_owned()],
                Preference::Group(g) => g.items.iter().map(|i| format!("{}/{}", g.title, i.title())).collect(),
            })
            .collect()
    }

    #[test]
    fn a_typed_twin_replaces_the_spec_row_in_place_and_the_rest_stays() {
        let prefs = Prefs::in_memory();
        let spec_store = prefs.store().clone();
        let junk = |title: &str| switch(title, &kuro_prefs::pref::Pref::new(spec_store.clone(), format!("spec/{title}"), false), true);
        let spec = vec![item(junk("Top")), group("Paged", vec![junk("A"), junk("B"), junk("C")])];
        let typed = vec![item(switch("Top", &prefs.reader.fullscreen, true)), group("Paged", vec![switch("B", &prefs.reader.crop_borders, true)])];
        let merged = merge(spec, typed);
        assert_eq!(titles(&merged), ["Top", "Paged/A", "Paged/B", "Paged/C"]);
        let Preference::Group(g) = &merged[1] else { panic!("group") };
        let PreferenceItem::Switch { pref, .. } = &g.items[1] else { panic!("switch") };
        assert_eq!(pref.key(), prefs.reader.crop_borders.key(), "B is the typed row");
        let PreferenceItem::Switch { pref, .. } = &g.items[0] else { panic!("switch") };
        assert_eq!(pref.key(), "spec/A", "A has no twin");
    }

    #[test]
    fn a_disabled_twin_hides_the_spec_row_and_the_same_title_in_another_group_is_untouched() {
        let prefs = Prefs::in_memory();
        let junk = |title: &str| switch(title, &prefs.reader.fullscreen, true);
        let spec = vec![group("Paged", vec![junk("Pan"), junk("Tap")]), group("Long strip", vec![junk("Pan")])];
        let typed = vec![group("Paged", vec![switch("Pan", &prefs.reader.navigate_to_pan, false)])];
        assert_eq!(titles(&merge(spec, typed)), ["Paged/Tap", "Long strip/Pan"]);
    }

    #[test]
    fn an_enabled_typed_row_without_a_twin_is_placed_before_the_next_typed_row_that_has_one() {
        let prefs = Prefs::in_memory();
        let spec = vec![group("Theme", vec![switch("Pure black", &prefs.reader.fullscreen, true)])];
        let typed = vec![group(
            "Theme",
            vec![switch("Theme mode", &prefs.reader.crop_borders, true), switch("Pure black", &prefs.reader.navigate_to_pan, true)],
        )];
        assert_eq!(titles(&merge(spec, typed)), ["Theme/Theme mode", "Theme/Pure black"]);
    }

    #[test]
    fn a_disabled_typed_row_without_a_twin_and_a_typed_group_without_a_spec_group_are_dropped() {
        let prefs = Prefs::in_memory();
        let spec = vec![group("Paged", vec![switch("Pan", &prefs.reader.fullscreen, true)])];
        let typed = vec![
            group("Paged", vec![switch("Hidden", &prefs.reader.crop_borders, false)]),
            group("Unknown", vec![switch("Lost", &prefs.reader.crop_borders, true)]),
        ];
        assert_eq!(titles(&merge(spec, typed)), ["Paged/Pan"]);
    }
}
