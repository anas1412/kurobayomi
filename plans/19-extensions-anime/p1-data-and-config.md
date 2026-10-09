# 19-extensions-anime: data models, preferences, files, endpoints, schemas

Paths are repo-relative to `/home/blackbox/aniyomi-src`. OBSERVED items describe the Kotlin. Proposal items describe Kurobayomi. Field and tag numbers are exact.

---

## 1. Domain and runtime models (OBSERVED)

### 1.1 `AnimeExtension.Installed` (MODEL:19-35)

| Field | Kotlin type | Nullable | Default | Notes |
|---|---|---|---|---|
| `name` | String | no | none | From `aniyomix.name` or label minus `"Aniyomi: "` |
| `pkgName` | String | no | none | Android package name |
| `versionName` | String | no | none | Non-empty (LDR:254) |
| `versionCode` | Long | no | none | `PackageInfoCompat.getLongVersionCode` |
| `libVersion` | Double | no | none | 14.0, 16.0, or 17.0 (LDR:265) |
| `lang` | String | no | none | `""`, one language, or `"all"` (LDR:348-353) |
| `isNsfw` | Boolean | no | none | `contentWarning > 0 || nsfw == 1` |
| `isTorrent` | Boolean | no | none | `aniyomix.torrent` or `tachiyomi.animeextension.torrent == 1` |
| `pkgFactory` | String? | yes | null | `tachiyomi.animeextension.factory` |
| `sources` | List<AnimeSource> | no | none | Instantiated sources |
| `icon` | Drawable? | yes | null | Android-only, dropped |
| `hasUpdate` | Boolean | no | false | MODEL:31 |
| `isObsolete` | Boolean | no | false | MODEL:32 |
| `isShared` | Boolean | no | none | Shared (system) or private location |
| `store` | AnimeExtensionStore? | yes | null | Set by reconciliation (MGR:219-221) |

### 1.2 `AnimeExtension.Available` (MODEL:37-66)

| Field | Kotlin type | Nullable | Notes |
|---|---|---|---|
| `name` | String | no | `extension.name` from the store, or the source name after a browse split |
| `pkgName` | String | no | Package name. Browse split rows use `"<pkg>-<sourceId>"` (BYTYPE:47) |
| `versionName` | String | no | |
| `versionCode` | Long | no | |
| `libVersion` | Double | no | `extensionLib.toDouble()` (WIRE:110) or legacy prefix parse (LEG:34) |
| `lang` | String | no | `"all"` when more than one language (WIRE:113) |
| `isNsfw` | Boolean | no | `contentWarning >= MIXED` (WIRE:114) |
| `isTorrent` | Boolean | no | Store field `isTorrent` (default false, WIRE:45) |
| `sources` | List<AnimeSource> | no | May be empty |
| `apkUrl` | String | no | `resources.apkUrl` |
| `iconUrl` | String | no | `resources.iconUrl` |
| `store` | AnimeExtensionStore | no | Owning store |

### 1.3 `AnimeExtension.Available.AnimeSource` (MODEL:52-65)

| Field | Type | Notes |
|---|---|---|
| `id` | Long | Source id, 64-bit |
| `lang` | String | Store `language` field |
| `name` | String | |
| `baseUrl` | String | Store `homeUrl` (WIRE:121) or legacy `baseUrl` (LEG:52-55) |

`toStubSource()` returns `StubAnimeSource(id, lang, name)`.

### 1.4 `AnimeExtension.Untrusted` (MODEL:68-78)

| Field | Type | Default | Notes |
|---|---|---|---|
| `name` | String | none | |
| `pkgName` | String | none | |
| `versionName` | String | none | |
| `versionCode` | Long | none | |
| `libVersion` | Double | none | |
| `signatureHash` | String | none | `signatures.last()` (LDR:283) |
| `lang` | String? | null | |
| `isNsfw` | Boolean | false | |
| `isTorrent` | Boolean | false | |

### 1.5 `AnimeLoadResult` (`app/src/main/java/eu/kanade/tachiyomi/extension/anime/model/AnimeLoadResult.kt:3-7`)
- `Success(extension: AnimeExtension.Installed)`
- `Untrusted(extension: AnimeExtension.Untrusted)`
- `Error` (`data object`)

### 1.6 `AnimeExtensionStore` (`domain/src/main/java/mihon/domain/extension/anime/model/AnimeExtensionStore.kt:3-15`)

| Field | Type | Nullable | Notes |
|---|---|---|---|
| `indexUrl` | String | no | Primary key in the table. Post-redirect URL |
| `name` | String | no | |
| `badgeLabel` | String | no | Short label |
| `signingKey` | String | no | Publisher key digest, or `"NO_SIGNING_KEY"` |
| `contact` | Contact | no | |
| `isLegacy` | Boolean | no | |
| `extensionListUrl` | String? | yes | Separate list URL, or null |
| `contact.website` | String | no | |
| `contact.discord` | String? | yes | |

### 1.7 `AnimeExtensions` browse bucket (`app/src/main/java/eu/kanade/domain/extension/anime/model/AnimeExtensions.kt:5-10`)
`updates: List<Installed>`, `installed: List<Installed>`, `available: List<Available>`, `untrusted: List<Untrusted>`.

### 1.8 `AnimeExtensionSourceItem` (BYSRC:33-37)
`source: AnimeSource`, `enabled: Boolean`, `labelAsName: Boolean`.

### 1.9 `StubAnimeSource` (`domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt:15-19`)
Fields: `id: Long`, `lang: String`, `name: String`, private `isInvalid = name.isBlank() || lang.isBlank()`, `supportsLatest = false`, `supportsRelatedAnime = false`. All fetch methods throw `AnimeSourceNotInstalledException` (STUB:68).

