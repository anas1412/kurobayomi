# 19-extensions-anime: behaviour spec (part 1 of 1)

Scope: anime extension loader, installer, trust, update API, and anime extension stores (repositories), as listed by `areas.py files 19-extensions-anime 1` (37 files, 3457 LOC). Paths are repo-relative to `/home/blackbox/aniyomi-src`. Line numbers are `path:line` of the cited declaration or statement.

Sections marked **OBSERVED** describe what the Kotlin does. Text marked **Proposal:** describes what Kurobayomi should do. Constants are quoted exactly as they appear in the source.

## 0. Abbreviations

| Abbrev | Repo-relative path |
|---|---|
| LDR | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt` |
| MGR | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt` |
| API | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt` |
| INST | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt` |
| IQ | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/InstallerAnime.kt` |
| PKGI | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/PackageInstallerInstallerAnime.kt` |
| SHZ | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/installer/ShizukuInstallerAnime.kt` |
| ACT | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt` |
| RCV | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt` |
| SVC | `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt` |
| SSVC | `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt` |
| SREPO | `data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt` |
| WIRE | `data/src/main/java/mihon/data/extension/anime/model/NetworkAnimeExtensionStore.kt` |
| LEG | `data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtension.kt` |
| LEGREPO | `data/src/main/java/mihon/data/extension/anime/model/NetworkLegacyAnimeExtensionRepo.kt` |
| SQ | `data/src/main/sqldelightanime/dataanime/extension_store.sq` |
| MODEL | `domain/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeExtension.kt` |
| TRUST | `app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt` |
| BYTYPE | `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionsByType.kt` |
| BYLANG | `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionLanguages.kt` |
| BYSRC | `app/src/main/java/eu/kanade/domain/extension/anime/interactor/GetAnimeExtensionSources.kt` |
| SMODEL | `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreenModel.kt` |
| SCREEN | `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/AnimeExtensionStoresScreen.kt` |
| BCREATE | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/AnimeExtensionStoresBackupCreator.kt` |
| BMODEL | `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionStore.kt` |
| BRESTORE | `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeExtensionStoreRestorer.kt` |

---

## 1. Model and lifecycle

### 1.1 `AnimeExtension` (OBSERVED, MODEL:8-78)
Sealed class with abstract properties `name`, `pkgName`, `versionName`, `versionCode: Long`, `libVersion: Double`, `lang: String?`, `isNsfw`, `isTorrent` (MODEL:10-17).

- `Installed` (MODEL:19-35): `name, pkgName, versionName, versionCode, libVersion, lang: String` (non-null), `isNsfw, isTorrent, pkgFactory: String?, sources: List<AnimeSource>, icon: Drawable?, hasUpdate = false, isObsolete = false, isShared: Boolean, store: AnimeExtensionStore? = null`.
- `Available` (MODEL:37-66): `name, pkgName, versionName, versionCode, libVersion, lang: String, isNsfw, isTorrent, sources: List<AnimeSource>, apkUrl: String, iconUrl: String, store: AnimeExtensionStore`. Nested `data class AnimeSource(id: Long, lang: String, name: String, baseUrl: String)` with `toStubSource(): StubAnimeSource` (MODEL:52-65).
- `Untrusted` (MODEL:68-78): `name, pkgName, versionName, versionCode, libVersion, signatureHash: String, lang: String? = null, isNsfw: Boolean = false, isTorrent: Boolean = false`.

### 1.2 `AnimeLoadResult` (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt:3-7`)
`sealed interface` with `Success(extension: Installed)`, `Untrusted(extension: Untrusted)`, `data object Error`.

### 1.3 `InstallStep` (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt:3-15`)
Enum values in this order: `Idle, Pending, Downloading, Installing, Installed, Error`. `isCompleted()` is true for `Installed`, `Error`, `Idle`. `Installing` is not completed.

### 1.4 `AnimeExtensionStore` (OBSERVED, `domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt:3-15`)
`data class AnimeExtensionStore(indexUrl, name, badgeLabel, signingKey, contact: Contact, isLegacy: Boolean, extensionListUrl: String?)` with nested `Contact(website: String, discord: String?)`.

### 1.5 Manager construction and initialisation order (OBSERVED, MGR:45-49, 51-82)
1. Constructor parameters: `context`, `preferences: SourcePreferences = Injekt.get()`, `trustExtension: TrustAnimeExtension = Injekt.get()` (MGR:45-49).
2. `scope = CoroutineScope(SupervisorJob())` with no dispatcher, so Dispatchers.Default (MGR:51).
3. State holders are declared in this order: `_isInitialized` (MGR:53), `api = AnimeExtensionApi()` (MGR:59), lazy `installer` (MGR:64), `iconMap` (MGR:66), three `MutableStateFlow` maps and their `mapExtensions(scope)` views (MGR:68-75).
4. `init` block (MGR:77-80) runs `initAnimeExtensions()` then `AnimeExtensionInstallReceiver(AnimeInstallationListener()).register(context)`.
5. Only after `init` is `subLanguagesEnabledOnFirstRun = preferences.enabledLanguages().isSet()` assigned (MGR:82). `initAnimeExtensions` does not read the flag, so the order is harmless today.
6. `availableAnimeExtensionsSourcesData` is declared after `init` (MGR:114). It is only written by `findAvailableExtensions`, so it is safe.

**Proposal:** the Rust manager has no constructor side effects. Startup order is: load installed packages, then start the background refresh.

---

## 2. Loader (OBSERVED, LDR)

### 2.1 Constants and fields (LDR:34-53, 61, 63)
- `preferences: SourcePreferences` and `trustExtension: TrustAnimeExtension` via `injectLazy` (LDR:34-35).
- `loadNsfwSource by lazy { preferences.showNsfwSource().get() }` (LDR:36-38). **OBSERVED:** evaluated once, on first use in the process. Toggling `show_nsfw_source` at runtime changes nothing until restart.
- `EXTENSION_FEATURE = "tachiyomi.animeextension"` (LDR:40).
- `METADATA_SOURCE_CLASS = "tachiyomi.animeextension.class"` (LDR:41).
- `METADATA_SOURCE_FACTORY = "tachiyomi.animeextension.factory"` (LDR:42).
- `METADATA_NSFW = "tachiyomi.animeextension.nsfw"` (LDR:43).
- `METADATA_HAS_README = "tachiyomi.animeextension.hasReadme"` (LDR:44). **OBSERVED:** declared, never read.
- `METADATA_HAS_CHANGELOG = "tachiyomi.animeextension.hasChangelog"` (LDR:45). **OBSERVED:** declared, never read.
- `METADATA_TORRENT = "tachiyomi.animeextension.torrent"` (LDR:46).
- `METADATA_NAME = "aniyomix.name"` (LDR:48).
- `METADATA_EXTENSION_LIB = "aniyomix.extensionLib"` (LDR:49).
- `METADATA_CONTENT_WARNING = "aniyomix.contentWarning"` (LDR:50).
- `METADATA_IS_TORRENT = "aniyomix.torrent"` (LDR:51).
- `SUPPORTED_LIB_VERSIONS = listOf(14.0, 16.0, 17.0)` (LDR:53).
- `PACKAGE_FLAGS = GET_CONFIGURATIONS or GET_META_DATA or GET_SIGNATURES or (SDK >= P ? GET_SIGNING_CERTIFICATES : 0)` (LDR:56-59).
- `PRIVATE_EXTENSION_EXTENSION = "ext"` (LDR:61).
- Private directory: `File(context.filesDir, "exts")` (LDR:63).

Note the anime lib list differs from manga. The manga side uses the range 1.4 to 1.5 (see area:18-extensions-manga). The anime side uses the discrete set 14, 16, 17.

### 2.2 Two locations and selection (OBSERVED, LDR:128-168, 378-392)
- **Shared**: packages installed in the system package manager with feature `tachiyomi.animeextension` (LDR:136-139). Kurobayomi has no equivalent (dropped, see section 17 and `p1-rust-design.md` section 9).
- **Private**: `<filesDir>/exts/<pkg>.ext` (LDR:96-99, 141-157).
- `loadExtensions` merges with `(sharedExtPkgs + privateExtPkgs).distinctBy { packageName }` (LDR:159-161), so shared wins on equal names at the list level. Then `selectExtensionPackage(sharedPkg, privatePkg)` runs per name (LDR:163-167).
- `selectExtensionPackage` (LDR:378-392): only shared → shared; only private → private; both null → null; both present → shared if `longVersionCode(shared) >= longVersionCode(private)`, else private.
- Private files are read-only by design. Before parsing, each `.ext` file is `setReadOnly()`'d if writable (LDR:146-149). Android 14 requires this.

### 2.3 `loadExtensions(context): List<AnimeLoadResult>` (OBSERVED, LDR:125-179)
1. `installedPkgs` = `getInstalledPackages(PackageInfoFlags.of(PACKAGE_FLAGS))` on SDK >= TIRAMISU (LDR:128-131), else `getInstalledPackages(PACKAGE_FLAGS)` (LDR:133).
2. Shared sequence: filter by `isPackageAnExtension`, map to `AnimeExtensionInfo(isShared = true)` (LDR:136-139).
3. Private sequence: `listFiles()` (null → empty), `isFile && extension == "ext"`, `setReadOnly`, `getPackageArchiveInfo(path, PACKAGE_FLAGS)`, `applicationInfo!!.fixBasePaths(path)` (LDR:141-157). Failed parses are dropped (`mapNotNull`). Then `isPackageAnExtension` filter, then map to `isShared = false`.
4. Merge and select as in 2.2 (LDR:159-168). The list is materialised with `.toList()`.
5. Empty → `emptyList()` (LDR:170).
6. `runBlocking { extPkgs.map { async { loadExtension(context, it) } }.awaitAll() }` (LDR:173-178).

**OBSERVED:** the comment says "Load each extension concurrently". `loadExtension` contains no suspension point, so each `async` runs to completion on the `runBlocking` thread. The loads run sequentially on the calling thread, one after another. The calling thread is the thread that constructs `AnimeExtensionManager`, which is the Injekt initialisation path.

