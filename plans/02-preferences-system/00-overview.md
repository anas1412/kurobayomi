# Area 02: preferences system, overview

Area id: `02-preferences-system`. Parts: one (`p1-*`). Paths are repo-relative to `/home/blackbox/aniyomi-src` unless they start with `plans/` or `spec/`. Every item is labelled OBSERVED (what the Kotlin does) or Proposal (what Kurobayomi does).

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

The preference layer is a flat key/value store (Android `SharedPreferences`, one file `<package>_preferences.xml`) wrapped by a typed contract (`Preference<T>`, `PreferenceStore`). Eighteen `*Preferences` classes declare every user setting, the app state (last-used source, onboarding flag, last backup timestamp, incognito flag) and the private tracker credentials. Keys live in three namespaces that share one string space:

| Namespace | Prefix | Backed up | Examples |
|---|---|---|---|
| normal | none | yes | `pref_display_mode_library`, `pref_try_hwdec`, `default_category` |
| app-state | `__APP_STATE_` | never | `__APP_STATE_pref_downloaded_only`, `__APP_STATE_last_app_closed` |
| private | `__PRIVATE_` | only with "Private settings" | `__PRIVATE_pref_mangasync_password_<id>`, `__PRIVATE_track_token_<id>` |

Per-source settings are separate `SharedPreferences` files named `source_<id>`, written by extensions and rendered as settings pages. Backup stores app preferences as `Backup.backupPreferences` (ProtoNumber 104) and per-source preferences as `Backup.backupSourcePreferences` (ProtoNumber 105), each value wrapped in one of six typed messages (`Int`, `Long`, `Float`, `String`, `Boolean`, `Set<String>`). The inventory is [p1-inventory.md](p1-inventory.md): 34 files, 2306 lines.

### 1.2 Why it matters for a usable Kurobayomi (Proposal)

Every settings page, the reader and player behaviour, the library filters and sort, download and update rules, tracker switches and backup restore read these values. Without this area nothing persists: the mockup's settings rows are inert today. `src/ui.rs:221` (`item_row`) builds a widget from each `Item` and discards its state; `src/spec.rs` `Item` variants carry only title, subtitle, options and a default, with no preference key. This area makes those rows real (T-02a-030) and gives every other area one typed, persisted, observable source of configuration.

### 1.3 Scope boundaries

In scope: the typed store and its backends, codecs for enums and sort/display strings, change streams for the GTK main loop, the 328 registry entries and their defaults, the per-source store, the backup interop of preference entries, the Secret Service interface for tracker credentials, and the value tables that the reader and player need (flag masks, mpv mapping).

Out of scope (owned elsewhere): the settings screens and search (area:47-settings-screens-general, area:47b-settings-widgets-search-more, area:48-settings-player-and-scripts); the behaviour of reader, player, library and downloads that consumes the values (area:43-reader-core, area:44-reader-viewers, area:45-player-core, area:46-player-controls-ui, area:37-ui-library, area:23-downloads-manga, area:24-downloads-anime, area:26-library-update-jobs); the backup container and restore orchestration (area:21-backup-format-and-create, area:22-backup-restore); the palettes behind `AppTheme` (area:49-theme-and-design-system); user-facing strings (area:51-i18n); extension loading and extension settings rendering (area:20-extension-strategy-redesign).

## 2. Module map

### 2.1 Kotlin packages (OBSERVED)

