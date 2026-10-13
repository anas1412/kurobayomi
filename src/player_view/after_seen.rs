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

/// Download-ahead: past a quarter of a downloaded episode, the next one is queued for download (the reader's rule,
/// `reader::download_ahead`, with the playing position standing for the page). Called once per episode.
pub fn download_ahead(episode_id: i64, position_s: i64, duration_s: i64) {
    if !crate::data::is_installed() {
        return;
    }
    let app = crate::data::app();
    let on_disk = downloaded_ids(&app.db, episode_id);
    let Ok(Some((list, at))) = playback_list(&app.db, episode_id, false, &on_disk) else { return };
    let ids: Vec<i64> = list.iter().map(|episode| episode.id).collect();
    let queue = crate::reader::download_ahead::chapters_to_queue(
        crate::misc::diet_defaults::DOWNLOAD_AUTO_WHILE_AMOUNT,
        &ids,
        |index| on_disk.contains(&ids[index]),
        at,
        position_s.max(0) as usize,
        duration_s.max(0) as usize,
    );
    let Some(entry_id) = list.get(at).map(|episode| episode.entry_id) else { return };
    let host = crate::updates::download_host::current();
    if !queue.is_empty() && host.enqueue(Kind::Anime, entry_id, &queue) {
        host.start_queue(Kind::Anime);
    }
}
