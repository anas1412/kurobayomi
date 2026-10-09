# NOT-PORTING: Aniyomi capabilities deliberately not ported to Kurobayomi

Status: synthesis, dated 2026-10-08. Every row was checked against the area overviews (`plans/NN-*/00-overview.md`), the P3 rows of `plans/TASKS.csv`, the Android API catalogs in `plans/catalogs/`, and `plans/addenda/requirements-cross-platform.md`. Source paths are repo-relative to the read-only Aniyomi clone at `/home/blackbox/aniyomi-src`. Kurobayomi paths are relative to `/home/blackbox/kurobayomi`.

Labels used in this document:

- **OBSERVED:** what the Kotlin code does, with the file and line where a line was checked.
- **Proposal:** what Kurobayomi should do instead.
- **Dropped:** the feature is removed. The user loses the stated behaviour.
- **Replaced:** the feature is removed and a named Linux or Rust mechanism covers the same job.
- **Degraded:** a smaller version ships.
- **Deferred:** not dropped, not in the P0 or P1 path, and still open for a later decision.
- **Open:** the plan contains two or more answers that disagree, or a question still needs the user's yes or no. Section 6 lists every one.

---

## 1. Scope and counts

### 1.1 What this document covers

Every Android-only, obsolete, or deliberately removed capability in the Aniyomi source, as recorded in the 54 area overviews and the 309 tasks whose priority is `P3`. A capability that is only deferred (P2 or later) is not a drop and appears in section 7 instead.

### 1.2 Counts

| Disposition | P3 tasks |
|---|---|
| Drop | 261 |
| Replace | 38 |
| Replace or drop, decision recorded | 2 |
| Keep, record, verify, defer | 8 |
| Total | 309 |

### 1.3 Plan defects found while writing this document

These are not capabilities. They are inconsistencies in the task graph that the drop decisions create.

1. Six P0-P2 tasks depend on a P3 drop task. P0 has none. The P1 and P2 ones are:

| Dependent task | Priority | Depends on (P3 drop) | What the dependent task needs from the drop |
|---|---|---|---|
| T-01a-025 | P2 | T-01a-024 (drop home-screen widget managers) | The launcher badge decision |
| T-36a-034 | P1 | T-36a-035 (secure screen flag, drop) | The secure-screen decision |
| T-40a-021 | P1 | T-40a-031 (extension app-info button, drop) | The extension-details action row |
| T-41a-020 | P1 | T-41a-024 (replace "App info" with "Open folder") | The replacement action |
| T-41a-023 | P1 | T-41a-046 (drop Fragment, reflection and Compose-only mechanics) | The source-preference host decision |
| T-45a-048 | P2 | T-45a-047 (PiP policy, dropped as Android PiP) | The PiP decision |

   Proposal: a P3 drop task is a decision record. Mark it done when the decision is written into this document, so the P1 and P2 dependents are unblocked. The dependency edge stays.


---

## 2. Decision classes and rules applied

1. **Android-only mechanism, no user-visible behaviour.** Dropped. Example: Conscrypt TLS provider insertion, T-01a-027. Nothing to replace because rustls or the system TLS through reqwest is used.
2. **Android-only mechanism with a user-visible job.** Replaced by a named Linux mechanism from section 5. Example: the Android notification channels are replaced by one `gio::Notification` per category, area:35-data-misc-updater-notification-export section 2.3.
3. **Feature whose job has no Linux counterpart.** Dropped, with the loss stated. Example: picture-in-picture, T-45a-047.
4. **Feature that depends on a third-party service.** Dropped or made opt-in, because the request leaves the user's machine. Example: the data-saver image proxies, section 4.2.
5. **Dead code.** Dropped with no loss. Example: `AnimeDownloadPart`, T-24a-003, which models an HTTP range that no caller reads.

Rules applied across the plan:

- Settings keys for dropped features are kept as readable keys so an existing Aniyomi backup still parses, and are never shown in the UI. Example: `pref_hardware_bitmap_threshold` is ignored and not migrated (area:13-core-util-storage-archive-torrent, D-F, T-13a-030). T-02a-032 applies the same rule to Android-only preferences.
- A dropped feature gets a line in the release notes with the reason, as the area:40-ui-browse-manga definition of done (item 13) requires.
- A P3 task never blocks a P0 task. Verified: no P0 task depends on a P3 task (section 1.3).

---

## 3. Headline losses the user will notice

These are the items a daily user will see. Each one has a task ID in section 4.

| # | Loss | Aniyomi behaviour | What Kurobayomi does | Related task IDs |
|---|---|---|---|---|
| L1 | Installed Aniyomi extensions stop working | APK extensions installed through the Android PackageInstaller or Shizuku, loaded by `dalvik.system` class loaders | No APK, dex, PackageInstaller, Shizuku, or NanoHTTPD path. Sources are built-in Rust sources, declarative TOML scrapers, or WASM components (area:20-extension-strategy-redesign section 4.1 item 1). Library rows keep their source ids, so a source reinstalled in Kurobayomi reattaches to the same library entries (item 3). Sources not yet ported show as stubs with `source_not_installed` (item 4). | T-19a-037 to T-19a-045, T-19a-071 to T-19a-075, T-20a-092, T-20a-093, T-18a-038, T-18a-041, T-18a-042, T-53a-036, T-53a-038 |
| L2 | Aniyomi backup extension list cannot be reinstalled | `ExtensionsRestorer.kt` writes each `BackupExtension` APK to cache and opens the Android installer (app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/ExtensionsRestorer.kt:82-99) | The count is reported as skipped (Q-19, T-22a-029). The user reinstalls extensions from the Kurobayomi extension store (area:22-backup-restore section 4.3). | T-22a-029 |
| L3 | Desktop widgets | Six Glance home and lock screen receivers, Samsung cover screen, widget picker previews | Updates count shown in the sidebar badge and tray or desktop notifications. A KDE Plasma applet is a later option (area:54-build-release-distribution-docs section 4). | T-01a-024, T-08a-038, T-50a-026 to T-50a-033, T-53a-068, T-54a-043 |
| L4 | Screenshot and recents-thumbnail protection | `FLAG_SECURE` is set through `app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt:8` when `secure_screen_v2` is on | Dropped. No GTK 4.16, Wayland, or X11 mechanism blocks capture of one window. The preference key is kept so existing settings still parse (area:52-util-app item 2). | T-01a-033, T-34a-064, T-36a-035, T-47a-066, T-52a-062 |
| L5 | Picture-in-picture for the player | Android PiP window, PiP remote actions, PiP icons | Dropped. The mpv window is a normal toplevel. The user can float it using the compositor. Optional compact mode is P3 (area:53-platform-manifest-intents-workers D6). | T-45a-047, T-46a-047, T-53a-033; PipActions.kt icons per area:49b section 4 item 11 |
| L6 | External downloader hand-off (IDM, ADM, 1DM) | Android intents to third-party downloaders | Dropped. Proposed replacement: aria2 over JSON-RPC, off by default (area:24-downloads-anime section 4 item 4). The aria2 option is P2 (T-24a-025). | T-09a-022, T-24a-024, T-47a-051 |
| L7 | Data-saver image proxies | Images rewritten through `bandwidth-hero`, `wsrv.nl`, or `resmush.it`. Enum values `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` at app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:120-122 | Dropped as a feature. Status of the enum and of `resmush.it` is an open question (section 6, O-3 and O-4). | T-09a-056, T-40a-042, T-40b-082, T-23a-017 |
| L8 | Biometric app lock | `AuthenticatorUtil.kt` and `UnlockActivity.kt` use `BiometricPrompt` to unlock after `lock_app_after` | Dropped as biometrics. A passphrase lock or PAM unlock is open (section 6, O-2). | T-36a-032, T-36a-033, T-36a-034, T-52a-033 |
| L9 | Hardware volume keys turn pages | Volume keys in the reader | Dropped. The preference key is kept. Arrow keys, Page Up and Page Down, and the mouse wheel replace them (area:43-reader-core section 4.4). | T-43a-071 |
| L10 | Screen orientation lock per manga | `ReaderOrientation.kt` | No effect on desktop (area:43-reader-core Q-3). | T-43a-068, T-48a-019 |
| L11 | Pull-to-refresh gesture | Swipe down to refresh | Replaced by a refresh button, F5 and Ctrl+R (area:49-theme-and-design-system D-8, T-49b-019). | T-38a-050 |
| L12 | Haptic feedback everywhere | `HapticFeedbackConstants` on long press, slider drag, and library add | Dropped. No portable haptics on Linux. | T-38a-047, T-39a-053, T-39b-067, T-40b-081, T-41b-055, T-42a-058, T-43a-070, T-44a-017, T-46a-045, T-49a-056 |
| L13 | Swipe-to-dismiss sheets | Compose sheets dismiss on swipe | Sheets close by their close button, Escape, or the scrim. | T-39b-068, T-46b-040 |
| L14 | Android share sheet | Share targets for URLs and images | Replaced by clipboard copy, `gtk::UriLauncher`, or "show in file manager". Three replacement texts exist and must be unified (section 6, O-8). | T-39b-051, T-39a-054; area:43 Q-4 |
| L15 | Assistant page context | `AssistContent` on entry, reader, and browse screens | Dropped. No Linux assistant API is in the plan. | T-36a-064, T-38a-048, T-39b-066, T-40b-080, T-41b-056 |
| L16 | Global search from the Android system search bar | `searchable.xml` hint and search provider | Dropped. In-app global search is unchanged (area:40-ui-browse-manga). | T-35a-029 |
| L17 | Launcher-wide deep-link trampolines | `DeepLinkAnimeActivity` and `DeepLinkMangaActivity` | Dropped as activities. `route_request` in `kuro-core` handles the same links (area:53-platform-manifest-intents-workers). `aniyomi://` and `tachiyomi://` are kept as parsed aliases in the first release, then dropped with a release note (53 section 4.2, item 3). | T-53a-046 |
| L18 | Galaxy S Pen remote actions | `s_pen_actions.xml` and `REMOTE_ACTION` filters | Dropped. Keyboard keys replace them. | T-35a-027, T-51a-037, T-53a-065 |
| L19 | In-app APK updater and preview channel | Download and install an APK, or the commit-count preview channel | Dropped. Distro packages or the Flatpak are the update path. The update prompt through the notifier stays (area:06 Q5, T-06a-073). | T-06a-073, T-07a-037, T-35a-056, T-35a-061 |
| L20 | Monet (wallpaper-derived colours) | Material You colours from `values-v31` | Dropped as dynamic colour. Accent colour from the desktop portal is an open option (section 6, O-1). | T-47a-012, T-48a-058, T-49ba-007, T-52a-063 |
| L21 | Seventeen named app palettes | `AppTheme.kt` | Dropped. The shipped look is the Tokyo Night theme (addenda rule 6). | T-47a-012 |
| L22 | Tablet UI mode | `TabletUiMode.kt` | Dropped. A width breakpoint replaces it (T-42a-055). | T-47a-017, T-48a-007 |
| L23 | Android home-screen and cover-screen update grid | Glance `BaseMangaUpdatesGridGlanceWidget` | Dropped. See L3. | T-08a-038 |
| L24 | Fast scroller | `VerticalFastScroller`, Compose scrollbar | Dropped. GTK overlay scrolling for every list (T-39a-064, T-49b-036). | T-39a-064, T-49b-036, T-34a-066 |

---

## 4. Dropped and replaced items by domain

Each table row names the Aniyomi item, its location, the decision, the Linux or Rust replacement (if any), the user loss, and the task IDs. Locations are repo-relative to the Aniyomi clone.

### 4.1 Extensions and extension installation

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Android PackageInstaller session installer | app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/PackageInstallerInstallerAnime.kt (117 LOC) | Dropped | Repo download, sha256 check, rename into `<data>/extensions/<pkg>/` (area:20 section 4.1 item 6) | Android APK install | T-19a-038, T-20a-093 |
| Shizuku and `pm install-*` installer | app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/ShizukuInstallerAnime.kt (124 LOC), app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/ShizukuInstallerManga.kt | Dropped | None | Shizuku-based install | T-19a-039, T-20a-093, T-47a-085, T-53a-038 |
| Foreground install service and notification -402 | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt (88 LOC), app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt | Dropped | In-page progress (T-18a-041, T-19a-041) | Notification during install | T-18a-038, T-18a-041, T-19a-041, T-53a-036 |
| Install activity and MIUI workaround | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt (79 LOC) | Dropped | None | MIUI installer path | T-19a-040, T-18a-039 |
| Install receiver (package add, replace, remove broadcasts) | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt (147 LOC) | Dropped; replaced by direct events | `tokio` channel from the manager to the UI | None | T-19a-043, T-19a-071 |
| DownloadManager polling and completion broadcast | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt (327 LOC) | Dropped | HTTP download through `reqwest` into `<data>/extensions/.tmp/` (area:20 item 6) | None | T-19a-042 |
| Dynamic class loading and source factories | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt; app/src/main/java/eu/kanade/tachiyomi/util/system/ChildFirstPathClassLoader.kt | Dropped | Built-in registry, TOML scrapers, WASM components | Loading Android extension binaries | T-19a-044, T-52a-035 |
| PackageManager, signatures, base paths, FileProvider, Android feature checks | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt | Dropped | Trust rule in area:20 section 4.1 item 5 (two tier, as in Aniyomi) | None | T-19a-045 |
| `isShared` (system-installed extension location) | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt:139, :156 | Dropped | None | Extensions installed system-wide are not discovered | T-19a-073, T-18a-042 |
| `hasReadme`, `hasChangelog`, `pkgFactory` metadata | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt | Dropped | Repo JSON has no such fields in Kurobayomi | README and changelog links in the extension list | T-19a-072 |
| `ACTION_UNINSTALL_PACKAGE` for system packages | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt | Dropped | Delete `<data>/extensions/<pkg>/` | None | T-19a-074 |
| Notification channel for extension install | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt | Dropped | None | None | T-19a-075 |
| Installer preference (`extension_installer`) | app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt | Dropped | Single install route `kuroext` (T-19a-037) | Choice of installer | T-01a-044, T-18a-039, T-47a-085 |
| Legacy extension paths: `"null"` URL sentinel, ext-lib 15 and 16 constructors, `videoUrlRequest` | area:16 D4, `EpisodeLoader.kt:180-187`, `Video.kt:60-85` | Dropped | None | Old-API extensions | T-16a-015 |
| `ffmpegStreamArgs`, `ffmpegVideoArgs`, local HTTP server (`createHttpServer`) | area:20 section 4.1 item 7; area:16 Q10 | Dropped (recommendation accepted in the overview, section 6, O-9) | `mpvArgs` only | Source-specified ffmpeg tweaks, and sources that rely on the local proxy | T-45a-080 |
| `BackupExtensionPreferences` and the duplicate `backup.full` models | app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionPreferences.kt | Dropped | None | None (dead code) | T-02a-028, T-21a-033 |
| Android `Video` and `VideoUrl` models, `VideoInfo.kt` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt | Dropped for manga; anime keeps its own `Video` model in area:16 | None for manga | None | T-15a-045, T-16a-049 |