| Path (repo-relative) | Classes and files | Responsibility |
|---|---|---|
| `core/common/src/main/java/tachiyomi/core/common/preference/` | `Preference.kt`, `PreferenceStore.kt`, `AndroidPreference.kt`, `AndroidPreferenceStore.kt`, `InMemoryPreferenceStore.kt`, `TriState.kt`, `CheckboxState.kt` | Store contract, typed handles, read/write rules, change flows, namespace prefixes, tri-state helpers, test store |
| `core/common/src/main/java/eu/kanade/tachiyomi/core/security/` | `SecurityPreferences.kt` | App lock delay, lock flag, secure-screen mode |
| `core/common/src/main/java/eu/kanade/tachiyomi/network/` | `NetworkPreferences.kt` (reads `DohProviders.kt`) | User agent, DNS-over-HTTPS provider, verbose logging |
| `core/common/src/main/java/aniyomi/core/common/torrent/` | `TorrentPreferences.kt` | TorrServer port, trackers, proxy mode |
| `app/src/main/java/eu/kanade/domain/base/` | `BasePreferences.kt`, `ExtensionInstallerPreference.kt` | Downloaded-only, incognito, onboarding flags; Android-only installer and bitmap knobs |
| `app/src/main/java/eu/kanade/domain/ui/` and `.../ui/model/` | `UiPreferences.kt`; `AppTheme.kt`, `NavStyle.kt`, `StartScreen.kt`, `TabletUiMode.kt`, `ThemeMode.kt` | Theme mode and palette name, date format, relative time, start tab, navigation style |
| `app/src/main/java/eu/kanade/domain/source/service/` | `SourcePreferences.kt` | Source languages, hidden and pinned sources, extension repositories, data saver, migration sort, last-used source |
| `app/src/main/java/eu/kanade/domain/track/service/` and `.../track/model/` | `TrackPreferences.kt`; `AutoTrackState.kt` | Tracker switches, score type, tracker credentials and expiry flag |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/` | `PlayerPreferences.kt`, `GesturePreferences.kt`, `DecoderPreferences.kt`, `AudioPreferences.kt`, `SubtitlePreferences.kt`, `AdvancedPlayerPreferences.kt` | Player behaviour, gestures, hardware decoding and filters, audio, subtitles, mpv user files |
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/` | `ReaderPreferences.kt` (reads `ReadingMode.kt`, `ReaderOrientation.kt`) | Manga reader behaviour, page scaling, tap zones, colour filters, webtoon options |
| `app/src/main/java/eu/kanade/tachiyomi/data/preference/` | `SharedPreferencesDataStore.kt` | Bridge from `SharedPreferences` to AndroidX preference widgets (extension settings) |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/` | `BackupExtensionPreferences.kt` | Unused proto model (no references) |
| `domain/src/main/java/tachiyomi/domain/library/service/` | `LibraryPreferences.kt` (reads sort-mode, display-mode and flag files) | Library display, sort, filters, badges, update rules, per-entry default templates, swipe actions |
| `domain/src/main/java/tachiyomi/domain/download/service/` | `DownloadPreferences.kt` | Download slots, speed limit, auto-download and auto-remove rules |
| `domain/src/main/java/tachiyomi/domain/backup/service/` | `BackupPreferences.kt` (reads `PreferenceValues.kt`) | Auto-backup interval and last run |
| `domain/src/main/java/tachiyomi/domain/storage/service/` | `StoragePreferences.kt` | Base storage directory |
| `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/utils/` | `Preferences.kt` | `source_<id>` key and file per source |
| `app/src/main/java/eu/kanade/tachiyomi/di/` | `PreferenceModule.kt` (read only) | One store and eighteen singletons |
| Read-only consumers | `PreferenceBackupCreator.kt`, `PreferenceRestorer.kt`, `BackupRestorer.kt`, `AniyomiMPVView.kt`, `PlayerActivity.kt`, `TrackSelect.kt`, `SecureActivityDelegate.kt` | Show how values reach the backup, the mpv options, the track picker and the app lock |

### 2.2 Rust crates and modules (Proposal, from [p1-rust-design.md](p1-rust-design.md) section 1)

```
crates/kuro-prefs/            no GTK dependency, headless tests
  src/lib.rs                  Prefs aggregate, re-exports
  src/value.rs                PrefValue, Namespace, key prefixes
  src/codec.rs                PrefType, Decoded<T>, pref_enum! macro, object codecs
  src/pref.rs                 Pref<T> handle (get, set, delete, update, changes, insert/extend/remove, toggle)
  src/store.rs                PrefStore (memory map, broadcast change bus, flush scheduling)
  src/backend/mod.rs          trait Backend
  src/backend/toml.rs         TomlBackend: config.toml and state.toml, atomic write, fd-lock
  src/backend/memory.rs       MemoryBackend (replaces InMemoryPreferenceStore)
  src/layout.rs               key to (TOML table, field) for the registry and dynamic families
  src/secrets.rs              SecretPref over kuro_platform::SecretStore
  src/tristate.rs             TriState, CheckboxState
  src/sort.rs                 LibraryDisplayMode, MangaLibrarySort, AnimeLibrarySort, flag algebra
  src/flags.rs                chapter, episode and season flag masks and helpers
  src/datefmt.rs              java DateTimeFormatter pattern to chrono translator
  src/schema/mod.rs           define_prefs! macro, Prefs struct
  src/schema/{base,ui,sources,tracking,library,downloads,backup,storage,security,network,torrent,reader,player}.rs
  src/source.rs               SourcePrefs (typed per-source store)
  src/backup.rs               export and import of neutral PrefEntry structs (protobuf lives in area:21)
  src/legacy.rs               legacy key rewrites (hook for area:11) and Aniyomi key import table
