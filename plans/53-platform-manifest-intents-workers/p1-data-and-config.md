# Area 53 — data, configuration, files, endpoints and identifiers

Source root: `/home/blackbox/aniyomi-src`. All paths are repo-relative. "Observed" facts come from the Kotlin and XML on disk; "Proposal" marks Kurobayomi decisions. Section numbers are referenced from `p1-behavior.md`, `p1-inventory.md`, `p1-tasks.md` and `p1-rust-design.md`.

## 1. Manifest components (observed, `app/src/main/AndroidManifest.xml`)

`namespace` is `eu.kanade.tachiyomi` (`app/build.gradle.kts:17`). `applicationId` is `xyz.jmir.tachiyomi.mi` (`app/build.gradle.kts:20`); build-type suffixes are `.dev` (line 47), `.debug` (line 65) and `.benchmark` (line 80). `BuildConfig.APPLICATION_ID` includes the suffix, and every `ACTION_*` constant in the receiver is built from it (section 3.1).

| Kind | Name | Exported | Attributes and filters | Decision |
|---|---|---|---|---|
| activity | `.ui.main.MainActivity` | true | `launchMode=singleTop`, `windowSoftInputMode=adjustResize`, theme `Theme.Tachiyomi.SplashScreen`. Filters: (a) MAIN + LAUNCHER; (b) VIEW, DEFAULT, BROWSABLE, scheme `tachiyomi`, host `add-repo` (label `action_add_repo`); (c) VIEW, DEFAULT, BROWSABLE, scheme `aniyomi`, host `add-repo` (label `action_addExtensionStore`); (d) VIEW, DEFAULT, BROWSABLE, scheme `aniyomi`, host `extension-store` (label `action_addExtensionStore`); (e) VIEW, DEFAULT, BROWSABLE, schemes `file` and `content`, host `*`, mimeType `*/*`, pathPatterns `.*\\.tachibk` through seven nested levels (label `pref_restore_backup`); (f) `meta-data android.app.shortcuts` = `@xml/shortcuts` | Port: section 2 of behaviour; tasks T-53a-001, -003, -004, -010, -011 |
| activity | `.crash.CrashActivity` | false | `process=":error_handler"` | owned by area:01 |
| activity | `.ui.deeplink.anime.DeepLinkAnimeActivity` | true | label `action_global_anime_search` ("Global Anime Search"), `launchMode=singleTask`, `Theme.NoDisplay`. Filters: SEARCH; `com.google.android.gms.actions.SEARCH_ACTION`; `eu.kanade.tachiyomi.ANIMESEARCH`; SEND with `text/plain`. `meta-data android.app.searchable` = `@xml/searchable` | Drop trampoline |
| activity | `.ui.deeplink.manga.DeepLinkMangaActivity` | true | label `action_global_manga_search` ("Global Manga Search"), same attributes. Filters: SEARCH; `com.google.android.gms.actions.SEARCH_ACTION`; `eu.kanade.tachiyomi.SEARCH`; `eu.kanade.tachiyomi.ANIMESEARCH`; SEND `text/plain` | Drop trampoline |
| activity | `.ui.reader.ReaderActivity` | false | `launchMode=singleTask`. Filter: `com.samsung.android.support.REMOTE_ACTION`; `meta-data com.samsung.android.support.REMOTE_ACTION` = `@xml/s_pen_actions` | Port (reader). S Pen dropped |
| activity | `.ui.player.PlayerActivity` | false | `autoRemoveFromRecents=true`, `configChanges="orientation|screenLayout|screenSize|smallestScreenSize|keyboardHidden|keyboard|uiMode"`, `launchMode=singleTask`, `resizeableActivity=true`, `supportsPictureInPicture=true`, theme `Theme.Tachiyomi`. Samsung REMOTE_ACTION filter and meta-data as reader | Port (player). PiP dropped |
| activity | `.ui.security.UnlockActivity` | false | theme `Theme.Tachiyomi` | owned by area:36 |
| activity | `.ui.webview.WebViewActivity` | false | `configChanges="uiMode|orientation|screenSize"` | owned by area:36 |
| activity | `.extension.manga.util.MangaExtensionInstallActivity` | false | theme `Theme.Translucent.NoTitleBar` | Drop |
| activity | `.extension.anime.util.AnimeExtensionInstallActivity` | false | theme `Theme.Translucent.NoTitleBar` | Drop |
| activity | `.ui.setting.track.TrackLoginActivity` | true | label `track_activity_name` ("Tracker login"). Filter: VIEW, DEFAULT, BROWSABLE, scheme `aniyomi`, hosts `myanimelist-auth`, `anilist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth` | Port (OAuth routing) |
| receiver | `.data.notification.NotificationReceiver` | false | none | Rewrite (section 3 and behaviour section 4) |
| service | `.extension.manga.util.MangaExtensionInstallService` | false | `foregroundServiceType="shortService"` | Drop |
| service | `.extension.anime.util.AnimeExtensionInstallService` | false | `foregroundServiceType="shortService"` | Drop |
| service | `androidx.appcompat.app.AppLocalesMetadataHolderService` | false | `enabled=false`, meta-data `autoStoreLocales=true` | Drop |
| service | `androidx.work.impl.foreground.SystemForegroundService` | (library) | `foregroundServiceType="dataSync"`, `tools:node="merge"` | Drop (job runner replaces it) |
| service | `.data.torrent.service.TorrentServerService` | (default) | `foregroundServiceType="dataSync"` | Port (background service) |
| service | `.data.player.service.HttpServerService` | (default) | `foregroundServiceType="dataSync"` | Port (background service) |
| provider | `androidx.core.content.FileProvider` | false | authority `${applicationId}.provider`, `grantUriPermissions=true`, meta-data `android.support.FILE_PROVIDER_PATHS` = `@xml/provider_paths` | Drop |
| provider | `rikka.shizuku.ShizukuProvider` | true | authority `${applicationId}.shizuku`, `enabled=true`, `multiprocess=false`, `permission=android.permission.INTERACT_ACROSS_USERS_FULL` | Drop |
| meta-data | `android.webkit.WebView.EnableSafeBrowsing` | | value `false` | Drop |
| meta-data | `android.webkit.WebView.MetricsOptOut` | | value `true` | Drop |

Application element attributes (observed, lines from the manifest): `android:name=".App"`, `allowBackup="false"`, `enableOnBackInvokedCallback="true"`, `hardwareAccelerated="true"`, `icon="@mipmap/ic_launcher"`, `label="@string/app_name"`, `largeHeap="true"`, `localeConfig="@xml/locales_config"`, `networkSecurityConfig="@xml/network_security_config"`, `preserveLegacyExternalStorage="true"`, `requestLegacyExternalStorage="true"`, `roundIcon="@mipmap/ic_launcher_round"`, `supportsRtl="true"`, `theme="@style/Theme.Tachiyomi"`.

