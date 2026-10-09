# Area 02 preferences system, part 1: implementation tasks

Ordering is by dependency. All crate and module names are defined in `p1-rust-design.md`; behaviours in `p1-behavior.md`; every key in `p1-topic-key-registry.md`. Task ids T-02a-001 to T-02a-040.

### T-02a-001 — Preference core types, store API and registry macro
- Priority: P0
- Effort: L
- Depends: area:01-foundation-app-bootstrap
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt
- Acceptance:
  - crate `kuro-prefs` exists with `PrefValue` (Int, Long, Float, Str, Bool, Set), `Namespace`, `PrefType`, `Decoded<T>`, `Pref<T>` (`get`, `set`, `is_set`, `delete`, `default_value`, `key`) and `PrefStore` (`get_raw`, `set_raw`, `remove`, `all`, `replace_many`, `subscribe`, `flush`, `pref`)
  - reading a key stored as another kind returns the default, removes the key and logs `Invalid value for <key>; deleting`
  - reading an enum or object value that does not decode returns the default and leaves the key stored
  - a `define_prefs!` macro generates the section structs plus a `REGISTRY` of key, namespace, kind, default, TOML table and field; a test asserts `REGISTRY` has 328 entries and equals the rows of `plans/02-preferences-system/p1-topic-key-registry.md` (key, store, kind, default)

### T-02a-002 — TOML backend with atomic writes, file lock and debounced flusher
- Priority: P0
- Effort: L
- Depends: T-02a-001, T-req-platform-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt
- Acceptance:
  - preferences persist to `$XDG_CONFIG_HOME/kurobayomi/config.toml` (normal keys) through the `Paths` trait, mode 0600, and survive an app restart
  - 100 `set` calls within 50 ms produce exactly one disk write about 250 ms after the first (verified with a counting backend and paused tokio time)
  - writes use atomic replace; killing the process during a write leaves either the old or the new file (fault-injection test)
  - unknown tables, unknown keys and comments in the file are preserved after a rewrite
  - a file that fails to parse is renamed `config.toml.corrupt-<unix seconds>` and the app starts with defaults; a field with a wrong TOML type resets only that field; an Int field holding a value outside i32 is treated as wrong type
  - a second process loading the same directory gets `PrefError::Locked`
  - only explicitly set keys are written; `delete()` removes the key from the file

### T-02a-003 — Key namespaces: normal, app-state, private; state.toml and the extra table
- Priority: P0
- Effort: S
- Depends: T-02a-001, T-02a-002
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt
- Acceptance:
  - `Namespace::of` classifies `__APP_STATE_x` as AppState, `__PRIVATE_x` as Private, anything else as Normal; helper functions `app_state_key`, `private_key`, `is_app_state`, `is_private` match Kotlin
  - app-state keys are persisted only to `$XDG_STATE_HOME/kurobayomi/state.toml`; private strings never reach either file
  - keys not present in the registry are kept in an `[extra]` table with explicit kind (`{ t = "long", v = 5 }`) and are returned by `PrefStore::all()`
  - dynamic key families (`pref_filter_library_tracked_<id>_v2`, `pref_filter_animelib_tracked_<id>_v2`, per-tracker auth-expired flag) round-trip through `[library.filter_tracked_manga]`, `[library.filter_tracked_anime]` and `[tracking.auth_expired]` tables keyed by tracker id

### T-02a-004 — Change notification and GTK main-loop bridge
- Priority: P0
- Effort: M
- Depends: T-02a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt
- Acceptance:
  - `Pref::changes()` yields the current value first, then one value per change of its key, re-reading with `get()`; a lagged receiver re-reads and continues
  - `replace_many` publishes a single `Changed { key: None }` that every `changes()` stream treats as a change
  - `kuro_ui::bind::watch` runs its closure on the GTK main thread when the preference is set from a tokio task (test with a `glib::MainContext` iteration)
  - dropping the returned guard stops the subscription (no task leak after a settings page closes)

### T-02a-005 — Preference helper operations
- Priority: P0
- Effort: S
- Depends: T-02a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt
- Acceptance:
  - `update`, `delete_and_get`, `toggle` (returns the new value), set `insert`, `extend`, `remove` behave like `getAndSet`, `deleteAndGet`, `toggle`, `plusAssign` (item and items) and `minusAssign`
  - `set` with a value equal to the stored one publishes no change event and schedules no write

