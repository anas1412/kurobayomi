# Area 36 — Behaviour specification

Labels: **OBSERVED** = what the Kotlin does (with path:line). **Proposal:** = what Kurobayomi should do. Message keys are `MR.strings.*` (core i18n) or `AYMR.strings.*` (Aniyomi i18n). Paths are repo-relative to `/home/blackbox/aniyomi-src`.

---

## 1. Process and window startup (MainActivity)

### 1.1 OBSERVED: onCreate order
File: app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:158-346

1. `isLaunch = savedInstanceState == null` (line 159). Only a fresh launch (not a configuration change or process restore) is a "launch".
2. If `isLaunch`, `installSplashScreen()` (line 162). Otherwise the splash is null.
3. `super.onCreate` (BaseActivity: applies theme first, see §9).
4. `Migrator.awaitAndRelease()` (line 166) is a blocking `runBlocking` that waits for the migration job started earlier, then releases it. Result `didMigration: Boolean` is used by the changelog dialog (§7).
5. `if (!isTaskRoot) { finish(); return }` (lines 169-172). A second launcher instance is finished immediately.
6. `setComposeContent { ... }` (line 174) builds the whole UI.
7. Splash keep-on-screen condition (lines 317-321):
   - `startTime = currentTimeMillis()`
   - keep on screen while `elapsed <= SPLASH_MIN_DURATION(500) || (!ready && elapsed <= SPLASH_MAX_DURATION(5000))`.
   - `ready` is set true by `handleIntentAction` (see §4.1 for the cases where it is NOT set).
8. `setSplashScreenExitAnimation(splashScreen)` (line 322). On API < 31 only: status and navigation bar transparent; exit animation 400 ms (`SPLASH_EXIT_ANIM_DURATION`), root translates Y from 16 dp to 0 with `LinearOutSlowInInterpolator`, splash alpha 1->0 with `FastOutSlowInInterpolator`, `splashProvider.remove()` on end.
9. If `isLaunch && libraryPreferences.autoClearItemCache().get()` then `chapterCache.clear()` on IO (lines 324-328). Key `auto_clear_chapter_cache`, default false.
10. Registers `externalPlayerResult` = `registerForActivityResult(StartActivityForResult)` (lines 330-345). On `RESULT_OK`: reads animeId/episodeId from `savedInstanceState` (captured from onCreate, so often null on a fresh start). If both non-null, `ExternalIntents.externalIntents.initAnime(animeId, episodeId)` (runBlocking), then `onActivityResult(result.data)`.

### 1.2 OBSERVED: Compose content
(MainActivity.kt:175-315)

- Reads, as state: `incognito` (Manga, `GetMangaIncognitoState.await(null)`), `incognitoAnime` (Anime), `downloadOnly` (`BasePreferences.downloadedOnly()`), `indexing` (`MangaDownloadCache.isInitializing`), `indexingAnime` (`AnimeDownloadCache.isInitializing`).
- Status bar background colour (lines 184-189), first match wins:
  1. `indexing || indexingAnime` -> `IndexingBannerBackgroundColor` (= colorScheme.secondary)
  2. `downloadOnly` -> `DownloadedOnlyBannerBackgroundColor` (= tertiary)
  3. `incognito || incognitoAnime` -> `IncognitoModeBannerBackgroundColor` (= primary)
  4. else `colorScheme.surface`
- Edge-to-edge (lines 190-198): `lightStyle = SystemBarStyle.light(TRANSPARENT, BLACK)`, `darkStyle = SystemBarStyle.dark(TRANSPARENT)`. Status style light if `statusBarBackgroundColor.luminance() > 0.5`, navigation style light unless system is dark.
- `Navigator(screen = HomeScreen, disposeBehavior = NavigatorDisposeBehavior(disposeNestedNavigators=false, disposeSteps=true))` (lines 200-205).
- Inside the navigator: `LaunchedEffect(navigator)` stores navigator into `this@MainActivity.navigator`. If `isLaunch`: `handleIntentAction(intent, navigator)` then `preferences.incognitoMode().set(false)` (lines 208-218). Incognito is always reset on launch.
- `LaunchedEffect(navigator.lastItem)` subscribes `getMangaIncognitoState.subscribe(sourceId)` only when `lastItem is BrowseMangaSourceScreen` (lines 219-223), and the same for anime (lines 225-229). Other screens emit `null`.
- Scaffold: `topBar = AppStateBanners(downloadedOnlyMode, incognitoMode = incognito||incognitoAnime, indexing = indexing||indexingAnime, modifier = windowInsetsPadding(scaffoldInsets))`; `contentWindowInsets = scaffoldInsets` where `scaffoldInsets = WindowInsets.navigationBars.only(Horizontal)` (lines 231-241).
- Body: `DefaultNavigatorScreenTransition` with `padding(contentPadding).consumeWindowInsets(contentPadding)`. Then if `isNavigationBarNeedsScrim()` (true when `SDK < Q` or resource `config_navBarNeedsScrim` true), a bottom spacer of navigation-bar height, `alpha(0.8f)`, background `surfaceContainer` (lines 253-262). Android-only; dropped on Linux (see §10).
- Incognito-off watcher (lines 267-286): `preferences.incognitoMode().changes().drop(1).filter { !it }`. On each emission of `false`, if the last item is `BrowseMangaSourceScreen`, `MangaScreen` with `fromSource == true`, `BrowseAnimeSourceScreen`, or `AnimeScreen` with `fromSource == true`, then `navigator.popUntilRoot()`.
- `HandleOnNewIntent(context, navigator)` (line 288), `CheckForUpdates()` (line 290), `ShowOnboarding()` (line 291).
- Changelog dialog (lines 294-314): `showChangelog = remember { didMigration && !BuildConfig.DEBUG }`. `AlertDialog` title `stringResource(MR.strings.updated_version, BuildConfig.VERSION_NAME)`, dismiss button `TextButton(MR.strings.whats_new)` opens `RELEASE_URL`, confirm `TextButton(MR.strings.action_ok)` closes.

### 1.3 OBSERVED: onNewIntent
(MainActivity.kt:358-368) A `callbackFlow` registers `ComponentActivity.addOnNewIntentListener` and sends each intent to `handleIntentAction` with `collectLatest`. The listener is removed on `awaitClose`.

### 1.4 OBSERVED: assist content
(MainActivity.kt:348-355) `onProvideAssistContent` sets `outContent.webUri` to `screen.onProvideAssistUrl()` only if the top screen implements `AssistContentScreen`. Android-only; dropped (§10).

### 1.5 Proposal: Linux startup
- Process start: create `gtk::Application` with id `io.github.blackbox.Kurobayomi` (already in mockup at `src/main.rs:13`). Use `connect_activate` for the first window and `connect_open` for URIs and files (replaces `onCreate` / `onNewIntent`).
- Single instance is provided by GApplication D-Bus registration. A second launch with args is forwarded to the primary instance via `open` or `activate`; the second process exits. This replaces `isTaskRoot`.
- Run migrations (area 11) on a background tokio task before the window is shown. Show the window only after the migration result and the first intent are handled (see §1.6). This replaces the splash gate.
- Changelog dialog: show once after an upgrade when migration reported "did migrate" and the build is not a debug build (see §7).
- Kurobayomi has no splash screen. A plain window with a centered `gtk::Spinner` is shown while `ready == false`, with the same 500 ms minimum and 5000 ms maximum (see §1.6).

### 1.6 Splash / ready state machine (Proposal)
States: `Starting -> Ready`. Transition to `Ready` when (a) the first intent or the default route has been applied and (b) at least 500 ms have elapsed; or force `Ready` at 5000 ms. The window is not presented while `Starting` and elapsed < 500 ms; after 5000 ms it is presented regardless.

Note the OBSERVED subtlety: `handleIntentAction` returns early with `ready` unset for `SHORTCUT_MANGA` / `SHORTCUT_ANIME` when the id extra is missing (`return false` at MainActivity.kt:475 and :480) and for `else -> return false` (line 564). In those cases the splash stays until the 5000 ms cap. Proposal: always set `Ready` after the intent router finishes, including the failure branch.

---

## 2. Navigation shell (HomeScreen, navigator, tabs)

### 2.1 OBSERVED: static defaults captured at object init
(HomeScreen.kt:68-79) `defaultTab = uiPreferences.startScreen().get().tab` and `moreTab = uiPreferences.navStyle().get().moreTab` are read once when the `HomeScreen` object initializes (process lifetime). Changing `start_screen` or `bottom_rail_nav_style` in Settings does not change `defaultTab` or `moreTab` until the next process start. The bar contents (`navStyle.tabs`) do update live because `navStyle` is collected in `Content()` (line 83). Proposal: read both reactively, or document and apply on restart.

### 2.2 OBSERVED: TabNavigator
(HomeScreen.kt:85-88) `TabNavigator(tab = defaultTab, key = "HomeTabs")`. The tab state survives recomposition under that key.

### 2.3 OBSERVED: Scaffold layout
- Tablet (`isTabletUi()`, see §8): `startBar = NavigationRail { navStyle.tabs.fastForEach { NavigationRailItem(it) } }` (lines 93-99).
- Phone: `bottomBar` (lines 102-117). `bottomNavVisible` comes from `produceState(true)` collecting `showBottomNavEvent`. The bar is `AnimatedVisibility(visible = bottomNavVisible && tabNavigator.current != navStyle.moreTab, enter = expandVertically(), exit = shrinkVertically())`. So the bar hides when the current tab is the one that was moved into More.
- Content: `AnimatedContent(targetState = tabNavigator.current, transitionSpec = materialFadeThroughIn(initialScale = 1f, durationMillis = 200) togetherWith materialFadeThroughOut(durationMillis = 200), label="tabContent")`, each tab rendered through `tabNavigator.saveableState("currentTab", it) { it.Content() }` (lines 126-140).
- `contentWindowInsets = WindowInsets(0)` (line 119).