**OBSERVED:** `privateExtPkgs` is a lazy sequence. It is traversed in step 4 and again in the `singleOrNull` inside the `mapNotNull` (LDR:164-165). Each traversal re-lists the directory and re-parses every `.ext` file.

### 2.4 `loadExtensionFromPkgName(context, pkgName): AnimeLoadResult` (OBSERVED, LDR:185-192)
- `info = getAnimeExtensionInfoFromPkgName(context, pkgName)`. Null → log `"Extension package is not found ($pkgName)"` at ERROR and return `AnimeLoadResult.Error` (LDR:187-190).
- Otherwise `loadExtension(context, info)`.

### 2.5 `getAnimeExtensionInfoFromPkgName` (OBSERVED, LDR:198-234)
1. Private: `<dir>/<pkgName>.ext` if `isFile` → `getPackageArchiveInfo(path, PACKAGE_FLAGS)`, `takeIf(isPackageAnExtension)`, `applicationInfo!!.fixBasePaths(path)`, wrap with `isShared = false` (LDR:199-218).
2. Shared: `getPackageInfo(pkgName, PACKAGE_FLAGS)`, `takeIf(isPackageAnExtension)`, wrap `isShared = true`; `NameNotFoundException` → null (LDR:220-231).
3. Return `selectExtensionPackage(sharedPkg, privatePkg)` (LDR:233).

### 2.6 `getAnimeExtensionPackageInfoFromPkgName` (OBSERVED, LDR:194-196)
Returns `PackageInfo?` from 2.5. Used by the installer's signature checks and by `MGR.getAppIconForSource`.

### 2.7 `loadExtension(context, extensionInfo)`: ordered checks (OBSERVED, LDR:242-370)

Each step returns on failure. The order matters because an earlier check hides later ones.

| Step | Lines | Check | On failure |
|---|---|---|---|
| 1 | 245-247 | `pkgInfo = extensionInfo.packageInfo`, `appInfo = pkgInfo.applicationInfo!!`, `pkgName = pkgInfo.packageName` | Throws NPE on a null `applicationInfo`. There is no guard. |
| 2 | 249-250 | `extName = appInfo.metaData.getString("aniyomix.name") ?: pkgManager.getApplicationLabel(appInfo).toString().substringAfter("Aniyomi: ")` | Never fails. `substringAfter` returns the whole label when the prefix is absent. |
| 3 | 251-252 | `versionName = pkgInfo.versionName`; `versionCode = PackageInfoCompat.getLongVersionCode(pkgInfo)` | Never fails. |
| 4 | 254-257 | `versionName.isNullOrEmpty()` → log `"Missing versionName for extension $extName"` at WARN | `AnimeLoadResult.Error` |
| 5 | 260-270 | Lib version (2.8) | log `"Lib version is $libVersion, while only version(s) ${SUPPORTED_LIB_VERSIONS.joinToString()} are supported"` at WARN | `AnimeLoadResult.Error` |
| 6 | 272-275 | `signatures = getSignatures(pkgInfo)`; null or empty → log `"Package $pkgName isn't signed"` at WARN | `AnimeLoadResult.Error` |
| 7 | 276-287 | `!trustExtension.isTrusted(pkgInfo, signatures)` → build `AnimeExtension.Untrusted(extName, pkgName, versionName, versionCode, libVersion, signatures.last())`, log `"Extension $pkgName isn't trusted"` at WARN | `AnimeLoadResult.Untrusted(extension)` |
| 8 | 289-294 | `isNsfw = getInt("aniyomix.contentWarning") > 0 || getInt("tachiyomi.animeextension.nsfw") == 1`. If `!loadNsfwSource && isNsfw` → log `"NSFW extension $pkgName not allowed"` at WARN | `AnimeLoadResult.Error` |
| 9 | 296-297 | `isTorrent = getBoolean("aniyomix.torrent") || getInt("tachiyomi.animeextension.torrent") == 1` | Never fails. |
| 10 | 298-303 | `classLoader = ChildFirstPathClassLoader(appInfo.sourceDir, null, context.classLoader)`; exception → log `"Extension load error: $extName ($pkgName)"` at ERROR | `AnimeLoadResult.Error` |
| 11 | 305-346 | Source classes (2.9) | `AnimeLoadResult.Error` |
| 12 | 348-353 | `lang` derivation (2.10) | Never fails. |
| 13 | 355-369 | Return `Success(AnimeExtension.Installed(name = extName, pkgName, versionName, versionCode, libVersion, lang, isNsfw, isTorrent, sources, pkgFactory = getString("tachiyomi.animeextension.factory"), icon = appInfo.loadIcon(pkgManager), isShared = extensionInfo.isShared))` | |

**OBSERVED ordering consequence:** an untrusted NSFW extension is reported as `Untrusted`, not `Error`, because step 7 runs before step 8. An unsigned extension is `Error` at step 6, before the trust check.

**OBSERVED:** `versionName` is validated before the lib version. A package with an empty `versionName` never reaches the lib check.

### 2.8 Lib version (OBSERVED, LDR:260-270)
```
libVersion = appInfo.metaData.getInt("aniyomix.extensionLib")
    .takeUnless { it == 0 }?.toString()?.toDouble()
    ?: versionName.substringBeforeLast('.').toDoubleOrNull()
accepted iff libVersion != null && libVersion in [14.0, 16.0, 17.0]
```
Worked examples (derived from the rule, not executed):
- Metadata `aniyomix.extensionLib = 16` → `16.0` → accepted.
- Metadata absent, `versionName = "16.0"` → `substringBeforeLast('.')` = `"16"` → `16.0` → accepted.
- Metadata absent, `versionName = "16.3.1"` → `"16.3"` → `16.3` → **rejected**. The fallback works only for two-part version names.
- Metadata absent, `versionName = "14"` → `"14"` → `14.0` → accepted.
- Metadata absent, `versionName = "1.4.13"` → `"1.4"` → `1.4` → rejected.
- Metadata `aniyomix.extensionLib = 0` (or absent) falls through to the name rule.

**OBSERVED:** the `Double` type is used for an integer concept. The fallback only makes sense for the two-part names that Aniyomi uses.

### 2.9 Source instantiation (OBSERVED, LDR:305-346)
1. `sourceClassNames = appInfo.metaData.getString("tachiyomi.animeextension.class")!!.split(";")` (LDR:305-306). A missing key throws `NullPointerException` **outside any try**. The exception escapes `loadExtension` and aborts the whole `runBlocking` batch in `loadExtensions`.
2. Each entry: `trim()` (LDR:308). If it starts with `"."`, prefix `pkgInfo.packageName` (LDR:309-310).
3. For each class name, `Class.forName(name, false, classLoader).getDeclaredConstructor().newInstance()` (LDR:317-321):
   - `is AnimeSource` → `listOf(obj)`.
   - `is AnimeSourceFactory` → `obj.createSources()`.
   - Anything else → `throw Exception("Unknown source class type: ${obj.javaClass}")`.
4. `LinkageError` on the first attempt → retry with `PathClassLoader(appInfo.sourceDir, null, context.classLoader)` (LDR:322-337). Any `Throwable` on the retry → log `"Extension load error: $extName ($it)"` and return `Error` (LDR:338-341).
5. Any other `Throwable` on the first attempt → same log and `Error` (LDR:342-345).
6. **OBSERVED:** one failing class makes the whole extension `Error`. There is no partial load.
7. **OBSERVED:** instantiating a source runs its constructor during discovery. A slow or throwing constructor blocks `loadExtensions`.

**Proposal (Linux):** discovery reads only `manifest.json` and never runs package code. Sources are instantiated lazily on first use. See section 17 and `p1-topic-install-and-trust.md`.

### 2.10 Language derivation (OBSERVED, LDR:348-353)
`langs = sources.map { it.lang }.toSet()`, then
- size 0 → `""`
- size 1 → the single language
- otherwise → `"all"`

**OBSERVED inconsistency:** the store side (2.22) uses `"all"` for zero sources. The loader uses `""`. An installed extension with no sources has `lang = ""` and an available one has `lang = "all"`.

