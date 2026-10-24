//! Keeps the screen awake while media plays (T-req-platform-007). The player holds an [`IdleGuard`] and drops it when
//! playback stops, so the release happens on every path out of playback.

use std::io;

/// Asks the desktop not to blank the screen or start its screen saver.
pub trait IdleInhibitor: Send + Sync {
    /// Keeps the screen awake until the returned guard is dropped. `reason` is shown by some desktops.
    fn inhibit(&self, reason: &str) -> io::Result<IdleGuard>;
}

/// The inhibitor of this platform, or `None` where it has none (the screen may then blank during playback).
#[allow(unreachable_code)]
pub fn system() -> Option<&'static dyn IdleInhibitor> {
    #[cfg(all(target_os = "linux", feature = "platform-linux"))]
    return Some(&crate::linux::ScreenSaver);
    None
}

/// An active inhibit. Dropping it runs the release once.
pub struct IdleGuard {
    release: Option<Box<dyn FnOnce() + Send>>,
}

impl IdleGuard {
    /// A guard that runs `release` when it is dropped.
    pub fn new(release: impl FnOnce() + Send + 'static) -> Self {
        Self { release: Some(Box::new(release)) }
    }
}

impl Drop for IdleGuard {
    fn drop(&mut self) {
        if let Some(release) = self.release.take() {
            release();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn dropping_the_guard_releases_exactly_once() {
        let released = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&released);
        drop(IdleGuard::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        }));
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_guard_taken_out_of_a_scope_holds_until_it_is_dropped() {
        let released = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&released);
        let guard = IdleGuard::new(move || {
            counter.fetch_add(1, Ordering::SeqCst);
        });
        assert_eq!(released.load(Ordering::SeqCst), 0);
        drop(guard);
        assert_eq!(released.load(Ordering::SeqCst), 1);
    }
}
