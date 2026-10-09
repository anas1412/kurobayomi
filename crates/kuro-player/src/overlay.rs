//! Control overlay visibility and auto-hide (area 46 `PlayerControls.kt` LaunchedEffect and
//! area 45 `PlayerViewModel.showControls`/`hideControls`, behaviour 10; T-46a-001, T-46a-002,
//! T-45b-051, T-45b-052). Pure: the GTK frame loop passes `now_ms` and acts on the answers.
//! Sheets, panels and dialogs are checked by the caller (`hosts::has_overlay`).

/// `pref_player_time_to_disappear` default (`PlayerPreferences.kt:43`).
pub const AUTO_HIDE_MS: u64 = 4000;

/// A press on the video closes an open panel first, unless `pref_allow_gestures_in_panels` is on
/// (GH:141-147, E-46-006). The tap that follows the press then toggles the controls as usual.
pub fn press_closes_panel(panel_open: bool, allow_in_panels: bool) -> bool {
    panel_open && !allow_in_panels
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ControlsModel {
    shown: bool,
    locked: bool,
    paused: bool,
    seeking: bool,
    /// Last trigger: a show, a control click, a resume or the end of a seek drag.
    last_trigger_ms: u64,
    auto_hide_ms: u64,
}

impl ControlsModel {
    /// `hide_controls` is `pref_hide_controls`. The controls start unlocked, and playback is treated
    /// as paused until the first `set_paused(false)`.
    pub fn new(hide_controls: bool, auto_hide_ms: u64) -> Self {
        Self {
            shown: !hide_controls,
            locked: false,
            paused: true,
            seeking: false,
            last_trigger_ms: 0,
            auto_hide_ms,
        }
    }

    pub fn shown(&self) -> bool {
        self.shown
    }

    /// `showControls`. Refuses, and changes nothing, while a sheet, panel or dialog is open.
    pub fn show(&mut self, now_ms: u64, overlay_open: bool) -> bool {
        if overlay_open {
            return false;
        }
        self.shown = true;
        self.last_trigger_ms = now_ms;
        true
    }

    pub fn hide(&mut self) {
        self.shown = false;
    }

    /// The surface tap: hides when shown, otherwise shows.
    pub fn toggle(&mut self, now_ms: u64, overlay_open: bool) -> bool {
        if self.shown {
            self.hide();
            true
        } else {
            self.show(now_ms, overlay_open)
        }
    }

    /// Any control click restarts the countdown (`reset_epoch`, PlayerControls.kt:158).
    pub fn control_clicked(&mut self, now_ms: u64) {
        self.last_trigger_ms = now_ms;
    }

    /// `lock_controls` / `unlock_controls`. Not persisted, so a new session starts unlocked.
    pub fn set_locked(&mut self, locked: bool) {
        self.locked = locked;
    }

    pub fn locked(&self) -> bool {
        self.locked
    }

    /// Pausing cancels the countdown. Resuming restarts it with the full delay.
    pub fn set_paused(&mut self, paused: bool, now_ms: u64) {
        if self.paused && !paused {
            self.last_trigger_ms = now_ms;
        }
        self.paused = paused;
    }

    /// An active seek drag cancels the countdown. Ending it restarts the full delay.
    pub fn set_seeking(&mut self, seeking: bool, now_ms: u64) {
        if self.seeking && !seeking {
            self.last_trigger_ms = now_ms;
        }
        self.seeking = seeking;
    }

    /// The full control set is drawn: shown and not locked.
    pub fn chrome_visible(&self) -> bool {
        self.shown && !self.locked
    }

    /// The lock affordance is drawn: locked and the controls are shown.
    pub fn unlock_visible(&self) -> bool {
        self.shown && self.locked
    }

    /// Double tap, long press and drags work only while unlocked.
    pub fn gestures_enabled(&self) -> bool {
        !self.locked
    }

    /// The time at which auto-hide fires, if it is armed: shown, playing and not dragging.
    pub fn auto_hide_due(&self) -> Option<u64> {
        (self.shown && !self.paused && !self.seeking)
            .then(|| self.last_trigger_ms + self.auto_hide_ms)
    }

    /// Hides the controls once the countdown has run out. Returns `true` when it hid them.
    pub fn tick(&mut self, now_ms: u64) -> bool {
        match self.auto_hide_due() {
            Some(due) if now_ms >= due => {
                self.shown = false;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> ControlsModel {
        let mut m = ControlsModel::new(false, AUTO_HIDE_MS);
        m.set_paused(false, 0);
        m
    }

    #[test]
    fn auto_hide_fires_four_seconds_after_the_last_trigger() {
        let mut m = playing();
        assert_eq!(m.auto_hide_due(), Some(4000));
        assert!(!m.tick(3999));
        assert!(m.tick(4000));
        assert!(!m.shown());
    }

    #[test]
    fn a_control_click_restarts_the_countdown() {
        let mut m = playing();
        m.control_clicked(3000);
        assert_eq!(m.auto_hide_due(), Some(7000));
    }

    #[test]
    fn pausing_cancels_and_resuming_restarts_with_the_full_delay() {
        let mut m = playing();
        m.set_paused(true, 1000);
        assert_eq!(m.auto_hide_due(), None);
        assert!(!m.tick(10_000));
        m.set_paused(false, 10_000);
        assert_eq!(m.auto_hide_due(), Some(14_000));
    }

    #[test]
    fn a_seek_drag_holds_the_controls_open() {
        let mut m = playing();
        m.set_seeking(true, 500);
        assert_eq!(m.auto_hide_due(), None);
        m.set_seeking(false, 2000);
        assert_eq!(m.auto_hide_due(), Some(6000));
    }

    #[test]
    fn show_is_refused_while_an_overlay_is_open() {
        let mut m = ControlsModel::new(true, AUTO_HIDE_MS);
        assert!(!m.show(0, true));
        assert!(!m.shown());
        assert!(m.show(0, false));
        assert!(m.shown());
    }

    #[test]
    fn lock_hides_chrome_but_keeps_the_unlock_affordance() {
        let mut m = playing();
        m.set_locked(true);
        assert!(!m.chrome_visible());
        assert!(m.unlock_visible());
        assert!(!m.gestures_enabled());
        m.set_locked(false);
        assert!(m.chrome_visible());
        assert!(!m.unlock_visible());
    }

    #[test]
    fn a_press_closes_an_open_panel_unless_gestures_are_allowed() {
        assert!(press_closes_panel(true, false));
        assert!(!press_closes_panel(true, true));
        assert!(!press_closes_panel(false, false));
    }

    #[test]
    fn a_tap_while_locked_still_toggles_the_controls() {
        let mut m = playing();
        m.set_locked(true);
        assert!(m.toggle(0, false));
        assert!(!m.shown());
        assert!(!m.unlock_visible());
    }

    #[test]
    fn surface_tap_toggles_visibility() {
        let mut m = playing();
        assert!(m.toggle(0, false));
        assert!(!m.shown());
        assert!(m.toggle(100, false));
        assert!(m.shown());
    }
}
