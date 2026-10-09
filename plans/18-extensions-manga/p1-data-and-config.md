# Area 18 — Manga extensions: data models, preferences, files, endpoints, tables

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Sections marked OBSERVED come from the Kotlin code. Sections marked **Proposal:** define the Kurobayomi shape. Behaviour is in `p1-behavior.md`.

---

## 1. Extension models (OBSERVED)

Source: `app/src/main/java/eu/kanade/tachiyomi/extension/manga/model/MangaExtension.kt`, `MangaLoadResult.kt`; `app/src/main/java/eu/kanade/domain/extension/manga/model/MangaExtensions.kt`; `app/src/main/java/eu/kanade/domain/extension/manga/interactor/GetMangaExtensionSources.kt`; `app/src/main/java/eu/kanade/tachiyomi/extension/InstallStep.kt`.

### 1.1 `MangaExtension.Installed` (`MangaExtension.kt:17-32`)
| Field | Kotlin type | Nullable | Default | Meaning |
|---|---|---|---|---|
| `name` | String | no | none | Label minus `"Tachiyomi: "` prefix. |
| `pkgName` | String | no | none | Package name (for example `eu.kanade.tachiyomi.extension.en.example`). |
| `versionName` | String | no | none | Version string, for example `1.4.13`. |
| `versionCode` | Long | no | none | Integer version. |
| `libVersion` | Double | no | none | Derived from `versionName`: `substringBeforeLast('.').toDouble()`. |
| `lang` | String | no | none | `""` (no catalogue sources), the single language, or `"all"` (several). |
| `isNsfw` | Boolean | no | none | From metadata `tachiyomi.extension.nsfw == 1`. |
| `pkgFactory` | String? | yes | none | Metadata `tachiyomi.extension.factory`, verbatim. |
| `sources` | List<MangaSource> | no | none | Loaded source objects (runtime). |
| `icon` | Drawable? | yes | none | Loaded eagerly from the package. |
| `hasUpdate` | Boolean | no | `false` | Set by `withUpdateCheck` and reconciliation. |
| `isObsolete` | Boolean | no | `false` | Set when the package is missing from every repo. Never reset (defect, behaviour section 6). |
| `isShared` | Boolean | no | none | True when installed in the system package manager (Drop on Linux). |
| `repoUrl` | String? | yes | `null` | Set by reconciliation from the matching available entry. |

### 1.2 `MangaExtension.Available` (`MangaExtension.kt:34-46`)
| Field | Kotlin type | Nullable | Meaning |
|---|---|---|---|
| `name` | String | no | `it.name.substringAfter("Tachiyomi: ")`. |
| `pkgName` | String | no | `it.pkg`. |
| `versionName` | String | no | `it.version`. |
| `versionCode` | Long | no | `it.code`. |
| `libVersion` | Double | no | `version.substringBeforeLast('.').toDouble()`. |
| `lang` | String | no | `it.lang` (index value). |
| `isNsfw` | Boolean | no | `it.nsfw == 1`. |
| `sources` | List<Available.MangaSource> | no | `it.sources ?: empty`. |
| `apkName` | String | no | `it.apk`. |
| `iconUrl` | String | no | `"$repoUrl/icon/${pkg}.png"`. |
| `repoUrl` | String | no | Repo base URL as stored. |

Nested `Available.MangaSource` (`MangaExtension.kt:48-61`): `id: Long`, `lang: String`, `name: String`, `baseUrl: String`. `toStubSource()` returns `StubMangaSource(id, lang, name)`.

### 1.3 `MangaExtension.Untrusted` (`MangaExtension.kt:64-73`)
| Field | Type | Nullable | Default |
|---|---|---|---|
| `name` | String | no | none |
| `pkgName` | String | no | none |
| `versionName` | String | no | none |
| `versionCode` | Long | no | none |
| `libVersion` | Double | no | none |
| `signatureHash` | String | no | none (set to `signatures.last()`) |
| `lang` | String? | yes | `null` |
| `isNsfw` | Boolean | no | `false` |

### 1.4 `MangaLoadResult` (`MangaLoadResult.kt:3-7`)
- `Success(extension: Installed)`
- `Untrusted(extension: MangaExtension.Untrusted)`
- `Error` (data object, no payload)

