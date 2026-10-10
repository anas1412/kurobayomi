//! Hoster and video selection (area 45 part 2: `EpisodeLoader.kt`, `HosterLoader.kt`; behaviour
//! 3.6-3.7, 3.9, 3.10; T-45b-008, T-45b-009, T-45b-010, T-45b-012). The source calls go through
//! `VideoSource`, so the logic is tested with a fake and the app implements the trait over the
//! real source. Calls are synchronous; cancellation is the caller's job (drop the call).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use url::Url;

use crate::types::{Hoster, HosterState, NO_HOSTER_LIST, ReadyHoster, Video, VideoState};

/// The source calls the player makes on a hoster or a video (`AnimeSource` / `AnimeHttpSource`).
pub trait VideoSource {
    type Error;
    /// The episode the source lists hosters or videos for (`episode.toSEpisode()`).
    type Episode;

    /// `source is AnimeHttpSource`.
    fn is_http(&self) -> bool;
    /// `source is LocalAnimeSource`. A source that is neither HTTP nor local is not supported.
    fn is_local(&self) -> bool {
        false
    }
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

/// The quality sheet's hoster rows, as indices into the input list (QualitySheet grouping, T-46b-007).
/// `valid` holds Idle, Loading and Ready hosters with videos, `failed` the Error hosters and `empty`
/// the Ready hosters without videos. Each list keeps the input order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HosterGroups {
    pub valid: Vec<usize>,
    pub failed: Vec<usize>,
    pub empty: Vec<usize>,
}

impl HosterGroups {
    /// The failed rows the sheet shows: only with `pref_show_failed_hosters` (E58). Grouping keeps
    /// them all, so the flag is applied here, in the view.
    pub fn failed_shown(&self, show_failed: bool) -> &[usize] {
        if show_failed { &self.failed } else { &[] }
    }

    /// The empty rows the sheet shows: only with `pref_show_empty_hosters` (E58).
    pub fn empty_shown(&self, show_empty: bool) -> &[usize] {
        if show_empty { &self.empty } else { &[] }
    }
}

/// Quality sheet selection and expansion, keyed by hoster name and `(hoster name, video index)`, so
/// a re-sorted or reloaded hoster list keeps them (E55, R4; T-46b-014).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct QualityKeys {
    expanded: HashSet<String>,
    selected: Option<(String, usize)>,
}

impl QualityKeys {
    /// The row arrow was clicked: flips the expansion of that hoster.
    pub fn toggle_expanded(&mut self, hoster_name: &str) {
        if !self.expanded.remove(hoster_name) {
            self.expanded.insert(hoster_name.to_owned());
        }
    }

    pub fn is_expanded(&self, hoster_name: &str) -> bool {
        self.expanded.contains(hoster_name)
    }

    pub fn select(&mut self, hoster_name: &str, video_index: usize) {
        self.selected = Some((hoster_name.to_owned(), video_index));
    }

    pub fn is_selected(&self, hoster_name: &str, video_index: usize) -> bool {
        self.selected
            .as_ref()
            .is_some_and(|(name, index)| name == hoster_name && *index == video_index)
    }

    /// Selects what `select_best_video` picks in `states`.
    pub fn select_best(&mut self, states: &[HosterState]) {
        self.selected = select_best_video(states)
            .map(|(hoster, video)| (states[hoster].name().to_owned(), video));
    }

    /// The selection as positions in `states`, `None` when its hoster is gone.
    pub fn selected_position(&self, states: &[HosterState]) -> Option<(usize, usize)> {
        let (name, video) = self.selected.as_ref()?;
        let hoster = states.iter().position(|state| state.name() == name)?;
        Some((hoster, *video))
    }
}

/// Groups hoster states for the quality sheet (`group_hosters`, H1-H3).
pub fn group_hosters(states: &[HosterState]) -> HosterGroups {
    let mut groups = HosterGroups::default();
    for (index, state) in states.iter().enumerate() {
        match state {
            HosterState::Idle(_) | HosterState::Loading(_) => groups.valid.push(index),
            HosterState::Error(_) => groups.failed.push(index),
            HosterState::Ready(ready) if ready.video_list.is_empty() => groups.empty.push(index),
            HosterState::Ready(_) => groups.valid.push(index),
        }
    }
    groups
}

/// The flat list replaces the groups only when the single hoster is the no-hoster list and it is
/// Ready (`no_hoster_list`, T-46b-007).
pub fn is_flat(states: &[HosterState]) -> bool {
    matches!(states, [HosterState::Ready(ready)] if ready.name == NO_HOSTER_LIST)
}

