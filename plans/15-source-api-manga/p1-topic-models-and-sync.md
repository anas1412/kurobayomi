# Topic: source models in the app — mapping, detail merge, chapter sync, paging, library rules

This topic covers how the app consumes `SManga`, `SChapter`, `Page` and `MangasPage`. The code that does this is outside the scope folder but defines the contract that `source-api` must honour. Each rule cites the file that implements it. Kurobayomi reproduces these rules in `kuro-app` (domain layer, area:07/08/10) using the `kuro-source` types.

## 1. Identity

| Entity | Identity in the app | Source of identity |
|---|---|---|
| Source | `MangaSource.id` (Long) | `HttpSource.id` = `generate_id` (`p1-behavior.md` section 1) |
| Manga (entry) | `(source, url)` — `mangas` row with `source = id` and `url` | `SManga.url` (path, see behaviour 6) |
| Chapter | `(manga_id, url)` — `chapters` row | `SChapter.url` |
| Page | `(chapter, index)` in the reader; `(chapter, index)` padded filename on disk | list position (behaviour 5.2) |

Consequences:
- Two different `SChapter` objects with the same `url` in one manga are the same chapter. The sync removes duplicates by `url` (section 5, step 2).
- Changing a manga's `url` (for example through a domain change with `setUrlWithoutDomain`) creates a new entry; the old one becomes unmatched.

## 2. `SManga` → domain `Manga` (OBSERVED)

`SManga.toDomainManga(sourceId)` (app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:72-86): `Manga.create().copy(url, title, artist, author, description, genre = getGenres(), status = status.toLong(), thumbnailUrl = thumbnail_url, updateStrategy = update_strategy, initialized, source = sourceId)`. Every other domain field keeps `Manga.create()` defaults (`favorite = false`, `dateAdded = 0`, `initialized = false`, `coverLastModified = 0`, `updateStrategy = ALWAYS_UPDATE`, `viewerFlags = 0`, `chapterFlags = 0`, `lastModifiedAt = 0`, `version = 0`, `source = -1`, `id = -1`; domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:115-131).

## 3. Domain `Manga` → `SManga` (OBSERVED)

`Manga.toSManga()` (Manga.kt:38-49):
```
url, title, artist, author, description
genre  = genre.orEmpty().joinToString()          // separator ", "
status = status.toInt()
thumbnail_url = thumbnailUrl
initialized = initialized
```
`update_strategy` is NOT copied: the new `SManga` keeps `ALWAYS_UPDATE` (SMangaImpl default). Observed hazard: any round trip through `toSManga` loses a manga's `ONLY_FETCH_ONCE`. The app only uses this for the source call and writes the source's own value back afterwards (`copyFrom`), so the hazard does not reach the database in the normal flow. Kurobayomi copies `update_strategy` in both directions (T-15a-036).

## 4. Detail merge into the database (OBSERVED)

### 4.1 `Manga.copyFrom(other: SManga)` — Manga.kt:50-69
Used by the metadata update job (app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaMetadataUpdateJob.kt:125):
```
author          = other.author ?: author
artist          = other.artist ?: artist
description     = other.description ?: description
genre           = if (other.genre != null) other.getGenres() else genre
thumbnailUrl    = other.thumbnail_url ?: thumbnailUrl
status          = other.status.toLong()          // always
updateStrategy  = other.update_strategy          // always
initialized     = other.initialized && initialized
```
Note the genre rule: a non-null blank source genre yields `getGenres() == null`, which CLEARS the stored genres here (observed; different from the refresh path in 4.2).

### 4.2 `UpdateManga` (library refresh) — UpdateManga.kt:40-79
Builds `MangaUpdate(id, title, coverLastModified, author, artist, description, genre = remote.getGenres(), thumbnailUrl, status, updateStrategy, initialized = true)`.
- `title` = null when `remoteTitle.isEmpty() || localManga.favorite`; else the remote title.
- `coverLastModified`: `null` when remote thumbnail is null/empty; `null` when `!manualFetch && local.thumbnailUrl == remote.thumbnail_url`; `now` when local is a local source; for a local custom cover: delete cached cover, `null`; otherwise delete cached cover and `now`.
- `thumbnailUrl = remote.thumbnail_url?.takeIf { it.isNotEmpty() }`.
- `status` and `updateStrategy` always from remote; `initialized = true`.
- SQL (data/src/main/sqldelight/data/mangas.sq update block): every field `coalesce(:x, x)`, so a `null` in `MangaUpdate` keeps the stored value. Therefore a source `null` for author/artist/description/genre never clears on this path, and an empty string for genre becomes `null` (keep).

### 4.3 Write fields for a refresh (summary)
`title`(conditional), `cover_last_modified`(conditional), `author`, `artist`, `description`, `genre`, `thumbnail_url`(non-empty only), `status`, `update_strategy`, `initialized` (true). `last_update`, `next_update` and fetch intervals are written by the library job (area:26), not here.

## 5. Chapter sync (`SyncChaptersWithSource`) — steps that depend on `SChapter`

File: app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt.

