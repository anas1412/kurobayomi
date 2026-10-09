# Area 17 — App source layer: behaviour specification

All paths are repo-relative to `/home/blackbox/aniyomi-src`. Labels: **OBSERVED** = what the Kotlin does (with `path:line`). **Proposal:** = what the Rust port should do. Source-id values are signed 64-bit (`Long`); in Rust use `i64`.

Deep dives: `p1-topic-registry-and-stubs.md` (managers and stubs), `p1-topic-enable-pin-language-incognito.md` (state and lists), `p1-topic-migration-sorting-and-favorites.md` (migration list), `p1-topic-remote-entry-update.md` (UpdateAnimeFromRemote).

---

## 0. Vocabulary

| Term | Meaning (OBSERVED) |
|---|---|
| Live source | An object of type `MangaSource`/`CatalogueSource`/`HttpSource` (manga) or `AnimeSource`/`AnimeHttpSource` (anime) provided by an installed extension, or the local source with id `0`. Held in `sourcesMapFlow`. |
| Stub source | `StubMangaSource` / `StubAnimeSource`: `(id, lang, name)` only. Every network method throws `SourceNotInstalledException` / `AnimeSourceNotInstalledException` (`domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt:17-24`, `domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:25-57`). |
| Domain `Source` | `data class Source(id, lang, name, supportsLatest, isStub, pin: Pins = Pins.unpinned, isUsedLast = false, isExcludedFromDataSaver = false)` (`domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14`). Anime: same without `isExcludedFromDataSaver` (`domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:3-11`). |
| Enabled source | Live or stub row whose `lang` is in `source_languages` (or id == `0`) and whose id string is NOT in the disabled set. |
| Pinned | id string in `pinned_catalogues` / `pinned_anime_catalogues`. |
| Last used | id equal to `last_catalogue_source` / `last_anime_catalogue_source` (default `-1`, i.e. none). |

---

## 1. Source registry (`AndroidMangaSourceManager`, `AndroidAnimeSourceManager`)

### 1.1 Fields and state

OBSERVED (manga; anime identical except noted):
- `_isInitialized = MutableStateFlow(false)`; public `isInitialized: StateFlow<Boolean>` (`app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt:35-36`).
- `downloadManager` by `injectLazy()` (`:38`).
- `scope = CoroutineScope(Job() + Dispatchers.IO)` (`:40`).
- `sourcesMapFlow = MutableStateFlow(ConcurrentHashMap<Long, MangaSource>())` (`:42`).
- `stubSourcesMap = ConcurrentHashMap<Long, StubMangaSource>()` (`:44`).
- Manga exposes `catalogueSources: Flow<List<CatalogueSource>>` = `sourcesMapFlow.map { values.filterIsInstance<CatalogueSource>() }` (`:46-48`). Anime exposes `sources: Flow<List<AnimeSource>>` = `sourcesMapFlow.map { values.toList() }` (`app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:45`).

### 1.2 Init coroutine A: live sources from installed extensions

OBSERVED (`AndroidMangaSourceManager.kt:50-72`; anime `AndroidAnimeSourceManager.kt:47-72`):
1. `scope.launch { extensionManager.installedExtensionsFlow.collectLatest { extensions -> ... } }`.
2. Each emission builds a NEW `ConcurrentHashMap` seeded with the local source: manga `LocalMangaSource.ID to LocalMangaSource(context, Injekt.get(), Injekt.get())`; anime `LocalAnimeSource.ID to LocalAnimeSource(context, Injekt.get() x5)`.
3. For each extension, for each `extension.sources`: `mutableMap[it.id] = it` (a later duplicate id silently replaces an earlier one, and an extension source with id `0` replaces the local source). Then `registerStubSource(StubMangaSource.from(it))` (anime: `StubAnimeSource.from`).
4. `sourcesMapFlow.value = mutableMap`, then `_isInitialized.value = true`.

Notes OBSERVED: `collectLatest` cancels the previous block when a new extension list arrives, but the `registerStubSource` coroutines are launched on `scope` (not a child of the collector), so they are NOT cancelled.

Proposal: the registry consumes a `watch::Receiver<Arc<Vec<ExtensionSnapshot>>>` from the extension manager (area 18-20). On each change: build the map (local first, then extensions in extension order; later duplicates replace earlier ones, the same as Kotlin), publish it atomically (copy-on-write), mark initialized, then `tokio::spawn` one detached registration task per source (not aborted by the next change). Abort only the map-building step, never the detached tasks.

