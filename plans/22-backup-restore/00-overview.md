# 22-backup-restore — Area overview

Area lead overview for the restore half of Aniyomi's backup system. Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Target crate: `/home/blackbox/kurobayomi` (crate `kurobayomi`, edition 2024, gtk4 0.11.5 with `v4_16`, no libadwaita).

Labels: **OBSERVED** = what the Kotlin does (verified in source). **Proposal:** = what Kurobayomi should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Restore reads a `.tachibk` file (gzip-compressed protobuf, or a raw protobuf, or a 0.x legacy file) produced by Aniyomi's backup creator and writes its contents into the local SQLite database. The user picks options (seven booleans, see §1.2 of [p1-behavior.md](p1-behavior.md)), then the restore runs in a WorkManager job (`BackupRestoreJob`) with a foreground notification, progress counters, and an error log file for entries that failed.

Restore covers:
- Library entries: manga (`BackupManga`, chapters, history, tracks, categories, excluded scanlators, fetch interval) and anime (`BackupAnime`, seasons, episodes, history, tracks, categories).
- Categories for both kinds (by name, with the categorised-display flag).
- App preferences and per-source preferences, with type gating and category-id remapping.
- Extension stores (anime), extension repos (manga), custom player buttons.
- APK extension payloads (counted only; see the drop decision in §4).

Source of truth for behaviour: [p1-behavior.md](p1-behavior.md) (§1 to §14). Wire and DB details: [p1-data-and-config.md](p1-data-and-config.md).

Files in scope: 24 files (2,536 LOC): 15 under `app/`, 9 under `core-metadata/`. Full list with per-file decisions in [p1-inventory.md](p1-inventory.md).

### 1.2 Why it matters for a usable Kurobayomi

- A user who moves from Aniyomi on Android to Kurobayomi on Linux needs the library, read and watch positions, categories, and tracker links to arrive intact. Without restore the migration path is empty.
- Restore is also the only way to exercise the wire-format decoder and the DB write paths end to end. Those paths are shared with area:21 (create), so a round-trip test (Q-21) protects both.
- Backups are the user's only safety net against DB loss. The restore code has to be correct about merge rules, or a restore silently overwrites reading progress.

### 1.3 Explicit non-goals (per area brief)

- Creating backups (area:21-backup-format-and-create).
- Decoding and wire models (area:21-backup-format-and-create). Restore calls them; it does not own them.
- Installing APK extensions (dropped, §4).
- Restoring Gradle or R8 plumbing (`core-metadata` build files are dropped).

---

## 2. Module map

### 2.1 Kotlin packages (OBSERVED)

