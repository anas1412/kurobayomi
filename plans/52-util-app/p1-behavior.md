# 52-util-app — behaviour specification

Conventions
- `OBSERVED` = what the Kotlin in `/home/blackbox/aniyomi-src` does (path:line where useful).
- `Proposal:` = what the Rust port should do. Where a Proposal changes an observed behaviour, the reason is given.
- Message keys: `MR.strings.<key>` (base file `i18n/src/commonMain/moko-resources/base/strings.xml`) and `AYMR.strings.<key>` (base file `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`). Plurals are in `.../base/plurals.xml` of the same modules.
- Hard-coded English strings are listed as `LITERAL` and must be kept verbatim in the Rust port (they are not translated upstream).
- Line numbers refer to the files at the time of reading.

---

## 1. Preference binding: `PreferenceMutableState` and `asState`

File: `app/src/main/java/eu/kanade/core/preference/PreferenceMutableState.kt` (38 lines).

OBSERVED
- `class PreferenceMutableState<T>(private val preference: Preference<T>, scope: CoroutineScope) : MutableState<T>`.
- Backing state: `mutableStateOf(preference.get())` (line 15).
- In `init` (lines 17-21): `preference.changes().onEach { state.value = it }.launchIn(scope)`. The collector lives in the caller's scope; there is no explicit cancellation. It stops when `scope` is cancelled.
- `value` getter returns the backing state; `value` setter calls `preference.set(value)` (lines 23-27). The setter does NOT write the backing state directly; it relies on the `changes()` flow to echo the new value back (so the state lags by one dispatch).
- `component1()` returns the backing state; `component2()` returns `preference::set` (lines 29-35).
- Extension `fun <T> Preference<T>.asState(scope: CoroutineScope) = PreferenceMutableState(this, scope)` (line 38).

Proposal
- Rust type `kuro_prefs::Pref<T>` (owned by area:02-preferences-system) exposes `get()`, `set(v)`, and `subscribe() -> watch::Receiver<T>`.
- `kuro-core::prefs_bridge::bind(pref, scope)` creates a `glib::Object` property or `ObservableValue` that: (1) reads `get()` at creation, (2) updates on every watch change on the GTK main context, (3) writes through `set()` on UI change. Writes are NOT echoed directly to the widget; the watch echo is used, to match upstream. Keep the echo because it keeps two windows showing the same preference in sync.
- Dispatch: `watch` changes are forwarded with `glib::MainContext::spawn_local`. Drop the subscription when the widget is destroyed (`connect_destroy`).

Edge cases
- `set` with an equal value still emits a change in the flow (upstream `Preference` semantics); the Rust port emits only when the value differs (`watch::Sender::send_if_modified`). This is a deliberate deviation with no user-visible loss.

---

## 2. Tri-state checkbox mapping

File: `app/src/main/java/eu/kanade/core/preference/CheckboxState.kt` (10 lines).

OBSERVED: `CheckboxState.TriState<T>`:
- `Exclude` -> `ToggleableState.Indeterminate`
- `Include` -> `ToggleableState.On`
- `None` -> `ToggleableState.Off`

Proposal (GTK): `gtk::CheckButton`:
- `Exclude` -> `inconsistent = true`, `active = false`
- `Include` -> `inconsistent = false`, `active = true`
- `None` -> `inconsistent = false`, `active = false`

Click cycle: this file defines only the value-to-state mapping. The click order belongs to the filter dialogs (owned by area:37-ui-library and area:38-ui-entry-details-manga). Read the cycle from those files before implementing the widget; it is not decided here.

---

## 3. Source readiness gate

Files: `app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt`, `app/src/main/java/eu/kanade/core/util/AnimeSourceUtil.kt` (13 lines each).

OBSERVED
- `remember { Injekt.get<MangaSourceManager>().isInitialized }.collectAsState().value` (and the anime twin).
- Returns `Boolean`. `false` until the source manager finished its first load of installed sources.

Proposal
- `kuro-core::sources::ready`: `fn manga_sources_ready() -> watch::Receiver<bool>`. The GTK page shows a spinner (`gtk::Spinner`, `spinning = true`) until the value becomes `true`, then rebuilds its list once. No timeout: the upstream has none either.

---

## 4. Collection helpers

File: `app/src/main/java/eu/kanade/core/util/CollectionUtils.kt` (99 lines).

### 4.1 `insertSeparators` (lines 8-21)
Signature: `fun <T : R, R : Any> List<T>.insertSeparators(generator: (before: T?, after: T?) -> R?): List<R>`.

Algorithm (OBSERVED, exact):
1. If the list is empty, return `emptyList()`.
2. `newList = mutableListOf<R>()`.
3. For `i` from `-1` through `lastIndex` inclusive:
   a. `before = getOrNull(i)` (null when i = -1).
   b. If `before != null`, append `before`.
   c. `after = getOrNull(i + 1)` (null after the last item).
   d. `separator = generator(before, after)`; if non-null, append it.
4. Return `newList`.

So for `[a, b]` the output is: `[sep(null,a), a, sep(a,b), b, sep(b,null)]`. The separator generator is called `size + 1` times and always gets at least one null side.

Proposal (Rust): `pub fn insert_separators<T, R>(items: &[T], mut gen: impl FnMut(Option<&T>, Option<&T>) -> Option<R>) -> Vec<R> where T: Into<R> + Clone`. Use `Option<R>` for the list row enum. Exactly the same call pattern.

### 4.2 `insertSeparatorsReversed` (lines 26-39)
Algorithm (OBSERVED):
1. Empty list -> `emptyList()`.
2. For `i` from `size` down to `0` inclusive:
   a. `after = getOrNull(i)` (null when i = size). If non-null, append.
   b. `before = getOrNull(i - 1)` (null when i = 0).
   c. `separator = generator(before, after)`; if non-null append.
3. Return `newList.asReversed()`.

Note the argument order: `generator(before, after)` where `before` is the element at `i-1` (earlier in the original list). After reversing, the final list is in original order. The separator sits between the same pair as in 4.1 (between `before` and `after`), but is appended in reversed iteration order before the reverse. For `[a, b]`: iteration appends `[ (i=2) sep(b,null), (i=1) b, sep(a,b), (i=0) a, sep(null,a)]`, then reverse => `[sep(null,a), a, sep(a,b), b, sep(b,null)]`. Identical to 4.1 for pure functions. Proposal: implement with the same loop for parity. Callers are `mihon/feature/upcoming/manga/UpcomingMangaScreenModel.kt` and `mihon/feature/upcoming/anime/UpcomingAnimeScreenModel.kt` under `app/src/main/java/`. The generator call order is kept so any side effect in a generator behaves the same.

### 4.3 `addOrRemove` (lines 41-47)
`HashSet<E>.addOrRemove(value, shouldAdd)`: `add` when `shouldAdd`, `remove` otherwise. Proposal: `if should_add { set.insert(v) } else { set.remove(&v) }`.

### 4.4 `fastFilterNot`, `fastPartition`, `fastCountNot` (lines 57-99)
- `fastFilterNot`: `fastFilter { !predicate(it) }`. Result preserves order.
- `fastPartition`: iterates once with `fastForEach`, appends to `first` when predicate is true, else to `second`. Returns `Pair(first, second)`.
- `fastCountNot`: `count = size` then decrements for each matching element; returns the number of non-matching elements.
- Doc comment (lines 52-55, 62-70, 86-92): "Do not use for collections that come from public APIs". Upstream restriction is about random access performance only.

Proposal: Rust `Iterator::filter`, `Iterator::partition`, and `iter().filter(|x| !p(x)).count()`. No separate functions.

Edge cases
- `insertSeparators` on a single element list returns exactly 3 entries: `[sep(null,a), a, sep(a,null)]`.
- The `R : Any` bound: the Rust type must be `Sized`; `null` separators are dropped (not represented).

---

## 5. Chapter and episode selection

Files:
- `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterGetNextUnread.kt` (33)
- `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt` (15)
- `app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt` (27)
- `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpsiodeGetNextUnseen.kt` (33)
- `app/src/main/java/eu/kanade/tachiyomi/util/episode/EpisodeFilterDownloaded.kt` (27)

Related (not in scope but called here): `app/src/main/java/eu/kanade/domain/items/chapter/model/ChapterFilter.kt` (`applyFilters`), `app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt`, `domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt`, `domain/src/main/java/tachiyomi/domain/items/episode/service/EpisodeSorter.kt`, `domain/src/main/java/tachiyomi/domain/entries/TriState.kt`.

### 5.1 `applyFilter` (TriState helper, `domain/src/main/java/tachiyomi/domain/entries/TriState.kt:5-9`)
- `DISABLED` -> `true` (keep)
- `ENABLED_IS` -> `predicate()`
- `ENABLED_NOT` -> `!predicate()`

### 5.2 Chapter view filters (`ChapterFilter.kt:16-35`, domain version)
Order, OBSERVED:
1. unread: `applyFilter(manga.unreadFilter) { !chapter.read }`.
2. bookmark: `applyFilter(manga.bookmarkedFilter) { chapter.bookmark }`.
3. downloaded: `applyFilter(manga.downloadedFilter) { downloadManager.isChapterDownloaded(chapter.name, chapter.scanlator, manga.title, manga.source) || isLocalManga }`.
4. `sortedWith(getChapterSort(manga))`.

The list version for UI items (`ChapterFilter.kt:42-53`) does the same with `ChapterList.Item.chapter` and `it.isDownloaded`, returning a lazy `Sequence`.