### 2.4 OBSERVED: NavStyle and tab list
(core: app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt)
- `tabs` = `[AnimeLibraryTab, MangaLibraryTab, UpdatesTab, HistoriesTab, BrowseTab, MoreTab]` minus `this.moreTab`. Order is preserved.
- Four styles, each with `moreTab`:
  | NavStyle | title key | moreTab | Resulting bar (order) |
  |---|---|---|---|
  | MOVE_MANGA_TO_MORE | AYMR.strings.pref_bottom_nav_no_manga | MangaLibraryTab | Anime, Updates, History, Browse, More |
  | MOVE_UPDATES_TO_MORE | AYMR.strings.pref_bottom_nav_no_updates | UpdatesTab | Anime, Manga, History, Browse, More |
  | MOVE_HISTORY_TO_MORE (default) | AYMR.strings.pref_bottom_nav_no_history | HistoriesTab | Anime, Manga, Updates, Browse, More |
  | MOVE_BROWSE_TO_MORE | AYMR.strings.pref_bottom_nav_no_browse | BrowseTab | Anime, Manga, Updates, History, More |
- The "More" entry is always last (`MoreTab` is never removed unless it is the moreTab, which it never is).
- Icons per style for the More entry: MOVE_MANGA_TO_MORE -> `Icons.Outlined.CollectionsBookmark`; MOVE_UPDATES_TO_MORE -> drawable `ic_updates_outline_24dp`; MOVE_HISTORY_TO_MORE -> `Icons.Outlined.History`; MOVE_BROWSE_TO_MORE -> `Icons.Outlined.Explore`.

Tab indices (TabOptions.index), used for keyboard ordering on Android: AnimeLibraryTab 0; MoreTab 4; BrowseTab 3; DownloadsTab 6. Other tabs use a runtime `index` variable.

### 2.5 OBSERVED: StartScreen -> tab
(StartScreen.kt) `ANIME -> AnimeLibraryTab`, `MANGA -> MangaLibraryTab`, `UPDATES -> UpdatesTab`, `HISTORY -> HistoriesTab`, `BROWSE -> BrowseTab`. Titles: `AYMR.strings.label_anime`, `AYMR.strings.manga`, `MR.strings.label_recent_updates`, `MR.strings.label_recent_manga`, `MR.strings.browse`.

### 2.6 OBSERVED: go-to-start-screen and back
(HomeScreen.kt:145-156)
```
goToStartScreen():
  if defaultTab != moreTab  -> current = defaultTab
  else                      -> current = AnimeLibraryTab
BackHandler(
  enabled = (current == moreTab || current != defaultTab)
            && (current != AnimeLibraryTab || defaultTab != moreTab),
  onBack = goToStartScreen)
```
Truth table for `enabled`, with A = (current == moreTab), B = (current != defaultTab), C = (current != AnimeLibraryTab), D = (defaultTab != moreTab): `enabled = (A || B) && (C || D)`.

Consequences:
- On the start tab: back is not handled by the shell (exits app / falls through).
- On any other tab, back returns to start (or to AnimeLibraryTab when the start tab is the moreTab, which is the case that stops it looping).

### 2.7 OBSERVED: tab reselect
(HomeScreen.kt:209-220) Clicking the already-selected bar item calls `scope.launch { tab.onReselect(navigator) }`. Clicking a different item sets `tabNavigator.current`. `selected` = `tabNavigator.current::class == tab::class`. `MoreTab.onReselect` pushes `SettingsScreen()` (MoreTab.kt:37-39).

### 2.8 OBSERVED: badges
(HomeScreen.kt:262-312)
- Updates tab: `combine(newAnimeUpdatesCount.changes(), newMangaUpdatesCount.changes()) { a, m -> a + m }`, then `value = if (newShowUpdatesCount().get()) it else 0`. Shown when `count > 0`. Accessibility text: `pluralStringResource(MR.plurals.notification_chapters_generic, count)`. Sources: `library_unseen_updates_count` (anime, default 0), `library_unread_updates_count` (manga, default 0), `library_show_updates_count` (default true).
- Browse tab: `combine(mangaExtensionUpdatesCount, animeExtensionUpdatesCount) { m, a -> m + a }`, shown when `> 0`, description `MR.plurals.update_check_notification_ext_updates`. Sources: `ext_updates_count` (manga, default 0), `animeext_updates_count` (anime, default 0).
- Other tabs: no badge.
- Badge text is `count.toString()`, count is not capped.

### 2.9 OBSERVED: events channels
(HomeScreen.kt:70-72, 158-200, 323-333)
- `librarySearchEvent: Channel<String>` (`search(query)`): `goToStartScreen()`, then if `defaultTab == AnimeLibraryTab` -> `AnimeLibraryTab.search(query)`; if `MangaLibraryTab` -> `MangaLibraryTab.search(query)`; else nothing.
- `openTabEvent: Channel<Tab>` (`openTab`): sets `tabNavigator.current`:
  - `Tab.AnimeLib` -> AnimeLibraryTab; `Tab.Library` -> MangaLibraryTab; `Tab.Updates` -> UpdatesTab; `Tab.History` -> HistoriesTab.
  - `Tab.Browse(toExtensions, anime)`: if `toExtensions` then `BrowseTab.showAnimeExtension()` when `anime` else `BrowseTab.showExtension()`; then BrowseTab.
  - `Tab.More` -> MoreTab.
  - Then: `AnimeLib` with `animeIdToOpen != null` -> `navigator.push(AnimeScreen(id))`; `Library` with `mangaIdToOpen != null` -> `navigator.push(MangaScreen(id))`; `More(toDownloads = true)` -> `navigator.push(DownloadsTab)`.
- `showBottomNavEvent: Channel<Boolean>` (`showBottomNav(show)`) toggles the phone bar.
- Channels are `Channel` (rendezvous, unbuffered). `send` suspends until received, so callers block until the shell collects. Proposal: a bounded `async_channel` with capacity 16 and a drop-oldest policy.

### 2.10 OBSERVED: Tab sealed interface
`HomeScreen.Tab` (HomeScreen.kt:335-342): `AnimeLib(animeIdToOpen: Long? = null)`, `Library(mangaIdToOpen: Long? = null)`, `Updates`, `History`, `Browse(toExtensions: Boolean = false, anime: Boolean = false)`, `More(toDownloads: Boolean)`.

### 2.11 OBSERVED: tab reselect vs. tab change for library search
A reselected library tab does not auto-focus search. Search is only via `search(query)`.

### 2.12 Proposal: keyboard for the shell
- `Alt+1`..`Alt+5` select bar entries by position in `navStyle.tabs`. `Ctrl+F` focuses the active toolbar's search entry (see §5). `Esc` behaves like back (§2.6).

---

## 3. Navigator (Voyager replacement)

### 3.1 OBSERVED
(app/src/main/java/eu/kanade/presentation/util/Navigator.kt)
- `interface Tab : cafe.adriel.voyager.navigator.tab.Tab { suspend fun onReselect(navigator: Navigator) {}; currentNavigationStyle(): NavStyle }`.
- `abstract class Screen : Screen { override val key = uniqueScreenKey }`: every screen instance has a unique key, so two pushes of the same class are two stack entries.
- `DefaultNavigatorScreenTransition`: `materialSharedAxisX(forward = navigator.lastEvent != StackEvent.Pop, slideDistance = rememberSlideDistance())`.
- `ScreenTransition`: `AnimatedContent(targetState = navigator.lastItem)`, content wrapped in `navigator.saveableState("transition", screen)`.
- `ScreenModel.ioCoroutineScope`: `CoroutineScope(Dispatchers.IO + SupervisorJob()) + CoroutineName(key)`, cancelled on dispose.
- `LocalBackPress` composition local (nullable), used to invoke parent Activity back.

### 3.2 OBSERVED: sheet navigators
(AdaptiveSheet.kt:22-60) `NavigatorAdaptiveSheet(screen, enableSwipeDismiss, onDismissRequest)` creates a nested Navigator inside an `AdaptiveSheet`. Transition `fadeIn(tween(220, delayMillis=90)) togetherWith fadeOut(tween(90))`. `BackHandler(enabled = sheetNavigator.size > 1, onBack = sheetNavigator::pop)`. When the parent has `disposeNestedNavigators == false`, a `DisposableEffectIgnoringConfiguration` disposes all sheet items in reverse on leave.

### 3.3 Proposal: navigator model
A `Vec<Page>` stack per window (`Page` = enum of routes, see `p1-rust-design.md`). Push, pop, replace, popUntilRoot are the four operations. `disposeSteps=true` means each popped page drops its state object; `disposeNestedNavigators=false` means child stacks (sheets) are kept alive by their parent until the parent is popped.

---

## 4. Intent / URI / action dispatch

### 4.1 OBSERVED: handleIntentAction
(MainActivity.kt:461-573) Full algorithm, in order:

1. `notificationId = intent.getIntExtra("notificationId", -1)`. If `> -1`, `NotificationReceiver.dismissNotification(app, notificationId, intent.getIntExtra("groupId", 0))`.
2. `when (intent.action)`:

| Action constant (value) | Branch | Result |
|---|---|---|
| `Constants.SHORTCUT_ANIMELIB` ("eu.kanade.tachiyomi.SHOW_ANIMELIB") | tab = `Tab.AnimeLib()` | open tab |
| `Constants.SHORTCUT_LIBRARY` ("eu.kanade.tachiyomi.SHOW_LIBRARY") | tab = `Tab.Library()` | open tab |
| `Constants.SHORTCUT_MANGA` ("eu.kanade.tachiyomi.SHOW_MANGA") | id = extras.getLong("manga") ?: **return false**; popUntilRoot; tab = `Tab.Library(id)` | open tab + push MangaScreen(id) |
| `Constants.SHORTCUT_ANIME` ("eu.kanade.tachiyomi.SHOW_ANIME") | id = extras.getLong("anime") ?: **return false**; popUntilRoot; tab = `Tab.AnimeLib(id)` | open tab + push AnimeScreen(id) |
| `Constants.SHORTCUT_UPDATES` ("eu.kanade.tachiyomi.SHOW_RECENTLY_UPDATED") | `Tab.Updates` | open tab |
| `Constants.SHORTCUT_HISTORY` ("eu.kanade.tachiyomi.SHOW_RECENTLY_READ") | `Tab.History` | open tab |
| `Constants.SHORTCUT_SOURCES` ("eu.kanade.tachiyomi.SHOW_CATALOGUES") | `Tab.Browse(false)` | open tab |
| `Constants.SHORTCUT_ANIMEEXTENSIONS` ("eu.kanade.tachiyomi.ANIMEEXTENSIONS") | `Tab.Browse(true, true)` | open anime extensions |
| `Constants.SHORTCUT_EXTENSIONS` ("eu.kanade.tachiyomi.EXTENSIONS") | `Tab.Browse(true)` | open manga extensions |
| `Constants.SHORTCUT_DOWNLOADS` ("eu.kanade.tachiyomi.SHOW_DOWNLOADS") | popUntilRoot; `Tab.More(toDownloads=true)` | open downloads |
| `Constants.SHORTCUT_ANIME_DOWNLOADS` ("eu.kanade.tachiyomi.SHOW_ANIME_DOWNLOADS") | identical to SHOW_DOWNLOADS | open downloads |
| `Intent.ACTION_SEARCH`, `Intent.ACTION_SEND`, "com.google.android.gms.actions.SEARCH_ACTION" | see §4.2 | global search |
| `INTENT_SEARCH` ("eu.kanade.tachiyomi.SEARCH") | see §4.3 | manga global search |
| `INTENT_ANIMESEARCH` ("eu.kanade.tachiyomi.ANIMESEARCH") | see §4.3 | anime global search |
| `Intent.ACTION_VIEW` | see §4.4 | backup / repo deep links |
| else | `return false` (line 564) | nothing, ready NOT set |

