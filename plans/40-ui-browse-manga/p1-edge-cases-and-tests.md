# Area 40 (part 1): edge cases, failure modes, races, and tests

No `*Test.kt` file exists in the scope (checked: the 40 scope files are all `src/main`). So there are no upstream tests to port directly. The tests below are new, derived from the behaviour in p1-behavior.md. Each test ID is `TC-40-nnn`. The pure functions they target are in p1-rust-design.md section 3.

---

## 1. Edge cases (E-40-nn)

### 1.1 Extension search and grouping

| ID | Case | Expected | Source of truth |
|---|---|---|---|
| E-40-01 | Query `""` | every extension matches | `queryFilter` |
| E-40-02 | Query `"   "` (spaces only) | `split(",")` gives `["   "]`, trimmed to `""`, no match for that term; the whole query matches nothing | `queryFilter`: empty term returns false |
| E-40-03 | Query `"a,,b"` | terms `a`, `""`, `b`; matches `a` OR `b` | OR of trimmed terms |
| E-40-04 | Query `"123"` | matches a source whose id is 123 (`toLongOrNull`); matches an extension whose name contains "123" | `it.id == input.toLongOrNull()` |
| E-40-05 | Query `"-5"` | `toLongOrNull` gives `-5`; matches only a source with id -5 (negative ids exist for stubs/local) | |
| E-40-06 | Query contains a comma inside a name (`"Foo, Bar"`) | split into two terms; the name matches neither exactly. Known limitation, keep | |
| E-40-07 | Installed extension, non-HTTP source | `baseUrl` is not searched | `it is HttpSource` guard |
| E-40-08 | Available extension, source with empty `baseUrl` | `"".contains(term)` is false for non-empty terms; no crash | |
| E-40-09 | Untrusted extension, query matches only its source name | NOT shown (Untrusted matches name only) | `is MangaExtension.Untrusted ->` branch |
| E-40-10 | Same extension is both in updates and installed | appears twice (once per group) | group construction |
| E-40-11 | Extension with `hasUpdate` but query excludes it | absent from updates group and from installed group | filter applied to both |
| E-40-12 | All extensions filtered out, query non-empty | `isEmpty` true -> `no_results_found` with repos button | `ExtensionContent` / `MangaExtensionScreen` |
| E-40-13 | No extensions at all, no query | `empty_screen` with repos button | |
| E-40-14 | Language group with display name collision (two langs map to the same display name) | two header rows with the same text; keys differ by lang in Kotlin (`header.hashCode()`), keep the two groups separate (key by lang, not by text) | `groupBy { it.lang }` then header from display name |
| E-40-15 | `lang` empty string for Available | grouped under `""` lang; display name `""` (`getLocalizedDisplayName("")`); sorted first by `compareTo` (empty string is smallest) | `LocaleHelper.getLocalizedDisplayName` |
| E-40-16 | Untrusted extension with `lang == null` | in installed group; language line hidden (only Installed shows language) | row content |
| E-40-17 | Installed with `lang == ""` | language line hidden | `extension.lang.isNotEmpty()` |
| E-40-18 | `versionName == ""` | version element hidden | `isNotEmpty` |
| E-40-19 | Warning priority: Untrusted and NSFW | shows only `ext_untrusted` (first `when` branch) | `when` order |
| E-40-20 | Installed, obsolete and NSFW | shows only `ext_obsolete` | `when` order |
| E-40-21 | Install step `Installed` (completed) | no status text, no action buttons for that instant | `isCompleted` and `when` |
| E-40-22 | Install step set to a non-busy value in the text branch (defensive) | Kotlin throws; Rust shows nothing | `error("Must not show ...")` |
| E-40-23 | Two concurrent installs of the same pkg | second `addDownloadState` overwrites the first; first's `onCompletion` removes the entry while the second still runs (progress disappears early) | `currentDownloads` keyed by pkg. Upstream bug; Kurobayomi: reference-count per pkg (E-40-23 fix) |
| E-40-24 | Cancel an install | job cancels, `onCompletion` removes the entry; row returns to Idle | `cancelInstallUpdateExtension` |
| E-40-25 | Update-all while another install runs | sequential; updates after the running one? No: update-all starts a new loop; two installs of different pkgs may run in parallel | `launchIO` per call |
| E-40-26 | Update-all with zero updates | no-op | `filter { hasUpdate }` |
| E-40-27 | Refresh while already refreshing | second call sets `isRefreshing = true` again and resets to false after its own delay; the spinner may hide early | `findAvailableExtensions` |
| E-40-28 | Refresh fails inside `findAvailableExtensions` | upstream: exception propagates in `launchIO` (no catch), `isRefreshing` stays `true` forever | Kurobayomi: use `finally` to reset `is_refreshing` (E-40-28 fix, deliberate) |
| E-40-29 | Trust extension: `trust` throws | launched on `screenModelScope.launch`, uncaught; log only | Kurobayomi: log and show toast |
| E-40-30 | Long-press Available with sources empty | install (no web action) | |

