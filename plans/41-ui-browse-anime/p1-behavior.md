# Area 41 (part 1): Anime browse UI, behavior spec

All paths are repo-relative to `/home/blackbox/aniyomi-src`. `OBSERVED` = what the Kotlin does. `Proposal:` = what Kurobayomi should do. Constants are quoted exactly as in source.

## 0. Conventions used everywhere in this area

- Debounce: `SEARCH_DEBOUNCE_MILLIS = 250L` (`app/src/main/java/eu/kanade/presentation/components/AppBar.kt:65`). Used by the extension search only in this part.
- Padding tokens (`presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt`): extraLarge 32dp, large 24dp, medium 16dp, mediumSmall 12dp, small 8dp, extraSmall 4dp. `SECONDARY_ALPHA = .78f`, `DISABLED_ALPHA = .38f` (same file). `topSmallPaddingValues = PaddingValues(top = MaterialTheme.padding.small)`.
- Secondary text alpha: `Modifier.secondaryItemAlpha()` = alpha 0.78 (`presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt:39`).
- Icon indicator size `IndicatorSize = 26.dp` (`app/src/main/java/eu/kanade/presentation/components/ItemDownloadIndicator.kt:36`).
- Grid spacing: `CommonEntryItemDefaults.GridHorizontalSpacer = 4.dp`, `GridVerticalSpacer = 4.dp`, `BrowseFavoriteCoverAlpha = 0.34f` (`app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:50-54`).
- Browse grid content padding: `contentPadding + PaddingValues(8.dp)`; list content padding `contentPadding + PaddingValues(vertical = 8.dp)`.
- Loading row (`BrowseSourceLoadingItem`, `app/src/main/java/eu/kanade/presentation/browse/BrowseSourceLoadingItem.kt`): fillMaxWidth, vertical padding 16dp, centered spinner (owned by area:40).
- In-library badge (`app/src/main/java/eu/kanade/presentation/browse/BrowseBadges.kt`): shown only when `anime.favorite`, icon `Icons.Outlined.CollectionsBookmark` (owned by area:40).
- Language display: `LocaleHelper.getSourceDisplayName(lang, context)` maps `"last_used"` to `MR.strings.last_used_source`, `"pinned"` to `MR.strings.pinned_sources`, `"other"` to `MR.strings.other_source`, `"all"` to `MR.strings.multi_lang`, otherwise `getLocalizedDisplayName(lang)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:30-36`). `getLocalizedDisplayName(null)` = `""`; `""` uses the system default locale; `"zh-CN"` maps to `zh-Hans`, `"zh-TW"` to `zh-Hant`; otherwise `Locale.forLanguageTag(lang)`; display name is `getDisplayName(locale)` with first char uppercased (`LocaleHelper.kt:55-70`).
- `LocaleHelper.comparator` (`LocaleHelper.kt:17-24`): `"all"` sorts first; otherwise compare `getLocalizedDisplayName(a)` vs `(b)` with `String.compareTo`.
- Default enabled languages: `LocaleHelper.getDefaultEnabledLanguages() = setOf("all", "en", Locale.getDefault().language)` (`LocaleHelper.kt:72-74`). Kurobayomi: use `"all"`, `"en"`, and the language part of `$LANG` (see p1-data-and-config.md).

## 1. Extensions tab: data model and grouping (AnimeExtensionsScreenModel)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt`.

### 1.1 Inputs

- `getExtensions.subscribe()` returns `AnimeExtensions(updates, installed, available, untrusted)` (a 4-tuple destructured as `(_updates, _installed, _available, _untrusted)`), built by `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt`.
- `currentDownloads: MutableStateFlow<Map<String, InstallStep>>` keyed by `pkgName` (see section 3).
- `state.searchQuery` (`String?`), debounced 250 ms after `distinctUntilChanged`.

### 1.2 GetAnimeExtensionsByType (the list source, OBSERVED)

Let `showNsfw = preferences.showNsfwSource().get()` (default `true`, key `show_nsfw_source`, see data file). Combined over `enabledLanguages`, `installedExtensionsFlow`, `untrustedExtensionsFlow`, `availableExtensionsFlow`:

1. Installed: filter `showNsfw || !it.isNsfw`; sort by `compareBy<Installed> { !it.isObsolete }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name }` (non-orphaned first, then case-insensitive name); `partition { it.hasUpdate }` gives `updates` and `installed`.
2. Untrusted: sort by `compareBy(String.CASE_INSENSITIVE_ORDER) { it.name }`. No NSFW filter is applied to untrusted.
3. Available: keep an extension only if no installed extension has the same `pkgName`, no untrusted extension has the same `pkgName`, and `showNsfw || !isNsfw`. Then `flatMap` each extension into one entry per source whose `lang in enabledLanguages`: `ext.copy(name = it.name, lang = it.lang, pkgName = "${ext.pkgName}-${it.id}", sources = listOf(it))`. Sort by case-insensitive `name`.

Consequence (OBSERVED): one multi-source extension becomes several Available rows, each with a synthetic `pkgName` `<pkg>-<sourceId>`. The synthetic name is the key used for install-step tracking and for the installer call.

### 1.3 Grouping in the screen model (OBSERVED)

`combine(queryDebounced, currentDownloads, getExtensions.subscribe())` then, for each emission:

1. `query = query ?: ""`.
2. `updates = _updates.filter(queryFilter(query)).map(mapper)`; if non-empty, group key `Header.Resource(MR.strings.ext_updates_pending)`.
3. `installed = _installed.filter(queryFilter).map(mapper)`; `untrusted = _untrusted.filter(queryFilter).map(mapper)`; if either non-empty, group key `Header.Resource(MR.strings.ext_installed)` with value `installed + untrusted` (untrusted rows are shown inside the Installed group, no own header).
4. `languagesWithExtensions = _available.filter(queryFilter).groupBy { it.lang }.toSortedMap(LocaleHelper.comparator).map { (lang, exts) -> Header.Text(LocaleHelper.getSourceDisplayName(lang, context)) to exts.map(mapper) }`. Each language is one group keyed by a `Header.Text` containing the display name.
5. Returned map `ItemGroups = MutableMap<Header, List<Item>>` preserves insertion order (updates, installed, then languages in comparator order).
6. `mapper(downloads)(ext) = Item(ext, downloads[ext.pkgName] ?: InstallStep.Idle)`.
7. `collectLatest` sets `isLoading = false` and `items = it`.

Note: `state.isEmpty` is `items.isEmpty()`.

### 1.4 Query filter (OBSERVED, exact)

`queryFilter(query)(extension)`:

- If `query.isEmpty()` return true.
- Split on `,`. For each term `input = term.trim()`: empty term yields false. The extension matches if ANY non-empty term matches (OR semantics across comma-separated terms).
- Available: `extension.sources.any { it.name.contains(input, ignoreCase=true) || it.baseUrl.contains(input, ignoreCase=true) || it.id == input.toLongOrNull() } || extension.name.contains(input, ignoreCase=true)`.
- Installed: `extension.sources.any { it.name.contains(input, true) || it.id == input.toLongOrNull() || (it is AnimeHttpSource && it.getHomeUrl().contains(input, true)) } || extension.name.contains(input, true)`.
- Untrusted: `extension.name.contains(input, ignoreCase = true)` only.

Note: `input.toLongOrNull()` makes a numeric term match an exact source id. The Available branch matches `baseUrl`, the Installed branch matches `getHomeUrl()` for HTTP sources.

### 1.5 Search state (OBSERVED)

- `search(query: String?)` sets `state.searchQuery = query` (null allowed; screen passes `state.searchQuery`).
- `searchQuery` null or "" shows everything. The empty-state message depends on it (section 2.3).

### 1.6 Refresh (OBSERVED)

`findAvailableExtensions()` sets `isRefreshing = true`, calls `extensionManager.findAvailableExtensions()` (network fetch, area:19), then `delay(1.seconds)` ("Fake slower refresh so it doesn't seem like it's not doing anything"), then `isRefreshing = false`. It runs once on model init too. `findAvailableExtensions` in the manager (`app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt:147-170`): on exception it logs and toasts `MR.strings.extension_api_error`, uses an empty list; then `enableAdditionalSubLanguages`, replaces `availableExtensionsMapFlow` keyed by `pkgName`, re-checks installed statuses, rebuilds the per-source data map.

### 1.7 Update-all (OBSERVED)

`updateAllExtensions()` launches a coroutine that iterates `state.value.items.values.flatten().map { it.extension }.filterIsInstance<Installed>().filter { it.hasUpdate }.forEach(::updateExtension)`. Each `updateExtension` call launches its own `launchIO` coroutine, so updates run concurrently, not sequentially. The set is the CURRENT visible items, so a search filter restricts what "Update all" touches.

### 1.8 Installing, updating, cancelling, uninstalling (OBSERVED)

- `installExtension(ext: Available)`: `launchIO { extensionManager.installExtension(ext).collectToInstallUpdate(ext) }`.
- `updateExtension(ext: Installed)`: `launchIO { extensionManager.updateExtension(ext).collectToInstallUpdate(ext) }`. `AnimeExtensionManager.updateExtension` returns `emptyFlow()` if no available entry with the same `pkgName` exists; otherwise it installs the available one.
- `collectToInstallUpdate(extension)`: `onEach { addDownloadState(extension, step) }.onCompletion { removeDownloadState(extension) }.collect()`. `addDownloadState` = `currentDownloads.update { it + (pkgName to step) }`; `removeDownloadState` = `it - pkgName`.
- `cancelInstallUpdateExtension(ext)` calls `extensionManager.cancelInstallUpdateExtension(ext)` which calls `installer.cancelInstall(extension.pkgName)`.
- `uninstallExtension(ext)` calls `extensionManager.uninstallExtension(ext)` which calls `installer.uninstallApk(extension.pkgName)`.
- `trustExtension(ext: Untrusted)`: `launch { extensionManager.trust(ext) }`. `trust` returns early if the untrusted entry is missing; else writes the trust record (`trustExtension.trust(pkgName, versionCode, signatureHash)`), removes the entry from `untrustedExtensionsMapFlow`, loads the extension from its package name and, on success, registers it as installed.

### 1.9 Updates badge (OBSERVED)

`preferences.animeExtensionUpdatesCount().changes()` writes `state.updates` (Int). The tab shows `TabContent.badgeNumber = state.updates.takeIf { it > 0 }`.

### 1.10 Installer setting (OBSERVED)

