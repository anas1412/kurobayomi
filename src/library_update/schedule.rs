//! The automatic schedule of one kind (behaviour 1.1 and the scheduling topic, section 4): a period of N hours,
//! started a random 0 to 10 minutes early (the flex window), linear backoff of 10 minutes per retry capped at
//! 5 hours, and a restart on every interval change. Interval 0 means off.

use std::future::Future;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::watch;

use crate::library_update::pass::PassStatus;

pub const FLEX_MS: u64 = 10 * 60_000;
pub const BACKOFF_STEP_MS: u64 = 10 * 60_000;
pub const BACKOFF_CAP_MS: u64 = 5 * 3_600_000;

pub fn period_ms(interval_hours: u32) -> u64 {
    u64::from(interval_hours) * 3_600_000
}

/// Delay before the next automatic pass: the period minus a jitter in `[0, FLEX_MS]`.
pub fn next_delay_ms(interval_hours: u32, jitter_ms: u64) -> u64 {
    period_ms(interval_hours).saturating_sub(jitter_ms.min(FLEX_MS))
}

/// Delay after `attempt` retries (1 for the first): 10 minutes per attempt, capped at 5 hours.
pub fn backoff_ms(attempt: u32) -> u64 {
    (u64::from(attempt) * BACKOFF_STEP_MS).min(BACKOFF_CAP_MS)
}

/// A jitter in `[0, FLEX_MS]` from the sub-second clock. Good enough to spread the starts; not random in a
/// cryptographic sense.
fn jitter_ms() -> u64 {
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| u64::from(d.subsec_nanos())).unwrap_or(0);
    nanos % (FLEX_MS + 1)
}

/// The loop of one kind. `run` starts one automatic pass and reports its status. The pass is awaited outside the
/// select, so an interval change never cancels a running pass; it only changes the next wait.
pub async fn run_schedule<F, Fut>(mut interval_hours: watch::Receiver<u32>, mut run: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = PassStatus>,
{
    let mut attempt: u32 = 0;
    // Set while the schedule is off: the first pass after turning it on runs at once (T-26a-010, WorkManager parity).
    let mut run_now = false;
    loop {
        let hours = *interval_hours.borrow_and_update();
        if hours == 0 {
            if interval_hours.changed().await.is_err() {
                return;
            }
            attempt = 0;
            run_now = true;
            continue;
        }
        let wait = if run_now {
            0
        } else if attempt == 0 {
            next_delay_ms(hours, jitter_ms())
        } else {
            backoff_ms(attempt)
        };
        run_now = false;
        tokio::select! {
            () = tokio::time::sleep(Duration::from_millis(wait)) => {
                attempt = match run().await {
                    PassStatus::Retry => attempt + 1,
                    _ => 0,
                };
            }
            changed = interval_hours.changed() => {
                if changed.is_err() {
                    return;
                }
                attempt = 0;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn jitter_window_is_n_hours_minus_up_to_ten_minutes() {
        // SC-01/SC-02: interval N hours runs with jitter in [N h - 10 min, N h].
        assert_eq!(next_delay_ms(12, 0), 12 * 3_600_000);
        assert_eq!(next_delay_ms(12, FLEX_MS), 12 * 3_600_000 - FLEX_MS);
        assert_eq!(next_delay_ms(1, 99 * 60_000), 50 * 60_000, "jitter beyond the flex window is clamped");
    }

    #[test]
    fn backoff_is_linear_in_ten_minute_steps_and_capped_at_five_hours() {
        // SC-08
        assert_eq!(backoff_ms(1), 10 * 60_000);
        assert_eq!(backoff_ms(2), 20 * 60_000);
        assert_eq!(backoff_ms(3), 30 * 60_000);
        assert_eq!(backoff_ms(30), BACKOFF_CAP_MS);
        assert_eq!(backoff_ms(31), BACKOFF_CAP_MS);
    }

    #[tokio::test]
    async fn turning_the_schedule_on_runs_the_first_pass_at_once() {
        // T-26a-010: enabling runs the first pass without waiting a period.
        let (tx, rx) = watch::channel(0u32);
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let handle = tokio::spawn(run_schedule(rx, move || {
            counter.fetch_add(1, Ordering::SeqCst);
            async { PassStatus::Success }
        }));
        // let the loop reach its wait for the change first: a value already there at start is a startup value
        tokio::time::sleep(Duration::from_millis(10)).await;
        tx.send(12).expect("the loop is listening");
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(runs.load(Ordering::SeqCst), 1, "the first pass runs at once");
        drop(tx);
        tokio::time::timeout(Duration::from_secs(5), handle).await.expect("the loop ends").expect("no panic");
    }

    #[tokio::test]
    async fn interval_zero_never_runs_a_pass() {
        // SC-01: no loop while off.
        let (tx, rx) = watch::channel(0u32);
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let result = tokio::time::timeout(
            Duration::from_millis(50),
            run_schedule(rx, move || {
                counter.fetch_add(1, Ordering::SeqCst);
                async { PassStatus::Success }
            }),
        )
        .await;
        assert!(result.is_err(), "the loop only waits while the interval is 0");
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        drop(tx);
    }

    #[tokio::test]
    async fn a_changed_interval_replaces_the_wait_and_closing_the_setting_ends_the_loop() {
        // SC-03: a new interval replaces the wait without running a pass. Closing the setting ends the loop.
        let (tx, rx) = watch::channel(1_000u32);
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&runs);
        let handle = tokio::spawn(run_schedule(rx, move || {
            counter.fetch_add(1, Ordering::SeqCst);
            async { PassStatus::Success }
        }));
        tx.send(2_000).expect("the loop is listening");
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(runs.load(Ordering::SeqCst), 0);
        drop(tx);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("the loop ends when its setting is dropped")
            .expect("the loop does not panic");
        assert_eq!(runs.load(Ordering::SeqCst), 0);
    }
}
