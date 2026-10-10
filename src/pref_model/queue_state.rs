//! Download queue state for the More tab (tasks T-47ba-015 and T-47ba-016).
//!
//! [`derive`] is the Kotlin truth table of `MoreTab` (T-E54). [`QueueCombiner`] keeps the latest four inputs (manga
//! running, manga queue length, anime running, anime queue length) and publishes the derived state on a
//! `watch` channel. It wakes only when an input changes, and it sends only when the state changed.

use tokio::sync::watch;

/// The download queue as the More tab shows it. `pending` counts queued items, not the one in flight.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QueueState {
    Stopped,
    Paused { pending: i64 },
    Downloading { pending: i64 },
}

/// The four values the More tab combines (the manga and anime download managers).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueInputs {
    pub manga_running: bool,
    pub manga_len: i64,
    pub anime_running: bool,
    pub anime_len: i64,
}

/// The state for `inputs`. An empty queue is `Stopped` even when a downloader reports running (Kotlin checks the
/// queue first). A non-empty queue is `Downloading` while a downloader runs and `Paused` otherwise.
pub fn derive(inputs: QueueInputs) -> QueueState {
    let pending = inputs.manga_len + inputs.anime_len;
    let running = inputs.manga_running || inputs.anime_running;
    if pending == 0 {
        QueueState::Stopped
    } else if !running {
        QueueState::Paused { pending }
    } else {
        QueueState::Downloading { pending }
    }
}

/// Keeps the latest four inputs and publishes [`derive`] of them. The published value starts as `Stopped`.
pub struct QueueCombiner {
    manga_running: watch::Receiver<bool>,
    manga_len: watch::Receiver<i64>,
    anime_running: watch::Receiver<bool>,
    anime_len: watch::Receiver<i64>,
    out: watch::Sender<QueueState>,
}

impl QueueCombiner {
    /// The combiner and the receiver the More tab reads.
    pub fn new(
        manga_running: watch::Receiver<bool>,
        manga_len: watch::Receiver<i64>,
        anime_running: watch::Receiver<bool>,
        anime_len: watch::Receiver<i64>,
    ) -> (Self, watch::Receiver<QueueState>) {
        let (out, rx) = watch::channel(QueueState::Stopped);
        let combiner = Self { manga_running, manga_len, anime_running, anime_len, out };
        (combiner, rx)
    }

    /// The latest value of each input.
    pub fn inputs(&self) -> QueueInputs {
        QueueInputs {
            manga_running: *self.manga_running.borrow(),
            manga_len: *self.manga_len.borrow(),
            anime_running: *self.anime_running.borrow(),
            anime_len: *self.anime_len.borrow(),
        }
    }

    /// Recomputes the state from the latest inputs and sends it when it differs from the published one.
    pub fn publish(&self) {
        let state = derive(self.inputs());
        self.out.send_if_modified(|current| {
            let changed = *current != state;
            *current = state;
            changed
        });
    }

