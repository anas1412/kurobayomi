# Topic: on-disk lifecycle of an anime download, and the downloaded-episode index

Deep dive over `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadProvider.kt`, `AnimeDownloadCache.kt`, `AnimeDownloadManager.kt` (delete and rename paths) and `AnimeDownloader.kt` (staging and finalise). Rules are in `p1-behavior.md` §5.9, §6, §8, §9. This note traces one episode through every operation that touches the disk, and says what the index knows at each step.

## 1. States of one episode on disk

| # | On-disk state | Index (`AnimeDownloadCache`) says | Queue says | How it gets there |
|---|---|---|---|---|
| S0 | nothing | not downloaded | absent | initial |
| S1 | `<dir>_tmp/` exists, empty | not downloaded (`_tmp` excluded, `:374`) | DOWNLOADING | `downloadEpisode` creates staging (`AnimeDownloader.kt:359`) |
| S2 | `<dir>_tmp/<name>.tmp` growing | not downloaded | DOWNLOADING, progress 0-99 | ffmpeg writes output (`:568-632`) |
| S3 | `<dir>_tmp/<name>.mkv` | not downloaded | DOWNLOADING, 100 | ffmpeg rename (`:617-619`) |
| S4 | `<dir>/<name>.mkv` + `.nomedia` | downloaded (via `addEpisode`) | removed (DOWNLOADED) | finalise rename and cache add (`:382-390`) |
| S5 | `<dir>/<name>.mkv`, renamed metadata | old name removed, new name added | absent | `renameEpisode` (`AnimeDownloadManager.kt:624-643`) |
| S6 | `<dir>/` removed | removed | absent | `deleteEpisodes` (`:506-527`) |
| S7 | `<dir>_tmp/` after crash in S2 | not downloaded | restored as QUEUE | restart: staging left as is |

Transitions a port must reproduce: S0→S1→S2→S3→S4 for a normal download; S2→S0 on pause (partial deleted); S3→S4 on resume after crash (finished remux reused); S4→S5 on metadata rename; S4→S6 on delete.

## 2. Naming inside one anime folder

For episode `name = "Episode 1"`, `scanlator = "Scan"`, `anime.title = "Some Show"`:

| Object | Name | Source |
|---|---|---|
| final episode directory | `Scan_Episode 1` | `getEpisodeDirName` (`AnimeDownloadProvider.kt:427-435`) |
| staging directory | `Scan_Episode 1_tmp` | `episodeDirname + TMP_DIR_SUFFIX` (`AnimeDownloader.kt:359`) |
| staging remux file | `Episode 1.tmp` | `"$filename.tmp"` with `filename = buildValidFilename(episode.name)` (`:426`, `:506`) |
| finished remux | `Episode 1.mkv` | `"$filename.mkv"` (`:618`) |
| external placeholder | `Some Show - Episode 1_tmp.mkv` | `betterFileName + "_tmp.mkv"` (`:718`) |
| legacy directory | `Scan_Episode 1` (no sanitising) | `getOldEpisodeDirName` (`:454-461`) |

Note that the remux file name uses the episode name only, while the external placeholder uses anime title plus episode. This asymmetry is intentional in the Kotlin (it looks inconsistent but it is the behaviour). The Rust port keeps both, and the resume check (`startsWith("<name>.mkv")`) applies only to the internal path.

## 3. Operations and their disk effects

### 3.1 Enqueue (no disk write)

Only the queue and the store change. The episode directory is not created until the download actually starts (`downloadEpisode`). Enqueue filters out episodes already on disk using `findEpisodeDir` (both names), which reads the disk.

### 3.2 Start a download

1. `getAnimeDir` creates `<source dir>` and `<anime dir>` if missing (`AnimeDownloadProvider.kt:331-345`). The source directory name is `buildValidFilename("<name> (<LANG>)")`, which is the same string as `AnimeHttpSource.toString()` (`AnimeHttpSource.kt:117`). Renaming an extension's language or name changes this string and orphans old folders unless `renameSource` runs (`AnimeDownloadManager.kt:596-614`).
2. Free-space check on the anime directory (200 MiB minimum).
3. Create `<dir>_tmp`.
4. Resolve video (no disk write).
5. Remux into `<name>.tmp`. Retries delete the file each time.

### 3.3 Pause or cancel

The active `.tmp` is deleted (`AnimeDownloader.kt:514`, `:402`). The staging directory itself is kept, so a later start reuses it. A finished `.mkv` is never deleted by pause.

### 3.4 Finalise

1. Delete `<anime> - <name>_tmp.mkv` if present (no-op for internal downloads, the placeholder only exists on the external path).
2. `tmpDir.renameTo(episodeDirname)`. The return value is ignored in Kotlin (`:384`). **Defect**: if the target exists, the rename fails silently, the cache still records the episode, and the item is marked DOWNLOADED. The Rust port must treat this as an error (T-24a-066).
3. Cache add.
4. Create `.nomedia`. In Kotlin it is created in the staging path after the rename, which may be a stale handle (`AnimeDownloader.kt:388`). The Rust port creates it in the final directory (T-24a-056).

### 3.5 Metadata rename (`renameEpisode`)

