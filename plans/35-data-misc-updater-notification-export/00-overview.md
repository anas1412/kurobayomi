# Area 35 - Notifications, app updater, library CSV export, misc data services

Overview and area lead summary. Every Kotlin path is repo-relative to `/home/blackbox/aniyomi-src` (read-only). Every Rust path is relative to `/home/blackbox/kurobayomi`. Other areas are referenced as `area:<id>`.

Scope: 11 files, 1,875 LOC (7 Kotlin files, 4 Android XML resources), listed in [p1-inventory.md](p1-inventory.md).

---

## 1. Purpose and scope

### What Aniyomi does here

| Feature | Kotlin origin | What the user sees |
|---|---|---|
| Notification channels and id registry | `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt` | Channels in system settings (per-app mutable there), 41 constants (channel, group and notification ids) |
| Notification action router | `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt` (999 LOC) | Buttons on notices: mark read/seen, download, pause/resume/clear queues, cancel jobs, dismiss, share, open entry |
| PendingIntent factory | `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt` | Opens MainActivity shortcuts, image viewer, APK installer, URL |
| App update check | `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt` (+ domain `GetApplicationRelease`, data `ReleaseServiceImpl`) | GitHub latest-release lookup, at most every 3 days automatically, forced from About |
| App update download | `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt` | APK into external cache with progress notice, then system installer |
| Update notices | `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt` | "New version available!", progress, install prompt, error |
| Library CSV export | `app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt` | Save all favourites (manga and anime) to a CSV with selectable columns |
| Android XML glue | `app/src/main/res/xml/network_security_config.xml`, `provider_paths.xml`, `s_pen_actions.xml`, `searchable.xml` | Cleartext HTTP, FileProvider paths, Samsung S Pen, global-search hint |

### Why it matters for a usable Kurobayomi

- Every long job in the app (library update 26, downloads 23/24, backup 21/22, extension install 18/19, save image 34) reports back through the notice system and offers buttons on it. Without this area those producers have nothing to call.
- The updater is the only self-update path. Without it a user never learns about new builds.
- The CSV export is the only way to take the favourites list out of the app.
- The network config (T-35a-031) is the only P0 item in this area: the HTTP client must trust the system CA store and allow `http://` sources, because source sites are often plain HTTP.

### Scope boundaries

In scope: the 11 files, their Linux replacements, the Rust modules `src/notify`, `src/update`, `src/export`, and the traits in `kuro-platform` that this area needs (`Notifier`, `Opener`, `Paths`, `NetworkStatus`).

Out of scope, owned elsewhere: the producers of notices (areas 21-26, 34, 13, 18, 19, 01, 53), the release domain and HTTP client (`area:09-domain-source-track-download-release-misc`, `area:06-data-repos-history-track-source-misc`, `area:12-network-stack`), the settings UI (`area:47-settings-screens-general`), the navigation shell (`area:36-ui-shell-navigation-security-webview`).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (scope plus the code it calls)

| Package or file (repo-relative) | Responsibility |
|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/notification/` | Notification ids, channels, PendingIntents, action receiver |
| `app/src/main/java/eu/kanade/tachiyomi/data/updater/` | Update check, background APK download, update notices |
| `app/src/main/java/eu/kanade/tachiyomi/data/export/` | Favourites to CSV |
| `app/src/main/res/xml/` | network security, FileProvider, S Pen, searchable resources |
| `app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt` | `Context.notificationBuilder`, `Context.notify` (outside scope, the posting helper) |
| `app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt` | `isPreviewBuildType`, `updaterEnabled` build switches |
| `domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`, `domain/src/main/java/tachiyomi/domain/release/service/ReleaseService.kt`, `domain/src/main/java/tachiyomi/domain/release/model/Release.kt` | Check algorithm and version rule (specified here) |
| `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt`, `data/src/main/java/tachiyomi/data/release/GithubRelease.kt` | GitHub HTTP contract and asset selection (specified here) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt`, `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt` | Update page (specified here) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt` | "Check for updates" row (specified here) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` | Export dialog (specified here) |

### 2.2 Proposed Rust layout (from [p1-rust-design.md](p1-rust-design.md) section 1)

