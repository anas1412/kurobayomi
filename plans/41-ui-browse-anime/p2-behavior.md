# Area 41 part 2: behavior spec (anime sources, browse, global search, source filters, migration sources)

All paths are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` = what the Kotlin does (cited path:line). `PROPOSAL:` = what the Rust port should do. Quoted English strings are the values of the named resource keys (`MR.strings.<key>` from `i18n/src/commonMain/moko-resources/base/strings.xml`, `AYMR.strings.<key>` from `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`).

Inventory of files: `p2-inventory.md`. Data model and keys: `p2-data-and-config.md`. Rust design: `p2-rust-design.md`. UI: `p2-ui-parity.md`. Edge cases: `p2-edge-cases-and-tests.md`. Tasks: `p2-tasks.md`. Deep dives: `p2-topic-sources-list.md`, `p2-topic-browse-listing.md`, `p2-topic-source-filters.md`, `p2-topic-global-search.md`.

---

## 1. Navigation map

Browse tab host: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt`.
- Tabs in order (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt:77-84`): `animeSourcesTab()`, `mangaSourcesTab()`, `animeExtensionsTab(animeExtensionsScreenModel)`, `mangaExtensionsTab(mangaExtensionsScreenModel)`, `migrateAnimeSourceTab()`, `migrateMangaSourceTab()`. Anime sources is index 0, Migrate Anime is index 4.
- Re-selecting the Browse tab pushes `GlobalAnimeSearchScreen()` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt:52-54`).

Outgoing edges from scope screens:

| From | Action | To | Source |
|---|---|---|---|
| Sources tab | row click | `BrowseAnimeSourceScreen(source.id, Popular)` (listing.query = `GetRemoteAnime.QUERY_POPULAR`) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:51-53` |
| Sources tab | "Latest" text button | `BrowseAnimeSourceScreen(source.id, Latest)` (part 1 renders the button) | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:126-127` |
| Sources tab | toolbar Global search | `GlobalAnimeSearchScreen()` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:39` |
| Sources tab | toolbar Filter | `AnimeSourcesFilterScreen()` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:44` |
| Sources tab | long press | `AnimeSourceOptionsDialog` (in place) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:58-72` |
| Global search | header click | `BrowseAnimeSourceScreen(source.id, state.searchQuery)` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:75` |
| Global search | card click or long click | `AnimeScreen(anime.id, true)` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:77-78` |
| Global search | single-result auto open | `navigator.replace(AnimeScreen(anime.id, true))` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:56` |
| Browse | cover click or long click | `AnimeScreen(anime.id, true)` (click) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:231` |
| Browse | overflow Settings | `AnimeSourcePreferencesScreen(sourceId)` (part 1) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:147` |
| Browse | overflow / empty-state "Open in WebView" | `WebViewScreen(url = source.getHomeUrl(), initialTitle = source.name, sourceId = source.id)` (area:36) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:114-124` |
| Browse | duplicate dialog "Show anime" | `AnimeScreen(duplicate.id)` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:268` |
| Browse | duplicate dialog "Migrate existing entry" | `MigrateAnimeDialog` (part 1) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:269-289` |
| Browse | migrate dialog seasons | `MigrateSeasonSelectScreen(oldAnime, newAnime)` (part 1) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:284` |
| Migrate tab | row click | `MigrateAnimeScreen(source.id)` (part 1) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:44-46` |
| Migrate tab | help action | `uriHandler.openUri("https://aniyomi.org/help/guides/source-migration/")` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:35-37` |

Incoming edges (other areas call into scope):

| Caller | Call | Effect | Source |
|---|---|---|---|
| `MainActivity` (INTENT_ANIMESEARCH, extension URL handlers) | `navigator.popUntilRoot(); navigator.push(GlobalAnimeSearchScreen(query, filter))` | Global search with extension package filter | `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:532-541` |
| `MainActivity` (deep link ANIME) | `navigator.push(GlobalAnimeSearchScreen(query)); navigator.push(DeepLinkAnimeScreen(query))` | Global search under deep-link screen | `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:514-520` |
| `DeepLinkAnimeScreen` | `NoResults` state → `navigator.replace(GlobalAnimeSearchScreen(query))` | Fall back to global search | `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreen.kt:49` |
| `AnimeScreen` | `performSearch(global=true)` → `GlobalAnimeSearchScreen(query)` | Global search | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:478` |
| `AnimeScreen` | related-anime title search → `GlobalAnimeSearchScreen(relatedAnime.title)` | Global search | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:227` |
| `AnimeScreen` | `performSearch(global=false)`: if previous screen is `HomeScreen` → `AnimeLibraryTab.search(query)`; if previous is `BrowseAnimeSourceScreen` → `navigator.pop(); previousController.search(query)` | Text search in library or source browse | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:480-498` |
| `AnimeScreen` | `performGenreSearch`: previous is `BrowseAnimeSourceScreen` and source is `AnimeHttpSource` → `navigator.pop(); previousController.searchGenre(genre)`; otherwise `performSearch(global=false)` | Genre search in browse | `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreen.kt:503-520` |
| `AnimeLibraryTab` | empty-state global search → `GlobalAnimeSearchScreen(screenModel.state.value.searchQuery ?: "")` | Global search | `app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt:221` |

---

## 2. Feature inventory at a glance

| Feature id | Feature | Screens | Priority |
|---|---|---|---|
| A | Anime sources list (sectioned: Last used, Pinned, per language) | `AnimeSourcesTab`, `AnimeSourcesScreenModel` | P0 |
| B | Source visibility (languages and individual sources on/off) | `AnimeSourcesFilterScreen`, `AnimeSourcesFilterScreenModel` | P1 |
| C | Browse one source (Popular, Latest, search, filters, paging, favorites) | `BrowseAnimeSourceScreen`, `BrowseAnimeSourceScreenModel` | P0 |
| D | Source-defined filter sheet | `SourceFilterAnimeDialog` | P1 |
| E | Global search across sources | `GlobalAnimeSearchScreen`, `GlobalAnimeSearchScreenModel`, `AnimeSearchScreenModel` | P1 |
| F | Migration source picker (tab) | `MigrateAnimeSourceTab` (+ `MigrateAnimeSourceScreenModel`, out of scope but used) | P1 |

---

## 3. Feature A: Anime sources list

### 3.1 Data pipeline (OBSERVED)

`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetEnabledAnimeSources.kt`, function `subscribe()`:

1. `combine(preferences.pinnedAnimeSources().changes(), preferences.enabledLanguages().changes(), preferences.disabledAnimeSources().changes(), preferences.lastUsedAnimeSource().changes(), repository.getAnimeSources())` (lines 19-25).
2. Filter: keep a source when `it.lang in enabledLanguages || it.id == LocalAnimeSource.ID` (line 27). `LocalAnimeSource.ID` is `0L` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:365`) and the local source has `lang = "other"` (line 68).
3. Drop when `it.id.toString() in disabledSources` (line 28). The local source is not protected here; the options dialog hides "Disable" for it instead (part 1).
4. Sort: `sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.name })` (line 29). Sort key is name only; there is no language tiebreak.
5. flatMap (lines 30-38): `flag = if (id in pinnedSourceIds) Pins.pinned else Pins.unpinned`; `source = it.copy(pin = flag)`; emit `source`; if `source.id == lastUsedSource` also emit `source.copy(isUsedLast = true, pin = source.pin - Pin.Actual)`.
6. `distinctUntilChanged()` (line 40).

Pin bit values (`domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt:3-7, 17-28`): `Pin.Unpinned = 0b00`, `Pin.Pinned = 0b01`, `Pin.Actual = 0b10`; `Pins.unpinned = 0`, `Pins.pinned = 0b11`. `Pins.contains(pin)` is `pin.code and code == pin.code`. `Pins.minus(pin)` is XOR (`code xor pin.code`, domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt:23), so `unpinnedSource - Pin.Actual` yields `0b10` (quirk, harmless because `isUsedLast` is checked first in grouping).

### 3.2 Grouping algorithm (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesScreenModel.kt`, `collectLatestAnimeSources` (lines 53-89), executed on every emission of the upstream flow:

Key for each source (line 67-73, `groupByTo`):
- `it.isUsedLast` → `LAST_USED_KEY` (`"last_used"`, `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:85`).
- else `Pin.Actual in it.pin` → `PINNED_KEY` (`"pinned"`, `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:84`).
- else `it.lang` (may be `""`).

Map is `TreeMap<String, MutableList<AnimeSource>>` with this comparator (lines 55-66), applied in order:
1. `d1 == LAST_USED && d2 != LAST_USED` → -1
2. `d2 == LAST_USED && d1 != LAST_USED` → 1
3. `d1 == PINNED && d2 != PINNED` → -1
4. `d2 == PINNED && d1 != PINNED` → 1
5. `d1 == "" && d2 != ""` → 1
6. `d2 == "" && d1 != ""` → -1
7. otherwise `d1.compareTo(d2)` (ordinal String compare on language codes; for example `"all" < "ar" < "en" < "other" < "pt"`)

Output (lines 77-86): for each key in map order emit `Header(key)` then `Item(source)` for each source in input order (groupByTo preserves insertion order, input order comes from 3.1 step 4: name case-insensitive). `isLoading = false`.

Consequences (verified against the code):
- A pinned source appears once in the `pinned` section (primary copy). If it is also the last used source it appears once more under `last_used`. A non-pinned, non-last source appears in its language section only.
- The last-used source appears under `last_used` and also in its normal place (pinned section or language section). It is not de-duplicated.
- Sources with `lang == ""` are grouped last (rule 5/6), after all language groups.

