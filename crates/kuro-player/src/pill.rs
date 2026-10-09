//! Volume and brightness pills and the player update pill (area 46 `VerticalSliders.kt`,
//! `BrightnessOverlay.kt`, `PlayerUpdates.kt`; T-46a-021, T-46a-022, T-46a-023). Pure texts, icons,
//! gauge fractions and the 2 s timer. The GTK layer draws them.

use crate::gauge::{Gauge, gauge, percentage};

/// Pill bar geometry in pixels: 120 high, 54 wide (aspect 0.45).
pub const BAR_HEIGHT: i32 = 120;
pub const BAR_WIDTH: i32 = 54;
/// How long a volume or brightness pill stays up after the last change.
pub const PILL_MS: u64 = 2000;
/// How long the player update pill stays up.
pub const UPDATE_PILL_MS: u64 = 2000;

/// Device volume as a whole percent of `max_steps` (`roundToInt`). Zero maximum gives 0.
pub fn volume_percent(step: i32, max_steps: i32) -> i32 {
    (percentage(step as f32, (0.0, max_steps as f32)) * 100.0).round() as i32
}

/// The device line of the volume pill: a percent when `as_percent`, else the step.
pub fn device_text(step: i32, max_steps: i32, as_percent: bool) -> String {
    if as_percent {
        format!("{}%", volume_percent(step, max_steps))
    } else {
        step.to_string()
    }
}

/// The boost line: empty at 0, `+N` for 1..=1000, `-N` for -100..=-1, else the plain number.
pub fn boost_text(boost: i32) -> String {
    match boost {
        0 => String::new(),
        1..=1000 => format!("+{boost}"),
        -100..=-1 => format!("-{}", -boost),
        _ => boost.to_string(),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeIcon {
    Off,
    Mute,
    Down,
    Up,
}

/// Icon of the volume pill by rounded percent: 0 is off, 1..=30 mute, 31..=60 down, 61..=100 up.
pub fn volume_icon(percent: i32) -> VolumeIcon {
    match percent {
        1..=30 => VolumeIcon::Mute,
        31..=60 => VolumeIcon::Down,
        61..=100 => VolumeIcon::Up,
        _ => VolumeIcon::Off,
    }
}

/// Volume bar: the device fill and the boost overflow up to `boost_cap`. `boost` is mpv volume
/// minus 100, so a negative boost is not drawn as overflow.
pub fn volume_gauge(percent: i32, boost: i32, boost_cap: i32) -> Gauge {
    gauge(
        percent as f32,
        (0.0, 100.0),
        boost.max(0) as f32,
        (0.0, boost_cap.max(0) as f32),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrightnessIcon {
    Night,
    Low,
    Medium,
    High,
}

/// Brightness pill text: the integer of brightness times 100, no percent sign (`-40` is possible).
pub fn brightness_text(brightness: f32) -> String {
    ((brightness * 100.0) as i32).to_string()
}

/// Icon of the brightness pill: night at 0, low up to 0.3, medium up to 0.6, high above.
pub fn brightness_icon(brightness: f32) -> BrightnessIcon {
    let p = percentage(brightness, (0.0, 1.0));
    if p == 0.0 {
        BrightnessIcon::Night
    } else if p <= 0.3 {
        BrightnessIcon::Low
    } else if p <= 0.6 {
        BrightnessIcon::Medium
    } else {
        BrightnessIcon::High
    }
}

/// Brightness bar: fill 0..1, and the error overflow for negative values up to 0.75.
pub fn brightness_gauge(brightness: f32) -> Gauge {
    gauge(brightness, (0.0, 1.0), -brightness, (0.0, 0.75))
}

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
    fn device_text_is_a_percent_or_the_step_and_never_divides_by_zero() {
        assert_eq!(device_text(7, 15, true), "47%");
        assert_eq!(device_text(7, 15, false), "7");
        assert_eq!(device_text(3, 0, true), "0%");
        assert_eq!(volume_percent(15, 15), 100);
    }

    #[test]
    fn boost_text_covers_each_branch() {
        assert_eq!(boost_text(0), "");
        assert_eq!(boost_text(12), "+12");
        assert_eq!(boost_text(1000), "+1000");
        assert_eq!(boost_text(-30), "-30");
        assert_eq!(boost_text(-500), "-500");
    }

    #[test]
    fn volume_icon_thresholds_follow_the_kotlin_order() {
        assert_eq!(volume_icon(0), VolumeIcon::Off);
        assert_eq!(volume_icon(1), VolumeIcon::Mute);
        assert_eq!(volume_icon(30), VolumeIcon::Mute);
        assert_eq!(volume_icon(31), VolumeIcon::Down);
        assert_eq!(volume_icon(60), VolumeIcon::Down);
        assert_eq!(volume_icon(61), VolumeIcon::Up);
        assert_eq!(volume_icon(100), VolumeIcon::Up);
    }

    #[test]
    fn brightness_text_truncates_and_keeps_the_sign() {
        assert_eq!(brightness_text(0.456), "45");
        assert_eq!(brightness_text(-0.4), "-40");
    }

    #[test]
    fn brightness_icon_thresholds() {
        assert_eq!(brightness_icon(0.0), BrightnessIcon::Night);
        assert_eq!(brightness_icon(-0.5), BrightnessIcon::Night);
        assert_eq!(brightness_icon(0.3), BrightnessIcon::Low);
        assert_eq!(brightness_icon(0.31), BrightnessIcon::Medium);
        assert_eq!(brightness_icon(0.6), BrightnessIcon::Medium);
        assert_eq!(brightness_icon(0.61), BrightnessIcon::High);
    }

    #[test]
    fn negative_brightness_is_an_overflow_up_to_three_quarters() {
        let g = brightness_gauge(-0.4);
        assert_eq!(g.fill, 0.0);
        assert!((g.overflow - 0.4 / 0.75).abs() < 1e-6);
        assert_eq!(brightness_gauge(-2.0).overflow, 1.0);
    }

    #[test]
    fn volume_overflow_is_the_boost_only() {
        let g = volume_gauge(100, 15, 30);
        assert_eq!(g.fill, 1.0);
        assert_eq!(g.overflow, 0.5);
        assert_eq!(volume_gauge(50, -20, 30).overflow, 0.0);
    }

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
