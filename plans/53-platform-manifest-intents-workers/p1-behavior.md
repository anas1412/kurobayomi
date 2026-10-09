# Area 53 — behaviour spec (Aniyomi observed, Kurobayomi proposed)

Source root: `/home/blackbox/aniyomi-src` (paths below are repo-relative). Line numbers are `path:line` into the files as they are on disk. Section 1 gives the component map; sections 2 to 13 give the behaviour per component. Every section ends with the Kurobayomi proposal, labelled **Proposal:**. Everything else is **Observed**.

Terminology: "the receiver" means `app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt`. "the dispatcher" is the Kurobayomi replacement for it (`AppAction` dispatch, `p1-rust-design.md` section 5).

## 1. Component map and the decision for each component

Observed: the app declares 11 activities, 1 receiver, 6 services, 2 providers and 14 permissions (`app/src/main/AndroidManifest.xml`, full table in `p1-data-and-config.md` section 1). Each is listed here with what it does at run time and the decision.

| # | Component (manifest line) | Run-time role | Proposal | Owner area |
|---|---|---|---|---|
| 1 | `.ui.main.MainActivity` (`app/src/main/AndroidManifest.xml` activity block with `singleTop`, `adjustResize`, splash theme) | Single window: home navigator, intent router, OAuth return target, notification target | GTK `gtk::Application` with one `ApplicationWindow`; launch requests routed by `kurobayomi::launch` (section 2) | area:36 |
| 2 | `.crash.CrashActivity` (`:error_handler` process) | Crash screen | Owned by area:01 crash handling | area:01 |
| 3 | `.ui.deeplink.anime.DeepLinkAnimeActivity` | Trampoline for anime search intents | Drop trampoline (section 3) | this area |
| 4 | `.ui.deeplink.manga.DeepLinkMangaActivity` | Trampoline for manga search intents | Drop trampoline (section 3) | this area |
| 5 | `.ui.reader.ReaderActivity` (singleTask, Samsung REMOTE_ACTION) | Reader; activity transitions; S Pen | Reader page; S Pen dropped (section 11) | area:43 |
| 6 | `.ui.player.PlayerActivity` (singleTask, PiP, resizable, configChanges, autoRemoveFromRecents) | Player | Player page; PiP dropped (section 11) | area:45 |
| 7 | `.ui.security.UnlockActivity` | App lock prompt | Owned by area:36 (startup lock) | area:36 |
| 8 | `.ui.webview.WebViewActivity` | In-app WebView for Cloudflare and source pages | Owned by area:36 (`WebViewHost`) | area:36 |
| 9 | `.extension.manga.util.MangaExtensionInstallActivity` | Starts `ACTION_INSTALL_PACKAGE` and reports the result | Drop (no APK install on Linux; area:20) | area:18 |
| 10 | `.extension.anime.util.AnimeExtensionInstallActivity` | Same for anime extensions | Drop | area:19 |
| 11 | `.ui.setting.track.TrackLoginActivity` (`exported=true`, scheme `aniyomi`, five auth hosts) | OAuth redirect receiver for trackers | Port: OAuth redirect routing (section 2.6) | area:27 |
| 12 | `.data.notification.NotificationReceiver` (`exported=false`) | Dispatcher for every notification button and some content intents | Rewrite as GIO action dispatcher (sections 4 to 6) | this area + area:35 |
| 13 | `.extension.manga.util.MangaExtensionInstallService` (`shortService`) | Foreground notification and install queue | Drop (area:18, area:20) | area:18 |
| 14 | `.extension.anime.util.AnimeExtensionInstallService` (`shortService`) | Same for anime | Drop | area:19 |
| 15 | `androidx.appcompat.app.AppLocalesMetadataHolderService` (disabled, `autoStoreLocales=true`) | Per-app locale storage (AndroidX) | Drop (section 12) | area:51 |
| 16 | `androidx.work.impl.foreground.SystemForegroundService` (`dataSync`, `tools:node=merge`) | WorkManager foreground host for `setForeground` workers | Drop; replaced by a job-runner notice and `hold()` (section 9) | area:26 |
| 17 | `.data.torrent.service.TorrentServerService` (`dataSync`) | Local torrent streaming server (TorrServer) | Port as a background service with a notice (section 10) | area:13 |
| 18 | `.data.player.service.HttpServerService` (`dataSync`) | Local HTTP server for external players | Port as background service with a notice (section 10) | area:45 |
| 19 | `androidx.core.content.FileProvider` (`${applicationId}.provider`, `grantUriPermissions`) | `content://` URIs for files shared with other apps | Drop (section 12.2) | this area |
| 20 | `rikka.shizuku.ShizukuProvider` (`${applicationId}.shizuku`, `INTERACT_ACROSS_USERS_FULL`) | Shizuku binder bootstrap | Drop (section 9.4) | area:20 |
| 21 | `uses-permission` x14 | Capabilities | Replace or drop (section 13) | this area |
| 22 | `application` attributes | Process-wide settings | Drop or replace (section 13.2) | this area |
| 23 | `meta-data` WebView safe browsing / metrics | Off | Drop | this area |
| 24 | `shortcuts.xml` (`android.app.shortcuts`) | Launcher long-press | `[Desktop Action]` (section 2.4) | this area |
| 25 | `searchable.xml` | Search hint for three activities | Desktop search provider (P2, section 2.5) | this area |
| 26 | `network_security_config.xml` | Cleartext and CA trust | Reqwest TLS policy (section 13.3) | area:12 |
| 27 | `provider_paths.xml` | FileProvider whitelist | Drop with 19 | this area |
| 28 | `s_pen_actions.xml` | Samsung remote actions | Drop (section 11.4) | this area |
| 29 | `presentation-widget` receivers (x6) | Glance widgets | Drop (section 13.4) | area:50 |

## 2. Launch, routing and the main window

### 2.1 Observed: MainActivity start-up

1. `MainActivity.onCreate` (`app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:158`) sets `isLaunch = savedInstanceState == null` (line 159). When launching, it installs the splash screen (line 162). It awaits the migrator (`Migrator.awaitAndRelease()`, line 166).
2. If the activity is not the task root (`!isTaskRoot`, line 169), it calls `finish()` and returns (lines 169 to 172). This is the reason that a second launcher tap does not create a second window.
3. The Compose content creates a `Navigator(screen = HomeScreen)` (lines 200 to 206). Inside the `LaunchedEffect(navigator)` (lines 208 to 218):
   - it stores the navigator in `this@MainActivity.navigator`;
   - when `isLaunch` is true it calls `handleIntentAction(intent, navigator)` (line 213) and then sets `preferences.incognitoMode().set(false)` (line 216). This is the "reset incognito on relaunch" rule.
4. `HandleOnNewIntent` (lines 357 to 368) registers `addOnNewIntentListener` and forwards every later intent to `handleIntentAction` through `collectLatest`. Because the activity is `singleTop`, a new intent for the running window is delivered here, not to a new activity.

### 2.2 Observed: handleIntentAction (MainActivity.kt:461 to 573)

Step order:

1. `notificationId = intent.getIntExtra("notificationId", -1)` (line 462). If `> -1`, call `NotificationReceiver.dismissNotification(applicationContext, notificationId, intent.getIntExtra("groupId", 0))` (lines 463 to 469). This is the only place where the content intents of notifications dismiss their notice.
2. `when (intent.action)` maps the action to a tab (lines 471 to 565). The table is in `p1-data-and-config.md` section 4. Key rules:
   - `SHORTCUT_MANGA` requires the extra `"manga"` (Long). Missing extra returns `false` and the window is not marked ready (line 475). Otherwise `navigator.popUntilRoot()` and `HomeScreen.Tab.Library(idToOpen)` (lines 476 to 477).
   - `SHORTCUT_ANIME` requires the extra `"anime"` (line 480) and opens `HomeScreen.Tab.AnimeLib(idToOpen)`.
   - `SHORTCUT_DOWNLOADS` and `SHORTCUT_ANIME_DOWNLOADS` call `popUntilRoot()` and open `HomeScreen.Tab.More(toDownloads = true)` (lines 489 to 496).
   - `ACTION_SEARCH`, `ACTION_SEND` or `com.google.android.gms.actions.SEARCH_ACTION` (lines 497 to 524): the query is `intent.getStringExtra(SearchManager.QUERY)` or else `Intent.EXTRA_TEXT` (lines 502 to 503). If non-empty, `popUntilRoot()`, the type comes from extra `type` (default `"ANIME"`, `.ifBlank { "ANIME" }`), converted with `DeepLinkScreenType.valueOf` (line 510). MANGA pushes `GlobalMangaSearchScreen(query)` then `DeepLinkMangaScreen(query)` (lines 514 to 515). ANIME pushes `GlobalAnimeSearchScreen(query)` then `DeepLinkAnimeScreen(query)` (lines 518 to 519). An empty query does nothing.
   - `INTENT_SEARCH` = `eu.kanade.tachiyomi.SEARCH` (lines 525 to 533): reads `query` and `filter` extras, pops to root, pushes `GlobalMangaSearchScreen(query, filter)`.
   - `INTENT_ANIMESEARCH` = `eu.kanade.tachiyomi.ANIMESEARCH` (lines 534 to 542): same for `GlobalAnimeSearchScreen(query, filter)`.
   - `Intent.ACTION_VIEW` (lines 543 to 563):
     - if `intent.data.toString().endsWith(".tachibk")` then `popUntilRoot()` and push `RestoreBackupScreen(intent.data.toString())` (lines 545 to 547);
     - else if the scheme is `aniyomi` and the host is `add-repo` or `extension-store` (`isAddAnimeExtensionStoreIntent`, lines 586 to 588), read query parameter `url`; when present, `popUntilRoot()` and push `AnimeExtensionStoresScreen(repoUrl)` (lines 550 to 554);
     - else if the scheme is `tachiyomi` and the host is `add-repo`, read `url` and push `MangaExtensionReposScreen(repoUrl)` (lines 556 to 560).
     - the `ACTION_VIEW` branch always ends with `null` (line 562), so the window is marked ready.
   - `else` returns `false` (line 564), so an unknown action leaves the window not ready.
3. If `tabToOpen != null`, `lifecycleScope.launch { HomeScreen.openTab(tabToOpen) }` (lines 567 to 569).
4. `ready = true` and `return true` (lines 571 to 572).

Observed defects of this function (see also edge cases E-14 to E-18): the `valueOf` on line 510 throws `IllegalArgumentException` for any non-blank `type` other than `ANIME` or `MANGA`, which crashes the window; the `.tachibk` test is on the full URI string, so `content://` URIs whose display name does not end in `.tachibk` in the string are ignored; the `ACTION_VIEW` path does not check the intent's MIME type even though the manifest filter lists `*/*`.

**Proposal:** `kurobayomi::launch::route(LaunchRequest) -> Result<(Vec<NavAction>, ready: bool), RouteError>` in `kuro-core::launch`, as already defined by area:36 (`p1-rust-design.md` in area 36). Every case above keeps its mapping. `type` parse errors return `RouteError::UnknownKind(String)` and the window opens the Browse tab with a toast (`AYMR.strings.download_error` is not reused; the new key is listed in p1-ui-parity). The `.tachibk` check is `Path::extension() == "tachibk"` after URL decoding, never a string suffix on the full URI.

### 2.3 Observed: Intent extras read by MainActivity

