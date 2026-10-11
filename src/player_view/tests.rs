//! Headless check of the player core: real libmpv (`vo=null`, `ao=null`) plays a generated clip, and the real anime
//! database holds the progress. No display is needed. The clip comes from ffmpeg; without ffmpeg the test says so
//! and returns.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use kuro_data::anime::{Anime, AnimeRepo};
use kuro_data::episode::{Episode, EpisodeRepo};
use kuro_db::handle::Db;
use kuro_player::mpv::MpvHandle;
use kuro_player::options::{MpvSetting, PlayerPrefs, startup_settings};
use kuro_testkit::{TempDir, TestDb};

use super::playback::Playback;
use super::store::Tracker;

/// Writes a 4 second 160x120 test pattern at 10 fps. Returns `None` when ffmpeg is not installed.
fn make_clip(dir: &Path) -> Option<PathBuf> {
    let path = dir.join("clip.mp4");
    let status = std::process::Command::new("ffmpeg")
        .args(["-v", "error", "-y", "-f", "lavfi", "-i", "testsrc=duration=4:size=160x120:rate=10"])
        .args(["-c:v", "mpeg4", "-pix_fmt", "yuv420p"])
        .arg(&path)
        .status()
        .ok()?;
    status.success().then_some(path)
}

/// A libmpv core with no window and no sound output.
fn headless(data_dir: &Path) -> Playback {
    let prefs = PlayerPrefs {
        data_dir: data_dir.to_owned(),
        ..PlayerPrefs::default()
    };
    let mut settings = startup_settings(&prefs);
    settings.push(MpvSetting {
        name: "ao",
        value: "null".to_owned(),
    });
    super::playback::use_c_numeric_locale();
    let mpv = MpvHandle::create(&settings).expect("libmpv must initialise headless");
    mpv.set_property_str("vo", "null").expect("vo=null");
    Playback::new(mpv)
}

/// Polls `check` until it holds or 15 seconds pass.
fn wait_for(mut check: impl FnMut() -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if check() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    false
}

/// One anime with one episode whose url is the clip. Returns the database and the episode id.
fn library(dir: &TempDir, clip: &Path) -> (Arc<Db>, i64) {
    let db = TestDb::anime_handle(dir);
    let anime_id = AnimeRepo::new(Arc::clone(&db)).insert(&Anime::create()).expect("anime row");
    let episode = Episode {
        anime_id,
        url: clip.display().to_string(),
        name: "Episode 1".to_owned(),
        ..Episode::create()
    };
    let id = EpisodeRepo::new(Arc::clone(&db)).add_all(&[episode]).expect("episode row")[0].id;
    (db, id)
}

fn stored(db: &Arc<Db>, id: i64) -> Episode {
    EpisodeRepo::new(Arc::clone(db)).by_id(id).expect("read").expect("row")
}

#[test]
fn a_clip_plays_seeks_pauses_and_its_progress_survives_a_reopen() {
    let dir = TempDir::new().unwrap();
    let Some(clip) = make_clip(dir.path()) else {
        eprintln!("skipped: ffmpeg is not installed, so no test clip was made");
        return;
    };
    let (db, id) = library(&dir, &clip);
    let playback = headless(dir.path());

    let mut tracker = Tracker::open(Arc::clone(&db), id, false).expect("episode row");
    assert_eq!(tracker.start_ms(0), 0, "an unseen episode with no progress starts at zero");
    playback.load(clip.to_str().expect("UTF-8 path"), None, tracker.start_ms(0)).expect("loadfile");

    assert!(wait_for(|| playback.duration_ms().is_some_and(|d| d > 0)), "mpv reports the duration");
    let duration = playback.duration_ms().unwrap();
    assert!((3_500..=4_500).contains(&duration), "a 4 second clip, got {duration} ms");
    assert!(wait_for(|| playback.position_ms().is_some_and(|p| p > 300)), "playback advances");
    assert!(!playback.is_paused(), "a load starts playing");

    assert!(playback.toggle_pause().expect("pause"), "toggle reports paused");
    assert!(playback.is_paused());
    assert!(!playback.toggle_pause().expect("play"), "toggle reports playing");

    playback.seek_to_ms(2_500).expect("seek");
    assert!(wait_for(|| playback.position_ms().is_some_and(|p| p >= 2_400)), "seek moves the playhead");

    playback.set_volume(40.0).expect("volume");
    assert_eq!(playback.volume(), 40.0);

    let position = playback.position_ms().unwrap();
    tracker.save(1_000, position, duration, true);
    let saved = stored(&db, id);
    assert_eq!(saved.last_second_seen, position / 1000, "the forced save writes the playhead");
    assert_eq!(saved.total_seconds, duration / 1000);
    assert!(!saved.seen, "two thirds through is not finished");

    let reopened = Tracker::open(Arc::clone(&db), id, false).expect("reopen");
    assert_eq!(reopened.start_ms(0), saved.last_second_seen * 1000, "reopen restores the saved position");

    playback.seek_to_ms(3_800).expect("seek near the end");
    assert!(wait_for(|| playback.eof_reached()), "playback reaches the end of the clip");
    tracker.save(2_000, playback.duration_ms().unwrap(), playback.duration_ms().unwrap(), true);
    assert!(stored(&db, id).seen, "reaching the end marks the episode seen");

    let finished = Tracker::open(db, id, false).expect("reopen after finishing");
    assert_eq!(finished.start_ms(0), 0, "a seen episode restarts at zero");

    playback.stop();
}

