# Area 47 (part 1): Settings screens (general) — implementation tasks

Scope: the 18 files listed in `p1-inventory.md`. Every task ID starts with `T-47a-`. All source paths are repo-relative to `/home/blackbox/aniyomi-src`. Target crate: `kurobayomi` (GTK 4 via gtk4-rs 0.11, no libadwaita). Cross-references to other plan areas use `area:<id>`.

Global rules applied to every task:
- The settings UI renders from a Rust `Preference` tree (see `p1-rust-design.md`). A preference is hidden (not greyed) when `enabled == false`, matching `PreferenceItem` `StatusWrapper` (`app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt`).
- Stored keys keep the Android names so that backups remain importable (see `p1-data-and-config.md`).

---

## A. Framework and cross-cutting

### T-47a-001 — Preference store: SQLite key/value store with typed `Pref<T>`
- Priority: P0
- Effort: L
- Depends: area:02-preferences-system, area:03-db-manga-schema-migrations
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/InMemoryPreferenceStore.kt, app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt
- Acceptance:
  - A `prefs` table `(key TEXT PRIMARY KEY, value TEXT NOT NULL)` stores JSON-encoded values; keys are the exact Android names listed in `p1-data-and-config.md` section 2 (233 rows).
  - Typed getters exist for Bool, i32, i64, String, string-set (BTreeSet<String>), enum-by-name, and the Object serializers used by `LibraryDisplayMode` and `MangaLibrarySort`/`AnimeLibrarySort`.
  - A missing key returns the documented default; an unparsable enum name returns the default (matches `getEnum` `enumValueOf` catch in `PreferenceStore.kt`).
  - Keys starting with `__APP_STATE_` and `__PRIVATE_` are classified as app-state and private (see T-47a-009).
  - Writes made on the GTK thread are visible to the next read on that thread without waiting for the database write.
  - A `Pref<T>::subscribe()` stream emits the new value after every `set()` and `delete()`.

### T-47a-002 — Settings item model and row rendering rules
- Priority: P0
- Effort: M
- Depends: T-47a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScaffold.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/SwitchPreferenceWidget.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/PreferenceGroupHeader.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/InfoWidget.kt
- Acceptance:
  - Item kinds exist: Text, Switch, Slider, List, BasicList, MultiSelectList, EditText, MultiLineEditText, MPVConf (out of scope; unused here), EditTextInfo, Tracker, Info, Custom, plus Group. Every kind listed in `p1-behavior.md` section 2 maps to one widget.
  - A `PreferenceItem` with `enabled == false` is not shown (its row is collapsed with a 300 ms reveal/collapse). A `PreferenceGroup` with `enabled == false` is not rendered at all, including its header.
  - Tapping a Switch row toggles the value, not only the switch glyph (`SwitchPreferenceWidget` row `onPreferenceClick`).
  - A value change is applied only when `onValueChanged` returns true; returning false keeps the old value (dialog stays open for EditText; list dialog closes without change for ListPreference only when the same value is re-selected).
  - Group header text uses secondary colour, 14 px top and 8 px bottom spacing; a 12 px spacer follows each group except the last.
  - Info rows show an info glyph above text and use the secondary-alpha colour (`secondaryItemAlpha`).

### T-47a-003 — Settings main menu
- Priority: P0
- Effort: S
- Depends: T-47a-002, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsMainScreen.kt
- Acceptance:
  - Exactly these 11 rows, in this order, each with title, subtitle and icon: Appearance, Library, Reader, Player (opens `PlayerSettingsScreen(mainSettings = true)`), Downloads, Tracking, Browse, Data and storage, Security and privacy, Advanced, About.
  - About subtitle is `"<app_name> <version name without build date>"` (`Aniyomi Stable <VERSION_NAME>` in release builds; `Debug <COMMIT_SHA>` in debug builds).
  - Top-bar search action (icon `Search`, title `action_search`) opens the search screen (T-47a-004).
  - Back action uses the navigator's back-press handler when present.
  - Two-pane mode (`Content(twoPane = true)`) uses surface lightness +0.02 (light) or −0.05 (dark), clamped to [0, 1], and highlights the active row with a rounded 24 px background.

### T-47a-004 — Settings search across all settings screens
- Priority: P1
- Effort: M
- Depends: T-47a-002, T-47a-006
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt
- Acceptance:
  - The index contains exactly these screens in this order: Appearance, Library, Reader, Downloads, Tracking, Browse, Data and storage, Security and privacy, Advanced. Player screens (7) are added only as the player index.
  - A row matches when its title or its subtitle contains the query (case-insensitive). Disabled items and disabled groups are excluded. Info rows and rows with blank titles are excluded.
  - At most 10 results are shown (`take(10)`), in index order.
  - Each result shows the title (one line, ellipsised) and breadcrumbs `Category > Group` (or `Category` for top-level rows), with ` > ` as separator for LTR layouts.
  - Selecting a result replaces the search screen with the target screen and sets the highlight key to the row title.
  - Empty results show "No results found" (`no_results_found`). An empty query shows an empty list.
  - Each keystroke re-runs the search with no debounce.
  - Search field placeholder is `Search settings` (`action_search_settings`), or `Search player settings` in the player search. Clear button appears when the query is non-empty.

### T-47a-005 — Category label helper (include/exclude summary)
- Priority: P0
- Effort: S
- Depends: T-47a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/Commons.kt, app/src/main/java/eu/kanade/presentation/category/CategoryExtensions.kt
- Acceptance:
  - Output is exactly `"Include: <inc>\nExclude: <exc>"` using `include` and `exclude` format strings.
  - Included list: ids that resolve to categories, sorted by `order`. Exclude list: same rule.
  - `allExcluded` = excluded count equals total category count (also true when both are 0).
  - Include text: if 0 < included < total, join names with `, `; if included == total (including 0 == 0), `All`; else if allExcluded, `None`; else `All`.
  - Exclude text: none excluded → `None`; allExcluded → `All`; else joined names.
  - Unknown ids (deleted categories) are silently dropped.
  - Unit tests cover the 8 combinations in `p1-edge-cases-and-tests.md` section 3.

### T-47a-006 — SearchableSettings screen contract
- Priority: P0
- Effort: S
- Depends: T-47a-002
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScaffold.kt
- Acceptance:
  - Every settings screen implements: `title` (a localized string key), `preferences()` (returns the `Preference` tree, evaluated on each open and each search), optional `app_bar_action` (Tracking and Data screens add a help link).
  - The top bar shows the title, an up arrow when the stack is non-empty, and the optional action.
  - Each screen's list is scrollable and has a scrollbar.

### T-47a-007 — Highlight the target row after a search jump
- Priority: P1
- Effort: S
- Depends: T-47a-004
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt
- Acceptance:
  - After arriving via search, the screen waits 500 ms, scrolls the matching row into view, then highlights it for 3 s.
  - Highlight background is `surfaceTint` at alpha 0.12, blinking 5 times (200 ms per half-cycle, first delay 600 ms).
  - The highlight key is cleared after the first screen that consumes it.

### T-47a-008 — Keyboard and accessibility parity for settings
- Priority: P1
- Effort: S
- Depends: T-47a-002
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - Every row is reachable with Tab/Shift+Tab and activated with Space or Enter; switches toggle with Space.
  - Search field receives focus on open; Enter clears focus (hides the on-screen keyboard equivalent).
  - Icon-only buttons (help link, search, clear, up) have accessible names: `Tracking guide`, `Search settings`, `Clear`, `Back`.
  - Dialogs trap focus and close on Escape (Cancel behaviour).