| Extra | Type | Read at | Meaning |
|---|---|---|---|
| `notificationId` | Int | line 462 | Notice to withdraw |
| `groupId` | Int, default 0 | line 467 | Group summary to withdraw when the group has exactly two notices |
| `manga` | Long | line 475 | Manga id for `SHORTCUT_MANGA` (`Constants.MANGA_EXTRA = "manga"`) |
| `anime` | Long | line 480 | Anime id for `SHORTCUT_ANIME` (`Constants.ANIME_EXTRA = "anime"`) |
| `type` | String (`ANIME` or `MANGA`) | line 508 | Deep-link kind (`MainActivity.INTENT_SEARCH_TYPE = "type"`) |
| `query` | String | lines 526, 535 | Query for `INTENT_SEARCH` and `INTENT_ANIMESEARCH` (`INTENT_SEARCH_QUERY = "query"`) |
| `filter` | String | lines 528, 537 | Optional filter (`INTENT_SEARCH_FILTER = "filter"`) |
| `SearchManager.QUERY` | String | line 502 | Standard search query (`android.intent.extra.TEXT` fallback at line 503 via `Intent.EXTRA_TEXT`) |

### 2.4 Observed: launcher shortcuts (`app/shortcuts.xml`)

- Five `<shortcut>` entries, all `android:enabled="true"`, `android:shortcutDisabledMessage="@string/app_not_available"` (`MR` key `app_not_available`, English "App not available"), target class `eu.kanade.tachiyomi.ui.main.MainActivity`.
- Action to label mapping: `SHOW_LIBRARY` (`show_library`) label `label_manga_library` (English "Manga"), icon `sc_collections_bookmark_48dp`; `SHOW_ANIMELIB` (`show_animelib`) label `label_anime_library` ("Anime"), icon `sc_collections_baseline_48dp`; `SHOW_RECENTLY_UPDATED` (`show_recently_updated`) label `label_recent_updates` ("Updates"), icon `sc_new_releases_48dp`; `SHOW_RECENTLY_READ` (second entry, id `show_recently_updated`, duplicate) label `label_recent_manga` ("History"), icon `sc_history_48dp`; `SHOW_CATALOGUES` (`show_catalogues`) label `browse` ("Browse"), icon `sc_explore_48dp`.
- Defect: the history entry reuses the id `show_recently_updated`. Android keeps only one of two shortcuts with the same id, so the history shortcut is not reliably shown.
- Launcher shortcuts are static (no `ShortcutManager` call exists in the code: verified, zero matches).

**Proposal:** desktop actions in the `.desktop` file, one per entry:

| Action id | Name | Exec argument | Route |
|---|---|---|---|
| `show-library` | Manga library | `--action=show-library` | `Shortcut { Library, None }` |
| `show-animelib` | Anime library | `--action=show-animelib` | `Shortcut { AnimeLib, None }` |
| `show-updates` | Updates | `--action=show-updates` | `Shortcut { Updates, None }` |
| `show-history` | History | `--action=show-history` | `Shortcut { History, None }` |
| `show-browse` | Browse | `--action=show-browse` | `Shortcut { Catalogues, None }` |

The duplicate id is removed. The labels come from the same MR keys (localised at build time through `area:51`). The icon is the app icon name, `io.github.blackbox.Kurobayomi` (Linux icon theme lookup; no per-action icon file).

### 2.5 Observed: search metadata (`app/src/main/res/xml/searchable.xml`)

Three activities declare `android:name="android.app.searchable"` meta-data pointing to `@xml/searchable`: `DeepLinkAnimeActivity` (`app/src/main/AndroidManifest.xml` around the anime deep-link block), `DeepLinkMangaActivity` (manga block). The hint is `@string/action_global_search_hint` ("Global search…", `MR` key `action_global_search_hint`), the label is `@string/app_name` ("Aniyomi"). The searchable meta-data is what made the Android system search box and Google Assistant route to these activities; it has no effect inside the app.

**Proposal:** P2 task. A D-Bus search provider (`org.gnome.Shell.SearchProvider2` interface, `.ini` file in `share/gnome-shell/search-providers/`) and, when KRunner is installed, a KRunner D-Bus runner. Both call `route(Search { query, kind: Some(Anime|Manga) })` and show the existing global search page. Owner: `kuro-platform::linux::search_provider`. Priority P2 because the in-app search is the primary path.

### 2.6 Observed: OAuth redirect (TrackLoginActivity and BaseOAuthLoginActivity)

- `TrackLoginActivity` (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`) is an `exported="true"` activity with `android:label="@string/track_activity_name"` ("Tracker login"). Its filter: action `VIEW`, categories `DEFAULT` and `BROWSABLE`, scheme `aniyomi`, hosts `myanimelist-auth`, `anilist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth`.
- `BaseOAuthLoginActivity.onCreate` (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt:19` to `:27`) sets a `LoadingScreen` and calls `handleResult(intent.data)`.
- `handleResult` (`TrackLoginActivity.kt:9` to `:17`) dispatches on `data?.host`:
  - `anilist-auth`: `handleAnilist` (lines 19 to 31). Regex `(?:access_token=)(.*?)(?:&)` applied to `data.fragment.toString()` (line 20 to 21). If group 1 is found, `trackerManager.aniList.login(token)` on IO then `returnToSettings()`. Otherwise `trackerManager.aniList.logout()` then `returnToSettings()`. Note: the regex needs a trailing `&`; a fragment that ends with `access_token=...` (no further parameter) fails the match and logs the user out (edge E-22).
  - `bangumi-auth`: `data.getQueryParameter("code")` (line 34). Present: `login(code)`; absent: `logout()`.
  - `myanimelist-auth`: same with `myAnimeList` (lines 46 to 57).
  - `shikimori-auth`: same with `shikimori` (lines 59 to 70).
  - `simkl-auth`: `data?.getQueryParameter("code")` (line 73), `simkl.login(code)` or `simkl.logout()` (lines 72 to 83).
  - any other host: nothing happens and the activity finishes only through `returnToSettings` (not called), so the screen stays on the loading state (edge E-23).
- `returnToSettings()` (`BaseOAuthLoginActivity.kt:29` to `:37`) calls `finish()`, then starts `MainActivity` with `FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_SINGLE_TOP` and no extras.

**Proposal:** the OAuth redirect is a launch request `OpenUri(url)` with scheme `kurobayomi` (primary) or `aniyomi` (alias). `route` maps host `*-auth` to `Route::TrackerOAuth { provider, params }`, where `params` is the parsed fragment (for AniList implicit flow) or query (for code flows). The token is exchanged or stored by area:27. The regex is replaced by a parse of the fragment with `url::form_urlencoded` so the trailing `&` is not needed. The redirect-URI registration question (whether `kurobayomi://` needs new client registrations at each provider) is recorded as an open question in `p1-topic-launch-intents-and-deeplinks.md` section 6 and is shared with area:27.

### 2.7 Observed: deep-link trampolines (DeepLinkAnimeActivity, DeepLinkMangaActivity)

Both activities are `Activity` subclasses with `Theme.NoDisplay` (`app/src/main/AndroidManifest.xml`). `onCreate` (`DeepLinkAnimeActivity.kt:11` to `:21`; manga identical at `DeepLinkMangaActivity.kt:11` to `:21`):

1. `super.onCreate(savedInstanceState)`.
2. `intent.apply { flags = flags or FLAG_ACTIVITY_CLEAR_TOP or FLAG_ACTIVITY_NEW_TASK; putExtra(MainActivity.INTENT_SEARCH_TYPE, DeepLinkScreenType.ANIME.toString()) (manga: MANGA); setClass(applicationContext, MainActivity::class.java) }`.
3. `startActivity(intent)` then `finish()`.

Intent filters of the anime trampoline (lines in the manifest): `SEARCH`, `com.google.android.gms.actions.SEARCH_ACTION` (category DEFAULT), `eu.kanade.tachiyomi.ANIMESEARCH` (DEFAULT), `SEND` with `text/plain`. The manga trampoline has the same set with `eu.kanade.tachiyomi.SEARCH` and `eu.kanade.tachiyomi.ANIMESEARCH`.

Observed consequence (important): an `eu.kanade.tachiyomi.ANIMESEARCH` intent delivered to the manga trampoline gets `type=MANGA`, but `MainActivity` routes `INTENT_ANIMESEARCH` by action and ignores `type`, so it opens the anime search. The type is only used by the SEARCH/SEND branch.

**Proposal:** no trampolines. `route_request` receives `LaunchRequest` directly (area:36 section 4). The filters become: `Search { query, kind }` for `SEARCH`/`SEND`, `ExtSearch { kind: Anime, query, filter }` for `ANIMESEARCH`, and `ExtSearch { kind: Manga, ... }` for `eu.kanade.tachiyomi.SEARCH`. The mismatch above is not reproduced: `ExtSearch` carries its own kind.

### 2.8 Observed: the `Intent.ACTION_SEND` text share

`ACTION_SEND` with `text/plain` (both deep-link trampolines) passes the whole shared text as the query (`Intent.EXTRA_TEXT`, MainActivity line 503). There is no URL extraction in the app. Shared URLs therefore go to global search as text, not to a deep-link resolve. This is consistent with the deep-link resolution that runs in the screen model (section 3), which expects a URL: the URL only reaches it if the user types it into a search that the source's `getUriType` recognises.

**Proposal:** the `Search` route accepts text. If the text parses as an `http(s)` URL, the route is `Resolve { query: url, kind }` (section 3) and the global search page is pushed underneath as a fallback. Otherwise the route is the same as today. This is a small improvement that is not in Aniyomi; it is listed in the tasks as P2 (T-53a-005 notes).

### 2.9 Observed: MainActivity single-instance plumbing that is not carried over

- `launchMode="singleTop"` for MainActivity (manifest), `singleTask` for reader, player, deep-link trampolines (see manifest).
- `android:windowSoftInputMode="adjustResize"` (MainActivity).
- `SPLASH_MIN_DURATION = 500` ms, `SPLASH_MAX_DURATION = 5000` ms, `SPLASH_EXIT_ANIM_DURATION = 400L` ms (`MainActivity.kt:665` to `:667`). These belong to the Android splash screen API and are dropped (the app has no splash). The start-up delay that the splash hides is the migrator wait (`Migrator.awaitAndRelease`), which the GTK app shows as the loading state of the library page (area:36).
- `onSaveInstanceState` stores `ExternalIntents.externalIntents.animeId` and `episodeId` under `SAVED_STATE_ANIME_KEY = "saved_state_anime_key"` and `SAVED_STATE_EPISODE_KEY = "saved_state_episode_key"` (lines 575 to 584, 597 to 598). Process death during external playback is restored from these keys. Proposal: the same state lives in the player controller (area:45); no keys are needed because GTK restores the window from the app state DB.
- `startPlayerActivity` (lines 602 to 642) chooses between the external player (through `HttpServerService`, timeout 5 s, `startHttpServerService` lines 644 to 660) and `PlayerActivity.newIntent(...)`. The external path is area:45's decision; it is recorded here because the trampolines bypass it (section 2.7 step 2 of the anime screen model: the deep link to an episode calls `PlayerActivity.newIntent` directly, see section 3.1).

## 3. Deep-link resolution (both kinds)

Both screen models run the same algorithm; the differences are called out. The shared algorithm is also in area:36 (`p1-topic-deeplinks-intents.md` section 3). This section gives the Kotlin side exactly.

### 3.1 Anime (`DeepLinkAnimeScreenModel.kt:24` to `:88`)

Inputs: `query: String` (the deep-link URL or search text), injected `sourceManager: AnimeSourceManager`, `networkToLocalAnime`, `getEpisodeByUrlAndAnimeId`, `getAnimeByUrlAndSourceId`, `updateAnimeFromRemote`.

Initial state `State.Loading` (line 31). The `init` block (lines 33 to 61) runs in `screenModelScope.launchIO`:

