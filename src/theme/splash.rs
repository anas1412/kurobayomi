//! Splash page with ready gating (T-49ba-020, `MainActivity.kt` splash keep-on-screen rule).
//!
//! Linux has no launch window, so the splash is a page of the main stack. It stays for at least
//! 500 ms, at most 5000 ms, and until the first main page reports ready. Then it crossfades out over
//! 400 ms. The gate is a pure function; `run` drives it on the GTK main loop.

use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;

use super::anim;
use super::icons::Icon;
use super::motion;

/// The name of the splash page in the main stack.
pub const PAGE_NAME: &str = "splash";
/// The minimum time the splash stays on screen (`SPLASH_MIN_DURATION`).
pub const MIN_MS: u64 = 500;
/// The longest the splash waits for the main page (`SPLASH_MAX_DURATION`).
pub const MAX_MS: u64 = 5000;
/// The logo size on the splash page.
pub const LOGO_PX: i32 = 108;
/// How often the gate is checked on the main loop.
const POLL: Duration = Duration::from_millis(16);

/// True while the splash must stay on screen. `elapsed_ms` counts from when the splash appeared.
pub const fn keep_visible(elapsed_ms: u64, ready: bool) -> bool {
    elapsed_ms <= MIN_MS || (!ready && elapsed_ms <= MAX_MS)
}

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

/// Shows the splash page in `stack` and hides it once the gate allows. `is_ready` reports whether the
/// main page is ready. Call once at startup: there is no splash on later activations.
pub fn run(stack: &gtk::Stack, main_page: &'static str, is_ready: impl Fn() -> bool + 'static) {
    let stack = stack.clone();
    let started = Instant::now();
    glib::timeout_add_local(POLL, move || {
        let elapsed = started.elapsed().as_millis() as u64;
        if keep_visible(elapsed, is_ready()) {
            return glib::ControlFlow::Continue;
        }
        anim::crossfade_to(&stack, main_page, motion::SPLASH_EXIT_MS);
        glib::ControlFlow::Break
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splash_hides_at_500_ms_when_the_main_page_is_ready() {
        assert!(keep_visible(0, true));
        assert!(keep_visible(500, true));
        assert!(!keep_visible(501, true));
    }

    #[test]
    fn splash_waits_for_the_main_page_up_to_5000_ms() {
        assert!(keep_visible(4_999, false));
        assert!(keep_visible(5_000, false));
        assert!(!keep_visible(5_001, false));
    }

    #[test]
    fn the_splash_exit_is_400_ms() {
        assert_eq!(motion::SPLASH_EXIT_MS, 400);
        assert_eq!(LOGO_PX, 108);
        assert_eq!(PAGE_NAME, "splash");
    }
}
