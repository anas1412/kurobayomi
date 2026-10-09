//! Veto-gated preference writes. A write runs the veto first, applies the value only when the veto
//! accepts it, and only when no newer write for the same control was started (last write wins).

use std::cell::Cell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use gtk::glib;
use kuro_prefs::pref::Pref;
use kuro_prefs::values::PrefType;

/// `Ok(true)` writes, `Ok(false)` keeps the old value, `Err` keeps the old value and shows the message.
pub type VetoResult = Result<bool, String>;
pub type VetoFuture = Pin<Box<dyn Future<Output = VetoResult>>>;
pub type Veto<T> = Rc<dyn Fn(T) -> VetoFuture>;

/// Wraps an async closure as a [`Veto`].
pub fn veto<T, F, Fut>(f: F) -> Veto<T>
where
    T: 'static,
    F: Fn(T) -> Fut + 'static,
    Fut: Future<Output = VetoResult> + 'static,
{
    Rc::new(move |value| Box::pin(f(value)))
}

/// A veto that accepts every value.
pub fn accept<T: 'static>() -> Veto<T> {
    veto(|_| async { Ok(true) })
}

/// Generation counter: each write takes a ticket, and only the newest ticket may apply.
#[derive(Default)]
pub struct WriteGate {
    latest: Cell<u64>,
}

impl WriteGate {
    pub fn begin(&self) -> u64 {
        let next = self.latest.get() + 1;
        self.latest.set(next);
        next
    }

    pub fn is_latest(&self, ticket: u64) -> bool {
        self.latest.get() == ticket
    }
}

/// What a finished write did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Written,
    /// The veto said no, or a newer write replaced this one. The old value stays.
    Kept,
    /// The veto or the store failed. The dialog stays open and shows this message.
    Failed(String),
}

/// Pure decision for a finished veto. `latest` is `WriteGate::is_latest` at the time it finished.
pub fn decide(result: VetoResult, latest: bool) -> Result<bool, String> {
    match result {
        Ok(true) if latest => Ok(true),
        Ok(true) | Ok(false) => Ok(false),
        Err(message) => Err(message),
    }
}

/// Runs `veto(value)` on the GTK thread, then writes `value` to `pref` if it is accepted.
/// `on_done` receives the outcome. Must be called on the GTK main thread.
pub fn write<T>(
    gate: Rc<WriteGate>,
    veto: Veto<T>,
    pref: Pref<T>,
    value: T,
    on_done: impl FnOnce(Outcome) + 'static,
) where
    T: PrefType + Clone + 'static,
{
    let ticket = gate.begin();
    glib::spawn_future_local(async move {
        let result = veto(value.clone()).await;
        let outcome = match decide(result, gate.is_latest(ticket)) {
            Ok(true) => match pref.set(value) {
                Ok(()) => Outcome::Written,
                Err(err) => Outcome::Failed(err.to_string()),
            },
            Ok(false) => Outcome::Kept,
            Err(message) => Outcome::Failed(message),
        };
        on_done(outcome);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_latest_write_applies() {
        assert_eq!(decide(Ok(true), true), Ok(true));
    }

    #[test]
    fn rejected_or_superseded_write_keeps_the_old_value() {
        assert_eq!(decide(Ok(false), true), Ok(false));
        assert_eq!(decide(Ok(true), false), Ok(false));
    }

    #[test]
    fn veto_error_is_reported_not_written() {
        assert_eq!(decide(Err("disk full".into()), true), Err("disk full".to_owned()));
    }

    #[test]
    fn last_write_wins_when_two_writes_overlap() {
        let gate = WriteGate::default();
        let first = gate.begin();
        let second = gate.begin();
        assert!(!gate.is_latest(first));
        assert!(gate.is_latest(second));
        assert_eq!(decide(Ok(true), gate.is_latest(first)), Ok(false));
        assert_eq!(decide(Ok(true), gate.is_latest(second)), Ok(true));
    }
}

/// Calls `apply` with each new value of `pref` on the GTK thread, until `alive` turns false.
/// Polls, because `Watch` is a blocking channel that cannot be awaited from the main loop.
/// `apply` must not write the preference back (the feedback write is what E-59 forbids).
pub fn follow<T>(pref: &Pref<T>, alive: impl Fn() -> bool + 'static, apply: impl Fn(T) + 'static)
where
    T: PrefType + Clone + 'static,
{
    let watch = pref.watch();
    glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
        if !alive() {
            return glib::ControlFlow::Break;
        }
        if let Some(value) = watch.try_changed() {
            apply(value);
        }
        glib::ControlFlow::Continue
    });
}
