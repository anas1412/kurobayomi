//! The window side of the app lock (T-02a-022): leaving the window stores `__APP_STATE_last_app_closed` when the lock
//! waits a number of minutes, coming back deletes it. The rules are `kuro_prefs::applock`; this file feeds them the
//! window activity and the stored value.
//!
//! Linux has no unlock method yet, so nothing can be asked to unlock: the state is unlocked again after every step.
//! The unlock dialog, when it exists, replaces that line.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_prefs::applock::{AppLockConfig, AppLockState, on_application_start, on_application_stopped};
use kuro_prefs::security::SecurityPrefs;

/// One activity change. `active` is true when the window comes to the front.
fn step(sec: &SecurityPrefs, active: bool, now: i64, state: AppLockState) -> AppLockState {
    let cfg = AppLockConfig { use_authenticator: sec.use_authenticator.get(), lock_app_after: sec.lock_app_after.get() };
    let stored = sec.last_app_closed.is_set().then(|| sec.last_app_closed.get());
    let from = AppLockState { last_app_closed: stored, ..state };
    let next = if active { on_application_start(cfg, false, now, from) } else { on_application_stopped(cfg, false, now, from) };
    if next.last_app_closed != stored {
        let saved = match next.last_app_closed {
            Some(at) => sec.last_app_closed.set(at),
            None => sec.last_app_closed.delete(),
        };
        if let Err(e) = saved {
            tracing::warn!(error = %e, "could not store last_app_closed");
        }
    }
    next.unlock()
}

/// Follows the activity of `win` for as long as it lives. The window starts in the foreground, so the first step is
/// a start.
pub fn attach(win: &gtk::ApplicationWindow, sec: SecurityPrefs) {
    let now = || i64::try_from(kuro_core::crash::now_ms()).unwrap_or(i64::MAX);
    let state = Rc::new(Cell::new(step(&sec, true, now(), AppLockState::at_process_start())));
    win.connect_is_active_notify(move |w| state.set(step(&sec, w.is_active(), now(), state.get())));
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    const T: i64 = 1_000_000;

    fn prefs(use_auth: bool, after: i32) -> SecurityPrefs {
        let sec = SecurityPrefs::new(PrefStore::in_memory());
        sec.use_authenticator.set(use_auth).unwrap();
        sec.lock_app_after.set(after).unwrap();
        sec
    }

    #[test]
    fn leaving_stores_the_time_only_when_the_lock_waits_minutes_and_coming_back_deletes_it() {
        let sec = prefs(true, 5);
        let st = AppLockState::at_process_start().unlock();
        let st = step(&sec, false, T, st);
        assert_eq!(sec.last_app_closed.get(), T);
        assert!(sec.last_app_closed.is_set());
        step(&sec, true, T + 1, st);
        assert!(!sec.last_app_closed.is_set(), "entering the foreground deletes it");
    }

    #[test]
    fn nothing_is_stored_for_lock_after_zero_never_or_a_disabled_lock() {
        for sec in [prefs(true, 0), prefs(true, -1), prefs(false, 5)] {
            step(&sec, false, T, AppLockState::at_process_start().unlock());
            assert!(!sec.last_app_closed.is_set());
        }
    }

    #[test]
    fn a_value_left_by_an_earlier_run_is_deleted_at_the_start() {
        let sec = prefs(true, 5);
        sec.last_app_closed.set(T).unwrap();
        step(&sec, true, T + 1, AppLockState::at_process_start());
        assert!(!sec.last_app_closed.is_set());
    }
}
