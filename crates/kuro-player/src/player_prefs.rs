//! Player preference catalogue (area 45 data doc section 4, T-45b-048, T-45b-074). Every key the
//! player reads, with its exact Kotlin default, and typed readers over a text store. Enum values are
//! stored by their Kotlin enum name (`getEnum` semantics), and an unknown or missing value gives the
//! key's default. The text store is the one `pref_write::Write::Pref` writes to. Keys of the gesture
//! and subtitle panels are listed again here so one table answers "what is the default".
//!
//! DECISION: the filter keys (`pref_player_filter_*`) have no default in the source, so the store
//! default (0 for an Int) is used.

use std::collections::HashMap;

use crate::enums::{
    AudioChannels, Debanding, PlayerOrientation, SingleActionGesture, SubtitleJustification,
    SubtitlesBorderStyle, VideoAspect,
};

/// A key's default, typed as the source types it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PrefDefault {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
    /// An enum, by its Kotlin name.
    Enum(&'static str),
    StringSet(&'static [&'static str]),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PrefKey {
    pub key: &'static str,
    pub default: PrefDefault,
}

/// The key names, for code that reads or writes one preference.
pub mod keys {
    pub const MPV_SCRIPTS: &str = "mpv_scripts";
    pub const PREF_MPV_CONF: &str = "pref_mpv_conf";
    pub const PREF_MPV_INPUT: &str = "pref_mpv_input";
    pub const PREF_PLAYER_STATISTICS_PAGE: &str = "pref_player_statistics_page";
    pub const PREF_AUDIO_LANG: &str = "pref_audio_lang";
    pub const PREF_AUDIO_PITCH_CORRECTION: &str = "pref_audio_pitch_correction";
    pub const PREF_AUDIO_CONFIG: &str = "pref_audio_config";
    pub const PREF_AUDIO_VOLUME_BOOST_CAP: &str = "pref_audio_volume_boost_cap";
    pub const PREF_AUDIO_DELAY: &str = "pref_audio_delay";
    pub const PREF_TRY_HWDEC: &str = "pref_try_hwdec";
    pub const PREF_GPU_NEXT: &str = "pref_gpu_next";
    pub const PREF_VIDEO_DEBANDING: &str = "pref_video_debanding";
    pub const USE_YUV420P: &str = "use_yuv420p";
    pub const PREF_PLAYER_FILTER_BRIGHTNESS: &str = "pref_player_filter_brightness";
    pub const PREF_PLAYER_FILTER_SATURATION: &str = "pref_player_filter_saturation";
    pub const PREF_PLAYER_FILTER_CONTRAST: &str = "pref_player_filter_contrast";
    pub const PREF_PLAYER_FILTER_GAMMA: &str = "pref_player_filter_gamma";
    pub const PREF_PLAYER_FILTER_HUE: &str = "pref_player_filter_hue";
    pub const PREF_GESTURE_VOLUME_BRIGHTNESS: &str = "pref_gesture_volume_brightness";
    pub const PREF_SWAP_VOLUME_AND_BRIGHTNESS: &str = "pref_swap_volume_and_brightness";
    pub const PREF_GESTURE_HORIZONTAL_SEEK: &str = "pref_gesture_horizontal_seek";
    pub const PREF_SHOW_SEEKBAR: &str = "pref_show_seekbar";
    pub const PREF_DEFAULT_INTRO_LENGTH: &str = "pref_default_intro_length";
    pub const PREF_SKIP_LENGTH_PREFERENCE: &str = "pref_skip_length_preference";
    pub const PREF_PLAYER_SMOOTH_SEEK: &str = "pref_player_smooth_seek";
    pub const PREF_LEFT_DOUBLE_TAP: &str = "pref_left_double_tap";
    pub const PREF_CENTER_DOUBLE_TAP: &str = "pref_center_double_tap";
    pub const PREF_RIGHT_DOUBLE_TAP: &str = "pref_right_double_tap";
    pub const PREF_MEDIA_PREVIOUS: &str = "pref_media_previous";
    pub const PREF_MEDIA_PLAYPAUSE: &str = "pref_media_playpause";
    pub const PREF_MEDIA_NEXT: &str = "pref_media_next";
    pub const PREF_PRESERVE_WATCHING_POSITION: &str = "pref_preserve_watching_position";
    pub const PREF_PROGRESS_PREFERENCE: &str = "pref_progress_preference";
    pub const PREF_DEFAULT_PLAYER_ORIENTATION_TYPE_KEY: &str =
        "pref_default_player_orientation_type_key";
    pub const PREF_ALLOW_GESTURES_IN_PANELS: &str = "pref_allow_gestures_in_panels";
    pub const PREF_SHOW_LOADING: &str = "pref_show_loading";
    pub const PREF_SHOW_CURRENT_CHAPTER: &str = "pref_show_current_chapter";
    pub const PREF_REMEMBER_BRIGHTNESS: &str = "pref_remember_brightness";
    pub const PLAYER_BRIGHTNESS_VALUE: &str = "player_brightness_value";
    pub const PREF_REMEMBER_VOLUME: &str = "pref_remember_volume";
    pub const PLAYER_VOLUME_VALUE: &str = "player_volume_value";
    pub const PREF_SHOW_FAILED_HOSTERS: &str = "pref_show_failed_hosters";
    pub const PREF_SHOW_EMPTY_HOSTERS: &str = "pref_show_empty_hosters";
    pub const PLAYER_FULLSCREEN: &str = "player_fullscreen";
    pub const PLAYER_HIDE_CONTROLS: &str = "player_hide_controls";
    pub const PREF_DISPLAY_VOL_AS_PER: &str = "pref_display_vol_as_per";
    pub const PREF_SHOW_SYSTEM_STATUS_BAR: &str = "pref_show_system_status_bar";
    pub const PREF_REDUCE_MOTION: &str = "pref_reduce_motion";
    pub const PREF_PLAYER_TIME_TO_DISAPPEAR: &str = "pref_player_time_to_disappear";
    pub const PREF_PANEL_OPACITY: &str = "pref_panel_opacity";
    pub const PREF_ENABLE_SKIP_INTRO: &str = "pref_enable_skip_intro";
    pub const PREF_ENABLE_AUTO_SKIP_ANI_SKIP: &str = "pref_enable_auto_skip_ani_skip";
    pub const PREF_ENABLE_NETFLIXSTYLE_ANISKIP: &str = "pref_enable_netflixStyle_aniskip";
    pub const PREF_WAITING_TIME_ANISKIP: &str = "pref_waiting_time_aniskip";
    pub const PREF_ENABLE_ANI_SKIP: &str = "pref_enable_ani_skip";
    pub const PREF_DISABLED_ANI_SKIP_CHAPTERS: &str = "pref_disabled_ani_skip_chapters";
    pub const PREF_ENABLE_PIP: &str = "pref_enable_pip";
    pub const PREF_PIP_EPISODE_TOASTS: &str = "pref_pip_episode_toasts";
    pub const PREF_PIP_ON_EXIT: &str = "pref_pip_on_exit";
    pub const PIP_REPLACE_WITH_PREVIOUS: &str = "pip_replace_with_previous";
    pub const PREF_ALWAYS_USE_EXTERNAL_PLAYER: &str = "pref_always_use_external_player";
    pub const EXTERNAL_PLAYER_PREFERENCE: &str = "external_player_preference";
    pub const PREF_PLAYER_SPEED: &str = "pref_player_speed";
    pub const DEFAULT_SPEED_PRESETS: &str = "default_speed_presets";
    pub const INVERT_DURATION: &str = "invert_duration";
    pub const PREF_PLAYER_ASPECT_STATE: &str = "pref_player_aspect_state";
    pub const PREF_AUTO_PLAY_ENABLED: &str = "pref_auto_play_enabled";
    pub const PREF_SUBTITLE_LANG: &str = "pref_subtitle_lang";
    pub const PREF_SUBTITLE_WHITELIST: &str = "pref_subtitle_whitelist";
    pub const PREF_SUBTITLE_BLACKLIST: &str = "pref_subtitle_blacklist";
    pub const PREF_SCREENSHOT_SUBTITLES: &str = "pref_screenshot_subtitles";
    pub const PREF_SUBTITLE_FONT: &str = "pref_subtitle_font";
    pub const PREF_SUBTITLES_FONT_SIZE: &str = "pref_subtitles_font_size";
    pub const PREF_SUB_SCALE: &str = "pref_sub_scale";
    pub const PREF_SUB_BORDER_SIZE: &str = "pref_sub_border_size";
    pub const PREF_BOLD_SUBTITLES: &str = "pref_bold_subtitles";
    pub const PREF_ITALIC_SUBTITLES: &str = "pref_italic_subtitles";
    pub const PREF_TEXT_COLOR_SUBTITLES: &str = "pref_text_color_subtitles";
    pub const PREF_BORDER_COLOR_SUBTITLES: &str = "pref_border_color_subtitles";
    pub const PREF_BORDER_STYLE_SUBTITLES: &str = "pref_border_style_subtitles";
    pub const SUB_SHADOW_OFFSET: &str = "sub_shadow_offset";
    pub const PREF_BACKGROUND_COLOR_SUBTITLES: &str = "pref_background_color_subtitles";
    pub const PREF_SUB_JUSTIFY: &str = "pref_sub_justify";
    pub const PREF_SUB_POS: &str = "pref_sub_pos";
    pub const PREF_OVERRIDE_SUBTITLES_ASS: &str = "pref_override_subtitles_ass";
    pub const PREF_SUBTITLES_DELAY: &str = "pref_subtitles_delay";
    pub const PREF_SUBTITLES_SPEED: &str = "pref_subtitles_speed";
    pub const PREF_SUBTITLES_SECONDARY_DELAY: &str = "pref_subtitles_secondary_delay";
    pub const APP_STATE_PREF_DOWNLOADED_ONLY: &str = "__APP_STATE_pref_downloaded_only";
    pub const APP_STATE_INCOGNITO_MODE: &str = "__APP_STATE_incognito_mode";
    pub const PREF_AUTO_UPDATE_MANGA_SYNC_KEY: &str = "pref_auto_update_manga_sync_key";
    pub const RELATIVE_TIME_V2: &str = "relative_time_v2";
    pub const APP_DATE_FORMAT: &str = "app_date_format";
    pub const AUTO_DOWNLOAD_WHILE_WATCHING: &str = "auto_download_while_watching";
    pub const REMOVE_AFTER_READ_SLOTS: &str = "remove_after_read_slots";
    pub const MARK_DUPLICATE_SEEN_EPISODE_SEEN: &str = "mark_duplicate_seen_episode_seen";
}

