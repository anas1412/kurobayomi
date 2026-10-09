//! Pure mapping from player preferences to the startup mpv option table
//! (`AniyomiMPVView.kt:114-156`, `setupSubtitlesOptions`, `setupAudioOptions`, area 45 data doc
//! section 4 and behaviour doc 2.2). No libmpv and no I/O here: `startup_settings` returns the
//! ordered list that `MpvHandle::create` applies before `mpv_initialize`.

use std::path::{Path, PathBuf};

use crate::enums::{
    AudioChannels, Debanding, Decoder, SubtitleJustification, SubtitlesBorderStyle,
};

/// One startup setting. Every entry is applied before initialisation with `set_option`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MpvSetting {
    pub name: &'static str,
    pub value: String,
}

/// Demuxer cache size on Linux: 64 MiB for both forward and back cache (T-45a-005).
pub const CACHE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq)]
pub struct SubtitlePrefs {
    pub delay_ms: i32,
    pub speed: f32,
    pub secondary_delay_ms: i32,
    pub font: String,
    pub font_size: i32,
    pub bold: bool,
    pub italic: bool,
    pub justify: SubtitleJustification,
    pub text_color_argb: i32,
    pub back_color_argb: i32,
    pub border_color_argb: i32,
    pub border_size: i32,
    pub border_style: SubtitlesBorderStyle,
    pub shadow_offset: i32,
    pub pos: i32,
    pub scale: f32,
    pub override_ass: bool,
}

