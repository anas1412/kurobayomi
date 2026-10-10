//! Header shortcuts of the entry screens (T-38a-046). The chapter and episode list keys are `selection::key_command`. In the
//! header: F opens the filter sheet, R refreshes, Ctrl+M migrates and W opens the WebView when it is available. Upstream:
//! `MangaScreen.kt` key handling. Pure: the window asks `header_key` and shows `HELP`.

use super::selection::Mods;

/// What a header shortcut does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderKey {
    Filter,
    Refresh,
    Migrate,
    WebView,
}

/// The header action of a lowercase letter with these modifiers. W needs the WebView feature to be on.
pub fn header_key(letter: char, mods: Mods, webview_available: bool) -> Option<HeaderKey> {
    match (letter, mods.ctrl) {
        ('f', false) => Some(HeaderKey::Filter),
        ('r', false) => Some(HeaderKey::Refresh),
        ('m', true) => Some(HeaderKey::Migrate),
        ('w', false) if webview_available => Some(HeaderKey::WebView),
        _ => None,
    }
}

/// The in-app help list: each shortcut and what it does (the list is a labelled proposal, in English).
pub const HELP: &[(&str, &str)] = &[
    ("Up / Down", "Move focus in the chapter list"),
    ("Enter", "Open the focused chapter"),
    ("Space", "Toggle the focused chapter"),
    ("Shift+click", "Select a range from the anchor"),
    ("Ctrl+click", "Toggle one chapter"),
    ("Ctrl+A", "Select all"),
    ("Ctrl+I", "Invert the selection"),
    ("Escape", "Clear the selection"),
    ("F", "Open the filter sheet"),
    ("R", "Refresh"),
    ("Ctrl+M", "Migrate"),
    ("W", "Open the WebView"),
    ("F1 or ?", "Show this help"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_header_letters_map_to_their_actions() {
        let none = Mods::default();
        let ctrl = Mods { ctrl: true, shift: false };
        assert_eq!(header_key('f', none, false), Some(HeaderKey::Filter));
        assert_eq!(header_key('r', none, false), Some(HeaderKey::Refresh));
        assert_eq!(header_key('m', ctrl, false), Some(HeaderKey::Migrate));
        assert_eq!(header_key('m', none, false), None, "M alone does nothing");
        assert_eq!(header_key('x', none, true), None);
    }

    #[test]
    fn w_opens_the_webview_only_when_it_is_available() {
        let none = Mods::default();
        assert_eq!(header_key('w', none, true), Some(HeaderKey::WebView));
        assert_eq!(header_key('w', none, false), None);
    }

    #[test]
    fn the_help_lists_the_chapter_and_header_keys() {
        assert!(HELP.iter().any(|(keys, _)| *keys == "Ctrl+I"));
        assert!(HELP.iter().any(|(keys, _)| *keys == "Ctrl+M"));
        assert!(HELP.len() >= 13);
        assert!(HELP.iter().any(|(keys, _)| *keys == "F1 or ?"));
    }
}
