# 47-settings-screens-general — part 2 data models, preferences, files, endpoints, tables

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Types are Kotlin unless marked Rust (proposal).

---

## 1. Preference keys read by the 18 files

The scope reads **one** preference key directly or through a helper, and receives one platform-managed locale value.

| Key | Type | Default | Read by | Source |
|---|---|---|---|---|
| `app_date_format` | String | `""` (empty = localised SHORT date) | WorkerInfoScreen (`Model.constructString`), DebugInfoScreen via `AboutScreen.getFormattedBuildTime`, AboutScreen | `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:35` `preferenceStore.getString("app_date_format", "")`; formatter `UiPreferences.dateFormat(format)` at lines 44-47: `""` -> `DateTimeFormatter.ofLocalizedDate(FormatStyle.SHORT)`, else `ofPattern(format, Locale.getDefault())` |
| (none) app locale | platform | system | AppLanguageScreen | `AppCompatDelegate.getApplicationLocales()` / `setApplicationLocales()` (AndroidX). No Kotlin key is written by the screen. Persisted by AndroidX per-app locale storage. |

Time formatting companion: `DateExtensions.kt:19-23` `toDateTimestampString(formatter)` = `"$date $time"` where `time = DateTimeFormatter.ofLocalizedTime(FormatStyle.SHORT)`.

Preference keys **written** by the 18 files: none. The backup screens do not write preferences; the restore and create jobs do (area:21, area:22).

Rust proposal: settings table key `app_date_format` (text, default `""`) and new key `app_language` (text, default `""`). Store both in the Kurobayomi SQLite settings table (see area:02-preferences-system).

---

## 2. Database tables and models

### 2.1 Anime extension stores — table `extension_store`
Source: `data/src/main/sqldelightanime/dataanime/extension_store.sq` lines 3-12.

| Column | SQL type | Null | Notes |
|---|---|---|---|
| `index_url` | TEXT | NOT NULL, PRIMARY KEY | upsert key |
| `name` | TEXT | NOT NULL | |
| `badge_label` | TEXT | NOT NULL | |
| `signing_key` | TEXT | NOT NULL | sentinel `"NO_SIGNING_KEY"` for stores inserted from preference (`AnimeExtensionStoreRepositoryImpl.kt:29`) |
| `contact_website` | TEXT | NOT NULL | |
| `contact_discord` | TEXT | nullable | |
| `is_legacy` | INTEGER AS Boolean | NOT NULL | 0/1 |
| `extension_list_url` | TEXT | nullable | |

Queries used by the scope (via `AnimeExtensionStoreRepositoryImpl`, `data/src/main/java/mihon/data/extension/anime/repository/AnimeExtensionStoreRepositoryImpl.kt`):
- `extension_storeQueries.upsert(indexUrl, name, badgeLabel, signingKey, contactWebsite, contactDiscord, isLegacy, extensionListUrl)` (lines 56-65).
- `extension_storeQueries.getAll(::extensionStoreMapper)` (line 91 and line 95 as a flow via `handler.subscribeToList`).
- `extension_storeQueries.delete(indexUrl)` (line 103).
- `extension_storeQueries.getCount()` (line 99, flow for counts).
- Ordering: the `getAll` query is the one in the `.sq` file (line 14 onward); the screen does not sort. The list appears in DB order (`ORDER` is whatever `getAll` declares — area:19).

Domain model `AnimeExtensionStore` — `domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt:3-16`:

| Field | Type | Null | Default |
|---|---|---|---|
| `indexUrl` | String | no | — |
| `name` | String | no | — |
| `badgeLabel` | String | no | — |
| `signingKey` | String | no | — |
| `contact` | `Contact` | no | — |
| `contact.website` | String | no | — |
| `contact.discord` | String | yes | null |
| `isLegacy` | Boolean | no | — |
| `extensionListUrl` | String | yes | null |

