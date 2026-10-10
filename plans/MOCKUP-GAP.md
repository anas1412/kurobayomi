# Kurobayomi: mockup vs plan gap analysis

Synthesis document. It compares the UI-only mockup in `/home/blackbox/kurobayomi/src/*.rs` and `/home/blackbox/kurobayomi/spec/*.json` with what the area plans in `/home/blackbox/kurobayomi/plans/` require. Every screen the mockup defines is listed in section 5, with its mockup status, the required behaviour, the owning areas and the first task IDs.

Labels: **OBSERVED** = what the mockup code or the Aniyomi Kotlin does (repo-relative paths; Kotlin paths are relative to `/home/blackbox/aniyomi-src`). **Proposal:** = what Kurobayomi should do. Task IDs (`T-NN?-NNN`) are rows in [TASKS.csv](TASKS.csv). Each area's overview is the `00-overview.md` inside its directory, for example [43-reader-core](43-reader-core/00-overview.md).

Inputs read for this document: every `plans/NN-*/00-overview.md` (56 directories, including `47b` and `49b`), the mockup sources `src/main.rs`, `src/ui.rs`, `src/screens.rs`, `src/entry.rs`, `src/settings.rs`, `src/player.rs`, `src/spec.rs`, `src/clamp.rs`, `spec/SCHEMA.md`, `spec/app.json`, `spec/entries.json`, `spec/reader_player.json`, `spec/settings.json`, `plans/RISKS-AND-DECISIONS.md`, `plans/TEST-STRATEGY.md`, `plans/addenda/requirements-cross-platform.md`, the `p1-ui-parity.md` and `p2-ui-parity.md` files of each area, and `plans/TASKS.csv` (3,801 rows, exported by `plans/_tools/tasks.py export`, `errors=0`). The Kotlin references were checked against the source tree for the files named in sections 4 and 5.

---

## 0. Status legend (used in the tables)

| Code | Meaning |
|---|---|
| **LIT** | Hand-built in Rust. Content is fixed literals or generated sample rows (`Title 1`, `Title i`, fixed MB sizes). No data model, no action effect. |
| **PH** | Hand-built in Rust with invented placeholder names or text that do not come from any source (`Source A`, `Store A`, `Repo A`, a made-up job list). |
| **GEN** | Not hand-built. Rendered by `ui::generic` (`src/ui.rs:506`) from `spec/*.json`. Labels are real text, but every control is created by `ui::item_row` (`src/ui.rs:221`), which builds the widget and discards its state. Nothing is stored or read. |
| **NOSPEC** | The Rust code has a branch for the id, but the id is not present in any spec file, or the nav id has no spec screen. |

---

## 1. Summary

- **Spec screens: 67.** `spec/app.json` 13, `spec/entries.json` 17, `spec/reader_player.json` 2, `spec/settings.json` 35. The count is the same as the one in the task brief.
- **Hand-built or special-cased in Rust: 41.** Generic spec render: 26 (the list in section 5 says which).
- **Navigation: 5 tabs** (`library`, `updates`, `history`, `browse`, `more`, from `spec/app.json` `nav`). The `library` nav id has no spec screen (`spec/app.json` has `anime_library` and `manga_library` instead). `Spec::screen` returns an empty screen for unknown ids (`src/ui.rs:16-19`).
- **Mockup code size:** 2,746 lines of Rust in `src/` (`main.rs` 237, `ui.rs` 525, `screens.rs` 545, `entry.rs` 585, `settings.rs` 264, `player.rs` 337, `spec.rs` 191, `clamp.rs` 62). Spec JSON: 8,503 lines.
- **Dependencies in `Cargo.toml`:** `gtk` (`gtk4` 0.11.5, feature `v4_16`), `serde` 1.0.229, `serde_json` 1.0.151. No database, no HTTP, no async runtime, no image decoder, no mpv binding, no i18n. Every plan crate in section 3 is missing.
- **Persistence:** none. The spec is compiled in with `include_str!` (`src/spec.rs:121-128`) and loaded once per thread into a `thread_local!` (`src/ui.rs:10-14`). Every toggle, slider and sort in GEN and LIT screens is inert.
- **Plan size:** 3,801 tasks. P0 1,196, P1 1,793, P2 503, P3 309. Effort: S 2,448, M 1,234, L 118, XL 1.
- **Priority state in `TASKS.csv` does not match `RISKS-AND-DECISIONS.md` section 3.** Four corrections recorded there are not applied (section 10.1).
- **Largest gap:** every P0 screen in section 5 needs the data layer (area:03, area:04, area:05, area:07, area:08) and the source or engine layer (area:15, area:16, area:43, area:45) before the mockup can show real content. The mockup cannot be "patched" into a working app; section 7 lists the refactors that must come first.

---

## 2. What the mockup actually implements

### 2.1 Architecture as it exists

- **Startup.** `main()` creates a `gtk::Application` with id `io.github.anas1412.Kurobayomi` (`src/main.rs:9`). `build_ui` makes a 1180 x 780 window (`src/main.rs:17-19`). The window content is `[sidebar | content nav stack]` plus a bottom `gtk::StackSwitcher` shown when the window is narrower than 760 px (`src/main.rs:43-62`).
- **Navigation.** A single `gtk::Stack` holds the root tabs (`tabs()`, `src/main.rs:88-96`) and pushed pages (`ui::push`, `src/ui.rs:34-38`). Pages are addressed by string id: `ui::open(id)` (`src/ui.rs:88`), `ui::resolve(id, label)` (`src/ui.rs:437`). Pushed pages are removed 400 ms after pop (`src/ui.rs:40-47`).
- **Back.** Escape or Alt+Left pops one page (`src/main.rs:73-83`). There is no system back action, no back stack model and no navigate-up rule.
- **Spec-driven content.** `ui::screen(id)` returns a cloned `Screen` (`src/ui.rs:16-19`). `screens::build(id)` (`src/screens.rs:35-54`) dispatches in this order: the reader and player (`screens.rs:37-38`), the two entry details (`screens.rs:39-40`), then `screens::body(id)`, then `ui::generic`. `screens::body` (`src/screens.rs:17-32`) sends the anime and manga library, updates, history, more, download queue, categories, stats, storage and the three info screens to hand-built functions, and everything else to `entry::body` (`src/entry.rs:8-22`), which sends to `settings::body` (`src/settings.rs:7-23`) for the rest.
- **Dev hooks.** `KUROBAYOMI_OPEN`, `KUROBAYOMI_SHEET`, `KUROBAYOMI_SHOT`, `KUROBAYOMI_SMOKE`, `KUROBAYOMI_SIZE`, `KUROBAYOMI_RAIL` (`src/main.rs:24-27`, `:61`, `:93-134`, `:64-68`). These are development tools. They are not Aniyomi features. Keep them behind a debug build flag in the rewrite (Proposal).

### 2.2 Invented or placeholder data (must be removed, not patched)

Each row is data the mockup shows that does not exist in Aniyomi or in any plan.

| Location | Content | Plan that replaces it |
|---|---|---|
| `src/screens.rs:56` | `pub const SAMPLE: usize = 18;` drives every library grid and list (`screens.rs:181`) | [37-ui-library](37-ui-library/00-overview.md), T-37a-061 |
| `src/screens.rs:58-59` | `title_n(i)` returns `"Title {i}"`. Used by library, updates, history, storage, entry screens and tracker search (`src/entry.rs:68,464,486,509,541,551`) | T-05a-026, T-08a-033 |
| `src/screens.rs:37-38` | Reader and player always open `"Title 1"`, `"Chapter 1"` and `"Title 1"`, `"Episode 1"` | T-43a-077, T-45b-075 |
| `src/screens.rs:321` | History row opens `"Episode 12"` or `"Chapter 39"` | T-06a-080, T-42a-021 |
| `src/screens.rs:359` | Download queue rows `("Source A", 3)`, `("Source B", 2)` | T-25a-033 |
| `src/screens.rs:484` | Storage rows show `format!("● {} MB · {n}", 900 / i)` | T-42a-052, T-13a-016 |
| `src/entry.rs:103` | Entry details cover source row `"Source A (EN)"` | T-15a-042, T-41a-005 |
| `src/entry.rs:286` | Browse sources sections `"Last used"`, `"Pinned"`, `"English"`, `"Other"` with `"Source A"` to `"Source D"` | T-41a-001, T-40a-001 |
| `src/entry.rs:369` | Migrate sources `("Source A", 12), ("Source B", 4), ("Source C", 1)` | T-41a-036, T-40a-012 |
| `src/entry.rs:444` | Global search source names `"Source A"` to `"Source C"` | T-41a-027 |
| `src/settings.rs:12` | Custom buttons `"Skip opening"`, `"Toggle shader"` | T-48a-045 |
| `src/settings.rs:13` | Anime extension stores `"Store A"`, `"Store B"` | T-47b-010, T-19a-048 |
| `src/settings.rs:14` | Manga extension repos `"Repo A"` | T-47b-023 |
| `src/settings.rs:17` | Worker info text: `Enqueued`, `Id: 1a2b`, `Tags: LibraryUpdate`, `State: SUCCEEDED` | T-26a-110, T-47b-043 |
| `src/settings.rs:18` | Backup schema text: a hand-written 4-line `message Backup` block | T-21a-035, T-47b-045 |
| `src/settings.rs:253` | Licence screen text is a 3-line Apache header stub | T-47a-096, T-47a-097 |
| `src/ui.rs:177-182` | Cover art is a grey frame with an icon (`ponytail: grey frame stands in for cover art`) | [34-images-covers-cache](34-images-covers-cache/00-overview.md), T-34a-037, T-34a-038 |
| `src/player.rs:159` | Player surface is a placeholder (`ponytail: placeholder surface`) | [45-player-core](45-player-core/00-overview.md), T-45b-075 |
| `src/entry.rs:580` | WebView screen has no engine (`ponytail: no web engine linked`) | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md), T-36a-049, [12-network-stack](12-network-stack/00-overview.md) |

