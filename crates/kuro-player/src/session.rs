//! The player session: the non-GTK half of the player window (area 45 `PlayerActivity`). It owns
//! the mpv host and the `PlayerState`, and joins the pure modules into the runtime paths: observer
//! event, dispatch (T-45a-020); launch (T-45a-070, T-45a-011); the
//! `FILE_LOADED` steps (T-45a-013, T-45a-016); filters, fonts, volume restore, screenshots and the
//! external-player run (T-45a-061). The GTK window forwards events here and applies what comes back.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use kuro_platform::media::MediaCommand;
use kuro_source_http::local_server::LocalHandler;

use crate::chapters::{self, Segment};
use crate::completion::Completion;
use crate::desktop::Desktop;
use crate::config::{apply_fonts_dir, ensure_mpv_config};
use crate::enums::{Decoder, GpuKind, VideoRotate};
use crate::errors::PlayerMessage;
use crate::external::{self, ExternalError, FinalSave, Launch, Plan, Preset};
use crate::ipc;
use crate::load::{FileLoadedSteps, VideoSpec, loadfile_command, run_file_loaded};
use crate::local_server::{LocalServer, prepare_url};
use crate::mpv::{MpvError, MpvHandle, PropertySink, PropertySource};
use crate::observer::{MpvEvent, observe_all};
use crate::options::{
    FILTER_NAMES, PlayerPrefs, VideoChain, filter_write, get_decoder, startup_settings,
    update_decoder,
};
use crate::progress::{self, SecondAction};
use crate::screenshot::screenshot_result;
use crate::seek::absolute_flag;
use crate::skip::{Label, SkipConfig, SkipIntro, SkipSeek, Tap, Tick};
use crate::state::PlayerState;
use crate::system_av::volume_max_write;
use crate::tracks::{
    AddKind, TrackValue, add_file_args, external_track_commands, select_sub, track_id_from_value,
};
use crate::tracking::Tracking;
use crate::types::{TimeStamp, Track, Video};
use crate::volume::key_step;
use crate::window::{CommandSink, accepts_event, execute, on_resume, take_volume_restore};

/// The `FILE_LOADED` steps that the window performs itself, after the session ran the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileLoadedStep {
    MediaTitle,
    Chapters,
    SkipIntro,
}

/// What the window does with one mpv event.
#[derive(Clone, Debug, PartialEq)]
pub enum Dispatch {
    /// The window is exiting, so the event was dropped.
    Dropped,
    /// `FILE_LOADED` ran. The metadata options were applied and the side tracks were added.
    FileLoaded {
        pending: Vec<FileLoadedStep>,
        finish_tracks_now: bool,
    },
    /// The state took the event, or the event needs nothing.
    Handled,
    /// A controller event the state does not own (time, duration, tracks, end of file).
    Forward(MpvEvent),
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("{}", .0.text())]
    Message(PlayerMessage),
    #[error(transparent)]
    Mpv(#[from] MpvError),
}

/// What the loaded video carries that `FILE_LOADED` needs.
#[derive(Default)]
struct Loaded {
    url: String,
    audio: Vec<Track>,
    subtitles: Vec<Track>,
    timestamps: Vec<TimeStamp>,
}

/// What one whole-second tick leaves for the window (behaviour 7.1 and 6.8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SecondReport {
    /// The decisions of the tick, for the progress save and the download-ahead step. `None` while
    /// the episode loads, without an episode, or while the duration is unknown.
    pub action: Option<SecondAction>,
    /// The episode went past the completion threshold on this tick, for the first time.
    pub completed: bool,
    /// The countdown toast of the netflix-style skip, shown once per countdown.
    pub toast: Option<String>,
}

pub struct Session<H> {
    pub host: H,
    pub state: PlayerState,
    /// The trackers of the playing anime and the ports a seen episode is sent through.
    pub tracking: Tracking,
    loaded: Loaded,
    skip_config: SkipConfig,
    skip: SkipIntro,
    completion: Completion,
    /// `pref_player_smooth_seek`, for the seeks the skip button makes.
    precise_seek: bool,
    /// The inputs of the `vf` chain, so a filter change rewrites the whole chain (E-26).
    video: VideoChain,
    gpu: GpuKind,
    /// Media keys, now-playing and the idle inhibit; they follow the pause state of every event.
    pub desktop: Desktop,
}

