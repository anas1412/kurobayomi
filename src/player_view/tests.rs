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
    playback.load(&clip, tracker.start_ms(0)).expect("loadfile");

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

/// Opens a real player window on the desktop and pumps the main loop while the clip plays. Needs a display, so it
/// runs only with `--ignored`. It writes the app's mpv config files, as opening any episode does.
#[test]
#[ignore = "opens a real player window and writes the app's mpv config files; run on purpose: cargo test -p kurobayomi window_plays -- --ignored"]
fn window_plays_the_clip_on_screen() {
    crate::testing::with_display(|| {
        for name in ["glGetIntegerv", "glGetString", "glBindFramebuffer"] {
            assert!(!super::gl::get_proc_address(name).is_null(), "{name} did not resolve once GTK had loaded GL");
        }
        let dir = TempDir::new().unwrap();
        let Some(clip) = make_clip(dir.path()) else {
            return;
        };
        super::open_episode(&clip, 0, 0);
        let end = Instant::now() + Duration::from_secs(8);
        while Instant::now() < end {
            gtk::glib::MainContext::default().iteration(false);
            std::thread::sleep(Duration::from_millis(10));
        }
    });
}