crates/kuro-platform/         Paths and SecretStore traits (used, never bypassed)
src/bind.rs (kuro-ui::bind)   GTK binders: switch, dropdown, chips, scale, entry, multi, tristate, sort
```

Dependencies (Proposal): `serde`, `toml_edit`, `tokio` (sync, rt, time, macros), `parking_lot`, `thiserror`, `tracing`, `atomic-write-file`, `fd-lock`, `sys-locale`, `chrono`, `kuro-platform`. Dev: `proptest`, `tempfile`.

## 3. Reading guide

| Document | One-line description |
|---|---|
| [p1-inventory.md](p1-inventory.md) | The 34 scope files with LOC, declared names, Port/Rewrite/Replace/Drop decision and target module per file |
| [p1-behavior.md](p1-behavior.md) | Algorithms: read/write/change contract, namespaces, enum and sort codecs, flag bit tables, per-class behaviour, app-lock state machine, restore and concurrency rules |
| [p1-data-and-config.md](p1-data-and-config.md) | Where values live on Linux (config.toml, state.toml, keyring, per-source TOML, mpv files), typed value model, integer bit layouts, enum stored names, environment defaults, DoH and tracker lists |
| [p1-rust-design.md](p1-rust-design.md) | Crate and module layout, core types, `Pref<T>` API, store and backend flushing, change streams and GTK binders, typed sections, secrets, per-source store, backup surface, mockup wiring |
| [p1-topic-key-registry.md](p1-topic-key-registry.md) | Every preference row (Kotlin function, key, store, type, default, Rust field, decision, priority, notes) for the 18 classes |
| [p1-topic-default-config.md](p1-topic-default-config.md) | Reference rendering of config.toml and state.toml with every default (a reference, not written to disk; see decision D2) |
| [p1-topic-quirks-and-bugs.md](p1-topic-quirks-and-bugs.md) | Quirks Q1 to Q24 with path:line evidence, impact and the Kurobayomi decision for each |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E1 to E40, race conditions, the test files to write, fixtures, golden backup bytes |
| [p1-topic-backup-interop.md](p1-topic-backup-interop.md) | Preference part of the backup: proto messages, polymorphic encoding, creation and restore algorithms, category remapping, legacy keys |
| [p1-topic-player-mpv-mapping.md](p1-topic-player-mpv-mapping.md) | Preference-to-mpv option table, initialisation order, live versus next-open application, track selection algorithm |
| [p1-tasks.md](p1-tasks.md) | The 40 implementation tasks T-02a-001 to T-02a-040 with priority, effort, dependencies, sources and acceptance |
| [00-overview.md](00-overview.md) | This document |

## 4. Key design decisions and open questions

### 4.1 Decided in the part docs (kept as written)

* D1. Stored enum names are the exact Kotlin constant names, and the `pref_enum!` macro keeps them (p1-behavior 3.1, p1-rust-design 2).
* D2. Only explicitly set keys are written to disk, and `delete()` removes the key (T-02a-002 acceptance, p1-rust-design 3 step 2). The defaults in p1-topic-default-config are a reference rendering.
* D3. Key strings keep Aniyomi spelling so backups copy straight across. Rust field names are the snake_case of the Kotlin function (p1-rust-design 5).
* D4. Reads keep Aniyomi semantics: wrong primitive kind deletes the key and returns the default; an enum or object value that does not decode returns the default and stays on disk (E1, E2, Q16).
* D5. Tracker usernames, passwords and tokens go only to the Secret Service through `SecretStore`; the expiry flag goes to `state.toml` (T-02a-018, T-req-platform-003).
* D6. Android-only keys (installer, PiP, display profile, tablet mode, volume keys, cutout, status bar, orientation, secure screen, backup flags, external downloader selection) stay in the registry so restored values round-trip unchanged, but no UI shows them (T-02a-032).
* D7. Shared-key quirk Q2 is fixed internally (two independent fields) while the wire key stays shared on export.
* D8. Filter tri-state, sort-string and flag integer codecs are reproduced exactly (p1-behavior 3.2 to 3.5, T-02a-007).
* D9. A single `Prefs` aggregate is created once at startup before any widget (T-02a-039, p1-rust-design 9 step 7).

### 4.2 Open questions for the user

Each question has a recommendation. None blocks T-02a-001 to T-02a-009, which are behaviour-neutral.

| # | Question | Recommendation | Why it matters |
|---|---|---|---|
| Q-A | Default app theme: `DEFAULT` (inherit the user's GTK theme, no extra CSS) or a bundled Tokyo Night stylesheet? | `DEFAULT` as the enum default, as written in p1-rust-design 9 item 6. Confirm the user's GTK theme is Tokyo Night; if it is, `DEFAULT` already looks right with no CSS shipped here. | The task requires the user's GTK theme to apply; the user's stated preference is Tokyo Night everywhere. Both must hold. |
| Q-B | Store every tracker password, or only tokens? | Store username, password and token in the Secret Service, as Aniyomi does (T-02a-018). | Affects how much credential material leaves the keyring. |
| Q-C | Keep an app lock at all? If yes, passphrase (argon2id hash in keyring) instead of biometrics? | Keep it as P2 with a passphrase (p1-behavior 5.10, area:36). Drop it if the user never locks the app. | Biometric lock does not exist on Linux desktops; this is a product choice. |
| Q-D | What does "Wi-Fi only" mean on Linux? | "Unmetered connection only", via GIO `NetworkMonitor` `network-metered` (T-02a-017). Metered Wi-Fi counts as not Wi-Fi; unmetered Ethernet counts as Wi-Fi (p1-behavior 8). | Desktops rarely expose the transport type. |
| Q-E | Auto and Auto-safe audio channels: keep the swapped mpv mapping? | Keep the wire values and labels (Q5) and add help text that explains the swap. Do not "fix" labels, which come from area:51. | Changing the mapping changes what existing Aniyomi backups mean. |
| Q-F | Which external tools may the player and downloader call? | Player: a command template with `$URL` (mpv, vlc). Downloader: a command template (aria2c). Document both in area:45 and area:25. | `external_player_preference` is an Android package name in backups (E39). |
| Q-G | Should Kurobayomi-only settings travel in backups? | Yes, as normal keys with a `kuro_` prefix (T-02a-040). Aniyomi restores them harmlessly. | Keeps user-visible Kurobayomi settings across backup and restore. |
| Q-H | Retry schedule after a failed write: which one? | The E12 schedule: 1, 2, 4, 8, 16 seconds, then every 30 seconds, toast after 3 failures. Update p1-rust-design 3 to match. | Two docs disagree (section 8, item 8). |
| Q-I | Promote T-02a-036 (value ranges) and T-02a-038 (shutdown flush) to P0? | Yes, both are S effort. T-02a-030 (P0) needs the ranges for its widgets, and a quit within 250 ms otherwise loses the change. | Priority inversion (section 6.4). |
| Q-J | Default `source_languages` from the environment? | Yes: `all`, `en` and the first two letters of `LC_MESSAGES` or `LANG` (not `C` or `POSIX`), stored once (E19, T-02a-035). | Browse results depend on it on first run. |
| Q-K | Are the Kotlin SY keys worth porting? | No. Remove the four commented-out SY rows from the registry (section 6.3). | They have no live code path (Q24). |

## 5. Cross-area dependencies

### 5.1 This area needs

| Need | From | Used by |
|---|---|---|
| `Paths` trait (config, state, data, cache dirs, default storage dir) | T-req-platform-001 (kuro-platform), area:01-foundation-app-bootstrap | T-02a-001, 002, 016, 024, 035 |
| Tokio runtime and GLib main context; `ui::ctx()` and app startup order | area:01-foundation-app-bootstrap | T-02a-001, 004, 039 |
| `SecretStore` (Secret Service, encrypted-file fallback) | T-req-platform-003 | T-02a-018, E22 |
| Single-instance forwarding and the GIO shutdown hook | area:01-foundation-app-bootstrap | T-02a-002, 038 |
| Migration runner and `last_version_code` | area:11-app-migrations | T-02a-029, 034 |
| Category table (ids and names) during restore | area:22-backup-restore | T-02a-026 |
| Scheduler for update jobs and auto-backup | area:26-library-update-jobs, area:22-backup-restore | T-02a-024, 026 |
| Protobuf encode and decode of `BackupPreference` (`prost`) | area:21-backup-format-and-create | T-02a-025, 026 |
| Extension trust unit, repository import, extension settings rendering | area:20-extension-strategy-redesign | T-02a-019, 027, 031 |
| `gtk::Settings` dark-mode and portal `org.freedesktop.appearance` | area:36-ui-shell-navigation-security-webview | T-02a-023 |
| Palettes and theme CSS for non-DEFAULT `AppTheme` values | area:49-theme-and-design-system | T-02a-023 |
| Message keys for enum labels and skip reasons | area:51-i18n | T-02a-011, 023 |
| Settings spec files and item rendering | area:47-settings-screens-general, area:47b-settings-widgets-search-more | T-02a-030, 037 |
| Chapter, episode and season flag consumers | area:07-domain-entries-items, area:08-domain-library-category-history-updates | T-02a-009, 010 |

### 5.2 Other areas need from this area

| Consumer | What it needs | Provided by |
|---|---|---|
| area:12-network-stack | `default_user_agent`, `doh_provider`, `verbose_logging` | T-02a-020 |
| area:13-core-util-storage-archive-torrent | `torr_server_*` values (port, trackers, proxy mode, enable flag) | T-02a-021 |
| area:17 and area:20 (extensions) | `SourcePrefs` per source, `SourcePrefs::open`, widget binders | T-02a-027, 031 |
| area:21 and area:22 (backup) | `export_app`, `export_sources`, `import_app`, `import_sources`, `ImportReport` | T-02a-025, 026 |
| area:23, 24, 25 (downloads) | `DownloadPreferences` fields, `categoryPreferenceKeys` names | T-02a-017 |
| area:26 (library update jobs) | interval, device and item restrictions, category include and exclude sets, `filter_update_candidates` | T-02a-008, 011 |
| area:27 and trackers | credentials through `SecretPref`, tracker switches, `set_credentials` order | T-02a-018 |
| area:34 (images) | data saver mode, quality, excluded sources | T-02a-019 |
| area:35 (updater) | `Prefs::in_memory()` for the release-check test | T-02a-033 |
| area:36 (app lock, shell) | `require_unlock` function, lock delay values, `secure_screen_v2` (hidden) | T-02a-022 |
| area:37 to 42 (library, entry details, browse, updates) | `MangaLibrarySort`, `AnimeLibrarySort`, `LibraryDisplayMode`, `TriState`, default templates | T-02a-007, 008, 009 |
| area:43 and area:44 (reader) | reader keys and value tables (reading mode, orientation, tap zones, scale type, zoom start, colour filter modes) | T-02a-012, 013 |
| area:45, 46, 48 (player) | player, gesture, decoder, audio, subtitle keys, mpv option mapping, `TrackSelect`, `CustomKeyCodes` | T-02a-014, 015, 016 |
| area:07 and area:08 (domain) | `chapter_flags`, `episode_flags`, `season_flags` constants in `kuro_prefs::flags` | T-02a-009, 010 |
| area:10 (domain layer) | `TriState` and `CheckboxState` | T-02a-006 |
| area:47 and area:48 (settings screens) | every `pref` path and its `values`, registry ranges | T-02a-030, 036, 037 |
| area:49 (theme) | `AppTheme` names and order, `ThemeMode` | T-02a-023 |

## 6. Task summary

### 6.1 Counts

Source: `plans/TASKS.csv` rows whose file starts with `02-preferences-system/` (exported with `python3 plans/_tools/tasks.py export`, which regenerated `plans/TASKS.csv` on this run). `python3 plans/_tools/tasks.py lint plans/02-preferences-system/p1-tasks.md` prints `ok`.

Total 40 tasks (T-02a-001 to T-02a-040).

| Priority | S | M | L | Total |
|---|---|---|---|---|
| P0 | 7 | 7 | 3 | 17 |
| P1 | 6 | 7 | 0 | 13 |
| P2 | 4 | 3 | 0 | 7 |
| P3 | 3 | 0 | 0 | 3 |
| All | 20 | 17 | 3 | 40 |

P0 tasks: T-02a-001 (L), 002 (L), 003 (S), 004 (M), 005 (S), 006 (S), 007 (M), 008 (M), 009 (M), 012 (L), 013 (S), 014 (M), 020 (S), 023 (M), 030 (M), 035 (S), 039 (S).

### 6.2 P0 critical path (in dependency order)

1. External: area:01-foundation-app-bootstrap and T-req-platform-001 (`Paths`). Without them no file is opened.
2. T-02a-006 (no dependencies) and T-02a-001 (L, store core, registry macro) start in parallel.
3. T-02a-007 (M, codecs) after T-02a-001. This unlocks the enum-typed sections.
4. T-02a-002 (L, TOML backend, lock, flusher) after T-02a-001 and T-req-platform-001. T-02a-004 (M, change bridge) after T-02a-001 in parallel.
5. T-02a-003 (S, namespaces) and T-02a-005 (S, helpers) after T-02a-002; T-02a-035 (S, environment defaults) after T-02a-001 and T-req-platform-001.
6. T-02a-039 (S, `Prefs` aggregate) after T-02a-001 and T-02a-002.
7. Schema sections in parallel: T-02a-008 (M, library) needs 001, 006, 007; then T-02a-009 (M, default templates). T-02a-012 (L, reader) then T-02a-013 (S, value tables). T-02a-014 (M, player, gesture, decoder). T-02a-020 (S, network). T-02a-023 (M, UI, base).
8. T-02a-030 (M, bind mockup pages) last: needs 001, 004 and 039. It also needs the ranges of T-02a-036 (P1) to render sliders (section 6.4).

Two longest chains by effort ceiling (S at most 1 day, M at most 3 days, L at most 7 days, from the effort definitions): 001, 002, 039, 030 (7, 7, 1, 3 days) and 001, 007, 008, 009 or 001, 007, 012, 013 (7, 3, 7, 1 days). Each chain is about 18 days at ceilings. The P0 set as a whole is at most 49 engineer-days at ceilings.

### 6.3 Corrections needed in the registry before T-02a-001 starts

These are facts checked against the Kotlin source during review. They change the test that T-02a-001 asserts ("REGISTRY has 328 entries and equals the registry rows").

* SourcePreferences has 30 live declarations, not 34. The four rows 22 to 25 (`eh_enable_source_blacklist`, `sources_tab_categories`, `sources_tab_categories_filter`, `sources_tab_source_categories`) are commented out at `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt` (the `// SY -->` block, lines 86 to 95). Remove them from the registry, or mark them "not ported".
* Consequence: 324 live Kotlin declarations (328 rows minus the four commented SY rows). Two of them share a key string (`default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`, rows 58 to 61), so 322 distinct live key strings.
* The registry header says "327 rows + `extension_installer`". The table has 328 rows and `extension_installer` is already row 1 of BasePreferences. Fix the header.
* Rows 14 and 27 to 38 give the Kotlin type as "Enum None". The source uses `TriState` (for example `LibraryPreferences.kt:74-77`, `filterIntervalCustom` uses `TriState.DISABLED`). Correct the type column.
* Filters: the library has ten plain TriState filters (rows 27 to 36). p1-data-and-config section 2 says "nine". Correct it.
* Data saver: p1-data-and-config section 2 and p1-topic-default-config put the data saver keys in a `[sources.data_saver]` sub-table. The registry gives flat Rust fields (`Settings.sources.data_saver_*`). Choose one; the registry naming is recommended because the registry "Rust field" column is the authority for Rust field names (p1-rust-design 5, "Field naming rule").

