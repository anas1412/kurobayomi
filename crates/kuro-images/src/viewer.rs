//! The full-size cover and background viewer, as a model the UI draws (area:34 behavior section 26, T-34a-053).
//!
//! Manga shows one page, the cover. Anime shows two: the cover, then the background. The viewer requests the
//! original size and never reads or writes the memory cache (behavior section 26.2).

use crate::options::{CachePolicy, LoadOptions, RequestSize};

/// Length of the zoom animation when a page opens, in milliseconds (behavior section 26.2).
pub const ZOOM_MS: u64 = 500;

/// Which kind of entry the viewer shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind {
    Manga,
    Anime,
}

/// How many pages the viewer has (behavior section 26.1).
pub fn page_count(kind: EntryKind) -> usize {
    match kind {
        EntryKind::Manga => 1,
        EntryKind::Anime => 2,
    }
}

/// The page the switch button moves to: anime toggles between cover and background, manga has none (behavior
/// section 26.1, `1 - currentPage`).
pub fn switch_page(kind: EntryKind, page: usize) -> usize {
    match kind {
        EntryKind::Manga => 0,
        EntryKind::Anime => usize::from(page == 0),
    }
}

/// Request options of one page: original size, the background for page 1 of an anime entry, and the memory cache
/// disabled (behavior section 26.2).
pub fn page_options(kind: EntryKind, page: usize) -> LoadOptions {
    LoadOptions {
        use_background: kind == EntryKind::Anime && page == 1,
        size: RequestSize::Original,
        memory_cache: CachePolicy::Disabled,
        ..LoadOptions::default()
    }
}

/// What the edit button does (behavior sections 24.1 and 26.3): with a custom image it opens a menu with Edit and
/// Delete, otherwise it opens the file picker straight away.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditButton {
    Picker,
    Menu,
}

pub fn edit_button(has_custom: bool) -> EditButton {
    if has_custom {
        EditButton::Menu
    } else {
        EditButton::Picker
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manga_has_one_page_and_anime_has_two() {
        assert_eq!(page_count(EntryKind::Manga), 1);
        assert_eq!(page_count(EntryKind::Anime), 2);
    }

    #[test]
    fn switch_toggles_anime_pages_only() {
        assert_eq!(switch_page(EntryKind::Anime, 0), 1);
        assert_eq!(switch_page(EntryKind::Anime, 1), 0);
        assert_eq!(switch_page(EntryKind::Manga, 0), 0);
    }

    #[test]
    fn page_one_of_anime_asks_for_the_background() {
        assert!(!page_options(EntryKind::Anime, 0).use_background);
        assert!(page_options(EntryKind::Anime, 1).use_background);
        assert!(!page_options(EntryKind::Manga, 0).use_background);
    }

    #[test]
    fn viewer_pages_skip_the_memory_cache_and_use_original_size() {
        for kind in [EntryKind::Manga, EntryKind::Anime] {
            for page in 0..page_count(kind) {
                let opts = page_options(kind, page);
                assert_eq!(opts.memory_cache, CachePolicy::Disabled);
                assert_eq!(opts.size, RequestSize::Original);
            }
        }
    }

    #[test]
    fn zoom_is_500_ms() {
        assert_eq!(ZOOM_MS, 500);
    }

    #[test]
    fn edit_button_opens_a_menu_only_for_a_custom_image() {
        assert_eq!(edit_button(true), EditButton::Menu);
        assert_eq!(edit_button(false), EditButton::Picker);
    }
}
