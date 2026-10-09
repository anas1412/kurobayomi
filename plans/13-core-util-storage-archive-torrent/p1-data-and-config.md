# Area 13 — data models, preference keys, files, formats, endpoints

All source paths are repo-relative to `/home/blackbox/aniyomi-src`. Nothing in this area owns a database table (see section 8). The settings store itself belongs to area:02-preferences-system.

## 1. Preference keys used by this area

| Key | Type (Kotlin) | Default | Read at | Who sets it | Meaning |
|---|---|---|---|---|---|
| `pref_torrserver_enable` | Boolean | `false` | `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt:9` | Torrent settings switch (`PlayerSettingsTorrentScreen.kt:76-89`) | Master switch for torrent playback. |
| `pref_torrserver_shownotice` | Boolean | `false` | `TorrentPreferences.kt:10` | Set `true` the first time the switch is turned on (`PlayerSettingsTorrentScreen.kt:80-83`) | Legal notice shown once. |
| `pref_torrserver_port` | String | `"8090"` | `TorrentPreferences.kt:12` | Port field (`PlayerSettingsTorrentScreen.kt:90-109`) | TorrServer listen port; `0` lets the system choose (per the summary string). Stored as a string, validated as an integer 0..65535. |
| `pref_torrserver_tackers` (sic) | String (multi-line) | the 26-line list in section 6 | `TorrentPreferences.kt:13-41` | Trackers field and reset (`PlayerSettingsTorrentScreen.kt:110-134`) | Newline-separated tracker URLs. The key is misspelled in the source; the spelling must be kept for backup compatibility. |
| `pref_torrserver_proxymode` | enum `ProxyMode`, stored as the constant name | `ProxyMode.None` | `TorrentPreferences.kt:42` | List preference (`PlayerSettingsTorrentScreen.kt:135-148`) | Proxy mode sent to the server; see the mapping table in section 3. |
| `pref_torrserver_proxyurl` | String | `""` | `TorrentPreferences.kt:43` | Proxy URL field (`PlayerSettingsTorrentScreen.kt:149-185`) | Proxy URL; validated (section 5). |
| `split_tall_images` | Boolean | `true` | `domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt:23` | Download settings (`SettingsDownloadScreen.kt`, title `split_tall_images`) | Enables tall-image splitting at download time (`MangaDownloader.kt:585`). |
| `pref_hardware_bitmap_threshold` | Int | `GLUtil.SAFE_TEXTURE_LIMIT` = `2048` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:40`) | `App.kt:141-146` sets it to the device limit when unset | Reader advanced setting (`SettingsAdvancedScreen.kt`) | Android texture/tile size. Dropped on Linux (task T-13a-030); the key is ignored and not migrated. |

Proposed new key (not in the Kotlin, Linux only): `pref_torrserver_binary` (String, default empty) holds the path of the TorrServer binary used by the supervisor (`p1-topic-torrserver-lifecycle.md` section 4.1). It is not part of the Android backup set and is excluded from migration.

Prefix conventions that decide backup behaviour (see behaviour section 1.1):

| Prefix | Rule | Example in this area | Backup |
|---|---|---|---|
| `__APP_STATE_` | internal state, never a user preference | none in this area (used by other areas, e.g. `Preference.appStateKey("incognito_mode")` at `app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:21`) | excluded (`PreferenceBackupCreator.kt:62`) |
| `__PRIVATE_` | secrets and per-user tokens | none in this area (tracker tokens elsewhere, `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`) | excluded (`PreferenceBackupCreator.kt:82`) |

PROPOSAL for Rust: the key strings above are kept byte-identical, so a backup file written by Aniyomi restores into Kurobayomi without renaming. Keys whose type cannot be kept (for example the Android-only hardware threshold) are listed in the migration notes at section 9.

## 2. Enums and their stored values

### 2.1 ProxyMode (`TorrentPreferences.kt:46-51`)

| Constant | `value` sent to TorrServer as `proxyMode` | Stored preference value (`getEnum` uses `name`) | Label key (`PlayerSettingsTorrentScreen.kt:137-143`) |
|---|---|---|---|
| `None` | `"tracker"` (**defect**, see below) | `None` | `pref_player_torrents_proxy_mode_none` ("None") |
| `Tracker` | `"tracker"` | `Tracker` | `pref_player_torrents_proxy_mode_tracker` ("Tracker") |
| `Peers` | `"peers"` | `Peers` | `pref_player_torrents_proxy_mode_peers` ("Peers") |
| `Full` | `"full"` | `Full` | `pref_player_torrents_proxy_mode_full` ("Full") |

Defect: `None` and `Tracker` both send `"tracker"`. `TorrentServerService.kt:70-76` sends `proxyMode.value` for every choice and sends an empty proxy URL when the mode is `None`. Rust proposal: `ProxyMode::None` sends no proxy-mode field at all (open question OQ-13a-3 in p1-edge-cases-and-tests.md; verify against the TorrServer build before shipping).

### 2.2 ImageUtil.ImageType (`ImageUtil.kt:106-114`)

| Constant | MIME | Extension (file name) | Sniffed `Format` code |
|---|---|---|---|
| `AVIF` | `image/avif` | `avif` | 5 |
| `GIF` | `image/gif` | `gif` | 3 |
| `HEIF` | `image/heif` | `heif` | 4 |
| `JPEG` | `image/jpeg` | `jpg` (not `jpeg`) | 0 |
| `JXL` | `image/jxl` | `jxl` | 6 |
| `PNG` | `image/png` | `png` | 1 |
| `WEBP` | `image/webp` | `webp` | 2 |

The `Format` codes are the integers returned by the native decoder (`library/src/main/java/tachiyomi/decoder/ImageType.kt` of `tachiyomiorg/image-decoder`: `0 -> Jpeg, 1 -> Png, 2 -> Webp, 3 -> Gif, 4 -> Heif, 5 -> Avif, 6 -> Jxl`; any other value is an error).

### 2.3 ImageUtil.Side (`ImageUtil.kt:196-199`)

`RIGHT`, `LEFT`. Meaning for `splitInHalf`: the side of the spread that is kept. For `splitAndMerge`: the side placed on top (behaviour 5.9).

### 2.4 Format detection table (native decoder, exact bytes)

Source: `library/src/main/cpp/decoder_headers.h` and `java_wrapper.cpp` (`nativeFindType`) in `tachiyomiorg/image-decoder` (commit `41c059e540`, pinned in `gradle/libs.versions.toml:52`).

| Format | Test | Animated flag |
|---|---|---|
| Jpeg | `b[0]=0xFF, b[1]=0xD8, b[2]=0xFF` | false |
| Png | `b[0]=0x89, b[1..3]="PNG"` | false |
| Webp | `b[0..3]="RIFF"`; animated flag from the WebP header (false when the header cannot be parsed) | parsed |
| Gif | `b[0..3]="GIF8"` | always true |
| Jxl | container `00 00 00 0C 4A 58 4C 20 0D 0A 87 0A` at bytes 0..11, or codestream `FF 0A` at bytes 0..1 | false |
| Heif | `b[4..7]="ftyp"` and a brand `hei?` (`h`,`e`,`i` or `v`) at an offset `o`, `o` = 8, 12, ..., `<= min(headerSize, 32) - 4`, where `headerSize` = big-endian `b[0..3]` | false |
| Avif | same as Heif but brand `avi` | false |

Order of tests: Jpeg, Png, Webp, Gif, Jxl, then ftyp (Heif/Avif). The Kotlin always reads 32 bytes; `findType` returns `null` when no test matches.

### 2.5 NaturalComparator and Collator modes

- Natural comparator: `CaseInsensitiveSimpleNaturalComparator` (`net.greypanther.natsort`, library `com.github.gpanther:java-nat-sort:natural-comparator-1.1`, `gradle/libs.versions.toml:54`). Full algorithm in behaviour 2.3.
- Collator: `java.text.Collator`, locale `Locale.getDefault()`, strength `PRIMARY` (behaviour 2.4).

### 2.6 TriState and CheckboxState

- `TriState` (`TriState.kt`): `DISABLED`, `ENABLED_IS`, `ENABLED_NOT`. Not persisted in this area (filters persist elsewhere).
- `CheckboxState.State`: `Checked`, `None`. `CheckboxState.TriState`: `Include`, `Exclude`, `None`.

### 2.7 Read-only and temp file naming

- Java `File.createTempFile(prefix, null, dir)` produces `<prefix><random long>.tmp` (behaviour 3.11).

## 3. Data models

### 3.1 Torrent (`core/common/src/main/java/aniyomi/core/common/torrent/model/Torrent.kt`, class at line 7)

| Field (Kotlin) | JSON name | Type | Nullable | Default | Notes |
|---|---|---|---|---|---|
| `title` | `title` | String | no | (required) | Torrent name. |
| `hash` | `hash` | String | yes | `null` | 40 lowercase hex characters (SHA-1 of the info dictionary). |
| `torrentSize` | `torrent_size` | Long | yes | `null` | Total bytes. |
| `trackers` | `trackers` | List of String | yes | `null` | Tracker URLs. |
| `fileStats` | `file_stats` | List of FileStats | yes | `null` | File list. |

### 3.2 FileStats (`Torrent.kt`, class at line 18)

| Field | JSON | Type | Nullable | Default | Notes |
|---|---|---|---|---|---|
| `id` | `id` | Int | yes | `null` | 1-based for multi-file torrents; `null` for a single-file torrent parsed from a .torrent file (`TorrentHelpers.kt:39`). |
| `path` | `path` | String | no | required | `/`-joined path inside the torrent. For a single-file torrent parsed from a file it equals the torrent name. |
| `length` | `length` | Long | no | required | Bytes. |

### 3.3 TorrentRequest (`core/common/src/main/java/aniyomi/core/common/torrent/model/TorrentRequest.kt`, class at line 7)

| Field | JSON | Type | Default | Serialised when |
|---|---|---|---|---|
| `action` | `action` | String | required | always (`"add"` in the code) |
| `hash` | `hash` | String | `""` | only when not `""` (encodeDefaults is off) |
| `link` | `link` | String | `""` | only when not `""` |
| `title` | `title` | String | `""` | only when not `""` |
| `poster` | `poster` | String | `""` | only when not `""` |
| `data` | `data` | String | `""` | only when not `""` |
| `saveToDb` | `save_to_db` | Boolean | `false` | only when `true` |

Encoder settings (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:174-177`): `ignoreUnknownKeys = true`, `explicitNulls = false`, `encodeDefaults` left at `false`. Example body for `addTorrent("magnet:?xt=...", "Ep 1", "", "", false)`: `{"action":"add","link":"magnet:?xt=...","title":"Ep 1"}`.