### 1.5 `InstallStep` (`InstallStep.kt:3-15`)
Ordinal order (this order is kept for any persisted or compared value):
| Ordinal | Name | `isCompleted()` |
|---|---|---|
| 0 | `Idle` | true |
| 1 | `Pending` | false |
| 2 | `Downloading` | false |
| 3 | `Installing` | false |
| 4 | `Installed` | true |
| 5 | `Error` | true |

### 1.6 Browse bucket (`app/src/main/java/eu/kanade/domain/extension/manga/model/MangaExtensions.kt:5-10`)
`MangaExtensions(updates: List<Installed>, installed: List<Installed>, available: List<Available>, untrusted: List<Untrusted>)`.

### 1.7 Source row (`GetMangaExtensionSources.kt:33-37`)
`MangaExtensionSourceItem(source: MangaSource, enabled: Boolean, labelAsName: Boolean)`.

### 1.8 Load-internal (`MangaExtensionLoader.kt:437-440`)
`MangaExtensionInfo(packageInfo: PackageInfo, isShared: Boolean)`. Not persisted.

### 1.9 Repo model (outside scope, used here) (`domain/src/main/java/mihon/domain/extensionrepo/model/ExtensionRepo.kt`)
`ExtensionRepo(baseUrl: String, name: String, shortName: String?, website: String, signingKeyFingerprint: String)`.

---

## 2. Aniyomi APK metadata and package flags (OBSERVED)

### 2.1 Manifest metadata keys (read from `ApplicationInfo.metaData`)
| Key | Read as | Missing | Used for |
|---|---|---|---|
| `tachiyomi.extension.class` | String, `!!` | NPE (defect, behaviour 3.8 step 9) | `;`-separated class names. Leading `.` is prefixed with the package name. |
| `tachiyomi.extension.factory` | String? | `null` | `pkgFactory`. |
| `tachiyomi.extension.nsfw` | Int, `getInt` | `0` | NSFW gate. Only `1` is true. |

### 2.2 Required package feature
`<uses-feature name="tachiyomi.extension">` (checked through `PackageInfo.reqFeatures[].name`, `MangaExtensionLoader.kt:399`).

### 2.3 `PackageManager` flags (`MangaExtensionLoader.kt:60-63`)
| Flag | Hex value (Android SDK constant) | Included |
|---|---|---|
| `GET_CONFIGURATIONS` | `0x00004000` | always |
| `GET_META_DATA` | `0x00000080` | always |
| `GET_SIGNATURES` | `0x00000040` | always |
| `GET_SIGNING_CERTIFICATES` | `0x08000000` | only SDK >= 28 (`Build.VERSION_CODES.P`) |

### 2.4 Signing digest
SHA-256 of the certificate DER bytes, lowercase hex, 64 characters (`core/common/src/main/java/eu/kanade/tachiyomi/util/lang/Hash.kt:7-10` alphabet `0123456789abcdef`).

### 2.5 Version-code types
`PackageInfoCompat.getLongVersionCode(pkgInfo)` returns Long. Kurobayomi uses `i64`.

---

## 3. Preferences used in this area

Source of keys and defaults: `app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt`, `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt`, `app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt`, `core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt`, and the raw `preferenceStore` in `MangaExtensionApi`.

