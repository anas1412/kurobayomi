//! Control button primitives (area 46 `ControlsButton.kt`, T-46a-058). Pure: the GTK button reads
//! the sizes and alpha, and calls `click` for each activation.

use crate::overlay::ControlsModel;

/// Icon size of an icon button, in logical pixels.
pub const ICON_SIZE: i32 = 20;
/// Padding on every side of a button, in logical pixels.
pub const PADDING: i32 = 16;
/// Opacity of a disabled button (`DISABLED_ALPHA`, presentation-core).
pub const DISABLED_ALPHA: f32 = 0.38;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonKind {
    Icon,
    Text,
    /// The filled pill: the suggested-action look.
    Filled,
}

impl ButtonKind {
    /// Every kind takes a long press (T-46a-058).
    pub fn accepts_long_press(self) -> bool {
        true
    }

    /// Only the filled pill uses the suggested-action look.
    pub fn suggested_action(self) -> bool {
        self == Self::Filled
    }
}

/// Opacity of a button: full when enabled, `DISABLED_ALPHA` otherwise.
pub fn alpha(enabled: bool) -> f32 {
    if enabled { 1.0 } else { DISABLED_ALPHA }
}

/// Handles a click. A disabled button ignores it and reports `false`. An enabled one restarts the
/// auto-hide countdown and reports `true`.
pub fn click(model: &mut ControlsModel, now_ms: u64, enabled: bool) -> bool {
    if enabled {
        model.control_clicked(now_ms);
    }
    enabled
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::overlay::AUTO_HIDE_MS;

    #[test]
    fn metrics_match_the_design_constants() {
        assert_eq!(ICON_SIZE, 20);
        assert_eq!(PADDING, 16);
        assert_eq!(alpha(true), 1.0);
        assert_eq!(alpha(false), 0.38);
    }

    #[test]
    fn only_the_filled_pill_is_the_suggested_action() {
        assert!(ButtonKind::Filled.suggested_action());
        assert!(!ButtonKind::Icon.suggested_action());
        assert!(!ButtonKind::Text.suggested_action());
        assert!(
            [ButtonKind::Icon, ButtonKind::Text, ButtonKind::Filled]
                .iter()
                .all(|k| k.accepts_long_press())
        );
    }

    #[test]
    fn a_click_restarts_the_countdown_only_when_enabled() {
        let mut m = ControlsModel::new(false, AUTO_HIDE_MS);
        m.set_paused(false, 0);
        assert!(!click(&mut m, 2000, false));
        assert_eq!(m.auto_hide_due(), Some(4000));
        assert!(click(&mut m, 2000, true));
        assert_eq!(m.auto_hide_due(), Some(6000));
    }
}
