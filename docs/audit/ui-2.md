# UI page audit 2: settings pages, sheets, dialogs, popovers, onboarding, crash and update screens

Date: 2026-10-11. Method: reading code, test names and callers only. Cargo was not run, so no test was executed; "tests exist" means a test of that name is in the source. A page counts as working only when the app window reaches it. Nothing was looked at on screen.

Scope: every id in `src/settings_ui/route.rs` `handles()` and `src/pref_model/pages.rs` `IDS`, the spec fallbacks in `src/settings.rs`, and every sheet, dialog and popover the user can reach (library and entry sheets, category pickers, delete dialogs, extension info, source settings, tracking dialogs, backup and restore, onboarding, crash and update screens).

Counts are per row of the master table (54 rows).

## Summary

| Verdict | Count |
|---|---|
| done (new design, real data, working actions) | 23 |
| partial (old look, dead rows or placeholder text) | 23 |
| missing (pure placeholder) | 5 |
| lib_only (built, not reachable) | 2 |
| not_applicable (dead, superseded) | 1 |

What the user saw is real, and the cause is the same in each case:

1. **Global search still shows the mockup in one path.** Library, "Search globally" (empty library search) calls `open_global_search` (`src/library/mount.rs:168`), which does `ui::open("global_search")` and parks nothing. `browse::search_screen::body` returns `None` when no intent, no route and no entry page is waiting (`src/browse/search_screen.rs:366-385`), so `screens.rs:26` falls through to `entry::global_search` (`src/entry.rs:657`): "Source A/B/C", a progress bar fixed at 0.6, eight "Title n" cards per source, "No results found". Opened from Browse, an entry page or a deep link, the page is the real one. Fix is one call (S).
2. **The tracking sheet and the track search are mockups, and the entry page opens them.** `ui::open("tracking_sheet")` is called from `entry_core/route.rs:689,1155` and `entry_core/anime/route.rs:1064,1119`. It builds `entry::tracking` (`src/entry.rs:723`): AniList / MyAnimeList / Kitsu, a button "Title 1", "Watching", "12 / 24", "8.0". The track search (`entry.rs:764`) is "Title 1..4", "Author name", fixed scores. Every tracking dialog is the spec dialog with an OK that does nothing. After a tracker login the sheet also opens by itself when you add an entry to the library. The logic exists in `src/entry_core/tracking/*` (pickers, writes, search, remove, summary) and `kuro_track::picker`, with no GTK caller.
3. **The settings pages show spec rows that have no typed twin.** `settings_ui/merge.rs` keeps every spec row without a twin, and `pref_model/spec_convert.rs:96-157` binds such a row to a made-up key `"<page>/<title>"`. The row looks live and changes nothing. Android-only rows (Tablet UI, cutout, Secure screen, battery optimization, Installer, legacy decoder, display profile) were meant to be dropped (`settings_ui/mod.rs` header comment) but no code drops them. One more is not Android-only: the Downloads page shows "Always use external downloader for anime" bound to a fake key while the runtime reads `DownloadPrefs::use_external_downloader` (`misc/runner.rs:96`, `updates/updates_screen.rs:159`), so the switch has no effect. On Tracking, "Show next episode's airing time " has a trailing space in the spec, so the typed twin does not match and the page shows the row twice (`spec/settings.json:2070`, `settings_ui/tracking.rs:379`).
4. **The App theme picker is not on Appearance** (known from `p0-14.md`): `theme::picker::theme_cards` and `pref_model::widgets::app_theme::strip` have no caller (chips are skipped by the spec converter).
5. **The player settings pages are complete but the player ignores almost all of them** (`src/player_view/mod.rs:75-83` builds `PlayerPrefs::default()` and reads only the remembered decoder and volume; see `p0-14.md` finding 5). Those pages are marked partial for that reason, not for their look.
6. **Dead code that looks like a screen**: `entry::browse`, `browse_source`, `seasons`, `migrate_entries` (`src/entry.rs`) and `settings.rs` `cards/editor/code_editor/text_dump/licenses` are unreachable while data is installed, but they still hold "Source A", "Title n" and "Apache License" strings. `settings_ui/menu.rs` (old settings main menu) has no caller; `pref_model::shell` replaced it.
7. **Left of the redesign**: onboarding (own layout, `title-1`, loose labels, no column), pref dialogs repeat their title, global search keeps a title row plus a separate entry row, the code editor header says "file.lua", About and Help point to aniyomi.org and Aniyomi's Discord and GitHub.