### 6.4 Risks

| # | Risk | Mitigation |
|---|---|---|
| R1 | The registry errors in 6.3 would be encoded in the T-02a-001 test and in the default config. | Fix the registry first (one pass by the part author). |
| R2 | p1-topic-default-config lists the four SY keys as active under `[sources]`, and writes `extension_installer = "PACKAGEINSTALLER (LEGACY on MIUI)"`, which is not a stored value. | Treat it as a reference only (D2); correct both lines when the registry is fixed. |
| R3 | Priority inversion: T-02a-030 (P0) needs ranges from T-02a-036 (P1) and the flush guarantee from T-02a-038 (P1). | Promote both to P0 (Q-I). |
| R4 | Golden backup bytes in p1-edge-cases-and-tests section 4 were derived from encoding rules and are not verified against a real `.tachibk`. T-02a-025 cannot be final without one. | The user supplies a scrubbed `.tachibk` (fixture listed in p1-edge-cases-and-tests section 5). Until then the bytes stay provisional. |
| R5 | Secret Service availability on KDE Plasma. The API is normally served by KWallet; if it is off, the encrypted-file fallback of T-req-platform-003 applies and the user sees one notice. | Test both paths; keep the fallback in T-req-platform-003. |
| R6 | Two processes writing one config directory. `fd-lock` releases on process exit, so a crash never leaves a stale lock. | Keep E14 and the `Locked` test (T-02a-002). |
| R7 | TOML float round trip. `0.85F` must stay `0.85`, not `0.8500000238418579`. | Write shortest-round-trip text (p1-rust-design 11). Test with `f32` epsilon (E23). |
| R8 | The mockup `spec::Item` variants do not carry the fields the design assumes. `Tristate` has only `title`; `Sort` has `options` but no `default`. | T-02a-030 adds `pref` and `values` to every bound variant; `every_pref_exists_in_registry` test catches gaps. |
| R9 | The export step of `tasks.py` printed unknown-area warnings for tasks in other folders (for example `10-app-domain-layer`, `addenda/requirements-cross-platform-tasks.md`). Not caused by this area. | Owners of those files fix their area tokens. |
| R10 | Scope creep into the screens. The settings pages need the spec change (R8) and a search index change (T-02a-037). | Keep T-02a-030 limited to the five P0 pages named in its acceptance. |

