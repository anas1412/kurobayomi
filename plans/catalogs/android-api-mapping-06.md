# Android/Java/third-party API mapping, chunk 6 of 8

Chunk 6 covers 54 imported class names, alphabetically from `Observer` (`rx.Observer`) to `RequestBody` (`okhttp3.RequestBody`). Source paths are relative to `/home/blackbox/aniyomi-src`. "Files" is the number of Kotlin files importing the class. "Refs" is the number of non-import lines in those files that mention the name (counted with `grep -E "\bName\b"`, so words like `Request` or `Project` overcount a little). Every row reads the Kotlin source; "Observed" is what Aniyomi does, "Proposal:" is what Kurobayomi should do.

Related areas: area:12-network-stack, area:data-layer, area:reader, area:player, area:extensions, area:background-jobs, area:notifications, area:settings.

Difficulty scale: trivial (a few lines or a crate call), moderate (a day or more of design), hard (a subsystem).

Index (alphabetical):
Observer, OkHttpClient, OkHttpNetworkFetcherFactory, OnItemSelectedListener, OneTimeWorkRequestBuilder, OpenableColumns, Options, Os, OsConstants, PackageInfo, PackageInfoCompat, PackageInstaller, PackageManager, Pager, PagerAdapter, PagingConfig, PagingData, PagingSource, PagingState, Paint, ParcelFileDescriptor, PendingIntent, PeriodicWorkRequestBuilder, PermissionChecker, PhotoView, PictureInPictureParams, PlaybackState, Plural, PluralFormatted, PluralsResource, PointF, PopupMenu, PowerManager, Precision, PreferenceDataStore, PreferenceFragmentCompat, PreferenceManager, PreferenceScreen, Process, ProcessLifecycleOwner, Producer, ProfileVerifier, Project, QuantizerCelebi, Query, QuickJs, Rational, Rect, RectF, RecyclerView, RemoteAction, RequerySQLiteOpenHelperFactory, Request, RequestBody.

---

## Cross-cutting decisions (read first)

1. **Extensions are Android APKs.** `PackageInfo`, `PackageInfoCompat`, `PackageInstaller`, `PackageManager`, `Process`, `Os` (partly) exist only to load/install Tachiyomi extension APKs (Dalvik code, `PathClassLoader`). Rust cannot run them. Proposal: Kurobayomi uses its own source mechanism (native plugin/WASM/script sources), see area:extensions. The Android-install plumbing is dropped; the *trust model* (signature fingerprint, repo-signed key, version codes, no downgrade) is kept in a Rust form.
2. **Paging 3 is replaced by explicit page cursors** in a `tokio` task plus a GTK `gio::ListStore` that is appended to.
3. **WorkManager is replaced by an in-process scheduler** (tokio timers) plus an optional systemd user timer for background runs while the app is closed.
4. **Coil / OkHttp are replaced by `reqwest` + an on-disk image cache + `gdk_pixbuf`/`gdk::Texture` decoding.**

---

## Reactive streams (RxJava 1)

### Observer
- **Does:** `rx.Observer` is implemented anonymously in the `Observable<T>.asFlow()` bridge: `onNext` -> `trySend`, `onError` -> `close(e)`, `onCompleted` -> `close()`; `awaitClose` unsubscribes. File: `app/src/main/java/eu/kanade/core/util/RxJavaExtensions.kt:10`. Files 1, refs 2.
- **Rust:** drop. Source API v1.4 methods returning `Observable` (e.g. `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt`, `AnimeHttpSource.kt` use `asObservableSuccess`) are legacy shims over the suspend functions. Rust source trait is `async fn` only (`Stream` via `futures::Stream` only where a long-lived feed is needed).
- **Preserve:** error propagates to the collector; cancel of the collector cancels the in-flight HTTP call.
- **Difficulty:** trivial. **Risk:** third-party extensions that still call `Observable` APIs (not relevant to Rust sources).

### Producer
- **Does:** `rx.Producer` is implemented on an `AtomicBoolean` "request arbiter" inside `Call.asObservable()` so a call executes exactly once on the first `request(n>0)` (`if (n == 0L || !compareAndSet(false, true)) return`). It also implements `Subscription` so `unsubscribe()` -> `call.cancel()`. File: `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:31`. Files 1, refs 2. `asObservableSuccess` additionally throws `HttpException(response.code)` when `!isSuccessful` after closing the body.
- **Rust:** drop. Equivalent behaviour is a future: `reqwest::Client::execute` once, then `error_for_status()` mapped to a project `HttpError(u16)`.
- **Preserve:** non-2xx -> `HttpError(code)` and the body is dropped (connection released).
- **Difficulty:** trivial.

---

## HTTP stack (OkHttp, Coil network)

### OkHttpClient
- **Does:** the single shared HTTP client, built in `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`: `connectTimeout(30, SECONDS)`, `readTimeout(30, SECONDS)`, `callTimeout(2, MINUTES)`, `Cache(directory = cacheDir/"network_cache", maxSize = 5L * 1024 * 1024)`, `cookieJar = AndroidCookieJar()`, interceptors in order: `UncaughtExceptionInterceptor`, `UserAgentInterceptor`, optional `HttpLoggingInterceptor(Level.HEADERS)` as *network* interceptor when `verboseLogging`, a DNS-over-HTTPS provider chosen by `dohProvider` (`PREF_DOH_CLOUDFLARE`, `PREF_DOH_GOOGLE`, `PREF_DOH_ADGUARD`, `PREF_DOH_QUAD9`, `PREF_DOH_ALIDNS`, `PREF_DOH_DNSPOD`, `PREF_DOH_360`, `PREF_DOH_QUAD101`, `PREF_DOH_MULLVAD`, `PREF_DOH_CONTROLD`, `PREF_DOH_NJALLA`, `PREF_DOH_SHECAN`, `PREF_DOH_LIBREDNS`), and last `CloudflareInterceptor` (WebView challenge solver). Files 26 (trackers, `DataSaver.kt`, `Tracker.kt`, `BaseTracker.kt`, `AnilistApi.kt:36`, `MyAnimeListApi.kt`, Coil fetchers, source API). Refs 45. Trackers derive an `authClient` via `client.newBuilder().addInterceptor(interceptor).build()` (`MyAnimeListApi.kt:49`).
- **Rust:** `reqwest::Client` (rustls, `cookie_store(true)` via `reqwest_cookie_store`, `connect_timeout(30s)`, `read_timeout(30s)`, `timeout(120s)`) wrapped in `reqwest_middleware` (or a small `tower` layer) for: default User-Agent when the request has none, rate-limit middleware, auth-header injection for trackers. DoH: `reqwest` + `hickory-resolver` configured with the same 13 providers. HTTP cache 5 MiB: `http-cache-reqwest` (disk manager, `cacache`).
- **Preserve:** the three timeouts, the 5 MiB cache, UA fallback only when the header is empty, one shared cookie jar per process, per-tracker client = shared client + auth layer (not a new connection pool).
- **Cloudflare:** no WebView. Proposal: embed `webkit6`/WebKitGTK window for challenge solving and import cookies into the jar (optional, P1); otherwise surface "blocked by Cloudflare" with an "open in browser" action.
- **Difficulty:** moderate. **Risk:** HTTP/2 and TLS fingerprint differences may trigger Cloudflare more often than OkHttp.