Smallest root-cause fixes, in order of effect:
- One `ANDROID_ONLY` title list in `Registry::build` (`pref_model/spec_convert.rs`) removes seven dead rows on five pages (S).
- Compare trimmed titles in `merge.rs` (or trim in `spec_convert::common`) removes the duplicate Tracking row (S).
- Park a `BrowseRoute::GlobalSearch` in `library/mount.rs:168` (S).
- Add the external downloader rows as typed rows, or drop them and the pref reads (S).
- Until the tracking sheet is built, make the Track button open Settings > Tracking and stop the auto-open (S).

## Master table, worst first

Size: S under half a day, M one to two days, L more.

| # | Page or screen | Verdict | Module | Real or mock | Design | Actions | One-line fix | Size |
|---|---|---|---|---|---|---|---|---|
| 1 | tracking_sheet (entry tracking) | missing | `entry.rs:723` | mock | spec chrome | none work | Build the sheet from `entry_core/tracking/*` | L |
| 2 | tracker_search | missing | `entry.rs:764` | mock | spec chrome | Track button does nothing | Build over `entry_core/tracking/search.rs` | L |
| 3 | tracking dialogs (status, progress, score, start and finish date, remove) | missing | `ui::show_dialog` over spec | mock | old modal | OK does nothing | Real pickers from `kuro_track::picker` | L |
| 4 | global_search, opened from Library | missing | `entry.rs:657` | mock ("Source A/B/C", "Title n") | old | cards open `Title n` mock entries | Park the route in `library/mount.rs:168`; delete the mock arm | S |
| 5 | webview (Open in WebView, source menu) | missing | `entry.rs:797` | mock ("https://example.org/", "Web content") | old | back and forward do nothing | Interim: open the URL with `gtk::UriLauncher` in `handoff::open_webview`; real engine later | S, L |
| 6 | Appearance, App theme row | lib_only | `theme/picker.rs:200`, `widgets/app_theme.rs` | real widget, no caller | n/a | n/a | Add a `Custom` row in `appearance::page_rows` | S |
| 7 | new_update screen | lib_only | `settings_ui/about.rs:132 show_update` | real | modal, `title-1` | Download, Not now work | Set `KB_UPDATE_REPOSITORY` and the `updater` feature in release builds; nothing in the repo sets them | S |
| 8 | global_search, migrate_search (real page) | partial | `browse/search_screen.rs`, `browse/pages.rs:538` | real | title row plus its own entry row, bare `ProgressBar`, plain boxes, icon-only arrow, `chip` class has no CSS | work | Put the entry in the header (`ui::page` rule), boxed list per source | M |
| 9 | settings_downloads | partial | `settings_ui/downloads.rs` | real, 2 dead rows | family | "Always use external downloader for anime" and "Downloader app preference" write fake keys | Type both rows over `DownloadPrefs` (`kuro-domain/src/download/prefs.rs:37-38`) | S |
| 10 | settings_advanced | partial | `settings_ui/advanced.rs` | real, 6 dead rows | family | Disable battery optimization, Don't kill my app!, Use legacy decoder, Custom display profile, Installer: no effect | `ANDROID_ONLY` list | S |
| 11 | settings_tracking | partial | `settings_ui/tracking.rs` | real | family | "Show next episode's airing time" twice, one dead | Trim titles in the merge | S |
| 12 | settings_appearance | partial | `settings_ui/appearance.rs` | real | family | "Tablet UI" dead; App theme missing (row 6) | `ANDROID_ONLY` list | S |
| 13 | settings_security | partial | `settings_ui/security.rs` | real | family | "Secure screen" list and its info line dead | `ANDROID_ONLY` list | S |
| 14 | settings_reader | partial | `settings_ui/reader/page.rs` | real | family | "Show content in cutout area" dead | `ANDROID_ONLY` list | S |
| 15 | Player pages: internal, gestures, decoder, subtitle, audio, torrents, advanced, custom_buttons | partial (8 rows, counted individually) | `settings_ui/player/*` | real UI | family | Values are stored; the live player reads only remember decoder and volume | Read the prefs in `player_view` (see `p0-14.md` 5) | L |
| 16 | settings_restore_backup | partial | `pref_model/backup_pages.rs:301,428` | real | family | The check uses `NothingInstalled`, so every source is missing and no tracker is logged in | Adapters over `ext_host::registry()` and the tracker host | M |
| 17 | onboarding | partial | `shell/onboarding.rs:140` | real | no column, `title-1`, bare labels, DropDowns, aniyomi.org links | Next, Back, folder, Finish, Restore work | Rebuild with `ui::column`, `heading`, boxed lists; new URLs | M |
| 18 | settings_about | partial | `settings_ui/about.rs:258` | real | family | Website, Discord, GitHub, Privacy, Help translate open Aniyomi's pages; Check for updates is a disabled twin in every default build | New URLs; set the update repository | S |
| 19 | settings_worker_info | partial | `settings_ui/worker_info.rs` | real page, empty registry | family | Always "-": no code calls `jobs::enqueue_unique/start/finish` | Register library update, backup and download jobs | M |
| 20 | settings_player_code_editor | partial | `settings_ui/player/scripts.rs:683` | real | header title is the spec's "file.lua", file name repeated in the body; syntax colours are hard-coded hex (`player/highlight.rs:19-25`) | save, Ctrl+S, unsaved dialog work | Title from the open file; colours from the theme | S |
| 21 | Pref dialogs (list, multi-select, edit text, slider, speed limit) | partial | `pref_model/widgets/dialog.rs:75,97` | real | window title and a `title-4` heading both show the title | work | Drop one of the two | S |
| 22 | settings_clear_manga_database, settings_clear_anime_database | partial | `settings.rs:285` | real | empty state is a bare label; confirm is the spec dialog | work | `ui::status` empty state | S |
| 23 | settings_backup_schema | partial | `settings.rs:272` | real | bare monospace label | Copy works | Boxed row like Worker info | S |
| 24 | settings_open_source_library_license | not_applicable | `settings.rs:26` | mock "Apache License" | n/a | unreachable; `licenses::detail_page` replaced it | Delete the arm | S |

