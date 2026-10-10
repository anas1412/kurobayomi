//! Registry constructor for the MangaUpdates and Simkl services (T-32a-001).

use std::sync::Arc;

use kuro_platform::secrets::SecretStore;

use crate::error::TrackError;
use crate::events::EventBus;
use crate::mangaupdates::MangaUpdatesTracker;
use crate::simkl::SimklTracker;
use crate::tracker::Tracker;

use super::secret_migration::{Migration, migrate_legacy_tracker_secrets};

/// The two services and what the first-start secret move did (T-32a-065).
pub struct Started {
    pub trackers: Vec<Arc<dyn Tracker>>,
    /// `migration.warning()` is the text to show in the tracking settings when the secret store is unreachable.
    pub migration: Migration,
}

/// Builds the two services of this area in registry order, MangaUpdates before Simkl (`TrackerManager.trackers`).
/// The shared secrets and events are the only inputs the two services need; the caller adds the result to the other
/// services and hands the whole list to `TrackerManager::new`.
pub fn all_trackers(
    secrets: Arc<dyn SecretStore>,
    events: EventBus,
) -> Result<Vec<Arc<dyn Tracker>>, TrackError> {
    Ok(start_trackers(secrets, events)?.trackers)
}

/// [`all_trackers`], after moving the old credential entries into the tracker layout, and with the outcome of that move.
/// The move runs first because a tracker reads its login state when it is created. A failed or impossible move never
/// stops the trackers from starting: they start logged out when nothing was moved.
pub fn start_trackers(
    secrets: Arc<dyn SecretStore>,
    events: EventBus,
) -> Result<Started, TrackError> {
    let migration = migrate_legacy_tracker_secrets(secrets.as_ref());
    match &migration {
        Migration::Moved(0) => {}
        Migration::Moved(n) => tracing::info!("moved {n} tracker credentials to the secret store layout"),
        Migration::StoreUnavailable => tracing::warn!("secret store unavailable: tracker credentials were not moved"),
        Migration::Failed(e) => tracing::warn!("moving tracker credentials failed: {e}"),
    }
    let trackers: Vec<Arc<dyn Tracker>> = vec![
        Arc::new(MangaUpdatesTracker::new(
            Arc::clone(&secrets),
            events.clone(),
        )?),
        Arc::new(SimklTracker::new(secrets, events)?),
    ];
    Ok(Started { trackers, migration })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fakes::MemSecrets;
    use crate::model::TrackerId;

    #[test]
    fn t_32a_001_both_services_come_back_in_registry_order() {
        let secrets: Arc<dyn SecretStore> = MemSecrets::new();
        let got = all_trackers(secrets, EventBus::default()).unwrap();
        let ids: Vec<TrackerId> = got.iter().map(|t| t.id()).collect();
        assert_eq!(ids, [TrackerId::MANGAUPDATES, TrackerId::SIMKL]);
    }
}