### T-02a-006 — TriState and CheckboxState
- Priority: P0
- Effort: S
- Depends: none
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/TriState.kt, core/common/src/main/java/tachiyomi/core/common/preference/CheckboxState.kt, domain/src/main/java/tachiyomi/domain/entries/TriState.kt
- Acceptance:
  - `TriState` has DISABLED, ENABLED_IS, ENABLED_NOT with `next()` cycling in that order and an `apply_filter(state, predicate)` returning true, `p()`, `!p()` respectively
  - `CheckboxState::State::next` toggles Checked and None; `CheckboxState::TriState::next` maps None to Include, Include to Exclude, Exclude to None
  - `as_checkbox_state` and `map_as_checkbox_state` return Checked exactly when the predicate is true

### T-02a-007 — Value codecs: enums, display mode, library sort strings and flags, season display mode, startup repair
- Priority: P0
- Effort: M
- Depends: T-02a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, domain/src/main/java/tachiyomi/domain/library/model/LibraryDisplayMode.kt, domain/src/main/java/tachiyomi/domain/library/manga/model/MangaLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/anime/model/AnimeLibrarySortMode.kt, domain/src/main/java/tachiyomi/domain/library/model/Flag.kt, domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt
- Acceptance:
  - `pref_enum!` produces enums whose stored names equal the Kotlin constant names and whose decode returns `Invalid` for unknown names
  - `LibraryDisplayMode` serialises to `COMPACT_GRID`, `COMFORTABLE_GRID`, `LIST`, `COVER_ONLY_GRID` and any other string decodes to CompactGrid
  - `MangaLibrarySort` and `AnimeLibrarySort` round-trip all 10 and 11 type strings with `,ASCENDING` and `,DESCENDING`; empty, no-comma and unknown-type inputs follow edge cases E3 to E5
  - the integer flag form matches the table in `p1-behavior.md` 3.3 for every type and direction (type mask 0x3C, ascending 0x40) and unassigned bit patterns fall back to Alphabetical
  - `SeasonDisplayMode` converts 0 to 3 and returns an error for any other number
  - at startup an invalid enum or object value is rewritten to its default and logged once

### T-02a-008 — LibraryPreferences: display, sort, columns, filters, badges, categories, counters
- Priority: P0
- Effort: M
- Depends: T-02a-001, T-02a-006, T-02a-007
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - the `[library]` section exposes every non-template, non-season key of `LibraryPreferences` with the exact key strings and defaults from the registry (display mode, both sort modes, last-updated timestamp, categoryTabs true, numberOfItems false, categorizedDisplay false, hiddenCategories false, interval filter, four badges with defaults false/true/true/false, new-updates-count true, auto-clear cache false, both random seeds 0, four column counts 0, the ten plain filters DISABLED, both new-update counters 0, default and last-used categories)
  - the per-tracker filter getters take a tracker id and read `pref_filter_library_tracked_<id>_v2` / `pref_filter_animelib_tracked_<id>_v2`
  - `display_continue_reading_button` key is `display_continue_reading_button` (not renamed) and defaults to false
  - unit test lists every key string of this class and compares with the registry

### T-02a-009 — Chapter and episode default-flag templates and the "set as default" operations
- Priority: P0
- Effort: M
- Depends: T-02a-007, T-02a-008
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/entries/manga/model/Manga.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt
- Acceptance:
  - flag constants for `chapter_flags` and `episode_flags` (masks and values of `p1-data-and-config.md` 3.1 and 3.2) exist in `kuro_prefs::flags` and are shared with the DB area
  - the default template getters use keys `default_chapter_filter_by_read`, `default_chapter_filter_by_downloaded`, `default_chapter_filter_by_bookmarked`, `default_chapter_sort_by_source_or_number`, `default_episode_filter_by_seen`, `default_episode_filter_by_downloaded`, `default_episode_filter_by_bookmarked`, `default_episode_filter_by_fillermarked`, `default_episode_sort_by_source_or_number`, `default_episode_show_thumbnail_previews`, `default_episode_show_summaries` with default 0 (all sort and display defaults are the zero constants)
  - `set_chapter_settings_default(manga)` and `set_episode_settings_default(anime)` write the listed raw values and map `sortDescending()` to `SORT_DESC` (0x0) or `SORT_ASC` (0x1)
  - export writes the shared keys `default_chapter_display_by_name_or_number` and `default_chapter_sort_by_ascending_or_descending` from both the manga and the anime field (anime written last); import initialises both fields (quirk Q2)
  - a new manga or anime created after "set as default" gets its flags from the templates (integration test with the DB area fixtures)

