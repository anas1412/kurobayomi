# Area 21: Edge cases, failure modes, race conditions and tests

Ids `E-nn` are referenced from p1-behavior.md, p1-rust-design.md and p1-tasks.md. "Kotlin" = current Aniyomi behaviour (B-nn in p1-behavior.md). "Kurobayomi" = the proposed behaviour.

Upstream tests: none exist for this scope. A search of the repository for `*Backup*Test*.kt` and for any `*.kt` under `src/test` or `src/androidTest` containing "backup" returned no files. Every test below is new.

---

## 1. Edge cases

| Id | Case | Kotlin behaviour (observed) | Kurobayomi behaviour (proposal) | Task |
|---|---|---|---|---|
| E-01 | Input file is 0 or 1 byte | `source.peek().require(2)` throws `EOFException`, not converted to a user message; the validator wraps it as `IllegalStateException(e)` | "Backup file is corrupted" for both | T-21a-007, T-21a-038 |
| E-02 | Input is JSON | First two bytes `{}`, `{"` or `{\n` -> "JSON backup not supported". JSON with leading whitespace is read as raw protobuf -> "Backup file is corrupted" | Same two messages; do not parse JSON in any case | T-21a-007 |
| E-03 | Two automatic backups in the same minute; filename collision on SAF | Same name; SAF typically suffixes `name (1).tachibk`, which the regex does not match, so it is never pruned | Seconds in name, then `-2`, `-3` suffixes before `.tachibk`; regex accepts `(-\d{2})?(-\d+)?`; regex dot escaped | T-21a-036 |
| E-04 | Retention ordering | Prunes older files first, then writes. A write failure after pruning leaves one fewer backup | Write `.part`, verify, rename, then prune to 4. Divergence recorded in p1-behavior.md B-07 | T-21a-025 |
| E-05 | Library changes during a backup | Several independent queries without a transaction; mixed snapshot possible | One read transaction for the whole build (`BEGIN DEFERRED` ... `COMMIT`) | T-21a-012 |
| E-06 | Nothing selected, or only `extensions` selected | Create button disabled (`canCreate()` false) | Same. `extensions` alone is NOT enough (it is not in `canCreate()`) | T-21a-010, T-21a-011 |
| E-07 | Float precision | `chapterNumber`, `episodeNumber`, `lastChapterRead`, `lastEpisodeSeen`, `score` stored as float32 | Same (format fixed). Chapter numbers above about 16,777,216 or with more than about 7 significant digits round. Document; do not widen the format | T-21a-003, T-21a-004 |
| E-08 | First automatic run after enabling | WorkManager schedules the first periodic run; behaviour depends on WorkManager | If `last_auto_epoch_ms == 0`, first due time = enable time + interval | T-21a-026 |
| E-09 | `libraryEntries` off but `readEntries` on | Non-favourite query still runs; its result is then dropped by `backupAnimes`/`backupMangas` | Skip the query. No behaviour change in output | T-21a-012 |
| E-10 | Automatic run fails | `Result.failure()`: no retry until the next period; nothing shown to the user | Retry after `10 min * 2^(n-1)` capped at the interval; `last_auto` not advanced; show one warning notification after 3 consecutive failures | T-21a-026 |
| E-11 | Gzip stream corrupted or truncated | `okio` IOException (not a `SerializationException`), so the message is the raw IO message | Map to "Backup file is corrupted" like any undecodable file | T-21a-038 |
| E-12 | Restore preflight error text | `IllegalStateException(e).message` = `java.io.IOException: Backup file is corrupted` shown in restore UI | Show the localized key text only | T-21a-024 |
| E-13 | Interval value not in UI list | Code accepts any Int (for example 3 hours) | Reject anything outside {0, 6, 12, 24, 48, 168} on load (fall back to 12) and in the setter | T-21a-032 |
| E-14 | Integer overflow of `status`, `chapterFlags`, `viewerFlags`, `totalChapters`, `totalEpisodes`, `score` | `toInt()` wraps silently | `try_from`; on overflow fail the backup with "Backup failed" and log the row id | T-21a-013, T-21a-014 |
| E-15 | Two categories with equal `order` | Entries link by order; restore maps by order and may link to the wrong category | Keep the link as order (format), but the Kurobayomi category layer must guarantee unique `order` per kind; creation logs a warning on duplicates | T-21a-015 |
| E-16 | Extension package removed during backup (extensions option on) | `NameNotFoundException` aborts the whole backup | Skip that artifact, log, and report "1 extension could not be read" in the completion notification; the rest is written | T-21a-019 |
| E-17 | Stub source with blank name | Backup stores `name = ""`; restore preflight lists an empty string as a missing source | Store the stub label if known; if unknown, list the numeric id, never an empty string | T-21a-016, T-21a-024 |
| E-18 | Source name is all digits | Validator treats the name as a numeric id (`toLongOrNull`) and looks up a stub | Same rule for compatibility with Aniyomi-created files; add a test pinning the behaviour | T-21a-024 |
| E-19 | Tracking row with `mediaId == 0` and `mediaIdInt != 0` (legacy) | Restore prefers `mediaIdInt` | Creation always writes `mediaId` (100) and never `mediaIdInt` (3). Test that a round-trip keeps `remoteId` | T-21a-003, T-21a-004 |
| E-20 | Manual target already exists | Truncated and overwritten after the SAF dialog | Same. GTK's save dialog already confirms overwrite; do not add a second prompt | T-21a-022 |
| E-21 | Automatic directory missing or base storage unavailable | `getAutomaticBackupsDirectory()` null -> job `Result.failure()`, silently | Show one "Automatic backup failed: folder unavailable" notification per failure streak; do not spam | T-21a-025, T-21a-028 |
| E-22 | No notification daemon | `notify` silently returns when permission is missing (API 33+) | `notify-rust` error logged at warn level and ignored | T-21a-028 |
| E-23 | Automatic backup due while a restore runs | `Result.retry()` with backoff | `Defer(RestoreRunning)`, retry after 10 minutes | T-21a-026 |
| E-24 | Manual and automatic run overlap | Allowed (no guard between them) | Single-flight: the automatic tick is skipped while a manual run is active; a manual request is refused with "Backup is already in progress" while an automatic run is active | T-21a-027 |
| E-25 | Excluded scanlators with `chapters` off | Excluded scanlator list is still written | Same (format parity). Document that restore then restores exclusions without chapters | T-21a-013 |
| E-26 | `backgroundUrl` for anime | Never written | Write it from the anime entity when present (area:34 decides whether backgrounds exist) | T-21a-014 |
| E-27 | `privateSettings` checked, then app and source settings both unchecked | UI disables the private row; the stored flag stays true but has no effect (no prefs are dumped) | Reset `privateSettings` to false when both parents are off, so the UI state equals the effect | T-21a-010 |
| E-28 | Memo column invalid JSON | Mapper decode fails earlier when reading the row | Validate memo is a JSON object at load time; a row with a bad memo writes `{}` and logs a warning (no backup failure) | T-21a-004 |
| E-29 | Non-ASCII names, spaces in path, very long titles | Written as UTF-8 | Same. Test with emoji, RTL, combining marks, 4 KiB titles | T-21a-040 |
| E-30 | Power loss or kill during write | Partial file remains at the chosen path (manual) or a partial newest automatic file | `.part` file; rename only after fsync; startup removes stale `*.tachibk.part` in the automatic directory | T-21a-022, T-21a-025 |
| E-31 | System clock moves backwards | `Instant.now()` used for the timestamp only | If `last_auto_epoch_ms > now`, treat last as `now` (so the schedule does not freeze) | T-21a-026 |
| E-32 | Time zone change | Name uses device local time | Same; the seconds suffix plus collision suffix keeps names unique | T-21a-036 |
| E-33 | Protobuf `int64` negative values (for example `sortIndex`) | kotlinx writes 10-byte varints | Same encoding; test a negative value round-trips | T-21a-001 |
| E-34 | Unknown field numbers in a newer file | Skipped | Skipped (forward compatibility) | T-21a-001 |
| E-35 | Enum ordinal out of range (newer Aniyomi) | kotlinx throws `SerializationException` -> "corrupted" | Same, via `WireError::BadEnum` | T-21a-001 |
| E-36 | Packed repeated fields (not produced by Aniyomi) | kotlinx default is unpacked; packed input would fail decode | Accept both (section 3 of p1-rust-design.md) | T-21a-001 |
| E-37 | Required field missing (for example `url`) | `MissingFieldException` -> "corrupted" | Same | T-21a-001 |
| E-38 | Sealed `PreferenceValue` unknown variant | kotlinx throws -> "corrupted" | Same. Fixtures decide the variant tags (T-21a-034) | T-21a-034 |
| E-39 | Same file in two destinations | Two independent runs | Independent; single-flight applies to the job, not to destinations | T-21a-027 |
| E-40 | Automatic backup started when app is closed | Runs (WorkManager survives process death) | Does not run (in-process loop). P2 systemd timer covers this (T-21a-041) | T-21a-041 |
| E-41 | Very large library (10,000+ manga with chapters) | Whole payload in memory (`encodeToByteArray`) | Same for P1; document the memory peak; streaming writer is a v2 item | T-21a-012 |
| E-42 | Duplicate source references | `distinct()` in source creators | Same | T-21a-016 |
| E-43 | Preferences with unsupported runtime type | Dropped silently (`else -> null`) | Same. A Rust `PrefValue` enum makes this impossible; log the key count of skipped values at debug level | T-21a-020 |
| E-44 | `StringSet` with non-string elements (cannot occur in Kotlin storage) | Unchecked cast; fails at encode | Not representable in Rust; not applicable | T-21a-020 |
| E-45 | Backup created with restore of the same file concurrently | Restore refuses only when another restore runs; an automatic backup is retried | Restore reads a file, the job never writes into the file being read (new name each run) | T-21a-026 |

