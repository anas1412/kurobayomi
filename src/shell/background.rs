//! Background holds (T-53a-037, T-53a-008): a service that runs with no window (the torrent and HTTP servers, jobs)
//! holds the application, so closing the last window keeps the process alive. The registry counts holds per label. The
//! first hold takes the GIO hold on the application and the last release drops it, so GApplication exits once no window
//! and no hold is left. The counting is pure; [`acquire`] and [`release`] are the GTK wrappers, used on the GTK thread.

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;

use gtk::prelude::*;

/// What the GIO hold must do after a change to the registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gio {
    /// The first hold of any label: take the application hold.
    Hold,
    /// The last hold of every label is gone: drop the application hold.
    Release,
    Nothing,
}

/// Holds per label. A label with no holds is not listed.
#[derive(Default, Debug)]
pub struct HoldRegistry {
    counts: BTreeMap<String, usize>,
}

impl HoldRegistry {
    /// Adds one hold for `label`. Returns [`Gio::Hold`] when this is the first hold of all.
    pub fn acquire(&mut self, label: &str) -> Gio {
        let was_empty = self.counts.is_empty();
        *self.counts.entry(label.to_owned()).or_insert(0) += 1;
        if was_empty { Gio::Hold } else { Gio::Nothing }
    }

    /// Removes one hold for `label`. A release with no matching hold does nothing. Returns [`Gio::Release`] when no hold
    /// of any label is left.
    pub fn release(&mut self, label: &str) -> Gio {
        let Some(count) = self.counts.get_mut(label) else { return Gio::Nothing };
        *count -= 1;
        if *count == 0 {
            self.counts.remove(label);
        }
        if self.counts.is_empty() { Gio::Release } else { Gio::Nothing }
    }

    /// The labels that hold the application, in order.
    pub fn active(&self) -> Vec<String> {
        self.counts.keys().cloned().collect()
    }
}

thread_local! {
    static REGISTRY: RefCell<HoldRegistry> = RefCell::new(HoldRegistry::default());
    // the GIO hold guard while any label holds; dropping it releases the application
    static GUARD: RefCell<Option<Box<dyn Any>>> = const { RefCell::new(None) };
}

/// Holds the application for `label` (the first hold takes the GIO hold).
pub fn acquire(app: &gtk::Application, label: &str) {
    if REGISTRY.with(|r| r.borrow_mut().acquire(label)) == Gio::Hold {
        GUARD.with(|g| *g.borrow_mut() = Some(Box::new(app.hold())));
    }
}

/// Releases one hold for `label`. The last release drops the GIO hold.
pub fn release(label: &str) {
    if REGISTRY.with(|r| r.borrow_mut().release(label)) == Gio::Release {
        GUARD.with(|g| g.borrow_mut().take());
    }
}

/// The labels that hold the application now.
pub fn active() -> Vec<String> {
    REGISTRY.with(|r| r.borrow().active())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_hold_takes_the_application_and_later_ones_do_not() {
        let mut r = HoldRegistry::default();
        assert_eq!(r.acquire("torrent server"), Gio::Hold);
        assert_eq!(r.acquire("http server"), Gio::Nothing);
        assert_eq!(r.acquire("torrent server"), Gio::Nothing, "a second hold of the same label");
    }

    #[test]
    fn the_last_release_drops_the_application_hold() {
        let mut r = HoldRegistry::default();
        r.acquire("torrent server");
        r.acquire("torrent server");
        r.acquire("http server");
        assert_eq!(r.release("torrent server"), Gio::Nothing, "one torrent hold is still left");
        assert_eq!(r.release("http server"), Gio::Nothing, "a torrent hold is still left");
        assert_eq!(r.release("torrent server"), Gio::Release);
    }

    #[test]
    fn a_release_without_a_hold_does_nothing() {
        let mut r = HoldRegistry::default();
        assert_eq!(r.release("nothing held"), Gio::Nothing);
        r.acquire("a");
        assert_eq!(r.release("b"), Gio::Nothing, "another label does not release a");
        assert_eq!(r.active(), vec!["a".to_owned()]);
    }

    #[test]
    fn active_lists_the_labels_that_hold_and_only_those() {
        let mut r = HoldRegistry::default();
        assert!(r.active().is_empty());
        r.acquire("http server");
        r.acquire("torrent server");
        assert_eq!(r.active(), vec!["http server".to_owned(), "torrent server".to_owned()]);
        r.release("http server");
        assert_eq!(r.active(), vec!["torrent server".to_owned()]);
    }
}