### T-02a-010 — Season preferences and templates
- Priority: P2
- Effort: M
- Depends: T-02a-009
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, domain/src/main/java/tachiyomi/domain/entries/anime/model/Anime.kt, domain/src/main/java/aniyomi/domain/anime/SeasonDisplayMode.kt
- Acceptance:
  - all 16 `default_season_*` keys and the two update-behaviour booleans exist with the registry defaults (overlays false, true, true, false, true; grid mode 0; grid size 0)
  - `season_flags` constants (masks 0x1 through 0x8000000 as in `p1-data-and-config.md` 3.3, shifts 16 and 18) are available
  - `set_season_settings_default` writes the downloaded filter from the downloaded mask (not the unseen mask, quirk Q3) and a regression test fails if the unseen raw value is used
  - an invalid stored grid mode number is read as CompactGrid

### T-02a-011 — Library update, restrictions, category sets, swipe actions and duplicate rules
- Priority: P1
- Effort: M
- Depends: T-02a-008
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt
- Acceptance:
  - `auto_update_interval` default 0, `auto_update_device_restrictions` default {"wifi"} with the three allowed strings `wifi`, `network_not_metered`, `ac`, `auto_update_item_restrictions` default the four strings `manga_fully_read`, `manga_ongoing`, `manga_started`, `manga_outside_release_period`, `auto_update_metadata` false
  - a pure function `filter_update_candidates(entries, restrictions, ...)` implements the five skip rules in the documented order with the skip-reason message keys `skipped_reason_not_always_update`, `skipped_reason_completed`, `skipped_reason_not_caught_up`, `skipped_reason_not_started`, `skipped_reason_not_in_release_period` and sorts the kept entries by title
  - include/exclude category sets are applied as included-minus-excluded with distinct ids, empty include set meaning all entries
  - swipe actions: field `swipe_chapter_start_action` reads key `pref_chapter_swipe_end_action`, `swipe_chapter_end_action` reads `pref_chapter_swipe_start_action`, `swipe_episode_start_action` reads `pref_episode_swipe_end_action`, `swipe_episode_end_action` reads `pref_episode_swipe_start_action`, with defaults ToggleRead, ToggleBookmark, ToggleSeen, ToggleBookmark, and a test pins the crossed pairing (quirk Q1)
  - `mark_duplicate_read_chapter_as_read` and `mark_duplicate_seen_episode_as_seen` hold subsets of `new`, `existing` and `new_episode`, `existing_episode`

### T-02a-012 — ReaderPreferences
- Priority: P0
- Effort: L
- Depends: T-02a-001, T-02a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - all 54 keys of the registry section `[reader]` exist with exact key strings and defaults (for example `pref_enable_transitions_key` true, `pref_reader_flash_duration` 100, `pref_double_tap_anim_speed` 500, `pref_default_reading_mode_key` 2, `pref_default_orientation_type_key` 8, `pref_image_scale_type_key` 1, `pref_zoom_start_key` 1, `pref_reader_theme_key` 1, `webtoon_side_padding` 0, `reader_hide_threshold` LOW)
  - enums `FlashColor`, `TappingInvertMode` (with the two invert booleans per variant) and `ReaderHideThreshold` (5, 13, 31, 47) exist with Kotlin names
  - constants `WEBTOON_PADDING_MIN` 0, `WEBTOON_PADDING_MAX` 25, `MILLI_CONVERSION` 100 exist and the padding setter clamps to 0..25 in the UI binder
  - each key marked DROP in the registry (cutout_short, reader_volume_keys, reader_volume_keys_inverted, pref_default_orientation_type_key stored only) is present for backup round trip but is not offered in the UI
  - a settings change to any of these keys is seen by an open reader through `changes()` within one main-loop iteration

### T-02a-013 — Reader value tables: reading mode, orientation, tap zones, scale type, zoom start, colour filter modes
- Priority: P0
- Effort: S
- Depends: T-02a-012
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `ReadingMode` has flag values 0 to 5 with mask 0x7, `from_preference` returns DEFAULT for unknown ints, `is_pager_type` is true for LTR, RTL, VERTICAL only, and resolving DEFAULT to the preference default happens before a viewer is created (opening a viewer with DEFAULT is an error, as Kotlin throws)
  - `ReaderOrientation` has flag values 0x00, 0x08, 0x10, 0x18, 0x20, 0x28, 0x30 with mask 0x38
  - lists `TAP_ZONES` (6), `IMAGE_SCALE_TYPE` (6), `ZOOM_START` (4), `COLOR_FILTER_MODE` (6, all offered on Linux) hold the string-resource keys in the documented order; UI writes index+1 for scale type and zoom start
  - `COLOR_FILTER_MODE` maps indexes 0 to 5 to SrcOver, Modulate, Screen, Overlay, Lighten, Darken