| Package / path | Responsibility | Owner |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt` | gzip sniff, protobuf decode | area:21 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDetector.kt` | legacy 0.x detection | area:21 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt` | missing sources and trackers | area:22 (logic), area:21 (file) |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt` | restore and create notification builders | shared (21/22) |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/*.kt` | wire models (`Backup`, `LegacyBackup`, `BackupManga`, `BackupAnime`, and friends) | area:21 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/*` | create job, options, creators | area:21 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt` | WorkManager worker, job start, stop, isRunning | area:22 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt` | orchestration, sections, error collection, error log | area:22 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/RestoreOptions.kt` | seven flags, labels, BooleanArray codec | area:22 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/*.kt` | nine restorers plus `PreferenceRestorer` | area:22 |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt` | restore screen and screen model | area:22 |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt` | create screen (UI only) | area:21 (screen is in this area's inventory) |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt` | per-root free and total space | area:22 |
| `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt` | ComicInfo.xml model and mappings | area:22 |
| `core-metadata/src/main/java/tachiyomi/core/metadata/tachiyomi/*Details.kt` | details.json models (local source) | area:22 |
| `core-metadata/build.gradle.kts`, `consumer-rules.pro`, `proguard-rules.pro`, `src/main/AndroidManifest.xml` | module plumbing | dropped |

### 2.2 Restorers (OBSERVED, inside `restore/restorers/`)

| Class | Restores | Lines (approx.) |
|---|---|---|
| `MangaRestorer` | manga entry, chapters, categories, history, tracks, excluded scanlators, fetch interval | 437 |
| `AnimeRestorer` | anime entry, seasons (via parent pointer), episodes, history, tracks, categories | 455 |
| `MangaCategoriesRestorer` | manga categories by name, categorised-display flag | 42 |
| `AnimeCategoriesRestorer` | anime categories by name, categorised-display flag | 42 |
| `PreferenceRestorer` | app preferences, per-source preferences, category-id remap, scheduler re-arm | 155 |
| `MangaExtensionRepoRestorer` | manga extension repos (duplicates are an error) | 38 |
| `AnimeExtensionStoreRestorer` | anime extension store (upsert) | 25 |
| `CustomButtonRestorer` | custom player buttons | 43 |
| `ExtensionsRestorer` | APK payloads (dropped; counted as skipped) | 32 |

### 2.3 Proposed Rust module map (PROPOSAL, from [p1-rust-design.md](p1-rust-design.md) §1)

```
src/
  backup/
    mod.rs              pub use + module docs
    options.rs          RestoreOptions (7 bools), labels, defaults, bit array (de)serialization
    format.rs           (area:21) protobuf structs, gzip sniff, decode()
    validate.rs         ValidationResult { missing_sources, missing_trackers }; validate(path)
    error_log.rs        ErrorLog { entries }; write(dir) -> Option<PathBuf>
    restore/
      mod.rs            RestoreRequest, RestoreEvent, RestoreSummary, RestoreError, run(); section order
      ctx.rs            RestoreCtx { conn, cancel, events, amount, progress, now, fetch_window, source_names, errors }
      categories.rs     restore_categories(tx, kind, backup) for manga and anime
      manga.rs          restore_manga_entry(tx, ctx, entry, category_map)
      anime.rs          restore_anime_entry(tx, ctx, entry, seasons, category_map, backup_to_db)
      merge.rs          chapter/episode merge and track merge helpers
      prefs.rs          app and source preferences, category-id remap, PrefStore trait
      extensions.rs     anime stores, manga repos, custom buttons, APK skip counter
    job.rs              RunningRestore, JobSlot (single-job guard), start(), stop()
    ui/
      restore_page.rs   GTK page: options, validation card, action bar
      create_page.rs    GTK page for create (area:21 pieces)
      progress.rs       in-window banner: Revealer, title, item label, ProgressBar, Cancel
  metadata/
    comicinfo.rs        ComicInfo model, quick-xml read and write, copy mappings
    details.rs          MangaDetails, AnimeDetails, ChapterDetails, EpisodeDetails (serde)
  storage_info.rs       statvfs rows for the storage section
```

Crate choices (from [p1-rust-design.md](p1-rust-design.md) §2): `rusqlite`, `prost`, `flate2`, `quick-xml` (serialize feature), `chrono`, `async-channel`, `thiserror`, `log`, `rustix` (statvfs), `glib::user_cache_dir()`. Not used: tokio for restore (restore is synchronous), libadwaita (forbidden by the brief).

Threading (PROPOSAL, [p1-rust-design.md](p1-rust-design.md) §4): one `std::thread` per restore, one owned SQLite connection, one `rusqlite` transaction per entry (Immediate behaviour), events over `async_channel` to the GTK main context through `glib::spawn_future_local`.

---

## 3. Reading guide

Read in this order when implementing.

1. [p1-inventory.md](p1-inventory.md): every in-scope file with declarations, port decision, target module, and task ids.
2. [p1-behavior.md](p1-behavior.md): the full observed behaviour: flow (§1), options (§2), decode and validation (§3), restorer algorithms (§4), preference restore (§5), concurrency (§6), error log and cancel (§7), notifications (§8), create screen interaction (§9), dropped extension path (§10), storage (§11), message keys (§12), defect list D1 to D24 (§13).
3. [p1-data-and-config.md](p1-data-and-config.md): wire models (§1 and §2), wire-format rules for the decoder (§3), domain mappings (§4), DB tables and columns written by restore (§5), preference keys and defaults (§6), files on disk (§7), network endpoints (§8), WorkManager names (§9), notification ids (§10), bit layouts (§11), output text (§12).
4. [p1-rust-design.md](p1-rust-design.md): proposed Rust layout, types, signatures, threading, pipeline, cancellation, GTK UI, notifications, scheduler re-arm, Android-to-Linux mapping, lifecycle, pitfalls.
5. [p1-topic-merge-rules.md](p1-topic-merge-rules.md): decision tables for entries, chapters, episodes, history, tracks, excluded scanlators, categories, custom buttons, extension stores and repos, preferences, and the per-run decision order.
6. [p1-topic-seasons-and-legacy.md](p1-topic-seasons-and-legacy.md): anime seasons (the D7 and D18 defects and the Kurobayomi decision), and legacy 0.x backups (detection, conversion, decision).
7. [p1-topic-protobuf-decode.md](p1-topic-protobuf-decode.md): decode pipeline, the prost rules restore depends on, the sealed `PreferenceValue` wire form, legacy conversion details, size and performance.
8. [p1-ui-parity.md](p1-ui-parity.md): restore and create screen parity with the Kotlin screens, error card variants, states, interactions, mockup gaps, notifications, dialogs, keyboard, responsive rules.
9. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases E-01 to E-67 (input, validation, semantics, preferences, extension stores, custom buttons), race and failure modes, test cases X-nn, fixtures Q-01 to Q-21.
10. [p1-tasks.md](p1-tasks.md): 53 implementation tasks T-22a-001 to T-22a-053 with priority, effort, dependencies, sources, and acceptance criteria.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the docs (Proposal, keep unless the user objects)

| Topic | Kotlin behaviour (defect id) | Kurobayomi decision | Where |
|---|---|---|---|
| Sections run in order (D3) | unjoined `launch` children race | sequential sections; categories before entries | rust-design §5 |
| Cancel is not an entry error (D4, D17) | cancel recorded as entry error; log skipped on cancel | `RestoreError::Cancelled` re-raised; error log written on every exit path | rust-design §6 |
| Progress amount (D2) | categories add 2, progress adds 1 | each emitted step matches the amount exactly | tasks T-22a-006 |
| Anime source names (D1) | anime errors show numeric id | `SourceNames` maps for both kinds | rust-design §3.3 |
| Seasons (D7, D18) | seasons restored twice; parent pointer overwritten | pre-pass; children restored inside the parent transaction; orphans get `parent_id = NULL` | seasons-and-legacy §1.4 |
| `initialized` on insert (D6) | persists the backup value | persists the computed value | tasks T-22a-014 |
| Custom button favourite (D10) | only one check, so multiple favourites all win | one favourite per restore; the rule is kept, the count is fixed | tasks T-22a-028 |
| Manga repo duplicate (D11) | identical repo always errors | identical repo is a no-op, not an error | tasks T-22a-027 |
| Category string sets (D12) | appended on every restore | replaced on every restore | tasks T-22a-009 |
| Error log write failure (D15) | silent | `log::warn!` | rust-design §3.11 |
| Error text (D13, D19) | `null` literal; `java.lang.IllegalStateException:` prefix | show the inner message only | tasks T-22a-043 |
| Storage bar (D14) | not clamped | clamped to 0.0..=1.0 | rust-design §7.5 |
| Extension section label (D24) | reuses `source_settings` label | label "Extensions" | behaviour §13 |
| APK extensions (T-22a-029) | install through Android installer | dropped; counted as skipped | §4.3 below |
| MIUI warning (T-22a-039) | Android device helper | dropped | tasks |
| Dead sync mode (D21, T-22a-045) | `isSync` never set true | removed | tasks |
| Restore job not persisted across app restart | WorkManager persists the job | accepted loss; a restore is interactive | rust-design §10 |
| Notifications | `NotificationCompat` progress with channel and repost each item | GNotification with throttled progress (at most one update every 500 ms, plus final 100 percent); in-window `gtk::ProgressBar` | rust-design §4, §8 |
| Timestamps in error log | `SimpleDateFormat(Locale.getDefault())` | `chrono` with ASCII digits | rust-design §3.11 |
| Paths | Kotlin `Uri` strings | `PathBuf` only, lossy display | edge E-08 |

### 4.2 Decisions that need the user's call

Each item lists the choice and a recommendation. Nothing here is implemented until the user answers.

1. **Asymmetric metadata merge (D5).** Kotlin: when the backup is newer, DB keeps title, viewer flags, chapter flags, update strategy, date added and last update; when older or equal, the backup overwrites them. Choice: keep Aniyomi's rule (parity, no surprise for a migrating user) or make it symmetric (newest-wins by `lastModifiedAt`). Recommendation: **keep** (documented in T-22a-012 and E-23); it never downgrades a newer local edit.
2. **Progress regression (D8).** Kotlin overwrites `lastPageRead` and `lastSecondSeen` with the backup value even when the DB value is further along. Choice: keep or take the maximum. Recommendation: **take the maximum** (the doc's PROPOSAL, E-29, E-30). A restore should not move a reader backward.
3. **Duplicate category names in one backup (E-16, Q-10).** Kotlin creates duplicate DB rows with the same name. Choice: keep duplicates or merge by name. Recommendation: **merge by name** within the restore (PROPOSAL, X-103).
4. **Unknown and internal preference keys (E-60, behaviour §5.5).** Kotlin writes every key with a matching type, including `__APP_STATE_*`. Choice: write all keys, or only keys in the Kurobayomi schema and count the rest as skipped. Recommendation: **allowlist** (T-22a-052). It also stops a backup from overwriting private app state such as `last_auto_backup_timestamp`.
5. **Text and JSON input (Q-08, Q-09, E-04, E-05).** Kotlin decodes a plain text file to an empty backup without error. Choice: reject non-protobuf input with an error, or accept it as an empty restore. Recommendation: **reject** with "Invalid backup file" (the text in behaviour §12).
6. **Favourites on custom buttons (D10, E-66, Q-16).** Choice: keep "all favourites" (Kotlin) or "one favourite" (PROPOSAL). Recommendation: **one favourite** (the Kotlin rule is clearly a bug in the count).
7. **Close the window during a restore (T-22a-046).** Choice: cancel and close, or keep running in the background. The desktop app has no background service. Recommendation: **confirmation dialog** with "Keep running" and "Cancel restore and close" (rust-design §11). The restore is not resumable, but each entry is atomic.
8. **Legacy 0.x backups (T-22a-050, seasons-and-legacy §2).** Choice: support or drop. Recommendation: **support** (the conversion is small and keeps very old backups usable).
9. **Orphan seasons (E-44).** Choice: keep raw parent id (Kotlin) or set `parent_id = NULL`. Recommendation: **NULL** (a raw id that matches nothing is a dangling pointer).
10. **Chapter float rounding (E-27).** Choice: store the f32-converted value as Kotlin does, or round to six decimals. Recommendation: **round to six decimals**, documented in the test plan.
11. **Restore when sources or trackers are missing (E-12).** Choice: block restore or allow it with a warning card. Recommendation: **allow** (Kotlin behaviour; the card tells the user what to install or log into).

### 4.3 Dropped features (user loses)

- **APK extension payloads.** Kotlin writes each `BackupExtension` APK to cache and opens the Android package installer (`ExtensionsRestorer.kt:82-99`). Kurobayomi extensions are not APKs (area:20-extension-strategy-redesign). Loss: an Aniyomi backup's installed-extension list cannot be re-installed; the restore reports the count as skipped (Q-19, T-22a-029). The user must reinstall extensions from the Kurobayomi extension store.
- **MIUI warning.** Android-only device check. Loss: none.
- **Gradle and R8 files in `core-metadata/`.** Build plumbing. Loss: none. The Cargo manifest replaces them (T-22a-053).

### 4.4 Defect citations that need a line-number audit before implementation

Spot checks against `/home/blackbox/aniyomi-src` found that some line citations in [p1-behavior.md](p1-behavior.md) §13 do not match the file:

- **D1** cites `BackupRestorer.kt:196`. The defect itself holds: `mangaSourceMapping` is assigned from the anime list at `BackupRestorer.kt:87` and from the manga list at `:89`, and `animeSourceMapping` is only read (`:173`, `:193`). Use the cited lines `:87`, `:89`, `:173`, `:193`.
- **D11** cites `MangaExtensionRepoRestorer.kt:122-125`. That file is 38 lines long. The "same signing key fingerprint" error is at `restorers/MangaExtensionRepoRestorer.kt:25`.
- **D14** cites `StorageInfo.kt:214`. That file is 75 lines long. Re-derive the line for the unclamped progress value.
- **D21** cites `BackupRestoreJob.kt:38`. Verified: the `isSync` read is at that line.

Recommendation: a one-pass citation fix in p1-behavior.md §13 before any implementer relies on the line numbers. The defect descriptions are not in doubt.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What restore needs | Used by |
|---|---|---|
| area:21-backup-format-and-create | `BackupDecoder` (gzip sniff, protobuf decode), `BackupDetector` (legacy), `BackupFileValidator` file reading, wire models, legacy conversion, `BackupNotifier` shared builders, the create page and create job for the round trip | T-22a-001, 003, 010, 034, 041, 050 |
| area:02-preferences-system | preference store backend (type names per key, set, string-set replace), key schema, `source_<id>` per-source stores, hide-notification-content preference, settings entry for "Restore backup" | T-22a-008, 009, 049, 052, 053 |
| area:03-db-manga-schema-migrations | `mangas`, `chapters`, `history`, categories and excluded-scanlator tables and columns (SQL in p1-data-and-config.md §5) | T-22a-011, 013, 014, 015 |
| area:04-db-anime-schema-migrations | `animes`, `episodes`, `animehistory`, `parentId` column for seasons | T-22a-022, 024 |
| area:05-data-repos-entries-items | repository lookups and write paths for categories and entries | T-22a-008 |
| area:06-data-repos-history-track-source-misc | transaction semantics and history and track write paths | T-22a-018, 019, 025 |
| area:07-domain-entries-items | `Chapter`, `Episode` domain defaults and copy rules | T-22a-023, 024 |
| area:08-domain-library-category-history-updates | fetch window (`MangaFetchInterval`), next-update calculation, library and auto-backup scheduler re-arm | T-22a-010, 021 |
| area:09-domain-source-track-download-release-misc | track model and merge scope; source model | T-22a-004, 019 |
| area:14-source-local-and-metadata | ComicInfo and details.json semantics for local source | T-22a-036, 038 |
| area:17-source-app-layer | source existence lookups and stub names (`Name (id)`) for validation | T-22a-004 |
| area:18-extensions-manga | manga extension repo table and signing fingerprint rules | T-22a-027 |
| area:19-extensions-anime | anime extension store table and upsert semantics | T-22a-026 |
| area:20-extension-strategy-redesign | the reason APK payloads are dropped and what replaces them | T-22a-029 |
| area:27-tracking-core | tracker registry and "logged in" state for the validation card (added as a dependency of T-22a-004 in this pass) | T-22a-004 |

### 5.2 What others need from this area

| Area | What it needs from area:22 |
|---|---|
| area:21-backup-format-and-create | restore fixtures and expected counts (Q-01 to Q-04), the round-trip check (Q-21, X-112), the shared notification ids and the error-log path convention |
| area:02-preferences-system | the restore writes its own preference values (`__APP_STATE_*` allowlist decision) and the per-source store contract |
| area:36-ui-shell-navigation-security-webview | the window close hook and the progress banner host (main window), the app actions `cancel-restore` and `open-error-log` registered in `src/main.rs` (rust-design §8, §11) |
| area:08-domain-library-category-history-updates | the signal that a restore finished so library views refresh (rust-design §4) |

---

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `22-backup-restore/`)

Total: **53 tasks**. Priority: P0 0, P1 40, P2 9, P3 4. Effort: S 36, M 17, L 0, XL 0.

| Priority | S | M | L | XL | Total | Task ids |
|---|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 | none (restore is parity, not required for first daily use) |
| P1 | 24 | 16 | 0 | 0 | 40 | S: 002, 004, 006, 008, 010, 011, 014, 016, 017, 020, 021, 025, 026, 027, 028, 030, 031, 032, 040, 044, 047, 048, 050, 051. M: 001, 003, 005, 007, 009, 012, 013, 015, 018, 019, 022, 023, 024, 033, 034, 042 |
| P2 | 8 | 1 | 0 | 0 | 9 | S: 035, 037, 038, 041, 043, 046, 049, 052. M: 036 |
| P3 | 4 | 0 | 0 | 0 | 4 | S: 029, 039, 045, 053 |

Task ids above are the numeric suffix of `T-22a-<nnn>`.

Per-task priority and effort are in `TASKS.csv` (regenerate with `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`).

Sizing upper bound from the effort definitions (S ≤ 1 day, M ≤ 3 days): 36 × 1 + 17 × 3 = **87 person-days** at most. This is a ceiling, not an estimate.

### 6.2 Priority P0

None. Restore is migration parity, not a requirement for reading or watching. Its P1 tasks are the first ones to schedule once the library, DB and reader areas are usable.

### 6.3 Critical path (P1, in order)

1. T-22a-051 (wire fixtures), T-22a-044 (mockup wiring), T-22a-048 (decision record): no dependencies; start first.
2. T-22a-002 (RestoreOptions) → T-22a-006 (progress) → T-22a-007 (ordered sections, D3).
3. T-22a-011 (manga sort by new) → T-22a-012 (existing merge) → T-22a-013 (UPDATE) → T-22a-014 (INSERT, D6) → T-22a-015 (chapter merge) → T-22a-016 (chapter writes).
4. T-22a-022 (anime sort and merge, depends on 012) → T-22a-024 (episode merge) → T-22a-023 (seasons, D7 and D18).
5. T-22a-005 (job runner, depends on 006 and 007) → T-22a-040 (in-progress guards, depends on 005) → T-22a-033 (restore page UI, depends on 001, 002, 004, 040).
6. T-22a-030 (error collection and log file, depends on 005) → T-22a-031 (completion notification, depends on 030 and 046) → T-22a-032 (cancel semantics, depends on 005).
7. T-22a-042 (test suite and fixtures, depends on 003, 007, 012, 015, 022, 023).

Parallel tracks that do not block the path: categories (T-22a-008), preferences (T-22a-009, 010), extension stores and repos (T-22a-026, 027), custom buttons (T-22a-028), validation (T-22a-004), decode (T-22a-003, area:21).

### 6.4 Risks

1. **Dependency inversion.** T-22a-031 (P1) depends on T-22a-046 (P2, close-while-running). The completion notification should not wait for the close-confirmation work. Either raise T-22a-046 to P1 or drop its edge from T-22a-031 when the notification is built.
2. **Schema coupling.** The SQL in p1-data-and-config.md §5 is copied from the Kotlin `.sq` files. If area:03 or area:04 rename columns, restore breaks silently. Mitigation: write queries against the Kurobayomi schema and let the round-trip test catch drift (rust-design §12 item 2).
3. **Missing upstream tests.** The Aniyomi source has no restore tests (edge file header). Golden comparison (Q-01 to Q-04) needs one run of the Kotlin restore in an emulator. Plan for that effort outside the area's sizing.
4. **Byte-level wire fixtures.** T-22a-051 needs byte-for-byte captures from the Kotlin app for the sealed `PreferenceValue` and enum encodings. Without them the decoder is guessed. Owner must have an Android build or a captured sample.
5. **Behaviour changes.** These deliberate deviations change results compared with Aniyomi: sequential sections (D3), season parent fix (D7), maximum progress (D8), single custom-button favourite (D10), no-op identical repo (D11), replaced category string sets (D12), merged duplicate category names (E-16), `NULL` parent for orphan seasons (E-44), preference allowlist (E-60), rejected text input (Q-08). Each needs an entry in the decision record (T-22a-048). Users migrating from Aniyomi may see different counts.
6. **Line citations.** See §4.4. Wrong line numbers cost an implementer time, not correctness, but they should be fixed first.
7. **Cancel during a write.** Entry transactions are atomic, but a cancel during a multi-entry section leaves a partial section. Acceptable per rust-design §11; the next restore is idempotent by the merge rules.
8. **Locale and encoding.** Non-ASCII, newline and invalid-UTF-8 paths (E-08). Use `PathBuf` everywhere (T-22a-047).

---

## 7. Definition of done for area 22 in Kurobayomi

Each item is an observable behaviour a tester can check on the desktop app.

1. **Entry point.** The "Restore backup" action in Settings > Data opens a GTK file dialog. Choosing a `.tachibk` file (any name, gzip or raw) opens the restore page. Cancelling the dialog changes nothing.
2. **Validation card.** The page shows "Loading" until validation finishes. A missing source or tracker shows the card text from behaviour §12 with sorted, distinct names. An unreadable file shows "Invalid backup file:" with the path, a blank line, and "Full error:" with the message. The message is never the literal `null` (D13) and never starts with `java.lang.` (D19).
3. **Options.** Seven checkboxes with the labels from behaviour §2 and the defaults from rust-design §3.1 (extensions off). The Restore button is disabled when every option is off. Restore is disabled when validation says `canRestore: false`.
4. **Single job.** Starting a second restore while one runs shows "Restore is already in progress" and does not open the dialog. The auto-backup timer skips its tick during a restore.
5. **Progress.** The in-window banner shows a title, the current item, and a bar that reaches exactly 100 percent at completion. The desktop notification shows "{done} / {total}: {title}" and is withdrawn before the completion notification.
6. **Cancel.** Cancel stops at the next entry boundary within one second for a typical backup. Entries already committed stay. The error log is written with the errors collected so far, and the completion notification says "Canceled restore" (D4, D17).
7. **Merge rules.** Restoring the same file twice changes no entry, chapter, episode, history or track row (test X-102). Preference string sets for categories are replaced, not appended (D12).
8. **Reading progress is not reduced.** A chapter or episode with a higher progress in the DB keeps it after restore (D8 fix). A bookmark set in either side stays set (OR).
9. **Metadata.** For a manga or anime where the backup is newer, the DB keeps the fields listed in merge rules §1 (D5 kept). Title and scanlator of an existing chapter stay as in the DB (D16).
10. **Favourite default.** A manga whose `favorite` field is absent is restored as a library entry (D22).
11. **Seasons.** Every season's `parent_id` is the database id of its parent in the same restore, regardless of order in the file (D7 fix). A season with no parent in the file is top-level with no parent (E-44).
12. **Categories.** Missing categories are created with the backup name, order and flags. Duplicate names are merged by name (E-16 decision). Entries with no category membership are not moved.
13. **Extension stores and repos.** Anime stores upsert. An identical manga repo is a no-op, not an error (D11). A repo with the same URL and a different fingerprint, or the same fingerprint and a different URL, is an error line with the text from behaviour §12.
14. **Custom buttons.** Names already present are skipped. At most one restored button becomes favourite (D10).
15. **APK extensions.** When the extensions option is on, each APK payload is counted and listed as skipped. The completion text reports the skipped count. No install is attempted.
16. **Error log.** Written to `glib::user_cache_dir()/kurobayomi/aniyomi_restore_error.txt` (same file name, per rust-design §3.11) on Finished, Canceled and Failed. Each line is `[yyyy-MM-dd HH:mm:ss.SSS] message`. The completion notification's "Tap to see details" opens the file with `gtk::UriLauncher`. A write failure produces a `log::warn!` line and no UI error.
17. **Notifications.** Progress, completion (no errors, with errors, canceled, failed) match the table in rust-design §8, with plurals for one and many errors.
18. **Legacy file.** A 0.x backup (T-22a-050 decision) restores its anime, manga, categories and anime sources (Q-02 counts) and creates no season rows.
19. **Preferences.** App preferences are written only for keys in the allowlist (E-60 decision). Scheduler re-arm runs once after app preferences are written. Source preferences are written to `source_<id>` even when the source is not installed (E-52).
20. **Storage info.** The storage section lists the home mount and each mounted external disk with a bar clamped to 0 to 100 percent, and a caption `Available: {avail} / Total: {total}` in SI units (D14).
21. **Close during restore.** Closing the window during a restore asks first. "Keep running" returns to the app; "Cancel restore and close" stops at the next check point and then closes.
22. **Round trip.** A backup created by area:21 restores into an empty database with entry counts equal to the source (Q-21, X-112).
23. **Tests.** `cargo test` runs the fixtures Q-01 to Q-21 and the cases X-01 to X-112 that are not marked benchmark-only (X-100 to X-112 and the decode and validation cases). Q-20 is a benchmark, not a test.
24. **Strings.** Every user-facing string in this area is a constant in one module, with the English text as the key value (T-22a-043).