### T-47a-009 — Backup-compatibility classification of preference keys
- Priority: P1
- Effort: S
- Depends: T-47a-001, area:21-backup-format-and-create
- Sources: core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - Keys prefixed `__APP_STATE_` are excluded from backups and from the settings export.
  - Keys prefixed `__PRIVATE_` (tracker usernames, passwords, token, auth-expired flags) are excluded from backups without user consent.
  - All other keys listed in `p1-data-and-config.md` section 2 are eligible for backup.
  - A test imports a fixture backup containing one private key and asserts it is not restored.

### T-47a-010 — Restart-required versus live-apply matrix
- Priority: P1
- Effort: S
- Depends: T-47a-002
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt
- Acceptance:
  - Every setting that shows the `requires_app_restart` toast in Android is either applied live on Linux or documented as restart-required in its subtitle. The matrix is in `p1-behavior.md` section 12.
  - Settings applied live take effect without a toast.
  - Settings that still need a restart show the text "Requires app restart to take effect" (`requires_app_restart`) in the subtitle, not as a transient toast.

---

## B. Appearance

### T-47a-011 — Theme mode (System / Light / Dark)
- Priority: P0
- Effort: S
- Depends: T-47a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - Preference `pref_theme_mode_key` default `SYSTEM`; values `LIGHT`, `DARK`, `SYSTEM` (stored by enum name).
  - Changing it applies the theme immediately: LIGHT forces light, DARK forces dark, SYSTEM follows the XDG portal `org.freedesktop.appearance color-scheme` (0 = no preference, 1 = dark, 2 = light), updated live when the portal signals a change.
  - Pure-black switch (T-47a-013) is hidden unless the effective theme is dark.

### T-47a-012 — Drop the 17 named app palettes
- Priority: P3
- Effort: S
- Depends: T-47a-011
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - The Rust build has no in-app palette picker. The GTK theme is the single source of colour.
  - Stored values `pref_app_theme` are ignored; on first run after upgrade the key is deleted.
  - The 19 selectable options are Default, Dynamic and 17 named palettes; `Default` maps to the GTK theme (kept), `Dynamic` is task 013. `p1-ui-parity.md` lists the 17 lost named themes (Cloudflare, Cotton Candy, Doom, Green Apple, Lavender, Matrix, Midnight Dusk, Mocha, Sapphire, Nord, Strawberry Daiquiri, Tako, Teal & Turquoise, Tidal Wave, Yin & Yang, Yotsuba, Monochrome) and the user-facing consequence.
  - Decision requires explicit user sign-off before implementation (surface this in the summary).

### T-47a-013 — Dynamic accent replaces "Dynamic" (Monet) theme
- Priority: P2
- Effort: S
- Depends: T-47a-011
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/DeviceUtilExtensions.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - Option "Dynamic" (`MR.strings.theme_monet`) becomes "Follow desktop accent" and reads `org.freedesktop.appearance accent-color` from the XDG portal (RGB triple) and applies it as the accent CSS colour.
  - If the portal does not provide an accent, the option is hidden (equivalent of `isDynamicColorAvailable == false`).

### T-47a-014 — Pure black (AMOLED) dark mode switch
- Priority: P2
- Effort: S
- Depends: T-47a-011
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - Key `pref_theme_dark_amoled_key` default `false`; label `Pure black dark mode`.
  - Row hidden when theme mode is LIGHT.
  - When on, dark surfaces use `#000000` for the window background and screen containers; applies live via a CSS provider swap, no restart.

### T-47a-015 — Start screen selection
- Priority: P1
- Effort: S
- Depends: T-47a-001, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - Key `start_screen` default `ANIME`; entries in this order with labels: `ANIME` = "Anime", `MANGA` = "Manga", `UPDATES` = "Updates", `HISTORY` = "History", `BROWSE` = "Browse".
  - The app opens on the selected tab at next launch (live change is not required).

### T-47a-016 — Date format and relative timestamps
- Priority: P1
- Effort: M
- Depends: T-47a-001
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt
- Acceptance:
  - Key `app_date_format` default `""`. Entries in order: `""` (localized short), `MM/dd/yy`, `dd/MM/yy`, `yyyy-MM-dd`, `dd MMM yyyy`, `MMM dd, yyyy`. Each label is `<pattern or "Default"> (<today formatted>)`.
  - Patterns are converted to chrono format strings (`MM`→`%m`, `dd`→`%d`, `yy`→`%y`, `yyyy`→`%Y`, `MMM`→`%b`); `""` uses the locale short date.
  - Key `relative_time_v2` default `true`; when on, "today" dates render as `Today` (`relative_time_today`) and relative times use "Just now" under one minute (`updates_last_update_info_just_now`), "Never" for timestamp ≤ 0 (`relative_time_span_never`).
  - Subtitle reads `"Today" instead of "<today formatted>"` (`pref_relative_format_summary`).

### T-47a-017 — Drop Tablet UI mode
- Priority: P3
- Effort: S
- Depends: T-47a-003
- Sources: app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - No `tablet_ui_mode` row. Layout switches by window width only (breakpoint documented in `p1-ui-parity.md`).
  - Stored key `tablet_ui_mode` is deleted on upgrade.
  - Lost behaviour (forcing tablet layout on phones or disabling it on tablets) is listed in `p1-ui-parity.md`.

### T-47a-018 — Navigation style → sidebar "More" entry
- Priority: P2
- Effort: S
- Depends: T-47a-003
- Sources: app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt
- Acceptance:
  - Key `bottom_rail_nav_style` default `MOVE_HISTORY_TO_MORE`; entries `MOVE_MANGA_TO_MORE`, `MOVE_UPDATES_TO_MORE`, `MOVE_HISTORY_TO_MORE`, `MOVE_BROWSE_TO_MORE` with labels from the strings table.
  - The sidebar shows the five non-"More" tabs (Anime library, Manga library, Updates, History, Browse) minus the one moved into More; More is always last.
  - The row label is "Navigation Style" (hard-coded in Android; Rust uses the string key `pref_navigation_style` with the same English text).

### T-47a-019 — App language entry point
- Priority: P1
- Effort: S
- Depends: area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt
- Acceptance:
  - A `TextPreference` titled `App language` (`pref_app_language`) pushes the language screen implemented in part 2 of this area.
  - Row is always enabled.

---

## C. Library

### T-47a-020 — Default category for new manga and anime
- Priority: P0
- Effort: M
- Depends: T-47a-005, area:08-domain-library-category-history-updates
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt
- Acceptance:
  - "Edit anime categories" subtitle is the count of user (non-system) anime categories using `num_categories` plural (`"0 categories"`, `"1 category"`, `"N categories"`). Tap pushes the Categories screen.
  - "Edit manga categories" does the same and opens the Categories screen on the manga tab.
  - "Default anime category" list: keys `default_anime_category`, default `-1`; entries: `-1` → "Always ask", then each category id → name, ordered as the category list.
  - "Default manga category" list: keys `default_category`, default `-1`; same entry rules.
  - Deleting the chosen category resets the stored value to `-1` on the next read.