Plain statement of the extension loss for the user: no Aniyomi APK extension runs in Kurobayomi. The library survives because source ids are preserved, not because binaries are reused (area:20 section 4.1 item 2). Recommended answer to area:20 question Q1 is yes (section 6, O-11).

### 4.2 Source networking and the data-saver feature

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Data-saver enum and proxies | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:120-122 (`BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT`) | Feature dropped; enum status open (section 6, O-3) | None | Images no longer proxied through third-party services | T-09a-056, T-40a-042, T-17a-039 |
| Data-saver row in the source menu | app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt | Dropped | None | Data-saver toggle per source | T-40a-042, T-40b-082 |
| Data-saver state on the Sources model | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt | Dropped | None | None | T-40b-013 |
| SY data-saver flag `isExcludedFromDataSaver` | domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt  | Dropped; fork-only flag | None | Excluding a source from data saver | T-09a-056 |
| Commented SY source keys | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt | Dropped | None | None (no readers) | T-17a-039 |
| Image proxies (`resmush.it`) in the library update job | area:26 D11, area:23 D-DL-09 | Open (section 6, O-4) | Proposal in area:26: HTTPS first, fall back to HTTP, settings note | Cannot be decided until O-4 is answered | T-23a-017; area:26 D11 |

### 4.3 Backup and restore

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Dead backup models `full/models/*` (2 files) | app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/Backup.kt | Dropped | None | None | T-21a-033 |
| `Broken*` backup variants | area:21 p1-inventory.md rows 19, 20, 24, 31, 37 | Dropped (area:21 D9) | None | None | T-21a-033 |
| `backup_flags` preference | area:21 D9 | Dropped | None | None | T-21a-033; conflict with O-7 |
| APK extension payloads | ExtensionsRestorer.kt:82-99 | Dropped | Skipped counter | Reinstall extensions by hand (L2) | T-22a-029 |
| MIUI warning on restore | app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt | Dropped | None | None | T-22a-039 |
| Dead sync mode in the restore job | app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt:38 | Dropped (the `isSync` read is dead) | None | None | T-22a-045 |
| MAL login state in backup | location per the T-29a-046 sources | Dropped: backup does not carry the MAL login | Re-login in Kurobayomi | MAL login must be entered again | T-29a-046 |
| Gradle and R8 files in `core-metadata/` | core-metadata/build.gradle.kts, core-metadata/consumer-rules.pro, core-metadata/proguard-rules.pro | Dropped | Cargo manifest | None | T-22a-053 |
| Restore of anime store preferences | area:09 Q-OV-5 | Recommended drop unless the user has such data (T-09a-035) | None | Only matters for users with that preference data | T-09a-035 |

### 4.4 Downloads

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| External downloader intents (IDM, ADM, 1DM) | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt | Dropped | aria2 over JSON-RPC, off by default (T-24a-025, P2) | Third-party downloader use | T-09a-022, T-24a-024 |
| External downloader selector | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt | Dropped | None | Choice of downloader | T-47a-051 |
| `AnimeDownloadPart` (HTTP range model) | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownloadPart.kt | Dropped (dead) | None | None | T-24a-003 |
| `ensureSuccessfulAnimeDownload` | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt | Dropped (dead) | None | None | T-24a-047 |
| Background-job, foreground, and channel plumbing | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt; app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt | Dropped | tokio tasks in the download queue (area:25) | Android background-job behaviour | T-24a-048, T-23a-038 |
| MediaScanner indexing | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt | Dropped | None (desktop file managers index on demand) | None | T-24a-063 |
| SAF and content URIs | area:24 section 4 item 9 | Dropped | Plain paths | None for a folder on disk | T-24a-059 |
| Migration of Android download state | area:23 item 15 | Dropped | Download state rebuilt from the file tree | Queue order of an Android install is not carried | T-23a-039 |
| Manga download adapter (FlexibleAdapter moves) | app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadAdapter.kt | Mechanism dropped; rule kept in `src/download_queue.rs` | GTK list with drag rule | None | area:23 table row |
| Downloads tab Android-only items | app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt | Replaced or dropped with recorded decision | Area:25 decision record | Recorded in area:25 | T-25a-034 |
| `.nomedia` marker and Android storage roots | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt | Dropped; "local paths only, no media-scanner marker" | Paths trait (addenda rule 4) | None | T-09a-050, T-13a-056, T-14a-053 |

### 4.5 Library updates, notifications, and the updater

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Foreground service declaration for the library update job | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt | Dropped | tokio task | Android service shown while updating | T-26a-120 |
| Notification channels and deprecated channel cleanup | app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt; App.kt:240 | Dropped; 14 Notifier categories replace them | `gio::Notification` (area:35 section 4.2) | Channel-level settings in Android | T-26a-121, T-35a-012, T-51a-036 |
| Wi-Fi pre-check for API < 28 | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt | Dropped | None | None | T-26a-122 |
| Bitmap decode for notification images | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt | Dropped (Android-only decode) | `gio` icon | Big-picture thumbnail shows as icon only | T-26a-126, area:43 section 4.4 |
| WorkManager migrations | app/src/main/java/mihon/core/migration/migrations/UseWorkManagerMigration.kt | Dropped | None | None | T-26a-125 |
| Notification runtime permission | app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt | Dropped | None | None | T-35a-063 |
| Accent tint and sound override for notifications | app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt; app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt | Dropped | None | Accent colour and custom sound | T-35a-064, T-35a-065 |
| Group-summary dismissal logic | app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt | Dropped | `gio` handles grouping | None | T-35a-026 |
| `ACTION_OPEN_ENTRY` (unused) | app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt | Dropped | None | None | T-35a-025 |
| App update: APK download and install | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt | Dropped | Distro package or Flatpak | In-app APK update | T-06a-073, T-07a-037, T-35a-059 |
| App update: AppImage self-replacement | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt | Dropped | None | In-app AppImage update | T-35a-056 |
| App update: preview channel | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt | Dropped (commit-count tags) | Stable releases only | Preview builds | T-35a-061, T-06a-073 |
| `OsTooOld` result | domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt | Dropped (never produced on Linux) | None | None | T-07a-042, T-09a-046, T-35a-060 |
| Platform architecture asset selection | data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt | Replaced by Linux architecture selection | Proposal: `std::env::consts::ARCH` | None | T-06a-072, T-07a-037 |
| Updater flag `BuildConfig.UPDATER_ENABLED` | area:52 item 3 | Recommended false; in-app updater dropped | None | None | area:52 item 3 |

### 4.6 Tracking and OAuth

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| `TrackLoginActivity` and `aniyomi://myanimelist-auth` deep link | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt; app/src/main/AndroidManifest.xml | Dropped | Loopback redirect with the system browser (addenda `WebViewHost` fallback) | Deep-link login shortcut | T-29a-044 |
| `BaseOAuthLoginActivity` loading screen | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt | Dropped | None | Loading screen during OAuth | T-29a-045, T-27b-033 |
| Tracking guide link | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt | Dropped | None | External guide link | T-27a-047 |
| OkHttp interceptor chain (Jellyfin) | app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt | Dropped | Request builder in `reqwest` (area:33) | None | T-33a-023 |
| Injekt and `injectLazy` wiring in trackers | app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt | Dropped | Explicit services struct | None | T-33a-103 |
| Android resource lookups in trackers | app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt | Dropped | Strings in `src/i18n` | None | T-33a-104 |
| Coroutine dispatcher mapping for trackers | app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt | Replaced by `tokio` tasks | `tokio::spawn` | None | T-33a-105 |
| Score display tied to device locale | app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt | Replaced by a locale-independent score display | Fixed format | None | T-30a-035 |
| Kitsu, Shikimori, Simkl, AniList Android plumbing | area:28, area:30, area:31, area:32 | Replaced per the tables in those areas | `reqwest`, `serde`, `tokio` | None | T-28a-041, T-30a-034, T-31a-049, T-32a-062 |

### 4.7 Storage, files, and platform utilities

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Storage Access Framework and `UniFile` | core/archive/src/main/kotlin/mihon/core/archive/UniFileExtensions.kt | Dropped | Plain `std::path::PathBuf` | Choosing folders outside normal paths | T-13a-035, T-13a-031 |
| `ParcelFileDescriptor` and archive module plumbing | core/archive/ | Dropped | Proposal: `zip` and `tar` crates over files | None | T-13a-035 |
| FileProvider URI and cache share directory | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt; app/src/main/res/xml/provider_paths.xml | Dropped | Plain paths and `Opener` | Sharing via content URIs | T-13a-055, T-35a-030, T-53a-064 |
| SD card and external storage enumeration | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt | Dropped | Paths trait | Choosing an SD card | T-13a-056 |
| FFmpeg SAF string helper | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt | Dropped | None | None | T-13a-054 |
| Android cache-directory and cover-file migration steps | app/src/main/java/mihon/core/migration/migrations/InternalChapterCacheDirMigration.kt | Dropped | None | None | T-11a-020 |
| Storage platform adaptation | domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt | Replaced by local paths | Paths trait | None | T-09a-050 |
| WebView package spoofing and inferred user agent | core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt | Dropped | WebKitGTK default user agent (addenda `WebViewHost`) | None | T-01a-026, T-12a-037 |
| WebView data-directory suffix | app/src/main/java/eu/kanade/tachiyomi/App.kt | Dropped | None | None | T-01a-028 |
| Conscrypt TLS provider insertion | app/src/main/java/eu/kanade/tachiyomi/App.kt; app/build.gradle.kts | Dropped | rustls or system TLS in reqwest | None | T-01a-027 |
| Vendor ROM and browser probes in `DeviceUtil` | core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt | Dropped | None | None | T-12a-038, T-13a-045 |
| Hardware bitmap threshold and EGL texture limit | core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt; core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt | Dropped; key ignored and not migrated | GTK manages textures | None | T-13a-030, T-34a-034 |
| Low-RAM RGB565 option | app/src/main/java/eu/kanade/tachiyomi/App.kt; core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt | Dropped | None | Low-memory decode mode | T-34a-035 |
| `dpToPx` and density helpers | core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt | Dropped | CSS and GTK units | None | T-12a-040, T-13a-046 |
| Display cutout and navigation-bar scrim checks | app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt | Dropped | None | None | T-52a-046 |
| Package and installer checks (`isPackageInstalled`, MIUI, Shizuku, install permission) | app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt | Dropped | None | None | T-52a-043 |
| Runtime permission check, default browser resolver | app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt | Dropped | `open` crate and `gio::AppInfo` | None | T-52a-037, T-52a-040 |
| TV box detection | app/src/main/java/eu/kanade/tachiyomi/util/system/TvUtils.kt | Dropped | None | None | T-52a-055 |
| Foreground promotion helper | app/src/main/java/eu/kanade/tachiyomi/util/system/WorkManagerExtensions.kt | Dropped | None | None | T-52a-057 |
| Intent extras compat getters | app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt | Dropped | `clap` for CLI, `gio::Application` for URLs | None | T-52a-049 |
| System resource booleans | app/src/main/java/eu/kanade/tachiyomi/util/system/InternalResourceHelper.kt | Dropped | None | None | T-52a-050 |
| Closeable arrays | app/src/main/java/eu/kanade/tachiyomi/util/lang/CloseableExtensions.kt | Dropped | RAII (`Drop`) | None | T-52a-030 |
| RxJava bridges (`RxJavaExtensions.kt`, `RxCoroutineBridge.kt`, `RxExtension.kt`) | app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt; core/common/src/main/java/tachiyomi/core/common/util/lang/RxCoroutineBridge.kt; source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt | Dropped (RxJava 1 is not used in Rust) | `async fn` | None. Legacy Rx-only extension APIs are not ported (area:15 D10) | T-01a-038, T-13a-047, T-15a-045, T-15a-047, T-16a-048, T-20a-092, T-52a-006 |
| Serializable models | domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt and related | Replaced by `serde` derives | `serde` | None | T-07a-038, T-08a-042, T-09a-054 |
| Compose and DI annotations (`Immutable`, `Serializable`, Injekt factories) | domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt and related | Dropped | Plain structs | None | T-07a-038 |
| logcat in domain code | domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetManga.kt and related | Replaced by `tracing` | `tracing` | None | T-07a-039 |
| `withNonCancellableContext` | domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetManga.kt and related | Replaced by `tokio` cancellation (Proposal) | `tokio` | None | T-07a-039 |
| `runBlocking` and `Dispatchers` in the source manager | app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt | Replaced by tokio | `tokio` runtime (area:17 item 4) | None | T-17a-044 |
| Injekt and `Context` wiring in the source manager | app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt | Replaced by constructor injection | Services struct (area:01) | None | T-17a-041, T-41b-059, T-26a-124 |

