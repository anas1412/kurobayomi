//! Colors card of the subtitle settings (area 46 `SubtitleSettingsColorsCard.kt`, T-46b-034). One
//! of three targets (text, border, background) with four channels (red, green, blue, alpha) in
//! 0..=255. Colors are `#AARRGGBB` in mpv and an `Int` ARGB in the preference. Drag writes are
//! coalesced to one every 50 ms (E17), and the last value is sent when the drag ends.

use std::time::{Duration, Instant};

use crate::mpv::PropertySource;
use crate::options::{SubtitlePrefs, color_hex};
use crate::pref_write::Write;

/// Minimum time between two drag writes (E17, proposal).
pub const COLOR_WRITE_INTERVAL: Duration = Duration::from_millis(50);

/// Which subtitle color is edited.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorTarget {
    Text,
    Border,
    Background,
}

impl ColorTarget {
    pub const ALL: [Self; 3] = [Self::Text, Self::Border, Self::Background];

    pub fn title(self) -> &'static str {
        match self {
            Self::Text => "Text",
            Self::Border => "Border",
            Self::Background => "Background",
        }
    }

    /// The mpv property that holds the color.
    pub fn property(self) -> &'static str {
        match self {
            Self::Text => "sub-color",
            Self::Border => "sub-border-color",
            Self::Background => "sub-back-color",
        }
    }

    /// The preference that holds the color.
    pub fn pref_key(self) -> &'static str {
        match self {
            Self::Text => "pref_text_color_subtitles",
            Self::Border => "pref_border_color_subtitles",
            Self::Background => "pref_background_color_subtitles",
        }
    }

    /// Default ARGB: white, opaque black, transparent.
    pub fn default_argb(self) -> i32 {
        match self {
            Self::Text => -1,
            Self::Border => -16_777_216,
            Self::Background => 0,
        }
    }

    fn preference(self, prefs: &SubtitlePrefs) -> i32 {
        match self {
            Self::Text => prefs.text_color_argb,
            Self::Border => prefs.border_color_argb,
            Self::Background => prefs.back_color_argb,
        }
    }
}

/// One channel of the color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Red,
    Green,
    Blue,
    Alpha,
}

impl Channel {
    fn shift(self) -> u32 {
        match self {
            Self::Alpha => 24,
            Self::Red => 16,
            Self::Green => 8,
            Self::Blue => 0,
        }
    }
}

/// The value of one channel, 0..=255.
pub fn channel_value(argb: i32, channel: Channel) -> u8 {
    ((argb as u32 >> channel.shift()) & 0xFF) as u8
}

/// The color with one channel replaced.
pub fn with_channel(argb: i32, channel: Channel, value: u8) -> i32 {
    let cleared = (argb as u32) & !(0xFFu32 << channel.shift());
    (cleared | (u32::from(value) << channel.shift())) as i32
}

