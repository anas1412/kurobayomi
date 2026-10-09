# Area 18 — Manga extensions: behaviour spec

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Section headings marked OBSERVED describe the Kotlin code. Text marked **Proposal:** describes what Kurobayomi should do. Constants are quoted exactly. Line numbers are `path:line` of the cited declaration.

Abbreviations used below:
- **M** = `app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt`
- **L** = `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt`
- **I** = `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstaller.kt`
- **A** = `app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt`
- **IQ** = `app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/InstallerManga.kt`

---

## 1. Model and lifecycle (OBSERVED)

1.1 `MangaExtension` is a sealed class with three variants (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt:7-74`):
- `Installed` (fields in `p1-data-and-config.md` section 1).
- `Available` (includes `apkName`, `iconUrl`, `repoUrl`, and `sources: List<Available.MangaSource>`).
- `Untrusted` (name, pkgName, versionName, versionCode, libVersion, `signatureHash`, lang = null, isNsfw = false).

1.2 `MangaLoadResult` is `Success(extension: Installed)`, `Untrusted(extension: Untrusted)`, or `Error` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaLoadResult.kt:3-7`).

1.3 `InstallStep` is an enum with values in this order: `Idle, Pending, Downloading, Installing, Installed, Error`. `isCompleted()` returns true for `Installed`, `Error`, `Idle`. `Installing` is not completed (`app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt:3-15`).

1.4 `MangaExtensionManager` is constructed once (Injekt singleton). Constructor order (M:42-46): `context`, `preferences: SourcePreferences`, `trustExtension: TrustMangaExtension`.

1.5 Initialisation order inside the class body (matters for Rust port, it is a quirk):
1. Property initialisers run in declaration order. The `init` block (M:74-77) runs **before** `subLanguagesEnabledOnFirstRun` (M:79) is initialised. `init` calls `initExtensions()` and registers the install broadcast receiver. `initExtensions` does not read the sub-language flag, so this is harmless today.
2. `init` order: `initExtensions()` first, then `MangaExtensionInstallReceiver(InstallationListener()).register(context)`.

---

## 2. MangaExtensionManager state and operations (OBSERVED, M)

### 2.1 State holders (M:48-72)
- `scope = CoroutineScope(SupervisorJob())` (M:48). No dispatcher: default dispatcher.
- `_isInitialized: MutableStateFlow<Boolean>(false)`, exposed as `isInitialized` (M:50-51).
- `api = MangaExtensionApi()` (M:56).
- `installer` lazy `MangaExtensionInstaller(context)` (M:61).
- `iconMap: MutableMap<String, Drawable>` (M:63), not thread-safe.
- `installedExtensionsMapFlow: MutableStateFlow<Map<String, Installed>>` keyed by `pkgName` (M:65); `installedExtensionsFlow` = that map mapped to `values.toList()` with `stateIn(scope, SharingStarted.Lazily, initial)` (M:385-391).
- `availableExtensionsMapFlow: MutableStateFlow<Map<String, Available>>` (M:68-69).
- `untrustedExtensionsMapFlow: MutableStateFlow<Map<String, Untrusted>>` (M:71-72).

### 2.2 initExtensions (M:125-137)
1. `extensions = MangaExtensionLoader.loadMangaExtensions(context)` (see section 3).
2. `installedExtensionsMapFlow.value = Success results .associate { pkgName to extension }`.
3. `untrustedExtensionsMapFlow.value = Untrusted results .associate { pkgName to extension }`.
4. `Error` results are dropped with no state change.
5. `_isInitialized.value = true`.

### 2.3 findAvailableExtensions (M:142-156)
1. `extensions = api.findExtensions()` inside try. On any `Exception`: `logcat(ERROR)`, `withUIContext { context.toast(MR.strings.extension_api_error) }`, `extensions = emptyList()`.
2. `enableAdditionalSubLanguages(extensions)` (section 2.6).
3. `availableExtensionsMapFlow.value = extensions.associateBy { pkgName }`. **Last entry wins** if two repos publish the same `pkg`.
4. `updatedInstalledExtensionsStatuses(extensions)` (section 6).
5. `setupAvailableExtensionsSourcesDataMap(extensions)` (section 2.5).

Consequence: an empty result replaces the available map with an empty map. A repo-level failure never reaches this try block (see section 4.2), so the toast only shows for an error that escapes `findExtensions` (for example a database error reading the repo list).

### 2.4 Lookup helpers (M:81-109)
- `getExtensionPackage(sourceId: Long): String?` (M:81-86): first installed extension where any `sources[i].id == sourceId`, returns its `pkgName`.
- `getExtensionPackageAsFlow(sourceId)` (M:88-95): same, as a flow over `installedExtensionsFlow`.
- `getAppIconForSource(sourceId): Drawable?` (M:97-109): finds the package as above, returns `iconMap[pkg]` or loads and caches `MangaExtensionLoader.getMangaExtensionPackageInfoFromPkgName(context, pkg)!!.applicationInfo!!.loadIcon(packageManager)`. **OBSERVED defect:** the `!!` throws `NullPointerException` if the package vanished after the map was built.

