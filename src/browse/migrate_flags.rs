//! The migrate option bits of the two catalogues (`MangaMigrationFlags.kt`, `AnimeMigrationFlags.kt`; T-40a-015, T-41a-032):
//! which options the migrate dialog offers and starts selected, the bitmap it stores, and the guard that keeps one
//! migration from starting twice (T-40a-052). Pure: the dialog widgets are the callers'. Tracking data is always copied
//! and has no bit, so the manga bit 0x04 is not offered.

use std::cell::Cell;

/// Anime options (data 5.1).
pub const EPISODES: u32 = 0x01;
pub const CATEGORIES: u32 = 0x02;
pub const CUSTOM_BACKGROUND: u32 = 0x04;
pub const CUSTOM_COVER: u32 = 0x08;
pub const DELETE_DOWNLOADED: u32 = 0x10;

/// The anime bits this catalogue uses. Other bits of a stored bitmap are kept as they are.
pub const ANIME_KNOWN: u32 = EPISODES | CATEGORIES | CUSTOM_BACKGROUND | CUSTOM_COVER | DELETE_DOWNLOADED;
/// The manga bits this catalogue uses: chapters, categories, custom cover, delete downloaded (D§4).
pub const MANGA_CHAPTERS: u32 = 0x01;
pub const MANGA_KNOWN: u32 = MANGA_CHAPTERS | CATEGORIES | CUSTOM_COVER | DELETE_DOWNLOADED;

/// The anime options in display order: episodes (only for an episodes title), categories, custom cover, custom
/// background, and delete downloaded (only when downloads exist). The conditions come from the caller's titles.
pub fn anime_options(episodes_title: bool, has_custom_cover: bool, has_custom_background: bool, has_downloads: bool) -> Vec<u32> {
    let mut offered = Vec::new();
    if episodes_title {
        offered.push(EPISODES);
    }
    offered.push(CATEGORIES);
    if has_custom_cover {
        offered.push(CUSTOM_COVER);
    }
    if has_custom_background {
        offered.push(CUSTOM_BACKGROUND);
    }
    if has_downloads {
        offered.push(DELETE_DOWNLOADED);
    }
    offered
}

/// The manga options in display order: chapters, categories, custom cover (only when the old manga has one), and delete
/// downloaded (only when downloads exist).
pub fn manga_options(has_custom_cover: bool, has_downloads: bool) -> Vec<u32> {
    let mut offered = vec![MANGA_CHAPTERS, CATEGORIES];
    if has_custom_cover {
        offered.push(CUSTOM_COVER);
    }
    if has_downloads {
        offered.push(DELETE_DOWNLOADED);
    }
    offered
}

/// The options a dialog starts with: the offered ones whose bit is set in the saved bitmap. The saved default `i32::MAX`
/// selects all of them.
pub fn preselected(saved: i32, offered: &[u32]) -> Vec<u32> {
    offered.iter().copied().filter(|bit| (saved as u32) & bit != 0).collect()
}

/// The bitmap to store after a migrate action (T-41a-032): the selected bits of this catalogue, with every bit the
/// catalogue does not use kept as it was stored.
pub fn stored_bitmap(saved: i32, selected: &[u32], known: u32) -> i32 {
    let chosen = selected.iter().fold(0, |map, bit| map | bit) & known;
    (((saved as u32) & !known) | chosen) as i32
}

/// Keeps one migration from starting twice (E-40-90). `begin` is true for the first request only, until `finish` ends
/// the run. A failed run finishes too, so the dialog can be used again.
#[derive(Default)]
pub struct RunGuard {
    running: Cell<bool>,
}

impl RunGuard {
    pub fn begin(&self) -> bool {
        !self.running.replace(true)
    }

    pub fn finish(&self) {
        self.running.set(false);
    }

    pub fn is_running(&self) -> bool {
        self.running.get()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_anime_bits_and_their_option_order_match_the_data_layer() {
        assert_eq!((EPISODES, CATEGORIES, CUSTOM_BACKGROUND, CUSTOM_COVER, DELETE_DOWNLOADED), (0x01, 0x02, 0x04, 0x08, 0x10));
        assert_eq!(ANIME_KNOWN, 0x1F);
        assert_eq!(anime_options(true, true, true, true), [EPISODES, CATEGORIES, CUSTOM_COVER, CUSTOM_BACKGROUND, DELETE_DOWNLOADED]);
        assert_eq!(anime_options(false, false, false, false), [CATEGORIES], "episodes only for an episodes title");
        assert_eq!(anime_options(false, false, false, true), [CATEGORIES, DELETE_DOWNLOADED], "delete only when downloads exist");
    }

    #[test]
    fn the_manga_options_are_chapters_categories_and_the_conditional_two() {
        assert_eq!(MANGA_KNOWN, 0x1B);
        assert_eq!(manga_options(false, false), [MANGA_CHAPTERS, CATEGORIES]);
        assert_eq!(manga_options(true, true), [MANGA_CHAPTERS, CATEGORIES, CUSTOM_COVER, DELETE_DOWNLOADED]);
    }

    #[test]
    fn the_saved_default_selects_every_offered_option() {
        let all = anime_options(true, true, true, true);
        assert_eq!(preselected(i32::MAX, &all), all, "T55: 0x7FFFFFFF selects all five");
        assert_eq!(preselected(0, &all), Vec::<u32>::new(), "a saved zero selects nothing");
        assert_eq!(preselected(CATEGORIES as i32, &all), [CATEGORIES]);
    }

    #[test]
    fn the_stored_bitmap_is_the_selection_and_keeps_the_unused_bits() {
        assert_eq!(stored_bitmap(i32::MAX, &[EPISODES], ANIME_KNOWN), 0x7FFF_FFE1, "T54: the unused bits stay set");
        assert_eq!(stored_bitmap(0, &[CATEGORIES, DELETE_DOWNLOADED], ANIME_KNOWN), 0x12);
        assert_eq!(stored_bitmap(0x40, &[], MANGA_KNOWN), 0x40, "a bit outside the catalogue survives an empty selection");
        assert_eq!(stored_bitmap(0, &[CUSTOM_BACKGROUND], MANGA_KNOWN), 0, "a bit the catalogue does not use is not written");
    }

    #[test]
    fn a_second_migrate_request_is_ignored_until_the_run_finishes() {
        let guard = RunGuard::default();
        assert!(guard.begin(), "the first request starts");
        assert!(!guard.begin(), "E-40-90: a second request is ignored");
        guard.finish();
        assert!(guard.begin(), "a failed or finished run lets the dialog start again");
    }
}