impl<H: PropertySink + PropertySource + CommandSink> Session<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            state: PlayerState::default(),
            tracking: Tracking::default(),
            loaded: Loaded::default(),
            skip_config: SkipConfig::default(),
            skip: SkipIntro::new(SkipConfig::default()),
            completion: Completion::default(),
            precise_seek: false,
            video: VideoChain::default(),
            gpu: GpuKind::default(),
            desktop: Desktop::default(),
        }
    }

    /// Sets the skip-intro preferences and starts the machine again.
    pub fn set_skip_config(&mut self, config: SkipConfig) {
        self.skip_config = config;
        self.skip = SkipIntro::new(config);
    }

    pub fn set_precise_seek(&mut self, precise: bool) {
        self.precise_seek = precise;
    }

    /// `selectSub` (behaviour 5.6): works out the new `(sid, secondary-sid)` pair from what mpv
    /// has now, then writes `secondary-sid` and `sid` in the Kotlin order. Returns the new pair.
    pub fn select_sub(&self, id: i64) -> Result<(i64, i64), MpvError> {
        let read = |name| track_id_from_value(self.host.get_str(name).as_deref());
        let (sid, secondary) = select_sub((read("sid"), read("secondary-sid")), id);
        for (name, value) in [("secondary-sid", secondary), ("sid", sid)] {
            let text = match TrackValue::for_id(value) {
                TrackValue::No => "no".to_owned(),
                TrackValue::Id(id) => id.to_string(),
            };
            self.host.set_str(name, &text)?;
        }
        Ok((sid, secondary))
    }

    /// `addAudio` / `addSubtitle` for a file the user picked: sends `audio-add` or `sub-add`. The
    /// track list reloads on the next `track-list` event. A path that is not valid text sends nothing.
    pub fn add_track(&self, kind: AddKind, path: &Path) -> Result<(), MpvError>
    where
        H: CommandSink,
    {
        let Some(args) = add_file_args(kind, path) else {
            tracing::warn!("track file path is not valid text; not added");
            return Ok(());
        };
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        self.host.command(&args)
    }

    /// The decoder mpv is using now (`getDecoder`); its `title()` is shown in the More sheet.
    pub fn get_decoder(&self) -> Decoder {
        get_decoder(&self.host)
    }

    /// `updateDecoder`: writes `hwdec` for the chosen decoder.
    pub fn update_decoder(&self, decoder: Decoder) -> Result<(), MpvError> {
        update_decoder(&self.host, decoder, self.gpu)
    }

    /// Loads a video: the local server is started for a placeholder URL, then `loadfile` is sent.
    /// A failed server start stops the load with its toast message.
    pub fn load_video(
        &mut self,
        video: &Video,
        server: &mut LocalServer,
        lookup: impl FnOnce() -> Option<Arc<dyn LocalHandler>>,
    ) -> Result<(), LoadError> {
        let url = prepare_url(&video.video_url, server, lookup).map_err(LoadError::Message)?;
        let spec = VideoSpec {
            url: url.clone(),
            mpv_args: video.mpv_args.clone(),
        };
        self.loaded = Loaded {
            url,
            audio: video.audio_tracks.clone(),
            subtitles: video.subtitle_tracks.clone(),
            timestamps: video.timestamps.clone(),
        };
        if let Some(command) = loadfile_command(Some(&spec)) {
            let args: Vec<&str> = command.iter().map(String::as_str).collect();
            execute(&self.state, &self.host, &args)?;
        }
        Ok(())
    }

    /// Handles one drained mpv event.
    pub fn on_event(&mut self, event: MpvEvent) -> Dispatch {
        if !accepts_event(&self.state) {
            return Dispatch::Dropped;
        }
        match &event {
            MpvEvent::FileLoaded => self.on_file_loaded(),
            MpvEvent::Seek => {
                self.state.on_seek();
                Dispatch::Handled
            }
            MpvEvent::PropertyFlag(name, value) if self.state.on_flag(name, *value) => {
                let state = &self.state;
                self.desktop.sync(state.paused, state.is_exiting, state.pos);
                Dispatch::Handled
            }
            MpvEvent::PropertyDouble(name, value) if self.state.on_double(name, *value) => {
                Dispatch::Handled
            }
            _ => Dispatch::Forward(event),
        }
    }

    fn on_file_loaded(&mut self) -> Dispatch {
        // `waitingSkipIntro` is read again on every file: each episode starts its own countdown.
        self.skip = SkipIntro::new(self.skip_config);
        let mut steps = LoadedSteps {
            host: &self.host,
            loaded: &self.loaded,
            pending: Vec::new(),
            finish_tracks_now: false,
        };
        let state = &self.state;
        run_file_loaded(&mut steps, || state.is_exiting);
        self.publish_now_playing(None);
        Dispatch::FileLoaded {
            pending: steps.pending,
            finish_tracks_now: steps.finish_tracks_now,
        }
    }

    /// Publishes the playing episode to the desktop media widget: the anime and media title and, when the window knows
    /// one, the artwork URL. Called on `FILE_LOADED`, and again by the window once the title or duration is known.
    pub fn publish_now_playing(&self, artwork: Option<&str>) {
        let state = &self.state;
        let title = match (state.anime_title.is_empty(), state.media_title.is_empty()) {
            (false, false) => format!("{} - {}", state.anime_title, state.media_title),
            (false, true) => state.anime_title.clone(),
            _ => state.media_title.clone(),
        };
        self.desktop.now_playing(&title, artwork, state.duration);
    }

    /// Runs a media key or media widget command on the player. Next and Previous switch episodes, which the window
    /// owns, so they come back as `Some`.
    pub fn apply_media_command(&self, command: MediaCommand) -> Option<MediaCommand> {
        let secs = |d: Duration| d.as_secs_f64().to_string();
        let result = match command {
            MediaCommand::Play => self.host.set_str("pause", "no"),
            MediaCommand::Pause => self.host.set_str("pause", "yes"),
            MediaCommand::Toggle => execute(&self.state, &self.host, &["cycle", "pause"]),
            MediaCommand::Stop => execute(&self.state, &self.host, &["stop"]),
            MediaCommand::SeekForward(d) => execute(&self.state, &self.host, &["seek", &secs(d), "relative"]),
            MediaCommand::SeekBackward(d) => execute(&self.state, &self.host, &["seek", &format!("-{}", secs(d)), "relative"]),
            MediaCommand::SeekTo(d) => execute(&self.state, &self.host, &["seek", &secs(d), "absolute"]),
            MediaCommand::Next | MediaCommand::Previous => return Some(command),
        };
        if let Err(e) = result {
            tracing::warn!("media command {command:?} failed: {e}");
        }
        None
    }

    /// The chapter list with the video's timestamps merged in, or `None` to leave it as it is.
    pub fn merged_chapters(
        &self,
        current: &[Segment],
        duration: Option<i32>,
    ) -> Option<Vec<Segment>> {
        chapters::with_timestamps(current, &self.loaded.timestamps, duration)
    }

    /// `setupChapters`: stores the chapters read from mpv with the video's timestamps merged in,
    /// then makes the chapter at the playback position current (`setChapter`, T-45b-022).
    pub fn set_chapters(&mut self, from_mpv: Vec<Segment>, duration: Option<i32>) {
        self.state.chapters = self
            .merged_chapters(&from_mpv, duration)
            .unwrap_or(from_mpv);
        self.sync_skip();
    }

    /// Merges AniSkip intervals into the chapters and syncs the skip button (`fileLoaded`, after
    /// `aniskip::lookup`). An empty list changes nothing.
    pub fn apply_aniskip(&mut self, stamps: &[TimeStamp], duration: Option<i32>) {
        if let Some(merged) = chapters::with_timestamps(&self.state.chapters, stamps, duration) {
            self.state.chapters = merged;
            self.sync_skip();
        }
    }

    /// The AniSkip step of `fileLoaded`: when `aniskip::lookup_wanted` allows it, looks up the intervals of the
    /// playing episode with `aniskip::lookup` and merges them into the chapters. The host passes the tracker refs of
    /// the anime in track order and the four preference values; an unwanted or failed lookup changes nothing.
    pub async fn run_aniskip(
        &mut self,
        api: &impl crate::aniskip::SkipApi,
        tracks: &[crate::aniskip::TrackerRef],
        wanted: bool,
    ) {
        let Some(episode) = self.state.current_episode.as_ref().map(|e| e.episode_number) else {
            return;
        };
        if !wanted {
            return;
        }
        let duration = self.state.duration;
        let stamps = crate::aniskip::lookup(api, tracks, episode, Some(f64::from(duration))).await;
        if let Some(stamps) = stamps {
            self.apply_aniskip(&stamps, Some(duration as i32));
        }
    }

    /// The whole-second tick (`time-pos` as an integer): `updatePlayBackPos` then `setChapter`.
    /// It reads the progress decisions, marks the episode seen once it completes and sends that
    /// to the trackers, then drives the skip-intro machine (T-45a-066, T-45b-022).
    pub fn on_time_pos(&mut self, position_s: i64) -> SecondReport {
        let policy = self.tracking.policy;
        let allowed = progress::progress_allowed(policy.incognito_source, self.tracking.has_trackers());
        let action = progress::on_second(
            self.state.is_loading_episode,
            self.state.current_episode.as_ref().map(|e| e.id),
            position_s,
            self.state.duration as i64,
            policy.threshold,
            allowed,
        );
        let mut completed = false;
        if let (Some(action), Some(episode)) = (&action, self.state.current_episode.as_mut()) {
            episode.last_second_seen = action.position_ms;
            episode.total_seconds = action.total_ms;
            if action.completed && self.completion.complete(episode) {
                completed = true;
                self.tracking.on_seen(episode.episode_number);
            }
        }
        self.state.pos = position_s as f32;
        self.set_current_chapter();
        let tick = self.skip.on_tick(&self.state.chapters, self.state.pos);
        SecondReport {
            action,
            completed,
            toast: self.apply_tick(tick),
        }
    }

    /// `onSkipIntro`: the user taps the skip button.
    pub fn on_skip_tap(&mut self) -> Tap {
        let tap = self.skip.on_skip(&self.state.chapters, self.state.pos);
        if let Tap::Seek(seek) = &tap {
            self.seek_with_text(seek.clone());
        }
        tap
    }

    fn sync_skip(&mut self) {
        self.set_current_chapter();
        let tick = self.skip.sync(&self.state.chapters, self.state.pos);
        self.apply_tick(tick);
    }

    /// `_currentChapter` follows the position. It is left alone before the first chapter.
    fn set_current_chapter(&mut self) {
        let state = &mut self.state;
        if let Some(i) = chapters::current_chapter(&state.chapters, state.pos) {
            if state.current_chapter.as_ref() != Some(&state.chapters[i]) {
                state.current_chapter = Some(state.chapters[i].clone());
            }
        }
    }

    /// Applies the label and the seek of a skip tick and hands back its toast.
    fn apply_tick(&mut self, tick: Tick) -> Option<String> {
        if let Label::Set(text) = tick.label {
            self.state.skip_intro_text = text;
        }
        if let Some(seek) = tick.seek {
            self.seek_with_text(seek);
        }
        tick.toast
    }

    /// `seekToWithText`: the seek indicator state, then an absolute seek inside the duration.
    fn seek_with_text(&mut self, seek: SkipSeek) {
        self.state.is_seeking_forwards = seek.position > 0;
        self.state.double_tap_seek_amount = (seek.position - self.state.pos as i64) as i32;
        self.state.seek_text = Some(seek.text);
        if !(0..=self.state.duration as i64).contains(&seek.position) {
            return;
        }
        let position = seek.position.to_string();
        let args = ["seek", position.as_str(), absolute_flag(self.precise_seek)];
        if let Err(e) = execute(&self.state, &self.host, &args) {
            tracing::error!("failed to seek to {position}: {e}");
        }
    }

    /// One video filter change, clamped (T-45a-032). A name outside the five filters writes
    /// nothing. The filters live in the `vf` chain, so the whole chain is written again.
    pub fn set_filter(&mut self, name: &str, value: i64) -> Result<(), MpvError> {
        let Some((name, value)) = filter_write(name, value) else {
            return Ok(());
        };
        if let Some(i) = FILTER_NAMES.iter().position(|f| *f == name) {
            self.video.filters[i] = value as i32;
        }
        self.host.set_str("vf", &self.video.vf())
    }

    /// Writes `volume-max` for a boost cap: before the first playback and on every cap change
    /// (E-46-025).
    pub fn set_boost_cap(&self, boost_cap: i32) -> Result<(), MpvError> {
        let (name, value) = volume_max_write(boost_cap);
        self.host.set_str(name, &value.to_string())
    }

    /// `onResume`: clears the exit gate and, once after a hidden window, applies the volume rule
    /// (T-45a-046). `device_steps` is the device volume read by the caller.
    pub fn resume(&mut self, device_steps: i32) -> Result<(), MpvError> {
        on_resume(&mut self.state);
        match take_volume_restore(&mut self.state, device_steps) {
            Some(volume) => self.host.set_str("volume", &volume.to_string()),
            None => Ok(()),
        }
    }

    /// Sets mpv `video-rotate` from the desktop rotation control (T-45a-035).
    pub fn set_video_rotate(&self, rotate: VideoRotate) -> Result<(), MpvError> {
        self.host.set_str("video-rotate", &rotate.degrees().to_string())
    }

    /// A volume key press (T-45a-046): mpv's own `volume` moves one step inside `0..=volume-max`.
    pub fn volume_key(&self, step: i32) -> Result<(), MpvError> {
        let volume = self.read_int("volume").unwrap_or(100);
        let max = self.read_int("volume-max").unwrap_or(130);
        self.host
            .set_str("volume", &key_step(volume, step, max).to_string())
    }

    fn read_int(&self, name: &str) -> Option<i64> {
        self.host.get_str(name)?.parse::<f64>().ok().map(|v| v as i64)
    }

    /// The toast for a finished screenshot, if any.
    pub fn screenshot_done(&self, result: Result<(), &str>) -> Option<PlayerMessage> {
        screenshot_result(result)
    }
}

