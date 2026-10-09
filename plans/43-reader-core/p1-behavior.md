# 43-reader-core: behavior specification

Scope: the 32 files listed in `p1-inventory.md`. All paths are repo-relative to `/home/blackbox/aniyomi-src`.
Short prefixes used below (always expand to the full path when copying):

- `R/` = `app/src/main/java/eu/kanade/tachiyomi/ui/reader/`
- `RL/` = `R/loader/`, `RM/` = `R/model/`, `RS/` = `R/setting/`
- `P/` = `app/src/main/java/eu/kanade/presentation/reader/`

Labels: **OBSERVED** = what the Kotlin does (with `path:line`). **Proposal:** = what the Rust port should do.
Line numbers refer to the files as read in this session.

---

## 1. Chapter model and state machine

### 1.1 `ReaderChapter` (`RM/ReaderChapter.kt`)

OBSERVED:
- `data class ReaderChapter(val chapter: Chapter)` (`RM/ReaderChapter.kt:9`). `chapter` is the database model (`eu.kanade.tachiyomi.data.database.models.manga.Chapter`, an interface implemented by `ChapterImpl`).
- Equality of `ReaderChapter` is the data-class equality of `chapter`, i.e. `ChapterImpl.equals` (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/ChapterImpl.kt:35-40`): same runtime class AND equal `url` AND equal `id`. `hashCode = url.hashCode() + id.hashCode()` (`ChapterImpl.kt:44-46`). `state`, `pageLoader`, `requestedPage` are NOT part of equality.
- `stateFlow: MutableStateFlow<State>` initialised to `State.Wait` (`RM/ReaderChapter.kt:11`). `state` is a property over `stateFlow.value` (`:12-15`).
- `pages: List<ReaderPage>?` = `(state as? State.Loaded)?.pages` (`:18-19`).
- `pageLoader: PageLoader?` = null (`:21`). `requestedPage: Int` = 0 (`:23`) — the page the viewer should scroll to when the chapter is shown.
- `private var references = 0` (`:25`).
- Secondary constructor from the domain model: `ReaderChapter(chapter: tachiyomi.domain.items.chapter.model.Chapter) : this(chapter.toDbChapter())` (`:27`).
- `ref()`: `references++` (`:29-31`).
- `unref()` (`:33-43`): `references--`; if `references == 0`: log "Recycling chapter ..." when a loader exists, call `pageLoader?.recycle()`, set `pageLoader = null`, set `state = State.Wait`. Note: the check is `== 0`, not `<= 0`. A negative count never triggers recycling again.
- `sealed interface State` (`:45-50`): `Wait`, `Loading` (data objects), `Error(val error: Throwable)`, `Loaded(val pages: List<ReaderPage>)` (data classes).

State transitions (derived from `RL/ChapterLoader.kt` and `R/ReaderViewModel.kt`):

```
Wait --loadChapter--> Loading --getPages ok, non-empty--> Loaded(pages)
                              \--any Throwable-------> Error(e)  (then the exception is rethrown)
Loaded / Error --unref to 0--> Wait (pageLoader recycled and nulled)
Error --loadChapter / preload--> Loading (retry is allowed; Loaded short-circuits)
Loaded --loadChapter--> no-op (chapterIsReady)
Wait with a DOWNLOADED chapter, previously loaded online --preload--> set to Wait so the next load uses the download loader
```

OBSERVED quirk: `ChapterLoader.loadChapter` catches `Throwable`, including `CancellationException`, and writes `State.Error(e)` before rethrowing (`RL/ChapterLoader.kt:64-67`). A cancelled load therefore leaves the chapter in `Error`, not `Wait`. A retry is still possible because `Error` is retryable.

OBSERVED quirk: on a retry after `getPages()` threw, `chapter.pageLoader` was already assigned (`RL/ChapterLoader.kt:48-49`), and the new loader overwrites it without recycling the old one. The old loader's cache write and its coroutine scope are leaked (`HttpPageLoader` starts a worker in `init`, `RL/HttpPageLoader.kt:54-64`).

OBSERVED quirk: `ReaderViewModel.preload` sets a downloaded chapter to `Wait` when `pageLoader.isLocal == false` (`R/ReaderViewModel.kt:392-405`) but does not recycle the old `HttpPageLoader` first. The old loader is orphaned in the same way.

**Proposal:** model `ChapterState` as a Rust enum `Wait | Loading | Error(Arc<LoadError>) | Loaded(Vec<PageRef>)`. Attach the loader as `Option<Box<dyn PageLoader>>` and enforce "recycle before replace" in one `set_loader` function. This removes both leaks.

### 1.2 `ReaderPage` (`RM/ReaderPage.kt`)

OBSERVED:
- `open class ReaderPage(index: Int, url: String = "", imageUrl: String? = null, var stream: (() -> InputStream)? = null) : Page(index, url, imageUrl, null)` (`RM/ReaderPage.kt:6-11`).
- `open lateinit var chapter: ReaderChapter` (`:13`). Must be set before use. `ChapterLoader` sets it with `pages.onEach { it.chapter = chapter }` (`RL/ChapterLoader.kt:52`).
- `Page` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt`):
  - `val index: Int`, `val url: String = ""`, `var imageUrl: String?`, `@Transient var uri: Uri?` (deprecated).
  - `val number: Int get() = index + 1` (`Page.kt:18-19`). The UI uses 1-based `number`.
  - `status: Page.State` backed by `MutableStateFlow(State.QUEUE)` (`Page.kt:21-30`). `progress: Int` backed by a `MutableStateFlow(0)` (`Page.kt:32-41`).
  - `update(bytesRead, contentLength, done)`: `progress = 100 * bytesRead / contentLength` when `contentLength > 0`, else `-1` (`Page.kt:43-49`).
  - `enum class State { QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE, READY, ERROR }` (`Page.kt:51-57`). Initial value `QUEUE`.

### 1.3 `InsertPage` (`RM/InsertPage.kt`)

OBSERVED: `class InsertPage(val parent: ReaderPage) : ReaderPage(parent.index, parent.url, parent.imageUrl)`. `chapter` = `parent.chapter`. `init` sets `status = READY` and `stream = parent.stream` (`RM/InsertPage.kt:3-10`). Viewers use it as a filler page (for example a split spread). `ReaderViewModel.onPageSelected` returns immediately for `InsertPage` and does not persist progress (`R/ReaderViewModel.kt:437-439`).

### 1.4 `ViewerChapters` (`RM/ViewerChapters.kt`)

OBSERVED: `data class ViewerChapters(currChapter: ReaderChapter, prevChapter: ReaderChapter?, nextChapter: ReaderChapter?)`. `ref()` and `unref()` apply to `currChapter` and to each non-null neighbour (`RM/ViewerChapters.kt:3-19`).

### 1.5 `ChapterTransition` (`RM/ChapterTransition.kt`)

OBSERVED:
- `sealed class ChapterTransition { abstract val from: ReaderChapter; abstract val to: ReaderChapter? }` (`:3-6`).
- `class Prev(from, to)` and `class Next(from, to)` (`:8-16`).
- `equals` (`:18-24`): true if same instance; or `from == other.from && to == other.to`; or `from == other.to && to == other.from`. The subclass is NOT compared, so `Prev(a, b) == Next(a, b)`.
- `hashCode` (`:26-30`): `from.hashCode()*31 + (to?.hashCode() ?: 0)`. This is not symmetric, so the swapped pair `(b, a)` is `equals` to `(a, b)` but has a different hash. The contract is violated.
- `toString` = `"${javaClass.simpleName}(from=${from.chapter.url}, to=${to?.chapter?.url})"` (`:32-34`).

**Proposal:** in Rust, make `Transition` a struct with an explicit `direction` field and derive `PartialEq` over `(direction, from_id, to_id)`. Do not copy the swapped-equality rule unless a caller depends on it (none in this scope observed).

### 1.6 `ReaderOrientation` (`RS/ReaderOrientation.kt`)

OBSERVED (full table, `RS/ReaderOrientation.kt:14-61`):

| Name | `flag` (Android `ActivityInfo` constant) | `stringRes` | `flagValue` (stored) |
|---|---|---|---|
| DEFAULT | `SCREEN_ORIENTATION_UNSPECIFIED` | `label_default` | `0x00000000` |
| FREE | `SCREEN_ORIENTATION_UNSPECIFIED` | `rotation_free` | `0x00000008` |
| PORTRAIT | `SCREEN_ORIENTATION_SENSOR_PORTRAIT` | `rotation_portrait` | `0x00000010` |
| LANDSCAPE | `SCREEN_ORIENTATION_SENSOR_LANDSCAPE` | `rotation_landscape` | `0x00000018` |
| LOCKED_PORTRAIT | `SCREEN_ORIENTATION_PORTRAIT` | `rotation_force_portrait` | `0x00000020` |
| LOCKED_LANDSCAPE | `SCREEN_ORIENTATION_LANDSCAPE` | `rotation_force_landscape` | `0x00000028` |
| REVERSE_PORTRAIT | `SCREEN_ORIENTATION_REVERSE_PORTRAIT` | `rotation_reverse_portrait` | `0x00000030` |

- `companion object { const val MASK = 0x00000038; fun fromPreference(preference: Int?): ReaderOrientation = entries.find { it.flagValue == preference } ?: DEFAULT }` (`:64-68`).
- Icons are Compose vectors and are not part of the behavior: DEFAULT and FREE use `Icons.Default.ScreenRotation`; PORTRAIT and REVERSE_PORTRAIT use `StayCurrentPortrait`; LANDSCAPE uses `StayCurrentLandscape`; LOCKED_PORTRAIT uses `ScreenLockPortrait`; LOCKED_LANDSCAPE uses `ScreenLockLandscape` (`RS/ReaderOrientation.kt:20-61`).
- `fromPreference(null)` and any unknown value return `DEFAULT`. Note that `fromPreference` compares the full stored value against `flagValue`, not the masked value. The caller must already have masked the value (`Manga.readerOrientation` does this, see 9.2).

### 1.7 `ReadingMode` and `ReaderPreferences` used here (cross-area, owned elsewhere)

The `ReadingMode` enum (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`) is not in this scope but is used by every function in this scope:

- `DEFAULT` = `0x00000000`, `LEFT_TO_RIGHT` = `0x00000001`, `RIGHT_TO_LEFT` = `0x00000002`, `VERTICAL` = `0x00000003`, `WEBTOON` = `0x00000004`, `CONTINUOUS_VERTICAL` = `0x00000005`. `MASK = 0x00000007`. `fromPreference` falls back to `DEFAULT`. `toViewer(...)` throws `IllegalStateException("Preference value must be resolved: ...")` for `DEFAULT` (it must be resolved first).
- The flag occupies the low 3 bits of `manga.viewerFlags`. `OrientationFlags` occupies bits `0x38`. Both are written through `SetMangaViewerFlags` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt:8-30`): `newFlags = (old and mask.inv()) or (flag and mask)`.

`ReaderPreferences` is at `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt` (owned by area:02-preferences-system). Every key and default used by this scope is listed in `p1-data-and-config.md` section 3.

---

## 2. Loader selection (`RL/ChapterLoader.kt`)

### 2.1 `loadChapter(chapter)` (`:39-69`)

OBSERVED step order:
1. If `chapterIsReady(chapter)` (state is `Loaded` AND `pageLoader != null`), return immediately (`:40-42`, `:74-76`).
2. `chapter.state = Loading` (`:44`).
3. Inside `withIOContext` (`:45`):
   1. `val loader = getPageLoader(chapter)`; `chapter.pageLoader = loader` (`:48-49`).
   2. `val pages = loader.getPages().onEach { it.chapter = chapter }` (`:51-52`).
   3. If `pages.isEmpty()` throw `Exception(stringResource(MR.strings.page_list_empty_error))` (`:53-55`). Message: "No pages found".
   4. Start page: `if (!chapter.chapter.read || readerPreferences.preserveReadingPosition().get()) chapter.requestedPage = chapter.chapter.last_page_read` (`:59-61`). When the chapter is already read and "preserve reading position" is false, `requestedPage` keeps its previous value (default 0), so a read chapter opens at page 0.
   5. `chapter.state = Loaded(pages)` (`:63`).
4. `catch (e: Throwable)`: `chapter.state = Error(e)`; rethrow (`:64-67`).

### 2.2 `getPageLoader(chapter)` (`:81-111`)

OBSERVED, evaluated top to bottom, first match wins:
1. `isDownloaded = downloadManager.isChapterDownloaded(dbChapter.name, dbChapter.scanlator, manga.title, manga.source, skipCache = true)` (`:83-89`). `skipCache = true` forces a filesystem check. If true, use `DownloadPageLoader(chapter, manga, source, downloadManager, downloadProvider)` (`:90-97`).
2. `source is LocalMangaSource` (`:98`): `source.getFormat(chapter.chapter)` then:
   - `Format.Directory` -> `DirectoryPageLoader(format.file)` (`:100`).
   - `Format.Archive` -> `ArchivePageLoader(format.file.archiveReader(context))` (`:101`).
   - `Format.Epub` -> `EpubPageLoader(format.file.epubReader(context))` (`:102`).
3. `source is HttpSource` -> `HttpPageLoader(chapter, source)` (`:105`).
4. `source is StubMangaSource` -> `error(stringResource(MR.strings.source_not_installed, source.toString()))` (`:106-108`). Message key `source_not_installed` = "Source not installed: %1$s".
5. else -> `error(stringResource(MR.strings.loader_not_implemented_error))` (`:109`). Message "Source not found".

All `error(...)` calls are thrown inside the `try` of `loadChapter`, so they become `State.Error`.

OBSERVED: step 1 runs before the source-type check. A local manga chapter that is somehow marked downloaded would use `DownloadPageLoader`. In practice local sources are not downloaded.

**Proposal:** Rust `enum ChapterSource { Download(PathBuf), LocalDirectory(PathBuf), LocalArchive(PathBuf), LocalEpub(PathBuf), Http(Arc<dyn Source>), NotInstalled(String), Unsupported }` and a pure function `choose_loader(&ChapterSource, downloaded: bool)`. Keep the same precedence.

---

## 3. Page loaders

### 3.1 `PageLoader` base (`RL/PageLoader.kt`)

