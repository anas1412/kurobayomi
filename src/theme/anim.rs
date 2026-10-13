//! Icon and stack animations of the Android resources (T-49ba-022, T-49ba-024, T-49ba-028):
//! the stack crossfade, the icon rotation classes, and the double-tap seek triangle schedule.
//!
//! Rotations are CSS transitions defined in `style.css`, so no per-frame code runs. The durations
//! are in `motion`, and every duration goes through `motion::duration_ms` when it is set from Rust.

#[cfg(test)]
/// The two rotations of the tab-enter icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rotation {
    /// `anim_browse_enter`: 0 to 180 degrees over `BROWSE_ROTATE_MS` when selected.
    Half,
    /// `anim_history_enter`: 360 to 0 degrees over `HISTORY_ROTATE_MS` when selected.
    FullBackSlow,
}

#[cfg(test)]
impl Rotation {
    /// The target class for the icon's state. The transition runs from the current angle to this one.
    pub const fn class(self, selected: bool) -> &'static str {
        match (self, selected) {
            (Rotation::Half, true) => "kb-rot-180",
            (Rotation::Half, false) => "kb-rot-0",
            (Rotation::FullBackSlow, true) => "kb-rot-0-slow",
            (Rotation::FullBackSlow, false) => "kb-rot-360",
        }
    }

    #[cfg(test)]
    /// The transition length of the selected state, in milliseconds.
    pub const fn duration_ms(self) -> u32 {
        match self {
            Rotation::Half => motion::BROWSE_ROTATE_MS,
            Rotation::FullBackSlow => motion::HISTORY_ROTATE_MS,
        }
    }
}

#[cfg(test)]
use super::motion;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browse_rotates_half_a_turn_and_history_back_from_a_full_turn() {
        assert_eq!(Rotation::Half.class(true), "kb-rot-180");
        assert_eq!(Rotation::Half.class(false), "kb-rot-0");
        assert_eq!(Rotation::FullBackSlow.class(true), "kb-rot-0-slow");
        assert_eq!(Rotation::FullBackSlow.class(false), "kb-rot-360");
        assert_eq!(Rotation::Half.duration_ms(), 300);
        assert_eq!(Rotation::FullBackSlow.duration_ms(), 500);
    }

}