| Key (stored) | Type | Default | Read/write sites | Notes |
|---|---|---|---|---|
| `source_languages` | StringSet | `LocaleHelper.getDefaultEnabledLanguages()` = `setOf("all", "en", Locale.getDefault().language)` | `SourcePreferences.kt:23-26`; `GetMangaExtensionsByType`, `GetMangaExtensionLanguages`, `MangaExtensionManager.enableAdditionalSubLanguages` | Enabled extension languages. Sub-language first-run fills it (behaviour 2.6). |
| `show_nsfw_source` | Boolean | `true` (`SourcePreferences.kt:28`) | `GetMangaExtensionsByType` (read at subscribe), `MangaExtensionLoader.loadNsfwSource` (read once per process) | Note the default is **true**. |
| `extension_repos` | StringSet | `emptySet()` (`SourcePreferences.kt`, `mangaExtensionRepos()`) | Legacy; read and deleted by `ExternalRepoMigration` / `TrustExtensionRepositoryMigration` (area:11-app-migrations). | Not used at runtime after migration. |
| `__APP_STATE_trusted_extensions` | StringSet | `emptySet()` (`SourcePreferences.kt:44-47`) | `TrustMangaExtension` | Key = `Preference.appStateKey("trusted_extensions")`, prefix `APP_STATE_PREFIX = "__APP_STATE_"` (`core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:47`). |
| `hidden_catalogues` | StringSet | `emptySet()` (`SourcePreferences.kt:57`) | `GetExtensionSources` | Disabled source ids as decimal strings. |
| `ext_updates_count` | Int | `0` (`SourcePreferences.kt:75`) | `MangaExtensionManager.updatePendingUpdatesCount`, `updatedInstalledExtensionsStatuses` | Badge count. |
| `last_ext_check` | Long | `0` | `MangaExtensionApi.lastExtCheck` (`preferenceStore.getLong("last_ext_check", 0)`) | Unix ms. Raw key, no app-state prefix. |
| `extension_installer` | enum `ExtensionInstaller` | `PACKAGEINSTALLER`, or `LEGACY` if the MIUI installer exists (`ExtensionInstallerPreference.kt:29-33`) | `ExtensionInstallerPreference` | Values in section 4. Dropped. |
| `hide_notification_content` | Boolean | `false` (`SecurityPreferences.kt:19`) | `ExtensionUpdateNotifier.promptUpdates` | When true, the notification shows only the title. |

**Proposal:** Kurobayomi keys live in the app's settings store (area:02-preferences-system). Same names where the meaning is unchanged, so an imported preferences backup keeps its values. `__APP_STATE_trusted_extensions` is kept as a state key. The `extension_installer` key is not created.

---

## 4. Installer preference values (OBSERVED)

