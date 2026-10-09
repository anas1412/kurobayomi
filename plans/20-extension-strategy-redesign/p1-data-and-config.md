# 20-extension-strategy-redesign: data models, config, endpoints

All field names and defaults are copied from the Kotlin sources (repo-relative to `/home/blackbox/aniyomi-src`). **Proposal:** Rust equivalents are given in p1-rust-design.md.

## D1. Source model (manga)

From `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/`:

### SManga (interface; impl `SMangaImpl`)
| Field | Type | Default (SMangaImpl) | Notes |
|---|---|---|---|
| url | String | lateinit | path relative to baseUrl |
| title | String | lateinit | |
| artist | String? | null | |
| author | String? | null | |
| description | String? | null | |
| genre | String? | null | comma-joined `", "` |
| status | Int | 0 | constants below |
| thumbnail_url | String? | null | |
| update_strategy | UpdateStrategy | ALWAYS_UPDATE | |
| initialized | Boolean | false | set true after details parse (`HttpSource.fetchMangaDetails` applies `initialized = true`) |

Status constants (companion): `UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6`.

### SChapter (impl `SChapterImpl`)
| Field | Type | Default | Notes |
|---|---|---|---|
| url | String | lateinit | |
| name | String | lateinit | |
| date_upload | Long | 0 | epoch milliseconds |
| chapter_number | Float | (unset in impl; required) | |
| scanlator | String? | null | |

### UpdateStrategy (enum)
`ALWAYS_UPDATE`, `ONLY_FETCH_ONCE`. Documented in source as "since extensions-lib 1.4".

### MangasPage
`data class MangasPage(val mangas: List<SManga>, val hasNextPage: Boolean)` (MangasPage.kt).

### Page (open class, Serializable)
Constructor `Page(index: Int, url: String = "", imageUrl: String? = null, uri: Uri? = null)`. `number = index + 1`. `status` enum `State { QUEUE, LOAD_PAGE, DOWNLOAD_IMAGE, READY, ERROR }`, default QUEUE. `progress: Int` default 0; `-1` when content length unknown.

### Filter (sealed class, Filter.kt)
Subclasses: `Header(name)`, `Separator(name = "")`, `Select<V>(name, values: Array<V>, state: Int = 0)`, `Text(name, state: String = "")`, `CheckBox(name, state: Boolean = false)`, `TriState(name, state: Int = STATE_IGNORE)`, `Group<V>(name, state: List<V>)`, `Sort(name, values: Array<String>, state: Selection? = null)` with `Selection(index: Int, ascending: Boolean)`.
TriState: `STATE_IGNORE = 0`, `STATE_INCLUDE = 1`, `STATE_EXCLUDE = 2`.

## D2. Source model (anime)

From `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/`:

### SAnime (impl `SAnimeImpl`)
Fields: `url`, `title`, `thumbnail_url: String?`, `background_url: String?`, `artist`, `author`, `status: Int`, `description`, `genre`, `update_strategy: AnimeUpdateStrategy`, `fetch_type: FetchType`, `season_number: Double`, `initialized: Boolean`, `memo: JsonObject`.
Status constants: `UNKNOWN=0, ONGOING=1, COMPLETED=2, LICENSED=3, PUBLISHING_FINISHED=4, CANCELLED=5, ON_HIATUS=6, UPCOMING=7`.

### SEpisode (impl `SEpisodeImpl`)
Fields: `url`, `name`, `episode_number: Float`, `fillermark: Boolean`, `scanlator: String?`, `date_upload: Long`, `summary: String?`, `preview_url: String?`, `memo: JsonObject`.

### AnimeUpdateStrategy, FetchType
`AnimeUpdateStrategy { ALWAYS_UPDATE, ONLY_FETCH_ONCE }`. `FetchType { Seasons, Episodes }`.

### AnimeRelation
`class AnimeRelation(val name: String, val animes: List<SAnime>)`.

