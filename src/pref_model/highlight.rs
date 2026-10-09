//! Search-jump highlight: a one-shot request for a row title, the lookup that finds it on the
//! page, the scroll reveal, and the three-stage flash (delay, 10 pulses, clear).

use std::cell::RefCell;
use std::collections::HashMap;
use std::time::Duration;

use gtk::prelude::*;
use gtk::glib;

use super::widgets::{FLASH_DELAY_MS, FLASH_STEP_MS, FLASH_TOGGLES, FLASH_TOTAL_MS, HIGHLIGHT_SCROLL_DELAY_MS};

thread_local! {
    static PENDING: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Asks the next settings page to reveal and flash the row titled `title`.
pub fn request(title: impl Into<String>) {
    PENDING.with(|p| *p.borrow_mut() = Some(title.into()));
}

/// Takes the pending request. Clears it even when no row matches, so a stale request never fires later.
pub fn take() -> Option<String> {
    PENDING.with(|p| p.borrow_mut().take())
}

/// Index of the first title equal to `key`. A blank key never matches (a tracker row has an empty
/// title, and a blank highlight must not flash it). Disabled rows are the caller's to include.
pub fn first_match<'a>(titles: impl IntoIterator<Item = &'a str>, key: &str) -> Option<usize> {
    if key.trim().is_empty() {
        return None;
    }
    titles
        .into_iter()
        .position(|title| !title.trim().is_empty() && title == key)
}

/// The flash steps as `(delay_ms, class change)`, in order. Pure, so the timeline is testable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlashStep {
    On,
    Pulse,
    Off,
}

pub fn flash_schedule() -> Vec<(u64, FlashStep)> {
    let mut steps = vec![(0, FlashStep::On)];
    for i in 0..u64::from(FLASH_TOGGLES) {
        steps.push((FLASH_DELAY_MS + i * FLASH_STEP_MS, FlashStep::Pulse));
    }
    steps.push((FLASH_TOTAL_MS, FlashStep::Off));
    steps
}

fn key_of(row: &gtk::ListBoxRow) -> usize {
    row.as_ptr() as usize
}

/// CSS class access for one flashing row. The GTK row implements it; tests use a set of names.
pub trait Classes {
    fn has(&self, class: &str) -> bool;
    fn add(&mut self, class: &str);
    fn remove(&mut self, class: &str);
}

/// Latest flash generation per row key. A step whose generation is no longer the latest does nothing.
#[derive(Default)]
pub struct FlashState {
    generations: HashMap<usize, u64>,
}

impl FlashState {
    /// Starts a flash on `key` and returns its generation; every earlier flash on `key` is now stale.
    pub fn begin(&mut self, key: usize) -> u64 {
        let next = self.generations.get(&key).copied().unwrap_or(0) + 1;
        self.generations.insert(key, next);
        next
    }

    pub fn is_current(&self, key: usize, generation: u64) -> bool {
        self.generations.get(&key).copied() == Some(generation)
    }

    /// Runs one step if its flash is still current. Returns whether it ran (and so, for a pulse, toggled).
    pub fn tick(&mut self, key: usize, generation: u64, step: FlashStep, classes: &mut impl Classes) -> bool {
        if !self.is_current(key, generation) {
            return false;
        }
        match step {
            FlashStep::On => classes.add("pref-flash"),
            FlashStep::Pulse => {
                if classes.has("pref-flash-off") {
                    classes.remove("pref-flash-off");
                } else {
                    classes.add("pref-flash-off");
                }
            }
            FlashStep::Off => {
                classes.remove("pref-flash");
                classes.remove("pref-flash-off");
                self.generations.remove(&key);
            }
        }
        true
    }
}

impl Classes for gtk::ListBoxRow {
    fn has(&self, class: &str) -> bool {
        self.has_css_class(class)
    }
    fn add(&mut self, class: &str) {
        self.add_css_class(class);
    }
    fn remove(&mut self, class: &str) {
        self.remove_css_class(class);
    }
}

thread_local! {
    static FLASHES: RefCell<FlashState> = RefCell::new(FlashState::default());
}

/// Starts the flash on `row`. A second call on the same row cancels the first one's pending steps.
pub fn flash(row: &gtk::ListBoxRow) {
    let key = key_of(row);
    let generation = FLASHES.with(|f| f.borrow_mut().begin(key));
    for (delay, step) in flash_schedule() {
        let mut row = row.clone();
        glib::timeout_add_local_once(Duration::from_millis(delay), move || {
            FLASHES.with(|f| f.borrow_mut().tick(key, generation, step, &mut row));
        });
    }
}

