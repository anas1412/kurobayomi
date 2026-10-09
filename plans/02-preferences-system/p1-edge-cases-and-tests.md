# Area 02 preferences system, part 1: edge cases, failure modes, tests

## 1. Upstream tests in scope

None. No `*Test.kt` file exists for any of the 34 scope files. The only upstream test touching the preference API is `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`, which mocks `PreferenceStore.getLong(any(), any())` with mockk to return a `Preference<Long>` mock whose `get()` returns 0 and `set` is a no-op. It tests release checking (area:35-data-misc-updater-notification-export), not preferences. Consequence for the Rust port: `PrefStore` must be injectable behind a trait or constructible in memory so area:35-data-misc-updater-notification-export can port that test with `Prefs::in_memory()` (task T-02a-033).

## 2. Edge cases and failure modes

| # | Situation | Expected behaviour | Source of the rule |
|---|---|---|---|
| E1 | Key present with another primitive kind (for example `pref_progress_preference` stored as Int) | `get()` deletes the key, logs `Invalid value for <key>; deleting`, returns the default; next `is_set()` is false | `AndroidPreference.kt:32-40` |
| E2 | Enum key holds an unknown name (`"FOO"`) or empty string | default returned; key stays in the file; value is repaired on startup repair pass (Kurobayomi addition) | `PreferenceStore.kt:27-43`, quirk Q16 |
| E3 | Sort string without comma (`"ALPHABETICAL"`) or empty | empty string gives default; no comma gives default (the `values[1]` access throws inside the `try`) | `MangaLibrarySortMode.kt` deserialize |
| E4 | Sort string with unknown type but valid direction (`"FOO,DESCENDING"`) | type falls back to `Alphabetical`, direction applies: `(Alphabetical, Descending)` | same |
| E5 | Sort string with valid type and a direction word that is not exactly `ASCENDING` (for example `"asc"`) | direction is `Descending` (only the exact word gives Ascending) | same |
| E6 | `default_season_grid_display_mode` holds 7 | `SeasonDisplayMode.fromLong` throws `IllegalArgumentException("Invalid sorting value")`; consumers must catch and use CompactGrid (Rust returns `Result`, the schema accessor maps `Err` to CompactGrid) | `SeasonDisplayMode.kt` |
| E7 | `LibraryDisplayMode` string unknown | default `CompactGrid`, no exception | `LibraryDisplayMode.kt` |
| E8 | Category flags value with unassigned sort bits (0x24, 0x28, 0x2C, 0x34, 0x38) | `Type.valueOf` falls back to Alphabetical | `MangaLibrarySortMode.kt:38-41` |
| E9 | `config.toml` does not parse | renamed `config.toml.corrupt-<unix>`, start with defaults, one-time toast; the previous `config.toml.bak` is NOT auto-restored (user decides) | design |
| E10 | `config.toml` parses but one field has the wrong TOML type (string where integer expected) | that field alone is treated as WrongType: removed from memory, default used, file rewritten without it at next flush; other fields unaffected | E1 |
| E11 | Integer outside the Rust type range (Int field holds 3_000_000_000) | WrongType, same as E10 | design |
| E12 | Disk full or read-only config dir at flush | `PrefError::Io` logged; dirty flag stays; retry after 1 s, 2 s, 4 s, 8 s, 16 s, then every 30 s; after 3 failures one toast `settings_save_failed`; in-memory values keep working; app does not crash | design |
| E13 | Process killed during write | atomic replace guarantees either the old or the new file; at most the last 250 ms of changes are lost | design |
| E14 | Two Kurobayomi processes | second start fails the `fd-lock`; `SingleInstance` forwards to the first window; if the lock is stale (crashed process) the OS releases the advisory lock automatically | design |
| E15 | Clock moves backward while the app lock timer runs | `last_app_closed + delay <= now` is false: the app stays locked longer, never unlocks early; a clock jump forward unlocks earlier (accepted, same as Kotlin) | `SecureActivityDelegate.kt:59-63` |
| E16 | App closed while locked | `require_unlock` stays true across the close; `last_app_closed` is only (re)written when not already locked | `SecureActivityDelegate.kt:40-46` |
| E17 | `lock_app_after` is 3 (not one of the UI values) | treated as 3 minutes (generic branch `last + 3 * 60_000 <= now`) | same |
| E18 | `source_languages` is empty | no sources are listed (no implicit fallback to `all`); the browse screen shows its empty-state message (area:40-ui-browse-manga/41) | `SourcePreferences.kt:23-26` |
| E19 | `LANG`/`LC_ALL` unset or `C`/`POSIX` | `source_languages` default is `{"all","en"}` | design |
| E20 | `default_category` points at a category that was deleted | the pref is not rewritten by this area; consumers check existence and fall back to "ask" (-1 behaviour) (area:37-ui-library/10) | design |
| E21 | `download_slots` restored as 0 or 99 | stored as is; the download manager clamps to 1..5 (area:23-downloads-manga/24) | UI range in `SettingsDownloadScreen.kt:102-106` |
| E22 | Tracker secret requested while no Secret Service is available | `SecretStore` fallback (encrypted file) answers; if that also fails, `get()` returns "" and the tracker shows "not logged in", `set()` returns `PrefError::Secret` and the login dialog shows the message from the error | `requirements-cross-platform.md` row `SecretStore` |
| E23 | Float precision | `0.85` written as `0.85` and read back as `0.85f32`; comparisons use `f32::EPSILON * 4` | pitfalls |
| E24 | Restore with a key of another kind than the registry | skipped and counted (stricter than Kotlin, see backup topic section 5) | design |
| E25 | Restore of a set into `library_update_categories` where none of the ids map | nothing written, device value untouched (matches Kotlin: `ids.isNotEmpty()` guard) | `PreferenceRestorer.kt:128-141` |
| E26 | Restore `default_category` = -1 | not written (Kotlin) -> Kurobayomi maps -1 to -1 and writes it (Q11 fix) | quirk Q11 |
| E27 | Backup contains `__APP_STATE_*` or `__PRIVATE_*` keys | skipped unless private restore was requested; app-state always skipped | design |
| E28 | Per-source file for an uninstalled source | kept untouched; Aniyomi exports only configurable sources it currently knows (`PreferenceBackupCreator.createSource`), Kurobayomi exports every file that has at least one key so settings of a temporarily removed extension are not lost | design |
| E29 | Subtitle/audio language list contains spaces or empty items (`"ja, ,en,"`) | split on `,`, drop empty, trim (see `TrackSelect`) | `TrackSelect.kt:16-31` |
| E30 | Date format pattern invalid (`"yyyy-MM-dd'T"`) | translator returns the locale short date, no panic | design |
| E31 | `app_date_format` is a non-empty pattern whose letters are unsupported by the translator (`"EEEE"`) | locale short date (documented limitation; patterns offered by the UI are the five in `p1-behavior.md` 5.3) | design |
| E32 | Preference written from a non-GTK thread while a GTK handler is reading the same key | readers take the read lock for microseconds; handlers run later on the GTK loop; no deadlock because no lock is held while calling out | design |
| E33 | Subscriber lag (more than 256 pending events, for example a restore writing 300 keys one by one) | `RecvError::Lagged` is converted into a single re-read; bulk restore uses `replace_many` which emits one event | design |
| E34 | Unknown tables or keys in `config.toml` (newer version) | preserved verbatim on rewrite | design |
| E35 | `config.toml` edited by hand while the app runs | not watched; changes are overwritten at next flush for known keys and unknown ones preserved from the in-memory document (documented limitation) | design |
| E36 | User resets one preference to default | `delete()` removes the key; file no longer contains it; `is_set()` false; exports omit it | `Preference.delete` |
| E37 | `torr_server_trackers` edited so it ends with a newline or has CRLF | stored verbatim; the TorrServer adapter trims lines and ignores empty ones (area:13-core-util-storage-archive-torrent) | design |
| E38 | `pref_mpv_conf` contains non-UTF8 bytes pasted from a file | the text field is `String` (UTF-8); invalid input is rejected by the entry widget; imports of a backup with lossy bytes use `String::from_utf8_lossy` | design |
| E39 | `external_player_preference` holds an Android package name from an Aniyomi backup (`org.videolan.vlc`) | not a valid command template on Linux: field kept (round trip) but the player settings page shows the "Not available on this system" hint and falls back to the internal player | design |
| E40 | `enable_pip` true after restore | ignored (no PiP) | dropped key |

