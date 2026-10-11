# Kurobayomi P0 audit: what really works

Date: 2026-10-11. This report sums up the 18 audit files next to it: `p0-01.md` to `p0-15.md` (all 1,196 must-have tasks, called P0) and `ui-1.md` to `ui-3.md` (every page, sheet and dialog).

How it was done: the code was read, nothing was built or run. Cargo was not run, so no test was run either: "tests exist" means a test with that name is in the source. A task counts as working (DONE) only when its code can be reached from the app window, not when it only sits in a crate. I re-checked the most important claims myself in the code (marked "checked" below).

Words used in this report:

- **DONE**: built, has tests by name, and the app window reaches it.
- **LIB_ONLY**: built and tested in a library crate or module, but nothing in the app calls it. The user never sees it.
- **PARTIAL**: reachable, but some of what the task asks for is missing or wrong.
- **MISSING**: not built at all.
- **NOT_APPLICABLE**: dropped on purpose, with a written decision.

## Short version

- **55% of P0 really works in the app** (661 of 1,196 tasks).
- **18% is built but switched off**: the code and its tests exist in a crate, but the window never calls them (219 tasks).
- **26% half works** (305 tasks). **Under 1% is not built** (9 tasks).
- What is left is mostly **wiring**: connecting finished code to the window. It is not polish.
- Strong parts: the reader (88% done), library and manga details (78%), anime details and Browse (72%), databases and preferences.
- Weak parts: the video player (16%), AniList tracking (11%), downloads (25%, and the download engine never starts), extensions (cannot install anything).
- The placeholders you saw are real. Opening an entry from the Library, Updates or History, and "Search globally" from the Library, still show the old mockup pages. The fix for both is small.

## 1. The numbers

### Overall

| Verdict | Tasks | Share |
|---|---|---|
| DONE | 661 | 55.3% |
| LIB_ONLY | 219 | 18.3% |
| PARTIAL | 305 | 25.5% |
| MISSING | 9 | 0.8% |
| NOT_APPLICABLE | 2 | 0.2% |
| **Total** | **1,196** | 100% |

### By plan area group

Shares are rounded to whole percent, so a row can add up to 99 or 101.

| Group (plan areas) | Tasks | DONE | LIB_ONLY | PARTIAL | MISSING | N/A |
|---|---|---|---|---|---|---|
| App start, settings store, databases, repositories (01a-05a) | 91 | 58 (64%) | 3 (3%) | 27 (30%) | 3 (3%) | 0 |
| Domain logic: history, flags, library, sync (06a-11a) | 89 | 40 (45%) | 14 (16%) | 35 (39%) | 0 | 0 |
| Network, storage, local files, source API (12a-15a) | 94 | 54 (57%) | 23 (24%) | 17 (18%) | 0 | 0 |
| Source layer and manga extensions (16a-18a) | 91 | 48 (53%) | 21 (23%) | 17 (19%) | 5 (5%) | 0 |
| Anime extensions and source runtime (19a, 20a) | 33 | 15 (45%) | 7 (21%) | 11 (33%) | 0 | 0 |
| Downloads (23a, 25a) | 24 | 6 (25%) | 15 (63%) | 3 (13%) | 0 | 0 |
| Library update jobs (26a) | 39 | 26 (67%) | 5 (13%) | 8 (21%) | 0 | 0 |
| AniList tracker (28a) | 35 | 4 (11%) | 28 (80%) | 3 (9%) | 0 | 0 |
| Covers, TLS, app shell (34a-36a) | 60 | 33 (55%) | 4 (7%) | 22 (37%) | 0 | 1 (2%) |
| Library and manga details (37a-38b) | 74 | 58 (78%) | 0 | 16 (22%) | 0 | 0 |
| Anime details and Browse (39a-40b) | 86 | 62 (72%) | 1 (1%) | 23 (27%) | 0 | 0 |
| Browse anime, History (41a-42a) | 62 | 36 (58%) | 2 (3%) | 24 (39%) | 0 | 0 |
| Reader (43a, 43b, 44a) | 137 | 121 (88%) | 2 (1%) | 14 (10%) | 0 | 0 |
| Video player (45a, 45b, 46a, 46b, 48a) | 132 | 21 (16%) | 66 (50%) | 44 (33%) | 1 (1%) | 0 |
| Settings screens (47a, 47b, 47ba) | 68 | 41 (60%) | 3 (4%) | 23 (34%) | 0 | 1 (1%) |
| Theme and design system (49a, 49b, 49ba) | 45 | 18 (40%) | 19 (42%) | 8 (18%) | 0 | 0 |
| Translations, helpers, launch, build, platform (51a-54a, req) | 36 | 20 (56%) | 6 (17%) | 10 (28%) | 0 | 0 |
| **All P0** | **1,196** | **661 (55%)** | **219 (18%)** | **305 (26%)** | **9 (1%)** | **2 (0%)** |