1. `source = sourceManager.getAll().filterIsInstance<ResolvableAnimeSource>().firstOrNull { it.getUriType(query) != UriType.Unknown }` (lines 35 to 37). Order is the order of `getAll()`; no sort is applied here.
2. `anime = source?.getAnime(query)?.let { getAnimeFromSAnime(it, source.id) }` (lines 39 to 41). Note: `getAnime` is called for every resolvable URI type, not only `UriType.Anime`, although the extension contract says it is called for `UriType.Anime` (`ResolvableAnimeSource.kt` line 23).
3. `episode = if (source?.getUriType(query) == UriType.Episode && anime != null) source.getEpisode(query)?.let { getEpisodeFromSEpisode(it, anime, source) } else null` (lines 43 to 47).
4. `mutableState.update { if (anime == null) NoResults else if (episode == null) Result(anime) else Result(anime, episode.id) }` (lines 49 to 59).

Helpers:

- `getAnimeFromSAnime(sAnime, sourceId)` (lines 73 to 76): `getAnimeByUrlAndSourceId.await(sAnime.url, sourceId) ?: networkToLocalAnime.await(sAnime.toDomainAnime(sourceId))`. Local hit first; otherwise the anime is inserted (no network refresh).
- `getEpisodeFromSEpisode(sEpisode, anime, source)` (lines 63 to 71): `localEpisode = getEpisodeByUrlAndAnimeId.await(sEpisode.url, anime.id)`; if non-null return it. Otherwise `updateAnimeFromRemote.awaitEpisodesUpdate(anime, fetchEpisodes = true).getOrElse { return null }.newEpisodes.find { it.url == sEpisode.url }`. The episode list is fetched from the source only when the local row is missing.

Screen effects (`DeepLinkAnimeScreen.kt:22` to `:72`):

- `Loading`: `LoadingScreen` inside the scaffold with title `action_search_hint` ("Search…").
- `NoResults`: `navigator.replace(GlobalAnimeSearchScreen(query))` (line 49).
- `Result` with `episodeId == null`: `navigator.replace(AnimeScreen(anime.id, true))` (lines 54 to 59). The second constructor argument is `fromSource = true`.
- `Result` with an episode: `navigator.pop()` then `PlayerActivity.newIntent(context, anime.id, episodeId).also(context::startActivity)` (lines 61 to 66). The external-player setting is not consulted here (edge E-09).

Error behaviour (observed): an exception inside the coroutine (for example a network error in `getAnime`) escapes the `launchIO` block; the state is never updated, so the screen remains on `Loading` forever (edge E-07).

### 3.2 Manga (`DeepLinkMangaScreenModel.kt:25` to `:91`)

Same structure with these differences:

1. Source list: `sourceManager.getCatalogueSources()` (line 36), filtered to `ResolvableSource` (line 37). Only catalogue sources are considered; anime's `getAll()` is wider.
2. `chapter = if (getUriType(query) == UriType.Chapter && manga != null) getChapter(query)?.let { getChapterFromSChapter(it, manga, source) }` (lines 44 to 48).
3. `getChapterFromSChapter(sChapter, manga, source)` (lines 64 to 74): `localChapter = getChapterByUrlAndMangaId.await(sChapter.url, manga.id)`. If null: `sourceChapters = source.getChapterList(manga.toSManga())`, then `newChapters = syncChaptersWithSource.await(sourceChapters, manga, source, false)` (line 69), then `newChapters.find { it.url == sChapter.url }`. The `false` argument is the third flag of `SyncChaptersWithSource` (the "override"/"is initial" flag; the Kotlin does not name it). Proposal: the same call with the same flag, documented in area:05/07 (chapter sync).
4. `getMangaFromSManga` (lines 76 to 79) mirrors the anime version.
5. Screen effects (`DeepLinkMangaScreen.kt` lines 48 to 68): `NoResults` replaces with `GlobalMangaSearchScreen(query)`; `Result` without chapter replaces with `MangaScreen(manga.id, true)`; `Result` with chapter pops and starts `ReaderActivity.newIntent(context, manga.id, chapterId)`.

### 3.3 Shared rules

- `UriType` is a sealed interface: anime `{Anime, Episode, Unknown}` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ResolvableAnimeSource.kt` lines 39 to 43), manga `{Manga, Chapter, Unknown}` (`.../source/online/ResolvableSource.kt` lines 39 to 43).
- Only the first matching source is used. If no source matches, the state is `NoResults` and the global search page replaces the loading page.
- The query is the raw string; no normalisation (trim, scheme check) happens in the models.
- Both models hold no references to UI; navigation happens in the composable (edge E-10).

**Proposal:** `kuro_core::deeplink::{anime, manga}::resolve(query, ctx) -> Result<Resolved, ResolveError>` on tokio. `Resolved` is `Entry { entry_id, child: Option<ChildId> }` or `NoMatch`. `ResolveError` covers source errors, network errors and missing rows; errors become a visible state with a retry button (`MR.strings.action_retry`, English "Retry"), and the Kotlin silent-hang is not reproduced. The screen subscribes through an `async_channel` and navigates once (a one-shot effect), never inside a render function. The anime path calls `PlayerActivity` through the player router of area:45 so the external-player setting applies (area:36 decision T-36a-058).

## 4. The notification receiver: actions and their behaviour

### 4.1 Dispatch skeleton (observed)

`NotificationReceiver.onReceive(context, intent)` (receiver lines 64 to 184) switches on `intent.action`. The action string is compared with exact equality. Each branch is listed in section 4.2. Common facts:

- No permission check is made before any action runs. The receiver is `exported="false"` (manifest), so only the app's own `PendingIntent`s reach it.
- Long-running work is started with `launchIO` or `runBlocking`. `openChapter` and `openEpisode` call `runBlocking { ... }` on the broadcast thread (lines 224 to 225, 244 to 245); the code comment (lines 48 to 52) calls the receiver "UI thread", so these run on the main thread. Edge E-03.
- Errors thrown in `launchIO` blocks are not caught (for example `markAsRead` lines 302 to 318). They go to the default coroutine handler of `launchIO`; no user message is shown.

### 4.2 Action table (observed)

Column "dismiss" means the branch calls the companion `dismissNotification(context, notificationId, groupId)` first (section 5.2) when the extra `EXTRA_NOTIFICATION_ID` is greater than -1.

| Action constant (value suffix) | Branch lines | Extras read | Steps | Side effects | Toast on error |
|---|---|---|---|---|---|
| `ACTION_DISMISS_NOTIFICATION` (`ACTION_DISMISS_NOTIFICATION`) | 66 to 70 | `EXTRA_NOTIFICATION_ID` (default -1) | calls the private member `dismissNotification(context, id)` (lines 191 to 194): `context.cancelNotification(id)` **then** `context.notificationManager.cancelAll()` | Cancels every notification of the app (defect E-01) | none |
| `ACTION_RESUME_DOWNLOADS` | 71 to 72 | none | `mangaDownloadManager.startDownloads()` | starts the manga queue | none |
| `ACTION_PAUSE_DOWNLOADS` | 73 to 74 | none | `mangaDownloadManager.pauseDownloads()` | pauses the manga queue | none |
| `ACTION_CLEAR_DOWNLOADS` | 75 to 76 | none | `mangaDownloadManager.clearQueue()` | clears the manga queue | none |
| `ACTION_RESUME_ANIME_DOWNLOADS` | 77 | none | `animeDownloadManager.startDownloads()` | starts the anime queue | none |
| `ACTION_PAUSE_ANIME_DOWNLOADS` | 79 | none | `animeDownloadManager.pauseDownloads()` | pauses the anime queue | none |
| `ACTION_CLEAR_ANIME_DOWNLOADS` | 81 | none | `animeDownloadManager.clearQueue()` | clears the anime queue | none |
| `ACTION_SHARE_IMAGE` | 83 to 87 | `EXTRA_URI` (String) | `shareImage(context, uri)` = `startActivity(uri.toShareIntent(context))` (lines 202 to 204). `!!` on the extra | opens the share chooser | crash if the extra is missing (edge E-04) |
| `ACTION_SHARE_BACKUP` | 89 to 94 | `EXTRA_URI` as Parcelable (`getParcelableExtraCompat`) | `shareFile(context, uri, "application/x-protobuf+gzip")` (lines 212 to 214) | opens the chooser for a backup file | crash if null (`!!`) |
| `ACTION_CANCEL_RESTORE` | 95 | `EXTRA_NOTIFICATION_ID` is set by the builder but not read | `BackupRestoreJob.stop(context)` (area:22) | cancels the unique work `BackupRestore` | none |
| `ACTION_CANCEL_LIBRARY_UPDATE` | 97 | none | `MangaLibraryUpdateJob.stop(context)` (lines 270 to 272) | cancels the manga library update work | none |
| `ACTION_CANCEL_ANIMELIB_UPDATE` | 98 | none | `AnimeLibraryUpdateJob.stop(context)` (lines 279 to 281) | cancels anime library update | none |
| `ACTION_START_APP_UPDATE` | 100 | `AppUpdateDownloadJob.EXTRA_DOWNLOAD_URL` ("DOWNLOAD_URL") | `startDownloadAppUpdate(context, intent)`: returns if the URL extra is null; else `AppUpdateDownloadJob.start(context, url)` (lines 283 to 286). The title extra (`DOWNLOAD_TITLE`) is not read here; it is sent but ignored | enqueues the APK download work (section 8) | none |
| `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | 102 | none | `AppUpdateDownloadJob.stop(context)` (lines 288 to 290) | cancels the APK download | none |
| `ACTION_OPEN_CHAPTER` | 104 to 110 | `EXTRA_MANGA_ID`, `EXTRA_CHAPTER_ID` | `openChapter` (lines 223 to 234) | **dead**: no builder sends this action (verified) | n/a |
| `ACTION_OPEN_EPISODE` | 112 to 118 | `EXTRA_MANGA_ID`, `EXTRA_CHAPTER_ID` (sic: reads the manga key for the anime id) | `openEpisode` (lines 243 to 254) | **dead** (verified) | n/a |
| `ACTION_MARK_AS_READ` | 120 to 134 | `EXTRA_NOTIFICATION_ID`, `EXTRA_GROUP_ID` (default 0), `EXTRA_CHAPTER_URL` (String array, required; `?: return`), `EXTRA_MANGA_ID` (must be > -1) | dismiss when id > -1 (`dismissNotification(context, id, groupId)`); then `markAsRead(urls, mangaId)` (lines 298 to 319) | updates chapters read flag; optionally deletes downloads | none |
| `ACTION_MARK_AS_SEEN` | 136 to 150 | same keys (`EXTRA_MANGA_ID` carries the anime id) | dismiss; then `markAsSeen(urls, animeId)` (lines 327 to 348) | updates episodes seen flag; optionally deletes episode downloads | none |
| `ACTION_DOWNLOAD_CHAPTER` | 152 to 166 | same keys | dismiss; then `downloadChapters(urls, mangaId)` (lines 356 to 362) | enqueues chapter downloads | none |
| `ACTION_DOWNLOAD_EPISODE` | 168 to 182 | same keys | dismiss; then `downloadEpisodes(urls, animeId)` (lines 370 to 376) | enqueues episode downloads | none |
| `ACTION_OPEN_ENTRY` | declared line 400 | none | no branch in `onReceive` (verified) | **dead**: never sent, never handled | n/a |

Important detail in the MARK actions: the dismiss uses `groupId = intent.getIntExtra(EXTRA_GROUP_ID, 0)`. The companion rule (section 5.2) runs only when the group id is non-zero.