Module-level manifests: `core-metadata`, `core/archive`, `core/common`, `data`, `domain`, `i18n`, `macrobenchmark`, `presentation-core`, `source-api/src/androidMain`, `source-local/src/androidMain` are `<manifest />` (2 LOC each). `presentation-widget` declares six `<receiver>` elements described in behaviour section 13.4.

Launcher shortcuts (`app/shortcuts.xml`): see section 4.4.

## 2. Permissions and application attributes (observed, with decisions)

| Declaration | Lines | Consumer in code | Linux decision (task) |
|---|---|---|---|
| `android.permission.INTERNET` | manifest line 6 | all HTTP | not needed (reqwest) |
| `android.permission.ACCESS_NETWORK_STATE` | 7 | `NetworkStateTracker.kt`, connectivity manager calls in `AnimeDownloadJob.kt`/`MangaDownloadJob.kt`/library jobs | `gio::NetworkMonitor` (T-53a-026) |
| `android.permission.ACCESS_WIFI_STATE` | 8 | `NetworkExtensions.kt:32` (`wifiManager.isWifiEnabled`) | NetworkManager `WirelessEnabled` and `PrimaryConnectionType` (T-53a-026) |
| `android.permission.MANAGE_EXTERNAL_STORAGE` (tools:ignore ScopedStorage) | 12 to 14 | `presentation/more/settings/Preference.kt:187` | folder picker through portal (T-53a-027) |
| `android.permission.FOREGROUND_SERVICE` | 18 | services | `hold()` and notices (T-53a-037) |
| `android.permission.WAKE_LOCK` | 19 | no acquire in app code | dropped (T-53a-028) |
| `android.permission.REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | 20 | `PermissionStep.kt:66,106`, `SettingsAdvancedScreen.kt:162,166` | dropped (T-53a-028) |
| `android.permission.REQUEST_INSTALL_PACKAGES` | 23 | `ContextExtensions.kt:175` | dropped with APK install (T-53a-038) |
| `android.permission.REQUEST_DELETE_PACKAGES` | 24 | not referenced by name (uninstall uses `ACTION_UNINSTALL_PACKAGE`) | dropped (T-53a-038) |
| `android.permission.UPDATE_PACKAGES_WITHOUT_USER_ACTION` | 25 | not referenced | dropped (T-53a-038) |
| `android.permission.QUERY_ALL_PACKAGES` (tools:ignore) | 28 to 29 | `getInstalledPackages(0)` in five places | dropped (T-53a-038) |
| `android.permission.POST_NOTIFICATIONS` | 31 | `NotificationExtensions.kt:30` to `:37` | replaced by notification server check (T-53a-023) |
| `android.permission.READ_APP_SPECIFIC_LOCALES` (tools:ignore) | 32 to 35 | `AppLanguageScreen.kt` | area:51 (T-53a-031) |
| `android.permission.FOREGROUND_SERVICE_DATA_SYNC` | 37 | services | dropped (T-53a-037) |

Application attributes map (Proposal):

| Attribute | Value | Linux decision |
|---|---|---|
| `allowBackup` | false | no OS backup; area:21 and area:22 files |
| `enableOnBackInvokedCallback` | true | Escape, Alt+Left, mouse back button (area:36) |
| `hardwareAccelerated` | true | not applicable (GTK renderer) |
| `largeHeap` | true | dropped; image cache budget in area:34 |
| `localeConfig` | `@xml/locales_config` | area:51 |
| `networkSecurityConfig` | `@xml/network_security_config` | section 7.3 of this file |
| `preserveLegacyExternalStorage`, `requestLegacyExternalStorage` | true | dropped |
| `supportsRtl` | true | GTK direction from locale; verify Arabic (area:51) |
| `theme` | `Theme.Tachiyomi` | area:49 |
| `icon`, `roundIcon`, `label` | `@mipmap/ic_launcher`, `@string/app_name` | `.desktop` `Icon=` and hicolor icon theme (area:49, T-53a-041) |

## 3. Notification registry: actions, extras, ids, channels (observed)

### 3.1 Receiver action strings

`NotificationReceiver` builds its actions as `"$ID.$NAME.<suffix>"`, where `ID = BuildConfig.APPLICATION_ID` and `NAME = "NotificationReceiver"` (`NotificationReceiver.kt:381` to `:410`). For the release build the prefix is `xyz.jmir.tachiyomi.mi.NotificationReceiver.`. The table gives the suffix (the constant's value after the prefix) and the constant name.

| Constant | Suffix | Kotlin line | Sent by |
|---|---|---|---|
| `ACTION_SHARE_IMAGE` | `SHARE_IMAGE` | 381 | `SaveImageNotifier.kt:85` |
| `ACTION_SHARE_BACKUP` | `SEND_BACKUP` | 383 | `BackupNotifier.kt:82` |
| `ACTION_CANCEL_RESTORE` | `CANCEL_RESTORE` | 385 | `BackupNotifier.kt:114` |
| `ACTION_CANCEL_LIBRARY_UPDATE` | `CANCEL_LIBRARY_UPDATE` | 387 | `MangaLibraryUpdateNotifier.kt:60` |
| `ACTION_CANCEL_ANIMELIB_UPDATE` | `CANCEL_ANIMELIB_UPDATE` | 388 | `AnimeLibraryUpdateNotifier.kt:61` |
| `ACTION_START_APP_UPDATE` | `ACTION_START_APP_UPDATE` | 390 | `AppUpdateNotifier.kt:39,163` |
| `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | `CANCEL_APP_UPDATE_DOWNLOAD` | 391 | `AppUpdateNotifier.kt:94` |
| `ACTION_MARK_AS_READ` | `MARK_AS_READ` | 393 | `MangaLibraryUpdateNotifier.kt:262` |
| `ACTION_MARK_AS_SEEN` | `MARK_AS_SEEN` | 394 | `AnimeLibraryUpdateNotifier.kt:276` |
| `ACTION_OPEN_CHAPTER` | `ACTION_OPEN_CHAPTER` | 395 | none (dead) |
| `ACTION_OPEN_EPISODE` | `ACTION_OPEN_EPISODE` | 396 | none (dead) |
| `ACTION_DOWNLOAD_CHAPTER` | `ACTION_DOWNLOAD_CHAPTER` | 397 | `MangaLibraryUpdateNotifier.kt:285` |
| `ACTION_DOWNLOAD_EPISODE` | `ACTION_DOWNLOAD_EPISODE` | 398 | `AnimeLibraryUpdateNotifier.kt:299` |
| `ACTION_OPEN_ENTRY` | `ACTION_OPEN_ENTRY` | 400 | none (unused, no branch) |
| `ACTION_RESUME_DOWNLOADS` | `ACTION_RESUME_DOWNLOADS` | 402 | `MangaDownloadNotifier.kt:138` |
| `ACTION_PAUSE_DOWNLOADS` | `ACTION_PAUSE_DOWNLOADS` | 403 | `MangaDownloadNotifier.kt:85` |
| `ACTION_CLEAR_DOWNLOADS` | `ACTION_CLEAR_DOWNLOADS` | 404 | `MangaDownloadNotifier.kt:144` |
| `ACTION_RESUME_ANIME_DOWNLOADS` | `ACTION_RESUME_ANIME_DOWNLOADS` | 406 | `AnimeDownloadNotifier.kt:143` |
| `ACTION_PAUSE_ANIME_DOWNLOADS` | `ACTION_PAUSE_ANIME_DOWNLOADS` | 407 | `AnimeDownloadNotifier.kt:87` |
| `ACTION_CLEAR_ANIME_DOWNLOADS` | `ACTION_CLEAR_ANIME_DOWNLOADS` | 408 | `AnimeDownloadNotifier.kt:149` |
| `ACTION_DISMISS_NOTIFICATION` | `ACTION_DISMISS_NOTIFICATION` | 410 | `AppUpdateNotifier.kt:138,168` |