---

## 2. Race conditions

| Id | Race | Mitigation |
|---|---|---|
| R-01 | Two manual starts (double click on Create) | Kotlin: `ExistingWorkPolicy.KEEP` drops the second; UI guard toast. Kurobayomi: `try_lock` on job slot; second returns `InProgress`; button disabled while the save dialog is open |
| R-02 | Manual run and automatic tick | Single-flight slot; tick skips; manual refused with message |
| R-03 | Restore starts while automatic run writes | Restore should check the job slot (area:22); automatic run checks restore state before starting (B-05.2) |
| R-04 | Library edit during backup (SQL reads) | One read transaction (E-05) |
| R-05 | Settings change of interval during a run | Watch channel; running job unaffected; new interval applies on next tick |
| R-06 | Process killed between write and rename | `.part` file ignored by readers and removed on startup (E-30) |
| R-07 | Two processes (Kurobayomi plus a future CLI `--run-backup`) writing the same automatic dir | Lock file `autobackup/.lock` with `flock` (advisory, `fs2` or `rustix`); skip when locked (P2, with T-21a-041) |
| R-08 | Notification update after window closed | Notifications are independent of window lifetime; handles held by the job |
| R-09 | Action callback (Show in folder) arrives after the app quit | Callback thread is dropped on exit; the action is lost (acceptable) |

