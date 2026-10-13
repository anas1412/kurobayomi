//! What the tracking rules need to know about one tracker, read off the tracker once. The rules in this folder take
//! these plain values, so they can be tested without a live tracker; [`facts_of`] is the only function that calls a
//! tracker. Upstream: `TrackerManager` and the `mapToTrackItem` rules of the track sheet (behaviour 3.4, 2.8).

use kuro_platform::kind::Kind;
use kuro_track::tracker::Tracker;

/// The facts of one tracker for one entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrackerFacts {
    pub id: i64,
    pub logged_in: bool,
    /// Always true for a plain tracker. An enhanced tracker is true only for the sources it accepts.
    pub accepts_source: bool,
    pub supports_private: bool,
    pub deletable: bool,
}

impl TrackerFacts {
    /// A tracker counts for an entry when it is logged in and accepts the entry's source (behaviour 2.8, 11.3).
    pub fn supported(&self) -> bool {
        self.logged_in && self.accepts_source
    }
}

/// The facts of `tracker` for an entry of `kind` from `source` (`None` for a stub source). `None` when the tracker
/// does not handle this kind.
pub fn facts_of(tracker: &dyn Tracker, kind: Kind, source: Option<&str>) -> Option<TrackerFacts> {
    let (handles_kind, accepts_source, deletable) = match kind {
        Kind::Anime => (
            tracker.as_anime().is_some(),
            tracker.as_enhanced_anime().is_none_or(|e| source.is_some_and(|s| e.accepts(s))),
            tracker.as_deletable_anime().is_some(),
        ),
        Kind::Manga => (
            tracker.as_manga().is_some(),
            tracker.as_enhanced_manga().is_none_or(|e| source.is_some_and(|s| e.accepts(s))),
            tracker.as_deletable_manga().is_some(),
        ),
    };
    handles_kind.then(|| TrackerFacts {
        id: tracker.id().0,
        logged_in: tracker.is_logged_in(),
        accepts_source,
        supports_private: tracker.supports_private_tracking(),
        deletable,
    })
}

/// One row of the tracking sheet: a supported tracker, and whether the entry has a binding to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Row {
    pub tracker_id: i64,
    /// `false` is the "Add tracking" row.
    pub bound: bool,
}

/// One row per supported tracker, in the order of `facts` (registry order). `bound` lists the tracker ids the entry
/// is bound to. A logged-out tracker or a refused enhanced source gets no row, and its binding is left as it is.
///
/// `map_track_items_one_row_per_logged_in_manga_tracker` and `enhanced_source_filter` cover this.
pub fn rows(facts: &[TrackerFacts], bound: &[i64]) -> Vec<Row> {
    facts
        .iter()
        .filter(|f| f.supported())
        .map(|f| Row { tracker_id: f.id, bound: bound.contains(&f.id) })
        .collect()
}

/// The tracking count and the logged-in flag of an entry, from the same filter as the sheet (`T-TRK-12`, D-10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Summary {
    /// Bindings of the entry whose tracker is supported.
    pub count: i64,
    /// Some supported tracker is logged in.
    pub has_logged_in: bool,
    /// The supported trackers, in registry order.
    pub accepted: Vec<i64>,
}

pub fn summarize(facts: &[TrackerFacts], bound: &[i64]) -> Summary {
    let rows = rows(facts, bound);
    Summary {
        count: rows.iter().filter(|r| r.bound).count() as i64,
        has_logged_in: !rows.is_empty(),
        accepted: rows.iter().map(|r| r.tracker_id).collect(),
    }
}

#[cfg(test)]
/// What the record of a bound row shows, read off the tracker and the record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordShown {
    /// The score list is not empty and the score is set.
    pub score: bool,
    pub start_date: bool,
    pub finish_date: bool,
}

#[cfg(test)]
/// Which actions a row offers (UI parity 4.1, E-TRK-1). An unbound row offers only "Add tracking"; a bound row never
/// reads a missing record, so every action on it is gated by the record it has.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowActions {
    pub add_tracking: bool,
    pub status: bool,
    pub progress: bool,
    pub score: bool,
    pub start_date: bool,
    pub finish_date: bool,
    pub private_toggle: bool,
    pub menu: bool,
}

#[cfg(test)]
/// `record` is `None` for an unbound row. The private toggle needs both a record and a tracker that supports it
/// (E-TRK-1).
pub fn row_actions(record: Option<RecordShown>, supports_private: bool) -> RowActions {
    match record {
        None => RowActions {
            add_tracking: true,
            status: false,
            progress: false,
            score: false,
            start_date: false,
            finish_date: false,
            private_toggle: false,
            menu: false,
        },
        Some(shown) => RowActions {
            add_tracking: false,
            status: true,
            progress: true,
            score: shown.score,
            start_date: shown.start_date,
            finish_date: shown.finish_date,
            private_toggle: supports_private,
            menu: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn f(id: i64, logged_in: bool, accepts_source: bool) -> TrackerFacts {
        TrackerFacts { id, logged_in, accepts_source, supports_private: false, deletable: false }
    }

    #[test]
    fn map_track_items_one_row_per_logged_in_manga_tracker() {
        let facts = [f(1, true, true), f(2, false, true), f(3, true, true)];
        let rows = rows(&facts, &[3]);
        assert_eq!(
            rows,
            vec![Row { tracker_id: 1, bound: false }, Row { tracker_id: 3, bound: true }],
            "a logged-out tracker has no row"
        );
    }

    #[test]
    fn enhanced_source_filter() {
        let facts = [f(101, true, false), f(2, true, true)];
        assert_eq!(rows(&facts, &[]), vec![Row { tracker_id: 2, bound: false }], "a refused enhanced source has no row");
        assert!(rows(&[f(101, true, false)], &[101]).is_empty(), "a bound row of a refused source is not shown");
    }

    #[test]
    fn summary_counts_bindings_of_supported_trackers_only() {
        let facts = [f(1, true, true), f(2, false, true), f(3, true, false)];
        let s = summarize(&facts, &[1, 2, 3]);
        assert_eq!(s, Summary { count: 1, has_logged_in: true, accepted: vec![1] });
        assert!(!summarize(&[f(2, false, true)], &[2]).has_logged_in, "no logged-in tracker");
    }

    #[test]
    fn no_track_hides_private_toggle() {
        let unbound = row_actions(None, true);
        assert!(unbound.add_tracking && !unbound.private_toggle && !unbound.menu);
        let bound_no_private = row_actions(Some(RecordShown { score: true, start_date: false, finish_date: false }), false);
        assert!(!bound_no_private.private_toggle, "a tracker without private tracking shows no toggle");
        assert!(row_actions(Some(RecordShown { score: false, start_date: false, finish_date: false }), true).private_toggle);
    }

    #[test]
    fn a_bound_row_gates_its_score_and_date_actions_on_the_record() {
        let a = row_actions(Some(RecordShown { score: false, start_date: true, finish_date: false }), true);
        assert!(!a.score && a.start_date && !a.finish_date && a.status && a.progress);
    }
}