### What share of P0 really works in the app

About **55%**. That is the most it can be, not a proven number:

- DONE rests on test names. No test was run.
- Some DONE rows are generous. Example: 26 library-update tasks are DONE because the update pass runs, but every entry in it ends with "Source not found" (gap 6).

If you count each PARTIAL task as half working, the figure is about 68%. The 219 LIB_ONLY tasks do nothing for the user today.

### Correction to the README progress claim (the README was not edited)

The README (`README.md`, Progress section, dated 2026-10-10) says: "Every must-have (P0) and important (P1) task has been built; what is left is verification, polish and the optional tasks." The audit does not support that.

- **Built:** true for 99% of P0 tasks. Code exists for all but 9.
- **Works:** only 55% of P0 is reachable from the window and complete. What is left is wiring code into the window, not verification and polish.
- **The 9 MISSING tasks:**
  - the AppContext startup design (3 tasks);
  - the manga extension index, find-available, source-id index and uninstall (5);
  - the player sheet host wiring (1).
- **README rows that are too high:**
  - *Downloads* (says 20% works): the download engine never starts, so no download can happen.
  - *AniList tracking* (8%): a browser login can never finish.
  - *Video player* (58%): only 16% of P0 player tasks are done, and the player ignores its settings.
  - *Screens* (62%): five mockup pages can still be reached from real buttons.
- **README rows that are too low:** *Manga reader* (68%): the reader's P0 tasks are 88% done.
- **The overall "about 55% works end to end"** covers all 3,800 tasks. The audit only looked at P0, which should be the best-built part. If P0 alone is at 55%, the overall figure is probably lower.

## 2. The 15 most important gaps, in the order a user would notice them

1. **Opening a title from the Library, Updates, History or a desktop shortcut shows a fake sample page.**
   - The cause is that pages are opened by title, not by id. `screens::open_entry` (`src/screens.rs:83-92`, checked) only finds titles from local folders. Every other title goes to the old mockup `entry::entry` (`src/entry.rs:85`), which shows "Source A (EN)", "Studio" and 24 made-up chapters.
   - Callers: `src/library/mount.rs:328`, `src/updates/updates_screen.rs:400`, `src/updates/history_screen.rs:225,277`, and `src/open_link.rs:36-40`, which even builds the text "Title <id>".
   - The real pages (`entry_core::route::open(id)`, `entry_core::anime::route::open(id)`) already work from Browse, search and notifications. Size S.
2. **"Search globally" from the Library shows the placeholder page.**
   - `open_global_search` (`src/library/mount.rs:168-179`, checked) opens the page without saying what to search. `browse::search_screen::body` then gives up (`src/browse/search_screen.rs:383-385`, checked), so `src/screens.rs:26` falls back to the mockup `entry::global_search` (`src/entry.rs:657`): "Source A/B/C", "Title 1..8", a progress bar stuck at 60%, and "No results found".
   - The same happens for the "Search globally" button on an empty library and for the "Search globally for ..." row. Size S.
3. **Resuming an anime opens a fake player with no video.**
   - `screens::open_media` (`src/screens.rs:94-99`, checked) sends any episode that is not from a local folder to `src/player.rs`. That page has a fixed 23:40 timeline, "00:00 • Opening", a "Skip Opening" button and no video.
   - Reached from Library Continue, History, Updates, and from the anime page when its source is not a live one (`src/entry_core/anime/route.rs:1575`). Size M.
4. **Downloads do not exist in the running app.**
   - The download manager in `kuro-downloads` is never started. `updates::download_host::install` and `entry_core::anime::download_backend::install_queue` have no caller (checked).
   - So nothing works on any Download button (Library, manga page, Updates), the Download queue page (always empty, More says "Stopped"), Storage delete, automatic downloads after a library update, or delete-after-read for online chapters.
   - The download index is never scanned either, so chapters already on disk show as not downloaded. With "Downloaded only" on, the Library's Continue button always says "Next chapter not found". Size L.
5. **Extensions cannot be installed, updated, removed or refreshed, and an installed extension never runs.**
   - The Extensions tab uses a stub (`HostBackend`, `src/browse/mount.rs:197-208`, checked): Install ends in an error, Uninstall and Refresh do nothing.
   - The Trust button writes the wrong value, so trusting changes nothing.
   - Manga packages are scanned with an empty trust list, so all of them show as untrusted.
   - The plugin runtime (`kuro-source-wasm`) is not linked into the app, and no manga extension list is ever downloaded.
   - Only the built-in sources and the recipe sources can be browsed or read. Size L.
