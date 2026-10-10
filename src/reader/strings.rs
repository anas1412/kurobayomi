//! English reader strings keyed by the upstream message keys (`p1-behavior.md` section 20, T-43a-061).
//! Format arguments are plain function parameters, so the `%1$s` and `%d` placeholders become `format!`.
//! Strings marked "text not re-read" in section 20 are kept as the spec gives them and must be verified.

pub const PAGE_LIST_EMPTY_ERROR: &str = "No pages found";
pub const LOADER_NOT_IMPLEMENTED_ERROR: &str = "Source not found";
/// A literal in `ReaderActivity.kt:181`, not a localised key.
pub const UNKNOWN_ERR: &str = "Unknown err";
pub const UNKNOWN_ERROR: &str = "Unknown error";
/// The downloaded-only card text (E-LIST-2 proposal). The card shows it as is, without the load-failure prefix.
pub const CHAPTER_NOT_DOWNLOADED: &str = "The chapter is not downloaded. Turn off downloaded-only mode or download it first.";
pub const DOWNLOAD_NOTIFIER_TITLE_ERROR: &str = "Error";
pub const LOADING: &str = "Loading…";
pub const ON: &str = "On";
pub const OFF: &str = "Off";
pub const RETRY: &str = "Retry";
pub const CLOSE: &str = "Close";

pub const TRANSITION_PREVIOUS: &str = "Previous:";
pub const TRANSITION_NEXT: &str = "Next:";
pub const TRANSITION_FINISHED: &str = "Finished:";
pub const TRANSITION_CURRENT: &str = "Current:";
pub const TRANSITION_NO_PREVIOUS: &str = "There's no previous chapter";
pub const TRANSITION_NO_NEXT: &str = "There's no next chapter";
/// `transition_pages_loading`: the status line while the adjacent chapter loads (B§11.3).
pub const TRANSITION_PAGES_LOADING: &str = "Loading pages…";
pub const LABEL_DOWNLOADED: &str = "Downloaded";

pub const PICTURE_SAVED: &str = "Picture saved";
pub const SHARE_PAGE_INFO: &str = "%1$s: %2$s, page %3$d";
pub const COVER_UPDATED: &str = "Cover updated";
pub const NOTIFICATION_FIRST_ADD_TO_LIBRARY: &str = "Please add the entry to your library before doing this";
pub const NOTIFICATION_COVER_UPDATE_FAILED: &str = "Failed to update cover";
pub const SET_AS_COVER: &str = "Set as cover";
pub const CONFIRM_SET_IMAGE_AS_COVER: &str = "Use this image as cover art?";
pub const ACTION_OK: &str = "OK";
pub const ACTION_CANCEL: &str = "Cancel";
pub const ACTION_SAVE: &str = "Save";
pub const ACTION_SHARE: &str = "Share";
pub const ACTION_COPY_TO_CLIPBOARD: &str = "Copy to clipboard";
pub const ACTION_APPLY: &str = "Apply";
pub const ACTION_REVERT_TO_DEFAULT: &str = "Revert to default";

pub const ACTION_BOOKMARK: &str = "Bookmark chapter";
pub const ACTION_REMOVE_BOOKMARK: &str = "Unbookmark chapter";
pub const ACTION_OPEN_IN_WEB_VIEW: &str = "Open in WebView";
pub const ACTION_OPEN_IN_BROWSER: &str = "Open in browser";
pub const ACTION_NEXT_CHAPTER: &str = "Next chapter";
pub const ACTION_PREVIOUS_CHAPTER: &str = "Previous chapter";
pub const VIEWER: &str = "Reading mode";
pub const ROTATION_TYPE: &str = "Rotation";
pub const PREF_CROP_BORDERS: &str = "Crop borders";
pub const ACTION_SETTINGS: &str = "Settings";