Extras (names are `"$ID.$NAME.<suffix>"` too; values as built):

| Constant | Suffix | Type | Kotlin line | Set by |
|---|---|---|---|---|
| `EXTRA_URI` | `URI` | String (`uri.toString()`) or Parcelable Uri (backup) | 412 | `shareImagePendingBroadcast` (`:598`), `shareBackupPendingBroadcast` (`:953`) |
| `EXTRA_NOTIFICATION_ID` | `NOTIFICATION_ID` | Int | 413 | dismiss, mark, download builders; `cancelRestorePendingBroadcast` |
| `EXTRA_GROUP_ID` | `EXTRA_GROUP_ID` | Int | 414 | mark and download builders |
| `EXTRA_MANGA_ID` | `EXTRA_MANGA_ID` | Long (manga id for manga builders; anime id for anime builders) | 415 | mark and download builders |
| `EXTRA_CHAPTER_ID` | `EXTRA_CHAPTER_ID` | Long | 416 | only the dead `ACTION_OPEN_*` path |
| `EXTRA_CHAPTER_URL` | `EXTRA_CHAPTER_URL` | String array (`chapters.map { it.url }.toTypedArray()` or `episodes.map { it.url }`) | 417 | mark and download builders |

Extras used by `AppUpdateDownloadJob` (`app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt:130` to `:131`): `EXTRA_DOWNLOAD_URL = "DOWNLOAD_URL"`, `EXTRA_DOWNLOAD_TITLE = "DOWNLOAD_TITLE"`. These are WorkManager input data keys, not intent extras.

Extras used by the extension installer (`MangaExtensionInstaller.kt:324`): `EXTRA_DOWNLOAD_ID = "ExtensionInstaller.extra.DOWNLOAD_ID"` (anime twin at `AnimeExtensionInstaller.kt:324`, same value). `EXTRA_INSTALLER = "EXTRA_INSTALLER"` (`MangaExtensionInstallService.kt:74`).

Extras used by MainActivity routing (`MainActivity.kt` lines 462 to 468, 475, 480, 508, 526, 528, 535, 537): `"notificationId"`, `"groupId"`, `"manga"`, `"anime"`, `"type"`, `"query"`, `"filter"`. Constants in `MainActivity` companion (lines 591 to 595): `INTENT_SEARCH = "eu.kanade.tachiyomi.SEARCH"`, `INTENT_ANIMESEARCH = "eu.kanade.tachiyomi.ANIMESEARCH"`, `INTENT_SEARCH_QUERY = "query"`, `INTENT_SEARCH_FILTER = "filter"`, `INTENT_SEARCH_TYPE = "type"`.

Extras set directly by the content-intent builders (not through the receiver): `"manga"` and `"chapter"` for `ReaderActivity.newIntent` (`ReaderActivity.kt:109` to `:110`); `"animeId"`, `"episodeId"`, `"hostIndex"`, `"vidIndex"`, `"hostList"` for `PlayerActivity.newIntent` (`PlayerActivity.kt:167` to `:171`); `Constants.MANGA_EXTRA` (`"manga"`), `Constants.ANIME_EXTRA` (`"anime"`), `"notificationId"` (= entry hashcode), `"groupId"` for the entry builders (`NotificationReceiver.kt:635` to `:637`, `:701` to `:702`, `:802`, `:821`).

Other extras: `Intent.EXTRA_TEXT` and `SearchManager.QUERY` (search), `Intent.EXTRA_RETURN_RESULT` (extension install activity, `true`), `Intent.EXTRA_STREAM` and `EXTRA_TEXT` (share, `IntentExtensions.kt:19` to `:23`). `ServiceExtras`: `HttpServerService.EXTRA_SOURCE_ID = "HttpServerSerivce.EXTRA.SOURCE.ID"` (`HttpServerService.kt:110`; the name `HttpServerSerivce` is misspelled in the source and must be kept only as a historical identifier).

### 3.2 Notification channels (observed, `Notifications.kt:116` to `:291`)

Importance values are `NotificationManagerCompat` constants: `IMPORTANCE_LOW = 2`, `IMPORTANCE_DEFAULT = 3`, `IMPORTANCE_HIGH = 4`.

| Channel id | Importance | Name (MR or AYMR key → English) | Group | Badge | Sound | Creator and use |
|---|---|---|---|---|---|---|
| `common_channel` | LOW (2) | `channel_common` → "Common" | none | default | default | `CHANNEL_COMMON`; `Notifications.kt:116` (generic) |
| `library_progress_channel` | LOW (2) | `channel_progress` → "Progress" | `group_library` | off | default | library progress (`CHANNEL_LIBRARY_PROGRESS`); also in the deprecated list (defect E-02) |
| `library_errors_channel` | LOW (2) | `channel_errors` → "Errors" | `group_library` | off | default | library errors |
| `new_chapters_episodes_channel` | DEFAULT (3) | AYMR `channel_new_chapters_episodes` → "Chapter/Episode updates" | none | default | default | new chapters and episodes (4 users) |
| `downloader_progress_channel` | LOW (2) | `channel_progress` → "Progress" | `group_downloader` | off | default | download progress |
| `downloader_error_channel` | LOW (2) | `channel_errors` → "Errors" | `group_downloader` | off | default | download errors |
| `backup_restore_progress_channel` | LOW (2) | `channel_progress` → "Progress" | `group_backup_restore` | off | default | backup and restore progress |
| `backup_restore_complete_channel_v2` | HIGH (4) | `channel_complete` → "Complete" | `group_backup_restore` | off | none (`setSound(null, null)`) | backup and restore completion (v2 id) |
| `incognito_mode_channel` | LOW (2) | `pref_incognito_mode` → "Incognito mode" | none | default | default | incognito notice |
| `torrent_server_channel` | LOW (2) | AYMR `pref_category_torrentserver` → "Torrent Server Preferences" | none | off | default | torrent server (the name string is the preference category title, a known mismatch) |
| `http_server_channel` | LOW (2) | AYMR `pref_http_server_name` → "Http server" | none | off | default | HTTP server |
| `app_apk_update_channel` | DEFAULT (3) | `channel_app_updates` → "App updates" | `group_apk_updates` | default | default | app update |
| `ext_apk_update_channel` | DEFAULT (3) | `channel_ext_updates` → "Extension updates" | `group_apk_updates` | default | default | extension updates and the extension install service (`ID_EXTENSION_INSTALLER`) |

