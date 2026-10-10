//! What a page row is shown with (T-43b-015, T-43b-017, T-43b-024, T-43b-025, T-43b-013). Pure: the settings a viewer
//! holds, and the viewer's kind, give the page config, the background, the double-tap time and the long-tap switch.

use kuro_prefs::reader::ReadingMode;

use super::settings::ViewerSettings;
use super::ViewerKind;
use crate::reader::page_transform::SplitSettings;
use crate::reader::page_zoom::{self, ImageScaleType, PageConfig};
use crate::reader::theme::{self, Background};

/// The values a page view takes from the viewer's settings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageOpts {
    pub config: PageConfig,
    pub background: Background,
    pub double_tap_ms: u32,
    pub long_tap: bool,
    /// A webtoon page: decoded no wider than the view (B§8.4).
    pub webtoon: bool,
    /// The dual-page settings the decode transform follows (T-44a-050).
    pub transform: SplitSettings,
}

/// The reading mode a viewer is built for, which the automatic zoom start follows.
pub fn reading_mode(kind: ViewerKind) -> ReadingMode {
    match kind {
        ViewerKind::LeftToRight => ReadingMode::LeftToRight,
        ViewerKind::RightToLeft => ReadingMode::RightToLeft,
        ViewerKind::Vertical => ReadingMode::Vertical,
        ViewerKind::Webtoon => ReadingMode::Webtoon,
        ViewerKind::WebtoonGapped => ReadingMode::ContinuousVertical,
    }
}

/// The dual-page settings of a viewer family, and for the pager the reading direction that picks the split side.
pub fn split_settings(settings: &ViewerSettings, kind: ViewerKind) -> SplitSettings {
    if kind.is_pager() {
        SplitSettings {
            split: settings.dual_split_pager,
            invert: settings.dual_invert_pager,
            rotate: settings.dual_rotate_pager,
            rotate_invert: settings.dual_rotate_invert_pager,
            l2r: kind == ViewerKind::LeftToRight,
            webtoon: false,
        }
    } else {
        SplitSettings {
            split: settings.dual_split_webtoon,
            invert: settings.dual_invert_webtoon,
            rotate: settings.dual_rotate_webtoon,
            rotate_invert: settings.dual_rotate_invert_webtoon,
            l2r: false,
            webtoon: true,
        }
    }
}

/// The page options for `settings` in a viewer of `kind`. A webtoon page always fits the width.
pub fn page_opts(settings: &ViewerSettings, kind: ViewerKind, system_dark: bool) -> PageOpts {
    PageOpts {
        transform: split_settings(settings, kind),
        config: PageConfig {
            scale: ImageScaleType::effective(settings.image_scale_type, !kind.is_pager()),
            zoom_start: page_zoom::zoom_start(settings.zoom_start, reading_mode(kind)),
        },
        background: theme::background_for(settings.reader_theme, system_dark),
        double_tap_ms: settings.double_tap_anim_ms,
        long_tap: settings.long_tap,
        webtoon: !kind.is_pager(),
    }
}

/// The colour a background paints, as the stylesheet does (`reader-bg-*`), in 0..=1 RGB.
pub fn background_rgb(background: Background) -> (f32, f32, f32) {
    match background {
        Background::White => (1.0, 1.0, 1.0),
        Background::Black => (0.0, 0.0, 0.0),
        Background::Gray => (32.0 / 255.0, 33.0 / 255.0, 37.0 / 255.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::page_zoom::ZoomStart;
    use kuro_prefs::reader::ReaderPrefs;
    use kuro_prefs::store::PrefStore;

    fn settings(edit: impl FnOnce(&ReaderPrefs)) -> ViewerSettings {
        let p = ReaderPrefs::new(PrefStore::in_memory());
        edit(&p);
        ViewerSettings::from_prefs(&p)
    }

    #[test]
    fn the_stored_scale_type_reaches_a_pager_page_and_a_webtoon_page_fits_the_width() {
        let s = settings(|p| p.image_scale_type.set(5).unwrap());
        assert_eq!(page_opts(&s, ViewerKind::LeftToRight, false).config.scale, ImageScaleType::OriginalSize);
        assert_eq!(page_opts(&s, ViewerKind::Webtoon, false).config.scale, ImageScaleType::FitWidth);
        assert_eq!(page_opts(&s, ViewerKind::WebtoonGapped, false).config.scale, ImageScaleType::FitWidth);
    }

    #[test]
    fn unknown_stored_scale_types_fit_the_screen() {
        for stored in [0, 7, 99] {
            let s = settings(|p| p.image_scale_type.set(stored).unwrap());
            assert_eq!(page_opts(&s, ViewerKind::Vertical, false).config.scale, ImageScaleType::FitScreen, "{stored}");
        }
    }

    #[test]
    fn the_automatic_zoom_start_follows_the_reading_mode() {
        let s = settings(|_| {});
        assert_eq!(page_opts(&s, ViewerKind::LeftToRight, false).config.zoom_start, ZoomStart::Left);
        assert_eq!(page_opts(&s, ViewerKind::RightToLeft, false).config.zoom_start, ZoomStart::Right);
        assert_eq!(page_opts(&s, ViewerKind::Vertical, false).config.zoom_start, ZoomStart::Center);
        let right = settings(|p| p.zoom_start.set(3).unwrap());
        assert_eq!(page_opts(&right, ViewerKind::LeftToRight, false).config.zoom_start, ZoomStart::Right);
    }

    #[test]
    fn the_background_double_tap_time_and_long_tap_follow_the_settings() {
        let s = settings(|p| {
            p.reader_theme.set(0).unwrap();
            p.double_tap_anim_speed.set(250).unwrap();
            p.read_with_long_tap.set(false).unwrap();
        });
        let o = page_opts(&s, ViewerKind::Webtoon, false);
        assert_eq!((o.background, o.double_tap_ms, o.long_tap), (Background::White, 250, false));
        let auto = settings(|p| p.reader_theme.set(3).unwrap());
        assert_eq!(page_opts(&auto, ViewerKind::Webtoon, true).background, Background::Gray);
        assert_eq!(page_opts(&auto, ViewerKind::Webtoon, false).background, Background::White);
    }

    #[test]
    fn each_background_has_the_stylesheet_colour() {
        assert_eq!(background_rgb(Background::Black), (0.0, 0.0, 0.0));
        assert_eq!(background_rgb(Background::White), (1.0, 1.0, 1.0));
    }

    #[test]
    fn the_dual_page_settings_of_the_family_reach_the_decode_transform() {
        let s = settings(|p| {
            p.dual_page_split_paged.set(true).unwrap();
            p.dual_page_invert_paged.set(true).unwrap();
            p.dual_page_split_webtoon.set(false).unwrap();
        });
        let pager = page_opts(&s, ViewerKind::LeftToRight, false).transform;
        assert!(pager.split && pager.invert && pager.l2r && !pager.webtoon);
        assert!(!page_opts(&s, ViewerKind::RightToLeft, false).transform.l2r, "only left to right reads the left half first");
        let web = page_opts(&s, ViewerKind::Webtoon, false).transform;
        assert!(!web.split && web.webtoon, "the webtoon follows its own settings");
    }
}
