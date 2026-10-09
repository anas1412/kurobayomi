# 22-backup-restore — topic: merge rules (decision tables)

Every restore decision is a function of the backup row (`B`) and the matching database row (`D`). "Match" means the lookup key in p1-behavior.md §4. Column "Write" lists what the restore writes; "keep" means the DB value is untouched.

## 1. Entry (manga or anime) row

Lookup: `(url, source)` on `mangas` / `animes` (`mangas.sq:70-74`).

| Case | Condition | Path | Result row |
|---|---|---|---|
| New | no D | `restoreNew*` (`MangaRestorer.kt:137-145`) | INSERT with all backup fields; `initialized` = B.initialized (false) (D6); `nextUpdate = 0`, `calculateInterval = 0` |
| Existing, B newer | `B.version > D.version` | `D.copyFrom(B).copy(id, parentId=B.parentId)` (anime) | UPDATE; metadata from B (author, artist, description, genre, thumbnail, status); favorite = D or B; initialized = D or B; version = B; title, viewer, chapter flags, update strategy, dateAdded, lastUpdate, coverLastModified keep D (D5) |
| Existing, B older or equal | `B.version <= D.version` | `B.copyFrom(D).copy(id, parentId=B.parentId)` | UPDATE; metadata from D; favorite = D or B; version = D; title, viewer, chapter flags, update strategy, dateAdded, lastUpdate, coverLastModified from B (D5) |
| Existing, anime seasons | `B.parentId != null` | top-level pass (D7) | UPDATE with `parentId = B.parentId` (raw) |

Always: `isSyncing = 1` on UPDATE (`MangaRestorer.kt:131`, `AnimeRestorer.kt:147`). The restore never resets it (E-25).

Field ownership for the two existing-row branches ("B" = backup value, "D" = DB value). Source: `Manga.copyFrom` / `Anime.copyFrom` (`MangaRestorer.kt:93-105`, `AnimeRestorer.kt:107-121`).

For the B-newer branch the call is `D.copyFrom(B)`, so `this = D` and `newer = B`. Therefore:
- `author, artist, description, genre, thumbnailUrl, status, version` = **B** (newer)
- `favorite, initialized` = D OR B
- `title, viewerFlags, chapterFlags, updateStrategy, dateAdded, lastUpdate, coverLastModified, source, url` = **D** (this)

For the else branch `B.copyFrom(D)`: `this = B`, `newer = D`:
- `author, artist, description, genre, thumbnailUrl, status, version` = **D**
- `favorite, initialized` = B OR D
- `title, viewerFlags, chapterFlags, updateStrategy, dateAdded, lastUpdate, coverLastModified, source, url` = **B**

Summary: the **newer** side owns the metadata and the version; the **older** side owns the non-metadata fields. This asymmetry is D5.

Anime adds: `fetchType` and `parentId` follow the newer side in `copyFrom`, then `parentId` is overwritten with `B.parentId` by `.copy(parentId = anime.parentId)` in both branches (D7).

## 2. Chapter row

Lookup: `url` within the manga (`dbChaptersByUrl`), not global.

| Case | Condition | Result |
|---|---|---|
| New | no D with same url | INSERT B (with `mangaId`) |
| Same state | `B.forComparison() == D.forComparison()` (all fields except id, mangaId, dateFetch, dateUpload, lastModifiedAt, version) | skip |
| Changed | otherwise | UPDATE: read, bookmark, lastPageRead, version, isSyncing = 0 (from merge result) |

Merge result for Changed (`MangaRestorer.kt:158-182`), in order:
1. `U = B.copyFrom(D)`: name, url, dateUpload, chapterNumber, scanlator from D (blank scanlator to null); other fields from B.
2. `U.id = D.id`; `U.bookmark = B.bookmark OR D.bookmark`.
3. If `D.read AND NOT U.read`: `U.read = true`, `U.lastPageRead = D.lastPageRead`.
4. Else if `U.lastPageRead == 0 AND D.lastPageRead != 0`: `U.lastPageRead = D.lastPageRead`.
5. Otherwise `U` keeps B's `read` and `lastPageRead` (can regress: D8).

Decision table for read and position (both existing):

| D.read | B.read | B.lastPageRead | D.lastPageRead | Result read | Result lastPageRead |
|---|---|---|---|---|---|
| true | true | any | any | true | B (may regress, D8) |
| true | false | any | any | true | D |
| false | true | 0 | 0 | true | 0 |
| false | true | 0 | 7 | true | 7 |
| false | true | 5 | 7 | true | 5 (regress, D8) |
| false | false | 0 | 7 | false | 7 |
| false | false | 3 | 0 | false | 3 |

