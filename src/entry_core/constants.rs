//! Shared constants of the entry screens (T-38a-045, T-38a-043). Upstream: `EntryScreenConstants.kt` (the three enums)
//! and `DotSeparatorText.kt` (the separator labels). Pure: the widgets read these values.

/// The width of the left info pane in the two-pane layouts of the manga and anime entry screens.
pub const LEFT_PANE_WIDTH: i32 = 360;
/// The window width from which the entry screens show two panes (info on the left, list on the right).
pub const TWO_PANE_MIN_WIDTH: i32 = 900;
/// The window size assumed before the page has a real one: the app's default size.
pub const FALLBACK_WIDTH: i32 = 1180;

/// True when a window of this width gets the two-pane entry layout.
pub fn two_pane(window_width: i32) -> bool {
    window_width >= TWO_PANE_MIN_WIDTH
}