### 4.3 Behaviour of the four worker-facing actions (observed)

`markAsRead(chapterUrls, mangaId)` (lines 298 to 319), run in `launchIO`:

1. `downloadPreferences = Injekt.get()`, `sourceManager = Injekt.get()` (lines 299 to 300).
2. `toUpdate = chapterUrls.mapNotNull { getChapter.await(it, mangaId) }` (lines 303): chapters that cannot be found by URL and manga id are skipped silently.
3. For each chapter `it`: `chapter = it.copy(read = true)`; if `downloadPreferences.removeAfterMarkedAsRead().get()` is true (preference key `pref_remove_after_marked_as_read_key`, default `false`): `manga = getManga.await(mangaId)`; if non-null, `source = sourceManager.get(manga.source)`; if non-null, `mangaDownloadManager.deleteChapters(listOf(it), manga, source)` (line 311). Note the argument is the original chapter `it`, not the copy, which is fine because only the id is used by the delete.
4. `updateChapter.awaitAll(toUpdate.map { chapter.toChapterUpdate() })` (lines 315 to 317).

`markAsSeen` mirrors this with `getEpisode`, `seen = true`, `animeDownloadManager.deleteEpisodes(listOf(it), anime, source)` and `updateEpisode.awaitAll`.

`downloadChapters(chapterUrls, mangaId)` (lines 356 to 362): `manga = getManga.await(mangaId) ?: return@launchIO`; `chapters = chapterUrls.mapNotNull { getChapter.await(it, mangaId) }`; `mangaDownloadManager.downloadChapters(manga, chapters)`. An empty list is passed through (the download manager decides, area:23).

`downloadEpisodes(episodeUrls, animeId)` (lines 370 to 376): mirror for anime, `animeDownloadManager.downloadEpisodes(anime, episodes)` (area:24).

### 4.4 The builders (observed, for each notification button)

Every builder uses `PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE`, except `installApkPendingActivity` (`NotificationHandler.kt:82`), `openUrl` (`:91`) and `openErrorLogPendingActivity` (`NotificationReceiver.kt:976`), which use `FLAG_IMMUTABLE` only (no update of extras), and the App incognito content intent, which uses `FLAG_ONE_SHOT or FLAG_IMMUTABLE`. Request code is `0` unless noted. The request code and the action decide which intent is updated.

| Builder (companion) | Lines | Action | Request code | Extras |
|---|---|---|---|---|
| `resumeDownloadsPendingBroadcast(context)` | 425 to 435 | `ACTION_RESUME_DOWNLOADS` | 0 | none |
| `pauseDownloadsPendingBroadcast(context)` | 443 to 453 | `ACTION_PAUSE_DOWNLOADS` | 0 | none |
| `clearDownloadsPendingBroadcast(context)` | 461 to 471 | `ACTION_CLEAR_DOWNLOADS` | 0 | none |
| `resumeAnimeDownloadsPendingBroadcast(context)` | 479 to 489 | `ACTION_RESUME_ANIME_DOWNLOADS` | 0 | none |
| `pauseAnimeDownloadsPendingBroadcast(context)` | 497 to 507 | `ACTION_PAUSE_ANIME_DOWNLOADS` | 0 | none |
| `clearAnimeDownloadsPendingBroadcast(context)` | 515 to 525 | `ACTION_CLEAR_ANIME_DOWNLOADS` | 0 | none |
| `dismissNotificationPendingBroadcast(context, id)` | 534 to 545 | `ACTION_DISMISS_NOTIFICATION` | 0 | `EXTRA_NOTIFICATION_ID` |
| `shareImagePendingBroadcast(context, uri)` | 595 to 606 | `ACTION_SHARE_IMAGE` | 0 | `EXTRA_URI` = `uri.toString()` |
| `openEpisodePendingActivity(context, anime, episode)` | 615 to 623 | none (component `PlayerActivity`) | `anime.id.hashCode()` | from `PlayerActivity.newIntent` (`animeId`, `episodeId`) |
| `openEpisodePendingActivity(context, anime, groupId)` | 631 to 644 | `eu.kanade.tachiyomi.SHOW_ANIME` (MainActivity) | `anime.id.hashCode()` | `anime` (Long), `notificationId` (= hashCode), `groupId` |
| `markAsViewedPendingBroadcast(context, anime, episodes, groupId)` | 652 to 671 | `ACTION_MARK_AS_SEEN` | `anime.id.hashCode()` | `EXTRA_CHAPTER_URL` (array of episode urls), `EXTRA_MANGA_ID` (anime id), `EXTRA_NOTIFICATION_ID` (= anime hashCode), `EXTRA_GROUP_ID` |
| `openChapterPendingActivity(context, manga, chapter)` | 680 to 688 | none (component `ReaderActivity`) | `manga.id.hashCode()` | from `ReaderActivity.newIntent` (`manga`, `chapter`) |
| `openChapterPendingActivity(context, manga, groupId)` | 696 to 709 | `eu.kanade.tachiyomi.SHOW_MANGA` | `manga.id.hashCode()` | `manga`, `notificationId`, `groupId` |
| `markAsViewedPendingBroadcast(context, manga, chapters, groupId)` | 717 to 736 | `ACTION_MARK_AS_READ` | `manga.id.hashCode()` | as the anime version |
| `downloadChaptersPendingBroadcast(context, manga, chapters, groupId)` | 744 to 763 | `ACTION_DOWNLOAD_CHAPTER` | `manga.id.hashCode()` | as above |
| `downloadEpisodesPendingBroadcast(context, anime, episodes, groupId)` | 771 to 790 | `ACTION_DOWNLOAD_EPISODE` | `anime.id.hashCode()` | as above |
| `openMangaEntryPendingActivity(context, mangaId)` | 798 to 809 | `eu.kanade.tachiyomi.SHOW_MANGA` | `mangaId.hashCode()` | `manga`, `notificationId` |
| `openAnimeEntryPendingActivity(context, animeId)` | 817 to 828 | `eu.kanade.tachiyomi.SHOW_ANIME` | `animeId.hashCode()` | `anime`, `notificationId` |
| `cancelLibraryUpdatePendingBroadcast(context)` | 836 to 846 | `ACTION_CANCEL_LIBRARY_UPDATE` | 0 | none |
| `cancelAnimelibUpdatePendingBroadcast(context)` | 854 to 864 | `ACTION_CANCEL_ANIMELIB_UPDATE` | 0 | none |
| `downloadAppUpdatePendingBroadcast(context, url, title?)` | 872 to 888 | `ACTION_START_APP_UPDATE` | 0 | `DOWNLOAD_URL`, optional `DOWNLOAD_TITLE` |
| `cancelDownloadAppUpdatePendingBroadcast(context)` | 893 to 903 | `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` | 0 | none |
| `openExtensionsPendingActivity(context)` | 911 to 922 | `eu.kanade.tachiyomi.EXTENSIONS` (MainActivity) | 0 | CLEAR_TOP flag |
| `openAnimeExtensionsPendingActivity(context)` | 930 to 941 | `eu.kanade.tachiyomi.ANIMEEXTENSIONS` | 0 | CLEAR_TOP flag |
| `shareBackupPendingBroadcast(context, uri)` | 950 to 961 | `ACTION_SHARE_BACKUP` | 0 | `EXTRA_URI` as Uri |
| `openErrorLogPendingActivity(context, uri)` | 970 to 977 | `ACTION_VIEW` with `text/plain` (NEW_TASK, GRANT_READ_URI) | 0 | data = uri |
| `cancelRestorePendingBroadcast(context, notificationId)` | 986 to 997 | `ACTION_CANCEL_RESTORE` | 0 | `EXTRA_NOTIFICATION_ID` |
| `openImagePendingActivity` (NotificationHandler.kt:58 to :69) | | `ACTION_VIEW` `image/*` | 0 | data = uri |
| `installApkPendingActivity` (NotificationHandler.kt:77 to :83) | | `ACTION_VIEW` APK MIME | 0 | data = uri (dropped) |
| `openUrl` (NotificationHandler.kt:85 to :93) | | `ACTION_VIEW` `url` | 0 | data = url |
| `openDownloadManagerPendingActivity` (NotificationHandler.kt:21 to :32) | | `Constants.SHORTCUT_DOWNLOADS` on MainActivity with `CLEAR_TOP or SINGLE_TOP` | 0 | none |
| `openAnimeDownloadManagerPendingActivity` (NotificationHandler.kt:39 to :50) | | `Constants.SHORTCUT_ANIME_DOWNLOADS` with `NEW_TASK or REORDER_TO_FRONT` | 0 | none |

Observed pitfall (defect E-05): because the request code is `0` for many builders and the `PendingIntent` identity is defined by request code plus action plus component plus data (extras are not part of the identity), two notices built with the same action and request code share one `PendingIntent`; the second `FLAG_UPDATE_CURRENT` overwrites the extras of the first. Affected: `dismissNotificationPendingBroadcast` (app update prompt id 2 and error id 3 both use `ACTION_DISMISS_NOTIFICATION` with request code 0), `downloadAppUpdatePendingBroadcast` (URL differs per release), `shareImagePendingBroadcast` (URI differs per image). The Linux design avoids this because each action carries its payload inside the GAction target (`p1-rust-design.md` section 5).

### 4.5 Proposal: the dispatcher

**Proposal:** `AppAction` (`src/notify/actions.rs`, area:35 section 3.1) replaces every action and extra. The mapping from the table above to `AppAction` variants is in `p1-data-and-config.md` section 5. Behaviour rules:

1. All `AppAction` values are delivered by the GIO action `app.notice` (parameter type `s`, the JSON codec of `AppAction`). Notification default action and buttons carry the same string (`set_default_action_and_target_value`, `add_button_with_target_value`).
2. The handler runs on the GTK main thread. It does not block. Work is spawned on the tokio runtime (`p1-rust-design.md` section 5) and results return through `glib::MainContext::channel` or `async_channel`.
3. `Dismiss { id }` withdraws exactly that id (`gio::Application::withdraw_notification`). It never calls a bulk withdraw (no `cancelAll`).
4. Mark-read and mark-seen dismiss the notice first (same order as Kotlin, lines 120 to 134), then run the database update. If the update fails, the notice stays withdrawn and a toast `AYMR.strings.download_error` ("Error") is shown (new in Kurobayomi; Kotlin is silent).
5. Share actions are replaced: `RevealFile { path }` opens the containing folder through `Opener::reveal`, and `CopyText { text }` copies a URL to the clipboard. No chooser (see section 12).
6. Dead actions (`OpenChapter`, `OpenEpisode`, `OpenEntry` from the Kotlin `ACTION_*` constants) are not created, so no handler is needed.

## 5. Notification ids, withdrawal and the group summary

### 5.1 Observed id conventions

- `Notifications` (`app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt`) keeps fixed negative ids for long-running notices and small positive ids for app-update notices. Full table in `p1-data-and-config.md` section 3.
- Per-entry notices use `entry.id.hashCode()` as the id (`NotificationReceiver.kt` lines 652 to 671, 680 to 688 and others). Kotlin `Long.hashCode()` is `(int)(value ^ (value >>> 32))`.
- Group summary ids: `ID_NEW_CHAPTERS = -301` and `ID_NEW_EPISODES = -1301` (`Notifications.kt:144` and `:145`) are used as group ids; the per-entry notice id is the entry hashcode.

### 5.2 Observed: the companion `dismissNotification(context, notificationId, groupId)` (receiver lines 554 to 585)

Algorithm:

1. `groupKey = context.notificationManager.activeNotifications.find { it.id == notificationId }?.groupKey` (lines 569 to 571).
2. If `groupId != null && groupId != 0 && !groupKey.isNullOrEmpty()` (line 573):
   - `notifications = activeNotifications.filter { it.groupKey == groupKey }` (lines 574 to 576);
   - if `notifications.size == 2` (line 578): `context.cancelNotification(groupId)` (cancel the summary) and `return` (line 580). The single child notice is **not** cancelled by this branch.
3. Otherwise `context.cancelNotification(notificationId)` (line 584).

The comment on lines 559 to 568 says that dismissing a single child programmatically leaves the group summary in place, and that the system removes the summary when the last child goes. The branch does the opposite: it cancels the summary and leaves the single child visible. **Observed (not verified on a device in this review):** the child remains in the tray. Edge E-06 records it for device testing (T-53a-020 acceptance criteria).

**Proposal:** no group summary on Linux. New-entry notices are one notice per update batch (the body lists up to three titles and "and N more"). Dismissing a batch withdraws its single id. The rule above is not ported.

### 5.3 Observed: the private member dismiss (receiver lines 191 to 194)

`context.cancelNotification(notificationId)` followed by `context.notificationManager.cancelAll()`. This runs only for `ACTION_DISMISS_NOTIFICATION` (the app-update prompt and error "Dismiss" buttons and the restore-cancel path do not go through it). Effect: every app notice is removed, including progress notices of running jobs (defect E-01).

### 5.4 Observed: `Context.notify` permission gate (`NotificationExtensions.kt:29` to `:40`)

On API 33 and above (`Build.VERSION_CODES.TIRAMISU`) the notice is dropped silently if `POST_NOTIFICATIONS` is not granted (`PermissionChecker.checkSelfPermission` != `PERMISSION_GRANTED`). No prompt is shown from this path; the prompt is in onboarding (`PermissionStep.kt`, area:36/51).

**Proposal:** on Linux the gate is "is a notification server reachable": `gio` posts through the portal or the D-Bus server; if posting fails, the message is shown as an in-app toast with the same text. Permission is not a concept on Linux (edge E-12).

## 6. Notification channels, creation and the incognito notice

### 6.1 Observed: channel creation on every process start

- `App.onCreate` calls `setupNotificationChannels()` (`app/src/main/java/eu/kanade/tachiyomi/App.kt:106`), which calls `Notifications.createChannels(this)` inside try/catch (`App.kt:238` to `:244`). The call runs at every process start, not once per install.
- `createChannels` (`Notifications.kt:208` to `:291`):
  1. deletes the nine deprecated channel ids (`deprecatedChannels`, lines 190 to 200): `downloader_channel`, `downloader_complete_channel`, `backup_restore_complete_channel`, `library_channel`, `library_progress_channel`, `updates_ext_channel`, `downloader_cache_renewal`, `crash_logs_channel`, `library_skipped_channel`;
  2. creates four channel groups (`createNotificationChannelGroupsCompat`, lines 214 to 229): `group_backup_restore` (name `MR.strings.label_backup`, "Backup and restore"), `group_downloader` (`download_notifier_downloader_title`, "Downloader"), `group_library` (`label_library`, "Library"), `group_apk_updates` (`label_recent_updates`, "Updates");
  3. creates 13 channels (`createNotificationChannelsCompat`, lines 231 to 290). Table in `p1-data-and-config.md` section 3.2.
- **Defect E-02:** `library_progress_channel` is both in the deprecated list (line 195) and the id of the current `CHANNEL_LIBRARY_PROGRESS` (line 123). Each start deletes it and recreates it, which discards any user change to that channel (importance, sound) in Android settings.

### 6.2 Observed: the incognito notice (`App.kt:112` to `:139`)

- Driven by `basePreferences.incognitoMode().changes()` (preference key `__APP_STATE_incognito_mode`, default `false`; see section 6.3 for the key derivation) collected on `ProcessLifecycleOwner.lifecycleScope`.
- When the value becomes `true`: `disableIncognitoReceiver.register()` (a runtime-registered `BroadcastReceiver` for action `tachi.action.DISABLE_INCOGNITO_MODE`, `App.kt:274`), then `notify(ID_INCOGNITO_MODE = -701, CHANNEL_INCOGNITO_MODE)` with:
  - content title `MR.strings.pref_incognito_mode` ("Incognito mode");
  - content text `MR.strings.notification_incognito_text` ("Disable incognito mode");
  - small icon `R.drawable.ic_glasses_24dp`; `setOngoing(true)`;
  - content intent `PendingIntent.getBroadcast(app, 0, Intent("tachi.action.DISABLE_INCOGNITO_MODE"), FLAG_ONE_SHOT or FLAG_IMMUTABLE)`.
- The receiver's `onReceive` sets `incognitoMode` to `false` (`App.kt:249` to `:251`). The change flow then cancels the notice (`cancelNotification(ID_INCOGNITO_MODE)`, line 136) and unregisters the receiver.
- The channel `CHANNEL_INCOGNITO_MODE` has importance `LOW` and no badge override (`Notifications.kt:270` to `:272`).
- Incognito mode is reset to `false` on relaunch (`MainActivity.kt:216`, section 2.1 step 3).

**Proposal:** the incognito state is shown by a persistent tray notice on Linux only when the desktop has no status indicator; the action "Disable incognito mode" is `AppAction::DisableIncognito`. The header banner (`AppStateBanners`, area:36) is the primary indicator. The `DisableIncognitoReceiver` runtime broadcast is replaced by the GIO action, no receiver registration is needed.

### 6.3 Observed: how preference keys are derived

`Preference.appStateKey(key)` returns `"__APP_STATE_" + key` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43` to `:47`, prefix constant line 47). So `incognito_mode` is stored as `__APP_STATE_incognito_mode`, and `onboarding_complete` as `__APP_STATE_onboarding_complete`, and `last_version_code` as `__APP_STATE_last_version_code`.

## 7. The app-update download job (receiver to worker)

Scope: `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt` (156 LOC) and the notifier that it drives, `AppUpdateNotifier.kt`. Area:35 owns the updater rules (release check, version comparison); this section covers the start, progress, cancel and failure paths that the receiver exposes.

### 7.1 Observed: start

- `start(context, url, title?)` (`AppUpdateDownloadJob.kt:133` to `:150`):
  - constraints: `requiredNetworkType = NetworkType.CONNECTED`;
  - `OneTimeWorkRequestBuilder<AppUpdateDownloadJob>()`, tag `TAG = "AppUpdateDownload"` (line 128);
  - input data `EXTRA_DOWNLOAD_URL = "DOWNLOAD_URL"` (line 130) and `EXTRA_DOWNLOAD_TITLE = "DOWNLOAD_TITLE"` (line 131); `title` may be null;
  - `context.workManager.enqueueUniqueWork(TAG, ExistingWorkPolicy.REPLACE, request)`. REPLACE cancels a running download and starts the new one.
- `doWork()` (lines 40 to 59): `url = inputData.getString(EXTRA_DOWNLOAD_URL)`; `title = inputData.getString(EXTRA_DOWNLOAD_TITLE) ?: stringResource(MR.strings.app_name)` (default "Aniyomi"). If the URL is null or empty, returns `Result.failure()`. Otherwise it calls `setForeground(getForegroundInfo())` in a try (lines 48 to 52), catching `IllegalStateException` and logging it. Then `withIOContext { downloadApk(title, url) }` and returns `Result.success()` (the job always reports success after the download function returns, even on error).
- `getForegroundInfo()` (lines 61 to 71): `ForegroundInfo(ID_APP_UPDATER = 1, notifier.onDownloadStarted().build(), FOREGROUND_SERVICE_TYPE_DATA_SYNC on API 29 and above, else 0)`.

### 7.2 Observed: download and progress (lines 78 to 125)

1. `notifier.onDownloadStarted(title)` (line 80): content title set to `title`, content text `MR.strings.update_check_notification_download_in_progress` ("Downloading…"), small icon `android.R.drawable.stat_sys_download`, `setOngoing(true)`, one action "Cancel" (`MR.strings.action_cancel`) with the `ACTION_CANCEL_APP_UPDATE_DOWNLOAD` broadcast.
2. Progress listener (lines 82 to 98):
   - `savedProgress = 0`, `lastTick = 0L`;
   - `progress = (100 * (bytesRead.toFloat() / contentLength)).toInt()`;
   - notify only when `progress > savedProgress && currentTime - 200 > lastTick`. So updates are throttled to at most one per 200 ms (strictly greater than 200 ms apart) and only when the integer percentage rises. Values are never decreasing.
   - `notifier.onProgressChange(progress)` sets `setProgress(100, progress, false)` and `setOnlyAlertOnce(true)` (AppUpdateNotifier.kt, `onProgressChange`).
3. Request: `network.client.newCachelessCallWithProgress(GET(url), progressListener).await()` (lines 101 to 103). The cacheless client is used so the download is not served from the HTTP cache.
4. Target file: `File(context.externalCacheDir, "update.apk")` (line 106). `externalCacheDir` can be null on some devices; the code then builds `File(null, "update.apk")`, which is a relative path (edge E-13).
5. If `response.isSuccessful`: `response.body.source().saveTo(apkFile)` (line 109). Else `response.close()` and `throw Exception("Unsuccessful response")` (lines 111 to 112).
6. On success: `notifier.cancel()` (line 114) then `notifier.promptInstall(apkFile.getUriCompat(context))` (line 115). `getUriCompat` uses the FileProvider authority `<package>.provider` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/FileExtensions.kt:16` to `:18`).
7. On any exception (lines 116 to 124): `shouldCancel = e is CancellationException || (e is StreamResetException && e.errorCode == ErrorCode.CANCEL)`. If true, `notifier.cancel()`; else `notifier.onDownloadError(url)`.

### 7.3 Observed: prompt and error notices (AppUpdateNotifier.kt)

- `promptUpdate(release)` (lines 38 to 65): a notice with title `MR.strings.update_check_notification_update_available` ("New version available!"), text = release version, small icon `android.R.drawable.stat_sys_download_done`, content intent = the download broadcast (tapping the body starts the download; area:35 changes this to open the release page, behaviour E-U4). Actions: "Download" (`MR.strings.action_download`) with the download broadcast and "What's new" (`MR.strings.whats_new`) with `ACTION_VIEW` on the release link. Notice id `ID_APP_UPDATER = 1` (default of `show`).
- `promptInstall(uri)`: content text `MR.strings.update_check_notification_download_complete` ("Tap to install update"); content intent and an action both use `NotificationHandler.installApkPendingActivity(uri)` (dropped on Linux); a second action dismisses `ID_APP_UPDATE_PROMPT = 2` through `dismissNotificationPendingBroadcast`. Shown with id 2 (`show(Notifications.ID_APP_UPDATE_PROMPT)`).
- `onDownloadError(url)`: content text `MR.strings.update_check_notification_download_error` ("Download error"); actions "Retry" (`MR.strings.action_retry`) with `downloadAppUpdatePendingBroadcast(url)` and "Dismiss" with `dismissNotificationPendingBroadcast(ID_APP_UPDATE_ERROR = 3)`. Shown with id 3.

**Proposal:** `AppUpdateJob` is owned by area:35 (updater). This area's requirement: the download is started by `AppAction::StartAppUpdate { url, title }`, one active download at a time (REPLACE semantics: a new start cancels the running one), progress is throttled with the same 200 ms rule and integer-percent monotonic rule, cancel is `AppAction::CancelAppUpdate`, and the file path is `$XDG_CACHE_HOME/<app>/update.apk` from the `Paths` trait (never a relative path). On Linux the update is not installed by the app: the completed download is revealed in the file manager (`Opener::reveal`) and the user installs through the package manager (the install prompt of area:35).

