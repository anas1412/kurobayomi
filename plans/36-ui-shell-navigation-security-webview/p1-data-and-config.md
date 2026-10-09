# Area 36 — Data models, preferences, files, endpoints, DB

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Storage notes: "stored" means the value written to the preference store (`PreferenceStore`, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt).

---

## 1. Data models in scope

### 1.1 Enum storage encoding (OBSERVED)
`PreferenceStore.getEnum` (PreferenceStore.kt:27-45) serializes with `it.name` (the enum constant name, case-sensitive) and deserializes with `enumValueOf`. An unknown stored name falls back to the default. There is no ordinal storage.

Proposal for Rust: store the exact Kotlin constant names as TEXT so backups from Aniyomi keep their meaning. NavStyle: MOVE_MANGA_TO_MORE, MOVE_UPDATES_TO_MORE, MOVE_HISTORY_TO_MORE (default), MOVE_BROWSE_TO_MORE. StartScreen: ANIME (default), MANGA, UPDATES, HISTORY, BROWSE. TabletUiMode: AUTOMATIC (default), ALWAYS, LANDSCAPE, NEVER. ThemeMode: LIGHT, DARK, SYSTEM (default). SecureScreenMode: ALWAYS, INCOGNITO (default), NEVER. AppTheme: the 22 names in section 1.8. Unknown names fall back to the default.

### 1.2 `HomeScreen.Tab` (sealed interface)
File: app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt:335-342

| Variant | Kind | Fields (type, default) | Meaning |
|---|---|---|---|
| AnimeLib | data class | animeIdToOpen: Long? = null | Open anime library; optionally push AnimeScreen |
| Library | data class | mangaIdToOpen: Long? = null | Open manga library; optionally push MangaScreen |
| Updates | data object | — | Updates tab |
| History | data object | — | History tab |
| Browse | data class | toExtensions: Boolean = false; anime: Boolean = false | Browse; optionally extensions page (manga or anime) |
| More | data class | toDownloads: Boolean | More tab; optionally push DownloadsTab |

Mapping to Tab objects: AnimeLib -> `AnimeLibraryTab`; Library -> `MangaLibraryTab`; Updates -> `UpdatesTab`; History -> `HistoriesTab`; Browse -> `BrowseTab`; More -> `MoreTab`.

### 1.3 `eu.kanade.presentation.util.Tab` (interface, Navigator.kt)
- `suspend fun onReselect(navigator: Navigator)` default no-op.
- `@Composable fun currentNavigationStyle(): NavStyle` reads `navStyle` from UiPreferences.

### 1.4 `NavStyle` (enum, app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt)
| Constant | Stored name | titleRes | moreTab |
|---|---|---|---|
| MOVE_MANGA_TO_MORE | "MOVE_MANGA_TO_MORE" | AYMR.strings.pref_bottom_nav_no_manga | MangaLibraryTab |
| MOVE_UPDATES_TO_MORE | "MOVE_UPDATES_TO_MORE" | AYMR.strings.pref_bottom_nav_no_updates | UpdatesTab |
| MOVE_HISTORY_TO_MORE | "MOVE_HISTORY_TO_MORE" (default) | AYMR.strings.pref_bottom_nav_no_history | HistoriesTab |
| MOVE_BROWSE_TO_MORE | "MOVE_BROWSE_TO_MORE" | AYMR.strings.pref_bottom_nav_no_browse | BrowseTab |

Derived: `tabs` (order preserved from `[AnimeLibraryTab, MangaLibraryTab, UpdatesTab, HistoriesTab, BrowseTab, MoreTab]` minus moreTab), `moreIcon` (see behavior §2.4).

### 1.5 `StartScreen` (enum, app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt)
| Constant | Stored | titleRes | tab |
|---|---|---|---|
| ANIME | "ANIME" (default) | AYMR.strings.label_anime | AnimeLibraryTab |
| MANGA | "MANGA" | AYMR.strings.manga | MangaLibraryTab |
| UPDATES | "UPDATES" | MR.strings.label_recent_updates | UpdatesTab |
| HISTORY | "HISTORY" | MR.strings.label_recent_manga | HistoriesTab |
| BROWSE | "BROWSE" | MR.strings.browse | BrowseTab |

