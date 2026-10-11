//! The manual desktop checklist for notices, reveal and single instance (T-53a-076).
//!
//! These checks need a real desktop session, so no test can run them. Each row names the steps, the expected result and
//! the desktop environments to run them on. The `result` column is filled in by hand when a row has been run; a row that
//! has not been run reads `NotRun`.

/// Where a row was run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Desktop {
    KdePlasma,
    Gnome,
}

/// The outcome of a manual run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    NotRun,
    Pass,
    Fail,
}

/// One manual check.
#[derive(Clone, Copy, Debug)]
pub struct Check {
    pub id: &'static str,
    pub steps: &'static str,
    pub expected: &'static str,
    pub desktops: &'static [Desktop],
    pub result: Outcome,
}

const BOTH: &[Desktop] = &[Desktop::KdePlasma, Desktop::Gnome];

/// The five manual rows of the test plan (`p1-edge-cases-and-tests.md` section 3).
pub const CHECKLIST: [Check; 5] = [
    Check {
        id: "TC-53-10",
        steps: "Post two notices from a desktop session, then act on one of them.",
        expected: "One notice is withdrawn and the other stays; no group summary is used.",
        desktops: BOTH,
        result: Outcome::NotRun,
    },
    Check {
        id: "TC-53-33",
        steps: "With the app running, start `kurobayomi --action=show-updates`.",
        expected: "No second window opens; the running window shows the Updates tab.",
        desktops: BOTH,
        result: Outcome::NotRun,
    },
    Check {
        id: "TC-53-40",
        steps: "Close the app, then click a notice that was posted before it closed.",
        expected: "The app starts and its window opens on the notice's target.",
        desktops: BOTH,
        result: Outcome::NotRun,
    },
    Check {
        id: "TC-53-41",
        steps: "Use Show in folder on a real downloaded file.",
        expected: "The file manager opens at the file and highlights it.",
        desktops: BOTH,
        result: Outcome::NotRun,
    },
    Check {
        id: "TC-53-42",
        steps: "Open and close the reader and the web view, with the desktop animation setting on and then off.",
        expected: "With animations on the page slides for 250 ms; with them off it switches at once.",
        desktops: BOTH,
        result: Outcome::NotRun,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_five_manual_rows_are_listed_once_each() {
        let ids: Vec<&str> = CHECKLIST.iter().map(|c| c.id).collect();
        assert_eq!(ids, ["TC-53-10", "TC-53-33", "TC-53-40", "TC-53-41", "TC-53-42"]);
    }

    #[test]
    fn every_row_names_both_desktops_and_has_steps_and_an_expectation() {
        for check in CHECKLIST {
            assert_eq!(check.desktops, [Desktop::KdePlasma, Desktop::Gnome], "{}", check.id);
            assert!(!check.steps.is_empty() && !check.expected.is_empty(), "{}", check.id);
        }
    }

    #[test]
    fn a_row_that_was_not_run_is_not_recorded_as_a_pass() {
        assert!(CHECKLIST.iter().all(|c| c.result == Outcome::NotRun));
        assert_ne!(Outcome::Pass, Outcome::Fail);
    }
}