## 3. Race conditions

* Flush vs quit: handled by `flush()` in the shutdown hook; if the shutdown hook times out (2 s) the app exits anyway and logs the loss.
* Restore vs UI listeners: bulk `replace_many` emits `Changed { key: None }` after all values are in memory, so a listener reading two related prefs (interval and restrictions) never observes a half-restored state.
* Category restore vs preference restore: serialised by the area:22-backup-restore orchestrator (categories first), fixing Q11b.
* Secret write vs `trackAuthExpired` clear in `set_credentials`: username, password, then the flag, in that order; if the flag write fails the secrets are already stored and the next login clears it.
* Two flushes: the flusher is a single task; `flush()` locks a `tokio::sync::Mutex` shared with it.

## 4. New tests to write (Rust, `crates/kuro-prefs/tests/` unless noted)

| Test file | Cases |
|---|---|
| `store_semantics.rs` | get default when absent; set/get each of the six kinds; `is_set` before/after; `delete`; `delete_and_get`; wrong-type read deletes key (E1) and logs once; enum unknown name returns default and keeps key (E2); object decode failure keeps key; `toggle`; set `insert`/`extend`/`remove`; `update`; equal-value `set` publishes nothing |
| `changes.rs` | first item is the current value (ignition); one item per set; conflation under burst; `replace_many` emits one `Changed(None)`; lagged receiver recovers; two subscribers independent; stream ends when store dropped |
| `codec_sort.rs` | all 10 manga and 11 anime type strings round trip; E3, E4, E5; flag bits table (every type/direction flag number from `p1-behavior.md` 3.3); `valueOf` fallback for 0x24..; `Flag.plus` algebra equals Kotlin on a table of 30 random pairs (golden values computed by hand in the test) |
| `codec_misc.rs` | `LibraryDisplayMode` four strings plus unknown; `SeasonDisplayMode` 0..3 and error for 4; `TriState::next` cycle; `CheckboxState::next` cycles including the Include -> Exclude -> None order |
| `registry.rs` | parse `plans/02-preferences-system/p1-topic-key-registry.md` and assert: 328 keys; every key string unique except the two documented shared pairs (`default_chapter_display_by_name_or_number`, `default_chapter_sort_by_ascending_or_descending`); kind and default equal; namespace equals prefix; every TOML (table, field) unique; `default_config` snapshot equals `p1-topic-default-config.md` |
| `toml_backend.rs` | round trip all defaults after setting every key once; unknown keys and comments preserved; atomic write leaves no `.tmp` on success; corrupt file renamed (E9); wrong field type (E10); out-of-range int (E11); read-only directory (E12, run only on Unix with a tempdir mode 0500); `.bak` created |
| `lock.rs` | second `TomlBackend::load` on the same dir returns `Locked` (E14) |
| `flush.rs` | 100 sets inside 50 ms yield one write (count via a counting backend) after 250 ms; `flush()` writes immediately; failure retry schedule using tokio paused time |
| `secrets.rs` | in-memory `SecretStore` fake: get/set/delete; `set_credentials` order and flag clearing; failure of password write leaves flag set |
| `backup_export.rs` | app-state keys excluded; private excluded by default and included on request; sets sorted; only set keys exported; source files exported in ascending id; exact bytes for the four golden messages below |
| `backup_import.rs` | kind mismatch skipped; unknown key kept in `[extra]`; app-state skipped; `default_category` remap by name incl. -1 and 0; category sets union; anime/manga maps independent; second import idempotent; report counts |
| `source_prefs.rs` | typed round trip for six kinds incl. Long vs Int; wrong-kind read deletes; file per source; concurrent writers |
| `datefmt.rs` | the six UI patterns render 2024-03-09 as `03/09/24`, `09/03/24`, `2024-03-09`, `09 Mar 2024`, `Mar 09, 2024`, locale short for `""`; E30, E31 fall back |
| `restrictions.rs` | the four item restriction strings and the device restriction strings are exactly the constants; defaults equal `{"wifi"}` and the four-item set |
| `swipe_keys.rs` | Q1 pairing: field `swipe_chapter_start_action` reads `pref_chapter_swipe_end_action`; `swipe_chapter_end_action` reads `pref_chapter_swipe_start_action`; `swipe_episode_start_action` reads `pref_episode_swipe_end_action`; `swipe_episode_end_action` reads `pref_episode_swipe_start_action` |
| `season_defaults.rs` | `set_season_settings_default` copies 16 values; Q3 regression: downloaded filter uses the downloaded mask |
| `applock.rs` (area:36-ui-shell-navigation-security-webview integration) | the state machine table: (lock_after, last_closed, now) -> require_unlock for -1, 0, 1, 5, 10 and clock regression (E15) |
| `tests/spec_binding.rs` (root crate) | every `pref` in `spec/*.json` resolves; `values.len() == options.len()`; item default equals registry default |
| UI smoke (`KUROBAYOMI_SMOKE` path of the mockup) | toggling a bound switch in `settings_library` writes `config.toml` within 400 ms and a second window instance of the page reflects it |

