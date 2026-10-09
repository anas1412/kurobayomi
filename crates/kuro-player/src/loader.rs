//! Hoster list per episode kind, the load of hosters and videos, clicks, and the load gate (area 45
//! p2 T-45b-007, T-45b-013, T-45b-014, T-45b-015, T-45b-064, T-45b-066; behaviour 3.1, 3.11, 4.1 to
//! 4.4). Pure and synchronous: the source calls go through `VideoSource`, and the caller runs this
//! on a worker thread. Cancellation is the load gate's generation check.

use crate::hosters::{
    VideoSource, get_resolved_video, load_hoster_state, load_hoster_state_forced, select_best_video,
};
use crate::types::{Hoster, HosterState, ReadyHoster, Video, VideoState};

/// The kind of episode, which decides where its hosters come from (behaviour 3.1, first match).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EpisodeKind {
    Downloaded,
    Http,
    Local,
}

/// The source is none of the three kinds the player knows (`error("source not supported")`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SourceNotSupported;

/// `EpisodeLoader.getHosters` dispatch: a downloaded episode first, then an HTTP source, then a
/// local source. Anything else is not supported.
pub fn episode_kind(
    downloaded: bool,
    is_http: bool,
    is_local: bool,
) -> Result<EpisodeKind, SourceNotSupported> {
    if downloaded {
        Ok(EpisodeKind::Downloaded)
    } else if is_http {
        Ok(EpisodeKind::Http)
    } else if is_local {
        Ok(EpisodeKind::Local)
    } else {
        Err(SourceNotSupported)
    }
}

/// `getHostersOnDownloaded` (behaviour 3.4): the built video is one hoster. Any error gives an empty
/// list, silently.
pub fn downloaded_hosters<E>(built: Result<Video, E>) -> Vec<Hoster> {
    match built {
        Ok(video) => vec![Hoster::from_videos(vec![video])],
        Err(_) => Vec::new(),
    }
}

/// Why a video could not be loaded at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoadFailure {
    /// `No available videos`: nothing is playing, nothing resolves, and no hoster is still loading.
    NoAvailableVideos,
}

/// What a video click did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClickOutcome {
    /// The video is error-marked, so the click is ignored (behaviour 4.3 step 3).
    Ignored,
    /// The video loaded. The caller closes the quality sheet.
    Loaded,
    /// The video did not load, and the episode is no longer loading.
    Failed,
}

/// The hoster and video state of one episode, as the quality sheet shows it (behaviour 4.1 to 4.4).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct HosterSession {
    pub hosters: Vec<Hoster>,
    pub states: Vec<HosterState>,
    /// Accordion state of each hoster row. All start expanded (step 2).
    pub expanded: Vec<bool>,
    /// The selected `(hoster, video)`. `None` is Kotlin's `(-1, -1)`.
    pub selected: Option<(usize, usize)>,
    /// The video that is playing, once one has loaded.
    pub current_video: Option<Video>,
    /// `isLoadingEpisode`: a video load is in progress.
    pub loading_episode: bool,
}

/// Step 3a: a lazy hoster is `Idle`, a hoster without a list is `Loading`, the rest are `Ready` with
/// every video queued.
pub fn initial_states(hosters: &[Hoster]) -> Vec<HosterState> {
    hosters
        .iter()
        .map(|hoster| match (&hoster.lazy, &hoster.video_list) {
            (true, _) => HosterState::Idle(hoster.hoster_name.clone()),
            (false, None) => HosterState::Loading(hoster.hoster_name.clone()),
            (false, Some(videos)) => HosterState::Ready(ReadyHoster {
                name: hoster.hoster_name.clone(),
                video_list: videos.clone(),
                video_state: vec![VideoState::Queue; videos.len()],
            }),
        })
        .collect()
}

impl HosterSession {
    /// A session over `hosters`, with the initial states and every row expanded.
    pub fn new(hosters: Vec<Hoster>) -> Self {
        let states = initial_states(&hosters);
        let expanded = vec![true; hosters.len()];
        Self {
            hosters,
            states,
            expanded,
            selected: None,
            current_video: None,
            loading_episode: false,
        }
    }

