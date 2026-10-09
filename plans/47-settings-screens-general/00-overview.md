# Area 47 — Settings screens (general): overview

Area id: `47-settings-screens-general`. Repo root for every path below: `/home/blackbox/aniyomi-src` (repo-relative paths). Target: Kurobayomi (`/home/blackbox/kurobayomi`), Rust stable, gtk4-rs 0.11 with feature `v4_16`, no libadwaita, tokio, rusqlite, reqwest, serde.

Status of this document: the two parts of the area are fully written; this overview adds the cross-part reconciliation, decisions, dependencies and summary. Nothing in this overview replaces the detailed specs in the part documents.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

The general settings screens are the Compose pages reached from Aniyomi's "More > Settings" menu. They let the user change every stored preference, run maintenance actions (clear non-library database rows, backup and restore, storage cache clear, CSV export), manage extension repositories and anime extension stores, log in to trackers, and read diagnostics (debug info, worker info, backup schema, licences, About).

The screens do not render the data themselves. Each row is a `Preference` object (see `app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`) that binds a typed key in a preference class to a widget, so the value is read from and written to the store on every change. Screens in this area are thin: the behaviour lives in the preference classes, domain services and job classes they call, most of which belong to other areas.

### 1.2 Scope split

The area is split across two parts, both owned by this area:

| Part | Folder files | Kotlin files (in scope) | LOC | Tasks | Covers |
|---|---|---|---|---|---|
| Part 1 (`T-47a-*`) | `p1-*.md` | 18 (`p1-inventory.md`) | 4,891 | 98 | Main menu, search, Appearance, Library, Reader, Downloads, Tracking, Browse, Security, Advanced, Data (scope rows), About, Clear-database screens, licence screens |
| Part 2 (`T-47b-*`) | `p2-*.md` | 18 (`p2-inventory.md`) | 2,596 | 70 | Settings router, app language, anime extension stores, manga extension repos, create/restore backup, storage usage, debug info, worker info, backup schema |

Total: 36 Kotlin files, 7,487 LOC, 168 tasks.

### 1.3 Why it matters for a usable Kurobayomi

- Every P0 reading and watching behaviour depends on a preference that a settings row writes (reader mode, reader display, library update behaviour, category defaults, storage location).
- Extension stores and manga extension repos are the source mechanism for the app. Without the two repo screens the user cannot add a source, so they are P0 in part 2.
- Without the Clear-database screens and the backup/restore pages, the user cannot recover data or reset a broken local database. Backup is P1 in this area because the core backup engine belongs to area:21 and area:22.
- The settings search and the two-pane layout are the navigation contract for every other settings page in the app.

### 1.4 Out of scope (owned elsewhere)

- Player settings and scripts: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/*` and the player-related rows (area:48-settings-player-and-scripts).
- Widgets and search engine internals shared by all screens, the `Preference` widget classes and tri-state dialog: area:47b-settings-widgets-search-more.
- The behaviour of the things the rows configure: reader (area:43, area:44), library update jobs (area:26), backup engine (area:21, area:22), trackers (area:27 to area:33), updater (area:35), theme (area:49), i18n (area:51).

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities

| Package / file (repo-relative) | Responsibility | In part |
|---|---|---|
| `app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt` | Settings tree model (`Preference.PreferenceItem.*`, `PreferenceGroup`) | support, read by both parts |
| `app/src/main/java/eu/kanade/presentation/more/settings/widget/` | Row widgets (switch, list, multi, slider, text, info, tracker rows) | support, area:47b |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/` (root) | `SettingsMainScreen.kt` (menu), `SettingsSearchScreen.kt` (search), `SearchableSettings.kt` (screen contract), `Commons.kt` (category label), and the nine `Settings*Screen.kt` pages (Advanced, Appearance, Browse, Data, Download, Library, Reader, Security, Tracking) | part 1 |
| `.../screen/about/` | `AboutScreen.kt`, `OpenSourceLicensesScreen.kt`, `OpenSourceLibraryLicenseScreen.kt` | part 1 |
| `.../screen/advanced/` | `ClearDatabaseScreen.kt` (manga), `ClearAnimeDatabaseScreen.kt` (anime) | part 1 |
| `.../screen/appearance/` | `AppLanguageScreen.kt` | part 2 |
| `.../screen/browse/` | `AnimeExtensionStoresScreen.kt` + `AnimeExtensionStoresScreenModel.kt`, `MangaExtensionReposScreen.kt` + `MangaExtensionReposScreenModel.kt` | part 2 |
| `.../screen/browse/components/` | `ExtensionReposDialogs.kt`, `anime/` (content, dialogs, screen), `manga/` (content, screen) | part 2 |
| `.../screen/data/` | `CreateBackupScreen.kt`, `RestoreBackupScreen.kt`, `StorageInfo.kt` | part 2 |
| `.../screen/debug/` | `DebugInfoScreen.kt`, `WorkerInfoScreen.kt`, `BackupSchemaScreen.kt` | part 2 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt` | Settings router (destinations `About`=0, `DataAndStorage`=1, `Tracking`=2; phone and tablet) | part 2 |
| `core/common/src/main/java/tachiyomi/core/common/preference/` | `Preference<T>`, `PreferenceStore`, `AndroidPreferenceStore`, `InMemoryPreferenceStore`, `TriState` | support |
| `domain/src/main/java/tachiyomi/domain/*/service/*Preferences.kt` and `app/src/main/java/eu/kanade/domain/*/` `*Preferences.kt` | Typed preference classes read by the screens (`LibraryPreferences`, `DownloadPreferences`, `StoragePreferences`, `BackupPreferences`, `TrackPreferences`, `UiPreferences`, `SourcePreferences`, `BasePreferences`) | support, both parts |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/` and `.../restore/` | Backup options, create and restore jobs (`BackupCreateJob`, `BackupOptions`, `RestoreOptions`) | read by part 2 |

