//! The image model of an anime entry: the cover and the background (T-39a-045, T-39a-046, T-39a-058, T-39a-065). Upstream:
//! `AnimeImageScreenModel.kt`, `ImageSaver.kt`, `AnimeCoverCache.kt`, `AnimeBackgroundCache.kt`, `DiskUtil.kt` and the
//! `AnimeImagesDialog.kt` pager. The rules are pure; the window and the file writes are the callers'. Defects fixed here:
//! D-2 (no image is an error), D-3 (background uses the background url), D-4 (safe directory names), D-13 (open failure
//! shows the update message), D-14 (log before the snackbar), D-15 (an empty share shows the share error).

use std::path::PathBuf;

use kuro_core::hash::md5_hex;
use kuro_data::anime::Anime;
use kuro_i18n::MsgKey;

/// Which image of the entry a page or an action is about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageKind {
    Cover,
    Background,
}

impl ImageKind {
    pub fn other(self) -> Self {
        match self {
            Self::Cover => Self::Background,
            Self::Background => Self::Cover,
        }
    }

    /// The file a saved image is written to, without the extension: `cover` or `background`.
    pub fn stem(self) -> &'static str {
        match self {
            Self::Cover => "cover",
            Self::Background => "background",
        }
    }

    /// The url a fetch of this image uses. D-3: a background always uses the background url, never the thumbnail one.
    pub fn source_url(self, anime: &Anime) -> Option<&str> {
        let url = match self {
            Self::Cover => anime.thumbnail_url.as_deref(),
            Self::Background => anime.background_url.as_deref(),
        };
        url.filter(|u| !u.is_empty())
    }

    /// The cache folder of the custom image of this kind, under the app data directory (T-39a-065). The name is the md5
    /// of the anime id, as the Android app stores it, so a file made there is found without renaming.
    pub fn custom_dir(self) -> &'static str {
        match self {
            Self::Cover => "animecovers/custom",
            Self::Background => "animebackgrounds/custom",
        }
    }
}

/// The path, relative to the app data directory, of the custom image of an anime (`animecovers/custom/<md5(id)>`).
pub fn custom_relative_path(kind: ImageKind, anime_id: i64) -> PathBuf {
    PathBuf::from(kind.custom_dir()).join(md5_hex(anime_id.to_string().as_bytes()))
}

/// Whether the anime has a custom image of this kind on disk under `data_dir`.
pub fn has_custom(data_dir: &std::path::Path, kind: ImageKind, anime_id: i64) -> bool {
    data_dir.join(custom_relative_path(kind, anime_id)).is_file()
}

/// The extension a saved image gets, from its magic bytes (PNG, JPEG or WebP). Anything else is not an image.
pub fn image_extension(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else {
        None
    }
}

/// The file name a saved image gets: `cover.<ext>` or `background.<ext>`.
pub fn saved_file_name(kind: ImageKind, ext: &str) -> String {
    format!("{}.{ext}", kind.stem())
}

/// `buildValidFilename` for the save folder (D-4): `/ \ : * ? " < > |` and control characters become `_`, leading and
/// trailing dots and spaces are trimmed, and an empty result becomes `(invalid)`.
pub fn sanitize_dir_name(title: &str) -> String {
    let replaced: String = title
        .chars()
        .map(|c| if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() { '_' } else { c })
        .collect();
    let trimmed = replaced.trim_matches(|c: char| c == '.' || c == ' ');
    if trimmed.is_empty() { "(invalid)".to_owned() } else { trimmed.to_owned() }
}

/// What the image window's pager shows: the page, the arrow that switches to the other page, and the edit label of the
/// current page. On the cover page the arrow says "Edit background" and edit says "Edit cover" (T-I10), and the reverse
/// on the background page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Pager {
    pub page: ImageKind,
}

impl Pager {
    pub fn new(page: ImageKind) -> Self {
        Self { page }
    }

    pub fn switch(self) -> Self {
        Self { page: self.page.other() }
    }

    pub fn switch_label(self) -> MsgKey {
        match self.page {
            ImageKind::Cover => MsgKey::action_edit_background,
            ImageKind::Background => MsgKey::action_edit_cover,
        }
    }