### T-47a-021 — Per-category display settings (sort/filter per category)
- Priority: P1
- Effort: S
- Depends: T-47a-020, area:37-ui-library
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Switch `categorized_display` default `false`, label "Per-category settings for sort".
  - Turning it off calls `ResetMangaCategoryFlags` (clears per-category flags for manga) and then sets the value.
  - Turning it on only sets the value.

### T-47a-022 — Hide hidden categories
- Priority: P1
- Effort: S
- Depends: T-47a-020
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Switch `hidden_categories` default `false`, label "Hide hidden categories from categories and storage screen".
  - When on, the categories screen and the storage screen omit categories flagged hidden.

### T-47a-023 — Automatic library update interval
- Priority: P1
- Effort: M
- Depends: area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateJob.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Key `pref_library_update_interval_key` default `0`; values `0`, `12`, `24`, `48`, `72`, `168` with labels Off, Every 12 hours, Daily, Every 2 days, Every 3 days, Weekly.
  - Changing it calls `setupTask(interval)` for both the manga and anime jobs. A value of 0 cancels the unique work `LibraryUpdate-auto` and `AnimeLibraryUpdate-auto`.
  - Values greater than 0 enqueue a periodic job with period = interval hours and flex 10 minutes, backoff linear 10 minutes (`UPDATE` policy).

### T-47a-024 — Automatic update device restrictions
- Priority: P1
- Effort: M
- Depends: T-47a-023
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateJob.kt
- Acceptance:
  - Multi-select key `library_update_restriction` default `{wifi}`; entries `wifi` = "Only on Wi-Fi", `network_not_metered` = "Only on unmetered network", `ac` = "When charging".
  - Row disabled (hidden) when interval is 0.
  - Subtitle reads `Restrictions: <joined labels>`, or `Restrictions: None` when the set is empty.
  - Changing it reschedules both jobs after the new value is stored (equivalent of posting to the main looper).
  - Restriction mapping to a scheduler constraint: `wifi` → transport Wi-Fi; `network_not_metered` → unmetered; `ac` → requires plugged-in power. Battery-not-low is always required.

### T-47a-025 — Include/exclude categories for automatic update (tri-state dialogs)
- Priority: P1
- Effort: M
- Depends: T-47a-005, T-47a-023
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/TriStateListDialog.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/Commons.kt
- Acceptance:
  - Keys: `library_update_categories` and `library_update_categories_exclude` (manga); `animelib_update_categories` and `animelib_update_categories_exclude` (anime). Values are sets of category id strings.
  - Dialog rows cycle Unchecked → Checked (include) → Inverted (exclude) → Unchecked on each tap. Confirm stores both sets; Cancel stores nothing.
  - Dialog message (anime): "Anime in excluded categories will not be updated even if they are also in included categories." Manga: "Manga in excluded categories will not be updated even if they are also in included categories."
  - Subtitle uses the T-47a-005 label helper.

### T-47a-026 — Metadata refresh, smart update and tab-badge switches
- Priority: P1
- Effort: S
- Depends: T-47a-023
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Switch `auto_update_metadata` default `false`: "Automatically refresh metadata" / "Check for new cover and details when updating library".
  - Multi-select `library_update_manga_restriction` (smart update) default is all four values `manga_fully_read`, `manga_started`, `manga_ongoing`, `manga_outside_release_period`; labels in order: "Skip entries with unseen episode(s)/unread chapter(s)", "Skip unstarted entries", "Skip entries with \"Completed\" status", "Predict next release time".
  - Switch `library_show_updates_count` default `true`: "Show unseen/unread count on Updates icon".
  - Row is hidden (not disabled) when the automatic interval is 0 only for the smart-update multi-select; the two switches are always visible. Verify this against `p1-behavior.md` section 3.

### T-47a-027 — Season update behaviour switches
- Priority: P1
- Effort: S
- Depends: area:16-source-api-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Switch `pref_update_season_on_refresh` default `false`: "Update seasons with episodes on refresh".
  - Switch `pref_update_season_on_library_update` default `false`: "Update seasons with episodes on library update".
  - Section header "Season".

### T-47a-028 — Chapter and episode swipe actions
- Priority: P0
- Effort: M
- Depends: area:37-ui-library, area:38-ui-entry-details-manga
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Chapter "swipe to left" (title key `pref_chapter_swipe_start`) reads `ChapterSwipeAction` from key `pref_chapter_swipe_end_action` (default `ToggleRead`). "Swipe to right" reads key `pref_chapter_swipe_start_action` (default `ToggleBookmark`). The key names are inverted relative to the UI labels; Rust keeps the key names.
  - Chapter entries: `Disabled` = "Disabled", `ToggleBookmark` = "Bookmark chapter", `ToggleRead` = "Mark as read", `Download` = "Download".
  - Episode "swipe to left" reads key `pref_episode_swipe_end_action` (default `ToggleSeen`); "swipe to right" reads `pref_episode_swipe_start_action` (default `ToggleBookmark`).
  - Episode entries: `Disabled` = "Disabled", `ToggleBookmark` = "Bookmark episode", `ToggleFillermark` = "Fillermark episode", `ToggleSeen` = "Mark as watched", `Download` = "Download".
  - Library rows honour the chosen action on swipe (verified by an integration test in area 37).

### T-47a-029 — Mark duplicate chapter/episode as read/seen
- Priority: P1
- Effort: S
- Depends: T-47a-028
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Multi-select `mark_duplicate_read_chapter_read` (default empty): `existing` = "After reading a chapter", `new` = "After fetching new chapter".
  - Multi-select `mark_duplicate_seen_episode_seen` (default empty): `existing_episode` = "After watching an episode", `new_episode` = "After fetching new episode".
  - Titles: "Mark duplicate read chapter as read", "Mark duplicate seen episode as seen".

### T-47a-030 — Fix shared storage keys in LibraryPreferences
- Priority: P1
- Effort: S
- Depends: T-47a-001
- Sources: domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Episode display mode and chapter display mode use separate keys. Current Android code shares `default_chapter_display_by_name_or_number` (lines 205–213). Rust uses `default_episode_display_by_name_or_number` for episodes and migrates any value found under the shared key by copying it into both keys once.
  - Episode and chapter ascending/descending use separate keys. Current code shares `default_chapter_sort_by_ascending_or_descending` (lines 215–223).
  - Season "downloaded" default is written from the correct field: `setSeasonSettingsDefault` writes `seasonUnseenFilterRaw` into the downloaded-filter key (line 331). Rust writes `seasonDownloadedFilterRaw`.
  - A test asserts that writing episode defaults does not change chapter defaults and vice versa.

---

## D. Reader

### T-47a-031 — Default reading mode for new series
- Priority: P0
- Effort: M
- Depends: area:43-reader-core, area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Key `pref_default_reading_mode_key` default `2` (`RIGHT_TO_LEFT`). Entries: `1` "Paged (left to right)", `2` "Paged (right to left)", `3` "Paged (vertical)", `4` "Long strip", `5` "Long strip with gaps". Value `0` (Default) is not offered.
  - Unknown stored values fall back to `ReadingMode.DEFAULT` on read (`ReadingMode.fromPreference`); the stored value is not rewritten by this screen.
  - Selecting `4` or `5` switches the reader to the webtoon viewer for series that use the default.

### T-47a-032 — Double-tap animation speed
- Priority: P1
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Key `pref_double_tap_anim_speed` default `500`; entries `1` "No animation", `500` "Normal", `250` "Fast".
  - Value `1` disables the zoom animation; `250` and `500` set its duration in ms.

