# Area 02 preferences system, part 1: data models, keys, files, formats

Companion files: `p1-topic-key-registry.md` (every key with default), `p1-behavior.md` (algorithms), `p1-rust-design.md` (types). Paths are repo-relative to `/home/blackbox/aniyomi-src` unless they start with `$XDG`.

## 1. Where preferences live

### 1.1 OBSERVED (Android)

| Store | Backing | Location | Keys |
|---|---|---|---|
| main store | `SharedPreferences` default file (`PreferenceManager.getDefaultSharedPreferences(context)`) | `/data/data/<package>/shared_prefs/<package>_preferences.xml` | all 328 keys of the 18 classes, in one flat namespace; three logical groups by key prefix (normal, `__APP_STATE_`, `__PRIVATE_`) |
| per-source store | `getSharedPreferences("source_<id>", MODE_PRIVATE)` | `/data/data/<package>/shared_prefs/source_<id>.xml` (id is the Long source id) | keys chosen by the extension |
| backup | protobuf (`kotlinx.serialization.protobuf`) gzip-compressed `.tachibk` | user chosen | `Backup.backupPreferences` (ProtoNumber 104) and `Backup.backupSourcePreferences` (ProtoNumber 105) |

SharedPreferences primitive kinds and how Aniyomi uses them: `String`, `Long`, `Int`, `Float`, `Boolean`, `Set<String>`; non-primitives are stored as `String` (enums by `name`; `LibraryDisplayMode` and library sorts via their string codecs, see `p1-behavior.md` section 3).

### 1.2 PROPOSAL (Linux)

| File | Content | Format | Included in backup | Mode |
|---|---|---|---|---|
| `$XDG_CONFIG_HOME/kurobayomi/config.toml` (fallback `~/.config/kurobayomi/config.toml`) | every normal-namespace key | TOML, one table per section (below) | yes | 0600 |
| `$XDG_STATE_HOME/kurobayomi/state.toml` (fallback `~/.local/state/kurobayomi/state.toml`) | every app-state key | TOML, one table per section | never | 0600 |
| Secret Service collection `login`, items with attributes `application=kurobayomi`, `kind=tracker-username\|tracker-password\|tracker-token`, `tracker=<id>` | private strings | libsecret item (via `kuro-platform::SecretStore`) | only with "Private settings" | managed by keyring |
| `$XDG_CONFIG_HOME/kurobayomi/sources/source_<id>.toml` | per-source preferences | TOML (typed values, section 5) | yes (the "Source settings" backup option) | 0600 |
| `$XDG_CONFIG_HOME/kurobayomi/mpv/mpv.conf`, `input.conf`, `scripts/`, `fonts/`, `script-opts/`, `shaders/` | mpv user files generated from the `pref_mpv_*` text prefs and user-provided folders | plain text | `mpv.conf` and `input.conf` as the two text prefs | 0644 |

All paths come from the `Paths` trait of `kuro-platform` (cross-platform requirement, `plans/addenda/requirements-cross-platform.md`). No other crate formats these paths.

File-level rules (Proposal):

* Encoding UTF-8; atomic replace (write `config.toml.tmp` in the same directory, `fsync`, `rename`); keep one previous copy `config.toml.bak` created at start when the file parsed successfully.
* Every field is `#[serde(default)]`; unknown tables/keys are preserved on write (kept in a `toml_edit::DocumentMut`, only known keys are touched) so a newer config survives a downgrade.
* A file that fails to parse is renamed `config.toml.corrupt-<unix seconds>` and the app starts with defaults, showing the one-time message "Settings file was unreadable; defaults restored (old file kept)". A single wrong-typed value never fails the whole file: it follows the Aniyomi rule (delete key, use default).
* Schema version: top-level integer `schema = 1` in both files, used by area:11-app-migrations equivalents (task T-02a-036). Aniyomi's counterpart is the app-state Int `last_version_code` (`App.kt:169`).

## 2. TOML schema

Every one of the 328 keys has exactly one field. The complete default files (every field with its default value) are generated from the Kotlin sources in `p1-topic-default-config.md`: section "config.toml" (309 fields in 18 tables, plus the 2 per-tracker filter maps) and section "state.toml" (13 fields in 6 tables); 4 more keys (three tracker secrets and the tracker auth-expired flag map) live in the keyring and state file respectively. Key strings for each field are in `p1-topic-key-registry.md`.

