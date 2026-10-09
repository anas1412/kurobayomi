//! Player enums ported from `PlayerEnums.kt` and the preference enums the mpv options read
//! (area 45 data doc section 2.9). Stored values use the Kotlin enum names; the mpv-facing
//! strings are exposed through `mpv_value` and `value` helpers.

/// Result of a set-as-art action (`PlayerEnums.kt:29-33`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SetAsArt {
    Success,
    AddToLibraryFirst,
    Error,
}

/// Artwork slot that a set-as-art action writes (`PlayerEnums.kt:35-39`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArtType {
    Cover,
    Background,
    Thumbnail,
}

/// Video aspect mode (`PlayerEnums.kt:52-56`). Default `Fit` (`PlayerPreferences.kt:81`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoAspect {
    Crop,
    #[default]
    Fit,
    Stretch,
}

/// Single-tap action bound to a media key or gesture (`PlayerEnums.kt:61-67`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SingleActionGesture {
    None,
    Seek,
    PlayPause,
    Switch,
    Custom,
}

/// Bottom sheets (`PlayerEnums.kt:99-108`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Sheets {
    #[default]
    None,
    PlaybackSpeed,
    SubtitleTracks,
    AudioTracks,
    QualityTracks,
    Chapters,
    More,
    Screenshot,
}

impl Sheets {
    pub const ALL: [Self; 8] = [
        Self::None,
        Self::PlaybackSpeed,
        Self::SubtitleTracks,
        Self::AudioTracks,
        Self::QualityTracks,
        Self::Chapters,
        Self::More,
        Self::Screenshot,
    ];
}

/// Side panels (`PlayerEnums.kt:110-116`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Panels {
    #[default]
    None,
    SubtitleSettings,
    SubtitleDelay,
    AudioDelay,
    VideoFilters,
}

impl Panels {
    pub const ALL: [Self; 5] = [
        Self::None,
        Self::SubtitleSettings,
        Self::SubtitleDelay,
        Self::AudioDelay,
        Self::VideoFilters,
    ];
}

/// Dialogs (`PlayerEnums.kt:118-131`). The Kotlin `onChange` and `onDismissRequest` callbacks
/// stay in the GTK layer; this value carries the data only.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Dialogs {
    #[default]
    None,
    EpisodeList,
    IntegerPicker {
        default_value: i32,
        min_value: i32,
        max_value: i32,
        step: i32,
        name_format: String,
        title: String,
    },
}

/// Hardware decoder choice (`PlayerEnums.kt:81-91`). `mpv_value` is the Android value stored
/// by the Kotlin app; Linux candidates are resolved by `options::resolve_hwdec`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decoder {
    AutoCopy,
    Auto,
    Sw,
    Hw,
    HwPlus,
}

impl Decoder {
    pub const fn title(self) -> &'static str {
        match self {
            Self::AutoCopy | Self::Auto => "Auto",
            Self::Sw => "SW",
            Self::Hw => "HW",
            Self::HwPlus => "HW+",
        }
    }

    pub const fn mpv_value(self) -> &'static str {
        match self {
            Self::AutoCopy => "auto-copy",
            Self::Auto => "auto",
            Self::Sw => "no",
            Self::Hw => "mediacodec-copy",
            Self::HwPlus => "mediacodec",
        }
    }

    /// Finds the decoder whose stored value matches. Kotlin throws on an unknown value; the
    /// Rust port falls back to `Auto` so a stale preference cannot stop playback.
    pub fn from_value(value: &str) -> Self {
        [Self::AutoCopy, Self::Auto, Self::Sw, Self::Hw, Self::HwPlus]
            .into_iter()
            .find(|d| d.mpv_value() == value)
            .unwrap_or(Self::Auto)
    }
}

/// Video debanding filter (`PlayerEnums.kt:93-97`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Debanding {
    #[default]
    None,
    Cpu,
    Gpu,
}

