//! Shared-axis page transitions as GTK `Stack` transitions (Kotlin `shared_axis_x_*` animations, T-53a-051).
//!
//! Opening the reader or the web view pushes (`SlideLeft`), closing pops (`SlideRight`), both 300 ms. The alpha part of the
//! Android animation is not reproduced: GTK cannot fade during a slide (`p1-ui-parity.md` section 3). The player has no
//! transition. With `gtk-enable-animations` off the switch is immediate.

use gtk::StackTransitionType;

/// Translate time of the Android animation, in milliseconds.
pub const DURATION_MS: u32 = 300;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// Opening a page (reader, web view).
    Push,
    /// Closing it again.
    Pop,
}

/// The transition and its length for one move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub kind: StackTransitionType,
    pub duration_ms: u32,
}

/// What a move looks like. With animations off the kind is `None` and the duration 0.
pub fn spec(direction: Direction, animations_enabled: bool) -> Spec {
    if !animations_enabled {
        return Spec { kind: StackTransitionType::None, duration_ms: 0 };
    }
    let kind = match direction {
        Direction::Push => StackTransitionType::SlideLeft,
        Direction::Pop => StackTransitionType::SlideRight,
    };
    Spec { kind, duration_ms: DURATION_MS }
}

/// The desktop's animation setting. True when there is no settings object.
pub fn animations_enabled() -> bool {
    gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations())
}

/// Sets the stack's transition for the next move to `child`. Call it right before `set_visible_child`.
pub fn apply(stack: &gtk::Stack, direction: Direction) {
    let spec = spec(direction, animations_enabled());
    stack.set_transition_type(spec.kind);
    stack.set_transition_duration(spec.duration_ms);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_slides_left_and_pop_slides_right_for_300_ms() {
        assert_eq!(spec(Direction::Push, true), Spec { kind: StackTransitionType::SlideLeft, duration_ms: 300 });
        assert_eq!(spec(Direction::Pop, true), Spec { kind: StackTransitionType::SlideRight, duration_ms: 300 });
    }

    #[test]
    fn with_animations_off_the_switch_is_immediate() {
        // TC-53-42
        for direction in [Direction::Push, Direction::Pop] {
            assert_eq!(spec(direction, false), Spec { kind: StackTransitionType::None, duration_ms: 0 });
        }
    }

    #[test]
    fn apply_sets_the_stack_transition() {
        crate::testing::with_display(|| {
            let stack = gtk::Stack::new();
            apply(&stack, Direction::Push);
            let expected = spec(Direction::Push, animations_enabled());
            assert_eq!(stack.transition_type(), expected.kind);
            assert_eq!(stack.transition_duration(), expected.duration_ms);
        });
    }
}