```
crates/kuro-platform/src/notifier.rs      trait Notifier, Notice, NoticeId, Category, Priority, NoticeAction, NotifyError
crates/kuro-platform/src/opener.rs        trait Opener, OpenError
crates/kuro-platform/src/paths.rs         trait Paths
crates/kuro-platform/src/network.rs       trait NetworkStatus
crates/kuro-platform/src/icons.rs         bundled symbolic icon names
crates/kuro-platform/src/linux/notifier.rs  GioNotifier (gio::Notification on gtk::Application)
crates/kuro-platform/src/linux/opener.rs    GioOpener (AppInfo, ShowItems via zbus, kuro-platform only)
crates/kuro-platform/src/linux/paths.rs     LinuxPaths (directories crate)
crates/kuro-platform/src/linux/network.rs   GioNetworkStatus (gio::NetworkMonitor)
src/notify/   mod.rs, actions.rs (AppAction codec), router.rs (GAction app.notice), registry.rs (ids, categories), throttle.rs, mute.rs
src/update/   mod.rs (UpdateService), model.rs, version.rs (semver), asset.rs, notes.rs, check.rs, download.rs, notify.rs
src/export/   mod.rs, library_csv.rs (render, write_csv)
```

Rule from `addenda/requirements-cross-platform.md` (cited by the design): no `cfg(target_os)`, `gio`, `zbus` or `xdg` outside `kuro-platform`. The one exception is `gtk::FileDialog` in the export dialog (T-35a-073).

Cargo features: `kuro-platform` gets `platform-linux` (default), `platform-windows`, `platform-macos`; `kurobayomi` gets `updater` (default off, mirrors `enable-updater`).

### 2.3 What Kurobayomi drops from the module map

- `NotificationChannels` and channel groups (`Notifications.createChannels`, `App.kt:240`): replaced by 14 Notifier categories (section 4.2).
- `FileProvider` (`provider_paths.xml`): plain paths plus `Opener`.
- The `Samsung S Pen` remote-action XML.
- The Android `Service` and `WorkManager` job classes: replaced by tokio tasks.

---

## 3. Reading guide

All links are relative to this folder.

| Document | What it contains |
|---|---|
| [00-overview.md](00-overview.md) | This file: scope, module maps, decisions, dependencies, task summary, definition of done |
| [p1-inventory.md](p1-inventory.md) | One row per scope file with declarations, port decision, target module, priority and task ids; plus external symbols that define the contract |
| [p1-behavior.md](p1-behavior.md) | Exact observed behaviour with `path:line` citations: channels, PendingIntent builders, receiver dispatch, updater algorithm, version rule, download, CSV rendering, XML resources |
| [p1-data-and-config.md](p1-data-and-config.md) | Data models in Rust form, preference keys and defaults, the 41 notification constants and the 14 category set, files and folders on disk, network endpoints, DB tables touched, UI strings |
| [p1-rust-design.md](p1-rust-design.md) | Trait signatures for `kuro-platform`, `AppAction` enum and codec, Linux GIO implementation, action router, threading, update service, CSV module, Android-to-Linux API table, crates, pitfalls |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases (N, U, X, R families), failure table, race conditions, upstream tests to port, new Rust tests, fixtures, manual desktop checklist |
| [p1-ui-parity.md](p1-ui-parity.md) | Desktop notice surfaces, update page layout, About row, export dialog, keyboard and mouse equivalents, responsive behaviour, what the mockup lacks |
| [p1-topic-notifications.md](p1-topic-notifications.md) | End-to-end notification design: lifecycle, action set, shortcut routing, GIO transport, decisions, threading, tests, open questions |
| [p1-topic-app-updater.md](p1-topic-app-updater.md) | Updater state machine, decisions (version rule, throttle, asset choice, install on Linux), failure handling, security notes, test list |
| [p1-topic-library-csv-export.md](p1-topic-library-csv-export.md) | Byte-exact CSV contract: column model, cell and row rules, worked example bytes, Kotlin-to-Rust mapping, atomic write, parity behaviours kept |
| [p1-tasks.md](p1-tasks.md) | 82 tasks `T-35a-001` to `T-35a-098` in the plan format; linted and exported to `../TASKS.csv` |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already made in the plan (with reasons)

