# Area 53 — Implementation tasks (platform manifest, intents and deep links, shortcuts, receivers, services and workers, res layouts, menus and animations)

Format is the repository task format (lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint <file>`). Priorities follow the plan rules: P0 needed for daily use, P1 parity most users rely on, P2 polish or long tail, P3 Android-only or obsolete with the replacement or drop decision recorded. Design details are in `p1-behavior.md`, `p1-data-and-config.md`, `p1-rust-design.md`, `p1-ui-parity.md` and `p1-edge-cases-and-tests.md`. Test IDs `TC-53-nn` and defect IDs `E-nn` are defined in `p1-edge-cases-and-tests.md`.

Every scope file is covered: the 56 files of the scope command appear in `p1-inventory.md` with at least one task ID. Dropped items have a P3 task that states the decision and the user loss.

### T-53a-001 — GTK application entry point replaces MainActivity (single window)
- Priority: P0
- Effort: M
- Depends: area:01-foundation-app-bootstrap, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/AndroidManifest.xml, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, src/main.rs
- Acceptance:
  - the binary starts one `gtk::ApplicationWindow` with application id `io.github.blackbox.Kurobayomi`, and `KUROBAYOMI_SMOKE=1 cargo run` still prints the `smoke:` line
  - the window opens on the Library tab when no launch request is given
  - the Android `isTaskRoot` check has no equivalent: a second process is handled by T-53a-007 and never creates a second window

### T-53a-002 — Desktop entry file with launch, D-Bus activation and MIME keys
- Priority: P1
- Effort: S
- Depends: T-53a-011, T-53a-003
- Sources: app/src/main/AndroidManifest.xml, app/shortcuts.xml
- Acceptance:
  - `io.github.blackbox.Kurobayomi.desktop` contains `Exec=kurobayomi %U`, `Icon=io.github.blackbox.Kurobayomi`, `StartupWMClass=io.github.blackbox.Kurobayomi`, `DBusActivatable=true`, `StartupNotify=true`
  - the file is byte-equal to the golden copy in `p1-rust-design.md` section 2.6 (test TC-53-36)
  - `desktop-file-validate` exits 0

### T-53a-003 — MIME type application/x-tachibk for backup files
- Priority: P1
- Effort: S
- Depends: T-53a-002
- Sources: app/src/main/AndroidManifest.xml (VIEW filter with `pathPattern .*\\.tachibk`), app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - the shared-mime-info file declares `application/x-tachibk`, subclass `application/gzip`, glob `*.tachibk` (test TC-53-37, `xmllint --noout` passes)
  - after installation `xdg-mime query filetype backup.tachibk` prints `application/x-tachibk`
  - the desktop entry lists the type in `MimeType=`

### T-53a-004 — Accept kurobayomi://, aniyomi:// and tachiyomi:// link routes
- Priority: P1
- Effort: S
- Depends: T-53a-002, T-53a-045
- Sources: app/src/main/AndroidManifest.xml (add-repo and extension-store filters), app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 543 to 563)
- Acceptance:
  - `parse_uri` in `kuro-core::launch::uri` returns `OpenUri` for the examples EC-12 to EC-15 and the route is the one in `p1-data-and-config.md` section 4.2
  - `kurobayomi://unknown-host` returns `UnknownHost` (EC-18) and a toast is shown
  - `ftp://` returns `UnsupportedScheme` (EC-17)

### T-53a-005 — Search routes: --search, shared text, extension search kind fix, URL text resolve
- Priority: P1
- Effort: M
- Depends: area:36-ui-shell-navigation-security-webview, T-53a-045
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaActivity.kt, app/src/main/AndroidManifest.xml, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 497 to 542)
- Acceptance:
  - EC-05 to EC-08 and EC-11 pass (empty query, kind given, default kind anime, URL text)
  - `eu.kanade.tachiyomi.ANIMESEARCH` always opens anime search whatever `type` says (defect E-17 not reproduced; test TC-53-02)
  - with `--search=` empty nothing is pushed

### T-53a-006 — Command-line and open handling in GApplication
- Priority: P0
- Effort: M
- Depends: T-53a-001, area:36-ui-shell-navigation-security-webview
- Sources: src/main.rs, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 461 to 573)
- Acceptance:
  - `--action`, `--id`, `--search`, `--type` and positional file or URL arguments are parsed by `parse_argv` (test TC-53-01)
  - `connect_command_line` and `connect_open` are connected; requests before the window exists are queued and applied once after `connect_activate` (test TC-53-32)
  - `kurobayomi --help` lists the options through GLib

