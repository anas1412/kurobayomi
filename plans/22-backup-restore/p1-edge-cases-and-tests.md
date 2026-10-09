# 22-backup-restore — Part 1 edge cases, failure modes, tests

Upstream tests: **none**. A search of every scope file found no `*Test.kt`, no test fixtures and no test resources under the restore path. Every test below is new. Fixture work is in T-22a-042 and T-22a-051.

Numbering: `E-nn` edge cases, `R-nn` races, `F-nn` failure modes, `X-nn` test cases, `Q-nn` fixtures.

---

## 1. Input file edge cases

### E-01 Empty file (0 bytes)
- Observed: `source.peek().require(2)` throws `EOFException` (`BackupDecoder.kt:106-108`), not wrapped. Validation wraps it as `IllegalStateException(e)` (`BackupFileValidator.kt:26-27`), so the invalid card shows `java.lang.IllegalStateException: java.io.EOFException...` (D19). The job shows the raw message.
- Kurobayomi: `RestoreError::Decode("Backup file is empty")`. Show "Invalid backup file:" + path + "Full error:" + the same text.
- Test: X-01.

### E-02 One-byte file
- Same as E-01 (`require(2)` fails). Kurobayomi: same text as E-01. Test: X-01.

### E-03 Gzip magic with corrupt body
- Observed: `source.gzip()` raises an `IOException` from okio while reading. Not a `SerializationException`, so it is not converted to "Backup file is corrupted". Message text is okio's.
- Kurobayomi: map any gzip or read error to `Decode("Backup file is corrupted")` and keep the detail for the "Full error" line.
- Test: X-02.

### E-04 Text or JSON file that is not gzip and not JSON-signature
- A file beginning `[`, a BOM, or leading whitespace is not rejected by the magic check; it is decoded as protobuf. Random text may decode with no error to an almost empty `Backup` (unknown fields are skipped by kotlinx `ProtoBuf`; required-field absence raises an error only for some field types). The restore then succeeds with zero work, showing "Done in 00 min, 00 sec".
- Observed risk: a wrong file "restores" silently.
- Kurobayomi: after decode, if the backup has no manga, no anime, no preferences, no categories and no extensions, return `Decode("Backup does not contain any library entries.")` (uses the unused key `invalid_backup_file_missing_manga`, D20). PROPOSAL; deviates from Kotlin on purpose.
- Test: X-03 (text file, JSON file with leading space, 2-byte file `"\x0a\x00"`).

### E-05 JSON backup (`{}`, `{"`, `{\n`)
- Observed: rejected with `invalid_backup_file_json` ("JSON backup not supported") (`BackupDecoder.kt:112-113`). Other JSON starts go to protobuf (E-04).
- Kurobayomi: same rejection for the three signatures. Test: X-04.

### E-06 Legacy detection false positive
- A new Mihon-compatible file that happens to contain field 103 without field 500 is detected as legacy (because `isLegacy` defaults to true). Only field 103 non-empty is checked after that. Mihon backups have no 103, so this does not affect them.
- Test: X-05 (synthetic file with 103 only, expected legacy; file with 500=false and 103, expected not legacy).

### E-07 Legacy file conversion loses no data
- `LegacyBackup.toBackup()` copies all mapped fields; fields 100 and 102 are not decoded. Test: X-06 (legacy fixture: counts of manga, anime, categories, sources, extensions equal the expected counts after conversion).

### E-08 Non-ASCII and unusual file names
- Kotlin accepts any URI string. Linux: paths may contain newlines, spaces, non-UTF-8 bytes. Use `PathBuf`, never `String` for paths; display with `to_string_lossy`. Test: X-07 (path with space, unicode, and an invalid UTF-8 byte).

---

## 2. Validation edge cases

### E-09 Source ids that do not parse as numbers
- `BackupFileValidator` uses `name` as the label when `name.toLongOrNull() == null`, otherwise `getOrStub(id)`. A source named "1234" that is actually a stub gets a stub label. Test: X-08.

### E-10 Missing source with duplicate names
- Missing manga names and anime names are each `distinct().sorted()`, then concatenated without re-sorting (`BackupFileValidator.kt:32-55`). Two source ids with the same name both appear once in the list. Kurobayomi: the lists are shown in the same order. Test: X-09 (manga "B", anime "A", manga "A": output `["A","B","A"]`... expected order: manga sorted distinct then anime sorted distinct).

### E-11 Tracker not logged in vs unknown tracker id
- Unknown `syncId` (no tracker registered) is silently ignored (not reported as missing). Tracker present and logged out is reported. Test: X-10.