### 1.10 Screen state (SMODEL:141-166)
- `AnimeExtensionStoreDialog`:
  - `Create(processing: Boolean = false, errorMessage: String? = null)`
  - `Delete(store: AnimeExtensionStore)`
  - `Confirm(url: String, alreadyExists: Boolean = false, processing: Boolean = false, errorMessage: String? = null)`
- `AnimeExtensionStoreScreenState`:
  - `Loading` (data object)
  - `Success(stores: List<AnimeExtensionStore>, dialog: AnimeExtensionStoreDialog? = null)` with `isEmpty = stores.isEmpty()`.

### 1.11 `InstallStep` (`app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt:3-15`)

| Ordinal | Name | `isCompleted()` |
|---|---|---|
| 0 | `Idle` | true |
| 1 | `Pending` | false |
| 2 | `Downloading` | false |
| 3 | `Installing` | false |
| 4 | `Installed` | true |
| 5 | `Error` | true |

### 1.12 Store service internal models
- `NetworkAnimeExtensionStore` and nested types: section 3.
- `NetworkLegacyAnimeExtension` (LEG:10-18): `name: String`, `pkg: String`, `apk: String`, `lang: String`, `code: Long`, `version: String`, `nsfw: Int`, `sources: List<Source>?` (nullable, may be absent).
  - `Source(id: Long, lang: String, name: String, baseUrl: String)` (LEG:20-26).
- `NetworkLegacyAnimeExtensionRepo` (LEGREPO:72-83): `indexV2: String?` (JSON `index_v2`), `meta: Meta`.
  - `Meta(name: String, shortName: String?, website: String, signingKeyFingerprint: String)`.
- `BaseNetworkAnimeExtensionStore` (`data/src/main/java/mihon/data/extension/anime/model/BaseNetworkAnimeExtensionStore.kt:5-7`): interface with `toExtensionStore(indexUrl): AnimeExtensionStore`.

---

## 2. Enum wire values

### 2.1 `ContentWarning` (WIRE:65-82)

| Constant | Protobuf tag (`@ProtoNumber`) | JSON accepted names (`@JsonNames`) |
|---|---|---|
| `UNSPECIFIED` | 0 | `CONTENT_WARNING_UNSPECIFIED` (and the constant name `UNSPECIFIED`) |
| `SAFE` | 1 | `CONTENT_WARNING_SAFE` (and `SAFE`) |
| `MIXED` | 2 | `CONTENT_WARNING_MIXED` (and `MIXED`) |
| `NSFW` | 3 | `CONTENT_WARNING_NSFW` (and `NSFW`) |

Comparison uses the ordinal (declaration order 0..3), so `contentWarning >= MIXED` is true for `MIXED` and `NSFW`.

### 2.2 `BasePreferences.ExtensionInstaller` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36`)

| Constant | Title resource | `requiresSystemPermission` |
|---|---|---|
| `LEGACY` | `ext_installer_legacy` | true |
| `PACKAGEINSTALLER` | `ext_installer_packageinstaller` | true |
| `SHIZUKU` | `ext_installer_shizuku` | false |
| `PRIVATE` | `ext_installer_private` | false |

Stored as the enum name (`getEnum`). Key `extension_installer`.

### 2.3 `AnimeExtensionStoreDialog` and screen state
No stored values. See 1.10.

### 2.4 Bit layouts
No bit-flag layout exists in this scope. `PACKAGE_FLAGS` (LDR:56-59) is an Android `PackageManager` flag set, combined with `or`. It is used only by the dropped PackageManager calls (section 9 of `p1-rust-design.md`). Nothing is stored as bits.

---

## 3. Wire formats (OBSERVED)

### 3.1 Anime store JSON and protobuf (WIRE:17-98)