### T-53a-007 — Single instance: a second launch is forwarded to the running window
- Priority: P0
- Effort: S
- Depends: T-53a-006
- Sources: app/src/main/AndroidManifest.xml (`launchMode="singleTop"`), app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 169 to 172, 357 to 368)
- Acceptance:
  - starting `kurobayomi --action=show-updates` while the app runs presents the existing window on the Updates tab (test TC-53-33, manual)
  - only one window exists after two launches
  - the `SingleInstance::forward` implementation is GApplication's own forwarding; no custom socket is added

### T-53a-008 — Keep the process alive while background jobs run with no window
- Priority: P1
- Effort: S
- Depends: T-53a-037
- Sources: app/src/main/AndroidManifest.xml (services with `dataSync`), app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - closing the last window while a torrent or HTTP server runs keeps the process alive (test TC-53-30)
  - when the last hold is released and no window exists, the process exits
  - the quit policy is documented in the application help text

### T-53a-009 — OAuth redirect routing for tracker hosts (AniList fragment fix)
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt
- Acceptance:
  - EC-19 to EC-23 pass (test TC-53-15); a token that is the last fragment parameter is accepted (defect E-22)
  - an unknown `*-auth` host shows a toast and returns to Settings (defect E-23, test TC-53-03)
  - the five hosts `anilist-auth`, `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth` route to area:27-tracking-core

### T-53a-010 — Open .tachibk files from the desktop and the command line
- Priority: P1
- Effort: S
- Depends: T-53a-003, T-53a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 543 to 548), app/src/main/AndroidManifest.xml (VIEW filter)
- Acceptance:
  - EC-09 opens the restore screen for the path; EC-10 is `Unrecognised`; EC-24 keeps a path with spaces and non-ASCII characters exactly (test TC-53-14)
  - the check is the path extension after URL decoding, not a suffix test on the full URI (defect E-11)
  - the file is only parsed by the restore screen after the user confirms (area:22-backup-restore)

### T-53a-011 — Desktop actions for the five shortcuts (replaces shortcuts.xml)
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/shortcuts.xml, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 471 to 496), core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt
- Acceptance:
  - the `.desktop` file has five `[Desktop Action]` groups: show-library, show-animelib, show-updates, show-history, show-browse
  - all five action ids are distinct (defect E-24, test TC-53-19)
  - each action opens its tab as in `p1-data-and-config.md` section 4.2

### T-53a-012 — Localised desktop action labels
- Priority: P2
- Effort: S
- Depends: area:51-i18n, T-53a-011
- Sources: app/shortcuts.xml, i18n/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - each label comes from a string key with the English value in `p1-ui-parity.md` section 4
  - packaging writes the translated `Name=` lines for each installed locale

### T-53a-013 — Packaging permission audit for the sandbox
- Priority: P1
- Effort: S
- Depends: T-53a-002
- Sources: app/src/main/AndroidManifest.xml (uses-permission block)
- Acceptance:
  - the Flatpak manifest (if one is published) grants network and the notifications and file-chooser portals, and no `filesystem=host`
  - each Android permission in `p1-data-and-config.md` section 2 has a recorded replacement or drop

### T-53a-014 — Launch and notice logging targets and the verbose switch
- Priority: P2
- Effort: S
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt (line 160), app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt (line 66)
- Acceptance:
  - the verbose preference enables the tracing filter for `kurobayomi::launch` and `kurobayomi::notice`
  - with verbose off, no payload contents are logged (only the action name)

### T-53a-015 — One reader and one player window at a time (singleTask)
- Priority: P1
- Effort: S
- Depends: area:43-reader-core, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt (lines 107 to 113), app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt (lines 158 to 174), app/src/main/AndroidManifest.xml
- Acceptance:
  - opening a chapter while the reader is open replaces the displayed chapter, with no second reader window
  - the same holds for the player and episodes
  - a request for a missing manga or chapter id finishes without opening anything (Kotlin `finish()` at `ReaderActivity.kt` lines 163 to 166)

### T-53a-017 — Notice dispatcher replacing NotificationReceiver (app.notice GAction)
- Priority: P1
- Effort: M
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt
- Acceptance:
  - every `AppAction` in `p1-data-and-config.md` section 5 is dispatched; the dead constants `ACTION_OPEN_CHAPTER`, `ACTION_OPEN_EPISODE`, `ACTION_OPEN_ENTRY` are not created (test TC-53-24)
  - the handler never calls a blocking call on the GTK thread (defect E-03, test TC-53-08)
  - cancel restore withdraws the restore notice id (defect E-28, test TC-53-25)

