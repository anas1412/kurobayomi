# UI page audit 3: global search, migrate search, reader, player and the pages that still fall back to mockups

Date: 2026-10-11. Method: reading code only (rg, sed, Read). Cargo was not run and no test was executed; "test" below means a test of that name exists in the source. A page counts as working only when the app window can reach it. Design rules come from `docs/design/main-window.md` and `docs/design/other-tabs.md`.

## Summary

The user is right on both points. The redesign never touched the pushed pages, and global search does still show placeholders, but only when it is opened from one place: the Library's "Search globally" button. The live global search page itself is real; its look is the old one.

Counts (31 page entries): done 4, partial 8, missing 6, lib_only 2, not_applicable 11.

The five worst findings, worst first:

1. **Tapping a library card of a non-local entry opens a sample entry page, not the real one.** `src/library/mount.rs:316-329` sends every non-local card to `screens::open_entry(title, anime)`. That builds `entry::entry` (`src/entry.rs:85`), which makes a model in a temporary database and seeds it with fake chapters (`src/entry_model.rs:98-121`) and hard-codes "Author", "Ongoing", "Source A (EN)", "2 days" (`src/entry.rs:104-123`). The real page, `entry_core::route::open(id)` / `entry_core::anime::route::open(id)`, is what global search cards already call (`src/browse/search_screen.rs:226,239`). Deep links do the same with the literal title "Title <id>" (`src/open_link.rs:36-40`). One line each.
2. **Global search from the Library is the fake page.** `src/library/mount.rs:168-179` calls `ui::open("global_search")` with nothing parked. `search_screen::body` returns `None` (`src/browse/search_screen.rs:384`), `screens.rs:26` falls through to `entry::global_search` (`src/entry.rs:657-705`): "Source A", "Source B", "Source C", a progress bar fixed at 0.6, eight "Title n" cards, "No results found". The typed query is pasted into that fake page's box and nothing is searched. The same function serves the empty-library "Search globally" button and the "Search globally for ..." row at the end of a library search (`src/library/view/mod.rs:614,1028`).
3. **The Tracking sheet and Track search are pure mockups, and real buttons open them.** `src/entry.rs:723-795`: "AniList / MyAnimeList / Kitsu", "Title 1", "12 / 24", "8.0", "Author name", fixed dates and scores. Opened by the Tracking button of every real manga and anime page (`src/entry_core/route.rs:689,1155`, `src/entry_core/anime/route.rs:1064,1119`). `src/entry_core/tracking/*.rs` has no GTK code, only rules, so there is no real sheet to switch to.
4. **WebView is a mockup.** `src/entry.rs:797-813` shows "https://example.org/", a 30 percent progress bar and "Web content". The real URL is parked by `handoff::open_webview` but nothing ever calls `handoff::take_webview` (`src/entry_core/handoff.rs:89`). Opened by the entry page's WebView button and by Browse's "Open in WebView".
5. **A fake player opens for any anime the real player does not take.** `src/player.rs` has a fixed 1420 second timeline, "00:00 • Opening", "Skip Opening", no video. It is reached through `screens::open_media` (`src/screens.rs:94-99`) from `entry_core::anime::route::open_player` (`src/entry_core/anime/route.rs:1575`, the last fallback) and from Library Continue for any non-local anime (`src/library/mount.rs:337-340`).

Root cause of 2, 3, 4 and part of 5: `ui::open(id)` carries no arguments, so pages read a parked hand-off and each builder falls back to the spec mockup when nothing is parked (`screens::body`, `entry::body`). In the shipped app the "no app data" fallbacks are dead (a failed database open quits, `src/main.rs:195-200`), but the "nothing parked" fallbacks are live.

What is fine: the reader (real loaders, viewers, bookmark, mode, crop, page actions, error card with Retry), the player core (libmpv, progress saving, track menu), the live global search runner (source fan-out, chips, Has results stored, link resolve, single-result auto-open), the migrate dialogs, Categories, Statistics, Storage, source preferences.

Not covered here: Upcoming, extension details (the real `ext_details_page`), settings pages, Library/Updates/History/Browse tabs themselves (other shards).

## Table of pages, worst first

