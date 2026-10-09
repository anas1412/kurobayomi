# 52-util-app — data models, enums, flag layouts, preferences, files, endpoints

Every value below was read from the source tree at `/home/blackbox/aniyomi-src` (repo-relative paths). Where a value is stored in the database, the column is named in the upstream domain model; the SQL schema belongs to areas 03 and 04 and is not restated here.

---

## 1. Domain models read by this area

### 1.1 `Chapter` — `domain/src/main/java/tachiyomi/domain/items/chapter/model/Chapter.kt`
| Field | Type | Nullable | Notes used by this area |
|---|---|---|---|
| `id` | `Long` | no | Identity; `removeDuplicates` matches `currentChapter.id` |
| `mangaId` | `Long` | no | not used here |
| `read` | `Boolean` | no | `getNextUnread` predicate `!read` |
| `bookmark` | `Boolean` | no | bookmark filter |
| `lastPageRead` | `Long` | no | not used here |
| `dateFetch` | `Long` | no | not used here |
| `sourceOrder` | `Long` | no | `CHAPTER_SORTING_SOURCE` comparator |
| `url` | `String` | no | not used here |
| `name` | `String` | no | downloaded-cache key; alphabet sort |
| `dateUpload` | `Long` | no | upload-date sort |
| `chapterNumber` | `Double` | no | number sort; `removeDuplicates` grouping key |
| `scanlator` | `String?` | yes | downloaded-cache key; `removeDuplicates` fallback |
| `lastModifiedAt` | `Long` | no | not used here |
| `version` | `Long` | no | not used here |
| `isRecognizedNumber` | `Boolean` | no | not used here |

### 1.2 `Episode` — `domain/src/main/java/tachiyomi/domain/items/episode/model/Episode.kt`
Same shape as `Chapter` with: `animeId: Long`, `seen: Boolean` (instead of `read`), `bookmark`, `fillermark: Boolean`, `lastSecondSeen: Long`, `totalSeconds: Long`, `dateFetch`, `sourceOrder`, `url`, `name`, `dateUpload`, `episodeNumber: Double`, `scanlator: String?`, `summary: String?`, `previewUrl: String?`, `lastModifiedAt`, `version`, `memo: JsonObject`, `isRecognizedNumber`. Used here: `seen`, `name`, `scanlator`, `episodeNumber`, `sourceOrder`, `dateUpload`, `id`.

### 1.3 `Manga` — `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt`
Used here: `id: Long`, `source: Long` (0L = local), `favorite: Boolean`, `coverLastModified: Long` (epoch ms), `url: String`, `title: String`, `status: Long` (SManga constants, section 3.4), `thumbnailUrl: String?`, `chapterFlags: Long`, `sorting: Long` (derived), `viewerFlags: Long`. `Manga.sortDescending()` at `Manga.kt:71`.

### 1.4 `Anime` — `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt`
Used here: `id: Long`, `source: Long`, `favorite: Boolean`, `coverLastModified: Long` (ms), `backgroundLastModified: Long` (ms), `title: String`, `status: Long`, `thumbnailUrl: String?`, `backgroundUrl: String?`, `viewerFlags: Long` (airing bits, section 4.3), `episodeFlags: Long`, `nextEpisodeToAir: Int` (derived, section 4.3), `nextEpisodeAiringAt: Long` (derived, seconds). `Anime.sortDescending()` at `Anime.kt:106`.

### 1.5 Tracks — `domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt`, `domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt`
- `remoteId: Long` (`AnimeTrack.kt:9`, `MangaTrack.kt:9`). For AniList it is the AniList media id, for MAL the MAL id, for Simkl the Simkl id. Used by `AniChartApi`.
- `AnimeTrackItem` (`eu.kanade.tachiyomi.ui.entries.anime.track.AnimeTrackItem`): `tracker: Tracker`, `track: AnimeTrack?`. Used as the input list of `loadAiringTime`.