### T-53a-018 — AppAction payload codec and validation
- Priority: P1
- Effort: M
- Depends: T-53a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt (extras lines 412 to 417 and builders), app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt
- Acceptance:
  - EN-02 to EN-07 pass: 500 URL limit, 2048-byte URL limit, https-only for update and open URL, absolute paths under Paths roots, malformed JSON logged
  - no missing extra panics (defect E-04, test TC-53-05); two notices keep their own payloads (defect E-05, test TC-53-09)
  - payloads over 64 KiB are rejected before parsing

### T-53a-019 — Dismiss withdraws one id only (no bulk withdraw)
- Priority: P1
- Effort: S
- Depends: T-53a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt (lines 66 to 70, 191 to 194, 554 to 585)
- Acceptance:
  - EN-01 passes and test TC-53-07 records exactly one withdraw call
  - no `cancelAll` equivalent exists on the `Notifier` trait (defect E-01)
  - the group-summary rule is not ported (defect E-06 documented; manual check TC-53-10 recorded in T-53a-076)

### T-53a-020 — Stable notice id registry (replaces Android integer ids)
- Priority: P1
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt (lines 116 to 188)
- Acceptance:
  - every constant in `p1-data-and-config.md` section 3.3 has a `NoticeId` constant or a recorded drop
  - ids are distinct across features: save image and app-update prompt no longer share a value (defect E-19), and entry notices include the kind (defect E-31); test TC-53-11
  - all ids satisfy the invariant (test EF-06)

### T-53a-021 — Notification categories and switches (replaces the 13 channels)
- Priority: P1
- Effort: S
- Depends: area:35-data-misc-updater-notification-export, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt (lines 190 to 291), app/src/main/java/eu/kanade/tachiyomi/App.kt (lines 238 to 244)
- Acceptance:
  - nine categories with `notify.enabled.<category>` switches default on, mapped from the channels in `p1-data-and-config.md` section 3.2
  - restarting the app does not reset a switch (defect E-02, test TC-53-17)
  - the deprecated-channel deletion is not ported

### T-53a-022 — Incognito notice and disable action
- Priority: P1
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt (lines 112 to 139, 246 to 274), app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (line 216)
- Acceptance:
  - the notice is present exactly while incognito is on (test TC-53-35, last state wins)
  - its single button "Disable incognito mode" sets the preference to off; the runtime receiver registration is not needed
  - incognito is off after each launch

### T-53a-023 — Notification server availability and in-app toast fallback
- Priority: P1
- Effort: S
- Depends: T-53a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt (lines 29 to 40)
- Acceptance:
  - `server_available() == false` shows the notice text as a toast (test TC-53-06, EN-10)
  - a category switched off posts nothing and shows no toast (EN-11)
  - no silent drop remains (defect E-12)

### T-53a-024 — Download and update gates read one NetworkStatus
- Priority: P1
- Effort: S
- Depends: T-53a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt (lines 8 to 25), app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt (lines 87 to 95)
- Acceptance:
  - `is_online` equals `is_connected && is_validated` (same rule as Kotlin `isOnline`)
  - the Wi-Fi-only download gate reads `NetworkStatus::current()` only (no other source)
  - tested with `FixedNetwork` (tests TC-53-16 and TC-53-43, built in T-53a-074)

### T-53a-025 — Metered-connection policy
- Priority: P2
- Effort: S
- Depends: T-53a-026
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadJob.kt
- Acceptance:
  - `is_metered` comes from GIO `network_metered()` and NetworkManager `Metered` values 1 or 3
  - the policy is written in the settings help text; with an unknown state no behaviour changes

### T-53a-026 — Wi-Fi and network state through NetworkManager (replaces ACCESS_WIFI_STATE)
- Priority: P1
- Effort: M
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt (line 32), app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt
- Acceptance:
  - `is_wifi` is true only when the radio is enabled and the primary connection type is `802-11-wireless` (test TC-53-43)
  - with NetworkManager absent `is_wifi` is false and downloads that require Wi-Fi are blocked (defect E-15, test TC-53-16)
  - an unknown D-Bus property is treated as unknown, not as false

### T-53a-027 — Folder chooser through the portal (replaces MANAGE_EXTERNAL_STORAGE)
- Priority: P1
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt (line 187), app/src/main/AndroidManifest.xml
- Acceptance:
  - the download and backup folder pickers use `gtk::FileDialog::select_folder`
  - in Flatpak the granted folder survives a restart through the portal document store
  - the app never asks for all-files access

