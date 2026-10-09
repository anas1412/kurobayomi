# Area 47b — Implementation tasks (settings widgets, search highlight, More tab, update dialog, onboarding wrapper)

Task IDs use the prefix `T-47ba-`. Sources are repo-relative to `/home/blackbox/aniyomi-src` (Kotlin reference) unless a path starts with `src/`, `spec/` or `resources/`, which is in `/home/blackbox/kurobayomi`. Behaviour details are cited as `p1-behavior.md §x`; UI as `p1-ui-parity.md §x`; design as `p1-rust-design.md §x`; edge cases as `T-E..` in `p1-edge-cases-and-tests.md`.

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint /home/blackbox/kurobayomi/plans/47b-settings-widgets-search-more/p1-tasks.md`

Priority: P0 needed for daily use; P1 parity; P2 polish; P3 Android-only or drop record.

Coverage: every file in scope is covered by at least one task; every behaviour section in `p1-behavior.md` has a task; every edge case ID is either a test in `p1-edge-cases-and-tests.md` or a task acceptance bullet.

---

## Preference model and rendering

### T-47ba-001 — Settings model: Preference, PreferenceItem and PreferenceGroup in Rust
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt, src/settings/model.rs
- Acceptance:
  - `src/settings/model.rs` defines `Preference`, `PreferenceGroup`, `ItemCommon` and the `PreferenceItem` enum with the 13 variants listed in `p1-rust-design.md §2` (Text, Switch, Slider, List, BasicList, MultiSelect, EditText, MultiLineEdit via EditText flags, MpvConf, Tracker, Info, Custom) and no unchecked casts.
  - Default values match `p1-data-and-config.md §1.1`: `enabled = true`, More/Info subtitle `None`, list and edit subtitle `Some("%s")`, slider steps `max(0, (last - first) - 1)`.
  - Unit test T-E16 passes (slider steps for `0..1`, `0..10`, `5..5`).

### T-47ba-002 — Settings page registry and spec-to-model conversion
- Priority: P0
- Effort: S
- Depends: T-47ba-001, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScaffold.kt, src/settings.rs, src/spec.rs, spec/settings.json
- Acceptance:
  - `src/settings.rs` moves to `src/settings/mod.rs` with its `body(id)` router unchanged for non-scope pages (refactor only; the mockup still renders every screen in `spec/settings.json`).
  - `src/spec.rs` maps spec items (`Item::Switch`, `List`, `Multi`, `Slider`, `Text`, `Tristate`, `Sort`, `Chips`, `Link`, `Action`, `Info`) to `PreferenceItem` values, with unsupported kinds logged and skipped rather than panicking.
  - Spec labels are the exact English strings (`spec/SCHEMA.md`).

### T-47ba-003 — Item dispatch: render every PreferenceItem variant
- Priority: P0
- Effort: M
- Depends: T-47ba-001, T-47ba-006, T-47ba-048, T-47ba-049, T-47ba-052, T-47ba-043, T-47ba-044, T-47ba-045, T-47ba-046, T-47ba-047, T-47ba-050
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, src/settings/render.rs
- Acceptance:
  - `settings::render::item_row` matches every variant exhaustively (compiler-checked), and returns a `gtk::ListBoxRow` for each.
  - A bound preference (`Pref<T>`) writes only when the veto returns true (behavior §A.5); a vetoed write leaves the widget unchanged (T-E20).
  - Unit-level check: the enum has exactly 13 variants (compile-time match, no `_` arm).

### T-47ba-004 — Enabled/disabled items and groups
- Priority: P0
- Effort: S
- Depends: T-47ba-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt
- Acceptance:
  - Disabled items are wrapped in a `gtk::Revealer` (`slide-down`, 150 ms) and made insensitive; they stay in the list.
  - A group with `enabled == false` is not built at all (behavior §A.3.3).
  - Re-enabling an item slides it in without a page rebuild (verified by toggling a bound flag in the mockup).

### T-47ba-005 — Settings page scaffold: header, ScrolledWindow, group ListBoxes, spacing
- Priority: P0
- Effort: M
- Depends: T-47ba-001, T-47ba-007, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScaffold.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt, src/ui.rs
- Acceptance:
  - `settings::page::build` returns a header bar (title, back when pushed), and a `gtk::ScrolledWindow` with `hscrollbar-policy: never` containing one `ListBox` per group (CSS `boxed-list`) plus one ListBox for standalone items.
  - 12 px spacer between groups, none after the last group (T-E15).
  - Page content width is capped at 720 px and centred (ui-parity §9).

### T-47ba-006 — Base preference row (PrefRow)
- Priority: P0
- Effort: S
- Depends: T-47ba-008
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt, src/ui.rs
- Acceptance:
  - Row min height 56 px, 16 px horizontal and vertical padding, icon column with 16 px start margin and 8 px end margin, title ellipsised at max 2 lines at 16 px, subtitle at 78% opacity up to 10 lines (`p1-data-and-config.md §9`).
  - Row is activatable only when `on_click` is provided (T-E21).
  - Mockup `row` helper (`src/ui.rs:147`) is replaced by this builder for settings rows.

### T-47ba-007 — Group header label
- Priority: P0
- Effort: S
- Depends: T-47ba-008
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/PreferenceGroupHeader.kt
- Acceptance:
  - Header uses secondary colour, body-medium size, top margin 14 px and bottom 8 px, horizontal 16 px.
  - A blank group title renders no header but keeps the spacer (T-E44).

### T-47ba-008 — Constants and dp-to-CSS mapping for settings
- Priority: P0
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/PreferenceGroupHeader.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt, src/settings/widgets/mod.rs, resources/style.css
- Acceptance:
  - `src/settings/widgets/mod.rs` exports the constants in `p1-rust-design.md §9`; a unit test asserts each value.
  - `resources/style.css` defines `.pref-title`, `.pref-flash`, `.pref-flash-off`, `.boxed-list` rows with the values from `p1-data-and-config.md §9`.
  - No other file hard-codes these numbers.

### T-47ba-009 — Preference binding glue (Pref watch to widget and veto runner)
- Priority: P0
- Effort: M
- Depends: T-47ba-001, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, src/settings/render.rs
- Acceptance:
  - `Pref::watch()` changes update the bound widget on the GTK thread through `glib::spawn_future_local` without causing a write (T-E59).
  - Writes for the same preference are serialized: the last write wins (T-E19).
  - A veto that returns an error keeps the dialog open and shows an inline error (T-E18).

---

## More tab

### T-47ba-010 — More tab: nav entry and 14-row page
- Priority: P0
- Effort: M
- Depends: T-47ba-006, T-47ba-007, T-47ba-012, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt, app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt, src/more/mod.rs, src/screens.rs, spec/app.json
- Acceptance:
  - Sidebar entry "More" (label `label_more`) in `spec/app.json` `nav`; the page shows the 14 rows in the order of `p1-behavior.md §M.2`.
  - Titles and icons match `p1-behavior.md §M.2` and `p1-data-and-config.md §8`.
  - `src/screens.rs` `more()` (lines 336-352) is replaced by `more::page::build`.

### T-47ba-011 — More switches: Downloaded only and Incognito mode
- Priority: P0
- Effort: S
- Depends: T-47ba-048, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, src/more/mod.rs
- Acceptance:
  - "Downloaded only" writes `__APP_STATE_pref_downloaded_only` (default false) and "Incognito mode" writes `__APP_STATE_incognito_mode` (default false), no veto.
  - Both switches reflect changes made elsewhere (T-E58).
  - Subtitles are exactly "Filters all entries in your library" and "Pauses your history".

### T-47ba-012 — Logo header
- Priority: P0
- Effort: S
- Depends: T-47ba-049
- Sources: app/src/main/java/eu/kanade/presentation/more/LogoHeader.kt, src/more/mod.rs
- Acceptance:
  - 64 px icon with 56 px above and below, followed by a separator (T-E70).
  - Icon is decorative (no accessible name, not focusable).

### T-47ba-013 — Alt-nav row (moved tab)
- Priority: P0
- Effort: S
- Depends: T-47ba-010, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt, src/more/mod.rs
- Acceptance:
  - Row title and icon come from the stored `bottom_rail_nav_style` (default `MOVE_HISTORY_TO_MORE`, title "Move History to the More tab" is the setting label, row title is the moved tab's name).
  - Activating the row opens the moved tab's page.
  - The four `NavStyle` values map to their tab per `p1-data-and-config.md §3.3`.

### T-47ba-014 — More navigation targets and Help URL
- Priority: P0
- Effort: S
- Depends: T-47ba-010
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt, src/more/mod.rs
- Acceptance:
  - Categories, Statistics, Data and storage, Settings, Player settings, About route per `p1-behavior.md §M.5`.
  - Help opens `https://aniyomi.org/docs/guides/troubleshooting/` through `gtk::UriLauncher` (T-E60).
  - "Data and storage" opens the Data and storage settings page, not the storage tab (T-E71).

