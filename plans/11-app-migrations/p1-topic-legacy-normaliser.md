# Area 11: legacy preference normaliser (Proposal)

OBSERVED context: Aniyomi never converts values at restore time. `PreferenceRestorer` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt`) writes each backed-up value to the store when the stored type matches (`prefs[key] is Boolean?`, `is Int?`, `is String?`, `is Long?`, `is Float?`, `is Set<*>?`) and only remaps category ids (`restoreCategoriesPreference`, lines 126-166 of that file). The 29 conversion steps of area:11-app-migrations run only on the device's own upgrade path. A backup taken from an old build therefore carries old-shaped values that Aniyomi tolerates because typed reads fall back to defaults.

Proposal: Kurobayomi starts with an empty profile, so those conversions would never run. But a user who imports an old Aniyomi or Tachiyomi backup (area:22-backup-restore) would silently lose sort modes, filters and similar settings. The normaliser is a pure function applied to the raw key/value map of a backup before area:02-preferences-system's importer maps keys to typed settings. It lives in `kuro_prefs::legacy::normalise` (the `legacy.rs` of `plans/02-preferences-system/p1-rust-design.md`, task T-02a-029 consumes it).

## 1. Interface

```rust
// crates/kuro-prefs/src/legacy/mod.rs
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq)]
pub enum RawValue { Int(i32), Long(i64), Float(f32), Str(String), Bool(bool), Set(BTreeSet<String>) }
pub type RawPrefs = BTreeMap<String, RawValue>;

#[derive(Default, Debug)]
pub struct NormaliseReport {
    pub converted: Vec<Converted>,      // { rule: &'static str, from_keys: Vec<String>, to_keys: Vec<String> }
    pub skipped:   Vec<Skipped>,        // { rule: &'static str, key: String, reason: SkipReason }
    pub ignored:   Vec<String>,         // legacy app-state / private keys dropped on purpose
    pub repo_sets: LegacyRepoSets,      // see section 8; handed to area:20-extension-strategy-redesign
}
pub enum SkipReason { DestinationPresent, WrongType, Unparseable, OutOfRange }

