# Area 53 — Platform: manifest, intents and deep links, shortcuts, receivers, services and workers, res layouts, menus and animations

Area lead overview. Source root for every path below: the read-only Aniyomi clone at `/home/blackbox/aniyomi-src` (repo-relative paths).

## 1. Purpose and scope

**What Aniyomi does here (observed).** Everything the Android system sees about the app and everything the app sends back to the system:

- Declarations in `app/src/main/AndroidManifest.xml` (324 LOC): 14 `uses-permission`, 11 `activity`, 6 `service`, 1 `receiver`, 2 `provider`, plus the application attributes.
- The launch router in `MainActivity` (`handleIntentAction`, `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:461` to `:573`) that turns intents, shortcuts, search, shared text, `.tachibk` opens, `aniyomi://` and `tachiyomi://` links and tracker OAuth returns into screens.
- Two deep-link trampolines and their screen models (`ui/deeplink/anime`, `ui/deeplink/manga`) that resolve a search or shared text to an entry.
- The notification layer: `NotificationReceiver.kt` (999 LOC, the largest file in scope), `NotificationHandler.kt`, `Notifications.kt` (13 channels, fixed integer ids) and builders for every notice button.
- Background hosts: the app-update download job, the HTTP server for external players, the torrent server, the WorkManager foreground host and the Shizuku provider.
- Activities that carry reader, player, web view, unlock and extension install flows.
- Layouts, menus, animations, raw resources and XML configs under `app/src/main/res/`, plus the presentation-widget receivers and the module manifests.

**Why it matters for a usable Kurobayomi.** A desktop app that cannot open a `.tachibk` backup, cannot follow a `kurobayomi://` or `aniyomi://` tracker return, cannot show a clickable "new chapter" notice and cannot keep a download or server alive with no window is not usable daily. Several P0 items sit here: the single GTK window (T-53a-001), command-line and open handling (T-53a-006), single instance forwarding (T-53a-007), the reader root (T-53a-057) and the player root (T-53a-055). Everything else in this area is parity (P1), polish (P2) or a recorded drop with the replacement named (P3).

**Scope as counted.** The scope command lists 56 files (`scope-list.txt`): 10 Kotlin files (1664 LOC), 46 XML files. 14 of the 56 are dead resources with zero references (`p1-inventory.md` section 3).

## 2. Module and crate maps

### 2.1 Kotlin module map (observed)

