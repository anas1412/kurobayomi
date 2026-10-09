//! Screenshots (area 45 `PlayerViewModel` screenshot functions, T-45a-049 and T-45b-039). A capture
//! goes to a timestamped temp file in the cache and is renamed to its own final name. Save, share
//! and set-as-art build their names from the title, the episode and the position. Pure name rules
//! sit next to the file operations; nothing here touches mpv or GTK.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::enums::{ArtType, SetAsArt};
use crate::errors::PlayerMessage;

/// Longest file name the Kotlin app builds, in UTF-8 bytes (`DiskUtil.MAX_FILE_NAME_BYTES`).
pub const MAX_FILE_NAME_BYTES: usize = 250;
/// What a capture is named in the cache after its timestamp: `<epoch-ms>_mpv_screenshot.png`.
pub const CAPTURE_NAME: &str = "mpv_screenshot.png";
/// Ending of a temp capture name: `<epoch-ms>_shot.png`.
const TEMP_SUFFIX: &str = "_shot.png";

/// Result of a screenshot (area 45 `PlayerActivity.onSaveImageResult`, T-45a-049). Success shows the
/// `Picture saved` toast. Failure is logged at error level and shows nothing.
pub fn screenshot_result(result: Result<(), &str>) -> Option<PlayerMessage> {
    match result {
        Ok(()) => Some(PlayerMessage::PictureSaved),
        Err(error) => {
            tracing::error!(error, "screenshot failed");
            None
        }
    }
}

/// Path of the temp file for a capture started at `millis` (`<millis>_shot.png`). Every call in
/// the process gets its own millisecond: a second call in the same one is moved to the next, so
/// two captures never share a path (E83).
pub fn temp_path(cache_dir: &Path, millis: u128) -> PathBuf {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let wanted = u64::try_from(millis).unwrap_or(u64::MAX);
    let mut used = wanted;
    let _ = LAST.try_update(Ordering::SeqCst, Ordering::SeqCst, |last| {
        used = wanted.max(last.saturating_add(1));
        Some(used)
    });
    cache_dir.join(format!("{used}{TEMP_SUFFIX}"))
}

/// The mpv command that writes a capture. The subtitle flag includes subtitles in the image.
pub fn capture_command(temp: &Path, show_subtitles: bool) -> Vec<String> {
    vec![
        "screenshot-to-file".to_owned(),
        temp.to_string_lossy().into_owned(),
        if show_subtitles { "subtitles" } else { "video" }.to_owned(),
    ]
}

/// Renames a finished temp capture (`<ms>_shot.png`) to `<ms>_mpv_screenshot.png` in the same
/// directory. The final name is unique because the temp name is, so Share and Save never race on
/// one file. Errors are returned, and the temp file is removed when the rename fails (E-52). A
/// capture that mpv did not write is `NotFound`.
pub fn finish_capture(cache_dir: &Path, temp: &Path) -> io::Result<PathBuf> {
    if !temp.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "the screenshot was not written",
        ));
    }
    let name = temp
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "bad capture name"))?;
    let stem = name.strip_suffix(TEMP_SUFFIX).unwrap_or(name);
    let target = cache_dir.join(format!("{stem}_{CAPTURE_NAME}"));
    match fs::rename(temp, &target) {
        Ok(()) => Ok(target),
        Err(error) => {
            let _ = fs::remove_file(temp);
            Err(error)
        }
    }
}

/// Cuts a string to at most `max` UTF-8 bytes, on a character boundary.
fn take_bytes(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// `DiskUtil.buildValidFilename`: trims dots and spaces, replaces control characters and the FAT
/// reserved characters with `_`, and keeps at most 240 characters. An empty name is `(invalid)`.
pub fn valid_filename(name: &str) -> String {
    let trimmed = name.trim_matches(|c| c == '.' || c == ' ');
    if trimmed.is_empty() {
        return "(invalid)".to_owned();
    }
    trimmed
        .chars()
        .map(|c| match c {
            '\u{0}'..='\u{1f}'
            | '"'
            | '*'
            | '/'
            | ':'
            | '<'
            | '>'
            | '?'
            | '\\'
            | '|'
            | '\u{7f}' => '_',
            other => other,
        })
        .take(240)
        .collect()
}

/// The name of a saved image: `<title> - <episode> - <position>`. The title and episode are cut so
/// the whole name stays within `MAX_FILE_NAME_BYTES`. Only the title and episode are validated, so
/// the position keeps its colons.
pub fn image_filename(title: &str, episode: &str, time_pos: &str) -> String {
    let suffix = format!(" - {time_pos}");
    let base = format!("{title} - {episode}");
    let budget = MAX_FILE_NAME_BYTES.saturating_sub(suffix.len());
    format!("{}{suffix}", valid_filename(take_bytes(&base, budget)))
}

/// A path in `dir` for `stem.ext` that does not exist yet. A taken name gets ` (n)` appended with
/// the first free `n` from 1, so two saves in the same second do not overwrite each other (E-51).
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (1..)
        .map(|n| dir.join(format!("{stem} ({n}).{ext}")))
        .find(|candidate| !candidate.exists())
        .unwrap_or(first)
}

