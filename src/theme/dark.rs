//! System dark detection: the Settings portal `color-scheme` value, with the GTK setting as fallback.
//!
//! The decision (`system_dark`, `gtk_fallback_dark`) is pure. The portal calls run as GIO futures
//! on the main context, so they never block the GTK thread (pitfall 7 in the design).

use gtk::gio::{self, prelude::*};
use gtk::glib;

const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const SETTINGS_IFACE: &str = "org.freedesktop.portal.Settings";
const NAMESPACE: &str = "org.freedesktop.appearance";
const KEY: &str = "color-scheme";
/// A portal call that takes longer than this is treated as unavailable.
const PORTAL_TIMEOUT_MS: i32 = 2000;

/// `color-scheme`: `1` prefers dark, `2` prefers light. `0` and any other value mean no preference
/// (F-49-04), which returns `None`.
pub fn parse_color_scheme(value: u32) -> Option<bool> {
    match value {
        1 => Some(true),
        2 => Some(false),
        _ => None,
    }
}

/// The system dark flag: the portal preference when it has one, else the GTK fallback.
pub fn system_dark(portal: Option<bool>, gtk_fallback: bool) -> bool {
    portal.unwrap_or(gtk_fallback)
}

/// What one portal read or signal produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortalRead {
    /// No portal (no session bus, no service, or no reply in time): use the GTK fallback (F-49-03).
    Missing,
    /// The portal answered with an error or a value that does not parse: keep the last good value
    /// (F-49-04).
    Failed,
    /// A parsed `color-scheme`; `None` is "no preference" and means the GTK fallback.
    Value(Option<bool>),
}

/// The new system dark flag. `last` is the last good value, if any.
pub fn next_system(read: PortalRead, last: Option<bool>, gtk_fallback: bool) -> bool {
    match read {
        PortalRead::Value(portal) => system_dark(portal, gtk_fallback),
        PortalRead::Missing => gtk_fallback,
        PortalRead::Failed => last.unwrap_or(gtk_fallback),
    }
}

/// Sorts a failed portal call into `Missing` (the portal is not there) or `Failed` (it replied
/// with an error). Logs at info for the first and warn for the second.
pub fn classify_error(err: &glib::Error) -> PortalRead {
    use gio::DBusError::{NameHasNoOwner, NoServer, ServiceUnknown, UnknownInterface, UnknownMethod, UnknownObject};
    let absent = matches!(
        err.kind::<gio::DBusError>(),
        Some(ServiceUnknown | NameHasNoOwner | NoServer | UnknownMethod | UnknownObject | UnknownInterface)
    ) || err.matches(gio::IOErrorEnum::TimedOut);
    if absent {
        tracing::info!(error = %err, "settings portal unavailable; using the GTK setting");
        PortalRead::Missing
    } else {
        tracing::warn!(error = %err, "settings portal returned an error; keeping the last colour scheme");
        PortalRead::Failed
    }
}

/// The GTK fallback: `gtk-application-prefer-dark-theme`, or a theme name ending in `-dark`.
pub fn gtk_fallback_dark(prefer_dark: bool, theme_name: &str) -> bool {
    prefer_dark || theme_name.to_ascii_lowercase().ends_with("-dark")
}

/// Reads the GTK settings fallback on the main thread (cheap, no IPC).
pub fn gtk_fallback() -> bool {
    match gtk::Settings::default() {
        Some(settings) => {
            let prefer = settings.is_gtk_application_prefer_dark_theme();
            let name = settings.gtk_theme_name().map(|s| s.to_string()).unwrap_or_default();
            gtk_fallback_dark(prefer, &name)
        }
        None => false,
    }
}

/// Unwraps `v` layers until a `u32` is found. Portals wrap the value in one or two variants.
fn unwrap_u32(mut v: glib::Variant) -> Option<u32> {
    for _ in 0..3 {
        if let Some(n) = v.get::<u32>() {
            return Some(n);
        }
        v = v.as_variant()?;
    }
    None
}

/// A `color-scheme` payload as a `PortalRead`. A payload that is not a `u32` is a failed read.
fn read_value(v: glib::Variant) -> PortalRead {
    match unwrap_u32(v) {
        Some(n) => PortalRead::Value(parse_color_scheme(n)),
        None => {
            tracing::warn!("settings portal sent a colour scheme that is not a u32; keeping the last one");
            PortalRead::Failed
        }
    }
}

