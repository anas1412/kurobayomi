//! Hoster and video selection (area 45 part 2: `EpisodeLoader.kt`, `HosterLoader.kt`; behaviour
//! 3.6-3.7, 3.9, 3.10; T-45b-008, T-45b-009, T-45b-010, T-45b-012). The source calls go through
//! `VideoSource`, so the logic is tested with a fake and the app implements the trait over the
//! real source. Calls are synchronous; cancellation is the caller's job (drop the call).

use crate::types::{Hoster, HosterState, ReadyHoster, Video, VideoState};

/// The source calls the player makes on a hoster or a video (`AnimeSource` / `AnimeHttpSource`).
pub trait VideoSource {
    type Error;
    /// The episode the source lists hosters or videos for (`episode.toSEpisode()`).
    type Episode;

    /// `source is AnimeHttpSource`.
    fn is_http(&self) -> bool;
    /// Replaces the reflection walk `checkHasHosters` (`EpisodeLoader.kt:60-78`). The app answers
    /// it from `kuro-source` `AnimeSource::provides_hosters`: the two names are one capability.
    fn has_hoster_list(&self) -> bool;
    /// `getHosterList(episode)`. Only called when `has_hoster_list` is true.
    fn hoster_list(&self, episode: &Self::Episode) -> Result<Vec<Hoster>, Self::Error>;
    /// `sortHosters`. Only called for the hoster list path.
    fn sort_hosters(&self, hosters: Vec<Hoster>) -> Vec<Hoster>;
    /// `getVideoList(episode)`, the legacy episode level list. Only called when `has_hoster_list`
    /// is false. // compat: ext-lib<1.6
    fn episode_video_list(&self, episode: &Self::Episode) -> Result<Vec<Video>, Self::Error>;
    /// `getVideoList(hoster)`. Only called for HTTP sources.
    fn video_list(&self, hoster: &Hoster) -> Result<Vec<Video>, Self::Error>;
    /// `getVideoUrl(video)`. Only called for lazy videos.
    fn video_url(&self, video: &Video) -> Result<String, Self::Error>;
    /// `sortVideos`. Only called for HTTP sources.
    fn sort_videos(&self, videos: Vec<Video>) -> Vec<Video>;
    /// `resolveVideo(video)`. Only called for HTTP sources.
    fn resolve_video(&self, video: &Video) -> Result<Video, Self::Error>;
}

/// Why `get_videos` failed: the source call failed, or the hoster has no list and the source
/// cannot list videos (`error("source not supported")`).
#[derive(Debug, PartialEq, Eq)]
pub enum VideosError<E> {
    NotSupported,
    Source(E),
}

/// `getHostersOnHttp(episode, source)` (behaviour 3.3). A source with a hoster list gives its
/// sorted hosters. Only a source without one takes the legacy video list path.
pub fn get_hosters_on_http<S: VideoSource>(
    source: &S,
    episode: &S::Episode,
) -> Result<Vec<Hoster>, S::Error> {
    if source.has_hoster_list() {
        let hosters = source.hoster_list(episode)?;
        Ok(source.sort_hosters(hosters))
    } else {
        legacy_hosters(source, episode)
    }
}

// compat: ext-lib<1.6
/// The legacy path: the sorted episode video list wrapped as one hoster (`toHosterList`). Reached
/// only from `get_hosters_on_http`, and only when `has_hoster_list` is false.
fn legacy_hosters<S: VideoSource>(
    source: &S,
    episode: &S::Episode,
) -> Result<Vec<Hoster>, S::Error> {
    let videos = source.episode_video_list(episode)?;
    Ok(vec![Hoster::from_videos(source.sort_videos(videos))])
}

