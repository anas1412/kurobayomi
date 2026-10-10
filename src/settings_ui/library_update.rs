//! The Global update group of the Library screen (tasks T-47a-023 to T-47a-026): the update interval, the device
//! restrictions (hidden while the interval is off), the include and exclude category rows, the metadata refresh, smart
//! update and the Updates badge switch. The interval rows are the ones `misc::interval` builds; the schedule reads the
//! same preference.
use std::collections::BTreeSet;

use kuro_prefs::prefs::Prefs;

use crate::misc::interval;
use crate::pref_model::model::{ItemCommon, Preference, PreferenceItem};
use crate::settings_ui::category::Category;
use crate::settings_ui::category_sets::{CategorySets, categories_row};
use crate::settings_ui::rows::{group, multi_choice, multi_choice_with, switch, switch_sub};

/// Device restrictions of the automatic update (`library_update_restriction`): Android constant names, with labels.
pub const DEVICE_RESTRICTIONS: [(&str, &str); 3] = [
    ("wifi", "Only on Wi-Fi"),
    ("network_not_metered", "Only on unmetered network"),
    ("ac", "When charging"),
];

/// Smart update (`library_update_manga_restriction`): every entry skips a kind of manga, all four are on by default.
pub const SMART_UPDATE: [(&str, &str); 4] = [
    ("manga_fully_read", "Skip entries with unseen episode(s)/unread chapter(s)"),
    ("manga_started", "Skip unstarted entries"),
    ("manga_ongoing", "Skip entries with \"Completed\" status"),
    ("manga_outside_release_period", "Predict next release time"),
];

/// Shown under the anime category dialog.
pub const ANIME_CATEGORIES_MESSAGE: &str = "Anime in excluded categories will not be updated even if they are also in included categories.";
/// Shown under the manga category dialog.
pub const MANGA_CATEGORIES_MESSAGE: &str = "Manga in excluded categories will not be updated even if they are also in included categories.";

/// The automatic update interval choice and its first-run note, as `misc::interval` builds them.
pub fn interval_items(pref: &kuro_prefs::pref::Pref<i32>) -> Vec<PreferenceItem> {
    interval::rows(pref)
        .into_iter()
        .flat_map(|entry| match entry {
            Preference::Group(group) => group.items,
            Preference::Item(item) => vec![item],
        })
        .collect()
}

/// Whether the automatic update runs at all: the stored interval is one of the choices and not Off.
pub fn interval_on(prefs: &Prefs) -> bool {
    interval::hours(prefs.library.auto_update_interval.get()) > 0
}

/// The device restrictions. Hidden while the interval is Off; the subtitle reads `Restrictions: <labels>`.
pub fn restriction_row(prefs: &Prefs) -> PreferenceItem {
    let mut common = ItemCommon::bound("Automatic updates device restrictions").with_subtitle("Restrictions: %s");
    common.enabled = interval_on(prefs);
    multi_choice_with(common, &prefs.library.auto_update_device_restrictions, &DEVICE_RESTRICTIONS)
}

/// The anime category sets of the automatic update.
pub fn anime_sets(prefs: &Prefs) -> CategorySets {
    CategorySets { include: prefs.library.anime_update_categories.clone(), exclude: prefs.library.anime_update_categories_exclude.clone() }
}

/// The manga category sets of the automatic update.
pub fn manga_sets(prefs: &Prefs) -> CategorySets {
    CategorySets { include: prefs.library.manga_update_categories.clone(), exclude: prefs.library.manga_update_categories_exclude.clone() }
}

/// The Global update group. `anime` and `manga` are the user categories of each kind.
pub fn global_update_group(prefs: &Prefs, anime: &[Category], manga: &[Category]) -> Preference {
    let l = &prefs.library;
    let mut items = interval_items(&l.auto_update_interval);
    items.push(restriction_row(prefs));
    items.push(categories_row("Anime Categories", &anime_sets(prefs), anime, ANIME_CATEGORIES_MESSAGE, true));
    items.push(categories_row("Manga Categories", &manga_sets(prefs), manga, MANGA_CATEGORIES_MESSAGE, true));
    items.push(switch_sub("Automatically refresh metadata", "Check for new cover and details when updating library", &l.auto_update_metadata, true));
    items.push(multi_choice("Smart update", &l.auto_update_item_restrictions, &SMART_UPDATE, true));
    items.push(switch("Show unseen/unread count on Updates icon", &l.new_show_updates_count, true));
    group("Global update", items)
}