/// Every key, in the order of the data doc (section 4.1 to 4.6, then the keys read from other
/// files).
pub const CATALOGUE: [PrefKey; 96] = [
    PrefKey {
        key: keys::MPV_SCRIPTS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_MPV_CONF,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_MPV_INPUT,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_PLAYER_STATISTICS_PAGE,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_AUDIO_LANG,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_AUDIO_PITCH_CORRECTION,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_AUDIO_CONFIG,
        default: PrefDefault::Enum("AutoSafe"),
    },
    PrefKey {
        key: keys::PREF_AUDIO_VOLUME_BOOST_CAP,
        default: PrefDefault::Int(30),
    },
    PrefKey {
        key: keys::PREF_AUDIO_DELAY,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_TRY_HWDEC,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_GPU_NEXT,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_VIDEO_DEBANDING,
        default: PrefDefault::Enum("None"),
    },
    PrefKey {
        key: keys::USE_YUV420P,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_PLAYER_FILTER_BRIGHTNESS,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_PLAYER_FILTER_SATURATION,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_PLAYER_FILTER_CONTRAST,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_PLAYER_FILTER_GAMMA,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_PLAYER_FILTER_HUE,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_GESTURE_VOLUME_BRIGHTNESS,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_SWAP_VOLUME_AND_BRIGHTNESS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_GESTURE_HORIZONTAL_SEEK,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_SHOW_SEEKBAR,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_DEFAULT_INTRO_LENGTH,
        default: PrefDefault::Int(85),
    },
    PrefKey {
        key: keys::PREF_SKIP_LENGTH_PREFERENCE,
        default: PrefDefault::Int(10),
    },
    PrefKey {
        key: keys::PREF_PLAYER_SMOOTH_SEEK,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_LEFT_DOUBLE_TAP,
        default: PrefDefault::Enum("Seek"),
    },
    PrefKey {
        key: keys::PREF_CENTER_DOUBLE_TAP,
        default: PrefDefault::Enum("PlayPause"),
    },
    PrefKey {
        key: keys::PREF_RIGHT_DOUBLE_TAP,
        default: PrefDefault::Enum("Seek"),
    },
    PrefKey {
        key: keys::PREF_MEDIA_PREVIOUS,
        default: PrefDefault::Enum("Switch"),
    },
    PrefKey {
        key: keys::PREF_MEDIA_PLAYPAUSE,
        default: PrefDefault::Enum("PlayPause"),
    },
    PrefKey {
        key: keys::PREF_MEDIA_NEXT,
        default: PrefDefault::Enum("Switch"),
    },
    PrefKey {
        key: keys::PREF_PRESERVE_WATCHING_POSITION,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_PROGRESS_PREFERENCE,
        default: PrefDefault::Float(0.85),
    },
    PrefKey {
        key: keys::PREF_DEFAULT_PLAYER_ORIENTATION_TYPE_KEY,
        default: PrefDefault::Enum("SensorLandscape"),
    },
    PrefKey {
        key: keys::PREF_ALLOW_GESTURES_IN_PANELS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_SHOW_LOADING,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_SHOW_CURRENT_CHAPTER,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_REMEMBER_BRIGHTNESS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PLAYER_BRIGHTNESS_VALUE,
        default: PrefDefault::Float(-1.0),
    },
    PrefKey {
        key: keys::PREF_REMEMBER_VOLUME,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PLAYER_VOLUME_VALUE,
        default: PrefDefault::Float(-1.0),
    },
    PrefKey {
        key: keys::PREF_SHOW_FAILED_HOSTERS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_SHOW_EMPTY_HOSTERS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PLAYER_FULLSCREEN,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PLAYER_HIDE_CONTROLS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_DISPLAY_VOL_AS_PER,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_SHOW_SYSTEM_STATUS_BAR,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_REDUCE_MOTION,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_PLAYER_TIME_TO_DISAPPEAR,
        default: PrefDefault::Int(4000),
    },
    PrefKey {
        key: keys::PREF_PANEL_OPACITY,
        default: PrefDefault::Int(60),
    },
    PrefKey {
        key: keys::PREF_ENABLE_SKIP_INTRO,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_ENABLE_AUTO_SKIP_ANI_SKIP,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_ENABLE_NETFLIXSTYLE_ANISKIP,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_WAITING_TIME_ANISKIP,
        default: PrefDefault::Int(5),
    },
    PrefKey {
        key: keys::PREF_ENABLE_ANI_SKIP,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_DISABLED_ANI_SKIP_CHAPTERS,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_ENABLE_PIP,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_PIP_EPISODE_TOASTS,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PREF_PIP_ON_EXIT,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PIP_REPLACE_WITH_PREVIOUS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_ALWAYS_USE_EXTERNAL_PLAYER,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::EXTERNAL_PLAYER_PREFERENCE,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_PLAYER_SPEED,
        default: PrefDefault::Float(1.0),
    },
    PrefKey {
        key: keys::DEFAULT_SPEED_PRESETS,
        default: PrefDefault::StringSet(&[
            "0.25", "0.5", "0.75", "1.0", "1.25", "1.5", "1.75", "2.0", "2.5", "3.0", "3.5", "4.0",
        ]),
    },
    PrefKey {
        key: keys::INVERT_DURATION,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_PLAYER_ASPECT_STATE,
        default: PrefDefault::Enum("Fit"),
    },
    PrefKey {
        key: keys::PREF_AUTO_PLAY_ENABLED,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_SUBTITLE_LANG,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_SUBTITLE_WHITELIST,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_SUBTITLE_BLACKLIST,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_SCREENSHOT_SUBTITLES,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_SUBTITLE_FONT,
        default: PrefDefault::Str("Sans Serif"),
    },
    PrefKey {
        key: keys::PREF_SUBTITLES_FONT_SIZE,
        default: PrefDefault::Int(55),
    },
    PrefKey {
        key: keys::PREF_SUB_SCALE,
        default: PrefDefault::Float(1.0),
    },
    PrefKey {
        key: keys::PREF_SUB_BORDER_SIZE,
        default: PrefDefault::Int(3),
    },
    PrefKey {
        key: keys::PREF_BOLD_SUBTITLES,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_ITALIC_SUBTITLES,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_TEXT_COLOR_SUBTITLES,
        default: PrefDefault::Int(-1),
    },
    PrefKey {
        key: keys::PREF_BORDER_COLOR_SUBTITLES,
        default: PrefDefault::Int(-16777216),
    },
    PrefKey {
        key: keys::PREF_BORDER_STYLE_SUBTITLES,
        default: PrefDefault::Enum("OutlineAndShadow"),
    },
    PrefKey {
        key: keys::SUB_SHADOW_OFFSET,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_BACKGROUND_COLOR_SUBTITLES,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_SUB_JUSTIFY,
        default: PrefDefault::Enum("Auto"),
    },
    PrefKey {
        key: keys::PREF_SUB_POS,
        default: PrefDefault::Int(100),
    },
    PrefKey {
        key: keys::PREF_OVERRIDE_SUBTITLES_ASS,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_SUBTITLES_DELAY,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::PREF_SUBTITLES_SPEED,
        default: PrefDefault::Float(1.0),
    },
    PrefKey {
        key: keys::PREF_SUBTITLES_SECONDARY_DELAY,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::APP_STATE_PREF_DOWNLOADED_ONLY,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::APP_STATE_INCOGNITO_MODE,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_AUTO_UPDATE_MANGA_SYNC_KEY,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::RELATIVE_TIME_V2,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::APP_DATE_FORMAT,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::AUTO_DOWNLOAD_WHILE_WATCHING,
        default: PrefDefault::Int(0),
    },
    PrefKey {
        key: keys::REMOVE_AFTER_READ_SLOTS,
        default: PrefDefault::Int(-1),
    },
    PrefKey {
        key: keys::MARK_DUPLICATE_SEEN_EPISODE_SEEN,
        default: PrefDefault::StringSet(&[]),
    },
];

