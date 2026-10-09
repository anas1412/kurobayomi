//! Reader settings model (T-43b-030, `ReaderSettingsScreenModel.kt`). Pure: it holds the manga the settings act on, the
//! viewer in use, and the reading mode and orientation writes. Equality is by manga id and viewer flags, not by object
//! identity (edge cases section 3.2).
//!
//! The Kotlin callbacks become return values: each write returns the new viewer flags, and the caller saves them to the
//! manga row (the `mangas.viewer` column) and reopens the viewer when the mode changed.

use kuro_prefs::reader::{ReaderOrientation, ReadingMode};

use crate::reader::flags;
use crate::reader::viewers::ViewerKind;

#[derive(Clone, Debug)]
pub struct ReaderSettingsModel {
    manga_id: i64,
    viewer_flags: i64,
    viewer: Option<ViewerKind>,
}

impl ReaderSettingsModel {
    pub fn new(manga_id: i64, viewer_flags: i64) -> Self {
        Self { manga_id, viewer_flags, viewer: None }
    }

    pub fn manga_id(&self) -> i64 {
        self.manga_id
    }

    pub fn viewer_flags(&self) -> i64 {
        self.viewer_flags
    }

    /// The viewer kind in use, `None` until one is set or while the mode is unresolved.
    pub fn viewer_kind(&self) -> Option<ViewerKind> {
        self.viewer
    }

    pub fn set_viewer_kind(&mut self, kind: Option<ViewerKind>) {
        self.viewer = kind;
    }

    /// The same manga with the same flags.
    pub fn same_manga(&self, other: &Self) -> bool {
        self.manga_id == other.manga_id && self.viewer_flags == other.viewer_flags
    }

    /// Linux has no display cutout, so the cutout setting is never offered.
    pub fn has_display_cutout(&self) -> bool {
        false
    }

    /// The reading mode in effect, falling back to the global preference when the manga has none.
    pub fn reading_mode(&self, default_pref: i32) -> ReadingMode {
        flags::resolve_reading_mode(self.viewer_flags, default_pref)
    }

    /// The orientation in effect, falling back to the global preference when the manga has none.
    pub fn orientation(&self, default_pref: i32) -> ReaderOrientation {
        flags::resolve_orientation(self.viewer_flags, default_pref)
    }

    /// Applies a reading-mode choice and updates the viewer kind. Returns the new flags to save.
    pub fn choose_reading_mode(&mut self, mode: ReadingMode) -> i64 {
        self.viewer_flags = flags::set_reading_mode(self.viewer_flags, mode);
        self.viewer = ViewerKind::from_flag(i64::from(mode.flag_value())).ok();
        self.viewer_flags
    }

    /// Applies an orientation choice. Returns the new flags to save.
    pub fn choose_orientation(&mut self, orientation: ReaderOrientation) -> i64 {
        self.viewer_flags = flags::set_orientation(self.viewer_flags, orientation);
        self.viewer_flags
    }
}

impl PartialEq for ReaderSettingsModel {
    fn eq(&self, other: &Self) -> bool {
        self.same_manga(other)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_uses_the_manga_id_and_flags_only() {
        let a = ReaderSettingsModel::new(7, 0x2);
        let mut b = ReaderSettingsModel::new(7, 0x2);
        b.set_viewer_kind(Some(ViewerKind::Webtoon));
        assert_eq!(a, b);
        assert_ne!(a, ReaderSettingsModel::new(7, 0x3));
        assert_ne!(a, ReaderSettingsModel::new(8, 0x2));
    }

    #[test]
    fn a_reading_mode_choice_updates_the_flags_and_the_viewer_kind() {
        let mut model = ReaderSettingsModel::new(1, 0x38);
        let saved = model.choose_reading_mode(ReadingMode::Webtoon);
        assert_eq!(saved, 0x38 | 0x4);
        assert_eq!(model.viewer_kind(), Some(ViewerKind::Webtoon));
        model.choose_reading_mode(ReadingMode::Default);
        assert_eq!(model.viewer_kind(), None, "DEFAULT has no viewer until it is resolved");
    }

    #[test]
    fn an_orientation_choice_keeps_the_mode_bits() {
        let mut model = ReaderSettingsModel::new(1, 0x2);
        assert_eq!(model.choose_orientation(ReaderOrientation::Landscape), 0x2 | 0x18);
        assert_eq!(model.reading_mode(2), ReadingMode::RightToLeft);
        assert_eq!(model.orientation(8), ReaderOrientation::Landscape);
    }

    #[test]
    fn there_is_never_a_display_cutout_on_linux() {
        assert!(!ReaderSettingsModel::new(1, 0).has_display_cutout());
    }
}