Decoder configuration (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`): `ignoreUnknownKeys = true`, `explicitNulls = false`. Missing nullable properties decode as null.

Store object (`NetworkAnimeExtensionStore`):

| Property | JSON key | Protobuf tag | Type | Required |
|---|---|---|---|---|
| `name` | `name` | 1 | string | yes |
| `badgeLabel` | `badgeLabel` | 2 | string | yes |
| `signingKey` | `signingKey` | 3 | string | yes |
| `contact` | `contact` | 4 | object | yes |
| `extensionList` | `extensionList` | 101 | object | no (null) |
| `extensionListUrl` | `extensionListUrl` | 102 | string | no (null) |

`contact` (`Contact`): `website` (JSON `website`, tag 1, string, required), `discord` (JSON `discord`, tag 2, string, nullable).

`extensionList` (`ExtensionList`): `extensions` (JSON `extensions`, tag 1, list of `Extension`, required).

`Extension`:

| Property | JSON key | Tag | Type | Default |
|---|---|---|---|---|
| `name` | `name` | 1 | string | required |
| `packageName` | `packageName` | 2 | string | required |
| `resources` | `resources` | 3 | `Resources` | required |
| `extensionLib` | `extensionLib` | 4 | string | required, parsed with `toDouble()` |
| `versionCode` | `versionCode` | 5 | Long | required |
| `versionName` | `versionName` | 6 | string | required |
| `contentWarning` | `contentWarning` | 7 | `ContentWarning` | required |
| `isTorrent` | `isTorrent` | 8 | Boolean | `false` |
| `sources` | `sources` | 9 | list of `Source` | required (may be empty) |

`Resources`: `apkUrl` (JSON `apkUrl`, tag 1, string, required), `iconUrl` (JSON `iconUrl`, tag 2, string, required).

`Source`:

| Property | JSON key | Tag | Type | Default |
|---|---|---|---|---|
| `id` | `id` | 1 | Long | required |
| `name` | `name` | 2 | string | required |
| `language` | `language` | 3 | string | required |
| `homeUrl` | `homeUrl` | 4 | string | `""` |
| `mirrorUrls` | `mirrorUrls` | 5 | list<string> | `emptyList()` |
| `message` | `message` | 6 | string? | `null` |

A JSON example of a valid store (Kurobayomi tests use this shape):

```json
{
  "name": "Example Store",
  "badgeLabel": "EX",
  "signingKey": "0000000000000000000000000000000000000000000000000000000000000000",
  "contact": { "website": "https://example.org", "discord": null },
  "extensionList": {
    "extensions": [
      {
        "name": "Example",
        "packageName": "eu.kanade.tachiyomi.animeextension.en.example",
        "resources": { "apkUrl": "https://example.org/apk/example.apk", "iconUrl": "https://example.org/icon/example.png" },
        "extensionLib": "16",
        "versionCode": 3,
        "versionName": "16.3",
        "contentWarning": "SAFE",
        "isTorrent": false,
        "sources": [ { "id": 1234567890123456789, "name": "Example", "language": "en", "homeUrl": "https://example.org" } ]
      }
    ]
  },
  "extensionListUrl": null
}
```

Note the `"contentWarning": "SAFE"` value. The JSON enum accepts the short name or the `CONTENT_WARNING_` name.

### 3.2 Repository detection (legacy)

`LEGREPO` JSON (`repo.json`):

```json
{ "index_v2": "https://example.org/index.v2.json", "meta": { "name": "Example", "shortName": "EX", "website": "https://example.org", "signingKeyFingerprint": "<hex>" } }
```
`index_v2` may be absent (null). `shortName` may be absent (null). `meta` is required, so a document without it fails to decode as this type (SSVC:45-49 relies on that failure).

Legacy extension list (`index.min.json`, JSON array of `NetworkLegacyAnimeExtension`):

```json
[ { "name": "Aniyomi: Example", "pkg": "eu.kanade.tachiyomi.animeextension.en.example", "apk": "example-v16.3.apk", "lang": "en", "code": 3, "version": "16.3", "nsfw": 0, "sources": [ { "id": 1, "lang": "en", "name": "Example", "baseUrl": "https://example.org" } ] } ]
```

### 3.3 Gzip
A body starting with bytes `0x1f 0x8b` is gzip (SSVC:111-121). Decompression happens for store index and extension list GETs (SSVC:32, 74, 86). The legacy repo second fetch (SSVC:40-42) and the legacy list fetch (SSVC:98-99) do not decompress.

### 3.4 Protobuf encoding
kotlinx `ProtoBuf`. The enum is encoded as its `@ProtoNumber` varint. `Long` is a varint. `Boolean` is a varint. Strings are length-delimited. Lists are repeated fields with the same tag.

---

## 4. Preference keys (OBSERVED)

| Key (store key) | Kotlin accessor and file | Type | Default | Read or write sites in scope | Kurobayomi plan |
|---|---|---|---|---|---|
| `source_languages` | `SourcePreferences.enabledLanguages()`, SourcePreferences.kt:23-26 | StringSet | `LocaleHelper.getDefaultEnabledLanguages()` = `setOf("all", "en", Locale.getDefault().language)` (LocaleHelper.kt:72-74) | MGR:182-189, BYLANG:15, BYTYPE:19 | Keep. Default computed from locale |
| `show_nsfw_source` | `SourcePreferences.showNsfwSource()`, SourcePreferences.kt:28 | Boolean | `true` | LDR:37, BYTYPE:16 | Keep. Read at load time and at browse subscription. Make the loader read it live (no once-per-process cache) |
| `hidden_anime_catalogues` | `SourcePreferences.disabledAnimeSources()`, SourcePreferences.kt:56 | StringSet | `emptySet()` | BYSRC:18 | Keep. Values are decimal source ids |
| `__APP_STATE_trusted_extensions` (`Preference.appStateKey("trusted_extensions")`) | `SourcePreferences.trustedExtensions()`, SourcePreferences.kt:44-47 | StringSet | `emptySet()` | TRUST:17, 21-26, 30 | Keep the entry format `"<pkg>:<versionCode>:<digest>"`. Stored in app state |
| `animeext_updates_count` | `SourcePreferences.animeExtensionUpdatesCount()`, SourcePreferences.kt:74 | Int | `0` | MGR:202, 384 | Not persisted in Kurobayomi. Derived in memory (Proposal) |
| `last_ext_check` | raw `preferenceStore.getLong("last_ext_check", 0)`, API:26-28 | Long (ms) | `0` | API:39-51 | Kurobayomi key `anime_ext_last_check` (Proposal, avoids the manga collision) |
| `extension_installer` | `ExtensionInstallerPreference`, ExtensionInstallerPreference.kt:19 | enum name | `LEGACY` on MIUI, else `PACKAGEINSTALLER` (ExtensionInstallerPreference.kt:29-33) | INST:62, SVC | Dropped (only one install path on Linux) |
| `anime_extension_repos` | `SourcePreferences.animeExtensionRepos()`, SourcePreferences.kt:40 | StringSet | `emptySet()` | TrustExtensionRepositoryMigration.kt:20-30 only | Legacy. Migration-only. Import from old backups only (Proposal) |
| `hide_notification_content` | `SecurityPreferences.hideNotificationContent()`, SecurityPreferences.kt:19 | Boolean | `false` | ExtensionUpdateNotifier.kt:31 | Keep. Applies to the update notification text |

Notes:
- `__APP_STATE_` is the app-state prefix (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:47`).
- The stored `trusted_extensions` value is compatible with backups of the Aniyomi `trusted_extensions` preference (the digest is a different algorithm on Linux, so the entries are not carried over as trust; see section 9.3).