### 4.8 Database and migrations

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| SQLDelight Gradle plugin, generated Kotlin API, manifest | data/build.gradle.kts; data/src/main/AndroidManifest.xml | Dropped | `rusqlite` with `.sql` constants | None | T-03a-036, T-04a-034 |
| Android SQLite drivers and Database Inspector support | app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt | Dropped | `rusqlite` | Database Inspector | T-03a-037, T-04a-034 |
| Schema snapshot directory and migration verification | data/build.gradle.kts; data/src/main/sqldelight/migrations/32.sqm | Dropped | Migration test in `kuro-core` (area:03) | None | T-03a-038 |
| Paging source over SQL queries | data/src/main/java/tachiyomi/data/handlers/manga/QueryPagingMangaSource.kt; data/src/main/java/tachiyomi/data/handlers/anime/QueryPagingAnimeSource.kt | Dropped | Keyset queries and list model | None | T-04a-033, T-05a-027, T-06a-006, T-49a-030 |
| Statements with no caller | data/src/main/sqldelightanime/dataanime/animes.sq; data/src/main/sqldelightanime/dataanime/episodes.sq | Dropped | None | None | T-04a-035 |
| Column `is_syncing` and `resetIsSyncing` | data/src/main/sqldelight/data/mangas.sq; data/src/main/sqldelightanime/dataanime/animes.sq | Dropped | None | None | T-05a-029 |
| Coroutine transaction context and driver factories | data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt | Dropped; `rusqlite` transactions | `rusqlite::Transaction` | None | T-06a-007 |
| Android-only migration steps (41 versioned steps, 11 dropped) | app/src/main/java/mihon/core/migration/migrations/ | 29 kept as pure rules that run on imported backups only; 11 dropped; 3 split | Area:11 | Legacy keys reported in the import report | T-11a-020 to T-11a-024, T-11a-029 |
| Force MAL log-out migration | app/src/main/java/mihon/core/migration/migrations/ForceMALLogOutMigration.kt | Dropped | None | None | T-11a-021 |
| WorkManager cleanup migration | app/src/main/java/mihon/core/migration/migrations/SetupBackgroundTasksMigration.kt | Dropped | None | None | T-11a-022 |
| App-state, private-key, and trust re-key migration; legacy keys ignored | app/src/main/java/mihon/core/migration/migrations/NoAppStateMigration.kt | Dropped | None | Legacy trusted signatures are not re-keyed | T-11a-023 |
| MIUI installer fallback and tracking-queue rewrite | app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt | Dropped | None | None | T-11a-024 |
| Register of settings not carried over | app/src/main/java/mihon/core/migration/migrations/Migrations.kt | Dropped as a register; replaced by the import report | Import report (area:11 item 8) | Visible list in the import report | T-11a-029 |

### 4.9 Reader

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Screen orientation lock and auto-rotate | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt | Dropped; values preserved in the store | None | Per-manga rotation | T-43a-068 |
| Display cutout short-edge mode | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt; app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt | Dropped; hidden | None | Cutout mode | T-43a-069, T-43b-050, T-47a-043 |
| Haptic feedback on slider drag | app/src/main/java/eu/kanade/presentation/reader/components/ChapterNavigator.kt | Dropped | None | Vibration | T-43a-070 |
| Volume keys page turning | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt | Dropped; preference key kept | Arrow keys, Page Up and Page Down, wheel | Volume-key paging | T-43a-071 |
| Activity transitions | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt | Replaced by a stack slide | GTK stack transition | Activity animation | T-43a-072 |
| Android platform plumbing in the reader | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt | Dropped | None | None | T-43a-073 |
| Hardware bitmap, `BitmapRegionDecoder`, SDK gating | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt | Dropped | `image` crate tiles | None | T-43b-051 |
| Long-strip "always decode with SSIV" option | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt | Dropped (one decode path) | Rust decode path only | Option not offered | T-44a-061 |
| `DirectionalViewPager` library | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt; gradle/libs.versions.toml | Replaced by an own `PagerView` widget | `PagerView` (area:44 D1) | None | T-44a-072 |
| Haptics on long press in the pager | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt | Dropped | None | Vibration | T-44a-017 |
| Android view plumbing in webtoon frames | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt | Dropped | Own widget | None | T-44a-073 |
| `ReaderButton`, `WebtoonBaseHolder`, `WebtoonSubsamplingImageView`, `WebtoonFrame` touch translation | area:44 | Dropped | Own widgets | None | area:44 table |
| Replacement record for Android view plumbing | area:43 | Record | Area:43 | None | T-43b-052 |
| Samsung S Pen remote actions | app/src/main/res/xml/s_pen_actions.xml; app/src/main/AndroidManifest.xml | Dropped | Keyboard | Stylus actions | T-35a-027, T-53a-065 |
| Brightness control above 0 | area:43 Q-5 | Degraded: partial | Hardware brightness through the platform layer | Brightness above 0 | area:43 Q-5 |
| ICC display profile | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt | Dropped (area:47); optional P2 (area:34) | None in P0 or P1 | Colour-managed display profile | T-47a-083; conflict O-5 |

### 4.10 Player

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Picture-in-picture policy and PiP actions | app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt; app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt | Dropped | None | PiP window | T-45a-047, T-46a-047; PipActions icons per area:49b section 4 item 11 |
| Android PiP capability flag | app/src/main/java/eu/kanade/domain/base/BasePreferences.kt | Dropped | None | None | T-01a-046 |
| Orientation policy | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt | Replaced by mpv `video-rotate` | mpv property | Android orientation | T-45a-035 |
| Audio focus policy | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt | Dropped (MPRIS pause-others is optional) | MPRIS through `souvlaki` | Automatic pause of other audio | T-45a-042 |
| Brightness memory and control | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt | Dropped on desktop | None | Brightness memory | T-45a-045 |
| Uncaught exception handler in the player | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt | Dropped | Crash handling in area:01 | None | T-45a-054 |
| Web Video Caster intent | app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt | Dropped | None | Hand-off to Web Video Caster | T-45a-060 |
| Foreground HTTP server service | app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt | Dropped | None | Local proxy for sources | T-45a-080 |
| Orientation cycling | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt | Dropped; fullscreen replaces it | Fullscreen toggle | Orientation cycling | T-45b-058 |
| Android IME keyboard handling | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt | Dropped | Keyboard shortcuts | None | T-45b-065 |
| Haptic feedback in the gesture handler | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt | Dropped | None | Vibration | T-46a-045 |
| Screen rotation button | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt | Dropped | None | Rotation button | T-46a-048 |
| Ripple configuration | app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt | Dropped | None | None | T-46a-057 |
| Sleep-timer clock dial | area:46 | Dropped for v1; two `gtk::SpinButton` (hours 0 to 99, minutes 0 to 59) | SpinButtons | Dial control | area:46 table |
| Sheet drag-to-dismiss | area:46 | Dropped; dismiss by scrim, Escape, close button, back | None | Swipe dismiss | area:46 table |
| Android-only player behaviour, documented | app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt | Dropped; documented | Area:46 | Recorded in area:46 | T-46b-040 |
| Android-only player behaviour, documented (list) | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt; app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt | Dropped; documented | Area:45 | The losses listed in T-45b-083 | T-45b-083 |
| External player allowlist | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt | Replaced by MIME lookup | `gio` MIME lookup | Package allowlist | T-48a-053 |
| Orientation setting (player) | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt | Dropped | None | Setting | T-48a-019 |
| System status bar setting (player) | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt | Dropped | None | Setting | T-48a-020 |
| PiP support and fullscreen SDK gate | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt | Dropped | None | Setting | T-48a-054 |
| Torrent engine (TorrServer) start before playback | area:39 Q-5, area:45 section 1 | Deferred (P2); not dropped | TorrServer HTTP client only (area:16 Q2) | Torrent sources do not play in v1 | T-39b-052, T-16a-041 |

### 4.11 Security

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| `FLAG_SECURE` (secure screen) | app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt:8, :10 | Dropped | None | Screenshot blocking | T-01a-033, T-34a-064, T-36a-035, T-47a-066 |
| Secure-screen setting | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt | Dropped; preference kept so existing settings still parse | None | Setting | T-47a-066, T-01a-033 |
| Install-unknown-apps permission state | app/src/main/java/eu/kanade/presentation/util/Permissions.kt | Dropped | None | None | T-34a-065 |
| Biometric unlock | app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt; app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt | Dropped; app lock open (O-2) | Passphrase or PAM | Fingerprint and device-credential unlock | T-36a-032, T-36a-033, T-52a-033 |
| Splash screen art and exit animation | app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt | Dropped | Spinner window up to 500 ms, at most 5000 ms | Splash art | area:36 section 4.5 |
| Assist content | app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt; app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt | Dropped | None | Assistant context | T-36a-064 |
| IME "no personalised learning" in incognito | TachiyomiTextInputEditText.kt (area:36 section 4.5) | Dropped | None | Keyboard learning suppression | area:36 section 4.5 |

### 4.12 Widgets and launcher integration

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Home-screen widget managers (`MangaWidgetManager`, `AnimeWidgetManager`) | presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt | Dropped | Sidebar badge | Desktop widgets | T-01a-024, T-50a-029 |
| Glance widgets and modifier helpers | presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt | Dropped | None | None | T-50a-030 |
| Home and lock-screen receivers (four home, lock-screen keyguard providers) | presentation-widget/src/main/AndroidManifest.xml; presentation-widget/src/main/res/xml/updates_grid_lockscreen_widget_info.xml | Dropped | None | Lock-screen widget | T-50a-026, T-50a-027, T-53a-068 |
| Samsung cover-screen metadata and classes | presentation-widget/src/main/res/xml/updates_grid_samsung_cover_widget_info.xml | Dropped | None | Cover-screen widget | T-50a-028 |
| Widget picker preview images | presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml | Dropped | None | None | T-50a-032 |
| Widget colour tokens | presentation-widget/src/main/res/values/colors_appwidget.xml | Dropped | None | None | T-50a-031 |
| Widget module wiring | presentation-widget/build.gradle.kts | Dropped | None | None | T-50a-033, T-54a-043 |
| Updates grid widget (Glance) | presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt | Dropped; updates tab and tray notifications replace it | Updates tab (area:08) | Launcher widget | T-08a-038 |
| Deep-link trampoline activities | app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt | Dropped | `route_request` in `kuro-core::deeplink` | None | T-53a-046 |
| Old bottom navigation menu | app/src/main/res/menu/main_nav.xml | Dropped | Sidebar | None | T-53a-061 |
| Launcher shortcuts | area:53 | Replaced by `.desktop` actions | `.desktop` file | Launcher context menu in Android | area:53 |

### 4.13 Theme and design system

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Monet, dynamic colour | app/src/main/res/values-v31/themes.xml; app/src/main/res/values-night-v31/themes.xml; app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt | Dropped (T-49ba-007, T-52a-063); portal option open (O-1) | Desktop portal accent (T-47a-013, T-49b-005) | Wallpaper palette | T-49ba-007, T-48a-058, T-52a-063 |
| Named app palettes | app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt | Dropped | Tokyo Night theme | Seventeen palettes | T-47a-012 |
| Tablet UI mode | app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt | Dropped | Width breakpoint (T-42a-055) | Tablet layout choice | T-47a-017, T-48a-007 |
| Bottom navigation bar (`NavigationBar`) | presentation-core/src/main/java/tachiyomi/presentation/core/components/material/NavigationBar.kt | Dropped (phone only) | Sidebar | None on desktop | T-49b-018 |
| Scrollbar and fast scroller | presentation-core/src/main/java/tachiyomi/presentation/core/util/Scrollbar.kt | Dropped in favour of GTK overlay scrolling | GTK overlay scrollbars | Fast scroller | T-49b-036, T-39a-064 |
| Pull-to-refresh | area:49 D-8 | Replaced by refresh button, F5, Ctrl+R | Button and accelerators | Touch gesture | T-49b-019, T-38a-050 |
| Clear focus on IME hide | presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt | Dropped | None | None | T-49b-032 |
| ThemePreviews annotation | presentation-core/src/main/java/tachiyomi/presentation/core/util/ThemePreviews.kt | Dropped | None | None | T-49b-037 |
| Paging helper for lists | presentation-core/src/main/java/mihon/presentation/core/util/PagingDataUtil.kt | Dropped | List model | None | T-49a-030 |
| Haptics in presentation-core | presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt | Dropped | None | Vibration | T-49a-056 |
| Window attributes (status bar, navigation bar, edge-to-edge) | app/src/main/res/values/themes.xml; app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt | Dropped | GTK draws the title bar (area:36 section 4.5) | None | T-49ba-008, T-49ba-047, T-36a-019 |
| More-tab three-dot morph | app/src/main/res/drawable/anim_more_enter.xml | Dropped (decorative) | Static icon | Animation | T-49ba-023 |
| Unused drawables (14 files), style items, attributes, widget corner radii | app/src/main/res/drawable/; app/src/main/res/values/styles.xml; app/src/main/res/values/attrs.xml; app/src/main/res/values/dimens.xml | Dropped | None | None | T-49ba-039, T-49ba-040, T-49ba-041, T-49ba-042 |
| Unused `keep.xml` `md_*` rule | app/src/main/res/raw/keep.xml | Dropped | None | None | T-53a-062 |
| Unused player animations | app/src/main/res/anim/player_enter_bottom.xml and related | Dropped | None | None | T-53a-052 |
| Unused preference switch layout | app/src/main/res/layout/pref_widget_switch_material.xml | Dropped | None | None | T-53a-056 |
| Chapter download menu | app/src/main/res/menu/chapter_download.xml | Dropped | Entry download menu (area:38) | None | T-53a-059 |
| List item animations | app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt and related | Dropped | None | Animations | T-47b-056, T-41b-057, T-39a-059 |
| Compose grid item keys | app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt | Dropped | None | None | T-39a-059 |
| Compose preview fixtures | app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHomePreviewProvider.kt and related | Dropped; fixtures kept as test data | Test fixtures | None | T-27a-061, T-42a-057 |
| `CARD_RADIUS_DP` constant | area:50 D-10 | Recommended drop | None | None | area:50 D-10 |

