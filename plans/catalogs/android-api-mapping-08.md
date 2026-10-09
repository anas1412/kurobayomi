# Android/Java/third-party API mapping, chunk 8 of 8

Chunk 8 of 8 of the Android API catalog. It covers 56 import-derived class names, from `TTFFile` (first) to `XmlVersion` (last), in the order printed by `inventory.py items imports 8 8`. All paths are relative to `/home/blackbox/aniyomi-src`. "Usage" is the number of Kotlin files that import the FQN, counted with `rg -l "import <fqn>"`, followed by the representative `path:line`. Difficulty is trivial, moderate or hard. Refer to other areas as area:<area-id>; the ones used here are area:12-network-stack (HTTP, cookies, Cloudflare), area:reader (reader viewers), area:player (mpv player), area:downloads, area:library-updates, area:backup, area:storage and area:settings.

Crate shorthand used below: `gtk4` = gtk4-rs 0.11 (GTK 4.16+, no libadwaita), `gio`/`glib` = gtk-rs companions, `tokio`, `rusqlite`, `reqwest`, `serde`, `quick-xml`, `zip`, `ttf-parser`, `libmpv2`, `notify-rust`, `ashpd` (XDG portals), `webkit6` (WebKitGTK 6.0 bindings), `directories`, `nix`/`rustix`.

## Summary table

| Item | Verdict | Replacement |
|---|---|---|
| TTFFile | replace | `ttf-parser` crate |
| Task | drop (build only) | Cargo `build.rs` / `cargo xtask` |
| TaskProvider | drop (build only) | `build.rs` |
| Test | replace | `#[test]` + `cargo test` |
| TestFactory | replace | table-driven `#[test]` or `rstest` |
| TestLogEvent | drop | `cargo test` output |
| Text | drop (Android widget) | none (no home-screen widgets on Linux) |
| TextAlign | drop | none |
| TextInputEditText | replace | `gtk::Entry`/`gtk::Text` + incognito flag |
| TextStyle | drop | none |
| Throttler | replace | custom token-bucket on the `reqwest` byte stream |
| Toast | replace | in-window `gtk::Revealer` toast overlay |
| TypedValue | drop | none |
| UiModeManager | replace/drop | `gtk::Settings` / XDG portal; TV detection dropped |
| UniFile | replace | `std::path::PathBuf` + `std::fs` + `gio::File` for picker results |
| Uri | replace | `url::Url` + `std::path::Path` |
| ValueAnimator | replace | `libadwaita`-free: `gtk::Widget::add_tick_callback` + easing fn |
| VerticalGrid | replace | `gtk::Grid` |
| View | replace | `gtk::Widget` |
| ViewAnimationUtils | replace | tick-callback circular clip, or plain fade |
| ViewCompat | drop | none |
| ViewConfiguration | replace | `gtk::Settings` (`gtk-double-click-time`, `gtk-dnd-drag-threshold`) |
| ViewGroup | replace | `gtk::Box`/`gtk::Overlay`/`gtk::Fixed` |
| ViewModel | replace | plain Rust structs + `Rc<RefCell<_>>`/`glib::Object` + tokio tasks |
| ViewModelProvider | drop | construct directly |
| ViewPager | replace | custom `PagedStrip` widget over `gtk::Stack` |
| ViewPropertyAnimator | replace | `gtk::Widget::set_opacity` + tick animation |
| ViewSizeResolver | replace | widget `allocation()` passed to image decode |
| WRAP_CONTENT | replace | default GTK size negotiation (`halign/valign`) |
| WallpaperManager | drop | none (no Monet) |
| WebResourceError | replace | `webkit6::LoadEvent`/`load-failed` signal |
| WebResourceRequest | replace | `webkit6::URIRequest` |
| WebSettings | replace | `webkit6::Settings` |
| WebStorage | replace | `webkit6::WebsiteDataManager` |
| WebView | replace | `webkit6::WebView` |
| WebViewClient | replace | `webkit6::WebView` signals |
| WebtoonLayoutManager | replace | custom vertical strip widget in `gtk::ScrolledWindow` |
| WifiManager | replace | NetworkManager D-Bus via `zbus` (or `gio::NetworkMonitor`) |
| Window | replace | `gtk::Window` / `gdk::Surface` |
| WindowCompat | replace | `gtk::Window::fullscreen()` |
| WindowInsetsCompat | drop | none |
| WindowInsetsControllerCompat | replace | `gtk::Window::fullscreen()/unfullscreen()` |
| WindowManager | replace | inhibit via `gtk::Application::inhibit`, secure screen dropped |
| WorkInfo | replace | in-process `JobState` enum |
| WorkManager | replace | tokio scheduler + systemd user timer (optional) |
| WorkQuery | replace | in-process job registry query |
| WorkerParameters | replace | plain job struct fields |
| XML | replace | `quick-xml` + `serde` |
| XmlElement | replace | `#[serde(rename)]` |
| XmlPullParser | drop | static locale list |
| XmlSerialName | replace | `#[serde(rename)]` |
| XmlValue | replace | `#[serde(rename = "$text")]` |
| XmlVersion | drop | `quick-xml` writes `<?xml version="1.0"?>` |

## Items

### TTFFile
- Does: `com.yubyf.truetypeparser.TTFFile` reads the font family name out of `.ttf`/`.otf` files the user dropped in the mpv `fonts` directory, to fill the subtitle font dropdown. `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt:109` is the only use (1 file): `runCatching { TTFFile.open(it.openInputStream()).families.values.first() }.getOrNull()`.
- Observed: files are listed from `StorageManager.getFontsDirectory()` (`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:77`, directory name `fonts` under `mpv-config`), filtered by `FONT_EXTENSION_REGEX = Regex(""".*\.[ot]tf$""")` (line 354, applied to the lower-cased name), unreadable files are skipped, results are `distinct()`.
- Replacement: Proposal: `ttf-parser` crate (`Face::parse`, then `face.names()` filtered on `name_id::FAMILY`, take the first Unicode-decodable). Scan `<data>/mpv-config/fonts` on a tokio blocking task. Also pass the directory to mpv as `sub-fonts-dir` and `osd-fonts-dir` (as `PlayerActivity.kt:530` does after copying) -- on Linux no copy is needed because mpv can read the directory directly.
- Preserve: case-insensitive `.ttf`/`.otf` match only (`.ttc`, `.woff` are ignored), silent skip of broken files, de-duplication, first family only.
- Difficulty: trivial. Risk: `.otf` CFF fonts with only localized names return no English family; fall back to the file stem.

### Task
- Does: `org.gradle.api.Task` is the return type of the Gradle task `generateLocalesConfig` in `buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt:4` (1 file). Build-time only.
- Observed behavior of that task: scans `src/commonMain/moko-resources/**/strings.xml`, drops files matching `<resources>\s*</resources>|<resources\s*/>`, maps the parent folder name with `.replace("base","en").replace("-r","-").replace("+","-")`, sorts, and writes `xml/locales_config.xml` with one `<locale android:name="..."/>` per language.
- Replacement: Proposal: not needed on Linux. The language list for the picker is produced at build time by `build.rs` (or a const array) from the `.po`/`.ftl` files that Kurobayomi ships; see `XmlPullParser` below.
- Difficulty: trivial (drop). Loses nothing for the user.

### TaskProvider
- Does: `org.gradle.api.tasks.TaskProvider` is the lazy handle returned by `Project.getLocalesConfigTask` (`LocalesConfigTask.kt:5`, 1 file); callers `i18n/build.gradle.kts:53` and `i18n-aniyomi/build.gradle.kts:54` wire it into resource generation.
- Replacement: Proposal: none; Cargo `build.rs` runs in order automatically. Drop.
- Difficulty: trivial.

### Test
- Does: three distinct FQNs. `org.gradle.api.tasks.testing.Test` (`buildSrc/src/main/kotlin/mihon/buildlogic/ProjectExtensions.kt:9`) configures every Gradle test task with `useJUnitPlatform()` and `testLogging { events(PASSED, SKIPPED, FAILED) }`. `org.junit.jupiter.api.Test` (8 files, e.g. `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt:10`) marks unit tests. `org.junit.Test` (2 files, `macrobenchmark/src/main/java/tachiyomi/macrobenchmark/StartupBenchmark.kt:26` and `BaselineProfileGenerator.kt`) are Android instrumentation benchmarks.
- Test files (the full set): `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`, `.../library/model/LibraryFlagsTest.kt`, `.../items/service/MissingItemsTest.kt`, `.../items/chapter/service/ChapterRecognitionTest.kt`, `.../entries/manga/interactor/MangaFetchIntervalTest.kt`, `.../entries/anime/interactor/AnimeFetchIntervalTest.kt`, `app/src/test/java/mihon/core/migration/MigratorTest.kt`, `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt`.
- Replacement: Proposal: `#[test]` functions in `#[cfg(test)] mod tests`, run by `cargo test`. Port these 8 test classes first because they document edge cases of release comparison, library flag bit layout, missing-item detection, chapter-number recognition, fetch-interval computation, migrations and torrent parsing. The macrobenchmark pair is dropped (Android startup profiling); use `cargo bench`/`criterion` if startup matters.
- Difficulty: trivial. Risk: pass/skip/fail event logging is irrelevant.

