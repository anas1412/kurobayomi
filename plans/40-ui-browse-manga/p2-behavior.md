# Area 40 (part 2): behavior spec, manga browse models

All paths are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` = what the Kotlin does (file:line). `Proposal:` = what Kurobayomi should do. Message keys are `MR.strings.<key>` (core i18n) or `AYMR.strings.<key>` (aniyomi i18n). English text is from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (section 16).

Part 1 (`p1-*.md`) owns the presentation of the same screens (rows, toolbars, dialogs, layouts). This part owns the models, state machines, algorithms, data writes and the filter tree. Where part 1 already specifies a widget, this file only refers to it.

Scope file set (12 files, 1749 LOC, from `areas.py files 40-ui-browse-manga 2`):

| # | File | LOC |
|---|---|---|
| 1 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceScreenModel.kt | 96 |
| 2 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources/MigrateMangaSourceTab.kt | 51 |
| 3 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesFilterScreen.kt | 48 |
| 4 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesFilterScreenModel.kt | 82 |
| 5 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt | 148 |
| 6 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesTab.kt | 90 |
| 7 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreen.kt | 337 |
| 8 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt | 370 |
| 9 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/SourceFilterMangaDialog.kt | 181 |
| 10 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreen.kt | 82 |
| 11 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/GlobalMangaSearchScreenModel.kt | 29 |
| 12 | app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/globalsearch/MangaSearchScreenModel.kt | 235 |

Shorthand used below (expanded here once, always full in citations of the tables): `SRC` = `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source`, `MIG` = `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/sources`. Citations in running text use the file name plus line; the full path is in the table above or in p2-inventory.md.

---

## 0. Model graph

| Screen (presentation) | Model (this scope) | Domain interactors used | Data |
|---|---|---|---|
| Browse tab, Sources page (`MangaSourcesTab.kt`) | `MangaSourcesScreenModel` | `GetEnabledMangaSources`, `ToggleMangaSource`, `ToggleMangaSourcePin`, `ToggleExcludeFromMangaDataSaver` (SY) | `MangaSourceRepository.getMangaSources()`, prefs |
| Sources filter (`MangaSourcesFilterScreen`) | `MangaSourcesFilterScreenModel` | `GetLanguagesWithMangaSources`, `ToggleMangaSource`, `ToggleLanguage` | `MangaSourceRepository.getOnlineMangaSources()`, prefs |
| Browse one source (`BrowseMangaSourceScreen`) | `BrowseMangaSourceScreenModel` | `GetRemoteManga`, `NetworkToLocalManga`, `GetManga`, `GetDuplicateLibraryManga`, `GetMangaCategories`, `SetMangaCategories`, `SetMangaDefaultChapterFlags`, `AddMangaTracks`, `UpdateManga`, `GetMangaIncognitoState` | source API, `mangas` table |
| Filter sheet (`SourceFilterMangaDialog`) | state in `BrowseMangaSourceScreenModel` | none | in-memory `FilterList` |
| Global search (`GlobalMangaSearchScreen`) | `GlobalMangaSearchScreenModel` extends `MangaSearchScreenModel` | `NetworkToLocalManga`, `GetManga` | source API, `mangas` table |
| Migrate search subclass (outside scope, referenced) | `MigrateMangaSearchScreenModel` extends `MangaSearchScreenModel` | `GetManga` | same |
| Migrate source picker (`MigrateMangaSourceTab.kt`) | `MigrateMangaSourceScreenModel` | `GetMangaSourcesWithFavoriteCount`, `SetMigrateSorting` | `mangas` table (favorite count), prefs |

Navigation entry points (callers into this scope):
- `app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt:79` adds `mangaSourcesTab()`, `:83` adds `migrateMangaSourceTab()`.
- `MangaSourcesTab.kt:39` global search action pushes `GlobalMangaSearchScreen()` (no args).
- `MangaSourcesTab.kt:44` filter action pushes `MangaSourcesFilterScreen()`.
- `MangaSourcesTab.kt:52` source row pushes `BrowseMangaSourceScreen(source.id, listing.query)`. `listing.query` is `GetRemoteManga.QUERY_POPULAR` or `QUERY_LATEST` (sentinel strings, see section 2.1).
- `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt:361` pushes `GlobalMangaSearchScreen(query)` (title search from manga details).
- `app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt:241` pushes `GlobalMangaSearchScreen(state.searchQuery ?: "")`.
- `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:514` (action `ACTION_SEARCH` manga branch) pushes `GlobalMangaSearchScreen(query)` after `popUntilRoot()`.
- `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:530` (`INTENT_SEARCH = "eu.kanade.tachiyomi.SEARCH"`, extras `query`, `filter`) pushes `GlobalMangaSearchScreen(query, filter)`. `filter` is an extension package name (`MainActivity.kt:591-594`).
- `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/manga/DeepLinkMangaScreen.kt:49` replaces with `GlobalMangaSearchScreen(query)` when the deep link finds no result.
- `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/manga/MigrateMangaScreen.kt:41` pushes `MigrateMangaSearchScreen` (subclass, outside scope). The migrate search subclass is referenced in section 12.
- `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreen.kt:165` pushes `MigrateMangaSearchScreen(manga.id)`.
- Genre tags on a manga page: `MangaScreen.kt:152` (`onTagSearch`) calls `performGenreSearch` (`MangaScreen.kt:386`), which calls `previousController.searchGenre(genreName)` (`MangaScreen.kt:398`) when the previous screen is a `BrowseMangaSourceScreen` and the source is an `HttpSource`; otherwise it falls back to `performSearch(navigator, genreName, global = false)` (`MangaScreen.kt:359`).

---

## 1. Shared rules used by all models

### 1.1 Preference snapshots vs live flows

OBSERVED:
- `MangaSearchScreenModel` reads `enabledLanguages`, `disabledMangaSources`, `pinnedMangaSources` once at construction (`MangaSearchScreenModel.kt:49-51`). Changes made while the screen is open do not affect `getEnabledSources()` until the screen is recreated.
- `BrowseMangaSourceScreenModel` reads `hideInMangaLibraryItems` once (`BrowseMangaSourceScreenModel.kt:108`).
- Live flows are used for: `globalSearchFilterState()` (`MangaSearchScreenModel.kt:68-70`), `migrationSortingDirection()` and `migrationSortingMode()` (`MigrateMangaSourceScreenModel.kt:52-58`), `enabledLanguages()` and `disabledMangaSources()` in the Sources filter (`MangaSourcesFilterScreenModel.kt:29-33`), `enabledLanguages`, `disabledMangaSources`, `lastUsedMangaSource`, `dataSaverExcludedSources`, `pinnedMangaSources` in `GetEnabledMangaSources.kt:19-29` (live, via `combine`).

Proposal: the Rust models take snapshots from the config store at screen creation and expose `refresh_prefs()`; the Sources list and the Sources filter subscribe to change notifications from the config store (area 02). Global search keeps the Kotlin behaviour (snapshot) unless area 02 makes subscriptions cheap; the difference is invisible in normal use.

### 1.2 Screen-model scope and cancellation

OBSERVED:
- `screenModelScope` (Voyager) is cancelled when the screen is popped.
- `ioCoroutineScope` (`app/src/main/java/eu/kanade/presentation/util/Navigator.kt:55-61`): `CoroutineScope(Dispatchers.IO + SupervisorJob()) + CoroutineName(key)`, cancelled on dispose. Used by the browse paging flow (`BrowseMangaSourceScreenModel.kt:111-125`) and by the global search fetches (`MangaSearchScreenModel.kt:153`).
- `launchIO` (`core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:49-50`) = `launch(Dispatchers.IO)`.
- `withIOContext` (same file, line 60) = `withContext(Dispatchers.IO)`.

Proposal: one tokio task group per screen (`tokio_util::task::TaskTracker` or `JoinSet`) cancelled in the page's `Drop`. See p2-rust-design.md section 5.

### 1.3 Source identity

- `SourceId` is `Long` (`i64`). Source ids are strings in the preference sets (`"${source.id}"`), e.g. `hidden_catalogues`, `pinned_catalogues` (`SourcePreferences.kt:57,63`). The text encoding is part of the on-disk contract (section 15 of p2-data-and-config.md).
- `LocalMangaSource.ID = 0L` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:378`).
- `StubMangaSource` is returned by `getOrStub` for unknown ids (`app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt:89-93`). Its `name` and `lang` are `""` when the id is unknown (`AndroidMangaSourceManager.kt:115-124`).

### 1.4 Source status flags

`Source` (`domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14`):
- `id: Long`, `lang: String`, `name: String`, `supportsLatest: Boolean`, `isStub: Boolean`, `pin: Pins = Pins.unpinned`, `isUsedLast: Boolean = false`, `isExcludedFromDataSaver: Boolean = false` (SY).
- `visualName` (`Source.kt:16-20`): `name` if `lang` is empty, else `"$name (${lang.uppercase()})"`.
- `key` (`Source.kt:22-27`): `"$id-lastused"` when `isUsedLast`, else `"$id"`. It is declared as a lambda property `val key: () -> String` (quirk: call as `key()`).

`Pin` / `Pins` (`domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt`):
- `Pin.Unpinned = 0b00`, `Pin.Pinned = 0b01`, `Pin.Actual = 0b10` (`Pin.kt:4-6`).
- `Pins.contains(pin) = (pin.code and code) == pin.code` (`Pin.kt:19`).
- `Pins.plus(pin) = code or pin.code` (`Pin.kt:21`).
- `Pins.minus(pin) = code xor pin.code` (`Pin.kt:23`). XOR, not AND-NOT. Subtracting a bit that is not set SETS it. Used at `GetEnabledMangaSources.kt:45` (`source.pin - Pin.Actual`). For an unpinned source this sets the `Actual` bit on the last-used copy; it has no visible effect because `isUsedLast` is tested first (`MangaSourcesScreenModel.kt:86-90`).
- `Pins.pinned = Pins(Pin.Pinned, Pin.Actual)` = `0b11` (`Pin.kt:28`). `Pins.unpinned = 0b00` (`Pin.kt:26`).
- Practical meaning: a source that is actually pinned has code `0b11` (both bits). The last-used copy of a pinned source has code `0b01` (`0b11 xor 0b10`).

Proposal: keep the exact codec (`u32` with the same bit values) so that backup and config round-trips stay compatible; implement `minus` as `and_not` only where the Kotlin code uses it for a set bit, and keep the XOR semantics for the one call site (documented in p2-rust-design.md section 3).

### 1.5 Language sort comparator (shared)

