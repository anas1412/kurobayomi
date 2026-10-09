# 52-util-app — App utilities: overview

Area lead output. Scope: `eu.kanade.core.*` and `eu.kanade.tachiyomi.util.*` in the Aniyomi source (repo-relative paths under `app/src/main/java/`, `core/common/...`; read-only clone at `/home/blackbox/aniyomi-src`). 40 files, all listed in [p1-inventory.md](p1-inventory.md).

## 1. Purpose and scope

What Aniyomi does here: a set of small, mostly pure helpers and thin Android wrappers that the rest of the app calls. They cover:

- Preference binding to UI state (`PreferenceMutableState`, `CheckboxState`).
- Collection helpers (`insertSeparators`, `addOrRemove`, `fastFilterNot`, `fastPartition`, `fastCountNot`).
- Source-readiness gates (`ifMangaSourcesLoaded`, `ifAnimeSourcesLoaded`).
- Chapter and episode selection: next unread, next unseen, dedupe by chapter number, downloaded-only filter, sort comparators.
- Cover, background and thumbnail invalidation, edit and removal.
- Airing-schedule lookup against AniList, MyAnimeList (via AniList id) and Simkl, with packed viewer flags.
- Crash log dump, debug info block, problematic-extensions report.
- PKCE verifier for MyAnimeList login.
- Byte-size text, recursive directory size, stream save, cache-file creation.
- Relative and absolute date and time text, zone conversion, language display names, default languages.
- Tap-zone inversion for the reader, animation scale, tablet and compact layout breakpoints, reader background choice.
- App lock (PAM replacement), build-variant flags, clipboard, open-in-browser, share/save, network state, notifications, job registry, popup menus, visibility check, screenshot protection.

Why it matters for a usable Kurobayomi:

- Reading and watching depend on selection logic (`getNextUnread`, `getNextUnseen`, dedupe, downloaded filter) and on tap-zone inversion. A wrong "next chapter" breaks the core loop.
- Library and entry screens depend on covers, sizes, relative dates and locale names.
- Background work (library updates, downloads, backups) depends on the notification core and the job registry.
- Most of the Android-specific code here is dropped or replaced. The plan states each replacement so nothing is silently lost.

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Package (under `app/src/main/java/eu/kanade/`) | Files | Responsibility |
|---|---|---|
| `core/preference` | `CheckboxState.kt`, `PreferenceMutableState.kt` | Preference-to-Compose state bridge; tri-state checkbox mapping. |
| `core/util` | `AnimeSourceUtil.kt`, `MangaSourceUtil.kt`, `CollectionUtils.kt`, `RxJavaExtensions.kt` | Source-loaded gates; list helpers; RxJava-to-Flow adapter (dropped). |
| `tachiyomi/util` | `AniChartApi.kt`, `AnimeExtensions.kt`, `CrashLogUtil.kt`, `MangaExtensions.kt`, `PkceUtil.kt`, `StorageUtil.kt` | Airing schedule; cover and thumbnail edits; crash and debug text; PKCE; byte sizes and directory size. |
| `tachiyomi/util/chapter` | `ChapterFilterDownloaded.kt`, `ChapterGetNextUnread.kt`, `ChapterRemoveDuplicates.kt` | Chapter selection and dedupe. |
| `tachiyomi/util/episode` | `EpisodeFilterDownloaded.kt`, `EpsiodeGetNextUnseen.kt` | Episode selection (file name is misspelled upstream; keep the name in references). |
| `tachiyomi/util/lang` | `CloseableExtensions.kt`, `DateExtensions.kt`, `RectFExtensions.kt` | Closeable array use (dropped); date text; tap-zone inversion. |
| `tachiyomi/util/storage` | `OkioExtensions.kt` | Stream save. |
| `tachiyomi/util/system` | 16 files: `AnimationExtensions.kt`, `AuthenticatorUtil.kt`, `BuildConfig.kt`, `ChildFirstPathClassLoader.kt`, `ContextExtensions.kt`, `DeviceUtilExtensions.kt`, `DisplayExtensions.kt`, `DrawableExtensions.kt`, `IntentExtensions.kt`, `InternalResourceHelper.kt`, `LocaleHelper.kt`, `NetworkExtensions.kt`, `NetworkStateTracker.kt`, `NotificationExtensions.kt`, `TvUtils.kt`, `WorkManagerExtensions.kt` | Android system wrappers: animation, app lock, build flags, clipboard, display, drawables, intents, locale, network, notifications, TV detection, WorkManager. |
| `tachiyomi/util/view` | `EditTextPreferenceExtensions.kt`, `ViewExtensions.kt`, `WindowExtensions.kt` | Compose host, popup menu, edit-text preference listener, `FLAG_SECURE`. |