### TestFactory
- Does: `org.junit.jupiter.api.TestFactory` (1 file, `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt:8`) generates `DynamicTest`s: one per `TestCase(inputFile, expected: Torrent)`, parsing `.torrent` fixtures through `TorrentHelpers.parseTorrentDetailsFromTorrentFileContent`.
- Replacement: Proposal: a `for case in cases { assert_eq!(parse(case.input), case.expected) }` loop in a single `#[test]`, or the `rstest` crate with `#[case]`. Parse bencode with `serde_bencode` or `lava_torrent`. Copy the fixture `.torrent` files under `core/common/src/test/resources`.
- Difficulty: trivial.

### TestLogEvent
- Does: `org.gradle.api.tasks.testing.logging.TestLogEvent` (1 file, `ProjectExtensions.kt:10`) selects which events Gradle prints: `PASSED`, `SKIPPED`, `FAILED`.
- Replacement: drop; `cargo test` prints these by default (`-- --nocapture` for output).
- Difficulty: trivial.

### Text
- Does: `androidx.glance.text.Text` (4 files) renders text inside the Glance home-screen widgets: `presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt:21` (empty-state text `MR.strings.information_no_recent`, style sets only the colour) and `LockedMangaWidget.kt` (text `MR.strings.appwidget_unavailable_locked`, 12.sp, centered), plus the `anime` twins `UpdatesAnimeWidget.kt` and `LockedAnimeWidget.kt`.
- Observed (whole widget feature, because every text item belongs to it): `BaseMangaUpdatesGridGlanceWidget.kt:59-125` shows a grid of the newest unread-update covers: `SizeMode.Exact`, `DateLimit = now - 3 months`, `distinctBy mangaId`, `take(rowCount * columnCount)`, locked placeholder if `SecurityPreferences.useAuthenticator()`. `MangaWidgetManager.kt` re-renders when the set of `chapterId`s or the lock flag changes.
- Replacement: Proposal: drop. Linux/KDE has no Android-style home-screen widgets. User loses: glance at new-chapter covers from the home screen. Mitigation: the Updates tab (area:updates) and a tray/notification (`notify-rust`) from the library-update job.
- Difficulty: trivial (drop). Priority P3.

### TextAlign
- Does: `androidx.glance.text.TextAlign` (2 files, `LockedMangaWidget.kt:15`, `LockedAnimeWidget.kt`) centers the "locked" widget text (`TextAlign.Center`).
- Replacement: drop with the widget. If ever recreated, `gtk::Label::set_justify(Justification::Center)`.
- Difficulty: trivial.

### TextInputEditText
- Does: `com.google.android.material.textfield.TextInputEditText` is the base class of `TachiyomiTextInputEditText` (`app/src/main/java/eu/kanade/tachiyomi/widget/TachiyomiTextInputEditText.kt:7`, 1 file). It observes `BasePreferences.incognitoMode().changes()` while attached and sets/clears `EditorInfoCompat.IME_FLAG_NO_PERSONALIZED_LEARNING` in `imeOptions`. It is applied only through `setIncognito(scope)` on the EditTexts of extension source preferences (`MangaSourcePreferencesScreen.kt:156`, `AnimeSourcePreferencesScreen.kt`).
- Replacement: Proposal: `gtk::Entry`/`gtk::PasswordEntry` for source preference text fields. The IME flag maps to `gtk::Entry::set_input_hints(InputHints::PRIVATE)` (GTK 4 `NO_EMOJI`/`PRIVATE` hints, honoured by IBus/Fcitx on Wayland text-input-v3) bound to the incognito setting via a `glib` property binding. Remove binding on `unrealize`.
- Preserve: toggling incognito while the field is open must update it live.
- Difficulty: trivial. Risk: many Linux IMEs ignore the hint; nothing else can be done.

### TextStyle
- Does: `androidx.glance.text.TextStyle` (4 files, `UpdatesMangaWidget.kt:22`) carries colour, size (`12.sp`) and alignment for widget text. (`java.time.format.TextStyle` used in `app/src/main/java/mihon/feature/upcoming/components/calendar/Calendar.kt:30` is a different class: `TextStyle.NARROW` day-of-week names; port that with `chrono` + `pure-rust-locales`/`icu`.)
- Replacement: drop with widget.
- Difficulty: trivial.

### Throttler
- Does: `okio.Throttler` (1 file, `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:47`). A single field `private val throttler = Throttler()` (line 114). In `downloadImage` (lines 505-535) before saving each page it calls `throttler.apply { bytesPerSecond(downloadPreferences.downloadSpeedLimit().get().toLong() * 1024) }`, then wraps `response.body.source()` with `throttler.source(...).buffer()` and `saveTo(file.openOutputStream())`.
- Preference: `downloadSpeedLimit` = `preferenceStore.getInt("download_speed_limit", 0)` (`domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:70`), unit KiB/s, `0` = unlimited, UI in `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt:62-100,394-422` with `OutlinedNumericChooser(step = 100, min = 0, suffix = "KiB/s")`, subtitle `"$speedLimit KiB/s"` or `MR.strings.off`.
- Behavior to preserve: one throttler instance is shared by all concurrent page downloads of that downloader (the limit is a global budget, not per file); `okio.Throttler` defaults `maxByteCount` and `waitByteCount` (8 KiB and 64 KiB) so bursts are small; a changed preference applies from the next page. Only manga images are throttled; the anime downloader does not use `Throttler` (it delegates to ffmpeg/aria2-style external downloader).
- Replacement: Proposal: a token-bucket `struct Throttle { bytes_per_sec: AtomicU64, state: Mutex<Bucket> }` in the downloader crate; wrap the `reqwest` `bytes_stream()` so each chunk first `acquire(chunk.len()).await` (sleep via `tokio::time::sleep_until`). Bucket capacity 64 KiB. Limit 0 skips the wrapper. Share one `Arc<Throttle>` across tasks.
- Difficulty: moderate. Risk: accounting must include only body bytes; do not hold the mutex across `.await`.

