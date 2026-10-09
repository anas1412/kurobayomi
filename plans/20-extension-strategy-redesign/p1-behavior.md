# 20-extension-strategy-redesign: behavior spec

All OBSERVED statements cite Aniyomi source at `/home/blackbox/aniyomi-src` (repo-relative paths). Statements marked **Proposal:** are the Kurobayomi design.

## B1. Extension discovery (manga and anime loaders)

OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt`:
1. Two extension kinds exist: a *shared* package installed to the system (feature `tachiyomi.extension`), and a *private* file at `<filesDir>/exts/<pkgName>.ext`.
2. `loadMangaExtensions` builds `sharedExtPkgs` from `getInstalledPackages`, filtered by `isPackageAnExtension` (requested feature named `tachiyomi.extension`).
3. `privateExtPkgs` lists `<filesDir>/exts`, keeps files with extension `ext`, sets them read-only if writable, and loads them with `getPackageArchiveInfo`.
4. `extPkgs = (shared + private).distinctBy(pkgName)`, then for each shared package `selectExtensionPackage(shared, privateWithSamePkg)`.
5. `selectExtensionPackage`: if only one side exists it wins; if both exist, shared wins when `versionCode(shared) >= versionCode(private)`, else private.
6. Every selected package is loaded concurrently (`runBlocking { awaitAll }`), and the result list keeps one `MangaLoadResult` per package.
7. Anime differs (`app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt`): feature `tachiyomi.xextension`, a single load path (no private directory in the function body), and manifest metadata keys `aniyomix.*` in addition to the `tachiyomi.xextension.*` keys.

**Proposal:** one loader that scans `<data>/extensions/<pkg>/` directories. Each directory holds `manifest.toml` plus `component.wasm` (see p1-topic-wasm-component-design.md). The shared-vs-private duality disappears because there is one location. Version selection is kept: when the same `pkg` exists twice (for example a user drop-in and an official build), the higher `version_code` wins, ties go to the official repo copy.

### B1.1 Load checks, in the exact order Aniyomi applies them (manga)

| Step | Check | Failure result | Source |
|---|---|---|---|
| 1 | `versionName` non-empty | `Error` with warn log "Missing versionName" | MangaExtensionLoader.kt loadMangaExtension |
| 2 | `libVersion = versionName.substringBeforeLast('.').toDouble()` non-null and in `[LIB_VERSION_MIN=1.4, LIB_VERSION_MAX=1.5]` | `Error` | same |
| 3 | signatures non-empty | `Error` "isn't signed" | same |
| 4 | `trustExtension.isTrusted(pkgInfo, signatures)` | `Untrusted(extension)` where `signatureHash = signatures.last()` | same |
| 5 | `isNsfw = metaData.getInt("tachiyomi.extension.nsfw") == 1`; if NSFW and `showNsfwSource` off | `Error` | same |
| 6 | class loader creation | `Error` | same |
| 7 | each class in `metaData["tachiyomi.extension.class"]` split on `;`, trimmed; leading `.` prefixed with packageName; instantiate; `MangaSource` -> one source, `SourceFactory` -> `createSources()` | `Error` on any throwable | same |

Anime order (AnimeExtensionLoader.kt, `loadExtension`): name from `aniyomix.name` else label minus prefix `"Aniyomi: "`; libVersion from `aniyomix.extensionLib` (int, 0 means unset) else `versionName` minus last part; `libVersion !in [14.0, 16.0, 17.0]` -> `Error`; signature and trust checks same as manga; `isNsfw = metaData["aniyomix.contentWarning"] > 0 || metaData["tachiyomi.xextension.nsfw"] == 1`; `isTorrent = metaData["aniyomix.torrent"] (bool) || metaData["tachiyomi.xextension.torrent"] == 1`.

### B1.2 Language of an extension

OBSERVED (MangaExtensionLoader.kt): the set of `lang` values of `CatalogueSource`s. size 0 -> `""`, size 1 -> that lang, size >1 -> `"all"`. Anime keeps the raw set of langs for its sources (AnimeExtensionLoader.kt, `sources.map { it.lang }.toSet()`).

**Proposal:** same rule, applied to Rust sources.

### B1.3 Error and logging

- Every load failure is logged and yields a `LoadResult::Error`; the rest of the loading continues (`awaitAll` gets one result per package).
- The Kotlin code catches `LinkageError` once, retries with `PathClassLoader` (fallback), then returns `Error`. **Proposal:** no fallback needed; a WASM instantiate error is final for that package and shown as `Error` with the message in the extension detail view.

## B2. Installed / available / untrusted registry

OBSERVED (MangaExtensionManager.kt, AnimeExtensionManager.kt):
- Three state maps keyed by `pkgName`: `installedExtensionsMapFlow`, `availableExtensionsMapFlow`, `untrustedExtensionsMapFlow`. Each exposed as a `StateFlow<List<T>>` with `SharingStarted.Lazily`.
- `init` calls `initExtensions()` (load all, split into Success and Untrusted) and registers `InstallationListener`.
- `_isInitialized` flips to true after load.
- `findAvailableExtensions()`: calls the repo API; on exception logs and shows toast `MR.strings.extension_api_error`; then `enableAdditionalSubLanguages`, sets availability map, `updatedInstalledExtensionsStatuses`, rebuilds the stub map.
- `updatedInstalledExtensionsStatuses(available)`:
  - if available empty: set `mangaExtensionUpdatesCount` to 0 and return;
  - for every installed `ext`: if no available with same pkg and not already obsolete -> `isObsolete = true`; if available exists -> `hasUpdate = ext.updateExists(avail)`, `repoUrl = avail.repoUrl`;
  - `updatePendingUpdatesCount()` at the end.
- `updateExists(avail)`: `avail.versionCode > versionCode || avail.libVersion > libVersion`.
- `updatePendingUpdatesCount()`: count `hasUpdate`, write preference `mangaExtensionUpdatesCount`, dismiss the notification when zero.
- `trust(untrusted)`: calls `trustExtension.trust(pkg, versionCode, signatureHash)`, removes the entry from the untrusted map, reloads the package from disk and registers it as installed when it loads Success.
- `uninstallExtension`: delegates to installer. For a private extension the installer deletes the file directly and calls `notifyRemoved`.
- `InstallationListener`:
  - `onExtensionInstalled`: register with `withUpdateCheck()`, update count;
  - `onExtensionUpdated`: same with the update check;
  - `onExtensionUntrusted`: remove from installed, add to untrusted, update count;
  - `onPackageUninstalled`: delete private file, remove from both maps, update count.

Anime manager is the same, with one difference: `updatedInstalledXExtensionsStatuses` runs the same loop; the anime file uses `repoUrl` identically.

**Proposal:** the registry is one struct `ExtensionRegistry` with `installed: HashMap<pkg, Installed>`, `available`, `untrusted`, each behind `tokio::sync::watch`. The GTK main loop subscribes and converts to list models. The count preference becomes a row count in `kb_settings` (see p1-data-and-config.md).

## B3. Sub-language enablement on first run

OBSERVED (`enableAdditionalSubLanguages`, both managers):
- Runs once. `subLanguagesEnabledOnFirstRun = preferences.enabledLanguages().isSet()` at construction. If the user has never set languages, the flag starts `false`; the first time repo data is non-empty the flag flips true.
- Languages considered: every `sources.lang` of every available extension, distinct.
- `deviceLanguage = Locale.getDefault().language` (e.g. `"pt"`).
- Enabled set = `preferences.enabledLanguages().defaultValue()` plus every language `l` where `l != deviceLanguage && l.startsWith(deviceLanguage)` (for example `pt-BR` when device is `pt`).

**Proposal:** `$LANG` (POSIX locale, language part) instead of `Locale.getDefault()`. Identical rule otherwise.

## B4. Repository fetch (manga and anime)

OBSERVED (`MangaExtensionApi.findExtensions`, `AnimeExtensionApi.findExtensions`):
1. For every repo from `getExtensionRepo.getAll()`, in parallel (`async` per repo), `GET <baseUrl>/index.min.json` with `awaitSuccess()`.
2. Parse as a list of `ExtensionJsonObject` and map to `Available`.
3. Per-repo failure: log error, return `emptyList()`. The whole list is never failed by one repo.
4. Manga filter: keep entries where `libVersion >= 1.4 && libVersion <= 1.5` (MangaExtensionApi.kt `toExtensions`). Anime: `app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt` has no lib-version filter of its own (its 80 lines only call the repo fetch and run the update check); the anime repo conversion is outside this scope and was not read, so whether it filters by the 14/16/17 set is UNVERIFIED here. Kurobayomi applies the supported-api filter in `kb-ext::repo` regardless.
5. `name` is stripped of prefix `"Tachiyomi: "` (manga) or `"Aniyomi: "` (anime). Other fields copied, `iconUrl = <repo>/icon/<pkg>.png`, `apkUrl = <repo>/apk/<apk>`.
6. `checkForUpdates(context, fromAvailableExtensionList)`:
   - when called from the available list: return `null` if `now < last_ext_check + 1 day`;
   - otherwise fetch fresh, store `last_ext_check = now`;
   - compare to installed; collect extensions with update; if non-empty, `ExtensionUpdateNotifier.promptUpdates(names, anime)`.

Observed live index (checked 2026-10-08): `repos/aniyomiorg/aniyomi-extensions` branch `repo`, file `index.min.json`, 3 entries, all `lang: "all"`, all `version` starting `14.`, last commit on that branch dated 2024-07-11. Real example entry: `{"name":"Aniyomi: Google Drive","pkg":"eu.kanade.tachiyomi.animeextension.all.googledrive","apk":"aniyomi-all.googledrive-v14.15.apk","lang":"all","code":15,"version":"14.15","nsfw":0,"sources":[{"name":"Google Drive","lang":"all","id":"4222017068256633289","baseUrl":"https://drive.google.com"}]}`. Source ids are JSON strings.

Ecosystem size (checked with gh): `keiyoushi/extensions-source` default branch `main`, `src/all` has 134 entries and `src/en` has 407 entries (directory counts of the manga source tree). The Aniyomi anime tree is small by comparison; the anime ecosystem is the constraint on the migration value of this design.

**Proposal:** repo fetch and update check keep these semantics. Changes: the lib filter is `api` in `1.x` for the WASM contract (see p1-topic-wasm-component-design.md); the repo index also carries `sha256` per entry (new field, optional for compatibility with Aniyomi-format repos); `last_ext_check` becomes a row in `kb_settings`.

## B5. Installing and updating

OBSERVED (`app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt`, manga counterpart identical in structure):
1. `downloadAndInstall(url, ext)`:
   - if a download exists for `pkgName`, delete it first;
   - register the completion receiver;
   - `DownloadManager.Request(url)` with title `ext.name`, MIME `application/vnd.android.package-archive`, destination `Environment.DIRECTORY_DOWNLOADS` with the URL's last path segment, notification `VISIBILITY_VISIBLE_NOTIFY_COMPLETED`;
   - `activeDownloads[pkgName] = id`, state map entry `InstallStep.Pending`;
   - poll `downloadStatusFlow(id)` every `1.seconds` (`delay(1.seconds)`), emit only distinct values, stop on `STATUS_SUCCESSFUL` or `STATUS_FAILED`;
   - map status: `STATUS_PENDING -> Pending`, `STATUS_RUNNING -> Downloading`, others dropped;
   - flow ends when `isCompleted()` (Installed, Error, Idle); `onCompletion` removes the download.
2. `installApk(downloadId, uri)` by `extensionInstaller` preference:
   - `LEGACY`: start `AnimeExtensionInstallActivity` with the uri and `FLAG_GRANT_READ_URI_PERMISSION`;
   - `PRIVATE`: copy the uri to `cacheDir/temp_<id>`, call `installPrivateExtensionFile`, set `Installed` or `Error`, delete the temp file;
   - otherwise (Shizuku, PackageInstaller): start `AnimeExtensionInstallService` as a foreground service.
3. `installPrivateExtensionFile(context, file)` (MangaExtensionLoader.kt):
   - parse the APK archive info; not an extension -> `false`;
   - if a current extension with same pkg exists: if new `versionCode < current.versionCode` -> refuse (downgrade), if new signatures are empty -> refuse, if `!new.signatures.containsAll(current.signatures)` -> refuse (signature changed);
   - delete target, copy the file to `<filesDir>/exts/<pkg>.ext` and set read-only; notify `replaced` or `added`.
4. `cancelInstall(pkgName)`: remove from DownloadManager and from the installer queue (broadcast `Installer.action.CANCEL_QUEUE` with extra `Installer.extra.DOWNLOAD_ID`).
5. `uninstallApk(pkg)`: if installed as shared, open the system uninstall intent; else delete the private file and `notifyRemoved`.

**Proposal (Kurobayomi):**
- Download with `reqwest`, streaming to `<data>/extensions/.tmp/<pkg>.<version>.part`. Progress is a `tokio::sync::watch<u64>`.
- Verify sha256 against the repo entry when present; a mismatch deletes the temp file and yields `InstallStep::Error` with message key `ext_install_checksum_mismatch` (to be added to area:51-i18n).
- Downgrade and signature rules are kept as checks on the manifest: `version_code` must not decrease; the `key_id` must equal the key of the currently installed version.
- Polling is replaced by an awaited future; no 1-second loop.
- Cancel: abort the task and delete the temp file. State returns to `Idle`.

### B5.1 InstallStep state machine

```
Idle --start--> Pending --network--> Downloading --verify ok--> Installing --load ok--> Installed
  ^                |                    |                          |
  |                +-----cancel---------+--------cancel------------+--> Idle
  |                                     |
  |                                     +-- download fail / checksum mismatch / load fail --> Error
  +----------------------------------- terminal: Installed, Error, Idle (is_completed)
