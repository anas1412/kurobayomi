# DATA-COMPAT: compatibility with existing Aniyomi data

Status: plan, not implemented. Synthesis of the area documents in this folder, checked against the Kotlin source at commit `97414446b8a95994c72dd33c41c971a89d4d25b8` (committed 2026-09-14) in `/home/blackbox/aniyomi-src`. Every path below is repo-relative to that clone unless it starts with `plans/` or `/home/blackbox/kurobayomi`.

Labels: **OBSERVED** = what the Kotlin does (checked in this pass unless a task or area link is given). **PROPOSAL** = what Kurobayomi does. Area links: `area:NN-...` maps to `NN-.../00-overview.md` in this folder.

---

## 0. Summary of decisions

| # | Decision | Reason | Detail |
|---|---|---|---|
| C1 | Kurobayomi reads and writes `.tachibk` (gzip + protobuf) with the same field numbers as Aniyomi. This is the only interchange path in v1. | Backups are the one format Aniyomi itself restores across devices and Mihon-shaped files. | Section 4 |
| C2 | Raw Aniyomi database files (`tachiyomi.db`, `tachiyomi.animedb`) are imported only as a P2 path, and only after the user says yes (open question Q-DB1). | The anime chain is verified only on a reconstructed v113 baseline. | Section 3 |
| C3 | Source ids are the Kotlin `generateId` values, bit for bit (MD5 of `name.lowercase()/lang/versionId`, first 8 bytes big-endian, AND `Long.MAX_VALUE`). | Every library row, chapter row, history row and backup link stores this id. | Section 5 |
| C4 | Preference keys keep the Aniyomi spelling. Only keys that are explicitly set are written. Keys with the `__APP_STATE_` prefix are never exported. Keys with the `__PRIVATE_` prefix are exported only when the user turns on private settings. | Backups copy straight across. | Section 6 |
| C5 | Download folders, the local source folders and the cover cache use the Aniyomi layout (names built with `DiskUtil.buildValidFilename`, MD5 cover names). | Lets a user copy an Android download folder over and keep it. | Section 7 |
| C6 | Restore runs sequential sections (not Kotlin's racing coroutines), with the PROPOSAL fixes listed in [area:22-backup-restore](22-backup-restore/00-overview.md) section 4.1. | Parity with the migrating user, without the known data-loss defects. | Section 10 |
| C7 | Backup export keeps the Aniyomi wire format. Kurobayomi-only data goes into keys with the `kuro_` prefix (Q-G in [area:02](02-preferences-system/00-overview.md)). | Aniyomi ignores unknown keys. | Sections 4.9, 6 |

---

## 1. Inventory of Aniyomi data on disk

### 1.1 OBSERVED stores

| Store | Kotlin source of truth | File or folder | Schema or format | Kurobayomi counterpart |
|---|---|---|---|---|
| Manga database | `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:78-101` (`AndroidSqliteDriver`, `name = "tachiyomi.db"`) | `tachiyomi.db` (Android app data dir) | SQLDelight `Database.Schema`, `PRAGMA user_version = 33` for the current schema | `manga.db` ([area:03](03-db-manga-schema-migrations/00-overview.md)) |
| Anime database | `app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:103-119` (`name = "tachiyomi.animedb"`) | `tachiyomi.animedb` | `AnimeDatabase.Schema`, `user_version = 139` | `anime.db` under `<data_dir>` ([area:04](04-db-anime-schema-migrations/00-overview.md), Q8) |
| Both databases' pragmas | `AppModule.kt` `onOpen` | n/a | `PRAGMA foreign_keys = ON`, `PRAGMA journal_mode = WAL`, `PRAGMA synchronous = NORMAL` | Same three pragmas, set on every open ([area:03](03-db-manga-schema-migrations/00-overview.md)) |
| App preferences | `core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt:18` (`PreferenceManager.getDefaultSharedPreferences(context)`) | Android default shared-preferences XML | key/value | Kurobayomi preference store ([area:02](02-preferences-system/00-overview.md)); no file import, only backups carry preferences |
| Scheduler and download state | `getSharedPreferences("tracking_queue")` (`app/src/main/java/eu/kanade/tachiyomi/data/track/`-side stores), `"active_downloads"` (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadStore.kt:32`), the pending-deleter prefs (`AnimeDownloadPendingDeleter.kt:26`) | Android shared prefs | transient queues | Not imported. Queue replacement is owned by [area:25](25-downloads-shared/00-overview.md) |
| Storage base directory | `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:38-48` | user-chosen base directory | folders below | Storage settings ([area:13](13-core-util-storage-archive-torrent/00-overview.md)) |
| Automatic backups | `StorageManager.kt:98` `private const val AUTOMATIC_BACKUPS_PATH = "autobackup"` | `<base>/autobackup/` | `.tachibk` files | [area:21](21-backup-format-and-create/00-overview.md) section 5 |
| Downloads | `StorageManager.kt:99` `DOWNLOADS_PATH = "downloads"` | `<base>/downloads/` | see section 7 | [area:23](23-downloads-manga/00-overview.md), [area:24](24-downloads-anime/00-overview.md) |
| Local manga source | `StorageManager.kt:100` `LOCAL_SOURCE_PATH = "local"` | `<base>/local/` | folders and archives | [area:14](14-source-local-and-metadata/00-overview.md) |
| Local anime source | `StorageManager.kt:101` `LOCAL_ANIMESOURCE_PATH = "localanime"` | `<base>/localanime/` | folders and video files | [area:14](14-source-local-and-metadata/00-overview.md) |
| MPV config | `StorageManager.kt:102-108` `mpv-config`, `fonts`, `scripts`, `script-opts`, `shaders` | `<base>/mpv-config/...` | mpv files | [area:45](45-player-core/00-overview.md), [area:48](48-settings-player-and-scripts/00-overview.md) |
| Manga covers | `app/src/main/java/eu/kanade/tachiyomi/data/cache/MangaCoverCache.kt:21-22` `COVERS_DIR = "covers"`, `CUSTOM_COVERS_DIR = "covers/custom"` | `getExternalFilesDir("covers")` or `filesDir/covers` | image files | Section 7.3 |
| Anime covers | `app/src/main/java/eu/kanade/tachiyomi/data/cache/AnimeCoverCache.kt:22` `CUSTOM_COVERS_DIR = "animecovers/custom"`; `COVERS_DIR` is the anime twin | `animecovers/`, `animecovers/custom/` | image files | Section 7.3 |

### 1.2 Mapping of table names (OBSERVED)

The manga `.sq` files are in `data/src/main/sqldelight/data/` (9 files). The anime `.sq` files are in `data/src/main/sqldelightanime/dataanime/` (10 tables, listed by file name).

| Concept | Manga table (`tachiyomi.db`) | Anime table (`tachiyomi.animedb`) | Shared |
|---|---|---|---|
| Entries | `mangas` | `animes` | |
| Child items | `chapters` | `episodes` | |
| Read or watch history | `history` | `animehistory` | |
| Library to category links | `mangas_categories` | `animes_categories` | |
| Categories | `categories` | `categories` | Same name in both databases |
| Tracker bindings | `manga_sync` | `anime_sync` | |
| Sources | `sources` | `animesources` | |
| Extension repositories or stores | `extension_repos` | `extension_store` | |
| Excluded scanlators | `excluded_scanlators` | none | |
| Seasons (parent or child links) | none | `anime_relations` | |
| Custom buttons | none | `custom_buttons` | |
| Views | `libraryView`, `historyView`, `updatesView` (`data/src/main/sqldelight/view/`) | `animelibView`, `animehistoryView`, `animeupdatesView`, `animeseasonsView`, `animeseasonstatsView`, `animehistorystatsView`, `episodestatsView`, `animedeletableView` (`data/src/main/sqldelightanime/view/`) | |

The `categories` table is shared by name. Its row shape is `categories(_id, name, sort, flags, hidden)` ([area:05](05-data-repos-entries-items/00-overview.md), `p1-topic-categories.md` section 1).

---

## 2. Schema versions and the version guard

### 2.1 OBSERVED

- Manga: migrations are `data/src/main/sqldelight/migrations/1.sqm` through `32.sqm` (latest `32.sqm`). File `N.sqm` upgrades `PRAGMA user_version` from `N` to `N+1`. Current schema version is `33`.
- Anime: migrations are `data/src/main/sqldelightanime/migrations/113.sqm` through `138.sqm` (26 files). Current schema version is `139`. The chain starts at `113`, the first migration file present in the source ([area:04](04-db-anime-schema-migrations/00-overview.md) section 1).

### 2.2 Guard rules for importing a raw database (PROPOSAL, from [area:03](03-db-manga-schema-migrations/p1-topic-migration-history.md) section 5 and [area:04](04-db-anime-schema-migrations/p1-topic-legacy-migration-chain.md))

Manga (`tachiyomi.db`):

1. Open read-only. Read `PRAGMA user_version = v`.
2. `v > 33`: refuse with "This database comes from a newer Aniyomi version". Mihon databases above 33 are refused the same way.
3. `v < 14`: refuse with "Database too old; open it once in Aniyomi/Mihon first". Versions 1 to 13 are untested because the version-1 DDL is not available.
4. `14 <= v <= 33`: run `v.sql` through `32.sql` on a copy, then copy rows into a new Kurobayomi file (section 3.2).

Anime (`tachiyomi.animedb`), [area:04](04-db-anime-schema-migrations/p1-topic-legacy-migration-chain.md) line 69:

- `v < 113`: `Err(TooOldForImport)`.
- `113 <= v <= 139`: run `v.sqm` through `138.sqm` on the copy, then import.
- `v > 139`: refuse as newer.

Both guards exist only in the importer. Kurobayomi's own files are refused by any other tool when they are newer than the app (see 3.1).

### 2.3 Kurobayomi file identity (PROPOSAL, [area:03](03-db-manga-schema-migrations/00-overview.md) D3)

- Kurobayomi manga file: `PRAGMA application_id = 0x4B555230` ("KUR0"). Aniyomi files have `application_id = 0`.
- An Aniyomi file at any version has `application_id = 0`, so Kurobayomi refuses to write to it and routes it to the importer. This prevents an Aniyomi file with `user_version = 33` from being opened as a Kurobayomi file.
- The anime file gets the same check ([area:04](04-db-anime-schema-migrations/00-overview.md) section 1, D1).

---

## 3. Database compatibility

### 3.1 Compatibility rule

Kurobayomi's fresh schema is the Aniyomi shape at the same version (manga 33, anime 139). Same table names, column names, column order and trigger bodies. The only additions are native steps that Aniyomi never runs:

- Manga: `34.sql` (`chapters_url_index`, favourites-first `libraryView`). Not in the first release: Q6 of [area:03](03-db-manga-schema-migrations/00-overview.md) (measured 246 ms to 34 ms on a 300000-chapter database). Task in P2.
- Anime: native step `139 -> 140` (two category-link indexes, T-04a-036, P2).

Both keep the Aniyomi version numbers, so a row copied from Aniyomi needs no value conversion.

### 3.2 Import of a raw `tachiyomi.db` (P2, gated by Q-DB1)

The algorithm is in [area:03](03-db-manga-schema-migrations/p1-topic-migration-history.md) section 5 (`T-03a-023`). Summary of the observed steps and the proposed ones:

1. Guard (section 2.2).
2. Copy the file together with `-wal` and `-shm` siblings into a temp directory, open the copy read-write, set `PRAGMA foreign_keys = OFF`.
3. Run migrations `v..=32` one at a time, each inside its own `BEGIN IMMEDIATE` and `COMMIT`. After migration 15 also run `DROP TABLE IF EXISTS history_temp;`.
4. Run `PRAGMA foreign_key_check` and delete orphan rows in dependency order, counting each delete:
   - `DELETE FROM chapters WHERE manga_id NOT IN (SELECT _id FROM mangas)`
   - `DELETE FROM history WHERE chapter_id NOT IN (SELECT _id FROM chapters)`
   - `DELETE FROM manga_sync WHERE manga_id NOT IN (SELECT _id FROM mangas)`
   - `DELETE FROM mangas_categories WHERE manga_id NOT IN (SELECT _id FROM mangas) OR category_id NOT IN (SELECT _id FROM categories)`
   - `DELETE FROM excluded_scanlators WHERE manga_id NOT IN (SELECT _id FROM mangas)`
5. Build the target file from the baseline script in two halves: half A (tables, named indexes, seed row `categories(0, '', -1, 0, 0)`), half B (7 triggers, 3 views). Never mutate the live file.
6. `ATTACH DATABASE '<migrated copy>' AS src`. Copy in this order inside one transaction: `categories` (skip `_id = 0`), `mangas`, `chapters`, `mangas_categories`, `history`, `manga_sync`, `excluded_scanlators`, `sources`, `extension_repos`. Use explicit column lists. `history.last_read` is written as `coalesce(last_read, 0)`. Half B runs after the copy, so the triggers do not bump `version` or `last_modified_at` during import.
7. `PRAGMA user_version = 33`, `ANALYZE`.
8. Report: rows per table, orphan rows dropped, source version, and whether quirks applied (migration 10 shared `date_added`, `history_temp`).
9. Swap: if the live `manga.db` has at least one row in `mangas`, ask for confirmation. On yes, rename the live file to `manga.db.bak-<UTC yyyyMMddHHmmss>` and never delete it, then move the new file into place.

Known Aniyomi defects that the import keeps unchanged (data is not corrected silently): migration 10 gives every manga one shared `date_added` ([area:03](03-db-manga-schema-migrations/00-overview.md) I-6). The importer reports it.

### 3.3 Import of a raw `tachiyomi.animedb` (P2, gated by Q-DB2)

[area:04](04-db-anime-schema-migrations/00-overview.md) recommends deferring it. The chain was tested only on a reconstructed v113 baseline because no real v113 file was available. Until a real file is a fixture, the only supported anime import is the backup path (section 10). Task: T-04a-025 (P2). Quarantine on a failed `quick_check` is T-04a-031 (P1): the damaged file is moved aside under a new name and never deleted.

### 3.4 Column semantics that the importer must keep

- Seconds and milliseconds are mixed. `mangas.last_modified_at` and `favorite_modified_at` are in seconds; other timestamps are in milliseconds ([area:03](03-db-manga-schema-migrations/00-overview.md) Q3). Helper names `secs` and `millis` must be used in code.
- Coalesce updates cannot clear a column to NULL (for example `description`, `author`, `genre`, `scanlator`) ([area:03](03-db-manga-schema-migrations/00-overview.md) Q4). The Rust API uses `Option` patch fields so NULL cannot be written by accident.
- `genre` is stored as text joined with `", "`. A genre that contains `", "` splits on read. Kept for backup interchange ([area:04](04-db-anime-schema-migrations/00-overview.md) Q3 and [area:03](03-db-manga-schema-migrations/00-overview.md)).
- `memo` (opaque JSON per entry and per episode) is kept as bytes. Default `JsonObjectEmptyBytes` is the two bytes `{}` (`core/common/src/main/java/mihon/core/common/extensions/JsonObject.kt:7`).

### 3.5 Drop and fix decisions for database behaviour

| Item | Aniyomi | Kurobayomi | Source |
|---|---|---|---|
| Category delete renumbers all categories | renumbers system category too | renumbers only user categories | [area:03](03-db-manga-schema-migrations/00-overview.md) D8, Q7 |
| `%` and `_` in history search | not escaped | escaped (`ESCAPE '\'`) | D8, Q11 |
| Clear database | not in one transaction | one transaction | D8, Q17 |
| Unfiltered `DELETE FROM animehistory` | the update hook may not fire under SQLite's truncate optimisation, so a notification can be missed | Kurobayomi publishes the table change explicitly after the statement | [area:04](04-db-anime-schema-migrations/00-overview.md) section 4.3 item 1 |

---

## 4. Backup format (`.tachibk`)

### 4.1 Container (OBSERVED)

- Encoding: protobuf via `kotlinx.serialization.protobuf.ProtoBuf` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:190`, `AppModule.kt` registers it as a singleton).
- Compression: the encoded bytes are written through `okio.gzip()` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:131-142`). Gzip magic `0x1f 0x8b`.
- Mihon files use the same container shape for the manga part (fields 1 to 106). This pass did not decode a Mihon file, so the Mihon-specific fields are unverified (T-21a-043 covers the reader).
- Kotlin ProtoBuf rules that a Rust codec must follow: `Int` and `Long` are varints (not zigzag, `ProtoIntegerType.DEFAULT`); `Boolean` is a varint; `Float` is fixed32; `Double` is fixed64; `String` and `ByteArray` are length-delimited; `List` of any element type is written unpacked (no `@ProtoPacked` appears in `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/`; checked). Enums are written as their ordinal (varint). Nullable fields are omitted when null.
- The sealed `PreferenceValue` encoding (`BackupPreference.kt:19-37`) uses kotlinx polymorphic encoding. Its tag numbers are NOT in the source text. They must come from a real `.tachibk` fixture (T-21a-034, T-22a-051). Guessing corrupts backups ([area:21](21-backup-format-and-create/00-overview.md) D10).

### 4.2 Sniffing and legacy detection (OBSERVED)

`app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt`:

1. Peek 2 bytes. `0x1f8b` → gunzip the stream. Otherwise use the raw stream.
2. If the first two bytes are `0x7b7d` (`{}`), `0x7b22` (`{"`) or `0x7b0a` (`{\n`), throw `IOException` with string `invalid_backup_file_json` = "JSON backup not supported".
3. `BackupDetector.isLegacyBackup(bytes)` (`BackupDetector.kt`): decode only fields 103 (`backupAnimeSources`, list of `DetectAnimeSource{name=1, sourceId=2}`) and 500 (`isLegacy: Boolean = true`). Return `isLegacy && backupAnimeSources.isNotEmpty()`. A decode failure returns false.
4. If legacy: decode `LegacyBackup` and convert with `toBackup()` (`Backup.kt:7-44`). The converted object sets `isLegacy = false`.
5. Else decode `Backup`. `SerializationException` becomes `IOException` with `invalid_backup_file_unknown` = "Backup file is corrupted".

Rules for Kurobayomi (PROPOSAL, consistent with [area:22](22-backup-restore/00-overview.md) section 4.2):

- Files shorter than 2 bytes: "Backup file is corrupted" (Aniyomi throws a raw `EOFException`).
- Plain text that is not `{`-prefixed and not protobuf: reject as corrupted. Aniyomi decodes it to an empty backup without error ([area:22](22-backup-restore/00-overview.md) decision 5).
- A gzip stream that fails to gunzip: "Backup file is corrupted".

### 4.3 `Backup` container fields (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/Backup.kt:45-62`)

| Field | Proto number | Kotlin type | Default | Notes |
|---|---|---|---|---|
| `backupManga` | 1 | `List<BackupManga>` | empty | |
| `backupCategories` | 2 | `List<BackupCategory>` | empty | manga categories |
| `backupSources` | 101 | `List<BackupSource>` | empty | manga source names |
| `backupPreferences` | 104 | `List<BackupPreference>` | empty | app preferences |
| `backupSourcePreferences` | 105 | `List<BackupSourcePreferences>` | empty | per-source prefs |
| `backupMangaExtensionRepo` | 106 | `List<BackupExtensionRepos>` | empty | |
| `isLegacy` | 500 | `Boolean` | `true` | written `false` by `BackupCreator.kt:122`. Only for detection |
| `backupAnime` | 501 | `List<BackupAnime>` | empty | |
| `backupAnimeCategories` | 502 | `List<BackupCategory>` | empty | |
| `backupAnimeSources` | 503 | `List<BackupAnimeSource>` | empty | |
| `backupExtensions` | 504 | `List<BackupExtension>` | empty | APK bytes, see 4.10 |
| `backupAnimeExtensionStores` | 505 | `List<BackupExtensionStore>` | empty | |
| `backupCustomButton` | 506 | `List<BackupCustomButtons>` | empty | anime custom buttons |

Proto 100 (`backupBrokenSources`) is commented out in both containers (`Backup.kt`, lines 13 and 48): it is not a declared field, so it is not written and not read. The legacy type `BrokenBackupSource` (field 0 name, field 1 sourceId, `models/BackupSource.kt:12-15`) is also unused on the read path.

`LegacyBackup` (`Backup.kt:7-44`) uses: 1 manga, 2 manga categories, 3 anime, 4 anime categories, 101 sources, 103 anime sources, 104 preferences, 105 source preferences, 106 extensions, 107 anime extension stores, 108 manga extension repos, 109 custom buttons. Its `toBackup()` maps them to the 500+ fields above.

### 4.4 `BackupManga` (OBSERVED, `models/BackupManga.kt:10-42`)

| Field | No. | Type | Default | Notes |
|---|---|---|---|---|
| `source` | 1 | `Long` | required | source id (section 5) |
| `url` | 2 | `String` | required | entry url without domain (section 5.3) |
| `title` | 3 | `String` | `""` | |
| `artist` | 4 | `String?` | null | |
| `author` | 5 | `String?` | null | |
| `description` | 6 | `String?` | null | |
| `genre` | 7 | `List<String>` | empty | |
| `status` | 8 | `Int` | 0 | manga status (section 4.11) |
| `thumbnailUrl` | 9 | `String?` | null | |
| (10, 11, 12) | none | | | 1.x fields, not used in 0.x. Ignore on read, do not write |
| `dateAdded` | 13 | `Long` | 0 | ms |
| `viewer` | 14 | `Int` | 0 | "Replaced by viewer_flags". Written as `viewerFlags & ReadingMode.MASK` (`MangaBackupCreator.kt:95`) |
| `chapters` | 16 | `List<BackupChapter>` | empty | |
| `categories` | 17 | `List<Long>` | empty | category ids of this backup (section 9.1) |
| `tracking` | 18 | `List<BackupTracking>` | empty | |
| (15) | none | | | 1.x, ignore |
| `favorite` | 100 | `Boolean` | **`true`** | absent field reads as favourite. Written by the creator as the library flag |
| `chapterFlags` | 101 | `Int` | 0 | chapter flags (area:07 layout) |
| (102) | none | | | legacy broken history, not read |
| `viewer_flags` | 103 | `Int?` | null | full viewer word. Read as `viewer_flags ?: viewer` (`BackupManga.kt:57`) |
| `history` | 104 | `List<BackupHistory>` | empty | |
| `updateStrategy` | 105 | `UpdateStrategy` | `ALWAYS_UPDATE` | enum ordinal (section 4.11) |
| `lastModifiedAt` | 106 | `Long` | 0 | seconds (see 3.4) |
| `favoriteModifiedAt` | 107 | `Long?` | null | seconds |
| `excludedScanlators` | 108 | `List<String>` | empty | |
| `version` | 109 | `Long` | 0 | sync version counter |

### 4.5 `BackupChapter` (OBSERVED, `models/BackupChapter.kt:7-24`)

| Field | No. | Type | Default |
|---|---|---|---|
| `url` | 1 | `String` | required |
| `name` | 2 | `String` | required |
| `scanlator` | 3 | `String?` | null |
| `read` | 4 | `Boolean` | false |
| `bookmark` | 5 | `Boolean` | false |
| `lastPageRead` | 6 | `Long` | 0 |
| `dateFetch` | 7 | `Long` | 0 |
| `dateUpload` | 8 | `Long` | 0 |
| `chapterNumber` | 9 | `Float` | 0F |
| `sourceOrder` | 10 | `Long` | 0 |
| `lastModifiedAt` | 11 | `Long` | 0 |
| `version` | 12 | `Long` | 0 |

### 4.6 `BackupAnime` (OBSERVED, `models/BackupAnime.kt:13-63`)

Same fields 1 to 9, 13, 16 (`episodes`), 17 (`categories`), 18 (`tracking`), 100 (`favorite`, default true), 101 (`episodeFlags`, Int, 0), 103 (`viewer_flags`, **Int, default 0**, not nullable), 104 (`history`), 105 (`updateStrategy`, `AnimeUpdateStrategy`, default `ALWAYS_UPDATE`), 106 (`lastModifiedAt`), 107 (`favoriteModifiedAt`), 109 (`version`), 112 (`memo`, ByteArray, default `{}`), plus anime-only fields:

| Field | No. | Type | Default | Notes |
|---|---|---|---|---|
| `backgroundUrl` | 500 | `String?` | null | |
| (501) | none | | | "Broken, do not use" |
| `parentId` | 502 | `Long?` | null | season → parent link |
| `id` | 503 | `Long?` | null | "Used to associate seasons with parents. Do not use for anything else." |
| `seasonFlags` | 504 | `Long` | 0 | |
| `seasonNumber` | 505 | `Double` | -1.0 | |
| `seasonSourceOrder` | 506 | `Long` | 0 | |
| `fetchType` | 507 | `FetchType` | `Episodes` | ordinal: `Seasons` = 0, `Episodes` = 1 (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt`) |

Note: `BackupAnime` uses `class`, not `data class`, but the wire shape is the same.

### 4.7 `BackupEpisode` (OBSERVED, `models/BackupEpisode.kt:10-37`)

| Field | No. | Type | Default |
|---|---|---|---|
| `url` | 1 | `String` | required |
| `name` | 2 | `String` | required |
| `scanlator` | 3 | `String?` | null |
| `seen` | 4 | `Boolean` | false |
| `bookmark` | 5 | `Boolean` | false |
| `lastSecondSeen` | 6 | `Long` | 0 |
| `dateFetch` | 7 | `Long` | 0 |
| `dateUpload` | 8 | `Long` | 0 |
| `episodeNumber` | 9 | `Float` | 0F |
| `sourceOrder` | 10 | `Long` | 0 |
| `lastModifiedAt` | 11 | `Long` | 0 |
| `version` | 12 | `Long` | 0 |
| `memo` | 13 | `ByteArray` | `{}` |
| `totalSeconds` | 16 | `Long` | 0 |
| `fillermark` | 501 | `Boolean` | false |
| `summary` | 502 | `String?` | null |
| `previewUrl` | 503 | `String?` | null |

Note the numbering gap: 16 comes after 13, and the anime-only extras start at 501.

### 4.8 History, tracking, categories, sources

| Message | Fields (no., type, default) |
|---|---|
| `BackupHistory` (manga, `models/BackupHistory.kt`) | 1 `url: String` (chapter url); 2 `lastRead: Long` (required, ms); 3 `readDuration: Long` = 0 |
| `BackupAnimeHistory` (`models/BackupAnimeHistory.kt`) | 1 `url` (episode url); 2 `lastRead: Long`; 3 `readDuration: Long` = 0 |
| `BackupTracking` (manga, `models/BackupTracking.kt`) | 1 `syncId: Int`; 2 `libraryId: Long`; 3 `mediaIdInt: Int` = 0, `@Deprecated` ("Use mediaId instead", line 14-15), kept for 1.x compatibility; 4 `trackingUrl: String` = ""; 5 `title: String` = ""; 6 `lastChapterRead: Float` = 0F; 7 `totalChapters: Int` = 0; 8 `score: Float` = 0F; 9 `status: Int` = 0; 10 `startedReadingDate: Long` = 0; 11 `finishedReadingDate: Long` = 0; 12 `private: Boolean` = false; 100 `mediaId: Long` = 0 |
| `BackupAnimeTracking` (`models/BackupAnimeTracking.kt`) | Same as manga, but 6 `lastEpisodeSeen: Float`, 7 `totalEpisodes: Int`, 10 `startedWatchingDate`, 11 `finishedWatchingDate` |
| `BackupCategory` (`models/BackupCategory.kt:8-14`) | 1 `name: String` (required); 2 `order: Long` = 0; 3 `id: Long` = 0; 100 `flags: Long` = 0. Note the comment at line 12: field 3 was `updateInterval: Int` in 1.x and is not used in 0.x, but the same number is reused for `id` |
| `BackupSource` (manga, `models/BackupSource.kt:7-10`) | 1 `name: String` = ""; 2 `sourceId: Long` (required) |
| `BackupAnimeSource` (`models/BackupAnimeSource.kt:7-10`) | 1 `name: String` = ""; 2 `sourceId: Long` (required) |

`BackupTracking.syncId` values come from `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:19-40` (section 9.3).

### 4.9 Preferences and extensions

| Message | Fields |
|---|---|
| `BackupPreference` (`models/BackupPreference.kt:7-10`) | 1 `key: String`; 2 `value: PreferenceValue` (sealed) |
| `BackupSourcePreferences` (`:13-16`) | 1 `sourceKey: String`; 2 `prefs: List<BackupPreference>` |
| `PreferenceValue` subclasses (`:21-37`) | `IntPreferenceValue(Int)`, `LongPreferenceValue(Long)`, `FloatPreferenceValue(Float)`, `StringPreferenceValue(String)`, `BooleanPreferenceValue(Boolean)`, `StringSetPreferenceValue(Set<String>)`. Tag numbers: fixture only (section 4.1) |
| `BackupExtensionRepos` (`models/BackupExtensionRepos.kt:8-14`) | 1 `baseUrl`; 2 `name`; 3 `shortName: String?`; 4 `website`; 5 `signingKeyFingerprint` |
| `BackupExtensionStore` (`models/BackupExtensionStore.kt:8-17`) | 1 `indexUrl`; 2 `name`; 3 `badgeLabel: String?`; 5 `signingKey`; 4 `contactWebsite`; 6 `contactDiscord: String?`; 7 `isLegacy: Boolean?`; 8 `extensionListUrl: String?`. The declaration order in the source is 1, 2, 3, 5, 4, 6, 7, 8. Match on the number, not the position |
| `BackupCustomButtons` (`models/BackupCustomButtons.kt:8-16`) | 1 `name: String`; 2 `isFavorite: Boolean`; 3 `sortIndex: Long`; 4 `content: String`; 5 `longPressContent: String`; 6 `onStartup: String` |
| `BackupExtension` (`models/BackupExtension.kt:7-10`) | 1 `pkgName: String`; 2 `apk: ByteArray` |

### 4.10 Dropped backup pieces (Kurobayomi)

- `BackupExtension.apk` (container 504 and legacy 106): Kurobayomi extensions are not APKs ([area:20](20-extension-strategy-redesign/00-overview.md)). Export writes an empty list. Import reads the list and counts every entry as skipped. User loses: the installed-extension list does not reinstall automatically; the user reinstalls from the Kurobayomi extension store ([area:22](22-backup-restore/00-overview.md) 4.3).
- `BackupExtensionPreferences`, `full/models/*`, `BrokenBackupSource`, `BrokenBackupAnimeSource`, and the `backup_flags` preference are dead in the Kotlin (no references in the repository, [area:21](21-backup-format-and-create/00-overview.md) D9). Not written. On read, field 100 and 102 are ignored.
- `ExtensionsBackupCreator` APK extraction: opt-in and off by default (`BackupOptions.extensions = false`), so it is dropped for v1.

### 4.11 Enum and status tables (OBSERVED)

| Enum | Values, in ordinal order | Source |
|---|---|---|
| Manga `UpdateStrategy` | `ALWAYS_UPDATE` = 0, `ONLY_FETCH_ONCE` = 1 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt:15,22` |
| Anime `AnimeUpdateStrategy` | `ALWAYS_UPDATE` = 0, `ONLY_FETCH_ONCE` = 1 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt:15,22` |
| Anime `FetchType` | `Seasons` = 0, `Episodes` = 1 | `FetchType.kt:14,19` |
| Manga `SManga` status | `UNKNOWN` 0, `ONGOING` 1, `COMPLETED` 2, `LICENSED` 3, `PUBLISHING_FINISHED` 4, `CANCELLED` 5, `ON_HIATUS` 6 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt:48-54` |
| Anime `SAnime` status | `UNKNOWN` 0, `ONGOING` 1, `COMPLETED` 2, `LICENSED` 3, `PUBLISHING_FINISHED` 4, `CANCELLED` 5, `ON_HIATUS` 6, `UPCOMING` 7 | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt:72-79` |

The status values are stored as plain integers in `status` (field 8) and in the entry tables. An integer outside the table is kept as-is and displayed as unknown (the Kotlin does not validate it; PROPOSAL: keep the raw value so a round-trip is lossless). `SManga` has no `UPCOMING` constant in this commit (grep checked).

Ordinal values of the enums are the wire values, so the Rust enum maps `0` and `1` to the variants listed above. An unknown ordinal falls back to the default (the Kotlin does the same for enum values that do not decode, [area:02](02-preferences-system/00-overview.md) D4).

### 4.12 Bit-flag words stored in backups

| Backup field | Word | Layout | Area |
|---|---|---|---|
| `BackupManga.chapterFlags` (101) | chapter flags | `Manga.kt:75-103`: `CHAPTER_SORT_ASC = 0x1`, `CHAPTER_SHOW_UNREAD = 0x2`, `CHAPTER_SHOW_READ = 0x4`, `CHAPTER_SHOW_DOWNLOADED = 0x8`, `CHAPTER_SHOW_NOT_DOWNLOADED = 0x10`, `CHAPTER_SHOW_BOOKMARKED = 0x20`, `CHAPTER_SHOW_NOT_BOOKMARKED = 0x40` | [area:07](07-domain-entries-items/p1-topic-flags-bit-layouts.md) |
| `BackupAnime.episodeFlags` (101) | episode flags | `Anime.kt:215-257`: sort ASC 0x1, unseen 0x2/0x4, downloaded 0x8/0x10, bookmark 0x20/0x40, filler 0x80/0x100, sort field 0x600 (SOURCE 0, NUMBER 0x200, UPLOAD 0x400, ALPHABET 0x600), previews 0x800, summaries 0x1000, display NUMBER 0x100000 | [area:07](07-domain-entries-items/p1-topic-flags-bit-layouts.md) |
| `BackupAnime.seasonFlags` (504) | season flags | `Anime.kt:257-308` | [area:07](07-domain-entries-items/p1-topic-flags-bit-layouts.md) |
| `BackupAnime.viewer_flags` (103) | anime viewer | `Anime.kt:310-313` intro and airing masks | [area:07](07-domain-entries-items/p1-topic-flags-bit-layouts.md) |
| `BackupCategory.flags` (100) | library sort mode | type mask `0x3C`, direction `0x40`, bits outside `0x7C` preserved | [area:08](08-domain-library-category-history-updates/p1-topic-flag-algebra-and-sort-modes.md) |
| `BackupManga.viewer_flags` (103) / `viewer` (14) | reading mode and orientation | `ReadingMode.MASK = 0x7` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt:60`): DEFAULT 0, LEFT_TO_RIGHT 1, RIGHT_TO_LEFT 2, VERTICAL 3, WEBTOON 4, CONTINUOUS_VERTICAL 5. `ReaderOrientation.MASK = 0x38` (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt:65`, orientation values in the enum, e.g. `REVERSE_PORTRAIT` = 0x30) | [area:44](44-reader-viewers/00-overview.md) |

Rule: a word read from a backup is stored as-is. Bits the Kurobayomi code does not understand are kept on write (`setFlag` never touches bits outside its mask, [area:07](07-domain-entries-items/p1-topic-flags-bit-layouts.md) section 1).

### 4.13 Backup file name and retention (OBSERVED)

- Manual or automatic file name: `${applicationId}_yyyy-MM-dd_HH-mm.tachibk` (`BackupCreator.kt:232-235`, `SimpleDateFormat("yyyy-MM-dd_HH-mm", Locale.ENGLISH)`). `BuildConfig.APPLICATION_ID` is the build's application id, which differs between build variants. Kurobayomi uses `kurobayomi_yyyy-MM-dd_HH-mm.tachibk` in its own folder (PROPOSAL, [area:21](21-backup-format-and-create/00-overview.md)).
- Retention: `MAX_AUTO_BACKUPS = 4` (`BackupCreator.kt:229`), applied with `.drop(MAX_AUTO_BACKUPS - 1)` at line 89. The regex `FILENAME_REGEX` (line 230) is used to find old files. Its `.` before `tachibk` is unescaped in the source (matches any character). Kurobayomi escapes it.
- Aniyomi prunes before writing (`BackupCreator.kt:89`). Kurobayomi prunes after a successful write (decision D6 in [area:21](21-backup-format-and-create/00-overview.md)).

---

## 5. Source identity

### 5.1 Algorithm (OBSERVED, both kinds)

`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:101-105` (anime), `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:87-91` (manga):

```kotlin
val key = "${name.lowercase()}/$lang/$versionId"
val bytes = MessageDigest.getInstance("MD5").digest(key.toByteArray())
return (0..7).map { bytes[it].toLong() and 0xff shl 8 * (7 - it) }.reduce(Long::or) and Long.MAX_VALUE
```

Operator precedence: `and` and `shl` are infix functions of equal precedence, applied left to right. `8 * (7 - it)` binds tighter. So byte `i` goes to bits `8*(7-i)` through `8*(7-i)+7`. The result is big-endian and the top bit is cleared.

- `name`: the source's `name` property. `lang`: its `lang` string as declared. `versionId`: the extension's version id, default `1`.
- `name.lowercase()` is Unicode, root locale. [area:20](20-extension-strategy-redesign/00-overview.md) Q6 keeps Unicode lowercasing (`str::to_lowercase`). Special cases (Turkish dotted I and similar) must be covered by golden vectors produced by running the Kotlin (E12 in area:20). This pass did not run those cases.
- Local sources: `LocalMangaSource.ID = 0L` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/manga/LocalMangaSource.kt:378`) and `LocalAnimeSource.ID = 0L` (`source-local/src/androidMain/kotlin/tachiyomi/source/local/entries/anime/LocalAnimeSource.kt:365`).

### 5.2 Worked vectors (computed in this pass with MD5, Python `hashlib`, same formula)

| Key string | Hex result | Decimal result |
|---|---|---|
| `sample/en/1` | `0x304db60dd2b7b2ff` | 3480638257513345791 |
| `sample/en/2` | `0x1a4f38882118cd2e` | 1895796125468249390 |
| `SAMPLE` with `en`, version 1 gives the same key as row 1 | `0x304db60dd2b7b2ff` | 3480638257513345791 |

These three rows are only a sanity check for the implementer. The golden vectors that gate T-20a-011 must still be generated by running the Kotlin (E12, [area:20](20-extension-strategy-redesign/00-overview.md)).

### 5.3 Entry url without domain (OBSERVED)

`AnimeHttpSource.getUrlWithoutDomain` (`AnimeHttpSource.kt:685-699`) uses `java.net.URI(orig)`, then `uri.path` (decoded), `"?" + uri.query` (decoded) if present, `"#" + uri.fragment` (decoded) if present. On `URISyntaxException` it returns the input. Manga twin: `HttpSource.kt:412` (replaces spaces). Full vectors: [area:16](16-source-api-anime/p1-topic-url-and-id-compat.md) section 4.

Compatibility rule: entry and item `url` values in a backup are these strings. Kurobayomi must store them unchanged. A url that Kurobayomi would normalise differently breaks the match on restore, so the restore falls back to a new row.

---

## 6. Preference keys

### 6.1 Storage and key rules (OBSERVED)

`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt`:

- `APP_STATE_PREFIX = "__APP_STATE_"` (line 47). `Preference.appStateKey(k)` = `"__APP_STATE_" + k`. Never exported (`PreferenceBackupCreator.kt:62`).
- `PRIVATE_PREFIX = "__PRIVATE_"` (line 48). Exported only when `BackupOptions.privateSettings = true` (`PreferenceBackupCreator.kt:82`).
- Only values that were set are written. A `delete()` removes the key.
- Types written to a backup (`PreferenceBackupCreator.kt:64-74`): `Int`, `Long`, `Float`, `String`, `Boolean`, `Set<String>` (only when the set holds strings). Other types are dropped (`mapNotNull`).

### 6.2 Keys that matter for backup and restore (OBSERVED)

| Key | Kotlin accessor | Default | Notes |
|---|---|---|---|
| `backup_interval` | `domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt:10` `getInt("backup_interval", 12)` | 12 (hours) | UI values 0, 6, 12, 24, 48, 168 ([area:21](21-backup-format-and-create/00-overview.md) section 1) |
| `__APP_STATE_last_auto_backup_timestamp` | `BackupPreferences.kt:12` `getLong(Preference.appStateKey("last_auto_backup_timestamp"), 0L)` | 0 | never exported; app state |
| `hide_notification_content` | [area:02](02-preferences-system/00-overview.md) | see area 02 | restore notification text only ([area:21](21-backup-format-and-create/00-overview.md) T-21a-030) |

Key spelling rule (decision D3 in [area:02](02-preferences-system/00-overview.md), and [area:21](21-backup-format-and-create/00-overview.md) Q7): keys that are written to a backup use the Aniyomi literal names. Rust field names are the snake_case of the Kotlin function name. Kurobayomi-only keys that must survive a backup use the `kuro_` prefix (Q-G in [area:02](02-preferences-system/00-overview.md)). Keys that are never exported may use any name.

### 6.3 Restore allowlist (PROPOSAL, [area:22](22-backup-restore/00-overview.md) decision 4)

On restore, a backup key is written only if (a) it is in the Kurobayomi key registry ([area:02](02-preferences-system/00-overview.md) catalog files `catalogs/preferences-01.md`, `preferences-02.md`, `preferences-03.md`, `preferences-04.md`, `preferences-05.md`, `preferences-06.md`, `preferences-07.md`, `preferences-08.md`), and (b) the stored value has the registered primitive type. Other keys are counted as skipped and listed in the report. Keys with `__APP_STATE_` are always skipped. Restoring them would overwrite private state such as the last auto-backup timestamp (T-22a-052, P2).

Type mismatch (a key present with a different value type): Aniyomi deletes the key and returns the default on read ([area:02](02-preferences-system/00-overview.md) D4). Restore does the same.

### 6.4 Android-only keys (OBSERVED, dropped on the UI side)

[area:02](02-preferences-system/00-overview.md) D6: keys for the installer, PiP, display profile, tablet mode, volume keys, cutout, status bar, orientation, secure screen, backup flags and external downloader selection stay in the registry so restored values round-trip unchanged. No UI shows them. `external_player_preference` holds an Android package name in backups (E39); it is kept, and the Linux player uses a command template instead (Q-F in [area:02](02-preferences-system/00-overview.md)).

### 6.5 Stored enum names (OBSERVED, decision D1 in [area:02](02-preferences-system/00-overview.md))

Stored enum values are the Kotlin constant names (for example `ALWAYS_UPDATE`), not ordinals, except where the wire format uses ordinals (section 4.11). A stored value that does not decode returns the default and stays on disk (E1, E2, Q16 in [area:02](02-preferences-system/00-overview.md)).

### 6.6 Per-source preferences

`PreferenceBackupCreator.kt:33-55` writes one `BackupSourcePreferences` per configurable source with at least one key (`filter { it.prefs.isNotEmpty() }`, lines 45 and 55). The `sourceKey` is the key of that source's own preference store (T-02a-027, [area:02](02-preferences-system/00-overview.md)). Restore writes them into the per-source store only if the source is installed; otherwise it skips them with a count.

---

## 7. Filesystem layouts

### 7.1 Filename sanitizer (OBSERVED, `core/common/src/main/java/eu/kanade/tachiyomi/util/storage/DiskUtil.kt:109-143`)

`buildValidFilename(origName)`:

1. `name = origName.trim('.', ' ')` (trims `.` and space from both ends).
2. If `name` is empty: return `"(invalid)"`.
3. Replace each character with `_` if it is in `0x00..0x1F`, or one of `"`, `*`, `/`, `:`, `<`, `>`, `?`, `\`, `|`, or `0x7F`. Other characters are kept.
4. Return the first 240 characters (`take(240)`). Kotlin counts UTF-16 code units. Rust must count UTF-16 code units too (not bytes, not scalar values) to produce identical names.

Other constants: `NOMEDIA_FILE = ".nomedia"` (line 140) and `MAX_FILE_NAME_BYTES = 250` (line 143). `MAX_FILE_NAME_BYTES` is not used by `buildValidFilename`.

### 7.2 Download folders (OBSERVED)

Root: `<base>/downloads/` (`StorageManager.kt:99`, `getDownloadsDirectory`). Then:

- Source directory: `buildValidFilename(source.toString())`. `HttpSource.toString()` = `"$name (${lang.uppercase()})"` (`source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:103`), anime twin at `AnimeHttpSource.kt:117`. Example: name `Sample`, lang `en` → `Sample (EN)`.
- Entry directory: `buildValidFilename(title)` (`AnimeDownloadProvider.kt:124-126`, `MangaDownloadProvider.kt:124-126`).
- Item directory (anime episode): `getEpisodeDirName(name, scanlator)`. `sanitizeEpisodeName(name)` = `name.ifBlank { "Episode" }`. If `scanlator` is not blank: `buildValidFilename("${scanlator}_$newName")`, otherwise `buildValidFilename(newName)` (`AnimeDownloadProvider.kt:134-152`).
- Item directory (manga chapter): same rule with `sanitizeChapterName(name)` = `name.ifBlank { "Chapter" }` (`MangaDownloadProvider.kt:134-152`).
- Lookup candidates for anime: `getValidEpisodeDirNames(name, scanlator)` = `[getEpisodeDirName(...), getOldEpisodeDirName(...)]` (`AnimeDownloadProvider.kt:181-185`). `getOldEpisodeDirName` uses `scanlator != null` (not blank) and no `"Episode"` fallback (`:161-167`). The first match wins per item (`:101-107`).
- Lookup candidates for manga: `[chapterDirName, "$chapterDirName.cbz"]` (`MangaDownloadProvider.kt:166-177`). The `.cbz` candidate is the archive written when "save chapters as CBZ" (`save_chapter_as_cbz`, default `true`) is set. There is no archive form for anime.

Compatibility rule (PROPOSAL): Kurobayomi writes the same names and reads both candidate forms. A user copying an Aniyomi download folder can then use it without re-downloading. [area:24](24-downloads-anime/00-overview.md) T-24a-005 implements the legacy name.

### 7.3 Covers (OBSERVED)

- Manga covers: file name = `md5(thumbnailUrl)` (lowercase hex, `DiskUtil.hashKeyForDisk` = `Hash.md5(key)`, `DiskUtil.kt:32-34`), in `covers/` (`MangaCoverCache.kt:38-41`).
- Manga custom covers: file name = `md5(mangaId.toString())`, in `covers/custom/` (`MangaCoverCache.kt:50`).
- Anime covers: same rule in `animecovers/` (`AnimeCoverCache.kt:38-41`). Custom anime covers: `md5(animeId.toString())` in `animecovers/custom/` (`AnimeCoverCache.kt:50-52`).
- Cover cache dirs use `getExternalFilesDir(dir)` with a fallback to `filesDir/dir` (`AnimeCoverCache.kt:101-104`).

Covers are not in backups. Restore re-downloads the cover from `thumbnailUrl`. Custom covers are not restored in v1. PROPOSAL: keep custom covers out of scope, and list it in the restore report as a loss.

### 7.4 Local source and automatic backups

- Local manga source: `<base>/local/` ([area:14](14-source-local-and-metadata/00-overview.md)).
- Local anime source: `<base>/localanime/` ([area:14](14-source-local-and-metadata/00-overview.md)).
- Automatic backups: `<base>/autobackup/` (`StorageManager.kt:61-62`).

---

## 8. Mapping of Aniyomi concepts to Kurobayomi

| Concept | Aniyomi row | Kurobayomi row | Rule on import |
|---|---|---|---|
| Manga entry | `mangas` (column list in `data/src/main/sqldelight/data/mangas.sq`) | `mangas` (same columns, same order) | Match by `(source, url)`. Miss → insert with `initialized` computed (T-22a-014) |
| Chapter | `chapters` | `chapters` | Match by `(manga, url)`. Read and bookmark from backup. `last_page_read` takes the maximum of local and backup (PROPOSAL, [area:22](22-backup-restore/00-overview.md) decision 2) |
| Episode | `episodes` | `episodes` | Same as chapter. `last_second_seen` takes the maximum. `total_seconds` takes the maximum |
| Read history | `history` | `history` | Match by chapter url (backup `BackupHistory.url`). Only the chapter within the restored manga is searched (T-22a-018) |
| Anime history | `animehistory` (`last_seen` is the column read by the Kotlin, [area:04](04-db-anime-schema-migrations/00-overview.md) Q6) | `animehistory` | Match by episode url (`BackupAnimeHistory.url`) |
| Category | `categories` (`_id`, `name`, `sort`, `flags`, `hidden`) | `categories` | Name match inside the same kind (manga or anime). Duplicate names in one backup are merged by name (PROPOSAL, [area:22](22-backup-restore/00-overview.md) decision 3). Backup `id` is a key for remapping only (section 9.1) |
| Library-to-category link | `mangas_categories` / `animes_categories` | same | Written from the entry's `categories` list after the entry exists |
| Tracker binding | `manga_sync` / `anime_sync` | same | Keyed by `(manga, sync_id)` (T-22a-019). `syncId` maps to a Kurobayomi tracker (section 9.3) |
| Source | `sources` / `animesources` | same | Stub row created when the source is not installed (`getOrStub`, [area:17](17-source-app-layer/00-overview.md)) |
| Extension repository | `extension_repos` / `extension_store` | same | Manga repo: identical repo is a no-op (T-22a-027). Anime store: T-22a-026 |
| Custom button | `custom_buttons` | same | One favourite only (PROPOSAL, T-22a-028) |
| Excluded scanlators | `excluded_scanlators` | same | Replaced by the backup list (T-22a-020) |
| Season | `anime_relations` | same | Pre-pass, child restored inside the parent transaction, orphan `parent_id = NULL` (T-22a-023) |

### 9.1 Category ids

Backups reference categories by backup `id`, not by database `_id`. Restore keeps the id map `backup id → local id` for the run. Category rows with name collisions are resolved by name first (PROPOSAL). Category `_id = 0` is the seeded row with `UNCATEGORIZED_ID = 0L` (`domain/src/main/java/tachiyomi/domain/category/model/Category.kt:16`). PROPOSAL: never remap it. The table layout is in [area:05](05-data-repos-entries-items/p1-topic-categories.md).

### 9.2 Metadata merge rule for entries (OBSERVED, [area:22](22-backup-restore/00-overview.md) decision 1, Kept by default)

When the backup entry is newer (`lastModifiedAt` greater), local `title`, `viewer_flags`, `chapter_flags`, `update_strategy`, `date_added` and `last_update` stay. When the backup is older or equal, backup values overwrite them. Keep Aniyomi's rule (T-22a-012, E-23).

### 9.3 Tracker ids (OBSERVED, `app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:19-40`)

| `syncId` | Tracker | Source line |
|---|---|---|
| 1 | MyAnimeList | `val myAnimeList = MyAnimeList(1L)` |
| 2 | AniList | `const val ANILIST = 2L` |
| 3 | Kitsu | `const val KITSU = 3L` |
| 4 | Shikimori | `Shikimori(4L)` |
| 5 | Bangumi | `Bangumi(5L)` |
| 6 | Komga | `Komga(6L)` |
| 7 | MangaUpdates | `MangaUpdates(7L)` |
| 8 | Kavita | `const val KAVITA = 8L` |
| 9 | Suwayomi | `Suwayomi(9L)` |
| 101 | Simkl | `const val SIMKL = 101L` |
| 102 | Jellyfin | `const val JELLYFIN = 102L` |

Tracker status numbers are per tracker and are owned by [area:27](27-tracking-core/00-overview.md) through [area:33](33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/00-overview.md). T-31a-048 covers the tracker ids and status numbers in backup and restore. A tracker that is not configured on Kurobayomi keeps its binding row in the database and is shown as inactive.

Tracker secrets are not in backups. Kurobayomi stores them in the Secret Service ([area:02](02-preferences-system/00-overview.md) D5). Re-login is required after import.

---

## 10. Import (restore) algorithm for `.tachibk`

Owner: [area:22-backup-restore](22-backup-restore/00-overview.md). Tasks listed in section 13. Steps:

1. **Choose file.** Accept any path. Read the whole file into memory (Aniyomi does the same). Report the byte count in the restore screen.
2. **Sniff** (section 4.2). Map errors to `invalid_backup_file_json` or `invalid_backup_file_unknown` strings (PROPOSAL: "JSON backup not supported", "Backup file is corrupted").
3. **Legacy detect** and convert (section 4.2 steps 3 to 4).
4. **Decode** the `Backup` message. Missing optional fields take the defaults in sections 4.3 to 4.8. Absent `favorite` (100) means favourite.
5. **Preflight** (`BackupFileValidator`, `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt`): list source ids that are not installed (stubs will be created) and trackers that are not logged in. Show them in a warning card. Restore is allowed (Kotlin behaviour; [area:22](22-backup-restore/00-overview.md) decision 11).
6. **Confirm** the restore options. Seven flags (T-22a-002). Default: all on.
7. **Sections, in this order** (PROPOSAL; the rule "sequential, categories before entries" is [area:22](22-backup-restore/00-overview.md) decision D3, and the order below is this plan's concrete choice): categories (manga then anime), extension stores and repos, sources, app preferences, source preferences, custom buttons, entries (manga, then anime), tracking, history, excluded scanlators. Each section runs to completion before the next starts. Kotlin started the sections as racing coroutines.
8. **Categories.** Match by name (section 9.1). Create missing ones with `flags` and `sort` from the backup. Keep system category 0.
9. **Sources.** For each `(name, sourceId)`, create a stub row if the source is not installed. Stub name from `BackupSource.name` (T-22a-026 and [area:17](17-source-app-layer/00-overview.md)).
10. **Preferences.** Apply the allowlist (section 6.3). Re-arm the library and auto-backup schedulers after the app preference restore (T-22a-010).
11. **Entries.** For each backup entry: match by `(source, url)`. Insert or update the entry row. Apply the metadata rule (section 9.2). Set `favorite`, `favorite_modified_at`, `date_added`, `chapter_flags` and the other fields. Recompute the fetch interval (T-22a-021).
12. **Chapters and episodes.** Match by url. Apply the read, bookmark and progress rules. Progress uses the maximum (PROPOSAL, [area:22](22-backup-restore/00-overview.md) decision 2).
13. **Categories of the entry.** Write the link rows from the backup `categories` list after remapping.
14. **Tracking.** Match by `(entry, syncId)`. Merge scope is per entry (T-22a-019). Track secrets are not restored.
15. **History.** Match each `BackupHistory.url` against the entry's chapters only (T-22a-018). Unmatched rows are counted as skipped.
16. **Seasons** (anime). Pre-pass, then children inside the parent transaction. Orphan `parent_id` becomes NULL (T-22a-023, [area:22](22-backup-restore/00-overview.md) decision 9).
17. **Extensions** (`backupExtensions`): counted as skipped (section 4.10).
18. **Progress.** Each emitted step matches its amount exactly (T-22a-006). Emit at most one GNotification update every 500 ms plus a final 100 percent update ([area:22](22-backup-restore/00-overview.md) section 4.1).
19. **Report.** Counts per section, skipped items with reasons, stubs created, trackers not logged in, the metadata-rule decisions made. Written to the error log on every exit path, including cancel ([area:22](22-backup-restore/00-overview.md) decision D15 and D4).
20. **Cancel.** `RestoreError::Cancelled` is not an entry error. Each entry is atomic. The restore is not resumable ([area:22](22-backup-restore/00-overview.md) decision 7).

### 10.1 Errors

| Situation | Message | Behaviour |
|---|---|---|
| Not gzip and starts with `{` | "JSON backup not supported" | Stop. No change |
| Protobuf decode error | "Backup file is corrupted" | Stop. No change |
| Source missing | none (stub created) | Warning card |
| Tracker not logged in | none | Warning card |
| Entry insert fails | the inner error message only (not `java.lang.IllegalStateException:`) | Count as entry error, continue |
| Category name is a duplicate in one backup | none | Merge by name |

Error text: the Kotlin shows `null` for a null message and prefixes `java.lang.IllegalStateException:`. PROPOSAL: show the inner message only (T-22a-043).

---

## 11. Export (write) algorithm for `.tachibk`

Owner: [area:21](21-backup-format-and-create/00-overview.md). Steps as observed (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt`), with the PROPOSAL changes.

1. **Options.** `BackupOptions` has 12 flags. Defaults: `libraryEntries`, `categories`, `chapters`, `tracking`, `history`, `readEntries`, `appSettings`, `extensionStores`, `customButton`, `sourceSettings` all `true`; `privateSettings` and `extensions` `false` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt:9-20`). Automatic backups always use defaults (Q4 in [area:21](21-backup-format-and-create/00-overview.md): no extensions, no private settings).
2. **Target.** Manual: file chosen by the user through the save dialog. Automatic: `<base>/autobackup/` with the file name in section 4.13.
3. **Category list.** System categories are excluded with `filterNot(Category::isSystemCategory)` (`MangaCategoriesBackupCreator.kt:16`, `AnimeCategoriesBackupCreator.kt:16`). The entry query shapes are in the creators listed in [area:21](21-backup-format-and-create/00-overview.md) section 2.1.
4. **Entries.** Manga entries with chapters, categories, tracking, history and excluded scanlators. Anime entries with episodes, categories, tracking, history, `backgroundUrl` and the season fields (section 4.6).
5. **Sources and stores.** `sources` from the used source ids (`MangaSourcesBackupCreator.kt`, `AnimeSourcesBackupCreator.kt`). Stores and repos (`AnimeExtensionStoresBackupCreator.kt`, `MangaExtensionRepoBackupCreator.kt`).
6. **Preferences.** App preferences minus `__APP_STATE_` keys, minus `__PRIVATE_` keys unless `privateSettings`. Source preferences for configured sources (section 6.6).
7. **Assemble** `Backup` (`isLegacy = false`).
8. **Encode** with `ProtoBuf.encodeToByteArray(Backup.serializer(), backup)` (`BackupCreator.kt:131`).
9. **Write** through `gzip` to the target (`BackupCreator.kt:141-142`). Overwrite the target (the Kotlin forces overwrite, `:138`).
10. **Self-check.** The creator decodes the written file again (the pipeline step named in [area:21](21-backup-format-and-create/00-overview.md) section 1, via `BackupFileValidator`). Kurobayomi does the same in `src/backup/validate.rs` (T-21a-023).
11. **Retention.** Keep 4 automatic files (`MAX_AUTO_BACKUPS = 4`). Prune after the write succeeds (PROPOSAL D6).
12. **Timestamp.** Write `__APP_STATE_last_auto_backup_timestamp` for automatic runs only.
13. **Notify** with the create notification texts in [area:21](21-backup-format-and-create/00-overview.md) section 6. `hide_notification_content` applies to restore texts only (T-21a-030).

Export does not write `isLegacy = true`.

---

## 12. Version handling

| Mechanism | Where | Rule |
|---|---|---|
| Container format | `isLegacy` (500) | Absent or `true` with anime sources present → legacy. Written `false` |
| Entry sync version | `version` (manga 109, anime 109, chapter 12, episode 12) | Integer counter bumped by DB triggers (`UPDATE chapters SET version = version + 1` in `data/src/main/sqldelight/data/chapters.sq:43`, and `UPDATE mangas SET version = version + 1` at line 47). Used to resolve restore conflicts |
| Entry modified time | `lastModifiedAt` (106 entry, 11 child) | Seconds for entries, milliseconds for children (see 3.4). Used to pick the newer side for the metadata rule |
| Favourite time | `favoriteModifiedAt` (107) | Seconds |
| Database schema | `PRAGMA user_version` | section 2 |
| App version | none in the backup | Backups carry no app version. Compatibility is decided by the message shape alone |
| Source version | `versionId` (part of the source id) | Source id changes with `versionId`. A source whose version changed gets a new id and a stub. Kotlin behaviour kept |

Kurobayomi writes the same fields. It keeps `version` and `lastModifiedAt` from the backup when they are newer, and recomputes them on local writes through the trigger tests (T-03a-021, TC-S00..TC-S12 in [area:03](03-db-manga-schema-migrations/p1-edge-cases-and-tests.md)).

Q1 in [area:03](03-db-manga-schema-migrations/00-overview.md): if the user does not need Aniyomi or Mihon interchange, the version and timestamp machinery can drop to P2. Default answer in this plan: keep it.

---

## 13. Tasks

Task IDs are from `plans/TASKS.csv` (exported by `python3 plans/_tools/tasks.py export`, 3801 tasks, 0 errors at the time of this pass).

### 13.1 P0 (needed for daily use, no backup yet)

- T-03a-003 Baseline schema v33 SQL and creation (M)
- T-03a-004 Kurobayomi migration runner, versions 34 and later (M)
- T-03a-006 Flag constants and helpers, chapter, category sort, viewer (S)
- T-03a-021 Schema conformance and statement-prepare tests (M)
- T-03a-035 Sibling database contract for the anime schema (S)
- T-04a-002 Schema version guard and fresh-install creation (S)
- T-04a-003 Embed the fresh anime schema (M)
- T-04a-006 Flag bit-layout constants (S)
- T-20a-003 Source manager and stub sources for missing extensions (M). Needed so an imported backup can show its entries without the source installed
- T-20a-011 HTTP source base, id generation and URL normalisation (M). Gates every source id in sections 5 and 10. Golden vectors from the Kotlin are required (E12 in area:20)

### 13.2 P1 (backup, restore, interchange)

- Backup write: T-21a-002 container schema, T-21a-003 manga schema and mappers, T-21a-004 anime schema and mappers, T-21a-005 category, source, store, repo, button and extension schema, T-21a-006 preference schema, T-21a-009 `LegacyBackup` to `Backup` conversion, T-21a-010 `BackupOptions`, T-21a-012 pipeline, T-21a-013 manga creator, T-21a-014 anime creator, T-21a-015 category creators, T-21a-020 app preferences creator, T-21a-021 source preferences creator, T-21a-024 preflight, T-21a-034 fixtures and wire compatibility (sealed `PrefValue`, enum ordinals), T-21a-043 read Mihon-shaped backups.
- Backup scheduler: T-21a-026 scheduler, T-21a-031 last timestamp, T-21a-032 interval setting, T-21a-037 storage directory, T-21a-039 migration equivalents.
- Restore: T-22a-001 restore page entry, T-22a-002 options (seven flags), T-22a-003 decode pipeline, T-22a-004 validation, T-22a-005 restore job runner, T-22a-008 category restore, T-22a-009 preference restorer (type gating, category remap, string sets), T-22a-010 re-arm schedulers, T-22a-017 manga category membership, T-22a-018 manga history, T-22a-019 manga tracks, T-22a-020 excluded scanlators, T-22a-021 fetch interval recomputation, T-22a-023 season restore, T-22a-025 anime history, tracks and categories, T-22a-026 anime extension stores, T-22a-027 manga extension repos, T-22a-028 custom buttons, T-22a-047 file acceptance and content sniffing, T-22a-051 wire fixtures for `PreferenceValue` and enum encodings.
- Database import and compatibility: T-03a-031 backup and restore query contract, T-04a-023 DB-level restore API, T-04a-031 file safety (quick_check, backup before upgrade, quarantine), T-05a-032 compatibility test against real Aniyomi databases, T-06a-054 legacy preference import helper, T-08a-040 fixtures and manual compatibility checklist.
- Tracker identity: T-31a-048 backup and restore compatibility for tracker ids and status numbers. (Source identity tasks T-20a-011 and T-20a-003 are P0, listed in 13.1.)
- Migration equivalents: T-11a-001 framework conformance, T-11a-002 migration list, T-11a-004 automatic backup set-up, T-11a-008 dangling category id pruning.
- Downloads: T-24a-005 legacy episode directory name compatibility.

### 13.3 P2 (optional import and long tail)

- T-03a-022 (embedded legacy migrations 1..32 and replay harness) and T-03a-023 (`tachiyomi.db` importer): promoted to P1 only when Q-DB1 is yes.
- T-04a-025: raw `tachiyomi.animedb` import (versions 114 to 138). Gated by Q-DB2.
- T-04a-036: native anime step 139 to 140 (category-link indexes).
- T-03a-024: migration 34 (`chapters.url` index and favourites-first library view). Q6 of area 03 keeps it at P2.
- T-21a-041: systemd user timer for automatic backups with the app closed.
- T-22a-043: localized restore error and status strings.
- T-22a-052: preference key allowlist on restore (the P2 rating in TASKS.csv applies; section 6.3 recommends it before the first restore is released).

### 13.4 P3 (drop decisions with replacement)

- APK extension payloads, MIUI warning, Android installer, WorkManager-specific behaviour: dropped. Section 4.10 and [area:22](22-backup-restore/00-overview.md) 4.3.

---

## 14. Open questions for the user

| ID | Question | Recommendation | Affects |
|---|---|---|---|
| Q-DB1 | Do you have an Aniyomi library file (`tachiyomi.db`) to bring over? | If yes, promote T-03a-022 and T-03a-023 to P1 and schedule them before daily use. Otherwise keep P2. Backup restore is the default path. | section 3.2 |
| Q-DB2 | Import a raw `tachiyomi.animedb`? | No for v1. The chain is verified only on a reconstructed v113 baseline. Provide a real file as a fixture first. | section 3.3 |
| Q-BK1 | Must Kurobayomi backups be readable by Aniyomi and Mihon? | Yes. Keep the wire format (section 4). | sections 4, 11 |
| Q-BK2 | Store custom covers in backups? | No for v1. Covers are re-downloaded from `thumbnailUrl`. Custom covers are listed as a loss in the restore report. | section 7.3 |
| Q-BK3 | Automatic backups without extensions and without private settings? | Keep the Aniyomi defaults. Tell the user on the Settings row. | section 11 |
| Q-BK4 | Keep Aniyomi's metadata merge rule (section 9.2)? | Keep. | section 10 step 11 |
| Q-BK5 | Take the maximum for reading progress on restore? | Yes (PROPOSAL, [area:22](22-backup-restore/00-overview.md) decision 2). | section 10 step 12 |
| Q-BK6 | Keep the folder layout of Aniyomi downloads? | Yes. It costs nothing, and it allows reuse of an Android download folder. | section 7.2 |
| Q-BK7 | Restore a backup with missing sources? | Allow with a warning card (Kotlin behaviour). | section 10 step 5 |

---

## 15. Verification log for this document

Checked in this pass against `/home/blackbox/aniyomi-src` (commit `97414446b8a95994c72dd33c41c971a89d4d25b8`):

- `AppModule.kt`: database names `tachiyomi.db` and `tachiyomi.animedb`, pragmas, ProtoBuf singleton.
- `models/Backup.kt`, `models/BackupManga.kt`, `models/BackupAnime.kt`, `models/BackupChapter.kt`, `models/BackupEpisode.kt`, `models/BackupHistory.kt`, `models/BackupAnimeHistory.kt`, `models/BackupCategory.kt`, `models/BackupTracking.kt`, `models/BackupAnimeTracking.kt`, `models/BackupSource.kt`, `models/BackupAnimeSource.kt`, `models/BackupPreference.kt`, `models/BackupExtension.kt`, `models/BackupExtensionRepos.kt`, `models/BackupExtensionStore.kt`, `models/BackupCustomButtons.kt`: field numbers and defaults.
- `BackupDecoder.kt`, `BackupDetector.kt`: sniffing and legacy rule.
- `create/BackupCreator.kt`: file name, retention constant, gzip write, `isLegacy = false`.
- `create/BackupOptions.kt`: 12 flags, defaults.
- `create/creators/PreferenceBackupCreator.kt`: type mapping, app-state and private filters.
- `DiskUtil.kt`: `buildValidFilename`, `hashKeyForDisk`.
- `AnimeDownloadProvider.kt`, `MangaDownloadProvider.kt`: directory naming and lookup candidates.
- `MangaCoverCache.kt`, `AnimeCoverCache.kt`: cache dirs and naming.
- `StorageManager.kt`: folder names.
- `Preference.kt`: `__APP_STATE_` and `__PRIVATE_` prefixes.
- `BackupPreferences.kt`: `backup_interval` default 12, last timestamp key.
- `TrackerManager.kt`: tracker ids.
- `SManga.kt`, `SAnime.kt`, `UpdateStrategy.kt`, `AnimeUpdateStrategy.kt`, `FetchType.kt`: enum values.
- `ReadingMode.kt`, `ReaderOrientation.kt`: viewer masks.
- `Manga.kt`, `Anime.kt`: chapter and episode flag constants (via area 07).
- `AnimeHttpSource.kt`, `HttpSource.kt`: `generateId`, `toString`, `getUrlWithoutDomain`.
- `LocalMangaSource.kt`, `LocalAnimeSource.kt`: local source id `0L`.
- `i18n/src/commonMain/moko-resources/base/strings.xml` lines 556-557: backup error text.

Computed in this pass: the three source-id vectors in section 5.2 (Python `hashlib`, same formula as the Kotlin).

Open items, not blocking the plan:

- The sealed `PreferenceValue` tag numbers are not in the source text. They need a real `.tachibk` fixture (T-21a-034, T-22a-051). Until then, no preference value is written.
- Not verified in this pass: the Mihon-only backup fields, and the reading-progress and `BackupFileValidator` details beyond what [area:21](21-backup-format-and-create/00-overview.md) and [area:22](22-backup-restore/00-overview.md) state.
- The download sorting and lookup line numbers `AnimeDownloadProvider.kt:101-107`, `:161-167`, `:181-185` and `MangaDownloadProvider.kt:166-177` come from [area:25](25-downloads-shared/00-overview.md) and [area:24](24-downloads-anime/00-overview.md). The functions were confirmed in this pass (`getValidEpisodeDirNames` at line 181, `getValidChapterDirNames` at line 166).
- Line citations in [area:22](22-backup-restore/00-overview.md) section 4.4 are wrong for D1, D11 and D14. Use the corrected lines given there.
- [area:25](25-downloads-shared/00-overview.md) section 6.3 said `DiskUtil.buildValidFilename` was not specified anywhere. It is now specified in section 7.1 of this file.