`LocaleHelper.comparator` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:17-25`):
- If `a == "all"` then `-1`; else if `b == "all"` then `1`; else `getLocalizedDisplayName(a).compareTo(getLocalizedDisplayName(b))`.
- Quirk: `compare("all","all")` returns `-1`, not `0`. Sorted maps never compare a key with itself in this scope, so it does not corrupt results here, but Rust must return `Equal` for `a == b`.
- `getLocalizedDisplayName(lang)` (`LocaleHelper.kt:55-70`): `null` -> `""`; `""` -> system default locale; `zh-CN` -> `zh-Hans`; `zh-TW` -> `zh-Hant`; otherwise `Locale.forLanguageTag(lang)`. Result: `locale.getDisplayName(locale)` with first char uppercased.
- `getSourceDisplayName(lang)` (`LocaleHelper.kt:30-42`): `LAST_USED_KEY` -> `last_used_source`; `PINNED_KEY` -> `pinned_sources`; `"other"` -> `other_source`; `"all"` -> `multi_lang`; else localized name.
- `getDefaultEnabledLanguages()` (`LocaleHelper.kt:72-74`) = `setOf("all", "en", Locale.getDefault().language)`.
- Internal keys (`LocaleHelper.kt:84-85`): `PINNED_KEY = "pinned"`, `LAST_USED_KEY = "last_used"`. These are group keys, not languages.

Proposal: part 1's `browse::display_name` module implements these (p1-rust-design.md section 9). This area only calls it.

### 1.6 Result-of-interactor error handling

OBSERVED:
- `SetMangaCategories.await` catches all exceptions and logs at ERROR (`domain/src/main/java/tachiyomi/domain/category/manga/interactor/SetMangaCategories.kt:11-17`). Category writes never surface errors to the UI.
- `ToggleMangaSource`, `ToggleMangaSourcePin`, `ToggleLanguage`, `ToggleExcludeFromMangaDataSaver`, `SetMigrateSorting` have no error handling; they are preference writes.
- `updateManga.await(...)` in `changeMangaFavorite` has no try/catch in the model (`BrowseMangaSourceScreenModel.kt:251`). An exception there ends the coroutine silently (no UI).

Proposal: every write returns `Result<(), String>`; errors are logged (`tracing::error!`) and shown as a toast with the `MR.strings.internal_error` key only for the UI actions that the Kotlin shows an error for (section 14).

---

## 2. Sources page (installed sources, pins, last used)

Files: `SRC/MangaSourcesScreenModel.kt`, `SRC/MangaSourcesTab.kt`, `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt`.

### 2.1 Entry

- The row click opens the Popular listing: `onClickItem(source, Listing.Popular)` (presentation `app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt:124`).
- The Latest text button (only if `source.supportsLatest`, `MangaSourcesScreen.kt:127-128`) opens `Listing.Latest`.
- Long press opens the options dialog (`MangaSourcesTab.kt:55`, `screenModel::showSourceDialog`).
- Tab actions: global search (`MangaSourcesTab.kt:37-40`) and filter (`MangaSourcesTab.kt:42-45`), see section 2.6.

### 2.2 Source list pipeline (GetEnabledMangaSources)

Source: `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt`

Steps (`subscribe()`, lines 18-51):
1. Inputs (combined, any change re-runs): `pinnedMangaSources()` (set of id strings), `enabledLanguages()` (set of lang codes), and a triple of `disabledMangaSources()`, `lastUsedMangaSource()` (Long, default -1), `dataSaverExcludedSources()`; and `repository.getMangaSources()` (all catalogue sources of the manager).
2. `filter { it.lang in enabledLanguages || it.id == LocalMangaSource.ID }` (line 32). The local source is always listed.
3. `filterNot { it.id.toString() in disabledSources }` (line 33).
4. `sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })` (line 34). Name sort, case-insensitive, not language.
5. For each source (`flatMap`, lines 35-48):
   - `flag = if ("${it.id}" in pinnedSourceIds) Pins.pinned else Pins.unpinned` (line 36).
   - `source = it.copy(pin = flag, isExcludedFromDataSaver = ...)` (lines 37-42).
   - Output list starts with `source`.
   - If `source.id == lastUsedSource` (line 44), also append `source.copy(isUsedLast = true, pin = source.pin - Pin.Actual)` (line 45). The copy follows the original immediately.
6. `distinctUntilChanged()` (line 50).

`lastUsedMangaSource()` default is `-1` (no last used). Written by `BrowseMangaSourceScreenModel.kt:100-102` (section 5.3).

### 2.3 Grouping and ordering (MangaSourcesScreenModel.collectLatestSources)

Source: `SRC/MangaSourcesScreenModel.kt:71-107`.

- Group key for each source (`groupByTo`, lines 85-91):
  - `isUsedLast` -> `LAST_USED_KEY` (`"last_used"`).
  - else `Pin.Actual in it.pin` -> `PINNED_KEY` (`"pinned"`).
  - else `it.lang`.
- Key order: a `TreeMap<String, MutableList<Source>>` with comparator (lines 73-84):
  1. `LAST_USED_KEY` before everything else.
  2. `PINNED_KEY` before everything except `LAST_USED_KEY`.
  3. `""` (no language) after all non-empty keys.
  4. Otherwise `d1.compareTo(d2)` (Java `String.compareTo`, code-unit order, case-sensitive, not locale aware). Consequence: `"all"` sorts among language codes by code unit, not first. (Contrast with `LocaleHelper.comparator` used in the Sources filter, section 3.3.)
- Output (lines 93-105): `isLoading = false`; `items` = for each group in key order: `Header(key)` then `Item(source)` for each source in that group, in the order produced by section 2.2 step 4 (name order).
- Pinned sources appear under the PINNED header only. A last-used source appears under LAST_USED and, in its own language or pinned group, as its original item.

Sample (OBSERVED from the code, verified by reading the comparator): sources A (en, last used), B (ja, pinned), C (en), D (ja). Output: `Header(last_used)` `A(copy)`, `Header(pinned)` `B`, `Header(en)` `A`, `C`, `Header(ja)` `D`.

Quirk: a pinned + last-used source is shown twice (LAST_USED copy and the original in PINNED or in its language group, depending on pin). The original keeps `Pins.pinned` only when pinned.

### 2.4 Loading, empty, events

- `state.isLoading = true` initially (`MangaSourcesScreenModel.kt:140`). It becomes `false` on the first emission of `getEnabledSources.subscribe()` (line 93-95).
- `isEmpty` = `items.isEmpty()` (line 146). The presentation shows `source_empty_screen` when empty (part 1, section 3.1).
- Error: `catch` on the subscribe flow logs at ERROR and sends `Event.FailedFetchingSources` (lines 52-55). The channel is `Channel<Event>(Int.MAX_VALUE)` (line 46) and the tab shows a snackbar with `MR.strings.internal_error` (`MangaSourcesTab.kt:78-87`). The model state is not changed.
- OBSERVED race (from the code): `MangaSourceManager.catalogueSources` is backed by `sourcesMapFlow = MutableStateFlow(ConcurrentHashMap())` (`AndroidMangaSourceManager.kt:42`), which starts empty. `getMangaSources()` maps it immediately. The first emission can therefore be an empty list with `isLoading = false`, which shows the empty state until the extensions finish loading. Only the browse screens call `ifMangaSourcesLoaded()`; the Sources tab does not.

Proposal: gate the Sources list on `MangaSourceManager.isInitialized` (the same flag the browse screens use, `app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt:11-13`) and keep `isLoading = true` until both the flag and the first emission are true.

### 2.5 Actions

- `toggleSource(source)` (`MangaSourcesScreenModel.kt:109-111`) -> `ToggleMangaSource.await(source)` with the default `enable` (section 3.2). Disables an enabled source and enables a disabled one.
- `togglePin(source)` (lines 113-115) -> `ToggleMangaSourcePin.await(source)` (`ToggleMangaSourcePin.kt:11-16`): `isPinned = source.id.toString() in pinnedMangaSources().get()`; then `getAndSet` adds or removes `"${source.id}"`.
- `showSourceDialog(source)` (lines 123-125) sets `dialog = Dialog(source)`. `closeDialog()` (lines 127-129).
- `MangaSourceOptionsDialog` rows (part 1 section 3.3, presentation `MangaSourcesScreen.kt:168-221`):
  1. Pin/unpin. Pin row label: `action_unpin` if `Pin.Pinned in source.pin`, else `action_pin`. Callback: `togglePin(source)` then `closeDialog()` (`MangaSourcesTab.kt:62-65`).
  2. Disable: `action_disable`, shown only if `source.id != LocalMangaSource.ID` (part 1). Callback: `toggleSource(source)` then `closeDialog()` (`MangaSourcesTab.kt:66-69`).
  3. Data saver (SY): `onClickToggleDataSaver` is `null` unless `state.dataSaverEnabled` (`MangaSourcesTab.kt:70-73`). Dropped (T-40b-082).
- `dataSaverEnabled = dataSaver().get() != DataSaver.NONE` (lines 59-66), listened via `changes()`.

### 2.6 Tab actions

`mangaSourcesTab()` (`MangaSourcesTab.kt:28-89`):
- Title: `AYMR.strings.label_manga_sources` (`MangaSourcesTab.kt:34`, English "Manga Sources").
- Action 1: `action_global_search` ("Global search"), icon `Icons.Outlined.TravelExplore`, pushes `GlobalMangaSearchScreen()` (`MangaSourcesTab.kt:36-40`).
- Action 2: `action_filter` ("Filter"), icon `Icons.Outlined.FilterList`, pushes `MangaSourcesFilterScreen()` (`MangaSourcesTab.kt:41-45`).
- Snackbar: `internal_error` on `FailedFetchingSources` (lines 78-87).

Proposal: the two actions go into the Browse page header bar (part 1 section 2 and 3). No separate state.

### 2.7 Edge cases (OBSERVED)

- A source whose language is not in `enabledLanguages` disappears from the list, even if pinned or last used (`GetEnabledMangaSources.kt:32`). Last-used can therefore point to a hidden source; the copy is never created because the source is filtered before `flatMap` (lines 31-33 run before 35-48).
- Two sources with identical `lang` and `name`: both appear. `groupByTo` into a `TreeMap` keyed by language is safe (keys are languages). No collision.
- A source disabled while it is last used: it is removed by `filterNot` (line 33), so the last-used header disappears.
- Pin of the local source: allowed (no guard in `togglePin`).

---

## 3. Sources filter page (show or hide sources and languages)

Files: `SRC/MangaSourcesFilterScreen.kt`, `SRC/MangaSourcesFilterScreenModel.kt`, `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt`, `app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt`, `app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt`.

### 3.1 State machine

`MangaSourcesFilterScreenModel.State` (`SRC/MangaSourcesFilterScreenModel.kt:61-81`):
- `Loading` (initial, `:25`).
- `Error(throwable)`.
- `Success(items: SortedMap<String, List<Source>>, enabledLanguages: Set<String>, disabledSources: Set<String>)`, `isEmpty = items.isEmpty()`.

Transitions (`:27-51`):
- `combine(getLanguagesWithSources.subscribe(), enabledLanguages().changes(), disabledMangaSources().changes())` (`:29-33`) emits a `Triple`.
- `catch` -> `State.Error(throwable)` (`:34-40`).
- `collectLatest` -> `State.Success(items = languages, enabledLanguages, disabledSources)` (`:41-49`).

Screen (`SRC/MangaSourcesFilterScreen.kt:25-46`):
- `Loading` -> `LoadingScreen()`.
- `Error` -> `context.toast(MR.strings.internal_error)` then `navigator.pop()` in a `LaunchedEffect(Unit)` (`:30-37`). The screen does not render an error view.
- `Success` -> render with `onClickLanguage = toggleLanguage`, `onClickSource = toggleSource`.

### 3.2 Source list for the filter (GetLanguagesWithMangaSources)

Source: `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt:16-33`.

Steps:
1. Input: `repository.getOnlineMangaSources()` (only `HttpSource` instances, `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:33-39`). The local source and stub sources are not listed here.
2. Combine with `enabledLanguages().changes()` and `disabledMangaSources().changes()`.
3. Per source sort (`:22-25`): `compareBy<Source> { it.id.toString() in disabledSource }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name }`. Enabled sources first (false < true), then case-insensitive name.
4. Group by `lang` (`:28`) and sort groups (`:29-31`) with `compareBy<String> { it !in enabledLanguage }.then(LocaleHelper.comparator)`. Enabled languages first, then `LocaleHelper.comparator` (section 1.5).

Quirk: two different language codes whose localized display names compare equal (`LocaleHelper.comparator` returns `0`) are merged into one `TreeMap` entry. One group is silently lost. Not observed in practice; recorded as an edge case (p2-edge-cases-and-tests.md E-SF-03).

### 3.3 Toggles

- `toggleSource(source)` (`SRC/MangaSourcesFilterScreenModel.kt:53-55`) -> `ToggleMangaSource.await(source)`. Default `enable` = `isEnabled(source.id)`, which is `true` when the id is currently in `disabledMangaSources` (`ToggleMangaSource.kt:34-36`). So the default param means "is currently disabled" and the call re-enables it. The name `isEnabled` is inverted but the toggle works.
- `toggleLanguage(language)` (`SRC/MangaSourcesFilterScreenModel.kt:57-59`) -> `ToggleLanguage.await(language)` (`ToggleLanguage.kt:10-15`): `isEnabled = language in enabledLanguages().get()`; then `getAndSet` removes the language when enabled, adds it otherwise.
- `ToggleMangaSource.await(sourceIds: List<Long>, enable)` (`ToggleMangaSource.kt:21-32`) exists for bulk use; the Sources filter does not call it.

Presentation rules (part 1 section 4): a language header switch shows `enabled = language in enabledLanguages`; source rows are shown only when the language is enabled; a source checkbox is checked when `"${id}" !in disabledSources` (checked = visible).

### 3.4 Edge cases (OBSERVED)

- Turning a language off does not remove its sources from `hidden_catalogues`. Turning it on again restores the previous per-source state.
- A source hidden here is also removed from the Sources page (`GetEnabledMangaSources.kt:33`).
- The filter list is built from online sources only. A source with an empty `lang` string is grouped under `""`, shown with `LocaleHelper.getSourceDisplayName("")` (= localized name of `""` = system language, see `LocaleHelper.kt:55-62`).

---

## 4. Browse one source (BrowseMangaSourceScreenModel: listing, paging, filters)

Files: `SRC/browse/BrowseMangaSourceScreenModel.kt` (370), `SRC/browse/BrowseMangaSourceScreen.kt` (337), `SRC/browse/SourceFilterMangaDialog.kt` (181).

### 4.1 Construction

`BrowseMangaSourceScreenModel(sourceId, listingQuery)` (`BrowseMangaSourceScreenModel.kt:58-75`):
- `state` starts as `State(listing = Listing.valueOf(listingQuery))` (`:75`).
- `displayMode` is a delegated `var` bound to `sourcePreferences.sourceDisplayMode()` via `asState(screenModelScope)` (`:77`). Key `pref_display_mode_catalogue`, default `LibraryDisplayMode.CompactGrid` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:20-23`, `SourcePreferences.kt:16-21`).
- `source = sourceManager.getOrStub(sourceId)` (`:79`). Never null; may be a `StubMangaSource`.
- `init` (`:81-103`):
  1. If `source is CatalogueSource`: `mutableState.update` with: `query = null`, `listing = it.listing`. If `listing is Listing.Search`, set `query = listing.query` and `listing = Listing.Search(query, source.getFilterList())` (`:87-90`). Then `copy(listing = listing, filters = source.getFilterList(), toolbarQuery = query)` (`:92-97`).
  2. `if (!getIncognitoState.await(source.id)) sourcePreferences.lastUsedMangaSource().set(source.id)` (`:100-102`).