Section table (Rust sub-struct, Kotlin class it comes from, number of fields in that table in the default file):

| TOML table | Kotlin class | File | Notes |
|---|---|---|---|
| `[base]` | `BasePreferences` | config (4 fields), state (3 fields) | `extension_installer`, `display_profile`, `hardware_bitmap_threshold`, `always_decode_long_strip_with_ssiv` in config; `downloaded_only`, `incognito_mode`, `shown_onboarding_flow` in state |
| `[ui]` | `UiPreferences` | config | 8 fields |
| `[sources]`, `[sources.data_saver]` | `SourcePreferences` | config (non-state), state (`trusted_extensions`, `global_search_filter_state`, `last_used_anime_source`, `last_used_manga_source`) | data-saver keys are in the sub-table |
| `[tracking]` | `TrackPreferences` | config (switches), state (`track_auth_expired` map), keyring (username, password, token) | |
| `[library]` | `LibraryPreferences` | config; state (`last_updated_timestamp`, `last_used_anime_category`, `last_used_manga_category`) | contains the nine plain filters, two tracked-filter maps, all `default_*` templates and season fields |
| `[downloads]` | `DownloadPreferences` | config | |
| `[backup]` | `BackupPreferences` | config (`backup_interval`, dead `backup_flags`), state (`last_auto_backup_timestamp`) | |
| `[storage]` | `StoragePreferences` | state | |
| `[security]` | `SecurityPreferences` | config; state (`last_app_closed`) | |
| `[network]` | `NetworkPreferences` | config | |
| `[torrent]` | `TorrentPreferences` | config | |
| `[reader]` | `ReaderPreferences` | config | |
| `[player]`, `[player.gestures]`, `[player.decoder]`, `[player.audio]`, `[player.subtitles]`, `[player.advanced]` | `PlayerPreferences`, `GesturePreferences`, `DecoderPreferences`, `AudioPreferences`, `SubtitlePreferences`, `AdvancedPlayerPreferences` | config | `playerStatisticsPage` and the per-session values (`player_speed`, `aspect_state`, subtitle styling) are normal keys in Aniyomi and so are backed up |

State file extras (not preference keys): top-level `schema = 1` and `last_version_code` (Int, mirrors `Preference.appStateKey("last_version_code")` in `App.kt:169`, used by the migration runner of area:11-app-migrations).

## 3. Integer bit layouts that preference values feed

All `Long` values below are stored in preferences exactly as in the per-entry DB columns (SQLite INTEGER = 64-bit). Source: `domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt:77-103`, `domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt:217-313`. Tables touched by these values: `mangas.chapter_flags` (`data/src/main/sqldelight/data/mangas.sq:22`), `animes.episode_flags` (`.../sqldelightanime/dataanime/animes.sq:24`), `animes.season_flags` (`animes.sq:35`), `mangas.viewer`/`animes.viewer` (reading mode, orientation, see 3.4), `categories.flags` (`categories.sq:5`, sort flags).

### 3.1 Manga `chapter_flags`

| Field | Mask | Values |
|---|---|---|
| sort direction | `CHAPTER_SORT_DIR_MASK` 0x1 | `CHAPTER_SORT_DESC` 0x0, `CHAPTER_SORT_ASC` 0x1 |
| unread filter | `CHAPTER_UNREAD_MASK` 0x6 | `SHOW_ALL` 0x0, `CHAPTER_SHOW_UNREAD` 0x2 (TriState ENABLED_IS), `CHAPTER_SHOW_READ` 0x4 (ENABLED_NOT) |
| downloaded filter | `CHAPTER_DOWNLOADED_MASK` 0x18 | 0x0, `CHAPTER_SHOW_DOWNLOADED` 0x8, `CHAPTER_SHOW_NOT_DOWNLOADED` 0x10 |
| bookmarked filter | `CHAPTER_BOOKMARKED_MASK` 0x60 | 0x0, `CHAPTER_SHOW_BOOKMARKED` 0x20, `CHAPTER_SHOW_NOT_BOOKMARKED` 0x40 |
| sort mode | `CHAPTER_SORTING_MASK` 0x300 | `CHAPTER_SORTING_SOURCE` 0x0, `CHAPTER_SORTING_NUMBER` 0x100, `CHAPTER_SORTING_UPLOAD_DATE` 0x200, `CHAPTER_SORTING_ALPHABET` 0x300 |
| title display | `CHAPTER_DISPLAY_MASK` 0x100000 | `CHAPTER_DISPLAY_NAME` 0x0, `CHAPTER_DISPLAY_NUMBER` 0x100000 |

