//! Onboarding gate (Kotlin `OnboardingScreen`): back and close are blocked until the completion flag is set.
//! Finishing sets the flag, then leaves. Restoring a backup also highlights the Backup row on Data and storage.

use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;

use super::highlight;
use super::strings;

/// Close and back are allowed only after onboarding is complete (T-E66).
pub fn close_allowed(complete: bool) -> bool {
    complete
}

/// Blocks the window's close request until onboarding is complete.
pub fn guard(window: &impl IsA<gtk::Window>, complete: Pref<bool>) {
    window.connect_close_request(move |_| {
        if close_allowed(complete.get()) {
            glib::Propagation::Proceed
        } else {
            glib::Propagation::Stop
        }
    });
}

/// Sets the completion flag, then runs `leave` (T-E67: the flag is set even if leaving fails later).
pub fn finish(complete: &Pref<bool>, leave: impl FnOnce()) {
    if let Err(err) = complete.set(true) {
        tracing::warn!(%err, "could not save onboarding state");
    }
    leave();
}

/// Restore path: finish, then request the highlight, then open Data and storage. The request is set
/// before the page is pushed, so the flash happens once (R-03).
pub fn restore(complete: &Pref<bool>, leave: impl FnOnce(), open_data: impl FnOnce()) {
    finish(complete, leave);
    highlight::request(strings::label_backup());
    open_data();
}

/// The onboarding screen's id in the spec.
pub const SCREEN: &str = "onboarding";

/// The screen the restore path opens: Data and storage, where the backup rows are.
pub const DATA_AND_STORAGE: &str = "settings_data_storage";

/// Whether the action row `title` of screen `screen` is the onboarding's Restore backup.
pub fn is_restore_action(screen: &str, title: &str) -> bool {
    screen == SCREEN && title == tr(MsgKey::pref_restore_backup)
}

/// Runs the onboarding's Restore action. Returns false for any other row, which the caller handles as usual. Restore
/// finishes onboarding, leaves the onboarding page, asks for the highlight and only then pushes Data and storage, so
/// that page shows the flash once.
pub fn on_action(screen: &str, title: &str) -> bool {
    if !is_restore_action(screen, title) {
        return false;
    }
    let complete = crate::data::app().base.shown_onboarding_flow.clone();
    restore(&complete, crate::ui::pop, || crate::ui::open(DATA_AND_STORAGE));
    true
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use kuro_prefs::base::BasePrefs;
    use kuro_prefs::store::PrefStore;

    use super::*;

    #[test]
    fn back_is_blocked_until_onboarding_is_complete() {
        assert!(!close_allowed(false));
        assert!(close_allowed(true));
    }

    #[test]
    fn finish_sets_the_flag_and_leaves() {
        let base = BasePrefs::new(PrefStore::in_memory());
        assert!(!base.shown_onboarding_flow.get());
        finish(&base.shown_onboarding_flow, || {});
        assert!(base.shown_onboarding_flow.get());
    }

    #[test]
    fn t_47ba_061_only_the_restore_row_of_the_onboarding_is_the_restore_action() {
        assert!(is_restore_action("onboarding", "Restore backup"));
        assert!(!is_restore_action("onboarding", "Getting started guide"));
        assert!(!is_restore_action("settings_data_storage", "Restore backup"), "Data and storage opens the plain page");
        crate::ui::with_spec(|spec| {
            let onboarding = spec.screen(SCREEN).expect("the onboarding screen");
            let has_restore = onboarding.groups.iter().flat_map(|g| &g.items).any(|i| crate::pref_model::spec_convert::item_title(i) == "Restore backup");
            assert!(has_restore, "the spec's step 4 carries the row");
            assert!(spec.screen(DATA_AND_STORAGE).is_some());
        });
    }

    #[test]
    fn restore_requests_the_backup_highlight_before_opening_the_page() {
        let base = BasePrefs::new(PrefStore::in_memory());
        let order = RefCell::new(Vec::new());
        restore(
            &base.shown_onboarding_flow,
            || order.borrow_mut().push("leave"),
            || {
                order.borrow_mut().push(if highlight::take() == Some(strings::label_backup()) { "open+highlight" } else { "open" });
            },
        );
        assert_eq!(*order.borrow(), vec!["leave", "open+highlight"]);
        assert!(base.shown_onboarding_flow.get());
    }
}