- Quirk: for a `Listing.Popular` or `Listing.Latest` start, the listing keeps empty filters (`Listing.Popular` and `Listing.Latest` are `data object`s with `FilterList()`, `:332-333`). The source's default filters go to `State.filters` only. The Kotlin later converts to `Search` when the user applies filters.

`Listing.valueOf(query)` (`:340-347`): `GetRemoteManga.QUERY_POPULAR` -> `Popular`; `QUERY_LATEST` -> `Latest`; otherwise `Search(query, FilterList())`. Note a `null` query becomes `Search(null, ...)`, not `Popular`. Sentinels: `GetRemoteManga.QUERY_POPULAR = "eu.kanade.domain.source.manga.interactor.POPULAR"`, `QUERY_LATEST = "eu.kanade.domain.source.manga.interactor.LATEST"` (`domain/src/main/java/tachiyomi/domain/source/manga/interactor/GetRemoteManga.kt:20-21`).

### 4.2 Remote query dispatch

`GetRemoteManga.subscribe(sourceId, query, filterList)` (`GetRemoteManga.kt:11-17`):
- `QUERY_POPULAR` -> `repository.getPopularManga(sourceId)`.
- `QUERY_LATEST` -> `repository.getLatestManga(sourceId)`.
- else -> `repository.searchManga(sourceId, query, filterList)`.

The model passes `listing.query ?: ""` (`BrowseMangaSourceScreenModel.kt:113`). So a `Search(null, filters)` becomes a search with the empty string and the filters. A blank query with filters is a valid browse-with-filters request.

Repository (`data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:71-88`):
- `searchManga` -> `SourceSearchPagingSource(source, query, filterList)`, calling `source.getSearchManga(page, query, filters)`.
- `getPopularManga` -> `SourcePopularPagingSource`, calling `source.getPopularManga(page)`.
- `getLatestManga` -> `SourceLatestPagingSource`, calling `source.getLatestUpdates(page)`.
- Each uses `as CatalogueSource` (`MangaSourceRepositoryImpl.kt:76,81,86`); a non-catalogue source throws `ClassCastException` (uncaught in the model: see edge case B-EC-07).

### 4.3 Paging (exact)

Source: `data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt`.

`SourcePagingSource.load(params)` (lines 43-61):
1. `page = params.key ?: 1` (`Long`).
2. `withIOContext { requestNextPage(page.toInt()).takeIf { it.mangas.isNotEmpty() } ?: throw NoChaptersException() }` (lines 46-51).
3. Any exception -> `LoadResult.Error(e)` (lines 52-54).
4. Success -> `LoadResult.Page(data = mangasPage.mangas, prevKey = null, nextKey = if (mangasPage.hasNextPage) page + 1 else null)` (lines 56-60).

Consequences:
- An empty page is an error, not an empty list. The error is `NoChaptersException` (`domain/src/main/java/tachiyomi/domain/items/chapter/model/NoChaptersException.kt:3`). The user-visible text is `MR.strings.no_results_found` ("No results found") via `ExceptionFormatter` (`app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:28-30`).
- `prevKey` is always `null`; paging is forward only.
- `getRefreshKey` (lines 63-68) = `anchorPage.prevKey ?: anchorPage.nextKey` (for refresh after an error or pull-to-refresh).

Paging config (`BrowseMangaSourceScreenModel.kt:112`): `PagingConfig(pageSize = 25)`. The androidx.paging defaults that apply because they are not overridden: `prefetchDistance = pageSize` (25), `initialLoadSize = 3 * pageSize` (75), `enablePlaceholders = true`, `maxSize = unbounded`. The source's own page size is not used; the paging library only uses `pageSize` for prefetch and the initial window.

Proposal (Kurobayomi): `PAGE_SIZE = 25` for prefetch, initial window of 75 items (three pages) unless the source returns fewer. Pages are loaded in order: page 1, then `nextKey`. A request is issued when the last visible item index is within 25 items of the end (prefetch).

### 4.4 Row mapping and the favorite flag

`mangaPagerFlowFlow` (`BrowseMangaSourceScreenModel.kt:109-125`):
- `state.map { it.listing }.distinctUntilChanged().map { listing -> Pager(...) }` (`:109-111`): a new `Pager` is created whenever the listing emits a value that is not `equals` to the previous one. See the equality note in 4.4.1.
- Each remote manga (`:115-120`): `networkToLocalManga.await(it.toDomainManga(sourceId))` (inserts or returns the local row), then `getManga.subscribe(localManga.url, localManga.source).filterNotNull().stateIn(ioCoroutineScope)`. The result is a live `StateFlow<Manga>` per row, so the favorite flag and cover changes reach the row without reloading the page.
- Filter (`:121`): `filter { !hideInLibraryItems || !it.value.favorite }`. `hideInLibraryItems` is read once (`:108`). The filter reads the current value of the row's `StateFlow` when the page is collected, so a favorite added later stays visible until the listing changes.
- `cachedIn(ioCoroutineScope)` (`:123`).
- `stateIn(ioCoroutineScope, SharingStarted.Lazily, emptyFlow())` (`:125`).

`NetworkToLocalManga.await` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/NetworkToLocalManga.kt:10-26`):
- No local row for `(url, source)`: insert the network manga (favorite = false) and return it with its new id. Side effect: a DB insert for every browsed result.
- Local row exists and is not a favorite: return the local row with `title = manga.title` (the remote title wins for non-library rows).
- Local row is a favorite: return the local row unchanged.

Side effect table (browse): every browsed page writes up to 25 `mangas` rows (non-favorite). Non-library rows are removed by `deleteMangasNotInLibraryBySourceIds` (`data/src/main/sqldelight/data/mangas.sq`, `DELETE ... WHERE favorite = 0 AND source IN :sourceIds`). The caller of that query is outside this scope (area:05 / area:35 to confirm).

#### 4.4.1 Equality note (OBSERVED, important)

- `FilterList` overrides `equals` to always return `false` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/FilterList.kt:10-12`).
- `Listing.Search` is a `data class` containing `filters: FilterList`, so `Search(...) == Search(...)` is always `false` unless the two references are identical (data class `equals` checks `===` first).
- Effect: `distinctUntilChanged()` on the listing emits for every new `Search` object, even with the same query. `search()` always creates a new object (`:167-182`), so the pager restarts and the list scrolls to the top. `Popular` and `Latest` are singletons and are deduplicated.
- `State` (data class with a `FilterList`) is also never equal to another instance, so every `state` update emits to collectors. This only costs recomposition.