### Golden backup messages (expected encoding of `BackupPreference`, hex)

These values were derived from the kotlinx-serialization-protobuf rules (polymorphic message = field 1 serial name, field 2 value message; Int/Long varint; negative Int as 10-byte varint; Float fixed32 little endian; repeated string for sets) and MUST be confirmed against a real `.tachibk` created by Aniyomi before the test is marked final (acceptance of T-02a-025); if a byte differs, correct the doc and the test together.

| Preference | Hex |
|---|---|
| `download_slots` Int 3 | `0a0e646f776e6c6f61645f736c6f7473123f0a3965752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e496e74507265666572656e636556616c756512020803` (81 bytes; wrapper length byte `3f` = 63) |
| `pref_progress_preference` Float 0.85 | `0a18707265665f70726f67726573735f707265666572656e636512440a3b65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e466c6f6174507265666572656e636556616c756512050d9a99593f` |
| `default_category` Int -1 | `0a1064656661756c745f63617465676f727912480a3965752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e496e74507265666572656e636556616c7565120b08ffffffffffffffffff01` |
| `library_update_restriction` Set {"wifi","ac"} | `0a1a6c6962726172795f7570646174655f7265737472696374696f6e124d0a3f65752e6b616e6164652e7461636869796f6d692e646174612e6261636b75702e6d6f64656c732e537472696e67536574507265666572656e636556616c7565120a0a04776966690a026163` |