/// Reads `color-scheme` once. A missing portal is an info-level `Missing`; an error reply is a
/// warn-level `Failed` (F-49-03, F-49-04).
pub fn read_portal<F: FnOnce(PortalRead) + 'static>(on_done: F) {
    glib::spawn_future_local(async move {
        let read = async {
            let conn = match gio::bus_get_future(gio::BusType::Session).await {
                Ok(conn) => conn,
                Err(e) => return classify_error(&e),
            };
            let params = (NAMESPACE, KEY).to_variant();
            let reply = conn
                .call_future(
                    Some(PORTAL_BUS),
                    PORTAL_PATH,
                    SETTINGS_IFACE,
                    "Read",
                    Some(&params),
                    None,
                    gio::DBusCallFlags::NONE,
                    PORTAL_TIMEOUT_MS,
                )
                .await;
            match reply {
                Ok(reply) => read_value(reply.child_value(0)),
                Err(e) => classify_error(&e),
            }
        };
        on_done(read.await);
    });
}

/// Subscribes to `SettingChanged` for `color-scheme` and calls `on_change` with each new reading.
/// The subscription lives as long as the process; the bus connection is held by the closure.
pub fn watch<F: Fn(PortalRead) + 'static>(on_change: F) {
    glib::spawn_future_local(async move {
        let Ok(conn) = gio::bus_get_future(gio::BusType::Session).await else {
            tracing::info!("no session bus; live dark-mode updates are off");
            return;
        };
        let subscription = conn.subscribe_to_signal(
            Some(PORTAL_BUS),
            Some(SETTINGS_IFACE),
            Some("SettingChanged"),
            Some(PORTAL_PATH),
            None,
            gio::DBusSignalFlags::NONE,
            move |signal| {
                let params = signal.parameters;
                let ns = params.child_value(0).get::<String>();
                let key = params.child_value(1).get::<String>();
                if ns.as_deref() == Some(NAMESPACE) && key.as_deref() == Some(KEY) {
                    on_change(read_value(params.child_value(2)));
                }
            },
        );
        // The watch lasts for the process: keep the subscription and the connection alive.
        std::mem::forget(subscription);
        std::mem::forget(conn);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_scheme_values() {
        assert_eq!(parse_color_scheme(1), Some(true));
        assert_eq!(parse_color_scheme(2), Some(false));
        assert_eq!(parse_color_scheme(0), None);
        assert_eq!(parse_color_scheme(3), None);
    }

    #[test]
    fn portal_wins_over_gtk_fallback() {
        assert!(system_dark(Some(true), false));
        assert!(!system_dark(Some(false), true));
    }

    #[test]
    fn no_preference_uses_gtk_fallback() {
        assert!(system_dark(None, true));
        assert!(!system_dark(None, false));
    }

    #[test]
    fn missing_portal_uses_the_fallback_and_an_error_keeps_the_last_value() {
        assert!(next_system(PortalRead::Missing, Some(false), true));
        assert!(!next_system(PortalRead::Missing, Some(true), false));
        // Error: the last good value wins over the fallback.
        assert!(next_system(PortalRead::Failed, Some(true), false));
        assert!(!next_system(PortalRead::Failed, Some(false), true));
        // Error with no good value yet: the fallback.
        assert!(next_system(PortalRead::Failed, None, true));
        // A parsed value, or no preference.
        assert!(next_system(PortalRead::Value(Some(true)), Some(false), false));
        assert!(next_system(PortalRead::Value(None), Some(false), true));
    }

    #[test]
    fn dbus_absence_is_missing_and_other_errors_fail() {
        for kind in [
            gio::DBusError::ServiceUnknown,
            gio::DBusError::NameHasNoOwner,
            gio::DBusError::NoServer,
            gio::DBusError::UnknownMethod,
            gio::DBusError::UnknownInterface,
        ] {
            assert_eq!(classify_error(&glib::Error::new(kind, "x")), PortalRead::Missing, "{kind:?}");
        }
        assert_eq!(
            classify_error(&glib::Error::new(gio::IOErrorEnum::TimedOut, "slow")),
            PortalRead::Missing
        );
        assert_eq!(
            classify_error(&glib::Error::new(gio::DBusError::AccessDenied, "no")),
            PortalRead::Failed
        );
        assert_eq!(
            classify_error(&glib::Error::new(gio::DBusError::Failed, "boom")),
            PortalRead::Failed
        );
    }

    #[test]
    fn dark_theme_name_suffix_counts_as_dark() {
        assert!(gtk_fallback_dark(false, "Breeze-Dark"));
        assert!(gtk_fallback_dark(true, "Breeze"));
        assert!(!gtk_fallback_dark(false, "Breeze"));
    }
}
