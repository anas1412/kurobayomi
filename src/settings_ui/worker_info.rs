//! The Worker info page (task T-47b-043): the jobs of the registry in three sections, Enqueued, Finished and Running,
//! as the Android dump prints them. The text refreshes while the page is open, the copy action puts the three dumps
//! together on the clipboard, and the monospace text scrolls sideways instead of wrapping.
//!
//! Decisions: the next scheduled run uses the platform's short date and time (`%x %X`), not the `app_date_format`
//! preference, because the shared date-time formatter lives in `kuro-prefs`, which is not in this lane. `Blocked`
//! jobs are in no section, as in the Android queries.
use std::time::Duration;

use chrono::{Local, TimeZone};
use gtk::prelude::*;

use crate::screens::{built, Built};
use crate::settings_ui::clipboard::copy_to_clipboard;
use crate::settings_ui::jobs::{global, JobInfo, JobState};

/// How often the open page re-reads the registry.
const REFRESH: Duration = Duration::from_secs(1);

/// The dump of `jobs`: `-` for none, else Id, Tags, State and, for an enqueued job, its next run and attempt, each job
/// closed by a blank line.
pub fn construct_string(jobs: &[JobInfo]) -> String {
    if jobs.is_empty() {
        return "-\n".to_owned();
    }
    let mut out = String::new();
    for job in jobs {
        out.push_str(&format!("Id: {}\nTags:\n", job.id));
        for tag in &job.tags {
            out.push_str(&format!(" - {tag}\n"));
        }
        out.push_str(&format!("State: {}\n", job.state.label()));
        if job.state == JobState::Enqueued {
            if let Some(next) = job.next_run.and_then(|ms| Local.timestamp_millis_opt(ms).single()) {
                out.push_str(&format!("Next scheduled run: {}\n", next.format("%x %X")));
            }
            out.push_str(&format!("Attempt #{}\n", job.attempts + 1));
        }
        out.push('\n');
    }
    out
}

/// The three dumps of a registry snapshot: enqueued, finished, running.
pub fn sections(jobs: &[JobInfo]) -> [String; 3] {
    let pick = |states: &[JobState]| -> Vec<JobInfo> { jobs.iter().filter(|job| states.contains(&job.state)).cloned().collect() };
    [
        construct_string(&pick(&[JobState::Enqueued])),
        construct_string(&pick(&[JobState::Succeeded, JobState::Failed, JobState::Cancelled])),
        construct_string(&pick(&[JobState::Running])),
    ]
}

/// What the copy action copies: the three dumps one after the other.
pub fn copy_text(sections: &[String; 3]) -> String {
    sections.concat()
}

const TITLES: [&str; 3] = ["Enqueued", "Finished", "Running"];

/// The page: a section title and a monospace text per section, refreshed every second while the page is alive.
pub fn page() -> Option<Built> {
    let body = gtk::Box::new(gtk::Orientation::Vertical, 0);
    body.set_margin_start(16);
    body.set_margin_end(16);
    let texts = TITLES.map(|title| {
        let heading = gtk::Label::builder().label(title).xalign(0.0).css_classes(["title-4"]).build();
        heading.set_margin_top(8);
        heading.set_margin_bottom(8);
        let text = gtk::Label::builder().xalign(0.0).yalign(0.0).selectable(true).wrap(false).css_classes(["monospace"]).build();
        body.append(&heading);
        body.append(&text);
        text
    });
    let refresh = {
        let texts = texts.clone();
        move || {
            for (label, dump) in texts.iter().zip(sections(&global().snapshot())) {
                if label.label() != dump {
                    label.set_label(&dump);
                }
            }
        }
    };
    refresh();
    let weak = body.downgrade();
    gtk::glib::timeout_add_local(REFRESH, move || match weak.upgrade() {
        Some(_) => {
            refresh();
            gtk::glib::ControlFlow::Continue
        }
        None => gtk::glib::ControlFlow::Break,
    });
    let scroller = gtk::ScrolledWindow::builder().child(&body).vexpand(true).build();
    let tools = super::route::toolbar("settings_worker_info");
    for copy in tools.iter().filter_map(|w| w.downcast_ref::<gtk::Button>()) {
        let texts = texts.clone();
        copy.connect_clicked(move |_| {
            let dumps = texts.clone().map(|label| label.label().to_string());
            copy_to_clipboard(&copy_text(&dumps));
        });
    }
    built(scroller, tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: u64, state: JobState, next_run: Option<i64>, attempts: u32) -> JobInfo {
        JobInfo { id, name: format!("job{id}"), tags: vec!["LibraryUpdate".into(), "Manual".into()], state, next_run, attempts }
    }

    #[test]
    fn no_jobs_print_a_dash_line() {
        assert_eq!(construct_string(&[]), "-\n");
    }

    #[test]
    fn a_finished_job_prints_id_tags_state_and_a_blank_line() {
        let text = construct_string(&[job(7, JobState::Succeeded, None, 0)]);
        assert_eq!(text, "Id: 7\nTags:\n - LibraryUpdate\n - Manual\nState: SUCCEEDED\n\n");
    }

    #[test]
    fn an_enqueued_job_adds_its_next_run_and_the_attempt_number() {
        let text = construct_string(&[job(1, JobState::Enqueued, Some(0), 2)]);
        assert!(text.contains("State: ENQUEUED\nNext scheduled run: "));
        assert!(text.ends_with("Attempt #3\n\n"));
        let none = construct_string(&[job(1, JobState::Enqueued, None, 0)]);
        assert!(!none.contains("Next scheduled run"));
        assert!(none.ends_with("Attempt #1\n\n"));
    }

    #[test]
    fn jobs_land_in_their_section_and_blocked_ones_in_none() {
        let jobs = [
            job(1, JobState::Enqueued, None, 0),
            job(2, JobState::Running, None, 0),
            job(3, JobState::Failed, None, 0),
            job(4, JobState::Cancelled, None, 0),
            job(5, JobState::Blocked, None, 0),
        ];
        let [enqueued, finished, running] = sections(&jobs);
        assert!(enqueued.contains("Id: 1") && !enqueued.contains("Id: 2"));
        assert!(finished.contains("Id: 3") && finished.contains("Id: 4"));
        assert!(running.contains("Id: 2"));
        assert!([&enqueued, &finished, &running].iter().all(|s| !s.contains("Id: 5")));
    }

    #[test]
    fn the_copy_text_joins_the_three_dumps() {
        let dumps = ["-\n".to_owned(), "-\n".to_owned(), "-\n".to_owned()];
        assert_eq!(copy_text(&dumps), "-\n-\n-\n");
    }
}