### 2.5 Stub source map (M:111-120)
- `availableExtensionsSourcesData: Map<Long, StubMangaSource>` (M:111).
- `setupAvailableExtensionsSourcesDataMap(extensions)` (M:113-118): if `extensions` is empty, **return without changing the map** (the previous map is kept). Otherwise `extensions.flatMap { ext -> ext.sources.map { it.toStubSource() } }.associateBy { it.id }`.
- `getSourceData(id)` returns the stub or null. `StubMangaSource(id, lang, name)` is built by `MangaExtension.Available.MangaSource.toStubSource()` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt:54-60`).

### 2.6 Sub-language first-run enablement (M:158-186)
Trigger: runs inside `findAvailableExtensions`.
1. Exit early if `subLanguagesEnabledOnFirstRun` is true or `extensions` is empty. The flag is initialised to `preferences.enabledLanguages().isSet()` (M:79), so it is true once the user has ever saved the setting.
2. `availableLanguages = extensions.flatMap(Available::sources).distinctBy(MangaSource::lang).map(lang)`. Note: this reads **source** languages only. Extensions without sources are ignored here.
3. `deviceLanguage = Locale.getDefault().language` (two-letter ISO 639 code, e.g. `zh`).
4. `defaultLanguages = preferences.enabledLanguages().defaultValue()` = `setOf("all", "en", Locale.getDefault().language)` (see `p1-data-and-config.md` section 3).
5. `languagesToEnable = availableLanguages.filter { it != deviceLanguage && it.startsWith(deviceLanguage) }`.
6. `preferences.enabledLanguages().set(defaultLanguages + languagesToEnable)`; then `subLanguagesEnabledOnFirstRun = true`.

Example: device language `zh`, available source languages `zh-Hans`, `zh-Hant`, `en`, `ja`. Result: `{all, en, zh, zh-Hans, zh-Hant}`.

### 2.7 Status reconciliation (M:193-229)
Described in section 6.

### 2.8 Install and update entry points (M:238-278)
- `installExtension(extension: Available): Flow<InstallStep>` = `installer.downloadAndInstall(api.getApkUrl(extension), extension)` (M:238-240).
- `updateExtension(extension: Installed): Flow<InstallStep>` (M:249-252): `availableExt = availableExtensionsMapFlow.value[extension.pkgName] ?: return emptyFlow()`; then `installExtension(availableExt)`. **Observed:** if no available entry exists, the flow is empty and the caller gets no step at all.
- `cancelInstallUpdateExtension(extension)` = `installer.cancelInstall(extension.pkgName)` (M:254-256).
- `setInstalling(downloadId)` = `installer.updateInstallStep(downloadId, InstallStep.Installing)` (M:263-265).
- `updateInstallStep(downloadId, step)` (M:267-269).
- `uninstallExtension(extension)` = `installer.uninstallApk(extension.pkgName)` (M:276-278).

### 2.9 Trust (M:286-296)
`trust(extension: Untrusted)`:
1. If `untrustedExtensionsMapFlow.value[pkgName]` is null: return (no-op).
2. `trustExtension.trust(pkgName, versionCode, signatureHash)` (section 10).
3. `untrustedExtensionsMapFlow.value -= pkgName`.
4. `MangaExtensionLoader.loadMangaExtensionFromPkgName(context, pkgName)`; if the result is `Success`, `registerNewExtension(extension)`.

Note: if the reload returns `Untrusted` again (for example the signature changed), nothing is registered and the package stays out of both maps.

### 2.10 Registration (M:303-326)
- `registerNewExtension(ext)` = `installedExtensionsMapFlow.value += ext` (keyed by `pkgName`, replace).
- `registerUpdatedExtension(ext)` = the same operation (M:313-315).
- `unregisterExtension(pkgName)` removes the key from **both** installed and untrusted maps (M:323-326).

### 2.11 InstallationListener (M:331-354)
Receives events from `MangaExtensionInstallReceiver` (section 5.4).
- `onExtensionInstalled(ext)`: `registerNewExtension(ext.withUpdateCheck())`; `updatePendingUpdatesCount()`.
- `onExtensionUpdated(ext)`: `registerUpdatedExtension(ext.withUpdateCheck())`; `updatePendingUpdatesCount()`.
- `onExtensionUntrusted(ext)`: `installedExtensionsMapFlow.value -= pkgName`; `untrustedExtensionsMapFlow.value += ext`; `updatePendingUpdatesCount()`.
- `onPackageUninstalled(pkgName)`: `MangaExtensionLoader.uninstallPrivateExtension(context, pkgName)`; `unregisterExtension(pkgName)`; `updatePendingUpdatesCount()`. This fires for **every** package removed from the device, not only extensions. For non-extensions the calls are no-ops.

### 2.12 Update flags (M:359-383)
- `withUpdateCheck()`: if `updateExists()` then `copy(hasUpdate = true)` else `this`. It can only set `true`.
- `updateExists(availableExtension: Available? = null): Boolean` (M:367-375): `availableExt = param ?: availableExtensionsMapFlow.value[pkgName] ?: return false`; returns `availableExt.versionCode > versionCode || availableExt.libVersion > libVersion`.
- `updatePendingUpdatesCount()` (M:377-383): `count = installedExtensionsMapFlow.value.values.count { it.hasUpdate }` (obsolete items are counted if they have `hasUpdate`); `preferences.mangaExtensionUpdatesCount().set(count)`; if `count == 0`, `ExtensionUpdateNotifier(context).dismiss()`.

---

## 3. Loader: discovery, selection, validation, load (OBSERVED, L)

### 3.1 Constants (L:52-65)
- `EXTENSION_FEATURE = "tachiyomi.extension"`
- `METADATA_SOURCE_CLASS = "tachiyomi.extension.class"`
- `METADATA_SOURCE_FACTORY = "tachiyomi.extension.factory"`
- `METADATA_NSFW = "tachiyomi.extension.nsfw"`
- `LIB_VERSION_MIN = 1.4`, `LIB_VERSION_MAX = 1.5` (public, Double)
- `PACKAGE_FLAGS = GET_CONFIGURATIONS or GET_META_DATA or GET_SIGNATURES or (SDK >= P ? GET_SIGNING_CERTIFICATES : 0)`
- `PRIVATE_EXTENSION_EXTENSION = "ext"`
- private dir = `File(context.filesDir, "exts")` (L:67)
- `loadNsfwSource` = `by lazy { preferences.showNsfwSource().get() }` (L:48-50). **OBSERVED:** evaluated once per process, at the first load. Toggling the setting at runtime does not change loader behaviour until restart.

### 3.2 Two extension locations (L:24-38)
- **Shared**: installed in the system package manager, so other Tachiyomi/Aniyomi variants can use it. Kurobayomi has no equivalent (section 14, drop).
- **Private**: a copy stored in the app's private data directory and only usable by the running app.

Duplicate rule (L:39-41): when both exist for one package name, the shared one is used unless version codes differ; the higher version code wins.

### 3.3 Private install file (L:69-118) — `installPrivateExtensionFile(context, file)`
Ordered steps:
1. `extension = packageManager.getPackageArchiveInfo(file.absolutePath, PACKAGE_FLAGS)?.takeIf(isPackageAnExtension) ?: return false`.
2. `currentExtension = getMangaExtensionPackageInfoFromPkgName(context, extension.packageName)` (the best of shared or private, section 3.6).
3. If `currentExtension != null`:
   a. If `longVersionCode(extension) < longVersionCode(currentExtension)`: log `"Installed extension version is higher. Downgrading is not allowed."`; return false. Equal version codes are allowed (reinstall).
   b. `extensionSignatures = getSignatures(extension)`; if null or empty: log `"Extension to be installed is not signed."`; return false.
   c. If `!extensionSignatures.containsAll(getSignatures(currentExtension)!!)`: log `"Installed extension signature is not matched."`; return false. The new package must contain every signing digest of the installed one.
4. `target = File(privateDir, "${extension.packageName}.ext")`.
5. Inside try: `target.delete()`; `file.copyAndSetReadOnlyTo(target, overwrite = true)`; then if `currentExtension != null` → `MangaExtensionInstallReceiver.notifyReplaced(context, pkg)`, else → `notifyAdded(context, pkg)`; return true.
6. On `Exception`: log `"Failed to copy extension file."`; `target.delete()`; return false.

Observed: step 5 uses the broadcast to tell the manager. The manager then reloads the package asynchronously (section 5.4). The install step is set to `Installed` by the caller, not by this function (section 7.6).

### 3.4 Uninstall private (L:120-122)
`uninstallPrivateExtension(context, pkgName)` deletes `"$pkgName.ext"` in the private dir. Return value ignored.

### 3.5 Discovery: `loadMangaExtensions(context): List<MangaLoadResult>` (L:129-183)
1. `installedPkgs = getInstalledPackages(PackageInfoFlags.of(PACKAGE_FLAGS))` on API 33+ (`TIRAMISU`), else `getInstalledPackages(PACKAGE_FLAGS)`.
2. `sharedExtPkgs = installedPkgs.filter(isPackageAnExtension).map { Info(pkg, isShared = true) }` (lazy sequence).
3. `privateExtPkgs = privateDir.listFiles()` filtered `isFile && extension == "ext"`, then for each file: if `canWrite()` → `setReadOnly()`; `getPackageArchiveInfo(path, PACKAGE_FLAGS)` then `applicationInfo!!.fixBasePaths(path)`; mapNotNull (null on parse failure is dropped); then `filter(isPackageAnExtension)`; then `map { Info(pkg, isShared = false) }`. If the directory listing is null, the sequence is empty.
4. `extPkgs = (sharedExtPkgs + privateExtPkgs).distinctBy { packageName }` (first occurrence wins, so shared beats private when both appear). Then for each remaining entry `sharedPkg`: `privatePkg = privateExtPkgs.singleOrNull { same packageName }`; `selectExtensionPackage(sharedPkg, privatePkg)`. Null results dropped. Materialise to list.
5. If empty: return empty list.
6. `runBlocking { extPkgs.map { async { loadMangaExtension(context, it) } }.awaitAll() }`. Results are in `extPkgs` order.

**OBSERVED:** `privateExtPkgs` is a lazy sequence. Each traversal re-lists the directory, re-parses every `.ext` file and calls `setReadOnly()` again. It is traversed at least twice (steps 4 and the `singleOrNull`).

**OBSERVED:** `runBlocking` blocks the calling thread for the whole batch. There is no concurrency cap; one coroutine per installed extension.

### 3.6 Select between shared and private (L:377-391) — `selectExtensionPackage(shared?, private?)`
- Only shared → shared. Only private → private. Neither → null.
- Both: if `longVersionCode(shared) >= longVersionCode(private)` → shared, else private.

### 3.7 `loadMangaExtensionFromPkgName(context, pkgName): MangaLoadResult` (L:189-196)
1. `info = getMangaExtensionInfoFromPkgName(context, pkgName)`; if null: log `"Extension package is not found ($pkgName)"`; return `Error`.
2. Otherwise `loadMangaExtension(context, info)`.

`getMangaExtensionInfoFromPkgName` (L:202-238): private = `<pkg>.ext` if `isFile`, parsed with `getPackageArchiveInfo` + `isPackageAnExtension` + `fixBasePaths` (isShared false). Shared = `getPackageInfo(pkg, PACKAGE_FLAGS)` filtered by `isPackageAnExtension` (isShared true), with `NameNotFoundException` → null. Result = `selectExtensionPackage(shared, private)`.

### 3.8 `loadMangaExtension(context, info)` — ordered checks (L:246-369)

Each step returns on failure. Order matters because an earlier check hides later ones.

1. `extName = pm.getApplicationLabel(appInfo).toString().substringAfter("Tachiyomi: ")`. `substringAfter` returns the whole label when the prefix is missing.
2. `versionName = pkgInfo.versionName`; `versionCode = PackageInfoCompat.getLongVersionCode(pkgInfo)`.
3. If `versionName` is null or empty: log `"Missing versionName for extension $extName"`; return `Error`.
4. Lib version: `libVersion = versionName.substringBeforeLast('.').toDoubleOrNull()`. If null, `< 1.4`, or `> 1.5`: log `"Lib version is $libVersion, while only versions $LIB_VERSION_MIN to $LIB_VERSION_MAX are allowed"`; return `Error`.
   - Worked examples (OBSERVED from the string rule): `"1.4.13"` → `"1.4"` → 1.4 accepted. `"1.5.0"` → 1.5 accepted. `"1.6.0"` → 1.6 rejected. `"1.10.0"` → `"1.10"` → 1.1 rejected (Double compares 1.10 as 1.1). `"1.4"` → `"1"` → 1.0 rejected. `"14"` (no dot) → `"14"` → 14.0 rejected.
5. `signatures = getSignatures(pkgInfo)`. If null or empty: log `"Package $pkgName isn't signed"`; return `Error`.
6. Else if `!trustExtension.isTrusted(pkgInfo, signatures)`: log `"Extension $pkgName isn't trusted"`; return `Untrusted(MangaExtension.Untrusted(extName, pkgName, versionName, versionCode, libVersion, signatures.last()))`.
7. `isNsfw = appInfo.metaData.getInt(METADATA_NSFW) == 1` (missing key = 0 = false). If `!loadNsfwSource && isNsfw`: log `"NSFW extension $pkgName not allowed"`; return `Error`.
   - Order note: an untrusted NSFW extension is reported as `Untrusted`, not `Error`.
8. `classLoader = ChildFirstPathClassLoader(appInfo.sourceDir, null, context.classLoader)`. On exception: log `"Extension load error: $extName ($pkgName)"`; return `Error`.
9. Sources. `sourceClassNames = appInfo.metaData.getString(METADATA_SOURCE_CLASS)!!.split(";")` then per entry: `trim()`; if it starts with `"."` prefix with `pkgInfo.packageName`.
   - **OBSERVED defect:** the `!!` is outside any try. A package with no `tachiyomi.extension.class` metadata throws `NullPointerException` and aborts the whole batch (section 3.5 step 6 rethrows).
   - For each class name, `Class.forName(name, false, classLoader).getDeclaredConstructor().newInstance()`:
     - `MangaSource` → `listOf(obj)`.
     - `SourceFactory` → `obj.createSources()`.
     - Any other type → throw `Exception("Unknown source class type: ...")`.
     - `LinkageError` → retry the same with `PathClassLoader(appInfo.sourceDir, null, context.classLoader)`. Any `Throwable` on the retry → log `"Extension load error: $extName ($it)"`; return `Error`.
     - Any other `Throwable` on the first try → same log; return `Error`.
   - **OBSERVED:** one failing class makes the whole extension `Error`. There is no partial load.
10. `langs = sources.filterIsInstance<CatalogueSource>().map { lang }.toSet()`. `lang = when (langs.size) { 0 -> ""; 1 -> the one; else -> "all" }`.
11. Return `Success(Installed(name = extName, pkgName, versionName, versionCode, libVersion, lang, isNsfw, sources, pkgFactory = metaData.getString(METADATA_SOURCE_FACTORY), icon = appInfo.loadIcon(pm), isShared = info.isShared))`.
    - `hasUpdate`, `isObsolete`, `repoUrl` take their defaults (false, false, null). They are set later by sections 2.12 and 6.

### 3.9 Signing digests (L:408-422) — `getSignatures(pkgInfo): List<String>?`
- API 28+: `signingInfo!!`. If `hasMultipleSigners()`: `apkContentsSigners`; else `signingCertificateHistory`.
- API < 28: `pkgInfo.signatures` (deprecated path).
- Each signature → `Hash.sha256(bytes)` → lowercase hex string of 64 characters (`Hash.kt` alphabet `0-9a-f`).
- Returns null when the input list is null.

### 3.10 `fixBasePaths` (L:428-435)
If `sourceDir` is null, set it to the APK path. Same for `publicSourceDir`. Needed on Android 13+ where archive-parsed `ApplicationInfo` lacks these.

### 3.11 `isPackageAnExtension` (L:398-400)
True when `pkgInfo.reqFeatures` contains a feature whose `name == "tachiyomi.extension"`.

---

## 4. Index fetch and repo API (OBSERVED, A)

### 4.1 Overall (A:42-49)
`findExtensions()`: inside `withIOContext`: `getExtensionRepo.getAll()` (repo table, section 9 of data file) → one `async { getExtensions(repo) }` per repo → `awaitAll()` → `flatten()`. Repos are fetched in parallel. No ordering is applied; output order = repo table order, then index order within each repo.

### 4.2 Per repo (A:51-67) — `getExtensions(extRepo)`
- Request: `GET "$repoBaseUrl/index.min.json"` through `networkService.client.newCall(...).awaitSuccess()`. Non-2xx throws.
- Parse: `parseAs<List<ExtensionJsonObject>>()` (root must be a JSON array).
- Map: `toExtensions(repoBaseUrl)`.
- **Any** `Throwable` → log `"Failed to get extensions from $repoBaseUrl"`; return empty list. The repo is silently empty. There is no per-repo error surfaced to the user.
- The `repoBaseUrl` is used exactly as stored in the table (no trailing-slash normalisation). A stored trailing `/` produces `//index.min.json`.