/// Saves a PNG to `<pictures>/<valid title>/<name>.png` and returns the path written.
pub fn save_image(pictures: &Path, title: &str, name: &str, png: &[u8]) -> io::Result<PathBuf> {
    let dir = pictures.join(valid_filename(title));
    fs::create_dir_all(&dir)?;
    let path = unique_path(&dir, name, "png");
    fs::write(&path, png)?;
    Ok(path)
}

/// Writes a PNG for sharing. The cache image directory is wiped first, so only the last shared
/// image stays on disk.
pub fn share_image(cache_image_dir: &Path, name: &str, png: &[u8]) -> io::Result<PathBuf> {
    match fs::remove_dir_all(cache_image_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    fs::create_dir_all(cache_image_dir)?;
    let path = unique_path(cache_image_dir, name, "png");
    fs::write(&path, png)?;
    Ok(path)
}

/// Result of a set-as-art action. The art is written first; the result says whether it was kept.
/// Art for an entry that is neither local nor in the library asks the user to add it first.
pub fn set_as_art_result(written: Result<(), &str>, local_or_favorite: bool) -> SetAsArt {
    match written {
        Err(error) => {
            tracing::error!(error, "set as art failed");
            SetAsArt::Error
        }
        Ok(()) if local_or_favorite => SetAsArt::Success,
        Ok(()) => SetAsArt::AddToLibraryFirst,
    }
}

/// Where set-as-art writes (`anime.editCover`, `anime.editBackground`, `episode.editThumbnail`).
/// The app layer implements it over the cover, background and thumbnail stores.
pub trait ArtStore {
    fn edit_cover(&self, png: &[u8]) -> Result<(), String>;
    fn edit_background(&self, png: &[u8]) -> Result<(), String>;
    fn edit_thumbnail(&self, png: &[u8]) -> Result<(), String>;
}

/// Writes the PNG to the slot of `art` (behaviour 14.5) and classifies the outcome.
pub fn set_as_art(
    store: &impl ArtStore,
    art: ArtType,
    png: &[u8],
    local_or_favorite: bool,
) -> SetAsArt {
    let written = match art {
        ArtType::Cover => store.edit_cover(png),
        ArtType::Background => store.edit_background(png),
        ArtType::Thumbnail => store.edit_thumbnail(png),
    };
    set_as_art_result(
        written.as_ref().map(|_| ()).map_err(String::as_str),
        local_or_favorite,
    )
}

/// Tiles of the screenshot sheet, in order (`ScreenshotSheet.kt`, T-46b-021).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScreenshotTile {
    SetCover,
    SetBackground,
    SetThumbnail,
    Share,
    Save,
}

impl ScreenshotTile {
    pub const fn label(self) -> &'static str {
        match self {
            Self::SetCover => "Set as cover",
            Self::SetBackground => "Set as background",
            Self::SetThumbnail => "Set as thumbnail",
            Self::Share => "Share",
            Self::Save => "Save",
        }
    }

    /// The artwork slot a set-as tile writes; `None` for Share and Save.
    pub const fn art(self) -> Option<ArtType> {
        match self {
            Self::SetCover => Some(ArtType::Cover),
            Self::SetBackground => Some(ArtType::Background),
            Self::SetThumbnail => Some(ArtType::Thumbnail),
            Self::Share | Self::Save => None,
        }
    }
}