`Manga.downloadedFilter` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:23`) returns `TriState.ENABLED_IS` whenever the `BasePreferences.downloadedOnly()` preference is on, regardless of the per-manga filter. Episode twin: `EpisodeFilter.kt`. Keep this rule.

Boolean constants (upstream, `Manga.kt` companion): `CHAPTER_SHOW_UNREAD = 0x2`, `CHAPTER_SHOW_READ = 0x4`, `CHAPTER_UNREAD_MASK = 0x6`, `CHAPTER_SHOW_DOWNLOADED = 0x8`, `CHAPTER_SHOW_NOT_DOWNLOADED = 0x10`, `CHAPTER_DOWNLOADED_MASK = 0x18`, `CHAPTER_SHOW_BOOKMARKED` and `CHAPTER_SHOW_NOT_BOOKMARKED` (data layout in p1-data-and-config.md).

### 5.3 Sort rules (`ChapterSorter.kt:7-27`; exact)
`getChapterSort(manga, sortDescending = manga.sortDescending())` returns a comparator by `manga.sorting`:

| `manga.sorting` constant (value) | sortDescending = true | sortDescending = false |
|---|---|---|
| `CHAPTER_SORTING_SOURCE` (0x0) | `c1.sourceOrder.compareTo(c2.sourceOrder)` (ASCENDING, inverted) | `c2.sourceOrder.compareTo(c1.sourceOrder)` (descending) |
| `CHAPTER_SORTING_NUMBER` (0x100) | `c2.chapterNumber.compareTo(c1.chapterNumber)` | `c1.chapterNumber.compareTo(c2.chapterNumber)` |
| `CHAPTER_SORTING_UPLOAD_DATE` (0x200) | `c2.dateUpload.compareTo(c1.dateUpload)` | `c1.dateUpload.compareTo(c2.dateUpload)` |
| `CHAPTER_SORTING_ALPHABET` (0x300) | `c2.name.compareToWithCollator(c1.name)` | `c1.name.compareToWithCollator(c2.name)` |
| other | throws `NotImplementedError("Invalid chapter sorting method: ...")` | same |

OBSERVED quirk: the SOURCE branch is inverted relative to the other branches (`true` gives ascending `sourceOrder`). Keep the observed behaviour (the UI's "descending" for source order is ascending source index, which is newest-first on most sources). Document in the sort UI tests (area:37-ui-library).

Episode sorter (`EpisodeSorter.kt:7-27`) is the same with `Anime.EPISODE_SORTING_*`: SOURCE 0x0, NUMBER 0x200, UPLOAD_DATE 0x400, ALPHABET 0x600, and the same inverted SOURCE branch; uses `episodeNumber`, `dateUpload`, `name`, `sourceOrder`.

Direction bit: `sortDescending()` = `(flags & DIR_MASK) == DESC`, where `CHAPTER_SORT_DESC = 0x0` and `CHAPTER_SORT_ASC = 0x1`, mask `0x1`. Note that the default flags value 0 therefore means DESCENDING. Episode: `EPISODE_SORT_DESC = 0x0`, `EPISODE_SORT_ASC = 0x1`, mask `0x1`.

`compareToWithCollator` (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:13`) uses `Collator.getInstance(Locale.getDefault())` with `strength = Collator.PRIMARY`, lazily created once.

Proposal (Rust): `icu_collator::Collator` with `Strength::Primary`, locale from the system locale list (`glib::language_names()`), created once in a `OnceLock`.

### 5.4 `getNextUnread` (chapter, domain list) — `ChapterGetNextUnread.kt:12-20`
OBSERVED algorithm:
1. `chapters = applyFilters(manga, downloadManager)` (section 5.2: unread filter, bookmark filter, downloaded filter, then the sort of 5.3 applied).
2. If `manga.sortDescending()`: return `chapters.findLast { !it.read }`; else `chapters.find { !it.read }`.

Because the list is already sorted with `getChapterSort(manga)`, "find first unread in the sorted list" is the "next" chapter in the user's reading direction.

Quirks (observed, keep but document):
- The unread filter is applied BEFORE the search. If the manga's unread filter is `ENABLED_IS` (show unread only) the search is still correct. If the filter is `ENABLED_NOT` (show read only), `getNextUnread` can return `null` even though unread chapters exist, because `!it.read` is then never true. The upstream does not special-case this; the Rust port keeps it.
- Downloaded-only mode (5.2 point 3) can make the result `null` when no unread chapter is downloaded.

### 5.5 `getNextUnread` (UI items) — `ChapterGetNextUnread.kt:25-33`
Same as 5.4 but operates on `ChapterList.Item` (wrapper with `.chapter`), uses `applyFilters(manga)` (no download manager; uses `it.isDownloaded`), and returns `?.chapter`.

### 5.6 `getNextUnseen` (episode) — `EpsiodeGetNextUnseen.kt:12-33`
Identical to 5.4/5.5 with `episode.seen` instead of `read`. Upstream file name is misspelled (`EpsiodeGetNextUnseen.kt`); the Rust module is `episodes::next_unseen`.

### 5.7 `removeDuplicates` — `ChapterRemoveDuplicates.kt:8-15`
OBSERVED algorithm:
1. `groupBy { it.chapterNumber }`. Groups preserve the first-appearance order of each chapter number (`LinkedHashMap`).
2. For each group `(number, chapters)`, pick:
   a. `chapters.find { it.id == currentChapter.id }` (the chapter currently being read), else
   b. `chapters.find { it.scanlator == currentChapter.scanlator }`, else
   c. `chapters.first()`.
3. Output is one chapter per distinct `chapterNumber`, in first-appearance order of the groups.

Notes
- `chapterNumber` is a `Double` in the domain model. `groupBy` uses `Double` equality (so `1.0` and `1.00` are the same key; `NaN` is equal to itself in `Double.equals` but `NaN != NaN` in `==`. The Rust port keys on `f64::to_bits` after normalising `-0.0` to `0.0`).
- `scanlator` may be null; `null == null` matches in step b (observed). Rust: `Option<String>` equality.
- Called from `ReaderViewModel.kt:209` (after sorting ascending, only when pref `skipDupe` is on) and `ReaderViewModel.kt:481` (download-ahead).

### 5.8 Downloaded filter (chapter) — `ChapterFilterDownloaded.kt:13-26`
OBSERVED:
1. If `manga.isLocal()` (`source == LocalMangaSource.ID`, `0L`, `LocalMangaSource.kt:378`): return the list unchanged.
2. Otherwise keep chapters where `MangaDownloadCache.isChapterDownloaded(chapter.name, chapter.scanlator, manga.title, manga.source, false)` is true. The last argument is `skipCache = false` (uses the in-memory directory cache, renewed by `renewCache()`).

Episode twin (`EpisodeFilterDownloaded.kt`): `AnimeDownloadCache.isEpisodeDownloaded(episode.name, episode.scanlator, anime.title, anime.source, false)`.

Callers: `ReaderViewModel.kt:216` (downloaded-only mode) and `PlayerViewModel.kt:1343`.

Edge cases (observed)
- The cache check ignores `chapter.id`; it matches on name, scanlator, title, source. Two chapters with identical names in the same manga both match.

### 5.9 Sorting pipeline used by callers
Reader (`ReaderViewModel.kt:203-220`): `chaptersForReader.sortedWith(getChapterSort(manga, sortDescending = false))` then optional `removeDuplicates(selectedChapter)` (if `skipDupe`), then optional `filterDownloadedChapters(manga)` (if `downloadedOnly`). The reader always sorts ascending first. The player does the same with `getEpisodeSort(anime, sortDescending = false)` (`PlayerViewModel.kt:1340-1343`).

---

## 6. Covers, backgrounds, thumbnails

Files: `app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt` (65), `app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt` (74).

### 6.1 Cache file layout (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt:28-52`)
- Thumbnail cache dir: `getCacheDir(COVERS_DIR)`; file name `Hash.md5(thumbnailUrl)` lowercase hex (`DiskUtil.hashKeyForDisk` = `Hash.md5(key)`, `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:32-34`). Returns `null` file when URL is null.
- Custom cover dir: `getCacheDir(CUSTOM_COVERS_DIR)`; file name `Hash.md5(mangaId.toString())`.
- Anime background and cover caches follow the same pattern in `AnimeBackgroundCache.kt`, `AnimeCoverCache.kt` (owned by area:34-images-covers-cache).
- `hasCustomCover(coverCache)` = `coverCache.getCustomCoverFile(id).exists()` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:88-90`). Anime twin at `app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:112`.

### 6.2 `deleteFromCache(item, deleteCustom)` (OBSERVED, `MangaCoverCache.kt:75-89`)
1. `deleted = 0`.
2. `getCoverFile(item.thumbnailUrl)?.let { if (it.exists() && it.delete()) ++deleted }`.
3. If `deleteCustom`: `if (deleteCustomCover(item.id)) ++deleted` where `deleteCustomCover` deletes the custom file if it exists.
4. Return `deleted` (0..2).

### 6.3 `prepUpdateCover` (MangaExtensions.kt:19-41) — exact order
Called before updating `thumbnailUrl`. Inputs: `this` (current manga, OLD url), `remoteManga` (new SManga), `refreshSameUrl`.
1. `newUrl = remoteManga.thumbnail_url ?: return this` (null: never touch covers, the old URL may have gone stale).
2. `if (newUrl.isEmpty()) return this` (empty string: never "lose" a cover).
3. `if (!refreshSameUrl && thumbnailUrl == newUrl) return this`.
4. `when`:
   - `isLocal()` -> `copy(coverLastModified = Instant.now().toEpochMilli())`. No cache deletion (local covers are files on disk).
   - `hasCustomCover(coverCache)` -> `coverCache.deleteFromCache(this, false)` (deletes the OLD thumbnail file only) and return `this` UNCHANGED (no `coverLastModified` bump, because the custom cover keeps showing).
   - else -> `coverCache.deleteFromCache(this, false)` then `copy(coverLastModified = now ms)`.

The timestamp is epoch milliseconds (`Instant.now().toEpochMilli()`), stored on the entry and used as a cache-buster in the cover URL.

### 6.4 `removeCovers` (both entries)
Manga (`MangaExtensions.kt:43-50`), anime (`AnimeExtensions.kt:18-25`):
1. `isLocal()` -> return `this`.
2. `deleted = coverCache.deleteFromCache(this, true)` (thumbnail + custom).
3. `deleted > 0` -> `copy(coverLastModified = now ms)`, else `this`.

Anime background removal (`AnimeExtensions.kt:27-34`) is identical using `backgroundCache.deleteFromCache(this, true)` and `backgroundLastModified`.

### 6.5 `editCover` / `editBackground` (suspend)
Manga (`MangaExtensions.kt:52-65`):
1. `isLocal()` -> `coverManager.update(toSManga(), stream)` then `updateManga.awaitUpdateCoverLastModified(id)`.
2. else if `favorite` -> `coverCache.setCustomCoverToCache(this, stream)` (copies stream to the custom cover file; throws `IOException` on error) then `awaitUpdateCoverLastModified(id)`.
3. else -> no-op. The stream is NOT closed here; the caller owns it.

Anime cover (`AnimeExtensions.kt:36-49`) and background (`:51-64`) follow the same pattern with `LocalAnimeCoverManager`, `LocalAnimeBackgroundManager`, `awaitUpdateCoverLastModified` / `awaitUpdateBackgroundLastModified`.

### 6.6 `editThumbnail` (episode, `AnimeExtensions.kt:66-74`)
`fun SEpisode.editThumbnail(anime, thumbnailManager, stream)`: only when `anime.isLocal()` -> `thumbnailManager.update(anime.toSAnime(), this, stream)`. Non-local: silent no-op (no error). Remote episode thumbnails are not cached by this function.

Proposal (Rust, `kuro-core::covers`)
- `pub fn prep_update_cover(cur: &CoverState, new_url: Option<&str>, refresh_same_url: bool, now_ms: i64) -> CoverDecision` where `CoverDecision` is `Keep | BumpOnly(now) | DeleteThumbAndBump(now) | DeleteThumbOnly`. Pure, testable.
- File operations run through `kuro-platform::files` with the same names (MD5 hex of URL, MD5 hex of id) so existing caches keep working if the user copies them across. Verify MD5 use with `md5` crate; no other hash is accepted.

Edge cases
- A custom cover removal (`removeCovers`) deletes both files; if only the thumbnail existed, the return value is 1 and the timestamp is still bumped.
- `coverLastModified` is `Long` (ms). Default `0L` for new entries (`Anime.kt:327`).

---

## 7. Airing schedule lookup (`AniChartApi`)

File: `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt` (165 lines). Caller: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1536-1543`.