### 1.3 Init coroutine B: stub rows from the database (quirk)

OBSERVED (`AndroidMangaSourceManager.kt:74-82`; anime `AndroidAnimeSourceManager.kt:74-82`):
```kotlin
scope.launch {
    sourceRepository.subscribeAllManga()
        .collectLatest { sources ->
            val mutableMap = stubSourcesMap.toMutableMap()
            sources.forEach {
                mutableMap[it.id] = it
            }
        }
}
```
`mutableMap` is never written back to `stubSourcesMap`. Consequence: the stub cache is filled only by `createStubSource` through `getOrStub`, never by the DB subscription. `getStubSources()` therefore lists only stubs that were requested this session (or created from extension index data). This is a bug.

Proposal: the registry loads the whole `sources` (manga) / `animesources` (anime) table into memory at startup and on each DB change (table is small), replaces the stub map wholesale. `get_stub_sources()` then lists every persisted stub. Mark as deliberate fix in the Rust docs (T-17a-034).

### 1.4 Lookup functions

| Function (manga/anime) | OBSERVED body | Proposal |
|---|---|---|
| `get(sourceKey)` | `sourcesMapFlow.value[sourceKey]` (`AndroidMangaSourceManager.kt:85-87`; anime `:85-87`) | `live.get(id)` returns `Option` |
| `getOrStub(sourceKey)` | `sourcesMapFlow.value[sourceKey] ?: stubSourcesMap.getOrPut(sourceKey) { runBlocking { createStubSource(sourceKey) } }` (`:89-93`) | Pure memory lookup: live, else memory stub map, else compute fallback (1.6) and insert. No DB call on the caller thread. |
| `getOnlineSources()` manga | `sourcesMapFlow.value.values.filterIsInstance<HttpSource>()` (`:95`) | `live.values().filter(is_http)` |
| `getOnlineSources()` anime | `filterIsInstance<AnimeHttpSource>()` (`AndroidAnimeSourceManager.kt:97-99`) | same |
| `getCatalogueSources()` manga only | `filterIsInstance<CatalogueSource>()` (`:97`) | same |
| `getAll()` anime only | `sourcesMapFlow.value.values.toList()` (`:95`) | `live.values()` |
| `getStubSources()` manga | `val onlineSourceIds = getOnlineSources().map { it.id }; stubSourcesMap.values.filterNot { it.id in onlineSourceIds }` (`:99-102`) | exclude ids that are live HTTP sources |
| `getStubSources()` anime | excludes `getAll()` ids (all live sources, incl. local) (`AndroidAnimeSourceManager.kt:101-104`) | exclude all live ids (anime rule, which also excludes local id 0 — local is never a stub anyway) |

Note on `getOrStub` OBSERVED: `runBlocking` inside `getOrPut` blocks the calling thread on a DB read the first time an unknown id is seen. Proposal: forbidden on the GTK thread (see section 13).

### 1.5 `registerStubSource(source)` (OBSERVED, both)

```kotlin
private fun registerStubSource(source: StubMangaSource) {
    scope.launch {
        val dbSource = sourceRepository.getStubMangaSource(source.id)
        if (dbSource == source) return@launch
        sourceRepository.upsertStubMangaSource(source.id, source.lang, source.name)
        if (dbSource != null) {
            downloadManager.renameSource(dbSource, source)
        }
    }
}
```
(`AndroidMangaSourceManager.kt:104-113`; anime `AndroidAnimeSourceManager.kt:106-115`.)

Steps:
1. Read the stored row `dbSource` (`SELECT * FROM sources WHERE _id = :id`).
2. Skip if `dbSource == source`. **OBSERVED DEFECT:** `StubMangaSource` and `StubAnimeSource` are plain classes with no `equals` override (`domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt:9-13`, `domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:15-19`), so the check is reference equality and is always false. Every emission of `installedExtensionsFlow` therefore upserts every installed source and calls `renameSource` for every row that existed.
3. Upsert `(id, lang, name)`.
4. If a row existed, call `renameSource(old, new)`. `renameSource` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:352-368`) resolves the old folder via `findSourceDir(old)`, returns if the new dir name equals the old folder name, handles a capitalisation-only change through a temporary `TMP_DIR_SUFFIX` folder, and otherwise calls `renameTo`. Failures are logged (`LogPriority.ERROR`) and not thrown.

Proposal: `#[derive(PartialEq, Eq)]` on `StubSource` so step 2 works; this removes the redundant write and the redundant rename call. Behaviour difference: none for valid data, because the rename is a no-op when names match.