### Toast
- Does: `android.widget.Toast` (6 files). `core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt:4` defines `Context.toast(StringResource|String?, duration = Toast.LENGTH_SHORT, block)` which calls `Toast.makeText(applicationContext, text.orEmpty(), duration).also { block(it); it.show() }`. About 110 call sites call `.toast(`. `LENGTH_LONG` is used for `information_webview_required` (`WebViewInterceptor.kt:62`, `WebViewActivity.kt:56`), `information_webview_outdated` (`CloudflareInterceptor.kt:126`), and mpv playback errors (`PlayerObserver.kt:45`, text is `err` plus `: $httpError` if the last mpv log line contained "HTTP error"). `ReaderActivity.kt:21` imports it too.
- Replacement: Proposal: `ToastOverlay` widget: a `gtk::Overlay` child at bottom-center containing a `gtk::Revealer` (`SlideUp`, 150 ms) with a `gtk::Label` in a `.osd` styled box; queue FIFO; short = 2000 ms, long = 3500 ms (Android's values); latest message replaces the current one only if the queue is empty, otherwise wait. API: `toast(&self, text: &str, long: bool)`. For background jobs that finish while the window is hidden, also send `notify-rust` desktop notification.
- Difficulty: trivial. Risk: callers on non-UI threads must go through `glib::MainContext::invoke`.

### TypedValue
- Does: `android.util.TypedValue` (2 files, `MangaSourcePreferencesScreen.kt:5`/`:125`, `AnimeSourcePreferencesScreen.kt:5`/`:125`) resolves theme attribute `R.attr.preferenceTheme` so the AndroidX `PreferenceFragmentCompat` gets the app's preference theme: `superCtx.theme.resolveAttribute(R.attr.preferenceTheme, tv, true); ContextThemeWrapper(superCtx, tv.resourceId)`.
- Replacement: drop. GTK styling comes from the user's GTK theme. The source-preference screen is rebuilt from the `ConfigurableSource` preference tree; see area:extensions for the preference widgets.
- Difficulty: trivial.

### UiModeManager
- Does: `android.app.UiModeManager` (2 files). (1) `app/src/main/java/eu/kanade/tachiyomi/util/system/TvUtils.kt:3,15` `isTvBox(context)`: true if mode type is `UI_MODE_TYPE_TELEVISION`, or no activity can resolve `ACTION_OPEN_DOCUMENT` for `video/*` (missing DocumentsUI), or on SDK < 30: no touchscreen, `android.hardware.hdmi.cec`, or manufacturer "zidoo". Used once, `app/src/main/java/eu/kanade/presentation/more/onboarding/StorageStep.kt:49`: on TV boxes the storage step offers a "create folder" button (`folderProvider.directory().mkdirs()`) instead of the SAF picker. (2) `app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt:79` reads `uiModeManager.contrast` (API 34+, else 0.0) as the `contrast` argument of `SchemeContent` for the seeded Monet scheme.
- Replacement: Proposal: (1) drop TV detection; Linux always uses a folder chooser (`gtk::FileDialog::select_folder`), with default `~/.local/share/kurobayomi` created silently if the user declines. (2) Contrast: read `org.freedesktop.appearance contrast` through `ashpd::desktop::settings::Settings` (0 = no preference, 1 = more); use it only to bump the app's own accent contrast, or ignore.
- Difficulty: trivial. User loses: Android TV workaround, irrelevant.

### UniFile
- Does: `com.hippo.unifile.UniFile` (44 import sites, including `androidMain`/`commonMain` duplicates) abstracts `File` and Storage Access Framework `content://` documents. Methods actually used: `fromUri/fromFile`, `createDirectory`, `createFile`, `findFile`, `listFiles`, `openInputStream`, `openOutputStream`, `renameTo`, `delete`, `exists`, `isDirectory`, `isFile`, `name`, `filePath`, `uri`, `length`, `lastModified`, `parentFile`. Representative: `source-local/src/commonMain/kotlin/tachiyomi/source/local/io/manga/LocalMangaSourceFileSystem.kt:3`.
- Files that import it: `source-local/src/{commonMain,androidMain}/kotlin/tachiyomi/source/local/**` (file systems, cover/thumbnail/background managers, `Format.kt`, `Archive.kt`, `LocalMangaSource.kt`, `LocalAnimeSource.kt`), `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt`, `core/archive/src/main/kotlin/mihon/core/archive/{ZipWriter,UniFileExtensions}.kt`, `core/common/src/main/java/{tachiyomi/core/common/util/system/ImageUtil.kt, eu/kanade/tachiyomi/util/storage/{FFmpegUtils,DiskUtil}.kt, tachiyomi/core/common/storage/{UniFileTempFileManager,UniFileExtensions}.kt}`, `app/src/main/java/eu/kanade/tachiyomi/util/{StorageUtil.kt,system/ContextExtensions.kt}`, downloaders/providers/caches under `data/download/{manga,anime}/`, `data/coil/{MangaCoverFetcher,AnimeImageFetcher}.kt`, `ui/reader/loader/{DownloadPageLoader,DirectoryPageLoader}.kt`, `ui/reader/ReaderActivity.kt`, `ui/player/PlayerActivity.kt`, `data/backup/create/{BackupCreator,BackupCreateJob}.kt`, `data/backup/BackupNotifier.kt`, `SettingsDataScreen.kt`, `CodeEditScreenModel.kt`.
- Helper semantics to port: `UniFile.extension = name?.substringAfterLast('.')`, `nameWithoutExtension = name?.substringBeforeLast('.')`, `displayablePath = filePath ?: uri.toString()` (`core/common/src/main/java/tachiyomi/core/common/storage/UniFileExtensions.kt`); `UniFile.size()` recursive sum clamping negative lengths to 0 and `Long.toSize()` using base 1000 with formats `"%.2f GB"`, `"%.2f MB"`, `"%.2f KB"`, `"$this B"` (`app/src/main/java/eu/kanade/tachiyomi/util/StorageUtil.kt`); `DiskUtil.buildValidFilename`: trim `.` and space, empty -> `"(invalid)"`, replace chars 0x00-0x1f and `" * / : < > ? \ |` and 0x7f with `_`, `take(240)`; `MAX_FILE_NAME_BYTES = 250`; `NOMEDIA_FILE = ".nomedia"`; `getTotalStorageSpace/getAvailableStorageSpace` return `-1L` on error; `UniFileTempFileManager` copies content-URI files to `externalCacheDir/tmp` with prefix padded to 3 chars (needed because `createTempFile` rejects shorter prefixes) and `deleteTempFiles()` = `dir.deleteRecursively()`.
- Storage layout (`StorageManager.kt:22-103`): base dir pref `baseStorageDirectory`; subdirectories `autobackup`, `downloads` (with `.nomedia`), `local`, `localanime`, `mpv-config` (`fonts`, `scripts`, `script-opts`, `shaders`). `getBaseDir` returns `null` unless the directory `exists()`. Changes to the pref (dropping the first emission, `distinctUntilChanged`) re-create every subdirectory and emit on `changes` so caches refresh.
- Replacement: Proposal: `PathBuf` everywhere; no SAF layer. `StorageManager` becomes a struct holding `base: PathBuf` with the same subdirectory names, `ensure_dirs()`, and a `tokio::sync::watch` for change events. Skip `.nomedia` (Linux has no media scanner; `tracker`/KDE Baloo can be excluded via `.nomedia`-like `.noindex`: write `.noindex` optionally). `statvfs` through `rustix::fs::statvfs` for free space. `fs::read_dir` order is unspecified, so sort explicitly wherever Aniyomi relied on `listFiles()` order (chapter names use natural sort, see area:local-source). Temp files: `tempfile::Builder` under `$XDG_CACHE_HOME/kurobayomi/tmp`. File pickers: `gtk::FileDialog` (portal-backed under Flatpak) returns `gio::File::path()`.
- Behavior to preserve: directory creation is idempotent; every operation returns `Option`/error instead of panicking when the base dir is missing; renames go through temp name then rename so partial downloads are never visible; `.tmp` suffix files (`"$filename.tmp"` in the downloader) are cleaned.
- Difficulty: moderate (many call sites, mostly mechanical). Risk: Android case-insensitive/FAT name restrictions are kept by `buildValidFilename` intentionally so downloads stay portable to other devices, keep the 240-char cap even on ext4.

### Uri
- Does: `android.net.Uri` (49 files). Observed operations (counts over those files): `.toUri()` 48, `appendQueryParameter` 42, `buildUpon` 22, `appendPath` 10, `getQueryParameter` 4, `lastPathSegment` 2, `FileProvider.getUriForFile` 2, `Uri.EMPTY` 1. Representative: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:3` (`@Transient var uri: Uri? = null`, deprecated but kept for extension binary compatibility) and `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt`.
- Groups: (a) tracker API URL builders: `AnilistApi.kt:584` `authUrl() = "${BASE_URL}oauth/authorize"` with `client_id` and `response_type=token`; `MyAnimeListApi.kt:83-403`, `ShikimoriApi.kt:126-261`, `BangumiApi.kt:279`, `SimklApi.kt:144-249` build query strings with `appendQueryParameter`. (b) OAuth callback parsing: `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt` switches on `data.host` in {`anilist-auth`, `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth`}; AniList reads the fragment with regex `(?:access_token=)(.*?)(?:&)`, others read the `code` query parameter; no code means `logout()`. (c) storage `content://` URIs (see UniFile). (d) share/open intents (`util/system/IntentExtensions.kt:13` `Uri.toShareIntent(context, "image/*"|"text/plain", message)`, `ExternalIntents.kt` builds intents for external players with `subs` and `subs.enable` extras). (e) backup `Uri` args (`BackupCreator.kt`, `BackupRestorer.kt`, `BackupDecoder.kt`, `BackupFileValidator.kt`).
- Replacement: Proposal: `url::Url` with `query_pairs_mut().append_pair()`, `path_segments_mut().push()`, `query_pairs()`, `path_segments().last()`; local paths stay `PathBuf` (never `file://` strings in the DB). `Page.uri` is dropped from the core model, but keep the field when loading extensions that still set it (see area:extensions). OAuth: register an XDG URL scheme handler via `.desktop` `MimeType=x-scheme-handler/kurobayomi;` and a single-instance `gio::Application` `open` handler, or run a loopback `127.0.0.1:<port>` listener for redirect (preferred for MAL/Shikimori/Bangumi/Simkl: add `redirect_uri=http://127.0.0.1:<port>/`); AniList implicit grant returns the token in the fragment which a loopback server cannot see, so serve a tiny HTML page that forwards `location.hash` as a query to the listener. The five hosts `anilist-auth`, `bangumi-auth`, `myanimelist-auth`, `shikimori-auth`, `simkl-auth` become the five loopback paths of the same names.
- Preserve: percent-encoding must match Android's `Uri.Builder` (RFC 3986, space as `%20` not `+`); `Url::query_pairs_mut` uses `+` for spaces (form encoding), so use `url::form_urlencoded` carefully or build with `percent-encoding` and `NON_ALPHANUMERIC` minus `-_.~` for server compatibility.
- Difficulty: moderate. Risk: OAuth redirect URIs registered at the tracker developer consoles are fixed; Aniyomi's client ids (see area:trackers) may only accept the Android scheme `aniyomi://`, so Kurobayomi needs its own client ids or a documented fallback of manual token paste.

### ValueAnimator
- Does: `android.animation.ValueAnimator` (2 files). `ui/reader/viewer/webtoon/WebtoonRecyclerView.kt:113-155` animates `x`, `y` and scale together for double-tap zoom (`ANIMATOR_DURATION_TIME = 200` ms, `DecelerateInterpolator`) and `zoomFling` (`distanceTimeFactor = 0.4f`, `dx = factor * velocityX / 2`, duration 400 ms, decelerate; vertical fling only when `atFirstPosition || atLastPosition`). `ui/main/MainActivity.kt:434,443` (SDK < 31 splash exit): translate root by `value * 16.dpToPx` and fade splash with `SPLASH_EXIT_ANIM_DURATION = 400L`, `LinearOutSlowInInterpolator`/`FastOutSlowInInterpolator`.
- Replacement: Proposal: `fn animate(widget, duration_ms, easing, on_frame, on_end)` over `Widget::add_tick_callback` using `FrameClock::frame_time()` (microseconds). Easings: decelerate `1 - (1-t)^2` (Android `DecelerateInterpolator` default factor 1), fast-out-slow-in = cubic-bezier(0.4, 0, 0.2, 1), linear-out-slow-in = cubic-bezier(0, 0, 0.2, 1). Drop the splash animation (no SDK<31 splash on Linux).
- Difficulty: moderate. Risk: respect `gtk-enable-animations`; when false jump to the end value.

### VerticalGrid
- Does: `io.woong.compose.grid.VerticalGrid` (1 file) lays out the upcoming-releases calendar: `app/src/main/java/mihon/feature/upcoming/components/calendar/Calendar.kt:83` `VerticalGrid(columns = SimpleGridCells.Fixed(DAYS_OF_WEEK = 7))`. Header row: 7 `DayOfWeek.getDisplayName(TextStyle.NARROW, Locale.getDefault())` starting at the locale's first day of week (`WeekFields.of(Locale.getDefault()).firstDayOfWeek`); `emptyFieldCount = weekDays.indexOf(firstOfMonth.dayOfWeek)` blank cells; then one `CalendarDay` per day; max width 360 dp on medium (non-expanded) windows.
- Replacement: Proposal: `gtk::Grid` with `column_homogeneous(true)`, 7 columns, `chrono::Weekday`; first day of week from `gtk::Calendar` or `LC_TIME` (`nl_langinfo(_NL_TIME_FIRST_WEEKDAY)` via `libc`). Or embed `gtk::Calendar` with marked days (`mark_day`) for less code (ponytail: gtk::Calendar covers it; custom grid only if per-day release counts must be drawn).
- Difficulty: trivial.

### View
- Does: `android.view.View` (23 files import it exactly; 34 files import some `android.view.View*` name). Used as the base of custom drawing widgets and view-based reader/download UI: `RevealAnimationView.kt`, `ReaderNavigationOverlayView.kt`, `ReaderPageImageView.kt`, `Viewer.kt` (interface returning `View`), webtoon/pager holders, `MangaDownloadItem/Holder/HeaderItem/HeaderHolder.kt` and anime twins (FlexibleAdapter-based download queue), `PlayerActivity.kt` (`View.SYSTEM_UI_FLAG_*` at `:388-392`), `ReaderActivity.kt:971` `setLayerType(LAYER_TYPE_HARDWARE, paint)` for grayscale/inverted colors, `presentation-core/.../Scrollbar.kt:34`, `ViewExtensions.kt` (`ComposeView.setComposeContent` wrapping `TachiyomiTheme` with `bodySmall` text style and `onBackground` content colour).
- Replacement: Proposal: `gtk::Widget` subclasses via `glib::wrapper!`/`subclass`, custom drawing in `snapshot()` using `gtk::Snapshot` (rects, `append_layout` text). Grayscale and inverted colour (`ReaderActivity.setLayerPaint`) become a `gsk::ColorMatrix` node: `snapshot.push_color_matrix(&matrix, &offset)` with the standard luminance matrix (0.2126, 0.7152, 0.0722) for grayscale and `-1` diagonal with offset `1` for invert.
- Difficulty: moderate. Risk: the reader viewers are the large rewrite (area:reader); this entry only covers the base class.

### ViewAnimationUtils
- Does: `android.view.ViewAnimationUtils` (1 file) in `app/src/main/java/eu/kanade/tachiyomi/widget/RevealAnimationView.kt:7`: `createCircularReveal(view, cx, cy, startR, endR)`: hide = radius `initialRadius` to `0f`, 500 ms, then `isInvisible = true`; show = radius `0f` to `height.toFloat()`, 350 ms, with a caller listener. It is a legacy helper (no call sites found beyond its own definition).
- Replacement: drop (dead code). If a reveal is later wanted: tick callback animating a circular `gsk::RoundedRect` clip via `snapshot.push_rounded_clip`.
- Difficulty: trivial.

### ViewCompat
- Does: `androidx.core.view.ViewCompat` (2 files): `MangaDownloadQueueScreen.kt:66` and `AnimeDownloadQueueScreen.kt:66` call `ViewCompat.setNestedScrollingEnabled(screenModel.controllerBinding.root, true)` so the RecyclerView inside Compose cooperates with the collapsing top bar.
- Replacement: drop; the queue is a native `gtk::ListView` in a `gtk::ScrolledWindow` (area:downloads).
- Difficulty: trivial.

### ViewConfiguration
- Does: `android.view.ViewConfiguration` (4 files). `GestureDetectorWithLongTap.kt:21-23`: `scaledTouchSlop`, `getLongPressTimeout()` (Android default 500 ms) and `getDoubleTapTimeout()` (300 ms). `WebtoonRecyclerView.kt:263` touch slop (8 dp). `Scrollbar.kt:232-243` and `VerticalFastScroller.kt:444-448`: `scaledScrollBarSize`, `getScrollBarFadeDuration()` (250 ms), `getScrollDefaultDelay()` (300 ms).
- Replacement: Proposal: a `Gestures` const struct: double-click window from `gtk::Settings::gtk_double_click_time()` (default 400 ms), drag threshold from `gtk_dnd_drag_threshold` (8 px), long press 500 ms via `gtk::GestureLongPress` (its own delay factor `gtk-long-press-time`, GTK 4.? default 500 ms), scrollbars: `gtk::ScrolledWindow` with `overlay_scrolling(true)`; fade timing is theme controlled.
- Difficulty: trivial.

### ViewGroup
- Does: `android.view.ViewGroup` (12 files) as parent type and for `LayoutParams` constants `MATCH_PARENT`/`WRAP_CONTENT`: `ViewPagerAdapter.kt:4`, `MinMaxNumberPicker.kt:6`, `WebtoonViewer.kt:7-8`, `WebtoonPageHolder.kt`, `WebtoonTransitionHolder.kt`, `WebtoonBaseHolder.kt`, `WebtoonAdapter.kt`, `PagerViewerAdapter.kt`, `PagerTransitionHolder.kt`, `PagerViewer.kt`, `ReaderProgressIndicator.kt`, `ReaderPageImageView.kt`.
- Replacement: Proposal: `gtk::Box` (linear), `gtk::Overlay` (stacked), `gtk::Fixed` (absolute) and `hexpand`/`vexpand` properties; `MATCH_PARENT` = `set_hexpand(true)` + `set_halign(Fill)`.
- Difficulty: trivial.

### ViewModel
- Does: `androidx.lifecycle.ViewModel` (2 files): `ReaderViewModel` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt:8,106`) and `PlayerViewModel` (`.../ui/player/PlayerViewModel.kt:39,178`). Both hold a `SavedStateHandle` to restore after process death: reader keys `"chapter_id"` (default -1L) and `"page_index"` (default -1); `viewModelScope` for coroutines; `onCleared()`: reader unrefs `viewerChapters` and re-queues `chapterToDownload` at start of the download queue (`downloadManager.addDownloadsToStartOfQueue`); player saves watching progress for `currentEpisode` and re-queues `episodeToDownload`. (All other screens use Voyager `ScreenModel`, outside this chunk.)
- Replacement: Proposal: `ReaderSession`/`PlayerSession` Rust structs owned by the window; state in `tokio::sync::watch` channels (`State` snapshots) and an `mpsc` event channel (equivalent of `eventChannel`); a `Drop`/explicit `close()` implementing the `onCleared` logic. Process-death restore: persist `{chapter_id, page_index}` in the `settings` table on every page change (debounced), so a crash resumes (area:reader).
- Difficulty: moderate. Risk: do `close()` on window close-request, not just Drop, since it awaits DB writes.

### ViewModelProvider
- Does: `androidx.lifecycle.ViewModelProvider` (1 file, `PlayerViewModel.kt:40`) defines `PlayerViewModelProviderFactory` (line 145) so `PlayerViewModel(activity, extras.createSavedStateHandle())` receives the `PlayerActivity`.
- Replacement: drop; construct the session struct directly with explicit dependencies.
- Difficulty: trivial.

### ViewPager
- Does: `androidx.viewpager.widget.ViewPager` (1 file, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt:12`). Aniyomi's `Pager` extends a vendored `DirectionalViewPager` (horizontal or vertical) and adds tap/long-tap via `GestureDetectorWithLongTap`, haptic feedback on long-press, try/catch around touch handling (`IllegalArgumentException`, `NullPointerException`, `IndexOutOfBoundsException`), and disables default key handling. Settings: `offscreenPageLimit = 1`, `setCurrentItem(position, smoothScroll)` (smooth only when `config.usePageTransitions`), listener ignoring non-idle states, `SCROLL_STATE_IDLE` bookkeeping, adapter reset preserving `currentItem` on setting changes.
- Replacement: Proposal: custom `PagedStrip` widget: a `gtk::Overlay` holding the current page, with prev/next preloaded in a `gtk::Stack` (`Stack::set_transition_type(SlideLeftRight|SlideUpDown|None)`, `transition_duration(250)`), RTL by reversing the page order, `gtk::GestureClick` for taps (relative x/y within widget, as `PagerViewer.kt:104-111` computes), `gtk::GestureSwipe`/`GestureDrag` for finger swipe, `gtk::EventControllerKey` for keys, wheel via `EventControllerScroll`. Preload window = current +/- 1.
- Difficulty: hard (core of the manga reader). See area:reader.