### 4.14 Navigation and UI platform

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Voyager navigator | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt and related | Replaced by GTK page stack | GTK stack | Voyager-only behaviour | T-39b-068, T-39b-069, T-40b-083, T-41b-058, T-42a-056 |
| Injekt in UI models | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt | Replaced by an explicit services struct | Services struct | None | T-41b-059 |
| Haptics on long press in browse | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt | Dropped | None | Vibration | T-40b-081, T-41b-055 |
| Assist content URL in browse and entry screens | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt | Dropped | None | Assistant context | T-40b-080, T-41b-056 |
| Package-install permission banner | app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt | Dropped | None | Banner | T-40a-026, T-41a-025 |
| Extension app-info button | app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt | Dropped (40 Q8); replaced by "Open folder" (41 T-41a-024). Conflict O-6 | Open-folder action pending O-6 | App-info button | T-40a-031, T-41a-024 |
| Data-saver row in the sources screen | app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt | Dropped | None | Data-saver row | T-40a-042 |
| Fragment-based preference host and reflection | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt; app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt | Dropped; declarative GTK preference page | Native page from the source's declarative list (area:40 A13) | None | T-40a-043, T-41a-046 |
| Android tablet detection | app/src/main/java/eu/kanade/presentation/util/WindowSize.kt | Replaced by width breakpoint | GTK width breakpoint | None | T-42a-055 |
| MainActivity ready hand-off | app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt | Dropped | None | None | T-42a-059 |
| Haptic feedback on long press in updates | app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt | Dropped | None | Vibration | T-42a-058 |
| Animated tab icons | app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt and related | Dropped | Static icons | Animation | T-42a-056 |
| Android-only behaviours in updates | app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt | Dropped or replaced with recorded decision | Area:42 | Recorded | T-42b-042 |
| Android-only mechanisms in settings widgets | app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt | Dropped; decision recorded | None | None | T-47ba-075 |
| Android storage route (unused) | app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt | Dropped | None | None | T-47ba-017 |
| Share from the web view | app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt | Replaced by clipboard copy | `gtk::Clipboard` | Share targets | area:36 section 4.5 |
| Marquee subtitle in app bar | AppBar.kt (area:36 section 4.5) | Replaced by ellipsis plus tooltip | GTK label | Scrolling long URLs | area:36 section 4.5 |

### 4.15 Settings screens and preference rows

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Cutout row (general settings) | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt | Dropped | None | Cutout row | T-47a-043 |
| External downloader selector (IDM, ADM) | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt | Dropped | None | Selector | T-47a-051 |
| Battery optimisation and "Don't kill my app" rows | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt | Dropped | None | Rows | T-47a-073 |
| Custom display profile (ICC) | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt | Dropped (area:47); optional P2 (area:34). Conflict O-5 | None | Profile setting | T-47a-083 |
| Extension installer choice | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt | Dropped | None | Choice | T-47a-085 |
| MIUI optimisation banner | app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt | Dropped | None | Banner | T-47b-035 |
| ART profile compilation row | app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt | Dropped | None | Row | T-47b-042 |
| OneUI and MIUI rows | app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt | Dropped | None | Rows | T-47b-051 |
| Android persistable URI permission | app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt | Dropped | None | None | T-47b-054 |
| Orientation and status bar (player settings) | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt | Dropped | None | Rows | T-48a-019, T-48a-020 |
| Secure screen (settings) | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt | Dropped | None | Row | T-47a-066 |
| Tablet UI mode row | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt | Dropped | None | Row | T-47a-017 |
| Dynamic (Monet) row | app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt | Replaced by a portal accent option (O-1) | Desktop portal | Monet | T-48a-058 |
| Data-saver rows (sources) | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt | Dropped | None | Rows | T-40b-082 |
| Commented SY source keys | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt | Dropped | None | None | T-17a-039 |

### 4.16 Localisation

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| moko-resources Gradle plugin and code generation | i18n/build.gradle.kts; i18n-aniyomi/build.gradle.kts | Dropped | `build.rs` and `xtask` | None | T-51a-022 |
| `Localize.kt` Android localisation API | core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt | Dropped; `tr`, `trf`, `tr_plural` replace it | `src/i18n/mod.rs` | None | T-51a-023 |
| `LocalesConfigTask` | buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt | Dropped | Locale table in `src/i18n` | None | T-51a-007 |
| Notification channel strings | i18n/src/commonMain/moko-resources/base/strings.xml (`channel_common`, `channel_progress`, `channel_complete`) | Dropped; text kept as category labels | Notifier categories | None | T-51a-036 |
| S-Pen strings | i18n/src/commonMain/moko-resources/base/strings.xml (`spen_previous_page`, `spen_next_page`) | Dropped | None | None | T-51a-037 |
| 133 names with no literal Kotlin reference | i18n/src/commonMain/moko-resources/base/strings.xml (86 names); i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml (47 names) | Decision recorded (T-51a-019) | Area:51 decision | None until decided | T-51a-019 |

### 4.17 Build, release, and distribution

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Gradle build and buildSrc precompiled plugins | buildSrc/src/main/kotlin/mihon.android.application.compose.gradle.kts and related | Dropped | Cargo workspace (T-01a-001) | None | T-54a-036, T-01a-001 |
| R8 and ProGuard rules | app/proguard-rules.pro; app/proguard-android-optimize.txt; core-metadata/proguard-rules.pro; data/proguard-rules.pro | Dropped | `strip`, LTO, linker dead-code elimination | None | T-01a-041, T-54a-026 |
| Baseline profile and generator | app/src/main/baseline-prof.txt; macrobenchmark/README.md | Dropped | None | None | T-54a-024 |
| Macrobenchmark module | macrobenchmark/build.gradle.kts and related | Dropped | Optional `criterion` startup bench (catalog) | None | T-54a-025 |
| Integer `versionCode` | app/build.gradle.kts | Dropped | `version` in Cargo.toml | None | T-54a-022 |
| Dead variant filter | app/build.gradle.kts | Dropped | None | None | T-54a-030 |
| Upstream funding links | .github/FUNDING.yml | Dropped | None | None | T-54a-014 |
| Android-only CI steps | .github/workflows/build_pull_request.yml; .github/workflows/build_push.yml | Dropped with recorded decision | Cargo CI and Flatpak build (area:54) | None | T-54a-060 |
| Empty module manifests | core-metadata/src/main/AndroidManifest.xml; core/archive/src/main/AndroidManifest.xml; core/common/src/main/AndroidManifest.xml | Dropped | None | None | T-53a-067 |
| APK signing, ABI splits, R8 mapping, profile installer, desugaring, LeakCanary | area:54 A-9 | Dropped | None | None | area:54 A-9 |
| Flatpak runtime notes | area:54 Q5 and Q13 | Open: confirm the GTK version on the CI runner (4.14 expected on `ubuntu-24.04`; the gtk4 `v4_16` feature must be checked at build time) | Flatpak packaging | None | area:54 Q13 |

### 4.18 Platform manifest, permissions, and intents

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| `WAKE_LOCK` and battery-optimisation prompts | app/src/main/AndroidManifest.xml (lines 19 to 20); app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt | Dropped | None | Prompts | T-53a-028 |
| Foreground-service permissions | app/src/main/AndroidManifest.xml (lines 18 and 37) | Dropped | `gio::Application::hold` | None | T-53a-029 |
| Notification permission prompt in onboarding | app/src/main/AndroidManifest.xml (line 31); PermissionStep.kt | Dropped; replaced by a status row (area:36) | Status row | Prompt | T-53a-030 |
| Application attributes (backup, heap, legacy storage, icons) | app/src/main/AndroidManifest.xml (application element) | Dropped | None | None | T-53a-032 |
| Extension install permissions (`REQUEST_INSTALL_PACKAGES`, `REQUEST_DELETE_PACKAGES`, `UPDATE_PACKAGES_WITHOUT_USER_ACTION`) | app/src/main/AndroidManifest.xml | Dropped | None | None | T-53a-038 |
| Android permission and intent rows in onboarding | app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt | Dropped | None | Rows | T-36a-068 |
| Install-unknown-apps permission and Android permission checks | app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt | Dropped | None | None | T-52a-043 |
| TV box folder creation | StorageStep.kt (area:36 section 4.5) | Dropped | None | TV box support | area:36 section 4.5 |
| Remote action filters for S Pen | app/src/main/AndroidManifest.xml | Dropped | None | Stylus actions | T-53a-065 |
| Unused animation resources | app/src/main/res/anim/ | Dropped | None | None | T-53a-052 |

### 4.19 Dead code and Android-only remains

| Aniyomi item | Location | Decision | Replacement | User loss | Tasks |
|---|---|---|---|---|---|
| Rx shim classes and dead listener classes (`Observer`, `Producer`, `OnItemSelectedListener`) | catalogs/android-api-mapping-01.md to catalogs/android-api-mapping-08.md | Dropped | None | None | catalogs |
| `getPageList` and `getVideoList` unused hooks | area:14 item 6 | Dropped | None | None (both throw `Unused`) | area:14 list item 6 |
| Android `Video` stub | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt | Dropped | None | None | T-16a-049 |
| Android actuals and expect/actual manifests in source-api | source-api/src/androidMain/; source-api/build.gradle.kts | Dropped | None | None | T-15a-046, T-16a-050, T-20a-092 |
| Deprecated Rx fetch API and `chapterPageParse` | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt | Dropped | `async fn` fetch API | Legacy source extensions | T-15a-047, T-16a-053 |
| Source preference Android typealias | source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt | Dropped | None | None | T-16a-050 |
| Empty module manifests | core/common, core/archive, presentation-core, presentation-widget, core-metadata, data, domain | Dropped | None | None | T-53a-067, T-09a-055, T-49a-054 |
| Gradle build for `domain` | domain/build.gradle.kts | Dropped | Cargo `kuro-core` | None | T-09a-055 |
| Dead code in domain: `OsTooOld` and `NetworkToLocalAnime` | domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt | Dropped | None | None | T-07a-042 |
| Dead code in the category model | domain/src/main/java/tachiyomi/domain/category/model/Category.kt | Dropped (Android-only plumbing) | None | None | T-08a-042 |
| Android-only preferences | app/src/main/java/eu/kanade/domain/base/BasePreferences.kt; app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt | Kept for round trip, hidden in UI | Parsed, not shown | None | T-02a-032 |
| `SharedPreferencesDataStore` | app/src/main/java/eu/kanade/tachiyomi/data/preference/SharedPreferencesDataStore.kt | Dropped | `PreferenceStore` (area:02) | None | T-02a-031 |

---

## 5. Replacement map: Android and third-party API to Linux or Rust

The table gives the replacement for each platform capability that Kurobayomi keeps. The trait names come from `plans/addenda/requirements-cross-platform.md`, which requires every OS-specific call to live in the `kuro-platform` crate. Crate names are proposals taken from the area overviews.