### Request
- **Does:** immutable request value. Helpers `GET`, `POST`, `PUT`, `DELETE` in `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt` default to `CacheControl.Builder().maxAge(10, MINUTES)`, empty `Headers`, and an empty `FormBody` body for POST/PUT/DELETE. Coil fetchers build requests with `Request.Builder()` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt:180`, `AnimeImageFetcher.kt:181`): source headers are applied, and `cacheControl` is `CACHE_CONTROL_NO_STORE` when network-cache reads are enabled (do not fill OkHttp cache; covers have their own cache) else `CACHE_CONTROL_NO_NETWORK_NO_CACHE` (forces 504 "Unsatisfiable Request"). Files 13, refs 49.
- **Rust:** `reqwest::Request`/`RequestBuilder`. Provide `fn get(url, headers, cache)`, `post`, `put`, `delete` in `net/requests.rs` with the same defaults (10 min max-age; empty form body).
- **Preserve:** 10-minute default cache window for source requests; cover/page image requests bypass the HTTP cache.
- **Difficulty:** trivial.

### RequestBody
- **Does:** body holder; used in `Requests.kt:18` (`DEFAULT_BODY = FormBody.Builder().build()`), `KitsuApi.kt:397` (login password grant: `username`, `password`, `grant_type=password`, `client_id`, `client_secret`) and `MyAnimeListApi.kt:52,409` (`client_id`, `code`, `code_verifier`, `grant_type=authorization_code`; refresh token flow at :409). Files 3, refs 9.
- **Rust:** `reqwest::RequestBuilder::form(&[(..)])` for `application/x-www-form-urlencoded`; `.json()` for JSON bodies; `reqwest::multipart` for multipart sources.
- **Preserve:** exact field names above; form encoding (not JSON) for OAuth token endpoints.
- **Difficulty:** trivial.

### OkHttpNetworkFetcherFactory
- **Does:** Coil network fetcher factory registered in `App.newImageLoader` (`app/src/main/java/eu/kanade/tachiyomi/App.kt:187`) with a lazy `NetworkHelper.client`. Image loader config in the same function: crossfade `(300 * animatorDurationScale)` ms, `allowRgb565` on low-RAM, fetcher dispatcher `Dispatchers.IO.limitedParallelism(8)`, decoder dispatcher `limitedParallelism(3)`. Files 1, refs 2.
- **Rust:** `ImageLoader` service: `tokio::sync::Semaphore(8)` for fetches and `Semaphore(3)` for decodes, `reqwest` client from NetworkHelper, in-memory LRU of `gdk::Texture`, disk cache directory under `$XDG_CACHE_HOME/kurobayomi/`. Crossfade 300 ms via `gtk::Revealer`/opacity animation (`adw`-free: `gtk::Widget::add_tick_callback`).
- **Preserve:** parallelism limits 8 fetch / 3 decode; 300 ms crossfade (0 when the desktop disables animations, `gtk-enable-animations`).
- **Difficulty:** moderate.

### Options
- **Does:** `coil3.request.Options` is the per-request context in Keyers and Fetchers. Keyers define cache keys: `MangaKeyer` -> `"manga;${id};${coverLastModified}"` if custom cover else `"manga;${thumbnailUrl};${coverLastModified}"`; `MangaCoverKeyer` -> `"manga;${mangaId};${lastModified}"` if a custom cover file exists else `"manga;${url};${lastModified}"` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt:14`); anime equivalents in `AnimeCoverKeyer.kt:14,27`. Fetchers read `options.networkCachePolicy.readEnabled` and `options.diskCachePolicy.writeEnabled` (`MangaCoverFetcher.kt`). Files 7, refs 17.
- **Rust:** a `struct ImageOptions { network_cache_read: bool, disk_cache_write: bool }` and a `fn cache_key(&Entry) -> String` using the exact key formats, so cover refresh on `coverLastModified` change works.
- **Preserve:** key formats (a change of `lastModified` must invalidate); custom cover wins over remote URL.
- **Difficulty:** trivial.