Channel groups (`Notifications.kt:214` to `:229`):

| Group id | Name key → English |
|---|---|
| `group_backup_restore` | `label_backup` → "Backup and restore" |
| `group_downloader` | `download_notifier_downloader_title` → "Downloader" |
| `group_library` | `label_library` → "Library" |
| `group_apk_updates` | `label_recent_updates` → "Updates" |

Deprecated channel ids deleted at every start (`Notifications.kt:190` to `:200`): `downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel`.

Note: the channel count of the current set is 13 and the number of groups is 4. `CHANNEL_NEW_CHAPTERS_EPISODES` has group none; `GROUP_NEW_CHAPTERS` and `GROUP_NEW_EPISODES` (`Notifications.kt:146` to `:147`) are group keys used on posted notices (`NotificationCompat.setGroup`), not channel groups.

### 3.3 Notification ids (observed, `Notifications.kt`)

| Constant | Value | Used for | Consumers |
|---|---|---|---|
| `ID_APP_UPDATER` | 1 | app update download foreground and progress | `AppUpdateDownloadJob.kt:63`, `AppUpdateNotifier.kt:29` (default) |
| `ID_APP_UPDATE_PROMPT` | 2 | update available or download complete | `AppUpdateNotifier.kt:140,144` |
| `ID_DOWNLOAD_IMAGE` | 2 | save image progress (`SaveImageNotifier.kt:28`) | **collides with** `ID_APP_UPDATE_PROMPT` (defect E-19) |
| `ID_APP_UPDATE_ERROR` | 3 | download error | `AppUpdateNotifier.kt:168,171` |
| `ID_LIBRARY_PROGRESS` | -101 | library progress | library notifiers (8 uses) |
| `ID_LIBRARY_ERROR` | -102 | library errors | library notifiers (2) |
| `ID_LIBRARY_SIZE_WARNING` | -103 | bulk update warning | `AnimeLibraryUpdateNotifier.kt:120,149`, `MangaLibraryUpdateNotifier.kt:131` |
| `ID_DOWNLOAD_CHAPTER_PROGRESS` | -201 | manga download progress | 4 uses |
| `ID_DOWNLOAD_CHAPTER_ERROR` | -202 | manga download errors | 2 uses |
| `ID_DOWNLOAD_EPISODE_PROGRESS` | -203 | anime download progress | 4 uses |
| `ID_DOWNLOAD_EPISODE_ERROR` | -204 | anime download errors | 2 uses |
| `ID_NEW_CHAPTERS` | -301 | new chapters group and summary | `MangaLibraryUpdateNotifier.kt:176,266,276,289`, `ReaderActivity.kt:173`, `AnimeLibraryUpdateNotifier.kt:303` |
| `ID_NEW_EPISODES` | -1301 | new episodes group and summary | `AnimeLibraryUpdateNotifier.kt:194,280,290`, `PlayerActivity.kt:198` |
| `ID_BACKUP_PROGRESS` | -501 | backup progress | 5 uses |
| `ID_BACKUP_COMPLETE` | -502 | backup complete | 2 uses |
| `ID_RESTORE_PROGRESS` | -503 | restore progress | 6 uses |
| `ID_RESTORE_COMPLETE` | -504 | restore complete | 2 uses |
| `ID_INCOGNITO_MODE` | -701 | incognito notice | `App.kt:118,136` |
| `ID_TORRENT_SERVER` | -801 | torrent server foreground | `TorrentServerService.kt:89,131,136` |
| `ID_HTTP_SERVER` | -901 | HTTP server foreground | `HttpServerService.kt:90,95` |
| `ID_UPDATES_TO_EXTS` | -401 | extension updates available | `ExtensionUpdateNotifier.kt:21,47` |
| `ID_EXTENSION_INSTALLER` | -402 | extension install foreground | `MangaExtensionInstallService.kt:35`, `AnimeExtensionInstallService.kt:35` |
| `ID_NEW_CHAPTERS` as group | -301 | group summary key used with `groupId` | `NotificationReceiver.dismissNotification` |

Per-entry ids: `entry.id.hashCode()` (Kotlin `Long.hashCode`, `(int)(v ^ (v >>> 32))`) for the manga or anime notice, and the same value as `notificationId` and as request code.

### 3.4 Desktop mapping constants (Proposal)

The stable string ids replace the integers (area:35 section 5 decision 2). The table in `p1-tasks.md` T-53a-020 has the complete list. Summary: `notice:app-update-prompt` (was 2), `notice:app-update-progress` (was 1), `notice:app-update-error` (was 3), `notice:save-image` (was 2, separated), `notice:library-progress`, `notice:library-error`, `notice:library-size-warning`, `notice:download-progress`, `notice:download-error`, `notice:new-chapters`, `notice:new-episodes`, `notice:backup-progress`, `notice:backup-complete`, `notice:restore-progress`, `notice:restore-complete`, `notice:incognito`, `notice:torrent-server`, `notice:http-server`, `notice:extensions-updates`, `notice:extension-install`. Per-entry notices use `notice:entry:<kind>:<id>`.

## 4. Launch requests and the route table (observed + Proposal)

### 4.1 Shortcut constants (`core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt`)