Proposal: the Rust model keeps a `search_generation: u64`, incremented by every `search()`, `search_genre()` and `set_filters()`-with-apply, and the page loader is keyed by `(listing_kind, query, generation)`. Identical searches restart the list (matching the Kotlin visible result) and no equality on filter trees is needed.

### 4.5 Columns preference

`getColumnsPreference(orientation)` (`:127-135`):
- Landscape: `mangaLandscapeColumns()`; portrait: `mangaPortraitColumns()` (`:129-133`). Both default `0` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:103,106`).
- `columns == 0` -> `GridCells.Adaptive(128.dp)` (`:134`); else `GridCells.Fixed(columns)`.
- `getColumnsPreferenceForCurrentOrientation` (`:138-145`) returns the raw int (the slider value, 0 meaning adaptive).

Proposal: `0` = adaptive with a minimum cell width of 128 logical px (matching `128.dp`); `n > 0` = exactly `n` columns. The keys are owned by area 37 (`p1-data-and-config.md` section 5).

### 4.6 Mutations of listing and filters

| Function | Lines | Effect on state |
|---|---|---|
| `resetFilters()` | `:147-151` | `filters = source.getFilterList()` (fresh default list). No-op for non-catalogue sources. |
| `setListing(listing)` | `:153-155` | `listing = listing`, `toolbarQuery = null`. |
| `setFilters(filters)` | `:157-165` | `filters = filters`. Listing unchanged until a search. |
| `search(query, filters)` | `:167-182` | see 4.6.1. |
| `setToolbarQuery(q)` | `:327-329` | `toolbarQuery = q`. Does not search. |
| `openFilterSheet()` | `:319-321` | `dialog = Dialog.Filter`. |
| `setDialog(d)` | `:323-325` | `dialog = d`. |

Chip behaviour (`BrowseMangaSourceScreen.kt:157-211`):
- `Popular` chip (`:158-174`): `resetFilters()` then `setListing(Popular)`. Selected when `listing == Popular`.
- `Latest` chip (`:176-194`): only if `(source as CatalogueSource).supportsLatest` (`:175`). Same pattern with `Latest`.
- Filter chip (`:195-211`): shown only if `state.filters.isNotEmpty()` (the list includes headers and separators). Selected when `listing is Search`. Click opens the sheet (`openFilterSheet`).

#### 4.6.1 search(query, filters) exact algorithm

Source: `BrowseMangaSourceScreenModel.kt:167-182`.

```
if source !is CatalogueSource: return
input = (state.listing as? Search) ?: Search(query = null, filters = source.getFilterList())   // fresh defaults, NOT state.filters
newListing = input.copy(query = query ?: input.query, filters = filters ?: input.filters)
state.listing = newListing
state.toolbarQuery = query ?: input.query
```

Points:
- `query == null` keeps the current query. Passing `""` replaces it with `""` (a blank query is accepted and sent as `""`).
- `filters == null` keeps the current listing's filters (for a non-search listing, the fresh defaults).
- Starting from `Popular`, a search with a query uses the default filters of the source, not the filter sheet's values (because `state.filters` is not consulted). The filter sheet's apply button passes `filters = state.filters` explicitly (`BrowseMangaSourceScreen.kt:260`).

Toolbar submit: `onSearch = screenModel::search` (`BrowseMangaSourceScreen.kt:148`). The model signature is `search(query: String? = null, filters: FilterList? = null)`, so a toolbar submit with the text passes it as `query`; the presentation (part 1) decides whether it passes the text or nothing. Typing alone (`onSearchQueryChange = setToolbarQuery`, `:140`) never searches.

#### 4.6.2 Navigate-up rule

`navigateUp` (`BrowseMangaSourceScreen.kt:92-99`):
- If `!state.isUserQuery && state.toolbarQuery != null` -> `setToolbarQuery(null)` (close the search text without popping).
- Else `navigator.pop()`.
- `isUserQuery` (`BrowseMangaSourceScreenModel.kt:368`) = `listing is Search && !listing.query.isNullOrEmpty()`.

Consequence: when a real search is active, back clears only the toolbar text if the toolbar has text that is not the listing query; the second back pops the screen. Rust: the same two-step rule.

### 4.7 Genre search (searchGenre)

Source: `BrowseMangaSourceScreenModel.kt:184-227`. Called from the manga page's genre tag through `BrowseMangaSourceScreen.searchGenre(name)` (`BrowseMangaSourceScreen.kt:327`) which sends `SearchType.Genre(name)` on the companion `Channel` (`:330`).

Algorithm:
1. `if source !is CatalogueSource: return`.
2. `defaultFilters = source.getFilterList()` (fresh).
3. `genreExists = false`.
4. For each top-level `sourceFilter` in order (label `filter@`, `:190`):
   - If `Filter.Group`: for each child `filter` in `sourceFilter.state` (`:192`), if `filter is Filter<*>` and `filter.name.equals(genreName, ignoreCase = true)`:
     - `TriState` -> `state = 1` (`STATE_INCLUDE`); `CheckBox` -> `state = true`; other child types -> no change (`:194-198`).
     - `genreExists = true`; `break@filter` (exits the whole outer loop, `:199-200`).
   - Else if `Filter.Select`: `index = values.filterIsInstance<String>().indexOfFirst { it.equals(genreName, true) }` (`:203-205`). If `index != -1`: `state = index` (the index into the string-filtered list, not into `values`; quirk), `genreExists = true`, `break` (`:207-210`).
   - Other top-level filter types are ignored.
5. `mutableState.update`: `listing = if (genreExists) Search(query = null, filters = defaultFilters) else Search(query = genreName, filters = defaultFilters)`, `filters = defaultFilters`, `toolbarQuery = listing.query` (`:215-225`).

Consequences:
- A genre matching a group child (even a text or select child, which does not change state) counts as found. The search runs with `query = null` and the filter state as set.
- No match: a text search with `genreName` as the query and default filters.
- The genre match is case-insensitive (`equals(..., true)`).

Proposal: keep the algorithm; for a `Select` the Rust `FilterNode::Select` stores `values: Vec<SelectValue>` with `is_string` so `state` indexes the original `values` (fixes the quirk, documented as a deliberate deviation in section 14).

### 4.8 Favorite (library) actions from the browse page

#### 4.8.1 changeMangaFavorite(manga)

Source: `BrowseMangaSourceScreenModel.kt:234-253`. Runs on `screenModelScope` (Dispatchers.Main by default, but all awaited calls suspend).

```
new = manga.copy(
    favorite = !manga.favorite,
    dateAdded = if (manga.favorite) 0 else Instant.now().toEpochMilli())     // L236-242
if (!new.favorite):
    new = new.removeCovers(coverCache)                                        // L244-245
else:
    setMangaDefaultChapterFlags.await(manga)                                  // L247
    addTracks.bindEnhancedTrackers(manga, source)                             // L248
updateManga.await(new.toMangaUpdate())                                        // L251
```

`removeCovers` (`app/src/main/java/eu/kanade/tachiyomi/util/MangaExtensions.kt:43-50`): if the manga is local, no-op; else `coverCache.deleteFromCache(this, true) > 0` -> `copy(coverLastModified = now)`; else unchanged.

Notes:
- `dateAdded` is milliseconds since epoch (`Instant.toEpochMilli()`). `0` when removed.
- `setMangaDefaultChapterFlags.await(manga)` runs on add: it sets the manga's chapter sort/filter flags from the library defaults (area 08). `addTracks.bindEnhancedTrackers(manga, source)` binds enhanced trackers (area 27/33). `source` is the screen's source (the one being browsed), not the manga's source object.
- Both are awaited before `updateManga`; an exception in them stops the coroutine before `updateManga` (no UI feedback).
- Probable defect (OBSERVED from code, verify on device): `updateManga.await(new.toMangaUpdate())` sends every non-null field of the snapshot `manga` (`domain/src/main/java/tachiyomi/domain/entries/manga/model/MangaUpdate.kt:29-41` includes `chapterFlags`, `viewerFlags`, `lastUpdate`, ...). The SQL is `coalesce(:field, field)` per column (`data/src/main/sqldelight/data/mangas.sq:156-178`), so every snapshot value is written back. `setMangaDefaultChapterFlags.await(manga)` wrote `chapter_flags` a moment earlier (`domain/src/main/java/tachiyomi/domain/items/chapter/interactor/SetMangaDefaultChapterFlags.kt:14-27`), and the snapshot still holds the old `chapterFlags`, so the final write reverts the library default flags for the new favorite. Proposal: a favorite toggle writes only `favorite`, `dateAdded`, `coverLastModified` (partial `MangaUpdate`) and never the whole row. Tracked as T-40b-041 and edge case B-EC-12.

#### 4.8.2 addFavorite(manga) decision tree

Source: `BrowseMangaSourceScreenModel.kt:255-288`.

```
categories = getCategories()                        // member fun, see 4.8.3
defaultCategoryId = libraryPreferences.defaultMangaCategory().get()   // Int, default -1
defaultCategory = categories.find { it.id == defaultCategoryId.toLong() }

when:
  defaultCategory != null  -> moveMangaToCategories(manga, defaultCategory)   // L264
                              changeMangaFavorite(manga)                      // L266
  defaultCategoryId == 0
    || categories.isEmpty() -> moveMangaToCategories(manga)                   // L271 (empty list)
                              changeMangaFavorite(manga)                      // L273
  else                      -> preselected = getCategories.await(manga.id).map { it.id }   // L278
                               setDialog(ChangeMangaCategory(manga, categories.mapAsCheckboxState { it.id in preselected }))  // L279-284