### Precision
- **Does:** `coil3.size.Precision`: `INEXACT` for reader pages (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:339`, view-size resolved, no disk/memory cache, no crossfade); `EXACT` for widget thumbnails (`presentation-widget/src/main/java/tachiyomi/presentation/widget/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt:137`, anime twin :137). Files 3, refs 3.
- **Rust:** a `Precision` enum on the loader request; EXACT => decode at exactly the target size (`Pixbuf::from_file_at_scale`), INEXACT => decode at the closest power-of-two downsample ("scale on load"). Widgets are dropped (see PendingIntent / ProfileVerifier notes: no Glance widgets on Linux; replacement is a notification-less library; optional GNOME/KDE extras out of scope).
- **Difficulty:** trivial.

### QuickJs
- **Does:** `JavaScriptEngine.evaluate(script)` creates a fresh `QuickJs`, evaluates, returns a primitive, closes (`core/common/src/main/java/eu/kanade/tachiyomi/network/JavaScriptEngine.kt:22`). Exposed to extensions since extensions-lib 1.4. Files 1, refs 2.
- **Rust:** `rquickjs` crate (QuickJS bindings). One short-lived `Runtime`+`Context` per call, result converted to a `serde_json::Value` limited to primitives (string, number, bool, null).
- **Preserve:** fresh context per call (no state leakage), run on a blocking thread (`spawn_blocking`), enforce a memory limit and an interrupt-handler timeout (Proposal: 10 s, 64 MiB) to avoid runaway scripts.
- **Difficulty:** moderate. **Risk:** only useful to JS-capable sources; if sources are WASM, drop.

---

## Background work (WorkManager)

### OneTimeWorkRequestBuilder
- **Does:** 11 one-shot jobs, all in `app/src/main/java/eu/kanade/...`: `DelayedMangaTrackingUpdateJob` and `DelayedAnimeTrackingUpdateJob` (`NetworkType.CONNECTED`, backoff EXPONENTIAL 5 min, tag `"DelayedMangaTrackingUpdate"`, `ExistingWorkPolicy.REPLACE`), `BackupCreateJob` (tag `TAG_MANUAL`, input `IS_AUTO_BACKUP_KEY=false`, `LOCATION_URI_KEY`, `OPTIONS_KEY`), `BackupRestoreJob` (`ExistingWorkPolicy.KEEP`, input `LOCATION_URI_KEY`, `SYNC_KEY`, `OPTIONS_KEY`), `MangaDownloadJob`/`AnimeDownloadJob` (tag `"MangaDownloader"`, `REPLACE`; stops with notification text when no network or only-Wi-Fi violated), `MangaLibraryUpdateJob`/`AnimeLibraryUpdateJob` (`startNow`: refuse if already running; `KEEP`; input `KEY_CATEGORY`), `MangaMetadataUpdateJob`/`AnimeMetadataUpdateJob` (`KEEP`, per-source queue warning threshold `MANGA_PER_SOURCE_QUEUE_WARNING_THRESHOLD = 60`), `AppUpdateDownloadJob` (`CONNECTED`, `REPLACE`, inputs `DOWNLOAD_URL`, `DOWNLOAD_TITLE`). Files 11, refs 11.
- **Rust:** `JobRunner` with named unique jobs: `enum ExistingPolicy { Keep, Replace }`; each job is a `tokio::spawn`ed task with a `CancellationToken`, tracked in a `HashMap<&'static str, JobHandle>`. Retry with backoff: `tokio::time::sleep(5 min * 2^n)` for tracking updates. Persist pending delayed tracking updates in SQLite so they survive restarts.
- **Preserve:** uniqueness semantics (KEEP vs REPLACE), "already running -> return false", cancel by tag, network-required gating, notification text for stop reasons.
- **Difficulty:** moderate.

### PeriodicWorkRequestBuilder
- **Does:** three periodic jobs. Library update (`MangaLibraryUpdateJob.kt:466`, anime `:479`): period `interval` hours, flex 10 minutes, `BackoffPolicy.LINEAR` 10 min, tags `TAG` + `WORK_NAME_AUTO`, `ExistingPeriodicWorkPolicy.UPDATE`; constraints from `autoUpdateDeviceRestrictions`: `DEVICE_ONLY_ON_WIFI` (TRANSPORT_WIFI), `DEVICE_NETWORK_NOT_METERED` (UNMETERED + NET_CAPABILITY_NOT_METERED), `DEVICE_CHARGING` (requires charging), always `requiresBatteryNotLow`. `interval <= 0` cancels the work. Backup (`BackupCreateJob.kt:98`): period `interval` hours, flex 10 min, backoff EXPONENTIAL 10 min, `requiresBatteryNotLow`, input `IS_AUTO_BACKUP_KEY=true`. Files 3, refs 3.
- **Rust:** `Scheduler::set_periodic(name, every: Duration, flex: Duration, constraints)`. Next-run time stored in the settings table (`last_run_at`), computed at startup so missed runs fire within the flex window. Constraints: Wi-Fi/metered via NetworkManager D-Bus (`org.freedesktop.NetworkManager` `Metered` property, `PrimaryConnection` type via `zbus`), charging/battery via UPower D-Bus (`org.freedesktop.UPower` `OnBattery`, `Percentage`; "battery not low" = not (OnBattery && percentage <= 15)). Background while app is closed: optional `systemd --user` timer unit that launches `kurobayomi --headless-update`.
- **Preserve:** 10 min flex, backoff type per job, device restriction flags.
- **Difficulty:** moderate. **Risk:** desktops have no unified "battery low" signal; if no battery, treat as OK.

---

## Packages and installers (Android package system)

### PackageInfo
- **Does:** parsed APK manifest of an extension. `isTrusted(pkgInfo, fingerprints)` in `app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt:14`; loader reads `reqFeatures`, `versionName`, `applicationInfo.metaData` in `app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt` (anime twin `AnimeExtensionLoader.kt:194,399`). Files 4, refs 10. Constants: feature `tachiyomi.extension`, metadata `tachiyomi.extension.class`, `tachiyomi.extension.factory`, `tachiyomi.extension.nsfw` (== 1), `LIB_VERSION_MIN = 1.4`, `LIB_VERSION_MAX = 1.5` (libVersion = versionName major.minor), `isPackageAnExtension` = `reqFeatures` contains `tachiyomi.extension`.
- **Rust:** `struct ExtensionManifest { pkg_name, version_name, version_code: u64, lib_version: f64, class_names, factory, nsfw: bool, signing_sha256: Vec<String> }` read from the new plugin format's manifest (Proposal: a JSON/TOML manifest inside a signed archive; see area:extensions). Same accept rule: reject when `lib_version` outside `[1.4, 1.5]` and when NSFW is not allowed (`showNsfwSource` pref).
- **Difficulty:** moderate.

### PackageInfoCompat
- **Does:** `getLongVersionCode(pkgInfo)` returns the 64-bit version code. Uses: trust key `"${packageName}:${versionCode}:${fingerprints.last()}"` (`TrustMangaExtension.kt:16`); downgrade check `if (code(extension) < code(currentExtension)) -> "Installed extension version is higher. Downgrading is not allowed."` (`MangaExtensionLoader.kt:77`); shared-vs-private selection `shared >= private` picks shared. Files 4, refs 12.
- **Rust:** `u64 version_code` field; same comparisons.
- **Difficulty:** trivial.

### PackageManager
- **Does:** (a) extension loading: `getPackageArchiveInfo(path, PACKAGE_FLAGS)` with `GET_CONFIGURATIONS | GET_META_DATA | GET_SIGNATURES | GET_SIGNING_CERTIFICATES`, `getPackageInfo`, `getApplicationLabel` (extension name is the label with prefix `"Tachiyomi: "` removed); (b) `BasePreferences.deviceHasPip()` -> `FEATURE_PICTURE_IN_PICTURE` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:26`); (c) onboarding `PERMISSION_GRANTED` checks (`app/src/main/java/eu/kanade/presentation/more/onboarding/PermissionStep.kt:61`); (d) `ExtensionsBackupCreator` reads each installed extension's `publicSourceDir` APK bytes into `BackupExtension(packageName, apk)` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/ExtensionsBackupCreator.kt:26,39`). Files 12, refs 31.
- **Rust:** (a) `ExtensionRegistry` scanning `$XDG_DATA_HOME/kurobayomi/extensions/*.ext` (private-ext dir analog of `filesDir/"exts"` with extension `ext`, files made read-only); (b) drop PiP feature check: always true on desktop but see PictureInPictureParams; (c) drop; (d) backup stores extension *identifiers + repo URL + version* instead of binaries (APK bytes are meaningless on Linux). Restoring an Aniyomi backup: ignore `BackupExtension` binaries, show "extensions not restored" list.
- **Difficulty:** moderate. **Loses:** cross-app shared extension (APKs installed system-wide).

### PackageInstaller
- **Does:** `PackageInstallerInstallerManga/Anime` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/PackageInstallerInstallerManga.kt:25`) handle the session result broadcast: `STATUS_PENDING_USER_ACTION` -> launch the system confirmation intent, `STATUS_FAILURE_ABORTED` -> `InstallStep.Idle`, `STATUS_SUCCESS` -> `InstallStep.Installed`, everything else -> `InstallStep.Error`. Files 2, refs 15. The four installer modes are `enum ExtensionInstaller { LEGACY, PACKAGEINSTALLER, SHIZUKU, PRIVATE }` (`BasePreferences.kt`).
- **Rust:** one installer: download the extension archive with `reqwest`, verify sha256/signature, write to `$XDG_DATA_HOME/kurobayomi/extensions/<pkg>.ext` atomically (`tempfile` + `rename`), make read-only, hot-reload. States `Idle, Pending, Downloading, Installing, Installed, Error` (same as `InstallStep`).
- **Drop:** LEGACY, PACKAGEINSTALLER, SHIZUKU; the PRIVATE mode is the only one kept. User loses: nothing on Linux (no system installer exists).
- **Difficulty:** trivial.

### Process
- **Does:** `Process.myUserHandle().hashCode()` used to build `pm install-create --user $userId -r -i <pkg> -S <size>` in `ShizukuInstallerManga.kt:53` (anime twin). Followed by `pm install-write -S size sessionId base -` and `pm install-commit`; on error `pm install-abandon`. Files 2, refs 2.
- **Rust:** drop (Shizuku/ADB shell is Android-only). **Loses:** privileged silent install, not needed.
- **Difficulty:** trivial (deletion).

### Os
- **Does:** `android.system.Os` low-level syscalls: `Os.mmap(0, size, PROT_READ, MAP_PRIVATE, fd, 0)` and `Os.munmap` in `core/archive/src/main/kotlin/mihon/core/archive/ArchiveReader.kt:12,36` (archive is mapped and handed to libarchive as one address range); `Os.fstat(fd)` and `Os.read(fd, buffer)` in `ZipWriter.kt:36,41` (copy file stat into the zip entry; copy loop with an 8192-byte direct buffer until `position() == 0`). Files 2, refs 4.
- **Rust:** `memmap2::Mmap` for reading; `zip` crate (`ZipArchive`) for CBZ/ZIP, `unrar`/`sevenz-rust`/`compress-tools` (libarchive via `compress-tools` or `libarchive3-sys`) for CBR/7z/tar. Writing: `zip::ZipWriter` with `CompressionMethod::Stored` and UTF-8 names (`SimpleFileOptions::default().compression_method(Stored)`), copying mtime/permissions from `std::fs::metadata`.
- **Preserve:** store-only (no compression) for downloaded CBZ; UTF-8 entry names; entry mtime from source file; `getInputStream(name)` returns the entry stream or None.
- **Difficulty:** moderate.

### OsConstants
- **Does:** `PROT_READ`, `MAP_PRIVATE` constants passed to `Os.mmap` (`ArchiveReader.kt:12`). Files 1, refs 2.
- **Rust:** `memmap2` hides them (`Mmap::map` is read-only private mapping). **Difficulty:** trivial.

### ParcelFileDescriptor
- **Does:** fd wrapper from SAF `contentResolver.openFileDescriptor(uri, mode)`: `ArchiveReader(pfd)` uses `pfd.statSize` and `pfd.fileDescriptor`; `UniFileExtensions.kt` `openFileDescriptor(context, mode)` errors with `"Failed to open file descriptor: ${filePath ?: uri}"`; `PlayerUtils.kt:31` `ParcelFileDescriptor.adoptFd(it).close()` to close a detached fd after resolving a real path for mpv. Files 3, refs 3.
- **Rust:** `std::fs::File` / `OwnedFd`; paths are real filesystem paths so no SAF. For mpv pass the path or `file://` URL directly.
- **Preserve:** error message text for open failure (use `anyhow::Context`).
- **Difficulty:** trivial.