impl Session<MpvHandle> {
    /// Creates the mpv config directory and any missing empty config files, creates the mpv core from the startup table, observes the
    /// property set and points the font lookup at `fonts_dir`. The video filters are in the `vf`
    /// chain of the startup table.
    pub fn start(prefs: &PlayerPrefs, fonts_dir: &Path) -> Result<Self, MpvError> {
        prepare_mpv_dir(prefs);
        let mut host = MpvHandle::create(&startup_settings(prefs))?;
        observe_all(&mut host)?;
        apply_fonts_dir(&host, fonts_dir)?;
        let mut session = Self::new(host);
        session.video = VideoChain::from(prefs);
        session.gpu = prefs.gpu;
        session.desktop = Desktop::connect();
        Ok(session)
    }
}

/// Step before mpv is created: the config files are created when they are missing and never rewritten. A failure is
/// logged and does not stop playback.
pub fn prepare_mpv_dir(prefs: &PlayerPrefs) {
    let dir = prefs.data_dir.join("mpv");
    if let Err(e) = ensure_mpv_config(&dir) {
        tracing::error!("failed to create the mpv config files: {e}");
    }
}

/// The `FILE_LOADED` steps the session performs. The rest are handed to the window as `pending`.
struct LoadedSteps<'a, H> {
    host: &'a H,
    loaded: &'a Loaded,
    pending: Vec<FileLoadedStep>,
    finish_tracks_now: bool,
}