1. **Empty guard.** `if (rawSourceChapters.isEmpty() && !source.isLocal()) throw NoChaptersException()` (line 56-58). Messages: library job shows `MR.strings.no_chapters_error` = "No chapters found" (MangaLibraryUpdateJob.kt:295-296); the generic formatter shows `MR.strings.no_results_found` = "No results found" (ExceptionFormatter.kt:28-30). The DB is not touched.
2. **Normalise input.** `sourceChapters = raw.distinctBy { it.url }.mapIndexed { i, s -> Chapter.create().copyFromSChapter(s).copy(name = ChapterSanitizer.sanitize(s.name, manga.title)).copy(mangaId, sourceOrder = i) }`.
   - `distinctBy` keeps the FIRST occurrence.
   - `sourceOrder = i` is the position in the source list (after distinct). The source list order is therefore the reading order key.
   - `ChapterSanitizer.sanitize(name, title)` (data/src/main/java/tachiyomi/data/items/chapter/ChapterSanitizer.kt): `trim()`, then `removePrefix(title)` (exact, case-sensitive; no-op when the name does not start with the manga title), then `trim(chars)` where `chars` = the Unicode whitespace list in that file (space, tab, LF, VT, FF, CR, NEL U+0085, NBSP U+00A0, U+1680, U+2000-U+200A, U+2028, U+2029, U+202F, U+205F, U+3000) plus `-`, `_`, `,`, `:`.
3. **Diff.** `dbChapters = getChaptersByMangaId(manga.id)`. `removedChapters = dbChapters.filterNot { db.url in sourceChapters.url }`.
4. **Per source chapter, in list order** (`maxSeenUploadDate = 0` before the loop):
   a. If `source is HttpSource`: `sChapter = chapter.toSChapter()`; `source.prepareNewChapter(sChapter, manga.toSManga())`; `chapter = chapter.copyFromSChapter(sChapter)`. This runs for every chapter on every sync (behaviour 8).
   b. `chapterNumber = ChapterRecognition.parseChapterNumber(manga.title, chapter.name, chapter.chapterNumber)` (domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterRecognition.kt:33-37): returns the given number when it is `-2.0` or greater than `-1.0`; otherwise parses the number from the name (recognition rules in area:07).
   c. `dbChapter = dbChapters.find { it.url == chapter.url }`.
   d. **New** (`dbChapter == null`):
      - if `chapter.dateUpload == 0L`: `altDateUpload = if (maxSeenUploadDate == 0L) now else maxSeenUploadDate`; `dateUpload = altDateUpload`;
      - else: `maxSeenUploadDate = max(maxSeenUploadDate, sourceChapter.dateUpload)`.
      - add to `newChapters`.
   e. **Existing**: if `shouldUpdateDbChapter(dbChapter, chapter)` (area:07): `toChange = dbChapter.copy(name = chapter.name, chapterNumber = chapter.chapterNumber, scanlator = chapter.scanlator, sourceOrder = chapter.sourceOrder)`; if `chapter.dateUpload != 0` also `dateUpload = chapter.dateUpload`; add to `updatedChapters`. If the chapter directory name would change and the chapter is downloaded, the download is renamed (`renameChapter`).
5. **Early exit.** When nothing is new, removed or updated: possibly update the fetch interval, return `emptyList()`.
6. **Date fetch for new chapters.** `itemCount = newChapters.size`; each new chapter gets `dateFetch = nowMillis + itemCount--`. The comment in the code states: "Sources MUST return the chapters from most to less recent, which is common." The first item of the list therefore gets the largest `dateFetch`.
7. Removed chapters: their read/bookmark flags and `dateFetch` are reused when a new chapter has the same recognised chapter number (lines ~170-200; area:07).

### 5.1 Chapter number types (OBSERVED hazard)
- Source: `SChapter.chapter_number: Float` (default `-1f`).
- Domain: `Chapter.chapterNumber: Double` (default `-1.0`); DB `chapter_number REAL NOT NULL`.
- `toSChapter`: `chapterNumber.toFloat()` (Chapter.kt:9-17, `it.chapter_number = chapterNumber.toFloat()`).
- `copyFromSChapter`: `chapterNumber = sChapter.chapter_number.toDouble()` (Chapter.kt:19-27).
- Float to Double widening keeps the binary value of the float: `1.1f` becomes `1.100000023841858`. The DB then stores that value, and comparisons with recognised numbers (`chapter.chapterNumber in readChapterNumbers`, `in deletedChapterNumbers`) compare exact doubles. Observed hazard.
- Proposal (T-15a-004): keep `chapter_number: f32` in the model; convert to `f64` with `chapter_number.to_string().parse::<f64>()`, which yields `1.1` for `1.1f32` (Rust prints the shortest round-trip form). Document as a deliberate correction.

### 5.2 Scanlator
`scanlator` blank → `null`; otherwise `trim()` (Chapter.kt:24). Applied when reading the source value into the domain model (before the sync).

### 5.3 Date upload
`SChapterImpl.date_upload` default `0`. In the domain, `Chapter.create().dateUpload` default is `-1`, but `copyFromSChapter` copies the source value, so a source `0` reaches the sync as `0` and is replaced by step 4d. A source value `-1` is not special-cased (it is stored as-is).