Called when a re-synced episode changes `name` or `scanlator` (`isEpisodeDirNameChanged`, `AnimeDownloadProvider.kt:463-466`). It looks up both valid names for the old episode, renames the first found, then updates the index (`AnimeDownloadManager.kt:624-643`). If no folder exists it logs an error and does nothing else. The new name always uses the new format (no legacy).

### 3.6 Delete an episode

1. Filter by bookmark and category rules (behaviour §3.2).
2. Remove matching queued items (pausing the runner if it was running).
3. Delete every existing valid-name directory for each episode.
4. Remove those names from the index.
5. If the anime directory is now empty, delete it and its index entry.
6. If the source directory is then empty, delete it and its index entry.

Steps 5 and 6 are done by `deleteAnime(removeQueued = false)` when triggered from `deleteEpisodes`, and by `deleteAnime` directly otherwise (`AnimeDownloadManager.kt:536-550`).

### 3.7 Delete an anime

`removeQueued` removes queued items for the whole anime (without pause, which is defect E-D02). The anime directory is deleted recursively. The index entry is removed. Empty source directory is removed.

## 4. The index: what it is and what it is not

The index (`AnimeDownloadCache`) is a snapshot of directory names. It is never the source of truth for playback. It answers three questions quickly:
- "is this episode downloaded" (`isEpisodeDownloaded`);
- "how many episodes / how much space is there for this anime or in total";
- "which folders are there" (for the storage screen).

### 4.1 Structure

```
rootDownloadsDir: RootDirectory
  dir: root path
  sourceDirs: { sourceId -> SourceDirectory }
    dir: source folder path
    animeDirs: { animeDirName -> AnimeDirectory }
      dir: anime folder path
      episodeDirs: Set<String>   (folder names, or stems of .mp4/.mkv files)
```

Keys are the Kotlin-identical folder names (after `buildValidFilename`). Lookups for a given anime use `getAnimeDirName(anime.title)`; lookups for episodes use both valid names.

### 4.2 Lifecycle

| Trigger | Action | Cost |
|---|---|---|
| App start with a snapshot on disk | load snapshot, `lastRenew = now` (no walk) | read one file |
| App start without snapshot, or snapshot older than 1 h | full walk on first query | proportional to folder count |
| Storage-root change (`storageManager.changes`) | delete snapshot, walk again | full walk |
| Any finalise, delete or rename | in-memory mutation under mutex, then debounced snapshot write (1 s) | write one file |
| Renewal failure | `lastRenew` set anyway; notification of change | none |

Renewal waits up to 30 s for extensions and sources to initialise (`AnimeDownloadCache.kt:342`). If they are still not ready, folders whose source is unknown are simply not indexed, and they reappear on the next renewal after the sources load (E-C03).

### 4.3 Consistency rules

- A folder whose name ends in `_tmp` is never indexed (in progress or orphaned). So a finished episode is only "downloaded" after its staging directory is renamed.
- A folder whose source is not installed is never indexed. Its files are safe; the Storage screen can still see them through the filesystem (area:42-ui-updates-history-stats-storage-upcoming and E-D08).
- Files inside episode folders are not inspected; only the folder name matters, except that a file `X.mkv` in an anime folder is treated as episode `X` (this is how legacy single-file downloads are recognised).

### 4.4 Known weak points

- Reads are not mutex-protected (behaviour §8.3). Port: clone the snapshot before iterating (E-D07).
- The `isEpisodeDownloaded(skipCache = true)` path bypasses the index and hits the disk. Enqueue-time filtering does not use the index either: `queueEpisodes` calls `findEpisodeDir` directly (`AnimeDownloader.kt:298`). Keep the disk lookup for enqueue; it is correct and cheap per episode.
- The index does not notice external deletes until renewal (E-C05). Port proposal: a manual "Refresh" action plus a renewal when the Downloads page is opened.

## 5. Compatibility with an existing Aniyomi download folder

A user who migrates from Aniyomi on Android will copy the downloads folder to Linux. The layout is the same; the port must read it without changes (T-24a-057):

- Source folders are matched case-insensitively against installed sources' `"<name> (<LANG>)"` strings after `buildValidFilename`.
- Episode directories with the legacy name (no sanitising, `scanlator_name`) are found by `getValidEpisodeDirNames`.
- Legacy single-file episodes (`X.mkv` or `X.mp4` at anime level) are indexed by stem.
- `.nomedia` files are ignored by the indexer and are harmless on Linux.
- Staging folders (`_tmp`) left by an Android crash are ignored by the index and are removed by the startup sweep (T-24a-062) only when they are not in the restored queue.

Not compatible: SAF `content://` roots. The user must choose a normal folder on Linux (T-24a-059).

## 6. Test checklist for this topic

- Round-trip: download → rename metadata → delete; disk state and index agree at each step (S0 to S6).
- Crash simulation: kill after S3, restart, confirm resume (no ffmpeg call) and finalise.
- Target-exists finalise (E-F02): ERROR and staging kept.
- Case-only source rename on a case-insensitive filesystem (E-F18).
- Snapshot corruption: delete mid-write, restart, confirm rebuild.
- Import: a copied Aniyomi tree with legacy names and `.mp4` files is listed correctly.