`basePreferences.extensionInstaller().changes()` writes `state.installer: BasePreferences.ExtensionInstaller?`. The list UI reads `installer?.requiresSystemPermission`. Enum (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36`): `LEGACY(true)`, `PACKAGEINSTALLER(true)`, `SHIZUKU(false)`, `PRIVATE(false)`. Linux replacement for this: see section 26 and p1-rust-design.md.

### 1.11 Install step model (OBSERVED)

`enum class InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }` (`app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt:3-9`). `isCompleted()` returns `this == Installed || this == Error || this == Idle`. Non-completed steps are `Pending`, `Downloading`, `Installing`.

Per-row presentation (`AnimeExtensionItem` / `AnimeExtensionItemActions`, section 2.4):
- Icon: while not completed, a `CircularProgressIndicator` of 40dp (strokeWidth 2dp) sits under the icon, and the icon gets 8dp padding (animated to 0 when idle).
- Text: appended DotSeparator plus `ext_pending` / `ext_downloading` / `ext_installing` when not completed. `else -> error("Must not show non-install process text")` for Installed/Error (not displayed).

## 2. Extensions tab UI (AnimeExtensionScreen and tab wiring)

### 2.1 Tab wiring (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsTab.kt`)

- Title `AYMR.strings.label_anime_extensions` ("Anime Extensions").
- `badgeNumber = updates if > 0`.
- `searchEnabled = true`.
- Overflow actions in order: `action_filter` ("Filter", pushes `AnimeExtensionFilterScreen()`), `extensionStores` ("Extension stores", pushes `AnimeExtensionStoresScreen()`, area:19).
- Row tap (`onClickItem`, section 2.4) and long-click (below) are different.
- `onLongClickItem(extension)`:
  - `Available` -> `installExtension(extension)` (immediate install).
  - Otherwise: if `context.isPackageInstalled(extension.pkgName)` -> `uninstallExtension(extension)` directly; else set `privateExtensionToUninstall = extension` and show the confirm dialog.
- `onOpenWebView(ext: Available)`: `ext.sources.getOrNull(0)?.let { push WebViewScreen(url = it.baseUrl, initialTitle = it.name, sourceId = it.id) }`.
- `onOpenExtension(ext)` pushes `AnimeExtensionDetailsScreen(it.pkgName)`.
- `onTrustExtension`, `onUninstallExtension`, `onUpdateExtension`, `onInstallExtension`, `onRefresh` map to model methods; `onClickItemCancel` maps to `cancelInstallUpdateExtension`; `onClickUpdateAll` to `updateAllExtensions`.
- Confirm dialog `AnimeExtensionUninstallConfirmation` (title `ext_confirm_remove` = "Remove Extension?", text `remove_private_extension_message` = `Do you really want to remove "%s" extension?`, confirm `ext_remove` = "Remove", dismiss `action_cancel` = "Cancel"). Confirm calls `uninstallExtension` then dismisses.

### 2.2 Screen states (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt`)

Wrapped in `PullRefresh(refreshing = state.isRefreshing, onRefresh = onRefresh, enabled = !state.isLoading)`.

- `isLoading` -> `LoadingScreen(Modifier.padding(contentPadding))`.
- `isEmpty`:
  - if `searchQuery` not null or empty -> `MR.strings.no_results_found` ("No results found").
  - else -> `MR.strings.empty_screen` ("Well, this is awkward").
  - Plus one `EmptyScreenAction(stringRes = MR.strings.extensionStores, icon = Icons.Outlined.Settings, onClick = push AnimeExtensionStoresScreen())`.
- Otherwise `AnimeExtensionContent`.

### 2.3 List content (OBSERVED)

`FastScrollLazyColumn(contentPadding + topSmallPaddingValues)`:

1. If `!installGranted && state.installer?.requiresSystemPermission == true`: a `WarningBanner(ext_permission_install_apps_warning)` item keyed `extension-permissions-warning` that opens the Android "install unknown apps" permission (Dropped, T-41a-025). `installGranted` is `rememberRequestPackageInstallsPermissionState(initialValue = true)`, so on desktop the banner never shows.
2. For each `(header, items)` in `state.items`:
   - Header item keyed `extensionHeader-${header.hashCode()}` with `contentType "header"`:
     - `Resource(ext_updates_pending)` shows a `Button` with `ext_update_all` (text color `onPrimary`) calling `onClickUpdateAll`.
     - other `Resource` headers show no action.
     - `Text` headers show only the text.
   - Items keyed by type: `extension-untrusted-<hash>`, `extension-installed-<hash>`, `extension-available-<hash>`; `contentType "item"`.
3. Trust dialog (section 2.5).

### 2.4 Row behavior (OBSERVED)

Row `combinedClickable(onClick, onLongClick)` on `BaseBrowseItem`.

- Row click (`onClickItem`):
  - Available -> install.
  - Installed -> open details.
  - Untrusted -> open trust dialog.
- Row long press: section 2.1.
- Secondary action (`onClickItemSecondaryAction`) slot:
  - Available -> open WebView.
  - Installed -> open details.
  - Untrusted -> nothing.
- Primary action (`onClickItemAction`):
  - Available -> install.
  - Installed -> if `hasUpdate` update, else open details.
  - Untrusted -> trust dialog.

Action buttons (`AnimeExtensionItemActions`, icon-only `IconButton`s, row spacing `small`):

| installStep | extension | Buttons (in order) |
|---|---|---|
| not completed (Pending/Downloading/Installing) | any | Close icon, description `action_cancel` -> `onClickItemCancel` |
| `Error` | any | Refresh icon, `action_retry` -> `onClickItemAction` |
| `Idle` | Installed | Settings icon `action_settings` -> `onClickItemSecondaryAction` (details); then if `hasUpdate`: GetApp icon `ext_update` -> `onClickItemAction` |
| `Idle` | Untrusted | VerifiedUser icon `ext_trust` -> `onClickItemAction` (trust dialog) |
| `Idle` | Available | if `sources.isNotEmpty()`: Public icon `action_open_in_web_view` -> `onClickItemSecondaryAction`; then GetApp icon `ext_install` -> `onClickItemAction` |

Note: `installStep == InstallStep.Installed` (transient) renders no action buttons (`when` has no branch for it; `isIdle` is true, so only the `Idle`/`Error` branches apply and `Installed` matches neither). Proposal: treat `Installed` like `Idle` for buttons.

Item content (`AnimeExtensionItemContent`):
- Title line: `(Torrent)` inline magnet icon prefix when `extension.isTorrent`, then `extension.name`. One line, ellipsis.
- Subline (FlowRow, `secondaryItemAlpha`, `bodySmall`, spacing extraSmall):
  - if Installed and `lang` not empty -> localized language display name.
  - if `versionName` not empty -> version.
  - warning, uppercased, error color, one line: Untrusted -> `ext_untrusted` ("Untrusted"), Installed and `isObsolete` -> `ext_obsolete` ("Orphaned"), `isNsfw` -> `ext_nsfw_short` ("18+"). Precedence is that order.
  - install step text (see 1.11).

Icon: `AnimeExtensionIcon(extension, modifier = matchParentSize().padding(padding))`, 40dp box.

### 2.5 Trust dialog (OBSERVED)

`ExtensionTrustDialog` (shared, owned by area:40, `app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt:521-546`): title `untrusted_extension` ("Untrusted extension"), text `untrusted_extension_message` ("Malicious extensions can read any stored login credentials or execute arbitrary code.\n\nBy trusting this extension, you accept these risks."), confirm `ext_trust` ("Trust") -> `onTrustExtension(trustState)`, dismiss button `ext_uninstall` ("Uninstall") -> `onUninstallExtension(trustState)` (uninstall the untrusted package), tap outside -> just close.

### 2.6 Extension filter (languages) (OBSERVED)

Model `AnimeExtensionFilterScreenModel`:
- `combine(getExtensionLanguages.subscribe(), preferences.enabledLanguages().changes())`. `getExtensionLanguages` = `availableExtensionsFlow.flatMap { ext.sources.map { it.lang } }.distinct().sortedWith(compareBy<String> { it !in enabledLanguage }.then(LocaleHelper.comparator))`. So enabled languages first, then comparator order.
- `catch` -> log, send `FailedFetchingLanguages` on the Channel; the screen toasts `MR.strings.internal_error` ("InternalError: Check crash logs for further information").
- State `Success(languages, enabledLanguages)` `isEmpty = languages.isEmpty()`.
- `toggle(language) = toggleLanguage.await(language)` which flips membership in `preferences.enabledLanguages()` (`app/src/main/java/eu/kanade/domain/source/interactor/ToggleLanguage.kt`).

Screen: `Scaffold` title `MR.strings.label_extensions` ("Extensions"). Empty -> `empty_screen`. Otherwise `LazyColumn` of `SwitchPreferenceWidget(title = LocaleHelper.getSourceDisplayName(language, ctx), checked = language in enabledLanguages, onCheckedChanged = toggle)`.

Note: this language set is the set of languages of AVAILABLE extensions (remote index), not installed ones. The filter also changes which Available rows show (section 1.2).

## 3. Install and update engine boundary (what the UI relies on)

- `AnimeExtensionManager.installExtension(ext)` returns `Flow<InstallStep>` via `installer.downloadAndInstall(extension.apkUrl, extension)`. Its progress is reported as steps; the flow completes at Installed or Error. Cancelling collects with `cancelInstall(pkgName)`.
- Linux replacement: see p1-rust-design.md section 6 (extension install = download an archive of the extension bundle, verify, place under the extension dir; area:20 decides the mechanism). The UI here consumes only `Flow<InstallStep>` plus a cancel.

## 4. Extension details (AnimeExtensionDetailsScreenModel / AnimeExtensionDetailsScreen)

### 4.1 Model (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeExtensionDetailsScreenModel.kt`)

Three coroutines in `screenModelScope.launch`:

1. Watches `extensionManager.installedExtensionsFlow.map { firstOrNull { pkgName == pkgName } }`. If null -> send `Uninstalled` event; else `state.extension = it`.
2. `state.collectLatest { if extension == null return; getExtensionSources.subscribe(extension).map { sortedWith(compareBy({ !it.enabled }, { if labelAsName: source.name else getSourceDisplayName(source.lang).lowercase() })) }.catch { log; _sources = persistentListOf() }.collectLatest { _sources = it } }`.
   - Sort: enabled sources first (`!enabled` is false for enabled), then by display label lowercased.
   - Note the label rule uses `item.source.name.takeIf { item.labelAsName } ?: LocaleHelper.getSourceDisplayName(item.source.lang, context).lowercase()`.
3. `preferences.incognitoAnimeExtensions().changes().map { pkgName in it }.distinctUntilChanged()` -> `state.isIncognito`.

`GetAnimeExtensionSources.subscribe(extension)` (`app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt`):
- `isMultiSource = sources.size > 1`; `isMultiLangSingleSource = isMultiSource && sources.map { name }.distinct().size == 1`.
- Emits on `disabledAnimeSources()` changes; per source `enabled = id.toString() !in disabledSources`; `labelAsName = isMultiSource && !isMultiLangSingleSource`.

