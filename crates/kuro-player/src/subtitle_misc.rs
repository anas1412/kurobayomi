//! Miscellaneous card of the subtitle settings (area 46 `SubtitleSettingsMiscellaneousCard.kt`,
//! T-46b-035): the ASS/SSA override switch, the scale and the vertical position. Pure: the setters
//! return the preference and mpv writes. The Kotlin debug print of the mpv value is not ported (E19).

use crate::mpv::PropertySource;
use crate::options::{SubtitlePrefs, kotlin_f32, kotlin_f64};
use crate::pref_write::Write;

/// Scale slider range. Zero is allowed and hides the subtitles (hint shown).
pub const SCALE_RANGE: (f32, f32) = (0.0, 5.0);
/// Position slider range.
pub const POS_RANGE: (i32, i32) = (0, 150);
/// Hint shown under the scale slider at zero (p2-ui-parity 10.2, new string).
pub const SCALE_ZERO_HINT: &str = "Subtitles are hidden at scale 0";

/// The scale text with two decimals.
pub fn scale_text(scale: f32) -> String {
    format!("{scale:.2}")
}

#[derive(Clone, Debug, PartialEq)]
pub struct Misc {
    pub override_ass: bool,
    pub scale: f32,
    pub pos: i32,
}

impl Misc {
    /// Reads the three controls from mpv. A missing value falls back to the preference.
    pub fn read(source: &impl PropertySource, prefs: &SubtitlePrefs) -> Self {
        Self {
            override_ass: source
                .get_str("sub-ass-override")
                .map_or(prefs.override_ass, |v| v == "force"),
            scale: source
                .get_str("sub-scale")
                .and_then(|v| v.trim().parse::<f64>().ok())
                .map_or(prefs.scale, |v| v as f32),
            pos: source
                .get_str("sub-pos")
                .and_then(|v| v.trim().parse().ok())
                .unwrap_or(prefs.pos),
        }
    }

    /// The ASS/SSA override switch: `force` when on, `scale` when off.
    pub fn set_override(&mut self, on: bool) -> Vec<Write> {
        self.override_ass = on;
        vec![
            Write::pref("pref_override_subtitles_ass", if on { "true" } else { "false" }),
            Write::mpv("sub-ass-override", if on { "force" } else { "scale" }),
        ]
    }

    pub fn set_scale(&mut self, scale: f32) -> Vec<Write> {
        self.scale = scale.clamp(SCALE_RANGE.0, SCALE_RANGE.1);
        vec![
            Write::pref("pref_sub_scale", kotlin_f32(self.scale)),
            Write::mpv("sub-scale", kotlin_f64(f64::from(self.scale))),
        ]
    }

    pub fn set_pos(&mut self, pos: i32) -> Vec<Write> {
        self.pos = pos.clamp(POS_RANGE.0, POS_RANGE.1);
        vec![
            Write::pref("pref_sub_pos", self.pos.to_string()),
            Write::mpv("sub-pos", self.pos.to_string()),
        ]
    }

    /// The hint for scale zero, `None` otherwise.
    pub fn scale_hint(&self) -> Option<&'static str> {
        (self.scale == 0.0).then_some(SCALE_ZERO_HINT)
    }

    /// Reset (behaviour 15.1): position 100, scale 1, override off, and `sub-ass-override` is written
    /// as `scale` whatever the preference said. The preferences are deleted.
    pub fn reset(&mut self) -> Vec<Write> {
        self.pos = 100;
        self.scale = 1.0;
        self.override_ass = false;
        vec![
            Write::PrefDelete {
                key: "pref_sub_pos",
            },
            Write::mpv("sub-pos", "100"),
            Write::PrefDelete {
                key: "pref_sub_scale",
            },
            Write::mpv("sub-scale", "1.0"),
            Write::PrefDelete {
                key: "pref_override_subtitles_ass",
            },
            Write::mpv("sub-ass-override", "scale"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_write::test_support::FakeMpv;

    #[test]
    fn override_switch_reads_force_and_writes_force_or_scale() {
        let mpv = FakeMpv::with(&[("sub-ass-override", "force")]);
        let mut misc = Misc::read(&mpv, &SubtitlePrefs::default());
        assert!(misc.override_ass);
        assert_eq!(
            misc.set_override(false),
            vec![
                Write::pref("pref_override_subtitles_ass", "false"),
                Write::mpv("sub-ass-override", "scale"),
            ]
        );
    }

    #[test]
    fn scale_clamps_to_zero_to_five_and_formats_two_decimals() {
        let mut misc = Misc::read(&FakeMpv::default(), &SubtitlePrefs::default());
        assert_eq!(
            misc.set_scale(7.0),
            vec![
                Write::pref("pref_sub_scale", "5.0"),
                Write::mpv("sub-scale", "5.0"),
            ]
        );
        assert_eq!(scale_text(misc.scale), "5.00");
        assert_eq!(scale_text(1.5), "1.50");
    }

    #[test]
    fn zero_scale_shows_the_hint() {
        let mut misc = Misc::read(&FakeMpv::default(), &SubtitlePrefs::default());
        assert_eq!(misc.scale_hint(), None);
        misc.set_scale(0.0);
        assert_eq!(misc.scale_hint(), Some("Subtitles are hidden at scale 0"));
    }

    #[test]
    fn position_slider_is_zero_to_one_fifty() {
        let mut misc = Misc::read(&FakeMpv::default(), &SubtitlePrefs::default());
        assert_eq!(misc.set_pos(200).last(), Some(&Write::mpv("sub-pos", "150")));
        assert_eq!(misc.pos, 150);
        misc.set_pos(-3);
        assert_eq!(misc.pos, 0);
    }

    #[test]
    fn reset_restores_position_scale_and_override_and_writes_scale_mode() {
        let mpv = FakeMpv::with(&[("sub-pos", "20"), ("sub-scale", "3"), ("sub-ass-override", "force")]);
        let mut misc = Misc::read(&mpv, &SubtitlePrefs::default());
        let writes = misc.reset();
        assert_eq!((misc.pos, misc.scale, misc.override_ass), (100, 1.0, false));
        assert_eq!(
            writes,
            vec![
                Write::PrefDelete {
                    key: "pref_sub_pos"
                },
                Write::mpv("sub-pos", "100"),
                Write::PrefDelete {
                    key: "pref_sub_scale"
                },
                Write::mpv("sub-scale", "1.0"),
                Write::PrefDelete {
                    key: "pref_override_subtitles_ass"
                },
                Write::mpv("sub-ass-override", "scale"),
            ]
        );
    }

    #[test]
    fn missing_mpv_values_fall_back_to_the_preference() {
        let prefs = SubtitlePrefs {
            pos: 80,
            scale: 1.25,
            override_ass: true,
            ..SubtitlePrefs::default()
        };
        let misc = Misc::read(&FakeMpv::default(), &prefs);
        assert_eq!((misc.pos, misc.scale, misc.override_ass), (80, 1.25, true));
    }
}