/// A source stream's load on real libmpv, over local files: the headers land on the core before `loadfile`, and the
/// subtitle added once the file is loaded becomes a track.
#[test]
fn a_stream_load_sets_its_headers_and_adds_its_subtitle_after_the_file_loads() {
    let dir = TempDir::new().unwrap();
    let Some(clip) = make_clip(dir.path()) else {
        eprintln!("skipped: ffmpeg is not installed, so no test clip was made");
        return;
    };
    let subtitle = dir.path().join("en.srt");
    std::fs::write(&subtitle, "1\n00:00:00,000 --> 00:00:03,000\nHello\n").unwrap();
    let mut playback = headless(dir.path());
    playback.load(clip.to_str().unwrap(), Some("Origin: https://player.test,X-Note: a\\, b"), 0).expect("loadfile");
    let headers = playback.mpv().get_string("http-header-fields").unwrap_or_default();
    assert!(headers.contains("Origin: https://player.test") && headers.contains("X-Note: a"), "{headers}");

    assert!(wait_for(|| playback.mpv_mut().drain_events().contains(&kuro_player::observer::MpvEvent::FileLoaded)), "the file loads");
    let before = playback.mpv().get_i64("track-list/count").unwrap_or(0);
    playback.add_tracks(&[vec!["sub-add".into(), subtitle.display().to_string(), "auto".into(), "English".into()]]);
    // the side track is added on a worker thread
    assert!(wait_for(|| playback.mpv().get_i64("track-list/count") == Some(before + 1)), "sub-add adds the subtitle track");
    let lists = kuro_player::tracks::load_track_lists(&super::menu::MpvTracks(playback.mpv()));
    assert!(lists.subtitles.iter().any(|t| t.name == "English"), "the tracks menu lists the added subtitle: {lists:?}");
    playback.stop();
}

/// The mpv side of a Detach or Attach: video goes off before the render context is freed and comes back after the
/// new one exists. The file stays loaded (a clip with no audio would end if mpv dropped its video as failed), the
/// playhead does not jump back, and the pause state is the one from before the move.
#[test]
fn a_handoff_keeps_the_file_its_position_and_its_pause_state() {
    let dir = TempDir::new().unwrap();
    let Some(clip) = make_clip(dir.path()) else {
        eprintln!("skipped: ffmpeg is not installed, so no test clip was made");
        return;
    };
    let playback = headless(dir.path());
    playback.load(clip.to_str().unwrap(), None, 0).expect("loadfile");
    assert!(wait_for(|| playback.position_ms().is_some_and(|p| p > 300)), "playback advances");

    for paused in [false, true] {
        playback.mpv().set_property_flag("pause", paused).expect("pause state");
        let before = playback.position_ms().unwrap();
        let handoff = playback.hand_off_video();
        assert_eq!(playback.mpv().get_string("vid").as_deref(), Some("no"), "video is off during the move");
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(playback.mpv().get_flag("idle-active"), Some(false), "the file is still loaded with video off");
        playback.resume_video(&handoff);
        assert_ne!(playback.mpv().get_string("vid").as_deref(), Some("no"), "video is back");
        assert_eq!(playback.is_paused(), paused, "the pause state survives the move");
        assert!(playback.position_ms().is_some_and(|p| p + 100 >= before), "the playhead does not jump back");
    }
    playback.mpv().set_property_flag("pause", false).unwrap();
    let resumed = playback.position_ms().unwrap();
    assert!(wait_for(|| playback.position_ms().is_some_and(|p| p > resumed + 300)), "playback goes on after the move");
    playback.stop();
}