impl Default for SubtitlePrefs {
    fn default() -> Self {
        Self {
            delay_ms: 0,
            speed: 1.0,
            secondary_delay_ms: 0,
            font: "Sans Serif".to_owned(),
            font_size: 55,
            bold: false,
            italic: false,
            justify: SubtitleJustification::Auto,
            text_color_argb: -1,
            back_color_argb: 0,
            border_color_argb: -16_777_216,
            border_size: 3,
            border_style: SubtitlesBorderStyle::OutlineAndShadow,
            shadow_offset: 0,
            pos: 100,
            scale: 1.0,
            override_ass: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AudioPrefs {
    pub lang: String,
    pub delay_ms: i32,
    pub pitch_correction: bool,
    pub channels: AudioChannels,
    pub volume_boost_cap: i32,
}

impl Default for AudioPrefs {
    fn default() -> Self {
        Self {
            lang: String::new(),
            delay_ms: 0,
            pitch_correction: true,
            channels: AudioChannels::AutoSafe,
            volume_boost_cap: 30,
        }
    }
}

/// Pure input for `startup_settings`. `data_dir` holds `mpv/`, `screenshot_dir` is the
/// Pictures folder. `hwdec_available` is the candidate list probed from mpv (empty at startup).
#[derive(Clone, Debug, PartialEq)]
pub struct PlayerPrefs {
    pub try_hwdec: bool,
    pub hwdec_available: Vec<String>,
    pub gpu_next: bool,
    pub debanding: Debanding,
    pub use_yuv420p: bool,
    pub verbose_logging: bool,
    pub player_speed: f32,
    /// brightness, saturation, contrast, gamma, hue.
    pub filters: [i32; 5],
    pub sub: SubtitlePrefs,
    pub audio: AudioPrefs,
    pub data_dir: PathBuf,
    pub screenshot_dir: PathBuf,
}

impl Default for PlayerPrefs {
    fn default() -> Self {
        Self {
            try_hwdec: true,
            hwdec_available: Vec::new(),
            gpu_next: false,
            debanding: Debanding::None,
            use_yuv420p: true,
            verbose_logging: false,
            player_speed: 1.0,
            filters: [0; 5],
            sub: SubtitlePrefs::default(),
            audio: AudioPrefs::default(),
            data_dir: PathBuf::new(),
            screenshot_dir: PathBuf::new(),
        }
    }
}

/// Kotlin `Double.toString()` for values that come from `/ 1000.0` or stored floats: always a
/// fractional part, so `0` prints `0.0`. Rust's shortest round-trip form matches otherwise.
pub(crate) fn kotlin_f64(v: f64) -> String {
    let s = v.to_string();
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// Kotlin `Float.toString()`, see `kotlin_f64`.
pub(crate) fn kotlin_f32(v: f32) -> String {
    let s = v.to_string();
    if s.contains('.') { s } else { format!("{s}.0") }
}

/// Kotlin `Int.toColorHexString()`: `#` plus eight uppercase hex digits of the ARGB value.
pub fn color_hex(argb: i32) -> String {
    format!("#{:08X}", argb as u32)
}

/// Chooses the mpv `hwdec` value from the decoder and the candidates reported by mpv.
/// Copy-back candidates are tried first for `Hw`; `HwPlus` tries zero-copy first. When none of
/// the candidates is available the result is `auto` (Linux mapping, area 45 rust-design section 6).
pub fn resolve_hwdec(decoder: Decoder, available: &[String]) -> String {
    let candidates: &[&str] = match decoder {
        Decoder::AutoCopy | Decoder::Auto | Decoder::Sw => return decoder.mpv_value().to_owned(),
        Decoder::Hw => &["vaapi-copy", "nvdec-copy", "auto-copy"],
        Decoder::HwPlus => &["vaapi", "nvdec", "vaapi-copy", "nvdec-copy", "auto-copy"],
    };
    candidates
        .iter()
        .find(|c| available.iter().any(|a| a == *c))
        .map_or_else(|| "auto".to_owned(), |c| (*c).to_owned())
}

/// Video filter names and the prefs they come from (`VideoFilters` in `PlayerEnums.kt:141-171`).
pub const FILTER_NAMES: [&str; 5] = ["brightness", "saturation", "contrast", "gamma", "hue"];

/// Builds the ordered startup table of behaviour doc 2.2 (including the subtitle and audio
/// groups, and the two margin options from `PlayerActivity.kt:438-439`).
pub fn startup_settings(p: &PlayerPrefs) -> Vec<MpvSetting> {
    let mut out = Vec::new();
    let mut set = |name: &'static str, value: String| out.push(MpvSetting { name, value });

    // Config first (topic doc section 3 step 1): mpv reads `mpv.conf` and `input.conf` from the
    // config dir that `config.rs` writes, and only when `config` is on.
    set("config", "yes".to_owned());
    set("config-dir", path_text(&p.data_dir.join("mpv")));
    set("vo", if p.gpu_next { "gpu-next" } else { "gpu" }.to_owned());
    set("pause", "yes".to_owned());
    set("profile", "fast".to_owned());
    let hwdec = if p.try_hwdec {
        resolve_hwdec(Decoder::Auto, &p.hwdec_available)
    } else {
        "no".to_owned()
    };
    set("hwdec", hwdec);

    // Debanding and the yuv420p format share one `vf` list. Kotlin overwrote `vf` in that case
    // (behaviour 2.2 defect); the joined list keeps both filters.
    let mut vf: Vec<&str> = Vec::new();
    match p.debanding {
        Debanding::None => {}
        Debanding::Cpu => vf.push("gradfun=radius=12"),
        Debanding::Gpu => set("deband", "yes".to_owned()),
    }
    if p.use_yuv420p {
        vf.push("format=yuv420p");
    }
    if !vf.is_empty() {
        set("vf", vf.join(","));
    }

    set(
        "msg-level",
        if p.verbose_logging {
            "all=v"
        } else {
            "all=warn"
        }
        .to_owned(),
    );
    set("keep-open", "yes".to_owned());
    set("input-default-bindings", "yes".to_owned());
    set("ytdl", "no".to_owned());
    set("tls-verify", "yes".to_owned());
    set(
        "tls-ca-file",
        path_text(&p.data_dir.join("mpv").join("cacert.pem")),
    );
    set("demuxer-max-bytes", CACHE_BYTES.to_string());
    set("demuxer-max-back-bytes", CACHE_BYTES.to_string());
    set("screenshot-directory", path_text(&p.screenshot_dir));
    for (name, value) in FILTER_NAMES.into_iter().zip(p.filters) {
        set(name, value.to_string());
    }
    set("speed", kotlin_f32(p.player_speed));
    // Workaround for mpv issue 14651 (`AniyomiMPVView.kt:150-152`).
    set("vd-lavc-film-grain", "cpu".to_owned());

    push_subtitles(&p.sub, &mut set);
    push_audio(&p.audio, &mut set);

    set("sub-ass-force-margins", "yes".to_owned());
    set("sub-use-margins", "yes".to_owned());
    let (channels_name, channels_value) = p.audio.channels.setting();
    set(channels_name, channels_value.to_owned());
    out
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

fn push_subtitles(s: &SubtitlePrefs, set: &mut impl FnMut(&'static str, String)) {
    set("sub-delay", kotlin_f64(f64::from(s.delay_ms) / 1000.0));
    set("sub-speed", kotlin_f32(s.speed));
    set(
        "secondary-sub-delay",
        kotlin_f64(f64::from(s.secondary_delay_ms) / 1000.0),
    );
    set("sub-font", s.font.clone());
    if s.override_ass {
        set("sub-ass-override", "force".to_owned());
        set("sub-ass-justify", "yes".to_owned());
    }
    set("sub-font-size", s.font_size.to_string());
    set("sub-bold", yes_no(s.bold));
    set("sub-italic", yes_no(s.italic));
    set("sub-justify", s.justify.value().to_owned());
    set("sub-color", color_hex(s.text_color_argb));
    set("sub-back-color", color_hex(s.back_color_argb));
    set("sub-border-color", color_hex(s.border_color_argb));
    set("sub-border-size", s.border_size.to_string());
    set("sub-border-style", s.border_style.value().to_owned());
    set("sub-shadow-offset", s.shadow_offset.to_string());
    set("sub-pos", s.pos.to_string());
    set("sub-scale", kotlin_f32(s.scale));
}

fn push_audio(a: &AudioPrefs, set: &mut impl FnMut(&'static str, String)) {
    set("alang", a.lang.clone());
    set("audio-delay", kotlin_f64(f64::from(a.delay_ms) / 1000.0));
    set("audio-pitch-correction", yes_no(a.pitch_correction));
    set("volume-max", (a.volume_boost_cap + 100).to_string());
}

fn yes_no(v: bool) -> String {
    if v { "yes" } else { "no" }.to_owned()
}

/// Property writes for a channel layout picked at runtime (`MoreSheet.kt:246-256`). The reverse
/// layout first resets `audio-channels` to auto. Any other layout clears the `af` filter first,
/// then writes its own property. Both cases end with the layout's own `setting()`.
pub fn channel_change_writes(next: AudioChannels) -> Vec<(&'static str, &'static str)> {
    match next {
        AudioChannels::ReverseStereo => vec![
            AudioChannels::AutoSafe.setting(),
            AudioChannels::ReverseStereo.setting(),
        ],
        other => vec![("af", ""), other.setting()],
    }
}

/// Looks a setting up by name in an ordered table (the last value wins, as mpv does).
pub fn setting_value<'a>(settings: &'a [MpvSetting], name: &str) -> Option<&'a str> {
    settings
        .iter()
        .rev()
        .find(|s| s.name == name)
        .map(|s| s.value.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefs() -> PlayerPrefs {
        PlayerPrefs {
            data_dir: PathBuf::from("/data"),
            screenshot_dir: PathBuf::from("/pics"),
            ..PlayerPrefs::default()
        }
    }

    fn pairs(list: &[MpvSetting]) -> Vec<(&'static str, String)> {
        list.iter().map(|s| (s.name, s.value.clone())).collect()
    }

    fn golden_defaults() -> Vec<(&'static str, &'static str)> {
        vec![
            ("config", "yes"),
            ("config-dir", "/data/mpv"),
            ("vo", "gpu"),
            ("pause", "yes"),
            ("profile", "fast"),
            ("hwdec", "auto"),
            ("vf", "format=yuv420p"),
            ("msg-level", "all=warn"),
            ("keep-open", "yes"),
            ("input-default-bindings", "yes"),
            ("ytdl", "no"),
            ("tls-verify", "yes"),
            ("tls-ca-file", "/data/mpv/cacert.pem"),
            ("demuxer-max-bytes", "67108864"),
            ("demuxer-max-back-bytes", "67108864"),
            ("screenshot-directory", "/pics"),
            ("brightness", "0"),
            ("saturation", "0"),
            ("contrast", "0"),
            ("gamma", "0"),
            ("hue", "0"),
            ("speed", "1.0"),
            ("vd-lavc-film-grain", "cpu"),
            ("sub-delay", "0.0"),
            ("sub-speed", "1.0"),
            ("secondary-sub-delay", "0.0"),
            ("sub-font", "Sans Serif"),
            ("sub-font-size", "55"),
            ("sub-bold", "no"),
            ("sub-italic", "no"),
            ("sub-justify", "auto"),
            ("sub-color", "#FFFFFFFF"),
            ("sub-back-color", "#00000000"),
            ("sub-border-color", "#FF000000"),
            ("sub-border-size", "3"),
            ("sub-border-style", "outline-and-shadow"),
            ("sub-shadow-offset", "0"),
            ("sub-pos", "100"),
            ("sub-scale", "1.0"),
            ("alang", ""),
            ("audio-delay", "0.0"),
            ("audio-pitch-correction", "yes"),
            ("volume-max", "130"),
            ("sub-ass-force-margins", "yes"),
            ("sub-use-margins", "yes"),
            ("audio-channels", "auto"),
        ]
    }

    #[test]
    fn startup_defaults_match_golden() {
        let got = pairs(&startup_settings(&prefs()));
        let want: Vec<(&str, String)> = golden_defaults()
            .into_iter()
            .map(|(n, v)| (n, v.to_owned()))
            .collect();
        assert_eq!(got, want);
    }

    #[test]
    fn debanding_cpu_with_yuv420p_joins_vf() {
        let p = PlayerPrefs {
            debanding: Debanding::Cpu,
            ..prefs()
        };
        assert_eq!(
            setting_value(&startup_settings(&p), "vf"),
            Some("gradfun=radius=12,format=yuv420p")
        );
    }

    #[test]
    fn debanding_cpu_without_yuv420p_is_gradfun_only() {
        let p = PlayerPrefs {
            debanding: Debanding::Cpu,
            use_yuv420p: false,
            ..prefs()
        };
        assert_eq!(
            setting_value(&startup_settings(&p), "vf"),
            Some("gradfun=radius=12")
        );
    }

    #[test]
    fn debanding_gpu_sets_deband_and_no_vf_when_format_off() {
        let p = PlayerPrefs {
            debanding: Debanding::Gpu,
            use_yuv420p: false,
            ..prefs()
        };
        let s = startup_settings(&p);
        assert_eq!(setting_value(&s, "deband"), Some("yes"));
        assert_eq!(setting_value(&s, "vf"), None);
    }

    #[test]
    fn gpu_next_selects_vo() {
        let p = PlayerPrefs {
            gpu_next: true,
            ..prefs()
        };
        assert_eq!(setting_value(&startup_settings(&p), "vo"), Some("gpu-next"));
    }

    #[test]
    fn hwdec_flag_false_is_no() {
        let p = PlayerPrefs {
            try_hwdec: false,
            ..prefs()
        };
        assert_eq!(setting_value(&startup_settings(&p), "hwdec"), Some("no"));
    }

    #[test]
    fn resolve_hwdec_picks_first_candidate_and_falls_back_to_auto() {
        let avail = vec!["nvdec-copy".to_owned(), "auto-copy".to_owned()];
        assert_eq!(resolve_hwdec(Decoder::Hw, &avail), "nvdec-copy");
        assert_eq!(resolve_hwdec(Decoder::Hw, &[]), "auto");
        assert_eq!(resolve_hwdec(Decoder::Sw, &avail), "no");
        assert_eq!(resolve_hwdec(Decoder::AutoCopy, &[]), "auto-copy");
    }

    #[test]
    fn speed_formats_with_point_zero() {
        let p = PlayerPrefs {
            player_speed: 2.0,
            ..prefs()
        };
        assert_eq!(setting_value(&startup_settings(&p), "speed"), Some("2.0"));
    }

    #[test]
    fn audio_delay_ms_to_seconds() {
        let p = PlayerPrefs {
            audio: AudioPrefs {
                delay_ms: -250,
                ..AudioPrefs::default()
            },
            ..prefs()
        };
        assert_eq!(
            setting_value(&startup_settings(&p), "audio-delay"),
            Some("-0.25")
        );
    }

    #[test]
    fn volume_boost_cap_adds_100() {
        let p = PlayerPrefs {
            audio: AudioPrefs {
                volume_boost_cap: 50,
                ..AudioPrefs::default()
            },
            ..prefs()
        };
        assert_eq!(
            setting_value(&startup_settings(&p), "volume-max"),
            Some("150")
        );
    }

    #[test]
    fn subtitle_colors_hex() {
        assert_eq!(color_hex(-1), "#FFFFFFFF");
        assert_eq!(color_hex(0), "#00000000");
        assert_eq!(color_hex(-16_777_216), "#FF000000");
    }

    #[test]
    fn subtitle_delay_formats_seconds() {
        let mut p = prefs();
        p.sub.delay_ms = 1500;
        assert_eq!(
            setting_value(&startup_settings(&p), "sub-delay"),
            Some("1.5")
        );
    }

    #[test]
    fn font_default_sans_serif() {
        assert_eq!(
            setting_value(&startup_settings(&prefs()), "sub-font"),
            Some("Sans Serif")
        );
    }

    #[test]
    fn ass_override_only_when_enabled() {
        let off = startup_settings(&prefs());
        assert_eq!(setting_value(&off, "sub-ass-override"), None);
        assert_eq!(setting_value(&off, "sub-ass-justify"), None);
        let mut p = prefs();
        p.sub.override_ass = true;
        let on = startup_settings(&p);
        assert_eq!(setting_value(&on, "sub-ass-override"), Some("force"));
        assert_eq!(setting_value(&on, "sub-ass-justify"), Some("yes"));
    }

    #[test]
    fn cache_limits_are_64_mib() {
        let s = startup_settings(&prefs());
        assert_eq!(setting_value(&s, "demuxer-max-bytes"), Some("67108864"));
        assert_eq!(
            setting_value(&s, "demuxer-max-back-bytes"),
            Some("67108864")
        );
    }

    #[test]
    fn filters_default_zero_and_write_names() {
        let p = PlayerPrefs {
            filters: [10, -20, 0, 5, -1],
            ..prefs()
        };
        let s = startup_settings(&p);
        assert_eq!(setting_value(&s, "brightness"), Some("10"));
        assert_eq!(setting_value(&s, "saturation"), Some("-20"));
        assert_eq!(setting_value(&s, "hue"), Some("-1"));
    }

    #[test]
    fn audio_channel_values_map_to_property_and_value() {
        let cases = [
            (AudioChannels::Auto, "audio-channels", "auto-safe"),
            (AudioChannels::AutoSafe, "audio-channels", "auto"),
            (AudioChannels::Mono, "audio-channels", "mono"),
            (AudioChannels::Stereo, "audio-channels", "stereo"),
            (
                AudioChannels::ReverseStereo,
                "af",
                "pan=[stereo|c0=c1|c1=c0]",
            ),
        ];
        for (channels, name, value) in cases {
            let p = PlayerPrefs {
                audio: AudioPrefs {
                    channels,
                    ..AudioPrefs::default()
                },
                ..prefs()
            };
            assert_eq!(setting_value(&startup_settings(&p), name), Some(value));
        }
    }

    #[test]
    fn runtime_channel_change_for_reverse_resets_auto_first() {
        assert_eq!(
            channel_change_writes(AudioChannels::ReverseStereo),
            vec![
                ("audio-channels", "auto"),
                ("af", "pan=[stereo|c0=c1|c1=c0]"),
            ]
        );
    }

    #[test]
    fn runtime_channel_change_for_others_clears_af_first() {
        assert_eq!(
            channel_change_writes(AudioChannels::Mono),
            vec![("af", ""), ("audio-channels", "mono")]
        );
    }

    #[test]
    fn verbose_logging_selects_msg_level() {
        let p = PlayerPrefs {
            verbose_logging: true,
            ..prefs()
        };
        assert_eq!(
            setting_value(&startup_settings(&p), "msg-level"),
            Some("all=v")
        );
    }
}
