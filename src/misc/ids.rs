//! The notice id registry (T-35a-005): every desktop notice slot of the app has one id here, so two producers never
//! share a slot. The Android collision (`ID_APP_UPDATE_PROMPT` and `ID_DOWNLOAD_IMAGE` were both 2) is fixed by giving
//! `save-image` and `app-update.install` their own ids.
//!
//! Decision: the registry lives in `misc` because `src/notify` is not part of this lane; the ids are the strings of
//! p1-rust-design.md 3.2. A per-entry slot (one notice per manga or anime) is the base id plus `:` and the entry id.

use kuro_domain::kind::EntryKind;

pub const APP_UPDATE_PROGRESS: &str = "app-update.progress";
pub const APP_UPDATE_INSTALL: &str = "app-update.install";
pub const APP_UPDATE_ERROR: &str = "app-update.error";
pub const SAVE_IMAGE: &str = "save-image";
pub const LIBRARY_PROGRESS: &str = "library.progress";
pub const LIBRARY_ERROR: &str = "library.error";
pub const LIBRARY_SIZE_WARNING: &str = "library.size-warning";
pub const DOWNLOAD_MANGA_PROGRESS: &str = "download.manga.progress";
pub const DOWNLOAD_MANGA_ERROR: &str = "download.manga.error";
pub const DOWNLOAD_ANIME_PROGRESS: &str = "download.anime.progress";
pub const DOWNLOAD_ANIME_ERROR: &str = "download.anime.error";
pub const NEW_CHAPTERS: &str = "new.chapters";
pub const NEW_EPISODES: &str = "new.episodes";
// One id source for backup and restore (T-35a-082): the slots the backup crate posts to are the registry's ids, so the
// router's withdraw of a restore and the restore's own notice name the same slot.
pub const BACKUP_PROGRESS: &str = kuro_backup::notify::ID_BACKUP_PROGRESS;
pub const BACKUP_COMPLETE: &str = kuro_backup::notify::ID_BACKUP_RESULT;
pub const RESTORE_PROGRESS: &str = kuro_backup::notify::ID_RESTORE_PROGRESS;
pub const RESTORE_COMPLETE: &str = kuro_backup::notify::ID_RESTORE_RESULT;
pub const INCOGNITO: &str = "incognito";
pub const SERVER_TORRENT: &str = "server.torrent";
pub const SERVER_HTTP: &str = "server.http";
pub const EXTENSIONS_UPDATES: &str = "extensions.updates";
pub const EXTENSIONS_INSTALLING: &str = "extensions.installing";

/// Every base id, for the duplicate test.
pub const ALL_IDS: &[&str] = &[
    APP_UPDATE_PROGRESS,
    APP_UPDATE_INSTALL,
    APP_UPDATE_ERROR,
    SAVE_IMAGE,
    LIBRARY_PROGRESS,
    LIBRARY_ERROR,
    LIBRARY_SIZE_WARNING,
    DOWNLOAD_MANGA_PROGRESS,
    DOWNLOAD_MANGA_ERROR,
    DOWNLOAD_ANIME_PROGRESS,
    DOWNLOAD_ANIME_ERROR,
    NEW_CHAPTERS,
    NEW_EPISODES,
    BACKUP_PROGRESS,
    BACKUP_COMPLETE,
    RESTORE_PROGRESS,
    RESTORE_COMPLETE,
    INCOGNITO,
    SERVER_TORRENT,
    SERVER_HTTP,
    EXTENSIONS_UPDATES,
    EXTENSIONS_INSTALLING,
];

/// The notice kinds of the registry (T-35a-080, T-35a-081, T-35a-082).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    AppUpdate,
    LibraryProgress,
    LibraryErrors,
    NewChapters,
    DownloadProgress,
    DownloadErrors,
    BackupProgress,
    BackupComplete,
}

/// How urgently a notice is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Default,
    High,
}

impl Category {
    /// The category of a slot id, with or without its per-entry or per-kind `:suffix`. `None` for an id that has no
    /// category (the save-image notice, the incognito notice and the server notices).
    pub fn of(id: &str) -> Option<Self> {
        let base = id.split(':').next().unwrap_or(id);
        Some(match base {
            APP_UPDATE_PROGRESS | APP_UPDATE_INSTALL | APP_UPDATE_ERROR => Self::AppUpdate,
            LIBRARY_PROGRESS => Self::LibraryProgress,
            LIBRARY_ERROR | LIBRARY_SIZE_WARNING => Self::LibraryErrors,
            NEW_CHAPTERS | NEW_EPISODES => Self::NewChapters,
            DOWNLOAD_MANGA_PROGRESS | DOWNLOAD_ANIME_PROGRESS => Self::DownloadProgress,
            DOWNLOAD_MANGA_ERROR | DOWNLOAD_ANIME_ERROR => Self::DownloadErrors,
            BACKUP_PROGRESS | RESTORE_PROGRESS => Self::BackupProgress,
            BACKUP_COMPLETE | RESTORE_COMPLETE => Self::BackupComplete,
            _ => return None,
        })
    }

