# 50-android-widgets — Part 1 behaviour spec

Source root: `/home/blackbox/aniyomi-src`. Widget code is under `presentation-widget/src/main/java/tachiyomi/presentation/widget/` (abbreviated `W/` below). Line numbers are from the files as read.

Scope summary: four Glance AppWidget providers (manga home, anime home, manga cover/lock screen, anime cover/lock screen), two refresh managers, one row/column formula, one lock placeholder per kind. All of them show the same thing: the most recent unread chapter or episode covers of the library, one tap per cover. Nothing in this area writes to the database. The only writes are cover image files made by Coil (see section 6).

## 0. Vocabulary

- **Entry**: a manga (`mangaId`) or an anime (`animeId`).
- **Update row**: one unread chapter (`chapterId`) or episode (`episodeId`) joined with its entry and cover data.
- **Cover**: `MangaCover` / `AnimeCover` (domain models), built from the update row.
- **Feed**: the ordered list of covers the widget shows (one per entry, newest first, capped to the grid).

## 1. Startup wiring (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/App.kt:151-158`:

1. After `setAppCompatDelegateThemeMode(...)`, the app creates `MangaWidgetManager(Injekt.get(), Injekt.get())` and calls `init(ProcessLifecycleOwner.get().lifecycleScope)`.
2. It then does the same for `AnimeWidgetManager`.
3. Both run on the process lifecycle scope, so they live as long as the process. They do not stop when the app goes to background.

Glance widgets only redraw when Android asks (`APPWIDGET_UPDATE`, `updateAll`). No `updatePeriodMillis` is set in the provider XML (see `p1-data-and-config.md`), so there is no periodic refresh.

## 2. `MangaWidgetManager.init` (OBSERVED, `W/entries/manga/MangaWidgetManager.kt:22-45`)

Steps, in order:

1. `combine( getUpdates.subscribe(read = false, after = BaseMangaUpdatesGridGlanceWidget.DateLimit.toEpochMilli()), securityPreferences.useAuthenticator().changes(), transform = { a, b -> a to b } )`.
2. `distinctUntilChanged { old, new -> old.second == new.second && old.first.map { it.chapterId }.toSet() == new.first.map { it.chapterId }.toSet() }`. Two emissions are "the same" when the lock flag is equal and the set of unread chapter ids is equal. Ordering and other fields are ignored, so a changed title or cover does not refresh the widget.
3. `onEach { try { MangaUpdatesGridGlanceWidget().updateAll(this); MangaUpdatesGridCoverScreenGlanceWidget().updateAll(this) } catch (e: Exception) { logcat(LogPriority.ERROR, e) { "Failed to update widget" } } }`. Only the two manga providers are refreshed. The `this` is the Android `Context`.
4. `flowOn(Dispatchers.Default)`, then `launchIn(scope)`.

Semantics that matter for the port:

- `combine` emits only after both inputs have emitted. `Preference.changes()` emits the current value first (`core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt:60-66`: `keyFlow.filter{...}.onStart { emit("ignition") }.map { get() }.conflate()`), so the first combined emission happens right after the DB subscription emits its first list.
- The `after` bound is computed once, when `init` runs (`DateLimit` evaluated at line 26). The window does not slide while the process stays alive. The widget's own `provideGlance` recomputes it on each render (section 4).
- Errors thrown by the upstream flows (DB or preferences) are not inside the `try`. They terminate the `combine` and therefore the whole auto-refresh chain until the next process start. The catch only covers `updateAll`.
- The anime manager is identical except: it calls `getUpdates.subscribe(seen = false, after = ...)` (named argument), compares `episodeId` sets, and refreshes `AnimeUpdatesGridGlanceWidget` and `AnimeUpdatesGridCoverScreenGlanceWidget` (`W/entries/anime/AnimeWidgetManager.kt:22-42`).

Interaction: the subscribe query is `updatesView`/`animeupdatesView` with `read = 0`/`seen = 0`. Any change to a chapter's read flag removes or adds it to the set, so marking a chapter read triggers a refresh (through the set change) unless the chapter is outside the 3-month window.

## 3. `provideGlance` (OBSERVED, `W/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt:68-110`; anime twin lines 68-110)

Steps:

1. `locked = preferences.useAuthenticator().get()`. A synchronous read of `use_biometric_lock` (default `false`). It does NOT read `lock_app_after` or the app's current unlock state (`SecureActivityDelegate.requireUnlock`). So when the lock is on, the widget shows the locked placeholder even while the app is unlocked. This is the observed behaviour and the port must keep it or document the change.
2. `containerModifier = GlanceModifier.fillMaxSize().background(background).appWidgetBackground().padding(top = topPadding, bottom = bottomPadding).appWidgetBackgroundRadius()`.
   - `background` is an `ImageProvider` of a drawable. Home providers use `appwidget_background`, cover-screen providers use `appwidget_coverscreen_background`.
   - `appWidgetBackgroundRadius()` = `cornerRadius(R.dimen.appwidget_background_radius)` (16 dp by default; system value on Android 12+).
3. `manager = GlanceAppWidgetManager(context)`; `ids = manager.getGlanceIds(javaClass)`; the grid size is taken from `ids.flatMap { manager.getAppWidgetSizes(it) }.maxBy { it.height.value * it.width.value }`. So the grid is sized from the largest-AREA instance of this provider class (see edge case E-2 in the edge-cases file).
4. `(rowCount, columnCount) = size.calculateRowAndColumnCount(topPadding, bottomPadding)` (section 5).
5. `provideContent { ... }`:
   - If `locked`: render `LockedXWidget(foreground, containerModifier)` and return. No DB subscription, no cover fetch.
   - Else: `flow = remember { getUpdates.subscribe(false, DateLimit.toEpochMilli()).map { it.prepareData(rowCount, columnCount) } }`, `data by flow.collectAsState(initial = null)`, render `UpdatesXWidget(data, contentColor = foreground, topPadding, bottomPadding, modifier = containerModifier)`.
   - `DateLimit` is evaluated inside `remember`, so it is fresh for each new composition.
   - `SizeMode.Exact` means Glance recomposes per exact size bucket.

## 4. `DateLimit` (OBSERVED, companion object, `W/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt:158-161`; anime twin same)

`val DateLimit: Instant get() = ZonedDateTime.now().minusMonths(3).toInstant()`

- Local system time zone (`ZonedDateTime.now()` uses the default zone).
- Calendar-month subtraction: 2026-10-08 minus 3 months = 2026-07-08. Month-end rule: java.time clamps to the last valid day (e.g. 2026-05-31 minus 3 months = 2026-02-28).
- Converted to epoch milliseconds (`toEpochMilli()`) for the query.
- The query uses `dateUpload > after` (strictly greater). A chapter whose `date_upload` equals the bound exactly is excluded.

Proposal for the port: compute in local time with `chrono::Local::now().checked_sub_months(chrono::Months::new(3))`, which clamps the same way; use `dateUpload > after_ms` (strict) to match.

## 5. Row and column formula (OBSERVED, `W/util/GlanceUtils.kt:29-39`)

```kotlin
fun DpSize.calculateRowAndColumnCount(topPadding: Dp, bottomPadding: Dp): Pair<Int, Int> {
    val height = this.height - topPadding - bottomPadding
    val rowCount = (height.value / 95).toInt().coerceIn(1, 10)
    val columnCount = (width.value / 64).toInt().coerceIn(1, 10)
    return Pair(rowCount, columnCount)
}
```

