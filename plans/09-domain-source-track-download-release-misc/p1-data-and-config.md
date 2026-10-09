# Area 09 — data models, storage, preferences, endpoints

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Types: Kotlin types as written in source; Rust mapping in `p1-rust-design.md`. "Nullable" = Kotlin `?`. Defaults are the Kotlin defaults.

## 1. Domain models

### 1.1 Source models

**`Source`** — `domain/src/main/java/tachiyomi/domain/source/manga/model/Source.kt:3-14`
| Field | Type | Null | Default | Notes |
|---|---|---|---|---|
| `id` | `Long` | no | — | Source key (installed sources use the upstream id) |
| `lang` | `String` | no | — | ISO-like language code, may be empty |
| `name` | `String` | no | — | |
| `supportsLatest` | `Boolean` | no | — | |
| `isStub` | `Boolean` | no | — | true when built from `StubMangaSource` |
| `pin` | `Pins` | no | `Pins.unpinned` (code 0) | bit flags, section 2.3 |
| `isUsedLast` | `Boolean` | no | `false` | true for the "last used" pseudo-entry |
| `isExcludedFromDataSaver` | `Boolean` | no | `false` | SY fork flag, dropped (T-09a-056) |
Derived: `visualName` (String), `key: () -> String`.

**`AnimeSource`** — `domain/src/main/java/tachiyomi/domain/source/anime/model/AnimeSource.kt:3-11`: same fields without `isExcludedFromDataSaver`.

**`MangaSourceWithCount`** — `…/manga/model/MangaSourceWithCount.kt:3-5`: `source: Source`, `count: Long`. Derived `id`, `name`.

**`AnimeSourceWithIds`** — `…/anime/model/AnimeSourceWithIds.kt:3-7`: `source: AnimeSource`, `ids: List<Long>`, `orphaned: List<Long>`. Derived `count: Long = ids.size`, `id = source.id`, `name = source.name`.

**`DeletableAnime`** — `…/anime/model/DeletableAnime.kt:5-9`: `animeId: Long`, `sourceId: Long`, `fetchType: FetchType`.
**`FetchType`** (upstream, `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt:10-19`): `Seasons`, `Episodes`.

**`StubMangaSource`** / **`StubAnimeSource`** — `…/manga/model/StubMangaSource.kt:9-13`, `…/anime/model/StubAnimeSource.kt:15-19`: `id: Long`, `lang: String`, `name: String`; private `isInvalid: Boolean = name.isBlank() || lang.isBlank()`.

### 1.2 Track models

**`MangaTrack`** — `domain/src/main/java/tachiyomi/domain/track/manga/model/MangaTrack.kt:5-20` implements `java.io.Serializable`.
| Field | Type | Null | Stored as (manga_sync) |
|---|---|---|---|
| `id` | `Long` | no | `_id INTEGER PRIMARY KEY` |
| `mangaId` | `Long` | no | `manga_id INTEGER NOT NULL` |
| `trackerId` | `Long` | no | `sync_id INTEGER NOT NULL` |
| `remoteId` | `Long` | no | `remote_id INTEGER NOT NULL` |
| `libraryId` | `Long?` | yes | `library_id INTEGER` |
| `title` | `String` | no | `title TEXT NOT NULL` |
| `lastChapterRead` | `Double` | no | `last_chapter_read REAL NOT NULL` |
| `totalChapters` | `Long` | no | `total_chapters INTEGER NOT NULL` |
| `status` | `Long` | no | `status INTEGER NOT NULL` |
| `score` | `Double` | no | `score REAL NOT NULL` |
| `remoteUrl` | `String` | no | `remote_url TEXT NOT NULL` |
| `startDate` | `Long` | no | `start_date INTEGER NOT NULL` |
| `finishDate` | `Long` | no | `finish_date INTEGER NOT NULL` |
| `private` | `Boolean` | no | `private INTEGER AS Boolean DEFAULT 0 NOT NULL` |

**`AnimeTrack`** — `domain/src/main/java/tachiyomi/domain/track/anime/model/AnimeTrack.kt:5-20`: identical except `animeId`, `lastEpisodeSeen: Double`, `totalEpisodes: Long`; table `anime_sync` with `anime_id`, `last_episode_seen REAL`, `total_episodes INTEGER`.