### T-02a-014 — Player, gesture and decoder preferences
- Priority: P0
- Effort: M
- Depends: T-02a-001, T-02a-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/GesturePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt
- Acceptance:
  - every key of `PlayerPreferences` (36), `GesturePreferences` (13) and `DecoderPreferences` (9) exists with exact key strings and defaults (for example `pref_progress_preference` 0.85, `pref_player_time_to_disappear` 4000, `pref_panel_opacity` 60, `pref_default_intro_length` 85, `pref_skip_length_preference` 10, `pref_try_hwdec` true, `use_yuv420p` true, the five filter sliders 0, `default_speed_presets` the twelve decimal strings)
  - enums `PlayerOrientation` (8), `VideoAspect` (Crop, Fit, Stretch), `SingleActionGesture` (None, Seek, PlayPause, Switch, Custom), `Debanding` (None, CPU, GPU) and `CustomKeyCodes` (`0x10001` to `0x10006`) exist
  - the decoder options builder returns `hwdec=auto-safe` or `no`, `deband=yes` for GPU and one `vf` chain combining `format=yuv420p` and `gradfun=radius=12` (quirk Q9 regression test: CPU debanding plus YUV420P yields both filters)
  - the "mark seen" predicate is `last_second_seen >= total_seconds * progress` only when `total_seconds > 0`, only for unseen episodes, and never while `incognito_mode` is true
  - Android-only keys (`pref_default_player_orientation_type_key`, `pref_show_system_status_bar`, the four PiP keys) exist for round trip but have no UI

### T-02a-015 — Subtitle and audio preferences, mpv option mapping and track auto-selection
- Priority: P1
- Effort: M
- Depends: T-02a-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/TrackSelect.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - all 21 subtitle keys and 5 audio keys exist with defaults (`pref_subtitle_font` "Sans Serif", `pref_subtitles_font_size` 55, `pref_sub_border_size` 3, `pref_sub_pos` 100, `pref_sub_scale` 1.0, text colour -1, border colour -16777216, background colour 0, `pref_audio_volume_boost_cap` 30, `pref_audio_pitch_correction` true)
  - `SubtitleJustification` and `SubtitlesBorderStyle` produce the mpv strings `left|center|right|auto` and `outline-and-shadow|opaque-box|background-box`
  - the mapping function produces exactly the option list of `p1-topic-player-mpv-mapping.md` section 2 (colours as `#AARRGGBB`, delays as ms/1000 decimal, `volume-max` = cap+100, ASS override sets `sub-ass-override=force` and `sub-ass-justify=yes`)
  - `AudioChannels` yields (`audio-channels`,`auto-safe`) for Auto, (`audio-channels`,`auto`) for AutoSafe (default), mono, stereo, and (`af`, `pan=[stereo|c0=c1|c1=c0]`) for ReverseStereo
  - `TrackSelect` is ported with the documented algorithm and tests for: no preferred language and system locale fallback, blacklist removal, whitelist priority, no match returns none

### T-02a-016 — Advanced player preferences: mpv.conf, input.conf, user scripts, statistics page
- Priority: P1
- Effort: S
- Depends: T-02a-014, T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AdvancedPlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt
- Acceptance:
  - keys `mpv_scripts` false, `pref_mpv_conf` "", `pref_mpv_input` "", `pref_player_statistics_page` 0 exist
  - on each player start `mpv.conf` and `input.conf` in the mpv config dir are overwritten with the preference text (empty text produces empty files) and `scripts`, `script-opts`, `shaders` are emptied, then refilled from the user folders only when `mpv_scripts` is true
  - a non-zero statistics page triggers `script-binding stats/display-stats-toggle` then `stats/display-page-N` after init

### T-02a-017 — DownloadPreferences
- Priority: P1
- Effort: S
- Depends: T-02a-001
- Sources: domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt
- Acceptance:
  - all 23 keys exist with exact strings and defaults (`download_slots` 1, `download_speed_limit` 0, `remove_after_read_slots` -1, `pref_download_only_over_wifi_key` true, `save_chapter_as_cbz` true, `split_tall_images` true, the six category sets empty, the two download-new booleans false, the two `download_new_unread_*_only` booleans false)
  - UI choices are bound as documented: slots 1..5, auto-download-ahead 0,2,3,5,10, remove-after-read -1,0,1,2,3,4, speed limit in KiB/s with 0 meaning off
  - the six category-set key names are exposed as constants usable by the backup importer for id remapping
  - the "Wi-Fi only" preference is evaluated by the download scheduler through the GIO network monitor (metered connection counts as not Wi-Fi), and the setting label says "Unmetered connection only"

