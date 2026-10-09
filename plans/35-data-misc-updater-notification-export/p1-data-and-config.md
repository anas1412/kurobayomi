# Area 35 - data models, preferences, files, endpoints and tables

All Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`. "Proposal" marks the Kurobayomi design. Types are given as Kotlin (OBSERVED) and Rust (PROPOSAL).

---

## 1. Data models

### 1.1 Export models (OBSERVED: LibraryExporter.kt:10-45)

| Type | Field | Kotlin type | Nullable | Default | Rust (PROPOSAL) |
|---|---|---|---|---|---|
| `enum ExportEntryType` | variants | `MANGA`, `ANIME` | - | - | `enum EntryKind { Manga, Anime }`; CSV text = `name().lowercase()` = `"manga"` / `"anime"` |
| `data class ExportEntry` | `type` | `ExportEntryType` | no | - | `kind: EntryKind` |
| | `title` | `String` | no | - | `title: String` |
| | `author` | `String?` | yes | - | `author: Option<String>` |
| | `artist` | `String?` | yes | - | `artist: Option<String>` |
| `LibraryExporter.ExportOptions` | `includeTitle` | `Boolean` | no | (UI default `true`) | `title: bool` |
| | `includeType` | `Boolean` | no | `true` | `kind: bool` |
| | `includeAuthor` | `Boolean` | no | `true` | `author: bool` |
| | `includeArtist` | `Boolean` | no | `true` | `artist: bool` |

Source of `ExportEntry`: `Anime.toExportEntry()` maps `title`, `ANIME`, `author`, `artist` from the anime domain model (LibraryExporter.kt:22-27); `Manga.toExportEntry()` analogously (lines 29-34). The domain `Anime`/`Manga` models have `title: String`, `author: String?`, `artist: String?` (area:07-domain-entries-items).

Rust (PROPOSAL):
```rust
// src/export/library_csv.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind { Manga, Anime }