Tracker identity is `trackerId` (`sync_id`), not a foreign table; the tracker registry is in area 27.

### 1.3 Extension repos (manga)

**`ExtensionRepo`** — `domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt:3-9`:
| Field | Type | Null | Column |
|---|---|---|---|
| `baseUrl` | `String` | no | `base_url TEXT NOT NULL PRIMARY KEY` |
| `name` | `String` | no | `name TEXT NOT NULL` |
| `shortName` | `String?` | yes | `short_name TEXT` |
| `website` | `String` | no | `website TEXT NOT NULL` |
| `signingKeyFingerprint` | `String` | no | `signing_key_fingerprint TEXT UNIQUE NOT NULL` |

**`ExtensionRepoMetaDto`** / **`ExtensionRepoDto`** (wire) — `…/service/ExtensionRepoDto.kt:6-17`: `ExtensionRepoMetaDto { meta: ExtensionRepoDto }`; `ExtensionRepoDto { name: String, shortName: String?, website: String, signingKeyFingerprint: String }`. Serialization: kotlinx.serialization, default field names (no `@SerialName`), so JSON keys are `meta`, `name`, `shortName`, `website`, `signingKeyFingerprint`. Mapping `toExtensionRepo(baseUrl)` copies the four fields and sets `baseUrl` from the request URL (ExtensionRepoDto.kt:19-26).

### 1.4 Extension stores (anime)

**`AnimeExtensionStore`** — `domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt:3-15`:
| Field | Type | Null | Column (`extension_store`) |
|---|---|---|---|
| `indexUrl` | `String` | no | `index_url TEXT NOT NULL PRIMARY KEY` |
| `name` | `String` | no | `name TEXT NOT NULL` |
| `badgeLabel` | `String` | no | `badge_label TEXT NOT NULL` |
| `signingKey` | `String` | no | `signing_key TEXT NOT NULL` |
| `contact` | `Contact` | no | flattened: `contact_website TEXT NOT NULL`, `contact_discord TEXT` |
| `isLegacy` | `Boolean` | no | `is_legacy INTEGER AS Boolean NOT NULL` |
| `extensionListUrl` | `String?` | yes | `extension_list_url TEXT` |

`Contact { website: String; discord: String? }` (nested class, same file lines 12-15).

### 1.5 Custom buttons

**`CustomButton`** — `domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt:3-11`:
| Field | Type | Null | Column (`custom_buttons`) |
|---|---|---|---|
| `id` | `Long` | no | `_id INTEGER NOT NULL PRIMARY KEY` |
| `name` | `String` | no | `name TEXT NOT NULL` |
| `isFavorite` | `Boolean` | no | `isFavorite INTEGER AS Boolean NOT NULL` |
| `sortIndex` | `Long` | no | `sortIndex INTEGER NOT NULL` |
| `content` | `String` | no | `content TEXT NOT NULL` |
| `longPressContent` | `String` | no | `longPressContent TEXT NOT NULL` |
| `onStartup` | `String` | no | `onStartup TEXT NOT NULL` |

**`CustomButtonUpdate`** — `…/model/CustomButtonUpdate.kt:3-10`: `id: Long` (required), and optional (`= null`) `name: String?`, `isFavorite: Boolean?`, `sortIndex: Long?`, `content: String?`, `longPressContent: String?`, `onStartup: String?`.

### 1.6 Release

**`Release`** — `domain/src/main/java/tachiyomi/domain/release/model/Release.kt:6-11`: `version: String`, `info: String`, `releaseLink: String`, `downloadLink: String` (all non-null).

**`GetApplicationRelease.Arguments`** — `domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt:78-84`: `isPreview: Boolean`, `commitCount: Int`, `versionName: String`, `repository: String`, `forceCheck: Boolean = false`.

**`GithubRelease`** (data layer, `data/src/main/java/tachiyomi/data/release/GithubRelease.kt:9-29`): `tag_name` → `version: String`; `body` → `info: String`; `html_url` → `releaseLink: String`; `assets: List<GitHubAsset>`; `GitHubAsset { name: String; browser_download_url → downloadLink: String }`.