```

Details:
- `defaultMangaCategory` key `default_category`, default `-1` (`LibraryPreferences.kt:154` and `:413`). `-1` is the "Always ask" option (`default_category_summary`, `SettingsLibraryScreen.kt:91-96`). So on a fresh install with at least one user category, adding a manga opens the chooser.
- `0` is the uncategorised system category; with `0`, the manga goes to no category (empty list) and is favorited.
- The vararg overload `moveMangaToCategories(manga, vararg categories)` (`:306-308`) drops category id `0` and calls the list overload. A non-zero default category is therefore always passed as `[id]`.
- The list overload (`:310-317`) launches `setMangaCategories.await(mangaId, categoryIds)` on IO. It replaces all links for the manga (`MangaRepositoryImpl.setMangaCategories` deletes then inserts in one transaction, `data/src/main/java/tachiyomi/data/entries/manga/MangaRepositoryImpl.kt:91-97`).
- Race (OBSERVED): `moveMangaToCategories` runs on IO and `changeMangaFavorite` on the main scope with no ordering. The favorite flag and the category links can be written in either order. The category write is not visible to a concurrent library refresh in a defined order.
- `getCategories.await(manga.id)` (`GetMangaCategories.kt:22-24`) returns the category links of the manga (may include `0` if linked; not filtered).

Proposal: run the category write, then the favorite write, sequentially in one task; no change in visible result.

#### 4.8.3 getCategories()

`suspend fun getCategories(): List<Category>` (`:295-300`): `getCategories.subscribe().firstOrNull()?.filterNot { it.isSystemCategory }.orEmpty()`. `subscribe()` = `categoryRepository.getAllMangaCategoriesAsFlow()` (`GetMangaCategories.kt:10-12`). `isSystemCategory` = `id == 0` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:13`).

#### 4.8.4 Long press on a browse row

Source: `BrowseMangaSourceScreen.kt:232-249`:
```
launchIO {
  duplicate = getDuplicateLibraryManga(manga)           // first element or null
  when {
    manga.favorite  -> setDialog(RemoveManga(manga))
    duplicate != null -> setDialog(AddDuplicateManga(manga, duplicate))
    else            -> addFavorite(manga)
  }
  haptic.performHapticFeedback(LongPress)               // Android only, dropped
}
```

`getDuplicateLibraryManga(manga)` (`:302-304`) -> `GetDuplicateLibraryManga.await(manga).getOrNull(0)`. The DB query (`data/src/main/sqldelight/data/mangas.sq`, `getDuplicateLibraryManga`): `SELECT * FROM mangas WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id`. The Kotlin passes `manga.title.lowercase()` (`domain/src/main/java/tachiyomi/domain/entries/manga/interactor/GetDuplicateLibraryManga.kt:11`). Matching across all sources, including the same source.

Quirk (OBSERVED): SQLite `LOWER()` folds ASCII only unless built with ICU, while Kotlin `lowercase()` folds Unicode. Titles with non-ASCII capitals may not match. Proposal in section 14.

`RemoveManga` -> confirmation dialog -> `changeMangaFavorite(dialog.manga)` (`BrowseMangaSourceScreen.kt:289-297`).
`AddDuplicateManga` -> `DuplicateMangaDialog` (presentation, `app/src/main/java/eu/kanade/presentation/entries/manga/DuplicateMangaDialog.kt`, referenced at `BrowseMangaSourceScreen.kt:265-274`):
- confirm -> `addFavorite(dialog.manga)` (adds anyway; the duplicate stays);
- open -> push `MangaScreen(dialog.duplicate.id)`;
- migrate -> `setDialog(Migrate(newManga = dialog.manga, oldManga = dialog.duplicate))`.

`Migrate` -> `MigrateMangaDialog(oldManga, newManga, MigrateMangaDialogScreenModel(), onDismissRequest, onClickTitle = push MangaScreen(oldManga.id), onPopScreen = dismiss)` (`BrowseMangaSourceScreen.kt:277-288`). The migrate dialog model is outside this scope.

`ChangeMangaCategory` -> `ChangeCategoryDialog(initialSelection, onDismissRequest, onEditCategories = push CategoriesTab + showMangaCategory(), onConfirm = { include, _ -> changeMangaFavorite(manga); moveMangaToCategories(manga, include) })` (`BrowseMangaSourceScreen.kt:298-311`). The second callback parameter (exclude list) is ignored. The favorite and category writes race as in 4.8.2.

### 4.9 Dialog state (BrowseMangaSourceScreenModel.Dialog)

`sealed interface Dialog` (`:350-359`): `Filter`, `RemoveManga(manga)`, `AddDuplicateManga(manga, duplicate)`, `ChangeMangaCategory(manga, initialSelection: ImmutableList<CheckboxState.State<Category>>)`, `Migrate(newManga, oldManga)`. Only one dialog at a time (single `dialog` field in `State`, `:366`).

### 4.10 Filter sheet apply and reset

- Sheet: `SourceFilterMangaDialog(onDismissRequest, filters = state.filters, onReset = resetFilters, onFilter = { search(filters = state.filters) }, onUpdate = setFilters)` (`BrowseMangaSourceScreen.kt:256-262`).
- Reset button -> `resetFilters()` (replaces the state filters with fresh defaults; the listing is not searched; the sheet stays open).
- Filter button -> `onFilter()` (search with the current filter values) then `onDismissRequest()`.
- Each control change calls `onUpdate(filters)` -> `setFilters(filters)` (state only, no search).

### 4.11 Source-level helpers

- `getCategories()`, `getDuplicateLibraryManga(manga)`, `moveMangaToCategories(...)` are described in 4.8.
- The model holds the `source`, so `BrowseMangaSourceScreen` branches on `screenModel.source is StubMangaSource` (`BrowseMangaSourceScreen.kt:101-107`) before creating any page. The stub page is part 1 (`MissingSourceScreen`).
- `onWebViewClick` (`:115-124`): only for `HttpSource`; opens `WebViewScreen(url = source.baseUrl, initialTitle = source.name, sourceId = source.id)`.
- `onHelpClick` (`:114`): `LocalMangaSource.HELP_URL` (`https://aniyomi.org/help/guides/local-manga/`) for the local source; BrowseSourceContent's error help uses `Constants.URL_HELP` (`https://aniyomi.org/docs/guides/troubleshooting/`, `BrowseMangaSourceScreen.kt:229`).
- Settings icon: `MangaSourcePreferencesScreen(sourceId)` (`BrowseMangaSourceScreen.kt:147`).
- `ifMangaSourcesLoaded()` gate (`:83-86`): show `LoadingScreen` until `MangaSourceManager.isInitialized` is true.

### 4.12 Query events from other screens

`BrowseMangaSourceScreen` companion has a static `Channel<SearchType>()` (`:329-331`, rendezvous, default capacity). `search(query)` and `searchGenre(name)` (`:326-327`) `send` into it; the composable's `LaunchedEffect(Unit)` collects and calls `screenModel.searchGenre(txt)` or `screenModel.search(txt)` (`:315-323`).

OBSERVED consequence: the channel is shared by all browse screens. A send with no collector suspends the caller. The caller is `performSearch` / `performGenreSearch` in `MangaScreen`, which only calls these when `navigator.items[size - 2]` is a `BrowseMangaSourceScreen` (`MangaScreen.kt:390-398`).

Proposal: a direct method on the open page (the page is found from the nav stack), not a global channel.

---

## 5. Browse source: pagination and listing rules that the UI relies on

- Initial state: `Listing` from the constructor; `filters` from the source; `toolbarQuery` from a search listing; `dialog = null`.
- Loading state: presentation (part 1 section 2.5) shows the spinner while the paging `LoadState` is `Loading` and the list is empty.
- Error state with no items: `LoadState.Error` on refresh (presentation shows the error and a retry; part 1 section 2.4).
- Error with items: appended-page error shows a snackbar (part 1 section 2.3).
- Empty success (page 1 empty): paging `Error(NoChaptersException)`, shown as `no_results_found`.
- Display mode: `displayMode` (4 values, section 15 of p2-data-and-config.md).

---

## 6. Sources filter (in the Browse tab) for the dialog: SourceFilterMangaDialog

Source: `SRC/browse/SourceFilterMangaDialog.kt`.

Layout (`:34-77`):
- `AdaptiveSheet(onDismissRequest)` containing a `LazyColumn`.
- `stickyHeader` (`:45-70`): `Row` with background `colorScheme.background`, padding `8.dp`. `TextButton(onReset)` labelled `action_reset` ("Reset", colour `primary`, `:51-58`). `Spacer(weight 1)`. `Button` labelled `action_filter` ("Filter", `:62-67`) calling `onFilter()` then `onDismissRequest()`. `HorizontalDivider()`.
- Body: `items(filters) { FilterItem(it, updateFilters) }` (`:72-74`). `updateFilters = { onUpdate(filters) }` (`:41`) is called after each control change.

`FilterItem(filter, onUpdate)` (`:79-164`) by the runtime class of `filter`:

| Filter type | Rendering | Mutation on click or change | Calls onUpdate |
|---|---|---|---|
| `Filter.Header` | `HeadingItem(filter.name)` | none | no |
| `Filter.Separator` | `HorizontalDivider()` | none | no |
| `Filter.CheckBox` | `CheckboxItem(label = name, checked = state)` | `state = !state` | yes |
| `Filter.TriState` | `TriStateItem(label, state = state.toTriStateFilter())` | `state = state.toTriStateFilter().next().toTriStateInt()` | yes |
| `Filter.Text` | `TextItem(label, value = state)` | `state = newText` on every keystroke | yes |
| `Filter.Select<*>` | `SelectItem(label, options = values, selectedIndex = state, onSelect)` | `state = index` | yes |
| `Filter.Sort` | `CollapsibleBox(heading = name)` with one `SortItem` per value | see 6.1 | yes |
| `Filter.Group<*>` | `CollapsibleBox(heading = name)` with children `FilterItem` recursively (children filtered to `Filter<*>`) | children act themselves | via children |

### 6.1 Sort semantics (exact)

Source `:126-151`. For each value index `i`:
- `SortItem(label = item, sortDescending = filter.state?.ascending?.not()?.takeIf { i == filter.state?.index })` (`:132-135`). Arrow down when the selected item is descending, arrow up when ascending, no arrow otherwise.
- On click (`:136-147`):
  - `ascending = if (i == state.index) !state.ascending else state.ascending` (`:137-141`).
  - `state = Selection(index = i, ascending = ascending)`.
- Clicking the already selected item flips the direction. Clicking another item keeps the current direction.
- Null state: `filter.state!!` at `:138,140` throws `NullPointerException` when `state == null` (no selection yet). A source that declares `Sort` with a null initial state crashes on the first click. Proposal: when `None`, select `i` with `ascending = true` (documented deviation D-BR-02 in section 14).

### 6.2 TriState semantics (exact)

- Raw states (`Filter.TriState`, `domain` source-api `Filter.kt:17-21`): `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2`.
- Mapping (`:166-181`): `0 -> TriState.DISABLED`, `1 -> ENABLED_IS`, `2 -> ENABLED_NOT`; any other raw value throws `IllegalStateException("Unknown TriState state: $this")` (`:171`). Reverse mapping: `DISABLED -> 0`, `ENABLED_IS -> 1`, `ENABLED_NOT -> 2`.
- Cycle (`TriState.next()`, `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:9-15`): `DISABLED -> ENABLED_IS -> ENABLED_NOT -> DISABLED`.
- `TriStateItem` click (`presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:355-375`) computes the next state and calls the `onClick` callback with it. The model then applies `toTriStateInt()`.

