# 47-settings-screens-general — part 2 behaviour spec

All source paths are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` = what the Kotlin does. `Proposal:` = what the Rust port should do. Other areas are referenced as `area:<id>`.

---

## 1. Settings router (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/SettingsScreen.kt`)

### 1.1 OBSERVED

- Constructor: `SettingsScreen(private val destination: Int? = null)`. Secondary constructor `SettingsScreen(destination: Destination)` delegates with `destination.id` (lines 26-30).
- `sealed class Destination(val id: Int)` (lines 81-85):
  - `About` id `0`
  - `DataAndStorage` id `1`
  - `Tracking` id `2`
- `Content()` (lines 33-79) picks the start screen with `isTabletUi()`:
  - **Phone** (`!isTabletUi()`, lines 35-55): `Navigator(screen = when (destination) { About.id -> AboutScreen; DataAndStorage.id -> SettingsDataScreen; Tracking.id -> SettingsTrackingScreen; else -> SettingsMainScreen })`. Content wraps the navigator in `CompositionLocalProvider(LocalBackPress provides pop)` where `pop` = `if (nav.canPop) nav.pop() else parentNavigator.pop()`. Transition: `DefaultNavigatorScreenTransition(navigator = it)`.
  - **Tablet** (lines 56-77): same mapping except `else -> SettingsAppearanceScreen`. Insets: `WindowInsets.systemBars.only(WindowInsetsSides.Horizontal)`, applied with `windowInsetsPadding(insets).consumeWindowInsets(insets)`. Layout `TwoPanelBox`: `startContent` = `SettingsMainScreen.Content(twoPane = true)` with `LocalBackPress provides parentNavigator::pop`; `endContent` = `DefaultNavigatorScreenTransition(navigator = it)`.
