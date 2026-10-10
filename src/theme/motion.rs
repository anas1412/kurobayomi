//! Reduced motion (T-49a-057): the GTK `gtk-enable-animations` switch. When it is off, transitions
//! are instant and the progress ring does not spin.
//!
//! CSS transitions are left to GTK, which reads the same switch. The durations that are set from
//! Rust, and the ring, go through this module.

/// Whether GTK animations are on. With no settings object (no display), they are on.
pub fn enabled() -> bool {
    gtk::Settings::default().is_none_or(|s| s.is_gtk_enable_animations())
}

/// `ms` while animations are on, and 0 (instant) when they are off.
pub const fn duration_ms(ms: u32, enabled: bool) -> u32 {
    if enabled { ms } else { 0 }
}

/// The progress ring spins only while animations are on.
pub const fn ring_spins(enabled: bool) -> bool {
    enabled
}

/// Player overlay show and hide transitions (T-49ba-035): every `player_*` transition is 200 ms.
pub const PLAYER_ANIM_MS: u32 = 200;
/// Play and pause crossfade in the player (T-49ba-024, `anim_play_to_pause`).
pub const PLAY_PAUSE_CROSSFADE_MS: u32 = 300;
/// Tab-enter crossfades between a static icon and its end state (T-49ba-022).
pub const TAB_CROSSFADE_MS: u32 = 300;
/// Browse icon rotation, 0 to 180 degrees (T-49ba-022, `anim_browse_enter`).
pub const BROWSE_ROTATE_MS: u32 = 300;
/// History icon rotation, 360 to 0 degrees (T-49ba-022, `anim_history_enter`).
pub const HISTORY_ROTATE_MS: u32 = 500;
/// Splash exit crossfade (T-49ba-020, `SPLASH_EXIT_ANIM_DURATION`).
pub const SPLASH_EXIT_MS: u32 = 400;

/// Watches `gtk-enable-animations` and calls `on_change` with the new value (T-49ba-046). Does
/// nothing when there is no settings object (no display).
pub fn watch<F: Fn(bool) + 'static>(on_change: F) {
    if let Some(settings) = gtk::Settings::default() {
        settings.connect_gtk_enable_animations_notify(move |s| on_change(s.is_gtk_enable_animations()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transitions_keep_their_duration_when_animations_are_on() {
        assert_eq!(duration_ms(350, true), 350);
        assert_eq!(duration_ms(0, true), 0);
    }

    #[test]
    fn transitions_are_instant_when_animations_are_off() {
        assert_eq!(duration_ms(350, false), 0);
        assert_eq!(duration_ms(500, false), 0);
    }

    #[test]
    fn the_ring_stops_spinning_with_animations() {
        assert!(ring_spins(true));
        assert!(!ring_spins(false));
    }
}