| # | Decision | Reason | Source |
|---|---|---|---|
| D1 | Android channels become a Kurobayomi category set with per-category enable flags (`notify.enabled.<category>`) | GNOME and KDE do not expose per-app channels, so the app keeps the mute control | [p1-topic-notifications.md](p1-topic-notifications.md) section 5 |
| D2 | Notice ids are stable strings; the Android integer registry is kept as documentation only; the id-2 collision is removed | GIO ids are strings; two Android notices shared id 2 | [p1-data-and-config.md](p1-data-and-config.md) section 3 |
| D3 | Dismiss is withdraw-by-id only; `cancelAll()` is never called | `cancelAll()` in the Android receiver removes unrelated notices | [p1-behavior.md](p1-behavior.md) 1.5 |
| D4 | Progress is text, throttled to 200 ms and strictly increasing | GIO has no progress bar; floods are avoided | T-35a-008 |
| D5 | Actions carry data (JSON `AppAction`), validated on arrival | the session bus is shared with other processes of the same user | [p1-rust-design.md](p1-rust-design.md) section 6.1 |
| D6 | Clicking the update notice body opens the release page; it does not start a download | Android's body tap started a download | [p1-topic-notifications.md](p1-topic-notifications.md) section 5 item 6 |
| D7 | Version rule uses `semver` ordering (leading `v` stripped, prerelease below release) | Aniyomi's digit-stripping rule offers older releases and throws on short tags (`GetApplicationRelease.kt:48-76`) | [p1-topic-app-updater.md](p1-topic-app-updater.md) section 3.1 |
| D8 | Throttle: 3 days, bypassed by a forced check, set only when a release was found | Avoids GitHub rate limits without hiding a failed platform lookup | [p1-data-and-config.md](p1-data-and-config.md) section 7 |
| D9 | No in-place replacement of an AppImage; package-managed installs do not download | Self-replacement needs permission and signature handling this plan does not design | T-35a-056 (dropped) |
| D10 | SHA-256 is checked when the release notes carry a hash; without one the notice says "(not verified)" | Aniyomi checks nothing beyond the installer | T-35a-054 |
| D11 | CSV is byte-exact: no header, no BOM, CRLF row joins, no trailing newline, fixed column order | Parity with the Kotlin byte contract | [p1-topic-library-csv-export.md](p1-topic-library-csv-export.md) section 5 |
| D12 | Export writes atomically (temp file, `sync_all`, rename) and reports the error | Aniyomi's success callback runs even when the stream is null (`LibraryExporter.kt:55-58`) | T-35a-074 |
| D13 | Export options are not persisted | Parity (`SettingsDataScreen.kt:349-358`, `remember`) | T-35a-077 |

### 4.2 Open questions for the user, with recommendation

These are the questions the area documents leave to the user. None blocks implementation: each has a default in the plan.

| # | Question | Recommendation | Where it is decided |
|---|---|---|---|
| Q1 | Should a notice click start the app when it is closed (a `.desktop` action)? | No. Android notices also need the app process. The app is started by the user. | [p1-topic-notifications.md](p1-topic-notifications.md) section 8 |
| Q2 | Should the update notice still appear while the app window is focused? | Yes (parity). A focused window can ignore it, but the notice is cheap. | [p1-topic-notifications.md](p1-topic-notifications.md) section 8 |
| Q3 | Release channel: stable only, or a preview channel? | Stable only. Preview (commit-count tags) is dropped (T-35a-061). | [p1-topic-app-updater.md](p1-topic-app-updater.md) section 7 |
| Q4 | Where does the update repository come from? | Setting `update.repo` (`owner/name`), empty by default, which disables checks. The Kurobayomi remote is not configured yet. | [p1-topic-app-updater.md](p1-topic-app-updater.md) section 7 |
| Q5 | Check automatically on start? | Yes, with `update.auto_check` (default `true`). Manual check always works. | [p1-topic-app-updater.md](p1-topic-app-updater.md) section 7 |
| Q6 | Default CSV file name | `kurobayomi_library.csv` (Android used `aniyomi_library.csv`). | [p1-topic-library-csv-export.md](p1-topic-library-csv-export.md) section 10 |
| Q7 | Keep the CSV without a header row and without spreadsheet formula escaping? | Keep both (parity). The export help text warns that a title starting with `=` may be evaluated by a spreadsheet. A header row or escaping would change the byte contract. | [p1-topic-library-csv-export.md](p1-topic-library-csv-export.md) section 10 |
| Q8 | Should a release without a SHA-256 be installable? | Yes, with the "(not verified)" text. Make the hash mandatory once Kurobayomi's release pipeline (`area:54-build-release-distribution-docs`) publishes one. | T-35a-054 |
| Q9 | Should the update button be hidden for package-managed installs, or shown disabled? | Hidden (T-35a-055), with the About subtitle "Update with your package manager". | T-35a-055 |
| Q10 | Should a `hide_notification_content` privacy mode apply to notices? | Yes. It replaces title and body with generic text (`notify.hide_content`, default `false`). | [p1-data-and-config.md](p1-data-and-config.md) section 2 |

