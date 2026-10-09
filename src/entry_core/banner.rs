//! Window-scoped banners and toasts, driven by a clock the caller passes in (no timers here).
//! Replaces Compose `SnackbarHostState` and the Android `Toast`. Plan: `p2-ui-parity.md` 5, `p2-behavior.md` 9.1-9.2.

use std::collections::VecDeque;

/// Short banner: 4000 ms. A toast uses the same Short length (`p2-ui-parity.md` 5, T-38b-024), not the 2500 ms of the
/// `p2-behavior` 9.2 proposal. Indefinite: until dismissed or its action runs.
pub const SHORT_MS: u64 = 4000;
pub const TOAST_MS: u64 = SHORT_MS;
/// At most this many banners wait behind the visible one. The oldest waiting banner is dropped first.
pub const MAX_QUEUE: usize = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Length {
    Short,
    Toast,
    Indefinite,
    /// An exact length: the anime snack layer shows its toasts for 2 s (`message::TOAST_DURATION`).
    Millis(u64),
}

impl Length {
    fn millis(self) -> Option<u64> {
        match self {
            Self::Short => Some(SHORT_MS),
            Self::Toast => Some(TOAST_MS),
            Self::Indefinite => None,
            Self::Millis(ms) => Some(ms),
        }
    }
}

/// A banner with an optional action of type `A` (for example a `StoreCmd`). Toasts have no action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Banner<A> {
    pub id: u64,
    pub text: String,
    pub action: Option<(String, A)>,
    pub length: Length,
}

#[derive(Debug)]
struct Shown<A> {
    banner: Banner<A>,
    until_ms: Option<u64>,
}

/// The banner host: one visible banner and a queue of at most `MAX_QUEUE` waiting ones.
#[derive(Debug)]
pub struct Banners<A> {
    next_id: u64,
    current: Option<Shown<A>>,
    queue: VecDeque<Banner<A>>,
}

impl<A: Clone> Default for Banners<A> {
    fn default() -> Self {
        Self {
            next_id: 1,
            current: None,
            queue: VecDeque::new(),
        }
    }
}

impl<A: Clone> Banners<A> {
    /// Adds a banner at time `now_ms`. Shows it at once if nothing is visible, otherwise queues it.
    pub fn push(
        &mut self,
        text: impl Into<String>,
        action: Option<(String, A)>,
        length: Length,
        now_ms: u64,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let banner = Banner {
            id,
            text: text.into(),
            action,
            length,
        };
        if self.current.is_none() {
            self.show(banner, now_ms);
        } else {
            if self.queue.len() == MAX_QUEUE {
                self.queue.pop_front();
            }
            self.queue.push_back(banner);
        }
        id
    }

    /// Expires the visible banner if its time is up, and shows the next one. Call it on each frame or timer.
    pub fn tick(&mut self, now_ms: u64) {
        let expired =
            matches!(&self.current, Some(Shown { until_ms: Some(until), .. }) if now_ms >= *until);
        if expired {
            self.advance(now_ms);
        }
    }

    /// Dismisses the visible banner (the X button) and shows the next one.
    pub fn dismiss(&mut self, now_ms: u64) {
        self.advance(now_ms);
    }

    /// Runs the banner's action. The action is returned only while the page is alive. Otherwise it is dropped and
    /// nothing is written (E-UI-1). Running an action dismisses the banner.
    pub fn activate_action(&mut self, page_alive: bool, now_ms: u64) -> Option<A> {
        let action = self.current.as_ref()?.banner.action.as_ref()?.1.clone();
        self.advance(now_ms);
        if page_alive { Some(action) } else { None }
    }

    pub fn current(&self) -> Option<&Banner<A>> {
        self.current.as_ref().map(|s| &s.banner)
    }

    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    fn advance(&mut self, now_ms: u64) {
        self.current = None;
        if let Some(next) = self.queue.pop_front() {
            self.show(next, now_ms);
        }
    }

    fn show(&mut self, banner: Banner<A>, now_ms: u64) {
        let until_ms = banner.length.millis().map(|ms| now_ms + ms);
        self.current = Some(Shown { banner, until_ms });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    type Host = Banners<&'static str>;

    #[test]
    fn short_banner_disappears_at_4000_ms() {
        let mut host = Host::default();
        host.push("saved", None, Length::Short, 0);
        host.tick(3999);
        assert!(host.current().is_some());
        host.tick(4000);
        assert!(host.current().is_none());
    }

    #[test]
    fn indefinite_banner_stays_until_dismissed() {
        let mut host = Host::default();
        host.push(
            "delete downloads?",
            Some(("Delete".into(), "delete")),
            Length::Indefinite,
            0,
        );
        host.tick(1_000_000);
        assert!(host.current().is_some());
        host.dismiss(1_000_000);
        assert!(host.current().is_none());
    }

    #[test]
    fn toast_hides_after_the_short_length() {
        let mut host = Host::default();
        host.push("No match found", None, Length::Toast, 100);
        host.tick(4099);
        assert!(host.current().is_some());
        host.tick(4100);
        assert!(host.current().is_none());
    }

    #[test]
    fn queue_holds_at_most_three_and_drops_the_oldest() {
        let mut host = Host::default();
        host.push("first", None, Length::Indefinite, 0);
        for text in ["a", "b", "c", "d"] {
            host.push(text, None, Length::Indefinite, 0);
        }
        assert_eq!(host.queued(), 3);
        host.dismiss(0);
        assert_eq!(
            host.current().map(|b| b.text.as_str()),
            Some("b"),
            "the oldest queued banner was dropped"
        );
    }

    #[test]
    fn next_banner_starts_its_own_timer_when_promoted() {
        let mut host = Host::default();
        host.push("one", None, Length::Short, 0);
        host.push("two", None, Length::Short, 1000);
        host.tick(4000);
        assert_eq!(host.current().map(|b| b.text.as_str()), Some("two"));
        host.tick(7999);
        assert!(host.current().is_some());
        host.tick(8000);
        assert!(host.current().is_none());
    }

    #[test]
    fn action_runs_only_when_the_page_is_alive() {
        let mut host = Host::default();
        host.push("add?", Some(("Add".into(), "add")), Length::Indefinite, 0);
        assert_eq!(
            host.activate_action(false, 0),
            None,
            "page gone: nothing is written"
        );
        host.push("add?", Some(("Add".into(), "add")), Length::Indefinite, 0);
        assert_eq!(host.activate_action(true, 0), Some("add"));
    }
}