---

## 3. Failure modes and expected user-visible result

| Failure | Manual run result | Automatic run result |
|---|---|---|
| Destination not writable | Notification "Backup failed" + reason text | Retry with backoff; warning after 3 failures (E-10) |
| Disk full during write | "Backup failed"; `.part` removed | Same; previous backups kept (E-04 fix) |
| Self-check decode fails | "Backup failed" (corrupted file deleted) | Same |
| DB locked (SQLITE_BUSY) for more than 5 s | "Backup failed" with the SQLite message | Retry after 10 minutes |
| Extension artifact unreadable | Completes; notification notes skipped artifact (E-16) | Not applicable (extensions off by default) |
| Restore running | Manual allowed (Kotlin does not check) | Deferred (E-23) |
| Battery low on battery power | Not applicable (manual ignores battery) | Deferred (section 8 of p1-rust-design.md) |
| Storage folder missing | "Backup failed" (create_backup_file_error) | One warning, retry (E-21) |

---

## 4. Test plan

Each test is named by the id in the first column. Tests live in `tests/backup/` (integration) and `#[cfg(test)]` modules under `src/backup/` (unit).

### 4.1 Wire codec (T-21a-001)

| Test | Assertion |
|---|---|
| `wire_varint_roundtrip` | 0, 1, 127, 128, 300, `u64::MAX` encode/decode identically |
| `wire_negative_int64_is_10_bytes` | `-1i64` encodes to 10 bytes `FF FF FF FF FF FF FF FF FF 01` |
| `wire_negative_int32_is_sign_extended` | `-1i32` encodes to the same 10 bytes |
| `wire_fixed32_float_le` | `1.5f32` encodes to `00 00 C0 3F` with tag wire 5 |
| `wire_fixed64_double_le` | `-1.0f64` encodes to `00 00 00 00 00 00 F0 BF` with tag wire 1 |
| `wire_string_utf8` | `"héllo 🙂"` round-trips; invalid UTF-8 -> `Utf8` error |
| `wire_unknown_field_skipped` | payload with tag 999 (varint, len, fixed32, fixed64) decodes as if absent |
| `wire_field_zero_rejected` | tag 0 -> error |
| `wire_truncated_len` | length prefix larger than buffer -> `Truncated`, no panic |
| `wire_varint_too_long` | 11 continuation bytes -> `VarintTooLong` |
| `wire_wrong_type_known_field` | field 1 of `BackupManga` as wire 0 -> `WrongWireType` |
| `wire_missing_required` | `BackupManga` without field 2 -> `MissingField("url")` |
| `wire_bad_enum` | `update_strategy = 9` -> `BadEnum` |
| `wire_packed_and_unpacked_categories` | `categories` in both encodings decode to the same `Vec<i64>` |
| `wire_empty_message_defaults` | empty `BackupManga` payload of required fields only -> defaults per data doc |
| `wire_round_trip_all_structs` | proptest or table: encode then decode equals the input for each message type |

