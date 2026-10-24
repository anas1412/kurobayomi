//! Free disk space, for the filesystem that holds a path.
//!
//! The engine is per platform: `statvfs` on Linux (`linux::storage`), `GetDiskFreeSpaceExW` on Windows
//! (`windows::storage`). Elsewhere there is none yet, so [`free_bytes`] gives `None`, which callers treat as "unknown"
//! and skip their space checks.

use std::path::Path;

/// Bytes available to unprivileged users on the filesystem holding `path`. `None` when the path is missing, the call
/// fails or this platform has no engine.
#[allow(unused_variables, unreachable_code)]
pub fn free_bytes(path: &Path) -> Option<u64> {
    #[cfg(all(target_os = "linux", feature = "platform-linux"))]
    return crate::linux::storage::free_bytes(path);
    #[cfg(all(target_os = "windows", feature = "platform-windows"))]
    return crate::windows::storage::free_bytes(path);
    None
}