Race (OBSERVED, not guarded): two emissions processed concurrently both read the same old row, both upsert, and both call `renameSource(old, new)`. The second rename fails because the first already moved the folder (logged, not thrown). Proposal: serialize registrations per registry with a `tokio::sync::Mutex<()>` around read-compare-write-rename.

### 1.6 `createStubSource(id)` (OBSERVED, both)

```kotlin
private suspend fun createStubSource(id: Long): StubMangaSource {
    sourceRepository.getStubMangaSource(id)?.let { return it }
    extensionManager.getSourceData(id)?.let { registerStubSource(it); return it }
    return StubMangaSource(id = id, lang = "", name = "")
}
```
(`AndroidMangaSourceManager.kt:115-124`; anime `AndroidAnimeSourceManager.kt:117-126`.)

Chain: (a) persisted row; (b) `getSourceData(id)` = a `StubMangaSource` built from the AVAILABLE extension index (`MangaExtensionManager.kt:111-120`: `availableExtensionsSourcesData` is set only from the remote index and is empty until it loads), and it is registered (persisted); (c) a fabricated `StubMangaSource(id, "", "")` that is NOT persisted.

Proposal: same chain, but (a) reads the in-memory map loaded at startup (1.3). Case (c) is cached in memory only, so a later index load can still upgrade it: when the index loads, the registry re-evaluates cached entries whose lang and name are empty.

### 1.7 Start-up gate

OBSERVED: `isInitialized` is false until the first `installedExtensionsFlow` emission has been published (`AndroidMangaSourceManager.kt:70`). UI screens show a loading state until then (`app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt`, `AnimeSourceUtil.kt`, each returning `isInitialized` as Compose state). Warm-up: `AppModule.kt:242-251` calls `get<MangaSourceManager>()` and `get<AnimeSourceManager>()` on the main executor after `App.onCreate`, so the managers start loading before the first screen.

Proposal: `SourceGate` (`kuro-core::sources`) is a `tokio::sync::watch::Receiver<bool>`. The registry is constructed during app start on a background task; the UI awaits `changed()` through `glib::spawn_future_local` and shows a `gtk::Spinner` page until `true`.

### 1.8 Local source identity

OBSERVED: `LocalMangaSource.ID = 0L` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:378`); `LocalAnimeSource.ID` likewise (`.../anime/LocalAnimeSource.kt`). Both sources have `lang = "other"` (`LocalMangaSource.kt:72`, `LocalAnimeSource.kt:68`) and names from `AYMR.strings.local_manga_source` = "Local manga source" and `AYMR.strings.local_anime_source` = "Local anime source" (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:495-496`). Local source implements `CatalogueSource` and `UnmeteredSource` (`LocalMangaSource.kt:53-59`). `isLocal()` = `id == LocalMangaSource.ID` (`LocalMangaSource.kt:387`).

Lists exclude the local source only in the migration list (see topic file). It is included in enabled sources (`lang in enabledLanguages || id == LocalMangaSource.ID`) even when `"other"` is not an enabled language.

---

## 2. Extension helpers (`MangaSourceExtensions.kt`, `AnimeSourceExtensions.kt`)

### 2.1 `getPreferenceKey()` = `"source_$id"` (OBSERVED, `MangaSourceExtensions.kt:14`, `AnimeSourceExtensions.kt:14`). Used as the per-source preference namespace (for source-defined preferences) and as the backup key. Proposal: `format!("source_{id}")`, identical string for backup compatibility (see data file).

### 2.2 `toStubSource()`: `StubMangaSource(id, lang, name)` (`MangaSourceExtensions.kt:16`; anime `AnimeSourceExtensions.kt:16`).

### 2.3 `getNameForMangaInfo()` / `getNameForAnimeInfo()` (OBSERVED, identical logic)

```kotlin
val enabledLanguages = preferences.enabledLanguages().get()
    .filterNot { it in listOf("all", "other") }
val hasOneActiveLanguages = enabledLanguages.size == 1
val isInEnabledLanguages = lang in enabledLanguages
return when {
    hasOneActiveLanguages && !isInEnabledLanguages -> toString()
    hasOneActiveLanguages && isInEnabledLanguages -> name
    else -> toString()
}
```
(`MangaSourceExtensions.kt:18-31`; `AnimeSourceExtensions.kt:18-31`.)