### 3.2 Anime `episode_flags`

| Field | Mask | Values |
|---|---|---|
| sort direction | `EPISODE_SORT_DIR_MASK` 0x1 | `EPISODE_SORT_DESC` 0x0, `EPISODE_SORT_ASC` 0x1 |
| unseen filter | `EPISODE_UNSEEN_MASK` 0x6 | 0x0, `EPISODE_SHOW_UNSEEN` 0x2, `EPISODE_SHOW_SEEN` 0x4 |
| downloaded filter | `EPISODE_DOWNLOADED_MASK` 0x18 | 0x0, `EPISODE_SHOW_DOWNLOADED` 0x8, `EPISODE_SHOW_NOT_DOWNLOADED` 0x10 |
| bookmarked filter | `EPISODE_BOOKMARKED_MASK` 0x60 | 0x0, `EPISODE_SHOW_BOOKMARKED` 0x20, `EPISODE_SHOW_NOT_BOOKMARKED` 0x40 |
| fillermarked filter | `EPISODE_FILLERMARKED_MASK` 0x180 | 0x0, `EPISODE_SHOW_FILLERMARKED` 0x80, `EPISODE_SHOW_NOT_FILLERMARKED` 0x100 |
| sort mode | `EPISODE_SORTING_MASK` 0x600 | `EPISODE_SORTING_SOURCE` 0x0, `EPISODE_SORTING_NUMBER` 0x200, `EPISODE_SORTING_UPLOAD_DATE` 0x400, `EPISODE_SORTING_ALPHABET` 0x600 |
| thumbnails | `EPISODE_PREVIEWS_MASK` 0x800 | `EPISODE_SHOW_PREVIEWS` 0x0 (shown), `EPISODE_SHOW_NOT_PREVIEWS` 0x800 |
| summaries | `EPISODE_SUMMARIES_MASK` 0x1000 | `EPISODE_SHOW_SUMMARIES` 0x0 (shown), `EPISODE_SHOW_NOT_SUMMARIES` 0x1000 |
| title display | `EPISODE_DISPLAY_MASK` 0x100000 | `EPISODE_DISPLAY_NAME` 0x0, `EPISODE_DISPLAY_NUMBER` 0x100000 |

### 3.3 Anime `season_flags`

| Field | Mask | Values |
|---|---|---|
| sort direction | `SEASON_SORT_DIR_MASK` 0x1 | `SEASON_SORT_DESC` 0x0, `SEASON_SORT_ASC` 0x1 |
| downloaded filter | `SEASON_DOWNLOADED_MASK` 0x6 | 0x0, `SEASON_SHOW_DOWNLOADED` 0x2, `SEASON_SHOW_NOT_DOWNLOADED` 0x4 |
| unseen filter | `SEASON_UNSEEN_MASK` 0x18 | 0x0, `SEASON_SHOW_UNSEEN` 0x8, `SEASON_SHOW_SEEN` 0x10 |
| started filter | `SEASON_STARTED_MASK` 0x60 | 0x0, `SEASON_SHOW_STARTED` 0x20, `SEASON_SHOW_NOT_STARTED` 0x40 |
| completed filter | `SEASON_COMPLETED_MASK` 0x180 | 0x0, `SEASON_SHOW_COMPLETED` 0x80, `SEASON_SHOW_NOT_COMPLETED` 0x100 |
| bookmarked filter | `SEASON_BOOKMARKED_MASK` 0x600 | 0x0, `SEASON_SHOW_BOOKMARKED` 0x200, `SEASON_SHOW_NOT_BOOKMARKED` 0x400 |
| fillermarked filter | `SEASON_FILLERMARKED_MASK` 0x1800 | 0x0, `SEASON_SHOW_FILLERMARKED` 0x800, `SEASON_SHOW_NOT_FILLERMARKED` 0x1000 |
| sort mode | `SEASON_SORT_MASK` 0xE000 | `SEASON_SORT_SOURCE` 0x0, `SEASON_SORT_SEASON` 0x2000, `SEASON_SORT_UPLOAD` 0x4000, `SEASON_SORT_ALPHABET` 0x6000, `SEASON_SORT_COUNT` 0x8000, `SEASON_SORT_LAST_SEEN` 0xA000, `SEASON_SORT_FETCHED` 0xC000 |
| grid mode | `SEASON_GRID_DISPLAY_MODE_MASK` 0x30000, shift 16 | 0 CompactGrid, 1 ComfortableGrid, 2 CoverOnlyGrid, 3 List |
| grid size | `SEASON_GRID_DISPLAY_SIZE_MASK` 0x3C0000, shift 18 | 0 = auto; 0..15 representable |
| overlay downloaded | `SEASON_OVERLAY_DOWNLOADED_MASK` 0x400000 | bit set = on |
| overlay unseen | `SEASON_OVERLAY_UNSEEN_MASK` 0x800000 | bit set = on |
| overlay local | `SEASON_OVERLAY_LOCAL_MASK` 0x1000000 | bit set = on |
| overlay language | `SEASON_OVERLAY_LANG_MASK` 0x2000000 | bit set = on |
| overlay continue | `SEASON_OVERLAY_CONT_MASK` 0x4000000 | bit set = on |
| title display | `SEASON_DISPLAY_MODE_MASK` 0x8000000 | `SEASON_DISPLAY_MODE_SOURCE` 0x0, `SEASON_DISPLAY_MODE_NUMBER` 0x8000000 |