```

OBSERVED transitions are defined in `app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt` and `InstallerManga.kt` (`setInstalling`, `updateInstallStep`, `continueQueue`). The queue (`InstallerManga.addToQueue`) processes one entry at a time: `checkQueue` moves the head into `waitingInstall` with `compareAndSet(null, entry)`, and `continueQueue(result)` clears it and sets the step. When the queue is empty the service stops itself. **Proposal:** a `tokio::sync::Semaphore(1)` for installs; no service.

## B6. Trust

OBSERVED (`TrustMangaExtension.kt`, `TrustAnimeExtension.kt`):
- `isTrusted(pkgInfo, fingerprints)`:
  - `trustedFingerprints = repo.getAll().map { it.signingKeyFingerprint }` (manga) or `repository.getAll().map { it.signingKey }` (anime);
  - `key = "${pkgName}:${versionCode}:${fingerprints.last()}"`;
  - trusted if any repo fingerprint is in `fingerprints`, or `key` is in `preferences.trustedExtensions()`.
- `trust(pkg, versionCode, signatureHash)`: rewrite the preference set without any entry starting with `"$pkg:"`, then add `"$pkg:$versionCode:$signatureHash"`.
- `revokeAll()`: delete the preference.

Trust is per version code. A new version of a trusted package is not trusted when its signature differs from the one stored, and it is not trusted when its key is not in a repo key set.

**Proposal:** same two-tier rule. Repo entries carry `signing_key` (sha256 hex of the certificate). A WASM package may also be signed (ed25519 over the component digest) by the same key set; trust uses the key id. Stored as `kb_settings` rows `trusted_ext:<pkg>` = `"<version_code>:<key_id>"`.

## B7. Source manager and stub sources

OBSERVED (`AndroidMangaSourceManager.kt`):
- `sourcesMapFlow`: a `ConcurrentHashMap<Long, MangaSource>` rebuilt when installed extensions change; local source id `LocalMangaSource.ID` is always present.
- On every installed-extensions change, every source's `toStub()` is passed to `registerStubSource` (upsert in DB when `(id, lang, name)` changed; rename downloaded folders via `downloadManager.renameSource`).
- `get(id)` = live map lookup, may be null. `getOrStub(id)` = live, else cached stub, else `createStubSource(id)`:
  1. DB row `getStubMangaSource(id)` if present;
  2. `extensionManager.getSourceData(id)` from the repo availability map (`StubMangaSource` built from `Available.MangaSource`);
  3. otherwise `StubMangaSource(id, lang="", name="")`.
- `getOnlineSources()` = `HttpSource` instances; `getCatalogueSources()` = `CatalogueSource` instances; `getStubSources()` = stubs whose id is not an online source id.

**Proposal:** `SourceManager` with `get(id) -> Option<Arc<dyn Source>>`, `get_or_stub(id) -> SourceRef` where `SourceRef::Live | SourceRef::Stub`. A stub renders its name and lang and returns `SourceError::NotInstalled` from every network call. Message key: `source_not_installed` (see T-20a-095).

## B8. Source helper behaviour

OBSERVED (`MangaSourceExtensions.kt`):
- `getPreferenceKey() = "source_$id"`.
- `getNameForMangaInfo()`:
  - `enabled = enabledLanguages - {"all","other"}`;
  - if `enabled.size == 1` and `lang` not in enabled: show `toString()` (`"Name (LANG)"`);
  - if `enabled.size == 1` and `lang` in enabled: show `name`;
  - else `toString()`.
- `isLocalOrStub()` = local or `StubMangaSource`.
- `toString()` of HttpSource = `"$name (${lang.uppercase()})"`.

## B9. Source identity (HttpSource.generateId)

OBSERVED (`HttpSource.kt` lines near `generateId`; identical in `AnimeHttpSource.kt`):
```
key = "${name.lowercase()}/$lang/$versionId"
bytes = MD5(key as UTF-8)
id = ( (bytes[0]&0xff) << 56 | (bytes[1]&0xff) << 48 | ... | (bytes[7]&0xff) ) and Long.MAX_VALUE
```
`versionId` defaults to `1` in both `HttpSource` (`open val versionId = 1`) and `AnimeHttpSource` (`open val versionId: Int = 1`). The Kotlin expression `bytes[it].toLong() and 0xff shl 8 * (7 - it)` parses as `((bytes[it].toLong() and 0xff) shl (8*(7-it)))` because `and` and `shl` are left-associative infix functions with equal precedence and `*` binds tighter.

Consequences: changing `name` (case-insensitively) or `lang` changes the id; bumping `versionId` changes the id. Aniyomi keeps old ids reachable through `generateId` overrides (documented in AnimeHttpSource.kt lines ~62-100).

## B10. Anime-specific behaviour

- Hosters: `getHosterList(episode)` returns `List<Hoster>`; `getVideoList(hoster)` returns videos for one hoster; `resolveVideo(video)` returns a resolved video or null (`open suspend fun resolveVideo(video: Video): Video?` returns the input by default). `sortHosters` and `sortVideos` are identity by default (AnimeHttpSource.kt around line 591-609).
- `Hoster.lazy` means the video list is fetched only when the user selects the hoster (inferred from field semantics; the Kotlin does not fetch lazy hosters in `getVideoList(episode)`).
- `NO_HOSTER_LIST = "no_hoster_list"`: a flat list is wrapped into one hoster with this name (`List<Video>.toHosterList()`).
- Thumbnails: `getVideoThumbnails(video)` returns `ThumbnailInfo(tileInfo, imageTileUrls)`; `getImageTile(url)` returns a bitmap. Used for seek preview; **Proposal:** libmpv does not expose tile previews, so the Rust design stores the tile metadata and renders previews in GTK from the tile sheet; see R9 in p1-rust-design.md.
- `createHttpServer(): HttpServer?` lets a source serve local data to the player (NanoHTTPD on port 0, `http://localhost:<port>`). **Proposal:** dropped in favour of libmpv's own HTTP support; no local server (see the option matrix in p1-topic-option-matrix.md).
- `TimeStamp(start, end, name, type)` with `ChapterType` Opening/Ending/Recap/MixedOp/Other: skip markers.
- `Track(url, lang)` for subtitles and audio; passed to libmpv as external tracks.
- `mpvArgs`, `ffmpegStreamArgs`, `ffmpegVideoArgs` are lists of `(key, value)` pairs (passed as-is to mpv/ffmpeg in Aniyomi; **Proposal:** only `mpvArgs` is honoured, the ffmpeg ones are dropped, see p1-data-and-config.md).

## B11. Edge cases of the existing code (observed)

- The shared/private duplicate pkg case uses version code, not version name (`PackageInfoCompat.getLongVersionCode`).
- `Page.status` uses a `MutableStateFlow`; `progress` = `100*bytesRead/contentLength` or `-1` when length unknown (Page.kt `update`). **Proposal:** the same integer semantics for download progress display.
- `Hoster` equality is not defined (plain class), `Video` equality is by fields. Caching uses ids, not equality.
