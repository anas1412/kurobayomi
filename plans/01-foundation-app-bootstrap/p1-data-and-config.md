# Area 01 — Data models, preference keys, files, network, DB touched

All Kotlin paths are repo-relative to `/home/blackbox/aniyomi-src`. "Proposal" marks Kurobayomi decisions.

## 1. Build identity constants (OBSERVED `app/build.gradle.kts`)

| Item | Value | Line |
|---|---|---|
| Gradle namespace | `eu.kanade.tachiyomi` | 17 |
| `applicationId` | `xyz.jmir.tachiyomi.mi` | 20 |
| `versionCode` | `132` | 22 |
| `versionName` | `"0.18.2.1"` | 23 |
| BuildConfig `COMMIT_COUNT` (String) | output of `git rev-list --count HEAD` (`buildSrc/src/main/kotlin/mihon/buildlogic/Commands.kt:11`) | 25 |
| BuildConfig `COMMIT_SHA` (String) | output of `git rev-parse --short HEAD` | 26 |
| BuildConfig `BUILD_TIME` (String) | `getBuildTime(useLastCommitTime = false)`: current UTC time formatted `yyyy-MM-dd'T'HH:mm:ss'Z'`; release build type overrides with `useLastCommitTime = true` (UTC time of `git log -1 --format=%ct`); preview build type uses `false` | 27, 57, 72 |
| BuildConfig `UPDATER_ENABLED` (boolean) | `Config.enableUpdater` = Gradle property `enable-updater` present | 28 |
| Config `enableCodeShrink` | true unless property `disable-code-shrink` present | 52-53 |
| Config `includeDependencyInfo` | true only if property `include-dependency-info` present | 148-149 |
| debug | `applicationIdSuffix = ".dev"`, `versionNameSuffix = "-<commit count>"`, `isPseudoLocalesEnabled = true` | 46-50 |
| release | `isMinifyEnabled = isShrinkResources = Config.enableCodeShrink`, `proguardFiles("proguard-android-optimize.txt", "proguard-rules.pro")` | 51-58 |
| preview | `initWith(release)`, `applicationIdSuffix = ".debug"`, `versionNameSuffix` = debug's, `signingConfig` = debug's, `matchingFallbacks += release` | 62-73 |
| benchmark | `initWith(release)`, `isDebuggable = false`, `isProfileable = true`, `versionNameSuffix = "-benchmark"`, `applicationIdSuffix = ".benchmark"`, debug signing, `matchingFallbacks += release`; the variant is enabled only when product flavors contain `default` = `dev` (lines 320-328) | 74-85 |
| ABI splits | enabled; universal APK on; includes `armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64` | 93-100 |
| `keepDebugSymbols` | `libandroidx.graphics.path`, `libarchive-jni`, `libavcodec`, `libavdevice`, `libavfilter`, `libavformat`, `libavutil`, `libconscrypt_jni`, `libc++_shared`, `libffmpegkit_abidetect`, `libffmpegkit`, `libimagedecoder`, `liblibrary`, `libmpv`, `libplayer`, `libpostproc`, `libquickjs`, `libsqlite3x`, `libswresample`, `libswscale`, `libtorrserver`, `libxml2` (each as `**/<name>.so`) | 104-128 |
| Packaging resource excludes | `kotlin-tooling-metadata.json`, `LICENSE.txt`, `META-INF/**/*.properties`, `META-INF/**/LICENSE.txt`, `META-INF/*.properties`, `META-INF/*.version`, `META-INF/**/*.MF`, `META-INF/DEPENDENCIES`, `META-INF/LICENSE`, `META-INF/NOTICE`, `META-INF/README.md` | 131-143 |
| buildFeatures | `viewBinding = true`, `buildConfig = true`, `aidl = false`, `renderScript = false`, `shaders = false` | 152-160 |
| lint | `abortOnError = false`, `checkReleaseBuilds = false` | 162-165 |
| Kotlin opt-ins | `androidx.compose.animation.ExperimentalAnimationApi`, `androidx.compose.animation.graphics.ExperimentalAnimationGraphicsApi`, `androidx.compose.foundation.ExperimentalFoundationApi`, `androidx.compose.foundation.layout.ExperimentalLayoutApi`, `androidx.compose.material3.ExperimentalMaterial3Api`, `androidx.compose.ui.ExperimentalComposeUiApi`, `coil3.annotation.ExperimentalCoilApi`, `kotlinx.coroutines.ExperimentalCoroutinesApi`, `kotlinx.coroutines.FlowPreview`, `kotlinx.coroutines.InternalCoroutinesApi`, `kotlinx.serialization.ExperimentalSerializationApi` | 170-182 |
| Static shortcuts file | `shortcutHelper.setFilePath("./shortcuts.xml")` (plugin `com.github.zellius.shortcut-helper`) | 14 |
| Test runner | `androidx.test.runner.AndroidJUnitRunner` | 42 |
| ACRA (crash upload) | fully commented out (lines 30-40): `acra.properties` fields `ACRA_URI`, `ACRA_LOGIN`, `ACRA_PASSWORD` would become BuildConfig fields. Not active: Aniyomi has no automatic crash upload | 30-40 |

