# 20-extension-strategy-redesign: implementation tasks

Scope: replace Aniyomi's APK/classloader extension system with a Rust design. Each task cites repo-relative Aniyomi sources. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint p1-tasks.md`.

### T-20a-001 — WASM component loader and package discovery
- Priority: P0
- Effort: L
- Depends: T-20a-010, T-20a-011, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionLoader.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionLoader.kt
- Acceptance:
  - Scans `<data_dir>/extensions/*/manifest.toml`, rejects a package whose `api` field is not in `1.x` (mirrors Aniyomi libVersion check, MangaExtensionLoader.kt:`LIB_VERSION_MIN = 1.4`, `LIB_VERSION_MAX = 1.5`).
  - Loads each `component.wasm` with wasmtime; a failed instantiate yields `LoadResult::Error` without panicking.
  - An extension whose sha256 digest differs from the repo-signed digest yields `LoadResult::Untrusted`.

### T-20a-002 — Extension registry state (installed / available / untrusted)
- Priority: P0
- Effort: M
- Depends: T-20a-001, T-20a-020, T-20a-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/AnimeExtensionManager.kt
- Acceptance:
  - Three maps keyed by pkgName; `installed` entries carry `has_update` and `is_obsolete` flags exactly as `updatedInstalledExtensionsStatuses` computes them.
  - Updates are published through a tokio `watch` channel; the GTK thread reads snapshots, never blocks.
  - `updatePendingUpdatesCount` writes the badge count and dismisses the notification at zero.

### T-20a-003 — Source manager and stub sources for missing extensions
- Priority: P0
- Effort: M
- Depends: T-20a-002, area:17-source-app-layer
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt
- Acceptance:
  - `get(id)` returns the live source; `get_or_stub(id)` returns a `StubSource{id, lang, name}` when the source is not installed.
  - A local source with id `0` is always registered (LocalMangaSource.ID).
  - Stub rows are upserted when an installed source's name or lang changes (`registerStubSource`).

### T-20a-004 — Source helper functions (icon, preference key, language label)
- Priority: P0
- Effort: S
- Depends: T-20a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt
- Acceptance:
  - `preference_key(id) == "source_" + id` (decimal).
  - `name_for_manga_info`: if exactly one enabled language and the source lang is in it, show the bare name; otherwise show `toString()` (`"Name (LANG)"`).
  - Languages `all` and `other` are ignored when counting enabled languages.

### T-20a-005 — Per-source settings namespace and configurable sources
- Priority: P0
- Effort: M
- Depends: T-20a-010, area:02-preferences-system
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/ConfigurableAnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/utils/Preferences.kt
- Acceptance:
  - Each source's preferences live in table `source_settings(source_id, key, value)` (replaces Android SharedPreferences file `source_<id>`).
  - The Rust trait `Configurable::setup_settings(&mut SettingsScreen)` declares the same items the Kotlin `setupPreferenceScreen` declares.
  - Changing a value triggers `on_settings_changed` on the source instance, not a reload.

### T-20a-006 — Filter / search-control model
- Priority: P0
- Effort: M
- Depends: T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/FilterList.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt
- Acceptance:
  - TriState values are `STATE_IGNORE=0`, `STATE_INCLUDE=1`, `STATE_EXCLUDE=2`.
  - Sort selection is `{index: i32, ascending: bool}` or `None`; equality compares name and state (Filter.kt equals()).
  - A Rust enum with variants Header, Separator, Select, Text, CheckBox, TriState, Group, Sort round-trips through serde to the same JSON used by the source-side API.

### T-20a-007 — Source data models (manga and anime)
- Priority: P0
- Effort: M
- Depends: T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SEpisode.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt
- Acceptance:
  - SManga status constants: UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6. SAnime adds UPCOMING=7.
  - `get_genres()` splits on `", "`, trims, drops blanks, dedupes (SManga.kt getGenres).
  - Page states QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE, READY, ERROR; Hoster states IDLE, LOADING, READY, ERROR.

### T-20a-010 — Core Source trait (manga and anime)
- Priority: P0
- Effort: M
- Depends: none
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/CatalogueSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeCatalogueSource.kt
- Acceptance:
  - Rust `trait MangaSource` exposes `id() -> i64`, `name()`, `lang()`, `supports_latest()`, `popular/latest/search`, `details`, `chapters`, `pages`, `filters`; all async, all `Send + Sync`.
  - Trait methods that Kotlin marks deprecated (RxJava `fetch*`) are absent.
  - Anime trait additionally has `hoster_list`, `video_list`, `episode_update`, `season_update`, `related_anime`.

