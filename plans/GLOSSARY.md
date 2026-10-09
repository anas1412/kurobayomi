# Kurobayomi glossary

Every Aniyomi, Tachiyomi, Mihon and Kotlin/Android term used in the area plans, with its meaning in the Kotlin source and its Rust counterpart in Kurobayomi. Read this file before any area document. Each entry names the owning area.

Scope: the 54 area directories (01 to 54, plus 47b and 49b), `RISKS-AND-DECISIONS.md`, the catalogs in `plans/catalogs/`, the addenda in `plans/addenda/`, and the task table `plans/TASKS.csv` (3,801 tasks).

## 0. Conventions

- **OBSERVED** = what the Aniyomi Kotlin does. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Line numbers are given where they were checked.
- **Proposal:** = what Kurobayomi should do. Rust names are proposals unless the entry says they are recorded decisions (`RISKS-AND-DECISIONS.md`, "ADR").
- `area:<id>` names an area directory. Links point to the area overview.
- Task IDs are `T-<NN><part letter>-<3 digits>`. Priority and effort below come from `plans/TASKS.csv` at the time of writing.
- Priorities: P0 = needed for daily reading and watching, P1 = parity, P2 = long tail, P3 = Android-only or obsolete.

## 1. Product and repository names

| Term | OBSERVED meaning | Kurobayomi counterpart | Owner |
|---|---|---|---|
| Aniyomi | The Android app this plan rewrites: a Tachiyomi/Mihon fork with manga and anime. Licence: `LICENSE` at the repository root (Apache-2.0). | Not used as a brand. No "Aniyomi" in the product name, icons or update URLs (RISKS section 6). Tasks `T-36a-022` and `T-36a-023` must point at the Kurobayomi repository. | [area:54](54-build-release-distribution-docs/00-overview.md), [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| Tachiyomi / Mihon | Upstream manga reader lineage named in the plans. The checked files `app/src/main/java/eu/kanade/tachiyomi/App.kt` and `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt` carry no copyright header. | Ported files keep their copyright header (RISKS section 6). | [area:54](54-build-release-distribution-docs/00-overview.md) |
| Kurobayomi | The Rust and GTK 4 rewrite at `/home/blackbox/kurobayomi`. Application crate `kurobayomi` (`Cargo.toml`). | Application crate `kurobayomi`, library crates `kuro-*` (section 13). | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| Injekt | Kotlin DI library `uy.kohesive.injekt`. `Injekt.get<T>()` resolves a service. Imported at `app/src/main/java/eu/kanade/tachiyomi/App.kt:69`. The app calls `dev.mihon.injekt.patchInjekt()` at `App.kt:84` (import at `:24`). | No DI container. One `AppContext` struct, with one `OnceLock` per lazy service (decision D1). | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `DomainModule` | `app/src/main/java/eu/kanade/domain/DomainModule.kt:208`: `class DomainModule : InjektModule`. Registers singletons with `addSingletonFactory` and factories with `addFactory` (for example `:211` and `:212`). Area:10 counts 22 singleton and 156 factory registrations. | Explicit construction in `AppContext` (`kuro-core/src/app_context.rs`). | [area:10](10-app-domain-layer/00-overview.md) |
| `addSingletonFactory` / `addFactory` | Injekt registration calls. A singleton is created once. A factory is created on each `get()`. | `OnceLock<T>` field for a singleton; a plain constructor call for a factory. | [area:01](01-foundation-app-bootstrap/00-overview.md) |

## 2. Kotlin and Android platform terms

| Term | OBSERVED meaning | Kurobayomi counterpart | Owner |
|---|---|---|---|
| Kotlin coroutine | Structured concurrency through `CoroutineScope`, `Job`, `launch`, `async`. | `tokio` tasks on a multi-thread runtime owned by `kuro-core` (`rt-multi-thread`, `sync`, `time`, `macros`). Blocking work uses `spawn_blocking` (area:01 section 2.3). | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `Dispatchers.IO` / `Dispatchers.Default` | Thread pools for blocking I/O and CPU work. | `tokio` blocking pool (`spawn_blocking`) and worker threads. | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `Flow` | Cold asynchronous stream. Database queries are exposed as `subscribe` methods on interactors. | Latest-value state: `tokio::sync::watch`. Event streams: `tokio::sync::broadcast` with capacity 64 (area:08 and area:10 rx notes). | [area:08](08-domain-library-category-history-updates/00-overview.md) |
| `StateFlow` | Hot state holder that keeps the latest value. | `watch::Sender<T>` / `watch::Receiver<T>`. | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `Channel` (`kotlinx.coroutines.channels`) | Queue between coroutines. `Channel(BufferOverflow.DROP_OLDEST)` is used for tab switches. `Channel(UNLIMITED)` carries `Event.OpenChapter` and `HistoryCleared`. | Tab switch: `async_channel::bounded(1)` with drain-then-send. Events: `async_channel::unbounded()` consumed on the GTK main context. Events are not dropped (E-EVT-1). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `shareIn(replay 1)` | Shares one Flow across collectors and replays the last value. | `watch` channel. | [area:08](08-domain-library-category-history-updates/00-overview.md) |
| RxJava `Observable` | Legacy source calls: `fetchAnimeDetails` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt:160`), `fetchEpisodeList` (`:178`), `fetchVideoList` (`:184`). Default bodies throw `UnsupportedOperationException()`. | Dropped. `async fn` only. The RxJava bridge is not ported (area:01 section 5). | [area:16](16-source-api-anime/00-overview.md) |
| Voyager `ScreenModel` / `StateScreenModel` | Presentation state holder from `cafe.adriel.voyager.core.model`. `StateScreenModel<S>` holds one state value. 54 files use it, for example `app/src/main/java/eu/kanade/presentation/more/settings/screen/data/CreateBackupScreen.kt:130`. | A GTK view model: a Rust struct owned by the GTK main thread and updated by events from `async-channel` (rule A2 in area:40). File pattern `src/<screen>_model.rs`. | [area:38](38-ui-entry-details-manga/00-overview.md), [area:39](39-ui-entry-details-anime/00-overview.md) |
| Interactor (naming convention) | Not a framework type. One use case per class, for example `app/src/main/java/eu/kanade/domain/source/manga/interactor/`. Exposes a one-shot `await` call and/or a `subscribe` Flow. | Proposal: a free `async fn`, or a struct with one `async fn run`. Interactors never touch GTK (area:40). | [area:10](10-app-domain-layer/00-overview.md) |
| `awaitSuccess` | OkHttp extension that throws on a non-success response. | `reqwest` response with an explicit status check in `kuro-net` (area:12). | [area:12](12-network-stack/00-overview.md) |
| Compose, `@Composable`, `LazyColumn`, `LazyVerticalGrid`, `AlertDialog` | Android declarative UI. Presentation code lives under `app/src/main/java/eu/kanade/presentation/`. | GTK 4 widgets built in Rust. `LazyColumn` maps to `gtk::ListView` with `gtk::SignalListItemFactory`. `LazyVerticalGrid` maps to `gtk::GridView` with columns computed from a 128 dp adaptive width. `AlertDialog` maps to a modal `gtk::Window` (`transient_for`), or `gtk::AlertDialog` (GTK 4.10+) for plain confirmations. | [area:01](01-foundation-app-bootstrap/00-overview.md), [area:42](42-ui-updates-history-stats-storage-upcoming/00-overview.md) |
| `ColorScheme` (Compose) | Colour objects in `app/src/main/java/eu/kanade/presentation/theme/colorscheme/`. | Theme tokens and the GTK-theme-following stylesheet (`T-49ba-001`, area:49). | [area:49](49-theme-and-design-system/00-overview.md) |
| Coil (`AsyncImage`, `ImageLoader`, `ImageRequest`, `MemoryCache`, `DiskCache`, `Fetcher`, `Decoder`, `Keyer`) | Android image library. Replaced by `ImageService`, `decode::decode`, `MemoryCache`, `DiskImageCache`, `keys::cache_key` (area:34 sections 2 to 7). Crossfade is a `gtk::Picture` opacity transition of 150 ms. | `reqwest`, an on-disk cache, `gdk::Texture::from_bytes`. | [area:34](34-images-covers-cache/00-overview.md) |
| OkHttp / `Interceptor` | HTTP client and interceptors in `app/src/main/java/eu/kanade/tachiyomi/network/` and `app/src/main/java/eu/kanade/tachiyomi/network/interceptor/`. | `reqwest` client with `rustls` in `kuro-net`. Each interceptor becomes a middleware function (area:12). | [area:12](12-network-stack/00-overview.md) |
| `Conscrypt` / `Security.insertProviderAt` | Android TLS provider installation. | Dropped. `rustls` is the TLS stack (area:01 section 5). | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `kotlinx.serialization` `Json` | JSON codec for extension data, `Memo`, `SerializableVideo`, `SerializableHoster`. | `serde_json` with `#[serde(rename_all = "camelCase")]` and `#[serde(default)]` where Kotlin has a default (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `@ProtoNumber` / protobuf | Backup wire format. Field numbers are fixed, for example `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupEpisode.kt:34`: `@ProtoNumber(501) var fillermark: Boolean = false`. | `prost` with the same field numbers (area:21). | [area:21](21-backup-format-and-create/00-overview.md) |
| SQLDelight | Kotlin SQL compiler. `.sq` files under `data/src/main/sqldelight/` and `data/src/main/sqldelightanime/` generate typed queries. Migrations are `.sqm` files, for example `data/src/main/sqldelight/migrations/14.sqm`. | `rusqlite` with hand-written statements in `kuro-db`. Statement names keep the `.sq` names (areas 03 and 04). | [area:03](03-db-manga-schema-migrations/00-overview.md), [area:04](04-db-anime-schema-migrations/00-overview.md) |
| `ColumnAdapter` | SQLDelight codec between a Kotlin type and a column. `FetchTypeColumnAdapter` (`data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:43`) and `MemoColumnAdapter` (same file) are examples; the file also holds the update-strategy adapters at lines 33 and 40. | `FromSql` and `ToSql` implementations in `kuro-db` (area:04). | [area:04](04-db-anime-schema-migrations/00-overview.md) |
| `user_version` | SQLite schema version pragma. | `PRAGMA user_version` read and written by `Db::open` and the migrator in `kuro-core/src/migrate/` (area:01 section 2.2). | [area:11](11-app-migrations/00-overview.md) |
| WorkManager | Android background job API. Examples named in the plans: `MangaDownloadJob`, `BackupRestoreJob`, `DelayedAnimeTrackingUpdateJob`, `DelayedMangaTrackingUpdateJob`. | Tokio tasks, `SchedulerBackend::hold` (area:53), and the `kuro-downloads` actor (area:23). | [area:53](53-platform-manifest-intents-workers/00-overview.md) |
| `BroadcastReceiver`, `LocalBroadcastManager`, `ACTION_*` | Intent broadcasts. `NotificationReceiver` is replaced by a notice dispatcher (`T-53a-017`). | Proposal: `ExtEvent` over `async_channel`, a `Command` enum over tokio `mpsc`, and the `app.notice` GAction (area:53 section 4). | [area:53](53-platform-manifest-intents-workers/00-overview.md) |
| `Intent`, `PendingIntent`, deep links | Android messages and links. Scheme `aniyomi`, for example intent filter scheme `aniyomi` with host `anilist-auth` (`app/src/main/AndroidManifest.xml:250-260`, `T-28a-040`). | `DeepLink` enum in `kuro-core/src/deep_link.rs` (area:01). Desktop file registers `x-scheme-handler/aniyomi` (area:32 O-6). | [area:53](53-platform-manifest-intents-workers/00-overview.md) |
| Notification channel | Android notification category with an id. | `NotificationCategory` table in `kuro-core/src/notify.rs` and a `Notifier` trait in `kuro-platform`, using `gio::Notification` and `org.freedesktop.Notifications` through `notify-rust` (RISKS section 4, `T-01a-023`). | [area:35](35-data-misc-updater-notification-export/00-overview.md) |
| `FLAG_SECURE` | Android window flag that blocks screenshots. | The setting is kept and shows "Not available on Linux" (RISKS section 4, area:34 Q-1). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `BiometricPrompt` | Android biometric unlock, used by `SecureActivityDelegate.kt`. | Dropped for v1. Passphrase lock, opt-in, default off (ADR-09). fprintd is a later option (`T-36a-033`). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `Activity` | Android screen host, for example `MainActivity.kt` and `TrackLoginActivity`. | `gtk::ApplicationWindow` in a `gtk::Application`. | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `WebView` | Android browser control for Cloudflare challenges and logins (`WebViewUtil.kt`). | `webkit6` (WebKitGTK) window behind the `WebViewHost` trait in `kuro-platform` (ADR-04). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `SharedPreferences` | Android key-value settings store. | `settings.toml` and `state.toml` (decision D6, area:01). | [area:02](02-preferences-system/00-overview.md) |
| `PreferenceStore` and `Preference<T>` | Kotlin interfaces in `core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt`. `getEnum` is declared at line 27. | `Pref<T>` in `kuro-prefs` (area:02). `PrefBinding<T>` binds it to a widget (`src/prefs_bind.rs`, area:01). | [area:02](02-preferences-system/00-overview.md) |
| `getEnum` storage form | `PreferenceStore.kt:27-42`: serializer `it.name`, deserializer `enumValueOf(it)`, and on `IllegalArgumentException` the default value. Enums in preferences are stored by name, and an unknown name falls back to the default. | Preference enums are stored as names. Unknown names fall back to the default (area:02). | [area:02](02-preferences-system/00-overview.md) |
| `TriState` (preference enum) | `core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt:3-7`: `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`. Comments: "Disable filter", "Enabled with 'is' filter", "Enabled with 'not' filter". `next()` cycles DISABLED to ENABLED_IS to ENABLED_NOT to DISABLED (lines 9-15). | `enum TriState { #[default] Disabled, EnabledIs, EnabledNot }` in `kuro-core/src/prefs/tri_state.rs` (area:02). The checkbox widget `tristate(check, &Pref<TriState>)` (area:01). | [area:02](02-preferences-system/00-overview.md) |
| `TriState` (source filter) | Source-API filter state. `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:21-23`: `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2`. Manga twin: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt:18`. | Proposal: `TriStateValue { #[default] Ignore = 0, Include = 1, Exclude = 2 }` (area:41 D-4). Unknown integers map to `Ignore` with a warning (R-16). | [area:41](41-ui-browse-anime/00-overview.md) |

Conflict: both types are named `TriState` in some plan documents. Keep the preference enum as `TriState` and the source-filter state as `TriStateValue` (section 14).

## 3. Enum values and stored integers

| Kotlin enum | Declared in (repo-relative) | Values, declaration order | Stored as | Rust (proposal) | Owner |
|---|---|---|---|---|---|
| `UpdateStrategy` (manga) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt:10` | `ALWAYS_UPDATE`, `ONLY_FETCH_ONCE` | SQL: ordinal 0 and 1 (`DatabaseAdapter.kt:33`) | `UpdateStrategy { AlwaysUpdate = 0, OnlyFetchOnce = 1 }`, default `AlwaysUpdate` | [area:07](07-domain-entries-items/00-overview.md) |
| `AnimeUpdateStrategy` (anime) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:10` | `ALWAYS_UPDATE`, `ONLY_FETCH_ONCE` | SQL: ordinal (`DatabaseAdapter.kt:40`) | Same as manga | [area:16](16-source-api-anime/00-overview.md) |
| `FetchType` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10` | `Seasons` (calls only `getSeasonList`), `Episodes` (calls only `getEpisodeList`) | SQL: ordinal (`DatabaseAdapter.kt:47`). Decode: `FetchType.entries.getOrElse(value) { FetchType.Episodes }` (`:43-46`). Column default `1` (`data/src/main/sqldelightanime/dataanime/animes.sq:33`). | `FetchType { Seasons = 0, Episodes = 1 }`, default `Episodes` | [area:16](16-source-api-anime/00-overview.md) |
| `Hoster.State` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt` (nested in `Hoster`) | `IDLE`, `LOADING`, `READY`, `ERROR` | Not persisted | `HosterState` enum used by the loader (area:45) | [area:45](45-player-core/00-overview.md) |
| `ChapterType` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:14` | `Opening`, `Ending`, `Recap`, `MixedOp`, `Other` | Inside the video JSON | Enum with the same five variants (area:16) | [area:16](16-source-api-anime/00-overview.md) |
| `ReadingMode` | `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:14-60` | `DEFAULT` = `0x00000000`, `LEFT_TO_RIGHT` = `0x00000001`, `RIGHT_TO_LEFT` = `0x00000002`, `VERTICAL` = `0x00000003`, `WEBTOON` = `0x00000004`, `CONTINUOUS_VERTICAL` = `0x00000005`. `MASK = 0x00000007` (line 60). | Low 3 bits of `viewerFlags` | Enum with the same values (area:43) | [area:43](43-reader-core/00-overview.md) |
| `DataSaver` | `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:118-123` | `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` | Preference name | `DataSaver { None, BandwidthHero, WsrvNl, ResmushIt }`. The three services are dropped (RISKS section 4), and `NONE` is kept so stored values parse. | [area:17](17-source-app-layer/00-overview.md) |
| `ThemeMode` | `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt` | `LIGHT`, `DARK`, `SYSTEM` | Preference name | `ThemeMode { Light, Dark, System }`, keeping the Kotlin names as keys | [area:49](49-theme-and-design-system/00-overview.md) |
| `AppTheme` | `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7-31` | 22 values: `DEFAULT`, `MONET`, `CLOUDFLARE`, `COTTONCANDY`, `DOOM`, `GREEN_APPLE`, `LAVENDER`, `MATRIX`, `MIDNIGHT_DUSK`, `MOCHA`, `SAPPHIRE`, `NORD`, `STRAWBERRY_DAIQUIRI`, `TAKO`, `TEALTURQUOISE`, `TIDAL_WAVE`, `YINYANG`, `YOTSUBA`, `MONOCHROME`, `DARK_BLUE`, `HOT_PINK`, `BLUE` | Preference name | Same 22 names. Default is `DEFAULT` (inherit the GTK theme, ADR-11). Theme colours are set by the GTK theme. | [area:49](49-theme-and-design-system/00-overview.md) |
| `NavStyle` | `app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt:21` | `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE` | Preference name | Four variants kept, because the names are in backups (ADR-11) | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `StartScreen` | `app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt` | `ANIME`, `MANGA`, `UPDATES`, `HISTORY`, `BROWSE` | Preference name | Enum with the same five variants (area:36) | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| `SetMigrateSorting.Mode` | `app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt:14-17` | `ALPHABETICAL`, `TOTAL` | Preference name | `MigrationMode { Alphabetical, Total }` | [area:41](41-ui-browse-anime/00-overview.md) |
| `SetMigrateSorting.Direction` | Same file, `:19-22` | `ASCENDING`, `DESCENDING` | Preference name | `MigrationDirection { Ascending, Descending }` | [area:41](41-ui-browse-anime/00-overview.md) |
| Tracker IDs | `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20-37` | MyAnimeList `1`, AniList `2`, Kitsu `3`, Shikimori `4`, Bangumi `5`, Komga `6`, MangaUpdates `7`, Kavita `8`, Suwayomi `9`, Simkl `101`, Jellyfin `102` | `manga_sync.sync_id` and `anime_sync.sync_id` (INTEGER) | `i64` constants with the same numbers (area:27) | [area:27](27-tracking-core/00-overview.md) |
| `EntryKind` | Proposal only | `Manga`, `Anime` (order conflict, section 14) | Not stored | One generic interactor per concept, parameterised by `EntryKind` (areas 07 and 10) | [area:07](07-domain-entries-items/00-overview.md) |
| `HosterSource` | Proposal only | `Downloaded`, `Http`, `Local` | Not stored | Used by the player loader (area:45) | [area:45](45-player-core/00-overview.md) |

### 3.1 Viewer flags

- `viewerFlags` is a `Long` column. The manga reading mode is `viewerFlags and ReadingMode.MASK.toLong()` (`app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt:18`).
- `SetMangaViewerFlags.awaitSetReadingMode(id, flag)` replaces only the masked bits: `manga.viewerFlags.setFlag(flag, ReadingMode.MASK.toLong())` (`app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt:12-17`).
- Anime viewer flags hold skip-intro and airing bits (`app/src/main/java/eu/kanade/domain/entries/anime/interactor/SetAnimeViewerFlags.kt`).
- Proposal: keep the same `i64` column and masks, and expose typed setters only. The full bit layout is in [area:07](07-domain-entries-items/00-overview.md) and [area:43](43-reader-core/00-overview.md).

## 4. Source and extension terms

| Term | OBSERVED meaning (repo-relative) | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| `AnimeSource` | Interface `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt:17`. Members with line numbers: `id: Long` (`:22`), `name` (`:27`), `lang` (`:29`), `supportsLatest` (`:35`), `getFilterList()` (`:40`), `getPopularAnime(page)` (`:48`), `getLatestUpdates(page)` (`:56`), `getSearchAnime(page, query, filters)` (`:66`), `getAnimeEpisodeUpdate` (`:84`), `getAnimeSeasonUpdate` (`:107`), `supportsRelatedAnime` (`:121`), `getRelatedAnimeList(anime)` (`:129`), `getHosterList(episode)` (`:139`, default throws `IllegalStateException("Not used")`), `getVideoList(hoster)` (`:148`), `getSeasonList(anime)` (`:151`, default throws `UnsupportedOperationException()`), `getVideoList(episode)` (`:154`), `getAnimeDetails(anime)` (`:166`), `getEpisodeList(anime)` (`:172`). | `trait AnimeSource` in `kuro-source` (area:16). Method names keep the Kotlin names in snake_case. | [area:16](16-source-api-anime/00-overview.md) |
| `MangaSource` / `CatalogueSource` | Manga twins under `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/`. | `trait MangaSource` and `trait CatalogueSource` in `kuro-source` (area:15). | [area:15](15-source-api-manga/00-overview.md) |
| `AnimeCatalogueSource` | Browse-capable anime source, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeCatalogueSource.kt`. | Same trait family as `CatalogueSource` (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `ConfigurableAnimeSource` | Source that exposes a settings tree, built with `PreferenceScreen` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt`). | Proposal: a `ConfigurableSource` trait with a preference-screen method (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `ResolvableSource` / `ResolvableAnimeSource` | Interface named in areas 15, 16 and 17 for resolving a URL to an entry. | Proposal: `trait ResolvableSource` (areas 15 and 17). | [area:17](17-source-app-layer/00-overview.md) |
| `UnmeteredSource` | Marker interface, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/UnmeteredSource.kt`. | Marker trait. Its effect is specified in area:12. | [area:12](12-network-stack/00-overview.md) |
| `HttpSource` / `ParsedHttpSource` / `AnimeHttpSource` / `ParsedAnimeHttpSource` | Base classes for HTTP sources. `Parsed*` variants parse HTML. | Trait with default methods. HTML parsing uses the `scraper` crate in place of Jsoup (areas 14 and 15). | [area:15](15-source-api-manga/00-overview.md) |
| `StubSource` | Placeholder for an uninstalled source, kept so that the library still knows the source. `getOrStub` returns the installed source or a stub (area:17). | `SourceHandle<S> { Installed(Arc<S>), Stub(StubSource) }` with `StubSource { id, lang, name }` (area:17). | [area:17](17-source-app-layer/00-overview.md) |
| Local source | Built-in source that reads local files, for manga and anime. Language code `"other"` (area:17 Q4). | `kuro-source-local` crate. `local_source(paths)` constructor exposed by area:14 (area:17 Q5). | [area:14](14-source-local-and-metadata/00-overview.md) |
| Source id | Long derived from the source name and language. Ids are frozen into backups. Lowercasing keeps Unicode (`str::to_lowercase`, U-11, R-05). | `i64` in `kuro-source`, same derivation (areas 18 and 20 Q6). | [area:20](20-extension-strategy-redesign/00-overview.md) |
| Source pin | Sources can be pinned in the Sources page. Pins are bit flags (area:17). | Pin flags per source (area:17). | [area:17](17-source-app-layer/00-overview.md) |
| Extension | An installable Android package (APK) that provides one or more sources. | Dropped as an APK. Replaced by built-in Rust sources, then WASM components (ADR-01). | [area:18](18-extensions-manga/00-overview.md), [area:19](19-extensions-anime/00-overview.md), [area:20](20-extension-strategy-redesign/00-overview.md) |
| Extension repository | A URL that serves `repo.json` and a package index. | `extension_repos` SQLite table (area:20 Q5). Existing `repo.json` format kept (area:20). | [area:20](20-extension-strategy-redesign/00-overview.md) |
| Trust | Record that a user accepted a given extension version from a publisher. Trust keys are `pkg:versionCode:digest` (area:18 D0d). Trust keys are not carried into backups (ADR-06). | Trust model in `kb-ext::trust` (`T-20a-060`, "Trust model for repos and per-version keys", P1). | [area:18](18-extensions-manga/00-overview.md) |
| `Untrusted` | State of an extension version the user has not trusted yet (area:18). | Same state name. | [area:18](18-extensions-manga/00-overview.md) |
| `Filter` / `FilterList` (source search filters) | Source-defined search controls. `AnimeFilter` is a sealed class (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt:3`) with the subclasses `Header`, `Separator`, `Select<V>`, `Text`, `CheckBox`, `TriState` (state `STATE_IGNORE` default), `Group<V>` (lines 4 to 27). | `enum Filter` and `FilterList` in `kuro-source` (areas 15 and 16). | [area:16](16-source-api-anime/00-overview.md) |
| `AnimesPage` / `MangasPage` | Paged result of `getPopularAnime`, `getLatestUpdates` and `getSearchAnime`. | `AnimesPage` and `MangasPage` structs (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `SAnime` | Source-side anime model. Interface `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:8`. Fields: `url`, `title`, `thumbnail_url`, `background_url`, `artist`, `author`, `status: Int`, `description`, `genre`, `update_strategy: AnimeUpdateStrategy`, `fetch_type: FetchType`, `season_number: Double`, `initialized: Boolean`, `memo: JsonObject`. | `struct SAnime` in `kuro-source/src/anime.rs` with the same field names (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `SEpisode` | Source-side episode model. Interface `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SEpisode.kt:8`. Fields: `url`, `name`, `episode_number: Float`, `fillermark: Boolean`, `scanlator: String?`, `date_upload: Long`, `summary: String?`, `preview_url: String?`, `memo: JsonObject`. | `struct SEpisode` with the same fields. `episode_number` is `f32` (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `SChapter` / `SManga` | Manga twins of `SEpisode` and `SAnime`. | `struct SChapter` and `struct SManga` (area:15). | [area:15](15-source-api-manga/00-overview.md) |
| `Page` (manga) | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt:11`: `class Page(val index: Int, val url: String = "", var imageUrl: String? = null, var uri: Uri?)`. `number` is `index + 1`. | `struct Page { index, url, image_url }` (area:15). The `uri` field is dropped (Android `Uri`). | [area:15](15-source-api-manga/00-overview.md) |
| `Video` | Playable stream, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt:30`. Fields: `videoUrl`, `videoTitle`, `resolution: Int?`, `bitrate: Int?`, `headers`, `preferred: Boolean`, `subtitleTracks: List<Track>`, `audioTracks: List<Track>`, `timestamps: List<TimeStamp>`, `mpvArgs`, `ffmpegStreamArgs`, `ffmpegVideoArgs`, `internalData`, `initialized`, `memo`. | `struct Video` (area:16). `ffmpegStreamArgs` and `ffmpegVideoArgs` are dropped (RISKS section 4, area:20 Q10). | [area:16](16-source-api-anime/00-overview.md) |
| `Track` (subtitle or audio, source side) | `Video.kt:11`: `data class Track(val url: String, val lang: String)`. | Subtitle and audio track struct (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `TimeStamp` | Marker inside a video, `Video.kt:23`, with a `ChapterType` (used by skip-intro). | `struct TimeStamp` (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `Hoster` | One mirror or host of an episode. `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt:10`: `hosterUrl`, `hosterName`, `videoList: List<Video>?`, `internalData: String`, `lazy: Boolean`, `memo: JsonObject`, plus a transient `status: State`. | `struct Hoster` (areas 16 and 45). | [area:16](16-source-api-anime/00-overview.md) |
| `NO_HOSTER_LIST` | `Hoster.kt` companion constant `"no_hoster_list"`. `List<Video>.toHosterList()` wraps a flat video list in one hoster with this name. | Same constant (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `HosterLoader` | App class that resolves hosters to one video. `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt:16`, `getBestVideo` at `:78`. | `HosterResolver` trait (area:45). | [area:45](45-player-core/00-overview.md) |
| Two-step resolve | `getHosterList(episode)` then `getVideoList(hoster)`. Lazy hosters load on demand. | Same two-step API (area:45). | [area:45](45-player-core/00-overview.md) |
| `Memo` | `JsonObject` extra data on `SAnime`, `SEpisode` and `Hoster`. Stored as UTF-8 JSON text (`MemoColumnAdapter` in `DatabaseAdapter.kt`). Source keys are namespaced `aniyomi.*` (doc comment in `SEpisode.kt`). | `pub struct Memo(pub serde_json::Map<String, serde_json::Value>)` (area:04). Keys are kept verbatim. | [area:04](04-db-anime-schema-migrations/00-overview.md) |
| `SeasonAnime` | Domain model `domain/src/main/java/aniyomi/domain/anime/SeasonAnime.kt`: `anime`, `totalCount`, `seenCount`, `bookmarkCount`, `fillermarkCount`, `latestUpload`, `fetchedAt`, `lastSeen`. `seen` is `totalCount == seenCount`. `hasStarted` is `seenCount > 0`. | `struct SeasonAnime` (area:07). | [area:07](07-domain-entries-items/00-overview.md) |

## 5. Entries, items and read or seen state

An entry is one title: a manga or an anime. An item is a unit inside an entry: a chapter for manga, an episode for anime.

| Term | OBSERVED meaning (with the schema column list) | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Manga | Domain entry for a comic or novel. Table `mangas` in the manga database. | `Manga` in `kuro-domain`, table `mangas` (area:03). | [area:07](07-domain-entries-items/00-overview.md) |
| Anime | Domain entry for a series or film. Table `animes` in the anime database. Insert column list at `data/src/main/sqldelightanime/dataanime/animes.sq:167` includes `fetch_type`, `season_flags`, `season_number`, `season_source_order`, `parent_id`, `background_url`, `background_last_modified`, `calculate_interval`, `memo`. | `Anime` in `kuro-domain`, table `animes` (area:04). | [area:07](07-domain-entries-items/00-overview.md) |
| Entry | Shared word for a manga or an anime in backups and the UI. | `EntryKind` (section 3). | [area:07](07-domain-entries-items/00-overview.md) |
| Chapter | Manga item. Table `chapters` in `data/src/main/sqldelight/data/chapters.sq:3-21`: `_id`, `manga_id`, `url`, `name`, `scanlator`, `read` (Boolean), `bookmark` (Boolean), `last_page_read`, `chapter_number` (REAL), `source_order`, `date_fetch`, `date_upload`, `last_modified_at` (default 0), `version` (default 0), `is_syncing` (default 0). Foreign key to `mangas`, cascade delete. | `Chapter` in `kuro-domain`, table `chapters` (area:03). `chapter_number` is `f64`. | [area:07](07-domain-entries-items/00-overview.md) |
| Episode | Anime item. Table `episodes` in `data/src/main/sqldelightanime/dataanime/episodes.sq:4-27`: `_id`, `anime_id`, `url`, `name`, `scanlator`, `seen` (Boolean), `bookmark` (Boolean), `last_second_seen`, `total_seconds`, `episode_number` (REAL), `source_order`, `date_fetch`, `date_upload`, `last_modified_at` (default 0), `version` (default 0), `is_syncing` (default 0), `summary`, `preview_url`, `fillermark` (Boolean), `memo` (BLOB JsonObject, default `'{}'`). Foreign key to `animes`, cascade delete. | `Episode` in `kuro-domain`, table `episodes` (area:04). | [area:07](07-domain-entries-items/00-overview.md) |
| Read | Manga chapter flag `chapters.read`. Set through `SetReadStatus` (`app/src/main/java/eu/kanade/domain/items/chapter/interactor/SetReadStatus.kt`). | `read: bool` (area:07). | [area:07](07-domain-entries-items/00-overview.md) |
| Seen | Anime episode flag `episodes.seen`. Set through `SetSeenStatus` (`app/src/main/java/eu/kanade/domain/items/episode/interactor/SetSeenStatus.kt`). The UI calls episodes "seen" and chapters "read". | `seen: bool` (area:07). | [area:07](07-domain-entries-items/00-overview.md) |
| Bookmark | User mark on an item (`bookmark` column). Not a browser bookmark. | `bookmark: bool`. | [area:07](07-domain-entries-items/00-overview.md) |
| Fillermark | Anime episode flag `episodes.fillermark` (Boolean). Copied from the source's `SEpisode.fillermark` (`app/src/main/java/eu/kanade/domain/items/episode/model/Episode.kt:15` and `:29`). Filtered by `fillermarkedFilter` (`app/src/main/java/eu/kanade/domain/items/episode/model/EpisodeFilter.kt:21-25`). Stored in backups as `@ProtoNumber(501)`. Download gate in section 7. | `fillermark: bool` (area:07). | [area:07](07-domain-entries-items/00-overview.md) |
| `fillermarkedFilter` | Filter on the episode list, typed `TriState` (section 2). `DISABLED` shows all episodes. `ENABLED_IS` shows only fillermarked episodes ("is" filter). `ENABLED_NOT` hides fillermarked episodes ("not" filter). Source: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1687-1691`. | `TriState` (area:39). | [area:39](39-ui-entry-details-anime/00-overview.md) |
| Season (anime) | A group of episodes for one series season. Created from `getSeasonList` when the entry's `FetchType` is `Seasons`. Season rows are `animes` rows whose `parent_id` points to the parent (`animes.sq:167`). `season_number` orders them (`Double`). | `Season` rows with `parent_id` (areas 04 and 07). | [area:07](07-domain-entries-items/00-overview.md) |
| `SyncSeasonsWithSource` | Reconciles source seasons with stored rows. An empty source list is exempt from deletion (`SyncSeasonsWithSource.kt:31`). The chapter and episode twins are at `app/src/main/java/eu/kanade/domain/items/chapter/interactor/SyncChaptersWithSource.kt:56` and `app/src/main/java/eu/kanade/domain/items/episode/interactor/SyncEpisodesWithSource.kt:54`. | `sync_seasons` (area:10). | [area:10](10-app-domain-layer/00-overview.md) |
| Related anime | Titles linked to an entry through `getRelatedAnimeList`. Synced by `SyncRelatedAnimeWithSource`. | `RelatedAnime` rows (areas 04 and 10). | [area:39](39-ui-entry-details-anime/00-overview.md) |
| `NoSeasonsException`, `NoEpisodesException` | Domain exceptions in `tachiyomi.domain.entries.anime.model`, imported by `AnimeScreenModel.kt:90` and `:98`. | Proposal: `Error::NoSeasons` and `Error::NoEpisodes` (area:07). | [area:07](07-domain-entries-items/00-overview.md) |
| Scanlator | Manga translation group, column `chapters.scanlator`. Excluded scanlators are stored in table `excluded_scanlators` (sql-01 catalog). | `Option<String>` on `Chapter` and table `excluded_scanlators` (area:03). | [area:03](03-db-manga-schema-migrations/00-overview.md) |
| Chapter number, episode number | Stored as SQL `REAL`. The source model uses `Float` for episodes (`SEpisode.kt`). | `f64` in the database and models (area:07 and area:16). | [area:07](07-domain-entries-items/00-overview.md) |
| `SyncChaptersWithSource`, `SyncEpisodesWithSource` | Reconcile the source list with the stored rows: add, update, remove. Read, seen and bookmark state carry over. Ordering and date rules are in `10-app-domain-layer/p1-topic-chapter-episode-sync.md`. | `sync_items` (area:10). | [area:10](10-app-domain-layer/00-overview.md) |
| `UpdateManga`, `UpdateAnime` | Interactors that write entry metadata from `SManga` or `SAnime` (detail merge and the two mapper directions named in area:10), declared in `app/src/main/java/eu/kanade/domain/entries/manga/interactor/UpdateManga.kt` and its anime twin. | `update_entry` (area:10). | [area:10](10-app-domain-layer/00-overview.md) |
| `fetchInterval` | Stored `Int` on each entry, the number of hours between library update checks. Range and presets are in area:26. | `fetch_interval: i32` (area:26). | [area:26](26-library-update-jobs/00-overview.md) |
| `calculate_interval` | Column that enables automatic interval calculation for an anime. | `calculate_interval: bool` (area:04). | [area:26](26-library-update-jobs/00-overview.md) |
| `MangaFetchInterval`, `AnimeFetchInterval` | Interval presets used by the settings UI. | Presets in `kuro-domain` (area:26). | [area:26](26-library-update-jobs/00-overview.md) |
| `date_added` | Epoch time when an entry joined the library. | `date_added: i64` (area:03). | [area:08](08-domain-library-category-history-updates/00-overview.md) |
| `favorite` | Boolean: the entry is in the library. | `favorite: bool` (area:08). | [area:08](08-domain-library-category-history-updates/00-overview.md) |
| `favorite_modified_at` | Timestamp of the last library membership change, in seconds, kept for backup compatibility (RISKS ADR-05, area:03 Q3). | `favorite_modified_at: i64` in seconds, with the `secs` helper (R-24). | [area:03](03-db-manga-schema-migrations/00-overview.md) |
| `last_modified_at` | Timestamp in seconds. Mixed units with milliseconds elsewhere (RISKS ADR-05). | `last_modified_at: i64` in seconds. | [area:03](03-db-manga-schema-migrations/00-overview.md) |
| `time_read` | Reading time in history, in milliseconds. Dropped by `DELETE FROM history` (area:42 D-18). | `time_read: i64` (area:06). | [area:06](06-data-repos-history-track-source-misc/00-overview.md) |
| Reading-time clamp | Reading duration is clamped to 0 through 12 h per flush (R-14, area:08). Recording semantics are in `T-08a-019` ("History recording: upsert semantics, session timer, throttle, incognito", P0). | Clamp in history recording (area:08). | [area:08](08-domain-library-category-history-updates/00-overview.md) |

## 6. Library, categories, history and updates

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Library | Entries with `favorite = true`, shown in the Library tab grouped by category. | Library query in `kuro-domain` (area:08). | [area:37](37-ui-library/00-overview.md) |
| Category | User group of library entries. Table `categories` (`data/src/main/sqldelight/data/categories.sq:1-7`): `_id`, `name`, `sort` (INTEGER), `flags` (INTEGER), `hidden` (INTEGER, default 0). The table exists in both databases (sql-01 catalog). | `Category` in `kuro-domain`, `CategoryRepo { db, kind, stmts }` (area:05). | [area:08](08-domain-library-category-history-updates/00-overview.md) |
| `default_category`, `default_anime_category` | Preferences that name the category used when an entry is added. | Same keys, kept for backup compatibility (area:02). | [area:08](08-domain-library-category-history-updates/00-overview.md) |
| Library update | Background check of sources for new items. Concurrency and interval rules are in area:26. | Library update task in `kuro-downloads` and the `kuro-platform` scheduler (area:26). | [area:26](26-library-update-jobs/00-overview.md) |
| Updates tab | Chronological feed of new items from library updates. | `UpdatesRepo::by_status` (area:42). | [area:42](42-ui-updates-history-stats-storage-upcoming/00-overview.md) |
| History | Read and watch log. Table `history` (`data/src/main/sqldelight/data/history.sq:3-9`): `_id`, `chapter_id` (UNIQUE, foreign key to `chapters`), `last_read` (Date), `time_read` (INTEGER). Anime history is the statement and table `animehistory` (sql-01 catalog). | `HistoryRepo` (area:06). | [area:06](06-data-repos-history-track-source-misc/00-overview.md) |
| Upcoming | Calendar of airing anime and upcoming releases. The week start depends on the locale (U-12, area:42 D-24). | `UpcomingScreen` (area:42). | [area:42](42-ui-updates-history-stats-storage-upcoming/00-overview.md) |
| Stats | Library and reading statistics screen. | `StatsScreen` (area:42). | [area:42](42-ui-updates-history-stats-storage-upcoming/00-overview.md) |
| Storage screen | Downloaded size per entry. | `StorageScreen` (area:42). | [area:42](42-ui-updates-history-stats-storage-upcoming/00-overview.md) |
| Global search | Search across all enabled sources at once. Kotlin creates `Executors.newFixedThreadPool(5)` per model (area:41 A-5). | One process-wide semaphore with 5 permits (area:41 D-6). | [area:41](41-ui-browse-anime/00-overview.md) |
| Migrate (source migration) | Moves an entry to another source. Migrate screens are listed in area:40 and area:41. The "Delete downloaded" option is OFF by default (U-13). | `MigrateEntry` action (areas 40 and 41). | [area:41](41-ui-browse-anime/00-overview.md) |
| Duplicate | Entries that are already in the library. `DuplicateAnimeDialog.kt` and `DuplicateMangaDialog.kt` show the warning. | `DuplicateEntryDialog` (areas 38 and 39). | [area:39](39-ui-entry-details-anime/00-overview.md) |

## 7. Downloads

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Download | Saving a chapter or episode to disk for offline use. Managers: `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt` and the manga twin. | `kuro-downloads` actor (areas 23 and 24). | [area:25](25-downloads-shared/00-overview.md) |
| Download queue | Ordered list of pending downloads. Anime and manga each have a queue tab (`AnimeDownloadQueueTab`, `MangaDownloadQueueTab`). | Queue model in `src/download_queue_model.rs` (area:23). | [area:25](25-downloads-shared/00-overview.md) |
| `download_slots` | Number of parallel downloads. Default 1, UI range 1 to 5 (ADR-08, area:25 Q1). | `download_slots: u8`. | [area:25](25-downloads-shared/00-overview.md) |
| `download_speed_limit` | Speed cap. Aniyomi's anime downloader ignores it. Kurobayomi applies it to both kinds (ADR-08, area:25 Q2). | Rate limit in `kuro-downloads` (area:25). | [area:25](25-downloads-shared/00-overview.md) |
| `pref_download_fillermarked` (`downloadFillermarkedItems`) | Default `false` (`domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:37`). When `false`, fillermarked episodes are excluded: `AnimeDownloadManager.kt:431` runs `episodes.filterNot { it.fillermark }`. | `download_fillermarked_items: bool`, default `false` (area:24). | [area:24](24-downloads-anime/00-overview.md) |
| Orphaned download | Files of a source that is no longer installed. Kept, logged as a warning, with a P2 action "Clean orphaned downloads" (ADR-08, area:25 Q3). | Same behaviour. | [area:25](25-downloads-shared/00-overview.md) |
| Permanent delete | Deleting a download removes the files at once. No trash (ADR-08, area:25 Q4). | Same behaviour. | [area:25](25-downloads-shared/00-overview.md) |
| `DeleteChapterDownload`, `DeleteEpisodeDownload` | Interactors that delete downloads: `app/src/main/java/eu/kanade/domain/download/manga/interactor/DeleteChapterDownload.kt` and `app/src/main/java/eu/kanade/domain/download/anime/interactor/DeleteEpisodeDownload.kt`. Non-cancellable (area:10). | Delete path in area:10. | [area:10](10-app-domain-layer/00-overview.md) |
| `DownloadIndex` | Trait that counts downloaded items per entry (areas 23 and 24). | `trait DownloadIndex { fn count(&self, kind: EntryKind, entry_id: i64) -> u32; }`. | [area:23](23-downloads-manga/00-overview.md) |

## 8. Tracking

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Tracker | Remote service that stores progress: MyAnimeList, AniList, Kitsu, Shikimori, Bangumi, MangaUpdates, Simkl, Kavita, Komga, Suwayomi, Jellyfin (section 3). Base interface `app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt:10`. | `trait Tracker` in `kuro-track` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| `MangaTracker`, `AnimeTracker` | Interfaces `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt:24` and `app/src/main/java/eu/kanade/tachiyomi/data/track/AnimeTracker.kt:24`. | `MangaTracker` and `AnimeTracker` traits in `kuro-track` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| `BaseTracker` | Shared base class. Each tracker extends it, for example `class Kavita(id: Long) : BaseTracker(id, "Kavita"), EnhancedMangaTracker, MangaTracker` (`app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt:23`). | Default trait methods in `kuro-track` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| Track | Link between one entry and one tracker entry. Table `manga_sync` (`data/src/main/sqldelight/data/manga_sync.sq:3-21`): `_id`, `manga_id`, `sync_id` (tracker id), `remote_id`, `library_id`, `title`, `last_chapter_read` (REAL), `total_chapters`, `status`, `score` (REAL), `remote_url`, `start_date`, `finish_date`, `private` (Boolean, default 0). `UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE`. Anime twin: table `anime_sync` (`data/src/main/sqldelightanime/dataanime/anime_sync.sq:3`) with `last_episode_seen` (area:27). | `struct Track` and `TrackRecord` in `kuro-track`, tables `manga_sync` and `anime_sync` (areas 03, 04, 27). | [area:27](27-tracking-core/00-overview.md) |
| Track upsert | Writing a track updates the existing row and keeps its `_id` (area:27). | Same behaviour. | [area:27](27-tracking-core/00-overview.md) |
| Enhanced tracker | Tracker that never shows a search dialog. It binds automatically to entries from a fixed source class and talks to the same server as that source. Interfaces: `app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedMangaTracker.kt` and `app/src/main/java/eu/kanade/tachiyomi/data/track/EnhancedAnimeTracker.kt`. Methods: `accept(source)`, `getAcceptedSources()`, `loginNoop()`, `match(anime)` or `match(manga)`, `isTrackFrom(track, entry, source)`, `migrateTrack(track, entry, newSource)`. | Proposal: `trait EnhancedTracker` in `kuro-track` (area:33). | [area:33](33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/00-overview.md) |
| Enhanced manga trackers | Kavita (`app/src/main/java/eu/kanade/tachiyomi/data/track/kavita/Kavita.kt:23`), Komga (`app/src/main/java/eu/kanade/tachiyomi/data/track/komga/Komga.kt:20`), Suwayomi (`app/src/main/java/eu/kanade/tachiyomi/data/track/suwayomi/Suwayomi.kt:18`). | Same three, in `kuro-track` (area:33). | [area:33](33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/00-overview.md) |
| Enhanced anime tracker | Jellyfin (`app/src/main/java/eu/kanade/tachiyomi/data/track/jellyfin/Jellyfin.kt:20`). | Same (area:33). | [area:33](33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/00-overview.md) |
| `bindEnhancedTrackers` | Binds enhanced trackers when an entry is added: `app/src/main/java/eu/kanade/domain/track/manga/interactor/AddMangaTracks.kt:81-105`. | `bind_enhanced` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| Delayed tracking queue | Progress updates that failed offline. Kept in an Android preference file named `tracking_queue`, keyed by `trackId.toString()`. Retried by WorkManager jobs (area:27). The Android queue is not imported (area:27). | Proposal: `tracking_queue` table with retry logic in `kuro-track` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| `TrackError` | Error type returned by tracker calls. `index_to_score` returns `Result<f64, TrackError>` (area:32 O-2). | `enum TrackError` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| `SyncChapterProgressWithTrack`, `SyncEpisodeProgressWithTrack` | Push progress to a tracker. The enhanced-sync rule is at `app/src/main/java/eu/kanade/domain/track/manga/interactor/SyncChapterProgressWithTrack.kt:21-50`. Kitsu is a no-op here (area:27). | `sync_progress` (area:27). | [area:27](27-tracking-core/00-overview.md) |
| Simkl custom scheme | `aniyomi://simkl-auth`. Kept. Registered as `x-scheme-handler/aniyomi` (ADR-07, area:32 O-6). | Same scheme string. | [area:32](32-tracker-mangaupdates-simkl/00-overview.md) |
| Tracker credentials | Username, password and token. Stored in the Secret Service through `kuro-platform::SecretStore` (ADR-07, `T-02a-018`, "TrackPreferences and tracker secrets", P1). | `SecretPref { store, kind, tracker, cache }` (area:02). | [area:02](02-preferences-system/00-overview.md) |
| MangaUpdates "not on list" | A 404 means not on the list. Other errors are errors (R-20, area:32 O-1). | Same rule. | [area:32](32-tracker-mangaupdates-simkl/00-overview.md) |

## 9. Player and video (anime)

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Player | Anime playback screen under `app/src/main/java/eu/kanade/tachiyomi/ui/player/`, using mpv through `AniyomiMPVView.kt`. | libmpv embedded in the GTK window with the GL render context (ADR-02). | [area:45](45-player-core/00-overview.md) |
| mpv | Video engine. | `libmpv` C API behind a Rust wrapper (ADR-02). Pin the libmpv version (R-27). | [area:45](45-player-core/00-overview.md) |
| mpv Lua scripting | Custom button code runs inside mpv as Lua. | Same, with a restore confirmation (R-06, `T-06a-061`, "Custom button interactors", P1). | [area:48](48-settings-player-and-scripts/00-overview.md) |
| Custom button | User-defined player button with Lua code, stored in a backup list. | Per-button `pcall` isolation (area:48 D-04). | [area:48](48-settings-player-and-scripts/00-overview.md) |
| Primary button | The button bound to the main tap action (`PlayerViewModel.kt:315-345` and `:884-945`). | Same semantics in the player module (area:45). | [area:45](45-player-core/00-overview.md) |
| Intro skip and AniSkip | Skip-intro using the AniSkip service, configured by `T-48a-016` ("Intro skip group and AniSkip settings", P1). | Settings and calls specified in area:45 and area:48. | [area:48](48-settings-player-and-scripts/00-overview.md) |
| Skip intro and airing bits | Viewer-flag bits (section 3.1). | Same bit layout (area:07). | [area:07](07-domain-entries-items/00-overview.md) |
| Torrent playback | Anime from torrents through a TorrServer binary (Go) or `librqbit` in-process (ADR-03). P2, skipped for v1 unless U-5 is yes. Tasks `T-45a-072` to `T-45a-079` are P2. | `librqbit` by default, TorrServer fallback at `<data>/bin/torrserver` (ADR-03, area:13 section 5). | [area:45](45-player-core/00-overview.md) |
| TorrServer | Torrent streaming server. Its JSON DTOs are in `core/common/src/main/java/aniyomi/core/common/torrent/model/` (`Torrent.kt`, `TorrentRequest.kt`). | Optional fallback binary (ADR-03). | [area:13](13-core-util-storage-archive-torrent/00-overview.md) |
| Volume boost | mpv `volume` above 100, capped by `pref_audio_volume_boost_cap`. `volume-max` = cap + 100 (area:45, p1-behavior section 8.5). | Same. | [area:45](45-player-core/00-overview.md) |
| Audio mapping "Auto" | Aniyomi's "Auto" mapping reverses channels (R-30). | Fixed mapping, noted in release notes (area:48 D-02). | [area:48](48-settings-player-and-scripts/00-overview.md) |
| External player | Command template with `$URL` (area:02 Q-F). | Same template. | [area:02](02-preferences-system/00-overview.md) |

## 10. Reader (manga)

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Reader | Manga reading screen under `app/src/main/java/eu/kanade/tachiyomi/ui/reader/`. | Reader module (area:43). | [area:43](43-reader-core/00-overview.md) |
| Viewer | Layout engine for a reading mode. The pager modes are LTR, RTL and vertical; the webtoon modes are `WEBTOON` and `CONTINUOUS_VERTICAL` (`ViewerType` in `ReadingMode.kt`). | `ViewerEngine` (area:44). | [area:44](44-reader-viewers/00-overview.md) |
| Reading mode | One of the six `ReadingMode` values in section 3. | `ReadingMode` enum (area:43). | [area:43](43-reader-core/00-overview.md) |
| Page | One image of a chapter: `index`, `url`, `imageUrl`, `uri` (section 4). | `struct Page` (area:15). | [area:15](15-source-api-manga/00-overview.md) |
| Page status | Load state of one page image, exposed through watch handles (`PageStatus`, area:44). | `PageStatus` with watch handles (area:44). | [area:44](44-reader-viewers/00-overview.md) |
| Tap zones | Screen regions that map taps to reader actions. Settings UI `TapZones` (area:43). | Tap-zone settings in area:43 and area:44. | [area:43](43-reader-core/00-overview.md) |
| Cropped borders | Crop of white borders before display. Decoder request field `crop_borders` (area:34). | `DecodeRequest { target_width, crop_borders }` (area:34). | [area:34](34-images-covers-cache/00-overview.md) |
| Display cutout | Android notch padding. Hidden: there is no Linux equivalent (area:43 D-6). | Dropped. | [area:43](43-reader-core/00-overview.md) |
| Decoded budget | Image memory limit of 64 MiB (R-15, area:34 Q-8). | Budget in `kuro-images` (area:34). | [area:34](34-images-covers-cache/00-overview.md) |

## 11. Backup, migration and app upgrade

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Backup (`.tachibk`) | Protobuf file written by `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupCreator.kt` and read by `BackupDecoder.kt`. Models are in `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/`. | `kuro-backup` with `prost`, same wire format (areas 21 and 22). | [area:21](21-backup-format-and-create/00-overview.md) |
| `BackupOptions` | Flags that choose what a backup contains. | Same flags (area:21). | [area:21](21-backup-format-and-create/00-overview.md) |
| `LegacyBackup` | Older backup model read by the restorer (area:22). The legacy migration chain was tested only on a reconstructed v113 baseline (R-28). | Read path for legacy files, pending a real sample (area:04 Q1, R-28). | [area:22](22-backup-restore/00-overview.md) |
| Restorer | Per-kind restore code, `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/AnimeRestorer.kt` (manga twin in the same folder). Merges incoming items with database rows, for example `fillermark = episode.fillermark || dbEpisode.fillermark` (`AnimeRestorer.kt:194`). | `kuro-backup::restore` (area:22). | [area:22](22-backup-restore/00-overview.md) |
| Auto backup | Scheduled backup (`autobackup`). | Scheduled task (area:21). | [area:21](21-backup-format-and-create/00-overview.md) |
| `kuro_` prefix | Prefix for Kurobayomi-only settings in backups. Aniyomi ignores them harmlessly (ADR-06, U-9). | Settings keys with the `kuro_` prefix (area:02). | [area:02](02-preferences-system/00-overview.md) |
| `__APP_STATE_*` keys | App-internal state keys. Excluded from backups (area:17 Q3). | Excluded. | [area:17](17-source-app-layer/00-overview.md) |
| Database migrations | Kotlin migrations are `data/src/main/sqldelight/migrations/*.sqm` (for example `14.sqm`), selected by `user_version`. | `kuro-core/src/migrate/` with `Migration`, `Strategy`, `select`, `plan`, `run`, `run_blocking` and the static `MIGRATIONS` (area:01 section 2.2, area:11). | [area:11](11-app-migrations/00-overview.md) |
| `MigrationFuture` | `Pin<Box<dyn Future<Output = anyhow::Result<bool>> + Send + 'a>>` (area:11). | Same type alias. | [area:11](11-app-migrations/00-overview.md) |
| App migrations | Steps run at startup when the app version changes. Tracked by `last_version_code`. | `last_version_code` preference and the migration chain (area:11). | [area:11](11-app-migrations/00-overview.md) |
| Importing an Aniyomi library | Restore from a backup file (area:22). Direct import of the Android database: `T-03a-023` ("Aniyomi tachiyomi.db importer", P2) and `T-04a-025` ("Legacy chain: import a copied Aniyomi animedb (versions 114 to 138)", P2). | Backup file first. Direct import is P2 (RISKS section 3, U-1). | [area:03](03-db-manga-schema-migrations/00-overview.md), [area:04](04-db-anime-schema-migrations/00-overview.md) |

## 12. Settings, navigation and shell

| Term | OBSERVED meaning | Kurobayomi counterpart (proposal) | Owner |
|---|---|---|---|
| Settings screen | Compose screens under `app/src/main/java/eu/kanade/presentation/more/settings/screen/`. | GTK pages (area:47). | [area:47](47-settings-screens-general/00-overview.md) |
| Settings search | Index of settings for the search box. | Search index built from the settings tree (area:47b). | [area:47b](47b-settings-widgets-search-more/00-overview.md) |
| Preference widgets | Rows: switch, list, slider, text, multi-select, tri-state. | GTK row widgets (area:47b). | [area:47b](47b-settings-widgets-search-more/00-overview.md) |
| `PreferenceScreen` (source) | Source-defined settings tree, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt`. | Source settings built from the same tree (area:16). | [area:16](16-source-api-anime/00-overview.md) |
| `SourcePreferences` | Source keys, including `DataSaver`. `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt`. | `SourcePreferences` in `kuro-prefs` (area:17). | [area:17](17-source-app-layer/00-overview.md) |
| `LibraryPreferences`, `UiPreferences`, `TrackPreferences` | Preference holders for library display, app look and behaviour, and tracking. | Same keys in `kuro-prefs` (area:02). | [area:02](02-preferences-system/00-overview.md) |
| `Navigator` | Navigation stack with push, pop and replace (Voyager in Kotlin). | Proposal: `Navigator` with `push`, `pop`, `replace`, `pop_until_root`, `SelectTab` and `SearchLibrary(query)` (area:36). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| Tabs in More | `NavStyle` decides which tabs move into the More tab (section 3). | `NavStyle` (area:36). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| Incognito mode | Stops history writes and shows a banner. Notification channel id `incognito_mode_channel:-701` (area:01 D8). | Window banner and one notification (area:01 Q6, `T-53a-022`). | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| App lock | Lock after inactivity, controlled by `lock_app_after`. Passphrase, opt-in, default off (ADR-09). | `AppLock` trait in `kuro-platform` (`T-36a-032` to `T-36a-034`). | [area:36](36-ui-shell-navigation-security-webview/00-overview.md) |
| Crash report | Written on panic. Opened by the same binary with `--crash-report <file>` (area:01 D2). | `kuro-core/src/crash.rs` and `kuro-core/src/crash_dump.rs`. | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| Notice | Kurobayomi term for a desktop notification handled by the notice dispatcher. GAction `app.notice` (`T-53a-017`, area:53). | `gio::Notification` plus the `app.notice` action. | [area:53](53-platform-manifest-intents-workers/00-overview.md) |
| Update check | Release check against `update.repo` (empty disables checks). `update.auto_check` defaults to `true`. Throttled to 3 days (ADR-10, area:06 Q4). | `UPDATE_REPOSITORY` build constant (`T-06a-070`, `T-06a-073`). | [area:35](35-data-misc-updater-notification-export/00-overview.md) |
| Stable channel | The only update channel. The preview channel is dropped (`T-35a-061`, P3). | Same. | [area:35](35-data-misc-updater-notification-export/00-overview.md) |
| `app_date_format`, `relative_time_v2` | Date and relative time preferences. | `format::relative_time_span` (areas 01 and 42 D-3). | [area:42](42-ui-updates-history-stats-storage-upcoming/00-overview.md) |
| `app_language` | UI language preference. | Strings from `kuro-i18n` (area:51). | [area:51](51-i18n/00-overview.md) |
| `MR` and `AYMR` | String resource objects from `tachiyomi.i18n.MR` and `tachiyomi.i18n.aniyomi.AYMR`. Keys are dotted names such as `pref_bottom_nav_no_manga` (`NavStyle.kt`). | Keys kept as message ids in `kuro-i18n` (area:51). | [area:51](51-i18n/00-overview.md) |

## 13. Rust crate names

The canonical names below are the proposal. Plan documents use some other names (section 14). The table lists each crate with how often the area documents mention it, to show which name is in use.

| Canonical name (proposal) | Role | Mentions in area documents | Owner |
|---|---|---|---|
| `kurobayomi` | GTK 4 application, `src/` | application crate, named in area:01 | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `kuro-core` | Traits shared across areas, startup, database, migrations, deep links, settings | 614 | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `kuro-platform` | The only crate with `cfg(target_os)`, D-Bus, `Paths`, `SecretStore`, `Notifier` | 343 | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `kuro-testkit` | Fixtures, in-memory settings, `FakeTracker` | 51 | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `kuro-domain` | Pure domain types with no I/O | 290 | [area:07](07-domain-entries-items/00-overview.md) |
| `kuro-db` | `rusqlite` statements and codecs | 186 | [area:04](04-db-anime-schema-migrations/00-overview.md) |
| `kuro-prefs` | `Pref<T>`, `PrefBinding<T>` and preference enums | 91 | [area:02](02-preferences-system/00-overview.md) |
| `kuro-net` | `reqwest` client, rate limiting, interceptors | 64 | [area:12](12-network-stack/00-overview.md) |
| `kuro-source` | Source traits and source models | 78 | [area:17](17-source-app-layer/00-overview.md) |
| `kuro-sources` | Alternative spelling of `kuro-source` | 58 | Conflict, section 14 |
| `kuro-source-http` | HTTP source base | 37 | [area:15](15-source-api-manga/00-overview.md) |
| `kuro-source-local` | Local source | 66 | [area:14](14-source-local-and-metadata/00-overview.md) |
| `kuro-source-wasm` | WASM extension host | 22 | [area:20](20-extension-strategy-redesign/00-overview.md) |
| `kuro-extensions` | Extension install, repos and trust | 53 | [area:18](18-extensions-manga/00-overview.md) |
| `kuro-track` | Trackers | 260 | [area:27](27-tracking-core/00-overview.md) |
| `kuro-downloads` | Download actor and queue | 33 | [area:23](23-downloads-manga/00-overview.md) |
| `kuro-images` | Image decode, covers and cache | 33 | [area:34](34-images-covers-cache/00-overview.md) |
| `kuro-chapters` | Chapter and episode sync | 39 | [area:10](10-app-domain-layer/00-overview.md) |
| `kuro-data` | Repositories | 21 | [area:05](05-data-repos-entries-items/00-overview.md) |
| `kuro-ui` | Shared GTK helpers | 40 | [area:01](01-foundation-app-bootstrap/00-overview.md) |
| `kuro-i18n` | Message ids | 23 | [area:51](51-i18n/00-overview.md) |
| `kb-source` | Used as a crate name in some areas, conflicts with `kuro-source` | 61 | Conflict, section 14 |
| `kb-ext` | Used as a crate name in some areas, conflicts with `kuro-extensions` | 30 | Conflict, section 14 |
| `kb-*` icon and CSS names | Icon names such as `kb-library-symbolic`, and CSS classes such as `kb-dark` and `kb-header` | many | [area:49](49-theme-and-design-system/00-overview.md) |

## 14. Naming conflicts across the plan

These conflicts must be resolved before the affected tasks are scheduled.

- **Crate prefixes.** `kuro-*` is used far more than `kb-*` for crates. Proposal: every crate is `kuro-*`, and `kb-` is only for icon and CSS names. Decide before `T-01a-001`.
- **`kuro-source` vs `kuro-sources` vs `kb-source`.** Proposal: `kuro-source` for the trait and model crate, and `kuro-source-http`, `kuro-source-local`, `kuro-source-wasm` for implementations.
- **`TriState` vs `TriStateValue`.** Two different Kotlin types (section 2). Keep both names separate.
- **`EntryKind` variant order.** Declared as `Manga, Anime` in `25-downloads-shared/p1-rust-design.md`, and as `Anime, Manga` in `27-tracking-core/p2-rust-design.md` and in the `EntryRef` snippet (also `27-tracking-core`). Proposal: `Manga, Anime`, matching the Kotlin packages.
- **`FetchType` default.** Area 16 uses `Episodes`. The Kotlin decode fallback is `Episodes` (`DatabaseAdapter.kt:43-46`). Consistent.
- **`UpdateStrategy` default.** Area 07 uses `#[default] AlwaysUpdate`, which matches the column default. Consistent.
- **Repository functions for history and anime** are specified by areas 04, 05 and 06 (RISKS R-03). Area 04 owns the SQL and codecs.
- **Repository rows.** SQLite `extension_repos` (area:20 Q5) wins over `repos.toml` (areas 03, 18 and 20).
- **Module layout.** `crates/kuro-domain` and `kuro-data` (area:01) conflict with a flat `src/updates/` (area:42). Area 01 wins (area:42 D-2).
- **Task priorities in RISKS section 3 that do not match `TASKS.csv`.** `T-06a-070` is P2 in `TASKS.csv` (RISKS says P1). `T-06a-073` is P3 (drop of in-app APK download). `T-02a-036` and `T-02a-038` are P1 in `TASKS.csv`, as RISKS states before its proposal to make them P0.
- **Task title mismatches.** RISKS and some area docs cite task IDs for behaviour that the task titles do not describe. `T-47a-002` is "Settings item model and row rendering rules" in `TASKS.csv`, but is cited for modal dialogs. `T-47a-035` is "E-ink flash group" (P2) in `TASKS.csv`, but is cited for the `gtk::Scale` slider. `T-03a-022` is "Embedded legacy migrations 1..32 and replay harness" (P2), but RISKS cites it as a library import task. `T-04a-026` is "Fresh-versus-upgraded equivalence and drift tests" (P2), not a raw import task. `T-06a-061` is "Custom button interactors: create, delete, reorder, favourite, update" (P1), but RISKS cites it for the restore confirmation.

## 15. Priority, scope and reference terms

| Term | Meaning in this plan | Owner |
|---|---|---|
| P0 | Needed for daily reading and watching from one working source: library, details, reader, player, progress, local database, essential settings, one source mechanism. | [RISKS section 3](RISKS-AND-DECISIONS.md) |
| P1 | Parity features most Aniyomi users rely on: downloads, tracking, backup and restore, library updates and notifications, categories, global search, source and extension management, history and updates, migration. | [RISKS section 3](RISKS-AND-DECISIONS.md) |
| P2 | Long-tail features and polish. | [RISKS section 3](RISKS-AND-DECISIONS.md) |
| P3 | Android-only or obsolete. Each P3 task states the replacement or the drop decision. | [RISKS section 4](RISKS-AND-DECISIONS.md) |
| Effort S, M, L, XL | S up to 1 day, M up to 3 days, L up to 1.5 weeks, XL more than 1.5 weeks. | [TASKS.csv](TASKS.csv) |
| `area:<id>` | Reference to an area directory. | This file |
| Ownership conflict | Two areas specify the same item. The owner named in RISKS section 7 wins. | [RISKS section 7](RISKS-AND-DECISIONS.md) |
| ADR-nn | Recorded decision in RISKS section 1. | [RISKS section 1](RISKS-AND-DECISIONS.md) |
| Q-nn (per area), U-nn (user) | Q-numbers are open questions inside one area. U-numbers are the blocking questions for the user (RISKS section 2). | [RISKS section 2](RISKS-AND-DECISIONS.md) |
| R-nn | Risk entries in RISKS section 5. | [RISKS section 5](RISKS-AND-DECISIONS.md) |

## 16. Task IDs cited in this glossary

Priority and title are taken from `plans/TASKS.csv`.

| Task | Priority, effort | Title in TASKS.csv |
|---|---|---|
| `T-01a-001` | P0, M | Cargo workspace with kuro-core, kuro-platform and kuro-testkit crates |
| `T-01a-003` | P0, L | AppContext: the dependency-injection replacement |
| `T-01a-023` | P1, M | Notification category registry and Notifier mapping |
| `T-01a-032` | P1, M | App lock state machine |
| `T-02a-018` | P1, M | TrackPreferences and tracker secrets |
| `T-02a-036` | P1, S | Value range and choice metadata for settings widgets and import clamping |
| `T-02a-038` | P1, S | Shutdown flush and save-failure notification |
| `T-03a-022` | P2, M | Embedded legacy migrations 1..32 and replay harness |
| `T-03a-023` | P2, L | Aniyomi tachiyomi.db importer |
| `T-04a-025` | P2, L | Legacy chain: import a copied Aniyomi animedb (versions 114 to 138) |
| `T-04a-026` | P2, M | Fresh-versus-upgraded equivalence and drift tests |
| `T-04a-031` | P1, S | Database file safety: integrity check, backup before upgrade, corruption quarantine |
| `T-05a-025` | P1, M | Category rules: create order, delete renumbering, reorder, exclude cleanup |
| `T-06a-061` | P1, M | Custom button interactors: create, delete, reorder, favourite, update |
| `T-06a-070` | P2, M | GitHub latest-release client |
| `T-06a-073` | P3, S | Drop in-app APK download/install and the preview update channel |
| `T-08a-019` | P0, M | History recording: upsert semantics, session timer, throttle, incognito |
| `T-20a-001` | P0, L | WASM component loader and package discovery |
| `T-20a-060` | P1, M | Trust model for repos and per-version keys |
| `T-23a-019` | P0, M | Downloader notifications: progress, paused, complete, warning, error |
| `T-28a-040` | P0, M | Redirect scheme handler and token landing |
| `T-34a-028` | P0, M | AVIF and JPEG XL still-image decoders |
| `T-34a-029` | P2, M | HEIF/HEIC decoder |
| `T-35a-055` | P2, M | Install kind detection (package-managed vs AppImage vs other) |
| `T-35a-061` | P3, S | Drop: preview build channel (commit-count tags) |
| `T-36a-022` | P1, M | Startup update checks (app release and extensions) |
| `T-36a-023` | P1, M | New update prompt page |
| `T-36a-032` | P1, L | App lock state machine |
| `T-36a-033` | P1, L | App lock authenticator (Linux replacement for BiometricPrompt) |
| `T-36a-034` | P1, M | Unlock window |
| `T-39b-052` | P2, M | Torrent server start before playback (child process, not an Android service) |
| `T-45a-072` | P2, M | Torrent server lifecycle |
| `T-45a-073` | P2, M | Torrent server API client |
| `T-45a-079` | P2, S | Torrent server status notification |
| `T-47a-002` | P0, M | Settings item model and row rendering rules |
| `T-47a-035` | P2, M | E-ink flash group |
| `T-48a-016` | P1, M | Intro skip group and AniSkip settings |
| `T-49ba-001` | P0, M | GTK-theme-following base stylesheet and palette provider |
| `T-53a-017` | P1, M | Notice dispatcher replacing NotificationReceiver (app.notice GAction) |
| `T-53a-022` | P1, S | Incognito notice and disable action |

Full list: [TASKS.csv](TASKS.csv). Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint <file>`.

## 17. Plan tools

| Tool | Purpose (from its header) |
|---|---|
| `plans/_tools/inventory.py` | Aniyomi source inventory and plan-coverage checker. `list` prints what must be covered. `check` prints what the plans do not yet mention. |
| `plans/_tools/areas.py` | Partition of the Aniyomi repository into plan areas. Prints sizes and uncovered files. `json` prints the workflow arguments. |
| `plans/_tools/tasks.py` | `lint` checks task format. `export` writes `TASKS.csv`. |
| `plans/_tools/verify.py` | Whole-plan verification: coverage inventory, task graph, required files, placeholders and links. |
| `plans/catalogs/` | Chunked catalogs of Android APIs, network endpoints, preferences and SQL statements. |
| `plans/addenda/` | Cross-platform requirements (`requirements-cross-platform.md`) and their tasks. |
