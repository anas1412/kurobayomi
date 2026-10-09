//! Video aspect helpers (area 45: `AniyomiMPVView.kt:88-93` `getVideoOutAspect`; part 2 behaviour
//! 9.3 `changeVideoAspect`; T-45a-034). This module only computes values and property writes. The
//! app sends them to mpv, and the GL area applies them.

use crate::enums::VideoAspect;

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

#[cfg(test)]
mod tests {
    use super::*;

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