### 3.3 Header text (OBSERVED, `LocaleHelper.getSourceDisplayName`, `app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:30-38`)

| Key | Header text |
|---|---|
| `last_used` | `MR.strings.last_used_source` = "Last used" |
| `pinned` | `MR.strings.pinned_sources` = "Pinned" |
| `other` | `MR.strings.other_source` = "Other" |
| `all` | `MR.strings.multi_lang` = "Multi" |
| any other | `getLocalizedDisplayName(lang)` (see 9.1). For `""` this returns the system-language display name, because `getLocalizedDisplayName("")` uses `LocaleListCompat.getAdjustedDefault()[0]` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:61`). |

Header rendering (part 1, `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:96-110`): `MaterialTheme.typography.header`, padding horizontal `padding.medium` (16dp), vertical `padding.small` (8dp).

### 3.4 Row (OBSERVED, part 1 renders, behavior owned here)

`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:113-141`, `BaseAnimeSourceItem` (`app/src/main/java/eu/kanade/presentation/browse/anime/components/BaseAnimeSourceItem.kt`):
- Icon slot: `AnimeSourceIcon(source)` (part 1).
- Name: `source.name`, one line, ellipsis, `bodyMedium`.
- Language line: `LocaleHelper.getSourceDisplayName(source.lang)`, `bodySmall`, alpha `SECONDARY_ALPHA` (0.78, `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:10`). Shown because `showLanguageInContent` defaults to true.
- Action slot, in order:
  1. If `source.supportsLatest`: `TextButton` label "Latest" (`MR.strings.latest`), primary color → `onClickItem(source, Listing.Latest)`.
  2. Pin `IconButton`: icon `PushPin` (filled) with `colorScheme.primary` when `Pin.Pinned in source.pin`, else `PushPin` outlined with `onBackground` at alpha 0.78. Content description "Unpin" (`action_unpin`) when pinned, else "Pin" (`action_pin`). Click → `togglePin(source)`.
- Row click → `onClickItem(source, Listing.Popular)` (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:123`).
- Row long click → `showSourceDialog(source)` (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:124`).

### 3.5 Options dialog (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:167-202`)

- Title: `source.visualName` (`domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:13-17`): `name` when `lang` is empty, else `"$name (${lang.uppercase()})"`.
- Entry 1: `MR.strings.action_unpin` ("Unpin") if pinned else `action_pin` ("Pin"). Padding vertical 16dp, full width, clickable. Click → `togglePin(source)` then `closeDialog()`.
- Entry 2: `MR.strings.action_disable` ("Disable") only when `source.id != LocalAnimeSource.ID`. Click → `toggleSource(source)` then `closeDialog()`.
- Dialog is an `AlertDialog` with no confirm button and `onDismissRequest = closeDialog`.

### 3.6 Toggle semantics (OBSERVED)

- `togglePin(source)` → `ToggleAnimeSourcePin.await(source)` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSourcePin.kt:11-16`): `isPinned = id.toString() in pinnedAnimeSources`; set = `pinned - id` if pinned else `pinned + id`. Preference key `pinned_anime_catalogues`.
- `toggleSource(source)` → `ToggleAnimeSource.await(source)` with default `enable = isEnabled(source.id)` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt:11-19, 34-36`). `isEnabled(id)` returns `id in disabledAnimeSources` (true when currently DISABLED), so the parameter is inverted in name only. Effect: a disabled source is re-enabled (removed from set), an enabled source is disabled (added to set). Preference key `hidden_anime_catalogues`.
- PROPOSAL: name the parameter `currentlyDisabled` and keep the behavior.

### 3.7 Screen states (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt:49-92`)

| State | Condition | Rendering |
|---|---|---|
| Loading | `state.isLoading` (initial true) | `LoadingScreen` |
| Empty | `state.isEmpty` (items empty) | `EmptyScreen(stringRes = MR.strings.source_empty_screen)` = "No source found" |
| List | otherwise | scrollbar list with `topSmallPaddingValues` |

Empty happens when no installed source has an enabled language and none is local. The local source always passes the filter, so Empty requires the local source to be absent, which does not happen in practice.

### 3.8 Failure path (OBSERVED)

- `AnimeSourcesScreenModel.init` (lines 43-49): on upstream exception → `logcat(ERROR)`, then `_events.send(Event.FailedFetchingSources)`. The `.catch` operator then completes the flow, so no further source updates arrive.
- `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:74-83`: `LaunchedEffect(Unit)` collects `events` and calls `snackbarHostState.showSnackbar(internalErrString)` where `internalErrString = stringResource(MR.strings.internal_error)` = "InternalError: Check crash logs for further information". Default snackbar duration (Compose `SnackbarDuration.Short`).
- If the exception happens before the first emission, `isLoading` stays `true` and the screen shows `LoadingScreen` forever, with the snackbar on top. OBSERVED; see E-SRC-02.

### 3.9 Toolbar actions (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:33-46`)

- "Global search": icon `TravelExplore`, label `MR.strings.action_global_search` = "Global search" → `navigator.push(GlobalAnimeSearchScreen())`.
- "Filter": icon `FilterList`, label `MR.strings.action_filter` = "Filter" → `navigator.push(AnimeSourcesFilterScreen())`.

### 3.10 Quirks to keep or fix (see edge cases)

- Section header order inside language sections is by raw language code (rule 7), while the Visibility page (section 4) orders by display name with "all" first. The two screens disagree. PROPOSAL: use display-name order in both (see decision D-1 in `p2-topic-sources-list.md`).
- Local source and `lang == "other"`: visible in the list regardless of enabled languages (rule in 3.1 step 2) but not in the global search unless "other" is an enabled language (7.3).

---

## 4. Feature B: Source visibility page (Sources filter)

### 4.1 Data (OBSERVED)

`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetLanguagesWithAnimeSources.kt`, `subscribe()`:
- `combine(enabledLanguages.changes(), disabledAnimeSources.changes(), repository.getOnlineAnimeSources())` (lines 17-21).
- `getOnlineAnimeSources()` = installed sources that are `AnimeHttpSource` (`data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt:31-37`). Local source and stub sources are excluded.
- Within the list: `sortedWith(compareBy<AnimeSource> { it.id.toString() in disabledSource }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name })` (lines 22-25). Enabled sources first, then by name.
- `groupBy { it.lang }.toSortedMap(compareBy<String> { it !in enabledLanguage }.then(LocaleHelper.comparator))` (lines 27-31). Enabled languages first; within each group by `LocaleHelper.comparator`.
- `LocaleHelper.comparator` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:17-25`): `"all"` sorts first; otherwise compare `getLocalizedDisplayName(a)` with `compareTo` (display-name, case-sensitive ordinal).

### 4.2 Model (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreenModel.kt`)

- Initial state `Loading` (line 25).
- `init` (lines 27-51): `combine(getLanguagesWithSources.subscribe(), preferences.enabledLanguages().changes(), preferences.disabledAnimeSources().changes()) { a, b, c -> Triple(a, b, c) }`; `.catch { State.Error(throwable) }`; `.collectLatest { State.Success(items, enabledLanguages, disabledSources) }`.
- `toggleLanguage(language)` (lines 57-59) → `ToggleLanguage.await(language)`: `isEnabled = language in enabledLanguages`; set = `enabled - language` if enabled else `enabled + language` (`app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt:10-15`). Key `source_languages`.
- `toggleSource(source)` → `ToggleAnimeSource.await(source)` (same semantics as 3.6).
- `State.Success.isEmpty` = `items.isEmpty()` (line 78-79).

### 4.3 Screen (OBSERVED)

`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt` (part 1 renders) and the wrapper `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt` (part 2):
- Wrapper: `Loading` → `LoadingScreen()` and return (lines 25-28). `Error` → `LaunchedEffect(Unit) { context.toast(MR.strings.internal_error); navigator.pop() }` and return (lines 30-37). `Success` → render (lines 39-46).
- Top bar title: `MR.strings.label_sources` = "Sources", with back navigation (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:32-36`).
- Empty (`state.isEmpty`): `EmptyScreen(stringRes = MR.strings.source_filter_empty_screen)` = "No installed source found" (lines 39-45).
- Content (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt:65-93`): `FastScrollLazyColumn`. For each `(language, sources)` in map order:
  - Header item (key = language): `SwitchPreferenceWidget(title = LocaleHelper.getSourceDisplayName(language), checked = language in enabledLanguages, onCheckedChanged = { onClickLanguage(language) })`.
  - If language enabled: items for each source (key `"source-filter-${source.key()}"`): `BaseAnimeSourceItem(showLanguageInContent = false, onClickItem = onClickSource(source), action = Checkbox(checked = "${source.id}" !in disabledSources, onCheckedChange = null))`.
  - If language disabled: its sources are not rendered.
- The header for `"all"` reads "Multi" (via `getSourceDisplayName`), and its position is first among enabled languages (rule of 4.1).

### 4.4 Defaults (OBSERVED)

- `source_languages` default = `LocaleHelper.getDefaultEnabledLanguages()` = `setOf("all", "en", Locale.getDefault().language)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:72-74`). For a Linux user with system locale `ar` the default is `{all, en, ar}`.
- Language toggling never changes per-source disabled state (see 4.5).

### 4.5 Quirks