### AnimesPage
`class AnimesPage(val animes: List<SAnime>, val hasNextPage: Boolean)`. Deprecated `component1/2`/`copy` shims kept for compatibility.

### Hoster (open class)
| Field | Type | Default |
|---|---|---|
| hosterUrl | String | "" |
| hosterName | String | "" |
| videoList | List<Video>? | null |
| internalData | String | "" |
| lazy | Boolean | false |
| memo | JsonObject | JsonObject.EMPTY |
Transient `status: State` default `IDLE`. `State { IDLE, LOADING, READY, ERROR }`. Companion: `NO_HOSTER_LIST = "no_hoster_list"`; `List<Video>.toHosterList()` wraps videos in `Hoster(hosterUrl = "", hosterName = NO_HOSTER_LIST, videoList = this, memo = EMPTY)`.

`SerializableHoster(hosterUrl, hosterName, videoList: String?, internalData: String, lazy: Boolean = false, memo: JsonObject)` is the storage form; `videoList` is a JSON string of the list.

### Video (data class with secondary constructors)
Primary constructor fields and defaults:
| Field | Type | Default |
|---|---|---|
| videoUrl | String | "" |
| videoTitle | String | "" |
| resolution | Int? | null |
| bitrate | Int? | null |
| headers | Headers? | null |
| preferred | Boolean | false |
| subtitleTracks | List<Track> | emptyList() |
| audioTracks | List<Track> | emptyList() |
| timestamps | List<TimeStamp> | emptyList() |
| mpvArgs | List<Pair<String,String>> | emptyList() |
| ffmpegStreamArgs | List<Pair<String,String>> | emptyList() |
| ffmpegVideoArgs | List<Pair<String,String>> | emptyList() |
| internalData | String | "" |
| initialized | Boolean | false |
| memo | JsonObject | JsonObject.EMPTY |
Deprecated `quality` alias for `videoTitle`; `url` is the page URL, stored in a private `videoPageUrl`.

### Track, TimeStamp, ChapterType
- `Track(url: String, lang: String)`
- `ChapterType { Opening, Ending, Recap, MixedOp, Other }` (`@Serializable` enum)
- `TimeStamp(start: Double, end: Double, name: String, type: ChapterType = ChapterType.Other)`

### ThumbnailInfo, TileInfo
- `ThumbnailInfo(tileInfo: List<TileInfo>, imageTileUrls: List<String>)`
- `TileInfo(imageIndex: Int, timeMs: Long, x: Int, y: Int, width: Int, height: Int)`

### HttpServer
`open class HttpServer : NanoHTTPD(0)`, `url = "http://localhost:$listeningPort"`, `PLACEHOLDER_URL = "http://localhost:1"`. **Proposal:** dropped (see p1-topic-option-matrix.md).

### TorrentInfo / TorrentFile (source-api torrentutils)
- `TorrentInfo(title: String, files: List<TorrentFile>, hash: String, size: Long, trackers: List<String> = emptyList())`
- `TorrentFile` fields are passed by the constructor `TorrentFile(path, id, length, hash, trackers)` (TorrentUtils.kt `torrentToTorrentInfo`).
- Errors: `DeadTorrentException` (timeout on magnet add), `DisabledTorrServerException` (any other error).

## D3. Extension model (app side)

From `app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt`. Anime is structurally identical (its file is not in scope, its shape is inferred from `AnimeLoadResult` and the managers).

### MangaExtension (sealed)
Common abstract fields: `name: String`, `pkgName: String`, `versionName: String`, `versionCode: Long`, `libVersion: Double`, `lang: String?`, `isNsfw: Boolean`.

- `Installed(name, pkgName, versionName, versionCode, libVersion, lang: String, isNsfw, pkgFactory: String?, sources: List<MangaSource>, icon: Drawable?, hasUpdate: Boolean = false, isObsolete: Boolean = false, isShared: Boolean, repoUrl: String? = null)`
- `Available(name, pkgName, versionName, versionCode, libVersion, lang: String, isNsfw, sources: List<MangaSource>, apkName: String, iconUrl: String, repoUrl: String)`; nested `MangaSource(id: Long, lang: String, name: String, baseUrl: String)`.
- `Untrusted(name, pkgName, versionName, versionCode, libVersion, signatureHash: String, lang: String? = null, isNsfw: Boolean = false)`