/// Applies every rule below in this order. Never panics, never writes a default for a missing legacy key.
pub fn normalise(raw: &mut RawPrefs) -> NormaliseReport;
```

General rules for every conversion (these fix the unconditional-write defects of quirks items 4, 5, 9, 14):
* G1: a rule fires only when its legacy key is present with the legacy type. A key of the unexpected type is reported `WrongType` and left alone.
* G2: a rule never overwrites its destination key when it is already present in the same backup. A backup from a current build can contain both the old key (Aniyomi leaves many of them behind: `secure_screen`, `enable_doh`, `relative_time`, `pref_update_only_non_completed_key`, `pref_sub_select_conf`) and the user's later choice in the new key; the new key wins and the rule reports `DestinationPresent`.
* G3: after a rule converts, its legacy keys are removed from the map.
* G4: out-of-range numbers fall back to the destination default by omitting the destination key (the typed default then applies); they are reported `OutOfRange`.

## 2. Rule group A: library sort, filters and chapter flags (task T-11a-011)

Sources: steps 44, 52, 64 (sort), 81, 82, 85, 99.

A1 library sort, run separately for manga (`library_sorting_mode`) and anime (`animelib_sorting_mode`), with the shared direction key `library_sorting_ascending`:

| Shape of the stored mode value | Action |
|---|---|
| `Int` (pre-64) | table below, direction from `library_sorting_ascending` when it is `Bool` (default `true`) |
| `Str` without `,` (builds 64 to 81) | rename table, direction from `library_sorting_ascending` when it is `Str` (default `"ASCENDING"`) |
| `Str` containing `,` (build 82 and later) | unchanged (already `<TYPE>,<DIRECTION>`) |

Int tables (step 64 plus step 81 combined, with the anime names corrected per quirks item 7; step 44 maps Int 5 `SOURCE` to 0 first, which is what the `5 -> ALPHABETICAL` entry below does):

| Int | manga type string | anime type string |
|---|---|---|
| 0 | `ALPHABETICAL` | `ALPHABETICAL` |
| 1 | `LAST_READ` | `LAST_SEEN` |
| 2 | `LAST_MANGA_UPDATE` | `LAST_ANIME_UPDATE` |
| 3 | `UNREAD_COUNT` | `UNSEEN_COUNT` |
| 4 | `TOTAL_CHAPTERS` | `TOTAL_EPISODES` |
| 5 | `ALPHABETICAL` | `ALPHABETICAL` |
| 6 | `LATEST_CHAPTER` | `LATEST_EPISODE` |
| 7 | `DATE_ADDED` | `DATE_ADDED` |
| 8 | `CHAPTER_FETCH_DATE` | `EPISODE_FETCH_DATE` |
| other | `ALPHABETICAL` | `ALPHABETICAL` |

Rename table for the string-without-comma shape: manga `LAST_CHECKED -> LAST_MANGA_UPDATE`, `UNREAD -> UNREAD_COUNT`, `DATE_FETCHED -> CHAPTER_FETCH_DATE`; anime `LAST_CHECKED -> LAST_ANIME_UPDATE`, `UNSEEN -> UNSEEN_COUNT`, `DATE_FETCHED -> EPISODE_FETCH_DATE`; any other name unchanged. Direction mapping: `true`/`"ASCENDING"` -> `ASCENDING`, `false`/anything else -> `DESCENDING`. Output value `"<TYPE>,<DIRECTION>"` in the original mode key; remove `library_sorting_ascending` after both media types were processed. Valid final type names are listed in `p1-data-and-config.md` section 4.

A2 filters from build 52: Bool keys `pref_filter_downloaded_key`, `pref_filter_unread_key`, `pref_filter_completed_key` -> enum-name String keys `pref_filter_library_downloaded_v2`, `pref_filter_library_unread_v2`, `pref_filter_library_completed_v2`; `true -> "ENABLED_IS"`, `false -> "DISABLED"`.

A3 filters from build 99: for each legacy Int key below (value 0 `DISABLED`, 1 `ENABLED_IS`, 2 `ENABLED_NOT`, anything else `DISABLED`) write `"<newKey>"` as enum name:

| legacy key | new key |
|---|---|
| `pref_filter_library_downloaded` | `pref_filter_library_downloaded_v2` |
| `pref_filter_library_unread` | `pref_filter_library_unread_v2` |
| `pref_filter_library_started` | `pref_filter_library_started_v2` |
| `pref_filter_library_bookmarked` | `pref_filter_library_bookmarked_v2` |
| `pref_filter_library_completed` | `pref_filter_library_completed_v2` |
| `pref_filter_library_tracked_<id>` for each integer id | `pref_filter_library_tracked_<id>_v2` |
| `pref_filter_library_unseen` | `pref_filter_animelib_unread_v2` (Proposal: anime filter, the key the anime library reads) |

A4 chapter/episode default flags (step 85): for the 10 distinct keys `default_chapter_filter_by_read`, `default_chapter_filter_by_downloaded`, `default_chapter_filter_by_bookmarked`, `default_chapter_sort_by_source_or_number`, `default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`, `default_episode_filter_by_seen`, `default_episode_filter_by_downloaded`, `default_episode_filter_by_bookmarked`, `default_episode_sort_by_source_or_number`: an `Int` value becomes `Long` (same numeric value); `Long` stays. Bit layouts are in `p1-data-and-config.md`.

## 3. Rule group B: update scheduling and restrictions (task T-11a-012)

B1 `pref_library_update_interval_key` (Int hours): values `1, 2, 3, 4, 6, 8` become `12` (composition of step 61: 1,2 -> 3, then step 71: 3,4,6,8 -> 12). Other values unchanged. Rationale: the UI offers 0, 12, 24, 48, 72, 168.
B2 `pref_update_only_non_completed_key` (Bool): when `false`, remove `"manga_ongoing"` from the Str-set `library_update_manga_restriction`, starting from the default set `{"manga_fully_read", "manga_ongoing", "manga_started", "manga_outside_release_period"}` when that key is absent from the backup; `true` changes nothing. The legacy key is removed.
B3 `library_update_restriction` (Str-set): remove the member `"battery_not_low"` if present.
B4 `backup_interval`: intentionally untouched (step 84 `0 -> 12` applies only on its own upgrade path; an explicit `0` in a backup means the user disabled automatic backups).

## 4. Rule group C: reader (task T-11a-013)

C1 `pref_rotation_type_key` (Int) -> `pref_default_orientation_type_key` (Int flag): 1 -> 8, 2 -> 16, 3 -> 24, 4 -> 32, 5 -> 40, other -> 8. Step 59 (reset to Free) is not applied because a backup does not say whether the stored value predates build 59; applying the step 60 table is correct for every value written by build 59 or later. Remove the legacy key.
C2 `pref_default_viewer_key` (Int 1..5) -> `pref_default_reading_mode_key` (Int, same value). Remove the legacy key.
C3 `reader_tap` (Bool): if `false` write `reader_navigation_mode_pager = 5` and `reader_navigation_mode_webtoon = 5` unless those keys exist (G2); if `true`, remove the legacy key only. When the key is absent nothing is written (quirks item 5).
Desktop note: orientation is meaningless in a desktop reader window; area:43-reader-core still stores the imported flag so a later export round-trips.

## 5. Rule group D: player (task T-11a-014)

D1 `pref_progress_preference`: `Str` that parses as `f32` becomes `Float`; a `Float` stays.
D2 `pref_skip_length_preference`: `Str` that parses as `i32` becomes `Int`.
D3 `pref_default_player_orientation_type_key`: `Str` of decimal digits becomes `Int` (step 93); an `Int` is mapped by the step 127 table to an enum-name String: 10 -> `Free`, 1 -> `Portrait`, 9 -> `ReversePortrait`, 0 -> `Landscape`, 8 -> `ReverseLandscape`, 7 -> `SensorPortrait`, 6 -> `SensorLandscape`, other -> `Free`; an enum-name `Str` stays. Orientation has no desktop effect (T-11a-026).
D4 enums (step 123), each only if the legacy key is present: `pref_deband` Int 0/1/2 -> `pref_video_debanding` `"None"`/`"CPU"`/`"GPU"`; `pref_player_view_mode` Int 0/1/2 -> `pref_player_aspect_state` `"Crop"`/`"Fit"`/`"Stretch"`; `gpu_next` Bool -> `pref_gpu_next` Bool. Out-of-range ordinals omit the destination (G4). The keys `pref_invert_playback_txt`, `pref_invert_duration_txt`, `pref_hwdec` are removed and reported as ignored.
D5 `pref_sub_select_conf` (Str JSON object `{"lang": [..], "blacklist": [..], "whitelist": [..]}`, unknown fields ignored, missing fields empty): when the three destination keys `pref_subtitle_lang`, `pref_subtitle_whitelist`, `pref_subtitle_blacklist` are all absent, write each as the list joined by `","` (no spaces); on JSON error report `Unparseable` and write nothing. The legacy key is removed.

## 6. Rule group E: language codes (task T-11a-015)

E1 applied after D5 to `pref_audio_lang` and `pref_subtitle_lang` (Str): `value.split(',')`, drop empty items, trim each item (in that order: an item consisting only of spaces becomes empty and is then rejected as invalid), keep items for which `is_valid_language_code` is true, join with `","`; write the result even when empty (Kotlin writes the empty string). Items keep their original case and spelling.
`is_valid_language_code(code)` Proposal (matches the JDK predicate `Locale(code)` with `isO3Language == language && language == displayName(ENGLISH)` meaning invalid, plus `MissingResourceException` meaning invalid):
* lowercase the code; length 2: valid iff it is an ISO 639-1 code;
* length 3: valid iff it is a known ISO 639-2 (bibliographic or terminology) or ISO 639-3 code;
* any other length, or any non-ASCII-letter character (so `en-US`, `pt_BR`, `""` are invalid): invalid.
Open point to settle with a JDK run while writing the fixtures: whether bibliographic three-letter codes such as `fre` and `ger` have English display names in the JDK data (the Kotlin result for them). The fixture rows are tagged `jdk-unverified` until checked; the Proposal treats every ISO 639-2 code as valid because mpv's `--alang`/`--slang` accept them.
Crate: `isolang` (ISO 639-1/3 tables, MIT) in `kuro-prefs`.

## 7. Rule group F: UI and miscellaneous (task T-11a-016)

F1 `pref_theme_mode_key` Str: uppercase it (step 86); values then are `LIGHT`, `DARK`, `SYSTEM`.
F2 `relative_time` Int: value `0` and `relative_time_v2` absent -> `relative_time_v2 = false`; any other value changes nothing. Legacy key removed.
F3 navigation (step 120), only when `bottom_nav_style` (Int) or `default_home_tab_library` (Bool) is present and the destinations are absent: `start_screen` = `"MANGA"` if `default_home_tab_library` is `true`, else `"ANIME"`; `bottom_rail_nav_style` = `0 -> "MOVE_HISTORY_TO_MORE"`, `1 -> "MOVE_UPDATES_TO_MORE"`, other -> `"MOVE_MANGA_TO_MORE"`. Legacy keys removed. (Desktop shell may ignore both values, area:36-ui-shell-navigation-security-webview.)
F4 `enable_doh` Bool: `true` and `doh_provider` absent -> `doh_provider = 1` (Cloudflare); the legacy key is removed in any case.
F5 `secure_screen` Bool: `true` and `secure_screen_v2` absent -> `secure_screen_v2 = "ALWAYS"`. The MIUI installer fallback of step 75 is dropped (T-11a-024).
F6 `source_languages` Str-set: if present and lacking `"all"`, add `"all"` (step 70).

## 8. Rule group G: repository URL lists (task T-11a-017)

Input keys `extension_repos` (manga) and `anime_extension_repos` (anime), Str-sets of repository URLs. The normaliser removes both keys from the map and returns neutral rows; area:20-extension-strategy-redesign decides persistence.

```rust
pub struct LegacyRepoSets { pub manga: Vec<LegacyMangaRepo>, pub anime: Vec<LegacyAnimeStore> }
pub struct LegacyMangaRepo { pub base_url: String, pub name: String, pub short_name: Option<String>, pub website: String, pub signing_key_fingerprint: String }
pub struct LegacyAnimeStore { pub index_url: String, pub name: String, pub badge_label: String, pub signing_key: String,
                              pub contact_website: String, pub contact_discord: Option<String>, pub is_legacy: bool, pub extension_list_url: Option<String> }
