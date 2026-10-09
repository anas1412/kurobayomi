# Area 41 (part 1): edge cases, failure modes, race conditions, tests

Upstream tests: none of the 36 scope files has a `*Test.kt` counterpart. A search of `app/src/test` and `app/src/androidTest` for browse, extension, migration and source names found nothing (`app/src/test/java/mihon/core/migration/MigratorTest.kt` belongs to area:11). So every test below is NEW for Kurobayomi. Expected values are derived from the Kotlin logic cited in p1-behavior.md.

## 1. Edge cases (numbered; E1-E20 mirror p1-behavior.md section 19, E21+ are new)

| ID | Situation | Observed (Kotlin) | Required Kurobayomi behavior | Test |
|---|---|---|---|---|
| E1 | Multi-source extension, two sources | two Available rows with synthetic pkg `<pkg>-<id>` | same split; installer and step map use synthetic key; DB/prefs never store the synthetic key | T1, T2 |
| E2 | "Update all" with an active search | updates only the visible (filtered) set | same | T14 |
| E3 | NSFW untrusted extension with NSFW filter off | shown (untrusted not NSFW-filtered) | same | T3 |
| E4 | Install progress on a split row | progress keyed by synthetic pkg; row disappears on success | same | T15 |
| E5 | Transient `Installed` step | no action buttons | render as Idle buttons | T16 |
| E6 | Source row activation | Popular listing; Latest only if `supportsLatest` | same | T20 |
| E7 | Local source (`ID = 0`) | cannot be disabled; help action replaces WebView | same | T21, T22 |
| E8 | Migration network failure | dialog pops silently | error shown; nothing written | T30 |
| E9 | Shared `migrate_flags` key with manga | last dialog wins for both | separate anime key `anime_migrate_flags`; read-only import of `migrate_flags` once | T33 |
| E10 | Episode `seen` derivation | max-seen rule only | same | T31 |
| E11 | Enhanced tracker rejects new source | track dropped | same; record in outcome | T32 |
| E12 | Custom cover copy | input stream never closed | file copy; no leaked handle | T34 |
| E13 | Migrate search default filter | PinnedOnly | same; visible toggle | T40 |
| E14 | Season list paging | single call, page size 25 hint | single call; no pagination | T50 |
| E15 | Source filter language off | sources hidden, checkboxes untouched | same | T23 |
| E16 | Clear cookies | no UI feedback | banner "Cleared N cookies" (Proposal) | T25 |
| E17 | Update all concurrency | concurrent launches | bounded concurrency is NOT required; keep concurrent but cap at 4 (Proposal) | T14 |
| E18 | Package vanishes while details open | pops itself | same (watch channel closes or entry missing -> pop) | T26 |
| E19 | "all" language subtitle in global search | localized name, not "Multi" | same | T41 |
| E20 | Migrate resume position | `last_second_seen` not copied | copy `last_second_seen` and `fillermark` (Proposal, documented deviation) | T31 |
| E21 | Query `"  ,  "` (only separators) | every term empty -> no match, all filtered out | same (empty terms match nothing, so list empties) | T4 |
| E22 | Query numeric `"123"` | matches source id 123 exactly, not substrings | same; `"0123"` parses to 123 (Kotlin `toLongOrNull`) | T5 |
| E23 | Query with comma and spaces `"a, b"` | OR of `a` and `b` | same | T4 |
| E24 | Case: Turkish dotted I and German sharp s in query | Kotlin `ignoreCase` uses locale-insensitive Unicode folding | Rust `to_lowercase` (Unicode default); document; test with `"İ"` and `"ß"` | T6 |
| E25 | Empty `name` on a Stub source | `toString()` returns bare id | same | T42 |
| E26 | `lang == ""` for a source | display name = system language (`getLocalizedDisplayName("")`) | same; if system lang unknown, use `"Other"` (the `other_source` key) | T43 |
| E27 | `enabled_languages` empty set | no Available rows at all; filter list still shows all languages | same; show "No results found" in list | T13 |
| E28 | Installed extension with no sources | `sources` empty; details shows no source rows; enable-all no-op | same | T27 |
| E29 | Installed extension `isObsolete` and `hasUpdate` both true | sort puts it after non-obsolete; still in updates group | same | T11 |
| E30 | Untrusted and Installed with same pkg | Available filter excludes both; Installed group shows installed + untrusted | same | T9 |
| E31 | Two Installed extensions, same name, different pkg | sorted by name, stable by insertion order | sort must be stable (`sort_by` is stable in Rust) | T10 |
| E32 | Search matches only the store name | no match (store not searched) | same | T7 |
| E33 | Migration with `flags = 0` | still fetches episodes, tracks, and flips favorite | same (documented side effects) | T35 |
| E34 | Migration `replace = true` with a failure after step 5 | Kotlin: old still favorite (step 6 not reached), new maybe partly written | transactional: nothing written; both unchanged | T30 |
| E35 | Migrate source picker, all sources stubs | list only stubs | same | T44 |
| E36 | Migrate source picker DESCENDING with stubs | comparator reversed: stubs last | same (document) | T45 |
| E37 | Display mode pref unknown string | `CompactGrid` | same | T46 |
| E38 | Grid columns pref 0 vs n | adaptive 128 vs fixed | same | T47 |
| E39 | Global search progress 0 of 0 | bar hidden (`1..<total` empty) | same | T48 |
| E40 | Global search: all sources finished | bar hidden | same | T48 |
| E41 | Dialog: "Show seasons" for an Episodes target | button hidden | same | T36 |
| E42 | Dialog: old is Seasons, new is Episodes | canMigrate false; error text "season to episode" | same | T36 |
| E43 | Dialog: old is Episodes, new is Seasons | canMigrate false; error text "episode to season, select season to continue" | same | T36 |
| E44 | Extension list refresh twice quickly | `isRefreshing` toggled twice; 1 s min spinner per call | second call while refreshing is ignored (Proposal) | T17 |
| E45 | Search typed then cleared quickly | debounce emits last value only | same | T18 |
| E46 | Install cancelled | cancel by pkg; step flow completes; step removed | same; cancel of synthetic key works | T19 |
| E47 | Trust accepted for an extension that changed version | trust record keyed by pkg+versionCode+signature (area:19) | trust only the snapshot version (area:19) | T28 |
| E48 | Clipboard copy of debug info with no store | "Repository:" line omitted | same | T24 |
| E49 | Repo link for non-GitHub index URL | fallback to raw `indexUrl` | same | T24 |
| E50 | Icon decode fails | fallback default icon (`ic_default_source`), not error | same (`Result.Error` -> fallback) | T29 |
| E51 | Available icon URL 404 | error placeholder `cover_error` | same | T29 |
| E52 | Migration copies custom cover when old has none | skipped (check existence first) | same | T34 |
| E53 | Two migrate dialogs open (double click) | second start ignored while migrating | disable buttons while running | T37 |
| E54 | Source toggle ids stored as strings | parse back as long | `i64::from_str`; reject non-numeric entries silently (log) | T38 |
| E55 | `incognito` for pkg with synthetic suffix | never applies (toggle uses real pkg) | same | T39 |
| E56 | Source list pinned order | pinned first, then name | area:part-2 test; here only row render check | T49 |