## Done

| Page or screen | Module | Evidence |
|---|---|---|
| settings (shell, one and two pane) | `pref_model/shell.rs` | tests `t_47b_003_*`, `every_settings_page_builds_and_shows_rows` (`settings_ui/route.rs:508`, child process) |
| settings_search | `settings_ui/search.rs:132` | header entry, hint, "No results found"; route test above |
| settings_app_language | `pref_model/language.rs:44` | tests `t_47b_005`, `t_47b_007` |
| settings_library | `settings_ui/library.rs`, `library_update.rs`, `misc/interval.rs` | all spec rows have typed twins (checked by title) |
| settings_browse | `settings_ui/browse.rs` | all spec rows typed |
| settings_player (root) | `settings_ui/player/root.rs` | one split view, no nested sidebar (route test) |
| settings_player_editor (script list) | `settings_ui/player/scripts.rs:565` | create, rename, delete, empty state |
| settings_anime_extension_stores, settings_manga_extension_repos | `pref_model/stores_page.rs` | reducer and page tests; list, add, refresh, delete work; add is a floating button (old pattern), empty state has no button |
| settings_data_storage | `settings_ui/route.rs:333`, `storage.rs`, `usage.rs`, `cache.rs`, `library_export.rs` | live usage rows, folder chooser, cache clear, CSV export |
| settings_create_backup | `pref_model/backup_pages.rs:178` | column, headings, boxed lists, file dialog, job slot, host registered in `backup_host.rs:105` |
| settings_debug_info | `pref_model/debug_info.rs` | Linux rows, copy toasts; tests `t_47b_040`, `t_47b_041` |
| settings_open_source_licenses (list and detail) | `settings_ui/licenses.rs` | real data; `licenses.json` leaves out seven platform crates (file header) |
| Library filter, sort, display sheet | `library/view/sheet.rs`, `filter_tab.rs`, `sort_tab.rs`, `display_tab.rs` | writes each control at once; tests `a_sort_tap_flips...`, `the_display_tab_writes_every_control_immediately` |
| Library category picker, delete and name dialogs | `library/view/dialog.rs`, `delete_dialog.rs`, `name_dialog.rs` | real rows, confirm callbacks |
| Entry dialogs: categories, delete, duplicate, fetch interval, set default, intro length, scanlator filter, full cover | `entry_core/*_view.rs`, `fetch_interval_dialog.rs`, `scanlator_sheet.rs`, `anime/*` | all built from real state, no spec fallback |
| Entry chapter, episode and season settings sheets | `entry_core/route.rs:1235`, `anime/settings_view.rs`, `season_settings_view.rs` | real |
| Browse filter sheet, display mode, source options, extension trust, remove, migrate dialogs | `browse/filter_sheet.rs`, `dialogs.rs`, `manga_migrate_dialog.rs`, `anime/migrate_dialog.rs` | real |
| extension_details | `browse/ext_details_page.rs` | uninstall, source toggles, clear cookies, repo link; tests exist |
| source_preferences | `browse/source_prefs_screen.rs`, `source_prefs_page.rs` | rows come from the source |
| Updates and History dialogs | `updates/updates_dialog.rs`, `history_dialog.rs` | real |
| crash screen | `reporter.rs` | own process; copy, reveal, restart work; theme colours; no test (p0-01) |