Proposal (Rust build identity): `kuro_core::app_info::BUILD` constants filled by `build.rs`:
`APP_ID = "io.github.blackbox.Kurobayomi"` (already the application id in `src/main.rs`), `VERSION_NAME = env!("CARGO_PKG_VERSION")` plus `-<commit count>` in debug builds, `VERSION_CODE: u32 = 1` (integer ladder for the migrator; independent of the semver), `COMMIT_COUNT`, `COMMIT_SHA` (7 chars), `BUILD_TIME` (same `%Y-%m-%dT%H:%M:%SZ` pattern; release uses last-commit time, others current time), `BUILD_TYPE` in {`debug`, `preview`, `release`} derived from `PROFILE` env of the build script and the cargo profile name, `UPDATER_ENABLED = cfg!(feature = "updater")`. When `git` is unavailable (source tarball) `COMMIT_COUNT = "0"`, `COMMIT_SHA = "unknown"`.

## 2. Gradle dependency declarations (OBSERVED `app/build.gradle.kts:186-318`)

Fully enumerated with Rust replacements in `p1-topic-build-and-dependency-map.md` section 2.

## 3. Preference store semantics needed by this area (OBSERVED `core/common/src/main/java/tachiyomi/core/common/preference/`)

* Types: String (default `""`), Long (0), Int (0), Float (0f), Boolean (false), `Set<String>` (empty), Object (serializer/deserializer to String), Enum (stored as `Enum.name`; unknown name returns the default).
* Key namespaces by prefix (`Preference.kt:28-48`): app-state prefix `__APP_STATE_` (`appStateKey(key)`, `isAppState(key)`), private prefix `__PRIVATE_` (`privateKey`, `isPrivate`). App-state keys are excluded from backups; private keys are excluded unless the user opts in.
* Read semantics: `get()` catches `ClassCastException`, logs `"Invalid value for $key; deleting"`, deletes the key, returns the default; `isSet()` = key present; `delete()` removes the key; `changes()` = emits the current value immediately and again after each change of that key (or of "all keys", the `null` key); the flow is conflated; `stateIn(scope)` = `changes().stateIn(scope, Eagerly, get())`.
* Helper operations: `getAndSet`, `deleteAndGet`, `plusAssign`/`minusAssign` for sets, `toggle()` for booleans (writes then re-reads and returns the stored value).
* Backing file on Android: default SharedPreferences (`PreferenceManager.getDefaultSharedPreferences`), one flat namespace. Proposal (area:02-preferences-system owns it): `$XDG_CONFIG_HOME/kurobayomi/settings.toml` for user preferences and `$XDG_STATE_HOME/kurobayomi/state.toml` for `__APP_STATE_` keys, `Preference` keys kept as the Aniyomi key strings in a `legacy_key` table for backup import/export.

## 4. Preference holders registered by `PreferenceModule` (OBSERVED)