### 2.2 Proposed Rust module map

Single tree under `src/settings.rs` (Rust 2018 module style, no `mod.rs`). This reconciles two conflicting proposals in the part documents (see section 4, decision D-1).

```
src/settings.rs                      body(id) dispatch, shared Node/Row types re-exported
src/settings/model.rs                Node, Row, RowKind, ScreenDef                      (p1-rust-design section 4)
src/settings/store.rs                PrefStore (SQLite kv), Pref<T>, change channels     (p1-rust-design section 2)
src/settings/prefs.rs                Prefs struct, every key typed                       (p1-rust-design section 3)
src/settings/enums.rs                ReadingMode, ReaderOrientation, ThemeMode, ...      (p1-rust-design section 2.2)
src/settings/labels.rs               category_label() include/exclude text               (T-47a-005)
src/settings/search.rs               SearchIndex::build(&Prefs), query()                 (T-47a-004)
src/settings/widgets.rs              row builders                                        (p1-rust-design section 1)
src/settings/dialogs.rs              modal builders (radio, multi, text, confirm, tri, numeric)
src/settings/auth.rs                 confirm(title, subtitle) for app lock               (T-47a-068)
src/settings/screens/mod.rs          by_id() registry
src/settings/screens/appearance.rs   T-47a-010..019
src/settings/screens/library.rs      T-47a-020..030
src/settings/screens/reader.rs       T-47a-031..043
src/settings/screens/downloads.rs    T-47a-044..052
src/settings/screens/tracking.rs     T-47a-053..059
src/settings/screens/browse.rs       T-47a-060..062
src/settings/screens/security.rs     T-47a-063..068
src/settings/screens/advanced.rs     T-47a-069..087
src/settings/screens/data.rs         T-47a-088..092 (storage location, export, cache)
src/settings/screens/about.rs        T-47a-093..095
src/settings/screens/licenses.rs     T-47a-096..097
src/settings/screens/clear_db.rs     T-47a-075 (manga and anime variants)
src/settings/screens/main.rs         T-47a-003, T-47a-098 (menu, sidebar)
src/settings/shell.rs                router, two-pane switch, back navigation            (p2-rust-design section 2, T-47b-001..003)
src/settings/toast.rs                in-window toast overlay                             (T-47b-052)
src/settings/app_language.rs         language list and apply                             (T-47b-005..008)
src/settings/repos/mod.rs            shared repo model and UI helpers
src/settings/repos/model.rs          AnimeStore, MangaRepo, dialog and state enums
src/settings/repos/anime.rs          anime store page and model                          (T-47b-009..018)
src/settings/repos/manga.rs          manga repo page and model                           (T-47b-019..026)
src/settings/repos/dialogs.rs        all repo dialogs                                    (T-47b-016/017/021/024/025)
src/settings/repos/store.rs          rusqlite access: extension_store, extension_repos   (T-47b-009, T-47b-022)
src/settings/backup/mod.rs
src/settings/backup/options.rs       BackupOptions, RestoreOptions, OptionEntry          (T-47b-027, T-47b-028)
src/settings/backup/create.rs        create page, save dialog, job hand-off              (T-47b-029, T-47b-030)
src/settings/backup/restore.rs       restore page, validation state machine              (T-47b-031..034, T-47b-036)
src/settings/backup/validator.rs     missing sources and trackers                        (T-47b-032)
src/settings/storage.rs              mount scan, statvfs, size format                    (T-47b-037..039)
src/settings/debug/mod.rs            text-dump page helper                               (T-47b-047)
src/settings/debug/info.rs           app and device info                                 (T-47b-040, 041, 048, 049, 050)
src/settings/debug/workers.rs        worker info page                                    (T-47b-043)
src/settings/debug/schema.rs         backup schema page                                  (T-47b-045)
src/jobs/registry.rs                 job registry, shared with area:26, area:21, area:22 (T-47b-044)
src/util/clipboard.rs                copy and toast, truncate_center(50)                 (T-47b-046)
src/util/fmt_size.rs                 SI size formatter                                   (T-47b-039)
build.rs                             git SHA, build time                                 (T-47b-049)
proto/backup.proto                   protobuf schema, owned by area:21                   (T-47b-045)
```