### 1.6 Extension model — `app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt` (anime twin in `.../extension/anime/model/`)
Abstract members (lines 9-12) and installed variant (lines 18-21): `name: String`, `pkgName: String`, `versionName: String`, `versionCode: Long`, `isObsolete: Boolean` (used by CrashLogUtil). Available extension: `versionCode: Long`, `versionName: String`, `pkgName: String` (used as lookup key).

### 1.7 Source/extension manager surface used
- `AnimeSourceManager.isInitialized`, `MangaSourceManager.isInitialized` (`StateFlow`/`Boolean`), via `Injekt`.
- `MangaExtensionManager.availableExtensionsFlow.value`, `installedExtensionsFlow.value`; anime twin.

### 1.8 Image-cache collaborators (owned by area 34; listed for field usage)
- `MangaCoverCache`, `AnimeCoverCache`, `AnimeBackgroundCache`: `deleteFromCache(item, deleteCustom)`, `setCustomCoverToCache(item, InputStream)`, `setCustomBackgroundToCache`, `getCustomCoverFile(id)`, `getCoverFile(url)`.
- `LocalMangaCoverManager.update(SManga, InputStream)`, `LocalAnimeCoverManager`, `LocalAnimeBackgroundManager`, `LocalEpisodeThumbnailManager.update(SAnime, SEpisode, InputStream)`.
- `UpdateManga.awaitUpdateCoverLastModified(id)`, `UpdateAnime.awaitUpdateCoverLastModified(id)`, `awaitUpdateBackgroundLastModified(id)`.

---

## 2. Enums and their stored values

### 2.1 `ThemeMode` — `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt`
`LIGHT`, `DARK`, `SYSTEM`. Preference `pref_theme_mode_key`, default `ThemeMode.SYSTEM` (`UiPreferences.kt:20`). Storage: `preferenceStore.getEnum`, which persists the enum constant name (verify in `core/common/src/main/java/tachiyomi/core/common/preference/`).
Mapping to AppCompat: `LIGHT -> MODE_NIGHT_NO`, `DARK -> MODE_NIGHT_YES`, `SYSTEM -> MODE_NIGHT_FOLLOW_SYSTEM` (`ThemeMode.kt:11-17`). Rust: portal colour scheme (section 6.3).

### 2.2 `TabletUiMode` — `app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt`
`AUTOMATIC(titleRes automatic_background)`, `ALWAYS(lock_always)`, `LANDSCAPE(landscape)`, `NEVER(lock_never)`. Preference `tablet_ui_mode`, default `AUTOMATIC` (`UiPreferences.kt:37`).

### 2.3 `ReaderPreferences.TappingInvertMode` — `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt:183-195`
| Constant | `titleRes` | `shouldInvertHorizontal` | `shouldInvertVertical` |
|---|---|---|---|
| `NONE` | `tapping_inverted_none` | false | false |
| `HORIZONTAL` | `tapping_inverted_horizontal` | true | false |
| `VERTICAL` | `tapping_inverted_vertical` | false | true |
| `BOTH` | `tapping_inverted_both` | true | true |

### 2.4 Reader background theme (int preference) — `ReaderPreferences.kt:57`
`pref_reader_theme_key`, `getInt(..., 1)`. Values (from `GeneralSettingsPage.kt:19-24`): `0` White, `1` Black (default), `2` Gray, `3` Automatic.

### 2.5 `SecurityPreferences.SecureScreenMode` — `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:30-34`
`ALWAYS(lock_always)`, `INCOGNITO(pref_incognito_mode)`, `NEVER(lock_never)`. Preference `secure_screen_v2`, default `INCOGNITO` (`:17`).

### 2.6 `AppTheme` — `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7-10` (first entries)
`DEFAULT(label_default)`, `MONET(theme_monet)`, `CLOUDFLARE(AYMR theme_cloudflare)`, further entries not in scope. Preference `pref_app_theme`, default `MONET` when `DeviceUtil.isDynamicColorAvailable`, else `DEFAULT` (`UiPreferences.kt:22-29`). Rust: `AppTheme` is dropped in favour of the user's GTK theme (task T-52a-063).

