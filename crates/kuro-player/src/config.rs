//! Player config files (area 45 `PlayerActivity.setupPlayerMPV`, T-45a-026). `mpv.conf` and `input.conf` are the user's
//! own files in the mpv config folder: the app creates them when they are missing and never rewrites them, so an edit
//! made by hand (or in the Script editor) survives every start. Scripts, script options and shaders stay where the user
//! put them; mpv reads them from that folder.

use std::fs;
use std::io;
use std::path::Path;

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

/// Creates `mpv.conf` and `input.conf` in `mpv_dir` when they are missing (the folder too). An existing file is left
/// exactly as it is.
pub fn ensure_mpv_config(mpv_dir: &Path) -> io::Result<()> {
    fs::create_dir_all(mpv_dir)?;
    for name in ["mpv.conf", "input.conf"] {
        match fs::OpenOptions::new().write(true).create_new(true).open(mpv_dir.join(name)) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_config_files_are_created_empty_and_existing_ones_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        let mpv = dir.path().join("mpv");
        ensure_mpv_config(&mpv).unwrap();
        assert_eq!(fs::read_to_string(mpv.join("mpv.conf")).unwrap(), "");
        assert_eq!(fs::read_to_string(mpv.join("input.conf")).unwrap(), "");
        fs::write(mpv.join("mpv.conf"), "profile=fast\nvo=gpu\n").unwrap();
        ensure_mpv_config(&mpv).unwrap();
        assert_eq!(fs::read_to_string(mpv.join("mpv.conf")).unwrap(), "profile=fast\nvo=gpu\n");
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
}