### 7.1 Caller behaviour (OBSERVED)
`AnimeScreenModel.fetchAiringTime(anime, trackItems, manualFetch)`:
1. `airingEpisodeData = AniChartApi().loadAiringTime(anime, trackItems, manualFetch)` — a new `AniChartApi` (and a new bare `OkHttpClient`) per call.
2. `setAnimeViewerFlags.awaitSetNextEpisodeAiring(anime.id, airingEpisodeData)` — ALWAYS writes both fields, even when the result is `(1, 0L)`.
3. `updateSuccessState { it.copy(nextAiringEpisode = airingEpisodeData) }`.

### 7.2 `loadAiringTime` (lines 22-49) — exact
Signature: `internal suspend fun loadAiringTime(anime: Anime, trackItems: List<AnimeTrackItem>, manualFetch: Boolean): Pair<Int, Long>`.
1. `airingEpisodeData = Pair(anime.nextEpisodeToAir, anime.nextEpisodeAiringAt)` (stored values, decoded from viewer flags, section 7.8).
2. If `anime.status == SAnime.COMPLETED.toLong() && !manualFetch` -> return `airingEpisodeData`. (`SAnime.COMPLETED = 2`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:74`.) No network.
3. Inside `withIOContext`:
   a. `matchingTrackItem = trackItems.firstOrNull { (tracker is Anilist && track != null) || (tracker is MyAnimeList && track != null) || (tracker is Simkl && track != null) }`. The first in list order wins.
   b. If none: `return Pair(1, 0L)`. NOTE: this returns a fresh default, NOT the stored data, and the caller writes it. A user with no linked tracker therefore loses the previously stored airing info on each fetch. Observed; the Rust port keeps the return value but flags it in the tasks as a decision (T-52a-013).
   c. `item.track!!.remoteId` (Long) dispatch:
      - `Anilist` -> `getAnilistAiringEpisodeData(remoteId)`
      - `MyAnimeList` -> `getAnilistAiringEpisodeData(getAlIdFromMal(remoteId))`
      - `Simkl` -> `getSimklAiringEpisodeData(remoteId)`
      - `else` -> `Pair(1, 0L)` (unreachable given the filter).
   d. Return `airingEpisodeData` (the value assigned in c; if c was skipped it is the stored pair).

### 7.3 HTTP client and request format (OBSERVED)
- `client = OkHttpClient()` (line 20). No app interceptors, no custom timeouts (OkHttp defaults: 10 s connect, 10 s read, 10 s write), no User-Agent.
- AniList: `POST("https://graphql.anilist.co", body = buildJsonObject { put("query", query) }.toString().toRequestBody(jsonMime))`. `jsonMime = "application/json; charset=utf-8"` (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:22`). The `POST` helper (`core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:43-56`) sends empty headers and `Cache-Control: max-age=600` (default `CacheControl.Builder().maxAge(10, MINUTES)`).
- Simkl: `GET("https://data.simkl.in/calendar/$it.json")` — `GET(url)` helper (`Requests.kt:20-29`) with default headers and `Cache-Control: max-age=600`.
- GraphQL query text (`trimMargin`, leading indentation is kept because the lines do not start with `|`, and the first and last blank lines are removed). GraphQL is whitespace-insensitive, so the Rust port sends the compact form shown below. The compact form is the only deviation and has no effect on results.

### 7.4 `getAlIdFromMal(idMal)` (lines 51-76)
1. Query: `query { Media(idMal:<idMal>,type: ANIME) { id } }` (`<idMal>` is the Long, interpolated).
2. POST as 7.3. On any exception from `execute()` -> return `0L`.
3. Parse: `response.body.string().substringAfter("id\":").substringBefore("}").toLongOrNull() ?: 0L`.
   - Success example `{"data":{"Media":{"id":1535}}}` -> `1535`.
   - No media (`{"data":{"Media":null}}`) -> `substringAfter` returns the whole body (delimiter absent); `substringBefore("}")` gives `{"data":{"Media":null` -> not a Long -> `0L`.
   - HTTP errors are not checked; the body is parsed the same way.
4. Returns `0L` for "not found". The caller then calls `getAnilistAiringEpisodeData(0)`, which sends `Media(id:0)`. This is an observed wasted request; the Rust port short-circuits `0` to `(1, 0)`.

### 7.5 `getAnilistAiringEpisodeData(id)` (lines 78-107)
1. Query: `query { Media(id:<id>) { nextAiringEpisode { episode airingAt } } }`.
2. POST. On exception -> `Pair(1, 0L)`.
3. `data = body.string()`.
4. `episodeNumber = data.substringAfter("episode\":").substringBefore(",").toIntOrNull() ?: 1`.
5. `airingAt = data.substringAfter("airingAt\":").substringBefore("}").toLongOrNull() ?: 0L`.
6. Return `Pair(episodeNumber, airingAt)`.

Observed results
- `{"data":{"Media":{"nextAiringEpisode":{"episode":12,"airingAt":1765000000}}}}` -> `(12, 1765000000)`.
- `{"data":{"Media":{"nextAiringEpisode":null}}}` -> `episode` marker absent so `episodeNumber` parses the whole first chunk, which is not an int -> `1`; `airingAt` marker absent -> `0`. Result `(1, 0)`.

Proposal: parse with `serde_json` into `Option<struct { episode: i32, airing_at: i64 }>`. Result for null: `(1, 0)` to match the stored sentinel, so the UI output is identical.

### 7.6 `getSimklAiringEpisodeData(id)` (lines 109-148)
State: `episodeNumber = 1`, `airingAt = 0L`.
For `calendarTypes = ["anime", "tv", "movie_release"]`, in this order:
1. `response = client.newCall(GET("https://data.simkl.in/calendar/$it.json")).execute()`. On exception -> `return Pair(1, 0L)` (discards progress made on earlier types).
2. `body = response.body.string()`.
3. `data = removeAiredSimkl(body)` (7.7).
4. `malId = data.substringAfter("\"simkl_id\":$id,", "").substringAfter("\"mal\":\"").substringBefore("\"").toLongOrNull() ?: 0L`.
   - If `malId != 0` -> `return getAnilistAiringEpisodeData(getAlIdFromMal(malId))`. The Simkl air date is ignored in this case.
5. `epNum = data.substringAfter("\"simkl_id\":$id,", "").substringBefore("\"}}").substringAfterLast("\"episode\":")`.
   - `episodeNumber = epNum.substringBefore(",").toIntOrNull() ?: episodeNumber`.
6. `date = data.substringBefore("\"simkl_id\":$id,", "").substringAfterLast("\"date\":\"").substringBefore("\"")`.
   - `airingAt = if (date.isNotBlank()) toUnixTimestamp(date) else airingAt`.
7. If `airingAt != 0L` -> `return Pair(episodeNumber, airingAt)`.
After the loop: `return Pair(episodeNumber, airingAt)` (may be `(1, 0)`).

Sample of the real Simkl calendar (fetched read-only for this plan, `https://data.simkl.in/calendar/anime.json`, first 700 bytes): an array of objects with these keys in this order: `title`, `poster`, `date` (ISO 8601 with offset, e.g. `"2026-10-07T00:00:00+09:00"`), `release_date`, `rank`, `ratings` (`{"simkl":{"rating":8.5,"votes":4}}`), `url`, `ids` (`{"simkl_id":3194924,"slug":"...","tmdb":null,"mal":"62248"}`), `episode` (`{"episode":14,"url":"..."}`), `anime_type` (`"ona"`). `mal` is a JSON string or `null`.

Fragility (OBSERVED from the algorithm, relevant to the sample):
- `substringAfter("\"mal\":\"")` when the target's `mal` is `null` searches the REST of the string and can take the MAL id of a LATER entry. Wrong airing source.
- `substringBefore("\"}}")` depends on that exact byte sequence occurring; if it does not, `substringBefore` returns the whole remainder and `substringAfterLast("\"episode\":")` takes the last episode number in the file. Wrong episode number.

Proposal (Rust): deserialize each calendar with `serde_json` into `Vec<CalendarEntry { date: DateTime<FixedOffset>, ids: { simkl_id: i64, mal: Option<String> }, episode: { episode: i32 } }>`. Select the first entry where `ids.simkl_id == id` and `date > now`. Use `date` to compute `airing_at` and `episode.episode` for the episode number. If `mal` is present, use the AniList path (7.5) as the observed code does. Documented in topic file `p1-topic-airing-and-simkl.md`.

### 7.7 `removeAiredSimkl(body)` (lines 150-158)
OBSERVED:
1. `now = Calendar.getInstance().timeInMillis` (ms).
2. `parts = body.split("\"date\":\"").drop(1)`.
3. `index = parts.indexOfFirst { date = it.substringBefore("\""); time = if (date.isNotBlank()) toUnixTimestamp(date) else 0L; time * 1000 > now }`.
4. Return `if (index >= 0) body.substring(index) else ""`.