Result table (with `toString()` = `"$name (LANG)"` or the id, see 2.5):

| enabled languages after removing `all`/`other` | source lang enabled | output |
|---|---|---|
| exactly 1 | yes | `name` (no language tag) |
| exactly 1 | no | `toString()` (tagged) |
| 0 or 2+ | any | `toString()` (tagged) |

Proposal: identical function in `kuro-sources::naming::display_name_for_info(source, enabled_langs)`.

### 2.4 `isLocalOrStub()` = `isLocal() || this is Stub` (`MangaSourceExtensions.kt:33`; `AnimeSourceExtensions.kt:33`).

### 2.5 `toString()` of stubs (OBSERVED, `StubMangaSource.kt:26-27`, `StubAnimeSource.kt:59-60`)

`if (!isInvalid) "$name (${lang.uppercase()})" else id.toString()`, where `isInvalid = name.isBlank() || lang.isBlank()` (`StubMangaSource.kt:15`). So a stub with blank name or lang renders as its numeric id.

### 2.6 `icon()` (OBSERVED)

`MangaSource.icon(): Drawable? = Injekt.get<MangaExtensionManager>().getAppIconForSource(this.id)` (`MangaSourceExtensions.kt:12`). `getAppIconForSource` finds the installed extension that owns the id and returns the package icon (`MangaExtensionManager.kt:97-109`), caching per `pkgName` in `iconMap` (`MangaExtensionManager.kt:63`). Returns null for local and stub sources. The `!!` on `getMangaExtensionPackageInfoFromPkgName` (`MangaExtensionManager.kt:108`) throws if the package was uninstalled between the lookup and the load.

Proposal: `icon_for_source(id) -> Option<gdk::Texture>` loaded from the extension package icon file (area 18-20 stores it on disk); cache per package; on any failure return `None`; the UI then uses the freedesktop name `application-x-addon-symbolic` (fallback chosen here, not in Kotlin).

### 2.7 `isSourceForTorrents()` (OBSERVED, `AnimeSourceExtensions.kt:35-40`)

```kotlin
fun AnimeSource?.isSourceForTorrents(): Boolean {
    if (this == null || this.isLocalOrStub()) return false
    val sourceUsed = Injekt.get<AnimeExtensionManager>().installedExtensionsFlow.value
        .find { ext -> ext.sources.any { it.id == this.id } }!!
    return sourceUsed.isTorrent
}
```
Steps: null or local or stub → false; find owning installed extension; `!!` throws `NullPointerException` if none (race between a check and a removal); return its `isTorrent` flag.

Proposal: `fn is_source_for_torrents(src: Option<&dyn AnimeSourceLike>, exts: &ExtensionSnapshot) -> bool`; a missing owner returns `false`, never panics.

---

## 3. Domain `Source` values and keys

OBSERVED (`domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:16-27`):
- `visualName`: if `lang.isEmpty()` → `name`; else `"$name (${lang.uppercase()})"`.
- `key: () -> String` (a lambda property, not a function): `isUsedLast` → `"$id-lastused"`; else `"$id"`.

The anime `AnimeSource` has the same `visualName` and `key` (`domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:13-24`).