Rust proposal: `#[serde(default, skip_serializing_if = "String::is_empty")]` on the string fields and `skip_serializing_if = "std::ops::Not::not"` on `save_to_db`. The `action` field is always written.

### 3.4 Source-API torrent model (consumer)

`TorrentUtils.kt:63-72` constructs `TorrentInfo(title, files, hash, size, trackers)` and `TorrentFile(path, id, length, hash, trackers)`. The model classes are in the source-api package (outside this area). Field types inferred from the call: `TorrentInfo.title: String`, `files: List<TorrentFile>`, `hash: String`, `size: Long`, `trackers: List<String>`; `TorrentFile.path: String`, `id: Int`, `length: Long`, `hash: String`, `trackers: List<String>`. Verify against `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/model/` before porting.

### 3.5 Bencode value model (`bencode/BencodeValue.kt`, variants at lines 9, 11, 54, 56)

| Variant | Payload | Notes |
|---|---|---|
| `Integer` | `Long` | from `i...e` |
| `ByteString` | `ByteArray` | equality by content; ordering by signed-byte comparison in the source (defect, behaviour 6.1) |
| `List` | `List<BencodeValue>` | from `l...e` |
| `Dictionary` | `SortedMap<ByteString, BencodeValue>` | keys unique and sorted; `getByString(key)` = lookup of `ByteString.fromUTF8String(key)` |