### E-12 Validation result does not block restore
- `missingSources` or `missingTrackers` non-empty leaves `canRestore = true` (`RestoreBackupScreen.kt:203-209`). Entries are restored even though their source is not installed. Test: X-11 (UI model: restore enabled with missing sources).

### E-13 Validation runs before options and again in the job
- The file is decoded twice (once for validation on the UI side, once in the job). For a 200 MB backup this doubles time and memory. Kurobayomi: cache the decoded `Backup` in the restore page until the job starts; the job then reuses it (PROPOSAL). Test: X-12 (decode counter).

---

## 3. Restore semantics edge cases

### E-14 Options all false
- UI disables Restore. If the job starts anyway (only possible through code), `run` completes with zero work and a success notification "Done in 00 min, 00 sec". Kurobayomi: refuse with `RestoreError::Io("Nothing selected to restore")`? PROPOSAL: return `Failed` with "Nothing selected" and do not write a log. Test: X-13.

### E-15 Categories off but entries carry category memberships
- Category restore is skipped and entry membership is skipped (`emptyList()`), so entries keep their existing memberships. For a new entry the membership is empty. Test: X-14.

### E-16 Duplicate backup category names (not in DB)
- `dbCategoriesByName` is built once before the map loop (`MangaCategoriesRestorer.kt:216-222`), so a name repeated in the backup creates two DB categories with the same name. Same for anime. Kurobayomi: keep a mutable name map updated inside the loop (PROPOSAL, deviates from Kotlin, which creates duplicates). Test: X-15.

### E-17 Duplicate custom button names
- Same pattern as E-16 for `CustomButtonRestorer.kt:43,50-65`. Kurobayomi: mutable map. Test: X-16.

### E-18 Category order gaps and max order
- `nextOrder = max(order) + 1` or 0 when empty. Gaps are preserved, new categories are appended after the max. Test: X-17 (orders {0, 5} → new order 6).

### E-19 Entry matched by (url, source) with different case or trailing slash
- Lookup is exact string equality. A backup URL that differs only by case or a trailing slash creates a new entry. Test: X-18 (documented behaviour).

### E-20 Same entry twice in one backup
- Second occurrence finds the row inserted by the first and runs the merge path. No error. Test: X-19 (two identical manga records → one manga row, chapters not duplicated).

### E-21 Favourite never removed
- `favorite = this.favorite || newer.favorite` (`MangaRestorer.kt:94-95`). A restore never unfavourites. Test: X-20.

### E-22 Missing `favorite` field
- Absent field 100 means `favorite = true` (D22). A file from a writer that omits the field makes every entry a library entry. Test: X-21 (proto without field 100 → favorite true).

### E-23 Metadata direction depends on version (D5)
- Backup version greater than DB version: DB keeps title, viewer flags, chapter flags, update strategy, date added, last update; backup metadata (author, artist, description, genre, thumbnail, status) wins. Backup version less or equal: backup keeps those non-metadata fields; DB metadata wins. Test: X-22 (four cases: newer/older × title and viewer flags).

### E-24 `initialized` not persisted for new entries (D6)
- New manga row gets `initialized = false`. The in-memory object says `description != null`. Test: X-23 (insert a new manga with description; read row; expect initialized = 0 in the kept behaviour, or 1 with PROPOSAL fix; decide in T-22a-013).

### E-25 `is_syncing` left at 1
- Restore writes `isSyncing = 1` on manga and anime rows (`MangaRestorer.kt:131`, `AnimeRestorer.kt:147`) and `0` on chapters and episodes only when updating (`MangaRestorer.kt:230`, `AnimeRestorer.kt:264`). No reset for entry rows is in the restore scope. Kurobayomi: write `is_syncing = 0` at the end of each entry transaction (PROPOSAL), or document that the flag is a sync-in-progress marker owned by area:03/04. Test: X-24.

### E-26 `lastModifiedAt` and `favoriteModifiedAt` not written
- The insert and update argument lists do not include these fields, so restored values are dropped. Sorting uses `lastModifiedAt` from the backup object only. Kurobayomi: store both (PROPOSAL) or document loss. Test: X-25.

### E-27 Chapter number float artefact
- `chapterNumber` is `Float` on the wire and widened with `toDouble()` (`BackupChapter.kt:30`). Example: wire value for 1.1 becomes `1.100000023841858`. For existing chapters the DB value is kept (copyFrom), so only new chapters carry the artefact. The comparison `forComparison()` then compares the widened value with the stored DB value. Kurobayomi: round to 6 decimal places on restore (PROPOSAL, T-22a-016 acceptance). Test: X-26.

