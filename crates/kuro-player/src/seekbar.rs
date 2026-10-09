//! Seekbar visibility (area 46 `PlayerControls.kt` and `VideoPlayerViewModel.showSeekBar`, T-46a-012).
//! The seekbar can show on its own when the controls are hidden: at start, on a gesture seek when
//! `pref_show_seekbar` is on, and it hides again after a double-tap clear or a horizontal drag end.
//! Pure: the GTK layer reads `visible` and feeds the events in. Time is in milliseconds.

/// Delay between the end of a double-tap feedback and the seekbar hiding, in milliseconds.
pub const DOUBLE_TAP_HIDE_MS: u64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeekbarVisibility {
    controls_shown: bool,
    seekbar_shown: bool,
    locked: bool,
    sheet_open: bool,
    /// `pref_show_seekbar`: a gesture seek shows the seekbar.
    show_on_seek: bool,
    /// Pending hide from a double-tap clear.
    hide_at_ms: Option<u64>,
}

impl SeekbarVisibility {
    /// The seekbar starts shown unless `pref_hide_controls` is on.
    pub fn new(hide_controls: bool, show_on_seek: bool) -> Self {
        Self {
            controls_shown: false,
            seekbar_shown: !hide_controls,
            locked: false,
            sheet_open: false,
            show_on_seek,
            hide_at_ms: None,
        }
    }

    /// Shown when (controls shown or seekbar shown) and the controls are not locked.
    pub fn visible(&self) -> bool {
        (self.controls_shown || self.seekbar_shown) && !self.locked
    }

    pub fn set_controls_shown(&mut self, shown: bool) {
        self.controls_shown = shown;
    }

    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn set_sheet_open(&mut self, open: bool) {
        self.sheet_open = open;
    }

    /// `showSeekBar()`: a no-op while a sheet is open. Cancels a pending hide, because a new seek
    /// restarts the feedback timer.
    pub fn show_seekbar(&mut self) {
        if self.sheet_open {
            return;
        }
        self.seekbar_shown = true;
        self.hide_at_ms = None;
    }

    /// A gesture seek: shows the seekbar only when `pref_show_seekbar` is on.
    pub fn gesture_seek(&mut self) {
        if self.show_on_seek {
            self.show_seekbar();
        }
    }

    /// The double-tap feedback was cleared. The seekbar hides `DOUBLE_TAP_HIDE_MS` later; call
    /// `tick` to apply it.
    pub fn double_tap_cleared(&mut self, now_ms: u64) {
        self.hide_at_ms = Some(now_ms + DOUBLE_TAP_HIDE_MS);
    }

    /// Applies a pending hide that is due.
    pub fn tick(&mut self, now_ms: u64) {
        if self.hide_at_ms.is_some_and(|at| at <= now_ms) {
            self.hide_seekbar();
        }
    }

    /// A horizontal drag ended: the seekbar hides at once.
    pub fn horizontal_drag_ended(&mut self) {
        self.hide_seekbar();
    }

    fn hide_seekbar(&mut self) {
        self.seekbar_shown = false;
        self.hide_at_ms = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_shown_unless_controls_are_hidden_by_pref() {
        assert!(SeekbarVisibility::new(false, false).visible());
        assert!(!SeekbarVisibility::new(true, false).visible());
    }

    #[test]
    fn locked_controls_hide_the_seekbar() {
        let mut bar = SeekbarVisibility::new(false, false);
        bar.set_locked(true);
        assert!(!bar.visible());
        bar.set_locked(false);
        assert!(bar.visible());
    }

    #[test]
    fn controls_alone_also_show_it() {
        let mut bar = SeekbarVisibility::new(true, false);
        bar.set_controls_shown(true);
        assert!(bar.visible());
    }

    #[test]
    fn gesture_seek_shows_only_with_the_pref() {
        let mut off = SeekbarVisibility::new(true, false);
        off.gesture_seek();
        assert!(!off.visible());
        let mut on = SeekbarVisibility::new(true, true);
        on.gesture_seek();
        assert!(on.visible());
    }

    #[test]
    fn double_tap_clear_hides_after_one_hundred_ms() {
        let mut bar = SeekbarVisibility::new(false, true);
        bar.double_tap_cleared(1000);
        bar.tick(1099);
        assert!(bar.visible());
        bar.tick(1100);
        assert!(!bar.visible());
    }

    #[test]
    fn horizontal_drag_end_hides_at_once() {
        let mut bar = SeekbarVisibility::new(false, true);
        bar.horizontal_drag_ended();
        assert!(!bar.visible());
    }

    #[test]
    fn show_is_a_no_op_while_a_sheet_is_open() {
        let mut bar = SeekbarVisibility::new(true, true);
        bar.set_sheet_open(true);
        bar.show_seekbar();
        assert!(!bar.visible());
        bar.set_sheet_open(false);
        bar.show_seekbar();
        assert!(bar.visible());
    }

    #[test]
    fn a_new_seek_cancels_the_pending_hide() {
        let mut bar = SeekbarVisibility::new(true, true);
        bar.gesture_seek();
        bar.double_tap_cleared(0);
        bar.gesture_seek();
        bar.tick(500);
        assert!(bar.visible());
    }
}