### 3.6 ArchiveEntry (`core/archive/src/main/kotlin/mihon/core/archive/ArchiveEntry.kt`)

`name: String` (UTF-8 path inside the archive, as stored), `isFile: Boolean` (`filetype == AE_IFREG`). Directories and links have `isFile = false`.

### 3.7 SplitData (`ImageUtil.kt:309-316`)

`index: Int` (0-based), `topOffset: Int`, `splitHeight: Int`, `splitWidth: Int` (equals the source width), derived `bottomOffset = topOffset + splitHeight`.

### 3.8 Preference handle shape (`Preference.kt:7-23`)

Methods: `key(): String`, `get(): T`, `set(value: T)`, `isSet(): Boolean`, `delete()`, `defaultValue(): T`, `changes(): Flow<T>`, `stateIn(scope): StateFlow<T>`. Rust: `Pref<T>` with the same methods, `changes()` returning `watch::Receiver<T>`.

### 3.9 Chapter page filename (consumer of the byte budget)

Built by `ReaderViewModel` (behaviour 2.5): `buildValidFilename(takeBytes("<manga title> - <chapter name>", 250 - byteSize(" - <page number>"))) + " - <page number>"`. The page number is `page.number` (1-based). The extension is added by the caller: one of `avif`, `gif`, `heif`, `jpg`, `jxl`, `png`, `webp`, chosen by the detected kind (section 2.2).

