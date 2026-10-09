# Android / third-party API mapping, chunk 1 of 8

Source of the item list: `python3 plans/_tools/inventory.py items imports 1 8` (54 items). First item: `AccompanistWebViewClient`. Last item: `Box`. All paths are repo-relative to the Aniyomi clone (`/home/blackbox/aniyomi-src`).

Each item is a class name imported by Aniyomi (`import <fully.qualified.Name>`). Counts are given as `files/occurrences`: number of Kotlin/Java files that import it, and whole-word occurrences of the simple name inside those files (the import line itself is included, so a count of 2 means one real use). Representative `path:line` is the most informative use. Replacement choices assume the target stack: Rust stable, gtk4-rs 0.11 (GTK 4.16, no libadwaita), tokio, rusqlite, reqwest, serde, libmpv, Linux with Wayland and X11. Difficulty is one of trivial / moderate / hard.

Naming caveat: several simple names are shared by unrelated classes. `Alignment` and `Box` in this chunk are the **Glance** (home-screen widget) classes `androidx.glance.layout.Alignment` and `androidx.glance.layout.Box`; the Compose classes of the same name (`androidx.compose.ui.Alignment`, `androidx.compose.foundation.layout.Box`) are a different import and belong to the Compose-UI chunks. `Animatable` in this chunk is `android.graphics.drawable.Animatable` (reader GIF/WebP animation), not Compose's `androidx.compose.animation.core.Animatable`. `ArchiveEntry` appears twice in the source: the libarchive class `me.zhanghai.android.libarchive.ArchiveEntry` (this item) and Aniyomi's own wrapper `mihon.core.archive.ArchiveEntry(name, isFile)`; both are covered under the `ArchiveEntry` entry. `Archive` is the libarchive class, not `ArchiveReader`. `Application` is `android.app.Application`.

## 1. Index

| # | Item | Fully qualified | Files/occ. | Replacement (one line) | Difficulty |
|---|---|---|---|---|---|
| 1 | AccompanistWebViewClient | com.kevinnzou.web.AccompanistWebViewClient | 1/2 | `webkit6` (WebKitGTK 6) `WebView` + `NetworkSession` | hard |
| 2 | Activity | android.app.Activity | 13/29 | `gtk::ApplicationWindow` / `gtk::Application`; transitions dropped | moderate |
| 3 | ActivityCompat | androidx.core.app.ActivityCompat | 3/6 | re-apply CSS provider; no window recreate | trivial |
| 4 | ActivityInfo | android.content.pm.ActivityInfo | 4/32 | settings enums only; orientation locks dropped | trivial |
| 5 | ActivityManager | android.app.ActivityManager | 1/4 | read `/proc/meminfo` `MemTotal` | trivial |
| 6 | ActivityNotFoundException | android.content.ActivityNotFoundException | 4/8 | `Result` from `gtk::FileDialog` / `gio::AppInfo::launch_default_for_uri` | trivial |
| 7 | ActivityResult | androidx.activity.result.ActivityResult | 1/2 | child-process exit status / mpv IPC result | moderate |
| 8 | ActivityResultContracts | androidx.activity.result.contract.ActivityResultContracts | 8/19 | `gtk::FileDialog` (+ xdg-desktop-portal) | trivial |
| 9 | ActivityResultLauncher | androidx.activity.result.ActivityResultLauncher | 1/2 | async fn spawning external player | moderate |
| 10 | AdapterView | android.widget.AdapterView | 1/4 | `gtk::DropDown` `notify::selected` | trivial |
| 11 | AfterAll | org.junit.jupiter.api.AfterAll | 1/2 | Rust test fixture `Drop` / `ctor` | trivial |
| 12 | Alignment | androidx.glance.layout.Alignment | 4/18 | dropped (Android home-screen widget) | trivial |
| 13 | AndroidJUnit4ClassRunner | androidx.test.internal.runner.junit4.AndroidJUnit4ClassRunner | 1/4 | dropped; `criterion` optional | trivial |
| 14 | AndroidLogcatLogger | logcat.AndroidLogcatLogger | 1/2 | `tracing` + `tracing-subscriber` (stderr/journald) | trivial |
| 15 | AndroidSqliteDriver | app.cash.sqldelight.driver.android.AndroidSqliteDriver | 1/5 | `rusqlite` with bundled SQLite, PRAGMAs, `user_version` | moderate |
| 16 | AndroidXmlReader | nl.adaptivity.xmlutil.core.AndroidXmlReader | 1/2 | `quick-xml` + serde | trivial |
| 17 | Animatable | android.graphics.drawable.Animatable | 1/3 | `gdk_pixbuf::PixbufAnimation` / `gtk::MediaFile` | moderate |
| 18 | Animator | android.animation.Animator | 2/2 | dropped (dead code) | trivial |
| 19 | AnimatorSet | android.animation.AnimatorSet | 1/3 | `gtk::Widget::add_tick_callback` tween | moderate |
| 20 | AppCompatActivity | androidx.appcompat.app.AppCompatActivity | 2/6 | `gtk::ApplicationWindow` | trivial |
| 21 | AppCompatButton | androidx.appcompat.widget.AppCompatButton | 1/2 | `gtk::Button` | trivial |
| 22 | AppCompatDelegate | androidx.appcompat.app.AppCompatDelegate | 2/8 | `gtk::Settings::set_gtk_application_prefer_dark_theme`, `LC_MESSAGES` | moderate |
| 23 | AppCompatImageView | androidx.appcompat.widget.AppCompatImageView | 1/6 | `gtk::Picture` | moderate |
| 24 | AppCompatTextView | androidx.appcompat.widget.AppCompatTextView | 2/6 | `gtk::Label` | trivial |
| 25 | Application | android.app.Application | 60/137 | `AppContext` struct (paths, DB, settings) held in `Rc`/`Arc` | moderate |
| 26 | ApplicationInfo | android.content.pm.ApplicationInfo | 3/8 | extension package manifest struct; `cfg!(debug_assertions)` | moderate |
| 27 | Archive | me.zhanghai.android.libarchive.Archive | 3/19 | `zip` + `unrar`/`compress-tools` or `libarchive3-sys` | hard |
| 28 | ArchiveEntry | me.zhanghai.android.libarchive.ArchiveEntry | 2/14 | `ArchiveEntry { name, is_file }` Rust struct | trivial |
| 29 | ArchiveException | me.zhanghai.android.libarchive.ArchiveException | 3/6 | `ArchiveError` (`thiserror`) | trivial |
| 30 | AssetManager | android.content.res.AssetManager | 1/2 | `include_bytes!` / GResource | trivial |
| 31 | AssistContent | android.app.assist.AssistContent | 3/6 | dropped | trivial |
| 32 | AsyncImage | coil3.compose.AsyncImage | 5/10 | `gtk::Picture` + async cover loader/cache | moderate |
| 33 | AttributeSet | android.util.AttributeSet | 11/22 | no equivalent; subclass constructors with no XML attrs | trivial |
| 34 | AudioAttributesCompat | androidx.media.AudioAttributesCompat | 1/4 | mpv `audio-client-name` / PipeWire role | trivial |
| 35 | AudioFocusRequestCompat | androidx.media.AudioFocusRequestCompat | 1/3 | dropped (MPRIS pause-others optional) | trivial |
| 36 | AudioManager | android.media.AudioManager | 2/16 | mpv `volume` property; `wpctl` not touched | moderate |
| 37 | AudioManagerCompat | androidx.media.AudioManagerCompat | 1/4 | dropped (same as 35) | trivial |
| 38 | AuthPromptCallback | androidx.biometric.auth.AuthPromptCallback | 1/3 | dropped (or PAM via polkit later) | moderate |
| 39 | AuthenticationError | androidx.biometric.BiometricPrompt.AuthenticationError | 1/2 | dropped with 38 | trivial |
| 40 | Authenticators | androidx.biometric.BiometricManager.Authenticators | 1/3 | dropped with 38 | trivial |
| 41 | BackHandler | androidx.activity.compose.BackHandler | 14/31 | `gtk::ShortcutController` Escape/Alt+Left + nav stack | moderate |
| 42 | BackoffPolicy | androidx.work.BackoffPolicy | 5/10 | in-process scheduler (tokio) + `systemd --user` timer optional | moderate |
| 43 | BaselineProfileMode | androidx.benchmark.macro.BaselineProfileMode | 1/3 | dropped (Android AOT profile) | trivial |
| 44 | BaselineProfileRule | androidx.benchmark.macro.junit4.BaselineProfileRule | 1/2 | dropped | trivial |
| 45 | BeforeAll | org.junit.jupiter.api.BeforeAll | 1/2 | Rust test setup | trivial |
| 46 | BeforeEach | org.junit.jupiter.api.BeforeEach | 2/4 | Rust test setup (fresh fixture per `#[test]`) | trivial |
| 47 | BiometricManager | androidx.biometric.BiometricManager | 1/4 | dropped (see 38) | trivial |
| 48 | BiometricPrompt | androidx.biometric.BiometricPrompt | 2/7 | dropped (see 38) | trivial |
| 49 | Bitmap | android.graphics.Bitmap | 23/52 | `image` crate `DynamicImage` / `gdk::Texture` / `cairo::ImageSurface` | hard |
| 50 | BitmapDrawable | android.graphics.drawable.BitmapDrawable | 4/10 | `gdk::Texture` | trivial |
| 51 | BitmapFactory | android.graphics.BitmapFactory | 8/23 | `image::ImageReader` + `image::image_dimensions` | moderate |
| 52 | BitmapImage | coil3.BitmapImage | 1/2 | `gdk::Texture` result of the loader | trivial |
| 53 | BitmapRegionDecoder | android.graphics.BitmapRegionDecoder | 1/5 | `image` crop after full decode (or `libvips`) | moderate |
| 54 | Box | androidx.glance.layout.Box | 6/14 | dropped (Android home-screen widget) | trivial |

