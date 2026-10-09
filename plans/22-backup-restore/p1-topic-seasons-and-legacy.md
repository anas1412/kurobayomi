# 22-backup-restore — topic: anime seasons and legacy backups

Two areas of the restore path have non-obvious structure: how seasons (anime rows with a parent) are linked and restored, and how legacy 0.x files are read. Both are described with exact Kotlin references and the Kurobayomi decision.

## 1. Seasons

### 1.1 What a season is (OBSERVED)
- An anime row whose `parentId` (DB column `parent_id`) is not null. The parent is another anime row. The export writes `parentId = anime.parentId` and `id = anime.id` for every anime (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeBackupCreator.kt:97-98`).
- The backup field `BackupAnime.parentId` (proto 502) is the source device's DB id of the parent. `BackupAnime.id` (proto 503) is the source device's DB id of this row. Both are only meaningful within the same backup file.
- `fetchType` (proto 507) = `Seasons` or `Episodes`. A parent whose fetch type is Seasons lists season rows; a season has its own episodes.

### 1.2 The Kotlin restore procedure (OBSERVED, `BackupRestorer.kt:270-289`, `AnimeRestorer.kt:58-93`)
```
sorted = animeRestorer.sortByNew(backup.backupAnime)          // includes season rows
for it in sorted:                                              // includes season rows (no filter)
    seasons = backup.backupAnime.filter { s -> s.parentId == it.id }
    restore(it, categories, seasons):
        tx begin
        D_parent = find(it)  (url, source)
        P = restoreNew(it.getAnimeImpl())  or  restoreExisting(it.getAnimeImpl(), D_parent)
        for bs in seasons:                                     // children of this parent
            D_s = find(bs)
            S = bs.getAnimeImpl().copy(parentId = P.id)        // DB id of parent: CORRECT
            restoreNew(S) or restoreExisting(S, D_s)           // restoreExisting: parentId = S.parentId = P.id (correct)
        restoreAnimeDetails(P, episodes, categories, ...)      // episodes/history/tracks of the PARENT only
        tx commit
```
Key facts:
- Season rows are restored twice: once under the parent (correct `parentId`), and once as a top-level element of the sorted list.
- The top-level pass calls `restore(season, categories, seasonsOf(season))`. `seasonsOf(season)` is empty in practice, so no nested work happens; but the top-level `restoreExisting` writes `parentId = season.getAnimeImpl().parentId`, which is the **source device's** parent id (D7).
- Whether the final `parentId` is correct depends on order. The parent pass sets it correctly; if the top-level pass for the season runs after the parent pass, it overwrites with the raw source id.
- Episodes, history and tracks of a season row are restored only in the top-level pass (the `restoreAnimeDetails` call happens for every top-level row, including seasons, with the season's own `episodes` list). Under the parent pass, a season's episodes are not restored at all (the nested loop calls only the entry merge, not `restoreAnimeDetails`). So the top-level pass is the only place season episodes come from.

  Consequence (observed, important): if a season row has no top-level turn, its episodes are never restored. Because every row is in `backupAnime`, every season does get a top-level turn, so episodes are restored. The fact that they are restored at top level is the reason D7 does not lose data, only the parent pointer.

### 1.3 Season fetch interval (OBSERVED)
- `awaitUpdateFetchInterval` runs in `restoreAnimeDetails` (`AnimeRestorer.kt:324`).
- Top-level season rows call `restoreAnimeDetails`, so each season gets its interval from its own episodes at its top-level turn.
- Nested season rows (parent pass, `AnimeRestorer.kt:72-82`) never call `restoreAnimeDetails`, so they get no interval from that pass.

### 1.4 Kurobayomi decision (PROPOSAL, fixes D7 and D18)
1. Pre-pass: build `children: HashMap<i64 /*backup parent id*/, Vec<usize>>` from rows with `parent_id.is_some()`.
2. Top-level loop: process only rows with `parent_id.is_none()`. For each parent:
   - restore the parent entry, record `backup_to_db[parent.backup_id] = db_id`;
   - restore each child with `parent_id = db_id` and its own episodes, history and tracks (`restoreAnimeDetails` semantics);
   - compute the fetch interval for parent and children.
3. Orphan rows (a `parent_id` that is not a backup id in the file): restore as top-level rows with `parent_id = NULL` (log as an entry note, not an error).
4. Amount: count every anime row once (as now). Progress increments once per row processed (parent or child), so final progress equals the amount.

Expected difference from Kotlin: the final `parentId` is always the correct DB id, and seasons are never re-written by a second pass. The set of restored episodes is the same.

Test: X-42, X-43, X-44, X-101 and X-104 with Q-11 and Q-12.

### 1.5 Season restore with an existing season in the DB
- The child path uses `restoreExisting` with `parentId = P.id`. The DB row keeps its title and other fields per the B-newer/B-older rule (D5 applies to seasons too).
- No special handling for season `seasonNumber` (-1.0 default) or `seasonFlags`; they follow the same merge (they are not in `copyFrom` but are written by `updateAnime` from the object, so the backup value is written; see the field list in p1-behavior.md §4.4.1).

## 2. Legacy backups (Aniyomi 0.x)

### 2.1 Detection (OBSERVED, `BackupDetector.kt:14-34`)
- Decoded with a reduced message holding only field 103 (`backupAnimeSources`, repeated `DetectAnimeSource`) and field 500 (`isLegacy`, default true).
- Legacy iff `isLegacy && backupAnimeSources.isNotEmpty()`.
- A 0.x file has field 103 and no field 500 (defaults to true), so it is legacy. A 1.x Aniyomi file has field 500 = false, so it is not legacy. A Mihon file has neither field, so it is not legacy.
- Decode errors inside detection return false (`catch (SerializationException) false`): the file then goes through the normal path, which may fail with "corrupted".

### 2.2 Conversion (OBSERVED, `models/Backup.kt:6-42`)
`LegacyBackup` field numbers differ from `Backup` for anime:

| Concept | LegacyBackup | Backup |
|---|---|---|
| anime entries | 3 | 501 |
| anime categories | 4 | 502 |
| anime sources | 103 | 503 |

Everything else maps by name. `toBackup()` sets `isLegacy = false` and passes `backupExtensions` (106), `backupAnimeExtensionStores` (107), `backupCustomButton` (109) through.

### 2.3 Differences that matter after conversion
- Legacy entries use the same `BackupAnime` model as new files. Fields absent in 0.x (for example `parentId`, `id`, `fetchType`, `seasonNumber`) take their defaults: `parentId = null`, `id = null`, `fetchType = Episodes`, `seasonNumber = -1.0`. So legacy files never contain seasons.
- Legacy anime with `viewer_flags` (proto 103 in BackupAnime) defaults to 0 when absent.
- Legacy manga history is in field 104 with `readDuration` default 0.

### 2.4 Kurobayomi decision
- Keep detection and conversion (behaviour exact).
- Test with Q-02 that counts equal the expected table and that no season logic runs (no `parent_id`).
- No change to the 0.x decode; a 0.x file with `isLegacy` absent and 103 present is legacy by definition (E-06 shows the detector behaviour is by design).

## 3. Interaction summary

| Situation | Kotlin | Kurobayomi (PROPOSAL) |
|---|---|---|
| 1.x file with seasons, parent before season in sorted order | season `parentId` = raw source id (bug) | season `parentId` = DB id of parent |
| 1.x file with seasons, season before parent | correct final `parentId` | correct |
| Orphan season (parent id not in file) | raw id kept | `parentId = NULL` |
| Legacy 0.x file | converted; no seasons | same |
| Mihon file | no seasons, no 5xx | same |

Task links: T-22a-023 (seasons), T-22a-050 (legacy conversion tests), T-22a-051 (wire fixtures).