BUG (observed): `index` is an index into `parts` (the list of segments), NOT a character offset into `body`. `body.substring(index)` therefore slices at the wrong position (the k-th character instead of the k-th upcoming entry). For the first upcoming entry at segment 40 the slice starts at character 40, inside the first entry. The effect is that the "already aired entries are pruned" intent is not achieved. The search for the target id still finds the first matching `simkl_id`, which may be an entry that already aired (its date is in the past, so `airingAt` is in the past). The Rust port selects the first future entry for the id (section 7.6 Proposal), which is the intended behaviour.

Other observed behaviour
- `toUnixTimestamp` (`OffsetDateTime.parse(dateFormat).toInstant().epochSecond`) throws `DateTimeParseException` for any date without an offset. This is not caught in `removeAiredSimkl` or `getSimklAiringEpisodeData`; it propagates out of `loadAiringTime`. Rust: parse errors on a single entry are skipped, not fatal.
- `Calendar.getInstance()` uses the system zone; the comparison is zone-independent because both sides are epoch-based.

### 7.8 Stored representation of the result (decoded in `Anime`, encoded in `SetAnimeViewerFlags`)
Source: `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:79-83`, `:310-312`; `app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt:30-58`.
- `nextEpisodeToAir: Int` = `(viewerFlags and 0x000000000FFFF00).removeHexZeros(2).toInt()`; i.e. bits 8..23 (16 bits) hold the episode number; stored as `episode * 16^2` (`addHexZeros(2)`), so `episode = (bits) / 256`.
- `nextEpisodeAiringAt: Long` = `(viewerFlags and 0x0FFFFFFFF000000).removeHexZeros(6)`; i.e. bits 24..55 (32 bits) hold Unix seconds; stored as `seconds * 16^6` (`addHexZeros(6)`), so `seconds = (bits) / 2^24`.
- Write path `awaitSetNextEpisodeAiring`: first writes the episode (`updateAnime` #1), then the time (`updateAnime` #2). Two non-atomic writes; a crash between them leaves a new episode with an old time. Proposal: one `UPDATE` with both masks (single statement, see area:04-db-anime-schema-migrations).
- Range limits: episode values above 65535 are silently truncated by the mask; times above `2^32 - 1` (2106-02-07T06:28:15Z) are truncated.
- `setFlag(flag, mask)` = `this and mask.inv() or (flag and mask)` (`SetAnimeViewerFlags.kt:51-53`).

### 7.9 Summary of the airing algorithm for the Rust port
`airing::load(anime, tracks, manual) -> Result<(i32, i64), AiringError>`:
1. If `anime.status == 2 && !manual` -> stored pair.
2. First track with `remote_id` for AniList, MAL or Simkl. None -> `(1, 0)`.
3. Dispatch as 7.2c with the fixes listed in 7.4, 7.5, 7.6, 7.7.
4. On network error: `(1, 0)` (observed; a Rust `Err` is converted to `(1, 0)` at the boundary to match the stored sentinel). Keep the observed sentinel so the UI stays identical.

---

## 8. Crash report and debug information

File: `app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt` (118 lines). Caller: `CrashScreen.kt`, `SettingsAdvancedScreen.kt`, `AboutScreen.kt` (presentation layer, area:36-ui-shell-navigation-security-webview / area:47-settings-screens-general).

### 8.1 `dumpLogs(exception: Throwable? = null)` (lines 26-42) — exact
Runs inside `withNonCancellableContext`.
1. `file = context.createFileInCacheDir("aniyomi_crash_logs.txt")` (section 9.5). Deletes an existing file first.
2. `file.appendText(getDebugInfo() + "\n\n")`.
3. `getMangaExtensionsInfo()?.let { file.appendText("$it\n\n") }`.
4. `getAnimeExtensionsInfo()?.let { file.appendText("$it\n\n") }`.
5. `exception?.let { file.appendText("$it\n\n") }` (`Throwable.toString()` = `<class name>: <message>`).
6. `Runtime.getRuntime().exec("logcat *:E -d -v year -v zone -f ${file.absolutePath}").waitFor()`.
   - OBSERVED hazard: `logcat -f` writes its own output file, which may REPLACE the text appended in steps 2-5. Treat as an open issue; the Rust port writes the logs last and appends, so nothing is lost.
7. `uri = file.getUriCompat(context)`; `context.startActivity(uri.toShareIntent(context, "text/plain"))` (share chooser titled `MR.strings.action_share`, "Share").
8. Any `Throwable` -> `withUIContext { context.toast("Failed to get logs") }` (LITERAL).

### 8.2 `getDebugInfo()` (lines 44-65) — exact template (`trimIndent`)
Lines, in order (the placeholders are the Android values; the Linux equivalents are the Proposal column):

| Line label (LITERAL) | Android source | Proposal (Linux) |
|---|---|---|
| `App ID:` | `BuildConfig.APPLICATION_ID` | Rust const `APP_ID` (set in `kurobayomi/src/main.rs`) |
| `App version:` | `VERSION_NAME (COMMIT_SHA, VERSION_CODE, BUILD_TIME)` | `env!("CARGO_PKG_VERSION")`, `option_env!("KUROBAYOMI_COMMIT")`, build timestamp from `build.rs` |
| `Android version:` | `Build.VERSION.RELEASE (SDK SDK_INT; build DISPLAY)` | `OS:` from `/etc/os-release` `PRETTY_NAME`, plus `kernel` from `/proc/sys/kernel/osrelease` |
| `Android build ID:` | `Build.DISPLAY` | `Desktop session:` from `XDG_CURRENT_DESKTOP` and `XDG_SESSION_TYPE` |
| `Device brand:` | `Build.BRAND` | `Hardware:` from `/sys/class/dmi/id/sys_vendor` (fallback `unknown`) |
| `Device manufacturer:` | `Build.MANUFACTURER` | (merged into `Hardware:`) |
| `Device name:` | `Build.DEVICE (Build.PRODUCT)` | `Hardware:` `/sys/class/dmi/id/product_name` |
| `Device model:` | `Build.MODEL` | (merged into `Hardware:`) |
| `WebView:` | `WebViewUtil.getVersion(context)` | `GTK:` `gtk::major_version()`.`minor`.`micro`; `GLib:` `glib::major_version()` etc. |
| `Current time:` | `OffsetDateTime.now(ZoneId.systemDefault())` (ISO-8601 with offset) | `chrono::Local::now().to_rfc3339()` |
| `MPV version:` | LITERAL `6764488` | `libmpv` property `mpv-version` (read at runtime) |
| `Libplacebo version:` | LITERAL `v7.349.0` | `libmpv` property `libplacebo-version` (read at runtime; verify availability on the target libmpv) |
| `FFmpeg version:` | LITERAL `n7.1` | `libmpv` property `ffmpeg-version` (read at runtime) |

The original file has two commented-out lines (lines 60-64) that would read `Utils.VERSIONS.*`; the Rust port reads those values directly from libmpv instead.

The `trimIndent` output has no leading or trailing blank lines. The Rust port emits the same field order and labels.

### 8.3 `getMangaExtensionsInfo()` (lines 67-91) and `getAnimeExtensionsInfo()` (lines 93-117)
Algorithm (identical for both except the label, see below):
1. `availableExtensions = manager.availableExtensionsFlow.value.associateBy { it.pkgName }`.
2. `installedExtensionsFlow.value.sortedBy { it.name }` (natural `String` order, case-sensitive).
3. For each installed extension `it`:
   - `availableExtension = availableExtensions[it.pkgName]`.
   - `hasUpdate = (availableExtension?.versionCode ?: 0) > it.versionCode`.
   - If `!hasUpdate && !it.isObsolete` -> skip.
   - Otherwise the block (raw string, `trimIndent`):
     ```
     - <it.name>
       Installed: <it.versionName> / Available: <availableExtension?.versionName ?: "?">
       Orphaned: <it.isObsolete>        (manga)
       Obsolete: <it.isObsolete>        (anime)
     ```
4. If the list is empty -> `null`. Else `listOf("Problematic extensions:") + items` joined with `"\n"`.

LITERALS: `Problematic extensions:`, `Installed:`, `Available:`, `?`, `Orphaned:` (manga), `Obsolete:` (anime). The label mismatch is observed; the Rust port uses `Obsolete:` for both (Proposal; the field is the same, the report is a diagnostic).

Dependencies: `MangaExtensionManager` / `AnimeExtensionManager` (`availableExtensionsFlow`, `installedExtensionsFlow`, `pkgName`, `versionCode`, `versionName`, `isObsolete`, `name`). Owned by area:18-extensions-manga and area:19-extensions-anime. The Rust port receives a snapshot `Vec<ExtensionInfo>` and does not depend on the manager.

### 8.4 Proposal: crash logging on Linux
- A panic hook (`std::panic::set_hook`) writes `<cache>/kurobayomi/crash_<unix-seconds>.txt` with `getDebugInfo()` + panic payload + backtrace (`std::backtrace::Backtrace::force_capture()`), then calls the default hook.
- The manual "dump logs" action writes `<cache>/kurobayomi/kurobayomi_crash_logs.txt` (same name pattern, `_` instead of `aniyomi_`) containing debug info, the extension list, the last 500 lines of the in-process log ring buffer, and the exception text. Then it opens a GTK save dialog (portal `org.freedesktop.portal.FileChooser`) so the user picks where to save. The original opens a share sheet; a save dialog is the Linux equivalent.
- Messages: `Failed to get logs` (LITERAL, kept).

---

## 9. Platform helpers

### 9.1 PKCE code verifier (`app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt`)
OBSERVED
- `ByteArray(50)`, filled by `SecureRandom().nextBytes`, encoded with `Base64.getUrlEncoder().withoutPadding()`. Output length is 67 characters from `[A-Za-z0-9_-]`.
- Used once by MyAnimeListApi (`app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt:425-428`, `getPkceChallengeCode()` sets `codeVerifier = PkceUtil.generateCodeVerifier()` and returns it as the challenge).

Proposal: `rand::rngs::OsRng` + `base64::engine::general_purpose::URL_SAFE_NO_PAD`. Same length, same alphabet. Store the verifier in the auth flow state only (never log it).

### 9.2 Size formatting (`app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt:8-18`)
`Long.toSize()` exact:
- `kb = 1000`, `mb = kb*kb`, `gb = mb*kb` (decimal, NOT 1024).
- `this >= gb` -> `"%.2f GB".format(this.toFloat() / gb)`.
- `this >= mb` -> `"%.2f MB"`, `this >= kb` -> `"%.2f KB"`, else `"$this B"`.
- Notes: `toFloat()` loses precision above 2^24 bytes (16 MiB); the Rust port divides as `f64`. `format` uses the default locale (decimal comma in many locales); the Rust port always uses `.`. Negative values fall into the `else` branch and print as e.g. `-5 B`.
- Units are literal strings `GB`, `MB`, `KB`, `B` (not translated).

Examples: `999` -> `999 B`; `1000` -> `1.00 KB`; `1_500_000` -> `1.50 MB`; `2_000_000_000` -> `2.00 GB`.

### 9.3 Directory size (`StorageUtil.kt:23-38`)
`UniFile.size()`:
1. If `isDirectory`: `listFiles()?.forEach` (null -> 0); for each child: if child is a directory, add `child.size()` (recursive); else add `child.length()` when `> 0`, else 0.
2. If a file: `length()`.
3. Returns `Long` total.

Notes: no symlink handling (Java `File.isDirectory` follows symlinks, so a symlink loop recurses forever). The Rust port uses `symlink_metadata` and does not follow symlinks (Proposal; this is a deliberate deviation to avoid an infinite loop, no user-visible loss in normal layouts). Errors while listing a child are ignored (upstream returns 0 for that child).

Adjacent code (not in scope): `DiskUtil.getDirectorySize` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:36`) is the same operation used by storage screens; owned by area:13-core-util-storage-archive-torrent.

### 9.4 Stream save (`app/src/main/java/eu/kanade/tachiyomi/util/storage/OkioExtensions.kt`)
`BufferedSource.saveTo(file: File)` (lines 14-26):
1. `try`: `file.parentFile?.mkdirs()` (result ignored); then `saveTo(file.outputStream())`.
2. `catch (e: Exception)`: `close()` (source), `file.delete()` (result ignored), rethrow `e`.

`BufferedSource.saveTo(stream: OutputStream)` (lines 33-40): `use { input -> stream.sink().buffer().use { it.writeAll(input); it.flush() } }`. Closes both the source and the stream (the stream is closed by the buffered sink's `use`).

Proposal (Rust, `kuro-platform::files::save_stream`):
- `async fn save_stream(src: impl AsyncRead, dest: &Path) -> io::Result<()>`:
  1. `create_dir_all(dest.parent())` (error propagated; upstream ignores mkdirs failure but then fails on open, so the observable result is the same).
  2. Write to `dest.with_extension(<ext>.part)` (`<name>.part`), `tokio::io::copy`, `flush`, `sync_data`, then `rename` to `dest` (atomic on the same filesystem).
  3. On any error: remove the `.part` file (not `dest`), return the error.
- Reason for the deviation: a crash during the copy leaves `dest` absent, not truncated. Upstream could leave a truncated `dest` if the process died (the delete only runs on exceptions). No user-visible change otherwise.

### 9.5 `createFileInCacheDir(name)` (`ContextExtensions.kt:104-111`)
1. `file = File(externalCacheDir, name)`.
2. If `file.exists()` -> `file.delete()`.
3. `file.createNewFile()`; return `file`.
Proposal: `glib::user_cache_dir().join("kurobayomi").join(name)`; create the directory; remove an existing file; create an empty file. Return a `PathBuf`.

### 9.6 Clipboard (`ContextExtensions.kt:39-55`)
`copyToClipboard(label, content)`:
1. If `content.isBlank()` -> return (no toast, no copy).
2. `clipboard.setPrimaryClip(ClipData.newPlainText(label, content))`.
3. If `Build.VERSION.SDK_INT <= S_V2` (Android 12L and lower): toast `MR.strings.copied_to_clipboard` with `content.truncateCenter(50)`. Newer Android shows its own confirmation.
4. Catch `Throwable`: `logcat(ERROR, e)` and toast `MR.strings.clipboard_copy_error`.

Message keys
- `MR.strings.copied_to_clipboard` = `Copied to clipboard:\n%1$s`.
- `MR.strings.clipboard_copy_error` = `Failed to copy to clipboard`.

Proposal: `gdk::Display::default().clipboard().set_text(&content)`. Show a toast with `Copied to clipboard:\n<truncated 50 chars>` ALWAYS (Linux has no system confirmation). `truncateCenter(50)` is in `app/src/main/java/eu/kanade/tachiyomi/util/lang/...` (not in scope); `truncateCenter` is defined outside this scope (imported from `eu.kanade.tachiyomi.util.lang`), so its exact rule is not verified here. Before implementing, read its definition in the Aniyomi tree and port it to `kuro-core::util::text::truncate_center` with the same output for 50 characters. Until then, the toast uses the full content.

Edge: `set_text` on Wayland needs the window to have focus; if the call fails (no display), show `Failed to copy to clipboard`.

### 9.7 Open URL (`ContextExtensions.kt:71-102`)
`openInBrowser(url|uri, forceDefaultBrowser = false)`:
1. `Intent(ACTION_VIEW, uri)`; when `forceDefaultBrowser`, sets the package to the default browser unless it is in `invalidDefaultBrowsers` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtil.kt:74-84`: `android`, `com.hihonor.android.internal.app`, `com.huawei.android.internal.app`, `com.zui.resolver`, `com.transsion.resolver`, `com.android.intentresolver`).
2. `startActivity`. On `Exception` -> `toast(e.message)` (the raw exception message, unlocalised).