### 4.2 Format sniffing and legacy detection (T-21a-007, 008)

| Test | Input bytes | Expected |
|---|---|---|
| `sniff_gzip` | `1F 8B ...` | `Gzip` |
| `sniff_json_brace_brace` | `7B 7D` | `JsonForbidden` |
| `sniff_json_quote` | `7B 22` | `JsonForbidden` |
| `sniff_json_newline` | `7B 0A` | `JsonForbidden` |
| `sniff_json_leading_space` | `20 7B 22` | `Raw` -> decode error `Corrupt` (E-02) |
| `sniff_one_byte` | `1F` | `Truncated` -> `Corrupt` (E-01) |
| `sniff_empty` | `` | `Truncated` |
| `legacy_detect_true` | fields: 3 (anime), 103 (one source), no 500 | legacy |
| `legacy_detect_false_current` | fields: 500 = false, 501 (anime) | current |
| `legacy_detect_false_mihon` | fields: 1, 2, 101, 104 only | current |
| `legacy_detect_false_no_sources` | no 500, no 103, has 3 | current (documented weakness B-12) |

### 4.3 Creation pipeline (T-21a-012 .. 018)

Seeded in-memory SQLite with: 3 manga (one favourite with 2 chapters, one non-favourite read, one excluded scanlator), 2 anime (one favourite with 2 episodes and tracking, one non-favourite watched), 2 manga categories (one system id 0), 1 anime category, history rows, 1 custom button, 1 anime store, 1 manga repo, 2 configurable sources with preferences (one with a private key), app prefs including `__APP_STATE_` and `__PRIVATE_` keys.

