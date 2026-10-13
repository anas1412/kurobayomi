//! Splash page with ready gating (T-49ba-020, `MainActivity.kt` splash keep-on-screen rule).
//!
//! Linux has no launch window, so the splash is a page of the main stack. It stays for at least
//! 500 ms, at most 5000 ms, and until the first main page reports ready. Then it crossfades out over
//! 400 ms. The gate is a pure function; `run` drives it on the GTK main loop.


