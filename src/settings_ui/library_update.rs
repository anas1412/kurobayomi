//! The Global update group of the Library screen (tasks T-47a-023 to T-47a-026): the update interval. The category
//! sets, the metadata refresh, the Updates badge switch, the device restrictions and Smart update are cut (settings
//! diet). The interval rows are the ones `misc::interval` builds; the schedule reads the same preference.
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::prefs::Prefs;

use crate::misc::interval;
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::rows::group;

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

/// The Global update group: the interval rows. The category sets, the metadata refresh and the Updates badge switch are
/// cut (settings diet): every category is updated, the metadata is left to a manual refresh and the badge is always on.
pub fn global_update_group(prefs: &Prefs) -> Preference {
    group(&tr(MsgKey::pref_category_library_update), interval_items(&prefs.library.auto_update_interval))
}

/// What the Global update rows read when they are built. The page is rebuilt when this changes: the interval decides
/// whether the first-run note shows.
pub fn rule_state(prefs: &Prefs) -> i32 {
    prefs.library.auto_update_interval.get()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::prefs::Prefs;

    fn titles(items: &[PreferenceItem]) -> Vec<&str> {
        items.iter().map(PreferenceItem::title).collect()
    }

    #[test]
    fn the_global_group_lists_the_interval_rows_only() {
        let prefs = Prefs::in_memory();
        let Preference::Group(g) = global_update_group(&prefs) else { panic!("a group") };
        assert_eq!(g.title, "Global update");
        assert_eq!(titles(&g.items), ["Automatic updates", &interval::first_run_note()]);
    }

    #[test]
    fn the_rule_state_changes_when_the_interval_changes() {
        let prefs = Prefs::in_memory();
        let before = rule_state(&prefs);
        prefs.library.auto_update_interval.set(24).unwrap();
        assert_ne!(before, rule_state(&prefs));
    }
}
