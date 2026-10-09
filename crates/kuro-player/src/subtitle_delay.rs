//! Subtitle delay panel (area 46 `SubtitleDelayPanel.kt`, T-46b-030, T-46b-038). The panel edits
//! the primary delay, the secondary delay or both, plus the subtitle speed. Each change returns the
//! mpv writes; "Set as default" returns the preference writes. Pure: the GTK layer applies them.

use crate::mpv::PropertySource;
use crate::options::{SubtitlePrefs, kotlin_f32, kotlin_f64};
use crate::pref_write::Write;

/// Lowest and highest subtitle speed (T-46b-030, the same range for live and saved values).
pub const SPEED_MIN: f32 = 0.1;
pub const SPEED_MAX: f32 = 10.0;
/// Step of the delay field in milliseconds. The field itself is not clamped.
pub const DELAY_STEP_MS: i64 = 50;

/// Which subtitle the delay applies to (behaviour 11.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelayTarget {
    Primary,
    Secondary,
    Both,
}

impl DelayTarget {
    pub const ALL: [Self; 3] = [Self::Primary, Self::Secondary, Self::Both];

    pub fn title(self) -> &'static str {
        match self {
            Self::Primary => "Primary",
            Self::Secondary => "Secondary",
            Self::Both => "Both",
        }
    }
}

/// Whether the speed of a value is in the saved and live range.
pub fn speed_in_range(speed: f32) -> bool {
    (SPEED_MIN..=SPEED_MAX).contains(&speed)
}

/// The target dropdown is disabled while a tap-sync run is active (E10).
pub fn target_enabled(run_active: bool) -> bool {
    !run_active
}

/// mpv seconds to panel milliseconds, rounded (E7: `round` everywhere).
pub fn ms_from_seconds(seconds: f64) -> i64 {
    (seconds * 1000.0).round() as i64
}

/// Panel milliseconds to the mpv value in seconds (`delay / 1000.0`).
pub fn seconds_text(ms: i64) -> String {
    kotlin_f64(ms as f64 / 1000.0)
}

/// Rounds a speed to three decimals, half to even (`round(x * 1000) / 1000`).
pub fn round_speed(speed: f32) -> f32 {
    (speed * 1000.0).round_ties_even() / 1000.0
}

fn read_f64(source: &impl PropertySource, name: &str) -> Option<f64> {
    source.get_str(name)?.trim().parse().ok()
}