OBSERVED:
- `abstract class PageLoader` (`:10`).
- `var isRecycled = false; private set` (`:15-16`).
- `abstract var isLocal: Boolean` (`:18`).
- `abstract suspend fun getPages(): List<ReaderPage>` (`:23`).
- `open suspend fun loadPage(page: ReaderPage) {}` (`:30`). Contract text: "Progress of the page loading should be followed via [page.statusFlow]. [loadPage] is not currently guaranteed to complete, so it should be launched asynchronously."
- `open fun retryPage(page: ReaderPage) {}` (`:36`). Used only for online sources.
- `@CallSuper open fun recycle() { isRecycled = true }` (`:42-45`).

### 3.2 `DirectoryPageLoader` (`RL/DirectoryPageLoader.kt`)

OBSERVED (`:16-28`), `getPages()`:
1. `file.listFiles()` (UniFile, one directory level only, no recursion).
2. `filter { !it.isDirectory && ImageUtil.isImage(it.name) { it.openInputStream() } }`. A file is accepted if its extension matches the image list (section 3.8) or, when the extension does not match, if the stream's magic bytes are recognised.
3. `sortedWith { f1, f2 -> f1.name.orEmpty().compareToCaseInsensitiveNaturalOrder(f2.name.orEmpty()) }`.
4. `mapIndexed { i, file -> ReaderPage(i) { stream = { file.openInputStream() }; status = READY } }`. The index `i` is the position after sorting, so `index` is contiguous from 0.
5. `?: .orEmpty()` when `listFiles()` returns null (directory missing or unreadable). The result is an empty list, which `ChapterLoader` turns into "No pages found".

OBSERVED: `loadPage` and `recycle` are not overridden (no-ops beyond the base). `isLocal = true`.

### 3.3 `ArchivePageLoader` (`RL/ArchivePageLoader.kt`)

OBSERVED (`:12-35`):
- Constructed from `ArchiveReader` (`mihon.core.archive.ArchiveReader`, `core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt`). The reader memory-maps the file and creates a fresh `ArchiveInputStream` per call (`ArchiveReader.kt:10-16`, `:18-22`).
- `getPages()` (`:15-26`): `reader.useEntries { entries -> entries.filter { it.isFile && ImageUtil.isImage(it.name) { reader.getInputStream(it.name)!! } }.sortedWith { f1, f2 -> f1.name.compareToCaseInsensitiveNaturalOrder(f2.name) }.mapIndexed { i, entry -> ReaderPage(i).apply { stream = { reader.getInputStream(entry.name)!! }; status = READY } }.toList() }`.
- The `stream` lambda re-scans the archive from the start for each page (`ArchiveReader.getInputStream` iterates entries until the name matches, `ArchiveReader.kt:18-22`). This is O(n) per page, O(n^2) over the chapter. Noted for the Rust port.
- `loadPage(page)` = `check(!isRecycled)` (`:28-30`). Throws `IllegalStateException` after recycle.
- `recycle()` = `super.recycle(); reader.close()` (`:32-35`).
- `isLocal = true`.

### 3.4 `EpubPageLoader` (`RL/EpubPageLoader.kt`)

OBSERVED (`:10-31`):
- Constructed from `EpubReader` (`core/archive/src/main/kotlin/mihon/core/archive/EpubReader.kt`). `EpubReader.getImagesFromPages()` (`EpubReader.kt:29-34`) resolves `META-INF/container.xml` (`:39-41`) to the package document, reads the page list from it (`getPagesFromDocument`), and for each page parses the XHTML with Jsoup (`:76-78`) and resolves image paths against that page's directory (`:79-80`). The exact selector list used to extract image elements was not re-read in this session (the function continues past line 80) and is owned by area:13-core-util-storage-archive-torrent.
- `getPages()`: `mapIndexed { i, path -> ReaderPage(i) { stream = { reader.getInputStream(path)!! }; status = READY } }`. No sorting is applied (order is the spine order).
- `loadPage` = `check(!isRecycled)`; `recycle` = base + `reader.close()`. `isLocal = true`.

### 3.5 `DownloadPageLoader` (`RL/DownloadPageLoader.kt`)

OBSERVED (`:20-77`):
- `context: Application by injectLazy()` (`:28`).
- `archivePageLoader: ArchivePageLoader? = null` (`:30`).
- `getPages()` (`:34-47`): `chapterPath = downloadProvider.findChapterDir(name, scanlator, manga.title, source)`. If `chapterPath?.isFile == true` -> `getPagesFromArchive(chapterPath)` (a `.cbz` download, see the `.cbz` check at `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadCache.kt:380`), else `getPagesFromDirectory()`.
- `getPagesFromArchive(file)` (`:54-57`): `ArchivePageLoader(file.archiveReader(context)).also { archivePageLoader = it }.getPages()`.
- `getPagesFromDirectory()` (`:59-72`): `downloadManager.buildPageList(source, manga, chapter.chapter.toDomainChapter()!!)`, then each `page` becomes `ReaderPage(page.index, page.url, page.imageUrl) { context.contentResolver.openInputStream(page.uri ?: Uri.EMPTY)!! }` with `status = READY`. The stream comes from the page's content URI.
- `loadPage(page)` = `archivePageLoader?.loadPage(page)` (`:74-76`). Directory pages are already READY and never queued.
- `recycle()` = base + `archivePageLoader?.recycle()` (`:49-52`).
- `isLocal = true`.

### 3.6 `HttpPageLoader` (`RL/HttpPageLoader.kt`): the queue

This is the only loader with network I/O and the only one with a scheduler. Full algorithm:

**Construction** (`:32-41`, `:46-52`, `:54-64`):
- `scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)`.
- `queue = PriorityBlockingQueue<PriorityPage>()`.
- `preloadSize = 4` (`:48`).
- `dataSaver = DataSaver(source, sourcePreferences)` (`:51`, from `app/src/main/java/aniyomi/util/DataSaver.kt`).
- `init` launches ONE consumer on IO: `flow { while (true) emit(runInterruptible { queue.take() }.page) }.filter { it.status == Page.State.QUEUE }.collect(::internalLoadPage)` (`:55-63`).
- The consumer processes one page at a time. Concurrency for image downloads from one chapter is exactly 1.
- The consumer starts in the constructor, so a loader that is never recycled keeps a thread parked in `queue.take()`.

**Priority and ordering** (`PriorityPage`, `:214-228`):
- `compareTo`: `p = other.priority.compareTo(priority)`; if `p != 0` return `p`; else `identifier.compareTo(other.identifier)`.
- Result: higher `priority` value dequeues first. Equal priority is FIFO by a process-wide `AtomicInteger` id.
- Priority values used: `0` = preload (lowest), `1` = the page the viewer is on, `2` = manual retry (highest).

**`getPages()`** (`:72-85`):
1. Try `chapterCache.getPageListFromCache(chapter.chapter.toDomainChapter()!!)`.
2. On any non-cancellation `Throwable`, fall back to `source.getPageList(chapter.chapter)` (network).
3. Map every result to `ReaderPage(index, page.url, page.imageUrl)` using the loader's own index (`:81-84`). Comment: "Don't trust sources and use our own indexing".
4. The network result is NOT written to the cache here. It is written in `recycle()` (see below).

**`loadPage(page)`** (`:90-123`), run `withIOContext`:
1. If `status == READY`, `imageUrl != null`, and `!chapterCache.isImageInCache(imageUrl)`: set `status = QUEUE` (the image was evicted from the disk cache, re-download it) (`:94-101`).
2. If `status == ERROR`: set `status = QUEUE` (automatic retry while the page is visible) (`:104-106`).
3. `queuedPages = mutableListOf<PriorityPage>()`.
4. If `status == QUEUE`: `queuedPages += PriorityPage(page, 1).also { queue.offer(it) }` (`:109-111`).
5. `queuedPages += preloadNextPages(page, preloadSize)` (`:112`).
6. `suspendCancellableCoroutine<Nothing> { continuation -> continuation.invokeOnCancellation { queuedPages.forEach { if (it.page.status == QUEUE) queue.remove(it) } } }` (`:114-122`). The function never returns normally. The caller must cancel it. On cancellation, every still-QUEUE entry this call enqueued is removed from the queue.

**`retryPage(page)`** (`:128-133`): if `status == ERROR`, set `QUEUE`. Then `queue.offer(PriorityPage(page, 2))` regardless of the status. At dequeue, the consumer's `filter { status == QUEUE }` drops the entry if the page is not QUEUE.

**`preloadNextPages(currentPage, amount)`** (`:164-178`):
1. `pageIndex = currentPage.index`; `pages = currentPage.chapter.pages ?: return emptyList()`.
2. If `pageIndex == pages.lastIndex` return empty.
3. `pages.subList(pageIndex + 1, min(pageIndex + 1 + amount, pages.size))`, then for each page whose `status == QUEUE`, offer `PriorityPage(it, 0)` and include it in the returned list. Other statuses are skipped.
4. Only forward pages in the SAME chapter are preloaded. Pages before the current one are never preloaded. Cross-chapter preloading is done by the view model (section 6.4).

**`internalLoadPage(page)`** (`:186-208`):
1. If `page.imageUrl.isNullOrEmpty()`: `status = LOAD_PAGE`; `page.imageUrl = source.getImageUrl(page)` (network, source-specific).
2. `imageUrl = page.imageUrl!!`.
3. If `!chapterCache.isImageInCache(imageUrl)`: `status = DOWNLOAD_IMAGE`; `imageResponse = source.getImage(page, dataSaver)`; `chapterCache.putImageToCache(imageUrl, imageResponse)`. The cache key is the ORIGINAL `imageUrl`, not the data-saver URL (the `getImage` extension rewrites `page.imageUrl` temporarily, see `app/src/main/java/aniyomi/util/DataSaver.kt:30-41`).
4. `page.stream = { chapterCache.getImageFile(imageUrl).inputStream() }`; `status = READY`.
5. `catch (e: Throwable)`: `status = ERROR`; if `e is CancellationException` rethrow. Other errors are NOT propagated; the consumer continues with the next queued page.

**`recycle()`** (`:135-157`):
1. `super.recycle()` (sets `isRecycled`).
2. `scope.cancel()` (stops the consumer).
3. `queue.clear()`.
4. If `chapter.pages != null`: on IO (`launchIO`, which is `GlobalScope`-based, `core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:32-33`), write `pages.map { Page(it.index, it.url, it.imageUrl) }` to the page-list cache under `chapter.chapter.toDomainChapter()!!`. Errors other than cancellation are swallowed. This is the only point where a freshly fetched page list is persisted.

**`isLocal = false`** (`:66`).

### 3.7 Image download pipeline summary (for the Rust port)

```
loadPage(p) [viewer, visible]
  -> p READY & evicted?      -> QUEUE
  -> p ERROR?                -> QUEUE
  -> p QUEUE?                -> enqueue(p, prio 1)
  -> enqueue forward QUEUE pages (<= 4, prio 0)
  -> wait until cancelled; on cancel remove still-QUEUE entries

worker (single, forever):
  take highest priority, FIFO on ties
  skip if p.status != QUEUE
  LOAD_PAGE -> getImageUrl if needed
  DOWNLOAD_IMAGE -> getImage (data saver rewrite) -> cache put (key = original URL)
  READY (stream = cache file) | ERROR (swallow non-cancel)
```

### 3.8 Image file detection (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt`)

OBSERVED:
- `isImage(name, openStream)` (`:39-44`): `extension = name.substringAfterLast('.')`. Accepted if `ImageType.entries.any { it.extension == extension }` OR `openStream` yields a recognised image type (magic bytes).
- `enum class ImageType(val mime: String, val extension: String)` (`:106-114`): `AVIF("image/avif","avif")`, `GIF("image/gif","gif")`, `HEIF("image/heif","heif")`, `JPEG("image/jpeg","jpg")`, `JXL("image/jxl","jxl")`, `PNG("image/png","png")`, `WEBP("image/webp","webp")`.
- Extension comparison is case-sensitive and uses the text after the LAST dot. `PAGE.JPG` is not matched by the extension rule and must be recognised by magic bytes. `.jpeg` is NOT in the list (only `jpg`), so `.jpeg` files rely on the magic-byte check.
- Magic-byte detection maps the `tachiyomi.decoder` formats to these types (`findImageType`, `:50-63`).

**Proposal:** match the list above exactly, extension check first (case-insensitive is an improvement and must be stated as a deliberate change), then magic bytes via the `image` crate's `ImageReader::with_guessed_format`, plus HEIF and JXL detection done by byte signature.

### 3.9 Natural and case-insensitive sort

