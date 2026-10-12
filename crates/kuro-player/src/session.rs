//! The mpv config folder step that runs before mpv is created.

use crate::config::ensure_mpv_config;
use crate::options::PlayerPrefs;

/// Step before mpv is created: the config files are created when they are missing and never rewritten. A failure is
/// logged and does not stop playback.
pub fn prepare_mpv_dir(prefs: &PlayerPrefs) {
    let dir = prefs.data_dir.join("mpv");
    if let Err(e) = ensure_mpv_config(&dir) {
        tracing::error!("failed to create the mpv config files: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepare_creates_the_config_files_and_keeps_the_users_text() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = PlayerPrefs {
            data_dir: dir.path().join("data"),
            ..PlayerPrefs::default()
        };
        prepare_mpv_dir(&prefs);
        let mpv = dir.path().join("data/mpv");
        std::fs::write(mpv.join("mpv.conf"), "volume=40\n").unwrap();
        prepare_mpv_dir(&prefs);
        assert_eq!(std::fs::read_to_string(mpv.join("mpv.conf")).unwrap(), "volume=40\n");
        assert!(mpv.join("input.conf").is_file());
    }
}
