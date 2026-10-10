//! Copy to the clipboard with a confirmation toast (tasks T-47b-046 and T-47b-070). The message shows at most 50
//! characters of the copied text, cut from the middle with `kuro_core` `truncate_center`; blank text is a no-op.
#![allow(dead_code)] // no screen copies yet; the Debug and entry screens call this when they are routed here
use gtk::prelude::*;
use kuro_core::util::text::truncate_center;

/// Characters of the copied text the confirmation shows (`truncateCenter(50)` in the Kotlin code).
pub const SHOWN_CHARS: usize = 50;

/// The confirmation text for copying `content`, or `None` when there is nothing to copy.
pub fn copied_message(content: &str) -> Option<String> {
    if content.trim().is_empty() {
        return None;
    }
    let shown = truncate_center(content, SHOWN_CHARS, "...").unwrap_or_else(|_| content.to_owned());
    Some(format!("Copied to clipboard:\n{shown}"))
}

/// Puts `content` on the clipboard and shows the confirmation. A missing clipboard is logged and nothing is shown.
pub fn copy_to_clipboard(content: &str) {
    let Some(message) = copied_message(content) else { return };
    match gtk::gdk::Display::default() {
        Some(display) => {
            display.clipboard().set_text(content);
            crate::toast::notify(message);
        }
        None => tracing::error!("the clipboard is not available"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_text_gives_no_message() {
        assert_eq!(copied_message(""), None);
        assert_eq!(copied_message("  \n"), None);
    }

    #[test]
    fn the_shown_text_is_cut_from_the_middle_at_fifty_characters() {
        let short = "a".repeat(49);
        assert_eq!(copied_message(&short).unwrap(), format!("Copied to clipboard:\n{short}"));
        let exact = "b".repeat(50);
        assert_eq!(copied_message(&exact).unwrap(), format!("Copied to clipboard:\n{exact}"));
        let long: String = (0..51).map(|i| char::from(b'a' + (i % 26) as u8)).collect();
        let shown = copied_message(&long).unwrap();
        let body = shown.strip_prefix("Copied to clipboard:\n").unwrap();
        assert_eq!(body.chars().count(), 49, "23 + '...' + 23");
        assert!(body.starts_with(&long[..23]) && body.ends_with(&long[long.len() - 23..]), "{body}");
        assert!(body.contains("..."));
    }
}