6. **The library update and backup restore cannot reach any source.**
   - Both use an empty source list called `NothingInstalled` (`src/misc/runner.rs:71`, `src/backup_run.rs:42`, checked).
   - The refresh button and the timer start a real update pass, but every entry ends with "Source not found" and an error notice. No new chapters are ever found this way.
   - The restore check lists every source as missing and every tracker as logged out. Size M.
7. **The player ignores the Player settings and erases your mpv files.**
   - `src/player_view/mod.rs:75-83` (checked) starts mpv from built-in defaults. Only the remembered decoder is read from settings.
   - Ignored settings: subtitle style, audio language, speed, GPU options, debanding, filters, "open fullscreen", the "mark as seen" threshold and "preserve watching position".
   - Each start also writes empty `mpv.conf` and `input.conf` files, so text typed into Settings > Player > Advanced is wiped. Size M.
8. **The player window only has basic controls.**
   - It has play/pause, a seek bar, volume, fullscreen and a track menu.
   - Missing: previous or next episode, autoplay, speed, aspect, chapters, skip intro, quality or hoster choice, gestures, lock and auto-hide.
   - A downloaded episode of an online source is streamed again over the network.
   - A playback error only goes to the log, and the window stays black. The logic for nearly all of this is already in `kuro-player`. Size L.
9. **Tracking (AniList and the others) does not work.**
   - The browser login never comes back: `src/shell/launch.rs:61` (checked) only raises the window and never calls `tracker_host::on_redirect`.
   - The Tracking sheet and tracker search on every entry page are mockups (`src/entry.rs:723-795`: "AniList / MyAnimeList / Kitsu", "12 / 24", "8.0").
   - Reading never updates a tracker (`track_update: None`, `src/reader/launch.rs:204`, checked). Size L (the login part alone is S).
10. **"Open in WebView" shows a fake page.**
    - The page shows "https://example.org/" with a progress bar stuck at 30% (`src/entry.rs:797`). Reached from entry pages, Browse and the extension list.
    - In the reader, Open in browser, Open in WebView and Share fail with "Unknown error" for most sources, because they need a full web address and sources store short ones.
    - Size S for opening the real address in the system browser.
11. **The Library tab goes stale.**
    - It only reloads when a setting changes, after a folder scan or after its own bulk actions. It does not watch the database.
    - So reading a chapter, adding a title from Browse or finishing an update does not update it until restart. This was found by reading the code; it was not seen in a running window. Size S.
12. **The folder picked during onboarding is not recognised.**
    - Onboarding stores the plain path (`src/shell/onboarding.rs:277`, checked). The rest of the app expects it encoded.
    - So the Storage page says "No storage location set", the folder tree is not created, and backups and downloads do not use the folder. Size S.
13. **Theme choices are incomplete.**
    - There is no app theme picker after onboarding: the "App theme" row is skipped, and the theme cards that were built have no caller.
    - Changing Theme mode on Appearance only half applies: the app palette keeps the startup mode until restart, and "System" can force light.
    - Pure black is a separate hard-coded layer. Size M.
14. **Settings pages show rows that do nothing.**
    - Android-only rows are still shown: Tablet UI, cutout area, Secure screen, battery optimisation, Installer, legacy decoder, display profile.
    - "Always use external downloader for anime" writes a key nothing reads.
    - "Show next episode's airing time" appears twice because of a trailing space in the spec.
    - Worker info is always empty.
    - About, Help, onboarding and More links point to Aniyomi's website, Discord and GitHub. Size S.
15. **Error messages and cookies are not wired.**
    - Browse shows raw text such as "HTTP error 404" instead of the helpful message ("check website in WebView"). The formatter `source_error_message` has no caller.
    - Clear cookies and cookie import change a separate copy of the cookie jar, not the one the running sources use.
    - The network client's notices (for example Cloudflare) are thrown away (`src/net_client.rs:13`). Size S.

Next in line:

- Covers on the anime Related and Seasons grids never load (`ui::cover` is still a grey frame).
- The category delete dialog shows a raw `%s`.
- Chapter numbers are stored as 1.100000023841858 instead of 1.1 (`kuro-domain/src/items/chapter_sync.rs:72,96`).
- One broken archive as the first chapter fails a whole local manga folder.
- The reader's rotation choice is stored but never applied.
- The Library always opens on its first category tab.
- With "Move Browse to More", the More page has no Browse row.
- The CI boundary check fails because two older audit files contain its forbidden words; `scripts/check-platform-boundary.sh` does not skip `docs/` (checked).

## 3. UI findings

### Pages the redesign did not reach

The redesign (`docs/design/main-window.md`, `other-tabs.md`) reached the main shell: the sidebar, the single header row, boxed lists, word badges, the Library, Updates, History, Upcoming, Statistics, Categories, More and most settings pages. It did not reach these:

| Page | What is old about it | Where |
|---|---|---|
| Global search (the real page) and migrate search | Title row plus a second search box in the body (the design puts search in the header); chips use a `chip` style no CSS defines; plain boxes instead of boxed lists; no empty state, so the default "Pinned" filter with no pinned source shows a blank column | `src/browse/pages.rs:538-720` |
| Browse source listing | Search box in a second row, plain toggle chips, older empty state | `src/browse/pages.rs:182-436` |
| Manga and anime details | Chapter and episode lists are full-width lists, not boxed; own `@kuro_*` colours; grey picture placeholder instead of the initials tile | `src/entry_core/list_view.rs`, `anime/row_view.rs`, `cover_view.rs:88` |
| Player window | Own toolbar bar that never hides | `src/player_view/mod.rs` |
| Onboarding | No page column, large `title-1` heading, drop-downs and loose labels, Aniyomi links | `src/shell/onboarding.rs:140` |
| Settings dialogs | Window title and heading repeat the same text | `src/pref_model/widgets/dialog.rs:75,97` |
| Script code editor | Header always says "file.lua"; fixed colours meant for a dark background | `src/settings_ui/player/scripts.rs:683` |
| Clear database, backup schema, migrate entries | Bare labels instead of the empty-state or row look | `src/settings.rs:272,310`, `src/browse/migrate_screen.rs:73-81` |
| Settings main list | Subtitles still say "App lock, secure screen" and "battery optimizations" | `src/pref_model/shell.rs:73-80` |

The new design-system pieces themselves were never used by any screen:

- in `src/theme/`: the kaomoji empty state, the delayed loading spinner, the cover placeholder, the icon set (download indicator, reading-mode icons, caret, sidebar two-state icon), and the `.kb-selected` and `.kb-active` classes;
- in `src/theme/components/`: badges, progress ring, section card, settings rows, dialog layout;
- in `src/shell/`: app bar, search bar, tabbed page, empty page, date text.

Screens kept their own versions instead.

Nobody has yet looked at these pages in the Tokyo Night theme. The pages use the `card`, `pill` and `accent` styles, which stock GTK 4.24 does not define (Tokyonight-Dark does), and `chip`, which neither defines.

### Pages that still show placeholders

Can be reached from real buttons today:

| Placeholder | What it shows | Opened from |
|---|---|---|
| Sample entry page (`entry::entry`) | "Source A (EN)", "Studio", "Summary of the series goes here", 24 fake chapters | Library, Updates, History, duplicate dialog, desktop actions |
| Global search mockup (`entry::global_search`) | "Source A/B/C", "Title 1..8", 60% progress, "No results found" | Library "Search globally" (both buttons) |
| Tracking sheet (`entry::tracking`) | AniList bound, MyAnimeList, Kitsu, "Watching", "12 / 24", "8.0" | Tracking button on every manga and anime page; also opens by itself after adding to the library when a tracker is logged in |
| Tracker search (`entry::tracker_search`) | "Title 1..4", "Author name", "Score: 8.4"; Track does nothing | Tracking sheet |
| Tracking dialogs | Spec dialogs whose OK changes nothing | Tracking sheet |
| WebView (`entry::webview`) | "https://example.org/", 30% progress, "Web content" | WebView buttons, Browse menu, extension list |
| Fake player (`src/player.rs`) | Fixed 23:40 timeline, "Skip Opening", no video | Anime resume from Library, History, Updates; anime page fallback |
| Download queue | Always "No downloads"; More subtitle stuck on "Stopped" | More, Updates |
| Reader page rows | A loading page shows "Chapter <database id> page <n>" above the spinner | Reader (`src/reader/viewers/engine.rs:151-153`) |

Cannot be reached, but still in the code (delete once the rows above are fixed): `entry::browse`, `browse_source`, `seasons`, `migrate_entries`, `migrate_sources`, `sources`, `extensions` (with "Extension A"), the `screens::library/updates/history` fallbacks, `screens::build("reader")` and `build("player")` with "Title 1", and the mock arms in `src/settings.rs` (including a static "Apache License" text). Most of `src/entry.rs` (842 lines) and all of `src/player.rs` can go.

### Global search

- **From the Library: placeholder.** Both the "Search globally" button and the "Search globally for ..." row open the mockup (gap 2). Checked in code: `src/library/mount.rs:168` opens the page without parking a request, and `search_screen::body` returns `None` when nothing is parked.
  - Note: `p0-10.md` says "all current openers park first". That is wrong for this opener.
  - Fix: call `browse::pending::open(BrowseRoute::GlobalSearch { kind, source_id: None, query })` there, as the Browse header (`src/browse/shell.rs:350`) and deep links already do. Then delete the mockup fallback at `src/screens.rs:26`.