| Holder | Defined in | Owner area for the key list |
|---|---|---|
| `NetworkPreferences` | `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt` | this table (section 5.3) and area:12-network-stack |
| `SourcePreferences` | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt` | area:10-app-domain-layer |
| `SecurityPreferences` | `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt` | this table (section 5.2) |
| `LibraryPreferences` | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt` | area:08-domain-library-category-history-updates |
| `ReaderPreferences` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt` | area:43-reader-core |
| `PlayerPreferences`, `GesturePreferences`, `DecoderPreferences`, `SubtitlePreferences`, `AudioPreferences`, `AdvancedPlayerPreferences` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/` | area:45-player-core and area:48-settings-player-and-scripts |
| `TorrentPreferences` | `aniyomi.core.common.torrent` (core/common) | area:13-core-util-storage-archive-torrent |
| `TrackPreferences` | `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt` | area:27-tracking-core |
| `DownloadPreferences` | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt` | area:25-downloads-shared |
| `BackupPreferences` | `domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt` | area:21-backup-format-and-create |
| `StoragePreferences` | `domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt` | area:13-core-util-storage-archive-torrent |
| `UiPreferences` | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt` | area:10-app-domain-layer, area:49-theme-and-design-system |
| `BasePreferences` | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt` | this table (section 5.1) |

## 5. Preference keys used by this area

### 5.1 BasePreferences (read by `App.kt`, `PreferenceModule.kt`)

| Accessor | Key | Type | Default | Stored as | Notes |
|---|---|---|---|---|---|
| `downloadedOnly()` | `__APP_STATE_pref_downloaded_only` | Boolean | `false` | boolean | Downloaded-only banner (MainActivity); not in backups |
| `incognitoMode()` | `__APP_STATE_incognito_mode` | Boolean | `false` | boolean | Drives the incognito notification; not in backups |
| `extensionInstaller()` | `extension_installer` | Enum `ExtensionInstaller` | `PACKAGEINSTALLER` (`LEGACY` on MIUI) | enum name | Values `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE`; on read the wrapper corrects an invalid value: PACKAGEINSTALLER on MIUI becomes LEGACY, SHIZUKU without Shizuku installed becomes the default. Android only: Drop, replaced by the extension strategy of area:20-extension-strategy-redesign |
| `deviceHasPip()` | not a preference; `PackageManager.hasSystemFeature(FEATURE_PICTURE_IN_PICTURE)` | Boolean | n/a | n/a | Drop (T-01a-046): GTK has no Picture-in-Picture; the mini player window is area:46-player-controls-ui |
| `shownOnboardingFlow()` | `__APP_STATE_onboarding_complete` | Boolean | `false` | boolean | First-run onboarding (area:36) |
| `displayProfile()` | `pref_display_profile_key` | String | `""` | string | Path of a colour profile for the reader (area:47-settings-screens-general) |
| `hardwareBitmapThreshold()` | `pref_hardware_bitmap_threshold` | Int | `GLUtil.SAFE_TEXTURE_LIMIT` = 2048 (set to the probed device limit at first launch, `App.kt:141-143`) | int | Section 5 of `p1-behavior.md` |
| `alwaysDecodeLongStripWithSSIV()` | `pref_always_decode_long_strip_with_ssiv` | Boolean | `false` | boolean | Reader long-strip decoding (area:43) |

### 5.2 SecurityPreferences (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt`)

| Accessor | Key | Type | Default | Notes |
|---|---|---|---|---|
| `useAuthenticator()` | `use_biometric_lock` | Boolean | `false` | Master switch of the app lock |
| `lockAppAfter()` | `lock_app_after` | Int (minutes) | `0` | `-1` never lock, `0` lock immediately when the app returns to foreground, positive N lock after N minutes in background |
| `secureScreen()` | `secure_screen_v2` | Enum `SecureScreenMode` | `INCOGNITO` | stored as enum name |
| `hideNotificationContent()` | `hide_notification_content` | Boolean | `false` | Hides titles/text in library-update and download notifications |
| `lastAppClosed()` | `__APP_STATE_last_app_closed` | Long (epoch ms) | `0` | Written only when a timed lock is pending; deleted on foreground |

`SecureScreenMode(titleRes)`: `ALWAYS` (`MR.strings.lock_always`), `INCOGNITO` (`MR.strings.pref_incognito_mode`), `NEVER` (`MR.strings.lock_never`). The enum key `secure_screen_v2` replaced the old boolean key `secure_screen`: `MigrateSecureScreenMigration` (version 75, area:11-app-migrations) sets the mode to `ALWAYS` when the old boolean was true (file `app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt`).

### 5.3 NetworkPreferences (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt`)

