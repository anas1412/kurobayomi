//! Retry schedule shared by the manga page fetch and the anime episode download (T-24a-015, behaviour anime §5.5).
//!
//! Four attempts in total. A failed attempt waits 2 s, then 4 s, then 8 s before the next one. The final failure is
//! returned as it is.

use std::future::Future;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::error::DownloadError;

/// Waits before retries 1, 2 and 3: `(2 << attempt) * 1000` ms.
pub const RETRY_DELAYS: [Duration; 3] = [Duration::from_secs(2), Duration::from_secs(4), Duration::from_secs(8)];

/// Runs `op` until it succeeds or the schedule is spent. Cancellation during a wait returns
/// [`DownloadError::Cancelled`] at once, and a `Cancelled` from `op` is never retried. A missing ffmpeg binary is not
/// retried either: waiting does not put it on `PATH` (T-24a-067).
pub async fn with_retry<T, F, Fut>(cancel: &CancellationToken, mut op: F) -> Result<T, DownloadError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, DownloadError>>,
{
    let mut attempt = 0;
    loop {
        if cancel.is_cancelled() {
            return Err(DownloadError::Cancelled);
        }
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if matches!(err, DownloadError::Cancelled | DownloadError::FfmpegMissing) || attempt >= RETRY_DELAYS.len() {
                    return Err(err);
                }
                tokio::select! {
                    _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
                    _ = tokio::time::sleep(RETRY_DELAYS[attempt]) => {}
                }
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test(start_paused = true)]
    async fn a_missing_ffmpeg_is_returned_without_a_retry() {
        let calls = AtomicUsize::new(0);
        let cancel = CancellationToken::new();
        let result: Result<(), DownloadError> = with_retry(&cancel, || {
            calls.fetch_add(1, Ordering::SeqCst);
            async { Err(DownloadError::FfmpegMissing) }
        })
        .await;
        assert!(matches!(result, Err(DownloadError::FfmpegMissing)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn schedule_is_two_four_eight_seconds() {
        assert_eq!(RETRY_DELAYS, [Duration::from_secs(2), Duration::from_secs(4), Duration::from_secs(8)]);
    }

    #[tokio::test(start_paused = true)]
    async fn four_attempts_with_the_exact_delays() {
        let cancel = CancellationToken::new();
        let calls = AtomicUsize::new(0);
        let started = tokio::time::Instant::now();
        let result: Result<(), DownloadError> = with_retry(&cancel, || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(DownloadError::NoPages)
        })
        .await;
        assert!(matches!(result, Err(DownloadError::NoPages)));
        assert_eq!(calls.load(Ordering::SeqCst), 4);
        assert_eq!(started.elapsed(), Duration::from_secs(14));
    }

    #[tokio::test(start_paused = true)]
    async fn success_on_a_later_attempt_returns_the_value() {
        let cancel = CancellationToken::new();
        let calls = AtomicUsize::new(0);
        let value = with_retry(&cancel, || async {
            if calls.fetch_add(1, Ordering::SeqCst) < 2 { Err(DownloadError::NoPages) } else { Ok(7) }
        })
        .await;
        assert_eq!(value.unwrap(), 7);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
    }

    #[tokio::test(start_paused = true)]
    async fn cancellation_during_a_wait_returns_at_once() {
        let cancel = CancellationToken::new();
        let stopper = cancel.clone();
        let started = tokio::time::Instant::now();
        let result: Result<(), DownloadError> =
            tokio::join!(with_retry(&cancel, || async { Err(DownloadError::NoPages) }), async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                stopper.cancel();
            })
            .0;
        assert!(matches!(result, Err(DownloadError::Cancelled)));
        assert_eq!(started.elapsed(), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn a_cancelled_operation_is_not_retried() {
        let cancel = CancellationToken::new();
        let calls = AtomicUsize::new(0);
        let result: Result<(), DownloadError> = with_retry(&cancel, || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Err(DownloadError::Cancelled)
        })
        .await;
        assert!(matches!(result, Err(DownloadError::Cancelled)));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
