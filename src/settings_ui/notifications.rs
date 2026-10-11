//! The one notification switch of the Library page: Notify about new chapters and episodes. It stores into the
//! `muted_notification_channels` set (the channel is muted while the switch is off), which the app notifier reads on
//! every send, so a backup of the 13-channel list still imports. The other channels follow the system settings.
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::base::BasePrefs;

use crate::pref_model::model::PreferenceItem;
use crate::settings_ui::rows::tap_item;

/// The channel the switch turns on or off.
pub const NEW_ITEMS_CHANNEL: &str = "new_chapters_episodes_channel";
/// The title of the switch.
pub const NEW_ITEMS_TITLE: &str = "Notify about new chapters and episodes";

/// `muted` with the new-items channel added or taken out; every other channel keeps its state.
fn with_channel_muted(muted: &BTreeSet<String>, mute: bool) -> BTreeSet<String> {
    let mut next = muted.clone();
    if mute {
        next.insert(NEW_ITEMS_CHANNEL.to_owned());
    } else {
        next.remove(NEW_ITEMS_CHANNEL);
    }
    next
}

/// The switch row. It is on while the channel is not muted; a tap on the row turns the switch.
pub fn notify_row(base: &BasePrefs) -> PreferenceItem {
    let pref = base.muted_notification_channels.clone();
    let trailing: Rc<dyn Fn() -> gtk::Widget> = Rc::new(move || {
        let switch = gtk::Switch::new();
        switch.set_valign(gtk::Align::Center);
        switch.set_active(!pref.get().contains(NEW_ITEMS_CHANNEL));
        let pref = pref.clone();
        switch.connect_active_notify(move |switch| {
            if let Err(err) = pref.set(with_channel_muted(&pref.get(), !switch.is_active())) {
                tracing::warn!(error = %err, "the notification switch could not be stored");
            }
        });
        switch.upcast()
    });
    let on_tap: Rc<dyn Fn(&crate::settings_ui::rows::TapRow)> = Rc::new(|row| {
        if let Some(switch) = row.trailing.as_ref().and_then(|widget| widget.downcast_ref::<gtk::Switch>()) {
            switch.set_active(!switch.is_active());
        }
    });
    tap_item(NEW_ITEMS_TITLE, None, true, Some(trailing), None, on_tap)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_platform::notifications::CATEGORIES;

    #[test]
    fn the_switch_names_a_real_channel() {
        assert!(CATEGORIES.iter().any(|category| category.channel_id == NEW_ITEMS_CHANNEL));
    }

    #[test]
    fn turning_the_switch_off_mutes_only_the_new_items_channel() {
        let muted = BTreeSet::from(["common_channel".to_owned()]);
        let off = with_channel_muted(&muted, true);
        assert_eq!(off, BTreeSet::from(["common_channel".to_owned(), NEW_ITEMS_CHANNEL.to_owned()]));
        assert_eq!(with_channel_muted(&off, false), muted);
    }

    #[test]
    fn the_row_has_the_diet_title() {
        let prefs = kuro_prefs::prefs::Prefs::in_memory();
        assert_eq!(notify_row(&prefs.base).title(), NEW_ITEMS_TITLE);
    }
}