### T-47ba-015 — Download queue state: derive and combine
- Priority: P0
- Effort: M
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt, src/more/queue_state.rs
- Acceptance:
  - `queue_state::derive` matches the truth table in T-E54 (8 rows).
  - The combiner takes four `tokio::sync::watch` receivers (manga running, manga queue size, anime running, anime queue size) and publishes a `watch::Sender<DownloadQueueState>`; the final state equals `derive` of the final inputs (T-E57).
  - Starts as `Stopped` before the first update.

### T-47ba-016 — Download queue row subtitle and navigation
- Priority: P0
- Effort: S
- Depends: T-47ba-015, T-47ba-006, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt, src/more/queue_state.rs
- Acceptance:
  - Subtitle: none for Stopped; "Paused" for `Paused{0}`; "Paused • N remaining" for `Paused{N}`; "N remaining" for `Downloading{N}` (T-E53, T-E55, T-E56).
  - Row opens the download queue page (area:25).
  - The subtitle updates live without rebuilding the page.

### T-47ba-017 — Drop the unused storage parameter and route (drop record)
- Priority: P3
- Effort: S
- Depends: T-47ba-010
- Sources: app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt
- Acceptance:
  - Drop decision recorded: `onClickStorage` in Kotlin is never rendered; Kurobayomi has no storage route from More. The user loses nothing because "Data and storage" reaches the same screen group.
  - Test T-E71 asserts no More row routes to a storage tab.

