//! `kuro_platform::notifier::Notifier` over `gio::Notification`. The trait is `Send + Sync` and GTK objects are
//! not, so calls go through the platform's [`QueuedNotifier`] to the main context, where the application sends them.
//! Every send goes through `show_unless_muted`, so a channel the user muted (the `muted_notification_channels`
//! preference) never reaches the desktop, and a full [`Notice`] keeps its progress and its action buttons.

use std::collections::BTreeSet;

use gtk::glib;
use gtk::prelude::*;
use kuro_platform::linux::notifier::{Command, QueuedNotifier, to_notification};
use kuro_platform::notifier::{Notice, Notifier, show_unless_muted};
use kuro_prefs::pref::Pref;

#[derive(Clone)]
pub struct GioNotifier {
    queue: QueuedNotifier,
    muted: Option<Pref<BTreeSet<String>>>,
}

/// The channel of a plain `show` call: the id is `"{channel}:{n}"` (`notifications::key`), so the channel is its prefix.
fn channel_of(id: &str) -> &str {
    id.split(':').next().unwrap_or(id)
}

/// The notice a plain `show` call stands for. A `None` body stays hidden (an empty text has no body).
fn plain_notice(id: &str, title: &str, body: Option<&str>) -> Notice {
    Notice { id: id.into(), channel: channel_of(id).into(), title: title.into(), text: body.unwrap_or_default().into(), ..Notice::default() }
}

impl GioNotifier {
    /// `default_actions` maps a notification id to the application action its click activates.
    /// Call from the GTK main thread.
    pub fn new(app: &gtk::Application, default_actions: Vec<(String, String)>) -> Self {
        let (queue, rx) = QueuedNotifier::new();
        let app = app.clone();
        glib::spawn_future_local(async move {
            while let Ok(cmd) = rx.recv().await {
                match cmd {
                    Command::Show(spec) => {
                        let n = to_notification(&spec);
                        if let Some((_, action)) = default_actions.iter().find(|(i, _)| *i == spec.id) {
                            n.set_default_action(action);
                        }
                        app.send_notification(Some(&spec.id), &n);
                    }
                    Command::Withdraw(id) => app.withdraw_notification(&id),
                }
            }
        });
        let muted = crate::data::is_installed().then(|| crate::data::app().base.muted_notification_channels.clone());
        Self { queue, muted }
    }

    fn muted_channels(&self) -> Vec<String> {
        self.muted.as_ref().map(|p| p.get().into_iter().collect()).unwrap_or_default()
    }
}

impl Notifier for GioNotifier {
    fn show(&self, id: &str, title: &str, body: Option<&str>) {
        let notice = plain_notice(id, title, body);
        let muted = self.muted_channels();
        match body {
            Some(_) => {
                show_unless_muted(&self.queue, &notice, &muted);
            }
            // a hidden body stays `None` all the way to the desktop
            None if !notice.is_muted(&muted) => self.queue.show(id, title, None),
            None => {}
        }
    }

    fn show_notice(&self, notice: &Notice) {
        show_unless_muted(&self.queue, notice, &self.muted_channels());
    }

    fn withdraw(&self, id: &str) {
        self.queue.withdraw(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_notice_takes_its_channel_from_the_key_prefix() {
        assert_eq!(plain_notice("incognito_mode_channel:-701", "t", None).channel, "incognito_mode_channel");
        assert_eq!(plain_notice("restore", "t", Some("b")).channel, "restore");
        assert_eq!(plain_notice("a:1", "t", Some("b")).text, "b");
    }
}
