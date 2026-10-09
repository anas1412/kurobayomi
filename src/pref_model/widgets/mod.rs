//! Shared row and dialog pieces. Numbers come from Aniyomi's `Constants.kt` and the widget files,
//! converted at scale 1 (GTK scales them with the desktop factor).

pub mod app_theme;
pub mod dialog;
pub mod edit_text;
pub mod group;
pub mod info;
pub mod list;
pub mod multi_select;
pub mod row;
pub mod slider;
pub mod switch;
pub mod text;
pub mod theme_mode;

pub const PREF_HORIZONTAL_PADDING: i32 = 16;
pub const PREF_VERTICAL_PADDING: i32 = 16;
pub const TRAILING_WIDGET_BUFFER: i32 = 16;
pub const PREF_MIN_HEIGHT: i32 = 56;
pub const GROUP_SPACER: i32 = 12;
pub const GROUP_HEADER_TOP: i32 = 14;
pub const GROUP_HEADER_BOTTOM: i32 = 8;
pub const ICON_START_MARGIN: i32 = 16;
pub const ICON_END_MARGIN: i32 = 8;
pub const SUBTITLE_MAX_LINES: i32 = 10;
pub const TITLE_MAX_LINES: i32 = 2;
pub const TRI_STATE_ROW_MIN: i32 = 48;
pub const CHECK_ROW_MIN: i32 = 48;
pub const THEME_CARD_WIDTH: i32 = 114;
pub const DIALOG_WIDTH: i32 = 360;
pub const PAGE_MAX_WIDTH: i32 = 720;
pub const SECONDARY_ALPHA: f64 = 0.78;
pub const DISABLED_ALPHA: f64 = 0.38;
pub const FLASH_DELAY_MS: u64 = 600;
pub const FLASH_STEP_MS: u64 = 200;
pub const FLASH_TOGGLES: u32 = 10;
pub const FLASH_TOTAL_MS: u64 = 3000;
pub const HIGHLIGHT_SCROLL_DELAY_MS: u64 = 500;
pub const SLIDER_ROW_PADDING: i32 = 16;
pub const TRACKER_CHECK_COLOR: &str = "#4CAF50";
pub const TRACKER_CHECK_SIZE: i32 = 32;

pub const HELP_URL: &str = "https://aniyomi.org/docs/guides/troubleshooting/";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constants_match_the_aniyomi_values() {
        assert_eq!(PREF_HORIZONTAL_PADDING, 16);
        assert_eq!(PREF_VERTICAL_PADDING, 16);
        assert_eq!(TRAILING_WIDGET_BUFFER, 16);
        assert_eq!(PREF_MIN_HEIGHT, 56);
        assert_eq!(GROUP_SPACER, 12);
        assert_eq!((GROUP_HEADER_TOP, GROUP_HEADER_BOTTOM), (14, 8));
        assert_eq!(SUBTITLE_MAX_LINES, 10);
        assert_eq!(TITLE_MAX_LINES, 2);
        assert_eq!(TRI_STATE_ROW_MIN, 48);
        assert_eq!(THEME_CARD_WIDTH, 114);
        assert_eq!(SECONDARY_ALPHA, 0.78);
        assert_eq!(DISABLED_ALPHA, 0.38);
        assert_eq!((FLASH_DELAY_MS, FLASH_STEP_MS, FLASH_TOGGLES, FLASH_TOTAL_MS), (600, 200, 10, 3000));
        assert_eq!(HIGHLIGHT_SCROLL_DELAY_MS, 500);
        assert_eq!(TRACKER_CHECK_COLOR, "#4CAF50");
        assert_eq!(TRACKER_CHECK_SIZE, 32);
    }

    #[test]
    fn help_url_and_restore_title_are_the_exact_strings() {
        assert_eq!(HELP_URL, "https://aniyomi.org/docs/guides/troubleshooting/");
        assert_eq!(super::super::strings::label_backup(), "Backup and restore");
    }
}
