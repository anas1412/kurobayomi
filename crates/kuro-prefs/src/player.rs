//! `[player]` preferences (Aniyomi `PlayerPreferences`) and the
//! "mark as seen" rule.
//!
//! The settings diet kept two player keys here, the volume and the autoplay switch. The gesture, decoder, aspect,
//! speed, subtitle, advanced player and detach-window keys were cut: their hard-coded defaults are read where they
//! are used, and the keys are in `registry::DROPPED_KEYS`.

use crate::registry::define_prefs;

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
    /// `[player]` section: the player keys the settings diet kept.
    pub struct PlayerPrefs in "player" {
        player_volume_value: f32 = "player_volume_value" => -1.0f32;
        autoplay_enabled: bool = "pref_auto_play_enabled" => false;
        /// The video height an episode starts at when its servers offer it (user, 2026-10-15: 1080p by default).
        default_quality: i32 = "pref_player_default_quality" => 1080;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::lookup;
    use crate::store::PrefStore;
    use crate::testutil::assert_section_registered;

    #[test]
    fn every_player_section_matches_its_registry_rows() {
        let store = PrefStore::in_memory();
        let player = PlayerPrefs::new(store.clone());
        assert_eq!(PlayerPrefs::FIELDS.len(), 3);
        assert_section_registered(PlayerPrefs::TABLE, PlayerPrefs::FIELDS, &player.defaults());
    }

    #[test]
    fn plan_defaults_hold_on_an_empty_store() {
        let p = PlayerPrefs::new(PrefStore::in_memory());
        assert_eq!(p.player_volume_value.get(), -1.0);
        assert!(!p.autoplay_enabled.get());
        assert_eq!(p.default_quality.get(), 1080);
    }

    #[test]
    fn cut_player_keys_are_dropped_not_registered() {
        for key in [
            "pref_default_player_orientation_type_key",
            "pref_gesture_volume_brightness",
            "pref_swap_volume_and_brightness",
            "pref_remember_brightness",
            "player_brightness_value",
            "pref_mpv_conf",
            "pref_mpv_input",
            "mpv_scripts",
            "player_open_detached",
        ] {
            assert!(lookup(key).is_none(), "{key} is registered");
            assert!(crate::registry::is_dropped(key), "{key} is not in DROPPED_KEYS");
        }
    }

    #[test]
    fn mark_seen_needs_a_known_length_an_unseen_episode_and_no_incognito() {
        assert!(should_mark_seen(850, 1000, 0.85, false, false));
        assert!(!should_mark_seen(849, 1000, 0.85, false, false));
        assert!(!should_mark_seen(1000, 0, 0.85, false, false));
        assert!(!should_mark_seen(1000, 1000, 0.85, true, false));
        assert!(!should_mark_seen(1000, 1000, 0.85, false, true));
    }
}
