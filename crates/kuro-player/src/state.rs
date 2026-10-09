//! Player UI state model (area 45 p1-rust-design section 3.4). Plain data, no GTK. The GTK
//! window owns one `PlayerState` and applies mpv events to it on the main loop.

use crate::custom_buttons::CustomButton;
use crate::enums::{Dialogs, Panels, Sheets};

/// The reason text given to the idle inhibitor while a video plays (T-45a-041).
pub const INHIBIT_REASON: &str = "Playing video";

/// The idle inhibitor action for the current playback state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdleInhibit {
    Inhibit,
    Release,
    Keep,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerState {
    /// Set while the window is closing (behaviour 1.6 / 6.4). Events are ignored then.
    pub is_exiting: bool,
    /// Mirrors the mpv `pause` property. `None` until the first value arrives.
    pub paused: Option<bool>,
    /// Mirrors `seeking` and `paused-for-cache` (`PlayerActivity.kt:709-714`).
    pub is_loading: bool,
    pub is_loading_episode: bool,
    pub is_loading_hosters: bool,
    pub is_loading_tracks: bool,
    /// Seconds. Zero when unknown.
    pub duration: f32,
    /// Seconds.
    pub pos: f32,
    pub playback_speed: f32,
    pub sheet: Sheets,
    pub panel: Panels,
    pub dialog: Dialogs,
    /// Mini-player window visible.
    pub pip_active: bool,
    /// Cookie returned by `gtk::Application::inhibit`, released with `uninhibit`.
    pub keep_awake: Option<u32>,
    pub restore_volume_on_resume: bool,
    /// `controls_locked` (`lock_controls` / `unlock_controls`, T-45b-052). While set, only the
    /// unlock affordance is visible.
    pub controls_locked: bool,
    /// `controlsShown`: the control layer is visible (`toggle_ui`, behaviour 12).
    pub controls_shown: bool,
    /// `_seekBarShown`: the seek bar is visible on its own (`hideSeekBar`/`showSeekBar`, behaviour 10).
    pub seek_bar_shown: bool,
    /// `_dismissSheet`: set by `dismiss_sheet`, cleared by `reset_dismiss_sheet` and by closing.
    pub dismiss_sheet: bool,
    /// `_primaryButton`: the custom button shown beside the controls. `None` hides it.
    pub primary_button: Option<CustomButton>,
    /// `_primaryButtonTitle`: set by `set_button_title` or from the favourite button's name.
    pub primary_button_title: String,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self {
            is_exiting: false,
            paused: None,
            is_loading: false,
            is_loading_episode: false,
            is_loading_hosters: false,
            is_loading_tracks: false,
            duration: 0.0,
            pos: 0.0,
            playback_speed: 1.0,
            sheet: Sheets::None,
            panel: Panels::None,
            dialog: Dialogs::None,
            pip_active: false,
            keep_awake: None,
            restore_volume_on_resume: false,
            controls_locked: false,
            controls_shown: true,
            seek_bar_shown: true,
            dismiss_sheet: false,
            primary_button: None,
            primary_button_title: String::new(),
        }
    }
}

impl PlayerState {
    /// The state at startup. `hide_controls` is the `player_hide_controls` preference: when it is set,
    /// the controls and the seek bar start hidden (behaviour 1.1).
    pub fn with_hide_controls(hide_controls: bool) -> Self {
        Self {
            controls_shown: !hide_controls,
            seek_bar_shown: !hide_controls,
            ..Self::default()
        }
    }

    /// Whether a sheet, a panel or a dialog is open. Chrome stays hidden while one is.
    pub fn overlay_open(&self) -> bool {
        self.sheet != Sheets::None || self.panel != Panels::None || self.dialog != Dialogs::None
    }

    /// `showControls` (behaviour 10): refuses while a sheet, panel or dialog is open. Returns whether
    /// the controls are shown.
    pub fn show_controls(&mut self) -> bool {
        if self.overlay_open() {
            return false;
        }
        self.controls_shown = true;
        true
    }

    /// `hideControls`.
    pub fn hide_controls(&mut self) {
        self.controls_shown = false;
    }

    /// `showSeekBar`: refuses while a sheet is open. Returns whether the seek bar is shown.
    pub fn show_seek_bar(&mut self) -> bool {
        if self.sheet != Sheets::None {
            return false;
        }
        self.seek_bar_shown = true;
        true
    }

    /// `hideSeekBar`.
    pub fn hide_seek_bar(&mut self) {
        self.seek_bar_shown = false;
    }

    /// `dismissSheet` sets the flag and `resetDismissSheet` clears it.
    pub fn dismiss_sheet(&mut self) {
        self.dismiss_sheet = true;
    }

    pub fn reset_dismiss_sheet(&mut self) {
        self.dismiss_sheet = false;
    }

    /// `showSheet` (behaviour 10). A sheet hides the chrome and clears the panel and the dialog.
    /// `Sheets::None` closes the sheet, clears the dismiss flag and shows the controls again.
    pub fn show_sheet(&mut self, sheet: Sheets) {
        self.sheet = sheet;
        if sheet == Sheets::None {
            self.reset_dismiss_sheet();
            self.show_controls();
        } else {
            self.hide_controls();
            self.panel = Panels::None;
            self.dialog = Dialogs::None;
        }
    }