### 2.2 Manga extension repos — table `extension_repos`
Source: `data/src/main/sqldelight/data/extension_repos.sq` lines 1-7.

| Column | SQL type | Null | Notes |
|---|---|---|---|
| `base_url` | TEXT | NOT NULL, PRIMARY KEY | repo URL without `/index.min.json` (or `/repo.json`) |
| `name` | TEXT | NOT NULL | from `meta.name` |
| `short_name` | TEXT | nullable | from `meta.shortName` |
| `website` | TEXT | NOT NULL | from `meta.website` |
| `signing_key_fingerprint` | TEXT | NOT NULL, **UNIQUE** | conflict key for the Replace flow |

Domain model `ExtensionRepo` — `domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt:3-9`:

| Field | Type | Null |
|---|---|---|
| `baseUrl` | String | no |
| `name` | String | no |
| `shortName` | String | yes |
| `website` | String | no |
| `signingKeyFingerprint` | String | no |

Fetch DTO — `domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoDto.kt`:
- `ExtensionRepoMetaDto { meta: ExtensionRepoDto }`
- `ExtensionRepoDto { name: String, shortName: String?, website: String, signingKeyFingerprint: String }`
- Mapping `toExtensionRepo(baseUrl)` copies the four fields plus `baseUrl`.

Repo details request: `GET "$repo/repo.json"` where `repo` = base URL, parsed with `parseAs<ExtensionRepoMetaDto>()` (`domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt:19-40`). Any exception -> `null` (logged). The screen's `InvalidUrl` therefore also covers "repo.json missing or malformed".

Note the asymmetry: the URL check accepts `.../index.min.json` (regex in area:18) but the details fetch uses `repo.json`. The base URL is what is stored.

### 2.3 Screen state types (in-memory, not persisted)

`AnimeExtensionStoreScreenState` (`AnimeExtensionStoresScreenModel.kt:152-165`):
- `Loading` (object).
- `Success(stores: List<AnimeExtensionStore>, dialog: AnimeExtensionStoreDialog? = null)` with `isEmpty = stores.isEmpty()`.

`AnimeExtensionStoreDialog` (lines 141-150):
- `Create(processing: Boolean = false, errorMessage: String? = null)`
- `Delete(store: AnimeExtensionStore)`
- `Confirm(url: String, alreadyExists: Boolean = false, processing: Boolean = false, errorMessage: String? = null)`

`RepoScreenState` (`MangaExtensionReposScreenModel.kt:131-146`):
- `Loading`
- `Success(repos: ImmutableSet<ExtensionRepo>, oldRepos: ImmutableSet<String>? = null, dialog: RepoDialog? = null)` with `isEmpty`. `oldRepos` unused.

`RepoDialog` (lines 124-129): `Create` (object), `Delete(repo: String)` where String = baseUrl, `Conflict(oldRepo: ExtensionRepo, newRepo: ExtensionRepo)`, `Confirm(url: String)` where String = index URL.

`RepoEvent` (lines 118-122): `LocalizedMessage(stringRes: StringResource)` with subclasses `InvalidUrl` -> `MR.strings.invalid_repo_name`, `RepoAlreadyExists` -> `MR.strings.error_repo_exists`.

`CreateMangaExtensionRepo.Result` (domain): `DuplicateFingerprint(oldRepo, newRepo)`, `InvalidUrl`, `RepoAlreadyExists`, `Success`, `Error`.