### 2.11 Signing digests (OBSERVED, LDR:409-423)
- API 28 and up: `signingInfo!!`. If `hasMultipleSigners()`, use `apkContentsSigners`; else `signingCertificateHistory`.
- API below 28: `pkgInfo.signatures` (deprecated path).
- Each signature → `Hash.sha256(it.toByteArray())` (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt:16-18`, which encodes lowercase hex with alphabet `0123456789abcdef`).
- Returns `List<String>?`. Null when the input list is null.

### 2.12 `fixBasePaths` (OBSERVED, LDR:429-436)
If `sourceDir` is null, set it to the APK path. The same for `publicSourceDir`. Needed on Android 13 and later, where archive-parsed `ApplicationInfo` has no source dir. The icon loader depends on it.

### 2.13 `isPackageAnExtension` (OBSERVED, LDR:399-401)
True when `pkgInfo.reqFeatures.orEmpty().any { it.name == "tachiyomi.animeextension" }`.

### 2.14 Install-directory helpers (OBSERVED, LDR:116-118)
`uninstallPrivateExtension(context, pkgName)` deletes `<dir>/<pkgName>.ext`. The return value is ignored.

---

## 3. Private install file (OBSERVED, LDR:65-114)

`installPrivateExtensionFile(context, file: File): Boolean`. Ordered steps:

1. `extension = getPackageArchiveInfo(file.absolutePath, PACKAGE_FLAGS)?.takeIf(isPackageAnExtension) ?: return false` (LDR:66-70).
2. `currentExtension = getAnimeExtensionPackageInfoFromPkgName(context, extension.packageName)` (LDR:71-74). This is the best of shared and private (2.2).
3. If `currentExtension != null`:
   - a. If `longVersionCode(extension) < longVersionCode(currentExtension)`: log `"Installed extension version is higher. Downgrading is not allowed."` at ERROR, return false (LDR:77-82). Equal version codes are allowed (reinstall).
   - b. `extensionSignatures = getSignatures(extension)`. Null or empty → log `"Extension to be installed is not signed."` at ERROR, return false (LDR:84-88).
   - c. `!extensionSignatures.containsAll(getSignatures(currentExtension)!!)` → log `"Installed extension signature is not matched."` at ERROR, return false (LDR:90-93). The new package must contain every signing digest of the installed one.
4. `target = File(getPrivateExtensionDir(context), "${extension.packageName}.ext")` (LDR:96-99).
5. Inside `try`: `target.delete()`, then `file.copyAndSetReadOnlyTo(target, overwrite = true)`. If `currentExtension != null`, `RCV.notifyReplaced(context, pkg)`, else `RCV.notifyAdded(context, pkg)`. Return true (LDR:100-108).
6. `catch (e: Exception)` → log `"Failed to copy extension file."` at ERROR, `target.delete()`, return false (LDR:109-113).

**OBSERVED:** for a first install (no current extension), **no signature check runs here**. The package is copied. Trust is decided later, at load time (2.7 step 7). An unsigned package is copied and then never appears, because load returns `Error` (step 6).

**OBSERVED:** the install step is not set here. The caller sets `Installed` (section 8.5).

---

## 4. Install receiver semantics (OBSERVED, RCV:24-146)

The receiver is a Linux-dropped mechanism (see section 17). Its *semantics* are kept because the manager reacts to package events.

1. Filter (RCV:32-40): actions `ACTION_PACKAGE_ADDED`, `ACTION_PACKAGE_REPLACED`, `ACTION_PACKAGE_REMOVED`, and self actions `"${applicationId}.ACTION_EXTENSION_ADDED"`, `"...ACTION_EXTENSION_REPLACED"`, `"...ACTION_EXTENSION_REMOVED"`. Data scheme `package`. Registered `RECEIVER_NOT_EXPORTED` (RCV:28-30).
2. `ADDED` (system or self): if `EXTRA_REPLACING` is true, return (RCV:50-51). Otherwise launch `getExtensionFromIntent` and map `Success` → `listener.onExtensionInstalled`, `Untrusted` → `listener.onExtensionUntrusted`, `Error` → nothing (RCV:53-59).
3. `REPLACED` (system or self): no replacing check. Launch the same load. `Success` → `onExtensionUpdated`, `Untrusted` → `onExtensionUntrusted` (RCV:61-69).
4. `REMOVED` (system or self): if replacing, return (RCV:71). Otherwise `pkgName = intent.data?.encodedSchemeSpecificPart`; non-null → `onPackageUninstalled(pkgName)` (RCV:73-76).
5. A missing package name logs `"Package name not found"` at WARN and returns `Error` (RCV:97-101).
6. Self notifications (RCV:127-145): `notifyAdded`, `notifyReplaced`, `notifyRemoved` send an intent with `data = "package:$pkgName"` and `setPackage(context.packageName)`.
7. Scope: `CoroutineScope(SupervisorJob())` (RCV:26), default dispatcher.

**OBSERVED:** `onPackageUninstalled` fires for **every** removed package on the device, not only extensions. For non-extensions the calls are no-ops.

---

## 5. Manager operations (OBSERVED, MGR)

### 5.1 State flows
- `installedExtensionsMapFlow: MutableStateFlow<Map<String, Installed>>` keyed by `pkgName` (MGR:68). `installedExtensionsFlow = ...mapExtensions(scope)` (MGR:69).
- `availableExtensionsMapFlow: MutableStateFlow<Map<String, Available>>` (MGR:71-72).
- `untrustedExtensionsMapFlow: MutableStateFlow<Map<String, Untrusted>>` (MGR:74-75).
- `mapExtensions` = `map { it.values.toList() }.stateIn(scope, SharingStarted.Lazily, value.values.toList())` (MGR:392-396).
- `iconMap: MutableMap<String, Drawable>` (MGR:66). Not thread-safe.

### 5.2 `initAnimeExtensions()` (OBSERVED, MGR:130-142)
1. `extensions = AnimeExtensionLoader.loadExtensions(context)`.
2. `installedExtensionsMapFlow.value = Success results .associate { pkgName to extension }`.
3. `untrustedExtensionsMapFlow.value = Untrusted results .associate { pkgName to extension }`.
4. `Error` results are dropped with no state change.
5. `_isInitialized.value = true`.

Note that `initAnimeExtensions` runs synchronously in the constructor (MGR:78).

### 5.3 Lookup helpers (OBSERVED, MGR:84-112)
- `getExtensionPackage(sourceId: Long): String?` (MGR:84-89): first installed extension where `sources.any { it.id == sourceId }`, returns `pkgName`.
- `getExtensionPackageAsFlow(sourceId): Flow<String?>` (MGR:91-98): the same over `installedExtensionsFlow`.
- `getAppIconForSource(sourceId): Drawable?` (MGR:100-112): finds the package, then `iconMap[pkg] ?: iconMap.getOrPut(pkg) { AnimeExtensionLoader.getAnimeExtensionPackageInfoFromPkgName(context, pkg)!!.applicationInfo!!.loadIcon(context.packageManager) }`. **OBSERVED defect:** both `!!` throw `NullPointerException` if the package was removed after the map was built.

### 5.4 Stub source map (OBSERVED, MGR:114-125)
- `availableAnimeExtensionsSourcesData: Map<Long, StubAnimeSource>` (MGR:114).
- `setupAvailableAnimeExtensionsSourcesDataMap(list)` (MGR:116-123): if `list` is empty, return **without changing the map**. Otherwise `list.flatMap { ext -> ext.sources.map { it.toStubSource() } }.associateBy { it.id }`.
- `getSourceData(id)` (MGR:125) returns the stub or null.
- `StubAnimeSource(id, lang, name)` (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:15-67`). Every fetch method throws `AnimeSourceNotInstalledException` (STUB:25-57). `toString()` returns `"$name (${lang.uppercase()})"` or the id when `name` or `lang` is blank (STUB:59-60).

### 5.5 `findAvailableExtensions()` (OBSERVED, MGR:147-161)
1. `extensions = try { api.findExtensions() } catch (e: Exception) { logcat(ERROR); withUIContext { context.toast(MR.strings.extension_api_error) }; emptyList() }` (MGR:148-154).
2. `enableAdditionalSubLanguages(extensions)` (MGR:156).
3. `availableExtensionsMapFlow.value = extensions.associateBy { it.pkgName }` (MGR:158). **Last entry wins** on duplicate package names.
4. `updatedInstalledAnimeExtensionsStatuses(extensions)` (MGR:159; 5.7).
5. `setupAvailableAnimeExtensionsSourcesDataMap(extensions)` (MGR:160).

**OBSERVED:** `api.findExtensions()` never throws in practice. `SREPO.fetchExtensions()` catches every `Exception` and returns an empty list (SREPO:69-88; section 7.8). The toast therefore never shows for a store error. An empty result replaces the available map with an empty map.

### 5.6 First-run sub-language enablement (OBSERVED, MGR:172-191)
Runs inside `findAvailableExtensions`.
1. Return early if `subLanguagesEnabledOnFirstRun` is true or `animeextensions.isEmpty()` (MGR:173-175). The flag starts as `preferences.enabledLanguages().isSet()` (MGR:82).
2. `availableLanguages = extensions.flatMap(AnimeExtension.Available::sources).distinctBy(AnimeSource::lang).map(lang)`. Source languages only (MGR:178-181).
3. `deviceLanguage = Locale.getDefault().language` (MGR:183).
4. `defaultLanguages = preferences.enabledLanguages().defaultValue()`, which is `setOf("all", "en", Locale.getDefault().language)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:72-74`) (MGR:184).
5. `languagesToEnable = availableLanguages.filter { it != deviceLanguage && it.startsWith(deviceLanguage) }` (MGR:185-187).
6. `preferences.enabledLanguages().set(defaultLanguages + languagesToEnable)`; then `subLanguagesEnabledOnFirstRun = true` (MGR:189-190).

Example: device `zh`, source languages `zh-Hans`, `zh-Hant`, `en`, `ja`. Result `{all, en, zh, zh-Hans, zh-Hant}`.

**OBSERVED:** the flag is only in memory until the first `set`. After the first set, `isSet()` is true on later launches.

### 5.7 Status reconciliation (OBSERVED, MGR:198-234)
Called from `findAvailableExtensions` with the new available list.

1. If `availableExtensions.isEmpty()`: `preferences.animeExtensionUpdatesCount().set(0)` and return (MGR:201-204). **The installed map is not touched.** No obsolete marks, no update flags.
2. `installedExtensionsMap = installedExtensionsMapFlow.value.toMutableMap()`; `changed = false` (MGR:206-207).
3. For each `(pkgName, extension)` in the map (MGR:209-229):
   - `availableExt = availableExtensions.find { it.pkgName == pkgName }` (first match, MGR:210).
   - If `availableExt == null && !extension.isObsolete`: `copy(isObsolete = true)`, `changed = true` (MGR:212-214).
   - Else if `availableExt != null`: `hasUpdate = extension.updateExists(availableExt)`. If `extension.hasUpdate != hasUpdate`: `copy(hasUpdate = hasUpdate, store = availableExt.store)`. Else `copy(store = availableExt.store)`. In both cases `changed = true` (MGR:215-228).
   - If `availableExt == null && extension.isObsolete`: no change.
4. If `changed`: `installedExtensionsMapFlow.value = installedExtensionsMap` (MGR:230-232).
5. `updatePendingUpdatesCount()` (MGR:233).

**OBSERVED defect (sticky obsolete):** `isObsolete` is never reset to `false`. The `else if` branch copies only `hasUpdate` and `store`. An extension that disappears from every store and reappears stays marked "Orphaned" until restart, because the loader creates fresh objects with `isObsolete = false` on reload.

**OBSERVED:** `find` returns the first match, while `availableExtensionsMapFlow` (used by `updateExists()` with no argument, MGR:376) keeps the last match. With duplicate package names across stores, the two can disagree.

**OBSERVED:** `changed = true` is set even when nothing differs. The flow is re-emitted on every refresh.