- **From Browse, an entry page or a deep link: the real page.** The search engine works: it searches each source, filter chips, the "Has results" setting is stored, a card opens the entry by id, and a single result opens on its own. But the look is old (see the table above) and there is no empty state.
- Global search ignores the two "Hide entries already in library" switches (T-47a-060).
- It shows raw error text, like Browse (gap 15).
- Migrate search is the same page with the same look. It works because every caller parks a request first.

## 4. Fix plan

All 533 gap tasks (LIB_ONLY, PARTIAL, MISSING) are assigned below to exactly one lane, checked by script. Lanes follow the repo rule in `CLAUDE.md`: crate lanes own crates, GUI lanes own separate `src/` folders. Shared files (`src/main.rs`, `src/ui.rs`, `src/screens.rs`, `Cargo.toml`) get small added lines only, except where a lane below is named as their owner for that wave. Size: S under half a day, M one to two days, L more.

### Order

| Wave | Lanes (run in parallel) | Why this order |
|---|---|---|
| 0 | Z | CI is red now; one-line fix |
| 1 | A, C, G1, I, J, L, M, N, O (9 lanes, 198 tasks) | Removes every placeholder you can reach, fixes wrong data, and needs no new services |
| 2 | E, D, K, G2, H (5 lanes, 179 tasks) | The missing services: source registry, download manager, tracking. Needs wave 1 to have freed `src/library`, `src/entry_core`, `src/shell`, `src/screens.rs` and `src/player_view` |
| 3 | F, G2b, G2c (3 lanes, 96 tasks) | Extensions need the registry from E; the player controls and sheets need the session from G2 |
| 4 | Q (60 tasks) | Removing duplicates and adopting shared components touches many folders, so it goes last |

E (50 tasks), F (51), G2 (43) and Q (60) are above the 25-tasks-per-lane rule of thumb. Run each as two back-to-back runs in the same lane.

### Lanes

