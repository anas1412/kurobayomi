//! The player's control bar and its behaviour. The bottom bar hides after three seconds without mouse movement or key
//! press (`autohide`, 2.5 seconds in fullscreen, where the pointer hides with it). The header hides only in fullscreen.
//! A click on the video toggles pause and a double click toggles fullscreen. The buttons follow the playing episode and file: previous and next are enabled only with a neighbour,
//! the quality button shows only for an online episode, and the speed and chapter buttons show what plays now.
//! Main thread only; the shared logic is in the parent module (`toggle_pause`, `toggle_fullscreen`, `adjacent`).

mod autohide;

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::glib;
use gtk::prelude::*;

use super::neighbours::{Step, adjacent};
use super::sheets;
use super::{Shared, toggle_fullscreen, toggle_pause};

/// How often the buttons and the auto-hide are checked.
const TICK: Duration = Duration::from_millis(250);

/// A second press within this time is a double click (GTK's default).
const DOUBLE_CLICK: Duration = Duration::from_millis(400);

/// The widgets the controls wire up and keep up to date.
pub struct Parts<'a> {
    pub page: &'a gtk::Box,
    pub area: &'a gtk::GLArea,
    pub bar: &'a gtk::Box,
    pub previous: &'a gtk::Button,
    pub next: &'a gtk::Button,
    pub quality: &'a gtk::MenuButton,
    pub speed: &'a gtk::MenuButton,
    pub chapters: &'a gtk::MenuButton,
}

/// The widgets the timer keeps up to date. Clones of the page's own widgets, so no closure holds the page.
struct Live {
    bar: gtk::Box,
    previous: gtk::Button,
    next: gtk::Button,
    quality: gtk::MenuButton,
    speed: gtk::MenuButton,
    chapters: gtk::MenuButton,
    /// The episode row the buttons last followed. `i64::MIN` before the first check.
    episode: Cell<i64>,
}

impl Live {
    /// Follows the playing episode: previous and next are enabled only with a neighbour, and the quality button shows
    /// only for an online episode.
    fn episode_changed(&self, shared: &Shared) {
        let episode = shared.borrow().episode_id;
        self.episode.set(episode);
        let (has_previous, has_next) = if crate::data::is_installed() {
            let db = &crate::data::app().anime_db;
            let has = |step| adjacent(db, episode, step).ok().flatten().is_some();
            (has(Step::Previous), has(Step::Next))
        } else {
            (false, false)
        };
        self.previous.set_sensitive(has_previous);
        self.next.set_sensitive(has_next);
        self.quality.set_visible(sheets::quality::is_online(episode));
    }

    /// A speed or chapter menu that is open keeps the bar shown.
    fn menu_open(&self) -> bool {
        [&self.speed, &self.chapters].iter().any(|button| button.popover().is_some_and(|p| p.is_visible()))
    }
}

/// Wires the bar's auto-hide, the video's clicks and the buttons' state. Call once the page is built.
pub fn wire(parts: Parts, shared: &Shared) {
    let activity = Rc::new(Cell::new(Instant::now()));
    {
        let (activity, bar) = (Rc::clone(&activity), parts.bar.clone());
        let motion = gtk::EventControllerMotion::new();
        // GTK also sends motion events when the widgets under a still pointer change (the time label and the seek bar
        // update every tick), so only a real move of the pointer counts as activity; else the bar never hides
        let last = Cell::new((f64::NAN, f64::NAN));
        motion.connect_motion(move |_, x, y| {
            let (px, py) = last.replace((x, y));
            if (x - px).abs() >= 1.0 || (y - py).abs() >= 1.0 || px.is_nan() {
                reveal(&activity, &bar);
            }
        });
        parts.page.add_controller(motion);
    }
    {
        let (activity, bar) = (Rc::clone(&activity), parts.bar.clone());
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed(move |_, _, _, _| {
            reveal(&activity, &bar);
            glib::Propagation::Proceed
        });
        parts.page.add_controller(keys);
    }
    {
        // one press waits out the double click time before it pauses, so a double click only goes fullscreen
        let click = gtk::GestureClick::new();
        let (shared, page) = (Rc::clone(shared), parts.page.downgrade());
        let pending = Rc::new(Cell::new(0u64));
        click.connect_pressed(move |_, presses, _, _| match presses {
            1 => {
                let mine = pending.get() + 1;
                pending.set(mine);
                let (shared, pending) = (Rc::clone(&shared), Rc::clone(&pending));
                glib::timeout_add_local_once(DOUBLE_CLICK, move || {
                    if pending.get() == mine {
                        toggle_pause(&shared);
                    }
                });
            }
            2 => {
                pending.set(pending.get() + 1);
                if let Some(page) = page.upgrade() {
                    toggle_fullscreen(&page);
                }
            }
            _ => {}
        });
        parts.area.add_controller(click);
    }

    let live = Live {
        bar: parts.bar.clone(),
        previous: parts.previous.clone(),
        next: parts.next.clone(),
        quality: parts.quality.clone(),
        speed: parts.speed.clone(),
        chapters: parts.chapters.clone(),
        episode: Cell::new(i64::MIN),
    };
    live.episode_changed(shared);
    let (page, shared) = (parts.page.downgrade(), Rc::clone(shared));
    glib::timeout_add_local(TICK, move || {
        let Some(page) = page.upgrade() else {
            return glib::ControlFlow::Break;
        };
        if shared.borrow().closed {
            return glib::ControlFlow::Break;
        }
        if live.episode.get() != shared.borrow().episode_id {
            live.episode_changed(&shared);
        }
        let fullscreen = super::window_of(&page).is_some_and(|w| w.is_fullscreen());
        let hide = autohide::hidden(activity.get().elapsed(), live.menu_open(), fullscreen);
        if hide {
            live.bar.set_visible(false);
        }
        // in fullscreen the pointer hides with the bar, and comes back with it
        let pointer_hidden = fullscreen && hide;
        if page.cursor().is_some() != pointer_hidden {
            page.set_cursor_from_name(pointer_hidden.then_some("none"));
        }
        sheets::speed::refresh(&live.speed, &shared);
        sheets::chapters::refresh(&live.chapters, &shared);
        glib::ControlFlow::Continue
    });
}

/// Shows the bar and restarts its countdown.
fn reveal(activity: &Cell<Instant>, bar: &gtk::Box) {
    activity.set(Instant::now());
    bar.set_visible(true);
}