Crates used by this area (versions from `p1-rust-design.md` section 7 and `p2-rust-design.md` section 1): `gtk4 0.11` (feature `v4_16`), `glib 0.22`, `tokio`, `tokio-rusqlite 0.6`, `rusqlite 0.32 (bundled)`, `async-channel 2`, `serde`, `serde_json`, `thiserror 2`, `tracing` and `tracing-subscriber` (reload handle), `zbus 5`, `ashpd 0.11`, `keyring 3`, `argon2 0.5`, `chrono 0.4`, `reqwest 0.12 (rustls)`, `hickory-resolver 0.24`, `directories 5`, `rustix 0.38` (statvfs), `icu_displaynames 2`, `url 2`, `flate2 1`, `prost 0.13`. Build tool only: `cargo-about`.

Replacements for Android APIs are tabulated in `p1-rust-design.md` section 8 and `p2-rust-design.md` section 6.

---

## 3. Reading guide

Read in this order: this overview, `p1-rust-design.md` (architecture), `p1-behavior.md` (what the screens do), `p1-tasks.md`; then the part 2 equivalents; the topic notes are deep dives referenced from both parts.

### 3.1 Part 1 (`T-47a-*`)

- [p1-inventory.md](p1-inventory.md): the 18 Kotlin files of part 1 with LOC, declared names, port decision, target module and task IDs.
- [p1-behavior.md](p1-behavior.md): OBSERVED behaviour of every part 1 screen row by row, with line references, plus restart versus live-apply table (section 12) and drops.
- [p1-data-and-config.md](p1-data-and-config.md): data models, enum stored values, every preference key with default, file and folder locations, network endpoints, exact SQL for the clear-database screens, CSV wire format, default category label list.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases, failure modes, races, CSV golden examples, test plan, and the known Android bugs and their Rust treatment (section 14).
- [p1-rust-design.md](p1-rust-design.md): crate and module layout, SQLite preference store, typed `Prefs`, settings tree model, threading model, background scheduling decision, crates with reasons, Android-to-Linux API mapping, app lock options, error handling.
- [p1-ui-parity.md](p1-ui-parity.md): layout, states, dialogs, gestures and what the mockup lacks, per screen.
- [p1-tasks.md](p1-tasks.md): the 98 `T-47a-*` tasks (lint: ok).
- [p1-topic-clear-database.md](p1-topic-clear-database.md): what "non-library" means, the recursive child algorithm for anime seasons, delete algorithm, test vectors.
- [p1-topic-scheduling-constraints.md](p1-topic-scheduling-constraints.md): what the library and backup schedule rows write, Android constraints, the Linux mapping options, toggle semantics.
- [p1-topic-settings-search.md](p1-topic-settings-search.md): search index contents, matching, ordering, breadcrumb format, navigation and highlight, test vectors.

### 3.2 Part 2 (`T-47b-*`)