### 3.10 Download page filename prefix (consumer of tall-image splitting)

`MangaDownloader.kt:588`: `filenamePrefix = "%03d".format(Locale.ENGLISH, page.number)` (for example `007`). Split files: `007__001.jpg`, `007__002.jpg`, ... (`ImageUtil.kt:271-274`).

Downloader completeness check (`MangaDownloader.kt:618-628`, consumer): files named `COMIC_INFO_FILE` and `NOMEDIA_FILE` are ignored; names ending `.tmp` are ignored; names containing `__` and not ending in `__001.jpg` are ignored; every other file counts once.

## 4. Files, folders and formats on disk

| Item | Path or pattern (Aniyomi) | Created by | Format | Linux proposal (kuro-platform `Paths`) |
|---|---|---|---|---|
| Storage root | `<external storage>/Aniyomi` (`AndroidStorageFolderProvider.kt:14-20`), URL form `file:///storage/emulated/0/Aniyomi` | `FolderProvider` | directory | `Paths::downloads_default()` = `$XDG_DOWNLOAD_DIR/Kurobayomi` (fallback `~/Downloads/Kurobayomi`); user can pick another folder with the GTK folder chooser |
| Chapter temp dir | `<downloads>/<manga>/<chapter>_tmp` (suffix `TMP_DIR_SUFFIX = "_tmp"`, `MangaDownloader.kt:766`) | downloader | directory | same pattern under the storage root |
| CBZ output | `<manga>/<chapter>.cbz` (written as `<chapter>.cbz_tmp` first, `MangaDownloader.kt:639,645`) | `ZipWriter` | ZIP, stored | same |
| No-media marker | `<chapter dir>/.nomedia`, empty | `DiskUtil.createNoMediaFile` | empty file | same |
| Split page parts | `<chapter dir>/NNN__MMM.jpg` (`NNN` = page number padded to 3, `MMM` = part number from 001) | `ImageUtil.splitTallImage` | JPEG quality 100 | same |
| Image temp file | `<tmp>/<prefix padded to 3><random>.tmp` | `UniFileTempFileManager` | arbitrary | `<cache>/tmp` (`Paths::cache()`) |
| Shared image dir | `<app cache>/shared_image` (`FileExtensions.kt:35-36`) | sharing images | files | dropped (XDG portal share, T-13a-055) |
| Extension install copy | installed extension file, read-only (`copyAndSetReadOnlyTo`) | extension loaders | APK or JAR (not in this area) | `<data>/extensions/`, mode `0444` after copy (behaviour 3.8) |
| TorrServer data | `filesDir.absolutePath` passed as `path` to `TorrServer.startServer` (`TorrentServerService.kt:73`) | torrent service | TorrServer's database | `Paths::data()/torrserver` |
| Archive inputs | `.cbz`, `.zip`, `.cbr`, `.cb7`, `.epub`, and any libarchive format (`ArchiveInputStream.kt:21-22`) | user | libarchive | same set (see topic file) |