/// What the Global update rows read when they are built. The page is rebuilt when this changes: the interval turns the
/// restrictions on or off, and the category sets are shown as their labels.
pub fn rule_state(prefs: &Prefs) -> (i32, BTreeSet<String>, BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let l = &prefs.library;
    (
        l.auto_update_interval.get(),
        l.anime_update_categories.get(),
        l.anime_update_categories_exclude.get(),
        l.manga_update_categories.get(),
        l.manga_update_categories_exclude.get(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::prefs::Prefs;

    fn titles(items: &[PreferenceItem]) -> Vec<&str> {
        items.iter().map(PreferenceItem::title).collect()
    }

    #[test]
    fn the_restriction_row_is_hidden_while_the_interval_is_off() {
        let prefs = Prefs::in_memory();
        let PreferenceItem::MultiSelect { common, .. } = restriction_row(&prefs) else { panic!("multi-select") };
        assert!(!common.enabled, "the interval defaults to Off");
        prefs.library.auto_update_interval.set(24).unwrap();
        let PreferenceItem::MultiSelect { common, .. } = restriction_row(&prefs) else { panic!("multi-select") };
        assert!(common.enabled);
        assert_eq!(common.subtitle.as_deref(), Some("Restrictions: %s"));
    }

    #[test]
    fn an_interval_value_outside_the_choices_counts_as_off() {
        let prefs = Prefs::in_memory();
        prefs.library.auto_update_interval.set(5).unwrap();
        assert!(!interval_on(&prefs));
        prefs.library.auto_update_interval.set(168).unwrap();
        assert!(interval_on(&prefs));
    }

    #[test]
    fn the_restriction_labels_are_the_three_android_entries_in_order() {
        let labels: Vec<&str> = DEVICE_RESTRICTIONS.iter().map(|(_, label)| *label).collect();
        assert_eq!(labels, ["Only on Wi-Fi", "Only on unmetered network", "When charging"]);
        let keys: Vec<&str> = DEVICE_RESTRICTIONS.iter().map(|(key, _)| *key).collect();
        assert_eq!(keys, ["wifi", "network_not_metered", "ac"]);
    }

    #[test]
    fn the_restriction_default_is_wifi_and_smart_update_starts_with_all_four() {
        let prefs = Prefs::in_memory();
        let selected = prefs.library.auto_update_device_restrictions.get();
        assert_eq!(selected, BTreeSet::from(["wifi".to_owned()]));
        assert_eq!(prefs.library.auto_update_item_restrictions.get().len(), SMART_UPDATE.len());
    }

    #[test]
    fn the_global_group_lists_the_update_rows_in_the_spec_order() {
        let prefs = Prefs::in_memory();
        let Preference::Group(g) = global_update_group(&prefs, &[], &[]) else { panic!("a group") };
        assert_eq!(g.title, "Global update");
        assert_eq!(
            titles(&g.items),
            [
                "Automatic updates",
                interval::FIRST_RUN_NOTE,
                "Automatic updates device restrictions",
                "Anime Categories",
                "Manga Categories",
                "Automatically refresh metadata",
                "Smart update",
                "Show unseen/unread count on Updates icon",
            ]
        );
    }

    #[test]
    fn the_category_dialogs_carry_the_spec_messages() {
        assert!(ANIME_CATEGORIES_MESSAGE.starts_with("Anime in excluded categories"));
        assert!(MANGA_CATEGORIES_MESSAGE.starts_with("Manga in excluded categories"));
    }

    #[test]
    fn the_rule_state_changes_when_the_interval_or_a_category_set_changes() {
        let prefs = Prefs::in_memory();
        let before = rule_state(&prefs);
        prefs.library.manga_update_categories.set(BTreeSet::from(["3".to_owned()])).unwrap();
        assert_ne!(before, rule_state(&prefs));
    }
}
