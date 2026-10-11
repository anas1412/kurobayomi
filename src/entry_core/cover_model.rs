//! The manga cover model (T-38a-037, T-38a-038): what the full-cover window does with the cover of a manga. Save writes it
//! to the pictures folder ("Cover saved" / "Error saving cover"), Share stages it in the cache ("Error sharing cover"),
//! Edit copies a picked file over the custom cover ("Cover updated" / "Failed to update cover") and Delete removes the
//! custom cover. Edit and Delete move `cover_last_modified` forward so every cache reloads the picture. The file work is
//! `anime::images_io` (the same functions, called with the cover kind); the texts are `anime::images::outcome`.
//! No GTK here: `full_cover_view` draws it.

use kuro_i18n::MsgKey;
use kuro_images::art::{ArtKind, ArtRef, Owner};
use kuro_images::viewer::{EditButton, ZOOM_MS, edit_button};

use super::anime::images::{ImageAction, ImageError, ImageKind, outcome};
pub use super::anime::images::{ZOOM_MIN, bumped_last_modified, hits_image, zoom_after_pinch, zoom_after_scroll};
use super::detail::MangaRow;

/// The length of the zoom transition when the window opens.
pub const OPEN_TRANSITION_MS: u64 = ZOOM_MS;

/// The cover of a manga as the image service names it.
pub fn art_of(manga: &MangaRow) -> ArtRef {
    ArtRef {
        owner: Owner::Manga,
        entry_id: manga.id,
        source_id: manga.source,
        kind: ArtKind::Cover,
        url: manga.thumbnail_url.clone(),
        favorite: manga.favorite,
        last_modified: manga.cover_last_modified,
    }
}

/// The text of a finished action: `Ok(Some(text))` is a success notice, `Ok(None)` a success with none (a share),
/// `Err(text)` the failure notice. A missing image, a failed open and an empty share all fail (never a silent success).
pub fn notice(action: ImageAction, error: Option<ImageError>) -> Result<Option<MsgKey>, MsgKey> {
    outcome(ImageKind::Cover, action, error)
}

/// Whether the action changed the custom cover, so the page must bump `cover_last_modified` and reload the picture.
pub fn bumps_cover(action: ImageAction, ok: bool) -> bool {
    ok && matches!(action, ImageAction::Edit | ImageAction::Delete)
}

/// What the Edit pill does: the file picker straight away, or a menu with Edit and Delete when a custom cover exists.
pub fn edit_pill(has_custom: bool) -> EditButton {
    edit_button(has_custom)
}

/// The labelled menu entries of the Edit pill when it is a menu, in order.
pub const EDIT_MENU: [ImageAction; 2] = [ImageAction::Edit, ImageAction::Delete];