| Lane | What it fixes | Owns (files) | Tasks | Size | Wave |
|---|---|---|---|---|---|
| Z | CI boundary check skips `docs/` | `scripts/check-platform-boundary.sh` | none (audit finding) | S | 0 |
| A | Open everything by id; Library global search; delete the mock entry, mock global search, mock player and WebView arm; Library watches the database; More page from `pref_model::more`; category delete `%s`; Updates rows react to Enter | `src/screens.rs`, `src/entry.rs`, `src/entry_model.rs`, `src/player.rs`, `src/open_link.rs`, `src/live.rs`, `src/selection.rs`, `src/library/**`, `src/updates/**` (not `download_host.rs`) | 31 | M | 1 |
| C | Entry page backend: WebView and Share buttons, source Settings item, cover delete on remove, cover refresh rules, enhanced tracker bind, external player, downloaded-episode playback, number format, single-language source name, `from_source`, WebView opens the system browser, anime Related and Seasons covers | `src/entry_core/**` (not `tracking/**`, not `anime/download_backend.rs`) | 34 | L | 1 |
| G1 | Player reads its settings (one mapper from `kuro_prefs::player`), stops wiping `mpv.conf`, threshold and preserve-position settings, open fullscreen, error toast | `src/player_view/**` | 20 | M | 1 |
| I | Onboarding stores the folder like Settings does and checks it can write there; tracker login lands (`launch.rs` calls `tracker_host::on_redirect`); stores deep link; onboarding look and links | `src/shell/**` | 6 | S | 1 |
| J | App theme picker on Appearance; theme mode applies the palette; one theme-mode path; dead Android rows dropped; duplicate Tracking row; external downloader rows typed; About and Help links; repo fingerprint and toast; dialog titles; per-category sort resets both kinds | `src/settings_ui/**` (not `cookie_import.rs`, `tracker_host.rs`, `tracking.rs`), `src/pref_model/**`, `src/theme/**`, `src/settings.rs`, `src/toast.rs`, `spec/*.json`, `crates/kuro-domain/src/extension_repo/**` | 34 | M | 1 |
| L | Reader: relative chapter addresses for the web actions, data saver, chapter reference counts, page-area spinner, loading-row text, slider keeps the menu open, rotation, set cover for local manga, Stub "Source not installed" message | `src/reader/**` | 26 | M | 1 |
| M | One shared cookie jar for the app; live notice channel for the network client; rate limiter used | `crates/kuro-net`, `src/net_client.rs`, `src/settings_ui/cookie_import.rs`, `src/browse/cookies.rs` | 6 | S | 1 |
| N | Chapter number precision (f32), one name cleaner for chapters and episodes, sync tests, stale coverage matrix | `crates/kuro-domain/src/items/**`, `crates/kuro-domain/src/entries/coverage.rs`, `crates/kuro-domain/tests/**`, `crates/kuro-data/src/{sanitize,episode,tests}.rs`, `crates/kuro-db/src/episode_sanitizer.rs`, `crates/kuro-testkit` | 12 | S | 1 |
| O | Startup: verbose-logging setting applied at start, one migration path that stores `last_version_code`, `Prefs::open` with repair, storage tree created at start, decide AppContext (build or drop), separate episode default keys, sidebar bars built from the nav style, release profile | `src/main.rs`, `src/data.rs`, `src/startup.rs`, `src/reporter.rs`, `crates/kuro-core`, `crates/kuro-prefs`, `crates/kuro-domain/src/entries/defaults.rs` | 29 | M | 1 |
| E | One source registry fed by built-in, recipe and local sources; library update, backup restore and the reader use it; readiness gate is real; stub names stored; local source in Browse; summary as a desktop notification; job registry filled | `crates/kuro-domain/src/source/**`, `crates/kuro-domain/src/app/source.rs`, `crates/kuro-data/src/source.rs`, `crates/kuro-local`, `src/ext_host.rs`, `src/browse/directory.rs`, `src/browse/anime/directory.rs`, `src/browse/mount.rs`, `src/source_gate.rs`, `src/misc/**`, `src/backup_run.rs`, `src/pref_model/backup_pages.rs`, `src/library_update/**`, `src/local_flow/**`, `src/local_flow_anime/**` | 50 | L | 2 |
| D | Start the download manager; queue store and migration; index scan at start; register it with the Library, entry pages, Updates and Storage; queue page and More subtitle; anime queue | `crates/kuro-downloads`, new `src/downloads/`, `src/updates/download_host.rs`, `src/entry_core/anime/download_backend.rs`, `src/shell/host.rs`, new queue migration file in `crates/kuro-db`, `src/screens.rs` (queue page and More row only) | 27 | L | 2 |
| K | Browse: helpful error text, loading rows while paging, manga source Settings, Global search header button, cover clearing and add effects on favourite, source list and language groups from the tested domain functions, Global search look and empty states, hide-in-library in global search, migrate source listing | `src/browse/**` (not `directory.rs`, `anime/directory.rs`, `mount.rs`, `cookies.rs`) | 29 | M | 2 |
| G2 | Player session: property observers, events, episode list, previous and next, autoplay, hosters and quality, offline episodes, title properties, keys forwarded to mpv, single window, completion hooks | `src/player_view/**`, `crates/kuro-player` | 43 | L | 2 |
| H | Tracking: real tracking sheet, tracker search, status, score and date pickers, bind, refresh, delete, reader and player progress to trackers, secret-store notice | `crates/kuro-track`, `src/entry_core/tracking/**`, `src/settings_ui/tracker_host.rs`, `src/settings_ui/tracking.rs` | 30 | L | 2 |
| F | Extensions: real install, update, uninstall, refresh and trust; plugin runtime linked; manga extension list fetch; installed sources registered in E's registry | `crates/kuro-extensions`, `crates/kuro-source-wasm`, `crates/kuro-source`, `crates/kuro-source-http`, `src/ext_host.rs`, `src/browse/mount.rs`, `src/browse/extensions_page.rs`, `src/browse/installs.rs` | 51 | L | 3 |
| G2b | Player controls: auto-hide, lock, gestures, double tap, speed, aspect, volume boost, brightness, custom buttons | new `src/player_view/controls/` | 30 | L | 3 |
| G2c | Player sheets: quality and hosters, audio, subtitles, chapters, speed, More sheet | new `src/player_view/sheets/` | 15 | M | 3 |
| Q | Remove the duplicate copies (sanitisers, pagers, history recorders, migrators, helpers); synchronous database writes off the GTK thread; adopt or delete the unused theme and shell components; translate the spec titles and literals; move `reqwest` use in `src/` behind `kuro-net` | many folders, small edits; run alone | 60 | L | 4 |

Each duplicate pair in Q needs a keep-or-delete decision first. The pairs are listed in `p0-02.md` G5, `p0-03.md` point 1, `p0-04.md` point 5, `p0-13.md` point 4 and `p0-15.md` point 5.

### Task ids per lane

All ids start with `T-`.

