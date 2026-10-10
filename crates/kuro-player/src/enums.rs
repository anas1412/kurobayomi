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

/// Orientation the player locks to (`PlayerEnums.kt:41-50`). Default `SensorLandscape`. Desktop
/// windows ignore it, but the preference is stored and read by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PlayerOrientation {
    Free,
    Video,
    Portrait,
    ReversePortrait,
    SensorPortrait,
    Landscape,
    ReverseLandscape,
    #[default]
    SensorLandscape,
}

/// Rotation the desktop control sets in place of the orientation lock (T-45a-035): mpv `video-rotate`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum VideoRotate {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl VideoRotate {
    /// The value mpv takes for `video-rotate`.
    pub fn degrees(self) -> i64 {
        match self {
            Self::Deg0 => 0,
            Self::Deg90 => 90,
            Self::Deg180 => 180,
            Self::Deg270 => 270,
        }
    }
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
    /// The id the mockup spec uses for each sheet (`spec/reader_player.json`).
    pub const fn spec_id(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::PlaybackSpeed => "playback_speed",
            Self::SubtitleTracks => "subtitle_tracks",
            Self::AudioTracks => "audio_tracks",
            Self::QualityTracks => "quality_tracks",
            Self::Chapters => "chapters",
            Self::More => "more",
            Self::Screenshot => "screenshot",
        }
    }

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
    /// The chips of the hardware decoding row. `Auto` is never shown (DC5).
    pub const CHIPS: [Self; 4] = [Self::AutoCopy, Self::Sw, Self::Hw, Self::HwPlus];

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

    /// The decoder for the `hwdec` value mpv reports as active (`hwdec-current`, T-45b-068). Linux
    /// names map to the choice they came from: VA-API and NVDEC zero-copy to `HwPlus`, their copy
    /// modes to `Hw`. Anything unknown is `Auto` (E-07).
    pub fn from_active(value: &str) -> Self {
        Self::from_hwdec_current(value).unwrap_or(Self::Auto)
    }

    /// The decoder for a `hwdec-current` string, or `None` for a string this port does not know
    /// (DC4). `no` is software, the copy modes are `Hw` and the zero-copy modes are `HwPlus`.
    pub fn from_hwdec_current(value: &str) -> Option<Self> {
        match value {
            "auto-copy" => Some(Self::AutoCopy),
            "auto" => Some(Self::Auto),
            "no" => Some(Self::Sw),
            "vaapi-copy" | "nvdec-copy" | "vulkan-copy" | "mediacodec-copy" => Some(Self::Hw),
            "vaapi" | "nvdec" | "vulkan" | "mediacodec" => Some(Self::HwPlus),
            _ => None,
        }
    }

    /// The chip that is selected for the `hwdec-current` value mpv reports (E71). Unknown values and
    /// `auto` select no chip, so the More sheet shows nothing selected instead of failing.
    pub fn selected_chip(value: &str) -> Option<Self> {
        Self::from_hwdec_current(value).filter(|d| Self::CHIPS.contains(d))
    }

    /// The Linux `hwdec` value for a chip, from the GPU vendor (design table, §8.1). Android values
    /// never reach Linux, so this replaces `mpv_value` when a chip is written.
    pub fn linux_value(self, gpu: GpuKind) -> &'static str {
        let vendor_va = matches!(gpu, GpuKind::Intel | GpuKind::Amd);
        match self {
            Self::AutoCopy => "auto-copy",
            Self::Auto => "auto",
            Self::Sw => "no",
            Self::Hw if vendor_va => "vaapi-copy",
            Self::Hw if gpu == GpuKind::Nvidia => "nvdec-copy",
            Self::Hw => "auto-copy",
            Self::HwPlus if vendor_va => "vaapi",
            Self::HwPlus if gpu == GpuKind::Nvidia => "nvdec",
            Self::HwPlus => "auto",
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

/// The GPU vendor that picks the Linux `hwdec` names (design table §8.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GpuKind {
    Intel,
    Amd,
    Nvidia,
    #[default]
    Other,
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
    /// Label in the chip row. `Auto` and `AutoSafe` are the swapped pair (A6).
    pub const fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::AutoSafe => "Auto-safe",
            Self::Mono => "Mono",
            Self::Stereo => "Stereo",
            Self::ReverseStereo => "Reverse stereo",
        }
    }

    /// The stored name (`getEnum`). An unknown name falls back to the default, `AutoSafe`.
    pub fn from_stored(name: &str) -> Self {
        match name {
            "Auto" => Self::Auto,
            "AutoSafe" => Self::AutoSafe,
            "Mono" => Self::Mono,
            "Stereo" => Self::Stereo,
            "ReverseStereo" => Self::ReverseStereo,
            _ => Self::default(),
        }
    }

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
    fn sheet_spec_ids_match_the_mockup() {
        let ids: Vec<&str> = Sheets::ALL.iter().map(|s| s.spec_id()).collect();
        assert_eq!(
            ids,
            [
                "none",
                "playback_speed",
                "subtitle_tracks",
                "audio_tracks",
                "quality_tracks",
                "chapters",
                "more",
                "screenshot",
            ]
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
    fn active_hwdec_maps_linux_names_and_falls_back_to_auto() {
        assert_eq!(Decoder::from_active("vaapi-copy"), Decoder::Hw);
        assert_eq!(Decoder::from_active("nvdec"), Decoder::HwPlus);
        assert_eq!(Decoder::from_active("no"), Decoder::Sw);
        assert_eq!(Decoder::from_active("auto-copy"), Decoder::AutoCopy);
        assert_eq!(Decoder::from_active("something-new"), Decoder::Auto);
        assert_eq!(Decoder::from_active("vaapi").title(), "HW+");
    }

    #[test]
    fn hwdec_current_returns_none_for_unknown_strings() {
        assert_eq!(Decoder::from_hwdec_current("no"), Some(Decoder::Sw));
        assert_eq!(Decoder::from_hwdec_current("vaapi-copy"), Some(Decoder::Hw));
        assert_eq!(Decoder::from_hwdec_current("vaapi"), Some(Decoder::HwPlus));
        assert_eq!(Decoder::from_hwdec_current("bogus"), None);
    }

    #[test]
    fn chips_are_the_four_visible_decoders_in_order() {
        assert_eq!(
            Decoder::CHIPS,
            [Decoder::AutoCopy, Decoder::Sw, Decoder::Hw, Decoder::HwPlus]
        );
        assert!(!Decoder::CHIPS.contains(&Decoder::Auto));
    }

    #[test]
    fn auto_writes_auto_safe_and_auto_safe_writes_auto() {
        assert_eq!(
            AudioChannels::Auto.setting(),
            ("audio-channels", "auto-safe")
        );
        assert_eq!(
            AudioChannels::AutoSafe.setting(),
            ("audio-channels", "auto")
        );
    }

    #[test]
    fn audio_channel_labels_are_not_swapped() {
        assert_eq!(AudioChannels::Auto.label(), "Auto");
        assert_eq!(AudioChannels::AutoSafe.label(), "Auto-safe");
        assert_eq!(AudioChannels::ReverseStereo.label(), "Reverse stereo");
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

#[cfg(test)]
mod decoder_chip_tests {
    use super::*;

    #[test]
    fn dc6_linux_values_follow_the_vendor_table() {
        assert_eq!(Decoder::Hw.linux_value(GpuKind::Nvidia), "nvdec-copy");
        assert_eq!(Decoder::Hw.linux_value(GpuKind::Intel), "vaapi-copy");
        assert_eq!(Decoder::HwPlus.linux_value(GpuKind::Amd), "vaapi");
        assert_eq!(Decoder::Hw.linux_value(GpuKind::Other), "auto-copy");
        assert_eq!(Decoder::HwPlus.linux_value(GpuKind::Other), "auto");
        assert_eq!(Decoder::Sw.linux_value(GpuKind::Nvidia), "no");
    }

    #[test]
    fn unknown_and_auto_values_select_no_chip() {
        assert_eq!(Decoder::selected_chip("bogus"), None);
        assert_eq!(Decoder::selected_chip("auto"), None);
        assert_eq!(Decoder::selected_chip("vulkan-copy"), Some(Decoder::Hw));
        assert_eq!(Decoder::selected_chip("nvdec"), Some(Decoder::HwPlus));
        assert_eq!(Decoder::selected_chip("auto-copy"), Some(Decoder::AutoCopy));
    }

    #[test]
    fn audio_channels_from_stored_falls_back_to_auto_safe() {
        assert_eq!(AudioChannels::from_stored("Bogus"), AudioChannels::AutoSafe);
        assert_eq!(
            AudioChannels::from_stored("ReverseStereo"),
            AudioChannels::ReverseStereo
        );
        assert_eq!(AudioChannels::from_stored("Auto"), AudioChannels::Auto);
    }
}
