//! Decode session guard (T-43b-059, T-44a-050, E-40). A decode runs on a worker and its result comes back later. The
//! result is applied only if the reader session that started it is still open and the page has not been loaded again
//! since. Otherwise it is dropped without touching any widget.
//!
//! The session counter is global because the reader has one GTK thread: each opened reader bumps it with
//! [`new_session`]. Pure: the decision is [`accepts`], which takes the current values as arguments.

use std::sync::atomic::{AtomicU64, Ordering};

static SESSION: AtomicU64 = AtomicU64::new(0);

/// Starts a new reader session. Every decode started in an earlier session is stale from now on.
pub fn new_session() -> u64 {
    SESSION.fetch_add(1, Ordering::SeqCst) + 1
}

/// The session a decode started now belongs to.
pub fn current_session() -> u64 {
    SESSION.load(Ordering::SeqCst)
}

/// What a decode was started for: the reader session and the page's load generation at that time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    pub session: u64,
    pub generation: u64,
}

impl Ticket {
    /// A ticket for a load started now, in the current session.
    pub fn now(generation: u64) -> Self {
        Self { session: current_session(), generation }
    }
}

/// Whether a result for `ticket` is still wanted: it belongs to `session` and the page is still at `generation`.
pub fn accepts(ticket: Ticket, session: u64, generation: u64) -> bool {
    ticket.session == session && ticket.generation == generation
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_result_is_applied_only_for_the_current_session_and_generation() {
        let ticket = Ticket { session: 3, generation: 7 };
        assert!(accepts(ticket, 3, 7));
    }

    #[test]
    fn a_result_from_an_older_generation_is_dropped() {
        let ticket = Ticket { session: 3, generation: 7 };
        assert!(!accepts(ticket, 3, 8), "the page was loaded again");
    }

    #[test]
    fn a_result_from_a_closed_reader_session_is_dropped() {
        let ticket = Ticket { session: 3, generation: 7 };
        assert!(!accepts(ticket, 4, 7), "a newer reader session is open");
    }

    #[test]
    fn a_new_session_makes_every_older_ticket_stale() {
        let before = new_session();
        let ticket = Ticket::now(1);
        assert_eq!(ticket.session, before);
        let after = new_session();
        assert!(after > before);
        assert!(!accepts(ticket, after, 1));
    }
}