impl EntryKind {
    /// Matches Kotlin `name.lowercase()`.
    pub fn csv_name(self) -> &'static str {
        match self { EntryKind::Manga => "manga", EntryKind::Anime => "anime" }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportEntry {
    pub kind: EntryKind,
    pub title: String,
    pub author: Option<String>,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub title: bool,
    pub kind: bool,
    pub author: bool,
    pub artist: bool,
}

impl Default for ExportOptions {
    /// UI default (SettingsDataScreen.kt:349-358): all four on.
    fn default() -> Self {
        Self { title: true, kind: true, author: true, artist: true }
    }
}
```

### 1.2 Release models (OBSERVED)

| Type (file) | Field | Kotlin type | JSON key / source | Notes |
|---|---|---|---|---|
| `GithubRelease` (data/src/main/java/tachiyomi/data/release/GithubRelease.kt:9-19) | `version` | `String` | `tag_name` | required |
| | `info` | `String` | `body` | required (null -> parse error, edge E-U9) |
| | `releaseLink` | `String` | `html_url` | required |
| | `assets` | `List<GitHubAsset>` | `assets` | required array |
| `GitHubAsset` (GithubRelease.kt:24-29) | `name` | `String` | `name` | - |
| | `downloadLink` | `String` | `browser_download_url` | - |
| `Release` (domain/src/main/java/tachiyomi/domain/release/model/Release.kt:6-11) | `version` | `String` | from `tag_name` | shown as the notification text |
| | `info` | `String` | `body` with @mentions rewritten | markdown |
| | `releaseLink` | `String` | `html_url` | opened by "What's new" and "Open on GitHub" |
| | `downloadLink` | `String` | the chosen asset's `browser_download_url` | - |
| `GetApplicationRelease.Arguments` (GetApplicationRelease.kt:78-84) | `isPreview` | `Boolean` | `isPreviewBuildType` | - |
| | `commitCount` | `Int` | `BuildConfig.COMMIT_COUNT.toInt()` | used only for preview |
| | `versionName` | `String` | `BuildConfig.VERSION_NAME` | used for release comparison |
| | `repository` | `String` | `GITHUB_REPO` | `owner/name` |
| | `forceCheck` | `Boolean` | default `false` | bypasses the 3-day gate |
| `GetApplicationRelease.Result` (GetApplicationRelease.kt:86-90) | variants | `NewUpdate(release: Release)`, `NoNewUpdate` (object), `OsTooOld` (object) | - | `OsTooOld` never produced (commented check) |

Rust (PROPOSAL):
```rust
// src/update/model.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Release {
    pub version: String,        // tag_name, verbatim
    pub notes_markdown: String, // body with @mentions rewritten; checksum section kept, stripped at display
    pub release_url: String,    // html_url
    pub download_url: String,   // chosen asset
    pub asset_name: String,     // chosen asset name (needed for the .part file name and checksum lookup)
    pub sha256: Option<String>, // parsed from notes when present (T-35a-054)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateCheck {
    NewUpdate(Release),
    NoNewUpdate,
}
```

### 1.3 Interactor arguments that move to config (PROPOSAL)

```rust
pub struct UpdateConfig {
    pub repo: Option<String>,      // "owner/name", None = updater disabled (no default)
    pub channel: UpdateChannel,    // Stable only in v1
    pub check_interval: std::time::Duration, // 3 days = 259_200 s
}
```

### 1.4 ChapterUpdate and EpisodeUpdate (OBSERVED, used by the mark-read path)

`ChapterUpdate` fields written by `Chapter.toChapterUpdate()` (domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt:19-34): `id`, `mangaId`, `read`, `bookmark`, `lastPageRead`, `dateFetch`, `sourceOrder`, `url`, `name`, `dateUpload`, `chapterNumber`, `scanlator`, `version`. `EpisodeUpdate` (domain/src/main/java/tachiyomi/domain/items/episode/model/EpisodeUpdate.kt, from line 26): `id`, `animeId`, `seen`, `bookmark`, `fillermark`, `lastSecondSeen`, `totalSeconds`, `dateFetch`, `sourceOrder`, `url`, `name`, `dateUpload`, `episodeNumber`, `scanlator`, `summary` (every field of `Episode.toEpisodeUpdate()`, all set to the snapshot values).

PROPOSAL: the notification path sends `read = Some(true)` (and nothing else) as a partial update (T-35a-016/017).

---

## 2. Preferences and persisted state

| Key (exact string) | Kotlin accessor (file:line) | Type | Default | Scope | Used by | Kurobayomi key (PROPOSAL) |
|---|---|---|---|---|---|---|
| `__APP_STATE_last_app_check` (= `Preference.appStateKey("last_app_check")`, prefix `__APP_STATE_`) | `GetApplicationRelease.kt:15-17` | Long (epoch millis) | `0` | app state (excluded from backups by `Preference.isAppState`, core/common Preference.kt:40-41; `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt:62`) | updater throttle | `app_state.last_update_check_unix_ms` (i64), default 0 |
| `hide_notification_content` | `SecurityPreferences.kt:19` | Boolean | `false` | user pref (settings Security, SettingsSecurityScreen.kt:73-74) | download and library notifiers | `notify.hide_content` (bool, default false) |
| `pref_remove_after_marked_as_read_key` | `DownloadPreferences.kt:30-33` | Boolean | `false` | user pref | mark read/seen from notifications; also SetReadStatus/SetSeenStatus | owned by area:23-downloads-manga and area:24-downloads-anime; used here unchanged |
| (none) library export options | `SettingsDataScreen.kt:349-358` (`remember`) | in memory | all `true` | screen lifetime only | export dialog | not persisted (parity) |
| Gradle property `enable-updater` | `buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt:12` | build flag | absent = disabled | build | `UPDATER_ENABLED` | cargo feature `updater`, default off |
| Gradle `BUILD_TYPE` | `util/system/BuildConfig.kt:13-14` | build type | `release` | build | `isPreviewBuildType` | not used (no preview channel; T-35a-061) |

PROPOSAL-only keys (new):
- `notify.enabled.<category>` (bool, default `true`) for each category in section 3.2 (T-35a-004). Stored in the app config table, not in app state.
- `update.repo` (string, default empty = disabled), `update.auto_check` (bool, default `true`; when false the 3-day automatic check does not run; manual check always runs).
- `export.last_options` is NOT added (parity: options are not persisted).

Rust (PROPOSAL) for the throttle:
```rust
// src/update/check.rs
pub const CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3 * 24 * 60 * 60);