| Test | Assertion |
|---|---|
| `option_matrix_gating` | For each of the 12 options toggled alone (others at default) the output contains exactly the expected sections (B-06 gating summary) |
| `library_off_drops_entries_and_nonfavourites` | `libraryEntries=false`, `readEntries=true` -> no anime or manga entries; the non-favourite queries are not executed (E-09; use a query-counting wrapper) |
| `read_entries_off` | non-favourites absent |
| `chapters_off_keeps_exclusions` | `chapters=false` -> chapters empty, `excludedScanlators` present (E-25) |
| `system_category_excluded` | id 0 not in `categories` (B-08c) |
| `category_link_is_order` | an entry in category id 5 with order 2 writes `categories = [2]` |
| `tracking_writes_media_id_only` | `media_id_int` (3) absent, 100 present (E-19) |
| `history_uses_urls` | history rows carry chapter/episode URL, not id; manga `readDuration` copied; anime `readDuration` = 0 |
| `background_url_written` | anime with background -> 500 present (E-26) |
| `source_list_distinct_first_seen` | two entries from source A then B -> `[A, B]` |
| `stub_name_empty_when_unknown` | unknown source id -> name "" in `sources`, preflight label shows id (E-17) |
| `app_prefs_exclude_app_state_and_private` | with private off: no `__APP_STATE_`, no `__PRIVATE_`; with private on: `__PRIVATE_` present, `__APP_STATE_` still absent |
| `source_prefs_only_non_empty` | a source with zero prefs is omitted |
| `value_types_preserved` | Int, Long, Float, String, Boolean, Set<String> each keep their variant |
| `private_disabled_when_parents_off` | E-27: `privateSettings=true` with app and source off -> stored option reset to false |
| `extensions_opt_in` | default options -> no 504 entries; `extensions=true` -> one entry per installed package |
| `artifact_missing_skipped` | E-16: fake artifact source raising not-found for one package -> others written, warning recorded |
| `interval_setting_clamped` | E-13 (in options/settings tests) |
| `overflow_fails` | E-14: `status` = 2^40 -> `build_backup` returns error; row id in message |
| `memo_invalid_writes_empty_object` | E-28 |
| `unicode_titles_roundtrip` | E-29 with emoji, RTL, combining marks, 4096-character title |
| `float_precision_documented` | E-07: 1234.5678 becomes the nearest float32; test asserts the stored value equals `1234.5678f32` (documents loss) |

### 4.4 File write, retention, names (T-21a-022, 025, 036)

| Test | Assertion |
|---|---|
| `write_uses_part_then_rename` | no final file exists until rename; fault injection at write, fsync, rename |
| `failure_removes_part` | any injected error leaves no `.part` (E-30) |
| `manual_overwrite_truncates` | existing larger file replaced fully (no trailing bytes) |
| `prune_keeps_four_newest` | 6 matching files plus 2 unrelated -> after an automatic run: 4 matching + 2 unrelated |
| `prune_after_write` | fault injection at prune does not lose the new file; fault injection at write does not delete existing files (E-04) |
| `prune_ignores_non_matching` | `kurobayomi_x.tachibk`, `notes.txt`, `xyz.jmir.tachiyomi.mi_*.tachibk` untouched |
| `regex_dot_escaped` | `kurobayomiX2026-10-08_19-31.tachibk` does not match |
| `name_collision_suffix` | existing `kurobayomi_2026-10-08_19-31-05.tachibk` -> new name `...-05-2.tachibk` |
| `name_format_local_time` | fixed clock with TZ `Europe/Berlin`: name uses local hour |
| `stale_part_removed_on_start` | `*.tachibk.part` in automatic dir removed on startup, real backups kept |
| `automatic_dir_created` | missing dir created |
| `automatic_dir_missing_base` | base unavailable -> error `Io`, one warning (E-21) |