Proposal: `gtk::UriLauncher::new(url).launch(Some(&window), None::<&Cancellable>, |r| ...)` (GTK 4.10+, so OK on 4.16) which uses the portal `org.freedesktop.portal.OpenURI` under Flatpak and `gio::AppInfo::launch_default_for_uri` natively. `forceDefaultBrowser` is dropped: the Linux equivalent (`xdg-settings get default-web-browser`) is already what `launch_default_for_uri` uses. Loss: none on KDE.

Error messages: on failure show the error's display text (LITERAL behaviour: raw message).

### 9.8 Reader theme background (`ContextExtensions.kt:119-147`)
`createReaderThemeContext()`: decides dark or light background for the reader and returns a context with night-mode overridden.
- Inputs: `UiPreferences.themeMode()` (default `ThemeMode.SYSTEM`, key `pref_theme_mode_key`), `ReaderPreferences.readerTheme()` (default `1`, key `pref_reader_theme_key`).
- `isDarkBackground`:
  - `readerTheme` 1 (Black) or 2 (Gray) -> `true`.
  - 3 (Automatic) -> if `themeMode == SYSTEM` then `isNightMode()`, else `themeMode == DARK`.
  - anything else (0 White) -> `false`.
- Values come from the reader background chips (`app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt:19-24`): Black=1, Gray=2, White=0, Automatic=3.
- The override only applies when the current night mode differs from the expected one.

Proposal (`kuro-core::reader::theme`): pure function `reader_background(theme: i32, theme_mode: ThemeMode, system_dark: bool) -> Background { Light | Dark }`. The GTK reader applies CSS class `reader-dark` or `reader-light` to its page surface. No Context wrapping.

Labels (MR): `black_background` = `Black`, `gray_background` = `Gray`, `white_background` = `White`, `automatic_background` = `Auto`.

### 9.9 Content size (`ContextExtensions.kt:154-156`)
`getUriSize(uri)`: `UniFile.fromUri(this, uri)?.length()?.takeIf { it >= 0 }`. Null when unknown. Rust: `std::fs::metadata(path).map(|m| m.len()).ok()`; for `file://` URIs only. Non-file schemes return `None`.

### 9.10 Package and installer checks (drop)
- `isPackageInstalled(pkg)` (`ContextExtensions.kt:161-168`), `hasMiuiPackageInstaller`, `isShizukuInstalled`, `launchRequestPackageInstallsPermission` (`:170-179`): Android package manager and install-permission settings. Drop. Loss: none, Linux extensions do not use Android installers.
- `hasPermission(permission)` (`:63-66`): Android runtime permission. Drop; Linux uses portals (no runtime check).
- `powerManager` (`:68-69`): Replace with `gtk::Application::inhibit(Some(&window), ApplicationInhibitFlags::IDLE, Some("Playing video"))` during playback (area:45-player-core).

### 9.11 Animation scale (`AnimationExtensions.kt:10-11`)
`Context.animatorDurationScale: Float = Settings.Global.getFloat(ANIMATOR_DURATION_SCALE, 1f)`.
- Used at `App.kt:203` (`crossfade((300 * scale).toInt())`) and `ReaderPageImageView.kt:421` (`(this * scale).toInt().coerceAtLeast(1)`).

Proposal (`kuro-platform::appearance::animation_scale() -> f32`):
1. If `gtk::Settings::default().gtk_enable_animations()` is `false` -> `0.0`.
2. Else read KDE `AnimationDurationFactor` from `~/.config/kdeglobals`, group `[KDE]` (default `1.0`). Verify the exact key on the target machine before relying on it.
3. Clamp to `[0.0, 10.0]`.
The crossfade uses `(300 * scale).toInt()` ms; page animation uses `coerceAtLeast(1)`. Observed rule kept.

### 9.12 Build flags (`BuildConfig.kt`)
- `updaterEnabled`: `BuildConfig.UPDATER_ENABLED`. Rust: `false` (in-app updater dropped, owned by area:35-data-misc-updater-notification-export).
- `isDebugBuildType`: `BuildConfig.BUILD_TYPE == "debug"` -> `cfg!(debug_assertions)`.
- `isPreviewBuildType`: `BUILD_TYPE == "preview"` -> no equivalent; Rust: `false` (no preview channel).
- `isReleaseBuildType`: `BUILD_TYPE == "release"` -> `!cfg!(debug_assertions)`.

---

## 10. Date and time (`app/src/main/java/eu/kanade/tachiyomi/util/lang/DateExtensions.kt`)

### 10.1 Message keys used
- `MR.strings.relative_time_today` = `Today`
- `MR.plurals.relative_time` = one: `Yesterday`; other: `%1$d days ago`
- `MR.plurals.upcoming_relative_time` = one: `Tomorrow`; other: `In %1$d days`
- `AYMR.strings.relative_time_now` = `Now`
- `AYMR.plurals.relative_time_hours` = one: `1 hour ago`; other: `%1$d hours ago`
- `AYMR.plurals.relative_time_minutes` = one: `1 minute ago`; other: `%1$d minutes ago`
- `AYMR.plurals.upcoming_relative_time_hours` = one: `In 1 hour`; other: `In %1$d hours`
- `AYMR.plurals.upcoming_relative_time_minutes` = one: `In 1 minute`; other: `In %1$d minutes`