/// Returns true when a check may run now. `last_checked_ms` is 0 for never checked.
pub fn may_check(now_ms: i64, last_checked_ms: i64, force: bool) -> bool {
    force || now_ms >= last_checked_ms.saturating_add(CHECK_INTERVAL.as_millis() as i64)
}
```
Note: Kotlin uses `now.isBefore(last + 3 days)` to skip; the negation is `now >= last + 3 days`, which is what `may_check` returns for `!force`.

---

## 3. Notification ids, channels and groups (OBSERVED)

### 3.1 Integer ids (`Notifications.kt`)

| Constant | Value | Line | Channel (constant) | Owner area (producer) | Kurobayomi category (PROPOSAL) |
|---|---|---|---|---|---|
| `ID_DOWNLOAD_IMAGE` | 2 | 23 | `CHANNEL_COMMON` | area:34-images-covers-cache (SaveImageNotifier) | `SaveImage` (new id, T-35a-086) |
| `ID_LIBRARY_PROGRESS` | -101 | 30 | `CHANNEL_LIBRARY_PROGRESS` | area:26-library-update-jobs | `LibraryProgress` |
| `ID_LIBRARY_ERROR` | -102 | 33 | `CHANNEL_LIBRARY_ERROR` | area:26-library-update-jobs | `LibraryErrors` |
| `ID_LIBRARY_SIZE_WARNING` | -103 | 31 | not verified in this scope (the producer in area:26-library-update-jobs decides; the progress channel is the likely one) | area:26-library-update-jobs | `LibraryErrors` (warning text) |
| `ID_DOWNLOAD_CHAPTER_PROGRESS` | -201 | 40 | `CHANNEL_DOWNLOADER_PROGRESS` | area:23-downloads-manga | `DownloadProgress` |
| `ID_DOWNLOAD_CHAPTER_ERROR` | -202 | 43 | `CHANNEL_DOWNLOADER_ERROR` | area:23-downloads-manga | `DownloadErrors` |
| `ID_DOWNLOAD_EPISODE_PROGRESS` | -203 | 41 | `CHANNEL_DOWNLOADER_PROGRESS` | area:24-downloads-anime | `DownloadProgress` |
| `ID_DOWNLOAD_EPISODE_ERROR` | -204 | 44 | `CHANNEL_DOWNLOADER_ERROR` | area:24-downloads-anime | `DownloadErrors` |
| `ID_NEW_CHAPTERS` | -301 | 50 | `CHANNEL_NEW_CHAPTERS_EPISODES` | area:26-library-update-jobs | `NewChapters` |
| `ID_NEW_EPISODES` | -1301 | 51 | `CHANNEL_NEW_CHAPTERS_EPISODES` | area:26-library-update-jobs | `NewChapters` |
| `ID_BACKUP_PROGRESS` | -501 | 60 | `CHANNEL_BACKUP_RESTORE_PROGRESS` | area:21-backup-format-and-create | `BackupProgress` |
| `ID_BACKUP_COMPLETE` | -502 | 63 | `CHANNEL_BACKUP_RESTORE_COMPLETE` | area:21-backup-format-and-create | `BackupComplete` |
| `ID_RESTORE_PROGRESS` | -503 | 61 | `CHANNEL_BACKUP_RESTORE_PROGRESS` | area:22-backup-restore | `BackupProgress` |
| `ID_RESTORE_COMPLETE` | -504 | 64 | `CHANNEL_BACKUP_RESTORE_COMPLETE` | area:22-backup-restore | `BackupComplete` |
| `ID_INCOGNITO_MODE` | -701 | 70 | `CHANNEL_INCOGNITO_MODE` | area:01-foundation-app-bootstrap (App.kt) | `Incognito` |
| `ID_TORRENT_SERVER` | -801 | 76 | `CHANNEL_TORRENT_SERVER` | area:13-core-util-storage-archive-torrent | `ServerStatus` |
| `ID_HTTP_SERVER` | -901 | 82 | `CHANNEL_HTTP_SERVER` | area:53-platform-manifest-intents-workers (HttpServerService.kt) | `ServerStatus` |
| `ID_APP_UPDATER` | 1 | 89 | `CHANNEL_APP_UPDATE` | this area | `AppUpdate` (progress and prompt) |
| `ID_APP_UPDATE_PROMPT` | 2 | 90 | `CHANNEL_APP_UPDATE` | this area | `AppUpdate` (collides with ID_DOWNLOAD_IMAGE) |
| `ID_APP_UPDATE_ERROR` | 3 | 91 | `CHANNEL_APP_UPDATE` | this area | `AppUpdate` |
| `ID_UPDATES_TO_EXTS` | -401 | 93 | `CHANNEL_EXTENSIONS_UPDATE` | area:18-extensions-manga/19 | `ExtensionUpdates` |
| `ID_EXTENSION_INSTALLER` | -402 | 94 | `CHANNEL_EXTENSIONS_UPDATE` | area:18-extensions-manga/19 | `ExtensionUpdates` (foreground) |

Group ids (`GROUP_*` strings used as notification `setGroup` keys): `GROUP_NEW_CHAPTERS = "eu.kanade.tachiyomi.NEW_CHAPTERS"` (line 52), `GROUP_NEW_EPISODES = "eu.kanade.tachiyomi.NEW_EPISODES"` (line 53). Channel-level group names: `group_library`, `group_downloader`, `group_backup_restore`, `group_apk_updates` (lines 28, 38, 58, 87).

Channel-id constants (values): `common_channel`, `library_progress_channel`, `library_errors_channel`, `new_chapters_episodes_channel`, `downloader_progress_channel`, `downloader_error_channel`, `backup_restore_progress_channel`, `backup_restore_complete_channel_v2`, `incognito_mode_channel`, `torrent_server_channel`, `http_server_channel`, `app_apk_update_channel`, `ext_apk_update_channel` (13 channels).

Deprecated channel ids deleted at startup (Notifications.kt:97-105): `downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel`. Note `library_progress_channel` is both deprecated (deleted) and current (created in the same method): the delete runs first, so the current channel is re-created (OBSERVED; harmless).

Count: 41 constants in `Notifications`: 22 `ID_*` entries, 13 `CHANNEL_*` entries and 6 `GROUP_*` entries (4 private groups plus the 2 public `GROUP_NEW_*`). 22 + 13 + 6 = 41.

### 3.2 Kurobayomi category set (PROPOSAL, T-35a-004)

`Category` enum (kuro-platform) with 14 variants, each a user-mutable toggle, plus `AppUpdate` non-mutable (always allowed when the user enabled updates):
`LibraryProgress, LibraryErrors, NewChapters, DownloadProgress, DownloadErrors, BackupProgress, BackupComplete, Incognito, ServerStatus, AppUpdate, ExtensionUpdates, SaveImage, Common, Actions` (`Actions` = the ad hoc completion notices).

Category to Android channel mapping (replacement table):
- `Common` -> `common_channel` (LOW)
- `LibraryProgress` -> `library_progress_channel` (LOW)
- `LibraryErrors` -> `library_errors_channel` (LOW)
- `NewChapters` -> `new_chapters_episodes_channel` (DEFAULT)
- `DownloadProgress` -> `downloader_progress_channel` (LOW)
- `DownloadErrors` -> `downloader_error_channel` (LOW)
- `BackupProgress` -> `backup_restore_progress_channel` (LOW)
- `BackupComplete` -> `backup_restore_complete_channel_v2` (HIGH, silent)
- `Incognito` -> `incognito_mode_channel` (LOW)
- `ServerStatus` -> `torrent_server_channel` and `http_server_channel` (LOW)
- `AppUpdate` -> `app_apk_update_channel` (DEFAULT)
- `ExtensionUpdates` -> `ext_apk_update_channel` (DEFAULT)
- `SaveImage` -> `common_channel` (LOW)

Linux urgency mapping (PROPOSAL, `gio::NotificationPriority`): `BackupComplete` -> `High`; `NewChapters`, `AppUpdate`, `ExtensionUpdates` -> `Normal`; progress and errors -> `Low` for progress, `Normal` for errors. Silent: `BackupComplete` sets `sound = false` through the category hint if the desktop supports it (not guaranteed by GIO; documented limitation).

### 3.3 Notification extras (OBSERVED, NotificationReceiver.kt:381-417)

Action strings are built as `"$ID.$NAME.<SUFFIX>"` where `ID = BuildConfig.APPLICATION_ID` (`xyz.jmir.tachiyomi.mi` on release, with `.dev` / `.debug` suffixes per build type) and `NAME = "NotificationReceiver"`. Full set:

| Constant | Suffix (literal value after `NotificationReceiver.`) |
|---|---|
| `ACTION_SHARE_IMAGE` | `SHARE_IMAGE` |
| `ACTION_SHARE_BACKUP` | `SEND_BACKUP` |
| `ACTION_CANCEL_RESTORE` | `CANCEL_RESTORE` |
| `ACTION_CANCEL_LIBRARY_UPDATE` | `CANCEL_LIBRARY_UPDATE` |
| `ACTION_CANCEL_ANIMELIB_UPDATE` | `CANCEL_ANIMELIB_UPDATE` |
| `ACTION_START_APP_UPDATE` | `ACTION_START_APP_UPDATE` |
| `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | `CANCEL_APP_UPDATE_DOWNLOAD` |
| `ACTION_MARK_AS_READ` | `MARK_AS_READ` |
| `ACTION_MARK_AS_SEEN` | `MARK_AS_SEEN` |
| `ACTION_OPEN_CHAPTER` | `ACTION_OPEN_CHAPTER` |
| `ACTION_OPEN_EPISODE` | `ACTION_OPEN_EPISODE` |
| `ACTION_DOWNLOAD_CHAPTER` | `ACTION_DOWNLOAD_CHAPTER` |
| `ACTION_DOWNLOAD_EPISODE` | `ACTION_DOWNLOAD_EPISODE` |
| `ACTION_OPEN_ENTRY` | `ACTION_OPEN_ENTRY` (unused) |
| `ACTION_RESUME_DOWNLOADS` | `ACTION_RESUME_DOWNLOADS` |
| `ACTION_PAUSE_DOWNLOADS` | `ACTION_PAUSE_DOWNLOADS` |
| `ACTION_CLEAR_DOWNLOADS` | `ACTION_CLEAR_DOWNLOADS` |
| `ACTION_RESUME_ANIME_DOWNLOADS` | `ACTION_RESUME_ANIME_DOWNLOADS` |
| `ACTION_PAUSE_ANIME_DOWNLOADS` | `ACTION_PAUSE_ANIME_DOWNLOADS` |
| `ACTION_CLEAR_ANIME_DOWNLOADS` | `ACTION_CLEAR_ANIME_DOWNLOADS` |
| `ACTION_DISMISS_NOTIFICATION` | `ACTION_DISMISS_NOTIFICATION` |