### E-28 Track score round trip
- `score` Float to Double (`BackupTracking.kt:47`). Equality on re-restore holds because both sides widen the same way (idempotency). Test: X-27 (restore twice; second restore writes zero track rows).

### E-29 Chapter progress regression (D8)
- Both sides non-zero and backup smaller: backup wins. Test: X-28 (DB lastPageRead 10, backup 5, not read → 5 expected in kept behaviour; PROPOSAL max → 10).

### E-30 Episode progress regression (D8)
- Same as E-29 with `lastSecondSeen`. Test: X-29.

### E-31 Read or seen flag never reverts
- `if (dbChapter.read && !updatedChapter.read)` forces read = true with the DB position (`MangaRestorer.kt:171-175`). A backup can never mark a read chapter unread. Test: X-30.

### E-32 Bookmark OR
- `bookmark = chapter.bookmark || dbChapter.bookmark`. Test: X-31.

### E-33 Episode `fillermark` OR
- `fillermark = episode.fillermark || dbEpisode.fillermark` (`AnimeRestorer.kt:194`). Test: X-32.

### E-34 Chapter metadata ignored for existing rows
- `Chapter.copyFrom(dbChapter)` keeps DB name, number, upload date and scanlator (`domain/.../Chapter.kt:22-30`). A corrected name in the backup is ignored. Test: X-33 (documented, D16).

### E-35 Chapter removed in the backup
- Chapters in the DB that are absent in the backup are not deleted. Test: X-34.

### E-36 Track merge keeps DB status and score (observed merge scope)
- For existing tracks only `remoteId`, `libraryId` and `lastChapterRead` (max) are taken from the backup. Test: X-35.

### E-37 Track for an unknown tracker id is inserted
- The track row is inserted even when the tracker is not registered in this build. Kurobayomi: keep the row (data-preserving) and show it as unavailable. Test: X-36.

### E-38 History for a chapter that does not exist
- Skipped without error (`MangaRestorer.kt:328-331`). Test: X-37.