Notes
- The Kotlin folder names use the app name `Aniyomi`; Kurobayomi uses `Kurobayomi`. A user who points Kurobayomi at an existing Aniyomi folder keeps the existing files (no rename); only new downloads go to the new root.
- `.cbz` naming is `"$dirname.cbz"` where `dirname` is the chapter directory name (`MangaDownloader.kt:639`).

## 5. Network endpoints (TorrServer, local)

Base URL: `http://127.0.0.1:<port>` (`TorrentServerApi.kt:22-23`). Loopback only, no authentication, no TLS. Port is the value from `pref_torrserver_port` (or the port returned by `startServer`).

| Method | Path | Request | Response | Client code |
|---|---|---|---|---|
| GET | `/echo` | none | text body (any 2xx body); used only as an "alive" probe | `TorrentServerApi.kt:36-43`; `""` on any exception |
| POST | `/torrents` | `application/json`, `TorrentRequest` (action `"add"`) | `application/json`, `Torrent` | `TorrentServerApi.kt:46-70` |
| POST | `/torrent/upload` | `multipart/form-data`: part `file` (filename = title, `application/x-bittorrent`), part `save` (`"true"` or `"false"`), part `title` | `application/json`, `Torrent` | `TorrentServerApi.kt:72-95` |
| GET | `/stream/{name}?link={hash}&index={n}&play` | none; `name` is `URLEncoder.encode(fileName, "utf8")` | media stream (consumed by mpv, not by the app) | `TorrentServerUtils.kt:21-27` |

Request/response errors: non-2xx responses throw through `awaitSuccess` (area:12-network-stack). The app treats a thrown exception on `add` (non-magnet path is not used) as `DisabledTorrServerException` for magnets (`TorrentUtils.kt:28-33`) and lets it propagate in playback (`PlayerActivity.kt:1185`).

Not part of the TorrServer HTTP API (native calls in the Android AAR, `xyz.secozzi.torrserver.TorrServer`, version `0.1.1`, `gradle/aniyomi.versions.toml:9,20`):
- `TorrServer.startServer(port: String, path: String, proxyMode: String, proxyUrl: String): Int` (returns the bound port, or `-1`).
- `TorrServer.stopServer()`.
- `TorrServer.addTrackers(trackers: String)` (comma and newline separated list).
- `TorrServer.registerLogCallback()` (only when verbose network logging is on, `TorrentServerService.kt:66-68`).

These signatures are inferred from the call sites in this repository; the AAR source is not in the repository. OQ-13a-1 in p1-edge-cases-and-tests.md.

Also consumed (other area): `.torrent` files and HTTP(S) links are fetched with the app HTTP client when a source supplies a .torrent URL (`TorrentUtils.kt:37-39`).

## 6. Default tracker list (exact value)

Source: `TorrentPreferences.kt:15-40`. The raw literal is a triple-quoted string with indentation; `.replace(" ", "")` removes **every space character** (indentation only, no tracker contains a space). The result has 26 lines separated by `\n` with no trailing newline (967 characters). Verified by running the same transform over the source text while preparing this plan.

