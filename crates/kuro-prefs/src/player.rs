//! `[player]`, `[player.gestures]` and `[player.decoder]` preferences (Aniyomi `PlayerPreferences`,
//! `GesturePreferences`, `DecoderPreferences`), the player enums, the mpv decoder options and the
//! "mark as seen" rule.
//!
//! Subtitle, audio and advanced player keys live in their own sections (`player.subtitles`,
//! `player.audio`, `player.advanced`), not here.

use std::collections::BTreeSet;

use crate::pref_enum;
use crate::registry::define_prefs;

pref_enum! {
    /// Orientation the player locks to (`PlayerEnums.kt`, `PlayerOrientation`).
    pub enum PlayerOrientation { Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape }
}

pref_enum! {
    /// Video aspect mode (`VideoAspect`).
    pub enum VideoAspect { Crop, Fit, Stretch }
}

pref_enum! {
    /// Action of a double tap or media button (`SingleActionGesture`).
    pub enum SingleActionGesture { None, Seek, PlayPause, Switch, Custom }
}

pref_enum! {
    /// Debanding filter (`Debanding`). CPU adds `gradfun`, GPU sets mpv `deband`.
    pub enum Debanding { None, CPU, GPU }
}

/// Key codes sent through the `Custom` gesture option (`CustomKeyCodes`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CustomKeyCodes {
    DoubleTapLeft,
    DoubleTapCenter,
    DoubleTapRight,
    MediaPrevious,
    MediaPlay,
    MediaNext,
}

impl CustomKeyCodes {
    pub const fn key_code(self) -> &'static str {
        match self {
            Self::DoubleTapLeft => "0x10001",
            Self::DoubleTapCenter => "0x10002",
            Self::DoubleTapRight => "0x10003",
            Self::MediaPrevious => "0x10004",
            Self::MediaPlay => "0x10005",
            Self::MediaNext => "0x10006",
        }
    }
}

/// The mpv options the decoder preferences produce, in the order the player sets them.
///
/// `hwdec` is `auto-safe` when hardware decoding is on and `no` otherwise. A `vf` chain carries
/// `format=yuv420p` when YUV420P is forced and `gradfun=radius=12` for CPU debanding. Both go into
/// one chain, because a second `vf` option would replace the first (quirk Q9).
pub fn mpv_decoder_options(try_hw: bool, debanding: Debanding, use_yuv420p: bool) -> Vec<(&'static str, String)> {
    let mut options = vec![("hwdec", if try_hw { "auto-safe" } else { "no" }.to_owned())];
    if debanding == Debanding::GPU {
        options.push(("deband", "yes".to_owned()));
    }
    let mut filters = Vec::new();
    if use_yuv420p {
        filters.push("format=yuv420p");
    }
    if debanding == Debanding::CPU {
        filters.push("gradfun=radius=12");
    }
    if !filters.is_empty() {
        options.push(("vf", filters.join(",")));
    }
    options
}

/// Whether finishing playback at `last_second_seen` marks the episode as seen (`PlayerViewModel`).
///
/// True when the episode is unseen, `incognito` is off, the total length is known (greater than zero),
/// and the position has reached `progress` of the total.
pub fn should_mark_seen(last_second_seen: i64, total_seconds: i64, progress: f32, already_seen: bool, incognito: bool) -> bool {
    !already_seen
        && !incognito
        && total_seconds > 0
        && last_second_seen as f32 >= total_seconds as f32 * progress
}