### 4.3 Open questions this lead adds (not in the area docs)

| # | Question | Recommendation |
|---|---|---|
| Q11 | Should the update check send a request to GitHub at all, for a personal app? | Keep it, default on once `update.repo` is set. It sends one unauthenticated request per 3 days, and nothing is sent while `update.repo` is empty. |
| Q12 | Should the P1 notifier chain be promoted to P0? | Only if download, library-update and backup notices must work from the first daily-use build. Reading and watching do not need them. See section 6.3. |

### 4.4 Inconsistencies found while linting (decide, then fix in the task file)

These are not fixed in this pass because each changes a priority or a dependency, which is the user's call.

1. T-35a-049 (P1, update screen) depends on T-35a-044 (P2, release-notes rewrite). It is the only priority inversion in the area.
2. T-35a-053 (P1, completed-download notice) does not depend on T-35a-054 (P2, SHA-256 before reveal). The topic says a hash must be checked before the file is revealed. Fix: add T-35a-054 to the dependencies of T-35a-053, or promote T-35a-054 to P1.
3. T-35a-049 renders Markdown, but its P1 dependencies do not include the renderer T-35a-062 (P2). Either promote T-35a-062 or have the page use plain text in v1.
4. T-35a-049 hides the download button for package-managed installs (T-35a-055, P2) but does not depend on T-35a-055.
5. Duplicate task ids in the inventory's Table 1 (`T-35a-060`, `T-35a-061`, `T-35a-057`, `T-35a-080`, `T-35a-095`): documentation noise only.
6. No task outside this area depends on any `T-35a-*` task (checked against `../TASKS.csv`). Producers in areas 21, 22, 23, 24, 26, 34, 18, 19, 13, 53 and 01 do not list the category tasks T-35a-080 to T-35a-086 as dependencies. This is a gap in those areas' task files, not here.

---

## 5. Cross-area dependencies

### 5.1 This area needs from others

| Need | Provider | Tasks in this area that need it |
|---|---|---|
| Workspace, `kuro-platform` crate skeleton, `Notifier`/`Opener`/`Paths`/`NetworkStatus` traits set up | `T-req-platform-001`, `T-req-platform-004`, `T-req-platform-007`, `T-req-platform-009` (addenda) | T-35a-001, T-35a-010, T-35a-050, T-35a-055, T-35a-057, T-35a-087 |
| App bootstrap, `gtk::Application`, `app.notice` GAction registration | `area:01-foundation-app-bootstrap`, `area:36-ui-shell-navigation-security-webview` | T-35a-006, T-35a-015, T-35a-049, T-35a-073 |
| Preferences store (keys, defaults, mute flags) | `area:02-preferences-system` | T-35a-004, T-35a-009, T-35a-045, T-35a-046 |
| HTTP client, native roots, reqwest | `area:12-network-stack` | T-35a-031, T-35a-040, T-35a-042, T-35a-050 |
| Favourites queries, favourite flag | `area:05-data-repos-entries-items`, `area:07-domain-entries-items`, `area:08-domain-library-category-history-updates` | T-35a-070, T-35a-016 |
| Downloads: mark read/seen, download, queue control | `area:23-downloads-manga`, `area:24-downloads-anime`, `area:25-downloads-shared` | T-35a-016, T-35a-017, T-35a-018, T-35a-019, T-35a-020 |
| Library update jobs (cancel) | `area:26-library-update-jobs` | T-35a-021, T-35a-080 |
| Backup and restore (cancel, reveal) | `area:21-backup-format-and-create`, `area:22-backup-restore` | T-35a-022, T-35a-024, T-35a-082 |
| Reader and player entry points (open chapter or episode) | `area:43-reader-core`, `area:45-player-core` | T-35a-014 |
| Settings widgets and screens (downloads shortcut, About row, export columns) | `area:47b-settings-widgets-search-more`, `area:47-settings-screens-general` | T-35a-013, T-35a-058, T-35a-072 |
| Atomic write helper and utility module | `area:52-util-app` | T-35a-074 |
| Desktop file, app id, notification service check at startup | `area:53-platform-manifest-intents-workers` | T-35a-011, T-35a-084 |
| Build feature flags and release process (asset names, checksum table) | `area:54-build-release-distribution-docs` | T-35a-046 |
| Markdown renderer theme (Tokyo Night) | `area:49-theme-and-design-system` | T-35a-062 |
| Save-image producer | `area:34-images-covers-cache` | T-35a-023, T-35a-086 |
| Extension installers | `area:18-extensions-manga`, `area:19-extensions-anime` | T-35a-085 |
| Incognito and server status producers | `area:01-foundation-app-bootstrap`, `area:13-core-util-storage-archive-torrent` | T-35a-083, T-35a-084 |
| Search entry (replaces `searchable.xml`) | `area:40-ui-browse-manga`, `area:41-ui-browse-anime` | T-35a-029 |
| S Pen replacement keyboard map | `area:44-reader-viewers`, `area:45-player-core` | T-35a-027, T-35a-028 |