```
http://nyaa.tracker.wf:7777/announce
http://anidex.moe:6969/announce
http://tracker.anirena.com:80/announce
udp://tracker.uw0.xyz:6969/announce
http://share.camoe.cn:8080/announce
http://t.nyaatracker.com:80/announce
udp://47.ip-51-68-199.eu:6969/announce
udp://9.rarbg.me:2940
udp://9.rarbg.to:2820
udp://exodus.desync.com:6969/announce
udp://explodie.org:6969/announce
udp://ipv4.tracker.harry.lu:80/announce
udp://open.stealth.si:80/announce
udp://opentor.org:2710/announce
udp://opentracker.i2p.rocks:6969/announce
udp://retracker.lanta-net.ru:2710/announce
udp://tracker.cyberia.is:6969/announce
udp://tracker.dler.org:6969/announce
udp://tracker.ds.is:6969/announce
udp://tracker.internetwarriors.net:1337
udp://tracker.openbittorrent.com:6969/announce
udp://tracker.opentrackr.org:1337/announce
udp://tracker.tiny-vps.com:6969/announce
udp://tracker.torrent.eu.org:451/announce
udp://valakas.rollo.dnsabr.com:2710/announce
udp://www.torrent.eu.org:451/announce
```

Tracker list as pushed to the server (`TorrentServerUtils.kt:14-15`): the lines joined with `,\n` (comma then newline between entries, no trailing comma).

## 7. Message keys and English text used in this area

Keys follow the moko-resources naming (`MR.strings.*` for `i18n/`, `AYMR.strings.*` for `i18n-aniyomi/`). English text is from the base `strings.xml` files.

| Key | Namespace | English text (exact) | Used at |
|---|---|---|---|
| `app_name` | MR | `Aniyomi` (not translatable) | `AndroidStorageFolderProvider.kt:18`, `TorrentServerService.kt:117` |
| `torrserver_disabled` | AYMR | `TorrServer is not running` | `DisabledTorrServerException` display (`ExceptionFormatter.kt:19`) |
| `torrentserver_is_running` | AYMR | `Torrent Server is running` | torrent server notification text (`TorrentServerService.kt:116`) |
| `pref_player_torrents` | AYMR | `Torrent` | settings group title |
| `pref_player_torrents_enable` | AYMR | `Enable TorrServer` | switch |
| `pref_player_torrents_notice` | AYMR | `Legal Notice` | notice dialog title |
| `pref_player_torrents_notice_text` | AYMR | `TorrServer relies on peer to peer file sharing to access content. By doing so, you also upload files to others. Any content you share is your sole responsibility.` | notice dialog |
| `pref_player_torrents_notice_footer` | AYMR | `No further notices will be issued.` | notice dialog |
| `pref_player_torrents_port` | AYMR | `TorrServer port` | port row |
| `pref_player_torrents_port_summary` | AYMR | `Enter 0 to let system choose a port` | port dialog subtitle |
| `pref_player_torrents_port_error` | AYMR | `Port must be a number between 0 and 65535` | port validation error |
| `pref_player_torrents_trackers` | AYMR | `Torrent trackers` | trackers row |
| `pref_player_torrents_trackers_reset` | AYMR | `Reset default torrent trackers string` | reset row |
| `pref_player_torrents_proxy_mode` | AYMR | `Proxy mode` | list row |
| `pref_player_torrents_proxy_mode_none` | AYMR | `None` | list entry |
| `pref_player_torrents_proxy_mode_tracker` | AYMR | `Tracker` | list entry |
| `pref_player_torrents_proxy_mode_peers` | AYMR | `Peers` | list entry |
| `pref_player_torrents_proxy_mode_full` | AYMR | `Full` | list entry |
| `pref_player_torrents_proxy_url` | AYMR | `Proxy url` | proxy row |
| `pref_player_torrents_proxy_url_dialog` | AYMR | `Supports http, https, socks4, socks4a, socks5, socks5h` | proxy dialog subtitle |
| `pref_player_torrents_proxy_url_invalid_uri` | AYMR | `Invalid uri` | proxy validation error |
| `pref_player_torrents_proxy_url_invalid_protocol` | AYMR | `Unsupported protocol: "%s"` | proxy validation error with scheme argument |
| `http_server_start_failure` | AYMR | `Failed to start http server` | toast when the local HTTP source server fails (`PlayerActivity.kt:1116`), outside this area |
| `split_tall_images` | MR | `Split tall images` | download setting title |
| `split_tall_images_summary` | MR | `Improves reader performance` | download setting summary |
| `pref_hardware_bitmap_threshold` | MR | `Custom hardware bitmap threshold` | dropped setting (T-13a-030) |
| `pref_hardware_bitmap_threshold_default` | MR | `Default (%d)` | dropped setting |
| `pref_hardware_bitmap_threshold_summary` | MR | `If reader loads a blank image incrementally reduce the threshold.\nSelected: %s` | dropped setting |
| `download_notifier_split_page_not_found` | MR | `Page %d not found while splitting` | error when a split page file is missing (`MangaDownloader.kt:590`), logged |
| `action_ok` | MR | `OK` | notice dialog button |
| `pref_always_decode_long_strip_with_ssiv_2` | MR | `Use legacy decoder for long strip reader` | reader setting, outside this area |