### 1.2 Extension details

| ID | Case | Expected | Source |
|---|---|---|---|
| E-40-31 | Extension uninstalled while details open | `installedExtensionsFlow` emits null -> `Uninstalled` event -> pop | details model init |
| E-40-32 | `repoUrl` is a raw GitHub URL with extra path | regex `(.+?)/(.+?)/.+` captures user and repo; the rest is dropped | `repo_web_url` |
| E-40-33 | `repoUrl` is raw GitHub with only two segments (`https://raw.githubusercontent.com/user/repo`) | no match (needs a third segment); URL used as-is (raw) | regex requires `/.+` after repo |
| E-40-34 | `repoUrl` null | open-repo action hidden; `url` null | |
| E-40-35 | `repoUrl` empty string | `find("")` no match; `url = ""`, and `url != null` so the action is shown with an empty URI; Kurobayomi: treat empty as absent (deliberate fix) | `orEmpty()` then `?: state.extension?.repoUrl` |
| E-40-36 | Clear cookies with sources having empty baseUrl | empty filtered out; distinct; cookie removal for the rest | `takeUnless { isEmpty }` |
| E-40-37 | Clear cookies with an invalid URL string | `toHttpUrl()` throws; caught per URL, counted as 0; loop continues | `try/catch` |
| E-40-38 | Enable all for an extension with zero sources | no-op | `?.let` on list |
| E-40-39 | Sources sort with `labelAsName` true for some | key = raw name (not lowercased) mixed with lowercased display names; ordering quirk kept | `compareBy` |
| E-40-40 | Incognito toggle with extension null | no-op | `state.value.extension?.pkgName?.let` |
| E-40-41 | Debug copy with `Untrusted` (not Installed) | only the first block is copied | `if (extension is Installed)` |
| E-40-42 | `pkgName` without the expected prefix | `substringAfter` returns the whole string (no crash) | |
| E-40-43 | NSFW dialog opened, then extension changes NSFW flag | dialog stays until dismissed | |

### 1.3 Browse, grids, and sources

| ID | Case | Expected | Source |
|---|---|---|---|
| E-40-44 | Paging error on refresh with zero items | empty-error state with actions | `BrowseSourceContent` |
| E-40-45 | Paging error on append with items | snackbar with retry; retry calls `mangaList.retry()` | |
| E-40-46 | Refresh error takes priority over append error | refresh error shown | `takeIf` order |
| E-40-47 | Source is `LocalMangaSource` with error | only local help action | `source is LocalMangaSource` |
| E-40-48 | Source is null (not loaded) with error | non-local actions | `source is LocalMangaSource` false |
| E-40-49 | `mangaList[index]` returns null (placeholder) | nothing rendered at that index; no crash | `?: return@items` |
| E-40-50 | `CoverOnlyGrid` preference | compact grid rendering | dispatch |
| E-40-51 | Source with `supportsLatest == false` | no Latest button | `if (source.supportsLatest)` |
| E-40-52 | Source name blank in migrate list | falls back to `id.toString()` | `ifBlank` |
| E-40-53 | Stub source with icon null | warning icon | `isStub && icon == null` |
| E-40-54 | Local source (id 0) in options dialog | no Disable row | `source.id != LocalMangaSource.ID` |
| E-40-55 | Source list with a language present only in pinned sources | header still shown by the part-2 model | part 2 |
| E-40-56 | Pin toggle while the list is loading | ignored (list not shown) | |