- Units: dp (`Dp.value` is a float). `height.value / 95` is float division, then `toInt()` truncates toward zero. `coerceIn(1, 10)` clamps.
- `95` = cover height 87 + 2 x 4 (the row's `padding(vertical = 4.dp)`).
- `64` = cover width 58 + 2 x 3 (each cell's `padding(horizontal = 3.dp)`).
- Minimum 1 row and 1 column even for a tiny instance. Maximum 10 each ("Set max to 10 children each direction because of Glance limitation", comment at line 33-34).
- Result of `Pair(rowCount, columnCount)`: the feed is capped at `rowCount * columnCount` items (max 100).

## 6. Data preparation `prepareData` (OBSERVED, `W/entries/manga/BaseMangaUpdatesGridGlanceWidget.kt:113-156`)

Input: `List<MangaUpdatesWithRelations>` from the DB flow, ordered as the SQL returns it (see section 13). Output: `ImmutableList<Pair<Long, Bitmap?>>`.

Steps:

1. `widthPx = CoverWidth.value.toInt().dpToPx` where `CoverWidth = 58.dp`, so `58` then `dpToPx`. `heightPx = CoverHeight.value.toInt().dpToPx` with `CoverHeight = 87.dp`.
   - `dpToPx` (`core/common/src/main/java/eu/kanade/tachiyomi/util/system/DensityExtensions.kt:8`) is `(this * Resources.getSystem().displayMetrics.density).toInt()`. It uses the SYSTEM display density, not the widget's own context.
2. `roundPx = context.resources.getDimension(R.dimen.appwidget_inner_radius)`: 12 dp in px for the app context.
3. `withIOContext` (`core/common/src/main/java/tachiyomi/core/common/util/lang/CoroutinesExtensions.kt:60`, dispatcher IO):
   a. `distinctBy { it.mangaId }` (anime: `animeId`). Keeps the FIRST row per entry, so the newest unread chapter's cover is used.
   b. `take(rowCount * columnCount)`.
   c. For each row in order, build `ImageRequest.Builder(context)`:
      - `.data(MangaCover(mangaId, sourceId, isMangaFavorite = true, url = coverData.url, lastModified = coverData.lastModified))`. Note `isMangaFavorite = true` is hard-coded.
      - `.memoryCachePolicy(CachePolicy.DISABLED)`
      - `.precision(Precision.EXACT)`
      - `.size(widthPx, heightPx)`
      - `.scale(Scale.FILL)`
      - if `Build.VERSION.SDK_INT < Build.VERSION_CODES.S` (i.e. below API 31): `.transformations(RoundedCornersTransformation(roundPx))`. Otherwise no transform, because "Handled by system" (the system clips the Glance `cornerRadius`).
      - `.build()`
   d. `bitmap = context.imageLoader.executeBlocking(request).image?.asDrawable(context.resources)?.toBitmap()`. `executeBlocking` is synchronous, one cover after the other.
   e. `Pair(updatesView.mangaId, bitmap)`. A failed load yields `null`, which the UI renders as the placeholder.
4. `.toImmutableList()`.

Cover fetch rules that this triggers (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverFetcher.kt`):

- The factory maps `isLibraryManga = data.isMangaFavorite` (line 330), so the widget's covers are always treated as library covers.
- `httpLoader` (line 105-): when `isLibraryManga` is true it reads or writes the library cover cache file (`coverFileLazy` = `MangaCoverCache.getCoverFile(url)`), so widget loads WRITE a file in the library cover cache. This is a side effect of the widget (listed as E-7 in the edge-case file).
- Custom cover: `MangaCoverFetcher.fetch` (lines 63-71) reads `options.extras.getOrDefault(USE_CUSTOM_COVER_KEY)`. `USE_CUSTOM_COVER_KEY = Extras.Key(true)` (line 343), so the default is `true`: the user's custom cover file is returned first when it exists. The widget does not override the default. `MangaCoverKeyer` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/MangaCoverKeyer.kt:22-31`) uses `"manga;<mangaId>;<lastModified>"` when the custom cover file exists, else `"manga;<url>;<lastModified>"`.
- Request headers: source headers from `sourceLazy.value?.headers` when the source is loaded; cache control `CACHE_CONTROL_NO_STORE` when network reads are enabled, otherwise `CACHE_CONTROL_NO_NETWORK_NO_CACHE`.
- `url == null` throws `error("No cover specified")` (line 74), which Coil turns into an error result, so the bitmap is `null`.

## 7. Compose rendering `UpdatesMangaWidget` (OBSERVED, `W/components/manga/UpdatesMangaWidget.kt:30-98`; anime twin identical with anime names)

Outer: `Box(contentAlignment = Center, modifier = modifier)`.

States:

1. `data == null` -> `CircularProgressIndicator(color = contentColor)` (loading; no timeout).
2. `data.isEmpty()` -> `Text(text = stringResource(MR.strings.information_no_recent), style = TextStyle(color = contentColor))`. Text: "No recent updates".
3. Otherwise:
   - `(rowCount, columnCount) = LocalSize.current.calculateRowAndColumnCount(topPadding, bottomPadding)`. This is the size of THIS widget instance, not the max area used for the feed. So a smaller instance shows only the first covers of the feed. A larger instance cannot exceed the feed cap because the cap was set from the max-area instance.
   - `Column(modifier = fillMaxHeight(), verticalAlignment = CenterVertically, horizontalAlignment = CenterHorizontally)`.
   - `for i in 0 until rowCount`: `coverRow = (0 until columnCount).mapNotNull { j -> data.getOrNull(j + i * columnCount) }`. Cell index is row-major, so the first row holds feed items 0..columnCount-1.
   - If `coverRow` is non-empty: `Row(modifier = padding(vertical = 4.dp).fillMaxWidth(), horizontalAlignment = CenterHorizontally, verticalAlignment = CenterVertically)`. A short last row is centred, not left-aligned.
   - Each cell: `Box(modifier = padding(horizontal = 3.dp), contentAlignment = Center)`.
   - Each cell's intent: `Intent(LocalContext.current, Class.forName(Constants.MAIN_ACTIVITY))` with `action = Constants.SHORTCUT_MANGA` (`eu.kanade.tachiyomi.SHOW_MANGA`), `putExtra(Constants.MANGA_EXTRA, mangaId)` (key `"manga"`, a Long), flags `FLAG_ACTIVITY_NEW_TASK | FLAG_ACTIVITY_CLEAR_TOP`, and `addCategory(mangaId.toString())`. The category makes each PendingIntent unique (the comment cites issuetracker 238793260).
   - Anime: `SHOW_ANIME` (`eu.kanade.tachiyomi.SHOW_ANIME`) and extra key `"anime"`.
   - The cell modifier: `UpdatesMangaCover(cover = bitmap, modifier = GlanceModifier.clickable(actionStartActivity(intent)))`.
   - Covers without a bitmap are still clickable.

## 8. Cover tile `UpdatesMangaCover` (OBSERVED, `W/components/manga/UpdatesMangaCover.kt:16-48`)

- Constants: `val CoverWidth = 58.dp`, `val CoverHeight = 87.dp`.
- `Box(modifier.size(width = CoverWidth, height = CoverHeight).appWidgetInnerRadius())`, where `appWidgetInnerRadius()` = `cornerRadius(R.dimen.appwidget_inner_radius)` (12 dp).
- If `cover != null`: `Image(provider = ImageProvider(cover), contentDescription = null, modifier = fillMaxSize().appWidgetInnerRadius(), contentScale = ContentScale.Crop)`.
- Else: `Image(provider = ImageProvider(R.drawable.appwidget_cover_error), contentDescription = null, modifier = fillMaxSize(), contentScale = ContentScale.Crop)`. The placeholder has no radius of its own; it inherits the box radius.
- `contentDescription = null` for every cover: no accessible name (see the UI parity file for the desktop improvement).

## 9. Locked placeholder `LockedMangaWidget` (OBSERVED, `W/components/manga/LockedMangaWidget.kt:22-45`; anime twin identical)

- `intent = Intent(LocalContext.current, Class.forName(Constants.MAIN_ACTIVITY)).apply { addFlags(FLAG_ACTIVITY_NEW_TASK) }`. No action, no extra. So tapping only opens the app.
- `Box(modifier = modifier.clickable(actionStartActivity(intent)).padding(8.dp), contentAlignment = Center)`.
- `Text(text = stringResource(MR.strings.appwidget_unavailable_locked), style = TextStyle(color = foreground, fontSize = 12.sp, textAlign = TextAlign.Center))`. Text: "Widget not available when app lock is enabled".

## 10. Provider configurations (OBSERVED)

| Provider class | foreground | background drawable | topPadding | bottomPadding | Manifest XML |
|---|---|---|---|---|---|
| `MangaUpdatesGridGlanceWidget` (`W/entries/manga/MangaUpdatesGridGlanceWidget.kt:9-15`) | `ColorProvider(R.color.appwidget_on_secondary_container)` | `appwidget_background` | 0 dp | 0 dp | `updates_grid_homescreen_widget_info` |
| `AnimeUpdatesGridGlanceWidget` (`W/entries/anime/AnimeUpdatesGridGlanceWidget.kt:9-15`) | same | same | 0 dp | 0 dp | `updates_grid_homescreen_widget_info` (same XML as manga home) |
| `MangaUpdatesGridCoverScreenGlanceWidget` (`W/entries/manga/MangaUpdatesGridCoverScreenGlanceWidget.kt:9-14`) | `ColorProvider(Color.White)` | `appwidget_coverscreen_background` | 0 dp | 24 dp | `updates_grid_lockscreen_widget_info` + Samsung `updates_grid_samsung_cover_widget_info` |
| `AnimeUpdatesGridCoverScreenGlanceWidget` (`W/entries/anime/AnimeUpdatesGridCoverScreenGlanceWidget.kt:9-14`) | same | same | 0 dp | 24 dp | same XMLs as manga cover |

Manifest receivers (`presentation-widget/src/main/AndroidManifest.xml`), each `exported="false"`, `enabled="@bool/glance_appwidget_available"` (the bool comes from the Glance AAR, not this repo):

- `MangaUpdatesGridGlanceReceiver`, label `@string/label_recent_updates`, home XML.
- `AnimeUpdatesGridGlanceReceiver`, label `@string/label_recent_anime_updates`, home XML.
- `MangaUpdatesGridCoverScreenGlanceReceiver`, label `@string/label_recent_updates`, lockscreen XML + Samsung XML + `com.samsung.android.sdk.subscreen.widget.support_visibility_callback = true`.
- `AnimeUpdatesGridCoverScreenGlanceReceiver`, label `@string/label_recent_anime_updates`, same extras as manga cover.

Note: the home manga and the home anime providers share one provider XML, so they share the same sizes, preview and category.

## 11. Click routing in the app (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:465-500`)

`when (intent.action)`:

- `SHOW_ANIMELIB` -> `HomeScreen.Tab.AnimeLib()`.
- `SHOW_LIBRARY` -> `HomeScreen.Tab.Library()`.
- `SHOW_MANGA` -> `val idToOpen = intent.extras?.getLong(MANGA_EXTRA) ?: return false`; `navigator.popUntilRoot()`; `HomeScreen.Tab.Library(idToOpen)`.
- `SHOW_ANIME` -> `val idToOpen = intent.extras?.getLong(ANIME_EXTRA) ?: return false`; `navigator.popUntilRoot()`; `HomeScreen.Tab.AnimeLib(idToOpen)`.
- `SHOW_RECENTLY_UPDATED` (`Constants.SHORTCUT_UPDATES`) -> `HomeScreen.Tab.Updates`.
- Any other action falls through to the rest of the `when`.

A missing extra returns `false`, which means the intent is not handled and the app opens at its default screen.

Locked placeholder: no action, so it opens the app with the normal startup lock (`app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt`, outside this scope; see area:36-ui-shell-navigation-security-webview).

## 12. Constants, text and keys (OBSERVED)

| Name | Value | Where |
|---|---|---|
| Cover width | 58 dp | `UpdatesMangaCover.kt:16`, `UpdatesAnimeCover.kt:16` |
| Cover height | 87 dp | `UpdatesMangaCover.kt:17`, `UpdatesAnimeCover.kt:17` |
| Row height step | 95 dp (87 + 2 x 4) | `GlanceUtils.kt:36` |
| Column width step | 64 dp (58 + 2 x 3) | `GlanceUtils.kt:37` |
| Row padding | `vertical = 4.dp` | `UpdatesMangaWidget.kt:63` |
| Cell padding | `horizontal = 3.dp` | `UpdatesMangaWidget.kt:71` |
| Locked padding | 8 dp; text 12 sp centred | `LockedMangaWidget.kt:33,40` |
| Cover screen bottom padding | 24 dp | `MangaUpdatesGridCoverScreenGlanceWidget.kt:13` |
| Cap rows / cols | 1..10 each | `GlanceUtils.kt:36-37` |
| DB limit | 500 rows (before dedupe) | `domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt:21` (`subscribe(read, after)` -> `subscribeWithRead(..., limit = 500)`) |
| Window | 3 calendar months | `BaseMangaUpdatesGridGlanceWidget.kt:160` |
| Inner radius | 12 dp (`appwidget_inner_radius`) | `values/dimens.xml:3` |
| Background radius | 16 dp (`appwidget_background_radius`); system value on API 31+ | `values/dimens.xml:2`, `values-v31/dimens.xml:2` |
| Placeholder colour | `#1F888888` (`cover_placeholder`) | `presentation-core/src/main/res/values/colors.xml:5` |
| Shortcut / action strings | `eu.kanade.tachiyomi.SHOW_MANGA`, `SHOW_ANIME`, `SHOW_RECENTLY_UPDATED`, `SHOW_LIBRARY`, `SHOW_ANIMELIB` | `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:14-18` |
| Extra keys | `"manga"`, `"anime"` (Long ids) | `Constants.kt:7,9` |
| Main activity class name | `eu.kanade.tachiyomi.ui.main.MainActivity` | `Constants.kt:11` |

User-visible message keys (MR / AYMR):

| Key | Module | English text | Used by |
|---|---|---|---|
| `information_no_recent` | `i18n` base (`i18n/src/commonMain/moko-resources/base/strings.xml:947`) | "No recent updates" | manga and anime body (empty state). The anime widget does NOT use the anime-specific key `information_no_recent_anime` ("Nothing watched recently", `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml:534`). |
| `appwidget_unavailable_locked` | `i18n` base (`strings.xml:990`) | "Widget not available when app lock is enabled" | locked placeholders |
| `loading` | `i18n` base (`strings.xml:170`) | "Loading…" | XML initial layouts only (`appwidget_loading.xml`, `appwidget_coverscreen_loading.xml`) |
| `label_recent_updates` | `i18n` base (`strings.xml:28`) | "Updates" | manifest label, manga receivers; also the launcher shortcut `app/shortcuts.xml` |
| `label_recent_anime_updates` | `i18n-aniyomi` base (`strings.xml:414`) | "Anime Updates" | manifest label, anime receivers |
| `appwidget_updates_description` | `i18n` base (`strings.xml:989`) | "See your recently updated library entries" | provider XML `android:description` |

Code references: `MR.strings.information_no_recent` at `UpdatesMangaWidget.kt:46` and `UpdatesAnimeWidget.kt:46`; `MR.strings.appwidget_unavailable_locked` at `LockedMangaWidget.kt:37` and `LockedAnimeWidget.kt:37`.

## 13. Query semantics feeding the widget (OBSERVED)

Manga: `data/src/main/sqldelight/view/updatesView.sq` (view `updatesView`):

```sql
CREATE VIEW updatesView AS
SELECT mangas._id AS mangaId, mangas.title AS mangaTitle, chapters._id AS chapterId, chapters.name AS chapterName,
       chapters.scanlator, chapters.read, chapters.bookmark, chapters.last_page_read, mangas.source,
       mangas.favorite, mangas.thumbnail_url AS thumbnailUrl, mangas.cover_last_modified AS coverLastModified,
       chapters.date_upload AS dateUpload, chapters.date_fetch AS datefetch
FROM mangas JOIN chapters ON mangas._id = chapters.manga_id
WHERE favorite = 1 AND date_fetch > date_added
ORDER BY date_fetch DESC;
```

Query used by the widget (`getUpdatesByReadStatus`):

```sql
SELECT * FROM updatesView WHERE read = :read AND dateUpload > :after LIMIT :limit;
```

Parameters from the widget: `read = 0` (false), `after = DateLimit ms`, `limit = 500`.

Points that matter:

- Only favourites (`favorite = 1`).
- Only chapters whose `date_fetch > date_added` (fetched after the manga was added to the library, so the first import is excluded).
- The window is on `dateUpload` (the source's upload date), not on `date_fetch`.
- The view has `ORDER BY date_fetch DESC`, but `getUpdatesByReadStatus` has no `ORDER BY` of its own. SQLite does not guarantee the view's ordering survives the outer query. The widget's "newest first" depends on it (edge case E-4).
- `LIMIT 500` applies to chapter rows before `distinctBy`, so a library with many chapters per title can yield fewer than `rows * cols` covers (E-5).

Anime: `data/src/main/sqldelightanime/view/animeupdatesView.sq` (view `animeupdatesView`) is the same shape with `animes`/`episodes`, `episodes.seen`, `episodes.date_upload AS dateUpload`, `episodes.date_fetch AS datefetch`, and `getUpdatesBySeenStatus` with `seen = :seen AND dateUpload > :after LIMIT :limit`. Widget call: `subscribe(seen = false, after)`.

Repository mapping: `data/src/main/java/tachiyomi/data/updates/manga/MangaUpdatesRepositoryImpl.kt:30-39,41-70` maps the row to `MangaUpdatesWithRelations` and builds `MangaCover(mangaId, sourceId, isMangaFavorite = favorite, url = thumbnailUrl, lastModified = coverLastModified)`. The anime twin is `data/src/main/java/tachiyomi/data/updates/anime/AnimeUpdatesRepositoryImpl.kt:34-43`.

Interactor (`domain/src/main/java/tachiyomi/domain/updates/manga/interactor/GetMangaUpdates.kt`): `subscribe(read, after) = repository.subscribeWithRead(read, after, limit = 500)`. Anime: `GetAnimeUpdates.subscribe(seen, after) = repository.subscribeWithSeen(seen, after, limit = 500)`.

Database flow: `AndroidAnimeDatabaseHandler.subscribeToList` = `block(db).asFlow().mapToList(queryDispatcher)` with `queryDispatcher = Dispatchers.IO` (`data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt:20,65-67`). SQLDelight re-emits the list when the underlying tables change. Manga side uses the same pattern.

## 14. Lock flag semantics (OBSERVED)

- Key `use_biometric_lock`, default `false` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:13`).
- Read by the widgets at render time (`BaseMangaUpdatesGridGlanceWidget.kt:69`) and by the managers as a flow (`MangaWidgetManager.kt:28`).
- Toggling it changes the flow, so the manager refreshes both providers (the lock state is part of the distinct key).
- The widgets do not consult `lock_app_after` (default `0`, `SecurityPreferences.kt:15`) or the in-process unlock state. The app's own lock UI is in area:36-ui-shell-navigation-security-webview.

## 15. Error handling (OBSERVED)

| Failure | What happens |
|---|---|
| Cover missing (`thumbnail_url` null) | Coil error, bitmap `null`, placeholder shown, cell still clickable. |
| Cover download/IO error | Same as above (`executeBlocking(...).image` is null). |
| Coil throws | Propagates out of `prepareData` (no try/catch around `executeBlocking` in the widget). The flow is not guarded, so that composition fails and the widget keeps its last rendered state. |
| DB flow throws | Not caught. Widget keeps the last drawn data or stays on the spinner. Manager flow stops (section 2). |
| `updateAll` throws | Caught and logged as `Failed to update widget` at ERROR (`logcat`). |
| `MainActivity` class not found | `Class.forName` throws `ClassNotFoundException` during composition. Not guarded. |
| Intent extra missing on tap | `MainActivity` returns `false` for that action; the app opens normally. |

## 16. Concurrency (OBSERVED)

- Manager flows: `combine`/`distinctUntilChanged` on `Dispatchers.Default` (`flowOn`), launched in the process lifecycle scope.
- DB flow: emissions on the DB handler dispatcher (`Dispatchers.IO`).
- `provideGlance` is a `suspend` function run by Glance on its own worker. Covers are loaded inside `withIOContext` (Dispatchers.IO) with `executeBlocking`, one at a time.
- `provideContent` composition runs on the Glance composition thread; `collectAsState(initial = null)` shows the spinner until the first list.

## 17. Interactions with other areas

- area:42-ui-updates-history-stats-storage-upcoming: the in-app Updates screen (`app/src/main/java/eu/kanade/presentation/updates/manga/MangaUpdatesScreen.kt:66`, anime twin) uses the same `information_no_recent` text. The widget's query is the same family as that screen's query.
- area:34-images-covers-cache: `MangaCoverFetcher`, `MangaCoverCache`, keyers. Widget covers go through that pipeline (E-7).
- area:36-ui-shell-navigation-security-webview: app lock (`SecureActivityDelegate`, `UnlockActivity`), `MainActivity` routing and deep links.
- area:05-data-repos-entries-items and area:04-db-anime-schema-migrations: the views and repositories.
- area:26-library-update-jobs: new chapters and episodes are what create update rows.
- area:35-data-misc-updater-notification-export: notifications open the same `SHOW_MANGA` / `SHOW_ANIME` routes (`app/src/main/java/eu/kanade/tachiyomi/data/notification/NotificationReceiver.kt:633-820`).
- area:49-theme-and-design-system: the Tachiyomi colour tokens and the Material You dynamic palette.

## 18. Desktop mapping (PROPOSAL, summary; full design in the other files)

- Feed rule: unchanged (window, unread, favourite, dedupe, cap). Grid size comes from the applet's own size, not a max-area guess.
- Lock: the same rule (`use_biometric_lock` = true means "show locked placeholder, no data") is applied in the CLI output and the applet. The applet never receives titles or covers while locked.
- Refresh: 60 s poll of the CLI, plus a manual refresh. No push.
- Click: `kurobayomi --open manga:<id>` / `anime:<id>` (the same routes as section 11; area:36-ui-shell-navigation-security-webview owns the URI/argument parser).
- Covers: cache-only read (no network from the CLI). Missing covers show the placeholder. The widget's side effect of filling the library cover cache is not reproduced (the Updates screen still fetches normally).