### ViewPropertyAnimator
- Does: `android.view.ViewPropertyAnimator` (1 file, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderNavigationOverlayView.kt:10,24`): fades the tap-zone overlay in (`alpha(1f)`, `FADE_DURATION = 1000L`, start action sets visible) when navigation changes (unless the overlay is already visible, or `!showOnStart && firstLaunch`, or navigation is `DisabledNavigation`), and out on click (`alpha(0f)`, end action sets invisible). It also draws each navigation region as filled rects (`region.type.color`) with 64f white text with 8f black stroke, text from `region.type.nameRes`.
- Replacement: Proposal: `gtk::Widget::set_opacity` driven by the same tick-callback `animate()` helper (see ValueAnimator), 1000 ms linear; draw regions in `snapshot()` with `append_color` and Pango layout with stroked outline (draw layout twice: black offset-stroke via `gsk::Stroke`, white fill). Hide on first tap/swipe.
- Difficulty: trivial.

### ViewSizeResolver
- Does: `coil3.size.ViewSizeResolver` (1 file, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:29`): at line 338 the page `ImageRequest` uses `.size(ViewSizeResolver(this@ReaderPageImageView))`, `Precision.INEXACT`, `cropBorders(config.cropBorders)`, `crossfade(false)`, `customDecoder(true)`, so the image is decoded at the view's size.
- Replacement: Proposal: decode with the `image` crate (or `libvips`/`gdk-pixbuf::Pixbuf::from_stream_at_scale`) at `min(natural, widget.width() * scale_factor)`; but for tall webtoon strips decode at width = widget width and full height. Never downscale below widget size when zoom is allowed; keep the full-size file on disk for the zoom path (the app uses `SubsamplingScaleImageView` for tiles, see area:reader).
- Difficulty: moderate.

