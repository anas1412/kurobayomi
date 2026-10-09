# Topic: backup create and restore screens

Scope files: `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt`, `RestoreBackupScreen.kt`. Engine files (not in this area): `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt`, `create/BackupCreator.kt`, `create/BackupOptions.kt`, `restore/BackupRestoreJob.kt`, `restore/RestoreOptions.kt`, `BackupFileValidator.kt` (validator is in scope as a callee and documented here). Full behaviour: `p2-behavior.md` sections 5 and 6. Tasks: T-47b-027 to T-47b-036, T-47b-060, T-47b-064.

## 1. Options as data

Create uses 12 booleans, restore uses 7. Each option is a row in a table:

```
OptionEntry { label, get, set, enabled }
```

- Create: 6 library rows, 5 settings rows, 1 extension row (`BackupOptions.kt` companion lists).
- Restore: 7 rows, no `enabled` (always enabled).
- The screen renders one checkbox per row, grouped into cards. Groups are the Kotlin list order.

Table of enabled rules (create):

| Row | Enabled when |
|---|---|
| chapters, tracking, history, readEntries | libraryEntries |
| privateSettings | appSettings or sourceSettings |
| all others | always |

Important: disabling a parent does not reset its children. A child can remain true in the array while disabled. The engine (area:21) must treat it as not applicable when the parent is off. Test T-EC-38.

## 2. Create flow

```
page opened (options = defaults)
  |
  | toggle row -> options = set(options, value)   (state only)
  |
  | Create clicked (enabled iff can_create)
  v
busy? (job tag "BackupCreator:manual" RUNNING)
  |-- yes --> toast "Backup is already in progress"; stop
  |
  no
  v
save dialog (initial name "<applicationId>_yyyy-MM-dd_HH-mm.tachibk")
  |-- cancel --> nothing
  |
  picked path
  v
enqueue manual create job (KEEP)  with options.to_array()
  v
pop page
  (progress and result: notifications from area:22/21)
```

Notes:
- The name uses local time, fixed pattern, `.tachibk` extension (Kotlin `BackupCreator.getFilename`).
- Rust adds a lock so the busy check and enqueue are atomic (RACE-06).
- Disabled while the save dialog is open (T-47b-064).

## 3. Restore flow

```
page opened with a path (from part-1 file chooser, filter *.tachibk)
  |
  v
state = Validating   (tokio task: decode file, compute missing lists)
  |
  +-- decode error ---------------> Invalid { chain }
  |                                  - options card hidden
  |                                  - Restore disabled
  |
  +-- ok, lists empty ------------> Valid
  |                                  - options card shown
  |                                  - Restore enabled iff any option
  |
  +-- ok, lists not empty --------> Missing { sources, trackers }
                                     - error card with lists
                                     - options card shown
                                     - Restore enabled iff any option
  
Restore clicked:
  busy? (job tag "BackupRestore" RUNNING) -> toast, stay
  else enqueue (KEEP) with options.to_array(), pop page
```

Closing the page during Validating cancels the task (T-47b-060).

## 4. Validation algorithm (pseudocode, proposal-faithful)

```text
fn validate(backup, sources, trackers) -> Result<Results, Invalid>:
    manga_missing = []
    for (id, name) in backup.manga_sources (unique by id):
        if not sources.has_manga_source(id):
            label = name if name is not numeric else sources.manga_stub_name(id)
            manga_missing.push(label)
    manga_missing = dedupe(manga_missing) sorted by code point

    anime_missing = same with anime catalogue, computed separately
    anime_missing = dedupe(anime_missing) sorted

    missing_sources = manga_missing ++ anime_missing       (NOT re-sorted together)

    sync_ids = dedupe(
        [t.sync_id for anime in backup.anime for t in anime.tracking]
        ++ [t.sync_id for manga in backup.manga for t in manga.tracking])

    missing_trackers = []
    for id in sync_ids:
        if let Some(name) = trackers.name_if_not_logged_in(id):   // None: unknown or logged in
            missing_trackers.push(name)
    missing_trackers = sorted(missing_trackers)

    return Results { missing_sources, missing_trackers }
```

"Numeric" means the Kotlin `toLongOrNull()` succeeds; the stub name is `getOrStub(id).toString()`.

Error case: any decode exception is wrapped (Kotlin: `IllegalStateException(e)`), and the page shows `e.toString()` of the wrapper. Rust shows the full cause chain joined with ": " (proposal, T-47b-033).

## 5. Error card text

Missing:

```
You may need to install any missing extensions and log in to tracking services afterwards to use them.

Missing sources:
- Source A
- Source B

Trackers not logged into:
- AniList
```

Invalid:

```
Invalid backup file:
<path>

Full error:
<message>
```

Kotlin composes these as one annotated string; headers are bold. Rust uses Pango markup with escaping and a selectable label.

## 6. Interaction with other areas
- area:21 defines the file layout, the backup job, and the auto-backup schedule. Create page only enqueues a manual job.
- area:22 defines decoding, applying options, and restore notifications.
- area:26 provides the job registry used by busy checks (T-47b-044).
- area:36 provides the page navigation.

## 7. Known Kotlin behaviours kept and changed

| Behaviour | Kotlin | Rust |
|---|---|---|
| Synchronous validation on UI thread | yes | no (async) |
| Restore busy check | none | toast |
| Create busy check | toast | toast (kept) |
| Canceled picker | no-op | no-op |
| "Extensions" alone | Create disabled | Create disabled (kept) |
| Persistable URI permission | taken | not needed |
| MIUI banner | shown on MIUI | dropped |
| Options reset | on each open | on each open (kept) |

## 8. Tests
- Options tables and rules: T-UT-01..04.
- Validator: T-UT-05..07 and fixtures `complete.tachibk`, `missing-numeric.tachibk`, `empty-message.tachibk`, `truncated.tachibk`, `random.bin` (T-IT-03).
- Reducer: T-UT-08 (state matrix), T-UT-09 (text).
- Create end-to-end with a stub writer: T-IT-04.
- GTK smoke: GS-04, GS-05.
