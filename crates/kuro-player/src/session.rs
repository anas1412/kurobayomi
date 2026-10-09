//! The player session: the non-GTK half of the player window (area 45 `PlayerActivity`). It owns
//! the mpv host and the `PlayerState`, and joins the pure modules into the runtime paths: observer
//! event, bridge parse, dispatch and reset (T-45a-020); launch (T-45a-070, T-45a-011); the
//! `FILE_LOADED` steps (T-45a-013, T-45a-016); filters, fonts, volume restore, screenshots and the
//! external-player run (T-45a-061). The GTK window forwards events here and applies what comes back.

use std::ffi::OsStr;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use kuro_source_http::local_server::LocalHandler;

use crate::bridge::{self, BridgeCommand};
use crate::chapters::{self, Segment};
use crate::config::{apply_fonts_dir, apply_user_files, write_mpv_config};
use crate::enums::Decoder;
use crate::errors::PlayerMessage;
use crate::external::{self, ExternalError, FinalSave, Launch, Plan, Preset};
use crate::ipc;
use crate::load::{FileLoadedSteps, VideoSpec, loadfile_command, run_file_loaded};
use crate::local_server::{LocalServer, prepare_url};
use crate::mpv::{MpvError, MpvHandle, PropertySink, PropertySource};
use crate::observer::{MpvEvent, Route, observe_all, route};
use crate::options::{
    FILTER_NAMES, PlayerPrefs, VideoChain, filter_write, get_decoder, startup_settings,
    update_decoder,
};
use crate::screenshot::screenshot_result;
use crate::state::PlayerState;
use crate::system_av::volume_max_write;
use crate::tracks::{
    AddKind, TRACK_NONE, TrackValue, add_file_args, external_track_commands, select_sub,
    track_id_from_value,
};
use crate::types::{TimeStamp, Track, Video};
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
    /// A `user-data/aniyomi/*` request. The property was already reset to empty.
    Bridge(BridgeCommand),
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

pub struct Session<H> {
    pub host: H,
    pub state: PlayerState,
    loaded: Loaded,
    /// The inputs of the `vf` chain, so a filter change rewrites the whole chain (E-26).
    video: VideoChain,
    hwdec_available: Vec<String>,
    statistics_page: i32,
}

impl<H: PropertySink + PropertySource + CommandSink> Session<H> {
    pub fn new(host: H) -> Self {
        Self {
            host,
            state: PlayerState::default(),
            loaded: Loaded::default(),
            video: VideoChain::default(),
            hwdec_available: Vec::new(),
            statistics_page: 0,
        }
    }

