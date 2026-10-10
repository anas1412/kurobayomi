//! The player window (areas 45 and 46): a GL area that shows libmpv's render output, a control bar and keyboard
//! shortcuts. A window is opened through [`open_episode`]. Logic lives in `playback` (mpv) and `store` (watch
//! progress); this file is widget wiring. Main thread only.

mod gl;
mod playback;
mod store;
#[cfg(test)]
mod tests;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use gtk::prelude::*;
use gtk::glib::translate::IntoGlib;
use gtk::{gdk, glib};
use kuro_platform::linux::LinuxPaths;
use kuro_platform::paths::Paths as _;
use kuro_player::keys::Modifiers;
use kuro_player::mpv::MpvHandle;
use kuro_player::observer::MpvEvent;
use kuro_player::options::{PlayerPrefs, startup_settings};
use kuro_player::seekbar_model::{drags_enabled, duration_label, fraction, position_label};
use kuro_player::session::prepare_mpv_dir;
use kuro_player::shortcuts::{Action, action_for};

use playback::{Playback, VOLUME_MAX, seek_delta_ms, seek_target_ms, volume_after_steps};
use store::Tracker;

/// How often the window reads mpv, updates the controls and saves progress when one is due.
const TICK: Duration = Duration::from_millis(250);

/// Opens `path` in a new player window for the anime episode `episode_id`.
///
/// `start_ms` above zero starts there. Zero or less resumes from the saved progress: an unseen episode resumes at
/// its last second, a seen one restarts at zero. Progress is saved every five seconds while playing, on pause, at
/// the end of the file and on close. An episode missing from the database plays without saving.
pub fn open_episode(path: &Path, episode_id: i64, start_ms: i64) {
    let paths = match LinuxPaths::system() {
        Ok(paths) => paths,
        Err(e) => return tracing::error!(error = %e, "player: no app directories; not opened"),
    };
    let mut prefs = PlayerPrefs {
        data_dir: paths.data_dir(),
        ..PlayerPrefs::default()
    };
    if crate::data::is_installed() {
        // T-46b-043: the opt-in decoder comes from the preferences; without it the startup is the plain one
        let decoder = kuro_prefs::player::DecoderPrefs::new(crate::data::app().base.store().clone());
        prefs.remember_decoder = decoder.remember_decoder.get();
        prefs.remembered_decoder = kuro_player::options::decoder_from_stored_name(&decoder.remembered_decoder.get());
    }
    prepare_mpv_dir(&prefs);
    playback::use_c_numeric_locale();
    let mpv = match MpvHandle::create(&startup_settings(&prefs)) {
        Ok(mpv) => mpv,
        Err(e) => return tracing::error!(error = %e, "player: libmpv did not start; not opened"),
    };
    let (tracker, start) = if crate::data::is_installed() {
        let app = crate::data::app();
        let tracker = Tracker::open(app.anime_db.clone(), episode_id, app.base.incognito_mode.get());
        let start = tracker.as_ref().map_or(start_ms.max(0), |t| t.start_ms(start_ms));
        (tracker, start)
    } else {
        (None, start_ms.max(0))
    };

    let shared = Rc::new(RefCell::new(Player {
        playback: Playback::new(mpv),
        tracker,
        finished_saved: false,
    }));
    let window = build_window(path, &shared, start);
    window.present();
}

/// Everything the window reads and writes on mpv and the database. Shared by the GTK closures; a `RefCell` borrow
/// never spans a GTK call, so a re-entrant render cannot panic.
struct Player {
    playback: Playback,
    tracker: Option<Tracker>,
    /// The end-of-file save has run, so it runs once.
    finished_saved: bool,
}

type Shared = Rc<RefCell<Player>>;