    pub fn priority(self) -> Priority {
        match self {
            Self::LibraryProgress | Self::DownloadProgress | Self::BackupProgress => Priority::Low,
            Self::BackupComplete => Priority::High,
            _ => Priority::Default,
        }
    }
}

fn kind_tag(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Manga => "manga",
        EntryKind::Anime => "anime",
    }
}

/// The slot of the "new chapters / episodes" notice of one entry.
pub fn new_items(kind: EntryKind, entry_id: i64) -> String {
    let base = match kind {
        EntryKind::Manga => NEW_CHAPTERS,
        EntryKind::Anime => NEW_EPISODES,
    };
    format!("{base}:{entry_id}")
}

/// The slot of the failure notice of one pass. The two kinds keep apart slots so one pass never replaces the other's.
pub fn library_error(kind: EntryKind) -> String {
    format!("{LIBRARY_ERROR}:{}", kind_tag(kind))
}

/// The slot of the large-queue warning of one pass.
pub fn library_size_warning(kind: EntryKind) -> String {
    format!("{LIBRARY_SIZE_WARNING}:{}", kind_tag(kind))
}

/// The slot of the progress notice of one update pass (T-26a-054 shares it with the update passes).
pub fn library_progress(kind: EntryKind) -> String {
    format!("{LIBRARY_PROGRESS}:{}", kind_tag(kind))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn the_registry_has_22_ids_and_no_duplicates() {
        // T-35a-005
        let unique: BTreeSet<&str> = ALL_IDS.iter().copied().collect();
        assert_eq!(unique.len(), ALL_IDS.len(), "no duplicate id");
        assert_eq!(ALL_IDS.len(), 22);
    }

    #[test]
    fn every_category_id_has_a_category_and_backup_complete_is_high() {
        // T-35a-080, T-35a-081, T-35a-082
        assert_eq!(Category::of(&library_progress(EntryKind::Manga)), Some(Category::LibraryProgress));
        assert_eq!(Category::of(&library_error(EntryKind::Anime)), Some(Category::LibraryErrors));
        assert_eq!(Category::of(&new_items(EntryKind::Anime, 3)), Some(Category::NewChapters));
        assert_eq!(Category::of(DOWNLOAD_ANIME_ERROR), Some(Category::DownloadErrors));
        assert_eq!(Category::of(RESTORE_COMPLETE), Some(Category::BackupComplete));
        assert_eq!(Category::of(APP_UPDATE_INSTALL), Some(Category::AppUpdate));
        assert_eq!(Category::of(SAVE_IMAGE), None);
        assert_eq!(Category::BackupComplete.priority(), Priority::High);
        assert_eq!(Category::BackupProgress.priority(), Priority::Low);
        assert_eq!(Category::AppUpdate.priority(), Priority::Default);
    }

    #[test]
    fn backup_and_restore_share_the_backup_crates_slots() {
        assert_eq!(RESTORE_PROGRESS, kuro_backup::notify::ID_RESTORE_PROGRESS);
        assert_eq!(BACKUP_PROGRESS, kuro_backup::notify::ID_BACKUP_PROGRESS);
    }

    #[test]
    fn save_image_and_the_install_prompt_do_not_collide() {
        assert!(ALL_IDS.contains(&SAVE_IMAGE) && ALL_IDS.contains(&APP_UPDATE_INSTALL));
        assert_ne!(SAVE_IMAGE, APP_UPDATE_INSTALL);
    }

    #[test]
    fn the_library_producers_build_their_slots_from_registry_ids() {
        assert_eq!(new_items(EntryKind::Manga, 7), "new.chapters:7");
        assert_eq!(new_items(EntryKind::Anime, 7), "new.episodes:7");
        assert_ne!(new_items(EntryKind::Manga, 7), new_items(EntryKind::Anime, 7));
        assert_ne!(library_error(EntryKind::Manga), library_error(EntryKind::Anime));
        assert_ne!(library_size_warning(EntryKind::Manga), library_size_warning(EntryKind::Anime));
        assert!(library_error(EntryKind::Manga).starts_with(LIBRARY_ERROR));
        assert!(library_size_warning(EntryKind::Anime).starts_with(LIBRARY_SIZE_WARNING));
        assert_ne!(library_progress(EntryKind::Manga), library_progress(EntryKind::Anime));
        assert!(library_progress(EntryKind::Manga).starts_with(LIBRARY_PROGRESS));
    }
}