### 4.3 Wire model (A:144-171)
`ExtensionJsonObject` (`@Serializable`, private):
| Field | Kotlin type | Notes |
|---|---|---|
| `name` | String | Required. Display name with optional `"Tachiyomi: "` prefix. |
| `pkg` | String | Required. Package name. |
| `apk` | String | Required. APK file name, relative to `/apk/`. |
| `lang` | String | Required. |
| `code` | Long | Required. Must be a JSON number. A string fails the whole repo. |
| `version` | String | Required. |
| `nsfw` | Int | Required. Only `1` means NSFW. |
| `sources` | List<ExtensionSourceJsonObject>? | Nullable. Can be absent (`explicitNulls = false`). |

`ExtensionSourceJsonObject`: `id: Long`, `lang: String`, `name: String`, `baseUrl: String`. All required.

Json instance (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`): `ignoreUnknownKeys = true`, `explicitNulls = false`.

**OBSERVED:** a single entry that fails to decode (missing required field, wrong type) makes `parseAs` throw, so the **whole repo** returns no extensions.

### 4.4 Mapping and filter (A:112-133) — `toExtensions(repoUrl)`
1. Filter: `libVersion = extractLibVersion()`; keep when `libVersion >= 1.4 && libVersion <= 1.5` (inclusive, `MangaExtensionLoader.LIB_VERSION_MIN/MAX`).
2. `extractLibVersion()` = `version.substringBeforeLast('.').toDouble()` (A:139-141). **Uses `toDouble()`, not `toDoubleOrNull()`.** When the prefix is not a number (for example `"abc.1"` or `""`), `NumberFormatException` escapes `toExtensions`, so the whole repo is dropped (section 4.2). A version without a dot (for example `"14"`) does not throw: `substringBeforeLast` returns `"14"`, which parses as 14.0 and is then removed by the range filter.
3. Map each to `Available`:
   - `name = it.name.substringAfter("Tachiyomi: ")`
   - `pkgName = it.pkg`
   - `versionName = it.version`
   - `versionCode = it.code`
   - `libVersion = it.extractLibVersion()`
   - `lang = it.lang`
   - `isNsfw = it.nsfw == 1`
   - `sources = it.sources?.map(extensionSourceMapper).orEmpty()`, mapper = `Available.MangaSource(id, lang, name, baseUrl)` (A:164-171)
   - `apkName = it.apk`
   - `iconUrl = "$repoUrl/icon/${it.pkg}.png"`
   - `repoUrl = repoUrl`

### 4.5 URLs
- Index: `"$repoBaseUrl/index.min.json"` (A:55).
- Icon: `"$repoUrl/icon/${pkg}.png"` (A:129).
- APK: `"${extension.repoUrl}/apk/${extension.apkName}"` (A:135-137).
- Repo details (used by refresh, section 9): `"$repo/repo.json"` (domain `ExtensionRepoService.kt`).

---

## 5. Install receiver (OBSERVED, `MangaExtensionInstallReceiver.kt`)

Path: `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallReceiver.kt`.

5.1 Filter (lines 32-40): actions `ACTION_PACKAGE_ADDED`, `ACTION_PACKAGE_REPLACED`, `ACTION_PACKAGE_REMOVED`, plus self actions `"${applicationId}.ACTION_EXTENSION_ADDED"`, `"...ACTION_EXTENSION_REPLACED"`, `"...ACTION_EXTENSION_REMOVED"`. Data scheme `package`. Registered `RECEIVER_NOT_EXPORTED` (line 29).

5.2 `onReceive` (lines 46-79):
- `ADDED` (system or self): if `EXTRA_REPLACING` is true → return. Otherwise launch: `loadMangaExtensionFromPkgName`; `Success` → `onExtensionInstalled`; `Untrusted` → `onExtensionUntrusted`; `Error` → nothing.
- `REPLACED` (system or self): **no** replacing check. Launch the same load. `Success` → `onExtensionUpdated`; `Untrusted` → `onExtensionUntrusted`; else nothing.
- `REMOVED` (system or self): if replacing → return. Otherwise `pkgName = intent.data.encodedSchemeSpecificPart`; if non-null → `onPackageUninstalled(pkgName)`.

5.3 Package name source: `intent.data?.encodedSchemeSpecificPart` (line 109). Missing name → log `"Package name not found"` and `Error` (line 98-100).

5.4 Self broadcasts (companion, lines 127-145): `notifyAdded`, `notifyReplaced`, `notifyRemoved` send an intent with `data = "package:$pkgName"` and `setPackage(context.packageName)`.

5.5 Scope: `scope = CoroutineScope(SupervisorJob())` (line 26). Launches are on the default dispatcher.

---

## 6. Status reconciliation (OBSERVED, M:193-229)

Called from `findAvailableExtensions` with the new available list.

1. If `availableExtensions.isEmpty()`: `preferences.mangaExtensionUpdatesCount().set(0)`; return. **The installed map is not touched.** No obsolete marks, no `hasUpdate` changes.
2. `installed = installedExtensionsMapFlow.value.toMutableMap()`; `changed = false`.
3. For each `(pkgName, extension)` in `installed`:
   - `availableExt = availableExtensions.find { it.pkgName == pkgName }` (**first** match in list order).
   - If `availableExt == null && !extension.isObsolete`: `installed[pkgName] = extension.copy(isObsolete = true)`; `changed = true`.
   - Else if `availableExt == null` (already obsolete): no change.
   - Else (`availableExt != null`): `hasUpdate = extension.updateExists(availableExt)`. If `extension.hasUpdate != hasUpdate`: `copy(hasUpdate = hasUpdate, repoUrl = availableExt.repoUrl)`; else `copy(repoUrl = availableExt.repoUrl)`. `changed = true`.
4. If `changed`: `installedExtensionsMapFlow.value = installed`.
5. `updatePendingUpdatesCount()`.

**OBSERVED defect:** `isObsolete` is never reset to `false`. An extension that disappears from a repo and later reappears stays marked obsolete, and the UI keeps the "ORPHANED" state.

**OBSERVED inconsistency:** this loop uses the first match (`find`), while `availableExtensionsMapFlow` (used by `updateExists()` default) keeps the last match (`associateBy`). With duplicate `pkgName` across repos, the two can disagree.

**Proposal:** pick one rule for duplicate `pkgName` (see section 14 of this file and T-18a-024). Reset `isObsolete` when the package is found again.

---

## 7. Installer (OBSERVED, I) and install queue (OBSERVED, IQ)

### 7.1 Downloads map (I:58-60)
- `activeDownloads: HashMap<String, Long>`: package name → DownloadManager id.
- `downloadsStateFlows: HashMap<Long, MutableStateFlow<InstallStep>>`: id → state.
- `extensionInstaller = Injekt.get<BasePreferences>().extensionInstaller()` (I:62). The preference value is read at each `installApk`.

### 7.2 `downloadAndInstall(url, extension): Flow<InstallStep>` (I:71-120)
1. `pkgName = extension.pkgName`. If `activeDownloads[pkgName]` exists → `deleteDownload(pkgName)` (removes the old DM entry, its state flow, and unregisters the receiver when the map becomes empty).
2. `downloadReceiver.register()` (I:80).
3. `downloadUri = url.toUri()`. Request: `setTitle(extension.name)`, `setMimeType("application/vnd.android.package-archive")`, `setDestinationInExternalFilesDir(context, DIRECTORY_DOWNLOADS, downloadUri.lastPathSegment)`, `setNotificationVisibility(VISIBLE_NOTIFY_COMPLETED)`.
4. `id = downloadManager.enqueue(request)`. `activeDownloads[pkgName] = id`. `downloadsStateFlows[id] = MutableStateFlow(InstallStep.Pending)`.
5. `pollStatusFlow = downloadStatusFlow(id).mapNotNull { STATUS_PENDING → Pending; STATUS_RUNNING → Downloading; else → null }`.
6. Return `merge(downloadStateFlow, pollStatusFlow).transformWhile { emit(it); !it.isCompleted() }.onCompletion { withUIContext { deleteDownload(pkgName) } }`.

### 7.3 Poll (I:128-150)
- Query `DownloadManager.Query().setFilterById(id)`.
- Loop: read `COLUMN_STATUS`. If no row (`!moveToFirst()`): return (the poll flow completes **silently**, without emitting).
- Emit the status (`distinctUntilChanged`). If `STATUS_SUCCESSFUL` or `STATUS_FAILED`: return. Otherwise `delay(1.seconds)` (1000 ms) and repeat.
- Consequence: the poll **never** emits `Error`. A failed download is turned into `Error` only by the broadcast path (section 7.5).

### 7.4 DownloadManager status constants used
`STATUS_PENDING` (1), `STATUS_RUNNING` (2), `STATUS_SUCCESSFUL` (8), `STATUS_FAILED` (16). Paused (4) is mapped to `null` (no emission).

### 7.5 DownloadCompletionReceiver (I:262-320)
- Registered for `ACTION_DOWNLOAD_COMPLETE` with `RECEIVER_EXPORTED` (I:276-277). Unregistered in `unregister()` (I:283-288).
- `onReceive`: `id = intent.getLongExtra(EXTRA_DOWNLOAD_ID, 0)`. If `id !in activeDownloads.values` → return (events for other downloads are ignored).
- `uri = downloadManager.getUriForDownloadedFile(id)`. If null → log `"Couldn't locate downloaded APK"`; `updateInstallStep(id, Error)`; return.
- Query by id. If a row exists: `localUri = COLUMN_LOCAL_URI.removePrefix("file://")`; `installApk(id, File(localUri).getUriCompat(context))`. If no row: nothing happens (silent).
- `getUriCompat` builds a FileProvider content URI with authority `"<packageName>.provider"` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt:16-18`).

