//! Typography card of the subtitle settings (area 46 `SubtitleSettingsTypographyCard.kt`, T-46b-033,
//! T-46b-039). Every setter returns the preference and mpv writes; `reset` returns the writes of
//! the reset oracle (p2-topic-subtitle-style-and-delay section 6, with the G4 fix). The state is read
//! from mpv with the preference as fallback (T-46b-004), and an unknown mpv value falls back to the
//! default instead of failing (E23, E24).

use crate::enums::{SubtitleJustification, SubtitlesBorderStyle};
use crate::font_scan::DEFAULT_FAMILY;
use crate::mpv::PropertySource;
use crate::options::SubtitlePrefs;
use crate::pref_write::Write;

/// Font size slider range.
pub const FONT_SIZE_RANGE: (i32, i32) = (1, 100);
/// Border size and shadow offset slider range.
pub const BORDER_SIZE_RANGE: (i32, i32) = (0, 100);
pub const SHADOW_RANGE: (i32, i32) = (0, 100);

/// The stored name of a justification: the enum variant name (T-46b-005).
pub fn justify_pref(justify: SubtitleJustification) -> &'static str {
    match justify {
        SubtitleJustification::Left => "Left",
        SubtitleJustification::Center => "Center",
        SubtitleJustification::Right => "Right",
        SubtitleJustification::Auto => "Auto",
    }
}

/// The stored name of a border style: the enum variant name (T-46b-005).
pub fn border_style_pref(style: SubtitlesBorderStyle) -> &'static str {
    match style {
        SubtitlesBorderStyle::OutlineAndShadow => "OutlineAndShadow",
        SubtitlesBorderStyle::OpaqueBox => "OpaqueBox",
        SubtitlesBorderStyle::BackgroundBox => "BackgroundBox",
    }
}

/// Justification read from mpv. Unknown text is `Auto` (E23).
pub fn justify_from_mpv(value: &str) -> SubtitleJustification {
    match value {
        "left" => SubtitleJustification::Left,
        "center" => SubtitleJustification::Center,
        "right" => SubtitleJustification::Right,
        _ => SubtitleJustification::Auto,
    }
}

/// Border style read from mpv. Unknown text is `OutlineAndShadow` (E24).
pub fn border_style_from_mpv(value: &str) -> SubtitlesBorderStyle {
    match value {
        "opaque-box" => SubtitlesBorderStyle::OpaqueBox,
        "background-box" => SubtitlesBorderStyle::BackgroundBox,
        _ => SubtitlesBorderStyle::OutlineAndShadow,
    }
}

/// The font dropdown: "Sans Serif" first, then the scanned families without a repeat of it or of
/// each other (E30).
pub fn font_choices(scanned: &[String]) -> Vec<String> {
    let mut fonts = vec![DEFAULT_FAMILY.to_owned()];
    for family in scanned {
        if !fonts.contains(family) {
            fonts.push(family.clone());
        }
    }
    fonts
}

/// Flag text from mpv (`yes`/`no`, or `true`/`false`). Anything else is absent.
fn flag(source: &impl PropertySource, name: &str) -> Option<bool> {
    match source.get_str(name)?.as_str() {
        "yes" | "true" => Some(true),
        "no" | "false" => Some(false),
        _ => None,
    }
}

fn int(source: &impl PropertySource, name: &str) -> Option<i32> {
    source.get_str(name)?.trim().parse().ok()
}

fn clamp_to(value: i32, (lo, hi): (i32, i32)) -> i32 {
    value.clamp(lo, hi)
}