- **A (31):** 03a-034, 05a-026, 07a-022, 07a-040, 08a-016, 08a-032, 08a-033, 15a-051, 36a-060, 37a-012, 37a-017, 37a-022, 37a-033, 38a-001, 39a-074, 39b-063, 40a-034, 42a-002, 42a-006, 42a-008, 42a-014, 42a-016, 42a-021, 46b-046, 47ba-010, 47ba-013, 49b-010, 49b-029, 52a-007, 52a-008, 53a-055
- **C (34):** 07a-024, 07a-025, 08a-030, 10a-004, 10a-007, 15a-044, 17a-014, 17a-028, 20a-004, 34a-011, 34a-012, 34a-052, 34a-056, 34a-059, 36a-049, 38a-017, 38a-019, 38b-004, 38b-010, 38b-011, 38b-017, 38b-018, 38b-028, 38b-041, 39a-028, 39a-051, 39a-063, 39b-017, 39b-021, 39b-024, 39b-061, 39b-062, 41b-044, 45a-068
- **G1 (20):** 45a-003, 45a-004, 45a-005, 45a-009, 45a-012, 45a-026, 45a-028, 45a-030, 45a-031, 45a-033, 45a-040, 45a-044, 45a-052, 45a-064, 45b-026, 45b-044, 46b-005, 48a-013, 48a-022, 48a-036
- **I (6):** 13a-015, 28a-040, 28a-047, 36a-002, 36a-026, 47b-017
- **J (34):** 01a-021, 02a-023, 02a-030, 08a-012, 09a-049, 37a-040, 47a-002, 47a-003, 47a-006, 47a-011, 47b-001, 47b-020, 47b-024, 47b-052, 47ba-004, 47ba-008, 47ba-040, 47ba-041, 47ba-042, 47ba-046, 47ba-047, 47ba-048, 47ba-049, 47ba-052, 47ba-070, 47ba-073, 48a-010, 48a-042, 48a-056, 49a-059, 49b-006, 49b-007, 49b-043, 49ba-002
- **L (26):** 12a-012, 13a-022, 13a-023, 13a-024, 14a-020, 15a-027, 15a-034, 34a-024, 34a-037, 34a-038, 34a-040, 43a-002, 43a-007, 43a-014, 43a-023, 43a-040, 43a-076, 43a-078, 43a-088, 43b-013, 43b-030, 43b-032, 44a-026, 44a-045, 44a-062, 49ba-036
- **M (6):** 12a-009, 12a-010, 12a-011, 12a-014, 12a-036, 35a-031
- **N (12):** 05a-009, 05a-016, 06a-034, 07a-023, 07a-033, 07a-034, 10a-008, 10a-009, 10a-036, 15a-004, 26a-085, 43b-011
- **O (29):** 01a-001, 01a-002, 01a-003, 01a-004, 01a-005, 01a-006, 01a-007, 01a-009, 01a-012, 01a-013, 01a-017, 01a-018, 02a-007, 02a-020, 02a-035, 02a-039, 09a-048, 11a-001, 11a-002, 13a-048, 13a-049, 36a-005, 36a-006, 36a-008, 39b-010, 43b-004, 49ba-015, 54a-001, 54a-003
- **E (50):** 01a-037, 06a-040, 06a-041, 07a-032, 09a-004, 09a-005, 10a-001, 14a-001, 14a-003, 14a-012, 14a-013, 14a-014, 14a-016, 14a-017, 14a-018, 14a-019, 14a-025, 14a-043, 14a-051, 15a-035, 17a-004, 17a-005, 17a-006, 17a-007, 17a-008, 17a-009, 17a-010, 17a-011, 17a-012, 17a-013, 17a-036, 17a-037, 20a-003, 20a-094, 26a-004, 26a-006, 26a-008, 26a-009, 26a-024, 26a-025, 26a-032, 26a-033, 26a-092, 26a-129, 26a-130, 26a-131, 40b-003, 40b-040, 41b-010, 52a-056
- **D (27):** 08a-022, 09a-021, 15a-028, 23a-005, 23a-006, 23a-007, 23a-008, 23a-009, 23a-010, 23a-011, 23a-012, 23a-013, 23a-014, 23a-019, 23a-020, 23a-023, 23a-024, 23a-027, 23a-030, 23a-035, 23a-036, 37a-026, 39a-039, 39b-002, 47ba-015, 47ba-016, 52a-010
- **K (29):** 06a-042, 06a-043, 06a-081, 09a-060, 10a-018, 10a-019, 17a-016, 17a-017, 17a-045, 20a-005, 40a-001, 40a-002, 40a-038, 40b-014, 40b-041, 41a-005, 41a-006, 41a-007, 41a-042, 41a-049, 41b-004, 41b-015, 41b-016, 41b-018, 41b-020, 41b-023, 41b-042, 41b-053, 47a-060
- **G2 (43):** 16a-019, 45a-001, 45a-006, 45a-007, 45a-010, 45a-011, 45a-014, 45a-015, 45a-018, 45a-019, 45a-024, 45a-025, 45a-036, 45a-037, 45a-038, 45a-039, 45a-084, 45b-001, 45b-002, 45b-003, 45b-004, 45b-005, 45b-006, 45b-007, 45b-008, 45b-009, 45b-010, 45b-012, 45b-013, 45b-014, 45b-015, 45b-016, 45b-017, 45b-018, 45b-023, 45b-027, 45b-030, 45b-061, 45b-064, 45b-066, 45b-073, 45b-075, 48a-040
- **H (30):** 28a-003, 28a-004, 28a-005, 28a-006, 28a-007, 28a-008, 28a-009, 28a-010, 28a-011, 28a-014, 28a-015, 28a-016, 28a-017, 28a-018, 28a-019, 28a-020, 28a-021, 28a-023, 28a-024, 28a-025, 28a-026, 28a-027, 28a-029, 28a-031, 28a-037, 28a-039, 28a-053, 28a-054, 28a-057, req-platform-003
- **F (51):** 15a-001, 15a-020, 15a-024, 15a-030, 15a-031, 15a-032, 15a-048, 16a-011, 16a-051, 18a-001, 18a-002, 18a-003, 18a-004, 18a-005, 18a-006, 18a-008, 18a-009, 18a-012, 18a-013, 18a-020, 18a-021, 18a-022, 18a-028, 18a-029, 18a-030, 18a-033, 18a-034, 18a-036, 18a-037, 18a-047, 18a-048, 19a-011, 19a-013, 19a-024, 19a-030, 19a-031, 19a-032, 19a-033, 19a-061, 19a-069, 20a-001, 20a-002, 20a-011, 20a-012, 40a-017, 40a-018, 40b-051, 41a-011, 41a-012, 41a-013, 41a-015
- **G2b (30):** 45a-034, 45b-024, 45b-031, 45b-032, 45b-033, 45b-038, 45b-051, 46a-001, 46a-002, 46a-003, 46a-004, 46a-006, 46a-007, 46a-008, 46a-009, 46a-010, 46a-011, 46a-014, 46a-015, 46a-016, 46a-018, 46a-027, 46a-029, 46a-039, 46a-042, 46a-058, 46a-059, 48a-025, 48a-043, 48a-044
- **G2c (15):** 20a-096, 45b-078, 46b-001, 46b-002, 46b-004, 46b-007, 46b-009, 46b-010, 46b-011, 46b-012, 46b-013, 46b-014, 46b-015, 46b-016, 46b-017
- **Q (60):** 01a-014, 01a-029, 01a-035, 02a-014, 03a-033, 04a-008, 05a-001, 05a-022, 05a-023, 06a-002, 06a-003, 06a-004, 06a-005, 06a-014, 07a-007, 07a-020, 08a-001, 08a-019, 09a-001, 10a-002, 10a-013, 10a-014, 13a-014, 16a-001, 34a-023, 34a-055, 36a-037, 36a-038, 36a-039, 36a-040, 36a-047, 36a-048, 36a-069, 42a-001, 49a-024, 49a-025, 49a-033, 49a-035, 49a-036, 49a-037, 49a-038, 49a-039, 49a-045, 49a-049, 49b-025, 49b-027, 49b-028, 49ba-012, 49ba-016, 49ba-019, 49ba-025, 49ba-026, 51a-043, 52a-003, 52a-023, 52a-024, 52a-028, 52a-053, 52a-064, req-platform-008