Recurring Rust crates named in this chunk: `gtk4` 0.11, `gdk-pixbuf`, `webkit6`, `image` (with `jpeg`, `png`, `gif`, `webp` features), `zip`, `quick-xml`, `serde`, `rusqlite` (feature `bundled`), `tracing`, `tracing-subscriber`, `thiserror`, `tokio`, `libmpv2` (or `libmpv-sys`), `ashpd` (xdg portals, optional). Cross-references: `area:12-network-stack` (HTTP, cookies, Cloudflare), `area:22-backup-restore`, `area:43-reader-core`, `area:45-player-core`, `area:23-downloads-manga`. These ids name the architecture areas of this plan set.

---

## 2. Windowing, activities, navigation

### 2.1 Activity
* Aniyomi: `android.app.Activity` is the base of the extension-install trampolines `app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt:18` and `.../manga/util/MangaExtensionInstallActivity.kt:18`, of the deep-link receivers `app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt:9` and `.../deeplink/manga/DeepLinkMangaActivity.kt:9`, and of casts/consts: `(context as? Activity)?.let { ActivityCompat.recreate(it) }` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt:93`), `Activity.RESULT_OK` (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:333`), `Activity.OVERRIDE_TRANSITION_OPEN/CLOSE` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:149,298`, `app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt:45,102`, `SecureActivityDelegate.kt:113`), `Activity.hasDisplayCutout()` (`app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt:62`), `ThemingDelegate.applyAppTheme(activity: Activity)` (`.../ui/base/delegate/ThemingDelegate.kt:11`). 13 files / 29 occurrences.
* Observed behavior to know: the install activities exist only because Android returns an install result solely via `startActivityForResult`; the deep-link activities rewrite the intent (`FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_NEW_TASK`, extra `INTENT_SEARCH_TYPE`) and forward to `MainActivity` with action `eu.kanade.tachiyomi.SEARCH` / `eu.kanade.tachiyomi.ANIMESEARCH` (`MainActivity.kt:592-596`: extras `query`, `filter`, `type`).
* Replacement: one `gtk::ApplicationWindow` main window (library/browse/updates/history/more as a `gtk::Stack`) plus separate top-level windows for the reader and the player (decision lives in `area:43-reader-core` / `area:45-player-core`). Deep links become CLI arguments / `gio::Application` `open` signal (`kurobayomi search --type manga "query"`), registered via a `.desktop` file `MimeType=x-scheme-handler/...` if the user wants it. Extension installation is a plain file operation (see `ApplicationInfo`, item 26), so the trampolines vanish.
* Preserve: query/filter/type of a global-search request; the "recreate on theme change" effect (item 3).
* Difficulty moderate. Risks: GTK has no per-window transition override (`OVERRIDE_TRANSITION_*` is dropped: user loses the zero-duration reader open animation, nothing else); notch/cutout logic (`hasDisplayCutout`) is dropped, desktops have none.

### 2.2 AppCompatActivity
* Aniyomi: `BaseActivity : AppCompatActivity()` (`app/src/main/java/eu/kanade/tachiyomi/ui/base/activity/BaseActivity.kt:13`) and `SecureActivityDelegateImpl` keeps `private lateinit var activity: AppCompatActivity` (`SecureActivityDelegate.kt:77`). 2 files / 6 occurrences. The delegate observes `onCreate` to apply the secure-screen flag and `onResume` to start app lock.
* Replacement: `gtk::ApplicationWindow` with `connect_map` (first show) and `connect_notify("is-active")` standing in for `onCreate`/`onResume`. Secure screen flag (`FLAG_SECURE`) has no Linux equivalent: drop (user loses screenshot blocking when the "secure screen" setting is on; keep the preference key for backup round trip but mark it inert in the UI).
* Difficulty trivial. Risk: `onResume` fires on every focus regain on Android; use `is-active` false-to-true edges, not `map`.

### 2.3 ActivityCompat
* Aniyomi: only `ActivityCompat.recreate(activity)` (3 files / 6 occ.): `SettingsAppearanceScreen.kt:93`, `app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt:108`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt:149` (reader recreates on `themeChangedListener`, `WebtoonConfig.kt:97`).
* Replacement: never recreate a window. Theme change = swap the app-level `gtk::CssProvider` and call `gtk::Settings::set_gtk_application_prefer_dark_theme`; reader widgets re-read colors from CSS classes on the next `notify`.
* Difficulty trivial. Risk: Webtoon reader holds per-page bitmaps whose background (`ImageUtil.chooseBackground`) is theme independent, so nothing needs rebuilding.

### 2.4 ActivityInfo
* Aniyomi: screen-orientation constants (4 files / 32 occ.): `PlayerActivity.kt:838-850` maps `PlayerOrientation` {Free, Video, Portrait, ReversePortrait, SensorPortrait, Landscape, ReverseLandscape, SensorLandscape} to `SCREEN_ORIENTATION_*`; `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt:20-60` maps `ReaderOrientation` to flags; `PlayerViewModel.kt:840` `cycleScreenRotations()`.
* Exact values to keep for settings/backup compatibility: `ReaderOrientation` stored flag values `DEFAULT=0x00000000`, `FREE=0x00000008`, `PORTRAIT=0x00000010`, `LANDSCAPE=0x00000018`, `LOCKED_PORTRAIT=0x00000020`, `LOCKED_LANDSCAPE=0x00000028`, `REVERSE_PORTRAIT=0x00000030`, mask `0x00000038`; `fromPreference` returns `DEFAULT` for an unknown value. `PlayerOrientation` enum names persist as the preference string `defaultPlayerOrientationType`.
* Observed upstream bug: `PlayerOrientation.ReverseLandscape` maps to `SCREEN_ORIENTATION_REVERSE_PORTRAIT` (`PlayerActivity.kt:849`), not `REVERSE_LANDSCAPE`. `cycleScreenRotations()` flips between SensorPortrait and SensorLandscape and also writes the preference.
* Replacement: drop orientation locking (desktop window is user resizable). Keep the two enums as serde types so imported backups and settings round-trip. "Video" orientation is replaced by an optional "resize window to video aspect on open" toggle (P2); `cycleScreenRotations` button is not shown.
* Difficulty trivial. User loses: forced portrait/landscape on a convertible/tablet.

### 2.5 AdapterView
* Aniyomi: `IgnoreFirstSpinnerListener : AdapterView.OnItemSelectedListener` (`app/src/main/java/eu/kanade/tachiyomi/widget/listener/IgnoreFirstSpinnerListener.kt:11,19`); it swallows the first `onItemSelected` (fired on attach) and forwards later ones. 1 file / 4 occ.; no remaining callers (`rg IgnoreFirstSpinnerListener` finds only its definition), i.e. dead code from the XML-view era.
* Replacement: `gtk::DropDown` emits `notify::selected` only on change if the model is populated before connecting the handler; connect after `set_selected`. Do not port the class.
* Difficulty trivial.

### 2.6 BackHandler
* Aniyomi: Compose back-press interception (14 files / 31 occ.). Concrete rules (OBSERVED):
  * Library tab: `BackHandler(enabled = state.selectionMode || state.searchQuery != null)`: first clear selection, else clear the search query (`app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt:299`, anime twin at `AnimeLibraryTab.kt:278`).
  * Manga/anime detail: if any chapter/episode is selected, deselect all, else navigate up (`app/src/main/java/eu/kanade/presentation/entries/manga/MangaScreen.kt:282,537`; `.../anime/AnimeScreen.kt:362,718`).
  * Updates screens: enabled while `selectionMode`, action `onSelectAll(false)` (`MangaUpdatesScreen.kt:49`, `AnimeUpdatesScreen.kt:53`).
  * Home: when current tab is not the start tab (or is More) go back to the start screen; the library tab is the start unless the default-tab preference says otherwise (`app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt:152`).
  * Sheets: close a nested sheet when `sheetNavigator.size > 1` (`app/src/main/java/eu/kanade/presentation/components/AdaptiveSheet.kt:42`), dismiss when alpha > 0 (`presentation-core/.../AdaptiveSheet.kt:108,191`).
  * Onboarding: `enabled = currentStep != 0`, decrement step (`app/src/main/java/eu/kanade/presentation/more/onboarding/OnboardingScreen.kt:45`).
  * Player subtitle panel: dismiss (`SubtitleSettingsPanel.kt:63`); player sheet (`PlayerSheet.kt:180`).
  * Script editor: `enabled = hasModified` -> show GoBack ("unsaved changes") dialog (`CodeEditScreen.kt:66`).
* Replacement: one `Navigator` struct (stack of pages) owned by the main window. A window-level `gtk::ShortcutController` (phase Capture) bound to `Escape` and `Alt+Left` (and mouse back button 8 via `gtk::GestureClick` button 8) calls `navigator.handle_back()`; each page implements `fn on_back(&mut self) -> bool` returning true when it consumed the event (clear selection, close search, close sheet, ask to discard). The first page that returns true wins, top-down; otherwise pop.
* Preserve: the precedence order above (selection before search before navigate-up).
* Difficulty moderate. Risk: Escape also closes dialogs and `gtk::SearchEntry`; ensure focused-entry Escape clears the entry first (GTK default) and does not bubble twice.

---

## 3. Intents, pickers, external player

### 3.1 ActivityNotFoundException
* Aniyomi: caught around `launch()` of file pickers and a settings intent (4 files / 8 occ.): `StorageStep.kt:86`, `SettingsDataScreen.kt:183`, `CreateBackupScreen.kt:76`, `SettingsAdvancedScreen.kt:171`. Messages: `file_picker_error` = "No file picker app found" (all three picker sites) and `battery_optimization_setting_activity_not_found` = "Couldn't open device settings" (battery site; `i18n/src/commonMain/moko-resources/base/strings.xml:928,630`). CreateBackup additionally refuses to start when `BackupCreateJob.isManualJobRunning(context)` and toasts `backup_in_progress`.
* Replacement: `gtk::FileDialog` returns `Err` on failure/portal absence; map any `Err` that is not `Dismissed` to the toast `file_picker_error`. The battery-optimization row is Android-only: removed from the UI (see 42, 3.2 below).
* Difficulty trivial. Risk: distinguish user cancel (`gtk::DialogError::Dismissed`, silent) from real failure (toast).