OBSERVED: `String.compareToCaseInsensitiveNaturalOrder` (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:37-40`) delegates to `net.greypanther.natsort.CaseInsensitiveSimpleNaturalComparator` (import at `:4`). This is a natural-order comparator (digit runs compare numerically), case-insensitive, applied to the file NAME only (not the path).

**Proposal:** `natord::compare_ignore_case` (crate `natord`). Ties (equal under the comparator) keep stable input order in both implementations; use a stable sort.

---

## 4. `ReaderViewModel`: construction, state, and the restore keys

File: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt`.

### 4.1 Dependencies (`:87-106`)

Injected with Injekt defaults: `SavedStateHandle`, `MangaSourceManager`, `MangaDownloadManager`, `MangaDownloadProvider`, `ImageSaver`, `ReaderPreferences`, `BasePreferences`, `DownloadPreferences`, `TrackPreferences`, `TrackChapter`, `GetManga`, `GetChaptersByMangaId`, `GetNextChapters`, `UpsertMangaHistory`, `UpdateChapter`, `SetMangaViewerFlags`, `GetMangaIncognitoState`, `LibraryPreferences`.

### 4.2 Flows

- `mutableState = MutableStateFlow(State())`, `state = mutableState.asStateFlow()` (`:108-109`).
- `eventChannel = Channel<Event>()` (`:111`): **RENDEZVOUS** channel (capacity 0). `eventFlow = eventChannel.receiveAsFlow()` (`:112`).
- OBSERVED consequence: `trySend` succeeds only when a receiver is suspended at that moment. Events sent with `trySend` while the activity's collector is busy are DROPPED silently. Affected call sites: `onPageSelected` -> `PageChanged` (`:459`), `preload` -> `ReloadViewerChapters` (`:421`). `send` (suspending) is used in `setMangaReadingMode` (`:700`) and `setMangaOrientationType` (`:736-737`).

### 4.3 `State` (`:956-977`, `@Immutable`)

| Field | Type | Default |
|---|---|---|
| `manga` | `Manga?` | null |
| `viewerChapters` | `ViewerChapters?` | null |
| `bookmarked` | Boolean | false |
| `isLoadingAdjacentChapter` | Boolean | false |
| `currentPage` | Int | -1 |
| `viewer` | `Viewer?` | null |
| `dialog` | `Dialog?` | null |
| `menuVisible` | Boolean | false |
| `brightnessOverlayValue` | Int, annotated `@IntRange(-100..100)` | 0 |

Derived: `currentChapter = viewerChapters?.currChapter`; `totalPages = currentChapter?.pages?.size ?: -1`.

### 4.4 Restore keys (`:121-136`)

- `chapterId` (Long, default `-1L`) is stored in `savedState["chapter_id"]` on every set (`:123-127`).
- `chapterPageIndex` (Int, default `-1`) is stored in `savedState["page_index"]` on every set (`:132-136`).
- Purpose: restore after process death. The activity passes the manga and the initial chapter through intent extras (section 12.1), but after process death the view model is recreated with the saved values.
- `chapterPageIndex` is written in `updateChapterProgress` (`:535`).

### 4.5 `needsInit()` and `init()` (`:266-301`)

- `needsInit() = manga == null` (`:266-268`).
- `init(mangaId, initialChapterId): Result<Boolean>` (`:274-301`):
  1. If not `needsInit()`, return success(true).
  2. `withIOContext`: `manga = getManga.await(mangaId)`.
  3. If `manga != null`:
     - `sourceManager.isInitialized.first { it }` (blocks until extension/source loading completes).
     - `mutableState.update { it.copy(manga = manga) }`.
     - `if (chapterId == -1L) chapterId = initialChapterId`. A restored `chapterId` wins over the intent value.
     - `source = sourceManager.getOrStub(manga.source)` (a `StubMangaSource` is returned for an uninstalled source).
     - `loader = ChapterLoader(context, downloadManager, downloadProvider, manga, source)`.
     - `loadChapter(loader!!, chapterList.first { chapterId == it.chapter.id })`. See 4.6 for the failure mode of `first`.
     - return success(true).
  4. Else: success(false) (activity shows "Unknown err", section 12.3).
  5. `catch (e: Throwable)`: rethrow `CancellationException`; otherwise `Result.failure(e)`.

### 4.6 `chapterList`: the reader's chapter set (`:154-223`)

This `lazy` is computed once, on the first access (in practice inside `init`, on IO). It is built with `runBlocking`.

OBSERVED algorithm:
1. `manga = manga!!`. `chapters = runBlocking { getChaptersByMangaId.await(manga.id, applyScanlatorFilter = true) }` (`:156`). The scanlator filter is applied by the repository.
2. `selectedChapter = chapters.find { it.id == chapterId } ?: error("Requested chapter of id $chapterId not found in chapter list")` (`:158-159`). This is an `IllegalStateException` with a message that is NOT a localisation key. It is shown raw by `setInitialChapterError` if it reaches the activity (it is rare, because the chapter came from the same list).
3. Filtering (`:161-203`), applied when `skipRead || skipFiltered` is true (both are preferences, section 3 of the data file):
   - Per chapter, the removal predicate is evaluated as a `when` with three arms in this order:
     1. `skipRead && chapter.read` -> REMOVE.
     2. `skipFiltered` -> REMOVE when any of these holds:
        - `manga.unreadFilterRaw == CHAPTER_SHOW_READ && !read`
        - `manga.unreadFilterRaw == CHAPTER_SHOW_UNREAD && read`
        - `manga.downloadedFilterRaw == CHAPTER_SHOW_DOWNLOADED && !isChapterDownloaded(name, scanlator, title, source)` (no `skipCache`, so the download cache is used)
        - `manga.downloadedFilterRaw == CHAPTER_SHOW_NOT_DOWNLOADED && isChapterDownloaded(...)`
        - `manga.bookmarkedFilterRaw == CHAPTER_SHOW_BOOKMARKED && !bookmark`
        - `manga.bookmarkedFilterRaw == CHAPTER_SHOW_NOT_BOOKMARKED && bookmark`
     3. else KEEP.
   - Note: with `skipRead = true, skipFiltered = false`, only the read rule applies.
   - `filteredChapters.any { it.id == chapterId }` -> keep `filteredChapters`; otherwise `filteredChapters + listOf(selectedChapter)` (the current chapter is re-appended at the END before sorting, so it stays reachable even when filtered out).
   - When neither preference is set, the whole list is used (`else -> chapters`).
4. `sortedWith(getChapterSort(manga, sortDescending = false))` (`:206`). Always ascending by the manga's `sorting` mode (section 5).
5. If `skipDupe`: `removeDuplicates(selectedChapter)` (`:208-210`, section 5.2). Else unchanged.
6. If `basePreferences.downloadedOnly()`: `filterDownloadedChapters(manga)` (`:215-217`). For a local manga this returns the list unchanged. Otherwise it keeps only chapters whose download cache entry exists (`app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterFilterDownloaded.kt:13-26`, `skipCache = false`). The current chapter is NOT re-appended here.
7. `map { it.toDbChapter() }.map(::ReaderChapter)` (`:221-222`).

OBSERVED failure mode: step 6 can remove the requested chapter. `init` then calls `chapterList.first { chapterId == it.chapter.id }` (`:288`), which throws `NoSuchElementException`. The message is the Kotlin default "Collection contains no element matching the predicate.". `init` returns `Result.failure`, and the activity shows that text as a toast after closing (section 12.3). This is an edge case the Rust port must handle explicitly (section 4 of `p1-edge-cases-and-tests.md`).

### 4.7 `incognitoMode` and download-ahead amount (`:225-226`)

- `incognitoMode: Boolean by lazy { getIncognitoState.await(manga?.source) }` (computed once).
- `downloadAheadAmount = downloadPreferences.autoDownloadWhileReading().get()` is read ONCE when the view model is constructed. Changing the preference while the reader is open has no effect until the next open.

### 4.8 The state-save `init` block (`:228-243`)

OBSERVED: for every distinct `viewerChapters.currChapter` (by `ReaderChapter` equality):
- if `chapterPageIndex >= 0` -> `currentChapter.requestedPage = chapterPageIndex`;
- else if `!currentChapter.chapter.read` -> `requestedPage = last_page_read`;
- then `chapterId = currentChapter.chapter.id!!`.

Note the asymmetry with `ChapterLoader` (section 2.1 step 4): here an unread chapter opens at `last_page_read` regardless of `preserveReadingPosition`. The restore key wins when present.

### 4.9 `onCleared` and `onActivityFinish` (`:245-261`)

- `onCleared()`: if `viewerChapters != null`, `unref()` it (this recycles every loader). If `chapterToDownload` is non-null, `downloadManager.addDownloadsToStartOfQueue(listOf(it))`. This runs when the view model is destroyed, not when the activity finishes.
- `onActivityFinish()` = `deletePendingChapters()` (section 7.8). Called from `ReaderActivity.finish()`.

---

## 5. Chapter ordering, duplicates and the sort modes

### 5.1 `getChapterSort(manga, sortDescending)` (`domain/src/main/java/tachiyomi/domain/items/chapter/service/ChapterSorter.kt:7-30`)

OBSERVED, for `manga.sorting` (= `chapterFlags and 0x300`, `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:43-44`):

| `sorting` | constant (`Manga.kt`) | `sortDescending = true` comparator | `sortDescending = false` comparator |
|---|---|---|---|
| `0x00000000` | `CHAPTER_SORTING_SOURCE` | `c1.sourceOrder.compareTo(c2.sourceOrder)` | `c2.sourceOrder.compareTo(c1.sourceOrder)` |
| `0x00000100` | `CHAPTER_SORTING_NUMBER` | `c2.chapterNumber.compareTo(c1.chapterNumber)` | `c1.chapterNumber.compareTo(c2.chapterNumber)` |
| `0x00000200` | `CHAPTER_SORTING_UPLOAD_DATE` | `c2.dateUpload.compareTo(c1.dateUpload)` | `c1.dateUpload.compareTo(c2.dateUpload)` |
| `0x00000300` | `CHAPTER_SORTING_ALPHABET` | `c2.name.compareToWithCollator(c1.name)` | `c1.name.compareToWithCollator(c2.name)` |
| other | | throw `NotImplementedError("Invalid chapter sorting method: ${manga.sorting}")` | same |

OBSERVED consequence: the reader calls it with `sortDescending = false` (`R/ReaderViewModel.kt:206`). For `SOURCE` that means DESCENDING `sourceOrder` (the source lists newest first, so the reader shows oldest first). For the other three modes the reader is ascending.

- `sourceOrder` is the index in the source's list (`sourceOrder: Long` in the domain model, `domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt:10`).
- `compareToWithCollator` is a locale-aware collator comparison from `core/common` (`tachiyomi.core.common.util.lang`). The collation rules are the platform `java.text.Collator` rules for the default locale (cross-reference area:13-core-util-storage-archive-torrent).

**Proposal:** `SOURCE` -> descending `source_order` (preserve the reversed behaviour explicitly and name it "oldest first"). `ALPHABET` -> `icu_collator` with the user's locale (crate `icu`), not a byte compare. Keep `NUMBER` ascending by `chapter_number` (Double, `-1.0` for unknown) with unknown numbers sorted first.

### 5.2 `removeDuplicates(currentChapter)` (`app/src/main/java/eu/kanade/tachiyomi/util/chapter/ChapterRemoveDuplicates.kt:8-15`)

OBSERVED: `groupBy { it.chapterNumber }` (keys are `Double`), then for each group, in group first-appearance order:
1. `chapters.find { it.id == currentChapter.id }` (keep the current chapter if it is in the group);
2. else `chapters.find { it.scanlator == currentChapter.scanlator }` (keep the same scanlator);
3. else `chapters.first()`.

OBSERVED consequences:
- Every chapter with an UNKNOWN number (`chapterNumber == -1.0`) is in one group. With `skipDupe` on, ALL unknown-number chapters collapse into ONE entry.
- The output keeps the first-appearance order of the groups. The input was already sorted (section 4.6 step 4), so the output stays sorted.
- Duplicates removed here are not visible to `markDuplicateReadChapterAsRead` (section 7.6).

### 5.3 `Chapter.isRecognizedNumber` (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/manga/Chapter.kt:30-31`)

OBSERVED: `chapter_number >= 0f`. Domain: `Chapter.isRecognizedNumber` (`domain/.../chapter/model/Chapter.kt:19`). The "unknown" sentinel is `-1` (`MissingChapters.kt:13` filters `-1.0`).

### 5.4 Missing chapter gap (`domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt:40-49`)

OBSERVED:
- `calculateChapterGap(higher: Chapter?, lower: Chapter?)`: 0 if either is null or either number is unrecognised; else `calculateChapterGap(higher.chapterNumber, lower.chapterNumber)`.
- `calculateChapterGap(h: Double, l: Double)`: 0 if `h < 0 || l < 0`; else `floor(h).toInt() - floor(l).toInt() - 1`.
- Example: current 5.0, next 9.0 gives 3 missing chapters (6, 7, 8). Current 5.0, next 5.5 gives `5 - 5 - 1 = -1`, which is NOT greater than 0, so no warning. Only a positive gap shows the warning (section 14).

---

## 6. Loading chapters (view model side)

### 6.1 `loadChapter(loader, chapter)` (`:307-334`)

OBSERVED:
1. `loader.loadChapter(chapter)` (throws on failure; callers handle it).
2. `chapterPos = chapterList.indexOf(chapter)` (uses `ReaderChapter` equality).
3. `newChapters = ViewerChapters(chapter, chapterList.getOrNull(chapterPos - 1), chapterList.getOrNull(chapterPos + 1))`.
4. `withUIContext`: `mutableState.update { newChapters.ref(); it.viewerChapters?.unref(); chapterToDownload = cancelQueuedDownloads(newChapters.currChapter); it.copy(viewerChapters = newChapters, bookmarked = newChapters.currChapter.chapter.bookmark) }`.
   - New references are taken BEFORE old ones are released, so a chapter present in both sets is not recycled.
   - OBSERVED hazard: `MutableStateFlow.update` may re-run its lambda on contention. The lambda contains side effects (`ref`, `unref`, `cancelQueuedDownloads`). Under a race the side effects can run twice (for example a double `ref` or a double cancel). Proposal: compute the new set first, then commit state in a single assignment.
5. `return newChapters`.

Neighbour rule: prev/next are by POSITION in `chapterList`. If the chapter is not in the list, `indexOf` is -1, so prev = `getOrNull(-2)` (null) and next = `getOrNull(0)` (the first chapter). This case is not reachable through normal flows (the chapter comes from the list) but is possible through restore edge cases.

### 6.2 `loadNewChapter(chapter)` (`:340-358`): the swipe/scroll path

Called when `onPageSelected` sees a page from a different chapter than the current one.
1. `viewModelScope.launchIO`.
2. `flushReadTimer()` (save history for the OLD chapter, section 7.9) then `restartReadTimer()`.
3. `loadChapter(loader, chapter)`; errors are logged (`LogPriority.ERROR`) and swallowed (`CancellationException` rethrown).

### 6.3 `loadAdjacent(chapter)` (`:363-381`): the toolbar button path

1. `mutableState.update { isLoadingAdjacentChapter = true }`.
2. `withIOContext { loadChapter(loader, chapter) }`.
3. `catch`: log, swallow (except `CancellationException`).
4. `finally`: `isLoadingAdjacentChapter = false`.

OBSERVED consequence: on failure the caller cannot tell. `loadNextChapter()` / `loadPreviousChapter()` return normally (`:608-619`). The activity then calls `moveToPageIndex(0)` (section 12.5), which moves to page 0 of the STILL-CURRENT chapter. A failed "next chapter" press therefore jumps to the first page of the current chapter.

### 6.4 `preload(chapter)` (`:387-422`): cross-chapter preloading

Called from `ReaderActivity.requestPreloadChapter` (`:702-704`) on `launchIO`. The viewers call it near chapter edges (area:44-reader-viewers).

OBSERVED steps:
1. If `chapter.state` is `Loaded` or `Loading`, return.
2. If `chapter.pageLoader?.isLocal == false` (an online loader exists) and `isChapterDownloaded(..., skipCache = true)`: set `chapter.state = Wait` (so the next load picks the download loader). Note the `manga ?: return` in this branch.
3. If state is neither `Wait` nor `Error`, return. (After step 2 a downloaded online chapter is `Wait`.)
4. `loader = loader ?: return`.
5. `try { loader.loadChapter(chapter) } catch (e) { if CancellationException throw; return }`. A failure is silent: the state remains `Error`, and no event is sent.
6. `eventChannel.trySend(Event.ReloadViewerChapters)`. Subject to the rendezvous drop (section 4.2).

### 6.5 `setMangaReadingMode` / `setMangaOrientationType` side effects

Covered in section 9.

---

## 7. Progress, completion, history and deletion

### 7.1 `onPageSelected(page)` (`:435-460`)

Called from the viewers via `ReaderActivity.onPageSelected` (`ReaderActivity.kt:686-688`) every time the visible page changes.

OBSERVED, in this order:
1. If `page is InsertPage` return (no progress change).
2. `selectedChapter = page.chapter`; `pages = selectedChapter.pages ?: return`.
3. `viewModelScope.launchNonCancellable { updateChapterProgress(selectedChapter, page) }` (section 7.2).
4. If `selectedChapter != getCurrentChapter()`: `loadNewChapter(selectedChapter)` (section 6.2).
5. `inDownloadRange = page.number.toDouble() / pages.size > 0.25`; if true, `downloadNextChapters()` (section 8).
6. `eventChannel.trySend(Event.PageChanged)` (drop-able; triggers the display refresh flash).

OBSERVED: `page.number` is `index + 1`. The threshold is therefore on the 1-based number:
- size 1: 1/1 = 1.0 > 0.25 at the only page.
- size 3: 1/3 = 0.33 > 0.25, so page 1 already triggers the download-ahead check.
- size 4: 1/4 = 0.25, not greater; page 2 (2/4 = 0.5) triggers.

### 7.2 `updateChapterProgress(readerChapter, page)` (`:528-552`)

OBSERVED:
1. `pageIndex = page.index`.
2. `mutableState.update { currentPage = pageIndex + 1 }` (drives the page indicator and slider).
3. `readerChapter.requestedPage = pageIndex`.
4. `chapterPageIndex = pageIndex` (saved state, runs in incognito too).
5. If `!incognitoMode && page.status != Page.State.ERROR`:
   - `readerChapter.chapter.last_page_read = pageIndex`
   - if `readerChapter.pages?.lastIndex == pageIndex`: `updateChapterProgressOnComplete(readerChapter)` (section 7.3). This runs EVERY time the last page becomes visible, including when the chapter is already read.
   - `updateChapter.await(ChapterUpdate(id = chapter.id!!, read = chapter.read, lastPageRead = last_page_read.toLong()))`.

OBSERVED: the database update always writes `read` with the in-memory value, so a read chapter stays read when the user scrolls back. Nothing ever sets `read = false` in this file.

### 7.3 `updateChapterProgressOnComplete(readerChapter)` (`:554-577`)

OBSERVED, in order:
1. `readerChapter.chapter.read = true`.
2. `updateTrackChapterRead(readerChapter)` (section 7.5).
3. `deleteChapterIfNeeded(readerChapter)` (section 7.7).
4. `markDuplicateAsRead = libraryPreferences.markDuplicateReadChapterAsRead().get().contains(MARK_DUPLICATE_CHAPTER_READ_EXISTING)`. Constant `"existing"` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:409`). Preference key `"mark_duplicate_read_chapter_read"` (`LibraryPreferences.kt:377`), a string set. If absent, return.
5. `duplicateUnreadChapters = chapterList.mapNotNull { chapter where !read && isRecognizedNumber && chapter_number == readerChapter.chapter_number -> ChapterUpdate(id, read = true) }` (`:563-575`).
6. `updateChapter.awaitAll(duplicateUnreadChapters)` (`:576`).

OBSERVED: `chapter_number` is a `Float` (`SChapter.chapter_number`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapter.kt:15`). Equality is float equality. It only matches chapters in `chapterList`, so with `skipDupe` on, duplicates already removed from the list are not marked (section 5.2).

