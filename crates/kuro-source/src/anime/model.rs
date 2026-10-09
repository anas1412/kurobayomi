//! Anime models: `SAnime`, `SEpisode`, `AnimeStatus`, `FetchType` and the listing page (`SAnime.kt`, `SAnimeImpl.kt`,
//! `SEpisode.kt`, `FetchType.kt`, `AnimesPage.kt`; T-16a-006, T-16a-007, T-16a-009, T-16a-013).

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{Map, Value};

use super::url::url_without_domain;
use crate::json::null_as_default;
use crate::model::UpdateStrategy;
use crate::model::manga::parse_genres;

/// Publication status. The integers are the stored values; `Other` keeps a value written by a newer version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "i32", into = "i32")]
pub enum AnimeStatus {
    #[default]
    Unknown,
    Ongoing,
    Completed,
    Licensed,
    PublishingFinished,
    Cancelled,
    OnHiatus,
    Upcoming,
    Other(i32),
}

impl AnimeStatus {
    pub fn from_i32(value: i32) -> Self {
        match value {
            0 => Self::Unknown,
            1 => Self::Ongoing,
            2 => Self::Completed,
            3 => Self::Licensed,
            4 => Self::PublishingFinished,
            5 => Self::Cancelled,
            6 => Self::OnHiatus,
            7 => Self::Upcoming,
            other => Self::Other(other),
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::Ongoing => 1,
            Self::Completed => 2,
            Self::Licensed => 3,
            Self::PublishingFinished => 4,
            Self::Cancelled => 5,
            Self::OnHiatus => 6,
            Self::Upcoming => 7,
            Self::Other(value) => value,
        }
    }
}

impl From<i32> for AnimeStatus {
    fn from(value: i32) -> Self {
        Self::from_i32(value)
    }
}

impl From<AnimeStatus> for i32 {
    fn from(status: AnimeStatus) -> Self {
        status.to_i32()
    }
}

/// Whether an entry is a list of seasons or a list of episodes. The ordinal is stored; `Episodes` is the default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(from = "i64", into = "i64")]
pub enum FetchType {
    Seasons = 0,
    #[default]
    Episodes = 1,
}

impl From<i64> for FetchType {
    fn from(stored: i64) -> Self {
        Self::from_stored(stored)
    }
}

impl From<FetchType> for i64 {
    fn from(fetch_type: FetchType) -> Self {
        fetch_type.stored()
    }
}

impl FetchType {
    /// The stored ordinal. Any value other than `0` reads as `Episodes` (`DatabaseAdapter.kt:43-47`).
    pub fn from_stored(value: i64) -> Self {
        if value == 0 { Self::Seasons } else { Self::Episodes }
    }

    pub fn stored(self) -> i64 {
        self as i64
    }

    /// Whether an update of this entry calls the episode path. A season entry never does (behaviour 4.7).
    pub fn fetches_episodes(self) -> bool {
        self == Self::Episodes
    }

    /// Whether an update of this entry calls the season path. An episode entry never does.
    pub fn fetches_seasons(self) -> bool {
        self == Self::Seasons
    }
}

/// An anime as a source reports it (`SAnime`). `status` keeps unknown stored values in `Other`.
///
/// Serde follows `defaultJson` (see `crate::json`): unknown keys are ignored, a `null` reads as absent, and an
/// optional field that is `None` is not written. The JSON keys are camelCase.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SAnime {
    #[serde(deserialize_with = "null_as_default")]
    pub url: String,
    #[serde(deserialize_with = "null_as_default")]
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Genres joined with `", "`. [`SAnime::genres`] parses it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(deserialize_with = "null_as_default")]
    pub status: AnimeStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thumbnail_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_url: Option<String>,
    #[serde(deserialize_with = "null_as_default")]
    pub update_strategy: UpdateStrategy,
    #[serde(deserialize_with = "null_as_default")]
    pub fetch_type: FetchType,
    /// `-1.0` when the entry is not a season.
    #[serde(deserialize_with = "null_is_not_a_season")]
    pub season_number: f64,
    #[serde(deserialize_with = "null_as_default")]
    pub initialized: bool,
    #[serde(deserialize_with = "null_as_default")]
    pub memo: Map<String, Value>,
}

/// A `null` season number reads as `-1.0`, the not-a-season value of [`SAnime::default`], as a missing key does.
fn null_is_not_a_season<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f64, D::Error> {
    Ok(Option::<f64>::deserialize(deserializer)?.unwrap_or(-1.0))
}

impl Default for SAnime {
    /// `SAnimeImpl` defaults: unknown status, always updated, episodes, not a season, not initialised, empty memo.
    fn default() -> Self {
        Self {
            url: String::new(),
            title: String::new(),
            artist: None,
            author: None,
            description: None,
            genre: None,
            status: AnimeStatus::Unknown,
            thumbnail_url: None,
            background_url: None,
            update_strategy: UpdateStrategy::AlwaysUpdate,
            fetch_type: FetchType::Episodes,
            season_number: -1.0,
            initialized: false,
            memo: Map::new(),
        }
    }
}