| Constant | Value | Kotlin line | Route (MainActivity line) |
|---|---|---|---|
| `MANGA_EXTRA` | `"manga"` | 7 | extra (475) |
| `ANIME_EXTRA` | `"anime"` | 9 | extra (480) |
| `MAIN_ACTIVITY` | `"eu.kanade.tachiyomi.ui.main.MainActivity"` | 11 | unused in scope |
| `SHORTCUT_ANIMELIB` | `"eu.kanade.tachiyomi.SHOW_ANIMELIB"` | 14 | `HomeScreen.Tab.AnimeLib()` (472) |
| `SHORTCUT_LIBRARY` | `"eu.kanade.tachiyomi.SHOW_LIBRARY"` | 15 | `HomeScreen.Tab.Library()` (473) |
| `SHORTCUT_ANIME` | `"eu.kanade.tachiyomi.SHOW_ANIME"` | 16 | `HomeScreen.Tab.AnimeLib(id)` after `popUntilRoot` (479 to 483) |
| `SHORTCUT_MANGA` | `"eu.kanade.tachiyomi.SHOW_MANGA"` | 17 | `HomeScreen.Tab.Library(id)` after `popUntilRoot` (474 to 478) |
| `SHORTCUT_UPDATES` | `"eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED"` | 18 | `HomeScreen.Tab.Updates` (484) |
| `SHORTCUT_HISTORY` | `"eu.kanade.tachiyomi.SHOW_RECENTLY_READ"` | 19 | `HomeScreen.Tab.History` (485) |
| `SHORTCUT_SOURCES` | `"eu.kanade.tachiyomi.SHOW_CATALOGUES"` | 20 | `HomeScreen.Tab.Browse(false)` (486) |
| `SHORTCUT_ANIMEEXTENSIONS` | `"eu.kanade.tachiyomi.ANIMEEXTENSIONS"` | 21 | `HomeScreen.Tab.Browse(true, true)` (487) |
| `SHORTCUT_EXTENSIONS` | `"eu.kanade.tachiyomi.EXTENSIONS"` | 22 | `HomeScreen.Tab.Browse(true)` (488) |
| `SHORTCUT_ANIME_DOWNLOADS` | `"eu.kanade.tachiyomi.SHOW_ANIME_DOWNLOADS"` | 23 | `popUntilRoot`, `HomeScreen.Tab.More(toDownloads = true)` (493 to 496) |
| `SHORTCUT_DOWNLOADS` | `"eu.kanade.tachiyomi.SHOW_DOWNLOADS"` | 24 | `popUntilRoot`, `HomeScreen.Tab.More(toDownloads = true)` (489 to 492) |
| `URL_HELP` | `"https://aniyomi.org/docs/guides/troubleshooting/"` | 5 | not in scope (menu help) |
| `URL_HELP_UPCOMING` | `"https://aniyomi.org/docs/faq/updates/upcoming"` | 6 | not in scope |

Two `SHORTCUT_*` constants are not in `shortcuts.xml`: `SHORTCUT_DOWNLOADS` and `SHORTCUT_ANIME_DOWNLOADS` are sent only by notices (`NotificationHandler.kt:24` and `:42`), and `SHORTCUT_EXTENSIONS`, `SHORTCUT_ANIMEEXTENSIONS` are sent only by `ExtensionUpdateNotifier.kt:38` and `:40`. `SHORTCUT_MANGA` and `SHORTCUT_ANIME` are sent only by notices (`NotificationReceiver.kt:633` and `:698`, `:799` and `:818`).

### 4.2 Route output (observed behaviour) and Proposal mapping

| Input | Observed output | Proposal (`LaunchRequest` in area:36) |
|---|---|---|
| action `SHOW_LIBRARY` | tab Library | `Shortcut { action: Library, id: None }` |
| action `SHOW_ANIMELIB` | tab AnimeLib | `Shortcut { AnimeLib, None }` |
| action `SHOW_RECENTLY_UPDATED` | tab Updates | `Shortcut { Updates, None }` |
| action `SHOW_RECENTLY_READ` | tab History | `Shortcut { History, None }` |
| action `SHOW_CATALOGUES` | Browse (anime false) | `Shortcut { Catalogues, None }` |
| action `EXTENSIONS` | Browse manga extensions tab | `Shortcut { Extensions, None }` |
| action `ANIMEEXTENSIONS` | Browse anime extensions tab | `Shortcut { AnimeExtensions, None }` |
| action `SHOW_MANGA` with `manga` | Library tab at entry | `Shortcut { MangaEntry, Some(id) }` (missing id: `RouteError::MissingId`) |
| action `SHOW_ANIME` with `anime` | AnimeLib tab at entry | `Shortcut { AnimeEntry, Some(id) }` |
| action `SHOW_DOWNLOADS` or `SHOW_ANIME_DOWNLOADS` | More with downloads | `Shortcut { Downloads, None }` and `Shortcut { AnimeDownloads, None }` |
| action `eu.kanade.tachiyomi.SEARCH` with `query`, `filter` | manga global search | `ExtSearch { Manga, query, filter }` |
| action `eu.kanade.tachiyomi.ANIMESEARCH` with `query`, `filter` | anime global search | `ExtSearch { Anime, query, filter }` |
| action `SEARCH`, `SEND`, `SEARCH_ACTION` with `query` or `EXTRA_TEXT` and `type` | global search plus deep-link page for `type` (default ANIME) | `Search { query, kind }` (kind default Anime) |
| VIEW `*.tachibk` | restore backup screen | `OpenFile(path)` |
| VIEW `tachiyomi://add-repo?url=U` | manga repos with U | `OpenUri(url)` (route Manga extension repos) |
| VIEW `aniyomi://add-repo?url=U` | anime extension stores with U | `OpenUri(url)` (route Anime extension stores) |
| VIEW `aniyomi://extension-store?url=U` | anime extension stores with U | `OpenUri(url)` |
| VIEW `*-auth` hosts | TrackLoginActivity | `OpenUri(url)` (route tracker OAuth, area:27) |
| notice content intent with `notificationId`, `groupId` | withdraw notice | `Notification { id, group_id }` (area:35: withdraw by id) |

Important mismatch (defect E-17): `DeepLinkMangaActivity` forwards `eu.kanade.tachiyomi.ANIMESEARCH` with `type=MANGA` but MainActivity routes that action to the anime search regardless of `type`.

### 4.3 MainActivity "ready" flag

`handleIntentAction` returns `true` (and sets `ready = true`, `MainActivity.kt:571`) for every handled case, and `false` for unhandled actions and for `SHORTCUT_MANGA`/`SHORTCUT_ANIME` without an id (`:475`, `:480`). Proposal: the `ready` value is the second element of `route_request` (area:36): `false` means "show the default home page and keep the request pending until the window is shown", which the GTK app does by default (no pending state needed).

### 4.4 Launcher shortcuts (`app/shortcuts.xml`)

| shortcutId | enabled | icon | shortcutShortLabel and LongLabel | Intent action |
|---|---|---|---|---|
| `show_library` | true | `@drawable/sc_collections_bookmark_48dp` | `label_manga_library` ("Manga") | `eu.kanade.tachiyomi.SHOW_LIBRARY` |
| `show_animelib` | true | `@drawable/sc_collections_baseline_48dp` | `label_anime_library` ("Anime") | `eu.kanade.tachiyomi.SHOW_ANIMELIB` |
| `show_recently_updated` | true | `@drawable/sc_new_releases_48dp` | `label_recent_updates` ("Updates") | `eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED` |
| `show_recently_updated` (duplicate) | true | `@drawable/sc_history_48dp` | `label_recent_manga` ("History") | `eu.kanade.tachiyomi.SHOW_RECENTLY_READ` |
| `show_catalogues` | true | `@drawable/sc_explore_48dp` | `browse` ("Browse") | `eu.kanade.tachiyomi.SHOW_CATALOGUES` |

`android:shortcutDisabledMessage="@string/app_not_available"` on each (English "App not available"). `android:targetClass="eu.kanade.tachiyomi.ui.main.MainActivity"` on each intent.

Desktop action set (Proposal): see behaviour section 2.4 and `p1-tasks.md` T-53a-011.

## 5. Mapping Kotlin notification actions to AppAction (Proposal, area:35 section 3.1)

