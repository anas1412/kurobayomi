//! Open and copy for a tracker link (T-38b-038; behaviour 8.1, E-43, E-44). Both actions are hidden when the URL is
//! blank. The copy text is the URL itself. The open action is `kuro_track::sheet::open_tracker_page` and is run by the
//! UI layer with the platform opener; this file only decides what shows.

use kuro_track::sheet::copy_link;

/// Whether the open and copy actions show for `url`.
pub fn actions_shown(url: &str) -> bool {
    !url.trim().is_empty()
}

/// The clipboard text of the copy action, or `None` for a blank link (nothing is copied).
pub fn clipboard_text(url: &str) -> Option<String> {
    copy_link(url).map(|(text, _)| text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_and_copy_are_hidden_for_a_blank_link() {
        assert!(!actions_shown(""));
        assert!(!actions_shown("   "));
        assert!(actions_shown("https://anilist.co/manga/1"));
        assert_eq!(clipboard_text(" "), None);
    }

    #[test]
    fn the_clipboard_text_is_the_url() {
        assert_eq!(clipboard_text("https://anilist.co/manga/1").as_deref(), Some("https://anilist.co/manga/1"));
    }
}