| Capability | Aniyomi API | Kurobayomi replacement | Crate or interface | Source of the decision |
|---|---|---|---|---|
| Paths | `Context.filesDir`, `getExternalFilesDir`, SAF | XDG base directories | `Paths` trait, `directories` crate | addenda trait table; area:13 |
| Secrets | Android keystore or preference store for tokens | Secret Service | `SecretStore` trait, `keyring` crate | addenda trait table |
| Notifications | `NotificationCompat`, channels | `gio::Notification` | `Notifier` trait; area:35 section 4.2 (14 categories) | addenda trait table; area:35 |
| Media keys and now-playing | `MediaSession` | MPRIS | `MediaControls` trait, `souvlaki` crate | addenda trait table |
| Web view | `WebView`, Cloudflare solver | WebKitGTK | `WebViewHost` trait, `webkit6` crate; fallback is the system browser with a loopback redirect and cookie import | addenda trait table; area:52 |
| Open URL, reveal file, open player | `Intent.ACTION_VIEW`, `ShareCompat` | `xdg-open` through `gio::AppInfo` | `Opener` trait, `open` crate | addenda trait table; area:52 item 3 |
| Screen awake while playing | `FLAG_KEEP_SCREEN_ON` | `org.freedesktop.ScreenSaver` or the portal Inhibit call | `IdleInhibitor` trait | addenda trait table |
| Single instance and CLI or URL forwarding | `singleTask` activity | `gio::Application` unique id | `SingleInstance` trait | addenda trait table |
| File associations | Intent filters | `.desktop` file and shared-mime-info | `FileAssociations` trait | addenda trait table |
| Autostart | Boot receiver | XDG autostart entry | `Autostart` trait | addenda trait table |
| External player | `Intent` to mpv or VLC | Spawn process or `gio` activation | `ExternalPlayer`; area:45 D-14 presets `default`, `mpv`, `vlc`, `celluloid`, `custom` with placeholders | addenda trait table; area:45 |
| Scheduling | `WorkManager` | In-process `tokio` interval; optional systemd user timer | `SchedulerBackend` trait | addenda trait table; area:26 |
| App lock | `BiometricPrompt` | Passphrase or PAM or polkit | `Biometrics/AppLock` trait; open O-2 | addenda trait table; area:36 Q2; area:52 item 1 |
| Image decode | `BitmapFactory`, Coil | `image` crate and `gdk::Texture` | area:34 | area:34; catalog |
| Video | Media3, `ExoPlayer` | libmpv through the render API into a `GtkGLArea` | area:45; addenda "embedding mpv" | addenda; area:45 |
| HTTP | `OkHttp` with interceptors | `reqwest` with middleware | area:12 | area:12 |
| Preferences | `SharedPreferences`, DataStore | `PreferenceStore` over `rusqlite` | area:02 | area:02 |
| Database | SQLDelight over Android SQLite | `rusqlite` with `.sql` constants | area:03, area:04 | area:03; area:04 |
| Async | RxJava 1 and Kotlin coroutines | `tokio` tasks and `async_channel` to the UI | area:01; area:52 |
| Dependency injection | Injekt | Explicit services struct in `kuro-core` | area:01 | area:01; area:41 T-41b-059 |
| Logging | logcat | `tracing` | area:01 | area:07 T-07a-039 |
| Serialisation | `java.io.Serializable` | `serde` derives | area:09 T-09a-054 | area:09 |
| Icons | Android vector drawables | Bundled symbolic SVG, Tokyo Night CSS | addenda rule 6 | addenda rule 6 |
| Theme | `DynamicColors`, Monet | Desktop portal accent or fixed Tokyo Night | open O-1 | area:49; area:47 D-8 |
| List paging | `Paging3` | Keyset SQL and list models | area:05, area:06 | area:05; area:06 |
| Haptics | `HapticFeedbackConstants` | None | Dropped (section 3, L12) | Dropped |
| Share sheet | `Intent.ACTION_SEND` | `gtk::Clipboard` and `gtk::UriLauncher`; open-in-file-manager | open O-8 | area:39; area:43 Q-4; area:36 |
| Pull to refresh | `SwipeRefreshLayout` | Refresh button, F5, Ctrl+R | area:49 D-8 | area:49 |
| Toasts | `Toast` | In-app overlay, queue cap 5 | area:12 Q8 | area:12 |
| Navigation | Voyager | GTK page stack | area:36 | area:36; area:41 T-41b-058 |
| Global search from the OS | `searchable.xml` | Not replaced | Dropped | T-35a-029 |
| Crash reports | ACRA, Firebase | `CrashLogUtil` consumer, local file | area:52 item 1 | area:52 |

---

## 6. Open decisions and conflicts

Each item lists the plan answers that disagree and the question that needs the user's answer. Nothing in this list is decided. Each item has a recommendation from the owning area.

| ID | Topic | Answer A | Answer B | Question for the user | Recommendation |
|---|---|---|---|---|---|
| O-1 | Dynamic colour (Monet) | Drop: T-49ba-007, T-52a-063, T-47a-012 | Keep as portal option: T-49b-005, T-47a-013 (area:49 section 4.3 asks the user, Q8) | Do you want the desktop accent colour from the portal, or only the fixed Tokyo Night theme? | Portal accent as an option, default Tokyo Night. |
| O-2 | App lock | PAM: T-52a-033 (area:52 item 1, option a) | Passphrase, opt-in, default off, P1: area:36 Q2 | Do you want an app lock, and if so, PAM or a passphrase? A forgotten passphrase has no recovery path (area:36 Q2). | Passphrase, opt-in, default off (area:36 Q2). |
| O-3 | Data-saver enum | Keep the four enum values and the key so stored values parse: area:17 Q2 | Drop the enum, the feature, and the excluded-source list: area:40 Q2 | Confirm dropping the feature. The enum question affects only how a stored value is read. | Drop the feature. Keep the key so existing values parse. Proposal: map every stored non-`NONE` value to `NONE` with one message. |
| O-4 | `resmush.it` image proxy | Drop: area:23 D-DL-09 (T-23a-017) | HTTPS first, fall back to HTTP, settings note: area:26 D11 | Keep an HTTP fallback for a third-party image service, or drop it? | Area:26 proposal is weaker than dropping. Recommend drop, consistent with O-3. |
| O-5 | ICC display profile | Optional P2 feature `icc` (area:34 Q-2, T-34a-031) | Drop (area:47 D-13, T-47a-083) | Keep an optional ICC profile feature at P2, or drop it? | Drop. A solo user is unlikely to set a custom profile (area:34 Q-2). |
| O-6 | Extension details "app info" action | Drop: area:40 Q8 (b) | Replace with "Open folder": area:41 T-41a-024 | Want an "Open folder" action on extension details? | Drop for P0 and P1. Keep as a P2 option. |
| O-7 | `backup_flags` preference | Drop: area:21 D9 | Keep the key for format parity, no UI: area:09 Q-OV-6 | Keep the key unread, or drop it? | Keep the key for round-trip, hide it in the UI (area:09 Q-OV-6). |
| O-8 | Share replacement | Clipboard copy: area:36 section 4.5. Open or show in file manager: area:43 Q-4. Clipboard and `gtk::UriLauncher`: area:39 section 4.1 item 9 | Which replacement for URLs, text, and images? Three areas disagree in wording | Proposal: clipboard for text, `gtk::UriLauncher` for links, "show in file manager" for files. |
| O-9 | Source-side ffmpeg args and local HTTP server | Drop: area:20 section 4.1 item 7, area:16 Q10 | Keep `mpvArgs`, drop `ffmpegStreamArgs`, `ffmpegVideoArgs` (both recommended drop) | Drop source-specified ffmpeg tweaks and the local proxy? | Drop (area:20 Q10). |
| O-10 | Torrent backend | TorrServer HTTP client only: area:16 Q2 | Defer: area:20 Q9 (librqbit or rqbit HTTP API) | Do you watch torrent-sourced anime? | Defer. Skip torrent for v1 unless a torrent-only source is needed (area:39 Q-5). |
| O-11 | Existing Aniyomi APK extensions stop working | Accept: area:20 Q1 recommends yes | None proposed | Accept that Aniyomi APK extensions stop working, and only library data carries over? | Yes. Area:20 Q1. |
| O-12 | Anime store preference import | Keep: area:09 T-09a-035 | Drop: area:09 Q-OV-5 (b) | Do you have anime store preference data to import? | Drop unless you have such data. |
| O-13 | Notification permission row | Status row: area:36 | Dropped prompt: area:53 T-53a-030 | Confirm the status row replaces the permission prompt. | Status row. The two plans agree in substance. |
| O-14 | Drop tasks that P1 and P2 tasks depend on | Edge kept: section 1.3 | Edge removed | Treat drop tasks as decision records that complete on writing the decision? | Yes. Section 1.3 proposal. |

Two more items need no user answer but need a plan fix:

- Share replacements in areas 36, 39, and 43 disagree in wording (O-8). The fix is one line per area that names the rule in section 5.
- The area:24 dependency cycle `T-24a-016` and `T-24a-049` is a task-graph defect, not a drop. Recorded in area:24 section 6 item 12. Not changed here.

---

## 7. Deferred items that are not drops

These are capabilities removed from the P0 and P1 path. They are not in the P3 list, and they keep their P2 or later priority.

| Capability | Tasks | Note |
|---|---|---|
| aria2-based external downloader | T-24a-025 (P2) | Off by default. Shown only when enabled (area:24 section 4 item 4). |
| Protobuf extension store format | T-19a-014 (P2) | JSON `index.min.json` first. Gzip is cheap to add. Reject `repo.json`-only repos with a clear message (area:20 Q11). |
| Torrent server start before playback | T-39b-052 (P2) | See O-10. |
| TorrServer client | T-16a-041 (P2) | See O-10. |
| Skip-intro, airing, related sync | area:39 section 6.3 | P2. |
| Animated GIF cover save | area:39 OD-8 | Save the original bytes so the animation survives. Display stays static. |
| Optional ICC profile | T-34a-031 (P2) | See O-5. |
| WASM sources | T-20a-001 (P0 in the overview, proposed P1) | Area:20 Q3. The built-in loader is P0. |
| Windows port | T-req-platform-011 (P3) | Deferred to a later milestone. Not a drop. The addenda require every OS call to go through `kuro-platform` so the port is a platform-layer job. |

---

## 8. Appendix A: every P3 task

Generated from `plans/TASKS.csv` (priority `P3`, 309 rows). Titles are verbatim. The disposition column applies the title-keyword rule in section 1.2. Fifteen rows whose titles have no drop or replace keyword were classified by hand (for example T-21a-035 is Record and T-45a-042 is Drop). Depends and first source are copied from the CSV.