- [p2-inventory.md](p2-inventory.md): the 18 Kotlin files of part 2 with LOC, declared names, port decision, target module and task IDs.
- [p2-behavior.md](p2-behavior.md): OBSERVED behaviour and Rust proposal for the router, app language, both repo screens, backup create and restore, storage, debug pages; drops and replacements (section 12); message keys (section 15).
- [p2-data-and-config.md](p2-data-and-config.md): preference keys, the `extension_store` and `extension_repos` tables, screen state types, backup file format, locale list, storage roots, build metadata, enums and bit-flag layouts.
- [p2-edge-cases-and-tests.md](p2-edge-cases-and-tests.md): edge cases per file, race conditions, numeric fixtures (size format, used fraction, repo URL check, backup filename, language order), test plan and failure modes.
- [p2-rust-design.md](p2-rust-design.md): crate list with reasons, module layout, key types (shell, language, repos, backup options, restore validation, storage, job registry), async model, mockup integration, Android-to-Linux mapping, icon names.
- [p2-ui-parity.md](p2-ui-parity.md): layout, states, dialogs, keyboard and mouse, accessibility, and mockup gaps per screen.
- [p2-tasks.md](p2-tasks.md): the 70 `T-47b-*` tasks (lint: ok).
- [p2-topic-backup-screens.md](p2-topic-backup-screens.md): create and restore flows as data, validation pseudocode, error card text, Kotlin behaviours kept and changed.
- [p2-topic-diagnostics-and-storage.md](p2-topic-diagnostics-and-storage.md): debug page map, worker info, schema, storage roots and numbers, date formatting, build metadata, drops.
- [p2-topic-extension-repos-screens.md](p2-topic-extension-repos-screens.md): state machines for the anime store page and manga repo page, shared dialog set, error catalogue, races.

### 3.3 Generated helper tables (dot-files)

- [.keys_table.md](.keys_table.md): generated table of every `AYMR.strings.*` key used by the in-scope files, with the file that uses it. Produced by [.keys_build.py](.keys_build.py), which scans the in-scope files from `areas.py`.
- [.prefs_table.md](.prefs_table.md): generated table of every preference read by the in-scope files: class, function, store call, stored key (with `__APP_STATE_` prefix where applicable), default expression. Produced by [.prefs_build.py](.prefs_build.py).

---

## 4. Key design decisions and open questions

Each item needs the user's call unless marked "Decided in this overview". Recommendation is the option the part documents already prefer, or the one this overview adopts.