### 1.6 `TabletUiMode` (enum, app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt)
| Constant | Stored | titleRes |
|---|---|---|
| AUTOMATIC | "AUTOMATIC" (default) | MR.strings.automatic_background |
| ALWAYS | "ALWAYS" | MR.strings.lock_always |
| LANDSCAPE | "LANDSCAPE" | MR.strings.landscape |
| NEVER | "NEVER" | MR.strings.lock_never |

### 1.7 `ThemeMode` (enum, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt)
No titleRes. Constants LIGHT, DARK, SYSTEM (default). Mapping to AppCompat night mode: LIGHT -> MODE_NIGHT_NO (1), DARK -> MODE_NIGHT_YES (2), SYSTEM -> MODE_NIGHT_FOLLOW_SYSTEM (-1). Those are Android AppCompat int values; Rust does not use them (see behavior §9.4).

### 1.8 `AppTheme` (enum, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt)
Constants with titleRes (in declaration order): DEFAULT (MR label_default), MONET (theme_monet), CLOUDFLARE (AYMR theme_cloudflare), COTTONCANDY (AYMR theme_cottoncandy), DOOM (AYMR theme_doom), GREEN_APPLE (theme_greenapple), LAVENDER (theme_lavender), MATRIX (AYMR theme_matrix), MIDNIGHT_DUSK (theme_midnightdusk), MOCHA (AYMR theme_mocha), SAPPHIRE (AYMR theme_sapphire), NORD (theme_nord), STRAWBERRY_DAIQUIRI (theme_strawberrydaiquiri), TAKO (theme_tako), TEALTURQUOISE (theme_tealturquoise), TIDAL_WAVE (theme_tidalwave), YINYANG (theme_yinyang), YOTSUBA (theme_yotsuba), MONOCHROME (theme_monochrome). Deprecated with `titleRes = null`: DARK_BLUE, HOT_PINK, BLUE.

Stored names are the constant names. Default: MONET when `DeviceUtil.isDynamicColorAvailable`, else DEFAULT.

Android style map (ThemingDelegate.kt): 18 entries (all except DEFAULT and the three deprecated values) map to `R.style.Theme_Tachiyomi_<Name>`; absent entries use `R.style.Theme_Tachiyomi`. Amoled overlay: `R.style.ThemeOverlay_Tachiyomi_Amoled`.

### 1.9 `SecurityPreferences.SecureScreenMode` (enum, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt)
| Constant | Stored | titleRes |
|---|---|---|
| ALWAYS | "ALWAYS" | MR.strings.lock_always |
| INCOGNITO | "INCOGNITO" (default) | MR.strings.pref_incognito_mode |
| NEVER | "NEVER" | MR.strings.lock_never |

Effective secure flag (Android only): `ALWAYS || (INCOGNITO && incognito)`.

### 1.10 `BasePreferences.ExtensionInstaller` (enum, for reference only; owned by area 18/19)
LEGACY, PACKAGEINSTALLER, SHIZUKU, PRIVATE; not used in this scope.

### 1.11 `DeepLinkScreenType` (enum, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/DeepLinkScreenType.kt)
`ANIME`, `MANGA`. The enum's `name` is written as the extra value by the trampolines and parsed with `valueOf` (MainActivity.kt:510).

### 1.12 `DeepLinkAnimeScreenModel.State` and `DeepLinkMangaScreenModel.State`
| State | Fields | Annotation |
|---|---|---|
| Loading | — | `@Immutable data object` |
| NoResults | — | `@Immutable data object` |
| Result | anime: Anime (domain), episodeId: Long? = null | `@Immutable data class` |
| Result (manga) | manga: Manga (domain), chapterId: Long? = null | `@Immutable data class` |

### 1.13 `DownloadQueueState` (sealed interface, MoreTab.kt:70-74)
- `Stopped` (data object), `Paused(pending: Int)`, `Downloading(pending: Int)`.
- Derivation (MoreTab.kt:60-74): `pending = mangaQueueSize + animeQueueSize`; if `pending == 0` -> Stopped; else if neither downloader is running -> Paused(pending); else Downloading(pending).