### 2.7 `TriState` — `domain/src/main/java/tachiyomi/domain/entries/TriState.kt`
`DISABLED`, `ENABLED_IS`, `ENABLED_NOT`. Semantics in behaviour spec section 5.1.

### 2.8 `CheckboxState.TriState` — `core/common` (`tachiyomi.core.common.preference.CheckboxState`)
`None`, `Include`, `Exclude`. Mapping in behaviour spec section 2.

### 2.9 `NetworkState` (`NetworkStateTracker.kt`)
Data class: `isConnected: Boolean`, `isValidated: Boolean`, `isWifi: Boolean`, derived `isOnline`.

### 2.10 `LocaleHelper` sentinel strings
`PINNED_KEY = "pinned"`, `LAST_USED_KEY = "last_used"` (`LocaleHelper.kt:84-85`); `"all"` and `"other"` are literal language keys.

### 2.11 `ChapterList.Item` / `EpisodeList.Item`
Wrapper types from the UI (`eu.kanade.tachiyomi.ui.entries.manga.ChapterList.Item`, `eu.kanade.tachiyomi.ui.entries.anime.EpisodeList.Item`) with a `.chapter` / `.episode` domain object and `isDownloaded: Boolean` (used by the list overloads of `getNextUnread` / `getNextUnseen` and `applyFilters`).

---

## 3. Bit-flag layouts

All flags are `Long`. Masks are hex, LSB = bit 0.

### 3.1 Manga `chapterFlags` — `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:79-99`
| Bits | Mask | Constants (value) | Meaning |
|---|---|---|---|
| 0 | `CHAPTER_SORT_DIR_MASK = 0x1` | `CHAPTER_SORT_DESC = 0x0`, `CHAPTER_SORT_ASC = 0x1` | direction; default 0 = descending |
| 1-2 | `CHAPTER_UNREAD_MASK = 0x6` | `CHAPTER_SHOW_UNREAD = 0x2`, `CHAPTER_SHOW_READ = 0x4` | unread filter (both 0 = show all) |
| 3-4 | `CHAPTER_DOWNLOADED_MASK = 0x18` | `CHAPTER_SHOW_DOWNLOADED = 0x8`, `CHAPTER_SHOW_NOT_DOWNLOADED = 0x10` | downloaded filter |
| 5-6 | `CHAPTER_BOOKMARKED_MASK = 0x60` | `CHAPTER_SHOW_BOOKMARKED = 0x20`, `CHAPTER_SHOW_NOT_BOOKMARKED = 0x40` | bookmark filter |
| 8-9 | `CHAPTER_SORTING_MASK = 0x300` | `CHAPTER_SORTING_SOURCE = 0x0`, `CHAPTER_SORTING_NUMBER = 0x100`, `CHAPTER_SORTING_UPLOAD_DATE = 0x200`, `CHAPTER_SORTING_ALPHABET = 0x300` | sort key |
| all zero | — | `SHOW_ALL = 0x0` | "no filter" |

Rule: a filter value of `0` in a mask means `DISABLED` (`TriState.DISABLED`, keep all).

