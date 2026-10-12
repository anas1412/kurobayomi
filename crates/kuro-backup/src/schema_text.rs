//! The `Backup` message as text, for the debug screen's backup schema page (T-21a-035, `BackupSchemaScreen.kt`). The
//! text lists the thirteen fields of the current container with their numbers. Entry messages are not listed here.

/// The current `Backup` container in the proto-style text the debug screen shows.
pub const TEXT: &str = "message Backup {
  repeated BackupManga backupManga = 1;
  repeated BackupCategory backupCategories = 2;
  repeated BackupSource backupSources = 101;
  repeated BackupPreference backupPreferences = 104;
  repeated BackupSourcePreferences backupSourcePreferences = 105;
  repeated BackupExtensionRepos backupMangaExtensionRepo = 106;
  bool isLegacy = 500;
  repeated BackupAnime backupAnime = 501;
  repeated BackupCategory backupAnimeCategories = 502;
  repeated BackupAnimeSource backupAnimeSources = 503;
  repeated BackupExtension backupExtensions = 504;
  repeated BackupExtensionStore backupAnimeExtensionStores = 505;
  repeated BackupCustomButtons backupCustomButton = 506;
}";
