# UI page audit 1: the five sidebar tabs, their sub-pages and the entry details pages

Read-only audit by reading code (rg, sed, Read). Cargo was not run and no window was opened, so every "test exists" below means a test with that name is in the tree, not that it passes. Judged against `docs/design/main-window.md` and `docs/design/other-tabs.md`. A page counts as working only when the code path is reachable from the app window.

## Summary

The redesign reached the chrome (sidebar, one header row, boxed lists, badge pills, empty states) of every tab. It did not reach the places where one page hands over to another. Most of the old mockup pages are still reachable from the new ones through a handful of wrong call sites. Three problems cause most of what you see:

1. **Pages open by title, not by id.** `screens::open_entry(title, anime)` (`src/screens.rs:83-92`) only finds LOCAL entries; any other title pushes `entry::entry(title, anime)` (`src/entry.rs:85`), the old page with "Summary of the series goes here", "Source A (EN)", "Studio", fixed genres and 24 fake chapters read from a throwaway database (`src/entry_model.rs:433-456`). It is called by the Library card for every non-local entry (`src/library/mount.rs:328-329`, also "Open random entry"), the Updates cover tap (`src/updates/updates_screen.rs:400`), the History cover and the duplicate dialog (`src/updates/history_screen.rs:225,277`) and the desktop `open-manga` / `open-anime` actions (`src/open_link.rs:39`, which builds "Title N"). The real id-based pages (`entry_core::route::open`, `entry_core::anime::route::open`) are only reached from Browse, search, migrate, Upcoming, notifications and deep links. In a real library every remote entry opens the mockup.
2. **Global search placeholders.** The Library's "Search globally" button (`src/library/mount.rs:168-169`) calls `ui::open("global_search")` with nothing parked. `browse::search_screen::body` (`src/browse/search_screen.rs:372-385`) then returns `None` and `screens.rs:26` falls back to `entry::global_search` (`src/entry.rs:657-720`): "Source A/B/C", cards "Title 1..8", a progress bar fixed at 0.6, "No results found". The real page only appears when Browse, an entry page or a deep link parks a request first. This is what you saw.
3. **No download manager, no tracking sheet, no WebView.** `download_host::install` and `anime::download_backend::install_queue` have no caller, so every Download action (Library selection, Updates rows and bar, manga entry, Storage delete) does nothing. The tracking sheet, tracker search and WebView pages are still the mockups (`src/entry.rs:723-810`, reached through `ui::open("tracking_sheet" | "tracker_search" | "webview")` from the real entry pages and from Browse). The Extensions pages install, update, uninstall and refresh through a stub (`src/browse/mount.rs:197-208`).

Counts (pages and features in scope): done 10, partial 10, missing 6, lib_only 2, not_applicable 0.

| # | Page | Verdict | Size |
|---|---|---|---|
| 1 | Global search opened from the Library | missing | S |
| 2 | Sample entry page (reached from Library, Updates, History, desktop actions) | missing | S |
| 3 | Tracking sheet | missing | L |
| 4 | Tracker search | missing | M |
| 5 | WebView page | missing | S (browser fallback) / L (WebKit) |
| 6 | Download queue | missing | L |
| 7 | Download manager behind every Download action | lib_only | L |
| 8 | Tracking logic behind the sheet | lib_only | M |
| 9 | Browse, Extensions (anime and manga) | partial | L |
| 10 | Extension details | partial | S |
| 11 | Manga details | partial | M |
| 12 | Anime details | partial | M |
| 13 | Library (anime and manga views) | partial | M |
| 14 | Updates | partial | M |
| 15 | History | partial | S |
| 16 | Storage | partial | M |
| 17 | Global search (real page, from Browse, entry, deep link) | partial | M |
| 18 | Migrate source search | partial | S |
| 19 | Browse, Sources (anime and manga) | done | - |
| 20 | Browse, Migrate tab and its entries | done | - |
| 21 | Browse, source listing | done | - |
| 22 | Browse, language / source filter pages and Source settings | done (read lightly) | - |
| 23 | Library settings sheet (Filter, Sort, Display) | done | - |
| 24 | Upcoming | done | - |
| 25 | Statistics | done | - |
| 26 | Categories | done | - |
| 27 | More | done | - |
| 28 | Sidebar | done | - |