## Per-page tables (not done)

Each table: module, real or mock, design, actions, fix.

### tracking_sheet, tracker_search, tracking dialogs (missing)

| | |
|---|---|
| Module | `src/entry.rs:723 tracking`, `:764 tracker_search`; dialogs from `spec/entries.json` through `ui::show_dialog` |
| Real or mock | Mock. Strings: "AniList", "MyAnimeList", "Kitsu", `title_n(1)`, "Watching", "12 / 24", "8.0", "Author name", "Started: 2020-01-01", "Score: 8.4". |
| Design | Spec layout, `gtk::Frame` logos, `Grid` with `card`, no header row of the new kind, no empty state |
| Actions | Every button opens a spec dialog whose OK only closes it; "Track" in the search does nothing |
| Reach | The entry page tracking button when a tracker is logged in (`route.rs:1155`, `anime/route.rs:1117`) and automatically after adding to the library (`route.rs:689`, `anime/route.rs:1064`) |
| Fix | Build the sheet and search over `entry_core/tracking/{session,summary,pickers,writes,search,remove}.rs` and `kuro_track::picker`. Interim (S): `tracking_pressed` opens `settings_tracking` and `auto_open_tracking` returns false. |

### global_search from Library (missing)

| | |
|---|---|
| Module | `src/entry.rs:657 global_search` (also `migrate_search` when no migrate request is parked) |
| Real or mock | Mock: three fixed sources, `ProgressBar` at 0.6, `title_n(1..8)`, "No results found" |
| Design | Old: chips, own entry row |
| Actions | Source arrows open `browse_source` mock, cards open `Title n` entries |
| Fix | In `library/mount.rs:390` capture `kind` and call `browse::pending::open(global_search::deep_link_fallback_for(kind, query))` (same as `shell/deep_link_page.rs:229`); then delete the `entry.rs` fallback arms (`browse`, `browse_source`, `seasons`, `global_search`, `migrate_entries`, `webview`) |