### E-39 History chapter URL collision across manga (D9)
- Global `getChapterByUrl` attaches history to the first chapter with that URL, which may belong to another manga. Test: X-38 (two manga share chapter URL "/c/1"; history on manga B attaches to manga A's chapter in the kept behaviour; PROPOSAL scopes lookup to the manga).

### E-40 History `readDuration` merge
- New row: `time_read = backup readDuration`. Existing row: `time_read = max(backup, db)` via the delta. Test: X-39 (idempotency of the delta on second run).

### E-41 History readAt zero
- `readAt = null` when both timestamps are zero, so the upsert writes `last_read = NULL` on conflict (`history.sq:61-69`). Test: X-40.

### E-42 Anime history `readDuration` dropped
- `AnimeHistory` has no duration, so `readDuration` is not stored. Test: X-41 (documented loss).

### E-43 Anime season parent pointer (D7)
- Parent processed before the season's top-level turn: season's `parentId` ends as the source-device id. Test: X-42 (fixture F9 in both orders; expected in PROPOSAL: season `parentId` equals the DB id of the parent in both orders).

### E-44 Season without parent in the backup
- Restored as a top-level row with its raw `parentId`. PROPOSAL: `parentId = null`. Test: X-43.

### E-45 Season fetch interval depends on the pass
- `awaitUpdateFetchInterval` runs in `restoreAnimeDetails` (`AnimeRestorer.kt:324`). A season gets it at its top-level turn only. A season written under its parent (nested pass) keeps `next_update` and `calculate_interval` from the write, which are 0 for a new row. Test: X-44 (after restore, every season row has a non-zero next_update in the kept behaviour; PROPOSAL: season interval computed in the same place as the parent).

### E-46 Manga `nextUpdate` and `calculateInterval` recomputed from chapters
- The update SQL receives `null` (keep) for both, then `awaitUpdateFetchInterval` sets `next_update` and `calculate_interval` from the restored chapter dates. Test: X-45 (fixture with chapter upload dates spanning 3 weeks; expected interval computed by `calculateInterval`).

### E-47 Fetch window computed once per run
- `now` is captured in the restorer constructor (`MangaRestorer.kt:37-43`). A restore that crosses midnight uses the start day's window. Test: X-46 (fake clock).

### E-48 DST days
- Local-midnight arithmetic: the window lower and upper bounds use local start of day (`MangaFetchInterval.kt:36-41`). On DST days the day length is 23 or 25 hours; chrono `and_local_timezone` must be used (not `+ 86400 s`). Test: X-47 (America/New_York DST switch day).

### E-49 Exclude scanlators duplicates
- Inserted only if not in the existing list (`MangaRestorer.kt:428`). A duplicate inside the backup list is inserted twice (the query has no unique constraint in the scope). Test: X-48.

### E-50 Genre with ", " inside a name
- Manga genres are stored as one `", "`-joined string (`MangaRestorer.kt:115`). A genre name containing ", " is split when read back. Test: X-49 (documented).

### E-51 Null genre list
- `genre: List<String>` is non-null on the wire but `genre?.joinToString` is applied: restore writes "" for an empty list (`genre = manga.genre?.joinToString(", ")` yields "" for empty). Test: X-50.

### E-52 Source preferences for uninstalled sources
- Restore creates the per-source preference file regardless. Kurobayomi: write the store keyed by `source_<id>` (PROPOSAL: same). Test: X-51.

### E-53 Preference type mismatch
- Existing key with a different type: write skipped silently (`PreferenceRestorer.kt:74,89,94,99,104,109`). Test: X-52 (keep silent but count in summary PROPOSAL).

### E-54 default_category -1 in backup
- Backup id "-1" is not in the backup category map, so newValue is null and the setting is skipped. An existing default is NOT reset to -1. Test: X-53.

### E-55 default_category references a category absent from the backup
- Same as E-54: skipped. Test: X-54.

### E-56 Category id sets accumulate (D12)
- `+=` appends (`PreferenceRestorer.kt:151`). Restoring twice doubles entries in the set (after dedupe by the set type, ids stay unique; but stale ids from earlier restores remain). Test: X-55 (restore twice, expect replace semantics in PROPOSAL).

### E-57 Category set with no mapping
- `restoreCategoriesPreference` returns true when no ids map (`:139-154`), so the whole-set replace does not happen and the existing value stays. Test: X-56.

### E-58 Restoring app preferences re-arms schedulers
- `AnimeLibraryUpdateJob.setupTask`, `MangaLibraryUpdateJob.setupTask` and `BackupCreateJob.setupTask` run after every app-prefs restore, even if the interval did not change (`PreferenceRestorer.kt:45-47`). Test: X-57 (fake scheduler counts calls: 3 per app-prefs restore).

### E-59 Private settings in the file are restored without a separate flag
- The backup's private section (tracker tokens) is restored whenever app or source prefs are restored; there is no option for it on restore. Kurobayomi: same (documented). Test: X-58.

### E-60 Preference keys outside the schema
- Restored as-is (Kotlin). `__APP_STATE_*` keys can be overwritten. PROPOSAL: restrict to schema keys (§5.5 in behaviour). Test: X-59 (key `__APP_STATE_last_auto_backup_timestamp` in file: expected skipped under PROPOSAL).

### E-61 Extension stores: badgeLabel and isLegacy defaults
- `badgeLabel ?: name` and `isLegacy ?: true`. Test: X-60.

### E-62 Extension store upsert overwrites metadata
- Same index URL with different name: updated. Test: X-61.

### E-63 Manga repo duplicate (D11)
- Identical repo always produces "has the same signing key fingerprint". Test: X-62.

### E-64 Manga repo same URL different fingerprint
- Error "Already Exists with different signing key fingerprint". Test: X-63.

### E-65 Manga repo same fingerprint different URL
- Error "<name> has the same signing key fingerprint". Test: X-64.

### E-66 Custom button favourite rule (D10)
- Backup with two favourites and no DB favourite: both inserted as favourite. Test: X-65 (expected in kept behaviour: 2 favourites; PROPOSAL: 1).

### E-67 Custom button sort order
- Backup sorted by `sortIndex` then appended after DB max. Test: X-66.

### E-68 APK extensions on Linux
- Counted as skipped, not errors. Test: X-67 (count in summary; no error line; no file written).

### E-69 Error text with newline
- A message containing "\n" splits the log line into two lines. Kurobayomi: escape newlines in the log (PROPOSAL). Test: X-68.

### E-70 Error log overwrite
- Each restore overwrites `aniyomi_restore_error.txt`. The previous log is lost. Test: X-69.

### E-71 Error log directory missing
- Kotlin: exception swallowed (D15). Kurobayomi: create the directory (`create_dir_all`) and log a warning on failure. Test: X-70.

### E-72 Progress total zero
- Nothing selected or empty backup: total 0, fraction would be NaN. Kurobayomi: fraction 1.0 when total is 0. Test: X-71.

### E-73 Progress counts (D2)
- Categories add 2 to amount; final progress equals amount only with fix. Test: X-72 (every combination of options: final `progress == amount`).

### E-74 Progress reaches amount exactly with seasons
- Seasons count in `backupAnime.size` and also run as top-level entries: progress increments per top-level entry. With the PROPOSAL (skip seasons at top level), amount must exclude them, or progress must count them once. Test: X-73 (with and without seasons, final progress equals amount).

---

## 4. Failure modes

### F-01 Cancel mid-entry (D4)
- Kotlin: the cancel exception is recorded as an error for that entry and the entry rolls back. Kurobayomi: cancel is re-raised, the entry rolls back, no error line. Test: X-74 (cancel flag set during the second entry's chapter loop; expected: first entry committed, second rolled back, no error line for second).

### F-02 Cancel between sections
- Kotlin: `ensureActive()` at section start. Kurobayomi: `check_cancel()` before each section and entry. Test: X-75.

### F-03 Cancel leaves no error log (D17)
- Kotlin: no log, no completion notification, cancel notification only. Kurobayomi: log written, `Canceled` event carries the path. Test: X-76.

### F-04 Process killed mid-restore
- Each entry commits or rolls back atomically. No resume. Test: X-77 (kill the process after N entries in a child process; reopen DB; `PRAGMA integrity_check` ok; re-run completes).

### F-05 SQLite BUSY during entry
- Immediate transactions with `busy_timeout` (PROPOSAL 5000 ms). After timeout the entry fails with the SQLite message and is recorded as an error. Test: X-78 (second connection holds write lock for 6 s; expect entry error).

### F-06 Database write error mid-entry
- Entry rolls back; error line "title [source]: <sqlite text>". Test: X-79.

### F-07 Preference write error
- Logged (not collected). Restore continues. Test: X-80.

### F-08 Decode error in one section after others succeeded
- Decode happens first; no partial writes. Test: X-81 (corrupt file → zero writes; `total_changes == 0`).

### F-09 Missing source at restore time
- Restore continues; entries are stored with their source id. Test: X-82 (already E-12).

### F-10 Cancel while validation runs
- Validation has no cancel (it finishes fast). The page closes; the thread result is dropped. Test: X-83 (manual).

### F-11 Notification send failure
- `send_notification` failure is logged and ignored; restore continues. Test: X-84 (headless run with no notification server; restore completes).

---

## 5. Races

### R-01 Categories versus entries (D3)
- Kotlin: `restoreCategories` and `restoreAnime` run concurrently; an entry can read the category list before the categories exist, dropping memberships silently. Kurobayomi: sequential sections. Test: X-90 (run 200 iterations of restore with categories; memberships always present).

### R-02 Categories versus app prefs default category
- Same race for `default_category`. Kurobayomi: sequential. Test: X-91.

### R-03 Progress counter races
- Kotlin: non-atomic increments from several coroutines. Kurobayomi: single thread. Test: X-92 (final progress equals amount, repeated 100 times).

### R-04 Two restores started at once
- Kotlin: `ExistingWorkPolicy.KEEP` keeps the first. Kurobayomi: `JobSlot::start` returns `AlreadyRunning`. Test: X-93 (second start returns Err).

### R-05 Restore while UI writes
- The library screen may write (favourite toggle) during restore. Immediate transactions serialize. Test: X-94 (concurrent writer thread; both finish; no data loss for the UI write).

### R-06 Auto-backup during restore
- Kotlin: auto backup returns `Result.retry()` while restore runs (`BackupCreateJob.kt:42`). Kurobayomi: the scheduler checks `JobSlot::is_running()` and skips the tick. Test: X-95.

---

## 6. Performance and limits

- Entries are processed one per transaction. Expected cost is dominated by SQLite commits; batching several entries per transaction would be faster but changes failure granularity. Keep one per entry (failure isolation is more valuable).
- Memory: the whole file is read into memory (`BackupDecoder.kt:116`), and APK bytes stay in memory. Kurobayomi: same; document the limit (no streaming in protobuf decode).
- Performance target (PROPOSAL): restoring 10,000 manga with 200 chapters each completes in under 10 minutes on an SSD; 2,000 manga in under 60 seconds. Test: X-96 (benchmark, ignored in CI, run manually).

---

## 7. Fixtures

| ID | File | Purpose | Expected counts |
|---|---|---|---|
| Q-01 | `fixtures/restore/mihon_1x_small.tachibk` | Mihon-style backup (no 5xx fields): 3 manga, 1 category, 2 chapters each, 1 history, 1 track (syncId 1, trackers not logged in) | manga 3, categories 1, chapters 6, history 1, tracks 1 |
| Q-02 | `fixtures/restore/aniyomi_legacy_0x.tachibk` | 0.x legacy file: fields 1-4 and 103 populated | anime 2, manga 1, anime categories 1, anime sources 1 |
| Q-03 | `fixtures/restore/aniyomi_current.tachibk` | current Aniyomi format (5xx), gzip | anime 4 (2 seasons), manga 3, extension stores 1, custom buttons 2 (one favourite) |
| Q-04 | `fixtures/restore/aniyomi_current_raw.tachibk` | same as Q-03 but not gzipped | same as Q-03 |
| Q-05 | `fixtures/restore/truncated.tachibk` | first 100 bytes of Q-03 | Decode error "corrupted" |
| Q-06 | `fixtures/restore/empty.tachibk` | 0 bytes | EOF error (E-01) |
| Q-07 | `fixtures/restore/one_byte.tachibk` | 1 byte `0x0a` | EOF error |
| Q-08 | `fixtures/restore/text.txt` | ASCII text "hello world" | decodes to empty backup under Kotlin; rejected under PROPOSAL |
| Q-09 | `fixtures/restore/json_brace.json` | `{"a":1}` | JSON rejection message |
| Q-10 | `fixtures/restore/dup_categories.tachibk` | backup has two categories named "Reading" and an entry referencing both orders | category rows: 2 (kept) or 1 (PROPOSAL) |
| Q-11 | `fixtures/restore/seasons_parent_first.tachibk` | parent order before season in sortByNew | season parentId: correct in PROPOSAL |
| Q-12 | `fixtures/restore/seasons_season_first.tachibk` | season sorted first | season parentId correct in both |
| Q-13 | `fixtures/restore/float_numbers.tachibk` | chapter numbers 1.1, 2.5, 10.0; track scores 7.3, 8.5 | stored values per E-27 |
| Q-14 | `fixtures/restore/prefs_mixed.tachibk` | prefs: int default_category referencing known and unknown ids, StringSet category keys, type mismatch key, `__APP_STATE_last_auto_backup_timestamp` | per behaviour §5 |
| Q-15 | `fixtures/restore/repos_dup.tachibk` | two manga repos: identical to existing; same URL different fingerprint; same fingerprint different URL | errors: 3 lines (kept) |
| Q-16 | `fixtures/restore/buttons_two_fav.tachibk` | two custom buttons isFavorite true, DB has none | favourites: 2 (kept) or 1 (PROPOSAL) |
| Q-17 | `fixtures/restore/history_collision.tachibk` | manga A and B both have chapter url "/c/1"; history for B | attach per E-39 |
| Q-18 | `fixtures/restore/no_favorite_field.tachibk` | manga with field 100 absent | favorite = true (D22) |
| Q-19 | `fixtures/restore/apk_ext.tachibk` | 2 BackupExtension entries with small byte arrays | skipped 2, errors 0 |
| Q-20 | `fixtures/restore/large_10k.tachibk` | 10,000 manga, 200 chapters each (generated) | benchmark only |
| Q-21 | `fixtures/restore/export_roundtrip.tachibk` | created by the Kurobayomi backup creator (area:21) then restored into an empty DB | round trip equality of entries |

Generation: every fixture is produced by a small Rust generator under `tests/fixtures_gen/` (no Kotlin dependency). Each fixture's expected counts are written next to it as `expected.json`. Q-01 to Q-04 also include one byte-for-byte capture from the Kotlin app (T-22a-051) to pin the sealed `PreferenceValue` and enum wire forms.

Database fixtures: an in-memory SQLite created from the Kurobayomi schema (areas 03 and 04). Kurobayomi tests must not depend on Kotlin test databases.

---

## 8. Test cases (X-nn)

Unit (no database):
- X-01 .. X-04 decode errors and JSON rejection (Q-06, Q-07, Q-08, Q-09).
- X-05 legacy detection truth table.
- X-06 legacy conversion (Q-02).
- X-07 path handling (non-UTF-8, spaces, unicode).
- X-08 stub label for numeric names (fixture from validator).
- X-09 missing source ordering (E-10): manga missing {"B", "A", "A"} and anime missing {"C"} give `["A", "B", "C"]` only if the lists are merged; the kept Kotlin rule gives manga part sorted distinct `["A", "B"]` followed by anime part `["C"]`, i.e. `["A", "B", "C"]`. Test both orders of insertion.
- X-10 unknown tracker versus logged-out tracker.
- X-11 UI model: restore enabled with missing sources.
- X-12 decode counter (validation and job share one decode when cached).
- X-13 nothing selected refuses (E-14).
- X-17 category order with gaps (E-18).
- X-18 URL exact-match (E-19).
- X-22 metadata direction four cases (E-23).
- X-24 `is_syncing` after entry (E-25).
- X-25 lastModifiedAt and favoriteModifiedAt persistence (E-26).
- X-26 chapter number rounding (E-27).
- X-27 track score idempotency (E-28).
- X-28, X-29 progress regression cases (E-29, E-30).
- X-30 read flag never reverts (E-31).
- X-31, X-32 bookmark and fillermark OR (E-32, E-33).
- X-33 chapter metadata ignored (E-34).
- X-34 chapter deletion not performed (E-35).
- X-35 track merge scope (E-36).
- X-36 unknown tracker row kept (E-37).
- X-37 history for missing chapter skipped (E-38).
- X-39 history delta idempotency (E-40).
- X-40 history readAt null (E-41).
- X-41 anime history duration dropped (E-42).
- X-44 season fetch interval per pass (E-45).
- X-45 fetch interval from chapter dates (E-46).
- X-46 fake clock window (E-47).
- X-47 DST window (E-48).
- X-48 scanlator duplicates (E-49).
- X-49 genre join (E-50).
- X-50 empty genre list (E-51).
- X-51 source prefs for uninstalled source (E-52).
- X-52 type mismatch skip (E-53).
- X-53, X-54 default category -1 and unknown (E-54, E-55).
- X-55 category string set replace (E-56).
- X-56 no-mapping keeps existing set (E-57).
- X-57 scheduler re-arm count (E-58).
- X-58 private settings restored without option (E-59).
- X-59 app-state key handling (E-60).
- X-60 extension store defaults (E-61).
- X-61 extension store overwrite (E-62).
- X-62, X-63, X-64 repo conflicts (E-63, E-64, E-65).
- X-65, X-66 custom buttons (E-66, E-67).
- X-67 APK skip (E-68).
- X-68 newline in error text (E-69).
- X-69 log overwrite (E-70).
- X-70 log directory creation (E-71).
- X-71 zero total fraction (E-72).
- X-72 progress equals amount for each option combination (E-73).
- X-73 progress with seasons (E-74).
- X-74 cancel mid-entry (F-01).
- X-75 cancel between sections (F-02).
- X-76 cancel writes log (F-03).
- X-77 kill mid-restore integrity (F-04).
- X-78 busy timeout (F-05).
- X-79 write error rollback (F-06).
- X-80 preference error continues (F-07).
- X-81 decode error writes nothing (F-08).
- X-82 missing source restores (F-09).
- X-83 cancel during validation (F-10, manual).
- X-84 notification failure ignored (F-11).
- X-90 categories versus entries race (R-01).
- X-91 categories versus default category (R-02).
- X-92 progress counter (R-03).
- X-93 two starts (R-04).
- X-94 concurrent UI writer (R-05).
- X-95 auto-backup skip (R-06).
- X-96 benchmark (manual).

UI-model and presentation tests (no GTK main loop needed; the model structs are tested directly):
- X-200 Create option enable rules: Chapters, Tracking, History, All read/watched disabled when Library entries off; Include sensitive disabled unless App settings or Source settings on (`BackupOptions.kt:46-110`).
- X-201 Create action disabled when `canCreate()` is false (only Tracking or only Extensions checked).
- X-202 Create click while the manual create job runs yields the `backup_in_progress` text, not a save dialog.
- X-203 Restore card text variants match behaviour §1.2 and UI parity §1.2 character for character (Missing with sources only, trackers only, both; Invalid).
- X-204 Restore action enable matrix: (canRestore, anyOption) in {(T,T),(T,F),(F,T),(F,F)} → enabled only for (T,T).
- X-205 StorageInfo fraction: total 100, available 25 → 0.75; total -1 → hidden bar; total 0 → hidden bar; available > total → clamped to 0.0.
- X-206 StorageInfo caption: "Available: {avail} / Total: {total}" with `glib::format_size` values.
- X-207 RestoreOptions bit array round trip for all 128 combinations; label order equals the Kotlin order.
- X-210 ComicInfo export of an SManga with title, description, author, artist, genre, status 1 (ONGOING) produces the expected XML (element order: Series, Summary, Writer, Penciller, Genre, PublishingStatusTachiyomi; attributes xmlns:xsd and xmlns:xsi on the root).
- X-211 ComicInfo import: `copyFromComicInfo` genre joins genre, tags, categories with ", " after distinct and trim; artist joins penciller, inker, colorist, letterer, coverArtist after splitting on ", ".
- X-212 Publishing status mapping both ways for all seven entries; unknown string maps to UNKNOWN (0).
- X-213 Duplicate-after-trim genre values stay duplicated (E-ComicInfo: "Action" and " Action" → "Action, Action"), kept for parity; test documents it.
- X-214 ComicInfo with missing optional elements (no Writer) leaves author unchanged.
- X-215 ComicInfo with empty `Series` value sets title to "" (kept).
- X-216 details.json round trip for MangaDetails, AnimeDetails, ChapterDetails (chapter_number required), EpisodeDetails (fillermark default false).

Integration (database, fixtures):
- X-100 Q-01 restore into empty DB → counts equal expected.
- X-101 Q-03 restore into empty DB → counts equal expected; seasons linked (season parent ids are DB ids of the parent).
- X-102 restore Q-03 twice → second run: `total_changes` delta is 0 for entries, history, tracks, categories; preference StringSets are the only allowed change (until D12 fix).
- X-103 Q-10 duplicate categories (per E-16 decision).
- X-104 Q-11 and Q-12 seasons both orders → identical final state.
- X-105 Q-13 float values stored.
- X-106 Q-14 preferences.
- X-107 Q-15 repos.
- X-108 Q-16 favourites.
- X-109 Q-17 history collision.
- X-110 Q-18 favourite default.
- X-111 Q-19 APK skip count.
- X-112 Q-21 round trip with area 21 creator.
- X-113 restore into DB that already contains the same entries with newer and older versions (E-23 cases end-to-end).

Golden comparison (manual, once per release): for Q-01..Q-04 compare the Kurobayomi result table with the expected counts and key columns produced by reading the same files with the Kotlin restore (run in an Android emulator; record the table dump as `expected.json`). Owner: T-22a-042.

---

## 9. Coverage of the scope files by tests

| Scope file | Tests |
|---|---|
| CreateBackupScreen.kt | X-200 option enable rules (UI model), X-201 create disabled when `canCreate` false, X-202 toast on running job |
| RestoreBackupScreen.kt | X-11, X-12, X-203 card text variants (exact strings), X-204 action enable matrix |
| StorageInfo.kt | X-205 fraction clamp and unknown size, X-206 caption format |
| BackupRestoreJob.kt | X-93, X-74..X-76, X-84 |
| BackupRestorer.kt | X-72, X-73, X-101, X-102, X-104 |
| RestoreOptions.kt | X-207 bit array round trip and label order |
| AnimeCategoriesRestorer.kt, MangaCategoriesRestorer.kt | X-16, X-17, X-103 |
| AnimeExtensionStoreRestorer.kt | X-60, X-61 |
| AnimeRestorer.kt | X-22, X-24..X-27, X-35..X-37, X-41, X-42..X-45, X-101, X-104 |
| CustomButtonRestorer.kt | X-65, X-66 |
| ExtensionsRestorer.kt | X-67 |
| MangaExtensionRepoRestorer.kt | X-62, X-63, X-64 |
| MangaRestorer.kt | X-18..X-40, X-46..X-50, X-100 |
| PreferenceRestorer.kt | X-51..X-59, X-106 |
| core-metadata ComicInfo.kt | X-210..X-215 (p1-tasks T-22a-036/037) |
| core-metadata details models | X-216 round trip of details.json fixtures |

---

## 10. Test infrastructure decisions

- Rust `#[cfg(test)]` modules next to the code, plus `tests/restore_integration.rs` for database tests.
- SQLite in-memory via `rusqlite::Connection::open_in_memory()` plus the schema SQL from areas 03/04.
- A `FakeClock` trait for `now` and the fetch window (the Kotlin uses `ZonedDateTime.now()` in the restorer constructor).
- A `FakeSources` and `FakeTrackers` for validation tests.
- A `FakePrefStore` for the preference restorer (`HashMap<String, PrefValue>`).
- No GTK in these tests. UI tests are manual (T-22a-033 acceptance list).