### 3.2 Anime `episodeFlags` — `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:217-255`
| Bits | Mask | Constants (value) |
|---|---|---|
| 0 | `EPISODE_SORT_DIR_MASK = 0x1` | `EPISODE_SORT_DESC = 0x0`, `EPISODE_SORT_ASC = 0x1` |
| 1-2 | `EPISODE_UNSEEN_MASK = 0x6` | `EPISODE_SHOW_UNSEEN = 0x2`, `EPISODE_SHOW_SEEN = 0x4` |
| 3-4 | `EPISODE_DOWNLOADED_MASK = 0x18` | `EPISODE_SHOW_DOWNLOADED = 0x8`, `EPISODE_SHOW_NOT_DOWNLOADED = 0x10` |
| 5-6 | `EPISODE_BOOKMARKED_MASK = 0x60` | `EPISODE_SHOW_BOOKMARKED = 0x20`, `EPISODE_SHOW_NOT_BOOKMARKED = 0x40` |
| 7-8 | `EPISODE_FILLERMARKED_MASK = 0x180` | `EPISODE_SHOW_FILLERMARKED = 0x80`, `EPISODE_SHOW_NOT_FILLERMARKED = 0x100` |
| 9-10 | `EPISODE_SORTING_MASK = 0x600` | `EPISODE_SORTING_SOURCE = 0x0`, `EPISODE_SORTING_NUMBER = 0x200`, `EPISODE_SORTING_UPLOAD_DATE = 0x400`, `EPISODE_SORTING_ALPHABET = 0x600` |
| 11 | `EPISODE_PREVIEWS_MASK = 0x800` | `EPISODE_SHOW_PREVIEWS = 0x0`, `EPISODE_SHOW_NOT_PREVIEWS = 0x800` |
| 12 | `EPISODE_SUMMARIES_MASK = 0x1000` | `EPISODE_SHOW_SUMMARIES = 0x0`, `EPISODE_SHOW_NOT_SUMMARIES = 0x1000` |
| 20 | `EPISODE_DISPLAY_MASK = 0x100000` | `EPISODE_DISPLAY_NAME = 0x0`, `EPISODE_DISPLAY_NUMBER = 0x100000` |

### 3.3 Anime `viewerFlags` airing and intro fields — `Anime.kt:79-83`, `:310-313`
| Field | Mask | Bits | Encoding | Decoding (`Anime.kt`) |
|---|---|---|---|---|
| `nextEpisodeToAir` | `ANIME_AIRING_EPISODE_MASK = 0x000000000FFFF00` | 8..23 (16 bits) | `episode * 256` (`addHexZeros(2)`) | `(flags and mask) / 256` (`removeHexZeros(2)`), `.toInt()` |
| `nextEpisodeAiringAt` | `ANIME_AIRING_TIME_MASK = 0x0FFFFFFFF000000` | 24..55 (32 bits) | `seconds * 2^24` (`addHexZeros(6)`) | `(flags and mask) / 2^24` |
| `skipIntro` length | `ANIME_INTRO_MASK = 0x0000000000000FF` | 0..7 | raw value | `.toInt()` |
| `skipIntroDisable` | `ANIME_INTRO_DISABLE_MASK = 0x100000000000000` | bit 56 | set when intro length is 0 (`SetAnimeViewerFlags.kt` writes `(flag == 0L)` at `addHexZeros(14)`) | `(flags and mask) == mask` |

Write rule (`SetAnimeViewerFlags.kt:30-58`): `new = (old and mask.inv()) or (value and mask)`. Values wider than the mask are truncated silently.

### 3.4 `SAnime` / `SManga` status values — `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-74`, `.../source/model/SManga.kt:48-50`
`UNKNOWN = 0`, `ONGOING = 1`, `COMPLETED = 2`. (`SManga` also defines `LICENSED`, `PUBLISHING_FINISHED`, `CANCELLED`, `ON_HIATUS`; not read by this area.) Stored in `Anime.status` / `Manga.status` as `Long`. `AniChartApi` compares `anime.status == 2L`.

### 3.5 Remote-sync note (not in scope)
`Anime.kt:237-251` also defines `EPISODE_SORTING_*` used as above. The `seasonFlags` layout (`SEASON_*`) is owned by area 39.

---

## 4. Preference keys read by util code