### OpenableColumns
- **Does:** `getFileName` queries `OpenableColumns.DISPLAY_NAME` for a content Uri to show a name for externally opened video (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerUtils.kt:51`). Same file has `resolveUri`: `file` -> path, `content` -> fd or real path, `data` -> `"data://$schemeSpecificPart"`, `in Utils.PROTOCOLS` -> `toString()`, else null + error log `"unknown scheme: $scheme"`. Files 1, refs 2.
- **Rust:** `std::path::Path::file_name()` for `file://`; for URLs use the last path segment percent-decoded (`percent-encoding`). mpv handles `data://`, http(s), rtmp etc. directly; keep the "unknown scheme" error.
- **Difficulty:** trivial.

### PermissionChecker
- **Does:** `Context.hasPermission(permission)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt:63`) and the guard in `Context.notify` returning silently when `POST_NOTIFICATIONS` is not granted on API 33+ (`NotificationExtensions.kt:31`). Files 2, refs 6.
- **Rust:** drop. Desktop notifications need no runtime permission; `gio::Application::send_notification` (or `notify-rust` via the freedesktop `org.freedesktop.Notifications` D-Bus API). If the notification daemon is absent, ignore errors.
- **Difficulty:** trivial.

### PowerManager
- **Does:** `isIgnoringBatteryOptimizations(packageName)` for onboarding (`PermissionStep.kt:65`) and the Advanced settings row (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:162`). `Context.powerManager` accessor at `ContextExtensions.kt:68`. Files 2, refs 2.
- **Rust:** drop; background timers on Linux are not killed by doze. Optional: `org.freedesktop.login1` inhibitor (`zbus`, `Inhibit("sleep", ..., "block")`) while a download or library update runs so the machine does not suspend mid-job. Loses: nothing.
- **Difficulty:** trivial.

---

## Notifications and intents

### PendingIntent
- **Does:** 13 files, 167 refs. Notifications get tap/action intents: `NotificationHandler.kt` (`getActivity` with `FLAG_UPDATE_CURRENT or FLAG_IMMUTABLE`: open downloads manager, open image, open entry/reader), `NotificationReceiver.kt` (125 refs: broadcast actions e.g. cancel download, mark as read, resume/pause, dismiss), `AnimeDownloadNotifier.kt:176` / `MangaDownloadNotifier.kt:172` (`onWarning(reason, timeout, contentIntent, id)`), `MangaLibraryUpdateNotifier.kt`/`AnimeLibraryUpdateNotifier.kt`, `AppUpdateNotifier.kt`, `HttpServerService.kt`, `TorrentServerService.kt`, installers, `App.kt:126` (incognito notification: `getBroadcast(..., Intent(ACTION_DISABLE_INCOGNITO_MODE), FLAG_ONE_SHOT or FLAG_IMMUTABLE)`), and `PipActions.kt` (see RemoteAction).
- **Rust:** `gio::Notification` with `add_button_with_target_value("Cancel", "app.cancel-download", ...)` and `set_default_action_and_target("app.open-entry", ...)`. Each former broadcast action becomes a `gio::SimpleAction` on the `gtk::Application` (`app.*`). Notification IDs: use `gio::Application::send_notification(Some(id), &n)` / `withdraw_notification(id)` so updates replace in place. On KDE the GApplication notification backend uses the portal / `org.freedesktop.Notifications` and supports actions.
- **Preserve:** per-notification ID scheme (same keyed replace-in-place behaviour for progress), incognito persistent notification with a "disable" action, `ONE_SHOT` semantics (action removed after use).
- **Difficulty:** moderate (many action types). **Risk:** progress bars in notifications are not supported by freedesktop; use a text "n/m" and in-app progress, see area:notifications.

---

## Pagination (Paging 3)

### Pager
- **Does:** produces `Flow<PagingData<T>>` from a `PagingSource` factory. Browse source screens: `Pager(PagingConfig(pageSize = 25)) { getRemoteManga.subscribe(sourceId, listing.query ?: "", listing.filters) }.flow.map {...}.cachedIn(ioCoroutineScope)` (`app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/source/browse/BrowseMangaSourceScreenModel.kt:112`; anime `BrowseAnimeSourceScreenModel.kt:112`); season-select for migration (`MigrateSeasonSelectScreenModel.kt:66`, pageSize 25, one-shot list). Each item is mapped to a local DB entry through `networkToLocalManga.await(...)` then `getManga.subscribe(url, source).filterNotNull().stateIn(...)`, and filtered with `!hideInLibraryItems || !it.value.favorite`. Files 3, refs 5.
- **Rust:** `struct PagedLoader<T> { next_page: Option<u32>, loading: bool, items: gio::ListStore, error: Option<Error> }` with `async fn load_next(&mut self)` triggered by the scroller's `edge-reached`(bottom) signal on `gtk::ScrolledWindow` (or when the last visible index >= len - prefetch). Reset the loader whenever `(query, filters)` changes (equivalent of `distinctUntilChanged` on `listing`). The "hide in library" filter runs at append time with the favorite flag read live from the DB.
- **Preserve:** page size is advisory (a source decides page length; Aniyomi's `pageSize = 25` only sets Paging's prefetch window); stop when `hasNextPage` is false; one load in flight at a time; results cached for the screen's lifetime (re-collecting does not refetch).
- **Difficulty:** moderate.

### PagingConfig
- **Does:** only `PagingConfig(pageSize = 25)` at the three call sites above (`BrowseAnimeSourceScreenModel.kt:112`, `BrowseMangaSourceScreenModel.kt:112`, `MigrateSeasonSelectScreenModel.kt:67`). Defaults used otherwise: `prefetchDistance = pageSize`, `initialLoadSize = 3 * pageSize`, `enablePlaceholders = true`. Files 3, refs 3.
- **Rust:** `const BROWSE_PREFETCH: usize = 25;` start loading the next page when fewer than 25 items remain below the viewport.
- **Difficulty:** trivial.

### PagingData
- **Does:** wrapper consumed by `StateFlow<Flow<PagingData<T>>>.collectAsLazyPagingItems()` (`presentation-core/src/main/java/mihon/presentation/core/util/PagingDataUtil.kt:13`), which flattens the flow-of-flows to Compose `LazyPagingItems`. Files 1, refs 2.
- **Rust:** `gio::ListStore` / `gtk::SliceListModel`; `gtk::GridView` with a `gtk::SignalListItemFactory`. Flow-of-flows is replaced by "recreate the `PagedLoader` and the model on listing change".
- **Difficulty:** trivial.

### PagingSource
- **Does:** 9 files. (1) `SourcePagingSource` family (`data/src/main/java/tachiyomi/data/source/manga/MangaSourcePagingSource.kt`, anime twin): key = page number starting at 1 (`params.key ?: 1`); `SourceSearchPagingSource` calls `getSearchManga(page, query, filters)`, `SourcePopularPagingSource` -> `getPopularManga(page)`, `SourceLatestPagingSource` -> `getLatestUpdates(page)`; empty `mangas` list is converted to `NoChaptersException()` and returned as `LoadResult.Error`; `nextKey = if (hasNextPage) page + 1 else null`, `prevKey = null`. (2) `QueryPagingMangaSource`/`QueryPagingAnimeSource` (`data/src/main/java/tachiyomi/data/handlers/manga/QueryPagingMangaSource.kt`): DB-backed offset paging with `jumpingSupported = true`, runs `countQuery` then `queryProvider(limit, offset)`, `prevKey = if (offset <= 0 || prevKey < 0) null else prevKey`, `nextKey = if (offset + loadSize >= count) null else nextKey`, `itemsBefore = max(0, offset)`, `itemsAfter = max(0, count - (offset + loadSize))`, listener invalidates on any table change. Exposed by `subscribeToPagingSource` in `AndroidMangaDatabaseHandler.kt:77`, but no screen calls it in this tree (declared in `AnimeDatabaseHandler`/`MangaDatabaseHandler` only). (3) `SeasonListPagingSource` (one page, `nextKey = null`). Files 9, refs 9.
- **Rust:** `trait PageSource { async fn load(&self, page: u32) -> Result<Page<T>, SourceError>; }` with `Page { items, has_next }`. Empty first page => user-visible "no results" state (message key `no_results_found`), not a crash. DB-backed paged lists: drop (unused); library uses an in-memory list from one query, see area:data-layer.
- **Preserve:** empty page => error state; page numbers start at 1; stop on `!hasNextPage`.
- **Difficulty:** moderate.

### PagingState
- **Does:** `getRefreshKey(state)` returns `anchorPosition -> closestPageToPosition -> prevKey ?: nextKey` so a refresh restarts at the page nearest the viewport (`QueryPagingMangaSource.kt:66`, `MangaSourcePagingSource.kt`); season source returns `null`. Files 5, refs 5.
- **Rust:** not needed: refresh resets to page 1 for remote sources. Drop.
- **Difficulty:** trivial.

### Query
- **Does:** SQLDelight `Query<T>` is the typed query handle. `DatabaseHandler` wrappers (`data/src/main/java/tachiyomi/data/handlers/manga/AndroidMangaDatabaseHandler.kt:25-110`): `awaitList` -> `executeAsList`, `awaitOne` -> `executeAsOne`, `awaitOneOrNull` -> `executeAsOneOrNull`, `subscribeToList/One/OneOrNull` -> `asFlow().mapToList/One/OneOrNull(queryDispatcher)`; `Query.Listener.queryResultsChanged()` triggers re-run when any table in the query changes. Transactions: `inTransaction` -> `withMangaTransaction`; nested calls inside an existing transaction run inline (`driver.currentTransaction() != null`); otherwise `withContext(getCurrentMangaDatabaseContext())`. Files 6, refs 40. Two databases: `tachiyomi.db` (manga) and `tachiyomi.animedb` (anime).
- **Rust:** `rusqlite` + one dedicated DB thread per database (actor with `std::sync::mpsc`/`tokio::sync::mpsc` + `oneshot`) so every call is `await`able and transactions are naturally serialised. Change notification: a `tokio::sync::broadcast::Sender<TableSet>` fired after each write transaction with the set of touched tables (use `rusqlite::Connection::update_hook` to collect table names, emit at commit). Subscribers re-run their query when their table set intersects. Provide `fn subscribe_list<T>(tables: &[Table], f: impl Fn(&Connection) -> Result<Vec<T>>) -> impl Stream<Item=Vec<T>>`.
- **Preserve:** write-then-notify only on commit (not mid-transaction); nested transaction inlining; one writer.
- **Difficulty:** hard (reactive layer is the core of the app). See area:data-layer.

### RequerySQLiteOpenHelperFactory
- **Does:** swaps the platform SQLite for the bundled Requery SQLite build (consistent SQLite version, FTS, newer features) at `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:86,111`; debug builds on API 30+ use `FrameworkSQLiteOpenHelperFactory`. Both drivers set on open: `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL`; names `"tachiyomi.db"` and `"tachiyomi.animedb"`. Files 1, refs 3.
- **Rust:** `rusqlite` with the `bundled` feature (ships its own SQLite). On open: execute the same three pragmas. Schema versioning with `PRAGMA user_version` and migrations from the SQLDelight `.sqm` files (see area:data-layer).
- **Preserve:** foreign keys on (cascading deletes depend on it), WAL, `synchronous = NORMAL`.
- **Difficulty:** trivial.

---

## Preferences (AndroidX Preference / SharedPreferences)

### PreferenceDataStore
- **Does:** `SharedPreferencesDataStore` adapts a `SharedPreferences` to AndroidX `PreferenceDataStore` for each source's own prefs (get/put Boolean, Int, Long, Float, String, StringSet), each write via `prefs.edit { ... }` (`app/src/main/java/eu/kanade/tachiyomi/data/preference/SharedPreferencesDataStore.kt:7`). Files 1, refs 2.
- **Rust:** `trait SourcePrefStore { fn get(&self, key) -> Option<PrefValue>; fn set(&self, key, PrefValue); }` backed by a `source_prefs` SQLite table (`source_id, key, type, value`) or one `serde_json` file per source; `enum PrefValue { Bool, Int, Long, Float, String, StringSet(BTreeSet<String>) }`.
- **Preserve:** the six value types; write-through on change.
- **Difficulty:** trivial.

### PreferenceFragmentCompat
- **Does:** hosts a source's settings (`MangaSourcePreferencesFragment`, `app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt:121`; anime twin same dir). `populateScreen` creates a screen, installs the data store, calls `source.setupPreferenceScreen(screen)`, and for every pref sets `isIconSpaceReserved = false`, `isSingleLineTitle = false`, dialog title = pref title if empty, and wraps `EditTextPreference` bind listeners with an incognito keyboard flag. Files 2, refs 2.
- **Rust:** a `SourceSettingsPage` built by walking a declarative `PrefSpec` list returned by the source (`Switch`, `List`, `MultiSelect`, `EditText`, `Group`) into `gtk::ListBox` rows (`gtk::Switch`, `gtk::DropDown`, `gtk::CheckButton` list, `gtk::Entry`). Entry rows: `input-hints = NO_SPELLCHECK | PRIVATE` (the GTK analog of incognito IME).
- **Preserve:** dialog title falls back to row title; no icons; multi-line titles.
- **Difficulty:** moderate.

### PreferenceScreen
- **Does:** root container returned by `populateScreen(): PreferenceScreen` in the same two files. Files 2, refs 2.
- **Rust:** the `PrefSpec` tree root (above). **Difficulty:** trivial.

### PreferenceManager
- **Does:** 24 migration files use `PreferenceManager.getDefaultSharedPreferences(context)` as the raw preference file to rewrite old keys, e.g. `app/src/main/java/mihon/core/migration/migrations/DOHMigration.kt:18` (v57: `enable_doh` true -> `dohProvider = PREF_DOH_CLOUDFLARE`, remove `enable_doh`), `EnumsMigration.kt:20` (v123: `pref_deband` int -> `Debanding` enum, `pref_player_view_mode` -> `VideoAspect` enum, `gpu_next` -> `pref_gpu_next`; removes `pref_invert_playback_txt`, `pref_invert_duration_txt`, `pref_hwdec`), plus `CombineUpdateRestrictionMigration`, `MergeSortTypeDirectionMigration` and the rest of the migrations folder. Files 24, refs 24. Runner: `Migrator.initialize(old = last_version_code, new = BuildConfig.VERSION_CODE, migrations, onMigrationComplete)` in `App.kt:~165` using key `Preference.appStateKey("last_version_code")`.
- **Rust:** none of the old Android keys exist in a fresh Kurobayomi install. Proposal: a clean `settings` table (`key TEXT PRIMARY KEY, value TEXT`) via `rusqlite`; a `settings_version` row and a `Vec<Migration>` for Kurobayomi's own future changes. Importing an Aniyomi *backup* needs only the value-mapping part (e.g. `pref_deband` ordinal -> enum), see area:backup. Drop the 24 Android-specific migrations.
- **Difficulty:** trivial (new design), moderate (backup import mapping).

---

## Lifecycle, profiling, build

### ProcessLifecycleOwner
- **Does:** app-wide foreground/background lifecycle: `ProcessLifecycleOwner.get().lifecycle.addObserver(this)` (`App.kt:108`) drives `onStart`/`onStop` (e.g. `SecureActivityDelegate.onApplicationStart()`), `lifecycleScope` as the long-lived coroutine scope for incognito-notification reaction (`basePreferences.incognitoMode().changes()`), and for `MangaWidgetManager`/`AnimeWidgetManager.init(scope)`. Files 1, refs 6.
- **Rust:** a single `tokio::runtime::Runtime` owned by `main` plus `glib::MainContext` bridging; "app scope" = a `CancellationToken` cancelled on `gio::Application::shutdown`. Foreground/background: `gtk::Window::is-active` notify for the app-lock feature (re-lock after the window is unfocused longer than the configured delay).
- **Difficulty:** trivial.

### ProfileVerifier
- **Does:** debug page row "Profile compilation status" mapping `profileInstallResultCode` to `"No profile installed"`, `"Compiled"`, `"Compiled non-matching"`, `"Error $result"`, `"Not supported"`, `"Pending compilation"`, `"No profile embedded"`, `"Unknown code $result"` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt:79`). Files 1, refs 11.
- **Rust:** drop (ART baseline profiles). Replace the row by "Build: rustc version, profile (release/debug), git sha" from `env!`/`vergen`. Loses: nothing.
- **Difficulty:** trivial.