---

## Search highlight

### T-47ba-020 — Highlight handoff and flash timers
- Priority: P0
- Effort: M
- Depends: T-47ba-006, T-47ba-008
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt, src/settings/highlight.rs, resources/style.css
- Acceptance:
  - `highlight::request` / `take` replace the global `SearchableSettings.highlightKey`; `take()` clears the request on every page build (T-E01, T-R04).
  - `flash(row)` runs: class on at 0 ms, 10 toggles from 600 ms at 200 ms steps, class off at 3000 ms (`p1-rust-design.md §6.4`).
  - Calling `flash` twice on one row cancels the first timers (P-flash-idempotent), verified by a test that counts toggles.

### T-47ba-021 — Highlight target lookup and reveal (scroll)
- Priority: P0
- Effort: M
- Depends: T-47ba-005, T-47ba-020
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt, src/settings/page.rs, src/settings/highlight.rs
- Acceptance:
  - Lookup map title to first built row, disabled groups excluded, disabled items included (T-E02, T-E07, T-E13, T-E14).
  - Reveal after 500 ms scrolls the target into view and then flashes it (T-E02 and T-E13).
  - Titles that are blank never match (T-E05, T-E17 for trackers using the tracker name).

### T-47ba-022 — Highlight for slider and tracker rows
- Priority: P1
- Effort: S
- Depends: T-47ba-021, T-47ba-052, T-47ba-050
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/TrackingPreferenceWidget.kt
- Acceptance:
  - Slider rows flash when targeted (T-E08). This is a deliberate divergence from Kotlin; record it in the parity notes.
  - Tracker rows flash by tracker name (T-E17).

---

## Update dialog

### T-47ba-030 — Update info screen layout
- Priority: P1
- Effort: M
- Depends: T-47ba-049, area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt, src/more/update_dialog.rs, src/screens.rs
- Acceptance:
  - Layout per `p1-ui-parity.md §3.5`: icon 48 px, heading "New version available!", subtitle = version, body, "Download" (suggested) and "Not now".
  - Body scrolls; buttons stay pinned (ui-parity §3.5).
  - Enter triggers Download, Escape triggers Not now (ui-parity §7).