Cross-cutting findings are after the tables. Previous shard audits (`docs/audit/p0-08.md`, `p0-09.md`, `p0-10.md`) found items 1, 2, 13, 14, 15 independently; I re-read the code and the line numbers below are current.

---

## 1. Global search opened from the Library (MISSING, S)

| Check | Finding |
|---|---|
| Module | `entry::global_search` (`src/entry.rs:657`), a spec mockup. Reached by `library::mount::open_global_search` (`src/library/mount.rs:168`), the Library's "Search globally" button on an empty search (`src/library/view/mod.rs:610`), and by any other bare `ui::open("global_search")` (the old mock Browse header `src/entry.rs:491`). |
| Real or fake | Fake: "Source A/B/C", `title_n(1..8)` cards, `ProgressBar::set_fraction(0.6)`, "English", "No results found". The query typed by the Library is put in a search entry that searches nothing. |
| Design | Old: its own search row under a title, `chip_row` toggles, flat cover buttons, no boxed list. |
| Actions | Source arrows open the mock Browse; cards open the mock entry; Enter does nothing. |
| Fix | In `open_global_search` call `browse::pending::open(BrowseRoute::GlobalSearch { kind, source_id: None, query })` (the Browse header already does this, `src/browse/shell.rs:350`); then delete `entry::global_search` and the `or_else(entry::body)` at `src/screens.rs:26`. S. |

## 2. Sample entry page (MISSING, S)

| Check | Finding |
|---|---|
| Module | `entry::entry` (`src/entry.rs:85-250`) over `EntryModel::open`, which seeds a temp database (`src/entry_model.rs:91-134, 433-480`). |
| Real or fake | Fake: description "Summary of the series goes here...", "Source A (EN)", "Studio" or "Author", "Ongoing", genres Action/Adventure/Comedy/Fantasy, "Next episode airs in 2d 4h", 24 chapters "Chapter n" with the first 19 read, "2 days" fetch interval, Resume opens "Chapter 20". The library button just flips its label. |
| Reached by | Library card and random entry for non-local entries (`library/mount.rs:328`), Updates cover (`updates_screen.rs:400`), History cover and duplicate dialog (`history_screen.rs:225,277`), desktop `open-manga` / `open-anime` (`open_link.rs:39`, "Title N"), `route::build()` with no target (`entry_core/route.rs:78-83`, "Title 1"), and the smoke run (`main.rs:411`). |
| Design | Old: 18 px margins, grey `image-x-generic-symbolic` frame, `title-1` label, download menu whose items do nothing (`entry.rs:154`). |
| Fix | Call `entry_core::route::open(id)` / `entry_core::anime::route::open(id)` with the id at the five call sites above (the Library already has the id, `mount.rs:322`); then delete `entry::entry`, the seeded `EntryModel::open` and the `build()` fallback. S. |

## 3. Tracking sheet (MISSING, L)

| Check | Finding |
|---|---|
| Module | `entry::tracking` (`src/entry.rs:723-762`). Opened by `ui::open("tracking_sheet")` from the real manga page (`entry_core/route.rs:689,1155`) and anime page (`entry_core/anime/route.rs:1064,1119`). |
| Real or fake | Fake: three trackers named in code (AniList bound, MyAnimeList, Kitsu), title "Title 1", "Watching", "12 / 24", "8.0", dates; every button opens a spec dialog with no effect on data. The login check that decides sheet or settings is real (`tracking::session::has_logged_in`), so a logged-in user gets this fake. |
| Design | Old: bare `Frame` logos, flat buttons in a `card` grid. |
| Actions | Nothing is stored. Add tracking and the title button open the fake tracker search (item 4). |
| Fix | Build the sheet over `entry_core::tracking::{session, summary, writes, dates, remove}` (item 8) and the tracker services. Task T-27a-048, T-29a-059. L. |

## 4. Tracker search (MISSING, L)

| Check | Finding |
|---|---|
| Module | `entry::tracker_search` (`src/entry.rs:764-795`). |
| Real or fake | Fake: search text "Title 1", four cards `title_n(i)` with "Author name", "Type: Manga", "Started: 2020-01-01", "Status: Publishing", "Score: 8.4". The Track button does nothing. |
| Fix | Wire the search state machine in `entry_core::tracking::search` to a real page (plan task T-28a-057 is P0 and says "replace mock"). M. |

