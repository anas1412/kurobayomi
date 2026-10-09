//! Player settings and mpv config files (area 45 `PlayerActivity.setupPlayerMPV`, T-45a-026).
//! `mpv.conf` and `input.conf` are rewritten from the advanced preferences every time a player
//! window opens. Writes truncate, so an empty preference leaves an empty file.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::mpv::{MpvError, PropertySink};

/// Points mpv's subtitle and OSD font lookup at the user fonts directory (T-45a-029). mpv reads the
/// directory from disk, so it is used in place and nothing is copied. A missing directory sets
/// nothing.
pub fn apply_fonts_dir(sink: &impl PropertySink, fonts_dir: &Path) -> Result<(), MpvError> {
    if !fonts_dir.is_dir() {
        return Ok(());
    }
    let path = fonts_dir.to_string_lossy();
    sink.set_str("sub-fonts-dir", &path)?;
    sink.set_str("osd-fonts-dir", &path)
}

/// Advanced preferences (`AdvancedPlayerPreferences`, T-45b-034). The text fields are written to
/// the mpv config files every time a player window opens.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AdvancedPrefs {
    /// `mpv_scripts`: copy the user's scripts, script options and shaders into the mpv directory.
    pub user_files: bool,
    /// Where the user keeps `scripts`, `script-opts` and `shaders` (copied when `user_files`).
    pub source_dir: PathBuf,
    /// `pref_mpv_conf`: text of `mpv.conf`.
    pub mpv_conf: String,
    /// `pref_mpv_input`: text of `input.conf`.
    pub mpv_input: String,
    /// `pref_player_statistics_page`: the statistics page last shown (0..5).
    pub statistics_page: i32,
}

/// Which mpv config file a text belongs to. Each has its own line rules.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfFile {
    Mpv,
    Input,
}

/// One-based numbers of the lines mpv will not read. Blank lines and `#` comments are fine. An
/// `mpv.conf` line is `option=value` or a bare flag, so its key must be a single word. An
/// `input.conf` line is `KEY command`, so it needs both parts. These lines are logged, not fatal.
pub fn invalid_lines(file: ConfFile, text: &str) -> Vec<usize> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                return false;
            }
            match file {
                ConfFile::Mpv => {
                    let key = line.split_once('=').map_or(line, |(key, _)| key).trim();
                    key.is_empty() || key.split_whitespace().count() != 1
                }
                ConfFile::Input => line.split_whitespace().count() < 2,
            }
        })
        .map(|(index, _)| index + 1)
        .collect()
}

/// Rewrites `<mpv_dir>/mpv.conf` and `<mpv_dir>/input.conf`, creating the directory if needed.
/// Lines mpv cannot read are logged at warn level.
pub fn write_mpv_config(mpv_dir: &Path, mpv_conf: &str, input_conf: &str) -> io::Result<()> {
    fs::create_dir_all(mpv_dir)?;
    for (file, text, name) in [
        (ConfFile::Mpv, mpv_conf, "mpv.conf"),
        (ConfFile::Input, input_conf, "input.conf"),
    ] {
        for line in invalid_lines(file, text) {
            tracing::warn!(file = name, line, "mpv config line is not valid");
        }
    }
    fs::write(mpv_dir.join("mpv.conf"), mpv_conf)?;
    fs::write(mpv_dir.join("input.conf"), input_conf)
}

/// The directories under the mpv config dir that user files go to.
pub const USER_DIRS: [&str; 3] = ["scripts", "script-opts", "shaders"];

/// Clears the user script directories in `mpv_dir`, then copies the files of `source` into them
/// when `enabled` (`mpv_scripts`). Each `source/<dir>` is copied flat, one level deep, as the Kotlin
/// copies `listFiles()`. A missing source directory copies nothing.
pub fn apply_user_files(mpv_dir: &Path, source: &Path, enabled: bool) -> io::Result<()> {
    for name in USER_DIRS {
        match fs::remove_dir_all(mpv_dir.join(name)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    if !enabled {
        return Ok(());
    }
    for name in USER_DIRS {
        let from = source.join(name);
        if !from.is_dir() {
            continue;
        }
        let to = mpv_dir.join(name);
        fs::create_dir_all(&to)?;
        for entry in fs::read_dir(&from)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::copy(entry.path(), to.join(entry.file_name()))?;
            }
        }
    }
    Ok(())
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

    #[derive(Default)]
    struct Recorder(std::cell::RefCell<Vec<(String, String)>>);

    impl PropertySink for Recorder {
        fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
            self.0
                .borrow_mut()
                .push((name.to_owned(), value.to_owned()));
            Ok(())
        }
    }

    #[test]
    fn fonts_dir_is_used_in_place_for_both_properties() {
        let dir = tempfile::tempdir().unwrap();
        let fonts = dir.path().join("fonts");
        fs::create_dir(&fonts).unwrap();
        let rec = Recorder::default();
        apply_fonts_dir(&rec, &fonts).unwrap();
        let path = fonts.to_string_lossy().into_owned();
        assert_eq!(
            *rec.0.borrow(),
            vec![
                ("sub-fonts-dir".to_owned(), path.clone()),
                ("osd-fonts-dir".to_owned(), path),
            ]
        );
    }

    #[test]
    fn missing_fonts_dir_sets_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let rec = Recorder::default();
        apply_fonts_dir(&rec, &dir.path().join("absent")).unwrap();
        assert!(rec.0.borrow().is_empty());
    }

    #[test]
    fn mpv_conf_lines_need_a_single_word_key() {
        let text = "# comment\n\nvolume=40\nno-osc\n=bad\nlong key=1\n";
        assert_eq!(invalid_lines(ConfFile::Mpv, text), vec![5, 6]);
    }

    #[test]
    fn input_conf_lines_need_a_key_and_a_command() {
        let text = "SPACE cycle pause\nLEFT\n# note\n";
        assert_eq!(invalid_lines(ConfFile::Input, text), vec![2]);
    }

    #[test]
    fn user_files_are_copied_only_when_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        fs::create_dir_all(source.join("scripts")).unwrap();
        fs::write(source.join("scripts/skip.lua"), b"lua").unwrap();
        let mpv = dir.path().join("mpv");
        apply_user_files(&mpv, &source, true).unwrap();
        assert_eq!(fs::read(mpv.join("scripts/skip.lua")).unwrap(), b"lua");
        assert!(!mpv.join("shaders").exists());
        apply_user_files(&mpv, &source, false).unwrap();
        assert!(!mpv.join("scripts").exists());
    }

    #[test]
    fn advanced_prefs_default_to_off_and_empty() {
        let prefs = AdvancedPrefs::default();
        assert!(!prefs.user_files);
        assert_eq!(prefs.mpv_conf, "");
        assert_eq!(prefs.mpv_input, "");
        assert_eq!(prefs.statistics_page, 0);
    }
}