| Kotlin action / builder | AppAction variant (`src/notify/actions.rs`) | Payload |
|---|---|---|
| `ACTION_DISMISS_NOTIFICATION` (dismiss id) | `Dismiss { notice: NoticeId }` | id only; never bulk |
| `ACTION_RESUME_DOWNLOADS` / `ACTION_RESUME_ANIME_DOWNLOADS` | `ResumeDownloads { kind: Manga or Anime }` | none |
| `ACTION_PAUSE_*` | `PauseDownloads { kind }` | none |
| `ACTION_CLEAR_*` | `ClearDownloads { kind }` | none |
| `ACTION_MARK_AS_READ` | `MarkChaptersRead { manga_id, urls: Vec<String>, notice: Option<NoticeId>, group: Option<NoticeId> }` | URLs validated, max 500 entries (Proposal) |
| `ACTION_MARK_AS_SEEN` | `MarkEpisodesSeen { anime_id, urls, notice, group }` | as above |
| `ACTION_DOWNLOAD_CHAPTER` | `DownloadChapters { manga_id, urls, notice, group }` | as above |
| `ACTION_DOWNLOAD_EPISODE` | `DownloadEpisodes { anime_id, urls, notice, group }` | as above |
| `openMangaEntryPendingActivity` / `openAnimeEntryPendingActivity` / group variants | `OpenManga { id, notice }` / `OpenAnime { id, notice }` | id, optional notice to withdraw |
| `openChapterPendingActivity` (both) | `OpenChapter { manga_id, chapter_id, notice }` | ids |
| `openEpisodePendingActivity` (both) | `OpenEpisode { anime_id, episode_id, notice }` | ids |
| `ACTION_CANCEL_LIBRARY_UPDATE` | `CancelLibraryUpdate { kind: Manga }` | none |
| `ACTION_CANCEL_ANIMELIB_UPDATE` | `CancelLibraryUpdate { kind: Anime }` | none |
| `ACTION_CANCEL_RESTORE` | `CancelRestore` | none |
| `ACTION_START_APP_UPDATE` | `StartAppUpdate { url: Url }` | URL validated (https only, Proposal) |
| `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | `CancelAppUpdate` | none |
| `ACTION_SHARE_IMAGE` | `RevealFile { path }` | path under the app's cache or download folders only |
| `ACTION_SHARE_BACKUP` | `RevealFile { path }` | as above |
| `openErrorLogPendingActivity` | `OpenFile { path }` (text editor via Opener) | path under log folder |
| `openImagePendingActivity` | `OpenFile { path }` | image path |
| `openUrl` (help warnings) | `OpenUrl { url: Url }` | https only |
| `openDownloadManagerPendingActivity` / anime | `Shortcut { Downloads }` / `Shortcut { AnimeDownloads }` | none |
| `openExtensionsPendingActivity` / anime | `Shortcut { Extensions }` / `Shortcut { AnimeExtensions }` | none |
| `installApkPendingActivity` | dropped | n/a |
| incognito content intent | `DisableIncognito` | none |

## 6. Files, folders and formats (observed and Proposal)

| Item | Observed location or format | Proposal |
|---|---|---|
| APK for the app update | `File(context.externalCacheDir, "update.apk")` (`AppUpdateDownloadJob.kt:106`) | `Paths::cache()/update/update.apk` (area:35, `Paths` trait) |
| Backup share MIME | `application/x-protobuf+gzip` (`NotificationReceiver.kt:93`) | keep the MIME in the backup format spec (area:21) |
| Backup file pattern | `*.tachibk` (manifest pathPatterns; `MainActivity.kt:545`) | MIME `application/x-tachibk`, glob `*.tachibk`, subclass of `application/gzip` (area:36 section 4) |
| Error log view | `text/plain` ACTION_VIEW (`NotificationReceiver.kt:973`) | `OpenFile` with a text/plain handler |
| Image view | `image/*` ACTION_VIEW (`NotificationHandler.kt:60`) | `OpenFile` |
| Torrent server working dir | `filesDir.absolutePath` (`TorrentServerService.kt:73`) | `Paths::data()/torrserver` (area:13) |
| FileProvider authority | `<packageName>.provider` (`FileExtensions.kt:17`) | none (dropped) |
| Cached-HTTP-free download | `newCachelessCallWithProgress` (`AppUpdateDownloadJob.kt:102`) | reqwest with cache disabled (area:12) |

## 7. Network endpoints and URLs (observed)

### 7.1 Help and documentation links used by scope code

| Constant | URL | Used by |
|---|---|---|
| `MangaLibraryUpdateNotifier.HELP_WARNING_URL` | `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads` (`MangaLibraryUpdateNotifier.kt:392`) | `openUrl` in `MangaLibraryUpdateNotifier.kt:141`, `MangaDownloader.kt:321` (via `NotificationHandler.openUrl`) |
| `AnimeLibraryUpdateNotifier.HELP_WARNING_URL` | same value (`AnimeLibraryUpdateNotifier.kt:405`) | `AnimeLibraryUpdateNotifier.kt:131,159`, `AnimeDownloader.kt:327` |
| `Constants.URL_HELP` | `https://aniyomi.org/docs/guides/troubleshooting/` | not in scope (menu) |

Proposal: the help URLs remain as data constants in `kuro-core::links` and point to the Aniyomi documentation (the feature is the same). Opening them uses `Opener::open_url` (area:36 Opener).

### 7.2 Update download

- Request: `GET` to `release.downloadLink` (the URL of the APK asset from the release check in area:35), via `network.client.newCachelessCallWithProgress(GET(url), progressListener)` (`AppUpdateDownloadJob.kt:102`).
- Release page: `release.releaseLink` opened with `ACTION_VIEW` (`AppUpdateNotifier.kt:45`).
- Redirect and TLS: default OkHttp behaviour (follow redirects), plus the network security config of section 8.

### 7.3 Network security config (observed, `app/src/main/res/xml/network_security_config.xml`)

```
base-config cleartextTrafficPermitted="true"
trust-anchors: certificates src="system"; certificates src="user"
```

Proposal: reqwest with rustls, `http://` allowed, root store = system roots plus user store (area:12).

### 7.4 Tracker OAuth redirect hosts (observed, `TrackLoginActivity.kt`)

| Host | Parameter read | Action |
|---|---|---|
| `anilist-auth` | fragment `access_token` (regex `(?:access_token=)(.*?)(?:&)`) | `login(token)` or `logout()` |
| `bangumi-auth` | query `code` | `login(code)` or `logout()` |
| `myanimelist-auth` | query `code` | `login(code)` or `logout()` |
| `shikimori-auth` | query `code` | `login(code)` or `logout()` |
| `simkl-auth` | query `code` | `login(code)` or `logout()` |

Proposal: the scheme is `kurobayomi` with the same hosts (`kurobayomi://anilist-auth#access_token=...`); `aniyomi` stays as an alias. Whether each provider accepts a new scheme is an open question shared with area:27 (topic file section 6).

## 8. Preference keys used in the scope (observed, with defaults)

| Key (literal) | Type | Default | Read or written in | Meaning | Kurobayomi decision |
|---|---|---|---|---|---|
| `__APP_STATE_incognito_mode` (from `appStateKey("incognito_mode")`, `BasePreferences.kt:21`) | Boolean | `false` | `App.kt:113`, `MainActivity.kt:177,216`, receiver incognito branch | incognito mode on or off; reset on each launch | keep under app-state prefix (area:02) |
| `__APP_STATE_onboarding_complete` (`BasePreferences.kt:29`) | Boolean | `false` | `shownOnboardingFlow()` (onboarding in `MainActivity`) | onboarding finished | area:36 |
| `extension_installer` (`ExtensionInstallerPreference.kt:19`) | enum `ExtensionInstaller` | `LEGACY` if MIUI installer present, else `PACKAGEINSTALLER` (`ExtensionInstallerPreference.kt:29` to `:33`) | installer selection | dropped (APK install) |
| `pref_remove_after_marked_as_read_key` (`DownloadPreferences.kt:30` to `:33`) | Boolean | `false` | receiver mark-read and mark-seen | delete downloads when marked read or seen | keep (area:23 and area:24) |
| `pref_remove_bookmarked` (`DownloadPreferences.kt`) | Boolean | `false` | not in scope | | area:23 |
| `remove_after_read_slots` (`DownloadPreferences.kt:28`) | Int | `-1` | not in scope | | area:23 |
| `pref_hardware_bitmap_threshold` (`BasePreferences.kt:33`) | Int | `GLUtil.SAFE_TEXTURE_LIMIT` (set once to `GLUtil.DEVICE_TEXTURE_LIMIT` in `App.kt:141` to `:143`) | `App.kt` | image decode threshold | dropped with GL (area:34) |
| `pref_always_decode_long_strip_with_ssiv` (`BasePreferences.kt:35`) | Boolean | `false` | reader | not in scope | area:43 |
| `pref_display_profile_key` (`BasePreferences.kt:31`) | String | `""` | display profile | not in scope | area:49 |
| `__APP_STATE_last_version_code` (`App.kt:169`, from `appStateKey`) | Int | `0` | migrator | migration gate | area:11 |
| `ExtensionInstaller`-independent torrent keys | | | `TorrentPreferences` (`torrServerPort`, `torrServerProxyMode`, `torrServerProxyUrl`) | torrent server port and proxy | area:13 |
| verbose logging (`networkPreferences.verboseLogging()`) | Boolean | from area:02 | `App.kt:160`, `TorrentServerService.kt:66` | logcat and TorrServer log callback | area:02 |

Preference key derivation: `Preference.appStateKey(key) = "__APP_STATE_" + key` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43` to `:47`).

## 9. Database effects of the receiver (observed)

Tables and columns touched by the mark actions, through `GetChapter`, `GetEpisode`, `UpdateChapter`, `UpdateEpisode`:

| Table | Columns written | Triggers that fire (SQLDelight) | Source |
|---|---|---|---|
| `chapters` (manga) | `read` (set true), `last_modified_at` (by trigger) | `update_last_modified_at_chapters` sets `last_modified_at = strftime('%s','now')` (epoch seconds, text of integer); `update_chapter_and_manga_version` (when `is_syncing = 0` and `read` changes) increments `chapters.version` and `mangas.version` (when the manga is not syncing) | `data/src/main/sqldelight/data/chapters.sq` (trigger block at lines 26 to 35 and 36 to 48) |
| `episodes` (anime) | `seen` (set true), `last_modified_at` (by trigger) | `update_last_modified_at_episodes`, `update_episode_and_anime_version` (when `seen` changes, increments `episodes.version` and the anime version) | `data/src/main/sqldelightanime/dataanime/episodes.sq` (lines 36 to 50) |
| `chapters` / `episodes` (delete path) | download files only (no row delete); the download manager removes files, area:23 and area:24 | none | `DeleteChapterDownload`, `DeleteEpisodeDownload` (area:23/24) |

Queries used through interactors (observed names): `getChapter.await(url, mangaId)` (`GetChapter`), `getEpisode.await(url, animeId)` (`GetEpisode`), `getManga.await(id)`, `getAnime.await(id)`, `updateChapter.awaitAll(list)` (`UpdateChapter`), `updateEpisode.awaitAll(list)` (`UpdateEpisode`). The `ChapterUpdate` / `EpisodeUpdate` shapes come from `toChapterUpdate()` and `toEpisodeUpdate()` (`tachiyomi.domain.items.chapter.model`, `tachiyomi.domain.items.episode.model`).

Deep-link models add these reads and writes (observed): `getAnimeByUrlAndSourceId`, `networkToLocalAnime` (insert), `getEpisodeByUrlAndAnimeId` (read), `updateAnimeFromRemote.awaitEpisodesUpdate(anime, fetchEpisodes = true)` (write new episodes), `getMangaByUrlAndSourceId`, `networkToLocalManga` (insert), `getChapterByUrlAndMangaId` (read), `syncChaptersWithSource.await(sourceChapters, manga, source, false)` (write chapters).

## 10. Resource identifiers (observed)

### 10.1 Layout binding names (ViewBinding classes generated from `res/layout`)

| Layout file | Binding class (observed usage) | Users |
|---|---|---|
| `reader_activity.xml` | `ReaderActivityBinding` | `ReaderActivity.kt` |
| `reader_error.xml` | `ReaderErrorBinding` | `PagerPageHolder.kt`, `WebtoonPageHolder.kt` (under `ui/reader/viewer`) |
| `player_layout.xml` | `PlayerLayoutBinding` | `PlayerActivity.kt` |
| `download_list.xml` | `DownloadListBinding` | `AnimeDownloadQueueScreen.kt`, `AnimeDownloadQueueScreenModel.kt`, `MangaDownloadQueueScreen.kt`, `MangaDownloadQueueScreenModel.kt` |
| `download_item.xml` | `DownloadItemBinding` | `AnimeDownloadHolder.kt`, `MangaDownloadHolder.kt` |
| `download_header.xml` | `DownloadHeaderBinding` | `AnimeDownloadHeaderHolder.kt`, `MangaDownloadHeaderHolder.kt` |
| `pref_widget_switch_material.xml` | none | none (dead) |

### 10.2 Menu and animation references

| Resource | Reference (observed) |
|---|---|
| `R.menu.download_single` | `AnimeDownloadHolder.kt:100`, `MangaDownloadHolder.kt:90` |
| `R.anim.shared_axis_x_push_enter` / `_exit` | `ReaderActivity.kt:150,151,155`; `WebViewActivity.kt:46,47,51` |
| `R.anim.shared_axis_x_pop_enter` / `_exit` | `ReaderActivity.kt:299,300,304`; `WebViewActivity.kt:103,104,108` |
| `R.xml.searchable` | manifest `@xml/searchable` only |
| `R.xml.s_pen_actions` | manifest only |
| `R.xml.provider_paths` | manifest only |
| `R.xml.shortcuts` | manifest only |
| `R.xml.network_security_config` | manifest only |
| `R.xml.locales_config` | manifest and `AppLanguageScreen.kt` |
| `R.menu.chapter_download`, `R.menu.main_nav`, `R.raw.keep`, `R.anim.player_*` | no reference |

### 10.3 Strings with exact English values used by the scope

| Key (module) | English value | Used at |
|---|---|---|
| `app_name` (i18n, unTranslatable) | Aniyomi | manifest label, `AppUpdateDownloadJob.kt:42`, torrent notice title |
| `action_add_repo` (i18n) | Add repo | manifest label |
| `action_addExtensionStore` (manifest reference) | Add extension store. The moko source key is `action.addExtensionStore` in `i18n/src/commonMain/moko-resources/base/strings.xml:374` (dot form); the manifest uses the underscore form, which the Android resource build derives (not verifiable from the source tree alone; the English value is taken from the moko source) | manifest labels of the two `add-repo` and `extension-store` filters |
| `pref_restore_backup` (i18n) | Restore backup | manifest label |
| `action_global_anime_search` (i18n) | Global Anime Search | manifest label |
| `action_global_manga_search` (i18n) | Global Manga Search | manifest label |
| `track_activity_name` (i18n) | Tracker login | manifest label |
| `action_global_search_hint` (i18n) | Global search… | `searchable.xml` |
| `label_manga_library` (i18n) | Manga | shortcut label |
| `label_anime_library` (i18n) | Anime | shortcut label |
| `label_recent_updates` (i18n) | Updates | shortcut label, channel group |
| `label_recent_anime_updates` (i18n) | Anime Updates | widget label |
| `label_recent_manga` (i18n) | History | shortcut label |
| `app_not_available` (i18n) | App not available | shortcut disabled message |
| `browse` (i18n) | Browse | shortcut label |
| `label_more` (i18n) | More | `main_nav` only (dropped) |
| `action_start_downloading_now` (i18n) | Start downloading now | `chapter_download` menu (dropped) |
| `action_start_download_externally` / `_internally` (i18n) | Use external downloader / Use internal downloader | dropped menu |
| `action_delete` (i18n) | Delete | dropped menu, and download delete |
| `action_cancel` (i18n) | Cancel | download row, app update |
| `action_move_to_top` (i18n) | Move to top | `download_single` |
| `action_move_to_top_all_for_series` (i18n) | Move series to top | `download_single` |
| `action_move_to_bottom` (i18n) | Move to bottom | `download_single` |
| `action_move_to_bottom_all_for_series` (i18n) | Move series to bottom | `download_single` |
| `cancel_all_for_series` (i18n) | Cancel all for this series | `download_single` |
| `action_webview_back` (i18n) | Back | `s_pen_actions` (dropped) |
| `action_next_chapter` (i18n) | Next chapter | `s_pen_actions` (dropped) |
| `action_previous_chapter` (i18n) | Previous chapter | `s_pen_actions` (dropped) |
| `spen_next_page` (i18n) | Next page | `s_pen_actions` (dropped) |
| `spen_previous_page` (i18n) | Previous page | `s_pen_actions` (dropped) |
| `decode_image_error` (i18n) | The image couldn't be loaded | `reader_error.xml` |
| `action_retry` (i18n) | Retry | `reader_error.xml`, app update error |
| `action_open_in_web_view` (i18n) | Open in WebView | `reader_error.xml` |
| `action_search_hint` (i18n) | Search… | deep-link page title |
| `action_menu` (i18n) | Menu | `download_item.xml` content description |
| `action_close` (i18n) | Close | HTTP server notice action |
| `action_share` (i18n) | Share | share chooser title |
| `action_download` (i18n) | Download | app update prompt action |
| `whats_new` (i18n) | What's new | app update prompt action |
| `channel_common` (i18n) | Common | channel name |
| `channel_progress` (i18n) | Progress | channel name (four channels) |
| `channel_errors` (i18n) | Errors | channel name (three channels) |
| `channel_complete` (i18n) | Complete | backup complete channel |
| `channel_app_updates` (i18n) | App updates | channel name |
| `channel_ext_updates` (i18n) | Extension updates | channel name |
| `pref_incognito_mode` (i18n) | Incognito mode | channel name, incognito notice title |
| `notification_incognito_text` (i18n) | Disable incognito mode | incognito notice text |
| `label_backup` (i18n) | Backup and restore | channel group |
| `label_library` (i18n) | Library | channel group |
| `download_notifier_downloader_title` (i18n) | Downloader | channel group |
| `connected_to_wifi` (i18n) | Only on Wi-Fi | download preference |
| `update_check_notification_update_available` (i18n) | New version available! | app update prompt |
| `update_check_notification_download_in_progress` (i18n) | Downloading… | app update progress |
| `update_check_notification_download_complete` (i18n) | Tap to install update | app update complete |
| `update_check_notification_download_error` (i18n) | Download error | app update error |
| `unlock_app_title` (i18n) | Unlock %s | unlock prompt |
| `download_error` (i18n-aniyomi, AYMR) | Error | receiver toast for missing entities |
| `channel_new_chapters_episodes` (i18n-aniyomi) | Chapter/Episode updates | channel name |
| `pref_category_torrentserver` (i18n-aniyomi) | Torrent Server Preferences | torrent channel name |
| `pref_http_server_name` (i18n-aniyomi) | Http server | HTTP channel name |
| `torrentserver_is_running` (i18n-aniyomi) | Torrent Server is running (multi-line value) | torrent notice text |
| `http_server_is_running` (i18n-aniyomi) | Http server is running | HTTP notice text |
| `http_server_start_failure` (i18n-aniyomi) | Failed to start http server | toast when start fails |
| `ext_install_service_notif` (i18n-aniyomi) | Installing extension… | extension install notice |

The torrent notice's "Stop" action label is a hard-coded English literal (`TorrentServerService.kt:124`), not a string resource (defect E-20).

## 11. Lines of code and file counts (observed)

| Group | Files | LOC |
|---|---|---|
| Kotlin (`app/src/main/java/...`) | 10 | 1664 |
| `app/src/main/AndroidManifest.xml` | 1 | 324 |
| `app/shortcuts.xml` | 1 | 57 |
| `presentation-widget/src/main/AndroidManifest.xml` | 1 | 73 |
| 10 empty module manifests | 10 | 20 |
| `res/anim-v33` | 4 | 54 |
| `res/anim` | 14 | 134 |
| `res/layout` | 7 | 249 |
| `res/menu` | 3 | 77 |
| `res/raw` | 1 | 3 |
| `res/xml` (non-manifest) | 4 | 100 |
| Total | 56 | 2755 |

Check: 1664 + 324 + 57 + 73 + 20 + 54 + 134 + 249 + 77 + 3 + 100 = 2755.
