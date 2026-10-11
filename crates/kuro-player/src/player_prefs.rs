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
    AudioChannels, Debanding, SingleActionGesture, SubtitleJustification,
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
    pub const PREF_AUDIO_LANG: &str = "pref_audio_lang";
    pub const PREF_AUDIO_PITCH_CORRECTION: &str = "pref_audio_pitch_correction";
    pub const PLAYER_VOLUME_VALUE: &str = "player_volume_value";
    pub const PREF_ENABLE_PIP: &str = "pref_enable_pip";
    pub const PREF_PIP_EPISODE_TOASTS: &str = "pref_pip_episode_toasts";
    pub const PREF_PIP_ON_EXIT: &str = "pref_pip_on_exit";
    pub const PIP_REPLACE_WITH_PREVIOUS: &str = "pip_replace_with_previous";
    pub const PREF_TORRSERVER_ENABLE: &str = "pref_torrserver_enable";
    pub const PREF_TORRSERVER_SHOWNOTICE: &str = "pref_torrserver_shownotice";
    pub const PREF_TORRSERVER_PORT: &str = "pref_torrserver_port";
    pub const PREF_TORRSERVER_PROXYMODE: &str = "pref_torrserver_proxymode";
    pub const PREF_TORRSERVER_PROXYURL: &str = "pref_torrserver_proxyurl";
    pub const PREF_TORRSERVER_TACKERS: &str = "pref_torrserver_tackers";
    pub const PREF_ALWAYS_USE_EXTERNAL_PLAYER: &str = "pref_always_use_external_player";
    pub const EXTERNAL_PLAYER_PREFERENCE: &str = "external_player_preference";
    pub const PREF_PLAYER_SPEED: &str = "pref_player_speed";
    pub const DEFAULT_SPEED_PRESETS: &str = "default_speed_presets";
    pub const INVERT_DURATION: &str = "invert_duration";
    pub const PREF_PLAYER_ASPECT_STATE: &str = "pref_player_aspect_state";
    pub const PREF_AUTO_PLAY_ENABLED: &str = "pref_auto_play_enabled";
    pub const PREF_SUBTITLE_LANG: &str = "pref_subtitle_lang";
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
pub const CATALOGUE: [PrefKey; 29] = [
    PrefKey {
        key: keys::PREF_AUDIO_LANG,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_AUDIO_PITCH_CORRECTION,
        default: PrefDefault::Bool(true),
    },
    PrefKey {
        key: keys::PLAYER_VOLUME_VALUE,
        default: PrefDefault::Float(-1.0),
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
        key: keys::PREF_TORRSERVER_ENABLE,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_TORRSERVER_SHOWNOTICE,
        default: PrefDefault::Bool(false),
    },
    PrefKey {
        key: keys::PREF_TORRSERVER_PORT,
        default: PrefDefault::Str("8090"),
    },
    PrefKey {
        key: keys::PREF_TORRSERVER_PROXYMODE,
        default: PrefDefault::Enum("None"),
    },
    PrefKey {
        key: keys::PREF_TORRSERVER_PROXYURL,
        default: PrefDefault::Str(""),
    },
    PrefKey {
        key: keys::PREF_TORRSERVER_TACKERS,
        default: PrefDefault::Str(crate::torrent::trackers::DEFAULT_TRACKER_PREF),
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
        assert_eq!(CATALOGUE.len(), 29);
        for key in [
            "pref_audio_lang",
            "pref_subtitle_lang",
            "pref_player_aspect_state",
        ] {
            assert!(find(key).is_some(), "{key} missing");
        }
    }

    /// T74: every key of data doc section 4 with its default.
    #[test]
    fn t74_all_defaults_match_source() {
        use PrefDefault::*;
        let table: &[(&str, PrefDefault)] = &[
            ("pref_audio_lang", Str("")),
            ("pref_audio_pitch_correction", Bool(true)),
            ("player_volume_value", Float(-1.0)),
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
            ("__APP_STATE_pref_downloaded_only", Bool(false)),
            ("__APP_STATE_incognito_mode", Bool(false)),
            ("pref_auto_update_manga_sync_key", Bool(true)),
            ("relative_time_v2", Bool(true)),
            ("app_date_format", Str("")),
            ("auto_download_while_watching", Int(0)),
            ("remove_after_read_slots", Int(-1)),
            ("mark_duplicate_seen_episode_seen", StringSet(&[])),
            ("pref_torrserver_enable", Bool(false)),
            ("pref_torrserver_shownotice", Bool(false)),
            ("pref_torrserver_port", Str("8090")),
            ("pref_torrserver_proxymode", Enum("None")),
            ("pref_torrserver_proxyurl", Str("")),
            ("pref_torrserver_tackers", Str(crate::torrent::trackers::DEFAULT_TRACKER_PREF)),
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
    }

    #[test]
    fn typed_readers_parse_the_stored_text_or_fall_back() {
        let mut store = HashMap::new();
        assert!(get_bool(&store, keys::PREF_AUDIO_PITCH_CORRECTION));
        assert_eq!(get_str(&store, keys::PREF_SUBTITLE_LANG), "");
        store.insert(keys::PREF_AUDIO_PITCH_CORRECTION.to_owned(), "false".to_owned());
        store.insert(keys::PREF_PLAYER_SPEED.to_owned(), "1.5".to_owned());
        store.insert(keys::PREF_SUBTITLE_LANG.to_owned(), "en".to_owned());
        assert!(!get_bool(&store, keys::PREF_AUDIO_PITCH_CORRECTION));
        assert_eq!(get_float(&store, keys::PREF_PLAYER_SPEED), 1.5);
        assert_eq!(get_str(&store, keys::PREF_SUBTITLE_LANG), "en");
        store.insert(keys::PREF_PLAYER_SPEED.to_owned(), "x".to_owned());
        assert_eq!(get_float(&store, keys::PREF_PLAYER_SPEED), 1.0);
    }
}