---

## 5. Files and folders on disk

### 5.1 Aniyomi (OBSERVED)

| Path | Written by | Read by | Format | Notes |
|---|---|---|---|---|
| `<context.filesDir>/exts/<pkg>.ext` | LDR:96-108 (`copyAndSetReadOnlyTo`) | LDR:141-157, 199-210 | Android APK | Read-only (`setReadOnly`, LDR:146-149) |
| `<context.cacheDir>/temp_<downloadId>` | INST:172-197 | INST:187 | APK copy | Deleted after install |
| `<external files>/Download/<lastPathSegment of apkUrl>` | DownloadManager (INST:86-90) | INST:300-316 | APK | `VISIBLE_NOTIFY_COMPLETED`; deleted by `DownloadManager.remove` (INST:251) |

Folder name `exts` is `getPrivateExtensionDir` (LDR:63). Temp prefix `temp_` (INST:172).

### 5.2 Kurobayomi (Proposal)

| Path | Contents | Created by | Notes |
|---|---|---|---|
| `$XDG_DATA_HOME/kurobayomi/extensions/anime/<pkg>.kuroext` | Verified `.kuroext` zip | Installer (section 8) | Source of truth for installed anime packages. Read-only mode `0444` |
| `$XDG_CACHE_HOME/kurobayomi/ext-download/<pkg>-<versionCode>.kuroext.part` | Partial download | Downloader | Renamed to `.kuroext` after verification. Removed on cancel or failure |
| `$XDG_CACHE_HOME/kurobayomi/ext-icons/<pkg>.png` | Extension icon cache | Icon loader | Optional, from the package `icon.png` or the store `iconUrl` |

The `$XDG_*` roots come from the `Paths` trait defined in `plans/addenda/requirements-cross-platform.md` (the cross-platform rule that every directory comes from that trait).

### 5.3 Atomic commit (Proposal)
Write `<pkg>.kuroext.part` in the same directory as the target, fsync, then `rename` onto `<pkg>.kuroext`. A crash leaves either the old file or the new file. A stray `.part` file is deleted at startup.

---

## 6. Network endpoints

| Name | Method | URL | Request headers | Response expected | Source |
|---|---|---|---|---|---|
| Store index (JSON or protobuf, or legacy list) | GET | the stored `indexUrl`, as typed by the user or as stored (SSVC:31, 85) | Default client headers (`DEFAULT_HEADERS` = empty builder, `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:17`). Default cache control `maxAge(10, MINUTES)` (`Requests.kt:16`) | 2xx body, optionally gzip | SSVC:31, 85 |
| Legacy repo details | GET | `indexUrl.replace("/index.min.json", "/repo.json")` (SSVC:39-40) | as above | `NetworkLegacyAnimeExtensionRepo` JSON, not gzip-decoded | SSVC:40-42 |
| Store extension list (separate URL) | GET | `store.extensionListUrl` (SSVC:73) | as above | JSON `ExtensionList` or protobuf `ExtensionList` | SSVC:73-82 |
| Legacy extension list | GET | `"$storeBaseUrl/index.min.json"` where `storeBaseUrl = indexUrl.removeSuffix("/repo.json")` (SSVC:96-97) | as above | JSON array, not gzip-decoded | SSVC:97-99 |
| APK download (listed only on Linux, see section 8.4) | GET (DownloadManager) | `resources.apkUrl` (WIRE:108) or `"$storeBaseUrl/apk/$apk"` (LEG:32) | none added | APK bytes | INST:82-91 |
| Icon | GET (Coil in the app) | `resources.iconUrl` (WIRE:109-110) or `"$storeBaseUrl/icon/$pkg.png"` (LEG:33) | none | PNG | WIRE:108, LEG:33 |

Status handling: `awaitSuccess()` throws on any non-2xx response. No retries are configured in this scope. The HTTP client, rate limits, and Cloudflare handling come from area:12-network-stack.

---

## 7. Database

### 7.1 Table `extension_store` in the anime database (OBSERVED, SQ:3-12)

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

| Column | SQL type | Nullable | Maps to |
|---|---|---|---|
| `index_url` | TEXT | no, primary key | `indexUrl` |
| `name` | TEXT | no | `name` |
| `badge_label` | TEXT | no | `badgeLabel` |
| `signing_key` | TEXT | no | `signingKey` |
| `contact_website` | TEXT | no | `contact.website` |
| `contact_discord` | TEXT | yes | `contact.discord` |
| `is_legacy` | INTEGER (0 or 1, Kotlin Boolean) | no | `isLegacy` |
| `extension_list_url` | TEXT | yes | `extensionListUrl` |

The database file is area:04-db-anime-schema-migrations. This table has no index beyond the primary key and no triggers.

### 7.2 Named queries (OBSERVED, SQ:14-44)

