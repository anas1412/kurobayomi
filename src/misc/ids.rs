//! The notice id registry (T-35a-005): every desktop notice slot of the app has one id here, so two producers never
//! share a slot. The Android collision (`ID_APP_UPDATE_PROMPT` and `ID_DOWNLOAD_IMAGE` were both 2) is fixed by giving
//! `save-image` and `app-update.install` their own ids.
//!
//! Decision: the registry lives in `misc` because `src/notify` is not part of this lane; the ids are the strings of
//! p1-rust-design.md 3.2. A per-entry slot (one notice per manga or anime) is the base id plus `:` and the entry id.

use kuro_platform::kind::Kind;

pub const APP_UPDATE_PROGRESS: &str = "app-update.progress";
pub const APP_UPDATE_INSTALL: &str = "app-update.install";
pub const APP_UPDATE_ERROR: &str = "app-update.error";
pub const SAVE_IMAGE: &str = "save-image";
pub const LIBRARY_PROGRESS: &str = "library.progress";
pub const LIBRARY_ERROR: &str = "library.error";
pub const LIBRARY_SIZE_WARNING: &str = "library.size-warning";
pub const LIBRARY_SUMMARY: &str = "library.summary";
#[cfg(test)]
pub const DOWNLOAD_MANGA_PROGRESS: &str = "download.manga.progress";
#[cfg(test)]
pub const DOWNLOAD_MANGA_ERROR: &str = "download.manga.error";
#[cfg(test)]
pub const DOWNLOAD_ANIME_PROGRESS: &str = "download.anime.progress";
#[cfg(test)]
pub const DOWNLOAD_ANIME_ERROR: &str = "download.anime.error";
pub const NEW_CHAPTERS: &str = "new.chapters";
pub const NEW_EPISODES: &str = "new.episodes";
#[cfg(test)]
// One id source for backup and restore (T-35a-082): the slots the backup crate posts to are the registry's ids, so the
// router's withdraw of a restore and the restore's own notice name the same slot.
pub const BACKUP_PROGRESS: &str = kuro_backup::notify::ID_BACKUP_PROGRESS;
#[cfg(test)]
pub const BACKUP_COMPLETE: &str = kuro_backup::notify::ID_BACKUP_RESULT;
#[cfg(test)]
pub const RESTORE_PROGRESS: &str = kuro_backup::notify::ID_RESTORE_PROGRESS;
#[cfg(test)]
pub const RESTORE_COMPLETE: &str = kuro_backup::notify::ID_RESTORE_RESULT;
#[cfg(test)]
pub const INCOGNITO: &str = "incognito";
#[cfg(test)]
pub const SERVER_TORRENT: &str = "server.torrent";
#[cfg(test)]
pub const SERVER_HTTP: &str = "server.http";
#[cfg(test)]
pub const EXTENSIONS_UPDATES: &str = "extensions.updates";
#[cfg(test)]
pub const EXTENSIONS_INSTALLING: &str = "extensions.installing";

#[cfg(test)]
/// Every base id, for the duplicate test.
pub const ALL_IDS: &[&str] = &[
    APP_UPDATE_PROGRESS,
    APP_UPDATE_INSTALL,
    APP_UPDATE_ERROR,
    SAVE_IMAGE,
    LIBRARY_PROGRESS,
    LIBRARY_ERROR,
    LIBRARY_SIZE_WARNING,
    LIBRARY_SUMMARY,
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

#[cfg(test)]
/// The notice kinds of the registry (T-35a-080, T-35a-081, T-35a-082, T-35a-083, T-35a-084, T-35a-085, T-35a-086).
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
    Incognito,
    ServerStatus,
    ExtensionUpdates,
    SaveImage,
}

#[cfg(test)]
/// How urgently a notice is shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Priority {
    Low,
    Default,
    High,
}