### 2.3 Hard-coded numbers in the shell

- Sidebar badges: `"updates" => "8"`, `"browse" => "1"` (`src/main.rs:175`). The area plans cite `src/ui.rs:175` for this line in [37-ui-library](37-ui-library/00-overview.md) and in [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md). The correct file is `src/main.rs:175`. Replace with the count queries of [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) and [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) (T-42b-039 owns the shared toast; the badge source is area:08 and area:35 counts).
- Sidebar rail breakpoint: `win.default_width() >= 760` (`src/main.rs:45`). The plans use the breakpoints in each area's `p1-ui-parity.md`; there is no shared breakpoint constant yet (Proposal: one constant in `kuro-core` or in the GUI crate root, owned by area:36).
- Reader: `let pages = 20.0;` (`src/player.rs:52`). A fixed page count.
- Download and storage: `900 / i` MB (`src/screens.rs:484`).

### 2.4 Mockup features that work and should be kept

- The sidebar plus bottom-bar responsive switch (`src/main.rs:43-62`, `:70-81`). Area:36 keeps the same shape.
- The icon-only rail toggle, badge dots and `Collapse sidebar` tooltip (`src/main.rs:166-199`). Keep the behaviour; the plan wants it (area:36, T-36a-002, T-36a-004).
- Generic spec rendering of groups, empty states and tab chips (`src/ui.rs:506-523`). Keep it as the fallback for read-only static text pages. Do not use it for pages with live state.
- The spec-consistency unit test (`src/spec.rs:131-191`): every spec link target exists and no item type is unknown. Keep it and extend it to the registry check of area:02 (T-02a-030, `every_pref_exists_in_registry`).
- The `ui::with_spec` / `Spec::load` split and the JSON schema in `spec/SCHEMA.md`. The schema is the seed for the typed page builders of area:36.

---

## 3. What the plan requires

### 3.1 Scope and sizes

| Area directory | Tasks | P0 | P1 | P2 | P3 |
|---|---|---|---|---|---|
| 01-foundation-app-bootstrap | 48 | 21 | 11 | 7 | 9 |
| 02-preferences-system | 40 | 17 | 13 | 7 | 3 |
| 03-db-manga-schema-migrations | 39 | 19 | 12 | 5 | 3 |
| 04-db-anime-schema-migrations | 36 | 14 | 11 | 8 | 3 |
| 05-data-repos-entries-items | 32 | 20 | 9 | 1 | 2 |
| 06-data-repos-history-track-source-misc | 41 | 17 | 17 | 3 | 4 |
| 07-domain-entries-items | 44 | 21 | 14 | 5 | 4 |
| 08-domain-library-category-history-updates | 44 | 20 | 16 | 6 | 2 |
| 09-domain-source-track-download-release-misc | 61 | 11 | 30 | 14 | 6 |
| 10-app-domain-layer | 37 | 17 | 15 | 4 | 1 |
| 11-app-migrations | 26 | 3 | 6 | 11 | 6 |
| 12-network-stack | 44 | 16 | 17 | 8 | 3 |
| 13-core-util-storage-archive-torrent | 58 | 19 | 22 | 9 | 8 |
| 14-source-local-and-metadata | 58 | 25 | 23 | 8 | 2 |
| 15-source-api-manga | 51 | 34 | 11 | 3 | 3 |
| 16-source-api-anime | 56 | 31 | 15 | 5 | 5 |
| 17-source-app-layer | 50 | 33 | 10 | 3 | 4 |
| 18-extensions-manga | 54 | 27 | 16 | 7 | 4 |
| 19-extensions-anime | 75 | 19 | 33 | 9 | 14 |
| 20-extension-strategy-redesign | 28 | 14 | 10 | 2 | 2 |
| 21-backup-format-and-create | 44 | 0 | 40 | 2 | 2 |
| 22-backup-restore | 53 | 0 | 40 | 9 | 4 |
| 23-downloads-manga | 45 | 23 | 16 | 4 | 2 |
| 24-downloads-anime | 74 | 0 | 55 | 14 | 5 |
| 25-downloads-shared | 35 | 1 | 27 | 6 | 1 |
| 26-library-update-jobs | 104 | 39 | 43 | 15 | 7 |
| 27-tracking-core | 123 | 0 | 112 | 8 | 3 |
| 28-tracker-anilist | 58 | 35 | 16 | 6 | 1 |
| 29-tracker-myanimelist | 59 | 0 | 48 | 8 | 3 |
| 30-tracker-kitsu | 35 | 0 | 28 | 5 | 2 |
| 31-tracker-shikimori-bangumi | 49 | 0 | 45 | 3 | 1 |
| 32-tracker-mangaupdates-simkl | 55 | 0 | 50 | 4 | 1 |
| 33-tracker-enhanced-kavita-komga-suwayomi-jellyfin | 81 | 0 | 70 | 7 | 4 |
| 34-images-covers-cache | 71 | 36 | 23 | 6 | 6 |
| 35-data-misc-updater-notification-export | 82 | 1 | 46 | 22 | 13 |
| 36-ui-shell-navigation-security-webview | 69 | 23 | 37 | 4 | 5 |
| 37-ui-library | 63 | 36 | 21 | 4 | 2 |
| 38-ui-entry-details-manga | 105 | 38 | 48 | 13 | 6 |
| 39-ui-entry-details-anime | 139 | 47 | 69 | 15 | 8 |
| 40-ui-browse-manga | 111 | 39 | 55 | 8 | 9 |
| 41-ui-browse-anime | 110 | 44 | 53 | 5 | 8 |
| 42-ui-updates-history-stats-storage-upcoming | 108 | 18 | 65 | 17 | 8 |
| 43-reader-core | 153 | 86 | 49 | 9 | 9 |
| 44-reader-viewers | 85 | 51 | 24 | 6 | 4 |
| 45-player-core | 164 | 73 | 60 | 20 | 11 |
| 46-player-controls-ui | 96 | 39 | 37 | 15 | 5 |
| 47-settings-screens-general | 168 | 36 | 75 | 44 | 13 |
| 47b-settings-widgets-search-more | 49 | 32 | 10 | 5 | 2 |
| 48-settings-player-and-scripts | 58 | 20 | 25 | 7 | 6 |
| 49-theme-and-design-system | 100 | 32 | 44 | 17 | 7 |
| 49b-android-res-values-drawables | 50 | 13 | 19 | 10 | 8 |
| 50-android-widgets | 35 | 0 | 3 | 24 | 8 |
| 51-i18n | 43 | 9 | 19 | 9 | 6 |
| 52-util-app | 66 | 15 | 32 | 5 | 14 |
| 53-platform-manifest-intents-workers | 77 | 5 | 43 | 12 | 17 |
| 54-build-release-distribution-docs | 51 | 3 | 29 | 10 | 9 |
| addenda (requirements-cross-platform) | 11 | 4 | 6 | 0 | 1 |
| **Total** | **3,801** | **1,196** | **1,793** | **503** | **309** |

Areas with **zero P0 tasks**: 21, 22, 24, 27, 29, 30, 31, 32, 33, 50. Area 24 (anime downloads) and area 27 (tracking core) are both P1 entirely. That is consistent with the P0 definition in the brief, but it means the anime daily-use path needs no downloads and no tracking. Area 21 and 22 (backup) are also P1. A user who wants backup restore on day one must promote them (section 10.2).

### 3.2 Cross-platform requirement

[requirements-cross-platform.md](addenda/requirements-cross-platform.md) requires that all OS-specific code sits in one crate named `kuro-platform` (rule 1). Every mockup change below must go through the platform traits in that addendum (`Paths`, `SecretStore`, `Notifier`, `MediaControls`, `WebViewHost`, `Opener`, `IdleInhibitor`, `SingleInstance`). The current mockup calls GTK directly and has no `kuro-platform` crate. The `Cargo.toml` workspace is also not yet a workspace (section 7, R-01).

### 3.3 Workspace layout the plans depend on

Adopted layout: [01-foundation-app-bootstrap/p1-rust-design.md](01-foundation-app-bootstrap/p1-rust-design.md) section 1. The GUI package stays at the root (`src/`), and the app crates are `crates/kuro-core`, `crates/kuro-platform`, `crates/kuro-testkit`, `crates/kuro-db` and the crates created by other areas (`kuro-data`, `kuro-domain`, `kuro-source`, `kuro-source-local`, `kuro-i18n`). [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) decision D-2 confirms the workspace layout over the flat `src/updates/` layout that its own part 2 used.

---

## 4. App shell, navigation and chrome