Hard-coded strings in this area (not in resources; the port must localise them): the torrent notification action label `"Stop"` (`TorrentServerService.kt:124`). The Rust port uses the resource key `torrent_server_stop` with English text `Stop`.

## 8. Database

This area owns no tables and no SQL. The Kotlin `DiskUtil`, archive and image code have no persistence. The torrent preferences are settings rows owned by area:02-preferences-system. The chapter and page caches that use `DiskUtil.hashKeyForDisk` store files, not rows (`ChapterCache.kt`, `MangaCoverCache.kt`).

## 9. Migration notes (Aniyomi data to Kurobayomi)

| Item | Action | Task |
|---|---|---|
| Torrent keys (6) | keep names and default values; `pref_torrserver_tackers` keeps the misspelling | T-13a-042 |
| `pref_torrserver_proxymode` | read enum name; `None` value handled per section 2.1 | T-13a-043 |
| `pref_hardware_bitmap_threshold` | ignore; do not migrate | T-13a-030 |
| `__PRIVATE_*` keys | keep prefix; do not export into backups without consent | T-13a-005 |
| `__APP_STATE_*` keys | keep prefix; never export | T-13a-005 |
| `split_tall_images` | keep (Boolean) | T-13a-028 |
| Folder names `Aniyomi` | new downloads go to the Kurobayomi root; existing folders are read as they are | T-13a-015 |
| `.cbz` and split page names | identical; archive readers accept the old names | T-13a-031, T-13a-034 |

## 10. Constants (single list)

| Constant | Value | Source |
|---|---|---|
| `NOMEDIA_FILE` | `.nomedia` | `DiskUtil.kt:251` |
| `MAX_FILE_NAME_BYTES` | `250` | `DiskUtil.kt:254` |
| component truncation | `240` UTF-16 units | `DiskUtil.kt:235` |
| `SAFE_TEXTURE_LIMIT` | `2048` | `GLUtil.kt:50` |
| texture option step | `1024` | `GLUtil.kt:65` |
| `MINIMUM_WEBVIEW_VERSION` | `118` | `WebViewUtil.kt:116` |
| low RAM threshold | `3 * 1024^3` bytes | `DeviceUtil.kt:199` |
| `TMP_DIR_SUFFIX` | `_tmp` | `MangaDownloader.kt:766`, `AnimeDownloader.kt:939` |
| `COMIC_INFO_FILE` | `ComicInfo.xml` | `core-metadata/src/main/java/tachiyomi/core/metadata/comicinfo/ComicInfo.kt:9` (used by the downloader, not in scope) |
| image sniff length | `32` bytes | `ImageUtil.kt:90` |
| JPEG quality | `100` | `ImageUtil.kt:145,155,192,249` |
| background min size | `50` x `50` | `ImageUtil.kt:345` |
| background sample rows | `25` | `ImageUtil.kt:443` |
| tall-image ratio | `H / W > 3` (integer division) | `ImageUtil.kt:208` |
| bencode max string length | `Int.MAX_VALUE` | `BencodeParser.kt:53` |
| port range | `0..65535` | `PlayerSettingsTorrentScreen.kt:98` |
| torrent readiness polls | `10` (wait(10)) | `TorrentServerService.kt:79,153` |
| torrent echo poll interval | `1 s` | `TorrentServerService.kt:179` |
| default torrent port | `"8090"` | `TorrentPreferences.kt:12` |
| `TorrentRequest` action | `"add"` | `TorrentServerApi.kt:55` |
| upload content type | `application/x-bittorrent` | `TorrentServerApi.kt:78` |
| Copy buffer | `8192` bytes | `FileExtensions.kt:52`, `UniFileTempFileManager.kt:78` |
| Direct archive buffer | `8192` bytes | `ZipWriter.kt:17` |
| Java temp suffix | `.tmp` | `UniFileTempFileManager.kt:67` (File.createTempFile default) |
| Temp prefix minimum | `3` characters (padded with spaces) | `UniFileTempFileManager.kt:68` |

