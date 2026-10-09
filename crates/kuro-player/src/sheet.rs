//! Sheet and panel geometry and dismissal rules (area 46 `PlayerSheet.kt` and `PlayerPanels.kt`;
//! T-46a-029, T-46b-001, T-46a-059). Pure: the GTK sheet reads these values and calls
//! `Dismissal::take` before it runs `on_close`. Lengths are logical pixels. The reveal duration is
//! `anim::SHEET_MS`.

use crate::anim::{LINEAR_OUT_SLOW_IN, SHEET_MS, Tween};

/// Sheet width in portrait-sized windows.
pub const SHEET_WIDTH: i32 = 420;
/// Sheet width when the window is at least `WIDE_FROM_WINDOW` wide (the landscape width).
pub const SHEET_WIDE_WIDTH: i32 = 720;
/// Window width from which the wide sheet is used (T-46b-001 decision for "landscape").
pub const WIDE_FROM_WINDOW: i32 = 900;
/// The sheet never grows past this share of the player height.
pub const SHEET_MAX_HEIGHT_FRACTION: f32 = 0.95;
/// Scrim black alpha behind an open sheet.
pub const SCRIM_ALPHA: f32 = 0.5;
/// A drag down farther than this dismisses the sheet.
pub const DRAG_DISMISS_PX: f32 = 56.0;
/// A downward fling faster than this (px per second) dismisses the sheet.
pub const DRAG_FLING_PX_PER_S: f32 = 125.0;
/// Panel width (`PlayerPanels.kt`).
pub const PANEL_WIDTH: i32 = 420;
/// Panels start at this share of the available height.
pub const PANEL_TOP_FRACTION: f32 = 0.8;

/// Sheet width for a window `window_width` wide.
pub fn sheet_width(window_width: i32) -> i32 {
    if window_width >= WIDE_FROM_WINDOW {
        SHEET_WIDE_WIDTH
    } else {
        SHEET_WIDTH
    }
}

/// Largest sheet height for a player `player_height` tall.
pub fn sheet_max_height(player_height: i32) -> i32 {
    (player_height as f32 * SHEET_MAX_HEIGHT_FRACTION) as i32
}

/// Top edge of a panel in an area `available_height` tall.
pub fn panel_top(available_height: i32) -> i32 {
    (available_height as f32 * PANEL_TOP_FRACTION) as i32
}

/// Whether a drag of `drag_px` downward, ending at `velocity_px_per_s`, dismisses the sheet.
pub fn drag_dismisses(drag_px: f32, velocity_px_per_s: f32) -> bool {
    drag_px > DRAG_DISMISS_PX || velocity_px_per_s > DRAG_FLING_PX_PER_S
}

/// A click on the scrim closes the sheet only once it is fully open (not while it animates).
pub fn scrim_click_closes(fully_open: bool) -> bool {
    fully_open
}

/// Open animation of a sheet and its scrim (350 ms, `anim::SHEET_MS`). The scrim alpha and the
/// fully-open state both read the same tween, so a scrim click during the animation is ignored
/// (T-46a-029, T-46a-059).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SheetAnim {
    tween: Tween,
}

impl SheetAnim {
    /// The sheet started to open at `now_ms`.
    pub fn open(now_ms: u64) -> Self {
        Self {
            tween: Tween {
                from: 0.0,
                to: 1.0,
                start_ms: now_ms,
                duration_ms: SHEET_MS,
                curve: LINEAR_OUT_SLOW_IN,
            },
        }
    }

    /// Reveal progress, 0 to 1.
    pub fn progress(&self, now_ms: u64) -> f32 {
        self.tween.value_at(now_ms)
    }

    /// Scrim black alpha: `SCRIM_ALPHA` once open.
    pub fn scrim_alpha(&self, now_ms: u64) -> f32 {
        SCRIM_ALPHA * self.progress(now_ms)
    }

    pub fn fully_open(&self, now_ms: u64) -> bool {
        self.tween.finished(now_ms)
    }

    /// Whether a scrim click at `now_ms` closes the sheet.
    pub fn scrim_click_closes(&self, now_ms: u64) -> bool {
        scrim_click_closes(self.fully_open(now_ms))
    }
}

/// Guards `on_close` so one open runs it once, even when a drag and a dismiss event both fire
/// (E-46-064).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Dismissal {
    armed: bool,
}

impl Dismissal {
    /// A sheet opened: the next dismissal is allowed.
    pub fn open(&mut self) {
        self.armed = true;
    }

    /// Returns `true` once per open. Later calls return `false` until the next `open`.
    pub fn take(&mut self) -> bool {
        std::mem::replace(&mut self.armed, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheet_width_is_wide_only_from_nine_hundred_pixels() {
        assert_eq!(sheet_width(899), 420);
        assert_eq!(sheet_width(900), 720);
    }

    #[test]
    fn sheet_height_and_panel_top_are_fractions_of_the_player() {
        assert_eq!(sheet_max_height(1000), 950);
        assert_eq!(panel_top(1000), 800);
    }

    #[test]
    fn drag_dismisses_past_56_px_or_a_fast_fling() {
        assert!(!drag_dismisses(56.0, 0.0));
        assert!(drag_dismisses(56.5, 0.0));
        assert!(!drag_dismisses(10.0, 125.0));
        assert!(drag_dismisses(10.0, 125.5));
    }

    #[test]
    fn scrim_click_needs_a_fully_open_sheet() {
        assert!(!scrim_click_closes(false));
        assert!(scrim_click_closes(true));
    }

    #[test]
    fn the_scrim_fades_to_half_alpha_and_ignores_clicks_until_the_sheet_is_open() {
        let a = SheetAnim::open(1000);
        assert_eq!(a.scrim_alpha(1000), 0.0);
        assert!(a.scrim_alpha(1100) > 0.0 && a.scrim_alpha(1100) < 0.5);
        assert!(!a.scrim_click_closes(1349));
        assert!(a.scrim_click_closes(1350));
        assert_eq!(a.scrim_alpha(1350), 0.5);
    }

    #[test]
    fn dismissal_runs_once_per_open() {
        let mut d = Dismissal::default();
        assert!(!d.take());
        d.open();
        assert!(d.take());
        assert!(!d.take());
        d.open();
        assert!(d.take());
    }
}
