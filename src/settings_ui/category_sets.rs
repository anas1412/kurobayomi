//! Category sets of the automatic update and the auto-download rules (tasks T-47a-025 and T-47a-049), and the excluded
//! category lists of the delete rules (T-47a-048). An include set and an exclude set are stored as id strings, as the
//! Android keys hold them. The tri-state dialog cycles None, Include, Exclude, None on each tap; OK stores both sets
//! and Cancel stores nothing.
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::checkbox::TriState;
use kuro_prefs::pref::Pref;

use crate::pref_model::bind;
use crate::pref_model::model::{ItemCommon, PreferenceItem};
use crate::pref_model::widgets::dialog;
use crate::settings_ui::category::{self, Category};
use crate::settings_ui::rows::text;

/// The two stored sets of one category rule: the categories the rule takes and the ones it skips.
#[derive(Clone)]
pub struct CategorySets {
    pub include: Pref<BTreeSet<String>>,
    pub exclude: Pref<BTreeSet<String>>,
}

/// The tri-state a category starts the dialog in. Include wins when an id is in both sets (a hand-edited file).
pub fn state_of(id: i32, include: &BTreeSet<String>, exclude: &BTreeSet<String>) -> TriState<()> {
    let key = id.to_string();
    if include.contains(&key) {
        TriState::Include(())
    } else if exclude.contains(&key) {
        TriState::Exclude(())
    } else {
        TriState::None(())
    }
}

/// The id sets the dialog ends with: the included ids and the excluded ids, as strings.
pub fn split(states: &BTreeMap<i32, TriState<()>>) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut include = BTreeSet::new();
    let mut exclude = BTreeSet::new();
    for (id, state) in states {
        match state {
            TriState::Include(()) => {
                include.insert(id.to_string());
            }
            TriState::Exclude(()) => {
                exclude.insert(id.to_string());
            }
            TriState::None(()) => {}
        }
    }
    (include, exclude)
}

/// The action row that shows the include and exclude label of a rule and opens the tri-state dialog. `enabled` is the
/// rule's enable input (the switch above it), so a row whose rule is off is hidden with it.
pub fn categories_row(title: &str, sets: &CategorySets, categories: &[Category], message: &'static str, enabled: bool) -> PreferenceItem {
    let include = sets.include.get();
    let exclude = sets.exclude.get();
    let subtitle = category::label(&include, &exclude, categories);
    let mut row = text(title, Some(&subtitle), enabled);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        let (title, sets, categories) = (title.to_owned(), sets.clone(), categories.to_vec());
        *on_click = Some(Rc::new(move || open_dialog(&title, message, &sets, &categories)));
    }
    row
}

/// Shows the tri-state dialog for `categories`. Each row starts from the stored sets; OK writes both, a failed write
/// keeps the dialog open with its message.
pub fn open_dialog(title: &str, message: &str, sets: &CategorySets, categories: &[Category]) {
    let include = sets.include.get();
    let exclude = sets.exclude.get();
    let states: Rc<RefCell<BTreeMap<i32, TriState<()>>>> = Rc::new(RefCell::new(
        categories.iter().map(|c| (c.id, state_of(c.id, &include, &exclude))).collect(),
    ));
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    column.append(&gtk::Label::builder().label(message).wrap(true).xalign(0.0).max_width_chars(50).build());
    let mut checks = Vec::new();
    for category in categories {
        let id = category.id;
        let initial = state_of(id, &include, &exclude);
        let recorded = Rc::clone(&states);
        let check = crate::tri_check::tri_check(initial, move |next| {
            recorded.borrow_mut().insert(id, next);
        });
        check.widget().set_label(Some(&category.name));
        column.append(check.widget());
        checks.push(check);
    }
    let sets = sets.clone();
    dialog::show(
        &crate::ui::window(),
        title,
        &column,
        Some("OK"),
        move |dialog| {
            let (include, exclude) = split(&states.borrow());
            let stored = sets.include.set(include).and_then(|()| sets.exclude.set(exclude));
            match stored {
                Ok(()) => dialog.finish(),
                Err(err) => dialog.set_error(Some(&err.to_string())),
            }
        },
        || {},
    );
}

/// A multi-select of the user categories (ids as keys, names as labels), for the excluded lists of the delete rules.
/// The stored set is kept as strings; an id that no longer names a category is not offered and stays stored.
pub fn category_multi_row(title: &str, pref: &Pref<BTreeSet<String>>, categories: &[Category], enabled: bool) -> PreferenceItem {
    let mut sorted = categories.to_vec();
    sorted.sort_by_key(|c| c.order);
    let entries = sorted.iter().map(|c| (c.id.to_string(), c.name.clone())).collect();
    let mut common = ItemCommon::bound(title);
    common.enabled = enabled;
    let writer = pref.clone();
    let veto = bind::veto(move |selected: BTreeSet<String>| {
        let result = writer.set(selected).map(|()| true).map_err(|err| err.to_string());
        async move { result }
    });
    PreferenceItem::MultiSelect { common, pref: pref.clone(), entries, veto }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(ids: &[i32]) -> BTreeSet<String> {
        ids.iter().map(i32::to_string).collect()
    }

    #[test]
    fn a_category_starts_included_excluded_or_neither_and_include_wins_a_clash() {
        assert_eq!(state_of(1, &set(&[1]), &set(&[])), TriState::Include(()));
        assert_eq!(state_of(2, &set(&[]), &set(&[2])), TriState::Exclude(()));
        assert_eq!(state_of(3, &set(&[]), &set(&[])), TriState::None(()));
        assert_eq!(state_of(4, &set(&[4]), &set(&[4])), TriState::Include(()));
    }

    #[test]
    fn the_tri_state_cycle_is_none_include_exclude_none() {
        let mut state = TriState::None(());
        let mut seen = Vec::new();
        for _ in 0..4 {
            state = state.next();
            seen.push(state.clone());
        }
        assert_eq!(seen, [TriState::Include(()), TriState::Exclude(()), TriState::None(()), TriState::Include(())]);
    }

    #[test]
    fn the_dialog_result_splits_into_the_two_stored_sets() {
        let states = BTreeMap::from([(1, TriState::Include(())), (2, TriState::Exclude(())), (3, TriState::None(()))]);
        let (include, exclude) = split(&states);
        assert_eq!(include, set(&[1]));
        assert_eq!(exclude, set(&[2]));
    }

    #[test]
    fn a_stored_pair_round_trips_through_the_dialog_states() {
        let (include, exclude) = (set(&[1, 3]), set(&[2]));
        let states: BTreeMap<i32, TriState<()>> = [1, 2, 3, 4].into_iter().map(|id| (id, state_of(id, &include, &exclude))).collect();
        assert_eq!(split(&states), (include, exclude));
    }

    #[test]
    fn the_excluded_list_offers_user_categories_in_their_order() {
        let prefs = kuro_prefs::prefs::Prefs::in_memory();
        let pref = prefs.store().pref("remove_exclude_categories", BTreeSet::<String>::new());
        let categories = vec![
            Category { id: 9, name: "Later".into(), order: 2 },
            Category { id: 4, name: "Reading".into(), order: 1 },
        ];
        let PreferenceItem::MultiSelect { entries, common, .. } = category_multi_row("Excluded manga categories", &pref, &categories, true) else {
            panic!("a multi-select row")
        };
        assert_eq!(entries, vec![("4".to_owned(), "Reading".to_owned()), ("9".to_owned(), "Later".to_owned())]);
        assert!(common.enabled);
    }
}