### T-02a-018 — TrackPreferences and tracker secrets
- Priority: P1
- Effort: M
- Depends: T-02a-001, T-req-platform-003
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt
- Acceptance:
  - switches `pref_auto_update_manga_sync_key` true, `track_on_adding_to_library` true, `show_next_episode_airing_time` true, `anilist_score_type` "POINT_10", `pref_auto_update_manga_on_mark_read` ALWAYS (ALWAYS, ASK, NEVER) exist
  - `SecretPref` get/set/delete for username, password and token per tracker id store values only through `SecretStore` (a grep test proves no tracker secret reaches `config.toml` or `state.toml`)
  - `set_credentials` stores username, then password, then clears the auth-expired flag; a failing password write leaves the flag unchanged
  - exporting with "Private settings" off omits the four private key families; with it on they appear as `__PRIVATE_pref_mangasync_username_<id>`, `__PRIVATE_pref_mangasync_password_<id>`, `__PRIVATE_track_token_<id>`, `__PRIVATE_pref_tracker_auth_expired_<id>`

### T-02a-019 — SourcePreferences: languages, hidden and pinned sets, repos, data saver, migration sort
- Priority: P1
- Effort: M
- Depends: T-02a-001, T-02a-007, T-02a-035
- Sources: app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt, app/src/main/java/eu/kanade/domain/source/interactor/SetMigrateSorting.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt
- Acceptance:
  - all 34 keys exist with exact strings and defaults (`show_nsfw_source` true, `browse_hide_in_library_items` false, data saver `data_saver` NONE, `ignore_jpeg` false, `ignore_gif` true, `data_saver_image_quality` 80, `data_saver_downloader` true)
  - `SetMigrateSorting::await(mode, direction)` writes `pref_migration_sorting` and `pref_migration_direction` together; enums ALPHABETICAL, TOTAL and ASCENDING, DESCENDING
  - app-state keys (`trusted_extensions`, `has_filters_toggle_state`, `last_anime_catalogue_source`, `last_catalogue_source`) land in `state.toml` with defaults empty, false, -1, -1
  - legacy `anime_extension_repos` and `extension_repos` are readable (for import) but no screen writes them
  - the commented-out SY keys are not created

### T-02a-020 — NetworkPreferences
- Priority: P0
- Effort: S
- Depends: T-02a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt
- Acceptance:
  - `default_user_agent` defaults to `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0` and the shared HTTP client sends it when a request has no `User-Agent`
  - `doh_provider` defaults to -1 and accepts 1 to 13 with the provider table of `p1-data-and-config.md` 6.2
  - `verbose_logging` defaults to true only in debug builds and switches the `tracing` filter for the HTTP target between `warn` and `debug` while the app runs
  - the Mullvad endpoint is used without its leading space

### T-02a-021 — TorrentPreferences
- Priority: P2
- Effort: S
- Depends: T-02a-001
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt
- Acceptance:
  - keys `pref_torrserver_enable` false, `pref_torrserver_shownotice` false, `pref_torrserver_port` "8090", `pref_torrserver_tackers` (sic), `pref_torrserver_proxymode` None, `pref_torrserver_proxyurl` "" exist
  - the default tracker string equals the 26 lines of `p1-data-and-config.md` 6.1 joined by `\n` with no trailing newline (byte-exact test)
  - `ProxyMode` wire values are None `tracker`, Tracker `tracker`, Peers `peers`, Full `full` (test pins quirk Q6)

### T-02a-022 — SecurityPreferences and app-lock state machine
- Priority: P2
- Effort: M
- Depends: T-02a-001, T-02a-003
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/SecureActivityDelegate.kt, app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt
- Acceptance:
  - keys `use_biometric_lock` false, `lock_app_after` 0, `secure_screen_v2` INCOGNITO (ALWAYS, INCOGNITO, NEVER), `hide_notification_content` false and app-state `last_app_closed` 0 exist
  - the pure function `require_unlock(lock_after, last_closed, now)` returns false for -1, true for 0, and `last_closed + lock_after * 60_000 <= now` otherwise; table test covers -1, 0, 1, 2, 5, 10 and a clock regression
  - leaving the app while unlocked writes `last_app_closed` only when `lock_after > 0`; entering the foreground deletes it
  - the secure-screen setting is stored but has no effect; the settings page hides it