3. If `tabToOpen != null`: `lifecycleScope.launch { HomeScreen.openTab(tabToOpen) }`.
4. `ready = true`; return true.

Note: for the search and VIEW branches, `tabToOpen` stays null, so `ready = true` is still reached.

Note on shortcut ids: the value strings above are the Constants.kt values. The static shortcuts XML referenced by the manifest (`android.app.shortcuts` -> `@xml/shortcuts`, area 53) is expected to use these action names; verify there before relying on it.

### 4.2 OBSERVED: search actions (ACTION_SEARCH / ACTION_SEND / SEARCH_ACTION)
(MainActivity.kt:497-524)
- `query = intent.getStringExtra(SearchManager.QUERY) ?: intent.getStringExtra(Intent.EXTRA_TEXT)`. `SearchManager.QUERY` is the string key "query".
- If `query` is non-null and non-empty:
  - `popUntilRoot()`
  - `screenType = intent.getStringExtra("type").orEmpty().ifBlank { "ANIME" }.let(DeepLinkScreenType::valueOf)`. **Edge: an unknown `type` value throws IllegalArgumentException** (no try/catch). Only "ANIME" and "MANGA" are valid.
  - MANGA: push `GlobalMangaSearchScreen(query)` then push `DeepLinkMangaScreen(query)`.
  - ANIME: push `GlobalAnimeSearchScreen(query)` then push `DeepLinkAnimeScreen(query)`.
- Empty or missing query: nothing happens and the function still returns `ready = true`.
- The `type` extra is the one written by `DeepLinkAnimeActivity` / `DeepLinkMangaActivity` (§6.1). A user typing into the search (SEARCH_ACTION from Google Assistant) gets no type, so it defaults to ANIME.

### 4.3 OBSERVED: extension-driven search intents
(MainActivity.kt:525-542)
- `INTENT_SEARCH`: query = extra "query"; if non-empty: `filter = extra "filter"`; `popUntilRoot()`; `push(GlobalMangaSearchScreen(query, filter))`. No deep-link screen.
- `INTENT_ANIMESEARCH`: identical, pushing `GlobalAnimeSearchScreen(query, filter)`.
- Constants: `INTENT_SEARCH_QUERY = "query"`, `INTENT_SEARCH_FILTER = "filter"`, `INTENT_SEARCH_TYPE = "type"`.

### 4.4 OBSERVED: ACTION_VIEW
(MainActivity.kt:543-563)
- If `intent.data.toString().endsWith(".tachibk")`: `popUntilRoot()`, `push(RestoreBackupScreen(intent.data.toString()))`. Note the check is on the full URI string, so `content://` URIs ending in `.tachibk` match too.
- Else if `isAddAnimeExtensionStoreIntent()` (scheme `aniyomi` and host `add-repo` or `extension-store`): `intent.data?.getQueryParameter("url")` non-null -> `popUntilRoot()`, `push(AnimeExtensionStoresScreen(repoUrl))`.
- Else if `scheme == "tachiyomi" && host == "add-repo"`: `url` query param -> `popUntilRoot()`, `push(MangaExtensionReposScreen(repoUrl))`.
- Else: nothing.
- Missing `url` query param: nothing happens (no error shown).
- Gap: there is no branch for `tachiyomi://extension-store` or `aniyomi://add-repo` for manga repos. Proposal keeps the same set of routes for compatibility and adds nothing new.

### 4.5 OBSERVED: deep-link trampolines do not reach the VIEW branch
DeepLinkAnimeActivity and DeepLinkMangaActivity forward the same intent (same action) to MainActivity with `type` extra. Their manifest intent filters (app/src/main/AndroidManifest.xml:137-190) are `SEARCH`, `SEARCH_ACTION`, `eu.kanade.tachiyomi.ANIMESEARCH` / `eu.kanade.tachiyomi.SEARCH` and `SEND` with mime `text/plain`. They do NOT register `VIEW`. So a shared URL reaches MainActivity via `ACTION_SEND` with `EXTRA_TEXT` (§4.2).

### 4.6 Proposal: Linux intent sources
| Android source | Linux equivalent |
|---|---|
| Launcher / MAIN | `.desktop` file + `app.activate()` |
| Static shortcuts (`@xml/shortcuts`) | `.desktop` `[Desktop Action X]` entries with `Exec=kurobayomi --action=show-anime` mapped to the same Constants values; `gio::Application` actions `app.show-anime`, `app.show-manga`, `app.show-updates`, `app.show-history`, `app.show-catalogues`, `app.show-anime-extensions`, `app.show-extensions`, `app.show-downloads`, `app.show-anime-downloads` |
| `ACTION_VIEW` file `.tachibk` | `.desktop` `MimeType=application/x-tachibk;` (shared-mime-info glob `*.tachibk`), `app.open(files)` |
| `ACTION_VIEW` `tachiyomi://add-repo?url=`, `aniyomi://add-repo?url=`, `aniyomi://extension-store?url=` | `.desktop` `MimeType=x-scheme-handler/tachiyomi;x-scheme-handler/aniyomi;`, `app.open(uris)` |
| `ACTION_SEND` text/plain | a CLI `--search <text>` argument and a drop target on the Browse page; a `Share` portal target is out of scope |
| `ACTION_SEARCH` / SEARCH_ACTION | `--search <text> [--type anime|manga]`; `Ctrl+K` palette |
| `notificationId` extra | `gio::Notification` `default-action` carries `app.notification-dismiss` with the id; dismiss = `withdraw_notification` |

---

## 5. Toolbars, search and action mode (AppBar.kt)

### 5.1 OBSERVED: SEARCH_DEBOUNCE_MILLIS = 250
(AppBar.kt:65) `const val SEARCH_DEBOUNCE_MILLIS = 250L`. Used by consumers to debounce search filtering. Note: `SearchToolbar` itself does not debounce: it calls `onChangeSearchQuery` on every keystroke (AppBar.kt:320-322). The constant is declared here; its consumers are outside this scope (grep `SEARCH_DEBOUNCE_MILLIS` in areas 37, 40, 41 to confirm the exact use).

### 5.2 OBSERVED: AppBar title and subtitle
(AppBar.kt:166-211) `AppBarTitle(title, modifier, subtitle, count)`:
- If `count > 0`: a row with `Text(title!!, maxLines=1, weight(1f,false), Ellipsis)` followed by a `Pill(text="$count", padding start 4dp, color = onBackground @ 0.12 (dark) / 0.08 (light), fontSize 14sp)`. The `title!!` throws NPE if title is null with count > 0 (edge).
- Else a Column: title (1 line, ellipsis); subtitle in `bodyMedium` with `basicMarquee(repeatDelayMillis = 2000)` so long URLs scroll, 1 line.

### 5.3 OBSERVED: AppBar overloads
- Overload 1 (AppBar.kt:67-114) with `title: String?`, `subtitle`, `actionModeCounter: Int = 0`. `isActionMode = actionModeCounter > 0` (derived). Title becomes `actionModeCounter.toString()` in action mode; actions switch to `actionModeActions`.
- Overload 2 (AppBar.kt:116-164) with `titleContent` slot, `navigateUp: (() -> Unit)?`, `navigationIcon: ImageVector?`. Nav icon: in action mode, `Icons.Outlined.Close` with `MR.strings.action_cancel`; else if `navigateUp != null`, `UpIcon(navigationIcon)`.
- Container colour: `backgroundColor ?: surfaceColorAtElevation(elevation = if (isActionMode) 3.dp else 0.dp)`.

### 5.4 OBSERVED: UpIcon
(AppBar.kt:428-439) default icon `Icons.AutoMirrored.Outlined.ArrowBack`, content description `MR.strings.action_bar_up_description`. Custom icon overrides.

### 5.5 OBSERVED: AppBarActions
(AppBar.kt:214-280)
- For each `AppBar.Action` (title, icon, iconTint, onClick, enabled=true): an icon button with tooltip `title` (plain tooltip), `enabled` respected, tint `iconTint ?: LocalContentColor`.
- All `AppBar.OverflowAction`s go into a single `MoreVert` button (tooltip and description `MR.strings.action_menu_overflow_description`), shown only when there is at least one overflow entry. Clicking toggles `showMenu`. Each item: `DropdownMenuItem(text = title, fontWeight Normal)`, click runs `onClick` then closes.
- Order: all Action buttons (in declaration order) then the overflow button.