    /// `pref_player_statistics_page`, the page the statistics overlay opens on.
    pub fn statistics_page(&self) -> i32 {
        self.statistics_page
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
        update_decoder(&self.host, decoder, &self.hwdec_available)
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

    /// Handles one drained mpv event. A bridge property is parsed and, when it carries a value,
    /// reset to empty so a repeated identical request fires again.
    pub fn on_event(&mut self, event: MpvEvent) -> Dispatch {
        if !accepts_event(&self.state) {
            return Dispatch::Dropped;
        }
        if route(&event) == Route::Bridge {
            return self.on_bridge(&event);
        }
        match &event {
            MpvEvent::FileLoaded => self.on_file_loaded(),
            MpvEvent::Seek => {
                self.state.on_seek();
                Dispatch::Handled
            }
            MpvEvent::PropertyFlag(name, value) if self.state.on_flag(name, *value) => {
                Dispatch::Handled
            }
            MpvEvent::PropertyDouble(name, value) if self.state.on_double(name, *value) => {
                Dispatch::Handled
            }
            _ => Dispatch::Forward(event),
        }
    }

    fn on_bridge(&mut self, event: &MpvEvent) -> Dispatch {
        let MpvEvent::PropertyStr(name, value) = event else {
            return Dispatch::Handled;
        };
        let command = bridge::parse(name, value);
        if bridge::reset_needed(value) {
            let (property, empty) = bridge::reset_write(name);
            if let Err(e) = self.host.set_str(&property, empty) {
                tracing::error!("failed to reset {property}: {e}");
            }
        }
        command.map_or(Dispatch::Handled, Dispatch::Bridge)
    }

    fn on_file_loaded(&mut self) -> Dispatch {
        let mut steps = LoadedSteps {
            host: &self.host,
            loaded: &self.loaded,
            pending: Vec::new(),
            finish_tracks_now: false,
        };
        let state = &self.state;
        run_file_loaded(&mut steps, || state.is_exiting);
        Dispatch::FileLoaded {
            pending: steps.pending,
            finish_tracks_now: steps.finish_tracks_now,
        }
    }

    /// The chapter list with the video's timestamps merged in, or `None` to leave it as it is.
    pub fn merged_chapters(
        &self,
        current: &[Segment],
        duration: Option<i32>,
    ) -> Option<Vec<Segment>> {
        chapters::with_timestamps(current, &self.loaded.timestamps, duration)
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

    /// `onResume`: clears the exit gate and restores the remembered volume once after a hidden window.
    pub fn resume(&mut self, remembered_volume: Option<i64>) -> Result<(), MpvError> {
        on_resume(&mut self.state);
        match take_volume_restore(&mut self.state, remembered_volume) {
            Some(volume) => self.host.set_str("volume", &volume.to_string()),
            None => Ok(()),
        }
    }

    /// The toast for a finished screenshot, if any.
    pub fn screenshot_done(&self, result: Result<(), &str>) -> Option<PlayerMessage> {
        screenshot_result(result)
    }
}

impl Session<MpvHandle> {
    /// Writes the mpv config files, creates the mpv core from the startup table, observes the
    /// property set and points the font lookup at `fonts_dir`. The video filters are in the `vf`
    /// chain of the startup table.
    pub fn start(prefs: &PlayerPrefs, fonts_dir: &Path) -> Result<Self, MpvError> {
        prepare_mpv_dir(prefs);
        let mut host = MpvHandle::create(&startup_settings(prefs))?;
        observe_all(&mut host)?;
        apply_fonts_dir(&host, fonts_dir)?;
        let mut session = Self::new(host);
        session.video = VideoChain::from(prefs);
        session.hwdec_available = prefs.hwdec_available.clone();
        session.statistics_page = prefs.advanced.statistics_page;
        Ok(session)
    }
}

/// Step before mpv is created (T-45b-034): the user script folders follow `mpv_scripts`, then
/// `mpv.conf` and `input.conf` are rewritten from the preferences. A failure is logged and does
/// not stop playback.
pub fn prepare_mpv_dir(prefs: &PlayerPrefs) {
    let dir = prefs.data_dir.join("mpv");
    let advanced = &prefs.advanced;
    if let Err(e) = apply_user_files(&dir, &advanced.source_dir, advanced.user_files) {
        tracing::error!("failed to apply the mpv user files: {e}");
    }
    if let Err(e) = write_mpv_config(&dir, &advanced.mpv_conf, &advanced.mpv_input) {
        tracing::error!("failed to write the mpv config files: {e}");
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

    fn bridge_event(name: &str, value: &str) -> MpvEvent {
        MpvEvent::PropertyStr(format!("{}{name}", bridge::PREFIX), value.to_owned())
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
    fn bridge_event_is_parsed_dispatched_and_reset_to_empty() {
        let mut session = Session::new(Fake::default());
        let out = session.on_event(bridge_event("show_text", "\"Hi\""));
        assert_eq!(
            out,
            Dispatch::Bridge(BridgeCommand::ShowText("Hi".to_owned()))
        );
        assert_eq!(
            *session.host.writes.borrow(),
            vec![("user-data/aniyomi/show_text".to_owned(), String::new())]
        );
        // The reset itself comes back as an empty value: nothing to run, nothing to reset.
        assert_eq!(
            session.on_event(bridge_event("show_text", "")),
            Dispatch::Handled
        );
        assert_eq!(session.host.writes.borrow().len(), 1);
    }

    #[test]
    fn unknown_bridge_value_is_reset_but_runs_nothing() {
        let mut session = Session::new(Fake::default());
        assert_eq!(
            session.on_event(bridge_event("toggle_ui", "sideways")),
            Dispatch::Handled
        );
        assert_eq!(session.host.writes.borrow().len(), 1);
    }

    #[test]
    fn exiting_window_drops_events() {
        let mut session = Session::new(Fake::default());
        session.state.is_exiting = true;
        assert_eq!(
            session.on_event(bridge_event("show_text", "x")),
            Dispatch::Dropped
        );
        assert!(session.host.writes.borrow().is_empty());
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
        session.hwdec_available = vec!["vaapi-copy".to_owned()];
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
    fn prepare_writes_both_mpv_files_and_gates_user_scripts() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("user");
        std::fs::create_dir_all(source.join("scripts")).unwrap();
        std::fs::write(source.join("scripts/a.lua"), b"lua").unwrap();
        let mut prefs = PlayerPrefs {
            data_dir: dir.path().join("data"),
            ..PlayerPrefs::default()
        };
        prefs.advanced = crate::config::AdvancedPrefs {
            user_files: true,
            source_dir: source,
            mpv_conf: "volume=40\n".to_owned(),
            mpv_input: "SPACE cycle pause\n".to_owned(),
            statistics_page: 2,
        };
        prepare_mpv_dir(&prefs);
        let mpv = dir.path().join("data/mpv");
        assert_eq!(
            std::fs::read_to_string(mpv.join("mpv.conf")).unwrap(),
            "volume=40\n"
        );
        assert_eq!(
            std::fs::read_to_string(mpv.join("input.conf")).unwrap(),
            "SPACE cycle pause\n"
        );
        assert!(mpv.join("scripts/a.lua").is_file());
        prefs.advanced.user_files = false;
        prepare_mpv_dir(&prefs);
        assert!(!mpv.join("scripts").exists());
    }

    #[test]
    fn resume_restores_the_volume_once_and_screenshot_toasts() {
        let mut session = Session::new(Fake::default());
        session.state.is_exiting = true;
        session.resume(Some(70)).unwrap();
        session.resume(Some(70)).unwrap();
        assert_eq!(
            *session.host.writes.borrow(),
            vec![("volume".to_owned(), "70".to_owned())]
        );
        assert_eq!(
            session.screenshot_done(Ok(())),
            Some(PlayerMessage::PictureSaved)
        );
        assert_eq!(session.screenshot_done(Err("full")), None);
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
}