```

Manga: sort the URLs lexicographically (quirks item 16), number from 1; `source` = URL unchanged if it starts with `http://` or `https://`, else `https://raw.githubusercontent.com/<source>/repo` (step 114, non-idempotent flaw removed); row = `base_url = source`, `name = "Repo #<n>"`, `short_name = None`, `website = source`, `signing_key_fingerprint = "NOFINGERPRINT-<n>"` (step 7). Table constraints to respect: `base_url` primary key, `signing_key_fingerprint` UNIQUE.
Anime: sort lexicographically, number from 1; `index_url = url.removeSuffix("/index.min.json").removeSuffix("/index.json") + "/repo.json"`, `name = badge_label = "Repo #<n>"`, `signing_key = "NO_SIGNING_KEY"`, `contact_website = index_url`, `contact_discord = None`, `is_legacy = false`, `extension_list_url = None`. A duplicate `index_url` collapses (upsert semantics).

## 9. Rule group H: ignored legacy keys (task T-11a-023)

The normaliser removes and reports as `ignored`: un-prefixed `pref_download_only`, `incognito_mode`, `last_catalogue_source`, `trusted_signatures`, `last_app_closed`, `library_update_last_timestamp`, `library_unseen_updates_count`, `last_used_category`, `last_app_check`, `last_ext_check`, `last_version_code`, `storage_dir`; keys starting `pref_mangasync_` or `track_token_`; the legacy `automatic_ext_updates`. (Newer backups never contain app-state keys because `Preference.isAppState` keys are filtered at backup time.)

## 10. Ordering of the pipeline

`normalise` applies groups in this order: A, B, C, D, E, F, G, H. E must follow D5 (D5 can create `pref_subtitle_lang`). A1 must run before any code that reads `library_sorting_ascending`. Each group is a separate function (`fn group_a(raw: &mut RawPrefs, rep: &mut NormaliseReport)`), so tasks T-11a-011 to T-11a-017 are independently testable; fixtures per rule are listed in `p1-edge-cases-and-tests.md`.
