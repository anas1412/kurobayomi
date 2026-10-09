# Area 47 (part 1) — edge cases, failure modes, races, tests

Upstream test coverage: none of the 18 scope files has a unit test (`find … src/test` in the repo lists 8 test files; none reference `LibraryExporter`, `getCategoriesLabel`, `SettingsSearchScreen`, `AuthenticatorUtil`, `ChapterCache`, `CrashLogUtil`, `*Preferences`, or `ReadingMode`). Nothing is ported as-is. Section 13 lists new tests. Related upstream tests in other areas: `domain/src/test/java/tachiyomi/domain/library/model/LibraryFlagsTest.kt` (area 08), `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt` (area 35, used by the About update check).

Each edge case has: trigger, observed Android result (file:line), and the required Rust behaviour.

---

## 1. Preference store (T-47a-001, T-47a-009)

| # | Trigger | Android result | Rust behaviour |
|---|---|---|---|
| 1.1 | Stored value of the wrong JSON type (e.g. string where int expected) | `getInt` throws in SharedPreferences cast → app-level crash risk; Compose reads crash | `get()` returns default, logs one `warn` per key |
| 1.2 | Enum name removed in a later version (e.g. `pref_app_theme = "MOCHA"` after dropping the palette) | `enumValueOf` throws `IllegalArgumentException`; `getEnum` returns default (`PreferenceStore.kt:27-40`) | same: unknown name → default; no rewrite until the next `set` |
| 1.3 | Two sets in quick succession (slider drag writes 15 values) | each `set` commits on the main thread | writer coalesces within 50 ms; the cache holds the latest value immediately; last write wins |
| 1.4 | Crash between `set` (cache updated) and the DB write | Android `apply` is async too; lost write | the write is in the unbounded channel; at-most-50 ms loss on crash (documented); acceptable for settings |
| 1.5 | `delete()` of a key that does not exist | no-op | no-op; no error |
| 1.6 | Backup import includes `__PRIVATE_` keys | excluded by key prefix (area 21 checks `Preference.isPrivate`) | ignored; test fixture §13 F-3 |
| 1.7 | Backup import includes `__APP_STATE_` keys | excluded from backups | ignored |
| 1.8 | Migration of the shared keys (task 030): Android installs that wrote `default_chapter_display_by_name_or_number` for episodes | episode display and chapter display read the same key | Rust reads `default_episode_display_by_name_or_number` for episodes; on first start, if the new key is unset and the shared key is set, copy the shared value into the new key, then keep the shared key for chapters only |
| 1.9 | Same for `default_chapter_sort_by_ascending_or_descending` | shared | as 1.8 for episode sort |
| 1.10 | `setSeasonSettingsDefault` writes unseen filter into the downloaded-filter key (`LibraryPreferences.kt:331`) | the downloaded filter for seasons takes the unseen value | Rust writes the downloaded value into the downloaded key; migration does not copy the wrong value (values are re-derived from the entry on next open) |
| 1.11 | Database file missing `prefs` table on first run | — | created with `CREATE TABLE IF NOT EXISTS` before first read |
| 1.12 | Concurrent writers (two Rust processes) | — | single-instance guard (existing `Application` flags); the second process forwards and exits |

## 2. Preference item rendering (T-47a-002)