Consumers: `MangaSourcesScreenModel` keys lists by `Source::key()` so the same source can appear twice (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/MangaSourcesScreenModel.kt:71` (`collectLatestSources`)).

Proposal: `fn key(&self) -> String` (method, not a closure field) with the same two formats. `visual_name()` same rule (empty lang → name).

Note: local source has lang `"other"`, so its `visualName` is `"Local manga source (OTHER)"`. Keep (it is what Aniyomi shows).

---

## 4. Enabled source list (`GetEnabledMangaSources`, `GetEnabledAnimeSources`)

### 4.1 Manga (OBSERVED, `app/src/main/java/eu/kanade/domain/source/manga/interactor/GetEnabledMangaSources.kt:18-51`)

Inputs (as one `combine`):
1. `preferences.pinnedMangaSources().changes()`
2. `preferences.enabledLanguages().changes()`
3. `Triple(disabledMangaSources, lastUsedMangaSource, dataSaverExcludedSources)` — three `changes()` flows combined.
4. `repository.getMangaSources()`

Pipeline, in this exact order:
1. `filter { it.lang in enabledLanguages || it.id == LocalMangaSource.ID }`
2. `filterNot { it.id.toString() in disabledSources }`
3. `sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })` — case-insensitive name order (Kotlin `CASE_INSENSITIVE_ORDER` = Unicode simple case folding per char). Stable sort (TimSort), so equal names keep repository order.
4. `flatMap`: for each source:
   - `flag = if ("${it.id}" in pinnedSourceIds) Pins.pinned else Pins.unpinned` (`Pins.pinned` = `Pinned|Actual` = `0b11`; `Pins.unpinned` = `0b00`).
   - `source = it.copy(pin = flag, isExcludedFromDataSaver = "${it.id}" in excludedFromDataSaver)`.
   - emit `source`.
   - if `source.id == lastUsedSource`: also emit `source.copy(isUsedLast = true, pin = source.pin - Pin.Actual)` directly after it.
5. `.distinctUntilChanged()` on the resulting list (structural equality; `key` is not part of the constructor so is ignored).

Pin arithmetic (OBSERVED): `Pins.minus` is XOR (`domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt:23`: `Pins(code xor pin.code)`). So for the last-used copy:
- unpinned source: `0b00 xor 0b10 = 0b10` (Actual bit set, Pinned bit clear);
- pinned source: `0b11 xor 0b10 = 0b01` (Pinned bit set, Actual bit clear).

The UI keys off `Pin.Actual` only after `isUsedLast` is checked first (`MangaSourcesScreenModel.kt:87-88` groups `it.isUsedLast -> LAST_USED_KEY` before `Pin.Actual in it.pin -> PINNED_KEY`), so the unpinned `0b10` copy is never placed in the pinned section. Menus read `Pin.Pinned` only (`app/src/main/java/eu/kanade/presentation/browse/manga/MangaSourcesScreen.kt:138,184`).

Proposal: `pin.without(ACTUAL)` = `bits & !0b10`. Results: unpinned → `0b00`, pinned → `0b01`. Same UI behaviour, no stray bit (T-17a-002).

### 4.2 Anime (OBSERVED, `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt:18-41`)

Same as 4.1 with these differences: five flat inputs (pinned, enabled languages, disabled, lastUsed, sources) — no Triple and no data-saver input; no `isExcludedFromDataSaver`; the same filter/sort/flatMap/last-used duplication; the same XOR.

### 4.3 Test vector (both, from area:09 vectors, re-verified against the Kotlin)

Input (id, lang, name): (1, en, "Zeta"), (2, en, "alpha"), (3, ja, "Beta"), (0, other, "Local manga source"), (4, en, "Hidden"), (5, fr, "Omega").
Settings: languages `{en, ja}`; disabled `{4}`; pinned `{3}`; last used `2`.

1. Language filter keeps 1, 2, 3, 0 (local), 4; drops 5.
2. Disabled filter drops 4 → 1, 2, 3, 0.
3. Name sort (case-insensitive): "alpha"(2), "Beta"(3), "Local manga source"(0), "Zeta"(1).
4. Pins: 3 → `0b11`; others `0b00`.
5. Last used 2: emit `2 alpha` (`0b00`), then `2 alpha` copy with `isUsedLast`, `pin = 0b10` (Kotlin) / `0b00` (Proposal).

Output order: `[2 alpha, 2 alpha(lastused), 3 Beta(pinned), 0 Local, 1 Zeta]`.

Note: the local source appears in the `en` group? No — its lang is `"other"`, so the section header is `other` (manga screen groups by `lang`; the `""` special case in `TreeMap` does not apply).

### 4.4 Edge rules (OBSERVED)

- Pinned but language disabled: the source disappears (filter 1 runs before pinning) and stays in `pinned_catalogues`; re-enabling the language restores the pin.
- Disabled and last used: appears in neither place.
- All languages unticked: only the local source remains.
- Last used id that is not installed: no duplicate emitted (no matching element).

---

## 5. Language grouping (`GetLanguagesWithMangaSources`, `GetLanguagesWithAnimeSources`)

OBSERVED (`app/src/main/java/eu/kanade/domain/source/manga/interactor/GetLanguagesWithMangaSources.kt:16-33`; anime `GetLanguagesWithAnimeSources.kt:16-33`):

Inputs: `enabledLanguages` changes, `disabledMangaSources` changes, `repository.getOnlineMangaSources()` (HTTP sources only: `filterIsInstance<HttpSource>()`, `data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:33-39`; anime `XHttpSource`).

Steps:
1. `sortedWith(compareBy<Source> { it.id.toString() in disabledSource }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name })`: enabled sources first (`false < true`), then case-insensitive name.
2. `groupBy { it.lang }` (LinkedHashMap, insertion order = step 1 order inside each group).
3. `.toSortedMap(compareBy<String> { it !in enabledLanguage }.then(LocaleHelper.comparator))`: enabled languages first, then `LocaleHelper.comparator`.

`LocaleHelper.comparator` (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:17-25`): `"all"` sorts before everything else (`a == "all"` → -1, `b == "all"` → 1); otherwise `getLocalizedDisplayName(a).compareTo(getLocalizedDisplayName(b))`.