    pub fn edit_label(self) -> MsgKey {
        match self.page {
            ImageKind::Cover => MsgKey::action_edit_cover,
            ImageKind::Background => MsgKey::action_edit_background,
        }
    }
}

/// What the Edit button does: a custom image gives a menu with Edit and Delete, none gives Edit straight away (T-I11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditButton {
    /// Runs the edit at once.
    Edit,
    /// Shows the menu: Edit and Delete.
    Menu,
}

pub fn edit_button(has_custom: bool) -> EditButton {
    if has_custom { EditButton::Menu } else { EditButton::Edit }
}

/// Delete asks first on Linux (OD-3): the destructive action is confirmed.
pub const DELETE_NEEDS_CONFIRMATION: bool = true;

/// The actions the window runs on the page's image.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageAction {
    Save,
    Share,
    Edit,
    Delete,
}

/// Why an action failed. `NoImage` is the missing bitmap of D-2, `OpenFailed` the edit stream of D-13, `EmptyShare` the
/// share with no bytes of D-15.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    NoImage,
    OpenFailed,
    EmptyShare,
    Io,
}

/// The snackbar text after a successful action (p1-ui-parity 10). A share has no success text.
pub fn success_message(kind: ImageKind, action: ImageAction) -> Option<MsgKey> {
    match (action, kind) {
        (ImageAction::Save, ImageKind::Cover) => Some(MsgKey::cover_saved),
        (ImageAction::Save, ImageKind::Background) => Some(MsgKey::background_saved),
        (ImageAction::Edit | ImageAction::Delete, ImageKind::Cover) => Some(MsgKey::cover_updated),
        (ImageAction::Edit | ImageAction::Delete, ImageKind::Background) => Some(MsgKey::background_updated),
        (ImageAction::Share, _) => None,
    }
}

/// The snackbar text for a failed action (p1-ui-parity 10). An edit or delete failure says the update failed, whatever
/// the error, so D-13 is covered: the open failure of the stream is one of these.
pub fn failure_message(kind: ImageKind, action: ImageAction) -> MsgKey {
    match (action, kind) {
        (ImageAction::Save, ImageKind::Cover) => MsgKey::error_saving_cover,
        (ImageAction::Save, ImageKind::Background) => MsgKey::error_saving_background,
        (ImageAction::Share, ImageKind::Cover) => MsgKey::error_sharing_cover,
        (ImageAction::Share, ImageKind::Background) => MsgKey::error_sharing_background,
        (ImageAction::Edit | ImageAction::Delete, ImageKind::Cover) => MsgKey::notification_cover_update_failed,
        (ImageAction::Edit | ImageAction::Delete, ImageKind::Background) => MsgKey::notification_background_update_failed,
    }
}

/// The result of one action after its error is mapped. A missing image, a failed open, or an empty share all end in
/// the failure text of the action; none of them reads as success (D-2, D-13, D-15).
pub fn outcome(kind: ImageKind, action: ImageAction, error: Option<ImageError>) -> Result<Option<MsgKey>, MsgKey> {
    match error {
        None => Ok(success_message(kind, action)),
        Some(_) => Err(failure_message(kind, action)),
    }
}

/// Reports a failed action: the log line is written first, then the snackbar text is returned (D-14). The sink is the
/// logger, so a test can check the order.
pub fn report_failure(
    kind: ImageKind,
    action: ImageAction,
    error: ImageError,
    log: &mut impl FnMut(String),
) -> MsgKey {
    log(format!("image {:?} {:?} failed: {:?}", kind, action, error));
    failure_message(kind, action)
}

/// The zoom of the image window: 1 fits the window, and a pinch or Ctrl+scroll goes up to eight times that.
pub const ZOOM_MIN: f64 = 1.0;
pub const ZOOM_MAX: f64 = 8.0;

pub fn clamp_zoom(zoom: f64) -> f64 {
    if zoom.is_nan() { ZOOM_MIN } else { zoom.clamp(ZOOM_MIN, ZOOM_MAX) }
}

/// A pinch: the zoom the gesture started at times its scale.
pub fn zoom_after_pinch(start: f64, scale: f64) -> f64 {
    clamp_zoom(start * scale)
}

