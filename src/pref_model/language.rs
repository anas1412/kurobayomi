//! The App language page (T-47b-005 to T-47b-008): "Default", then every language the catalogue ships, each in its own
//! name with the English name under it. A tap saves the choice at once and shows the restart note, because the
//! language is read when the app starts.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::picker::{self, PickerRow};
use kuro_prefs::pref::Pref;

use crate::screens::Built;

/// The preference that holds the choice: a language tag, or empty for Default.
pub const PREF_KEY: &str = "app_language";

/// The preference over `store`.
pub fn pref(store: &kuro_prefs::store::PrefStore) -> Pref<String> {
    store.pref(PREF_KEY, String::new())
}

/// The rows the page shows for the saved choice.
pub fn rows(saved: &str) -> Vec<PickerRow> {
    picker::rows(saved, &kuro_i18n::offered_locales())
}

/// Saves `value` unless it is already the choice. Returns whether it was written (and the restart note applies).
pub fn choose(pref: &Pref<String>, rows: &[PickerRow], value: &str) -> bool {
    let current = rows.iter().find(|r| r.checked).map(|r| r.value.as_str()).unwrap_or("");
    if current == value {
        return false;
    }
    match pref.set(value.to_owned()) {
        Ok(()) => true,
        Err(err) => {
            tracing::warn!(%err, "could not save the app language");
            false
        }
    }
}

/// The page. `pref` holds the saved choice.
pub fn page(pref: Pref<String>) -> Option<Built> {
    let column = crate::ui::column();
    let note = crate::ui::dim(&picker::restart_note());
    note.set_visible(false);
    column.append(&note);
    let list = crate::ui::boxed_list();
    let rows = Rc::new(RefCell::new(rows(&pref.get())));
    let checks: Vec<(String, gtk::Image)> = rows
        .borrow()
        .iter()
        .map(|r| {
            let check = gtk::Image::from_icon_name("object-select-symbolic");
            check.set_visible(r.checked);
            let row = crate::ui::row(&r.primary, Some(&r.secondary), Some(check.clone().upcast()));
            row.set_activatable(true);
            list.append(&row);
            (r.value.clone(), check)
        })
        .collect();
    list.connect_row_activated(move |_, row| {
        let Some((value, _)) = checks.get(row.index() as usize) else { return };
        if choose(&pref, &rows.borrow(), value) {
            for r in rows.borrow_mut().iter_mut() {
                r.checked = r.value == *value;
            }
            for (v, check) in &checks {
                check.set_visible(v == value);
            }
            note.set_visible(true);
        }
    });
    column.append(&list);
    Some(Built { content: crate::ui::scrolled(&column).upcast(), end: vec![] })
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    #[test]
    fn t_47b_005_default_comes_first_and_the_offered_languages_follow() {
        let rows = rows("");
        assert_eq!(rows[0].value, "");
        assert_eq!(rows[0].primary, "Default");
        assert!(rows[0].checked);
        assert!(rows.len() > 1, "the catalogue ships languages");
        assert_eq!(rows.iter().filter(|r| r.checked).count(), 1);
    }

    #[test]
    fn t_47b_007_a_choice_is_saved_once_and_picking_the_same_one_writes_nothing() {
        let store = PrefStore::in_memory();
        let language = pref(&store);
        let shown = rows(&language.get());
        let tag = shown.iter().find(|r| !r.value.is_empty()).expect("a language").value.clone();
        assert!(choose(&language, &shown, &tag));
        assert_eq!(language.get(), tag);
        let after = rows(&language.get());
        assert!(after.iter().any(|r| r.checked && r.value == tag));
        assert!(!choose(&language, &after, &tag), "the same row again changes nothing");
        assert!(choose(&language, &after, ""), "Default clears the choice");
        assert_eq!(language.get(), "");
    }

    #[test]
    fn t_47b_007_a_saved_tag_that_is_no_longer_offered_reads_as_default() {
        let shown = rows("xx-not-a-locale");
        assert!(shown[0].checked);
    }
}