`BasePreferences.ExtensionInstaller` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:31-36`):
| Enum name | Title resource | `requiresSystemPermission` | Ordinal |
|---|---|---|---|
| `LEGACY` | `MR.strings.ext_installer_legacy` ("Legacy") | true | 0 |
| `PACKAGEINSTALLER` | `MR.strings.ext_installer_packageinstaller` ("PackageInstaller", not translatable) | true | 1 |
| `SHIZUKU` | `MR.strings.ext_installer_shizuku` ("Shizuku", not translatable) | false | 2 |
| `PRIVATE` | `MR.strings.ext_installer_private` ("Private", not translatable) | false | 3 |

Validation (`ExtensionInstallerPreference.kt:35-45`): `PACKAGEINSTALLER` falls back to `LEGACY` when the MIUI package is present; `SHIZUKU` falls back to the default when Shizuku is not installed. The `entries` list hides `PACKAGEINSTALLER` on MIUI.

**Proposal:** no installer setting. `PRIVATE` behaviour is the only path (T-18a-038 and T-18a-039).

---

## 5. Files and folders on disk

### 5.1 Aniyomi (OBSERVED)
| Path | Created by | Format | Notes |
|---|---|---|---|
| `<filesDir>/exts/<pkgName>.ext` | `installPrivateExtensionFile` (L:100-103) | An APK, copied with `copyAndSetReadOnlyTo` (read-only) | Private extension store. Lookup by name only. |
| `<cacheDir>/temp_<downloadId>` | `MangaExtensionInstaller.installApk` PRIVATE path (I:172) | Temporary APK copy | Deleted after each attempt. |
| DownloadManager destination: `<external files>/Download/<lastPathSegment of the apk URL>` | `MangaExtensionInstaller.downloadAndInstall` (I:86-90) | APK | Deleted by `DownloadManager.remove` when the flow completes. |
| Package archive reads | `PackageManager.getPackageArchiveInfo` | — | Used for private files. |

### 5.2 Kurobayomi (Proposal)
| Path | Purpose | Format | Lifetime |
|---|---|---|---|
| `$XDG_DATA_HOME/kurobayomi/extensions/<pkgName>.kuroext` | Installed extension package (one per package name; the version is inside the archive) | zip (`.kuroext`), section 7 | Until uninstall. Written atomically (section 5.3). Read-only mode `0444`. |
| `$XDG_CACHE_HOME/kurobayomi/extensions/download/<pkgName>-<versionName>.kuroext.part` | In-progress download | raw bytes | Renamed on success, deleted on failure or cancel. |
| `$XDG_CONFIG_HOME/kurobayomi/` | Not used by this area | — | — |

Default `XDG_DATA_HOME` = `~/.local/share`, `XDG_CACHE_HOME` = `~/.cache`, `XDG_CONFIG_HOME` = `~/.config` (XDG Base Directory spec, as implemented by the `dirs` crate).

### 5.3 Atomic commit (Proposal)
1. Write the body to `<target>.part` in the same directory.
2. `fsync` the file.
3. `rename(<target>.part, <target>)` (atomic on the same filesystem).
4. `chmod 0444` on the final file.
5. `fsync` the directory.
The old file is replaced only after step 3 succeeds. Replaces Kotlin `target.delete()` then copy.

---

## 6. Network endpoints (OBSERVED, and Proposal for the Linux package)

### 6.1 Index (Aniyomi, retained)
- **Method:** GET.
- **URL:** `"<repo.baseUrl>/index.min.json"` (`MangaExtensionApi.kt:55`).
- **Headers:** whatever `NetworkHelper.client` sets (area:12-network-stack). No extra header in this area.
- **Auth:** none.
- **Success:** 2xx. Non-2xx → treated as failure (`awaitSuccess`).
- **Response:** JSON array of objects (schema section 7.1).

### 6.2 Repo details (Aniyomi, retained)
- **Method:** GET. **URL:** `"<repo.baseUrl>/repo.json"` (`ExtensionRepoService.kt`).
- **Response:** `{ "meta": { "name", "shortName", "website", "signingKeyFingerprint" } }` (section 7.2).

### 6.3 APK download (Aniyomi, retained only for listing)
- **Method:** GET. **URL:** `"<repo.baseUrl>/apk/<apk>"` (`MangaExtensionApi.kt:135-137`).
- **Headers:** DownloadManager default (none set).
- **Linux:** an `ApkOnly` entry is never downloaded.

### 6.4 Icon (Aniyomi, retained)
- **Method:** GET. **URL:** `"<repo.baseUrl>/icon/<pkg>.png"` (`MangaExtensionApi.kt:129`). Kurobayomi loads it through reqwest into the image cache (area:17-source-app-layer owns the cache); the Android image loader is not carried over.

### 6.5 Kurobayomi package download (Proposal)
- **Method:** GET. **URL:** `"<repo.baseUrl>/<wasm path from index>"`.
- **Headers:** `User-Agent` from area:12-network-stack; `Accept: application/zip, application/octet-stream`.
- **Limits:** response body max `64 MiB` (`67108864` bytes). A larger body aborts with `Error::TooLarge`.
- **Timeouts:** connect 10 s, total per request 120 s (`reqwest::Client::builder().connect_timeout(10 s).timeout(120 s)`).
- **Verification:** `sha256` of the body must equal the index `sha256` (lowercase hex) before commit. Then the `ed25519` signature over the raw sha256 digest bytes must verify against the public key whose SHA-256 equals the repo fingerprint (section 9.2 of `p1-behavior.md` and `p1-rust-design.md` section 6).

### 6.6 Aniyomi endpoint summary
| Operation | Method | URL template | Body | Response | Failure |
|---|---|---|---|---|---|
| List repo | GET | `{base}/index.min.json` | none | JSON array | empty list, log |
| Repo details | GET | `{base}/repo.json` | none | `{meta:{...}}` | `null`, log |
| APK | GET | `{base}/apk/{apk}` | none | bytes | install Error |
| Icon | GET | `{base}/icon/{pkg}.png` | none | PNG | placeholder |

---

## 7. Wire schemas (OBSERVED for Aniyomi, Proposal for `.kuroext`)

### 7.1 `index.min.json` (Aniyomi)
Root: JSON array. Each element:
```json
{
  "name": "Tachiyomi: Example",
  "pkg": "eu.kanade.tachiyomi.extension.en.example",
  "apk": "tachiyomi-en.example-v1.4.13.apk",
  "lang": "en",
  "code": 13,
  "version": "1.4.13",
  "nsfw": 0,
  "sources": [
    { "id": 1234567890123456789, "lang": "en", "name": "Example", "baseUrl": "https://example.org" }
  ]
}
```
Required: `name`, `pkg`, `apk`, `lang`, `code` (number), `version` (string), `nsfw` (number). Optional: `sources` (may be absent, `explicitNulls = false`). Unknown keys ignored.

Kurobayomi extension: `sources[].id` is a 64-bit integer. JSON numbers above 2^53 lose precision in some parsers; `serde_json` parses integers exactly into `i64`, which is correct. Source ids are compared as `i64`, never as `f64`.

### 7.2 `repo.json` (Aniyomi)
```json
{ "meta": { "name": "Example Store", "shortName": "EX", "website": "https://example.org", "signingKeyFingerprint": "<64 lowercase hex>" } }
```
`shortName` nullable. All other fields required.

### 7.3 `.kuroext` package (Proposal)
Zip archive. Required entries:
- `manifest.json`
- `source.wasm` (a WebAssembly component, see area:15-source-api-manga ABI)
- `icon.png` (optional, see the `icon` field below)

Integrity and signature are carried by the index entry, not inside the archive (section 7.4).

`manifest.json`:
```json
{
  "schema": 1,
  "pkg": "eu.kanade.tachiyomi.extension.en.example",
  "name": "Example",
  "versionName": "1.4.13",
  "versionCode": 13,
  "abi": "1.4",
  "lang": "en",
  "nsfw": false,
  "factory": null,
  "hosts": ["example.org", "*.example-cdn.net"],
  "signerPub": "<base64 of the 32-byte ed25519 public key, written by the installer>",
  "sources": [
    { "id": 1234567890123456789, "lang": "en", "name": "Example", "baseUrl": "https://example.org" }
  ],
  "icon": "icon.png"
}
```
Field rules:
- `schema`: integer, must be `1`. Other values → `Error::UnsupportedSchema`.
- `pkg`: 1 to 255 bytes, `[a-z0-9_.]`. Must equal the index `pkg`.
- `name`: 1 to 128 Unicode scalar values. Trimmed. Must not start with `"Tachiyomi: "` (stripped before save, behaviour 3.8 step 1).
- `versionName`: `<abi-major>.<abi-minor>.<patch>` pattern `^\d+\.\d+\.\d+$`. The abi major/minor must equal `abi`.
- `versionCode`: integer `0 <= n <= 2^63-1`.
- `abi`: `"1.4"` or `"1.5"` (inclusive range; see section 16.3 of behaviour).
- `lang`: BCP-47-like tag, 2 to 8 characters, or `"all"`.
- `nsfw`: boolean.
- `factory`: string or `null`.
- `hosts`: array of host patterns the guest may call through the `http` import. Exact host, or `*.` followed by a suffix. Empty array means no network. Required.
- `signerPub`: written by the installer after verification (behaviour 16.4). Required in an installed package; ignored when present in a package downloaded from a repo (the installer overwrites it).
- `sources`: array, may be empty. Each `id` is `i64`, unique within the package. `baseUrl` must parse as an absolute `https://` or `http://` URL.
- `icon`: relative path inside the archive, PNG, at most 256 KiB. Optional, default `icon.png` if that entry exists.

