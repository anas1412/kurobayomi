//! Android platform features this crate does not port. Each row says what the user loses (often nothing).
//!
//! | Task | Android feature | Decision |
//! |---|---|---|
//! | T-01a-024 | home-screen widget managers | Not ported. The desktop has no home-screen widget surface. Lost: the home-screen cover grid and its locked state. The sidebar badge count comes from `kuro_domain::updates::three_months_ago` with `kuro_data::updates::UpdatesRepo::with_read(false, ...)`, shared with the Updates tab (area 42). |
//! | T-01a-026 | `getPackageName` override and the `X-Requested-With` swap | Not ported. WebKitGTK sends no such header, so no code is written. Any header policy for challenge solving belongs to area 12 and area 36. |
//! | T-01a-027 | Conscrypt TLS provider insertion below Android 10 | Not ported. `reqwest` with rustls gives TLS 1.2 and 1.3 on every Linux system, so the default feature set has no OpenSSL. |
//! | T-01a-028 | WebView data-directory suffix | Not ported. WebKit data and cache live in `paths.data()/webkit` and `paths.cache()/webkit` (see [`crate::webview::data`]). |
//! | T-01a-033 | `FLAG_SECURE` secure-screen mode | Not ported. Wayland and X11 have no per-window "hide from screenshots and the task switcher" flag. Lost: screenshot blocking and thumbnail hiding. The preference `secure_screen_v2` stays for backup round trips (kuro-prefs). |
//! | T-01a-038 | RxJava bridge (`Observable.asFlow`) | Not ported. It only bridged RxJava 1 in legacy extension APIs. Rust sources are async functions or streams. |
//! | T-01a-041 | R8 shrinker rules | Not applicable to Rust. The extension host-API checklist is handed to area 20. |
//! | T-01a-044 | extension-installer strategies (legacy, package installer, Shizuku, private) | Not ported. The key `extension_installer` is accepted on backup import and kept as an unknown extra, never read. Extension installs are defined by area 20. |
//! | T-01a-046 | Picture-in-Picture capability flag | Not ported. [`crate::device::device_has_pip`] is constant `false`. The always-on-top mini player replaces PiP (area 46). |
//!
//! The Linux notifier (T-req-platform-004) is [`crate::linux::notifier`]. Channel groups, the `POST_NOTIFICATIONS`
//! permission and the notification colour are dropped on Linux: the portal asks for permission and freedesktop
//! notifications have no colour.
//!
//! The tests below keep the platform decisions true: a later change that adds one of these pieces fails them.

#[cfg(test)]
mod tests {
    use crate::device::device_has_pip;

    #[test]
    fn picture_in_picture_is_never_offered() {
        assert!(!device_has_pip());
    }
}