### T-53a-028 — Drop WAKE_LOCK and battery-optimisation prompts
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/AndroidManifest.xml (lines 19 to 20), app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - no battery-optimisation setting or prompt exists on Linux; the decision and "no user loss" are recorded in `p1-data-and-config.md` section 2
  - no wake lock is taken anywhere in the Rust code

### T-53a-029 — Drop foreground-service permissions (replaced by holds and notices)
- Priority: P3
- Effort: S
- Depends: T-53a-037
- Sources: app/src/main/AndroidManifest.xml (lines 18 and 37)
- Acceptance:
  - no `startForeground` equivalent is needed; long jobs are covered by `BackgroundHold` (test TC-53-30)
  - the drop is recorded with the user-visible replacement (progress is shown in the download screen and in notices)

### T-53a-030 — Drop the notification permission prompt (onboarding step)
- Priority: P3
- Effort: S
- Depends: T-53a-023
- Sources: app/src/main/AndroidManifest.xml (line 31), app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt
- Acceptance:
  - no notification permission step exists on Linux
  - the toast fallback of T-53a-023 covers a missing notification server

### T-53a-031 — Language selection replaces per-app locales
- Priority: P2
- Effort: S
- Depends: area:51-i18n
- Sources: app/src/main/AndroidManifest.xml (READ_APP_SPECIFIC_LOCALES and AppLocalesMetadataHolderService), app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt
- Acceptance:
  - the in-app language setting applies after restart
  - no AppCompat locale service or per-app locale permission is ported
  - test TC-53-23 (string key coverage) passes for the chosen locale

### T-53a-032 — Drop Android application attributes (backup, heap, legacy storage, icons)
- Priority: P3
- Effort: S
- Depends: area:49-theme-and-design-system
- Sources: app/src/main/AndroidManifest.xml (application element)
- Acceptance:
  - every attribute in `p1-data-and-config.md` section 2 has a recorded replacement or drop
  - the icon and label move to the desktop entry (T-53a-041)

### T-53a-033 — Picture-in-picture dropped; optional compact player mode
- Priority: P3
- Effort: S
- Depends: area:46-player-controls-ui
- Sources: app/src/main/AndroidManifest.xml (PlayerActivity attributes), app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - the app never requests always-on-top; the user loss (floating window) is recorded in `p1-ui-parity.md` section 11
  - if compact mode is built, it uses a fixed aspect-preserving size and hides controls

### T-53a-034 — App update download as a unique job
- Priority: P1
- Effort: M
- Depends: area:35-data-misc-updater-notification-export, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - a new start cancels the running download (REPLACE semantics; test TC-53-29) and a cancel withdraws only the progress notice (defect E-33, test TC-53-27)
  - the file is written under `Paths` cache as an absolute path; a missing cache path is a visible error (defect E-13, test TC-53-18)
  - the title passed to the start action is shown (defect E-25)

### T-53a-035 — Torrent and HTTP servers as background services with notices and Stop
- Priority: P1
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent, area:45-player-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt, app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt
- Acceptance:
  - each server has start and stop functions and one persistent notice with a "Stop" button (test TC-53-12: no delete-restart, defect E-08)
  - the Stop label comes from a string key (defect E-20, test TC-53-23)
  - the HTTP server's misspelled identifiers are not kept (defect E-26)
  - the 10-second wait and the 20-second default wait of the Kotlin helper are kept as `tokio::time::timeout` values

### T-53a-036 — Extension install services and activities dropped
- Priority: P3
- Effort: S
- Depends: area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallActivity.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt
- Acceptance:
  - no APK install code path exists on Linux; extension install follows area:20-extension-strategy-redesign
  - the lost capability (installing APK extensions) is recorded with the area:20-extension-strategy-redesign replacement
  - the MIUI timing workaround (defect E-29) is not carried

### T-53a-037 — BackgroundHold registry
- Priority: P1
- Effort: S
- Depends: T-53a-006
- Sources: app/src/main/AndroidManifest.xml (SystemForegroundService and dataSync services), app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt
- Acceptance:
  - holds are acquired and released on the GTK main thread; `active()` lists the labels (test TC-53-30)
  - the registry counts per label and takes the GIO hold on the first acquire and releases it on the last