define_prefs! {
    /// `[player]` section: the 36 keys of Aniyomi `PlayerPreferences`.
    pub struct PlayerPrefs in "player" {
        preserve_watching_position: bool = "pref_preserve_watching_position" => false;
        progress_preference: f32 = "pref_progress_preference" => 0.85f32;
        default_player_orientation_type: PlayerOrientation = "pref_default_player_orientation_type_key" => PlayerOrientation::SensorLandscape;
        allow_gestures: bool = "pref_allow_gestures_in_panels" => false;
        show_loading_circle: bool = "pref_show_loading" => true;
        show_current_chapter: bool = "pref_show_current_chapter" => true;
        remember_player_brightness: bool = "pref_remember_brightness" => false;
        player_brightness_value: f32 = "player_brightness_value" => -1.0f32;
        remember_player_volume: bool = "pref_remember_volume" => false;
        player_volume_value: f32 = "player_volume_value" => -1.0f32;
        show_failed_hosters: bool = "pref_show_failed_hosters" => false;
        show_empty_hosters: bool = "pref_show_empty_hosters" => false;
        player_fullscreen: bool = "player_fullscreen" => true;
        hide_controls: bool = "player_hide_controls" => false;
        display_vol_per: bool = "pref_display_vol_as_per" => true;
        show_system_status_bar: bool = "pref_show_system_status_bar" => false;
        reduce_motion: bool = "pref_reduce_motion" => false;
        player_time_to_disappear: i32 = "pref_player_time_to_disappear" => 4000;
        panel_opacity: i32 = "pref_panel_opacity" => 60;
        enable_skip_intro: bool = "pref_enable_skip_intro" => true;
        auto_skip_intro: bool = "pref_enable_auto_skip_ani_skip" => false;
        enable_netflix_style_intro_skip: bool = "pref_enable_netflixStyle_aniskip" => false;
        waiting_time_intro_skip: i32 = "pref_waiting_time_aniskip" => 5;
        ani_skip_enabled: bool = "pref_enable_ani_skip" => false;
        disable_ani_skip_on_chapters: bool = "pref_disabled_ani_skip_chapters" => true;
        enable_pip: bool = "pref_enable_pip" => true;
        pip_episode_toasts: bool = "pref_pip_episode_toasts" => true;
        pip_on_exit: bool = "pref_pip_on_exit" => false;
        pip_replace_with_previous: bool = "pip_replace_with_previous" => false;
        always_use_external_player: bool = "pref_always_use_external_player" => false;
        external_player_preference: String = "external_player_preference" => String::from("");
        player_speed: f32 = "pref_player_speed" => 1.0f32;
        speed_presets: BTreeSet<String> = "default_speed_presets" => BTreeSet::from(["0.25".to_owned(), "0.5".to_owned(), "0.75".to_owned(), "1.0".to_owned(), "1.25".to_owned(), "1.5".to_owned(), "1.75".to_owned(), "2.0".to_owned(), "2.5".to_owned(), "3.0".to_owned(), "3.5".to_owned(), "4.0".to_owned()]);
        invert_duration: bool = "invert_duration" => false;
        aspect_state: VideoAspect = "pref_player_aspect_state" => VideoAspect::Fit;
        autoplay_enabled: bool = "pref_auto_play_enabled" => false;
    }
}

define_prefs! {
    /// `[player.gestures]` section: the 13 keys of Aniyomi `GesturePreferences`.
    pub struct GesturePrefs in "player.gestures" {
        gesture_volume_brightness: bool = "pref_gesture_volume_brightness" => true;
        swap_volume_brightness: bool = "pref_swap_volume_and_brightness" => false;
        gesture_horizontal_seek: bool = "pref_gesture_horizontal_seek" => true;
        show_seek_bar: bool = "pref_show_seekbar" => false;
        default_intro_length: i32 = "pref_default_intro_length" => 85;
        skip_length_preference: i32 = "pref_skip_length_preference" => 10;
        player_smooth_seek: bool = "pref_player_smooth_seek" => false;
        left_double_tap_gesture: SingleActionGesture = "pref_left_double_tap" => SingleActionGesture::Seek;
        center_double_tap_gesture: SingleActionGesture = "pref_center_double_tap" => SingleActionGesture::PlayPause;
        right_double_tap_gesture: SingleActionGesture = "pref_right_double_tap" => SingleActionGesture::Seek;
        media_previous_gesture: SingleActionGesture = "pref_media_previous" => SingleActionGesture::Switch;
        media_play_pause_gesture: SingleActionGesture = "pref_media_playpause" => SingleActionGesture::PlayPause;
        media_next_gesture: SingleActionGesture = "pref_media_next" => SingleActionGesture::Switch;
    }
}