`getLocalizedDisplayName(lang)` (OBSERVED, `LocaleHelper.kt:55-67`): `null` → `""`; `""` → `LocaleListCompat.getAdjustedDefault()[0]`; `"zh-CN"` → `Locale.forLanguageTag("zh-Hans")`; `"zh-TW"` → `zh-Hant`; else `Locale.forLanguageTag(lang)`; returns `locale.getDisplayName(locale)` with the first character uppercased, displayed in that same locale.

Edge OBSERVED: two different language codes with identical localized display names compare equal, so `TreeMap` keeps only the first key and drops the second group silently. Proposal: tie-break on the raw code (`then(a.cmp(b))`), so no group is dropped.

Languages with no enabled source still appear (sorted after enabled ones).

Proposal display names: ICU4X display names in the UI locale (see rust design), with the `zh-CN`/`zh-TW` mapping retained.

---

## 6. Toggle interactors (exact semantics)

### 6.1 `ToggleMangaSource` / `ToggleAnimeSource` (OBSERVED, `app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleMangaSource.kt:7-37`)

Entry points:
- `await(source: Source, enable: Boolean = isEnabled(source.id))` → `await(source.id, enable)`.
- `await(sourceId: Long, enable: Boolean = isEnabled(sourceId))`: `disabledMangaSources().getAndSet { disabled -> if (enable) disabled.minus("$sourceId") else disabled.plus("$sourceId") }`.
- `await(sourceIds: List<Long>, enable: Boolean)`: maps ids to strings, `minus` all when enabling, `plus` all when disabling.
- private `isEnabled(sourceId) = sourceId.toString() in disabledMangaSources().get()`.

**Naming trap OBSERVED:** `isEnabled` returns true when the source is currently DISABLED (membership in the disabled set). With the default argument it passes that value as `enable`, so the default flips the state: disabled (`true`) → enable; enabled (`false`) → disable. Proposal: name the helpers `is_disabled` and `flip`, and keep the behaviour.

Anime (`ToggleAnimeSource.kt:7-37`) is identical with `disabledAnimeSources`.

### 6.2 `ToggleMangaSourcePin` / `ToggleAnimeSourcePin` (OBSERVED, `ToggleMangaSourcePin.kt:11-16`)

`isPinned = "${source.id}" in pinnedMangaSources().get()`; then `getAndSet { if (isPinned) minus else plus }`. Pure flip. Note: `isPinned` is read outside the atomic `getAndSet`; a concurrent second toggle can make the result stale (Proposal: do the flip inside one lock/transaction on the set).

### 6.3 `ToggleLanguage` (OBSERVED, `app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt:10-15`)

`isEnabled = language in enabledLanguages().get()`; `getAndSet { if (isEnabled) minus(language) else plus(language) }`. Same non-atomic read as 6.2.

### 6.4 `ToggleExcludeFromMangaDataSaver` (OBSERVED, `ToggleExcludeFromMangaDataSaver.kt:11-19`)

`dataSaverExcludedSources().getAndSet { if ("${source.id}" in it) it - id else it + id }`. Pure flip, no enable flag.

### 6.5 `ToggleMangaIncognito` / `ToggleAnimeIncognito` (OBSERVED, `ToggleMangaIncognito.kt:9-13`)

`await(extensions: String, enable: Boolean)`: `incognitoMangaExtensions().getAndSet { if (enable) it.plus(extensions) else it.minus(extensions) }`. The `extensions` parameter is an extension PACKAGE NAME (the set stores package names, see 7.1), despite the plural name.

---

## 7. Incognito (`GetMangaIncognitoState`, `GetAnimeIncognitoState`)