/// `loadHosterVideos(source, hoster)` (behaviour 3.8, not forced). A lazy hoster stays `Idle`. A
/// hoster whose videos cannot be listed is `Error`. Every other video starts `Queue`.
pub fn load_hoster_state<S: VideoSource>(source: &S, hoster: &Hoster) -> HosterState {
    if hoster.lazy {
        return HosterState::Idle(hoster.hoster_name.clone());
    }
    load_hoster_state_forced(source, hoster)
}

/// `loadHosterVideos(source, hoster, force = true)`: the videos are listed even for a lazy hoster,
/// which is how a click on an `Idle` hoster loads it (behaviour 4.4).
pub fn load_hoster_state_forced<S: VideoSource>(source: &S, hoster: &Hoster) -> HosterState {
    match get_videos(source, hoster) {
        Ok(videos) => HosterState::Ready(ReadyHoster {
            name: hoster.hoster_name.clone(),
            video_state: vec![VideoState::Queue; videos.len()],
            video_list: videos,
        }),
        Err(_) => HosterState::Error(hoster.hoster_name.clone()),
    }
}

fn set_video_state(ready: &mut ReadyHoster, index: usize, state: VideoState) {
    if let Some(slot) = ready.video_state.get_mut(index) {
        *slot = state;
    }
}

/// Step 2a of `getBestVideo`: the hoster's first preferred video that is not initialised. It is
/// marked `LoadVideo` and resolved. A non-empty URL wins. Otherwise it is marked `Error`.
fn preferred_winner<S: VideoSource>(source: &S, state: &mut HosterState) -> Option<Video> {
    let HosterState::Ready(ready) = state else {
        return None;
    };
    let index = ready
        .video_list
        .iter()
        .position(|video| video.preferred && !video.initialized)?;
    set_video_state(ready, index, VideoState::LoadVideo);
    let video = ready.video_list.get(index)?.clone();
    match get_resolved_video(source, &video) {
        Some(resolved) if !resolved.video_url.is_empty() => Some(resolved),
        _ => {
            set_video_state(ready, index, VideoState::Error);
            None
        }
    }
}

/// Step 2c of `getBestVideo`: the first resolvable video that `select_best_video` picks. Each failed
/// pick is marked `Error`, so the loop ends.
fn fallback_winner<S: VideoSource>(source: &S, mut states: Vec<HosterState>) -> Option<Video> {
    loop {
        let (hoster, index) = select_best_video(&states)?;
        let HosterState::Ready(ready) = states.get_mut(hoster)? else {
            return None;
        };
        set_video_state(ready, index, VideoState::LoadVideo);
        let video = ready.video_list.get(index)?.clone();
        match get_resolved_video(source, &video) {
            Some(resolved) if !resolved.video_url.is_empty() => return Some(resolved),
            _ => set_video_state(ready, index, VideoState::Error),
        }
    }
}

/// `HosterLoader.getBestVideo` (behaviour 3.11, T-45b-011): the external-player path. Every hoster
/// loads in parallel on a blocking thread. The first preferred video that resolves to a non-empty
/// URL wins, and the other loads are dropped. A blocking call that already started runs to its end
/// on its thread, so it cannot be cancelled. If no preferred video wins, the hosters are searched in
/// order. `None` when nothing resolves.
pub async fn get_best_video<S>(source: Arc<S>, hosters: Vec<Hoster>) -> Option<Video>
where
    S: VideoSource + Send + Sync + 'static,
{
    let names: Vec<String> = hosters.iter().map(|h| h.hoster_name.clone()).collect();
    let mut races = tokio::task::JoinSet::new();
    for (index, hoster) in hosters.into_iter().enumerate() {
        let source = Arc::clone(&source);
        races.spawn_blocking(move || {
            let mut state = load_hoster_state(source.as_ref(), &hoster);
            let winner = preferred_winner(source.as_ref(), &mut state);
            (index, state, winner)
        });
    }

    let mut states: Vec<HosterState> = names.into_iter().map(HosterState::Idle).collect();
    while let Some(joined) = races.join_next().await {
        // A task that panicked contributes no state, as a failed load would.
        let Ok((index, state, winner)) = joined else {
            continue;
        };
        if let Some(slot) = states.get_mut(index) {
            *slot = state;
        }
        if let Some(video) = winner {
            races.abort_all();
            return Some(video);
        }
    }
    fallback_winner(source.as_ref(), states)
}