pub const LABEL_DEFAULT: &str = "Default";
pub const ROTATION_FREE: &str = "Free";
pub const ROTATION_PORTRAIT: &str = "Portrait";
pub const ROTATION_LANDSCAPE: &str = "Landscape";
pub const ROTATION_FORCE_PORTRAIT: &str = "Locked portrait";
pub const ROTATION_FORCE_LANDSCAPE: &str = "Locked landscape";
pub const ROTATION_REVERSE_PORTRAIT: &str = "Reverse portrait";
/// The note under the rotation choices: the value is stored but no rotation follows (E-ORI-1).
pub const ROTATION_NO_EFFECT: &str = "Rotation has no effect on this platform";
/// PROPOSAL (E-BRIGHT-2): shown when a positive custom brightness is asked for, since there is no backlight control.
pub const CUSTOM_BRIGHTNESS_UNAVAILABLE: &str = "Custom brightness is not available on this display";
/// The accessible name and tooltip of the top bar overflow button (the Kotlin content description, in English).
pub const OVERFLOW_MENU: &str = "More";
/// PROPOSAL (E-SAVE-2): the suffix a saved page gets when its file name is taken, as in ` (2)`, ` (3)`.
pub fn name_collision_suffix(n: u32) -> String {
    format!(" ({n})")
}