/// The catalogue entry for `key`.
pub fn find(key: &str) -> Option<&'static PrefKey> {
    CATALOGUE.iter().find(|entry| entry.key == key)
}

/// An enum stored by its Kotlin name.
pub trait NamedEnum: Copy + 'static {
    const ALL: &'static [Self];
    fn name(self) -> &'static str;

    /// `getEnum`: the variant with this name, or `None`.
    fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.name() == name)
    }
}

macro_rules! named_enum {
    ($ty:ty { $($variant:ident => $name:literal),+ $(,)? }) => {
        impl NamedEnum for $ty {
            const ALL: &'static [Self] = &[$(Self::$variant),+];
            fn name(self) -> &'static str {
                match self { $(Self::$variant => $name),+ }
            }
        }
    };
}

named_enum!(VideoAspect { Crop => "Crop", Fit => "Fit", Stretch => "Stretch" });
named_enum!(SingleActionGesture {
    None => "None", Seek => "Seek", PlayPause => "PlayPause", Switch => "Switch", Custom => "Custom"
});
named_enum!(Debanding { None => "None", Cpu => "CPU", Gpu => "GPU" });
named_enum!(AudioChannels {
    Auto => "Auto", AutoSafe => "AutoSafe", Mono => "Mono", Stereo => "Stereo",
    ReverseStereo => "ReverseStereo"
});
named_enum!(SubtitleJustification {
    Left => "Left", Center => "Center", Right => "Right", Auto => "Auto"
});
named_enum!(SubtitlesBorderStyle {
    OutlineAndShadow => "OutlineAndShadow", OpaqueBox => "OpaqueBox", BackgroundBox => "BackgroundBox"
});
named_enum!(PlayerOrientation {
    Free => "Free", Video => "Video", Portrait => "Portrait", ReversePortrait => "ReversePortrait",
    SensorPortrait => "SensorPortrait", Landscape => "Landscape",
    ReverseLandscape => "ReverseLandscape", SensorLandscape => "SensorLandscape"
});

