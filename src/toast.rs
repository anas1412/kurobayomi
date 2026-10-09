//! The toast overlay (T-12a-014 presenter). `kuro_platform::toast` queues the messages; this shows them one at a
//! time at the bottom of the main window, each for its length. GTK objects stay on the main thread.

use gtk::glib;
use kuro_platform::toast::{ToastFeed, Toaster, toast_channel};

/// Adds the toast layer to `overlay` and starts the presenter on the main context. Returns the sender that
/// producers use. Call once, from the GTK main thread.
pub fn attach(overlay: &gtk::Overlay) -> Toaster {
    let (toaster, feed) = toast_channel();
    let label = gtk::Label::builder()
        .wrap(true)
        .max_width_chars(60)
        .css_classes(["osd"])
        .margin_bottom(24)
        .margin_start(12)
        .margin_end(12)
        .build();
    let revealer = gtk::Revealer::builder()
        .child(&label)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::End)
        .transition_type(gtk::RevealerTransitionType::SlideUp)
        .can_target(false)
        .build();
    overlay.add_overlay(&revealer);
    glib::spawn_future_local(present(feed, label, revealer, toaster.clone()));
    toaster
}

/// `keep_open` holds a sender for as long as the presenter runs, so the feed stays open even when the
/// returned handle is dropped.
async fn present(feed: ToastFeed, label: gtk::Label, revealer: gtk::Revealer, keep_open: Toaster) {
    while let Some(message) = feed.next().await {
        label.set_text(&message.text);
        revealer.set_reveal_child(true);
        glib::timeout_future(message.length.duration()).await;
        revealer.set_reveal_child(false);
    }
    drop(keep_open);
}