### 7.1 `await(sourceId: Long?): Boolean` (OBSERVED, `GetMangaIncognitoState.kt:15-20`)

1. If global `incognito_mode` is true → `true` (`basePreferences.incognitoMode()`, `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21`, key `__APP_STATE_incognito_mode`, default `false`).
2. If `sourceId == null` → `false`.
3. `pkg = extensionManager.getExtensionPackage(sourceId) ?: return false` (`MangaExtensionManager.kt:81-86`: searches the installed-extensions map only, so stubs and local source → `null` → `false`).
4. Return `pkg in incognitoMangaExtensions()`.

### 7.2 `subscribe(sourceId: Long?): Flow<Boolean>` (OBSERVED, `:22-31`)

If `sourceId == null`: `incognitoMode().changes()`. Otherwise `combine(incognitoMode.changes(), incognitoMangaExtensions.changes(), extensionManager.getExtensionPackageAsFlow(sourceId)) { incognito || pkg in set }`, then `distinctUntilChanged()`.

Note: `subscribe` with a non-null id does NOT check for a null package (`pkg in set` with `pkg == null` is false), which matches `await`.

Proposal: `fn is_incognito(&self, source: Option<i64>) -> bool` computed from the current settings; the GTK side re-evaluates when the settings `watch` changes.

---

## 8. Data-saver exclusion (manga only)