### 7.4 `ChapterUpdate` fields used here

`ChapterUpdate(id, read?, lastPageRead?, bookmark?)`. Only the fields given are changed (the others are null). Lines: `:544-550` (progress), `:571` (duplicates, `read = true` only), `:651-656` (bookmark). Exact definition: `domain/src/main/java/tachiyomi/domain/items/chapter/model/ChapterUpdate.kt` (area:07-domain-entries-items).

### 7.5 `updateTrackChapterRead(readerChapter)` (`:918-928`)

OBSERVED: returns if `incognitoMode`; returns if `!trackPreferences.autoUpdateTrack().get()`; `manga ?: return`; `launchNonCancellable { trackChapter.await(context, manga.id, readerChapter.chapter.chapter_number.toDouble()) }`. Errors are ignored by the tracker interactor (area:27-tracking-core). The `chapter_number` is passed as `Double`.

### 7.6 `deleteChapterIfNeeded(currentChapter)` (`:508-522`)

OBSERVED:
1. `removeAfterReadSlots = downloadPreferences.removeAfterReadSlots().get()` (default `-1`, `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:28`).
2. If `removeAfterReadSlots == -1` return. Note: `chapterToDownload` is NOT cleared in this case.
3. `currentChapterPosition = chapterList.indexOf(currentChapter)`; `chapterToDelete = chapterList.getOrNull(currentChapterPosition - removeAfterReadSlots)`.
4. `chapterToDownload = null` ("If chapter is completely read, no need to download it").
5. If `chapterToDelete != null`: `enqueueDeleteReadChapters(chapterToDelete)`.

OBSERVED quirks:
- Slot value `0` deletes the chapter itself (position minus 0).
- Values below `-1` (for example `-2`) delete chapters AHEAD of the current one (`position + 2`). Only `-1` is documented as "disabled". The Rust port must clamp to `>= 0` or treat negatives as disabled. This is a stated deviation in the proposal.

### 7.7 `enqueueDeleteReadChapters(chapter)` (`:934-944`)

OBSERVED: returns if `!chapter.chapter.read` (only READ chapters are ever queued for deletion). Otherwise `launchNonCancellable { downloadManager.enqueueChaptersToDelete(listOf(chapter.chapter.toDomainChapter()!!), manga) }`. Deletion itself is deferred:

### 7.8 `deletePendingChapters()` (`:950-954`) and `MangaDownloadManager.deletePendingChapters` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:338-344`)

OBSERVED: triggered by `ReaderActivity.finish()` -> `viewModel.onActivityFinish()`. The download manager's pending deleter returns the queue and deletes each chapter using the source found by `sourceManager.get(manga.source)` (sources that no longer exist are skipped with `?: continue`). The queue is persisted by the download manager across process death (comment at `R/ReaderViewModel.kt:930-933`).

### 7.9 History: `flushReadTimer`, `restartReadTimer`, `updateHistory` (`:579-603`)

OBSERVED:
- `restartReadTimer()`: `chapterReadStartTime = Instant.now().toEpochMilli()` (`:579-581`). Called from `onResume` and `loadNewChapter`.
- `flushReadTimer()` (`:583-589`): `getCurrentChapter()?.let { launchNonCancellable { updateHistory(it) } }`. The chapter is captured synchronously; the write happens later. Called from `onPause` (`ReaderActivity.kt:258-261`) and `loadNewChapter`.
- `updateHistory(readerChapter)` (`:594-603`): returns if `incognitoMode`. `readAt = Date()`; `sessionReadDuration = chapterReadStartTime?.let { readAt.time - it } ?: 0`; `upsertHistory.await(MangaHistoryUpdate(chapterId, readAt, sessionReadDuration))`; `chapterReadStartTime = null`.
- Duration unit is milliseconds. If the start time is null (already flushed), the upsert still happens with duration 0. Two consecutive flushes (pause, then a page change) therefore write two history rows' updates for the same chapter, the second with duration 0.
- OBSERVED race: `chapterReadStartTime` is read and cleared on IO inside a coroutine, but `restartReadTimer` writes it on the main thread. No synchronisation.

### 7.10 Incognito (`incognitoMode`)

OBSERVED: `incognitoMode` gates: progress DB writes (7.2), history (7.9), track updates (7.5). It does NOT gate the saved-state keys (4.4) or the page-list cache. The activity also closes the reader when the global incognito preference is turned OFF: `preferences.incognitoMode().changes().drop(1).onEach { if (!it) finish() }` (`ReaderActivity.kt:192-196`).

---

## 8. Download-ahead (`downloadNextChapters`, `:462-491`)

OBSERVED:
1. `if (downloadAheadAmount == 0) return`. The amount is `auto_download_while_reading` (default `0`, `DownloadPreferences.kt:25`).
2. `manga ?: return`.
3. `if (getCurrentChapter()?.pageLoader !is DownloadPageLoader) return`. Download-ahead only runs while the CURRENT chapter is read from downloads.
4. `nextChapter = state.viewerChapters?.nextChapter?.chapter ?: return`.
5. `viewModelScope.launchIO`:
   - `isNextChapterDownloaded = downloadManager.isChapterDownloaded(nextChapter.name, nextChapter.scanlator, manga.title, manga.source)`. If false, return.
   - `chaptersToDownload = getNextChapters.await(manga.id, nextChapter.id!!)` then `if (skipDupe) removeDuplicates(nextChapter.toDomainChapter()!!)`, then `.take(downloadAheadAmount)`.
   - `downloadManager.downloadChapters(manga, chaptersToDownload)`.

`GetNextChapters.await(mangaId, fromChapterId, onlyUnread = true)` (`domain/src/main/java/tachiyomi/domain/history/manga/interactor/GetNextChapters.kt`): the chapters are sorted with `getChapterSort(manga, sortDescending = false)`, the read ones are removed, then the list is sub-listed from the index of `fromChapterId`.

OBSERVED quirk: if the `nextChapter` is itself READ, `indexOfFirst` returns -1 and `max(0, -1) = 0`, so the list starts at the FIRST unread chapter of the manga. The download-ahead can therefore queue chapters far before the reader's position.

---

## 9. Per-manga reading mode, orientation, crop

### 9.1 Reading mode (`:669-703`)

- `getMangaReadingMode(resolveDefault = true)` (`:669-676`):
  - `default = readerPreferences.defaultReadingMode().get()` (default `ReadingMode.RIGHT_TO_LEFT.flagValue = 0x2`, `ReaderPreferences.kt:38-41`).
  - `readingMode = ReadingMode.fromPreference(manga?.readingMode?.toInt())`.
  - If `resolveDefault && readingMode == DEFAULT` return `default`; else return `manga?.readingMode?.toInt() ?: default`.
  - So with `resolveDefault = false`, a manga whose flag is `0` returns `0`, not the default.
- `manga.readingMode` = `viewerFlags and 0x7` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:17-18`).
- `setMangaReadingMode(readingMode)` (`:681-703`), called from the settings sheet (UI thread):
  1. `runBlocking(Dispatchers.IO) { ... }`. This BLOCKS the caller (the UI thread) until the body finishes.
  2. `setMangaViewerFlags.awaitSetReadingMode(manga.id, readingMode.flagValue.toLong())`.
  3. If `viewerChapters != null`: `currChapter.requestedPage = currChapter.chapter.last_page_read` (the saved page becomes the restore target).
  4. `mutableState.update { manga = getManga.await(id); viewerChapters = currChapters }`.
  5. `eventChannel.send(Event.ReloadViewerChapters)` (suspending send inside `runBlocking` on the UI thread, section 12.9 risk).

### 9.2 Orientation (`:708-740`)

- `getMangaOrientation(resolveDefault = true)`: same structure. `default = defaultOrientationType().get()` (default `ReaderOrientation.FREE.flagValue = 0x8`, `ReaderPreferences.kt:43-46`). `orientation = ReaderOrientation.fromPreference(manga?.readerOrientation?.toInt())`. `manga.readerOrientation = viewerFlags and 0x38` (`Manga.kt:20-21`).
- `setMangaOrientationType(orientation)` (`:720-740`) on `launchIO`:
  1. `awaitSetOrientation(manga.id, orientation.flagValue.toLong())`.
  2. Same save-page + state update as 9.1.
  3. `eventChannel.send(Event.SetOrientation(getMangaOrientation()))` (resolved value).
  4. `eventChannel.send(Event.ReloadViewerChapters)`.
- Activity application: `setOrientation(orientation: Int)` (`ReaderActivity.kt:785-790`): `ReaderOrientation.fromPreference(orientation).flag`; sets `requestedOrientation` only if it differs from the current value.

### 9.3 Crop borders (`:742-749`)

OBSERVED: `isPagerType = ReadingMode.isPagerType(getMangaReadingMode())`. If pager, toggle `cropBorders` (default false, `ReaderPreferences.kt:69`); else toggle `cropBordersWebtoon` (default false, `:75`). `toggle()` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:73-76`) sets `!get()` and returns the new value. The activity shows "On" or "Off" (`ReaderActivity.kt:425-429`).

### 9.4 Bookmark (`:645-664`)

OBSERVED: `chapter = getCurrentChapter()?.chapter ?: return`; `bookmarked = !chapter.bookmark`; the in-memory chapter is set first; `launchNonCancellable { updateChapter.await(ChapterUpdate(id, bookmark = bookmarked)) }`; then `state.bookmarked = bookmarked`.

---

## 10. Page actions: save, share, copy, cover

The page is taken from `state.dialog` (`Dialog.PageActions(page)`). All four actions return immediately unless `page.status == READY`, and `manga` is non-null.

### 10.1 Filename (`generateFilename`, `:754-766`)

OBSERVED: `chapter = page.chapter.chapter`; `suffix = " - ${page.number}"`; result = `DiskUtil.buildValidFilename("${manga.title} - ${chapter.name}".takeBytes(DiskUtil.MAX_FILE_NAME_BYTES - suffix.byteSize())) + suffix`. `MAX_FILE_NAME_BYTES = 250` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:143`). `takeBytes` truncates at a UTF-8 boundary and removes U+FFFD (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/StringExtensions.kt:50-58`). `buildValidFilename` is in `DiskUtil.kt:109` (cross-reference area:13-core-util-storage-archive-torrent).

Example: manga "Foo", chapter "Ch 1", page 5 gives "Foo - Ch 1 - 5".