| Name | SQL | Parameters |
|---|---|---|
| `get` | `SELECT * FROM extension_store WHERE index_url = :indexUrl;` | `indexUrl` |
| `getAll` | `SELECT * FROM extension_store;` | none (no ORDER BY) |
| `getCount` | `SELECT COUNT(*) FROM extension_store;` | none |
| `upsert` | `INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url) VALUES (:indexUrl, :name, :badgeLabel, :signingKey, :contactWebsite, :contactDiscord, :isLegacy, :extensionListUrl) ON CONFLICT(index_url) DO UPDATE SET name = :name, badge_label = :badgeLabel, signing_key =: signingKey, contact_website = :contactWebsite, contact_discord = :contactDiscord, is_legacy = :isLegacy, extension_list_url = :extensionListUrl WHERE index_url = :indexUrl;` | `indexUrl, name, badgeLabel, signingKey, contactWebsite, contactDiscord, isLegacy, extensionListUrl` |
| `delete` | `DELETE FROM extension_store WHERE index_url = :indexUrl;` | `indexUrl` |

Upsert semantics: on a conflict, the row is updated in place. The `index_url` itself is not changed.

### 7.3 Rust schema (Proposal)

```sql
CREATE TABLE IF NOT EXISTS anime_extension_store (
    index_url          TEXT NOT NULL PRIMARY KEY,
    name               TEXT NOT NULL,
    badge_label        TEXT NOT NULL,
    signing_key        TEXT NOT NULL,
    contact_website    TEXT NOT NULL,
    contact_discord    TEXT,
    is_legacy          INTEGER NOT NULL CHECK (is_legacy IN (0, 1)),
    extension_list_url TEXT
);
```
Table name differs from Aniyomi because the Rust database keeps the anime and manga stores in separate files and uses a clearer name. If area:04-db-anime-schema-migrations keeps the Aniyomi name, use `extension_store`. Both are acceptable. Pick one in T-19a-004.

Rust queries: `get_by_index_url`, `list_all`, `count`, `upsert`, `delete_by_index_url`, all with bound parameters. Upsert uses `INSERT ... ON CONFLICT(index_url) DO UPDATE SET ...` with `excluded.*` columns.

### 7.4 Migration rows (OBSERVED)
- Legacy `anime_extension_repos` preference rows are moved in migration version `7f` (TrustExtensionRepositoryMigration.kt:14). That migration does not run on current installs (area:11-app-migrations).
- In the Kurobayomi migration, rows are created only from an imported backup (section 9.1).

---

## 8. Install and package data (Proposal, Linux)

### 8.1 Kurobayomi anime package manifest (`manifest.json` inside `.kuroext`)

```json
{
  "schema": 1,
  "kind": "anime",
  "pkg": "eu.kanade.tachiyomi.animeextension.en.example",
  "name": "Example",
  "versionName": "3.0.0",
  "versionCode": 3,
  "abi": "1.0",
  "lang": "en",
  "nsfw": false,
  "torrent": false,
  "sources": [
    { "id": 1234567890123456789, "lang": "en", "name": "Example", "baseUrl": "https://example.org" }
  ],
  "icon": "icon.png"
}
```

Field rules:
- `schema` (integer): must be `1`. Other values are an unsupported-schema error.
- `kind` (string): must be `"anime"`. The manga package uses `"manga"` (area:18-extensions-manga).
- `pkg` (string): 1 to 255 bytes, characters `[a-z0-9_.]`. Must equal the store `packageName`.
- `name` (string): 1 to 128 Unicode scalar values after trimming. Must not start with `"Aniyomi: "` (the prefix is stripped on save).
- `versionName` (string): pattern `^\d+\.\d+\.\d+$`.
- `versionCode` (integer): `0 <= n <= 9223372036854775807`.
- `abi` (string): `"<major>.<minor>"`. The accepted set is pinned by T-19a-002 against area:16-source-api-anime. Initial accepted set: `["1.0"]`.
- `lang` (string): BCP-47-like tag of 2 to 8 characters, or `"all"`. Derived as in section 2.10 when there are several languages.
- `nsfw` (boolean).
- `torrent` (boolean). Default false when absent.
- `sources` (array, may be empty). `id` is an i64 and unique within the package. `baseUrl` must be an absolute `http://` or `https://` URL.
- `icon` (string, optional): relative path inside the archive. PNG, at most 262144 bytes.

Archive entries: `manifest.json` (required), `source.wasm` (required for `kind = "anime"`), `icon.png` (optional).

### 8.2 Index entry extension (Proposal)
Added under `extensionList.extensions[].resources` in the JSON store. Aniyomi ignores it (`ignoreUnknownKeys = true`):

```json
"resources": {
  "apkUrl": "https://example.org/apk/example.apk",
  "iconUrl": "https://example.org/icon/example.png",
  "kuroext": {
    "url": "https://example.org/kuroext/example-3.kuroext",
    "sha256": "<64 lowercase hex characters of the .kuroext file>",
    "ed25519": "<standard base64 with padding, 64 bytes, over the raw 32-byte sha256 digest>",
    "publicKey": "<standard base64 with padding, the 32-byte ed25519 public key; SHA-256 of its bytes equals the store signingKey>"
  }
}
```

- `kuroext` absent → the entry is `ApkOnly` (the only kind an Aniyomi APK entry can have on Linux).
- `kuroext` present but `ed25519` missing → the entry is listed as `Unsigned` (listed, install disabled). Area:18 section 16.2 calls the same case `Untrusted`; the name is reconciled in T-19a-013.
- `publicKey` is required in addition to area:18-extensions-manga's field list (area:18-extensions-manga section 7.4 names only `sha256` and `ed25519`). Without it the verifier has no key. Its SHA-256 must equal the store `signingKey`; a mismatch is refused before the signature check (`p1-rust-design.md` section 4.2).
- `kuroext.url` is resolved relative to the store index URL when not absolute.