Plural selection: Android `pluralStringResource(res, quantity, formatArg)` chooses by the locale's CLDR plural rules. English uses `one` for exactly 1 and `other` otherwise. The Rust port uses `icu_plurals::PluralRules::try_new_cardinal(locale)`.

### 10.2 `LocalDate.toRelativeString(context, relative = true, dateFormat = ofLocalizedDate(SHORT))` (lines 47-72) — exact
- If `!relative` -> `dateFormat.format(this)`.
- `now = LocalDate.now()` (system zone). `difference = ChronoUnit.DAYS.between(this, now)` (signed: today minus this).
- `difference < -7` -> `dateFormat.format(this)`.
- `difference < 0` -> `upcoming_relative_time(|difference|)`. (`-1` -> `Tomorrow`, `-7` -> `In 7 days`.)
- `difference < 1` (i.e. `0`) -> `Today`.
- `difference < 7` (`1..6`) -> `relative_time(difference)`. (`1` -> `Yesterday`, `6` -> `6 days ago`.)
- else (`>= 7`) -> `dateFormat.format(this)`.

### 10.3 `LocalDateTime.toRelativeString(context, relative = true, dateFormat = ofLocalizedDate(SHORT))` (lines 75-131) — exact
Used for chapter and episode release times.
- If `!relative` -> `dateFormat.format(this)`.
- `now = LocalDateTime.now()`.
- `timeDifference = DAYS.between(this, now)` (whole 24 h periods, truncated toward zero).
- `dateDifference = DAYS.between(this.toLocalDate(), now.toLocalDate())` (calendar days).
- Branch on `timeDifference`:
  1. `< -7` -> `dateFormat.format(this)`.
  2. `< 0` -> `upcoming_relative_time(|dateDifference|)` (note: count is the CALENDAR difference).
  3. `< 1` (i.e. `0`) -> hour branch:
     - `hourDifference = HOURS.between(this, now)` (truncated).
     - `< 0` -> `AYMR upcoming_relative_time_hours(|hourDifference|)`.
     - `< 1` (i.e. `0`) -> minute branch:
       - `minuteDifference = MINUTES.between(this, now)`.
       - `< 0` -> `AYMR upcoming_relative_time_minutes(|minuteDifference|)`.
       - `== 0` -> `AYMR relative_time_now` (`Now`). Covers 0 to 59 seconds in the past and up to 59 seconds in the future.
       - else -> `AYMR relative_time_minutes(minuteDifference)`.
     - else -> `AYMR relative_time_hours(hourDifference)`.
  4. `< 7` (`1..6`) -> `relative_time(dateDifference)` (MR plural, count is the CALENDAR difference).
  5. else -> `dateFormat.format(this)`.

Observed quirks (keep)
- A release 30 seconds in the future shows `Now` (minute diff 0).
- Step 2 uses the calendar difference for the count, so a release 25 hours ahead on the next-next day may read `In 2 days`.
- Step 4 uses the calendar difference, so a release exactly 24 h ago can show `1 day`-based text computed from dates.

### 10.4 Absolute formats
- `LocalDateTime.toDateTimestampString(dateTimeFormatter)`: `"$date $time"` where `date = dateTimeFormatter.format(this)` and `time = DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT).format(this)` (`DateExtensions.kt:19-23`). Example en_US: `1/5/26 3:04 PM`.
- `Date.toTimestampString()`: `DateFormat.getTimeInstance(DateFormat.SHORT).format(this)`, time only (`:25-27`). Example en_US: `3:04 PM`.
- `dateFormat` default: `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)` (locale short date).

Proposal (ICU4X `icu_datetime`, length Short): date `DateFormatter::try_new_with_length(locale, Length::Short)`, time `TimeFormatter::try_new_with_length(locale, Length::Short)`. Both locale-aware, close to Java's SHORT.

### 10.5 Zone and epoch helpers
- `Long.convertEpochMillisZone(from, to)` (`:29-37`): interprets the epoch millis as a wall-clock time in `from`, then returns the epoch millis of that wall-clock time in `to`. Example: `from = UTC`, `to = Asia/Tokyo`, value `0` returns `-32400000` (wall clock 1970-01-01T00:00 in Tokyo is 1969-12-31T15:00Z). Callers: `AddAnimeTracks.kt`, `AddMangaTracks.kt`, `AnimeTrackInfoDialog.kt`, `MangaTrackInfoDialog.kt` (date pickers in tracker dialogs, area:27-tracking-core).
- `Long.toLocalDate()` (`:39-41`): `LocalDate.ofInstant(Instant.ofEpochMilli(this), ZoneId.systemDefault())`.
- `Instant.toLocalDate(zoneId = systemDefault)` (`:43-45`).

Proposal: `chrono` with `chrono::Local` for system zone, `chrono::FixedOffset`/`chrono-tz` where the upstream passes an explicit zone. Use `DateTime::<Utc>::from_timestamp_millis`.

### 10.6 Edge cases
- Times in the far past or future use the absolute format (`>= 7` days, or `< -7`).
- Timezone change during session: `now` is read on each call, not cached.
- DST: `LocalDateTime` arithmetic ignores DST; the Rust port uses naive local times for the same computation.

---

## 11. Locale and language names (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt`)

### 11.1 Sentinels (`LocaleHelper.kt:84-85`)
- `PINNED_KEY = "pinned"`, `LAST_USED_KEY = "last_used"` (internal constants, used as source-group keys).
- Special language code `"all"` = multi-language group; `"other"` = other sources group.

### 11.2 `comparator` (lines 17-25)
`{ a, b -> if (a == "all") -1 else if (b == "all") 1 else getLocalizedDisplayName(a).compareTo(getLocalizedDisplayName(b)) }`.
- `String.compareTo` (UTF-16 code unit order, NOT the collator).
- Quirk: when both are `"all"`, returns `-1` (not a valid comparator). Not reachable in practice (keys are unique). Rust: `sort_by` with the same rule, but compare both-`all` as `Equal`.
- Callers: `SourcePreferences.kt`, `ExternalIntents.kt` (area:14-source-local-and-metadata/area:17-source-app-layer source lists).

### 11.3 `getSourceDisplayName(lang: String?, context)` (lines 30-38)
- `last_used` -> `MR.strings.last_used_source` = `Last used`
- `pinned` -> `MR.strings.pinned_sources` = `Pinned`
- `other` -> `MR.strings.other_source` = `Other`
- `all` -> `MR.strings.multi_lang` = `Multi`
- else -> `getLocalizedDisplayName(lang)` (note: `null` input gives `""` via that function, not an error).

### 11.4 `getDisplayName(lang)` (lines 40-48)
1. Normalise: `zh-CN` -> `zh-Hans`, `zh-TW` -> `zh-Hant`, else unchanged.
2. `Locale.forLanguageTag(normalized).displayName` (name in the DEFAULT display locale, typically English in the UI).

### 11.5 `getLocalizedDisplayName(lang: String?)` (lines 55-67)
1. `null` -> `""`.
2. Locale: `""` -> `LocaleListCompat.getAdjustedDefault()[0]` (system language); `zh-CN` -> `zh-Hans`; `zh-TW` -> `zh-Hant`; else `forLanguageTag(lang)`.
3. Return `locale.getDisplayName(locale)` (self name, in its own language, e.g. `Deutsch` for `de`) with the first character uppercased using `replaceFirstChar { it.uppercase(locale) }`.

Proposal (ICU4X `icu_displaynames::LanguageDisplayNames::try_new(locale.into(), options)` with the target locale equal to the language itself; first-char uppercase with `char::to_uppercase`). The Rust port caches results in a `HashMap<String, String>` behind a `Mutex`.

### 11.6 `getDefaultEnabledLanguages()` (lines 72-74)
`setOf("all", "en", Locale.getDefault().language)`. Note: on a system with an empty default language the set contains `""`. Rust: `["all", "en", system_lang]`, dropping an empty `system_lang` (Proposal; the empty string enables nothing and only shows in the UI as an empty name).

System language: `glib::language_names()` first entry (strip `.UTF-8`, `@modifier`, convert `_` to `-`), fallback `en`.

### 11.7 `getSimpleLocaleDisplayName()` (lines 79-81)
`LocaleListCompat.getDefault()[0]!!.displayLanguage` (English-language name of the system language, e.g. `German`). Throws if the list is empty (not handled upstream). Rust: returns the ICU English display name, or `English` on failure.

---

## 12. Rectangle inversion for tap zones (`app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt`)

`RectF.invert(invertMode: ReaderPreferences.TappingInvertMode): RectF` (lines 6-20). Coordinates are normalised to `0..1`.