    /// `loadHosters` (behaviour 4.1). Each hoster loads in turn. The saved `(hoster, video)` is tried
    /// first; without one, the first preferred video of each hoster is tried. If nothing has loaded
    /// and nothing is selected, the best video is tried, and no candidate at all is
    /// `NoAvailableVideos`. The first successful load wins (the `compareAndSet` of the Kotlin code).
    pub fn load_hosters<S: VideoSource>(
        &mut self,
        source: &S,
        saved: Option<(usize, usize)>,
    ) -> Result<(), LoadFailure> {
        self.states = initial_states(&self.hosters);
        self.expanded = vec![true; self.hosters.len()];
        let mut found = false;
        for index in 0..self.hosters.len() {
            let state = load_hoster_state(source, &self.hosters[index]);
            self.states[index] = state;
            if !matches!(self.states[index], HosterState::Ready(_)) {
                continue;
            }
            if let Some((saved_hoster, saved_video)) = saved
                && saved_hoster == index
            {
                let video = self.video_at(index, saved_video);
                if let Some(video) = video {
                    found = true;
                    if !self.load_video(source, video, index, saved_video)? {
                        found = false;
                    }
                }
            }
            if saved.is_none()
                && !found
                && self.selected.is_none()
                && let Some(preferred) = self.preferred_index(index)
            {
                let video = self.video_at(index, preferred);
                if let Some(video) = video {
                    found = true;
                    if !self.load_video(source, video, index, preferred)? {
                        found = false;
                    }
                }
            }
        }
        if !found && self.selected.is_none() {
            match select_best_video(&self.states) {
                Some((hoster, video)) => {
                    let picked = self.video_at(hoster, video);
                    if let Some(picked) = picked {
                        self.load_video(source, picked, hoster, video)?;
                    }
                }
                None => return Err(LoadFailure::NoAvailableVideos),
            }
        }
        Ok(())
    }

    /// `loadVideo` (behaviour 4.2) as a loop, not a recursion. Each failure marks one video `Error`,
    /// so the loop ends. `Ok(false)` is a load that did not succeed and that the caller may retry.
    pub fn load_video<S: VideoSource>(
        &mut self,
        source: &S,
        video: Video,
        hoster: usize,
        index: usize,
    ) -> Result<bool, LoadFailure> {
        let (mut hoster, mut index, mut video) = (hoster, index, video);
        loop {
            let was_ready = match self.states.get(hoster) {
                Some(HosterState::Ready(ready)) => {
                    ready.video_state.get(index) == Some(&VideoState::Ready)
                }
                _ => return Ok(false),
            };
            self.loading_episode = true;
            let old_selected = self.selected.replace((hoster, index));
            self.set_video(hoster, index, video.clone(), VideoState::LoadVideo);
            let resolved = if was_ready {
                Some(video.clone())
            } else {
                get_resolved_video(source, &video)
            };
            match resolved {
                Some(resolved) if !resolved.video_url.is_empty() => {
                    self.set_video(hoster, index, resolved.clone(), VideoState::Ready);
                    self.current_video = Some(resolved);
                    return Ok(true);
                }
                _ => {
                    self.set_video(hoster, index, video.clone(), VideoState::Error);
                    if self.current_video.is_some() {
                        self.selected = old_selected;
                        return Ok(false);
                    }
                    match select_best_video(&self.states) {
                        Some((next_hoster, next_index)) => {
                            let Some(next) = self.video_at(next_hoster, next_index) else {
                                return Ok(false);
                            };
                            hoster = next_hoster;
                            index = next_index;
                            video = next;
                        }
                        None => {
                            if self
                                .states
                                .iter()
                                .any(|state| matches!(state, HosterState::Loading(_)))
                            {
                                self.selected = None;
                                return Ok(false);
                            }
                            return Err(LoadFailure::NoAvailableVideos);
                        }
                    }
                }
            }
        }
    }