### T-47ba-031 — Changelog cleanup, markdown rendering and link button
- Priority: P1
- Effort: S
- Depends: T-47ba-030
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt, app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt, src/more/update_dialog.rs
- Acceptance:
  - `strip_checksums` passes all seven vectors in `p1-edge-cases-and-tests.md §2` (T-E10).
  - Markdown renders headings, bullets, links (link colour from the theme) via `pulldown-cmark` to Pango markup; links open with `gtk::UriLauncher`.
  - "Open on GitHub" label (or the Proposal "Open release page") comes from the string table; the link button is hidden when the release link is empty (T-E65, T-E09).
  - Version subtitle comes from the update check, not a literal (T-E61).

### T-47ba-032 — Download action and double-click guard
- Priority: P1
- Effort: L
- Depends: T-47ba-030, area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt, src/more/update_dialog.rs
- Acceptance:
  - Download starts a tokio download task to `$XDG_DOWNLOAD_DIR/Kurobayomi/updates/<version>/` (fallback `~/Downloads/...`) and shows progress through `gio::Notification`, then reveals the file with `gtk::FileLauncher`.
  - The Download button disables on the first click (T-E63); the page pops immediately like Kotlin.
  - Offline start produces a failure notification "Could not download the update" (T-E62).

---

## Theme pickers

### T-47ba-040 — Theme mode selector (System, Light, Dark)
- Priority: P0
- Effort: S
- Depends: area:02-preferences-system, area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemeModePreferenceWidget.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, src/settings/widgets/theme_mode.rs
- Acceptance:
  - Three linked toggle buttons in order System, Light, Dark; stored value `pref_theme_mode_key` with enum names, default `SYSTEM` (T-E50 for equal writes).
  - SYSTEM passes `None` to `set_gtk_application_prefer_dark_theme`, LIGHT `Some(false)`, DARK `Some(true)` (T-E51).
  - Re-clicking the selected segment does not write.

### T-47ba-041 — App theme strip: preview cards
- Priority: P0
- Effort: M
- Depends: T-47ba-040, area:49-theme-and-design-system
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, src/settings/widgets/app_theme.rs
- Acceptance:
  - Cards are 114 px wide, 9:16 frame with border 4 px (primary when selected), 2-line label; 18 cards on Linux (MONET hidden, deprecated names hidden) per `p1-behavior.md §T.2`.
  - Card contents match the mini-preview layout (app bar, 2:3 cover, two chips, bottom bar) and use the theme's own palette.
  - Horizontal scroll, no wrapping (replaces the mockup FlowBox chips).

### T-47ba-042 — Live theme apply, amoled and deprecated-name mapping
- Priority: P0
- Effort: S
- Depends: T-47ba-041
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, src/theme/mod.rs
- Acceptance:
  - Clicking a card swaps the CSS provider live without restart (no `ActivityCompat.recreate` equivalent).
  - Stored `DARK_BLUE`, `HOT_PINK`, `BLUE`, `MONET` read as `DEFAULT` and written back (T-E48, T-E49).
  - Amoled applies only in dark mode (T-E52); key `pref_theme_dark_amoled_key`, default false.

---

## Edit, list, multi-select, switch, text, info, tracker, tri-state, slider

### T-47ba-043 — Single-line text edit dialog
- Priority: P0
- Effort: M
- Depends: T-47ba-006, T-47ba-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/EditTextPreferenceWidget.kt, src/settings/widgets/edit_text.rs
- Acceptance:
  - OK enabled only when text differs from the snapshot, is non-blank or canBeBlank, and validates (T-E32, T-E33).
  - Invalid or blank-not-allowed text shows the error icon in place of the clear button and the `errorMessage` as supporting text when one exists; blank-only shows no supporting text (T-E03, T-E31).
  - Dialog stays open when the veto returns false; Escape cancels; Enter submits when OK is enabled (T-E35 snapshot).