## 11. Test fixtures (for the port)

Both fixtures are in `core/common/src/test/resources/aniyomi/core/common/torrent/` (outside the scope list, referenced by the test row in p1-inventory.md). Verified while writing this plan:

| File | Bytes | Info name | Info hash (SHA-1 of re-encoded info, = raw slice for these files) | Length or files | Tracker list (announce, then announce-list flattened) |
|---|---|---|---|---|---|
| `test-torrent-file.torrent` | 218 | `test-torrent-file` | `104701de4b710ee9720ee249bf220384659e8c38` | `length` 18 | `udp://tracker.opentrackr.org:1337/announce` |
| `test-torrent-directory.torrent` | 570 | `test-torrent-directory` | `863eb76f1cd0643d9fd2eb4a7da866c3752991b5` | `files`: `[3,3.1]` 4 bytes, `[2]` 2, `[1]` 2, `[3,3.2]` 4; total 12 | `udp://tracker.opentrackr.org:1337/announce`, then `udp://tracker.opentrackr.org:1337/announce`, `https://tracker.tamersunion.org:443/announce`, `http://tracker.ipv6tracker.org:80/announce`, `udp://opentracker.io:6969/announce`, `udp://tracker.torrent.eu.org:451/announce` (announce-list tiers flattened, duplicates kept) |

Expected `FileStats` for the directory fixture (from the test): `(1, "3/3.1", 4)`, `(2, "2", 2)`, `(3, "1", 2)`, `(4, "3/3.2", 4)`. Single-file expectation: `FileStats(null, "test-torrent-file", 18)`.

## 12. Linux `Paths` mapping (for kuro-platform, per the cross-platform rule)

| Trait method (kuro-platform) | Linux source | Used by |
|---|---|---|
| `data()` | `$XDG_DATA_HOME/kurobayomi` (fallback `~/.local/share/kurobayomi`) | TorrServer data (`torrserver/`), extension copies (`extensions/`) |
| `cache()` | `$XDG_CACHE_HOME/kurobayomi` (fallback `~/.cache/kurobayomi`) | temp files (`tmp/`), shared images (dropped), chapter cache |
| `config()` | `$XDG_CONFIG_HOME/kurobayomi` | settings file if area 02 uses one |
| `downloads_default()` | `$XDG_DOWNLOAD_DIR/Kurobayomi` (fallback `~/Downloads/Kurobayomi`) | storage root (`FolderProvider`) |

Only kuro-platform may read `XDG_*` variables (rule 1 of `plans/addenda/requirements-cross-platform.md`). Path building in this area uses `PathBuf::join`, never string concatenation of separators (rule 4).

## 13. Sizes and quantities worth knowing

- Default tracker list: 26 entries, 967 characters.
- `ProxyMode`: 4 entries. `ImageType`: 7 entries. Native `Format`: 7 entries.
- Torrent preferences: 6 keys.
- Torrent endpoints: 3 (plus the stream URL).
- Message keys listed in section 7: 31 (plus the hard-coded `"Stop"`).
- Image split part count for a page: `((H - 1) / (2 * maxDisplayDim)) + 1`; for a 1080 px maximum dimension display and a 10,000 px page, `((10000 - 1) / 2160) + 1 = 5` parts.
