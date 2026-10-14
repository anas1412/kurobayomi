//! Incognito indicator (T-01a-020): the banner precedence and the persistent notification.
//! The banner widget is GTK and lives in the GUI crate; this module holds the parts that do not need a display.

use crate::notifier::{Notice, Notifier, Tap};
use crate::notifications;

/// Channel that the incognito notification is posted on.
pub const CHANNEL_ID: &str = "incognito_mode_channel";
/// Notification id on that channel (`ID_INCOGNITO_MODE`).
pub const NOTIFICATION_ID: i32 = -701;
/// Title of the banner and of the notification (`pref_incognito_mode`).
pub const TITLE: &str = "Incognito mode";
/// Text of the notification and the label of the banner button (`notification_incognito_text`).
pub const DISABLE_TEXT: &str = "Disable incognito mode";
/// The application action that turns incognito off (the notification default action and the banner button).
pub const DISABLE_ACTION: &str = "app.incognito-off";

/// The banner shown at the top of the main window. Only one is visible at a time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Banner {
    Indexing,
    DownloadedOnly,
    Incognito,
}

/// The banner that wins: indexing, then downloaded-only, then incognito (`MainActivity` order).
pub fn active_banner(indexing: bool, downloaded_only: bool, incognito: bool) -> Option<Banner> {
    if indexing {
        Some(Banner::Indexing)
    } else if downloaded_only {
        Some(Banner::DownloadedOnly)
    } else if incognito {
        Some(Banner::Incognito)
    } else {
        None
    }
}

/// The string key the notifier uses for the incognito notification: `incognito_mode_channel:-701`.
pub fn notification_key() -> String {
    notifications::key(CHANNEL_ID, NOTIFICATION_ID)
}

/// Shows the notification while incognito is on and withdraws it when off. Called at startup and on every change.
pub fn sync_notification(incognito: bool, notifier: &dyn Notifier) {
    let key = notification_key();
    if incognito {
        notifier.notify(&Notice {
            id: key,
            title: TITLE.to_owned(),
            body: Some(DISABLE_TEXT.to_owned()),
            low_priority: true,
            tap: Some(Tap::App(DISABLE_ACTION.to_owned())),
            ..Notice::default()
        });
    } else {
        notifier.withdraw(&key);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[derive(Default)]
    struct Recorder(Mutex<Vec<String>>);

    impl Notifier for Recorder {
        fn show(&self, id: &str, title: &str, body: Option<&str>) {
            self.0.lock().unwrap().push(format!("show {id} {title} {}", body.unwrap_or("-")));
        }
        fn withdraw(&self, id: &str) {
            self.0.lock().unwrap().push(format!("withdraw {id}"));
        }
    }

    #[test]
    fn banner_precedence_is_indexing_then_downloaded_only_then_incognito() {
        assert_eq!(active_banner(true, true, true), Some(Banner::Indexing));
        assert_eq!(active_banner(false, true, true), Some(Banner::DownloadedOnly));
        assert_eq!(active_banner(false, false, true), Some(Banner::Incognito));
        assert_eq!(active_banner(false, false, false), None);
    }

    #[test]
    fn notification_is_posted_while_on_and_withdrawn_when_off() {
        let rec = Recorder::default();
        sync_notification(true, &rec);
        sync_notification(false, &rec);
        assert_eq!(
            *rec.0.lock().unwrap(),
            [
                "show incognito_mode_channel:-701 Incognito mode Disable incognito mode",
                "withdraw incognito_mode_channel:-701",
            ]
        );
    }
}
