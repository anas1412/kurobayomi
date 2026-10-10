//! Small helpers behind the util layer of the Android app (area 52) and the platform glue of area 53.
//!
//! Each module is pure rules with unit tests; the GTK calls are thin wrappers at the bottom of a file. The callers (the
//! player, the settings pages and the lock window) live in other lanes.

// the player, the downloads and the lock window call these from their own lanes
#[allow(dead_code)]
pub mod app_lock_auth;
#[allow(dead_code)]
pub mod build_flags;
#[allow(dead_code)]
pub mod collections;
#[allow(dead_code)]
pub mod dropped;
#[allow(dead_code)]
pub mod keep_awake;
#[allow(dead_code)]
pub mod logging;
#[allow(dead_code)]
pub mod metered;