### T-53a-038 — Drop extension-related permissions and the Shizuku provider
- Priority: P3
- Effort: S
- Depends: area:20-extension-strategy-redesign
- Sources: app/src/main/AndroidManifest.xml (REQUEST_INSTALL_PACKAGES, REQUEST_DELETE_PACKAGES, UPDATE_PACKAGES_WITHOUT_USER_ACTION, QUERY_ALL_PACKAGES, ShizukuProvider), app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt
- Acceptance:
  - no package query or install permission is needed on Linux
  - the `extension_installer` preference is not ported
  - the user loss (Shizuku-based install) is recorded

### T-53a-039 — Opener trait: open URL, open file, reveal
- Priority: P1
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt (openImagePendingActivity, openUrl), app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt (openErrorLogPendingActivity), app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt
- Acceptance:
  - `gtk::UriLauncher` and `gtk::FileLauncher` are used; no `xdg-open` call exists outside `kuro-platform` (addendum rule 1)
  - `reveal` uses `org.freedesktop.FileManager1.ShowItems` with a fallback to the parent folder (manual test TC-53-41)

### T-53a-040 — Share replacement: Show in folder and Copy link
- Priority: P2
- Effort: S
- Depends: T-53a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt (shareImage and shareFile)
- Acceptance:
  - no share chooser is opened; "Show in folder" reveals the file and "Copy link" copies the URL
  - the backup share MIME `application/x-protobuf+gzip` is kept only as the backup format's metadata (area:21-backup-format-and-create)

### T-53a-041 — Desktop packaging: MIME registration, icon names and desktop database
- Priority: P1
- Effort: S
- Depends: T-53a-002, T-53a-003
- Sources: app/src/main/AndroidManifest.xml (icon and roundIcon), app/src/main/res/drawable/ic_ani.xml
- Acceptance:
  - the icon `io.github.blackbox.Kurobayomi` resolves in the hicolor theme at 48, 128 and 256 pixels
  - the package post-install hook runs `update-desktop-database` and `update-mime-database`
  - `xdg-mime query default x-scheme-handler/kurobayomi` returns the desktop file

### T-53a-042 — Notification builders: open image and open URL; drop APK install
- Priority: P1
- Effort: S
- Depends: T-53a-017, T-53a-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt (lines 21 to 93)
- Acceptance:
  - the download-manager notice opens the Downloads route (`Shortcut { Downloads }`, test TC-53-02)
  - open-image and open-URL notices use `Opener`; the APK install builder is absent

### T-53a-043 — Mark read and seen with optional download removal
- Priority: P1
- Effort: S
- Depends: T-53a-017, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt (lines 298 to 348)
- Acceptance:
  - EN-08 and EN-09 pass; the manga is fetched once per batch (defect E-30, test TC-53-26)
  - applying twice gives the same database state (test TC-53-28)
  - the chapter or episode `read` or `seen` flag and the version triggers of `p1-data-and-config.md` section 9 are preserved

### T-53a-044 — Progress throttle and app-update strings
- Priority: P1
- Effort: S
- Depends: T-53a-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt (lines 82 to 98), app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt
- Acceptance:
  - sequences ED-01 to ED-04 match (test TC-53-13); total 0 gives no notice (defect E-21)
  - the update strings `update_check_notification_*` and `action_download` are in the Kurobayomi string table
  - the Proposal strings `deeplink_no_source`, `deeplink_search_instead`, `track_connecting`, `route_missing_id`, `notice_invalid`, `open_failed` exist in the string table with the English values of `p1-ui-parity.md` and `p1-edge-cases-and-tests.md`

### T-53a-045 — Route errors and kind parsing without crashes
- Priority: P1
- Effort: S
- Depends: T-53a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 471 to 510), app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/DeepLinkScreenType.kt
- Acceptance:
  - an unknown `type` returns `RouteError::UnknownKind` and a toast (defect E-14, test TC-53-03, EC-07)
  - `SHOW_MANGA` without an id returns `RouteError::MissingId` and a toast (defect E-18, EC-02)
  - every route error produces a visible message (defect E-16, test TC-53-04)

### T-53a-046 — Deep-link trampoline activities dropped
- Priority: P3
- Effort: S
- Depends: T-53a-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaActivity.kt
- Acceptance:
  - `route_request` receives the search request directly; no trampoline window exists
  - the user loss is none (the forwarding was internal)

### T-53a-047 — Anime deep-link loading page with states and error panel
- Priority: P1
- Effort: M
- Depends: T-53a-048
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreen.kt
- Acceptance:
  - Loading shows the title "Search…" and a spinner; NoResults opens the global anime search with the query; Result opens the entry or the player (test TC-53-22)
  - a resolve error shows Retry and "Search instead" (defect E-07)
  - navigation happens once per state change, never inside the render function (defect E-10)