### 3.2 ActivityResultContracts
* Aniyomi uses (8 files / 19 occ.), each with its exact contract and Linux replacement:

| Site | Contract | Purpose | Replacement |
|---|---|---|---|
| `SettingsDataScreen.kt:128` (`storageLocationPicker`), `StorageStep.kt` | `OpenDocumentTree()` | pick base storage directory; takes persistable R/W grant; `SecurityException` -> toast `file_picker_uri_permission_unsupported` | `gtk::FileDialog::select_folder`; store the absolute path; no grant concept; drop the SecurityException branch |
| `SettingsDataScreen.kt:198` | `GetContent()` wrapped in `Intent.createChooser(..., file_select_backup)` | choose backup file; null -> toast `file_null_uri_error`; then push `RestoreBackupScreen(uri)` | `gtk::FileDialog::open` with `gtk::FileFilter` for `*.tachibk;*.proto.gz;*.json;*.gz` (names finalised in `area:22-backup-restore`) |
| `SettingsDataScreen.kt:373` | `CreateDocument("text/csv")` | library CSV export | `gtk::FileDialog::save` with initial name and `text/csv` filter |
| `CreateBackupScreen.kt:46` | `CreateDocument("application/*")` with name `BackupCreator.getFilename()` | create backup; then `takePersistableUriPermission` | `gtk::FileDialog::save` with initial name from the same filename formula |
| `SettingsAdvancedScreen.kt:354` | `OpenDocument()` | pick display ICC colour profile (`displayProfile` pref stores the URI string) | `gtk::FileDialog::open`, `*.icc;*.icm`; store path string |
| `AnimeScreen.kt:333`, `MangaScreen.kt:262` | `GetContent()` with `"image/*"` | replace custom cover/background | `gtk::FileDialog::open` with image filter (`image/*`) |
| `PlayerSheets.kt:105,123` | `OpenDocument()` with `arrayOf("*/*")` | add external subtitle / audio track | `gtk::FileDialog::open`; pass the path to mpv `sub-add`/`audio-add` |
| `PermissionStep.kt:87` | `RequestPermission()` | POST_NOTIFICATIONS on Android 13+ | dropped; Linux has no runtime permission. Onboarding permission page reduced to nothing (see `area:36-ui-shell-navigation-security-webview`) |
| `MainActivity.kt:331` | `StartActivityForResult()` | external-player result | see 3.3 |

* Replacement notes: `gtk::FileDialog` (GTK 4.10+, available in 4.16) uses the xdg-desktop-portal FileChooser automatically under Flatpak/KDE, which also gives sandbox-safe paths. Use `ashpd` only if a portal feature `FileDialog` lacks is needed (none is).
* Preserve: toast keys `file_picker_error`, `file_null_uri_error`; default file names; filters.
* Difficulty trivial. Risk: portals return `/run/user/<uid>/doc/...` paths under Flatpak; store the path, not a file descriptor, and re-check existence on use.

### 3.3 ActivityResult
* Aniyomi: the result object of the external-player launch. `registerForActivityResult(StartActivityForResult())` in `MainActivity.kt:330-343`: if `result.resultCode == Activity.RESULT_OK`, restore `animeId`/`episodeId` from saved state (`SAVED_STATE_ANIME_KEY = "saved_state_anime_key"`, `SAVED_STATE_EPISODE_KEY = "saved_state_episode_key"`), call `ExternalIntents.externalIntents.initAnime(animeId, episodeId)` and then `onActivityResult(result.data)`, which writes back playback position/watched status reported by the external app. 1 file / 2 occ.
* Replacement: external players are launched as a child process (`mpv`, `vlc`, `celluloid`, user-configurable command template, `tokio::process::Command`). Progress write-back is only possible for players that report it: for mpv launched with `--input-ipc-server=<sock>`, read `time-pos`/`duration`/`eof-reached` over the IPC socket; for others, write-back is dropped (user loses auto-progress with arbitrary external players; detail in `area:45-player-core`).
* Difficulty moderate. Risk: no standard exit protocol, so do not invent one.

### 3.4 ActivityResultLauncher
* Aniyomi: `private var externalPlayerResult: ActivityResultLauncher<Intent>? = null` in the `MainActivity` companion (`MainActivity.kt:600`), assigned in `onCreate`, used in `startPlayerActivity` (`MainActivity.kt:629`): `externalPlayerResult?.launch(intent) ?: return`. If `extPlayer` is true the function first starts the in-app HTTP server service (`startHttpServerService(context, sourceId, timeout = 5.seconds)` which waits `withTimeoutOrNull(timeout) { HttpServerService.isRunning.first { it } }`), toasts `http_server_start_failure` on failure, builds the intent via `ExternalIntents.newIntent`, and toasts the exception message on build failure. 1 file / 2 occ.
* Replacement: `async fn launch_external_player(ctx, anime_id, episode_id, video)`; the local proxy server is a `tokio` task using `hyper`/`axum` bound to `127.0.0.1:0` (ephemeral port) with a 5 s readiness timeout (`tokio::time::timeout(Duration::from_secs(5), ready_rx)`). Needed only when the external player cannot send required request headers; mpv can, so headers are passed with `--http-header-fields` and the proxy is skipped for mpv (P2 for other players).
* Difficulty moderate. Risk: port reuse and lifetime; shut the proxy down on player exit or after 30 s idle (Proposal).

### 3.5 AssistContent
* Aniyomi: `MainActivity.onProvideAssistContent` returns the current page URL to the Android "assistant" (`MainActivity.kt:347-355`), via `AssistContentScreen.onProvideAssistUrl()` implemented by browse-source and webview screens (`app/src/main/java/eu/kanade/presentation/util/Navigator.kt:63`). Also overridden in `ReaderActivity.kt` and `WebViewActivity.kt`. 3 files / 6 occ.
* Replacement: dropped (Android "Now on Tap"/assistant integration). Equivalent user action "Open in browser" and "Share" exist (see WebView, item 1).
* Difficulty trivial. User loses: assistant context share only.

---

## 4. WebView

### 4.1 AccompanistWebViewClient
* Aniyomi: `object : AccompanistWebViewClient()` in `app/src/main/java/eu/kanade/presentation/webview/WebViewScreenContent.kt:68` (library `com.kevinnzou:compose-webview`). Behaviors (OBSERVED):
  * `onPageStarted` and `doUpdateVisitedHistory` update `currentUrl` and call `onUrlChange(url)`.
  * `onPageFinished`: runs `document.documentElement.outerHTML` (`WebView.getHtml()`, `core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt:73`); `showCloudflareHelp = "window._cf_chl_opt" in html || "Ray ID is" in html`. When true a banner with string `information_cloudflare_help` ("Tap here for help with Cloudflare") links to `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare`.
  * `shouldOverrideUrlLoading`: `blob:http` -> let the WebView handle (return false); `intent://` -> block (return true); everything else -> `view.loadUrl(url, headers)` so custom headers (including `user-agent` set from `headers["user-agent"]` via `settings.userAgentString`) apply to every navigation.
  * Toolbar actions in order: back, forward (both disabled when impossible), overflow: refresh, share (`action_share`), open in browser (`action_open_in_browser`), clear cookies (`pref_clear_cookies` = "Clear cookies").
  * Settings (`WebViewUtil.kt:78-93`): `javaScriptEnabled=true`, `domStorageEnabled=true`, `useWideViewPort=true`, `loadWithOverviewMode=true`, `cacheMode=LOAD_DEFAULT`, zoom support with built-in controls hidden, `CookieManager.acceptThirdPartyCookies`. Debug-only `WebView.setWebContentsDebuggingEnabled(true)` when `BuildConfig.DEBUG` and `FLAG_DEBUGGABLE` (`WebViewScreenContent.kt:217`).
  * Progress: `LoadingState.Initializing` and `LoadingState.Loading(progress)` drive a bottom `LinearProgressIndicator` (indeterminate while initialising).
  1 file / 2 occ. The same WebView is used headlessly by the network layer for Cloudflare solving (`core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt:87`, details in `area:12-network-stack`).
* Replacement: `webkit6` crate (WebKitGTK 6.0, GTK4). Map: `WebView` + `NetworkSession`; `connect_load_changed` (STARTED/REDIRECTED/COMMITTED/FINISHED) for URL and the Cloudflare check; `connect_decide_policy` for navigation filtering (block scheme `intent`, allow `blob`); `webkit6::Settings::set_enable_javascript(true)`, `enable_html5_local_storage(true)`; `WebView::evaluate_javascript` for `outerHTML`; `can_go_back()`/`can_go_forward()`/`reload()`; `estimated_load_progress` property for the bar; `CookieManager` (`NetworkSession::cookie_manager`) for clear cookies via `delete_all_cookies`/`delete_cookies_for_domain`; custom headers per navigation are applied by cancelling and re-issuing with `load_request` of a `URIRequest` whose `http_headers()` are set. Set UA with `Settings::set_user_agent`.
* Preserve: the two Cloudflare markers; blob/intent rules; header propagation; cookie clearing scoped to the current URL host. Cookie hand-off to `reqwest` (copy `cf_clearance` plus matching User-Agent into the shared `reqwest` cookie jar after `FINISHED`) is specified in `area:12-network-stack`.
* Difficulty hard. Risks: WebKitGTK is a heavy optional runtime dependency (`webkit2gtk-6.0` on Arch is `webkitgtk-6.0`); `cf_clearance` is bound to the UA string and TLS fingerprint, so the UA used by `reqwest` must equal WebKitGTK's. Header injection per navigation is weaker than Android's `loadUrl(url, headers)`. If the user does not install WebKitGTK the feature must degrade to "Open in browser" (`gio::AppInfo::launch_default_for_uri`).