/// Ctrl+scroll: a step up (negative `dy`) zooms in by 10%, a step down zooms out.
pub fn zoom_after_scroll(zoom: f64, dy: f64) -> f64 {
    clamp_zoom(zoom * 1.1_f64.powf(-dy))
}

/// The rectangle `(x, y, width, height)` a picture of `image` pixels covers inside a widget of `widget` size when it is
/// fitted to contain (`ContentFit::Contain`): centred, scaled to fit, never cropped.
pub fn contained_rect(widget: (f64, f64), image: (f64, f64)) -> (f64, f64, f64, f64) {
    if image.0 <= 0.0 || image.1 <= 0.0 {
        return (0.0, 0.0, 0.0, 0.0);
    }
    let scale = (widget.0 / image.0).min(widget.1 / image.1);
    let (w, h) = (image.0 * scale, image.1 * scale);
    ((widget.0 - w) / 2.0, (widget.1 - h) / 2.0, w, h)
}

/// Whether a click at `point` (in widget coordinates) lands on the picture. A click beside it is a click on the
/// background, which closes the window.
pub fn hits_image(widget: (f64, f64), image: (f64, f64), point: (f64, f64)) -> bool {
    let (x, y, w, h) = contained_rect(widget, image);
    point.0 >= x && point.0 <= x + w && point.1 >= y && point.1 <= y + h
}

