//! Incognito indicator (T-01a-020): the window banner, the desktop notification, the `app.incognito-off` action and the
//! `app.incognito` check item of the app menu. The preference is the one source of truth; every change goes through
//! [`set`], which updates the banner, the notification and the check item (the panel's title icon follows the item) so
//! nothing needs a restart.

use std::cell::RefCell;

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
    /// The app menu's check item, `app.incognito`.
    toggle: gtk::gio::SimpleAction,
}

/// The name of the app menu's check item, an application action with a boolean state.
pub const TOGGLE_NAME: &str = "incognito";
/// The check item as the app menu names it.
pub const TOGGLE_ACTION: &str = "app.incognito";
/// The incognito icon of the banner and of the panel title.
pub const ICON: &str = "dialog-password-symbolic";

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

/// Registers `app.incognito-off` and `app.incognito`, builds the banner and applies the stored value (banner and notification).
/// Call once from the GTK main thread; the returned widget goes at the top of the main window.
pub fn attach(app: &gtk::Application, incognito: Pref<bool>, downloaded_only: Pref<bool>) -> gtk::Widget {
    let label = gtk::Label::builder().xalign(0.0).hexpand(true).build();
    let button = gtk::Button::builder().label(tr(MsgKey::notification_incognito_text)).css_classes(["flat"]).build();
    button.set_action_name(Some(DISABLE_ACTION));
    let bar = gtk::Box::builder().spacing(12).margin_start(12).margin_end(12).margin_top(6).margin_bottom(6).build();
    bar.append(&gtk::Image::from_icon_name(ICON));
    bar.append(&label);
    bar.append(&button);
    bar.add_css_class("toolbar");
    let revealer = gtk::Revealer::builder().child(&bar).transition_type(gtk::RevealerTransitionType::SlideDown).build();

    let off = gtk::gio::SimpleAction::new("incognito-off", None);
    off.connect_activate(|_, _| set(false));
    app.add_action(&off);
    let toggle = gtk::gio::SimpleAction::new_stateful(TOGGLE_NAME, None, &incognito.get().to_variant());
    toggle.connect_change_state(|_, value| {
        if let Some(on) = value.and_then(|v| v.get::<bool>()) {
            set(on);
        }
    });
    app.add_action(&toggle);

    let notifier = GioNotifier::new(app, vec![(platform::notification_key(), DISABLE_ACTION.to_owned())]);
    STATE.with(|s| {
        *s.borrow_mut() = Some(State { incognito, downloaded_only, revealer: revealer.clone(), label, button, notifier, toggle });
    });
    refresh();
    revealer.upcast()
}

/// Stores `on` and updates the banner, the notification and the check item. No-op when unchanged.
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
    st.toggle.set_state(&on.to_variant());
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
    fn the_app_menu_item_names_the_application_action() {
        assert_eq!(TOGGLE_ACTION, format!("app.{TOGGLE_NAME}"));
    }

    #[test]
    fn the_notification_default_action_is_the_incognito_off_action() {
        assert_eq!(DISABLE_ACTION, "app.incognito-off");
        assert_eq!(platform::notification_key(), "incognito_mode_channel:-701");
    }
}