Operations:
- `clearCookies()`: `urls = extension.sources.filterIsInstance<AnimeHttpSource>().flatMap { listOf(baseUrl, getHomeUrl()) }.filter { isNotEmpty }.distinct()`; `cleared = urls.sumOf { try network.cookieJar.remove(it.toHttpUrl()) catch -> log, 0 }`; logs "Cleared N cookies for: ...". No UI feedback beyond the log (OBSERVED, no toast).
- `uninstallExtension()`: `extensionManager.uninstallExtension(extension)`; screen pops on `Uninstalled`.
- `toggleSource(sourceId)`: `toggleSource.await(sourceId)` (enable = `!isEnabled(sourceId)`, i.e., flip).
- `toggleSources(enable)`: `toggleSource.await(sources.map { id }, enable)`.
- `toggleIncognito(enable)`: `toggleIncognito.await(pkgName, enable)`.

`ToggleAnimeSource` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/ToggleAnimeSource.kt`): the list `disabled_anime_sources` (`hidden_anime_catalogues`), `await(id, enable)` removes or adds `"$sourceId"`.

`ToggleAnimeIncognito` (`ToggleAnimeIncognito.kt`): add or remove the pkgName in `incognito_anime_extensions`.

### 4.2 Screen (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt`)

- Wrapper: while `isLoading` (extension null or sources null) show `LoadingScreen`.
- App bar title `MR.strings.label_extension_info` ("Extension info").
- Actions:
  - `url` derived: `"https://raw.githubusercontent.com/(.+?)/(.+?)/.+"` matched against `extension.store?.indexUrl`; if matched -> `"https://github.com/$user/$repo"`; else `store?.indexUrl`. If `url != null`, a non-overflow action `action_open_repo` ("Open source repo", icon `AutoMirrored.Outlined.Launch`) opens the URL via the URI handler.
  - Overflow: `action_enable_all` ("Enable all") -> `toggleSources(true)`, `action_disable_all` ("Disable all") -> `toggleSources(false)`, `pref_clear_cookies` ("Clear cookies") -> `clearCookies`.
- Content (`ScrollbarLazyColumn(contentPadding)`):
  1. If `extension.isObsolete` -> `WarningBanner(obsolete_extension_message)` ("This extension is not associated with any store. It will not receive updates, may not function properly, and can cause issues with the app. Uninstalling it is recommended.").
  2. Header (see 4.3).
  3. For each source in `sources`, keyed by `source.id`, a row (`SourceSwitchPreference`, 4.4).
- Missing extension (`extension == null`): `EmptyScreen(empty_screen)`.
- NSFW dialog: opened from the 18+ info cell (4.3). `NsfwWarningDialog` (shared, area:40): text `ext_nsfw_warning` ("Sources from this extension may contain NSFW (18+) content"), OK `action_ok`, dismiss = confirm.

### 4.3 Header (OBSERVED)

Column:
- Tap anywhere on the top block copies debug information to the clipboard with label `"Extension Debug information"`. Text (exact):
  ```
  Extension name: <name> (lang: <lang>; package: <pkgName>)
  Extension version: <versionName> (lib: <libVersion>; version code: <versionCode>)
  NSFW: <isNsfw>
  ```
  and for Installed extensions appended after a blank line:
  ```
  Update available: <hasUpdate>
  Orphaned: <isObsolete>
  Shared: <isShared>
  Repository: <store.indexUrl>   (only if store != null)
  ```
- Icon 112dp (`AnimeExtensionIcon(density = DisplayMetrics.DENSITY_XXXHIGH)`), name (`headlineSmall`, centered), stripped package name = `pkgName.substringAfter("eu.kanade.tachiyomi.animeextension.")` (`bodySmall`).
- Info row (`SpaceEvenly`): `InfoText(versionName, ext_info_version "Version")`, divider, `InfoText(getSourceDisplayName(lang), ext_info_language "Language")`, and when `isNsfw`: divider + `InfoText("18+" in error color, bold, ext_info_age_rating "Age rating" + " ⓘ", onClick = show NSFW dialog)` with weight 1.5 on the language cell.
- Buttons row: `OutlinedButton(ext_uninstall "Uninstall")` (weight 1) -> `onClickUninstall`; if `isShared`, a `Button(ext_app_info "App info")` opens the Android app-info page (Dropped, section 26).
- Incognito row (`TextPreferenceWidget`): title `pref_incognito_mode` ("Incognito mode"), subtitle `pref_incognito_mode_extension_summary` ("Pause reading history for extension"), icon `R.drawable.ic_glasses_24dp`, trailing `Switch(checked = isIncognito)`.
- Divider.

### 4.4 Source switch rows (OBSERVED)

`TextPreferenceWidget(title, onPreferenceClick = onClickSource(source.id))`:
- title = `source.source.toString()` if `labelAsName` else `getSourceDisplayName(source.source.lang)`.
- `AnimeSource.toString()` = `"$name (${lang.uppercase()})"` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:117`).
- Trailing: if `source is ConfigurableAnimeSource` -> `IconButton(Settings, label_settings "Settings")` -> `onClickSourcePreferences(sourceId)`. Then `Switch(checked = enabled, onCheckedChange = null)`.

## 5. Source preferences screen (AnimeSourcePreferencesScreen)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt`):
- Wait for `ifAnimeSourcesLoaded()` (else LoadingScreen).
- `Scaffold` with `AppBar(title = AnimeSourceManager.getOrStub(sourceId).toString(), navigateUp = pop)`.
- Body: an `AndroidView` `FragmentContainerView` hosting `SourcePreferencesFragment.getInstance(sourceId)` (commits once; later recompositions call a reflection hack `onContainerAvailable`).
- Fragment: `getContext()` wraps in `ContextThemeWrapper(theme R.attr.preferenceTheme)`. `onCreatePreferences`: if `source is ConfigurableAnimeSource`: `preferenceDataStore = SharedPreferencesDataStore(source.sourcePreferences())`, `source.setupPreferenceScreen(screen)`, then for each pref: `isIconSpaceReserved = false`, `isSingleLineTitle = false`, `DialogPreference` with empty `dialogTitle` gets `dialogTitle = title`, and `EditTextPreference` gets `setOnBindEditTextListener { it.setIncognito(lifecycleScope) }` (incognito IME).
- Non-configurable sources show an empty screen (no preferences).

Proposal: native GTK page built from a Rust `SourcePreferenceSpec` (see p1-rust-design.md section 8). Replace the fragment. Behavior to keep: the title is `toString()` of the source, the page reads and writes the source's own preference store, EditText fields use a non-learning input (incognito equivalent `gtk::Text` with input-purpose and no IM preedit persistence is not guaranteed; proposal: `gtk::Entry` with `set_input_hints(InputHints::NO_SPELLCHECK | InputHints::PRIVATE)`), and dialog-type prefs use a dialog titled with the pref title.

## 6. Source list (AnimeSourcesScreen)

Presentation only; the data model `AnimeSourcesScreenModel` is part 2. OBSERVED (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesScreen.kt`):

- States: `isLoading` -> `LoadingScreen(padding)`; `isEmpty` -> `EmptyScreen(source_empty_screen "No source found")` (base English text, section 24).
- List `ScrollbarLazyColumn(contentPadding + topSmallPaddingValues)`. Items are `AnimeSourceUiModel`:
  - `Header(language)`: text `getSourceDisplayName(language)`, style `MaterialTheme.typography.header`, padding horizontal medium, vertical small.
  - `Item(source)`: keyed `"source-${source.key()}"`.
- Item row: `BaseAnimeSourceItem(source, onClickItem = onClickItem(source, Listing.Popular), onLongClickItem = onLongClickItem(source), action = {...})`.
  - Action slot: if `source.supportsLatest` -> `TextButton` with text `latest` ("Latest", color primary) -> `onClickItem(source, Listing.Latest)`.
  - Then `AnimeSourcePinButton(isPinned = Pin.Pinned in source.pin, onClick = onClickPin(source))`: IconButton; pinned icon `Icons.Filled.PushPin` in primary tint; unpinned `Icons.Outlined.PushPin` tinted `onBackground.copy(alpha = SECONDARY_ALPHA)`. Content description `action_unpin` ("Unpin") when pinned else `action_pin` ("Pin").
- `Listing` is `BrowseAnimeSourceScreenModel.Listing` (part 2: `Popular` and `Latest`).
- `AnimeSourceOptionsDialog(source, onClickPin, onClickDisable, onDismiss)`: AlertDialog with `title = source.visualName` (`name` or `"$name ($LANG)"` when lang not empty; `AnimeSource.visualName`, `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:13-17`); items: pin/unpin text (padding 16dp vertical, full width), and `action_disable` ("Disable") only when `source.id != LocalAnimeSource.ID`. No confirm button. Local source = `LocalAnimeSource.ID = 0L` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:365`).
- Pin semantics (`domain/src/main/java/tachiyomi/domain/source/anime/model/Pin.kt`): `Unpinned = 0b00`, `Pinned = 0b01`, `Actual = 0b10`. The pin set is `pinned_anime_catalogues` (section 12 of p1-data-and-config.md). `Pin.Pinned in source.pin` is a membership check on the pins set.

Header-language grouping for installed sources is produced by the part 2 model; this part just renders `AnimeSourceUiModel`.

## 7. Browse source screen (BrowseAnimeSourceContent and toolbar)

### 7.1 Content state machine (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/BrowseAnimeSourceScreen.kt`)

Inputs: `animeList: LazyPagingItems<StateFlow<Anime>>`, `displayMode`, `columns: GridCells`.

Derived:
- `errorState = refresh (if Error) ?: append (if Error)`.
- `getErrorMessage(state) = with(context) { state.error.formattedMessage }` (`eu.kanade.presentation.util.formattedMessage`, owned by area:36/13).

Rules, in order:
1. `LaunchedEffect(errorState)`: if `itemCount > 0 && errorState is LoadState.Error`, show a snackbar with message = error text, `actionLabel = action_retry` ("Retry"), `duration = Indefinite`. `Dismissed` -> dismiss current snackbar; `ActionPerformed` -> `animeList.retry()`.
2. If `itemCount <= 0 && errorState is Error`: full EmptyScreen with the error message and actions:
   - Local source (`source is LocalAnimeSource`): one action `local_source_help_guide` ("Local source guide", icon `HelpOutline`) -> `onLocalAnimeSourceHelpClick` (`LocalAnimeSource.HELP_URL = "https://aniyomi.org/help/guides/local-anime/"`).
   - Else three actions in order: `action_retry` ("Retry", icon Refresh) -> `animeList::refresh`; `action_open_in_web_view` ("Open in WebView", icon Public) -> `onWebViewClick`; `label_help` ("Help", icon HelpOutline) -> `onHelpClick` (`Constants.URL_HELP = "https://aniyomi.org/docs/guides/troubleshooting/"`).
   - Return (no grid).