Extras: `EXTRA_URI` = `"$ID.$NAME.URI"` (String or Parcelable); `EXTRA_NOTIFICATION_ID` = `".NOTIFICATION_ID"` (Int); `EXTRA_GROUP_ID` = `".EXTRA_GROUP_ID"` (Int); `EXTRA_MANGA_ID` = `".EXTRA_MANGA_ID"` (Long; holds the anime id for the anime builders); `EXTRA_CHAPTER_ID` = `".EXTRA_CHAPTER_ID"` (Long, declared; used by `openChapter`); `EXTRA_CHAPTER_URL` = `".EXTRA_CHAPTER_URL"` (`Array<String>`).

Activity extras used by MainActivity: `"notificationId"` (Int), `"groupId"` (Int), `Constants.MANGA_EXTRA = "manga"` (Long), `Constants.ANIME_EXTRA = "anime"` (Long).

Kurobayomi replacement (PROPOSAL): no string actions. An `AppAction` enum is serialised to a GVariant string (JSON, `serde_json`) and carried as the target of `app.notice` (section in p1-rust-design.md). Equivalent of each extra is a named field of the enum variant.

---

## 4. Files, folders and formats on disk

| Path / name | Created by | Format | Lifetime | Notes |
|---|---|---|---|---|
| `File(context.externalCacheDir, "update.apk")` (AppUpdateDownloadJob.kt:106) | download job | Android APK (`application/vnd.android.package-archive`) | overwritten each download; never deleted after install | parent `mkdirs()`; partial file deleted on exception (OkioExtensions.kt:14-27). If `externalCacheDir` is null the path is the relative `update.apk` (OBSERVED). |
| FileProvider URI `content://<applicationId>.provider/<path>` (`File.getUriCompat`, FileExtensions.kt:16-18) | install prompt | content URI | per grant | Dropped on Linux. |
| `aniyomi_library.csv` (suggested name, SettingsDataScreen.kt:397) | CSV export | `text/csv`, UTF-8, no BOM, CRLF | user-chosen location via CreateDocument | Kurobayomi default name: `kurobayomi_library.csv` (PROPOSAL, T-35a-053). |
| `shared_image` cache dir (`Context.cacheImageDir`, FileExtensions.kt:8-9) | SaveImageNotifier share path (area:34-images-covers-cache) | images | cache | Not owned here. |
| Backup share MIME `application/x-protobuf+gzip` (NotificationReceiver.kt:93) | backup share action | gzip'd protobuf backup | user file | Owned by area:21-backup-format-and-create. |