### WRAP_CONTENT
- Does: `android.view.ViewGroup.LayoutParams.WRAP_CONTENT` (4 files): `WebtoonTransitionHolder.kt:5,46,55`, `WebtoonPageHolder.kt:7,99` (frame is `MATCH_PARENT x WRAP_CONTENT`: width fills, height follows the image), `PagerTransitionHolder.kt:9,50,144`, `ReaderProgressIndicator.kt:6`. Also used qualified as `LayoutParams.WRAP_CONTENT` in `WebtoonBaseHolder.kt:27` and `ReaderTransitionView.kt:27`.
- Replacement: default GTK sizing: a `gtk::Picture`/custom widget reports its height-for-width in `measure()`; for webtoon pages set `hexpand=true`, `valign=Start`, `content_fit=ScaleDown` is wrong, use width-fill with computed height `w * img_h / img_w`.
- Difficulty: trivial.

### WallpaperManager
- Does: `android.app.WallpaperManager` (1 file, `app/src/main/java/eu/kanade/presentation/theme/colorscheme/MonetColorScheme.kt:5,26`): on Android 8.1-11 reads `getWallpaperColors(FLAG_SYSTEM).primaryColor` as a seed for `MonetCompatColorScheme` (`SchemeContent`, `Hct.fromInt(seed)`, `MaterialDynamicColors`); on 12+ uses `dynamicLightColorScheme`; otherwise `TachiyomiColorScheme`.
- Replacement: drop. Linux is themed by the user's GTK theme (area:theme); a "follow system accent" option may read `org.freedesktop.appearance accent-color` through `ashpd` (xdg-desktop-portal 1.18+) and apply it as a `@define-color accent_color` CSS override. User loses: wallpaper-derived palette.
- Difficulty: trivial (drop).

### WebResourceError
- Does: `android.webkit.WebResourceError` (1 file, `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/CloudflareInterceptor.kt:5,87`): in `onReceivedError(view, request, error)`: for main-frame requests, `error.errorCode in ERROR_CODES (403, 503)` sets `challengeFound = true`; other codes unlock the waiting thread (challenge not present).
- Replacement: Proposal: `webkit6::WebView` `load-failed` signal (error domain/code) and `resource-load-started` with the `ResourceLoad` response status (`WebResource::response().status_code()`); detect the Cloudflare challenge by main-resource status 403/503 plus `Server` header `cloudflare` or `cloudflare-nginx` (`SERVER_CHECK`).
- Difficulty: moderate. See area:12-network-stack.

### WebResourceRequest
- Does: `android.webkit.WebResourceRequest` (2 files): `CloudflareInterceptor.kt:6` (`request.isForMainFrame`) and `app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt` (`shouldOverrideUrlLoading`: return false for `blob:http` URLs, true (ignore) for `intent://` URLs, otherwise reload with `view.loadUrl(url, headers)` so the source headers apply).
- Replacement: Proposal: `webkit6::WebView` `decide-policy` signal with `NavigationPolicyDecision`; `decision.navigation_action().request().uri()`: `blob:http` -> `use_()`, `intent://` -> `ignore()`, else `ignore()` and `load_request(&URIRequest)` with headers from `request.http_headers().append(k, v)` for each source header.
- Difficulty: moderate.

### WebSettings
- Does: `android.webkit.WebSettings` (2 files). `core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt:84` `setDefaultSettings()`: `javaScriptEnabled = true`, `domStorageEnabled = true`, `useWideViewPort = true`, `loadWithOverviewMode = true`, `cacheMode = LOAD_DEFAULT`, `setSupportZoom(true)`, `builtInZoomControls = true`, `displayZoomControls = false`, plus `CookieManager.acceptThirdPartyCookies`. `WebViewInterceptor.kt:42`: `WebSettings.getDefaultUserAgent(context)` to pre-warm WebView (skipped on MIUI and Samsung SDK 31).
- Replacement: Proposal: `webkit6::Settings`: `set_enable_javascript(true)`, `set_enable_html5_local_storage(true)`, `set_user_agent(...)`; zoom through `WebView::set_zoom_level`; third-party cookies via `NetworkSession::website_data_manager().set_..`/`CookieManager::set_accept_policy(Always)`. User agent: reuse the HTTP client UA (`request.header("User-Agent") ?: defaultUserAgent`, an empty UA must not be set) so Cloudflare cookies match the reqwest client.
- Difficulty: trivial.