/// The GL area going away: during a move the video is handed off and playback stays open; any other unrealize (Back,
/// Alt+Left, a closed detached window, the app quitting) saves the progress and stops mpv, and stays a close.
#[test]
fn the_area_going_away_hands_off_on_a_move_and_saves_and_stops_otherwise() {
    let dir = TempDir::new().unwrap();
    let Some(clip) = make_clip(dir.path()) else {
        eprintln!("skipped: ffmpeg is not installed, so no test clip was made");
        return;
    };
    let (db, id) = library(&dir, &clip);
    let playback = headless(dir.path());
    playback.load(clip.to_str().unwrap(), None, 0).expect("loadfile");
    assert!(wait_for(|| playback.position_ms().is_some_and(|p| p > 300)), "playback advances");
    let media = super::Media { target: clip.display().to_string(), title: "clip".into(), headers: None, tracks: Vec::new() };
    let mut player = super::Player {
        playback,
        tracker: Tracker::open(Arc::clone(&db), id, false),
        media,
        finished_saved: false,
        closed: false,
        handoff: None,
    };

    assert!(!player.area_gone(true), "a move is not a close");
    assert!(!player.closed && player.handoff.is_some());
    assert_eq!(player.playback.mpv().get_flag("idle-active"), Some(false), "the file stays loaded");
    let handoff = player.handoff.take().unwrap();
    player.playback.resume_video(&handoff);
    assert!(wait_for(|| player.playback.position_ms().is_some_and(|p| p > 1_200)), "playback goes on after the move");

    let before = player.playback.position_ms().unwrap();
    assert!(player.area_gone(false), "an unrealize that is not a move closes");
    assert!(player.closed);
    assert!(wait_for(|| player.playback.mpv().get_flag("idle-active") == Some(true)), "mpv stopped");
    assert!(stored(&db, id).last_second_seen >= before / 1000, "the close saved the playhead");
    assert!(player.area_gone(true), "after a close, a move is still a close");
}

/// Every Detach or Attach writes `player_open_detached`, which the next playback reads to open the same way.
#[test]
fn a_detach_or_attach_is_remembered_for_the_next_playback() {
    crate::testing::with_display(|| {
        use gtk::prelude::*;
        let (_stack, window) = crate::testing::nav();
        let was_visible = window.is_visible();
        window.present();
        let store = kuro_prefs::store::PrefStore::in_memory();
        let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let host = crate::widgets::detachable::Detachable::new(&page, "clip", true);
        host.show(false);
        super::remember_choice(&host, kuro_prefs::player::PlayerWindowPrefs::new(store.clone()));
        let stored = || kuro_prefs::player::PlayerWindowPrefs::new(store.clone()).open_detached.get();
        assert!(!stored(), "embedded until a choice is made");
        host.toggle();
        assert!(stored(), "Detach is remembered");
        host.toggle();
        assert!(!stored(), "Attach is remembered");
        window.set_visible(was_visible);
    });
}