## 6. Paging of catalogue results (OBSERVED)

File: data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt.
- `page = params.key ?: 1`: pages start at **1** for all three listings.
- `withIOContext { requestNextPage(page).takeIf { it.mangas.isNotEmpty() } ?: throw NoChaptersException() }`; any exception → `LoadResult.Error(e)`.
- Empty `mangas` (even with `hasNextPage == true`) is an error, not an end-of-list.
- `nextKey = if (hasNextPage) page + 1 else null`; `prevKey = null`.
- `requestNextPage`: `SourceSearchPagingSource` → `getSearchManga(page, query, filters)`; `SourcePopularPagingSource` → `getPopularManga(page)`; `SourceLatestPagingSource` → `getLatestUpdates(page)`.
- Pager: `PagingConfig(pageSize = 25)` (BrowseMangaSourceScreenModel.kt:112).
- Source `MangasPage.mangas` order is the display order (no sort in the app).
- Entries already in the local database are replaced by their local rows: `getManga.subscribe(localManga.url, localManga.source)` (BrowseMangaSourceScreenModel.kt:117); the source list is not deduplicated.

Proposal: keep page numbering from 1, the `nextKey = page + 1` rule, and the empty-page rule (an empty page is an error, shown with the text "No results found"). The paging code is area:06's; the source contract is unchanged.

## 7. Library update eligibility from source fields (OBSERVED)

File: app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:186-200.
1. `updateStrategy != ALWAYS_UPDATE` → skipped with `MR.strings.skipped_reason_not_always_update` ("Skipped because series does not require updates").
2. `ENTRY_NON_COMPLETED in restrictions && status.toInt() == SManga.COMPLETED` → skipped with `MR.strings.skipped_reason_completed` ("Skipped because series is complete").
3. Other restrictions follow (area:26).

## 8. Status display and filters (OBSERVED)

- Library filter "Completed": `status.toInt() == SManga.COMPLETED` (MangaLibraryScreenModel.kt:216).
- Stats: `status == COMPLETED && unreadCount == 0` (MangaStatsScreenModel.kt:56); update restrictions (MangaStatsScreenModel.kt:112).
- Upcoming: `GetUpcomingManga` uses `ONGOING` and `PUBLISHING_FINISHED` (domain/src/main/java/mihon/domain/upcoming/manga/interactor/GetUpcomingManga.kt:12-15).
- Domain `Manga.kt:39` excludes `COMPLETED` in one computed property (`takeIf { status != SManga.COMPLETED.toLong() }`).
- Display text and icons: behaviour section 12.

## 9. Web URLs and deep links

- Web URL: behaviour section 7.
- Deep-link resolution: behaviour section 16 (`ResolvableSource`).

## 10. Chapter page list cache key (for completeness)

`ChapterCache.getKey(chapter) = "${chapter.mangaId}${chapter.url}"` (ChapterCache.kt:197-199), hashed with lowercase MD5 hex. Changing a chapter URL or moving the manga id invalidates its page-list cache entry (the entry is then re-fetched from the source).

## 11. Test vectors for T-15a-036 and T-15a-035

1. `Manga.copyFrom` with `SManga{ author: None, artist: Some(""), description: None, genre: Some(""), thumbnail: None, status: 2, update_strategy: OnlyFetchOnce, initialized: false }` onto a manga with `author "A"`, `artist "B"`, `description "D"`, `genre ["x"]`, `thumbnail "t"`, `initialized true`: result `author "A"`, `artist ""`, `description "D"`, `genre null`, `thumbnail "t"`, `status 2`, `update_strategy OnlyFetchOnce`, `initialized false`.
2. `UpdateManga` path with the same input: genre keeps `["x"]` (coalesce), artist becomes `""`.
3. Sync with source list `[c3, c2, c2-dup, c1]` (c2 duplicated by url): `distinctBy` keeps `c3, c2, c1` → `sourceOrder 0, 1, 2`; `dateFetch` of new chapters: first is `now + 3`, then `now + 2`, `now + 1`.
4. Sync with chapter `dateUpload 0` as the first new chapter and no earlier date: `dateUpload = now`; a second chapter with `dateUpload 0` after a chapter with `dateUpload 1000`: `dateUpload = 1000` (`maxSeenUploadDate`).
5. Name sanitiser: manga title `One Piece`, name `"  One Piece - Chapter 1 "` → `Chapter 1`. Name `"Onepiece 2"` → unchanged (`removePrefix` is case-sensitive).
6. Chapter number: source `1.1f` → domain `1.1` (Kurobayomi) versus `1.100000023841858` (Kotlin).
7. Empty source list for a non-local source → `NoChaptersException`; for a local source → proceeds.
8. `update_strategy` round trip: `to_s_manga` then `to_domain_manga` on `OnlyFetchOnce` must yield `OnlyFetchOnce` in Kurobayomi (Kotlin yields `ALWAYS_UPDATE`, observed hazard in section 3).