### 10.2 Save (`saveImage`, `:804-843`)

1. `notifier = SaveImageNotifier(context)`; `notifier.onClear()` (cancels any previous "saved" notification, id `Notifications.ID_DOWNLOAD_IMAGE`).
2. `relativePath = if (readerPreferences.folderPerManga().get()) DiskUtil.buildValidFilename(manga.title) else ""` (default `create_folder_per_manga` = false, `ReaderPreferences.kt:81`).
3. `launchNonCancellable`: `try { uri = imageSaver.save(Image.Page(inputStream = page.stream!!, name = filename, location = Location.Pictures.create(relativePath))) ; withUIContext { notifier.onComplete(uri); send(SavedImage(Success(uri))) } } catch (e: Throwable) { notifier.onError(e.message); send(SavedImage(Error(e))) }`.
4. The activity then toasts `picture_saved` ("Picture saved") (`ReaderActivity.kt:757-766`). This is a second confirmation next to the notification.
- Location: the app Pictures directory (`Location.Pictures`, implemented by `ImageSaver`, owned by area:13-core-util-storage-archive-torrent and area:35-data-misc-updater-notification-export).

### 10.3 Share and copy (`shareImage(copyToClipboard)`, `:852-877`)

1. `destDir = context.cacheImageDir` (`eu.kanade.tachiyomi.util.storage.cacheImageDir`).
2. `try { viewModelScope.launchNonCancellable { destDir.deleteRecursively(); uri = imageSaver.save(Image.Page(stream, filename, Location.Cache)); send(if (copyToClipboard) CopyImage(uri) else ShareImage(uri, page)) } } catch (e: Throwable) { logcat(ERROR, e) }`.
- OBSERVED: `launchNonCancellable` only starts a coroutine. The surrounding `try/catch` cannot catch exceptions raised later inside that coroutine. Exceptions there escape to the `viewModelScope` (no handler), which crashes the app on Android. The catch is dead code. Proposal: handle errors inside the coroutine and show a message.
- Only the LAST shared image is kept, because the cache directory is emptied first.
- Activity: share -> `onShareImageResult` (section 12.8), copy -> `onCopyImageResult` (clipboard, section 12.8).

### 10.4 Set as cover (`setAsCover`, `:882-901`)

1. `stream = page.stream ?: return`.
2. `launchNonCancellable`: `result = try { manga.editCover(Injekt.get(), stream()); if (manga.isLocal() || manga.favorite) Success else AddToLibraryFirst } catch (e: Exception) { Error }`; `send(SetCoverResult(result))`.
- OBSERVED: `editCover` runs BEFORE the library check. The cover is written even when the manga is not in the library, and then `AddToLibraryFirst` is reported.
- `catch (e: Exception)` does not catch `Throwable`; an `Error` (for example OutOfMemoryError) would escape.

### 10.5 Result types

- `SetAsCoverResult { Success, AddToLibraryFirst, Error }` (`:903-907`).
- `SaveImageResult { Success(uri: Uri), Error(error: Throwable) }` (`:909-912`).

---

## 11. Dialogs and the `Dialog` sealed interface (`:979-985`)

| Dialog | Trigger | Rendered by |
|---|---|---|
| `Loading` | `showLoadingDialog()` (from `isLoadingAdjacentChapter`) | `ReaderActivity.kt:441-455` |
| `Settings` | `openSettingsDialog()` (bottom bar cog) | `ReaderSettingsDialog` (section 19) |
| `ReadingModeSelect` | `openReadingModeSelectDialog()` (bottom bar reading-mode button) | `ReadingModeSelectDialog` (section 15) |
| `OrientationModeSelect` | `openOrientationModeSelectDialog()` | `OrientationSelectDialog` (section 15) |
| `PageActions(page)` | `openPageDialog(page)` (viewer long-tap via `onPageLongTap`) | `ReaderPageActionsDialog` (section 15) |

`closeDialog()` sets `dialog = null`. The Loading dialog is non-dismissable (`onDismissRequest = {}`, no buttons, `ReaderActivity.kt:442-454`).

`ReaderActivity.setProgressDialog(show)` (`:641-647`) maps the `isLoadingAdjacentChapter` flag to `showLoadingDialog()` or `closeDialog()`. Note that `closeDialog()` also closes any other open dialog, so a page-actions sheet open during a chapter switch is closed.

---

## 12. `ReaderActivity`: lifecycle, input, menu, viewer swap