### T-47a-033 — Top-level reader switches
- Priority: P0
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `pref_show_reading_mode` default `true`: "Show reading mode" / "Briefly show current mode when reader is opened".
  - `reader_navigation_overlay_on_start` default `false`: "Show tap zones overlay" / "Briefly show when reader is opened".
  - `pref_enable_transitions_key` default `true`: "Animate page transitions".
  - These three rows have no group header.

### T-47a-034 — Display group (rotation, background, fullscreen, keep awake, page number)
- Priority: P0
- Effort: M
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Default rotation: key `pref_default_orientation_type_key` default `8` (`FREE`). Entries `8` Free, `16` Portrait, `24` Landscape, `32` Locked portrait, `40` Locked landscape, `48` Reverse portrait. Value `0` is not offered.
  - Background: key `pref_reader_theme_key` default `1`; entries `1` Black, `2` Gray, `0` White, `3` Auto.
  - Fullscreen: key `fullscreen` default `true`.
  - Keep screen on: key `pref_keep_screen_on_key` default `true`; while the reader is open, the window holds an idle inhibit (GTK `Application::inhibit`, flag IDLE).
  - Show page number: key `pref_show_page_number_key` default `true`.
  - "Show content in cutout area" is not present (T-47a-052).

### T-47a-035 — E-ink flash group
- Priority: P2
- Effort: M
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Switch `pref_reader_flash` default `false`.
  - Duration slider over `1..15`, stored value is `slider × 100` ms (`pref_reader_flash_duration`, default `100`); subtitle `%1$s ms` with the stored ms value.
  - Interval slider over `1..10` (`pref_reader_flash_interval`, default `1`); subtitle plural "1 page" / "N pages".
  - Colour list `pref_reader_flash_mode` default `BLACK`: `BLACK` "Black", `WHITE` "White", `WHITE_BLACK` "White and Black".
  - Duration, interval and colour rows are hidden while the flash switch is off.

### T-47a-036 — Reading group (skip rules, transitions, position)
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `skip_read` default `false`: "Skip chapters marked read".
  - `skip_filtered` default `true`: "Skip filtered chapters".
  - `skip_dupe` default `false`: "Skip duplicate chapters".
  - `always_show_chapter_transition` default `true`: "Always show chapter transition".
  - `pref_preserve_reading_position` default `false`: "Preserve reading position on read chapters".
  - Section header "Reading".

### T-47a-037 — Paged viewer group
- Priority: P0
- Effort: M
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Tap zones `reader_navigation_mode_pager` default `0`; entries `0` Default, `1` L shaped, `2` Kindle-ish, `3` Edge, `4` Right and Left, `5` Disabled.
  - Invert tap zones `reader_tapping_inverted` default `NONE` (enum name); entries NONE, HORIZONTAL, VERTICAL, BOTH; hidden when tap zones = 5.
  - Scale type `pref_image_scale_type_key` default `1`; entries `1` Fit screen, `2` Stretch, `3` Fit width, `4` Fit height, `5` Original size, `6` Smart fit.
  - Zoom start `pref_zoom_start_key` default `1`; entries `1` Automatic, `2` Left, `3` Right, `4` Center.
  - Crop borders `crop_borders` default `false`.
  - Zoom into wide images `landscape_zoom` default `true`; hidden unless scale type = 1.
  - Pan wide images `navigate_pan` default `true`; hidden when tap zones = 5.
  - Split wide pages `pref_dual_page_split` default `false`; turning it on forces rotate-to-fit off.
  - Invert split page placement `pref_dual_page_invert` default `false`; hidden unless split is on.
  - Rotate wide pages to fit `pref_dual_page_rotate` default `false`; turning it on forces split off.
  - Flip orientation of rotated pages `pref_dual_page_rotate_invert` default `false`; hidden unless rotate is on.

### T-47a-038 — Long-strip (webtoon) group
- Priority: P0
- Effort: M
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Tap zones `reader_navigation_mode_webtoon` default `0` (same entries as T-47a-037).
  - Invert tap zones `reader_tapping_inverted_webtoon` default `NONE`; hidden when tap zones = 5.
  - Side padding slider `webtoon_side_padding` over `0..25`, default `0`; subtitle is the percent with `NumberFormat.getPercentInstance()` applied to `value / 100`.
  - Sensitivity for hiding menu on scroll `reader_hide_threshold` default `LOW`; entries `HIGHEST` = 5, `HIGH` = 13, `LOW` = 31, `LOWEST` = 47 (scroll pixels threshold used by the viewer).
  - Crop borders `crop_borders_webtoon` default `false`.
  - Split wide pages `pref_dual_page_split_webtoon` default `false` (forces rotate off); invert `pref_dual_page_invert_webtoon` default `false`; rotate `pref_dual_page_rotate_webtoon` default `false` (forces split off); flip `pref_dual_page_rotate_invert_webtoon` default `false`.
  - Double tap to zoom `pref_enable_double_tap_zoom_webtoon` default `true`.
  - Disable zoom out `webtoon_disable_zoom_out` default `false`.

### T-47a-039 — Navigation group (volume keys)
- Priority: P1
- Effort: S
- Depends: area:44-reader-viewers, area:46-player-controls-ui
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `reader_volume_keys` default `false`: "Volume keys" → page with PageUp/PageDown and the XF86 Audio Raise/Lower keys while the reader has focus.
  - `reader_volume_keys_inverted` default `false`: "Invert volume keys"; hidden unless volume keys are on.
  - Global (unfocused) key capture is not implemented; the setting only works while the reader window is focused.

### T-47a-040 — Actions group (long tap, folder per manga)
- Priority: P1
- Effort: S
- Depends: area:44-reader-viewers, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `reader_long_tap` default `true`: "Show actions on long tap" (long-press on a page opens the actions menu; the GTK gesture is `GestureLongPress`).
  - `create_folder_per_manga` default `false`: "Save pages into separate folders" / "Creates folders according to entries' title" (page saves go to `<storage>/<entry title>/`).

### T-47a-041 — Enable/disable dependency graph for reader settings
- Priority: P0
- Effort: S
- Depends: T-47a-034, T-47a-037, T-47a-038
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt
- Acceptance:
  - Implemented as a single function `reader_row_enabled(id, state) -> bool` used by all three viewer groups; no per-row ad hoc conditions.
  - The rules are listed in `p1-behavior.md` section 4 and each has one unit test.

### T-47a-042 — Reserved colour and brightness keys
- Priority: P2
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Keys `pref_custom_brightness_key`, `custom_brightness_value`, `pref_color_filter_key`, `color_filter_value`, `color_filter_mode`, `pref_grayscale`, `pref_inverted_colors` exist in the Rust prefs with the defaults in `p1-data-and-config.md`. They are rendered in the reader's quick-settings panel, owned by area 44; this area does not show them in the settings tree.

### T-47a-043 — Drop "Show content in cutout area"
- Priority: P3
- Effort: S
- Depends: T-47a-034
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - No display-cutout concept exists on Linux desktops; the row is removed and `cutout_short` is ignored.
  - Lost behaviour: none on desktop hardware (the Android option only applied to notched phones).

---

## E. Downloads