| # | Question | Options | Recommendation |
|---|---|---|---|
| D-1 | Module root for settings: `src/settings/mod.rs` (part 1 proposal) or `src/settings.rs` plus children (part 2 proposal). | A: `mod.rs`; B: `settings.rs` plus children | **Decided in this overview: B** (the mockup already has `src/settings.rs`). Part 1 `settings/mod.rs` and `settings/dialogs.rs` paths are read as `src/settings.rs` and `src/settings/dialogs.rs`. Also `settings/search.rs` (part 1) and `settings/storage.rs` (part 2) remain separate files. No behaviour change. |
| D-2 | Background library updates and backup schedule (`T-47a-023`, `T-47a-024`, `T-47a-090`, decision in `p1-rust-design.md` section 6). | A: in-process tokio scheduler, runs only while the app is open, with optional autostart desktop entry. B: `systemd --user` timer `kurobayomi-update.timer` runs `kurobayomi --update-library` headless. | **A**, with the autostart row added only with explicit sign-off. Needs the user's decision. Blocks the schedule rows. |
| D-3 | Whether "Run in background" / autostart exists at all. | yes / no | Off by default. Needs sign-off. |
| D-4 | App lock method (`T-47a-063`, `T-47a-067`, `T-47a-068`, `p1-rust-design.md` section 9). | A: argon2id passphrase in keyring entry `kurobayomi-applock`; B: fprintd over D-Bus; C: PAM with login password. | **A**, with B as optional second method when a fingerprint device exists. Needs sign-off. |
| D-5 | Verbose logging applies live or after restart (`p1-behavior.md` section 12). | live via `tracing-subscriber` reload handle; restart | **Live** (Proposal in `p1-rust-design.md` section 7). |
| D-6 | Start screen applies at next launch only. | restart; live | Restart, with subtitle "Requires app restart to take effect". |
| D-7 | Dropped rows (13 P3 tasks, listed in section 6.4). | drop; keep | Drop, because they are Android-only. Confirm the list. |
| D-8 | Named app palettes (`T-47a-012`) and tablet UI mode (`T-47a-017`). | drop; keep | Drop. Accent colour from the desktop portal (`T-47a-013`) replaces dynamic colour. |
| D-9 | UI strings: `&'static str` constants (part 1) or gettext (`LANGUAGE`). | constants now; gettext now | Constants now (`p1-rust-design.md` section 7, "Not used: gettext"); gettext is decided with area:51-i18n. Needs the user's call on timing. |
| D-10 | Language display names. | `icu_displaynames` (native names, ICU data size); show tags only (ceiling) | `icu_displaynames`, unless binary size matters to the user. |
| D-11 | About screen links (Website, Discord, GitHub) point to Aniyomi repositories and channels in the Android source. | keep Aniyomi links; point to Kurobayomi; hide | Point to Kurobayomi's own repository; hide the Discord row until a Kurobayomi channel exists. Needs the user to supply URLs. |
| D-12 | Discord link present but empty string (`p2-edge-cases-and-tests.md` E22). | show button; show only when non-empty | Show only when non-empty (deviation, flagged in the edge-case list). |
| D-13 | Cutout row and custom ICC display profile (`T-47a-043`, `T-47a-083`). | drop; keep | Drop (no Linux equivalent in scope). |
| D-14 | Backup file format compatibility (`.tachibk`, gzip plus protobuf). | keep Aniyomi-compatible container; new format | Keep compatibility so Aniyomi backups restore and the reverse works. Owned by area:21-backup-format-and-create; this area consumes it. |
| D-15 | Known Android bugs (`p1-edge-cases-and-tests.md` section 14: B-1 to B-15). | fix in Rust; replicate | Fix in Rust (all listed as fixed or proposal). The migrations for shared keys B-3, B-4, B-5 belong to area:02 and area:03; confirm with those areas. |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Needed from | What | Used by tasks (examples) |
|---|---|---|
| area:01-foundation-app-bootstrap | app skeleton, config dirs | T-47a-001 |
| area:02-preferences-system | preference store contract, `UiPreferences` keys, `app_date_format` | T-47a-001, T-47a-016 |
| area:03-db-manga-schema-migrations | `prefs` table, `extension_repos`, manga clear-DB SQL (`mangas`, `chapters`), migrations for shared keys | T-47a-001, T-47a-075, T-47b-019, T-47b-022 |
| area:04-db-anime-schema-migrations | `extension_store`, anime clear-DB SQL (`animes`, `episodes`, season relations) | T-47b-009, T-47a-075 |
| area:08-domain-library-category-history-updates | category list and category service used by default category and include/exclude rows | T-47a-005, T-47a-020 |
| area:12-network-stack | `reqwest` client, DoH providers, user agent, cookie jar | T-47a-076, T-47a-077, T-47a-078 |
| area:13-core-util-storage-archive-torrent | `truncateCenter`, storage locations, cache sizes, chapter cache readable size | T-47a-088, T-47b-046, T-47b-037 |
| area:18-extensions-manga | repo create, replace, delete, update, fetch, fingerprint result types | T-47b-019, T-47b-020, T-47b-022 |
| area:19-extensions-anime | store add, subscribe, remove, update, fetch; extension list | T-47b-009, T-47b-011, T-47b-013 |
| area:20-extension-strategy-redesign | how extension lists refresh after store changes | T-47b-013 |
| area:21-backup-format-and-create, area:22-backup-restore | create and restore job APIs, options semantics, `.tachibk` container, restore validation | T-47b-027..036 |
| area:25-downloads-shared | download options and external downloader decision | T-47a-044..052 |
| area:26-library-update-jobs | scheduler, job registry, library update restrictions | T-47a-023, T-47a-024, T-47b-044 |
| area:27-tracking-core and area:28 to area:33 (trackers) | tracker login, OAuth callback, logout, credential keyring | T-47a-053..059 |
| area:34-images-covers-cache | cover refresh and image cache clear and size | T-47a-079, T-47a-088..092 |
| area:35-data-misc-updater-notification-export | update check, build metadata, updater row | T-47a-093..095, T-47a-096 |
| area:36-ui-shell-navigation-security-webview | router, `ui::push`, `ui::replace`, back stack, two-pane, deep links, webview data | T-47b-001..003, T-47a-003, T-47a-076 |
| area:37-ui-library, area:38-ui-entry-details-manga, area:40/41 (browse) | consumers of swipe actions and hide-in-library | T-47a-028, T-47a-060 |
| area:43-reader-core, area:44-reader-viewers | reader defaults and viewer keys (`ReadingMode`, `ReaderOrientation`, paged and long-strip settings) | T-47a-031..041 |
| area:47b-settings-widgets-search-more | row widgets, tri-state dialog, numeric chooser, search index engine | every T-47a task that renders rows |
| area:48-settings-player-and-scripts | player rows and their boundary with general settings | T-47a-003 (menu) |
| area:49-theme-and-design-system | theme mode, pure black CSS, accent colour | T-47a-011..014 |
| area:51-i18n | language list, strings, locale handling | T-47a-011, T-47b-005..008 |
| area:53-platform-manifest-intents-workers | `UriLauncher`, notification settings, workers | T-47a-056, T-47a-072, T-47b-044 |

