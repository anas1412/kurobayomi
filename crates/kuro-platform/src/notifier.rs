//! The desktop notification boundary (CRATE-MAP C-04). The app implements [`Notifier`] over `gio::Notification`; library
//! code only calls it, so it never needs GTK.

/// Shows and withdraws desktop notices by id. A notice shown with an id that is already shown replaces it.
pub trait Notifier: Send + Sync {
    /// `body` is `None` when the notice must not show its content.
    fn show(&self, id: &str, title: &str, body: Option<&str>);
    /// Shows a [`Notice`]. The default sends its title and text through [`Notifier::show`] and drops the progress and
    /// the action buttons; an implementation that can draw them overrides this.
    fn show_notice(&self, notice: &Notice) {
        self.show(&notice.id, &notice.title, Some(&notice.body()));
    }
    /// Removes the notice with this id. A missing id is not an error.
    fn withdraw(&self, id: &str);
}

/// How far a long task has got, shown as `done/total` after the text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub done: u64,
    pub total: u64,
}

/// A button on a notice. Clicking it activates `action`, an application action name the app routes back to itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    pub label: String,
    pub action: String,
}

/// A notice with everything the desktop can show: title, text, progress and action buttons.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notice {
    /// Replaces an earlier notice with the same id.
    pub id: String,
    /// The channel id from `notifications::CATEGORIES`. The user mutes a whole channel by this id.
    pub channel: String,
    pub title: String,
    pub text: String,
    pub progress: Option<Progress>,
    pub actions: Vec<Action>,
}

impl Notice {
    /// The text with the progress appended, for example `3 new chapters (3/10)`. A notice with no text shows `3/10`.
    pub fn body(&self) -> String {
        match (self.progress, self.text.is_empty()) {
            (None, _) => self.text.clone(),
            (Some(p), true) => format!("{}/{}", p.done, p.total),
            (Some(p), false) => format!("{} ({}/{})", self.text, p.done, p.total),
        }
    }

    /// True when the user muted this notice's channel.
    pub fn is_muted(&self, muted_channels: &[String]) -> bool {
        muted_channels.contains(&self.channel)
    }
}

/// Shows `notice` unless the user muted its channel. Returns whether it was shown. `muted_channels` is the preference's
/// list of channel ids.
pub fn show_unless_muted(notifier: &dyn Notifier, notice: &Notice, muted_channels: &[String]) -> bool {
    if notice.is_muted(muted_channels) {
        return false;
    }
    notifier.show_notice(notice);
    true
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

    fn notice(text: &str, progress: Option<Progress>) -> Notice {
        Notice {
            id: "n".into(),
            channel: "library_progress_channel".into(),
            title: "Naruto 12".into(),
            text: text.into(),
            progress,
            actions: vec![Action { label: "Open".into(), action: "app.open".into() }],
        }
    }

    #[test]
    fn progress_is_appended_to_the_text_or_stands_alone() {
        let p = Some(Progress { done: 3, total: 10 });
        assert_eq!(notice("Downloading", p).body(), "Downloading (3/10)");
        assert_eq!(notice("", p).body(), "3/10");
        assert_eq!(notice("Done", None).body(), "Done");
    }

    #[test]
    fn a_muted_channel_mutes_its_notices_only() {
        let muted = vec!["library_progress_channel".to_owned()];
        assert!(notice("", None).is_muted(&muted));
        let other = Notice { channel: "common_channel".into(), ..notice("", None) };
        assert!(!other.is_muted(&muted));
        assert!(!notice("", None).is_muted(&[]));
    }

    /// Records what `show` received, to see what the default `show_notice` sends.
    #[derive(Default)]
    struct Recorder {
        shown: Mutex<Vec<(String, String, Option<String>)>>,
    }

    impl Notifier for Recorder {
        fn show(&self, id: &str, title: &str, body: Option<&str>) {
            self.shown.lock().unwrap().push((id.into(), title.into(), body.map(str::to_owned)));
        }
        fn withdraw(&self, _id: &str) {}
    }

    #[test]
    fn the_default_show_notice_sends_title_and_progressed_body_without_actions() {
        let rec = Recorder::default();
        rec.show_notice(&notice("Downloading", Some(Progress { done: 1, total: 4 })));
        assert_eq!(
            *rec.shown.lock().unwrap(),
            vec![("n".into(), "Naruto 12".into(), Some("Downloading (1/4)".into()))]
        );
    }

    #[test]
    fn a_muted_channel_is_not_shown_and_others_are() {
        let fake = Fake::default();
        let notice = Notice { id: "n".into(), channel: "downloader_channel".into(), title: "t".into(), ..Notice::default() };
        assert!(!show_unless_muted(&fake, &notice, &["downloader_channel".to_owned()]));
        assert!(fake.shown.lock().unwrap().is_empty());
        assert!(show_unless_muted(&fake, &notice, &["other".to_owned()]));
        assert_eq!(fake.shown.lock().unwrap().len(), 1);
    }
}
