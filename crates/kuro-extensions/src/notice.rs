//! The update-available notice and the badge count (`ExtensionUpdateNotifier` and `updatePendingUpdatesCount`;
//! T-20a-050, T-20a-002). The title is the English base of `update_check_notification_ext_updates`; translations belong
//! to the i18n area.

use kuro_platform::notifier::{APP_NAME, Notifier, send};
use kuro_prefs::pref::Pref;

/// `Notifications.ID_UPDATES_TO_EXTS`.
pub const UPDATES_NOTICE_ID: &str = "updates-to-exts";

/// The generic body of a hidden extension update notice.
pub const GENERIC_TEXT: &str = "Extension updates are available";

/// The plural title: one update reads "Extension update available", more read "N extension updates available".
pub fn update_title(count: usize) -> String {
    if count == 1 { "Extension update available".to_owned() } else { format!("{count} extension updates available") }
}

/// `promptUpdates`: shows the notice for the names of the packages with an update, which the caller passes non-empty.
/// The body is the names joined with ", ". With `hide_content` (`hide_notification_content`) the notice goes out
/// through `send` as sensitive: the title is the app name and the body is [`GENERIC_TEXT`].
pub fn prompt_updates(notifier: &dyn Notifier, names: &[String], hide_content: bool) {
    send(notifier, hide_content, true, UPDATES_NOTICE_ID, APP_NAME, &update_title(names.len()), &names.join(", "), GENERIC_TEXT);
}

/// `updatePendingUpdatesCount`: stores the pending count in the badge preference, and withdraws the notice when no
/// update is pending.
pub fn sync_pending_count(count_pref: &Pref<i32>, pending: usize, notifier: &dyn Notifier) {
    if let Err(e) = count_pref.set(i32::try_from(pending).unwrap_or(i32::MAX)) {
        tracing::warn!("could not store the pending extension update count: {e}");
    }
    if pending == 0 {
        notifier.withdraw(UPDATES_NOTICE_ID);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use kuro_prefs::store::PrefStore;

    use super::*;

    #[derive(Default)]
    struct Recorder {
        shown: Mutex<Vec<(String, Option<String>)>>,
        withdrawn: Mutex<Vec<String>>,
    }

    impl Notifier for Recorder {
        fn show(&self, id: &str, title: &str, body: Option<&str>) {
            assert_eq!(id, UPDATES_NOTICE_ID);
            self.shown.lock().unwrap().push((title.to_owned(), body.map(str::to_owned)));
        }

        fn withdraw(&self, id: &str) {
            self.withdrawn.lock().unwrap().push(id.to_owned());
        }
    }

    fn names() -> Vec<String> {
        vec!["Alpha".to_owned(), "Beta".to_owned()]
    }

    #[test]
    fn the_title_is_singular_for_one_and_counted_for_more() {
        assert_eq!(update_title(1), "Extension update available");
        assert_eq!(update_title(2), "2 extension updates available");
    }

    #[test]
    fn the_body_joins_the_names_unless_the_content_is_hidden() {
        let recorder = Recorder::default();
        prompt_updates(&recorder, &names(), false);
        prompt_updates(&recorder, &names(), true);
        let shown = recorder.shown.lock().unwrap();
        assert_eq!(shown[0], ("2 extension updates available".to_owned(), Some("Alpha, Beta".to_owned())));
        assert_eq!(shown[1], (APP_NAME.to_owned(), Some(GENERIC_TEXT.to_owned())));
    }

    #[test]
    fn the_count_is_stored_and_the_notice_is_withdrawn_only_at_zero() {
        let recorder = Recorder::default();
        let pref = PrefStore::in_memory().pref("manga_extension_updates_count", 0);
        sync_pending_count(&pref, 2, &recorder);
        assert_eq!(pref.get(), 2);
        assert!(recorder.withdrawn.lock().unwrap().is_empty(), "updates are still pending");
        sync_pending_count(&pref, 0, &recorder);
        assert_eq!(pref.get(), 0);
        assert_eq!(*recorder.withdrawn.lock().unwrap(), [UPDATES_NOTICE_ID]);
    }
}
