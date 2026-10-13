//! The JSON shapes of the anime ops (ABI 1.4, additive: a guest without them answers "unknown op"). Field names are
//! camelCase, as in `wire.rs`, and each `kuro-source` anime model has a wire twin and a conversion.

use kuro_source::anime::{AnimesPage, Headers, Hoster, SAnime, SEpisode, Track, Video};
use kuro_source::model::UpdateStrategy;
use serde::{Deserialize, Serialize};

fn unknown_episode_number() -> f32 {
    -1.0
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireAnime {
    pub url: String,
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub thumbnail_url: Option<String>,
    #[serde(default)]
    pub author: Option<String>,
    #[serde(default)]
    pub genre: Option<String>,
    /// `AnimeStatus` as its number (0 unknown, 1 ongoing, 2 completed, ...).
    #[serde(default)]
    pub status: i32,
    #[serde(default)]
    pub artist: Option<String>,
    #[serde(default)]
    pub update_strategy: UpdateStrategy,
    #[serde(default)]
    pub initialized: bool,
}

/// The answer of `popular`, `latest` and `search` in an anime package.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireAnimesPage {
    #[serde(default)]
    pub animes: Vec<WireAnime>,
    #[serde(default)]
    pub has_next_page: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireEpisode {
    pub url: String,
    pub name: String,
    #[serde(default = "unknown_episode_number")]
    pub episode_number: f32,
    #[serde(default)]
    pub fillermark: bool,
    #[serde(default)]
    pub scanlator: Option<String>,
    #[serde(default)]
    pub date_upload: i64,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireTrack {
    pub url: String,
    #[serde(default)]
    pub lang: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireVideo {
    pub video_url: String,
    #[serde(default)]
    pub video_title: String,
    #[serde(default)]
    pub resolution: Option<i32>,
    #[serde(default)]
    pub bitrate: Option<i32>,
    /// (name, value) pairs sent with the player's requests.
    #[serde(default)]
    pub headers: Vec<(String, String)>,
    #[serde(default)]
    pub preferred: bool,
    #[serde(default)]
    pub subtitle_tracks: Vec<WireTrack>,
    #[serde(default)]
    pub audio_tracks: Vec<WireTrack>,
    #[serde(default)]
    pub internal_data: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WireHoster {
    pub hoster_url: String,
    #[serde(default)]
    pub hoster_name: String,
    /// `None` when the videos are fetched later, with the `videos` op and this hoster.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_list: Option<Vec<WireVideo>>,
    #[serde(default)]
    pub internal_data: String,
    #[serde(default)]
    pub lazy: bool,
}

impl From<&SAnime> for WireAnime {
    fn from(anime: &SAnime) -> Self {
        Self {
            url: anime.url.clone(),
            title: anime.title.clone(),
            description: anime.description.clone(),
            thumbnail_url: anime.thumbnail_url.clone(),
            author: anime.author.clone(),
            genre: anime.genre.clone(),
            status: anime.status.to_i32(),
            artist: anime.artist.clone(),
            update_strategy: anime.update_strategy,
            initialized: anime.initialized,
        }
    }
}

impl From<WireAnime> for SAnime {
    fn from(wire: WireAnime) -> Self {
        Self {
            url: wire.url,
            title: wire.title,
            description: wire.description,
            thumbnail_url: wire.thumbnail_url,
            author: wire.author.filter(|a| !a.trim().is_empty()),
            genre: wire.genre.filter(|g| !g.trim().is_empty()),
            status: kuro_source::anime::model::AnimeStatus::from_i32(wire.status),
            artist: wire.artist.filter(|a| !a.trim().is_empty()),
            update_strategy: wire.update_strategy,
            initialized: wire.initialized,
            ..SAnime::default()
        }
    }
}

impl From<WireAnimesPage> for AnimesPage {
    fn from(wire: WireAnimesPage) -> Self {
        Self {
            animes: wire.animes.into_iter().map(SAnime::from).collect(),
            has_next_page: wire.has_next_page,
        }
    }
}

impl From<&SEpisode> for WireEpisode {
    fn from(episode: &SEpisode) -> Self {
        Self {
            url: episode.url.clone(),
            name: episode.name.clone(),
            episode_number: episode.episode_number,
            fillermark: episode.fillermark,
            scanlator: episode.scanlator.clone(),
            date_upload: episode.date_upload,
            summary: episode.summary.clone(),
        }
    }
}

impl From<WireEpisode> for SEpisode {
    fn from(wire: WireEpisode) -> Self {
        Self {
            url: wire.url,
            name: wire.name,
            episode_number: wire.episode_number,
            fillermark: wire.fillermark,
            scanlator: wire.scanlator,
            date_upload: wire.date_upload,
            summary: wire.summary,
            ..SEpisode::default()
        }
    }
}

impl From<&Track> for WireTrack {
    fn from(track: &Track) -> Self {
        Self { url: track.url.clone(), lang: track.lang.clone() }
    }
}

impl From<WireTrack> for Track {
    fn from(wire: WireTrack) -> Self {
        Self { url: wire.url, lang: wire.lang }
    }
}

impl From<&Video> for WireVideo {
    fn from(video: &Video) -> Self {
        Self {
            video_url: video.video_url.clone(),
            video_title: video.video_title.clone(),
            resolution: video.resolution,
            bitrate: video.bitrate,
            headers: video.headers.as_ref().map(|h| h.0.clone()).unwrap_or_default(),
            preferred: video.preferred,
            subtitle_tracks: video.subtitle_tracks.iter().map(WireTrack::from).collect(),
            audio_tracks: video.audio_tracks.iter().map(WireTrack::from).collect(),
            internal_data: video.internal_data.clone(),
        }
    }
}

impl From<WireVideo> for Video {
    fn from(wire: WireVideo) -> Self {
        Self {
            video_url: wire.video_url,
            video_title: wire.video_title,
            resolution: wire.resolution,
            bitrate: wire.bitrate,
            headers: (!wire.headers.is_empty()).then_some(Headers(wire.headers)),
            preferred: wire.preferred,
            subtitle_tracks: wire.subtitle_tracks.into_iter().map(Track::from).collect(),
            audio_tracks: wire.audio_tracks.into_iter().map(Track::from).collect(),
            internal_data: wire.internal_data,
            ..Video::default()
        }
    }
}

impl From<&Hoster> for WireHoster {
    fn from(hoster: &Hoster) -> Self {
        Self {
            hoster_url: hoster.hoster_url.clone(),
            hoster_name: hoster.hoster_name.clone(),
            video_list: hoster.video_list.as_ref().map(|list| list.iter().map(WireVideo::from).collect()),
            internal_data: hoster.internal_data.clone(),
            lazy: hoster.lazy,
        }
    }
}

impl From<WireHoster> for Hoster {
    fn from(wire: WireHoster) -> Self {
        Self {
            hoster_url: wire.hoster_url,
            hoster_name: wire.hoster_name,
            video_list: wire.video_list.map(|list| list.into_iter().map(Video::from).collect()),
            internal_data: wire.internal_data,
            lazy: wire.lazy,
            ..Hoster::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hoster_with_videos_and_tracks_round_trips_through_its_wire_form() {
        let json = r#"{"hosterUrl":"https://h/1","hosterName":"Main","videoList":[{"videoUrl":"https://v/1.m3u8","videoTitle":"1080p","resolution":1080,"headers":[["Referer","https://h/"]],"preferred":true,"subtitleTracks":[{"url":"https://s/en.vtt","lang":"en"}]}]}"#;
        let wire: WireHoster = serde_json::from_str(json).expect("valid");
        let hoster = Hoster::from(wire.clone());
        let videos = hoster.video_list.as_ref().expect("videos");
        assert_eq!(videos[0].resolution, Some(1080));
        assert_eq!(videos[0].headers.as_ref().map(|h| h.0.len()), Some(1));
        assert_eq!(videos[0].subtitle_tracks[0].lang, "en");
        assert_eq!(WireHoster::from(&hoster), wire);
    }

    #[test]
    fn an_episode_without_a_number_takes_the_unknown_number() {
        let wire: WireEpisode = serde_json::from_str(r#"{"url":"/e/1","name":"Ep 1"}"#).expect("valid");
        let episode = SEpisode::from(wire);
        assert_eq!(episode.episode_number, -1.0);
        assert!(!episode.fillermark);
        assert_eq!(episode.date_upload, 0);
    }

    #[test]
    fn an_anime_page_reads_its_animes_in_camel_case() {
        let wire: WireAnimesPage =
            serde_json::from_str(r#"{"animes":[{"url":"/a/1","title":"One","thumbnailUrl":"t"}],"hasNextPage":true}"#)
                .expect("valid");
        let page = AnimesPage::from(wire);
        assert!(page.has_next_page);
        assert_eq!(page.animes[0].title, "One");
        assert_eq!(page.animes[0].thumbnail_url.as_deref(), Some("t"));
    }
}
