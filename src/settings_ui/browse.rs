//! Browse rows this module binds to the typed preferences: the two "hide entries already in library" switches of the
//! Sources group (task T-47a-060). Each switch is independent; the Browse lists and global search read them.
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::settings_ui::rows::{group, switch};

/// The Sources group rows the typed preferences drive. The extension repo links and the NSFW group stay as the spec
/// builds them.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    vec![group(
        "Sources",
        vec![
            switch("Hide anime entries already in library", &prefs.sources.hide_in_anime_library_items, true),
            switch("Hide manga entries already in library", &prefs.sources.hide_in_manga_library_items, true),
        ],
    )]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_model::model::PreferenceItem;

    #[test]
    fn the_two_hide_switches_read_their_own_keys_and_default_to_off() {
        let prefs = Prefs::in_memory();
        let tree = page_rows(&prefs);
        let Preference::Group(sources) = &tree[0] else { panic!("group") };
        assert_eq!(sources.title, "Sources");
        let keys: Vec<(&str, bool)> = sources
            .items
            .iter()
            .map(|item| match item {
                PreferenceItem::Switch { common, pref, .. } => (common.title.as_str(), pref.get()),
                _ => panic!("switch"),
            })
            .collect();
        assert_eq!(keys, [("Hide anime entries already in library", false), ("Hide manga entries already in library", false)]);
    }

    #[test]
    fn turning_one_switch_leaves_the_other_alone() {
        let prefs = Prefs::in_memory();
        prefs.sources.hide_in_anime_library_items.set(true).unwrap();
        assert!(!prefs.sources.hide_in_manga_library_items.get());
        let tree = page_rows(&prefs);
        let Preference::Group(sources) = &tree[0] else { panic!("group") };
        let PreferenceItem::Switch { pref: anime, .. } = &sources.items[0] else { panic!("switch") };
        let PreferenceItem::Switch { pref: manga, .. } = &sources.items[1] else { panic!("switch") };
        assert!(anime.get());
        assert!(!manga.get());
    }
}