### 1.7 Upcoming and items

Upcoming returns `Anime`/`Manga` domain entries (owned by area 07); the only scope-local data is the status sets (section 3.5).

### 1.8 Backup flags and items-filter inputs

- `FLAG_*` are `String` constants (section 2.4).
- Filter inputs: `Chapter` / `Episode` (area 07), `Category.id: Long` (area 08), `DownloadPreferences` values (section 4).

## 2. Enums, value encodings and bit layouts

### 2.1 Pin codes (`Pin`, both kinds)
| Name | Code (binary) | Decimal | Source |
|---|---|---|---|
| `Unpinned` | `0b00` | 0 | `domain/src/main/java/tachiyomi/domain/source/manga/model/Pin.kt:4` |
| `Pinned` | `0b01` | 1 | Pin.kt:5 |
| `Actual` | `0b10` | 2 | Pin.kt:6 |
| `Pins.pinned` (`Pinned` + `Actual`) | `0b11` | 3 | Pin.kt:28 |
In memory `Pins` wraps a plain `Int` (`code`). Where the pin value is persisted is not defined in this scope (the pin writer lives in the source-list code, outside area 09); Proposal: persist `Pins.code` as an integer column or preference when the source list is built.

### 2.2 Source status constants (upstream, Long in domain)
`SManga` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54`): `UNKNOWN=0`, `ONGOING=1`, `COMPLETED=2`, `LICENSED=3`, `PUBLISHING_FINISHED=4`, `CANCELLED=5`, `ON_HIATUS=6`.
`SAnime` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79`): same values plus `UPCOMING=7`.

### 2.3 Upcoming status sets
- Anime `{1, 4, 7}`; manga `{1, 4}` (see behavior section 2).

### 2.4 Backup flag tags (`PreferenceValues.kt:3-9`)
| Constant | Value (String) | Meaning (per upstream naming) |
|---|---|---|
| `FLAG_CATEGORIES` | `"1"` | categories |
| `FLAG_CHAPTERS` | `"2"` | chapters (manga) |
| `FLAG_HISTORY` | `"4"` | history |
| `FLAG_TRACK` | `"8"` | track |
| `FLAG_SETTINGS` | `"10"` | app settings |
| `FLAG_EXT_SETTINGS` | `"20"` | extension settings |
| `FLAG_EXTENSIONS` | `"40"` | extensions |
Encoded as strings in a `StringSet`. They look like bit masks (1, 2, 4, 8, 0x10, 0x20, 0x40) but are never OR-ed together in scope; the Rust port keeps them as string tags (`enum BackupFlag { Categories, Chapters, History, Track, Settings, ExtSettings, Extensions }` with `as_tag()`), and an explicit mapping to the on-disk backup format is owned by area 21.

### 2.5 Anime `episode` / manga `chapter` numbers
`chapterNumber` and `episodeNumber` are `Float`/`Double` in models owned by area 07; "recognized" means `>= 0f` (Chapter.kt:19-20; Episode.kt:27-28).

