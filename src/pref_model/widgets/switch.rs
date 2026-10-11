//! Switch row (Kotlin `SwitchPreferenceWidget`). The whole row toggles; the switch is not a target.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::pref::Pref;

use super::super::bind::{self, Veto, WriteGate};
use super::super::model::ItemCommon;
use super::row::{self, PrefRow, RowSpec};

pub fn build(common: &ItemCommon, pref: Pref<bool>, veto: Veto<bool>) -> PrefRow {
    let switch = gtk::Switch::new();
    switch.set_valign(gtk::Align::Center);
    switch.set_can_target(false);
    switch.set_active(pref.get());

    let gate = Rc::new(WriteGate::default());
    let toggle = {
        let (switch, pref, gate) = (switch.clone(), pref.clone(), gate.clone());
        move || {
            let next = !switch.is_active();
            let (done_switch, done_pref) = (switch.clone(), pref.clone());
            bind::write(gate.clone(), veto.clone(), pref.clone(), next, move |outcome| {
                match outcome {
                    bind::Outcome::Written => done_switch.set_active(next),
                    bind::Outcome::Kept => {}
                    bind::Outcome::Failed(message) => {
                        tracing::warn!(key = done_pref.key(), %message, "setting not saved");
                    }
                }
            });
        }
    };
    let built = row::build(RowSpec {
        title: &common.title,
        subtitle: common.subtitle.as_deref(),
        icon: common.icon,
        trailing: Some(switch.upcast_ref()),
        on_click: Some(Rc::new(toggle)),
    });

    let (alive, weak) = (switch.downgrade(), switch.downgrade());
    bind::follow(&pref, move || alive.upgrade().is_some(), move |value| {
        if let Some(switch) = weak.upgrade() {
            if switch.is_active() != value {
                switch.set_active(value);
            }
        }
    });
    built.row.set_sensitive(common.enabled);
    built
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::prefs::Prefs;

    fn pump() {
        let until = std::time::Instant::now() + std::time::Duration::from_millis(150);
        while std::time::Instant::now() < until {
            gtk::glib::MainContext::default().iteration(false);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    fn find_switch(root: &gtk::Widget) -> Option<gtk::Switch> {
        if let Some(found) = root.downcast_ref::<gtk::Switch>() {
            return Some(found.clone());
        }
        let mut child = root.first_child();
        while let Some(c) = child {
            if let Some(found) = find_switch(&c) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }

    fn switch_in(root: &gtk::Widget) -> gtk::Switch {
        find_switch(root).expect("a switch in the row")
    }

    // T-47ba-048, T-E20: a click, Enter or Space on the row toggles; the switch is not a target; a veto of false
    // leaves both the switch and the stored value alone.
    #[test]
    fn the_row_toggles_by_click_and_key_and_a_vetoed_write_changes_nothing() {
        crate::testing::with_display(|| {
            let prefs = Prefs::in_memory();
            let pref = prefs.store().pref("switch_row_test", false);
            let accepted = build(&ItemCommon::new("Accepts"), pref.clone(), bind::accept());
            let list = gtk::ListBox::new();
            list.append(&accepted.row);
            let switch = switch_in(accepted.row.upcast_ref());
            assert!(!switch.can_target(), "the whole row is the target, not the glyph");
            crate::widgets::row_activate::click_row(&accepted.row);
            pump();
            assert!(switch.is_active() && pref.get(), "a click on the row toggles it");
            accepted.row.emit_by_name::<()>("activate", &[]);
            pump();
            assert!(!switch.is_active() && !pref.get(), "Enter or Space toggles it back");

            let vetoed_pref = prefs.store().pref("switch_row_vetoed", false);
            let vetoed = build(&ItemCommon::new("Vetoed"), vetoed_pref.clone(), bind::veto(|_: bool| async { Ok(false) }));
            list.append(&vetoed.row);
            let vetoed_switch = switch_in(vetoed.row.upcast_ref());
            crate::widgets::row_activate::click_row(&vetoed.row);
            pump();
            assert!(!vetoed_switch.is_active() && !vetoed_pref.get(), "a vetoed write leaves the switch and the value");
        });
    }
}