### 1.14 `TabContent` (data class, TabbedScreen.kt:125-134)
| Field | Type | Default |
|---|---|---|
| titleRes | StringResource | required |
| badgeNumber | Int? | null |
| searchEnabled | Boolean | false |
| actions | ImmutableList<AppBar.AppBarAction> | persistentListOf() |
| content | @Composable (PaddingValues, SnackbarHostState) -> Unit | required |
| numberTitle | Int | 0 |
| cancelAction | () -> Unit | {} |
| navigateUp | (() -> Unit)? | null |

### 1.15 `AppBar` model (AppBar.kt:441-456)
- `sealed interface AppBar`, `sealed interface AppBarAction`.
- `data class Action(title: String, icon: ImageVector, iconTint: Color? = null, onClick: () -> Unit, enabled: Boolean = true)`: icon button.
- `data class OverflowAction(title: String, onClick: () -> Unit)`: menu item.

### 1.16 `TabbedDialogPaddings`
`Horizontal = 24.dp`, `Vertical = 8.dp`.

### 1.17 `OnboardingStep` contract
`isComplete: Boolean` (property), `Content()` composable. Implementations: ThemeStep (isComplete = true, constant), StorageStep (mutable, true iff `baseStorageDirectory().isSet()`), PermissionStep (true), GuidesStep (true).

### 1.18 `NewUpdateScreen` inputs
`versionName: String`, `changelogInfo: String`, `releaseLink: String`, `downloadLink: String`. Values come from `GetApplicationRelease.Result.NewUpdate.release` (`version`, `info`, `releaseLink`, `downloadLink`); field definitions are in area 35.

### 1.19 WebView inputs
- `url: String` (required), `initialTitle: String?`, `sourceId: Long?`, `headers: Map<String, String>` (lowercase keys from OkHttp `toMultimap`; value = first header value or `""`).
- `assistUrl: String?` (last `onPageStarted`/`doUpdateVisitedHistory` URL).

### 1.20 Lock state (in-memory, OBSERVED)
- `SecureActivityDelegate.requireUnlock: Boolean` (static). Initial `true`. Not persisted.
- `AuthenticatorUtil.isAuthenticating: Boolean` (static). Initial `false`. Not persisted.

### 1.21 Intent extras and saved-state keys (OBSERVED)
| Key | Type | Set by | Read by |
|---|---|---|---|
| "notificationId" | Int (default -1) | notification PendingIntents (area 35) | MainActivity.handleIntentAction |
| "groupId" | Int (default 0) | notification (area 35) | MainActivity.handleIntentAction |
| Constants.MANGA_EXTRA = "manga" | Long | SHORTCUT_MANGA | MainActivity |
| Constants.ANIME_EXTRA = "anime" | Long | SHORTCUT_ANIME | MainActivity |
| "type" (MainActivity.INTENT_SEARCH_TYPE) | String "ANIME"/"MANGA" | DeepLink trampolines | MainActivity (search) |
| "query" (INTENT_SEARCH_QUERY) | String | extension intents | MainActivity |
| "filter" (INTENT_SEARCH_FILTER) | String | extension intents | MainActivity |
| SearchManager.QUERY = "query" | String | ACTION_SEARCH | MainActivity |
| Intent.EXTRA_TEXT = "android.intent.extra.TEXT" | String | ACTION_SEND | MainActivity |
| "saved_state_anime_key" (SAVED_STATE_ANIME_KEY) | Long | onSaveInstanceState | onCreate result handler |
| "saved_state_episode_key" (SAVED_STATE_EPISODE_KEY) | Long | onSaveInstanceState | onCreate result handler |
| "url_key" (WebViewActivity.URL_KEY) | String | WebViewActivity.newIntent | WebViewActivity |
| "source_key" (SOURCE_KEY) | Long? | newIntent | WebViewActivity (getLong; absent -> 0) |
| "title_key" (TITLE_KEY) | String? | newIntent | WebViewActivity |
| "anime_key" (ANIME_KEY) | Boolean | newIntent | never read |