### T-47a-044 — Download only on Wi-Fi
- Priority: P1
- Effort: S
- Depends: area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Switch `pref_download_only_over_wifi_key` default `true`, label "Only on Wi-Fi".
  - Linux: "Wi-Fi" is detected via NetworkManager (`org.freedesktop.NetworkManager` `ActiveConnection` type `802-11-wireless`) or `gio::NetworkMonitor` metered flag; if the connection type cannot be read the download proceeds and a warning is logged.

### T-47a-045 — Chapter download speed limit
- Priority: P1
- Effort: M
- Depends: area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/player/components/OutlinedNumericChooser.kt
- Acceptance:
  - Key `download_speed_limit` default `0`, unit KiB/s, value ≥ 0.
  - Row subtitle: `Off` when 0, else `<n> KiB/s`.
  - Dialog title "Chapter download speed limit"; numeric chooser with label, placeholder `0`, suffix `KiB/s`, step `100`, minimum `0`, no maximum; hint "Set to 0 to disable the speed limit."; buttons Cancel and OK.
  - OK stores the chosen value; Cancel discards it.
  - The limit is applied as a token-bucket rate in the download HTTP client (area 25).

### T-47a-046 — Save as CBZ and split tall images
- Priority: P1
- Effort: S
- Depends: area:23-downloads-manga, area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - `save_chapter_as_cbz` default `true`: "Save as CBZ archive".
  - `split_tall_images` default `true`: "Split tall images" / "Improves reader performance".

### T-47a-047 — Maximum concurrent downloads
- Priority: P1
- Effort: S
- Depends: area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Key `download_slots` default `1`; entries `1`–`5`; label "Maximum downloads".
  - Info text "Will only download concurrently from self-hosted or unmetered sources".
  - Changing it updates the download scheduler's concurrency limit without restart.

### T-47a-048 — Delete-after-read group
- Priority: P1
- Effort: M
- Depends: area:25-downloads-shared, area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Switch `pref_remove_after_marked_as_read_key` default `false`: "After manually marked as read/watched".
  - List `remove_after_read_slots` default `-1`; entries `-1` Disabled, `0` Last read chapter, `1` Second to last read chapter, `2` Third to last read chapter, `3` Fourth to last read chapter, `4` Fifth to last read chapter.
  - Switch `pref_remove_bookmarked` default `false`: "Allow deleting bookmarked chapters/episodes".
  - Switch `pref_download_fillermarked` default `false`: "Allow downloading of fillermarked episodes".
  - Multi-select `remove_exclude_categories` (manga) and `remove_exclude_anime_categories` (anime), default empty; entries are category ids → names; titles "Excluded manga categories" and "Excluded anime categories".

### T-47a-049 — Auto-download new items per category
- Priority: P1
- Effort: M
- Depends: area:25-downloads-shared, area:26-library-update-jobs
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/TriStateListDialog.kt
- Acceptance:
  - `download_new_episode` default `false` ("Download new episodes"); `download_new_unread_episodes_only` default `false` ("Skip downloading duplicate seen episodes"), enabled only when the first is on.
  - `download_new_anime_categories` and `download_new_anime_categories_exclude` define the anime include/exclude set; row enabled only when new-episode download is on; dialog message "Entries in excluded categories will not be downloaded even if they are also in included categories."
  - `download_new` default `false` ("Download new chapters"); `download_new_unread_chapters_only` default `false` ("Skip downloading duplicate read chapters"), enabled only when the first is on.
  - `download_new_categories` and `download_new_categories_exclude` define the manga set; row enabled only when new-chapter download is on.
  - Auto-download runs after each library update for entries in the included categories (area 26).

### T-47a-050 — Download-ahead limits
- Priority: P1
- Effort: S
- Depends: area:25-downloads-shared
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - `auto_download_while_watching` default `0`; entries `0` Disabled, `2` Next 2 unseen episodes, `3` Next 3 unseen episodes, `5` Next 5 unseen episodes, `10` Next 10 unseen episodes (plural `next_unseen_episodes`).
  - `auto_download_while_reading` default `0`; entries `0` Disabled, `2`/`3`/`5`/`10` as "Next N unread chapters" (plural `next_unread_chapters`).
  - Info text "Only works if the current chapter/episode + the next one are already downloaded."

### T-47a-051 — Drop the external-downloader selector (IDM / ADM)
- Priority: P3
- Effort: S
- Depends: T-47a-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt, domain/src/main/java/tachiyomi/domain/download/service/DownloadPreferences.kt
- Acceptance:
  - Keys `use_external_downloader` and `external_downloader_selection` are read and ignored on Linux; the group is removed.
  - Lost behaviour: handing anime downloads to Android IDM/ADM packages. Replacement, if wanted later, is aria2c (not in this plan).
  - Requires user sign-off (listed in the summary).

### T-47a-052 — Downloads info text and enable/disable rules
- Priority: P1
- Effort: S
- Depends: T-47a-049
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt
- Acceptance:
  - Each enable/disable dependency in the Downloads screen is unit-tested (table in `p1-behavior.md` section 6).

---

## F. Tracking

### T-47a-053 — Tracking toggles
- Priority: P1
- Effort: S
- Depends: area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - `pref_auto_update_manga_sync_key` default `true`: "Update progress after reading/watching".
  - `track_on_adding_to_library` default `true`: "Open track menu on adding to library".
  - `show_next_episode_airing_time` default `true`: "Show next episode's airing time".

### T-47a-054 — Auto-update progress when marked read/watched
- Priority: P1
- Effort: S
- Depends: T-47a-053
- Sources: app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt, app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Key `pref_auto_update_manga_on_mark_read` default `ALWAYS`; enum `ALWAYS` = "Always", `ASK` = "Always ask", `NEVER` = "Never".
  - Label "Update progress when marked as read/watched".

### T-47a-055 — Tracker service rows and login/logout dialogs
- Priority: P1
- Effort: M
- Depends: area:27-tracking-core, area:28-tracker-anilist, area:29-tracker-myanimelist, area:30-tracker-kitsu, area:31-tracker-shikimori-bangumi, area:32-tracker-mangaupdates-simkl
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/widget/TrackingPreferenceWidget.kt
- Acceptance:
  - Rows in order: MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, Bangumi; each shows the tracker logo in a rounded square, the name, and a green check when logged in.
  - Kitsu login dialog: title `Log in to <name>`, Email field (`email`), Password field with show/hide toggle, Login button.
  - MangaUpdates login dialog: Username field (`username`), Password field, Login button.
  - Login button disabled until both fields are non-blank and no login is in flight; label becomes "Loading…" while in flight.
  - On success: toast "Logged in", dialog closes. On failure: the tracker's `logout()` runs (credentials cleared) and the error message is shown; dialog stays open with the error outline.
  - Logout dialog: title `Log out from <name>?`, Cancel and red "Log out"; on confirm toast "You are now logged out".
  - Info text "One-way sync to update the chapter progress in external tracker services. Set up tracking for individual entries from their tracking button."

### T-47a-056 — OAuth browser launch for trackers
- Priority: P1
- Effort: M
- Depends: T-47a-055, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt, app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiApi.kt
- Acceptance:
  - Login for MyAnimeList, AniList, Shikimori, Simkl and Bangumi opens the tracker's `authUrl()` in the system default browser (`xdg-open` / `gtk::UriLauncher`).
  - The OAuth redirect is handled as defined in area 27; this task only covers the launcher call and the "pending login" state until the token arrives.