fn read_delay_ms(source: &impl PropertySource, name: &str, fallback: i64) -> i64 {
    read_f64(source, name).map_or(fallback, ms_from_seconds)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubtitleDelayPanel {
    target: DelayTarget,
    /// Primary delay in milliseconds.
    delay_ms: i64,
    /// Secondary delay in milliseconds. In `Both` mode it follows the primary value.
    secondary_ms: i64,
    speed: f32,
}

impl SubtitleDelayPanel {
    /// Opens the panel: values come from mpv, and a missing or unparseable mpv value falls back to
    /// the stored preference (behaviour 11.2, T-46b-004).
    pub fn open(source: &impl PropertySource, prefs: &SubtitlePrefs) -> Self {
        let speed = read_f64(source, "sub-speed").map_or(prefs.speed, |v| v as f32);
        Self {
            target: DelayTarget::Primary,
            delay_ms: read_delay_ms(source, "sub-delay", i64::from(prefs.delay_ms)),
            secondary_ms: read_delay_ms(
                source,
                "secondary-sub-delay",
                i64::from(prefs.secondary_delay_ms),
            ),
            speed,
        }
    }

    pub fn target(&self) -> DelayTarget {
        self.target
    }

    /// The delay the field shows for the current target.
    pub fn delay_ms(&self) -> i64 {
        match self.target {
            DelayTarget::Secondary => self.secondary_ms,
            DelayTarget::Primary | DelayTarget::Both => self.delay_ms,
        }
    }

    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// The speed field is shown only for the primary subtitle.
    pub fn speed_visible(&self) -> bool {
        self.target == DelayTarget::Primary
    }

    /// Changes the target. The values are read again from mpv, as the Kotlin effect does. Nothing
    /// is written on a target change.
    pub fn select_target(
        &mut self,
        target: DelayTarget,
        source: &impl PropertySource,
        prefs: &SubtitlePrefs,
    ) {
        let fresh = Self::open(source, prefs);
        self.target = target;
        self.delay_ms = fresh.delay_ms;
        self.secondary_ms = match target {
            DelayTarget::Both => fresh.delay_ms,
            _ => fresh.secondary_ms,
        };
    }

    /// A change from the delay field or the tap-sync result. Returns the mpv writes. `Both` writes
    /// the primary value to both properties (behaviour 11.2).
    pub fn set_delay(&mut self, ms: i64) -> Vec<Write> {
        match self.target {
            DelayTarget::Primary => {
                self.delay_ms = ms;
                vec![Write::mpv("sub-delay", seconds_text(ms))]
            }
            DelayTarget::Secondary => {
                self.secondary_ms = ms;
                vec![Write::mpv("secondary-sub-delay", seconds_text(ms))]
            }
            DelayTarget::Both => {
                self.delay_ms = ms;
                self.secondary_ms = ms;
                vec![
                    Write::mpv("sub-delay", seconds_text(ms)),
                    Write::mpv("secondary-sub-delay", seconds_text(ms)),
                ]
            }
        }
    }

    /// A change of the speed field. The value is rounded to three decimals and sent live when it
    /// is in 0.1..=10 (E3: the live range is the saved range).
    pub fn set_speed(&mut self, speed: f32) -> Vec<Write> {
        self.speed = round_speed(speed);
        if speed_in_range(self.speed) {
            vec![Write::mpv("sub-speed", kotlin_f32(self.speed))]
        } else {
            Vec::new()
        }
    }

    /// "Set as default": saves the delay of the target (`Both` saves the primary value to both
    /// keys, Secondary saves only the secondary key) and the speed when it is in range (E4, T-46b-038).
    pub fn apply(&self) -> Vec<Write> {
        let mut writes = Vec::new();
        match self.target {
            DelayTarget::Primary => {
                writes.push(Write::pref("pref_subtitles_delay", self.delay_ms.to_string()));
            }
            DelayTarget::Secondary => {
                writes.push(Write::pref(
                    "pref_subtitles_secondary_delay",
                    self.secondary_ms.to_string(),
                ));
            }
            DelayTarget::Both => {
                writes.push(Write::pref("pref_subtitles_delay", self.delay_ms.to_string()));
                writes.push(Write::pref(
                    "pref_subtitles_secondary_delay",
                    self.delay_ms.to_string(),
                ));
            }
        }
        if speed_in_range(self.speed) {
            writes.push(Write::pref("pref_subtitles_speed", self.speed.to_string()));
        }
        writes
    }

    /// Reset: delay and speed back to 0 and 1 in the session, nothing persisted. The mpv writes
    /// cover the target delay and the speed.
    pub fn reset(&mut self) -> Vec<Write> {
        self.delay_ms = 0;
        self.secondary_ms = 0;
        self.speed = 1.0;
        let mut writes = match self.target {
            DelayTarget::Primary => vec![Write::mpv("sub-delay", seconds_text(0))],
            DelayTarget::Secondary => vec![Write::mpv("secondary-sub-delay", seconds_text(0))],
            DelayTarget::Both => vec![
                Write::mpv("sub-delay", seconds_text(0)),
                Write::mpv("secondary-sub-delay", seconds_text(0)),
            ],
        };
        writes.push(Write::mpv("sub-speed", kotlin_f32(1.0)));
        writes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_write::test_support::FakeMpv;

    fn prefs() -> SubtitlePrefs {
        SubtitlePrefs {
            delay_ms: 75,
            secondary_delay_ms: -30,
            speed: 1.5,
            ..SubtitlePrefs::default()
        }
    }

    #[test]
    fn open_rounds_mpv_seconds_and_falls_back_to_the_preference() {
        let mpv = FakeMpv::with(&[("sub-delay", "0.0999999"), ("sub-speed", "2")]);
        let panel = SubtitleDelayPanel::open(&mpv, &prefs());
        assert_eq!(panel.delay_ms(), 100);
        assert_eq!(panel.speed(), 2.0);

        let empty = FakeMpv::with(&[("sub-delay", "soon")]);
        let panel = SubtitleDelayPanel::open(&empty, &prefs());
        assert_eq!(panel.delay_ms(), 75);
        assert_eq!(panel.speed(), 1.5);
        let mut secondary = panel;
        secondary.select_target(DelayTarget::Secondary, &empty, &prefs());
        assert_eq!(secondary.delay_ms(), -30);
    }

    #[test]
    fn primary_change_writes_only_sub_delay() {
        let mut panel = SubtitleDelayPanel::open(&FakeMpv::default(), &prefs());
        assert_eq!(panel.set_delay(350), vec![Write::mpv("sub-delay", "0.35")]);
        assert_eq!(panel.delay_ms(), 350);
        assert_eq!(panel.set_delay(-50), vec![Write::mpv("sub-delay", "-0.05")]);
    }

    #[test]
    fn both_writes_the_primary_value_to_both_properties() {
        let mut panel = SubtitleDelayPanel::open(&FakeMpv::default(), &prefs());
        panel.select_target(DelayTarget::Both, &FakeMpv::default(), &prefs());
        assert_eq!(
            panel.set_delay(200),
            vec![
                Write::mpv("sub-delay", "0.2"),
                Write::mpv("secondary-sub-delay", "0.2"),
            ]
        );
        assert_eq!(panel.delay_ms(), 200);
    }

    #[test]
    fn secondary_change_writes_only_the_secondary_property() {
        let mpv = FakeMpv::with(&[("sub-delay", "1.0"), ("secondary-sub-delay", "0.5")]);
        let mut panel = SubtitleDelayPanel::open(&mpv, &prefs());
        panel.select_target(DelayTarget::Secondary, &mpv, &prefs());
        assert_eq!(panel.delay_ms(), 500);
        assert_eq!(
            panel.set_delay(600),
            vec![Write::mpv("secondary-sub-delay", "0.6")]
        );
        assert!(!panel.speed_visible());
    }

    #[test]
    fn speed_is_live_across_the_whole_saved_range() {
        let mut panel = SubtitleDelayPanel::open(&FakeMpv::default(), &prefs());
        assert_eq!(panel.set_speed(2.5), vec![Write::mpv("sub-speed", "2.5")]);
        assert_eq!(panel.set_speed(10.0), vec![Write::mpv("sub-speed", "10.0")]);
        assert_eq!(panel.set_speed(0.1), vec![Write::mpv("sub-speed", "0.1")]);
        assert!(panel.set_speed(0.05).is_empty());
        assert!(panel.set_speed(10.5).is_empty());
    }

    #[test]
    fn speed_rounds_to_three_decimals_half_to_even() {
        assert_eq!(round_speed(2.0004), 2.0);
        assert_eq!(round_speed(2.0006), 2.001);
        assert_eq!(round_speed(0.0625), 0.062);
        assert_eq!(round_speed(0.1875), 0.188);
    }

    #[test]
    fn set_as_default_saves_the_target_and_speed_in_range_only() {
        let mut panel = SubtitleDelayPanel::open(&FakeMpv::default(), &prefs());
        panel.set_delay(120);
        panel.set_speed(0.5);
        assert_eq!(
            panel.apply(),
            vec![
                Write::pref("pref_subtitles_delay", "120"),
                Write::pref("pref_subtitles_speed", "0.5"),
            ]
        );

        panel.set_speed(20.0);
        assert_eq!(panel.apply(), vec![Write::pref("pref_subtitles_delay", "120")]);
    }

    #[test]
    fn secondary_default_saves_the_secondary_key_not_the_primary() {
        let mpv = FakeMpv::default();
        let mut panel = SubtitleDelayPanel::open(&mpv, &prefs());
        panel.select_target(DelayTarget::Secondary, &mpv, &prefs());
        panel.set_delay(-400);
        assert_eq!(
            panel.apply(),
            vec![
                Write::pref("pref_subtitles_secondary_delay", "-400"),
                Write::pref("pref_subtitles_speed", "1.5"),
            ]
        );
    }

    #[test]
    fn reset_writes_defaults_and_is_not_persisted() {
        let mut panel = SubtitleDelayPanel::open(&FakeMpv::default(), &prefs());
        panel.set_speed(3.0);
        assert_eq!(
            panel.reset(),
            vec![
                Write::mpv("sub-delay", "0.0"),
                Write::mpv("sub-speed", "1.0"),
            ]
        );
        assert_eq!(panel.delay_ms(), 0);
        assert_eq!(panel.speed(), 1.0);
    }

    #[test]
    fn dropdown_is_locked_while_a_run_is_active() {
        assert!(target_enabled(false));
        assert!(!target_enabled(true));
    }
}