/// The modification value after an edit or delete (T-I12): later than the old one, so the cache reloads the picture.
pub fn bumped_last_modified(old: i64, now_ms: i64) -> i64 {
    now_ms.max(old + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_names_and_extensions_from_magic_bytes() {
        // T-I1
        assert_eq!(saved_file_name(ImageKind::Cover, "jpg"), "cover.jpg");
        assert_eq!(saved_file_name(ImageKind::Background, "png"), "background.png");
        assert_eq!(image_extension(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0]), Some("png"));
        assert_eq!(image_extension(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("jpg"));
        let mut webp = b"RIFF\0\0\0\0WEBPVP8 ".to_vec();
        assert_eq!(image_extension(&webp), Some("webp"));
        webp[8] = b'X';
        assert_eq!(image_extension(&webp), None, "a RIFF file that is not WebP is not an image");
        assert_eq!(image_extension(b"plain text"), None);
    }

    #[test]
    fn a_background_uses_the_background_url_not_the_thumbnail() {
        // T-I3 (D-3)
        let anime = Anime {
            thumbnail_url: Some("https://a/thumb.jpg".into()),
            background_url: Some("https://a/back.jpg".into()),
            ..Anime::create()
        };
        assert_eq!(ImageKind::Background.source_url(&anime), Some("https://a/back.jpg"));
        assert_eq!(ImageKind::Cover.source_url(&anime), Some("https://a/thumb.jpg"));
        let none = Anime { background_url: Some(String::new()), ..Anime::create() };
        assert_eq!(ImageKind::Background.source_url(&none), None, "an empty url is no image");
    }

    #[test]
    fn directory_names_are_sanitised() {
        // T-I4 (D-4)
        assert_eq!(sanitize_dir_name("Fate/Zero"), "Fate_Zero");
        assert_eq!(sanitize_dir_name("..."), "(invalid)");
        assert_eq!(sanitize_dir_name(""), "(invalid)");
        assert_eq!(sanitize_dir_name("a:b*c"), "a_b_c");
        assert_eq!(sanitize_dir_name(" .x. "), "x");
        assert_eq!(sanitize_dir_name("tab\there"), "tab_here");
    }

    #[test]
    fn custom_images_live_under_the_md5_of_the_id() {
        // T-39a-065
        assert_eq!(
            custom_relative_path(ImageKind::Cover, 7),
            PathBuf::from("animecovers/custom").join(md5_hex(b"7")),
        );
        assert_eq!(
            custom_relative_path(ImageKind::Background, 7),
            PathBuf::from("animebackgrounds/custom").join(md5_hex(b"7")),
        );
    }

    #[test]
    fn an_android_custom_cover_is_found_without_renaming() {
        // T-39a-065: the file the Android app wrote under the md5 of the id is the one the page reads
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("animecovers/custom").join(md5_hex(b"42"));
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]).unwrap();
        assert!(has_custom(dir.path(), ImageKind::Cover, 42));
        assert!(!has_custom(dir.path(), ImageKind::Background, 42));
        assert!(!has_custom(dir.path(), ImageKind::Cover, 43));
    }

    #[test]
    fn the_pager_labels_are_reversed_on_the_background_page() {
        // T-I10
        let cover = Pager::new(ImageKind::Cover);
        assert_eq!(cover.switch_label(), MsgKey::action_edit_background);
        assert_eq!(cover.edit_label(), MsgKey::action_edit_cover);
        let back = cover.switch();
        assert_eq!(back.page, ImageKind::Background);
        assert_eq!(back.switch_label(), MsgKey::action_edit_cover);
        assert_eq!(back.edit_label(), MsgKey::action_edit_background);
    }

    #[test]
    fn edit_shows_a_menu_only_for_a_custom_image() {
        // T-I11
        assert_eq!(edit_button(true), EditButton::Menu);
        assert_eq!(edit_button(false), EditButton::Edit);
    }

    #[test]
    fn a_missing_image_or_failed_open_is_an_error_not_a_success() {
        // T-I2, T-I5, T-I7
        assert_eq!(outcome(ImageKind::Cover, ImageAction::Save, None), Ok(Some(MsgKey::cover_saved)));
        assert_eq!(outcome(ImageKind::Cover, ImageAction::Save, Some(ImageError::NoImage)), Err(MsgKey::error_saving_cover));
        assert_eq!(outcome(ImageKind::Background, ImageAction::Edit, Some(ImageError::OpenFailed)), Err(MsgKey::notification_background_update_failed));
        assert_eq!(outcome(ImageKind::Cover, ImageAction::Share, Some(ImageError::EmptyShare)), Err(MsgKey::error_sharing_cover));
        assert_eq!(outcome(ImageKind::Cover, ImageAction::Share, None), Ok(None), "a share has no success text");
    }

    #[test]
    fn the_log_line_comes_before_the_snackbar() {
        // T-I6 (D-14)
        let mut order = Vec::new();
        let message = report_failure(ImageKind::Cover, ImageAction::Save, ImageError::Io, &mut |line| order.push(line));
        assert_eq!(message, MsgKey::error_saving_cover);
        assert_eq!(order.len(), 1, "one log line, written before the text is returned");
    }

    #[test]
    fn zoom_stays_between_fit_and_eight_times() {
        assert_eq!(zoom_after_pinch(1.0, 0.5), ZOOM_MIN);
        assert_eq!(zoom_after_pinch(4.0, 4.0), ZOOM_MAX);
        assert_eq!(zoom_after_pinch(2.0, 1.5), 3.0);
        assert!(zoom_after_scroll(1.0, -1.0) > 1.0, "scrolling up zooms in");
        assert_eq!(zoom_after_scroll(1.0, 1.0), ZOOM_MIN, "scrolling down at fit stays at fit");
        assert_eq!(zoom_after_scroll(ZOOM_MAX, -5.0), ZOOM_MAX);
        assert_eq!(clamp_zoom(f64::NAN), ZOOM_MIN);
    }

    #[test]
    fn a_click_beside_the_picture_is_a_background_click() {
        // a 100x200 picture in a 400x200 widget sits in the middle: x from 150 to 250
        assert_eq!(contained_rect((400.0, 200.0), (100.0, 200.0)), (150.0, 0.0, 100.0, 200.0));
        assert!(hits_image((400.0, 200.0), (100.0, 200.0), (200.0, 100.0)));
        assert!(!hits_image((400.0, 200.0), (100.0, 200.0), (20.0, 100.0)));
        assert!(!hits_image((400.0, 200.0), (0.0, 0.0), (200.0, 100.0)), "no picture, no hit");
        assert_eq!(contained_rect((100.0, 100.0), (400.0, 200.0)), (0.0, 25.0, 100.0, 50.0), "a wide picture is letterboxed");
    }

    #[test]
    fn an_edit_bumps_the_modified_value_so_the_picture_reloads() {
        // T-I12
        assert_eq!(bumped_last_modified(5, 9), 9);
        assert_eq!(bumped_last_modified(9, 9), 10);
    }
}