| Accessor | Key | Type | Default |
|---|---|---|---|
| `verboseLogging()` | `verbose_logging` | Boolean | constructor argument `verboseLogging`, supplied as `isDebugBuildType` (true only when `BuildConfig.BUILD_TYPE == "debug"`); class default `false` |
| `dohProvider()` | `doh_provider` | Int | `-1` (DNS-over-HTTPS off; provider ids in area:12-network-stack) |
| `defaultUserAgent()` | `default_user_agent` | String | `"Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"` |

### 5.4 UiPreferences entry read by `App.kt`
`themeMode()`: key `pref_theme_mode_key`, enum `ThemeMode` {`LIGHT`, `DARK`, `SYSTEM`}, default `SYSTEM`, stored as the enum name.

### 5.5 App-state key written by the migrator
`last_version_code` through `Preference.appStateKey` → stored key `__APP_STATE_last_version_code`, Int, default `0` (`App.kt:169`).

### 5.6 Proposal: Rust settings structs for the keys above

```toml
# settings.toml  (user preferences)
[base]
display_profile = ""                 # pref_display_profile_key
hardware_bitmap_threshold = 2048     # pref_hardware_bitmap_threshold, set on first launch to device limit
always_decode_long_strip_with_ssiv = false   # pref_always_decode_long_strip_with_ssiv
[security]
use_authenticator = false            # use_biometric_lock
lock_app_after = 0                   # lock_app_after (minutes; -1 never, 0 always)
secure_screen = "INCOGNITO"          # secure_screen_v2 (stored but ignored on Linux)
hide_notification_content = false    # hide_notification_content
[network]
verbose_logging = false              # verbose_logging; default true in debug builds
doh_provider = -1                    # doh_provider
default_user_agent = "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0"
[ui]
theme_mode = "SYSTEM"                # pref_theme_mode_key
```
```toml
# state.toml  (app state, never exported)
downloaded_only = false              # __APP_STATE_pref_downloaded_only
incognito_mode = false               # __APP_STATE_incognito_mode
onboarding_complete = false          # __APP_STATE_onboarding_complete
last_app_closed = 0                  # __APP_STATE_last_app_closed
last_version_code = 0                # __APP_STATE_last_version_code
```
Unknown keys must survive load/save (round trip through `toml::Table`) so that a newer settings file is not damaged by an older binary.

## 6. Enums and their stored values

| Enum | Constants in declaration order | Stored as | Where |
|---|---|---|---|
| `ThemeMode` | `LIGHT`, `DARK`, `SYSTEM` | enum name string | `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt` |
| `SecureScreenMode` | `ALWAYS`, `INCOGNITO`, `NEVER` | enum name string | `SecurityPreferences.kt:30-34` |
| `ExtensionInstaller` | `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE` (with `requiresSystemPermission` true, true, false, false) | enum name string | `BasePreferences.kt` |
| `TriState` (preference) | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` | enum name string (filters in library, updates) | `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt` |
| `CheckboxState.TriState<T>` | `Include(value)`, `Exclude(value)`, `None(value)` | not stored (UI model) | `.../preference/CheckboxState.kt` |
| `CheckboxState.State<T>` | `Checked(value)`, `None(value)` | not stored | same |
| `UpdateStrategy` (manga) | `ALWAYS_UPDATE` (0), `ONLY_FETCH_ONCE` (1) | `update_strategy` column Long = ordinal; decode of an out-of-range value gives `ALWAYS_UPDATE` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt`, `MangaUpdateStrategyColumnAdapter` |
| `AnimeUpdateStrategy` | `ALWAYS_UPDATE` (0), `ONLY_FETCH_ONCE` (1) | same, decode default `ALWAYS_UPDATE` | `.../animesource/model/AnimeUpdateStrategy.kt` |
| `FetchType` | `Seasons` (0), `Episodes` (1) | `fetch_type` column Long = ordinal; decode default `Episodes` | `.../animesource/model/FetchType.kt`, `FetchTypeColumnAdapter` |
| `ImageUtil.ImageType` (`mime`, `extension`) | `AVIF` (`image/avif`, `avif`), `GIF` (`image/gif`, `gif`), `HEIF` (`image/heif`, `heif`), `JPEG` (`image/jpeg`, `jpg`), `JXL` (`image/jxl`, `jxl`), `PNG` (`image/png`, `png`), `WEBP` (`image/webp`, `webp`) | not stored; the `mime` list is what `AppInfo.getSupportedImageMimeTypes()` returns | `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:106-114` |