### WebStorage
- Does: `android.webkit.WebStorage` (1 file, `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:7,250`): in "Clear WebView data" (`MR.strings.pref_clear_webview_data`): creates a WebView, `clearCache(true)`, `clearFormData()`, `clearHistory()`, `clearSslPreferences()`, then `WebStorage.getInstance().deleteAllData()` and deletes `dataDir/app_webview/` recursively; success toast `MR.strings.webview_data_deleted`, failure `MR.strings.cache_delete_error`.
- Replacement: Proposal: `webkit6::WebsiteDataManager::clear(WebsiteDataTypes::ALL, TimeSpan(0), ...)` on the app's `NetworkSession`, then remove the persistent storage dirs (`$XDG_DATA_HOME/kurobayomi/webkit`, `$XDG_CACHE_HOME/kurobayomi/webkit`). Also clear the shared reqwest cookie jar only via the separate "clear cookies" action, as Aniyomi does (`networkHelper.cookieJar.removeAll()`, toast `MR.strings.cookies_cleared`).
- Difficulty: trivial.

### WebView
- Does: two FQNs. `android.webkit.WebView` (6 files): `core/common/.../util/system/WebViewUtil.kt`, `network/interceptor/WebViewInterceptor.kt`, `CloudflareInterceptor.kt`, `app/src/main/java/eu/kanade/tachiyomi/App.kt:12`, `WebViewScreenContent.kt`, `SettingsAdvancedScreen.kt`. `com.kevinnzou.web.WebView` (1 file, `WebViewScreenContent.kt:33`, the Accompanist-style Compose wrapper) is the in-app browser.
- Observed behaviors to preserve:
  - Cloudflare solver (`CloudflareInterceptor.kt:30-135`): trigger when response code in `{403, 503}` and `Server` header in `{"cloudflare-nginx", "cloudflare"}`; close the response, remove cookie `cf_clearance` for the URL, remember the old cookie, load the original URL (headers filtered by `isRequestHeaderSafe`: drop `content-length, host, trailer, te, upgrade, cookie2, keep-alive, transfer-encoding, set-cookie`, any `proxy-*`, and `connection: upgrade`), wait up to 30 s on a `CountDownLatch` (`awaitFor30Seconds`), success when a `cf_clearance` cookie exists that differs from the old one, or abort early if the first page loaded at the original URL without a challenge; on failure toast `MR.strings.information_webview_outdated` (long) when major Chrome version `< 118` (`MINIMUM_WEBVIEW_VERSION`) and throw `IOException(MR.strings.information_cloudflare_bypass_failure)`. Then retry the original request with `chain.proceed`.
  - Missing WebView: `supportsWebView` false means toast `MR.strings.information_webview_required` (long) and return the original response.
  - In-app browser (`WebViewScreenContent.kt`): top bar title = page title or initial title, subtitle = current URL, close icon, back/forward (enabled by `canGoBack/canGoForward`), overflow: refresh, share, open in browser, clear cookies (`MR.strings.pref_clear_cookies`, removes cookies for current URL and logs `Cleared $cleared cookies for: $url`), linear progress bar, and Cloudflare help banner (`MR.strings.information_cloudflare_help`, link `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare`) shown when page HTML contains `window._cf_chl_opt` or `Ray ID is`.
  - `App.kt:96` sets `WebView.setDataDirectorySuffix(process)` for non-main processes, and `App.kt:224-231` overrides `getPackageName()` for Chromium `BuildInfo` calls to spoof `X-Requested-With` as `com.android.chrome` if installed, else `com.android.settings` (`WebViewUtil.spoofedPackageName`).
  - `WebViewUtil.getInferredUserAgent`: replace `; Android .*?\)` with `; Android 10; K)` and `Version/.* Chrome/` with `Chrome/`.
- Replacement: Proposal: `webkit6::WebView` (WebKitGTK 6.0, `webkitgtk-6.0` package on Arch) in a `gtk::Window`; headless solver: create the WebView off-screen in a hidden `gtk::Window` (needs a realized widget for Wayland/X11) with a shared `NetworkSession` persisted at `$XDG_DATA_HOME/kurobayomi/webkit`; read cookies with `CookieManager::cookies(uri)`, export `cf_clearance` into the reqwest cookie jar (area:12-network-stack). User agent must match: set WebKit UA to the reqwest default UA string or the TLS/UA mismatch invalidates `cf_clearance`. X-Requested-With spoof: not applicable (WebKit sends none). Replace "WebView outdated" check with WebKitGTK version `< ` a documented floor.
- Risks: Cloudflare Turnstile may reject WebKitGTK; allow the user to show the solver window ("interactive mode", the Aniyomi browser screen) and then retry. Difficulty: hard.

### WebViewClient
- Does: `android.webkit.WebViewClient` (1 file, `CloudflareInterceptor.kt:8,76`): `onPageFinished` (success check above; abort when `url == origRequestUrl && !challengeFound`) and `onReceivedError`. The compose wrapper's `AccompanistWebViewClient` overrides `onPageStarted`, `onPageFinished`, `doUpdateVisitedHistory`, `shouldOverrideUrlLoading` (`WebViewScreenContent.kt`).
- Replacement: Proposal: connect `load-changed` (events `Started`, `Committed`, `Finished`), `notify::uri`, `notify::title`, `notify::estimated-load-progress` (progress bar), `decide-policy`, `load-failed` on `webkit6::WebView`; read HTML for Cloudflare banner with `WebView::evaluate_javascript("document.documentElement.outerHTML")` (async) and test for `window._cf_chl_opt` or `Ray ID is`.
- Difficulty: moderate.

### WebtoonLayoutManager
- Does: `androidx.recyclerview.widget.WebtoonLayoutManager` (1 file importing, `ui/reader/viewer/webtoon/WebtoonViewer.kt:13`; defined in `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonLayoutManager.kt`, in the support package to reach package-private fields). A `LinearLayoutManager` with `isItemPrefetchEnabled = false`, `getExtraLayoutSpace(state) = extraLayoutSpace` and `findLastEndVisibleItemPosition()` scanning children from the last backwards for `childEnd <= parentEnd || childStart < parentStart` using the bound-check callbacks. `WebtoonViewer.kt:53,58`: `scrollDistance = displayMetrics.heightPixels * 3 / 4`, `layoutManager = WebtoonLayoutManager(activity, scrollDistance)`; page up/down scroll by `scrollDistance`, smooth when page transitions are enabled.
- Replacement: Proposal: `gtk::ScrolledWindow` containing a custom vertical `gtk::Widget` that lays out chapter pages at width = viewport width and height from the image aspect ratio, with a virtualisation window of one viewport (`extra = 3/4 * viewport_height`) above and below so images load before becoming visible. "Last end visible index" = last page whose bottom edge `<= viewport bottom` or whose top `< viewport top`; used for progress.
- Difficulty: hard. See area:reader.

### WifiManager
- Does: `android.net.wifi.WifiManager` (1 file, `app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkExtensions.kt:6`). `isConnectedToWifi()`: false if `!wifiManager.isWifiEnabled`; API >= 29: active network has `TRANSPORT_WIFI` and `NET_CAPABILITY_INTERNET`, else `connectionInfo.bssid != null`. Callers: `MangaLibraryUpdateJob.kt:99` and `AnimeLibraryUpdateJob.kt:98` (only SDK < 28 for the "only on Wi-Fi" restriction `DEVICE_ONLY_ON_WIFI`; returns `Result.retry()`), `MangaDownloadJob.kt` via `activeNetworkState()` for `downloadOnlyOverWifi` (stops downloader with `R.string.download_notifier_text_only_wifi` or `R.string.download_notifier_no_network`). `isOnline()` (same file) is used by `ExceptionFormatter.kt:22`, `ExternalIntents.kt:545`, `TrackEpisode.kt:38`.
- Replacement: Proposal: `gio::NetworkMonitor::default()`: `is_network_available()`, `connectivity()`, `is_network_metered()` and the `network-changed` signal. "Wi-Fi only" = not metered and, via NetworkManager D-Bus (`zbus`, `org.freedesktop.NetworkManager` `PrimaryConnectionType == "802-11-wireless"` or `Ethernet`), treat Ethernet as acceptable. Map `DEVICE_NETWORK_NOT_METERED` to `!is_network_metered()`.
- Difficulty: moderate.