## 2. Failure modes and handling (Kurobayomi)

| Failure | Detection | User-visible result | State after |
|---|---|---|---|
| Extension index fetch fails | `ExtError::Network` | banner "Failed to fetch available extensions" + Retry | previous snapshot kept |
| Install download fails | `InstallStep::Error` in stream | row action Retry (refresh icon) | step = Error until retry or cancel |
| Install bundle invalid | `ExtError::InvalidBundle` | row Error + banner text | nothing installed |
| Uninstall of missing dir | `ExtError::NotFound` | banner "Uninstalled" is NOT shown; error banner | row list refreshed |
| Trust write fails | `ExtError::Io` | banner | untrusted row stays |
| Cookie clear fails for one URL | `Result` per URL | logged; success count shown in banner (E16) | others cleared |
| Source missing at migrate time | `MigrateError::SourceMissing` | dialog red text | nothing written |
| Episode fetch fails | `MigrateError::FetchFailed` | dialog red text, Close | nothing written (transaction not begun) |
| DB write fails in transaction | `MigrateError::Db` | dialog red text | rolled back |
| Download delete fails (step 8) | `MigrateError::Io` | warning banner "Migrated, but downloads could not be deleted" | migration committed (step 8 after commit) |
| Cover copy fails | `Io` | warning banner | migration committed |
| Season fetch fails | error in season paging | empty state with error text + Retry | nothing written |
| Global search source throws | `Error(e)` item | per-source error text | other sources continue |
| Browse append fails | paging `LoadState::Error` | snackbar with Retry (indefinite) | items kept |
| Prefs corrupt (non-numeric id) | parse error | id skipped, logged | pref not rewritten |