- Disabling a language hides its sources but does not change their disabled flags. Re-enabling the language restores each source's own state.
- A source can be disabled while its language is enabled; the list then shows the checkbox unchecked.
- Pinned state is not shown on this page.

---

## 5. Feature C: Browse one source

### 5.1 Gating (OBSERVED)

- `BrowseAnimeSourceScreen.Content()` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:83-86`): if `!ifAnimeSourcesLoaded()` show `LoadingScreen()`.
- `ifAnimeSourcesLoaded()` (`app/src/main/java/eu/kanade/core/util/AnimeSourceUtil.kt:10-13`) is `remember { Injekt.get<AnimeSourceManager>().isInitialized }.collectAsState().value`.
- `AndroidAnimeSourceManager.isInitialized` becomes true after the first emission of the installed-extensions flow (`app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:47-72`).
- Source lookup (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:81`): `sourceManager.getOrStub(sourceId)`. `getOrStub` (`app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:89-93, 117-126`): installed source, else in-memory stub, else `createStubSource(id)` which reads the stub DB table (`sourceRepository.getStubAnimeSource(id)`), else `extensionManager.getSourceData(id)`, else `StubAnimeSource(id, lang = "", name = "")`.
- If `screenModel.source is StubAnimeSource` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:101-107`): `MissingSourceScreen(source, navigateUp)` and return. Its top bar title is `source.name`. Empty screen message = `stringResource(MR.strings.source_not_installed, source.toString())` = "Source not installed: %1$s" with `source.toString()` = `"$name (${lang.uppercase()})"` when name and lang are not blank, else the numeric id (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:101-102`).

### 5.2 Model construction (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:59-103`)

Constructor parameters are listed in `p2-inventory.md`. Construction order:
1. `displayMode` is a delegated property on `sourcePreferences.sourceDisplayMode()` (line 79), persisted on write.
2. `source = sourceManager.getOrStub(sourceId)` (line 81).
3. Initial `State(Listing.valueOf(listingQuery))` (line 77).
4. `init` (lines 83-103): when `listing is Search`, `query = listing.query` and `listing = Search(query, source.getFilterList())`. `filters = source.getFilterList()`. `toolbarQuery = query` (null for Popular and Latest). Then `if (!getIncognitoState.await(source.id)) sourcePreferences.lastUsedAnimeSource().set(source.id)`.
5. `hideInLibraryItems = sourcePreferences.hideInAnimeLibraryItems().get()` (line 108). Snapshot at construction.

`Listing.valueOf(query)` (lines 336-344):
- `"eu.kanade.domain.source.anime.interactor.POPULAR"` → `Popular`.
- `"eu.kanade.domain.source.anime.interactor.LATEST"` → `Latest`.
- otherwise → `Search(query = query, filters = AnimeFilterList())` (includes `null`).

### 5.3 Listing → source call (OBSERVED)

`GetRemoteAnime` (`domain/src/main/java/tachiyomi/domain/source/anime/interactor/GetRemoteAnime.kt:11-22`), `subscribe(sourceId, query, filterList)`:
- `query == QUERY_POPULAR` → `repository.getPopularAnime(sourceId)`.
- `query == QUERY_LATEST` → `repository.getLatestAnime(sourceId)`.
- else → `repository.searchAnime(sourceId, query, filterList)`.

Caller passes `listing.query ?: ""` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:113`). So:

| Listing | Query passed | Filters | Paging source | Source method |
|---|---|---|---|---|
| Popular | `QUERY_POPULAR` | `AnimeFilterList()` (empty) | `AnimeSourcePopularPagingSource` | `getPopularAnime(page)` |
| Latest | `QUERY_LATEST` | empty | `AnimeSourceLatestPagingSource` | `getLatestUpdates(page)` |
| Search(q, f) | `q` or `""` if null | `f` | `AnimeSourceSearchPagingSource(source, query, filters)` | `getSearchAnime(page, query, filters)` |

Quirk: a search whose text equals one of the two sentinel strings is treated as Popular or Latest (E-BR-03).

### 5.4 Paging (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:109-125`:

```
val animePagerFlowFlow = state.map { it.listing }
    .distinctUntilChanged()
    .map { listing ->
        Pager(PagingConfig(pageSize = 25)) {
            getRemoteAnime.subscribe(sourceId, listing.query ?: "", listing.filters)
        }.flow.map { pagingData ->
            pagingData.map {
                networkToLocalAnime.await(it.toDomainAnime(sourceId))
                    .let { localAnime -> getAnime.subscribe(localAnime.url, localAnime.source) }
                    .filterNotNull()
                    .stateIn(ioCoroutineScope)
            }
                .filter { !hideInLibraryItems || !it.value.favorite }
        }
            .cachedIn(ioCoroutineScope)
    }
    .stateIn(ioCoroutineScope, SharingStarted.Lazily, emptyFlow())
```

Paging source (`data/src/main/java/tachiyomi/data/source/anime/AnimeSourcePagingSource.kt:34-65`):
- `load(params)`: `page = params.key ?: 1`. Calls `requestNextPage(page.toInt())` inside `withIOContext`. If `animes` is empty → `throw NoEpisodesException()` (`domain/src/main/java/tachiyomi/domain/items/episode/model/NoEpisodesException.kt:3`) → `LoadResult.Error(e)`.
- Success: `LoadResult.Page(data = animes, prevKey = null, nextKey = if (hasNextPage) page + 1 else null)`.
- `getRefreshKey(state)`: `state.anchorPosition` → `closestPageToPosition` → `prevKey ?: nextKey`.
- An empty page is an error even when it is not the first page, so an empty trailing page with `hasNextPage = true` produces an append error (E-BR-07).

Pager settings: `PagingConfig(pageSize = 25)` only. Other fields keep androidx.paging defaults: `prefetchDistance = pageSize` (25 items), `initialLoadSize = 3 * pageSize` (75, a hint that this paging source ignores), `enablePlaceholders = true`, `maxSize = unbounded`. The source decides its own page size.

Every change of `listing` (object inequality, see 5.14) creates a new Pager, which restarts from page 1 and discards the previous list.

### 5.5 Per-item mapping (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:114-121`)

For each `SAnime` in a loaded page, in order, sequentially within the page:
1. `networkToLocalAnime.await(it.toDomainAnime(sourceId))` (`app/src/main/java/eu/kanade/domain/entries/anime/model/Anime.kt:92-110`). The mapping sets `url`, `title`, `artist`, `author`, `description`, `genre = getGenres()`, `status`, `thumbnailUrl = thumbnail_url`, `backgroundUrl = background_url`, `updateStrategy`, `fetchType`, `seasonNumber`, `initialized`, `memo`, `source = sourceId`. Every other field comes from `Anime.create()` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:315-345`): `id = -1`, `favorite = false`, `dateAdded = 0`, `coverLastModified = 0`, `episodeFlags = 0`, `viewerFlags = 0`, `seasonNumber = -1.0`, `fetchType = Episodes`, `updateStrategy = ALWAYS_UPDATE`, `status = 0`.
2. `getAnime.subscribe(localAnime.url, localAnime.source)` (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/GetAnime.kt:26-28` → `AnimeRepositoryImpl.getAnimeByUrlAndSourceIdAsFlow`). Returns a live DB flow of the row.
3. `filterNotNull().stateIn(ioCoroutineScope)`: an eager StateFlow per item, which lives until the scope is cancelled (the scope is cancelled when the screen model is disposed, `app/src/main/java/eu/kanade/presentation/util/Navigator.kt:55-61`).
4. `.filter { !hideInLibraryItems || !it.value.favorite }` (see 5.6).

`NetworkToLocalAnime.await(anime)` (`domain/src/main/java/tachiyomi/domain/entries/anime/interactor/NetworkToLocalAnime.kt:12-27`):
- `localAnime = animeRepository.getAnimeByUrlAndSourceId(anime.url, anime.source)` (SQL `SELECT * FROM animes WHERE url = :url AND source = :source LIMIT 1`, `data/src/main/sqldelightanime/dataanime/animes.sq:81-85`).
- If null: `id = animeRepository.insertAnime(anime)`; return `anime.copy(id = id!!)`. The insert SQL is at `data/src/main/sqldelightanime/dataanime/animes.sq:166-168` and uses the fields from 5.5 (favorite false, dateAdded 0, and every other column at its `Anime.create()` default listed in section 1.4 of `p2-data-and-config.md`).
- If found and `!favorite`: return `localAnime.copy(title = anime.title)`. The title override is in memory only. Because step 2 re-reads the DB row, the list displays the stored title, not the fresh title. OBSERVED; see E-BR-10.
- If found and favorite: return `localAnime` unchanged.

### 5.6 Hide-in-library filter (OBSERVED)

- Preference `browse_hide_in_anime_library_items` (default `false`, `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:77-80`).
- When true: items with `favorite == true` are removed after mapping. Removal happens per item in the paging stream; the page is not refetched and the paging count shrinks.
- Read once at model construction (line 108). Changing the preference does not affect an open browse screen.
- Because the filter reads `it.value.favorite` at filter time, a favorite toggled later on the same screen stays visible until the screen is recreated (E-BR-11).

### 5.7 Chip row (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:151-212`)

Horizontal scroll row, padding horizontal `padding.small` (8dp), spacing 8dp, below the toolbar, above a `HorizontalDivider`.