---

## 5. Compose-era views replaced by GTK widgets

### 5.1 AppCompatButton
* Aniyomi: Webtoon transition holder builds a retry button `AppCompatButton(context)` with text `action_retry` ("Retry") that calls `viewer.activity.requestPreloadChapter(toChapter)` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonTransitionHolder.kt:126`). 1 file / 2 occ. The pager variant uses `ReaderButton` (`PagerTransitionHolder.kt:119`).
* Replacement: `gtk::Button::with_label("Retry")`, `connect_clicked` re-queues chapter preload. Difficulty trivial.

### 5.2 AppCompatTextView
* Aniyomi: transition-page messages: loading `transition_pages_loading` = "Loading pages…" and error `transition_pages_error` = "Failed to load pages: %1$s" (`error.message ?: ""`) in `PagerTransitionHolder.kt:106,119` and `WebtoonTransitionHolder.kt:108,121`. 2 files / 6 occ.
* Replacement: `gtk::Label` (wrap, centered) next to `gtk::Spinner` (loading). Difficulty trivial.

### 5.3 AppCompatImageView
* Aniyomi: in `ReaderPageImageView` it is the view for animated pages in webtoon mode (non-webtoon uses `PhotoView`): `prepareAnimatedImageView` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt:353-357`), `setAnimatedImage` (`:394`), `recycle` calls `it.dispose()` (`:188`). 1 file / 6 occ. Constants in the file: `MAX_ZOOM_SCALE = 5F` (`:442`), scale levels `setScaleLevels(1F, 2F, MAX_ZOOM_SCALE)`, double tap toggles 1x <-> 2x, zoom animation `withDuration(500)` with `EASE_IN_OUT_QUAD` (`:155`), 250 ms (`:241`), duration scaled by system animator scale `(this * animatorDurationScale).toInt().coerceAtLeast(1)` (`:420`).
* Replacement: `gtk::Picture` with `set_paintable(&gdk::Paintable)` (a `gdk_pixbuf::PixbufAnimation` wrapped in `gdk::Paintable` via a small `AnimatedPaintable`, or `gtk::MediaFile` for WebP/APNG). `set_content_fit(gtk::ContentFit::Contain)`. Zoom is implemented in the reader widget itself (area:43-reader-core): scale levels 1.0 / 2.0 / 5.0, double tap (double click) toggles 1.0 and 2.0 around the click point, animation 500 ms ease-in-out-quad. System animator scale = `gtk::Settings::is_gtk_enable_animations` (0 or 1).
* Preserve: `MAX_ZOOM_SCALE`, two-level double tap, durations. Difficulty moderate. Risk: GTK4 `gtk::Picture` does not pan/zoom; reader needs `gtk::GestureZoom` + custom snapshot.

### 5.4 Animatable
* Aniyomi: `android.graphics.drawable.Animatable` marks an animated drawable. `setImage(drawable, config)` picks the animated view if `drawable is Animatable` (`ReaderPageImageView.kt:165`); after load `(drawable as? Animatable)?.start()` (`:407`). 1 file / 3 occ.
* Replacement: decide animation by decoded format: GIF (`gif` crate via `image`), animated WebP (`webp-animation` or `image` webp), APNG, animated AVIF/JXL are not supported (render first frame). `gdk_pixbuf::PixbufAnimation::from_stream_async` decodes GIF/APNG/WebP when the loaders exist; call `iter()` and tick with `glib::timeout_add_local(delay)`; or hand file path to `gtk::MediaFile::for_file`. Start on map, stop on unmap/recycle.
* Difficulty moderate. Risk: memory (all frames decoded); cap at 300 frames or 100 MB then fall back to first frame (Proposal).

### 5.5 Animator
* Aniyomi: only the `Animator.AnimatorListener` parameter of `RevealAnimationView.showRevealEffect(centerX, centerY, listener)` (`app/src/main/java/eu/kanade/tachiyomi/widget/RevealAnimationView.kt:53`) and `WebtoonRecyclerView.kt` (no direct use beyond import). 2 files / 2 occ. `RevealAnimationView` has no caller in the tree (`rg RevealAnimationView` only matches its own file): dead code. Behavior: `createCircularReveal(view, cx, cy, 0f, height)` with `duration = 350` ms.
* Replacement: drop. Difficulty trivial.

### 5.6 AnimatorSet
* Aniyomi: Webtoon zoom and fling (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonRecyclerView.kt:112,138`). 1 file / 3 occ.
  * `zoom(fromRate,toRate,fromX,toX,fromY,toY)`: plays three `ValueAnimator.ofFloat` together (x, y translation and scale), duration `ANIMATOR_DURATION_TIME = 200` ms (`:346`), `DecelerateInterpolator` (factor 1.0: `1 - (1 - t)^2`), sets `isZooming=true` until end, then `currentScale = toRate`.
  * `zoomFling(velocityX, velocityY)`: returns false if `currentScale <= 1f`; `distanceTimeFactor = 0.4f`; `dx = 0.4 * velocityX / 2`; `newX = getPositionX(x + dx)` (clamped to `[-maxPositionX, maxPositionX]`); Y fling applies only when `atFirstPosition || atLastPosition`; duration 400 ms, decelerate.
* Replacement: a tiny `Tween { from, to, duration_ms, easing }` driven by `widget.add_tick_callback` using `FrameClock::frame_time()`; one tick callback updates x, y, scale together (that is the AnimatorSet). Easing `ease_out_quad(t) = 1 - (1 - t) * (1 - t)`.
* Difficulty moderate. Risk: coordinates clamp rules must be ported with the viewport math in `area:43-reader-core`.

---

## 6. Theme, locale, application object

### 6.1 AppCompatDelegate
* Aniyomi: (a) theme mode: `setAppCompatDelegateThemeMode(ThemeMode)` maps `LIGHT -> MODE_NIGHT_NO`, `DARK -> MODE_NIGHT_YES`, `SYSTEM -> MODE_NIGHT_FOLLOW_SYSTEM` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt:12-16`), called at start in `App.kt` after the `hardwareBitmapThreshold` setup; (b) per-app language: `AppCompatDelegate.getApplicationLocales().get(0)?.toLanguageTag() ?: ""` and `setApplicationLocales(LocaleListCompat.forLanguageTags(tag))` / empty list for "system" (`app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt:47-56`). 2 files / 8 occ.
* Replacement: (a) `ThemeMode {Light, Dark, System}`; Light/Dark -> `gtk::Settings::default().set_gtk_application_prefer_dark_theme(bool)`; System -> do not override and follow `gtk-application-prefer-dark-theme` / the KDE colour scheme via `xdg-desktop-portal` `org.freedesktop.appearance color-scheme` (`ashpd::desktop::settings`). Respect the user's GTK theme (project rule). (b) Language: `gettext-rs` / `fluent` catalogs; "system" = empty tag; a chosen tag sets `LANGUAGE` env var before GTK init and requires restart (Linux cannot switch live): show toast "Restart to apply" (Proposal key `restart_to_apply`).
* Preserve: three-valued theme enum names; empty string = follow system for language.
* Difficulty moderate. Risk: GTK reads the colour scheme once at start under some themes; listen to the portal `SettingChanged` signal.

### 6.2 Application
* Aniyomi: `android.app.Application` is the DI anchor and Android context everywhere: `class App : Application()` (`app/src/main/java/eu/kanade/tachiyomi/App.kt`), `AppModule(val app: Application)`, and `Injekt.get<Application>()` 72 times, mainly to reach `SharedPreferences` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/utils/Preferences.kt:24`, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt:17`), toasts, notifications, files dir. 60 files / 137 occ.
* Startup sequence (OBSERVED, `App.kt:95-200`): WebView data dir suffix for non-main processes; import DI modules (`PreferenceModule`, `AppModule`, `DomainModule`, `SYDomainModule`); `setupNotificationChannels()`; lifecycle observer; incognito-mode notification (id `Notifications.ID_INCOGNITO_MODE`, channel `CHANNEL_INCOGNITO_MODE`, ongoing, text `notification_incognito_text`, tap = broadcast `ACTION_DISABLE_INCOGNITO_MODE` one-shot); `hardwareBitmapThreshold` default set to `GLUtil.DEVICE_TEXTURE_LIMIT` when unset; widget managers init; verbose logger install; `initializeMigrator()` (key `__APP_STATE_last_version_code`, default 0, via `Migrator.initialize(old, new, migrations, onMigrationComplete)`); Coil `allowRgb565(DeviceUtil.isLowRamDevice(...))` (`App.kt:204`).
* Replacement: `struct AppContext { dirs: Dirs, db: Db, settings: Settings, http: HttpClient, notifier: Notifier, ... }` created once in `main` and passed as `Rc<AppContext>` on the GTK main thread, `Arc<AppContext>` into tokio tasks. `Dirs` from the `directories` crate / XDG: config `$XDG_CONFIG_HOME/kurobayomi`, data `$XDG_DATA_HOME/kurobayomi`, cache `$XDG_CACHE_HOME/kurobayomi`. SharedPreferences -> `settings.toml` (area:02-preferences-system) and per-extension prefs in the DB or `ext_prefs/<source_id>.toml`. Toast -> in-app `gtk::Revealer` banner (since no libadwaita `Toast`). Notifications -> `gio::Notification` (see area:35-data-misc-updater-notification-export). Migrator keeps the same algorithm with app-state key `last_version_code`.
* Preserve: boot order (dirs, settings, DB open + migrations, migrator, UI); incognito indicator becomes a persistent header-bar badge plus a `gio::Notification` only when the window is not focused (Proposal).
* Difficulty moderate. Risk: the 137 uses show how pervasive hidden global context is; forbid global statics except `AppContext` accessor behind `OnceLock` to ease tests.