/// The tiles shown. Set as thumbnail appears only for a local source.
pub fn tiles(is_local_source: bool) -> Vec<ScreenshotTile> {
    let mut tiles = vec![ScreenshotTile::SetCover, ScreenshotTile::SetBackground];
    if is_local_source {
        tiles.push(ScreenshotTile::SetThumbnail);
    }
    tiles.extend([ScreenshotTile::Share, ScreenshotTile::Save]);
    tiles
}

/// Confirm text for all three set-as actions (E86 parity).
pub const SET_ART_CONFIRM: &str = "Use this image as cover art?";

/// The Include Subtitles switch is shown only when the video has subtitle tracks.
pub fn subtitle_switch_visible(has_subtitle_tracks: bool) -> bool {
    has_subtitle_tracks
}

/// Whether the next capture includes subtitles: the switch is on and subtitle tracks exist.
pub fn include_subtitles(switch_on: bool, has_subtitle_tracks: bool) -> bool {
    switch_on && has_subtitle_tracks
}

/// Default name for Save: `<title>_<seconds>.png`, with the title validated for file names.
pub fn save_default_name(title: &str, seconds: i64) -> String {
    format!("{}.png", valid_filename(&format!("{title}_{seconds}")))
}

/// A capture that produced no file shows `Screenshot failed` instead of crashing (E82).
pub fn sheet_capture_message(result: &io::Result<PathBuf>) -> Option<PlayerMessage> {
    result
        .as_ref()
        .err()
        .map(|_| PlayerMessage::ScreenshotFailed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_shows_the_picture_saved_toast() {
        assert_eq!(screenshot_result(Ok(())), Some(PlayerMessage::PictureSaved));
    }

    #[test]
    fn failure_shows_nothing() {
        assert_eq!(screenshot_result(Err("disk full")), None);
    }

    #[test]
    fn capture_command_picks_the_subtitle_flag() {
        let temp = Path::new("/cache/1_shot.png");
        assert_eq!(
            capture_command(temp, true),
            vec!["screenshot-to-file", "/cache/1_shot.png", "subtitles"]
        );
        assert_eq!(capture_command(temp, false)[2], "video");
    }

    #[test]
    fn two_captures_get_different_temp_and_final_paths() {
        let dir = tempfile::tempdir().unwrap();
        let (first, second) = (temp_path(dir.path(), 42), temp_path(dir.path(), 42));
        assert_ne!(first, second);
        assert!(first.to_string_lossy().ends_with("_shot.png"));
        fs::write(&first, b"one").unwrap();
        fs::write(&second, b"two").unwrap();
        let out_first = finish_capture(dir.path(), &first).unwrap();
        let out_second = finish_capture(dir.path(), &second).unwrap();
        assert_ne!(out_first, out_second);
        assert!(out_first.to_string_lossy().ends_with("_mpv_screenshot.png"));
        assert_eq!(fs::read(&out_first).unwrap(), b"one");
        assert_eq!(fs::read(&out_second).unwrap(), b"two");
        assert!(!first.exists() && !second.exists());
    }

    #[test]
    fn missing_capture_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let temp = temp_path(dir.path(), 7);
        let error = finish_capture(dir.path(), &temp).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn image_name_keeps_colons_in_the_position_and_strips_bad_characters() {
        assert_eq!(
            image_filename("Show: Part/2", "Ep 1", "01:02"),
            "Show_ Part_2 - Ep 1 - 01:02"
        );
        assert_eq!(valid_filename(" .. "), "(invalid)");
    }

    #[test]
    fn image_name_is_capped_at_250_bytes() {
        let long_title = "é".repeat(200);
        let name = image_filename(&long_title, "Episode", "00:01");
        assert!(name.len() <= MAX_FILE_NAME_BYTES);
        assert!(name.ends_with(" - 00:01"));
    }

    #[test]
    fn same_second_saves_get_a_counter() {
        let dir = tempfile::tempdir().unwrap();
        let first = save_image(dir.path(), "Show", "Ep - 00:10", b"a").unwrap();
        let second = save_image(dir.path(), "Show", "Ep - 00:10", b"b").unwrap();
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), b"a");
        assert_eq!(fs::read(&second).unwrap(), b"b");
        assert!(second.to_string_lossy().ends_with("Ep - 00:10 (1).png"));
        assert!(first.parent().unwrap().ends_with("Show"));
    }

    #[test]
    fn share_wipes_the_cache_dir_first() {
        let dir = tempfile::tempdir().unwrap();
        let cache = dir.path().join("images");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("old.png"), b"old").unwrap();
        let shared = share_image(&cache, "new", b"new").unwrap();
        assert!(!dir.path().join("images/old.png").exists());
        assert_eq!(fs::read(&shared).unwrap(), b"new");
    }

    #[test]
    fn set_as_art_needs_the_entry_to_be_kept() {
        assert_eq!(set_as_art_result(Ok(()), true), SetAsArt::Success);
        assert_eq!(
            set_as_art_result(Ok(()), false),
            SetAsArt::AddToLibraryFirst
        );
        assert_eq!(set_as_art_result(Err("io"), true), SetAsArt::Error);
    }

    #[derive(Default)]
    struct Store {
        writes: std::cell::RefCell<Vec<(&'static str, Vec<u8>)>>,
        fail: bool,
    }

    impl Store {
        fn put(&self, slot: &'static str, png: &[u8]) -> Result<(), String> {
            if self.fail {
                return Err("io".to_owned());
            }
            self.writes.borrow_mut().push((slot, png.to_vec()));
            Ok(())
        }
    }

    impl ArtStore for Store {
        fn edit_cover(&self, png: &[u8]) -> Result<(), String> {
            self.put("cover", png)
        }
        fn edit_background(&self, png: &[u8]) -> Result<(), String> {
            self.put("background", png)
        }
        fn edit_thumbnail(&self, png: &[u8]) -> Result<(), String> {
            self.put("thumbnail", png)
        }
    }

    #[test]
    fn each_art_type_writes_its_own_slot() {
        let store = Store::default();
        for art in [ArtType::Cover, ArtType::Background, ArtType::Thumbnail] {
            assert_eq!(set_as_art(&store, art, b"png", true), SetAsArt::Success);
        }
        let slots: Vec<_> = store.writes.borrow().iter().map(|(s, _)| *s).collect();
        assert_eq!(slots, ["cover", "background", "thumbnail"]);
        assert_eq!(store.writes.borrow()[0].1, b"png");
    }

    #[test]
    fn set_as_art_reports_a_failed_write_and_a_non_library_entry() {
        let ok = Store::default();
        assert_eq!(
            set_as_art(&ok, ArtType::Cover, b"x", false),
            SetAsArt::AddToLibraryFirst
        );
        let bad = Store {
            fail: true,
            ..Store::default()
        };
        assert_eq!(
            set_as_art(&bad, ArtType::Thumbnail, b"x", true),
            SetAsArt::Error
        );
    }
}