### 2.6 Category id conventions
- `UNCATEGORIZED_ID = 0L` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:16`). `isSystemCategory = id == UNCATEGORIZED_ID` (Category.kt:13).
- Category id sets are stored in preferences as decimal strings (`toLong()` on read, section 4.2).

### 2.7 Application release versions
- Preview tags: `r<digits>` (e.g. `r2000`), compared to `commitCount: Int`.
- Release tags: `v<semver>` or `<semver>` (e.g. `v2.0.0`), compared after stripping every non-`[0-9.]` character.

## 3. Database tables and SQL (owned by data layer; restated because the domain contract depends on them)

### 3.1 `extension_repos` (manga DB) — `data/src/main/sqldelight/data/extension_repos.sq`
```sql
CREATE TABLE extension_repos (
    base_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT,
    website TEXT NOT NULL,
    signing_key_fingerprint TEXT UNIQUE NOT NULL
);
```
Queries (exact):
- `findOne`: `SELECT * FROM extension_repos WHERE base_url = :base_url;`
- `findOneBySigningKeyFingerprint`: `SELECT * FROM extension_repos WHERE signing_key_fingerprint = :fingerprint;`
- `findAll`: `SELECT * FROM extension_repos;` (no ORDER BY; Proposal: `ORDER BY name COLLATE NOCASE`)
- `count`: `SELECT COUNT(*) FROM extension_repos;`
- `insert`: `INSERT INTO extension_repos(base_url, name, short_name, website, signing_key_fingerprint) VALUES (:base_url, :name, :short_name, :website, :fingerprint);`
- `upsert`: same INSERT `ON CONFLICT(base_url) DO UPDATE SET name = :name, short_name = :short_name, website =: website, signing_key_fingerprint = :fingerprint WHERE base_url = base_url;`
- `replace`: same INSERT `ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website =: website WHERE signing_key_fingerprint = signing_key_fingerprint;`
- `delete`: `DELETE FROM extension_repos WHERE base_url = :base_url;`

Note: `website =: website` is SQLDelight/SQLite-accepted shorthand for `website = :website`; the Rust port writes `website = ?`.

Mapping: `MangaExtensionRepoRepositoryImpl.kt:80-92`. Exceptions: `SQLiteException` → `SaveExtensionRepoException` on `insertRepo` and `upsertRepo` only (`:43-47`, `:57-61`); `replaceRepo` and `deleteRepo` do not wrap.

### 3.2 `extension_store` (anime DB) — `data/src/main/sqldelightanime/dataanime/extension_store.sq`
```sql
CREATE TABLE extension_store(
    index_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    badge_label TEXT NOT NULL,
    signing_key TEXT NOT NULL,
    contact_website TEXT NOT NULL,
    contact_discord TEXT,
    is_legacy INTEGER AS Boolean NOT NULL,
    extension_list_url TEXT
);
```
Queries:
- `get`: `SELECT * FROM extension_store WHERE index_url = :indexUrl;`
- `getAll`: `SELECT * FROM extension_store;`
- `getCount`: `SELECT COUNT(*) FROM extension_store;`
- `upsert`: `INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url) VALUES (:indexUrl, :name, :badgeLabel, :signingKey, :contactWebsite, :contactDiscord, :isLegacy, :extensionListUrl) ON CONFLICT(index_url) DO UPDATE SET name = :name, badge_label = :badgeLabel, signing_key =: signingKey, contact_website = :contactWebsite, contact_discord = :contactDiscord, is_legacy = :isLegacy, extension_list_url = :extensionListUrl WHERE index_url = :indexUrl;`
- `delete`: `DELETE FROM extension_store WHERE index_url = :indexUrl;`

Mapping: `AnimeExtensionStoreRepositoryImpl.kt:106-126`. Note: `is_legacy` is `INTEGER AS Boolean` (0/1).

### 3.3 `custom_buttons` (anime DB) — `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`
```sql
import kotlin.Boolean;

CREATE TABLE custom_buttons (
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    isFavorite INTEGER AS Boolean NOT NULL,
    sortIndex INTEGER NOT NULL,
    content TEXT NOT NULL,
    longPressContent TEXT NOT NULL,
    onStartup TEXT NOT NULL
);
```
Seed (exact, `INSERT OR IGNORE`):
- `_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`.
- `content` (Lua):
```lua
local intro_length = mp.get_property_number("user-data/current-anime/intro-length")
aniyomi.right_seek_by(intro_length)
```
- `longPressContent` (Lua, one line):
```lua
aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")
```
- `onStartup` (Lua, exact text as in the .sq file):
```lua
function update_button(_, length)
  if length ~= nil then
    if length == 0 then
	  aniyomi.hide_button()
	  return
	else
	  aniyomi.show_button()
	end
    aniyomi.set_button_title("+" .. length .. " s")
  end
end

if $isPrimary then
  mp.observe_property("user-data/current-anime/intro-length", "number", update_button)
