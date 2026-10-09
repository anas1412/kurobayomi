//! Responsive thresholds for small windows (area 46 `PlayerControls.kt` and
//! `BottomLeftPlayerControls.kt`, T-46a-055). Pure widths; the GTK layout applies them. The desktop
//! minimum is 640 by 360 (the proposal in `p1-ui-parity.md`), where every row must still fit.

/// Desktop minimum window size in pixels.
pub const MIN_WIDTH: i32 = 640;
pub const MIN_HEIGHT: i32 = 360;
/// The current chapter pill hides below this window width.
pub const CHAPTER_PILL_MIN_WIDTH: i32 = 560;
/// Sheets are this wide when the window is wider than `SHEET_WIDE_AT`, otherwise `SHEET_NARROW`.
pub const SHEET_NARROW: i32 = 420;
pub const SHEET_WIDE: i32 = 720;
pub const SHEET_WIDE_AT: i32 = 720;
/// Side panels are at most this wide.
pub const PANEL_MAX_WIDTH: i32 = 420;

pub fn chapter_pill_visible(window_width: i32) -> bool {
    window_width >= CHAPTER_PILL_MIN_WIDTH
}

pub fn sheet_width(window_width: i32) -> i32 {
    if window_width > SHEET_WIDE_AT {
        SHEET_WIDE
    } else {
        SHEET_NARROW
    }
}

pub fn panel_width(window_width: i32) -> i32 {
    window_width.min(PANEL_MAX_WIDTH)
}

/// Opacity of the panel surface from `pref_panel_opacity` (0..=100, default 60).
pub fn panel_surface_alpha(opacity_percent: i32) -> f32 {
    opacity_percent.clamp(0, 100) as f32 / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chapter_pill_hides_below_560() {
        assert!(!chapter_pill_visible(559));
        assert!(chapter_pill_visible(560));
        assert!(chapter_pill_visible(MIN_WIDTH));
    }

    #[test]
    fn sheets_are_720_only_when_the_window_is_wider_than_720() {
        assert_eq!(sheet_width(720), 420);
        assert_eq!(sheet_width(721), 720);
        assert_eq!(sheet_width(MIN_WIDTH), 420);
    }

    #[test]
    fn panels_cap_at_420() {
        assert_eq!(panel_width(300), 300);
        assert_eq!(panel_width(1200), 420);
    }

    #[test]
    fn panel_opacity_is_a_percent_clamped_to_range() {
        assert_eq!(panel_surface_alpha(60), 0.6);
        assert_eq!(panel_surface_alpha(150), 1.0);
        assert_eq!(panel_surface_alpha(-5), 0.0);
    }
}