Key format: `Preference.appStateKey(key)` prefixes `__APP_STATE_` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-45`). App-state keys are not user-editable preferences; they are stored in a separate store and cleared on reset.

| Key (literal) | Owner | Type | Default | Read by | Purpose |
|---|---|---|---|---|---|
| `__APP_STATE_pref_downloaded_only` (via `appStateKey("pref_downloaded_only")`) | `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:16-18` | Boolean | `false` | `Manga.downloadedFilter`, `ReaderViewModel`, `PlayerViewModel` | downloaded-only mode |
| `__APP_STATE_incognito_mode` (via `appStateKey("incognito_mode")`) | `BasePreferences.kt:21` | Boolean | `false` | `SecureActivityDelegate` (secure screen) | incognito |
| `pref_theme_mode_key` | `UiPreferences.kt:20` | enum `ThemeMode` | `SYSTEM` | `createReaderThemeContext`, `isNightMode` | app theme mode |
| `pref_app_theme` | `UiPreferences.kt:22-29` | enum `AppTheme` | `MONET` if dynamic colour else `DEFAULT` | `AppThemePreferenceWidget` (drop) | app colour theme |
| `pref_theme_dark_amoled_key` | `UiPreferences.kt:31` | Boolean | `false` | `createReaderThemeContext` (theme ids) | AMOLED dark |
| `tablet_ui_mode` | `UiPreferences.kt:37` | enum `TabletUiMode` | `AUTOMATIC` | `prepareTabletUiContext` | tablet layout |
| `pref_reader_theme_key` | `ReaderPreferences.kt:57` | Int | `1` (Black) | `createReaderThemeContext` | reader background |
| `skip_dupe` | `ReaderPreferences.kt:87` | Boolean | `false` | `ReaderViewModel` (uses `removeDuplicates`) | skip duplicate chapters |
| `pref_reader_flash` | `ReaderPreferences.kt:18` | Boolean | `false` | reader (not in scope) | flash on page change |
| `use_biometric_lock` | `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:13` | Boolean | `false` | `SecureActivityDelegate`, `AuthenticatorUtil` callers | app lock enabled |
| `lock_app_after` | `SecurityPreferences.kt:15` | Int (minutes) | `0` | `SecureActivityDelegate` | lock delay: `-1` never, `0` always, `n > 0` after n minutes |
| `secure_screen_v2` | `SecurityPreferences.kt:17` | enum `SecureScreenMode` | `INCOGNITO` | `SecureActivityDelegate` | secure screen |
| `__APP_STATE_last_app_closed` (via `appStateKey("last_app_closed")`) | `SecurityPreferences.kt:25-28` | Long (epoch ms) | `0` | `SecureActivityDelegate` | lock timing |
| `ui_mode`-derived `UI_MODE_NIGHT` (system) | Android configuration | — | — | `isNightMode` | replaced by portal colour scheme |
| `ro.miui.ui.version.name` (system property) | `core/common/.../DeviceUtil.kt` | string | — | `DeviceUtil.isMiui` | Drop (no MIUI on Linux) |

Migration note: `MigrateSecureScreenMigration.kt:21` reads the old boolean `secure_screen`; upgrade path belongs to area 11 (migrations), not the Rust port.

Rust preference names (Proposal, `kuro-prefs`): keep the same key strings so a future settings import keeps working; drop the `__APP_STATE_` prefix and store app-state values in a separate `[state]` table of the same settings file.

---

## 5. Files, folders and formats on disk

### 5.1 Android paths (observed)
| Path | Created by | Format | Used by |
|---|---|---|---|
| `externalCacheDir/aniyomi_crash_logs.txt` | `Context.createFileInCacheDir` (`ContextExtensions.kt:104-111`) | UTF-8 text, appended sections, see behaviour section 8 | `CrashLogUtil.dumpLogs` |
| `cacheDir/<COVERS_DIR>/<md5(thumbnailUrl)>` | `MangaCoverCache` | image bytes (format from source) | `deleteFromCache` |
| `cacheDir/<CUSTOM_COVERS_DIR>/<md5(mangaId)>` | `MangaCoverCache.setCustomCoverToCache` | raw stream copy | `hasCustomCover`, `removeCovers` |
| `<COVERS_DIR>` and `<CUSTOM_COVERS_DIR>` | `MangaCoverCache.kt:28-30` | directory names are constants in the cache class (not in scope) | — |
| Anime background/cover equivalents | `AnimeBackgroundCache.kt`, `AnimeCoverCache.kt` (area 34) | — | `AnimeExtensions.kt` |
| Episode thumbnail (local source) | `LocalEpisodeThumbnailManager.update` | image | `editThumbnail` |

Hash: `DiskUtil.hashKeyForDisk(key) = Hash.md5(key)` (`core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:32-34`). MD5 of the UTF-8 key, lowercase hex.

### 5.2 Rust paths (Proposal)
| Path | Purpose |
|---|---|
| `$XDG_CACHE_HOME/kurobayomi/` (via `glib::user_cache_dir()`) | crash logs, image caches |
| `$XDG_CACHE_HOME/kurobayomi/covers/<md5>` | thumbnail cache (same naming as upstream) |
| `$XDG_CACHE_HOME/kurobayomi/covers/custom/<md5>` | custom covers (same naming) |
| `$XDG_CACHE_HOME/kurobayomi/kurobayomi_crash_logs.txt` | manual crash dump (replaces `aniyomi_crash_logs.txt`) |
| `$XDG_CACHE_HOME/kurobayomi/crash_<unix-seconds>.txt` | panic hook dump |
| `$XDG_DATA_HOME/kurobayomi/` | database and downloads (areas 03, 23-25) |
| `$XDG_CONFIG_HOME/kurobayomi/` | preferences (`kuro-prefs`) |
| `/etc/pam.d/kurobayomi` (installed by the package) | PAM service for app lock |
| `~/.config/kdeglobals` (read only) | KDE `AnimationDurationFactor` |
| `~/Downloads` | default save location for "Save as" dialogs (portal picks) |

### 5.3 Temporary write format
- Save-stream temp name: `<dest file name>.part` in the same directory (Proposal, section 9.4 of behaviour), renamed on success.

### 5.4 Size and date text formats
- Byte sizes: decimal, two decimals for KB/MB/GB, plain integer for B (section 9.2 of behaviour). Strings `B`, `KB`, `MB`, `GB` are literals.
- Timestamps: `chrono` ISO-8601 with offset in crash reports (`OffsetDateTime.now().toString()` upstream: `yyyy-MM-ddTHH:mm:ss.SSS+zz:zz` form, verify minute precision).

---

## 6. Network endpoints used by this area

### 6.1 AniList GraphQL
- URL: `https://graphql.anilist.co`
- Method: `POST`
- Headers: `Content-Type: application/json; charset=utf-8` (from `jsonMime`). No auth (public queries).
- Body: JSON object `{"query": "<graphql>"}`.
- Queries (exact text as upstream builds them, whitespace normalised):
  - MAL to AniList id: `query { Media(idMal:<idMal>,type: ANIME) { id } }`
  - Airing: `query { Media(id:<id>) { nextAiringEpisode { episode airingAt } } }`