## 5. WebView page (MISSING, S or L)

| Check | Finding |
|---|---|
| Module | `entry::webview` (`src/entry.rs:797-810`). Reached by `handoff::open_webview` (`entry_core/handoff.rs:184-187`) from the manga and anime WebView buttons, Browse source menu, error state and extension row globe (`browse/mount.rs:556`, `browse_screen.rs:224`). |
| Real or fake | Fake: the URL line is the literal "https://example.org/", a progress bar fixed at 0.3, "Web content" status, Back, Forward and a menu with empty handlers. The parked `WebViewRequest` (url, title) is never read. `src/cloudflare_window.rs:5-7` says WebKitGTK is not available to this build. |
| Fix | S: replace the page with `gtk::UriLauncher` opening the parked URL in the default browser (plan task T-req-platform-006 names this fallback). L: embed WebKitGTK (T-36a-049, P0). |

## 6. Download queue (MISSING, L)

| Check | Finding |
|---|---|
| Module | `screens::downloads` (`src/screens.rs:432-438`), reached from More, the Updates reselect (`updates/mount.rs:348`) and deep links. |
| Real or fake | Honest but empty: always "No downloads" plus a Library button. No sort, Cancel all, source groups or progress. The More row subtitle is `queue_subtitle(QueueState::Stopped)`, which is `None` (`screens.rs:386`), so the page says nothing about a stopped manager. |
| Design | New: one header row, `browse::look::empty_state`. |
| Actions | The one button works. Nothing else exists. |
| Fix | Needs the manager (item 7), then source groups and rows (T-47ba-015 and T-47ba-016, both P0, T-24a-040). L. |

## 7. Download manager (LIB_ONLY, L)

`kuro-downloads` (`AnimeQueue`, `DownloadIndex`) and the seams `updates::download_host::install` (`src/updates/download_host.rs:93`), `anime::download_backend::install_queue` (`src/entry_core/anime/download_backend.rs:20`) and `library::mount::{on_download, on_delete_downloads}` (`library/mount.rs:85-91`) have no production caller (rg). The default `IndexBackend` (`download_host.rs:107-142`) answers the disk check for manga and refuses everything else. What does nothing today:

- Library selection Download menu and "delete downloads" in the remove dialog (`mount.rs:366-388` only logs at debug level).
- Updates row indicator menu, bulk Download, Delete (Delete toasts "source missing", `updates_screen.rs:549-570`).
- Manga entry Download menu, row indicators and bulk Download: `Backend::enqueue_downloads` defaults to `false` (`entry_core/backend.rs:41`) and `SourceBackend` does not override it; the runner then emits nothing (`entry_core/runner.rs:368-373`). The menu is shown, so it looks broken.
- Anime entry: no Download menu at all (`can_enqueue` is false), so it is honest.
- Storage delete (item 16).

Fix: start the manager in `misc::start()` or `main.rs` and register it through the four seams. L.

## 8. Tracking logic (LIB_ONLY, M)

`src/entry_core/tracking/*` (session, summary, links, dates, enhanced, auto_track, search, writes, remove) contains no widget code (rg for `gtk::` finds nothing). Only `session::has_logged_in` and `refresh_from_host` are called from the pages. `bind_enhanced` and `remove_cover` are default no-ops on `SourceBackend` (`entry_core/backend.rs:47,59`). Feeds items 3 and 4.

## 9. Browse, Extensions pages, anime and manga (PARTIAL, L)

