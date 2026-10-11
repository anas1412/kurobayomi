//! The player update pill (area 46 `PlayerUpdates.kt`; T-46a-023). Pure texts and the 2 s timer.
//! The GTK layer draws it.

/// How long the player update pill stays up.
pub const UPDATE_PILL_MS: u64 = 2000;

/// What the player update pill can show (`PlayerUpdates`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayerUpdate {
    None,
    /// Double speed. Its pill is commented out in the Kotlin, so it never starts the timer.
    DoubleSpeed,
    /// The live aspect name, read when drawn.
    AspectRatio,
    Text(String),
}

/// The update pill's state and its 2 s timer, in milliseconds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdatePill {
    current: PlayerUpdate,
    started_ms: u64,
}

impl Default for UpdatePill {
    fn default() -> Self {
        Self {
            current: PlayerUpdate::None,
            started_ms: 0,
        }
    }
}

impl UpdatePill {
    /// A new update or an aspect change. Restarts the timer. `visible_text` never shows `None` or
    /// `DoubleSpeed`, so those never display.
    pub fn set(&mut self, update: PlayerUpdate, now_ms: u64) {
        self.current = update;
        self.started_ms = now_ms;
    }

    /// Long-press release clears the pill at once.
    pub fn clear(&mut self) {
        self.current = PlayerUpdate::None;
    }

    /// Text to draw at `now_ms`, or `None` when the pill is hidden. `aspect_name` is the live
    /// aspect title, used for `AspectRatio`.
    pub fn visible_text(&self, now_ms: u64, aspect_name: &str) -> Option<String> {
        if now_ms.saturating_sub(self.started_ms) >= UPDATE_PILL_MS {
            return None;
        }
        match &self.current {
            PlayerUpdate::None | PlayerUpdate::DoubleSpeed => None,
            PlayerUpdate::AspectRatio => Some(aspect_name.to_owned()),
            PlayerUpdate::Text(text) => Some(text.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_pill_shows_for_two_seconds_and_restarts() {
        let mut pill = UpdatePill::default();
        pill.set(PlayerUpdate::Text("Saved".into()), 1000);
        assert_eq!(pill.visible_text(2999, "Fit").as_deref(), Some("Saved"));
        assert_eq!(pill.visible_text(3000, "Fit"), None);
        pill.set(PlayerUpdate::AspectRatio, 2500);
        assert_eq!(
            pill.visible_text(4499, "Stretch").as_deref(),
            Some("Stretch")
        );
        assert_eq!(pill.visible_text(4500, "Stretch"), None);
    }

    #[test]
    fn double_speed_and_none_never_show() {
        let mut pill = UpdatePill::default();
        pill.set(PlayerUpdate::DoubleSpeed, 0);
        assert_eq!(pill.visible_text(10, "Fit"), None);
        pill.set(PlayerUpdate::Text("x".into()), 0);
        pill.clear();
        assert_eq!(pill.visible_text(10, "Fit"), None);
    }
}