### 5.6 OBSERVED: SearchToolbar
(AppBar.kt:288-425)
- Parameters: `searchQuery: String?` (null = search closed), `onChangeSearchQuery`, `searchEnabled = true`, `placeholderText` (default `MR.strings.action_search_hint`), `onSearch`, `onClickCloseSearch` (default sets query null), `actions`, `scrollBehavior`.
- Title content:
  - If `searchQuery == null`: `titleContent()`.
  - Else `BasicTextField`:
    - `singleLine = true`, `ImeAction.Search`.
    - Textstyle: `titleMedium` with `fontWeight Normal`, `fontSize 18sp`.
    - `runOnEnterKeyPressed(searchAndClearFocus)`, `showSoftKeyboard(remember { searchQuery.isEmpty() })` (keyboard shown only when the field starts empty), `clearFocusOnSoftKeyboardHide()`.
    - `searchAndClearFocus`: if `searchQuery.isBlank()` return (no search); else `onSearch(searchQuery)`, clear focus, hide keyboard.
    - Placeholder: `placeholderText ?: MR.strings.action_search_hint`, `secondaryItemAlpha`.
- Nav icon: `if (searchQuery == null) navigateUp else onClickCloseSearch`. When search is open, the nav icon is the up/close slot and calls `onClickCloseSearch` (default `onChangeSearchQuery(null)`).
- Actions (`key("search")`):
  - `searchEnabled == false`: no search action.
  - `searchQuery == null`: `Icons.Outlined.Search` button, tooltip and description `MR.strings.action_search`, click sets `""`.
  - `searchQuery.isNotEmpty()`: `Icons.Outlined.Close` button, tooltip and description `MR.strings.action_reset`, click sets `""` and requests focus back.
  - `searchQuery == ""` (open but empty): no reset button.
- Then the caller's `actions` (`key("actions")`).

### 5.7 OBSERVED: TopAppBar selection and action mode counts
Action mode: the title is the count, the nav icon is Close (cancel), and actions are the caller's `actionModeActions`. Selection behaviour itself is owned by the list areas (37-42).

### 5.8 Proposal: GTK mapping
- `AppBar` -> `gtk::HeaderBar` with `set_title_widget` holding a `gtk::Box` (title label, optional count pill as a CSS-classed label).
- Subtitle marquee: GTK has no marquee; use ellipsis plus a tooltip with the full text, and a `gtk::Label` `set_ellipsize(End)`. Document the drop of the marquee behaviour (§10).
- Search: `gtk::SearchEntry` with `search-changed` (no debounce in the widget; apply 250 ms debounce in the consumer, Proposal) and `activate` (Enter), `stop-search` (Esc) -> `onClickCloseSearch`.
- Keyboard focus: request focus on open when the query is empty (matching `showSoftKeyboard(remember{isEmpty})`).

---

## 6. Deep links and resolve screens

### 6.1 OBSERVED: trampolines
- `DeepLinkAnimeActivity` (app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeActivity.kt:11-21): `flags |= FLAG_ACTIVITY_CLEAR_TOP | FLAG_ACTIVITY_NEW_TASK`; `putExtra("type", "ANIME")`; `setClass(MainActivity)`; `startActivity`; `finish()`.
- `DeepLinkMangaActivity`: identical with `"MANGA"`.
- Manifest: `android:exported="true"`, `launchMode="singleTask"`, `Theme.NoDisplay` (no UI shown), labels `action_global_anime_search` / `action_global_manga_search`.

### 6.2 OBSERVED: DeepLinkAnimeScreen / DeepLinkMangaScreen
(DeepLinkAnimeScreen.kt:26-71; DeepLinkMangaScreen.kt:26-71)
- Scaffold with `AppBar(title = MR.strings.action_search_hint, navigateUp = navigator::pop)`.
- `State.Loading` -> `LoadingScreen(padding)`.
- `State.NoResults` -> `navigator.replace(GlobalAnimeSearchScreen(query))` (or Manga).
- `State.Result`:
  - Anime: `episodeId == null` -> `navigator.replace(AnimeScreen(anime.id, true))`; else `navigator.pop()` then `PlayerActivity.newIntent(context, anime.id, episodeId).also(context::startActivity)`.
  - Manga: `chapterId == null` -> `navigator.replace(MangaScreen(manga.id, true))`; else `navigator.pop()` then `ReaderActivity.newIntent(context, manga.id, chapterId)`.
- The second argument `true` is `fromSource`, which later makes the incognito-off watcher pop this entry (§1.2).
- Gap: the Result branch performs `navigator.replace` / `navigator.pop` and `startActivity` directly in the composition body, not in a `LaunchedEffect`. Recomposition can repeat these side effects. Proposal: move the side effect into a one-shot effect keyed on the resolved state.
- Gap: the anime result routes to `PlayerActivity` directly, bypassing `MainActivity.startPlayerActivity` (so the external-player setting is ignored for deep links). Proposal: route through the same player router (§8.2).

### 6.3 OBSERVED: DeepLinkAnimeScreenModel
(DeepLinkAnimeScreenModel.kt:33-61, 63-76)
1. Starts in `State.Loading`. `init` launches on IO (`screenModelScope.launchIO`).
2. `source` = first of `sourceManager.getAll()` that is `ResolvableAnimeSource` and where `getUriType(query) != UriType.Unknown`. `firstOrNull` (null if none).
3. `anime` = `source?.getAnime(query)?.let { getAnimeFromSAnime(it, source.id) }`.
   - `getAnimeFromSAnime`: `getAnimeByUrlAndSourceId.await(sAnime.url, sourceId) ?: networkToLocalAnime.await(sAnime.toDomainAnime(sourceId))`.
4. `episode` = if `source?.getUriType(query) == UriType.Episode && anime != null`: `source.getEpisode(query)?.let { getEpisodeFromSEpisode(it, anime, source) }`, else null.
   - `getEpisodeFromSEpisode`: `localEpisode = getEpisodeByUrlAndAnimeId.await(sEpisode.url, anime.id)`; if non-null return it; else `updateAnimeFromRemote.awaitEpisodesUpdate(anime, fetchEpisodes = true).getOrElse { return null }.newEpisodes.find { it.url == sEpisode.url }`.
5. `mutableState.update`: `anime == null` -> `NoResults`; else if `episode == null` -> `Result(anime)`; else `Result(anime, episode.id)`.

Notes:
- `getUriType` of `UriType.Episode` requires `getEpisode` to be a second network call. A fetched episode that is not in local DB is inserted via the update path.
- No error state exists. Any exception from `getAnime`, `getEpisode`, or the DB lookups propagates out of `launchIO` and leaves the screen in `Loading` forever (OBSERVED, no try/catch in lines 33-61). Proposal: add `State.Error(msg)` and show a toast.

### 6.4 OBSERVED: DeepLinkMangaScreenModel
(DeepLinkMangaScreenModel.kt:34-62, 64-79)
- Source: `sourceManager.getCatalogueSources()` filtered by `ResolvableSource` and `getUriType(query) != UriType.Unknown`.
- `manga` as in anime, using `getMangaByUrlAndSourceId` / `networkToLocalManga`.
- `chapter` when `UriType.Chapter`: `getChapterFromSChapter`:
  - `localChapter = getChapterByUrlAndMangaId.await(sChapter.url, manga.id)`
  - If null: `sourceChapters = source.getChapterList(manga.toSManga())`, `newChapters = syncChaptersWithSource.await(sourceChapters, manga, source, false)`, return `newChapters.find { it.url == sChapter.url }`. Otherwise return local.
- States identical to anime: `NoResults`, `Result(manga, chapterId?)`.
- Difference from anime: manga does a full chapter-list sync when the chapter is not local, anime fetches episodes through `updateAnimeFromRemote.awaitEpisodesUpdate(fetchEpisodes = true)`. Both are owned by areas 07/08 and 16/24.

### 6.5 Proposal: resolution flow in Rust
- `resolve(query) -> Result<Resolved, ResolveError>` where `Resolved = { entry_id, child_id: Option<i64> }`.
- Runs on tokio. The UI shows a spinner page while running, then navigates. Errors use the "Loading failed" banner with a Retry button (not in the Kotlin, see §10).

---

## 7. Update check and new-version prompt

### 7.1 OBSERVED: CheckForUpdates
(MainActivity.kt:370-404)
- App update: `if (updaterEnabled)` (`BuildConfig.UPDATER_ENABLED`, see area 35). `AppUpdateChecker().checkForUpdate(context)`. If result is `GetApplicationRelease.Result.NewUpdate`, push `NewUpdateScreen(versionName = release.version, changelogInfo = release.info, releaseLink = release.releaseLink, downloadLink = release.downloadLink)`. Exceptions are logged with `logcat(LogPriority.ERROR, e)`.
- Extension updates: `AnimeExtensionApi().checkForUpdates(context)` then `MangaExtensionApi().checkForUpdates(context)`. Both inside one try; failure in the first skips the second.
- Both checks are keyed on `LaunchedEffect(Unit)`, so they run once per composition of `CheckForUpdates()` (the first composition of the navigator content), which in practice is once per launch.
- `AppUpdateChecker.checkForUpdate(context, forceCheck = false)` (AppUpdateChecker.kt:17-34): calls `getApplicationRelease.await(Arguments(isPreviewBuildType, COMMIT_COUNT.toInt(), VERSION_NAME, GITHUB_REPO, forceCheck))`; on `NewUpdate` also calls `AppUpdateNotifier(context).promptUpdate(release)`.
- `GITHUB_REPO`: `"aniyomiorg/aniyomi-preview"` for preview builds, else `"aniyomiorg/aniyomi"`.
- `RELEASE_TAG`: preview -> `"r${COMMIT_COUNT}"`; else `"v${VERSION_NAME}"`.
- `RELEASE_URL = "https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG"`.

### 7.2 OBSERVED: NewUpdateScreen
(app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt:20-42)
- `changelogInfoNoChecksum = changelogInfo.replace("""---(\R|.)*Checksums(\R|.)*""".toRegex(), "")`: removes everything from the first `---` line through a `Checksums` section. Note the regex uses `.` which does not match newlines by default, but `(\R|.)*` covers newlines via `\R`.
- Buttons: open in browser -> `context.openInBrowser(releaseLink)`; reject -> `navigator.pop()`; accept -> `AppUpdateDownloadJob.start(context, url = downloadLink, title = versionName)` then `navigator.pop()`.
- The screen itself (`presentation/more/NewUpdateScreen`) is owned by area 35 for UI copy; this file only wires it.

### 7.3 OBSERVED: Changelog dialog
(MainActivity.kt:294-314) shows once when `didMigration && !BuildConfig.DEBUG`. Title `MR.strings.updated_version` with version name; buttons `MR.strings.whats_new` (opens RELEASE_URL) and `MR.strings.action_ok`. It is not persisted: a second launch will not show it because `didMigration` is false then.