### webview (missing)

| | |
|---|---|
| Module | `src/entry.rs:797` |
| Real or mock | Mock: fixed URL, 30 % progress, "Web content". Nothing calls `handoff::take_webview` (only tests). `cloudflare_window.rs` says WebKitGTK is not available to this build. |
| Reach | Entry WebView button, source menu "Open in WebView", extension list (`browse/mount.rs:556`) |
| Fix | S: open the request URL in the system browser. L: embed a web view when the build has one. |

### global_search and migrate_search, real page (partial)

| | |
|---|---|
| Module | `browse/search_screen.rs`, `browse/pages.rs:538-720` |
| Real or mock | Real: per source search, chips, progress, cards with covers, long-press preview, migrate dialog |
| Design | Header with back and the title "Global search", then a second row with the entry; `docs/design/other-tabs.md` says a search page puts the entry in the header; sections are plain boxes; the arrow button is icon-only; the `chip` class has no CSS (`rg` finds it only in `pages.rs:577`) |
| Actions | Work |
| Fix | Return the entry from `GlobalSearchPage` as `Built.end[0]` so `ui::page` takes it; one boxed list per source |

### settings_downloads (partial)

| | |
|---|---|
| Module | `settings_ui/downloads.rs`, spec merged by `route.rs:150` |
| Real or mock | Real except the "External downloader" group |
| Design | Family |
| Actions | The two rows write `settings_downloads/Always use external downloader for anime` and `.../Downloader app preference`; nothing reads those keys |
| Fix | Type them over `external_downloader_selection` and `use_external_downloader` |

### settings_advanced (partial)