File: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt`.

### 12.1 Intent contract (`:107-113`, `:163-169`)

- `newIntent(context, mangaId: Long?, chapterId: Long?)` puts extras `"manga"` and `"chapter"` (Long, may be null) with `FLAG_ACTIVITY_CLEAR_TOP`.
- `onCreate` reads `intent.extras?.getLong("manga", -1) ?: -1L` and the same for `"chapter"`. If either is `-1` the activity calls `finish()` and returns (no error shown).
- Only when `viewModel.needsInit()` (fresh view model, not a recreation): dismiss notification `Notifications.ID_NEW_CHAPTERS` for `manga.hashCode()` (`:170-174`), then `lifecycleScope.launchNonCancellable { init(manga, chapter) }`. On failure `withUIContext { setInitialChapterError(exception) }`.

### 12.2 Activity transitions (`:145-156`, `:293-305`)

- On API 34+ (`UPSIDE_DOWN_CAKE`): `overrideActivityTransition(OPEN, R.anim.shared_axis_x_push_enter, R.anim.shared_axis_x_push_exit)`; on finish `CLOSE` with `shared_axis_x_pop_enter/exit`. Older: `overridePendingTransition` with the same resources.

### 12.3 Initial load error (`setInitialChapterError`, `:629-633`)

OBSERVED: `logcat(ERROR, error); finish(); toast(error.message)`. The reader closes. The message is the raw exception message: "No pages found" (`page_list_empty_error`), "Source not installed: ..." (`source_not_installed`), "Source not found" (`loader_not_implemented_error`), "Unknown err" (`IllegalStateException("Unknown err")`, when the manga does not exist, `:179-181`), or the Kotlin `NoSuchElementException` text (section 4.6). `error.message` may be null; the activity passes it straight to `toast(...)` (`:632`). The null behaviour of that helper is in `app/src/main/java/eu/kanade/tachiyomi/util/system/` and was not re-read in this session.

### 12.4 Key handling (`:308-325`)

- `onKeyUp`: `KEYCODE_N` -> `loadNextChapter()`, consumed; `KEYCODE_P` -> `loadPreviousChapter()`, consumed. Other keys -> `super`.
- `dispatchKeyEvent`: `viewer.handleKeyEvent(event)` first (the viewer owns arrow/page keys, area:44-reader-viewers), then `super`.
- `dispatchGenericMotionEvent`: `viewer.handleGenericMotionEvent(event)` first, then `super` (mouse wheel and scroll belong to the viewer).

### 12.5 Chapter navigation from the toolbar (`:664-680`)

- `loadNextChapter()`: `lifecycleScope.launch { viewModel.loadNextChapter(); moveToPageIndex(0) }`. Same for previous. The page index is `0` in both directions (for previous chapters too; the user does not land on the last page).
- `moveToPageIndex(index)` (`:653-658`): if the viewer, the current chapter, and `pages.getOrNull(index)` are all non-null, `viewer.moveToPage(page)`.

### 12.6 Menu visibility and immersive mode (`:516-528`, `:267-282`)

OBSERVED `setMenuVisibility(visible)`:
- `viewModel.showMenus(visible)` first.
- Visible: `windowInsetsController.show(systemBars())` and `window.addFlags(FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS)`.
- Hidden: only if the fullscreen preference (default `true`) is on, `hide(systemBars())` and `systemBarsBehavior = BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE`.
- Re-applied on resume (`onResume`), on window focus gain (`onWindowFocusChanged`), after the cutout change, and at initial menu setup.
- `toggleMenu()`, `showMenu()`, `hideMenu()` (`:710-730`) are called by the viewers (they handle taps).

### 12.7 Viewer creation and replacement (`updateViewer`, `:533-563`)

Called whenever `state.manga` changes (`distinctUntilChanged` by `Manga` equality, `:204-209`). OBSERVED steps:
1. `prevViewer = state.viewer`; `newViewer = ReadingMode.toViewer(getMangaReadingMode(), this)`.
2. If the shared-element enter transition is a `MaterialContainerTransform`, apply orientation after `doOnEnd`; otherwise immediately: `setOrientation(getMangaOrientation())`.
3. If there was a previous viewer: `destroy()` it and `removeAllViews()` on the container.
4. `viewModel.onViewerLoaded(newViewer)`; `updateViewerInset(fullscreen)`; add the viewer view.
5. If `showReadingMode` (default true, `ReaderPreferences.kt:30`): `showReadingModeToast(getMangaReadingMode())`.
6. Add a `ReaderProgressIndicator` to the container; it is removed on the first `setChapters` call (`:615`).
7. `startPostponedEnterTransition()`.

OBSERVED consequence: every manga state change recreates the viewer, including the one caused by `setMangaReadingMode` (which also emits `ReloadViewerChapters`).

### 12.8 Results of page actions (`:732-780`)

- `onShareImageResult(uri, page)` (`:736-745`): message = `stringResource(MR.strings.share_page_info, manga.title, chapter.name, page.number)` (key "share_page_info" = "%1$s: %2$s, page %3$d"), `uri.toShareIntent(applicationContext, message)`, chooser titled `action_share`.
- `onCopyImageResult(uri)` (`:747-751`): `ClipData.newUri(contentResolver, "", uri)` set as the primary clip.
- `onSaveImageResult` (`:757-766`): `Success` -> `toast(picture_saved)`; `Error` -> log only.
- `onSetAsCoverResult` (`:772-780`): `Success` -> `cover_updated`; `AddToLibraryFirst` -> `notification_first_add_to_library`; `Error` -> `notification_cover_update_failed`.

### 12.9 Concurrency risks in the activity

- `setMangaReadingMode` runs `runBlocking(Dispatchers.IO)` containing `eventChannel.send(...)` (rendezvous). The UI thread is blocked until the body completes. The collector that receives the event runs on the main thread and is blocked too. The send succeeds only if a receiver is already waiting. If the main collector is busy when the send happens, the process deadlocks. Stated as a risk, not an observed failure.
- Proposal: no blocking on the UI thread. Make the write async and send the state change through the single UI-owned event queue.

### 12.10 Other activity lifecycle hooks

- `onPause`: `viewModel.flushReadTimer()` (`:258-261`).
- `onDestroy`: `viewer.destroy()`, `config = null`, cancel both toasts (`:250-256`).
- `onProvideAssistContent`: `webUri = assistUrl` (`:284-287`). `assistUrl` = `getChapterUrl()` of the current chapter, set in `setChapters` on IO (`:618-622`). Used by the system assistant; Linux has no equivalent (drop, P3).
- `openMangaScreen()` (`:565-575`): `MainActivity` with `Constants.SHORTCUT_MANGA` and `MANGA_EXTRA` = manga id, `CLEAR_TOP`. Bound to the top bar title tap (`clickable(onClickTopAppBar)`, `ReaderAppBars.kt:97`).
- `openChapterInWebView()` (`:577-584`): `WebViewActivity.newIntent(this, assistUrl, source.id, manga.title)`. Only for HTTP sources (menu entry is absent otherwise).
- `openChapterInBrowser()` (`:586-590`): `openInBrowser(uri, forceDefaultBrowser = false)`.
- `shareChapter()` (`:592-597`): `toShareIntent(type = "text/plain")` of `assistUrl`, chooser titled `action_share`.
- `setChapters(viewerChapters)` (`:614-623`): remove the loading indicator; `viewer.setChapters(viewerChapters)`; `assistUrl = getChapterUrl()` on IO.

### 12.11 Reading-mode toast logic (`showReadingModeToast`, `:599-606`; `onChange` in `initializeMenu`, `:466-482`)

OBSERVED (note the inverted condition):
- When a viewer is created and `showReadingMode` is TRUE, the toast with the reading mode name is shown (`ReaderActivity.kt:555-557`).
- When the reading mode is changed in the dialog, the toast is shown only if `showReadingMode` is FALSE (`:470-472`). Toggling the setting on therefore suppresses the dialog toast, which is the intended "already shown" behaviour, but the rule is not symmetric with the orientation dialog, which always toasts (`:480-482`).

### 12.12 Keys and menu accessibility

- The bottom bar and top bar are Compose; their accessible names come from the string keys in section 15 and 16.

---

## 13. `ReaderActivity.ReaderConfig`: preference subscriptions (`:808-973`)

Every subscription uses `lifecycleScope.launchIn` and `changes()` (emit current value first, then changes).

| Preference | Action | Source |
|---|---|---|
| `readerTheme` | container background: `0` -> `Color.WHITE`; `2` -> `grayBackgroundColor` (`Color.rgb(0x20,0x21,0x25)`, `:834`); `3` -> `automaticBackgroundColor()` (gray when `isNightMode()`, else white); any other value (incl. `1`) -> `Color.BLACK` | `:840-850` |
| `BasePreferences.displayProfile` | read the ICC profile file bytes, `SubsamplingScaleImageView.setDisplayProfile(data)` and `TachiyomiImageDecoder.displayProfile = data` | `:853-855`, `:895-909` |
| `cutoutShort` | API 28+ only. `true` -> `LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES`, `false` -> `..._NEVER`; then `setMenuVisibility` | `:857-859`, `:911-921` |
| `keepScreenOn` | add or clear `FLAG_KEEP_SCREEN_ON` | `:861-863`, `:926-932` |
| `customBrightness` | `true` -> subscribe to `customBrightnessValue().changes().sample(100)` and apply each value; `false` -> apply `0` once | `:865-867`, `:937-946` |
| `grayscale` OR `invertedColors` | `setLayerPaint(grayscale, invertedColors)` on the viewer container | `:869-871`, `:969-972` |
| `fullscreen` | `WindowCompat.setDecorFitsSystemWindows(window, !fullscreen)` and `updateViewerInset(fullscreen)` | `:873-878` |

OBSERVED details:
- `setCustomBrightness(true)` launches a NEW subscription each time custom brightness is turned on, and the subscriptions are never cancelled until the lifecycle ends. Repeated toggles stack subscriptions.
- After custom brightness is turned OFF, the earlier value subscription is still alive. Any later change of `customBrightnessValue` re-applies the brightness even though the feature is off. The settings UI hides the slider while off, so this rarely fires.
- `setCustomBrightnessValue(value)` (`:954-968`): `value > 0` -> `window.screenBrightness = value / 100f`; `value < 0` -> `0.01f`; `0` -> `BRIGHTNESS_OVERRIDE_NONE` (-1, system brightness). Then `viewModel.setBrightnessOverlayValue(value)`. The black overlay (section 18) uses `value < 0`.
- `setLayerPaint` (`:969-972`): `paint = if (grayscale || invertedColors) getCombinedPaint(...) else null`, then `viewerContainer.setLayerType(LAYER_TYPE_HARDWARE, paint)`.
- `getCombinedPaint` (`:810-832`): `ColorMatrix()`; if grayscale `setSaturation(0f)`; if inverted `postConcat(ColorMatrix(floatArrayOf(-1,0,0,0,255, 0,-1,0,0,255, 0,0,-1,0,255, 0,0,0,1,0)))`. Order: grayscale first, then invert, applied to the already-desaturated matrix.

---

## 14. Chapter transition page (`P/ChapterTransition.kt`)

Composable entry: `ChapterTransition(transition, currChapterDownloaded, goingToChapterDownloaded)` (`:49-85`). The surrounding viewer page is area:44-reader-viewers.

OBSERVED:
- `currChapter = transition.from.chapter.toDomainChapter()`; `goingToChapter = transition.to?.chapter?.toDomainChapter()`.
- `Prev`:
  - top slot: label `transition_previous` ("Previous:"), chapter = `goingToChapter`, downloaded = `goingToChapterDownloaded`; fallback when null: `transition_no_previous` ("There's no previous chapter").
  - bottom slot: label `transition_current` ("Current:"), chapter = `currChapter`, downloaded = `currChapterDownloaded`.
  - gap = `calculateChapterGap(currChapter, goingToChapter)`.
- `Next`:
  - top slot: label `transition_finished` ("Finished:"), chapter = `currChapter`.
  - bottom slot: label `transition_next` ("Next:"), chapter = `goingToChapter`; fallback `transition_no_next` ("There's no next chapter").
  - gap = `calculateChapterGap(goingToChapter, currChapter)`.
- `TransitionText` (`:88-142`): `Column(widthIn(max = 460.dp), fillMaxWidth)`.
  - top: if chapter non-null, `ChapterText` then `Spacer(24.dp)`; else `NoChapterNotification(fallback)` centred.
  - bottom: if chapter non-null: if `gap > 0`, `ChapterGapWarning(gap)` centred; `Spacer(24.dp)`; `ChapterText`. Else `NoChapterNotification(fallback)`.
- `NoChapterNotification` (`:145-171`): `OutlinedCard` (container transparent, content `onSurface`), row padding 16 horizontal / 12 vertical, spacing 16, icon `Icons.Outlined.Info` tinted `primary`, text `bodyMedium`.
- `ChapterGapWarning` (`:174-203`): same card, icon `Icons.Outlined.Warning` tinted `error`, text `pluralStringResource(MR.plurals.missing_chapters_warning, count = gap, gap)`.
- `ChapterText` (`:218-270`): header (`titleMedium`, padding bottom 4 dp) with `header`; name (`titleLarge`, `fontSize = 20.sp`, `maxLines = 5`, `TextOverflow.Ellipsis`) prefixed with an inline check icon (`Icons.Filled.CheckCircle`, 22 sp x 22 sp, centred, content description `label_downloaded`) when downloaded, followed by a space; scanlator (`bodySmall`, `maxLines = 2`, ellipsis, `secondaryItemAlpha`, padding top 2 dp) if non-null.
- Preview fixtures (not runtime behaviour): `FakeChapter`, `FakeGapChapter`, `FakeChapterLongTitle` (`:290-308`).

OBSERVED: the chapter gap rule uses `isRecognizedNumber` on both chapters (section 5.4). Unknown numbers never warn.

---

## 15. Reader chrome: app bars, bottom bar, chapter navigator, mode dialogs

### 15.1 `ReaderAppBars` (`P/appbars/ReaderAppBars.kt:37-194`)

- `isRtl = viewer is R2LPagerViewer` (`:69`).
- `backgroundColor = colorScheme.surfaceColorAtElevation(3.dp).copy(alpha = if (isSystemInDarkTheme()) 0.9f else 0.95f)` (`:70-72`). The activity toolbar colour uses the same alpha pair (`ReaderActivity.kt:498-507`).
- `modifierWithInsetsPadding = Modifier.systemBarsPadding()` only when `fullscreen` (`:74-78`).
- Top: `AnimatedVisibility(visible, enter = slideInVertically(initialOffsetY = { -it }, tween(200)), exit = slideOutVertically(targetOffsetY = { -it }, tween(200)))` (`:84-94`, `animationSpec = tween<IntOffset>(200)`, `:35`). Contains `AppBar(title = mangaTitle, subtitle = chapterTitle, navigateUp, backgroundColor, modifier.clickable(onClickTopAppBar))`.
  - Actions: one `AppBar.Action` for the bookmark (title `action_remove_bookmark` "Unbookmark chapter" when bookmarked, else `action_bookmark` "Bookmark chapter"; icon `Bookmark` or `BookmarkBorder`; always shown).
  - Overflow (`AppBar.OverflowAction`), in this order, each only when its callback is non-null (HTTP sources only): `action_open_in_web_view` ("Open in WebView"), `action_open_in_browser` ("Open in browser"), `action_share` ("Share").
- Bottom: `Spacer(weight 1)` above; `AnimatedVisibility` slide from `initialOffsetY = { it }` / `targetOffsetY = { it }`, same 200 ms spec, containing a column (`spacedBy(MaterialTheme.padding.small)`): `ChapterNavigator` then `BottomReaderBar`.

### 15.2 `ChapterNavigator` (`P/components/ChapterNavigator.kt:45-149`)

- `horizontalPadding = if (isTabletUi()) 24.dp else 8.dp` (`P/components/ChapterNavigator.kt:56-57`). `isTabletUi` is imported from `eu.kanade.presentation.util` (`:40`); its definition is outside this scope.
- Whole row forced to LTR (`CompositionLocalProvider(LocalLayoutDirection provides Ltr)`, `:71`). Only the slider's inner row uses the reader direction (`:92`).
- Left button: `FilledIconButton(enabled = if (isRtl) enabledNext else enabledPrevious, onClick = if (isRtl) onNextChapter else onPreviousChapter)`. Icon is ALWAYS `Icons.Outlined.SkipPrevious`. Content description: `action_next_chapter` if `isRtl`, else `action_previous_chapter`.
- Middle (only when `totalPages > 1`, else `Spacer(weight 1)`):
  - `Row(weight 1, clip RoundedCornerShape(24.dp), background backgroundColor, padding horizontal 16.dp)` in the reader direction.
  - Current page text (`currentPage.toString()`), with an invisible (transparent) copy of `totalPages.toString()` to keep the width stable.
  - `Slider(value = currentPage, valueRange = 1..totalPages, onValueChange = { if (it == currentPage) return; onPageIndexChange(it - 1) })`, `padding horizontal 8.dp`.
  - Haptic: `HapticFeedbackType.TextHandleMove` on `currentPage` change while the slider is dragged (`:107-113`).
  - Total page text.
- Right button: `enabled = if (isRtl) enabledPrevious else enabledNext`, `onClick = if (isRtl) onPreviousChapter else onNextChapter`. Icon ALWAYS `Icons.Outlined.SkipNext`. Content description: `action_previous_chapter` if `isRtl`, else `action_next_chapter`.
- OBSERVED: in RTL, the left button shows a "skip previous" icon while performing "next chapter". The icons are not mirrored.
- OBSERVED: `currentPage` can be `-1` before the first `onPageSelected` (`ReaderViewModel.State.currentPage` default). Then the slider value is outside `valueRange`.

### 15.3 `BottomReaderBar` (`P/appbars/BottomReaderBar.kt:24-71`)

`Row(fillMaxWidth, background backgroundColor, padding 8.dp, SpaceEvenly, CenterVertically)` with four `IconButton`s in order:
1. Reading mode: `painterResource(readingMode.iconRes)`, content description `viewer` ("Reading mode"). `readingMode` is the RAW manga value (`getMangaReadingMode(resolveDefault = false)`, `ReaderActivity.kt:416-418`), so a manga on "Default" shows the Default icon.
2. Orientation: `orientation.icon` (vector), description `rotation_type` ("Rotation"). Also the raw value (`ReaderActivity.kt:420-422`).
3. Crop: `R.drawable.ic_crop_24dp` when `cropEnabled`, else `R.drawable.ic_crop_off_24dp`; description `pref_crop_borders` ("Crop borders").
4. Settings: `Icons.Outlined.Settings`, description `action_settings` ("Settings").

### 15.4 `ReadingModeSelectDialog` (`P/ReadingModeSelectDialog.kt:31-81`)

- `readingMode = ReadingMode.fromPreference(manga?.readingMode?.toInt())` (raw, `remember(manga)`).
- `AdaptiveSheet(onDismissRequest)` containing `ModeSelectionDialog` (`onUseDefault` = set DEFAULT, shown only when the current mode is not DEFAULT; `onApply` = apply the selected mode).
- Grid `SettingsIconGrid(MR.strings.pref_category_reading_mode)` over `ReadingMode.entries - DEFAULT`: `LEFT_TO_RIGHT`, `RIGHT_TO_LEFT`, `VERTICAL`, `WEBTOON`, `CONTINUOUS_VERTICAL`. Each `IconToggleButton(checked = mode == selected, icon = vectorResource(mode.iconRes), title = stringResource(mode.stringRes))`.
- The selection is local state (`remember { mutableStateOf(readingMode) }`); the change is only persisted on "Apply" (`onChangeReadingMode(selected)`, then `onChange(stringRes)` for the toast, then dismiss). Tapping a tile does NOT apply.

### 15.5 `OrientationSelectDialog` (`P/OrientationSelectDialog.kt:29-83`)

Identical structure with `ReaderOrientation.entries - DEFAULT` (`FREE`, `PORTRAIT`, `LANDSCAPE`, `LOCKED_PORTRAIT`, `LOCKED_LANDSCAPE`, `REVERSE_PORTRAIT`). Title `rotation_type`. Icons from the enum. The toast uses `stringRes` on every apply (`:45-48`). `onUseDefault` is set to `DEFAULT` when the current value is not DEFAULT.

### 15.6 `ModeSelectionDialog` (`P/components/ModeSelectionDialog.kt:28-68`)

`Box(padding vertical 16.dp)` > `Column` > content, then `Row(padding horizontal SettingsItemsPaddings.Horizontal)`: `OutlinedButton(action_revert_to_default)` (only if `onUseDefault != null`), `Spacer(weight 1)`, `FilledTonalButton(onApply)` with `Icons.Outlined.Check` and `action_apply` ("Apply").

### 15.7 `ReaderPageActionsDialog` (`P/ReaderPageActionsDialog.kt:28-110`)

- `AdaptiveSheet(onDismissRequest)` with a `Row(padding vertical 16.dp, spacedBy(padding.small))` of four `ActionButton(weight 1)`:
  1. `set_as_cover` ("Set as cover"), `Icons.Outlined.Photo`: opens `SetCoverDialog`. The sheet is NOT dismissed.
  2. `action_copy_to_clipboard` ("Copy to clipboard"), `Icons.Outlined.ContentCopy`: `onShare(true)` then dismiss.
  3. `action_share` ("Share"), `Icons.Outlined.Share`: `onShare(false)` then dismiss.
  4. `action_save` ("Save"), `Icons.Outlined.Save`: `onSave()` then dismiss.
- `SetCoverDialog` (`:89-110`): `AlertDialog` with text `confirm_set_image_as_cover` ("Use this image as cover art?"), confirm `action_ok` (runs `onSetAsCover()` and hides the confirm dialog), dismiss `action_cancel` (hides it).
- OBSERVED: after confirming, the page-actions sheet remains open (only the confirm dialog closes). The result toast appears on top.

---

## 16. Overlays: page indicator, content overlay, navigation overlay, flash

### 16.1 `PageIndicatorText` (`P/PageIndicatorText.kt:17-50`)

- Returns nothing when `currentPage <= 0 || totalPages <= 0`.
- Text `"$currentPage / $totalPages"`.
- Fill: `Color(235,235,235)`, `fontSize = bodySmall`, `FontWeight.Bold`, `letterSpacing = 1.sp`. Outline: same text with `Color(45,45,45)` and `drawStyle = Stroke(width = 4f)`, drawn underneath.
- Rendered (`ReaderActivity.kt:340-350`) only when `!menuVisible && showPageNumber` (default true). It is in the `pageNumber` compose slot, not inside the menu.

### 16.2 `ReaderContentOverlay` (`P/ReaderContentOverlay.kt:15-49`)

- `brightness < 0`: a full-size `Canvas` filled with `Color.Black`, `graphicsLayer { alpha = abs(brightness)/100f }` (remember keyed on brightness). Range of brightness is -75..100 in practice (section 19.5).
- `color != null`: a full-size `Canvas` with `drawRect(color = Color(color), blendMode = colorBlendMode ?: SrcOver)`.
- Order in the activity: this overlay is composed BEFORE the app bars (so the bars are not dimmed) (`ReaderActivity.kt:383-387`).

### 16.3 `ReaderNavigationOverlayView` (`R/ReaderNavigationOverlayView.kt:19-116`)

A plain Android `View` drawn on top of the viewer. Behaviour:
- `setNavigation(navigation, showOnStart)`: `firstLaunch = (this.navigation == null)`; store; `invalidate()`. Then the fade-in is skipped when `isVisible` already, or `(!showOnStart && firstLaunch)`, or `navigation is DisabledNavigation`. Otherwise `animate().alpha(1f).setDuration(1000).withStartAction { isVisible = true }.withEndAction { viewPropertyAnimator = null }`.
- `showOnStart` comes from `showNavigationOverlayOnStart` (default false, `ReaderPreferences.kt:170-173`). `showNavigationOverlayNewUser` (default true, `:165-168`) is read by the viewer, not here.
- `onDraw` (`:65-90`): for each region of `navigation.getRegions()`:
  1. `canvas.withScale(width, height) { regionPaint.color = region.type.color; drawRect(region.rectF, regionPaint) }`. Region rectangles are in 0..1 normalised coordinates.
  2. At the region's top-left, `x = width * |left - right| / 2`, `y = height * |top - bottom| / 2`. Draws `context.stringResource(region.type.nameRes)` twice: first with `textBorderPaint` (black, stroke 8f, size 64f), then with `textPaint` (white, fill, size 64f), centred.
- `performClick()` (`:92-107`): calls super; if no animator is running and the view is visible, fade out `alpha(0f)` over 1000 ms, then `isVisible = false`.
- `onTouchEvent` (`:109-113`): calls `performClick()` on EVERY touch event (down, move, up), then `super.onTouchEvent`. Any touch starts the fade-out. The return value is `super`, so the touch is not consumed by this view when it is not clickable.
- Constant `FADE_DURATION = 1000L` (`:116`).

Region names and colours come from `ViewerNavigation` / `ViewerNavigation.getRegions()` (area:44-reader-viewers). The label keys are the `nameRes` of the region types.

### 16.4 `DisplayRefreshHost` and the flash (`P/DisplayRefreshHost.kt`)

Class (`:21-47`):
- `currentDisplayRefresh` (Compose `mutableStateOf(false)`).
- `flashMillis = readerPreferences.flashDurationMillis()` (default 100 ms, `ReaderPreferences.kt:20`), `flashMode = flashColor()` (default `BLACK`, `:24`), `flashIntervalPref = flashPageInterval()` (default 1, `:22`).
- `flashInterval` (read once at construction) and `timesCalled = 0`.
- `flash()`: `if (timesCalled % flashInterval == 0) currentDisplayRefresh = true`; then `timesCalled += 1`. So with interval 1 every page flashes; with 3, pages 0, 3, 6, ... flash (counting from the first call).
- `setInterval(interval)`: sets the interval and resets `timesCalled = 0`.

Composable `DisplayRefreshHost(hostState, modifier)` (`:49-90`):
- Collects duration, mode and interval from the preferences.
- `LaunchedEffect(currentDisplayRefresh)`: if false, `currentColor = null` and return. If true: `half = refreshDuration.milliseconds / 2`; `currentColor = if (mode == BLACK) Color.Black else Color.White`; `delay(half)`; if mode is `WHITE_BLACK`, `currentColor = Color.Black`; `delay(half)`; `hostState.currentDisplayRefresh = false`.
  - Result: BLACK = black for the whole duration; WHITE = white for the whole duration; WHITE_BLACK = white for the first half, then black.
- `LaunchedEffect(flashInterval) { hostState.setInterval(flashInterval) }`.
- Draws `drawRect(currentColor)` over `fillMaxSize` when `currentColor != null`. The overlay is a full-screen solid colour, not a translucent one.
- Composed only when `flashOnPageChange` is true (`ReaderActivity.kt:433-437`).

OBSERVED edge: `flash()` is called on every `PageChanged` (`ReaderActivity.kt:224-226`) even when the composable is not present. The flag then stays `true`, and the first time the feature is enabled the composable starts with a pending flash. Proposal: reset the flag when the feature is off.

---

## 17. `SaveImageNotifier` (`R/SaveImageNotifier.kt:25-96`)

OBSERVED:
- Channel `Notifications.CHANNEL_COMMON`, id `Notifications.ID_DOWNLOAD_IMAGE` (`:27-28`).
- `onComplete(uri)` (`:35-46`): loads the image through Coil with `memoryCachePolicy = DISABLED` and `size(720, 1280)`; on success `showCompleteNotification(uri, bitmap)`; on error `onError(null)`.
- `onClear()`: cancel the notification id (`:51-53`).
- `onError(error: String?)` (`:59-67`): title `download_notifier_title_error` ("Error"), text `error ?: unknown_error` ("Unknown error"), small icon `android.R.drawable.ic_menu_report_image`, notify.
- `showCompleteNotification` (`:69-90`): title `picture_saved`; small icon `R.drawable.ic_photo_24dp`; `BigPictureStyle` with the bitmap (when decoded); large icon = bitmap; `autoCancel(true)`; clear actions; content intent `NotificationHandler.openImagePendingActivity(context, uri)`; one action `R.drawable.ic_share_24dp` titled `action_share` with `NotificationReceiver.shareImagePendingBroadcast(context, uri)`; notify.

Linux replacement is in the design file (GNotification with `app.share-image`).

---

## 18. Brightness and colour filter: model and math

- Custom brightness value range: `-75..100` (slider `valueRange = -75..100`, `P/settings/ColorFilterPage.kt:41`), step 0 (continuous).
- Mapping (`ReaderActivity.kt:954-968`):
  - `v > 0`: window brightness `v / 100f` (0.01..1.0).
  - `v < 0`: window brightness `0.01f` (minimum) AND a black overlay with alpha `|v| / 100f` (`ReaderContentOverlay`, section 16.2). Range of overlay alpha: 0.01..0.75.
  - `v == 0`: system brightness (`BRIGHTNESS_OVERRIDE_NONE`), no overlay.
- Colour filter: four 0..255 channels (`colorFilterValue` as ARGB Int, default 0). Setter (`ColorFilterPage.kt:127-129`): `(channel shl shift) or (current and mask.inv())` with masks `ALPHA 0xFF000000 (shift 24)`, `RED 0x00FF0000 (16)`, `GREEN 0x0000FF00 (8)`, `BLUE 0x000000FF (0)` (`:130-133`). Verified: `mask.inv().toInt()` keeps only the other three bytes.
- Colour filter blend modes (`ReaderPreferences.ColorFilterMode`, `ReaderPreferences.kt:236-253`), index into the stored `color_filter_mode` (default 0):
  - 0 `label_default` -> `SrcOver`
  - 1 `filter_mode_multiply` -> `Modulate`
  - 2 `filter_mode_screen` -> `Screen`
  - 3 `filter_mode_overlay` -> `Overlay`, 4 `filter_mode_lighten` -> `Lighten`, 5 `filter_mode_darken` -> `Darken`. Indices 3-5 are added only when `Build.VERSION.SDK_INT >= P` (`ReaderPreferences.kt:244-252`). The desktop port has no SDK gate and offers all six.
  - 4 `filter_mode_lighten` -> `Lighten`
  - 5 `filter_mode_darken` -> `Darken`

---

## 19. Reader settings UI logic

### 19.1 `ReaderSettingsDialog` (`P/settings/ReaderSettingsDialog.kt:22-71`)

- Tabs (`persistentListOf`): `pref_category_reading_mode` ("Reading mode"), `pref_category_general` ("General"), `custom_filter` ("Custom filter").
- Dialog max height `0.75 * maxHeight`.
- Dim and chrome: when the pager is on page index 2 (custom filter), dim amount `0f` and `onHideMenus()`; otherwise dim `0.5f` and `onShowMenus()`. The dialog's dismiss callback calls `onDismissRequest()` then `onShowMenus()`.
- Pages: `0` -> `ReadingModePage`, `1` -> `GeneralPage`, `2` -> `ColorFilterPage`. Each in `Column(padding vertical TabbedDialogPaddings.Vertical, verticalScroll)`.

### 19.2 `ReadingModePage` (`P/settings/ReadingModePage.kt:27-68`)

- `HeadingItem(pref_category_for_this_series)`.
- Reading-mode chips (`SettingsChipRow(pref_category_reading_mode)`): one `FilterChip` per `ReadingMode.entries` (INCLUDING `DEFAULT`), `selected = it == readingMode` (raw manga value), `onClick = screenModel.onChangeReadingMode(it)` (applies immediately, unlike the bottom sheet).
- Orientation chips (`rotation_type`): one per `ReaderOrientation.entries` (INCLUDING `DEFAULT`), `onChangeOrientation(it)` immediately.
- If `viewer is WebtoonViewer` (`WEBTOON` or `CONTINUOUS_VERTICAL`) -> `WebtoonViewerSettings`; else `PagerViewerSettings`.

`PagerViewerSettings` (`:70-145`):
- `HeadingItem(pager_viewer)`.
- `TapZonesItems(navigationModePager, pagerNavInverted)`.
- Scale type chips over `ImageScaleType` (6 entries) stored as `index + 1` in `imageScaleType` (default `1` = fit screen, `ReaderPreferences.kt:53`).
- Zoom start chips over `ZoomStart` (4 entries), stored as `index + 1` in `zoomStart` (default `1` = automatic, `:55`).
- Checkboxes: `pref_crop_borders` (`cropBorders`, default false), `pref_landscape_zoom` (`landscapeZoom`, default true), `pref_navigate_pan` (`navigateToPan`, default true).
- `pref_dual_page_split` checkbox is BOUND TO `dualPageInvertPaged()` (key `pref_dual_page_invert`), not to `dualPageSplitPaged()` (key `pref_dual_page_split`) (`:120-124`). OBSERVED BUG: the "Split wide pages" control writes the invert flag.
- If `dualPageSplitPaged` is true: checkbox `pref_dual_page_invert` ("Invert split page placement") bound to `dualPageRotateToFitInvert()` (`pref_dual_page_rotate_invert`) (`:126-131`). This control also writes the same key as the "Flip orientation" control below.
- `pref_page_rotate` checkbox bound to `dualPageRotateToFit()` (`:133-137`). If true: `pref_page_rotate_invert` bound to `dualPageRotateToFitInvert()` (`:139-144`).
- The split toggle that is correct on the settings screen is `SettingsReaderScreen.kt:213` (it reads `dualPageSplitPaged`). The key mismatch above is only in this dialog.

`WebtoonViewerSettings` (`:147-213`):
- `HeadingItem(webtoon_viewer)`; `TapZonesItems(navigationModeWebtoon, webtoonNavInverted)`.
- `SliderItem(webtoonSidePadding, valueRange = 0..25, label pref_webtoon_side_padding, valueText = NumberFormat.getPercentInstance().format(value / 100f))`.
- `pref_crop_borders` -> `cropBordersWebtoon`.
- `pref_dual_page_split` -> `dualPageSplitWebtoon` (correct key); if on, `pref_dual_page_invert` -> `dualPageInvertWebtoon`.
- `pref_page_rotate` -> `dualPageRotateToFitWebtoon`; if on, `pref_page_rotate_invert` -> `dualPageRotateToFitInvertWebtoon`.
- `pref_double_tap_zoom` -> `webtoonDoubleTapZoomEnabled` (default true). `pref_webtoon_disable_zoom_out` -> `webtoonDisableZoomOut` (default false).

`TapZonesItems` (`:215-243`):
- Chips over `TapZones` (6 entries, index 0..5) stored in `reader_navigation_mode_pager` / `_webtoon` (default 0): `label_default`, `l_nav`, `kindlish_nav`, `edge_nav`, `right_and_left_nav`, `disabled_nav`.
- If the selected index is NOT 5 (disabled), a second chip row `pref_read_with_tapping_inverted` over `TappingInvertMode.entries`: `NONE` (`tapping_inverted_none`), `HORIZONTAL`, `VERTICAL`, `BOTH` (the `tapping_inverted_*` keys). Stored by enum name (`getEnum`, default `NONE`, `ReaderPreferences.kt:155-163`).

### 19.3 `GeneralPage` (`P/settings/GeneralSettingsPage.kt:33-127`)

- Background chips over `themes` (in this order): `black_background` -> 1, `gray_background` -> 2, `white_background` -> 0, `automatic_background` -> 3. Stored in `pref_reader_theme_key` (default 1, black).
- Checkboxes: `pref_show_page_number` (default true); `pref_fullscreen` (default true); `pref_cutout_short` (only when `hasDisplayCutout && fullscreen`; default true); `pref_keep_screen_on` (default true); `pref_read_with_long_tap` (default true); `pref_always_show_chapter_transition` (default true); `pref_page_transitions` (default true); `pref_flash_page` (default false).
- When flash is on:
  - `SliderItem(value = flashMillis / 100, valueRange = 1..15, label pref_flash_duration, valueText = pref_flash_duration_summary (raw ms), onChange = set(it * 100))`. Stored value is in MILLISECONDS (100..1500).
  - `SliderItem(value = flashInterval, valueRange = 1..10, label pref_flash_page_interval, valueText = plural pref_pages)`.
  - Chips `pref_flash_with`: `pref_flash_style_black` (BLACK), `pref_flash_style_white` (WHITE), `pref_flash_style_white_black` (WHITE_BLACK).

### 19.4 `ColorFilterPage` (`P/settings/ColorFilterPage.kt:24-125`)

- `pref_custom_brightness` checkbox (`customBrightness`, default false). When on: `SliderItem(customBrightnessValue, valueRange = -75..100, steps = 0, label pref_custom_brightness)`.
- `pref_custom_color_filter` checkbox (`colorFilter`, default false). When on: R/G/B/A sliders `0..255` (`color_filter_r_value`, `_g_value`, `_b_value`, `_a_value`), then chips `pref_color_filter_mode` over the six modes (section 18), default index 0.
- `pref_grayscale` (default false) and `pref_inverted_colors` (default false) checkboxes, always visible.

### 19.5 Value ranges summary

| Setting | Range | Step | Default | Stored as |
|---|---|---|---|---|
| Custom brightness | -75..100 | 1 | 0 | Int |
| Colour channel R/G/B/A | 0..255 | 1 | 0 (ARGB) | Int (packed) |
| Flash duration | 100..1500 ms (slider 1..15 x 100) | 100 ms | 100 ms | Int ms |
| Flash interval | 1..10 pages | 1 | 1 | Int |
| Webtoon side padding | 0..25 (shown as percent of value/100) | 1 | 0 | Int |
| Tap zone | 0..5 | — | 0 | Int |
| Image scale type | 1..6 | — | 1 | Int |
| Zoom start | 1..4 | — | 1 | Int |

---

## 20. User-visible message keys (complete for this scope)

Keys referenced by the 32 files (text verified in `i18n/src/commonMain/moko-resources/base/strings.xml` where noted; a key listed without text is referenced but its English text was not re-read here and must be resolved by area:02-preferences-system and area:47-settings-screens-general when the strings table is ported):

Errors and load states:
- `page_list_empty_error` = "No pages found" (`RL/ChapterLoader.kt:54`)
- `source_not_installed` = "Source not installed: %1$s" (`:107`)
- `loader_not_implemented_error` = "Source not found" (`:109`)
- `unknown_error` = "Unknown error" (`SaveImageNotifier.kt:63`)
- `download_notifier_title_error` = "Error" (`SaveImageNotifier.kt:62`)
- `loading` = "Loading…" (`ReaderActivity.kt:451`)
- `on` = "On", `off` = "Off" (`ReaderActivity.kt:428`)
- "Unknown err" is a literal string in `ReaderActivity.kt:181`, not a key (not localised).

Transitions:
- `transition_previous` = "Previous:", `transition_next` = "Next:", `transition_finished` = "Finished:", `transition_current` = "Current:"
- `transition_no_previous` = "There's no previous chapter", `transition_no_next` = "There's no next chapter"
- `missing_chapters_warning` (plural): one = "Skipping %d chapter, either the source is missing it or it has been filtered out"; other = "Skipping %d chapters, either the source is missing them or they have been filtered out"
- `label_downloaded` = "Downloaded"

Page actions and notifications:
- `picture_saved` = "Picture saved"
- `share_page_info` = "%1$s: %2$s, page %3$d"
- `cover_updated` = "Cover updated"
- `notification_first_add_to_library` = "Please add the entry to your library before doing this"
- `notification_cover_update_failed` = "Failed to update cover"
- `set_as_cover` = "Set as cover"
- `confirm_set_image_as_cover` = "Use this image as cover art?"
- `action_ok`, `action_cancel`, `action_save` = "Save", `action_share` = "Share", `action_copy_to_clipboard` = "Copy to clipboard"

App bars and navigation:
- `action_bookmark` = "Bookmark chapter", `action_remove_bookmark` = "Unbookmark chapter"
- `action_open_in_web_view` = "Open in WebView", `action_open_in_browser` = "Open in browser"
- `action_next_chapter` = "Next chapter", `action_previous_chapter` = "Previous chapter"
- `viewer` = "Reading mode" (content description), `rotation_type` = "Rotation", `pref_crop_borders` = "Crop borders", `action_settings` = "Settings"
- `action_apply` = "Apply", `action_revert_to_default` = "Revert to default"

Rotation labels (`ReaderOrientation.stringRes`): `label_default` ("Default"), `rotation_free` ("Free"), `rotation_portrait` ("Portrait"), `rotation_landscape` ("Landscape"), `rotation_force_portrait` ("Locked portrait"), `rotation_force_landscape` ("Locked landscape"), `rotation_reverse_portrait` ("Reverse portrait").

Settings labels: `pref_category_reading_mode` ("Reading mode"), `pref_category_general` ("General"), `custom_filter` ("Custom filter"), `pref_category_for_this_series` ("For this series"), `pager_viewer` ("Paged"), `webtoon_viewer` ("Long strip"), `pref_viewer_nav` ("Tap zones"), `pref_read_with_tapping_inverted` ("Invert tap zones"), `pref_image_scale_type` ("Scale type"), `pref_zoom_start` ("Zoom start position"), `pref_landscape_zoom` ("Automatically zoom into wide images"), `pref_navigate_pan` ("Pan wide images"), `pref_dual_page_split` ("Split wide pages"), `pref_dual_page_invert` ("Invert split page placement"), `pref_page_rotate` ("Rotate wide pages to fit"), `pref_page_rotate_invert` ("Flip orientation of rotated wide pages"), `pref_webtoon_side_padding` ("Side padding"), `pref_double_tap_zoom` ("Double tap to zoom"), `pref_webtoon_disable_zoom_out` ("Disable zoom out"), `pref_reader_theme` ("Background color"), `pref_show_page_number` ("Show page number"), `pref_fullscreen` ("Fullscreen"), `pref_cutout_short` ("Show content in cutout area"), `pref_keep_screen_on` ("Keep screen on"), `pref_read_with_long_tap` ("Show actions on long tap"), `pref_always_show_chapter_transition` ("Always show chapter transition"), `pref_page_transitions` ("Animate page transitions"), `pref_flash_page` ("Flash on page change"), `pref_flash_duration` ("Flash duration"), `pref_flash_duration_summary` ("%1$s ms"), `pref_flash_page_interval` ("Flash every"), `pref_flash_with` ("Flash with"), `pref_flash_style_black` ("Black"), `pref_flash_style_white` ("White"), `pref_flash_style_white_black` ("White and Black"), `pref_pages` (plural: one "1 page", other "%1$s pages"), `black_background` ("Black"), `gray_background` ("Gray"), `white_background` ("White"), `automatic_background` ("Auto"), `pref_custom_brightness` ("Custom brightness"), `pref_custom_color_filter` ("Custom color filter"), `color_filter_r_value` ("Red"), `color_filter_g_value`, `color_filter_b_value`, `color_filter_a_value` (text not re-read), `pref_color_filter_mode` ("Color filter blend mode"), `filter_mode_multiply` ("Multiply"), `filter_mode_screen` ("Screen"), `filter_mode_overlay` ("Overlay"), `filter_mode_lighten` ("Dodge / Lighten"), `filter_mode_darken` ("Burn / Darken"), `pref_grayscale` ("Grayscale"), `pref_inverted_colors` ("Inverted"), `tapping_inverted_none` ("None"), `tapping_inverted_horizontal` ("Horizontal"), `tapping_inverted_vertical` ("Vertical"), `tapping_inverted_both` ("Both"), `label_default` ("Default"), `l_nav` ("L shaped"), `kindlish_nav` ("Kindle-ish"), `edge_nav` ("Edge"), `right_and_left_nav` ("Right and Left"), `disabled_nav` ("Disabled"), `scale_type_fit_screen` ("Fit screen"), `scale_type_stretch` ("Stretch"), `scale_type_fit_width` ("Fit width"), `scale_type_fit_height` ("Fit height"), `scale_type_original_size` ("Original size"), `scale_type_smart_fit` ("Smart fit"), `zoom_start_automatic` ("Automatic"), `zoom_start_left` ("Left"), `zoom_start_right` ("Right"), `zoom_start_center` ("Center").

Viewer names (`ReadingMode.stringRes`, cross-area): `left_to_right_viewer` ("Paged (left to right)"), `right_to_left_viewer`, `vertical_viewer`, `vertical_plus_viewer`, `webtoon_viewer` (text not re-read for the first three).

Other: `action_share` for the sharing chooser; `label_default` for "Default" options.

---

## 21. Timings, limits and constants (complete)

| Name | Value | Where |
|---|---|---|
| Preload-ahead pages (HTTP) | 4 | `RL/HttpPageLoader.kt:48` |
| Image download concurrency per chapter | 1 | `RL/HttpPageLoader.kt:55-63` |
| Queue priorities | preload 0, visible 1, manual retry 2 | `:110`, `:173`, `:132` |
| Page-list and image disk cache | `DiskLruCache`, size `100L*1024*1024` (100 MiB), app version 1, value count 1, directory `cacheDir/chapter_disk_cache` | `app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt:37-39`, `:203-209`, `:30-35` |
| Page list key | md5(`"${mangaId}${url}"`) | `ChapterCache.kt:197-199` |
| Image key | md5(imageUrl), file `md5 + ".0"` | `ChapterCache.kt:130-134` |
| Download-ahead threshold | `number / size > 0.25` | `R/ReaderViewModel.kt:454` |
| Download-ahead default | 0 (off) | `DownloadPreferences.kt:25` |
| Remove-after-read default | -1 (off) | `DownloadPreferences.kt:28` |
| Share/clipboard cache | cleared before each share | `R/ReaderViewModel.kt:864` |
| Filename byte cap | 250 bytes, minus suffix | `DiskUtil.kt:143`, `R/ReaderViewModel.kt:761-762` |
| Notification bitmap request size | 720 x 1280 | `R/SaveImageNotifier.kt:39` |
| Brightness sample period | 100 ms | `R/ReaderActivity.kt:940` |
| Chrome slide animation | 200 ms tween | `P/appbars/ReaderAppBars.kt:35` |
| Navigation overlay fade | 1000 ms | `R/ReaderNavigationOverlayView.kt:116` |
| Flash default | 100 ms total, half/half | `ReaderPreferences.kt:20`, `P/DisplayRefreshHost.kt:67-77` |
| Flash interval default | 1 | `ReaderPreferences.kt:22` |
| Chapter gap warning | > 0 missing | `P/ChapterTransition.kt:120` |
| Transition card width | 460 dp max | `P/ChapterTransition.kt:100` |
| Chapter name max lines | 5 (transition), 2 (scanlator) | `P/ChapterTransition.kt:239, 264` |
| Tablet horizontal padding (navigator) | 24 dp, phone 8 dp | `P/components/ChapterNavigator.kt:57` |
| Page indicator outline | width 4f | `P/PageIndicatorText.kt:34` |
| Brightness window range | 0.01..1.0 | `R/ReaderActivity.kt:956-964` |
| Overlay alpha | abs(v)/100 | `P/ReaderContentOverlay.kt:24` |
| Gray background | rgb(0x20,0x21,0x25) | `R/ReaderActivity.kt:834` |
| Toolbar alpha | 230 dark / 242 light (of 255) | `R/ReaderActivity.kt:498-501` |
| Toast on crop, reading mode, orientation | `toast(stringRes)` | `R/ReaderActivity.kt:427-428`, `:470-472`, `:481-482` |
| Webtoon padding | 0..25 | `ReaderPreferences.kt:206-207` |
| MILLI_CONVERSION | 100 | `ReaderPreferences.kt:209` |

Android toast duration is not in this scope; `toast(...)` is defined in `app/src/main/java/eu/kanade/tachiyomi/util/system/` (area:12-network-stack and area:13-core-util-storage-archive-torrent). Not verified here.

---

## 22. Concurrency and dispatchers (complete for this scope)

| Code | Thread/dispatcher | Notes |
|---|---|---|
| `ChapterLoader.loadChapter` | `withIOContext` (IO) | writes `ReaderChapter.state` (a `MutableStateFlow`, thread-safe) |
| `HttpPageLoader` consumer | `Dispatchers.IO`, `runInterruptible { queue.take() }` | one coroutine per loader |
| `HttpPageLoader.loadPage` | `withIOContext` | never resumes normally; cancelled by the viewer |
| `HttpPageLoader.recycle` cache write | `launchIO` (GlobalScope on IO) | survives the loader's cancelled scope |
| `ReaderViewModel.init` | `withIOContext` | `chapterList` lazily built with `runBlocking` here |
| `ReaderViewModel.loadChapter` state commit | `withUIContext` (Main) | `mutableState.update` lambda has side effects (see 6.1) |
| `ReaderViewModel.loadNewChapter`, `downloadNextChapters` | `viewModelScope.launchIO` | |
| `launchNonCancellable` | `launchIO { withContext(NonCancellable, block) }` | exceptions escape to the viewModelScope and crash (see 10.3) |
| `setMangaReadingMode` | `runBlocking(Dispatchers.IO)` on the UI thread | blocks the UI; contains a rendezvous send (see 12.9) |
| `setMangaOrientationType` | `viewModelScope.launchIO` | |
| `ReaderViewModel.preload` | called from `lifecycleScope.launchIO` (activity) | |
| `ReaderActivity` observers | `lifecycleScope` (Main) | |
| `ReaderActivity.setChapters` assistUrl | `lifecycleScope.launchIO` | |
| Compose effects in `DisplayRefreshHost` | Compose `LaunchedEffect` (Main) | `delay` for the flash |
| Brightness sample | `Flow.sample(100)` on Main | |

OBSERVED: `ReaderViewModel.State` is shared between the main thread (activity, Compose) and IO (loader callbacks, `updateChapterProgress`). All updates use `MutableStateFlow.update`, but some update lambdas have side effects (section 6.1), and `ReaderChapter.state` is written from IO and read on Main.

---

## 23. Cross-area interactions (this scope calls into)

| Area | What this scope uses |
|---|---|
| area:02-preferences-system | `ReaderPreferences`, `LibraryPreferences`, `DownloadPreferences`, `BasePreferences`, `TrackPreferences`, `SourcePreferences` (defaults in `p1-data-and-config.md`) |
| area:03-db-manga-schema-migrations and area:05-data-repos-entries-items and area:07-domain-entries-items | `getChaptersByMangaId`, `updateChapter`, `ChapterUpdate`, `getManga`, the chapter model and `toDbChapter`/`toDomainChapter` |
| area:08-domain-library-category-history-updates | `UpsertMangaHistory`, `MangaHistoryUpdate`, `GetNextChapters` (history package) |
| area:09-domain-source-track-download-release-misc | `MangaDownloadManager`, `MangaDownloadProvider`, `MangaDownloadCache`, `enqueueChaptersToDelete`, `buildPageList`, `addDownloadsToStartOfQueue` |
| owner not determined | `DataSaver` (`app/src/main/java/aniyomi/util/DataSaver.kt`); closest candidate area:12-network-stack, not confirmed |
| area:12-network-stack | network helpers used by the reader (`toast`, `openInBrowser`, `toShareIntent` are in `app/src/main/java/eu/kanade/tachiyomi/util/system/`, location not re-read) |
| area:13-core-util-storage-archive-torrent | `ArchiveReader` / `EpubReader`, `ImageUtil`, `DiskUtil`, `takeBytes`, natural sort, `ZipWriter` not used here |
| area:14-source-local-and-metadata | `LocalMangaSource.getFormat`, `Format` sealed type, `isLocal` |
| area:15-source-api-manga | `HttpSource.getPageList`, `getImageUrl`, `getImage`, `getChapterUrl`, `StubMangaSource` |
| area:17-source-app-layer | `MangaSourceManager.getOrStub`, `isInitialized` |
| area:27-tracking-core | `TrackChapter` |
| area:34-images-covers-cache | cover editing (`editCover`) and cover storage |
| area:36-ui-shell-navigation-security-webview | `WebViewActivity`, `registerSecureActivity`, `MainActivity` shortcut contract |
| area:44-reader-viewers | viewers (`Viewer`, `PagerViewer`, `WebtoonViewer`), `ViewerNavigation`, `ViewerConfig`, `ReaderProgressIndicator`, `moveToPage`, `handleKeyEvent`, `handleGenericMotionEvent`, `setChapters` |
| area:47-settings-screens-general | `SettingsReaderScreen` (the reader settings screen that also edits these prefs; reads `dualPageSplitPaged` at `SettingsReaderScreen.kt:213`) |