define_prefs! {
    /// `[player.decoder]` section: the 9 keys of Aniyomi `DecoderPreferences`, and the two Kurobayomi keys that
    /// remember the chosen decoder (T-46b-043, opt-in, off by default).
    pub struct DecoderPrefs in "player.decoder" {
        try_hw_decoding: bool = "pref_try_hwdec" => true;
        gpu_next: bool = "pref_gpu_next" => false;
        video_debanding: Debanding = "pref_video_debanding" => Debanding::None;
        use_yuv420p: bool = "use_yuv420p" => true;
        brightness_filter: i32 = "pref_player_filter_brightness" => 0;
        saturation_filter: i32 = "pref_player_filter_saturation" => 0;
        contrast_filter: i32 = "pref_player_filter_contrast" => 0;
        gamma_filter: i32 = "pref_player_filter_gamma" => 0;
        hue_filter: i32 = "pref_player_filter_hue" => 0;
        remember_decoder: bool = "pref_remember_decoder" => false;
        remembered_decoder: String = "player_decoder" => String::new();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::REGISTRY;
    use crate::store::PrefStore;
    use crate::testutil::assert_section_registered;
    use crate::values::{Decoded, PrefType, PrefValue};

    #[test]
    fn every_player_section_matches_its_registry_rows() {
        let store = PrefStore::in_memory();
        let player = PlayerPrefs::new(store.clone());
        assert_eq!(PlayerPrefs::FIELDS.len(), 36);
        assert_section_registered(PlayerPrefs::TABLE, PlayerPrefs::FIELDS, &player.defaults());
        let gestures = GesturePrefs::new(store.clone());
        assert_eq!(GesturePrefs::FIELDS.len(), 13);
        assert_section_registered(GesturePrefs::TABLE, GesturePrefs::FIELDS, &gestures.defaults());
        let decoder = DecoderPrefs::new(store);
        assert_eq!(DecoderPrefs::FIELDS.len(), 11);
        assert_section_registered(DecoderPrefs::TABLE, DecoderPrefs::FIELDS, &decoder.defaults());
    }

    #[test]
    fn plan_defaults_hold_on_an_empty_store() {
        let store = PrefStore::in_memory();
        let p = PlayerPrefs::new(store.clone());
        assert_eq!(p.progress_preference.get(), 0.85);
        assert_eq!(p.player_time_to_disappear.get(), 4000);
        assert_eq!(p.panel_opacity.get(), 60);
        assert_eq!(p.aspect_state.get(), VideoAspect::Fit);
        assert_eq!(p.default_player_orientation_type.get(), PlayerOrientation::SensorLandscape);
        assert_eq!(p.speed_presets.get().len(), 12);
        let g = GesturePrefs::new(store.clone());
        assert_eq!(g.default_intro_length.get(), 85);
        assert_eq!(g.skip_length_preference.get(), 10);
        assert_eq!(g.left_double_tap_gesture.get(), SingleActionGesture::Seek);
        assert_eq!(g.center_double_tap_gesture.get(), SingleActionGesture::PlayPause);
        assert_eq!(g.media_next_gesture.get(), SingleActionGesture::Switch);
        let d = DecoderPrefs::new(store);
        assert!(d.try_hw_decoding.get());
        assert!(d.use_yuv420p.get());
        assert_eq!(d.video_debanding.get(), Debanding::None);
        assert_eq!(d.brightness_filter.get(), 0);
    }

    #[test]
    fn decoder_options_for_each_debanding_mode() {
        assert_eq!(
            mpv_decoder_options(true, Debanding::None, false),
            vec![("hwdec", "auto-safe".to_owned())]
        );
        assert_eq!(
            mpv_decoder_options(false, Debanding::GPU, false),
            vec![("hwdec", "no".to_owned()), ("deband", "yes".to_owned())]
        );
    }

    #[test]
    fn cpu_debanding_and_yuv420p_share_one_vf_chain() {
        let options = mpv_decoder_options(true, Debanding::CPU, true);
        let vf: Vec<&String> = options.iter().filter(|(k, _)| *k == "vf").map(|(_, v)| v).collect();
        assert_eq!(vf.len(), 1, "a second vf option would replace the first");
        assert!(vf[0].contains("format=yuv420p") && vf[0].contains("gradfun=radius=12"));
    }

    #[test]
    fn mark_seen_needs_a_known_length_an_unseen_episode_and_no_incognito() {
        assert!(should_mark_seen(850, 1000, 0.85, false, false));
        assert!(!should_mark_seen(849, 1000, 0.85, false, false));
        assert!(!should_mark_seen(1000, 0, 0.85, false, false));
        assert!(!should_mark_seen(1000, 1000, 0.85, true, false));
        assert!(!should_mark_seen(1000, 1000, 0.85, false, true));
    }

    #[test]
    fn enums_are_stored_by_name_and_custom_codes_are_fixed() {
        assert_eq!(PlayerOrientation::SensorLandscape.encode(), PrefValue::Str("SensorLandscape".into()));
        assert_eq!(PlayerOrientation::ALL.len(), 8);
        assert_eq!(VideoAspect::ALL.len(), 3);
        assert_eq!(SingleActionGesture::ALL.len(), 5);
        assert_eq!(Debanding::decode(&PrefValue::Str("GPU".into())), Decoded::Ok(Debanding::GPU));
        assert_eq!(CustomKeyCodes::MediaNext.key_code(), "0x10006");
        assert_eq!(CustomKeyCodes::DoubleTapLeft.key_code(), "0x10001");
        assert!(REGISTRY.iter().any(|m| m.key == "pref_enable_pip"));
    }
}