| Check | Finding |
|---|---|
| Module | `browse::pages::extensions` over `HostBackend` (`src/browse/mount.rs:197-208, 564`); rows from `ext_host` (real registry and anime buckets). |
| Real or fake | Rows are real. The backend is a stub: `install` reports `InstallStep::Error`, `uninstall` returns at once, `refresh` returns `Ok` at once. Install, Update, Update all, header refresh and details Uninstall therefore end in an error step or do nothing. |
| Design | New: boxed lists, "2 updates" pill, kind and section switches, Ctrl+1..6. |
| Actions | Trust writes the bare package name into `trusted_extensions` (`mount.rs:544-549`), but the trust store expects `<pkg>:<versionCode>:<digest>`, so Trust changes nothing. `has_installed` (`mount.rs:254`) reads the manga registry for the anime tab too. Header menu uses `ui::menu_button` (flat-button popover), not the Library's `GtkPopoverMenu`. |
| Fix | Implement `ExtensionBackend` over `kuro-extensions` (install, uninstall, refresh) and write the real trust key. T-40a-018, T-41a-012, T-18a-013 (all P0). L. |

## 10. Extension details (PARTIAL, S)

Real page (`browse/ext_details_page.rs`, `mount.rs:386-411`): boxed lists, repo link, clear cookies, per-source settings handoff. Uninstall calls `HostBackend.uninstall`, which does nothing (`mount.rs:401`). Fix with item 9. S.

## 11. Manga details (PARTIAL, M)

| Check | Finding |
|---|---|
| Module | `entry_core::route::{build, page_for, assemble}` (`src/entry_core/route.rs`, 1802 lines), opened by id. Real: live query, source fetch (`SourceBackend::live`), flags, filters, selection, swipe, categories dialog, scanlator sheet, duplicate dialog, interval dialog, full cover, banners. |
| Real or fake | Real data. Placeholders left: cover is the grey `image-x-generic-symbolic` frame (`entry_core/cover_view.rs:88`, not the initials tile of the Library) until the image service answers. The `build()` fallback with no target is "Title 1" (`route.rs:78-83`). |
| Design | One header row with back and title, column clamped at 720 px, but the chapter list is a full-bleed `gtk::ListView` with 10 px row padding (`list_view.rs:240-282`), not a boxed list. Header buttons and popovers are built by hand (`plain_row`, `route.rs:1537`), not like the Library menu. Colours use `@kuro_primary` (`list_view.rs:135`), the app palette, not `@theme_*`. |
| Actions | Download menu, row indicators and bulk Download do nothing (item 7). Share and WebView never show because `Backend::is_http_source` is `false` for `SourceBackend` (`backend.rs:54`, used at `runner.rs:177`). Overflow "Settings" never shows because `bar_state(.., false)` hard-codes `configurable_source` (`route.rs:819,1115`). Tracking button opens the mock sheet (item 3). Chapter tap, Resume, mark read, bookmark, category, migrate, refresh, filter popover work. |
| Fix | Override `is_http_source` and add a `configurable` lookup in `SourceBackend`; hide the Download menu until the manager is registered; show initials on the cover; make the chapter list a boxed list. M. |

## 12. Anime details (PARTIAL, M)

| Check | Finding |
|---|---|
| Module | `entry_core::anime::route::{build, open, page_for}` (`src/entry_core/anime/route.rs`, 2020 lines) over `RemoteAnimeBackend` (`anime/remote.rs`). `NO_ENTRY` shows a not-found page, not a mock. |
| Real or fake | Real: episodes, seasons, airing line, sheets (episode, season), options dialog, intro dialog, related. `is_http_source` and `is_configurable_source` are wired (`remote.rs:265-271`), so WebView, Share and Settings show. |
| Design | Same as item 11: unboxed `ListView` rows, `@kuro_primary`/`@kuro_tertiary` classes (`anime/row_view.rs:42`), grey cover placeholder. |
| Actions | No Download menu (honest). Playing a remote episode uses `player_view::open_source_episode` (real). Tracking and WebView open the mock pages (items 3 and 5). `Resume` from History and Library for a non-local anime goes through `screens::open_media`, which pushes `player::player(title, unit)`, a placeholder surface with a fixed 1420 s duration (`src/player.rs:53-70`). |
| Fix | As item 11, plus open the player by episode id from `open_media`. M. |

## 13. Library, anime and manga views (PARTIAL, M)