Anime `viewer_flags` masks (`Anime.kt:310-313`, not preference templates but listed because they sit next to the season constants): `ANIME_INTRO_MASK` 0xFF (bits 0..7, intro skip seconds), `ANIME_AIRING_EPISODE_MASK` 0xFFFF00 (bits 8..23, next airing episode), `ANIME_AIRING_TIME_MASK` 0x0FFFFFFFF000000 (bits 24..55, airing time seconds), `ANIME_INTRO_DISABLE_MASK` 0x100000000000000 (bit 56). `removeHexZeros(zeros)` = `value / 16^zeros` (floating `Double.pow`, truncated to Long): airing episode uses 2 zeros (>> 8), airing time uses 6 zeros (>> 24).

### 3.4 Reader value codes in preferences (Int)

| Preference | Codes |
|---|---|
| `pref_default_reading_mode_key` (`ReadingMode.flagValue`, mask `MASK` 0x7) | 0x0 DEFAULT, 0x1 LEFT_TO_RIGHT, 0x2 RIGHT_TO_LEFT, 0x3 VERTICAL, 0x4 WEBTOON, 0x5 CONTINUOUS_VERTICAL |
| `pref_default_orientation_type_key` (`ReaderOrientation.flagValue`, mask `MASK` 0x38) | 0x00 DEFAULT, 0x08 FREE, 0x10 PORTRAIT, 0x18 LANDSCAPE, 0x20 LOCKED_PORTRAIT, 0x28 LOCKED_LANDSCAPE, 0x30 REVERSE_PORTRAIT |

The per-manga `viewer` column stores `readingMode | orientation` in one Int using these masks (area:07-domain-entries-items).

### 3.5 Library sort flags

See `p1-behavior.md` section 3.3 for the full table. In short `categories.flags = type | direction` with type mask 0x3C and direction bit 0x40 (ascending).

## 4. Enum value tables (stored names)

Every enum is stored by its Kotlin constant name; the "stored name" column is the exact string.

