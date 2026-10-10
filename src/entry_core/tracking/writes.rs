//! Detached remote writes of the track sheet (T-38b-031, T-38b-033; behaviour 8.2 to 8.6, E-TRK-1, E-TRK-7, E-TRK-15).
//! Each picker closes first and then runs its write detached, so the write survives the sheet closing. A write that
//! fails keeps the local row as it was and names the service in the banner (proposal: the upstream code is silent).

/// One remote edit of a bound row, from a picker or a toggle.
#[derive(Clone, Debug, PartialEq)]
pub enum Edit {
    Status(i64),
    /// The chapter or episode number, already truncated.
    Progress(i64),
    /// The score label chosen in the list.
    Score(String),
    /// Epoch milliseconds of local midnight, or `0` to remove the date (E-DATE-3).
    StartDate(i64),
    FinishDate(i64),
    Private(bool),
}

/// The banner of a write that failed: it names the service that was captured when the write started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteFailure {
    pub service: String,
    pub reason: String,
}

pub fn write_failure(service: &str, reason: &str) -> WriteFailure {
    WriteFailure { service: service.to_owned(), reason: reason.to_owned() }
}

/// The private toggle needs a bound row and a tracker that supports private tracking. It never runs without a track
/// (E-TRK-1, `no_track_hides_private_toggle`).
pub fn private_toggle_allowed(bound: bool, supports_private: bool) -> bool {
    bound && supports_private
}

/// Applies a write to a fresh copy of the row, read after the picker closed (`T-TRK-11`, D-1). The local row changes
/// only when the remote write succeeded (E-TRK-15): on failure the caller gets the banner and the old row stays.
pub fn commit<T: Clone>(
    service: &str,
    fresh: &T,
    apply: impl FnOnce(&mut T),
    remote: Result<(), String>,
) -> Result<T, WriteFailure> {
    match remote {
        Ok(()) => {
            let mut row = fresh.clone();
            apply(&mut row);
            Ok(row)
        }
        Err(reason) => Err(write_failure(service, &reason)),
    }
}

/// The date a remove-date confirm stores: `0`, which clears the date (E-DATE-3, behaviour 8.6).
pub const REMOVE_DATE: i64 = 0;

/// The date an edit stores for a picked day: its local midnight in `zone` (see `dates::stored_ms`).
pub fn date_edit(is_start: bool, stored_ms: i64) -> Edit {
    if is_start { Edit::StartDate(stored_ms) } else { Edit::FinishDate(stored_ms) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Row {
        progress: i64,
    }

    #[test]
    fn a_failed_write_keeps_the_local_row_and_names_the_service() {
        let fresh = Row { progress: 3 };
        let failed = commit("AniList", &fresh, |r| r.progress = 4, Err("timeout".into()));
        assert_eq!(
            failed,
            Err(WriteFailure { service: "AniList".into(), reason: "timeout".into() }),
            "the banner names the service"
        );
        let ok = commit("AniList", &fresh, |r| r.progress = 4, Ok(()));
        assert_eq!(ok, Ok(Row { progress: 4 }), "a successful write changes the local row");
        assert_eq!(fresh.progress, 3, "the fresh copy is only read");
    }

    #[test]
    fn toggle_private_never_runs_without_a_track() {
        assert!(!private_toggle_allowed(false, true), "no track, no toggle (E-TRK-1)");
        assert!(!private_toggle_allowed(true, false), "a tracker without private tracking has no toggle");
        assert!(private_toggle_allowed(true, true));
    }

    #[test]
    fn the_remove_date_confirm_names_the_tracker_and_the_date() {
        let start = kuro_track::picker::DateRemoval { tracker: "AniList".into(), is_start: true };
        assert_eq!(start.text(), "This will remove your previously selected start date from AniList");
        let finish = kuro_track::picker::DateRemoval { tracker: "Kitsu".into(), is_start: false };
        assert!(finish.text().contains("finish date from Kitsu"));
    }

    #[test]
    fn remove_date_writes_zero_and_is_offered_only_for_a_set_date() {
        assert_eq!(REMOVE_DATE, 0);
        assert!(!kuro_track::picker::remove_offered(0), "an unset date has no remove row");
        assert!(kuro_track::picker::remove_offered(1_700_000_000_000));
        assert_eq!(date_edit(true, 0), Edit::StartDate(0));
        assert_eq!(date_edit(false, 5), Edit::FinishDate(5));
    }
}