### 5.8 Install and update entry points (OBSERVED, MGR:243-283)
- `installExtension(extension: Available): Flow<InstallStep> = installer.downloadAndInstall(extension.apkUrl, extension)` (MGR:243-245).
- `updateExtension(extension: Installed): Flow<InstallStep>` (MGR:254-257): `availableExt = availableExtensionsMapFlow.value[extension.pkgName] ?: return emptyFlow()`; then `installExtension(availableExt)`. **OBSERVED:** when no available entry exists, the caller gets an empty flow and no step at all.
- `cancelInstallUpdateExtension(extension)` = `installer.cancelInstall(extension.pkgName)` (MGR:259-261).
- `setInstalling(downloadId)` = `installer.updateInstallStep(downloadId, InstallStep.Installing)` (MGR:268-270).
- `updateInstallStep(downloadId, step)` (MGR:272-274).
- `uninstallExtension(extension)` = `installer.uninstallApk(extension.pkgName)` (MGR:281-283).

### 5.9 Trust action (OBSERVED, MGR:291-301)
`trust(extension: Untrusted)`:
1. `untrustedExtensionsMapFlow.value[pkgName] ?: return` (MGR:292). No-op when not in the untrusted map.
2. `trustExtension.trust(pkgName, versionCode, signatureHash)` (MGR:294; 6.1).
3. `untrustedExtensionsMapFlow.value -= pkgName` (MGR:296).
4. `AnimeExtensionLoader.loadExtensionFromPkgName(context, pkgName)`; if `Success`, `registerNewExtension(extension)` (MGR:298-300).

**OBSERVED:** `registerNewExtension` does not call `withUpdateCheck()` and does not call `updatePendingUpdatesCount()` (MGR:308-310). A freshly trusted extension with an available update shows no update flag until the next `findAvailableExtensions`.

**OBSERVED:** if the reload returns `Untrusted` again (for example, the signature changed), nothing is registered. The package is in neither map.

### 5.10 Registration (OBSERVED, MGR:308-331)
- `registerNewExtension(ext) = installedExtensionsMapFlow.value += ext` (MGR:308-310). `+` replaces by `pkgName` (MGR:390).
- `registerUpdatedExtension(ext)` is the same operation (MGR:318-320).
- `unregisterAnimeExtension(pkgName)` removes the key from **both** installed and untrusted maps (MGR:328-331).

### 5.11 `AnimeInstallationListener` (OBSERVED, MGR:336-359)
- `onExtensionInstalled(ext)`: `registerNewExtension(ext.withUpdateCheck())`; `updatePendingUpdatesCount()` (MGR:338-341).
- `onExtensionUpdated(ext)`: `registerUpdatedExtension(ext.withUpdateCheck())`; `updatePendingUpdatesCount()` (MGR:343-346).
- `onExtensionUntrusted(ext)`: `installedExtensionsMapFlow.value -= pkgName`; `untrustedExtensionsMapFlow.value += ext`; `updatePendingUpdatesCount()` (MGR:348-352).
- `onPackageUninstalled(pkgName)`: `AnimeExtensionLoader.uninstallPrivateExtension(context, pkgName)`; `unregisterAnimeExtension(pkgName)`; `updatePendingUpdatesCount()` (MGR:354-358).

### 5.12 Update flags (OBSERVED, MGR:364-388)
- `withUpdateCheck()` (MGR:364-370): if `updateExists()` then `copy(hasUpdate = true)`, else `this`. It can only set `true`.
- `updateExists(availableExtension: Available? = null): Boolean` (MGR:372-380): `availableExt = param ?: availableExtensionsMapFlow.value[pkgName] ?: return false`; returns `availableExt.versionCode > versionCode || availableExt.libVersion > libVersion`.
- `updatePendingUpdatesCount()` (MGR:382-388): `count = installedExtensionsMapFlow.value.values.count { it.hasUpdate }`; `preferences.animeExtensionUpdatesCount().set(count)` (`animeext_updates_count`); if `count == 0`, `ExtensionUpdateNotifier(context).dismiss()`.

**OBSERVED:** the obsolete extensions with `hasUpdate` are counted too, because the count does not filter `isObsolete`.

---

## 6. Trust (OBSERVED, TRUST:9-31)

### 6.1 `trust(pkgName, versionCode, signatureHash)` (TRUST:20-27)
`preferences.trustedExtensions().getAndSet { exts -> removed = exts.filterNot { it.startsWith("$pkgName:") }.toMutableSet(); removed += "$pkgName:$versionCode:$signatureHash" }`. Effect: one trusted version per package. Trusting version N removes earlier trusted entries for that package. The stored string format is `"<pkgName>:<versionCode>:<signatureHash>"`.

### 6.2 `isTrusted(pkgInfo, fingerprints: List<String>): Boolean` (TRUST:14-18)
1. `trustedFingerprints = repository.getAll().map { it.signingKey }.toHashSet()` (TRUST:15). This reads the anime store table on every call.
2. `key = "${pkgInfo.packageName}:${getLongVersionCode(pkgInfo)}:${fingerprints.last()}"` (TRUST:16).
3. Return `trustedFingerprints.any { fingerprints.contains(it) } || key in preferences.trustedExtensions().get()` (TRUST:17).

The **repo rule** matches any signing digest of the package against any stored store `signingKey`. The **per-version rule** matches only the **last** digest. A store with `signingKey = "NO_SIGNING_KEY"` (created by the legacy migration, section 7.9) can never match a real digest.

### 6.3 `revokeAll()` (TRUST:29-31)
`preferences.trustedExtensions().delete()`. Store-level trust is not affected. The settings action is at `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:70-75` (it also calls the manga revoke and shows `requires_app_restart`).

### 6.4 Trust dialog (OBSERVED, `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionsScreen.kt:261-275`)
Shown for an `AnimeExtension.Untrusted` row. Confirm → `onTrustExtension`. Dismiss button → `onUninstallExtension` on the same item (AnimeExtensionsScreen.kt:264-270). Outside dismiss clears the state. The dialog composable itself is `ExtensionTrustDialog` in `app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionsScreen.kt:522-546`, which is outside this scope. Strings: `untrusted_extension`, `untrusted_extension_message`, `ext_trust`, `ext_uninstall`.

---

## 7. Store protocol (OBSERVED, SSVC, WIRE, LEG, LEGREPO)

### 7.1 `fetch(indexUrl): Result<AnimeExtensionStore>` (OBSERVED, SSVC:28-68)
1. `updatedIndexUrl = indexUrl`. `response = GET(updatedIndexUrl).awaitSuccess()` (SSVC:31). Non-2xx throws.
2. `response.body.source().decompressIfGzipped().use { source -> ... }` (SSVC:32). Gzip is detected by the first two bytes `0x1f 0x8b` (SSVC:111-121).
3. Detect the format by the first byte, `source.peek().readByte()` (SSVC:33):
   - `0x5B` (`[`): **legacy extension list.** Requires `indexUrl.endsWith("/index.min.json")`, otherwise throws `IllegalArgumentException("Provided legacy store url is not valid")` (SSVC:35-38). Sets `updatedIndexUrl = indexUrl.replace("/index.min.json", "/repo.json")` (SSVC:39). Fetches that URL with `awaitSuccess()` and decodes `NetworkLegacyAnimeExtensionRepo` from the body (SSVC:40-42). This second body is **not** gzip-decompressed.
   - `0x7B` (`{`): try `json.decodeFromBufferedSource<NetworkLegacyAnimeExtensionRepo>(source.peek())`. On `IllegalArgumentException` (SerializationException is one), decode `NetworkAnimeExtensionStore` from `source` (SSVC:45-49).
   - Any other byte: `protoBuf.decodeFromByteArray<NetworkAnimeExtensionStore>(source.readByteArray())` (SSVC:50).
4. If the result is `NetworkLegacyAnimeExtensionRepo` with `indexV2 != null`, `return fetch(networkStore.indexV2)` (SSVC:53-55). **OBSERVED:** there is no depth limit, so a cycle recurses until `StackOverflowError`. That is an `Error`, not an `Exception`, so it escapes the `catch` at SSVC:62 and the app does not handle it.
5. Otherwise `networkStore.toExtensionStore(updatedIndexUrl)` (SSVC:57).
6. `Result.success(store)`. Cancellation is re-thrown (SSVC:60-61). Any other `Exception` → log `"Failed to add extension store '$updatedIndexUrl'"` at ERROR and `Result.failure(e)` (SSVC:62-67).

### 7.2 `NetworkAnimeExtensionStore` (OBSERVED, WIRE:17-98)
`@Serializable data class` with `@ProtoNumber` tags:

| Field | Type | Tag | Notes |
|---|---|---|---|
| `name` | String | 1 | required |
| `badgeLabel` | String | 2 | required |
| `signingKey` | String | 3 | required |
| `contact` | `Contact` | 4 | required |
| `extensionList` | `ExtensionList?` | 101 | nullable |
| `extensionListUrl` | String? | 102 | nullable |

Nested:
- `Contact(website: String @1, discord: String? @2)` (WIRE:27-31).
- `ExtensionList(extensions: List<Extension> @1)` (WIRE:33-34).
- `Extension` (WIRE:36-47): `name @1`, `packageName @2`, `resources: Resources @3`, `extensionLib: String @4`, `versionCode: Long @5`, `versionName: String @6`, `contentWarning: ContentWarning @7`, `isTorrent: Boolean = false @8`, `sources: List<Source> @9`.
- `Resources(apkUrl @1: String, iconUrl @2: String)` (WIRE:49-53).
- `Source(id: Long @1, name @2, language @3, homeUrl: String = "" @4, mirrorUrls: List<String> = emptyList() @5, message: String? = null @6)` (WIRE:55-63).
- `enum ContentWarning` (WIRE:65-82): `UNSPECIFIED @0 (JSON alias "CONTENT_WARNING_UNSPECIFIED")`, `SAFE @1 (JSON alias "CONTENT_WARNING_SAFE")`, `MIXED @2 (JSON alias "CONTENT_WARNING_MIXED")`, `NSFW @3 (JSON alias "CONTENT_WARNING_NSFW")`. `@Suppress("Unused")`.

`toExtensionStore(indexUrl)` (WIRE:84-97) builds `AnimeExtensionStore(indexUrl, name, badgeLabel, signingKey, contact, isLegacy = false, extensionListUrl)`.

