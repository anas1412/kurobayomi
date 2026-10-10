//! The Linux [`Notifier`] (T-req-platform-004). A [`Notice`] becomes a `gio::Notification` with its text, its progress
//! (in the body, see [`Notice::body`]) and its action buttons. A click on a button activates the application action
//! named by the button, so the app routes it back into itself.
//!
//! Library code holds a [`QueuedNotifier`]. The `gio::Application` is owned by the main thread, so the application
//! drains the receiver on the GLib main context and calls [`deliver`] for each command.

use gio::prelude::*;

use crate::notifier::{Notice, Notifier};

/// What the desktop shows for one notice. It holds plain strings, so it is built and tested without a display.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spec {
    pub id: String,
    pub title: String,
    /// `None` when the notice must not show its content (the `hide_notification_content` case).
    pub body: Option<String>,
    /// `(label, application action)` in the order the notice lists them.
    pub buttons: Vec<(String, String)>,
}

/// The spec of a full notice: its text with progress, and its buttons.
pub fn spec(notice: &Notice) -> Spec {
    Spec {
        id: notice.id.clone(),
        title: notice.title.clone(),
        body: Some(notice.body()),
        buttons: notice.actions.iter().map(|a| (a.label.clone(), a.action.clone())).collect(),
    }
}

/// One call to the desktop, to be made on the thread that owns the application.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Show(Spec),
    Withdraw(String),
}

/// A [`Notifier`] that queues its calls. It is `Send + Sync`, so any thread can hold it. A full queue cannot happen
/// (the channel is unbounded) and a closed one means the application stopped, so the send result is ignored.
#[derive(Clone)]
pub struct QueuedNotifier(async_channel::Sender<Command>);

impl QueuedNotifier {
    /// The notifier and the receiver the application drains.
    pub fn new() -> (Self, async_channel::Receiver<Command>) {
        let (tx, rx) = async_channel::unbounded();
        (Self(tx), rx)
    }
}

impl Notifier for QueuedNotifier {
    fn show(&self, id: &str, title: &str, body: Option<&str>) {
        let spec = Spec { id: id.into(), title: title.into(), body: body.map(str::to_owned), buttons: Vec::new() };
        let _ = self.0.try_send(Command::Show(spec));
    }

    fn show_notice(&self, notice: &Notice) {
        let _ = self.0.try_send(Command::Show(spec(notice)));
    }

    fn withdraw(&self, id: &str) {
        let _ = self.0.try_send(Command::Withdraw(id.into()));
    }
}

/// The `gio::Notification` for a spec. Each button is added with its application action; no default action is set,
/// so a click on the body only raises the window.
pub fn to_notification(spec: &Spec) -> gio::Notification {
    let notification = gio::Notification::new(&spec.title);
    notification.set_body(spec.body.as_deref());
    for (label, action) in &spec.buttons {
        notification.add_button(label, action);
    }
    notification
}

/// Makes one call on `app`. Call it from the GLib main context, where the application lives.
pub fn deliver(app: &gio::Application, command: &Command) {
    match command {
        Command::Show(spec) => app.send_notification(Some(&spec.id), &to_notification(spec)),
        Command::Withdraw(id) => app.withdraw_notification(id),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifier::{Action, Progress};

    fn notice() -> Notice {
        Notice {
            id: "library-update".into(),
            channel: "library_progress_channel".into(),
            title: "Naruto 12".into(),
            text: "3 new chapters".into(),
            progress: Some(Progress { done: 3, total: 10 }),
            actions: vec![
                Action { label: "Open".into(), action: "app.open-entry".into() },
                Action { label: "Mark read".into(), action: "app.mark-read".into() },
            ],
        }
    }

    #[test]
    fn a_notice_keeps_its_text_progress_and_buttons_in_order() {
        let s = spec(&notice());
        assert_eq!(s.id, "library-update");
        assert_eq!(s.title, "Naruto 12");
        assert_eq!(s.body.as_deref(), Some("3 new chapters (3/10)"));
        assert_eq!(
            s.buttons,
            vec![("Open".into(), "app.open-entry".into()), ("Mark read".into(), "app.mark-read".into())]
        );
    }

    #[test]
    fn a_notice_without_actions_has_no_buttons() {
        let plain = Notice { actions: Vec::new(), ..notice() };
        assert!(spec(&plain).buttons.is_empty());
    }

    #[test]
    fn a_plain_show_call_queues_a_spec_without_buttons_and_keeps_hidden_bodies_hidden() {
        let (notifier, rx) = QueuedNotifier::new();
        notifier.show("n", "Kurobayomi", None);
        notifier.show_notice(&notice());
        notifier.withdraw("n");
        assert_eq!(
            rx.try_recv().unwrap(),
            Command::Show(Spec { id: "n".into(), title: "Kurobayomi".into(), body: None, buttons: Vec::new() })
        );
        assert!(matches!(rx.try_recv().unwrap(), Command::Show(s) if s.buttons.len() == 2));
        assert_eq!(rx.try_recv().unwrap(), Command::Withdraw("n".into()));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn a_built_notification_can_be_created_without_a_display() {
        // The GLib object is only built here; sending needs a registered application.
        let _ = to_notification(&spec(&notice()));
    }
}