| # | Trigger | Android result | Rust behaviour |
|---|---|---|---|
| 2.1 | `ListPreference` value not in `entries` (e.g. `download_slots = 6` from a restored backup) | subtitle `subtitle.format(null)` → **"null"** (`Preference.kt:84-86`, `subtitleProvider`) | subtitle = empty string; the list still shows the other options; the value is not rewritten |
| 2.2 | Same for `default_anime_category` / `default_category` after deleting the category | subtitle "null" | subtitle shows "Always ask" and the stored id is reset to -1 when the screen opens (task 020 acceptance) |
| 2.3 | Same for `reader_navigation_mode_pager = 6` | subtitle "null" | subtitle shows "Default" (index 0) |
| 2.4 | Switch `onValueChanged` returns false | value unchanged, row shows the old state | same |
| 2.5 | Two rows with the same title (search highlight is title-based) | `findHighlightedIndex` finds the first | same rule; titles are unique per screen except "Crop borders" (paged and long strip) — the highlight key includes the group title in Rust |
| 2.6 | Group with `enabled = false` and search | group excluded from search | same |
| 2.7 | `MultiSelectListPreference` with an empty set | subtitle "None" | "None" |
| 2.8 | Slider integer division: stored `pref_reader_flash_duration = 150` | `value = 150 / 100 = 1`, subtitle "150 ms" | slider position 1; subtitle "150 ms"; dragging writes multiples of 100 |
| 2.9 | Webtoon side padding percent in a locale using a comma decimal | `NumberFormat` locale-dependent | format with the locale decimal separator (glib) |

## 3. Category label helper (T-47a-005) — exact combinations

Let `allCategories` = [A(order 0), B(order 1), C(order 2)] unless stated. Output is `Include: <x>\nExclude: <y>`.

| # | included | excluded | Include text | Exclude text | Reason (branch) |
|---|---|---|---|---|---|
| 3.1 | [] | [] | All | None | include: none of branches 1,2 → `allExcluded` false → else All; exclude: empty → None |
| 3.2 | [A] | [] | A | None | branch 1 (some, not all) |
| 3.3 | [A,B,C] | [] | All | None | branch 2 |
| 3.4 | [] | [A,B,C] | None | All | branch 3 (allExcluded) |
| 3.5 | [A] | [B] | A | B | branch 1; exclude joined |
| 3.6 | [A,B] | [A] | A, B | A | included size 2 ≠ 3 |
| 3.7 | [] | [A] | All | A | excluded size 1 ≠ 3 |
| 3.8 | [] | [] with **zero categories** | All | None | branch 2 (0 == 0) |
| 3.9 | [A,B,C] | [A,B,C] | All | All | overlap: branch 2 for include; allExcluded for exclude |
| 3.10 | [99, A] | [] | A | None | id 99 not in categories → dropped |

Ordering: categories in both lists are sorted by `order` before joining (`Commons.kt:24-28`), so `[B, A]` as stored prints `A, B`.

## 4. Reader enable/disable rules (T-47a-041)

Each row in behaviour §4 table is a test:

| # | Setting | Input | Expected enabled |
|---|---|---|---|
| 4.1 | Paged invert tap zones | tap zones 0 | true |
| 4.2 | Paged invert tap zones | tap zones 5 | false |
| 4.3 | Paged landscape zoom | scale 1 | true |
| 4.4 | Paged landscape zoom | scale 2 | false |
| 4.5 | Paged pan | tap zones 5 | false |
| 4.6 | Paged invert split | split false | false |
| 4.7 | Long strip invert tap zones (webtoon nav) | webtoon nav 5, paged nav 0 | false (uses webtoon value) |
| 4.8 | E-ink interval | flash off | false |
| 4.9 | Volume key invert | volume keys false | false |
| 4.10 | Split/rotate mutual exclusion | set split true while rotate true | rotate becomes false (value write side effect) |
| 4.11 | Cutout row | (dropped) | not present |

## 5. Settings search (T-47a-004)

