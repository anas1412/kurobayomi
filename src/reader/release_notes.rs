//! Release notes for the reader's intentional deviations from the Kotlin build (T-43a-090).
//!
//! The table lists every edge case of `plans/43-reader-core/p1-edge-cases-and-tests.md` that is marked Fix, Drop or Open,
//! plus the four the task names that the plan keeps with a Linux fix note. The test checks that each id is listed once
//! and that the ids the task names are all present.

/// What the reader does about the edge case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Changed on purpose; the Kotlin behaviour was a bug or does not fit the desktop.
    Fixed,
    /// Removed on purpose: the platform cannot do it.
    Dropped,
    /// Not decided upstream or in this port; behaviour is documented and may change.
    Open,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deviation {
    pub id: &'static str,
    pub kind: Kind,
    pub note: &'static str,
}

pub const DEVIATIONS: &[Deviation] = &[
    Deviation { id: "E-LEAK-1", kind: Kind::Fixed, note: "A failed page list no longer leaves an old loader in place on retry." },
    Deviation { id: "E-LEAK-2", kind: Kind::Fixed, note: "Preloading a downloaded chapter no longer orphans the online loader." },
    Deviation { id: "E-CANCEL-1", kind: Kind::Fixed, note: "A cancelled load is not stored as an error." },
    Deviation { id: "E-TR-1", kind: Kind::Fixed, note: "Chapter transitions compare and hash by value." },
    Deviation { id: "E-HTTP-5", kind: Kind::Fixed, note: "The page list is saved after it is fetched, not only when the chapter is recycled." },
    Deviation { id: "E-HTTP-7", kind: Kind::Fixed, note: "The page worker starts only after the first page list is loaded." },
    Deviation { id: "E-SORT-1", kind: Kind::Fixed, note: "Chapter sorting is documented as natural order, which can differ from the Kotlin collator." },
    Deviation { id: "E-SORT-3", kind: Kind::Fixed, note: "An invalid sort mode falls back to the default instead of throwing." },
    Deviation { id: "E-LIST-2", kind: Kind::Fixed, note: "Downloaded-only mode keeps the requested chapter and shows a card when it is missing." },
    Deviation { id: "E-INIT-1", kind: Kind::Fixed, note: "A failed first load shows an error in the reader instead of closing it." },
    Deviation { id: "E-NAV-1", kind: Kind::Fixed, note: "A jump to page 0 after a failed neighbour load keeps the current chapter." },
    Deviation { id: "E-EVT-1", kind: Kind::Fixed, note: "Viewer events are queued, so none are dropped by a rendezvous channel." },
    Deviation { id: "E-DEL-1", kind: Kind::Fixed, note: "Remove-after-read accepts only -1 and the documented slot values; others are treated as -1." },
    Deviation { id: "E-HIST-1", kind: Kind::Fixed, note: "Consecutive history flushes write one row, not duplicates." },
    Deviation { id: "E-HIST-2", kind: Kind::Fixed, note: "The read-start timestamp is taken once, without a race." },
    Deviation { id: "E-INC-1", kind: Kind::Fixed, note: "Incognito blocks the reader's state keys as well as its writes." },
    Deviation { id: "E-SHARE-1", kind: Kind::Fixed, note: "Sharing a page reports failure through the toast instead of a dead error handler." },
    Deviation { id: "E-COVER-2", kind: Kind::Fixed, note: "Cover loading catches every error, not only exceptions." },
    Deviation { id: "E-IMG-3", kind: Kind::Fixed, note: "A page stream evicted from the cache is read again instead of failing." },
    Deviation { id: "E-DL-1", kind: Kind::Fixed, note: "A downloaded directory page with a missing file shows an error card on Linux." },
    Deviation { id: "E-UI-1", kind: Kind::Fixed, note: "The current page is shown as 1 before the first page, not -1." },
    Deviation { id: "E-UI-2", kind: Kind::Fixed, note: "Blocking work runs off the UI thread." },
    Deviation { id: "E-UI-3", kind: Kind::Fixed, note: "The split and invert bindings in the paged settings are applied to the right setting." },
    Deviation { id: "E-UI-5", kind: Kind::Fixed, note: "The page-actions sheet closes when the chapter changes." },
    Deviation { id: "E-UI-7", kind: Kind::Fixed, note: "Icons are mirrored for right-to-left reading." },
    Deviation { id: "E-FLASH-1", kind: Kind::Fixed, note: "The page flash counter resets when the feature is turned off." },
    Deviation { id: "E-BRIGHT-1", kind: Kind::Fixed, note: "Custom brightness has one value subscription, removed when the feature is off." },
    Deviation { id: "E-BRIGHT-2", kind: Kind::Fixed, note: "A positive custom brightness has no effect on Linux; the reader says so." },
    Deviation { id: "E-STATE-1", kind: Kind::Fixed, note: "State updates no longer run side effects inside the update closure." },
    Deviation { id: "E-CONC-1", kind: Kind::Fixed, note: "Background work is cancelled when its page goes away." },
    Deviation { id: "E-CONC-2", kind: Kind::Fixed, note: "Decoded image size is capped by the memory budget." },
    Deviation { id: "E-ORI-1", kind: Kind::Dropped, note: "Orientation is stored and synced but no rotation is applied; the rotation sheet says so." },
    Deviation { id: "E-SAVE-2", kind: Kind::Open, note: "A saved page whose file name is taken gets a suffix such as (2). The upstream collision rule was not read." },
    Deviation { id: "E-TRACK-1", kind: Kind::Open, note: "Track auto-update checks only the autoUpdateTrack setting. The intended rule is still open." },
];

/// The ids the release notes must list (T-43a-090, first list).
pub const REQUIRED_IDS: &[&str] = &[
    "E-LEAK-1", "E-LEAK-2", "E-CANCEL-1", "E-TR-1", "E-HTTP-5", "E-HTTP-7", "E-SORT-3", "E-LIST-2", "E-NAV-1", "E-EVT-1", "E-DEL-1",
    "E-HIST-1", "E-HIST-2", "E-INC-1", "E-SHARE-1", "E-COVER-2", "E-IMG-3", "E-DL-1", "E-UI-2", "E-UI-3", "E-UI-5", "E-UI-7", "E-FLASH-1",
    "E-BRIGHT-1", "E-STATE-1", "E-ORI-1", "E-SAVE-2", "E-TRACK-1",
];

/// The entry for an id, when the notes list it.
pub fn find(id: &str) -> Option<&'static Deviation> {
    DEVIATIONS.iter().find(|d| d.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_required_id_is_listed_once() {
        for id in REQUIRED_IDS {
            let count = DEVIATIONS.iter().filter(|d| d.id == *id).count();
            assert_eq!(count, 1, "{id} must be listed exactly once");
        }
    }

    #[test]
    fn no_id_is_listed_twice() {
        let mut ids: Vec<&str> = DEVIATIONS.iter().map(|d| d.id).collect();
        ids.sort_unstable();
        let before = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), before);
    }

    #[test]
    fn drops_and_open_questions_are_marked_as_such() {
        assert_eq!(find("E-ORI-1").map(|d| d.kind), Some(Kind::Dropped));
        assert_eq!(find("E-SAVE-2").map(|d| d.kind), Some(Kind::Open));
        assert_eq!(find("E-TRACK-1").map(|d| d.kind), Some(Kind::Open));
        assert!(find("E-NOT-A-REAL-ID").is_none());
    }
}