end
```
(Indentation inside the SQL literal uses tabs on two lines; reproduce as written in the .sq file.) Placeholders `$isPrimary` and `$id` are substituted by `CustomButton.getButton*` before the script is run (behavior section 8.1).

Queries (exact):
- `findAll`: `SELECT * FROM custom_buttons ORDER BY sortIndex;`
- `insert`: `INSERT INTO custom_buttons(name, isFavorite, sortIndex, content, longPressContent, onStartup) VALUES (:name, :isFavorite, :sortIndex, :content, :longPressContent, :onStartup);`
- `delete`: `DELETE FROM custom_buttons WHERE _id = :customButtonId;`
- `update`: `UPDATE custom_buttons SET name = coalesce(:name, name), isFavorite = coalesce(:isFavorite, isFavorite), sortIndex = coalesce(:sortIndex, sortIndex), content = coalesce(:content, content), longPressContent = coalesce(:longPressContent, longPressContent), onStartup = coalesce(:onStartup, onStartup) WHERE _id = :customButtonId;`
- `selectLastInsertedRowId`: `SELECT last_insert_rowid();`

Mapping: `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt:67-84`. Insert always writes `isFavorite = false` (line 31). Batch update wraps every `updatePartialBlocking` in one transaction (line 44-48).

### 3.4 `manga_sync` (manga DB) — `data/src/main/sqldelight/data/manga_sync.sq`
```sql
CREATE TABLE manga_sync(
    _id INTEGER NOT NULL PRIMARY KEY,
    manga_id INTEGER NOT NULL,
    sync_id INTEGER NOT NULL,
    remote_id INTEGER NOT NULL,
    library_id INTEGER,
    title TEXT NOT NULL,
    last_chapter_read REAL NOT NULL,
    total_chapters INTEGER NOT NULL,
    status INTEGER NOT NULL,
    score REAL NOT NULL,
    remote_url TEXT NOT NULL,
    start_date INTEGER NOT NULL,
    finish_date INTEGER NOT NULL,
    private INTEGER AS Boolean DEFAULT 0 NOT NULL,
    UNIQUE (manga_id, sync_id) ON CONFLICT REPLACE,
    FOREIGN KEY(manga_id) REFERENCES mangas (_id)
    ON DELETE CASCADE
);
```
Queries used by scope: `getTrackById` (`WHERE _id = :id`), `getTracksByMangaId` (`WHERE manga_id = :mangaId`), `getTracks` (`SELECT * FROM manga_sync;`), `insert` (13 columns, all named parameters), `delete` (`DELETE FROM manga_sync WHERE manga_id = :mangaId AND sync_id = :syncId;`). Also `update` (13 `coalesce` columns, `WHERE _id = :id`), not used in scope.
Constraint effect: `ON CONFLICT REPLACE` on the unique pair (Q-TRK-2).

### 3.5 `anime_sync` (anime DB) — `data/src/main/sqldelightanime/dataanime/anime_sync.sq`
Same layout with `anime_id`, `last_episode_seen REAL`, `total_episodes INTEGER`, `UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE`, `FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE`. Queries: `getAnimeTracks`, `getTrackByAnimeId` (`WHERE _id = :id`), `getTracksByAnimeId`, `insert`, `delete` (`WHERE anime_id = :animeId AND sync_id = :syncId`), `update`.

### 3.6 `animesources` / `mangas` / `chapters` — referenced only
Non-library listings read entries grouped by source (area 05 owns the exact query). The domain contract is: count rows where `favorite = 0` (non-library) grouped by `source` id.

## 4. Preferences (keys, types, defaults)

Preference store API: `PreferenceStore.getBoolean/getInt/getLong/getString/getStringSet` (area 02). Keys beginning with `__APP_STATE_` come from `Preference.appStateKey(key)` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:43-47`, prefix constant `APP_STATE_PREFIX = "__APP_STATE_"` at line 47). The prefix marks them as app state (Preference.kt:41 checks it); how backup treats them is area 21's concern.

