//! Temporary frame outputs for `ffmpeg` (area:14 section 14, T-14a-049).
//!
//! A frame is written to `tmp_<random><suffix>` in the app temp directory. The file is removed when the guard drops, so
//! a finished or failed extraction leaves nothing behind (E-ANIME-7). The `ffmpeg` runner (T-14a-026) owns the child
//! process and holds this guard for as long as the child runs.

use std::io;
use std::path::Path;

/// A temporary file named `tmp_*<suffix>` in `dir`, removed when dropped.
pub fn temp_output(dir: &Path, suffix: &str) -> io::Result<tempfile::NamedTempFile> {
    tempfile::Builder::new()
        .prefix("tmp_")
        .suffix(suffix)
        .tempfile_in(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_file_has_the_prefix_and_suffix_and_goes_away_on_drop() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let path = {
            let guard = temp_output(dir.path(), "-thumb.jpg")?;
            let name = guard
                .path()
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            assert!(
                name.starts_with("tmp_") && name.ends_with("-thumb.jpg"),
                "{name}"
            );
            guard.path().to_path_buf()
        };
        assert!(!path.exists());
        assert_eq!(std::fs::read_dir(dir.path())?.count(), 0);
        Ok(())
    }

    #[test]
    fn a_missing_temp_folder_is_an_error_and_leaves_nothing() -> io::Result<()> {
        let dir = tempfile::tempdir()?;
        let gone = dir.path().join("missing");
        assert!(temp_output(&gone, ".jpg").is_err());
        assert_eq!(std::fs::read_dir(dir.path())?.count(), 0);
        Ok(())
    }
}