## 7. `Constants` values (`core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt`, package `eu.kanade.tachiyomi.core.common`)

| Name | Value | Use |
|---|---|---|
| `URL_HELP` | `https://aniyomi.org/docs/guides/troubleshooting/` | Help row (`MoreScreen.kt:172`), `onHelpClick` of browse/search/migration screens |
| `URL_HELP_UPCOMING` | `https://aniyomi.org/docs/faq/updates/upcoming` | Upcoming screens' help button |
| `MANGA_EXTRA` | `manga` | Long extra: manga id |
| `ANIME_EXTRA` | `anime` | Long extra: anime id |
| `MAIN_ACTIVITY` | `eu.kanade.tachiyomi.ui.main.MainActivity` | widgets resolve the activity by `Class.forName` |
| `SHORTCUT_ANIMELIB` | `eu.kanade.tachiyomi.SHOW_ANIMELIB` | anime library tab |
| `SHORTCUT_LIBRARY` | `eu.kanade.tachiyomi.SHOW_LIBRARY` | manga library tab |
| `SHORTCUT_ANIME` | `eu.kanade.tachiyomi.SHOW_ANIME` | open anime (needs `anime` extra) |
| `SHORTCUT_MANGA` | `eu.kanade.tachiyomi.SHOW_MANGA` | open manga (needs `manga` extra) |
| `SHORTCUT_UPDATES` | `eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED` | updates tab |
| `SHORTCUT_HISTORY` | `eu.kanade.tachiyomi.SHOW_RECENTLY_READ` | history tab |
| `SHORTCUT_SOURCES` | `eu.kanade.tachiyomi.SHOW_CATALOGUES` | browse tab, sources |
| `SHORTCUT_ANIMEEXTENSIONS` | `eu.kanade.tachiyomi.ANIMEEXTENSIONS` | browse tab, anime extensions |
| `SHORTCUT_EXTENSIONS` | `eu.kanade.tachiyomi.EXTENSIONS` | browse tab, manga extensions |
| `SHORTCUT_ANIME_DOWNLOADS` | `eu.kanade.tachiyomi.SHOW_ANIME_DOWNLOADS` | More tab, downloads |
| `SHORTCUT_DOWNLOADS` | `eu.kanade.tachiyomi.SHOW_DOWNLOADS` | More tab, downloads |
| Private broadcast action in `App.kt:274` | `tachi.action.DISABLE_INCOGNITO_MODE` | incognito notification tap |
| Parcel key in `GEH` | `Throwable` | crash intent extra |

Proposal mapping (T-01a-031, T-01a-042): `enum DeepLink { AnimeLibrary, Library, Anime(i64), Manga(i64), Updates, History, Sources, AnimeExtensions, Extensions, AnimeDownloads, Downloads }` with the CLI flag `--open <name>[=<id>]` where `<name>` is one of `anime-library`, `library`, `anime`, `manga`, `updates`, `history`, `sources`, `anime-extensions`, `extensions`, `anime-downloads`, `downloads`; the same names are the application's `gio::SimpleAction` names with prefix `app.open-` (so `.desktop` file actions, notification default actions and `kurobayomi --open updates` all go through one `DeepLink::dispatch`).

## 8. Files, folders, formats

### 8.1 Android (OBSERVED)
| File | Where | Content |
|---|---|---|
| `tachiyomi.db` | app databases dir (name passed to `AndroidSqliteDriver`, `AppModule.kt:81`) | manga database, SQLDelight schema `tachiyomi.data.Database` |
| `tachiyomi.animedb` | app databases dir (`AppModule.kt:106`) | anime database, schema `tachiyomi.mi.data.AnimeDatabase` |
| `*-wal`, `*-shm` | next to each DB | SQLite WAL side files created by `PRAGMA journal_mode = WAL` |
| default SharedPreferences xml | app `shared_prefs` | all preferences (section 3) |
| `aniyomi_crash_logs.txt` | `externalCacheDir` | crash dump built by `CrashLogUtil` (deleted and recreated each time) |
| Intent extra `Throwable` | in-memory intent | JSON string of the full stack-trace text |