### Project
- **Does:** Gradle build logic in `buildSrc/src/main/kotlin/mihon/buildlogic/`: `Project.getCommitCount()` (`git rev-list --count HEAD`), `getGitSha()` (`git rev-parse --short HEAD`), `getBuildTime(useLastCommitTime)` (`git log -1 --format=%ct` or now, UTC, pattern `yyyy-MM-dd'T'HH:mm:ss'Z'`), `Project.Config` flags `enable-updater`, `disable-code-shrink`, `include-dependency-info`, and `tasks/LocalesConfigTask.kt`. Files 4, refs 14.
- **Rust:** `build.rs` (or `vergen`) emitting `GIT_SHA`, `COMMIT_COUNT`, `BUILD_TIME` (same UTC format) as `cargo:rustc-env`; Cargo features `updater` (self-update check) and `deps-info` replace the Gradle properties; `cargo-about` for dependency info. Locales config: no equivalent needed (gettext/`fluent` locale list).
- **Difficulty:** trivial.

### QuantizerCelebi
- **Does:** Material color quantizer used by `MonetColorScheme.extractSeedColorFromImage(bitmap)`: `Score.score(QuantizerCelebi.quantize(pixels, 128), 1, 0)[0].takeIf { it != 0 }` (`app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt:53`). Marked `@Suppress("Unused")` (no caller). Files 1, refs 2.
- **Rust:** drop. The user's GTK theme drives colors (no Monet). If cover-derived accent is wanted later: `material-colors` crate (`QuantizerCelebi` + `Score`).
- **Loses:** dynamic wallpaper/cover color themes (Android 8.1+ Monet).
- **Difficulty:** trivial.