/// Opens the real player page in the test window, detaches it into its own window, pauses, attaches it back, plays
/// on, detaches again and closes that window, pumping the main loop. mpv's config files go to a scratch folder. Needs
/// a display (and ffmpeg for the clip); without them it says so and returns.
#[test]
fn window_plays_the_clip_on_screen() {
    crate::testing::with_display(|| {
        use gtk::prelude::*;
        for name in ["glGetIntegerv", "glGetString", "glBindFramebuffer"] {
            assert!(!super::gl::get_proc_address(name).is_null(), "{name} did not resolve once GTK had loaded GL");
        }
        let dir = TempDir::new().unwrap();
        let Some(clip) = make_clip(dir.path()) else {
            return;
        };
        let (_stack, window) = crate::testing::nav();
        let was_visible = window.is_visible();
        window.present();
        let media = super::Media { target: clip.display().to_string(), title: "clip".into(), headers: None, tracks: Vec::new() };
        let (host, shared) = super::open_in(dir.path().to_owned(), media, 0, 0).expect("the player opens");
        assert_eq!(crate::ui::page_stack().0, ["screen:player"], "the player opens inside the main window by default");
        let pump = |ms: u64| {
            let end = Instant::now() + Duration::from_millis(ms);
            while Instant::now() < end {
                gtk::glib::MainContext::default().iteration(false);
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        let position = || shared.borrow().playback.position_ms().unwrap_or(0);
        let video_on = || shared.borrow().playback.mpv().get_string("vid").is_some_and(|v| v != "no");
        let paused = || shared.borrow().playback.is_paused();

        pump(1_000);
        let before = position();
        assert!(before > 300, "the clip plays embedded, at {before} ms");
        host.toggle();
        assert!(host.is_detached());
        pump(700);
        assert!(video_on(), "video is back after the detach");
        assert!(position() > before, "playback went on across the detach");
        assert!(!shared.borrow().closed);

        shared.borrow().playback.mpv().set_property_flag("pause", true).unwrap();
        let before = position();
        host.toggle();
        assert!(!host.is_detached());
        pump(500);
        assert!(video_on(), "video is back after the attach");
        assert!(paused(), "a paused player stays paused across the attach");
        assert!(position() + 100 >= before, "the playhead did not jump back");

        shared.borrow().playback.mpv().set_property_flag("pause", false).unwrap();
        host.toggle();
        assert!(host.is_detached());
        let before = position();
        pump(500);
        assert!(video_on() && !paused() && position() > before, "playing again in its own window");

        host.close();
        pump(300);
        assert!(shared.borrow().closed, "closing the detached window closes the player");
        assert_eq!(shared.borrow().playback.mpv().get_flag("idle-active"), Some(true), "mpv stopped");
        window.set_visible(was_visible);
    });
}

/// Runs the main loop for `ms`.
fn pump_for(ms: u64) {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        gtk::glib::MainContext::default().iteration(false);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Escape, Alt+Left and the mouse back button leave the embedded page through `ui::pop`, which removes it only
/// 400 ms later. Playback must stop and save at that call, not at the removal. Needs a display and ffmpeg.
#[test]
fn popping_the_embedded_player_stops_playback_at_once() {
    crate::testing::with_display(|| {
        use gtk::prelude::*;
        let dir = TempDir::new().unwrap();
        let Some(clip) = make_clip(dir.path()) else {
            return;
        };
        let (_stack, window) = crate::testing::nav();
        let was_visible = window.is_visible();
        window.present();
        crate::ui::push(&gtk::Label::new(Some("below")));
        let media = super::Media { target: clip.display().to_string(), title: "clip".into(), headers: None, tracks: Vec::new() };
        let (_host, shared) = super::open_in(dir.path().to_owned(), media, 0, 0).expect("the player opens");
        pump_for(500);
        assert!(!shared.borrow().closed, "playing");

        crate::ui::pop();
        assert!(shared.borrow().closed, "closed by the pop itself, before the 400 ms removal");
        assert_eq!(shared.borrow().playback.mpv().get_flag("idle-active"), Some(true), "mpv stopped");
        pump_for(500);
        window.set_visible(was_visible);
    });
}

/// Closing the player frees everything: after Back the page goes away and nothing holds the player any more, so the
/// mpv handle (and its render context and timers) is destroyed. A leak would keep the weak reference alive.
/// Needs a display and ffmpeg.
#[test]
fn a_closed_player_is_freed_and_leaves_no_leak() {
    crate::testing::with_display(|| {
        use gtk::prelude::*;
        let dir = TempDir::new().unwrap();
        let Some(clip) = make_clip(dir.path()) else {
            return;
        };
        let (_stack, window) = crate::testing::nav();
        let was_visible = window.is_visible();
        window.present();
        crate::ui::push(&gtk::Label::new(Some("below")));
        let media = super::Media { target: clip.display().to_string(), title: "clip".into(), headers: None, tracks: Vec::new() };
        let (host, shared) = super::open_in(dir.path().to_owned(), media, 0, 0).expect("the player opens");
        pump_for(400);
        let weak = std::rc::Rc::downgrade(&shared);
        crate::ui::pop();
        drop(shared);
        drop(host);
        pump_for(1200);
        assert!(weak.upgrade().is_none(), "nothing keeps the closed player (and its mpv handle) alive");
        assert!(!super::is_open(), "no player counts as open any more");
        window.set_visible(was_visible);
    });
}

/// F11 and Escape act on the window that holds the page: the detached window, not the main one. Needs a display, a
/// compositor that honours fullscreen, and ffmpeg; without them it says so and returns.
#[test]
fn fullscreen_and_escape_act_on_the_window_that_holds_the_player() {
    crate::testing::with_display(|| {
        use gtk::prelude::*;
        let dir = TempDir::new().unwrap();
        let Some(clip) = make_clip(dir.path()) else {
            return;
        };
        let (_stack, window) = crate::testing::nav();
        let was_visible = window.is_visible();
        window.present();
        let media = super::Media { target: clip.display().to_string(), title: "clip".into(), headers: None, tracks: Vec::new() };
        let (host, shared) = super::open_in(dir.path().to_owned(), media, 0, 0).expect("the player opens");
        host.toggle();
        let own = gtk::Window::list_toplevels()
            .into_iter()
            .filter_map(|w| w.downcast::<gtk::Window>().ok())
            .find(|w| w.title().as_deref() == Some("clip") && w.is_visible())
            .expect("the detached window");
        let page = own.child().and_downcast::<gtk::Box>().expect("the page is the window's child");

        super::toggle_fullscreen(&page);
        pump_for(700);
        if own.is_fullscreen() {
            assert!(!window.is_fullscreen(), "the main window did not change");
            assert!(super::apply_action(&shared, &page, kuro_player::shortcuts::Action::Escape), "Escape leaves fullscreen");
            pump_for(700);
            assert!(!own.is_fullscreen() && !window.is_fullscreen(), "back to normal");
        } else {
            eprintln!("skipped: the compositor did not fullscreen the window");
        }
        host.close();
        pump_for(300);
        assert!(shared.borrow().closed);
        window.set_visible(was_visible);
    });
}