3. If `itemCount == 0 && refresh is Loading`: `LoadingScreen(padding)`.
4. Otherwise dispatch on `displayMode`:
   - `ComfortableGrid` -> `BrowseAnimeSourceComfortableGrid`.
   - `List` -> `BrowseAnimeSourceList` (receives `entries` and `topBarHeight` for container height).
   - `CompactGrid`, `CoverOnlyGrid` -> `BrowseAnimeSourceCompactGrid`.

Grids (`BrowseAnimeSource{Comfortable,Compact}Grid.kt`): `LazyVerticalGrid(columns, contentPadding + 8dp, verticalArrangement spaced 4dp, horizontalArrangement spaced 4dp)`:
- `prepend is Loading` -> full-span `BrowseSourceLoadingItem` at top.
- items: `animeList[index]?.collectAsState()`; each cell `EntryComfortableGridItem` / `EntryCompactGridItem` with `title = anime.title`, `coverData = AnimeCover(animeId, sourceId, isAnimeFavorite = favorite, url = thumbnailUrl, lastModified = coverLastModified)`, `coverAlpha = 0.34` if favorite else 1, `coverBadgeStart = InLibraryBadge(favorite)`, `onClick`, `onLongClick` (default `onClick`).
- `refresh is Loading || append is Loading` -> full-span loading item at bottom.

List (`BrowseAnimeSourceList.kt`): `BoxWithConstraints` measures `containerHeightPx = maxHeight`. `LazyColumn(state, contentPadding + vertical 8dp)`:
- first item: prepend loading.
- rows: `EntryListItem(title, coverData, coverAlpha, badge = InLibraryBadge, onClick, onLongClick, entries, containerHeight = containerHeightPx - topBarHeight)`.
- last item: refresh or append loading.

Note: `BrowseAnimeSourceList` uses `entries` (Int) and `containerHeight`; `entries` is passed by the caller (part 2) and defaults to 0 in `BrowseAnimeSourceContent`.

`MissingSourceScreen(source: StubAnimeSource, navigateUp)`: AppBar title `source.name`; `EmptyScreen(message = source_not_installed.format(source.toString()))` where English text is `"Source not installed: %1$s"` and `source.toString()` is `"$name (${LANG})"` or the bare id when `name` or `lang` is blank (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:59-60`).

### 7.2 Browse toolbar (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceToolbar.kt`)

- Title: `source?.name` via `AppBarTitle`.
- Search via `SearchToolbar(searchQuery, onChangeSearchQuery, onSearch, onClickCloseSearch = navigateUp)`.
- Action 1: `AppBar.Action(title = action_display_mode, icon = ViewList if displayMode == List else ViewModule, onClick = show radio menu)`.
- Overflow (in order):
  - if `source is LocalAnimeSource`: `label_help` -> `onHelpClick`;
  - else: `action_open_in_web_view` -> `onWebViewClick`.
  - if `source is ConfigurableAnimeSource`: `action_settings` -> `onSettingsClick`.
- Display mode radio menu (`DropdownMenu`, `RadioMenuItem`, checked when equal, selecting closes the menu and calls `onDisplayModeChange`):
  - `action_display_comfortable_grid` ("Comfortable grid") -> `ComfortableGrid`;
  - `action_display_grid` ("Compact grid") -> `CompactGrid`;
  - `action_display_list` ("List") -> `List`.
  - `CoverOnlyGrid` is not offered in this menu (but renders as compact if persisted).

Display-mode persistence: `sourcePreferences.sourceDisplayMode()` key `pref_display_mode_catalogue`, default `LibraryDisplayMode.default = CompactGrid`, deserialization map `COMFORTABLE_GRID`, `COMPACT_GRID`, `COVER_ONLY_GRID`, `LIST`; unknown -> default. Shared between manga and anime browse (the key has no anime prefix). Proposal: keep the shared key for parity; note in p1-data-and-config.md.

## 8. Global search screen (UI only; model is part 2)

### 8.1 Scaffold and results (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/GlobalAnimeSearchScreen.kt`)

- Toolbar: GlobalAnimeSearchToolbar (8.3) with `progress`, `total`.
- `GlobalSearchContent`: `LazyColumn(contentPadding)`; for each `(source, result)` in `state.filteredItems` (a `PersistentMap` already sorted by the model), an item keyed `source.id`:
  - `GlobalSearchResultItem(title, subtitle, onClick = onClickSource(source), modifier = animateItem())`.
  - Title: `fromSourceId?.let { "▶ ${source.name}".takeIf { source.id == fromSourceId } } ?: source.name`. The `▶` prefix (U+25B6 followed by space) marks the origin source during migration; non-origin rows keep their name.
  - Subtitle: `LocaleHelper.getLocalizedDisplayName(source.lang)` (NOT getSourceDisplayName; no "all" special case).
  - Body by result type: `Loading` -> `GlobalSearchLoadingResultItem()`; `Success` -> `GlobalAnimeSearchCardRow(titles = result.result, getAnime, onClick, onLongClick)`; `Error` -> `GlobalSearchErrorResultItem(message = throwable.message)`.

Note: GlobalSearchContent is called from the global screen without `fromSourceId` (null) and from the migrate search with `fromSourceId`.

### 8.2 Card row (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt`)

- Empty `titles` -> `EmptyResultItem` = `Text(no_results_found, padding horizontal medium, vertical small)`.
- `LazyRow(contentPadding = PaddingValues(small), horizontalArrangement spaced extraSmall)`.
- Each card: `Box(width 96dp)` containing `EntryComfortableGridItem(title, titleMaxLines = 3, coverData = asAnimeCover(), coverBadgeStart = InLibraryBadge(isFavorite), coverAlpha = 0.34 if favorite else 1, onClick, onLongClick)`.
- `getAnime(it)` is a `@Composable` `produceState` that subscribes to the DB row so the favorite badge updates live; the card uses the live `title` (from the DB) rather than the network title.

### 8.3 Toolbar (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchToolbar.kt`)

- Column with `Modifier.background(surface)`.
- `SearchToolbar(searchQuery, onChangeSearchQuery, onSearch, onClickCloseSearch = navigateUp, navigateUp, scrollBehavior)`.
- If `progress in 1..<total`: a `LinearProgressIndicator(progress = progress / total.toFloat())` aligned bottom-start, full width. So it is hidden at 0 and when all sources are finished.
- Horizontally scrolling chip row (padding horizontal small, spacing small):
  1. `FilterChip(selected = sourceFilter == PinnedOnly, onClick = onChangeSearchFilter(PinnedOnly), leading PushPin icon, label pinned_sources "Pinned")`.
  2. `FilterChip(selected = sourceFilter == All, onClick = onChangeSearchFilter(All), leading DoneAll, label all "All")`.
  3. VerticalDivider.
  4. `FilterChip(selected = onlyShowHasResults, onClick = onToggleResults(), leading FilterList, label has_results "Has results")`.
- The source filter chip row carries an author note at `GlobalAnimeSearchToolbar.kt:75`: "make this UX better; it only applies when triggering a new search". Observed: changing the source filter re-runs `search()` immediately in the model (`setSourceFilter` calls `search()`), and the author note says the UX is unclear. Proposal: keep behavior, make the chip change re-run the query and show a spinner on the chip.

### 8.4 Model behaviors the UI depends on (summary of part 2 `AnimeSearchScreenModel`, OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt`)

These are the semantics the migrate search inherits; part 2 documents them in full.
- State defaults: `sourceFilter = PinnedOnly`, `onlyShowHasResults = false` (overwritten live from `globalSearchFilterState`, app-state key `has_filters_toggle_state`), `items` empty.
- `enabledLanguages`, `disabledSources`, `pinnedSources` are snapshotted at model construction (`sourcePreferences.enabledLanguages().get()`, and the same for the other two), not live.
- `getEnabledSources()` default: `sourceManager.getAll()` filtered by `lang in enabledLanguages && "${id}" !in disabledSources`, sorted by `(!pinned, "${name.lowercase()} (${lang})")`.
- `search()`: returns if query null or blank; returns if same query and same source filter as last search; reuses prior results for the same query; each source runs `getSearchAnime(1, query, source.getFilterList())` on a fixed pool of 5 threads (`Executors.newFixedThreadPool(5)`), `async`, `awaitAll`. Results mapped through `networkToLocalAnime`. Errors become `Error(e)`. Note the user's filter UI state is NOT applied to global search (default filter list only).
- Result map sort (`sortComparator`, `AnimeSearchScreenModel.kt`): first key `(map[source] as? Success)?.isEmpty ?: true`, so only non-empty successes sort first and Loading or Error entries sort with empty results; then unpinned last (`"${id}" !in pinnedSources`); then `"${name.lowercase()} (${lang})"`.
- `filteredItems = items.filter { result.isVisible(onlyShowHasResults) }`, `isVisible = !onlyShowHasResults || (Success && !isEmpty)`.
- `progress = count(result !is Loading)`, `total = items.size`.

## 9. Source filter (AnimeSourcesFilterScreen, presentation)

OBSERVED (`app/src/main/java/eu/kanade/presentation/browse/anime/AnimeSourcesFilterScreen.kt`):
- Title `MR.strings.label_sources` ("Sources"). Empty -> `source_filter_empty_screen` ("No installed source found").
- `FastScrollLazyColumn(contentPadding)`. For each `(language, sources)` in `state.items`:
  - Header item keyed `language`, `contentType "source-filter-header"`: `SwitchPreferenceWidget(title = getSourceDisplayName(language), checked = language in enabledLanguages, onCheckedChanged = onClickLanguage(language))`.
  - If `language in enabledLanguages`: items for each source keyed `"source-filter-${source.key()}"`, `contentType "source-filter-item"`: `BaseAnimeSourceItem(source, showLanguageInContent = false, onClickItem = onClickSource(source), action = Checkbox(checked = "${source.id}" !in state.disabledSources, onCheckedChange = null))`.
- So toggling a language hides its sources in the list but does not change their enabled state.

Semantics: the header switch writes `enabled_languages` (ToggleLanguage). The per-source checkbox writes `hidden_anime_catalogues` (ToggleAnimeSource). The part 2 model supplies `items`, `enabledLanguages`, `disabledSources`.

## 10. Migration: flags (AnimeMigrationFlags)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/AnimeMigrationFlags.kt`.

Bit layout (exact):