`TappingInvertMode` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:183-195`):
| Name | label (MR) | horizontal | vertical |
|---|---|---|---|
| `NONE` | `tapping_inverted_none` = `None` | false | false |
| `HORIZONTAL` | `tapping_inverted_horizontal` = `Horizontal` | true | false |
| `VERTICAL` | `tapping_inverted_vertical` = `Vertical` | false | true |
| `BOTH` | `tapping_inverted_both` = `Both` | true | true |

Transform (`RectF(left, top, right, bottom)` constructor order):
- both: `RectF(1 - right, 1 - bottom, 1 - left, 1 - top)` (point reflection through the centre).
- vertical only: `RectF(left, 1 - bottom, right, 1 - top)`.
- horizontal only: `RectF(1 - right, top, 1 - left, bottom)`.
- none: returns `this` (same instance).

Proposal: `pub struct NormRect { pub left: f32, pub top: f32, pub right: f32, pub bottom: f32 }` and `pub fn invert(self, mode: TapInvert) -> NormRect` with the same formulas. Verify with a unit test (edge cases in p1-edge-cases-and-tests.md). The reader (area:43-reader-core) uses this for its tap zones.

---

## 13. Animation and visibility helpers (`View`)

### 13.1 `View?.isVisibleOnScreen()` (`app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt:85-101`)
1. `null` -> false.
2. `!isShown` -> false.
3. `actualPosition = Rect(); getGlobalVisibleRect(actualPosition)`.
4. `screen = Rect(0, 0, widthPixels, heightPixels)` from `Resources.getSystem().displayMetrics`.
5. Return `actualPosition.intersect(screen)`. Note: `Rect.intersect` returns `true` when the rectangles overlap with non-zero area (`left < right && top < bottom` after intersection). Rectangles touching only at an edge are NOT visible.

Used by `ReaderPageImageView.kt` to start or pause page work for visible pages (area:44-reader-viewers).

Proposal: `fn is_visible_on_screen(widget: &gtk::Widget) -> bool { widget.is_mapped() && widget.compute_bounds(&toplevel)` intersects `toplevel.bounds` with positive area }`. Use `widget.is_visible()` plus `compute_bounds`; the `Resources.getSystem()` screen is replaced by the window bounds.

### 13.2 `popupMenu` (`ViewExtensions.kt:65-83`)
`View.popupMenu(menuRes, initMenu, onMenuItemClick)`:
1. `PopupMenu(context, this, Gravity.NO_GRAVITY, R.attr.actionOverflowMenuStyle, 0)`.
2. Inflate the menu resource; if `initMenu` is provided, run it on the menu.
3. `setOnMenuItemClickListener { item.onMenuItemClick(); true }` (always consumes the click).
4. `show()`; returns the popup.

Proposal: `gtk::PopoverMenu` with a `gio::Menu` model; items activate an action; the popover closes after activation (GTK default). Keep the "always consumed" behaviour (no propagation).

### 13.3 `setComposeContent` (`ViewExtensions.kt:26-56`)
Wraps content in `TachiyomiTheme` with `LocalTextStyle = MaterialTheme.typography.bodySmall` and `LocalContentColor = onBackground`. Compose-only. Drop; the GTK equivalent is the widget tree with the user's GTK theme. Loss: none.

### 13.4 Animation durations
See 9.11.

---

## 14. Network state (`NetworkExtensions.kt`, `NetworkStateTracker.kt`)

### 14.1 `isOnline()` (`NetworkExtensions.kt:16-26`)
1. `activeNetwork ?: return false`.
2. `getNetworkCapabilities(activeNetwork) ?: return false`.
3. `maxTransport = TRANSPORT_LOWPAN` on API >= 27 (`O_MR1`), else `TRANSPORT_WIFI_AWARE`.
4. Return `(TRANSPORT_CELLULAR..maxTransport).any(hasTransport)`. Transport constants 0..(LOWPAN=6) range: CELLULAR=0, WIFI=1, BLUETOOTH=2, ETHERNET=3, VPN=4, WIFI_AWARE=5, LOWPAN=6.
   - Note: VPN (4) is inside the range, so a VPN-only network counts as online.

Proposal: `gio::NetworkMonitor::default()`:
- `network_available()` true, and `connectivity()` in {`Limited`, `Portal`, `Full`} (values `LOCAL=1`, `LIMITED=2`, `PORTAL=3`, `FULL=4`) means "online" only for `FULL` (matches the upstream "has internet" intent). `LOCAL` and `LIMITED` are offline.

### 14.2 `isConnectedToWifi()` (`NetworkExtensions.kt:31-44`)
1. `!wifiManager.isWifiEnabled` -> false.
2. API >= 29 (`Q`): active network has `TRANSPORT_WIFI` AND `NET_CAPABILITY_INTERNET`.
3. Older: `wifiManager.connectionInfo.bssid != null`.

Proposal (zbus, NetworkManager): `org.freedesktop.NetworkManager` property `PrimaryConnectionType` (string) equals `802-11-wireless`, OR the active connections include a `802-11-wireless` type. Fallback to `false` when NM is absent.

### 14.3 `activeNetworkState()` and `networkStateFlow()` (`NetworkStateTracker.kt`)
`data class NetworkState(isConnected, isValidated, isWifi)` with `isOnline = isConnected && isValidated`.
- `activeNetworkState()` (`:19-26`):
  - `isConnected = activeNetworkInfo?.isConnected ?: false` (deprecated API)
  - `isValidated = capabilities?.hasCapability(NET_CAPABILITY_VALIDATED) ?: false`
  - `isWifi = wifiManager.isWifiEnabled && capabilities?.hasTransport(TRANSPORT_WIFI) ?: false`. Operator precedence: `isWifiEnabled && (capabilities?.hasTransport(...) ?: false)`, which is what is intended.
- `networkStateFlow()` (`:28-42`): `registerDefaultNetworkCallback`; on `onCapabilitiesChanged` and `onLost` emits `activeNetworkState()`. Emits nothing initially (no seed value). `awaitClose { unregisterNetworkCallback }`.

Consumers (found by grep): `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadJob.kt` and `.../download/anime/AnimeDownloadJob.kt` (areas 23, 24 and 26 own what they do with the state).

Proposal (`kuro-platform::net`):
- `pub struct NetworkState { pub is_connected: bool, pub is_validated: bool, pub is_wifi: bool }` with `is_online()`.
- Source: NetworkManager `StateChanged` / `PropertiesChanged` on `org.freedesktop.NetworkManager` (`State` >= 50 => connected; `Connectivity == 4` => validated; wifi from active connection types) with `gio::NetworkMonitor::network-changed` as a trigger to re-read.
- Emit the seed value immediately (new behaviour, no loss) so the download job does not wait for the first change. Use `tokio::sync::watch`.

NM state constants used: `NM_STATE_CONNECTED_LOCAL = 50`, `NM_STATE_CONNECTED_SITE = 60`, `NM_STATE_CONNECTED_GLOBAL = 70` (connected if >= 50). Connectivity: `UNKNOWN=0`, `NONE=1`, `PORTAL=2`, `LIMITED=3`, `FULL=4`.

---

## 15. Notifications (`NotificationExtensions.kt`)

### 15.1 Posting rules (OBSERVED)
- `Context.notify(id, notification)` (`:29-40`): on API >= 33 (`TIRAMISU`) and when `POST_NOTIFICATIONS` is not granted -> return silently (no error). Else `NotificationManagerCompat.notify(id, notification)`.
- `Context.notify(list of NotificationWithIdAndTag)` (`:42-53`): same permission rule, batch variant.
- `Context.notify(id, channelId, block)` (`:20-27`): builds with `notificationBuilder` then `notify(id, notification)`.
- `Context.cancelNotification(id)` (`:55-57`): `NotificationManagerCompat.cancel(id)` (no tag).
- `notificationBuilder(channelId, block)` (`:66-76`): `NotificationCompat.Builder(this, channelId).setColor(getColor(R.color.accent_blue))`, then the block. The accent colour is `accent_blue` (a resource in the app module, not in scope).

Proposal (`kuro-platform::notify`):
- Post with `gtk::Application::send_notification(Some(tag_id), &gio::Notification)`, where `tag_id` is the existing integer id mapped to a string (`format!("kuro-{id}")`). The batch variant loops.
- Cancel with `withdraw_notification`.
- Permission rule: none needed (the XDG notification portal asks the desktop). If the desktop refuses (`gio` error), log at `warn` and continue (matches the silent drop).
- Accent colour: dropped (no per-notification colour in the portal). Loss: the accent tint in notification banners.

### 15.2 Channel definitions (OBSERVED)
- `buildNotificationChannelGroup(channelId, block)` (`:85-92`) and `buildNotificationChannel(channelId, channelImportance, block)` (`:102-110`) are builders. Importance values are Android `NotificationManagerCompat.IMPORTANCE_*` ints supplied by the caller (area:26-library-update-jobs, area:35-data-misc-updater-notification-export).

Proposal: a channel is a Rust enum `NotifyChannel { Library, Downloads, Updates, Backup, Other }` with a mapped `gio::NotificationPriority` (`Low`, `Normal`, `High`, `Urgent`). Android `IMPORTANCE_MIN=1`/`LOW=2` -> `Low`; `DEFAULT=3` -> `Normal`; `HIGH=4`/`MAX=5` -> `High`. The GNotification `priority` is the only per-notification setting used by KDE; "groups" are dropped (no grouping in the portal). Channel ids are kept as the enum string names for logs.

### 15.3 `notificationManager` (`:17-18`)
`getSystemService()!!` on Android. Drop; no equivalent object is needed (the `gio` application owns the notifications).

---

## 16. Background job registry (`WorkManagerExtensions.kt`)

### 16.1 `isRunning(tag)` (`:14-17`)
`getWorkInfosByTag(tag).get()` (blocking) and `any { state == RUNNING }`. Used by `BackupCreateJob`, `BackupRestoreJob`, `AnimeDownloadJob`, `MangaDownloadJob`, `AnimeDownloadManager`, `SettingsDataScreen` (area:23-downloads-manga, area:24-downloads-anime, area:26-library-update-jobs, area:47-settings-screens-general).

Proposal (`kuro-platform::jobs::JobRegistry`):
- `HashMap<&'static str, JobHandle>` behind `Mutex`, where a `JobHandle` holds a `tokio::task::JoinHandle` and a `CancellationToken`.
- `is_running(tag) -> bool`: true when an entry exists and `!handle.is_finished()`.
- `start(tag, fut)`: refuses to start a second job with the same tag while one runs (returns `false`). Matches WorkManager's `ExistingWorkPolicy.KEEP` used upstream (verify per job in areas 23-26).
- Non-blocking: no `.get()` on the UI thread.

### 16.2 `setForegroundSafely()` (`:28-35`)
OBSERVED: `setForeground(getForegroundInfo())`, then `delay(500)`, catching `IllegalStateException` (logs `Not allowed to set foreground job`).

Drop: Linux has no foreground-service restriction. The `500 ms` delay existed only for Android's foreground-service start. Loss: none. The Rust port shows the download or backup as "busy" in the UI through the registry instead.

---

## 17. Windows: secure screen (`WindowExtensions.kt`)

`Window.setSecureScreen(enabled)`: sets or clears `FLAG_SECURE` (`WindowManager.LayoutParams`). Caller: `SecureActivityDelegate.kt` (area:36-ui-shell-navigation-security-webview), which enables it when `secureScreen == ALWAYS`, or `INCOGNITO` and incognito mode is on.

Drop with user-visible note. No stable GTK 4.16, Wayland or X11 mechanism blocks screenshots or screen recording of a single window. The Rust port keeps the preference (`SecurityPreferences.secureScreen`, owned by area:36-ui-shell-navigation-security-webview) so the setting is not lost, shows a one-line note in the security settings ("Screenshot blocking is not available on Linux"), and does not enforce it.

---

## 18. Tablet and display layout (`DisplayExtensions.kt`)

### 18.1 Constants (lines 12-18)
- `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP = 720` (smallest width at which tablet UI is used when forced to show)
- `TABLET_UI_MIN_SCREEN_WIDTH_PORTRAIT_DP = 700`
- `TABLET_UI_MIN_SCREEN_WIDTH_LANDSCAPE_DP = 600`

### 18.2 `Configuration.isTabletUi()` (`:20-22`)
`smallestScreenWidthDp >= 720`.

### 18.3 `Context.prepareTabletUiContext()` (`:25-48`) — decision table
`expected` (Boolean) from `UiPreferences.tabletUiMode()` (key `tablet_ui_mode`, default `TabletUiMode.AUTOMATIC`):
- `AUTOMATIC`: `smallestScreenWidthDp >= (700 if portrait else 600)`. Orientation: `ORIENTATION_PORTRAIT` -> 700, anything else (landscape or undefined) -> 600.
- `ALWAYS`: `true`.
- `LANDSCAPE`: `orientation == ORIENTATION_LANDSCAPE`.
- `NEVER`: `false`.
If `isTabletUi() != expected`, return a context whose `smallestScreenWidthDp` is raised to 720 when `expected`, or capped at 719 otherwise. Else return `this`.

`TabletUiMode` (`app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt`): `AUTOMATIC` (title `automatic_background` = `Auto`), `ALWAYS` (`lock_always` = `Always`), `LANDSCAPE` (`landscape` = `Landscape`), `NEVER` (`lock_never` = `Never`).

Proposal (`kurobayomi::ui::layout`):
- Read the window width in logical px (`window.width()`), height and orientation (`width > height`).
- `TabletUiMode` maps to: `AUTOMATIC` -> `width >= 720` (ignore portrait/landscape thresholds, GTK windows are resizable, so orientation is not a stable signal; the observed 700/600 values are kept as the threshold for `landscape` and `portrait` respectively, computed from the same numbers, see tasks). `ALWAYS` -> wide layout. `LANDSCAPE` -> wide layout when `width > height`. `NEVER` -> compact layout.
- Output: a `enum Layout { Compact, Wide }` consumed by `kurobayomi::ui::nav` (navigation rail vs bottom bar). The re-evaluation runs on `connect_default_width_notify` (see `src/main.rs:68` in the mockup).

### 18.4 `Context.isNightMode()` (`:53-55`)
`uiMode & UI_MODE_NIGHT_MASK == UI_MODE_NIGHT_YES`. Proposal: `ashpd::desktop::settings::Settings::read("org.freedesktop.appearance", "color-scheme")` value `1` = prefer dark; `0` = no preference; `2` = prefer light. Use the value of the portal cache at start, subscribe to `SettingChanged` for updates. Fallback: `gtk::Settings::gtk_application_prefer_dark_theme()`.

### 18.5 `Activity.hasDisplayCutout()` (`:62-65`) and `Context.isNavigationBarNeedsScrim()` (`:70-73`)
Android-only (API 28 display cutout; `config_navBarNeedsScrim`). Drop. Loss: none on Linux.

---

## 19. Intent and share (`IntentExtensions.kt`)

### 19.1 `Uri.toShareIntent(context, type = "image/*", message = null)` (`:13-34`)
- `ACTION_SEND`; for `http`/`https`: `EXTRA_TEXT = uri.toString()`; for `content`: optional `EXTRA_TEXT = message` and `EXTRA_STREAM = uri`.
- `clipData = ClipData.newRawUri(null, uri)`; `type` (MIME); flag `FLAG_GRANT_READ_URI_PERMISSION`.
- Chooser titled `MR.strings.action_share` = `Share`, flags `FLAG_ACTIVITY_NEW_TASK`.
- Callers: 11 files (found by `grep -rl toShareIntent` in the Aniyomi tree), including `CrashLogUtil.kt` (MIME `text/plain`).

Proposal (Linux): the share action becomes one of two operations, chosen per caller:
- "Save as" (cover, backup, crash logs, exported lists): `gtk::FileDialog::save` (portal `org.freedesktop.portal.FileChooser`). Writes the file on success.
- "Open or reveal" (for a local file): `gtk::FileLauncher::new(file).launch(...)` or `gtk::FileLauncher::open_containing_folder`.
- For `http(s)` URIs (link sharing): copy the URL to the clipboard with the copy toast (9.6). There is no portable Linux share sheet in GTK 4.16 (Android intent chooser has no equivalent). Loss: no third-party share targets on the desktop; the user copies the link instead.

### 19.2 `Intent.getParcelableExtraCompat` and `getSerializableExtraCompat` (`:36-47`)
Android Intent extras. Drop (no Intents on Linux). Loss: none.

---

## 20. Drawable to bitmap (`DrawableExtensions.kt`)

`Drawable.getBitmapOrNull()`: `BitmapDrawable` -> `bitmap`; Coil `ScaleDrawable` -> `child.toBitmap()`; else `null`. Callers: `AnimeLibraryUpdateNotifier.kt`, `MangaLibraryUpdateNotifier.kt` (notification large icon), `AnimeImageScreenModel.kt`, `MangaCoverScreenModel.kt` (save cover), `SaveImageNotifier.kt`, `DrawableExtensions.kt` (area:34-images-covers-cache, area:26-library-update-jobs, area:43-reader-core).

Proposal: the Rust decoded page or cover is already a `gdk::Texture` or `gdk_pixbuf::Pixbuf`. `fn texture_or_none(img: &CoverImage) -> Option<gdk::Texture>` returns `None` for placeholder images. Notification large icon: `gio::BytesIcon` from PNG bytes (the portal accepts PNG; 256x256 max is applied by the caller).

---

## 21. Closeable array (`CloseableExtensions.kt`)

`inline fun <T : Closeable?> Array<T>.use(block: () -> Unit)` (lines 12-31):
- Runs `block`. If it throws: records the exception, closes every element with each close exception added as suppressed to the block exception, rethrows the block exception.
- If `block` succeeds: closes every element in order; a close exception propagates immediately and the remaining elements are NOT closed.
- Return type is `Unit` (the KDoc says "result of block", which is wrong).

Drop: RAII (`Drop`) closes every handle. Observed edge that the Rust port fixes: a failing close during the success path does not stop the other closes (it is collected and returned after all closes run). No user-visible change.

---

## 22. Reactive bridge: RxJava to Flow (`RxJavaExtensions.kt`)

`Observable<T>.asFlow()`: `callbackFlow` subscribes; `onNext` -> `trySend(t)`, `onError` -> `close(e)`, `onCompleted` -> `close()`; `awaitClose { subscription.unsubscribe() }`.

Drop: RxJava is not used in Rust. Async producers use `tokio::sync::mpsc` or `watch`, and cancellation uses `CancellationToken`. The only internal consumers are in download managers (area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared); those are rewritten there.

---

## 23. Source and extension classloading (`ChildFirstPathClassLoader.kt`)

Parent-last loading for extension APKs (`PathClassLoader` subclass). Resolution order (OBSERVED, lines 23-45): `findLoadedClass` -> system class loader -> own dex (`findClass`) -> parent (`super.loadClass`). Resource lookup (lines 47-77): system loader, then local, then parent (concatenated enumeration for `getResources`).

Drop: Linux extensions are not DEX. The extension design is in area:20-extension-strategy-redesign. The Rust port has no class loader. Loss: none for the user; the decision is recorded so area:20-extension-strategy-redesign does not reintroduce parent-last semantics by accident.

---

## 24. Authenticator (app lock)

File: `app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt` (136 lines).

### 24.1 Behaviour (OBSERVED)
- `isAuthenticating` (global `var`, default `false`): set `true` before launching the prompt (`startAuthentication`, line 40), set `false` in `AuthenticationCallback.onAuthenticationError` and `onAuthenticationSucceeded`. Used by `SecureActivityDelegate` (area:36-ui-shell-navigation-security-webview) to avoid a second unlock when the prompt itself pauses the app.
- `startAuthentication(title, subtitle = null, confirmationRequired = true, callback)`: `startClass2BiometricOrCredentialAuthentication(title, subtitle, confirmationRequired, executor = mainExecutor, callback)`.
- `authenticate(title, subtitle = stringResource(MR.strings.confirm_lock_change))` (suspend, lines 50-82):
  - If `!isAuthenticationSupported()` -> resume `true` (no lock configured means "allowed").
  - On success -> `true`. On error -> `activity?.toast(errString.toString())` (raw error text) then `false`. Cancellation is supported (`suspendCancellableCoroutine`).
- `isAuthenticationSupported()` (lines 87-90): `BiometricManager.canAuthenticate(BIOMETRIC_WEAK or DEVICE_CREDENTIAL) == BIOMETRIC_SUCCESS`.
- Strings: `MR.strings.confirm_lock_change` = `Authenticate to confirm change`.

### 24.2 Proposal (Linux)
- Authentication = the user's own password through PAM: `pam` crate, service name `kurobayomi` shipped as `/etc/pam.d/kurobayomi` (copy of `login`, without `pam_nologin`), `pam_authenticate` for the current user name (`users` crate / `glib::user_name()`).
- Optional fingerprint: `fprintd` over D-Bus (`net.reactivated.Fprint`) if available; never required.
- `isAuthenticationSupported()` = PAM service loads and the current user has a password entry (`/etc/shadow` not readable -> treat as supported and rely on PAM result).
- Failures: PAM error text is shown (`AuthenticationError` text), matching the upstream "raw error" behaviour.
- Tradeoff (to surface to the user): PAM means the app must read the user's authentication stack; a lock that is weaker than the desktop lock would be a security regression. The Rust port defaults the lock to off, as upstream does (`SecurityPreferences.useAuthenticator` default false, area:36-ui-shell-navigation-security-webview).
- Prompt UI: a modal `gtk::Window` with a `gtk::PasswordEntry` (no password is stored by the app).

---

## 25. Miscellaneous observed details

- `Anime.nextEpisodeAiringAt` and `Manga`/`Anime` `*LastModified` are epoch millis or seconds: `coverLastModified` and `backgroundLastModified` are ms (`Instant.now().toEpochMilli()`); `nextEpisodeAiringAt` is seconds (from AniList `airingAt`, Simkl parse `epochSecond`).
- `removeDuplicates` is used with `Double` chapter numbers (see 5.7).
- All Kotlin `String.format` calls use the default locale; the Rust port uses a fixed `.` decimal (9.2).
- `kotlin.time`/`System.currentTimeMillis` clock usage: all "now" values are read once per call.
- Dispatchers: `withIOContext` (`core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:60`) for network and file work; `withUIContext` (`:55`) for toasts; `withNonCancellableContext` (`:65`) for crash dumps. Rust mapping: `tokio::task::spawn_blocking` for file IO, `glib::MainContext` for toasts, a detached `tokio::spawn` for crash dumps (not cancelled with the window).