### 7.3 `NetworkLegacyAnimeExtensionRepo` (OBSERVED, LEGREPO:72-98)
`data class (indexV2: String? @SerialName("index_v2"), meta: Meta)`. `Meta(name: String, shortName: String?, website: String, signingKeyFingerprint: String)`.
`toExtensionStore(indexUrl)` (LEGREPO:85-97): `name = meta.name`, `badgeLabel = meta.shortName ?: meta.name`, `signingKey = meta.signingKeyFingerprint`, `contact = (website = meta.website, discord = null)`, `isLegacy = true`, `extensionListUrl = null`.

### 7.4 `getExtensions(store): Result<List<Available>>` (OBSERVED, SSVC:70-109)
Three branches, chosen in this order:
1. `store.extensionListUrl != null` (SSVC:72). `GET(extensionListUrl)`. Decompress if gzipped. First byte `{` → JSON `NetworkAnimeExtensionStore.ExtensionList`; else protobuf `ExtensionList`. Then `toAvailableExtensions(store)` (SSVC:73-83).
2. `!store.isLegacy` (SSVC:84). `GET(store.indexUrl)`. Decompress. `{` → JSON `NetworkAnimeExtensionStore`, else protobuf. Then `.extensionList!!` (SSVC:92): a null `extensionList` throws `NullPointerException`, which is a failure of the whole store. Then `toAvailableExtensions(store)` (SSVC:93).
3. Otherwise, legacy (SSVC:95-101). `storeBaseUrl = store.indexUrl.removeSuffix("/repo.json")`. `GET("$storeBaseUrl/index.min.json")`. **No gzip handling.** Decode `List<NetworkLegacyAnimeExtension>` as JSON. Map each to `toAvailableExtension(store, storeBaseUrl)`.

Errors: `CancellationException` is re-thrown (SSVC:104-105). Any other `Exception` → `Result.failure(e)` (SSVC:106-107). The failure is silent at the UI level (section 7.8).

### 7.5 `NetworkAnimeExtensionStore.ExtensionList.toAvailableExtensions(store)` (OBSERVED, WIRE:100-127)
For each `extension`:
- `lang = extension.sources.map { it.language }.toSet()`.
- `libVersion = extension.extensionLib.toDouble()` (WIRE:110). **OBSERVED:** `toDouble()`, not `toDoubleOrNull()`. A non-numeric `extensionLib` throws `NumberFormatException` for the whole list, which fails the whole store (section 7.4).
- `lang = if (lang.size == 1) lang.first() else "all"` (WIRE:113). Zero sources gives `"all"`.
- `isNsfw = extension.contentWarning >= ContentWarning.MIXED` (WIRE:114). Enum ordinal comparison, so `MIXED` and `NSFW` are NSFW, `SAFE` and `UNSPECIFIED` are not.
- `isTorrent = extension.isTorrent` (WIRE:115).
- Sources: `AnimeSource(id = source.id, name = source.name, lang = source.language, baseUrl = source.homeUrl)` (WIRE:116-123). `homeUrl` is the base URL. `mirrorUrls` and `message` are not carried into the domain model.
- `apkUrl = resources.apkUrl`, `iconUrl = resources.iconUrl`, `versionCode`, `versionName`, `pkgName = packageName`, `store = store` (WIRE:105-125).

**OBSERVED inconsistency:** the loader sets `isNsfw` from `contentWarning > 0` (LDR:289-290 uses the manifest metadata integer). The store uses `>= MIXED`. A `SAFE` (1) extension is NSFW when installed from its APK metadata and safe in the listing. See edge cases.

### 7.6 Anime store is not lib-filtered (OBSERVED)
Unlike the manga store, no filter by lib version appears in `getExtensions` or `toAvailableExtensions`. Every listed extension is in the list, whatever its `extensionLib`.

### 7.7 Legacy list mapping (OBSERVED, LEG:10-61)
`NetworkLegacyAnimeExtension(name, pkg, apk, lang, code: Long, version: String, nsfw: Int, sources: List<Source>?)`. `toAvailableExtension(store, storeBaseUrl)`:
- `name = name.substringAfter("Aniyomi: ")` (LEG:30).
- `apkUrl = "$storeBaseUrl/apk/$apk"` (LEG:32).
- `iconUrl = "$storeBaseUrl/icon/$pkg.png"` (LEG:33).
- `libVersion = version.substringBeforeLast('.').toDouble()` (LEG:34).
- `isNsfw = nsfw == 1` (LEG:38). `isTorrent = false` (LEG:39).
- `sources`: when `sources` is null or empty, one synthetic source `(id = 0, name = name, lang = lang, baseUrl = "")` (LEG:40-48). Otherwise each `(id, name, lang, baseUrl)` (LEG:50-57).

### 7.8 Repository aggregation (OBSERVED, SREPO:69-88)
`fetchExtensions()`:
1. `try { supervisorScope { handler.awaitList { getAll(mapper) }.map { store -> async { service.getExtensions(store).onFailure { log "Failed to fetch extensions for store '${store.name} (${store.indexUrl})'" } } }.awaitAll().flatMap { it.getOrDefault(emptyList()) } } }` (SREPO:70-87).
2. `catch (e: Exception) { logcat(ERROR); emptyList() }` (SREPO:84-87).

Consequences (OBSERVED):
- Stores fetch in parallel with no limit.
- A failed store contributes nothing and is only logged. **No per-store error reaches the UI.**
- **OBSERVED defect (cancellation swallowed):** `CancellationException` is an `Exception`, so the outer `catch` returns an empty list on cancellation. The caller then continues without checking for cancellation (MGR:156-158), so it writes an empty available map.

### 7.9 Store table operations (OBSERVED, SREPO:19-104; SQ:1-44)
- `insert(indexUrl)` (SREPO:19-21): `service.fetch(indexUrl).mapCatching { upsert(it) }`. The stored URL is the **updated** URL (legacy and indexV2 redirects change it).
- `insertFromPreference(indexUrl, name)` (SREPO:23-36): upsert with `badgeLabel = name`, `signingKey = "NO_SIGNING_KEY"`, `contactWebsite = indexUrl`, `contactDiscord = null`, `isLegacy = false`, `extensionListUrl = null`. Used only by the migration `TrustExtensionRepositoryMigration` (version 7f, `app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt:13-31`). The stored row has `isLegacy = false` but points at a legacy `repo.json` URL, so `getExtensions` will fail for it (section 7.4 branch 2).
- `refreshAll()` (SREPO:38-52): reads raw rows with `getAll()`, then for each row `service.fetch(store.index_url)`, `mapCatching { upsert(it) }`, failures logged. A failing read of the row list is caught by the outer `catch` and logged.
- `getAll()`, `getAllAsFlow()` (`subscribeToList`), `getCountAsFlow()` (`subscribeToOne { getCount() }`) (SREPO:90-100).
- `remove(indexUrl)` (SREPO:102-104): deletes only the row.
- `upsert` (SREPO:54-67) writes all eight columns. SQL: see section 7.10.

**OBSERVED consequence of remove:** installed extensions keep their `store` reference. On the next `findAvailableExtensions`, extensions that no store lists become obsolete (5.7). Trust for those extensions (6.2) is re-evaluated only at the next load, when the removed store's key is gone.

### 7.10 SQL (OBSERVED, SQ:1-44)
- `CREATE TABLE extension_store(index_url TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL, badge_label TEXT NOT NULL, signing_key TEXT NOT NULL, contact_website TEXT NOT NULL, contact_discord TEXT, is_legacy INTEGER AS Boolean NOT NULL, extension_list_url TEXT)` (SQ:3-12).
- `get`: `SELECT * FROM extension_store WHERE index_url = :indexUrl` (SQ:14-17).
- `getAll`: `SELECT * FROM extension_store` (SQ:19-21). **No ORDER BY.** Order is the rowid order in practice.
- `getCount`: `SELECT COUNT(*) FROM extension_store` (SQ:23-25).
- `upsert`: `INSERT ... VALUES (...) ON CONFLICT(index_url) DO UPDATE SET name = :name, badge_label = :badgeLabel, signing_key =: signingKey, contact_website = :contactWebsite, contact_discord = :contactDiscord, is_legacy = :isLegacy, extension_list_url = :extensionListUrl WHERE index_url = :indexUrl` (SQ:27-40). The text `signing_key =: signingKey` (SQ:35) is written without a space. It parses as `signing_key = :signingKey`.
- `delete`: `DELETE FROM extension_store WHERE index_url = :indexUrl` (SQ:42-44).

### 7.11 Legacy detection order summary (OBSERVED)
| First byte | Gzip-wrapped? | Parsed as |
|---|---|---|
| `0x5B` `[` | yes (store fetch only) | legacy extension list at `index.min.json`, then `repo.json` |
| `0x7B` `{` | yes | `NetworkLegacyAnimeExtensionRepo` if decodable, else `NetworkAnimeExtensionStore` JSON |
| other | yes | `NetworkAnimeExtensionStore` protobuf |

For `getExtensions` (7.4), the detection is simpler: `{` is JSON, anything else is protobuf.

---

## 8. Installer (OBSERVED, INST)

### 8.1 State (INST:47-62)
- `downloadManager = context.getSystemService<DownloadManager>()!!` (INST:47). Replaced in Rust (`p1-rust-design.md` section 6).
- `downloadReceiver = DownloadCompletionReceiver()` (INST:52).
- `activeDownloads: HashMap<String, Long>`: package name → DownloadManager id (INST:58).
- `downloadsStateFlows: HashMap<Long, MutableStateFlow<InstallStep>>` (INST:60).
- `extensionInstaller = Injekt.get<BasePreferences>().extensionInstaller()` (INST:62). The value is read in `installApk`.