### 4.5 Self-check and preflight (T-21a-023, 024)

| Test | Assertion |
|---|---|
| `self_check_decodes_written_file` | every fixture written by the creator decodes |
| `self_check_failure_deletes` | injected corrupt payload -> file deleted, `Corrupt` |
| `preflight_missing_manga_source_label` | unknown installed id with stub label -> "Name (EN)" |
| `preflight_numeric_name_is_id` | E-18: name "12345" -> stub label used |
| `preflight_blank_name_shows_id` | E-17 |
| `preflight_tracker_not_logged_in` | tracker with `logged_in=false` listed; logged-in not listed |
| `preflight_unknown_tracker_ignored` | tracker id 999 not listed |
| `preflight_distinct_sorted` | duplicates removed, sorted, manga before anime |
| `preflight_decode_error_message` | E-12: message is the key text, not `java.io.IOException:` |

### 4.6 Scheduler (T-21a-026, 032, 037)

Table test over `decide(now, cfg, restore_running, power, manual_running, retry)`:

| Case | Expected |
|---|---|
| interval 0 | Off |
| never run, enabled at T, now < T + interval | NotDue(T + interval) (E-08) |
| last = now - interval exactly | Run |
| restore running | Defer(RestoreRunning) |
| on battery, 14% | Defer(BatteryLow) |
| on battery, 15% | Run |
| AC power, 5% | Run |
| battery percent unknown | Run (treat unknown as not low; document) |
| manual running | Defer(ManualRunning) |
| retry attempt 3, base 10 min, interval 6 h | Backoff until last_attempt + 40 min |
| retry attempt 10 | backoff capped at interval |
| last in the future (E-31) | treated as now; not frozen |
| interval changed from 12 to 6 while due | Run on next tick |
| interval value 3 (invalid, E-13) | loaded as 12 |

### 4.7 Options (T-21a-010, 011)

| Test | Assertion |
|---|---|
| `defaults_match_kotlin` | 10 true, `privateSettings=false`, `extensions=false` |
| `as_bool_array_order` | index order exactly as B-02 |
| `from_bool_array_roundtrip` | identity for all 4096 combinations (cheap) |
| `from_short_array_panics_or_errors` | returns error (Rust: `Option`/`Result`, not panic) |
| `can_create_matrix` | 64 combinations of the six fields in `canCreate()`; only extensions-only, chapters-only, tracking-only, history-only, readEntries-only, privateSettings-only, extensions-only cases are false (E-06) |
| `disable_rules` | chapters/tracking/history/readEntries disabled iff `libraryEntries=false`; privateSettings disabled iff neither appSettings nor sourceSettings |
| `toggle_keeps_children` | libraryEntries off then on restores chapters/tracking/history/readEntries values |
| `labels_match_mr_keys` | label strings equal the English texts of B-02 |

### 4.8 Notifications (T-21a-028, 029, 030)

| Test | Assertion |
|---|---|
| `restore_duration_format` | 0 ms -> "00 min, 00 sec"; 59_999 -> "00 min, 59 sec"; 61_000 -> "01 min, 01 sec" |
| `plural_errors` | 0 -> "Done in ... with 0 errors", 1 -> "error", 2 -> "errors" |
| `hide_content_restore_only` | `hide_content=true` -> restore progress body empty; backup texts unchanged |
| `error_action_only_when_errors` | errors=0 -> no details action; errors>0 with path -> action present |
| `progress_replaces_by_id` | two progress updates use the same notification id |
| `progress_closed_on_finish` | handle closed on success and on failure |
| `create_complete_action` | Manual completion shows "Show in folder". Automatic success is silent (Proposal; Kotlin shows nothing for automatic success either); automatic failure shows the warning described in E-10 |

### 4.9 Single-flight and job (T-21a-027)