impl Player {
    /// Saves the position if there is one. At the end of the file the position is the duration.
    fn save(&mut self, forced: bool) {
        let Some(tracker) = self.tracker.as_mut() else {
            return;
        };
        let duration = self.playback.duration_ms().unwrap_or(0);
        let position = if self.playback.eof_reached() {
            Some(duration)
        } else {
            self.playback.position_ms()
        };
        if let Some(position) = position {
            tracker.save(now_ms(), position, duration, forced);
        }
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Toggles pause. Pausing saves progress at once.
fn toggle_pause(shared: &Shared) {
    let mut player = shared.borrow_mut();
    match player.playback.toggle_pause() {
        Ok(true) => player.save(true),
        Ok(false) => {}
        Err(e) => tracing::warn!(error = %e, "player: pause failed"),
    }
}

fn toggle_fullscreen(window: &gtk::Window) {
    if window.is_fullscreen() {
        window.unfullscreen();
    } else {
        window.fullscreen();
    }
}

/// Runs one shortcut. `false` when the window does nothing with it, so the key goes on to GTK.
fn apply_action(shared: &Shared, window: &gtk::Window, action: Action) -> bool {
    match action {
        Action::TogglePause => toggle_pause(shared),
        Action::Seek { forward, long } => {
            if let Err(e) = shared.borrow().playback.seek_by_ms(seek_delta_ms(forward, long)) {
                tracing::warn!(error = %e, "player: seek failed");
            }
        }
        Action::VolumeStep(steps) => {
            let player = shared.borrow();
            let volume = volume_after_steps(player.playback.volume(), steps);
            if let Err(e) = player.playback.set_volume(volume) {
                tracing::warn!(error = %e, "player: volume change failed");
            }
            remember_volume(&player.playback);
        }
        Action::ToggleFullscreen => toggle_fullscreen(window),
        Action::Escape if window.is_fullscreen() => window.unfullscreen(),
        _ => return false,
    }
    true
}

fn build_window(path: &Path, shared: &Shared, start: i64) -> gtk::Window {
    let area = gtk::GLArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_auto_render(false);

    let play = gtk::Button::from_icon_name("media-playback-start-symbolic");
    let seek = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.001);
    seek.set_hexpand(true);
    seek.set_draw_value(false);
    let time = gtk::Label::new(None);
    time.add_css_class("numeric");
    let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, VOLUME_MAX, 1.0);
    volume.set_value(VOLUME_MAX);
    volume.set_width_request(120);
    volume.set_draw_value(false);
    let fullscreen = gtk::Button::from_icon_name("view-fullscreen-symbolic");

    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    bar.add_css_class("osd");
    bar.add_css_class("toolbar");
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    bar.set_margin_bottom(12);
    bar.set_valign(gtk::Align::End);
    for widget in [play.upcast_ref::<gtk::Widget>(), seek.upcast_ref(), time.upcast_ref(), volume.upcast_ref(), fullscreen.upcast_ref()] {
        bar.append(widget);
    }

    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&area));
    overlay.add_overlay(&bar);

    let title = path.file_name().map_or_else(|| "Player".to_owned(), |n| n.to_string_lossy().into_owned());
    let window = gtk::Window::builder()
        .title(title)
        .default_width(960)
        .default_height(540)
        .child(&overlay)
        .build();
    if let Some(app) = gtk::gio::Application::default().and_then(|a| a.downcast::<gtk::Application>().ok()) {
        window.set_application(Some(&app));
    }

    wire_gl(&area, shared, path, start);

    {
        let shared = Rc::clone(shared);
        play.connect_clicked(move |_| toggle_pause(&shared));
    }
    {
        let shared = Rc::clone(shared);
        seek.connect_change_value(move |_, _, value| {
            let player = shared.borrow();
            if let Some(duration) = player.playback.duration_ms() {
                if let Err(e) = player.playback.seek_to_ms(seek_target_ms(value, duration)) {
                    tracing::warn!(error = %e, "player: seek failed");
                }
            }
            glib::Propagation::Proceed
        });
    }
    {
        let shared = Rc::clone(shared);
        volume.connect_change_value(move |_, _, value| {
            let player = shared.borrow();
            if let Err(e) = player.playback.set_volume(value) {
                tracing::warn!(error = %e, "player: volume change failed");
            }
            remember_volume(&player.playback);
            glib::Propagation::Proceed
        });
    }
    {
        let weak = window.downgrade();
        fullscreen.connect_clicked(move |_| {
            if let Some(window) = weak.upgrade() {
                toggle_fullscreen(&window);
            }
        });
    }

    let keys = gtk::EventControllerKey::new();
    {
        let shared = Rc::clone(shared);
        let weak = window.downgrade();
        keys.connect_key_pressed(move |_, keyval, _, state| {
            let Some(window) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            let Some(action) = action_for(keyval.into_glib(), modifiers(state), false) else {
                return glib::Propagation::Proceed;
            };
            if apply_action(&shared, &window, action) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    window.add_controller(keys);

    {
        let shared = Rc::clone(shared);
        window.connect_close_request(move |_| {
            let mut player = shared.borrow_mut();
            player.save(true);
            player.playback.stop();
            glib::Propagation::Proceed
        });
    }

    start_ticks(&window, &play, &seek, &time, &volume, shared);
    window
}

/// Connects the GL area: the render context is made on realize, frames are drawn on render, and the context is
/// freed on unrealize while the GL context is still current.
fn wire_gl(area: &gtk::GLArea, shared: &Shared, path: &Path, start: i64) {
    let loaded = Rc::new(Cell::new(false));
    {
        let shared = Rc::clone(shared);
        let path = path.to_owned();
        area.connect_realize(move |area| realize(area, &shared, &path, start, &loaded));
    }
    {
        let shared = Rc::clone(shared);
        area.connect_unrealize(move |area| {
            area.make_current();
            shared.borrow_mut().playback.mpv_mut().destroy_render_context();
        });
    }
    {
        let shared = Rc::clone(shared);
        area.connect_render(move |area, _| {
            let scale = area.scale_factor();
            let player = shared.borrow();
            if let Err(e) = player.playback.mpv().render(gl::current_framebuffer(), area.width() * scale, area.height() * scale) {
                tracing::debug!(error = %e, "player: frame not drawn");
            }
            glib::Propagation::Stop
        });
    }
}

fn realize(area: &gtk::GLArea, shared: &Shared, path: &Path, start: i64, loaded: &Cell<bool>) {
    area.make_current();
    if let Some(error) = area.error() {
        return tracing::error!(error = %error, "player: no GL context; the video cannot be drawn");
    }
    // mpv asks for a redraw from its own thread, so the request crosses to the main loop over a channel
    let (wake_tx, wake_rx) = async_channel::unbounded::<()>();
    let weak = area.downgrade();
    glib::spawn_future_local(async move {
        while wake_rx.recv().await.is_ok() {
            match weak.upgrade() {
                Some(area) => area.queue_render(),
                None => break,
            }
        }
    });
    let mut player = shared.borrow_mut();
    if let Err(e) = player.playback.mpv_mut().create_render_context(gl::get_proc_address) {
        return tracing::error!(error = %e, "player: render context failed");
    }
    if let Err(e) = player.playback.mpv_mut().set_render_update_callback(move || {
        let _ = wake_tx.try_send(());
    }) {
        tracing::error!(error = %e, "player: render updates are not wired");
    }
    if !loaded.replace(true) {
        restore_volume(&player.playback);
        if let Err(e) = player.playback.load(path, start) {
            tracing::error!(error = %e, "player: could not load the file");
        }
    }
}

/// The preference sections of the player, or `None` before the app data is installed.
fn player_prefs() -> Option<kuro_prefs::player::PlayerPrefs> {
    crate::data::is_installed().then(|| kuro_prefs::player::PlayerPrefs::new(crate::data::app().base.store().clone()))
}

/// Session start (T-45b-053): the remembered volume is written to mpv when `pref_remember_volume` is on and the stored
/// value is not negative. Brightness has no control on the desktop (T-45a-045), so its stored value is kept as it is.
fn restore_volume(playback: &Playback) {
    let Some(prefs) = player_prefs() else { return };
    if let Some(volume) = kuro_player::remember::value_to_restore(prefs.remember_player_volume.get(), prefs.player_volume_value.get())
        && let Err(e) = playback.set_volume(f64::from(volume))
    {
        tracing::warn!(error = %e, "player: the remembered volume was not applied");
    }
}

/// A volume change: stored under `player_volume_value` when `pref_remember_volume` is on.
fn remember_volume(playback: &Playback) {
    let Some(prefs) = player_prefs() else { return };
    if let Some(value) = kuro_player::remember::value_to_store(prefs.remember_player_volume.get(), playback.volume() as f32) {
        if let Err(e) = prefs.player_volume_value.set(value) {
            tracing::warn!(error = %e, "player: the volume was not stored");
        }
    }
}

/// Reads mpv on a timer. Each tick drains mpv's events, saves progress when due, and refreshes the controls.
/// It stops when the window is gone.
fn start_ticks(window: &gtk::Window, play: &gtk::Button, seek: &gtk::Scale, time: &gtk::Label, volume: &gtk::Scale, shared: &Shared) {
    let weak = window.downgrade();
    let play = play.clone();
    let seek = seek.clone();
    let time = time.clone();
    let volume = volume.clone();
    let shared = Rc::clone(shared);
    glib::timeout_add_local(TICK, move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let snapshot = {
            let mut player = shared.borrow_mut();
            for event in player.playback.mpv_mut().drain_events() {
                if let MpvEvent::EndFile { error: Some(_), .. } = event {
                    tracing::warn!("player: playback stopped with an error");
                }
            }
            if player.playback.eof_reached() && !player.finished_saved {
                player.finished_saved = true;
                player.save(true);
            } else if !player.playback.is_paused() {
                player.save(false);
            }
            Snapshot {
                paused: player.playback.is_paused(),
                position_s: player.playback.position_ms().unwrap_or(0) as f32 / 1000.0,
                duration_s: player.playback.duration_ms().unwrap_or(0) as f32 / 1000.0,
                volume: player.playback.volume(),
            }
        };
        play.set_icon_name(if snapshot.paused { "media-playback-start-symbolic" } else { "media-playback-pause-symbolic" });
        seek.set_sensitive(drags_enabled(snapshot.duration_s));
        seek.set_value(f64::from(fraction(snapshot.position_s, snapshot.duration_s)));
        time.set_label(&format!(
            "{} / {}",
            position_label(snapshot.position_s),
            duration_label(snapshot.position_s, snapshot.duration_s, false)
        ));
        if (volume.value() - snapshot.volume).abs() > 0.5 {
            volume.set_value(snapshot.volume);
        }
        glib::ControlFlow::Continue
    });
}

struct Snapshot {
    paused: bool,
    position_s: f32,
    duration_s: f32,
    volume: f64,
}

fn modifiers(state: gdk::ModifierType) -> Modifiers {
    Modifiers {
        shift: state.contains(gdk::ModifierType::SHIFT_MASK),
        ctrl: state.contains(gdk::ModifierType::CONTROL_MASK),
        alt: state.contains(gdk::ModifierType::ALT_MASK),
        meta: state.intersects(gdk::ModifierType::META_MASK | gdk::ModifierType::SUPER_MASK),
    }
}