### 8.2 `downloadAndInstall(url, extension): Flow<InstallStep>` (OBSERVED, INST:71-120)
1. `pkgName = extension.pkgName` (INST:72).
2. If `activeDownloads[pkgName]` exists: `deleteDownload(pkgName)` (INST:74-77).
3. `downloadReceiver.register()` (INST:80). Idempotent via `isRegistered`. Registered `RECEIVER_EXPORTED` for `ACTION_DOWNLOAD_COMPLETE` (INST:272-278).
4. `downloadUri = url.toUri()`. Request: `setTitle(extension.name)`, `setMimeType("application/vnd.android.package-archive")`, `setDestinationInExternalFilesDir(context, DIRECTORY_DOWNLOADS, downloadUri.lastPathSegment)`, `setNotificationVisibility(VISIBLE_NOTIFY_COMPLETED)` (INST:82-91).
5. `id = downloadManager.enqueue(request)`; `activeDownloads[pkgName] = id`; `downloadsStateFlows[id] = MutableStateFlow(InstallStep.Pending)` (INST:93-97).
6. `pollStatusFlow = downloadStatusFlow(id).mapNotNull { STATUS_PENDING → Pending; STATUS_RUNNING → Downloading; else → null }` (INST:100-107).
7. Return `merge(downloadStateFlow, pollStatusFlow).transformWhile { emit(it); !it.isCompleted() }.onCompletion { withUIContext { deleteDownload(pkgName) } }` (INST:109-119).

### 8.3 Poll (OBSERVED, INST:128-151)
- Query by id. Loop: read `COLUMN_STATUS`. If the cursor has no row, `return@flow` (INST:134). The poll completes **without emitting**.
- Emit the status, then stop on `STATUS_SUCCESSFUL` or `STATUS_FAILED` (INST:138-145). Otherwise `delay(1.seconds)` (INST:147) and repeat.
- `distinctUntilChanged()` (INST:151).

**OBSERVED:** the poll never emits `Installed` or `Error`. Those come only from the broadcast path (8.4) and installer callbacks.

**OBSERVED defect (hang when the download row disappears):** `merge` completes only when every input completes. `downloadStateFlow` is a `MutableStateFlow` and never completes. So once the poll completes silently, the merged flow stays open. It only ends when the collector is cancelled.

### 8.4 `DownloadCompletionReceiver` (OBSERVED, INST:262-320)
- `onReceive`: `id = intent.getLongExtra(EXTRA_DOWNLOAD_ID, 0)`. `if (id !in activeDownloads.values) return` (INST:295-298).
- `uri = downloadManager.getUriForDownloadedFile(id)`. Null → log `"Couldn't locate downloaded APK"` at ERROR, `updateInstallStep(id, Error)`, return (INST:300-307).
- Query by id. If a row exists: `localUri = COLUMN_LOCAL_URI.removePrefix("file://")`, then `installApk(id, File(localUri).getUriCompat(context))` (INST:309-317). No row → nothing happens, silently.

### 8.5 `installApk(downloadId, uri)` (OBSERVED, INST:158-205)
Dispatch by `extensionInstaller.get()`:
- `LEGACY` (INST:160-169): start `ACT` with data uri, MIME `application/vnd.android.package-archive`, extra `EXTRA_DOWNLOAD_ID`, flags `FLAG_ACTIVITY_NEW_TASK | FLAG_GRANT_READ_URI_PERMISSION`.
- `PRIVATE` (INST:170-198):
  a. `tempFile = File(context.cacheDir, "temp_$downloadId")` (INST:172).
  b. If `tempFile.exists() && !tempFile.delete()`: `updateInstallStep(downloadId, Error)` and **return** (INST:174-178). No cleanup on this path.
  c. In `try`: copy `contentResolver.openInputStream(uri)` into `tempFile` (INST:181-185). Then `if (AnimeExtensionLoader.installPrivateExtensionFile(context, tempFile)) Installed else Error` (INST:187-191).
  d. `catch (e: Exception)`: log `"Failed to read downloaded extension file."` at ERROR, `Error` (INST:192-195).
  e. After the branches: `tempFile.delete()` (INST:197).
  **OBSERVED:** the `PRIVATE` path never sets `Installing`. The step goes from `Downloading` to `Installed` or `Error`.
- Anything else, that is `PACKAGEINSTALLER` or `SHIZUKU` (INST:199-203): `ContextCompat.startForegroundService(context, SVC.getIntent(context, downloadId, uri, installer))`.

### 8.6 `cancelInstall(pkgName)` (OBSERVED, INST:210-214)
1. `downloadId = activeDownloads.remove(pkgName) ?: return`.
2. `downloadManager.remove(downloadId)`. This also deletes the downloaded file.
3. `InstallerAnime.cancelInstallQueue(context, downloadId)` (IQ:164-168): local broadcast `"InstallerAnime.action.CANCEL_QUEUE"` with extra `"InstallerAnime.extra.DOWNLOAD_ID"`.

**OBSERVED:** nothing sets the step to `Idle` here. `downloadsStateFlows[downloadId]` is not removed. Consequences:
- If the entry is **queued in the service** (PACKAGEINSTALLER or SHIZUKU), IQ `cancelQueue` sets `Idle` (IQ:128-139). The flow completes.
- If the download is **still in progress**, no queue entry exists. `cancelQueue` returns early (IQ:130). The poll completes silently (8.3). The merged flow hangs with `Downloading` (8.3). The UI row stays in progress until the screen model is cleared.
- **OBSERVED consequence on cleanup:** when the hung collector is finally cancelled, its `onCompletion` calls `deleteDownload(pkgName)` (INST:248-257). That removes **any newer** download for the same package from `activeDownloads`, and it calls `downloadManager.remove` on the newer id.

### 8.7 `uninstallApk(pkgName)` (OBSERVED, INST:221-231)
- If `context.isPackageInstalled(pkgName)` (system package exists): `ACTION_UNINSTALL_PACKAGE` for `"package:$pkgName"` with `FLAG_ACTIVITY_NEW_TASK` (INST:222-226).
- Else: `AnimeExtensionLoader.uninstallPrivateExtension(context, pkgName)` and `RCV.notifyRemoved(context, pkgName)` (INST:227-230). The manager then removes the entry (5.11).

### 8.8 `updateInstallStep(downloadId, step)` (OBSERVED, INST:239-241)
`downloadsStateFlows[downloadId]?.let { it.value = step }`. A silent no-op for unknown ids.

### 8.9 `deleteDownload(pkgName)` (OBSERVED, INST:248-257)
`downloadId = activeDownloads.remove(pkgName)`. If non-null: `downloadManager.remove(downloadId)` and `downloadsStateFlows.remove(downloadId)`. If `activeDownloads` is then empty: `downloadReceiver.unregister()`.

**OBSERVED:** `DownloadManager.remove` deletes the downloaded APK. The flow's `onCompletion` calls this, so the file is removed as soon as the flow completes.

### 8.10 Step values by install path (OBSERVED, from 8.2-8.5 and 9)
| Path | Pending | Downloading | Installing | Installed | Error | Idle |
|---|---|---|---|---|---|---|
| `PRIVATE` | initial | poll RUNNING | never set | after copy and install succeed | download missing, copy error, install failed, temp delete failed | never |
| `LEGACY` | initial | poll RUNNING | never set | activity `RESULT_OK` | other result code | activity `RESULT_CANCELED` |
| `PACKAGEINSTALLER` (dropped) | initial | poll RUNNING | `processEntry` | `STATUS_SUCCESS` | other status, exception | `STATUS_FAILURE_ABORTED`, cancel of queued entry |
| `SHIZUKU` (dropped) | initial | poll RUNNING | `processEntry` | `pm install-commit` exit 0 | exception, non-zero exit | cancel of queued entry |

---

## 9. Install queue (OBSERVED, IQ:20-170)

Used only by `PACKAGEINSTALLER` and `SHIZUKU`.
- `waitingInstall = AtomicReference<Entry>(null)` (IQ:24). `queue = Collections.synchronizedList(mutableListOf<Entry>())` (IQ:25).
- `data class Entry(downloadId: Long, uri: Uri)` (IQ:148).
- `abstract var ready: Boolean` (IQ:39). When false, `checkQueue` does nothing.
- `addToQueue(downloadId, uri)`: append, then `checkQueue()` (IQ:47-50).
- `processEntry(entry)`: `open`, `@CallSuper`, calls `extensionManager.setInstalling(entry.downloadId)` (IQ:60-62).
- `continueQueue(resultStep)`: `completed = waitingInstall.getAndSet(null)`; if non-null, `updateInstallStep(completed.downloadId, resultStep)` and `checkQueue()` (IQ:81-87).
- `checkQueue()` (IQ:95-108): if not ready, return. If the queue is empty, `service.stopSelf()` and return. Else `nextEntry = queue.first()`; if `waitingInstall.compareAndSet(null, nextEntry)`: `queue.removeAt(0)` and `processEntry(nextEntry)`.
- `cancelQueue(downloadId)` (IQ:128-140): `waiting = waitingInstall.get()`; `toCancel = queue.find { it.downloadId == downloadId } ?: waiting ?: return`. If `cancelEntry(toCancel)`: `queue.remove(toCancel)`; if `waiting == toCancel`, `waitingInstall.set(null)` and `checkQueue()`; then `updateInstallStep(downloadId, Idle)`.
- `onDestroy()` (IQ:114-119): unregisters the cancel receiver; sets every **queued** entry to `Error`; clears the queue; `waitingInstall.set(null)`. **OBSERVED:** the active entry's step is not updated.
- `cancelEntry` default returns `true` (IQ:70-72). `ShizukuInstallerAnime.cancelEntry` returns `getActiveEntry() != entry` (SHZ:82). `PackageInstallerInstallerAnime.cancelEntry` abandons the active session and returns `false` (PKGI:92-100).
- A cancel receiver is registered on the local broadcast manager in `init` (IQ:150-153).

**OBSERVED:** `checkQueue`'s read-then-remove is not atomic as a pair (IQ:103-105). The CAS guards only `waitingInstall`.

---

## 10. Installer variants (OBSERVED, dropped on Linux, behaviour recorded for the drop record)