| Check | Finding |
|---|---|
| Module | `library::mount::build` over `library::view::LibraryView` (real), in `screens::libraries` / `library_page` (`src/screens.rs:193-227`). The `screens::library()` prototype below it (`screens.rs:229-352`, "Title N" via `crate::live`) runs only if the worker thread fails to start. |
| Real or fake | Real data from the library databases, live updates, covers (file thumbnails at once, remote through the image service, else initials), badges in words, empty states, categories tabs, three-tab settings sheet, selection bar, update runner behind the refresh button (`misc/runner.rs:307`). |
| Design | New: the reference page. |
| Actions that do nothing or the wrong thing | (a) Opening a non-local card or "Open random entry" shows the sample entry (item 2). (b) Selection Download and "delete downloads" (item 7). (c) Continue on a non-local anime opens the placeholder player; `Resume.external` is ignored and `deps().always_external` is `|| false` (`mount.rs:446`); `item_downloaded` is `|_| false` (`mount.rs:447`), so with Downloaded only on the next item is always filtered out and the toast "Next chapter not found" shows. (d) "Edit categories" opens the More Categories page on its Anime tab whatever kind it came from. (e) "Search globally" shows the mock (item 1). |
| Fix | Id-based open in `nav().open`/`resume` (S), register the manager (item 7), read `always_use_external_player` and the download index in `deps()` (S). M overall. |

## 14. Updates (PARTIAL, M)

| Check | Finding |
|---|---|
| Module | `updates::mount::updates` over `UpdatesScreen` (`src/updates/updates_screen.rs`), real feed from the updates view with date headers, boxed lists, initials covers, selection row plus action bar, F5, Upcoming button with a word, reset of the new-updates counters. `screens::updates()` and `live::updates_content` are the old fallback and are unreachable once the data opens. |
| Design | New. Unread dot and bookmark star use the `accent` class (`updates_screen.rs:378,417`); stock GTK 4.24 has no `.accent` rule, the Tokyonight-Dark theme in `~/.themes` does. |
| Actions | Cover tap opens the sample entry for non-local entries (item 2); row tap, bar play and Continue open by title and unit name, so a remote anime goes to the placeholder player (`open_route.rs:40-48`); `open_route::install_external` has no caller, so "always use external player" is read and ignored. Download indicator and bar actions do nothing (item 7). Rows are `activatable(false)` with their own `GestureClick` (`updates_screen.rs:424-441`), so Enter and Space on a focused row do nothing; the click is not routed through `row_activate`. Mark read, bookmark, selection and refresh work. |
| Fix | Id-based open (S), row key activation (S), manager (item 7). M. |

## 15. History (PARTIAL, S)

| Check | Finding |
|---|---|
| Module | `updates::mount::history` over `HistoryScreen` (`src/updates/history_screen.rs`) and `history_row`. Real: live query, search in the header, per-kind Clear history, remove dialog with reset-all checkbox, favourite button with duplicate and category dialogs, resume-last on sidebar reselect. |
| Design | New: boxed lists under date headers, `row_cover` with initials, one header row. Tooltips "Add to library" and "Remove" are English literals (`history_row.rs:59,69`). |
| Actions | Row click goes through `row_activate` (keyboard and mouse). Cover click and "Show existing" in the duplicate dialog open the sample entry for non-local entries (`history_screen.rs:225,277`). Resume opens by title and unit name (`:377`); remote anime goes to the placeholder player; the external-player preference is only logged (`:371-376`). |
| Fix | `entry_core::*::route::open(line.entry_id)` and open units by id. S. |

## 16. Storage (PARTIAL, M)

| Check | Finding |
|---|---|
| Module | `updates::mount::storage` over `StorageScreen` (`src/updates/storage_screen.rs`, `storage_row.rs`). Real: category dropdown, half donut with total, entry rows, "Scanning downloads…" state, one page per kind in the header switch. |
| Real or fake | Manga sizes come from the disk index; anime `entry_usage` returns `(0, 0)` and `on_disk` is false for anime in the default backend (`download_host.rs:107-142`), so the anime page lists entries with 0 B. |
| Design | Own two-column layout, not `ui::column`; donut and dots use a fixed per-entry colour from `storage::entry_color` (`storage.rs:44`), not theme colours (acceptable for a data series, listed for completeness). |
| Actions | Delete always reports the missing-source outcome because `source_installed` is false (`mount.rs:371-381`, item 7). |
| Fix | Manager (item 7) and the anime index sizes. M. |

## 17. Global search, the real page (PARTIAL, M)