### 8.3 Extension kind (Proposal)
- `Wasm`: `resources.kuroext` present and valid. Installable.
- `Unsigned`: `kuroext` present, `ed25519` or `publicKey` missing or malformed. Listed. Badge "Unsigned package". Install disabled.
- `ApkOnly`: no `kuroext`, or `kuroext` with a malformed `sha256` or `url`. Listed. Badge "Not runnable on Linux". Install disabled.

### 8.4 Trust and signing data (Proposal, consistent with area:18-extensions-manga section 16.4)
- Package signer digest = lowercase hex SHA-256 of the 32-byte ed25519 public key that verifies the package signature.
- Trust entry key = `"<pkg>:<versionCode>:<signer digest>"` (same format as `trusted_extensions`).
- Store trust: store `signingKey` (lowercase hex) equals a signer digest.
- A store `signingKey` of `"NO_SIGNING_KEY"` never matches.

---

## 9. Backup data

### 9.1 `BackupExtensionStore` (OBSERVED, BMODEL:24-34)

Class is `@Serializable`, not a data class. All fields are `var`.

| Property | Protobuf tag | Type | Nullable | Notes |
|---|---|---|---|---|
| `indexUrl` | 1 | String | no | |
| `name` | 2 | String | no | |
| `badgeLabel` | 3 | String | yes | Written as `store.badgeLabel` (never null in practice) |
| `contactWebsite` | 4 | String | no | |
| `signingKey` | 5 | String | no | Declaration order in the class is tags 1, 2, 3, 5, 4, 6, 7, 8. Tag numbers, not order, matter on the wire |
| `contactDiscord` | 6 | String | yes | |
| `isLegacy` | 7 | Boolean | yes | |
| `extensionListUrl` | 8 | String | yes | |

Mapper `backupAnimeExtensionStoreMapper` (BMODEL:36-47) sets every field from `AnimeExtensionStore`.

Creator (BCREATE:13-16): `getExtensionStores.get().map(mapper)`. The backup option `extensionStores` gates it (`BackupCreator.kt:199-201`).

### 9.2 Restore mapping (OBSERVED, BRESTORE:58-70)
`database.extension_storeQueries.upsert(indexUrl, name, badgeLabel = backup.badgeLabel ?: backup.name, signingKey, contactWebsite, contactDiscord, isLegacy = backup.isLegacy ?: true, extensionListUrl)`.
- A missing `badgeLabel` becomes `name`.
- A missing `isLegacy` becomes **true**. An old backup without that field turns every store into a legacy store (section 7.4 branch 3).
- No validation. No network call. The restored row is not fetched and not checked.
- The restorer is called once per backup entry from `BackupRestorer.kt:243`. A thrown exception is recorded as `"Error Adding Anime Repo: ${it.name} : ${e.message}"` (`BackupRestorer.kt:244-245`).

### 9.3 Trusted-extension entries in backup
`trusted_extensions` (section 4) is app-state and is included in backups only when that option is on (area:21-backup-format-and-create). On Linux, the entries are signed with a different key type, so old entries never match. They are harmless and not migrated.

---

## 10. Constants summary (exact values)

| Constant | Value | Source |
|---|---|---|
| `EXTENSION_FEATURE` | `"tachiyomi.animeextension"` | LDR:40 |
| `METADATA_SOURCE_CLASS` | `"tachiyomi.animeextension.class"` | LDR:41 |
| `METADATA_SOURCE_FACTORY` | `"tachiyomi.animeextension.factory"` | LDR:42 |
| `METADATA_NSFW` | `"tachiyomi.animeextension.nsfw"` | LDR:43 |
| `METADATA_HAS_README` | `"tachiyomi.animeextension.hasReadme"` (unused) | LDR:44 |
| `METADATA_HAS_CHANGELOG` | `"tachiyomi.animeextension.hasChangelog"` (unused) | LDR:45 |
| `METADATA_TORRENT` | `"tachiyomi.animeextension.torrent"` | LDR:46 |
| `METADATA_NAME` | `"aniyomix.name"` | LDR:48 |
| `METADATA_EXTENSION_LIB` | `"aniyomix.extensionLib"` | LDR:49 |
| `METADATA_CONTENT_WARNING` | `"aniyomix.contentWarning"` | LDR:50 |
| `METADATA_IS_TORRENT` | `"aniyomix.torrent"` | LDR:51 |
| `SUPPORTED_LIB_VERSIONS` | `listOf(14.0, 16.0, 17.0)` | LDR:53 |
| `PRIVATE_EXTENSION_EXTENSION` | `"ext"` | LDR:61 |
| Private directory | `"exts"` under `filesDir` | LDR:63 |
| `NO_SIGNING_KEY` | `"NO_SIGNING_KEY"` | SREPO:29 |
| Update gate | `1.days` = 86,400,000 ms | API:40 |
| Last-check key | `"last_ext_check"` | API:27 |
| `APK_MIME` | `"application/vnd.android.package-archive"` | INST:323 |
| `EXTRA_DOWNLOAD_ID` (installer) | `"AnimeExtensionInstaller.extra.DOWNLOAD_ID"` | INST:324 |
| `FILE_SCHEME` | `"file://"` | INST:325 |
| Temp file prefix | `"temp_"` | INST:172 |
| Poll interval | `1.seconds` = 1000 ms | INST:147 |
| Activity request code | `500` | ACT:79 |
| MIUI ignore window | `1.seconds` = 1,000,000,000 ns | ACT:36 |
| Shizuku permission request code | `14045` | SHZ:123 |
| Session id regex | `(?<=\[).+?(?=])` | SHZ:124 |
| Queue cancel action | `"InstallerAnime.action.CANCEL_QUEUE"` | IQ:156 |
| Queue cancel extra | `"InstallerAnime.extra.DOWNLOAD_ID"` | IQ:157 |
| Package installer action | `"PackageInstallerInstaller.INSTALL_ACTION"` | PKGI:117 |
| Installer service extra | `"EXTRA_INSTALLER"` | SVC:74 |
| Update notification id | `-401` | Notifications.kt:93 |
| Installer service notification id | `-402` | Notifications.kt:94 |
| Notification channel | `"ext_apk_update_channel"` | Notifications.kt:92 |
| Store index path | `/index.min.json` | SSVC:36, 97 |
| Legacy repo path | `/repo.json` | SSVC:39 |
| Legacy error text | `"Provided legacy store url is not valid"` | SSVC:37 |
| Unknown error fallback | `"unknown error"` | SMODEL:80, 84 |
| Gzip magic | `0x1f 0x8b` | SSVC:114 |
| JSON first byte | `0x7B` (`{`) | SSVC:45, 77, 89 |
| Legacy list first byte | `0x5B` (`[`) | SSVC:35 |
| Lang for several languages | `"all"` | WIRE:113 |
| Hash alphabet | `0123456789abcdef` | Hash.kt:7-10 |
| Default store cache control | `maxAge(10, MINUTES)` | Requests.kt:16 |
| Default language set | `{"all", "en", device}` | LocaleHelper.kt:73 |
| Show NSFW default | `true` | SourcePreferences.kt:28 |
| `indexV2` redirect depth limit | none in Aniyomi (see edge cases); Kurobayomi uses 4 | SSVC:53-55 (Proposal) |

