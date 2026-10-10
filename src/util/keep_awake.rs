//! Keeps the screen awake while video plays (Kotlin `powerManager` wake lock, T-52a-038).
//!
//! The player reports play and pause; [`KeepAwake`] holds one inhibit while video plays and gives it back when it pauses,
//! stops or closes. The host is a trait so the rules run against a fake; [`GtkInhibit`] is the real one.

use std::cell::Cell;

use gtk::prelude::*;

/// The reason the desktop may show for the inhibit.
pub const REASON: &str = "Playing video";

/// Something that can hold and release an idle inhibit. A cookie of 0 means the desktop refused.
pub trait InhibitHost {
    fn inhibit(&self, reason: &str) -> u32;
    fn uninhibit(&self, cookie: u32);
}

/// `gtk::Application::inhibit` with the `IDLE` flag, anchored to the player window.
pub struct GtkInhibit {
    app: gtk::Application,
    window: gtk::Window,
}

impl GtkInhibit {
    pub fn new(app: &gtk::Application, window: &impl IsA<gtk::Window>) -> Self {
        Self { app: app.clone(), window: window.clone().upcast() }
    }
}

impl InhibitHost for GtkInhibit {
    fn inhibit(&self, reason: &str) -> u32 {
        self.app.inhibit(Some(&self.window), gtk::ApplicationInhibitFlags::IDLE, Some(reason))
    }

    fn uninhibit(&self, cookie: u32) {
        self.app.uninhibit(cookie);
    }
}

/// The inhibit of one player. Dropping it (the player closing) releases a held cookie.
pub struct KeepAwake<H: InhibitHost> {
    host: H,
    cookie: Cell<Option<u32>>,
}

impl<H: InhibitHost> KeepAwake<H> {
    pub fn new(host: H) -> Self {
        Self { host, cookie: Cell::new(None) }
    }

    /// Playback started (`true`) or paused or stopped (`false`). Repeating a state does nothing.
    pub fn set_playing(&self, playing: bool) {
        match (playing, self.cookie.get()) {
            (true, None) => {
                let cookie = self.host.inhibit(REASON);
                // a refusal is not held, so a later play asks again
                self.cookie.set((cookie != 0).then_some(cookie));
            }
            (false, Some(cookie)) => {
                self.host.uninhibit(cookie);
                self.cookie.set(None);
            }
            _ => {}
        }
    }

    pub fn is_held(&self) -> bool {
        self.cookie.get().is_some()
    }
}

impl<H: InhibitHost> Drop for KeepAwake<H> {
    fn drop(&mut self) {
        self.set_playing(false);
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::rc::Rc;

    use super::*;

    #[derive(Clone, Default)]
    struct Fake {
        log: Rc<RefCell<Vec<String>>>,
        refuse: bool,
    }

    impl InhibitHost for Fake {
        fn inhibit(&self, reason: &str) -> u32 {
            self.log.borrow_mut().push(format!("inhibit {reason}"));
            if self.refuse { 0 } else { 7 }
        }

        fn uninhibit(&self, cookie: u32) {
            self.log.borrow_mut().push(format!("uninhibit {cookie}"));
        }
    }

    #[test]
    fn playing_inhibits_with_the_reason_and_pausing_releases() {
        let fake = Fake::default();
        let awake = KeepAwake::new(fake.clone());
        awake.set_playing(true);
        assert!(awake.is_held());
        awake.set_playing(false);
        assert!(!awake.is_held());
        assert_eq!(*fake.log.borrow(), ["inhibit Playing video", "uninhibit 7"]);
    }

    #[test]
    fn repeated_states_do_not_stack_inhibits() {
        let fake = Fake::default();
        let awake = KeepAwake::new(fake.clone());
        awake.set_playing(true);
        awake.set_playing(true);
        awake.set_playing(false);
        awake.set_playing(false);
        assert_eq!(fake.log.borrow().len(), 2);
    }

    #[test]
    fn closing_the_player_releases_a_held_cookie() {
        let fake = Fake::default();
        let awake = KeepAwake::new(fake.clone());
        awake.set_playing(true);
        drop(awake);
        assert_eq!(*fake.log.borrow(), ["inhibit Playing video", "uninhibit 7"]);
    }

    #[test]
    fn a_refused_inhibit_is_not_held_and_the_next_play_asks_again() {
        let fake = Fake { refuse: true, ..Fake::default() };
        let awake = KeepAwake::new(fake.clone());
        awake.set_playing(true);
        assert!(!awake.is_held());
        awake.set_playing(true);
        assert_eq!(fake.log.borrow().len(), 2);
        awake.set_playing(false);
        assert_eq!(fake.log.borrow().len(), 2, "nothing to release");
    }
}