### Load results
- `MangaLoadResult`: `Success(extension: Installed)`, `Untrusted(extension: Untrusted)`, `Error` (data object). Anime: `AnimeLoadResult` identical shape (`AnimeLoadResult.kt` in scope).

### InstallStep (enum)
`Idle, Pending, Downloading, Installing, Installed, Error`. `isCompleted() = this in {Installed, Error, Idle}`.

## D4. Repository index format

Endpoint: `GET {repoBaseUrl}/index.min.json`. Response: JSON array.

| Field | JSON type | Used as |
|---|---|---|
| name | string | display name, prefix `"Tachiyomi: "` or `"Aniyomi: "` stripped |
| pkg | string | package name (unique key) |
| apk | string | APK filename under `/apk/` |
| lang | string | extension language |
| code | integer (Long) | version code |
| version | string | version name; lib version = `version` minus last `.N` |
| nsfw | integer 0/1 | NSFW flag, `== 1` |
| sources | array or null | `[{id: string(Long), lang, name, baseUrl}]` |

Icon: `GET {repoBaseUrl}/icon/{pkg}.png`. APK: `GET {repoBaseUrl}/apk/{apk}`.

Live example (observed 2026-10-08 on `aniyomiorg/aniyomi-extensions` branch `repo`): 3 entries, all `"lang":"all"`, all `"version":"14.x"`, `sources[].id` strings such as `"4222017068256633289"`.

Observed index sizes for the manga ecosystem (keiyoushi, GitHub directory counts, default branch `main`): `src/all` 134 directories, `src/en` 407 directories.

## D5. Preference keys used by the extension system (Aniyomi)

| Key | Type | Default | Where | Meaning |
|---|---|---|---|---|
| `last_ext_check` | Long (`preferenceStore.getLong`) | 0 | MangaExtensionApi.kt, AnimeExtensionApi.kt | epoch ms of last repo check; throttle is 1 day (`1.days`) |
| `trusted_extensions` | Set<String> | empty | SourcePreferences via `trustedExtensions()` | entries `"<pkg>:<versionCode>:<sha256>"` |
| `enabled_languages` | Set<String> | `defaultValue()` | SourcePreferences `enabledLanguages()` | languages shown in browse |
| `manga_extension_updates_count` | Int | 0 | SourcePreferences `mangaExtensionUpdatesCount()` | pending manga updates badge |
| `anime_extension_updates_count` | Int | 0 | SourcePreferences (anime counterpart) | pending anime updates badge |
| `show_nsfw_source` | Boolean | (SourcePreferences) | MangaExtensionLoader `loadNsfwSource` | allow NSFW sources |
| `extension_installer` | enum `BasePreferences.ExtensionInstaller` (LEGACY, PRIVATE, Shizuku, PackageInstaller) | (BasePreferences) | installers | how APKs are installed (Android only) |
| `hide_notification_content` | Boolean | (SecurityPreferences) | ExtensionUpdateNotifier | hides names in notification |
| `source_<id>` | per-source key-value | none | source preferences | source settings namespace |

Note: the exact defaults of `show_nsfw_source`, `enabled_languages`, the update count preferences and `extension_installer` live in the preference files outside this scope (see area:02-preferences-system). Values listed as "(…)" are not read here.

## D6. Files on disk (Aniyomi)

| Path | Created by | Contents |
|---|---|---|
| `<filesDir>/exts/<pkg>.ext` | `installPrivateExtensionFile` | private APK, read-only |
| `<cacheDir>/temp_<downloadId>` | installer PRIVATE path | temp copy, deleted after install |
| `<externalFilesDir>/Downloads/<last segment of apk url>` | DownloadManager | downloaded APK (Android) |
| Android package manager install | system | shared extension APK |