### 1.4 Global search and migrate search

| ID | Case | Expected | Source |
|---|---|---|---|
| E-40-57 | Progress `total == 0` | progress bar hidden (`progress in 1..<0` is empty range) | `1..<total` |
| E-40-58 | Progress `total == 1, progress == 1` | hidden (`1..<1` empty) | |
| E-40-59 | Progress equal to total | hidden | |
| E-40-60 | Division by zero in `progress / total` | never reached because range check guards | |
| E-40-61 | Result row for a source with empty list | "no_results_found" line | `EmptyResultItem` |
| E-40-62 | Error row with `throwable.message == null` | `unknown_error` | `message ?: ...` |
| E-40-63 | Migrate search when the manga does not exist | `getManga(...)!!` crashes upstream; Kurobayomi: error page with back button | D-UI (p1-rust-design section 11) |
| E-40-64 | Migrate search origin source is disabled (hidden) | origin still first? Only if it is in the enabled set: upstream filters by `PinnedOnly` only; disabled sources come from part-2 `getEnabledSources()` | part 2; verify |
| E-40-65 | Origin source is pinned and PinnedOnly | origin appears first only if it is pinned | filter then sort |
| E-40-66 | Two sources with equal name and lang | order = stable sort order of input; Kotlin `sortedWith` is stable; Rust `sort_by_key` is stable | |
| E-40-67 | `"▶ "` prefix when origin source not in results | no prefix (takeIf) | `fromSourceId?.let { ... takeIf }` |

### 1.5 Migration

| ID | Case | Expected | Source |
|---|---|---|---|
| E-40-68 | `flags = 0` (nothing selected) | chapters not copied, categories not copied, no downloads deleted, no custom cover; tracks still copied; favorite flip and metadata still happen | engine steps 2-8 |
| E-40-69 | `flags = Int.MAX_VALUE` | all flags on | codec |
| E-40-70 | `flags` has bit 4 (value 4) set | ignored by decoders; not an error | reserved bit |
| E-40-71 | Old manga has no custom cover, CUSTOM_COVER flag set | step 7 skipped (`hasCustomCover()` false) | engine |
| E-40-72 | Old manga has a custom cover, flag off | step 7 skipped; new manga does not get the cover | engine |
| E-40-73 | Old source uninstalled (`get` returns null) | `prevSource = null`; delete-downloads skipped (if oldSource null); tracks: `isTrackFrom(..., null)` | engine step 5 |
| E-40-74 | New source uninstalled | `migrateManga` returns silently (`?: return`) | engine 14.3 |
| E-40-75 | `getChapterList` throws | isMigrating reset; nothing written | engine 14.3 |
| E-40-76 | `syncChaptersWithSource` throws | ignored; chapter copy proceeds with pre-sync list (re-fetch from DB after sync) | step 1 |
| E-40-77 | Chapter copy with no prev chapters | no dateFetch/bookmark copy; read unchanged; max read null so no `read = true` | merge |
| E-40-78 | Prev chapters all unread | max read null; no `read = true` | merge |
| E-40-79 | Prev read chapter numbers include `-1` (unrecognised, read) | `maxOfOrNull` includes -1; `max` = max of recognised reads if any, else -1; if -1 is the only read chapter, max = -1 and new chapters with `chapterNumber <= -1` (only unrecognised ones, which are skipped) = nothing marked. Effect: no visible change | upstream quirk; kept |
| E-40-80 | New chapter number equal to a prev read chapter | `read = true` (`<=`) | merge |
| E-40-81 | Duplicate chapter numbers in prev (e.g., 5.0 twice) | `find` takes the first; the second's dateFetch/bookmark ignored | merge |
| E-40-82 | Chapter number `NaN` | not produced by any source; `NaN >= 0` false, so unrecognised; keep Rust float compare semantics | merge |
| E-40-83 | Tracks: `migrateTrack` returns null | track dropped | `mapNotNull` |
| E-40-84 | Tracks: no enhanced tracker matches | original track copied with new mangaId | `firstOrNull` then `else updatedTrack` |
| E-40-85 | Tracks: old manga has tracks but `insertTrack` fails | exception; migration aborts after chapters/categories were written; no rollback | engine (proposal: transaction, p1-behavior 15) |
| E-40-86 | `replace = true` and copy-failure in step 7 | old manga already un-favorited (step 6) and new not yet favorited (step 8) -> user loses the entry from the library | ordering defect upstream; Kurobayomi fix: move favorite flip for the new manga before step 6 inside one transaction |
| E-40-87 | `dateAdded` for copy | now in epoch ms | |
| E-40-88 | Migrate to the same manga id (old == new) | upstream does not guard; Kurobayomi: reject with error `same_manga` (new key, P2) | |
| E-40-89 | Migrate while old manga is in a category and the new manga is also in categories | new categories replaced (not merged) | `setMangaCategories` |
| E-40-90 | Dialog: double-click Migrate | second click starts a second migration; upstream has no guard (`launchIO` twice). Kurobayomi: disable buttons when migrating | state `isMigrating` set only after the call starts; race window |
| E-40-91 | Dialog: close while migrating | cannot close (overlay replaces dialog) upstream; the work continues in the scope | |
| E-40-92 | Migrate pops to a manga screen that no longer exists in the stack | `popUntil { items.contains(lastItem) }` leaves stack unchanged; push new details | S16 |
| E-40-93 | Favorites list with a manga whose title is `""` | sorts first (empty string) | sort |
| E-40-94 | Favorites list sort with unicode titles | `CASE_INSENSITIVE_ORDER` (per-char `toUpperCase/toLowerCase` compare). Rust `to_lowercase()` differs for some special cases (e.g., `ß`); accept, document | sort |
| E-40-95 | Source picker counts equal | order by the part-2 model; stable | |
| E-40-96 | `SetMigrateSorting` invalid stored value | enum `getEnum` fallback to default (`ALPHABETICAL`, `ASCENDING`) | prefs |
| E-40-97 | `migrate_flags` stored as a value that has no bits set in 1,2,8,16 | every flag unchecked on open; copy/migrate with nothing ticked (E-40-68) | |