### T-02a-023 — UiPreferences and BasePreferences, UI enums, date format, theme application
- Priority: P0
- Effort: M
- Depends: T-02a-001, T-02a-007
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt, app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt, app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - eight `UiPreferences` keys and the `BasePreferences` keys exist with exact strings and defaults (`pref_theme_mode_key` SYSTEM, `pref_app_theme` DEFAULT, `relative_time_v2` true, `app_date_format` "", `start_screen` ANIME, `bottom_rail_nav_style` MOVE_HISTORY_TO_MORE, `tablet_ui_mode` AUTOMATIC); `pref_downloaded_only`, `incognito_mode`, `onboarding_complete` are app-state
  - `AppTheme` has the 22 names in the documented order and the three deprecated names have no title
  - `NavStyle::tabs` returns the five tabs of `[AnimeLibrary, MangaLibrary, Updates, History, Browse, More]` minus the style's tab
  - `ThemeMode` LIGHT, DARK set `gtk-application-prefer-dark-theme` false and true; SYSTEM follows the portal colour-scheme and updates when the portal signals a change
  - `datefmt::format("")` gives the locale short date; the five listed patterns render `2024-03-09` as the documented strings; an unsupported pattern falls back to the locale short date
  - `relative_time` rendering follows the day-difference table of `p1-behavior.md` 5.3 (today, n days, upcoming, otherwise formatted date)

### T-02a-024 — BackupPreferences and StoragePreferences
- Priority: P1
- Effort: S
- Depends: T-02a-001, T-02a-003, T-02a-035
- Sources: domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt, domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt, domain/src/main/java/tachiyomi/domain/backup/service/PreferenceValues.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - `backup_interval` default 12 with UI choices 0, 6, 12, 24, 48, 168 hours; app-state `last_auto_backup_timestamp` default 0; app-state `storage_dir` default from `Paths`
  - `backup_flags` is accepted on import and re-exported unchanged but never read by app code
  - changing `backup_interval` calls the backup scheduler (area:22-backup-restore) with the new value in the same main-loop iteration
  - the constants FLAG_CATEGORIES "1", FLAG_CHAPTERS "2", FLAG_HISTORY "4", FLAG_TRACK "8", FLAG_SETTINGS "10", FLAG_EXT_SETTINGS "20", FLAG_EXTENSIONS "40" exist as strings for import tolerance

### T-02a-025 — Export preferences into backup entries
- Priority: P1
- Effort: M
- Depends: T-02a-003, T-02a-018, T-02a-027, area:21-backup-format-and-create
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupPreference.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupOptions.kt
- Acceptance:
  - `export_app(include_private)` returns only explicitly set normal keys with the Aniyomi key strings and Kotlin kinds, never app-state keys, private keys only when requested, sets sorted
  - `export_sources(include_private)` returns one entry per source file that has keys, `source_<id>` keys in ascending id order
  - the prost encoding of the four golden messages in `p1-edge-cases-and-tests.md` section 4 is byte-identical and a real `.tachibk` produced by Aniyomi decodes with all preference kinds; the golden table is corrected if the real file differs
  - the decoder identifies the value kind by the serial-name suffix so `...backup.models.IntPreferenceValue` and `...backup.full.models.IntPreferenceValue` both decode

### T-02a-026 — Import preferences from a backup
- Priority: P1
- Effort: M
- Depends: T-02a-025, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/PreferenceRestorer.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/BackupRestorer.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - known keys with the registry kind are written, kind mismatches are skipped and counted, unknown keys are kept in `[extra]`, app-state keys are skipped, private keys are written to the keyring only when the user chose private restore
  - `default_category` and `default_anime_category` are remapped by category name through separate manga and anime maps; unmapped values are not written, while -1 and 0 map to themselves
  - the twelve category-id keys (six library, six download) are remapped per element through the matching map and unioned with the device set; an all-unmapped set changes nothing
  - the import runs after the categories were restored and commits with one `Changed { key: None }`; importing the same backup twice yields an identical `config.toml`
  - after import the library-update (manga and anime) and auto-backup schedules are recomputed and the report with counts is returned to the restore dialog

### T-02a-027 — Per-source preference store
- Priority: P1
- Effort: M
- Depends: T-02a-002, T-req-platform-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/utils/Preferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/preference/SharedPreferencesDataStore.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/manga/extension/details/MangaSourcePreferencesScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt
- Acceptance:
  - `SourcePrefs::open(paths, id)` maps to `sources/source_<id>.toml` with typed values for Int, Long, Float, String, Bool and Set, preserved across restarts without losing the Int/Long distinction
  - the helper `pref_key(id)` returns `source_<id>` exactly
  - a wrong-kind read deletes the key and returns the default, like the main store
  - writes use the same atomic and debounced flushing as the main store; two sources never share a file
  - widget binders accept a `SourcePrefs` accessor so area:20-extension-strategy-redesign can render extension settings with the same switch, dropdown, entry and multi-select controls

