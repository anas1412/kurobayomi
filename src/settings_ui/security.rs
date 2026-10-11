//! The Security and privacy page rows this module binds: Require unlock (T-47a-063), Lock when idle (T-47a-064), the
//! Hide notification content switch (T-47a-065); the unlock gate every lock change passes is `auth` (T-47a-067, T-47a-068).
//! The runner already shows the generic summary when Hide notification content is on (`misc::runner`), so that row only
//! writes its preference.
use kuro_prefs::prefs::Prefs;

use crate::pref_model::bind::{self, Veto};
use crate::pref_model::model::{Preference, PreferenceItem};
use crate::settings_ui::auth;
use crate::settings_ui::rows::{int_choice, item, switch, switch_with};

pub const REQUIRE_UNLOCK_TITLE: &str = "Require unlock";
pub const LOCK_WHEN_IDLE_TITLE: &str = "Lock when idle";
pub const HIDE_NOTIFICATION_TITLE: &str = "Hide notification content";
/// Whether an unlock method exists: the passphrase method needs only the app's secret store, which always has a
/// file fallback, so the Require unlock switch is always sensitive (T-47a-063). Fingerprint is not built.
pub const UNLOCK_METHOD_AVAILABLE: bool = true;

/// Lock-when-idle choices in minutes: `0` Always, `-1` Never (`lock_app_after`).
pub const LOCK_AFTER: [(i32, &str); 6] = [
    (0, "Always"),
    (1, "After 1 minute"),
    (2, "After 2 minutes"),
    (5, "After 5 minutes"),
    (10, "After 10 minutes"),
    (-1, "Never"),
];

/// The veto of the Require unlock switch: the passphrase is set on, and an unlock is needed on and off.
fn lock_veto() -> Veto<bool> {
    bind::veto(|on: bool| auth::set_lock(REQUIRE_UNLOCK_TITLE, on))
}

/// Puts the unlock confirm (T-47a-068) in front of a list row's own write.
fn gated(mut row: PreferenceItem, title: &'static str) -> PreferenceItem {
    if let PreferenceItem::BasicList { on_change, .. } = &mut row {
        let write = on_change.clone();
        *on_change = bind::veto(move |key: String| {
            let write = write.clone();
            async move {
                if auth::confirm(title, auth::CONFIRM_LOCK_CHANGE).await { write(key).await } else { Ok(false) }
            }
        });
    }
    row
}

/// The typed rows of the Security and privacy page, in the spec's order: the lock switch, lock when idle, and the
/// notification switch at the top level.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    let sec = &prefs.security;
    let lock_on = sec.use_authenticator.get();
    vec![
        item(switch_with(REQUIRE_UNLOCK_TITLE, None, &sec.use_authenticator, UNLOCK_METHOD_AVAILABLE, lock_veto())),
        item(gated(int_choice(LOCK_WHEN_IDLE_TITLE, &sec.lock_app_after, &LOCK_AFTER, lock_on), LOCK_WHEN_IDLE_TITLE)),
        item(switch(HIDE_NOTIFICATION_TITLE, &sec.hide_notification_content, true)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_model::model::PreferenceItem;

    /// The veto futures here resolve at once, so one poll is enough.
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let waker = std::task::Waker::noop();
        let mut cx = std::task::Context::from_waker(waker);
        let mut future = std::pin::pin!(future);
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("a veto that resolves at once stayed pending"),
        }
    }

    #[test]
    fn the_hide_notification_switch_is_off_by_default_and_writes_the_security_key() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let Preference::Item(PreferenceItem::Switch { common, pref, .. }) = &rows[2] else { panic!("a switch") };
        assert_eq!(common.title, HIDE_NOTIFICATION_TITLE);
        assert!(!pref.get());
        assert_eq!(pref.key(), "hide_notification_content");
    }

    #[test]
    fn require_unlock_is_sensitive_because_the_passphrase_method_is_available() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let Preference::Item(PreferenceItem::Switch { common, pref, .. }) = &rows[0] else { panic!("a switch") };
        assert_eq!(common.title, REQUIRE_UNLOCK_TITLE);
        assert!(common.enabled, "an unlock method is available");
        assert!(!pref.get());
        assert_eq!(pref.key(), "use_biometric_lock");
    }

    #[test]
    fn lock_when_idle_lists_the_six_delays_and_is_insensitive_while_the_lock_is_off() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs);
        let Preference::Item(PreferenceItem::BasicList { common, entries, value, .. }) = &rows[1] else {
            panic!("a list")
        };
        assert_eq!(common.title, LOCK_WHEN_IDLE_TITLE);
        assert!(!common.enabled);
        assert_eq!(value, "0");
        let labels: Vec<&str> = entries.iter().map(|(_, label)| label.as_str()).collect();
        assert_eq!(labels, ["Always", "After 1 minute", "After 2 minutes", "After 5 minutes", "After 10 minutes", "Never"]);
        let codes: Vec<&str> = entries.iter().map(|(code, _)| code.as_str()).collect();
        assert_eq!(codes, ["0", "1", "2", "5", "10", "-1"]);
    }

    #[test]
    fn the_lock_switch_veto_passes_turning_off_when_no_passphrase_is_set() {
        let veto = lock_veto();
        assert_eq!(block_on(veto(false)), Ok(true));
    }

    #[test]
    fn lock_when_idle_asks_first_and_then_writes_the_delay() {
        let prefs = Prefs::in_memory();
        prefs.security.use_authenticator.set(true).unwrap();
        let rows = page_rows(&prefs);
        let Preference::Item(PreferenceItem::BasicList { on_change, .. }) = &rows[1] else { panic!("a list") };
        // no passphrase is set in the test store, so the confirm passes at once and the choice is written
        assert_eq!(block_on(on_change("5".to_owned())), Ok(true));
        assert_eq!(prefs.security.lock_app_after.get(), 5);
    }
}