### 5.2 Others need from this area

| Consumer | What it needs | Provided by |
|---|---|---|
| `area:26-library-update-jobs` | Category `LibraryProgress`, `LibraryErrors`, `NewChapters`, ids, and the cancel action | T-35a-080, T-35a-021 |
| `area:23-downloads-manga`, `area:24-downloads-anime` | Category `DownloadProgress`, `DownloadErrors`, ids, and the download and mark-read actions | T-35a-081, T-35a-016 to T-35a-019 |
| `area:21-backup-format-and-create`, `area:22-backup-restore` | Categories `BackupProgress`, `BackupComplete`, ids, reveal and cancel actions | T-35a-082, T-35a-022, T-35a-024 |
| `area:01-foundation-app-bootstrap` | Category `Incognito` | T-35a-083 |
| `area:13-core-util-storage-archive-torrent`, `area:53-platform-manifest-intents-workers` | Categories `ServerStatus` | T-35a-084 |
| `area:18-extensions-manga`, `area:19-extensions-anime` | Category `ExtensionUpdates` | T-35a-085 |
| `area:34-images-covers-cache` | Category `SaveImage`, its own id | T-35a-086, T-35a-023 |
| `area:36-ui-shell-navigation-security-webview` | `app.notice` router, the `AppAction` route, the update page | T-35a-006, T-35a-049 |
| `area:47-settings-screens-general` | Notification categories toggles, About row, export dialog | T-35a-004, T-35a-058, T-35a-072 |
| `area:12-network-stack` | Release DTOs and `fetch_latest` contract | T-35a-040, T-35a-042 |

The producer areas must add `T-35a-080`..`T-35a-086` (and `T-35a-004`) to their own `Depends:` lines. Today none do (see section 4.4 item 6).

---

## 6. Task summary

Counts come from `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, filtered to rows whose file starts with `35-data-misc-updater-notification-export/` in `../TASKS.csv`. Total: 82 tasks.

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 1 | 0 | 0 | 0 | 1 |
| P1 | 35 | 11 | 0 | 0 | 46 |
| P2 | 20 | 2 | 0 | 0 | 22 |
| P3 | 13 | 0 | 0 | 0 | 13 |
| Total | 69 | 13 | 0 | 0 | 82 |

Blocks in [p1-tasks.md](p1-tasks.md): 001-031 notification platform, routing and actions (T-35a-031 is the only P0); 040-065 app updater (including drops 056, 059, 060, 061, 063, 064, 065); 070-077 library CSV export; 080-087 categories and id registry for other areas; 090-098 tests (T-35a-097 is the downloader integration test, T-35a-098 the router test).

Drops (P3, each states the replacement): T-35a-012, T-35a-025, T-35a-026, T-35a-027, T-35a-029, T-35a-030, T-35a-056, T-35a-059, T-35a-060, T-35a-061, T-35a-063, T-35a-064, T-35a-065.

### 6.2 P0 critical path

Only one task in this area is P0:

1. `area:12-network-stack` (HTTP client with native roots) must exist.
2. T-35a-031 (TLS and cleartext configuration, S). Acceptance: the client trusts system CAs and accepts `http://` sources, with no Android XML.

### 6.3 P1 critical path (what makes notices usable)

Longest dependency chain for the notice path, in order:

1. `T-req-platform-004` (addenda) then T-35a-001 (M): `Notifier` trait, `Notice`, `Category`.
2. T-35a-002 (M): `GioNotifier` posts and withdraws through `gtk::Application`. Used by T-35a-007, T-35a-011, T-35a-063.
3. T-35a-003 (S): notice validation (button limit, content rules).
4. T-35a-006 (M): `AppAction` codec and the `app.notice` router. Fans out to T-35a-013 to T-35a-022, T-35a-092, T-35a-098.
5. T-35a-004 (M) then T-35a-005 (S): categories and ids. Fans out to T-35a-080 to T-35a-082 (P1), used by areas 21-26.

Updater path, in order: T-35a-040 (S), T-35a-042 (M, after `area:12-network-stack`), T-35a-047 (M, check orchestration), T-35a-058 (S, About row). Download path: T-35a-050 (M), T-35a-051 and T-35a-052 (S), T-35a-053 (S), T-35a-097 (M, integration test).

Recommendation if notices are needed from the first daily-use build (Q12): promote T-35a-001, T-35a-002, T-35a-003, T-35a-004, T-35a-005, T-35a-006 to P0, and add the category dependencies in the producer areas.

### 6.4 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Desktop notification servers differ (KDE Plasma, GNOME, none). Buttons and body-click routing are not guaranteed. | Actions may not be clickable on some desktops | Every notice has a default action; the toast fallback keeps the message (T-35a-010, T-35a-011); verify on KDE in T-35a-002 |
| `Notifier::show` returns `Unavailable` when no notification server runs | Notices lost silently | In-app toast fallback (`gtk::Overlay`) with the same text |
| Kurobayomi release pipeline does not yet publish assets named `kurobayomi-<version>-<arch>.AppImage` and SHA-256 tables | Updater finds no asset, returns "No build for this platform" | Area 54 owns the names; the check uses the two tiers in T-35a-043 and a test fixture until then |
| No default `update.repo` | Update checks do nothing until the user sets it | Intended: Q4. The About row says so (T-35a-058) |
| Producer areas do not depend on the category tasks | Producers may ship without categories or ids | Section 4.4 item 6; add dependencies in those areas |
| `isBlank()` (Kotlin) and `trim()` (Rust) differ on rare Unicode code points in author or artist fields | CSV bytes differ for those rare values | Accepted in [p1-topic-library-csv-export.md](p1-topic-library-csv-export.md) section 7 |
| Priority inversions (section 4.4 items 1-4) | Ship order may not match the dependency graph | Resolve before scheduling |
| Notice click arrives while a download replaces the job | Race between replace and cancel | Router awaits the old task's join handle (T-35a-050, race 5 in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md)) |

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable on KDE Plasma and on a desktop with no notification server.

1. A notice posted by the app appears in the desktop notification centre with its title and body. Clicking the body opens the thing the notice is about (for example the chapter) and withdraws the notice.
2. Each notice button performs its action: "Mark read" updates the chapter's read flag without changing `last_page_read` (race 1); "Cancel" stops the job; "Dismiss" removes only that notice.
3. With no notification server running, the same notice text appears as an in-app toast, so no message is lost.
4. Progress notices update text as `Downloading 42%`, no faster than every 200 ms, and never decrease.
5. Turning a category off in settings suppresses that category's notices. With "hide notification content" on, titles and bodies are replaced by generic text.
6. The app never calls a bulk withdraw (grep check: no `withdraw_all` or `cancel_all` in `src/`).
7. Update check: with `update.repo` set and a tag newer than the running version, the update notice appears and the update page shows the changelog without the Checksums section. A tag equal or older produces no notice. A second automatic check inside 3 days sends no request; a manual check always sends one.
8. Update download: progress notice runs from 0 to 100 in text. On completion the notice says "Downloaded <file>" with an "Open folder" button that reveals the file. The file is never executed. With a wrong SHA-256 the partial file is deleted and an error notice with Retry appears.
9. On a package-managed install the download button is hidden and the About row says "Update with your package manager".
10. Library export: with all four columns on, the saved CSV is byte-identical to the golden file. Zero favourites give an empty file. A failed write shows an error toast and leaves the previous file untouched.
11. `cargo test` passes for `src/notify`, `src/update`, `src/export` and `kuro-platform`, including the tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) sections 5.1 to 5.8.
12. The Samsung S Pen mapping and the Android FileProvider are absent from the codebase. The keyboard equivalents (N, P, Back, arrow keys) are wired in the reader and player.