### T-02a-028 — Drop BackupExtensionPreferences and the duplicate backup.full models
- Priority: P3
- Effort: S
- Depends: T-02a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/models/BackupExtensionPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/BackupPreference.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/full/models/Backup.kt
- Acceptance:
  - the Rust proto definitions contain no `BackupExtensionPreferences` message and no `full.models` copy; decoding a backup that carries unknown fields ignores them without error
  - the drop reason (unused in Aniyomi, nothing is lost) is recorded in the crate docs

### T-02a-029 — Legacy and foreign key import table
- Priority: P2
- Effort: M
- Depends: T-02a-026
- Sources: app/src/main/java/mihon/core/migration/migrations/CombineUpdateRestrictionMigration.kt, app/src/main/java/mihon/core/migration/migrations/MigrateSecureScreenMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveOneTwoHourUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/RemoveQuickUpdateMigration.kt, app/src/main/java/mihon/core/migration/migrations/EnableAutoBackupMigration.kt
- Acceptance:
  - an imported `secure_screen` true becomes `secure_screen_v2 = ALWAYS`; an imported `bottom_nav_style` is stored in `[extra]`; legacy `extension_repos` and `anime_extension_repos` sets are handed to the repo importer of area:20-extension-strategy-redesign
  - imported `pref_library_update_interval_key` of 1 or 2 becomes 3 and a retired quick value becomes 12; imported `backup_interval` of 0 stays 0 (the Aniyomi migration only runs on its own upgrade path)
  - imported `library_update_manga_restriction` containing the retired split values is combined as `CombineUpdateRestrictionMigration` does
  - every conversion has a test fixture and is reported in the import report

### T-02a-030 — Bind the mockup settings pages to the preference store
- Priority: P0
- Effort: M
- Depends: T-02a-001, T-02a-004, T-02a-039
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - `spec/SCHEMA.md` and `src/spec.rs` accept optional `pref` and `values` on switch, list, multi, slider, text, tristate, sort and chips items; the existing spec files still load
  - `ui::item_row` attaches `bind::switch`, `dropdown`, `chips`, `scale_i32/f32`, `entry`, `multi`, `tristate` or `sort` for items with `pref`; items without `pref` keep today's behaviour
  - the pages `settings_appearance`, `settings_library`, `settings_reader`, `settings_downloads` and `settings_browse` have `pref` on every item that has a registry key; toggling a switch changes `config.toml` within 400 ms and restarting the app shows the stored state
  - the test `every_pref_exists_in_registry` fails when a `pref` path is unknown, when `values.len() != options.len()` or when the item default differs from the registry default
  - reflecting a change made elsewhere (second window or backup import) updates the visible widget without recursion or flicker

### T-02a-031 — Drop SharedPreferencesDataStore (AndroidX preference bridge)
- Priority: P3
- Effort: S
- Depends: T-02a-027
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/preference/SharedPreferencesDataStore.kt
- Acceptance:
  - no Rust type mirrors `PreferenceDataStore`; extension settings screens read and write through `SourcePrefs` and the shared binders
  - the replacement mapping (get/put Boolean, Int, Long, Float, String, StringSet to `SourcePrefs` kinds) is documented in `kuro-prefs` module docs, and the user loses nothing

### T-02a-032 — Android-only preferences: keep for round trip, hide from the UI
- Priority: P3
- Effort: S
- Depends: T-02a-001
- Sources: app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt, app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - the registry marks as DROP and the UI hides: `extension_installer`, `pref_display_profile_key`, `pref_hardware_bitmap_threshold`, `pref_always_decode_long_strip_with_ssiv`, `tablet_ui_mode`, `pref_default_player_orientation_type_key`, `pref_show_system_status_bar`, `pref_enable_pip`, `pref_pip_episode_toasts`, `pref_pip_on_exit`, `pip_replace_with_previous`, `cutout_short`, `reader_volume_keys`, `reader_volume_keys_inverted`, `pref_default_orientation_type_key`, `secure_screen_v2`, `backup_flags`, `use_external_downloader`, `external_downloader_selection`
  - each key still imports and exports with its original kind and value (round-trip test over the list)
  - `device_has_pip()` is a constant false and no code path depends on PiP
  - the crate docs list each drop with its reason and what the user loses

