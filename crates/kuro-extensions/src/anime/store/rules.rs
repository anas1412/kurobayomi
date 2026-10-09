//! The fixed text and enable rules of the store dialogs, the failure banner and the copy action (T-19a-049, T-19a-057,
//! T-19a-058). The GTK layer draws these strings. The rules live here so they can be tested without a display.

use kuro_core::util::text::truncate_center;

/// The label of the Create dialog's URL field.
pub const STORE_URL_LABEL: &str = "Store index URL";
/// The body of the Confirm dialog, above the read-only URL.
pub const CONFIRM_BODY: &str = "Do you wish to add the extension store below?";
/// The label of the Add button while a store is being fetched.
pub const PROCESSING_LABEL: &str = "Processing…";

/// The Create dialog's `Add` button: enabled only for text that is not blank, not a duplicate and not processing.
pub fn create_add_enabled(text: &str, duplicate: bool, processing: bool) -> bool {
    !text.trim().is_empty() && !duplicate && !processing
}

/// The Confirm dialog's `Add` button: enabled only when the store is not a duplicate and not processing.
pub fn confirm_add_enabled(duplicate: bool, processing: bool) -> bool {
    !duplicate && !processing
}

/// The Delete dialog body.
pub fn delete_body(name: &str, url: &str) -> String {
    format!("Do you wish to remove the extension store \"{name}\" ({url})?")
}

/// The banner for one store that a refresh could not update (K-6).
pub fn failed_store_banner(name: &str) -> String {
    format!("Could not update store \"{name}\"")
}

/// The banner after the index URL is copied. The URL is cut to 50 characters, with `...` in the middle (K-4).
pub fn copied_banner(url: &str) -> String {
    let shown = truncate_center(url, 50, "...").unwrap_or_else(|_| url.to_owned());
    format!("Copied to clipboard: {shown}")
}

/// The text to copy. Blank content is not copied (`copyToClipboard`).
pub fn clipboard_text(text: &str) -> Option<&str> {
    (!text.trim().is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_create_add_button_needs_text_and_no_duplicate_and_no_processing() {
        assert!(create_add_enabled("https://a.test/index.min.json", false, false));
        assert!(!create_add_enabled("   ", false, false));
        assert!(!create_add_enabled("https://a.test/index.min.json", true, false));
        assert!(!create_add_enabled("https://a.test/index.min.json", false, true));
    }

    #[test]
    fn the_confirm_add_button_ignores_the_text_and_needs_no_duplicate_and_no_processing() {
        assert!(confirm_add_enabled(false, false));
        assert!(!confirm_add_enabled(true, false));
        assert!(!confirm_add_enabled(false, true));
    }

    #[test]
    fn the_delete_body_quotes_the_name_and_the_url() {
        assert_eq!(
            delete_body("Demo", "https://a.test/index.min.json"),
            "Do you wish to remove the extension store \"Demo\" (https://a.test/index.min.json)?"
        );
    }

    #[test]
    fn the_failure_banner_names_the_store() {
        assert_eq!(failed_store_banner("Demo"), "Could not update store \"Demo\"");
    }

    #[test]
    fn a_short_url_is_copied_whole_and_a_long_one_is_cut_to_fifty_characters() {
        assert_eq!(copied_banner("https://a.test/i.json"), "Copied to clipboard: https://a.test/i.json");
        let long = format!("https://example.org/{}/index.min.json", "x".repeat(40));
        let banner = copied_banner(&long);
        let shown = banner.strip_prefix("Copied to clipboard: ").unwrap();
        assert!(shown.chars().count() <= 50, "the shared truncate_center rule keeps the budget, and an odd one loses a character");
        assert!(shown.starts_with("https://ex") && shown.contains("...") && shown.ends_with("index.min.json"));
    }

    #[test]
    fn blank_content_is_not_copied() {
        assert_eq!(clipboard_text("  "), None);
        assert_eq!(clipboard_text("https://a.test"), Some("https://a.test"));
    }
}