- Response (used fields): `data.Media.id` (Int), `data.Media.nextAiringEpisode.episode` (Int), `.airingAt` (Long, Unix seconds). `nextAiringEpisode` may be `null`.
- Timeouts: OkHttp defaults (10 s connect/read/write). No retry. Rust: `reqwest` with 10 s timeouts.

### 6.2 Simkl calendar
- URLs (in this order): `https://data.simkl.in/calendar/anime.json`, `https://data.simkl.in/calendar/tv.json`, `https://data.simkl.in/calendar/movie_release.json`.
- Method: `GET`. Headers: none beyond OkHttp defaults and `Cache-Control: max-age=600` (request header from `GET` helper). No auth.
- Response: JSON array. Entry shape (observed, `https://data.simkl.in/calendar/anime.json`, first 700 bytes read 2026-10-08):
  ```
  {
    "title": "<string>",
    "poster": "<string>",
    "date": "<ISO-8601 with offset, e.g. 2026-10-07T00:00:00+09:00>",
    "release_date": "<YYYY-MM-DD>",
    "rank": <int>,
    "ratings": {"simkl": {"rating": <float>, "votes": <int>}},
    "url": "<string>",
    "ids": {"simkl_id": <int>, "slug": "<string>", "tmdb": <int|null>, "mal": "<string>|null"},
    "episode": {"episode": <int>, "url": "<string>"},
    "anime_type": "<string>"
  }
  ```
  Note `ids.mal` is a JSON STRING (e.g. `"62248"`) or `null`. `episode.episode` is an integer.