## 3. Race conditions and ordering

| # | Race | Risk | Mitigation | Test |
|---|---|---|---|---|
| R1 | Snapshot update arrives while search debounce fires | stale groups | groups are recomputed from the latest snapshot and latest query every time; generation counter; drop outputs with older generation | T17, T18 |
| R2 | Install step events arrive after the row was removed (installed) | stale spinner | step map keyed by synthetic pkg; removed on stream end (`onCompletion` parity) | T15 |
| R3 | `update_all` while a manual update of the same extension runs | double install | per-pkg mutex in the install service (area:19); second call attaches to the running stream | T14 |
| R4 | Uninstall while install in progress for same pkg | file races | cancel first, then uninstall; uninstall waits for cancel ack | T19 |
| R5 | Migration started twice from the same dialog | double writes | dialog buttons disabled + `migrating` guard in model | T37 |
| R6 | Two concurrent migrations touching the same new anime | ordering of favorite flip | serialize migrations per (old,new) pair with a `tokio::sync::Mutex` keyed map | T37 |
| R7 | Details page open while uninstall from list | page shows a removed extension | watch on installed list: missing pkg -> pop | T26 |
| R8 | Pref change (`enabled_languages`) while list is recomputing | flicker of groups | recompute on the watch change; generation drop | T13 |
| R9 | Migrate search `search()` re-entry with the same query | duplicate network calls | `last_query` + `last_filter` guard (behavior 8.4) | T40 |
| R10 | Cover load for a recycled grid cell | wrong cover shown | cancel guard on unbind; compare anime id before applying texture | T51 |
| R11 | Source preference write from settings page while the source runs a request | value read mid-change | read through the source's pref store, not a cached copy | T52 |
| R12 | Global search started, then query changed | results of the old query arrive last | per-search generation id; results tagged and dropped on mismatch | T53 |
| R13 | Trust completes after the untrusted row was uninstalled by another action | duplicate registration | `trust` returns early when the untrusted entry is gone (behavior 1.8) | T28 |

## 4. Test plan (numbered T-IDs used above)

Unit tests (pure Rust, `cargo test`), owned by the `browse::anime` module.