| # | Query | Expected |
|---|---|---|
| 5.1 | "" (empty) | no results list (`SearchResult` returns) |
| 5.2 | "  " (spaces only) | matches rows whose title or subtitle contains two spaces; typically none; shows "No results found" |
| 5.3 | "PAGE" | case-insensitive match on "Page" rows (Show page number, Paged …) |
| 5.4 | "Turkish İ" (dotless i handling) | Android `contains(ignoreCase = true)` uses Unicode simple folding; Rust: compare `to_lowercase()` on both sides and a second pass with `to_uppercase()` for `ı`/`i` |
| 5.5 | "wifi" | "Only on Wi-Fi" in downloads and "Automatic updates device restrictions" subtitle ("Restrictions: Only on Wi-Fi") → both results, in index order |
| 5.6 | "e" (a single letter) | more than 10 hits → only the first 10 in index order |
| 5.7 | "Debug info" | found: Advanced screen, link row (not info; not excluded) |
| 5.8 | disabled row "Pure black dark mode" in light mode | not found |
| 5.9 | info row text "Only works if the current chapter…" | not found (info rows excluded) |
| 5.10 | tracker row (empty title) | not found |
| 5.11 | player query | index is player screens only; breadcrumb has no "Player settings" prefix |

## 6. Clear-database (T-47a-075)

Fixtures: a SQLite DB with `animes` rows as below.

| # | Scenario | Expected SQL effect |
|---|---|---|
| 6.1 | Parent P (favorite 0, parent null, fetch_type Seasons), child C1 (favorite 0), child C2 (favorite 1) | `getDeletableChildren(P)` = [C1 to ids, C2 to orphaned]; delete `animeIds` = [P, C1]; orphaned = [C2] minus animeIds = [C2]; `removeParentIdByIds([C2])` sets C2.parent_id NULL; P and C1 gone; C2 remains |
| 6.2 | Grandchild G under C1 (favorite 0, fetch_type Seasons) | recursion adds G to ids; deletion removes P, C1, G |
| 6.3 | Grandchild G under C1, but G is favorite 1 | G goes to orphaned; parent_id of G set NULL; not deleted |
| 6.4 | Library anime (favorite 1, parent null) under source S | not in `animedeletableView`; never deleted |
| 6.5 | Manga in library (favorite 1) in source S | not in `getSourceIdsWithNonLibraryManga`; not deleted |
| 6.6 | Source selected contains both deletable and in-library rows | only non-library rows deleted |
| 6.7 | Select all, then invert | invert yields empty selection; button disabled |
| 6.8 | Delete confirmed, then the list refreshes (subscribe) | row disappears; if none left → "Nothing to clear" |
| 6.9 | `removeResettedHistory` | removes history rows with `last_read = 0` (manga) and `last_seen = 0` (anime) only |
| 6.10 | Source icon for a stub source (not installed) | `isStub = true`; icon placeholder; still deletable |
| 6.11 | Confirm dialog OK pressed twice quickly | second press: selection already cleared → no-op delete (`ids` empty) — must not error; Rust disables the button while the delete future is in flight |
| 6.12 | DB error during delete | Android: exception propagates (no catch); Rust: toast "Could not delete entries" (Proposal) and selection kept |

## 7. Downloads (T-47a-044..052)

| # | Scenario | Android result | Rust behaviour |
|---|---|---|---|
| 7.1 | Speed-limit dialog opened, then value changed elsewhere (e.g. another window) | `currentSpeedLimit` was captured once (`SettingsDownloadScreen.kt:63`); dialog shows the old value; OK overwrites with it | Rust reads the current value when the dialog opens; if it changes while open, the dialog updates its field |
| 7.2 | Speed limit typed as 0 | subtitle "Off" | same |
| 7.3 | Speed limit negative (typed "-100") | `OutlinedNumericChooser` min 0 clamps | clamp to 0 |
| 7.4 | Very large speed limit (no max) | accepted | accepted, capped in the rate limiter at u32::MAX KiB/s |
| 7.5 | `download_slots = 0` (imported) | subtitle "null" (see 2.1) | subtitle empty; treated as 1 by the scheduler (area 25) |
| 7.6 | Category id present in both include and exclude sets | TriStateListDialog cannot produce this (one state per row); imported data may contain it | exclude wins (matches the Android note "Entries in excluded categories will not be downloaded even if they are also in included categories") |
| 7.7 | Auto-download new episodes off, but unseen-only on | unseen-only row disabled (hidden) | same; the stored value is kept |
| 7.8 | External downloader selection with an uninstalled app | list shows only installed packages; stored value for a missing package shows "null" | removed group (task 051); value ignored |