| Check | Finding |
|---|---|
| Module | `browse::search_screen::body` and `pages::global_search` (`src/browse/pages.rs:538-720`) over `SearchRunner`, real engine, per-source rows, "Has results" persisted, Escape steps, migrate variant. Reached from the Browse header (Enter in the entry, menu, reselect), entry-page genre and title searches, search intents and deep links. |
| Design | Old: it builds its own search entry inside the page body under a back-and-title header ("Global search"), which the design says a search page must not do (`ui::page` puts the entry in the header). The chips carry a `chip` class that no theme defines (`pages.rs:577`; checked stock GTK 4.24 and Tokyonight-Dark), so they are plain toggle buttons. Sections are flat boxes with an arrow button, not boxed lists. |
| Actions | Arrow opens the source listing, cards open the entry by id, long press opens the entry, single result with an extension filter replaces the page: all real. |
| Fix | Move the entry into the header with `ui::page(.., end = [entry])`, style the chips with `pill`/`flat`, put each source in a boxed list. M. |

## 18. Migrate source search (PARTIAL, S)

`browse_screen::page("migrate_source_search")` (`src/browse/browse_screen.rs:236-247`) matches `BrowseRoute::BrowseSource { source_id, query, .. }` and ignores `migrating`, so it builds the plain browse listing: a card opens the entry instead of the migrate dialog (`production_env` has no migrate-aware `open_entry`). Fix: pass `migrating` into `source_page` and route cards to `manga_migrate_dialog`. S.

## 19. Browse, Sources (DONE)

Anime and manga Sources pages (`browse/sources_page.rs`, `browse/anime/sources_view.rs`) are real, boxed, reread on map and on pref change; pin, disable, latest, options popover, empty state "No source found" + Extensions button. Header: kind switch, entry ("Global search…"), refresh, filter, menu (`browse/shell.rs`). Residual: anime sources from installed extension packages have no runnable instance, only built-in and recipe sources run (`browse/anime/directory.rs`, see `p0-10.md` T-41a-001); the Browse menu is a `ui::menu_button` popover.

## 20. Browse, Migrate tab and entries (DONE)

`browse::migrate_list` counts favourites per source from the library DBs and opens `MigrateEntries` by source id (`mount.rs:269-345`); `migrate_screen::page` (`browse/migrate_screen.rs:189-195`) opens the migrate dialog by entry id. Boxed rows with "12 in library" pills.

## 21. Browse, source listing (DONE)

`browse_screen::page` / `anime::browse_screen::page` build real listings (`listing_view.rs`, `view.rs grid_cell_lines`): Popular/Latest/search/filters, display modes, "In library" pill, remote covers through the image service, long-press favourite flow with duplicate and category dialogs, open by id. See item 18 for the migrate variant and `p0-10.md` for the missing prepend/append spinner rows.

## 22. Browse, language and source filter pages, Source settings (DONE, read lightly)

`lang_pages.rs` (`ExtensionLanguagesPage`, `SourcesFilterPage`) and `source_prefs_screen::page` are real and reached with real data (`mount.rs:347-381`, `handoff::open_source_preferences`). I read their wiring only, not every row.

## 23. Library settings sheet (DONE)

`library/view/sheet.rs` with `filter_tab.rs`, `sort_tab.rs`, `display_tab.rs`: each control writes its preference at once; opened from the header menu, the sidebar reselect and the empty state. Modal built with `ui::modal`.

## 24. Upcoming (DONE)

`updates/upcoming/page.rs`: calendar plus list, boxed list per day with a worded count badge, rows open by id and through `row_activate` (`page.rs:280-306`), Home and arrow keys, help button. Calendar CSS uses `@theme_selected_bg_color` only (`calendar_view.rs:27-33`). It has no empty state by design (`page.rs:5`).

## 25. Statistics (DONE)

`updates::mount::stats` with `stats_data::load` over both databases, the download index and the tracker logins; tiles show "N/A" on a failed section; spinner while loading; Anime | Manga switch in the one header row. Tiles use the `card` class (not in stock GTK 4.24, defined by Tokyonight-Dark).

## 26. Categories (DONE)

