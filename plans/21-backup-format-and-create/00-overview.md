# Area 21: Backup file format and create, overview

Area id: `21-backup-format-and-create`. Owner of the write side of Aniyomi backups (`.tachibk`). The restore side is `area:22-backup-restore`. Source tree for all repo-relative paths: `/home/blackbox/aniyomi-src`. OBSERVED = what the Kotlin does. PROPOSAL = what Kurobayomi should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Aniyomi writes a single backup file, a gzip-compressed Kotlin `kotlinx.serialization` ProtoBuf message (`Backup`), with the extension `.tachibk`. The file holds manga and anime library entries with their chapters or episodes, categories, tracker bindings, read history, source references, extension stores and repos, custom buttons, app preferences, per-source preferences, and optionally installed extension packages.

Two producers exist:
- Manual: `CreateBackupScreen` starts a one-time WorkManager job with the user's `BackupOptions` and a user-chosen file (`app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt`, B-01).
- Automatic: a periodic WorkManager job `BackupCreator` with interval `backup_interval` (default 12 hours, allowed UI values 0, 6, 12, 24, 48, 168), always with `BackupOptions()` defaults, written to `<storage dir>/autobackup`, keeping 4 files (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:229`, B-04, B-07).

### 1.2 Why it matters for a usable Kurobayomi

- It is the only data-portability path the user has between the Android app and Kurobayomi. A Kurobayomi library that cannot be exported to the same format leaves the user without an exit and without a recovery path for a corrupted local SQLite database.
- Restore (area:22) cannot be tested without files produced by this area. This area's output is the fixture source for the restore side.
- The format is also the only way to move tracker bindings and read history between the two apps.

### 1.3 Scope of this area

In scope: the protobuf wire codec, the container and entry schemas, format sniffing and legacy detection, `BackupOptions`, the Create Backup screen state, the build pipeline, all creators, file write and self-check, retention and file names, the automatic scheduler and manual single-flight job, create notifications, the "Show in folder" action, the preflight validator (shared with restore), and the drop decisions for dead code.

Out of scope (owned elsewhere): restore execution and restore notifications (area:22-backup-restore), the preference store itself (area:02-preferences-system), the shared job scheduling module (area:26-library-update-jobs, see section 5).

### 1.4 Size of the Kotlin scope

38 files, 2,239 LOC, all under `app/src/main/java/eu/kanade/tachiyomi/data/backup/`. Per package (LOC from the inventory):

| Package (repo-relative) | Files | LOC |
|---|---|---|
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/` (decoder, detector, validator, notifier) | 4 | 360 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/` (job, creator, options) | 3 | 515 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/` | 11 | 483 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/` | 18 | 827 |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/` (dead) | 2 | 54 |

Settings rows that trigger this area are in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (owned by area:02-preferences-system).

---

## 2. Module map

### 2.1 Kotlin module map (OBSERVED)

| Package or file (repo-relative under `app/src/main/java/eu/kanade/tachiyomi/data/backup/`) | Responsibility |
|---|---|
| `BackupDecoder.kt` | Opens a URI, sniffs the magic bytes (gzip, JSON, raw), decodes `Backup` or `LegacyBackup`. |
| `BackupDetector.kt` | Cheap two-field partial decode to classify a file as pre-1.0 legacy. |
| `BackupFileValidator.kt` | Decode round-trip plus preflight: missing sources and logged-out trackers. Used by restore and by the creator self-check. |
| `BackupNotifier.kt` | Progress, complete and error notifications for backup and restore. |
| `create/BackupCreateJob.kt` | WorkManager worker for manual and automatic runs, `setupTask` scheduling. |
| `create/BackupCreator.kt` | Pipeline: target file, retention, queries, assemble, encode, gzip write, self-check, timestamp. |
| `create/BackupOptions.kt` | The 12-flag option model, labels, enable rules, `canCreate`. |
| `create/creators/*.kt` (11 files) | One creator per record kind: manga and anime entries, manga and anime categories, sources, extension stores and repos, custom buttons, extension packages, preferences. |
| `models/*.kt` (18 files) | `@Serializable` wire models for the live format (`Backup`, `LegacyBackup`, entries, trackers, history, categories, sources, stores, repos, preferences, custom buttons, extension bytes), plus SQL row mappers and `get*Impl` conversions to domain types. |
| `full/models/*.kt` (2 files) | Earlier container and preference model, not referenced anywhere. Dropped (T-21a-033). |

Shared infrastructure referenced by this area but owned elsewhere: `AppModule.kt:190` registers `ProtoBuf` as a singleton (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt`); `domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt` and `PreferenceValues.kt` hold the preference keys and the `FLAG_*` constants; `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt` provides the `autobackup` directory.

### 2.2 Rust module map (PROPOSAL, crate `kurobayomi`, root `src/backup/`)

Taken from p1-rust-design.md section 2. Every module has its task ids.

| Rust module | Responsibility | Tasks |
|---|---|---|
| `src/backup/mod.rs` | Public re-exports, `BackupError`, user message keys | T-21a-038 |
| `src/backup/wire.rs` | Hand-written protobuf codec: `Writer`, `Reader`, `Message` trait, `WireError` | T-21a-001 |
| `src/backup/schema.rs` | `Backup`, `LegacyBackup`, entry structs, enums, `PrefValue`, `Message` impls, restore-side `into_domain` | T-21a-002..006, 009 |
| `src/backup/format.rs` | `sniff`, `read_backup_bytes`, legacy detection, `decode_file`, gzip helpers | T-21a-007, 008 |
| `src/backup/options.rs` | `BackupOptions` (struct, not bool array), labels, enable rules, `can_create` | T-21a-010 |
| `src/backup/creator/mod.rs` | `CreateInputs`, `build_backup`, `create_file`, `Target`, progress | T-21a-012, 022, 025, 036 |
| `src/backup/creator/manga.rs` | Manga entries, excluded scanlators, chapters, tracking, history | T-21a-013 |
| `src/backup/creator/anime.rs` | Anime entries, episodes, tracking, history | T-21a-014 |
| `src/backup/creator/categories.rs` | Manga and anime category lists | T-21a-015 |
| `src/backup/creator/sources.rs` | Source references, stub-name rule | T-21a-016 |
| `src/backup/creator/stores.rs` | Anime extension stores, manga extension repos | T-21a-017 |
| `src/backup/creator/custom_buttons.rs` | Custom buttons | T-21a-018 |
| `src/backup/creator/extensions.rs` | Installed extension artifacts (opt-in) | T-21a-019 |
| `src/backup/creator/preferences.rs` | App and per-source preferences, key rules | T-21a-020, 021, 044 |
| `src/backup/validate.rs` | Preflight (missing sources, logged-out trackers), post-write self-check | T-21a-023, 024 |
| `src/backup/auto.rs` | Automatic scheduler: `decide`, `run_loop`, retention, backoff | T-21a-026, 032, 037, 039 |
| `src/backup/job.rs` | Manual job, single-flight slot | T-21a-027 |
| `src/backup/notify.rs` | Desktop notifications, "Show in folder" | T-21a-028, 029, 030, 042 |
| `src/backup/settings.rs` | Settings keys and rows for the backup group | T-21a-031, 032 |
| `src/backup/schema_text.rs` | Generated schema text for the debug screen | T-21a-035 |
| `tests/backup/` | Fixtures and integration tests | T-21a-034, 040 |
| (no module path fixed in the plan) | P2 `--run-backup` entry point and systemd user timer units; the file location is decided when T-21a-041 starts | T-21a-041 |

Crates (PROPOSAL, from p1-rust-design.md section 1): `rusqlite` (bundled SQLite), `flate2` (gzip, miniz_oxide backend), `thiserror`, `tracing`, `tokio`, `async-channel`, `notify-rust` (freedesktop notifications), `zbus` (UPower, `org.freedesktop.FileManager1`), `chrono` (local time for file names), `gtk` = gtk4 0.11.5 with feature `v4_16`. No `prost`, no `protobuf` crate.

The current mockup has no backup logic. `src/settings.rs:18` contains a placeholder text dump for `settings_backup_schema`; T-21a-035 replaces it.

---

## 3. Reading guide

All files are in this folder. Read in the order listed for a first pass.

1. [p1-inventory.md](p1-inventory.md): every Kotlin file in scope with LOC, declared names, port decision and target Rust module.
2. [p1-behavior.md](p1-behavior.md): OBSERVED behaviour, section ids B-01 to B-20 (entry points, options, scheduling, job body, pipeline, file naming, creators, mapping rules, message keys, timings, defects).
3. [p1-data-and-config.md](p1-data-and-config.md): exact wire layout (container, entries, categories, sources, stores, preferences), field-number summary, Kurobayomi config keys, files on disk, DB tables read, round-trip contract.
4. [p1-rust-design.md](p1-rust-design.md): Rust types, traits, function signatures, decisions, threading model, Android-to-Linux mapping, error type, pitfalls, test seams.
5. [p1-topic-protobuf-wire-format.md](p1-topic-protobuf-wire-format.md): container layout, kotlinx ProtoBuf behaviours (K-1..K-10), decode tolerance policy, golden bytes, the fixture work needed to close the open items.
6. [p1-topic-autobackup-scheduling.md](p1-topic-autobackup-scheduling.md): due rule, deferrals, retry and backoff, single-flight, systemd user timer design, shared design with area:26.
7. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): 45 edge cases (E-01..E-45), race conditions, failure modes with user-visible result, test plan, new tests, fixtures needed.
8. [p1-ui-parity.md](p1-ui-parity.md): settings group, Create Backup screen layout and behaviour, keyboard and accessibility, notification texts, schema debug screen, file dialogs, relative time string, mockup gaps.
9. [p1-tasks.md](p1-tasks.md): 44 implementation tasks T-21a-001 to T-21a-044 with priority, effort, dependencies, sources and acceptance criteria.

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the area docs (no user call needed)

| # | Decision | Where | Why |
|---|---|---|---|
| D1 | Hand-written protobuf codec, no prost or protobuf crate | p1-rust-design.md section 1 item 1 | kotlinx ProtoBuf differs from proto3 defaults: repeated scalars are unpacked, sealed `PreferenceValue` layout, enums are ordinals, absent nullable fields stay absent |
| D2 | Gzip via `flate2`, level 6 (`Compression::default()`) | p1-rust-design.md section 5 | Matches Kotlin's `gzip()` default |
| D3 | In-process tokio loop with a persisted last-run timestamp instead of an OS job store; systemd user timer only in P2 | p1-rust-design.md section 1 item 3, T-21a-041 | No WorkManager on Linux |
| D4 | One single-flight slot shared by manual and automatic runs | p1-rust-design.md section 9 | Replaces WorkManager `KEEP` and `isManualJobRunning` |
| D5 | Output is `.part` file, fsync, then rename; self-check decodes the written file | p1-rust-design.md section 6.2 | Atomic publish; a failed write leaves no broken `.tachibk` |
| D6 | Retention runs after a successful write, not before | p1-rust-design.md section 6.2 step 5 | Aniyomi prunes first (`BackupCreator.kt:89`), so a failed write loses one backup (E-04) |
| D7 | File names carry seconds: `kurobayomi_yyyy-MM-dd_HH-mm-ss.tachibk`, collision suffix `-2`, `-3` | p1-behavior.md B-07, p1-rust-design.md 6.3 | Aniyomi uses minute resolution and can collide (B-07) |
| D8 | `is_legacy` is written as `false` | p1-data-and-config.md section 10 | Aniyomi's detector must classify Kurobayomi files as current |
| D9 | Dead code dropped: `full/models/*`, `BackupExtensionPreferences`, both `Broken*` variants, `backup_flags` preference | p1-inventory.md rows 19, 20, 24, 31, 37; p1-data-and-config.md section 6 | No references anywhere in the repository (T-21a-033) |
| D10 | Preference values are not encoded until the sealed `PrefValue` tags are verified from fixtures | p1-topic-protobuf-wire-format.md section 5, T-21a-034 | The tags are not in the Kotlin source text; guessing would corrupt backups |
| D11 | No JSON import, no JSON export, no silent format conversion | p1-rust-design.md pitfall 8 | Matches Aniyomi, which rejects JSON by magic bytes |

### 4.2 Open questions that need the user's call

**Q1. Fixtures from the user's Aniyomi app (blocking).**
- Need: `current-all.tachibk` and `current-default.tachibk` created in the Aniyomi app, and the text schema from Aniyomi's debug schema screen (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt`).
- Why: the nested tags of the six `PreferenceValue` subclasses (K-9) are only obtainable from a real dump. Until then, `T-21a-006`, `T-21a-020`, `T-21a-021` and therefore `T-21a-012` cannot be finished with preferences enabled. T-21a-034 is the gate.
- Recommendation: the user exports the two files and the schema text once. Meanwhile, ship the pipeline with the `appSettings` and `sourceSettings` options disabled in the UI and the preference encoder returning "preference encoding not verified". Do not guess tags.

**Q2. Should Kurobayomi files stay readable by Aniyomi?**
- Options: (a) full compatibility, same field numbers and `order`-based category links; (b) Kurobayomi-only format with a different extension.
- Recommendation: (a). The whole round-trip contract (p1-data-and-config.md section 10) depends on it, and it lets the user migrate back. The cost is that the schema is frozen to Aniyomi's field numbers, which the plan already assumes.

**Q3. Battery threshold for automatic backups.**
- area:21 proposes 15 percent on battery (p1-rust-design.md section 8). area:26-library-update-jobs proposes 20. The two must be one constant (p1-topic-autobackup-scheduling.md section 7).
- Recommendation: 20 percent, shared by both jobs. It is the more conservative value for a laptop user.

**Q4. Automatic backups use defaults: no extensions and no private settings.**
- Aniyomi does this (B-01, B-20 item 3). Private settings include tracker tokens and credentials (`__PRIVATE_*`).
- Recommendation: keep the exclusion for automatic runs (no secrets written unattended), and state it on the Settings row (T-21a-032). The user can still create a manual backup with private settings after an explicit toggle (T-21a-044).

**Q5. What does the `extensions` option mean on Linux?**
- Aniyomi embeds the installed extension APK bytes (`ExtensionsBackupCreator`, `BackupExtension`). Kurobayomi extensions are not APKs. The payload would be whatever artifact area:18/19 installs.
- Options: (a) keep the option and embed the extension artifact files (P2, T-21a-019); (b) remove the option and record that extensions are re-installed from their repos.
- Recommendation: (b) for P1, because restoring a binary also needs the extension trust and loading model owned by area:20-extension-strategy-redesign. Revisit with T-21a-019 in P2, once area:20 has fixed what an installed extension artifact is.

**Q6. Divergences from Aniyomi defects.**
- The plan fixes: retention after write (E-04), seconds in names (B-07), stub name shown as numeric id when unknown (E-17), `Corrupt` mapping for gzip errors as well as wire errors (p1-rust-design.md section 5), explicit `i32::try_from` instead of silent truncation (pitfall 9).
- Recommendation: accept all of them. Each one changes a bad outcome into a clear error without changing the file layout.

**Q7. Key naming for backed-up preferences.**
- Conflict: area:21 proposes Kurobayomi keys such as `backup.interval_hours` (p1-data-and-config.md section 6). area:47-settings-screens-general (T-47a-009) and the stored-key rule say keys keep the Android names so that backups remain importable.
- Recommendation: keys that are written into a backup use the Aniyomi literal names (`backup_interval`, `__APP_STATE_last_auto_backup_timestamp`, `hide_notification_content`). Kurobayomi-only keys that are never exported may use new names. area:02-preferences-system must confirm this rule before T-21a-020 is built.

**Q8. Automatic backups run only while the app runs.**
- The in-process loop checks on each 10-minute tick and performs a catch-up on startup (p1-topic-autobackup-scheduling.md section 6). A systemd user timer that runs the backup with the app closed is P2 (T-21a-041).
- Recommendation: P1 without the timer. Ask the user whether a closed-laptop backup matters; if yes, promote T-21a-041.

**Q9. Notifications channel.**
- Aniyomi uses importance levels (`IMPORTANCE_LOW` for progress, `IMPORTANCE_HIGH` for completion). Kurobayomi uses freedesktop urgency (p1-rust-design.md section 10).
- Recommendation: progress at Low, completion and error at Normal (not Critical, to avoid pop-up spam on KDE). Also show an in-window toast so the result is visible when notifications are off.

### 4.3 Risks specific to this area (summary, details in section 6.4)

- The fixture dependency (Q1) can stall the whole manual pipeline.
- The wire codec is correct only if the golden bytes match. Fixture tests are the only proof.
- Full in-memory build for large libraries: p1-rust-design.md pitfall 4 estimates a few hundred MB for 10,000 entries with chapters.

---

## 5. Cross-area dependencies

### 5.1 This area needs from others

| Area | What is needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | App context and the GTK main loop; the Create screen is a child of the app window | T-21a-011, T-21a-027 |
| area:02-preferences-system | Preference store with the key rules (`__APP_STATE_`, `__PRIVATE_`), `backup_interval`, `hide_notification_content`, all-keys enumeration, the settings rows in `SettingsDataScreen.kt` | T-21a-020, 021, 030, 032, 044 |
| area:03-db-manga-schema-migrations and area:04-db-anime-schema-migrations | Table and column semantics in p1-data-and-config.md section 9; WAL mode; a migrated schema before the scheduler starts (pitfall 10) | T-21a-003, 004, 012 |
| area:05-data-repos-entries-items | Repository queries: favourites, read or watched entries not in library, chapters, episodes | T-21a-013, 014 |
| area:06-data-repos-history-track-source-misc | History rows, track rows, custom buttons, source ids | T-21a-013, 014, 018 |
| area:08-domain-library-category-history-updates | Non-system categories and category links by `order` | T-21a-015 |
| area:09-domain-source-track-download-release-misc | `TrackerManager` for the validator (`name`, `logged_in`) and tracker records | T-21a-016 |
| area:11-app-migrations | `EnableAutoBackupMigration` (version `84f`) and `SetupBackupCreateMigration` (ALWAYS) rerun the scheduler setup | T-21a-039 |
| area:13-core-util-storage-archive-torrent | Base storage directory, `autobackup` directory creation, file operations, display path | T-21a-022, 025, 037 |
| area:17-source-app-layer | Source registry: installed name, stub name, `Name (LANG)` label | T-21a-016, 021 |
| area:18-extensions-manga and area:19-extensions-anime | Extension stores and manga repos (`extension_store`, `extension_repos`) | T-21a-017 |
| area:26-library-update-jobs | Shared scheduler module (`TICK`, `RETRY_BASE`, `PowerState`, battery threshold, single-flight abstraction, due arithmetic) | T-21a-026, 041 |
| area:27-tracking-core | Tracker login state for the preflight | T-21a-024 |
| area:34-images-covers-cache | `backgroundUrl` is not written by Aniyomi (B-08b gap). Confirm whether Kurobayomi keeps backgrounds at all | T-21a-014 |

### 5.2 Other areas need from this area

| Area | What it needs | Provided by |
|---|---|---|
| area:22-backup-restore | Wire codec, schema, sniffing, legacy detection, `decode_file`, `get*Impl` equivalents (`into_domain`), preflight `Preflight` struct, restore notification texts, `hide_content` handling | T-21a-001..009, 023, 024, 029, 030 |
| area:26-library-update-jobs | Shows how the due rule, backoff and single-flight slot are shared; this area is the first implementer of the shared module | T-21a-026 |
| area:02-preferences-system | Backup key rules for private and app-state keys | T-21a-020, 044 |
| area:47-settings-screens-general | Backup-compatibility classification of keys (T-47a-009) depends on this area's rules | T-21a-044 |
| area:02-preferences-system (settings rows) | The "last backup" info text and the interval row live in the settings screens; the storage screen in area:42-ui-updates-history-stats-storage-upcoming should show the autobackup folder only if it already does so there | T-21a-031, T-21a-032 |

Note: the Create Backup screen (T-21a-011) is a UI in the settings flow. Its placement follows area:02-preferences-system (settings screens).

---

## 6. Task summary

### 6.1 Counts (from `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, rows of `TASKS.csv` whose file starts with `21-backup-format-and-create/`)

Total 44 tasks (T-21a-001 to T-21a-044). The lint of `p1-tasks.md` prints `ok`.

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 0 | 0 | 0 | 0 | 0 |
| P1 | 25 | 14 | 1 | 0 | 40 |
| P2 | 0 | 2 | 0 | 0 | 2 |
| P3 | 2 | 0 | 0 | 0 | 2 |
| Total | 27 | 16 | 1 | 0 | 44 |

P1 tasks: every id from T-21a-001 to T-21a-044 except T-21a-019 and T-21a-041 (P2) and T-21a-033 and T-21a-035 (P3). P2: T-21a-019, T-21a-041. P3: T-21a-033, T-21a-035.

Area 21 has no P0 task. Backup is parity (P1). Kurobayomi can read and watch without it. The earliest useful slice is a manual backup without preferences (section 6.2).

### 6.2 P0 critical path (in order)

There is no P0 task. The P1 critical path for a usable manual backup, in order:

1. T-21a-001 wire codec (M) → verify: round-trip and unknown-field tests pass.
2. T-21a-002 container schema (M) → verify: `Backup` and `LegacyBackup` encode and decode.
3. T-21a-003 manga schema and mappers (M), T-21a-004 anime schema and mappers (M), in parallel → verify: golden bytes for an empty `BackupManga` (T-21a-034 fixtures).
4. T-21a-010 options model (S) → verify: `can_create` truth table.
5. T-21a-013 manga creator (M) and T-21a-014 anime creator (M), with T-21a-015..018 (S each) → verify: fixture DB produces the expected entries.
6. T-21a-034 fixtures (M) → verify: user-supplied Aniyomi files decode and the manifest is written. This is user-blocked (Q1).
7. T-21a-006 preference schema (S), T-21a-020 app prefs (M), T-21a-021 source prefs (S) → verify: encoded tags match the fixture. Blocked by step 6.
8. T-21a-012 pipeline (L) → verify: `build_backup` plus `create_file` produce a file that `decode_file` reads back.
9. T-21a-022 write (S) and T-21a-023 self-check (S) and T-21a-036 names (S) → verify: `.part` is renamed; a corrupted write is deleted.
10. T-21a-027 single-flight slot (M) → verify: a second start returns `InProgress`.
11. T-21a-011 Create screen (M) → verify: the button is disabled when `can_create` is false; the toast appears on a second start.
12. T-21a-028 notifications (S) → verify: progress, complete and error notifications appear on a freedesktop daemon.
13. T-21a-040 test suite (M) → verify: all listed tests pass in CI.

Automatic backup adds: T-21a-024 preflight (M), T-21a-025 retention and startup cleanup (M), T-21a-026 scheduler (M), T-21a-032 interval setting (S), T-21a-031 last timestamp (S), T-21a-037 directory (S), T-21a-039 migration equivalents (S).

Blocking note: step 7 depends on step 6, and step 8 depends on step 7 (T-21a-012 depends on T-21a-020 and T-21a-021). Recommendation in Q1: split step 8 so preferences can be disabled while fixtures are pending.

### 6.3 Dependency on other areas (critical)

The P1 chain also depends on: area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc (for T-21a-013 and 014), area:09 and area:18/19 (for sources and stores), area:26-library-update-jobs (T-21a-026), area:13 (T-21a-022, 025, 037), area:27 (T-21a-024). These must be scheduled before this area's critical path can finish.

### 6.4 Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Fixtures from the user are not provided (Q1) | Preferences and the full pipeline are blocked | Ship without preferences first; flag the preference options as disabled |
| Golden bytes differ from kotlinx output (wire codec defects) | Aniyomi cannot read Kurobayomi backups; restore tests give false confidence | Fixture round-trip tests (T-21a-034, 040); item-by-item comparison, not byte comparison |
| Sealed `PreferenceValue` tags unknown | Preference section cannot be written | No encoding until T-21a-034 step 3 is complete |
| Key naming conflict with area:47 (Q7) | Backups of preferences do not import | Decide with area:02 before T-21a-020 |
| Memory for large libraries (in-memory build) | Peak hundreds of MB for ~10,000 entries | Documented limit for P1; streaming writer is a v2 proposal (pitfall 4) |
| Battery threshold disagreement (Q3) | Two jobs with different rules | One constant owned by area:26 |
| Automatic backups only run while the app is open (Q8) | Long gaps on a closed laptop | Catch-up on startup; systemd timer in P2 |
| Notification daemon absent | No progress or completion notice | Log and continue; in-window toast |
| Portal document paths (Flatpak) | "Show in folder" opens the wrong folder | Documented limitation (pitfall 6) |
| Local-time file names around DST | Name collisions | Seconds suffix plus `-2`, `-3` suffix |
| Concurrent restore and automatic backup | Inconsistent read or two writers | `restore_running` deferral (`Defer(RestoreRunning)`) |
| Interrupted write leaves `.part` file | Clutter in the autobackup folder | Startup cleanup of `*.tachibk.part` (T-21a-025) |

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable by the user or by a test named in p1-edge-cases-and-tests.md.

1. Settings > Data > "Create backup" opens a GTK save dialog with the suggested name `kurobayomi_<yyyy-MM-dd_HH-mm-ss>.tachibk`. The extension is forced to `.tachibk`.
2. The Create button is disabled while none of library entries, categories, app settings, extension stores, custom buttons or source settings is selected (`can_create` rule, B-02). Enabling `Library entries` enables `Chapters and episodes`, `Tracking`, `History` and `All read/watched entries`. Disabling it leaves their values unchanged.
3. A manual backup of a library with N favourite entries writes one `.tachibk` file that the self-check decodes. The file contains N entries under field 1 (manga) and field 501 (anime) with the expected chapters, episodes, categories (by `order`), tracking and history.
4. A manual backup started while another is running shows the toast `backup_in_progress` and creates no second file.
5. A failed write (for example, a read-only target) shows the error text for `create_backup_file_error` and leaves no `.part` file and no partial final file.
6. Automatic backups: with interval 12, the first run occurs 12 hours after enabling (not immediately), and runs catch up once at startup if the due time has passed. Exactly four files named `kurobayomi_*.tachibk` remain in `<data dir>/kurobayomi/autobackup` after a fifth run. The fifth run deletes the oldest file only after the new file is written.
7. Automatic runs never include extension binaries or `__PRIVATE_*` settings.
8. Setting the interval to 0 stops automatic runs immediately. Setting it to a new value takes effect without restart.
9. A progress notification and a completion notification appear through the freedesktop service for a manual run. The completion notification action "Show in folder" selects the file in the file manager when `org.freedesktop.FileManager1` is available.
10. A file created by Kurobayomi is accepted by Aniyomi's restore preflight with `is_legacy = false`, and its category links are the category `order` values (verified by the manual acceptance step in p1-edge-cases-and-tests.md section 4.10 and 4.11).
11. A `.tachibk` file produced by Aniyomi (current or Mihon-shaped) is read by the Kurobayomi reader. Unknown field numbers are skipped. A JSON file is rejected with `invalid_backup_file_json`. A truncated or corrupt file is rejected with `invalid_backup_file_unknown`.
12. The tests in p1-edge-cases-and-tests.md sections 4.1 to 4.11 pass, and the fixture set in section 6 of that file is committed under `tests/backup/fixtures/`.

---

## 8. Definition of done checklist for this overview (self-check)

- Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/21-backup-format-and-create/p1-tasks.md` prints `ok`.
- Dependency tokens: every `area:` token in `p1-tasks.md` names an existing folder under `/home/blackbox/kurobayomi/plans/`. The shorthand tokens in the other area 21 documents (for example `area:03/04`, `area:01/02`, `area:17/18/19`, `area:22`) are not in the tasks file and were left unchanged by this pass.
- Source paths cited in this overview were checked to exist under `/home/blackbox/aniyomi-src`.