---

## 11. Logging keys (OBSERVED log texts, kept verbatim in Rust log events)

| Log text | Level | Source |
|---|---|---|
| `"Installed extension version is higher. Downgrading is not allowed."` | ERROR | LDR:80 |
| `"Extension to be installed is not signed."` | ERROR | LDR:86 |
| `"Installed extension signature is not matched."` | ERROR | LDR:91 |
| `"Failed to copy extension file."` | ERROR | LDR:110 |
| `"Extension package is not found ($pkgName)"` | ERROR | LDR:188 |
| `"Missing versionName for extension $extName"` | WARN | LDR:255 |
| `"Lib version is $libVersion, while only version(s) ${SUPPORTED_LIB_VERSIONS.joinToString()} are supported"` | WARN | LDR:267 |
| `"Package $pkgName isn't signed"` | WARN | LDR:274 |
| `"Extension $pkgName isn't trusted"` | WARN | LDR:285 |
| `"NSFW extension $pkgName not allowed"` | WARN | LDR:292 |
| `"Extension load error: $extName ($pkgName)"` | ERROR | LDR:301 |
| `"Extension load error: $extName ($it)"` | ERROR | LDR:339, 343 |
| `"Unknown source class type: ${obj.javaClass}"` (exception text) | n/a | LDR:320 |
| `"Failed to read downloaded extension file."` | ERROR | INST:193 |
| `"Couldn't locate downloaded APK"` | ERROR | INST:304 |
| `"Package name not found"` | WARN | RCV:99 |
| `"Failed to add extension store '$updatedIndexUrl'"` | ERROR | SSVC:63-65 |
| `"Failed to fetch extensions for store '${store.name} (${store.indexUrl})'"` | ERROR | SREPO:75-77 |
| `"Failed to refresh extension store '${store.name} (${store.index_url})'"` | ERROR | SREPO:45-46 |
| `"Error Migrating Extension Repo with baseUrl: $source"` | ERROR | TrustExtensionRepositoryMigration.kt:27 |
| `"Not implemented for installer $installerUsed"` | ERROR | SVC:56 |
| `"Shizuku is not ready to use"` | ERROR | SHZ:115 |
| `"Shizuku was killed prematurely"` | ERROR | SHZ:26 |
| `"Failed to install extension ${entry.downloadId} ${entry.uri}"` | ERROR | PKGI:84, SHZ:72 |

User-visible strings are listed in section 12 of this file.

---

## 12. User-visible strings used in this scope (English base values)

Values from `i18n/src/commonMain/moko-resources/base/strings.xml` and `plurals.xml`. The moko key uses dots in XML (for example `extensionStoresScreen.addStore.title`) and underscores in Kotlin (`extensionStoresScreen_addStore_title`).