### 7.4 Proposal
- Update check: run at startup on tokio, and then every 24 h while the app runs (no Android JobScheduler). Document the 24 h figure as a Proposal, not an observed value. Pref key proposal: `last_update_check_ms`.
- Prompt via `gio::Notification` plus an in-app page; accept starts a download via the downloader in area 35.

---

## 8. Layout breakpoints and tablet mode

### 8.1 OBSERVED
(DisplayExtensions.kt:12-52)
- `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP = 720`; `isTabletUi() = smallestScreenWidthDp >= 720`.
- `TABLET_UI_MIN_SCREEN_WIDTH_PORTRAIT_DP = 700`; `TABLET_UI_MIN_SCREEN_WIDTH_LANDSCAPE_DP = 600`.
- `prepareTabletUiContext()` overrides the configuration's `smallestScreenWidthDp` so that `isTabletUi()` matches the user's `tablet_ui_mode`:
  - `AUTOMATIC`: expected = `smallestScreenWidthDp >= (700 if portrait else 600)`.
  - `ALWAYS`: true. `LANDSCAPE`: `orientation == LANDSCAPE`. `NEVER`: false.
  - If `isTabletUi() != expected`: if expected, raise `smallestScreenWidthDp` to at least 720; else lower it to at most 719.
- Preference key `tablet_ui_mode`, enum `TabletUiMode` default `AUTOMATIC`. Titles: AUTOMATIC `MR.strings.automatic_background`, ALWAYS `MR.strings.lock_always`, LANDSCAPE `MR.strings.landscape`, NEVER `MR.strings.lock_never`.