| T-ID | Name | Asserts |
|---|---|---|
| T1 | `split_source_pkg_name` | Available split produces `pkg-<id>` with id as decimal |
| T2 | `synthetic_key_not_persisted` | after install of a split row, prefs contain only real pkg |
| T3 | `untrusted_ignores_nsfw_filter` | untrusted NSFW shown when `show_nsfw=false` |
| T4 | `query_or_terms_and_empty_terms` | `"a, b"` matches either; `"  ,  "` matches nothing |
| T5 | `query_numeric_exact_id` | `"123"` matches id 123 and not 1234; `"0123"` matches 123 |
| T6 | `query_unicode_case_fold` | `"ß"`, `"İ"` matched with Unicode lowercasing (document expected) |
| T7 | `query_store_name_not_searched` | store name alone does not match |
| T9 | `untrusted_and_installed_excludes_available` | pkg in both untrusted and installed removed from Available |
| T10 | `stable_sort_same_name` | equal names keep snapshot order |
| T11 | `obsolete_sorted_after_nonobsolete_in_updates` | E29 ordering |
| T13 | `empty_enabled_languages` | no Available rows; list shows empty state text |
| T14 | `update_all_only_visible_set` | with query "x", only matched updates are started |
| T15 | `install_step_map_removes_on_completion` | step entry removed when the stream ends |
| T16 | `installed_transient_step_has_idle_buttons` | Installed step renders Idle actions |
| T17 | `refresh_ignored_while_refreshing` | second refresh does not start a second fetch |
| T18 | `debounce_last_value_wins` | 5 quick edits produce one recompute with the last value (use a paused tokio clock) |
| T19 | `cancel_install_by_pkg` | cancel closes the stream and removes the step |
| T20 | `source_row_latest_only_when_supported` | Latest button absent when `supports_latest=false` |
| T21 | `local_source_cannot_disable` | options popover has no Disable for id 0 |
| T22 | `local_source_help_replaces_webview` | toolbar overflow shows Help for id 0 |
| T23 | `language_toggle_keeps_checkboxes` | toggling a language does not change disabled set |
| T24 | `debug_info_text_exact` | string equals the template in p1-behavior.md section 4.3, with and without store |
| T25 | `clear_cookies_counts` | cookie removal called per URL once; errors counted as 0 |
| T26 | `details_pops_when_pkg_missing` | model emits Uninstalled when pkg disappears |
| T27 | `no_sources_details` | details with empty sources renders no rows and no panic |
| T28 | `trust_noop_when_entry_gone` | trust returns Ok without writing |
| T29 | `icon_fallbacks` | decode error -> default; 404 -> error placeholder |
| T30 | `migrate_fetch_failure_writes_nothing` | fake net fails; DB snapshot unchanged |
| T31 | `migrate_episode_transfer` | seen derived from max-seen; bookmark/date_fetch copied; last_second_seen copied (E20) |
| T32 | `migrate_track_rejected_dropped` | enhanced tracker returning None -> track absent; outcome lists it |
| T33 | `migrate_flags_key_separate` | anime save writes `anime_migrate_flags`; manga key untouched |
| T34 | `migrate_custom_cover_copy` | file bytes equal; no copy when source missing |
| T35 | `migrate_zero_flags_still_fetches_and_flips` | flag 0: episodes refreshed, tracks moved, favorite true |
| T36 | `dialog_rules_fetch_type` | canMigrate false matrix; show-seasons visibility; messages |
| T37 | `migrate_double_start_guard` | second start returns `Busy` |
| T38 | `source_toggle_id_parse` | "12" parses; "x" skipped |
| T39 | `incognito_uses_real_pkg` | toggle writes real pkg |
| T40 | `migrate_search_default_pinned_only` | initial filter is PinnedOnly; origin first in order |
| T41 | `global_subtitle_uses_localized_name` | "all" -> "All" localized (not "Multi") |
| T42 | `stub_tostring` | blank name -> id string |
| T43 | `empty_lang_display` | "" -> system language or "Other" |
| T44 | `migrate_source_only_stubs` | all stubs listed, sorted |
| T45 | `migrate_source_desc_reverses_all` | DESCENDING reverses stub rule as well |
| T46 | `display_mode_unknown_default` | unknown -> CompactGrid |
| T47 | `grid_columns_zero_adaptive` | 0 -> adaptive 128; n -> fixed n |
| T48 | `progress_bar_visibility` | visible iff 1 <= progress < total |
| T49 | `source_list_pinned_first` | render order check only |
| T50 | `season_list_single_page` | one page, no next key |
| T51 | `cover_recycle_guard` | stale bind does not apply texture |
| T52 | `source_pref_read_through` | value written via settings reads back immediately |
| T53 | `global_search_generation_drop` | late results for an old query are dropped |
| T54 | `migrate_flags_bitmap_roundtrip` | `selected_bitmap(options)` decodes to same flags; unused bits preserved |
| T55 | `default_saved_flags_all_selected` | `MigrateFlags::DEFAULT_SAVED` selects all five options when present |
| T56 | `episode_flags_viewer_copied_verbatim` | new anime gets old episode_flags and viewer |