- Size: the full files are large (the calendar covers many entries). Rust: stream-parse or read to `Vec<u8>` with a 16 MiB cap (Proposal).
- Failure: any transport error aborts that calendar type and returns `(1, 0)` (observed, section 7.6 of behaviour).

### 6.3 Desktop portals and services used by this area (Proposal)
| Service | Interface | Used for |
|---|---|---|
| `org.freedesktop.portal.Settings` | `Read("org.freedesktop.appearance", "color-scheme")`, `SettingChanged` | dark mode (`isNightMode`) |
| `org.freedesktop.portal.OpenURI` | `OpenURI` (via `gtk::UriLauncher`) | open URLs |
| `org.freedesktop.portal.FileChooser` | `SaveFile`, `OpenFile` (via `gtk::FileDialog`) | save crash logs, covers, backups |
| `org.freedesktop.portal.Notification` | `AddNotification`, `RemoveNotification` (via `gio::Notification`) | notifications |
| `org.freedesktop.portal.Inhibit` | `Inhibit` flag `IDLE` (via `gtk::Application::inhibit`) | keep screen on during playback |
| `org.freedesktop.NetworkManager` | `State`, `Connectivity`, `PrimaryConnectionType`, `ActiveConnections`, `StateChanged` | online and Wi-Fi checks (`zbus`) |
| `org.freedesktop.login1` | not used | — |
| `net.reactivated.Fprint` (optional) | `Manager`, `Device.VerifyStart` | optional fingerprint for app lock |
| PAM (`libpam`) | `pam_authenticate` service `kurobayomi` | app lock |

### 6.4 Clipboard
- `gdk::Display::default().clipboard().set_text(text)`. No endpoint.

---

## 7. Database tables and columns touched

This area does not own SQL. The fields it reads or writes through repositories are:
- `anime.viewer_flags` (write) — airing bits (section 3.3). Written by `SetAnimeViewerFlags` through `AnimeUpdate(viewerFlags)`. Owner: area 04 (`04-db-anime-schema-migrations`), repository area 05.
- `anime.cover_last_modified`, `anime.background_last_modified` (write) — via `UpdateAnime.awaitUpdateCoverLastModified` / `awaitUpdateBackgroundLastModified`. Owner: area 05.
- `manga.cover_last_modified` (write) — via `UpdateManga.awaitUpdateCoverLastModified`. Owner: area 05.
- `chapters.read`, `chapters.bookmark`, `chapters.name`, `chapters.scanlator`, `chapters.chapter_number`, `chapters.source_order`, `chapters.date_upload` (read). Owner: area 03/05.
- `episodes.seen`, `episodes.bookmark`, `episodes.name`, `episodes.scanlator`, `episodes.episode_number`, `episodes.source_order`, `episodes.date_upload` (read). Owner: area 04/05.
- `manga.chapter_flags`, `anime.episode_flags` (read) — sort and filter bits (section 3).

Upstream table/column names are as in the SQLDelight schema under `data/src/main/sqldelight/` (areas 03/04). Verify before writing SQL; they are not restated here to avoid drift.

---

## 8. Build-time constants

| Constant | Source | Value | Rust |
|---|---|---|---|
| `BuildConfig.APPLICATION_ID` | Android | app id | `APP_ID` const in `kurobayomi` |
| `BuildConfig.VERSION_NAME` | Android | version | `env!("CARGO_PKG_VERSION")` |
| `BuildConfig.VERSION_CODE` | Android | int | dropped (no Android version code) |
| `BuildConfig.COMMIT_SHA` | Android | git sha | `option_env!("KUROBAYOMI_COMMIT")` set by build script; empty when unset |
| `BuildConfig.BUILD_TIME` | Android | timestamp | `build.rs` writes `KUROBAYOMI_BUILD_TIME` (UTC RFC-3339) |
| `BuildConfig.BUILD_TYPE` | Android | `debug`/`preview`/`release` | `cfg!(debug_assertions)` (see behaviour 9.12) |
| `BuildConfig.UPDATER_ENABLED` | Android | bool | `false` |
| libmpv / FFmpeg / libplacebo versions | literals in `CrashLogUtil.kt:56-58` | `6764488`, `v7.349.0`, `n7.1` | read at runtime from libmpv |