### 7.4 Integrity and signature (Proposal, decided)
- The index entry carries `sha256` (SHA-256 of the whole `.kuroext` file as downloaded) and `ed25519` (base64 signature over the raw 32-byte `sha256` digest).
- The installer verifies, in order: (1) `sha256` of the downloaded bytes equals the index value; (2) `ed25519` verifies against the public key whose SHA-256 equals the repo `signingKeyFingerprint`.
- Decision: no signature file inside the archive. This avoids a self-referential signature and lets a repo re-sign without re-zipping.

### 7.5 Index entry extension for Kurobayomi (Proposal)
Added fields, all optional, ignored by Aniyomi:
- `wasm`: string, relative path to the `.kuroext` file.
- `sha256`: string, 64 lowercase hex characters of the `.kuroext` file.
- `ed25519`: string, standard base64 (with padding) of the 64-byte signature over the **raw 32-byte** `sha256` digest.
- `kind`: string, `"wasm"` or `"apk"`. Derived when absent (section 16.2 of behaviour).
- `ed25519Pub`: string, standard base64 of the 32-byte ed25519 public key that signed the file. Required for `wasm` entries; its SHA-256 must equal the repo `signingKeyFingerprint` (design 6.3).

---

## 8. Constants summary (exact values)

| Constant | Value | Source |
|---|---|---|
| `LIB_VERSION_MIN` | `1.4` | `MangaExtensionLoader.kt:56` |
| `LIB_VERSION_MAX` | `1.5` | `MangaExtensionLoader.kt:57` |
| `EXTENSION_FEATURE` | `"tachiyomi.extension"` | `MangaExtensionLoader.kt:52` |
| `METADATA_SOURCE_CLASS` | `"tachiyomi.extension.class"` | `MangaExtensionLoader.kt:53` |
| `METADATA_SOURCE_FACTORY` | `"tachiyomi.extension.factory"` | `MangaExtensionLoader.kt:54` |
| `METADATA_NSFW` | `"tachiyomi.extension.nsfw"` | `MangaExtensionLoader.kt:55` |
| `PRIVATE_EXTENSION_EXTENSION` | `"ext"` | `MangaExtensionLoader.kt:65` |
| Private dir name | `"exts"` | `MangaExtensionLoader.kt:67` |
| Temp file prefix | `"temp_"` | `MangaExtensionInstaller.kt:172` |
| `APK_MIME` | `"application/vnd.android.package-archive"` | `MangaExtensionInstaller.kt:323` |
| `EXTRA_DOWNLOAD_ID` (installer) | `"ExtensionInstaller.extra.DOWNLOAD_ID"` | `MangaExtensionInstaller.kt:324` |
| `FILE_SCHEME` | `"file://"` | `MangaExtensionInstaller.kt:325` |
| `MangaExtensionInstallActivity` request code | `500` | `MangaExtensionInstallActivity.kt:79` |
| Shizuku permission request code | `14045` | `ShizukuInstallerManga.kt:123` |
| Install broadcast action (PackageInstaller) | `"PackageInstallerInstaller.INSTALL_ACTION"` | `PackageInstallerInstallerManga.kt:132` |
| Queue cancel action | `"Installer.action.CANCEL_QUEUE"` | `InstallerManga.kt:156` |
| Queue cancel extra | `"Installer.extra.DOWNLOAD_ID"` | `InstallerManga.kt:157` |
| Session id regex (Shizuku) | `(?<=\[).+?(?=])` | `ShizukuInstallerManga.kt:124` |
| Update gate | `1.days` = 86,400,000 ms | `MangaExtensionApi.kt:75` |
| Poll interval | `1.seconds` = 1000 ms | `MangaExtensionInstaller.kt:147` |
| MIUI ignore window | `1.seconds` = 1,000,000,000 ns | `MangaExtensionInstallActivity.kt:36` |
| Notification id: updates | `-401` | `Notifications.kt:93` |
| Notification id: installer service | `-402` | `Notifications.kt:94` |
| Notification channel | `"ext_apk_update_channel"` | `Notifications.kt:92` |
| Repo index path | `/index.min.json` | `MangaExtensionApi.kt:55` |
| Repo details path | `/repo.json` | `ExtensionRepoService.kt` |
| APK path | `/apk/` | `MangaExtensionApi.kt:136` |
| Icon path | `/icon/<pkg>.png` | `MangaExtensionApi.kt:129` |
| Lib-version parse | `substringBeforeLast('.')` | `MangaExtensionLoader.kt:264`, `MangaExtensionApi.kt:140` |
| Name prefix stripped | `"Tachiyomi: "` | `MangaExtensionLoader.kt:253`, `MangaExtensionApi.kt:120` |
| Trust key format | `"<pkg>:<versionCode>:<digest>"` | `TrustMangaExtension.kt:16` |
| Sub-language split | `startsWith(deviceLanguage)` | `MangaExtensionManager.kt:181` |

