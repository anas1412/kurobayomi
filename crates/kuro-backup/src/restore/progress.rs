//! Progress amounts and step events for a restore (area 22, T-22a-006). The amount is the sum of the plan, so the
//! last event always reaches the amount (X-72, X-73) and a zero amount reads as complete (E-72).

use super::options::RestoreOptions;
use super::sections::{Counts, plan};

/// One progress step: the label of the item just finished, and the counts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProgressEvent {
    pub title: String,
    pub done: u32,
    pub total: u32,
}

/// The fixed total for these options and counts.
pub fn amount(opts: RestoreOptions, counts: Counts) -> u32 {
    plan(opts, counts).iter().map(|(_, items)| items).sum()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    total: u32,
    done: u32,
}

impl Progress {
    pub fn new(total: u32) -> Self {
        Self { total, done: 0 }
    }

    pub fn total(&self) -> u32 {
        self.total
    }

    pub fn done(&self) -> u32 {
        self.done
    }

    /// Counts one finished step.
    pub fn advance(&mut self, title: &str) -> ProgressEvent {
        self.done = self.done.saturating_add(1);
        ProgressEvent { title: title.to_owned(), done: self.done, total: self.total }
    }

    /// Fraction for the bar. A zero total is complete (1.0), never a division by zero.
    pub fn fraction(&self) -> f64 {
        if self.total == 0 { 1.0 } else { f64::from(self.done) / f64::from(self.total) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_on() -> RestoreOptions {
        RestoreOptions { extensions: true, ..RestoreOptions::default() }
    }

    fn counts() -> Counts {
        Counts { anime_entries: 4, manga_entries: 5, anime_stores: 2, manga_repos: 1 }
    }

    #[test]
    fn amount_follows_the_behaviour_formula_with_d2_fix() {
        // library 4+5, categories 2, app 1, source 1, stores 2+1, buttons 1, extensions 1
        assert_eq!(amount(all_on(), counts()), 9 + 2 + 1 + 1 + 3 + 1 + 1);
        let none = RestoreOptions { library_entries: false, categories: false, app_settings: false, extension_stores: false, custom_buttons: false, source_settings: false, extensions: false };
        assert_eq!(amount(none, counts()), 0);
    }

    #[test]
    fn seasons_are_in_the_entry_counts() {
        // Seasons are separate anime rows, so the caller counts them into anime_entries.
        let base = counts();
        let with_seasons = Counts { anime_entries: base.anime_entries + 2, ..base };
        assert_eq!(amount(RestoreOptions::default(), with_seasons), amount(RestoreOptions::default(), base) + 2);
    }

    #[test]
    fn zero_amount_is_complete_without_division() {
        let p = Progress::new(0);
        assert_eq!(p.fraction(), 1.0);
    }

    #[test]
    fn advance_emits_label_and_counts() {
        let mut p = Progress::new(2);
        assert_eq!(p.fraction(), 0.0);
        let e = p.advance("Categories");
        assert_eq!(e, ProgressEvent { title: "Categories".into(), done: 1, total: 2 });
        assert_eq!(p.fraction(), 0.5);
        p.advance("App settings");
        assert_eq!(p.done(), p.total());
        assert_eq!(p.fraction(), 1.0);
    }
}