### 7.6 installApk(downloadId, uri) (I:158-205) — dispatch by `extension_installer`
Value read from `BasePreferences.ExtensionInstaller` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36`, values in `p1-data-and-config.md` section 4):
- `LEGACY` → start `MangaExtensionInstallActivity` with the uri, the APK MIME type, extra `ExtensionInstaller.extra.DOWNLOAD_ID`, flags `NEW_TASK | GRANT_READ_URI_PERMISSION`. The activity reports the result (section 9.3).
- `PRIVATE` → (a) `tempFile = File(context.cacheDir, "temp_$downloadId")`; if it exists and cannot be deleted: `updateInstallStep(downloadId, Error)`; return. (b) Inside try: copy `contentResolver.openInputStream(uri)` into `tempFile`. If `MangaExtensionLoader.installPrivateExtensionFile(context, tempFile)` → `Installed`, else `Error`. (c) On `Exception`: log `"Failed to read downloaded extension file."`; `Error`. (d) After all branches: `tempFile.delete()`.
- `else` (PACKAGEINSTALLER or SHIZUKU) → `ContextCompat.startForegroundService(context, MangaExtensionInstallService.getIntent(context, downloadId, uri, installer))`.

Observed: the `PRIVATE` path never sets `Installing`. The UI goes from `Downloading` straight to `Installed` or `Error`.

### 7.7 cancelInstall(pkgName) (I:210-214)
1. `downloadId = activeDownloads.remove(pkgName) ?: return`.
2. `downloadManager.remove(downloadId)` (deletes the DM entry and the downloaded file).
3. `InstallerManga.cancelInstallQueue(context, downloadId)` (local broadcast, section 8.4).

**OBSERVED:** nothing sets the step to `Idle` here. `downloadsStateFlows[downloadId]` is not removed. The poll flow completes when the DM row disappears (section 7.3), but the merged flow stays open until the collector is cancelled, and the UI's in-progress marker stays until then. The queue only sets `Idle` when the entry was already in the service queue (section 8.3).

**Proposal:** set the step to `Idle` in `cancelInstall` directly, and remove the state entry.

### 7.8 uninstallApk(pkgName) (I:221-231)
- If `context.isPackageInstalled(pkgName)` (PackageManager lookup succeeds): start `ACTION_UNINSTALL_PACKAGE` for `"package:$pkgName"` with `NEW_TASK`.
- Else: `MangaExtensionLoader.uninstallPrivateExtension(context, pkgName)`; `MangaExtensionInstallReceiver.notifyRemoved(context, pkgName)` (self broadcast; the manager removes the entry in section 2.11).

### 7.9 updateInstallStep(downloadId, step) (I:239-241)
`downloadsStateFlows[downloadId]?.let { it.value = step }`. Silent no-op for unknown ids.

### 7.10 deleteDownload(pkgName) (I:248-257)
`downloadId = activeDownloads.remove(pkgName)`; if non-null: `downloadManager.remove(downloadId)`; `downloadsStateFlows.remove(downloadId)`. If `activeDownloads` is empty: `downloadReceiver.unregister()`.

**Observed side effect:** `DownloadManager.remove` deletes the downloaded APK. It runs when the flow completes or is cancelled (section 7.2 step 6). For the `PRIVATE` path the APK was already copied to the temp file.

### 7.11 Install queue (IQ, for the Android installer services)
Used only by `PACKAGEINSTALLER` and `SHIZUKU` (section 9). Its semantics are ported (T-18a-036).
- `queue: synchronizedList<Entry>` (FIFO). `waitingInstall: AtomicReference<Entry?>` (the one active entry). `ready: Boolean` (abstract; false blocks the queue).
- `addToQueue(downloadId, uri)`: append; `checkQueue()`.
- `checkQueue()`: if not ready → return. If queue empty → `service.stopSelf()`; return. `next = queue.first()`; if `waitingInstall.compareAndSet(null, next)`: `queue.removeAt(0)`; `processEntry(next)`.
- `continueQueue(resultStep)`: `completed = waitingInstall.getAndSet(null)`; if non-null: `updateInstallStep(completed.downloadId, resultStep)`; `checkQueue()`.
- `cancelQueue(downloadId)` (from the local broadcast): `waiting = waitingInstall.get()`; `toCancel = queue.find { it.downloadId == downloadId } ?: waiting ?: return`. If `cancelEntry(toCancel)`: `queue.remove(toCancel)`; if `waiting == toCancel`: `waitingInstall.set(null)` then `checkQueue()`; `updateInstallStep(downloadId, Idle)`. If `cancelEntry` returns false: nothing happens.
- `onDestroy()`: unregister the cancel receiver; set every **queued** entry to `Error`; clear the queue; `waitingInstall.set(null)`. **OBSERVED:** the active entry's step is not updated.
- `Entry` is `data class Entry(downloadId: Long, uri: Uri)`.

### 7.12 Install step values by path (summary table)
| Path | Pending | Downloading | Installing | Installed | Error | Idle |
|---|---|---|---|---|---|---|
| Private (`PRIVATE`) | initial | poll RUNNING | never | after copy and install | download missing, copy error, install failed | never |
| Legacy activity | initial | poll RUNNING | never | activity RESULT_OK | other result, or no uri | activity RESULT_CANCELED |
| PackageInstaller (dropped) | initial | poll RUNNING | processEntry | STATUS_SUCCESS | other statuses, exceptions | STATUS_FAILURE_ABORTED, cancel |
| Shizuku (dropped) | initial | poll RUNNING | processEntry | commit ok | exceptions, failed commands | cancel of a queued entry |

---

## 8. Update check API (OBSERVED, A:69-110)

8.1 `checkForUpdates(context, fromAvailableExtensionList: Boolean = false): List<Installed>?`:
1. **Gate (only when `fromAvailableExtensionList` is true):** if `Instant.now().toEpochMilli() < lastExtCheck.get() + 1.days.inWholeMilliseconds`, return `null`. `lastExtCheck` = `preferenceStore.getLong("last_ext_check", 0)` (raw key, not in `SourcePreferences`). `1.days` = 86,400,000 ms.
   - **OBSERVED:** the gate does not apply when the parameter is false. The `MainActivity` call (section 13) uses the default false, so every app launch performs a full fetch.
2. `updateExtensionRepo.awaitAll()` (refreshes repo details for every repo, section 9).
3. `extensions = if (fromAvailable) extensionManager.availableExtensionsFlow.value else findExtensions().also { lastExtCheck.set(now) }`.
   - `lastExtCheck` is written only in the full-fetch branch.
4. `installedExtensions = MangaExtensionLoader.loadMangaExtensions(context).filterIsInstance<Success>().map { it.extension }`. A **full reload from disk**, not the manager's state.
5. For each installed: `availableExt = extensions.find { pkgName == installed.pkgName } ?: continue` (first match). `hasUpdate = availableExt.versionCode > installed.versionCode || availableExt.libVersion > installed.libVersion`. Collect where true.
6. If the list is non-empty: `ExtensionUpdateNotifier(context).promptUpdates(names)` where `names = list.map { it.name }` (installed names).
7. Return the list (possibly empty; never null unless gated).

8.2 **OBSERVED:** `checkForUpdates` does not write `ext_updates_count` and does not touch manager maps. The count is updated only by `findAvailableExtensions` (section 2.3 step 4) and by the listener (section 2.11).

8.3 No retry, no backoff. Errors from `findExtensions` are caught inside the repo layer (section 4.2). Errors from the repo refresh (`updateExtensionRepo.awaitAll()`) propagate to the caller. The caller (`MainActivity`) catches `Exception` and logs it.

---

## 9. Repo refresh (OBSERVED, domain files outside this scope, used here)

9.1 `UpdateMangaExtensionRepo.awaitAll()`: `coroutineScope { repository.getAll().map { async { await(it) } }.awaitAll() }`. Path: `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/UpdateMangaExtensionRepo.kt`.

9.2 `await(repo)`:
1. `newRepo = service.fetchRepoDetails(repo.baseUrl) ?: return` (`ExtensionRepoService.fetchRepoDetails` returns null on any exception, and logs `"Failed to fetch repo details"`).
2. Upsert only when `repo.signingKeyFingerprint.startsWith("NOFINGERPRINT")` **or** `repo.signingKeyFingerprint == newRepo.signingKeyFingerprint`. A fingerprint change on an already-fingerprinted repo is ignored silently (trust is not moved to a new key).
3. `repository.upsertRepo(newRepo)`.

9.3 `fetchRepoDetails(repo)` = `GET "$repo/repo.json"`, parsed as `ExtensionRepoMetaDto { meta: { name, shortName?, website, signingKeyFingerprint } }`, mapped to `ExtensionRepo(baseUrl = repo, ...)` (`domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt`, `ExtensionRepoDto.kt`).

9.4 **OBSERVED defect:** `awaitAll` has no per-repo exception handling. An `upsertRepo` that throws `SaveExtensionRepoException` (for example a fingerprint collision with another repo, enforced by `UNIQUE`) cancels the sibling refresh coroutines and propagates to `checkForUpdates`.

---

## 10. Trust (OBSERVED, `app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt`)

10.1 `isTrusted(pkgInfo, fingerprints: List<String>): Boolean` (suspend):
- `trustedFingerprints = mangaExtensionRepoRepository.getAll().map { signingKeyFingerprint }.toHashSet()`.
- `key = "${pkgInfo.packageName}:${getLongVersionCode(pkgInfo)}:${fingerprints.last()}"`.
- Return `trustedFingerprints.any { fingerprints.contains(it) } || key in preferences.trustedExtensions().get()`.
- The repo rule matches **any** signing digest of the package. The per-version rule matches only the **last** digest.

10.2 `trust(pkgName, versionCode, signatureHash)`: `preferences.trustedExtensions().getAndSet { exts -> exts.filterNot { it.startsWith("$pkgName:") }.toMutableSet() + "$pkgName:$versionCode:$signatureHash" }`.
- Effect: one trusted version per package. Trusting version N removes any earlier trusted entry for that package.

10.3 `revokeAll()` = `preferences.trustedExtensions().delete()`. Repo-level trust is unaffected (repos are not deleted).

10.4 Fingerprint values in the repo table come from `repo.json` `meta.signingKeyFingerprint`. The fingerprint format is whatever the repo publishes; the comparison is exact string equality with the locally computed digest (lowercase hex). A repo publishing uppercase hex never matches.

10.5 A repo with `NOFINGERPRINT-<n>` (created by the legacy migration, section 14) matches no digest until a refresh replaces it (section 9.2).

---

## 11. Browse views (OBSERVED)

### 11.1 `GetMangaExtensionsByType.subscribe()` (`app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionsByType.kt:15-59`)
- `showNsfwSources = preferences.showNsfwSource().get()` is read **once** when `subscribe()` is called (line 16). Changing the preference requires re-subscribing.
- Inputs combined: `enabledLanguages.changes()`, `installedExtensionsFlow`, `untrustedExtensionsFlow`, `availableExtensionsFlow`. Any change emits a new `MangaExtensions`.
- **Installed:**
  1. Filter: `showNsfwSources || !isNsfw`.
  2. Sort: `compareBy<Installed> { !it.isObsolete }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name }`. Because `false < true`, **obsolete extensions sort first**.
  3. Partition by `hasUpdate`: first list = `updates`, second = `installed`.
- **Untrusted:** sort by `name` case-insensitively. No NSFW filter applied.
- **Available:**
  1. Filter raw extensions: not installed (`_installed.none { pkgName == ext.pkgName }`), not untrusted, and `showNsfwSources || !ext.isNsfw`.
  2. `flatMap`: if `ext.sources` is empty → include `ext` iff `ext.lang in enabledLanguages`. Otherwise include, for each source with `source.lang in enabledLanguages`, a copy: `name = source.name`, `lang = source.lang`, `pkgName = "${ext.pkgName}-${source.id}"`, `sources = listOf(source)`.
  3. Sort by `name` case-insensitively.
- Output `MangaExtensions(updates, installed, available, untrusted)`.

**OBSERVED:** the `pkgName` of a split row (`"<pkg>-<sourceId>"`) is used as the key for in-flight state (section 7.1 uses `extension.pkgName`). Two split rows from one APK therefore start two downloads and two installs of the same APK, because their keys differ.

**Proposal:** key in-flight state and the install queue by the package name only (the APK), so two rows of one APK share one download (T-18a-034).

### 11.2 `GetMangaExtensionLanguages.subscribe()` (`app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionLanguages.kt:13-31`)
1. `combine(enabledLanguages.changes(), availableExtensionsFlow)`.
2. `flatMap`: extension with empty `sources` → `[ext.lang]`; otherwise each `source.lang`.
3. `distinct()`.
4. `sortedWith(compareBy<String> { it !in enabled }.then(LocaleHelper.comparator))`. Enabled languages first. Then `LocaleHelper.comparator` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:17-25`): `"all"` sorts first; otherwise compare localised display names.

