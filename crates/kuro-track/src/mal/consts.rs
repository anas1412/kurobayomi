//! Tracker id and endpoint constants (`p1-behavior.md` section 0, `p1-data-and-config.md` section 8).
//! The OAuth client id is not a constant: it comes from the user's config (`p1-rust-design.md` section 5).

use crate::model::TrackerId;

/// `TrackerManager.kt:27`: MAL is tracker 1.
pub const MAL_TRACKER_ID: TrackerId = TrackerId::MYANIMELIST;

pub const OAUTH_BASE: &str = "https://myanimelist.net/v1/oauth2";
pub const API_BASE: &str = "https://api.myanimelist.net/v2";

/// Page size of `users/@me/{mangalist,animelist}` requests (`MyAnimeListApi.kt:388`).
pub const PAGE_SIZE: u32 = 250;
/// Longest search query sent, counted in UTF-16 units (`MyAnimeListApi.kt:85`).
pub const QUERY_MAX: usize = 64;
/// Detail requests running at once for search and `my:` results (D-13 proposal).
pub const DETAIL_CONCURRENCY: usize = 4;

pub const SEARCH_ID_PREFIX: &str = "id:";
pub const SEARCH_LIST_PREFIX: &str = "my:";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_and_bases_are_fixed_strings() {
        assert_eq!(MAL_TRACKER_ID.0, 1);
        assert_eq!(OAUTH_BASE, "https://myanimelist.net/v1/oauth2");
        assert_eq!(API_BASE, "https://api.myanimelist.net/v2");
        assert_eq!(PAGE_SIZE, 250);
        assert_eq!(QUERY_MAX, 64);
    }
}