/// How far the open transition has run, eased out, from 0 to 1.
pub fn open_progress(elapsed_ms: u64) -> f64 {
    let t = (elapsed_ms as f64 / OPEN_TRANSITION_MS as f64).clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

/// The scale of the picture during the open transition: it grows from this much of its size to full size.
pub const OPEN_SCALE_FROM: f64 = 0.6;

pub fn open_scale(elapsed_ms: u64) -> f64 {
    OPEN_SCALE_FROM + (1.0 - OPEN_SCALE_FROM) * open_progress(elapsed_ms)
}

/// Whether a click at `point` closes the window: a click beside the picture does, a click on it does not.
pub fn click_closes(widget: (f64, f64), image: Option<(f64, f64)>, point: (f64, f64)) -> bool {
    image.is_none_or(|image| !hits_image(widget, image, point))
}

/// The mime filter of the file picker.
pub const PICKER_MIME: &str = "image/*";

#[cfg(test)]
use super::anime::images::ZOOM_MAX;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::detail::MangaRow;
    use crate::entry_core::flags::ChapterFlags;

    fn manga() -> MangaRow {
        MangaRow {
            id: 4,
            source: 9,
            url: "/m".into(),
            title: "T".into(),
            author: None,
            artist: None,
            description: None,
            genres: vec![],
            status: 0,
            next_update: 0,
            fetch_interval: 0,
            thumbnail_url: Some("https://x/c.jpg".into()),
            cover_last_modified: 77,
            initialized: true,
            favorite: true,
            local: false,
            flags: ChapterFlags(0),
        }
    }

    #[test]
    fn the_art_names_the_manga_cover() {
        let art = art_of(&manga());
        assert_eq!((art.owner, art.entry_id, art.source_id, art.kind), (Owner::Manga, 4, 9, ArtKind::Cover));
        assert_eq!((art.url.as_deref(), art.last_modified, art.favorite), (Some("https://x/c.jpg"), 77, true));
    }

    #[test]
    fn each_action_has_its_success_and_failure_text() {
        assert_eq!(notice(ImageAction::Save, None), Ok(Some(MsgKey::cover_saved)));
        assert_eq!(notice(ImageAction::Save, Some(ImageError::Io)), Err(MsgKey::error_saving_cover));
        assert_eq!(notice(ImageAction::Share, None), Ok(None));
        assert_eq!(notice(ImageAction::Share, Some(ImageError::EmptyShare)), Err(MsgKey::error_sharing_cover));
        assert_eq!(notice(ImageAction::Edit, None), Ok(Some(MsgKey::cover_updated)));
        assert_eq!(notice(ImageAction::Delete, None), Ok(Some(MsgKey::cover_updated)));
        assert_eq!(notice(ImageAction::Edit, Some(ImageError::OpenFailed)), Err(MsgKey::notification_cover_update_failed));
    }

    #[test]
    fn only_a_successful_edit_or_delete_bumps_the_cover_stamp() {
        assert!(bumps_cover(ImageAction::Edit, true));
        assert!(bumps_cover(ImageAction::Delete, true));
        assert!(!bumps_cover(ImageAction::Edit, false));
        assert!(!bumps_cover(ImageAction::Save, true));
        assert!(!bumps_cover(ImageAction::Share, true));
        assert_eq!(bumped_last_modified(77, 10), 78);
        assert_eq!(bumped_last_modified(77, 500), 500);
    }

    #[test]
    fn the_edit_pill_is_a_picker_without_a_custom_cover_and_a_menu_with_one() {
        assert_eq!(edit_pill(false), EditButton::Picker);
        assert_eq!(edit_pill(true), EditButton::Menu);
        assert_eq!(EDIT_MENU, [ImageAction::Edit, ImageAction::Delete]);
    }

    #[test]
    fn the_open_transition_runs_500_ms_and_grows_the_picture() {
        assert_eq!(OPEN_TRANSITION_MS, 500);
        assert_eq!(open_scale(0), OPEN_SCALE_FROM);
        assert_eq!(open_scale(500), 1.0);
        assert_eq!(open_scale(5_000), 1.0);
        assert!(open_scale(100) > open_scale(0) && open_scale(100) < open_scale(300));
    }

    #[test]
    fn a_click_beside_the_picture_closes_and_a_click_on_it_does_not() {
        let widget = (400.0, 400.0);
        let image = Some((100.0, 100.0)); // fitted: centred, 400 x 400
        assert!(!click_closes(widget, image, (200.0, 200.0)));
        let tall = Some((100.0, 400.0)); // fitted: 100 wide, centred
        assert!(click_closes(widget, tall, (10.0, 200.0)));
        assert!(click_closes(widget, None, (200.0, 200.0)), "no picture: any click closes");
    }

    #[test]
    fn zoom_is_clamped_between_fit_and_eight_times() {
        assert_eq!(zoom_after_pinch(1.0, 20.0), ZOOM_MAX);
        assert_eq!(zoom_after_pinch(2.0, 0.1), ZOOM_MIN);
        assert!(zoom_after_scroll(1.0, -1.0) > 1.0);
        assert_eq!(zoom_after_scroll(1.0, 1.0), ZOOM_MIN);
    }
}
