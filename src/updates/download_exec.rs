//! The executor of the Updates download work (T-42a-031, T-42b-011): it runs the steps of a [`download_actions::plan`]
//! against the download backend, and removes the downloads of newly read units. Pure of GTK; the screen hands the work
//! to [`download_host::spawn_detached`], so it is not cancelled when the screen goes away.

use kuro_platform::kind::Kind;

use crate::updates::download_actions::{self, DownloadAction, EntryFacts, Plan, UnitTarget};
use crate::updates::download_host::DownloadBackend;

/// Runs each step in order. `external` is the anime "use external downloader" choice, passed to every enqueue.
pub fn run_plan(backend: &dyn DownloadBackend, kind: Kind, steps: &[Plan], external: bool) {
    for step in steps {
        match step {
            Plan::Enqueue { entry_id, unit_ids } => {
                backend.enqueue(kind, *entry_id, unit_ids, external);
            }
            Plan::StartQueue => backend.start_queue(kind),
            Plan::StartNow { unit_id } => backend.start_now(kind, *unit_id),
            Plan::CancelQueued { unit_id } => backend.cancel(kind, *unit_id),
            Plan::Delete { entry_id, unit_ids } => backend.delete(kind, *entry_id, unit_ids),
        }
    }
}

/// The plan of `action` over `rows`, then its run. `facts` answers per entry id.
pub fn dispatch(
    backend: &dyn DownloadBackend,
    kind: Kind,
    action: DownloadAction,
    rows: &[UnitTarget],
    facts: impl Fn(i64) -> Option<EntryFacts>,
    external: bool,
) {
    run_plan(backend, kind, &download_actions::plan(action, rows, facts), external);
}

/// With "remove after marked as read" on, deletes the downloads of the newly read units, grouped per entry (E-U33).
/// `newly_read` holds only the rows the read write changed. Returns the groups it deleted.
pub fn remove_read_downloads(backend: &dyn DownloadBackend, kind: Kind, enabled: bool, newly_read: &[UnitTarget]) -> Vec<(i64, Vec<i64>)> {
    if !enabled {
        return Vec::new();
    }
    let groups = download_actions::remove_after_read(newly_read);
    for (entry_id, unit_ids) in &groups {
        backend.delete(kind, *entry_id, unit_ids);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::updates::download_host::fake::{Call, Fake};
    use crate::updates::download_state::DownloadState;

    fn unit(entry_id: i64, unit_id: i64, state: DownloadState) -> UnitTarget {
        UnitTarget { entry_id, unit_id, state }
    }

    fn facts(_: i64) -> Option<EntryFacts> {
        Some(EntryFacts { in_library: true, source_installed: true })
    }

    #[test]
    fn download_start_enqueues_per_entry_with_the_external_choice() {
        let fake = Fake::installed();
        let rows = [unit(1, 10, DownloadState::NotDownloaded), unit(2, 20, DownloadState::NotDownloaded), unit(1, 11, DownloadState::NotDownloaded)];
        dispatch(&fake, Kind::Anime, DownloadAction::Start, &rows, facts, true);
        assert_eq!(
            fake.calls(),
            vec![
                Call::Enqueue { entry_id: 1, unit_ids: vec![10, 11], external: true },
                Call::Enqueue { entry_id: 2, unit_ids: vec![20], external: true },
            ]
        );
    }

    #[test]
    fn download_start_over_an_error_row_starts_the_queue_after_enqueue() {
        let fake = Fake::installed();
        dispatch(&fake, Kind::Manga, DownloadAction::Start, &[unit(1, 10, DownloadState::Error)], facts, false);
        assert_eq!(fake.calls(), vec![Call::Enqueue { entry_id: 1, unit_ids: vec![10], external: false }, Call::StartQueue]);
    }

    #[test]
    fn download_start_now_cancel_and_delete_reach_the_backend() {
        let fake = Fake::installed();
        dispatch(&fake, Kind::Manga, DownloadAction::StartNow, &[unit(1, 10, DownloadState::Queue)], facts, false);
        dispatch(&fake, Kind::Manga, DownloadAction::Cancel, &[unit(1, 10, DownloadState::Queue)], facts, false);
        dispatch(&fake, Kind::Manga, DownloadAction::Delete, &[unit(1, 10, DownloadState::Downloaded), unit(1, 12, DownloadState::Downloaded)], facts, false);
        assert_eq!(
            fake.calls(),
            vec![Call::StartNow(10), Call::Cancel(10), Call::Delete { entry_id: 1, unit_ids: vec![10, 12] }]
        );
    }

    #[test]
    fn download_delete_with_a_missing_source_does_nothing() {
        let fake = Fake::default();
        let gone = |_: i64| Some(EntryFacts { in_library: true, source_installed: false });
        dispatch(&fake, Kind::Manga, DownloadAction::Delete, &[unit(1, 10, DownloadState::Downloaded)], gone, false);
        assert!(fake.calls().is_empty());
    }

    #[test]
    fn download_remove_after_read_deletes_only_newly_read_downloads_per_entry() {
        let fake = Fake::installed();
        let newly_read = [unit(1, 10, DownloadState::Downloaded), unit(1, 11, DownloadState::NotDownloaded), unit(2, 20, DownloadState::Downloaded)];
        let groups = remove_read_downloads(&fake, Kind::Manga, true, &newly_read);
        assert_eq!(groups, vec![(1, vec![10]), (2, vec![20])]);
        assert_eq!(fake.calls(), vec![Call::Delete { entry_id: 1, unit_ids: vec![10] }, Call::Delete { entry_id: 2, unit_ids: vec![20] }]);
    }

    #[test]
    fn download_remove_after_read_does_nothing_with_the_preference_off() {
        let fake = Fake::installed();
        assert!(remove_read_downloads(&fake, Kind::Manga, false, &[unit(1, 10, DownloadState::Downloaded)]).is_empty());
        assert!(fake.calls().is_empty());
    }
}