File count check: 6 files in `core/` (2 preference, 4 util), 6 in `tachiyomi/util` top level, 3 in `chapter`, 2 in `episode`, 3 in `lang`, 1 in `storage`, 16 in `system`, 3 in `view`. Total 40.

### 2.2 Proposed Rust crate and module map

Source of truth: [p1-rust-design.md](p1-rust-design.md) section 1. Summary:

- `kuro-core` (pure logic, no gtk, gio or glib dependency):
  - `util/size.rs` (`format_bytes`), `util/collections.rs` (`insert_separators`, `insert_separators_reversed`), `util/text.rs` (`truncate_center`).
  - `selection.rs` (next unread, next unseen, dedupe, downloaded filter), `sort.rs` (chapter and episode comparators).
  - `covers.rs`, `airing.rs`, `pkce.rs`, `crashlog.rs`, `reader_theme.rs`, `layout.rs`, `build_info.rs`, `geometry.rs`, `locale.rs`.
  - `time/relative.rs` (relative decisions), `time/format.rs` (absolute formats).
- `kuro-platform` (gio, ashpd, zbus, PAM wrappers, no widgets):
  - `files.rs` (save stream, directory size, cache file), `open.rs` (open URI, reveal file, save dialog), `notify.rs` (notifications over `gio::Notification`), `net.rs` (network state), `appearance.rs` (colour scheme, animation scale), `auth.rs` (PAM), `jobs.rs` (job registry), `paths.rs` (XDG directories).
- `kuro-i18n` (message table and plural rendering, shared by `kuro-core::locale` and `time::relative`).
- `kurobayomi` (existing GTK binary crate): `ui/toast.rs`, `ui/checkbox.rs`, `ui/popover.rs`, `ui/bind.rs`, `ui/layout.rs`.

Dependency direction: `kurobayomi` depends on `kuro-platform`, which depends on `kuro-core`. `kuro-core` never depends on `gtk`, `gio` or `glib`.

Threading model (from [p1-rust-design.md](p1-rust-design.md) section 3): GTK main loop on the main thread; one multi-thread tokio runtime on a separate thread; `async_channel` to return results to the UI; `spawn_blocking` for directory size and PAM.

## 3. Reading guide

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | The 40-row table: file, function signatures, Port / Rewrite / Replace-with-crate / Drop decision and the task id for each. Start here. |
| [p1-behavior.md](p1-behavior.md) | Observed behaviour per function (`OBSERVED` with path:line), exact algorithms, constants, message keys, and the Rust `Proposal:` for each. The main specification. |
| [p1-data-and-config.md](p1-data-and-config.md) | Domain models read by this area, enum stored values, bit-flag layouts (manga `chapterFlags`, anime `episodeFlags`, anime `viewerFlags`), preference keys, files and paths, network endpoints, database columns touched. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases per module, race conditions, failure-mode matrix, list of new tests, and detailed test vectors (`TC-…` ids, about 160). |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, key Rust types and signatures, async model, error handling, integration with the existing mockup, Android-to-Linux API map, pitfalls. |
| [p1-tasks.md](p1-tasks.md) | 66 implementation tasks `T-52a-001` to `T-52a-066` in the task format. Lint: ok. |
| [p1-ui-parity.md](p1-ui-parity.md) | Only the functions that render UI or trigger it (toasts, clipboard, share, lock screen, notifications, checkbox, layout, empty states, mockup gaps, keyboard and mouse summary). |
| [p1-topic-airing-and-simkl.md](p1-topic-airing-and-simkl.md) | Airing decision flow, AniList and Simkl wire formats, the three upstream parsing bugs, Rust algorithm, stored result encoding, edge-case table. |
| [p1-topic-selection-and-sort.md](p1-topic-selection-and-sort.md) | Reader pipeline for chapter and episode selection, reading-direction rule, TriState semantics, comparators, dedupe, downloaded filter, quirks to keep, Rust API. |
| [p1-topic-time-and-locale.md](p1-topic-time-and-locale.md) | Relative date and time decision model, absolute formats, zones, locale display names, collation, default enabled languages, Rust module. |

## 4. Key design decisions and open questions