### T-53a-048 — Anime deep-link resolver in kuro-core
- Priority: P1
- Effort: M
- Depends: area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreenModel.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ResolvableAnimeSource.kt
- Acceptance:
  - test TC-53-38 covers the local hit, network insert and episode fetch branches
  - an exception becomes `Failed { reason }` (test TC-53-20)
  - an episode result goes through the player router so the external-player setting applies (defect E-09, test TC-53-21)

### T-53a-049 — Manga deep-link loading page
- Priority: P1
- Effort: S
- Depends: T-53a-050, T-53a-047
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreen.kt
- Acceptance:
  - the states match T-53a-047; NoResults opens global manga search
  - a chapter result opens the reader (test TC-53-22)

### T-53a-050 — Manga deep-link resolver in kuro-core
- Priority: P1
- Effort: M
- Depends: area:15-source-api-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ResolvableSource.kt
- Acceptance:
  - test TC-53-39: the chapter list sync is called with the `false` flag (`syncChaptersWithSource.await(..., false)`)
  - only catalogue sources are considered, in the order of `getCatalogueSources()`
  - errors become `Failed { reason }` (defect E-07)

### T-53a-051 — Shared-axis page transitions as GTK Stack transitions
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/res/anim-v33/shared_axis_x_pop_enter.xml, app/src/main/res/anim-v33/shared_axis_x_pop_exit.xml, app/src/main/res/anim-v33/shared_axis_x_push_enter.xml, app/src/main/res/anim-v33/shared_axis_x_push_exit.xml, app/src/main/res/anim/shared_axis_x_pop_enter.xml, app/src/main/res/anim/shared_axis_x_pop_exit.xml, app/src/main/res/anim/shared_axis_x_push_enter.xml, app/src/main/res/anim/shared_axis_x_push_exit.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt (lines 143 to 156, 292 to 306), app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt (lines 38 to 52, 96 to 110)
- Acceptance:
  - push uses `SlideLeft` and pop uses `SlideRight` with a 300 ms duration
  - the alpha component is not reproduced; the difference is in `p1-ui-parity.md` section 3
  - when `gtk-enable-animations` is false the switch is immediate (test TC-53-42)

### T-53a-052 — Drop the unreferenced player animations
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/anim/player_enter_bottom.xml, app/src/main/res/anim/player_enter_left.xml, app/src/main/res/anim/player_enter_right.xml, app/src/main/res/anim/player_enter_top.xml, app/src/main/res/anim/player_exit_bottom.xml, app/src/main/res/anim/player_exit_left.xml, app/src/main/res/anim/player_exit_right.xml, app/src/main/res/anim/player_exit_top.xml, app/src/main/res/anim/player_fade_in.xml, app/src/main/res/anim/player_fade_out.xml
- Acceptance:
  - no `R.anim.player_*` reference exists (verified in the inventory); the ten files are not ported
  - the player has no transition, as in Aniyomi

### T-53a-053 — Download section header with a reorder handle
- Priority: P1
- Effort: M
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/res/layout/download_header.xml, app/src/main/res/layout/download_list.xml, src/screens.rs (function downloads, line 354)
- Acceptance:
  - each section header shows the title and a drag handle; section order can be changed by dragging the header
  - the list is a scrolling container with the padding behaviour of `download_list.xml` (content scrolls under the padding)
  - the mockup's header is updated (`p1-ui-parity.md` section 5.1)