## 8. Extension install services and activities (manifest-level drop)

### 8.1 Observed: MangaExtensionInstallService (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallService.kt`, 88 LOC)

- `onCreate` (lines 26 to 36): posts the foreground notice with id `ID_EXTENSION_INSTALLER = -402` on channel `CHANNEL_EXTENSIONS_UPDATE`, icon `R.drawable.ic_ani`, `setAutoCancel(false)`, `setOngoing(true)`, `setShowWhen(false)`, title `MR.strings.ext_install_service_notif` ("Installing extension…"), `setProgress(100, 100, true)` (indeterminate).
- `onStartCommand` (lines 38 to 64): reads `intent.data` (the APK uri), `EXTRA_DOWNLOAD_ID` (Long, `-1` treated as missing) and `EXTRA_INSTALLER` (serializable `BasePreferences.ExtensionInstaller`). If any is missing, `stopSelf()` and `START_NOT_STICKY`. If no installer exists yet: `PACKAGEINSTALLER` gives `PackageInstallerInstallerManga(this)`, `SHIZUKU` gives `ShizukuInstallerManga(this)`, any other value logs an error, stops and returns. Then `installer.addToQueue(id, uri)`.
- `onDestroy` calls `installer?.onDestroy()`.
- `getIntent(context, downloadId, uri, installer)` (lines 76 to 86): explicit intent to the service with `setDataAndType(uri, APK_MIME)`, extras `EXTRA_DOWNLOAD_ID` and `EXTRA_INSTALLER`.
- `EXTRA_INSTALLER = "EXTRA_INSTALLER"` (line 74).

### 8.2 Observed: MangaExtensionInstallActivity (`MangaExtensionInstallActivity.kt`, 79 LOC; anime twin)

- Translucent activity (`Theme.Translucent.NoTitleBar`, manifest). `onCreate` (lines 25 to 46) builds `Intent(ACTION_INSTALL_PACKAGE).setDataAndType(intent.data, intent.type).putExtra(EXTRA_RETURN_RESULT, true).setFlags(FLAG_GRANT_READ_URI_PERMISSION)`.
- MIUI workaround (lines 34 to 37): if `hasMiuiPackageInstaller`, set `ignoreResult = true` and `ignoreUntil = nanoTime + 1 s`.
- `startActivityForResult(installIntent, INSTALL_REQUEST_CODE = 500)`; on failure a toast with the exception message (line 44).
- `onActivityResult` (lines 48 to 57): if the MIUI ignore window is active, mark `hasIgnoredResult` and return. Otherwise, for request 500, `checkInstallationResult(resultCode)`; then `finish()`.
- `onStart` (lines 59 to 65): if a result was ignored, check it as `RESULT_CANCELED` and finish.
- `checkInstallationResult` (lines 67 to 76): `RESULT_OK` maps to `InstallStep.Installed`, `RESULT_CANCELED` to `InstallStep.Idle`, anything else to `InstallStep.Error`, passed to `MangaExtensionManager.updateInstallStep(downloadId, step)`.

### 8.3 Proposal

Dropped. APK installation of extensions does not exist on Linux. The extension mechanism of Kurobayomi is decided in area:20 (`p1-topic-wasm-component-design.md`). The `ExtensionInstaller` preference (section 13 of data) disappears with it. The user loses the ability to install Aniyomi APK extensions; the replacement is the area:20 mechanism (its own install and update flow).

## 9. Background services, workers and foreground behaviour

### 9.1 Observed: WorkManager foreground host

- `SystemForegroundService` (manifest, `foregroundServiceType="dataSync"`, `tools:node="merge"`) is the WorkManager service that hosts `setForeground` for workers. It is declared by the library, merged by the manifest. Used by `AppUpdateDownloadJob` (section 7) and by the backup and library jobs (areas 21, 22, 26).
- Unique work names used by this area: `AppUpdateDownload` (REPLACE, CONNECTED), stop = `cancelUniqueWork`.

### 9.2 Observed: HttpServerService (`app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt`, 122 LOC)

- `NAME = "HttpServerSerivce"` (typo in the Kotlin source, line 108); `EXTRA_SOURCE_ID = "$NAME.EXTRA.SOURCE.ID"` (line 110); `ACTION_STOP = "${BuildConfig.APPLICATION_ID}.$NAME.ACTION_STOP"` (line 109).
- `onStartCommand` (lines 33 to 99):
  1. `ACTION_STOP` → `stopSelf()`, `START_NOT_STICKY`.
  2. If already running (`isRunning.value`), set running false and `httpServer?.stop()` (lines 39 to 42).
  3. `sourceId = intent.getLongExtra(EXTRA_SOURCE_ID, -1)`; `server = (sourceManager.get(sourceId) as? AnimeHttpSource)?.createHttpServer()`; null → `stopSelf()`.
  4. `httpServer.start()` then `port = listeningPort`; exception → `stopSelf()` and return.
  5. `_isRunning = true`. Build a notice on `CHANNEL_HTTP_SERVER` (`-901`): large icon `R.mipmap.ic_launcher`, small icon `R.drawable.ic_dns_24dp`, action "Close" (`MR.strings.action_close`) with `PendingIntent.getService(ACTION_STOP)`, content text `AYMR.strings.http_server_is_running` ("Http server is running"), `setAutoCancel(false)`, `setOngoing(true)`, `setUsesChronometer(true)`.
  6. `startForeground(ID_HTTP_SERVER, notification, FOREGROUND_SERVICE_TYPE_DATA_SYNC)` on API 29 and above, else without type. Returns `START_STICKY`.
- `onDestroy` sets running false. `resetIsRunning()` and `isRunning` (`MutableStateFlow`) and `port` are companion members used by `MainActivity.startHttpServerService` (see section 2.9), which waits up to 5 seconds for `isRunning` to become true (`MainActivity.kt:655` to `:659`) and returns `(ready, port)`.

### 9.3 Observed: TorrentServerService (`app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt`, 188 LOC)

- Actions: `ACTION_START = "start_torrent_server"`, `ACTION_STOP = "stop_torrent_server"` (lines 141 to 142), not package-prefixed.
- `onStartCommand` (lines 44 to 61): `ACTION_START` → `startServer()` (asynchronous), `notification(applicationContext)`, `START_STICKY`; `ACTION_STOP` → `stopServer()`, `START_NOT_STICKY`; anything else → `START_NOT_STICKY`.
- `startServer` (lines 63 to 84): only when `api.echo() == ""` (no server answers). `TorrServer.registerLogCallback()` when verbose logging is on. `port = TorrServer.startServer(port = torrServerPort, path = filesDir.absolutePath, proxyMode = proxyMode.value, proxyUrl = if (mode == None) "" else torrServerProxyUrl)`. If `port != -1`: `api.setPort(port)`, `wait(10)`, `torrentServerUtils.setTrackersList()`.
- `stopServer` (lines 86 to 92): `TorrServer.stopServer()`, cancel `ID_TORRENT_SERVER = -801`, `stopSelf()`.
- `notification` (lines 94 to 127): start-again intent (`ACTION_START`) as the delete intent (`setDeleteIntent`), stop intent (`ACTION_STOP`) as the action labelled the literal string `"Stop"` (line 124, not a string resource), content text `AYMR.strings.torrentserver_is_running` ("Torrent Server is running"), content title `MR.strings.app_name`, small icon `R.drawable.ic_ani`, `setOngoing(true)`, `setUsesChronometer(true)`, foreground type DATA_SYNC on API 29 and above.
- Companion `start()` (lines 146 to 157): `startService(ACTION_START)` then `wait(10)`. `stop()` (lines 160 to 171): `startService(ACTION_STOP)`. `wait(timeout = -1)` (lines 173 to 186): `count = -20` when timeout is negative; polls `api.echo()` every 1 second until it is non-empty, returns false when `count > timeout`.
- Defect E-08: the delete intent restarts the server. Dismissing the notice (the notice is ongoing, so this happens only if the system clears it) starts the server again.

### 9.4 Observed: Shizuku

`rikka.shizuku.ShizukuProvider` is declared with `exported="true"`, `permission="android.permission.INTERACT_ACROSS_USERS_FULL"`, `multiprocess="false"` (manifest). It bootstraps Shizuku for `ShizukuInstallerManga` and `ShizukuInstallerAnime` (`rikka.shizuku.Shizuku` imports in `app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/ShizukuInstallerManga.kt` and `.../anime/installer/ShizukuInstallerAnime.kt`). Shizuku is not available on Linux. Dropped with the extension installer (section 8.3).

### 9.5 Proposal: services and workers

- No foreground service concept. A GTK application keeps running while a window exists. To keep background jobs running with the window closed, the app holds a `gio::Application::hold()` guard while any job with `keep_alive = true` is active (torrent server, HTTP server, library update, backup, app update download). Released when the last job finishes. This replaces `dataSync` foreground types and `START_STICKY`.
- The torrent and HTTP servers are started and stopped by `Service` objects in `kuro-core` with explicit `start`/`stop` functions (no intent actions). Their notices (`-801`, `-901`) become persistent desktop notices only while the server runs, with a Stop action. Ids become stable strings (`notice:torrent-server`, `notice:http-server`).
- `wait(10)` and the 20-second wait of the Kotlin helper become `tokio::time::timeout` with the same numbers (10 polls of 1 second, and 20 when no timeout is given).
- The WorkManager foreground host is not needed; `AppUpdateDownloadJob`'s foreground info is a progress notice.

## 10. Receivers and broadcast plumbing summary

| Broadcast source | Receiver | Action | Notes |
|---|---|---|---|
| Notification buttons (about 40 builders, section 4.4) | `NotificationReceiver` (manifest, `exported=false`) | `xyz.jmir.tachiyomi.mi.NotificationReceiver.*` (release applicationId; see data section 3.1) | section 4 |
| Notification content (open chapter/episode/entry) | Activities (MainActivity, ReaderActivity, PlayerActivity) | `eu.kanade.tachiyomi.SHOW_*` and component intents | section 2.2 |
| Incognito notice | runtime `DisableIncognitoReceiver` (`App.kt:246` to `:274`) | `tachi.action.DISABLE_INCOGNITO_MODE` | section 6.2 |
| WorkManager | `WorkManager` internal receivers | library | no app code |
| Widgets | six Glance receivers (presentation-widget manifest) | `APPWIDGET_UPDATE` | section 13.4 |
| Download manager completion | `MangaExtensionInstaller.kt` registers a receiver for `DownloadManager.ACTION_DOWNLOAD_COMPLETE` with `RECEIVER_EXPORTED` (`register()` at line 272 to 277, `unregister()` at line 287, `onReceive` at line 294); the anime installer has its twin | area:18 and area:19 | dropped with APK install |

No other `BroadcastReceiver` subclass exists in the app (verified by the inventory of scope files).

## 11. Activities: reader, player, web view, unlock

### 11.1 Observed: ReaderActivity intent and transitions