### T-47a-057 — Enhanced trackers (source-linked) section
- Priority: P1
- Effort: M
- Depends: area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Enhanced manga trackers and enhanced anime trackers are shown only when at least one installed source's class name is in the tracker's accepted-source list.
  - Trackers without an installed accepted source are listed in the info text: "Available but source not installed: <names joined by ', '>", appended after two newlines.
  - Enhanced rows have no credentials dialog; tapping them invokes `loginNoop()`.
  - Section header "Enhanced trackers".

### T-47a-058 — Tracking guide link
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt
- Acceptance:
  - Top-bar help icon (`HelpOutline`, tooltip "Tracking guide") opens `https://aniyomi.org/help/guides/tracking/` in the default browser.

### T-47a-059 — Tracker credentials and token storage
- Priority: P1
- Effort: M
- Depends: T-47a-001, area:27-tracking-core
- Sources: app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt
- Acceptance:
  - Username, password and token are stored in the OS keyring (Secret Service / KWallet via the `keyring` crate), keyed by `kurobayomi-tracker-<id>`.
  - The SQLite prefs hold only the auth-expired flag under `__PRIVATE_pref_tracker_auth_expired_<id>`.
  - If no keyring is reachable, the credentials are kept in a `0600` file under `$XDG_DATA_HOME/kurobayomi/` and a warning row is shown in the tracking screen.

---

## G. Browse (sources and NSFW)

### T-47a-060 — Hide entries already in library (browse lists)
- Priority: P0
- Effort: S
- Depends: area:40-ui-browse-manga, area:41-ui-browse-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `browse_hide_in_anime_library_items` default `false`: "Hide anime entries already in library".
  - `browse_hide_in_library_items` default `false`: "Hide manga entries already in library".
  - Global-search results and browse lists honour each switch independently.

### T-47a-061 — Show NSFW (18+) sources, auth-gated
- Priority: P1
- Effort: S
- Depends: area:40-ui-browse-manga, area:41-ui-browse-anime, T-47a-068
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSearchScreen.kt
- Acceptance:
  - `show_nsfw_source` default `true`: "Show in sources and extensions lists", subtitle "Requires app restart to take effect" (T-47a-010 rule applies; the Rust build applies the filter live).
  - Toggling requires device authentication (T-47a-068); if the gate fails the value is unchanged.
  - Info text "This does not prevent unofficial or potentially incorrectly flagged extensions from surfacing NSFW (18+) content within the app."
  - Section header "NSFW (18+) sources".

### T-47a-062 — Extension repository rows with counts
- Priority: P1
- Effort: S
- Depends: area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt
- Acceptance:
  - "Anime extension repos" row subtitle: plural `num_repos` of the anime store count (`0 stores`, `1 store`, `N stores`); opens the anime stores screen (part 2).
  - "Manga extension repos" row subtitle: plural `num_repos` of the manga repo count; opens the manga repos screen (part 2).
  - Counts update live when repositories are added or removed.

---

## H. Security and privacy

### T-47a-063 — App lock (Require unlock) on Linux
- Priority: P2
- Effort: M
- Depends: area:36-ui-shell-navigation-security-webview, T-47a-068
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt
- Acceptance:
  - Key `use_biometric_lock` default `false`; label "Require unlock".
  - Decision required (see `p1-rust-design.md` section 9): primary = app passphrase (argon2id hash stored in the keyring) with optional fingerprint through `net.reactivated.Fprint` on D-Bus; the switch is enabled when at least one method is available.
  - Turning the switch on or off always requires a successful unlock with the current method (the gating rule from `AuthenticatorUtil.authenticate`).

### T-47a-064 — Lock-when-idle delay list
- Priority: P2
- Effort: S
- Depends: T-47a-063
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - Key `lock_app_after` default `0`; entries in this order `0`, `1`, `2`, `5`, `10`, `-1` with labels "Always", "After 1 minute", "After 2 minutes", "After 5 minutes", "After 10 minutes", "Never".
  - Row enabled only when the lock switch is on.

### T-47a-065 — Hide notification content
- Priority: P1
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - Key `hide_notification_content` default `false`; label "Hide notification content".
  - When on, desktop notifications (`org.freedesktop.Notifications`) show the generic title "Kurobayomi" and body "New content" without entry or chapter titles. Proposal: these two strings are new Rust text.

### T-47a-066 — Drop the secure-screen (screenshot block) setting
- Priority: P3
- Effort: S
- Depends: T-47a-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt
- Acceptance:
  - Key `secure_screen_v2` is read and ignored; the row and its info text are removed.
  - Lost behaviour: no way to block screenshots or hide app content in the task switcher. Wayland and X11 do not offer a supported per-window capture block.
  - Requires user sign-off.

### T-47a-067 — Authentication gate semantics
- Priority: P2
- Effort: S
- Depends: T-47a-063
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt
- Acceptance:
  - `authenticate(title, subtitle)` returns true immediately when no authentication method is available.
  - It returns true only on success; on error it shows the error text and returns false. Subtitle default "Authenticate to confirm change" (`confirm_lock_change`).
  - Unit test covers the no-method, success and error branches.

### T-47a-068 — Unlock prompt used by the gates
- Priority: P2
- Effort: S
- Depends: T-47a-063
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt
- Acceptance:
  - A single Rust function `auth::confirm(title, subtitle) -> Future<bool>` is used by the lock switch, lock-when-idle, and NSFW gate. It shows the same modal dialog in every case.

---

## I. Advanced

### T-47a-069 — Share crash logs
- Priority: P1
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt
- Acceptance:
  - Row "Share crash logs" / "Saves error logs to a file for sharing with the developers" writes `$XDG_STATE_HOME/kurobayomi/crash-logs.txt` (fallback `~/.local/state/kurobayomi/`). Proposal: the Linux path.
  - File content: debug info block, then "Problematic extensions" list when any installed extension is orphaned or has an update, then the recent in-process ERROR-level log lines (Proposal: last 5000 lines kept in a ring buffer).
  - Afterwards the file manager opens the containing folder (`gtk::FileLauncher` or `xdg-open`).
  - On failure toast "Failed to get logs".

### T-47a-070 — Verbose logging
- Priority: P1
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/di/PreferenceModule.kt
- Acceptance:
  - Key `verbose_logging` default `false` in release builds (`true` in debug builds).
  - Subtitle "Print verbose logs to system log (reduces app performance)".
  - Applies live by changing the tracing filter (no restart needed on Linux).

### T-47a-071 — Onboarding guide entry
- Priority: P2
- Effort: S
- Depends: area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Row "Onboarding guide" pushes the onboarding flow (area 36) and does not change `__APP_STATE_onboarding_complete`.

### T-47a-072 — Manage notifications link
- Priority: P2
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Row "Manage notifications" runs `systemsettings kcm_notifications` on KDE Plasma and `xdg-open` on the `org.freedesktop.Notifications` settings URI where available; if neither exists the row is hidden.

### T-47a-073 — Drop battery-optimisation and "Don't kill my app" rows
- Priority: P3
- Effort: S
- Depends: T-47a-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - The "Background activity" group is removed.
  - Lost behaviour: none on the desktop; background jobs run while the app or the user session is alive. The scheduler decision is in `p1-rust-design.md` section 6.

### T-47a-074 — Reindex downloads
- Priority: P1
- Effort: S
- Depends: area:23-downloads-manga, area:24-downloads-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Row "Reindex downloads" / "Force app to recheck downloaded chapters and episodes" invalidates both manga and anime download indexes.
  - Toast "Downloads index invalidated".