### 8.2 Linux proposal (all paths from the `Paths` trait of `kuro-platform`; no literal paths in other crates)
| Logical path | Linux location | Content |
|---|---|---|
| `paths.config()` | `$XDG_CONFIG_HOME/kurobayomi/` (default `~/.config/kurobayomi/`) | `settings.toml` |
| `paths.state()` | `$XDG_STATE_HOME/kurobayomi/` (default `~/.local/state/kurobayomi/`) | `state.toml`, `logs/kurobayomi.log[.YYYY-MM-DD]` (7 kept), `crashes/crash-<unix_ms>.txt` |
| `paths.data()` | `$XDG_DATA_HOME/kurobayomi/` (default `~/.local/share/kurobayomi/`) | `manga.db`, `anime.db` (+ `-wal`, `-shm`), covers, downloads default root decided by area:13 |
| `paths.cache()` | `$XDG_CACHE_HOME/kurobayomi/` | image caches, `crash/kurobayomi_crash_logs.txt` (the "share" dump) |
Crash files older than 14 days or beyond the newest 20 are deleted at normal startup.

### 8.3 Crash report file format (Proposal)
UTF-8 text file `crash-<unix_ms>.txt` in `paths.state()/crashes/`, written atomically (temp file, `fsync`, rename). Magic first line `kurobayomi-crash-report 1`; header lines `time:`, `thread:`, `location:`, `message:`; then sections `[facts]` (one `key: value` per line: `app_id`, `app_version`, `os`, `kernel`, `desktop`, `session`, `gtk`, `webview`, `mpv`, `ffmpeg`, `sqlite`, `locale`), `[manga_extensions]`, `[anime_extensions]` and `[backtrace]`. The complete layout with an example and the reader rules is in `p1-topic-crash-handling.md` section 4. The reporter shows the four header lines followed by the `[backtrace]` body as the equivalent of Android's `exception.toString()` (facts and extension sections go only into the shared dump).

## 9. Notification channels registered at startup (OBSERVED `Notifications.kt`, owner area:35-data-misc-updater-notification-export)

Importance: `IMPORTANCE_LOW` (no sound, shows in shade), `IMPORTANCE_DEFAULT`, `IMPORTANCE_HIGH` (heads-up).

| Channel id | Group | Importance | Name resource | Extra | Notification ids on the channel |
|---|---|---|---|---|---|
| `common_channel` | none | LOW | `MR.strings.channel_common` "Common" | none | `ID_DOWNLOAD_IMAGE = 2` |
| `library_progress_channel` | `group_library` | LOW | `MR.strings.channel_progress` "Progress" | no badge | `ID_LIBRARY_PROGRESS = -101`, `ID_LIBRARY_SIZE_WARNING = -103` |
| `library_errors_channel` | `group_library` | LOW | `MR.strings.channel_errors` "Errors" | no badge | `ID_LIBRARY_ERROR = -102` |
| `new_chapters_episodes_channel` | none | DEFAULT | `AYMR.strings.channel_new_chapters_episodes` "Chapter/Episode updates" | none | `ID_NEW_CHAPTERS = -301`, `ID_NEW_EPISODES = -1301`; group keys `GROUP_NEW_CHAPTERS = "eu.kanade.tachiyomi.NEW_CHAPTERS"`, `GROUP_NEW_EPISODES = "eu.kanade.tachiyomi.NEW_EPISODES"` |
| `downloader_progress_channel` | `group_downloader` | LOW | `channel_progress` | no badge | `ID_DOWNLOAD_CHAPTER_PROGRESS = -201`, `ID_DOWNLOAD_EPISODE_PROGRESS = -203` |
| `downloader_error_channel` | `group_downloader` | LOW | `channel_errors` | no badge | `ID_DOWNLOAD_CHAPTER_ERROR = -202`, `ID_DOWNLOAD_EPISODE_ERROR = -204` |
| `backup_restore_progress_channel` | `group_backup_restore` | LOW | `channel_progress` | no badge | `ID_BACKUP_PROGRESS = -501`, `ID_RESTORE_PROGRESS = -503` |
| `backup_restore_complete_channel_v2` | `group_backup_restore` | HIGH | `MR.strings.channel_complete` "Complete" | no badge, sound `null` | `ID_BACKUP_COMPLETE = -502`, `ID_RESTORE_COMPLETE = -504` |
| `incognito_mode_channel` | none | LOW | `MR.strings.pref_incognito_mode` "Incognito mode" | none | `ID_INCOGNITO_MODE = -701` |
| `torrent_server_channel` | none | LOW | `AYMR.strings.pref_category_torrentserver` "Torrent Server Preferences" | no badge | `ID_TORRENT_SERVER = -801` |
| `http_server_channel` | none | LOW | `AYMR.strings.pref_http_server_name` "Http server" | no badge | `ID_HTTP_SERVER = -901` |
| `app_apk_update_channel` | `group_apk_updates` | DEFAULT | `MR.strings.channel_app_updates` "App updates" | none | `ID_APP_UPDATER = 1`, `ID_APP_UPDATE_PROMPT = 2`, `ID_APP_UPDATE_ERROR = 3` |
| `ext_apk_update_channel` | `group_apk_updates` | DEFAULT | `MR.strings.channel_ext_updates` "Extension updates" | none | `ID_UPDATES_TO_EXTS = -401`, `ID_EXTENSION_INSTALLER = -402` |

