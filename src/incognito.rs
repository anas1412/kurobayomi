//! Incognito indicator (T-01a-020): the window banner, the desktop notification and the `app.incognito-off` action.
//! The preference is the one source of truth; every change goes through [`set`], which updates the banner, the
//! notification and the More-screen switch so nothing needs a restart.

use std::cell::RefCell;

use gtk::glib;
use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_platform::incognito::{self as platform, Banner, DISABLE_ACTION};
use kuro_prefs::pref::Pref;

use crate::gio_notifier::GioNotifier;

#[derive(Clone)]
struct State {
    incognito: Pref<bool>,
    downloaded_only: Pref<bool>,
    revealer: gtk::Revealer,
    label: gtk::Label,
    button: gtk::Button,
    notifier: GioNotifier,
    switches: Vec<glib::WeakRef<gtk::Switch>>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

/// Banner text and whether it carries the disable button. The indexing banner has no source here yet.
fn banner_view(banner: Banner) -> (String, bool) {
    match banner {
        Banner::Indexing => (String::new(), false),
        Banner::DownloadedOnly => (tr(MsgKey::label_downloaded_only), false),
        Banner::Incognito => (tr(MsgKey::pref_incognito_mode), true),
    }
}

/// Registers `app.incognito-off`, builds the banner and applies the stored value (banner and notification).
/// Call once from the GTK main thread; the returned widget goes at the top of the main window.
pub fn attach(app: &gtk::Application, incognito: Pref<bool>, downloaded_only: Pref<bool>) -> gtk::Widget {
    let label = gtk::Label::builder().xalign(0.0).hexpand(true).build();
    let button = gtk::Button::builder().label(tr(MsgKey::notification_incognito_text)).css_classes(["flat"]).build();
    button.set_action_name(Some(DISABLE_ACTION));
    let bar = gtk::Box::builder().spacing(12).margin_start(12).margin_end(12).margin_top(6).margin_bottom(6).build();
    bar.append(&gtk::Image::from_icon_name("dialog-password-symbolic"));
    bar.append(&label);
    bar.append(&button);
    bar.add_css_class("toolbar");
    let revealer = gtk::Revealer::builder().child(&bar).transition_type(gtk::RevealerTransitionType::SlideDown).build();

    let off = gtk::gio::SimpleAction::new("incognito-off", None);
    off.connect_activate(|_, _| set(false));
    app.add_action(&off);

    let notifier = GioNotifier::new(app, vec![(platform::notification_key(), DISABLE_ACTION.to_owned())]);
    STATE.with(|s| {
        *s.borrow_mut() = Some(State { incognito, downloaded_only, revealer: revealer.clone(), label, button, notifier, switches: vec![] });
    });
    refresh();
    revealer.upcast()
}

/// Binds a More-screen switch to the preference: it shows the stored value and writes changes through [`set`].
pub fn bind_switch(switch: &gtk::Switch) {
    let Some(on) = STATE.with(|s| {
        s.borrow_mut().as_mut().map(|st| {
            st.switches.push(switch.downgrade());
            st.incognito.get()
        })
    }) else {
        return;
    };
    switch.set_active(on);
    switch.connect_active_notify(|sw| set(sw.is_active()));
}

/// Stores `on` and updates the banner, the notification and every bound switch. No-op when unchanged.
pub fn set(on: bool) {
    let Some(st) = STATE.with(|s| s.borrow().clone()) else { return };
    if st.incognito.get() == on {
        return;
    }
    if let Err(e) = st.incognito.set(on) {
        tracing::warn!(error = %e, "could not save incognito mode");
    }
    refresh();
}

fn refresh() {
    let Some(st) = STATE.with(|s| s.borrow().clone()) else { return };
    let on = st.incognito.get();
    let banner = platform::active_banner(false, st.downloaded_only.get(), on);
    if let Some((text, with_button)) = banner.map(banner_view) {
        st.label.set_text(&text);
        st.button.set_visible(with_button);
    }
    st.revealer.set_reveal_child(banner.is_some());
    platform::sync_notification(on, &st.notifier);
    for sw in st.switches.iter().filter_map(|w| w.upgrade()) {
        if sw.is_active() != on {
            sw.set_active(on);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_incognito_banner_has_the_disable_button_and_the_others_do_not() {
        assert_eq!(banner_view(Banner::Incognito), ("Incognito mode".to_owned(), true));
        assert_eq!(banner_view(Banner::DownloadedOnly), ("Downloaded only".to_owned(), false));
    }

    #[test]
    fn the_notification_default_action_is_the_incognito_off_action() {
        assert_eq!(DISABLE_ACTION, "app.incognito-off");
        assert_eq!(platform::notification_key(), "incognito_mode_channel:-701");
    }
}