/// The videos of a local-source episode (behaviour 3.5, T-45b-063). `episode_url` is
/// `<anime dir>/<episode file>` under the local source base directory. The video URL is the file's
/// `file://` URI, and the title names the episode URL. A URL without a slash or a missing file gives
/// an empty list, which the caller reports as an empty hoster list.
pub fn local_videos(base_dir: &Path, episode_url: &str) -> Vec<Video> {
    let Some((anime_dir, episode_name)) = episode_url.split_once('/') else {
        return Vec::new();
    };
    let file = base_dir.join(anime_dir).join(episode_name);
    if !file.is_file() {
        return Vec::new();
    }
    let Ok(uri) = Url::from_file_path(&file) else {
        return Vec::new();
    };
    vec![Video {
        video_url: uri.to_string(),
        video_title: format!("Local source: {episode_url}"),
        ..Video::default()
    }]
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

#[cfg(test)]
mod race_tests {
    use super::*;
    use crate::types::NO_HOSTER_LIST;

    /// A source whose resolve step maps a video URL to a resolved URL. Plain sources (`http` false)
    /// skip the resolve step.
    #[derive(Default)]
    struct Fake {
        resolved: Vec<(String, String)>,
        listed: Vec<Video>,
        http: bool,
    }

    impl VideoSource for Fake {
        type Error = ();
        type Episode = ();

        fn is_http(&self) -> bool {
            self.http
        }
        fn has_hoster_list(&self) -> bool {
            false
        }
        fn hoster_list(&self, _episode: &()) -> Result<Vec<Hoster>, ()> {
            Ok(Vec::new())
        }
        fn sort_hosters(&self, hosters: Vec<Hoster>) -> Vec<Hoster> {
            hosters
        }
        fn episode_video_list(&self, _episode: &()) -> Result<Vec<Video>, ()> {
            Ok(Vec::new())
        }
        fn video_list(&self, _hoster: &Hoster) -> Result<Vec<Video>, ()> {
            Ok(self.listed.clone())
        }
        fn video_url(&self, _video: &Video) -> Result<String, ()> {
            Ok(String::new())
        }
        fn sort_videos(&self, videos: Vec<Video>) -> Vec<Video> {
            videos
        }
        fn resolve_video(&self, video: &Video) -> Result<Video, ()> {
            self.resolved
                .iter()
                .find(|(from, _)| *from == video.video_url)
                .map(|(_, to)| Video {
                    video_url: to.clone(),
                    ..video.clone()
                })
                .ok_or(())
        }
    }

    fn video(url: &str, preferred: bool) -> Video {
        Video {
            video_url: url.to_owned(),
            preferred,
            ..Video::default()
        }
    }

    fn hoster(name: &str, videos: Vec<Video>) -> Hoster {
        Hoster {
            hoster_name: name.to_owned(),
            video_list: Some(videos),
            ..Hoster::default()
        }
    }

    #[test]
    fn listing_a_lazy_hoster_gives_idle() {
        let lazy = Hoster {
            hoster_name: "lazy".to_owned(),
            lazy: true,
            ..Hoster::default()
        };
        assert_eq!(
            load_hoster_state(&Fake::default(), &lazy),
            HosterState::Idle("lazy".to_owned())
        );
    }

    #[test]
    fn listing_videos_gives_ready_with_queue_states() {
        let source = Fake {
            listed: vec![video("a", false), video("b", true)],
            http: true,
            ..Fake::default()
        };
        let plain = Hoster {
            hoster_name: NO_HOSTER_LIST.to_owned(),
            ..Hoster::default()
        };
        let HosterState::Ready(ready) = load_hoster_state(&source, &plain) else {
            panic!("expected Ready");
        };
        assert_eq!(ready.video_list.len(), 2);
        assert_eq!(ready.video_state, vec![VideoState::Queue; 2]);
    }

    #[tokio::test]
    async fn preferred_resolved_video_wins_the_race() {
        let source = Arc::new(Fake {
            resolved: vec![("p".to_owned(), "p-final".to_owned())],
            http: true,
            ..Fake::default()
        });
        let hosters = vec![
            hoster("first", vec![video("a", false)]),
            hoster("second", vec![video("p", true)]),
        ];
        let winner = get_best_video(source, hosters).await.unwrap();
        assert_eq!(winner.video_url, "p-final");
        assert!(winner.initialized);
    }

    #[tokio::test]
    async fn failed_preferred_video_falls_back_to_the_first_valid_one() {
        let source = Arc::new(Fake::default());
        let hosters = vec![
            hoster("first", vec![video("a", false)]),
            hoster("second", vec![video("", true)]),
        ];
        let winner = get_best_video(source, hosters).await.unwrap();
        assert_eq!(winner.video_url, "a");
    }

    #[tokio::test]
    async fn nothing_resolvable_gives_none() {
        let source = Arc::new(Fake::default());
        let hosters = vec![hoster("only", vec![video("", false), video("", true)])];
        assert_eq!(get_best_video(source, hosters).await, None);
    }

    #[tokio::test]
    async fn no_hosters_gives_none() {
        assert_eq!(
            get_best_video(Arc::new(Fake::default()), Vec::new()).await,
            None
        );
    }
}

#[cfg(test)]
mod local_tests {
    use super::*;

    #[test]
    fn a_local_episode_becomes_one_file_video_named_after_its_url() {
        let dir = tempfile::tempdir().unwrap();
        let anime = dir.path().join("Show");
        std::fs::create_dir(&anime).unwrap();
        std::fs::write(anime.join("01.mkv"), b"x").unwrap();
        let videos = local_videos(dir.path(), "Show/01.mkv");
        assert_eq!(videos.len(), 1);
        assert!(videos[0].video_url.starts_with("file://"));
        assert!(videos[0].video_url.ends_with("/Show/01.mkv"));
        assert_eq!(videos[0].video_title, "Local source: Show/01.mkv");
    }

    #[test]
    fn a_missing_file_or_a_url_without_a_slash_gives_no_videos() {
        let dir = tempfile::tempdir().unwrap();
        assert!(local_videos(dir.path(), "Show/none.mkv").is_empty());
        assert!(local_videos(dir.path(), "no-slash").is_empty());
    }
}

#[cfg(test)]
mod group_tests {
    use super::*;
    use crate::types::{ReadyHoster, Video, VideoState};

    fn ready(name: &str, videos: usize) -> HosterState {
        HosterState::Ready(ReadyHoster {
            name: name.to_owned(),
            video_list: vec![Video::default(); videos],
            video_state: vec![VideoState::Queue; videos],
        })
    }

    fn sample() -> Vec<HosterState> {
        vec![
            HosterState::Idle("A".to_owned()),
            HosterState::Loading("B".to_owned()),
            ready("C", 2),
            HosterState::Error("D".to_owned()),
            ready("E", 0),
        ]
    }

    #[test]
    fn groups_are_valid_failed_and_empty_in_input_order() {
        let g = group_hosters(&sample());
        assert_eq!(g.valid, vec![0, 1, 2]);
        assert_eq!(g.failed, vec![3]);
        assert_eq!(g.empty, vec![4]);
    }

    #[test]
    fn the_view_applies_the_failed_and_empty_flags_not_the_grouping() {
        let g = group_hosters(&sample());
        assert_eq!(g.failed_shown(true), &[3]);
        assert!(g.failed_shown(false).is_empty());
        assert_eq!(g.empty_shown(true), &[4]);
        assert!(g.empty_shown(false).is_empty());
    }

    #[test]
    fn no_hosters_give_empty_groups() {
        assert_eq!(group_hosters(&[]), HosterGroups::default());
    }

    #[test]
    fn flat_mode_needs_one_ready_no_hoster_list() {
        assert!(is_flat(&[ready(NO_HOSTER_LIST, 3)]));
        assert!(!is_flat(&[HosterState::Loading(NO_HOSTER_LIST.to_owned())]));
        assert!(!is_flat(&[ready("C", 1)]));
        assert!(!is_flat(&[ready(NO_HOSTER_LIST, 1), ready("C", 1)]));
    }

    #[test]
    fn selection_and_expansion_follow_the_hoster_name_not_the_position() {
        let mut keys = QualityKeys::default();
        keys.toggle_expanded("C");
        keys.select("C", 1);
        assert!(keys.is_expanded("C") && !keys.is_expanded("A"));
        assert!(keys.is_selected("C", 1) && !keys.is_selected("C", 0));
        let states = sample();
        assert_eq!(keys.selected_position(&states), Some((2, 1)));
        let mut reordered = states.clone();
        reordered.reverse();
        assert_eq!(keys.selected_position(&reordered), Some((2, 1)));
        reordered.insert(0, HosterState::Idle("Z".to_owned()));
        assert_eq!(keys.selected_position(&reordered), Some((3, 1)));
        assert!(keys.is_expanded("C"));
        keys.toggle_expanded("C");
        assert!(!keys.is_expanded("C"));
        assert_eq!(keys.selected_position(&states[..2]), None);
    }
}