Groups and their names: `group_backup_restore` = `MR.strings.label_backup` "Backup and restore"; `group_downloader` = `MR.strings.download_notifier_downloader_title` "Downloader"; `group_library` = `MR.strings.label_library` "Library"; `group_apk_updates` = `MR.strings.label_recent_updates` "Updates". Deprecated channel ids deleted at every start: `downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel`. Observed id collisions: `ID_DOWNLOAD_IMAGE` and `ID_APP_UPDATE_PROMPT` are both `2` (different channels, same notification id; on Android the id space is shared, so the later notification replaces the earlier). The Rust string key (`"{channel}:{id}"`) removes the collision.

## 10. Databases touched (OBSERVED `AppModule.kt:78-157`)

* Two SQLite databases, both opened through the same pattern: manga `tachiyomi.db` (`Database`), anime `tachiyomi.animedb` (`AnimeDatabase`).
* Connection PRAGMAs executed in `onOpen` after the helper's own `onOpen`: `foreign_keys = ON`, `journal_mode = WAL`, `synchronous = NORMAL`. Schema create/upgrade is `Database.Schema` / `AnimeDatabase.Schema` (SQLDelight; area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations).
* Column adapters (SQL type ↔ Kotlin type) registered here:

| Table.column | Adapter | SQL storage | Kotlin type | Encode / decode |
|---|---|---|---|---|
| `history.last_read` (manga DB, generated class `data.History`) | `DateColumnAdapter` | INTEGER | `java.util.Date` | encode `date.time` (epoch ms); decode `Date(ms)` |
| `animehistory.last_seen` (anime DB, `dataanime.Animehistory`) | `DateColumnAdapter` | INTEGER | `Date` | same |
| `mangas.genre` (`data.Mangas`) | `StringListColumnAdapter` | TEXT | `List<String>` | encode `joinToString(", ")` (separator is comma + space); decode: empty string gives empty list, else `split(", ")` |
| `animes.genre` (`dataanime.Animes`) | `StringListColumnAdapter` | TEXT | `List<String>` | same |
| `mangas.update_strategy` | `MangaUpdateStrategyColumnAdapter` | INTEGER | `UpdateStrategy` | encode `ordinal`; decode `entries.getOrElse(value.toInt()) { ALWAYS_UPDATE }` |
| `animes.update_strategy` | `AnimeUpdateStrategyColumnAdapter` | INTEGER | `AnimeUpdateStrategy` | same |
| `animes.fetch_type` | `FetchTypeColumnAdapter` | INTEGER | `FetchType` | encode `ordinal`; decode default `Episodes` (ordinal 1) |
| `animes.memo`, `episodes.memo` | `MemoColumnAdapter` | BLOB | `kotlinx.serialization.json.JsonObject` | encode `value.toString().encodeToByteArray()` (UTF-8 JSON text); decode `Json.decodeFromString<JsonObject>(bytes.decodeToString())` (throws on malformed JSON) |

Known sharp edge (OBSERVED): a genre string that itself contains `", "` splits into two genres after a round trip; there is no escaping. Rust must reproduce the same encode/decode to remain compatible with Aniyomi backup content (area:22-backup-restore); it must not "fix" it silently.

Proposal for rusqlite (T-01a-014, T-01a-015): newtypes `UnixMillis(i64)`, `StringList(Vec<String>)`, `UpdateStrategy` (`#[repr(u8)]`, `FromSql` via ordinal with the same fallbacks), `FetchType`, `Memo(serde_json::Map<String, Value>)`, each implementing `rusqlite::types::{FromSql, ToSql}`.