- `ReaderActivity.newIntent(context, mangaId: Long?, chapterId: Long?)` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt:107` to `:113`): explicit intent with extras `"manga"` and `"chapter"`, flags `FLAG_ACTIVITY_CLEAR_TOP`.
- `onCreate` (lines 143 to 160): `registerSecureActivity(this)`; open transition `shared_axis_x_push_enter` / `shared_axis_x_push_exit` (API 34 `UPSIDE_DOWN_CAKE` and above via `overrideActivityTransition(OVERRIDE_TRANSITION_OPEN, ...)`, otherwise `overridePendingTransition` with the same resources). If `viewModel.needsInit()`, reads `manga` and `chapter` (default -1); if either is -1, `finish()` and return (lines 163 to 166 region). Dismisses the new-chapter notice `NotificationReceiver.dismissNotification(this, manga.hashCode(), Notifications.ID_NEW_CHAPTERS)` (lines 170 to 175).
- `finish()` (lines 292 to 306): `viewModel.onActivityFinish()`, then the close transition `shared_axis_x_pop_enter` / `shared_axis_x_pop_exit`.
- Samsung S Pen remote actions are declared on this activity (`REMOTE_ACTION` filter and `s_pen_actions` meta-data) but no Kotlin code reads them (verified).

### 11.2 Observed: PlayerActivity intent

- `PlayerActivity.newIntent(context, animeId, episodeId, hostList: List<Hoster>? = null, hostIndex: Int? = null, vidIndex: Int? = null)` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:158` to `:174`): extras `"animeId"`, `"episodeId"`, optional `"hostIndex"`, `"vidIndex"`, `"hostList"` (serialised with `serialize()`), flag `FLAG_ACTIVITY_CLEAR_TOP`.
- `onCreate` reads the extras with defaults -1 and "" (lines 185 to 192 region) and finishes when `animeId` or `episodeId` is -1 (line 193 to 195 region). Dismisses the new-episode notice `dismissNotification(this, animeId.hashCode(), Notifications.ID_NEW_EPISODES)` (lines 195 to 199).
- The manifest declares `configChanges="orientation|screenLayout|screenSize|smallestScreenSize|keyboardHidden|keyboard|uiMode"`, `resizeableActivity="true"`, `supportsPictureInPicture="true"`, `autoRemoveFromRecents="true"`, `launchMode="singleTask"`, `theme="@style/Theme.Tachiyomi"` (manifest).
- Player transitions: no `overridePendingTransition` in the player (verified). The `player_*` animations are unreferenced (section 14).

### 11.3 Observed: WebViewActivity

- `WebViewActivity.newIntent(context, url, sourceId?, title?, isAnime = false)` (`app/src/main/java/eu/kanade/tachiyomi/ui/webview/WebViewActivity.kt:135` to `:148`): extras `url_key` (String), `source_key` (Long?), `title_key` (String?), `anime_key` (Boolean); flag `FLAG_ACTIVITY_CLEAR_TOP`. Key constants `URL_KEY = "url_key"`, `SOURCE_KEY = "source_key"`, `TITLE_KEY = "title_key"`, `ANIME_KEY = "anime_key"` (lines 130 to 133).
- Open and close transitions as in the reader (lines 38 to 52 and 96 to 110), shared-axis push and pop.
- `configChanges="uiMode|orientation|screenSize"` and `exported="false"` (manifest).

### 11.4 Proposal: S Pen and PiP

- S Pen remote actions (`s_pen_actions.xml`) are dropped. Their gestures map to keyboard and mouse actions that already exist in the reader and player plans: `CTRL_LEFT+DPAD_RIGHT` (next page) and `DPAD_LEFT` (previous page), `N` and `P` (next and previous chapter), `BACK` (back). The keyboard map is area:43 and area:46 (see `p1-ui-parity.md` section 6). User loss: none on Linux.
- Picture-in-picture (`supportsPictureInPicture`) is dropped. Wayland does not allow an application to keep a window always on top, so a "mini player" window cannot be guaranteed. Replacement: a compact mode that shrinks the window to a fixed 480 x 270 aspect-preserving size and hides the chrome (area:46 decides; P3 in this area). User loss: the floating mini window while the user works in other apps.

### 11.5 Unlock activity

`UnlockActivity` (`app/src/main/java/eu/kanade/tachiyomi/ui/security/UnlockActivity.kt`, 47 LOC) shows the biometric prompt `startAuthentication(title = "Unlock %s" with app name, confirmationRequired = false)`. On error it calls `finishAffinity()` (closes the whole task). On success it calls `SecureActivityDelegate.unlock()` and `finish()`. Owned by area:36; the Linux replacement is a passphrase or PAM/polkit prompt (area:36 decision). This area records the consequence: a failed unlock on Android closes the task; on Linux the equivalent is quitting the application.

## 12. Share, open and file access

### 12.1 Observed: toShareIntent (`app/src/main/java/eu/kanade/tachiyomi/util/system/IntentExtensions.kt:13` to `:34`)

`Uri.toShareIntent(context, type = "image/*", message = null)`:

1. `Intent(ACTION_SEND)`.
2. By scheme: `http` or `https` → `EXTRA_TEXT = uri.toString()`; `content` → optional `EXTRA_TEXT = message`, `EXTRA_STREAM = uri`. Other schemes add nothing.
3. `clipData = ClipData.newRawUri(null, uri)`; `setType(type)`; `flags = FLAG_GRANT_READ_URI_PERMISSION`.
4. `Intent.createChooser(shareIntent, MR.strings.action_share)` ("Share") with `FLAG_ACTIVITY_NEW_TASK`.

Callers: `NotificationReceiver.shareImage` (image), `shareFile` (backup, type `application/x-protobuf+gzip`), and further screens and models outside this scope (for example `ui/entries/anime/AnimeImageScreenModel.kt` and `ui/entries/anime/AnimeScreen.kt`, which the scope grep matched for `ACTION_SEND`/`toShareIntent`).

### 12.2 Observed: FileProvider

- Authority `${applicationId}.provider` (manifest). Runtime authority is `context.packageName + ".provider"` (`FileExtensions.kt:17`).
- `provider_paths.xml` whitelists: `cache-path` `"."` (name `cache_files`), `external-path` `"."` (name `ext`), `external-cache-path` `"."` (name `ext_cache_files`), `external-files-path` `"."` (name `ext_files`), `root-path` `"/storage/"` (name `ext_storage`).
- Used for: the APK install (dropped), the backup share, error-log view (`openErrorLogPendingActivity` passes a file `Uri`), `BackupNotifier.kt:175` (`destFile.getUriCompat`).

**Proposal:** `Opener` trait (`addenda/requirements-cross-platform.md`) with `open_file(path)`, `reveal(path)`, `open_url(url)`. Linux implementation: `gtk::UriLauncher` (GTK 4.10 and above) for `file://` and `https://`; `gio::AppInfo::launch_default_for_uri` as the fallback; `ashpd` OpenURI portal inside Flatpak. `reveal` opens the parent folder with `org.freedesktop.FileManager1.ShowItems` over D-Bus (KDE Dolphin and GNOME Files implement it), else the parent folder through `Opener::open_file`. Share actions become "Copy link" (clipboard) and "Show in folder". Paths are never wrapped in content URIs.

### 12.3 Observed: `ACTION_VIEW` consumers

- `openImagePendingActivity` (`NotificationHandler.kt:58` to `:69`): `image/*`, flags `NEW_TASK or GRANT_READ_URI_PERMISSION`.
- `openErrorLogPendingActivity` (receiver `:970` to `:977`): `text/plain`.
- `openUrl` (`NotificationHandler.kt:85` to `:93`): `url.toUri()`.
- Entry points: `SaveImageNotifier.kt:80` (image), library update notifiers (`HELP_WARNING_URL`, `MangaLibraryUpdateNotifier.kt:392`: `https://aniyomi.org/docs/faq/library#why-am-i-warned-about-large-bulk-updates-and-downloads`; the anime notifier has the same value at `AnimeLibraryUpdateNotifier.kt:405`).

## 13. Permissions, application attributes and network config

### 13.1 Observed: permission to behaviour

| Permission | Where it matters in code | Linux decision |
|---|---|---|
| `INTERNET` | every network call | none needed; reqwest |
| `ACCESS_NETWORK_STATE` | `NetworkStateTracker.kt:13` (`isWifi`, capabilities); `NetworkExtensions.kt` | Replaced by `gio::NetworkMonitor` plus NetworkManager (section 13.5) |
| `ACCESS_WIFI_STATE` | `NetworkExtensions.kt:32` (`wifiManager.isWifiEnabled`) and `NetworkStateTracker.kt:24` | Replaced by NetworkManager `PrimaryConnectionType` (section 13.5) |
| `MANAGE_EXTERNAL_STORAGE` | `presentation/more/settings/Preference.kt:187` (`Environment.isExternalStorageManager`) | Replaced by a folder picker through the portal (section 13.6) |
| `FOREGROUND_SERVICE` and `FOREGROUND_SERVICE_DATA_SYNC` | torrent, HTTP, app update services (section 9) | Replaced by `hold()` and notices |
| `WAKE_LOCK` | no `acquire()` call in app code (verified: only `fairLock.acquire()` and transaction element `acquire()` which are not wake locks) | Dropped |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | `PermissionStep.kt:66` and `:106`, `SettingsAdvancedScreen.kt:162` and `:166` (Settings intent `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`) | Dropped (no battery optimisation on desktop) |
| `REQUEST_INSTALL_PACKAGES` | `ContextExtensions.kt:175` (`ACTION_MANAGE_UNKNOWN_APP_SOURCES`) and installers | Dropped (section 8.3) |
| `REQUEST_DELETE_PACKAGES` | uninstall path uses `ACTION_UNINSTALL_PACKAGE` (`MangaExtensionInstaller.kt:224`) without the delete permission in app code (verified) | Dropped |
| `UPDATE_PACKAGES_WITHOUT_USER_ACTION` | no reference in app code (verified) | Dropped |
| `QUERY_ALL_PACKAGES` | `pm.getInstalledPackages(0)` and `PACKAGE_FLAGS` in `MangaExtensionLoader.kt:133`, `AnimeExtensionLoader.kt:129` to `:137`, `SettingsDownloadScreen.kt:360`, `PlayerSettingsPlayerScreen.kt:305`, `ExtensionsRestorer.kt:16` | Dropped with APK extensions (area:20) |
| `POST_NOTIFICATIONS` | `NotificationExtensions.kt:30` to `:37`; onboarding `PermissionStep.kt` | Replaced by "notification server reachable" (section 5.4) |
| `READ_APP_SPECIFIC_LOCALES` | `AppLanguageScreen.kt` (per-app language) | Replaced by area:51 (in-app language and desktop locale) |

### 13.2 Observed: application attributes

- `android:allowBackup="false"`: no Android backup (Kurobayomi backups are area:21 and area:22 files).
- `android:enableOnBackInvokedCallback="true"`: predictive back (Android 13). Replacement: Escape, Alt+Left and the mouse back button pop the navigation stack (area:36).
- `android:hardwareAccelerated="true"`: Android renderer flag. Not needed: GTK renders with GL or Cairo.
- `android:largeHeap="true"`: a Dalvik heap-size request. Dropped; memory budget is governed by the image cache of area:34.
- `android:localeConfig="@xml/locales_config"`: per-app locales list (generated by `buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt`). Replaced by area:51.
- `android:networkSecurityConfig="@xml/network_security_config"`: section 13.3.
- `android:preserveLegacyExternalStorage` and `android:requestLegacyExternalStorage`: Android 10 scoped-storage compatibility flags. Dropped; the Linux app uses normal paths.
- `android:supportsRtl="true"`: right-to-left layouts. GTK chooses the text direction from the locale; Kurobayomi must verify RTL layout for Arabic (area:51).
- `android:roundIcon`, `android:icon`, `android:label`, `android:theme`: icon and label go to the `.desktop` file and icon theme (area:49); the theme is area:49.
- `android:name=".App"`: the application class (area:01).

### 13.3 Observed: network security config (`app/src/main/res/xml/network_security_config.xml`)