    /// `onVideoClicked` (behaviour 4.3). An error-marked video is ignored.
    pub fn on_video_clicked<S: VideoSource>(
        &mut self,
        source: &S,
        hoster: usize,
        index: usize,
    ) -> ClickOutcome {
        let Some(HosterState::Ready(ready)) = self.states.get(hoster) else {
            return ClickOutcome::Ignored;
        };
        let (Some(video), Some(state)) = (
            ready.video_list.get(index).cloned(),
            ready.video_state.get(index).copied(),
        ) else {
            return ClickOutcome::Ignored;
        };
        if state == VideoState::Error {
            return ClickOutcome::Ignored;
        }
        match self.load_video(source, video, hoster, index) {
            Ok(true) => ClickOutcome::Loaded,
            _ => {
                self.loading_episode = false;
                ClickOutcome::Failed
            }
        }
    }

    /// `onHosterClicked` (behaviour 4.4). A ready hoster toggles its accordion. An idle hoster loads
    /// now, forced. A loading or failed hoster does nothing.
    pub fn on_hoster_clicked<S: VideoSource>(&mut self, source: &S, index: usize) {
        match self.states.get(index) {
            Some(HosterState::Ready(_)) => {
                if let Some(open) = self.expanded.get_mut(index) {
                    *open = !*open;
                }
            }
            Some(HosterState::Idle(name)) => {
                let name = name.clone();
                self.states[index] = HosterState::Loading(name);
                let state = load_hoster_state_forced(source, &self.hosters[index]);
                self.states[index] = state;
            }
            _ => {}
        }
    }

    /// `resetState` / cancel (behaviour 4.1 step 4, T67): every hoster goes back to `Idle`.
    pub fn cancel(&mut self) {
        self.states = self
            .hosters
            .iter()
            .map(|hoster| HosterState::Idle(hoster.hoster_name.clone()))
            .collect();
        self.loading_episode = false;
    }

    fn video_at(&self, hoster: usize, index: usize) -> Option<Video> {
        match self.states.get(hoster)? {
            HosterState::Ready(ready) => ready.video_list.get(index).cloned(),
            _ => None,
        }
    }

    fn preferred_index(&self, hoster: usize) -> Option<usize> {
        match self.states.get(hoster)? {
            HosterState::Ready(ready) => ready.video_list.iter().position(|v| v.preferred),
            _ => None,
        }
    }

    fn set_video(&mut self, hoster: usize, index: usize, video: Video, state: VideoState) {
        if let Some(next) = self
            .states
            .get(hoster)
            .and_then(|s| s.with_video(index, video, state))
        {
            self.states[hoster] = next;
        }
    }
}

/// The load gate (T-45b-066): at most one video load is current. A new `begin` makes the earlier
/// ticket stale, and a stale result is dropped (E-38). A click while an episode loads is ignored
/// unless it is a deliberate re-selection (E-40).
#[derive(Debug, Default)]
pub struct LoadGate {
    generation: u64,
    in_flight: bool,
}

/// A load's place in the gate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);

impl LoadGate {
    /// Starts a load and makes every earlier ticket stale (the cancel-on-select rule).
    pub fn begin(&mut self) -> Ticket {
        self.generation += 1;
        self.in_flight = true;
        Ticket(self.generation)
    }

    /// Whether the ticket is still the current load.
    pub fn is_current(&self, ticket: Ticket) -> bool {
        ticket.0 == self.generation
    }

    /// Ends the load if it is still current. A stale ticket changes nothing and returns `false`.
    pub fn finish(&mut self, ticket: Ticket) -> bool {
        if !self.is_current(ticket) {
            return false;
        }
        self.in_flight = false;
        true
    }

    pub fn in_flight(&self) -> bool {
        self.in_flight
    }