Intent actions (Constants.kt): see behavior §4.1 table (SHOW_ANIMELIB, SHOW_LIBRARY, SHOW_ANIME, SHOW_MANGA, SHOW_RECENTLY_UPDATED, SHOW_RECENTLY_READ, SHOW_CATALOGUES, ANIMEEXTENSIONS, EXTENSIONS, SHOW_DOWNLOADS, SHOW_ANIME_DOWNLOADS, all prefixed `eu.kanade.tachiyomi.`). Custom search actions: `eu.kanade.tachiyomi.SEARCH`, `eu.kanade.tachiyomi.ANIMESEARCH`.

Route keys (Voyager/Compose keys, OBSERVED, not persisted): `"HomeTabs"` (TabNavigator), `"currentTab"` (saveableState key for tab content), `"tabContent"` (AnimatedContent label), `"stepContent"` (onboarding label), `"transition"` (screen transition key).

---

## 2. Constants

| Name | Value | File:line |
|---|---|---|
| SEARCH_DEBOUNCE_MILLIS | 250L | presentation/components/AppBar.kt:65 |
| SPLASH_MIN_DURATION | 500 (ms) | ui/main/MainActivity.kt:665 |
| SPLASH_MAX_DURATION | 5000 (ms) | MainActivity.kt:666 |
| SPLASH_EXIT_ANIM_DURATION | 400L (ms) | MainActivity.kt:667 |
| TAB_FADE_DURATION | 200 (ms) | ui/home/HomeScreen.kt:74 |
| TAB_NAVIGATOR_KEY | "HomeTabs" | HomeScreen.kt:75 |
| GETTING_STARTED_URL | "https://aniyomi.org/docs/guides/getting-started" | presentation/more/onboarding/GuidesStep.kt:59 |
| HELP_URL (storage FAQ) | "https://aniyomi.org/docs/faq/storage" | presentation/more/settings/screen/SettingsDataScreen.kt:89 |
| Cloudflare help URL | "https://aniyomi.org/docs/guides/troubleshooting/#cloudflare" | presentation/webview/WebViewScreenContent.kt:183 |
| Constants.URL_HELP | "https://aniyomi.org/docs/guides/troubleshooting/" | core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt (used outside this scope) |
| Constants.URL_HELP_UPCOMING | "https://aniyomi.org/docs/faq/updates/upcoming" | core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt (used outside this scope) |
| TABLET_UI_REQUIRED_SCREEN_WIDTH_DP | 720 | util/system/DisplayExtensions.kt:12 |
| TABLET_UI_MIN_SCREEN_WIDTH_PORTRAIT_DP | 700 | DisplayExtensions.kt:15 |
| TABLET_UI_MIN_SCREEN_WIDTH_LANDSCAPE_DP | 600 | DisplayExtensions.kt:18 |
| MINIMUM_WEBVIEW_VERSION | 118 | core/common/.../util/system/WebViewUtil.kt |
| Lock minutes -> ms factor | 60_000 | app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt (`onApplicationStart`, `lockDelay * 60_000`) |
| Sheet max width | 460 dp | presentation/components/AdaptiveSheet.kt:66 (doc comment) |
| Dropdown width | 196 dp (min = max) | DropdownMenu.kt:45 |
| Dropdown default offset | DpOffset(8.dp, -56.dp) | DropdownMenu.kt:37 |
| Tab-row edge padding (scrollable) | 13 dp | TabbedScreen.kt:145 |
| Download indicator size | 26 dp; padding 2 dp; arrow size 19 dp (26-7) | ItemDownloadIndicator.kt:36-46 |
| Relative sheet/tab transitions | fade 220 ms, delay 90 ms; out 90 ms | AdaptiveSheet.kt:37-38 |
| Shared-axis tab fades | 200 ms each | HomeScreen.kt:131-133 |
| Reveal animations | 500 ms hide; 350 ms show | RevealAnimationView.kt:36, 68 (dropped) |
| Exit animation (pre-S splash) | 400 ms | MainActivity.kt:667 |
| Marquee repeat delay | 2000 ms | AppBar.kt:205 |
| Cloudflare markers | "window._cf_chl_opt", "Ray ID is" | WebViewScreenContent.kt:81 |
| Blob prefix | "blob:http" | WebViewScreenContent.kt:103 |
| Intent blocklist prefix | "intent://" | WebViewScreenContent.kt:108 |
| Changelog regex | `---(\R|.)*Checksums(\R|.)*` | ui/more/NewUpdateScreen.kt:25 |
| Release repo | "aniyomiorg/aniyomi" (preview: "aniyomiorg/aniyomi-preview") | app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt (`GITHUB_REPO`) |
| Release URL | "https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG" | AppUpdateChecker.kt (`RELEASE_URL`) |
| Preview tag | "r${BuildConfig.COMMIT_COUNT}" | AppUpdateChecker.kt (`RELEASE_TAG`, preview branch) |
| Stable tag | "v${BuildConfig.VERSION_NAME}" | AppUpdateChecker.kt (`RELEASE_TAG`, stable branch) |
| Icon marker: `ic_updates_outline_24dp` | drawable | NavStyle |
| Tab indices | AnimeLibraryTab 0; BrowseTab 3; MoreTab 4; DownloadsTab 6 | TabOptions |
| Badge font (AppBar count pill) | 14 sp; alpha 0.12 dark / 0.08 light | AppBar.kt:181-186 |
| Search field font | titleMedium, 18 sp, Normal | AppBar.kt:329-333 |
| SearchToolbar placeholder font | titleMedium 18 sp, Normal | AppBar.kt:358-361 |
| Status bar luminance threshold | 0.5 | MainActivity.kt:195 |
| Scrim alpha | 0.8 (Android only) | MainActivity.kt:259 |
| Banner padding | 4 dp (downloaded-only, incognito); 8 dp (indexing); warning 16 dp | Banners.kt:134, 149, 164, 54 |
| Warning banner inset | 8 dp surround | WebViewScreenContent.kt:175 |
| Indexing spinner stroke | textHeight / 8 | Banners.kt:172 |

