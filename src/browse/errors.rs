//! The one place where a failed source request becomes a sentence for the user. A source request is never allowed to
//! leave a spinner on screen: [`limited`] ends it after [`LOAD_TIMEOUT`], [`settle`] also catches a panic, and
//! [`plain_message`] words the failure in plain English. The technical text (signature, key, HTTP code, parse
//! error) goes to the log only. [`note`] counts the failures in a row of each source for the "Not working right now"
//! badge of the Sources list.

use kuro_i18n::{MsgKey, tr};
use std::collections::HashMap;
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

use futures::FutureExt;
use kuro_source::SourceError;

/// A request that takes longer than this is given up.
pub const LOAD_TIMEOUT: Duration = Duration::from_secs(30);

/// The limit of the outer guard of a page, a little above [`LOAD_TIMEOUT`] so the loader's own limit speaks first.
pub const BACKSTOP: Duration = Duration::from_secs(35);

/// The sentence when the device has no connection.
pub fn offline() -> String {
    tr(MsgKey::browse_errors_offline)
}

/// The sentence for a failure that is not the source's (the library file, for example).
pub fn generic() -> String {
    tr(MsgKey::browse_errors_generic)
}

/// The badge of a source that failed [`BADGE_AFTER`] times in a row.
pub fn not_working_badge() -> String {
    tr(MsgKey::browse_errors_not_working_badge)
}

/// Failures in a row after which the Sources list shows [`not_working_badge()`].
pub const BADGE_AFTER: u32 = 3;

/// What went wrong, in the three kinds the user is told apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trouble {
    Offline,
    Slow,
    Broken,
}

/// Parts of a transport text that say the host could not be reached at all.
const OFFLINE_HINTS: [&str; 3] = ["dns error", "failed to lookup address", "network is unreachable"];

/// Parts of a transport text that say the source took too long.
const SLOW_HINTS: [&str; 3] = ["timed out", "timeout", "deadline"];

/// The kind of a failed source call. `online` is whether the device has a connection.
pub fn classify(error: &SourceError, online: bool) -> Trouble {
    match error {
        SourceError::UnknownHost { .. } if !online => Trouble::Offline,
        SourceError::Transport(_) if !online => Trouble::Offline,
        SourceError::Transport(text) => {
            let text = text.to_lowercase();
            if OFFLINE_HINTS.iter().any(|hint| text.contains(hint)) {
                Trouble::Offline
            } else if SLOW_HINTS.iter().any(|hint| text.contains(hint)) {
                Trouble::Slow
            } else {
                Trouble::Broken
            }
        }
        _ => Trouble::Broken,
    }
}

/// The sentence for a kind of trouble with `source` (its bare name; a blank name reads "This source").
pub fn says(source: &str, trouble: Trouble) -> String {
    let name = if source.trim().is_empty() { "This source" } else { source.trim() };
    match trouble {
        Trouble::Offline => offline().to_owned(),
        Trouble::Slow => format!("{name} is not answering. Try again in a moment."),
        Trouble::Broken => format!("{name} isn't working right now. It may be down or need an update; the repository checks for fixes every day."),
    }
}

/// The one line of a source's section in the global search.
pub fn couldnt_load(source: &str) -> String {
    format!("Couldn't load {}", if source.trim().is_empty() { "this source" } else { source.trim() })
}

fn device_online() -> bool {
    use gtk::gio::prelude::NetworkMonitorExt as _;
    gtk::gio::NetworkMonitor::default().is_network_available()
}

/// The sentence for a failed call of `source`. The error itself is logged and never shown.
pub fn plain_message(source: &str, error: &SourceError) -> String {
    tracing::warn!(source, %error, "a source request failed");
    says(source, classify(error, device_online()))
}

/// Runs a source call behind the panic guard and the [`LOAD_TIMEOUT`]. A call that does not answer in time fails with a
/// transport error that [`classify`] reads as slow.
pub async fn limited<T>(call: impl Future<Output = Result<T, SourceError>>) -> Result<T, SourceError> {
    limited_for(LOAD_TIMEOUT, call).await
}

async fn limited_for<T>(limit: Duration, call: impl Future<Output = Result<T, SourceError>>) -> Result<T, SourceError> {
    match tokio::time::timeout(limit, kuro_source::isolate(call)).await {
        Ok(result) => result,
        Err(_) => Err(SourceError::Transport(format!("no answer after {} s: timed out", limit.as_secs()))),
    }
}

/// [`limited`], with the failure as the sentence for the user.
pub async fn plain<T>(source: &str, call: impl Future<Output = Result<T, SourceError>>) -> Result<T, String> {
    limited(call).await.map_err(|error| plain_message(source, &error))
}

/// Ends a load that reports its failure as text: after `limit` it fails with the "not answering" sentence, and a panic
/// inside it fails with the "isn't working" sentence. An error the load reports itself passes through unchanged.
pub async fn settle<T>(limit: Duration, source: &str, load: impl Future<Output = Result<T, String>>) -> Result<T, String> {
    match tokio::time::timeout(limit, AssertUnwindSafe(load).catch_unwind()).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => {
            tracing::warn!(source, "a source request panicked");
            Err(says(source, Trouble::Broken))
        }
        Err(_) => {
            tracing::warn!(source, seconds = limit.as_secs(), "a source request did not answer");
            Err(says(source, Trouble::Slow))
        }
    }
}

/// The failures in a row of each source in this session.
#[derive(Default)]
struct Streaks(HashMap<i64, u32>);