| Name | Value | Binary | Title (MR/AYMR key, English) | Shown when |
|---|---|---|---|---|
| EPISODES | 0x01 | 0b00001 | `AYMR.strings.episodes` ("Episodes") | `anime.fetchType == FetchType.Episodes` only |
| CATEGORIES | 0x02 | 0b00010 | `MR.strings.categories` ("Categories") | always (when `anime != null`, and also when null) |
| CUSTOM_BACKGROUND | 0x04 | 0b00100 | `AYMR.strings.custom_background` ("Custom background") | `anime.hasCustomBackground(backgroundCache)` |
| CUSTOM_COVER | 0x08 | 0b01000 | `MR.strings.custom_cover` ("Custom cover") | `anime.hasCustomCover(coverCache)` |
| DELETE_DOWNLOADED | 0x10 | 0b10000 | `MR.strings.delete_downloaded` ("Delete downloaded") | `downloadCache.getDownloadCount(anime) > 0` |

Order of the rendered list is exactly: Episodes (if any), Categories, Custom cover, Custom background, Delete downloaded. Note CUSTOM_COVER is added before CUSTOM_BACKGROUND in `getFlags`, and the display order follows the `getFlags` insertion order: `EPISODES`, `CATEGORIES`, `CUSTOM_COVER`, `CUSTOM_BACKGROUND`, `DELETE_DOWNLOADED`.

Helpers: `hasEpisodes(v) = (v and 0x01) != 0`; the other four `has*` functions follow the same pattern with their own bit. `getFlags(anime?, defaultSelectedBitMap)`: `isDefaultSelected = (defaultSelectedBitMap and flag) != 0`. `getSelectedFlagsBitMap(selected: List<Boolean>, flags)` zips, keeps selected, ORs their `flag` values, `0` if none.

Default bitmap: `migrate_flags` preference, default `Int.MAX_VALUE` (0x7FFFFFFF). Because all low bits are set, EVERY flag is pre-selected by default, including DELETE_DOWNLOADED (OBSERVED). See the key-sharing bug in section 19 (E9).

Note: `canMigrate` (section 11.1) decides whether flags are shown at all; when `canMigrate` is false, the flag list is not rendered.

## 11. Migration: confirm dialog (MigrateAnimeDialog)

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeDialog.kt` (lines 68-197).

### 11.1 Rules (OBSERVED)

- `flags = remember { getFlags(oldAnime, screenModel.migrateFlags.get()) }`; `selectedFlags` = `isDefaultSelected` per flag.
- `canMigrate = remember { oldAnime.fetchType == newAnime.fetchType }`.
- While `state.isMigrating`: a full LoadingScreen with background `colorScheme.background.copy(alpha = 0.7f)` replaces the dialog.
- Title: `migration_dialog_what_to_include` ("Select data to include").
- Body (scrollable column):
  - `canMigrate`: a `LabeledCheckbox` per flag with `stringResource(flag.titleId)`.
  - else: an error row: `ErrorOutline` icon (26dp, `error` tint) and text in `error` color: if `oldAnime.fetchType == Seasons` -> `label_cant_migrate_season` ("Unable to migrate season to episode") else -> `label_cant_migrate_episode` ("Unable to migrate episode to season, select season to continue").
- Buttons (FlowRow, spacing extraSmall), in order:
  1. `action_show_anime` ("Show anime"): dismiss then `onClickTitle()` (push AnimeScreen(newAnime.id)). Always shown.
  2. `label_show_seasons` ("Show seasons"): only if `newAnime.fetchType != FetchType.Episodes` -> dismiss then `onClickSeasons()`.
  3. Spacer (weight 1).
  4. Only if `canMigrate`: `copy` ("Copy") -> `migrateAnime(old, new, replace = false, flags)` then `onPopScreen()` on UI context.
  5. Only if `canMigrate`: `migrate` ("Migrate") -> `migrateAnime(old, new, replace = true, flags)` then `onPopScreen()`.

Note: buttons 4 and 5 use `scope.launchIO { ... }` and call `onPopScreen` via `withUIContext`. The dialog does not show an error (see 11.3).

### 11.2 Post-migration navigation (OBSERVED)

- From `MigrateAnimeSearchScreen` (`MigrateAnimeSearchScreen.kt:63-71`): if `navigator.lastItem is AnimeScreen` -> `popUntil { items.contains(lastItem) }` then `push(AnimeScreen(new.id))`; else `replace(AnimeScreen(new.id))`.
- From `AnimeSourceSearchScreen` (`AnimeSourceSearchScreen.kt:131-136`): `scope.launch { popUntilRoot(); HomeScreen.openTab(HomeScreen.Tab.Browse()); push(AnimeScreen(new.id)) }`.
- From `MigrateSeasonSelectScreen` (`MigrateSeasonSelectScreen.kt:94-96`): only dismisses the dialog (no navigation).

### 11.3 Engine return path (OBSERVED, important)

`MigrateAnimeDialogScreenModel.migrateAnime(oldAnime, newAnime, replace, flags)`:
1. `migrateFlags.set(flags)` (persists first).
2. `source = sourceManager.get(newAnime.source) ?: return` (silent return if not installed; no toast).
3. `prevSource = sourceManager.get(oldAnime.source)` (nullable).
4. `isMigrating = true`.
5. `try { migrateAnimeInternal(...) } catch (_: Throwable) { isMigrating = false }`.

Because `migrateAnime` swallows every throwable, the caller always proceeds to `onPopScreen()`. So failures (network error during the episode fetch, missing source) are silent: the dialog closes and navigates to the new anime with no error message and no partial-state report (OBSERVED). Proposal: surface the error in the dialog (see T-41a-035).

## 12. Migration: engine (MigrateAnimeDialogScreenModel.migrateAnimeInternal)

OBSERVED order, exactly as in source (`MigrateAnimeDialog.kt:251-363`). Flags decoded via `AnimeMigrationFlags.has*`.

1. `updateAnimeFromRemote.awaitEpisodesUpdate(source = newSource, anime = newAnime, fetchEpisodes = true).getOrThrow()`. Fetches and stores the new anime's episode list first. A failure throws (caught in 11.3).
2. If `migrateEpisodes`:
   - `prevAnimeEpisodes = getEpisodesByAnimeId.await(oldAnime.id)`; `animeEpisodes = getEpisodesByAnimeId.await(newAnime.id)`.
   - `maxEpisodeSeen = prevAnimeEpisodes.filter { seen }.maxOfOrNull { episodeNumber }` (null if none seen).
   - For each new episode:
     - if `isRecognizedNumber`:
       - `prevEpisode = prevAnimeEpisodes.find { isRecognizedNumber && episodeNumber == updated.episodeNumber }`; if found, copy `dateFetch` and `bookmark` from it.
       - if `maxEpisodeSeen != null && updated.episodeNumber <= maxEpisodeSeen` -> `seen = true`.
     - Unrecognized-number episodes are copied unchanged.
   - `updateEpisode.awaitAll(all updated.toEpisodeUpdate())`.
   - Note: `seen` is NOT copied from the matching previous episode; it is derived from the max-seen rule only. `lastSecondSeen` and `fillermark` are not copied (E20) (OBSERVED: only `dateFetch`, `bookmark`, `seen`).
3. If `migrateCategories`: `categoryIds = getCategories.await(oldAnime.id).map { id }`; `setAnimeCategories.await(newAnime.id, categoryIds)` (replaces the new anime's categories).
4. Tracks (always runs, regardless of flags): `getTracks.await(oldAnime.id).mapNotNull { track -> updated = track.copy(animeId = newAnime.id); service = enhancedServices.firstOrNull { it.isTrackFrom(updated, oldAnime, oldSource) }; if service != null then service.migrateTrack(updated, newAnime, newSource) else updated }` then `.takeIf { notEmpty }?.let { insertTrack.awaitAll(it) }`.
   - `enhancedServices = TrackerManager.trackers.filterIsInstance<EnhancedAnimeTracker>()`.
   - `migrateTrack` returns `null` when the new source is not accepted (Jellyfin: `accept(newSource) ? track.copy(remoteUrl = anime.url) : null`) -> that track is DROPPED (not migrated).
   - Non-enhanced trackers keep `track.copy(animeId = newAnime.id)` (the remote id is kept).
   - Observed: the track step ignores the flags entirely (no `hasTracks` flag exists).
5. If `deleteDownloaded` and `oldSource != null`: `downloadManager.deleteAnime(oldAnime, oldSource)`.
6. If `replace`: `updateAnime.awaitUpdateFavorite(oldAnime.id, favorite = false)`. This sets `dateAdded = 0` for the old anime (`UpdateAnime.awaitUpdateFavorite`: `dateAdded = if favorite Instant.now() else 0`).
7. If `migrateCustomCover && oldAnime.hasCustomCover()`: `coverCache.setCustomCoverToCache(newAnime, coverCache.getCustomCoverFile(oldAnime.id).inputStream())`. OBSERVED: the input stream is never closed (leak; Proposal: copy file bytes with `std::fs::copy`).
8. If `migrateCustomBackground && oldAnime.hasCustomBackground()`: same with `backgroundCache`.
9. `updateAnime.await(AnimeUpdate(id = newAnime.id, favorite = true, episodeFlags = oldAnime.episodeFlags, viewerFlags = oldAnime.viewerFlags, dateAdded = if (replace) oldAnime.dateAdded else Instant.now().toEpochMilli()))`.
   - So the new anime becomes a favorite, inherits the old anime's episode-flags and viewer-flags (sort/filter, player settings, area:39/area:05 define these bit layouts), and inherits `dateAdded` only in replace mode.

Observed consequence: the episode fetch (step 1) and track insertion (step 4) happen even when every flag is off. The favorite flip (step 9) happens last, so a failure in steps 1-8 leaves the new anime non-favorite but with episode and track side effects possibly applied for earlier steps, and the old anime still favorite in the copy case. In replace mode, a failure before step 6 leaves both favorites.

Edge: `copy` (replace = false) never unfavorites the old anime; `migrate` (replace = true) unfavorites it.

## 13. Migration: source picker (MigrateAnimeSourceScreen and model)

### 13.1 Model (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/sources/MigrateAnimeSourceScreenModel.kt`)

- `getSourcesWithFavoriteCount.subscribe()` (`app/src/main/java/eu/kanade/domain/source/anime/interactor/GetAnimeSourcesWithFavoriteCount.kt`):
  - `combine(migrationSortingDirection, migrationSortingMode, repository.getAnimeSourcesWithFavoriteCount())`.
  - Filter out `LocalAnimeSource.ID` (0).
  - Sort: `ALPHABETICAL`: stubs (`isStub`) before non-stubs, then `name.lowercase().compareToWithCollator(...)`. `TOTAL`: stubs first, then by favorite count `a.second.compareTo(b.second)` ascending. `DESCENDING` = `Collections.reverseOrder(sortFn)`.
  - Note: with `TOTAL` ascending, the smallest counts come first (the UI default is ALPHABETICAL ascending).