| Kotlin package or file | Responsibility | Linux fate |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt` | launcher activity, intent router `handleIntentAction`, shortcut and search and OAuth branches | replaced by GTK application and `launch` module (T-53a-001, T-53a-006) |
| `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt` | broadcast receiver with the action switch (`:64` to `:184`), dismissal (`:191` to `:194`), builders, job starts | replaced by `app.notice` GAction dispatcher (T-53a-017 to T-53a-019) |
| `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationHandler.kt` | `PendingIntent` builders for open image, error log, URL, install | replaced by `Opener` and notice builders (T-53a-042) |
| `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt` | 13 channel ids, notification integer ids, channel creation | replaced by notice categories and a stable id registry (T-53a-020, T-53a-021) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/{anime,manga}/` | trampoline activity, screen, screen model that resolves a query | trampolines dropped (T-53a-046); resolvers move to `kuro-core::deeplink` (T-53a-048, T-53a-050) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/DeepLinkScreenType.kt` | enum of the two deep-link kinds | folded into `Kind` (area:36) |
| `app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt` | local HTTP server for external players | background service with notice (T-53a-035) |
| `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt` | torrent server, foreground notice with "Stop" | background service with notice and Stop button (T-53a-035) |
| `app/src/main/java/eu/kanade/tachiyomi/extension/{manga,anime}/util/*InstallService.kt` and `*InstallActivity.kt` | APK extension install | dropped; area:20 mechanism replaces (T-53a-036) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt` | biometric app lock | owned by area:36 (T-53a-070) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt` | web view with shared-axis transition | owned by area:36 (T-53a-071) |
| `app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt` | `toShareIntent`, share chooser | replaced by "Show in folder" and "Copy link" (T-53a-040) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt`, `ui/player/PlayerActivity.kt` | reader and player activities (singleTask, PiP, S Pen) | ported as windows (T-53a-015), PiP dropped (T-53a-033), S Pen dropped (T-53a-065) |
| `presentation-widget/` receivers (six) | home and lock screen widgets | dropped (T-53a-068) |

### 2.2 Proposed Rust crate and module map (Proposal)

Defined in `p1-rust-design.md` section 1. Workspace and crate names are owned by area:01.

```
kuro-core/                 no GTK, no OS code
  launch/   mod.rs (LaunchRequest, Kind, ShortcutAction, NavAction, RouteError)  area:36
            uri.rs   parse_uri(&Url)          this area
            argv.rs  parse_argv(&[OsString])  this area
  notify/   actions.rs (AppAction + JSON codec)  ids.rs (NoticeId registry)
            category.rs (switches)  payload.rs (validation: size, schemes, ids, paths)
  deeplink/ anime.rs (resolve_anime)  manga.rs (resolve_manga)
kuro-platform/             all OS code
  traits.rs  Notifier, Opener, SingleInstance, NetworkStatus, Paths   (area:01)
  linux/     desktop_entry.rs  mime.rs  network.rs (NetworkManager, GIO)  search_provider.rs (P2)
             notifier.rs  opener.rs  portal folder chooser
src/                       GTK binary
  main.rs (GApplication wiring)  launch.rs  nav.rs  notice.rs (app.notice action)
```

Threading rule (Proposal, `p1-rust-design.md` section 5): GTK main thread dispatches only; all database, network and file work runs on tokio and returns through a glib channel.

## 3. Reading guide

Relative links to every document in this folder.

| Document | What it contains |
|---|---|
| [p1-behavior.md](p1-behavior.md) | Observed behaviour of every component: start-up, `handleIntentAction`, shortcuts, OAuth, deep-link resolution, notification action table, ids and withdrawal, channels, app-update job, extension install drop, services and workers, activities, share and FileProvider, permissions and network config |
| [p1-data-and-config.md](p1-data-and-config.md) | Data tables: manifest components, permissions, receiver action strings, channels and ids, launch route table, shortcut constants, files, endpoints, preference keys and defaults, DB effects, resource identifiers, line counts |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Kotlin defects (`E-nn`), race conditions, edge cases by input (`EC-nn`, `EN-nn`, `R-nn`), fixtures, test list (`TC-53-nn`), visible failure modes, coverage checklist |
| [p1-inventory.md](p1-inventory.md) | One row per scope file with its decision, component table, counts and the scope-coverage self-check |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, Rust types (`LaunchRequest` parsing, `AppAction`, `NoticeId`, traits), platform implementations, route application, threading, GApplication wiring, Android-to-Linux mapping, error handling, test hooks, pitfalls |
| [p1-tasks.md](p1-tasks.md) | The 77 implementation tasks `T-53a-001` to `T-53a-078` (there is no `T-53a-016`; IDs are not renumbered) in the repository task format (lint: ok) |
| [p1-topic-launch-intents-and-deeplinks.md](p1-topic-launch-intents-and-deeplinks.md) | Inbound surface on Android and Linux entry points, argument grammar, scheme and host table, OAuth and scheme-ownership open questions, security notes |
| [p1-topic-manifest-components-and-permissions.md](p1-topic-manifest-components-and-permissions.md) | Decision ledger for every declaration, resource usage, what the user loses, permissions model on Linux |
| [p1-topic-notification-receiver-actions.md](p1-topic-notification-receiver-actions.md) | One button press end to end on Android and Linux, why the Android design is defective, mechanism mapping, JSON payload examples, dismissal semantics, concurrency, open items |
| [p1-ui-parity.md](p1-ui-parity.md) | What the user sees for the scope screens: deep-link loading page, notification surfaces, transitions, desktop actions, download row, reader and player roots, keyboard equivalents for S Pen, error view, state matrix, responsive rules, mockup gaps |
| [scope-list.txt](scope-list.txt) | The 56 scope paths with their line counts, as produced by the scope command |

## 4. Key design decisions and open questions

### 4.1 Decisions (with the recommendation already in the plan)

| # | Decision | Recommendation and reason | Tasks |
|---|---|---|---|
| D1 | One GTK window instead of per-activity windows | Keep. The Android `isTaskRoot` check has no equivalent; a second process forwards its request to the first (GApplication) | T-53a-001, T-53a-007 |
| D2 | Reader and player are separate windows, one of each at a time | Keep, matching `singleTask` in the manifest | T-53a-015 |
| D3 | Notifications carry their action as a GIO button target (`app.notice` with a JSON `AppAction`) | Keep. Removes Android request-code collisions (E-05) and the `cancelAll()` side effect (E-01) | T-53a-017, T-53a-018, T-53a-019 |
| D4 | Dismissal withdraws one id only; no bulk withdraw in the `Notifier` trait | Keep (test TC-53-07) | T-53a-019 |
| D5 | 13 Android channels become categories with in-app switches | Keep; the desktop server has no per-channel mute UI that users rely on | T-53a-021 |
| D6 | Picture-in-picture dropped; compact player mode optional | Drop PiP now, compact mode as P3 | T-53a-033 |
| D7 | APK extension install dropped; extensions follow area:20 | Keep. Shizuku and installer permissions go with it | T-53a-036, T-53a-038 |
| D8 | Shared text and Google search route to global search, not to URL resolution | Keep as text search. Direct URL resolution is P2 | T-53a-005 |
| D9 | Share chooser replaced by "Show in folder" and "Copy link" | Keep | T-53a-040 |
| D10 | Folder chooser through the desktop portal; no all-files access | Keep (T-53a-027) | T-53a-027 |
| D11 | Long-tail server and background hosts show a persistent notice with a Stop button | Keep | T-53a-035 |
| D12 | Home and lock screen widgets dropped; the Updates tab replaces them | Keep | T-53a-068 |
| D13 | Samsung S Pen gestures dropped; keyboard keys replace them | Keep | T-53a-065 |

### 4.2 Open questions that need the user's decision

1. **Tracker OAuth redirect URIs (blocks the final OAuth return).** Each tracker accepts only the redirect URIs registered for its client id. Aniyomi's client ids are registered for `aniyomi://<provider>-auth`. A `kurobayomi://` redirect needs new client registrations at AniList, MyAnimeList, Bangumi, Shikimori and Simkl.
   **Recommendation:** keep the `aniyomi://*-auth` alias working until the new registrations exist, then switch. Parsing is the same either way (`p1-topic-launch-intents-and-deeplinks.md` section 6, item 1; T-53a-009).
2. **Scheme ownership.** **Recommendation:** Kurobayomi registers `kurobayomi`, `aniyomi` and `tachiyomi`; a settings button makes Kurobayomi the default handler through `xdg-mime default` after confirmation (T-53a-002, area:36).
3. **Removing the legacy aliases.** **Recommendation:** keep `aniyomi://` and `tachiyomi://` for the first release (about three lines of parsing), then drop them with a release note.
4. **App lock method.** Biometric unlock has no direct desktop equivalent. **Options:** passphrase, or a PAM/polkit prompt. **Recommendation:** passphrase first; PAM/polkit only if the user asks. The decision belongs to area:36 (T-53a-070).
5. **Compact player mode.** **Recommendation:** P3 only; it is not needed for parity.
6. **Desktop search providers (GNOME SearchProvider2, KRunner).** **Recommendation:** P2. Ship text search (`--search`) first.
7. **Notification grouping on the desktop.** The Android group-summary behaviour (E-06) was not verified on a device, and whether the GNOME list groups by app id is not verified. **Recommendation:** the design does not rely on grouping; multiple new entries are one notice that lists up to three titles and "and N more" (text owned by area:35).
8. **Open-in-browser loop guard.** **Recommendation:** filter `kurobayomi://`, `aniyomi://` and `tachiyomi://` before calling `Opener::open_url` (`p1-topic-launch-intents-and-deeplinks.md` section 6, item 4).

## 5. Cross-area dependencies

Counted from the `Depends` lines and prose references in `p1-tasks.md` and the topic documents.

**This area needs from others:**

| Area | What it provides | Used by |
|---|---|---|
| area:01-foundation-app-bootstrap | Cargo workspace, `kuro-platform` traits (`Notifier`, `Opener`, `SingleInstance`, `NetworkStatus`, `Paths`), crash handling for `CrashActivity` | T-53a-001, T-53a-069 |
| area:36-ui-shell-navigation-security-webview | `LaunchRequest`, `route_request`, `ShortcutAction`, GTK shell and navigation, app lock, web view host | T-53a-001, T-53a-006, T-53a-015, T-53a-070, T-53a-071 |
| area:35-data-misc-updater-notification-export | `AppAction` enum, notice text and categories, app-update flow, notice strings | T-53a-017 to T-53a-023, T-53a-042, T-53a-044 |
| area:20-extension-strategy-redesign | Extension install replacement, extension repo and store routes | T-53a-036, T-53a-002 (`add-repo`, `extension-store`) |
| area:27-tracking-core | OAuth token storage and the redirect handling behind `*-auth` routes | T-53a-009 |
| area:22-backup-restore | `.tachibk` restore screen, which parses the file only after confirmation | T-53a-010 |
| area:23-downloads-manga, area:24-downloads-anime | Download actions from notices, removal after read, download row and menu behaviour | T-53a-043, T-53a-053, T-53a-060 |
| area:26-library-update-jobs | Library update job whose progress and finish drive notices | T-53a-008, T-53a-023 |
| area:43-reader-core, area:44-reader-viewers | Reader window root, viewer, error panel | T-53a-057, T-53a-058 |
| area:45-player-core, area:46-player-controls-ui | Player window root, mpv GL area and controls overlay | T-53a-055 |
| area:13-core-util-storage-archive-torrent | Torrent server behaviour behind T-53a-035 | T-53a-035 |
| area:12-network-stack | Network status and TLS policy for reqwest, replaces `network_security_config` | T-53a-024, T-53a-063 |
| area:21-backup-format-and-create | `application/x-protobuf+gzip` backup metadata, `.tachibk` MIME association | T-53a-003 |
| area:49-theme-and-design-system, area:50-android-widgets, area:51-i18n | Theme for transitions and rows, widget replacement, string keys with English values and the language setting | T-53a-031, T-53a-068, T-53a-012 |
| area:02-preferences-system | Preference keys and defaults for notification switches and link handling | T-53a-021, T-53a-025 |
| area:15-source-api-manga, area:16-source-api-anime, area:18-extensions-manga, area:19-extensions-anime | Source search routes and the extension search kind fix | T-53a-005 |
| area:34-images-covers-cache | Open image notice action | T-53a-042 |
| area:42-ui-updates-history-stats-storage-upcoming | Updates tab that replaces the widgets | T-53a-068 |

**Other areas need from this one:**

- area:36 needs the launch argument grammar, `parse_uri` and `parse_argv` outputs, and the notice dispatcher entry point.
- area:35 needs the `Notifier` and `Opener` trait behaviour, the withdraw-only-one-id rule and the stable `NoticeId` registry.
- area:20 needs the `kurobayomi://add-repo` and `extension-store` routes.
- area:27 needs the `*-auth` route parsing and the fragment handling for AniList (T-53a-009).
- area:22 and area:21 need the `application/x-tachibk` MIME type and `connect_open` file handling.
- area:43, area:44, area:45 and area:46 need the window roots (T-53a-055, T-53a-057) and the external-player routing (defect E-09).

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export` then the rows of `TASKS.csv` whose file starts with `53-platform-manifest-intents-workers/`. Lint of `p1-tasks.md` prints `ok`. The repository-wide export reports 0 errors.

**Counts (77 tasks):**

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 1 | 4 | 0 | 0 | 5 |
| P1 | 29 | 14 | 0 | 0 | 43 |
| P2 | 11 | 1 | 0 | 0 | 12 |
| P3 | 17 | 0 | 0 | 0 | 17 |
| **Total** | **58** | **19** | **0** | **0** | **77** |

**P0 critical path, in order:**

1. T-53a-001 (M): GTK application entry point, single window. Depends on area:01 and area:36.
2. T-53a-006 (M): command-line and open handling in GApplication. Depends on T-53a-001 and area:36.
3. T-53a-007 (S): single instance; a second launch is forwarded. Depends on T-53a-006.
4. T-53a-057 (M): reader root layout, depends on area:43 and area:44.
5. T-53a-055 (M): player root, mpv GL area with controls overlay, depends on area:45 and area:46. Can run in parallel with step 4 once the window shell exists.

Steps 4 and 5 depend on the reader and player engines (area:43 to area:46), so their real start is gated by those areas.

**Risks:**

- **Tracker OAuth registrations.** A `kurobayomi://` redirect will not work until new client registrations exist. The alias keeps working meanwhile (open question 1).
- **Desktop behaviour not verified.** Notice grouping in GNOME Shell, and the default-handler behaviour of `xdg-mime`, have not been checked on the target desktop (KDE Plasma, per the user's setup). Design avoids depending on grouping.
- **Notification actions from other local processes.** A `GApplication` command line and a `kurobayomi://` link can come from any local process or web page. Mitigation is payload validation (T-53a-018) and the rule that links cannot change settings by themselves.
- **Feature loss the user will notice.** PiP, widgets, the share chooser, S Pen gestures, biometric lock and APK extension install. All are recorded with a replacement; see `p1-topic-manifest-components-and-permissions.md` section 3.
- **Defects not to carry over.** 33 Kotlin defect rows (E-nn) in `p1-edge-cases-and-tests.md` section 1 each have a Linux behaviour and a test.
- **Sandbox permissions.** The packaging audit (T-53a-013) must keep the sandbox to the network, notification and file-chooser portals with no `filesystem=host`. A wider sandbox would break the portal-only design.

## 7. Definition of done for this area

Observable behaviours in Kurobayomi, each covered by a task above:

1. Launching `kurobayomi` with no arguments opens one window on the Library tab (T-53a-001). A second launch forwards its request to that window and creates no second window (T-53a-007).
2. `kurobayomi --action=show-updates`, and each of the other desktop actions, opens the named tab. The five desktop actions appear in the launcher's long-press menu with localised labels (T-53a-011, T-53a-012).
3. Double-clicking a `.tachibk` file, or running `kurobayomi backup.tachibk`, opens the restore screen and does not restore anything until the user confirms (T-53a-010, area:22).
4. `kurobayomi://add-repo?url=…`, `aniyomi://add-repo?url=…` and `tachiyomi://add-repo?url=…` each open the matching extension repository page. A link whose scheme is not handled shows a toast (T-53a-004, T-53a-045).
5. A tracker redirect with a token stores the token and returns to Settings. A missing code takes the logout branch (T-53a-009).
6. A text search from the command line or from shared text opens global search with that text (T-53a-005).
7. A desktop notice for new chapters shows its buttons. Pressing "Mark as read" marks the chapters read in one transaction and withdraws only that notice. A failure shows an error toast and keeps the notice (T-53a-017, T-53a-018, T-53a-043).
8. Dismissing one notice never removes another notice, including a running progress notice (T-53a-019, test TC-53-07).
9. Notice ids are stable across restarts, and a manga and an anime with the same numeric id produce different notice ids (T-53a-020, defect E-31).
10. The app-update notice shows progress with no more than one update per 200 ms and never a decreasing percent (T-53a-044).
11. Download and update gates read one network status. With Wi-Fi-only enabled and an unknown network state, Wi-Fi-only downloads wait and a message is shown (T-53a-024, T-53a-026, defect E-15).
12. The HTTP server for external players and the torrent server run with a persistent notice that has a Stop button. Stopping the server withdraws its notice (T-53a-035).
13. The app keeps running with no window while a background job is active, and exits when no job or hold remains (T-53a-008, T-53a-037).
14. The reader and the player each open in one window at a time, and a second request for the same kind reuses it (T-53a-015).
15. Shared-axis page transitions appear between library, detail and reader pages (T-53a-051).
16. Every Aniyomi capability that this area owns is either implemented or listed as a P3 drop with the replacement and the user's loss, and the lint of `p1-tasks.md` prints `ok`.