## 5. Fixtures needed

| Fixture | Content | Used by |
|---|---|---|
| `fixtures/config-default.toml` | the default config from `p1-topic-default-config.md` | registry, toml_backend |
| `fixtures/config-corrupt.toml` | truncated file (`[library`) | E9 |
| `fixtures/config-wrongtypes.toml` | `auto_update_interval = "12"`, `show_nsfw_source = 3` | E10 |
| `fixtures/config-future.toml` | unknown table `[experimental]` and unknown key in `[ui]` | E34 |
| `fixtures/aniyomi-prefs.tachibk` | a real gzip proto backup exported by the Aniyomi build this plan was written against containing app prefs, one source pref file and private prefs (user supplies; scrubbed of tokens) | backup_import, golden bytes |
| `fixtures/categories.json` | manga and anime categories with names for the remap tests | backup_import |
| `fixtures/subtitle-tracks.json` | a list of tracks (names, languages) for `TrackSelect` tests | T-02a-015 |

## 6. Non-functional checks

* Startup: `Prefs::open` on a 60 KB config in under 20 ms (criterion bench `prefs_open`).
* Memory: the value map holds under 400 entries; no per-entry heap beyond the key and value.
* No `unwrap()` on I/O or parse in the crate (clippy `unwrap_used` deny on non-test code).
* `cargo test -p kuro-prefs` runs headless (no display).