Episode rule (anime) is identical with `seen`, `lastSecondSeen`, plus `fillermark = B OR D` and `summary`, `previewUrl` from D (blank to null).

## 3. History row

Manga: lookup by chapter URL (`history.sq:25-31`); new entry requires a chapter with that URL (global, D9). Anime: lookup by episode URL; new entry requires an episode with that URL (global).

| Case | Result |
|---|---|
| No history row, chapter/episode absent | skip |
| No history row, chapter present | INSERT `(chapter_id, last_read = B.lastRead, time_read = B.readDuration)` |
| History row exists (manga) | `readAt = max(B.lastRead, D.last_read)` (null if 0); `readDuration = max(B, D.time_read) - D.time_read` (delta); upsert `last_read = readAt`, `time_read = time_read + delta` = `max(B, D)` |
| History row exists (anime) | `seenAt = max(B.lastRead, D.last_seen)` (null if 0); upsert `last_seen = seenAt` (no duration) |

Idempotency: re-running yields the same `time_read` and `last_read`.

## 4. Track row

Lookup: `trackerId` within the entry (`getTracks(entryId)` keyed by `trackerId`).

| Case | Result |
|---|---|
| New tracker for the entry | INSERT B (id 0 to let DB assign; entry id set) via `InsertMangaTrack.awaitAll` / `InsertAnimeTrack` |
| Same state (`forComparison` equal, id and entry id zeroed) | skip |
| Existing, changed | UPDATE `manga_sync` / `anime_sync` with: `remoteId = B.remoteId`, `libraryId = B.libraryId`, `lastChapterRead` (anime: `lastEpisodeSeen`) = `max(D, B)`, and every other column from D: title, totalChapters, status, score, remoteUrl, startDate, finishDate, private |

`remoteId` for a backup row: `mediaIdInt` if non-zero, else `mediaId` (deprecated int wins for old files).

## 5. Excluded scanlators

| Case | Result |
|---|---|
| B list empty | nothing |
| s in D list | nothing for s |
| s not in D list | INSERT (manga_id, s) |

## 6. Categories (membership and definitions)

Definitions (manga and anime):

| Case | Result |
|---|---|
| B name matches a D name (exact) | keep D row (flags of D kept) |
| No match | INSERT `(name, order = nextOrder++, flags = B.flags)`; nextOrder starts at max(D.order) + 1 or 0 |
| Duplicate B names, no D match | two rows (kept, E-16) |

Membership (entry): for each `order` in the entry's category list, find the backup category with that `order`, then the DB category with the same name (D). If found, pair `(entryId, D.id)`. If at least one pair: DELETE all memberships of the entry, then INSERT all pairs. If no pair: memberships unchanged.

`categorized_display` = `(D + created).distinctBy(flags).size > 1`.

## 7. Custom buttons

| Case | Result |
|---|---|
| Name matches D | skip |
| No match | INSERT with `isFavorite = B.isFavorite AND NOT dbHasFavorite` (dbHasFavorite computed once, D10) and `sortIndex = nextSortIndex++` |

PROPOSAL: `hasFavorite` becomes true after the first favourite insert.

## 8. Extension stores and repos

Anime store (UPSERT on index_url): always writes. `badgeLabel = B.badgeLabel ?: B.name`; `isLegacy = B.isLegacy ?: true`.

Manga repo (INSERT, may error):

| Condition | Result |
|---|---|
| D with same URL, different fingerprint | error "Already Exists with different signing key fingerprint" |
| D with same fingerprint (any URL) | error "<D.name> has the same signing key fingerprint" (includes identical repo, D11) |
| neither | INSERT |

PROPOSAL: same URL and same fingerprint → success without error and without insert.

## 9. Preferences (app and source)

| Stored type | Backup type | Non-category key | Category key |
|---|---|---|---|
| absent | any | write (Int default_category/default_anime_category: only if remapped id exists) | write mapped ids (append, D12); if no id mapped: no write |
| same as B | same | write | append mapped ids (D12) |
| different | any | skip silently | skip silently (the StringSet path only runs when the stored value is a Set or absent) |

## 10. Decision order summary (per run)

1. Decode; choose legacy or current; convert.
2. Amount.
3. Categories (manga and anime definitions).
4. App preferences (uses category maps only when categories enabled).
5. Source preferences.
6. Anime entries (sorted; seasons per D7 decision).
7. Manga entries (sorted).
8. Anime stores; manga repos.
9. Custom buttons.
10. Extensions (skipped count).
11. Write error log; emit completion.
