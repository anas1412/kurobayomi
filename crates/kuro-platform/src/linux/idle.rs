//! The screen-saver inhibit of `org.freedesktop.ScreenSaver`, which GNOME and KDE both serve on the session bus.

use std::io;

use zbus::blocking::{Connection, Proxy};

use crate::idle::{IdleGuard, IdleInhibitor};

const SERVICE: &str = "org.freedesktop.ScreenSaver";
const PATH: &str = "/org/freedesktop/ScreenSaver";
const APP_NAME: &str = "Kurobayomi";

/// Inhibits through the session bus. Each inhibit opens its own connection, which is cheap next to a playback session.
#[derive(Debug, Default, Clone, Copy)]
pub struct ScreenSaver;

impl IdleInhibitor for ScreenSaver {
    fn inhibit(&self, reason: &str) -> io::Result<IdleGuard> {
        let connection = Connection::session().map_err(io::Error::other)?;
        let proxy = Proxy::new(&connection, SERVICE, PATH, SERVICE).map_err(io::Error::other)?;
        let cookie: u32 = proxy.call("Inhibit", &(APP_NAME, reason)).map_err(io::Error::other)?;
        Ok(IdleGuard::new(move || {
            // The guard cannot report a failed release, and the desktop drops the inhibit when the session ends.
            let _ = proxy.call::<_, _, ()>("UnInhibit", &cookie);
        }))
    }
}
