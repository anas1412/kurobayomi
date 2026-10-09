//! Updates-feed domain (area 08 section 4): the feed repository, the three-month window, the tab state and the
//! new-item counters of the library preferences.

use kuro_prefs::library::LibraryPrefs;

use crate::kind::EntryKind;

pub mod model;
pub mod repo;
pub mod tab;
pub mod window;

/// Zeroes the new-items counter of one kind when the Updates tab opens or closes
/// (`library_unread_updates_count` for manga, `library_unseen_updates_count` for anime).
pub fn reset_new_counts(prefs: &LibraryPrefs, kind: EntryKind) {
    let stored = match kind {
        EntryKind::Manga => prefs.new_manga_updates_count.set(0),
        EntryKind::Anime => prefs.new_anime_updates_count.set(0),
    };
    if let Err(e) = stored {
        tracing::warn!("could not reset the new updates count: {e}");
    }
}