/// Custom key codes sent for custom gestures (`PlayerEnums.kt:72-79`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum CustomKeyCode {
    DoubleTapLeft = 0x10001,
    DoubleTapCenter = 0x10002,
    DoubleTapRight = 0x10003,
    MediaPrevious = 0x10004,
    MediaPlay = 0x10005,
    MediaNext = 0x10006,
}

impl CustomKeyCode {
    pub fn from_code(code: u32) -> Option<Self> {
        [
            Self::DoubleTapLeft,
            Self::DoubleTapCenter,
            Self::DoubleTapRight,
            Self::MediaPrevious,
            Self::MediaPlay,
            Self::MediaNext,
        ]
        .into_iter()
        .find(|c| *c as u32 == code)
    }
}

/// Subtitle text justification (`SubtitlePreferences.kt:56-64`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SubtitleJustification {
    Left,
    Center,
    Right,
    #[default]
    Auto,
}

impl SubtitleJustification {
    pub const fn value(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
            Self::Auto => "auto",
        }
    }
}

/// Subtitle border style (`SubtitleSettingsTypographyCard.kt:368-375`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SubtitlesBorderStyle {
    #[default]
    OutlineAndShadow,
    OpaqueBox,
    BackgroundBox,
}

impl SubtitlesBorderStyle {
    pub const fn value(self) -> &'static str {
        match self {
            Self::OutlineAndShadow => "outline-and-shadow",
            Self::OpaqueBox => "opaque-box",
            Self::BackgroundBox => "background-box",
        }
    }
}

/// Audio channel layout (`AudioPreferences.kt:43-49`). `option` is the mpv option or property
/// name that receives `value`. `ReverseStereo` writes the `af` filter instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum AudioChannels {
    Auto,
    #[default]
    AutoSafe,
    Mono,
    Stereo,
    ReverseStereo,
}

impl AudioChannels {
    pub const fn setting(self) -> (&'static str, &'static str) {
        match self {
            Self::Auto => ("audio-channels", "auto-safe"),
            Self::AutoSafe => ("audio-channels", "auto"),
            Self::Mono => ("audio-channels", "mono"),
            Self::Stereo => ("audio-channels", "stereo"),
            Self::ReverseStereo => ("af", "pan=[stereo|c0=c1|c1=c0]"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sheets_has_eight_variants() {
        assert_eq!(Sheets::ALL.len(), 8);
        assert_eq!(
            Sheets::ALL
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            8
        );
    }

    #[test]
    fn panels_has_five_variants() {
        assert_eq!(Panels::ALL.len(), 5);
        assert_eq!(
            Panels::ALL
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            5
        );
    }

    #[test]
    fn defaults_match_preferences() {
        assert_eq!(VideoAspect::default(), VideoAspect::Fit);
        assert_eq!(Sheets::default(), Sheets::None);
        assert_eq!(Dialogs::default(), Dialogs::None);
        assert_eq!(Debanding::default(), Debanding::None);
        assert_eq!(AudioChannels::default(), AudioChannels::AutoSafe);
    }

    #[test]
    fn decoder_values_and_fallback() {
        assert_eq!(Decoder::Sw.mpv_value(), "no");
        assert_eq!(Decoder::HwPlus.mpv_value(), "mediacodec");
        assert_eq!(Decoder::from_value("mediacodec-copy"), Decoder::Hw);
        assert_eq!(Decoder::from_value("garbage"), Decoder::Auto);
        assert_eq!(Decoder::HwPlus.title(), "HW+");
    }

    #[test]
    fn custom_key_codes_round_trip() {
        assert_eq!(
            CustomKeyCode::from_code(0x10001),
            Some(CustomKeyCode::DoubleTapLeft)
        );
        assert_eq!(
            CustomKeyCode::from_code(0x10006),
            Some(CustomKeyCode::MediaNext)
        );
        assert_eq!(CustomKeyCode::from_code(0x10007), None);
    }

    #[test]
    fn audio_channel_settings() {
        assert_eq!(
            AudioChannels::AutoSafe.setting(),
            ("audio-channels", "auto")
        );
        assert_eq!(AudioChannels::ReverseStereo.setting().0, "af");
    }
}
