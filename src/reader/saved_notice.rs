//! The notice after a page is saved (T-43a-055, `SaveImageNotifier.kt`). "Picture saved" carries a thumbnail of the
//! picture decoded to at most 720 by 1280, a default action that opens the file and a "Share" button that shares it. A
//! save again replaces the notice (one fixed id). A failed save shows "Error" with the message, or "Unknown error".
//!
//! The shared notifier of the app (`gio_notifier`) sends a title and a body only, and is not the reader's to change, so
//! the reader sends this notice itself over the application. The two actions are application actions that take the saved
//! file's path as their parameter. The thumbnail is built on the worker thread that saved the file.

use std::path::Path;

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use kuro_images::notify::PICTURE_NOTICE_ID;

use super::strings;

/// The thumbnail's bounds (`SaveImageNotifier`): at most 720 wide and 1280 tall.
pub const THUMBNAIL_MAX: (u32, u32) = (720, 1280);

/// The application action the notice's default click activates; its parameter is the saved file's path.
pub const OPEN_ACTION: &str = "reader-open-saved-picture";
/// The application action of the "Share" button; its parameter is the saved file's path.
pub const SHARE_ACTION: &str = "reader-share-saved-picture";

/// The size of a thumbnail of a `width` by `height` picture: it fits inside [`THUMBNAIL_MAX`], keeps its proportions,
/// and a picture that already fits is not enlarged. Each side is at least one pixel.
pub fn thumbnail_size(width: u32, height: u32) -> (u32, u32) {
    let (max_w, max_h) = THUMBNAIL_MAX;
    if width <= max_w && height <= max_h {
        return (width.max(1), height.max(1));
    }
    let scale = (f64::from(max_w) / f64::from(width)).min(f64::from(max_h) / f64::from(height));
    (((f64::from(width) * scale).round() as u32).clamp(1, max_w), ((f64::from(height) * scale).round() as u32).clamp(1, max_h))
}

/// The saved picture as a PNG thumbnail, or `None` when it cannot be decoded (the notice then has no picture). Blocking:
/// call it on the worker that saved the file.
pub fn thumbnail_png(path: &Path) -> Option<Vec<u8>> {
    let picture = image::open(path).ok()?;
    let (w, h) = thumbnail_size(picture.width(), picture.height());
    let small = if (w, h) == (picture.width(), picture.height()) { picture } else { picture.thumbnail(w, h) };
    let mut out = std::io::Cursor::new(Vec::new());
    small.write_to(&mut out, image::ImageFormat::Png).ok()?;
    Some(out.into_inner())
}

/// Registers the two actions on the application. Idempotent.
fn install_actions(app: &gtk::Application) {
    let register = |name: &str, run: fn(&Path)| {
        if app.lookup_action(name).is_some() {
            return;
        }
        let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
        action.connect_activate(move |_, parameter| {
            if let Some(path) = parameter.and_then(|p| p.get::<String>()) {
                run(Path::new(&path));
            }
        });
        app.add_action(&action);
    };
    register(OPEN_ACTION, |path| {
        if let Err(error) = kuro_platform::opener::open(path) {
            tracing::warn!(error = %error, "the saved picture could not be opened");
        }
    });
    register(SHARE_ACTION, |path| {
        if let Err(error) = kuro_images::share::reveal(path) {
            tracing::warn!(error = %error, "the saved picture could not be shared");
        }
    });
}

/// The application the reader runs in, when there is one.
pub fn application() -> Option<gtk::Application> {
    gio::Application::default()?.downcast::<gtk::Application>().ok()
}

/// Withdraws the notice of the previous save. Called when a save starts.
pub fn clear(app: &gtk::Application) {
    app.withdraw_notification(PICTURE_NOTICE_ID);
}

/// Builds the "Picture saved" notice for a file. Public so a test can look at what it carries.
pub fn saved_notification(path: &Path, thumbnail: Option<Vec<u8>>) -> gio::Notification {
    let notification = gio::Notification::new(strings::PICTURE_SAVED);
    let target = path.to_string_lossy().into_owned().to_variant();
    notification.set_default_action_and_target_value(&format!("app.{OPEN_ACTION}"), Some(&target));
    notification.add_button_with_target_value(strings::ACTION_SHARE, &format!("app.{SHARE_ACTION}"), Some(&target));
    if let Some(bytes) = thumbnail {
        notification.set_icon(&gio::BytesIcon::new(&glib::Bytes::from_owned(bytes)));
    }
    notification
}

/// Shows "Picture saved" for `path`, replacing the notice of an earlier save.
pub fn show_saved(app: &gtk::Application, path: &Path, thumbnail: Option<Vec<u8>>) {
    install_actions(app);
    app.send_notification(Some(PICTURE_NOTICE_ID), &saved_notification(path, thumbnail));
}

/// Shows "Error" with the message, or "Unknown error" when there is none.
pub fn show_failed(app: &gtk::Application, message: Option<&str>) {
    let notification = gio::Notification::new(strings::DOWNLOAD_NOTIFIER_TITLE_ERROR);
    notification.set_body(Some(message.filter(|m| !m.trim().is_empty()).unwrap_or(strings::UNKNOWN_ERROR)));
    app.send_notification(Some(PICTURE_NOTICE_ID), &notification);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thumbnail_fits_720_by_1280_keeps_its_proportions_and_never_enlarges() {
        assert_eq!(thumbnail_size(100, 200), (100, 200), "a small picture is left alone");
        assert_eq!(thumbnail_size(720, 1280), (720, 1280));
        assert_eq!(thumbnail_size(1440, 2560), (720, 1280));
        assert_eq!(thumbnail_size(1440, 1440), (720, 720), "width is the limit of a square");
        assert_eq!(thumbnail_size(800, 4000), (256, 1280), "a long strip is limited by its height");
        let (w, h) = thumbnail_size(10_000, 10);
        assert_eq!((w, h), (720, 1), "a sliver keeps at least one pixel");
        assert_eq!(thumbnail_size(0, 0), (1, 1));
    }

    #[test]
    fn the_thumbnail_of_a_large_picture_decodes_to_at_most_the_bound() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("page.png");
        image::RgbaImage::from_pixel(1000, 3000, image::Rgba([1, 2, 3, 255])).save(&path).unwrap();
        let png = thumbnail_png(&path).expect("a thumbnail");
        let small = image::load_from_memory(&png).unwrap();
        assert!(small.width() <= THUMBNAIL_MAX.0 && small.height() <= THUMBNAIL_MAX.1);
        assert_eq!((small.width(), small.height()), (427, 1280));
        assert!(thumbnail_png(&dir.path().join("missing.png")).is_none());
        std::fs::write(dir.path().join("bad.png"), b"nope").unwrap();
        assert!(thumbnail_png(&dir.path().join("bad.png")).is_none());
    }
}