| Task | Pri | Effort | Depends | Disposition | Title (verbatim) | First source |
|---|---|---|---|---|---|---|
| T-01a-024 | P3 | S | area:50-android-widgets | Drop | Drop home-screen widget managers (decision and sidebar badge source) | presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt |
| T-01a-026 | P3 | S | none | Drop | Drop WebView package-name spoofing (getPackageName override) | app/src/main/java/eu/kanade/tachiyomi/App.kt |
| T-01a-027 | P3 | S | none | Drop | Drop Conscrypt TLS provider insertion | app/src/main/java/eu/kanade/tachiyomi/App.kt |
| T-01a-028 | P3 | S | none | Drop | Drop WebView data-directory suffix | app/src/main/java/eu/kanade/tachiyomi/App.kt |
| T-01a-033 | P3 | S | T-01a-011 | Drop | Drop secure-screen mode (FLAG_SECURE) | core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt |
| T-01a-038 | P3 | S | none | Drop | Drop the RxJava bridge | app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt |
| T-01a-041 | P3 | S | area:20-extension-strategy-redesign | Drop | Drop R8 rules; hand the extension host-API checklist to area:20 | app/proguard-rules.pro |
| T-01a-044 | P3 | S | area:20-extension-strategy-redesign | Drop | Drop the extension-installer preference | app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt |
| T-01a-046 | P3 | S | area:46-player-controls-ui | Drop | Drop the Picture-in-Picture capability flag | app/src/main/java/eu/kanade/domain/base/BasePreferences.kt |
| T-02a-028 | P3 | S | T-02a-025 | Drop | Drop BackupExtensionPreferences and the duplicate backup.full models | app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionPreferences.kt |
| T-02a-031 | P3 | S | T-02a-027 | Drop | Drop SharedPreferencesDataStore (AndroidX preference bridge) | app/src/main/java/eu/kanade/tachiyomi/data/preference/SharedPreferencesDataStore.kt |
| T-02a-032 | P3 | S | T-02a-001 | Keep, hidden in the UI | Android-only preferences: keep for round trip, hide from the UI | app/src/main/java/eu/kanade/domain/base/BasePreferences.kt |
| T-03a-036 | P3 | S | T-03a-003 | Drop | Drop: SQLDelight Gradle plugin, generated Kotlin API, manifest | data/build.gradle.kts |
| T-03a-037 | P3 | S | T-03a-002 | Drop | Drop: Android SQLite drivers and Database Inspector support | app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt |
| T-03a-038 | P3 | S | T-03a-021 | Drop | Drop: schema snapshot directory and migration verification | data/build.gradle.kts |
| T-04a-033 | P3 | S | none | Drop | Drop: Paging source over SQL queries | data/src/main/java/tachiyomi/data/handlers/anime/QueryPagingAnimeSource.kt |
| T-04a-034 | P3 | S | none | Drop | Drop: Android SQLite driver wiring and SQLDelight build features | app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt |
| T-04a-035 | P3 | S | none | Drop | Drop: statements with no caller | data/src/main/sqldelightanime/dataanime/animes.sq |
| T-05a-027 | P3 | S | none | Drop | Drop: SQLDelight codegen, Paging sources, coroutine transaction machinery | data/src/main/java/tachiyomi/data/handlers/manga/QueryPagingMangaSource.kt |
| T-05a-029 | P3 | S | none | Drop | Drop: sync-state column `is_syncing` and `resetIsSyncing` | data/src/main/sqldelight/data/mangas.sq |
| T-06a-006 | P3 | S | T-06a-003 | Drop | Drop QueryPagingSource (Paging3 over limit/offset) | data/src/main/java/tachiyomi/data/handlers/manga/QueryPagingMangaSource.kt |
| T-06a-007 | P3 | S | T-06a-002 | Drop | Drop Android handler plumbing (coroutine transaction context, driver factories) | data/src/main/java/tachiyomi/data/handlers/manga/MangaTransactionContext.kt |
| T-06a-072 | P3 | S | T-06a-070 | Replace | Replace Android ABI asset selection with Linux architecture selection | data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt |
| T-06a-073 | P3 | S | T-06a-070 | Drop | Drop in-app APK download/install and the preview update channel | data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt |
| T-07a-037 | P3 | S | T-07a-036 | Drop | Drop Android release installer, notifier and ABI asset selection | data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt |
| T-07a-038 | P3 | S | none | Drop | Drop Compose and DI annotations: Immutable, Serializable, Injekt factories | domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt |
| T-07a-039 | P3 | S | area:01-foundation-app-bootstrap | Replace | Replace logcat and withNonCancellableContext | domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetManga.kt |
| T-07a-042 | P3 | S | T-07a-021, T-07a-036 | Drop | Drop dead code: OsTooOld result and unused NetworkToLocalAnime source manager | domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt |
| T-08a-038 | P3 | S | T-08a-024 | Replace | Drop: home-screen widgets fed by GetXUpdates (replaced by tray/desktop notification) | presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt |
| T-08a-042 | P3 | S | T-08a-001 | Drop | Drop: Android-only domain plumbing (Injekt wiring, Serializable, logcat, Compose state) | app/src/main/java/eu/kanade/domain/DomainModule.kt |
| T-09a-022 | P3 | S | T-09a-014 | Drop | External downloader preferences: dropped on Linux | domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt |
| T-09a-046 | P3 | S | T-09a-042 | Drop | Drop the unused OsTooOld result | domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt |
| T-09a-050 | P3 | S | T-09a-048 | Replace (local paths only) | Storage platform adaptation: local paths only, no media-scanner marker | domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt |
| T-09a-054 | P3 | S | T-09a-010 | Replace | Serializable models: serde replaces java.io.Serializable | domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt |
| T-09a-055 | P3 | S | T-09a-001 | Drop | Gradle build script and Android manifest: dropped | domain/build.gradle.kts |
| T-09a-056 | P3 | S | T-09a-002 | Drop | SY data-saver flag on the manga source: dropped | domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt |
| T-10a-035 | P3 | S | T-10a-001 | Drop | Android-only constructs dropped from this scope | app/src/main/java/eu/kanade/domain/items/chapter/model/Chapter.kt |
| T-11a-020 | P3 | S | none | Drop | Drop: Android cache-directory and cover-file steps | app/src/main/java/mihon/core/migration/migrations/InternalChapterCacheDirMigration.kt |
| T-11a-021 | P3 | S | none | Drop | Drop: MyAnimeList logout steps | app/src/main/java/mihon/core/migration/migrations/ForceMALLogOutMigration.kt |
| T-11a-022 | P3 | S | none | Drop | Drop: WorkManager and background-job cleanup steps | app/src/main/java/mihon/core/migration/migrations/SetupBackgroundTasksMigration.kt |
| T-11a-023 | P3 | S | T-11a-010 | Drop | Drop: app-state, private-key and trust re-key steps; ignored legacy keys | app/src/main/java/mihon/core/migration/migrations/NoAppStateMigration.kt |
| T-11a-024 | P3 | S | none | Drop | Drop: MIUI installer fallback and tracking-queue rewrite | app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt |
| T-11a-029 | P3 | S | T-11a-010 | Drop | Drop register: user-visible list of settings not carried over | app/src/main/java/mihon/core/migration/migrations/Migrations.kt |
| T-12a-037 | P3 | S | none | Drop | Drop WebView package spoofing and inferred user agent | core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt |
| T-12a-038 | P3 | S | T-01a-030 | Drop | Drop vendor ROM probes in DeviceUtil | core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt |
| T-12a-040 | P3 | S | none | Drop | Drop dpToPx | core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt |
| T-13a-030 | P3 | S | none | Drop | Drop the hardware bitmap threshold and EGL texture limit | core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt |
| T-13a-035 | P3 | S | T-13a-031 | Drop | Drop SAF, ParcelFileDescriptor and archive module plumbing | core/archive/src/main/kotlin/mihon/core/archive/UniFileExtensions.kt |
| T-13a-045 | P3 | S | none | Drop | Drop Android vendor and browser checks (DeviceUtil) | core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt |
| T-13a-046 | P3 | S | none | Drop | Drop dp to px conversion | core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt |
| T-13a-047 | P3 | S | none | Drop | Drop the RxJava to coroutine bridge | core/common/src/main/java/tachiyomi/core/common/util/lang/RxCoroutineBridge.kt |
| T-13a-054 | P3 | S | none | Drop | Drop the FFmpeg SAF string helper | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FFmpegUtils.kt |
| T-13a-055 | P3 | S | none | Drop | Drop the FileProvider URI and cache share directory | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt |
| T-13a-056 | P3 | S | none | Drop | Drop external storage enumeration (SD card listing) | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt |
| T-14a-053 | P3 | S | area:13-core-util-storage-archive-torrent | Drop | Drop Android-only plumbing | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt |
| T-14a-054 | P3 | S | area:01-foundation-app-bootstrap | Drop | Drop Gradle, ProGuard, consumer rules and manifests | core-metadata/build.gradle.kts |
| T-15a-045 | P3 | S | T-15a-001 | Drop | Drop Video/VideoUrl and the RxJava awaitSingle bridge | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt |
| T-15a-046 | P3 | S | T-15a-001 | Drop | Drop Android-only source-api files, expect/actual and Gradle build | source-api/build.gradle.kts |
| T-15a-047 | P3 | S | T-15a-013 | Drop | Drop deprecated Rx fetch API and chapterPageParse | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt |
| T-16a-048 | P3 | S | T-16a-003 | Drop | RxJava bridge (awaitSingle) dropped | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt |
| T-16a-049 | P3 | S | T-16a-001 | Drop | Unused util Video stub dropped | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt |
| T-16a-050 | P3 | S | T-16a-035 | Drop | Android actuals (PreferenceScreen typealias, awaitSingle actual) and manifest dropped | source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt |
| T-16a-053 | P3 | S | T-16a-003, T-16a-029, T-16a-030 | Drop | Deprecated Rx and request/parse hooks dropped | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt |
| T-16a-056 | P3 | S | T-16a-042, T-16a-026 | Record (keep one error policy) | Chapter-level and page-level manga sources share the same error policy | source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt |
| T-17a-039 | P3 | S | T-17a-015 | Drop | Drop the commented SY source keys | app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt |
| T-17a-040 | P3 | S | T-17a-025 | Drop | Drop Compose image types | app/src/main/java/eu/kanade/domain/source/anime/model/AnimeSource.kt |
| T-17a-041 | P3 | S | area:01-foundation-app-bootstrap, T-17a-036 | Replace | Replace Injekt and Context wiring with constructor injection | app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt |
| T-17a-044 | P3 | S | T-17a-037 | Drop | Drop runBlocking and Dispatchers in favour of tokio | app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt |
| T-18a-038 | P3 | S | none | Drop | Drop Android installers and install services (P3) | app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/PackageInstallerInstallerManga.kt |
| T-18a-039 | P3 | S | none | Drop | Drop the installer preference and MIUI workaround (P3) | app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt |
| T-18a-041 | P3 | S | none | Replace (in-page progress) | Install progress notification (P3) | app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt |
| T-18a-042 | P3 | S | none | Drop | Drop shared (system-installed) extension discovery (P3) | app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt |
| T-19a-037 | P3 | S | T-19a-032 | Drop | Install route: single kuroext path, preference dropped | app/src/main/java/eu/kanade/domain/base/BasePreferences.kt |
| T-19a-038 | P3 | S | T-19a-032 | Drop | Drop: Android PackageInstaller session installer | app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/PackageInstallerInstallerAnime.kt |
| T-19a-039 | P3 | S | T-19a-032 | Drop | Drop: Shizuku installer | app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/ShizukuInstallerAnime.kt |
| T-19a-040 | P3 | S | T-19a-032 | Drop | Drop: Legacy install activity and MIUI workaround | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt |
| T-19a-041 | P3 | S | T-19a-032 | Drop | Drop: foreground install service and notification -402 | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt |
| T-19a-042 | P3 | S | T-19a-031 | Drop | Drop: DownloadManager, its polling, and its completion broadcast | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt |
| T-19a-043 | P3 | S | T-19a-023 | Replace | Replace Android package broadcasts with direct events | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt |
| T-19a-044 | P3 | S | T-19a-006 | Drop | Drop: dynamic class loading and source factories | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt |
| T-19a-045 | P3 | S | T-19a-004 | Drop | Drop: PackageManager, signatures, base paths, FileProvider, Android feature checks | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt |
| T-19a-071 | P3 | S | T-19a-043 | Verify (no Android code left) | Verify: no Android broadcast or intent code remains for extensions | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt |
| T-19a-072 | P3 | S | T-19a-006 | Drop | Drop: `hasReadme`, `hasChangelog` metadata and `pkgFactory` | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt |
| T-19a-073 | P3 | S | T-19a-005 | Drop | Drop: `isShared` (system-installed) extension location | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt |
| T-19a-074 | P3 | S | T-19a-036 | Drop | Drop: `ACTION_UNINSTALL_PACKAGE` for system packages | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt |
| T-19a-075 | P3 | S | T-19a-053 | Drop | Drop: Android notification plumbing (channel, foreground, text formats) | app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt |
| T-20a-092 | P3 | S | none | Drop | Drop Rx and Android-only actuals | source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt |
| T-20a-093 | P3 | S | none | Drop | Drop Android APK installer paths | app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/ShizukuInstallerManga.kt |
| T-21a-033 | P3 | S | none | Drop | Decision record: drop dead backup code and dead preference | app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/Backup.kt |
| T-21a-035 | P3 | S | T-21a-002, T-21a-034 | Record (debug screen text) | Backup schema text for the debug screen | app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt |
| T-22a-029 | P3 | S | T-22a-007 | Drop | APK extension payloads: skipped counter (drop) | app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/ExtensionsRestorer.kt |
| T-22a-039 | P3 | S | none | Drop | MIUI warning (drop) | app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt |
| T-22a-045 | P3 | S | T-22a-005 | Drop (dead sync mode) | Remove the dead sync mode | app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt |
| T-22a-053 | P3 | S | none | Drop | core-metadata build plumbing (drop) | core-metadata/build.gradle.kts |
| T-23a-038 | P3 | S | T-23a-023, area:53-platform-manifest-intents-workers | Drop | Drop: Android-only machinery | app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt |
| T-23a-039 | P3 | S | T-23a-009, area:11-app-migrations | Drop | Drop: migration of Android download state | app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadStore.kt |
| T-24a-003 | P3 | S | none | Drop | Drop AnimeDownloadPart (dead HTTP range model) | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/model/AnimeDownloadPart.kt |
| T-24a-024 | P3 | M | T-24a-010 | Drop | External downloader handoff: drop Android intents, provide a desktop option | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt |
| T-24a-047 | P3 | S | none | Drop | Drop dead ensureSuccessfulAnimeDownload | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt |
| T-24a-048 | P3 | S | T-24a-030 | Drop | Drop Android background-job, foreground and channel plumbing | app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt |
| T-24a-063 | P3 | S | none | Drop | Drop MediaScanner indexing | core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt |
| T-25a-034 | P3 | S | T-25a-021 | Replace or drop (decision recorded) | Android-only items: replace or drop with recorded decision | app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt |
| T-26a-120 | P3 | S | T-26a-019 | Drop | Drop: Android foreground service declaration | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt |
| T-26a-121 | P3 | S | T-26a-036 | Drop | Drop: Android notification channels and deprecated channel cleanup | app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt |
| T-26a-122 | P3 | S | T-26a-027 | Drop | Drop: Wi-Fi pre-check for API < 28 | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt |
| T-26a-123 | P3 | S | T-26a-096 | Replace | Replace: FileProvider URIs | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt |
| T-26a-124 | P3 | S | T-26a-007 | Replace | Replace: Injekt dependency lookup | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt |
| T-26a-125 | P3 | S | none | Drop | Drop: WorkManager migrations | app/src/main/java/mihon/core/migration/migrations/UseWorkManagerMigration.kt |
| T-26a-126 | P3 | S | T-26a-127 | Drop | Drop: Android-only notification bitmap decode | app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt |
| T-27a-047 | P3 | S | area:36-ui-shell-navigation-security-webview | Drop | Drop the external tracking guide link | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt |
| T-27a-061 | P3 | S | area:49-theme-and-design-system | Drop (Compose previews; fixtures kept) | Compose preview fixtures | app/src/main/java/eu/kanade/presentation/track/anime/AnimeTrackInfoDialogHomePreviewProvider.kt |
| T-27b-033 | P3 | S | T-27b-019 | Drop | Drop the Android activity lifecycle and main-activity relaunch | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt |
| T-28a-041 | P3 | S | T-28a-001 | Replace or drop (decision recorded) | Android-only pieces: replacement or drop record | app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt |
| T-29a-044 | P3 | S | none | Drop | Android-only: TrackLoginActivity and deep-link intent filter (drop) | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt |
| T-29a-045 | P3 | S | none | Drop | Android-only: BaseOAuthLoginActivity loading screen (drop) | app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt |
| T-29a-046 | P3 | S | area:22-backup-restore | Drop | Android-only: backup does not carry MAL login (drop) | core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt |
| T-30a-034 | P3 | S | none | Replace | Android-to-Rust replacement map for scope code | app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt |
| T-30a-035 | P3 | S | T-30a-003 | Replace (locale-independent score) | Score display independent of the device locale | app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt |
| T-31a-049 | P3 | S | T-31a-040 | Replace | Android-only plumbing in scope: drop list and replacements (P3) | app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/Shikimori.kt |
| T-32a-062 | P3 | S | T-32a-001 | Replace | Android-only plumbing: replacement decisions (P3) | app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt |
| T-33a-023 | P3 | S | T-33a-021 | Replace | Drop the OkHttp interceptor chain (P3 replacement) | app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/JellyfinInterceptor.kt |
| T-33a-103 | P3 | S | T-33a-001 | Drop | Drop: Injekt, injectLazy and lazy wiring (P3) | app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt |
| T-33a-104 | P3 | S | T-33a-100 | Drop | Drop: Android resource lookups (P3) | app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt |
| T-33a-105 | P3 | S | T-27a-035, T-27a-036 | Replace (tokio) | Coroutine dispatcher mapping (P3) | app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt |
| T-34a-034 | P3 | S | T-34a-027 | Drop | Hardware-bitmap threshold and device denylist (drop) | core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt |
| T-34a-035 | P3 | S | T-34a-027 | Drop | Low-RAM RGB565 option (drop) | app/src/main/java/eu/kanade/tachiyomi/App.kt |
| T-34a-064 | P3 | S | area:36-ui-shell-navigation-security-webview | Drop | Secure-screen flag (drop) | app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt |
| T-34a-065 | P3 | S | none | Drop | Install-unknown-apps permission state (drop) | app/src/main/java/eu/kanade/presentation/util/Permissions.kt |
| T-34a-066 | P3 | S | none | Drop | Fast-scroll item animation workaround (drop) | app/src/main/java/eu/kanade/presentation/util/FastScrollAnimateItem.kt |
| T-34a-067 | P3 | S | none | Drop | Compose and preference-view plumbing (drop) | app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt |
| T-35a-012 | P3 | S | T-35a-004 | Drop | Drop: channel creation and deprecated-channel cleanup | app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt |
| T-35a-025 | P3 | S | none | Drop | Drop: the unused ACTION_OPEN_ENTRY | app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt |
| T-35a-026 | P3 | S | T-35a-007 | Drop | Drop: group-summary dismissal logic | app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt |
| T-35a-027 | P3 | S | area:44-reader-viewers | Drop | Drop: Samsung S Pen remote actions | app/src/main/res/xml/s_pen_actions.xml |
| T-35a-029 | P3 | S | area:40-ui-browse-manga, area:41-ui-browse-anime | Replace | Drop or replace searchable.xml (global search hint and search provider) | app/src/main/res/xml/searchable.xml |
| T-35a-030 | P3 | S | T-35a-010 | Drop | Drop: FileProvider paths (provider_paths.xml) | app/src/main/res/xml/provider_paths.xml |
| T-35a-056 | P3 | S | T-35a-053 | Replace | Drop: APK install and in-place AppImage self-replacement | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt |
| T-35a-059 | P3 | S | T-35a-050 | Drop | Drop: foreground service and DATA_SYNC type | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt |
| T-35a-060 | P3 | S | T-35a-041 | Drop | Drop: OsTooOld result | domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt |
| T-35a-061 | P3 | S | T-35a-042 | Drop | Drop: preview build channel (commit-count tags) | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt |
| T-35a-063 | P3 | S | T-35a-002 | Drop | Drop: runtime notification permission | app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt |
| T-35a-064 | P3 | S | T-35a-002 | Drop | Drop: accent colour tint on notifications | app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt |
| T-35a-065 | P3 | S | T-35a-082 | Drop | Drop: Android notification sound override for backup-complete | app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt |
| T-36a-019 | P3 | S | T-36a-005 | Drop | System bar and edge-to-edge styling (drop) | app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt |
| T-36a-035 | P3 | S | T-36a-032 | Drop | Secure screen flag (drop) | app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt |
| T-36a-063 | P3 | S | T-36a-046 | Replace | Android-only widgets (drop with replacement) | app/src/main/java/eu/kanade/tachiyomi/widget/MinMaxNumberPicker.kt |
| T-36a-064 | P3 | S | T-36a-001 | Drop | Assist content (drop) | app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt |
| T-36a-068 | P3 | S | T-36a-027 | Drop | Android permission and intent rows (drop) | app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt |
| T-37a-052 | P3 | S | T-37a-013 | Drop | Drop Android-only UI glue | app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt (lines 97 |
| T-37a-053 | P3 | S | T-37a-023 | Replace | Replace the Android URI handler for the getting-started link | app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibraryPager.kt |
| T-38a-047 | P3 | S | none | Drop | Drop: Android haptic feedback | app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt |
| T-38a-048 | P3 | S | none | Drop | Drop: Android assist content URL | app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt |
| T-38a-049 | P3 | S | none | Replace | Replace: Android UI and DI plumbing | app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt |
| T-38a-050 | P3 | S | T-38a-035 | Replace | Replace: pull-to-refresh gesture | app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt |
| T-38a-060 | P3 | S | T-38a-024 | Drop | Drop: stale timer and confirm-by-long-press (Android quirk) | app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt |
| T-38b-042 | P3 | S | T-38b-039 | Replace | Android-only plumbing: dropped or replaced (decision record) | app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt |
| T-39a-052 | P3 | S | T-39a-035 | Drop | Drop the ignorePadding layout trick | app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt |
| T-39a-053 | P3 | S | none | Drop | Drop haptic feedback and preview composables | app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeEpisodeListItem.kt |
| T-39a-059 | P3 | S | T-39a-035 | Drop | Drop Compose grid item keys | app/src/main/java/eu/kanade/presentation/entries/EntryScreenConstants.kt |
| T-39a-064 | P3 | S | T-39a-035 | Drop | Drop the fast scroller | app/src/main/java/eu/kanade/presentation/entries/anime/AnimeScreen.kt |
| T-39b-066 | P3 | S | none | Drop | Drop: Android assist URL (AssistContentScreen) | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt |
| T-39b-067 | P3 | S | none | Drop | Drop: haptic feedback on library add | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt |
| T-39b-068 | P3 | S | none | Drop | Drop: Compose sheet swipe-to-dismiss and Voyager navigator | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt |
| T-39b-069 | P3 | S | none | Drop | Drop: Voyager-specific behaviors that have no GTK counterpart | app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt |
| T-40a-026 | P3 | S | none | Drop | Package-install permission banner (drop) | app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt |
| T-40a-031 | P3 | S | none | Drop | Extension app-info button (drop) | app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt |
| T-40a-042 | P3 | S | none | Drop | Data saver row (drop) | app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt |
| T-40a-043 | P3 | S | none | Drop | Fragment-based preference host (drop of the implementation) | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt |
| T-40b-013 | P3 | S | none | Drop | Drop data-saver state from the Sources model | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt |
| T-40b-080 | P3 | S | none | Drop | Drop Android assist-content URL | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt |
| T-40b-081 | P3 | S | none | Drop | Drop haptic feedback on long press | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt |
| T-40b-082 | P3 | S | T-40b-013 | Drop | Drop data-saver rows in the Sources UI | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt |
| T-40b-083 | P3 | M | area:36-ui-shell-navigation-security-webview | Replace | Replace the Voyager navigator with GTK stack navigation | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt |
| T-41a-024 | P3 | S | none | Replace | Replace "App info" with "Open folder" | app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt |
| T-41a-025 | P3 | S | T-41a-010 | Drop | Drop the "install unknown apps" permission banner | app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt |
| T-41a-046 | P3 | S | none | Drop | Drop Fragment, reflection and Compose-only mechanics | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt |
| T-41b-055 | P3 | S | T-41b-019 | Drop | Drop: haptic feedback on browse long press | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt |
| T-41b-056 | P3 | S | T-41b-010 | Drop | Drop: Assist content URL | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt |
| T-41b-057 | P3 | S | T-41b-035 | Drop | Drop: Compose item animations | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt |
| T-41b-058 | P3 | S | T-41b-005 | Replace | Replace: Voyager navigation with the GTK page stack | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt |
| T-41b-059 | P3 | S | T-41b-047 | Replace | Replace: Injekt with an explicit services struct | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt |
| T-42a-055 | P3 | S | T-42a-053 | Replace | Android tablet detection replaced by width breakpoint (P3 drop) | app/src/main/java/eu/kanade/presentation/util/WindowSize.kt |
| T-42a-056 | P3 | S | T-42a-021, area:36-ui-shell-navigation-security-webview | Drop | Voyager navigation and animated tab icons (P3 drop) | app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt |
| T-42a-057 | P3 | S | T-42a-060 | Drop | Compose preview providers (P3 drop, fixtures kept) | app/src/main/java/eu/kanade/presentation/history/anime/AnimeHistoryScreenModelStateProvider.kt |
| T-42a-058 | P3 | S | T-42a-034 | Drop | Haptic feedback on long press (P3 drop) | app/src/main/java/eu/kanade/presentation/updates/anime/AnimeUpdatesUiItem.kt |
| T-42a-059 | P3 | S | T-42a-021 | Drop | MainActivity ready hand-off (P3 drop) | app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt |
| T-42a-061 | P3 | S | T-42a-013 | Drop | Dead-code drops (P3) | app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt |
| T-42b-042 | P3 | S | T-42b-036 | Replace | Android-only behaviours: drop decisions and replacements | app/src/main/java/eu/kanade/tachiyomi/ui/updates/manga/MangaUpdatesTab.kt |
| T-42b-046 | P3 | S | T-42b-027 | Keep, optional (off by default) | Optional Home key returns to the current month (off by default) | mihon/feature/upcoming/components/calendar/CalendarHeader.kt |
| T-43a-068 | P3 | S | T-43a-004, T-43a-044 | Drop | Screen orientation lock and auto-rotate (drop, values preserved) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt |
| T-43a-069 | P3 | S | T-43a-047 | Drop | Display cutout short-edge mode (drop, hide setting) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt |
| T-43a-070 | P3 | S | T-43a-041 | Drop | Haptic feedback on slider drag (drop) | app/src/main/java/eu/kanade/presentation/reader/components/ChapterNavigator.kt |
| T-43a-071 | P3 | S | T-43a-053 | Drop | Volume keys page turning (drop, preference kept) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt |
| T-43a-072 | P3 | S | T-43a-050 | Replace | Activity transitions (replace with stack slide) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt |
| T-43a-073 | P3 | S | none | Drop | Android platform plumbing (drop) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt |
| T-43b-050 | P3 | S | area:43-reader-core | Drop | Drop: cutout (cutout_short) on Linux | app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt |
| T-43b-051 | P3 | S | T-43b-021 | Drop | Drop: Android hardware bitmap, BitmapRegionDecoder and SDK gating | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt |
| T-43b-052 | P3 | S | area:43-reader-core | Replace | Replacement record for Android view plumbing in scope | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt |
| T-44a-017 | P3 | S | T-44a-011 | Drop | Haptic feedback on long press (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt |
| T-44a-061 | P3 | S | T-44a-050 | Drop | Always decode long strip with SSIV (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt |
| T-44a-072 | P3 | S | T-44a-023 | Replace | Replace the DirectionalViewPager library (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt |
| T-44a-073 | P3 | S | T-44a-023, T-44a-038 | Drop | Drop Android view plumbing (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonFrame.kt |
| T-45a-035 | P3 | S | T-45a-001 | Replace | Android orientation policy replaced by video-rotate | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt |
| T-45a-042 | P3 | S | T-45a-001 | Drop (audio focus) | Audio focus policy | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt |
| T-45a-045 | P3 | S | T-45a-044 | Drop | Brightness memory and control (dropped on desktop) | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt |
| T-45a-047 | P3 | S | T-45a-001 | Drop | Picture-in-picture policy (dropped as Android PiP) | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt |
| T-45a-054 | P3 | S | T-45a-001 | Drop | Uncaught exception handler (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt |
| T-45a-060 | P3 | S | T-45a-055 | Drop | Web Video Caster intent (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt |
| T-45a-080 | P3 | S | T-45a-069, T-45a-072 | Drop | Service lifetime model (dropped Android foreground services) | app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt |
| T-45a-081 | P3 | S | T-45a-001 | Drop | Android-only platform pieces (dropped) | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt |
| T-45b-058 | P3 | S | T-45b-059 | Replace | Orientation cycling dropped; fullscreen replaces it | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt |
| T-45b-065 | P3 | S | T-45b-035 | Drop | Drop Android IME keyboard handling | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt |
| T-45b-083 | P3 | S | T-45b-058, T-45b-065, T-45b-032, T-45b-053, T-45b-054 | Drop | Document Android-only drops and the user-visible losses | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt |
| T-46a-045 | P3 | S | none | Drop | Haptic feedback dropped | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/GestureHandler.kt |
| T-46a-047 | P3 | S | none | Drop | Android PiP params and remote actions dropped | app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt |
| T-46a-048 | P3 | S | none | Drop | Screen rotation button dropped | app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/BottomLeftPlayerControls.kt |
| T-46a-057 | P3 | S | none | Drop | Ripple configuration dropped | app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt |
| T-46b-040 | P3 | S | T-46b-001, T-46b-018 | Drop | Android-only behavior dropped (documented) | app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt |
| T-47a-012 | P3 | S | T-47a-011 | Drop | Drop the 17 named app palettes | app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt |
| T-47a-017 | P3 | S | T-47a-003 | Drop | Drop Tablet UI mode | app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt |
| T-47a-043 | P3 | S | T-47a-034 | Drop | Drop "Show content in cutout area" | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt |
| T-47a-051 | P3 | S | T-47a-003 | Drop | Drop the external-downloader selector (IDM / ADM) | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt |
| T-47a-066 | P3 | S | T-47a-003 | Drop | Drop the secure-screen (screenshot block) setting | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt |
| T-47a-073 | P3 | S | T-47a-003 | Drop | Drop battery-optimisation and "Don't kill my app" rows | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt |
| T-47a-083 | P3 | S | area:44-reader-viewers | Drop | Drop the custom display profile (ICC) | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt |
| T-47a-085 | P3 | S | area:20-extension-strategy-redesign | Drop | Drop the extension installer choice | app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt |
| T-47b-035 | P3 | S | none | Drop | Drop MIUI optimisation banner | app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt |
| T-47b-042 | P3 | S | none | Drop | Drop ART profile compilation row | app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt |
| T-47b-051 | P3 | S | none | Drop | Drop OneUI and MIUI rows | app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt |
| T-47b-054 | P3 | S | none | Drop | Drop Android persistable URI permission | app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt |
| T-47b-056 | P3 | S | none | Drop | Drop list item animations | app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/components/anime/AnimeExtensionStoresContent.kt |
| T-47ba-017 | P3 | S | T-47ba-010 | Drop | Drop the unused storage parameter and route (drop record) | app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt |
| T-47ba-075 | P3 | S | none | Drop | Drop record for Android-only mechanisms (P3) | app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt |
| T-48a-007 | P3 | S | T-48a-002 | Drop | Tablet UI mode: drop, use responsive layout | app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt |
| T-48a-019 | P3 | S | T-48a-010 | Drop | Orientation setting: drop | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt |
| T-48a-020 | P3 | S | T-48a-014 | Drop | System status bar setting: drop | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt |
| T-48a-053 | P3 | S | T-48a-018 | Replace | External player package allowlist: replace with MIME lookup | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt |
| T-48a-054 | P3 | S | T-48a-017, T-48a-014 | Drop | Android device gates (PiP support, fullscreen SDK gate): drop | app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt |
| T-48a-058 | P3 | S | T-48a-004 | Replace | Dynamic (Monet) theme replacement | app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt |
| T-49a-030 | P3 | S | none | Replace | Paging helper (dropped; list model replacement) | presentation-core/src/main/java/mihon/presentation/core/util/PagingDataUtil.kt |
| T-49a-054 | P3 | S | none | Drop | Build and packaging files (dropped) | presentation-core/build.gradle.kts |
| T-49a-056 | P3 | S | none | Drop | Haptics (dropped) | presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt |
| T-49b-018 | P3 | S | T-49b-017 | Drop | NavigationBar dropped (phone bottom bar) | presentation-core/src/main/java/tachiyomi/presentation/core/components/material/NavigationBar.kt |
| T-49b-032 | P3 | S | none | Drop | Clear focus on IME hide (dropped) | presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt |
| T-49b-036 | P3 | S | none | Drop | Scrollbar: dropped in favour of GTK overlay scrolling | presentation-core/src/main/java/tachiyomi/presentation/core/util/Scrollbar.kt |
| T-49b-037 | P3 | S | none | Drop | ThemePreviews annotation dropped | presentation-core/src/main/java/tachiyomi/presentation/core/util/ThemePreviews.kt |
| T-49ba-007 | P3 | S | T-49ba-002 | Drop | Drop Android dynamic colour (Monet) | app/src/main/res/values-v31/themes.xml |
| T-49ba-008 | P3 | S | none | Drop | Drop Android window bools and API-27 navigation-bar overrides | app/src/main/res/values/bools.xml |
| T-49ba-023 | P3 | S | T-49ba-009 | Drop | Drop the More-tab three-dot morph | app/src/main/res/drawable/anim_more_enter.xml |
| T-49ba-039 | P3 | S | none | Drop | Drop dead attributes and the unused NumberPicker | app/src/main/res/values/attrs.xml |
| T-49ba-040 | P3 | S | none | Drop | Drop the widget corner radii | app/src/main/res/values/dimens.xml |
| T-49ba-041 | P3 | S | none | Drop | Drop unused drawables (14 files) | app/src/main/res/drawable/ic_arrow_back_24dp.xml |
| T-49ba-042 | P3 | S | T-49ba-031 | Drop | Drop unused style items | app/src/main/res/values/styles.xml |
| T-49ba-047 | P3 | S | T-49ba-001 | Drop | Drop Android window attributes | app/src/main/res/values/themes.xml |
| T-50a-026 | P3 | S | T-50a-018 | Drop | Drop the four Android home and lock receivers and their manifest registrations | presentation-widget/src/main/AndroidManifest.xml |
| T-50a-027 | P3 | S | none | Drop | Drop the keyguard (lock-screen) providers | presentation-widget/src/main/res/xml/updates_grid_lockscreen_widget_info.xml |
| T-50a-028 | P3 | S | none | Drop | Drop the Samsung cover-screen metadata and cover-screen classes | presentation-widget/src/main/res/xml/updates_grid_samsung_cover_widget_info.xml |
| T-50a-029 | P3 | S | T-50a-019 | Drop | Drop push refresh (MangaWidgetManager / AnimeWidgetManager) in favour of the poll | presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/MangaWidgetManager.kt |
| T-50a-030 | P3 | S | T-50a-018 | Drop | Drop the Glance modifier helpers and rendering stack | presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt |
| T-50a-031 | P3 | S | none | Drop | Drop unused colour tokens | presentation-widget/src/main/res/values/colors_appwidget.xml |
| T-50a-032 | P3 | S | T-50a-018 | Drop | Drop the widget-picker preview images | presentation-widget/src/main/res/xml/updates_grid_homescreen_widget_info.xml |
| T-50a-033 | P3 | S | none | Drop | Drop the Gradle module wiring and empty or comment-only ProGuard files | presentation-widget/build.gradle.kts |
| T-51a-019 | P3 | S | T-51a-001 | Record (decision on 133 names) | Decision on 133 names with no literal Kotlin reference | i18n/src/commonMain/moko-resources/base/strings.xml (86 names) |
| T-51a-020 | P3 | S | T-51a-001 | Record (catalog data fixes) | Data-quality fixes in the Kurobayomi English catalog | i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml (`confirm_exit` |
| T-51a-022 | P3 | S | none | Drop | Drop Gradle, moko and Android manifest plumbing | i18n/build.gradle.kts |
| T-51a-023 | P3 | S | T-51a-006 | Drop | Drop the Android localisation API surface | core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt |
| T-51a-036 | P3 | S | area:35-data-misc-updater-notification-export | Drop | Drop notification channel concept, keep the text | i18n/src/commonMain/moko-resources/base/strings.xml (`channel_common` |
| T-51a-037 | P3 | S | none | Drop | Drop S-Pen strings | i18n/src/commonMain/moko-resources/base/strings.xml (`spen_previous_page` |
| T-52a-006 | P3 | S | none | Drop | RxJava to stream adapter (RxJavaExtensions) — dropped | app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt |
| T-52a-030 | P3 | S | none | Drop | Closeable array use (CloseableExtensions) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/lang/CloseableExtensions.kt |
| T-52a-035 | P3 | S | area:20-extension-strategy-redesign | Drop | Extension class loader (ChildFirstPathClassLoader) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/ChildFirstPathClassLoader.kt |
| T-52a-037 | P3 | S | none | Drop | Runtime permission check (hasPermission) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt |
| T-52a-040 | P3 | S | none | Drop | Default browser resolver (defaultBrowserPackageName) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt |
| T-52a-043 | P3 | S | none | Drop | Android package and installer checks (isPackageInstalled, MIUI, Shizuku, install permission) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt |
| T-52a-046 | P3 | S | none | Drop | Display cutout and navigation-bar scrim checks — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt |
| T-52a-049 | P3 | S | none | Drop | Intent extras getters (getParcelableExtraCompat, getSerializableExtraCompat) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt |
| T-52a-050 | P3 | S | none | Drop | System resource booleans (InternalResourceHelper) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/InternalResourceHelper.kt |
| T-52a-055 | P3 | S | none | Drop | TV box detection (isTvBox) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/TvUtils.kt |
| T-52a-057 | P3 | S | none | Drop | Foreground promotion helper (setForegroundSafely) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/WorkManagerExtensions.kt |
| T-52a-058 | P3 | S | area:47-settings-screens-general | Drop | EditText preference listener accessor — dropped | app/src/main/java/eu/kanade/tachiyomi/util/view/EditTextPreferenceExtensions.kt |
| T-52a-061 | P3 | S | area:36-ui-shell-navigation-security-webview | Drop | Compose host helpers (setComposeContent) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt |
| T-52a-063 | P3 | S | area:02-preferences-system | Drop | Dynamic colour and app-theme choice (isDynamicColorAvailable, pref_app_theme) — dropped | app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt |
| T-53a-028 | P3 | S | none | Drop | Drop WAKE_LOCK and battery-optimisation prompts | app/src/main/AndroidManifest.xml (lines 19 to 20) |
| T-53a-029 | P3 | S | T-53a-037 | Replace | Drop foreground-service permissions (replaced by holds and notices) | app/src/main/AndroidManifest.xml (lines 18 and 37) |
| T-53a-030 | P3 | S | T-53a-023 | Drop | Drop the notification permission prompt (onboarding step) | app/src/main/AndroidManifest.xml (line 31) |
| T-53a-032 | P3 | S | area:49-theme-and-design-system | Drop | Drop Android application attributes (backup, heap, legacy storage, icons) | app/src/main/AndroidManifest.xml (application element) |
| T-53a-033 | P3 | S | area:46-player-controls-ui | Drop | Picture-in-picture dropped; optional compact player mode | app/src/main/AndroidManifest.xml (PlayerActivity attributes) |
| T-53a-036 | P3 | S | area:20-extension-strategy-redesign | Drop | Extension install services and activities dropped | app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt |
| T-53a-038 | P3 | S | area:20-extension-strategy-redesign | Drop | Drop extension-related permissions and the Shizuku provider | app/src/main/AndroidManifest.xml (REQUEST_INSTALL_PACKAGES |
| T-53a-046 | P3 | S | T-53a-005 | Drop | Deep-link trampoline activities dropped | app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt |
| T-53a-052 | P3 | S | none | Drop | Drop the unreferenced player animations | app/src/main/res/anim/player_enter_bottom.xml |
| T-53a-056 | P3 | S | none | Drop | Drop the unused preference switch layout | app/src/main/res/layout/pref_widget_switch_material.xml |
| T-53a-059 | P3 | S | none | Drop | Drop the chapter download menu | app/src/main/res/menu/chapter_download.xml |
| T-53a-061 | P3 | S | area:36-ui-shell-navigation-security-webview | Drop | Drop the old bottom navigation menu | app/src/main/res/menu/main_nav.xml |
| T-53a-062 | P3 | S | none | Drop | Drop the md_* keep rule | app/src/main/res/raw/keep.xml |
| T-53a-064 | P3 | S | T-53a-039 | Drop | Drop FileProvider and the path whitelist | app/src/main/res/xml/provider_paths.xml |
| T-53a-065 | P3 | S | area:43-reader-core, area:46-player-controls-ui | Drop | Drop S Pen remote actions; keyboard parity | app/src/main/res/xml/s_pen_actions.xml |
| T-53a-067 | P3 | S | area:01-foundation-app-bootstrap | Drop | Drop the empty module manifests | core-metadata/src/main/AndroidManifest.xml |
| T-53a-068 | P3 | S | area:50-android-widgets | Drop | Drop the home-screen widget receivers | presentation-widget/src/main/AndroidManifest.xml |
| T-54a-014 | P3 | S | none | Drop | Drop upstream funding links | .github/FUNDING.yml |
| T-54a-022 | P3 | S | T-54a-001 | Drop | Drop the Android integer versionCode | app/build.gradle.kts |
| T-54a-024 | P3 | S | T-54a-023 | Drop | Drop the baseline profile and its generator | app/src/main/baseline-prof.txt |
| T-54a-025 | P3 | S | T-54a-023 | Drop | Drop the macrobenchmark module | macrobenchmark/build.gradle.kts |
| T-54a-026 | P3 | S | T-54a-001 | Drop | Drop ProGuard and consumer rule files | app/proguard-rules.pro |
| T-54a-030 | P3 | S | T-54a-029 | Drop | Drop the dead variant filter | app/build.gradle.kts |
| T-54a-036 | P3 | S | T-54a-001 | Drop | Drop buildSrc precompiled plugins | buildSrc/src/main/kotlin/mihon.android.application.compose.gradle.kts |
| T-54a-043 | P3 | S | T-54a-037 | Drop | Drop the Glance home-screen widget (presentation-widget) | presentation-widget/build.gradle.kts |
| T-54a-060 | P3 | S | T-54a-009 | Drop | Record drop decisions for Android-only CI steps | .github/workflows/build_pull_request.yml |
| T-req-platform-011 | P3 | XL | T-req-platform-003, T-req-platform-004, T-req-platform-005, T-req-platform-006, T-req-platform-007 | Defer (Windows, later milestone) | Windows port spike (later milestone) | app/src/main/AndroidManifest.xml |