impl<H: PropertySink + PropertySource + CommandSink> FileLoadedSteps for LoadedSteps<'_, H> {
    fn mpv_options(&mut self) {
        let metadata = self.host.get_str("metadata");
        crate::metadata::apply_from_metadata(self.host, &self.loaded.url, metadata.as_deref());
    }
    fn media_title(&mut self) {
        self.pending.push(FileLoadedStep::MediaTitle);
    }
    fn chapters(&mut self) {
        self.pending.push(FileLoadedStep::Chapters);
    }
    fn tracks(&mut self) {
        let tracks = external_track_commands(&self.loaded.audio, &self.loaded.subtitles);
        for command in &tracks.commands {
            let args: Vec<&str> = command.iter().map(String::as_str).collect();
            if let Err(e) = self.host.command(&args) {
                tracing::error!("failed to add external track: {e}");
            }
        }
        self.finish_tracks_now = tracks.finish_now;
    }
    fn skip_intro(&mut self) {
        self.pending.push(FileLoadedStep::SkipIntro);
    }
}

/// Starts an external player and waits for it to exit. With an IPC socket the position is polled
/// while it runs, and the final save follows. `None` when nothing was watched (the desktop default,
/// or no socket).
pub fn run_external(
    preset: &Preset,
    launch: &Launch,
    path_var: &OsStr,
    poll: Duration,
) -> Result<Option<FinalSave>, ExternalError> {
    let plan: Plan = external::plan(preset, launch, path_var)?;
    let (Some(mut child), Some(socket)) = (external::run(&plan)?, launch.ipc_socket) else {
        return Ok(None);
    };
    let last = ipc::watch_until_exit(&mut child, socket, poll)
        .map_err(|e| ExternalError::Spawn(e.to_string()))?;
    Ok(last
        .time_pos
        .zip(last.duration)
        .map(|(position, duration)| external::final_save(position, duration)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mpv::MpvError;
    use crate::tracks::TRACK_NONE;
    use std::cell::RefCell;
    use std::collections::HashMap;

    #[derive(Default)]
    struct Fake {
        props: RefCell<HashMap<String, String>>,
        commands: RefCell<Vec<Vec<String>>>,
        writes: RefCell<Vec<(String, String)>>,
    }

    impl PropertySink for Fake {
        fn set_str(&self, name: &str, value: &str) -> Result<(), MpvError> {
            self.writes
                .borrow_mut()
                .push((name.to_owned(), value.to_owned()));
            Ok(())
        }
        fn set_i64(&self, name: &str, value: i64) -> Result<(), MpvError> {
            self.set_str(name, &format!("i64:{value}"))
        }
        fn set_f64(&self, name: &str, value: f64) -> Result<(), MpvError> {
            self.set_str(name, &format!("f64:{value}"))
        }
    }
    impl PropertySource for Fake {
        fn get_str(&self, name: &str) -> Option<String> {
            self.props.borrow().get(name).cloned()
        }
    }
    impl CommandSink for Fake {
        fn command(&self, args: &[&str]) -> Result<(), MpvError> {
            self.commands
                .borrow_mut()
                .push(args.iter().map(|a| (*a).to_owned()).collect());
            Ok(())
        }
    }

    fn video(url: &str) -> Video {
        Video {
            video_url: url.to_owned(),
            ..Video::default()
        }
    }

    #[test]
    fn media_keys_drive_the_player_and_episode_switches_go_to_the_window() {
        let session = Session::new(Fake::default());
        assert_eq!(session.apply_media_command(MediaCommand::Pause), None);
        assert_eq!(session.apply_media_command(MediaCommand::SeekBackward(Duration::from_secs(10))), None);
        assert_eq!(session.apply_media_command(MediaCommand::SeekTo(Duration::from_secs(5))), None);
        assert_eq!(session.apply_media_command(MediaCommand::Next), Some(MediaCommand::Next));
        assert_eq!(session.host.writes.borrow().as_slice(), [("pause".to_owned(), "yes".to_owned())]);
        let commands = session.host.commands.borrow();
        assert_eq!(commands[0], ["seek", "-10", "relative"]);
        assert_eq!(commands[1], ["seek", "5", "absolute"]);
    }

    #[test]
    fn a_pause_event_reaches_the_desktop_ports() {
        let mut session = Session::new(Fake::default());
        session.on_event(MpvEvent::PropertyFlag("pause".into(), false));
        assert!(!session.desktop.holds_awake(), "the default desktop has no inhibitor");
    }

    #[test]
    fn a_boost_cap_change_writes_the_new_volume_max() {
        let session = Session::new(Fake::default());
        session.set_boost_cap(30).unwrap();
        session.set_boost_cap(80).unwrap();
        let writes = session.host.writes.borrow();
        assert_eq!(writes[0], ("volume-max".to_owned(), "130".to_owned()));
        assert_eq!(writes[1], ("volume-max".to_owned(), "180".to_owned()));
    }

    #[test]
    fn exiting_window_drops_events() {
        let mut session = Session::new(Fake::default());
        session.state.is_exiting = true;
        assert_eq!(session.on_event(MpvEvent::Seek), Dispatch::Dropped);
        assert!(!session.state.is_loading);
    }

    #[test]
    fn state_events_update_the_state_and_the_rest_is_forwarded() {
        let mut session = Session::new(Fake::default());
        assert_eq!(session.on_event(MpvEvent::Seek), Dispatch::Handled);
        assert!(session.state.is_loading);
        assert_eq!(
            session.on_event(MpvEvent::PropertyFlag("pause".to_owned(), true)),
            Dispatch::Handled
        );
        assert_eq!(session.state.paused, Some(true));
        let time = MpvEvent::PropertyInt("time-pos".to_owned(), 5);
        assert_eq!(session.on_event(time.clone()), Dispatch::Forward(time));
    }

    #[test]
    fn launch_rewrites_the_placeholder_then_sends_loadfile() {
        struct Fixed;
        impl LocalHandler for Fixed {
            fn serve(
                &self,
                _: &str,
                _: Option<&str>,
            ) -> Result<kuro_source_http::local_server::LocalResponse, kuro_source::SourceError>
            {
                unreachable!("nothing connects in this test")
            }
        }
        let mut session = Session::new(Fake::default());
        let mut server = LocalServer::default();
        let mut v = video("http://localhost:1/s.m3u8");
        v.mpv_args = vec![("speed".to_owned(), "2".to_owned())];
        session
            .load_video(&v, &mut server, || Some(Arc::new(Fixed)))
            .unwrap();
        let port = server.port().unwrap();
        assert_eq!(
            *session.host.commands.borrow(),
            vec![vec![
                "loadfile".to_owned(),
                format!("http://localhost:{port}/s.m3u8"),
                "replace".to_owned(),
                "0".to_owned(),
                "speed=\"2\"".to_owned(),
            ]]
        );
    }

    #[test]
    fn failed_server_start_stops_the_load_with_its_toast() {
        let mut session = Session::new(Fake::default());
        let mut server = LocalServer::default();
        let err = session
            .load_video(&video("http://localhost:1/s.m3u8"), &mut server, || None)
            .unwrap_err();
        assert!(matches!(
            err,
            LoadError::Message(PlayerMessage::HttpServerStartFailure)
        ));
        assert!(session.host.commands.borrow().is_empty());
    }

    #[test]
    fn file_loaded_reads_metadata_and_applies_options_for_a_local_video() {
        let mut session = Session::new(Fake::default());
        session.host.props.borrow_mut().insert(
            "metadata".to_owned(),
            serde_json::json!({ "ANIYOMI_MPV_ARGS": "speed=1.5" }).to_string(),
        );
        session
            .load_video(&video("file:///a.mkv"), &mut LocalServer::default(), || {
                None
            })
            .unwrap();
        let out = session.on_event(MpvEvent::FileLoaded);
        assert_eq!(
            *session.host.writes.borrow(),
            vec![("speed".to_owned(), "1.5".to_owned())]
        );
        assert_eq!(
            out,
            Dispatch::FileLoaded {
                pending: vec![
                    FileLoadedStep::MediaTitle,
                    FileLoadedStep::Chapters,
                    FileLoadedStep::SkipIntro
                ],
                finish_tracks_now: true,
            }
        );
    }

    #[test]
    fn file_loaded_leaves_an_online_video_alone_and_adds_its_side_tracks() {
        let mut session = Session::new(Fake::default());
        session.host.props.borrow_mut().insert(
            "metadata".to_owned(),
            serde_json::json!({ "ANIYOMI_MPV_ARGS": "speed=1.5" }).to_string(),
        );
        let mut v = video("https://cdn/x.m3u8");
        v.subtitle_tracks = vec![Track {
            url: "https://s/1.vtt".to_owned(),
            lang: "en".to_owned(),
        }];
        session
            .load_video(&v, &mut LocalServer::default(), || None)
            .unwrap();
        session.host.commands.borrow_mut().clear();
        let out = session.on_event(MpvEvent::FileLoaded);
        assert!(session.host.writes.borrow().is_empty());
        assert_eq!(
            *session.host.commands.borrow(),
            vec![vec!["sub-add", "https://s/1.vtt", "auto", "en"]]
        );
        assert!(matches!(
            out,
            Dispatch::FileLoaded {
                finish_tracks_now: false,
                ..
            }
        ));
    }

    #[test]
    fn filter_change_is_clamped_and_unknown_names_write_nothing() {
        let mut session = Session::new(Fake::default());
        session.set_filter("gamma", 500).unwrap();
        session.set_filter("volume", 5).unwrap();
        session.set_filter("hue", 50).unwrap();
        assert_eq!(
            *session.host.writes.borrow(),
            vec![
                ("vf".to_owned(), "eq=gamma=8,format=yuv420p".to_owned()),
                (
                    "vf".to_owned(),
                    "eq=gamma=8,hue=h=90,format=yuv420p".to_owned()
                ),
            ]
        );
    }

    fn sub_session(sid: &str, secondary: &str) -> Session<Fake> {
        let session = Session::new(Fake::default());
        let mut props = session.host.props.borrow_mut();
        props.insert("sid".to_owned(), sid.to_owned());
        props.insert("secondary-sid".to_owned(), secondary.to_owned());
        drop(props);
        session
    }

    #[test]
    fn select_sub_writes_secondary_then_primary() {
        let session = sub_session("no", "no");
        assert_eq!(session.select_sub(3).unwrap(), (3, TRACK_NONE));
        let session = sub_session("1", "no");
        assert_eq!(session.select_sub(4).unwrap(), (1, 4));
        assert_eq!(
            *session.host.writes.borrow(),
            vec![
                ("secondary-sid".to_owned(), "4".to_owned()),
                ("sid".to_owned(), "1".to_owned()),
            ]
        );
    }

    #[test]
    fn picking_the_primary_promotes_the_secondary_in_mpv() {
        let session = sub_session("1", "2");
        assert_eq!(session.select_sub(1).unwrap(), (2, TRACK_NONE));
        assert_eq!(
            *session.host.writes.borrow(),
            vec![
                ("secondary-sid".to_owned(), "no".to_owned()),
                ("sid".to_owned(), "2".to_owned()),
            ]
        );
    }

    #[test]
    fn decoder_is_read_from_mpv_and_written_to_hwdec() {
        let mut session = Session::new(Fake::default());
        session.gpu = GpuKind::Intel;
        session
            .host
            .props
            .borrow_mut()
            .insert("hwdec-current".to_owned(), "nvdec".to_owned());
        assert_eq!(session.get_decoder().title(), "HW+");
        session.update_decoder(Decoder::Hw).unwrap();
        assert_eq!(
            *session.host.writes.borrow(),
            vec![("hwdec".to_owned(), "vaapi-copy".to_owned())]
        );
    }

    #[test]
    fn prepare_creates_the_config_files_and_keeps_the_users_text() {
        let dir = tempfile::tempdir().unwrap();
        let prefs = PlayerPrefs {
            data_dir: dir.path().join("data"),
            ..PlayerPrefs::default()
        };
        prepare_mpv_dir(&prefs);
        let mpv = dir.path().join("data/mpv");
        std::fs::write(mpv.join("mpv.conf"), "volume=40\n").unwrap();
        prepare_mpv_dir(&prefs);
        assert_eq!(std::fs::read_to_string(mpv.join("mpv.conf")).unwrap(), "volume=40\n");
        assert!(mpv.join("input.conf").is_file());
    }

    #[test]
    fn resume_restores_the_volume_once_and_screenshot_toasts() {
        let mut session = Session::new(Fake::default());
        session.state.is_exiting = true;
        session.resume(3).unwrap();
        session.resume(3).unwrap();
        assert_eq!(
            *session.host.writes.borrow(),
            vec![("volume".to_owned(), "100".to_owned())]
        );
        assert_eq!(
            session.screenshot_done(Ok(())),
            Some(PlayerMessage::PictureSaved)
        );
        assert_eq!(session.screenshot_done(Err("full")), None);
    }

    #[test]
    fn t45a035_the_rotation_control_writes_video_rotate() {
        let session = Session::new(Fake::default());
        session.set_video_rotate(VideoRotate::Deg270).unwrap();
        assert_eq!(
            *session.host.writes.borrow(),
            vec![("video-rotate".to_owned(), "270".to_owned())]
        );
    }

    #[test]
    fn t46_volume_key_moves_mpv_volume_one_step_within_volume_max() {
        let session = Session::new(Fake::default());
        session.host.props.borrow_mut().insert("volume".into(), "100".into());
        session.host.props.borrow_mut().insert("volume-max".into(), "130".into());
        session.volume_key(1).unwrap();
        session.host.props.borrow_mut().insert("volume".into(), "130".into());
        session.volume_key(1).unwrap();
        session.host.props.borrow_mut().insert("volume".into(), "0".into());
        session.volume_key(-1).unwrap();
        assert_eq!(
            *session.host.writes.borrow(),
            vec![
                ("volume".to_owned(), "101".to_owned()),
                ("volume".to_owned(), "130".to_owned()),
                ("volume".to_owned(), "0".to_owned()),
            ]
        );
    }

    #[test]
    fn merged_chapters_uses_the_loaded_timestamps() {
        let mut session = Session::new(Fake::default());
        assert_eq!(session.merged_chapters(&[], Some(100)), None);
        let mut v = video("https://cdn/x");
        v.timestamps = vec![TimeStamp {
            start: 0.0,
            end: 10.0,
            name: "Intro".to_owned(),
            kind: crate::types::ChapterType::Opening,
        }];
        session
            .load_video(&v, &mut LocalServer::default(), || None)
            .unwrap();
        assert!(session.merged_chapters(&[], Some(100)).is_some());
    }

    #[test]
    fn external_run_watches_a_program_until_it_exits() {
        let launch = Launch {
            url: "https://cdn/x.mp4",
            title: "t",
            start_ms: 0,
            headers: &[],
            subtitle: None,
            ipc_socket: None,
        };
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("mpv");
        std::fs::write(&program, "#!/bin/sh\nexit 0\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        let out = run_external(
            &Preset::Mpv,
            &launch,
            dir.path().as_os_str(),
            Duration::from_millis(20),
        );
        assert_eq!(out, Ok(None));

        let socket = dir.path().join("none.sock");
        let launch = Launch {
            ipc_socket: Some(&socket),
            ..launch
        };
        // No socket answers, so nothing was read and nothing is saved.
        let out = run_external(
            &Preset::Mpv,
            &launch,
            dir.path().as_os_str(),
            Duration::from_millis(20),
        );
        assert_eq!(out, Ok(None));
    }

    #[test]
    fn load_sends_one_loadfile_command_and_writes_no_properties() {
        let mut session = Session::new(Fake::default());
        session
            .load_video(&video("file:///a.mkv"), &mut LocalServer::default(), || None)
            .unwrap();
        assert!(session.host.writes.borrow().is_empty());
        assert_eq!(session.host.commands.borrow().len(), 1);
    }

    struct Ports(std::rc::Rc<RefCell<Vec<String>>>);

    impl crate::tracking::TrackerPort for Ports {
        fn online(&self) -> bool {
            true
        }
        fn send(&self, track_id: i64, episode: f64) -> Result<(), String> {
            self.0.borrow_mut().push(format!("send {track_id} {episode}"));
            Ok(())
        }
    }

    impl crate::tracking::DelayedPort for Ports {
        fn add(&self, _: i64, _: f64) -> std::io::Result<()> {
            Ok(())
        }
        fn schedule(&self) {}
    }

    fn playing(duration: f32) -> Session<Fake> {
        let mut session = Session::new(Fake::default());
        session.state.duration = duration;
        session.state.current_episode = Some(kuro_data::episode::Episode {
            id: 4,
            episode_number: 3.0,
            ..kuro_data::episode::Episode::create()
        });
        session
    }

    #[test]
    fn completing_an_episode_updates_the_trackers_once() {
        let log = std::rc::Rc::new(RefCell::new(Vec::new()));
        let mut session = playing(100.0);
        session.tracking = crate::tracking::Tracking {
            trackers: vec![crate::progress::TrackerEntry {
                track_id: 9,
                logged_in: true,
                last_episode_seen: 2.0,
            }],
            ports: Some(crate::tracking::Ports {
                tracker: Box::new(Ports(log.clone())),
                delayed: Box::new(Ports(log.clone())),
            }),
            ..crate::tracking::Tracking::default()
        };
        let early = session.on_time_pos(50);
        assert!(!early.completed && early.action.is_some());
        assert!(log.borrow().is_empty());
        let done = session.on_time_pos(86);
        assert!(done.completed);
        assert!(session.state.current_episode.as_ref().unwrap().seen);
        assert!(!session.on_time_pos(87).completed);
        assert_eq!(*log.borrow(), vec!["send 9 3"]);
        assert_eq!(session.state.pos, 87.0);
    }

    #[test]
    fn a_tick_while_loading_reports_no_decisions_and_sends_nothing() {
        let mut session = playing(100.0);
        session.state.is_loading_episode = true;
        assert_eq!(session.on_time_pos(90).action, None);
        assert!(!session.state.current_episode.as_ref().unwrap().seen);
    }

    fn opening() -> Vec<TimeStamp> {
        vec![TimeStamp {
            start: 10.0,
            end: 100.0,
            name: "Opening".to_owned(),
            kind: crate::types::ChapterType::Opening,
        }]
    }

    #[test]
    fn aniskip_intervals_become_chapters_and_the_skip_button() {
        let mut session = playing(200.0);
        session.state.pos = 12.0;
        session.apply_aniskip(&opening(), Some(200));
        assert!(session.state.chapters.iter().any(|c| c.name == "Opening"));
        assert_eq!(
            session.state.current_chapter.as_ref().map(|c| c.name.as_str()),
            Some("Opening")
        );
        assert_eq!(
            session.state.skip_intro_text,
            chapters::skip_label(crate::types::ChapterType::Opening)
        );
        let before = session.state.chapters.clone();
        session.apply_aniskip(&[], Some(200));
        assert_eq!(session.state.chapters, before);
    }

    struct SkipFake;

    impl crate::aniskip::SkipApi for SkipFake {
        fn mal_id_of(&self, _: i64) -> impl std::future::Future<Output = Option<i64>> + Send {
            std::future::ready(Some(21))
        }

        fn skip_times(
            &self,
            _: i64,
            _: i32,
            _: i64,
        ) -> impl std::future::Future<Output = Option<Vec<TimeStamp>>> + Send {
            std::future::ready(Some(vec![TimeStamp {
                start: 10.0,
                end: 100.0,
                name: "Opening".to_owned(),
                kind: crate::types::ChapterType::Opening,
            }]))
        }
    }

    #[tokio::test]
    async fn run_aniskip_merges_the_intervals_only_when_wanted() {
        use crate::aniskip::TrackerRef;
        let mut session = playing(200.0);
        session.run_aniskip(&SkipFake, &[TrackerRef::AniList(9)], false).await;
        assert!(session.state.chapters.is_empty());
        session.run_aniskip(&SkipFake, &[TrackerRef::Other], true).await;
        assert!(session.state.chapters.is_empty());
        session.run_aniskip(&SkipFake, &[TrackerRef::AniList(9)], true).await;
        assert!(session.state.chapters.iter().any(|c| c.name == "Opening"));
    }

    #[test]
    fn the_skip_button_seeks_to_the_next_chapter_with_its_text() {
        let mut session = playing(200.0);
        session.state.pos = 12.0;
        session.apply_aniskip(&opening(), Some(200));
        let tap = session.on_skip_tap();
        assert!(matches!(tap, Tap::Seek(SkipSeek { position: 100, .. })));
        assert_eq!(
            *session.host.commands.borrow(),
            vec![vec!["seek", "100", "absolute+keyframes"]]
        );
        assert!(session.state.is_seeking_forwards);
        assert_eq!(session.state.double_tap_seek_amount, 88);
        assert!(session.state.seek_text.is_some());
    }

    #[test]
    fn auto_skip_seeks_on_the_tick_and_netflix_counts_down_from_the_file_load() {
        let mut session = playing(200.0);
        session.set_skip_config(SkipConfig {
            auto_skip: true,
            ..SkipConfig::default()
        });
        session.apply_aniskip(&opening(), Some(200));
        session.state.is_loading_episode = false;
        session.on_time_pos(12);
        assert_eq!(
            *session.host.commands.borrow(),
            vec![vec!["seek", "100", "absolute+keyframes"]]
        );

        let mut session = playing(200.0);
        session.set_skip_config(SkipConfig {
            netflix: true,
            default_wait: 1,
            ..SkipConfig::default()
        });
        session.set_precise_seek(true);
        session.apply_aniskip(&opening(), Some(200));
        let first = session.on_time_pos(12);
        assert!(first.toast.is_some());
        assert!(session.host.commands.borrow().is_empty());
        let second = session.on_time_pos(13);
        assert_eq!(second.toast, None);
        assert_eq!(
            *session.host.commands.borrow(),
            vec![vec!["seek", "100", "absolute"]]
        );
    }

    #[test]
    fn a_seek_outside_the_duration_only_sets_the_indicator() {
        let mut session = playing(50.0);
        session.state.pos = 12.0;
        session.apply_aniskip(&opening(), Some(50));
        session.on_skip_tap();
        assert!(session.host.commands.borrow().is_empty());
        assert!(session.state.seek_text.is_some());
    }

    #[test]
    fn file_loaded_starts_a_fresh_countdown() {
        let mut session = playing(200.0);
        session.set_skip_config(SkipConfig {
            netflix: true,
            default_wait: 3,
            ..SkipConfig::default()
        });
        session.apply_aniskip(&opening(), Some(200));
        session.on_time_pos(12);
        session.on_time_pos(13);
        assert_eq!(session.skip.waiting(), 1);
        session.on_event(MpvEvent::FileLoaded);
        assert_eq!(session.skip.waiting(), 3);
    }
}