pub const PREF_CATEGORY_READING_MODE: &str = "Reading mode";
pub const PREF_CATEGORY_GENERAL: &str = "General";
pub const CUSTOM_FILTER: &str = "Custom filter";
pub const PREF_CATEGORY_FOR_THIS_SERIES: &str = "For this series";
pub const PAGER_VIEWER: &str = "Paged";
pub const WEBTOON_VIEWER: &str = "Long strip";
pub const PREF_VIEWER_NAV: &str = "Tap zones";
pub const PREF_READ_WITH_TAPPING_INVERTED: &str = "Invert tap zones";
pub const PREF_IMAGE_SCALE_TYPE: &str = "Scale type";
pub const PREF_ZOOM_START: &str = "Zoom start position";
pub const PREF_LANDSCAPE_ZOOM: &str = "Automatically zoom into wide images";
pub const PREF_NAVIGATE_PAN: &str = "Pan wide images";
pub const PREF_DUAL_PAGE_SPLIT: &str = "Split wide pages";
pub const PREF_DUAL_PAGE_INVERT: &str = "Invert split page placement";
pub const PREF_PAGE_ROTATE: &str = "Rotate wide pages to fit";
pub const PREF_PAGE_ROTATE_INVERT: &str = "Flip orientation of rotated wide pages";
pub const PREF_WEBTOON_SIDE_PADDING: &str = "Side padding";
pub const PREF_DOUBLE_TAP_ZOOM: &str = "Double tap to zoom";
pub const PREF_WEBTOON_DISABLE_ZOOM_OUT: &str = "Disable zoom out";
pub const PREF_READER_THEME: &str = "Background color";
pub const PREF_SHOW_PAGE_NUMBER: &str = "Show page number";
pub const PREF_FULLSCREEN: &str = "Fullscreen";
pub const PREF_CUTOUT_SHORT: &str = "Show content in cutout area";
pub const PREF_KEEP_SCREEN_ON: &str = "Keep screen on";
pub const PREF_READ_WITH_LONG_TAP: &str = "Show actions on long tap";
pub const PREF_ALWAYS_SHOW_CHAPTER_TRANSITION: &str = "Always show chapter transition";
pub const PREF_PAGE_TRANSITIONS: &str = "Animate page transitions";
pub const PREF_SKIP_READ_CHAPTERS: &str = "Skip chapters marked read";
pub const PREF_SKIP_FILTERED_CHAPTERS: &str = "Skip filtered chapters";
pub const PREF_SKIP_DUPE_CHAPTERS: &str = "Skip duplicate chapters";
pub const PREF_PRESERVE_READING_POSITION: &str = "Preserve reading position on read chapters";
pub const PREF_READ_WITH_VOLUME_KEYS: &str = "Volume keys";
pub const PREF_READ_WITH_VOLUME_KEYS_INVERTED: &str = "Invert volume keys";
/// Under the volume key rows: the desktop can take the media keys before the reader sees them (E-25 caveat, the
/// catalogue's drop note). PageUp, PageDown and Space always page, so nothing is lost by it.
pub const VOLUME_KEYS_MEDIA_CAVEAT: &str = "The desktop can take the volume keys before the reader gets them. Page Up, Page Down and Space always turn pages.";
pub const PREF_DOUBLE_TAP_ANIM_SPEED: &str = "Double tap animation speed";
pub const DOUBLE_TAP_ANIM_SPEED_0: &str = "No animation";
pub const DOUBLE_TAP_ANIM_SPEED_NORMAL: &str = "Normal";
pub const DOUBLE_TAP_ANIM_SPEED_FAST: &str = "Fast";
pub const PREF_HIDE_THRESHOLD: &str = "Sensitivity for hiding menu on scroll";
pub const PREF_HIGHEST: &str = "Highest";
pub const PREF_HIGH: &str = "High";
pub const PREF_LOW: &str = "Low";
pub const PREF_LOWEST: &str = "Lowest";
/// Kurobayomi only (T-43a-039): the switch of the crash restore.
pub const PREF_RESTORE_AFTER_CRASH: &str = "Restore the chapter and page after a crash";
pub const PREF_FLASH_PAGE: &str = "Flash on page change";
pub const PREF_FLASH_DURATION: &str = "Flash duration";
pub const PREF_FLASH_PAGE_INTERVAL: &str = "Flash every";
pub const PREF_FLASH_WITH: &str = "Flash with";
pub const PREF_FLASH_STYLE_BLACK: &str = "Black";
pub const PREF_FLASH_STYLE_WHITE: &str = "White";
pub const PREF_FLASH_STYLE_WHITE_BLACK: &str = "White and Black";
pub const BLACK_BACKGROUND: &str = "Black";
pub const GRAY_BACKGROUND: &str = "Gray";
pub const WHITE_BACKGROUND: &str = "White";
pub const AUTOMATIC_BACKGROUND: &str = "Auto";
pub const PREF_CUSTOM_BRIGHTNESS: &str = "Custom brightness";
pub const PREF_CUSTOM_COLOR_FILTER: &str = "Custom color filter";
// The four colour labels were not re-read from strings.xml; verify before release (T-43a-061).
pub const COLOR_FILTER_R_VALUE: &str = "Red";
pub const COLOR_FILTER_G_VALUE: &str = "Green";
pub const COLOR_FILTER_B_VALUE: &str = "Blue";
pub const COLOR_FILTER_A_VALUE: &str = "Alpha";
pub const PREF_COLOR_FILTER_MODE: &str = "Color filter blend mode";
pub const FILTER_MODE_MULTIPLY: &str = "Multiply";
pub const FILTER_MODE_SCREEN: &str = "Screen";
pub const FILTER_MODE_OVERLAY: &str = "Overlay";
pub const FILTER_MODE_LIGHTEN: &str = "Dodge / Lighten";
pub const FILTER_MODE_DARKEN: &str = "Burn / Darken";
pub const PREF_GRAYSCALE: &str = "Grayscale";
pub const PREF_INVERTED_COLORS: &str = "Inverted";
pub const TAPPING_INVERTED_NONE: &str = "None";
pub const TAPPING_INVERTED_HORIZONTAL: &str = "Horizontal";
pub const TAPPING_INVERTED_VERTICAL: &str = "Vertical";
pub const TAPPING_INVERTED_BOTH: &str = "Both";
pub const L_NAV: &str = "L shaped";
pub const KINDLISH_NAV: &str = "Kindle-ish";
pub const EDGE_NAV: &str = "Edge";
pub const RIGHT_AND_LEFT_NAV: &str = "Right and Left";
pub const DISABLED_NAV: &str = "Disabled";
pub const SCALE_TYPE_FIT_SCREEN: &str = "Fit screen";
pub const SCALE_TYPE_STRETCH: &str = "Stretch";
pub const SCALE_TYPE_FIT_WIDTH: &str = "Fit width";
pub const SCALE_TYPE_FIT_HEIGHT: &str = "Fit height";
pub const SCALE_TYPE_ORIGINAL_SIZE: &str = "Original size";
pub const SCALE_TYPE_SMART_FIT: &str = "Smart fit";
pub const ZOOM_START_AUTOMATIC: &str = "Automatic";
pub const ZOOM_START_LEFT: &str = "Left";
pub const ZOOM_START_RIGHT: &str = "Right";
pub const ZOOM_START_CENTER: &str = "Center";