### T-47ba-044 — Multi-line, MPV config and info-variant editors
- Priority: P1
- Effort: M
- Depends: T-47ba-043, area:48-settings-player-and-scripts
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/EditTextPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, src/settings/widgets/edit_text.rs
- Acceptance:
  - Multi-line uses `gtk::TextView`; Enter inserts newline, Ctrl+Enter submits (T-E34).
  - MPV config subtitle shows the first two lines and `\n...` when more than two (T-E36); the file is written under `$XDG_CONFIG_HOME/kurobayomi/mpv/<fileName>` and the preference is updated only after a successful write (T-E06, proposal replaces Android permission check).
  - Info-variant honours the item's `canBeBlank` instead of hard-coding true (T-E37).

### T-47ba-045 — Info row
- Priority: P0
- Effort: S
- Depends: T-47ba-008
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/InfoWidget.kt, src/settings/widgets/info.rs
- Acceptance:
  - Info icon above body text at 78% opacity, padding 16 px all round horizontally and vertically (behavior §B.9).
  - Not activatable, not highlighted; blank text row is skipped (T-E43).

### T-47ba-046 — Single-choice list dialog
- Priority: P0
- Effort: M
- Depends: T-47ba-006, T-47ba-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/ListPreferenceWidget.kt, src/settings/widgets/list.rs, src/ui.rs
- Acceptance:
  - Radio list dialog with Cancel only; selecting a different entry applies and closes; re-selecting the current entry does nothing and keeps the dialog open (T-E24).
  - Scroll dividers above and below when the list can scroll; max height 60% of the window (ui-parity §3.1).
  - Replaces the mockup `gtk::DropDown` in `src/ui.rs:231-239` (gap G-01).

### T-47ba-047 — Multi-select dialog
- Priority: P0
- Effort: M
- Depends: T-47ba-006, T-47ba-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/MultiSelectListPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt, src/settings/widgets/multi_select.rs, src/ui.rs
- Acceptance:
  - Checkbox list in entries order; OK writes the selection, Cancel writes nothing (T-E30 snapshot).
  - Subtitle lists labels in entries order, "None" when empty (T-E12, T-E28).
  - Unknown stored keys are dropped on OK (T-E29); the proposal note in the parity file records this for review before release.
  - Replaces the mockup MenuButton popover in `src/ui.rs:240-250` (gap G-02).

### T-47ba-048 — Switch row
- Priority: P0
- Effort: S
- Depends: T-47ba-006, T-47ba-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/SwitchPreferenceWidget.kt, src/ui.rs
- Acceptance:
  - Whole row toggles; the `gtk::Switch` is not a separate target (`set_can_target(false)`).
  - Veto false leaves the switch unchanged (T-E20); Space or Enter on the focused row toggles.
  - Mockup switch (`src/ui.rs:223-230`) is kept but routed through this builder.

### T-47ba-049 — Text row, icon assets, and icon-name verification
- Priority: P0
- Effort: S
- Depends: T-47ba-006
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/TextPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt, src/settings/widgets/text.rs, resources/icons
- Acceptance:
  - Text row shows title, subtitle (hidden when blank, T-E22), icon tinted primary, optional trailing widget; activatable only with on_click.
  - Release gate: every icon name in `p1-data-and-config.md §8` exists in the installed theme (checked with `gtk4-icon-browser` or by a script that reads the icon theme index); missing names are replaced by bundled SVGs under `resources/icons`.
  - Glasses icon (`ic_glasses_24dp`) and the app logo (`ic_ani`) are bundled as SVG.

### T-47ba-050 — Tracker login row
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, T-47ba-006
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/TrackingPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, src/settings/widgets/tracker.rs
- Acceptance:
  - Row shows tracker logo, tracker name (one line), and a green check (`#4CAF50`, 32 px, accessible name "Logged in") when logged in.
  - Initial state correct without flicker (T-E41); click calls login or logout.
  - Proposal: logout asks for confirmation (T-E42); record as a divergence from Kotlin.