| Order | Chip | Shown when | Selected when | Leading icon (`FilterChipDefaults.IconSize`) | Click |
|---|---|---|---|---|---|
| 1 | "Popular" (`MR.strings.popular`) | always | `state.listing == Listing.Popular` | `Icons.Outlined.Favorite` | `screenModel.resetFilters()` then `screenModel.setListing(Listing.Popular)` |
| 2 | "Latest" (`MR.strings.latest`) | `source.supportsLatest` | `state.listing == Listing.Latest` | `Icons.Outlined.NewReleases` | `resetFilters()` then `setListing(Listing.Latest)` |
| 3 | "Filter" (`MR.strings.action_filter`) | `state.filters.isNotEmpty()` | `state.listing is Listing.Search` | `Icons.Outlined.FilterList` | `screenModel.openFilterSheet()` |

- `setListing(listing)` sets `listing` and `toolbarQuery = null` (line 151-153).
- `resetFilters()` sets `filters = source.getFilterList()` (fresh defaults, line 147-149). Note: `resetFilters` does not change `listing`. Selecting Popular or Latest changes `listing`, so the Pager restarts.
- The Filter chip is shown whenever the source defines any filter, even when Popular is selected. Its selected state only reflects the listing type, not whether filters are active.

### 5.8 Toolbar search and back navigation (OBSERVED)

- Toolbar: `BrowseAnimeSourceToolbar` (part 1) with `searchQuery = state.toolbarQuery`, `onSearchQueryChange = setToolbarQuery`, `onSearch = search` (String). `onClickCloseSearch = navigateUp` (part 1).
- `search(query: String)` is called on submit only (IME action). There is no debounce and no search on each keystroke.
- Clearing the field calls `setToolbarQuery(null)` and does not re-query.
- `navigateUp` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:92-99`): `if (!state.isUserQuery && state.toolbarQuery != null) setToolbarQuery(null) else navigator.pop()`.
- `isUserQuery` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:365`) = `listing is Search && !listing.query.isNullOrEmpty()`.

`search(query: String? = null, filters: AnimeFilterList? = null)` (lines 163-176):
1. `input = state.listing as? Listing.Search ?: Listing.Search(query = null, filters = source.getFilterList())`.
2. `listing = input.copy(query = query ?: input.query, filters = filters ?: input.filters)`.
3. `toolbarQuery = query ?: input.query`.

Each call produces a new `Search` object, so the Pager restarts even when the query is the same (E-BR-06). Blank text is sent to the source; the browse screen does not guard against it (the global search does, 7.5).

### 5.9 Display mode and grid columns (OBSERVED)

Display mode:
- Preference `pref_display_mode_catalogue`, type `LibraryDisplayMode`, default `CompactGrid` (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:22`). Stored strings: `COMPACT_GRID`, `COMFORTABLE_GRID`, `COVER_ONLY_GRID`, `LIST`. Unknown values deserialize to the default (`domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt:24-31`).
- Stored once for all sources (global), not per source.
- Toolbar display-mode button (`app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt:61-69`): icon `ViewList` when the current mode is `List`, otherwise `ViewModule`; content description `MR.strings.action_display_mode` = "Display mode". Opens a dropdown with three radio items: `action_display_comfortable_grid` = "Comfortable grid" (`ComfortableGrid`), `action_display_grid` = "Compact grid" (`CompactGrid`), `action_display_list` = "List" (`List`). Selecting one closes the menu and writes the preference.
- Rendering (`app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:121-150`): `ComfortableGrid` → comfortable grid; `List` → list; `CompactGrid` and `CoverOnlyGrid` → compact grid. `CoverOnlyGrid` is not selectable from the menu, but a stored value renders as compact grid.

Columns:
- `getColumnsPreference(orientation)` (lines 127-135): landscape uses `pref_animelib_columns_landscape_key` (default `0`), otherwise `pref_animelib_columns_portrait_key` (default `0`) (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:102-105`). Value `0` → `GridCells.Adaptive(128.dp)`; value > 0 → `GridCells.Fixed(value)`.
- `getColumnsPreferenceForCurrentOrientation(orientation)` (lines 138-145) returns the raw value; it is passed as `entries` to the list mode (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:223`).
- `orientation` = `LocalConfiguration.current.orientation`; landscape iff `ORIENTATION_LANDSCAPE`.

Grid geometry (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt:31-35`, `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceCompactGrid.kt:31-35`):
- Content padding: `contentPadding + PaddingValues(8.dp)`.
- Vertical and horizontal spacing: `CommonEntryItemDefaults.GridVerticalSpacer` and `GridHorizontalSpacer` = 4dp each (`app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:49-53`).
- Cover alpha: `CommonEntryItemDefaults.BrowseFavoriteCoverAlpha` = 0.34 for favorites, 1.0 otherwise (same file, line 52-53).
- In-library badge `InLibraryBadge(enabled = anime.favorite)` at the cover start.

List mode (`app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceList.kt:31-65`): `BoxWithConstraints` measures the container height; each row gets `containerHeight = containerHeightPx - topBarHeight` (`topBarHeight` is measured from the top bar `onSizeChanged`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:130-136`). Row content is `EntryListItem` (part 1 / area:37 owns the item).

Loading rows: `BrowseSourceLoadingItem()` at the top when `loadState.prepend is Loading`, and at the bottom when `refresh` or `append` is Loading (list and grids use `GridItemSpan(maxLineSpan)` for grid loaders).

### 5.10 Body state machine (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:57-151`)

`errorState = refresh.takeIf { is Error } ?: append.takeIf { is Error }`.

| Order | Condition | Rendering |
|---|---|---|
| 1 | `itemCount <= 0 && errorState is Error` | `EmptyScreen(message = formatted(error), actions)`. Actions for `LocalAnimeSource`: one action "Local source guide" (`local_source_help_guide`) → `HELP_URL`. Otherwise three actions in order: "Retry" (`action_retry`, `Icons.Outlined.Refresh`) → `animeList.refresh`; "Open in WebView" (`action_open_in_web_view`, `Icons.Outlined.Public`) → WebView; "Help" (`label_help`, `HelpOutline`) → `Constants.URL_HELP`. Return. |
| 2 | `itemCount == 0 && refresh is Loading` | `LoadingScreen` with `padding(contentPadding)`. Return. |
| 3 | otherwise | Grid or list for the current display mode, with loading rows (5.9). |

Error snackbar (lines 64-76): `LaunchedEffect(errorState)`: when `itemCount > 0 && errorState is Error`, `snackbarHostState.showSnackbar(message = formatted(error), actionLabel = "Retry", duration = Indefinite)`. Result `ActionPerformed` → `animeList.retry()`. Result `Dismissed` → `snackbarHostState.currentSnackbarData?.dismiss()`.

`formatted(error)` = `formattedMessage` (9.2). For `NoEpisodesException` this is "No results found" (`MR.strings.no_results_found`).

Help URL in content (line 229): `Constants.URL_HELP`. Local source help URL (line 114, used by toolbar Help and local empty-state action): `LocalAnimeSource.HELP_URL`.

### 5.11 Long press and favorite flow (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:232-249`)

Plain click: `navigator.push(AnimeScreen(it.id, true))`.

Long press (inside `scope.launchIO`):
1. `duplicateAnime = screenModel.getDuplicateAnimelibAnime(anime)`.
2. `when`:
   - `anime.favorite` → `setDialog(Dialog.RemoveAnime(anime))`.
   - `duplicateAnime != null` → `setDialog(Dialog.AddDuplicateAnime(anime, duplicateAnime))`.
   - else → `screenModel.addFavorite(anime)`.
3. `haptic.performHapticFeedback(LongPress)` (dropped on Linux, 12).

`getDuplicateAnimelibAnime(anime)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:293-295`) = `getDuplicateAnimelibAnime.await(anime).getOrNull(0)`, which runs `animeRepository.getDuplicateLibraryAnime(anime.id, anime.title.lowercase())` = SQL `SELECT * FROM animes WHERE favorite = 1 AND LOWER(title) = :title AND _id != :id` (`data/src/main/sqldelightanime/dataanime/animes.sq:128-133`, `data/src/main/java/tachiyomi/data/entries/anime/AnimeRepositoryImpl.kt:72-76`). No ORDER BY, so when several rows match the first returned row is not defined by the query (E-BR-13).

`changeAnimeFavorite(anime)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:225-245`), launched on `screenModelScope`:
1. `new = anime.copy(favorite = !anime.favorite, dateAdded = when (anime.favorite) { true -> 0; false -> Instant.now().toEpochMilli() })`.
2. If `!new.favorite` (removing): `new = new.removeCovers(coverCache)`; `new = new.removeBackgrounds(backgroundCache)`.
   - `Anime.removeCovers` (`app/src/main/java/eu/kanade/tachiyomi/util/AnimeExtensions.kt:18-25`): no-op for local source; `coverCache.deleteFromCache(this, true) > 0` → `copy(coverLastModified = now)`.
   - `Anime.removeBackgrounds` (lines 27-34): same pattern with `backgroundLastModified`.
