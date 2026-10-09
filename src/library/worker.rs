//! The library worker: one `std::thread` that loads the snapshot and runs the pipeline off the GTK thread (R-10).
//! When several requests queue up, only the newest runs. The view drops any reply whose generation is no longer
//! the latest it issued, which is the upstream `collectLatest` behaviour.

use std::sync::{Arc, mpsc};

use kuro_db::handle::Db;

use super::load;
use super::query::{Built, Facts, Request, compute};

/// One pass: the request, the database to read, and the outside lookups.
pub struct Job {
    pub generation: u64,
    pub db: Arc<Db>,
    pub request: Request,
    pub facts: Facts,
}

/// The result of one job. `Err` carries the storage error text for the toast.
pub struct Reply {
    pub generation: u64,
    pub result: Result<Built, String>,
}

/// Issues generation numbers and tells whether a reply is still the latest one.
#[derive(Debug, Default)]
pub struct Gate {
    latest: u64,
}

impl Gate {
    pub fn issue(&mut self) -> u64 {
        self.latest += 1;
        self.latest
    }

    pub fn is_current(&self, generation: u64) -> bool {
        generation != 0 && generation == self.latest
    }
}

/// Handle to the worker thread. Dropping it ends the thread once the queue is empty.
pub struct Worker {
    jobs: mpsc::Sender<Job>,
}

impl Worker {
    /// Starts the thread. Replies go to `replies`, which the GTK side drains on the main context.
    pub fn spawn(replies: async_channel::Sender<Reply>) -> std::io::Result<Self> {
        let (jobs, queue) = mpsc::channel::<Job>();
        std::thread::Builder::new().name("library-worker".into()).spawn(move || {
            while let Ok(mut job) = queue.recv() {
                while let Ok(newer) = queue.try_recv() {
                    job = newer;
                }
                let result = run(&job);
                if replies.send_blocking(Reply { generation: job.generation, result }).is_err() {
                    break;
                }
            }
        })?;
        Ok(Self { jobs })
    }

    /// Queues a job. False when the thread has stopped.
    pub fn submit(&self, job: Job) -> bool {
        self.jobs.send(job).is_ok()
    }
}

fn run(job: &Job) -> Result<Built, String> {
    let snap = load::snapshot(&job.db, job.request.kind).map_err(|e| e.to_string())?;
    Ok(compute(&job.request, &snap, &job.facts))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_generation_is_current() {
        let mut gate = Gate::default();
        let first = gate.issue();
        let second = gate.issue();
        assert!(!gate.is_current(first), "a superseded search must not be applied");
        assert!(gate.is_current(second));
        assert!(!gate.is_current(0), "generation 0 is never issued");
    }
}