impl SAnime {
    /// The genres in order of first occurrence; `None` for a missing or blank genre text (behaviour 4.10).
    pub fn genres(&self) -> Option<Vec<String>> {
        parse_genres(self.genre.as_deref())
    }

    /// `setUrlWithoutDomain`: stores the relative form of `raw` (T-16a-027).
    pub fn set_url_without_domain(&mut self, raw: &str) {
        self.url = url_without_domain(raw);
    }
}

/// One episode as a source reports it (`SEpisode`).
#[derive(Debug, Clone, PartialEq)]
pub struct SEpisode {
    pub url: String,
    pub name: String,
    /// `-1.0` when unknown.
    pub episode_number: f32,
    pub fillermark: bool,
    pub scanlator: Option<String>,
    /// Epoch milliseconds, `0` when unknown.
    pub date_upload: i64,
    pub summary: Option<String>,
    pub preview_url: Option<String>,
    pub memo: Map<String, Value>,
}

impl Default for SEpisode {
    fn default() -> Self {
        Self {
            url: String::new(),
            name: String::new(),
            episode_number: -1.0,
            fillermark: false,
            scanlator: None,
            date_upload: 0,
            summary: None,
            preview_url: None,
            memo: Map::new(),
        }
    }
}

impl SEpisode {
    /// `setUrlWithoutDomain`: stores the relative form of `raw` (T-16a-027).
    pub fn set_url_without_domain(&mut self, raw: &str) {
        self.url = url_without_domain(raw);
    }

    /// `SEpisode.copyFrom`: copies the nine fields of the episode (`SEpisode.kt:39-49`) from `other`.
    pub fn copy_from(&mut self, other: &SEpisode) {
        self.name.clone_from(&other.name);
        self.url.clone_from(&other.url);
        self.date_upload = other.date_upload;
        self.episode_number = other.episode_number;
        self.fillermark = other.fillermark;
        self.scanlator.clone_from(&other.scanlator);
        self.summary.clone_from(&other.summary);
        self.preview_url.clone_from(&other.preview_url);
        self.memo.clone_from(&other.memo);
    }
}

/// One listing page of anime (`AnimesPage`). An empty page with `has_next_page` is valid.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimesPage {
    pub animes: Vec<SAnime>,
    pub has_next_page: bool,
}

/// One group of related anime (`AnimeRelation.kt`; T-16a-046). Groups keep the source order, and so do the animes in
/// each group.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimeRelation {
    pub name: String,
    pub animes: Vec<SAnime>,
}

/// The result of a combined episode update (`SAnimeEpisodeUpdate`). Both fields are required.
#[derive(Debug, Clone, PartialEq)]
pub struct SAnimeEpisodeUpdate {
    pub anime: SAnime,
    pub episodes: Vec<SEpisode>,
}

/// The result of a combined season update (`SAnimeSeasonUpdate`). Both fields are required.
#[derive(Debug, Clone, PartialEq)]
pub struct SAnimeSeasonUpdate {
    pub anime: SAnime,
    pub seasons: Vec<SAnime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_match_the_kotlin_impl() {
        let anime = SAnime::default();
        assert_eq!(anime.status.to_i32(), 0);
        assert_eq!(anime.update_strategy, UpdateStrategy::AlwaysUpdate);
        assert_eq!(anime.fetch_type, FetchType::Episodes);
        assert_eq!(anime.season_number, -1.0);
        assert!(!anime.initialized);
        assert!(anime.memo.is_empty());
    }

    #[test]
    fn status_round_trips_0_to_7_and_keeps_unknown_values() {
        for value in 0..=7 {
            assert_eq!(AnimeStatus::from_i32(value).to_i32(), value);
        }
        assert_eq!(AnimeStatus::from_i32(99), AnimeStatus::Other(99));
        assert_eq!(AnimeStatus::from_i32(99).to_i32(), 99);
        assert_eq!(AnimeStatus::from_i32(7), AnimeStatus::Upcoming);
    }

    #[test]
    fn genres_follow_the_kotlin_split_on_comma_space() {
        let genres = |raw: Option<&str>| SAnime { genre: raw.map(str::to_owned), ..Default::default() }.genres();
        assert_eq!(
            genres(Some("Action, Drama, Action, Comedy")),
            Some(vec!["Action".into(), "Drama".into(), "Comedy".into()])
        );
        // A comma without a following space is not a separator (`"a,b"` is one genre).
        assert_eq!(genres(Some("a,b")), Some(vec!["a,b".into()]));
        // Kotlin splits on ", " only. In "Action,  ,Comedy" the separator after "Action" leaves " ,Comedy".
        assert_eq!(genres(Some("Action,  ,Comedy")), Some(vec!["Action".into(), ",Comedy".into()]));
        assert_eq!(genres(Some("   ")), None);
        assert_eq!(genres(None), None);
    }

    #[test]
    fn a_copy_is_equal_and_independent() {
        let mut original = SAnime { title: "One".into(), genre: Some("A".into()), ..Default::default() };
        original.memo.insert("k".into(), Value::Bool(true));
        let copy = original.clone();
        assert_eq!(copy, original);
        original.title = "Two".into();
        assert_eq!(copy.title, "One");
    }