### 10.1 `PackageInstallerInstallerAnime` (PKGI:19-117)
- `packageInstaller = service.packageManager.packageInstaller` (PKGI:21).
- Receiver for `"PackageInstallerInstaller.INSTALL_ACTION"` (PKGI:117), registered `RECEIVER_EXPORTED` (PKGI:107-114). Statuses: `STATUS_PENDING_USER_ACTION` → start the confirmation `EXTRA_INTENT` with `FLAG_ACTIVITY_NEW_TASK`, or `Error` if missing (PKGI:25-35); `STATUS_FAILURE_ABORTED` → `Idle`; `STATUS_SUCCESS` → `Installed`; else → `Error` (PKGI:36-41).
- `processEntry` (PKGI:50-90): `SessionParams(MODE_FULL_INSTALL)`; on S and later `setRequireUserAction(USER_ACTION_NOT_REQUIRED)`; `createSession`; `getUriSize(uri)` (null → `IllegalStateException`); `setSize`; `openSession`; `openWrite(downloadId.toString(), 0, fileSize)`; copy; `fsync`; `commit(PendingIntent.getBroadcast(...))` with `FLAG_MUTABLE` on S and later. On exception: `abandonSession`, `continueQueue(Error)`.

### 10.2 `ShizukuInstallerAnime` (SHZ:21-124)
- `ready` is `Shizuku.pingBinder()` and `checkSelfPermission() == PERMISSION_GRANTED` (SHZ:104-119). If the binder is missing, `toast(MR.strings.ext_installer_shizuku_stopped)` and `stopSelf()`. If permission is missing, request it with code `14045` (SHZ:123).
- Commands run through `Shizuku.newProcess(arrayOf("sh", "-c", command), null, null)` (SHZ:93):
  - `pm install-create --user $userId -r -i ${service.packageName} -S $size`, session id parsed with `(?<=\[).+?(?=])` (SHZ:124, 54-57).
  - `pm install-write -S $size $sessionId base -` with stdin (SHZ:59-62).
  - `pm install-commit $sessionId` (SHZ:64-67).
  - On failure, `pm install-abandon $sessionId` (SHZ:73-75).
- `userId = Process.myUserHandle().hashCode()` (SHZ:53).

### 10.3 `AnimeExtensionInstallActivity` (ACT:18-79)
- `startActivityForResult(ACTION_INSTALL_PACKAGE with EXTRA_RETURN_RESULT true, FLAG_GRANT_READ_URI_PERMISSION, request code 500)` (ACT:29-40).
- MIUI workaround (ACT:34-37): if `hasMiuiPackageInstaller`, `ignoreResult = true` and `ignoreUntil = System.nanoTime() + 1.seconds.inWholeNanoseconds`. A result inside the window sets `hasIgnoredResult = true` and returns (ACT:49-52). `onStart` then calls `checkInstallationResult(RESULT_CANCELED)` and finishes (ACT:59-65).
- Mapping (ACT:70-74): `RESULT_OK` → `Installed`; `RESULT_CANCELED` → `Idle`; other → `Error`.
- A failed start shows `toast(error.message)` (ACT:41-45).

### 10.4 `AnimeExtensionInstallService` (SVC:22-88)
- Foreground notification in `onCreate` (SVC:26-36): channel `Notifications.CHANNEL_EXTENSIONS_UPDATE`, small icon `R.drawable.ic_ani`, `setAutoCancel(false)`, `setOngoing(true)`, `setShowWhen(false)`, title `MR.strings.ext_install_service_notif` (`"Installing extension…"`), `setProgress(100, 100, true)`, id `Notifications.ID_EXTENSION_INSTALLER` (`-402`). `startForeground` with type `shortService` (manifest, `app/src/main/AndroidManifest.xml:272-274`).
- `onStartCommand` (SVC:38-64): needs `data`, `EXTRA_DOWNLOAD_ID`, and `EXTRA_INSTALLER` (key `"EXTRA_INSTALLER"`). Missing → `stopSelf()` and `START_NOT_STICKY`. Installer chosen by value: `PACKAGEINSTALLER` → `PackageInstallerInstallerAnime`, `SHIZUKU` → `ShizukuInstallerAnime`, else log `"Not implemented for installer $installerUsed"` and stop.
- `installer!!.addToQueue(id, uri)` (SVC:62).
- `onDestroy` (SVC:66-69) calls `installer?.onDestroy()`.

### 10.5 `ExtensionInstaller` preference values (OBSERVED, `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36`)
`LEGACY(MR.strings.ext_installer_legacy, requiresSystemPermission = true)`, `PACKAGEINSTALLER(MR.strings.ext_installer_packageinstaller, true)`, `SHIZUKU(MR.strings.ext_installer_shizuku, false)`, `PRIVATE(MR.strings.ext_installer_private, false)`. Key `"extension_installer"` (`ExtensionInstallerPreference.kt:19`). Default is `LEGACY` when MIUI is present, else `PACKAGEINSTALLER` (`ExtensionInstallerPreference.kt:29-33`). `check()` demotes `PACKAGEINSTALLER` to `LEGACY` on MIUI and `SHIZUKU` to the default when Shizuku is missing (`ExtensionInstallerPreference.kt:35-46`). The settings UI hides `PRIVATE` in release builds (`SettingsAdvancedScreen.kt:443-453`).

---

## 11. Update check API (OBSERVED, API:18-79)

### 11.1 `checkForUpdates(context, fromAvailableExtensionList = false): List<Installed>?`
1. **Gate** (API:39-43): only when `fromAvailableExtensionList` is true, `if (Instant.now().toEpochMilli() < lastExtCheck.get() + 1.days.inWholeMilliseconds) return null`. `lastExtCheck` = `preferenceStore.getLong("last_ext_check", 0)` (API:26-28). `1.days` = 86,400,000 ms.
   - **OBSERVED:** the only caller is `MainActivity` (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:398`), which passes the default `false`. The gate never applies, so the gate is dead code in practice.
2. `updateExtensionStores()` (API:46): refresh all store rows (7.9). This runs for both branches.
3. `extensions = if (fromAvailableExtensionList) animeExtensionManager.availableExtensionsFlow.value else findExtensions().also { lastExtCheck.set(Instant.now().toEpochMilli()) }` (API:48-52). The second `findExtensions()` is a **second full network fetch** of every store. The first one came from step 2.
4. `installedExtensions = AnimeExtensionLoader.loadExtensions(context).filterIsInstance<Success>().map { it.extension }` (API:54-56). A full reload from disk, not the manager's state.
5. For each installed extension (API:59-69): `availableExt = extensions.find { it.pkgName == installedExt.pkgName } ?: continue`. `hasUpdatedVer = availableExt.versionCode > installedExt.versionCode`; `hasUpdatedLib = availableExt.libVersion > installedExt.libVersion`; `hasUpdate = hasUpdatedVer || hasUpdatedLib`. Collect where true.
6. If the list is non-empty, `ExtensionUpdateNotifier(context).promptUpdates(names = list.map { it.name }, anime = true)` (API:71-76).
7. Return the list. The result is non-null unless gated.

**OBSERVED:** `checkForUpdates` never writes `animeext_updates_count` and never updates manager state. The count and the flags change only inside the manager (5.7, 5.11, 5.12).

**OBSERVED:** the key `"last_ext_check"` is also used by the manga API (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt:39`). Both classes read and write the same raw key.

**OBSERVED:** `updateExtensionStores()` is not wrapped in a try, so a failure there propagates to `MainActivity`, which catches `Exception` and logs it (`MainActivity.kt:399-404`).

### 11.2 Notification (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt:19-49`)
- Id `Notifications.ID_UPDATES_TO_EXTS = -401` (`app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:93`). Channel `"ext_apk_update_channel"` (Notifications.kt:92), importance default (Notifications.kt:191).
- Title `MR.plurals.update_check_notification_ext_updates` with quantity `names.size`: `one` → `"Extension update available"`, `other` → `"%d extension updates available"` (`i18n/src/commonMain/moko-resources/base/plurals.xml:89-92`).
- If `!securityPreferences.hideNotificationContent()`: content text and BigText are `names.joinToString(", ")`.
- Small icon `R.drawable.ic_extension_24dp`. Content intent: `NotificationReceiver.openAnimeExtensionsPendingActivity(context)` when `anime = true`. `setAutoCancel(true)`.
- `dismiss()` cancels id `-401`.

**OBSERVED:** the anime and manga notifications share id `-401`. The newer one replaces the older one.

### 11.3 Trigger timing (OBSERVED)
Runs once per `MainActivity` composition (`MainActivity.kt:396-404`), which is app start.

---

## 12. Store management actions (OBSERVED, SMODEL, SCREEN)

### 12.1 `createRepo(baseUrl)` (SMODEL:58-92)
1. Set dialog `processing = true` on `Create` or `Confirm`.
2. `addExtensionStore(baseUrl)`.
3. On success: `extensionManager.findAvailableExtensions()`, then `dismissDialog()`.
4. On failure: `processing = false`, `errorMessage = throwable.message ?: "unknown error"` on `Create` or `Confirm` (SMODEL:75-89). The raw exception text is shown.

### 12.2 `refreshRepos()` (SMODEL:97-105)
Runs only in `Success` state. Calls `updateExtensionStores()` only. **OBSERVED:** `findAvailableExtensions()` is not called, so the Browse list does not change after the refresh button.

### 12.3 `deleteRepo(baseUrl)` (SMODEL:110-115)
`removeExtensionStore(baseUrl)`, then `extensionManager.findAvailableExtensions()`.

### 12.4 `addFromDeeplink(storeIndexUrl)` (SMODEL:117-126)
Sets `Confirm(url = storeIndexUrl, alreadyExists = stores.any { it.indexUrl == storeIndexUrl })`. The comparison is exact string equality against the **stored** index URLs. A typed legacy `index.min.json` URL is not detected as existing, because the stored URL was rewritten to `repo.json`.

### 12.5 Entry points (OBSERVED)
- Settings: `SettingsBrowseScreen.kt:66` pushes `AnimeExtensionStoresScreen()`.
- Browse tab: `AnimeExtensionsScreen.kt:116` and `AnimeExtensionsTab.kt:53` push it.
- Deep link (`app/src/main/AndroidManifest.xml:78-94`): scheme `aniyomi`, host `add-repo` or `extension-store`. `MainActivity.kt:553-560` reads query parameter `url` and calls `popUntilRoot()` then pushes `AnimeExtensionStoresScreen(repoUrl)`. `MainActivity.kt:587` defines `isAddAnimeExtensionStoreIntent()`.