- Failure: `catch` logs and sends `Event.FailedFetchingSourcesWithCount` on a `Channel(Int.MAX_VALUE)`.
- `toggleSortingMode()`: `ALPHABETICAL <-> TOTAL`, keeping direction (`setMigrateSorting.await(newMode, sortingDirection)`).
- `toggleSortingDirection()`: `ASCENDING <-> DESCENDING`, keeping mode.
- State: `isLoading = true` initially; `items: ImmutableList<Pair<AnimeSource, Long>>`; `sortingMode = ALPHABETICAL`; `sortingDirection = ASCENDING`; `isEmpty`.
- Persistence: `pref_migration_sorting` (`SetMigrateSorting.Mode.ALPHABETICAL` default), `pref_migration_direction` (`ASCENDING` default). Shared with the manga migration (both use `SourcePreferences`; see data file).

### 13.2 Screen (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeSourceScreen.kt`)

- `isLoading` -> `LoadingScreen(padding)`; `isEmpty` -> `EmptyScreen(information_empty_library)` ("Your library is empty"; note: this is the library-empty text reused here, OBSERVED).
- List: `ScrollbarLazyColumn(contentPadding + topSmallPaddingValues)`:
  - Sticky header (`STICKY_HEADER_KEY_PREFIX`), background `colorScheme.background`, start padding medium: text `migration_selection_prompt` ("Select a source to migrate from", `header` typography, weight 1); `IconButton` sort-mode (`SortByAlpha` with description `action_sort_alpha` "Alphabetically" when ALPHABETICAL; `Numbers` with `action_sort_count` "Total entries" when TOTAL); `IconButton` direction (`ArrowUpward` `action_asc` "Ascending" when ASCENDING; `ArrowDownward` `action_desc` "Descending").
  - Items keyed `"migrate-${source.id}"`, `Modifier.animateItem()`.
- Item: `BaseAnimeSourceItem(source, showLanguageInContent = source.lang != "", onClickItem = onClickItem(source), onLongClickItem = copy id, icon = AnimeSourceIcon(source), action = BadgeGroup { Badge("$count") })`. Content: name (`ifBlank { id.toString() }`, one line), language (`secondaryItemAlpha`, when present), and `not_installed` ("Not installed", error color) when `source.isStub`.
- Long press copies the source id: `context.copyToClipboard(sourceId, sourceId)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/`).
- Click -> `navigator.push(MigrateAnimeScreen(sourceId))`? Note: the click handler is in part 2 (`MigrateAnimeSourceTab.kt` is part 2; the Voyager tab push goes to MigrateAnimeScreen(sourceId)). The presentation callback is `onClickItem(source)`.

## 14. Migration: favorites list of a source (MigrateAnimeScreen and model)

Model (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/MigrateAnimeScreenModel.kt`):
- `state.source = sourceManager.getOrStub(sourceId)`.
- `getFavorites.subscribe(sourceId)` -> `catch`: log, send `FailedFetchingFavorites`, set `titleList = []`.
- `.map { sortedWith(compareBy(String.CASE_INSENSITIVE_ORDER) { it.title }) }`.
- `isLoading = source == null || titleList == null`, `isEmpty = titles.isEmpty()`.

Screen (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/MigrateAnimeScreen.kt`):
- Title = `state.source!!.name` (the wrapper uses `!!`; Proposal: handle a missing source with the stub name).
- Empty -> `EmptyScreen(empty_screen)`.
- Rows: `BaseAnimeListItem(anime, onClickItem = pick, onClickCover = open anime)`. Click on the row pushes `MigrateAnimeSearchScreen(it.id)`; cover click pushes `AnimeScreen(it.id)`.
- Failure toast: `internal_error`.

## 15. Migration: search (MigrateAnimeSearchScreen, model, dialog state)

Model (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeSearchScreenModel.kt`):
- `init`: `extensionFilter = initialExtensionFilter` (default `""`); launch: `getAnime.await(animeId)!!` -> `state.fromSourceId = anime.source`, `state.searchQuery = anime.title`, then `search()`.
- `getEnabledSources()` override: `super.getEnabledSources().filter { sourceFilter != PinnedOnly || "${id}" in pinnedSources }.sortedWith(compareBy({ id != fromSourceId }, { "${id}" !in pinnedSources }, { "${name.lowercase()} (${lang})" }))`. So the origin source is first, then pinned, then name.
- Inherits `sourceFilter = PinnedOnly` (default) so migrate search starts with pinned sources only (OBSERVED; the user can switch to All in the toolbar).

Dialog model `AnimeMigrateSearchScreenDialogScreenModel` (OBSERVED): `getAnime.await(animeId)!!` into `state.anime`; holds `Dialog.Migrate(anime)` (the candidate).

Screen (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/MigrateAnimeSearchScreen.kt`):
- Results: `MigrateAnimeSearchScreen(state, fromSourceId = dialogState.anime?.source, ...)` (presentation). Click a source header -> push `AnimeSourceSearchScreen(oldAnime, sourceId, state.searchQuery)`. Click a result card -> dialog `Migrate(it)`. Long-click a card -> push `AnimeScreen(it.id, fromSource = true)`.
- Toolbar callbacks: `onChangeSearchQuery = updateSearchQuery`, `onSearch = search()` (the `onSearch` lambda receives a String but the screen calls `search()` ignoring it), `onChangeSearchFilter = setSourceFilter`, `onToggleResults = toggleFilterResults`.

## 16. Migration: per-source search (AnimeSourceSearchScreen)

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/search/AnimeSourceSearchScreen.kt`):
- `BrowseAnimeSourceScreenModel(sourceId, query)` (part 2) provides the paged anime (`animePagerFlowFlow`), `toolbarQuery`, `filters`, `search`, `resetFilters`, `setFilters`, `openFilterSheet`, `setDialog`.
- `SearchToolbar(searchQuery = toolbarQuery ?: "", onChangeSearchQuery = setToolbarQuery, onClickCloseSearch = pop, onSearch = search)`.
- FAB: `AnimatedVisibility(visible = state.filters.isNotEmpty())` with an `ExtendedFloatingActionButton` text `action_filter` and `FilterList` icon -> `openFilterSheet`.
- `BrowseAnimeSourceContent(...)` with `onAnimeClick = openMigrateDialog` (dialog `Migrate(newAnime = it, oldAnime)`), `onAnimeLongClick = push AnimeScreen(it.id, true)`, `onWebViewClick` (source is AnimeHttpSource -> `WebViewScreen(url = getHomeUrl(), initialTitle = name, sourceId = id)`), `onHelpClick` (`Constants.URL_HELP`), `onLocalAnimeSourceHelpClick` (`LocalAnimeSource.HELP_URL`).
- Filter dialog `SourceFilterAnimeDialog(filters, onReset = resetFilters, onFilter = search(filters), onUpdate = setFilters)` (part 2).
- Post-migrate: section 11.2.

## 17. Migration: seasons (MigrateSeasonSelectScreen and model)

Model (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreenModel.kt`):
- `displayMode` = `sourcePreferences.sourceDisplayMode()` (same key as browse).
- `source = sourceManager.getOrStub(anime.source)`.
- `getColumnsPreference(orientation)`: landscape -> `libraryPreferences.animeLandscapeColumns()` (key `pref_animelib_columns_landscape_key`, default 0), portrait -> `animePortraitColumns()` (key `pref_animelib_columns_portrait_key`, default 0). `0` -> `GridCells.Adaptive(128.dp)`, else `GridCells.Fixed(columns)`.
- `hideInLibraryItems = sourcePreferences.hideInAnimeLibraryItems().get()` (key `browse_hide_in_anime_library_items`, default false), captured once.
- `seasonPagerFlowFlow`: `flow { emit(anime) }.map { Pager(PagingConfig(pageSize = 25), pagingSourceFactory = { SeasonListPagingSource { updateAnimeFromRemote.awaitSeasonsUpdate(anime, fetchSeasons = true).getOrThrow().newSeasons.map { toSAnime() } } }).flow.map { pagingData -> pagingData.map { networkToLocalAnime.await(it.toDomainAnime(anime.source)).let { getAnime.subscribe(localAnime.url, localAnime.source) }.filterNotNull().stateIn(ioCoroutineScope) }.filter { !hideInLibraryItems || !it.value.favorite } }.cachedIn(ioCoroutineScope) }.stateIn(ioCoroutineScope, SharingStarted.Lazily, emptyFlow())`.
- `SeasonListPagingSource.load`: the whole season list is loaded in one call; `LoadResult.Page(data, prevKey = null, nextKey = null)`; exception -> `LoadResult.Error(e)`; `getRefreshKey = null`. So `pageSize = 25` is a hint only (all seasons come in one page).
- Dialog: `setDialog(Dialog.Migrate(newAnime, oldAnime))`; `State(dialog)`.

