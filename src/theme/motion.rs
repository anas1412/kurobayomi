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