### T-02a-033 — In-memory backend and test harness
- Priority: P2
- Effort: S
- Depends: T-02a-001
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/InMemoryPreferenceStore.kt, domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt
- Acceptance:
  - `Prefs::in_memory()` builds the full typed tree over `MemoryBackend` with working sets and change events (the Kotlin `TODO("Not yet implemented")` and never-emitting `changes()` are fixed)
  - `MemoryBackend` never spawns a flusher so tests need no runtime
  - the release-check test of area:35-data-misc-updater-notification-export can inject `Prefs::in_memory()` instead of a mock

### T-02a-034 — Settings schema version and migration hook
- Priority: P1
- Effort: S
- Depends: T-02a-002, area:11-app-migrations
- Sources: app/src/main/java/eu/kanade/tachiyomi/App.kt, app/src/main/java/mihon/core/migration/migrations/RemoveOneTwoHourUpdateMigration.kt
- Acceptance:
  - both TOML files carry `schema = 1`; opening a file with a higher schema logs a warning and loads known fields without modifying unknown ones
  - the app-state key `last_version_code` is available in `state.toml` through the raw store API for the migration runner of area:11-app-migrations
  - a migration can read and rewrite raw keys (`get_raw`, `set_raw`, `remove`) before typed sections are constructed

### T-02a-035 — Environment-dependent defaults
- Priority: P0
- Effort: S
- Depends: T-02a-001, T-req-platform-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/LocaleHelper.kt, core/common/src/main/java/tachiyomi/core/common/storage/AndroidStorageFolderProvider.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - on a first run with `LANG=de_DE.UTF-8` the default `source_languages` is {"all","en","de"}; with `LANG=C` or unset it is {"all","en"}; once the user edits the set it is never recomputed
  - the default `storage_dir` is `Paths::default_storage_dir()` as a plain path (no `file://` URI) and is created on first use
  - the default `app_theme` is DEFAULT and `verbose_logging` is true only for debug builds

### T-02a-036 — Value range and choice metadata for settings widgets and import clamping
- Priority: P1
- Effort: S
- Depends: T-02a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ColorFilterPage.kt, app/src/main/java/eu/kanade/presentation/library/anime/AnimeLibrarySettingsDialog.kt
- Acceptance:
  - the registry carries optional ranges or allowed values: `download_slots` 1..5, auto-download-ahead {0,2,3,5,10}, `remove_after_read_slots` {-1,0,1,2,3,4}, library interval {0,12,24,48,72,168}, backup interval {0,6,12,24,48,168}, `lock_app_after` {0,1,2,5,10,-1}, columns 0..10, `custom_brightness_value` -75..100, colour filter channels 0..255, `webtoon_side_padding` 0..25
  - widgets read these ranges so UI and metadata cannot diverge; import never clamps (it keeps the stored value) but consumers that need a valid value call `clamped()`

### T-02a-037 — Settings search terms from the registry
- Priority: P2
- Effort: S
- Depends: T-02a-030
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt
- Acceptance:
  - the settings search of the mockup also matches the preference's table name and Rust field name as hidden terms (searching "tapping" finds the tap-inversion settings)
  - results still show `Category > Group` breadcrumbs and the maximum of 50 hits, unchanged

### T-02a-038 — Shutdown flush and save-failure notification
- Priority: P1
- Effort: S
- Depends: T-02a-002
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt
- Acceptance:
  - `gio::Application::shutdown` waits up to 2 s for `PrefStore::flush()`; a test that sets a value and quits within 100 ms finds it on disk
  - after 3 consecutive failed writes one toast with key `settings_save_failed` is shown and no more until a write succeeds
  - values stay readable from memory while writes fail

### T-02a-039 — Prefs aggregate and application-context wiring
- Priority: P0
- Effort: S
- Depends: T-02a-001, T-02a-002, area:01-foundation-app-bootstrap
- Sources: app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt
- Acceptance:
  - `Prefs::open(paths, secrets)` builds the 18 sections once with one shared store, the same set as the 18 singleton registrations of `PreferenceModule`
  - `main.rs` opens `Prefs` before creating widgets, applies theme mode and start screen, and exposes it through `ui::ctx()` for GTK-thread callers
  - no other crate reads `config.toml` or builds a preference path (grep test)

### T-02a-040 — Kurobayomi-only settings in backups
- Priority: P2
- Effort: S
- Depends: T-02a-025
- Sources: app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/PreferenceBackupCreator.kt
- Acceptance:
  - settings that exist only in Kurobayomi are exported with the `kuro_` key prefix as normal-namespace entries and re-imported by Kurobayomi
  - an export containing `kuro_` keys restores cleanly in the Aniyomi restorer logic (keys are accepted as strings; test runs the Kotlin algorithm as a Rust port) and does not alter any other value