    /// `showPanel` (behaviour 10). A panel hides the chrome and clears the sheet and the dialog.
    pub fn show_panel(&mut self, panel: Panels) {
        self.panel = panel;
        if panel == Panels::None {
            self.show_controls();
        } else {
            self.hide_controls();
            self.sheet = Sheets::None;
            self.dialog = Dialogs::None;
        }
    }

    /// `showDialog` (behaviour 10). A dialog hides the chrome and clears the sheet and the panel.
    pub fn show_dialog(&mut self, dialog: Dialogs) {
        let closing = dialog == Dialogs::None;
        self.dialog = dialog;
        if closing {
            self.show_controls();
        } else {
            self.hide_controls();
            self.sheet = Sheets::None;
            self.panel = Panels::None;
        }
    }

    /// What the idle inhibitor must do now (T-45a-041): hold it while playing, release it on pause or
    /// when the window closes, and keep it as it is otherwise.
    pub fn idle_inhibit(&self) -> IdleInhibit {
        let playing = self.paused == Some(false) && !self.is_exiting;
        match (playing, self.keep_awake.is_some()) {
            (true, false) => IdleInhibit::Inhibit,
            (false, true) => IdleInhibit::Release,
            _ => IdleInhibit::Keep,
        }
    }

    /// `lock_controls`: the controls stay hidden except the unlock affordance (behaviour 1.5).
    pub fn lock_controls(&mut self) {
        self.controls_locked = true;
    }

    /// `unlock_controls`: the controls may show again.
    pub fn unlock_controls(&mut self) {
        self.controls_locked = false;
    }

    /// Whether the full control set may show. It never shows while the controls are locked.
    pub fn full_controls_allowed(&self) -> bool {
        !self.controls_locked
    }

    /// `MPV_EVENT_SEEK` sets the loading flag (`PlayerActivity.kt:759`).
    pub fn on_seek(&mut self) {
        self.is_loading = true;
    }

    /// Applies a boolean mpv property. Returns `false` for properties this state does not own.
    pub fn on_flag(&mut self, name: &str, value: bool) -> bool {
        match name {
            "pause" => self.paused = Some(value),
            // Kotlin assigns the loading flag directly from both properties.
            "paused-for-cache" | "seeking" => self.is_loading = value,
            _ => return false,
        }
        true
    }

    /// Applies a double property. `speed` sets the displayed playback speed (`value.toFloat()`,
    /// PlayerActivity.kt:748). Returns `false` for properties this state does not own.
    pub fn on_double(&mut self, name: &str, value: f64) -> bool {
        match name {
            "speed" => self.playback_speed = value as f32,
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_event_updates_playback_speed() {
        let mut s = PlayerState::default();
        assert!(s.on_double("speed", 1.5));
        assert_eq!(s.playback_speed, 1.5);
        assert!(!s.on_double("duration", 90.0));
        assert_eq!(s.playback_speed, 1.5);
    }

    #[test]
    fn lock_hides_the_full_controls_until_unlocked() {
        let mut s = PlayerState::default();
        assert!(s.full_controls_allowed());
        s.lock_controls();
        assert!(s.controls_locked);
        assert!(!s.full_controls_allowed());
        s.unlock_controls();
        assert!(!s.controls_locked);
        assert!(s.full_controls_allowed());
    }

    #[test]
    fn keep_awake_is_held_while_playing_and_released_on_pause_or_exit() {
        let mut s = PlayerState::default();
        assert_eq!(s.idle_inhibit(), IdleInhibit::Keep);
        s.paused = Some(false);
        assert_eq!(s.idle_inhibit(), IdleInhibit::Inhibit);
        s.keep_awake = Some(7);
        assert_eq!(s.idle_inhibit(), IdleInhibit::Keep);
        s.paused = Some(true);
        assert_eq!(s.idle_inhibit(), IdleInhibit::Release);
        s.paused = Some(false);
        s.is_exiting = true;
        assert_eq!(s.idle_inhibit(), IdleInhibit::Release);
    }

    #[test]
    fn defaults_are_closed_and_unpaused() {
        let s = PlayerState::default();
        assert!(!s.is_exiting);
        assert_eq!(s.paused, None);
        assert_eq!(s.playback_speed, 1.0);
        assert_eq!(s.sheet, Sheets::None);
        assert_eq!(s.panel, Panels::None);
        assert_eq!(s.dialog, Dialogs::None);
        assert_eq!(s.keep_awake, None);
    }

    #[test]
    fn seek_sets_loading_and_cache_flag_mirrors() {
        let mut s = PlayerState::default();
        s.on_seek();
        assert!(s.is_loading);
        assert!(s.on_flag("paused-for-cache", false));
        assert!(!s.is_loading);
        assert!(s.on_flag("seeking", true));
        assert!(s.is_loading);
    }

    #[test]
    fn pause_flag_updates_paused_and_unknown_is_not_handled() {
        let mut s = PlayerState::default();
        assert!(s.on_flag("pause", true));
        assert_eq!(s.paused, Some(true));
        assert!(!s.on_flag("eof-reached", true));
        assert_eq!(s.paused, Some(true));
    }
}