### T-20a-011 — HTTP source base, id generation and URL normalisation
- Priority: P0
- Effort: M
- Depends: T-20a-010, area:12-network-stack
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt
- Acceptance:
  - `source_id(name, lang, version_id) = (u64::from_be_bytes(md5(format!("{}/{}/{}", name.to_lowercase(), lang, version_id))[0..8]) & i64::MAX as u64) as i64`. Golden vectors must be produced by running `generateId` in HttpSource.kt once (not yet done); candidate `("Google Drive","all",1)` -> 4222017068256633289 is from the observed index and must be confirmed.
  - `get_url_without_domain` strips scheme and host, spaces become `%20`, keeps path, `?query`, `#fragment`; invalid URIs return the input unchanged.
  - Default headers contain only `User-Agent` from the network default provider.

### T-20a-012 — Source factory (one package, many sources)
- Priority: P0
- Effort: S
- Depends: T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/SourceFactory.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSourceFactory.kt
- Acceptance:
  - A package manifest may list several source entries; each becomes a separate source with its own id.
  - Factory output is not persisted; the loader calls `create_sources()` once per load.

### T-20a-013 — Unmetered marker and rate limit hook
- Priority: P1
- Effort: S
- Depends: T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/UnmeteredSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/UnmeteredSource.kt
- Acceptance:
  - `Unmetered` is a marker trait; a source implementing it is exempt from the per-host request limiter in area:12-network-stack.
  - Sources without the marker keep the default limiter.

### T-20a-020 — Repository index fetch and parse
- Priority: P1
- Effort: M
- Depends: T-20a-010, T-20a-099
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/api/AnimeExtensionApi.kt
- Acceptance:
  - GET `<repo_base>/index.min.json`; the body is a JSON array of objects with fields name, pkg, apk, lang, code, version, nsfw, sources.
  - `sources[].id` is a JSON string in the real index (`"4222017068256633289"`); parse to i64.
  - Entries whose lib version (`version` minus last component) is outside the supported set are dropped; a repo fetch error yields an empty list and a logged error, not a toast.
  - Icon URL is `<repo>/icon/<pkg>.png`; APK URL is `<repo>/apk/<apk>`.

### T-20a-030 — Install / update pipeline and InstallStep
- Priority: P1
- Effort: L
- Depends: T-20a-020, T-20a-060
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/InstallerManga.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstaller.kt
- Acceptance:
  - States `Idle, Pending, Downloading, Installing, Installed, Error`; `is_completed()` is true for Installed, Error, Idle.
  - Download writes `<data>/extensions/.tmp/<pkg>.part`, verifies sha256 from the repo index (when present) and atomically renames to `<pkg>.wasm` plus `manifest.toml`.
  - Only one install per pkgName is active; starting a second cancels the first (`downloadAndInstall` deletes the old download).
  - Cancelling sets state Idle and removes the temp file.

### T-20a-031 — Resolvable URLs and VideoInfo wrapper
- Priority: P2
- Effort: S
- Depends: T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ResolvableSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ResolvableAnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt
- Acceptance:
  - `UriType` has Manga/Chapter/Unknown (manga) and Anime/Episode/Unknown (anime); the trait's `get_manga(uri)`/`get_chapter(uri)` return `Option`.
  - Pasting a URL into global search resolves through `get_uri_type` before any catalogue search.

### T-20a-040 — Filesystem watch and hot reload
- Priority: P1
- Effort: M
- Depends: T-20a-001, T-20a-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/util/MangaExtensionInstallReceiver.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallReceiver.kt
- Acceptance:
  - An inotify watch on `<data>/extensions` fires add/replace/remove events; a debounce of 300 ms coalesces the burst from one install.
  - Add or replace reloads the single package; remove unloads it and deletes its stub entries from the UI.

### T-20a-050 — Update-available notification
- Priority: P1
- Effort: S
- Depends: T-20a-002, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt
- Acceptance:
  - Title is the plural form of `update_check_notification_ext_updates` with the count; body is `names.join(", ")` unless `hide_notification_content` is set.
  - The notification is dismissed when the pending count reaches zero.

### T-20a-052 — Declarative scraper engine (no code)
- Priority: P1
- Effort: L
- Depends: T-20a-010, T-20a-011
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ParsedHttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsoupExtensions.kt
- Acceptance:
  - A TOML source definition supplies, per operation, a request template and CSS selectors that mirror ParsedHttpSource's selector functions (popular, search, latest, details, chapters, pages).
  - `selectText` and `selectInt` semantics are reproduced: first match, default value when absent, `int` parse error propagates as a source error.
  - `attrOrText("text")` returns text content; any other value reads the attribute.