    /// Whether a click may start a load. While an episode loads, a click is ignored unless it is a
    /// deliberate re-selection.
    pub fn accepts_click(&self, deliberate: bool) -> bool {
        !self.in_flight || deliberate
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A source whose hosters list their videos by hoster name, with resolve failures by URL.
    #[derive(Default)]
    struct Source {
        videos: HashMap<String, Vec<Video>>,
        resolve_fails: Vec<String>,
        http: bool,
    }

    impl VideoSource for Source {
        type Error = ();
        type Episode = ();

        fn is_http(&self) -> bool {
            self.http
        }
        fn has_hoster_list(&self) -> bool {
            false
        }
        fn hoster_list(&self, _: &()) -> Result<Vec<Hoster>, ()> {
            Ok(Vec::new())
        }
        fn sort_hosters(&self, hosters: Vec<Hoster>) -> Vec<Hoster> {
            hosters
        }
        fn episode_video_list(&self, _: &()) -> Result<Vec<Video>, ()> {
            Ok(Vec::new())
        }
        fn video_list(&self, hoster: &Hoster) -> Result<Vec<Video>, ()> {
            self.videos.get(&hoster.hoster_name).cloned().ok_or(())
        }
        fn video_url(&self, video: &Video) -> Result<String, ()> {
            Ok(video.video_url.clone())
        }
        fn sort_videos(&self, videos: Vec<Video>) -> Vec<Video> {
            videos
        }
        fn resolve_video(&self, video: &Video) -> Result<Video, ()> {
            if self.resolve_fails.contains(&video.video_url) {
                Err(())
            } else {
                Ok(Video {
                    video_url: format!("{}?resolved", video.video_url),
                    ..video.clone()
                })
            }
        }
    }

    fn video(url: &str) -> Video {
        Video {
            video_url: url.to_owned(),
            ..Video::default()
        }
    }

    fn hoster(name: &str, videos: Option<Vec<Video>>, lazy: bool) -> Hoster {
        Hoster {
            hoster_name: name.to_owned(),
            video_list: videos,
            lazy,
            ..Hoster::default()
        }
    }

    fn source(entries: &[(&str, Vec<Video>)]) -> Source {
        Source {
            videos: entries
                .iter()
                .map(|(n, v)| ((*n).to_owned(), v.clone()))
                .collect(),
            http: true,
            ..Source::default()
        }
    }

    #[test]
    fn episode_kind_is_downloaded_then_http_then_local() {
        assert_eq!(episode_kind(true, true, true), Ok(EpisodeKind::Downloaded));
        assert_eq!(episode_kind(false, true, true), Ok(EpisodeKind::Http));
        assert_eq!(episode_kind(false, false, true), Ok(EpisodeKind::Local));
        assert_eq!(episode_kind(false, false, false), Err(SourceNotSupported));
    }

    #[test]
    fn a_downloaded_episode_is_one_hoster_and_an_error_is_empty() {
        let hosters = downloaded_hosters::<()>(Ok(video("file")));
        assert_eq!(hosters.len(), 1);
        assert_eq!(hosters[0].video_list.as_ref().map(Vec::len), Some(1));
        assert!(downloaded_hosters::<()>(Err(())).is_empty());
    }

    #[test]
    fn initial_states_follow_the_lazy_and_list_rules() {
        let hosters = vec![
            hoster("lazy", None, true),
            hoster("pending", None, false),
            hoster("ready", Some(vec![video("a")]), false),
        ];
        let states = initial_states(&hosters);
        assert!(matches!(states[0], HosterState::Idle(_)));
        assert!(matches!(states[1], HosterState::Loading(_)));
        assert_eq!(states[2].video_state(0), VideoState::Queue);
    }

    #[test]
    fn the_saved_selection_is_loaded_first() {
        let src = source(&[
            ("a", vec![video("a0"), video("a1")]),
            ("b", vec![video("b0")]),
        ]);
        let mut session =
            HosterSession::new(vec![hoster("a", None, false), hoster("b", None, false)]);
        session.load_hosters(&src, Some((1, 0))).unwrap();
        assert_eq!(session.selected, Some((1, 0)));
        assert_eq!(
            session.current_video.as_ref().map(|v| v.video_url.as_str()),
            Some("b0?resolved")
        );
    }

    #[test]
    fn without_a_saved_selection_the_first_preferred_video_wins() {
        let mut preferred = video("a1");
        preferred.preferred = true;
        let src = source(&[("a", vec![video("a0"), preferred])]);
        let mut session = HosterSession::new(vec![hoster("a", None, false)]);
        session.load_hosters(&src, None).unwrap();
        assert_eq!(session.selected, Some((0, 1)));
    }

    #[test]
    fn with_nothing_to_load_it_reports_no_available_videos() {
        let src = source(&[]);
        let mut session = HosterSession::new(Vec::new());
        assert_eq!(
            session.load_hosters(&src, None),
            Err(LoadFailure::NoAvailableVideos)
        );
    }

    #[test]
    fn a_failed_resolve_marks_that_video_and_tries_the_next_best() {
        let mut src = source(&[("a", vec![video("bad"), video("good")])]);
        src.resolve_fails.push("bad".to_owned());
        let mut session = HosterSession::new(vec![hoster(
            "a",
            Some(vec![video("bad"), video("good")]),
            false,
        )]);
        assert_eq!(session.load_video(&src, video("bad"), 0, 0), Ok(true));
        assert_eq!(session.states[0].video_state(0), VideoState::Error);
        assert_eq!(session.selected, Some((0, 1)));
        assert_eq!(
            session.current_video.as_ref().map(|v| v.video_url.as_str()),
            Some("good?resolved")
        );
    }

    #[test]
    fn when_every_video_fails_the_load_stops_with_no_available_videos() {
        let mut src = source(&[("a", vec![video("x")])]);
        src.resolve_fails.push("x".to_owned());
        let mut session = HosterSession::new(vec![hoster("a", Some(vec![video("x")]), false)]);
        assert_eq!(
            session.load_video(&src, video("x"), 0, 0),
            Err(LoadFailure::NoAvailableVideos)
        );
    }

    #[test]
    fn a_failed_switch_keeps_the_playing_video_selected_again() {
        let mut src = source(&[("a", vec![video("ok"), video("bad")])]);
        src.resolve_fails.push("bad".to_owned());
        let mut session = HosterSession::new(vec![hoster(
            "a",
            Some(vec![video("ok"), video("bad")]),
            false,
        )]);
        assert_eq!(session.load_video(&src, video("ok"), 0, 0), Ok(true));
        assert_eq!(session.load_video(&src, video("bad"), 0, 1), Ok(false));
        assert_eq!(session.selected, Some((0, 0)));
        assert_eq!(session.states[0].video_state(1), VideoState::Error);
    }

    #[test]
    fn an_error_video_click_is_ignored() {
        let src = source(&[]);
        let mut session = HosterSession::new(vec![hoster("a", Some(vec![video("v")]), false)]);
        session.set_video(0, 0, video("v"), VideoState::Error);
        assert_eq!(session.on_video_clicked(&src, 0, 0), ClickOutcome::Ignored);
        assert_eq!(session.current_video, None);
    }

    #[test]
    fn a_ready_hoster_click_toggles_the_accordion() {
        let src = source(&[]);
        let mut session = HosterSession::new(vec![hoster("a", Some(vec![]), false)]);
        session.on_hoster_clicked(&src, 0);
        assert!(!session.expanded[0]);
        session.on_hoster_clicked(&src, 0);
        assert!(session.expanded[0]);
    }

    #[test]
    fn an_idle_hoster_click_loads_it_even_when_lazy() {
        let src = source(&[("lazy", vec![video("l")])]);
        let mut session = HosterSession::new(vec![hoster("lazy", None, true)]);
        session.on_hoster_clicked(&src, 0);
        assert!(matches!(session.states[0], HosterState::Ready(_)));
    }

    #[test]
    fn a_failed_hoster_click_does_nothing() {
        let src = source(&[]);
        let mut session = HosterSession::new(vec![hoster("a", None, false)]);
        session.states[0] = HosterState::Error("a".to_owned());
        session.on_hoster_clicked(&src, 0);
        assert!(matches!(session.states[0], HosterState::Error(_)));
    }

    #[test]
    fn cancel_resets_every_hoster_to_idle() {
        let mut session = HosterSession::new(vec![hoster("a", Some(vec![video("x")]), false)]);
        session.cancel();
        assert_eq!(session.states, vec![HosterState::Idle("a".to_owned())]);
    }

    #[test]
    fn a_stale_load_ticket_is_dropped_after_a_newer_select() {
        let mut gate = LoadGate::default();
        let first = gate.begin();
        let second = gate.begin();
        assert!(!gate.is_current(first));
        assert!(!gate.finish(first));
        assert!(gate.in_flight());
        assert!(gate.finish(second));
        assert!(!gate.in_flight());
    }

    #[test]
    fn clicks_are_ignored_while_loading_unless_deliberate() {
        let mut gate = LoadGate::default();
        assert!(gate.accepts_click(false));
        gate.begin();
        assert!(!gate.accepts_click(false));
        assert!(gate.accepts_click(true));
    }
}
