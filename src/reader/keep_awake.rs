//! Keep the screen on while the reader is open (T-043a-051) or a video plays. The inhibit is taken with the application and
//! released when the reader or the player closes, the video pauses, or the preference goes off.

use std::cell::RefCell;

use kuro_platform::idle::{IdleGuard, IdleInhibitor};

/// The inhibit is wanted while the preference is on and the reader is open.
pub fn should_inhibit(pref_on: bool, reader_open: bool) -> bool {
    pref_on && reader_open
}

/// Holds the platform idle guard (the same `IdleInhibitor` the player uses). [`KeepAwake::sync`] takes or releases it
/// to match the wanted state; dropping `KeepAwake` releases it too.
#[derive(Default)]
pub struct KeepAwake {
    guard: RefCell<Option<IdleGuard>>,
}

impl KeepAwake {
    pub fn sync(&self, _window: &gtk::Window, want: bool) {
        self.sync_for(want, "Reading manga");
    }

    /// Takes or releases the inhibit to match `want`; `reason` is what the desktop shows for it.
    pub fn sync_for(&self, want: bool, reason: &str) {
        self.sync_with(&kuro_platform::linux::ScreenSaver, want, reason);
    }

    fn sync_with(&self, idle: &dyn IdleInhibitor, want: bool, reason: &str) {
        let mut guard = self.guard.borrow_mut();
        match (want, guard.is_some()) {
            (true, false) => match idle.inhibit(reason) {
                Ok(taken) => *guard = Some(taken),
                Err(error) => tracing::warn!(%error, "the screen may blank while playing"),
            },
            (false, true) => *guard = None,
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    struct Counting(Arc<AtomicUsize>);

    impl IdleInhibitor for Counting {
        fn inhibit(&self, _reason: &str) -> io::Result<IdleGuard> {
            self.0.fetch_add(1, Ordering::SeqCst);
            let live = Arc::clone(&self.0);
            Ok(IdleGuard::new(move || {
                live.fetch_sub(1, Ordering::SeqCst);
            }))
        }
    }

    #[test]
    fn the_guard_is_taken_once_and_released_with_the_reader() {
        let live = Arc::new(AtomicUsize::new(0));
        let idle = Counting(Arc::clone(&live));
        let awake = KeepAwake::default();
        awake.sync_with(&idle, true, "test");
        awake.sync_with(&idle, true, "test");
        assert_eq!(live.load(Ordering::SeqCst), 1);
        awake.sync_with(&idle, false, "test");
        assert_eq!(live.load(Ordering::SeqCst), 0);
        awake.sync_with(&idle, true, "test");
        drop(awake);
        assert_eq!(live.load(Ordering::SeqCst), 0, "dropping releases it");
    }

    #[test]
    fn the_inhibit_follows_the_preference_and_the_reader_being_open() {
        assert!(should_inhibit(true, true));
        assert!(!should_inhibit(false, true));
        assert!(!should_inhibit(true, false));
    }
}