### Hand-offs between lanes

Each of these lanes waits for an earlier lane:

- **J** calls M's new shared cookie jar from `settings_ui/advanced.rs` (Clear cookies).
- **D** registers into `library::mount::on_download` (A) and the entry backends (C).
- **H** replaces the `ui::open("tracking_sheet")` calls in `entry_core` (C) and gives the reader a track sink (L).
- **F** registers installed sources into E's registry.
- **K** adds the Browse reselect line in `src/main.rs` (O).
- **N** fixes chapter number precision in `kuro-domain`; E also fixes the same rule in `src/local_flow/sync.rs`.

## 5. What could not be checked

- **No test was run and no window was opened** (cargo is off limits on this machine). Every DONE assumes the named tests pass.
- **Nothing was looked at on screen.** All design and theme verdicts come from reading code. That includes sizes, focus and how pages look in the Tokyo Night theme. The `card`, `pill`, `accent` and `chip` styles were checked against stock GTK 4.24 and the Tokyonight-Dark theme files only.
- **The Library going stale (gap 11)** was found by following callers. It was not seen.
- **Refreshing a local manga** probably reports "source not installed", because the local source is not in the source list. Not confirmed.
- **The shard audits disagree in one place:** `p0-10.md` says every global search opener parks a request first. I checked `src/library/mount.rs:168` and it does not, so the other shards are right.
- **Some DONE verdicts are generous:** library update rows that run but reach no source; tasks marked DONE with a decision written only in a code comment, not in `docs/drop-decisions.md`.
- **The source id formula** still has an open blocker: one observed Google Drive id is not reproduced (`crates/kuro-source/tests/fixtures/id/vectors.json`, see T-20a-011).
- **Not read line by line:** the Library sheet's Filter and Display rows, `browse/lang_pages.rs` and `source_prefs_screen` rows, every entry-page dialog, and the reader and player sheets.
- **Only P0 was audited.** P1 to P3 were not, so the README's overall figures for all 3,800 tasks are neither confirmed nor corrected beyond what is said in section 1.