### 8.2 Proposal: Linux breakpoints
- Wide: window width >= 760 px (the mockup's existing threshold, `src/main.rs` `sidebar` refresh closure: `win.default_width() >= 760 || win.is_maximized()`). Wide shows a sidebar (`gtk::ListBox`) and no bottom bar.
- Narrow: < 760 px shows the bottom `gtk::StackSwitcher`.
- `tablet_ui_mode` is mapped to a desktop equivalent: AUTOMATIC = width rule; ALWAYS = sidebar always; NEVER = bottom bar always; LANDSCAPE = sidebar when width > height. Keep the key for backup compatibility (area 21/22 backs up prefs).

---

## 9. Theming and base activity

### 9.1 OBSERVED: BaseActivity
(BaseActivity.kt:12-25) Extends `AppCompatActivity`, delegates `SecureActivityDelegate` to `SecureActivityDelegateImpl()` and `ThemingDelegate` to `ThemingDelegateImpl()`. `attachBaseContext` returns `newBase.prepareTabletUiContext()`. `onCreate` calls `applyAppTheme(this)` before `super.onCreate`.

### 9.2 OBSERVED: ThemingDelegate
(ThemingDelegate.kt:13-53)
- `getThemeResIds(appTheme, isAmoled)`: `[themeResources[appTheme] ?: R.style.Theme_Tachiyomi] + (R.style.ThemeOverlay_Tachiyomi_Amoled if isAmoled)`.
- `applyAppTheme(activity)`: reads `UiPreferences.appTheme()` and `themeDarkAmoled()`, calls `activity.setTheme` for each id.
- `themeResources` maps 18 enum values: MONET, COTTONCANDY, GREEN_APPLE, LAVENDER, MIDNIGHT_DUSK, MONOCHROME, MOCHA, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE, YINYANG, YOTSUBA, CLOUDFLARE, SAPPHIRE, DOOM, MATRIX, TIDAL_WAVE. `DEFAULT` and the deprecated `DARK_BLUE`, `HOT_PINK`, `BLUE` are absent, so they fall back to `Theme_Tachiyomi`.
- `appTheme` default: `MONET` if `DeviceUtil.isDynamicColorAvailable` else `DEFAULT`. Key `pref_app_theme`.

### 9.3 OBSERVED: ThemeMode
(UiPreferences.kt:20) key `pref_theme_mode_key`, enum `ThemeMode { LIGHT, DARK, SYSTEM }` default SYSTEM. `setAppCompatDelegateThemeMode`: LIGHT -> `MODE_NIGHT_NO`, DARK -> `MODE_NIGHT_YES`, SYSTEM -> `MODE_NIGHT_FOLLOW_SYSTEM`.

### 9.4 Proposal: theme on Linux
- The user's GTK theme applies (vanilla GTK, no libadwaita). Kurobayomi ships no colour palette of its own beyond a small CSS file that uses the GTK named colours (`@window_bg_color`, `@accent_bg_color`, `@view_fg_color`) so it follows the user's theme.
- `ThemeMode` -> `gtk::Settings::default().set_gtk_application_prefer_dark_theme(bool)` for LIGHT/DARK; SYSTEM follows the freedesktop `org.freedesktop.appearance color-scheme` via `ashpd` (`Settings` portal), falling back to the GTK default.
- `AppTheme` (19 values) is kept as a preference for backup compatibility. On Linux the visual theme is not applied (the GTK theme wins), and the Settings screen (area 47) should show a note. Proposal: map `AMOLED` to a CSS class `kuro-amoled` (true black background on dark).
- `MONET` / dynamic colour has no Linux meaning: when it is selected, use `DEFAULT`.

---

## 10. Android-only behaviour: decisions

| Behaviour (OBSERVED) | Path | Decision | User loses |
|---|---|---|---|
| FLAG_SECURE via `Window.setSecureScreen` for `secure_screen_v2` ALWAYS/INCOGNITO/NEVER | eu/kanade/tachiyomi/util/view/WindowExtensions.kt | Drop (no Wayland/X11 API to block screen capture for a normal client) | Screenshot / screen-share protection |
| Edge-to-edge system bar styling | MainActivity.kt:190-198 | Drop; GTK draws the title bar | None |
| Navigation bar scrim | DisplayExtensions.kt:70-73 | Drop | None |
| Splash screen and exit animation | MainActivity.kt:162, 318-322, 424-458 | Replaced by a spinner window (§1.6) | Splash art |
| `onProvideAssistContent` | MainActivity.kt:348-355, WebViewActivity.kt:93-96 | Drop | Android assistant page context |
| `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` onboarding row | PermissionStep.kt:100-111 | Drop; no Linux battery-optimisation concept | None |
| `ACTION_MANAGE_UNKNOWN_APP_SOURCES` onboarding row | PermissionStep.kt:80-82 | Drop | None (no APK install) |
| `POST_NOTIFICATIONS` runtime permission row | PermissionStep.kt:85-98 | Replace: desktop notifications need no permission; show status "Notifications: available" | None |
| SAF storage picker (`SettingsDataScreen.storageLocationPicker`) | StorageStep.kt:81-93 | Replace with `gtk::FileDialog` (portal `OpenURI`/`FileChooser`) | None |
| TV-box folder creation branch | StorageStep.kt:65-79 | Drop | TV-box support |
| Incognito IME flag (`IME_FLAG_NO_PERSONALIZED_LEARNING`) | TachiyomiTextInputEditText.kt | Drop | Keyboard learning suppression in text fields |
| NumberPicker / SeekBar / Spinner / ViewPager widgets | widget/*.kt | Drop (replaced by GTK widgets in shared UI) | None |
| Circular reveal animation | RevealAnimationView.kt | Drop | Decorative animation |
| `isNavigationBarNeedsScrim` | DisplayExtensions.kt | Drop | None |
| Biometric prompt (`BiometricPrompt`, `Authenticators.BIOMETRIC_WEAK|DEVICE_CREDENTIAL`) | AuthenticatorUtil.kt, UnlockActivity.kt | Replace (§11.3) | Android fingerprint/face; device-credential reuse |
| `Intent.ACTION_SEND` share sheet | WebViewScreenModel.kt:47-53, WebViewActivity.kt:112-118, `toShareIntent` | Replace: `gio::AppInfo` / portal share is not reliable; copy URL to clipboard and show toast `MR.strings.copied_to_clipboard` (confirm key exists in area 51) | Share targets |
| WebView inspector (`WebView.setWebContentsDebuggingEnabled`) | WebViewScreenContent.kt:216-221 | Replace: WebKitGTK inspector via `WEBKIT_INSPECTOR_SERVER` env / `set_enable_developer_extras(true)` in debug builds | `chrome://inspect` workflow |
| `FLAG_ACTIVITY_CLEAR_TOP` / `singleTop` / activity transitions | several | Drop; GTK window stacking | None |

---

## 11. Security / app lock

### 11.1 OBSERVED: state (SecureActivityDelegate.kt:23-72)
Static state (companion object, process lifetime):
- `requireUnlock: Boolean = true`. Comment: "Always require unlock if app is killed."

Preferences (see p1-data-and-config.md for keys):
- `use_biometric_lock` (Boolean, false) = `useAuthenticator`.
- `lock_app_after` (Int, 0) = `lockAppAfter` minutes: -1 never, 0 always, >0 minutes.
- `__APP_STATE_last_app_closed` (Long, 0) = `lastAppClosed` millis; a "pending timed lock" marker, deleted on each start.

### 11.2 OBSERVED: onApplicationStopped
(lines 34-46)
```
if !useAuthenticator: return
if !AuthenticatorUtil.isAuthenticating:
   if requireUnlock: return            // closed while locked: stay locked
   if lockAppAfter > 0:
       lastAppClosed = now_ms()
```
Caller: outside this scope (grep `onApplicationStopped`; expected in the application lifecycle observer, area 01 / 53). Note that `lastAppClosed` is not written when `lockAppAfter` is 0 or -1.

### 11.3 OBSERVED: onApplicationStart
(lines 51-67)
```
if !useAuthenticator: return
lastClosedPref = lastAppClosed
if !AuthenticatorUtil.isAuthenticating && !requireUnlock:
    requireUnlock = when (lockDelay = lockAppAfter):
        -1 -> false
         0 -> true
        else -> lastClosed + lockDelay * 60_000 <= now_ms()
lastClosedPref.delete()           // always, after the block
```
- `lockDelay * 60_000` is in ms: minutes -> ms.
- Note: `lastClosed` defaults to 0 when never written, so with `lockDelay > 0` and no stored close time, the result is `0 + delay <= now` which is true, so the app locks.
- Note: `requireUnlock` is only *set* to true here, never set to false (except `unlock()`). If it is already true, it stays true.

### 11.4 OBSERVED: unlock
`unlock()` sets `requireUnlock = false` (line 69-71). Called from `UnlockActivity` on success.

### 11.5 OBSERVED: setAppLock (on each resume)
(lines 107-121)
```
if !useAuthenticator: return
if activity.isAuthenticationSupported():     // BIOMETRIC_WEAK | DEVICE_CREDENTIAL canAuthenticate == SUCCESS
    if !requireUnlock: return
    startActivity(UnlockActivity)           // transition override (0,0): no animation
else:
    useAuthenticator = false                // silently disables the lock
```
- Edge: `onResume` runs every time the activity returns to the foreground. While `requireUnlock` stays true (the user has not yet unlocked), each resume starts another `UnlockActivity`. Proposal: guard with an "unlock in progress" flag.
- Edge: `isAuthenticating` is not checked in `setAppLock`, so the guard in §11.2 and §11.3 is the only one.
- Edge: if the device credential is removed, the lock turns itself off with no message.

### 11.6 OBSERVED: AuthenticatorUtil
(app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt)
- `isAuthenticating` (global var, false): set true in `startAuthentication`, set false in the callback's `onAuthenticationError` and `onAuthenticationSucceeded`.
- `startAuthentication(title, subtitle?, confirmationRequired = true, callback)`: `startClass2BiometricOrCredentialAuthentication` on the main executor.
- `authenticate(title, subtitle = MR.strings.confirm_lock_change)` suspend: resolves `true` immediately if unsupported; else resolves `true` on success, `false` on error after `activity?.toast(errString)`.
- `isAuthenticationSupported()`: `BIOMETRIC_WEAK | DEVICE_CREDENTIAL` canAuthenticate == `BIOMETRIC_SUCCESS`.

### 11.7 OBSERVED: UnlockActivity
(UnlockActivity.kt:18-47)
- `startAuthentication(title = stringResource(MR.strings.unlock_app_title, app_name), confirmationRequired = false, callback)`.
- `onAuthenticationError`: log, then `finishAffinity()`. **Cancelling the prompt closes the whole app** (OBSERVED).
- `onAuthenticationSucceeded`: `SecureActivityDelegate.unlock()`, `finish()`.
- `UnlockActivity` has no `registerSecureActivity` call, so the lock screen itself is not FLAG_SECURE.

### 11.8 Proposal: Linux lock
- Backend: a PIN or passphrase verified with Argon2id (crate `argon2`), stored as a PHC string in `~/.config/kurobayomi/lock.json` (file mode 0600). Lock time logic: §11.2 to §11.4 unchanged.
- Optional: `fprintd` via D-Bus (`net.reactivated.Fprint`, crate `zbus`) when available. Fallback to the passphrase if fprintd is absent or after 5 failed verifies.
- Cancel on the unlock window closes the window and returns to the lock screen (it does not quit the app). Intentional change from §11.7; document in the release notes.
- Migration: `use_biometric_lock` cannot be reused because there is no device credential on Linux. On first run with `use_biometric_lock = true`, the lock is disabled and the user is shown a one-time banner to set a passphrase (see p1-topic-startup-lock-security.md).

---

## 12. WebView

### 12.1 OBSERVED: WebViewActivity
(app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt:42-127)
- Transitions: on API 34+ (`UPSIDE_DOWN_CAKE`) `overrideActivityTransition(OPEN, shared_axis_x_push_enter, shared_axis_x_push_exit)`; older: `overridePendingTransition`. Close: `shared_axis_x_pop_*`. Cosmetic.
- `WebViewUtil.supportsWebView(this)` false -> `toast(MR.strings.information_webview_required, LONG)` and `finish()`.
- `url = intent.extras?.getString("url_key") ?: return`. A missing URL returns without content (blank window). `assistUrl = url`.
- `headers`: start empty. Source lookup by `SOURCE_KEY` (`getLong`): `(sourceManager.get(id) as? HttpSource)?.headers`, then `(animeSourceManager.get(id) as? AnimeHttpSource)?.headers`; each becomes `headers.toMultimap().mapValues { it.value.getOrNull(0) ?: "" }`. The anime lookup runs second and overwrites. Errors logged `"Failed to build headers"`.
- `setComposeContent { WebViewScreenContent(onNavigateUp = finish, initialTitle = TITLE_KEY, url, headers, onUrlChange = assistUrl = it, onShare = shareWebpage, onOpenInBrowser = openInBrowser(forceDefault=true), onClearCookies = clearCookies) }`.
- `clearCookies(url)`: `network.cookieJar.remove(url.toHttpUrl())` (throws on invalid URL, unlike the Screen variant), logs `"Cleared $cleared cookies for: $url"`.
- `newIntent(context, url, sourceId = null, title = null, isAnime = false)`: `FLAG_ACTIVITY_CLEAR_TOP`; extras `url_key`, `source_key` (Long?), `title_key`, `anime_key` (never read on the receiving side).

### 12.2 OBSERVED: WebViewScreen and model
(WebViewScreen.kt:12-38; WebViewScreenModel.kt:21-64)
- `WebViewScreen(url, initialTitle, sourceId)` is a Voyager screen with the same content. `onNavigateUp = navigator.pop()`. `onClearCookies = screenModel::clearCookies`.
- `WebViewScreenModel` extends `StateScreenModel<StatsScreenState>(Loading)`: the state type is copied from the stats area, and is never read. Proposal: drop the unused state.
- `init` builds headers the same way as the activity, but uses `as? HttpSource` first then `as? AnimeHttpSource` on the sourceId.
- `clearCookies` uses `toHttpUrlOrNull()` (safe). Mismatch with the activity (§12.1) is recorded as a defect to fix on Linux by using the safe version everywhere.
- `shareWebpage` and `openInBrowser` are the same as §12.1; `openInBrowser` uses `forceDefaultBrowser = true`.

### 12.3 OBSERVED: WebViewScreenContent
(presentation/webview/WebViewScreenContent.kt:48-230)

State:
- `state = rememberWebViewState(url, additionalHttpHeaders = headers)`; `navigator = rememberWebViewNavigator()`.
- `currentUrl` (init url); `showCloudflareHelp` (false).

WebClient (an `AccompanistWebViewClient` subclass):
- `onPageStarted(url)`: `currentUrl = url`; `onUrlChange(url)`.
- `onPageFinished`: `scope.launch { html = view.getHtml(); showCloudflareHelp = "window._cf_chl_opt" in html || "Ray ID is" in html }`. `getHtml` = `evaluateJavascript("document.documentElement.outerHTML")` suspend (WebViewUtil.kt tail).
- `doUpdateVisitedHistory(url, isReload)`: `currentUrl = url`; `onUrlChange(url)`.
- `shouldOverrideUrlLoading(view, request)`:
  - If `request` null: return `super` (default).
  - If `request.url` starts with `blob:http`: return false (WebView loads it).
  - If starts with `intent://`: return true (blocked; no external intent launched).
  - Else `view.loadUrl(request.url, headers)` (re-issue with source headers), then return `super.shouldOverrideUrlLoading(view, request)`.
  - Edge: this reloads each navigation request with headers, so every link is requested twice. Observed; Proposal: set headers once on the request in `decide-policy` instead.

AppBar (lines 120-171):
- Title: `state.pageTitle ?: initialTitle`. Subtitle: `currentUrl` (marquee per §5.2).
- Nav icon `Icons.Outlined.Close`, action `onNavigateUp`.
- Actions (bar, not overflow): Back (`ArrowBack`, title `MR.strings.action_webview_back`, enabled `navigator.canGoBack`, click navigates back only if `canGoBack`); Forward (`ArrowForward`, `MR.strings.action_webview_forward`, `canGoForward`).
- Overflow in order: Refresh (`MR.strings.action_webview_refresh` -> `navigator.reload()`), Share (`MR.strings.action_share` -> `onShare(currentUrl)`), Open in browser (`MR.strings.action_open_in_browser` -> `onOpenInBrowser(currentUrl)`), Clear cookies (`MR.strings.pref_clear_cookies` -> `onClearCookies(currentUrl)`).

Cloudflare banner (lines 173-188):
- Shown when `showCloudflareHelp`. Inside a `Surface` with 8 dp padding, `WarningBanner(MR.strings.information_cloudflare_help)` with `clip(MaterialTheme.shapes.small)`, click opens `https://aniyomi.org/docs/guides/troubleshooting/#cloudflare`.

Progress (lines 190-203): `Initializing` -> indeterminate `LinearProgressIndicator` at bottom of the top bar; `Loading` -> determinate with `progress ?: 1f`; other states -> none.

WebView (lines 206-229):
- `onCreated`: `setDefaultSettings()`; if `BuildConfig.DEBUG` and the app is debuggable (`FLAG_DEBUGGABLE` check; note the expression `0 != webView.context.applicationInfo.flags and ApplicationInfo.FLAG_DEBUGGABLE`), enable web contents debugging; `headers["user-agent"]` (lower-case key) -> `settings.userAgentString`.

### 12.4 OBSERVED: setDefaultSettings
(WebViewUtil.kt tail, lines after `getHtml`) `javaScriptEnabled = true`, `domStorageEnabled = true`, `useWideViewPort = true`, `loadWithOverviewMode = true`, `cacheMode = LOAD_DEFAULT`, `setSupportZoom(true)`, `builtInZoomControls = true`, `displayZoomControls = false`.

`MINIMUM_WEBVIEW_VERSION = 118`; `WebView.isOutdated()` compares the major version to 118 (used by area 12, not here).

### 12.5 Proposal: WebKitGTK
- Engine: WebKitGTK 6.0 via crate `webkit6` (GTK 4 build). Package on CachyOS: `webkit2gtk-6.0`.
- Settings: same five flags. `enable-javascript`, `enable-developer-extras` only in debug builds, `enable-smooth-scrolling`.
- Cookie bridge: `WebKitCookieManager` with a persistent cookie store at `$XDG_DATA_HOME/kurobayomi/webview/cookies.sqlite`, and the same host-based removal as `cookieJar.remove(url)`. Shared with the HTTP client cookie jar (area 12) through a single cookie-sync function.
- Navigation policy: `decide-policy` signal implementing §12.3 rules (blob, intent:// blocked, custom headers via `NavigationAction` request header set).
- Title from `title` property; progress from `estimated-load-progress`; Cloudflare check with `run_javascript` returning `document.documentElement.outerHTML` substring test.
- Share -> copy the URL to the clipboard and toast. Open in browser -> `gio::AppInfo::launch_default_for_uri`.

---

## 13. Onboarding (4-step wizard)

### 13.1 OBSERVED: host
(presentation/more/onboarding/OnboardingScreen.kt:27-88; ui/more/OnboardingScreen.kt:20-52)
- `steps = [ThemeStep(), StorageStep(), PermissionStep(), GuidesStep(onRestoreBackup)]` (remembered).
- `currentStep` (rememberSaveable, 0). `isLastStep = currentStep == steps.lastIndex` (index 3).
- `BackHandler(enabled = currentStep != 0) { currentStep-- }`.
- `InfoScreen(icon = RocketLaunch, headingText = MR.strings.onboarding_heading, subtitleText = MR.strings.onboarding_description, acceptText = if last then MR.strings.onboarding_action_finish else MR.strings.onboarding_action_next, canAccept = steps[currentStep].isComplete, onAcceptClick)`.
- Accept: last -> `onComplete()`; else `currentStep++`.
- Content: a `surfaceVariant` rounded box with `materialSharedAxisX` between steps, direction forward when target > initial.
- The outer wrapper (ui/more/OnboardingScreen.kt): `BackHandler(enabled = !shownOnboardingFlow) {}` (back is swallowed until finished). `finishOnboarding()` = `shownOnboardingFlow.set(true)` then `navigator.pop()`.
- `onRestoreBackup`: `finishOnboarding()`, `SearchableSettings.highlightKey = stringResource(SettingsDataScreen.restorePreferenceKeyString)` (= `MR.strings.label_backup`), `navigator.push(SettingsScreen(SettingsScreen.Destination.DataAndStorage))`.

Gate (MainActivity.kt:407-415): `if (!shownOnboardingFlow && navigator.lastItem !is OnboardingScreen) push(OnboardingScreen())`. Keyed on `LaunchedEffect(Unit)`, so it runs on the first composition of the navigator content; it is shown until completed. Key `__APP_STATE_onboarding_complete` (Boolean, false).

### 13.2 OBSERVED: step 1, ThemeStep
(ThemeStep.kt:14-47) `isComplete = true`. Shows `AppThemeModePreferenceWidget(themeMode)` (click -> `themeMode.set(it)` and `setAppCompatDelegateThemeMode(it)`) and `AppThemePreferenceWidget(appTheme, amoled)` (click -> `appTheme.set(it)`). Both widgets live in `presentation/more/settings/widget/` (area 47).

### 13.3 OBSERVED: step 2, StorageStep
(StorageStep.kt:34-114)
- `storagePref = StoragePreferences.baseStorageDirectory()`; `folderProvider = AndroidStorageFolderProvider`.
- `_isComplete` (false) is updated from `storagePref.changes().collectLatest { _isComplete = storagePref.isSet() }`. **Next is disabled until a folder is set.**
- Text: `MR.strings.onboarding_storage_info` with args `(app_name, SettingsDataScreen.storageLocationText(storagePref))`.
- Non-TV: button `MR.strings.onboarding_storage_action_select` -> `SettingsDataScreen.storageLocationPicker(storagePref).launch(null)`; `ActivityNotFoundException` -> toast `MR.strings.file_picker_error`.
- TV box (`isTvBox`): if pref not set, button `AYMR.strings.onboarding_storage_action_create_folder` creates `folderProvider.directory()` (mkdirs if missing) and sets the pref to its current value.
- Help: divider, `MR.strings.onboarding_storage_help_info` (args app_name), button `MR.strings.onboarding_storage_help_action` -> open `SettingsDataScreen.HELP_URL` = `https://aniyomi.org/docs/faq/storage`.

### 13.4 OBSERVED: step 3, PermissionStep
(PermissionStep.kt:42-160)
- `isComplete = true` (never blocks).
- Live state from `ON_RESUME`: `notificationGranted` = `checkSelfPermission(POST_NOTIFICATIONS) == GRANTED` on API 33+ else `true`; `batteryGranted` = `PowerManager.isIgnoringBatteryOptimizations(packageName)`.
- Rows: (1) "install apps" via `launchRequestPackageInstallsPermission()` (`ACTION_MANAGE_UNKNOWN_APP_SOURCES`), state from `rememberRequestPackageInstallsPermissionState()`; (2) notifications (API 33+ only) via `RequestPermission` for `POST_NOTIFICATIONS`; (3) battery optimisation via `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` with data `package:<pkg>`.
- Each row is a `ListItem` with an `OutlinedButton` that is disabled when granted and shows a check icon.
- String keys: `MR.strings.onboarding_permission_install_apps` (+ `_description`), `MR.strings.onboarding_permission_notifications` (+ `_description`), `MR.strings.onboarding_permission_ignore_battery_opts` (+ `_description`), `MR.strings.onboarding_permission_action_grant`.

### 13.5 OBSERVED: step 4, GuidesStep
(GuidesStep.kt:21-69) `isComplete = true`. Text `MR.strings.onboarding_guides_new_user` (arg app_name) + button `MR.strings.getting_started_guide` -> `https://aniyomi.org/docs/guides/getting-started`. Divider. Text `MR.strings.onboarding_guides_returning_user` + button `MR.strings.pref_restore_backup` -> `onRestoreBackup`.

### 13.6 Proposal: Linux onboarding
- Same four steps and order. Step 3 becomes "Desktop" with: notifications status (informational), and an "Open background/autostart" note (no action). Battery and install rows dropped (§10).
- Step 2 uses `gtk::FileDialog::select_folder`. Default path shown: `$XDG_DATA_HOME/kurobayomi`. TV-box branch dropped.
- The restore button selects `DataAndStorage` settings and highlights the restore row (`highlightKey`), as today.
- Back is blocked until finish, as today. Escape on step 1 is blocked too, so the user must finish or close the window (window close does not set `onboarding_complete`).

---

## 14. Shared widgets (non-visual logic only)

### 14.1 OBSERVED: relative date text (DateText.kt)
- `relativeDateText(epochMillis)`: `LocalDate.ofInstant(Instant.ofEpochMilli(x), ZoneId.systemDefault()).takeIf { x > 0L }`. `x <= 0` -> `null` -> `MR.strings.not_applicable`.
- `relativeDateTimeText(epochMillis)`: same with `LocalDateTime`.
- Both pass `relative = UiPreferences.relativeTime()` (key `relative_time_v2`, default true) and `dateFormat = UiPreferences.dateFormat(app_date_format)`.
- `dateFormat("")` -> `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`; any other string -> `ofPattern(format, Locale.getDefault())`. An invalid pattern throws IllegalArgumentException (no catch in this file; edge).
- The actual relative-string rules live in `eu.kanade.tachiyomi.util.lang.toRelativeString` (area 52). Not reproduced here.

### 14.2 OBSERVED: DropdownMenu
(DropdownMenu.kt:33-51) default `offset = DpOffset(8.dp, (-56).dp)`, `properties = PopupProperties(focusable = true)`, `sizeIn(minWidth = 196.dp, maxWidth = 196.dp)`.
- `RadioMenuItem(text, isChecked, onClick)`: trailing icon `RadioButtonChecked` (tint primary, description `MR.strings.selected`) or `RadioButtonUnchecked` (description `MR.strings.not_selected`).
- `NestedMenuItem(text, children)`: a row with `ArrowRight` trailing icon; click opens a nested `DropdownMenu`; `children(closeMenu)` gets a function that closes the nested menu.

### 14.3 OBSERVED: EntryDownloadDropdownMenu
(EntryDownloadDropdownMenu.kt:14-47) Options in order:
1. `NEXT_1_ITEM` -> plural `downloadAmount` (1)
2. `NEXT_5_ITEMS` -> plural (5)
3. `NEXT_10_ITEMS` -> plural (10)
4. `NEXT_25_ITEMS` -> plural (25)
5. `UNVIEWED_ITEMS` -> `download_unread` (manga) / `download_unseen` (anime)

`downloadAmount` = `MR.plurals.download_amount` (manga) or `AYMR.plurals.download_amount_anime`. Click calls `onDownloadClicked(action)` then `onDismissRequest()`. The `DownloadAction` enum is owned by area 23/24.

### 14.4 OBSERVED: FloatingActionAddButton
(FloatingActionAddButton.kt:16-28) Extended FAB with `Icons.Outlined.Add`, text `MR.strings.action_add`. `expanded = lazyListState.shouldExpandFAB()`. `shouldExpandFAB` (LazyListState.kt:10-19): expanded when `(firstVisibleItemIndex == 0 && firstVisibleItemScrollOffset == 0) || lastScrolledBackward || !canScrollForward`. So it is collapsed while the user scrolls forward and expands at the top, at the end, or on any backward scroll.

### 14.5 OBSERVED: download indicator constants
(ItemDownloadIndicator.kt:36-46) `IndicatorSize = 26 dp`; `IndicatorPadding = 2 dp`; `IndicatorStrokeWidth = IndicatorPadding` (2 dp); `IndicatorModifier = size(26) + padding(2)`; `ArrowModifier = size(26 - 7 = 19 dp)`. `commonClickable` combines click and long-click with `Role.Button`, ripple unbounded with radius `IconButtonTokens.StateLayerSize / 2`, and a `LongPress` haptic on long click. `preferences: DownloadPreferences` injected lazily (unused in this file beyond the declaration).

### 14.6 OBSERVED: TabbedDialog and TabbedScreen
- TabbedDialog: `AdaptiveSheet` with a `Row { PrimaryTabRow(weight 1, containerColor surfaceContainerHigh, divider = {}) ; MoreMenu }`, a `HorizontalDivider`, and a `HorizontalPager` with `animateContentSize`. `TabbedDialogPaddings` Horizontal 24 dp, Vertical 8 dp.
- `MoreMenu`: returns when both callbacks are null; if `onClickIcon` is given the icon calls it directly; otherwise a `DropdownMenu` opens with `content(closeMenu)`. Icon `overflowIcon ?: MoreVert`, description `MR.strings.label_more`.
- TabbedScreen: `Scaffold` with `SearchToolbar` when `titleRes != null`. Search parity: `state.currentPage % 2` selects `mangaSearchQuery`/`onChangeMangaSearchQuery` when 1, `animeSearchQuery`/`onChangeAnimeSearchQuery` otherwise. **This depends on tabs being interleaved (anime at even indexes, manga at odd).** Proposal: pass the tab's own query explicitly in `TabContent`.
- `searchEnabled` false -> `searchQuery = null` (no search field).
- Tab row: `FlexibleTabRow(scrollable)`: `ScrollableTabRow(edgePadding = 13.dp, zIndex 1)` when scrollable, else `PrimaryTabRow(zIndex 1)`. Each tab: `TabText(text, badgeCount = tab.badgeNumber)`.
- `TabContent(titleRes, badgeNumber = null, searchEnabled = false, actions = [], content, numberTitle = 0, cancelAction = {}, navigateUp = null)`.
- Title: `AppBarTitle(stringResource(titleRes), null, tab.numberTitle)` (count pill when `numberTitle > 0`).

### 14.7 OBSERVED: EmptyScreen and Banners
- `EmptyScreen` is in `tachiyomi.presentation.core.screens` (another area). The file here only has previews.
- `WarningBanner(textRes)`: text centered, `error` background, `onError` text, `bodyMedium`, padding 16 dp.
- `AppStateBanners` (Banners.kt:61-125): a `SubcomposeLayout` stacking three banners in this fixed order: indexing (top), downloaded-only, incognito. Each uses `AnimatedVisibility(expandVertically / shrinkVertically)`. Their top inset is `max(0, statusBarTop - sum(heights of banners above))`, so the banners sit under the status bar without double padding. Heights are measured with `subcompose`.
- Text keys: downloaded-only `MR.strings.label_downloaded_only`; incognito `MR.strings.pref_incognito_mode`; indexing `MR.strings.download_notifier_cache_renewal` (with a `CircularProgressIndicator` whose size is the text height and stroke `height/8`).
- Colours: downloaded-only `tertiary`/`onTertiary`; incognito `primary`/`onPrimary`; indexing `secondary`/`onSecondary`. Labels use `labelMedium`, centered, 4 dp padding (indexing 8 dp).

### 14.8 OBSERVED: commonClickable haptics
See 14.5. Long-press fires `onLongClick` then `HapticFeedbackType.LongPress`.

### 14.9 OBSERVED: text input
`TachiyomiTextInputEditText` sets `IME_FLAG_NO_PERSONALIZED_LEARNING` when `BasePreferences.incognitoMode()` is true, by collecting changes in a `Dispatchers.Main` scope tied to attach/detach. Dropped on Linux (§10).

---

## 15. Concurrency and dispatchers (OBSERVED summary)

| Work | Dispatcher / scope | File |
|---|---|---|
| Migration wait | `runBlocking` on the main thread (blocks onCreate) | MainActivity.kt:166 |
| Intent router | `lifecycleScope` (main), `HomeScreen.openTab` suspends on channel | MainActivity.kt:568 |
| Splash keep-condition | main thread, polled | MainActivity.kt:318 |
| Auto-clear cache | `lifecycleScope.launchIO` | MainActivity.kt:325 |
| External-player result | `runBlocking` (main) | MainActivity.kt:338 |
| Update check | `LaunchedEffect` (main), inner `withIOContext` | MainActivity.kt:376; AppUpdateChecker.kt:23 |
| Deep-link resolve | `screenModelScope.launchIO` | DeepLinkAnimeScreenModel.kt:34; DeepLinkMangaScreenModel.kt:35 |
| Source headers | synchronous in `init` (WebViewActivity on main; model init on main) | WebViewScreenModel.kt:30-45 |
| Cookie removal | synchronous on caller | WebViewScreenModel.kt:59-64 |
| Page HTML for Cloudflare check | `rememberCoroutineScope` (main) with suspend `getHtml` | WebViewScreenContent.kt:79-82 |
| Lock state | main thread (`onResume`, lifecycle), prefs read synchronously | SecureActivityDelegate.kt:91-93 |
| Secure-screen flow | `lifecycleScope` (main) | SecureActivityDelegate.kt:96-104 |

Proposal for Rust:
- GTK main thread: all widget updates, navigation, lock prompts.
- tokio multi-thread runtime: migrations, update checks, deep-link resolve, cookie IO, source calls. Results come back via `glib::MainContext::channel` or `async-channel` + `glib::spawn_future_local`.
- Never block the GTK thread on `runBlocking`. The migration gate becomes an awaited future before the window is presented (§1.5).

---

## 16. Error handling and messages (observed keys)

| Key | Where shown | Trigger |
|---|---|---|
| `MR.strings.updated_version` (with version) | changelog dialog title | upgrade |
| `MR.strings.whats_new` | changelog button | — |
| `MR.strings.action_ok` | changelog button, generic | — |
| `AYMR.strings.http_server_start_failure` | toast | external player HTTP server failed to start |
| `MR.strings.information_webview_required` | toast (long) | WebView not available |
| `MR.strings.file_picker_error` | toast | no file picker activity |
| `MR.strings.unlock_app_title` (arg app_name) | biometric prompt title | lock |
| `MR.strings.confirm_lock_change` | prompt subtitle (`authenticate`) | settings change |
| `MR.strings.information_cloudflare_help` | banner | Cloudflare challenge detected |
| `MR.strings.not_applicable` | date text | timestamp <= 0 |
| `MR.strings.action_search_hint` | placeholder; deep-link screen title | — |
| `MR.strings.action_menu_overflow_description` | overflow button description | — |
| `MR.strings.action_bar_up_description` | up icon description | — |
| `MR.strings.action_cancel` | close icon in action mode | — |
| `MR.strings.action_reset` | clear search | — |
| `MR.strings.action_search` | search icon | — |
| `MR.strings.selected` / `MR.strings.not_selected` | radio icons | — |
| `MR.strings.label_more` | more icon | — |
| `MR.strings.label_downloaded_only` | banner | downloaded-only mode |
| `MR.strings.pref_incognito_mode` | banner | incognito |
| `MR.strings.download_notifier_cache_renewal` | banner | download cache indexing |
| `MR.plurals.notification_chapters_generic` | badge description | updates count |
| `MR.plurals.update_check_notification_ext_updates` | badge description | extension updates |
| `MR.strings.onboarding_heading`, `onboarding_description`, `onboarding_action_next`, `onboarding_action_finish` | onboarding | flow |
| `MR.strings.onboarding_storage_info` / `onboarding_storage_help_info` / `onboarding_storage_help_action` / `onboarding_storage_action_select` | storage step | — |
| `AYMR.strings.onboarding_storage_action_create_folder` | TV box only | — |
| `MR.strings.onboarding_guides_new_user` / `onboarding_guides_returning_user` / `getting_started_guide` / `pref_restore_backup` | guides step | — |
| `MR.strings.onboarding_permission_*` | permission step | — |
| `MR.strings.pref_clear_cookies`, `action_share`, `action_open_in_browser`, `action_webview_back`, `action_webview_forward`, `action_webview_refresh` | WebView overflow / bar | — |
| `MR.strings.label_backup` | restore highlight key | onboarding restore |
| `MR.strings.lock_always`, `pref_incognito_mode`, `lock_never` | SecureScreenMode titles (SecurityPreferences.kt) | settings |
| `MR.strings.automatic_background`, `landscape` | TabletUiMode titles | settings |
| `AYMR.strings.pref_bottom_nav_no_manga` / `_no_updates` / `_no_history` / `_no_browse` | NavStyle titles | settings |
| `AYMR.strings.label_anime`, `AYMR.strings.manga`, `MR.strings.label_recent_updates`, `MR.strings.label_recent_manga`, `MR.strings.browse` | StartScreen titles | settings |
| `MR.strings.label_default`, `theme_*` / `AYMR.strings.theme_*` | AppTheme titles | settings |
| `MR.strings.download_amount` / `AYMR.strings.download_amount_anime` / `download_unread` / `AYMR.strings.download_unseen` | download menu | entry |

Error paths without UI (OBSERVED):
- `handleIntentAction` `DeepLinkScreenType.valueOf` on unknown `type`: uncaught exception (crash).
- `WebViewActivity.clearCookies` with an invalid URL: uncaught (`toHttpUrl`).
- Deep-link model exceptions: stuck on loading (§6.3).
- `AuthenticatorUtil` error callbacks: silent except the toast in `authenticate` (not in the lock path, where the app exits).

---

## 17. Preference keys read or written here
See `p1-data-and-config.md` section 4 for the full table with defaults and types.

## 18. Interactions with other areas (summary)
- area:02-preferences-system: all preference getters.
- area:11-app-migrations: `Migrator` gate and `didMigration`.
- area:35-data-misc-updater-notification-export: app-update check, download job, notification dismissal.
- area:37-ui-library, area:38-ui-entry-details-manga, area:39-ui-entry-details-anime, area:40-ui-browse-manga, area:41-ui-browse-anime: `AnimeLibraryTab.search`, `MangaLibraryTab.search`, `BrowseTab.showExtension*`, `AnimeScreen`, `MangaScreen`, global search screens.
- area:42-ui-updates-history-stats-storage-upcoming: `UpdatesTab`, `HistoriesTab`, `StatsScreen`, storage screen.
- area:43-reader-core and area:44-reader-viewers; area:45-player-core and area:46-player-controls-ui: `ReaderActivity.newIntent`, `PlayerActivity.newIntent`, `ExternalIntents`, `HttpServerService`.
- area:12-network-stack: cookie jar, `WebViewUtil`, `NetworkHelper`.
- area:22-backup-restore: `RestoreBackupScreen`, backup of preferences (keys in this area are part of the backup set).
- area:47-settings-screens-general and area:48-settings-player-and-scripts settings: `SettingsScreen`, `SearchableSettings`.
- area:49-theme-and-design-system theme: visual palette, `AppTheme` look-up.
- area:53-platform-manifest-intents-workers manifest/intents: intent filters, shortcuts XML, process hooks.