### 1.6 Source filter and hidden/pinned encodings

| ID | Case | Expected | Source |
|---|---|---|---|
| E-40-98 | `hidden_catalogues` contains a non-numeric string | ignored by lookups (never equal to `"${id}"`); keep | |
| E-40-99 | `pinned_catalogues` stale id (uninstalled source) | ignored; no UI row | |
| E-40-100 | Language toggle off for the language of the only visible source | language header shows off, its sources hidden | S5 |
| E-40-101 | `source_languages` empty | no language headers; S5 shows the empty state only if no sources | S5 |
| E-40-102 | System locale `"C"` or `"POSIX"` for default languages | Kurobayomi: map `C`/`POSIX` to `en` (new, Linux-specific) | `Locale.getDefault().language` on Android gives "en"; Linux `LANG=C` must not produce `c` |

---

## 2. Failure modes and recovery

| Failure | Detection | User sees | Recovery |
|---|---|---|---|
| Repo index unreachable | `findAvailableExtensions` error | list keeps last results; refresh spinner stops (with fix E-40-28) | refresh button |
| Icon fetch fails | `reqwest` error | error placeholder (`cover-error.svg`) | retry on next paint (cache negative for 10 min, Proposal) |
| Source page fetch fails (paging) | `LoadState.Error` | toast bar or empty-error actions | retry |
| DB write fails in migrate | `MigrateError::Step` | dialog stays open, no toast (upstream) / toast (D-UI-4) | user retries; partial state possible (E-40-85) |
| Config file corrupt | TOML parse error | defaults used; log warning; the bad file is renamed `browse.toml.bad` | user fixes or deletes |
| Clipboard unavailable (Wayland without focus) | `set_text` no effect | nothing | none needed |
| WebView/browser cannot open URL | `UriLauncher` error | toast `internal_error` | copy URL manually (not offered; Proposal: show URL in toast) |

---

## 3. Races and concurrency