| Element | Mockup (OBSERVED) | Plan (Proposal unless stated) | Owner | First task IDs |
|---|---|---|---|---|
| Root tabs | 5 nav entries from `spec/app.json` `nav`: `library` (icon and title from spec), `updates`, `history`, `browse`, `more` (`src/main.rs:88-96`) | Aniyomi declares separate `AnimeLib` and `Library` destinations plus `Updates`, `History`, `Browse`, `More` (`app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt:338-345`). The mockup merges anime and manga into one `library` tab with a kind switch (`src/screens.rs:141-152`). Decide and document in area:36. | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | T-36a-002, T-36a-004 |
| Tab host | `gtk::Stack` with crossfade (`src/main.rs:88-96`) | Home tab host and start screen (T-36a-004) | area:36 | T-36a-004 |
| Tabbed screen header (search, actions per tab) | Each page has its own header with `end` actions from `Built.end` (`src/screens.rs:12-16`) | Tabbed screen with per-page search and actions | area:36 | T-36a-040 |
| Empty state | `ui::status` (`src/ui.rs:166-176`) | Empty screen component | area:36 | T-36a-048 |
| Back | Escape, Alt+Left pop one page (`src/main.rs:73-83`) | Root navigator and screen transition (T-36a-002); back rules per page in each area | area:36 | T-36a-002, T-36a-003 |
| Sidebar | GNOME-style list, badges from hard-coded `8` and `1` (`src/main.rs:153-199`) | Badge counts from queries; sidebar reuses the nav model | area:36, area:42 | T-42a-007, T-36a-004 |
| Narrow bottom bar | `gtk::StackSwitcher` below 760 px, hidden inside reader and player (`src/main.rs:52-62`, `:43-50`) | Same breakpoint model; area:36 owns the breakpoint constant | area:36 | T-36a-004 |
| Immersive reader and player | Sidebar and bottom bar hidden while `screen:reader` or `screen:player` is visible (`src/main.rs:43-50`) | Reader root layout (T-53a-057); player surface (T-45b-075) | area:43, area:45 | T-43a-050, T-45b-075 |
| Onboarding | `info_screen("onboarding")` static text (`src/screens.rs:493-535`) | Onboarding gate and four-step wizard | area:36 | T-36a-024 |
| Crash reporter | `info_screen("crash")` static text | Panic hook, crash file and reporter process (T-01a-006, T-01a-007) | area:01 | T-01a-006, T-01a-007 |
| New update prompt | `info_screen("new_update")` static text | New update prompt page | area:36 | T-36a-023 (P1) |
| App lock | Not present | App lock state machine (T-01a-032, P1); passphrase per ADR-09 (RISKS section 1). Open question U-6 | area:36 | T-36a-032, T-01a-032 |
| WebView | `entry::webview()` static (`src/entry.rs:572`) | WebKitGTK behind `WebViewHost` (ADR-04) | area:36, area:12 | T-36a-049, T-12a-019 |
| Keyboard | Escape and Alt+Left only | Keyboard map per area (for example T-40a-035) | each area | T-40a-035 |
| Toast | Not present. The mockup has no toast component. | One shared toast, queued, 4 s (`42` D-16; T-42b-039) | area:42 | T-42b-039 |
| Dialogs and sheets | Static rendering from spec (`src/ui.rs:377-431`); radio choices use `gtk::DropDown` in one case | Dialogs with state machines per area; single-choice settings use a dialog with radio rows (area:47b decision D4, gap G-01) | area:47b | T-47ba-009 |

---

## 5. Screen-by-screen gap table (all 67 spec screens)

Columns: **Status** (section 0); **Mockup code** (file:line); **Required behaviour** (what the plan says the screen does, short form; the owning area doc has the full spec); **Owner** (area directories); **First tasks** (earliest P0 tasks in `TASKS.csv` for the screen, then P1 where there is no P0).

### 5.1 `spec/app.json` (13 screens)

| # | Screen id | Status | Mockup code | Required behaviour | Owner | First tasks |
|---|---|---|---|---|---|---|
| 1 | `home` | GEN, NOSPEC (no link in any spec file) | `screens::build` fallback, `src/screens.rs:35-54` | Aniyomi has no `home` screen id. Home is the tab host. Remove from spec when the tab host lands. | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | T-36a-004 |
| 2 | `anime_library` | LIT | `screens::library(id, true)` `src/screens.rs:154-276`, 18 sample cards (`SAMPLE`, `screens.rs:181`), sample badges, `title_n(7)` open (`screens.rs:269`) | Anime library grid and list, categories as tabs, sort and filter sheets, badges (unseen, started, downloaded), selection with bulk actions, search, FAB. Data from the anime library query. | [37-ui-library](37-ui-library/00-overview.md) (display); [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) for open | T-37a-061, T-08a-013, T-05a-014 |
| 3 | `manga_library` | LIT | `screens::library(id, false)` `src/screens.rs:154-276` | Manga library, same shape as `anime_library`. | [37-ui-library](37-ui-library/00-overview.md) | T-37a-061, T-08a-013, T-05a-026 |
| 4 | `updates` | LIT | `screens::updates()` `src/screens.rs:277-304`, rows from `title_n` with a unit label | Updates feed grouped by date, unseen markers, download button, selection mode, refresh button, last-updated text, three-month and 500-row limits (RISKS D-25). | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-007, T-06a-080, T-42b-036 |
| 5 | `history` | LIT | `screens::history()` `src/screens.rs:305-335`; open row shows `"Episode 12"` / `"Chapter 39"` (`screens.rs:321`) | History feed grouped by day, resume, remove, clear-all with confirmation; anime and manga sub-tabs. Reselect rules (D-20). | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-021, T-06a-080, T-08a-034 |
| 6 | `more` | LIT (structure from spec) | `screens::more()` `src/screens.rs:336-352`; drops the History link (`:344-347`) | 14-row More page with downloads, categories, stats, storage, settings, about, help. Rule for hiding the History row is open (area:47b Q15). | [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md), [47-settings-screens-general](47-settings-screens-general/00-overview.md) | T-47ba-010, T-47b-001 |
| 7 | `download_queue` | PH | `screens::downloads()` `src/screens.rs:354-403`; `Source A` and `Source B` rows (`:359`) | Queue page: source groups, row menu, sort, drag reorder, footer, empty state, keyboard (Alt+Up, Alt+Down, Delete, Space, RISKS section 3). Anime and manga queues share one page. | [25-downloads-shared](25-downloads-shared/00-overview.md), [23-downloads-manga](23-downloads-manga/00-overview.md), [24-downloads-anime](24-downloads-anime/00-overview.md) | T-23a-002, T-23a-012, T-23a-013, T-25a-033 (P1 replacement) |
| 8 | `categories` | LIT | `screens::categories()` `src/screens.rs:404-425` | Category list with reorder, rename, delete, create, hide-from-library flags, default category. Dialogs with name validation. | [37-ui-library](37-ui-library/00-overview.md), [08-domain-library-category-history-updates](08-domain-library-category-history-updates/00-overview.md) | T-08a-033, T-37a-017, T-37a-041 (P1), T-05a-025 (P1 in file; see section 10.1) |
| 9 | `stats` | LIT | `screens::stats()` `src/screens.rs:443-467`, `stat_card` with fixed values (`:426-442`) | Statistics page: totals for entries, chapters, episodes, read duration, downloads. Numbers are live queries. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-043 (P1), T-08a-023 (P2 read duration), T-05a-014 |
| 10 | `storage` | LIT | `screens::storage()` `src/screens.rs:468-492`, fake sizes `900 / i` (`:484`) | Storage page: category select, chart, per-entry usage, delete downloaded. D-21 empty-state text and D-23 population decision. | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md), [13-core-util-storage-archive-torrent](13-core-util-storage-archive-torrent/00-overview.md) | T-13a-016, T-42a-052 (P1) |
| 11 | `onboarding` | LIT (static text) | `screens::info_screen("onboarding")` `src/screens.rs:493-535` | Four-step wizard: theme, storage folder, desktop status (P2), guides and restore entry. | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | T-36a-024, T-36a-025, T-36a-026, T-36a-028 |
| 12 | `new_update` | LIT (static text) | `screens::info_screen("new_update")` `src/screens.rs:497-498` | New update prompt: version, changelog, actions. In-app download dropped (RISKS section 4). | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md), [35-data-misc-updater-notification-export](35-data-misc-updater-notification-export/00-overview.md) | T-36a-023 (P1), T-35a-058 (P1) |
| 13 | `crash` | LIT (static text) | `screens::info_screen("crash")` | Crash reporter window: message, share logs, copy, reveal, restart, quit. | [01-foundation-app-bootstrap](01-foundation-app-bootstrap/00-overview.md) | T-01a-006, T-01a-007 |

### 5.2 `spec/entries.json` (17 screens)