#[cfg(test)]
mod sheet_tests {
    use super::*;

    #[test]
    fn thumbnail_tile_is_local_only() {
        assert_eq!(tiles(false).len(), 4);
        assert!(!tiles(false).contains(&ScreenshotTile::SetThumbnail));
        let local = tiles(true);
        assert_eq!(local.len(), 5);
        assert_eq!(local[2], ScreenshotTile::SetThumbnail);
        assert_eq!(local[4].label(), "Save");
    }

    #[test]
    fn set_as_tiles_map_to_art_slots() {
        assert_eq!(ScreenshotTile::SetCover.art(), Some(ArtType::Cover));
        assert_eq!(
            ScreenshotTile::SetBackground.art(),
            Some(ArtType::Background)
        );
        assert_eq!(ScreenshotTile::Share.art(), None);
        assert_eq!(SET_ART_CONFIRM, "Use this image as cover art?");
    }

    #[test]
    fn subtitles_need_tracks_and_the_switch() {
        assert!(!subtitle_switch_visible(false));
        assert!(include_subtitles(true, true));
        assert!(!include_subtitles(true, false));
        assert!(!include_subtitles(false, true));
    }

    #[test]
    fn save_name_is_title_then_seconds() {
        assert_eq!(save_default_name("Show", 83), "Show_83.png");
        assert_eq!(save_default_name("a/b", 5), "a_b_5.png");
    }

    #[test]
    fn missing_capture_file_gives_the_failure_message() {
        let dir = tempfile::tempdir().unwrap();
        let result = finish_capture(dir.path(), &dir.path().join("absent.png"));
        assert_eq!(
            sheet_capture_message(&result),
            Some(PlayerMessage::ScreenshotFailed)
        );
        let ok: io::Result<PathBuf> = Ok(dir.path().to_path_buf());
        assert_eq!(sheet_capture_message(&ok), None);
    }
}