### 6.3 Select, Text, CheckBox

- Select: `SelectItem` (`presentation-core/.../SettingsItems.kt:302-330`) shows `options[selectedIndex].toString()` in a read-only dropdown. An out-of-range `selectedIndex` throws (`options[selectedIndex]`). Proposal: clamp to `0` when out of range.
- Text: `TextItem` (`SettingsItems.kt:556-571`) single-line field; `onValueChange` writes `state` immediately (every keystroke calls `onUpdate`, so every keystroke calls `setFilters`, a state update only; no search).
- CheckBox: `CheckboxItem(label, checked)` toggles.

### 6.4 Filter count and visibility rules

- The filter chip is shown only when `state.filters.isNotEmpty()` (`BrowseMangaSourceScreen.kt:195`). A source whose filters are all headers and separators shows the chip and an empty sheet.
- The filter sheet has no "clear count" badge; the chip is selected when `listing is Search`.

---

## 7. Global search screen wrapper (GlobalMangaSearchScreen)

Source: `SRC/globalsearch/GlobalMangaSearchScreen.kt`.

Steps (`:26-81`):
1. `if (!ifMangaSourcesLoaded()) LoadingScreen()` (`:27-30`).
2. `rememberScreenModel { GlobalMangaSearchScreenModel(initialQuery = searchQuery, initialExtensionFilter = extensionFilter) }` (`:34-39`).
3. `showSingleLoadingScreen` (`:41-45`) initial value: `searchQuery.isNotEmpty() && !extensionFilter.isNullOrEmpty() && state.total == 1`.
4. If `showSingleLoadingScreen` (`:47-64`):
   - Show `LoadingScreen()`.
   - `LaunchedEffect(state.items)`: look at `state.items.values.singleOrNull()`:
     - `Loading` -> wait (return; re-runs on next items change).
     - `Success` with exactly one manga -> `navigator.replace(MangaScreen(manga.id, true))` (`:56`). The second parameter is `fromSource = true`.
     - `Success` with zero or several manga -> `showSingleLoadingScreen = false` (show results).
     - `else` (Error, or `singleOrNull()` is null because there is not exactly one source) -> `showSingleLoadingScreen = false`.
5. Else `GlobalMangaSearchScreen(state, navigateUp = pop, onChangeSearchQuery = updateSearchQuery, onSearch = { search() }, getManga = { getManga(it) }, onChangeSearchFilter = setSourceFilter, onToggleResults = toggleFilterResults, onClickSource = push BrowseMangaSourceScreen(it.id, state.searchQuery), onClickItem = push MangaScreen(it.id, true), onLongClickItem = push MangaScreen(it.id, true))` (`:66-79`).

Observed details:
- `onClickSource` passes `state.searchQuery` (the text currently in the field, even if it has not been submitted) as the browse listing query (`:75`). `BrowseMangaSourceScreen` then starts a `Search` with that text.
- `onLongClickItem` opens the manga too (`:78`). No long-click action exists on this screen.
- `searchQuery` (constructor) may be `""`. `extensionFilter` may be `null`.

---

## 8. Global search model (GlobalMangaSearchScreenModel)

Source: `SRC/globalsearch/GlobalMangaSearchScreenModel.kt:5-28`.

- Constructor (`:5-12`): `initialQuery: String = ""`, `initialExtensionFilter: String? = null`. Calls the base with `State(searchQuery = initialQuery)` (so `sourceFilter` = base default `PinnedOnly`).
- `init` (`:14-23`):
  1. `extensionFilter = initialExtensionFilter` (`:15`).
  2. If `initialQuery.isNotBlank() || !initialExtensionFilter.isNullOrBlank()`:
     - If `extensionFilter != null` (note: `""` also counts, `:17`): `setSourceFilter(MangaSourceFilter.All)` (`:18-20`). This calls `search()`.
     - `search()` (`:21`). Returns immediately if the query is blank (`MangaSearchScreenModel.kt:128`), so an extension filter with a blank query does nothing.
- `getEnabledSources()` override (`:25-28`): `super.getEnabledSources().filter { state.value.sourceFilter != PinnedOnly || "${it.id}" in pinnedSources }`.
- `GlobalMangaSearchScreen(query)` without an extension filter searches pinned sources only by default (see 9.7).

---

## 9. Global search base model (MangaSearchScreenModel): the algorithm

Source: `SRC/globalsearch/MangaSearchScreenModel.kt` (235 lines). This is the core of this area.

### 9.1 Construction and state

- Abstract class, `StateScreenModel<State>` (`:36-44`). Constructor defaults: `sourceManager = Injekt.get()`, `extensionManager`, `networkToLocalManga`, `getManga`, `preferences = Injekt.get()` (`SourcePreferences`).
- `coroutineDispatcher = Executors.newFixedThreadPool(5).asCoroutineDispatcher()` (`:46`). Never shut down (OBSERVED: thread leak per screen model; Proposal: a `tokio::sync::Semaphore` with 5 permits, section 5 of p2-rust-design.md).
- `searchJob: Job?` (`:47`).
- Snapshots (`:49-51`): `enabledLanguages`, `disabledSources`, `pinnedSources` (protected).
- `lastQuery: String?`, `lastSourceFilter: MangaSourceFilter?` (`:53-54`).
- `extensionFilter: String?` (`:56`, protected var, set by subclasses).
- `sortComparator` (`:58-64`): `compareBy<CatalogueSource>( { (map[it] as? Success)?.isEmpty ?: true }, { "${it.id}" !in pinnedSources }, { "${it.name.lowercase()} (${it.lang})" } )`.
- `init` (`:66-72`): `preferences.globalSearchFilterState().changes().collectLatest { onlyShowHasResults = it }`.

State (`:199-210`):
- `fromSourceId: Long?` (null), `searchQuery: String?` (null), `sourceFilter: MangaSourceFilter = PinnedOnly`, `onlyShowHasResults: Boolean = false`, `items: PersistentMap<CatalogueSource, MangaSearchItemResult> = {}`.
- Derived: `progress = items.count { it.value !is Loading }`, `total = items.size`, `filteredItems = items.filter { it.value.isVisible(onlyShowHasResults) }`.

`MangaSourceFilter` (`:213-216`): `All`, `PinnedOnly`.
`MangaSearchItemResult` (`:218-235`): `Loading` (object), `Error(throwable)`, `Success(result: List<Manga>)` with `isEmpty`. `isVisible(onlyShowHasResults) = !onlyShowHasResults || (this is Success && !this.isEmpty)` (`:232-234`).

### 9.2 Source selection

`getEnabledSources()` (`:85-94`, open):
- `sourceManager.getCatalogueSources()` (all catalogue sources currently registered).
- `filter { it.lang in enabledLanguages && "${it.id}" !in disabledSources }`.
- `sortedWith(compareBy({ "${it.id}" !in pinnedSources }, { "${it.name.lowercase()} (${it.lang})" }))`.
- Note: the local source (`lang = "other"`) is included only if `"other"` is in `enabledLanguages`. It is not forced in (unlike the Sources page, section 2.2).

`getSelectedSources()` (`:96-109`, private):
- `extensionFilter` null or empty -> `getEnabledSources()` (open, so subclasses apply).
- Else `extensionManager.installedExtensionsFlow.value.filter { it.pkgName == filter }.flatMap { it.sources }.filterIsInstance<CatalogueSource>().filter { it in enabledSources }`. The extension's sources, intersected with the enabled set (which, for the global model, includes the `PinnedOnly` filter).

### 9.3 search() exact algorithm

Source `:124-180`.

```
query = state.searchQuery
sourceFilter = state.sourceFilter
if query.isNullOrBlank(): return                               // L128
sameQuery = (lastQuery == query)
if sameQuery && lastSourceFilter == sourceFilter: return       // L129-130 (no-op)
lastQuery = query; lastSourceFilter = sourceFilter             // L132-133
searchJob?.cancel()                                            // L135
sources = getSelectedSources()                                 // L136
if sameQuery:
    items = sources.associateWith { existing[it] ?: Loading }  // L139-145 (keep finished results)
else:
    items = sources.associateWith { Loading }                  // L147-151
updateItems(items)                                             // sorts (9.5)
searchJob = ioCoroutineScope.launch {                          // L153
    sources.map { source -> async {                            // L154-155
        if (state.value.items[source] !is Loading) return      // L156-158
        try {
            page = withContext(pool5) {                        // L160-162
                source.getSearchManga(1, query, source.getFilterList())
            }
            titles = page.mangas.map {
                networkToLocalManga.await(it.toDomainManga(source.id))   // L164-166 (DB insert/read)
            }
            if (isActive) updateItem(source, Success(titles))  // L168-170
        } catch (e: Exception) {
            if (isActive) updateItem(source, Error(e))         // L171-175
        }
    } }.awaitAll()                                             // L177-178
}
```

Key facts:
- Only page 1 of each source is fetched (`getSearchManga(1, ...)`). There is no "load more" for global search.
- Filters: `source.getFilterList()` (fresh default values). The global search never uses user-set filter values.
- The query is sent as typed (no trimming). `isNullOrBlank` rejects whitespace-only queries.
- `search()` is only triggered by: the search action (`GlobalMangaSearchScreen.kt:70`), `setSourceFilter` (`:115-118`), and init (`GlobalMangaSearchScreenModel.kt:21`). There is no debounce. Typing does not search.
- `sameQuery` + same filter -> nothing happens (a second search press with the same text is a no-op, so an errored source is never retried without changing the query or the source filter).
- Changing the source filter with the same query keeps finished results and fetches only the sources that are newly selected (their state is `Loading`), because the per-source check at `:156-158` returns early for non-Loading entries.
- Results of sources that are no longer selected are dropped by `associateWith` (`:142-144`).

### 9.4 Result update and the race (OBSERVED)

`updateItem(source, result)` (`:192-197`):
```
newItems = state.value.items.mutate { it[source] = result }   // snapshot read
updateItems(newItems)                                         // mutableState.update { copy(items = sorted(newItems)) }
```
- Multiple per-source `async` blocks call `updateItem` concurrently on the IO pool (`ioCoroutineScope`). The read of `state.value.items` and the write are not atomic. Two results finishing together can each read the same snapshot; the second write overwrites the first. The lost source stays `Loading` (and `progress` is wrong) until the next search.
- The window is small but real (both calls run after their network I/O on Dispatchers.IO threads).