| # | Screen id | Status | Mockup code | Required behaviour | Owner | First tasks |
|---|---|---|---|---|---|---|
| 14 | `manga_entry_details` | LIT | `entry::entry(title, false)` `src/entry.rs:81-243`; reached via `screens::build` (`src/screens.rs:40`) with `Title 1` | Manga details: header, actions, chapter list with missing-chapter separators, selection, download menu, tracking sheet, migrate, duplicate. | [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) | T-38a-010, T-07a-040, T-07a-015 (P1; see RISKS OQ-6) |
| 15 | `anime_entry_details` | LIT | `entry::entry(title, true)` `src/entry.rs:81-243`; `screens.rs:39` | Anime details: header, flags and filters, season list, episode rows, quality dialog, cover background dialog, tracking, airing info. The mockup's anime half is replaced without touching the manga half (`area:39` mitigation). | [39-ui-entry-details-anime](39-ui-entry-details-anime/00-overview.md) | T-39b-063, T-39a-074, T-39b-064 |
| 16 | `tracking_sheet` | LIT | `entry::tracking()` `src/entry.rs:498-538` | Track sheet: one row per tracker, status, score, progress, remove, search, login. | [27-tracking-core](27-tracking-core/00-overview.md) | T-28a-025 (P0, AniList bind), T-27a-069 (P1 mockup wiring) |
| 17 | `tracker_search` | LIT | `entry::tracker_search()` `src/entry.rs:539-571`; sample `title_n(i)` rows | Remote search screen: auto-search, loading, results, empty, error, pre-select, confirm. | [27-tracking-core](27-tracking-core/00-overview.md), [28-tracker-anilist](28-tracker-anilist/00-overview.md) | T-28a-057, T-27b-012 (P1) |
| 18 | `browse` | PH | `entry::browse()` `src/entry.rs:244-282`; `sources()` `:283-310` with invented sections and `Source A` to `Source D` | Browse tab: six sub-pages (Sources, Extensions, Migrate, Global search, Stubs), tab-switch channel, reselect opens global search (T-40a-050). Manga and anime browse share the tab. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-034, T-40a-001, T-41a-001 |
| 19 | `extension_details` | GEN | not hand-built | Extension details: header, source rows, enable and disable all, incognito, repo link, copy debug info. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-021 (P1), T-41a-020 (P1) |
| 20 | `source_preferences` | GEN | not hand-built | Declarative source preference rows (list, multi, switch, text), stored per source. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [15-source-api-manga](15-source-api-manga/00-overview.md) | T-40a-023 (P1), T-41a-023 (P1), T-15a-041 (P1) |
| 21 | `extension_filter` | GEN | not hand-built | Language filter for extensions: one switch per language, search. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-020 (P1) |
| 22 | `sources_filter` | GEN | not hand-built | Source filter page: language switches and per-source checkboxes. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-031 (P1) |
| 23 | `extension_repos` | PH | `entry::body("extension_repos")` routes to `settings::body("settings_manga_extension_repos")` `src/entry.rs:18`; `Repo A` at `src/settings.rs:14` | Manga extension repos: list, add (confirm), delete, empty state. | [18-extensions-manga](18-extensions-manga/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47b-023 (P0) |
| 24 | `migrate_entries` | LIT | `entry::migrate_entries()` `src/entry.rs:482-497` | Migration page: list of library entries from the source being migrated; confirm dialog with flags. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-015 (P1), T-41a-035 (P1) |
| 25 | `migrate_search` | LIT | `entry::global_search(id, true)` `src/entry.rs:432-481` | Migrate search: global search seeded with the entry title, origin-first ordering. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md) | T-40a-014 (P1), T-40b-067 (P1) |
| 26 | `migrate_source_search` | PH | `entry::browse_source(id, true)` `src/entry.rs:380-426` | Per-source migrate search: pager, results, pick. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-039 (P1), T-40a-014 (P1) |
| 27 | `migrate_season_select` | LIT | `entry::seasons()` `src/entry.rs:427-431` | Anime migrate: season picker with fetch-type rules. | [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-040 (P1), T-41a-041 (P1) |
| 28 | `browse_source` | PH | `entry::browse_source(id, false)` `src/entry.rs:380-426` with sample rows | Browse one source: header (display mode, search, filter), pager, listing states (loading, error, empty, stub), navigate-up rule. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-005, T-41b-012, T-40a-002, T-40a-003 |
| 29 | `global_search` | PH | `entry::global_search(id, false)` `src/entry.rs:432-481` with `Source A` to `Source C` at `:444` | Global search: per-source blocks, progress, filter chips, cancellation, error per source, result rows. | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-41a-027 (P1), T-40a-010 (P1), T-37a-022 (P0, global search row) |
| 30 | `webview` | LIT | `entry::webview()` `src/entry.rs:572` | WebView screen: header, back, forward, reload, open in browser, progress, Cloudflare handling. | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md), [12-network-stack](12-network-stack/00-overview.md) | T-36a-049, T-12a-019 (P1) |

### 5.3 `spec/reader_player.json` (2 screens)

| # | Screen id | Status | Mockup code | Required behaviour | Owner | First tasks |
|---|---|---|---|---|---|---|
| 31 | `reader` | LIT | `player::reader("Title 1", "Chapter 1")` `src/player.rs:49-149`; `pages = 20.0` (`src/player.rs:52`); grey cover (`src/ui.rs:177`) | Manga reader: viewer per reading mode (pager LTR/RTL/vertical, webtoon, continuous), chapter loading and progress writes, chrome (top and bottom bars, navigator, page indicator), page actions, settings window, tap zones, brightness and colour filter, e-ink flash. | [43-reader-core](43-reader-core/00-overview.md), [44-reader-viewers](44-reader-viewers/00-overview.md) | T-43a-001, T-43a-077, T-43b-014, T-44a-021, T-44a-023 |
| 32 | `player` | PH | `player::player("Title 1", "Episode 1")` `src/player.rs:150` onward; placeholder surface (`src/player.rs:159`) | Anime player: mpv surface (libmpv, ADR-02), controls, gestures, sheets (subtitle, audio, quality, more), panels, custom buttons, skip intro, progress, screenshot, PiP-equivalent (desktop), MPRIS. | [45-player-core](45-player-core/00-overview.md), [46-player-controls-ui](46-player-controls-ui/00-overview.md) | T-45b-075, T-46b-046, T-45a-025, T-45a-044 |

### 5.4 `spec/settings.json` (35 screens)