    #[test]
    fn an_episode_has_the_kotlin_defaults() {
        let episode = SEpisode::default();
        assert_eq!(episode.episode_number, -1.0);
        assert!(!episode.fillermark);
        assert_eq!(episode.date_upload, 0);
        assert!(episode.memo.is_empty());
        assert_eq!((episode.scanlator, episode.summary, episode.preview_url), (None, None, None));
    }

    #[test]
    fn an_unknown_stored_fetch_type_is_episodes_and_routing_is_exclusive() {
        assert_eq!(FetchType::from_stored(0), FetchType::Seasons);
        assert_eq!(FetchType::from_stored(1), FetchType::Episodes);
        assert_eq!(FetchType::from_stored(42), FetchType::Episodes);
        assert_eq!(FetchType::Seasons.stored(), 0);
        assert!(FetchType::Seasons.fetches_seasons() && !FetchType::Seasons.fetches_episodes());
        assert!(FetchType::Episodes.fetches_episodes() && !FetchType::Episodes.fetches_seasons());
    }

    #[test]
    fn the_update_carriers_hold_their_fields_without_defaults() {
        let anime = SAnime { title: "Show".into(), ..Default::default() };
        let episodes = SAnimeEpisodeUpdate { anime: anime.clone(), episodes: vec![SEpisode::default()] };
        let seasons = SAnimeSeasonUpdate { anime: anime.clone(), seasons: vec![anime.clone()] };
        assert_eq!(episodes.clone(), episodes);
        assert_eq!(seasons.seasons.len(), 1);
        assert_eq!(episodes.anime.title, "Show");
    }

    #[test]
    fn an_empty_page_with_a_next_page_is_kept_as_it_is() {
        let page = AnimesPage { animes: Vec::new(), has_next_page: true };
        assert_eq!(page.clone(), AnimesPage { animes: vec![], has_next_page: true });
        assert!(page.animes.is_empty() && page.has_next_page);
    }

    #[test]
    fn set_url_without_domain_stores_the_relative_form() {
        let mut anime = SAnime::default();
        anime.set_url_without_domain("https://x.org/show/a%20b?p=2");
        assert_eq!(anime.url, "/show/a b?p=2");
        let mut episode = SEpisode::default();
        episode.set_url_without_domain("https://x.org/ep/1");
        assert_eq!(episode.url, "/ep/1");
    }

    #[test]
    fn copy_from_copies_the_nine_episode_fields() {
        let source = SEpisode {
            url: "/e/2".into(),
            name: "Episode 2".into(),
            episode_number: 2.0,
            fillermark: true,
            scanlator: Some("S".into()),
            date_upload: 5,
            summary: Some("sum".into()),
            preview_url: Some("p".into()),
            ..Default::default()
        };
        let mut target = SEpisode::default();
        target.copy_from(&source);
        assert_eq!(target, source);
    }

    #[test]
    fn a_nested_memo_survives_a_round_trip_with_every_key() {
        let body =
            br#"{"url":"/a","title":"T","memo":{"aniyomi":{"k":1,"list":[true,null],"deep":{"x":"y"}},"top":2}}"#;
        let anime: SAnime = crate::json::decode_source_json(body).unwrap();
        let encoded = crate::json::encode_source_json(&anime).unwrap();
        let back: SAnime = crate::json::decode_source_json(&encoded).unwrap();
        assert_eq!(back, anime);
        assert_eq!(back.memo.len(), 2);
        assert_eq!(back.memo["aniyomi"]["deep"]["x"], "y");
        assert_eq!(back.memo["aniyomi"]["list"], serde_json::json!([true, null]));
    }

    #[test]
    fn an_unknown_key_in_a_source_response_still_parses_into_sanime() {
        let body = br#"{"url":"/a","title":"T","futureField":{"x":[1]},"status":1,"fetchType":0}"#;
        let anime: SAnime = crate::json::decode_source_json(body).unwrap();
        assert_eq!(
            (anime.title.as_str(), anime.status, anime.fetch_type),
            ("T", AnimeStatus::Ongoing, FetchType::Seasons)
        );
    }

    #[test]
    fn a_null_reads_as_absent_and_an_unset_optional_is_not_written() {
        let body = br#"{"url":null,"title":"T","status":null,"seasonNumber":null,"memo":null,"artist":null}"#;
        let anime: SAnime = crate::json::decode_source_json(body).unwrap();
        assert_eq!(anime, SAnime { title: "T".into(), ..Default::default() });
        let text = String::from_utf8(crate::json::encode_source_json(&anime).unwrap()).unwrap();
        assert!(!text.contains("artist") && !text.contains("thumbnailUrl"), "{text}");
        assert!(text.contains(r#""seasonNumber":-1.0"#), "{text}");
    }

    #[test]
    fn episode_number_survives_an_f64_round_trip_in_half_steps() {
        let mut value = -1.0_f32;
        while value <= 10_000.0 {
            assert_eq!(value as f64 as f32, value);
            value += 0.5;
        }
    }
}