## 7. Definition of done for this area in Kurobayomi

Each item is observable. A test or a manual check named in brackets proves it.

1. A new install writes no preference file until the first explicit change. After one switch changes, `config.toml` contains that key and `schema = 1`, and the change is on disk within 400 ms. [T-02a-002, T-02a-030]
2. Every setting on the bound pages (appearance, library, reader, downloads, browse) keeps its value after a restart. [manual, T-02a-030 acceptance]
3. A `config.toml` that does not parse is renamed to `config.toml.corrupt-<unix seconds>`; the app starts with defaults and shows "Settings file was unreadable; defaults restored (old file kept)" once. A field with a wrong type resets only that field. [T-02a-002]
4. Tracker usernames, passwords and tokens never appear in `config.toml` or `state.toml` (grep test), and they are stored in the Secret Service item `application=kurobayomi`. [T-02a-018]
5. Theme mode LIGHT and DARK set `gtk-application-prefer-dark-theme` to false and true; SYSTEM follows the portal `color-scheme` and updates when it changes. [T-02a-023]
6. Changing the reader default reading mode, tap zones, scale type, zoom start or colour filter updates an open reader within one main-loop iteration. [T-02a-012]
7. The decoder option builder returns `hwdec=auto-safe` or `no`, `deband=yes` for GPU, and one `vf` chain that contains both `format=yuv420p` and `gradfun=radius=12` when CPU debanding and YUV420P are on (Q9). [T-02a-014]
8. "Set as default" on an anime entry does not change the manga default display or sort (Q2 fixed). Setting the season default writes the downloaded filter from the downloaded mask (Q3). [T-02a-009, T-02a-010]
9. The start swipe action for chapters reads `pref_chapter_swipe_end_action` (Q1 pairing kept on the wire). [T-02a-011]
10. A real Aniyomi `.tachibk` imports its preferences; a second import leaves `config.toml` byte-identical; `default_category` maps by category name; `-1` and `0` map to themselves. [T-02a-026, R4 fixture]
11. Export contains only explicitly set normal keys, with Aniyomi key strings and kinds, sets sorted, source files in ascending id order. [T-02a-025]
12. Android-only keys survive import and export with the same kind and value. [T-02a-032]
13. Settings search for "tapping" finds the tap-inversion settings. [T-02a-037]
14. Date pattern `MM/dd/yy` renders 2024-03-09 as `03/09/24`, `dd/MM/yy` as `09/03/24`, `yyyy-MM-dd` as `2024-03-09`, `dd MMM yyyy` as `09 Mar 2024`, `MMM dd, yyyy` as `Mar 09, 2024`; an empty pattern gives the locale short date. [T-02a-023]
15. Quitting within 100 ms of a change keeps the change on disk. [T-02a-038]
16. After three failed writes, one toast `settings_save_failed` appears; values stay readable from memory. [T-02a-038, E12]
17. A second instance fails the lock with `PrefError::Locked` and forwards its arguments to the first window. [T-02a-002, E14]
18. `cargo test -p kuro-prefs` passes headless, and the registry test passes against the corrected registry (section 6.3). [T-02a-001]