3. Else (adding): `setAnimeDefaultEpisodeFlags.await(anime)` (area:24/08) then `addTracks.bindEnhancedTrackers(anime, source)` (`app/src/main/java/eu/kanade/domain/track/anime/interactor/AddAnimeTracks.kt:81`).
4. `updateAnime.await(new.toAnimeUpdate())` (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/UpdateAnime.kt:15-17`). `toAnimeUpdate()` (`domain/src/main/java/tachiyomi/domain/entries/anime/model/AnimeUpdate.kt:39-60`) copies every field of the in-memory row (url, title, artist, author, description, genre, status, thumbnailUrl, backgroundUrl, lastUpdate, nextUpdate, fetchInterval, viewerFlags, episodeFlags, coverLastModified, backgroundLastModified, updateStrategy, initialized, fetchType, seasonFlags, seasonNumber, seasonSourceOrder, memo, version, lastModifiedAt, favoriteModifiedAt, parentId), not only `favorite` and `dateAdded`. The write therefore overwrites newer values with the snapshot taken when the list item was loaded (E-BR-02).

`addFavorite(anime)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:247-279`):
1. `categories = getCategories()`: `getCategories.subscribe().firstOrNull()?.filterNot { it.isSystemCategory }.orEmpty()` (lines 286-291). System category is the one with `id == UNCATEGORIZED_ID` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:13`), that is `0`.
2. `defaultCategoryId = libraryPreferences.defaultAnimeCategory().get()` (default `-1`, `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:153`, key `default_anime_category`, `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:414`).
3. `defaultCategory = categories.find { it.id == defaultCategoryId.toLong() }`.
4. `when`:
   - (a) `defaultCategory != null` → `moveAnimeToCategories(anime, defaultCategory)` then `changeAnimeFavorite(anime)`.
   - (b) `defaultCategoryId == 0 || categories.isEmpty()` → `moveAnimeToCategories(anime)` (no categories) then `changeAnimeFavorite(anime)`.
   - (c) otherwise → `preselectedIds = getCategories.await(anime.id).map { it.id }`; `setDialog(Dialog.ChangeAnimeCategory(anime, categories.mapAsCheckboxState { it.id in preselectedIds }.toImmutableList()))`.

Note: with the default pref (`-1`) and at least one user category, case (c) always shows the picker. With no user categories, case (b) adds directly.

`moveAnimeToCategories(anime, vararg categories: Category)` (private, lines 297-299) keeps only categories with `id != 0L`, then calls the public overload. The public overload (lines 301-308) runs `setAnimeCategories.await(animeId, categoryIds)` on `launchIO` (`domain/src/main/java/tachiyomi/domain/category/anime/interactor/SetAnimeCategories.kt:11-17`, errors are logged and swallowed). It is not awaited by `addFavorite`, so it can finish after `changeAnimeFavorite`.

### 5.12 Dialogs (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:253-311`)

`setDialog(null)` is the dismiss action. `state.dialog` is one of the following. Dialog values are kept in `State.dialog`; only one dialog is open at a time.

| Dialog | Content | Buttons and actions |
|---|---|---|
| `Filter` | `SourceFilterAnimeDialog` (section 6) | Reset → `resetFilters()`; Filter → `search(filters = state.filters)` then dismiss |
| `RemoveAnime(anime)` | `RemoveEntryDialog` (`app/src/main/java/eu/kanade/presentation/browse/BrowseSourceDialogs.kt:11-40`). Title `MR.strings.are_you_sure` = "Are you sure?". Text `MR.strings.remove_manga` = "You are about to remove \"%s\" from your library" with `anime.title`. | Cancel (`action_cancel`) → dismiss. Remove (`action_remove` = "Remove") → dismiss, then `changeAnimeFavorite(dialog.anime)`. |
| `AddDuplicateAnime(anime, duplicate)` | `DuplicateAnimeDialog` (`app/src/main/java/eu/kanade/presentation/entries/anime/DuplicateAnimeDialog.kt:35-120`), an `AdaptiveSheet`. Title "Are you sure?". Text `MR.strings.confirm_add_duplicate_manga` = "You have an entry in your library with the same name.\n\nDo you still wish to continue?". | Row 1 "Show anime" (`AYMR.strings.action_show_anime`, icon `Book`) → dismiss, `navigator.push(AnimeScreen(duplicate.id))`. Row 2 "Migrate existing entry" (`MR.strings.action_migrate_duplicate`, icon `SwapVert`) → dismiss, `setDialog(Migrate(anime, duplicate))`. Row 3 "Add anyway" (`MR.strings.action_add_anyway`, icon `Add`) → dismiss, `addFavorite(dialog.anime)` (so categories still apply). Bottom outlined "Cancel" (`action_cancel`) → dismiss. |
| `Migrate(newAnime, oldAnime)` | `MigrateAnimeDialog(oldAnime, newAnime, screenModel = MigrateAnimeDialogScreenModel(), onDismissRequest, onClickTitle, onClickSeasons, onPopScreen)` (part 1). `onClickTitle` → `AnimeScreen(oldAnime.id)`; `onClickSeasons` → `MigrateSeasonSelectScreen(oldAnime, newAnime)`; `onPopScreen` → dismiss. | Owned by part 1 (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt`). |
| `ChangeAnimeCategory(anime, initialSelection)` | `ChangeCategoryDialog(initialSelection, onEditCategories = push(CategoriesTab), onConfirm = { include, _ -> changeAnimeFavorite(dialog.anime); moveAnimeToCategories(dialog.anime, include) })` (`app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt:197`). The confirm button dismisses first, then calls `onConfirm(include, exclude)`. | Checkbox per user category (system category excluded), preselected from `getCategories.await(anime.id)`. Edit categories → `CategoriesTab`. |

Note on the Remove dialog: its confirm calls `changeAnimeFavorite(dialog.anime)`. `dialog.anime` is the snapshot from the long press, so `anime.favorite` is `true` and the removal path (5.11 step 2) runs.

### 5.13 Add-to-library side effects summary (OBSERVED)

| Action | DB writes | Filesystem | Other |
|---|---|---|---|
| Add (no categories) | `animes` update: favorite=1, date_added=now, all other fields rewritten from snapshot | none | default episode flags set; enhanced trackers bound |
| Add (category) | `animes_categories` set for anime, then `animes` update | none | as above |
| Remove | `animes` update: favorite=0, date_added=0, cover/background last-modified=now if cache deleted | cover and background cache files deleted for the anime (area:34) | none |

### 5.14 Listing equality (OBSERVED, important)

`Listing` is a `sealed class` with `data class Search(query, filters)`. The `filters` field is `AnimeFilterList`, whose `equals` always returns `false` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt:10-12`), including identity. Consequences:
- `Search` objects never compare equal, so `state.map { it.listing }.distinctUntilChanged()` (line 110) emits for every state change while a Search listing is active. Opening a dialog, toggling the toolbar query, or changing `displayMode` each rebuilds the Pager and reloads the list from page 1.
- `Popular` and `Latest` are `data object`s and compare by identity, so they do not restart on unrelated state changes.

PROPOSAL: compare listings by value (query plus a stable hash of filter states) and never by `AnimeFilterList` identity. Keep the reload when the listing really changes.

### 5.15 Genre search (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:178-218`)

Triggered from `AnimeScreen.performGenreSearch` (section 1) through `BrowseAnimeSourceScreen.searchGenre(name)`, which sends `SearchType.Genre(name)` into the companion `Channel<SearchType>()` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:325, 328`). The channel has capacity 0, so the sender suspends until the composed screen's `LaunchedEffect(Unit)` receives it (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:313-321`). `SearchType.Text` → `search(txt)`, `SearchType.Genre` → `searchGenre(txt)`.

Algorithm (`searchGenre(genreName)`):
1. `defaultFilters = source.getFilterList()`. `genreExists = false`.
2. For each top-level filter `sourceFilter` in order:
   - If `sourceFilter is AnimeFilter.Group`: for each child `filter` in `sourceFilter.state` in order: if `filter is AnimeFilter` and `filter.name.equals(genreName, ignoreCase = true)`: if `TriState` → `filter.state = 1` (STATE_INCLUDE); if `CheckBox` → `filter.state = true`; any other type → no state change; `genreExists = true`; stop all loops.
   - Else if `sourceFilter is AnimeFilter.Select`: `index = sourceFilter.values.filterIsInstance<String>().indexOfFirst { it.equals(genreName, true) }`; if `index != -1` → `sourceFilter.state = index`; `genreExists = true`; stop.
   - Other top-level types (CheckBox, TriState, Text, Header, Separator, Sort) are never matched.
3. Update state: `listing = if (genreExists) Search(query = null, filters = defaultFilters) else Search(query = genreName, filters = defaultFilters)`; `filters = defaultFilters`; `toolbarQuery = listing.query`.

Consequences: a genre that is matched by a Text, Select or Group child name (not TriState or CheckBox) still sets `genreExists = true` and produces `Search(query = null)`, which sends an empty query to the source (E-BR-04).

### 5.16 Overflow menu (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt:70-92`)

- If `source is LocalAnimeSource`: overflow item "Help" (`label_help`) → `onHelpClick` = `LocalAnimeSource.HELP_URL`.
- Else: overflow item "Open in WebView" (`action_open_in_web_view`) → `onWebViewClick` (no-op unless `source is AnimeHttpSource`, which then opens `WebViewScreen`).
- If `source is ConfigurableAnimeSource`: overflow item "Settings" (`action_settings`) → `AnimeSourcePreferencesScreen(sourceId)`.
- Title in the top bar: `source?.name` (`AppBarTitle`).

### 5.17 Assist URL (OBSERVED)

`BrowseAnimeSourceScreen` implements `AssistContentScreen` (lines 75-79) and sets `assistUrl = (source as? AnimeHttpSource)?.getHomeUrl()` in a `LaunchedEffect(screenModel.source)` (lines 126-128). `getHomeUrl()` returns `baseUrl` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:54`). The URL is used only by the Android assist framework (dropped, 12).

### 5.18 Incognito (OBSERVED)

`GetAnimeIncognitoState.await(sourceId)` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeIncognitoState.kt:15-20`): true when `basePreferences.incognitoMode()` (`__APP_STATE_incognito_mode`) is true, or when the source's extension package is in `incognito_anime_extensions`. In this screen incognito only gates the last-used write (5.2 step 4). It does not change any other behavior.