### T-47ba-051 — Tri-state include and exclude dialog
- Priority: P1
- Effort: M
- Depends: T-47ba-006, T-47ba-009
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/TriStateListDialog.kt, src/settings/widgets/tri_state.rs, src/ui.rs
- Acceptance:
  - Cycle unchecked, checked, excluded, unchecked (T-E46 duplicates cycle independently); checked wins over excluded when both lists contain an item (T-E45).
  - Accessible names differ per state: "Not included", "Included", "Excluded" (T-E04, Proposal fix of the Kotlin shared "Disabled" label).
  - Cancel and OK; OK returns `(included, excluded)` in original order and does not auto-close (T-E47). Space cycles focused row (Proposal).

### T-47ba-052 — Slider row with value pill
- Priority: P0
- Effort: M
- Depends: T-47ba-006
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, src/settings/widgets/slider.rs, src/ui.rs
- Acceptance:
  - Label left, value pill right (subtitle if non-empty, else the number), full-width `gtk::Scale` with `steps` from the model (T-E40).
  - Values are clamped to the range before display and before write (T-E38); equal values do not call the veto (T-E39).
  - Caller owns the value: the row re-renders only from the caller's new value.
  - Replaces the mockup slider (`src/ui.rs:251-260`, gap G-03) and uses 16/16 padding, not the `SettingsItemsPaddings` 24/10 (ui-parity §2.6).

---

## Onboarding wrapper

### T-47ba-060 — Onboarding gate: back-blocking and finish
- Priority: P0
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview, area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, src/onboarding/mod.rs, src/screens.rs
- Acceptance:
  - Close request is stopped (`glib::Propagation::Stop`) while `__APP_STATE_onboarding_complete` is false (T-E66).
  - Finish sets the flag to true and pops the page (T-E67).
  - Mockup `info_screen("onboarding")` (`src/screens.rs:492-535`) uses the gate.

### T-47ba-061 — Onboarding restore path with Data-and-storage highlight
- Priority: P0
- Effort: S
- Depends: T-47ba-060, T-47ba-020, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, src/onboarding/mod.rs
- Acceptance:
  - Restore finishes onboarding, calls `highlight::request("Backup and restore")` (the string from the shared constant, T-E68) and opens Data and storage.
  - The highlight request is set before the page is pushed (R-03), so the flash happens once.

---

## Strings, drops, tests and polish

### T-47ba-070 — Unit tests for the pure logic
- Priority: P0
- Effort: M
- Depends: T-47ba-001, T-47ba-015, T-47ba-031, T-47ba-008
- Sources: src/settings/model.rs, src/more/queue_state.rs, src/more/update_dialog.rs, tests/settings_model.rs, tests/queue_state.rs, tests/changelog.rs
- Acceptance:
  - `cargo test` runs every unit test in the "Unit tests" table of `p1-edge-cases-and-tests.md §5` (T-E10, T-E12, T-E16, T-E22, T-E23, T-E28, T-E29, T-E36, T-E38, T-E40, T-E45, T-E46, T-E48, T-E49, T-E50, T-E51, T-E53, T-E54, T-E55, T-E56, T-E60, T-E61, T-E64).
  - Fixtures from `p1-edge-cases-and-tests.md §6` are checked in under `tests/fixtures/`.

### T-47ba-071 — GTK integration test harness
- Priority: P1
- Effort: M
- Depends: T-47ba-070, T-47ba-005
- Sources: tests/gtk_settings.rs, src/settings/page.rs
- Acceptance:
  - Tests marked `#[ignore]` run only with `KUROBAYOMI_GTK_TESTS=1` (under `xvfb-run` or a headless Wayland compositor).
  - The GTK test list in `p1-edge-cases-and-tests.md §5` (T-E02 through T-R08) is implemented or explicitly marked as manual in the file.

### T-47ba-072 — Screenshot checks for the scope pages
- Priority: P2
- Effort: S
- Depends: T-47ba-010, T-47ba-041, T-47ba-052
- Sources: src/main.rs, src/settings/page.rs
- Acceptance:
  - `KUROBAYOMI_SHOT=more` and `KUROBAYOMI_SHOT=settings_appearance` produce PNGs at 1180x780 and 720x900 for the manual checks in `p1-edge-cases-and-tests.md §5`.