### 6.3 ApplicationInfo
* Aniyomi: (1) debug flag `0 != webView.context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE` (`WebViewScreenContent.kt:218`); (2) extension APK metadata. `AnimeExtensionLoader`/`MangaExtensionLoader` read `appInfo.metaData` keys `METADATA_NAME`, `METADATA_EXTENSION_LIB` (lib version must be in `SUPPORTED_LIB_VERSIONS = listOf(14.0, 16.0, 17.0)` for anime, `AnimeExtensionLoader.kt:53`), `METADATA_CONTENT_WARNING` (> 0), `METADATA_NSFW` (== 1), `METADATA_IS_TORRENT` / `METADATA_TORRENT` (== 1), `METADATA_SOURCE_CLASS`, `METADATA_SOURCE_FACTORY`, `appInfo.sourceDir`; load classes via `ChildFirstPathClassLoader(appInfo.sourceDir, null, context.classLoader)` with fallback `PathClassLoader`; `fixBasePaths(apkPath)` fills `sourceDir`/`publicSourceDir` on Android 13+ (`AnimeExtensionLoader.kt:429`); (3) extension backup reads `publicSourceDir` and the raw APK bytes (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/ExtensionsBackupCreator.kt:24,37`); (4) `ContextExtensions.kt:163` `packageManager.getApplicationInfo(packageName, 0)`. 3 files / 8 occ.
* Replacement: Android APK/dex extensions cannot run on Linux. Kurobayomi's source mechanism (area:20-extension-strategy-redesign) loads Rust-native plugins (WASM `wasmtime` or dynamic libs) described by a `manifest.toml` struct `ExtensionManifest { pkg_name, name, lib_version, nsfw: bool, torrent: bool, sources: Vec<SourceDecl> }`; `lib_version` supported set is a Rust const slice. Debug flag = `cfg!(debug_assertions)`. Extension backup entries (`BackupExtension(packageName, apk bytes)`) are kept for read-only import and ignored on restore with a toast (Proposal: `restore_extensions_skipped`).
* Difficulty moderate. Risk: this is the central incompatibility: the user's existing Aniyomi extensions are not usable; this is called out as dropped-with-loss in `area:20-extension-strategy-redesign`.

---

## 7. Storage, DB, archives, XML

### 7.1 AndroidSqliteDriver
* Aniyomi: two SQLDelight databases built in `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-130`: manga DB `AndroidSqliteDriver(schema = Database.Schema, name = "tachiyomi.db", ...)` and anime DB `name = "tachiyomi.animedb"` with `AnimeDatabase.Schema`. Factory: `RequerySQLiteOpenHelperFactory()` (bundled Requery SQLite) in release, `FrameworkSQLiteOpenHelperFactory()` in debug on API >= 30. `onOpen` runs three PRAGMAs, each via `db.query("PRAGMA $pragma")`: `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL`. Column adapters: `History.Adapter(last_readAdapter = DateColumnAdapter)`, `Mangas.Adapter(genreAdapter = StringListColumnAdapter, ...)`. 1 file / 5 occ.
* Replacement: `rusqlite` with feature `bundled` (consistent SQLite version). Open `data_dir/tachiyomi.db` and `data_dir/tachiyomi.animedb` (keep the file names so a copied Android DB can be imported), run the same three PRAGMAs after open, schema versioning with `PRAGMA user_version` and an ordered list of migration scripts (SQLDelight `.sqm` files, area:03-db-manga-schema-migrations (and area:04-db-anime-schema-migrations)). Adapters become `FromSql`/`ToSql` impls: date = integer epoch millis; string list = comma-separated per `StringListColumnAdapter` (separator and escaping documented in `area:03-db-manga-schema-migrations (and area:04-db-anime-schema-migrations)`). Single writer connection behind a `Mutex` + a pool of read-only connections (`r2d2_sqlite`) for WAL concurrency; run DB calls on `tokio::task::spawn_blocking` or a dedicated thread.
* Preserve: the two-DB split, WAL, foreign keys ON. Difficulty moderate. Risk: SQLDelight flows (`asFlow().mapToList`) re-emit on table change; Rust needs an explicit change-notification bus (`tokio::sync::watch` per table) to refresh the library after writes.

### 7.2 AndroidXmlReader
* Aniyomi: reads `ComicInfo.xml` for local manga: `AndroidXmlReader(stream, "UTF-8").use { xml.decodeFromReader<ComicInfo>(it) }` then `manga.copyFromComicInfo(comicInfo)` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:236`). 1 file / 2 occ. File name constant `COMIC_INFO_FILE = "ComicInfo.xml"` (`core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt:12`).
* Field mapping to preserve (`copyFromComicInfo`): `series` -> title; `writer` -> author; `summary` -> description; genre = distinct of `genre`, `tags`, `categories` values joined by `", "` (each trimmed; skip when empty); artist = distinct of `penciller`, `inker`, `colorist`, `letterer`, `coverArtist` (each split on `", "`, joined `", "`); `status` = `ComicInfoPublishingStatus.toSMangaValue(publishingStatus)`. Written elements (`getComicInfo`): `Series, Summary, Writer, Penciller, Genre, PublishingStatusTachiyomi`; namespaces `xmlns:xsd="http://www.w3.org/2001/XMLSchema"` and `xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"` on the root `ComicInfo`.
* Replacement: `quick-xml` with the `serialize` feature and a `#[derive(Deserialize, Serialize)] struct ComicInfo` (`#[serde(rename = "Series")]` on each of the 17 fields, all `Option<String>`).
* Difficulty trivial. Risk: `quick-xml` serde needs `$text` for element text; unknown elements must be ignored (default).

### 7.3 Archive
* Aniyomi: libarchive JNI (`me.zhanghai.android.libarchive`) in the `core/archive` module (3 files / 19 occ.):
  * `ArchiveInputStream(buffer, size)` (`core/archive/src/main/kotlin/mihon/core/archive/ArchiveInputStream.kt`): `Archive.readNew()`, `setCharset("UTF-8")`, `readSupportFilterAll`, `readSupportFormatAll`, `readOpenMemoryUnsafe(buffer, size)`; `read()` via direct `ByteBuffer`; `getNextEntry()` returns `ArchiveEntry(name, isFile)` where name = `pathnameUtf8(entry)` falling back to `pathname(entry).decodeToString()` and `isFile = filetype == AE_IFREG`.
  * `ArchiveReader(pfd)`: `Os.mmap(0, size, PROT_READ, MAP_PRIVATE, fd, 0)`; `useEntries { Sequence<ArchiveEntry> }`; `getInputStream(entryName)` scans from the start for an exact name match (O(n) per page) and returns the stream positioned at the entry; `close()` -> `munmap`.
  * `ZipWriter(context, file)`: `writeSetFormatZip`, `writeZipSetCompressionStore` (**stored, no compression**), 8192-byte direct buffer; per file sets name (UTF-8), POSIX stat (mode, uid, gid, size, times), writes data; used by `MangaDownloader.archiveChapter` to produce `<dir>.cbz` via a `.tmp`-suffixed file then `renameTo("$dirname.cbz")` and delete tmp dir (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:632-645`).
  * Supported local-manga archive extensions: `zip, cbz, rar, cbr, 7z, cb7, tar, cbt` (`source-local/src/commonMain/kotlin/tachiyomi/source/local/io/Archive.kt:17`); epub handled separately (`Format.kt:18`). Local anime video container list: `avi, flv, mkv, mov, mp4, webm, wmv, torrent` (`Archive.kt:8`).
  * Page ordering in `ArchivePageLoader` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/ArchivePageLoader.kt:14-26`): keep entries with `isFile && ImageUtil.isImage(name){stream}` (content sniffed, not by extension), sort by `compareToCaseInsensitiveNaturalOrder` on the entry name, page index = position after sort.
  * Epub (`EpubReader.kt`): container path `META-INF/container.xml` -> `rootfile@full-path`, default `OEBPS/content.opf`; pages = spine itemrefs mapped to manifest items with media-type `application/xhtml+xml`; images = `<img src>` and `<image xlink:href>` in each page, paths resolved relative to the page with canonicalisation; path separator detected as `\` if `META-INF\container.xml` exists, else `/`.
* Replacement: `zip` crate (read: `ZipArchive<File>` with `by_name`; write: `ZipWriter` with `CompressionMethod::Stored`) for zip/cbz; `unrar` (needs libunrar) or the `compress-tools`/`libarchive3-sys` binding to system libarchive for rar/7z/tar. Recommended (Proposal): bind system `libarchive` via `compress-tools` for the read side to keep the exact format coverage, and use `zip` for the write side. Natural order: `natord` or `lexical-sort::natural_lexical_cmp` with case-insensitive compare. Epub: `quick-xml` + the same algorithm. Index entry offsets once (`Vec<(name, idx)>`) to avoid the O(n) scan per page.
* Difficulty hard (formats), the read-by-name scan is the main performance hazard. Risk: libarchive streams are forward-only; random page access needs a cache of extracted bytes (limit 50 MB) or a per-entry reopen.

### 7.4 ArchiveEntry
* Aniyomi: libarchive `ArchiveEntry` is used to build/inspect entries (`ArchiveEntry.new2`, `clear`, `setPathnameUtf8`, `setStat`, `filetype`, `AE_IFREG`, `StructStat`, `StructTimespec`) in `ArchiveInputStream.kt` and `ZipWriter.kt`; Aniyomi's own `class ArchiveEntry(val name: String, val isFile: Boolean)` (`core/archive/src/main/kotlin/mihon/core/archive/ArchiveEntry.kt`) is what callers see. 2 files / 14 occ. for the libarchive class.
* Replacement: `struct ArchiveEntry { name: String, is_file: bool }`; for write, the `zip` crate's `FileOptions::default().unix_permissions(mode).last_modified_time(dt)` replaces `StructStat`.
* Difficulty trivial.

### 7.5 ArchiveException
* Aniyomi: libarchive `ArchiveException` thrown by `Archive.*` calls; caught in `ArchiveInputStream.init` (close then rethrow), `ArchiveReader.getInputStream` (close then rethrow) and `ZipWriter.init` (3 files / 6 occ.). The reader surfaces failures as page load errors using `transition_pages_error`.
* Replacement: `#[derive(thiserror::Error)] enum ArchiveError { Io(#[from] std::io::Error), Zip(#[from] zip::result::ZipError), Format(String) }`; same close-on-error discipline via `Drop`.
* Difficulty trivial.