`BackupFileValidator.Results` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupFileValidator.kt:73-76`): `missingSources: List<String>`, `missingTrackers: List<String>`.

`MissingRestoreComponents(uri: Uri, sources: List<String>, trackers: List<String>)` and `InvalidRestore(uri: Uri?, message: String)` (`RestoreBackupScreen.kt:231-240`), both `private data class`.

### 2.4 Rust proposal (models)

```rust
// src/settings/repos/model.rs
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeStore {            // table extension_store
    pub index_url: String,         // PK
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,       // "NO_SIGNING_KEY" sentinel kept
    pub contact_website: String,
    pub contact_discord: Option<String>,
    pub is_legacy: bool,
    pub extension_list_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MangaRepo {             // table extension_repos
    pub base_url: String,          // PK
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String, // UNIQUE
}
```

Rust storage: `rusqlite` with the same table and column names so an existing Aniyomi database file (if a user copies one) keeps working. Schema version and migrations belong to area:03 / area:04; the Kurobayomi DB must create these two tables with these exact names and constraints.

---

## 3. Backup file (as used by the backup screens)

Full format in area:21-backup-format-and-create. Facts used by the screens:

| Item | Value | Source |
|---|---|---|
| File name default | `"${BuildConfig.APPLICATION_ID}_yyyy-MM-dd_HH-mm.tachibk"` (date pattern `yyyy-MM-dd_HH-mm`, `Locale.ENGLISH`) | `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:232-235` |
| Container | gzip (`okio.gzip()`) around protobuf message `Backup` | `BackupCreator.kt:141` (`sink().gzip()`) |
| Top-level message fields used by validation | `backupSources` (manga, `sourceId`, `name`), `backupAnimeSources` (anime, `sourceId`, `name`), `backupManga[*].tracking[*].syncId`, `backupAnime[*].tracking[*].syncId` | `BackupFileValidator.kt:30-63` |
| Default options in the file | see `p2-behavior.md` section 5.1 bit array (12 booleans) | `BackupOptions.kt:23-36` |
| Restore options bit array | 7 booleans | `RestoreOptions.kt:18-26` |
| Sensitive data | tracker login tokens, source preferences with passwords (when `privateSettings`) | `BackupOptions.kt` label `private_settings` |

Job input payload (WorkManager `Data`, area:21/22; becomes Rust job arguments):
- Create: `is_auto_backup: Boolean` (false for manual), `location_uri: String`, `options: BooleanArray(12)`. Keys `IS_AUTO_BACKUP_KEY = "is_auto_backup"`, `LOCATION_URI_KEY = "location_uri"`, `OPTIONS_KEY = "options"` (`BackupCreateJob.kt:134-136`).
- Restore: `BackupRestoreJob.start(context, uri, options)` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestoreJob.kt:80-95`).

Rust proposal: `struct CreateJob { location: PathBuf, options: BackupOptions, auto: bool }` and `struct RestoreJob { path: PathBuf, options: RestoreOptions }`. Keep the `.tachibk` extension and gzip+protobuf container so Aniyomi backups restore into Kurobayomi and the reverse (compatibility goal; area:21 owns the decision).

---

## 4. Locale list (AppLanguageScreen input)

### 4.1 Generator
`buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt` (task `generateLocalesConfig`, registered by `i18n/build.gradle.kts:53-55`):
- Input: every `strings.xml` under `i18n/src/commonMain/moko-resources/` (fileTree include `**/strings.xml`), excluding files whose text matches `<resources>\s*</resources>` or `<resources\s*/>` (empty translation).
- Folder name -> tag: `.replace("base", "en").replace("-r", "-").replace("+", "-")`. Sorted by the **folder** name (before replace).
- Output: `xml/locales_config.xml` in generated Android resources, root `<locale-config xmlns:android="http://schemas.android.com/apk/res/android">`, one `<locale android:name="TAG"/>` per entry.
- `app/src/main/AndroidManifest.xml:44` `android:localeConfig="@xml/locales_config"`.

### 4.2 Folder list in this checkout
`i18n/src/commonMain/moko-resources/` contains 67 folders, `base` included:

`am ar as base be bg bn ca ceb cs cv da de el eo es eu fa fi fil fr gl he hi hr hu in it ja jv ka-rGE kk km kn ko lt lv ml mr ms my nb-rNO ne nl nn pl pt pt-rBR ro ru sa sah sc sdh sk sq sr sv ta te th tr uk uz vi zh-rCN zh-rTW`