### Window
- Does: `android.view.Window` (3 files). `app/src/main/java/eu/kanade/tachiyomi/util/view/WindowExtensions.kt:6` `Window.setSecureScreen(enabled)` sets/clears `FLAG_SECURE`; used in `SecureActivityDelegate.kt:103` driven by `combine(secureScreen, incognitoMode)`: secure when `secureScreen == ALWAYS` or `INCOGNITO && incognitoMode`. Preference `secure_screen_v2`, enum `SecureScreenMode { ALWAYS, INCOGNITO, NEVER }`, default `INCOGNITO` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:17,30`). `PlayerActivity.kt` uses `window` flags.
- Replacement: Proposal: `gtk::Window`/`gdk::Surface`. `FLAG_SECURE` has no Wayland/X11 equivalent; drop and show the setting only if a compositor protocol appears (user loses: blank screenshots/recents in incognito). Keep the preference in the schema for backup compatibility.
- Difficulty: trivial.

### WindowCompat
- Does: `androidx.core.view.WindowCompat` (3 files): `ReaderActivity.kt:875` `setDecorFitsSystemWindows(window, !fullscreen)` when `readerPreferences.fullscreen()` changes (plus `updateViewerInset`), `PlayerActivity.kt:121,382` (`getInsetsController`, `setDecorFitsSystemWindows(window, false)`), `CrashActivity.kt:5,16`.
- Replacement: Proposal: `gtk::Window::fullscreen()`/`unfullscreen()` toggled from the reader/player fullscreen preference; no inset math on desktop; the crash screen is a plain `gtk::Window` (or terminal panic hook writing `crash.log` under `$XDG_STATE_HOME/kurobayomi/`).
- Difficulty: trivial.

### WindowInsetsCompat
- Does: `androidx.core.view.WindowInsetsCompat` (3 files): `ReaderActivity.kt:519-523` `Type.systemBars()` for show/hide, `PlayerViewModel.kt:674,680` `Type.statusBars()` show/hide when controls toggle, `PlayerActivity.kt:393-394` hides `systemBars()` and `navigationBars()` at start.
- Replacement: drop (no system bars on desktop); where "immersive" mattered, fullscreen covers it.
- Difficulty: trivial.

### WindowInsetsControllerCompat
- Does: `androidx.core.view.WindowInsetsControllerCompat` (2 files): `ReaderActivity.kt:135,525` hides bars when menus hide and `fullscreen` is on, `systemBarsBehavior = BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE`; `PlayerActivity.kt:121,395` same behaviour.
- Replacement: Proposal: reader: when menus hide and `fullscreen` preference is true, hide the header bar (`gtk::HeaderBar` or custom toolbar via `set_visible(false)`) and optionally `fullscreen()`; menus visible shows them again. Player: auto-hide controls overlay after a timeout (see area:player) and hide the cursor with `set_cursor_from_name("none")`.
- Difficulty: trivial.

### WindowManager
- Does: `android.view.WindowManager` (3 files). `util/view/WindowExtensions.kt:8` `LayoutParams.FLAG_SECURE`. `PlayerActivity.kt:152,384-387,396,399-401,698-701,901`: `FLAG_KEEP_SCREEN_ON` added in `onStart` and while playing (`pause` property false), cleared when paused or when `PlayerActivity` stops; `FLAG_LAYOUT_NO_LIMITS`, `FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS`; `layoutInDisplayCutoutMode` = `SHORT_EDGES` if `playerFullscreen()` else `NEVER`. `ReaderActivity.kt:520` adds `FLAG_DRAWS_SYSTEM_BAR_BACKGROUNDS`.
- Replacement: Proposal: keep-screen-on = idle inhibit: `gtk::Application::inhibit(window, ApplicationInhibitFlags::IDLE, "Playing video")` stored cookie; `uninhibit(cookie)` on pause/stop. Works through xdg-desktop-portal `Inhibit` on Wayland. Cutout modes and window layout flags dropped. Secure-screen: dropped (see Window).
- Difficulty: trivial.

### WorkInfo
- Does: `androidx.work.WorkInfo` (8 files): `WorkManagerExtensions.kt:5,16` `isRunning(tag) = getWorkInfosByTag(tag).get().any { it.state == WorkInfo.State.RUNNING }`; `MangaDownloadJob.isRunning` counts `RUNNING == 1` for unique work `"MangaDownloader"`; `MangaLibraryUpdateJob.kt` and `MangaMetadataUpdateJob.kt` (+ anime twins) expose running state flows; `WorkerInfoScreen.kt:21,125-140` debug screen lists SUCCEEDED/FAILED/CANCELLED (finished), RUNNING and ENQUEUED works.
- Replacement: Proposal: `enum JobState { Enqueued, Running, Succeeded, Failed, Cancelled, Blocked }` and `JobRegistry: Arc<Mutex<HashMap<JobId, JobInfo{ name, tags, state, started_at, attempt }>>>` with a `watch` per job for UI. The debug screen shows the three lists.
- Difficulty: trivial.

### WorkManager
- Does: `androidx.work.WorkManager` (3 files import it directly: `WorkManagerExtensions.kt:6`, `MangaDownloadJob.kt`, `AnimeDownloadJob.kt`; the rest use the `Context.workManager` extension). It persists and schedules all background work. Full inventory with parameters:

| Job | File | Kind | Unique name / tag | Policy | Constraints / timing |
|---|---|---|---|---|---|
| Library update (auto) | `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:439-506` | periodic | `LibraryUpdate-auto` | `ExistingPeriodicWorkPolicy.UPDATE` | interval `autoUpdateInterval` hours (flex 10 min), backoff `LINEAR` 10 min, network `UNMETERED` if `DEVICE_NETWORK_NOT_METERED` else `CONNECTED`, Wi-Fi transport if `DEVICE_ONLY_ON_WIFI`, charging if `DEVICE_CHARGING`, battery-not-low always |
| Library update (manual) | same | one-time | `LibraryUpdate-manual` | `ExistingWorkPolicy.KEEP` | none; auto job returns `Result.retry()` while manual is running |
| Metadata update | `.../data/library/manga/MangaMetadataUpdateJob.kt:175-190` | one-time | `MetadataUpdate` | `KEEP` | foreground |
| Download queue | `.../data/download/manga/MangaDownloadJob.kt:98-122` | one-time | `MangaDownloader` | `REPLACE` | stops when offline or Wi-Fi-only unmet |
| Backup create (auto) | `.../data/backup/create/BackupCreateJob.kt:90-126` | periodic | `TAG_AUTO` | `UPDATE` | `backupInterval` hours, flex 10 min, backoff `EXPONENTIAL` 10 min, battery-not-low |
| Backup create (manual) | same | one-time | `TAG_MANUAL` | `KEEP` | none |
| Backup restore | `.../data/backup/restore/BackupRestoreJob.kt:91-104` | one-time | `BackupRestore` | `KEEP` | foreground |
| App update download | `.../data/updater/AppUpdateDownloadJob.kt:128-149` | one-time | `AppUpdateDownload` | `REPLACE` | network `CONNECTED`, input `DOWNLOAD_URL`, `DOWNLOAD_TITLE` |
| Delayed tracking update | `app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt:20-69` | one-time | `DelayedMangaTrackingUpdate` | `REPLACE` | network `CONNECTED`, backoff `EXPONENTIAL` 5 min; `runAttemptCount > 3` fails; retries while store non-empty |
| Anime twins | `.../data/library/anime/AnimeLibraryUpdateJob.kt`, `AnimeMetadataUpdateJob.kt`, `data/download/anime/AnimeDownloadJob.kt`, `domain/track/anime/service/DelayedAnimeTrackingUpdateJob.kt` | same | same names with Anime prefix | same | same |

- Replacement: Proposal: `Scheduler` service in the app process on the tokio runtime: `schedule_periodic(name, interval, flex, constraints, job)` persisting the next-run time in the `settings`/`jobs` table; on app start compute `next = last_run + interval`; if overdue by more than flex, run after a 10-minute grace. Constraints checked just before running: `gio::NetworkMonitor` (connected/not metered), on-AC via UPower D-Bus (`org.freedesktop.UPower` `OnBattery`) for "charging", battery not low via UPower `Percentage >= 15` (WorkManager's threshold is 15%). Backoff: LINEAR = `attempt * 10 min`, EXPONENTIAL = `10 min * 2^(attempt-1)` (cap 5 h as WorkManager's `MAX_BACKOFF_MILLIS = 18_000_000`). Unique-work policies `KEEP`/`REPLACE`/`UPDATE` implemented over a `HashMap<name, JobHandle>`. Because a desktop app is not always running, offer an optional systemd user timer (`kurobayomi-update.timer` running `kurobayomi --update-library`) generated from settings; this is a P2 follow-up to area:library-updates.
- Difficulty: hard (many semantics). Risk: foreground-service behaviour (`setForegroundSafely`, `delay(500)`) is dropped; replace with a desktop notification with progress (`notify-rust` hints `value`).

### WorkQuery
- Does: `androidx.work.WorkQuery` (5 files): `WorkerInfoScreen.kt:22,129-140` `WorkQuery.fromStates(...)`; `MangaLibraryUpdateJob.kt:513` `WorkQuery.Builder.fromTags(listOf(TAG))` to read the running update work and tell auto from manual (`it.tags.contains(WORK_NAME_AUTO)`); same in `MangaMetadataUpdateJob.kt`, `AnimeLibraryUpdateJob.kt`, `AnimeMetadataUpdateJob.kt`.
- Replacement: Proposal: `registry.query(states: &[JobState], tags: &[&str]) -> Vec<JobInfo>`.
- Difficulty: trivial.

### WorkerParameters
- Does: `androidx.work.WorkerParameters` (11 files, e.g. `AppUpdateDownloadJob.kt:12`): constructor argument for each `CoroutineWorker`, giving `inputData` (e.g. `IS_AUTO_BACKUP_KEY`, `EXTRA_DOWNLOAD_URL`, `EXTRA_DOWNLOAD_TITLE`, library-update category/`target` extras), `tags` (to branch auto vs manual), `runAttemptCount` (`DelayedMangaTrackingUpdateJob.kt:26`: fail after more than 3), `isStopped` (`MangaDownloadJob.kt:68`).
- Files: `AppUpdateDownloadJob.kt`, `MangaMetadataUpdateJob.kt`, `MangaLibraryUpdateJob.kt`, `MangaDownloadJob.kt`, `AnimeMetadataUpdateJob.kt`, `AnimeLibraryUpdateJob.kt`, `DelayedMangaTrackingUpdateJob.kt`, `AnimeDownloadJob.kt`, `DelayedAnimeTrackingUpdateJob.kt`, `BackupRestoreJob.kt`, `BackupCreateJob.kt`.
- Replacement: Proposal: `struct JobContext { input: serde_json::Value, tags: Vec<String>, attempt: u32, cancel: tokio_util::sync::CancellationToken }` passed to `async fn run(ctx) -> JobResult { Success, Failure, Retry }`.
- Difficulty: trivial.

### XML
- Does: `nl.adaptivity.xmlutil.serialization.XML` (3 files). Configured in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:179-189`: `defaultPolicy { ignoreUnknownChildren() }`, `autoPolymorphic = true`, `xmlDeclMode = Charset`, `indent = 2`, `xmlVersion = XML10`. Used to write `ComicInfo.xml` in downloaded chapters (`MangaDownloader.kt:650-682`: deletes old file, creates new `COMIC_INFO_FILE = "ComicInfo.xml"`, writes `xml.encodeToString(ComicInfo.serializer(), comicInfo)` as bytes) and in `source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:60,187,237` to read/write series metadata.
- Local-source metadata algorithm (`LocalMangaSource.kt:150-245`): list the manga folder; find `ComicInfo.xml`, a `.noxml` marker, and legacy `details.json`; if ComicInfo exists, delete `.noxml` and apply it; else if legacy JSON exists, apply its fields (`title`, `author`, `artist`, `description`, `genre` joined by `", "`, `status`), then replace with a generated `ComicInfo.xml` and delete the JSON; else if no `.noxml`, copy `ComicInfo.xml` from the first chapter archive that contains it into the top folder and apply it, or create `.noxml` to avoid rescanning. Errors are logged and swallowed.
- Replacement: Proposal: `quick-xml` with `serde` (`quick_xml::de::from_reader`, `quick_xml::se::to_string` and a manual writer for the declaration and 2-space indent via `quick_xml::writer::Writer::new_with_indent(w, b' ', 2)`). Unknown elements are ignored by default in serde. Write the declaration `<?xml version="1.0" encoding="UTF-8"?>` (matching `xmlDeclMode = Charset`).
- Difficulty: moderate. Risk: namespace-qualified elements (see XmlSerialName).

