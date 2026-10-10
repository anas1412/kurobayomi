//! Breakpoints and sizes of the Upcoming page (T-42b-030, T-42b-031, T-42b-032). Pure: the page reads these numbers.

use kuro_platform::links::URL_HELP_UPCOMING;

/// Row height of an upcoming entry, in pixels.
pub const ROW_HEIGHT: i32 = 96;
/// Vertical padding of a row, so the cover is the row height less twice this.
pub const ROW_PADDING: i32 = 8;
/// Width over height of a book cover (`ItemCover.Book`).
const BOOK_RATIO: f64 = 2.0 / 3.0;

/// The size of the cover of a row: the full height inside the padding, at the book ratio.
pub fn cover_size() -> (i32, i32) {
    let height = ROW_HEIGHT - 2 * ROW_PADDING;
    ((f64::from(height) * BOOK_RATIO).round() as i32, height)
}

/// The content width from which the calendar and the list sit side by side.
pub const TWO_PANEL_MIN_WIDTH: i32 = 720;
/// The calendar grid stops growing above this width, when it is in the medium band.
pub const CALENDAR_MAX_WIDTH: i32 = 360;

/// Two panels from 720 px of content width; one column below.
pub fn two_panels(content_width: i32) -> bool {
    content_width >= TWO_PANEL_MIN_WIDTH
}

/// The calendar's maximum width: 360 px when the content is wider than 600 and at most 840, else unconstrained.
pub fn calendar_max_width(content_width: i32) -> Option<i32> {
    (content_width > 600 && content_width <= 840).then_some(CALENDAR_MAX_WIDTH)
}

/// How the page is laid out at one content width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shape {
    /// The calendar and the list sit side by side.
    pub two_panels: bool,
    /// The calendar's maximum width, when it has one.
    pub calendar_cap: Option<i32>,
}

/// The layout for a content width in pixels.
pub fn shape(content_width: i32) -> Shape {
    Shape { two_panels: two_panels(content_width), calendar_cap: calendar_max_width(content_width) }
}

/// Where the divider of the two panels starts: the calendar's cap when it has one, else half the width. The user drags
/// it from there.
pub fn divider_start(content_width: i32) -> i32 {
    calendar_max_width(content_width).unwrap_or(content_width / 2)
}

/// The help button opens this page in the default browser.
pub fn help_url() -> &'static str {
    URL_HELP_UPCOMING
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_panels_start_at_720_px() {
        assert!(!two_panels(719));
        assert!(two_panels(720));
    }

    #[test]
    fn calendar_is_capped_only_in_the_medium_band() {
        assert_eq!(calendar_max_width(600), None);
        assert_eq!(calendar_max_width(601), Some(360));
        assert_eq!(calendar_max_width(840), Some(360));
        assert_eq!(calendar_max_width(841), None);
    }

    #[test]
    fn rows_are_96_px_and_help_points_at_the_upcoming_guide() {
        assert_eq!(ROW_HEIGHT, 96);
        assert_eq!(help_url(), "https://aniyomi.org/docs/faq/updates/upcoming");
    }

    #[test]
    fn the_cover_is_a_book_inside_the_row_padding() {
        let (width, height) = cover_size();
        assert_eq!(height, 80);
        assert_eq!(width, 53, "two thirds of the height, rounded");
    }

    #[test]
    fn the_shape_follows_the_breakpoints() {
        assert_eq!(shape(500), Shape { two_panels: false, calendar_cap: None });
        assert_eq!(shape(650), Shape { two_panels: false, calendar_cap: Some(360) });
        assert_eq!(shape(800), Shape { two_panels: true, calendar_cap: Some(360) });
        assert_eq!(shape(1000), Shape { two_panels: true, calendar_cap: None });
    }

    #[test]
    fn the_divider_starts_at_the_cap_or_the_middle() {
        assert_eq!(divider_start(800), 360);
        assert_eq!(divider_start(1000), 500);
    }
}