### T-53a-054 — Download item row layout parity
- Priority: P1
- Effort: S
- Depends: T-53a-053
- Sources: app/src/main/res/layout/download_item.xml, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt
- Acceptance:
  - each row shows: drag handle, series title (one line, ellipsis), chapter or episode title (12 sp), progress bar, counter text, overflow button
  - the counter and progress text formats agree with areas 23 and 24 (the mockup's anime percentage is verified against `AnimeDownloadHolder.kt`)

### T-53a-055 — Player root: mpv GL area with controls overlay
- Priority: P0
- Effort: M
- Depends: area:45-player-core, area:46-player-controls-ui
- Sources: app/src/main/res/layout/player_layout.xml, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - the video renders in a `GtkGLArea` with the controls overlay above it
  - in immersive mode the sidebar and tab bar hide (mockup `src/main.rs` lines 58 to 59)

### T-53a-056 — Drop the unused preference switch layout
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/layout/pref_widget_switch_material.xml
- Acceptance:
  - no `PrefWidgetSwitchMaterialBinding` or layout reference exists; the drop is recorded

### T-53a-057 — Reader root layout: viewer, page number pill, overlays, dialog host
- Priority: P0
- Effort: M
- Depends: area:43-reader-core, area:44-reader-viewers
- Sources: app/src/main/res/layout/reader_activity.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - the page number sits bottom-centre over the viewer; the navigation overlay is hidden by default and does not take input
  - a dialog host exists above the viewer for reader dialogs

### T-53a-058 — Reader error panel: message, Retry, Open in WebView
- Priority: P1
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/res/layout/reader_error.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - the message "The image couldn't be loaded", a Retry button and an "Open in WebView" button shown only for sources with a web URL
  - Retry decodes the page again

### T-53a-059 — Drop the chapter download menu
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/menu/chapter_download.xml
- Acceptance:
  - no `R.menu.chapter_download` reference exists; the Start, Delete and Cancel actions are in the row menu (T-53a-060)

### T-53a-060 — Download row overflow menu wired (six items)
- Priority: P1
- Effort: M
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/res/menu/download_single.xml, app/src/main/java/eu/kanade/tachiyomi/ui/download/manga/MangaDownloadHolder.kt (line 90), app/src/main/java/eu/kanade/tachiyomi/ui/download/anime/AnimeDownloadHolder.kt (line 100)
- Acceptance:
  - the six items (Move to top, Move series to top, Move to bottom, Move series to bottom, Cancel, Cancel all for this series) act on the queue and are in that order
  - the mockup callbacks (`|_| {}` in `src/screens.rs`) are replaced
  - the menu is a GTK popover with a `gio::Menu` model

### T-53a-061 — Drop the old bottom navigation menu
- Priority: P3
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/res/menu/main_nav.xml
- Acceptance:
  - no `R.menu.main_nav` reference exists; navigation is the `HomeScreen` tab set of area:36-ui-shell-navigation-security-webview and the icon `ic_animelibrary_selector_24dp` is decided in area:49-theme-and-design-system

### T-53a-062 — Drop the md_* keep rule
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/raw/keep.xml
- Acceptance:
  - no `md_*` layout exists in the project; the rule has no runtime effect and is not ported

### T-53a-063 — HTTP source TLS policy (cleartext allowed, system and user roots)
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/res/xml/network_security_config.xml, app/src/main/AndroidManifest.xml (networkSecurityConfig)
- Acceptance:
  - an `http://` source request succeeds (cleartext allowed as in Aniyomi)
  - a user-installed CA is trusted (reqwest with rustls and `rustls-native-certs` plus `webpki-roots` fallback)

### T-53a-064 — Drop FileProvider and the path whitelist
- Priority: P3
- Effort: S
- Depends: T-53a-039
- Sources: app/src/main/res/xml/provider_paths.xml, core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt
- Acceptance:
  - no `content://` URI is created by the Rust code; files are opened through `Opener`
  - the drop is recorded: the user loses nothing visible

### T-53a-065 — Drop S Pen remote actions; keyboard parity
- Priority: P3
- Effort: S
- Depends: area:43-reader-core, area:46-player-controls-ui
- Sources: app/src/main/res/xml/s_pen_actions.xml, app/src/main/AndroidManifest.xml (REMOTE_ACTION filters)
- Acceptance:
  - keys Right, Left, Page Up, Page Down, N and P exist in the reader and player keymaps (`p1-ui-parity.md` section 6)
  - Escape and Alt+Left go back in the reader and web view

### T-53a-066 — Desktop search provider (GNOME SearchProvider2 and KRunner)
- Priority: P2
- Effort: M
- Depends: T-53a-006, T-53a-007
- Sources: app/src/main/res/xml/searchable.xml
- Acceptance:
  - the `.ini` file has `Version=2`, `DesktopId`, `BusName` and `ObjectPath` (test EF-07)
  - activating a search result sends the Search route to the running instance

### T-53a-067 — Drop the empty module manifests
- Priority: P3
- Effort: S
- Depends: area:01-foundation-app-bootstrap
- Sources: core-metadata/src/main/AndroidManifest.xml, core/archive/src/main/AndroidManifest.xml, core/common/src/main/AndroidManifest.xml, data/src/main/AndroidManifest.xml, domain/src/main/AndroidManifest.xml, i18n/src/main/AndroidManifest.xml, macrobenchmark/src/main/AndroidManifest.xml, presentation-core/src/main/AndroidManifest.xml, source-api/src/androidMain/AndroidManifest.xml, source-local/src/androidMain/AndroidManifest.xml
- Acceptance:
  - each module is a Cargo crate in the workspace (area:01-foundation-app-bootstrap); none needs a manifest
  - the macrobenchmark module is not carried; the decision is recorded

### T-53a-068 — Drop the home-screen widget receivers
- Priority: P3
- Effort: S
- Depends: area:50-android-widgets
- Sources: presentation-widget/src/main/AndroidManifest.xml
- Acceptance:
  - no widget receiver or provider XML is ported; the Updates tab (area:42-ui-updates-history-stats-storage-upcoming) and desktop notices (area:35-data-misc-updater-notification-export) are the replacements
  - the loss is recorded in `p1-data-and-config.md` section 2 and `p1-behavior.md` section 13.4

### T-53a-069 — Startup: splash dropped, loading state while the migrator runs
- Priority: P2
- Effort: S
- Depends: area:01-foundation-app-bootstrap, area:11-app-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt (lines 158 to 172, 664 to 667), app/src/main/AndroidManifest.xml (CrashActivity block)
- Acceptance:
  - no splash timers (500 ms minimum, 5000 ms maximum, 400 ms exit) are ported
  - the library page shows a loading state until the migrator finishes; the crash screen is owned by area:01-foundation-app-bootstrap

### T-53a-070 — App lock: quit on authentication error, unlock on success
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt
- Acceptance:
  - an authentication error quits the application (Android `finishAffinity`)
  - success unlocks and shows the main window; the title "Unlock Kurobayomi" uses the Unlock string key

### T-53a-071 — Web view page behaviour and back action
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt
- Acceptance:
  - the page title comes from the `title_key` value; a back action labelled "Back" goes back in history
  - the open and close transitions follow T-53a-051

### T-53a-072 — Route and argv unit tests
- Priority: P1
- Effort: S
- Depends: T-53a-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt
- Acceptance:
  - tests TC-53-01, TC-53-02, TC-53-03, TC-53-14 and TC-53-15 pass in `cargo test -p kuro-core`
  - the fixtures of `p1-edge-cases-and-tests.md` section 4 are used

### T-53a-073 — Notice dispatch and payload tests
- Priority: P1
- Effort: M
- Depends: T-53a-018, T-53a-019
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - tests TC-53-04, TC-53-05, TC-53-07, TC-53-08, TC-53-09, TC-53-11, TC-53-12, TC-53-25 and TC-53-31 pass with `RecordingNotifier`

### T-53a-074 — Progress, network, resolver and worker tests
- Priority: P1
- Effort: M
- Depends: T-53a-044, T-53a-026, T-53a-048, T-53a-050
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreenModel.kt
- Acceptance:
  - tests TC-53-13, TC-53-16, TC-53-20, TC-53-21, TC-53-22, TC-53-26, TC-53-27, TC-53-29, TC-53-30, TC-53-38, TC-53-39 and TC-53-43 pass
  - the race rows R-01 to R-10 of `p1-edge-cases-and-tests.md` section 2 have a test or a manual check

### T-53a-075 — Desktop file, MIME and shortcut golden tests
- Priority: P1
- Effort: S
- Depends: T-53a-002, T-53a-003
- Sources: app/shortcuts.xml
- Acceptance:
  - tests TC-53-19 and TC-53-36 and TC-53-37 pass with the golden copies in `fixtures/desktop` and `fixtures/mime`
  - EF-01 to EF-05 are run in CI

### T-53a-076 — Manual desktop checklist for notices, reveal and single instance
- Priority: P2
- Effort: S
- Depends: T-53a-040, T-53a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt
- Acceptance:
  - manual tests TC-53-10, TC-53-33, TC-53-40, TC-53-41 and TC-53-42 are recorded with the desktop environment (KDE Plasma and GNOME) and the result

### T-53a-077 — Dead-code guard test for dropped notification actions
- Priority: P2
- Effort: S
- Depends: T-53a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt (lines 104 to 118, 400)
- Acceptance:
  - test TC-53-24 fails the build if `OpenChapter`, `OpenEpisode` or `OpenEntry` is constructed outside the enum definition

### T-53a-078 — String key coverage for the scope
- Priority: P1
- Effort: S
- Depends: area:51-i18n
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - test TC-53-23 checks that every key in `p1-data-and-config.md` section 10.3 exists in the Kurobayomi string table with the English value
  - the Stop label of the torrent notice is a key, not a literal (defect E-20)