## D7. Proposed on-disk layout (Kurobayomi)

```
$XDG_DATA_HOME/kurobayomi/extensions/
  <pkg>/                     one directory per installed extension
    manifest.toml            metadata (see D8)
    component.wasm           WASM component (manga or anime world)
    component.sha256         hex digest recorded at install time
    settings.json            optional, schema for settings screen
  .tmp/
    <pkg>.<version>.part     in-progress download
$XDG_DATA_HOME/kurobayomi/scrapers/<id>.toml     declarative sources
$XDG_CONFIG_HOME/kurobayomi/repos.toml           repository list (D10)
```

## D8. manifest.toml (Proposal)

```toml
pkg = "app.kurobayomi.ext.en.example"   # reverse-DNS package id
name = "Example"                         # display name (no prefix)
version = "1.4.2"                        # version name
version_code = 42                        # i64, monotonic
api = "1.0"                              # WIT world version, see topic doc
kind = "manga"                           # "manga" | "anime" | "both"
lang = "en"                              # "all" for multi-language packages
nsfw = false
sources = [
  { id = "4222017068256633289", lang = "en", name = "Example", base_url = "https://example.org" },
]
key_id = "<sha256 hex of signing key>"   # optional for unsigned local dev builds
```

`sources[].id` is a string in TOML and JSON, parsed to `i64` (see B9 for the derivation rule; the id is recomputed at load and must equal the stored id, else `Error`).

## D9. Database tables touched (Proposal)

Owned by area:05-data-repos-entries-items and area:06-data-repos-history-track-source-misc; this area adds or reads:

| Table | Columns | Use |
|---|---|---|
| `stub_source` | `id INTEGER PRIMARY KEY`, `lang TEXT NOT NULL`, `name TEXT NOT NULL` | name and lang of sources that are not installed; written by `register_stub_source` (B7) |
| `extension_repo` | `base_url TEXT PRIMARY KEY`, `signing_key TEXT NOT NULL`, `position INTEGER NOT NULL` | repo list (D10) |
| `kb_settings` | `key TEXT PRIMARY KEY`, `value TEXT NOT NULL` | `last_ext_check`, `trusted_ext:<pkg>`, update counts, `enabled_languages` |
| `source_settings` | `source_id INTEGER`, `key TEXT`, `value TEXT`, primary key `(source_id, key)` | per-source settings (T-20a-005) |

## D10. Repository list

The Aniyomi repo storage (`GetMangaExtensionRepo` / `GetAnimeExtensionRepo` and the `ExtensionRepo` model with `baseUrl` and `signingKeyFingerprint`) is outside this scope and was not read; the default repo URL list is therefore not stated here (owned by area:02-preferences-system and the repo domain area). **Proposal:** `repos.toml`:

```toml
[[repo]]
base_url = "https://raw.githubusercontent.com/aniyomiorg/aniyomi-extensions/repo"
signing_key = "<sha256 hex>"
```

The signing key is required. An empty key disables trust for that repo (its packages show as untrusted).

## D11. Notification strings

`R.plurals.update_check_notification_ext_updates` with quantity `names.size` and format arg `names.size`. Body: `names.joinToString(", ")`, omitted when `hide_notification_content` is true. Channel `Notifications.CHANNEL_EXTENSIONS_UPDATE`, id `Notifications.ID_UPDATES_TO_EXTS`. Click action: open the extensions screen for manga or anime (`openExtensionsPendingActivity` / `openAnimeExtensionsPendingActivity`).

## D12. Anime player options passed through Video

`mpvArgs`, `ffmpegStreamArgs`, `ffmpegVideoArgs`: `List<Pair<String,String>>`. **Proposal:** keep `mpvArgs` (passed as mpv options to libmpv via `mpv_set_option_string`, pairs as `key=value`); drop the ffmpeg lists (libmpv has no per-stream ffmpeg argument interface comparable to Aniyomi's). User loses: source-specified ffmpeg stream tweaks, rare in practice.