---

## 9. Database (OBSERVED)

### 9.1 Table `extension_repos`
Source: `data/src/main/sqldelight/data/extension_repos.sq`.

```sql
CREATE TABLE extension_repos (
    base_url TEXT NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    short_name TEXT,
    website TEXT NOT NULL,
    signing_key_fingerprint TEXT UNIQUE NOT NULL
);
```

| Column | SQL type | Null | Key | Notes |
|---|---|---|---|---|
| `base_url` | TEXT | not null | primary key | Repo base URL as typed by the user (no normalisation). |
| `name` | TEXT | not null | — | From `repo.json`. |
| `short_name` | TEXT | null | — | From `repo.json` `shortName`. |
| `website` | TEXT | not null | — | From `repo.json` `website`. |
| `signing_key_fingerprint` | TEXT | not null | UNIQUE | 64 lowercase hex, or `NOFINGERPRINT-<n>` (legacy). |

### 9.2 Named queries (verbatim, with semantics)
- `findOne(base_url)`: `SELECT * FROM extension_repos WHERE base_url = :base_url;`
- `findOneBySigningKeyFingerprint(fingerprint)`: `SELECT * FROM extension_repos WHERE signing_key_fingerprint = :fingerprint;`
- `findAll`: `SELECT * FROM extension_repos;` (no `ORDER BY`; order is rowid order in practice and is not guaranteed by SQL).
- `count`: `SELECT COUNT(*) FROM extension_repos;`
- `insert(base_url, name, short_name, website, fingerprint)`: plain `INSERT`. Fails with `SQLiteException` on duplicate base_url or fingerprint.
- `upsert(...)`: `INSERT ... ON CONFLICT(base_url) DO UPDATE SET name = :name, short_name = :short_name, website =: website, signing_key_fingerprint = :fingerprint WHERE base_url = base_url;`
  - Conflict target is base_url only. A second row with the same fingerprint and a different base_url violates the UNIQUE constraint on the **insert** path and raises `SQLiteException`.
  - The `WHERE base_url = base_url` clause is a tautology for the conflicting row (no effect).