#[cfg(test)]
impl Category {
    /// The category of a slot id, with or without its per-entry or per-kind `:suffix`. `None` for an id that has no
    /// category (the server notices are status lines in the window, see T-35a-084).
    pub fn of(id: &str) -> Option<Self> {
        let base = id.split(':').next().unwrap_or(id);
        Some(match base {
            APP_UPDATE_PROGRESS | APP_UPDATE_INSTALL | APP_UPDATE_ERROR => Self::AppUpdate,
            LIBRARY_PROGRESS => Self::LibraryProgress,
            LIBRARY_ERROR | LIBRARY_SIZE_WARNING => Self::LibraryErrors,
            LIBRARY_SUMMARY => Self::NewChapters,
            NEW_CHAPTERS | NEW_EPISODES => Self::NewChapters,
            DOWNLOAD_MANGA_PROGRESS | DOWNLOAD_ANIME_PROGRESS => Self::DownloadProgress,
            DOWNLOAD_MANGA_ERROR | DOWNLOAD_ANIME_ERROR => Self::DownloadErrors,
            BACKUP_PROGRESS | RESTORE_PROGRESS => Self::BackupProgress,
            BACKUP_COMPLETE | RESTORE_COMPLETE => Self::BackupComplete,
            INCOGNITO => Self::Incognito,
            SERVER_TORRENT | SERVER_HTTP => Self::ServerStatus,
            EXTENSIONS_UPDATES | EXTENSIONS_INSTALLING => Self::ExtensionUpdates,
            SAVE_IMAGE => Self::SaveImage,
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

fn kind_tag(kind: Kind) -> &'static str {
    match kind {
        Kind::Manga => "manga",
        Kind::Anime => "anime",
    }
}

/// The slot of the "new chapters / episodes" notice of one entry.
pub fn new_items(kind: Kind, entry_id: i64) -> String {
    let base = match kind {
        Kind::Manga => NEW_CHAPTERS,
        Kind::Anime => NEW_EPISODES,
    };
    format!("{base}:{entry_id}")
}

/// The slot of the failure notice of one pass. The two kinds keep apart slots so one pass never replaces the other's.
pub fn library_error(kind: Kind) -> String {
    format!("{LIBRARY_ERROR}:{}", kind_tag(kind))
}

/// The slot of the large-queue warning of one pass.
pub fn library_size_warning(kind: Kind) -> String {
    format!("{LIBRARY_SIZE_WARNING}:{}", kind_tag(kind))
}

/// The slot of the summary notice of one update pass: the new items of all its entries.
pub fn library_summary(kind: Kind) -> String {
    format!("{LIBRARY_SUMMARY}:{}", kind_tag(kind))
}

/// The slot of the progress notice of one update pass (T-26a-054 shares it with the update passes).
pub fn library_progress(kind: Kind) -> String {
    format!("{LIBRARY_PROGRESS}:{}", kind_tag(kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_category_id_has_a_category_and_backup_complete_is_high() {
        // T-35a-080, T-35a-081, T-35a-082
        assert_eq!(Category::of(&library_progress(Kind::Manga)), Some(Category::LibraryProgress));
        assert_eq!(Category::of(&library_error(Kind::Anime)), Some(Category::LibraryErrors));
        assert_eq!(Category::of(&new_items(Kind::Anime, 3)), Some(Category::NewChapters));
        assert_eq!(Category::of(DOWNLOAD_ANIME_ERROR), Some(Category::DownloadErrors));
        assert_eq!(Category::of(RESTORE_COMPLETE), Some(Category::BackupComplete));
        assert_eq!(Category::of(APP_UPDATE_INSTALL), Some(Category::AppUpdate));
        assert_eq!(Category::of(SAVE_IMAGE), Some(Category::SaveImage));
        assert_eq!(Category::of(INCOGNITO), Some(Category::Incognito));
        assert_eq!(Category::of(SERVER_TORRENT), Some(Category::ServerStatus));
        assert_eq!(Category::of(SERVER_HTTP), Some(Category::ServerStatus));
        assert_eq!(Category::of(EXTENSIONS_UPDATES), Some(Category::ExtensionUpdates));
        assert_eq!(Category::of(EXTENSIONS_INSTALLING), Some(Category::ExtensionUpdates));
        assert_eq!(Category::of("app-update.unknown"), None);
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
        assert_eq!(new_items(Kind::Manga, 7), "new.chapters:7");
        assert_eq!(new_items(Kind::Anime, 7), "new.episodes:7");
        assert_ne!(new_items(Kind::Manga, 7), new_items(Kind::Anime, 7));
        assert_ne!(library_error(Kind::Manga), library_error(Kind::Anime));
        assert_ne!(library_size_warning(Kind::Manga), library_size_warning(Kind::Anime));
        assert!(library_error(Kind::Manga).starts_with(LIBRARY_ERROR));
        assert!(library_size_warning(Kind::Anime).starts_with(LIBRARY_SIZE_WARNING));
        assert_ne!(library_progress(Kind::Manga), library_progress(Kind::Anime));
        assert!(library_progress(Kind::Manga).starts_with(LIBRARY_PROGRESS));
    }
}
