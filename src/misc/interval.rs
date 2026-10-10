//! The automatic library update interval (T-26a-104): the six choices stored as hours under
//! `pref_library_update_interval_key`, and the feed that tells both kinds' schedule loops when it changes.

use kuro_prefs::pref::Pref;
use tokio::sync::watch;

use crate::pref_model::model::Preference;
use crate::settings_ui::rows::{group, info, int_choice};

/// Shown under the interval choice: when the first automatic run happens (T-26a-010).
pub const FIRST_RUN_NOTE: &str = "The first automatic update runs as soon as the conditions are met after you turn this on.";

/// Stored hours and label of each choice. 0 is Off, the default.
pub const CHOICES: [(i32, &str); 6] = [
    (0, "Off"),
    (12, "Every 12 hours"),
    (24, "Daily"),
    (48, "Every 2 days"),
    (72, "Every 3 days"),
    (168, "Weekly"),
];

/// The hours the schedule runs on for a stored value. A value that is not one of the six choices (an old or edited
/// file) means Off.
pub fn hours(stored: i32) -> u32 {
    if CHOICES.iter().any(|(h, _)| *h == stored) { stored as u32 } else { 0 }
}

// Not called yet: `settings_ui::library::page_rows` (another lane) adds `misc::interval::rows(&l.auto_update_interval)`
// to its typed rows. The schedule below already follows the preference whoever writes it.
#[allow(dead_code)]
/// The settings row. The library settings page merges it over the spec row of the same group and title, so the
/// choice reads and writes the real preference.
pub fn rows(pref: &Pref<i32>) -> Vec<Preference> {
    vec![group("Global update", vec![int_choice("Automatic updates", pref, &CHOICES, true), info(FIRST_RUN_NOTE)])]
}

/// Sends `stored` as hours to every schedule loop. Each loop restarts its wait on the change (`run_schedule`).
pub fn apply(senders: &[watch::Sender<u32>], stored: i32) {
    let hours = hours(stored);
    for sender in senders {
        sender.send_replace(hours);
    }
}

/// Polls the preference on the GTK main thread and feeds `senders` on every change. The first value is sent now.
pub fn pump(pref: &Pref<i32>, senders: Vec<watch::Sender<u32>>) {
    use gtk::glib;
    apply(&senders, pref.get());
    let changes = pref.watch();
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        if let Some(stored) = changes.try_changed() {
            apply(&senders, stored);
        }
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::store::PrefStore;

    #[test]
    fn the_six_choices_store_0_12_24_48_72_168_and_default_to_off() {
        let stored: Vec<i32> = CHOICES.iter().map(|(h, _)| *h).collect();
        assert_eq!(stored, [0, 12, 24, 48, 72, 168]);
        let labels: Vec<&str> = CHOICES.iter().map(|(_, l)| *l).collect();
        assert_eq!(labels, ["Off", "Every 12 hours", "Daily", "Every 2 days", "Every 3 days", "Weekly"]);
        let pref = Pref::new(PrefStore::in_memory(), "pref_library_update_interval_key", 0_i32);
        assert_eq!(hours(pref.get()), 0);
    }

    #[test]
    fn a_stored_value_outside_the_choices_means_off() {
        assert_eq!(hours(24), 24);
        assert_eq!(hours(5), 0);
        assert_eq!(hours(-12), 0);
        assert_eq!(hours(169), 0);
    }

    #[test]
    fn a_change_reaches_both_kinds_at_once() {
        let (manga, mut manga_rx) = watch::channel(0_u32);
        let (anime, mut anime_rx) = watch::channel(0_u32);
        let senders = [manga, anime];
        apply(&senders, 72);
        assert!(manga_rx.has_changed().unwrap() && anime_rx.has_changed().unwrap());
        assert_eq!((*manga_rx.borrow_and_update(), *anime_rx.borrow_and_update()), (72, 72));
    }

    #[test]
    fn the_row_binds_the_interval_preference() {
        let pref = Pref::new(PrefStore::in_memory(), "pref_library_update_interval_key", 0_i32);
        let tree = rows(&pref);
        let Preference::Group(g) = &tree[0] else { panic!("one group") };
        assert_eq!(g.title, "Global update");
        assert_eq!(g.items[0].title(), "Automatic updates");
    }
}