| # | Page | Verdict | Worst problem | Fix | Size |
|---|---|---|---|---|---|
| 1 | Entry page opened from a Library card or deep link | missing | sample entry with fake rows | call the id-based `route::open` | S |
| 2 | Global search opened from the Library | missing | "Source A/B/C" mockup | park a `GlobalSearch` route | S |
| 3 | Tracking sheet | missing | fixed sample trackers | build the sheet over `entry_core::tracking` | L |
| 4 | Track search | missing | fixed sample results | build over `tracking::search` | L |
| 5 | WebView | missing | shows example.org | take the hand-off, open in browser | S |
| 6 | Mock player (`player.rs`) | missing | fake timeline, no video | route all playback to `player_view`, delete `player.rs` | M |
| 7 | Player window (`player_view`) | partial | five controls, no player settings | add controls, feed prefs | L |
| 8 | Reader | partial | tracker sync and orientation not wired | see table | M |
| 9 | Global search page (live) | partial | old look, no empty state | header search, empty state | M |
| 10 | Migrate search | partial | same look as 9 | same fix as 9 | S |
| 11 | Migrate source listing | partial | opens a normal listing | carry the old entry id | M |
| 12 | Source listing (`browse_source`) | partial | second row search, plain chips | move search to header | M |
| 13 | Migrate entries | partial | plain-label empty state | `look::empty_state` | S |
| 14 | Download queue | partial | permanently empty | needs a download manager | L |
| 15 | Categories | done | | | |
| 16 | Statistics | done | | | |
| 17 | Storage | done | | | |
| 18 | Source preferences | done | | | |
| 19 | Dead stand-ins (11 ids) | not_applicable | unreachable code | delete | S |
| 20 | `shell/webview.rs`, `intents::route_request` | lib_only | built, never called | wire or delete | S |

## 1. Entry page from a Library card or a deep link (`manga_entry_details`, `anime_entry_details`)

| Check | Finding |
|---|---|
| Module | Real page: `src/entry_core/route.rs`, `src/entry_core/anime/route.rs` (`open(id)`). Sample page: `src/entry.rs:85-245` through `screens::open_entry` (`src/screens.rs:83-92`). |
| Real or mockup | Mockup for every non-local entry. `local_id_with_title` is local-only (`src/local_flow/open.rs:17-19`) and `anime_id_for_title` likewise (`src/local_flow_anime/open.rs`), so a source entry falls to `push(entry::entry(title, anime))`. That uses `EntryModel::open`, which creates a temp database and seeds it (`src/entry_model.rs:98-121`). Fixture strings: "Author"/"Studio", "Ongoing", "Source A (EN)", "2 days", `title_n`. |
| Design | Old: hard-coded icons, 18 px margins, own `icon_label_button` row. |
| Actions | The sample page's Add to library button only flips its label (`src/entry.rs:124-129`). WebView and Tracking open the mockups 3 and 5. |
| Also | Library Continue for a non-local entry goes by title and a stringified id: `open_media(anime, &title, &r.item.to_string())` (`src/library/mount.rs:337-340`). Manga works by accident (`find_ids` parses the id, `src/reader/launch.rs:59-63`) and picks the wrong entry when two favourites share a title; anime lands in the mock player (6). Deep link `manga=ID`: `src/open_link.rs:36-40` opens `title_n(id)`. "Open random entry" in the fallback library is `title_n(7)` (`src/screens.rs:329`, dead). |
| Fix | `nav.open` in `library/mount.rs` calls `entry_core::route::open(id)` or `anime::route::open(id)` for every entry; `open_link::go` opens by id; Continue calls `reader::launch::open_chapter` / `player_view::open_source_episode` by id. Then delete `entry::entry`, `EntryModel` seeding, `open_entry`, `title_n`. Size S. |

## 2. Global search opened from the Library (`global_search`)

| Check | Finding |
|---|---|
| Module | Caller `open_global_search` `src/library/mount.rs:168-179`, wired at `:390`. Page: `entry::global_search` `src/entry.rs:657-705` via `screens.rs:26`. |
| Real or mockup | Mockup. Fixture strings: "Source A/B/C", `title_n(1..8)`, "English", `progress.set_fraction(0.6)`, "No results found" (hard-coded), a `"▶ Source A"` migrate prefix. |
| Design | Search box in the body under a "Global search" header; plain chips; flat headings. Not the new look. |
| Actions | Arrow, cards: open `browse_source` or the sample entry. Chips do nothing. Nothing is searched; the Library only pastes its query into the box (`library/mount.rs:176`). |
| Fix | `nav.global_search` becomes `pending::open(BrowseRoute::GlobalSearch { kind, source_id: None, query })` with `kind` from the view that owns the nav (`kind` is in scope at `library/mount.rs:316`). The route path already exists and is what Browse, deep links and the shell use (`browse/shell.rs:350`, `shell/deep_link_page.rs:229`). Delete `entry::global_search` and its `cover_grid` helper. Size S. |