| Kotlin key | XML key | Value |
|---|---|---|
| `MR.strings.extension_api_error` | `extension_api_error` | `Failed to fetch available extensions` |
| `MR.strings.extensionStores` | `extensionStores` | `Extension stores` |
| `MR.strings.extensionStoresScreen_emptyLabel` | `extensionStoresScreen.emptyLabel` | `You haven't added an extension store yet` |
| `MR.strings.extensionStoresScreen_addStore_title` | `extensionStoresScreen.addStore.title` | `Add extension store` |
| `MR.strings.extensionStoresScreen_addStore_alreadyExists` | `extensionStoresScreen.addStore.alreadyExists` | `This extension store already exists` |
| `MR.strings.extensionStoresScreen_addStore_processing` | `extensionStoresScreen.addStore.processing` | `Processing…` |
| `MR.strings.extensionStoresScreen_addStoreInput_inputLabel` | `extensionStoresScreen.addStoreInput.inputLabel` | `Store index URL` |
| `MR.strings.extensionStoresScreen_addStoreDeeplink_bodyText` | `extensionStoresScreen.addStoreDeeplink.bodyText` | `Do you wish to add the extension store below?` |
| `MR.strings.extensionStoresScreen_deleteStore_title` | `extensionStoresScreen.deleteStore.title` | `Remove extension store` |
| `MR.strings.extensionStoresScreen_deleteStore_body` | `extensionStoresScreen.deleteStore.body` | `Do you wish to remove the extension store "%1$s" (%2$s)?` |
| `MR.strings.information_required_plain` | `information_required_plain` | `*required` |
| `MR.strings.action_add` | `action_add` | `Add` |
| `MR.strings.action_cancel` | `action_cancel` | `Cancel` |
| `MR.strings.action_ok` | `action_ok` | `OK` |
| `MR.strings.action_delete` | `action_delete` | `Delete` |
| `MR.strings.action_open_in_browser` | `action_open_in_browser` | `Open in browser` |
| `MR.strings.action_copy_to_clipboard` | `action_copy_to_clipboard` | `Copy to clipboard` |
| `MR.strings.action_webview_refresh` | `action_webview_refresh` | `Refresh` |
| `MR.strings.ext_installer_shizuku_stopped` | `ext_installer_shizuku_stopped` | `Shizuku is not running` |
| `MR.strings.ext_install_service_notif` | `ext_install_service_notif` | `Installing extension…` |
| `MR.strings.ext_installed` | `ext_installed` | `Installed` |
| `MR.strings.ext_updates_pending` | `ext_updates_pending` | `Updates pending` |
| `MR.strings.ext_obsolete` | `ext_obsolete` | `Orphaned` |
| `MR.strings.ext_untrusted` | `ext_untrusted` | `Untrusted` |
| `MR.strings.ext_trust` | `ext_trust` | `Trust` |
| `MR.strings.ext_install` | `ext_install` | `Install` |
| `MR.strings.ext_update` | `ext_update` | `Update` |
| `MR.strings.ext_update_all` | `ext_update_all` | `Update all` |
| `MR.strings.ext_installing` | `ext_installing` | `Installing` |
| `MR.strings.ext_uninstall` | `ext_uninstall` | `Uninstall` |
| `MR.strings.ext_nsfw_short` | `ext_nsfw_short` | `18+` |
| `MR.strings.ext_nsfw_warning` | `ext_nsfw_warning` | `Sources from this extension may contain NSFW (18+) content` |
| `MR.strings.ext_revoke_trust` | `ext_revoke_trust` | `Revoke trusted unknown extensions` |
| `MR.strings.untrusted_extension` | `untrusted_extension` | `Untrusted extension` |
| `MR.strings.untrusted_extension_message` | `untrusted_extension_message` | `Malicious extensions can read any stored login credentials or execute arbitrary code.` + blank line + `By trusting this extension, you accept these risks.` |
| `MR.strings.requires_app_restart` | `requires_app_restart` | `Requires app restart to take effect` |
| `MR.strings.copied_to_clipboard` | `copied_to_clipboard` | `Copied to clipboard:\n%1$s` (SREPO n/a; ContextExtensions.kt:49) |
| `MR.strings.clipboard_copy_error` | `clipboard_copy_error` | `Failed to copy to clipboard` |
| `MR.strings.ext_installer_pref` | `ext_installer_pref` | `Installer` (outside scope) |
| `MR.strings.ext_installer_shizuku_unavailable_dialog` | `ext_installer_shizuku_unavailable_dialog` | `Install and start Shizuku to use Shizuku as extension installer.` (outside scope) |
| `MR.strings.channel_ext_updates` | `channel_ext_updates` | `Extension updates` |
| `MR.plurals.update_check_notification_ext_updates` | `plurals.xml:89-92` | one: `Extension update available`; other: `%d extension updates available` |
| `AYMR.strings.label_anime_extension_repos` | `label_anime_extension_repos` (i18n-aniyomi) | `Anime extension repos` |
| `AYMR.strings.unofficial_anime_extension_message` | `unofficial_anime_extension_message` (i18n-aniyomi) | `This extension is not from the official list.` |
| `AYMR.strings.label_anime_extensions` | `label_anime_extensions` (i18n-aniyomi) | `Anime Extensions` |

Note: `extensionStoresScreen.*` keys live in `i18n/src/commonMain/moko-resources/base/strings.xml`. The Aniyomi-only keys are in `i18n-aniyomi`.

---

## 13. Data fields not carried over (Proposal)

| Aniyomi data | Reason | Replacement or loss |
|---|---|---|
| `ApplicationInfo` metadata (`aniyomix.*`, `tachiyomi.animeextension.*`) | Read from APK. Kurobayomi reads `manifest.json` instead | Same values in `manifest.json` (section 8.1) |
| `PackageInfo.signingInfo` and `signatures` | Android APK signing | ed25519 signature in `kuroext` (section 8.2) |
| `ApplicationInfo.sourceDir`, `publicSourceDir`, `fixBasePaths` | APK paths | Not needed |
| `icon` (`Drawable`) | Android drawable | PNG cache file (section 5.2) |
| `hasReadme`, `hasChangelog` metadata | Never read | Dropped |
| `pkgFactory` (`tachiyomi.animeextension.factory`) | Only consumed by the APK loader for `AnimeSourceFactory` | Dropped with `AnimeSourceFactory` (area:16-source-api-anime) |
| `isShared` flag | No system package location on Linux | Dropped |
| `DownloadManager` id | Android service id | Replaced by an in-process download id (`u64`) |
| `extension_installer` preference value | One install path only | Dropped; see 4 |
| `mirrorUrls`, `message` on store `Source` | Not mapped into the domain model in Aniyomi | Kept in the Rust wire struct (read and ignored) so that a future feature can use them |
| `NetworkAnimeExtensionStore.Extension.resources` as Android icon URL | Same as icon | Kept as `iconUrl` |