### T-47ba-073 — English strings from the shared string table only
- Priority: P0
- Effort: S
- Depends: area:51-i18n
- Sources: i18n/src/commonMain/moko-resources/base/strings.xml, i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml, src/settings/widgets/mod.rs
- Acceptance:
  - Every key in `p1-behavior.md §X.1` resolves from the string table at build time; no duplicated English literal exists in scope code except the Proposal strings in T-47ba-074.
  - The download-queue plural `%1$s remaining` is one format string with a `{n}` placeholder.

### T-47ba-074 — Proposal strings (not in the Kotlin tree)
- Priority: P2
- Effort: S
- Depends: T-47ba-073, area:51-i18n
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/TriStateListDialog.kt, src/settings/widgets/tri_state.rs
- Acceptance:
  - Added to the string table and marked as Kurobayomi-only: "Not included", "Included", "Excluded", "This field can't be empty", "Could not save this setting", "Open release page", "Could not download the update", "Log out of this tracker?".
  - Each is referenced by one task above and nowhere else.

### T-47ba-075 — Drop record for Android-only mechanisms (P3)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt, app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt
- Acceptance:
  - Dropped with reasons and what the user loses: haptic feedback on slider steps (no equivalent; user loses tactile tick on touch devices only); `ActivityCompat.recreate` on theme change (replaced by live CSS swap, user loses nothing); Compose previews (dev tooling); `anim_more_enter` animated tab icon (static icon, user loses the decorative animation); MONET dynamic colour (Android 12+ wallpaper colour; Linux uses the GTK theme and the Kurobayomi palettes instead, user loses wallpaper-derived colours); `Environment.isExternalStorageManager` gate (Android storage permission, replaced by XDG path).
  - Each dropped item is listed in `p1-ui-parity.md` or `p1-behavior.md` with the same reason.

### T-47ba-076 — Responsive settings layout
- Priority: P2
- Effort: S
- Depends: T-47ba-005
- Sources: src/settings/page.rs, resources/style.css
- Acceptance:
  - Content max width 720 px, 16 px gutter below that width, no horizontal scroll at 360 px window width (the minimum width of area:36).
  - Dialogs 360 px wide and 80% max height of the window.

### T-47ba-077 — Keyboard and focus behaviour for settings rows and dialogs
- Priority: P1
- Effort: S
- Depends: T-47ba-046, T-47ba-047, T-47ba-043, T-47ba-051
- Sources: src/settings/widgets/mod.rs, src/settings/widgets/list.rs, src/settings/widgets/multi_select.rs, src/settings/widgets/edit_text.rs
- Acceptance:
  - Keyboard map of `p1-ui-parity.md §7` works for every control (Tab order, Space, Enter, Escape, arrows).
  - Escape in any dialog is Cancel; the default button for the multi-select is OK.

### T-47ba-078 — Accessibility labels and tooltips
- Priority: P2
- Effort: S
- Depends: T-47ba-048, T-47ba-049, T-47ba-051
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/widget/TrackingPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/AppThemePreferenceWidget.kt
- Acceptance:
  - Every icon-only control has a tooltip and accessible label from the string table (for example "Logged in", "Selected").
  - Decorative icons (logo, row icons) are hidden from assistive technology.

### T-47ba-079 — Preference key compatibility and value migration
- Priority: P1
- Effort: S
- Depends: area:02-preferences-system, T-47ba-042, T-47ba-013
- Sources: app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, src/settings/widgets/mod.rs
- Acceptance:
  - Keys from `p1-data-and-config.md §2` are used verbatim (including the `__APP_STATE_` prefix for the three app-state flags).
  - Stored enum names are read with fallbacks (deprecated `AppTheme` names to `DEFAULT`, unknown `NavStyle` to `MOVE_HISTORY_TO_MORE`), covered by tests T-E48 and T-E49.

### T-47ba-080 — Keyboard shortcuts for More and settings pages
- Priority: P2
- Effort: S
- Depends: T-47ba-010, T-47ba-005
- Sources: src/main.rs, src/more/mod.rs
- Acceptance:
  - No new global shortcuts are added in this area; Escape and Alt+Left back navigation (already in `src/main.rs:78-79`) work on pages in this area.
