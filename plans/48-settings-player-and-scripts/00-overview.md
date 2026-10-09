# 48 settings-player-and-scripts: area overview

Owner: area lead for `48-settings-player-and-scripts`. Repo-relative paths are relative to `/home/blackbox/aniyomi-src` (read-only Aniyomi clone, Apache-2.0). Paths inside Kurobayomi are relative to `/home/blackbox/kurobayomi`. Labels: **OBSERVED** = what the Kotlin does; **Proposal:** = what the Rust port should do.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does in this area

Area 48 is the player settings tree and the user-facing editors that feed the mpv player. Concretely (OBSERVED, from the 40 scope files, 3944 LOC, per `python3 plans/_tools/areas.py files 48-settings-player-and-scripts 1`):

- **Player settings tree**: a root list of nine rows, each opening one page: Internal player, Gestures, Decoder, Subtitles, Audio, Custom buttons, Script editor, Torrent, Advanced (`app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt`, rows at lines 184-239). Details: [p1-behavior.md](p1-behavior.md) section 1.
- **Preference pages** that write typed values into `PreferenceStore` (progress, controls, display, intro skip, PiP, external player, gestures, seeking, double tap, media keys, decoder, debanding, subtitles, audio, advanced mpv files). Details: [p1-behavior.md](p1-behavior.md) sections 3-8, keys and defaults in [p1-data-and-config.md](p1-data-and-config.md) section 1.
- **Custom buttons**: user-written Lua snippets (tap, long press, on-startup) that become buttons in the player. One favourite ("primary") button is drawn in the player. Stored in the anime database table `custom_buttons`. Details: [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md).
- **Script editor and code editor**: list, create, rename, delete files in `mpv-config/scripts` and `mpv-config/script-opts`; a full-file text editor with a regex-based syntax highlighter (Lua and conf rule sets). Details: [p1-topic-mpv-editor-and-highlight.md](p1-topic-mpv-editor-and-highlight.md).
- **TorrServer settings page**: enable switch, legal notice, port, trackers list, proxy mode and URL. The server itself is owned by area:13-core-util-storage-archive-torrent. Details: [p1-topic-torrent-settings.md](p1-topic-torrent-settings.md).
- **Global UI preferences** used by this area's screens (theme mode, colour theme, start screen, navigation style, date format, relative time, tablet mode). Details: [p1-behavior.md](p1-behavior.md) section 15.
- **Storage layout**: the user-chosen base directory and the mpv runtime directory. Details: [p1-behavior.md](p1-behavior.md) section 16, [p1-data-and-config.md](p1-data-and-config.md) section 4.

### 1.2 Why it matters for a usable Kurobayomi

Without this area, the player in Kurobayomi has no real configuration. The mockup today shows placeholder data only: `settings_player_custom_buttons` renders `cards(id, &["Skip opening", "Toggle shader"], true)`, `settings_player_editor` is hard-coded to three files, and `settings_player_code_editor` shows a fixed sample text (see the header of [p1-rust-design.md](p1-rust-design.md) and [p1-ui-parity.md](p1-ui-parity.md) section 6).

For daily use, the parts that matter most are:

1. The player must start with mpv configured (`mpv.conf`, `input.conf`, fonts, decoder choice), which needs the runtime sync (T-48a-036).
2. The default `+85 s` intro-skip custom button must work, so the user can skip openings (T-48a-041, T-48a-042, T-48a-043, T-48a-040).
3. Gesture and seek settings must be stored and read by the player (T-48a-022, T-48a-023, T-48a-025).
4. Language lists and decoder settings must validate and apply without silently breaking playback (T-48a-030, T-48a-026).

### 1.3 Scope boundaries

In scope: every file listed in [p1-inventory.md](p1-inventory.md).

Out of scope, owned elsewhere:

- The player runtime itself (mpv option application, AniSkip network calls, `PlayerViewModel` state): area:45-player-core.
- Rendering of the player controls, panels, sheets and the custom button pill: area:46-player-controls-ui.
- Searchable settings infrastructure (`SearchableSettings`, `PreferenceScaffold`, `SettingsSearchScreen`): area:47b-settings-widgets-search-more.
- The `PreferenceStore` implementation and key registry: area:02-preferences-system.
- The `custom_buttons` schema migration: area:04-db-anime-schema-migrations.
- Backup format fields for custom buttons: area:21-backup-format-and-create and area:22-backup-restore.
- The TorrServer binary and HTTP API: area:13-core-util-storage-archive-torrent.

---

## 2. Module map

### 2.1 Kotlin packages in scope (OBSERVED)

| Package or path (repo-relative) | Responsibility | Files (counted in the 40-file inventory) |
|---|---|---|
| `app/src/main/java/eu/kanade/domain/ui/` | Typed global UI preferences (`UiPreferences.kt`) | 1 |
| `app/src/main/java/eu/kanade/domain/ui/model/` | Enums: `AppTheme`, `NavStyle`, `StartScreen`, `TabletUiMode`, `ThemeMode` | 5 |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/` | Player settings pages: `PlayerSettingsMainScreen`, `PlayerSettingsPlayerScreen`, `PlayerSettingsGesturesScreen`, `PlayerSettingsDecoderScreen`, `PlayerSettingsSubtitleScreen`, `PlayerSettingsAudioScreen`, `PlayerSettingsTorrentScreen`, `PlayerSettingsAdvancedScreen` | 8 |
| `.../screen/player/custombutton/` | Custom button screen, screen model, `components/` (dialogs, list item, screen body) | 5 |
| `.../screen/player/editor/` | Script editor host, screen model, `components/` (list, dialogs, type dropdown) | 5 |
| `.../screen/player/editor/codeeditor/` | Full-file code editor (`CodeEditScreen`, `CodeEditScreenModel`), highlighter engine (`Highlight.kt`), rule sets (`SyntaxHighlight.kt`) | 4 |
| `app/src/main/java/eu/kanade/tachiyomi/ui/setting/PlayerSettingsScreen.kt` | Container choosing single pane or two pane | 1 |
| `data/src/main/java/tachiyomi/data/custombutton/` | SQLDelight repository for `custom_buttons` | 1 |
| `data/src/main/sqldelightanime/dataanime/custom_buttons.sq` | Table, seed row (`+85 s`), queries. Read by area:04 for the schema; referenced by T-48a-041. Not one of the 40 counted inventory files. | 0 |
| `domain/src/main/java/tachiyomi/domain/custombuttons/` | Model (`CustomButton`, `CustomButtonUpdate`), six interactors (create, update, delete, reorder, toggle favourite, get), repository port, exception | 10 |
| `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt` | TorrServer preference keys and defaults. Referenced by T-48a-032; not one of the 40 counted inventory files. | 0 |
| `app/src/main/assets/aniyomi.lua` | Bridge script loaded by mpv (Lua side of the protocol). Referenced by T-48a-040; not one of the 40 counted inventory files. | 0 |

### 2.2 Proposed Rust module map (Proposal)

Taken from [p1-rust-design.md](p1-rust-design.md) section 1. Crate `kurobayomi`, edition 2024, `gtk = gtk4 0.11.5` with feature `v4_16`, no libadwaita.

| Rust module | Replaces (Kotlin) | Notes |
|---|---|---|
| `src/prefs/mod.rs` | `PreferenceStore`, `getEnum` | `PrefStore` trait, typed `Pref<T>`, `NamedEnum`, `SqlitePrefStore` (rusqlite `prefs` table), `PrefCache` |
| `src/prefs/ui.rs` | `UiPreferences.kt` and `domain/ui/model/*` | Theme, nav, start screen, date format, relative time |
| `src/prefs/player.rs` | `PlayerPreferences`, `GesturePreferences`, `DecoderPreferences`, `AudioPreferences`, `SubtitlePreferences`, `AdvancedPlayerPreferences`, `TorrentPreferences` | One typed group per Kotlin file |
| `src/lang.rs` | `PrefLangMigration` filter and the language-list validators | ISO 639 table, shared by audio and subtitle |
| `src/db/custom_buttons.rs` | `CustomButtonRepositoryImpl`, `SaveCustomButtonException` | rusqlite; `thiserror` error variant |
| `src/player/custom_buttons/{model,plan,service,lua}.rs` | Domain model, interactors, mutex-based serialisation | `plan.rs` holds pure reorder, delete and favourite planners |
| `src/player/lua_bridge.rs` | `aniyomi.lua` protocol and the player-side handler | `LuaCall` enum with 14 properties |
| `src/player/vf.rs` | Debanding and `yuv420p` filter chain | `build_vf_chain`, hwdec and vo mapping |
| `src/player/mpv_dir.rs` | `PlayerActivity.setupPlayerMPV` and `copyUserFiles` | Runtime directory sync |
| `src/player/gestures.rs` | `SingleActionGesture` dispatch | Table from gesture to action |
| `src/editor/files.rs` | `PlayerSettingsEditorScreenModel`, `CodeEditScreenModel` file I/O | List, create, rename, delete, atomic save |
| `src/editor/highlight.rs` | `Highlight.kt`, `SyntaxHighlight.kt` | `fancy-regex` rules as data |
| `src/editor/code_view.rs` | `CodeEditScreen.kt` | GtkSourceView 5, with `gtk::TextView` fallback |
| `src/torrent/settings.rs`, `src/torrent/server.rs` | `TorrentPreferences`, JNI TorrServer | Validation, default trackers, child-process lifecycle |
| `src/settings/player/{mod,rows,internal,gestures,decoder,subtitle,audio,advanced,torrent,custom_buttons,script_editor,dialogs}.rs` | Player settings screens | One sidebar root (nine rows) and a `gtk::Stack` |
| `src/i18n_player.rs` | English strings for this area | Table copied from base XML, no invented keys (T-48a-055) |

---

## 3. Reading guide

Read in this order: this overview, the behaviour spec, the data spec, then the rest.

| Document | One-line description |
|---|---|
| [p1-behavior.md](p1-behavior.md) | OBSERVED behaviour of every page, the editor, the custom button flow, the mpv sync and the storage layout, with Proposal notes per section. |
| [p1-data-and-config.md](p1-data-and-config.md) | Every preference key with type and default, enum stored names and values, bit layouts, files and encodings, network endpoints, DB table, backup fields, mpv properties. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Numbered edge cases per area (E-PS, E-GS, E-DC, E-LG, E-AU, E-AV, E-SY, E-ED, E-CE, E-HL, E-CB, E-LB, E-TS, E-UP), races, failure modes and the test plan. |
| [p1-inventory.md](p1-inventory.md) | One row per scope file: LOC, declared names, port decision, target Rust module, priority and task IDs. |
| [p1-rust-design.md](p1-rust-design.md) | Rust module layout, preference traits, enum definitions, custom button planners and Lua generator, bridge, vf chain, settings UI model, Linux mapping table, threading, error handling, security notes, pitfalls. |
| [p1-tasks.md](p1-tasks.md) | 58 tasks T-48a-001 to T-48a-058 with priority, effort, dependencies, sources and acceptance criteria, plus the open decisions D-01 to D-08. |
| [p1-ui-parity.md](p1-ui-parity.md) | Screen-by-screen visible elements, dialogs, messages, responsive rules, animations, mockup gaps and accessibility for the desktop port. |
| [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) | Custom button domain model, storage, create, update, delete, reorder, favourite algorithms, `custombuttons.lua` output, bridge protocol, backup, limits. |
| [p1-topic-mpv-editor-and-highlight.md](p1-topic-mpv-editor-and-highlight.md) | Script and config file layout, editor file-list rules, code editor rules, the ten Lua and six conf highlight rules in order, performance, port decisions. |
| [p1-topic-torrent-settings.md](p1-topic-torrent-settings.md) | TorrServer preference keys and defaults, the 26-entry default tracker string, service lifecycle, Linux process port, validation rules, edge cases. |

---

## 4. Key design decisions and open questions

### 4.1 Decisions already taken in the area docs (Proposal, recommended as-is)

| # | Decision | Why | Source |
|---|---|---|---|
| DD-1 | Preferences live in a SQLite `prefs` table with an in-memory cache. Enum values are stored by constant name; unknown names fall back to the default silently, as in Kotlin. | Keeps Kotlin key names and behaviour; avoids a second storage system. | [p1-rust-design.md](p1-rust-design.md) section 2; [p1-data-and-config.md](p1-data-and-config.md) header |
| DD-2 | Custom buttons are one `custom_buttons` table with the same columns as Kotlin, the same seed row (`_id 1`, `+85 s`, favourite, sortIndex 0), and writes through one worker. The Kotlin mutex becomes "the worker runs one job at a time". | Same data, same behaviour, no lock needed. | `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`; [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) section 12 |
| DD-3 | Delete promotes no other button to favourite (current behaviour). | Matches Kotlin; a promote option can come later at P2. | [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) section 12 |
| DD-4 | Generated `custombuttons.lua` wraps each button in its own `pcall`, so one bad Lua body does not break the others. | Kotlin shares one script; a single error breaks all buttons. | [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) section 7 |
| DD-5 | The highlighter is one implementation using `fancy-regex`, with the 15000-character limit applied only to the full-file editor. | Kotlin uses a regex dialect that needs lookaround and backreferences; the custom button dialog is small. | [p1-topic-mpv-editor-and-highlight.md](p1-topic-mpv-editor-and-highlight.md) section 6; `CodeEditScreenModel.kt:133` (`HIGHLIGHT_MAX_SIZE = 15000`) |
| DD-6 | TorrServer runs as a child process (`torrserver` from configured path or `PATH`), not embedded. The child is stopped on switch-off and on app exit. | The upstream server is a Go program; JNI does not apply on Linux. | [p1-topic-torrent-settings.md](p1-topic-torrent-settings.md) section 5 |
| DD-7 | Android-only facilities are replaced by named Linux facilities: `gio::AppInfo` for external players, `gtk::UriLauncher` for links, `gio::Notification` for optional torrent notices, `gtk::AlertDialog` for plain confirmations, MPRIS for media keys. | Keeps the user's GTK theme; no libadwaita. | [p1-rust-design.md](p1-rust-design.md) section 11 |
| DD-8 | One settings layout: a left sidebar with the same nine rows and a right `gtk::Stack`. The phone and two-pane variants collapse into this. | Desktop has one window shape. | [p1-behavior.md](p1-behavior.md) section 1 (Proposal) |

### 4.2 Open questions for the user

Each has a recommendation. The IDs match the open decisions table at the end of [p1-tasks.md](p1-tasks.md).

| ID | Question | Recommendation |
|---|---|---|
| D-01 | Show disabled settings greyed out, or hide them as Android does? | Grey them out. Hiding makes it impossible to discover a setting that is unavailable on this system. |
| D-02 | The Auto and Auto-safe mapping for audio/decoder (E-AU-01): keep the Kotlin mapping or fix it? | Fix it and mention it in release notes. |
| D-03 | Language codes: accept ISO 639-1 only, or 639-1 and 639-3? | Accept both (the proposed answer in [p1-tasks.md](p1-tasks.md); E-LG-06 tests 3-letter codes). |
| D-04 | Per-button `pcall` isolation for custom button Lua (E-CB-01). | Yes. A user-written button should not disable every other button. |
| D-05 | Keep the "Remember volume" switch (E-PS-01)? | Remove it (T-48a-052). Desktop audio policy is handled by PipeWire or PulseAudio, and keeping a stored level invites surprise. |
| D-06 | Deleting the favourite button: promote nothing (current) or promote the next one? | Keep current behaviour. Add the promote option as P2 later. |
| D-07 | Custom button create dialog closes before the write result is known (Kotlin). | Keep the dialog open until the write succeeds and show the error inline. |
| D-08 | Renaming a script: keep the extension or allow it to change? | Keep the extension. Changing `.lua` to `.conf` silently changes how mpv loads the file. |

### 4.3 Additional questions for the user (not yet in the task file)

| ID | Question | Recommendation |
|---|---|---|
| Q-1 | TorrServer binary: bundle it, or require the user to install `torrserver` and point the app at it? | Require the user to install it and set the path. Bundling needs a licence and distribution review that is outside this area. Default search is `PATH`. The upstream CLI flags must be verified before T-48a-033 starts. |
| Q-2 | Use GtkSourceView 5 for the code editor, or plain `gtk::TextView`? | Use GtkSourceView 5 if the package is installed on the target system; otherwise fall back to `TextView` with the same highlighter spans. Confirm that the package is acceptable as a dependency. |
| Q-3 | Show a warning in the script editor that script changes apply at the next player start? | Yes. Kotlin has no warning, and the file is copied at start only (see [p1-topic-mpv-editor-and-highlight.md](p1-topic-mpv-editor-and-highlight.md) section 6). |

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | What is needed | Used by tasks |
|---|---|---|
| area:01-foundation-app-bootstrap | Base data directory layout and `mpv-config` root (T-48a-051 depends on it). | T-48a-051, T-48a-036 |
| area:02-preferences-system | Key registry and the `PreferenceStore` contract (Kotlin); the Rust preference store is owned by this area (T-48a-001 depends on area:02 for the key registry). | T-48a-001 |
| area:04-db-anime-schema-migrations | The `custom_buttons` table, migration 129 and the seed row. | T-48a-041 |
| area:13-core-util-storage-archive-torrent | TorrServer binary and HTTP API; `TorrentHelpers` test fixture (see [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 0). | T-48a-032, T-48a-033 |
| area:21-backup-format-and-create and area:22-backup-restore | `BackupCustomButtons` proto fields (`Backup` field 506, `LegacyBackup` field 109). | T-48a-048 |
| area:36-ui-shell-navigation-security-webview | Shell tab set, start screen and bottom-bar layout used by T-48a-005 and T-48a-006. | T-48a-003, T-48a-005, T-48a-006, T-48a-004 |
| area:45-player-core | mpv option application, Lua bridge dispatch, primary button semantics (`PlayerViewModel.kt:315-345, 884-945`), AniSkip calls, single-gesture dispatch targets. | T-48a-016, T-48a-025, T-48a-040, T-48a-052 |
| area:46-player-controls-ui | Rendering of the favourite pill and panels; receives state from T-48a-044. | (no task in this area depends on it; see 5.2) |
| area:47b-settings-widgets-search-more | `SearchableSettings`, `PreferenceScaffold`, search screen. | T-48a-010, T-48a-011, T-48a-057 |

Migrations: `VideoOrientationMigration` (127), `VideoPlayerPreferenceMigration` (126), `PlayerPreferenceMigration` (92), `MovePlayerPreferencesMigration` (93) and `PrefLangMigration` (130) are area:11 concerns. Desktop does not carry Android preference migrations, so the port starts from the current key names only.

### 5.2 What others need from this area

| Area | What it needs from area:48 |
|---|---|
| area:45-player-core | Typed player preference groups (`PlayerPrefs`, `GesturePrefs`, `DecoderPrefs`, `AudioPrefs`, `SubtitlePrefs`, `AdvancedPrefs`) from T-48a-001 and T-48a-012; the `custombuttons.lua` and `aniyomi.lua` runtime files from T-48a-036 and T-48a-043; the bridge from T-48a-040. |
| area:46-player-controls-ui | Favourite (primary) button state and name from T-48a-044; the gesture-to-action table from T-48a-025. |
| area:21-backup-format-and-create and area:22-backup-restore | Custom button rows and their columns (see [p1-data-and-config.md](p1-data-and-config.md) section 2 and 8). |
| area:49-theme-and-design-system | Named colour themes (T-48a-004) and the "System colours" mapping (T-48a-058). |

Gap: no row in `plans/TASKS.csv` lists `area:48-settings-player-and-scripts` in its Depends column. The owners of area:45 and area:46 should add `area:48-settings-player-and-scripts` to the tasks that read player preferences, the Lua bridge or the favourite button state. This pass did not edit their files.

### 5.3 Task file fixes made in this pass

- Lint: `python3 plans/_tools/tasks.py lint plans/48-settings-player-and-scripts/p1-tasks.md` prints `ok` (it was already clean; no format errors).
- Export: `python3 plans/_tools/tasks.py export` reports `errors=0` for all 3801 tasks. Every `area:` token in the area's Depends lines names a real folder.
- Dependency cycles inside this area were removed. The lint does not check cycles, so they were found with a graph walk over `TASKS.csv`:
  - T-48a-035 and T-48a-036 depended on each other. Removed T-48a-035 from T-48a-036's Depends (the runtime sync needs the layout and the generated Lua, not the page).
  - T-48a-038 and T-48a-039 depended on each other. Changed T-48a-039 to `Depends: none` (the highlighter is a pure engine).
  - T-48a-040 and T-48a-044 depended on each other. Removed T-48a-044 from T-48a-040 and changed T-48a-040 to depend on T-48a-042 (the repository it reads from).
- Task IDs were not changed.
- Other cycles exist in the whole plan set (43 cycle hits, in areas 14, 21, 24, 26, 29, 34, 37, 38, 42, 45, 46, 47, 53, 54). They are outside this area and were not changed.

---

## 6. Task summary

Source: `python3 plans/_tools/tasks.py export`, then the rows of `plans/TASKS.csv` whose `file` starts with `48-settings-player-and-scripts/`. There are 58 tasks, T-48a-001 to T-48a-058.

### 6.1 Counts

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 7 | 11 | 2 | 0 | 20 |
| P1 | 12 | 13 | 0 | 0 | 25 |
| P2 | 3 | 2 | 2 | 0 | 7 |
| P3 | 6 | 0 | 0 | 0 | 6 |
| **Total** | **28** | **26** | **4** | **0** | **58** |

Upper-bound effort using the plan's rules (S at most 1 day, M at most 3 days, L at most 1.5 weeks = 7.5 days): P0 at most 55 person-days; the whole area at most 136 person-days. These are ceilings, not a schedule.

Tasks listed as P0 (20): T-48a-001, 002, 005, 010, 012, 013, 014, 015, 022, 023, 025, 030, 036, 040, 041, 042, 043, 044, 051, 056.

### 6.2 P0 critical path (in order)

Two chains run in parallel. Each step lists the tasks it unlocks.

1. **Storage and data prerequisites** (no dependency inside the area): T-48a-051 (storage root and mpv-config layout, needs area:01) and T-48a-041 (`custom_buttons` table and seed row, needs area:04). T-48a-030 (language list validator) has no dependency at all.
2. **Typed preferences**: T-48a-001 (needs area:02 and area:04). Unlocks T-48a-002 and T-48a-025 and T-48a-042.
3. **UI preference model and start screen**: T-48a-002, then T-48a-005 (start screen, needs area:36).
4. **Preference row kit**: T-48a-012 (L, needs T-48a-001). Unlocks every settings page.
5. **Player settings root**: T-48a-010 (M, needs T-48a-012 and area:47b). Unlocks all page tasks.
6. **Player pages (P0 parts)**: T-48a-013 (Controls and Hosters), T-48a-014 (Display), T-48a-015 (progress threshold), T-48a-022 (seeking).
7. **Gesture dispatch and double-tap**: T-48a-025 (M, needs T-48a-001 and area:45), then T-48a-023 (double-tap settings, needs T-48a-025, T-48a-012 and T-48a-010).
8. **Custom button data path**: T-48a-042 (repository and worker, needs T-48a-041 and T-48a-001), then T-48a-043 (`custombuttons.lua` generation).
9. **Runtime sync**: T-48a-036 (needs T-48a-051 and T-48a-043). Writes the mpv runtime directory at each player start.
10. **Bridge and primary state**: T-48a-040 (L, needs T-48a-042 and area:45), then T-48a-044 (primary button state).
11. **Verification last**: T-48a-056 (tests, needs T-48a-001, 030, 040, 042, 043 and 039).

The two longest weighted chains (effort weights S=1, M=2, L=4) are T-48a-001, T-48a-012, T-48a-010, T-48a-023 and T-48a-001, T-48a-042, T-48a-040, T-48a-056. Both weigh 10.

### 6.3 Risks

1. **Lua bridge depends on area:45 (L).** T-48a-040 needs the player runtime's option and property handling. If area:45 changes its property names, the bridge breaks. Mitigation: the 14 property names are fixed in [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) section 8, and T-48a-040 tests use a fake mpv handle.
2. **Custom button Lua can break the player.** A bad body in the shared script stops every button (E-CB-01). Mitigation: DD-4 (per-button `pcall`), decision D-04.
3. **libmpv binding API is not verified.** [p1-rust-design.md](p1-rust-design.md) section 11 says "verify observe/set/command APIs" for `libmpv2`. Mitigation: spike before T-48a-040 starts.
4. **A P0 task depends on a P1 task.** T-48a-056 (P0 tests) depends on T-48a-039 (P1 highlighter). If the highlighter slips, the P0 test gate slips too. Mitigation: either raise T-48a-039 to P0 or drop it from T-48a-056's Depends and test the highlighter in its own task.
5. **GtkSourceView availability.** Q-2. If the package is absent, the editor is plainer and line numbers come from a gutter widget.
6. **TorrServer provenance and CLI flags.** Q-1. Upstream flags must be checked before T-48a-033; if the binary is not available the TorrServer page stays disabled (D-01).
7. **Silent enum fallback.** Unknown stored enum names fall back to the default with no log (see [p1-data-and-config.md](p1-data-and-config.md) header). A typo in a future migration would reset a setting silently. Mitigation: log a warning in `NamedEnum::from_name` (Proposal).
8. **Script edits during playback.** The file is copied only at player start. Mitigation: Q-3 warning text.
9. **Regex dialect drift.** Java `\w`, `\d`, `\s` differ from Rust defaults. T-48a-039 forces explicit ASCII classes (E-HL-07, E-HL-10); the golden tests in T-48a-056 catch drift.

---

## 7. Definition of done for this area

Observable behaviour in the running Kurobayomi app. Each item maps to one or more tasks.

1. **Player settings tree**: Settings opens the Player page. The left sidebar lists the nine rows in this order: Internal player, Gestures, Decoder, Subtitles, Audio, Custom buttons, Script editor, Torrent, Advanced (titles as in [p1-behavior.md](p1-behavior.md) section 1). Selecting a row replaces the right-hand page. Search for "subtitle" returns Subtitle page rows; search for "mpv" returns Advanced page rows (T-48a-010, T-48a-011, T-48a-057).
2. **Preferences persist**: Change any switch, list, slider or text on any page, close the app, reopen it: the value is still shown. Enum values survive a rename of the Rust variant because they are stored by Kotlin name (T-48a-001, T-48a-012).
3. **Player honours settings**: Change decoder hardware decoding and debanding; start a video; mpv runs with the matching options (check the mpv property values or the log). Set the progress threshold to a different value and confirm the saved choice is shown after a restart (T-48a-015, T-48a-026, T-48a-028, T-48a-036).
4. **Double tap**: Set the left double tap to Seek and the right double tap to Play/pause. Double tap on the left seeks back by the skip length; double tap on the right toggles pause (T-48a-023, T-48a-025).
5. **Default intro skip button**: Start an anime episode. The `+85 s` button is visible in the player's bottom-right area. Tap it: the player seeks forward by the intro length. Long-press it: a picker from 0 to 255 seconds appears; set 30 and the title changes to `+30 s`. Set 0 and the button hides (T-48a-040, T-48a-043, T-48a-044; [p1-topic-custom-buttons.md](p1-topic-custom-buttons.md) sections 1 and 7).
6. **Custom buttons CRUD**: Create a button with a name and a tap body, then edit it, reorder it by drag, mark it as favourite (only one can be favourite), delete it. The list renumbers `sortIndex` as 0..n-1 after a delete. A name that already exists shows "Button already exists!" under the Title field and disables the confirm button ([p1-ui-parity.md](p1-ui-parity.md) section 2.9; T-48a-042, T-48a-045, T-48a-046, T-48a-047).
7. **Bad custom button Lua does not break others**: Save a button whose tap body has a syntax error. Start a video: the other buttons still work and the error appears in the application log (T-48a-043; D-04).
8. **Script editor**: Open Script options, create `test.conf`, rename it to `test2.conf`, confirm the extension is kept (D-08). Delete it after a confirmation. Create a file that already exists and see "File already exists" (T-48a-037).
9. **Code editor**: Open `aniyomi.lua` or a user script. Edit it; Save is enabled; Ctrl+S saves; the file on disk changes. Press Back with unsaved edits: the "Warning" dialog with "You have unsaved changes. Do you still wish to go back?" appears. A file over 15000 characters opens with no highlighting (T-48a-038, T-48a-039).
10. **Atomic save**: Simulate a write failure during save (read-only target directory): the original file is unchanged and an error toast is shown (T-48a-038; E-CE-05).
11. **Runtime directory**: After start, the player runtime directory contains `mpv.conf`, `input.conf`, `scripts/aniyomi.lua` and `scripts/custombuttons.lua`. Deleting a user script removes it from the runtime directory on next start (T-48a-036; E-SY-01).
12. **Language lists**: Enter `en,xx` in the subtitle preferred languages field: the page reports `xx` as the invalid code (E-LG-07) and does not save the list. A space after a comma is invalid (E-LG-03). Whether 3-letter codes are accepted follows D-03 (T-48a-029, T-48a-030).
13. **TorrServer page**: With a valid `torrserver` path set, enabling the switch starts a child process that stops when the switch is turned off or the app quits. A port that is not a number in 0..=65535 is rejected with "Port must be a number between 0 and 65535" ([p1-topic-torrent-settings.md](p1-topic-torrent-settings.md) section 6; T-48a-032, T-48a-033, T-48a-034).
14. **Narrow layout**: Below 720 px width the window shows a single pane with a back button in the header; the minimum window size is 360 x 480 px ([p1-ui-parity.md](p1-ui-parity.md) section 4; T-48a-010).
15. **Tests**: `cargo test` runs the unit and integration tests listed in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 4 items 1-14 and passes (T-48a-056).
16. **No placeholder data**: The mockup entries `settings_player_custom_buttons`, `settings_player_editor` and `settings_player_code_editor` are no longer built from the fixed sample strings in `src/settings.rs`; each shows data read from the database or the file system (T-48a-037, T-48a-038, T-48a-045).