Integration tests (headless, `GDK_BACKEND=broadway` optional; skip in CI if unavailable):
- I1: build `Extensions` page with a fixture snapshot; assert three section headers in order; Update all button present only with updates.
- I2: build `BrowseSource` page in error-only state; assert buttons Retry, Open in WebView, Help in that order.
- I3: build dialog D1 with `canMigrate=false`; assert no Migrate button.
- I4: build global search page with two sources (one loading, one success); assert one progress bar fraction 0.5.

## 5. Fixtures

Store under `plans` tests folder of Kurobayomi (`tests/fixtures/browse_anime/`). JSON, one file per fixture:

| File | Content |
|---|---|
| `ext_installed.json` | 4 installed: A (update available, not obsolete, lang "en", 2 sources), B (obsolete, no update, 1 source, NSFW), C (lang "ja", untrusted-like name collision), D (torrent, 0 sources) |
| `ext_available.json` | 5 available: E (2 sources, langs "en" and "all"), F (1 source "de", NSFW), G (1 source "en" same pkg as installed A -> must be excluded), H (multi-source with name identical across sources), I (store-less) |
| `ext_untrusted.json` | 2 untrusted: J (signature hash "abc"), K (name "Zeta") |
| `languages.json` | enabled `["all","en"]`; available langs `["all","en","de","ja"]` |
| `sources_browse.json` | 3 sources: stub (blank name), local (id 0), HTTP with `getHomeUrl` |
| `anime_pair_episodes.json` | old: 12 episodes (1..12, seen 1..5, bookmark on 3, date_fetch varied, last_second_seen 1200 on episode 6); new: episodes 1..14, plus two unrecognized-number episodes (`episode_number` = -1 style) |
| `anime_pair_tracks.json` | old tracks: 1 enhanced accepting new source, 1 enhanced rejecting, 1 non-enhanced |
| `migrate_flags_defaults.json` | saved `0x7FFFFFFF`, plus saved `0x03`, plus saved `0x00` |
| `season_list.json` | 7 seasons, 2 of which are favorites |
| `search_results.json` | 4 sources x results (0, 3, error, loading) |
| `display_names.json` | `{ "en": "English", "de": "German", "ja": "Japanese", "zh-CN": "Chinese (Simplified)", "all": "All" }` (expected localized names; the test checks the locale-independent mapping; `all` is handled by the comparator) |

Locale-dependent fixtures run with `LANG=en_US.UTF-8` in CI.

## 6. Manual test script (run before each release of this part)

1. Install an extension from the list; watch the spinner; confirm the row becomes installed with the version text.
2. Cancel an install mid-download; confirm the row returns to Install.
3. Disable a language in the filter; confirm rows of that language vanish; re-enable; confirm they return.
4. Search `"aniyomi, 123"` and confirm the OR behavior.
5. Open an installed extension with two sources; toggle one; confirm the list order (enabled first).
6. Long-press an installed extension that is on disk; confirm it uninstalls immediately (parity).
7. Migrate a favorite to a second source with "Copy": confirm the old title stays favorite and the new one becomes favorite.
8. Migrate with "Migrate": confirm old unfavorited, date added preserved.
9. Migrate with the source offline: confirm the dialog shows the failure and nothing changed.
10. Open Global search with the "Pinned" chip; confirm only pinned sources appear; switch "All".
11. Set display mode to List; confirm row heights fit window height.
12. Resize to 360 px wide; confirm no clipped text (ellipsis) and no horizontal scroll on the list page.

## 7. Known untestable from this environment

- Real extension bundles (area:19 provides fixtures).
- Cover images and icons (fixtures only).
- Desktop focus behavior across compositors (manual).
