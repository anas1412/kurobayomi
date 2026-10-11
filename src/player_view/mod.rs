//! The player page (areas 45 and 46): a GL area that shows libmpv's render output, a control bar and keyboard
//! shortcuts. It opens inside the main window, or detached in its own window when that was the last choice, and the
//! header's Detach / Attach button moves it while it plays (docs/design/player-reader.md). It is opened through
//! [`open_episode`] for a file, or [`open_source_episode`] for an episode of an online source. Logic lives in `playback` (mpv), `source` (the source's video) and `store` (watch progress); this
//! file is widget wiring. Main thread only.

mod gl;
mod menu;
mod playback;
mod source;
mod store;
#[cfg(test)]
mod tests;

pub use source::open_source_episode;

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::{Rc, Weak};
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

use playback::{Handoff, Playback, VOLUME_MAX, seek_delta_ms, seek_target_ms, volume_after_steps};
use store::Tracker;

use crate::widgets::detachable::Detachable;

/// How often the page reads mpv, updates the controls and saves progress when one is due.
const TICK: Duration = Duration::from_millis(250);

/// What the player plays: a file or a source's stream with its request headers and side tracks.
struct Media {
    /// The path or URL `loadfile` gets.
    target: String,
    title: String,
    /// The `http-header-fields` value; `None` for a file.
    headers: Option<String>,
    /// `audio-add` and `sub-add` commands, sent once mpv has loaded the stream.
    tracks: Vec<Vec<String>>,
}

/// Opens `path` in the player for the anime episode `episode_id`.
///
/// `start_ms` above zero starts there. Zero or less resumes from the saved progress: an unseen episode resumes at
/// its last second, a seen one restarts at zero. Progress is saved every five seconds while playing, on pause, at
/// the end of the file and on close. An episode missing from the database plays without saving.
pub fn open_episode(path: &Path, episode_id: i64, start_ms: i64) {
    let Some(target) = path.to_str() else {
        return tracing::error!(path = %path.display(), "player: the path is not UTF-8; not opened");
    };
    let title = path.file_name().map_or_else(|| "Player".to_owned(), |n| n.to_string_lossy().into_owned());
    open_media(Media { target: target.to_owned(), title, headers: None, tracks: Vec::new() }, episode_id, start_ms);
}

thread_local! {
    /// The player that was opened last, held weakly: it is open while it lives and has not been closed.
    static CURRENT: RefCell<std::rc::Weak<RefCell<Player>>> = RefCell::new(std::rc::Weak::new());
}

/// A player page is open (embedded or detached). A second click on an episode then opens nothing more.
pub fn is_open() -> bool {
    CURRENT.with(|c| c.borrow().upgrade().is_some_and(|p| p.try_borrow().map_or(true, |p| !p.closed)))
}

fn open_media(media: Media, episode_id: i64, start_ms: i64) {
    if is_open() {
        return;
    }
    match LinuxPaths::system() {
        Ok(paths) => {
            open_in(paths.data_dir(), media, episode_id, start_ms);
        }
        Err(e) => tracing::error!(error = %e, "player: no app directories; not opened"),
    }
}

/// Opens the player with mpv's config files under `data_dir`. Returns the host that places the page and the state
/// the page shares, or `None` when libmpv did not start.
fn open_in(data_dir: std::path::PathBuf, media: Media, episode_id: i64, start_ms: i64) -> Option<(Rc<Detachable>, Shared)> {
    let mut prefs = PlayerPrefs {
        data_dir,
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
        Err(e) => {
            tracing::error!(error = %e, "player: libmpv did not start; not opened");
            return None;
        }
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
        closed: false,
        handoff: None,
        media,
    }));
    CURRENT.with(|c| *c.borrow_mut() = Rc::downgrade(&shared));
    // the page is held here until the host places it; the host only keeps it weakly
    let (_page, host) = build_page(&shared, start);
    let window_prefs = window_prefs();
    host.show(window_prefs.as_ref().is_some_and(|p| p.open_detached.get()));
    if let Some(window_prefs) = window_prefs {
        remember_choice(&host, window_prefs);
    }
    Some((host, shared))
}

/// Each Detach or Attach is stored, so the next playback opens the same way.
fn remember_choice(host: &Detachable, prefs: kuro_prefs::player::PlayerWindowPrefs) {
    host.connect_moved(move |detached| {
        if let Err(e) = prefs.open_detached.set(detached) {
            tracing::warn!(error = %e, "player: the detached choice was not stored");
        }
    });
}

/// The `[player.window]` preferences, or `None` before the app data is installed.
fn window_prefs() -> Option<kuro_prefs::player::PlayerWindowPrefs> {
    crate::data::is_installed().then(|| kuro_prefs::player::PlayerWindowPrefs::new(crate::data::app().base.store().clone()))
}

