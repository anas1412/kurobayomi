//! The Global update group of the Library screen (tasks T-47a-023 to T-47a-026): the update interval, the include and
//! exclude category rows, the metadata refresh and the Updates badge switch. The device restrictions and Smart update
//! are cut (settings diet). The interval rows are the ones `misc::interval` builds; the schedule reads the same
//! preference.
use std::collections::BTreeSet;

use kuro_prefs::prefs::Prefs;

use crate::misc::interval;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::category::Category;
use crate::settings_ui::category_sets::{CategorySets, categories_row};
use crate::settings_ui::rows::{group, switch, switch_sub};

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
    items.push(categories_row("Anime Categories", &anime_sets(prefs), anime, ANIME_CATEGORIES_MESSAGE, true));
    items.push(categories_row("Manga Categories", &manga_sets(prefs), manga, MANGA_CATEGORIES_MESSAGE, true));
    items.push(switch_sub("Automatically refresh metadata", "Check for new cover and details when updating library", &l.auto_update_metadata, true));
    items.push(switch("Show unseen/unread count on Updates icon", &l.new_show_updates_count, true));
    group("Global update", items)
}

/// What the Global update rows read when they are built. The page is rebuilt when this changes: the category sets are
/// shown as their labels.
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
    fn the_global_group_lists_the_update_rows_in_the_spec_order() {
        let prefs = Prefs::in_memory();
        let Preference::Group(g) = global_update_group(&prefs, &[], &[]) else { panic!("a group") };
        assert_eq!(g.title, "Global update");
        assert_eq!(
            titles(&g.items),
            [
                "Automatic updates",
                interval::FIRST_RUN_NOTE,
                "Anime Categories",
                "Manga Categories",
                "Automatically refresh metadata",
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