## 3. Tracking sheet (`tracking_sheet`)

| Check | Finding |
|---|---|
| Module | `src/entry.rs:723-762` (builder), openers in `entry_core/route.rs:689,1155` and `entry_core/anime/route.rs:1064,1119`. |
| Real or mockup | Mockup. Fixture strings: `["AniList", "MyAnimeList", "Kitsu"]`, `title_n(1)`, "Watching", "12 / 24", "8.0", "Start date", "Finish date". The first tracker is always "bound", the others never. |
| Design | Old: letter in a frame as logo, flat buttons in a `card` grid. |
| Actions | Status, chapters, score, dates open spec dialogs that change nothing. Remove opens a dialog that deletes nothing. The gate is real (`tracking_pressed`: sheet only when a tracker is logged in), so a user who logged in sees fake data. |
| Fix | Build the sheet from `entry_core::tracking::{summary, facts, pickers, writes, remove}` (all pure, tested, no GTK). Size L. |

## 4. Track search (`tracker_search`)

| Check | Finding |
|---|---|
| Module | `src/entry.rs:764-795`; opened only from the sheet above. |
| Real or mockup | Mockup. "Title 1".."Title 4", "Author name", "Type: Manga", "Started: 2020-01-01", "Status: Publishing", "Score: 8.4". |
| Design | Boxed list but rows are bare boxes; Track button has no handler. |
| Actions | Search box does nothing; Track and Track privately have no `connect_clicked`. |
| Fix | Build over `entry_core::tracking::search` (257 lines, pure). Size L, done together with 3. |

## 5. WebView (`webview`)

| Check | Finding |
|---|---|
| Module | `src/entry.rs:797-813`. Openers: `handoff::open_webview` (`entry_core/handoff.rs:184`) from `entry_core/route.rs:1177`, `entry_core/anime/route.rs:1127`, `browse/browse_screen.rs:224`, `browse/anime/browse_screen.rs:153`, `browse/mount.rs:556`. |
| Real or mockup | Mockup: "https://example.org/", progress 0.3, `status("web-browser-symbolic", "Web content")`. The parked request is never read. |
| Actions | Back, Forward and the menu do nothing (`menu_button(..., \|_\| {})`). |
| Fix | Take the request and open it in the system browser (`browse::anime::browse::open_external` already exists, used at `browse_screen.rs:223`), no new page. In-app WebKitGTK is a separate L task (`kuro_platform::webview` holds only data clearing; `src/shell/webview.rs` is chrome rules, see row 20). Size S for the browser hand-off. |

## 6. Mock player (`player.rs`, spec id `player`)

| Check | Finding |
|---|---|
| Module | `src/player.rs:52-262`. Reached through `screens::open_media` (`screens.rs:94-99`), `anime::route::open_player` (`:1575`), Library Continue (`library/mount.rs:337-340`), History and Updates rows only in their dead fallbacks (`src/live.rs:154,395,449`). |
| Real or mockup | Mockup. A grey `osd` box stands where video goes (`// ponytail: placeholder surface`), duration constant `1420.0`, chapter marks at 85 and 1330, "00:00 • Opening", "Skip Opening", speed button cycles a label, the seek bar moves a number. |
| Design | Own overlay look (`osd`/`toolbar`), no header row. |
| Actions | Every control changes only on-screen text. Nothing plays. |
| Reachable for | Any anime whose source is not in `live_anime_sources` (`anime/route.rs:97-102`), and every non-local anime resumed from the Library, because `local_flow_anime::open_episode(title, "<id>")` matches by name and fails. |
| Fix | All callers use `player_view::open_source_episode` / `open_episode` by id; on failure show a toast instead of a fake page; delete `player.rs`. Size M. |

## 7. Player window (`src/player_view/`)