- `replace(...)`: `INSERT ... ON CONFLICT(signing_key_fingerprint) DO UPDATE SET base_url = :base_url, name = :name, short_name = :short_name, website =: website WHERE signing_key_fingerprint = signing_key_fingerprint;`
  - Conflict target is the fingerprint. If a row with the same fingerprint exists under another base_url, **its base_url is overwritten** with the new one. The `WHERE` clause is a tautology.
- `delete(base_url)`: `DELETE FROM extension_repos WHERE base_url = :base_url;`

`MangaExtensionRepoRepositoryImpl` (`data/src/main/java/mihon/data/repository/manga/MangaExtensionRepoRepositoryImpl.kt`) maps `SQLiteException` to `SaveExtensionRepoException(ex)` (message `"Error Saving Repository to Database"`) on insert and upsert only. `replace` and `delete` do not map.

### 9.3 Proposed Rust schema (Proposal)
Same table and column names, same types, plus `PRAGMA foreign_keys = ON` (not used by this table). The `upsert` and `replace` statements in Rust use the explicit `excluded.` form:
```sql
INSERT INTO extension_repos (base_url, name, short_name, website, signing_key_fingerprint)
VALUES (?1, ?2, ?3, ?4, ?5)
ON CONFLICT(base_url) DO UPDATE SET
  name = excluded.name,
  short_name = excluded.short_name,
  website = excluded.website,
  signing_key_fingerprint = excluded.signing_key_fingerprint;
```
`replace` in Rust is removed. Its only caller is the Aniyomi repo-move screen (`MangaExtensionReposScreenModel.replaceRepo`, outside this scope), which Kurobayomi implements as `delete(old) + upsert(new)` inside one transaction, so no fingerprint row is ever reassigned silently.

### 9.4 Migration rows (legacy)
`TrustExtensionRepositoryMigration` (area:11-app-migrations) writes rows with `name = "Repo #<index+1>"`, `short_name = NULL`, `website = <base url>`, `signing_key_fingerprint = "NOFINGERPRINT-<index+1>"` (1-based index). Source: `app/src/main/java/mihon/core/migration/migrations/TrustExtensionRepositoryMigration.kt:34-49`.

