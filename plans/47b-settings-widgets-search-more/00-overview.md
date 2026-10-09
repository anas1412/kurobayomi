# Area 47b: settings widgets, search highlight, More tab, update dialog, onboarding wrapper, overview

Paths in this file are repo-relative to `/home/blackbox/aniyomi-src` (Kotlin reference) unless they start with `src/`, `spec/` or `resources/`, which are in `/home/blackbox/kurobayomi` (Rust target). Labels: OBSERVED = what the Kotlin does; Proposal = what Kurobayomi should do.

Area id: `47b-settings-widgets-search-more`. Task prefix: `T-47ba-`. Part docs: the nine `p1-*.md` files in this folder (see section 3).

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

This area is the shared UI machinery under Aniyomi's settings and the "More" tab. It has five parts:

1. **Settings model and page scaffold.** A declarative tree (`sealed class Preference`, `PreferenceGroup`, `PreferenceItem<T>`) in `app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`. A page is a title plus a list of groups and standalone items (`PreferenceScaffold.kt`, `PreferenceScreen.kt`). Each item variant is dispatched to one widget in `PreferenceItem.kt`.
2. **Twelve row and dialog widgets** in `app/src/main/java/eu/kanade/presentation/more/settings/widget/`: base row, text, switch, single-choice list dialog, multi-select dialog, text-edit dialog, info block, group header, tri-state include/exclude dialog, tracker login row, theme mode selector, theme card strip. A slider wrapper (`BaseSliderItem`) lives in presentation-core and is used from `PreferenceItem.kt`.
3. **Search-jump highlight.** A global `SearchableSettings.highlightKey` (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt:41`) names the row to reveal. `PreferenceScreen` scrolls to it after 0.5 s and the row pulses for 3 s (`BasePreferenceWidget.kt:89-120`).
4. **More tab.** `app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt` registers the tab (`index = 4u`, `MoreTab.kt:51`) and the download-queue combiner (`MoreTab.kt:88-132`). `app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt` renders 14 rows in fixed order (see [p1-behavior.md](p1-behavior.md) section M.2).
5. **Update info screen and onboarding wrapper.** `NewUpdateScreen` (presentation and `ui/more`) shows the new version, a cleaned changelog and Download / Not now. `OnboardingScreen` (`app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt`) blocks back navigation until onboarding is finished and can open the backup-restore row with a highlight.

Scope size: 22 files, 2364 LOC (`p1-inventory.md`). Every file is in the inventory table with its task IDs. Coverage was re-checked with `_tools/areas.py` and all 22 paths appear in `p1-inventory.md`.

### 1.2 Why it matters for a usable Kurobayomi (Proposal)

- Every settings page in Kurobayomi (areas 47 and 48, and the spec-driven pages of area 02) renders through the model, row builders and dialogs in this area. Without them no preference can be changed from the UI.
- The More tab is the only entry point to Categories, Statistics, Data and storage, Settings, Player settings, About and Help on a small layout. On the desktop it is one of the sidebar entries (`spec/app.json` `nav`).
- The onboarding gate decides whether a first-run user can reach the app's main screens and restore a backup.
- The update dialog is the only update UI. Linux cannot self-install a package, so this screen only starts a download and reveals the file (area:35 owns the download).
- The search-jump highlight is what makes the settings search usable. Without it a search hit lands on a page with no visible target.

### 1.3 Scope boundaries

Owned by this area: the settings model (`Preference`), `PreferenceItem` dispatch, `PreferenceScaffold` and `PreferenceScreen` behaviour, the 12 widgets, the highlight handoff and flash, the More tab rows and queue state, the update info screen, the onboarding wrapper and the 22 inventory files.

Owned elsewhere (do not implement here):

| Concern | Owner | Note |
|---|---|---|
| Preference store, `Pref<T>` handles, app-state key prefix | area:02-preferences-system | This area consumes `Pref<T>` (`p1-rust-design.md` section 4.1). |
| Settings screen contents, `SearchableSettings` trait, search index and search screen | area:47-settings-screens-general | See open question Q6 (ownership is written two ways). |
| Player settings and MPV config file names and paths | area:48-settings-player-and-scripts | This area only provides the file-edit dialog. |
| Theme palettes, `theme::apply`, CSS, logo and icon assets | area:49-theme-and-design-system, area:49b-android-res-values-drawables | Theme strip reads palettes from area:49. |
| Shell, navigation, onboarding gate in `MainActivity`, minimum window width | area:36-ui-shell-navigation-security-webview | More tab and onboarding are registered there. |
| Download queue producers (`isDownloaderRunning`, queue size) | area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared | |
| Update check, release link, download job, notification | area:35-data-misc-updater-notification-export | |
| Tracker login and logout flow | area:27-tracking-core | Tracker logos come from the per-tracker areas area:28-tracker-anilist through area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin. |
| Strings table and plural rules | area:51-i18n | |

---

## 2. Module map

### 2.1 Kotlin packages and responsibilities (OBSERVED)

| Package or path (repo-relative) | Files in scope | Responsibility |
|---|---|---|
| `app/src/main/java/eu/kanade/presentation/more/` | `LogoHeader.kt` (34 LOC), `MoreScreen.kt` (177), `NewUpdateScreen.kt` (88) | More tab page, logo header, update info screen (view). |
| `app/src/main/java/eu/kanade/presentation/more/settings/` | `Preference.kt` (261), `PreferenceItem.kt` (237), `PreferenceScaffold.kt` (33), `PreferenceScreen.kt` (97) | Settings model, per-variant dispatch, page scaffold, group and highlight rendering. |
| `app/src/main/java/eu/kanade/presentation/more/settings/widget/` | 12 files, 1 204 LOC in total | Row, dialog and inline widgets (list of files in section 2.2). |
| `app/src/main/java/eu/kanade/tachiyomi/ui/more/` | `MoreTab.kt` (138), `NewUpdateScreen.kt` (43), `OnboardingScreen.kt` (52) | Tab registration, `MoreScreenModel` with queue combiner, update screen controller, onboarding controller. |
| `app/src/main/java/eu/kanade/presentation/more/settings/screen/` (area:47) | `SearchableSettings.kt` (holds `highlightKey`) | Read by `PreferenceScreen`; written by search and onboarding. Not in this area's file list (see Q6). |
| `presentation-core/src/main/java/tachiyomi/presentation/core/components/` (external) | `SettingsItems.kt` (`BaseSliderItem`), `LabeledCheckbox.kt`, `LazyList.kt`, `material/Constants.kt` | Shared slider, checkbox row, scrollbar list and spacing tokens used by the widgets. Constants are copied into Rust constants (`p1-data-and-config.md` section 9). |

### 2.2 Rust crate and modules (Proposal, from [p1-rust-design.md](p1-rust-design.md) section 1)

Crate: `gtk4` 0.11.5 with feature `v4_16`, no libadwaita. Runs on the GTK main thread. Background work comes from tokio (area:36 owns the runtime).

```
src/
  settings/
    mod.rs            pub use; SettingsPage registry (ids from spec/settings.json)
    model.rs          Preference, PreferenceItem, PreferenceGroup (replaces Preference.kt)
    page.rs           page builder: header + ScrolledWindow + ListBox per group (replaces PreferenceScaffold.kt, PreferenceScreen.kt)
    render.rs         PreferenceItem -> gtk::Widget dispatch (replaces PreferenceItem.kt)
    highlight.rs      HighlightRequest handoff + flash timers (replaces SearchableSettings.highlightKey and the flash in BasePreferenceWidget.kt)
    widgets/
      mod.rs          constants, DialogSpec + show_dialog helper (rust-design section 9 and 3.1)
      row.rs          PrefRow base (BasePreferenceWidget.kt)
      text.rs         TextRow (TextPreferenceWidget.kt)
      switch.rs       SwitchRow (SwitchPreferenceWidget.kt)
      list.rs         single-choice dialog (ListPreferenceWidget.kt)
      multi_select.rs multi-select dialog (MultiSelectListPreferenceWidget.kt)
      edit_text.rs    text-edit dialog and variants (EditTextPreferenceWidget.kt)
      slider.rs       SliderRow (BaseSliderItem wrapper)
      tracker.rs      TrackerRow (TrackingPreferenceWidget.kt)
      info.rs         InfoRow (InfoWidget.kt)
      group.rs        GroupHeader (PreferenceGroupHeader.kt)
      tri_state.rs    include/exclude dialog (TriStateListDialog.kt)
      theme_mode.rs   ThemeModeRow (AppThemeModePreferenceWidget.kt)
      app_theme.rs    AppThemeStrip (AppThemePreferenceWidget.kt)
  more/
    mod.rs            MoreTab nav entry + page builder (MoreTab.kt, MoreScreen.kt, LogoHeader.kt)
    queue_state.rs    DownloadQueueState + pure derive() + subtitle() (MoreScreenModel combiner)
    update_dialog.rs  UpdateDialog view + controller + strip_checksums (both NewUpdateScreen.kt files)
  onboarding/
    mod.rs            back-blocking gate, finish, restore-highlight handoff (ui/more/OnboardingScreen.kt)
tests/
  settings_model.rs, queue_state.rs, changelog.rs   pure unit tests
  gtk_settings.rs                                   GTK tests, #[ignore] unless KUROBAYOMI_GTK_TESTS=1
resources/style.css                                  .pref-title, .pref-flash, .pref-flash-off, .boxed-list
```

Mockup files that change (from `p1-rust-design.md` section 15): `src/settings.rs` moves to `src/settings/mod.rs` (T-47ba-002); `src/ui.rs` `item_row` (line 221) dispatches into `settings::render`; `src/screens.rs` `more()` (lines 336-352) becomes `more::page::build`; `src/screens.rs` `info_screen("onboarding")` (lines 492-535) and `info_screen("new_update")` (lines 497-498) move to `onboarding` and `more::update_dialog`.

---

## 3. Reading guide

| Document | What it contains |
|---|---|
| [p1-inventory.md](p1-inventory.md) | All 22 in-scope Kotlin files with LOC, every declaration, the port decision (Port, Rewrite, Replace-with-crate, Drop), target Rust module, priority and task IDs. |
| [p1-behavior.md](p1-behavior.md) | Full observed behaviour with `path:line` citations, sections A (model and dispatch), B (B.1-B.14 widgets), C (highlight), T (theme pickers), M (More tab), U (update dialog), O (onboarding), L (logo), X (strings, errors, timings). |
| [p1-data-and-config.md](p1-data-and-config.md) | Field-level model table, preference keys with exact defaults, enum values and declaration order, file and endpoint touch points, Android-to-GTK icon map, layout constants in dp. |
| [p1-ui-parity.md](p1-ui-parity.md) | Every visible element, state, dialog, keyboard and mouse gesture per widget, the 14-row More layout, mockup gap list G-01 to G-18, Kotlin defects not to copy. |
| [p1-rust-design.md](p1-rust-design.md) | Rust types (`Preference`, `PreferenceItem`, `Veto`), dialog helper, highlight timers, queue `derive`, the constant table, Android-to-Linux API map, crates, the changelog strip function, pitfalls P-index to P-list-reselect. |
| [p1-topic-widget-state-machines.md](p1-topic-widget-state-machines.md) | One state machine per interactive widget and dialog (preference write, switch, text, list, multi-select, edit text, tri-state, slider, tracker, theme controls, update screen, onboarding). |
| [p1-topic-highlight-and-queue-state.md](p1-topic-highlight-and-queue-state.md) | The search-jump handoff timeline (Kotlin and Rust) and the download-queue derivation and subtitle rules. |
| [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) | Edge cases E-01 to E-71, races R-01 to R-08, failure modes F-01 to F-05, the unit and GTK test catalogue (the T-E test IDs, 69 in total, plus the race tests T-R03 to T-R08), fixtures, coverage map from behaviour sections to tests. |
| [p1-tasks.md](p1-tasks.md) | 49 implementation tasks, `T-47ba-001` to `T-47ba-080` (31 numbers are unused, section 6.1). Lint passes with `tasks.py lint`. |
| `../TASKS.csv` (registry export) | Machine-readable rows for all tasks in the plan set. The 49 rows for this area have `file` starting with `47b-settings-widgets-search-more/`. |

---

## 4. Key design decisions and open questions

### 4.1 Decided in the part docs (kept as written)

- D1. Settings pages are data. A Rust enum tree (`settings::model`) is rendered by one builder per widget kind. No stringly typed widgets (`p1-rust-design.md` section 0.1).
- D2. Preference bindings are typed handles `Pref<T>` from area:02. Widgets never touch the store directly. Writes go through a veto future: the value is written only when the veto returns true (`p1-behavior.md` A.5).
- D3. Dialogs are hand-built transient, modal `gtk::Window`s with one shared helper `show_dialog` (fixed width 360 px, not resizable, Escape = Cancel, confirm disabled by a predicate). `gtk::AlertDialog` is not used in this area.
- D4. Single-choice settings use a dialog with radio rows, not an inline `gtk::DropDown`. The Kotlin dialog closes only on a different selection (E-24). The mockup's DropDown is replaced (gap G-01).
- D5. The highlight flash uses `glib::timeout_add_local` timers and a CSS class with a CSS `transition`. GTK 4 CSS has no `@keyframes`. Timeline: 600 ms delay, 10 toggles at 200 ms, cleared at 3000 ms (`p1-rust-design.md` section 6.4).
- D6. The highlight lookup uses widget lookup (title to first built row), not flat indices. This removes the index bug E-07.
- D7. Everything in this area runs on the GTK main thread. Background work is tokio; UI updates arrive through `tokio::sync::watch` consumed with `glib::spawn_future_local` (`p1-rust-design.md` section 4.2).
- D8. The changelog is rendered from markdown to Pango markup with `pulldown-cmark`. No WebView.
- D9. Stored enum values are enum names (`ThemeMode`, `AppTheme`, `NavStyle`). Deprecated theme names read as `DEFAULT` (E-48).
- D10. Preference keys are kept byte-for-byte (`__APP_STATE_pref_downloaded_only`, `__APP_STATE_incognito_mode`, `__APP_STATE_onboarding_complete`, `pref_theme_mode_key`, `pref_app_theme`, `pref_theme_dark_amoled_key`, `bottom_rail_nav_style`), so a shared config stays readable (`p1-data-and-config.md` section 2).
- D11. The download-queue state is recomputed from the latest four values with one pure function `derive` (T-E54 truth table). The Kotlin nested `collectLatest` is replaced by a single select over four watch receivers.
- D12. The changelog cleanup keeps Kotlin behaviour: everything from the first `---` that has `Checksums` later in the text is removed (`p1-rust-design.md` section 12, vectors in `p1-edge-cases-and-tests.md` section 2).
- D13. Onboarding back-blocking uses `gtk::Window::connect_close_request` returning `glib::Propagation::Stop` until the completion flag is set (`p1-rust-design.md` section 13).

### 4.2 Open questions for the solo developer (each with a recommendation)

- **Q1. Help link target.** T-47ba-014 opens `https://aniyomi.org/docs/guides/troubleshooting/` (the Aniyomi docs, `core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt:4`). Kurobayomi is a different app. Recommendation: keep one constant `HELP_URL` in `src/settings/widgets/mod.rs` and point it at the Kurobayomi project page until Kurobayomi has docs. Decision needed because it changes what the Help row says it does.
- **Q2. Unknown stored multi-select keys on OK.** Kotlin rebuilds the set from entries it knows, so a stored key that no longer exists is silently deleted when the user presses OK, even with no change (E-29, `p1-behavior.md` B.5). Recommendation: preserve unknown keys and toggle only known entries. This departs from parity; T-47ba-047 currently records parity for review.
- **Q3. Tracker logout confirmation.** Kotlin logs out on the first click (B.8). Recommendation: confirm first (T-47ba-050, E-42), because logout discards the session token. Record as a divergence.
- **Q4. Slider highlight.** Kotlin does not flash a targeted slider (C.4). Recommendation: flash it (T-47ba-022). Record as a divergence.
- **Q5. Highlight key scheme.** Area 47b keys on title only and flashes the first match (E-02). Area 47 proposes including the group title in the key (area 47's `p1-edge-cases-and-tests.md` line 34, for the duplicate title "Crop borders"). Recommendation: request a key of (group title, item title), with the item title alone as fallback. Needs agreement with area:47 before T-47ba-020 is started.
- **Q6. Ownership of `SearchableSettings` and the search screen.** Area 47b's inventory says these belong to area:47 (`p1-inventory.md`, intro). Area 48 says area:47b owns `SearchableSettings`, `PreferenceScaffold` and `SettingsSearchScreen` (`48-settings-player-and-scripts/p1-behavior.md` line 460). Recommendation: area:47 owns the search screen, search index and the `SearchableSettings` trait; area:47b owns `PreferenceScaffold` / `page.rs` and the highlight handoff. Area 48's prose must be corrected.
- **Q7. Icon strategy.** Every icon is a GTK symbolic name looked up at runtime (`p1-data-and-config.md` section 8). A distro icon theme may lack some names. Recommendation: bundle the roughly 25 glyphs used here under `resources/icons` and make the release gate T-47ba-049 check the bundle, not the system theme.
- **Q8. Error text for failed saves.** `p1-rust-design.md` section 4.3 and T-47ba-074 say "Could not save this setting". `p1-behavior.md` X.3 says "Something went wrong". Recommendation: "Could not save this setting".
- **Q9. Logout confirmation wording.** T-47ba-074 says "Log out of this tracker?" and `p1-ui-parity.md` section 12 says "Log out of {tracker}?". Recommendation: the `{tracker}` form, since it names what is being logged out.
- **Q10. Dialog height cap.** T-47ba-046 says list dialogs use max 60% of the window height. T-47ba-076 and `p1-ui-parity.md` section 9 say 80%. Recommendation: 80% for all dialogs, and fix T-47ba-046.
- **Q11. Enum variant count.** T-47ba-001 and T-47ba-003 require 13 `PreferenceItem` variants. `p1-rust-design.md` section 2 defines 11 (MultiLine and Info-variant EditText are flags on `EditText`). Recommendation: 11 variants, with an `EditTextStyle` flag set. Fix the count in T-47ba-001 and T-47ba-003 to 11.
- **Q12. MPV write failure.** Kotlin sets the preference even when the file write fails (B.14, E-06). Recommendation: write first, set the preference only on success, and show an error label on failure (T-47ba-044).
- **Q13. Theme strip on Linux.** 18 cards (MONET hidden, deprecated names hidden). Recommendation: keep 18.
- **Q14. Update download location.** `$XDG_DOWNLOAD_DIR/Kurobayomi/updates/<version>/` with `~/Downloads` fallback (`p1-data-and-config.md` section 5). No self-install. Recommendation: keep, and confirm with area:35.
- **Q15. Mockup History row.** The mockup hides the History link on the More page (`src/screens.rs:344-347`). Recommendation: hide it only while History is a sidebar entry, derived from `bottom_rail_nav_style` (`p1-rust-design.md` section 8.2).

---

## 5. Cross-area dependencies

### 5.1 This area needs

| From | What is needed | Used by |
|---|---|---|
| area:02-preferences-system | `Pref<T>` with `get`, `set`, `watch` (`p1-rust-design.md` section 4.1, signature to be agreed). Enum storage by name. Boolean and string keys with defaults. The app-state key prefix. | T-47ba-001, 009, 011, 040, 042, 060, 079 |
| area:36-ui-shell-navigation-security-webview | Sidebar nav entry for More, header bar for pages, window close-request hook for onboarding, `Alt+Left` and Escape back navigation, 360 px minimum window width, onboarding gate in `MainActivity` (`MainActivity.kt:407-412`). | T-47ba-005, 010, 013, 060, 080 |
| area:47-settings-screens-general | Spec items (`spec/settings.json`) and the `body(id)` router, `SettingsDataScreen` restore row title "Backup and restore" as the highlight target, search requests. | T-47ba-002, 061 |
| area:48-settings-player-and-scripts | The MPV config file name and directory rule for the file-edit dialog. | T-47ba-044 |
| area:49-theme-and-design-system | `theme::palette(AppTheme)` for each preview card, `theme::apply(theme, amoled)` for live swap, CSS file and tokens, `ItemCover` ratio 2/3. | T-47ba-040, 041, 042 |
| area:49b-android-res-values-drawables | Logo SVG (`ic_ani`) and glasses SVG (`ic_glasses_24dp`) assets. | T-47ba-012, 049 |
| area:51-i18n | Every English string in `p1-behavior.md` section X.1, plurals for "N remaining", and the Kurobayomi-only proposal strings in T-47ba-074. | T-47ba-073, 074 |
| area:23-downloads-manga, area:24-downloads-anime, area:25-downloads-shared | `isDownloaderRunning` and queue size as `tokio::sync::watch` receivers (four inputs). | T-47ba-015, 016 |
| area:27-tracking-core | `Tracker.is_logged_in` (initial value), `is_logged_in` change stream, login and logout actions, tracker name and logo. | T-47ba-050, 022 |
| area:35-data-misc-updater-notification-export | Update check result (version name, changelog, release link, download link), download job with progress and notification, error reporting. | T-47ba-030, 031, 032 |

### 5.2 Other areas need from this area

| To | What they need | Where |
|---|---|---|
| area:47-settings-screens-general | Model types, `PrefRow` and dialog builders, `page::build`, `highlight::request` (the search jump). | T-47ba-001, 005, 020, 021 |
| area:48-settings-player-and-scripts | Same model, widgets and the file-edit dialog. Area 48 lists area:47b as a dependency of its tasks (`48-settings-player-and-scripts/p1-tasks.md` lines 102, 113, 579). | T-47ba-003, 043, 044 |
| area:35-data-misc-updater-notification-export | Update info screen, which area 35 lists as depending on area:47b (`35-data-misc-updater-notification-export/p1-tasks.md` line 137). | T-47ba-030, 032 |
| area:36-ui-shell-navigation-security-webview | More tab page, onboarding gate type, the close-request behaviour. | T-47ba-010, 060 |
| area:37-ui-library, area:40-ui-browse-manga | Tri-state include/exclude dialog for filters. | T-47ba-051 |
| area:38-ui-entry-details-manga, area:39-ui-entry-details-anime | `PREF_MIN_HEIGHT` (56) and the dialog helper. The Kotlin duplicate dialogs read `LocalPreferenceMinHeight` (`p1-inventory.md`, per-file notes). | T-47ba-008, 043 |
| area:42-ui-updates-history-stats-storage-upcoming | Statistics row target (the More row opens the stats page). | T-47ba-014 |
| area:02-preferences-system | Its tasks T-02a-030 and T-02a-037 cite area:47b for settings item rendering (`02-preferences-system/00-overview.md` line 151). | T-47ba-001, 003 |

---

## 6. Task summary

### 6.1 Counts (from `TASKS.csv`, rows whose file starts with `47b-settings-widgets-search-more/`)

49 tasks in total.

| Priority | Tasks | Effort S | Effort M | Effort L | Effort XL |
|---|---|---|---|---|---|
| P0 | 32 | 18 | 14 | 0 | 0 |
| P1 | 10 | 4 | 5 | 1 | 0 |
| P2 | 5 | 5 | 0 | 0 | 0 |
| P3 | 2 | 2 | 0 | 0 | 0 |
| **Total** | **49** | **29** | **19** | **1** | **0** |

P1 tasks: T-47ba-022, 030, 031, 032, 044, 050, 051, 071, 077, 079. P2 tasks: T-47ba-072, 074, 076, 078, 080. P3 tasks (drop records): T-47ba-017, 075.

ID numbering: IDs run from 001 to 080, but only 49 are defined. Unused numbers: 018, 019, 023-029, 033-039, 053-059, 062-069. This is not a lint error. Do not reuse these numbers for new tasks without checking the other part docs first.

Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint` on `p1-tasks.md` prints `ok`. The registry export reports 0 errors across all 3801 tasks in the plan set. Every dependency token that starts with `area:` in this area's task file is a full area id that exists as a folder.

### 6.2 P0 critical path (in dependency order)

Lane A, settings core (longest intra-area P0 chain, 6 tasks): T-47ba-008 (constants) → T-47ba-006 (base row) → T-47ba-049 (text row, icon names) → T-47ba-012 (logo header) → T-47ba-010 (More page) → T-47ba-013 (alt-nav row).

Lane B, settings rendering: T-47ba-001 (model, needs area:02) and T-47ba-009 (pref binding glue, needs area:02) → T-47ba-043 (text-edit dialog) → T-47ba-003 (dispatch of every variant) → T-47ba-004 (disabled items). In parallel: T-47ba-005 (page scaffold, needs T-47ba-001, T-47ba-007) → T-47ba-020 (highlight handoff and flash) → T-47ba-021 (target lookup and reveal) → T-47ba-061 (onboarding restore highlight, needs T-47ba-060). Lane B is the sequence that gets a settings page with working search jump on screen.

Lane C, More queue: T-47ba-015 (queue derive and combiner, needs area:23 and area:24) → T-47ba-016 (queue subtitle and navigation, needs area:25) → T-47ba-011 (switches, needs T-47ba-048).

Lane D, theme: T-47ba-040 (mode selector) → T-47ba-041 (card strip, needs area:49) → T-47ba-042 (live apply, amoled, deprecated names).

Lane E, onboarding: T-47ba-060 (back-blocking gate) → T-47ba-061.

Lane F, verification: T-47ba-070 (unit tests). This depends on T-47ba-031, which is P1 (see section 8, item 3).

Critical blockers outside this area: the `Pref<T>` API from area:02 (blocks Lane B and T-47ba-040), the queue watch receivers from area:23 and area:24 (blocks Lane C), and the palette API from area:49 (blocks Lane D).

### 6.3 Risks

1. **Pref API not agreed.** `Pref<T>` is a signature proposal in `p1-rust-design.md` section 4.1. Every P0 widget task depends on it. Mitigation: agree the signature with area:02 before Lane B starts.
2. **Priority inversion.** T-47ba-003 (P0) depends on P1 tasks T-47ba-044 and T-47ba-050, and T-47ba-070 (P0) depends on P1 task T-47ba-031. A P0 slice cannot ship without P1 work. Mitigation: make the Tracker and MPV variants stubs in T-47ba-003, and move the changelog unit test to P1, or raise T-47ba-044 and T-47ba-050 to P0.
3. **Hand-built dialogs underestimated.** Four dialog kinds (single-choice, multi-select, text-edit with its variants, tri-state) are hand-built GTK windows with keyboard rules (`p1-ui-parity.md` section 7). Their M effort estimates assume the shared `show_dialog` helper (`p1-rust-design.md` section 3.1) exists first, and it is not a task of its own.
4. **Highlight timing is verified by hand.** GTK frame timing and scroll animation are not testable headless until the T-47ba-071 harness exists, which is P1.
5. **Icon names and bundled assets.** Some names may be missing on the user's icon theme. See Q7.
6. **Ownership overlap.** Area 47, 47b and 48 each describe the search screen or the highlight (Q5, Q6). Two implementations of one search would waste the P0 window.
7. **Download queue inputs not designed yet.** T-47ba-015 assumes watch receivers from areas 23 to 25. If those producers arrive later, the Download queue row shows no subtitle, which is wrong but not harmful.
8. **Update download depends on area:35.** T-47ba-032 is L effort and cannot finish before area:35 provides a download job with progress.

---

## 7. Definition of done for this area in Kurobayomi

The area is done when every item below can be observed by a person at the desktop, or by a test named in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5.

1. Any settings page renders each item variant as a row with a 56 px minimum height, 16 px padding, a title that wraps at 2 lines, and a subtitle at 78% opacity. Groups have a 14 px top and 8 px bottom header margin, with 12 px between groups and none after the last group.
2. Clicking a switch row flips the value and persists it. If the write is vetoed, the switch stays at its old value.
3. A single-choice dialog applies and closes on a different selection, stays open when the current item is selected again, and has only a Cancel button.
4. A multi-select dialog shows labels in entries order, shows "None" when empty, and writes only on OK.
5. A text-edit dialog disables OK when the text is unchanged, blank without `canBeBlank`, or invalid. Invalid text replaces the clear button with an error icon.
6. A slider clamps its value to the range, shows a value pill, and calls the veto only when the value changes.
7. The theme mode selector has three linked segments (System, Light, Dark). System follows the desktop; Light and Dark apply at once.
8. The theme strip shows 18 preview cards, selecting one applies it with no restart, and the amoled switch affects dark themes only.
9. Searching for a setting and choosing a hit opens its page; after 500 ms the target row scrolls into view and pulses once for 3 s. A target inside a disabled group does nothing. When two rows share a title, only the first flashes.
10. The More tab shows 14 rows in the order of [p1-behavior.md](p1-behavior.md) section M.2. "Downloaded only" and "Incognito mode" persist and stay in sync with other screens. The download-queue subtitle updates live. Help opens the configured URL (Q1).
11. The update screen shows the real version, the changelog cut at the `Checksums` marker, and Download and Not now. Download starts the background job, disables its button on the first click, and pops the page. An offline start shows the notification "Could not download the update".
12. First run cannot be left by back or close until onboarding is finished. Restore finishes onboarding and opens Data and storage with the "Backup and restore" row flashing once.
13. Every string shown comes from the string table (area:51). The only English literals in code are the proposal strings listed in T-47ba-074.
14. `cargo test` passes every unit test listed in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section 5. With `KUROBAYOMI_GTK_TESTS=1` set, the GTK tests pass under `xvfb-run` or a headless Wayland compositor.
15. `KUROBAYOMI_SHOT=more` and `KUROBAYOMI_SHOT=settings_appearance` produce PNGs at 1180x780 and 720x900 for the manual checks in section 5 of the edge-case document.

---

## 8. Defects and inconsistencies found in the part documents during this review

None of these was fixed in place, except the task lint and token checks, which pass. Each needs an owner to correct its document.

1. **Variant count.** T-47ba-001 and T-47ba-003 say 13 variants. `p1-rust-design.md` section 2 defines 11. See Q11.
2. **Stale helper.** `p1-rust-design.md` section 11 (crate tests) and section 16 (work sequence) name `row_index_for` as a pure function to test. Section 6.2 of the same document replaces index lookup with a `HashMap` of rows, so the helper no longer exists.
3. **Priority inversion.** T-47ba-003 (P0) depends on P1 tasks T-47ba-044 and T-47ba-050. T-47ba-070 (P0) depends on P1 task T-47ba-031. See section 6.3, risk 2.
4. **Ownership conflict.** Area 47b's inventory and area 47's inventory say `SearchableSettings.kt` belongs to area 47. `48-settings-player-and-scripts/p1-behavior.md` line 460 says area 47b. See Q6.
5. **Highlight key.** Area 47's `p1-edge-cases-and-tests.md` line 34 says the key includes the group title. Area 47b keys on title only (E-02). See Q5.
6. **Highlight mechanism.** Area 47's `p1-ui-parity.md` line 330 proposes a CSS `@keyframes` pulse. GTK 4 CSS does not support keyframes. Area 47b uses timers (D5). Area 47's document should be corrected to match.
7. **Revealer duration.** Area 47's `p1-rust-design.md` line 433 uses a 300 ms slide for hidden rows. Area 47b uses 150 ms (`p1-ui-parity.md` section 8). Pick one value for the shared widget.
8. **Logout confirmation wording.** See Q9.
9. **Error message wording.** See Q8.
10. **Dialog height.** See Q10.
11. **MPV write location.** `p1-inventory.md` lists `PreferenceItem.kt` as the file holding the MPV file-write path, and the Replace-with-crate count is attributed to it. The write is in `Preference.kt:184-200` (`p1-behavior.md` B.14). The per-file note in the same inventory names `Preference.kt`. Correct the summary row.
12. **Test classification.** `p1-edge-cases-and-tests.md` section 5 lists T-E07 as a unit test ("lookup order with disabled group"). The lookup is widget-based (`p1-rust-design.md` section 6.2), so T-E07 needs either a pure helper or a move to the GTK list.
13. **Short area references in prose.** Some part documents use `area:02`, `area:36`, `area:38-41` and similar short forms in prose. Dependency tokens in the task files use full ids and pass the lint. Prose short forms are not validated.
14. **Naming near-collision.** Area 47 uses task IDs of the form `T-47b-NNN` (for example `47-settings-screens-general/p2-tasks.md` line 7). This area uses `T-47ba-NNN`. The lint passes, but the two look alike when read quickly. Note this in the registry.