| ID | Race | Mitigation |
|---|---|---|
| R-40-01 | Search query changes during a grouping computation | Grouping is synchronous on the main thread (p1-rust-design section 5); no race |
| R-40-02 | Install event arrives after the row was removed by a refresh | Map update keyed by pkg; row rebuild reads the map; stale event harmlessly updates the map; cleared on Finished |
| R-40-03 | Update-all + user clicks Update on the same extension | second install of the same pkg (E-40-23): Kurobayomi ref-count prevents early clear; the UI shows busy until both finish |
| R-40-04 | Uninstall while install running | cancel the install token first, then uninstall (order in the handler) |
| R-40-05 | Details page open for an extension that is being updated | `installedExtensionsFlow` re-emits the new version; state replaced; no pop |
| R-40-06 | Migration dialog open while the source is removed | engine returns `NewSourceMissing` (E-40-74) |
| R-40-07 | Two pages push migrate for the same manga | navigation stack holds both; each has its own state; acceptable |
| R-40-08 | Tab switch channel receives 2 then 3 quickly | DROP_OLDEST: only the last (3) is applied; Kurobayomi: drain then send (p1-behavior section 1) |
| R-40-09 | Paging request while a previous page request is in flight | Kurobayomi: single in-flight page request per list; new scroll events ignored until done |
| R-40-10 | Pin toggled twice quickly | last write wins; config writes are serialized on the main thread |
| R-40-11 | Async channel receiver dropped on page pop | sender dropped too; the tokio task's `send` returns Err; task exits (check result) |

---

## 4. Test plan

Unit tests (pure Rust, `cargo test`), each with at least one assertion that fails if the logic breaks:

| ID | Test | Targets | Assertions |
|---|---|---|---|
| TC-40-001 | `query_matches_empty_query_matches_all` | `matches_query` | E-40-01 |
| TC-40-002 | `query_terms_are_or_and_trimmed` | `matches_query` | E-40-02, E-40-03 |
| TC-40-003 | `query_matches_numeric_source_id` | `matches_query` | E-40-04, E-40-05 |
| TC-40-004 | `query_skips_baseurl_for_non_http_installed_source` | `matches_query` | E-40-07 |
| TC-40-005 | `query_untrusted_matches_name_only` | `matches_query` | E-40-09 |
| TC-40-006 | `group_updates_then_installed_then_languages` | `group` | group order: updates, installed, languages sorted with `all` first |
| TC-40-007 | `group_installed_includes_untrusted_after_installed` | `group` | order within group |
| TC-40-008 | `group_same_extension_in_updates_and_installed` | `group` | E-40-10 |
| TC-40-009 | `group_empty_when_no_match` | `group` | E-40-12 (state.is_empty) |
| TC-40-010 | `compare_languages_all_first_and_reflexive` | `compare_languages` | `all` < others; `compare("all","all") == Equal` (pitfall 2) |
| TC-40-011 | `install_step_completed_set` | `InstallStep::is_completed` | Idle/Installed/Error true; Pending/Downloading/Installing false |
| TC-40-012 | `download_states_ref_count_per_pkg` | `DownloadStates` (after E-40-23 fix) | two sets, one clear: still busy |
| TC-40-013 | `details_sort_enabled_first_then_label` | `sort_details_sources` | E-40-39 quirk preserved |
| TC-40-014 | `repo_url_conversion` | `repo_web_url` | E-40-32, E-40-33, E-40-34, E-40-35 (empty -> None, Kurobayomi fix) |
| TC-40-015 | `flags_decode_bits` | `MigrateFlags::has_*` | 1,2,8,16 each; value 4 ignored; `i32::MAX` all true |
| TC-40-016 | `flag_options_order_and_conditions` | `flag_options` | E-40-71/72/97: custom cover and delete-downloaded only when shown; order |
| TC-40-017 | `flag_options_default_selection_uses_and` | `flag_options` | `default_bitmap & flag != 0` (pitfall 3) |
| TC-40-018 | `selected_bitmap_or_and_zero` | `selected_bitmap` | E-40-68: no selection gives 0; OR of ticked flags |
| TC-40-019 | `selected_bitmap_roundtrip` | `flag_options` + `selected_bitmap` | for every subset of options, decoding gives back the same subset |
| TC-40-020 | `merge_chapter_state_copies_date_and_bookmark` | `merge_chapter_state` | E-40-77 |
| TC-40-021 | `merge_chapter_state_marks_read_at_or_below_max` | `merge_chapter_state` | E-40-78, E-40-80 |
| TC-40-022 | `merge_chapter_state_first_match_wins` | `merge_chapter_state` | E-40-81 |
| TC-40-023 | `merge_chapter_state_skips_unrecognised` | `merge_chapter_state` | `-1` chapters untouched; E-40-79 |
| TC-40-024 | `merge_chapter_state_does_not_copy_read` | `merge_chapter_state` | `read` is never copied from prev, only derived |
| TC-40-025 | `order_migrate_sources_origin_pinned_name` | `order_migrate_sources` | E-40-65, E-40-66 |
| TC-40-026 | `sort_favorites_case_insensitive` | `sort_favorites` | E-40-93 |
| TC-40-027 | `pins_contains_and_is_pinned` | `Pins` | `Pinned` bit only; `ACTUAL` alone not pinned in list |
| TC-40-028 | `display_name_zh_mapping` | `localized_display_name` | `zh-CN` -> `zh-Hans` label path (behavior 9) |
| TC-40-029 | `source_display_name_special_keys` | `source_display_name` | `other`, `all` map to labels |
| TC-40-030 | `prefs_defaults_match_upstream` | `BrowsePrefs::default` | `display_mode = CompactGrid`, `migrate_flags = i32::MAX`, sort `Alphabetical`/`Ascending`, `show_nsfw_sources = true` |
| TC-40-031 | `prefs_unknown_values_fallback` | `BrowsePrefs::load` | E-40-96, invalid display mode -> CompactGrid |
| TC-40-032 | `prefs_preserve_unknown_keys_on_save` | `BrowsePrefs::save` | unknown TOML keys survive |
| TC-40-033 | `engine_skips_chapters_when_flag_off` | `migrate_manga` with fakes | E-40-68 |
| TC-40-034 | `engine_copy_keeps_old_favorite_and_uses_now` | `migrate_manga` with fakes | `replace=false`: no `update_favorite(old,false)`; `date_added` = clock |
| TC-40-035 | `engine_migrate_unfavorites_old_and_keeps_date` | `migrate_manga` with fakes | `replace=true`: `update_favorite(old,false)`; `date_added` = old's |
| TC-40-036 | `engine_missing_new_source_is_silent_error` | `migrate_manga` | `NewSourceMissing`, no writes |
| TC-40-037 | `engine_fetch_failure_writes_nothing` | `migrate_manga` | E-40-75: no repo calls |
| TC-40-038 | `engine_tracks_copied_even_with_zero_flags` | `migrate_manga` | E-40-68: tracks inserted |
| TC-40-039 | `engine_drops_null_migrated_track` | `migrate_manga` | E-40-83 |
| TC-40-040 | `engine_custom_cover_only_when_both` | `migrate_manga` | E-40-71, E-40-72 |
| TC-40-041 | `engine_order_dependency` | `migrate_manga` with a recording fake | call order: sync, chapters, categories, tracks, delete downloads, unfavorite (if replace), cover, favorite flip |
| TC-40-042 | `engine_atomic_on_failure` (after fix) | `migrate_manga` with failing fake | E-40-86: no partial favorite loss |

Integration / GTK tests (manual or `gtk4` test harness, optional):