| Enum (Rust name) | Stored names in declaration order | Default where used |
|---|---|---|
| `ThemeMode` | `LIGHT`, `DARK`, `SYSTEM` | `SYSTEM` |
| `AppTheme` | `DEFAULT`, `MONET`, `CLOUDFLARE`, `COTTONCANDY`, `DOOM`, `GREEN_APPLE`, `LAVENDER`, `MATRIX`, `MIDNIGHT_DUSK`, `MOCHA`, `SAPPHIRE`, `NORD`, `STRAWBERRY_DAIQUIRI`, `TAKO`, `TEALTURQUOISE`, `TIDAL_WAVE`, `YINYANG`, `YOTSUBA`, `MONOCHROME`, `DARK_BLUE`, `HOT_PINK`, `BLUE` | `DEFAULT` (Android: `MONET` when dynamic colour available) |
| `NavStyle` | `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE` | `MOVE_HISTORY_TO_MORE` |
| `StartScreen` | `ANIME`, `MANGA`, `UPDATES`, `HISTORY`, `BROWSE` | `ANIME` |
| `TabletUiMode` | `AUTOMATIC`, `ALWAYS`, `LANDSCAPE`, `NEVER` | `AUTOMATIC` |
| `ExtensionInstaller` | `LEGACY`, `PACKAGEINSTALLER`, `SHIZUKU`, `PRIVATE` | `PACKAGEINSTALLER` (`LEGACY` on MIUI) |
| `DataSaver` | `NONE`, `BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT` | `NONE` |
| `MigrateSortMode` (`SetMigrateSorting.Mode`) | `ALPHABETICAL`, `TOTAL` | `ALPHABETICAL` |
| `MigrateSortDirection` (`SetMigrateSorting.Direction`) | `ASCENDING`, `DESCENDING` | `ASCENDING` |
| `AutoTrackState` | `ALWAYS`, `ASK`, `NEVER` | `ALWAYS` |
| `TriState` | `DISABLED`, `ENABLED_IS`, `ENABLED_NOT` | `DISABLED` |
| `LibraryDisplayMode` (string codec, not enum names) | `COMPACT_GRID`, `COMFORTABLE_GRID`, `LIST`, `COVER_ONLY_GRID` | `COMPACT_GRID` |
| `EpisodeSwipeAction` | `ToggleSeen`, `ToggleBookmark`, `ToggleFillermark`, `Download`, `Disabled` | start `ToggleSeen`, end `ToggleBookmark` |
| `ChapterSwipeAction` | `ToggleRead`, `ToggleBookmark`, `Download`, `Disabled` | start `ToggleRead`, end `ToggleBookmark` |
| `SecureScreenMode` | `ALWAYS`, `INCOGNITO`, `NEVER` | `INCOGNITO` |
| `FlashColor` | `BLACK`, `WHITE`, `WHITE_BLACK` | `BLACK` |
| `TappingInvertMode` | `NONE`, `HORIZONTAL`, `VERTICAL`, `BOTH` | `NONE` |
| `ReaderHideThreshold` (threshold value) | `HIGHEST` 5, `HIGH` 13, `LOW` 31, `LOWEST` 47 | `LOW` |
| `PlayerOrientation` | `Free`, `Video`, `Portrait`, `ReversePortrait`, `SensorPortrait`, `Landscape`, `ReverseLandscape`, `SensorLandscape` | `SensorLandscape` |
| `VideoAspect` | `Crop`, `Fit`, `Stretch` | `Fit` |
| `SingleActionGesture` | `None`, `Seek`, `PlayPause`, `Switch`, `Custom` | per key (see registry) |
| `Debanding` | `None`, `CPU`, `GPU` | `None` |
| `AudioChannels` (mpv property, value) | `Auto` (`audio-channels`, `auto-safe`), `AutoSafe` (`audio-channels`, `auto`), `Mono` (`audio-channels`, `mono`), `Stereo` (`audio-channels`, `stereo`), `ReverseStereo` (`af`, `pan=[stereo\|c0=c1\|c1=c0]`) | `AutoSafe` |
| `SubtitlesBorderStyle` (mpv value) | `OutlineAndShadow` `outline-and-shadow`, `OpaqueBox` `opaque-box`, `BackgroundBox` `background-box` | `OutlineAndShadow` |
| `SubtitleJustification` (mpv value) | `Left` `left`, `Center` `center`, `Right` `right`, `Auto` `auto` | `Auto` |
| `ProxyMode` (wire value) | `None` `tracker`, `Tracker` `tracker`, `Peers` `peers`, `Full` `full` | `None` |
| `CustomKeyCodes` (not stored; sent to mpv) | `DoubleTapLeft` `0x10001`, `DoubleTapCenter` `0x10002`, `DoubleTapRight` `0x10003`, `MediaPrevious` `0x10004`, `MediaPlay` `0x10005`, `MediaNext` `0x10006` | n/a |
| `Decoder` (not stored by these classes) | `AutoCopy` `auto-copy`, `Auto` `auto`, `SW` `no`, `HW` `mediacodec-copy`, `HWPlus` `mediacodec` | n/a |