---

## Reader drawing and gestures

### Paint
- **Does:** `ReaderActivity.getCombinedPaint(grayscale, invertedColors)` builds a `Paint` with `ColorMatrix`: `setSaturation(0f)` for grayscale, post-concat of invert matrix `[-1,0,0,0,255, 0,-1,0,0,255, 0,0,-1,0,255, 0,0,0,1,0]` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:810`). `ReaderNavigationOverlayView.kt:49-60` paints tap zones: `regionPaint`, `textPaint` (center, white, size 64), `textBorderPaint` (center, black, size 64, STROKE, width 8). Files 2, refs 8.
- **Rust:** color filters: a GLSL `gtk::GLShader`/`gsk::GLShader` is deprecated since 4.16; use `gtk::Snapshot::push_color_matrix(&graphene::Matrix, &graphene::Vec4)` with the grayscale matrix (saturation 0: luma weights 0.213, 0.715, 0.072 per row) and invert (`scale -1`, offset `1` per RGB channel, since GTK matrices work in 0..1: matrix = diag(-1,-1,-1,1), offset = (1,1,1,0)). Overlay: `gtk::DrawingArea` with `cairo` (`set_source_rgba`, `show_text` centered, stroke text width 8 then fill).
- **Preserve:** both filters can combine (grayscale first, then invert); overlay text size 64 and stroke 8 relative to screen.
- **Difficulty:** moderate.

### PhotoView
- **Does:** zoomable view for *animated* images (GIF/WebP/AVIF animations) in paged mode: `PhotoView(context)` with `setScaleLevels(1F, 2F, MAX_ZOOM_SCALE)` and a double-tap listener that toggles scale 1 <-> 2 at the tap point with animation (`ReaderPageImageView.kt:359-380`). Webtoon uses plain `AppCompatImageView`. `MAX_ZOOM_SCALE = 5F` (`ReaderPageImageView.kt:442`). Files 1, refs 6 (static images use `SubsamplingScaleImageView`).
- **Rust:** one custom `ZoomPicture` widget (a `gtk::Widget` with `gtk::GestureZoom`, `gtk::GestureDrag`, `gtk::GestureClick` n_press=2) built on `gtk::Picture` + `gdk_pixbuf::PixbufAnimation` for animated frames. Scale levels min 1, mid 2, max 5; double-tap -> `if scale > 1 { scale = 1 } else { scale = 2 }` about the tap point, animated (`adw`-free: `glib::timeout_add_local` at 60 fps, 200 ms).
- **Difficulty:** hard (gesture and tile handling shared with SubsamplingScaleImageView replacement, area:reader).

### PointF
- **Does:** float point. Tap hit-test `ViewerNavigation.getAction(pos: PointF)`: finds the first region whose `rectF` contains `(x,y)` after inversion, else `MENU` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt:59`); landscape auto-zoom target points `PointF(0,0)`, `PointF(sWidth,0)`, centre in `ReaderPageImageView.kt:135-145`. Files 4, refs 11. The landscape zoom: after a 500 ms delay, `targetScale = height / sHeight`, `animateScaleAndCenter(...).withDuration(500).withEasing(EASE_IN_OUT_QUAD).withInterruptible(true)`; start side depends on `ZoomStartPosition.LEFT/RIGHT/CENTER` and navigation direction (forward/backward swaps).
- **Rust:** `graphene::Point` or `(f32, f32)`; ease-in-out-quad `t < 0.5 ? 2t^2 : -1 + (4 - 2t) t`.
- **Preserve:** 500 ms delay and 500 ms animation; interruptible by touch.
- **Difficulty:** trivial.

### Rect
- **Does:** integer rect. Image splitting in `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt`: `splitInHalf(side)` crops `Rect(0,0,w/2,h)` for LEFT or `Rect(w - w/2, 0, w, h)` for RIGHT; `splitAndMerge(upperSide)` builds a `width/2 x height*2` bitmap (upper = chosen side, bottom = the other); both output JPEG quality 100; `rotateImage` uses `Matrix.postRotate(degrees)`. Webtoon touch redirect `WebtoonFrame.kt:57` shrinks recycler hit rect by `inset(1,1)` and clamps touch events into it; `ViewExtensions.kt:92` `isVisibleOnScreen` (global visible rect intersects screen); PiP source rect hint (`PlayerActivity.kt:282`). Files 4, refs 15.
- **Rust:** `image` crate (`DynamicImage::crop_imm`, `imageops::rotate90/180/270`, `ImageBuffer` for the merge) with `JpegEncoder::new_with_quality(w, 100)`; arbitrary angle via `imageproc::geometric_transformations::rotate_about_center`. WebtoonFrame clamp: GTK handles gestures on a scrolled window, so a gesture controller attached to the container with propagation phase `CAPTURE` replaces it. `isVisibleOnScreen`: `gtk::Widget::compute_bounds` against the root bounds.
- **Preserve:** integer halving rule (`w/2`, right = `w - w/2` so odd widths lose at most 1 px); JPEG quality 100.
- **Difficulty:** moderate.