### 4.1 Download (`domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt`)
| Accessor (line) | Key (exact) | Type | Default |
|---|---|---|---|
| `downloadOnlyOverWifi` (9-12) | `pref_download_only_over_wifi_key` | Boolean | `true` |
| `useExternalDownloader` (14) | `use_external_downloader` | Boolean | `false` |
| `externalDownloaderSelection` (16-19) | `external_downloader_selection` | String | `""` |
| `saveChaptersAsCBZ` (21) | `save_chapter_as_cbz` | Boolean | `true` |
| `splitTallImages` (23) | `split_tall_images` | Boolean | `true` |
| `autoDownloadWhileReading` (25) | `auto_download_while_reading` | Int | `0` |
| `autoDownloadWhileWatching` (26) | `auto_download_while_watching` | Int | `0` |
| `removeAfterReadSlots` (28) | `remove_after_read_slots` | Int | `-1` |
| `removeAfterMarkedAsRead` (30-33) | `pref_remove_after_marked_as_read_key` | Boolean | `false` |
| `removeBookmarkedChapters` (35) | `pref_remove_bookmarked` | Boolean | `false` |
| `downloadFillermarkedItems` (37) | `pref_download_fillermarked` | Boolean | `false` |
| `removeExcludeCategories` (39-42) | `remove_exclude_categories` | StringSet | `{}` |
| `removeExcludeAnimeCategories` (43-46) | `remove_exclude_anime_categories` | StringSet | `{}` |
| `downloadNewChapters` (48) | `download_new` | Boolean | `false` |
| `downloadNewEpisodes` (49) | `download_new_episode` | Boolean | `false` |
| `downloadNewChapterCategories` (51-54) | `download_new_categories` | StringSet | `{}` |
| `downloadNewEpisodeCategories` (55-58) | `download_new_anime_categories` | StringSet | `{}` |
| `downloadNewChapterCategoriesExclude` (60-63) | `download_new_categories_exclude` | StringSet | `{}` |
| `downloadNewEpisodeCategoriesExclude` (64-67) | `download_new_anime_categories_exclude` | StringSet | `{}` |
| `numberOfDownloads` (69) | `download_slots` | Int | `1` |
| `downloadSpeedLimit` (70) | `download_speed_limit` | Int | `0` (KiB/s) |
| `downloadNewUnreadChaptersOnly` (72) | `download_new_unread_chapters_only` | Boolean | `false` |
| `downloadNewUnseenEpisodesOnly` (73) | `download_new_unread_episodes_only` | Boolean | `false` |

`categoryPreferenceKeys` (companion, lines 83-91) = the six StringSet keys: `remove_exclude_categories`, `remove_exclude_anime_categories`, `download_new_categories`, `download_new_anime_categories`, `download_new_categories_exclude`, `download_new_anime_categories_exclude`. Consumer: `CategoryDeleteUseCase` / category delete flow (area 08) strips the deleted category id from these sets.

Note: the accessor count is 23; the set of keys is 23 (the 6 category keys are part of those 23).

Sets are stored as decimal category-id strings (`toLong()` on read). Consumer of `categoryPreferenceKeys` on category delete is `DeleteMangaCategory` / `DeleteAnimeCategory` (area 08).

### 4.2 Backup (`domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt`)
| Accessor (line) | Key | Type | Default |
|---|---|---|---|
| `backupInterval` (10) | `backup_interval` | Int | `12` |
| `lastAutoBackupTimestamp` (12) | `__APP_STATE_last_auto_backup_timestamp` | Long | `0` |
| `backupFlags` (14-17) | `backup_flags` | StringSet | `{"1","2","4","8"}` |

### 4.3 Storage (`domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt:12`)
| Accessor | Key | Type | Default |
|---|---|---|---|
| `baseStorageDirectory` | `__APP_STATE_storage_dir` | String (URI or path) | `folderProvider.path()` |

### 4.4 Release (`domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt:15-17`)
| Key | Type | Default |
|---|---|---|
| `__APP_STATE_last_app_check` | Long (epoch ms) | `0` |

## 5. Files and folders on disk