Integer-coded UI choices: see `p1-behavior.md` section 5.13 (reader) and the registry notes.

`ReaderPreferences` list tables (string-resource keys in value order): `TapZones` = `label_default`, `l_nav`, `kindlish_nav`, `edge_nav`, `right_and_left_nav`, `disabled_nav`; `ImageScaleType` = `scale_type_fit_screen`, `scale_type_stretch`, `scale_type_fit_width`, `scale_type_fit_height`, `scale_type_original_size`, `scale_type_smart_fit`; `ZoomStart` = `zoom_start_automatic`, `zoom_start_left`, `zoom_start_right`, `zoom_start_center`; `ColorFilterMode` = (`label_default`, SrcOver), (`filter_mode_multiply`, Modulate), (`filter_mode_screen`, Screen), then on API >= 28 (`filter_mode_overlay`, Overlay), (`filter_mode_lighten`, Lighten), (`filter_mode_darken`, Darken). On Linux all six are offered (Proposal: SrcOver = normal alpha blend, Modulate = multiply, Screen = screen; Overlay, Lighten, Darken map to `gsk::BlendMode::{Overlay, Lighten, Darken}` available in GTK 4.0+).

## 5. Typed value model (Rust)

```
enum PrefValue { Int(i32), Long(i64), Float(f32), String(String), Bool(bool), Set(BTreeSet<String>) }
```

* In TOML the six kinds are stored natively (`integer`, `float`, `string`, `boolean`, `array of string`); the Int/Long distinction is by the registry (typed field), not by TOML.
* In per-source files and in the generic "unknown key" bag the kind is explicit: `key = { t = "long", v = 5 }` with `t` in `int`, `long`, `float`, `string`, `bool`, `set`.
* Float is `f32` to match Kotlin `Float` (the progress pref `0.85F` is serialised as `0.85` and compared with `f32` epsilon `1e-6` in tests).
* Backup proto: `PreferenceValue` is a sealed class with six subclasses (`IntPreferenceValue(value: Int)`, `LongPreferenceValue(value: Long)`, `FloatPreferenceValue(value: Float)`, `StringPreferenceValue(value: String)`, `BooleanPreferenceValue(value: Boolean)`, `StringSetPreferenceValue(value: Set<String>)`) in `app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt`; `BackupPreference(key @ProtoNumber(1), value @ProtoNumber(2))`; `BackupSourcePreferences(sourceKey @ProtoNumber(1), prefs @ProtoNumber(2))`. Details and the polymorphic encoding in `p1-topic-backup-interop.md`.

## 6. Environment-dependent defaults

| Preference | Android | Linux proposal |
|---|---|---|
| `source_languages` | `{"all","en", Locale.getDefault().language}` | `{"all","en", <first two letters of $LC_ALL / $LC_MESSAGES / $LANG, lower case, skipping "C"/"POSIX">}`; read once at first run, then stored |
| `pref_app_theme` | MONET if dynamic colour available else DEFAULT | DEFAULT |
| `__APP_STATE_storage_dir` | `file:///storage/emulated/0/Aniyomi` (URI of `<external storage>/<app_name>`) | `Paths::default_storage_dir()` = `$XDG_DATA_HOME/kurobayomi` (plain path) |
| `verbose_logging` | `isDebugBuildType` | `cfg!(debug_assertions)` |
| `pref_hardware_bitmap_threshold` | 2048 | 2048 (inert) |

### 6.1 Default tracker list (`pref_torrserver_tackers`, 26 entries; spaces removed, newlines kept)

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

The Kotlin raw string is indented with 11 spaces on each continuation line; `.replace(" ", "")` removes them, leaving `\n` separators and no trailing newline. Proposal: store the default as a Rust `const DEFAULT_TRACKERS: &str` built with `concat!` and `\n` so it is byte-identical (test T-02a-021).

### 6.2 DoH provider ids (`doh_provider`; -1 off)