/// Parses `#AARRGGBB` or `#RRGGBB` (opaque). Anything else is `None` (E15).
pub fn parse_color(text: &str) -> Option<i32> {
    let hex = text.strip_prefix('#')?;
    match hex.len() {
        6 => u32::from_str_radix(hex, 16)
            .ok()
            .map(|rgb| (0xFF00_0000 | rgb) as i32),
        8 => u32::from_str_radix(hex, 16).ok().map(|v| v as i32),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColorCard {
    pub target: ColorTarget,
    pub argb: i32,
}

impl ColorCard {
    /// Reads the color of `target` from mpv. A missing or unparseable mpv value falls back to the
    /// preference (E14, E15).
    pub fn read(source: &impl PropertySource, prefs: &SubtitlePrefs, target: ColorTarget) -> Self {
        let argb = source
            .get_str(target.property())
            .and_then(|text| parse_color(&text))
            .unwrap_or_else(|| target.preference(prefs));
        Self { target, argb }
    }

    /// Changes the target and reads its color again.
    pub fn select(
        &mut self,
        target: ColorTarget,
        source: &impl PropertySource,
        prefs: &SubtitlePrefs,
    ) {
        *self = Self::read(source, prefs, target);
    }

    pub fn channel(&self, channel: Channel) -> u8 {
        channel_value(self.argb, channel)
    }

    /// One slider moved. The whole color is stored and sent, as the Kotlin does.
    pub fn set_channel(&mut self, channel: Channel, value: i32) -> Vec<Write> {
        let value = value.clamp(0, 255) as u8;
        self.argb = with_channel(self.argb, channel, value);
        self.writes()
    }

    /// Reset of the selected target: deletes the preference and writes the default color to mpv.
    pub fn reset(&mut self) -> Vec<Write> {
        self.argb = self.target.default_argb();
        vec![
            Write::PrefDelete {
                key: self.target.pref_key(),
            },
            Write::mpv(self.target.property(), color_hex(self.argb)),
        ]
    }

    fn writes(&self) -> Vec<Write> {
        vec![
            Write::pref(self.target.pref_key(), self.argb.to_string()),
            Write::mpv(self.target.property(), color_hex(self.argb)),
        ]
    }
}

/// Coalesces writes to at most one batch per `COLOR_WRITE_INTERVAL`. A batch that arrives too early
/// is kept and replaces any earlier pending one, so the last value of a drag is never lost.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteThrottle {
    last: Option<Instant>,
    pending: Option<Vec<Write>>,
}

impl WriteThrottle {
    /// Offers a batch at `now`. Returns the batch to send now, or `None` when it was held back.
    pub fn offer(&mut self, now: Instant, writes: Vec<Write>) -> Option<Vec<Write>> {
        let too_soon = self
            .last
            .is_some_and(|last| now.saturating_duration_since(last) < COLOR_WRITE_INTERVAL);
        if too_soon {
            self.pending = Some(writes);
            None
        } else {
            self.last = Some(now);
            self.pending = None;
            Some(writes)
        }
    }

    /// The drag ended: the held back batch is sent now, if there is one.
    pub fn flush(&mut self, now: Instant) -> Option<Vec<Write>> {
        let writes = self.pending.take()?;
        self.last = Some(now);
        Some(writes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_write::test_support::FakeMpv;

    fn prefs() -> SubtitlePrefs {
        SubtitlePrefs {
            text_color_argb: 0xFF00FF00u32 as i32,
            ..SubtitlePrefs::default()
        }
    }

    #[test]
    fn colors_parse_in_both_hex_forms_and_reject_the_rest() {
        assert_eq!(parse_color("#FFFFFFFF"), Some(-1));
        assert_eq!(parse_color("#00ff00"), Some(0xFF00FF00u32 as i32));
        assert_eq!(parse_color("#80112233"), Some(0x8011_2233u32 as i32));
        assert_eq!(parse_color("white"), None);
        assert_eq!(parse_color("#12345"), None);
        assert_eq!(parse_color("#GG0000"), None);
    }

    #[test]
    fn channels_extract_and_replace_without_touching_the_others() {
        let argb = 0x11223344u32 as i32;
        assert_eq!(channel_value(argb, Channel::Alpha), 0x11);
        assert_eq!(channel_value(argb, Channel::Red), 0x22);
        assert_eq!(channel_value(argb, Channel::Green), 0x33);
        assert_eq!(channel_value(argb, Channel::Blue), 0x44);
        assert_eq!(with_channel(argb, Channel::Green, 0xAA), 0x1122AA44u32 as i32);
    }

    #[test]
    fn read_prefers_mpv_then_the_preference_on_missing_or_bad_text() {
        let good = FakeMpv::with(&[("sub-color", "#FF0000FF")]);
        assert_eq!(
            ColorCard::read(&good, &prefs(), ColorTarget::Text).argb,
            0xFF0000FFu32 as i32
        );
        let bad = FakeMpv::with(&[("sub-color", "chartreuse")]);
        assert_eq!(
            ColorCard::read(&bad, &prefs(), ColorTarget::Text).argb,
            0xFF00FF00u32 as i32
        );
        let missing = FakeMpv::default();
        assert_eq!(
            ColorCard::read(&missing, &prefs(), ColorTarget::Background).argb,
            0
        );
    }

    #[test]
    fn a_channel_change_writes_the_pref_and_the_full_hex_color() {
        let mut card = ColorCard {
            target: ColorTarget::Border,
            argb: -16_777_216,
        };
        assert_eq!(
            card.set_channel(Channel::Alpha, 128),
            vec![
                Write::pref("pref_border_color_subtitles", "-2147483648"),
                Write::mpv("sub-border-color", "#80000000"),
            ]
        );
        assert_eq!(card.channel(Channel::Alpha), 128);
        card.set_channel(Channel::Red, 999);
        assert_eq!(card.channel(Channel::Red), 255);
    }

    #[test]
    fn reset_restores_the_selected_target_default() {
        let mut card = ColorCard {
            target: ColorTarget::Text,
            argb: 0,
        };
        assert_eq!(
            card.reset(),
            vec![
                Write::PrefDelete {
                    key: "pref_text_color_subtitles"
                },
                Write::mpv("sub-color", "#FFFFFFFF"),
            ]
        );
        assert_eq!(card.argb, -1);
    }

    #[test]
    fn drag_writes_are_held_to_one_per_fifty_milliseconds_and_the_last_is_flushed() {
        let start = Instant::now();
        let at = |ms: u64| start + Duration::from_millis(ms);
        let mut throttle = WriteThrottle::default();
        assert!(throttle.offer(at(0), vec![Write::mpv("sub-color", "a")]).is_some());
        assert!(throttle.offer(at(10), vec![Write::mpv("sub-color", "b")]).is_none());
        assert!(throttle.offer(at(20), vec![Write::mpv("sub-color", "c")]).is_none());
        assert!(throttle.offer(at(60), vec![Write::mpv("sub-color", "d")]).is_some());
        assert!(throttle.flush(at(61)).is_none());
        throttle.offer(at(70), vec![Write::mpv("sub-color", "e")]);
        assert_eq!(
            throttle.flush(at(75)),
            Some(vec![Write::mpv("sub-color", "e")])
        );
    }
}