| | |
|---|---|
| Module | `settings_ui/advanced.rs:159` |
| Real or mock | Real rows: share crash logs, verbose, onboarding guide, notifications, reindex, cookies, WebView data, DoH, user agent, covers, reader reset, revoke trust, data saver |
| Dead spec rows | Background activity group (Disable battery optimization, Don't kill my app!), Reader: Use legacy decoder, Custom display profile, Extensions: Installer. `decode.rs:49` and `settings_ui/mod.rs` say these are dropped; the page still shows them. `resolve()` finds no dialog for the actions, so a click does nothing. |
| Design | Family |
| Fix | `ANDROID_ONLY` list in `Registry::build` |

### settings_tracking (partial)

| | |
|---|---|
| Module | `settings_ui/tracking.rs` |
| Real or mock | Real: seven tracker rows with login, logout, browser flows and a keyring warning; enhanced trackers |
| Problem | Spec title `"Show next episode's airing time "` (trailing space) does not match the typed row, so two rows show; the spec one writes a fake key |
| Fix | Trim in `merge.rs` twin test (`merge_items`, `merge`) |

### settings_appearance, settings_security, settings_reader (partial)

| | |
|---|---|
| Appearance | "Tablet UI" list is a dead spec row (`settings_ui/mod.rs` says the four values are not offered). App theme is missing (row 6). Theme mode is a list row. |
| Security | "Secure screen" list and "Secure screen hides app contents ..." are dead (`security.rs` header: "left to the spec page"); Linux has no such flag. |
| Reader | "Show content in cutout area" is dead (`reader/strings.rs:103` says it is not used). |
| Fix | `ANDROID_ONLY` list for all three |

### Player pages (partial, 8 rows)

| | |
|---|---|
| Module | `settings_ui/player/{controls,seeking,decoder,subtitle,audio,torrent,advanced,scripts}.rs` |
| Real or mock | Real pages, typed rows, family look, each built in the child-process test |
| Actions | Store the values. `player_view/mod.rs:75-83` reads only the remembered decoder and volume; gestures, subtitle, audio, torrent, mpv.conf, input.conf, custom buttons and scripts are stored and not used. Decoder is partial for the same reason (only `remember_decoder` is read). |
| Fix | Wire the live player to `PlayerPrefs::new(store)` and the gesture, subtitle and bridge modules |

### settings_restore_backup (partial)

| | |
|---|---|
| Module | `pref_model/backup_pages.rs:261-430` |
| Real or mock | Real flow: file check, card, options, job |
| Problem | `check_file(.., &NothingInstalled, &NothingInstalled)` (`:301`, `:428`): the card lists every source as missing and every tracker as logged out. Create backup names no stub source for the same reason (`backup_run.rs:42-67`). |
| Fix | Implement `SourceRegistry` and `TrackerRegistry` over `ext_host` and `tracker_host` |

### onboarding (partial)

| | |
|---|---|
| Module | `shell/onboarding.rs:140` (the spec `info_screen` in `screens.rs:457` is unreachable) |
| Real or mock | Real, four steps, finish flag, guard, restore hand-off |
| Design | No 720 px column, `title-1` heading in the body, two `DropDown`s and bare labels, `suggested-action` buttons bottom right; rest of the app uses headings and boxed lists |
| Actions | Work. Guide and storage help open aniyomi.org (`:20-22`). |
| Fix | `ui::column`, `heading`, boxed rows; project URLs |

### settings_about (partial)

| | |
|---|---|
| Module | `settings_ui/about.rs` |
| Real or mock | Real rows; version from `APP_INFO` |
| Problem | `WEBSITE_URL`, `DISCORD_URL`, `GITHUB_URL`, `PRIVACY_URL`, `HELP_TRANSLATE_URL` (`:258-262`) are Aniyomi's. The same host is in `route.rs:84`, `onboarding.rs:20,22`, `pref_model/widgets/mod.rs:42` (More, Help), `scripts.rs:441`, `library_update/error_log.rs:26`, `library_update/text.rs:20`, `browse/texts.rs:21`. "Check for updates" and "What's new" are disabled or hidden because `UPDATE_REPOSITORY` is empty and `updater` is off (`app_update.rs:17`); no script sets it. |
| Fix | One `links.rs` with the project's URLs; set the repository in the release build |

### settings_worker_info (partial)

| | |
|---|---|
| Module | `settings_ui/worker_info.rs`, `jobs.rs` |
| Real or mock | Real page and registry (`#![allow(dead_code)]` "job owners are not in this lane yet") |
| Problem | `rg enqueue_unique` finds no caller outside `jobs.rs`, so the three sections always read "-" |
| Fix | Record library update, backup and download runs, or hide the row until they do |

### settings_player_code_editor, pref dialogs, clear database, backup schema (partial)

| Page | Problem | Fix |
|---|---|---|
| code editor | `screens::build` uses the spec title "file.lua" for every file and the body repeats the name as a heading; colours are fixed hex values for a dark background | header from `OPEN_FILE`; use theme colours |
| pref dialogs | `dialog.rs:75` sets the window title and `:97` adds a `title-4` heading with the same text | keep one |
| clear manga and anime database | empty result is a margin label (`settings.rs:310`), not `ui::status`; confirm is a generic spec dialog | `ui::status` |
| backup schema | bare monospace label with 16 px margins | boxed row, as Worker info |

### Left over

`settings_ui/menu.rs` (old main menu) has no caller. `settings_ui/search.rs` `SearchableSettings` is unused (`#![allow(dead_code)]`). `settings.rs` arms for `cards`, `editor`, `code_editor`, `text_dump`, `licenses`, `settings_open_source_library_license` are unreachable. `ui::show_sheet`, `ui::show_dialog` and `ui::generic` still render spec mockups with `gtk::DropDown` and popover multi-selects; after the dead rows and the tracking mock go, only the clear-database confirm uses them.

## Not checked

- No screen was looked at; every design verdict is from code.
- The done rows that I did not read line by line (settings_library, settings_browse, settings_data_storage, the entry and browse dialogs) rest on: a typed twin for each spec title (a script compared spec titles with string literals in `src/settings_ui` and `src/pref_model`), the child-process test that builds every settings page, and the absence of spec fallbacks in their modules.
- Reader and player sheets were not in scope.