Screen (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/migration/anime/season/MigrateSeasonSelectScreen.kt`):
- Guarded by `ifAnimeSourcesLoaded()`.
- Title = `anime.title` (the anime being migrated INTO).
- Body = `BrowseAnimeSourceContent(source, animeList = seasonPagerFlowFlow, columns = getColumnsPreference(orientation), displayMode, snackbarHostState, contentPadding, ... onAnimeClick = openMigrateDialog, onAnimeLongClick = push AnimeScreen(id, true))`.
- Dialog = `MigrateAnimeDialog(oldAnime, newAnime, ...)` with `onClickSeasons` push `MigrateSeasonSelectScreen(oldAnime, dialog.newAnime)`.

Semantics: `MigrateSeasonSelectScreen(oldAnime, anime)` means "the anime I pick a season of is `anime`"; `anime` is the Seasons-type parent. The seasons come from `updateAnimeFromRemote.awaitSeasonsUpdate` (area:34-ish/area:16), mapped one-to-one to local anime rows (`networkToLocalAnime` creates or fetches each season as a DB anime row).

## 18. Desktop input and state conventions (Kurobayomi mapping, Proposal)

- Pull-to-refresh (extension list) -> `F5` and a toolbar refresh button; `Ctrl+R` also refreshes.
- Long press -> right-click (`GestureClick` button 3) and `Shift+F10` (context menu key). Row click -> single click; secondary action buttons remain visible.
- Search: `Ctrl+F` focuses the search entry; `Esc` clears and returns focus to the list; `/` also opens search on list views.
- Chip and switch rows (filters, languages) use `gtk::Switch` and `gtk::CheckButton`; keyboard `space` toggles.
- Copy-to-clipboard (source id, debug info): `gdk::Display::default().clipboard().set_text(...)`, plus a toast-equivalent `adw`-free `gtk::AlertDialog`-free approach: a transient `gtk::Revealer` banner "Copied" for 2 s (Proposal).

## 19. Edge cases called out in this behavior spec

- E1: Multi-source extension split (1.2): Available rows carry synthetic pkgName `<pkg>-<id>`; uninstall/cancel/update of a split row uses the synthetic name. Keep the same key scheme or map synthetic -> real pkg.
- E2: "Update all" operates on filtered items only (1.7).
- E3: Untrusted rows are not NSFW-filtered (1.2 step 2).
- E4: Install-step key collisions: Available row `installStep` lookup uses the synthetic pkgName; the Installed row uses the real pkgName, so progress shown for a split row never appears on the installed row (intended, the row disappears on install).
- E5: `Installed` transient step shows no buttons (4.4 table note).
- E6: Source list default `Listing.Popular` on row click; `Latest` only when `supportsLatest` (6).
- E7: Local source (`ID = 0L`) cannot be disabled (6), and its help action replaces WebView/Settings (7.1, 7.2).
- E8: Migration silent failure path (11.3).
- E9: `migrate_flags` shared key with manga (section 10 above; p1-data-and-config.md section 9).
- E10: Migration episode `seen` derivation uses max-seen (12 step 2).
- E11: Track migration drops a track when the enhanced tracker rejects the new source (12 step 4).
- E12: Cover/background copy leaks the input stream (12 step 7/8).
- E13: Migrate search defaults to PinnedOnly (15).
- E14: Season migration fetches all seasons in one call; `pageSize 25` is not a real page size (17).
- E15: `AnimeSourcesFilterScreen` header switch hides sources but does not change enabled state (9).
- E16: Extension details `clearCookies` has no UI feedback (4.1).
- E17: `updateAllExtensions` launches concurrent installs (1.7).
- E18: `AnimeExtensionDetailsScreenModel` pops itself when the package disappears from `installedExtensionsFlow` (4.1 step 1), including after an uninstall from another screen.
- E19: Global search subtitle uses `getLocalizedDisplayName` not `getSourceDisplayName` (8.1), so "all" appears as its localized name rather than "Multi" in result subtitles.
- E20: Migration does not copy `episodes.last_second_seen` (resume position) or `fillermark` from the old episode; only `date_fetch`, `bookmark` and the derived `seen` are copied (section 12 step 2). A user who was mid-episode loses the resume point on migrate. Proposal: copy `last_second_seen` too (T-41a-034).

## 20. Dependencies on other areas, behavior-level (summary)

- `AnimeExtensionManager` (area:19): `installedExtensionsFlow`, `availableExtensionsFlow`, `untrustedExtensionsFlow`, `findAvailableExtensions`, `installExtension`, `updateExtension`, `cancelInstallUpdateExtension`, `uninstallExtension`, `trust`, `updateInstallStep`, `setInstalling`.
- `SourcePreferences` (area:02) keys in p1-data-and-config.md section 2.
- `AnimeSourceManager` (area:17): `get`, `getOrStub`, `getAll`.
- `LocaleHelper` (area:13 owns `util/system`).
- `TrackerManager`, `EnhancedAnimeTracker` (area:27).
- `AnimeDownloadManager`, `AnimeDownloadCache` (area:24).
- `AnimeCoverCache`, `AnimeBackgroundCache` (area:34).
- `NetworkHelper.cookieJar` (area:12).
- `WebViewScreen` (area:36).
- `AnimeScreen` (area:39).
- `updateAnimeFromRemote` (`mihon.domain.source.interactor.UpdateAnimeFromRemote`, area:10 / area:16) for `awaitEpisodesUpdate` and `awaitSeasonsUpdate`.

## 21. Behavior of the existing Kurobayomi mockup vs this spec

See p1-ui-parity.md section 25 (gap list) for what `src/entry.rs` does and does not do.

## 22. Kotlin-only behaviors deliberately not replicated

- Compose `animateItem()` and `animateItemFastScroll()`: GTK ListView has no per-item animation; Proposal: no animation, just a `gtk::Revealer` where a row appears.
- `Immutable` / `ImmutableList` / `persistentMapOf`: Rust `Arc<[T]>` and `im` crate (p1-rust-design.md).
- `rememberSaveable` for the FragmentContainer id: not needed.

## 23. Constants table (single list for this part)

| Constant | Value | Source |
|---|---|---|
| SEARCH_DEBOUNCE_MILLIS | 250L | app/src/main/java/eu/kanade/presentation/components/AppBar.kt:65 |
| extension refresh min spinner | 1 s (`delay(1.seconds)`) | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/AnimeExtensionsScreenModel.kt:211 |
| grid spacing | 4 dp | app/src/main/java/eu/kanade/presentation/library/components/CommonEntryItem.kt:50-51 |
| browse favorite cover alpha | 0.34f | CommonEntryItem.kt:54 |
| browse grid padding | 8 dp | app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceComfortableGrid.kt:33 |
| list vertical padding | 8 dp | app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeSourceList.kt:39 |
| compact/cover mode | `LibraryDisplayMode.CoverOnlyGrid` also compact | BrowseAnimeSourceScreen.kt:141 |
| comfortable cover card width (global) | 96 dp | app/src/main/java/eu/kanade/presentation/browse/anime/components/GlobalAnimeSearchCardRow.kt:64 |
| global card title max lines | 3 | GlobalAnimeSearchCardRow.kt:67 |
| extension icon size (list) | 40 dp | AnimeExtensionsScreen.kt:298-306 |
| extension icon size (details) | 112 dp, density XXXHIGH | AnimeExtensionDetailsScreen.kt:267-271 |
| installing icon padding | 8 dp (0 when idle) | AnimeExtensionsScreen.kt:311-314 |
| installing spinner | 40 dp, stroke 2 dp | AnimeExtensionsScreen.kt:305-307 |
| placeholder color (available icon) | `Color(0x1F888888)` | BrowseAnimeIcons.kt:93 |
| migrate season grid | adaptive 128 dp when columns preference 0 | MigrateSeasonSelectScreenModel.kt:60 |
| migrate season page size hint | 25 | MigrateSeasonSelectScreenModel.kt:67 |
| migrate dialog overlay alpha | 0.7 | MigrateAnimeDialog.kt:88 |
| migrate error icon size | `IndicatorSize` = 26 dp | MigrateAnimeDialog.kt:122 |
| migrate default bitmap | `Int.MAX_VALUE` | MigrateAnimeDialog.kt:216 |
| source search pool | 5 threads | app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/source/globalsearch/AnimeSearchScreenModel.kt:60 (part 2) |
| loading row padding | 16 dp vertical | app/src/main/java/eu/kanade/presentation/browse/BrowseSourceLoadingItem.kt:17 |
| secondary alpha | 0.78 | presentation-core Constants.kt:10 |
| URL_HELP | `https://aniyomi.org/docs/guides/troubleshooting/` | core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4 |
| LocalAnimeSource HELP_URL | `https://aniyomi.org/help/guides/local-anime/` | source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:366 |
| raw github regex | `https://raw.githubusercontent.com/(.+?)/(.+?)/.+` | AnimeExtensionDetailsScreen.kt:81 |
| extension pkg prefix stripped in header | `eu.kanade.tachiyomi.animeextension.` | AnimeExtensionDetailsScreen.kt:281 |
| debug clipboard label | `Extension Debug information` | AnimeExtensionDetailsScreen.kt:263 |

## 24. Strings used in this part (English base values)

Authority: `i18n/src/commonMain/moko-resources/base/strings.xml` (MR) and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (AYMR). Every key below is used in the scope files. Where the grep of a key is ambiguous (multiple keys share text), the key name is the identifier to port.

| Key | English | Used in |
|---|---|---|
| AYMR.label_anime_extensions | Anime Extensions | AnimeExtensionsTab.kt |
| MR.ext_updates_pending | Updates pending | AnimeExtensionsScreenModel.kt |
| MR.ext_installed | Installed | AnimeExtensionsScreenModel.kt |
| MR.ext_update_all | Update all | AnimeExtensionsScreen.kt |
| MR.ext_permission_install_apps_warning | Permissions are needed to install extensions. Tap here to grant. | AnimeExtensionsScreen.kt |
| MR.ext_untrusted | Untrusted | AnimeExtensionsScreen.kt |
| MR.ext_obsolete | Orphaned | AnimeExtensionsScreen.kt |
| MR.ext_nsfw_short | 18+ | AnimeExtensionsScreen.kt, AnimeExtensionDetailsScreen.kt |
| MR.ext_pending | Pending | AnimeExtensionsScreen.kt |
| MR.ext_downloading | Downloading | AnimeExtensionsScreen.kt |
| MR.ext_installing | Installing | AnimeExtensionsScreen.kt |
| MR.ext_trust | Trust | AnimeExtensionsScreen.kt, shared dialog |
| MR.ext_update | Update | AnimeExtensionsScreen.kt |
| MR.ext_install | Install | AnimeExtensionsScreen.kt |
| MR.ext_nsfw_warning | Sources from this extension may contain NSFW (18+) content | AnimeExtensionDetailsScreen.kt (shared dialog) |
| MR.action_ok | OK | shared dialog |
| MR.ext_info_age_rating | Age rating | AnimeExtensionDetailsScreen.kt |
| MR.ext_uninstall | Uninstall | AnimeExtensionDetailsScreen.kt, shared dialog |
| MR.pref_clear_cookies | Clear cookies | AnimeExtensionDetailsScreen.kt |
| MR.action_enable_all | Enable all | AnimeExtensionDetailsScreen.kt |
| MR.action_disable_all | Disable all | AnimeExtensionDetailsScreen.kt |
| MR.action_open_repo | Open source repo | AnimeExtensionDetailsScreen.kt |
| MR.action_display_mode | Display mode | BrowseAnimeSourceToolbar.kt |
| MR.action_display_comfortable_grid | Comfortable grid | BrowseAnimeSourceToolbar.kt |
| MR.action_display_grid | Compact grid | BrowseAnimeSourceToolbar.kt |
| MR.action_display_list | List | BrowseAnimeSourceToolbar.kt |
| MR.action_settings | Settings | BrowseAnimeSourceToolbar.kt, AnimeExtensionsScreen.kt |
| MR.action_open_in_web_view | Open in WebView | several |
| MR.action_cancel | Cancel | AnimeExtensionsScreen.kt, AnimeExtensionsTab.kt |
| MR.action_pin | Pin | AnimeSourcesScreen.kt |
| MR.action_unpin | Unpin | AnimeSourcesScreen.kt |
| MR.action_disable | Disable | AnimeSourcesScreen.kt |
| MR.label_settings | Settings | AnimeExtensionDetailsScreen.kt |
| MR.latest | Latest | AnimeSourcesScreen.kt |
| MR.no_results_found | No results found | AnimeExtensionsScreen.kt, GlobalAnimeSearchCardRow.kt |
| MR.empty_screen | Well, this is awkward | several |
| MR.extensionStores | Extension stores | AnimeExtensionsTab.kt, AnimeExtensionsScreen.kt |
| MR.ext_remove | Remove | AnimeExtensionsTab.kt |
| MR.ext_confirm_remove | Remove Extension? | AnimeExtensionsTab.kt |
| MR.remove_private_extension_message | Do you really want to remove "%s" extension? | AnimeExtensionsTab.kt |
| MR.untrusted_extension | Untrusted extension | shared dialog |
| MR.untrusted_extension_message | Malicious extensions can read any stored login credentials or execute arbitrary code. By trusting this extension, you accept these risks. | shared dialog |
| MR.obsolete_extension_message | This extension is not associated with any store. It will not receive updates, may not function properly, and can cause issues with the app. Uninstalling it is recommended. | AnimeExtensionDetailsScreen.kt |
| MR.label_extension_info | Extension info | AnimeExtensionDetailsScreen.kt |
| MR.ext_info_version | Version | AnimeExtensionDetailsScreen.kt |
| MR.ext_info_language | Language | AnimeExtensionDetailsScreen.kt |
| MR.ext_app_info | App info | AnimeExtensionDetailsScreen.kt (Dropped) |
| MR.pref_incognito_mode | Incognito mode | AnimeExtensionDetailsScreen.kt |
| MR.pref_incognito_mode_extension_summary | Pause reading history for extension | AnimeExtensionDetailsScreen.kt |
| MR.label_extensions | Extensions | AnimeExtensionFilterScreen.kt |
| MR.label_sources | Sources | AnimeSourcesFilterScreen.kt |
| MR.source_filter_empty_screen | No installed source found | AnimeSourcesFilterScreen.kt |
| MR.source_empty_screen | No source found | AnimeSourcesScreen.kt |
| MR.has_results | Has results | GlobalAnimeSearchToolbar.kt |
| MR.all | All | GlobalAnimeSearchToolbar.kt |
| MR.pinned_sources | Pinned | GlobalAnimeSearchToolbar.kt |
| MR.internal_error | InternalError: Check crash logs for further information | AnimeExtensionFilterScreen.kt, MigrateAnimeScreen.kt |
| MR.action_retry | Retry | BrowseAnimeSourceScreen.kt |
| MR.local_source_help_guide | Local source guide | BrowseAnimeSourceScreen.kt |
| MR.label_help | Help | BrowseAnimeSourceScreen.kt, BrowseAnimeSourceToolbar.kt |
| MR.source_not_installed | Source not installed: %1$s | BrowseAnimeSourceScreen.kt |
| MR.migration_selection_prompt | Select a source to migrate from | MigrateAnimeSourceScreen.kt |
| MR.action_sort_alpha | Alphabetically | MigrateAnimeSourceScreen.kt |
| MR.action_sort_count | Total entries | MigrateAnimeSourceScreen.kt |
| MR.action_asc | Ascending | MigrateAnimeSourceScreen.kt |
| MR.action_desc | Descending | MigrateAnimeSourceScreen.kt |
| MR.not_installed | Not installed | MigrateAnimeSourceScreen.kt |
| AYMR.information_empty_library (MR) | Your library is empty | MigrateAnimeSourceScreen.kt (reused) |
| MR.migration_dialog_what_to_include | Select data to include | MigrateAnimeDialog.kt |
| AYMR.label_cant_migrate_season | Unable to migrate season to episode | MigrateAnimeDialog.kt |
| AYMR.label_cant_migrate_episode | Unable to migrate episode to season, select season to continue | MigrateAnimeDialog.kt |
| AYMR.action_show_anime | Show anime | MigrateAnimeDialog.kt |
| AYMR.label_show_seasons | Show seasons | MigrateAnimeDialog.kt |
| MR.copy | Copy | MigrateAnimeDialog.kt |
| MR.migrate | Migrate | MigrateAnimeDialog.kt |
| AYMR.episodes | Episodes | AnimeMigrationFlags.kt |
| MR.categories | Categories | AnimeMigrationFlags.kt |
| AYMR.custom_background | Custom background | AnimeMigrationFlags.kt |
| MR.custom_cover | Custom cover | AnimeMigrationFlags.kt |
| MR.delete_downloaded | Delete downloaded | AnimeMigrationFlags.kt |
| MR.extension_api_error | Failed to fetch available extensions | AnimeExtensionManager.kt (area:19) |
| MR.last_used_source | Last used | LocaleHelper.kt |
| MR.other_source | Other | LocaleHelper.kt |
| MR.multi_lang | Multi | LocaleHelper.kt |

All values above were read from the base `strings.xml` files. Kurobayomi's string table must be generated from those files (task T-41a-045 includes a diff check against them).

## 25. Open questions to the user (non-blocking; defaults chosen)

- Q1: Keep the shared `pref_display_mode_catalogue` between manga and anime browse? Default: yes (parity).
- Q2: Keep the "Delete downloaded" pre-checked on migrate? Default: keep pre-checked (parity), but Proposal is to default it OFF because it is destructive.

## 26. Drops and Linux replacements (every Android-only behavior in this part)

| # | Android behavior (source) | Decision | Linux replacement | What the user loses |
|---|---|---|---|---|
| D1 | App-info button opens `Settings.ACTION_APPLICATION_DETAILS_SETTINGS` for shared extensions (AnimeExtensionDetailsScreen.kt:183-189) | Replace (T-41a-024) | "Open extension location": `gio open <extension dir>` via `gtk::FileLauncher` on the extension's directory (Kurobayomi extension dir, p1-data-and-config.md section 1) | Android permission/storage page is gone; the user gets the folder instead |
| D2 | "Install unknown apps" permission banner and `rememberRequestPackageInstallsPermissionState` / `launchRequestPackageInstallsPermission` (AnimeExtensionsScreen.kt:156-170) | Drop (T-41a-025) | none; no install permission exists on Linux | No warning when the installer needs a system permission |
| D3 | `context.isPackageInstalled(pkgName)` decides direct uninstall vs confirm for long-press (AnimeExtensionsTab.kt:67-71) | Replace (T-41a-011): Kurobayomi checks the extension's own directory instead of PackageManager | `Path::exists()` on the extension install dir | none visible; behavior kept: installed -> direct uninstall, not-on-disk -> confirm |
| D4 | `AnimeSourcePreferencesScreen` hosts an AndroidX `PreferenceFragmentCompat` in a `FragmentContainerView` through `AndroidView` and calls `FragmentManager.onContainerAvailable` by reflection (AnimeSourcePreferencesScreen.kt:93-118, 110-118) | Drop the host; rebuild as native GTK (T-41a-023, T-41a-046) | `gtk::Box` of rows built from the source's declared preference spec | The exact AndroidX preference widgets (dialog styles, summary provider behaviors) are re-skinned |
| D5 | `ContextThemeWrapper` with `R.attr.preferenceTheme` (AnimeSourcePreferencesScreen.kt:121-128) | Drop | GTK theme (user's KDE/GTK theme) | none |
| D6 | `setIncognito(lifecycleScope)` on `EditTextPreference` (incognito IME flag, AnimeSourcePreferencesScreen.kt:151-157) | Replace | `gtk::Entry` with `InputHints::NO_SPELLCHECK \| InputHints::PRIVATE`; no guarantee the IME honors it | Keyboard may still learn words on some IMEs |
| D7 | Coil `AsyncImage` with `ColorPainter(Color(0x1F888888))` placeholder and `R.drawable.cover_error` fallback for Available icons (BrowseAnimeIcons.kt:90-97) | Replace with the cover pipeline (area:34) | `reqwest` fetch + on-disk icon cache; placeholder = a 40px grey rounded `gtk::Image` fill `rgba(136,136,136,0.12)` (0x1F888888 = alpha 31/255 = 0.122) | none |
| D8 | `PackageManager.getResourcesForApplication` + `getDrawableForDensity(appInfo.icon, density)` for Installed icons (BrowseAnimeIcons.kt:124-145) | Replace (T-41a-004): read the icon file the extension bundle declares, decode with `image` crate, cache | `zip` crate for the package archive, `image` crate decode, `icon_cache` | Density selection: Kurobayomi uses a single size and scales (no per-density variants) |
| D9 | `Modifier.animateItem()` / `animateItemFastScroll()` (many files) | Drop | none (GTK ListView does not animate rows) | Smooth row reorder animation |
| D10 | `FastScrollLazyColumn` fast-scroller (multiple) | Replace | `gtk::ScrolledWindow` with overlay scrolling (GTK default) | Compose fast-scroll thumb with letter preview |
| D11 | `PullRefresh` pull-to-refresh gesture (AnimeExtensionsScreen.kt:96-100) | Replace | refresh button + F5 (p1-ui-parity.md) | Gesture; keyboard/button replaces it |
| D12 | `copyToClipboard(label, text)` Android `ClipboardManager` (AnimeSourcesMigrate copy id; AnimeExtensionDetailsScreen.kt:263) | Replace | `gdk::Display::default().clipboard().set_text(...)` | none |
| D13 | `Intent` / `Uri` / `UriHandler.openUri` for repo and help links | Replace | `gio::AppInfo::launch_default_for_uri` or `gtk::UriLauncher` | none |
| D14 | `LocalUriHandler`, `LocalContext`, `LocalConfiguration.orientation` (MigrateSeasonSelect, BrowseAnimeSource) | Replace | window width breakpoint (p1-rust-design.md section 10) | Orientation-specific column prefs become width-specific (see T-41a-040) |
| D15 | `DisplayMetrics.DENSITY_XXXHIGH` for icon density (AnimeExtensionDetailsScreen.kt:271) | Drop | single size | none |
| D16 | `isPackageInstalled` for `Available` (AnimeExtensionsTab.kt:67) | not applicable | n/a | n/a |
| D17 | Android `Toast` (`context.toast(MR.strings.internal_error)`, `extension_api_error`) | Replace | `adw`-free in-app banner (`gtk::Revealer` with a label, 4 s) | none |
| D18 | `FragmentActivity` / `supportFragmentManager` | Drop | none | none |

Additional Kotlin-only items: `kotlinx.collections.immutable` (Rust: `Arc<[T]>` / `im::Vector`), `Immutable` annotation (no-op), Voyager `Screen`/`ScreenModel` lifecycle (Rust: explicit `Rc<RefCell<Model>>` owned by the page, p1-rust-design.md section 4).

Nothing in this part is dropped without a replacement except D2 (no equivalent) and D15, D18 (no user-visible effect). D2 loss: on Linux there is no "install unknown apps" concept; the warning is removed.
