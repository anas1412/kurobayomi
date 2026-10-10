//! App-level wiring of the library update jobs (area 26): the automatic update interval, the runner behind the
//! library's refresh button and the schedule loops. Pure decisions are in `interval` and `runner`; GTK only
//! appears where the preference watch is polled on the main thread.

pub mod app_update;
pub mod badge;
pub mod ids;
pub mod interval;
pub mod notify;
pub mod router;
pub mod rules;
pub mod runner;
pub mod throttle;

/// Starts the update runner and the schedule once, from the GTK main thread inside the app's runtime.
pub fn start() {
    notify::attach();
    runner::start();
}