### 5.2 What others need from this area

| Area | Needs | Provided by |
|---|---|---|
| area:02-preferences-system | the pref key and default catalogue (`p1-data-and-config.md` section 2) | Part 1 and `.prefs_table.md` |
| area:22-backup-restore | the Clear-database and backup entry points, restore confirmation text | T-47a-075, T-47b-031..036 |
| area:26-library-update-jobs | library update rows (interval, restrictions, categories) that set the job's inputs | T-47a-020..030 |
| area:36-ui-shell-navigation-security-webview | `SettingsScreen(destination)` ids (`About`=0, `DataAndStorage`=1, `Tracking`=2) and the search action | T-47b-001, T-47a-003 |
| area:43-reader-core, area:44-reader-viewers | reader default rows and enable/disable graph | T-47a-031..041 |
| area:47b-settings-widgets-search-more | `highlightKey` contract and search index shape | T-47a-004, T-47a-006 |
| area:49-theme-and-design-system | theme mode and accent settings | T-47a-011..013 |
| area:51-i18n | the list of settings UI strings | T-47a-* strings, T-47b-*, `p2-behavior.md` section 15 |
| area:53-platform-manifest-intents-workers | job and worker dump format | T-47b-043, T-47b-044 |
| area:27-tracking-core | login and logout rows and tracker config keys | T-47a-053..059 |

Cross-references seen in other areas' tasks that point at area:47 (examples): `T-06a-057`, `T-12a-031`, `T-15a-041`, `T-27a-043`, `T-27a-044`, `T-36a-028`, `T-43b-031`, `T-49a-062`, `T-51a-021`, `T-52a-058`. All use area:47 as the owner of the settings row or as the consumer of a setting.

---

## 6. Task summary

### 6.1 Counts

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, filtered to rows whose file starts with `47-settings-screens-general/` in `TASKS.csv`. Lint of `p1-tasks.md` and `p2-tasks.md`: `ok` (no errors).

| Priority | Effort S | Effort M | Effort L | Effort XL | Part 1 | Part 2 | Area total |
|---|---|---|---|---|---|---|---|
| P0 | 20 | 15 | 1 | 0 | 17 | 19 | 36 |
| P1 | 51 | 24 | 0 | 0 | 54 | 21 | 75 |
| P2 | 36 | 8 | 0 | 0 | 19 | 25 | 44 |
| P3 | 13 | 0 | 0 | 0 | 8 | 5 | 13 |
| Total | 120 | 47 | 1 | 0 | 98 | 70 | 168 |

The only L task is `T-47a-001` (preference store).

### 6.2 P0 critical path (in dependency order)

External prerequisites first; these must land before the area can be used daily:

1. area:01, area:02 (preference store contract), area:03 and area:04 (`prefs`, `extension_repos`, `extension_store` tables), area:36 (router and navigation), area:18 and area:19 (repo and store services), area:13 (`truncateCenter`, storage locations).

Within the area:

