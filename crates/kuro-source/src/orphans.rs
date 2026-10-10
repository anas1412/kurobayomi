//! Orphaned download folders at start (T-17a-048). A download folder whose name matches no current source is logged at
//! info level with its name. Nothing is deleted: the clean-up action is a P2 feature (area 25, Q3).

use std::io;
use std::path::Path;

/// The names of the folders directly under `root` that match none of `source_names`, sorted. Plain files are skipped,
/// and a missing `root` has no folders at all.
pub fn orphaned_download_folders(root: &Path, source_names: &[&str]) -> io::Result<Vec<String>> {
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err),
    };
    let mut orphans = Vec::new();
    for entry in entries {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if !source_names.contains(&name.as_str()) {
            orphans.push(name);
        }
    }
    orphans.sort();
    Ok(orphans)
}

/// Logs each orphaned folder at info level and returns the names. Nothing is deleted.
pub fn log_orphaned_download_folders(root: &Path, source_names: &[&str]) -> io::Result<Vec<String>> {
    let orphans = orphaned_download_folders(root, source_names)?;
    for name in &orphans {
        tracing::info!("download folder {name:?} matches no installed source; kept");
    }
    Ok(orphans)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn folders_matching_no_source_are_listed_and_nothing_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        for name in ["Source A", "Gone Source", "Another Gone"] {
            fs::create_dir(dir.path().join(name)).unwrap();
        }
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();

        let orphans = log_orphaned_download_folders(dir.path(), &["Source A"]).unwrap();

        assert_eq!(orphans, vec!["Another Gone".to_string(), "Gone Source".to_string()]);
        assert!(dir.path().join("Gone Source").is_dir(), "orphans are kept");
        assert!(dir.path().join("notes.txt").is_file());
    }

    #[test]
    fn a_missing_download_root_has_no_orphans() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("absent");
        assert_eq!(orphaned_download_folders(&missing, &[]).unwrap(), Vec::<String>::new());
    }
}