### RectF
- **Does:** normalised (0..1) rectangles for tap zones. `ViewerNavigation.constantMenuRegion = RectF(0f, 0f, 1f, 0.05f)`. Layouts (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/`): `EdgeNavigation` PREV (0,0,0.33,1), MENU (0.33,0.66,0.66,1), NEXT (0.66,0,1,1); `KindlishNavigation` NEXT (0.33,0.33,1,1), PREV (0,0.33,0.33,1); `LNavigation` PREV (0,0.33,0.33,0.66), PREV (0,0,1,0.33), NEXT (0.66,0.33,1,0.66), NEXT (0,0.66,1,1); `RightAndLeftNavigation` LEFT (0,0,0.33,1), RIGHT (0.66,0,1,1); `DisabledNavigation` no regions. Overlay colors: PREV `0xCC FF7733`, NEXT `0xCC84E296`, LEFT `0xCC7D1128`, RIGHT `0xCCA6CFD5`. Inversion via `RectF.invert(TappingInvertMode)` in `app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt:6` (enum `NONE`, `HORIZONTAL`, `VERTICAL`, `BOTH` in `ReaderPreferences.kt:183`). `ReaderPageImageView.canPan` uses `view.getPanRemaining(RectF)`: can pan if `remaining > 1` px in the given edge. `Rect.contains(x,y)` is half-open (left/top inclusive, right/bottom exclusive). Files 7, refs 19.
- **Rust:** `struct RectF { l, t, r, b: f32 }` with `contains(x,y) = l <= x && x < r && t <= y && y < b` and `invert(mode)` (HORIZONTAL: `l' = 1 - r, r' = 1 - l`; VERTICAL: same on t/b; BOTH: both). Region table as `const` arrays using the numbers above; first match wins; fall back to MENU. Pan: `remaining_edge > 1.0`.
- **Preserve:** first-match order, half-open containment, 5% constant menu strip is a no-op (both branches return MENU).
- **Difficulty:** trivial.

### RecyclerView
- **Does:** (1) webtoon reader: `WebtoonRecyclerView` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt`) with pinch zoom (`currentScale`, `DEFAULT_RATE`, `MIN_RATE` unless `zoomOutDisabled`, animator duration `ANIMATOR_DURATION_TIME`, `zoomFling`, max pan `halfWidth * (currentScale - 1)`), `WebtoonLayoutManager` (`NO_POSITION`), `WebtoonAdapter`, `WebtoonBaseHolder`, `WebtoonViewer`; (2) download queue (`ui/download/manga/MangaDownloadItem.kt`, `MangaDownloadHeaderItem.kt`, anime twins) via FlexibleAdapter with drag-reorder holders and a per-item `popupMenu`. Files 8, refs 21.
- **Rust:** (1) a custom `WebtoonView`: `gtk::ScrolledWindow` (kinetic scrolling) + `gtk::ListView` over page models with `gtk::GestureZoom` applying a `gsk::Transform` scale to the content box; pre-size rows from known image dimensions to avoid jumps; (2) `gtk::ListView` + `gtk::DragSource`/`gtk::DropTarget` rows grouped by source with section headers (`gtk::ListView` header factory).
- **Preserve:** zoom bounds and pan clamping, fling inertia when zoomed, reorder semantics (drag within queue, "move to top/bottom" menu).
- **Difficulty:** hard.

### PagerAdapter
- **Does:** `ViewPagerAdapter` (`app/src/main/java/eu/kanade/tachiyomi/widget/ViewPagerAdapter.kt:7`): `instantiateItem` -> `createView` + `addView`; `destroyItem` -> `destroyView` + `removeView`; `isViewFromObject` identity; nested `PositionableView { item }`. Subclassed by `PagerViewerAdapter` (`ui/reader/viewer/pager/PagerViewerAdapter.kt:18`), implemented by `PagerPageHolder` and `PagerTransitionHolder`. Files 1, refs 2.
- **Rust:** `adw`-free pager: `gtk::Stack`/`gtk::Carousel` is libadwaita-only, so build `PagerView` from `gtk::ScrolledWindow` with snap-to-page (`gtk::Adjustment` animation) or a `gtk::Overlay` showing one page widget at a time with pre-created neighbours (current-1, current, current+1). Widget recycle callbacks map to `create_page(idx)` / `destroy_page(idx)`.
- **Difficulty:** moderate.

### OnItemSelectedListener
- **Does:** `IgnoreFirstSpinnerListener(block: (Int) -> Unit)` swallows the first `onItemSelected` event that Android Spinner fires on layout, then forwards `block(position)` (`app/src/main/java/eu/kanade/tachiyomi/widget/listener/IgnoreFirstSpinnerListener.kt:7`). No other file references the class (dead code). Files 1, refs 2.
- **Rust:** drop. `gtk::DropDown` emits `notify::selected` only on change; set the initial selection before connecting the handler.
- **Difficulty:** trivial.

### PopupMenu
- **Does:** `View.popupMenu(menuRes, initMenu, onMenuItemClick)` inflates an XML menu, lets the caller modify items, shows it (`app/src/main/java/eu/kanade/tachiyomi/util/view/ViewExtensions.kt:70`). Only callers: `ui/download/manga/MangaDownloadHolder.kt:89` and `ui/download/anime/AnimeDownloadHolder.kt:99`. Files 1, refs 3.
- **Rust:** `gtk::PopoverMenu::from_model(Some(&gio::Menu))` with `gio::SimpleAction`s; menu items for the download row: the reorder/cancel actions from the menu resource (see area:downloads).
- **Difficulty:** trivial.

---

## Player platform integration

### PictureInPictureParams
- **Does:** `PlayerActivity.createPipParams()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:764`): on API 33+ sets title (anime title) and subtitle (episode name); on API 31+ `setAutoEnterEnabled` and `setSeamlessResizeEnabled` when `player.paused == false && pipOnExit`; actions from `createPipActions`; `setSourceRectHint(pipRect)`; aspect ratio only when `0.42 <= h/w <= 2.38`. Back press enters PiP when `isPipSupportedAndEnabled && !paused && pipOnExit` else `finish()`. On entering PiP: hide controls, seek bar, brightness and volume sliders, close sheets. Files 1, refs 3.
- **Rust:** no system PiP on desktop Linux. Proposal: a "mini player" mode: the video `gtk::Window` switches to `set_decorated(false)`, keeps aspect ratio by constraining size on resize, `set_keep_above` is unavailable under Wayland, so on X11 use the `_NET_WM_STATE_ABOVE` hint via `gdk4-x11`, on Wayland the window stays a normal window (users can use KDE "Keep above"). Hide controls on entry like Aniyomi.
- **Loses:** true system overlay PiP on Wayland. **Difficulty:** moderate.

### Rational
- **Does:** `Rational(height, width).toFloat()` guard for PiP ratio, then `Rational(width, height)` (`PlayerActivity.kt:792-793`). Files 1, refs 3.
- **Rust:** `f64` ratio; apply the same `[0.42, 2.38]` clamp when sizing the mini player; otherwise leave the window size free.
- **Difficulty:** trivial.