// The four paged names match the base `strings.xml` of the upstream app (G-34 closed).
pub const LEFT_TO_RIGHT_VIEWER: &str = "Paged (left to right)";
pub const RIGHT_TO_LEFT_VIEWER: &str = "Paged (right to left)";
pub const VERTICAL_VIEWER: &str = "Paged (vertical)";
pub const VERTICAL_PLUS_VIEWER: &str = "Long strip with gaps";

/// `Source not installed: %1$s` (`source_not_installed`).
pub fn source_not_installed(source: &str) -> String {
    format!("Source not installed: {source}")
}

/// `Failed to load pages: %s`, the initial load error card (T-43a-024).
pub fn failed_to_load_pages(message: &str) -> String {
    format!("Failed to load pages: {message}")
}

/// `Requested chapter of id %d not found in chapter list` (`ReaderViewModel.kt:158-159`).
pub fn requested_chapter_not_found(id: i64) -> String {
    format!("Requested chapter of id {id} not found in chapter list")
}

/// `missing_chapters_warning` (plural): one and other forms.
pub fn missing_chapters_warning(count: i32) -> String {
    if count == 1 {
        format!("Skipping {count} chapter, either the source is missing it or it has been filtered out")
    } else {
        format!("Skipping {count} chapters, either the source is missing them or they have been filtered out")
    }
}

/// `pref_pages` (plural): "1 page" or "%1$s pages".
pub fn pages(count: i32) -> String {
    if count == 1 { "1 page".to_owned() } else { format!("{count} pages") }
}

/// `pref_flash_duration_summary`: "%1$s ms".
pub fn flash_duration_summary(millis: i32) -> String {
    format!("{millis} ms")
}

/// `share_page_info`: "manga: chapter, page n".
pub fn share_page_info(manga: &str, chapter: &str, page: i32) -> String {
    format!("{manga}: {chapter}, page {page}")
}

/// `On` or `Off`, shown by the crop toast and the reading-mode toast.
pub fn on_off(value: bool) -> &'static str {
    if value { ON } else { OFF }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_reading_mode_names_match_the_base_strings() {
        assert_eq!(VERTICAL_PLUS_VIEWER, "Long strip with gaps");
        assert_eq!(WEBTOON_VIEWER, "Long strip");
    }

    #[test]
    fn proposal_messages_match_the_spec_text() {
        assert_eq!(CHAPTER_NOT_DOWNLOADED, "The chapter is not downloaded. Turn off downloaded-only mode or download it first.");
        assert_eq!(UNKNOWN_ERROR, "Unknown error");
        assert_eq!(ROTATION_NO_EFFECT, "Rotation has no effect on this platform");
        assert_eq!(CUSTOM_BRIGHTNESS_UNAVAILABLE, "Custom brightness is not available on this display");
        assert_eq!(name_collision_suffix(2), " (2)");
        assert_eq!(name_collision_suffix(10), " (10)");
    }

    #[test]
    fn transition_and_error_strings_match_the_upstream_text() {
        assert_eq!(PAGE_LIST_EMPTY_ERROR, "No pages found");
        assert_eq!(TRANSITION_NO_NEXT, "There's no next chapter");
        assert_eq!(source_not_installed("Foo"), "Source not installed: Foo");
        assert_eq!(failed_to_load_pages("No pages found"), "Failed to load pages: No pages found");
    }

    #[test]
    fn missing_chapters_warning_has_one_and_other_forms() {
        assert!(missing_chapters_warning(1).starts_with("Skipping 1 chapter,"));
        assert!(missing_chapters_warning(3).starts_with("Skipping 3 chapters,"));
    }

    #[test]
    fn plural_and_format_strings_follow_the_spec() {
        assert_eq!(pages(1), "1 page");
        assert_eq!(pages(4), "4 pages");
        assert_eq!(flash_duration_summary(300), "300 ms");
        assert_eq!(share_page_info("M", "C 1", 2), "M: C 1, page 2");
    }

    #[test]
    fn on_off_is_always_one_of_two_words() {
        assert_eq!(on_off(true), "On");
        assert_eq!(on_off(false), "Off");
    }
}
