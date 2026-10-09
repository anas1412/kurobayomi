//! The desktop notification boundary (CRATE-MAP C-04). The app implements [`Notifier`] over `gio::Notification`; library
//! code only calls it, so it never needs GTK.

/// Shows and withdraws desktop notices by id. A notice shown with an id that is already shown replaces it.
pub trait Notifier: Send + Sync {
    /// `body` is `None` when the notice must not show its content.
    fn show(&self, id: &str, title: &str, body: Option<&str>);
    /// Removes the notice with this id. A missing id is not an error.
    fn withdraw(&self, id: &str);
}

/// The app name that replaces a hidden notice's title.
pub const APP_NAME: &str = "Kurobayomi";

/// Sends a notice through `notifier`, applying `hide_notification_content` (T-01a-034).
///
/// When `hide_content` is set and the notice is `sensitive` (library-update, download and similar notices), the
/// title keeps only `app_name` and the body is replaced by `generic`, the category's generic text. Other notices
/// are sent unchanged, so incognito and crash notices pass `sensitive = false`.
#[allow(clippy::too_many_arguments)]
pub fn send(
    notifier: &dyn Notifier,
    hide_content: bool,
    sensitive: bool,
    id: &str,
    app_name: &str,
    title: &str,
    body: &str,
    generic: &str,
) {
    if hide_content && sensitive {
        notifier.show(id, app_name, Some(generic));
    } else {
        notifier.show(id, title, Some(body));
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    /// Records every `show` call.
    #[derive(Default)]
    struct Fake {
        shown: Mutex<Vec<(String, String, Option<String>)>>,
    }

    impl Notifier for Fake {
        fn show(&self, id: &str, title: &str, body: Option<&str>) {
            self.shown.lock().unwrap().push((id.into(), title.into(), body.map(str::to_owned)));
        }
        fn withdraw(&self, _id: &str) {}
    }

    fn one(hide: bool, sensitive: bool) -> (String, Option<String>) {
        let fake = Fake::default();
        send(&fake, hide, sensitive, "n", "Kurobayomi", "Naruto 12", "3 new chapters", "New chapters are available");
        let shown = fake.shown.lock().unwrap();
        assert_eq!(shown.len(), 1);
        (shown[0].1.clone(), shown[0].2.clone())
    }

    #[test]
    fn a_sensitive_notice_is_replaced_when_hiding_is_on() {
        assert_eq!(one(true, true), ("Kurobayomi".into(), Some("New chapters are available".into())));
    }

    #[test]
    fn hiding_leaves_insensitive_notices_alone() {
        assert_eq!(one(true, false), ("Naruto 12".into(), Some("3 new chapters".into())));
    }

    #[test]
    fn the_content_is_shown_when_hiding_is_off() {
        assert_eq!(one(false, true), ("Naruto 12".into(), Some("3 new chapters".into())));
    }
}