### T-47a-075 — Clear manga and anime database screens
- Priority: P1
- Effort: M
- Depends: area:03-db-manga-schema-migrations, area:04-db-anime-schema-migrations, area:06-data-repos-history-track-source-misc
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearDatabaseScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/advanced/ClearAnimeDatabaseScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Lists every source that has non-library entries, sorted by source name; each row shows the source name, `"<n> non-library entries in database"` and a checkbox; the whole row toggles.
  - Select all and invert selection actions are available when the list is non-empty. The anime screen's invert action is labelled "Select inverse" (the Android build labels it "Select all"; Rust uses the correct label).
  - Delete is enabled only when at least one source is selected. Confirm dialog text: "Are you sure? Completed episodes and chapters and progress of non-library entries will be lost"; OK/Cancel.
  - Manga: deletes manga rows whose source is selected and that are not in the library, then removes reset history rows.
  - Anime: deletes anime rows for the selected sources and their non-library child seasons; children that are in the library are orphaned (parent id cleared) instead of deleted; then removes reset history rows.
  - Success toast "Entries deleted". Empty list shows "Nothing to clear".

### T-47a-076 — Clear cookies and clear WebView data
- Priority: P1
- Effort: S
- Depends: area:12-network-stack, area:36-ui-shell-navigation-security-webview
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - "Clear cookies" empties the HTTP cookie store and toasts "Cookies cleared".
  - "Clear WebView data" clears the WebKitGTK website data (cache, cookies, local storage, IndexedDB) through `WebsiteDataManager` and removes the webview cache directory; toasts "WebView data cleared"; on failure toasts "Error occurred while clearing".

### T-47a-077 — DNS over HTTPS provider
- Priority: P1
- Effort: M
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt
- Acceptance:
  - Key `doh_provider` default `-1`; entries `-1` Disabled, `1` Cloudflare, `2` Google, `3` AdGuard, `4` Quad9, `5` AliDNS, `6` DNSPod, `7` 360, `8` Quad 101, `9` Mullvad, `10` Control D, `11` Njalla, `12` Shecan, `13` LibreDNS.
  - Changing it restarts the HTTP client (or takes effect on the next request in Rust); the restart requirement is removed.
  - Provider URLs are taken from area 12 unchanged.

### T-47a-078 — User agent string
- Priority: P1
- Effort: S
- Depends: area:12-network-stack
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt
- Acceptance:
  - Key `default_user_agent` default `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0`.
  - Editing is single-line and cannot be blank. Invalid header characters (any CR, LF, or non-ASCII control) show "Invalid user agent string" and are rejected.
  - "Reset default user agent string" is enabled only when the value differs from the default; tapping it deletes the key (back to default).

### T-47a-079 — Refresh library covers
- Priority: P1
- Effort: S
- Depends: area:26-library-update-jobs, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Row "Refresh library covers" starts the manga library update, anime library update, manga metadata update and anime metadata update jobs immediately.
  - No toast; progress is shown by the library update notification (area 35).

### T-47a-080 — Reset per-series reader settings
- Priority: P1
- Effort: S
- Depends: area:43-reader-core, area:05-data-repos-entries-items
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt
- Acceptance:
  - Row "Reset per-series reader settings" / "Resets reading mode and orientation of all series" clears the stored per-series viewer flags for every manga.
  - Toast "All reader settings reset" on success; "Couldn't reset reader settings" on failure.
  - The anime equivalent is not offered (Android parity).

### T-47a-081 — Hardware bitmap threshold → maximum decode texture size
- Priority: P2
- Effort: M
- Depends: area:43-reader-core, area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Key `pref_hardware_bitmap_threshold` default `2048`.
  - Options are generated from the GL max texture size `D` (queried from the GDK GL context, floor 2048): `[D, then 1024·k for k from floor(D/1024) down to 2 where 1024·k < D]`.
  - Entry 0 is labelled "Default (<D>)"; others show the number.
  - Subtitle: "If reader loads a blank image incrementally reduce the threshold.\nSelected: <label>".
  - Row hidden when GL is unavailable.

### T-47a-082 — Legacy decoder for long strip
- Priority: P2
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Key `pref_always_decode_long_strip_with_ssiv` default `false`: "Use legacy decoder for long strip reader"; subtitle "Affects performance. Only enable if reducing bitmap threshold doesn't fix blank image issues".
  - Switches the long-strip reader to whole-image decoding when on.

### T-47a-083 — Drop the custom display profile (ICC)
- Priority: P3
- Effort: S
- Depends: area:44-reader-viewers
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - Key `pref_display_profile_key` is read and ignored; the row is removed.
  - Lost behaviour: colour-managed page display with an ICC profile. GTK 4 does not apply per-window ICC profiles; no replacement is planned.
  - Requires user sign-off.

### T-47a-084 — Revoke trusted unknown extensions
- Priority: P1
- Effort: S
- Depends: area:18-extensions-manga, area:19-extensions-anime
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/domain/extension/manga/interactor/TrustMangaExtension.kt, app/src/main/java/eu/kanade/domain/extension/anime/interactor/TrustAnimeExtension.kt
- Acceptance:
  - Row "Revoke trusted unknown extensions" clears the `trusted_extensions` app-state set for both manga and anime extensions.
  - Toast "Requires app restart to take effect" is replaced by an immediate refresh of the extension list.

### T-47a-085 — Drop the extension installer choice
- Priority: P3
- Effort: S
- Depends: area:20-extension-strategy-redesign
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/domain/base/ExtensionInstallerPreference.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt
- Acceptance:
  - The Legacy, PackageInstaller, Shizuku and Private options and the Shizuku-missing dialog are removed; key `extension_installer` is ignored.
  - Replacement for installs is defined in area 20 (extension strategy redesign), not here.
  - Requires user sign-off.

### T-47a-086 — Data Saver (image compression before load)
- Priority: P2
- Effort: M
- Depends: area:34-images-covers-cache, area:12-network-stack
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - Key `data_saver` default `NONE`; entries `NONE` Disabled, `BANDWIDTH_HERO` "Bandwidth Hero (requires a Bandwidth Hero Proxy server)", `WSRV_NL` "wsrv.nl", `RESMUSH_IT` "resmush.it".
  - Server text `data_saver_server` default empty; enabled only for `BANDWIDTH_HERO`.
  - Quality list `data_saver_image_quality` default `80`; entries 10, 20, 40, 50, 70, 80, 90, 95 labelled "10%" … "95%".
  - Compression is applied to reader and download image requests for sources not in `data_saver_excluded`.

### T-47a-087 — Data Saver flags
- Priority: P2
- Effort: S
- Depends: T-47a-086
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt, app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt
- Acceptance:
  - `data_saver_downloader` default `true` ("Use data saver in the downloader"), enabled when mode ≠ NONE.
  - `ignore_jpeg` default `false` ("Ignore Jpeg Images"), enabled when mode ≠ NONE.
  - `ignore_gif` default `true` ("Ignore Gif Animations"), enabled when mode ≠ NONE.
  - `data_saver_image_format_jpeg` default `false` ("Compress to Jpeg"), enabled when mode ≠ NONE and ≠ RESMUSH_IT; subtitle depends on the value (on/off summary strings).
  - `data_saver_color_bw` default `false` ("Convert to Black And White"), enabled only for BANDWIDTH_HERO.

---

## J. Data and storage (scope part)

