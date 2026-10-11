//! Shared-axis page transitions as GTK `Stack` transitions (Kotlin `shared_axis_x_*` animations, T-36a-002, T-53a-051).
//!
//! Every screen change is `SlideLeftRight` for 250 ms (T-36a-002, which is the P0 rule over the 300 ms of T-53a-051):
//! GTK slides left when the new page sits after the old one in the stack, which is a push, and right for a pop. The
//! alpha part of the Android animation is not reproduced: GTK cannot fade during a slide (`p1-ui-parity.md` section 3).
//! The player has no transition. With `gtk-enable-animations` off the switch is immediate.

use gtk::StackTransitionType;

/// Length of a screen slide, in milliseconds.
pub const DURATION_MS: u32 = super::nav::TRANSITION_MS;

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
    // the stack picks the side from the child order, so a pop is the reverse of a push without a second kind
    let _ = direction;
    Spec { kind: StackTransitionType::SlideLeftRight, duration_ms: DURATION_MS }
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
    use gtk::prelude::*;

    use super::*;

    #[test]
    fn push_and_pop_slide_left_right_for_250_ms() {
        for direction in [Direction::Push, Direction::Pop] {
            assert_eq!(spec(direction, true), Spec { kind: StackTransitionType::SlideLeftRight, duration_ms: 250 });
        }
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

    /// T-36a-002 bullet 3 through the window's real navigator: `ui::push` and `ui::pop` set the slide and its length, and
    /// the pop goes to the page before the pushed one in child order, which is what makes GTK slide the other way.
    #[test]
    fn ui_push_and_pop_slide_left_right_by_child_order() {
        crate::testing::with_display(|| {
            let (stack, _win) = crate::testing::nav();
            let (home, detail) = (gtk::Label::new(Some("home")), gtk::Label::new(Some("detail")));
            stack.add_child(&home);
            stack.set_visible_child(&home);
            let expected = spec(Direction::Push, animations_enabled());

            crate::ui::push(&detail);
            assert_eq!(stack.visible_child().as_ref(), Some(detail.upcast_ref()));
            assert_eq!((stack.transition_type(), stack.transition_duration()), (expected.kind, expected.duration_ms));
            assert!(detail.prev_sibling().as_ref() == Some(home.upcast_ref()), "the pushed page sits after the one it covers");

            crate::ui::pop();
            assert_eq!(stack.visible_child().as_ref(), Some(home.upcast_ref()));
            assert_eq!((stack.transition_type(), stack.transition_duration()), (expected.kind, expected.duration_ms));
        });
    }
}