/// Where stored preference text comes from. A missing key is `None`.
pub trait PrefStore {
    fn get(&self, key: &str) -> Option<String>;
}

impl PrefStore for HashMap<String, String> {
    fn get(&self, key: &str) -> Option<String> {
        HashMap::get(self, key).cloned()
    }
}

fn default_of(key: &str) -> Option<PrefDefault> {
    find(key).map(|entry| entry.default)
}

/// A Boolean preference. Anything but `true` or `false` gives the default.
pub fn get_bool(store: &impl PrefStore, key: &str) -> bool {
    match store.get(key).as_deref() {
        Some("true") => true,
        Some("false") => false,
        _ => matches!(default_of(key), Some(PrefDefault::Bool(true))),
    }
}

/// An Int preference.
pub fn get_int(store: &impl PrefStore, key: &str) -> i64 {
    store
        .get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(match default_of(key) {
            Some(PrefDefault::Int(v)) => v,
            _ => 0,
        })
}

/// A Float preference.
pub fn get_float(store: &impl PrefStore, key: &str) -> f64 {
    store
        .get(key)
        .and_then(|v| v.parse().ok())
        .unwrap_or(match default_of(key) {
            Some(PrefDefault::Float(v)) => v,
            _ => 0.0,
        })
}

/// A String preference.
pub fn get_str(store: &impl PrefStore, key: &str) -> String {
    store.get(key).unwrap_or_else(|| match default_of(key) {
        Some(PrefDefault::Str(v)) => v.to_owned(),
        _ => String::new(),
    })
}