Mapped tags (after the generator's replacements): `base` -> `en`; `ka-rGE` -> `ka-GE`; `nb-rNO` -> `nb-NO`; `pt-rBR` -> `pt-BR`; `zh-rCN` -> `zh-CN`; `zh-rTW` -> `zh-TW`; all others unchanged (`in` stays `in`, a legacy Indonesian code).

The generator's emptiness filter may drop some folders at build time; the effective list is whatever the build emits. Rust proposal: compute the list at build time from the same folders (`build.rs`), not from the generated file.

### 4.3 Display-name functions
- `LocaleHelper.getLocalizedDisplayName(tag)` (`app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt:55-67`). Special cases: `""` = system default (`LocaleListCompat.getAdjustedDefault()[0]`), `zh-CN` -> `zh-Hans`, `zh-TW` -> `zh-Hant`.
- `LocaleHelper.getDisplayName(tag)` (lines 40-48), same zh mapping, `Locale.displayName`.
- Constants in the same file: `PINNED_KEY = "pinned"`, `LAST_USED_KEY = "last_used"` (internal, used by source language lists, not by this screen).

### 4.4 Language row model
`private data class Language(langTag: String, displayName: String, localizedDisplayName: String?)` (`AppLanguageScreen.kt:123-127`). The `""` row has `localizedDisplayName = null`.

---

## 5. Storage roots and disk stats

Android source (OBSERVED):
- `ContextCompat.getExternalFilesDirs(context, null)` returns app-specific dirs per volume (`/storage/emulated/0/Android/data/<pkg>/files`, `/storage/XXXX-XXXX/Android/data/<pkg>/files`).
- Root = `absolutePath.substringBefore("/Android/")` -> `/storage/emulated/0`, `/storage/XXXX-XXXX`.
- `Environment.getExternalStorageState(root)` must be `MEDIA_MOUNTED` or `MEDIA_MOUNTED_READ_ONLY` (constants `"mounted"`, `"mounted_ro"`).
- `StatFs(path)`: total = `blockCountLong * blockSizeLong`, available = `availableBlocksLong * blockSizeLong` (note: `available`, not `free`). `-1` on any exception.

Linux (Proposal):
- Mount table: `/proc/self/mountinfo` (field 5 = mount point, field 9 after `-` = fs type, field 10 = mount source). Accept fs types: `ext2 ext3 ext4 btrfs xfs f2fs vfat exfat ntfs ntfs3 fuseblk`; require the source to start with `/dev/` or be a `fuseblk` device. Exclude `tmpfs`, `overlay`, `squashfs`, `proc`, `sysfs`, `cgroup*`.
- Stats: `rustix::fs::statvfs`; available = `f_bavail * f_frsize`; total = `f_blocks * f_frsize`.
- Config file location (for the data dir): XDG data dir, `$XDG_DATA_HOME/kurobayomi` (default `~/.local/share/kurobayomi`). Downloads and backups default under it (area:13).

---

## 6. Files and folders written by the scope
- The scope itself writes **no** files. Backup files are written by the create job to the URI the user picks (area:21). Restore reads the chosen file (area:22).
- Clipboard is not a file.
- Rust: no temporary files from these screens. Schema text is embedded at compile time (`include_str!`).

---

## 7. Network endpoints touched (by the screens, through domain code)

| Endpoint | Method | Params | Headers | Auth | Response | Caller | Source |
|---|---|---|---|---|---|---|---|
| `<indexUrl>` (anime store, any URL the user typed) | GET | none | NetworkHelper client defaults (area:12) | none | gzip or plain; JSON object `{...}`, legacy JSON array `[...]` (requires `/index.min.json` suffix), or protobuf | `AnimeExtensionStoresScreenModel.createRepo` via `AnimeExtensionStoreService.fetch` | `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt` |
| `<indexUrl>.replace("/index.min.json","/repo.json")` (legacy anime store) | GET | none | as above | none | legacy repo JSON, may have `indexV2` to recurse | same | same |
| `<baseUrl>/repo.json` (manga repo) | GET | none | as above | none | `{ "meta": { name, shortName, website, signingKeyFingerprint } }` | `CreateMangaExtensionRepo.await` -> `ExtensionRepoService.fetchRepoDetails` | `domain/src/main/java/mihon/domain/extensionrepo/service/ExtensionRepoService.kt:19-40` |
| `<baseUrl>/index.min.json` (manga repo extension list, not fetched by these screens) | GET | none | as above | none | extension list | area:18 | area:18 |
| `<extensionListUrl>` / `<indexUrl>` (anime extension list, refresh only) | GET | none | as above | none | extension list (JSON or protobuf) | `AnimeExtensionStoreService.getExtensions` | same file as above |
| `<indexUrl>` of every stored store (refresh) | GET | none | as above | none | as above | `AnimeExtensionStoreRepositoryImpl.refreshAll` | `AnimeExtensionStoreRepositoryImpl.kt:38-52` |

The screens themselves perform no direct HTTP. Errors from these calls surface only as: manga -> `InvalidUrl`; anime -> `throwable.message ?: "unknown error"` text in the dialog.

Rust proposal: one `reqwest::Client` (rustls), user agent from area:12, timeout as area:12 defines. `gzip` via `flate2::read::GzDecoder` when the first two bytes are `1f 8b`. Decode: first non-whitespace byte `{` -> JSON, `[` -> legacy array, otherwise protobuf (`prost`).

---

## 8. Build metadata consumed by the info screens

| Android `BuildConfig` / helper | Used by | Linux (Proposal) |
|---|---|---|
| `VERSION_NAME` | `getVersionName` Stable branch | `CARGO_PKG_VERSION` |
| `COMMIT_SHA` | Debug and Preview branches | `build.rs`: `git rev-parse --short HEAD` (fallback `unknown`) |
| `COMMIT_COUNT` | Preview branch | `build.rs`: `git rev-list --count HEAD` (fallback `0`) |
| `BUILD_TIME` (ISO-8601 `Instant.parse`) | `getFormattedBuildTime` | `build.rs` writes `KUROBAYOMI_BUILD_TIME` (RFC 3339 UTC, `SOURCE_DATE_EPOCH` honoured) |
| `DEBUG` | Debug branch | `cfg!(debug_assertions)` |
| `isPreviewBuildType` | Preview branch | Cargo feature `preview` (off by default) |
| `APPLICATION_ID` | backup file name | constant `dev.kurobayomi.app` (proposal; area:01 decides the id) |

Source: `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:239-285`.

---

## 9. Enumerations and constant values (exact)

| Name | Values | Source |
|---|---|---|
| `SettingsScreen.Destination.id` | About `0`, DataAndStorage `1`, Tracking `2` | `SettingsScreen.kt:81-85` |
| `TABLET_UI_REQUIRED_SCREEN_WIDTH_DP` | `720` | `app/src/main/java/eu/kanade/tachiyomi/util/system/DisplayExtensions.kt:12` |
| `BackupCreateJob` tags | auto `"BackupCreator"`, manual `"BackupCreator:manual"` | `BackupCreateJob.kt:131-132` |
| `BackupRestoreJob` tag | `"BackupRestore"` | `BackupRestoreJob.kt:104` |
| Work policies | create manual: `ExistingWorkPolicy.KEEP`; restore: `KEEP`; auto periodic: `ExistingPeriodicWorkPolicy.UPDATE` (area:21) | `BackupCreateJob.kt:126`, `BackupRestoreJob.kt:95` |
| `WorkInfo.State` names shown | `ENQUEUED`, `RUNNING`, `SUCCEEDED`, `FAILED`, `CANCELLED`, `BLOCKED` | WorkManager `WorkInfo.State` |
| Repo URL regex | `^https://.*/index\.min\.json$` | `domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt:15` |
| Anime store legacy suffix check | `indexUrl.endsWith("/index.min.json")`; rewrite to `/repo.json` | `AnimeExtensionStoreService.kt` |
| `"unknown error"` | fallback message string | `AnimeExtensionStoresScreenModel.kt:80,84` |
| Focus delay | `0.1.seconds` | `ExtensionReposDialogs.kt:89`, `AnimeExtensionStoresDialogs.kt:98` |
| Toast short duration | `Toast.LENGTH_SHORT` (2 s per Android docs) | `ToastExtensions.kt:14-35` |
| Clipboard toast truncation | `truncateCenter(50)` | `ContextExtensions.kt` (copyToClipboard) |
| Progress bar height | `12.dp`, clip `shapes.small` | `StorageInfo.kt:61-65` |
| Event channel capacity | `Int.MAX_VALUE` (unlimited) | `MangaExtensionReposScreenModel.kt:32` |
| Settings card padding | `LazyColumnWithAction` `padding(horizontal = 16.dp, vertical = 8.dp)` for the action button | `presentation-core/.../LazyColumnWithAction.kt` |
| Worker info horizontal padding | `16.dp` | `WorkerInfoScreen.kt:91` |
| Schema text padding | `16.dp` | `BackupSchemaScreen.kt:72` |
| Shared FAB | Extended FAB, text `action_add`, icon `Icons.Outlined.Add` | `CategoryFloatingActionButton.kt` |
| `shouldExpandFAB` | `(firstVisibleItemIndex == 0 && firstVisibleItemScrollOffset == 0) || lastScrolledBackward || !canScrollForward` | `presentation-core/src/main/java/tachiyomi/presentation/core/util/LazyListState.kt:10-16` |

---

## 10. Bit-flag and boolean-array layouts

No packed integers. Options cross the job boundary as `BooleanArray`.

**BackupOptions (12 entries, index = position in `asBooleanArray()`):**

| Index | Field | Default |
|---|---|---|
| 0 | libraryEntries | true |
| 1 | categories | true |
| 2 | chapters | true |
| 3 | tracking | true |
| 4 | history | true |
| 5 | readEntries | true |
| 6 | appSettings | true |
| 7 | extensionStores | true |
| 8 | customButton | true |
| 9 | sourceSettings | true |
| 10 | privateSettings | false |
| 11 | extensions | false |

**RestoreOptions (7 entries):**

| Index | Field | Default |
|---|---|---|
| 0 | libraryEntries | true |
| 1 | categories | true |
| 2 | appSettings | true |
| 3 | extensionStores | true |
| 4 | customButtons | true |
| 5 | sourceSettings | true |
| 6 | extensions | false |

Rust proposal: `#[derive(Clone, Copy, PartialEq, Eq)] pub struct BackupOptions { ... 12 bool fields ... }` with `to_array(self) -> [bool; 12]` and `from_array([bool; 12])`. Wrong-length input is not possible in Rust; the Kotlin `fromBooleanArray` throws `ArrayIndexOutOfBoundsException` on short arrays. Proposal: missing entries default to the table default (true, except indices 10 and 11), so old job payloads with fewer entries still decode.

---

## 11. Rust crate-level data proposal for config

| Setting | Type | Default | Storage |
|---|---|---|---|
| `app_date_format` | text | `""` | settings table (area:02) |
| `app_language` | text | `""` | settings table (new) |
| `backup.last_auto_timestamp` | i64 | none | area:21 (not owned here) |
| `storage.location` | text path | `$XDG_DATA_HOME/kurobayomi` | area:13 |

No secrets are stored by the scope. The backup file itself may contain tracker tokens (`privateSettings`); the UI warns in the option label only.