`live::categories_content` / `add_category` (`src/live.rs:323-348`): one header row `[back] Categories [Anime | Manga] [+]`, boxed list in the page column, empty state "You have no categories" + Add category, rename, hide, delete and drag reorder through `CategoryService`. Minor: the row built by `screens::list_row` is `activatable(true)` with no handler, the Hide tooltip never changes to "Show", and reorder is drag only.

## 27. More (DONE)

`screens::more` (`src/screens.rs:373-415`): no title or logo, three boxed lists with icons; the two switches are bound to `incognito_mode` and `downloaded_only` by matching the spec title and subtitle strings (`ui.rs:481-487`; fragile if the spec text changes); link rows go through `row_activate` and end in the chevron; Help opens the help site (`open_help`, test `more_help_opens_the_help_site_once_per_click_and_once_per_key`); the mock "Paused • 3 remaining" subtitle is replaced. The Download queue, Storage and the Settings pages it opens are items 6, 16 and out of scope.

## 28. Sidebar (DONE)

`main.rs:436-520`: fixed rail with the app name, `navigation-sidebar` list, icons, badge pills for Updates and Browse bound to the real counters (`misc::badge`), bottom switcher below 720 px, hide while selecting or in the reader.

---

## Cross-cutting findings

1. **Mockup code still shipped and partly reachable** (`src/entry.rs`, 842 lines, plus `src/screens.rs:229-352` and `src/live.rs:401-483`). Reachable today: `entry::entry`, `global_search`, `tracking`, `tracker_search`, `webview`. Unreachable once the data opens (dead weight, delete with the fixes): `entry::browse`, `browse_source`, `migrate_entries`, `migrate_sources`, `seasons`, the extension rows with "Installing" labels, `screens::library()`, `updates()`, `history()`, `live::history_content`, `live::updates_content`. Also `screens::build("reader")` and `("player")` build with the literals "Title 1" and "Episode 1" (`screens.rs:44-46`); nothing in Rust opens those ids, but `ui::open` would.
2. **Title-based navigation helpers** `screens::open_entry`, `screens::open_media`, `local_flow*::*_for_title` are the root of items 2, 13, 14 and 15. One id-based `open_entry(kind, id)` fixes all of them.
3. **Seams with no caller** (all behave as "feature off" with no message): `download_host::install`, `anime::download_backend::install_queue`, `library::mount::{on_download, on_delete_downloads}`, `open_route::install_external`. A Download button that does nothing and gives no toast looks broken; show a toast until the manager exists.
4. **Style classes.** The pages rely on `card`, `pill`, `accent`, `boxed-list`, `heading`, `dim-label`, `navigation-sidebar`. I extracted stock GTK 4.24's `Default-light.css`: it has `boxed-list`, `heading`, `dim-label`, `caption`, `navigation-sidebar`, not `card`, `pill` or `accent`. `~/.themes/Tokyonight-Dark/gtk-4.0` defines all three. `chip` (global search) exists in neither. Nobody has looked at these pages in the Tokyo Night theme by eye (both design docs say so).
5. **Two colour systems.** Redesigned pages use `@theme_*` names; the entry pages use `@kuro_primary`, `@kuro_tertiary` and `@kuro_primary_container` (`entry_core/list_view.rs:135`, `anime/row_view.rs:42-43`), defined by `theme::startup` from the app's own palette (`src/theme/css.rs`, which also redefines `theme_selected_bg_color`). If the app palette is not the GTK theme, the entry list dots will not match the rest. Needs an eye check.
6. **What I did not verify:** the Filter tab rows and Display tab of the Library sheet line by line, `browse/lang_pages.rs` and `source_prefs_screen` rows, every entry-page dialog (download_menu, set_as_default, tag_menu, intro length), and anything that needs a running window (focus, sizes, theme).

## Suggested order of fixes

1. Items 1 and 2 (S, S): one id-based entry open and one parked route. Removes the placeholders you saw and most of "the redesign did not reach it".
2. Item 5 browser fallback and a toast for Download actions (S): nothing looks dead.
3. Items 11 and 12 small wiring (`is_http_source`, configurable, cover initials) and items 15, 18 (S each).
4. Item 9 (extensions backend) and item 7 (download manager), then items 3, 4, 6, 16.