## 8. Tracking (T-47a-053..059)

| # | Scenario | Android result | Rust behaviour |
|---|---|---|---|
| 8.1 | Login fails due to network timeout | `checkLogin` catches `Throwable`, calls `tracker.logout()` → credentials wiped; error toast | Proposal: only an authentication failure (HTTP 401/403 or the tracker's "invalid credentials" error) wipes credentials; network failures keep them and show the error (requires a typed error from area 27) |
| 8.2 | Login succeeds but a later call fails | not handled here | not handled here (area 27) |
| 8.3 | Double-click on Login | `processing` guard; button disabled | same; the second click is ignored |
| 8.4 | Dialog closed while login in flight | `onDismissRequest` from IO thread; result may still write `inputError` on a disposed dialog | result dropped when the dialog is gone (weak reference) |
| 8.5 | Enhanced tracker with accepted source installed then removed | row moves to "not installed" info text on next composition | Rust recomputes on source change (signal from area 14) |
| 8.6 | Tracker login opens the browser but the redirect never arrives | no timeout on the screen | pending state shows "Waiting for browser…" with a Cancel link that clears the pending flag (Proposal) |
| 8.7 | Username with leading/trailing spaces | not trimmed | trimmed before storing (Proposal) |
| 8.8 | Password visible toggle while the app is locked | — | the toggle state is not persisted |

## 9. Security (T-47a-063..068)

| # | Scenario | Android result | Rust behaviour |
|---|---|---|---|
| 9.1 | No biometric and no device credential | `isAuthenticationSupported()` false; switch disabled; `authenticate()` returns true (`AuthenticatorUtil.kt:54-57`) | no method available → switch disabled; the gate returns true (matches) but the lock never engages |
| 9.2 | Lock turned on, then the device loses its credential | `authSupported = false` (`remember`); `useAuth` stays true; lock-when-idle row hidden (enabled = authSupported && useAuth) | lock state stored; the lock UI shows "Unlock method unavailable — disable lock in settings" on startup (Proposal) |
| 9.3 | Authentication error (user cancels) | `onAuthenticationError` → toast `errString`, resume false → value does not change | same; no toast for "user canceled" (Proposal) |
| 9.4 | Lock-when-idle `-1` (never) | row shows "Never" | same |
| 9.5 | NSFW switch toggled off | authentication also required | same (gate both directions) |
| 9.6 | Passphrase lock: wrong passphrase 5 times | not applicable (biometric) | Proposal: 1 s delay after each failure; no lockout counter |

## 10. Advanced (T-47a-069..087)

| # | Scenario | Android result | Rust behaviour |
|---|---|---|---|
| 10.1 | UA string with a control character | `Headers.Builder().add` throws IllegalArgumentException → error toast, return false | same check; reject |
| 10.2 | UA string blank | EditText `canBeBlank = false` (default) → confirm disabled | same |
| 10.3 | Reset UA when already default | row disabled (hidden) | hidden |
| 10.4 | DoH provider Mullvad, URL with leading space | OkHttp trims leading whitespace in `toHttpUrl` (assumed; verify with a unit test) | Rust constant is trimmed; test asserts it parses |
| 10.5 | Clear WebView data while a WebView is open | Android deletes `app_webview` recursively; open WebView may recreate files | Rust clears the WebKit data manager first and then removes the directory; open pages are reloaded |
| 10.6 | Display profile picked, then the file deleted | `takePersistableUriPermission` throws `SecurityException` in the picker callback (no try/catch, `SettingsAdvancedScreen.kt:358`) → crash | Rust: removed (task 083) |
| 10.7 | Hardware bitmap threshold changed while images are loading | applies to new decodes | applies to new decodes |
| 10.8 | Hardware bitmap threshold row when device texture limit == 2048 | row disabled (`DEVICE_TEXTURE_LIMIT > SAFE_TEXTURE_LIMIT` false) → hidden | hidden |
| 10.9 | Label mismatch: option 0 labelled "Default (D)" but stored default is 2048 | when D = 4096, the "Default" label is on 4096 while the stored default is 2048 | Rust labels the **stored default** as "Default (2048)" and shows the device maximum as a separate "Maximum (D)" entry (Proposal) |
| 10.10 | Data saver mode set to RESMUSH_IT | image format row disabled | same |
| 10.11 | Verbose logging enabled | toast requires restart; logger filter needs restart | live reload via `tracing_subscriber::reload` (task 070) |
| 10.12 | Crash-log export with no crash | file contains debug info and optional extension list | same |
| 10.13 | Crash-log export when the cache directory is not writable | `createFileInCacheDir` throws; toast "Failed to get logs" | toast "Failed to get logs" (same text kept) |
| 10.14 | Revoke trust during extension load | not handled | Rust: revoke takes effect on the next load (area 18/19) |

## 11. Data, storage and export (T-47a-088..092)

| # | Scenario | Android result | Rust behaviour |
|---|---|---|---|
| 11.1 | Storage directory deleted outside the app | subtitle "Invalid location: <uri>" | same text; picker opens |
| 11.2 | Storage location is a file, not a directory | `UniFile.fromUri` returns a file; subtitle shows its path | Rust refuses non-directories with "Invalid location" |
| 11.3 | Storage picker returns a `content://`-like URI (portal) | SAF handles it | `gio::File::path()` is None; treat as invalid, show the URI (task 088) |
| 11.4 | Cache size on a huge cache (many GB) | computed during composition (`SettingsDataScreen.kt:294-295`), blocking the UI | computed off the GTK thread; row shows "Calculating…" |
| 11.5 | Clear cache while a chapter is being read | DiskLruCache removes entries; reader may fail to load pages | reader falls back to network (area 43) |
| 11.6 | Clear cache: journal file | not deleted (journal names excluded, `ChapterCache.kt:182`) | same rule |
| 11.7 | Backup interval changed while a backup runs | `setupTask` replaces the periodic work | in-process scheduler replaces the next-run timestamp |
| 11.8 | "Last automatically backed up" never happened | `relativeTimeSpanString(0)` → "Never" | same |
| 11.9 | Export opened before favorites load finished | `favorites` empty; exporting writes an empty file and shows "Library Exported" | Save disabled until favourites are loaded (task 091) |
| 11.10 | Export with Title unchecked | all options false → rows are empty → file is N-1 times `\r\n` | Save disabled when Title is unchecked (Proposal) |
| 11.11 | Title with embedded quotes, commas, newlines | quoted and doubled quotes | same; see section 12 |
| 11.12 | Export with zero favourites | empty file, success toast | same |
| 11.13 | Export file path not writable | `openOutputStream` throws → coroutine exception | toast "Could not export library" (Proposal text) |
| 11.14 | Two exports at once | two coroutines write different URIs | serialised: the second Save waits for the first |

## 12. CSV golden examples (for tests)

Input favourites (order preserved, anime first):
1. anime, title `Ghost, The "Shell"`, author `Kenji`, artist null
2. manga, title `Line1\nLine2`, author null, artist `Ann`
3. manga, title `Plain`, author ``, artist `Bo`

Options all true → output (bytes, `\r\n` line endings, no trailing newline):
```
"Ghost, The ""Shell""",anime,Kenji,
"Line1
Line2",manga,,Ann
Plain,manga,,Bo
```
(line 1 ends with `,` because artist is null → empty field; line 3 has an empty author.)

Options title=true, type=false, author=false, artist=false → only titles:
```
"Ghost, The ""Shell"""
"Line1
Line2"
Plain
```

Options title=false (all false) → output is `\r\n\r\n` (two rows, each empty, joined by one `\r\n`): exactly the 2-byte separator `\r\n` for 2 rows.

Options title=true, type=true only → rows `"…",anime` / `…,manga` / `Plain,manga`.

## 13. Test plan

### 13.1 New unit tests (Rust)

| ID | Test | Covers |
|---|---|---|
| U-47-01 | Store round-trip for each `Pref` type (bool, i32, i64, String, BTreeSet, enum name) | 1.x |
| U-47-02 | Bad JSON → default + warn (once) | 1.1 |
| U-47-03 | Unknown enum name → default | 1.2 |
| U-47-04 | Coalesced writes: 15 sets in 10 ms → 1 transaction with the last value | 1.3 |
| U-47-05 | `reset()` deletes; `get()` returns default | 1.5 |
| U-47-06 | Private keys excluded from backup export | 1.6, T-47a-009 |
| U-47-07 | Migration of shared keys (fixture F-1) | 1.8, 1.9 |
| U-47-08 | Season default write goes to the downloaded key (fixture F-1) | 1.10 |
| U-47-09 | Category label: the 10 cases of section 3 | 3.1–3.10 |
| U-47-10 | Reader enable rules: 4.1–4.10 | 4.x |
| U-47-11 | Search: 5.1–5.11 with the fixture index | 5.x |
| U-47-12 | Clear-DB anime deletion plan (6.1–6.4) on fixture F-2 | 6.1–6.4 |
| U-47-13 | Clear-DB: library rows never selected (6.4–6.5) | 6.4, 6.5 |
| U-47-14 | Speed-limit clamp (7.3) | 7.3 |
| U-47-15 | Category include/exclude precedence (7.6) | 7.6 |
| U-47-16 | Tracker login: auth failure wipes credentials, network failure does not (8.1) | 8.1 |
| U-47-17 | `authenticate` returns true when unsupported (9.1) | 9.1 |
| U-47-18 | UA validation (10.1, 10.2) | 10.1, 10.2 |
| U-47-19 | DoH URL table parses (10.4) | 10.4 |
| U-47-20 | Hardware bitmap option generation for D = 2048, 4096, 8192, 5000 (expected lists: [2048]; [4096,3072,2048]; [8192,7168,6144,5120,4096,3072,2048]; [5000,4096,3072,2048]) | 10.8, 10.9 |
| U-47-21 | Date format labels for each pattern and a fixed "today" | 16 (behaviour §5) |
| U-47-22 | CSV golden examples (section 12) | 11.x, 12 |
| U-47-23 | Export Save disabled with Title unchecked (11.10) and before favourites load (11.9) | 11.9, 11.10 |
| U-47-24 | Version string formats (debug, preview, stable) | 13 (behaviour §13) |
| U-47-25 | Update-check result mapping (NewUpdate, NoNewUpdate, OsTooOld, error) | 94 |

### 13.2 Integration (GTK) tests

| ID | Test | Assertion |
|---|---|---|
| I-47-01 | Open every settings screen (ids from `screens()`) | no panic; row count equals the visible rows in behaviour §3–§17 |
| I-47-02 | Toggle "Show reading mode" | pref persisted; row title unchanged |
| I-47-03 | Set interval Daily then Off | job scheduled then cancelled (in-process scheduler state) |
| I-47-04 | Hide-rows: set "Automatic updates" to Off | device restrictions row hidden |
| I-47-05 | Search "wifi" | two results with breadcrumbs |
| I-47-06 | Tracking login with a fake tracker (success and auth failure) | "Logged in" toast; credentials wiped on auth failure only |
| I-47-07 | Clear manga DB on fixture | selected source rows removed; list refreshes |
| I-47-08 | Export CSV to a temp file | file bytes equal the golden output |

### 13.3 Fixtures

| ID | File (to create under `tests/fixtures/settings/`) | Content |
|---|---|---|
| F-1 | `android_prefs_sample.xml` | a SharedPreferences XML with `default_chapter_display_by_name_or_number="1"`, `default_chapter_sort_by_ascending_or_descending="1"`, `download_slots="6"`, `pref_app_theme="MOCHA"`, `__PRIVATE_pref_mangasync_password_1="x"` |
| F-2 | `clear_db.sqlite` | tables `animes` (rows: P, C1, C2, G from section 6), `mangas` (one favourite, one non-library), `history`, `animehistory` |
| F-3 | `backup_with_private.json` | preference list incl. one `__PRIVATE_` key and one `__APP_STATE_` key |
| F-4 | `categories_3.json` | categories A(0), B(1), C(2) |
| F-5 | `favorites_csv.json` | the three favourites of section 12 |
| F-6 | `source_index.json` | the search index flattened from the settings tree (for test 5.x) |

---

## 14. Known bugs in the Android source (carried into the plan as fixes or notes)

| # | Location | Bug | Rust treatment |
|---|---|---|---|
| B-1 | `SettingsDownloadScreen.kt:63` | `currentSpeedLimit` captured once | fixed (7.1) |
| B-2 | `ClearAnimeDatabaseScreen.kt:122` | invert action labelled "Select all" | fixed (label "Select inverse") |
| B-3 | `LibraryPreferences.kt:205-213` | episode and chapter display share key `default_chapter_display_by_name_or_number` | fixed by migration (1.8) |
| B-4 | `LibraryPreferences.kt:215-223` | episode and chapter ascending/descending share key | fixed (1.9) |
| B-5 | `LibraryPreferences.kt:331` | season downloaded-filter default written from unseen field | fixed (1.10) |
| B-6 | `SettingsAdvancedScreen.kt:358` | display-profile URI permission without try/catch | removed (task 083) |
| B-7 | `SettingsAdvancedScreen.kt:180; SettingsReaderScreen.kt:136; SettingsAppearanceScreen.kt:149` | literal English strings in a translatable UI | Rust uses constants; no untranslated strings in the new UI |
| B-8 | `SettingsAppearanceScreen.kt:149` | literal "Navigation Style" title | Rust uses a named string key |
| B-9 | `SettingsLibraryScreen.kt:262-278`, `SettingsDownloadScreen.kt:278-306` | category subtitle lambdas compute labels inside composition | Rust computes labels on change |
| B-10 | `TriStateListDialog.kt:217-223` | all three states use the content description "Disabled" | Rust: "Not selected", "Included", "Excluded" |
| B-12 | `SettingsTrackingScreen.kt:349-364` & `AuthenticatorUtil` | `checkLogin` logs out on any error | Proposal (8.1) |
| B-13 | `DohProviders.kt:136` | leading space in Mullvad URL | trimmed (10.4) |
| B-14 | `SettingsAdvancedScreen.kt:366-375` `hardwareBitmapThreshold` labelling | "Default" on the device max, stored default is 2048 | Proposal (10.9) |
| B-15 | `SettingsDataScreen.kt:294-295` | disk walk during composition | off-thread (11.4) |

---

## 15. Concurrency and race summary

| Race | Where | Mitigation |
|---|---|---|
| Two `set` calls from a slider drag | `Preference` writes | coalesced writer (1.3) |
| Dialog closed while a tracker login is running | `TrackingLoginDialog` | weak reference; drop result (8.4) |
| Speed-limit dialog stale value | `SettingsDownloadScreen` | read on open; live update (7.1) |
| Clear DB double tap | `ClearDatabaseScreenModel` | disable button while in flight (6.11) |
| Export before favourites loaded | `getExportGroup` | disable Save until loaded (11.9) |
| Restore started twice | `BackupRestoreJob.isRunning` check | same check; toast (task 092) |
| Library update setup from two rows at once | `setupTask` | the scheduler serialises updates (area 26) |
| Search results arriving after navigation | `produceState` | Rust: results computed synchronously from the in-memory index |
| Highlight key left set after navigation | `SearchableSettings.highlightKey` | cleared after the first screen consumes it (task 007) |