2. `T-47a-001` preference store (L). Blocks everything in part 1.
3. `T-47a-002` settings item model and row rules, then `T-47a-006` `SearchableSettings` contract.
4. `T-47a-005` category label helper, then `T-47a-020` default category for new manga and anime (needs area:08).
5. `T-47a-011` theme mode, `T-47a-003` settings main menu (needs area:36).
6. Reader group: `T-47a-033` top-level switches, `T-47a-034` display group, `T-47a-036` reading group, `T-47a-037` paged group, `T-47a-038` long-strip group, then `T-47a-041` enable/disable dependency graph (needs the reader areas).
7. `T-47a-028` chapter and episode swipe actions, `T-47a-060` hide entries already in library, `T-47a-031` default reading mode, `T-47a-088` storage location.
8. Part 2 router and toast: `T-47b-001` router, `T-47b-003` back navigation, `T-47b-052` toast overlay, `T-47b-058` mockup wiring.
9. Anime extension stores: `T-47b-009` DB and subscription, `T-47b-010` page state machine, `T-47b-011` add flow, `T-47b-012` refresh all, `T-47b-013` delete flow, then `T-47b-015` card list, `T-47b-016` add dialog, `T-47b-017` delete and deep-link dialogs, `T-47b-018` page chrome.
10. Manga extension repos: `T-47b-019` DB and reducer, `T-47b-020` create flow, `T-47b-022` refresh and delete, `T-47b-023` page and card list, `T-47b-024` add dialog, `T-47b-026` repo events as toasts (needs `T-47b-052`).

### 6.3 P1 and P2 scope in one line each

- P1 (75): the parity rows most users rely on. Downloads rows (`T-47a-044..050`), tracking rows and login (`T-47a-053..059`), Advanced network, user agent, DoH, cookies and WebView clearing (`T-47a-076..078`), library covers refresh, reset of per-series reader settings (`T-47a-079..080`), automatic library update and backup schedules (`T-47a-023`, `T-47a-024`, `T-47a-090`), both clear-database screens (`T-47a-075`), About updater (`T-47a-093..094`), create and restore backup pages, storage bars, settings search, extension store and repo follow-ups, and the main menu two-pane layout. Exact membership is in `p1-tasks.md` and `p2-tasks.md`.
- P2 (44): dynamic accent (`T-47a-013`), pure black (`T-47a-014`), sidebar navigation style (`T-47a-018`), e-ink flash group (`T-47a-035`), reserved colour and brightness keys (`T-47a-042`), app lock sub-rows on Linux (`T-47a-063`, `T-47a-064`, `T-47a-067`, `T-47a-068`), onboarding and notification links (`T-47a-071`, `T-47a-072`), hardware bitmap threshold and legacy long-strip decoder (`T-47a-081`, `T-47a-082`), Data Saver (`T-47a-086`, `T-47a-087`), CSV export (`T-47a-091`), licence screens (`T-47a-096`, `T-47a-097`), the part 2 language picker (`T-47b-005..008`), debug, worker and schema pages, edge-case decisions, search over in-scope pages (`T-47b-071`), and accessibility (`T-47b-059`).

### 6.4 P3 drops and replacements (Android-only)

| Task | Drop |
|---|---|
| T-47a-012 | 17 named app palettes |
| T-47a-017 | Tablet UI mode |
| T-47a-043 | Show content in cutout area |
| T-47a-051 | External downloader selector (IDM, ADM) |
| T-47a-066 | Secure-screen setting (FLAG_SECURE; no Wayland or X11 equivalent) |
| T-47a-073 | Battery optimisation and "Don't kill my app" rows |
| T-47a-083 | Custom display profile (ICC) |
| T-47a-085 | Extension installer choice (Shizuku) |
| T-47b-035 | MIUI optimisation banner |
| T-47b-042 | ART profile compilation row |
| T-47b-051 | OneUI and MIUI rows |
| T-47b-054 | Android persistable URI permission |
| T-47b-056 | List item animations (`animateItem()`) |

User loss for each is stated in `p2-behavior.md` section 12 and `p1-behavior.md` sections 10 and 12.

### 6.5 Risks

1. Blocking prerequisites in other areas (area:02, area:03, area:04, area:18, area:19, area:36). The area cannot show real values until these exist. Mitigation: the store and row model (`T-47a-001`, `T-47a-002`) can be built and tested with the in-memory store.
2. Scheduler decision (D-2) and app lock decision (D-4) block tasks in part 1. Mitigation: ship the menu and the other rows; hold the blocked rows behind the decision.
3. Destructive actions. Clear-database deletes rows across `mangas`, `chapters`, `animes`, `episodes`. A wrong SQL predicate can delete library data. Mitigation: confirmation dialog with counts, and tests from `p1-topic-clear-database.md` section 6 before enabling delete.
4. Restart versus live semantics (D-5, D-6). If a row's subtitle and behaviour disagree, the user sees stale state. Mitigation: `p1-behavior.md` section 12 is the single table.
5. Reader enable/disable graph (`T-47a-041`). Several reader rows depend on other rows. Errors here hide working rows. Mitigation: the graph is pure data and is unit-tested against `p1-edge-cases-and-tests.md` section 4.
6. Keyring availability on KDE and fallback (`T-47a-059`, `T-47a-063`). Mitigation: the decision in `p1-rust-design.md` section 7.
7. GTK re-entrancy when a `set` inside a change handler rebuilds rows (`p1-rust-design.md` section 12, pitfall 3). Mitigation: rebuild flag and rebuild only changed rows.
8. Naming: About and window text must say Kurobayomi, not Aniyomi (D-11, `p1-ui-parity.md` section 18 item 13).