### 7.6 AssetManager
* Aniyomi: `PlayerActivity.copyAssets(mpvDir)` copies `subfont.ttf` and `cacert.pem` from APK assets to the mpv config dir with `assetManager.open(filename, AssetManager.ACCESS_STREAMING)`; skips if the destination size equals `ins.available()`; logs `Copied asset file` (WARN) or `Skipping copy ... exists same size` (VERBOSE); IOException logged at ERROR (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:490-512`). `cacert.pem` path is set as mpv option `tls-ca-file` (`AniyomiMPVView.kt:135`). The bridge script `aniyomi.lua` is copied too (`PlayerActivity.kt:482-488`). 1 file / 2 occ.
* Replacement: `include_bytes!`/GResource for the bundled `aniyomi.lua` bridge; written to `$XDG_CONFIG_HOME/kurobayomi/mpv/scripts/` only when size differs. `subfont.ttf` is unnecessary (system fonts via fontconfig); `cacert.pem` is unnecessary (mpv uses system CA store `/etc/ssl/certs`). Both dropped.
* Difficulty trivial.

---

## 8. Images and bitmaps

### 8.1 Bitmap
* Aniyomi: `android.graphics.Bitmap` (23 files / 52 occ.). Representative uses:
  * `ImageUtil.splitInHalf`, `rotateImage`, `splitAndMerge`, `splitTallImage` (`core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:128-250`). Exact algorithms: `splitInHalf(side)`: width/2 crop, RIGHT crop starts at `width - width/2`; output JPEG quality 100. `splitAndMerge(upperSide)`: result `createBitmap(width/2, height*2)`; right half drawn to upper and left to bottom (or swapped when `upperSide == LEFT`). `rotateImage(degrees)`: `Matrix.postRotate`. Tall-image split (download-time): `isTallImage` is `(outHeight / outWidth) > 3` using **integer division** (so ratio must be >= 4.0), `optimalImageHeight = max(displayHeight, displayWidth) * 2`, `partCount = (imageHeight - 1) / optimalImageHeight + 1`, `optimalSplitHeight = imageHeight / partCount`, last part absorbs remainder; part file name `"${prefix}__${"%03d".format(Locale.ENGLISH, index + 1)}.jpg"`; on any exception delete created parts and keep the original; animated images are never split.
  * `chooseBackground` (`ImageUtil.kt:340-600`): background detection of page edges; returns white (`<50` px images), black, or a top-to-bottom `GradientDrawable`; `isDark = r<40 && g<40 && b<40 && alpha>200`, `isCloseTo` threshold 30 per channel, `isWhite = r+g+b > 740`, `overallBlackPixels > 9` rule.
  * Coil decoder `TachiyomiImageDecoder` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/TachiyomiImageDecoder.kt:25-60`): decodes with `sampleSize` from `DecodeUtils.calculateInSampleSize`, converts to a hardware bitmap if `ImageUtil.canUseHardwareBitmap` (max(w,h) <= `hardwareBitmapThreshold`).
  * `MonetColorScheme.extractSeedColorFromImage(bitmap)`: `Score.score(QuantizerCelebi.quantize(pixels, 128), 1, 0)[0]` unused (`@Suppress("Unused")`).
  * Player thumbnail tiles: `PlayerViewModel.createThumbnail` crops `Bitmap.createBitmap(tile, x, y, width, height)` from a 3-entry LRU `LinkedHashMap<Int, Bitmap>(4, 0.75f, true)` with `removeEldestEntry = size > 3` (`PlayerViewModel.kt:259-260,610`); fetch throttled to at most one every 2 s (`lastThumbnailFetch`, `PlayerViewModel.kt:571`).
  * Others: notifications large icon and widget covers (`BaseMangaUpdatesGridGlanceWidget.kt`), `ImageSaver`, `MangaCoverDialog.kt` / `AnimeImagesDialog.kt`, `WebViewScreenContent.kt` favicon parameter (unused).
* Replacement: in-memory pixel type `image::DynamicImage` / `RgbaImage` for processing; `gdk::MemoryTexture` (from RGBA bytes with `gdk::MemoryFormat::R8g8b8a8`) for display; `cairo::ImageSurface` only for drawing. Encode JPEG quality 100 with `image::codecs::jpeg::JpegEncoder::new_with_quality(w, 100)`. Decoder with downscale: `image::ImageReader` then `resize` via `fast_image_resize` to the viewport size (replaces `inSampleSize`); formats beyond the `image` crate (JXL, HEIF, AVIF) via `libjxl`/`libheif-rs`/`libavif` features if wanted (`area:43-reader-core`). Thumbnail LRU: `lru::LruCache::new(NonZeroUsize::new(3))`.
* Preserve: all numeric thresholds above, JPEG quality 100, part naming, error-rollback semantics, LRU size 3, 2 s throttle.
* Difficulty hard (many call sites, performance). Risks: `image` decodes fully into RAM (a 12 000 px webtoon strip is ~144 MB RGBA); decode on a rayon/`spawn_blocking` thread, downscale immediately, and cap the loaded-pages window (`area:43-reader-core`).

### 8.2 BitmapDrawable
* Aniyomi: wrapper whose `bitmap` is extracted (`DrawableExtensions.kt:10` `is BitmapDrawable -> bitmap`), copied in the cover dialogs (`MangaCoverDialog.kt:179`, `AnimeImagesDialog.kt:218`: `BitmapDrawable(resources, bitmap.copy(...))` so the share/save copy survives recycling) and branched in `ReaderPageImageView.kt:312` (`is BitmapDrawable -> setImage(ImageSource.bitmap(data.bitmap))`). 4 files / 10 occ.
* Replacement: `gdk::Texture` (immutable, refcounted; no defensive copy needed). Difficulty trivial.

### 8.3 BitmapFactory
* Aniyomi (8 files / 23 occ.): (a) `BitmapFactory.Options.inJustDecodeBounds = true` to read dimensions without decoding (`ImageUtil.kt:594-597`, used by `isWideImage`: `outWidth > outHeight`, `isTallImage`, `splitData`, `canUseHardwareBitmap`); (b) `decodeStream` for the three bitmap ops in 8.1; (c) `decodeResource(resources, R.mipmap.ic_launcher)` as the notification large icon (`MangaLibraryUpdateNotifier.kt:67`, `AnimeLibraryUpdateNotifier.kt:68`, `MangaDownloadNotifier.kt:34`, `AnimeDownloadNotifier.kt:34`, `BackupNotifier.kt:29,39`, `HttpServerService.kt:75`); (d) extension API `AnimeHttpSource.getImageTile(url)`: GET with `client` and source `headers` then `BitmapFactory.decodeStream` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:534`).
* Replacement: (a) `image::image_dimensions(path)` / `ImageReader::into_dimensions()` (header-only, no pixel decode); (b) `ImageReader::with_guessed_format().decode()`; (c) app icon is named in `gio::Notification::set_icon` (themed icon name `kurobayomi`), no bitmap; (d) the Rust source trait returns `Result<DynamicImage>` after a `reqwest` GET using the source headers.
* Difficulty moderate. Risk: `image_dimensions` can fail on truncated files; treat as "not tall/wide" (matches Aniyomi's lenient path where 0 dims give no split).

### 8.4 BitmapImage
* Aniyomi: Coil 3 result type; the webtoon page path enqueues an `ImageRequest` with `memoryCachePolicy(DISABLED)`, `diskCachePolicy(DISABLED)`, `size(ViewSizeResolver)`, `precision(INEXACT)`, `.cropBorders(config.cropBorders)`, `.customDecoder(true)`, `.crossfade(false)` and in `onSuccess` casts `result as BitmapImage` to take `.bitmap` (`ReaderPageImageView.kt:330`). 1 file / 2 occ. Non-webtoon, or when `alwaysDecodeLongStripWithSSIV` is set, bypasses Coil and feeds the stream to SubsamplingScaleImageView.
* Replacement: `fn decode_for_viewport(src: PageSource, target_w: u32, crop_borders: bool) -> Result<gdk::Texture>` run on `spawn_blocking`; no caches (as in Aniyomi). Border-crop algorithm lives in `area:43-reader-core`. Sub-sampling for very tall images: tile the strip into `gdk::Texture` tiles of at most `GL_MAX_TEXTURE_SIZE` (query via `gdk::GLContext`) instead of a custom SSIV.
* Difficulty trivial for the type; the viewport-decode pipeline itself is in area:43-reader-core.

### 8.5 BitmapRegionDecoder
* Aniyomi: decodes sub-rectangles to split tall images at download time (`ImageUtil.kt:220-262`): `BitmapRegionDecoder.newInstance(stream)` (API >= S) or `newInstance(stream, false)`; for each `SplitData(index, topOffset, splitHeight, splitWidth)` it decodes `Rect(0, topOffset, splitWidth, bottomOffset)` and compresses JPEG 100; always `recycle()` in `finally`. 1 file / 5 occ.
* Replacement: decode once with `image` then `crop_imm(0, top, width, height)` per part, encode, drop the full image after the loop. For multi-hundred-MB strips use `libvips` (`libvips` crate, `extract_area`) with sequential access. Same split math (see 8.1).
* Difficulty moderate. Risk: peak RAM = full decoded image; split only when `height > 3 * width` check passes (Aniyomi's integer-division rule) and enforce 512 MB decode budget (Proposal).

### 8.6 AsyncImage
* Aniyomi: Coil 3 Compose image composable (5 files / 10 occ.): `ItemCover.kt:32` (library/browse cover with `MangaCover`/`AnimeCover` models), `MangaInfoHeader.kt:118` and `AnimeInfoHeader.kt:123` (background blur backdrop), `BrowseMangaIcons.kt:90` / `BrowseAnimeIcons.kt:90` (remote extension icon from `extension.iconUrl` with `placeholder = ColorPainter(Color(0x1F888888))` and `error = rememberResourceBitmapPainter(R.drawable.cover_error)`, clipped by `shapes.extraSmall`).
* Replacement: `CoverImage` widget wrapping `gtk::Picture` that shows a placeholder `#888888` at alpha `0x1F` (`rgba(136,136,136,0.12)`) until a `tokio` task loads: memory LRU (`lru`, capacity 100 textures) -> disk cache (`$XDG_CACHE_HOME/kurobayomi/covers`, key = SHA-256 of model key, size cap per area:34-images-covers-cache) -> network via `reqwest` with source headers. On failure show the bundled `cover_error` SVG. Decode with 8.1.
* Difficulty moderate. Risk: list virtualisation (`gtk::ListView`/`GridView` recycles rows): cancel the in-flight load when a row is rebound.