### 5.1 Base storage tree (created by `StorageManager`, see behavior 9)
Relative to the base directory (Android default from `AndroidStorageFolderProvider`; Kurobayomi default `$XDG_DATA_HOME/kurobayomi`):
```
<base>/
  autobackup/                  # automatic backups (BackupCreateJob)
  downloads/                   # downloaded chapters/episodes (NoMedia marker on Android only)
  local/                       # local manga source folders (LocalMangaSource)
  localanime/                  # local anime source folders
  mpv-config/
    fonts/
    scripts/
    script-opts/
    shaders/
```
Folder names are exact and case-sensitive. Proposal: Kurobayomi keeps these folder names so that a folder copied from an Aniyomi install works unchanged.

### 5.2 Other persisted files
- None written directly by scope files. Backups (area 21) and downloads (area 23/24) write into the folders above.

## 6. Network endpoints

| # | Method | URL pattern | Client | Request headers / auth | Response | Used by |
|---|---|---|---|---|---|---|
| N1 | GET | `<baseUrl>/repo.json` where `baseUrl` = index URL minus `/index.min.json` | `NetworkHelper.client` (OkHttp) via `awaitSuccess()` | none set in scope (default client headers) | JSON `{ "meta": { name, shortName, website, signingKeyFingerprint } }` → `ExtensionRepoMetaDto` | `ExtensionRepoService.fetchRepoDetails` (ExtensionRepoService.kt:19-34) |
| N2 | GET | `<indexUrl>` (user URL, must match `^https://.*/index\.min\.json$` for manga repos) | same | none | manga repo index list (area 18) | `CreateMangaExtensionRepo` (indirect) |
| N3 | GET | anime store `indexUrl` | same | none | JSON `{` → `NetworkAnimeExtensionStore` (current); `[` → legacy list (requires `/index.min.json` suffix, rewritten to `/repo.json`); `{` decode failure → current; other first bytes → protobuf | `AnimeExtensionStoreService.fetch` (AnimeExtensionStoreService.kt:28-68) |
| N4 | GET | legacy repo: `<indexUrl with /index.min.json replaced by /repo.json>` | same | none | `NetworkLegacyAnimeExtensionRepo`; `indexV2` (optional URL) → recursive fetch | same |
| N5 | GET | `store.extensionListUrl` when non-null | same | none | JSON `{` or protobuf → `NetworkAnimeExtensionStore.ExtensionList` | `AnimeExtensionStoreService.getExtensions` (a) |
| N6 | GET | `store.indexUrl` for non-legacy stores without extensionListUrl | same | none | JSON or protobuf `NetworkAnimeExtensionStore` (`extensionList` must be non-null) | `getExtensions` (b) |
| N7 | GET | `<storeBaseUrl>/index.min.json` for legacy stores (`storeBaseUrl` = indexUrl minus `/repo.json`) | same | none | JSON list `[NetworkLegacyAnimeExtension]` | `getExtensions` (c) |
| N8 | GET | `https://api.github.com/repos/${repository}/releases/latest` | same | GitHub API defaults (no token; unauthenticated rate limit 60/h per IP) | `GithubRelease` JSON | `ReleaseServiceImpl.latest` (ReleaseServiceImpl.kt:18-24) |
| N9 | GET | `browser_download_url` of the chosen asset | not called by scope | — | — | only displayed/linked |

Gzip: every fetch in N1–N7 accepts gzip bodies; the first two bytes `0x1f 0x8b` trigger gunzip (AnimeExtensionStoreService.kt:111-120). Kurobayomi must do the same (`flate2`).
Encoding: protobuf bodies are decoded with `ProtoBuf` from kotlinx.serialization (`protoBuf.decodeFromByteArray`). The protobuf message definitions live in area 19 (outside this scope). Proposal for v1: JSON-only extension stores, protobuf stores rejected with a clear error until area 19 supplies the schema (see `p1-rust-design.md` section 6).

Timeouts and retries: from `NetworkHelper.client` (area 12); none added in scope.

## 7. Wire and JSON field names (summary)

- `repo.json` (N1): `meta.name`, `meta.shortName` (nullable), `meta.website`, `meta.signingKeyFingerprint`.
- GitHub release (N8): `tag_name`, `body`, `html_url`, `assets[].name`, `assets[].browser_download_url`.
- Extension store (N3/N5/N6/N7): fields of `NetworkAnimeExtensionStore`, `NetworkLegacyAnimeExtensionRepo`, `NetworkLegacyAnimeExtension` are defined in `data/src/main/java/mihon/data/extension/anime/model/` (area 19); not restated because they are outside this scope.

