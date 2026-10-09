//! Local entries in the library and the downloads (area:14 behavior 17, T-14a-044). The update and sync rules (T-14a-045,
//! T-14a-046) are in `kuro-domain` (`app::update`, `items::chapter_sync`, `items::episode_sync`), which the app calls.

use crate::format::{is_anime_episode, is_manga_archive};
use crate::fs::FsEntry;
use crate::ids::is_local_source;

/// `display_local_badge`'s default (`LibraryPreferences`).
pub const DISPLAY_LOCAL_BADGE_DEFAULT: bool = true;
const _: () = assert!(DISPLAY_LOCAL_BADGE_DEFAULT, "the badge is on by default");

/// The help link of the local source browse toolbar (behavior 29, T-14a-047). No Kurobayomi help page exists yet, so the
/// action is omitted. The Aniyomi guide URLs are never opened.
pub const LOCAL_HELP_URL: Option<&str> = None;

/// The library badge shows only when the preference is on and the entry is local (E-SYNC-2).
pub fn shows_local_badge(display_local_badge: bool, source_id: i64) -> bool {
    display_local_badge && is_local_source(source_id)
}

/// The Download action is offered only when no selected entry is local (behavior 17.3).
pub fn download_action_allowed(source_ids: impl IntoIterator<Item = i64>) -> bool {
    source_ids.into_iter().all(|id| !is_local_source(id))
}

/// Every chapter of a local manga counts as downloaded (E-SYNC-3).
pub fn chapter_counts_as_downloaded(source_id: i64) -> bool {
    is_local_source(source_id)
}

/// The download count of a local manga folder: directories and manga archives. An EPUB is not counted (E-SYNC-4).
pub fn local_manga_download_count(children: &[FsEntry]) -> usize {
    children
        .iter()
        .filter(|entry| entry.is_dir || is_manga_archive(&entry.name))
        .count()
}

/// The download count of a local anime folder: episode files (behavior 17.1).
pub fn local_anime_download_count(children: &[FsEntry]) -> usize {
    children
        .iter()
        .filter(|entry| is_anime_episode(&entry.name))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::LOCAL_SOURCE_ID;
    use std::path::PathBuf;

    fn entry(name: &str, is_dir: bool) -> FsEntry {
        FsEntry {
            name: name.into(),
            path: PathBuf::from(name),
            is_dir,
            is_file: !is_dir,
            modified_ms: 0,
        }
    }

    #[test]
    fn the_badge_needs_the_preference_and_a_local_entry() {
        assert!(shows_local_badge(true, LOCAL_SOURCE_ID));
        assert!(!shows_local_badge(false, LOCAL_SOURCE_ID));
        assert!(!shows_local_badge(true, 42));
    }

    #[test]
    fn the_download_action_is_hidden_when_any_selection_is_local() {
        assert!(download_action_allowed([1, 2]));
        assert!(!download_action_allowed([1, LOCAL_SOURCE_ID]));
        assert!(download_action_allowed(std::iter::empty()));
    }

    #[test]
    fn local_chapters_count_as_downloaded() {
        assert!(chapter_counts_as_downloaded(LOCAL_SOURCE_ID));
        assert!(!chapter_counts_as_downloaded(7));
    }

    #[test]
    fn t_14_download_count_local_excludes_epub() {
        let children = [
            entry("Ch 1", true),
            entry("Ch 2.cbz", false),
            entry("Ch 3.EPUB", false),
            entry("cover.jpg", false),
        ];
        assert_eq!(local_manga_download_count(&children), 2);
    }

    #[test]
    fn anime_download_count_is_the_episode_files() {
        let children = [
            entry("EP 1.mkv", false),
            entry("EP 2.MKV", false),
            entry("Season 1", true),
        ];
        assert_eq!(local_anime_download_count(&children), 1);
    }
}

#[cfg(test)]
mod help_tests {
    use super::LOCAL_HELP_URL;

    #[test]
    fn the_local_help_action_never_opens_an_aniyomi_guide() {
        assert!(LOCAL_HELP_URL.is_none_or(|url| !url.contains("aniyomi.org")));
    }
}