---

## 9. Audio and player platform glue

### 9.1 AudioManager
* Aniyomi (2 files / 16 occ.): `getStreamVolume(STREAM_MUSIC)` / `getStreamMaxVolume(STREAM_MUSIC)` (`PlayerViewModel.kt:292,765`), `setStreamVolume(STREAM_MUSIC, v, 0)` (`:782`), focus constants `AUDIOFOCUS_LOSS`, `AUDIOFOCUS_LOSS_TRANSIENT`, `AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK`, `AUDIOFOCUS_GAIN`, `AUDIOFOCUS_REQUEST_FAILED`, and `ACTION_AUDIO_BECOMING_NOISY` (`PlayerActivity.kt:150,602-632,983`).
* Volume algorithm to preserve (`PlayerViewModel.kt:765-790`): two layers: system volume `0..maxVolume` and mpv `volume` 100..(100+`volumeBoostCap`). `changeVolumeBy(change)`: if boost cap > 0 and system volume is at max: if mpv volume == 100 and `change < 0` -> lower the system volume by `change`; else `finalMPV = (mpvVolume + change).coerceAtLeast(100)`; if `finalMPV in 100..volumeBoostCap + 100` set mpv volume and return. Otherwise `changeVolumeTo(current + change)` which clamps to `0..maxVolume`. On resume after exit, if system volume < max, mpv volume is reset to 100 (`PlayerActivity.kt:644-646`).
* Becoming-noisy rule: on headphone unplug pause playback and clear `FLAG_KEEP_SCREEN_ON` (`PlayerActivity.kt:150-155`).
* Replacement: do **not** change the system mixer from an app. Use mpv's own `volume` (0..`volume-max`, default 100; set `volume-max = 100 + volumeBoostCap`) and `mute`; the OSD slider maps 0..100+cap to that property. Pause-on-unplug: optional listener on PipeWire/PulseAudio sink change (`libpulse-binding` subscribe on `Sink` removed events, or `zbus` to `org.freedesktop.ReserveDevice`), P2. Screen-keep-on -> `gtk::Application::inhibit(window, ApplicationInhibitFlags::IDLE, reason)` while playing and `uninhibit` on pause.
* Difficulty moderate. User loses: hardware-volume-key stepping coupling (keys already control the system sink on Linux), and the "boost above system max" two-layer behaviour becomes a single mpv scale.

### 9.2 AudioManagerCompat
* Aniyomi: `AudioManagerCompat.requestAudioFocus(audioManager, request)`, `abandonAudioFocusRequest(audioManager, it)` and `AUDIOFOCUS_GAIN` (`PlayerActivity.kt:303-306,588,595`). 1 file / 4 occ. Request failure (`AUDIOFOCUS_REQUEST_FAILED`) leaves `audioFocusRequest` null and playback continues.
* Replacement: dropped. Linux has no audio-focus arbitration. Optional (P3): on play, send `Pause` to other MPRIS players via `zbus` (`org.mpris.MediaPlayer2.*`) if the user enables "pause other players". User loses nothing by default.
* Difficulty trivial.

### 9.3 AudioAttributesCompat
* Aniyomi: `AudioAttributesCompat.Builder().setUsage(USAGE_MEDIA).setContentType(CONTENT_TYPE_MUSIC)` attached to the focus request (`PlayerActivity.kt:590-591`). 1 file / 4 occ.
* Replacement: mpv options `audio-client-name=Kurobayomi` and `audio-stream-silence=no`; PipeWire stream role via env `PULSE_PROP=media.role=video` set before mpv init (video role). Difficulty trivial.

### 9.4 AudioFocusRequestCompat
* Aniyomi: builder for the request `AudioFocusRequestCompat.Builder(AUDIOFOCUS_GAIN)` with `setOnAudioFocusChangeListener(audioFocusChangeListener)` (`PlayerActivity.kt:588-594`). Listener (OBSERVED, `:602-632`): LOSS/LOSS_TRANSIENT -> remember `wasPlayerPaused = player.paused`, `viewModel.pause()`, `restoreAudioFocus` = old restore + `unpause()` only if it was playing; CAN_DUCK -> `multiply volume 0.5`, restore `multiply volume 2`; GAIN -> run restore then clear. 1 file / 3 occ.
* Replacement: dropped with 9.2; the duck/pause state machine is not ported. Difficulty trivial.

---

## 10. App lock and biometrics

All four items below form one feature: "App lock" (`useAuthenticator` preference, `securityPreferences`) gated by `SecureActivityDelegate` (`app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt:77-125`): on every `onResume`, if `useAuthenticator` is true and authentication is supported and `requireUnlock` is true, start `UnlockActivity` with transition 0,0; if authentication is not supported, **auto-disable** `useAuthenticator` (`set(false)`). Unlock success calls `SecureActivityDelegate.unlock()` (sets `requireUnlock=false`); error calls `finishAffinity()` (quits the app). Also gates preference changes via `authenticate(title, subtitle = confirm_lock_change)`. Strings: `unlock_app_title` = "Unlock %s", `confirm_lock_change` = "Authenticate to confirm change".

Decision for Kurobayomi (all four items, Proposal): **dropped in P0/P1**, listed as P3 with this replacement: optional lock using the desktop's PAM stack through `polkit` (`pkcheck`/`org.freedesktop.PolicyKit1`) or simply the screen locker; the existing session login is already the trust boundary on a single-user Linux desktop. User loses: in-app biometric/PIN lock and "secure screen" blocking. The preference keys (`useAuthenticator`, `secureScreen` values `ALWAYS`/`INCOGNITO`/`DISABLED`) stay in the settings struct for backup round trip but are hidden in the UI.

### 10.1 BiometricManager
* Aniyomi: `BiometricManager.from(this).canAuthenticate(Authenticators.BIOMETRIC_WEAK or Authenticators.DEVICE_CREDENTIAL) == BIOMETRIC_SUCCESS` (`app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt:96-100`, `isAuthenticationSupported`). 1 file / 4 occ.
* Replacement: dropped. Difficulty trivial.

### 10.2 Authenticators
* Aniyomi: constants `BIOMETRIC_WEAK` (class 2) | `DEVICE_CREDENTIAL` (`AuthenticatorUtil.kt:97`); "Class 2 biometric or credential". 1 file / 3 occ. Replacement: dropped. Trivial.

### 10.3 BiometricPrompt
* Aniyomi: `startClass2BiometricOrCredentialAuthentication(title, subtitle, confirmationRequired = true, executor = mainExecutor, callback)` (`AuthenticatorUtil.kt:39-50`), `UnlockActivity` calls it with `confirmationRequired = false` (`app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt:18-28`); `AuthenticationResult` in callbacks; global flag `isAuthenticating` prevents double prompts on old APIs. 2 files / 7 occ. Replacement: dropped. Trivial.

### 10.4 AuthPromptCallback
* Aniyomi: abstract `AuthenticationCallback : AuthPromptCallback()` marks `isAuthenticating=false` in `onAuthenticationError(activity, errorCode, errString)` and `onAuthenticationSucceeded(activity, result)`; `authenticate()` toasts `errString` on error and returns false (`AuthenticatorUtil.kt:55-85,114-148`); when authentication is not supported it returns true (no gate). 1 file / 3 occ. Replacement: dropped. Moderate only if the optional polkit lock is later built.

### 10.5 AuthenticationError
* Aniyomi: annotation `@AuthenticationError errorCode: Int` on the callback parameter (`AuthenticatorUtil.kt:129`). 1 file / 2 occ. Replacement: dropped. Trivial.

---

## 11. Background work

### 11.1 BackoffPolicy
* Aniyomi (5 files / 10 occ.), exact WorkManager policies:

| Job | Policy | Constraints / period | Source |
|---|---|---|---|
| Manga/Anime library update (periodic) | `BackoffPolicy.LINEAR`, 10 min | period `interval` hours (flex 10 min), network `UNMETERED` if `DEVICE_NETWORK_NOT_METERED` ("network_not_metered") in restrictions else `CONNECTED`; Wi-Fi transport if `DEVICE_ONLY_ON_WIFI` ("wifi"); `requiresCharging` if `DEVICE_CHARGING` ("ac"); `requiresBatteryNotLow(true)`; unique work name `WORK_NAME_AUTO`, policy `ExistingPeriodicWorkPolicy.UPDATE`; `interval == 0` cancels | `app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt:440-486`, `.../anime/AnimeLibraryUpdateJob.kt:455-500` |
| Auto backup | `EXPONENTIAL`, 10 min | period `backupInterval` hours (flex 10 min), `requiresBatteryNotLow`, tag `TAG_AUTO`, input `IS_AUTO_BACKUP_KEY=true`, policy UPDATE | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt:90-111` |
| Delayed tracking update (manga, anime) | `EXPONENTIAL`, 5 min | one-time, network `CONNECTED`, unique `ExistingWorkPolicy.REPLACE`; returns `Result.retry()` while the delayed-tracking store is non-empty | `app/src/main/java/eu/kanade/domain/track/manga/service/DelayedMangaTrackingUpdateJob.kt:56-70`, `.../anime/DelayedAnimeTrackingUpdateJob.kt:64-77` |

* WorkManager semantics to reproduce: LINEAR retry delay = `n * backoff` (10, 20, 30... min); EXPONENTIAL = `backoff * 2^(n-1)` (5, 10, 20... min); both capped by WorkManager at 5 hours (`MAX_BACKOFF_MILLIS = 18_000_000`). Manual runs use `ExistingWorkPolicy.KEEP` (library/metadata/restore) or `REPLACE` (downloads, tracking, app update download): `startNow` returns false if `wm.isRunning(TAG)`.
* Replacement: in-process `Scheduler` on tokio: `struct JobSpec { name, period: Option<Duration>, flex: Duration, backoff: Backoff{Linear|Exponential, base}, constraints }`, a persisted table `job_state(name, next_run_ms, attempt)` in the app DB so periodic timing survives restarts; constraint checks: network via `gio::NetworkMonitor` (`is_network_metered()`, `connectivity()`), charging/battery via UPower over `zbus` (`org.freedesktop.UPower` `OnBattery`, `Percentage`; "battery not low" = not (on battery and below 15 %, Proposal)). While the GUI is closed nothing runs unless the user enables the optional `systemd --user` timer that launches `kurobayomi --headless-update` (P2).
* Preserve: the policies/periods in the table; unique-name/replace/keep semantics; "Wi-Fi only" -> `NetworkMonitor` connection type via NetworkManager (`zbus`, device type `wifi`); the constraint-key strings for settings and backup.
* Difficulty moderate. User loses: scheduled updates while the app is not running (unless the systemd timer is installed).

---

## 12. Logging

### 12.1 AndroidLogcatLogger
* Aniyomi: `LogcatLogger.install(AndroidLogcatLogger(LogPriority.VERBOSE))` only when no logger is installed and the preference `networkPreferences.verboseLogging()` is true (`App.kt:161`); otherwise the library default (no-op in release). 1 file / 2 occ.
* Replacement: `tracing` + `tracing_subscriber::fmt` to stderr with `EnvFilter`; default level `warn`, `debug` when setting "verbose logging" is on or env `KUROBAYOMI_LOG` is set; optional file appender `tracing-appender` at `$XDG_STATE_HOME/kurobayomi/kurobayomi.log` (daily, keep 7). Journald via `tracing-journald` when running under systemd. `LogPriority` mapping: VERBOSE->TRACE, DEBUG->DEBUG, INFO->INFO, WARN->WARN, ERROR->ERROR.
* Difficulty trivial.

---

## 13. Tests and benchmarks

### 13.1 AfterAll
* Aniyomi: `@AfterAll @JvmStatic fun tearDown()` resets the coroutine Main dispatcher and closes the surrogate (`app/src/test/java/mihon/core/migration/MigratorTest.kt:151`). 1 file / 2 occ. Replacement: not needed; tokio tests use `#[tokio::test]` and each test owns its runtime. Trivial.

### 13.2 BeforeAll
* Aniyomi: `@BeforeAll` sets `Dispatchers.setMain(mainThreadSurrogate)` (`MigratorTest.kt` companion). 1 file / 2 occ. Replacement: none. Trivial.

### 13.3 BeforeEach
* Aniyomi: per-test fixtures: `MigratorTest.kt` builds `MigrationContext(false)`, `MigrationJobFactory`, `MigrationCompletedListener`, `MigrationStrategyFactory`; `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt:23` sets up mocks. 2 files / 4 occ.
* Replacement: construct fixtures inside each `#[test]` via a helper fn; for the migrator port the same cases as tests: `initialVersion` (0 -> 1 runs ALWAYS-migrations only: 1 migration, completion listener called), `sameVersion` (1 -> 1: no-op strategy, returns false, no job created), `noMigrations` (1 -> 2 with empty list: false, no job), `smallMigration` (1 -> 2 with `[ALWAYS, 2f]`: both run, completion called). These four are the acceptance tests for the Rust migrator.
* Difficulty trivial.

### 13.4 AndroidJUnit4ClassRunner
* Aniyomi: `@RunWith(AndroidJUnit4ClassRunner::class)` on `ColdStartupBenchmark`, `WarmStartupBenchmark`, `HotStartupBenchmark` (`macrobenchmark/src/main/java/tachiyomi/macrobenchmark/StartupBenchmark.kt:33,40,47`). 1 file / 4 occ. Replacement: dropped (Android instrumentation). Startup time measured by a `criterion` or a simple `Instant` log line "startup complete in N ms" (P3). Trivial.

### 13.5 BaselineProfileMode
* Aniyomi: `CompilationMode.Partial(baselineProfileMode = BaselineProfileMode.Disable)` and `.Require` in the startup benchmark (`StartupBenchmark.kt:63-71`) beside `CompilationMode.None()` and `.Full()`. 1 file / 3 occ. Replacement: dropped; Rust is AOT-compiled; use `opt-level = 3`, `lto = "thin"`, `codegen-units = 1` in the release profile. Trivial.

### 13.6 BaselineProfileRule
* Aniyomi: `BaselineProfileGenerator` records a profile for package `xyz.jmir.tachiyomi.mi.benchmark` by clicking texts "Manga", "Updates", "History", "More", "Settings" (`macrobenchmark/src/main/java/tachiyomi/macrobenchmark/BaselineProfileGenerator.kt:14-36`). 1 file / 2 occ. Replacement: dropped. Optionally PGO via `cargo-pgo` (P3). Trivial.

---

## 14. Home-screen widget (Glance)

The Glance widgets are Android launcher widgets showing recent updates. Decision: **dropped**; replaced inside the app by the "Updates" tab, with an optional (P3) KDE Plasma applet or a `.desktop` action "Open updates" (`Actions=Updates`). User loses: at-a-glance cover grid on the home screen.

Facts for the record (so the Updates tab can reuse the data rule): widget data = updates since `DateLimit = ZonedDateTime.now().minusMonths(3)`, `distinctBy mangaId`/`animeId`, `take(rowCount * columnCount)` where `rowCount = (height_dp / 95).toInt().coerceIn(1, 10)` and `columnCount = (width_dp / 64).toInt().coerceIn(1, 10)` after subtracting top/bottom padding (`presentation-widget/src/main/java/tachiyomi/presentation/widget/util/GlanceUtils.kt:29-39`, `.../entries/manga/BaseMangaUpdatesGridGlanceWidget.kt:158-161`); cover size 58 x 87 dp (`UpdatesMangaCover.kt:16-17`); locked state text `appwidget_unavailable_locked` = "Widget not available when app lock is enabled".

### 14.1 Alignment
* Aniyomi: `androidx.glance.layout.Alignment` in the widget components, e.g. `contentAlignment = Alignment.Center`, `Alignment.CenterVertically`, `Alignment.CenterHorizontal` (`presentation-widget/src/main/java/tachiyomi/presentation/widget/components/manga/UpdatesMangaWidget.kt:39,53-54,65-66`, `LockedMangaWidget.kt:34`, anime twins). 4 files / 18 occ. (Note: the `rg` list for the simple name `Alignment` also matches Compose files in `app/`; those import `androidx.compose.ui.Alignment` and are out of scope.)
* Replacement: dropped with the widget. In GTK, alignment is `widget.set_halign/valign(gtk::Align::Center)`. Trivial.

### 14.2 Box
* Aniyomi: `androidx.glance.layout.Box` used as the container for widget cells and the locked message: `LockedMangaWidget.kt:30` (clickable box, padding 8 dp, opens `Constants.MAIN_ACTIVITY` with `FLAG_ACTIVITY_NEW_TASK`), `UpdatesMangaWidget.kt:38,69`, `UpdatesMangaCover.kt:24`, and anime twins. 6 files / 14 occ.
* Replacement: dropped with the widget; in GTK a `gtk::Box` / `gtk::Overlay`. Trivial.

---

## 15. Construction-time attributes

### 15.1 AttributeSet
* Aniyomi: constructor parameter on custom Android Views (11 files / 22 occ.): `AniyomiMPVView(context, attributes)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:41`), `ReaderNavigationOverlayView` (`ReaderNavigationOverlayView.kt:19`), `ReaderButton`, `ReaderTransitionView`, `ReaderProgressIndicator`, `ReaderPageImageView` (with `defStyleAttrs`, `defStyleRes`), `WebtoonRecyclerView`, `WebtoonSubsamplingImageView`, `TachiyomiTextInputEditText`, `MinMaxNumberPicker`, `RevealAnimationView`. XML inflation is the only reason for it.
* Replacement: none; GTK widgets are constructed in code or via `gtk::Builder`/`.ui` templates (`#[derive(CompositeTemplate)]`). Style attributes become CSS classes.
* Difficulty trivial. Note: `ReaderNavigationOverlayView` draws tap-zone overlays; reproduced with a `gtk::DrawingArea` in area:43-reader-core.

---

## 16. Verification

Verification command (prints nothing when every item is present):

```
for k in $(python3 /home/blackbox/kurobayomi/plans/_tools/inventory.py items imports 1 8 | cut -f1); do grep -qF -- "$k" /home/blackbox/kurobayomi/plans/catalogs/android-api-mapping-01.md || echo MISSING $k; done
```