| Check | Finding |
|---|---|
| Module | `src/player_view/mod.rs` (GLArea over libmpv), `playback.rs`, `source.rs`, `store.rs`, `menu.rs`. Reached from entry pages (`anime/route.rs:1572`) and local flow (`local_flow_anime/open.rs:55,85`). |
| Real or mockup | Real. mpv playback, resume and progress saving every 5 s, source video resolve, headers, side tracks, remembered volume, detach to own window. Tests: `a_clip_plays_seeks_pauses_and_its_progress_survives_a_reopen`, `a_stream_load_sets_its_headers_and_adds_its_subtitle_after_the_file_loads`, `a_handoff_keeps_the_file_its_position_and_its_pause_state` (not run; the first needs ffmpeg and libmpv). |
| Design | One header bar (back, title, Detach, tracks menu) and a bottom `osd toolbar`. Theme classes only, no hard-coded colours. The bar never hides on idle or tap. |
| Actions that work | Play/pause, seek bar, volume, fullscreen, Back, keyboard shortcuts (`kuro_player::shortcuts`), audio and subtitle track choice. |
| Missing | Next and previous episode, speed, aspect, chapters and Skip intro, auto-play, quality choice, lock, screenshot, subtitle and audio delay: these were in the mock (6) and the spec's sheets (`spec/reader_player.json`) and are not in the real window. The Player settings pages are stored but unused: `open_media` builds `PlayerPrefs { data_dir, ..default }` and reads only the decoder (`mod.rs:84-93`), so gestures, subtitle, audio, speed, filters, debanding, advanced and script settings have no effect. libmpv failure is only a log line and nothing opens (`mod.rs:76-81`). `docs/design/player-reader.md`, named in `mod.rs:3` and `widgets/detachable.rs:2`, does not exist. |
| Fix | Add the controls over the existing `Playback` calls and build `PlayerPrefs` from the stores; toast on mpv failure. Size L. |

## 8. Reader

| Check | Finding |
|---|---|
| Module | `src/reader/launch.rs` (`open_chapter`), `screen.rs`, viewers, sheets. Reached from the entry page (`entry_core/route.rs:702-706`), Library for local manga (`library/mount.rs:339`), notifications (`misc/notify.rs:323`), deep-link page (`shell/deep_link_page.rs:237`). The spec stub `build("reader")` with "Title 1" (`screens.rs:40`) has no caller. |
| Real or mockup | Real. Chapter loaders (download, local, HTTP with page-list prefetch), history, restore record, delete-after-read, error card with Retry, top bar (back, title opens manga, bookmark, overflow), bottom bar (slider, chapters, mode, rotation, crop, settings), page actions (copy, save, share, set cover), keys. Tests: `the_initial_load_opens_the_requested_chapter`, `a_missing_chapter_or_manga_gives_the_card_message`, `retry_repeats_the_whole_initial_load_and_can_succeed`, `next_and_previous_are_disabled_at_the_ends_and_the_buttons_follow_the_move`, `a_failed_adjacent_load_reports_its_message_and_keeps_the_chapter` (not run). |
| Design | Own chrome over the pages. The only literal colours are the reader background choices and scrim (`src/reader/theme.rs:73-79`), which are user settings, so acceptable. |
| Actions that do nothing | (a) Tracker sync: `track_update: None` (`launch.rs:203`), so reading never updates a tracker even with auto-update on. (b) Download ahead: `download_ahead: None` (`launch.rs:204`). (c) Rotation sheet stores the choice, but `ReaderEvent::SetOrientation` is dropped (`screen.rs:1347-1353`), so the window never rotates or locks. (d) Overflow entries (Open in WebView, Open in browser, Share) need an absolute URL (`chrome.rs:31-34`); source chapter URLs are usually relative, so these show "unknown error" (`screen.rs:1446-1448`). "Open in WebView" and "Open in browser" do the same thing (`screen.rs:1456`). |
| Fix | (a) needs a tracker service, L; (b) needs a download manager, L; (c) drop the sheet or implement fixed-size hint, S; (d) join the source's base URL, S. Overall M. |

## 9. Global search page, live (`global_search` with a parked route, intent or entry hand-off)