---

## 3. Preference keys used or read in scope

Key strings are exact. `App state` = the key is stored under `Preference.appStateKey(name)`, i.e. the literal prefix `__APP_STATE_` plus the name (Preference.kt:43-47).

| Key (stored string) | Type | Default | Owner (declared at) | Read/written by (scope file) | Notes |
|---|---|---|---|---|---|
| `use_biometric_lock` | Boolean | false | SecurityPreferences.kt:13 | SecureActivityDelegate, AuthenticatorUtil, settings area 47 | Lock enable. Written to false when no credential available (SecureActivityDelegate.kt:119) |
| `lock_app_after` | Int | 0 | SecurityPreferences.kt:15 | SecureActivityDelegate | -1 never; 0 always; >0 minutes |
| `secure_screen_v2` | enum name (SecureScreenMode) | INCOGNITO | SecurityPreferences.kt:17 | SecureActivityDelegate | FLAG_SECURE only on Android |
| `hide_notification_content` | Boolean | false | SecurityPreferences.kt:19 | area 35 (not read here) | listed for completeness |
| `__APP_STATE_last_app_closed` | Long | 0 | SecurityPreferences.kt (`lastAppClosed`, `Preference.appStateKey("last_app_closed")`) | SecureActivityDelegate | written on stop when lock delayed; deleted on start |
| `__APP_STATE_pref_downloaded_only` | Boolean | false | BasePreferences.kt (`downloadedOnly`) | MainActivity (banner, status colour), MoreTab | downloaded-only mode |
| `__APP_STATE_incognito_mode` | Boolean | false | BasePreferences.kt (`incognitoMode`) | MainActivity (reset on launch), SecureActivityDelegate, TachiyomiTextInputEditText | reset to false every launch (MainActivity.kt:216) |
| `__APP_STATE_onboarding_complete` | Boolean | false | BasePreferences.kt (`shownOnboardingFlow`) | OnboardingScreen, MainActivity.ShowOnboarding | onboarding finished |
| `__APP_STATE_storage_dir` | String | `folderProvider.path()` | domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt:12 | StorageStep | base storage directory; `isSet()` gates Next |
| `pref_theme_mode_key` | enum ThemeMode | SYSTEM | UiPreferences.kt:20 | ThemeStep, setAppCompatDelegateThemeMode | key literal contains "_key" (kept) |
| `pref_app_theme` | enum AppTheme | MONET if dynamic colour else DEFAULT | UiPreferences.kt:22-29 | ThemeStep, ThemingDelegate | Linux proposal default: DEFAULT |
| `pref_theme_dark_amoled_key` | Boolean | false | UiPreferences.kt:31 | ThemeStep, ThemingDelegate | AMOLED overlay |
| `relative_time_v2` | Boolean | true | UiPreferences.kt:33 | DateText | relative dates |
| `app_date_format` | String | "" | UiPreferences.kt:35 | DateText | "" = localized SHORT; else pattern |
| `tablet_ui_mode` | enum TabletUiMode | AUTOMATIC | UiPreferences.kt:37 | DisplayExtensions.prepareTabletUiContext | — |
| `start_screen` | enum StartScreen | ANIME | UiPreferences.kt:39 | HomeScreen (read at object init) | not live (behavior §2.1) |
| `bottom_rail_nav_style` | enum NavStyle | MOVE_HISTORY_TO_MORE | UiPreferences.kt:41 | HomeScreen, Navigator.kt (Tab.currentNavigationStyle), MoreTab | moreTab captured at object init |
| `library_show_updates_count` | Boolean | true | LibraryPreferences (`newShowUpdatesCount`) | HomeScreen (updates badge) | — |
| `library_unread_updates_count` | Int | 0 | LibraryPreferences (`newMangaUpdatesCount`) | HomeScreen (manga updates count) | key name says "unread" |
| `library_unseen_updates_count` | Int | 0 | LibraryPreferences (`newAnimeUpdatesCount`) | HomeScreen (anime updates count) | — |
| `ext_updates_count` | Int | 0 | SourcePreferences (`mangaExtensionUpdatesCount`) | HomeScreen (Browse badge, manga) | — |
| `animeext_updates_count` | Int | 0 | SourcePreferences (`animeExtensionUpdatesCount`) | HomeScreen (Browse badge, anime) | — |
| `auto_clear_chapter_cache` | Boolean | false | LibraryPreferences (`autoClearItemCache`) | MainActivity (clear on launch) | — |