## 11. Network endpoints touched by this area

This area performs no network request of its own. Constants only: the two help URLs (section 7) are opened in the system browser (`xdg-open` through the `Opener` trait, not fetched). `NetworkPreferences.defaultUserAgent` (section 5.3) is consumed by area:12-network-stack. The commented-out ACRA fields in `build.gradle.kts:30-40` describe an HTTP crash upload (URI, login, password) that is disabled in this fork.

## 12. User-visible strings used by this area (message keys and English text)

| Key | Module | English text | Use |
|---|---|---|---|
| `MR.strings.app_name` | `i18n/src/commonMain/moko-resources/base/strings.xml:3` | `Aniyomi` (non-translatable) | crash description argument |
| `MR.strings.crash_screen_title` | same `:880` | `Whoops!` | crash heading |
| `MR.strings.crash_screen_description` | same `:881` | `%s ran into an unexpected error. We suggest you share the crash logs in our support channel on Discord.` | crash subtitle |
| `MR.strings.crash_screen_restart_application` | same `:882` | `Restart the application` | crash secondary button |
| `MR.strings.pref_dump_crash_logs` | same `:624` | `Share crash logs` | crash primary button |
| `MR.strings.pref_dump_crash_logs_summary` | same `:625` | `Saves error logs to a file for sharing with the developers` | settings row (area:47) |
| `MR.strings.pref_incognito_mode` | same `:655` | `Incognito mode` | notification title, channel name, secure-screen option |
| `MR.strings.pref_incognito_mode_summary` | same `:656` (`i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:33` overrides with `Pauses your history`) | `Pauses reading history` | settings row |
| `MR.strings.notification_incognito_text` | same `:658` | `Disable incognito mode` | notification text |
| `MR.strings.action_share` | same `:151` | `Share` | chooser title |
| `MR.strings.lock_always` / `lock_never` | same `:256-257` | `Always` / `Never` | secure screen options |
| `MR.strings.label_library` / `label_recent_updates` / `label_backup` | same `:27`, `:28`, `:32` | `Library` / `Updates` / `Backup and restore` | channel group names |
| `MR.strings.download_notifier_downloader_title` | same `:965` | `Downloader` | channel group name |
| `MR.strings.channel_common` / `channel_progress` / `channel_complete` / `channel_errors` / `channel_app_updates` / `channel_ext_updates` | same `:976-982` | `Common` / `Progress` / `Complete` / `Errors` / `App updates` / `Extension updates` | channel names |
| `AYMR.strings.channel_new_chapters_episodes` | `i18n-aniyomi/.../strings.xml:539` | `Chapter/Episode updates` | channel name |
| `AYMR.strings.pref_category_torrentserver` | same `:392` | `Torrent Server Preferences` | channel name |
| `AYMR.strings.pref_http_server_name` | same `:394` | `Http server` | channel name |
| hard-coded `"Failed to get logs"` | `CrashLogUtil.kt` | `Failed to get logs` | toast when the dump fails (not a resource) |
| hard-coded log text | `App.kt`, `GEH` | `Failed to modify notification channels`, `Migration from X to Y`, `Updating last version to Y`, `Wasn't able to retrieve throwable from intent`, `Failed to update widget` | log lines only |

Proposal for Kurobayomi: new keys with the same names plus `crash_screen_description` reworded to `%s ran into an unexpected error. Share the crash logs when you report the problem.` (no Discord), `crash_logs_saved` = `Crash logs copied to the clipboard` and `crash_logs_failed` = `Failed to get logs` (moved to a resource). Translation mechanism is area:51-i18n.

## 13. `AppInfo` payload (what extension/plugin code can query)

| Field | Aniyomi value | Rust value |
|---|---|---|
| version code | 132 | `VERSION_CODE` (integer ladder, starts at 1) |
| version name | `0.18.2.1` (+ `-<commit count>` in debug) | `CARGO_PKG_VERSION` (+ `-<commit count>` in debug) |
| supported image MIME types | `image/avif`, `image/gif`, `image/heif`, `image/jpeg`, `image/jxl`, `image/png`, `image/webp` | the subset of those that the compiled decoders support, in the same order (see T-01a-002) |