/// Scrolls `scrolled` so that `row` is at the top, then flashes it. Runs once, after the page has laid out.
/// A row that is not mapped (inside a collapsed item) is not flashed.
pub fn reveal(scrolled: &gtk::ScrolledWindow, row: &gtk::ListBoxRow) {
    let scrolled = scrolled.clone();
    let row = row.clone();
    glib::timeout_add_local_once(Duration::from_millis(HIGHLIGHT_SCROLL_DELAY_MS), move || {
        if !row.is_mapped() {
            return;
        }
        if let Some(point) = row.compute_point(&scrolled, &gtk::graphene::Point::new(0.0, 0.0)) {
            let adj = scrolled.vadjustment();
            adj.set_value(adj.value() + f64::from(point.y()));
        }
        flash(&row);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Fake(std::collections::BTreeSet<String>);

    impl Classes for Fake {
        fn has(&self, class: &str) -> bool {
            self.0.contains(class)
        }
        fn add(&mut self, class: &str) {
            self.0.insert(class.to_owned());
        }
        fn remove(&mut self, class: &str) {
            self.0.remove(class);
        }
    }

    /// Plays every step of the given flashes in time order and returns the pulse toggles that ran.
    fn play(state: &mut FlashState, flashes: &[(u64, u64)], row: &mut Fake) -> u32 {
        let mut timeline: Vec<(u64, usize, u64, FlashStep)> = Vec::new();
        for (index, (start, generation)) in flashes.iter().enumerate() {
            for (delay, step) in flash_schedule() {
                timeline.push((start + delay, index, *generation, step));
            }
        }
        timeline.sort_by_key(|(at, index, ..)| (*at, *index));
        let mut toggles = 0;
        for (_, _, generation, step) in timeline {
            if state.tick(1, generation, step, row) && step == FlashStep::Pulse {
                toggles += 1;
            }
        }
        toggles
    }

    #[test]
    fn one_flash_toggles_ten_times_and_leaves_no_classes() {
        let mut state = FlashState::default();
        let mut row = Fake::default();
        let generation = state.begin(1);
        assert_eq!(play(&mut state, &[(0, generation)], &mut row), 10);
        assert!(row.0.is_empty());
    }

    #[test]
    fn a_second_flash_cancels_the_first_ones_pending_steps() {
        let mut state = FlashState::default();
        let mut row = Fake::default();
        let first = state.begin(1);
        let second = state.begin(1);
        // The first flash would have started 1000 ms earlier; none of its steps may run.
        assert_eq!(play(&mut state, &[(0, first), (1000, second)], &mut row), 10);
        assert!(row.0.is_empty());
        assert!(!state.is_current(1, first));
    }

    #[test]
    fn flashes_on_other_rows_do_not_cancel_each_other() {
        let mut state = FlashState::default();
        let a = state.begin(1);
        let b = state.begin(2);
        assert!(state.is_current(1, a) && state.is_current(2, b));
    }

    #[test]
    fn take_clears_the_request_even_when_nothing_matches() {
        request("Backup and restore");
        assert_eq!(take().as_deref(), Some("Backup and restore"));
        assert_eq!(take(), None);
    }

    #[test]
    fn a_stale_request_is_gone_after_a_page_takes_it() {
        request("Old row");
        let _ = take();
        assert_eq!(take(), None);
    }

    #[test]
    fn duplicate_titles_resolve_to_the_first_row() {
        let titles = ["Crop borders", "Other", "Crop borders"];
        assert_eq!(first_match(titles, "Crop borders"), Some(0));
    }

    #[test]
    fn a_blank_key_or_blank_title_never_matches() {
        assert_eq!(first_match(["", "Tracker"], ""), None);
        assert_eq!(first_match(["", "  "], "  "), None);
        assert_eq!(first_match(["", "Tracker"], "Tracker"), Some(1));
    }

    #[test]
    fn target_after_a_disabled_group_is_found_by_title() {
        // A disabled group is not built, so its titles are absent from the list entirely.
        let built = ["Appearance", "Theme"];
        assert_eq!(first_match(built, "Theme"), Some(1));
        assert_eq!(first_match(built, "Hidden"), None);
    }

    #[test]
    fn flash_timeline_is_on_then_ten_pulses_then_off() {
        let steps = flash_schedule();
        assert_eq!(steps.first(), Some(&(0, FlashStep::On)));
        assert_eq!(steps.last(), Some(&(3000, FlashStep::Off)));
        let pulses: Vec<u64> = steps.iter().filter(|s| s.1 == FlashStep::Pulse).map(|s| s.0).collect();
        assert_eq!(pulses.len(), 10);
        assert_eq!(pulses.first(), Some(&600));
        assert_eq!(pulses.last(), Some(&2400));
        assert!(pulses.windows(2).all(|w| w[1] - w[0] == 200));
    }
}
