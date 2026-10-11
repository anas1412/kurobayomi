//! Splash page with ready gating (T-49ba-020, `MainActivity.kt` splash keep-on-screen rule).
//!
//! Linux has no launch window, so the splash is a page of the main stack. It stays for at least
//! 500 ms, at most 5000 ms, and until the first main page reports ready. Then it crossfades out over
//! 400 ms. The gate is a pure function; `run` drives it on the GTK main loop.

use gtk::prelude::*;

use super::icons::Icon;

/// The logo size on the splash page.
pub const LOGO_PX: i32 = 108;

/// The splash page: the logo on the `kb-splash` background, `kb-dark` for the dark palette.
pub fn page(dark: bool) -> gtk::Box {
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.add_css_class("kb-splash");
    if dark {
        page.add_css_class("kb-dark");
    }
    page.set_halign(gtk::Align::Fill);
    page.set_valign(gtk::Align::Fill);
    page.set_hexpand(true);
    page.set_vexpand(true);
    let logo = gtk::Image::from_icon_name(Icon::Logo.name());
    logo.set_pixel_size(LOGO_PX);
    logo.set_halign(gtk::Align::Center);
    logo.set_valign(gtk::Align::Center);
    logo.set_hexpand(true);
    logo.set_vexpand(true);
    page.append(&logo);
    page
}