impl Streaks {
    /// Records one result. True when the source's badge turned on or off by it.
    fn record(&mut self, source_id: i64, ok: bool) -> bool {
        let before = self.failing(source_id);
        if ok {
            self.0.remove(&source_id);
        } else {
            let count = self.0.entry(source_id).or_insert(0);
            *count = count.saturating_add(1);
        }
        before != self.failing(source_id)
    }

    fn failing(&self, source_id: i64) -> bool {
        self.0.get(&source_id).is_some_and(|count| *count >= BADGE_AFTER)
    }
}

static STREAKS: LazyLock<Mutex<Streaks>> = LazyLock::new(Mutex::default);

/// Records the result of a source's listing. The Sources lists read again when a badge turns on or off.
pub fn note(source_id: i64, ok: bool) {
    let changed = STREAKS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).record(source_id, ok);
    if changed {
        crate::recipe_sources::notify_changed();
    }
}

/// Whether the source failed [`BADGE_AFTER`] times in a row, with no success since.
pub fn is_failing(source_id: i64) -> bool {
    STREAKS.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).failing(source_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn transport(text: &str) -> SourceError {
        SourceError::Transport(text.to_owned())
    }

    #[test]
    fn no_connection_is_the_offline_sentence_whatever_the_error() {
        for error in [SourceError::UnknownHost { host: "x.example".into() }, transport("connection reset")] {
            assert_eq!(classify(&error, false), Trouble::Offline);
        }
        assert_eq!(says("Comix", Trouble::Offline), "You're offline. Check your connection and try again.");
    }

    #[test]
    fn a_dns_failure_while_online_is_still_offline_and_a_timeout_is_not_answering() {
        assert_eq!(classify(&transport("error sending request: dns error: failed"), true), Trouble::Offline);
        assert_eq!(classify(&transport("operation timed out"), true), Trouble::Slow);
        assert_eq!(classify(&transport("Deadline exceeded"), true), Trouble::Slow);
        assert_eq!(says("Comix", Trouble::Slow), "Comix is not answering. Try again in a moment.");
    }

    #[test]
    fn everything_else_is_the_source_is_not_working_and_never_shows_technical_words() {
        let message = says("Comix", Trouble::Broken);
        assert_eq!(message, "Comix isn't working right now. It may be down or need an update; the repository checks for fixes every day.");
        let signature = transport("Comix refused the request signature (keys of site build tmonyg)");
        for error in [signature, SourceError::Http(403), SourceError::Parse("expected value at line 1".into()), SourceError::Unsupported("search")] {
            assert_eq!(classify(&error, true), Trouble::Broken, "{error}");
            let shown = says("Comix", classify(&error, true));
            for word in ["signature", "key", "HTTP", "403", "parse", "wasm", "ABI"] {
                assert!(!shown.contains(word), "{word} in {shown}");
            }
        }
    }

    #[test]
    fn a_blank_source_name_reads_this_source() {
        assert_eq!(says("  ", Trouble::Slow), "This source is not answering. Try again in a moment.");
        assert_eq!(couldnt_load("Comix"), "Couldn't load Comix");
        assert_eq!(couldnt_load(""), "Couldn't load this source");
    }

    fn run<T>(future: impl Future<Output = T>) -> T {
        tokio::runtime::Builder::new_current_thread().enable_time().build().expect("runtime").block_on(future)
    }

    #[test]
    fn a_call_that_never_answers_ends_as_a_slow_failure() {
        let result: Result<(), SourceError> = run(limited_for(Duration::from_millis(20), std::future::pending()));
        let error = result.expect_err("the limit ends the call");
        assert_eq!(classify(&error, true), Trouble::Slow);
    }

    #[test]
    fn a_call_that_answers_or_panics_is_not_swallowed_by_the_limit() {
        assert_eq!(run(limited_for(Duration::from_secs(5), async { Ok::<_, SourceError>(7) })), Ok(7));
        let panicked: Result<(), SourceError> = run(limited_for(Duration::from_secs(5), async { panic!("a parser bug") }));
        assert_eq!(classify(&panicked.expect_err("a panic is a failure"), true), Trouble::Broken);
    }

    #[test]
    fn settle_ends_a_hang_and_a_panic_and_keeps_the_loaders_own_answer() {
        let hang: Result<(), String> = run(settle(Duration::from_millis(20), "Comix", std::future::pending()));
        assert_eq!(hang, Err("Comix is not answering. Try again in a moment.".to_owned()));
        let panic: Result<(), String> = run(settle(Duration::from_secs(5), "Comix", async { panic!("boom") }));
        assert_eq!(panic, Err(says("Comix", Trouble::Broken)));
        assert_eq!(run(settle(Duration::from_secs(5), "Comix", async { Ok::<_, String>(1) })), Ok(1));
        assert_eq!(run(settle(Duration::from_secs(5), "Comix", async { Err::<(), _>("mine".to_owned()) })), Err("mine".to_owned()));
    }

    #[test]
    fn three_failures_in_a_row_turn_the_badge_on_and_one_success_turns_it_off() {
        let mut streaks = Streaks::default();
        assert!(!streaks.record(1, false));
        assert!(!streaks.record(1, false));
        assert!(!streaks.failing(1));
        assert!(streaks.record(1, false), "the third failure turns the badge on");
        assert!(!streaks.record(1, false), "no change after that");
        assert!(!streaks.failing(2), "other sources are not touched");
        assert!(streaks.record(1, true), "a success turns it off");
        assert!(!streaks.failing(1));
        streaks.record(1, false);
        streaks.record(1, true);
        streaks.record(1, false);
        streaks.record(1, false);
        assert!(!streaks.failing(1), "a success in between restarts the count");
    }
}