### XmlElement
- Does: `nl.adaptivity.xmlutil.serialization.XmlElement` (1 file, `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt:5`): `@XmlElement(false)` on `xmlSchema` and `xmlSchemaInstance` makes them XML attributes rather than elements.
- Replacement: Proposal: `#[serde(rename = "@xmlns:xsd")] xml_schema: String` (quick-xml attribute syntax uses the `@` prefix).
- Difficulty: trivial.

### XmlPullParser
- Does: `org.xmlpull.v1.XmlPullParser` (1 file, `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt:32,99-115`): reads the generated `R.xml.locales_config`, for each `<locale android:name="tag">` adds `Language(langTag, LocaleHelper.getLocalizedDisplayName(tag), LocaleHelper.getDisplayName(tag))` when the localized name is non-empty; sorts by `displayName`; inserts `Language("", MR.strings.label_default, null)` at index 0.
- Replacement: Proposal: a compile-time `const LANGS: &[&str]` generated by `build.rs` from the shipped translation files, display names from `icu_displaynames`/`isolang`, same sorting and "Default" first. No XML parsing needed.
- Difficulty: trivial.

### XmlSerialName
- Does: `nl.adaptivity.xmlutil.serialization.XmlSerialName` (1 file, `ComicInfo.kt:6`) names every element of the `ComicInfo` data class: `ComicInfo`, `Title`, `Series`, `Number`, `Summary`, `Writer`, `Penciller`, `Inker`, `Colorist`, `Letterer`, `CoverArtist`, `Translator`, `Genre`, `Tags`, `Web`, and three Aniyomi extensions in the namespace `http://www.w3.org/2001/XMLSchema`: `PublishingStatusTachiyomi` (prefix `ty`), `Categories` (prefix `ty`, class `CategoriesTachiyomi`), `SourceAniyomi` (prefix `ay`); attributes `xmlns:xsd="http://www.w3.org/2001/XMLSchema"` and `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"` on the root. Spec reference: `https://anansi-project.github.io/docs/comicinfo/schemas/v2.0`.
- Mapping rules to port (`ComicInfo.kt` top): `getComicInfo()` from `SManga`: `series = title`, `summary = description`, `writer = author`, `penciller = artist`, `genre = genre`, `publishingStatus = ComicInfoPublishingStatus.toComicInfoValue(status)`, all other fields null. `copyFromComicInfo()` into `SManga`: `title <- series`, `author <- writer`, `description <- summary`, `genre <-` distinct of `[genre, tags, categories]` trimmed and joined with `", "` (only if non-empty), `artist <-` distinct of the split-by-`", "` values of `[penciller, inker, colorist, letterer, coverArtist]` trimmed, joined by `", "`, `status <- toSMangaValue(publishingStatus)`. Status strings: `Ongoing`, `Completed`, `Licensed`, `Publishing finished`, `Cancelled`, `On hiatus`, `Unknown` (unknown or missing falls back to `Unknown`).
- Replacement: Proposal: `#[serde(rename = "ty:PublishingStatusTachiyomi")]` style renames; quick-xml serde does not manage namespaces, so write the prefix literally and declare `xmlns:ty`/`xmlns:ay` on the root when serializing; on read, match on the local name.
- Difficulty: moderate. Risk: output must stay readable by Komga/Kavita-style scanners; write elements in the declared field order.

### XmlValue
- Does: `nl.adaptivity.xmlutil.serialization.XmlValue` (1 file, `ComicInfo.kt:7`): `@XmlValue(true) val value: String = ""` makes every ComicInfo element a text node (`<Series>Foo</Series>`) instead of a nested `<value>`.
- Replacement: Proposal: `#[serde(rename = "$text")] value: String` on a newtype wrapper per element, or serialize fields as `Option<String>` directly under the renamed element name (simpler: skip the wrapper structs).
- Difficulty: trivial.

### XmlVersion
- Does: `nl.adaptivity.xmlutil.core.XmlVersion` (1 file, `AppModule.kt:42,187`): `xmlVersion = XmlVersion.XML10` so the declaration says `1.0`.
- Replacement: drop; `quick-xml`'s `BytesDecl::new("1.0", Some("UTF-8"), None)` is the only version used.
- Difficulty: trivial.

## Cross-cutting notes

- Everything under Glance, Monet, FLAG_SECURE, window insets and TV detection is dropped; the user loses home-screen widgets, wallpaper-derived colours, hidden-in-recents incognito and Android TV onboarding.
- The riskiest items in this chunk are WebView (Cloudflare), ViewPager and WebtoonLayoutManager (reader), WorkManager (scheduling semantics) and UniFile (storage layout), in that order.
- Items with direct data-format compatibility requirements: ComicInfo.xml (XML, XmlElement, XmlSerialName, XmlValue), download file/folder naming (UniFile), settings keys `download_speed_limit`, `secure_screen_v2`, `lock_app_after`, `use_biometric_lock` (Throttler, Window).