- Enum `SourcePreferences.DataSaver { NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT }` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:118-123`), stored by enum name (`getEnum`, `core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt:27-45`: serialize `it.name`; an unknown name falls back to the default).
- The exclusion set `data_saver_excluded` holds decimal source-id strings (7.1 form). `isExcludedFromDataSaver` is derived in 4.1 step 4.
- Use of the data saver (server URLs, image quality) lives in the image pipeline (not in scope). This area only stores and exposes the set and enum.

---

## 9. Migration sorting and source list with favourite counts

Full algorithm in `p1-topic-migration-sorting-and-favorites.md`. Summary:
- `SetMigrateSorting.await(mode, direction)`: two writes, `migrationSortingMode().set(mode)` then `migrationSortingDirection().set(direction)` (`app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:9-12`). Not atomic.
- `GetMangaSourcesWithFavoriteCount.subscribe()`: `combine(direction, mode, repository.getMangaSourcesWithFavoriteCount())` → `filterNot { it.first.id == LocalMangaSource.ID }` → `sortedWith(sortFn(direction, mode))` (`app/src/main/java/eu/kanade/domain/source/manga/interactor/GetMangaSourcesWithFavoriteCount.kt:18-28`).
- Comparator `sortFn` (`:30-57`): stub-first rule, then ALPHABETICAL (`name.lowercase().compareToWithCollator`) or TOTAL (`count` ascending); `ASCENDING = Comparator(sortFn)`; `DESCENDING = Collections.reverseOrder(sortFn)`. The reverse also reverses the stub-first rule, so stubs sort LAST in descending order.

---

## 10. Remote update (`UpdateAnimeFromRemote`)

Summary (details in `p1-topic-remote-entry-update.md`):
- Entry points: `awaitEpisodesUpdate(anime, fetchDetails=false, fetchEpisodes=false, manualFetch=false, fetchWindow=(0,0))` and the overload taking an explicit `source`; likewise `awaitSeasonsUpdate` (`mihon/domain/source/interactor/UpdateAnimeFromRemote.kt:36-140`).
- Source lookup: `sourceManager.getOrStub(anime.source)` (so a stub yields a failure when the source is missing).
- Episode list is read sorted by `sourceOrder` ascending; season list sorted by `seasonSourceOrder` ascending.
- Calls `source.getAnimeEpisodeUpdate(anime.toSAnime(), episodes.map(toSEpisode), fetchDetails, fetchEpisodes)` in `withIOContext`.
- Writes anime metadata (`awaitUpdateFromSource`), then `SyncEpisodesWithSource.await(...)` (or `SyncSeasonsWithSource`), then re-reads the anime.
- Returns `Result.success(RemoteAnimeEpisodeUpdate(anime, newEpisodes))` or `Result.failure(e)` after `logcat(LogPriority.ERROR, e)` (`UpdateAnimeFromRemote.kt:83-86`).

---

## 11. Local source and stub interplay with other areas

| Event | Owner | Effect in this area |
|---|---|---|
| Extension installed | area 18-20 | `installedExtensionsFlow` emits → section 1.2 |
| Extension updated, source renamed | area 18-20 | section 1.5 writes the new name and renames downloads |
| Extension uninstalled | area 18-20 | live entry removed; stub row remains; library entries show the stub name; fetches throw `SourceNotInstalled` |
| Library entry with an unknown source id | area 05/06 | `getOrStub` returns a fabricated stub (1.6 case c) |
| Download folder rename | area 23-25 | `renameSource` called only from 1.5 |

---

## 12. Error handling (OBSERVED summary)

| Place | Error | Behaviour |
|---|---|---|
| `getOrStub` (`runBlocking`) | DB exception | Propagates to caller (not caught) |
| `registerStubSource` | DB exception | Thrown inside `scope.launch` on `scope = CoroutineScope(Job() + Dispatchers.IO)` (`AndroidMangaSourceManager.kt:40`). The scope has a plain `Job()` (not `SupervisorJob`) and no `CoroutineExceptionHandler`, so the failure is uncaught, cancels the scope's Job, and every later `launch` on that scope (registrations, both collectors) silently never runs. No retry. |
| `MangaSourceRepositoryImpl.searchManga/getPopularManga/getLatestManga` | id not live | `sourceManager.get(sourceId) as CatalogueSource` → `NullPointerException` / `TypeCastException` (`data/src/main/java/tachiyomi/data/source/manga/MangaSourceRepositoryImpl.kt:76,81,86`). Anime twin uses `getOrStub` and does not throw (`data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt`). |
| Stub network call | not installed | `SourceNotInstalledException` / `AnimeSourceNotInstalledException` (no message) |
| `UpdateAnimeFromRemote.*` | any `Exception` | `logcat(ERROR)` then `Result.failure(e)`; includes `CancellationException` (swallowed, see behavior 12.1) |
| `animeRepository.updateAnime` inside `awaitUpdateFromSource` | DB exception | Returns `false` (`data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:140-148`); the boolean is discarded by `awaitEpisodesUpdate`, so the failure is silent |

12.1 OBSERVED: `catch (e: Exception)` also catches `kotlinx.coroutines.CancellationException`, so a cancelled refresh reports `Result.failure` and continues to the caller. Proposal: `Err(Cancelled)` distinct variant; never retry on cancel.

User-visible message keys in this area: none. The only resource keys in scope are `AYMR.strings.local_manga_source` and `AYMR.strings.local_anime_source` (local source names). Failure toasts are owned by the screens that call these functions.

---

## 13. Concurrency and threading (OBSERVED → Proposal)

| Element | OBSERVED | Proposal |
|---|---|---|
| Registry scope | `CoroutineScope(Job() + Dispatchers.IO)`, one per manager | One tokio runtime task set; published state behind `RwLock<Arc<Snapshot>>` (copy-on-write) |
| Live map | `MutableStateFlow<ConcurrentHashMap>` replaced per emission | `watch` of `Arc<LiveMap>` |
| Stub map | mutated in place (`ConcurrentHashMap`), `getOrPut` with `runBlocking` | `RwLock<HashMap>`; loaded at startup; no blocking DB call from UI |
| `getOrStub` thread | any (called from UI and IO) | Must be callable from the GTK thread: memory only |
| Registration | detached `launch` per source | detached `tokio::spawn`; serialized by a mutex (1.5 race) |
| Remote update | `withIOContext` for the source call; repository calls suspend | `tokio::task::spawn_blocking` not needed; async `reqwest` inside the source trait; DB writes through `spawn_blocking` on the rusqlite pool |
| Enabled list | `combine(...).distinctUntilChanged()` | a single recompute task driven by a settings `watch`, publishing only if the list changed |

---

## 14. Timings, limits, constants (complete list for this area)

- No debounce, timeout or retry constants are defined in the 27 files.
- Stub cache: unbounded (one entry per distinct id seen).
- Icon cache: unbounded `mutableMapOf<String, Drawable>()` keyed by package name (`MangaExtensionManager.kt:63`).
- Sort stability: TimSort (stable) for every `sortedWith` in scope.
- Pin bits: `PINNED = 0b01`, `ACTUAL = 0b10`, `PINNED_BOTH = 0b11`, `UNPINNED = 0b00`.
- Default `last_*` = `-1` (no id matches).
- Warm-up: `AppModule.kt:242-251` runs `get<MangaSourceManager>()` and `get<AnimeSourceManager>()` on the main executor after `App.onCreate` (no delay constant).