/// `getVideos(source, hoster)` (behaviour 3.6). A hoster list is used as it is; a missing list
/// is fetched from an HTTP source. HTTP sources resolve lazy URLs and sort the result.
pub fn get_videos<S: VideoSource>(
    source: &S,
    hoster: &Hoster,
) -> Result<Vec<Video>, VideosError<S::Error>> {
    let http = source.is_http();
    let videos = match &hoster.video_list {
        Some(list) if http => {
            parse_video_urls(source, list.clone()).map_err(VideosError::Source)?
        }
        Some(list) => list.clone(),
        None if http => {
            let listed = source.video_list(hoster).map_err(VideosError::Source)?;
            parse_video_urls(source, listed).map_err(VideosError::Source)?
        }
        None => return Err(VideosError::NotSupported),
    };
    Ok(if http {
        source.sort_videos(videos)
    } else {
        videos
    })
}

/// `parseVideoUrls` (behaviour 3.7). Only lazy videos (`"null"`) ask the source for a URL. Every
/// other video is kept as it is and is never resolved here.
pub fn parse_video_urls<S: VideoSource>(
    source: &S,
    videos: Vec<Video>,
) -> Result<Vec<Video>, S::Error> {
    videos
        .into_iter()
        .map(|video| {
            if video.is_lazy() {
                let url = source.video_url(&video)?;
                Ok(Video {
                    video_url: url,
                    ..video
                })
            } else {
                Ok(video)
            }
        })
        .collect()
}

/// `getResolvedVideo(source, video)` (behaviour 3.10). HTTP sources resolve only videos that are
/// not initialised yet. A resolve error gives `None`. The result is always marked initialised.
pub fn get_resolved_video<S: VideoSource>(source: &S, video: &Video) -> Option<Video> {
    let resolved = if source.is_http() && !video.initialized {
        source.resolve_video(video).ok()?
    } else {
        video.clone()
    };
    Some(Video {
        initialized: true,
        ..resolved
    })
}

/// `selectBestVideo` (behaviour 3.9). Returns `(hoster index, video index)` in the original list.
/// Only `Ready` hosters are searched, and only videos in `READY` or `QUEUE` state. The first pass
/// takes a preferred video. The second takes the first video with a non-empty URL. `None` is the
/// Kotlin `(-1, -1)`.
pub fn select_best_video(states: &[HosterState]) -> Option<(usize, usize)> {
    let ready: Vec<(usize, &ReadyHoster)> = states
        .iter()
        .enumerate()
        .filter_map(|(index, state)| match state {
            HosterState::Ready(ready) => Some((index, ready)),
            _ => None,
        })
        .collect();

    first_match(&ready, |video, state| video.preferred && selectable(state)).or_else(|| {
        first_match(&ready, |video, state| {
            !video.video_url.is_empty() && selectable(state)
        })
    })
}

fn selectable(state: VideoState) -> bool {
    matches!(state, VideoState::Ready | VideoState::Queue)
}

