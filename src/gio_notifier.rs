//! `kuro_platform::notifier::Notifier` over `gio::Notification`. The trait is `Send + Sync` and GTK objects are
//! not, so calls go through a channel to the main context, where the application sends them.

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_platform::notifier::Notifier;

enum Cmd {
    Show { id: String, title: String, body: Option<String> },
    Withdraw(String),
}

#[derive(Clone)]
pub struct GioNotifier(async_channel::Sender<Cmd>);

impl GioNotifier {
    /// `default_actions` maps a notification id to the application action its click activates.
    /// Call from the GTK main thread.
    pub fn new(app: &gtk::Application, default_actions: Vec<(String, String)>) -> Self {
        let (tx, rx) = async_channel::unbounded();
        let app = app.clone();
        glib::spawn_future_local(async move {
            while let Ok(cmd) = rx.recv().await {
                match cmd {
                    Cmd::Show { id, title, body } => {
                        let n = gio::Notification::new(&title);
                        n.set_body(body.as_deref());
                        if let Some((_, action)) = default_actions.iter().find(|(i, _)| *i == id) {
                            n.set_default_action(action);
                        }
                        app.send_notification(Some(&id), &n);
                    }
                    Cmd::Withdraw(id) => app.withdraw_notification(&id),
                }
            }
        });
        Self(tx)
    }
}

impl Notifier for GioNotifier {
    fn show(&self, id: &str, title: &str, body: Option<&str>) {
        let _ = self.0.try_send(Cmd::Show { id: id.into(), title: title.into(), body: body.map(Into::into) });
    }

    fn withdraw(&self, id: &str) {
        let _ = self.0.try_send(Cmd::Withdraw(id.into()));
    }
}
