//! Puts the typed rows of this module into a page built from the spec. The spec rows keep their place and their
//! text; a row that has a typed twin (same group title, same row title) is replaced by it, so it reads and writes the
//! real preference. A twin that is disabled drops the spec row (the reader hides rows its rules turn off).
use crate::pref_model::model::{Preference, PreferenceGroup, PreferenceItem};

fn take(pool: &mut Vec<PreferenceItem>, title: &str) -> Option<PreferenceItem> {
    pool.iter().position(|item| item.title() == title).map(|at| pool.remove(at))
}

/// `spec` is the spec-built page; `typed` the typed rows, disabled ones included. Rows and groups without a twin stay.
pub fn merge(spec: Vec<Preference>, typed: Vec<Preference>) -> Vec<Preference> {
    let mut loose: Vec<PreferenceItem> = Vec::new();
    let mut groups: Vec<PreferenceGroup> = Vec::new();
    for entry in typed {
        match entry {
            Preference::Item(item) => loose.push(item),
            Preference::Group(group) => groups.push(group),
        }
    }
    let swap = |pool: &mut Vec<PreferenceItem>, item: PreferenceItem| match take(pool, item.title()) {
        None => Some(item),
        Some(twin) if twin.enabled() => Some(twin),
        Some(_) => None,
    };
    spec.into_iter()
        .filter_map(|entry| match entry {
            Preference::Item(item) => swap(&mut loose, item).map(Preference::Item),
            Preference::Group(mut group) => {
                if let Some(at) = groups.iter().position(|g| g.title == group.title) {
                    let mut pool = std::mem::take(&mut groups[at].items);
                    group.items = group.items.into_iter().filter_map(|item| swap(&mut pool, item)).collect();
                }
                Some(Preference::Group(group))
            }
        })
        .collect()
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
}
