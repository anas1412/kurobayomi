//! The scanlator exclusion sheet (T-38b-015): the rules (GTK-free) and the window. Upstream:
//! `ScanlatorFilterDialog.kt`. It lists the available scanlators sorted without regard to case; a click toggles one
//! between included and excluded. One button selects every scanlator while none is excluded and resets the exclusions
//! otherwise; OK hands the excluded set back, Cancel and any other way out write nothing. With no scanlators the sheet
//! says so and offers only Cancel. A stale excluded name (one no chapter has any more) is kept through OK.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use crate::ui;

/// What the bulk button does now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bulk {
    SelectAll,
    Reset,
}

/// The state of the open sheet.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanlatorSheet {
    sorted: Vec<String>,
    excluded: BTreeSet<String>,
}

impl ScanlatorSheet {
    pub fn new(available: &[String], excluded: &BTreeSet<String>) -> Self {
        let mut sorted: Vec<String> = available.to_vec();
        sorted.sort_by_key(|name| name.to_lowercase());
        sorted.dedup();
        Self { sorted, excluded: excluded.clone() }
    }

    /// The scanlators, sorted without regard to case.
    pub fn rows(&self) -> &[String] {
        &self.sorted
    }

    pub fn is_empty(&self) -> bool {
        self.sorted.is_empty()
    }

    pub fn is_excluded(&self, name: &str) -> bool {
        self.excluded.contains(name)
    }

    /// A click on a row: excluded becomes included and the other way round.
    pub fn toggle(&mut self, name: &str) {
        if !self.excluded.remove(name) {
            self.excluded.insert(name.to_owned());
        }
    }

    /// Select all while nothing is excluded, Reset once something is.
    pub fn bulk(&self) -> Bulk {
        if self.excluded.is_empty() { Bulk::SelectAll } else { Bulk::Reset }
    }

    pub fn apply_bulk(&mut self) {
        match self.bulk() {
            Bulk::SelectAll => self.excluded.extend(self.sorted.iter().cloned()),
            Bulk::Reset => self.excluded.clear(),
        }
    }

    /// The set OK writes.
    pub fn confirm(&self) -> BTreeSet<String> {
        self.excluded.clone()
    }
}

/// Opens the sheet. `on_ok` runs with the excluded set when OK is pressed.
pub fn open(available: &[String], excluded: &BTreeSet<String>, on_ok: impl Fn(BTreeSet<String>) + 'static) {
    let sheet = Rc::new(RefCell::new(ScanlatorSheet::new(available, excluded)));
    let col = ui::column();
    let title = gtk::Label::new(Some(&tr(MsgKey::exclude_scanlators)));
    let window = ui::modal(title.upcast_ref(), &col, 360, 480);
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let cancel = gtk::Button::with_label(&tr(MsgKey::action_cancel));
    cancel.connect_clicked({
        let window = window.clone();
        move |_| window.close()
    });
    if sheet.borrow().is_empty() {
        col.append(&gtk::Label::builder().label(tr(MsgKey::no_scanlators_found)).xalign(0.0).wrap(true).build());
        buttons.set_halign(gtk::Align::End);
        buttons.append(&cancel);
        col.append(&buttons);
        window.present();
        return;
    }
    let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let checks: Vec<(String, gtk::CheckButton)> = sheet
        .borrow()
        .rows()
        .iter()
        .map(|name| {
            // a checked box is an excluded scanlator, as the cross of the upstream row
            let check = gtk::CheckButton::builder().label(name).active(sheet.borrow().is_excluded(name)).build();
            list.append(&check);
            (name.clone(), check)
        })
        .collect();
    let bulk = gtk::Button::builder().css_classes(["flat"]).build();
    let label_bulk = {
        let (sheet, bulk) = (Rc::clone(&sheet), bulk.clone());
        move || bulk.set_label(&tr(if sheet.borrow().bulk() == Bulk::SelectAll { MsgKey::action_select_all } else { MsgKey::action_reset }))
    };
    label_bulk();
    let syncing = Rc::new(std::cell::Cell::new(false));
    for (name, check) in &checks {
        let (sheet, name, label_bulk, syncing) = (Rc::clone(&sheet), name.clone(), label_bulk.clone(), Rc::clone(&syncing));
        check.connect_toggled(move |_| {
            if !syncing.get() {
                sheet.borrow_mut().toggle(&name);
                label_bulk();
            }
        });
    }
    bulk.connect_clicked({
        let (sheet, label_bulk) = (Rc::clone(&sheet), label_bulk.clone());
        move |_| {
            sheet.borrow_mut().apply_bulk();
            syncing.set(true);
            for (name, check) in &checks {
                check.set_active(sheet.borrow().is_excluded(name));
            }
            syncing.set(false);
            label_bulk();
        }
    });
    let ok = gtk::Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
    ok.connect_clicked({
        let (sheet, window) = (Rc::clone(&sheet), window.clone());
        move |_| {
            on_ok(sheet.borrow().confirm());
            window.close();
        }
    });
    let spacer = gtk::Box::builder().hexpand(true).build();
    for w in [bulk.upcast_ref::<gtk::Widget>(), spacer.upcast_ref(), cancel.upcast_ref(), ok.upcast_ref()] {
        buttons.append(w);
    }
    col.append(&ui::scrolled(&list));
    col.append(&buttons);
    window.present();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn set(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn rows_sort_without_regard_to_case() {
        let sheet = ScanlatorSheet::new(&names(&["beta", "Alpha", "gamma", "Beta2"]), &set(&[]));
        assert_eq!(sheet.rows(), ["Alpha", "beta", "Beta2", "gamma"]);
    }

    #[test]
    fn a_click_toggles_between_included_and_excluded() {
        let mut sheet = ScanlatorSheet::new(&names(&["A", "B"]), &set(&["B"]));
        assert!(sheet.is_excluded("B") && !sheet.is_excluded("A"));
        sheet.toggle("A");
        sheet.toggle("B");
        assert_eq!(sheet.confirm(), set(&["A"]));
    }

    #[test]
    fn the_bulk_button_selects_all_while_nothing_is_excluded_and_resets_after() {
        let mut sheet = ScanlatorSheet::new(&names(&["A", "B"]), &set(&[]));
        assert_eq!(sheet.bulk(), Bulk::SelectAll);
        sheet.apply_bulk();
        assert_eq!(sheet.confirm(), set(&["A", "B"]));
        assert_eq!(sheet.bulk(), Bulk::Reset);
        sheet.apply_bulk();
        assert!(sheet.confirm().is_empty());
    }

    #[test]
    fn a_stale_excluded_name_survives_ok_and_an_empty_sheet_says_so() {
        let mut sheet = ScanlatorSheet::new(&names(&["A"]), &set(&["Gone"]));
        sheet.toggle("A");
        assert_eq!(sheet.confirm(), set(&["A", "Gone"]), "the name no chapter has is kept");
        assert!(ScanlatorSheet::new(&[], &set(&[])).is_empty());
    }
}