Proposal: a single writer. Results arrive as events to the GTK main thread with `(generation, source, result)`, and only the main thread mutates the model. Section 9 of p2-rust-design.md.

### 9.5 Ordering of the result map

`updateItems(items)` (`:182-190`): `items.toSortedMap(sortComparator(items)).toPersistentMap()`. Sort key (`sortComparator`, `:58-64`), lexicographic:
1. Results that are not a non-empty `Success` first. Key value: `(result as? Success)?.isEmpty ?: true`. So non-empty `Success` = `false` (first). `Loading`, `Error`, and empty `Success` all = `true` (later, and tied).
2. Pinned sources first (`"${id}" !in pinnedSources` is `false` for pinned).
3. Name: `"${name.lowercase()} (${lang})"`, Java `String.compareTo` (code-unit order).

Consequences:
- The list reorders as results arrive: a result that becomes non-empty jumps to the top of the non-empty group.
- Within the `true` group, pinned sources go first, including those still `Loading`.

Edge case (OBSERVED, data loss): `toSortedMap` with this comparator treats two sources with the same `(emptiness, pinned, name (lang))` as equal keys. The `TreeMap` keeps one of them and drops the other. Possible when two installed sources have the same name and language (for example the same site from two repositories). Effect: that source vanishes from the results and `total` is one less. Proposal: the Rust model stores a `Vec` in insertion order and sorts it with a stable sort; nothing is dropped (edge case GS-EC-04).

### 9.6 The filtered view

- `filteredItems` (`:209`) applies `isVisible(onlyShowHasResults)`. With `onlyShowHasResults = true`: only non-empty `Success` results are shown. `Loading` and `Error` rows disappear.
- `progress` / `total` (`:207-208`) drive the progress bar (part 1 section 10.1): shown while `1 <= progress < total`, value `progress / total`.

### 9.7 Source filter (PinnedOnly / All) and extension filter

- `setSourceFilter(filter)` (`:115-118`): sets `sourceFilter` and calls `search()`. Same query -> only new sources are fetched (9.3).
- `toggleFilterResults()` (`:120-122`): `preferences.globalSearchFilterState().toggle()` (writes `__APP_STATE_has_filters_toggle_state`, default `false`; `SourcePreferences.kt:49-52`). The state is refreshed from the change flow (`:68-70`). The key is shared by the global and migrate search screens.
- Default: `PinnedOnly` (`:203`). A new global search shows only pinned sources. With no pinned sources, nothing is searched; the results area is empty. OBSERVED from the code (`GlobalMangaSearchScreenModel.getEnabledSources`, `:25-28`). Verify on device before relying on it.
- The `All` option is only forced by an extension filter (`GlobalMangaSearchScreenModel.kt:17-19`).

### 9.8 Query update

`updateSearchQuery(query)` (`:111-113`): `searchQuery = query` (state only, no search). `query` may be `null`.

### 9.9 getManga composable

`@Composable getManga(initialManga): State<Manga>` (`:74-83`): `produceState` over `getManga.subscribe(url, source).filterNotNull().collectLatest`. Used by result rows so that the favorite badge updates live (part 1 section 10.3).

---

## 10. Migrate search subclass (outside scope, referenced for ordering)

`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/migration/search/MigrateMangaSearchScreenModel.kt`:
- `MigrateMangaSearchScreenModel(mangaId, initialExtensionFilter = "", getManga)` extends the base with default state (so `sourceFilter = PinnedOnly`).
- `init` (`:19-32`): `extensionFilter = initialExtensionFilter`; loads the manga `getManga.await(mangaId)!!` (NPE when missing; Proposal: `Option`); sets `fromSourceId = manga.source`, `searchQuery = manga.title`; calls `search()`.
- `getEnabledSources()` override (`:34-44`): applies `PinnedOnly` filter and then `sortedWith(compareBy({ it.id != fromSourceId }, { !pinned }, { name (lang) }))`, i.e. origin source first.
- OBSERVED: this origin-first order is discarded. `search()` builds a map and `updateItems` sorts it with the base `sortComparator` (9.5), which does not know `fromSourceId`. The origin source ends up where the base comparator puts it (non-empty results first, then pinned, then name). Part 1 section 10.4 describes origin-first; that description does not hold for the current code. Proposal (T-40b-067): add the origin key as the first sort key in the migrate model's comparator, so the documented intent is implemented.

---

## 11. Migrate source picker: the list of sources with favorite counts

Files: `MIG/MigrateMangaSourceScreenModel.kt`, `MIG/MigrateMangaSourceTab.kt`, `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt`, `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt`.

### 11.1 Data

`GetMangaSourcesWithFavoriteCount.subscribe()` (`GetMangaSourcesWithFavoriteCount.kt:18-28`):
- Input: `repository.getMangaSourcesWithFavoriteCount()` (`data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:41-55`).
  - SQL (`data/src/main/sqldelight/data/mangas.sq`, `getSourceIdWithFavoriteCount`): `SELECT source, count(*) FROM mangas WHERE favorite = 1 GROUP BY source;` (no ORDER BY).
  - Each row: `source = sourceManager.getOrStub(sourceId)`; `domainSource = map(source).copy(isStub = source is StubMangaSource)`; `(domainSource, count)`.
  - Re-emits when the `mangas` table changes (`handler.subscribeToList`) and when `catalogueSources` changes (`combine`, value unused).
- Filter: `filterNot { it.first.id == LocalMangaSource.ID }` (`:25`). The local source is never listed.
- Sort: `sortedWith(sortFn(direction, mode))` (`:26`, `:30-57`).

`mapSourceToDomainSource` sets `supportsLatest = false` here (`MangaSourceRepositoryImpl.kt:90-96`); not used on this screen.

### 11.2 Sort function (exact)

`sortFn(direction, sorting)` (`:30-57`): base comparator `sortFn(a, b)`:
- `ALPHABETICAL`: `if a.isStub && !b.isStub -> -1`; `if b.isStub && !a.isStub -> 1`; else `a.name.lowercase().compareToWithCollator(b.name.lowercase())` (`:37-41`).
- `TOTAL`: same stub rule; else `a.count.compareTo(b.count)` (`:44-48`). Ascending count.
- Direction: `ASCENDING` -> `Comparator(sortFn)` (`:54`); `DESCENDING` -> `Collections.reverseOrder(sortFn)` (`:55`).

Consequences:
- Stub sources are first in ASCENDING and last in DESCENDING (the reverse applies to the stub rule too).
- Ties (equal names or equal counts) keep the input order (stable sort). The input is the SQL result order, which SQLite does not guarantee without `ORDER BY`. Proposal: `ORDER BY source` in the Rust query so ties are deterministic.
- `compareToWithCollator` (`core/common/src/main/java/tachiyomi/core/common/util/lang/SortUtil.kt:13-15`): `java.text.Collator.getInstance(Locale.getDefault())` with `strength = PRIMARY` (so case and accents are ignored). Proposal: ICU4X collator at primary strength for the user's locale (p2-rust-design.md section 8); fallback to `to_lowercase()` byte compare.

### 11.3 State (MigrateMangaSourceScreenModel)

- `State(isLoading = true, items = [], sortingMode = ALPHABETICAL, sortingDirection = ASCENDING)` (`MIG/MigrateMangaSourceScreenModel.kt:83-91`); `isEmpty = items.isEmpty()`.
- `init` (`:35-59`):
  - `getSourcesWithFavoriteCount.subscribe().catch { logcat(ERROR); _channel.send(FailedFetchingSourcesWithCount) }.collectLatest { items = sources; isLoading = false }` (`:36-50`).
  - `migrationSortingDirection().changes()` -> `sortingDirection` (`:52-54`).
  - `migrationSortingMode().changes()` -> `sortingMode` (`:56-58`).
- Event: `FailedFetchingSourcesWithCount` (`:93-95`). The model's `channel` is `Channel(Int.MAX_VALUE)` (`:32`).
- Presentation (part 1 section 11): `FailedFetchingSourcesWithCount` is consumed by the screen (not in this scope); the Tab does not collect the channel (`MigrateMangaSourceTab.kt:39-49` passes `contentPadding, _`). OBSERVED: the failure event is never shown. Proposal: show `internal_error` (T-40b-074).

### 11.4 Toggles

- `toggleSortingMode()` (`:61-70`): `ALPHABETICAL <-> TOTAL`, written with the current `sortingDirection` from state via `setMigrateSorting.await(newMode, sortingDirection)`.
- `toggleSortingDirection()` (`:72-81`): `ASCENDING <-> DESCENDING`, with the current `sortingMode`.
- `SetMigrateSorting.await(mode, direction)` (`app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:9-12`) writes `pref_migration_sorting` then `pref_migration_direction`. Two writes, not one transaction. Defaults (`SourcePreferences.kt:30-38`): `ALPHABETICAL`, `ASCENDING`.
- Both toggles read the state, not the preferences, so a double tap before the flow emits uses the old values (the second write reverts the first). OBSERVED; Proposal: read from the config store.

### 11.5 Tab

`migrateMangaSourceTab()` (`MIG/MigrateMangaSourceTab.kt:22-50`):
- Title: `MR.strings.label_migration` ("Migrate").
- Action: `migration_help_guide` ("Source migration guide"), icon `Icons.AutoMirrored.Outlined.HelpOutline`, opens `https://aniyomi.org/help/guides/source-migration/` (`:31-37`).
- Row click: `navigator.push(MigrateMangaScreen(source.id))` (`:43-45`). Next step: part 1 (`MigrateMangaScreen`).
- Sort toggles are passed to the presentation (`onToggleSortingDirection`, `onToggleSortingMode`).

---

## 12. Order summary: what the user sees by default

| Screen | Default | Source |
|---|---|---|
| Sources list | Groups: last used, pinned, then languages (code order); names case-insensitive inside | 2.2, 2.3 |
| Sources filter | Enabled languages first (`all`, `en`, system), then `LocaleHelper.comparator`; enabled sources first inside a language | 3.2 |
| Browse source | Popular chip selected; page 1 of popular; 25-item prefetch | 4.1, 4.3 |
| Global search | `PinnedOnly`; has-results off; no results until a search; results sorted non-empty first, pinned, name | 9.1, 9.5, 9.7 |
| Migrate sources | Alphabetical, ascending; stubs first | 11.2 |
| Add to library (default prefs) | `default_category = -1` = always ask (chooser when user categories exist) | 4.8.2 |

---

## 13. Concurrency, dispatchers, timings, limits