### 5.19 Last used (OBSERVED)

`lastUsedAnimeSource` (`__APP_STATE_last_anime_catalogue_source`, default `-1`) is written when the browse model is created, unless incognito. The Sources list (section 3) reads it as the "Last used" section key. The write also happens for stub sources.

---

## 6. Feature D: Source filter sheet

The sheet is implemented in `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/SourceFilterAnimeDialog.kt`.

### 6.1 Sheet layout (OBSERVED, lines 41-77)
- `AdaptiveSheet(onDismissRequest)` (`app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt`, part 1 territory) wrapping a `LazyColumn`.
- Sticky header row, background `colorScheme.background`, padding 8dp: left `TextButton` "Reset" (`action_reset`) with primary text color, Spacer(weight 1), right `Button` "Filter" (`action_filter`). Then `HorizontalDivider`.
- Items: one `FilterItem` per top-level filter.
- Reset → `onReset()` (`BrowseAnimeSourceScreenModel.resetFilters`), the sheet stays open.
- Filter → `onFilter()` (`search(filters = state.filters)`) then `onDismissRequest()`.

### 6.2 In-place mutation (OBSERVED)

`val updateFilters = { onUpdate(filters) }` (line 41). Every control mutates the `AnimeFilter` object's `state` field directly, then calls `updateFilters()`. `onUpdate` is `setFilters(filters)`, which stores the same list object. The mutated list is therefore the list used by the next Filter press or the next search. Cancelling the sheet keeps the mutations in memory but does not query the source. The listing changes only on Filter or Reset.

### 6.3 Per-type rendering and rules (OBSERVED, lines 79-164)

| Type | Rendering | Click or change | State rule |
|---|---|---|---|
| `AnimeFilter.Header` | `HeadingItem(filter.name)` | none | none |
| `AnimeFilter.Separator` | `HorizontalDivider()` | none | none |
| `AnimeFilter.CheckBox` | `CheckboxItem(label = name, checked = state)` | click | `state = !state` |
| `AnimeFilter.TriState` | `TriStateItem(label, state = state.toTriStateFilter())` | click | `state = toTriStateFilter().next().toTriStateInt()` |
| `AnimeFilter.Text` | `TextItem(label, value = state)` | each text change | `state = newText` |
| `AnimeFilter.Select<*>` | `SelectItem(label, options = values, selectedIndex = state)` | choose option | `state = index` |
| `AnimeFilter.Sort` | `CollapsibleBox(heading = name)` containing one `SortItem` per value | click a value | see 6.4 |
| `AnimeFilter.Group<*>` | `CollapsibleBox(heading = name)` containing children rendered recursively, only children that are `AnimeFilter` | per child | per child |

TriState mapping (lines 166-181):
- `STATE_IGNORE (0)` ↔ `TriState.DISABLED`; `STATE_INCLUDE (1)` ↔ `ENABLED_IS`; `STATE_EXCLUDE (2)` ↔ `ENABLED_NOT`.
- Cycle: `DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED` (`core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:9-15`).
- Any other stored int throws `IllegalStateException("Unknown TriState state: $this")`. This crashes the app.

Constants for TriState are in `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:20-24`: `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2`.

### 6.4 Sort rules (OBSERVED, lines 126-151)
- `SortItem.sortDescending` = `filter.state?.ascending?.not()?.takeIf { index == filter.state?.index }`. The arrow shows only on the selected index and reflects `!ascending`.
- Click on `index`:
  - if `index == filter.state?.index` → `ascending = !filter.state!!.ascending` (toggle).
  - else → `ascending = filter.state!!.ascending` (keep the current direction).
  - `filter.state = Selection(index, ascending)`.
- Crash: when `filter.state` is `null`, the `else` branch dereferences `filter.state!!` and throws `NullPointerException`. A source that ships a Sort with no default selection crashes on the first click. PROPOSAL: treat `null` as "no selection" and select index with `ascending = true`.
- `AnimeFilter.Sort.Selection(index: Int, ascending: Boolean)` is a data class (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:31`).

### 6.5 Nested groups (OBSERVED)
`FilterItem` for `Group` maps `filter.state.filterIsInstance<AnimeFilter<*>>()` and calls `FilterItem` on each child with the same `onUpdate`. Non-filter elements in the list are ignored.

### 6.6 Filter model and identity (OBSERVED)
- `AnimeFilter<T>` has `equals` on `name` and `state` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:34-45`). `AnimeFilterList` has `equals = false` always (see 5.14).
- `AnimeFilter.Sort.Selection` equality is data-class equality.
- A new `AnimeFilterList` comes from `source.getFilterList()` each time; defaults come from the source, not from the user.

---

## 7. Feature E: Global search

### 7.1 Construction and snapshots (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:36-72`)
- `coroutineDispatcher = Executors.newFixedThreadPool(5).asCoroutineDispatcher()` (line 46). Created per model instance, never shut down.
- `enabledLanguages`, `disabledSources`, `pinnedSources` are read once from `SourcePreferences` at construction (lines 49-51). Changes made while the screen is open are not seen.
- `init` (lines 66-72): collects `preferences.globalSearchFilterState().changes()` and writes `onlyShowHasResults`.
- `extensionFilter` is a protected var, set by the subclass (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreenModel.kt:15`).

### 7.2 Initial search (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreenModel.kt:14-23`)
```
init {
    extensionFilter = initialExtensionFilter
    if (initialQuery.isNotBlank() || !initialExtensionFilter.isNullOrBlank()) {
        if (extensionFilter != null) setSourceFilter(AnimeSourceFilter.All)
        search()
    }
}
```
- When `initialExtensionFilter` is non-null (even blank), the source filter is forced to `All`.
- `search()` runs whenever the query or the extension filter is non-blank.
- The default `sourceFilter` in `State` is `PinnedOnly` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:202`).

### 7.3 Source selection (OBSERVED)
`getEnabledSources()` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:85-94`), base implementation:
- `sourceManager.getAll()` (installed sources, including the local source; stubs excluded).
- Filter: `it.lang in enabledLanguages && "${it.id}" !in disabledSources`.
- Sort: `compareBy({ "${it.id}" !in pinnedSources }, { "${it.name.lowercase()} (${it.lang})" })` (pinned first, then `name (lang)` in lowercase, plain String compare).

`GlobalAnimeSearchScreenModel.getEnabledSources()` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreenModel.kt:25-28`) additionally filters: `state.value.sourceFilter != PinnedOnly || "${it.id}" in pinnedSources`.

Quirk: the local source has `lang = "other"`. It is searched only when `"other"` is in `enabledLanguages`, which is not in the default set (E-GS-05).

`getSelectedSources()` (lines 96-108):
- If `extensionFilter` is null or empty → `getEnabledSources()`.
- Else → `extensionManager.installedExtensionsFlow.value.filter { it.pkgName == filter }.flatMap { it.sources }.filter { it in enabledSources }`.

### 7.4 Search algorithm (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:123-179`)
1. `query = state.searchQuery`, `sourceFilter = state.sourceFilter`.
2. `if (query.isNullOrBlank()) return`. Results are kept and no request is sent.
3. `sameQuery = lastQuery == query` (exact string compare, no trim, case-sensitive).
4. `if (sameQuery && lastSourceFilter == sourceFilter) return`.
5. Store `lastQuery = query` and `lastSourceFilter = sourceFilter`.
6. `sources = getSelectedSources()`.
7. Items:
   - if `sameQuery`: `sources.associateWith { existingResults[it] ?: Loading }` (keep results already fetched for the same query).
   - else: `sources.associateWith { Loading }`.
   - `updateItems(items)` (sorted, see 7.6).
8. `searchJob = ioCoroutineScope.launch { sources.map { source -> async { <steps below> } }.awaitAll() }`. For each source:
   - `if (state.value.items[source] !is Loading) return@async` (skip sources with a result already).
   - `page = withContext(coroutineDispatcher) { source.getSearchAnime(1, query, source.getFilterList()) }`. Only page 1 is requested. The filter list is the source default, not the browse screen's filters (the global search has no filter UI).
   - `titles = page.animes.map { networkToLocalAnime.await(it.toDomainAnime(source.id)) }`.
   - `if (isActive) updateItem(source, Success(titles))`.
   - On `Exception e`: `if (isActive) updateItem(source, Error(e))`.

Concurrency: at most 5 `getSearchAnime` calls run at once (5 threads). The `async` blocks start concurrently, so the rest queue on the pool.

### 7.5 Trigger (OBSERVED)
- Search runs on toolbar submit (`onSearch = { screenModel.search() }`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:70`). Text changes alone do not search.
- A source-filter chip change calls `setSourceFilter(filter)` which sets the filter and calls `search()` immediately (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:114-117`).
- The "Has results" chip does not search. It only changes visibility (7.8).

### 7.6 Result ordering (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:58-64, 181-189`)
`sortComparator(map)` = `compareBy<AnimeSource>({ (map[it] as? Success)?.isEmpty ?: true }, { "${it.id}" !in pinnedSources }, { "${it.name.lowercase()} (${it.lang})" })`.
- Key 1: `Success` with at least one result sorts first (`false`). Loading, Error, and Success with zero results sort last (`true`).
- Key 2: pinned first.
- Key 3: `name (lang)` lowercase, plain String compare.
- Applied on every `updateItems`, so results move as they arrive. Items are stored in a `PersistentMap` sorted by this comparator.