| Id | Constant | Endpoint (from `DohProviders.kt`) |
|---|---|---|
| -1 | (disabled) | system DNS |
| 1 | `PREF_DOH_CLOUDFLARE` | `https://cloudflare-dns.com/dns-query` |
| 2 | `PREF_DOH_GOOGLE` | `https://dns.google/dns-query` |
| 3 | `PREF_DOH_ADGUARD` | `https://dns-unfiltered.adguard.com/dns-query` |
| 4 | `PREF_DOH_QUAD9` | `https://dns.quad9.net/dns-query` |
| 5 | `PREF_DOH_ALIDNS` | `https://dns.alidns.com/dns-query` |
| 6 | `PREF_DOH_DNSPOD` | `https://doh.pub/dns-query` |
| 7 | `PREF_DOH_360` | `https://doh.360.cn/dns-query` |
| 8 | `PREF_DOH_QUAD101` | `https://dns.twnic.tw/dns-query` |
| 9 | `PREF_DOH_MULLVAD` | `https://dns.mullvad.net/dns-query` (the source string has a leading space, `" https://dns.mullvad.net/dns-query"`) |
| 10 | `PREF_DOH_CONTROLD` | `https://freedns.controld.com/p0` |
| 11 | `PREF_DOH_NJALLA` | `https://dns.njal.la/dns-query` |
| 12 | `PREF_DOH_SHECAN` | `https://free.shecan.ir/dns-query` |
| 13 | `PREF_DOH_LIBREDNS` | `https://doh.libredns.gr/dns-query` |

(Bootstrap IP lists per provider are in `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt` and belong to area:12-network-stack.)

## 7. Network endpoints touched by this scope

None. The scope only stores values. Indirect consumers: DoH endpoints above (area:12-network-stack), data-saver proxies (area:34-images-covers-cache), TorrServer on `127.0.0.1:<pref_torrserver_port>` (area:13-core-util-storage-archive-torrent).

## 8. Database tables touched

None written by this scope. Read/related: `categories.flags` (sort flags), `mangas.chapter_flags`, `animes.episode_flags`, `animes.season_flags`, `mangas.viewer`, `animes.viewer` (templates in section 3). The preference code never opens the DB.

## 9. Message keys referenced by enums in this scope

`MR.strings.*`: `label_default`, `theme_monet`, `theme_greenapple`, `theme_lavender`, `theme_midnightdusk`, `theme_nord`, `theme_strawberrydaiquiri`, `theme_tako`, `theme_tealturquoise`, `theme_tidalwave`, `theme_yinyang`, `theme_yotsuba`, `theme_monochrome`, `ext_installer_legacy`, `ext_installer_packageinstaller`, `ext_installer_shizuku`, `ext_installer_private`, `automatic_background`, `lock_always`, `landscape`, `lock_never`, `default_category_summary`, `pref_incognito_mode`, `label_recent_updates`, `label_recent_manga`, `browse`, `tapping_inverted_none`, `tapping_inverted_horizontal`, `tapping_inverted_vertical`, `tapping_inverted_both`, `l_nav`, `kindlish_nav`, `edge_nav`, `right_and_left_nav`, `disabled_nav`, `scale_type_*` (six), `zoom_start_*` (four), `filter_mode_multiply`, `filter_mode_screen`, `filter_mode_overlay`, `filter_mode_lighten`, `filter_mode_darken`, `rotation_free`, `rotation_portrait`, `rotation_reverse_portrait`, `rotation_landscape`. `AYMR.strings.*`: `theme_cloudflare`, `theme_cottoncandy`, `theme_doom`, `theme_matrix`, `theme_mocha`, `theme_sapphire`, `pref_bottom_nav_no_manga`, `pref_bottom_nav_no_updates`, `pref_bottom_nav_no_history`, `pref_bottom_nav_no_browse`, `label_anime`, `manga`, `pref_player_audio_channels_auto`, `pref_player_audio_channels_auto_safe`, `pref_player_audio_channels_mono`, `pref_player_audio_channels_stereo`, `pref_player_audio_channels_reverse_stereo`, `rotation_video`, `rotation_sensor_portrait`, `rotation_reverse_landscape`, `rotation_sensor_landscape`, `video_crop_screen`, `video_fit_screen`, `video_stretch_screen`, `single_action_none`, `single_action_seek`, `single_action_playpause`, `single_action_switch`, `single_action_custom`, `player_sheets_subtitles_border_style_outline_and_shadow`, `player_sheets_subtitles_border_style_opaque_box`, `player_sheets_subtitles_border_style_background_box`. English texts live in `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml` (area:51-i18n). Note `pref_player_audio_channels_auto_safe` = "Auto-safe" and `..._auto` = "Auto" (strings.xml lines 169-170).