### T-47a-088 — Storage location
- Priority: P0
- Effort: M
- Depends: area:13-core-util-storage-archive-torrent
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, domain/src/main/java/tachiyomi/domain/storage/service/StoragePreferences.kt
- Acceptance:
  - Row "Storage location" subtitle: "No storage location set" when `__APP_STATE_storage_dir` is not stored; otherwise the display path; or `Invalid location: <value>` when the path does not exist or is not a readable directory.
  - Tapping opens a folder chooser (`gtk::FileDialog::select_folder`); the chosen path is stored. If the chooser cannot open, toast "No file picker app found".
  - Info text "Used for automatic backups, chapter downloads, and local source."
  - Writable check: the chosen folder must be writable; otherwise the path is refused with the message "Failed to acquire persistent folder access. The app may behave unexpectedly." (reuse of the Android message, text kept).

### T-47a-089 — Storage usage, chapter cache and auto-clear
- Priority: P1
- Effort: S
- Depends: area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/cache/ChapterCache.kt, domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt
- Acceptance:
  - Row "Clear chapter and episode cache" subtitle "Used: <size>" (size formatted with binary units, computed off the GTK thread).
  - Tap clears the cache directory (keeping the journal file) and toasts "Cache cleared, <n> files deleted". Failure toasts "Error occurred while clearing".
  - Switch `auto_clear_chapter_cache` default `false`: "Clear episode/chapter cache on app launch".
  - "Storage" row opens the storage tab (area 13/42).

### T-47a-090 — Automatic backup frequency
- Priority: P1
- Effort: S
- Depends: area:21-backup-format-and-create, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreateJob.kt
- Acceptance:
  - Key `backup_interval` default `12`; entries `0` Off, `6` Every 6 hours, `12` Every 12 hours, `24` Daily, `48` Every 2 days, `168` Weekly.
  - Changing it reschedules the auto-backup job (period = interval hours, flex 10 minutes, exponential backoff 10 minutes, battery-not-low required). `0` cancels it.
  - Info text: "You should keep copies of backups in other places as well. Backups may contain sensitive data including any stored passwords; be careful if sharing." followed by a blank line and "Last automatically backed up: <relative time>".

### T-47a-091 — Library CSV export
- Priority: P2
- Effort: M
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt
- Acceptance:
  - Row "Library List" under group "Export" opens a dialog "Select data to include" with checkboxes Title, Type, Author, Artist. Unchecking Title disables and clears the other three.
  - Save writes `kurobayomi_library.csv` through a save-file dialog (default name `aniyomi_library.csv` is replaced with the Kurobayomi name in Rust).
  - CSV: no header row; one row per favourite (anime and manga), rows joined with `\r\n`, no trailing newline; columns in the order title, type (`manga` or `anime`), author, artist; empty or null values are empty fields; a field containing `\r`, `\n`, `"` or `,` is wrapped in double quotes with `"` doubled.
  - Success toast "Library Exported" (`library_exported`). Proposal: on I/O failure toast "Could not export library" (new Rust string; Android shows no error toast).
  - Favourites are loaded before the dialog can be confirmed; if they are still loading the export button is disabled.

### T-47a-092 — Storage tab and backup/restore entry points
- Priority: P1
- Effort: S
- Depends: area:47-settings-screens-general, area:42-ui-updates-history-stats-storage-upcoming
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt
- Acceptance:
  - Backup/restore group shows two segmented buttons: "Create backup" (pushes the create screen, part 2) and "Restore backup" (opens a file chooser with the title "Select backup file", then pushes the restore screen).
  - If a restore is already running, toast "Restore is already in progress" and no chooser opens.
  - Null file selection toasts "No file selected".
  - Help icon in the top bar opens `https://aniyomi.org/docs/faq/storage`.

---

## K. About

### T-47a-093 — Version row and copy debug information
- Priority: P1
- Effort: S
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt, app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt, app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt
- Acceptance:
  - Row "Version" subtitle: `Stable <VERSION_NAME> (<date>)` in release, `Preview r<COMMIT_COUNT> (<COMMIT_SHA>, <date>)` in preview, `Debug <COMMIT_SHA> (<date>)` in debug. Date is `BUILD_TIME` formatted with the app date format (T-47a-016), falling back to the raw string.
  - Tapping copies the debug information block (App ID, versions, device, WebView, time) to the clipboard with the label "Debug information"; Proposal: toast "Copied to clipboard" (new Rust string; Android `copyToClipboard` behaviour is not verified in this area).

### T-47a-094 — Check for updates
- Priority: P1
- Effort: M
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt
- Acceptance:
  - Row shows a spinner while checking; a second tap while checking does nothing.
  - Forced check runs off the UI thread. Result handling: new release → open the update screen (version, changelog, release link, download link); no new release → toast "No new updates available"; OS too old → toast "This Android version is no longer supported" (`update_check_eol`; Proposal: Rust shows the same text with "Android" removed); error → toast with the error message and log it.
  - Row is hidden when the updater is disabled at build time (`UPDATER_ENABLED`). Proposal: source builds enable it; sandboxed packages disable it. Final decision belongs to area:35-data-misc-updater-notification-export.

### T-47a-095 — External links and "What's new"
- Priority: P2
- Effort: S
- Depends: none
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt, presentation-core/src/main/java/tachiyomi/presentation/core/components/LinkIcon.kt
- Acceptance:
  - "What's new" row (hidden in debug builds) opens `https://github.com/<GITHUB_REPO>/releases/tag/<RELEASE_TAG>` (`RELEASE_TAG` is `v<VERSION_NAME>` or `r<COMMIT_COUNT>` for preview).
  - "Help translate" opens `https://aniyomi.org/docs/contribute#translation`.
  - "Privacy policy" opens `https://aniyomi.org/privacy/`.
  - Bottom link row: "Website" → `https://aniyomi.org`; "Discord" → `https://discord.gg/F32UjdJZrR`; "GitHub" → `https://github.com/aniyomiorg/aniyomi`.
  - Logo header is shown at the top (area 36 component).

### T-47a-096 — Open source licenses list
- Priority: P2
- Effort: M
- Depends: area:35-data-misc-updater-notification-export
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/OpenSourceLicensesScreen.kt
- Acceptance:
  - The list is generated at build time (`cargo about generate` over the crate graph) into a JSON document embedded with `include_str!`.
  - Each entry has name, website (optional), and license text. The list is sorted by name.
  - Tapping an entry opens the detail page (T-47a-097).

### T-47a-097 — Open source library license detail
- Priority: P2
- Effort: S
- Depends: T-47a-096
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/about/OpenSourceLibraryLicenseScreen.kt
- Acceptance:
  - Page title = library name; top-bar action "Website" (globe icon) is shown only when the library has a website; it opens the URL.
  - Body: the first license text, shown as plain text in a scrollable label with 16 px padding. HTML markup from the Android build is stripped to text (Rust build stores plain text).

---

## L. Cross-cutting (settings screens general)

### T-47a-098 — Settings screen layout at phone and desktop widths
- Priority: P1
- Effort: S
- Depends: T-47a-003
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsMainScreen.kt, app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt
- Acceptance:
  - Content column width is capped at 720 px; the sidebar keeps the settings tree visible at widths ≥ 960 px.
  - All rows reflow with no horizontal scrolling down to 360 px window width.
  - Titles wrap to a maximum of two lines and then ellipsise.
