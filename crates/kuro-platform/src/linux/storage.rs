//! Disk space through `statvfs`, for the filesystem that holds a path.

use std::path::Path;

use rustix::fs::statvfs;

/// Bytes available to unprivileged users on the filesystem holding `path`.
/// `None` when the path is missing or `statvfs` fails.
pub fn free_bytes(path: &Path) -> Option<u64> {
    let s = statvfs(path).ok()?;
    Some(s.f_bavail * s.f_frsize)
}

/// Size of the filesystem holding `path`. `None` when the path is missing or `statvfs` fails.
pub fn total_bytes(path: &Path) -> Option<u64> {
    let s = statvfs(path).ok()?;
    Some(s.f_blocks * s.f_frsize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_path_has_no_space_figures() {
        let tmp = tempfile::tempdir().unwrap();
        let missing = tmp.path().join("absent");
        assert_eq!(free_bytes(&missing), None);
        assert_eq!(total_bytes(&missing), None);
    }

    #[test]
    fn existing_directory_reports_total_and_free_space() {
        let tmp = tempfile::tempdir().unwrap();
        let total = total_bytes(tmp.path()).unwrap();
        assert!(total > 0);
        assert!(free_bytes(tmp.path()).unwrap() <= total);
    }
}