| Check | Finding |
|---|---|
| Module | `src/browse/search_screen.rs` (start, env, routes), `src/browse/pages.rs:538-715` (widget), `search_runner.rs`, `global_search.rs` (engine). Reached from Browse header search and menu (`browse/shell.rs:350,369`), Browse reselect, deep-link fallback (`shell/deep_link_page.rs:229`), entry page genre and source-line searches (`handoff::take_search`), the anime `animesearch` deep link (`browse/deep_link.rs:32`). |
| Real or mockup | Real. No fixture strings. Tests: `a_source_row_opens_its_listing_with_the_typed_text_or_on_popular`, `an_intent_wins_over_a_route_and_a_route_over_the_text_an_entry_parked`, `the_toggle_is_stored` area at `search_screen.rs:552-563`, engine tests `zero_pinned_sources_gives_an_empty_selection_with_no_fallback`, `the_auto_open_waits_then_opens_one_manga_and_shows_everything_else` (not run). |
| Design: header | A pushed page with back and the title "Global search", and a second search box inside the body (`pages.rs:540-545`). The design rule puts a search entry in the header centre and no repeated title (`ui::page`). |
| Design: states | No empty state. Blank query, no sources, or Pinned with no pinned source (the default; `zero_pinned_sources_gives_an_empty_selection_with_no_fallback`) draw a blank column under the chips (`pages.rs:611-620`, `redraw` just loops the rows). The migrate-missing error is a bare label (`search_screen.rs:126-132`). |
| Design: look | Chips use a `chip` style class that no CSS defines (`pages.rs:577`, nothing in `src/theme/style.css`), so they are stock toggle buttons. Source rows are bare boxes, not a boxed list or card; the open-source arrow is a raised icon button; the origin source is marked with a `▶` character in the title (`pages.rs:row_title`) instead of a badge pill. Result cards do use the new cover look (`kb-card-cover`, initials) through `view::grid_cell_lines`. |
| Actions | Search on Enter, chips (filter stored), Has results stored, arrow with the typed text, card open, long press (opens the entry, as there is no preview), single-result auto-open, link resolve: all wired to real code. |
| Fix | Search entry into the header (`ui::page` first `end` widget); `look::empty_state` for no query, no pinned sources (with an "All" action) and no results; a `kb-badge` for the origin; flat arrow; boxed rows. Size M (the empty states alone are S). |

## 10. Migrate search (`migrate_search`)

| Check | Finding |
|---|---|
| Module | Same page as 9 through `search_screen::body(true)`; opened by `handoff::open_migrate` (`entry_core/handoff.rs:190`) from the manga and anime entry pages, History (`updates/history_screen.rs:282`), the migrate list (`browse/migrate_screen.rs:189`) and the anime browse page. |
| Real or mockup | Real, no fixtures. A plain open without a hand-off would show the `entry::global_search` mockup, but every caller parks one. Tests: `a_migrate_search_starts_from_its_manga_and_a_missing_one_is_an_error_state`, `a_migrate_search_with_a_missing_manga_is_an_error_state_that_searches_nothing`. |
| Design | Same as 9. |
| Actions | A card opens the migrate dialog for manga (`manga_migrate_dialog`, `search_screen.rs:305`) and anime (`anime_migrate_dialog`, `:237`). Both run the engines. Real. |
| Fix | Same as 9. Size S on top of it. |

## 11. Migrate source listing (`migrate_source_search`)

| Check | Finding |
|---|---|
| Module | `browse_screen::page` `src/browse/browse_screen.rs:234-253`. |
| Real or mockup | Real listing, wrong behaviour. The route's `migrating` flag is ignored (`BrowseRoute::BrowseSource { source_id, query, .. }`, `:243`) and `BrowseRoute::AnimeBrowseSource` has no flag at all (`route.rs:84-86`, `search_screen.rs:91-97`). The listing is the ordinary one: a tap opens the entry, not the migrate dialog, and the route does not carry the entry being migrated. The fallback for this id is `entry::browse_source(.., true)`, 24 "Title n" cards (`entry.rs:605-650`). |
| Design | See 12. |
| Fix | Carry the old entry id in the route, then build the env with `open_entry` = migrate dialog. Size M. |

## 12. Source listing (`browse_source`)

| Check | Finding |
|---|---|
| Module | `src/browse/pages.rs` (`source_page`, `:182-436`), `header_bar.rs`. Real. Falls back to the 24-card mockup when no route is parked or `directory::current()` is `None` (`browse_screen.rs:244`, `entry.rs:605-650`). |
| Design | Old toolbar style: header with title and three buttons, then a revealer row with a search box (`pages.rs:253-258`), then a row of plain `ToggleButton` chips, then a separator. The new rule is search in the header and no extra rows. Empty and error states use `browse::view::empty_state` (older) rather than `look::empty_state`. |
| Actions | Real (popular, latest, filter sheet, display mode, long-press favourite flow, WebView, settings). The WebView entry reaches mockup 5. |
| Fix | Move the search into the header centre, restyle chips as header toggles or a `linked` group, switch empty states. Size M. |