### T-20a-060 — Trust model for repos and per-version keys
- Priority: P1
- Effort: M
- Depends: T-20a-020
- Sources: app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt
- Acceptance:
  - A package is trusted when its signing key is in a repo's key set, or the key `"<pkg>:<versionCode>:<sha256>"` is in `trusted_extensions`.
  - `trust()` removes older trusted entries for the same pkg before adding the new one.
  - `revoke_all()` clears `trusted_extensions`.

### T-20a-070 — Torrent metadata for anime sources
- Priority: P2
- Effort: M
- Depends: T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/TorrentUtils.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/model/TorrentInfo.kt
- Acceptance:
  - A `magnet:` URL is added to the torrent backend; a socket timeout maps to `DeadTorrentException`, any other failure to `DisabledTorrServerException`.
  - A `.torrent` URL is parsed locally without starting the backend.
  - `hash` and `size` (-1 when unknown) are carried into `TorrentInfo`.

### T-20a-092 — Drop Rx and Android-only actuals
- Priority: P3
- Effort: S
- Depends: none
- Sources: source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/source/PreferenceScreen.kt, source-api/src/androidMain/AndroidManifest.xml
- Acceptance:
  - Rust has no Rx equivalent; async/await replaces `awaitSingle`.
  - The `PreferenceScreen` typealias is replaced by the Rust `SettingsScreen` type.

### T-20a-093 — Drop Android APK installer paths
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/ShizukuInstallerManga.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/installer/PackageInstallerInstallerManga.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallActivity.kt, app/src/main/java/eu/kanade/tachiyomi/extension/anime/util/AnimeExtensionInstallService.kt
- Acceptance:
  - No APK install, uninstall or PackageInstaller session code exists in Kurobayomi.
  - User loses: installing Android APK extensions directly. Replacement is T-20a-030 (WASM components).

### T-20a-094 — Backup source-id resolution
- Priority: P0
- Effort: M
- Depends: T-20a-003, T-20a-011, area:03-db-manga-schema-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - An Aniyomi backup's `source` integer equals the id computed by T-20a-011 for the same name, lang and version, so entries resolve without remapping.
  - A backup source id with no installed match becomes a stub, and the entry keeps its URL and title.

### T-20a-095 — Missing or unloaded source behaviour
- Priority: P0
- Effort: S
- Depends: T-20a-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt, app/src/main/java/eu/kanade/tachiyomi/extension/manga/MangaExtensionManager.kt
- Acceptance:
  - A stub source shows its title and cover from the library but disables reader, search and details with the message key `source_not_installed` (text to be translated in area:51-i18n).
  - Installing the extension later re-activates the same id with no data migration.

### T-20a-096 — Anime hoster and video resolution
- Priority: P0
- Effort: M
- Depends: T-20a-007, T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - `hoster_list(episode)` runs first; a `lazy` hoster is resolved only when selected, through `video_list(hoster)`.
  - `resolve_video` runs before playback; `sort_videos` is applied to the final list.
  - Hoster name `"no_hoster_list"` (Hoster.NO_HOSTER_LIST) means the video list is the flat list.

### T-20a-097 — Anime episode and season update
- Priority: P0
- Effort: M
- Depends: T-20a-007, T-20a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnimeEpisodeUpdate.kt
- Acceptance:
  - `episode_update(anime, episodes, fetch_details, fetch_episodes)` returns the details and episode diff.
  - `FetchType` is `Seasons` or `Episodes`; a season list is fetched only when `fetch_type == Seasons`.

### T-20a-098 — Update strategy in library refresh
- Priority: P1
- Effort: S
- Depends: T-20a-007, area:08-domain-library-category-history-updates
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt
- Acceptance:
  - `ALWAYS_UPDATE` entries are included in library updates; `ONLY_FETCH_ONCE` entries are skipped once they have chapters or episodes.
  - Anime and manga use the same two variants.

### T-20a-099 — Repository list, add, remove and order
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt
- Acceptance:
  - Repos are stored as `(base_url, signing_key)` rows; `getAll()` order is insertion order.
  - A repo index failure does not remove the repo; other repos still load.

### T-20a-100 — Anime related titles and timestamps
- Priority: P1
- Effort: S
- Depends: T-20a-096
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeRelation.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - `related_anime(anime)` returns `[{name, animes}]`; empty groups are hidden.
  - `TimeStamp{start, end, name, type}` where `type` is one of Opening, Ending, Recap, MixedOp, Other, used for skip buttons.