/// An enum preference stored by name. An unknown name gives the key's default, and a key with no
/// valid default gives the first variant.
pub fn get_enum<E: NamedEnum>(store: &impl PrefStore, key: &str) -> E {
    store
        .get(key)
        .and_then(|v| E::from_name(&v))
        .or_else(|| match default_of(key) {
            Some(PrefDefault::Enum(name)) => E::from_name(name),
            _ => None,
        })
        .unwrap_or(E::ALL[0])
}

/// The text to store for an enum value (`Write::pref(key, enum_text(value))`).
pub fn enum_text<E: NamedEnum>(value: E) -> &'static str {
    value.name()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_every_source_key_is_present() {
        let mut seen = std::collections::HashSet::new();
        assert!(CATALOGUE.iter().all(|e| seen.insert(e.key)));
        assert_eq!(CATALOGUE.len(), 96);
        for key in [
            "pref_audio_lang",
            "pref_subtitle_lang",
            "pref_subtitle_whitelist",
            "pref_subtitle_blacklist",
            "pref_player_aspect_state",
            "pref_progress_preference",
            "pref_preserve_watching_position",
            "pref_show_current_chapter",
            "pref_enable_skip_intro",
            "pref_remember_volume",
            "pref_try_hwdec",
            "pref_gpu_next",
            "pref_player_filter_hue",
            "player_fullscreen",
        ] {
            assert!(find(key).is_some(), "{key} missing");
        }
    }

    /// T74: every key of data doc section 4 with its default.
    #[test]
    fn t74_all_defaults_match_source() {
        use PrefDefault::*;
        let table: &[(&str, PrefDefault)] = &[
            ("mpv_scripts", Bool(false)),
            ("pref_mpv_conf", Str("")),
            ("pref_mpv_input", Str("")),
            ("pref_player_statistics_page", Int(0)),
            ("pref_audio_lang", Str("")),
            ("pref_audio_pitch_correction", Bool(true)),
            ("pref_audio_config", Enum("AutoSafe")),
            ("pref_audio_volume_boost_cap", Int(30)),
            ("pref_audio_delay", Int(0)),
            ("pref_try_hwdec", Bool(true)),
            ("pref_gpu_next", Bool(false)),
            ("pref_video_debanding", Enum("None")),
            ("use_yuv420p", Bool(true)),
            ("pref_player_filter_brightness", Int(0)),
            ("pref_player_filter_saturation", Int(0)),
            ("pref_player_filter_contrast", Int(0)),
            ("pref_player_filter_gamma", Int(0)),
            ("pref_player_filter_hue", Int(0)),
            ("pref_gesture_volume_brightness", Bool(true)),
            ("pref_swap_volume_and_brightness", Bool(false)),
            ("pref_gesture_horizontal_seek", Bool(true)),
            ("pref_show_seekbar", Bool(false)),
            ("pref_default_intro_length", Int(85)),
            ("pref_skip_length_preference", Int(10)),
            ("pref_player_smooth_seek", Bool(false)),
            ("pref_left_double_tap", Enum("Seek")),
            ("pref_center_double_tap", Enum("PlayPause")),
            ("pref_right_double_tap", Enum("Seek")),
            ("pref_media_previous", Enum("Switch")),
            ("pref_media_playpause", Enum("PlayPause")),
            ("pref_media_next", Enum("Switch")),
            ("pref_preserve_watching_position", Bool(false)),
            ("pref_progress_preference", Float(0.85)),
            (
                "pref_default_player_orientation_type_key",
                Enum("SensorLandscape"),
            ),
            ("pref_allow_gestures_in_panels", Bool(false)),
            ("pref_show_loading", Bool(true)),
            ("pref_show_current_chapter", Bool(true)),
            ("pref_remember_brightness", Bool(false)),
            ("player_brightness_value", Float(-1.0)),
            ("pref_remember_volume", Bool(false)),
            ("player_volume_value", Float(-1.0)),
            ("pref_show_failed_hosters", Bool(false)),
            ("pref_show_empty_hosters", Bool(false)),
            ("player_fullscreen", Bool(true)),
            ("player_hide_controls", Bool(false)),
            ("pref_display_vol_as_per", Bool(true)),
            ("pref_show_system_status_bar", Bool(false)),
            ("pref_reduce_motion", Bool(false)),
            ("pref_player_time_to_disappear", Int(4000)),
            ("pref_panel_opacity", Int(60)),
            ("pref_enable_skip_intro", Bool(true)),
            ("pref_enable_auto_skip_ani_skip", Bool(false)),
            ("pref_enable_netflixStyle_aniskip", Bool(false)),
            ("pref_waiting_time_aniskip", Int(5)),
            ("pref_enable_ani_skip", Bool(false)),
            ("pref_disabled_ani_skip_chapters", Bool(true)),
            ("pref_enable_pip", Bool(true)),
            ("pref_pip_episode_toasts", Bool(true)),
            ("pref_pip_on_exit", Bool(false)),
            ("pip_replace_with_previous", Bool(false)),
            ("pref_always_use_external_player", Bool(false)),
            ("external_player_preference", Str("")),
            ("pref_player_speed", Float(1.0)),
            ("invert_duration", Bool(false)),
            ("pref_player_aspect_state", Enum("Fit")),
            ("pref_auto_play_enabled", Bool(false)),
            ("pref_subtitle_lang", Str("")),
            ("pref_subtitle_whitelist", Str("")),
            ("pref_subtitle_blacklist", Str("")),
            ("pref_screenshot_subtitles", Bool(false)),
            ("pref_subtitle_font", Str("Sans Serif")),
            ("pref_subtitles_font_size", Int(55)),
            ("pref_sub_scale", Float(1.0)),
            ("pref_sub_border_size", Int(3)),
            ("pref_bold_subtitles", Bool(false)),
            ("pref_italic_subtitles", Bool(false)),
            ("pref_text_color_subtitles", Int(-1)),
            ("pref_border_color_subtitles", Int(-16_777_216)),
            ("pref_border_style_subtitles", Enum("OutlineAndShadow")),
            ("sub_shadow_offset", Int(0)),
            ("pref_background_color_subtitles", Int(0)),
            ("pref_sub_justify", Enum("Auto")),
            ("pref_sub_pos", Int(100)),
            ("pref_override_subtitles_ass", Bool(false)),
            ("pref_subtitles_delay", Int(0)),
            ("pref_subtitles_speed", Float(1.0)),
            ("pref_subtitles_secondary_delay", Int(0)),
            ("__APP_STATE_pref_downloaded_only", Bool(false)),
            ("__APP_STATE_incognito_mode", Bool(false)),
            ("pref_auto_update_manga_sync_key", Bool(true)),
            ("relative_time_v2", Bool(true)),
            ("app_date_format", Str("")),
            ("auto_download_while_watching", Int(0)),
            ("remove_after_read_slots", Int(-1)),
            ("mark_duplicate_seen_episode_seen", StringSet(&[])),
        ];
        for (key, want) in table {
            assert_eq!(find(key).map(|e| e.default), Some(*want), "{key}");
        }
        // 88 keys of section 4.1 to 4.6, the speed presets, and the eight read from other files
        assert_eq!(table.len() + 1, CATALOGUE.len());
    }

    /// T75
    #[test]
    fn t75_speed_presets_count_12() {
        let Some(PrefKey {
            default: PrefDefault::StringSet(set),
            ..
        }) = find("default_speed_presets")
        else {
            panic!("default_speed_presets missing");
        };
        assert_eq!(set.len(), 12);
        assert_eq!(set[0], "0.25");
        assert_eq!(set[11], "4.0");
    }

    /// T76
    #[test]
    fn t76_audio_channels_values() {
        let pairs = [
            (AudioChannels::Auto, "Auto", ("audio-channels", "auto-safe")),
            (
                AudioChannels::AutoSafe,
                "AutoSafe",
                ("audio-channels", "auto"),
            ),
            (AudioChannels::Mono, "Mono", ("audio-channels", "mono")),
            (
                AudioChannels::Stereo,
                "Stereo",
                ("audio-channels", "stereo"),
            ),
            (
                AudioChannels::ReverseStereo,
                "ReverseStereo",
                ("af", "pan=[stereo|c0=c1|c1=c0]"),
            ),
        ];
        for (value, name, setting) in pairs {
            assert_eq!(value.name(), name);
            assert_eq!(value.setting(), setting);
        }
    }

    #[test]
    fn t76_subtitle_justification_values() {
        let pairs = [
            (SubtitleJustification::Left, "Left", "left"),
            (SubtitleJustification::Center, "Center", "center"),
            (SubtitleJustification::Right, "Right", "right"),
            (SubtitleJustification::Auto, "Auto", "auto"),
        ];
        for (value, name, mpv) in pairs {
            assert_eq!(value.name(), name);
            assert_eq!(value.value(), mpv);
        }
    }

    #[test]
    fn t76_border_style_values() {
        let pairs = [
            (
                SubtitlesBorderStyle::OutlineAndShadow,
                "OutlineAndShadow",
                "outline-and-shadow",
            ),
            (SubtitlesBorderStyle::OpaqueBox, "OpaqueBox", "opaque-box"),
            (
                SubtitlesBorderStyle::BackgroundBox,
                "BackgroundBox",
                "background-box",
            ),
        ];
        for (value, name, mpv) in pairs {
            assert_eq!(value.name(), name);
            assert_eq!(value.value(), mpv);
        }
    }

    #[test]
    fn enums_are_stored_by_name_and_unknown_names_use_the_default() {
        let mut store = HashMap::new();
        assert_eq!(
            get_enum::<VideoAspect>(&store, keys::PREF_PLAYER_ASPECT_STATE),
            VideoAspect::Fit
        );
        store.insert(
            keys::PREF_PLAYER_ASPECT_STATE.to_owned(),
            enum_text(VideoAspect::Crop).to_owned(),
        );
        assert_eq!(
            get_enum::<VideoAspect>(&store, keys::PREF_PLAYER_ASPECT_STATE),
            VideoAspect::Crop
        );
        store.insert(keys::PREF_PLAYER_ASPECT_STATE.to_owned(), "crop".to_owned());
        assert_eq!(
            get_enum::<VideoAspect>(&store, keys::PREF_PLAYER_ASPECT_STATE),
            VideoAspect::Fit
        );
        store.insert(keys::PREF_VIDEO_DEBANDING.to_owned(), "GPU".to_owned());
        assert_eq!(
            get_enum::<Debanding>(&store, keys::PREF_VIDEO_DEBANDING),
            Debanding::Gpu
        );
        assert_eq!(
            get_enum::<PlayerOrientation>(&store, keys::PREF_DEFAULT_PLAYER_ORIENTATION_TYPE_KEY),
            PlayerOrientation::SensorLandscape
        );
    }

    #[test]
    fn typed_readers_parse_the_stored_text_or_fall_back() {
        let mut store = HashMap::new();
        assert!(!get_bool(&store, keys::PREF_PRESERVE_WATCHING_POSITION));
        assert!(get_bool(&store, keys::PLAYER_FULLSCREEN));
        assert_eq!(get_int(&store, keys::PREF_PLAYER_TIME_TO_DISAPPEAR), 4000);
        assert_eq!(get_float(&store, keys::PREF_PROGRESS_PREFERENCE), 0.85);
        assert_eq!(get_str(&store, keys::PREF_SUBTITLE_FONT), "Sans Serif");
        store.insert(
            keys::PREF_PRESERVE_WATCHING_POSITION.to_owned(),
            "true".to_owned(),
        );
        store.insert(
            keys::PREF_PLAYER_TIME_TO_DISAPPEAR.to_owned(),
            "x".to_owned(),
        );
        store.insert(keys::PREF_PROGRESS_PREFERENCE.to_owned(), "0.9".to_owned());
        assert!(get_bool(&store, keys::PREF_PRESERVE_WATCHING_POSITION));
        assert_eq!(get_int(&store, keys::PREF_PLAYER_TIME_TO_DISAPPEAR), 4000);
        assert_eq!(get_float(&store, keys::PREF_PROGRESS_PREFERENCE), 0.9);
    }
}