| Item | Value | Source |
|---|---|---|
| Browse paging page size | 25 | `BrowseMangaSourceScreenModel.kt:112` |
| Androidx paging initial load | 75 (3 x 25, library default) | androidx.paging default, not overridden |
| Global search concurrent source requests | 5 threads (`newFixedThreadPool(5)`) | `MangaSearchScreenModel.kt:46` |
| Global search pages per source | 1 (`getSearchManga(1, ...)`) | `MangaSearchScreenModel.kt:161` |
| Global search debounce | none | `GlobalMangaSearchScreen.kt:70` (direct call) |
| Browse search debounce | none (submit only) | `BrowseMangaSourceScreen.kt:148` |
| Column adaptive minimum | 128 dp | `BrowseMangaSourceScreenModel.kt:134` |
| Sources list/filter/migrate timings | none (flows) | section 2-3, 11 |
| Dispatchers | Main (screen model scope), IO (`ioCoroutineScope`, `launchIO`, `withIOContext`) | section 1.2 |
| Event channels | unbounded (`Int.MAX_VALUE` buffer) for source list and migrate events; rendezvous for browse query events | `MangaSourcesScreenModel.kt:46`, `MigrateMangaSourceScreenModel.kt:32`, `BrowseMangaSourceScreen.kt:330` |

---

## 14. Proposed deviations (Kurobayomi) with reasons

| ID | Kotlin behaviour | Proposal | Reason |
|---|---|---|---|
| D-BR-01 | Sort filter with null state crashes on click (`filter.state!!`) | None selected -> select the clicked index, ascending = true | No crash |
| D-BR-02 | `TriState` raw value other than 0-2 throws | Treat unknown as `DISABLED`, log a warning | No crash on a malformed source |
| D-BR-03 | `searchGenre` Select index is into the string-filtered list | Index into the full `values` list | Correct selection for mixed-type value arrays |
| D-GS-01 | Results list is a sorted map keyed by source (collisions drop entries) | Stable `Vec` sorted by the same key | No silent drops |
| D-GS-02 | Concurrent `updateItem` read-modify-write (lost updates) | Single writer on the GTK thread | Race-free |
| D-GS-03 | Thread pool of 5 never shut down | Semaphore of 5 permits | No thread leak |
| D-MG-01 | Origin-first order discarded by base sort | Origin key first in the migrate comparator | Matches documented intent |
| D-DUP-01 | `LOWER(title)` in SQLite (ASCII only) vs `lowercase()` | Compare in Rust with Unicode lowercase over the favorites set | Consistent with Kotlin `lowercase()` |
| D-SS-01 | Sources page shows empty state before extensions load | Gate on `MangaSourceManager.isInitialized` | No empty flash |
| D-MG-02 | Migrate failure event not shown | Show `internal_error` toast | Errors visible |
| D-FAV-01 | Category write and favorite write unordered | Sequential | Deterministic |
| D-CH-01 | Global `Channel` for browse query events | Direct method on the page | No cross-screen coupling |
| D-FAV-02 | Failed favorite write is only logged; the row keeps the new flag | Toast `internal_error` and roll the row back to the previous favorite flag | Errors visible; no silent divergence |

Each deviation is also tracked as a task in p2-tasks.md.

---

## 15. Drops (SY fork and Android-only behaviour)

- Data saver (`dataSaver`, `dataSaverExcludedSources`, `ToggleExcludeFromMangaDataSaver`, `DataSaver` enum, `dataSaverEnabled`, `isExcludedFromDataSaver`) in `MangaSourcesScreenModel.kt:59-67, 117-121, 143` and `GetEnabledMangaSources.kt:26,40`. Reason: the feature rewrites image URLs through third-party proxies (`bandwidth-hero`, `wsrv.nl`, `resmush.it`, per `SourcePreferences.kt:118-123`). Kurobayomi does not route images through proxies. Task T-40b-082 (P3 drop).
- `assistUrl` / `AssistContentScreen` (`BrowseMangaSourceScreen.kt:47,75,77-79,127-128`): Android assist-content API. Drop (T-40b-080).
- `haptic.performHapticFeedback(LongPress)` (`BrowseMangaSourceScreen.kt:110,247`). Drop (T-40b-081). Replacement: none needed; GTK long press has no haptic.
- `ifMangaSourcesLoaded` as a Compose `remember`: replaced by the page-level gate (section 4.11).

---

## 16. Message keys used by this scope (English text)

| Key | Text | Used at |
|---|---|---|
| `MR.strings.internal_error` | InternalError: Check crash logs for further information | `MangaSourcesFilterScreen.kt:33`, `MangaSourcesTab.kt:78` |
| `MR.strings.action_global_search` | Global search | `MangaSourcesTab.kt:37` |
| `MR.strings.action_filter` | Filter | `MangaSourcesTab.kt:42`, `BrowseMangaSourceScreen.kt:208`, `SourceFilterMangaDialog.kt:66` |
| `MR.strings.action_reset` | Reset | `SourceFilterMangaDialog.kt:53` |
| `MR.strings.popular` | Popular | `BrowseMangaSourceScreen.kt:172` |
| `MR.strings.latest` | Latest | `BrowseMangaSourceScreen.kt:191` |
| `MR.strings.label_migration` | Migrate | `MigrateMangaSourceTab.kt:29` |
| `MR.strings.migration_help_guide` | Source migration guide | `MigrateMangaSourceTab.kt:32` |
| `AYMR.strings.label_manga_sources` | Manga Sources | `MangaSourcesTab.kt:34` |
| `MR.strings.no_results_found` | No results found | `ExceptionFormatter.kt:29` (via NoChaptersException and in part 1 for global search empty) |
| `MR.strings.unknown_error` | Unknown error | part 1 (`GlobalSearchErrorResultItem`) |
| `MR.strings.default_category_summary` | Always ask | `SettingsLibraryScreen.kt:91-96` (value -1 label) |
| `MR.strings.last_used_source` | Last used | `LocaleHelper.kt:30-42` (group label via display name) |
| `MR.strings.pinned_sources` | Pinned | `LocaleHelper.kt:30-42` (group label) |
| `MR.strings.other_source` | Other | `LocaleHelper.kt:30-42` |
| `MR.strings.multi_lang` | Multi | `LocaleHelper.kt:30-42` (for `all`) |
| `MR.strings.not_installed` | Not installed | part 1 (migrate picker stub label) |
| `MR.strings.loader_not_implemented_error` | Source not found | `ExceptionFormatter.kt:31-33` (SourceNotInstalledException) |
| `MR.strings.source_filter_empty_screen` | No installed source found | part 1 |
| `MR.strings.source_empty_screen` | No source found | part 1 |
| `MR.strings.action_pin` / `action_unpin` / `action_disable` | Pin / Unpin / Disable | part 1 options dialog |
| `MR.strings.action_sort_alpha` / `action_sort_count` / `action_asc` / `action_desc` | Alphabetically / Total entries / Ascending / Descending | part 1 migrate header |
| `MR.strings.are_you_sure` / `MR.strings.remove_manga` | Are you sure? / You are about to remove "%s" from your library | part 1 `RemoveEntryDialog` (used by `BrowseMangaSourceScreen.kt:289-297`) |
| `MR.strings.has_results` | Has results | part 1 global search chip |
| `MR.strings.exception_offline` | No Internet connection | `ExceptionFormatter.kt` (any browse error) |
| `MR.strings.exception_http` | HTTP %d, check website in WebView | `ExceptionFormatter.kt` |
| `MR.strings.exception_unknown_host` | Couldn't reach %s | `ExceptionFormatter.kt` |

Strings not listed here are owned by part 1 or other areas.

---

## 17. Interactions with other areas

- area:02-preferences-system: `SourcePreferences`, `LibraryPreferences` read/write, `Preference.appStateKey` prefix `__APP_STATE_` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-48`).
- area:05 / area:06: `MangaRepository.getMangaByUrlAndSourceId`, `insertManga`, `getDuplicateLibraryManga`, `setMangaCategories`, `getSourceIdWithFavoriteCount`.
- area:08: category repository, default chapter flags (`SetMangaDefaultChapterFlags`), `GetMangaCategories`.
- area:12 / area:15: `CatalogueSource.getSearchManga`, `getPopularManga`, `getLatestUpdates`, `getFilterList`, `supportsLatest`.
- area:17 / area:18 / area:20: source manager (`AndroidMangaSourceManager`), extension manager (`installedExtensionsFlow`, `getExtensionPackage`), stub repository.
- area:27 / area:33: enhanced tracker binding on add (`AddMangaTracks.bindEnhancedTrackers`).
- area:34: cover cache (`MangaCoverCache.deleteFromCache`) on removal.
- area:36: deep-link and intent entry points (`MainActivity` search actions).
- area:37: library column preferences and the library's search (`MangaLibraryTab`).
- area:38: manga page (`MangaScreen`) which pushes these screens and receives genre search.
- Migration (part 1 of this area, `p1-behavior.md` section 14-15): `MigrateMangaDialog` and the migrate search wiring. The part-2 file `MigrateMangaSearchScreenModel` is covered in section 10 of this file.
- Part 1 (`p1-*.md`): presentation, extension tab, details, migrate favorites and search presentation.

---

## 18. Quick reference: functions and their lines

| Function | Location |
|---|---|
| `MangaSourcesScreenModel.collectLatestSources` | `MangaSourcesScreenModel.kt:71-107` |
| `GetEnabledMangaSources.subscribe` | `GetEnabledMangaSources.kt:18-51` |
| `MangaSourcesFilterScreenModel` init | `MangaSourcesFilterScreenModel.kt:27-51` |
| `GetLanguagesWithMangaSources.subscribe` | `GetLanguagesWithMangaSources.kt:16-33` |
| `BrowseMangaSourceScreenModel.init` | `BrowseMangaSourceScreenModel.kt:81-103` |
| `mangaPagerFlowFlow` | `BrowseMangaSourceScreenModel.kt:109-125` |
| `search(query, filters)` | `BrowseMangaSourceScreenModel.kt:167-182` |
| `searchGenre(genreName)` | `BrowseMangaSourceScreenModel.kt:184-227` |
| `changeMangaFavorite` | `BrowseMangaSourceScreenModel.kt:234-253` |
| `addFavorite` | `BrowseMangaSourceScreenModel.kt:255-288` |
| Long-click decision | `BrowseMangaSourceScreen.kt:232-249` |
| `FilterItem` | `SourceFilterMangaDialog.kt:79-164` |
| `MangaSearchScreenModel.search` | `globalsearch/MangaSearchScreenModel.kt:124-180` |
| `updateItems` / `updateItem` | `globalsearch/MangaSearchScreenModel.kt:182-197` |
| `GlobalMangaSearchScreen` single-result logic | `globalsearch/GlobalMangaSearchScreen.kt:41-64` |
| `MigrateMangaSourceScreenModel` toggles | `MIG/MigrateMangaSourceScreenModel.kt:61-81` |
| `GetMangaSourcesWithFavoriteCount.sortFn` | `GetMangaSourcesWithFavoriteCount.kt:30-57` |