---

## 7. Definition of done for this area in Kurobayomi

Observable behaviour, testable on the desktop:

1. Settings opens from the main menu. The menu lists the eleven top-level entries defined in `SettingsMainScreen.kt`. In a wide window it shows a two-pane layout; in a narrow window it shows one page at a time with back navigation.
2. `Ctrl+F` opens settings search. Typing a row name finds the row with its breadcrumb. Activating a result opens the page, scrolls to the row and highlights it; the highlight clears after the first screen consumes it.
3. Every switch, list, multi, slider and text row shows the stored value, writes on change, and keeps the value after restart. Defaults match `p1-data-and-config.md` section 2.
4. Theme mode changes apply immediately. Pure black applies immediately (live CSS swap). Start screen applies on next launch and shows its restart subtitle.
5. Reader rows apply to new reader sessions and follow the enable/disable graph: a disabled row is hidden or greyed exactly as in `p1-ui-parity.md` section 6.
6. Browse: hide-in-library switches filter browse lists; extension repo rows show counts; the NSFW gate changes listings immediately.
7. Anime extension stores page: lists stores from `extension_store`, adds a store by index URL, refreshes all, deletes with confirmation, and handles a deep link through the confirm dialog. Loading and error states are visible.
8. Manga extension repos page: lists repos from `extension_repos`, adds a valid repo, shows a toast for an invalid URL, shows the conflict dialog for a duplicate fingerprint, and deletes with confirmation.
9. Create backup: options match `BackupOptions` (`p2-behavior.md` section 5); a save dialog picks the `.tachibk` path; a second start while a job runs is refused with a toast.
10. Restore backup: picking a file validates it asynchronously; missing sources and trackers are listed; invalid files show the error card text; restore starts once.
11. Data and storage: each storage root shows a `statvfs` usage bar; cache size shows "Calculating…" until computed; storage location picker rejects non-local paths with `Invalid location: <uri>`; CSV export writes the golden format from `p1-edge-cases-and-tests.md` section 12.
12. Clear database (manga and anime): lists non-library sources with counts, select all, invert, select inverse (fixed label), and deletes selected only after confirmation; double click cannot start a second delete.
13. Advanced: crash log export, verbose logging live toggle, network and DoH rows (live), library and reader rows. Dropped rows (section 6.4) are absent.
14. Security: app lock uses the chosen method (D-4); unlock fails closed; the NSFW and notification-content rows apply live.
15. Tracking: seven tracker rows; login opens the system browser for OAuth; a failed login does not log out an existing session (known bug B-12 in `p1-edge-cases-and-tests.md` section 14); credentials go to the keyring.
16. About: shows version and build time, runs the update check on demand, opens the links per D-11; the licences page lists every embedded library.
17. Debug info, worker info and backup schema pages show real values with the Linux rows (`p2-behavior.md` section 8).
18. Language page lists locales from the embedded list, applies the choice, and shows the restart toast.
19. Every in-scope task's acceptance criteria in `p1-tasks.md` and `p2-tasks.md` passes its named test in `p1-edge-cases-and-tests.md` section 13 or `p2-edge-cases-and-tests.md` section 4.
20. No Android-only string, row or dialog from the 36 Kotlin files is visible in the Kurobayomi UI.

---

## 8. Changes made while writing this overview

- `p2-tasks.md` line 658: the prose reference `area:13` was expanded to `area:13-core-util-storage-archive-torrent` so that every area token in the area's task files is a real area id. No task IDs changed.
- Lint of `p1-tasks.md` and `p2-tasks.md` was run before and after: both `ok`.
- `TASKS.csv` was regenerated by `tasks.py export` (3,801 tasks, 0 errors).