## 13. Migrate entries (`migrate_entries`)

| Check | Finding |
|---|---|
| Module | `src/browse/migrate_screen.rs`, `pages.rs:736-770`. Real, boxed list in the page column, covers from the shared cell. Falls back to eight "Title n" rows only with nothing parked (`entry.rs:707-721`; the only opener parks). |
| Design | List is new. Empty and failed states are bare labels with `empty_screen` (`migrate_screen.rs:73-81`), no icon and no action. |
| Fix | `look::empty_state` for both. Size S. |

## 14. Download queue (`download_queue`)

| Check | Finding |
|---|---|
| Module | `src/screens.rs:430-437`. |
| Design | New: one header, the Library empty state with a Library button; test `the_download_queue_is_an_empty_state_with_one_way_forward`. |
| Real data | None. The page is always "No downloads" and the More row is forced to Stopped (`screens.rs:384-387`). The Library's Download actions log "no download manager is registered" and do nothing visible (`library/mount.rs:367-375`). |
| Fix | A download manager; until then the Library's download menu should say so. Size L. |

## 15 to 18. Done

| Page | Evidence | Note |
|---|---|---|
| Categories | `src/live.rs:323-336`, header switch and Add in `screens.rs:441-446` | Real list, rename, hide, delete, drag reorder, empty state "You have no categories" with Add. Writes ignore errors (`let _ =`). |
| Statistics | `src/updates/mount.rs:222-235` | Real data, switch in the header (`kind_switch` puts it in `end`). `docs/design/other-tabs.md` "Not done here" is out of date on this. Page body not read in depth. |
| Storage | `src/updates/mount.rs:262-271` | Same as Statistics. |
| Source preferences | `src/browse/source_prefs_screen.rs:62-73` | `ui::page` header, real rows; falls back to the spec page only if nothing is parked. |

## 19. Dead stand-ins (not_applicable): delete

None of these ids has a caller in the shipped app, or the only caller is another mockup.

| Id or code | Why dead |
|---|---|
| `migrate_season_select` (`entry.rs:652`) | no `open(...)` of that id anywhere |
| `onboarding`, `new_update`, `crash` (`screens.rs:456-505`) | the real ones are `shell::onboarding`, `app_update` and `reporter`; no `ui::open` of these ids; fixture text "v0.1.0", "java.lang.RuntimeException: something went wrong" |
| `home` | spec entry only |
| `extension_details`, `extension_filter`, `sources_filter`, `extension_repos` (`entry.rs:24,497,500,565,577`) | opened only by the fallback Browse in `entry.rs`; the real Browse has `ext_details_page` and `settings_manga_extension_repos` |
| `reader` spec stub (`screens.rs:40`) | no `open("reader")` |
| Library, Updates, History, Browse fallbacks (`screens.rs:22-26,236`, `entry.rs:469-600`) | run only when `data::is_installed()` is false, which cannot happen in the window (`main.rs:195-200`); they feed the smoke run (`main.rs:411-412`) |

Deleting these removes most of `entry.rs` (842 lines) and `player.rs`, but only after rows 1 to 6 no longer reach them.

## 20. Built but not reached (lib_only)

| Code | Evidence |
|---|---|
| `src/shell/webview.rs` (header chrome, back and forward rules, progress) | referenced only by tests; no page uses it (row 5) |
| `shell::intents::route::route_request`, including `Intent::ExtSearch` (the Android SEARCH intent into global search) | referenced only by its own tests and `dropped/mod.rs:178`; the desktop path is `shell::launch::apply` → `deep_link_page::open_search` and `pending::open_search_intent` (`browse/deep_link.rs:32`) |

## Order of work

1. Rows 1 and 2: Library open and Library global search (both S, remove the user-visible placeholders).
2. Row 6: stop the mock player from ever opening (M).
3. Row 5: WebView opens the browser (S).
4. Rows 9, 10, 13: global search look and empty states (M).
5. Rows 3 and 4: tracking sheet (L, needs a decision on trackers).
6. Rows 7, 8, 11, 12: player controls and settings, reader gaps, migrate listing, listing header.
7. Delete row 19 and the `entry.rs` mockups.