---

## 9. Lookup tables derived from source

### 9.1 Reader theme chips (`GeneralSettingsPage.kt:19-24`)
| Label (MR key) | Value |
|---|---|
| `black_background` (`Black`) | 1 |
| `gray_background` (`Gray`) | 2 |
| `white_background` (`White`) | 0 |
| `automatic_background` (`Auto`) | 3 |

### 9.2 Flash styles (`GeneralSettingsPage.kt:27-29`, not in scope, listed for reference)
`pref_flash_style_black` -> `FlashColor.BLACK`, `pref_flash_style_white` -> `WHITE`, `pref_flash_style_white_black` -> `WHITE_BLACK`.

### 9.3 Language sentinels (`LocaleHelper`)
`all` (multi, label `multi_lang` = `Multi`), `other` (`other_source` = `Other`), `pinned` (`pinned_sources` = `Pinned`), `last_used` (`last_used_source` = `Last used`).

### 9.4 Chinese locale normalisation (`LocaleHelper.kt:42-44`, `:61-63`)
`zh-CN` -> `zh-Hans`; `zh-TW` -> `zh-Hant`.

### 9.5 Invalid default browsers (`DeviceUtil.kt:74-84`)
`android`, `com.hihonor.android.internal.app`, `com.huawei.android.internal.app`, `com.zui.resolver`, `com.transsion.resolver`, `com.android.intentresolver` (Android only; Drop).

### 9.6 Tablet thresholds (`DisplayExtensions.kt:12-18`)
720 dp (forced tablet min width), 700 dp (portrait auto), 600 dp (landscape auto).

### 9.7 Relative-time count rules
See behaviour sections 10.2 and 10.3 (thresholds 7 days, 24 h, 1 h, 1 min).

### 9.8 Animation constants
`crossfade = 300 * scale` ms (`App.kt:203`); reader page animations use `coerceAtLeast(1)` (`ReaderPageImageView.kt:421`).

### 9.9 AniChart constants
Calendar types in order: `anime`, `tv`, `movie_release`. Default tuple `(1, 0)`. Cache header `max-age=600`. Status for completed: `2`.

### 9.10 Notification importance (Android ints, passed by callers)
Android `NotificationManagerCompat` values used by `buildNotificationChannel`: `IMPORTANCE_MIN = 1`, `LOW = 2`, `DEFAULT = 3`, `HIGH = 4`, `MAX = 5`. Rust mapping in behaviour section 15.2.

### 9.11 Transport constants (`NetworkCapabilities`, used by `isOnline`)
`CELLULAR=0`, `WIFI=1`, `BLUETOOTH=2`, `ETHERNET=3`, `VPN=4`, `WIFI_AWARE=5`, `LOWPAN=6`. Range checked: `0..6` (API 27+), `0..5` older.

### 9.12 NetworkManager constants (Proposal)
`State`: `UNKNOWN=0`, `ASLEEP=10`, `DISCONNECTED=20`, `DISCONNECTING=30`, `CONNECTING=40`, `CONNECTED_LOCAL=50`, `CONNECTED_SITE=60`, `CONNECTED_GLOBAL=70`. `Connectivity`: `UNKNOWN=0`, `NONE=1`, `PORTAL=2`, `LIMITED=3`, `FULL=4`.

### 9.13 `gio::NetworkConnectivity` (Proposal)
`LOCAL=1`, `LIMITED=2`, `PORTAL=3`, `FULL=4`.
