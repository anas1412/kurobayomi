//! Video aspect helpers (area 45: `AniyomiMPVView.kt:88-93` `getVideoOutAspect`; part 2 behaviour
//! 9.3 `changeVideoAspect`; T-45a-034). This module only computes values and property writes. The
//! app sends them to mpv, and the GL area applies them.

use kuro_i18n::{MsgKey, tr};

use crate::enums::VideoAspect;
use crate::player_prefs::{PrefStore, enum_text, get_enum, keys};
use crate::pref_write::Write;

/// `getVideoOutAspect()`. `None` when `video-params/aspect` is unknown. Below 0.001 the aspect is
/// `0`. A 90 or 270 degree rotation swaps width and height, so the aspect is inverted.
pub fn video_out_aspect(aspect: Option<f64>, rotate: Option<i64>) -> Option<f64> {
    let aspect = aspect?;
    if aspect < 0.001 {
        return Some(0.0);
    }
    if rotate.unwrap_or(0) % 180 == 90 {
        Some(1.0 / aspect)
    } else {
        Some(aspect)
    }
}

/// The two mpv properties that a mode writes, in write order: `panscan`, then
/// `video-aspect-override`. `stretch_ratio` is the display width over height, used only by
/// `Stretch` (behaviour 9.3, real monitor metrics on Linux).
pub fn aspect_property_writes(mode: VideoAspect, stretch_ratio: f64) -> [(&'static str, f64); 2] {
    let (pan, ratio) = match mode {
        VideoAspect::Crop => (1.0, -1.0),
        VideoAspect::Fit => (0.0, -1.0),
        VideoAspect::Stretch => (0.0, stretch_ratio),
    };
    [("panscan", pan), ("video-aspect-override", ratio)]
}

/// The catalogue key of the pill text of a mode (`video_fit_screen`, `video_crop_screen`,
/// `video_stretch_screen`).
pub const fn pill_key(mode: VideoAspect) -> MsgKey {
    match mode {
        VideoAspect::Fit => MsgKey::video_fit_screen,
        VideoAspect::Crop => MsgKey::video_crop_screen,
        VideoAspect::Stretch => MsgKey::video_stretch_screen,
    }
}

/// The pill text of a mode, in the active language.
pub fn pill_text(mode: VideoAspect) -> String {
    tr(pill_key(mode))
}

/// The stored aspect mode (`pref_player_aspect_state`). Missing or unknown gives `Fit`.
pub fn stored_aspect(store: &impl PrefStore) -> VideoAspect {
    get_enum(store, keys::PREF_PLAYER_ASPECT_STATE)
}

/// `changeVideoAspect`: the preference write (the mode name) followed by the two mpv writes.
pub fn change_aspect_writes(mode: VideoAspect, stretch_ratio: f64) -> Vec<Write> {
    let mut writes = vec![Write::pref(keys::PREF_PLAYER_ASPECT_STATE, enum_text(mode))];
    writes.extend(
        aspect_property_writes(mode, stretch_ratio)
            .into_iter()
            .map(|(name, value)| Write::mpv(name, value.to_string())),
    );
    writes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player_prefs::NamedEnum;
    use std::collections::HashMap;

    /// T50 to T52
    #[test]
    fn each_mode_has_its_own_pill_string() {
        assert_eq!(pill_text(VideoAspect::Fit), "Fit to screen");
        assert_eq!(pill_text(VideoAspect::Crop), "Cropped to screen");
        assert_eq!(pill_text(VideoAspect::Stretch), "Stretched to screen");
    }

    #[test]
    fn the_mode_is_stored_by_name_and_read_back() {
        let writes = change_aspect_writes(VideoAspect::Stretch, 1.78);
        assert_eq!(
            writes[0],
            Write::pref("pref_player_aspect_state", "Stretch")
        );
        assert_eq!(writes[1], Write::mpv("panscan", "0"));
        assert_eq!(writes[2], Write::mpv("video-aspect-override", "1.78"));
        let mut store = HashMap::new();
        assert_eq!(stored_aspect(&store), VideoAspect::Fit);
        store.insert("pref_player_aspect_state".to_owned(), "Stretch".to_owned());
        assert_eq!(stored_aspect(&store), VideoAspect::Stretch);
        assert_eq!(VideoAspect::from_name("Crop"), Some(VideoAspect::Crop));
    }

    #[test]
    fn unknown_aspect_gives_none() {
        assert_eq!(video_out_aspect(None, Some(90)), None);
    }

    #[test]
    fn tiny_aspect_gives_zero() {
        assert_eq!(video_out_aspect(Some(0.0005), None), Some(0.0));
        assert_eq!(video_out_aspect(Some(0.001), None), Some(0.001));
    }

    #[test]
    fn quarter_turn_inverts_the_aspect() {
        assert_eq!(video_out_aspect(Some(2.0), Some(90)), Some(0.5));
        assert_eq!(video_out_aspect(Some(2.0), Some(270)), Some(0.5));
        assert_eq!(video_out_aspect(Some(2.0), Some(180)), Some(2.0));
        assert_eq!(video_out_aspect(Some(2.0), Some(0)), Some(2.0));
        assert_eq!(video_out_aspect(Some(2.0), None), Some(2.0));
    }

    #[test]
    fn crop_fit_and_stretch_write_their_values() {
        assert_eq!(
            aspect_property_writes(VideoAspect::Crop, 1.78),
            [("panscan", 1.0), ("video-aspect-override", -1.0)]
        );
        assert_eq!(
            aspect_property_writes(VideoAspect::Fit, 1.78),
            [("panscan", 0.0), ("video-aspect-override", -1.0)]
        );
        assert_eq!(
            aspect_property_writes(VideoAspect::Stretch, 1.78),
            [("panscan", 0.0), ("video-aspect-override", 1.78)]
        );
    }
}