### 7.7 Progress (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:206-207`, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt:59-66`)
- `progress = items.count { it.value !is Loading }` (Success and Error both count).
- `total = items.size`.
- Bar shown when `progress in 1..<total`, fraction `progress / total`, full width, bottom of the search field.

### 7.8 Visibility (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:208, 231-233`)
- `filteredItems = items.filter { isVisible(onlyShowHasResults) }`.
- `isVisible(false)` = true for every result.
- `isVisible(true)` = `this is Success && !isEmpty`. Loading, Error and empty Success are hidden.

### 7.9 Known defects (OBSERVED)
- `searchJob` is assigned but never cancelled. A new search does not stop previous coroutines.
- Previous coroutines call `updateItem(source, result)`, which writes into the current state without checking the query. A late result from query A can overwrite the Loading entry of query B for the same source. See E-GS-03.
- The thread pool is created per model and not shut down. Each opened global search keeps 5 threads alive until process exit. PROPOSAL: use a shared semaphore of 5 permits.

### 7.10 "Has results" toggle (OBSERVED)
- `toggleFilterResults()` → `preferences.globalSearchFilterState().toggle()` (line 119-121). Key `__APP_STATE_has_filters_toggle_state`, default `false` (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:49-52`). Shared across sessions.

### 7.11 Chips (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt:69-124`)
Horizontal scroll row, padding horizontal 8dp, spacing 8dp, then a `HorizontalDivider`:
1. "Pinned" (`MR.strings.pinned_sources`, icon `PushPin`), selected when `sourceFilter == PinnedOnly`, click → `setSourceFilter(PinnedOnly)`.
2. "All" (`MR.strings.all`, icon `DoneAll`), selected when `sourceFilter == All`, click → `setSourceFilter(All)`.
3. `VerticalDivider()`.
4. "Has results" (`MR.strings.has_results`, icon `FilterList`), selected when `onlyShowHasResults`, click → `toggleFilterResults()`.

The source comment at line 75 says the chip UX is not final and that the chip only applies when a new search is triggered. Current behavior: changing the chip does not change visible results until a search runs.

### 7.12 Result rendering (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt:62-104`)
- `LazyColumn(contentPadding = paddingValues)`.
- For each `(source, result)` in `filteredItems` order: `item(key = source.id)` containing `GlobalSearchResultItem(title, subtitle, onClick, modifier = animateItem())`.
  - `title`: `source.name`. The `▶` marker only applies when `fromSourceId` is set, and this screen never sets it.
  - `subtitle`: `LocaleHelper.getLocalizedDisplayName(source.lang)` (not `getSourceDisplayName`). For `all` this is "All", not "Multi".
  - `onClick` → `onClickSource(source)` → `BrowseAnimeSourceScreen(source.id, state.searchQuery)`.
  - Body: `Loading` → `GlobalSearchLoadingResultItem()`; `Success` → `GlobalAnimeSearchCardRow(titles)`; `Error` → `GlobalSearchErrorResultItem(message = throwable.message)`.
- Card row (`app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt:27-89`): empty list → text "No results found" (`MR.strings.no_results_found`), padding horizontal 16dp and vertical 8dp. Otherwise `LazyRow(contentPadding = 8dp, spacing = 4dp)`. Each card: `Box(width 96dp)` containing `EntryComfortableGridItem(title, titleMaxLines = 3, coverData = asAnimeCover(), coverBadgeStart = InLibraryBadge(isFavorite), coverAlpha = 0.34 if favorite else 1.0, onClick, onLongClick)`.
- Card click and long click both call `onClickItem(anime)` → `AnimeScreen(id, true)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:77-78`). Long press does not toggle the library here.
- Each card's title and favorite come from `getAnime(initialAnime)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:75-83`), a `produceState` over the DB flow, so the badge updates live.
- Error item: `GlobalSearchErrorResultItem` shows an error icon, then the text `throwable.message`. A null message renders as empty text (E-GS-06). It is not run through `formattedMessage`.

### 7.13 Single-result auto open (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/GlobalAnimeSearchScreen.kt:41-64`)
- `showSingleLoadingScreen` is initialized once, at the first composition, to `searchQuery.isNotEmpty() && !extensionFilter.isNullOrEmpty() && state.total == 1`.
- While it is true: `LoadingScreen()`, and a `LaunchedEffect(state.items)` evaluates `state.items.values.singleOrNull()`:
  - `Loading` → keep waiting (`return@LaunchedEffect`).
  - `Success` with `result.singleOrNull() != null` → `navigator.replace(AnimeScreen(anime.id, true))`.
  - `Success` with zero or several results → `showSingleLoadingScreen = false` (show the list).
  - `Error` or several sources → `showSingleLoadingScreen = false`.
- `singleOrNull()` applies to the map values, so this path only applies when exactly one source is selected.
- `AnimeLibraryTab` and `MainActivity` pass a non-null extension filter from the URL-handler intent (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:539`). This is the only path that auto opens.

### 7.14 Toolbar text and navigation (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt:49-58`)
- `SearchToolbar(searchQuery, onChangeSearchQuery, onSearch, onClickCloseSearch = navigateUp, navigateUp, scrollBehavior)`.
- Global search has no per-source header title and no "Filter" button.

---

## 8. Feature F: Migration source picker tab

### 8.1 Tab (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:22-52`)
- `rememberScreenModel { MigrateAnimeSourceScreenModel() }` and `state.collectAsState()`.
- Title `AYMR.strings.label_migration_anime` = "Migrate Anime".
- One action: title `MR.strings.migration_help_guide` = "Source migration guide", icon `HelpOutline`, click → `uriHandler.openUri("https://aniyomi.org/help/guides/source-migration/")`.
- Content: `MigrateAnimeSourceScreen(state, contentPadding, onClickItem = { navigator.push(MigrateAnimeScreen(source.id)) }, onToggleSortingDirection = toggleSortingDirection, onToggleSortingMode = toggleSortingMode)`.
- The tab does not collect `screenModel.channel`. Errors on the `Event` channel are not shown (E-MS-02).

### 8.2 Data (OBSERVED)
`GetAnimeSourcesWithFavoriteCount.subscribe()` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt:18-28`):
- `combine(migrationSortingDirection.changes(), migrationSortingMode.changes(), repository.getAnimeSourcesWithFavoriteCount())`.
- `filterNot { it.first.id == LocalAnimeSource.ID }`, then `sortedWith(sortFn(direction, mode))`.

Repository (`data/src/main/java/tachiyomi/data/source/anime/AnimeSourceRepositoryImpl.kt:39-53`):
- `combine(handler.subscribeToList { animesQueries.getAnimeSourceIdWithFavoriteCount() }, sourceManager.sources) { rows, _ -> rows }`. The `sourceManager.sources` flow only re-triggers the query.
- For each `(sourceId, count)`: `source = sourceManager.getOrStub(sourceId)`; `DomainSource = mapSourceToDomainSource(source).copy(isStub = source is StubAnimeSource)`; emit `(DomainSource, count)`.
- Query `SELECT source, count(*) FROM animes WHERE favorite = 1 GROUP BY source` (`data/src/main/sqldelightanime/dataanime/animes.sq:114-120`). Each row is one installed or stub source that has at least one favorite.

### 8.3 Comparator (OBSERVED, `app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt:30-57`)
- `ALPHABETICAL`: if `a.isStub && !b.isStub` → -1; if `b.isStub && !a.isStub` → 1; else `a.name.lowercase().compareToWithCollator(b.name.lowercase())`.
- `TOTAL`: same stub rule; else `a.count.compareTo(b.count)` (ascending count).
- `ASCENDING` uses the comparator as is. `DESCENDING` uses `Collections.reverseOrder(sortFn)`, which reverses the whole ordering, so stubs move to the end.
- `compareToWithCollator` uses a locale collator (`tachiyomi.core.common.util.lang`, outside this scope).
- The sort is stable. Ties keep the GROUP BY order, which SQL does not define (E-MS-04).

### 8.4 Model (OBSERVED; the model file is listed in `p1-inventory.md` as part 1, and is described here because the tab consumes it, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt:26-96`)
- `State(isLoading = true, items = [], sortingMode = ALPHABETICAL, sortingDirection = ASCENDING)`; `isEmpty = items.isEmpty()`.
- `init`: subscribe to the sources flow (on `launchIO`), set `isLoading = false` on each emission. On exception: `logcat(ERROR)` and `_channel.send(Event.FailedFetchingSourcesWithCount)`; the `catch` operator then ends the flow.
- `preferences.migrationSortingDirection().changes()` and `migrationSortingMode().changes()` update the state.
- `toggleSortingMode()`: `ALPHABETICAL ↔ TOTAL`, then `setMigrateSorting.await(newMode, currentDirection)` (`app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:9-12`). This writes both preferences.
- `toggleSortingDirection()`: `ASCENDING ↔ DESCENDING`, then `setMigrateSorting.await(currentMode, newDirection)`.