Each item needs the user's call. The recommendation is the planned default unless the user says otherwise.

1. **App lock (PAM).** Upstream locks the app with the device credential prompt (`AuthenticatorUtil`). Linux has no equivalent prompt in the app. Options: (a) PAM authentication of the current user (T-52a-033), (b) drop the lock and point to the desktop lock, (c) keep the preference but make it a no-op. Recommendation: (a) as opt-in, P2, default off as upstream does. Risk: PAM needs the app to read the authentication stack; a weak lock would be a security regression. Document this in settings.
2. **Screenshot protection (`FLAG_SECURE`).** No stable GTK 4.16, Wayland or X11 mechanism blocks capture of one window. Recommendation: drop with a visible note (T-52a-062). Keep the preference so existing settings do not break.
3. **In-app updater.** `BuildConfig.UPDATER_ENABLED` is `false` in Kurobayomi (see area:35-data-misc-updater-notification-export). Recommendation: drop. Distribution is by the user's package manager or build. Confirm the user does not want an update check.
4. **Simkl 404 fallback.** Upstream returns `(1, 0)` on a 404 from the Simkl calendar. The topic file proposes trying the next calendar type first. This is better than upstream and changes nothing visible when the time is 0 either way. Recommendation: keep it and flag it in the changelog.
5. **Airing with no tracker linked.** Upstream returns `(1, 0)` and overwrites the stored value. Kept as observed, flagged in T-52a-013. Recommendation: keep unless the user wants stored airing data preserved.
6. **Time and locale libraries.** Absolute formats use ICU (`icu_datetime`); names and plurals use ICU data (`icu_displaynames`, `icu_plurals`). Recommendation: accept the ICU data size (a few MB) for correct display names and collation. Confirm that the user is fine with the binary size.
7. **Localisation scope.** The message keys are `MR.strings.*` and `AYMR.strings.*` from the base XML. Recommendation: ship English strings first (the literal strings in the spec are kept verbatim), and generate `kuro-i18n` from the base XML only when a second language is requested.
8. **Extension class loader and RxJava.** Both dropped. The extension design is in area:20-extension-strategy-redesign. No user-visible loss. Confirm the user agrees with Linux extensions not being DEX.
9. **Notification and job model.** Notifications go through `gio::Notification` (no in-app toast fallback). Recommendation: confirm the user accepts that channel grouping is shown by the desktop, not by the app.

## 5. Cross-area dependencies

### 5.1 This area needs from others

| Need | From | Why |
|---|---|---|
| Crate skeleton, tokio and glib wiring | area:01-foundation-app-bootstrap | `kuro-platform` skeleton (T-52a-066) and the Android-to-Linux API map. |
| Preference store and key registry | area:02-preferences-system | `Pref<T>` bridge (T-52a-001), defaults (T-52a-065), locale and layout preferences. |
| Domain models for chapter, episode, manga, anime, tracks | area:07-domain-entries-items, area:08-domain-library-category-history-updates | Selection, dedupe, sort and cover functions take these types. |
| Download-state lookup | area:23-downloads-manga, area:24-downloads-anime | Downloaded-only filters (T-52a-010). |
| Network client and rate policy | area:12-network-stack | AniList, Simkl, MAL calls (T-52a-013, T-52a-014, T-52a-015) and network state (T-52a-051). |
| Source list readiness | area:17-source-app-layer | Source-loaded gate (T-52a-005) and locale source group names (T-52a-028). |
| Extension managers | area:18-extensions-manga, area:19-extensions-anime | Problematic-extensions report (T-52a-019). |
| Cover cache files | area:34-images-covers-cache | Cover invalidation and edit (T-52a-011, T-52a-012, T-52a-047). |
| Repository writes for entries | area:05-data-repos-entries-items | Writing covers and airing values (T-52a-012, T-52a-016). |
| Tracker logins | area:28-tracker-anilist, area:29-tracker-myanimelist, area:32-tracker-mangaupdates-simkl, area:27-tracking-core | PKCE (T-52a-020), airing lookups (T-52a-014, T-52a-015), date helpers (T-52a-027). |
| Background jobs and backup | area:21-backup-format-and-create, area:22-backup-restore, area:26-library-update-jobs | Job registry (T-52a-056), share and save (T-52a-048). |
| Notifications and updater | area:35-data-misc-updater-notification-export | Notification core (T-52a-053) and crash log dump (T-52a-017). |
| Shell, navigation, security, webview | area:36-ui-shell-navigation-security-webview | Clipboard (T-52a-036), open in browser (T-52a-039), layout (T-52a-044), night mode (T-52a-045), app lock (T-52a-033), screenshot flag (T-52a-062). |
| Reader and player | area:43-reader-core, area:44-reader-viewers, area:45-player-core | Tap-zone inversion (T-52a-031), reader background (T-52a-041), animation scale (T-52a-032), visibility check (T-52a-060), keep-awake (T-52a-038). |
| Settings and archive helpers | area:13-core-util-storage-archive-torrent, area:47-settings-screens-general | Directory size (T-52a-022), stream save (T-52a-023), file URI size (T-52a-042), editable-text preference listener (T-52a-058). |
| Layout consumers | area:37-ui-library, area:38-ui-entry-details-manga, area:42-ui-updates-history-stats-storage-upcoming | Relative and byte-size text (T-52a-021, T-52a-025, T-52a-026), checkbox state (T-52a-002). |