/// The first hoster (in order) that has a matching video, with that video's index in the hoster.
/// The video list and the state list are paired by index, as Kotlin `zip` does.
fn first_match(
    ready: &[(usize, &ReadyHoster)],
    pred: impl Fn(&Video, VideoState) -> bool,
) -> Option<(usize, usize)> {
    ready.iter().find_map(|(hoster_index, hoster)| {
        hoster
            .video_list
            .iter()
            .zip(&hoster.video_state)
            .position(|(video, state)| pred(video, *state))
            .map(|video_index| (*hoster_index, video_index))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::NO_HOSTER_LIST;
    use std::cell::Cell;

    fn video(url: &str) -> Video {
        Video {
            video_url: url.to_owned(),
            ..Video::default()
        }
    }

    fn preferred(url: &str) -> Video {
        Video {
            preferred: true,
            ..video(url)
        }
    }

    fn ready(videos: Vec<(Video, VideoState)>) -> HosterState {
        let (video_list, video_state) = videos.into_iter().unzip();
        HosterState::Ready(ReadyHoster {
            name: "h".to_owned(),
            video_list,
            video_state,
        })
    }

    #[test]
    fn t1_preferred_beats_first_valid() {
        let states = vec![
            ready(vec![(video("a"), VideoState::Queue)]),
            ready(vec![(preferred("b"), VideoState::Queue)]),
        ];
        assert_eq!(select_best_video(&states), Some((1, 0)));
    }

    #[test]
    fn t2_preferred_error_is_skipped() {
        let states = vec![
            ready(vec![(preferred("x"), VideoState::Error)]),
            ready(vec![(video("b"), VideoState::Queue)]),
        ];
        assert_eq!(select_best_video(&states), Some((1, 0)));
    }

    #[test]
    fn t3_lazy_url_is_selectable() {
        let states = vec![ready(vec![(video("null"), VideoState::Queue)])];
        assert_eq!(select_best_video(&states), Some((0, 0)));
    }

    #[test]
    fn t4_none_selectable_when_all_error() {
        let states = vec![
            ready(vec![(preferred("a"), VideoState::Error)]),
            HosterState::Error("e".to_owned()),
        ];
        assert_eq!(select_best_video(&states), None);
    }

    #[test]
    fn t5_load_video_state_is_excluded() {
        let states = vec![ready(vec![(video("a"), VideoState::LoadVideo)])];
        assert_eq!(select_best_video(&states), None);
    }

    #[test]
    fn idle_and_loading_hosters_are_not_searched() {
        let states = vec![
            HosterState::Loading("l".to_owned()),
            ready(vec![(video("a"), VideoState::Ready)]),
        ];
        assert_eq!(select_best_video(&states), Some((1, 0)));
    }

    struct Fake {
        http: bool,
        has_hosters: bool,
        listed: Vec<Video>,
        url_calls: Cell<usize>,
        sorted: Cell<bool>,
        resolve_fails: bool,
        hoster_list_calls: Cell<usize>,
        episode_list_calls: Cell<usize>,
        hosters_sorted: Cell<bool>,
    }

    impl Fake {
        fn new(http: bool) -> Self {
            Self {
                http,
                has_hosters: false,
                listed: Vec::new(),
                url_calls: Cell::new(0),
                sorted: Cell::new(false),
                resolve_fails: false,
                hoster_list_calls: Cell::new(0),
                episode_list_calls: Cell::new(0),
                hosters_sorted: Cell::new(false),
            }
        }
    }

    impl VideoSource for Fake {
        type Error = String;
        type Episode = u32;

        fn is_http(&self) -> bool {
            self.http
        }

        fn has_hoster_list(&self) -> bool {
            self.has_hosters
        }

        fn hoster_list(&self, _episode: &u32) -> Result<Vec<Hoster>, String> {
            self.hoster_list_calls.set(self.hoster_list_calls.get() + 1);
            Ok(vec![hoster("a"), hoster("b")])
        }

        fn sort_hosters(&self, mut hosters: Vec<Hoster>) -> Vec<Hoster> {
            self.hosters_sorted.set(true);
            hosters.reverse();
            hosters
        }

        fn episode_video_list(&self, _episode: &u32) -> Result<Vec<Video>, String> {
            self.episode_list_calls
                .set(self.episode_list_calls.get() + 1);
            Ok(self.listed.clone())
        }

        fn video_list(&self, _hoster: &Hoster) -> Result<Vec<Video>, String> {
            Ok(self.listed.clone())
        }

        fn video_url(&self, video: &Video) -> Result<String, String> {
            self.url_calls.set(self.url_calls.get() + 1);
            Ok(format!("resolved:{}", video.video_title))
        }

        fn sort_videos(&self, mut videos: Vec<Video>) -> Vec<Video> {
            self.sorted.set(true);
            videos.reverse();
            videos
        }

        fn resolve_video(&self, video: &Video) -> Result<Video, String> {
            if self.resolve_fails {
                Err("boom".to_owned())
            } else {
                Ok(Video {
                    video_url: format!("final:{}", video.video_url),
                    ..video.clone()
                })
            }
        }
    }

    #[test]
    fn non_lazy_url_is_not_resolved() {
        let fake = Fake::new(true);
        let out = parse_video_urls(&fake, vec![video("https://x/a")]).unwrap();
        assert_eq!(out[0].video_url, "https://x/a");
        assert_eq!(fake.url_calls.get(), 0);
    }

    #[test]
    fn lazy_url_is_resolved_once_per_video() {
        let fake = Fake::new(true);
        let mut lazy = video("null");
        lazy.video_title = "720p".to_owned();
        let out = parse_video_urls(&fake, vec![lazy]).unwrap();
        assert_eq!(out[0].video_url, "resolved:720p");
        assert_eq!(fake.url_calls.get(), 1);
    }

    #[test]
    fn get_videos_sorts_only_for_http_sources() {
        let hoster = Hoster::from_videos(vec![video("a"), video("b")]);
        let http = Fake::new(true);
        let out = get_videos(&http, &hoster).unwrap();
        assert!(http.sorted.get());
        assert_eq!(out[0].video_url, "b");

        let local = Fake::new(false);
        let out = get_videos(&local, &hoster).unwrap();
        assert!(!local.sorted.get());
        assert_eq!(out[0].video_url, "a");
    }

    #[test]
    fn get_videos_fetches_missing_list_over_http() {
        let mut fake = Fake::new(true);
        fake.listed = vec![video("null")];
        let hoster = Hoster::default();
        let out = get_videos(&fake, &hoster).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(fake.url_calls.get(), 1);
    }

    #[test]
    fn missing_list_without_http_is_not_supported() {
        let fake = Fake::new(false);
        let err = get_videos(&fake, &Hoster::default()).unwrap_err();
        assert_eq!(err, VideosError::NotSupported);
    }

    #[test]
    fn resolve_marks_initialized_for_local_sources() {
        let fake = Fake::new(false);
        let out = get_resolved_video(&fake, &video("file:///a.mkv")).unwrap();
        assert!(out.initialized);
        assert_eq!(out.video_url, "file:///a.mkv");
    }

    #[test]
    fn http_resolve_skips_initialized_videos() {
        let fake = Fake::new(true);
        let done = Video {
            initialized: true,
            ..video("https://x/a")
        };
        let out = get_resolved_video(&fake, &done).unwrap();
        assert_eq!(out.video_url, "https://x/a");
    }

    #[test]
    fn http_resolve_error_gives_none() {
        let mut fake = Fake::new(true);
        fake.resolve_fails = true;
        assert_eq!(get_resolved_video(&fake, &video("https://x/a")), None);
    }

    fn hoster(name: &str) -> Hoster {
        Hoster {
            hoster_name: name.to_owned(),
            ..Hoster::default()
        }
    }

    #[test]
    fn hoster_list_source_never_takes_the_legacy_path() {
        let mut fake = Fake::new(true);
        fake.has_hosters = true;
        fake.listed = vec![video("legacy")];
        let out = get_hosters_on_http(&fake, &1).unwrap();
        assert_eq!(fake.episode_list_calls.get(), 0);
        assert_eq!(fake.hoster_list_calls.get(), 1);
        assert!(fake.hosters_sorted.get());
        let names: Vec<_> = out.iter().map(|h| h.hoster_name.as_str()).collect();
        assert_eq!(names, ["b", "a"]);
    }

    #[test]
    fn legacy_path_runs_only_without_a_hoster_list() {
        let mut fake = Fake::new(true);
        fake.has_hosters = false;
        fake.listed = vec![video("a"), video("b")];
        let out = get_hosters_on_http(&fake, &1).unwrap();
        assert_eq!(fake.hoster_list_calls.get(), 0);
        assert_eq!(fake.episode_list_calls.get(), 1);
        assert!(!fake.hosters_sorted.get());
        assert!(fake.sorted.get());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].hoster_name, NO_HOSTER_LIST);
        let urls: Vec<_> = out[0]
            .video_list
            .as_ref()
            .unwrap()
            .iter()
            .map(|v| v.video_url.as_str())
            .collect();
        assert_eq!(urls, ["b", "a"]);
    }
}