| # | Screen id | Status | Mockup code | Required behaviour | Owner | First tasks |
|---|---|---|---|---|---|---|
| 33 | `settings` | LIT (menu is spec-driven) | `settings::main_menu()` `src/settings.rs:26-46` (11 rows from `spec/settings.json` `main`) | Settings root: 11 categories with icons and subtitles, search action. | [47-settings-screens-general](47-settings-screens-general/00-overview.md) | T-47b-001 |
| 34 | `settings_search` | LIT | `settings::search()` `src/settings.rs:47-91`; searches spec titles only | Search all settings: index from the registry (T-02a-037), breadcrumbs `Category > Group`, results open the row. | [47-settings-screens-general](47-settings-screens-general/00-overview.md), [02-preferences-system](02-preferences-system/00-overview.md) | T-47a-006 (P0), T-47a-004 (P1), T-02a-037 (P2) |
| 35 | `settings_appearance` | GEN | not hand-built | Spec rows: Theme mode, App theme, Pure black dark mode; App language, Tablet UI, Start Screen, Navigation Style, Date format, Relative timestamps. Theme and navigation style are owned by area:49 and the UI preference model of area:48 (T-48a-002). | [49-theme-and-design-system](49-theme-and-design-system/00-overview.md), [02-preferences-system](02-preferences-system/00-overview.md) | T-01a-021, T-02a-023, T-48a-002 |
| 36 | `settings_app_language` | LIT | `settings::language()` `src/settings.rs:100-122` | Language picker: catalogue-derived list, native names, restart notice. | [51-i18n](51-i18n/00-overview.md) | T-51a-013 (P1), T-47b-007 (P2) |
| 37 | `settings_library` | GEN | not hand-built | Spec rows: Categories (edit and default category for anime and manga, per-category sort, hide hidden categories); Global update (automatic updates, device restrictions, anime and manga categories, refresh metadata, smart update, unseen count on Updates icon); Season (two update switches); Episode behavior and Chapter behavior (swipe left and right, duplicate rules). | [37-ui-library](37-ui-library/00-overview.md), [26-library-update-jobs](26-library-update-jobs/00-overview.md) | T-37a-063 (P1), T-08a-044 (P1), T-26a-100 (P1) |
| 38 | `settings_reader` | GEN | not hand-built | Spec rows: Reader root (default reading mode, double-tap animation speed, show reading mode, tap-zone overlay, animate page transitions); Display (default rotation, background color, fullscreen, cutout, keep screen on, page number); E-Ink (flash on page change, duration, every, with); Reading (skip read, skip filtered, skip duplicate, always show transition, preserve position); Paged and Long strip blocks; Navigation (volume keys); Actions. Area 43 owns the keys. | [43-reader-core](43-reader-core/00-overview.md), [47-settings-screens-general](47-settings-screens-general/00-overview.md) | T-43b-031, T-47a-034, T-47a-037 |
| 39 | `settings_player` | GEN | not hand-built | Player root: nine rows (Internal player, Gestures, Decoder, Subtitles, Audio, Custom buttons, Script editor, Torrent, Advanced). | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md) | T-48a-010 |
| 40 | `settings_player_internal` | GEN | not hand-built | Spec rows: mark-as-seen point, preserve watch position, orientation; Controls (gesture in panels, loading circle, chapter indicator, remember brightness, remember volume); Hosters (failed, empty); Display (cutout, hide controls, percentage volume, status bar, reduce animation, controls time, panel opacity); Intro skip (enable, auto skip, Netflix style, button timeout, AniSkip rows); Picture-in-Picture block; External player (always use, preference). The PiP block is an Android feature; area:48 drops the Android device gates (T-48a-054). | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md), [46-player-controls-ui](46-player-controls-ui/00-overview.md) | T-46a-042, T-48a-010, T-48a-054 (P3 drop) |
| 41 | `settings_player_gestures` | GEN | not hand-built | Spec rows: Sliders (volume and brightness gestures, swap slider); Seeking (horizontal seek, seekbar, default skip intro length, double-tap skip length, precise seeking); Double tap (left, center, right); Media controls (previous, play/pause, next). The spec text names input.conf key codes 0x10001 to 0x10006 for custom bindings. | [46-player-controls-ui](46-player-controls-ui/00-overview.md) | T-02a-014 (P0), T-46a-042 (P0) |
| 42 | `settings_player_decoder` | GEN | not hand-built | Spec rows: Try hardware decoding, Enable gpu-next, Debanding, Use YUV420P pixel format. | [45-player-core](45-player-core/00-overview.md) | T-46b-009 (P0), T-02a-014 (P0) |
| 43 | `settings_player_subtitle` | GEN | not hand-built | Spec rows: Preferred languages, Whitelist, Blacklist. | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md) | T-48a-029 (P1) |
| 44 | `settings_player_audio` | GEN | not hand-built | Spec rows: Preferred languages, Enable audio pitch correction, Audio channels, Volume boost cap. | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md) | T-48a-031 (P1) |
| 45 | `settings_player_torrents` | GEN | not hand-built | Spec rows: Enable TorrServer, TorrServer port, Torrent trackers, Reset default torrent trackers string, Proxy mode, Proxy url. Torrent playback is P2 (U-5). | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md), [13-core-util-storage-archive-torrent](13-core-util-storage-archive-torrent/00-overview.md) | T-48a-032 (P2), T-13a-043 (P2) |
| 46 | `settings_player_custom_buttons` | PH | `settings::cards(id, ["Skip opening", "Toggle shader"], true)` `src/settings.rs:12, 134-175` | Custom buttons list: create, delete, reorder, favourite, Lua placeholder expansion. | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md), [06-data-repos-history-track-source-misc](06-data-repos-history-track-source-misc/00-overview.md) | T-48a-045 (P1), T-06a-060 (P1) |
| 47 | `settings_player_editor` | LIT | `settings::editor()` `src/settings.rs:176-202` | Script editor: file list, type switch, create, rename, delete. | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md) | T-48a-037 (P1) |
| 48 | `settings_player_code_editor` | LIT | `settings::code_editor()` `src/settings.rs:203-216` | Code editor: load, edit, save, unsaved-changes guard. | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md) | T-48a-038 (P1) |
| 49 | `settings_player_advanced` | GEN | not hand-built | Spec rows: Use user MPV files, Edit MPV configuration file, Edit MPV input file for keyboard mapping. | [48-settings-player-and-scripts](48-settings-player-and-scripts/00-overview.md), [02-preferences-system](02-preferences-system/00-overview.md) | T-48a-035 (P1), T-02a-016 (P1) |
| 50 | `settings_downloads` | GEN | not hand-built | Spec rows: Only on Wi-Fi, chapter download speed limit, save as CBZ, split tall images, maximum downloads; Delete chapters/episodes (after manual read or watch, after reading or watching automatically, allow deleting bookmarked, allow fillermarked, excluded categories); Auto-download (new episodes and chapters, skip duplicates, categories); Download ahead (while watching, while reading); External downloader. | [23-downloads-manga](23-downloads-manga/00-overview.md), [24-downloads-anime](24-downloads-anime/00-overview.md), [25-downloads-shared](25-downloads-shared/00-overview.md) | T-23a-037 (P1), T-24a-054 (P1) |
| 51 | `settings_tracking` | GEN | not hand-built | Spec rows: Update progress after reading or watching, Open track menu on adding to library, next episode airing time, Update progress when marked as read or watched; Trackers (MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, Bangumi); Enhanced trackers (Komga, Kavita, Suwayomi, Jellyfin). | [27-tracking-core](27-tracking-core/00-overview.md), [32-tracker-mangaupdates-simkl](32-tracker-mangaupdates-simkl/00-overview.md), [33-tracker-enhanced-kavita-komga-suwayomi-jellyfin](33-tracker-enhanced-kavita-komga-suwayomi-jellyfin/00-overview.md) | T-32a-064 (P1), T-33a-009 (P1) |
| 52 | `settings_browse` | GEN | not hand-built | Spec rows: Sources (hide anime entries already in library, hide manga entries already in library, anime extension repos, manga extension repos); NSFW (18+) sources (show in sources and extensions lists). | [40-ui-browse-manga](40-ui-browse-manga/00-overview.md), [41-ui-browse-anime](41-ui-browse-anime/00-overview.md) | T-40a-032 (P0) |
| 53 | `settings_anime_extension_stores` | PH | `settings::cards(id, ["Store A", "Store B"], false)` `src/settings.rs:13` | Anime extension stores: list, add store (confirm), delete, reorder, empty state. | [19-extensions-anime](19-extensions-anime/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47b-010 (P0), T-19a-048 (P1) |
| 54 | `settings_manga_extension_repos` | PH | `settings::cards(id, ["Repo A"], false)` `src/settings.rs:14` | Manga extension repos. Same behaviour as row 23. | [18-extensions-manga](18-extensions-manga/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47b-023 (P0) |
| 55 | `settings_data_storage` | GEN | not hand-built | Spec rows: Storage location; Backup and restore (create backup, restore backup, automatic backup frequency); Storage usage (available and total space, storage, clear chapter and episode cache, clear cache on app launch); Export (Library List). The Library List export is a CSV renderer owned by area:35 (T-35a-071, T-35a-073, P2). | [13-core-util-storage-archive-torrent](13-core-util-storage-archive-torrent/00-overview.md), [47-settings-screens-general](47-settings-screens-general/00-overview.md), [35-data-misc-updater-notification-export](35-data-misc-updater-notification-export/00-overview.md) | T-13a-016 (P0), T-21a-026 (P1), T-35a-071 (P2) |
| 56 | `settings_create_backup` | GEN | not hand-built | Spec rows: Library (library entries, chapters and episodes, tracking, history, categories, all read or watched entries); Settings (app settings, extension stores, custom buttons, source settings, include sensitive settings such as tracker login tokens); Extensions; Create button. Rule on sensitive settings is area:21. | [21-backup-format-and-create](21-backup-format-and-create/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-21a-011 (P1), T-22a-034 (P1), T-47b-029 (P1) |
| 57 | `settings_restore_backup` | GEN | not hand-built | Spec rows: Library, Categories, App settings, Extension stores, Custom buttons, Source settings, Extensions; Restore button. | [22-backup-restore](22-backup-restore/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-22a-001 (P1), T-22a-033 (P1), T-47b-031 (P1) |
| 58 | `settings_security` | GEN | not hand-built | Spec rows: Require unlock, Lock when idle, Hide notification content, Secure screen. The Secure screen row is the Android FLAG_SECURE switch; the plan keeps the setting and shows "Not available on Linux" (RISKS section 4). App lock is off by default (U-6). | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md), [01-foundation-app-bootstrap](01-foundation-app-bootstrap/00-overview.md) | T-01a-032 (P1), T-36a-032 (P1), T-36a-033 (P1) |
| 59 | `settings_advanced` | GEN | not hand-built | Spec rows: Share crash logs, Verbose logging, Debug info, Onboarding guide, Manage notifications; Background activity (battery optimisation, "Don't kill my app!"); Data (reindex downloads, clear manga database, clear anime database); Networking (clear cookies, clear WebView data, DNS over HTTPS, default user agent string, reset default user agent); Library (refresh library covers, reset per-series reader settings); Reader (custom hardware bitmap threshold, legacy long-strip decoder, custom display profile); Extensions (installer, revoke trusted unknown extensions); Data Saver (proxy, image rows). The battery rows are dropped (T-47a-073, P3). The Installer row is Android-only. The data saver services are dropped (section 12). | [47-settings-screens-general](47-settings-screens-general/00-overview.md), [12-network-stack](12-network-stack/00-overview.md), [13-core-util-storage-archive-torrent](13-core-util-storage-archive-torrent/00-overview.md) | T-47a-070 (P1, verbose logging), T-47a-073 (P3, drop battery), T-12a-006 (P0, default user agent), T-01a-005 (P0, logging) |
| 60 | `settings_debug_info` | GEN | not hand-built | Spec rows: Worker info, Backup file schema; App info (Version, Build time, profile compilation status, WebView version); Device info (Model, Android version). The Android device rows are replaced with Linux device rows (T-47b-041). | [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47b-040 (P2), T-47b-041 (P2) |
| 61 | `settings_worker_info` | PH | `settings::text_dump` with invented job text `src/settings.rs:17` | Worker info: job-run table (enqueued, running, finished, state, tags). The job-run table is the source. | [26-library-update-jobs](26-library-update-jobs/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47b-043 (P2), T-26a-110 (P2) |
| 62 | `settings_backup_schema` | PH | `settings::text_dump` `src/settings.rs:18` | Backup schema page: monospace, the real `.proto` schema text as implemented (area 21). | [21-backup-format-and-create](21-backup-format-and-create/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47b-045 (P2), T-21a-035 (P3) |
| 63 | `settings_clear_manga_database` | LIT | `settings::clear_db(id)` `src/settings.rs:223-252` | Clear manga database: confirmation, type-to-confirm, progress, result. No UI for reset flags (RISKS section 4). | [47-settings-screens-general](47-settings-screens-general/00-overview.md), [03-db-manga-schema-migrations](03-db-manga-schema-migrations/00-overview.md) | T-47a-075 (P1), T-03a-029 (P1) |
| 64 | `settings_clear_anime_database` | LIT | `settings::clear_db(id)` `src/settings.rs:223-252` | Clear anime database. Same as row 63 for the anime tables. | [47-settings-screens-general](47-settings-screens-general/00-overview.md), [04-db-anime-schema-migrations](04-db-anime-schema-migrations/00-overview.md) | T-47a-075 (P1), T-04a-028 (P0 wiring) |
| 65 | `settings_about` | GEN | not hand-built | Spec rows: Version, Check for updates, What's new, Help translate, Open source licenses, Privacy policy. Gaps: no task in TASKS.csv covers the Privacy policy row or the Help translate row; the changelog dialog is T-36a-021 (P1) and the update check is T-35a-058 (P1). | [47-settings-screens-general](47-settings-screens-general/00-overview.md), [35-data-misc-updater-notification-export](35-data-misc-updater-notification-export/00-overview.md) | T-47a-093 (P1), T-35a-058 (P1), T-36a-021 (P1) |
| 66 | `settings_open_source_licenses` | LIT | `settings::licenses()` `src/settings.rs:253` | Licence list: one row per dependency. The list is generated at build time (T-54a-057). | [47-settings-screens-general](47-settings-screens-general/00-overview.md), [54-build-release-distribution-docs](54-build-release-distribution-docs/00-overview.md) | T-47a-096 (P2), T-54a-057 (P2) |
| 67 | `settings_open_source_library_license` | PH | `settings::text_dump` with a 3-line Apache header `src/settings.rs:21` | Licence detail: full licence text per library. | [47-settings-screens-general](47-settings-screens-general/00-overview.md) | T-47a-097 (P2) |

Notes on section 5:
- Rows 33 to 67 that are GEN have no spec control bound to any store. `ui::item_row` (`src/ui.rs:221-353`) returns a row built from the spec `default` value and throws it away. Every GEN settings row therefore needs the preference binding of area:02 (T-02a-030, P0, "Bind the mockup settings pages to the preference store") before it means anything.
- The `default` values in `spec/settings.json` are copies of Aniyomi's display text, not the registry defaults. Area 02 owns the defaults (`every_pref_exists_in_registry`, section 8).

---

## 6. Mockup surfaces that are not spec screens

| Surface | Mockup (OBSERVED) | Plan | Owner | First tasks |
|---|---|---|---|---|
| Entry grid cover (`cover_grid`) | `src/entry.rs:53-80`, grey frame cards | Cover cache and loader; placeholder on error (area 34, S1 and S2 in its parity doc) | [34-images-covers-cache](34-images-covers-cache/00-overview.md) | T-34a-037, T-34a-038, T-34a-040 |
| Per-entry context and sheets (`ui::show_sheet`) | `src/ui.rs:391-405`, sheets from spec `sheets` | Sheet host with typed sheets | [46-player-controls-ui](46-player-controls-ui/00-overview.md) (player), [38-ui-entry-details-manga](38-ui-entry-details-manga/00-overview.md) (manga) | T-46b-001, T-38a-010 |
| Dialogs (`ui::show_dialog`) | `src/ui.rs:406-431`, dialog buttons with no action | Dialog state machines (typed) | each owner area | T-47ba-009 for settings; T-38b-012 for change-category |
| Action routing by label (`ui::resolve`) | `src/ui.rs:437-468`, matches a label string against a spec id | Typed actions; no label matching | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | T-36a-040 |
| Item rows (`ui::item_row`) | `src/ui.rs:221-353` builds the widgets, no state | Bound rows with a `Pref<T>` handle and veto runner (`T-47ba-009`) | [02-preferences-system](02-preferences-system/00-overview.md), [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-02a-030, T-47ba-009 |
| Toolbar (`ui::toolbar`) | `src/ui.rs:469-488` | Per-tab header actions | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | T-36a-040 |
| Linked tab chips (`ui::linked`) | `src/ui.rs:184-200` | Segmented control | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | T-36a-040 |
| Menu buttons (`ui::menu_button`) | `src/ui.rs:202-220` | Row overflow menus | [25-downloads-shared](25-downloads-shared/00-overview.md) | T-25a-033 |
| Dropdown for single choice | `gtk::DropDown` in settings | Radio dialog (area 47b D4) | [47b-settings-widgets-search-more](47b-settings-widgets-search-more/00-overview.md) | T-47ba-044 |
| Row list widget | `gtk::ListBox` (`ui::boxed_list`, `src/ui.rs:133-139`) | `gtk::ListView` with `SignalListItemFactory` everywhere (area 42 D-15) | [42-ui-updates-history-stats-storage-upcoming](42-ui-updates-history-stats-storage-upcoming/00-overview.md) | T-42a-007 |
| Keyboard | Escape and Alt+Left only (`src/main.rs:73-83`) | Keyboard map per area; defaults listed in each `p1-ui-parity.md` | each area | T-40a-035, T-43b-058, T-25a-026 |
| Title and subtitle text | `ui::heading`, `ui::dim` (`src/ui.rs:125-132`) | Style classes per area 49 | [49-theme-and-design-system](49-theme-and-design-system/00-overview.md) | T-49ba-031 |
| Strings | About 756 string literals across `src/*.rs` (counts in [51-i18n](51-i18n/00-overview.md): `entry.rs` 270, `screens.rs` 204, `settings.rs` 107, `player.rs` 101, `ui.rs` 39, `main.rs` 30, `clamp.rs` 1, `spec.rs` 4) | Catalogue with keys, route all literals through it; do this after the UI areas settle (51 R2) | [51-i18n](51-i18n/00-overview.md) | T-51a-031 (P1 L) |
| Icons | `gtk::Image::from_icon_name` throughout (freedesktop names) | Bundled symbolic SVGs with a lookup function (addendum rule 6) | [49b-android-res-values-drawables](49b-android-res-values-drawables/00-overview.md) | T-49ba-038 (P1) |
| Clamp widget | `src/clamp.rs` (62 lines), `Clamp` subclass with `measure` and `size_allocate` | Keep; it is a GTK subclass with no data | [36-ui-shell-navigation-security-webview](36-ui-shell-navigation-security-webview/00-overview.md) | none (keep) |

---

## 7. Structural refactors required before real data flows

Each item lists the change, the reason in the plans, the files it touches, and the tasks that perform it. Numbering is for this document only.

- **R-01 Turn the package into a Cargo workspace.** `Cargo.toml` has a single package (`kurobayomi`) and no `[workspace]`. Add the `crates/` members from [01-foundation-app-bootstrap/p1-rust-design.md](01-foundation-app-bootstrap/p1-rust-design.md) section 1 (`kuro-core`, `kuro-platform`, `kuro-testkit`). Tasks: T-01a-001 (P0). Risk: low. Everything below depends on it.
- **R-02 Move the GUI crate's startup into `kuro-core`.** `main()` (`src/main.rs:9-15`) builds the GTK app directly. Area 01 moves logging, crash hook, settings load, the tokio runtime and `AppContext` into `kuro-core::bootstrap::run_startup()`. Tasks: T-01a-003 (AppContext, P0 L), T-01a-004 (startup order, P0), T-01a-005 (logging, P0), T-01a-006 (panic hook, P0).
- **R-03 Replace the compile-time spec with a runtime model for real data.** `Spec::load` uses `include_str!` (`src/spec.rs:121-128`) and a `thread_local!` (`src/ui.rs:10-14`). Keep the spec for static text (SCHEMA.md says spec strings are English text). Move every state-bearing control into typed page builders in their area module. Tasks: T-02a-030 (P0, bind settings pages), T-47b-058 (P0, mockup wiring for in-scope settings ids), T-47ba-002 (P0, settings page registry and spec-to-model conversion).
- **R-04 Replace string-id navigation with typed routes.** `ui::open(id)`, `ui::resolve(id, label)`, `screens::build(id)` dispatch on strings (`src/ui.rs:88`, `:437`; `src/screens.rs:35`). The area 36 decision A-4 says typed page builders replace string-id navigation (`Adopt. Spec JSON stays for static text.`). Tasks: T-36a-002 (root navigator, P0), T-40a-034 (browse route table, P0), T-41a-005 (typed navigation, P0), T-36a-040 (tabbed screen, P0).
- **R-05 Remove the fake `title_n` and `SAMPLE` data.** `src/screens.rs:56-59` and every caller in section 2.2. Replace with the repository queries. Tasks: T-03a-034 (P0, wire the database and replace fabricated data), T-05a-026 (P0 L, wire mockup screens to repositories), T-06a-080 (P0, history and updates), T-06a-081 (P0, browse list), T-15a-051 (P0, remove invented sample rows from browse).
- **R-06 Make `entry` an id-keyed page.** `entry::entry(title, anime)` (`src/entry.rs:81`) takes a display title, not an entry id, and is called with `"Title 1"` (`src/screens.rs:39-40`). The page must take the entry key (source id and url, or the database id). Tasks: T-39b-063 (P0 L, model-driven anime page), T-38a-010 (P0, row open model), T-07a-040 (P0, bind entry screens to domain flags).
- **R-07 Split `src/entry.rs` by area.** The 585-line file holds browse (manga and anime), migrate, tracking and webview. Area 39 says `T-39b-063` changes only the anime branch. Area 38 owns the manga branch. Proposal: `src/browse/`, `src/details/`, `src/tracking/`, `src/webview.rs`, each owned by one area. Tasks: T-40a-034 (replace browse), T-38a-010 (manga details), T-39b-063 (anime details), T-27a-069 (track sheet wiring), T-36a-049 (webview).
- **R-08 Split `src/player.rs` into `reader` and `player` modules.** `src/player.rs` holds both. Area 43 specifies `src/reader/` and area 45 specifies `src/player/`. Proposal: `src/player.rs` is removed; `reader::open(...)` and `player::open(...)` take typed keys. Tasks: T-43a-077 (replace placeholder reader), T-45b-075 (player surface and overlay bindings), T-46b-046 (wire the mockup player screen to the sheet host).
- **R-09 Move `src/settings.rs` to a directory.** Area 47b decision D-1 keeps `src/settings.rs` plus children (the children under `src/settings/`). Area 47 uses `src/settings/mod.rs`; area 47b D-1 chose the file form. Use the file form. Tasks: T-47ba-002 (settings page registry), T-47ba-005 (scaffold), T-47ba-009 (preference binding glue).
- **R-10 Replace `screens::body`, `entry::body` and `settings::body` chains with one registry.** Three nested `match` functions decide which builder a screen id uses, and falling through returns generic. Proposal: one `routes` table (typed enum to builder) in the GUI crate root; generic is an explicit route, not a fallback. Tasks: T-40a-034 (browse route table), T-47b-001 (settings router, P0), T-47ba-002 (page registry).
- **R-11 Make `item_row` stateful.** `src/ui.rs:221-353` builds a widget per spec item and does not keep it. Replace with `Pref<T>` binding (`PrefBinding`, T-01a-010 P0) and a veto runner (T-47ba-009). Spec `Item` variants need `pref`, `values` and range fields that the spec does not have (section 8).
- **R-12 Add the value and range model to `spec::Item`.** `Item::Tristate` has only `title` (`src/spec.rs:11-22`), `Item::Sort` has `options` but no default. Area 02 risk R8 says the same. Tasks: T-02a-036 (P1 in file, promote to P0 per RISKS section 3; see section 10.1), T-02a-030.
- **R-13 Add an image pipeline.** The cover is a grey frame (`src/ui.rs:177-182`). Area 34 requires an in-memory byte source, page-list and page-image caches with size limits, and a read-through reader API. Tasks: T-34a-024 (P0), T-34a-037 (P0), T-34a-038 (P0 L), T-34a-040 (P0).
- **R-14 Add an async runtime and the bridge to GTK.** No tokio anywhere in the mockup. Area 01 defines one multi-thread runtime started in `main`, and `async-channel` as the UI mailbox (`01` p1-rust-design section 2). Tasks: T-01a-004 (startup), T-01a-009 (P0, tokio runtime and GTK main-loop bridge), T-01a-010 (`PrefBinding`, P0).
- **R-15 Add the database and migrations.** Nothing in the mockup reads or writes data. Area 03 (manga) and 04 (anime) define the schema and migrations; area 05 defines repositories. Tasks: T-03a-004 (migration runner, P0), T-03a-034 (wire into mockup, P0), T-04a-028 (wire anime DB, P0), T-05a-026 (repositories, P0 L).
- **R-16 Add the preference store.** Area 02 defines the typed store, backend flushing and secrets. Tasks: T-02a-001 (store API, P0 L), T-02a-023 (UI preferences, P0), T-02a-030 (bind mockup pages, P0), T-02a-038 (shutdown flush, P1 in file, promote; section 10.1).
- **R-17 Add the network stack and the source layer.** Area 12 (HTTP, DoH, cache, rate limiter, WebView) and area 15 and 16 (source APIs). Tasks: T-12a-001 (P0, NetError enum), T-15a-006 (MangasPage), T-16a-013 (AnimesPage), T-15a-031 (list parsing, P0), T-16a-043 (manga models, P0).
- **R-18 Replace the placeholder player surface.** `src/player.rs:159` is a `gtk::Video`-like placeholder. ADR-02 chooses libmpv through `libmpv2`. The surface needs a GL area or the mpv render API. Tasks: T-45b-075 (P0 L), T-45a-025 (window key bindings, P0), T-45a-044 (fullscreen, P0), T-45b-078 (sheets, P0 L).
- **R-19 Replace the sidebar badge constants.** `src/main.rs:175` returns `"8"` and `"1"`. Tasks: T-42b-039 (toast), T-42b-041 (single-flight), T-08a-025 (updates tab state with counters).
- **R-20 Add the toast component.** None exists. Tasks: T-42b-039 (P1), decision D-16 in area 42.
- **R-21 Add the webview engine behind `WebViewHost`.** `src/entry.rs:580` has no engine. ADR-04 chooses WebKitGTK through `webkit6`. Tasks: T-36a-049 (P0), T-12a-019 (P1), T-12a-041 (P1 extraction).
- **R-22 Remove the development-only mockup paths from release builds.** `KUROBAYOMI_OPEN`, `KUROBAYOMI_SHOT`, `KUROBAYOMI_SMOKE` (section 2.1). Keep them behind `cfg(debug_assertions)` or a `dev-tools` feature (Proposal). No task title in `TASKS.csv` covers these hooks (searched for dev tool, debug hook, smoke, and the variable prefix). Record a new task in area 54 when the plan is next revised; T-54a-031 covers gettext only.
- **R-23 Replace the `ponytail:` shortcuts with the planned implementations.** Three comments mark deliberate shortcuts (`src/ui.rs:176`, `src/player.rs:159`, `src/entry.rs:580`). Track them with the ponytail debt ledger (`ponytail-debt`) until the replacing tasks land.

---

## 8. Spec format gaps (`spec/SCHEMA.md` vs what the plans need)

- **Item types lack bindings.** `spec/SCHEMA.md` defines `switch`, `list`, `multi`, `slider`, `text`, `tristate`, `sort`, `chips`, `link`, `action`, `info`. None carries a preference key, a registry default, a value type or a range source. Area 02 needs: `pref` (key), `values` (typed choices with stored values), and `min`, `max`, `step` on sliders (T-02a-036). Proposal: add a `"pref":"<key>"` field to every control variant and a `"stored"` array parallel to `options` for single and multi choice (the stored string or int value per option).
- **`tristate` has no states.** The plan needs three states: off, include, exclude (chip filters, area 40 Q1 and the library filter area 37). Proposal: `{"type":"tristate","title":str,"pref":str}` with states `Off`, `Include`, `Exclude` fixed in code.
- **`sort` has no default or direction.** The plan stores a sort key and an ascending flag (library, updates, migrate source list). Proposal: `{"type":"sort","title":str,"options":[str],"pref":str}` with the flag stored as `<pref>.asc` (area 52 selection and sort, T-52a-* series).
- **Unbound groups.** `Group.title` may be `null`. Fine for GEN screens. Area 47 needs a per-group key for search breadcrumbs (`Category > Group`, `src/settings.rs:47-91`).
- **Links to screens that do not exist.** `spec/settings.json` main lists `settings_tracking`, and the spec-consistency test (`src/spec.rs:131-191`) checks every link target. `library` is a nav id with no screen. Proposal: the nav list is typed in code (see R-10), not in JSON.
- **`home` is an orphan screen** (row 1). Remove it.
- **Strings.** The spec holds English text. Area 51 says every string moves to a catalogue with a key (T-51a-031, P1 L). Proposal: keep English in the spec only until the catalogue lands, then the spec holds keys.

---

## 9. Build order for usable Kurobayomi (P0 path, with the mockup screens each step unlocks)

The order below follows each area's own dependency chain. Each step names the first P0 tasks and the screens from section 5 that become real. Step 0 is a prerequisite for every other step.

| Step | Work | Tasks (first ones, in dependency order) | Screens made real |
|---|---|---|---|
| 0 | Workspace, startup, logging, crash hook, runtime, AppContext | T-01a-001, T-01a-003, T-01a-004, T-01a-005, T-01a-006, T-01a-007 | crash (row 13) |
| 1 | Preference store and bound settings | T-02a-001, T-02a-023, T-02a-030, T-02a-036 (promote), T-02a-038 (promote), T-01a-010 | settings (33), settings_search (34), settings_appearance (35), settings_reader (38), and the settings rows that bind in step 1 |
| 2 | Manga database and repositories | T-03a-002, T-03a-003, T-03a-004, T-03a-034, T-05a-026 | repositories used by rows 2 to 5 |
| 3 | Anime database | T-04a-028 | rows 2, 15, 64 |
| 4 | Domain: entries, library, categories | T-07a-040, T-08a-013, T-08a-033, T-37a-017, T-37a-061 | anime_library (2), manga_library (3), categories (8) |
| 5 | Details pages | T-38a-010, T-39b-063, T-39b-064, T-39a-074 | manga_entry_details (14), anime_entry_details (15) |
| 6 | Source layer: manga | T-15a-006, T-15a-027, T-15a-031, T-15a-051, T-16a-043, T-06a-081 | browse_source (28), browse (18, manga half) |
| 7 | Source layer: anime | T-16a-013, T-16a-029 (P1), T-16a-043 | browse (18, anime half) |
| 8 | Reader | T-43a-001, T-43a-005, T-43a-008, T-43a-013, T-43a-016, T-43a-025, T-43a-028, T-43a-029, T-43a-077, T-44a-021, T-44a-023 | reader (31) |
| 9 | Player | T-45b-075, T-46a-042, T-46b-009, T-46b-046, T-45a-025, T-45a-044 | player (32), settings_player_* as bound |
| 10 | Progress, history, updates | T-06a-080, T-42a-021, T-42a-007, T-42b-036, T-42b-039 | history (5), updates (4), sidebar badges (R-19) |
| 11 | Downloads | T-23a-002, T-23a-012, T-23a-013, T-23a-031, T-25a-033 | download_queue (7), settings_downloads (50) |
| 12 | Global search and extensions | T-40a-034, T-41a-001, T-41a-005, T-41b-010, T-41a-027, T-47b-010, T-47b-023 | global_search (29), extension_repos (23), settings_anime_extension_stores (53), settings_manga_extension_repos (54) |
| 13 | Tracking | T-28a-025, T-28a-057, T-27a-069 | tracking_sheet (16), tracker_search (17) |
| 14 | Settings completion, backup, storage, debug | T-47a-075, T-13a-016, T-42a-052, T-21a-011, T-22a-033, T-47b-040 | clear databases (63, 64), storage (10), settings_data_storage (55), create and restore backup (56, 57) |
| 15 | Onboarding, webview, app lock | T-36a-024, T-36a-049, T-36a-032 | onboarding (11), webview (30), settings_security (58) |

Notes:
- Step 1 must land before every settings row is real. Without it, rows 33 to 67 stay GEN and inert.
- Step 6 and step 7 are independent after step 0 and can run in parallel.
- The P0 queue of area 43 (86 P0 tasks) and area 45 (73 P0 tasks) are the largest single chains.

---

## 10. Plan-side gaps found while auditing

### 10.1 Priority state in `TASKS.csv` does not match `RISKS-AND-DECISIONS.md` section 3

| Task | `TASKS.csv` today | Proposed in RISKS section 3 | Action |
|---|---|---|---|
| `T-20a-001` WASM component loader | P0 | P1 (or split: built-in loader P0, host P1) | Demote in TASKS.csv or split. Blocks nothing in the mockup path if built-in sources are used. |
| `T-02a-036` value ranges | P1 | P0 | Promote. R-12 and every slider depend on it. |
| `T-02a-038` shutdown flush | P1 | P0 | Promote. Without it a quit within 250 ms loses a setting. |
| `T-05a-025` category rules | P1 | P0, or ship the UI subset | Promote, or accept that category create, rename and delete are missing from step 4. |
| `T-03a-022`, `T-03a-023` library import | P2 | P1 if U-1 is yes | Depends on the answer to U-1. Not changed here. |
| `T-34a-028` AVIF and JPEG XL | P0 | P0 only if sources serve AVIF or JXL | Keep P0 until U-3 is answered; otherwise P1. |
| `T-39b-052` torrent server start | P2 | P2, skip for v1 unless U-5 is yes | Consistent. |
| `T-08a-038` Glance widgets (drop) | P3 | drop | Consistent. |
| `T-25a-026` queue keyboard shortcuts | P1 | P1 (desktop-only, Alt+Up, Alt+Down, Delete, Space) | Consistent with RISKS (the table says P2 to P1, and the file says P1). |

### 10.2 Zero-P0 areas that the mockup screens depend on

- Area 21 and 22 (backup create and restore): rows 56 and 57 are GEN in the mockup and have no P0 tasks. Their first task is T-21a-011 (P1). Accept as P1 or promote the create and restore pages.
- Area 27 (tracking core, 123 tasks, 0 P0): row 16 (`tracking_sheet`) and row 51 (`settings_tracking`) get their first task from area 28 (AniList, P0). Confirm that AniList alone is the v1 tracker.
- Area 24 (anime downloads, 0 P0): row 50 and the anime half of row 7. Confirm that anime downloads are P1.
- Area 50 (Android widgets, 0 P0): no mockup screen. Correct: the widgets are dropped (RISKS section 4).

### 10.3 Cross-area ownership conflicts still open

- Row list widget: area 42 D-15 chose `gtk::ListView` for everything; area 01 and area 37 use `gtk::ListBox` in their own text. Confirm D-15 applies to library, browse and settings rows.
- Toast: area 42 D-16 owns the shared toast. Area 01 and area 35 propose their own. Use the area 42 component.
- Settings tree location: area 47 says `src/settings/mod.rs`; area 47b D-1 says `src/settings.rs` plus children. Section 7 R-09 uses the file form.
- Layout D-2 (area 42): adopted. Confirmed in section 3.3.
- Relative time: area 01 `format::relative_time_span` (area 42 D-3). Confirmed.
- Reader keys: area 43 owns the reader keys; area 47 displays them. Confirmed by area 43 overview section 1.3.

### 10.4 Line-number corrections

- Sidebar badge: `src/main.rs:175`, not `src/ui.rs:175` (area 37 and area 47b). Verified.
- Worker info text: `src/settings.rs:17`. Backup schema text: `src/settings.rs:18`. Area 21 says `src/settings.rs:18` contains the placeholder text dump for `settings_backup_schema`, which is correct, and it also covers worker info at 17.
- Library grid: `src/screens.rs:154-276` (area 37 cites `src/screens.rs` lines 154-272; the function ends at 276).
- Reader function: `src/player.rs:49-149` (area 43). Verified against the file.

---

## 11. Open questions that block parts of the gap closure

From [RISKS-AND-DECISIONS.md](RISKS-AND-DECISIONS.md) section 2, with the screens they affect.

| # | Question | Screens affected (section 5 rows) | Recommendation in RISKS |
|---|---|---|---|
| U-1 | Aniyomi library to import? | Library rows 2, 3, 8 (content only) | Yes if available |
| U-2 | Accept that Aniyomi APK extensions stop working? | Rows 18, 19, 20, 21, 22, 23, 53, 54 | Yes |
| U-3 | Which sources do you use? | Rows 18, 28, 29 (content) | Port the top sources by entry count |
| U-4 | WASM in the first release? | Rows 19, 20, 53, 54 | No |
| U-5 | Torrent-sourced anime? | Rows 45, 32 (torrent part) | No |
| U-6 | App lock in v1? | Rows 15 (no), 58 | Opt-in, default off, passphrase, P1 |
| U-7 | Cancel on the lock overlay | Row 58 | Overlay stays; the app stays |
| U-8 | Update check repository | Rows 12, 65 | Set `update.repo` when the repository exists; empty until then |
| U-9 | `kuro_` prefix for Kurobayomi-only settings in backups | Rows 56, 57 | Yes |
| U-10 | Simkl client secret in the binary | Row 16 (Simkl login) | Yes |
| U-11 | Unicode lowercasing in source ids | Rows 18, 28 | Yes |
| U-12 | Week start for Upcoming | Row 4 (updates) | Locale table, area 42 D-24 |
| U-13 | Delete-downloads box in migrate pre-checked? | Rows 24, 26 | OFF |
| U-14 | Clear history keeps time read? | Row 5 | Keep Aniyomi semantics; confirm |

---

## 12. Features dropped or replaced that change a mockup screen

From [RISKS-AND-DECISIONS.md](RISKS-AND-DECISIONS.md) section 4. The mockup shows some of these today.

| Feature | Mockup screen affected | Decision | What the user loses |
|---|---|---|---|
| Android APK extensions | rows 18 to 23, 53, 54 | Dropped; replaced by built-in and WASM sources | Existing APK extensions |
| Home-screen widgets | row 4 (updates badge), row 6 (More) | Dropped; later KDE Plasma widget or tray menu | Home-screen updates widget |
| Android notification channels and deep links | none directly | Replaced by freedesktop notifications via `notify-rust` | Starting the app from a notice |
| In-app APK update | row 12 (new update prompt) | Dropped; distro packages update the app | In-app update and preview builds |
| Display cutout | row 31 (reader) | Hidden | Cutout padding |
| Biometric lock | row 58 | Replaced by passphrase (ADR-09) | Biometric unlock |
| `FLAG_SECURE` screenshot block | row 58 and the screenshot setting (`settings_security`) | Setting kept, shows "Not available on Linux" | Screenshot blocking |
| ICC display profile | row 31 (reader) | Dropped for P0 and P1; optional `icc` feature at P2 (T-34a-031) | Colour-managed display |
| Data saver services (`bandwidth-hero`, `wsrv.nl`, `resmush.it`) | rows 52 (browse settings) | Dropped; `NONE` kept so settings parse | Image proxying |
| Migrate: pre-checked delete of downloads | rows 24, 26 | Kept, default OFF (U-13) | None if default OFF |
| Clear database reset-flags UI | rows 63, 64 | No UI; method kept and tested | UI for reset flags |
| Random sort exact shuffle | row 2, 3 (sort) | Accepted different order (ChaCha8) | Same order as Aniyomi for the same seed |
| Platform `ImageUtil` and `ImageDecoder` | row 31, 14 (cover and page decode) | Replaced by `image`, `avif-decode`, `jxl-oxide` | None beyond the format list in area 34 |
| Volume-key page turning | row 31 (reader) | Dropped; preference kept (T-43a-071) | Volume keys turn pages |
| Screen orientation lock | row 31 (reader) and row 39 (player) | Dropped; values preserved (T-43a-068, T-45b-058) | Orientation lock |
| Secure-screen flag | row 58 and `settings_security` | Dropped (T-47a-066, T-34a-064) | Screenshot block |

---

## 13. Verification checklist for this gap analysis

- Spec count: 67 = 13 + 17 + 2 + 35. Section 5 has 67 numbered rows (1 to 67).
- Every row has a status, a mockup code location or the note "not hand-built", an owner and first task IDs.
- Task IDs in section 5 and section 9 exist in `TASKS.csv`. Checked against the export at 22:07.
- Kotlin references checked against the tree: `app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt:338-345`, `app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt`.
- Mockup references checked against the files: `src/main.rs:175`, `src/screens.rs:56-59`, `src/screens.rs:141-152`, `src/screens.rs:154-276`, `src/entry.rs:81-243`, `src/player.rs:49-159`, `src/ui.rs:221-353`, `src/ui.rs:506-523`, `src/settings.rs:12-18`, `src/spec.rs:121-128`.
- Totals in section 3.1 match `python3 plans/_tools/tasks.py export` output: tasks 3,801; P0 1,196; P1 1,793; P2 503; P3 309; errors 0.

Hand-off: this file is the input to the task-scheduling step. The scheduling order is section 9. The priority corrections in section 10.1 must be applied to `TASKS.csv` (via the area `p1-tasks.md` source files, then `tasks.py export`) before the P0 count is used for planning.