### 5.2 Other areas need from this area

- area:12-network-stack needs the network state snapshot and watch (`kuro-platform::net`, T-52a-051, T-52a-052).
- area:13-core-util-storage-archive-torrent needs `save_stream`, `dir_size`, `cache_file` (T-52a-022, T-52a-023, T-52a-024).
- area:21-backup-format-and-create and area:22-backup-restore need the job registry and stream save (T-52a-056, T-52a-023).
- area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared need the job registry, network state and popup menu (T-52a-056, T-52a-052, T-52a-059).
- area:26-library-update-jobs and area:35-data-misc-updater-notification-export need notifications and channel priorities (T-52a-053, T-52a-054).
- area:34-images-covers-cache needs the drawable to texture conversion (T-52a-047) and cover invalidation (T-52a-011).
- area:37-ui-library, area:38-ui-entry-details-manga, area:42-ui-updates-history-stats-storage-upcoming need the size formatter, relative dates, locale names and checkbox tri-state.
- area:43-reader-core, area:44-reader-viewers, area:45-player-core need selection and sort functions (`kuro-core::selection`, `kuro-core::sort`), tap-zone inversion, animation scale and keep-awake.
- area:36-ui-shell-navigation-security-webview needs clipboard, open-URI, share, layout, night mode, app lock.

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then rows with `file` starting with `52-util-app/` in `TASKS.csv` (66 rows).

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 9 | 6 | 0 | 0 | 15 |
| P1 | 22 | 10 | 0 | 0 | 32 |
| P2 | 4 | 1 | 0 | 0 | 5 |
| P3 | 14 | 0 | 0 | 0 | 14 |
| **Total** | **49** | **17** | **0** | **0** | **66** |

All 14 P3 tasks have effort S and are either drops with a stated replacement or Android-only code (RxJava adapter, closeable arrays, class loader, runtime permissions, default browser resolver, cutout checks, intent extras getters, system resource booleans, TV detection, foreground promotion, EditText accessor, Compose host, dynamic colour).

### 6.2 P0 critical path (in order)

1. T-52a-066 kuro-platform skeleton and Android-to-Linux map (depends area:01-foundation-app-bootstrap).
2. T-52a-065 preference keys and defaults registry (depends area:02-preferences-system).
3. T-52a-001 `Pref<T>` to GTK state bridge (depends T-52a-066, area:02-preferences-system).
4. T-52a-003 `insert_separators` and `insert_separators_reversed`.
5. T-52a-007 next-unread chapter selection (depends T-52a-003, area:07-domain-entries-items).
6. T-52a-008 next-unseen episode selection (depends T-52a-007, area:08-domain-library-category-history-updates).
7. T-52a-009 chapter dedupe by chapter number (parallel with 5 and 6).
8. T-52a-010 downloaded-only filter (depends area:23-downloads-manga, area:24-downloads-anime). Blocked until those areas deliver their download-state lookup.
9. T-52a-024 cache-directory file creation.
10. T-52a-023 stream save with cleanup (depends area:13-core-util-storage-archive-torrent).
11. T-52a-028 language display names, comparator, source group names (depends area:02-preferences-system, area:17-source-app-layer).
12. T-52a-031 tap-zone rectangle inversion (depends area:43-reader-core).
13. T-52a-056 job registry (depends area:22-backup-restore, area:23-downloads-manga, area:24-downloads-anime, area:26-library-update-jobs).
14. T-52a-053 notifications core (depends area:26-library-update-jobs, area:35-data-misc-updater-notification-export, area:36-ui-shell-navigation-security-webview).
15. T-52a-064 unit tests for every ported pure function (last; depends on the P0 tasks above and on T-52a-019, T-52a-020, T-52a-021, T-52a-025, T-52a-026, which are P1).