/// Everything the page reads and writes on mpv and the database. Shared by the GTK closures; a `RefCell` borrow
/// never spans a GTK call, so a re-entrant render cannot panic.
struct Player {
    playback: Playback,
    tracker: Option<Tracker>,
    media: Media,
    /// The end-of-file save has run, so it runs once.
    finished_saved: bool,
    /// Playback was stopped and saved for good (Back, or the page or its window went away).
    closed: bool,
    /// Set while the page moves between windows: the GL area is between its old and new render context.
    handoff: Option<Handoff>,
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

    /// Closing: saves the progress and stops playback, once.
    fn finish(&mut self) {
        if !self.closed {
            self.closed = true;
            self.save(true);
            self.playback.stop();
        }
    }

    /// The GL area is going away. While the host moves the page (and playback is still open) the video is handed
    /// off to the next area; any other unrealize closes. Returns `true` for a close.
    fn area_gone(&mut self, moving: bool) -> bool {
        if moving && !self.closed {
            self.handoff = Some(self.playback.hand_off_video());
            false
        } else {
            self.finish();
            true
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

/// The window that holds the page now: the main window while embedded, its own while detached.
fn window_of(page: &impl IsA<gtk::Widget>) -> Option<gtk::Window> {
    page.as_ref().root().and_downcast::<gtk::Window>()
}

fn toggle_fullscreen(page: &gtk::Box) {
    let Some(window) = window_of(page) else { return };
    if window.is_fullscreen() {
        window.unfullscreen();
    } else {
        window.fullscreen();
    }
}

/// Runs one shortcut. `false` when the page does nothing with it, so the key goes on to GTK.
fn apply_action(shared: &Shared, page: &gtk::Box, action: Action) -> bool {
    let fullscreen = window_of(page).is_some_and(|w| w.is_fullscreen());
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
        Action::ToggleFullscreen => toggle_fullscreen(page),
        Action::Escape if fullscreen => toggle_fullscreen(page),
        _ => return false,
    }
    true
}

/// The player page: a slim header (Back, title, Detach, the tracks menu) over the video and its control bar. The
/// page is named `screen:player`, so the main window hides its sidebar while it shows. Returns the host that places
/// it in the main window or in its own window.
fn build_page(shared: &Shared, start: i64) -> (gtk::Box, Rc<Detachable>) {
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
    // shown from the first frame until mpv knows the duration: the stream, its audio and its subtitles take a moment
    let loading = gtk::Spinner::builder().spinning(true).halign(gtk::Align::Center).valign(gtk::Align::Center).width_request(48).height_request(48).can_target(false).build();
    overlay.add_overlay(&loading);
    overlay.set_vexpand(true);

    let title = shared.borrow().media.title.clone();
    let header = gtk::HeaderBar::new();
    header.set_decoration_layout(Some(&crate::ui::decoration(false)));
    // width_chars keeps the ellipsis from swallowing the whole title in a crowded header
    let title_label = gtk::Label::builder().label(&title).css_classes(["title"]).ellipsize(gtk::pango::EllipsizeMode::End).width_chars(24).max_width_chars(100).build();
    header.set_title_widget(Some(&title_label));
    let back = crate::ui::icon_button("go-previous-symbolic", "Back");
    header.pack_start(&back);
    let menu = menu::tracks_menu(shared);
    header.pack_end(&menu);

    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.set_widget_name("screen:player");
    page.set_focusable(true);
    page.append(&header);
    page.append(&overlay);

    let host = Detachable::new(&page, &title, true);
    header.pack_end(host.button());
    crate::ui::no_sticky_focus(&header);

    wire_gl(&area, shared, start, Rc::downgrade(&host));
    {
        // Escape, Alt+Left and the mouse back button pop the page without this file knowing: stop at once, not when
        // the stack removes the page 400 ms later
        let shared = Rc::clone(shared);
        host.connect_left(move || shared.borrow_mut().finish());
    }

    {
        // Back stops and saves at once, then leaves the page (or closes its window)
        let (shared, host, page) = (Rc::clone(shared), Rc::clone(&host), page.downgrade());
        back.connect_clicked(move |_| {
            shared.borrow_mut().finish();
            if let Some(window) = page.upgrade().and_then(|p| window_of(&p)).filter(|w| w.is_fullscreen()) {
                window.unfullscreen();
            }
            host.close();
        });
    }
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
        let page = page.downgrade();
        fullscreen.connect_clicked(move |_| {
            if let Some(page) = page.upgrade() {
                toggle_fullscreen(&page);
            }
        });
    }

    // capture phase: the shortcuts work wherever the focus is in the page, before a focused button sees the key
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let (shared, page, menu) = (Rc::clone(shared), page.downgrade(), menu.downgrade());
        keys.connect_key_pressed(move |_, keyval, _, state| {
            let Some(page) = page.upgrade() else {
                return glib::Propagation::Proceed;
            };
            // the open tracks menu keeps its own keys
            if menu.upgrade().is_some_and(|m| m.is_active()) {
                return glib::Propagation::Proceed;
            }
            let Some(action) = action_for(keyval.into_glib(), modifiers(state), false) else {
                return glib::Propagation::Proceed;
            };
            if apply_action(&shared, &page, action) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
    }
    page.add_controller(keys);
    page.connect_map(|page| {
        page.grab_focus();
    });

    start_ticks(&page, &header, &play, &seek, &time, &volume, &loading, shared);
    (page, host)
}

/// Connects the GL area: the render context is made on realize, frames are drawn on render, and the context is
/// freed on unrealize while the GL context is still current. An unrealize while `host` moves the page is a handoff:
/// video is switched off before the context goes and comes back once the next realize made a new one. Any other
/// unrealize is a close (the page was popped, its window closed, the app quit): progress is saved and playback stops.
fn wire_gl(area: &gtk::GLArea, shared: &Shared, start: i64, host: Weak<Detachable>) {
    let loaded = Rc::new(Cell::new(false));
    {
        let shared = Rc::clone(shared);
        area.connect_realize(move |area| realize(area, &shared, start, &loaded));
    }
    {
        let shared = Rc::clone(shared);
        area.connect_unrealize(move |area| {
            let moving = host.upgrade().is_some_and(|h| h.is_moving());
            let closed = {
                let mut player = shared.borrow_mut();
                let closed = player.area_gone(moving);
                area.make_current();
                player.playback.mpv_mut().destroy_render_context();
                closed
            };
            // however the page left (Back, Escape, Alt+Left, the mouse back button, a closed window), the window
            // that held it leaves fullscreen
            if closed && let Some(window) = window_of(area).filter(|w| w.is_fullscreen()) {
                window.unfullscreen();
            }
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

fn realize(area: &gtk::GLArea, shared: &Shared, start: i64, loaded: &Cell<bool>) {
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
        if let Err(e) = player.playback.load(&player.media.target, player.media.headers.as_deref(), start) {
            tracing::error!(error = %e, "player: could not load the file");
        }
    } else if let Some(handoff) = player.handoff.take() {
        player.playback.resume_video(&handoff);
    }
}

/// The preference sections of the player, or `None` before the app data is installed.
fn player_prefs() -> Option<kuro_prefs::player::PlayerPrefs> {
    crate::data::is_installed().then(|| kuro_prefs::player::PlayerPrefs::new(crate::data::app().base.store().clone()))
}

/// Session start (T-45b-053): the remembered volume is written to mpv when `pref_remember_volume` is on and the stored
/// value is not negative.
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

/// Reads mpv on a timer. Each tick drains mpv's events, saves progress when due, and refreshes the controls; the
/// header hides while the window holding the page is fullscreen. It stops once the page is gone or closed.
fn start_ticks(page: &gtk::Box, header: &gtk::HeaderBar, play: &gtk::Button, seek: &gtk::Scale, time: &gtk::Label, volume: &gtk::Scale, loading: &gtk::Spinner, shared: &Shared) {
    let loading = loading.clone();
    let opened = std::time::Instant::now();
    let page = page.downgrade();
    let header = header.clone();
    let play = play.clone();
    let seek = seek.clone();
    let time = time.clone();
    let volume = volume.clone();
    let shared = Rc::clone(shared);
    glib::timeout_add_local(TICK, move || {
        let Some(page) = page.upgrade().filter(|_| !shared.borrow().closed) else {
            return glib::ControlFlow::Break;
        };
        let snapshot = {
            let mut player = shared.borrow_mut();
            for event in player.playback.mpv_mut().drain_events() {
                match event {
                    MpvEvent::EndFile { error: Some(_), .. } => tracing::warn!("player: playback stopped with an error"),
                    MpvEvent::FileLoaded => {
                        let tracks = std::mem::take(&mut player.media.tracks);
                        player.playback.add_tracks(&tracks);
                    }
                    _ => {}
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
        header.set_visible(!window_of(&page).is_some_and(|w| w.is_fullscreen()));
        play.set_icon_name(if snapshot.paused { "media-playback-start-symbolic" } else { "media-playback-pause-symbolic" });
        // loading until mpv knows the duration; an idle mpv after the first seconds means the load failed, so stop
        let idle = shared.borrow().playback.mpv().get_flag("idle-active") == Some(true);
        let waiting = snapshot.duration_s <= 0.0 && !(idle && opened.elapsed() > std::time::Duration::from_secs(3));
        loading.set_visible(waiting);
        loading.set_spinning(waiting);
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