Preferences consumed by reference only (not declared here): `pref_display_profile_key` (BasePreferences.kt), `pref_hardware_bitmap_threshold`, and `pref_always_decode_long_strip_with_ssiv` (both BasePreferences.kt; reader area 43).

### 3.1 Keys not present in the Kotlin
- The lock timestamp and the "Pending timed lock" marker both use `__APP_STATE_last_app_closed`. No other keys.
- Shown-once changelog: not persisted (derived from migration result each launch).

---

## 4. Files, folders, formats on disk

Written by this area: none directly (OBSERVED). Read or referenced:
| Item | Format | Owner | Notes |
|---|---|---|---|
| Backup file `*.tachibk` | Protobuf-based backup (area 21) | area 21/22 | Opened by ACTION_VIEW; `RestoreBackupScreen(uri)` |
| Storage base directory | folder chosen in StorageStep | StoragePreferences | default `folderProvider.path()`; picker returns a SAF/Uri on Android |
| Cache directory | chapter cache | area 34 | cleared on launch when `auto_clear_chapter_cache` |
| Android share temp files | none | — | dropped |

Proposal files for Kurobayomi (new, this area):
| Path | Format | Contents | Permission |
|---|---|---|---|
| `$XDG_CONFIG_HOME/kurobayomi/lock.json` | JSON `{ "version": 1, "phc": "$argon2id$v=19$..." }` | Argon2id PHC of the lock passphrase | 0600 |
| `$XDG_DATA_HOME/kurobayomi/webview/` | WebKitGTK website data (cookies.sqlite, local storage) | WebView cookie and storage | 0700 |
| `$XDG_DATA_HOME/kurobayomi/` | directory | base storage default | 0755 |
| `$XDG_CONFIG_HOME/kurobayomi/shell.json` (Proposal) | JSON | `{ "window": { "width": 1180, "height": 780, "maximized": false }, "sidebar_collapsed": false }` | 0644 |