Sequencing risk: T-52a-064 is P0 but depends on P1 tasks. Either promote those four P1 tasks to P0 or accept that the test gate lands after the P0 features.

### 6.3 Risks

- Upstream Simkl and AniList parsing is by string search and has three bugs (`Bug A` slice by segment index, `Bug B` MAL lookup crossing entry boundaries, `Bug C` episode number taken from the rest of the file). See [p1-topic-airing-and-simkl.md](p1-topic-airing-and-simkl.md) section 3. Mitigation: parse with `serde_json` and use the test vectors.
- Airing writes are two non-atomic `updateAnime` calls upstream (area:04-db-anime-schema-migrations). A crash between them leaves a new episode with an old time. Mitigation: one `UPDATE` with both columns (Proposal in [p1-behavior.md](p1-behavior.md)).
- P0 blocked by other areas: T-52a-010 (area:23-downloads-manga, area:24-downloads-anime), T-52a-031 (area:43-reader-core), T-52a-053 (area:26-library-update-jobs, area:35-data-misc-updater-notification-export, area:36-ui-shell-navigation-security-webview), T-52a-056 (area:22-backup-restore, area:23-downloads-manga, area:24-downloads-anime, area:26-library-update-jobs). Mitigation: stub the lookup trait in `kuro-core` and wire the real implementation when the owning area lands.
- App lock through PAM can weaken security if misconfigured. Keep it opt-in and off by default.
- Dependency on `icu_*` crates and their data. Pin versions and check binary size.
- gtk4-rs 0.11 and GTK 4.16 API churn. Keep GTK code confined to `kurobayomi/src/ui/`.
- Platform APIs (`ashpd` portals, `zbus`, PAM) differ between KDE Plasma (Wayland and X11). Test notifications, open-URI and the lock on both.

## 7. Definition of done for this area

Observable behaviour in Kurobayomi:

1. Reading a manga: "next unread" opens the first chapter not marked read in the reading direction. The result matches the vectors `TC-07-a` to `TC-07-e` in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md).
2. Watching an anime: "next unseen" selects the same episode as the vectors for `TC-08` and the `EpsiodeGetNextUnseen` rules in [p1-topic-selection-and-sort.md](p1-topic-selection-and-sort.md).
3. Chapter list sort and dedupe match `TC-08-a` to `TC-08-e` and `TC-09-a` to `TC-09-d`.
4. Byte sizes shown in storage and download screens match `format_bytes` vectors `TC-21-a` to `TC-21-c`.
5. Dates in updates and releases show relative text matching `TC-25-a` (today = Friday 2026-10-09) and `TC-26-a` (now = 2026-10-09 12:00:00).
6. Covers: changing the remote thumbnail re-downloads the cover, and removing a cover deletes the cache file. Files live under the XDG cache path in the layout from [p1-behavior.md](p1-behavior.md) section 6.1.
7. Airing: an anime with a linked AniList, MAL or Simkl entry shows the next airing time. With no link it shows nothing and makes no request. A Simkl failure shows no time and does not crash.
8. Notifications appear in the desktop notification daemon with priorities from T-52a-054. A notification post failure is logged as `warn` and the operation continues.
9. Clipboard copy shows a toast on success and `clipboard_copy_error` on failure. Open-in-browser opens the URL in the default browser through the portal or `xdg-open`.
10. Share and save produce a file dialog, not an Android intent.
11. The tap-zone setting in the reader inverts the zones as in `TC-31`.
12. The reader background follows the theme setting. The animation scale follows the desktop setting.
13. Tablet and compact layouts switch at the window-width breakpoint from the layout table in [p1-behavior.md](p1-behavior.md) section 18.
14. App lock is off by default. When enabled, the app asks for the user's password through PAM before showing the library. Screenshot protection is not offered, with a note in settings.
15. The crash report dump (T-52a-017) and the debug information block (T-52a-018) are reachable from the crash screen, the advanced settings and the About screen, as in upstream (see [p1-ui-parity.md](p1-ui-parity.md) section 14).
16. `cargo test -p kuro-core` passes all test vectors listed in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5.
17. `cargo tree -p kuro-core` contains no `gtk`, `gio` or `glib` crate.
