//! Bundled tracker logos (T-33a-100, T-27a-072). The PNG files live in `crates/kuro-track/assets/tracker/` and are
//! compiled in with `include_bytes!`, so every frontend can draw them without a resource bundle. A tracker without a
//! bundled logo gets `None`, and the sheet draws its letter tile instead ([`crate::sheet::logo_fallback_letter`]).

use crate::model::TrackerId;
use crate::tracker::LogoRef;

const MYANIMELIST: &[u8] = include_bytes!("../assets/tracker/ic_tracker_mal.png");
const ANILIST: &[u8] = include_bytes!("../assets/tracker/ic_tracker_anilist.png");
const KITSU: &[u8] = include_bytes!("../assets/tracker/ic_tracker_kitsu.png");
const SHIKIMORI: &[u8] = include_bytes!("../assets/tracker/ic_tracker_shikimori.png");
const BANGUMI: &[u8] = include_bytes!("../assets/tracker/ic_tracker_bangumi.png");
const KOMGA: &[u8] = include_bytes!("../assets/tracker/ic_tracker_komga.png");
const KAVITA: &[u8] = include_bytes!("../assets/tracker/ic_tracker_kavita.png");
const SUWAYOMI: &[u8] = include_bytes!("../assets/tracker/ic_tracker_suwayomi.png");
const JELLYFIN: &[u8] = include_bytes!("../assets/tracker/ic_tracker_jellyfin.png");
const SIMKL: &[u8] = include_bytes!("../assets/tracker/ic_tracker_simkl.png");

/// The bundled logo of a tracker, or `None` when none is bundled (Mangaupdates has no logo asset).
pub fn logo_asset(id: TrackerId) -> Option<LogoRef> {
    let bytes = match id {
        TrackerId::MYANIMELIST => MYANIMELIST,
        TrackerId::ANILIST => ANILIST,
        TrackerId::KITSU => KITSU,
        TrackerId::SHIKIMORI => SHIKIMORI,
        TrackerId::BANGUMI => BANGUMI,
        TrackerId::KOMGA => KOMGA,
        TrackerId::KAVITA => KAVITA,
        TrackerId::SUWAYOMI => SUWAYOMI,
        TrackerId::JELLYFIN => JELLYFIN,
        TrackerId::SIMKL => SIMKL,
        _ => return None,
    };
    Some(LogoRef::Embedded(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [TrackerId; 11] = [
        TrackerId::MYANIMELIST,
        TrackerId::ANILIST,
        TrackerId::KITSU,
        TrackerId::SHIKIMORI,
        TrackerId::BANGUMI,
        TrackerId::MANGAUPDATES,
        TrackerId::SIMKL,
        TrackerId::KAVITA,
        TrackerId::KOMGA,
        TrackerId::SUWAYOMI,
        TrackerId::JELLYFIN,
    ];

    #[test]
    fn t_33a_100_each_bundled_logo_is_a_png() {
        for id in ALL {
            if let Some(LogoRef::Embedded(bytes)) = logo_asset(id) {
                assert_eq!(&bytes[..4], b"\x89PNG", "tracker {}", id.0);
            }
        }
    }

    #[test]
    fn t_33a_100_the_four_enhanced_logos_and_the_others_are_found_by_id() {
        for id in [
            TrackerId::KAVITA,
            TrackerId::KOMGA,
            TrackerId::SUWAYOMI,
            TrackerId::JELLYFIN,
        ] {
            assert!(matches!(logo_asset(id), Some(LogoRef::Embedded(_))));
        }
    }

    #[test]
    fn t_27a_072_a_tracker_without_an_asset_falls_back_to_none() {
        assert!(logo_asset(TrackerId::MANGAUPDATES).is_none());
        assert!(logo_asset(TrackerId(999)).is_none());
    }

    #[test]
    fn t_27a_072_ten_of_the_eleven_trackers_have_a_bundled_logo() {
        let found = ALL.iter().filter(|id| logo_asset(**id).is_some()).count();
        assert_eq!(found, 10);
    }
}