Shell.json is an optional proposal; the mockup already uses 1180x780 defaults (src/main.rs, `build_ui`, lines 22-26).

---

## 5. Network endpoints touched

No HTTP request is made directly by this scope. Browser / WebView navigations and external-link opens:

| Trigger | Method | URL | Headers | Response use |
|---|---|---|---|---|
| "Getting started guide" button (GuidesStep, onboarding empty-state) | external open (no HTTP from app) | `https://aniyomi.org/docs/guides/getting-started` | — | browser |
| Storage help | external | `https://aniyomi.org/docs/faq/storage` | — | browser |
| Cloudflare banner click | external | `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare` | — | browser |
| Changelog "What's new" | external | `RELEASE_URL` = `https://github.com/aniyomiorg/aniyomi/releases/tag/v<VERSION_NAME>` (preview: `.../aniyomi-preview/releases/tag/r<COMMIT_COUNT>`) | — | browser |
| Update prompt "open in browser" | external | `releaseLink` from update info | — | browser |
| Update check | GET via GetApplicationRelease (domain) | GitHub release for `GITHUB_REPO` (exact endpoint in area 35) | per area 12 | `NewUpdate` / other results |
| Extension update check | via extension APIs | owned by areas 18 and 19 | — | counts to `ext_updates_count`, `animeext_updates_count` |
| WebView load | GET (any) | user / source URL | `headers` map (source headers, lowercase names; `user-agent` overrides UA) | page |
| WebView `shouldOverrideUrlLoading` | re-issued GET | same URL | same headers | page |
| Cookie clear | none | host of the current URL | — | `cookieJar.remove(url)` |

URI schemes handled (inbound): `tachiyomi://add-repo?url=<repo>`, `aniyomi://add-repo?url=<repo>`, `aniyomi://extension-store?url=<repo>`, `file:`/`content:` with `.tachibk`.

---

## 6. Database

This scope issues no SQL directly. It calls interactors and therefore touches these tables indirectly (owners in brackets):

| Interactor (called) | Tables (owner area) |
|---|---|
| `GetAnime` (MainActivity.startPlayerActivity) | anime entries table (area 04/05) |
| `GetMangaByUrlAndSourceId`, `GetAnimeByUrlAndSourceId` | manga / anime entries by `(url, source)` (area 05) |
| `NetworkToLocalManga`, `NetworkToLocalAnime` | insert of new entries (area 05/07) |
| `GetChapterByUrlAndMangaId`, `GetEpisodeByUrlAndAnimeId` | chapters / episodes by `(url, manga_id)` / `(url, anime_id)` (area 05) |
| `SyncChaptersWithSource` | chapter insert/update (area 07) |
| `UpdateAnimeFromRemote.awaitEpisodesUpdate` | episode insert/update (area 16/24) |
| `ChapterCache.clear` | file cache, not DB (area 34) |
| `GetMangaIncognitoState`, `GetAnimeIncognitoState` | incognito state (area 08/09) |

Proposal: the deep-link resolve needs `get_by_url_and_source(url, source_id)` and `get_by_url_and_parent(url, parent_id)` helper functions in the data layer. Those are owned by areas 05/06; this area consumes them.

No bit-flag layouts exist in this scope.

---

## 7. Migrations
This scope does not define migrations. Migrator (mihon/core/migration/Migrator.kt) is consumed (owner area 11). Its result `didMigration` is in-memory only.

---

## 8. Limits and sizes
- Recent-page history of tabs: not stored (in memory per navigator).
- Deep-link resolve: one source lookup per query; no retry.
- Lock prompt attempts: unlimited (Android prompt rules apply); Proposal caps passphrase attempts at 5 before a 30 s delay (Proposal, not observed).
- WebView cookie store size: browser default (no limit set in the Kotlin).