fn flag_text(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn pref_flag(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Typography {
    pub bold: bool,
    pub italic: bool,
    pub justify: SubtitleJustification,
    pub font: String,
    pub font_size: i32,
    pub border_style: SubtitlesBorderStyle,
    pub border_size: i32,
    pub shadow_offset: i32,
}

impl Typography {
    /// Reads every control from mpv. A missing value falls back to the preference.
    pub fn read(source: &impl PropertySource, prefs: &SubtitlePrefs) -> Self {
        Self {
            bold: flag(source, "sub-bold").unwrap_or(prefs.bold),
            italic: flag(source, "sub-italic").unwrap_or(prefs.italic),
            justify: source
                .get_str("sub-justify")
                .map_or(prefs.justify, |v| justify_from_mpv(&v)),
            font: source
                .get_str("sub-font")
                .unwrap_or_else(|| prefs.font.clone()),
            font_size: int(source, "sub-font-size").unwrap_or(prefs.font_size),
            border_style: source
                .get_str("sub-border-style")
                .map_or(prefs.border_style, |v| border_style_from_mpv(&v)),
            border_size: int(source, "sub-border-size").unwrap_or(prefs.border_size),
            shadow_offset: int(source, "sub-shadow-offset").unwrap_or(prefs.shadow_offset),
        }
    }

    pub fn set_bold(&mut self, on: bool) -> Vec<Write> {
        self.bold = on;
        vec![
            Write::pref("pref_bold_subtitles", pref_flag(on)),
            Write::mpv("sub-bold", flag_text(on)),
        ]
    }

    pub fn set_italic(&mut self, on: bool) -> Vec<Write> {
        self.italic = on;
        vec![
            Write::pref("pref_italic_subtitles", pref_flag(on)),
            Write::mpv("sub-italic", flag_text(on)),
        ]
    }

    /// A justification toggle. Checking one selects it. Unchecking it selects `Auto` in the state,
    /// the preference and mpv (E25: the toggle no longer stays checked after a second click).
    pub fn toggle_justify(&mut self, justify: SubtitleJustification, checked: bool) -> Vec<Write> {
        if checked {
            self.justify = justify;
            vec![
                Write::mpv("sub-ass-justify", "yes"),
                Write::pref("pref_sub_justify", justify_pref(justify)),
                Write::mpv("sub-justify", justify.value()),
            ]
        } else {
            self.justify = SubtitleJustification::Auto;
            vec![
                Write::mpv("sub-ass-justify", "no"),
                Write::pref(
                    "pref_sub_justify",
                    justify_pref(SubtitleJustification::Auto),
                ),
                Write::mpv("sub-justify", SubtitleJustification::Auto.value()),
            ]
        }
    }

    pub fn set_font(&mut self, font: &str) -> Vec<Write> {
        self.font = font.to_owned();
        vec![
            Write::pref("pref_subtitle_font", font),
            Write::mpv("sub-font", font),
        ]
    }

    pub fn set_font_size(&mut self, size: i32) -> Vec<Write> {
        self.font_size = clamp_to(size, FONT_SIZE_RANGE);
        vec![
            Write::pref("pref_subtitles_font_size", self.font_size.to_string()),
            Write::mpv("sub-font-size", self.font_size.to_string()),
        ]
    }

    pub fn set_border_style(&mut self, style: SubtitlesBorderStyle) -> Vec<Write> {
        self.border_style = style;
        vec![
            Write::pref("pref_border_style_subtitles", border_style_pref(style)),
            Write::mpv("sub-border-style", style.value()),
        ]
    }

    pub fn set_border_size(&mut self, size: i32) -> Vec<Write> {
        self.border_size = clamp_to(size, BORDER_SIZE_RANGE);
        vec![
            Write::pref("pref_sub_border_size", self.border_size.to_string()),
            Write::mpv("sub-border-size", self.border_size.to_string()),
        ]
    }

    pub fn set_shadow_offset(&mut self, offset: i32) -> Vec<Write> {
        self.shadow_offset = clamp_to(offset, SHADOW_RANGE);
        vec![
            Write::pref("sub_shadow_offset", self.shadow_offset.to_string()),
            Write::mpv("sub-shadow-offset", self.shadow_offset.to_string()),
        ]
    }

    /// Reset (behaviour 13.8, oracle SV2). Deletes the typography preferences and writes the
    /// defaults. `sub-ass-justify` is set to `no` here, not from the ASS override preference (G4).
    /// The caller applies the writes and then reads the controls from mpv again with `read`.
    pub fn reset(&mut self) -> Vec<Write> {
        *self = Self::defaults();
        vec![
            Write::PrefDelete {
                key: "pref_bold_subtitles",
            },
            Write::mpv("sub-bold", "no"),
            Write::PrefDelete {
                key: "pref_italic_subtitles",
            },
            Write::mpv("sub-italic", "no"),
            Write::mpv("sub-ass-justify", "no"),
            Write::PrefDelete {
                key: "pref_sub_justify",
            },
            Write::mpv("sub-justify", "auto"),
            Write::PrefDelete {
                key: "pref_subtitle_font",
            },
            Write::mpv("sub-font", DEFAULT_FAMILY),
            Write::PrefDelete {
                key: "pref_subtitles_font_size",
            },
            Write::mpv("sub-font-size", "55"),
            Write::PrefDelete {
                key: "pref_sub_border_size",
            },
            Write::mpv("sub-border-size", "3"),
            Write::PrefDelete {
                key: "sub_shadow_offset",
            },
            Write::mpv("sub-shadow-offset", "0"),
            Write::PrefDelete {
                key: "pref_border_style_subtitles",
            },
            Write::mpv("sub-border-style", "outline-and-shadow"),
        ]
    }

    /// The defaults of the typography controls (p2-behavior 13.9).
    pub fn defaults() -> Self {
        let prefs = SubtitlePrefs::default();
        Self {
            bold: prefs.bold,
            italic: prefs.italic,
            justify: prefs.justify,
            font: prefs.font,
            font_size: prefs.font_size,
            border_style: prefs.border_style,
            border_size: prefs.border_size,
            shadow_offset: prefs.shadow_offset,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pref_write::apply_mpv;
    use crate::pref_write::test_support::FakeMpv;

    fn prefs() -> SubtitlePrefs {
        SubtitlePrefs {
            font: "Noto Sans".to_owned(),
            font_size: 40,
            ..SubtitlePrefs::default()
        }
    }

    #[test]
    fn read_uses_mpv_then_the_preference() {
        let mpv = FakeMpv::with(&[("sub-bold", "yes"), ("sub-font-size", "60")]);
        let t = Typography::read(&mpv, &prefs());
        assert!(t.bold);
        assert_eq!(t.font_size, 60);
        assert_eq!(t.font, "Noto Sans");
        assert_eq!(t.border_size, 3);
    }

    #[test]
    fn unknown_justification_and_border_style_fall_back_instead_of_failing() {
        let mpv = FakeMpv::with(&[("sub-justify", "diagonal"), ("sub-border-style", "neon")]);
        let t = Typography::read(&mpv, &prefs());
        assert_eq!(t.justify, SubtitleJustification::Auto);
        assert_eq!(t.border_style, SubtitlesBorderStyle::OutlineAndShadow);
    }

    #[test]
    fn checked_justify_writes_ass_justify_yes_and_its_value() {
        let mut t = Typography::defaults();
        let writes = t.toggle_justify(SubtitleJustification::Right, true);
        assert_eq!(t.justify, SubtitleJustification::Right);
        assert_eq!(
            writes,
            vec![
                Write::mpv("sub-ass-justify", "yes"),
                Write::pref("pref_sub_justify", "Right"),
                Write::mpv("sub-justify", "right"),
            ]
        );
    }

    #[test]
    fn unchecking_a_justify_toggle_selects_auto_everywhere() {
        let mut t = Typography::defaults();
        t.toggle_justify(SubtitleJustification::Left, true);
        let writes = t.toggle_justify(SubtitleJustification::Left, false);
        assert_eq!(t.justify, SubtitleJustification::Auto);
        assert_eq!(
            writes,
            vec![
                Write::mpv("sub-ass-justify", "no"),
                Write::pref("pref_sub_justify", "Auto"),
                Write::mpv("sub-justify", "auto"),
            ]
        );
    }

    #[test]
    fn slider_values_clamp_to_their_ranges() {
        let mut t = Typography::defaults();
        t.set_font_size(0);
        assert_eq!(t.font_size, 1);
        t.set_font_size(500);
        assert_eq!(t.font_size, 100);
        t.set_border_size(-4);
        assert_eq!(t.border_size, 0);
        assert_eq!(
            t.set_shadow_offset(42),
            vec![
                Write::pref("sub_shadow_offset", "42"),
                Write::mpv("sub-shadow-offset", "42"),
            ]
        );
    }

    #[test]
    fn border_style_stores_the_variant_name_and_writes_the_mpv_value() {
        let mut t = Typography::defaults();
        assert_eq!(
            t.set_border_style(SubtitlesBorderStyle::BackgroundBox),
            vec![
                Write::pref("pref_border_style_subtitles", "BackgroundBox"),
                Write::mpv("sub-border-style", "background-box"),
            ]
        );
    }

    #[test]
    fn font_list_starts_with_sans_serif_and_drops_repeats() {
        let scanned = vec![
            "Sans Serif".to_owned(),
            "Noto Sans".to_owned(),
            "Noto Sans".to_owned(),
        ];
        assert_eq!(
            font_choices(&scanned),
            vec!["Sans Serif".to_owned(), "Noto Sans".to_owned()]
        );
        assert_eq!(font_choices(&[]), vec!["Sans Serif".to_owned()]);
    }

    #[test]
    fn reset_writes_the_oracle_sequence_with_the_ass_justify_fix() {
        let mut t = Typography::read(&FakeMpv::default(), &SubtitlePrefs::default());
        t.set_bold(true);
        let writes = t.reset();
        let mpv: Vec<(&str, &str)> = writes
            .iter()
            .filter_map(|w| match w {
                Write::Mpv { name, value } => Some((*name, value.as_str())),
                _ => None,
            })
            .collect();
        assert_eq!(
            mpv,
            vec![
                ("sub-bold", "no"),
                ("sub-italic", "no"),
                ("sub-ass-justify", "no"),
                ("sub-justify", "auto"),
                ("sub-font", "Sans Serif"),
                ("sub-font-size", "55"),
                ("sub-border-size", "3"),
                ("sub-shadow-offset", "0"),
                ("sub-border-style", "outline-and-shadow"),
            ]
        );
        assert!(writes.contains(&Write::PrefDelete {
            key: "pref_sub_justify"
        }));
        assert!(!writes.iter().any(|w| matches!(
            w,
            Write::Mpv { name, .. } if *name == "sub-ass-override"
        )));
    }

    #[test]
    fn controls_read_back_the_defaults_after_reset_is_applied() {
        let fake = FakeMpv::with(&[("sub-font", "Noto Sans"), ("sub-font-size", "9")]);
        let mut t = Typography::read(&fake, &prefs());
        let writes = t.reset();
        apply_mpv(&fake, &writes);
        assert_eq!(Typography::read(&fake, &prefs()), Typography::defaults());
    }
}