## 8. Constants (single source)

| Constant | Value | File:line |
|---|---|---|
| `DEFAULT_CATEGORY_ID` | `0L` | FilterChaptersForDownload.kt:70; FilterEpisodesForDownload.kt:70 |
| `UNCATEGORIZED_ID` | `0L` | domain/src/main/java/tachiyomi/domain/category/model/Category.kt:16 |
| `QUERY_POPULAR` (manga) | `"eu.kanade.domain.source.manga.interactor.POPULAR"` | GetRemoteManga.kt:20 |
| `QUERY_LATEST` (manga) | `"eu.kanade.domain.source.manga.interactor.LATEST"` | GetRemoteManga.kt:21 |
| `QUERY_POPULAR` (anime) | `"eu.kanade.domain.source.anime.interactor.POPULAR"` | GetRemoteAnime.kt:20 |
| `QUERY_LATEST` (anime) | `"eu.kanade.domain.source.anime.interactor.LATEST"` | GetRemoteAnime.kt:21 |
| Repo URL regex | `^https://.*/index\.min\.json$` | CreateMangaExtensionRepo.kt:15 |
| GitHub mention regex | `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))` (IGNORE_CASE) | data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:58-59 |
| Release ABI list | `arm64-v8a`, `armeabi-v7a`, `x86_64`, `x86` | ReleaseServiceImpl.kt:46 |
| Version strip regex | `[^\d.]` | GetApplicationRelease.kt:55, 63 |
| Release throttle | 3 days | GetApplicationRelease.kt:20-25 |
| Fingerprint placeholder prefix | `NOFINGERPRINT` | UpdateMangaExtensionRepo.kt:24 |
| Extension store placeholder | signing key `NO_SIGNING_KEY`; `isLegacy = false`; `extensionListUrl = null` | AnimeExtensionStoreRepositoryImpl.kt:29-33 |
| Legacy store path rewrite | `/index.min.json` → `/repo.json` | AnimeExtensionStoreService.kt:36, 39 |
| Legacy extension list path | `"$storeBaseUrl/index.min.json"` | AnimeExtensionStoreService.kt:97 |
| Gzip magic | `0x1f8b` | AnimeExtensionStoreService.kt:114 |
| Download speed multiplier | `* 1024` | MangaDownloader.kt:525 |
| Download slots UI | `1..5` | SettingsDownloadScreen.kt:102-104 |
| Download ahead UI | `0, 2, 3, 5, 10` | SettingsDownloadScreen.kt:319-340 |
| Remove-after-read UI | `-1, 0, 1, 2, 3, 4` | SettingsDownloadScreen.kt:139-147 |
| Backup interval default | `12` | BackupPreferences.kt:10 |
| Backup flags default | `"1","2","4","8"` | BackupPreferences.kt:16 |
| Upcoming anime statuses | `1, 4, 7` | GetUpcomingAnime.kt:13-15 |
| Upcoming manga statuses | `1, 4` | GetUpcomingManga.kt:13-14 |
| Pin codes | `0b00`, `0b01`, `0b10` | source/*/model/Pin.kt:4-6 |
| Seed custom button id | `1` | custom_buttons.sq:14-15 |

## 9. Data-shape invariants the Rust port must keep

1. `extension_repos.signing_key_fingerprint` is UNIQUE; `base_url` is the PK.
2. `extension_store.index_url` is the PK; no uniqueness on any other column.
3. `custom_buttons.sortIndex` is not unique in SQL; the interactors maintain it as a dense 0..n-1 sequence after delete/reorder; creation computes max+1.
4. `manga_sync` / `anime_sync`: UNIQUE(entry_id, tracker_id); insert replaces.
5. Foreign keys with `ON DELETE CASCADE` from tracks to entries (area 06/04). Deleting an entry deletes its tracks.
6. `Pins` in memory is a 2-bit value; only `0b00` and `0b11` are persisted by app code.
