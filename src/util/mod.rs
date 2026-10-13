//! Small helpers behind the util layer of the Android app (area 52) and the platform glue of area 53.
//!
//! Each module is pure rules with unit tests; the GTK calls are thin wrappers at the bottom of a file. The callers (the
//! player, the settings pages and the lock window) live in other lanes.

// the player, the downloads and the lock window call these from their own lanes
pub mod dropped;
pub mod logging;
pub mod metered;