### 8.5 Screen (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt:45-144`)
- `isLoading` → `LoadingScreen(padding)`. `isEmpty` → `EmptyScreen(stringRes = MR.strings.information_empty_library)` = "Your library is empty".
- List: `ScrollbarLazyColumn(contentPadding + topSmallPaddingValues)`.
  - Sticky header (key `STICKY_HEADER_KEY_PREFIX`), background `colorScheme.background`, padding start 16dp: text `MR.strings.migration_selection_prompt` = "Select a source to migrate from" (`header` typography, weight 1); `IconButton` for sort mode; `IconButton` for direction.
  - Sort-mode icon: `ALPHABETICAL` → `SortByAlpha`, description `action_sort_alpha` = "Alphabetically"; `TOTAL` → `Numbers`, description `action_sort_count` = "Total entries". Click → `onToggleSortingMode`.
  - Direction icon: `ASCENDING` → `ArrowUpward`, description `action_asc` = "Ascending"; `DESCENDING` → `ArrowDownward`, description `action_desc` = "Descending". Click → `onToggleSortingDirection`.
  - Rows: `items(key = "migrate-${source.id}")`, `BaseAnimeSourceItem(showLanguageInContent = source.lang != "", icon = AnimeSourceIcon, action = BadgeGroup { Badge(text = "$count") })`. Content: `source.name.ifBlank { source.id.toString() }` (one line, ellipsis), then a row with language display (if shown) and, when `source.isStub`, the text `MR.strings.not_installed` = "Not installed" in `colorScheme.error`.
  - Row click → `onClickItem(source)`. Row long click → `copyToClipboard(sourceId, sourceId)` where `sourceId = source.id.toString()` (`app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt:65-68`).

### 8.6 Preferences (OBSERVED, `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:30-38`)
- `pref_migration_sorting`: enum `SetMigrateSorting.Mode`, default `ALPHABETICAL`.
- `pref_migration_direction`: enum `SetMigrateSorting.Direction`, default `ASCENDING`.
- Enum values: `Mode { ALPHABETICAL, TOTAL }`, `Direction { ASCENDING, DESCENDING }` (`app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:14-22`). Stored by name by `getEnum`.

---

## 9. Shared helpers used by this scope

### 9.1 LocaleHelper (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt`)
- `comparator` (lines 17-25): `"all"` first; else `getLocalizedDisplayName(a).compareTo(getLocalizedDisplayName(b))`.
- `getSourceDisplayName(lang, context)` (lines 30-38): `last_used` → `MR.strings.last_used_source`; `pinned` → `MR.strings.pinned_sources`; `other` → `MR.strings.other_source`; `all` → `MR.strings.multi_lang`; else `getLocalizedDisplayName(lang)`.
- `getLocalizedDisplayName(lang)` (lines 55-67): null → `""`; `""` → `LocaleListCompat.getAdjustedDefault()[0]`; `zh-CN` → `Locale.forLanguageTag("zh-Hans")`; `zh-TW` → `"zh-Hant"`; else `Locale.forLanguageTag(lang)`. Result `locale.getDisplayName(locale).replaceFirstChar { it.uppercase(locale) }`.
- `getDefaultEnabledLanguages()` (lines 72-74): `setOf("all", "en", Locale.getDefault().language)`.
- `PINNED_KEY = "pinned"` (line 84), `LAST_USED_KEY = "last_used"` (line 85); both `internal const`.

### 9.2 Browse error text (`app/src/main/java/eu/kanade/presentation/util/ExceptionFormatter.kt:16-49`)
`formattedMessage` (a context receiver extension):
- `DisabledTorrServerException` → `AYMR.strings.torrserver_disabled` = "TorrServer is not running".
- `HttpException` → `MR.strings.exception_http` = "HTTP %d, check website in WebView" with `code`.
- `UnknownHostException` → if offline: `MR.strings.exception_offline` = "No Internet connection"; else `MR.strings.exception_unknown_host` = "Couldn't reach %s" with `message ?: ""`.
- `NoChaptersException`, `NoEpisodesException` → `MR.strings.no_results_found` = "No results found".
- `SourceNotInstalledException`, `AnimeSourceNotInstalledException` → `MR.strings.loader_not_implemented_error` = "Source not found".
- Otherwise: class simple name `"Exception"` or `"IOException"` → `message ?: className`; any other class → `"$className: $message"`.

### 9.3 ioCoroutineScope (`app/src/main/java/eu/kanade/presentation/util/Navigator.kt:55-61`)
Per screen model: `CoroutineScope(Dispatchers.IO + SupervisorJob()) + CoroutineName(key)`, cancelled in `onDispose`.

### 9.4 URLs in scope
| Constant | Value | Source |
|---|---|---|
| `Constants.URL_HELP` | `https://aniyomi.org/docs/guides/troubleshooting/` | `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4` |
| `LocalAnimeSource.HELP_URL` | `https://aniyomi.org/help/guides/local-anime/` | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:366` |
| Migration guide | `https://aniyomi.org/help/guides/source-migration/` | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceTab.kt:36` |

---

## 10. Concurrency model (OBSERVED)

| Component | Thread or scope | Notes |
|---|---|---|
| Sources list model | `launchIO` (Dispatchers.IO) for the subscription | State updated with `MutableStateFlow.update` |
| Sources visibility model | `combine` in `screenModelScope` (main dispatcher) | Preference writes via `getAndSet` |
| Browse model | `ioCoroutineScope` for paging, `cachedIn`, per-item `stateIn` | Per-item DB subscriptions live until the scope is cancelled |
| Browse favorite add | `screenModelScope.launch` (main) for `changeAnimeFavorite`; `launchIO` for category writes | Category write is not awaited |
| Browse long press | `scope.launchIO` in the composable scope | Duplicate lookup off the main thread |
| Global search fan-out | `ioCoroutineScope.launch` with `async` per source; source call inside `withContext(pool of 5)` | Results via `updateItem` on the state flow |
| Migrate sources | `launchIO` subscription | Errors go to an unobserved channel |

PROPOSAL (Rust): see `p2-rust-design.md` section 4.

---

## 11. Constants and limits (OBSERVED unless marked)

| Name | Value | Where |
|---|---|---|
| Paging page size (prefetch distance) | 25 | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:112` |
| Paging initial load size (androidx default, unused by paging source) | 75 (= 3 × 25) | androidx.paging default |
| Paging placeholders | enabled (androidx default) | androidx.paging default |
| Source requests in global search | 5 concurrent | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:46` |
| Global search page requested | 1 only | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:160` |
| Adaptive grid minimum cell width | 128 dp | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreenModel.kt:134` |
| Grid content padding | 8 dp | `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt:33` |
| Grid spacing | 4 dp horizontal and vertical | `app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:50-51` |
| Favorite cover alpha | 0.34 | `app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:52` |
| Secondary alpha | 0.78 | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:10` |
| Card width in global search | 96 dp | `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt:64` |
| Card title max lines | 3 | `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt:67` |
| Paging snackbar duration | Indefinite | `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt:69` (presentation) |
| Sources failure snackbar duration | Short (default) | `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesTab.kt:79` |
| Padding tokens | extraSmall 4, small 8, mediumSmall 12, medium 16, large 24, extraLarge 32 dp | `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:12-24` |
| TriState cycle | DISABLED → ENABLED_IS → ENABLED_NOT → DISABLED | `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:9-15` |
| Pin bits | Pinned 0b01, Actual 0b10, Pins.pinned 0b11 | `domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt:3-28` |
| Category system id | 0 | `domain/src/main/java/tachiyomi/domain/category/model/Category.kt:13` (`UNCATEGORIZED_ID`) |
| Default category pref | -1 (ask) | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:153` |
| Local source id | 0 | `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:365` |
| Stub source name/lang on miss | `""` / `""` | `app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt:125` |

---

## 12. Drops and replacements (what the user loses)

| Item | Decision | Reason | What the user loses |
|---|---|---|---|
| Haptic feedback on browse long press (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:247`) | Drop | Linux has no standard haptic API | Tactile confirmation on long press. Visual state (dialog or added badge) remains. |
| Assist content URL (`AssistContentScreen`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/browse/BrowseAnimeSourceScreen.kt:75-79, 126-128`) | Drop | Android assist framework only | Android assist can read the source home URL. Nothing on Linux. Use "Open in WebView". |
| Compose `animateItem` / fast-scroll animations | Drop | Presentation animation only | Row movement animation when global search results reorder. Results still reorder. |
| Voyager navigator back stack | Replace with GTK page stack (`ui::push` / `ui::pop` in the mockup) | Platform navigation | None functional; back gesture semantics via Escape or the back button. |
| `Executors.newFixedThreadPool(5)` per model (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:46`) | Replace with a 5-permit semaphore | Leak and no shutdown | None; behavior is the same concurrency limit. |
| Injekt service locator | Replace with explicit `AppServices` struct | Platform DI | None. |
| `Uri handler` (`openUri`) | Replace with `gtk::UriLauncher` | GTK 4.10+ API, no libadwaita | Opens the default browser through the desktop portal. |
| Android `Toast` (`context.toast(MR.strings.internal_error)`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/AnimeSourcesFilterScreen.kt:33`) | Replace with the in-window snackbar (T-41a-045, part 1 banner primitive) | Toast is Android-only | Toast duration and placement differ; text is the same. |
| Clipboard `copyToClipboard` (part 1 location) | Replace with `gdk::Clipboard::set_text` | Platform API | None. |

---

## 13. Edge-case index

The edge cases referenced above are listed with IDs and test steps in `p2-edge-cases-and-tests.md`: E-SRC-*, E-VIS-*, E-BR-*, E-FD-*, E-GS-*, E-MS-*, and the race IDs RC-*.