| Test | Assertion |
|---|---|
| `second_manual_refused` | while first holds slot, `start_manual` -> `InProgress` |
| `auto_tick_skipped_during_manual` | decide returns Defer(ManualRunning) |
| `slot_released_on_error` | after an injected failure, slot is free |
| `slot_released_on_panic` | guard dropped during unwind |

### 4.10 Round trip with Aniyomi fixtures (T-21a-034, 040)

Fixtures (to be produced by the user with the Aniyomi app, or supplied by the user; they are NOT invented here):

| Fixture | How to produce | Used by |
|---|---|---|
| `fixtures/aniyomi/current-all.tachibk` | Aniyomi: Create backup with every checkbox on, extensions on, private settings on; include one manga and one anime with chapters, tracking (one tracker logged in), history, one custom button, one anime store, one manga repo, one source with preferences | round-trip, sealed value layout, enum ordinals |
| `fixtures/aniyomi/current-default.tachibk` | Aniyomi: default options only | defaults, `isLegacy=false` |
| `fixtures/aniyomi/auto-backup.tachibk` | Aniyomi automatic folder file | name regex (negative and positive), decode |
| `fixtures/aniyomi/legacy-1x.tachibk` | an Aniyomi file made before the 1.0 format (needs a real file from the user) | legacy detection and conversion |
| `fixtures/mihon/current.tachibk` | Mihon backup with no anime | reads as current (Kurobayomi must accept Mihon-shaped manga) |
| `fixtures/generated/schema-dump.txt` | output of the Aniyomi debug screen "Backup file schema" | T-21a-035 comparison (field names and numbers) |
| `fixtures/corrupt/*.bin` | truncated gzip, wrong wire type, bad enum, missing required, invalid UTF-8 | error mapping (E-11, E-35, E-37) |
| `fixtures/json/brace.json` | file starting `{"` | E-02 |
| `fixtures/bytes/one.bin` | single byte `1F` | E-01 |

Golden tests:
- `golden_decode_current_all`: decode `current-all.tachibk`; assert manga title, chapter count, tracking `remote_id`, source names, preference variants, enum values match the Aniyomi UI values captured with the fixture (record a text manifest next to the fixture).
- `golden_reencode_semantics`: decode then re-encode then decode: structures equal (bytes need not be equal; gzip levels differ).
- `kotlin_reads_ours` (manual, documented): create a Kurobayomi backup, import in Aniyomi, compare entry counts. Recorded as an acceptance step for T-21a-034, not an automated test.

### 4.11 Integration (T-21a-040)

- `end_to_end_manual`: seeded DB -> `create_file` to a temp dir -> decode -> assert totals.
- `end_to_end_auto_retention`: 6 runs with a fake clock -> 4 files remain, newest kept.
- `end_to_end_restore_running_defers`: fake restore flag -> auto run deferred, no file written.

---

## 5. New tests to write (list for the test task)

1. Wire codec unit tests (4.1).
2. Format sniff and legacy detection (4.2).
3. Creator matrix with in-memory SQLite (4.3).
4. File write, prune and names with a temp directory (4.4).
5. Preflight (4.5).
6. Scheduler decision table (4.6).
7. Options (4.7).
8. Notification texts and durations (4.8).
9. Single-flight (4.9).
10. Golden fixtures (4.10).
11. End-to-end (4.11).

## 6. Fixtures needed (summary)

- Seed SQL file for the in-memory DB (uses the area:03/04 schema).
- Fake registries: sources (installed, stub with name, stub blank), trackers (logged in, not logged in), extension artifacts (one present, one missing), prefs (app: normal, `__APP_STATE_`, `__PRIVATE_`; source: `source_1` with two keys, `source_2` empty).
- Aniyomi fixtures listed in 4.10 (must come from the user's app; flagged as blocking for T-21a-034).
- Clock fixture (fixed `now`, timezone `Europe/Berlin`).