### 11.3 `GetExtensionSources.subscribe(extension)` (`app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionSources.kt:13-30`)
- `isMultiSource = sources.size > 1`.
- `isMultiLangSingleSource = isMultiSource && sources.map { name }.distinct().size == 1`.
- Per source: `enabled = id.toString() !in disabledMangaSources` (`hidden_catalogues`, read as a `Set<String>`); `labelAsName = isMultiSource && !isMultiLangSingleSource`.
- Label rule in plain words: a single-source extension is labelled by language. A multi-source extension whose sources all share one name is labelled by language. A multi-source extension with different names is labelled by name.
- Re-emits on every change of `disabledMangaSources`.

---

## 12. Screen-model callers (OBSERVED, UI files outside this scope, behaviour that this area relies on)

12.1 `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/MangaExtensionsScreenModel.kt`:
- Line 145: `screenModelScope.launchIO { findAvailableExtensions() }` (on screen start).
- Lines 206-213: pull-to-refresh sets `isRefreshing = true`, calls `extensionManager.findAvailableExtensions()`, then `delay(1.seconds)` ("Fake slower refresh so it doesn't seem like it's not doing anything"), then clears the flag.
- Line 174: `installExtension(ext).collectToInstallUpdate(ext)`.
- Line 180: `updateExtension(ext).collectToInstallUpdate(ext)`.
- Line 185: `cancelInstallUpdateExtension(ext)`.
- Line 203: `uninstallExtension(ext)`.
- `collectToInstallUpdate` (lines 196-200): each step → `currentDownloads[pkgName] = step`; `onCompletion` → remove the key. Key = `extension.pkgName` (the row's, possibly split).

12.2 `MangaExtensionDetailsScreenModel.kt:125`: `uninstallExtension(extension)`.

12.3 Trust dialog text (spec): title `Untrusted extension`; body `Malicious extensions can read any stored login credentials or execute arbitrary code.` followed by a blank line and `By trusting this extension, you accept these risks.` (`spec/entries.json:1807-1812`, `extension_trust`). Private uninstall confirm: `Do you really want to remove "%s" extension?` (`spec/entries.json:1819`, `remove_private_extension`).

---

## 13. App-launch update trigger (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:397-402`:
```
LaunchedEffect(Unit) {
    try {
        AnimeExtensionApi().checkForUpdates(context)
        MangaExtensionApi().checkForUpdates(context)
    } catch (e: Exception) {
        logcat(LogPriority.ERROR, e)
    }
}
```
Runs once per `MainActivity` composition (app start). Calls with the default `fromAvailableExtensionList = false`, so there is no daily gate on launch (section 8.1).

---

## 14. Notifications (OBSERVED)

14.1 `ExtensionUpdateNotifier.promptUpdates(names, anime = false)` (`app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt:19-44`):
- Notification id `Notifications.ID_UPDATES_TO_EXTS = -401`; channel `CHANNEL_EXTENSIONS_UPDATE = "ext_apk_update_channel"`.
- Title: `MR.plurals.update_check_notification_ext_updates` with quantity `names.size`: `one` = `"Extension update available"`; `other` = `"%d extension updates available"`.
- If `!SecurityPreferences.hideNotificationContent()`: content text = `names.joinToString(", ")`, and `BigTextStyle` with the same text.
- Small icon `R.drawable.ic_extension_24dp`. Content intent: `NotificationReceiver.openExtensionsPendingActivity(context)` (or the anime variant when `anime = true`). `setAutoCancel(true)`.
- Channel importance `IMPORTANCE_DEFAULT`, group `GROUP_APK_UPDATES` (`app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:191-196`).

14.2 `dismiss()` cancels `ID_UPDATES_TO_EXTS` (-401).

14.3 **OBSERVED:** the anime variant uses the same id -401, so anime and manga notifications overwrite each other.

14.4 Install foreground notification (`MangaExtensionInstallService.kt:26-36`): id `ID_EXTENSION_INSTALLER = -402`; channel `CHANNEL_EXTENSIONS_UPDATE`; title `MR.strings.ext_install_service_notif` = `"Installing extension…"`; ongoing; not auto-cancel; `showWhen = false`; indeterminate progress (`setProgress(100, 100, true)`); small icon `R.drawable.ic_ani`. Dropped (section 15).

14.5 Hide-content preference: `SecurityPreferences.hideNotificationContent()` (read in 14.1; key in `p1-data-and-config.md`).

---

## 15. Dropped Android mechanisms (what is removed, what the user loses)

Every row names the Kotlin source, the reason, and the user-visible loss.

| Mechanism | Source | Reason | What the user loses |
|---|---|---|---|
| APK and DEX execution through `ChildFirstPathClassLoader` / `PathClassLoader` and `Class.forName` | `MangaExtensionLoader.kt:296-343` | Android runtime only. | Every existing Aniyomi APK extension (installed or in repos) stops working. The index still lists them, marked "Not runnable on Linux" (section 16). |
| `PackageManager` discovery, shared (system-installed) extensions | `MangaExtensionLoader.kt:129-183`, `:224-235` | No system package database for these packages on Linux. | Extensions installed by other Android apps are not visible. |
| `PackageInstallerInstallerManga` (system installer session, confirmation intent) | `installer/PackageInstallerInstallerManga.kt` | Android-only API. | Nothing; there is no system installer on Linux for these packages. |
| `ShizukuInstallerManga` (`pm install-*` shell) | `installer/ShizukuInstallerManga.kt` | Android-only. | Nothing for Linux. |
| `MangaExtensionInstallActivity` (`ACTION_INSTALL_PACKAGE`, result codes) and the MIUI workaround window of 1 s (`1.seconds.inWholeNanoseconds`) | `util/MangaExtensionInstallActivity.kt` | Android Activity result API. | Nothing for Linux. |
| `MangaExtensionInstallService` foreground service and notification `-402` | `util/MangaExtensionInstallService.kt` | Android foreground-service rules. | The system-tray "Installing extension…" notification. In-app progress remains (section 7.12). |
| `DownloadManager` (`enqueue`, `query`, `ACTION_DOWNLOAD_COMPLETE`, `DIRECTORY_DOWNLOADS`) | `util/MangaExtensionInstaller.kt` | Android-only. | Android download notifications. Replaced by a reqwest download with in-app progress. |
| `extension_installer` preference with values `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE` | `app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt`, `BasePreferences.kt:31-36` | Only one install path remains. | The choice of installer. |
| Trusted-key list keyed by APK certificate digests (`trusted_extensions`) | `TrustMangaExtension.kt` | Keys are APK certificate SHA-256 digests; they do not identify WASM packages. | Existing trust decisions are not carried over. The user re-trusts once per package. |
| `ExtensionUpdateNotifier` anime variant (`anime = true`, notification id shared with manga) | `ExtensionUpdateNotifier.kt:37-41` | Manga-only in this area. | None in this area; anime is a separate area. |
| `FileProvider` URIs (`getUriCompat`) | `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt:16` | Android content URIs. | None. |
| `ACTION_UNINSTALL_PACKAGE` for shared packages | `I:221-231` | Only private packages exist on Linux. | Removal of system-installed extensions (dropped with shared packages). |
| `ChildFirstPathClassLoader` (parent-last class loader) | `app/src/main/java/eu/kanade/tachiyomi/util/system/ChildFirstPathClassLoader.kt` | DEX only. | None. |

Keep and port (not dropped): the name normalisation, lib-version gate, NSFW gate, language derivation, the trust rule (with ed25519 replacing the digest), the downgrade and signer-continuity rules for installs, the install queue semantics, and the notification text.

---

## 16. Linux package model and index compatibility (Proposal)

16.1 **Decision (see `p1-rust-design.md` section 2):** a Kurobayomi extension is a `.kuroext` file: a zip archive containing `manifest.json` and `source.wasm`. Sources run as a WASM component in `wasmtime`.

16.2 Index: Kurobayomi reads the same `index.min.json` shape (section 4.3). Entries are classified:
- `apk` field present and no `wasm` field → kind `ApkOnly`. Listed in Browse with the badge text `Not runnable on Linux`. Cannot be installed. Never counted as an update.
- `wasm` field present (a relative path in the repo, for example `wasm/<pkg>-<version>.kuroext`) and `sha256` field present → kind `Wasm`. Installable.
- Both present → `Wasm` (the WASM package is preferred).
- A Kurobayomi-only field `ed25519` (base64 of the 64-byte signature of the file's SHA-256 digest) is required for `Wasm` entries. Missing signature → the entry is listed as `Untrusted` even when the repo key matches (see section 16.4).

16.3 Lib version gate: unchanged `1.4` to `1.5` inclusive for the Aniyomi APK kind (filter in section 4.4 still applies). For the `Wasm` kind, the gate is the Kurobayomi ABI version declared in `manifest.json` `"abi": "1.x"`; accepted when the major is 1 and the minor is in `[4, 5]`. The minor range is inherited and re-pinned in T-18a-002 when the ABI freezes. Comparison is by tuple (major, minor), not Double. For the inherited range `1.4`..`1.5` both rules give the same answers (`"1.10"` is rejected by Kotlin because it becomes 1.1, and by Rust because minor 10 is above 5). The tuple rule only differs from the Double rule for minors of 10 or more, so it is kept for the range that later ABI versions may use.

16.4 Trust: section 10 rules hold with these substitutions:
- The "signature digest" of a package = SHA-256 hex of the 32-byte ed25519 **public key** that signed it (the key must equal the repo's `signingKeyFingerprint` for the repo rule). The per-version key format `"<pkg>:<versionCode>:<digest>"` is kept.
- Repo rule: `signingKeyFingerprint` of any repo equals the digest of the key that signed the package.

16.5 **Proposal:** the `repo.json` `signingKeyFingerprint` format stays `sha256 hex lowercase`. Kurobayomi publishes the fingerprint of its ed25519 public key in the same field.

---

## 17. Concurrency and threading (OBSERVED, then Proposal)

17.1 Observed:
- Manager `scope`: default dispatcher (M:48). State flows are thread-safe. `iconMap` is not.
- Loader: `runBlocking` on the caller thread (L:177). The caller of `MangaExtensionManager` init is the Injekt initialisation path. Loading all packages blocks that thread.
- API: `withIOContext` for `findExtensions` (A:43). `checkForUpdates` is not wrapped; it runs on the caller dispatcher (MainActivity: `LaunchedEffect`, Main).
- Installer: downloads are polled with `delay` on the collector's dispatcher. `onCompletion` uses `withUIContext` (I:115).
- Receiver: `SupervisorJob` scope, default dispatcher (MangaExtensionInstallReceiver:26).
- Install queue: `AtomicReference` plus `Collections.synchronizedList`. The check-and-act in `checkQueue` is a CAS on `waitingInstall`; the queue read-then-remove is not atomic as a pair (IQ:103-105).
- Shizuku: IO scope (`Dispatchers.IO`) (ShizukuInstallerManga:23); `exec` is blocking inside the coroutine.

17.2 Proposal (Rust): see `p1-rust-design.md` section 4. In short: all extension state is owned by the GTK main thread (one `ExtensionStore` in `Rc<RefCell<…>>`). Network, filesystem and wasm work runs on a tokio runtime with a bounded worker. Events cross into the main thread through `async-channel`.

---

## 18. Interactions with other areas

| Area | Interaction | Where in this file |
|---|---|---|
| area:02-preferences-system | Keys `source_languages`, `show_nsfw_source`, `hidden_catalogues`, `ext_updates_count`, `trusted_extensions` (app-state prefix), `extension_installer`, `last_ext_check`, `extension_repos` (legacy, migrated) | sections 2, 3, 8, 10, 11 |
| area:03-db-manga-schema-migrations | `extension_repos` table (`data/src/main/sqldelight/data/extension_repos.sq`) | sections 4.1, 9, 10 |
| area:06-data-repos-history-track-source-misc | Repo DTO and repo impl | section 9 |
| area:09-domain-source-track-download-release-misc | Repo interactors (`UpdateMangaExtensionRepo`, `GetMangaExtensionRepo`) | section 9 |
| area:11-app-migrations | `ExternalRepoMigration.kt` and `TrustExtensionRepositoryMigration.kt` move the `extension_repos` preference to the table with `NOFINGERPRINT-<n>` fingerprints and the title `"Repo #<n>"` | section 10.5 |
| area:12-network-stack | `NetworkHelper`, `GET`, `awaitSuccess`, `parseAs`, `Json` config | sections 4, 9 |
| area:13-core-util-storage-archive-torrent | `Hash.sha256`, `copyAndSetReadOnlyTo`, `getUriSize`, `isPackageInstalled` | sections 3.3, 3.9 |
| area:15-source-api-manga | `MangaSource`, `CatalogueSource`, `SourceFactory`, `createSources()` | section 3.8 step 9 |
| area:17-source-app-layer | `StubMangaSource`, `LocaleHelper`, screen models, trust dialog | sections 2.5, 11, 12 |
| area:01-foundation-app-bootstrap | `MainActivity` launch trigger, notification channels | sections 13, 14 |

---

## 19. User-visible message keys

Keys are the Moko resource keys used by the Kotlin code (`MR.strings.*` and `MR.plurals.*`). Text is the base (English) value from `i18n/src/commonMain/moko-resources/base/`. Kurobayomi keeps the English text as the default and uses area:02-preferences-system translation catalogs for the rest.

| Key | Kind | Text (base) | Where used | Kurobayomi |
|---|---|---|---|---|
| `extension_api_error` | string | `Failed to fetch available extensions` | `MangaExtensionManager.kt:147` (toast) | Kept; shown only when every repo failed (T-18a-022) |
| `ext_install_service_notif` | string | `Installing extension…` | `MangaExtensionInstallService.kt:32` (notification) | Dropped (T-18a-041) |
| `ext_installer_shizuku_stopped` | string | `Shizuku is not running` | `ShizukuInstallerManga.kt:116` (toast) | Dropped (T-18a-038) |
| `ext_installer_shizuku_unavailable_dialog` | string | `Install and start Shizuku to use Shizuku as extension installer.` | Installer preference dialog (area:02-preferences-system) | Dropped (T-18a-039) |
| `ext_installer_pref` | string | `Installer` | Preference title | Dropped (T-18a-039) |
| `ext_installer_legacy` | string | `Legacy` | Installer preference entry | Dropped (T-18a-039) |
| `ext_installer_packageinstaller` | string, not translatable | `PackageInstaller` | Installer preference entry | Dropped (T-18a-039) |
| `ext_installer_shizuku` | string, not translatable | `Shizuku` | Installer preference entry | Dropped (T-18a-039) |
| `ext_installer_private` | string, not translatable | `Private` | Installer preference entry | Dropped (T-18a-039) |
| `update_check_notification_ext_updates` | plural | one: `Extension update available`; other: `%d extension updates available` | `ExtensionUpdateNotifier.kt:24-28`, `HomeScreen.kt:300` | Kept (T-18a-040) |
| `ext_revoke_trust` | string | `Revoke trusted unknown extensions` | `SettingsAdvancedScreen.kt:470` (caller, area:17-source-app-layer) | Kept; calls `revoke_all` (T-18a-012) |
| `channel_ext_updates` | string | channel display name | `Notifications.kt:195` | Dropped (no channels on Linux) |
| `extension_trust` dialog | spec (`spec/entries.json:1807-1812`) | title `Untrusted extension`; body `Malicious extensions can read any stored login credentials or execute arbitrary code.` + blank line + `By trusting this extension, you accept these risks.` | Trust dialog (area:17-source-app-layer) | Kept (T-18a-013) |
| `remove_private_extension` dialog | spec (`spec/entries.json:1817-1822`) | `Do you really want to remove "%s" extension?` | Uninstall confirm (area:17-source-app-layer) | Kept (T-18a-033) |

Notes:
- The `ext_installer_*` keys and `ext_install_service_notif` are listed because they exist in the Kotlin resources and must be removed from any copied string table. They are not used in Kurobayomi.
- The Kurobayomi `LoadError` and log texts are not translatable; they follow data section 13.