### 9.5 Kurobayomi tables touched (Proposal)
| Table | Owner | Used by this area |
|---|---|---|
| `extension_repos` | area:03-db-manga-schema-migrations | read all, upsert, delete, count |
| No other table | — | Installed extensions are not stored in the DB. They are derived from the extension directory on each start, as in Aniyomi. |

Persisting the installed list in the DB is **not** done (Aniyomi does not either; the package file is the source of truth).

---

## 10. Bit layouts and numeric enums

- No bit flags are defined in this scope.
- Numeric enums: `InstallStep` ordinals (section 1.5); `ExtensionInstaller` ordinals (section 4; dropped).
- `DownloadManager` statuses used (Android constants, not stored): `STATUS_PENDING = 1`, `STATUS_RUNNING = 2`, `STATUS_PAUSED = 4`, `STATUS_SUCCESSFUL = 8`, `STATUS_FAILED = 16`. Dropped with DownloadManager.
- `PackageInstaller` statuses used in the dropped path: `STATUS_PENDING_USER_ACTION`, `STATUS_FAILURE_ABORTED`, `STATUS_SUCCESS`, default `STATUS_FAILURE`. Dropped.
- Legacy `Activity` result codes: `RESULT_OK` → Installed, `RESULT_CANCELED` → Idle, other → Error. Dropped.

---

## 11. Rust-side type mapping (Proposal, short form; full in `p1-rust-design.md`)

| Kotlin | Rust | Notes |
|---|---|---|
| `Long` | `i64` | versionCode, source id |
| `Double` (libVersion) | `LibVersion { major: u32, minor: u32 }` | Tuple compare. Deliberate fix of the Double quirk. |
| `String?` | `Option<String>` | |
| `List<T>` | `Vec<T>` | |
| `Map<String, T>` | `BTreeMap<String, T>` | Deterministic iteration. |
| `Drawable` | `gdk::Texture` (lazy, behind an `Option`) | UI only. |
| `StateFlow<T>` | owned `T` in `ExtensionStore` + `ExtEvent` | Section 17 of behaviour. |
| `InstallStep` | `enum InstallStep { Idle, Pending, Downloading, Installing, Installed, Error }` | Same order. |
| `MangaLoadResult` | `enum LoadResult { Success(Installed), Untrusted(Untrusted), Error(LoadError) }` | `LoadError` keeps the reason for logging. |

---

## 12. Data fields not carried over

| Field | Reason |
|---|---|
| `isShared` | Shared (system) packages are dropped, so the flag is removed from the Rust struct. |
| `pkgFactory` | Kept as `Option<String>` (the WASM package has a factory export name, see area:15-source-api-manga). |
| `icon` (Drawable) | Replaced by a lazily loaded icon file path (`icon.png` in the package; section 7.3). |

---

## 13. Logging keys (OBSERVED log text, kept verbatim in Rust log events)

These strings are the `logcat` messages. Rust uses `tracing` with the same text in the `message` field.
- `"Installed extension version is higher. Downgrading is not allowed."`
- `"Extension to be installed is not signed."`
- `"Installed extension signature is not matched."`
- `"Failed to copy extension file."`
- `"Extension package is not found ($pkgName)"`
- `"Missing versionName for extension $extName"`
- `"Lib version is $libVersion, while only versions $LIB_VERSION_MIN to $LIB_VERSION_MAX are allowed"`
- `"Package $pkgName isn't signed"`
- `"Extension $pkgName isn't trusted"`
- `"NSFW extension $pkgName not allowed"`
- `"Extension load error: $extName ($pkgName)"`
- `"Extension load error: $extName ($it)"`
- `"Unknown source class type: ${obj.javaClass}"` (exception text)
- `"Failed to get extensions from $repoBaseUrl"`
- `"Failed to read downloaded extension file."`
- `"Couldn't locate downloaded APK"`
- `"Failed to install extension ${entry.downloadId} ${entry.uri}"` (installer services)
- `"Shizuku is not ready to use"`, `"Shizuku was killed prematurely"` (dropped)
- `"Failed to fetch repo details"` (domain service)
- `"Error Migrating Manga Extension Repo with baseUrl: $source"` (migration)
