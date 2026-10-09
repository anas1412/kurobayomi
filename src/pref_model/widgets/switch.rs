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