Kurobayomi paths (PROPOSAL, via `kuro-platform::Paths`, never string literals):
- Update download: `Paths::cache_dir()/updates/<asset_name>.part` while downloading; renamed to `Paths::cache_dir()/updates/<asset_name>` on success (T-35a-050).
- Export: user-chosen path from the file dialog; temp file `<target>.kuro-tmp` in the same directory (T-35a-074).
- File names pass through the portable sanitiser (addenda rule 5, T-req-platform-008). `asset_name` is from the release and is sanitised before use.

---

## 5. Network endpoints (OBSERVED)

| Name | Method | URL | Query / body | Headers | Auth | Response |
|---|---|---|---|---|---|---|
| Latest release (app updater) | GET | `https://api.github.com/repos/${arguments.repository}/releases/latest` (ReleaseServiceImpl.kt:21) | none | those set by the shared OkHttp client in NetworkHelper (area:12-network-stack); no custom Accept header in this code | none (unauthenticated) | 200 JSON `GithubRelease`; non-2xx -> `HttpException(code)` (`awaitSuccess`, OkHttpExtensions.kt) |
| Asset download (app updater) | GET | `asset.browser_download_url` (`GitHubAsset.downloadLink`) | none | shared client; cache disabled; `callTimeout` 30 h (OkHttpExtensions.kt:119-131) | none | 200 binary, streamed to `update.apk` |
| Release page (What's new / Open on GitHub) | opened in browser | `html_url` (`releaseLink`) | - | - | - | - |
| Repository used | - | `aniyomiorg/aniyomi` (release) and `aniyomiorg/aniyomi-preview` (preview) (AppUpdateChecker.kt:233-239) | - | - | - | Kurobayomi: `update.repo` config, no default (PROPOSAL) |

Rust (PROPOSAL) request shape, via reqwest (area:12-network-stack client):
```
GET https://api.github.com/repos/{owner}/{repo}/releases/latest
Accept: application/vnd.github+json
X-GitHub-Api-Version: 2022-11-28
User-Agent: kurobayomi/<version> (+https://github.com/<owner>/<repo>)
```
The `Accept` and `X-GitHub-Api-Version` headers are recommended by GitHub's REST documentation and are PROPOSAL additions. The User-Agent is required by GitHub for API requests.

Response mapping (PROPOSAL): 200 -> parse; 404 -> `UpdateError::NoReleases` ("No releases published yet"); 403/429 -> `UpdateError::RateLimited`; other non-2xx -> `UpdateError::Http(code)`; network error -> `UpdateError::Network(msg)`.

---

## 6. Database tables and columns touched (OBSERVED)

| Table (SQL name) | Source file | Columns read | Columns written | Written by | Notes |
|---|---|---|---|---|---|
| `mangas` | data/src/main/sqldelight/data/mangas.sq | `_id` (PK), `favorite`, `title`, `author`, `artist`, `source` | none | export reads (`getFavorites`, mangas.sq:76-79); notification path reads `source` for the download delete | index `library_favorite_index` (mangas.sq:33) |
| `animes` | data/src/main/sqldelightanime/dataanime/animes.sq | `_id` (PK), `favorite`, `title`, `author`, `artist`, `source` | none | export reads (`getFavorites`, animes.sq:87-90) | index `animelib_favorite_index` (animes.sq:43) |
| `chapters` | data/src/main/sqldelight/data/chapters.sq | `_id`, `manga_id`, `url`, `read`, `bookmark`, `last_page_read` | `read` (and, via the full-row update of the current code, every other column) | mark-read notification path (NotificationReceiver.kt:298-325) | partial index `chapters_unread_by_manga_index` (chapters.sq:24) |
| `episodes` | data/src/main/sqldelightanime/dataanime/episodes.sq | `_id`, `anime_id`, `url`, `seen`, `bookmark`, `last_second_seen` | `seen` (and full row in current code) | mark-seen notification path (NotificationReceiver.kt:327-348) | partial index `episodes_unseen_by_anime_index` (episodes.sq:30) |
| `downloads` queue / files | owned by area:23-downloads-manga / 24 / 25 | - | - | download and delete actions | not in this scope |
| `last_app_check` | app state key (section 2) | - | Long | updater throttle | excluded from backup |

Triggers `update_last_favorite_at_mangas` / `update_last_favorite_at_animes` (mangas.sq:36-40, animes.sq:48-52) set `favorite_modified_at` when `favorite` changes; they fire only from writes made by other areas (not this scope).

---

## 7. Strings used by this scope (OBSERVED keys and English values)

MR (`i18n/src/commonMain/moko-resources/base/strings.xml`):
`app_name` = "Aniyomi" (translatable=false), `label_backup` = "Backup and restore", `download_notifier_downloader_title` = "Downloader", `label_library` = "Library", `label_recent_updates` = "Updates", `channel_common` = "Common", `channel_progress` = "Progress", `channel_errors` = "Errors", `channel_complete` = "Complete", `pref_incognito_mode` = "Incognito mode", `channel_app_updates` = "App updates", `channel_ext_updates` = "Extension updates", `update_check_notification_update_available` = "New version available!", `action_download` = "Download", `whats_new` = "What's new", `update_check_notification_download_in_progress` = "Downloading…", `action_cancel` = "Cancel", `action_install` = "Install", `update_check_notification_download_complete` = "Tap to install update", `update_check_notification_download_error` = "Download error", `action_retry` = "Retry", `action_share` = "Share", `library_exported` = "Library Exported", `migration_dialog_what_to_include` = "Select data to include", `title` = "Title", `author` = "Author", `artist` = "Artist", `action_save` = "Save", `update_check_no_new_updates` = "No new updates available", `update_check_eol` = "This Android version is no longer supported", `export` = "Export", `library_list` = "Library List", `action_webview_back` = "Back", `action_next_chapter` = "Next chapter", `action_previous_chapter` = "Previous chapter", `spen_next_page` = "Next page", `spen_previous_page` = "Previous page", `action_global_search_hint` = "Global search…", `update_check_confirm` = "Download", `action_not_now` = "Not now", `update_check_open` = "Open on GitHub", `check_for_updates` (About row; text "Check for updates" per spec/settings.json:3173), `cache_deleted` = "Cache cleared, %1$d files deleted" (not used here).

AYMR (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`): `channel_new_chapters_episodes` = "Chapter/Episode updates", `download_error` = "Error", `pref_category_torrentserver` = "Torrent Server Preferences", `pref_http_server_name` = "Http server", `type` = "Type", `label_storage` = "Storage", `action_global_anime_search` = "Global Anime Search".

Kurobayomi string keys (PROPOSAL, English values, stored in the app's i18n table, area:51-i18n): `update.available.title` = "New version available!", `update.available.whats_new` = "What's new", `update.download` = "Download", `update.open_github` = "Open on GitHub", `update.not_now` = "Not now", `update.progress` = "Downloading…", `update.install_ready` = "Downloaded. Open folder", `update.error` = "Download error", `update.retry` = "Retry", `update.none` = "No new updates available", `update.no_asset` = "No build for this platform in the latest release", `update.rate_limited` = "GitHub rate limit reached, try again later", `update.disabled_hint` = "Set update.repo in settings to enable update checks", `export.title` = "Library export", `export.columns_title` = "Select data to include", `export.title_col` = "Title", `export.type_col` = "Type", `export.author_col` = "Author", `export.artist_col` = "Artist", `export.save` = "Save", `export.cancel` = "Cancel", `export.done` = "Library Exported", `export.failed` = "Export failed: {error}", `export.done_empty` = "Library Exported (0 entries)", `update.no_notes` = "No release notes.", `update.install_unverified` = "(not verified)", `notify.action.download` = "Download", `notify.action.cancel` = "Cancel", `notify.action.retry` = "Retry", `notify.action.mark_read` = "Mark as read", `notify.action.mark_seen` = "Mark as seen", `notify.action.open` = "Open", `notify.action.open_folder` = "Open folder", `notify.error.item_missing` = "Error", `notify.error.open_failed` = "Could not open: {error}".

---

## 8. Consistency rules for the data layer (PROPOSAL)

1. Action payloads contain ids, never full entities. The router re-reads each entity (a missing entity is the "Error" toast, section 1.9.4).
2. Chapter/episode URLs in payloads are used as lookup keys only (`(url, parent_id)`), matching `getChapter.await(url, mangaId)`.
3. Notification ids are `i32` in Rust (`Notifier` ids are strings in the GIO backend; the integer registry is kept for reference only).
4. Export output is a pure function of (entries, options); the same input gives byte-identical output (test T-35a-095).
5. The updater never writes to the library DB.
