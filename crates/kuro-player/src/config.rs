//! Player settings and mpv config files (area 45 `PlayerActivity.setupPlayerMPV`, T-45a-026).
//! `mpv.conf` and `input.conf` are rewritten from the advanced preferences every time a player
//! window opens. Writes truncate, so an empty preference leaves an empty file.

use std::fs;
use std::io;
use std::path::Path;

/// Rewrites `<mpv_dir>/mpv.conf` and `<mpv_dir>/input.conf`, creating the directory if needed.
pub fn write_mpv_config(mpv_dir: &Path, mpv_conf: &str, input_conf: &str) -> io::Result<()> {
    fs::create_dir_all(mpv_dir)?;
    fs::write(mpv_dir.join("mpv.conf"), mpv_conf)?;
    fs::write(mpv_dir.join("input.conf"), input_conf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_both_files_and_truncates_on_rewrite() {
        let dir = tempfile::tempdir().unwrap();
        let mpv = dir.path().join("mpv");
        write_mpv_config(&mpv, "profile=fast\nvo=gpu\n", "SPACE cycle pause\n").unwrap();
        assert_eq!(
            fs::read_to_string(mpv.join("mpv.conf")).unwrap(),
            "profile=fast\nvo=gpu\n"
        );
        write_mpv_config(&mpv, "x", "").unwrap();
        assert_eq!(fs::read_to_string(mpv.join("mpv.conf")).unwrap(), "x");
        assert_eq!(fs::read_to_string(mpv.join("input.conf")).unwrap(), "");
    }
}