- `isTabletUi()` (`app/src/main/java/eu/kanade/presentation/util/WindowSize.kt:10`) = `Configuration.isTabletUi()` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt:20-22`) = `smallestScreenWidthDp >= 720`. Constant `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP = 720` (line 12).

### 1.2 Proposal

- `SettingsShell::open(dest: Option<SettingsDestination>)` with `enum SettingsDestination { About = 0, DataAndStorage = 1, Tracking = 2 }`. Keep the numeric ids because other areas pass them (see area:36-ui-shell-navigation-security-webview for the deep-link entry point).
- Start page: `About`, `DataAndStorage`, `Tracking`, otherwise Main (phone/narrow) or Appearance (wide).
- Width test: the window's allocated width in logical px. Two-pane when `width >= 720`. Re-evaluate on `notify::default-width` and on the widget `resize` signal. Hysteresis is not needed (Android does not use it either).
- Back: if the inner stack has more than one page, pop it; otherwise close the settings shell (`parentNavigator.pop()` equivalent). Escape and Alt+Left map to back; mouse back button maps to back.

---

## 2. App language (`app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt`)

### 2.1 OBSERVED

- `Content()` (lines 41-96):
  - `langs = remember { getLangs(context) }`.
  - `currentLanguage` = `AppCompatDelegate.getApplicationLocales().get(0)?.toLanguageTag() ?: ""` (line 47).
  - `LaunchedEffect(currentLanguage)` (lines 50-58): if `currentLanguage.isEmpty()` then `LocaleListCompat.getEmptyLocaleList()` else `LocaleListCompat.forLanguageTags(currentLanguage)`; then `AppCompatDelegate.setApplicationLocales(locale)`. Note: the effect runs on first composition with the already-current value, so it is effectively idempotent.
  - Scaffold with `AppBar(title = MR.strings.pref_app_language, navigateUp = navigator::pop)`.
  - `LazyColumn` of `ListItem`s (lines 71-93): click sets `currentLanguage = it.langTag` (applies on the next recomposition through the effect). `headlineContent` = `it.displayName`; `supportingContent` = `it.localizedDisplayName` if non-null; `trailingContent` = check icon (`Icons.Default.Check`, tint `colorScheme.primary`) iff `currentLanguage == it.langTag`.
- `getLangs(context)` (lines 98-121):
  1. Open `R.xml.locales_config` (generated, see `p2-data-and-config.md` section 4).
  2. For every `START_TAG` named `locale`, read attribute `name` (the BCP-47 tag). For each tag: `displayName = LocaleHelper.getLocalizedDisplayName(tag)`; if non-empty add `Language(tag, displayName, LocaleHelper.getDisplayName(tag))`.
  3. `langs.sortBy { it.displayName }` (line 117). Sort is by the **localized** display name in the **current** locale.
  4. `langs.add(0, Language("", stringResource(MR.strings.label_default), null))` (line 118). "Default" is always first.
- `LocaleHelper.getLocalizedDisplayName(lang)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:55-67`): `""` -> system default locale; `zh-CN` -> `zh-Hans`; `zh-TW` -> `zh-Hant`; then `Locale.getDisplayName(locale)` with first char upper-cased in the locale.
- `LocaleHelper.getDisplayName(lang)` (lines 40-48): same zh mapping, then `Locale.forLanguageTag(...).displayName` (English display name in the default locale, not capitalised).

### 2.2 Proposal (Rust)

- Language list = the set of translation catalogs shipped with the binary (for Kurobayomi, initially just `en`). Sort by native name (ICU display name in the language itself, capitalised). Default row first, value `""`.
- Row: headline = native name; supporting = English name (`getDisplayName`); trailing check on the active row.
- Persist choice in settings key `app_language` (values `""` or BCP-47 tag). Apply at startup by setting `LANGUAGE` for gettext (or the Fluent bundle selection) before GTK starts. Changing language at runtime requires a restart: show a toast "Restart Kurobayomi to apply the language" (new key, see section 15). This is a deviation: Android applies at runtime.
- Lazy alternative (recorded, not built): drop the in-app picker and rely on the system locale only. Add the picker when a second catalog exists.

---

## 3. Anime extension stores

### 3.1 Screen host (`AnimeExtensionStoresScreen.kt`)

OBSERVED (lines 33-84):
- `screenModel = rememberScreenModel { AnimeExtensionStoresScreenModel() }`.
- `LaunchedEffect(url) { url?.let { screenModel.addFromDeeplink(url) } }` (lines 161-163). Runs before the Loading check.
- If `state is Loading`, render `LoadingScreen()` and return (lines 165-169).
- Else `successState = state as Success` and render `AnimeExtensionStoresScreen(...)` with callbacks:
  - `onClickCreate` -> `showDialog(Create())`
  - `onCopy` -> `context.copyToClipboard(it.indexUrl, it.indexUrl)`
  - `onOpenWebsite` -> `it.contact.website.let(context::openInBrowser)`
  - `onOpenDiscord` -> `it.contact.discord?.let(context::openInBrowser)`
  - `onClickDelete` -> `showDialog(Delete(it))`
  - `onClickRefresh` -> `refreshRepos()`
  - `navigateUp` -> `navigator::pop`
- Dialog routing (lines 183-212) by `successState.dialog`: `Create` -> `AnimeExtensionStoreCreateDialog`; `Delete` -> `AnimeExtensionStoreDeleteDialog`; `Confirm` -> `AnimeExtensionStoreConfirmDialog`; null -> nothing.

### 3.2 Model (`AnimeExtensionStoresScreenModel.kt`)

Constructor dependencies (defaults via Injekt): `GetAnimeExtensionStores`, `AddAnimeExtensionStore`, `RemoveAnimeExtensionStore`, `UpdateAnimeExtensionStores`, `AnimeExtensionManager`.

Initial state `Loading`.

`init` (lines 37-51): `getExtensionStores.subscribe().collectLatest { stores -> ... }` on `launchIO`:
- If state is `Loading` -> `Success(stores = stores)` with `dialog = null`.
- If state is `Success` -> `it.copy(stores = stores)` (dialog is preserved).

`updateSuccessState(func)` (lines 26-35): no-op while `Loading`.

#### 3.2.1 `createRepo(baseUrl)` (lines 58-92), `launchIO`
1. `updateSuccessState`: if dialog is `Create` or `Confirm`, set `processing = true`.
2. `addExtensionStore(baseUrl)` (= `AddAnimeExtensionStore` -> `AnimeExtensionStoreRepositoryImpl.insert` -> `service.fetch(indexUrl).mapCatching { upsert(it) }`; see `data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt:19-21`).
3. `onSuccess`: `extensionManager.findAvailableExtensions()`, then `dismissDialog()`.
4. `onFailure(throwable)`: if dialog is `Create` or `Confirm`, set `processing = false`, `errorMessage = throwable.message ?: "unknown error"`. The message is the raw exception text, not a localised string (OBSERVED, a known wart).

#### 3.2.2 `refreshRepos()` (lines 97-105)
Only when state is `Success`: `launchIO { updateExtensionStores() }` (= `UpdateAnimeExtensionStores` -> `refreshAll()`, which re-fetches every stored index and upserts; failures are logged and skipped per store, see area:19-extensions-anime). No spinner, no toast. Success is visible only through the list changing.

#### 3.2.3 `deleteRepo(baseUrl)` (lines 110-115)
`launchIO { removeExtensionStore(baseUrl); extensionManager.findAvailableExtensions() }`. No error handling. Note the dialog is dismissed by the UI immediately after `onDelete()` (see section 3.4).

#### 3.2.4 `addFromDeeplink(storeIndexUrl)` (lines 117-126)
`updateSuccessState` (no-op while Loading) sets `dialog = Confirm(url = storeIndexUrl, alreadyExists = stores.any { it.indexUrl == storeIndexUrl })`. The `alreadyExists` test is an exact string match on `indexUrl`.

#### 3.2.5 `showDialog(d)` / `dismissDialog()` (lines 128-138)
Replace / null the dialog, both through `updateSuccessState`.

### 3.3 Domain facts the screen depends on (cross-area)
- `AnimeExtensionStoreService.fetch(indexUrl)` (`data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt`): GET the URL, decompress if gzip (`0x1f8b`), then branch on the first byte: `0x5B` `[` = legacy array (requires URL to end in `/index.min.json`, rewrites to `/repo.json`), `0x7B` `{` = JSON (legacy `repo.json` first, else index), anything else = protobuf. Errors propagate as `Result.failure(e)`. Full algorithm in area:19-extensions-anime.
- Store row keys: `index_url` is the upsert key. Fields: `indexUrl, name, badgeLabel, signingKey, contact{website, discord}, isLegacy, extensionListUrl`. See `p2-data-and-config.md` section 2.

### 3.4 Dialogs (`AnimeExtensionStoresDialogs.kt`)

**Create** (`AnimeExtensionStoreCreateDialog`, lines 28-101)
- Title `extensionStoresScreen.addStore.title` ("Add extension store").
- Body: one `OutlinedTextField` (single line, keyboard type Uri, label `extensionStoresScreen.addStoreInput.inputLabel` = "Store index URL"). Text state is `rememberTextFieldState()`.
- `storeAlreadyExists` = `storeIndexUrls.contains(text)` (derived state, recomputed on text change).
- Supporting text: `errorMessage ?: (alreadyExists ? extensionStoresScreen.addStore.alreadyExists : "*required")`. Note: `errorMessage` is shown even after the user edits the text (it is not cleared on edit).
- `isError = errorMessage != null || storeAlreadyExists`.
- Confirm button (`TextButton`): label `Processing…` (`extensionStoresScreen.addStore.processing`) while `processing`, else `Add`. Enabled iff `!processing && text.isNotEmpty() && !storeAlreadyExists`. Click calls `onCreate(text)`. The dialog does not dismiss itself; the model dismisses on success.
- Dismiss button: `Cancel`.
- Focus: `LaunchedEffect(focusRequester) { delay(0.1 s); requestFocus() }` (workaround issuetracker 204502668).

**Delete** (`AnimeExtensionStoreDeleteDialog`, lines 104-132)
- Title `extensionStoresScreen.deleteStore.title` ("Remove extension store").
- Body `extensionStoresScreen.deleteStore.body` = `Do you wish to remove the extension store "%1$s" (%2$s)?` with `storeName`, `storeIndexUrl`.
- Confirm `OK` -> `onDelete()` then `onDismissRequest()`. Dismiss `Cancel`.

**Confirm / deep link** (`AnimeExtensionStoreConfirmDialog`, lines 135-192)
- Title "Add extension store". Body text `extensionStoresScreen.addStoreDeeplink.bodyText` = "Do you wish to add the extension store below?", then a read-only text field containing `storeIndexUrl` (`rememberTextFieldState(initialText = url)`).
- Supporting text: if `storeAlreadyExists` -> "This extension store already exists"; else if `errorMessage != null` -> error text; else none.
- `isError = errorMessage != null || storeAlreadyExists`.
- Confirm: label Processing/Add as above; enabled `!storeAlreadyExists && !processing`; click `onCreate()` (no dismiss; model dismisses on success).
- Cancel: `Cancel`.

### 3.5 Page and list (`components/anime/AnimeExtensionStoresScreen.kt`, `AnimeExtensionStoresContent.kt`)

- Scaffold; AppBar: `navigateUp`, title `MR.strings.extensionStores` ("Extension stores"), action: `IconButton(Refresh)` with content description `action_webview_refresh` ("Refresh").
- FAB: `CategoryFloatingActionButton(lazyListState, onCreate = onClickCreate)` (shared Extended FAB, label `action_add` = "Add", icon `Add`; expands when `shouldExpandFAB()` = at top, or last scroll direction is backward, or cannot scroll forward — `presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt:10-16`).
- Empty (`state.isEmpty`): `EmptyScreen(extensionStoresScreen.emptyLabel)` = "You haven't added an extension store yet", padded by scaffold padding, FAB still shown. Return.
- Non-empty: `LazyColumn(contentPadding = paddingValues + topSmallPaddingValues + PaddingValues(horizontal = padding.medium), verticalArrangement = spacedBy(padding.small))`. One `ElevatedCard` per store with `Modifier.animateItem()`.
- Card header row: `Icons.AutoMirrored.Outlined.Label` + `store.name` in `titleMedium`, `padding.medium` start/top/end.
- Card action row (end-aligned, `IconButton`s in this order):
  1. `Public` (globe) — content description `action_open_in_browser` ("Open in browser") -> `onOpenWebsite(store)`.
  2. `CustomIcons.Discord` — **only if** `store.contact.discord != null` — no content description (`null`) -> `onOpenDiscord(store)`.
  3. `ContentCopy` — content description `action_copy_to_clipboard` -> `onCopy(store)` (copies `indexUrl`).
  4. `Delete` — content description `action_delete` -> `onClickDelete(store)`.

### 3.6 Edge cases (anime)
- Deep link arrives while `Loading`: `addFromDeeplink` is a silent no-op (see section 13, race).
- Two dialogs: a deep-link `Confirm` replaces whatever dialog was open. No queueing.
- A successful add of a store that already exists cannot happen through the UI (button disabled) but can happen through a deep link race; `upsert` on the same `index_url` then overwrites the row.

---

## 4. Manga extension repos

### 4.1 Screen host (`MangaExtensionReposScreen.kt`, lines 27-95)
- `LaunchedEffect(url) { url?.let { showDialog(RepoDialog.Confirm(it)) } }` (lines 35-37). Runs before the Loading check. While state is `Loading`, `showDialog` is a no-op (section 13).
- Loading -> `LoadingScreen()` and return (lines 39-42).
- Success: `MangaExtensionReposScreen(state, onClickCreate = showDialog(Create), onOpenWebsite = openInBrowser(it.website), onClickDelete = showDialog(Delete(it)), onClickRefresh = refreshRepos(), navigateUp)`.
- Dialog routing (lines 55-86): `Create` -> `ExtensionRepoCreateDialog(repoUrls = repos.map{baseUrl}.toImmutableSet())`; `Delete` -> `ExtensionRepoDeleteDialog(repo = dialog.repo)`; `Conflict` -> `ExtensionRepoConflictDialog(oldRepo, newRepo, onMigrate = replaceRepo(newRepo))`; `Confirm` -> `ExtensionRepoConfirmDialog(repo = dialog.url, onCreate = createRepo(url))`.
- Events (lines 88-94): `LaunchedEffect(Unit) { screenModel.events.collectLatest { if (it is RepoEvent.LocalizedMessage) context.toast(it.stringRes) } }`. Only Success composes this effect, so events emitted earlier stay buffered in the channel until then.

### 4.2 Model (`MangaExtensionReposScreenModel.kt`)

Dependencies: `GetMangaExtensionRepo`, `CreateMangaExtensionRepo`, `DeleteMangaExtensionRepo`, `ReplaceMangaExtensionRepo`, `UpdateMangaExtensionRepo`.

Events: `Channel<RepoEvent>(Int.MAX_VALUE)` exposed as `receiveAsFlow()`. Unlimited buffer, so `send` never suspends.

`init` (lines 35-46): `getExtensionRepo.subscribeAll().collectLatest { repos -> state.update { RepoScreenState.Success(repos = repos.toImmutableSet()) } }`.
- **OBSERVED**: this always builds a fresh `Success`, so `dialog` and `oldRepos` are reset to null on every emission. An open dialog is closed by any DB change to the repo list. Contrast the anime model, which keeps the dialog (`it.copy(stores = stores)`).

#### 4.2.1 `createRepo(baseUrl)` (lines 53-64)
`launchIO { when (result = createExtensionRepo.await(baseUrl)) }`:
- `InvalidUrl` -> send `RepoEvent.InvalidUrl` (toast "Invalid repo URL").
- `RepoAlreadyExists` -> send `RepoEvent.RepoAlreadyExists` (toast "This repo already exists!").
- `DuplicateFingerprint(oldRepo, newRepo)` -> `showDialog(RepoDialog.Conflict(oldRepo, newRepo))`.
- `Success` and `Error` -> `else -> {}`. **OBSERVED**: no feedback at all. The Create dialog has already dismissed itself (section 4.4), so a generic error is silent.

`CreateMangaExtensionRepo.await(indexUrl)` (`domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt`, area:18-extensions-manga) for context: `indexUrl.toHttpUrlOrNull()?.toString()?.takeIf { matches ^https://.*/index\.min\.json$ }` else `InvalidUrl`; then `baseUrl = formatted.removeSuffix("/index.min.json")`; then `service.fetchRepoDetails(baseUrl)` (null -> `InvalidUrl`, so network failures are reported as "Invalid repo URL"); then insert. On `SaveExtensionRepoException`: if `getRepo(baseUrl) != null` -> `RepoAlreadyExists`; else if `getRepoBySigningKeyFingerprint(fp) != null` -> `DuplicateFingerprint(match, new)`; else `Error`.

#### 4.2.2 `replaceRepo(newRepo)` (lines 71-75)
`launchIO { replaceExtensionRepo.await(newRepo) }`. Replaces the existing repo with the same signing-key fingerprint (area:18). No toast. Dialog is dismissed by the UI before this completes (section 4.4).

#### 4.2.3 `refreshRepos()` (lines 80-88)
Only when Success: `launchIO { updateExtensionRepo.awaitAll() }`. No feedback.

#### 4.2.4 `deleteRepo(baseUrl)` (lines 93-97)
`launchIO { deleteExtensionRepo.await(baseUrl) }`. **OBSERVED**: unlike the anime model, the manga model does not call `findAvailableExtensions()` after delete, nor after create or replace.

#### 4.2.5 `showDialog` / `dismissDialog` (lines 99-115)
Both no-op while Loading.

### 4.3 Events and state types
- `RepoEvent.InvalidUrl` -> `MR.strings.invalid_repo_name` ("Invalid repo URL").
- `RepoEvent.RepoAlreadyExists` -> `MR.strings.error_repo_exists` ("This repo already exists!").
- `RepoScreenState.Success.isEmpty` = `repos.isEmpty()`. `oldRepos` is declared but never read in scope (dead field).

### 4.4 Dialogs (`components/ExtensionReposDialogs.kt`)

**Create** (`ExtensionRepoCreateDialog`, lines 28-92)
- Title `action_add_repo` ("Add repo").
- Body: `AYMR.strings.action_add_repo_message` = `Add additional repos to %s. This should be a URL that ends with "index.min.json".` where `%s` is the app name (Aniyomi's string resource; Kurobayomi substitutes its own name). Then one `OutlinedTextField` (single line, Uri keyboard), label `label_add_repo_input` ("Repo URL").
- `nameAlreadyExists = repoUrls.contains(name)` where `repoUrls` are **base** URLs (no `/index.min.json`). **OBSERVED**: the typed value is an index URL, so this check almost never matches. The real duplicate check is in the domain.
- Supporting text: `error_repo_exists` if `name.isNotEmpty() && nameAlreadyExists`, else `information_required_plain` ("*required"). `isError` mirrors the first condition.
- Confirm `Add`: enabled `name.isNotEmpty() && !nameAlreadyExists`; click runs `onCreate(name)` then `onDismissRequest()` (dismiss immediately; no processing state).
- Cancel: `Cancel`. Focus after 0.1 s.

**Delete** (`ExtensionRepoDeleteDialog`, lines 94-122)
- Title `action_delete_repo` ("Delete repo"). Body `delete_repo_confirmation` = `Do you wish to delete the repo "%s"?` with the **base URL**.
- `OK` -> `onDelete(); onDismissRequest()`. Cancel `Cancel`.

**Conflict** (`ExtensionRepoConflictDialog`, lines 124-155)
- Title `action_replace_repo_title` = "Signing Key Fingerprint Already Exists".
- Body `action_replace_repo_message` = `Repository %1$s has the same Signing Key Fingerprint as %2$s.` + newline + `If this is expected, %2$s will be replaced, otherwise contact your repo maintainer.` with `%1$s = newRepo.name`, `%2$s = oldRepo.name`.
- Confirm `Replace` (`action_replace_repo`) -> `onMigrate(); onDismissRequest()`. Cancel.

**Confirm / deep link** (`ExtensionRepoConfirmDialog`, lines 157-187)
- Title `action_add_repo` ("Add repo"). Body `add_repo_confirmation` = `Do you wish to add the repo "%s"?` with the **index URL** (`dialog.url`).
- `Add` -> `onCreate(); onDismissRequest()`. Cancel.

### 4.5 Page and list
- Title `label_extension_repos` ("Extension repos"); Refresh action; Extended FAB "Add".
- Empty: `EmptyScreen(information_empty_repos)` = "You have no repos set."
- List card: label icon + `repo.name` (titleMedium). Actions in order:
  1. `OpenInNew` (`AutoMirrored.Outlined.OpenInNew`), `action_open_in_browser` -> `openInBrowser(repo.website)`.
  2. `ContentCopy`, `action_copy_to_clipboard` -> copies `"${repo.baseUrl}/index.min.json"` (label and content identical).
  3. `Delete`, `action_delete` -> `onClickDelete(repo.baseUrl)`.
- No Discord button for manga repos (contrast section 3.5).

### 4.6 Edge cases (manga)
- Invalid URL, unreachable host, non-JSON, or repo without fingerprint: all surface as "Invalid repo URL" (because `fetchRepoDetails` returns null).
- Deep link arrives while Loading: lost (section 13).
- Duplicate typed twice quickly: the second call sees `RepoAlreadyExists` and toasts.

---

## 5. Create backup (`app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt`)

### 5.1 Options model (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt`)

`data class BackupOptions` (all 12 fields, defaults):

| Field | Default | Entry label (MR/AYMR key) | Group | enabled-when |
|---|---|---|---|---|
| `libraryEntries` | true | `AYMR.strings.entries` = "Library entries" | library | always |
| `chapters` | true | `AYMR.strings.chapters_episodes` = "Chapters and episodes" | library | `libraryEntries` |
| `tracking` | true | `MR.strings.track` = "Tracking" | library | `libraryEntries` |
| `history` | true | `MR.strings.history` = "History" | library | `libraryEntries` |
| `categories` | true | `MR.strings.categories` = "Categories" | library | always |
| `readEntries` | true | `AYMR.strings.non_library_settings` = "All read/watched entries" | library | `libraryEntries` |
| `appSettings` | true | `MR.strings.app_settings` = "App settings" | settings | always |
| `extensionStores` | true | `MR.strings.extensionStores` = "Extension stores" | settings | always |
| `customButton` | true | `AYMR.strings.custom_button_settings` = "Custom buttons" | settings | always |
| `sourceSettings` | true | `MR.strings.source_settings` = "Source settings" | settings | always |
| `privateSettings` | **false** | `MR.strings.private_settings` = "Include sensitive settings (e.g., tracker login tokens)" | settings | `appSettings || sourceSettings` |
| `extensions` | **false** | `MR.strings.label_extensions` = "Extensions" | extensions | always |

- Group membership and order: `libraryOptions` = [libraryEntries, chapters, tracking, history, categories, readEntries]; `settingsOptions` = [appSettings, extensionStores, customButton, sourceSettings, privateSettings]; `extensionOptions` = [extensions]. Section card titles: `MR.strings.label_library` ("Library"), `MR.strings.label_settings` ("Settings"), `MR.strings.label_extensions` ("Extensions").
- Bit order for the job input (`asBooleanArray()`): `[libraryEntries, categories, chapters, tracking, history, readEntries, appSettings, extensionStores, customButton, sourceSettings, privateSettings, extensions]` (12 booleans, index 0..11). `fromBooleanArray` reads the same indices.
- `canCreate()` = `libraryEntries || categories || appSettings || extensionStores || customButton || sourceSettings`. **Excludes** `extensions`, `privateSettings`, and the library sub-options. So "Extensions" alone cannot be backed up (OBSERVED).

### 5.2 Screen (`CreateBackupScreen.kt`)

- `Content()` (lines 38-109): `model = rememberScreenModel { CreateBackupScreenModel() }`.
- Launcher `chooseBackupDir` = `ActivityResultContracts.CreateDocument("application/*")` (lines 45-57). On a non-null URI: `takePersistableUriPermission(uri, READ | WRITE)`; `model.createBackup(context, uri)`; `navigator.pop()`. On null (cancelled): nothing happens and the screen stays open.
- Scaffold, AppBar title `pref_create_backup` ("Create backup"), navigateUp.
- `LazyColumnWithAction` (shared bottom-pinned full-width button, `presentation-core/.../LazyColumnWithAction.kt`): action label `action_create` ("Create"), `actionEnabled = state.options.canCreate()`.
- Click action (lines 72-82):
  - If `!BackupCreateJob.isManualJobRunning(context)` (tag `BackupCreator:manual` is RUNNING): `chooseBackupDir.launch(BackupCreator.getFilename())`. If `ActivityNotFoundException` (no picker app): toast `file_picker_error` = "No file picker app found".
  - Else toast `backup_in_progress` = "Backup is already in progress".
- Section 1 (only if `DeviceUtil.isMiui && DeviceUtil.isMiuiOptimizationDisabled()`): `WarningBanner(restore_miui_warning)` (see Drop, section 12).
- Three `SectionCard`s (Library, Settings, Extensions) each with `LabeledCheckbox`es. Checkbox checked = `getter(state.options)`. Toggle = `model.toggle(setter, checked)`. Enabled = `entry.enabled(state.options)`.

### 5.3 Create side effects
`CreateBackupScreenModel.createBackup(context, uri)` = `BackupCreateJob.startNow(context, uri, state.value.options)` (area:21-backup-format-and-create):
- `inputData` = `{ is_auto_backup: false, location_uri: uri.toString(), options: booleanArray(12) }`.
- `OneTimeWorkRequest`, tag `BackupCreator:manual`, `enqueueUniqueWork("BackupCreator:manual", ExistingWorkPolicy.KEEP, ...)`. KEEP means a second start while one is queued/running is silently dropped.
- Filename offered: `BackupCreator.getFilename()` = `"${BuildConfig.APPLICATION_ID}_" + SimpleDateFormat("yyyy-MM-dd_HH-mm", Locale.ENGLISH).format(Date()) + ".tachibk"` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:232-235`).
- Model state is in memory only. Options reset to defaults each time the screen opens.

### 5.4 Proposal (Rust)
- Same 12 bits, same labels, same `enabled` rules, same `canCreate` expression, same defaults.
- Save dialog: `gtk::FileDialog::save` with `set_initial_name(getFilename-equivalent)`, filter `*.tachibk`. Cancel = no-op.
- Busy check: query the job registry for a RUNNING job with tag `backup:manual` before opening the dialog; toast the same key if busy. Enqueue with KEEP semantics.
- Pop the screen after successful hand-off (not after the file is written) to match Android.

---

## 6. Restore backup (`app/src/main/java/eu/kanade/presentation/more/settings/screen/data/RestoreBackupScreen.kt`)

### 6.1 Options (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/RestoreOptions.kt`)

`data class RestoreOptions` (7 fields, defaults):
- Bit order `asBooleanArray()` = `[libraryEntries, categories, appSettings, extensionStores, customButtons, sourceSettings, extensions]` (indices 0..6).
- Defaults: all true except `extensions = false`.
- Entries (in order, no `enabled` lambda, so always enabled): `label_library` ("Library"), `categories` ("Categories"), `app_settings` ("App settings"), `extensionStores` ("Extension stores"), `AYMR.custom_button_settings` ("Custom buttons"), `source_settings` ("Source settings"), `label_extensions` ("Extensions").
- `canRestore()` = any of the 7 is true (includes `extensions`, unlike create).
- There is **no** private-settings entry on restore. Whatever sensitive settings the file contains are restored when `appSettings` or `sourceSettings` is true (area:22-backup-restore).

### 6.2 Validation (`validate(uri)`, lines 192-212)

Runs in the model `init` (line 173), i.e. **synchronously on the thread that creates the screen model** (main thread, during composition). OBSERVED. Algorithm:
1. `results = BackupFileValidator(context).validate(uri)`.
   - `BackupFileValidator.validate` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt:23-71`): decode the whole file with `BackupDecoder(context).decode(uri)`; any exception is rethrown as `IllegalStateException(e)`.
   - `sources = backupSources.associate { sourceId to name }` (manga); `animesources = backupAnimeSources.associate { sourceId to name }`.
   - Missing manga sources: entries where `mangaSourceManager.get(key) == null`; name `v` -> if `v.toLongOrNull()` is null use `v`, else `mangaSourceManager.getOrStub(id).toString()`; `.distinct().sorted()`.
   - Missing anime sources: same with `animeSourceManager`, computed separately, then appended: `missingSources = mangaMissing + animeMissing` (not re-sorted together).
   - Trackers: `syncId`s from `backupAnime[*].tracking` and `backupManga[*].tracking`, concatenated, `.distinct()`; each `trackerManager.get(syncId.toLong())` (null-safe, unknown ids dropped); keep those where `!isLoggedIn`; `.map { name }.sorted()`.
2. On exception from step 1: `setError(error = InvalidRestore(uri, e.message.toString()), canRestore = false)` and return. Note `e` is the `IllegalStateException` wrapper, so the text is `e.toString()` of the wrapper, e.g. `java.lang.IllegalStateException: java.io.EOFException`. Proposal: show the cause chain.
3. If `missingSources` or `missingTrackers` non-empty: `setError(MissingRestoreComponents(uri, sources, trackers), canRestore = true)`.
4. Else `setError(null, canRestore = true)`.

### 6.3 Screen (`Content()`, lines 47-99)
- `Scaffold` AppBar title `pref_restore_backup` ("Restore backup").
- `LazyColumnWithAction`: action label `action_restore` ("Restore"); `actionEnabled = state.canRestore && state.options.canRestore()`; click = `model.startRestore(); navigator.pop()`.
- Miui banner (same drop as section 5.2).
- If `state.canRestore`: one `SectionCard` with the 7 `LabeledCheckbox`es (no `enabled`).
- If `state.error != null`: `errorMessageItem(state.error)`.
- Initial state `State(error = null, canRestore = false, options = RestoreOptions())`. Since validation is synchronous, the first composed frame already has the result.

### 6.4 Error card text (`errorMessageItem`, lines 101-164)
One `SectionCard` containing a `SelectionContainer` around a `Text` built with `buildAnnotatedString`:
- `MissingRestoreComponents`:
  1. `appendLine(backup_restore_content_full)` = "You may need to install any missing extensions and log in to tracking services afterwards to use them."
  2. If sources non-empty: blank line; **bold** `backup_restore_missing_sources` = "Missing sources:"; then sources joined with separator `"\n- "` and prefix `"- "` (so `- A\n- B`), no trailing newline.
  3. If trackers non-empty: blank line; **bold** `backup_restore_missing_trackers` = "Trackers not logged into:"; trackers joined the same way.
- `InvalidRestore`:
  1. **bold** `invalid_backup_file` = "Invalid backup file:" + newline.
  2. `uri.toString()` + newline.
  3. blank line.
  4. **bold** `invalid_backup_file_error` = "Full error:" + newline.
  5. `message` + newline.
- Any other error object: `error.toString()` + newline.

### 6.5 Start (`startRestore()`, lines 184-190)
`BackupRestoreJob.start(context, uri, options)` (area:22-backup-restore): unique work tag `BackupRestore`, `ExistingWorkPolicy.KEEP`. There is **no** busy check here (OBSERVED; create has one). Screen pops immediately; no toast on success or on KEEP drop.

### 6.6 Proposal
- Validation runs on a worker (tokio). State `Validating` shows a spinner row in place of the options card; Restore button disabled until done.
- Use the same Missing/Invalid card text, with the full error chain.
- Add the busy check to restore (toast `backup_in_progress`), since KEEP otherwise drops silently.

---

## 7. Storage usage (`app/src/main/java/eu/kanade/presentation/more/settings/screen/data/StorageInfo.kt`)

### 7.1 OBSERVED
- `StorageInfo(modifier)` (lines 25-40): `storages = remember { DiskUtil.getExternalStorages(context) }`. Column with `spacedBy(padding.small)`, one `StorageInfo(file)` per root.
- `DiskUtil.getExternalStorages` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:18-31`): for each non-null entry of `ContextCompat.getExternalFilesDirs(context, null)`, root = `File(absolutePath.substringBefore("/Android/"))`; keep only if `Environment.getExternalStorageState(root)` is `MEDIA_MOUNTED` or `MEDIA_MOUNTED_READ_ONLY`.
- Per root (lines 42-75), all computed once per composition via `remember(file)` / `remember(available)`:
  - `available = DiskUtil.getAvailableStorageSpace(file)` = `StatFs.availableBlocksLong * blockSizeLong`, or `-1` on exception (lines 63-71 of DiskUtil).
  - `total = DiskUtil.getTotalStorageSpace(file)` = `blockCountLong * blockSizeLong`, or `-1` on exception (lines 19-26 of DiskUtil).
  - `availableText`/`totalText` = `Formatter.formatFileSize(context, x)`.
  - Header: `file.absolutePath`, style `header` (`tachiyomi.presentation.core.theme.header`).
  - Bar: `LinearProgressIndicator`, `clip(shapes.small)`, full width, height 12 dp, `progress = { 1 - (available / total.toFloat()) }` (**not clamped**).
  - Caption: `available_disk_space_info` = "Available: %1$s / Total: %2$s", `secondaryItemAlpha()`, style `bodySmall`.
- Values are not refreshed while the page is open.

### 7.2 Edge cases
- Exception on `StatFs`: `total = -1` -> progress = `1 - available/(-1)` = `1 + available` (huge). `available = -1` too -> progress = `1 - (-1 / -1)` = 0. OBSERVED wart; Proposal: hide the bar when `total <= 0`.
- `total = 0` -> division gives `NaN` or `Infinity` for `available > 0`. Same proposal.
- Zero external storages (none mounted): the section renders empty (no placeholder).

### 7.3 Proposal (Rust)
- Roots: parse `/proc/self/mountinfo`; keep mount points whose fs type is in {`ext2`, `ext3`, `ext4`, `btrfs`, `xfs`, `f2fs`, `vfat`, `exfat`, `ntfs`, `ntfs3`, `fuseblk`} and whose device is a real block device (`/dev/`). Always include the mount that holds the configured storage location (see area:13-core-util-storage-archive-torrent). Dedupe by `st_dev`. Upper bound: the list is the mount list, no cap.
- Stats: `rustix::fs::statvfs(path)`; `available = f_bavail * f_frsize`, `total = f_blocks * f_frsize` (use `f_frsize`, not `f_bsize`, to match `blockSizeLong`).
- Progress = `clamp((total - available) / total, 0.0, 1.0)`, bar hidden when `total == 0`.
- Formatting: see section 7.4.

### 7.4 Size formatting
Android `Formatter.formatFileSize` (AOSP) uses SI (1000-based) units with labels `B`, `kB`, `MB`, `GB`, `TB`. The mockup shows `12.3 GB`. Proposal for Rust: SI units, one decimal place when the value is below 10 of the unit, otherwise integer rounded half-up; `0 B` for zero. Exact AOSP rounding is not reproduced; add fixtures in `p2-edge-cases-and-tests.md` section 3.

---

## 8. Debug info (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt`)

Rendered as a `PreferenceScaffold` (`app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScaffold.kt`) with `titleRes = pref_debug_info` ("Debug info"). Items in order:

1. Text preference (navigates) `WorkerInfoScreen.TITLE` = "Worker info" -> `WorkerInfoScreen()`.
2. Text preference (navigates) `BackupSchemaScreen.TITLE` = "Backup file schema" -> `BackupSchemaScreen()`.
3. Group **"App info"** (title is hard-coded English, not localised):
   - "Version" = `AboutScreen.getVersionName(false)` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:239-269`): debug build -> `"Debug ${COMMIT_SHA}"`; preview build type -> `"Preview r${COMMIT_COUNT}"` + `" (${COMMIT_SHA})"`; else `"Stable ${VERSION_NAME}"`.
   - "Build time" = `AboutScreen.getFormattedBuildTime()` (lines 271-285): parse `BuildConfig.BUILD_TIME` as `Instant`, convert to system zone, format with `UiPreferences.dateFormat(app_date_format)` + `" " + ofLocalizedTime(SHORT)` (`DateExtensions.kt:19-23`). On parse failure, the raw string.
   - "Profile compilation status" (see the status map in section 8.1).
   - "WebView version" = `WebViewUtil.getVersion(context)` (Android System WebView).
4. Group **"Device info"**:
   - "Model" = `"${Build.MANUFACTURER} ${Build.MODEL} (${Build.DEVICE})"`.
   - If `DeviceUtil.oneUiVersion != null`: "OneUI version" = that double; else if `miuiMajorVersion != null`: "MIUI version".
   - "Android version" = `"$release (${Build.DISPLAY})"` where release = `Build.VERSION.RELEASE_OR_PREVIEW_DISPLAY` (API >= 33), else `RELEASE_OR_CODENAME` (API >= 30), else `RELEASE`.

### 8.1 Profile compilation status map (ART profile verifier)
`ProfileVerifier.getCompilationStatusAsync().await().profileInstallResultCode` mapped to:
- `RESULT_CODE_NO_PROFILE_INSTALLED` -> "No profile installed"
- `RESULT_CODE_COMPILED_WITH_PROFILE` -> "Compiled"
- `RESULT_CODE_COMPILED_WITH_PROFILE_NON_MATCHING` -> "Compiled non-matching"
- `RESULT_CODE_ERROR_CACHE_FILE_EXISTS_BUT_CANNOT_BE_READ`, `RESULT_CODE_ERROR_CANT_WRITE_PROFILE_VERIFICATION_RESULT_CACHE_FILE`, `RESULT_CODE_ERROR_PACKAGE_NAME_DOES_NOT_EXIST` -> "Error $result"
- `RESULT_CODE_ERROR_UNSUPPORTED_API_VERSION` -> "Not supported"
- `RESULT_CODE_PROFILE_ENQUEUED_FOR_COMPILATION` -> "Pending compilation"
- `RESULT_CODE_ERROR_NO_PROFILE_EMBEDDED` -> "No profile embedded"
- else -> "Unknown code $result"
- Initial value `"-"` while the async call runs.

### 8.2 Proposal (Rust)
Dropped (section 12): the profile status row. Keep the app info rows, replace WebView with WebKitGTK version, replace device rows with Linux facts (section 8.3). Add GTK and session rows.

### 8.3 Linux source of each row
| Row | Source |
|---|---|
| Version | `env!("CARGO_PKG_VERSION")` + `build.rs` git short SHA; "Debug" when `cfg(debug_assertions)` |
| Build time | `build.rs` writes RFC 3339 UTC into `KUROBAYOMI_BUILD_TIME`, honours `SOURCE_DATE_EPOCH` |
| WebKitGTK | `build.rs` runs `pkg-config --modversion webkitgtk-6.0` (or `webkit-6.0`) |
| GTK | `gtk::major_version()`, `gtk::minor_version()`, `gtk::micro_version()` |
| Session | `XDG_SESSION_TYPE` (wayland/x11) and `XDG_CURRENT_DESKTOP` |
| Model | `/sys/class/dmi/id/sys_vendor` + `product_name` (+ ` (board_name)`); absent files omitted |
| OS | `PRETTY_NAME` from `/etc/os-release` |
| Kernel | `rustix::system::uname().release()` |

---

## 9. Worker info (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/WorkerInfoScreen.kt`)

### 9.1 OBSERVED
- Title `TITLE` = "Worker info" (hard-coded English). Toolbar action: copy, icon `ContentCopy`, label `action_copy_to_clipboard`. Copies `TITLE` as the label and `enqueued + finished + running` as content (lines 76-80). No separators beyond the trailing newlines of each dump.
- Body: `LazyColumn(contentPadding = contentPadding + PaddingValues(horizontal = 16.dp), modifier = Modifier.horizontalScroll(rememberScrollState()))`. Sections in this order, each a `SectionTitle` (`titleMedium`, vertical padding 8 dp) then a `SectionText` (`softWrap = false`, `FontFamily.Monospace`): "Enqueued", "Finished", "Running".
- Model (`Model(context)`): `workManager = context.workManager`.
  - `enqueued` = `getWorkInfosFlow(WorkQuery.fromStates(ENQUEUED))`.
  - `running` = `fromStates(RUNNING)`.
  - `finished` = `fromStates(SUCCEEDED, FAILED, CANCELLED)`.
  - Each `.map(::constructString).stateIn(ioCoroutineScope, WhileSubscribed(), "")`.
- `constructString(list)`:
  - Empty -> `"-"` + newline.
  - Else for each `WorkInfo`:
    1. `Id: ${id}` (UUID)
    2. `Tags:` then one `" - $tag"` line per tag (leading single space, dash)
    3. `State: ${state}` (enum name: `ENQUEUED`, `RUNNING`, `SUCCEEDED`, `FAILED`, `CANCELLED`, `BLOCKED`)
    4. Only if `state == ENQUEUED`: `Next scheduled run: ${ts}` where `ts` = `LocalDateTime.ofInstant(Instant.ofEpochMilli(nextScheduleTimeMillis), systemDefault)` formatted with `UiPreferences.dateFormat(app_date_format)` + time; then `Attempt #${runAttemptCount + 1}`.
    5. blank line.

### 9.2 Tags present in the app (for the dump)
Work names and tags are defined by each job (area:26-library-update-jobs, area:21/22 for `BackupCreator` / `BackupRestore`, area:35 for updater/notification jobs). Tag strings seen in scope: `BackupCreator` (auto), `BackupCreator:manual`, `BackupRestore`.

### 9.3 Proposal (Rust)
- Job registry (`src/jobs/registry.rs`, see `p2-rust-design.md` section 5): every spawned job records `{ id: Uuid, tags: Vec<String>, state, next_run: Option<DateTime<Utc>>, attempts: u32 }`. Transitions: `Enqueued -> Running -> Succeeded|Failed|Cancelled`.
- Snapshot via `tokio::sync::watch`; the window subscribes while the page is open (matches `WhileSubscribed`).
- Same text format and `"-"` placeholder. Copy: `gdk::Clipboard::set_text` with the same concatenation.
- Horizontal scroll kept (monospace long lines).

---

## 10. Backup file schema (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/BackupSchemaScreen.kt`)

- Title `TITLE` = "Backup file schema" (hard-coded). Text = `ProtoBufSchemaGenerator.generateSchemaText(Backup.serializer().descriptor)` computed once (`remember`).
- Body: `Text(schema, monospace, verticalScroll, padding(contentPadding) + 16 dp)`.
- Copy action (`ContentCopy`, `action_copy_to_clipboard`): `copyToClipboard(TITLE, schema)`.
- The generated schema for the top-level `Backup` message begins `message Backup { repeated BackupManga backupManga = 1; repeated BackupCategory backupCategories = 2; repeated BackupAnime backupAnime = 3; ... }` as generated by kotlinx-serialization (the full field list is area:21-backup-format-and-create).

### 10.1 Proposal
- Ship the protobuf definition as `proto/backup.proto` (the same file the Rust decoder uses in area:21). Show the file content verbatim via `include_str!`. This replaces the generator output with a hand-readable schema. Field numbers must match the Aniyomi format (compatibility).

---

## 11. Shared helpers used by these screens

| Helper | OBSERVED behaviour | Proposal |
|---|---|---|
| `copyToClipboard(label, content)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:39-`) | No-op if content is blank. Sets primary clip. On Android API <= 32 (`S_V2`) shows toast `copied_to_clipboard` = `"Copied to clipboard:\n%1$s"` with `content.truncateCenter(50)`. | `gdk::Display::default().clipboard().set_text(content)`. Show the same toast (truncated to 50 chars, centre-ellipsis) because Linux desktops show no confirmation. No-op on blank. |
| `toast(resource)` / `toast(text)` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt:14-35`) | Short toast (`Toast.LENGTH_SHORT`, 2 s) on the application context. | In-window toast overlay (`gtk::Overlay` + `gtk::Revealer`), 2 s, bottom-centre, one at a time (new toast replaces old). No libadwaita toast. |
| `copied_to_clipboard` | see above | key kept |
| `LabeledCheckbox` | Checkbox with label, `enabled` flag | `gtk::CheckButton` with label, `set_sensitive(enabled)` |
| `SectionCard` | Card grouping with optional title | `gtk::Frame` or box with `.card` style class (theme-aware) |
| `EmptyScreen(textRes)` | centred text | centred `gtk::Label` with `.dim-label` |
| `LoadingScreen()` | centred indicator | `gtk::Spinner` centred |
| `WarningBanner(textRes)` | error-coloured bar | `gtk::Label` with `.error` style class in a box (only where a shared warning is needed; see section 12 for the MIUI drop) |

---

## 12. Drops and replacements (user-visible loss)

| Item | Where | Decision | What the user loses |
|---|---|---|---|
| MIUI optimisation warning banner (`restore_miui_warning`) | create and restore screens | **Drop** (P3) | Nothing on Linux. MIUI does not exist here. |
| OneUI / MIUI version rows | debug info | **Drop** (P3) | Nothing on Linux. |
| ART profile compilation status row | debug info | **Drop** (P3) | The row that tells whether Play profile-guided compilation is active. Not applicable to a native binary. |
| `takePersistableUriPermission` | create and restore | **Drop** (P3) | Nothing. Linux stores a path (or portal document path). Long-lived access is not needed. |
| `ActivityNotFoundException` -> "No file picker app found" | create | **Replace**: a GTK file dialog failure (no portal and no GTK chooser) shows a new toast `file_picker_error` mapped to "No file chooser available" | Same behaviour, different text. |
| Android Toast for copy on API >= 33 | clipboard | **Drop** the system confirmation; **keep** our own toast (section 11) | Nothing (Android 13+ shows its own clipboard overlay). |
| WorkManager job list | worker info | **Replace** with job registry | Android-only states such as `BLOCKED` will never occur; the dump format stays the same. |
| Compose `animateItem()` list animations | anime and manga lists | **Drop** animation | Item insert/remove animation. |
| Dialog focus workaround (100 ms delay) | create dialogs | **Replace** with `grab_focus()` after map | Nothing. |
| `Formatter.formatFileSize` | storage | **Replace** with own SI formatter | Exact AOSP rounding text (see section 7.4). |
| `DeviceUtil.isMiui` / `isMiuiOptimizationDisabled` probes | create, restore, debug | **Drop** | Nothing. |

No user-facing feature of the 18 files is dropped entirely except the Android-only rows above.

---

## 13. Cross-area interactions

- **area:18-extensions-manga**: `CreateMangaExtensionRepo`, `ReplaceMangaExtensionRepo`, `DeleteMangaExtensionRepo`, `UpdateMangaExtensionRepo`, `GetMangaExtensionRepo`, `ExtensionRepoService.fetchRepoDetails`. The screen depends on their result types (`InvalidUrl`, `RepoAlreadyExists`, `DuplicateFingerprint`, `Success`, `Error`).
- **area:19-extensions-anime**: `AddAnimeExtensionStore`, `GetAnimeExtensionStores.subscribe`, `RemoveAnimeExtensionStore`, `UpdateAnimeExtensionStores`, `AnimeExtensionStoreService.fetch`, `AnimeExtensionManager.findAvailableExtensions`.
- **area:20-extension-strategy-redesign**: how extension lists are refreshed after store changes.
- **area:21-backup-format-and-create**: `BackupCreateJob`, `BackupCreator`, `BackupOptions` semantics for auto vs manual, the `.tachibk` gzip-protobuf container, `Backup` message.
- **area:22-backup-restore**: `BackupRestoreJob`, `BackupDecoder`, `RestoreOptions` application, restore notifications.
- **area:26-library-update-jobs**: other WorkManager jobs that appear in the worker dump.
- **area:02-preferences-system**: `app_date_format` and the preference store used by `UiPreferences`.
- **area:36-ui-shell-navigation-security-webview**: how `SettingsScreen(destination)` and deep links are reached; `DefaultNavigatorScreenTransition`, `LocalBackPress`.
- **area:35-data-misc-updater-notification-export**: build-time metadata and updater row on About.
- **area:13-core-util-storage-archive-torrent**: `truncateCenter`, storage locations.
- **Part 1 of this area (47-settings-screens-general, other author)**: `SettingsMainScreen`, `SettingsDataScreen`, `SettingsTrackingScreen`, `SettingsAppearanceScreen`, `AboutScreen`, the Clear database screens and Licenses. Those screens open the pages in this part.

---

## 14. Concurrency and dispatchers

- Compose screens use `screenModelScope.launchIO` (Dispatchers.IO) for every mutation. State is a `MutableStateFlow` updated with `update {}` (atomic CAS).
- `ioCoroutineScope` (used by worker info flows) is an application-wide IO scope.
- The only synchronous IO on the UI thread is `RestoreBackupScreenModel.init` -> `validate` (section 6.2). Flagged.
- Deep-link `LaunchedEffect(url)` runs on the main dispatcher after composition; `showDialog`/`addFromDeeplink` mutate state from the main thread.
- Events (manga) use an unlimited `Channel`; the collector is `collectLatest` on the Compose scope.

Proposal: GTK main thread owns all widgets and page models (`Rc<RefCell<...>>`). IO runs on tokio. Results return through `async_channel` consumed by `glib::spawn_future_local`. Never block the main thread with file decoding.

---

## 15. User-visible message keys (English values)

### 15.1 Used by these 18 files
| Key (MR / AYMR) | English value (base strings) | Used in |
|---|---|---|
| `pref_app_language` | App language | 2 (title) |
| `label_default` | Default | 2 (first row) |
| `extensionStores` | Extension stores | 3, 9, 8 (create/restore labels) |
| `extensionStoresScreen.emptyLabel` | You haven't added an extension store yet | 9 |
| `extensionStoresScreen.addStore.title` | Add extension store | 8 |
| `extensionStoresScreen.addStore.alreadyExists` | This extension store already exists | 8 |
| `extensionStoresScreen.addStore.processing` | Processing… | 8 |
| `extensionStoresScreen.addStoreInput.inputLabel` | Store index URL | 8 |
| `extensionStoresScreen.addStoreDeeplink.bodyText` | Do you wish to add the extension store below? | 8 |
| `extensionStoresScreen.deleteStore.title` | Remove extension store | 8 |
| `extensionStoresScreen.deleteStore.body` | Do you wish to remove the extension store "%1$s" (%2$s)? | 8 |
| `label_extension_repos` | Extension repos | 4, 11 |
| `information_empty_repos` | You have no repos set. | 11 |
| `action_add_repo` | Add repo | 6, 5 |
| `action_add_repo_message` (AYMR) | Add additional repos to %s. This should be a URL that ends with "index.min.json". | 6 |
| `label_add_repo_input` | Repo URL | 6 |
| `action_delete_repo` | Delete repo | 6 |
| `delete_repo_confirmation` | Do you wish to delete the repo "%s"? | 6 |
| `action_replace_repo_title` | Signing Key Fingerprint Already Exists | 6 |
| `action_replace_repo_message` | Repository %1$s has the same Signing Key Fingerprint as %2$s. / If this is expected, %2$s will be replaced, otherwise contact your repo maintainer. | 6 |
| `action_replace_repo` | Replace | 6 |
| `add_repo_confirmation` | Do you wish to add the repo "%s"? | 6 |
| `invalid_repo_name` | Invalid repo URL | 5 (toast) |
| `error_repo_exists` | This repo already exists! | 5, 6 |
| `information_required_plain` | *required | 6, 8 |
| `action_webview_refresh` | Refresh | 9, 11 (Refresh action) |
| `action_open_in_browser` | Open in browser | 7, 10 |
| `action_copy_to_clipboard` | Copy to clipboard | 7, 10, 15, 16 |
| `action_delete` | Delete | 7, 10 |
| `action_add` | Add | 6, 8, 9, 11 |
| `action_cancel` | Cancel | 6, 8 |
| `action_ok` | OK | 6, 8 |
| `pref_create_backup` | Create backup | 12 |
| `action_create` | Create | 12 |
| `file_picker_error` | No file picker app found | 12 |
| `backup_in_progress` | Backup is already in progress | 12 |
| `restore_miui_warning` | Backup/restore may not function properly if MIUI Optimization is disabled. | 12, 13 (dropped) |
| `label_library` | Library | 12, 13 |
| `label_settings` | Settings | 12 |
| `label_extensions` | Extensions | 12, 13 |
| `entries` (AYMR) | Library entries | 12 |
| `chapters_episodes` (AYMR) | Chapters and episodes | 12 |
| `track` | Tracking | 12 |
| `history` | History | 12 |
| `categories` | Categories | 12, 13 |
| `non_library_settings` (AYMR) | All read/watched entries | 12 |
| `app_settings` | App settings | 12, 13 |
| `custom_button_settings` (AYMR) | Custom buttons | 12, 13 |
| `source_settings` | Source settings | 12, 13 |
| `private_settings` | Include sensitive settings (e.g., tracker login tokens) | 12 |
| `pref_restore_backup` | Restore backup | 13 |
| `action_restore` | Restore | 13 |
| `backup_restore_content_full` | You may need to install any missing extensions and log in to tracking services afterwards to use them. | 13 |
| `backup_restore_missing_sources` | Missing sources: | 13 |
| `backup_restore_missing_trackers` | Trackers not logged into: | 13 |
| `invalid_backup_file` | Invalid backup file: | 13 |
| `invalid_backup_file_error` | Full error: | 13 |
| `available_disk_space_info` | Available: %1$s / Total: %2$s | 14 |
| `pref_debug_info` | Debug info | 16 |
| `copied_to_clipboard` | Copied to clipboard:\n%1$s | 11 |

### 15.2 Hard-coded English (not in strings.xml)
"Worker info", "Backup file schema", "App info", "Version", "Build time", "Profile compilation status", "WebView version", "Device info", "Model", "OneUI version", "MIUI version", "Android version", "unknown error" (anime store add fallback), "Debug", "Preview r", "Stable", "Enqueued", "Finished", "Running", "Id:", "Tags:", "State:", "Next scheduled run:", "Attempt #", "-".

Proposal: keep the three debug screens English-only (developer tooling). Localise only user-facing keys.

### 15.3 New keys proposed for Kurobayomi
- `settings.language.restart_required` = "Restart Kurobayomi to apply the language."
- `settings.file_chooser.unavailable` = "No file chooser available" (replaces `file_picker_error` wording on Linux).

---

## 16. Keyboard and mouse (summary; full table in p2-ui-parity.md)
- Enter in single-line dialog text fields activates the primary button (proposal; Compose did not do this).
- Escape closes any dialog (cancel).
- F5 refreshes the store/repo list (proposal).
- Right-click on a list row opens the same actions as the card buttons (proposal; not in Android).
