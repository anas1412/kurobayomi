//! What a finished episode does to the downloads: delete-after-watching (`remove_after_read_slots`), the rule the reader
//! follows for chapters. The episode `slots` positions back in the playback list goes to the download service's
//! pending deletes when it is seen; the service deletes them when the player closes (and after a crash, at the next
//! flush). A negative slot count turns it off.

use kuro_downloads::unit::UnitRef;
use kuro_platform::kind::Kind;
use kuro_domain::download::prefs::DownloadPrefs;

use super::neighbours::{downloaded_ids, playback_list};
use crate::downloads::directory::AppDirectory;

/// Queues the episode to delete after `episode_id` was seen, when the rule is on and that episode is seen too.
pub fn run(episode_id: i64) {
    if !crate::data::is_installed() {
        return;
    }
    let app = crate::data::app();
    let slots = DownloadPrefs::new(app.base.store()).remove_after_read_slots.get();
    let Ok(Some((list, at))) = playback_list(&app.db, episode_id, false, &downloaded_ids(&app.db, episode_id)) else { return };
    let Some(target) = kuro_domain::download::policy::remove_after_read_target(&list, at, slots) else { return };
    if !target.done {
        return;
    }
    let Some(service) = crate::shell::host::Shell::get().and_then(|shell| shell.download_service()) else { return };
    let Some((entry, _)) = AppDirectory::new(app.db.clone()).entry_context(Kind::Anime, target.entry_id) else { return };
    if let Err(error) = service.defer_delete(&entry, &[UnitRef::from(target)]) {
        tracing::warn!(episode_id = target.id, %error, "a watched episode could not be queued for deletion");
    }
}

/// Deletes what [`run`] queued; called when the player closes.
pub fn flush() {
    let Some(shell) = crate::shell::host::Shell::get() else { return };
    let Some(service) = shell.download_service() else { return };
    shell.runtime().spawn(async move {
        if let Err(error) = service.flush_pending_deletes().await {
            tracing::warn!(%error, "the watched episodes could not be deleted");
        }
    });
}