    /// Publishes after every input change, until all four inputs are closed. The last state stays published.
    pub async fn run(mut self) {
        let mut open = [true; 4];
        while open.iter().any(|o| *o) {
            tokio::select! {
                r = self.manga_running.changed(), if open[0] => open[0] = r.is_ok(),
                r = self.manga_len.changed(), if open[1] => open[1] = r.is_ok(),
                r = self.anime_running.changed(), if open[2] => open[2] = r.is_ok(),
                r = self.anime_len.changed(), if open[3] => open[3] = r.is_ok(),
            }
            self.publish();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs(manga_running: bool, manga_len: i64, anime_running: bool, anime_len: i64) -> QueueInputs {
        QueueInputs { manga_running, manga_len, anime_running, anime_len }
    }

    #[test]
    fn derive_matches_the_t_e54_truth_table() {
        let table = [
            (inputs(false, 0, false, 0), QueueState::Stopped),
            (inputs(true, 0, false, 0), QueueState::Stopped),
            (inputs(false, 3, false, 0), QueueState::Paused { pending: 3 }),
            (inputs(true, 3, false, 0), QueueState::Downloading { pending: 3 }),
            (inputs(false, 0, true, 2), QueueState::Downloading { pending: 2 }),
            (inputs(false, 1, true, 2), QueueState::Downloading { pending: 3 }),
            (inputs(true, 0, false, 1), QueueState::Downloading { pending: 1 }),
            (inputs(false, 1, false, 1), QueueState::Paused { pending: 2 }),
        ];
        for (input, expected) in table {
            assert_eq!(derive(input), expected, "{input:?}");
        }
    }

    #[test]
    fn a_stopped_downloader_with_an_empty_queue_is_stopped() {
        assert_eq!(derive(QueueInputs::default()), QueueState::Stopped);
    }

    /// A small deterministic generator, so the convergence test needs no random crate.
    fn next(seed: &mut u64) -> u64 {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *seed >> 33
    }

    #[tokio::test]
    async fn the_published_state_converges_to_derive_of_the_final_inputs() {
        let (mt, mrx) = watch::channel(false);
        let (lt, lrx) = watch::channel(0i64);
        let (at, arx) = watch::channel(false);
        let (nt, nrx) = watch::channel(0i64);
        let (combiner, out) = QueueCombiner::new(mrx, lrx, arx, nrx);
        let task = tokio::spawn(combiner.run());

        let mut seed = 7u64;
        for _ in 0..20 {
            match next(&mut seed) % 4 {
                0 => {
                    mt.send_replace(next(&mut seed) % 2 == 0);
                }
                1 => {
                    lt.send_replace((next(&mut seed) % 5) as i64);
                }
                2 => {
                    at.send_replace(next(&mut seed) % 2 == 0);
                }
                _ => {
                    nt.send_replace((next(&mut seed) % 5) as i64);
                }
            }
            tokio::task::yield_now().await;
        }
        let last = inputs(*mt.borrow(), *lt.borrow(), *at.borrow(), *nt.borrow());
        drop((mt, lt, at, nt));
        task.await.expect("combiner task");
        assert_eq!(*out.borrow(), derive(last));
    }

    #[test]
    fn publish_sends_only_when_the_state_changes() {
        let (_mt, mrx) = watch::channel(false);
        let (lt, lrx) = watch::channel(0i64);
        let (_at, arx) = watch::channel(false);
        let (_nt, nrx) = watch::channel(0i64);
        let (combiner, mut out) = QueueCombiner::new(mrx, lrx, arx, nrx);
        assert_eq!(*out.borrow(), QueueState::Stopped);
        lt.send_replace(2);
        combiner.publish();
        assert_eq!(out.has_changed().ok(), Some(true));
        assert_eq!(*out.borrow_and_update(), QueueState::Paused { pending: 2 });
        combiner.publish();
        assert_eq!(out.has_changed().ok(), Some(false));
    }

    /// The T-E54 table as the checked-in fixture file.
    #[test]
    fn derive_matches_the_fixture_table() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/queue/derive_table.csv");
        let table = std::fs::read_to_string(path).unwrap();
        let flag = |s: &str| s == "true";
        let mut rows = 0;
        for line in table.lines().skip(1) {
            let f: Vec<&str> = line.split(',').collect();
            let expected = match f[4].split_once('{') {
                None => QueueState::Stopped,
                Some((kind, n)) => {
                    let pending = n.trim_end_matches('}').parse().unwrap();
                    if kind == "Paused" { QueueState::Paused { pending } } else { QueueState::Downloading { pending } }
                }
            };
            assert_eq!(derive(inputs(flag(f[0]), f[1].parse().unwrap(), flag(f[2]), f[3].parse().unwrap())), expected, "{line}");
            rows += 1;
        }
        assert_eq!(rows, 8);
    }
}