- `base-config` with `cleartextTrafficPermitted="true"` (line 4 to 5), so every HTTP (non-TLS) request is allowed for every domain (sources such as HTTP-only sites rely on it).
- `trust-anchors`: `certificates src="system"` (system CAs) and `certificates src="user"` (user-installed CAs, lint suppressed with `AcceptsUserCertificates`).
- The `tools:ignore="InsecureBaseConfiguration"` attribute suppresses the lint warning for cleartext.

**Proposal:** `kuro-core::net::client()` builds reqwest with `rustls` and the platform root store (`rustls-native-certs`) plus `webpki-roots` as fallback, and allows `http://` URLs (no HSTS). Area:12 owns the client; this area's requirement is that cleartext is allowed for source requests exactly as in Aniyomi and that user-installed CAs are trusted (a corporate proxy can break sources otherwise).

### 13.4 Observed: widgets (presentation-widget manifest)

- Six receivers, each `exported="false"`, `enabled="@bool/glance_appwidget_available"`, filter `android.appwidget.action.APPWIDGET_UPDATE`:
  - `tachiyomi.presentation.widget.entries.manga.MangaUpdatesGridGlanceReceiver` (label `label_recent_updates`), meta-data `android.appwidget.provider` = `@xml/updates_grid_homescreen_widget_info`;
  - `tachiyomi.presentation.widget.entries.anime.AnimeUpdatesGridGlanceReceiver` (label `label_recent_anime_updates`), same provider;
  - `tachiyomi.presentation.widget.entries.manga.MangaUpdatesGridCoverScreenGlanceReceiver` and the anime twin: lock-screen provider `@xml/updates_grid_lockscreen_widget_info`, Samsung cover provider `@xml/updates_grid_samsung_cover_widget_info`, `com.samsung.android.sdk.subscreen.widget.support_visibility_callback = true`.
- `WidgetManagers`: `MangaWidgetManager` and `AnimeWidgetManager` are initialised in `App.onCreate` (`App.kt:151` to `:158`) with `ProcessLifecycleOwner` scope. Their behaviour is area:50.

**Proposal:** dropped on Linux. KDE Plasma widgets (plasmoids) and GNOME Shell extensions are separate packaging with their own languages; out of scope. User loss: home-screen cards of recent updates. The Updates tab (area:42) and the desktop notifications for new chapters (area:35) are the replacements.

### 13.5 Proposal: network state on Linux

- Observed Kotlin shape (`app/src/main/java/eu/kanade/tachiyomi/util/system/NetworkStateTracker.kt:8` to `:25`): `data class NetworkState(isConnected, isValidated, isWifi)` with `isOnline = isConnected && isValidated`. `isConnected` is `activeNetworkInfo?.isConnected`. `isValidated` is `NET_CAPABILITY_VALIDATED`. `isWifi = wifiManager.isWifiEnabled && capabilities.hasTransport(TRANSPORT_WIFI)`, so Wi-Fi must be enabled and be the active transport.
- Proposal: `kuro-platform::linux::network::NetworkState { is_connected, is_validated, is_wifi, is_metered }` with `is_online = is_connected && is_validated` (same rule as Kotlin).
- `is_connected` from `gio::NetworkMonitor::is_network_available()`; `is_validated` from `gio::NetworkMonitor::connectivity() == Full`.
- `is_wifi` = Wi-Fi radio enabled (NetworkManager `WirelessEnabled`) AND the primary connection type is `802-11-wireless` (NetworkManager `PrimaryConnectionType`), both read over D-Bus via `zbus`. If NetworkManager is not running, `is_wifi = false`, so "Only on Wi-Fi" (`MR.strings.connected_to_wifi`, English "Only on Wi-Fi") blocks downloads; this is the conservative choice and is recorded in edge E-15.
- `is_metered` from `gio::NetworkMonitor::network_metered()`; NetworkManager property `Metered` (values 0 unknown, 1 yes, 2 no, 3 guess-yes, 4 guess-no) is consulted when available, and `1` or `3` means metered.
- The download-only-over-Wi-Fi rule (area:23 and area:24, `AnimeDownloadJob.kt:87` to `:95` `checkNetworkState`) reads this struct.

### 13.6 Observed and proposed: storage folder access

- Observed: `MANAGE_EXTERNAL_STORAGE` plus the legacy external storage flags allow the app to write to any path. The settings screen calls `Environment.isExternalStorageManager()` (`Preference.kt:187`).
- Proposal: the download and backup directory picker uses `gtk::FileDialog::select_folder` (portal-backed inside Flatpak, native in packages). The chosen path is stored as a `PathBuf` in the settings DB and read through the `Paths` trait. The app never asks for "all files" access; the portal grants the chosen folder. Owner: area:13 (storage) and area:23/24 (download folders).

## 14. Resources: layouts, menus, animations, raw and xml (behaviour)

### 14.1 Layouts that are used

| File | Used by | Behaviour carried over |
|---|---|---|
| `layout/download_header.xml` | `MangaDownloadHeaderHolder`, `AnimeDownloadHeaderHolder` (`DownloadHeaderBinding`) | Section header (title, drag handle); `container` is a card with 0 elevation and `draggable_card_foreground` (`app/src/main/res/color/draggable_card_foreground.xml`, area:49) on drag |
| `layout/download_item.xml` | `MangaDownloadHolder`, `AnimeDownloadHolder` (`DownloadItemBinding`) | Row: drag handle (padding 16 dp), series title (single line, end ellipsis), chapter title (12 sp, single line), progress bar (indeterminate off, progress 0 to 100, bottom margin 4 dp, top margin 8 dp), counter text (12 sp, secondary colour, sample `0/10`), overflow button (paddings 10 dp horizontal, 16 dp vertical, end margin 4 dp) |
| `layout/download_list.xml` | Manga and anime download queue screens (`DownloadListBinding`) | `RecyclerView` filling the page, `clipToPadding=false` (content scrolls under the padding) |
| `layout/player_layout.xml` | `PlayerActivity` (`PlayerLayoutBinding`) | Root `ConstraintLayout` (`fitsSystemWindows=false`): mpv view `player` and Compose controls `controls` on top |
| `layout/reader_activity.xml` | `ReaderActivity` (`ReaderActivityBinding`) | Viewer container, page number pill (bottom centre), tap navigation overlay (hidden, not clickable), dialog root |
| `layout/reader_error.xml` | `PagerPageHolder`, `WebtoonPageHolder` (`ReaderErrorBinding`) | Error text "The image couldn't be loaded" (`MR` key `decode_image_error`), Retry (`action_retry`), Open in WebView (`action_open_in_web_view`, hidden until a source web URL is known) |

Note: `download_item.xml` uses `android:layout_toEndOf` attributes that have no effect inside a `ConstraintLayout` (the constraint attributes win). The Kurobayomi row follows the constraint version (the visual result is the same).

### 14.2 Menus that are used

- `menu/download_single.xml`: `MangaDownloadHolder.kt:90` and `AnimeDownloadHolder.kt:100` set `menuRes = R.menu.download_single` for the row overflow. Items and their effects are in `p1-ui-parity.md` section 5.
- `menu/chapter_download.xml`, `menu/main_nav.xml`: unreferenced, dropped (section 14.4).

### 14.3 Animations that are used

- `shared_axis_x_push_enter`, `shared_axis_x_push_exit`, `shared_axis_x_pop_enter`, `shared_axis_x_pop_exit` are used by `ReaderActivity` (lines 150 to 155 push, 299 to 304 pop) and `WebViewActivity` (lines 46 to 51 push, 103 to 108 pop). Two copies exist: `res/anim/` (pre-API-34 `overridePendingTransition`) and `res/anim-v33/` (the versioned folder for API 33 and above; the Android resource system selects `anim-v33` on 33 and above). The `anim-v33` copies use absolute distances (30 dp) and the `anim` copies use percentages of the screen width (5%p). The durations are identical: translate 300 ms with `fast_out_slow_in`, alpha 195 ms (`linear_out_slow_in` for enter, `fast_out_linear_in` for exit).

**Proposal:** a single `nav::Transition::SharedAxisX { direction: Push|Pop }` concept in `kurobayomi::nav`, rendered with `gtk::Stack` transitions: push uses `StackTransitionType::SlideLeft` (outgoing page slides left, incoming from the right) with `transition-duration = 300`, pop uses `SlideRight`. The alpha component is not reproduced (GTK Stack cannot fade and slide together); the difference is documented in `p1-ui-parity.md` section 3.

### 14.4 Dead resources (verified: zero code references)

- `layout/pref_widget_switch_material.xml`: no `PrefWidgetSwitchMaterialBinding` and no `R.layout` reference. Dropped.
- `menu/chapter_download.xml`: no `R.menu.chapter_download` reference. Dropped.
- `menu/main_nav.xml`: no `R.menu.main_nav` reference (its icon `ic_animelibrary_selector_24dp` is referenced only by this menu in `app/src/main/res`). Dropped with the icon selector (area:49 decides the icon file).
- `anim/player_enter_bottom.xml`, `player_enter_left.xml`, `player_enter_right.xml`, `player_enter_top.xml`, `player_exit_bottom.xml`, `player_exit_left.xml`, `player_exit_right.xml`, `player_exit_top.xml`, `player_fade_in.xml`, `player_fade_out.xml`: no `R.anim.player_*` reference anywhere in `app/src/main/java` (verified). They reference `@integer/player_animation_duration`, which is defined elsewhere (area:49 values).
- `raw/keep.xml`: a shrinker rule for `md_*` layouts; no such layout exists in the project.

### 14.5 Resource files that are declared but need no port

- `xml/searchable.xml`: section 2.5 (P2 search provider).
- `xml/provider_paths.xml`: dropped with FileProvider (section 12.2).
- `xml/s_pen_actions.xml`: dropped (section 11.4).
- `xml/network_security_config.xml`: section 13.3.

## 15. Behaviour summary per scope file (quick lookup)

| File | Behaviour section(s) |
|---|---|
| app/shortcuts.xml | 2.4 |
| app/src/main/AndroidManifest.xml | 1, 2.9, 8, 9, 10, 11, 12, 13 |
| NotificationHandler.kt | 4.4, 12.3 |
| NotificationReceiver.kt | 4, 5, 10 |
| Notifications.kt | 5.1, 6.1 |
| DeepLinkScreenType.kt | 2.2, 2.7 |
| DeepLinkAnimeActivity.kt, DeepLinkMangaActivity.kt | 2.7 |
| DeepLinkAnimeScreen.kt, DeepLinkMangaScreen.kt | 3.1, 3.2 |
| DeepLinkAnimeScreenModel.kt, DeepLinkMangaScreenModel.kt | 3.1, 3.2, 3.3 |
| res/anim-v33/*.xml, res/anim/shared_axis_*.xml | 14.3 |
| res/anim/player_*.xml | 14.4 |
| res/layout/download_header.xml, download_item.xml, download_list.xml | 14.1 |
| res/layout/player_layout.xml | 14.1, 11.2 |
| res/layout/pref_widget_switch_material.xml | 14.4 |
| res/layout/reader_activity.xml, reader_error.xml | 14.1, 11.1 |
| res/menu/chapter_download.xml | 14.4 |
| res/menu/download_single.xml | 14.2, `p1-ui-parity.md` section 5 |
| res/menu/main_nav.xml | 14.4 |
| res/raw/keep.xml | 14.4 |
| res/xml/network_security_config.xml | 13.3 |
| res/xml/provider_paths.xml | 12.2 |
| res/xml/s_pen_actions.xml | 11.4 |
| res/xml/searchable.xml | 2.5 |
| 10 module manifests (2 LOC each) | 1 (row 21 and 29), 13 |
| presentation-widget/src/main/AndroidManifest.xml | 13.4 |