## 8. Defects found in the part documents during this review

Checked against the source and against each other. Not fixed in this pass, except the area tokens (section 8.0). Owner: the part author.

0. Area tokens: short forms such as `area:20` were expanded to full ids (for example `area:20-extension-strategy-redesign`) in 91 places across 10 files of this folder. `tasks.py lint p1-tasks.md` still prints `ok`. No task ids were changed.
1. The registry counts four commented-out SY keys as live (section 6.3). T-02a-019 acceptance ("all 34 keys exist") and quirk Q24 ("not ported") contradict each other.
2. p1-topic-default-config `[sources]` lists the SY keys as active values.
3. p1-topic-default-config `[base]` gives `extension_installer` the value `"PACKAGEINSTALLER (LEGACY on MIUI)"`; the stored value is `PACKAGEINSTALLER`, and the LEGACY rule is code in `ExtensionInstallerPreference.kt`.
4. p1-topic-default-config `[security]` uses `secure_screen = "INCOGNITO"`; the registry row is `secure_screen_v2` (key) with Rust field `secure_screen` (DROP). Use the key name in the default file.
5. p1-rust-design section 3 gives the retry schedule 1, 2, 4 seconds capped at 30; p1-edge-cases E12 gives 1, 2, 4, 8, 16 seconds then 30. Resolve as Q-H.
6. p1-rust-design section 2 is ambiguous for an equal-value `set`: its first sentence says it "still publishes", its proposal detail says it skips. The decision (skip, as T-02a-005 requires) should replace the first sentence.
7. p1-tasks T-02a-023 says "the five listed patterns"; p1-behavior 5.3 and p1-edge-cases section 4 list six UI patterns, one of them the empty pattern. Say that the five non-empty patterns are the ones tested.