---

## 13. Browse bucket, languages and source rows (OBSERVED, BYTYPE, BYLANG, BYSRC)

### 13.1 `GetAnimeExtensionsByType.subscribe(): Flow<AnimeExtensions>` (BYTYPE:15-56)
- `showNsfwSources = preferences.showNsfwSource().get()` is read **once** at `subscribe()` (BYTYPE:16). Changing the setting needs a new subscription.
- Inputs combined: `enabledLanguages.changes()`, `installedExtensionsFlow`, `untrustedExtensionsFlow`, `availableExtensionsFlow` (BYTYPE:18-23).
- **Installed** (BYTYPE:24-30):
  1. Filter `showNsfwSources || !it.isNsfw`.
  2. Sort `compareBy<Installed> { !it.isObsolete }.thenBy(String.CASE_INSENSITIVE_ORDER) { it.name }`. Because `false < true`, obsolete items sort **first**.
  3. `partition { it.hasUpdate }` gives `(updates, installed)`.
- **Untrusted** (BYTYPE:32-33): sort by `name` case-insensitively. **No NSFW filter.**
- **Available** (BYTYPE:35-52):
  1. Filter raw: `_installed.none { pkgName == ext.pkgName }`, `_untrusted.none { pkgName == ext.pkgName }`, `showNsfwSources || !ext.isNsfw` (BYTYPE:36-40).
  2. `flatMap`: `ext.sources.filter { it.lang in enabledLanguages }.map { ext.copy(name = it.name, lang = it.lang, pkgName = "${ext.pkgName}-${it.id}", sources = listOf(it)) }` (BYTYPE:41-50).
  3. Sort by `name` case-insensitively (BYTYPE:52).
- Output `AnimeExtensions(updates, installed, available, untrusted)` (BYTYPE:54). Data class at `app/src/main/java/eu/kanade/domain/extension/anime/model/AnimeExtensions.kt:5-10`.

**OBSERVED defect (empty-source extension disappears):** an available extension with **zero** sources produces an empty `flatMap`. It is missing from the Browse list. The manga side includes such an extension as a single row. (Area:18 section 11.1.)

**OBSERVED defect (split rows share no key):** an available extension with two sources in an enabled language becomes two rows with keys `"<pkg>-<id1>"` and `"<pkg>-<id2>"`. Download state (`currentDownloads`) and the install step are keyed by `pkgName` (`AnimeExtensionsScreenModel.kt:173`, `187-193`). Installing one row does not show progress on the other, and each row starts its own download of the same APK.

**Proposal:** key download and install state by the APK package name. Keep the split rows for display only.

### 13.2 `GetAnimeExtensionLanguages.subscribe(): Flow<List<String>>` (BYLANG:13-27)
- `combine(preferences.enabledLanguages().changes(), extensionManager.availableExtensionsFlow)`.
- `availableExtensions.flatMap { ext -> ext.sources.map { it.lang } }.distinct().sortedWith(compareBy<String> { it !in enabledLanguage }.then(LocaleHelper.comparator))` (BYLANG:18-25).
- `LocaleHelper.comparator` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:17-25`): `"all"` sorts first. Otherwise compare the localised display names (`getLocalizedDisplayName`).

**OBSERVED:** only source languages count. An extension with zero sources contributes no language. The `Available.lang` field is not used here.

### 13.3 `GetAnimeExtensionSources.subscribe(extension: Installed): Flow<List<AnimeExtensionSourceItem>>` (BYSRC:9-37)
- `isMultiSource = extension.sources.size > 1`.
- `isMultiLangSingleSource = isMultiSource && extension.sources.map { it.name }.distinct().size == 1`.
- Re-emits on every change of `preferences.disabledAnimeSources()` (`hidden_anime_catalogues`).
- Per source: `enabled = id.toString() !in disabledSources` (BYSRC:19); `labelAsName = isMultiSource && !isMultiLangSingleSource` (BYSRC:26).
- Label rule in plain words: a single-source extension is labelled by language. A multi-source extension whose sources all share one name is labelled by language. A multi-source extension whose sources have different names is labelled by name.
- `AnimeExtensionSourceItem(source: AnimeSource, enabled: Boolean, labelAsName: Boolean)` (BYSRC:33-37).

---

## 14. Concurrency and threading (OBSERVED)

| Component | Thread model | Source |
|---|---|---|
| Manager `scope` | `SupervisorJob()`, Dispatchers.Default | MGR:51 |
| Manager constructor | runs `loadExtensions` synchronously on the calling thread | MGR:78, LDR:173 |
| Loader | `runBlocking` on the caller, sequential in practice | LDR:173-178 |
| API `findExtensions` | `withIOContext` | API:31 |
| API `checkForUpdates` | caller dispatcher (Main, from `LaunchedEffect`) | MainActivity.kt:398 |
| Repository | `handler.awaitList` / `await` (database dispatcher) | SREPO |
| Store service | suspend network calls | SSVC |
| Installer | `delay` for poll on the collector's dispatcher; `onCompletion` uses `withUIContext` | INST:115 |
| Receivers | `SupervisorJob()` scope, default dispatcher | RCV:26 |
| Shizuku | `Dispatchers.IO`, blocking `exec` inside the coroutine | SHZ:23, 91-100 |
| Icon cache | `iconMap` is a plain `mutableMapOf`, not thread-safe | MGR:66 |
| Screen model | `screenModelScope.launchIO` | SMODEL:38, 59, 101, 111 |

**Proposal (Rust):** all extension state is owned by the GTK main thread (`Rc<RefCell<State>>`). Network, filesystem, and hashing work runs on a tokio runtime. Results return to the main thread through `async_channel`. Install work runs one at a time (`p1-rust-design.md` section 6). Details are in `p1-rust-design.md`.

---

## 15. Notifications and foreground work (OBSERVED)

| Item | Id | Channel | Source |
|---|---|---|---|
| Extension updates | `-401` | `ext_apk_update_channel` | ExtensionUpdateNotifier.kt, Notifications.kt:92-93 |
| Installer foreground service | `-402` | `ext_apk_update_channel` | SVC:26-36, Notifications.kt:94 |

The installer notification (`"Installing extension…"`) is an ongoing, indeterminate notification while the foreground service runs. It exists only for `PACKAGEINSTALLER` and `SHIZUKU`. The `DownloadManager` also posts its own download notification (`VISIBLE_NOTIFY_COMPLETED`, INST:91). That one is Android-only.

---

## 16. Interactions with other areas

| Area | Interaction | Where in this file |
|---|---|---|
| area:02-preferences-system | Keys `source_languages`, `show_nsfw_source`, `hidden_anime_catalogues`, `trusted_extensions`, `animeext_updates_count`, `last_ext_check`, `extension_installer`, `anime_extension_repos` | sections 2, 5, 6, 10.5, 11 |
| area:04-db-anime-schema-migrations | Table `extension_store` in the anime database | section 7.10 |
| area:11-app-migrations | `TrustExtensionRepositoryMigration` version 7 inserts legacy repos | section 7.9 |
| area:16-source-api-anime | `AnimeSource` interface, `StubAnimeSource`, `AnimeSourceFactory` (dropped) | sections 2.9, 5.4 |
| area:18-extensions-manga | Same store protocol family, same trust model, same install queue. Manga uses `.kuroext` packages and a 1.4 to 1.5 lib range | sections 2.2, 6, 8, 9; `p1-topic-install-and-trust.md` |
| area:20-extension-strategy-redesign | Chooses the package mechanism for Kurobayomi | sections 2.9, 13 |
| area:21-backup-format-and-create | Anime backup creator and the store field numbers | section 7.2 in data file; `p1-data-and-config.md` |
| area:22-backup-restore | Store restorer and error text | `p1-data-and-config.md` section 8 |
| area:26-library-update-jobs | Scheduling of update checks (Kurobayomi runs them on start and on a timer, see design) | section 11 |
| area:35-data-misc-updater-notification-export | Notification channel plumbing | section 11.2 |
| area:36-ui-shell-navigation-security-webview | Deep link routing `aniyomi://add-repo` and `extension-store` | section 12.5 |
| area:40-ui-browse-manga and area:41-ui-browse-anime | Extension list screen, install buttons, trust dialog, source toggles | sections 6.4, 13 |
| area:34-images-covers-cache | Extension icons | section 5.3 |
| area:47-settings-screens-general | Extension installer and revoke-trust settings rows | sections 6.3, 10.5 |
| area:12-network-stack | HTTP client, `awaitSuccess`, gzip | section 7 |
| area:13-core-util-storage-archive-torrent | `Hash`, `copyAndSetReadOnlyTo`, `truncateCenter`, archive helpers | sections 2.11, 3 |

---

## 17. Behaviour summary for Linux

**Proposal (Linux):** the Android mechanisms are replaced as follows. The full design is in `p1-rust-design.md`.
- A Kurobayomi anime extension is a `.kuroext` file. It is a zip archive with `manifest.json` and `source.wasm`. It is installed into `$XDG_DATA_HOME/kurobayomi/extensions/anime/<pkg>.kuroext`. The archive is verified before it is copied (`p1-topic-install-and-trust.md`).
- Aniyomi APK entries from the same store protocol are listed with kind `ApkOnly`, badge "Not runnable on Linux", and no install action (`p1-topic-install-and-trust.md` section 5).
- Discovery reads manifests only. It never runs package code.
- Trust keeps the `trusted_extensions` format. The digest is the SHA-256 of the signer's ed25519 public key (area:18-extensions-manga section 16.4).
- The store protocol keeps the JSON form and the Kurobayomi optional `resources.kuroext` block. Protobuf and legacy formats are kept for reading (P2).
- Update checks run on start and once a day. Updates are shown as a GNotification. Counts are derived in memory.
- Store rows keep the same columns. Downloads are one reqwest stream with progress. There are no system broadcasts. Events are direct method calls.