### RemoteAction
- **Does:** PiP window buttons (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt:38,91`): [first] `PIP_PREVIOUS` (3, enabled when `playlistPosition != 0`) when `pipReplaceWithPrevious` else `PIP_SKIP` (5, +10 s); [second] `PIP_PLAY` (2) when paused else `PIP_PAUSE` (1); [third] `PIP_NEXT` (4, enabled when `playlistPosition != playlistCount - 1`). Each is a `PendingIntent.getBroadcast` with action `"pip_control"` and extra `"media_control"`; the receiver maps to `pause()`, `unpause()`, `changeEpisode(false/true)`, `seekBy(10)` and refreshes the params. Files 1, refs 4.
- **Rust:** mini-player overlay buttons (`gtk::Button`) calling the same commands; same enable/disable rules; same constants if state is ever serialised (1..5).
- **Difficulty:** trivial.

### PlaybackState
- **Does:** `MediaSession` playback state with actions `ACTION_PLAY | ACTION_PAUSE | ACTION_STOP | ACTION_SKIP_TO_PREVIOUS | ACTION_SKIP_TO_NEXT` (`PlayerActivity.kt:970`); callbacks map next/previous to mpv keypresses `MediaNext`/`MediaPrevious` or `changeEpisode`; `ACTION_AUDIO_BECOMING_NOISY` receiver pauses playback. Files 1, refs 7.
- **Rust:** MPRIS2 over D-Bus with `mpris-server` or `zbus` (`org.mpris.MediaPlayer2.kurobayomi`; `Player` properties `PlaybackStatus`, `CanPlay`, `CanPause`, `CanGoNext`, `CanGoPrevious`, `Metadata` with `xesam:title` = episode name, `xesam:album` = anime title; methods `Play`, `Pause`, `PlayPause`, `Stop`, `Next`, `Previous`). KDE media keys and the Plasma media widget then work. "Becoming noisy" (headphones unplugged): optional via PipeWire/`logind` event; Proposal: drop unless PipeWire sink-removed is easy (`pipewire` crate), loses auto-pause on unplug.
- **Difficulty:** moderate.

---

## Localization (Moko resources)

### Plural
- **Does:** `StringDesc.Plural(resource, count).toString(context).fixed()` (`core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt:21`). `fixed()` replaces `\"` with `"` (workaround for moko-resources issue 337). Files 1, refs 2. About 64 call sites use `pluralStringResource`.
- **Rust:** `fluent-bundle` (Mozilla Fluent) with CLDR plural categories, or `gettext-rs` `ngettext` with `.po` plural forms. Pick `gettext` if translations are imported from Aniyomi's Android XML (convert `<plurals>` with `quantity=one/other/...`).
- **Preserve:** quantity selection by locale rules (`zero, one, two, few, many, other`); unescape `\"`.
- **Difficulty:** moderate (import tooling).

### PluralFormatted
- **Does:** plural plus `String.format` args: `StringDesc.PluralFormatted(resource, count, *args)` (`Localize.kt:25`). Files 1, refs 2.
- **Rust:** Fluent `{ $count }` placeables or `format!` after `ngettext`. Convert Android `%1$d` / `%s` to named args in the import tool.
- **Difficulty:** trivial.

### PluralsResource
- **Does:** typed key of an Android `<plurals>` entry. Declared as parameter in `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt:20,24` and `presentation-core/src/main/java/tachiyomi/presentation/core/i18n/Localize.kt:25,31` (Compose `pluralStringResource`). Files 2, refs 4.
- **Rust:** a `const` message id (`&'static str`) or generated `Msg` enum (build script from the `.ftl` or `.po`).
- **Difficulty:** trivial.

---

## Summary table

| Item | Files | Refs | Decision | Difficulty |
|---|---|---|---|---|
| Observer | 1 | 2 | drop (Rx shim) | trivial |
| OkHttpClient | 26 | 45 | `reqwest` + middleware | moderate |
| OkHttpNetworkFetcherFactory | 1 | 2 | custom image loader | moderate |
| OnItemSelectedListener | 1 | 2 | drop (dead code) | trivial |
| OneTimeWorkRequestBuilder | 11 | 11 | tokio JobRunner | moderate |
| OpenableColumns | 1 | 2 | `Path::file_name` | trivial |
| Options | 7 | 17 | `ImageOptions` + cache keys | trivial |
| Os | 2 | 4 | `memmap2`, `zip` | moderate |
| OsConstants | 1 | 2 | `memmap2` | trivial |
| PackageInfo | 4 | 10 | `ExtensionManifest` | moderate |
| PackageInfoCompat | 4 | 12 | `u64` version code | trivial |
| PackageInstaller | 2 | 15 | single private installer | trivial |
| PackageManager | 12 | 31 | extension registry | moderate |
| Pager | 3 | 5 | `PagedLoader` | moderate |
| PagerAdapter | 1 | 2 | custom pager widget | moderate |
| PagingConfig | 3 | 3 | const 25 | trivial |
| PagingData | 1 | 2 | `gio::ListStore` | trivial |
| PagingSource | 9 | 9 | `PageSource` trait | moderate |
| PagingState | 5 | 5 | drop | trivial |
| Paint | 2 | 8 | GTK snapshot color matrix, cairo | moderate |
| ParcelFileDescriptor | 3 | 3 | `std::fs::File` | trivial |
| PendingIntent | 13 | 167 | `gio::Notification` + actions | moderate |
| PeriodicWorkRequestBuilder | 3 | 3 | scheduler + UPower/NM | moderate |
| PermissionChecker | 2 | 6 | drop | trivial |
| PhotoView | 1 | 6 | custom zoom widget | hard |
| PictureInPictureParams | 1 | 3 | mini-player mode | moderate |
| PlaybackState | 1 | 7 | MPRIS2 | moderate |
| Plural | 1 | 2 | fluent/gettext | moderate |
| PluralFormatted | 1 | 2 | fluent/gettext | trivial |
| PluralsResource | 2 | 4 | message id const | trivial |
| PointF | 4 | 11 | `(f32,f32)` | trivial |
| PopupMenu | 1 | 3 | `gtk::PopoverMenu` | trivial |
| PowerManager | 2 | 2 | drop (+ logind inhibit) | trivial |
| Precision | 3 | 3 | loader enum | trivial |
| PreferenceDataStore | 1 | 2 | `SourcePrefStore` | trivial |
| PreferenceFragmentCompat | 2 | 2 | `SourceSettingsPage` | moderate |
| PreferenceManager | 24 | 24 | new settings table | trivial |
| PreferenceScreen | 2 | 2 | `PrefSpec` root | trivial |
| Process | 2 | 2 | drop (Shizuku) | trivial |
| ProcessLifecycleOwner | 1 | 6 | tokio + gio::Application | trivial |
| Producer | 1 | 2 | drop (Rx shim) | trivial |
| ProfileVerifier | 1 | 11 | drop | trivial |
| Project | 4 | 14 | `build.rs`/`vergen` | trivial |
| QuantizerCelebi | 1 | 2 | drop (unused) | trivial |
| Query | 6 | 40 | rusqlite actor + change bus | hard |
| QuickJs | 1 | 2 | `rquickjs` | moderate |
| Rational | 1 | 3 | `f64` clamp | trivial |
| Rect | 4 | 15 | `image` crate | moderate |
| RectF | 7 | 19 | const region tables | trivial |
| RecyclerView | 8 | 21 | `gtk::ListView` + custom webtoon | hard |
| RemoteAction | 1 | 4 | overlay buttons | trivial |
| RequerySQLiteOpenHelperFactory | 1 | 3 | rusqlite `bundled` + pragmas | trivial |
| Request | 13 | 49 | `reqwest::Request` helpers | trivial |
| RequestBody | 3 | 9 | `.form()` | trivial |