| ID | Scenario | Steps | Expected |
|---|---|---|---|
| TC-40-101 | Browse source empty-error actions | open a source with a failing fake repo | three actions in order; retry refreshes |
| TC-40-102 | Snackbar retry | items present, paging error | toast bar; retry triggers reload |
| TC-40-103 | Extensions grouping in UI | fixture repo with 3 languages + 2 installed + 1 update | headers and order as in TC-40-006 |
| TC-40-104 | Install busy row | fake installer emits Pending, Downloading, Installing, Finished | row shows spinner, text per step, buttons Cancel only; then Settings |
| TC-40-105 | Cancel install | start install, click Cancel | row returns to Install; map cleared |
| TC-40-106 | Trust dialog flow | untrusted fixture: click row, trust | extension becomes Installed |
| TC-40-107 | Trust dialog uninstall path | untrusted fixture: click row, uninstall-button | extension removed |
| TC-40-108 | Private extension confirm | fixture without installed package: long-press | confirm dialog; confirm removes |
| TC-40-109 | Details: enable all | details page, enable all | every source switch on |
| TC-40-110 | Details: uninstall pops | uninstall from details | page pops after package removal event |
| TC-40-111 | Language filter toggle | toggle a language | group list updates after debounce; prefs saved |
| TC-40-112 | Migrate dialog flags | old manga with custom cover and downloads | four checkboxes in order; defaults all on |
| TC-40-113 | Migrate copy vs migrate | run each on a fixture | copy keeps old favorite; migrate removes it |
| TC-40-114 | Migrate navigation from S17 | run migrate from per-source search | popUntilRoot, Browse opens, new details page |
| TC-40-115 | Migrate navigation from S16 | run migrate from global search with a MangaScreen below | pop to MangaScreen then push new details (or replace) |
| TC-40-116 | Global search progress bar | progress 0, 3/10, 10/10 | visible only at 3/10 |
| TC-40-117 | Chip filter behaviour | change chip without query | results unchanged (D-UI-10) |
| TC-40-118 | Keyboard map | run each shortcut in section 9 of p1-ui-parity.md | action occurs |
| TC-40-119 | Responsive | window 360 px wide | no horizontal page scroll; chips and cards scroll only inside their strip |
| TC-40-120 | Config migration | start with Aniyomi-style preference values exported to TOML | same display mode, hidden and pinned sets |

Fixtures needed (under `tests/fixtures/browse/`, to be created in the Kurobayomi repo):
- `extensions_repo.json`: an index with 3 languages, 6 extensions (one with update, one NSFW, one obsolete, one without sources, one with `lang` empty). Fields as in p1-data-and-config.md section 1.
- `installed_extensions.json`: 2 installed (one shared, one with icon missing), 1 untrusted.
- `sources.json`: 8 sources across `en`, `ja`, `all`, `other`, including one stub, one local (id 0), one with `supportsLatest = false`, one with blank name.
- `manga_favorites.json`: 12 titles with mixed case and an empty title.
- `chapters_old.json` and `chapters_new.json`: old has read chapters up to 10.5 with one `-1` unrecognised read chapter; new has 1..12 and 5.5.
- `tracks.json`: 2 tracks, one matched by a fake enhanced tracker, one not matched.
- A fake `MigrationDeps` implementation that records calls (for TC-40-041).

Test data hygiene: no real manga titles or cover images; use generic names ("Manga A") as fixtures.

## 5. Coverage check against the scope

Every behavioural section of p1-behavior.md maps to at least one test or edge case:

| Behaviour section | Tests / edge cases |
|---|---|
| 1 Browse tab | TC-40-117, TC-40-118; R-40-08 |
| 2 Browse source | E-40-44..50, TC-40-101, TC-40-102, TC-40-119 |
| 3 Sources list | E-40-51..54, E-40-55, E-40-56 |
| 4 Sources filter | E-40-100, E-40-101, E-40-98, E-40-99 |
| 5 Extensions tab | E-40-01..30, TC-40-001..009, TC-40-103 |
| 6 Install state machine | E-40-21..24, R-40-02, R-40-03, TC-40-011, TC-40-012, TC-40-104, TC-40-105 |
| 7 Extension details | E-40-31..43, TC-40-013, TC-40-014, TC-40-109, TC-40-110 |
| 8 Language filter | E-40-100, TC-40-111 |
| 9 Source prefs host | TC-40-120 (config) and manual check of S13 |
| 10 Global search | E-40-57..62, TC-40-116 |
| 11 Migration picker | E-40-95, E-40-96, TC-40-031 |
| 12 Migration favorites | E-40-93, E-40-94, TC-40-026 |
| 13 Migration search | E-40-63..67, TC-40-025, TC-40-114, TC-40-115 |
| 14 Migration dialog | E-40-68..70, E-40-97, TC-40-015..019, TC-40-112, TC-40-113 |
| 15 Migration engine | E-40-71..90, TC-40-020..024, TC-40-033..042 |
| 16 Drops | none (no behaviour to test; checked by code review) |
| 17 Concurrency | R-40-01..11 |
| 18 Message keys | checked by a lint: every key used in the Rust code exists in the i18n table (TC-40-121, to add) |
| 19 Interactions | covered by the owning areas |
