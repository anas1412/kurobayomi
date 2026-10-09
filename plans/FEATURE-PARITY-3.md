# Feature parity matrix 3/3: settings, platform, shell

Scope: every settings item (all Aniyomi settings screens), the app shell (navigation, onboarding, app lock, WebView, startup), notifications, home-screen widgets, i18n, theming, intents and deep links, desktop packaging and updates, build and release, and the crash and debug screens.

Inputs: the area overviews `01` to `54` in this folder (the in-scope ones are listed in section 2), `catalogs/preferences-01.md` to `preferences-08.md`, `TASKS.csv` (exported 2026-10-08 by `_tools/tasks.py export`), `RISKS-AND-DECISIONS.md`, `ARCHITECTURE.md`, `addenda/requirements-cross-platform.md`, and the Aniyomi source at `/home/blackbox/aniyomi-src`.

Every path is repo-relative to the Aniyomi clone unless it starts with `plans/`, `ARCHITECTURE.md`, or another Kurobayomi document name.

## 0. How to read this document

### 0.1 Labels

* OBSERVED: what the Kotlin code does. Each OBSERVED statement carries a path and line number.
* Proposal: what Kurobayomi should do. Every proposal is labelled `Proposal:`.
* Task IDs (`T-xxx-nnn`) point at rows in `TASKS.csv`. Priorities quoted in this document are copied from that file.
* Area references (`area:<id>`) point at folders in this directory.

### 0.2 Disposition codes

| Code | Meaning |
|---|---|
| KEEP | Port the item with the same behaviour and the same stored key. |
| KEEP-RENAME | Port the item, but the label, placement or stored value changes. |
| REPLACE | The Android mechanism is removed. A named Linux-native mechanism takes its place. |
| REDUCE | Part of the item is kept, part is dropped. The row names what is kept. |
| DROP | Android-only, obsolete, or has no Linux equivalent. The user loses what the row says. The key stays in the preference registry so that backups round-trip (`area:02`, decision D6). |

### 0.3 Storage model that every settings row uses

OBSERVED: Aniyomi keeps every preference in one flat Android `SharedPreferences` file, read through `AndroidPreferenceStore` (`core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt:14`). Enum values are stored by constant name. A wrong primitive type deletes the key and returns the default (`catalogs/preferences-01.md`, section 1).

Proposal: Kurobayomi stores user preferences in `config.toml` and app state in `state.toml`, both under `$XDG_CONFIG_HOME/kurobayomi/` and `$XDG_STATE_HOME/kurobayomi/` (`ARCHITECTURE.md` A8 and section 11, `area:02` section 2.2). Key strings keep the Aniyomi spelling so that backups copy across (`area:02` decision D3). Only explicitly set keys are written (decision D2).

Conflict C1 (see section 14): `T-47a-001` and `T-48a-001` say "SQLite key/value store". `area:02`, `ARCHITECTURE.md` A8 and `T-02a-002` say TOML. This document follows TOML. The two tasks must be amended.

### 0.4 Defaults in this document

Defaults are quoted from the Kotlin accessor that the screen binds to (for example `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20`). Where a screen binds a local value (`val tryHw = decoderPreferences.tryHWDecoding()`), the accessor is resolved through that local binding. Where the binding could not be resolved, the row says `unresolved: local binding <name>` and the Kotlin file is the authority.

## 1. Summary

### 1.1 Settings rows in this document

| Group | Rows | Source file |
|---|---|---|
| Main settings menu (search row plus 11 category rows) | 12 | `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsMainScreen.kt` |
| Settings item constructions in screen files | 243 | the screen files named in the headings of section 4 |

The 243 item constructions are the `Preference.PreferenceItem.*` calls in the screen files. The count excludes the class declarations in `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt` (13 declarations), the widget file `app/src/main/java/eu/kanade/presentation/more/settings/widget/MultiSelectListPreferenceWidget.kt` (1), and the search-result row in `SettingsSearchScreen.kt:210` (1). Each constructed item has one row.

### 1.2 Task counts by in-scope area (from TASKS.csv)

| Area folder | Tasks | P0 | P1 | P2 | P3 | S | M | L | XL |
|---|---|---|---|---|---|---|---|---|---|
| `02-preferences-system` | 40 | 17 | 13 | 7 | 3 | 20 | 17 | 3 | 0 |
| `35-data-misc-updater-notification-export` | 82 | 1 | 46 | 22 | 13 | 69 | 13 | 0 | 0 |
| `36-ui-shell-navigation-security-webview` | 69 | 23 | 37 | 4 | 5 | 38 | 28 | 3 | 0 |
| `47-settings-screens-general` (p1 and p2) | 168 | 36 | 75 | 44 | 13 | 120 | 47 | 1 | 0 |
| `47b-settings-widgets-search-more` | 49 | 32 | 10 | 5 | 2 | 29 | 19 | 1 | 0 |
| `48-settings-player-and-scripts` | 58 | 20 | 25 | 7 | 6 | 28 | 26 | 4 | 0 |
| `49-theme-and-design-system` (p1 and p2) | 100 | 32 | 44 | 17 | 7 | 72 | 25 | 3 | 0 |
| `49b-android-res-values-drawables` | 50 | 13 | 19 | 10 | 8 | 43 | 7 | 0 | 0 |
| `50-android-widgets` | 35 | 0 | 3 | 24 | 8 | 30 | 4 | 1 | 0 |
| `51-i18n` | 43 | 9 | 19 | 9 | 6 | 28 | 14 | 1 | 0 |
| `52-util-app` | 66 | 15 | 32 | 5 | 14 | 49 | 17 | 0 | 0 |
| `53-platform-manifest-intents-workers` | 77 | 5 | 43 | 12 | 17 | 58 | 19 | 0 | 0 |
| `54-build-release-distribution-docs` | 51 | 3 | 29 | 10 | 9 | 44 | 7 | 0 | 0 |
| `addenda/requirements-cross-platform-tasks.md` (`T-req-platform-*`) | 11 | 4 | 6 | 0 | 1 | 3 | 6 | 1 | 1 |

The `47` row includes both `p1-tasks.md` and `p2-tasks.md`. The `49` row includes both `p1` and `p2` files. The `T-req-platform-*` rows are the standing cross-platform requirement and are listed in section 12.4.

### 1.3 Disposition counts for the 243 settings rows

Counted from the Disposition column of the tables in section 4 (SET-001 to SET-243).

| Disposition | Rows |
|---|---|
| KEEP | 191 |
| KEEP-RENAME | 9 |
| REPLACE | 20 |
| REDUCE | 8 |
| DROP | 15 |
| Total | 243 |

Proposal: the row count and the disposition counts are checked by the test `settings_rows_match_inventory` (`T-47a-002`, one row per constructed item).

## 2. Areas covered by this document

| Area | Overview | Role in this document |
|---|---|---|
| `02-preferences-system` | `02-preferences-system/00-overview.md` | Store, key registry, defaults, migrations, backup mapping |
| `35-data-misc-updater-notification-export` | `35-data-misc-updater-notification-export/00-overview.md` | Notifications, app updater, library CSV export |
| `36-ui-shell-navigation-security-webview` | `36-ui-shell-navigation-security-webview/00-overview.md` | Shell, startup, onboarding, app lock, WebView, deep-link routing |
| `47-settings-screens-general` | `47-settings-screens-general/00-overview.md` | General settings screens and their rows |
| `47b-settings-widgets-search-more` | `47b-settings-widgets-search-more/00-overview.md` | Settings widgets, extension store and repo pages, backup pages, debug pages |
| `48-settings-player-and-scripts` | `48-settings-player-and-scripts/00-overview.md` | Player settings, mpv files, custom buttons, scripts |
| `49-theme-and-design-system` | `49-theme-and-design-system/00-overview.md` | Colour tokens, widgets, layout |
| `49b-android-res-values-drawables` | `49b-android-res-values-drawables/00-overview.md` | Resource values, theme tokens, drawable replacement |
| `50-android-widgets` | `50-android-widgets/00-overview.md` | Home-screen widgets and their Linux replacements |
| `51-i18n` | `51-i18n/00-overview.md` | Strings, locales, plurals, language picker |
| `52-util-app` | `52-util-app/00-overview.md` | Shared utilities and platform helpers |
| `53-platform-manifest-intents-workers` | `53-platform-manifest-intents-workers/00-overview.md` | Manifest, intents, deep links, background jobs, notification receiver |
| `54-build-release-distribution-docs` | `54-build-release-distribution-docs/00-overview.md` | Build, packaging, CI, release, documentation |
| `addenda/requirements-cross-platform.md` | `addenda/requirements-cross-platform.md` | Windows-portability rule that applies to every platform row |

## 3. Main settings menu

OBSERVED: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsMainScreen.kt`. The menu has a search row (`:96`, icon `Icons.Outlined.Search`, navigates to `SettingsSearchScreen`) and eleven category rows built from the `Item(titleRes, subtitleRes, icon, screen)` list (`:179` to `:246`).

Proposal: the Kurobayomi main menu is a sidebar "More" page (`T-47ba-010`, `T-47a-018`). Its rows keep the same order and the same subtitle keys.

| ID | Source (line) | Row title key | Subtitle key | Icon | Target | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| MAIN-01 | :96 | (search row, no title key) | none | Search | `SettingsSearchScreen` | KEEP | Search entry in the settings header | T-47a-004 (P1), T-47b-071 (P2) |
| MAIN-02 | :180 | `MR.strings.pref_category_appearance` | `MR.strings.pref_appearance_summary` | Palette | Appearance page | KEEP | Appearance page | T-47b-001 (P0) |
| MAIN-03 | :186 | `MR.strings.pref_category_library` | `AYMR.strings.pref_library_summary` | CollectionsBookmark | Library page | KEEP | Library page | T-47b-001 (P0) |
| MAIN-04 | :192 | `MR.strings.pref_category_reader` | `MR.strings.pref_reader_summary` | ChromeReaderMode | Reader page | KEEP | Reader page | T-47b-001 (P0) |
| MAIN-05 | :198 | `AYMR.strings.label_player` | `AYMR.strings.pref_player_settings_summary` | VideoSettings | Player root (section 4.9) | KEEP | Player root | T-48a-010 (P0) |
| MAIN-06 | :204 | `MR.strings.pref_category_downloads` | `MR.strings.pref_downloads_summary` | GetApp | Downloads page | KEEP | Downloads page | T-47a-044 (P1) |
| MAIN-07 | :210 | `MR.strings.pref_category_tracking` | `MR.strings.pref_tracking_summary` | Sync | Tracking page | KEEP | Tracking page | T-47a-053 (P1) |
| MAIN-08 | :216 | `MR.strings.browse` | `MR.strings.pref_browse_summary` | Explore | Browse page | KEEP | Browse page | T-47a-060 (P0) |
| MAIN-09 | :222 | `MR.strings.label_data_storage` | `MR.strings.pref_backup_summary` | Storage | Data and storage page | KEEP | Data and storage page | T-47a-092 (P1) |
| MAIN-10 | :228 | `MR.strings.pref_category_security` | `MR.strings.pref_security_summary` | Security | Security page | KEEP | Security page (app lock is P2) | T-47a-063 (P2) |
| MAIN-11 | :234 | `MR.strings.pref_category_advanced` | `MR.strings.pref_advanced_summary` | Code | Advanced page | KEEP | Advanced page | T-47a-069 (P1) |
| MAIN-12 | :240 | `MR.strings.pref_category_about` | none (icon `Icons.Outlined.Info`, `:246`) | Info | About page (section 4.13) | KEEP | About page | T-47a-094 (P1) |

Proposal: the Appearance row `MAIN-02` is the first item on the sidebar. The Linux settings router (`T-47b-001`, P0) owns the page list. The category order above is the order of the Kotlin list.

## 4. Settings item rows (243)

Each table lists one row per constructed item in one screen file. The file path is given once per table. Row IDs (`SET-nnn`) are unique across the document and are used in acceptance tests.

Column meanings: `Source` is the line of the `Preference.PreferenceItem.*` construction. `Item` is the Kotlin title key, or the literal string if the Kotlin source hard-codes the text. `Widget` is the Kotlin item class. `Store key` is the string passed to the preference store (`n/a` for navigation and info rows). `Kotlin default` is the default passed to the store. `Disposition` is one of the codes in 0.2.

### 4.1 Appearance: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAppearanceScreen.kt` (8 rows)

Backing accessors: `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt`.

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-001 | :69 | `MR.strings.pref_app_theme` (theme strip: mode row and app theme row) | Custom | `pref_theme_mode_key` (mode) and `pref_app_theme` (app theme) | mode `ThemeMode.SYSTEM` (`UiPreferences.kt:20`); app theme `AppTheme.MONET` when `DeviceUtil.isDynamicColorAvailable`, else `AppTheme.DEFAULT` (`UiPreferences.kt:22` to `:29`) | KEEP-RENAME | Mode row: System, Light, Dark. App theme: GTK-following base plus the palette list (decided in area 49). Monet becomes an XDG portal accent option. | T-47ba-040 (P0), T-47ba-041 (P0), T-47ba-042 (P0), T-47a-011 (P0), T-49b-007 (P0), T-49b-006 (P0), T-49b-005 (P1) |
| SET-002 | :88 | `MR.strings.pref_dark_theme_pure_black` | Switch | `pref_theme_dark_amoled_key` | `false` (`UiPreferences.kt:31`) | KEEP | AMOLED switch. Enabled only when mode is not Light (`enabled = themeMode != ThemeMode.LIGHT`, `:92`). | T-47a-014 (P2), T-49b-015 (P1), T-49ba-003 (P1), T-47ba-042 (P0) |
| SET-003 | :118 | `MR.strings.pref_app_language` | Text | none (navigation to `AppLanguageScreen`) | n/a | KEEP | App language page (`T-47b-005` to `T-47b-008`) | T-47a-019 (P1), T-51a-013 (P1), T-51a-021 (P1) |
| SET-004 | :122 | `MR.strings.pref_tablet_ui_mode` | List | `tablet_ui_mode` | `TabletUiMode.AUTOMATIC` (`UiPreferences.kt:37`) | DROP | Responsive layout only. Replaces the tablet switch with breakpoints. | T-47a-017 (P3), T-48a-007 (P3), T-36a-031 (P0) |
| SET-005 | :133 | `AYMR.strings.pref_start_screen` | List | `start_screen` | `StartScreen.ANIME` (`UiPreferences.kt:39`) | KEEP | Start screen selector | T-47a-015 (P1), T-48a-005 (P0), T-36a-004 (P0) |
| SET-006 | :144 | `"Navigation Style"` (literal, not a string key) | List | `bottom_rail_nav_style` | `NavStyle.MOVE_HISTORY_TO_MORE` (`UiPreferences.kt:41`) | REPLACE | Sidebar with a "More" entry. The bottom bar does not exist on desktop. | T-47a-018 (P2), T-36a-006 (P0), T-48a-006 (P1), T-49b-018 (P3) |
| SET-007 | :152 | `MR.strings.pref_date_format` | List | `app_date_format` | `""` (empty means locale default) (`UiPreferences.kt:35`) | KEEP | Date format list | T-47a-016 (P1), T-51a-017 (P1), T-48a-008 (P1) |
| SET-008 | :162 | `MR.strings.pref_relative_format` | Switch | `relative_time_v2` | `true` (`UiPreferences.kt:33`) | KEEP | Relative timestamps toggle | T-47a-016 (P1), T-51a-042 (P1), T-48a-009 (P1) |

### 4.2 Advanced: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt` (30 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-009 | :102 | `MR.strings.pref_dump_crash_logs` | Text | n/a (action) | n/a | KEEP | Share crash logs action | T-47a-069 (P1), T-52a-017 (P1) |
| SET-010 | :111 | `MR.strings.pref_verbose_logging` | Switch | `verbose_logging` | `false` (constructor default of `NetworkPreferences`, `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:8`) | KEEP | Verbose logging switch; writes `tracing` level DEBUG to `logs/kurobayomi.log` (`ARCHITECTURE.md` section 11) | T-47a-070 (P1), T-53a-014 (P2) |
| SET-011 | :120 | `MR.strings.pref_debug_info` | Text | n/a (navigation) | n/a | KEEP | Debug info page | T-47b-040 (P2), T-47b-041 (P2), T-47a-093 (P1) |
| SET-012 | :124 | `MR.strings.pref_onboarding_guide` | Text | n/a (action) | n/a | KEEP | Reopen onboarding | T-47a-071 (P2), T-36a-024 (P0) |
| SET-013 | :128 | `MR.strings.pref_manage_notifications` | Text | n/a (opens system notification settings) | n/a | REPLACE | Opens the desktop notification settings (KDE Plasma Notifications, GNOME Settings) through `Opener` (`T-35a-010`) | T-47a-072 (P2), T-35a-011 (P1) |
| SET-014 | :157 | `MR.strings.pref_disable_battery_optimization` | Text | n/a | n/a | DROP | None. Linux has no battery-optimisation kill list. | T-47a-073 (P3) |
| SET-015 | :179 | `MR.strings.about_dont_kill_my_app` | Text | n/a | n/a | DROP | None. Linux has no manufacturer background kill. | T-47a-073 (P3) |
| SET-016 | :196 | `MR.strings.pref_invalidate_download_cache` | Text | n/a (action) | n/a | REPLACE | "Reindex downloads" action that rescans the download folder | T-47a-074 (P1) |
| SET-017 | :205 | `AYMR.strings.pref_clear_manga_database` | Text | n/a (opens `ClearDatabaseScreen`) | n/a | KEEP | Clear manga database page | T-47a-075 (P1) |
| SET-018 | :210 | `AYMR.strings.pref_clear_anime_database` | Text | n/a (opens `ClearAnimeDatabaseScreen`) | n/a | KEEP | Clear anime database page | T-47a-075 (P1) |
| SET-019 | :232 | `MR.strings.pref_clear_cookies` | Text | n/a (action) | n/a | REPLACE | Clears the cookie jar of the `reqwest` client and the WebKitGTK cookie store | T-47a-076 (P1), T-36a-052 (P1) |
| SET-020 | :239 | `MR.strings.pref_clear_webview_data` | Text | n/a (action) | n/a | REPLACE | Clears the WebKitGTK data manager (cache, local storage) | T-47a-076 (P1), T-36a-052 (P1) |
| SET-021 | :259 | (no title key; the item is a DoH list) | List | `doh_provider` | `-1` (`NetworkPreferences.kt:15`, `getInt("doh_provider", -1)`) | KEEP | DNS over HTTPS provider list | T-47a-077 (P1); network stack in `area:12-network-stack` |
| SET-022 | :283 | `MR.strings.pref_user_agent_string` | EditText | unresolved: local binding `userAgentPref` | unresolved: local binding `userAgentPref` | KEEP-RENAME | User agent field. Default user agent changes to Kurobayomi (`T-54a-068`). | T-47a-078 (P1), T-54a-068 (P1) |
| SET-023 | :298 | `MR.strings.pref_reset_user_agent_string` | Text | n/a (action) | n/a | KEEP | Reset user agent to the Kurobayomi default | T-47a-078 (P1) |
| SET-024 | :318 | `MR.strings.pref_refresh_library_covers` | Text | n/a (action) | n/a | KEEP | Refresh library covers action | T-47a-079 (P1); cover cache in `area:34-images-covers-cache` |
| SET-025 | :327 | `MR.strings.pref_reset_viewer_flags` | Text | n/a (action, clears per-series reader flags) | n/a | KEEP | Reset per-series reader settings | T-47a-080 (P1) |
| SET-026 | :365 | `MR.strings.pref_hardware_bitmap_threshold` | List | `pref_hardware_bitmap_threshold` | `GLUtil.SAFE_TEXTURE_LIMIT` (Android GL limit, `:365` binding `basePreferences.hardwareBitmapThreshold()`) | REPLACE | Maximum decode texture size, chosen from the GPU limit reported by GTK (`T-47a-081`) | T-47a-081 (P2) |
| SET-027 | :385 | `MR.strings.pref_always_decode_long_strip_with_ssiv_2` | Switch | `pref_always_decode_long_strip_with_ssiv` | `false` (`:385` binding `basePreferences.alwaysDecodeLongStripWithSSIV()`) | REDUCE | Legacy decoder switch for long strip. Kept as a switch. The Subsampling Scale Image View library has no Rust equivalent; the Kurobayomi decoder path is defined in `area:44`. | T-47a-082 (P2) |
| SET-028 | :390 | `MR.strings.pref_display_profile` | Text | n/a | n/a | DROP | None. ICC display profiles are not applied in the Kurobayomi renderer (`T-47a-083`). | T-47a-083 (P3) |
| SET-029 | :442 | `MR.strings.ext_installer_pref` | List | unresolved: local binding `extensionInstallerPref` | unresolved: local binding `extensionInstallerPref` | DROP | None. APK install is replaced by the source model (`area:20`). | T-47a-085 (P3), T-35a-056 (P3) |
| SET-030 | :467 | `MR.strings.ext_revoke_trust` | Text | n/a (action) | n/a | KEEP-RENAME | Revoke trust of unknown extensions. Applies to the Kurobayomi trusted-source list (`area:18`, `area:19`). | T-47a-084 (P1) |
| SET-031 | :487 | `AYMR.strings.data_saver` | List | `data_saver` | `DataSaver.NONE` (`:487` binding `sourcePreferences.dataSaver()`) | REDUCE | Keeps the enum with `NONE` so that backups parse. The three third-party image services are dropped (section 14, C3). | T-47a-086 (P2), T-51a-038 (P2) |
| SET-032 | :498 | `AYMR.strings.bandwidth_data_saver_server` | EditText | `data_saver_server` | `""` (`:498` binding `sourcePreferences.dataSaverServer()`) | DROP | None. The server field exists only for the dropped image services. | T-47a-087 (P2) |
| SET-033 | :504 | `AYMR.strings.data_saver_downloader` | Switch | `data_saver_downloader` | `true` (`:504` binding `sourcePreferences.dataSaverDownloader()`) | REDUCE | Kept as a stored flag so that backups parse. No runtime effect while the service list is `NONE`. | T-47a-087 (P2) |
| SET-034 | :509 | `AYMR.strings.data_saver_ignore_jpeg` | Switch | `ignore_jpeg` | `false` (`:509`) | REDUCE | Stored flag, no runtime effect | T-47a-087 (P2) |
| SET-035 | :514 | `AYMR.strings.data_saver_ignore_gif` | Switch | `ignore_gif` | `true` (`:514`) | REDUCE | Stored flag, no runtime effect | T-47a-087 (P2) |
| SET-036 | :519 | `AYMR.strings.data_saver_image_quality` | List | `data_saver_image_quality` | `80` (`:519`) | REDUCE | Stored value, no runtime effect while the service list is `NONE` | T-47a-086 (P2) |
| SET-037 | :537 | `AYMR.strings.data_saver_image_format` | Switch | `data_saver_image_format_jpeg` | `false` (`:537`) | REDUCE | Stored flag, no runtime effect | T-47a-087 (P2) |
| SET-038 | :548 | `AYMR.strings.data_saver_color_bw` | Switch | `data_saver_color_bw` | `false` (`:548`) | REDUCE | Stored flag, no runtime effect | T-47a-087 (P2) |

Rows SET-022 and SET-029 bind a local variable. The local is resolved in the same file; the accessor was not captured by the extractor, so the key is `unresolved` in this table and must be filled in when `T-02a-030` binds the mockup page. This is recorded as open item O-3 in section 14.

### 4.3 Browse: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsBrowseScreen.kt` (6 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-039 | :50 | `AYMR.strings.pref_hide_in_anime_library_items` | Switch | `browse_hide_in_anime_library_items` | `false` (`:50` binding `sourcePreferences.hideInAnimeLibraryItems()`) | KEEP | Hide entries already in library, anime lists | T-47a-060 (P0) |
| SET-040 | :54 | `AYMR.strings.pref_hide_in_manga_library_items` | Switch | `browse_hide_in_library_items` | `false` (`:54`) | KEEP | Hide entries already in library, manga lists | T-47a-060 (P0) |
| SET-041 | :58 | `AYMR.strings.label_anime_extension_repos` | Text | n/a (opens anime store page) | n/a | KEEP-RENAME | Anime extension stores page (Kurobayomi term "store") | T-47b-009 to T-47b-018 (P0) |
| SET-042 | :69 | `AYMR.strings.label_manga_extension_repos` | Text | n/a (opens manga repo page) | n/a | KEEP | Manga extension repos page | T-47b-019 to T-47b-025 (P0) |
| SET-043 | :85 | `MR.strings.pref_show_nsfw_source` | Switch | `show_nsfw_source` | `true` (`:85` binding `sourcePreferences.showNsfwSource()`) | KEEP | Show NSFW sources, gated by the auth rule (section 4.14) | T-47a-061 (P1) |
| SET-044 | :95 | (info row, no title) | Info | n/a | n/a | KEEP | Info text on the browse page | T-47a-062 (P2) |

### 4.4 Data and storage: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt` (11 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-045 | :113 | (info row) | Info | n/a | n/a | KEEP | Info text | T-47a-092 (P1) |
| SET-046 | :173 | `MR.strings.pref_storage_location` | Text | n/a (opens the storage picker) | n/a | REPLACE | Folder chooser through `org.freedesktop.portal.FileChooser` (`T-53a-027`) | T-47a-088 (P0), T-53a-027 (P1) |
| SET-047 | :177 | `MR.strings.pref_storage_location` (second construction with the same title) | Text | n/a | n/a | REPLACE | Second row for the storage root of the anime library (same as SET-046) | T-47a-088 (P0) |
| SET-048 | :217 | `restorePreferenceKeyString` (restore entry, custom) | Custom | n/a | n/a | KEEP | Restore backup entry | T-47a-092 (P1), T-47b-031 (P1) |
| SET-049 | :261 | `MR.strings.pref_backup_interval` | List | `backup_interval` | `12` (`domain/src/main/java/tachiyomi/domain/backup/service/BackupPreferences.kt:10`, `getInt("backup_interval", 12)`) | KEEP | Automatic backup frequency | T-47a-090 (P1) |
| SET-050 | :277 | (info row) | Info | n/a | n/a | KEEP | Info text | T-47a-092 (P1) |
| SET-051 | :300 | `MR.strings.pref_storage_usage` | Custom | n/a | n/a | KEEP | Storage usage bars (`T-47b-038`) | T-47b-038 (P1), T-47b-037 (P1) |
| SET-052 | :312 | `AYMR.strings.label_storage` | Text | n/a (navigation) | n/a | KEEP | Storage tab | T-47a-092 (P1) |
| SET-053 | :320 | `AYMR.strings.pref_clear_chapter_cache` | Text | n/a (action) | n/a | KEEP | Clear chapter cache action | T-47a-089 (P1) |
| SET-054 | :338 | `AYMR.strings.pref_auto_clear_chapter_cache` | Switch | `auto_clear_chapter_cache` | `false` (`LibraryPreferences.kt:93`, `getBoolean("auto_clear_chapter_cache", false)`) | KEEP | Auto-clear chapter cache at launch (`T-36a-016`) | T-47a-089 (P1), T-36a-016 (P1) |
| SET-055 | :406 | `MR.strings.library_list` | Text | n/a (navigation) | n/a | KEEP | Library CSV export entry (`T-47a-091`, export topic in `area:35`). Mapping of this title key to the export page is recorded as open item O-4 (section 14). | T-47a-091 (P2), T-35a-070 to T-35a-077 (P2) |

### 4.5 Downloads: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDownloadScreen.kt` (25 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-056 | :79 | `MR.strings.connected_to_wifi` | Switch | `pref_download_only_over_wifi_key` | `true` (`DownloadPreferences.kt:9` to `:11`) | REPLACE | "Download only on unmetered connections" through GIO `NetworkMonitor.network-metered` (`area:02` Q-D) | T-47a-044 (P1), T-53a-025 (P2), T-53a-026 (P1) |
| SET-057 | :83 | `AYMR.strings.download_speed_limit` | Text | binding not confirmed (the extractor matched the next item's argument) | n/a | KEEP | Chapter download speed limit page | T-47a-045 (P1) |
| SET-058 | :92 | `MR.strings.save_chapter_as_cbz` | Switch | `save_chapter_as_cbz` | `true` (`:92` binding `downloadPreferences.saveChaptersAsCBZ()`) | KEEP | Save chapters as CBZ | T-47a-046 (P1) |
| SET-059 | :96 | `MR.strings.split_tall_images` | Switch | `split_tall_images` | `true` (`:96` binding `downloadPreferences.splitTallImages()`) | KEEP | Split tall images | T-47a-046 (P1) |
| SET-060 | :101 | `AYMR.strings.pref_download_slots` | List | `download_slots` | `1` (`:101` binding `downloadPreferences.numberOfDownloads()`) | KEEP | Maximum concurrent downloads | T-47a-047 (P1) |
| SET-061 | :106 | (info row) | Info | n/a | n/a | KEEP | Downloads info text | T-47a-052 (P1) |
| SET-062 | :134 | `AYMR.strings.pref_remove_after_marked_as_read` | Switch | `pref_remove_after_marked_as_read_key` | `false` (`:134` binding `downloadPreferences.removeAfterMarkedAsRead()`) | KEEP | Delete after marked as read | T-47a-048 (P1) |
| SET-063 | :138 | `AYMR.strings.pref_remove_after_read` | List | `remove_after_read_slots` | `-1` (`:138` binding `downloadPreferences.removeAfterReadSlots()`) | KEEP | Delete-after-read slot count (`-1` means off) | T-47a-048 (P1) |
| SET-064 | :150 | `AYMR.strings.pref_remove_bookmarked_chapters` | Switch | `pref_remove_bookmarked` | `false` (`:150` binding `downloadPreferences.removeBookmarkedChapters()`) | KEEP | Remove bookmarked items when deleting after read | T-47a-048 (P1) |
| SET-065 | :154 | `AYMR.strings.pref_download_fillermarked_items` | Switch | `pref_download_fillermarked` | `false` (`:154` binding `downloadPreferences.downloadFillermarkedItems()`) | KEEP | Download filler-marked episodes | T-47a-049 (P1), `area:24` |
| SET-066 | :174 | `AYMR.strings.pref_remove_exclude_categories_manga` | MultiSelect | `REMOVE_EXCLUDE_MANGA_CATEGORIES_PREF_KEY` | `emptySet()` (`DownloadPreferences.kt:39` to `:42`) | KEEP | Exclude manga categories from delete-after-read | T-47a-048 (P1) |
| SET-067 | :175 | `AYMR.strings.pref_remove_exclude_categories_manga` (second construction with the same title) | MultiSelect | same as SET-066 | `emptySet()` | KEEP | Duplicate construction; the Rust page renders one control bound to SET-066 | T-47a-048 (P1) |
| SET-068 | :188 | `AYMR.strings.pref_remove_exclude_categories_anime` | MultiSelect | `REMOVE_EXCLUDE_ANIME_CATEGORIES_PREF_KEY` | `emptySet()` (`DownloadPreferences.kt:43` to `:46`) | KEEP | Exclude anime categories from delete-after-read | T-47a-048 (P1) |
| SET-069 | :189 | `AYMR.strings.pref_remove_exclude_categories_anime` (second construction) | MultiSelect | same as SET-068 | `emptySet()` | KEEP | Duplicate construction; one control bound to SET-068 | T-47a-048 (P1) |
| SET-070 | :269 | `AYMR.strings.pref_download_new_episodes` | Switch | unresolved: local binding `downloadNewEpisodesPref` | unresolved | KEEP | Auto-download new episodes | T-47a-049 (P1) |
| SET-071 | :273 | `AYMR.strings.pref_download_new_unseen_episodes_only` | Switch | unresolved: local binding `downloadNewUnseenEpisodesOnlyPref` | unresolved | KEEP | Auto-download only unseen episodes | T-47a-049 (P1) |
| SET-072 | :278 | `AYMR.strings.anime_categories` | Text | unresolved: local binding `downloadNewChaptersPref` | unresolved | KEEP | Category include and exclude for anime auto-download | T-47a-049 (P1) |
| SET-073 | :288 | `MR.strings.pref_download_new` | Switch | unresolved: local binding `downloadNewChaptersPref` | unresolved | KEEP | Auto-download new chapters | T-47a-049 (P1) |
| SET-074 | :292 | `MR.strings.pref_download_new_unread_chapters_only` | Switch | unresolved: local binding `downloadNewUnreadChaptersOnlyPref` | unresolved | KEEP | Auto-download only unread chapters | T-47a-049 (P1) |
| SET-075 | :297 | `AYMR.strings.manga_categories` | Text | n/a | n/a | KEEP | Category include and exclude for manga auto-download | T-47a-049 (P1) |
| SET-076 | :318 | `AYMR.strings.auto_download_while_watching` | List | `auto_download_while_watching` | `0` (`:318` binding `downloadPreferences.autoDownloadWhileWatching()`) | KEEP | Download-ahead count for anime (`0` is off) | T-47a-050 (P1) |
| SET-077 | :331 | `MR.strings.auto_download_while_reading` | List | `auto_download_while_reading` | `0` (`:331` binding `downloadPreferences.autoDownloadWhileReading()`) | KEEP | Download-ahead count for manga | T-47a-050 (P1) |
| SET-078 | :344 | (info row) | Info | n/a | n/a | KEEP | Auto-download info text and enable rules | T-47a-052 (P1) |
| SET-079 | :380 | `AYMR.strings.pref_use_external_downloader` | Switch | `use_external_downloader` | `false` (`:380`) | DROP | None. IDM and ADM are Windows and Android tools. | T-47a-051 (P3) |
| SET-080 | :384 | `AYMR.strings.pref_external_downloader_selection` | List | unresolved: local binding `externalDownloaderPreference` | unresolved | DROP | None (same as SET-079). The registry keeps the key for round trip. | T-47a-051 (P3) |

### 4.6 Library: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsLibraryScreen.kt` (21 rows)

Backing accessors: `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt`.

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-081 | :106 | `AYMR.strings.action_edit_anime_categories` | Text | n/a (opens category editor) | n/a | KEEP | Category editor (anime) | T-47a-020 (P0) |
| SET-082 | :115 | `AYMR.strings.default_anime_category` | List | `default_anime_category` | `-1` (`LibraryPreferences.kt:414` and `:154`) | KEEP-RENAME | Default category for new anime. Stored by category name (`area:02` section 5). | T-47a-020 (P0), T-02a-009 (P0) |
| SET-083 | :120 | `AYMR.strings.action_edit_manga_categories` | Text | n/a | n/a | KEEP | Category editor (manga) | T-47a-020 (P0) |
| SET-084 | :132 | `AYMR.strings.default_manga_category` | List | `default_category` (`LibraryPreferences.kt:413`) | `-1` (`LibraryPreferences.kt:153`) | KEEP-RENAME | Default category for new manga, stored by category name | T-47a-020 (P0), T-02a-009 (P0) |
| SET-085 | :137 | `MR.strings.categorized_display_settings` | Switch | `categorized_display` | `false` (`LibraryPreferences.kt`, `categorizedDisplaySettings`) | KEEP | Per-category display settings | T-47a-021 (P1) |
| SET-086 | :149 | `AYMR.strings.pref_category_hide_hidden` | Switch | `hidden_categories` | `false` (`hideHiddenCategoriesSettings`) | KEEP | Hide hidden categories | T-47a-022 (P1) |
| SET-087 | :225 | `MR.strings.pref_library_update_interval` | List | `pref_library_update_interval_key` | `0` (`LibraryPreferences.kt:40`, `getInt("pref_library_update_interval_key", 0)`) | KEEP | Automatic library update interval | T-47a-023 (P1) |
| SET-088 | :242 | `MR.strings.pref_library_update_restriction` | MultiSelect | `library_update_restriction` | `setOf(DEVICE_ONLY_ON_WIFI)` (`autoUpdateDeviceRestrictions`) | REPLACE | Device restrictions: "Only on unmetered connection" (GIO `network-metered`), "Only when charging" (UPower), "Only when not on battery" (UPower) | T-47a-024 (P1), T-53a-025 (P2) |
| SET-089 | :261 | `AYMR.strings.anime_categories` | Text | n/a (opens anime include and exclude dialog) | n/a | KEEP | Anime include and exclude categories for automatic update | T-47a-025 (P1) |
| SET-090 | :270 | `AYMR.strings.manga_categories` | Text | `auto_update_metadata` (the Kotlin row carries the metadata binding) | `false` (`LibraryPreferences.kt`, `autoUpdateMetadata`) | KEEP | Manga include and exclude categories for automatic update | T-47a-025 (P1) |
| SET-091 | :279 | `MR.strings.pref_library_update_refresh_metadata` | Switch | `auto_update_metadata` | `false` | KEEP | Refresh metadata during library update | T-47a-026 (P1) |
| SET-092 | :284 | `MR.strings.pref_library_update_smart_update` | MultiSelect | `library_update_manga_restriction` | set of `ENTRY_HAS_UNVIEWED`, `ENTRY_NON_COMPLETED`, `ENTRY_NON_VIEWED`, `ENTRY_OUTSIDE_RELEASE_PERIOD` (the set is the value of `autoUpdateItemRestrictions`, `LibraryPreferences.kt:50`) | KEEP | Smart update rules | T-47a-026 (P1) |
| SET-093 | :294 | `AYMR.strings.pref_library_update_show_tab_badge` | Switch | `library_show_updates_count` | `true` (`LibraryPreferences.kt`, `newShowUpdatesCount`) | KEEP | Updates tab badge | T-47a-026 (P1), T-36a-007 (P1) |
| SET-094 | :309 | `AYMR.strings.pref_update_seasons_refresh` | Switch | `pref_update_season_on_refresh` | `false` (`updateSeasonOnRefresh`) | KEEP | Update seasons on refresh | T-47a-027 (P1) |
| SET-095 | :313 | `AYMR.strings.pref_update_seasons_update` | Switch | `pref_update_season_on_library_update` | `false` (`updateSeasonOnLibraryUpdate`) | KEEP | Update seasons on library update | T-47a-027 (P1) |
| SET-096 | :328 | `MR.strings.pref_chapter_swipe_start` | List | `pref_chapter_swipe_end_action` (the start row reads the end key, OBSERVED at `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:369` to `:370`) | `ChapterSwipeAction.ToggleRead` | KEEP | Chapter swipe start action. The crossed key is kept for backup compatibility (section 14, C8). Entries: `Disabled`, `ToggleBookmark`, `ToggleRead`, `Download`. | T-47a-028 (P0) |
| SET-097 | :342 | `MR.strings.pref_chapter_swipe_end` | List | `pref_chapter_swipe_start_action` (crossed, same as SET-096) | `ChapterSwipeAction.ToggleBookmark` | KEEP | Chapter swipe end action | T-47a-028 (P0) |
| SET-098 | :356 | `MR.strings.pref_mark_duplicate_read_chapter_read` | MultiSelect | `mark_duplicate_read_chapter_read` | `emptySet()` | KEEP | Mark duplicate chapter as read (`T-47a-029`) | T-47a-029 (P1) |
| SET-099 | :377 | (no title key in the source) | List | `pref_episode_swipe_end_action` (crossed, OBSERVED in `LibraryPreferences.kt` `swipeEpisodeStartAction`) | `EpisodeSwipeAction.ToggleSeen` | KEEP | Episode swipe start action. Entries: `Disabled`, `ToggleSeen`, `ToggleBookmark`, `ToggleFillermark`, `Download`. | T-47a-028 (P0) |
| SET-100 | :393 | (no title key in the source) | List | `pref_episode_swipe_start_action` (crossed) | `EpisodeSwipeAction.ToggleBookmark` | KEEP | Episode swipe end action | T-47a-028 (P0) |
| SET-101 | :409 | `AYMR.strings.pref_mark_duplicate_seen_episode_seen` | MultiSelect | `mark_duplicate_seen_episode_seen` | `emptySet()` | KEEP | Mark duplicate episode as seen | T-47a-029 (P1) |

Rows SET-099 and SET-100 have no title key in the Kotlin source; the title is taken from the section header `AYMR.strings.pref_behavior_episode` (`:340` region). Proposal: label them "Swipe start" and "Swipe end" under that header.

### 4.7 Reader: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt` (46 rows)

Backing accessors: `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt`. The reader engine is `area:43` and `area:44`. The settings here are rows only.

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-102 | :36 | `MR.strings.pref_viewer_type` | List | `pref_default_reading_mode_key` | `ReadingMode.RIGHT_TO_LEFT.flagValue` (`ReaderPreferences.kt:38`, `getInt`) | KEEP | Default reading mode for new series | T-47a-031 (P0), T-02a-013 (P0) |
| SET-103 | :43 | `MR.strings.pref_double_tap_anim_speed` | List | `pref_double_tap_anim_speed` | `500` (`ReaderPreferences.kt:26`) | KEEP | Double-tap animation speed | T-47a-032 (P1) |
| SET-104 | :52 | `MR.strings.pref_show_reading_mode` | Switch | `pref_show_reading_mode` | `true` (`showReadingMode`) | KEEP | Show reading mode label | T-47a-034 (P0) |
| SET-105 | :57 | `MR.strings.pref_show_navigation_mode` | Switch | `reader_navigation_overlay_on_start` | `false` (`showNavigationOverlayOnStart`) | KEEP | Show the tap-zone overlay on start | T-47a-033 (P0) |
| SET-106 | :62 | `MR.strings.pref_page_transitions` | Switch | `pref_enable_transitions_key` | `true` (`pageTransitions`) | KEEP | Page transitions | T-47a-033 (P0) |
| SET-107 | :83 | `MR.strings.pref_rotation_type` | List | `pref_default_orientation_type_key` | `ReaderOrientation.FREE.flagValue` (`defaultOrientationType`) | KEEP | Rotation type. Desktop rotation uses the window and the page transform, not a device sensor. | T-47a-034 (P0) |
| SET-108 | :90 | `MR.strings.pref_reader_theme` | List | `pref_reader_theme_key` | `1` (`readerTheme`) | KEEP | Reader background | T-47a-034 (P0) |
| SET-109 | :100 | `MR.strings.pref_fullscreen` | Switch | unresolved: local binding `fullscreenPref` | unresolved | KEEP | Fullscreen while reading. Maps to `gtk::Window::fullscreen`. | T-47a-034 (P0), T-53a-057 (P1) |
| SET-110 | :104 | `MR.strings.pref_cutout_short` | Switch | `cutout_short` | `true` (`cutoutShort`) | DROP | None. Display cutout does not exist on the desktop (`T-47a-043`). | T-47a-043 (P3) |
| SET-111 | :111 | `MR.strings.pref_keep_screen_on` | Switch | `pref_keep_screen_on_key` | `true` (`keepScreenOn`) | REPLACE | Keep the screen awake while reading through the freedesktop screensaver inhibit (`IdleInhibitor`, `T-52a-038`) | T-47a-034 (P0), T-52a-038 (P2) |
| SET-112 | :115 | `MR.strings.pref_show_page_number` | Switch | `pref_show_page_number_key` | `true` (`showPageNumber`) | KEEP | Show page number | T-47a-034 (P0) |
| SET-113 | :138 | `MR.strings.pref_flash_page` | Switch | `pref_reader_flash` | `false` (`flashOnPageChange`) | KEEP | E-ink flash on page change | T-47a-035 (P2) |
| SET-114 | :143 | `MR.strings.pref_flash_duration` | Slider | unresolved (no binding in the item) | unresolved | KEEP | Flash duration slider | T-47a-035 (P2) |
| SET-115 | :154 | `MR.strings.pref_flash_page_interval` | Slider | unresolved: local binding `flashColorPref` | unresolved | KEEP | Flash every N pages | T-47a-035 (P2) |
| SET-116 | :165 | `MR.strings.pref_flash_with` | List | unresolved: local binding `flashColorPref` | unresolved | KEEP | Flash colour (black or white) | T-47a-035 (P2) |
| SET-117 | :185 | `MR.strings.pref_skip_read_chapters` | Switch | `skip_read` | `false` (`ReaderPreferences.kt`, `skipRead`) | KEEP | Skip read chapters | T-47a-036 (P0) |
| SET-118 | :189 | `MR.strings.pref_skip_filtered_chapters` | Switch | `skip_filtered` | `true` (`ReaderPreferences.kt:85`) | KEEP | Skip filtered chapters | T-47a-036 (P0) |
| SET-119 | :193 | `MR.strings.pref_skip_dupe_chapters` | Switch | `skip_dupe` | `false` (`ReaderPreferences.kt:87`) | KEEP | Skip duplicate chapters | T-47a-036 (P0) |
| SET-120 | :197 | `MR.strings.pref_always_show_chapter_transition` | Switch | `always_show_chapter_transition` | `true` | KEEP | Always show chapter transition | T-47a-036 (P0) |
| SET-121 | :201 | `AYMR.strings.pref_preserve_reading_position` | Switch | `pref_preserve_reading_position` | `false` | KEEP | Preserve reading position | T-47a-036 (P0) |
| SET-122 | :224 | `MR.strings.pref_viewer_nav` (paged viewer) | List | unresolved: local binding `navModePref` | unresolved | KEEP | Navigation layout for the paged viewer (tap zones) | T-47a-037 (P0) |
| SET-123 | :232 | `MR.strings.pref_read_with_tapping_inverted` (paged) | List | `reader_tapping_inverted` | `TappingInvertMode.NONE` (`pagerNavInverted`) | KEEP | Inverted tapping, paged viewer | T-47a-037 (P0) |
| SET-124 | :245 | `MR.strings.pref_image_scale_type` | List | unresolved: local binding `imageScaleTypePref` | unresolved | KEEP | Image scale type | T-47a-037 (P0) |
| SET-125 | :253 | `MR.strings.pref_zoom_start` | List | `pref_zoom_start_key` | `1` (`ReaderPreferences.kt:55`, `zoomStart`) | KEEP | Zoom start | T-47a-037 (P0) |
| SET-126 | :261 | `MR.strings.pref_crop_borders` (paged) | Switch | `crop_borders` | `false` (`ReaderPreferences.kt:69`) | KEEP | Crop borders, paged | T-47a-037 (P0) |
| SET-127 | :265 | `MR.strings.pref_landscape_zoom` | Switch | `landscape_zoom` | `true` (`ReaderPreferences.kt:73`) | KEEP | Landscape zoom | T-47a-037 (P0) |
| SET-128 | :270 | `MR.strings.pref_navigate_pan` | Switch | `navigate_pan` | `true` (`ReaderPreferences.kt:71`) | KEEP | Navigate by panning | T-47a-037 (P0) |
| SET-129 | :275 | `MR.strings.pref_dual_page_split` (paged) | Switch | unresolved: local binding `dualPageSplitPref` | unresolved | KEEP | Split double pages | T-47a-037 (P0) |
| SET-130 | :283 | `MR.strings.pref_dual_page_invert` (paged) | Switch | `pref_dual_page_invert` | `false` (`ReaderPreferences.kt:97`) | KEEP | Invert double pages | T-47a-037 (P0) |
| SET-131 | :289 | `MR.strings.pref_page_rotate` (paged) | Switch | unresolved: local binding `rotateToFitPref` | unresolved | KEEP | Rotate to fit | T-47a-037 (P0) |
| SET-132 | :297 | `MR.strings.pref_page_rotate_invert` (paged) | Switch | `pref_dual_page_rotate_invert` | `false` (`ReaderPreferences.kt:105`) | KEEP | Invert rotate-to-fit | T-47a-037 (P0) |
| SET-133 | :323 | `MR.strings.pref_viewer_nav` (long strip) | List | unresolved: local binding `navModePref` | unresolved | KEEP | Navigation layout for long strip | T-47a-038 (P0) |
| SET-134 | :331 | `MR.strings.pref_read_with_tapping_inverted` (long strip) | List | `reader_tapping_inverted_webtoon` | `TappingInvertMode.NONE` (`webtoonNavInverted`) | KEEP | Inverted tapping, long strip | T-47a-038 (P0) |
| SET-135 | :344 | `MR.strings.pref_webtoon_side_padding` | Slider | `reader_hide_threshold` | `ReaderHideThreshold.LOW` (`ReaderPreferences.kt:79`) | KEEP | Side padding for long strip | T-47a-038 (P0) |
| SET-136 | :356 | `MR.strings.pref_hide_threshold` | List | `reader_hide_threshold` | `ReaderHideThreshold.LOW` (`ReaderPreferences.kt:79`) | KEEP | Hide threshold for long strip | T-47a-038 (P0) |
| SET-137 | :366 | `MR.strings.pref_crop_borders` (long strip) | Switch | `crop_borders_webtoon` | `false` (`ReaderPreferences.kt:75`) | KEEP | Crop borders, long strip | T-47a-038 (P0) |
| SET-138 | :370 | `MR.strings.pref_dual_page_split` (long strip) | Switch | unresolved: local binding `dualPageSplitPref` | unresolved | KEEP | Split double pages, long strip | T-47a-038 (P0) |
| SET-139 | :378 | `MR.strings.pref_dual_page_invert` (long strip) | Switch | `pref_dual_page_invert_webtoon` | `false` (`ReaderPreferences.kt:101`) | KEEP | Invert double pages, long strip | T-47a-038 (P0) |
| SET-140 | :384 | `MR.strings.pref_page_rotate` (long strip) | Switch | unresolved: local binding `rotateToFitPref` | unresolved | KEEP | Rotate to fit, long strip | T-47a-038 (P0) |
| SET-141 | :392 | `MR.strings.pref_page_rotate_invert` (long strip) | Switch | `pref_dual_page_rotate_invert_webtoon` | `false` | KEEP | Invert rotate-to-fit, long strip | T-47a-038 (P0) |
| SET-142 | :397 | `MR.strings.pref_double_tap_zoom` | Switch | `pref_enable_double_tap_zoom_webtoon` | `true` (`ReaderPreferences.kt:48`) | KEEP | Double-tap zoom, long strip | T-47a-038 (P0) |
| SET-143 | :401 | `MR.strings.pref_webtoon_disable_zoom_out` | Switch | `webtoon_disable_zoom_out` | `false` (`ReaderPreferences.kt:89`) | KEEP | Disable zoom out, long strip | T-47a-038 (P0) |
| SET-144 | :416 | `MR.strings.pref_read_with_volume_keys` | Switch | unresolved: local binding `readWithVolumeKeysPref` | unresolved | REPLACE | Volume keys turn pages (keyboard media keys through GTK key events; the volume keys are optional and not guaranteed on the desktop) | T-47a-039 (P1) |
| SET-145 | :420 | `MR.strings.pref_read_with_volume_keys_inverted` | Switch | `reader_volume_keys_inverted` | `false` (`readWithVolumeKeysInverted`) | KEEP | Inverted volume keys | T-47a-039 (P1) |
| SET-146 | :434 | `MR.strings.pref_read_with_long_tap` | Switch | `reader_long_tap` | `true` (`ReaderPreferences.kt:142`) | KEEP | Long tap action | T-47a-040 (P1) |
| SET-147 | :438 | `MR.strings.pref_create_folder_per_manga` | Switch | `create_folder_per_manga` | `false` (`ReaderPreferences.kt:81`) | KEEP | Create a folder per manga for downloads and CBZ export | T-47a-040 (P1) |

Rows SET-129, SET-131, SET-138, SET-140 and SET-109 use the same local bindings as the paged and long-strip screens. The keys are resolved in `T-02a-013` (reader value tables). Open item O-3 covers them.

Row count note: the Reader table lists 46 rows (SET-102 to SET-147). Rows SET-114 to SET-116 are the flash group and SET-109 is the fullscreen switch.

### 4.8 Security: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsSecurityScreen.kt` (5 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-148 | :39 | `MR.strings.lock_with_biometrics` | Switch | unresolved: local binding `useAuthPref` | unresolved | REPLACE | "Require unlock" through the Linux app lock authenticator. Biometrics are not available on the desktop. Authenticator options are a product decision (`area:02` Q-C). | T-47a-063 (P2), T-36a-033 (P1), T-52a-033 (P2) |
| SET-149 | :49 | (no title key in the source) | List | `lock_app_after` (`core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt:15`) | `0` (`getInt("lock_app_after", 0)`) | KEEP | Lock when idle, delay list | T-47a-064 (P2) |
| SET-150 | :72 | `MR.strings.hide_notification_content` | Switch | `hide_notification_content` | `false` (`SecurityPreferences.kt`) | KEEP | Hide notification content. Applies to `Notice` (`T-35a-009`). | T-47a-065 (P1), T-35a-009 (P1) |
| SET-151 | :76 | `MR.strings.secure_screen` | List | `secure_screen_v2` | `SecureScreenMode.INCOGNITO` (`SecurityPreferences.kt`) | DROP | None. Linux has no screenshot block for normal windows. The key stays in the registry. | T-47a-066 (P3), T-36a-035 (P3) |
| SET-152 | :83 | (info row) | Info | n/a | n/a | KEEP | Info text about the lock | T-47a-067 (P2) |

### 4.9 Player root and sub-pages (65 rows)

The player root is `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsMainScreen.kt`. It lists nine rows (`:186` to `:237`): Internal player (`pref_player_internal`, `:186`, opens `PlayerSettingsPlayerScreen`), Gestures (`:192`, `PlayerSettingsGesturesScreen`), Decoder (`:198`, `PlayerSettingsDecoderScreen`), Subtitle (`:204`, `PlayerSettingsSubtitleScreen`), Audio (`:210`, `PlayerSettingsAudioScreen`), Custom buttons (`:216`, `PlayerSettingsCustomButtonScreen`), Editor (`:222`, `PlayerSettingsEditorScreen`), Torrent (`:228`, `PlayerSettingsTorrentScreen`), Advanced (`:234`, `PlayerSettingsAdvancedScreen`). Those nine entries are rows MAIN-P1 to MAIN-P9 below.

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| MAIN-P1 | PlayerSettingsMainScreen.kt:186 | `AYMR.strings.pref_player_internal` | Nav | n/a | n/a | KEEP | Player page | T-48a-010 (P0) |
| MAIN-P2 | PlayerSettingsMainScreen.kt:192 | `AYMR.strings.pref_player_gestures` | Nav | n/a | n/a | KEEP | Gestures page | T-48a-010 (P0) |
| MAIN-P3 | PlayerSettingsMainScreen.kt:198 | `AYMR.strings.pref_player_decoder` | Nav | n/a | n/a | KEEP | Decoder page | T-48a-010 (P0) |
| MAIN-P4 | PlayerSettingsMainScreen.kt:204 | `AYMR.strings.pref_player_subtitle` | Nav | n/a | n/a | KEEP | Subtitle page | T-48a-010 (P0) |
| MAIN-P5 | PlayerSettingsMainScreen.kt:210 | `AYMR.strings.pref_player_audio` | Nav | n/a | n/a | KEEP | Audio page | T-48a-010 (P0) |
| MAIN-P6 | PlayerSettingsMainScreen.kt:216 | `AYMR.strings.pref_player_custom_button` | Nav | n/a | n/a | KEEP | Custom buttons page (`T-48a-045`) | T-48a-045 (P1) |
| MAIN-P7 | PlayerSettingsMainScreen.kt:222 | `AYMR.strings.pref_player_editor` | Nav | n/a | n/a | KEEP | Script and code editor (`T-48a-037`, `T-48a-038`) | T-48a-037 (P1) |
| MAIN-P8 | PlayerSettingsMainScreen.kt:228 | `AYMR.strings.pref_player_torrents` | Nav | n/a | n/a | KEEP | Torrent page (P2) | T-48a-032 (P2) |
| MAIN-P9 | PlayerSettingsMainScreen.kt:234 | `AYMR.strings.pref_player_advanced` | Nav | n/a | n/a | KEEP | Advanced page | T-48a-035 (P1) |

#### 4.9.1 Internal player: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsPlayerScreen.kt` (30 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-153 | :49 | `AYMR.strings.pref_progress_mark_as_seen` | List | `pref_progress_preference` | `0.85F` (`PlayerPreferences.kt:15`) | KEEP | Mark episode as seen at this watched fraction | T-48a-015 (P0) |
| SET-154 | :62 | `AYMR.strings.pref_preserve_watching_position` | Switch | `pref_preserve_watching_position` | `false` (`preserveWatchingPosition`) | KEEP | Preserve watching position | T-48a-015 (P0) |
| SET-155 | :66 | `AYMR.strings.pref_category_player_orientation` | List | `pref_default_player_orientation_type_key` | `PlayerOrientation.SensorLandscape` (`defaultPlayerOrientationType`) | DROP | None. Device orientation is not a desktop concept. | T-48a-019 (P3) |
| SET-156 | :96 | `AYMR.strings.pref_controls_allow_gestures_in_panels` | Switch | `pref_allow_gestures_in_panels` | `false` (`PlayerPreferences.kt`) | KEEP | Gestures inside panels | T-48a-013 (P0) |
| SET-157 | :100 | `AYMR.strings.pref_controls_show_loading` | Switch | unresolved: local binding `showLoading` | unresolved | KEEP | Show loading indicator | T-48a-013 (P0) |
| SET-158 | :104 | `AYMR.strings.pref_controls_show_chapter_indicator` | Switch | unresolved: local binding `showChapter` | unresolved | KEEP | Show chapter indicator | T-48a-013 (P0) |
| SET-159 | :109 | `AYMR.strings.pref_remember_brightness` | Switch | `pref_remember_brightness` | `false` (`PlayerPreferences.kt`) | KEEP | Remember brightness (decision in `T-48a-052`) | T-48a-052 (P2) |
| SET-160 | :113 | `AYMR.strings.pref_remember_volume` | Switch | `pref_remember_volume` | `false` (`PlayerPreferences.kt:28`) | KEEP | Remember volume (decision in `T-48a-052`) | T-48a-052 (P2) |
| SET-161 | :129 | `AYMR.strings.pref_hosters_show_failure` | Switch | unresolved: local binding `showFailure` | unresolved | KEEP | Show hoster failure | T-48a-013 (P0) |
| SET-162 | :133 | `AYMR.strings.pref_hosters_show_empty` | Switch | unresolved: local binding `showEmpty` | unresolved | KEEP | Show empty hoster list | T-48a-013 (P0) |
| SET-163 | :157 | `AYMR.strings.pref_player_fullscreen` | Switch | unresolved: local binding `fullScreen` | unresolved | KEEP | Fullscreen playback | T-48a-014 (P0) |
| SET-164 | :162 | `AYMR.strings.pref_player_hide_controls` | Switch | `player_hide_controls` | `false` (`PlayerPreferences.kt:39`) | KEEP | Hide controls | T-48a-014 (P0) |
| SET-165 | :166 | `AYMR.strings.pref_controls_display_volume_percentage` | Switch | unresolved: local binding `displayVol` | unresolved | KEEP | Show volume percentage | T-48a-014 (P0) |
| SET-166 | :170 | `AYMR.strings.pref_show_system_bar` | Switch | unresolved: local binding `showSystemBar` | unresolved | DROP | None. There is no Android system bar. | T-48a-020 (P3) |
| SET-167 | :174 | `AYMR.strings.pref_reduce_motion` | Switch | `pref_reduce_motion` | `false` (`PlayerPreferences.kt:42`) | KEEP | Reduce motion. Honours the GTK animations setting (`T-49ba-046`). | T-48a-014 (P0), T-49a-057 (P2) |
| SET-168 | :178 | `AYMR.strings.pref_player_time_to_disappear` | List | unresolved: local binding `hideTime` | unresolved | KEEP | Controls hide delay | T-48a-014 (P0) |
| SET-169 | :185 | `AYMR.strings.pref_panel_opacity` | Slider | unresolved (no binding in the item) | unresolved | KEEP | Panel opacity | T-48a-014 (P0) |
| SET-170 | :216 | `AYMR.strings.pref_enable_intro_skip` | Switch | `pref_enable_skip_intro` | `true` (`PlayerPreferences.kt:48`) | KEEP | Enable intro skip | T-48a-016 (P1) |
| SET-171 | :220 | `AYMR.strings.pref_enable_auto_skip_ani_skip` | Switch | unresolved: local binding `enableAutoAniSkip` | unresolved | KEEP | Auto skip with AniSkip | T-48a-016 (P1) |
| SET-172 | :225 | `AYMR.strings.pref_enable_netflix_style_aniskip` | Switch | unresolved: local binding `enableNetflixAniSkip` | unresolved | KEEP | Netflix-style skip button | T-48a-016 (P1) |
| SET-173 | :230 | `AYMR.strings.pref_waiting_time_aniskip` | List | unresolved: local binding `waitingTimeAniSkip` | unresolved | KEEP | AniSkip waiting time | T-48a-016 (P1) |
| SET-174 | :243 | `AYMR.strings.pref_enable_aniskip` | Switch | unresolved: local binding `enableAniSkip` | unresolved | KEEP | Enable AniSkip | T-48a-016 (P1) |
| SET-175 | :248 | `AYMR.strings.pref_disable_aniskip_chapter` | Switch | unresolved: local binding `disableAniSkipChapters` | unresolved | KEEP | Disable AniSkip for chapter-based skips | T-48a-016 (P1) |
| SET-176 | :253 | `AYMR.strings.pref_category_player_aniskip_info` | Info | n/a | n/a | KEEP | AniSkip info text | T-48a-016 (P1) |
| SET-177 | :273 | `AYMR.strings.pref_enable_pip` | Switch | `pref_enable_pip` | `true` (`PlayerPreferences.kt:60`) | REPLACE | Always-on-top mini player (`T-48a-017`) | T-48a-017 (P2) |
| SET-178 | :277 | `AYMR.strings.pref_pip_episode_toasts` | Switch | `pref_pip_episode_toasts` | `true` (`PlayerPreferences.kt:61`) | REPLACE | Episode toasts in the mini player | T-48a-017 (P2) |
| SET-179 | :282 | `AYMR.strings.pref_pip_on_exit` | Switch | `pref_pip_on_exit` | `false` (`PlayerPreferences.kt:62`) | REPLACE | Keep the mini player open when leaving the app | T-48a-017 (P2) |
| SET-180 | :287 | `AYMR.strings.pref_pip_replace_with_previous` | Switch | `pip_replace_with_previous` | `false` | REPLACE | Replace with previous episode in the mini player | T-48a-017 (P2) |
| SET-181 | :319 | `AYMR.strings.pref_always_use_external_player` | Switch | `pref_always_use_external_player` | `false` (`PlayerPreferences.kt`) | REPLACE | Always use the external player (`T-48a-018`) | T-48a-018 (P1) |
| SET-182 | :323 | `AYMR.strings.pref_external_player_preference` | List | `external_player_preference` | `""` (`PlayerPreferences.kt`) | REPLACE | External player command template with `$URL` (`area:02` Q-F). Android package name is replaced by a command. | T-48a-018 (P1), T-48a-053 (P3) |

Rows SET-171 to SET-175 are the AniSkip group; the store keys for those rows are unresolved in this document and are filled by `T-48a-016`.

#### 4.9.2 Gestures: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsGesturesScreen.kt` (15 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-183 | :61 | `AYMR.strings.enable_volume_brightness_gestures` | Switch | unresolved: local binding `enableVolumeBrightnessGestures` | unresolved | KEEP | Volume and brightness gestures (pointer drag on the video area) | T-48a-021 (P1) |
| SET-184 | :65 | `AYMR.strings.pref_controls_swap_vol_brightness` | Switch | unresolved: local binding `swapVol` | unresolved | KEEP | Swap volume and brightness sides | T-48a-021 (P1) |
| SET-185 | :97 | `AYMR.strings.enable_horizontal_seek_gesture` | Switch | unresolved: local binding `enableHorizontalSeekGesture` | unresolved | KEEP | Horizontal seek gesture | T-48a-022 (P0) |
| SET-186 | :102 | `AYMR.strings.pref_show_seekbar` | Switch | unresolved: local binding `showSeekbar` | unresolved | KEEP | Show seek bar | T-48a-022 (P0) |
| SET-187 | :106 | `AYMR.strings.pref_default_intro_length` | Text | `pref_skip_length_preference` | `10` (`GesturePreferences.kt`) | KEEP | Default intro length | T-48a-022 (P0) |
| SET-188 | :111 | `AYMR.strings.pref_skip_length` | List | `pref_skip_length_preference` | `10` (`GesturePreferences.kt:22`) | KEEP | Skip length | T-48a-022 (P0) |
| SET-189 | :123 | `AYMR.strings.pref_player_smooth_seek` | Switch | `pref_player_smooth_seek` | `false` (`GesturePreferences.kt:23`) | KEEP | Smooth seek | T-48a-022 (P0) |
| SET-190 | :141 | `AYMR.strings.pref_left_double_tap` | List | unresolved: local binding `leftDoubleTap` | unresolved | KEEP | Left double-tap action | T-48a-023 (P0) |
| SET-191 | :152 | `AYMR.strings.pref_center_double_tap` | List | unresolved: local binding `centerDoubleTap` | unresolved | KEEP | Centre double-tap action | T-48a-023 (P0) |
| SET-192 | :161 | `AYMR.strings.pref_right_double_tap` | List | unresolved: local binding `rightDoubleTap` | unresolved | KEEP | Right double-tap action | T-48a-023 (P0) |
| SET-193 | :172 | `AYMR.strings.pref_double_tap_info` | Info | n/a | n/a | KEEP | Double-tap info text | T-48a-023 (P0) |
| SET-194 | :188 | `AYMR.strings.pref_media_previous` | List | unresolved: local binding `mediaPrevious` | unresolved | KEEP | Media key: previous | T-48a-024 (P1) |
| SET-195 | :199 | `AYMR.strings.pref_media_playpause` | List | unresolved: local binding `mediaPlayPause` | unresolved | KEEP | Media key: play and pause | T-48a-024 (P1) |
| SET-196 | :208 | `AYMR.strings.pref_media_next` | List | unresolved: local binding `mediaNext` | unresolved | KEEP | Media key: next | T-48a-024 (P1) |
| SET-197 | :219 | `AYMR.strings.pref_media_info` | Info | n/a | n/a | KEEP | MPRIS and media keys info text | T-48a-024 (P1) |

#### 4.9.3 Decoder: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsDecoderScreen.kt` (4 rows)

Backing accessors: `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt`.

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-198 | :32 | `AYMR.strings.pref_try_hw` | Switch | `pref_try_hwdec` | `true` (`DecoderPreferences.kt:10`) | KEEP | mpv `hwdec` on | T-48a-026 (P1) |
| SET-199 | :36 | `AYMR.strings.pref_gpu_next_title` | Switch | `pref_gpu_next` | `false` (`DecoderPreferences.kt:11`) | KEEP | mpv `gpu-next` | T-48a-027 (P1) |
| SET-200 | :41 | `AYMR.strings.pref_debanding_title` | List | `pref_video_debanding` | `Debanding.None` (`DecoderPreferences.kt:12`) | KEEP | Debanding filter chain | T-48a-028 (P1) |
| SET-201 | :49 | `AYMR.strings.pref_use_yuv420p_title` | Switch | `use_yuv420p` | `true` (`DecoderPreferences.kt:13`) | KEEP | YUV 420P filter | T-48a-028 (P1) |

#### 4.9.4 Subtitle: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsSubtitleScreen.kt` (3 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-202 | :31 | `AYMR.strings.pref_player_subtitle_lang_info` | EditTextInfo | unresolved: local binding `langPref` | unresolved | KEEP | Preferred subtitle languages | T-48a-029 (P1), T-48a-030 (P0) |
| SET-203 | :72 | `AYMR.strings.pref_player_subtitle_whitelist_info` | EditTextInfo | unresolved: local binding `whitelist` | unresolved | KEEP | Subtitle whitelist | T-48a-029 (P1) |
| SET-204 | :77 | `AYMR.strings.pref_player_subtitle_blacklist_info` | EditTextInfo | unresolved: local binding `blacklist` | unresolved | KEEP | Subtitle blacklist | T-48a-029 (P1) |

#### 4.9.5 Audio: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsAudioScreen.kt` (4 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-205 | :37 | `AYMR.strings.pref_player_audio_lang_info` | EditTextInfo | unresolved: local binding `prefLangs` | unresolved | KEEP | Preferred audio languages | T-48a-031 (P1) |
| SET-206 | :78 | `AYMR.strings.pref_player_audio_pitch_correction` | Switch | unresolved: local binding `pitchCorrection` | unresolved | KEEP | Pitch correction (mpv `audio-pitch-correction`) | T-48a-031 (P1) |
| SET-207 | :83 | `AYMR.strings.pref_player_audio_channels` | List | `pref_audio_config` | `AudioChannels.AutoSafe` (`AudioPreferences.kt:13`) | KEEP | Audio channels. The mpv mapping swap is kept (`area:02` Q-E). | T-48a-031 (P1) |
| SET-208 | :90 | `AYMR.strings.pref_player_audio_boost_cap` | Slider | unresolved (no binding in the item) | unresolved | KEEP | Volume boost cap | T-48a-031 (P1) |

#### 4.9.6 Advanced (player): `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsAdvancedScreen.kt` (3 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-209 | :33 | `AYMR.strings.pref_mpv_user_files` | Switch | unresolved: local binding `enableUserFiles` | unresolved | KEEP | Enable user mpv files | T-48a-035 (P1), T-48a-036 (P0) |
| SET-210 | :38 | `AYMR.strings.pref_mpv_conf` | MPVConf | `pref_mpv_conf` | `""` (`AdvancedPlayerPreferences.kt:9`) | KEEP | mpv.conf editor | T-48a-035 (P1), T-48a-051 (P0) |
| SET-211 | :45 | `AYMR.strings.pref_mpv_input` | MPVConf | `pref_mpv_input` | `""` (`AdvancedPlayerPreferences.kt:10`) | KEEP | input.conf editor | T-48a-035 (P1) |

#### 4.9.7 Torrent: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsTorrentScreen.kt` (6 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-212 | :76 | `AYMR.strings.pref_player_torrents_enable` | Switch | unresolved: local binding `torrentEnablePref` | unresolved | KEEP | Enable torrent streaming (requires `area:13` torrent decision) | T-48a-032 (P2) |
| SET-213 | :90 | `AYMR.strings.pref_player_torrents_port_summary` | EditTextInfo | unresolved: local binding `portPref` | unresolved | KEEP | Torrent server port (validated by `T-48a-034`) | T-48a-034 (P2) |
| SET-214 | :110 | `AYMR.strings.pref_player_torrents_trackers` | MultiLine | unresolved: local binding `trackersPref` | unresolved | KEEP | Tracker list | T-48a-032 (P2) |
| SET-215 | :126 | `AYMR.strings.pref_player_torrents_trackers_reset` | Text | unresolved: local binding `proxyModePref` | unresolved | KEEP | Reset tracker list | T-48a-032 (P2) |
| SET-216 | :135 | `AYMR.strings.pref_player_torrents_proxy_mode` | List | unresolved: local binding `proxyModePref` | unresolved | KEEP | Proxy mode | T-48a-034 (P2) |
| SET-217 | :149 | `AYMR.strings.pref_player_torrents_proxy_url` | EditTextInfo | unresolved: local binding `proxyUrlPref` | unresolved | KEEP | Proxy URL (validated by `T-48a-034`) | T-48a-034 (P2) |

Counting check for 4.9: the nine root rows are MAIN-P1 to MAIN-P9, and the sub-pages contribute 30 + 15 + 4 + 3 + 4 + 3 + 6 = 65 rows (SET-153 to SET-217). The row IDs `SET-153` to `SET-217` are 65 IDs.

### 4.10 Tracking: `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt` (15 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-218 | :139 | `AYMR.strings.pref_auto_update_manga_sync` | Switch | `pref_auto_update_manga_sync_key` | `true` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:39`) | KEEP | Auto-update tracker progress | T-47a-054 (P1) |
| SET-219 | :143 | `AYMR.strings.pref_track_on_add_library` | Switch | `track_on_adding_to_library` | `true` (`TrackPreferences.kt:41`) | KEEP | Track on adding to library | T-47a-053 (P1) |
| SET-220 | :147 | `AYMR.strings.pref_show_next_episode_airing_time` | Switch | `show_next_episode_airing_time` | `true` (`TrackPreferences.kt:43`) | KEEP | Show next episode airing time | T-52a-013 (P1), T-47a-053 (P1) |
| SET-221 | :151 | `AYMR.strings.pref_auto_update_manga_on_mark_read` | List | `pref_auto_update_manga_on_mark_read` | `AutoTrackState.ALWAYS` (`TrackPreferences.kt`) | KEEP | Auto-update progress when marked read or watched | T-47a-054 (P1) |
| SET-222 | :161 | (TrackerPreference row 1, service identity from the tracker manager) | Tracker | unresolved (per-service keys in the tracker registry) | unresolved | KEEP | Login and logout row for one tracker service (`area:27`) | T-47a-055 (P1), T-47a-056 (P1) |
| SET-223 | :171 | (TrackerPreference row 2) | Tracker | unresolved | unresolved | KEEP | Login and logout row | T-47a-055 (P1) |
| SET-224 | :181 | (TrackerPreference row 3) | Tracker | unresolved | unresolved | KEEP | Login and logout row | T-47a-055 (P1) |
| SET-225 | :186 | (TrackerPreference row 4) | Tracker | unresolved | unresolved | KEEP | Login and logout row | T-47a-055 (P1) |
| SET-226 | :191 | (TrackerPreference row 5) | Tracker | unresolved | unresolved | KEEP | Login and logout row | T-47a-055 (P1) |
| SET-227 | :201 | (TrackerPreference row 6) | Tracker | unresolved | unresolved | KEEP | Login and logout row | T-47a-055 (P1) |
| SET-228 | :211 | (TrackerPreference row 7) | Tracker | unresolved | unresolved | KEEP | Login and logout row | T-47a-055 (P1) |
| SET-229 | :221 | `MR.strings.enhanced_services` | Info | n/a | n/a | KEEP | Enhanced trackers section header (source-linked services) | T-47a-057 (P1) |
| SET-230 | :229 | (TrackerPreference row 8, enhanced service) | Tracker | unresolved | unresolved | KEEP | Enhanced tracker row (areas 33) | T-47a-057 (P1), `area:33-tracker-enhanced-kavita-komga-suwayomi-jellyfin` |
| SET-231 | :237 | (TrackerPreference row 9, enhanced service) | Tracker | unresolved | unresolved | KEEP | Enhanced tracker row (area 33) | T-47a-057 (P1) |
| SET-232 | :243 | (info row) | Info | n/a | n/a | KEEP | Tracking guide link | T-47a-058 (P1) |

The nine tracker rows are constructed from the tracker manager (`area:27`), so their service names are not literal in the screen file. Open item O-5 (section 14) records that the per-service key names are filled in by `T-47a-055`.

### 4.11 Debug information: `app/src/main/java/eu/kanade/presentation/more/settings/screen/debug/DebugInfoScreen.kt` (11 rows)

| ID | Source | Item | Widget | Store key | Kotlin default | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|---|---|---|---|---|
| SET-233 | :33 | `WorkerInfoScreen.TITLE` | Text | n/a (navigation to `WorkerInfoScreen`) | n/a | KEEP-RENAME | Background job page (`T-47b-043`, `T-47b-044`) | T-47b-043 (P2), T-47b-044 (P1) |
| SET-234 | :37 | `BackupSchemaScreen.TITLE` | Text | n/a (navigation) | n/a | KEEP | Backup file schema page | T-47b-045 (P2) |
| SET-235 | :53 | `"Version"` (literal) | Text | n/a (info) | n/a | KEEP | Version row | T-47b-040 (P2), T-47a-093 (P1) |
| SET-236 | :57 | `"Build time"` (literal) | Text | n/a (info) | n/a | KEEP | Build time row from `build.rs` | T-47b-049 (P2) |
| SET-237 | :62 | `"WebView version"` (literal) | Text | n/a (info) | n/a | KEEP-RENAME | WebKitGTK version row | T-47b-048 (P2) |
| SET-238 | :77 | (no title; device info block) | Text | n/a | n/a | KEEP | Device rows for Linux (kernel, desktop, GTK, session type) | T-47b-041 (P2) |
| SET-239 | :107 | `"Profile compilation status"` (literal) | Text | n/a | n/a | DROP | None. ART profile compilation does not exist on Linux. | T-47b-042 (P3) |
| SET-240 | :116 | `"Model"` (literal) | Text | n/a (info) | n/a | KEEP-RENAME | Hardware model row (DMI product name) | T-47b-041 (P2) |
| SET-241 | :124 | `"OneUI version"` (literal) | Text | n/a | n/a | DROP | None. Samsung vendor build string. | T-47b-051 (P3) |
| SET-242 | :131 | `"MIUI version"` (literal) | Text | n/a | n/a | DROP | None. Xiaomi vendor build string. | T-47b-051 (P3) |
| SET-243 | :146 | `"Android version"` (literal) | Text | n/a (info) | n/a | REPLACE | Operating system row (`/etc/os-release` PRETTY_NAME and kernel release) | T-47b-041 (P2) |

Row SET-243 replaces the Android version row. Proposal: the Linux row shows the distribution name from `/etc/os-release`.

### 4.12 Extension store and repo pages

These pages are not `PreferenceItem` rows in the Kotlin screens. They are built by `AnimeExtensionStoresScreen.kt`, `MangaExtensionReposScreen.kt` and their models under `app/src/main/java/eu/kanade/presentation/more/settings/screen/browse/`. Their rows and dialogs are fully planned in `47b` (`T-47b-009` to `T-47b-025`, `T-47b-057`, `T-47b-062` to `T-47b-069`) and `47a` (`T-47a-062`).

### 4.13 About page

OBSERVED: `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt`, `OpenSourceLicensesScreen.kt`, `OpenSourceLibraryLicenseScreen.kt`.

| Row | Disposition | Linux target | Tasks (pri) |
|---|---|---|---|
| Version row | KEEP | Version from `build.rs` (`T-54a-002`) | T-47a-093 (P1) |
| Check for updates | KEEP-RENAME | Update check through GitHub releases of Kurobayomi, throttled 3 days (`T-35a-045`) | T-47a-094 (P1), T-35a-047 (P1) |
| External links and What's new | KEEP | Website, changelog, and release notes | T-47a-095 (P2), T-35a-044 (P2) |
| Open source licenses list | KEEP-RENAME | Licence list from `cargo-about`-style embedded data (`T-54a-057`) | T-47a-096 (P2), T-54a-057 (P2) |
| Open source library license detail | KEEP | Licence text view | T-47a-097 (P2) |

### 4.14 Authentication gate and NSFW rules

OBSERVED: `SettingsBrowseScreen.kt:85` (`show_nsfw_source`, default `true`) and the security screen rows above.

Proposal: NSFW sources stay hidden until the app lock gate (section 6.5) is satisfied when the lock is on. Authentication-gated rows are listed in `T-47a-067`.

### 4.15 Settings row total

| Section | Rows |
|---|---|
| 4.1 Appearance | 8 (SET-001 to SET-008) |
| 4.2 Advanced | 30 (SET-009 to SET-038) |
| 4.3 Browse | 6 (SET-039 to SET-044) |
| 4.4 Data and storage | 11 (SET-045 to SET-055) |
| 4.5 Downloads | 25 (SET-056 to SET-080) |
| 4.6 Library | 21 (SET-081 to SET-101) |
| 4.7 Reader | 46 (SET-102 to SET-147) |
| 4.8 Security | 5 (SET-148 to SET-152) |
| 4.9 Player (sub-pages) | 65 (SET-153 to SET-217) |
| 4.10 Tracking | 15 (SET-218 to SET-232) |
| 4.11 Debug | 11 (SET-233 to SET-243) |
| Total of the 243 item constructions | 243 |

Note: the table IDs above number 243 item rows (SET-001 to SET-243) plus the main-menu rows MAIN-01 to MAIN-12 and MAIN-P1 to MAIN-P9. The 4.12 and 4.13 sections are not counted in the 243 because they are not `PreferenceItem` constructions. Section 4.9 adds nine root rows (MAIN-P1 to MAIN-P9) that are constructed in `PlayerSettingsMainScreen.kt` through a different item class.

## 5. Dispositions by screen

| Section | Screen file (under `app/src/main/java/eu/kanade/presentation/more/settings/screen/` unless noted) | Rows | KEEP | KEEP-RENAME | REPLACE | REDUCE | DROP |
|---|---|---|---|---|---|---|---|
| 4.1 | `SettingsAppearanceScreen.kt` | 8 | 5 | 1 | 1 | 0 | 1 |
| 4.2 | `SettingsAdvancedScreen.kt` | 30 | 10 | 2 | 5 | 8 | 5 |
| 4.3 | `SettingsBrowseScreen.kt` | 6 | 5 | 1 | 0 | 0 | 0 |
| 4.4 | `SettingsDataScreen.kt` | 11 | 9 | 0 | 2 | 0 | 0 |
| 4.5 | `SettingsDownloadScreen.kt` | 25 | 22 | 0 | 1 | 0 | 2 |
| 4.6 | `SettingsLibraryScreen.kt` | 21 | 18 | 2 | 1 | 0 | 0 |
| 4.7 | `SettingsReaderScreen.kt` | 46 | 43 | 0 | 2 | 0 | 1 |
| 4.8 | `SettingsSecurityScreen.kt` | 5 | 3 | 0 | 1 | 0 | 1 |
| 4.9 | `player/*.kt` (the root and seven sub-page files) | 65 | 57 | 0 | 6 | 0 | 2 |
| 4.10 | `SettingsTrackingScreen.kt` | 15 | 15 | 0 | 0 | 0 | 0 |
| 4.11 | `debug/DebugInfoScreen.kt` | 11 | 4 | 3 | 1 | 0 | 3 |
| Total | | 243 | 191 | 9 | 20 | 8 | 15 |

The per-screen counts were read from the tables in section 4 and sum to the totals in 1.3.

## 6. App shell, navigation, onboarding, security and WebView (area 36)

### 6.1 Process bootstrap and single instance

OBSERVED: `app/src/main/AndroidManifest.xml` declares `MainActivity` with `android:launchMode="singleTop"` and `android:windowSoftInputMode="adjustResize"` (lines 53 to 60). `App` is the application class (`:37`).

Proposal: one `gtk::Application` with `application_id` and `ApplicationFlags::HANDLES_COMMAND_LINE` (`T-53a-006`). A second launch forwards its arguments to the running instance and exits (`T-53a-007`, P0). The migration gate runs before the first window is shown (`T-36a-001`, P0). The splash page is dropped and replaced by a loading state while the migrator runs (`T-53a-069`, P2; startup gate `T-36a-020`, P2).

### 6.2 Root navigation and start screen

Proposal (from `T-36a-002`, `T-36a-004`, `T-36a-005`, `T-36a-006`, `T-36a-008`):

* The root navigator is a `gtk::Stack` with the transitions listed in `T-53a-051`.
* The start screen is the value of `start_screen` (SET-005). The default is `StartScreen.ANIME` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:39`).
* Tabs: adaptive chrome. In wide windows the tabs are a sidebar (`T-49b-017`). In narrow windows they are a bottom switcher (`T-36a-005`). The bottom bar is dropped on desktop (`T-49b-018`, P3) and kept only as the narrow fallback.
* Re-selecting the current tab returns to its root (`T-36a-008`).
* Tab badges: updates and extensions (`T-36a-007`, P1). The updates badge follows `library_show_updates_count` (SET-093).

### 6.3 App state banners

OBSERVED: the shell shows state banners for indexing, downloaded-only mode, and incognito mode (`T-36a-018`, P0). The banner widget is `T-36a-069` (P0).

Proposal: banners are GTK `InfoBar`-style widgets in the window header. The incognito banner uses the incognito notice category (`T-35a-083`).

### 6.4 Onboarding (four steps)

OBSERVED tasks: `T-36a-024` (gate and four-step wizard, P0), `T-36a-025` (theme, P0), `T-36a-026` (storage folder, P0), `T-36a-027` (desktop status, replaces permissions, P2), `T-36a-028` (guides and restore entry, P0), `T-36a-062` (restore backup highlight, P1), `T-47ba-060` (back-blocking and finish, P0), `T-47ba-061` (restore path with Data-and-storage highlight, P0).

Proposal: step 1 sets `pref_theme_mode_key`, step 2 sets the storage root through the portal folder chooser (SET-046), step 3 reports the desktop status (notification service present, portal present, secret service present), step 4 links to the guides and to the restore page. The Android permission steps (notifications, storage, battery) are dropped (`T-53a-028`, `T-53a-029`, `T-53a-030`).

### 6.5 App lock

OBSERVED tasks: `T-36a-032` (lock state machine, P1, L), `T-36a-033` (authenticator, Linux replacement for BiometricPrompt, P1, L), `T-36a-034` (unlock window, P1), `T-36a-035` (secure screen flag, dropped, P3), `T-52a-033` (PAM authentication, P2), `T-53a-070` (quit on authentication error, P2).

OBSERVED: `UnlockActivity` is declared in `app/src/main/AndroidManifest.xml:223`.

Proposal: the lock is a passphrase stored as an argon2id hash in the Secret Service (`T-req-platform-003`). The product choice (passphrase versus PAM versus no lock) is open question Q-C in `area:02`. Until it is decided the lock is P2 and the rows SET-148 and SET-149 stay disabled.

### 6.6 Incognito

OBSERVED tasks: `T-36a-017` (incognito reset at launch and source-screen pop when incognito is turned off, P1), `T-36a-036` (incognito state, P1), `T-35a-083` (incognito notice category, P2), `T-53a-022` (incognito notice and disable action, P1).

### 6.7 WebView

OBSERVED: `app/src/main/AndroidManifest.xml:228` declares `WebViewActivity`. Tasks: `T-36a-049` (header, navigation actions, progress, P0), `T-36a-050` (Cloudflare help banner, P1), `T-36a-051` (navigation policy and custom headers, P1), `T-36a-052` (WebKitGTK engine, settings, and cookie bridge, P1, L), `T-36a-053` (activity, screen and model, P1), `T-36a-054` (share and open in browser, P1), `T-53a-071` (page behaviour and back action, P2), `T-req-platform-006` (WebViewHost trait with browser fallback, P1, L).

Proposal: WebKitGTK 6 (`webkit6` crate) is the engine. Cookies are shared with the `reqwest` cookie store through the WebKitGTK cookie manager. The "clear cookies" and "clear WebView data" rows (SET-019, SET-020) act on both stores.

### 6.8 Shell keyboard and accessibility

Tasks: `T-36a-065` (keyboard and mouse map for the shell, P1), `T-36a-066` (accessibility labels and focus order, P1), `T-36a-067` (shell test suite, P1), `T-47ba-077` (keyboard and focus behaviour for settings rows and dialogs, P1), `T-47ba-080` (keyboard shortcuts for More and settings pages, P2).

### 6.9 Shell widget and layout tasks

Tasks: `T-36a-037` (app bar: title, subtitle, count pill, navigation icon, P0), `T-36a-038` (app bar action mode, P0), `T-36a-039` (search toolbar, P0), `T-36a-040` (tabbed screen with per-page search and actions, P0), `T-36a-041` (tabbed dialog, P1), `T-36a-042` (adaptive sheet, phone bottom sheet and tablet dialog, P1), `T-36a-043` (dropdown menus, P1), `T-36a-044` (entry download menu, P1), `T-36a-045` (floating add button with collapse-on-scroll, P1), `T-36a-046` (download indicator, P1), `T-36a-047` (relative date text, P0), `T-36a-048` (empty screen component, P0), `T-36a-029` (app theme application: name map, AMOLED, Linux fallback, P1), `T-36a-030` (theme mode to GTK colour scheme, P0), `T-36a-031` (tablet UI mode and breakpoints, P0), `T-36a-061` (base activity and theming lifecycle, P0).

### 6.10 Navigation and routing

Tasks: `T-36a-003` (navigator disposal and nested sheet navigators, P1), `T-36a-009` (tab routing events, P1), `T-36a-010` (intent: notification dismiss, P1), `T-36a-011` (intent: shortcut actions, P1), `T-36a-012` (intent: search, P1), `T-36a-013` (intent: extension-driven search, P1), `T-36a-014` (intent: file and URI open, P1), `T-36a-015` (onNewIntent equivalent through GApplication activate and open, P1), `T-36a-016` (auto-clear chapter cache at launch, P1), `T-36a-060` (entry open routing from tabs, P0).

### 6.11 Startup update checks and the update prompt

Tasks: `T-36a-021` (changelog dialog after migration, P1), `T-36a-022` (startup update checks for the app release and extensions, P1), `T-36a-023` (new update prompt page, P1). Release URL: the update page must point at the Kurobayomi repository and not at Aniyomi (`RISKS-AND-DECISIONS.md`, distribution note; `T-36a-022`, `T-36a-023`).

### 6.12 Deep links, intents and player routing

Tasks: `T-36a-055` (deep-link kinds and routing, P1), `T-36a-056` (anime deep-link resolve page, P1), `T-36a-057` (manga deep-link resolve page, P1), `T-36a-058` (player router, internal versus external, P2), `T-36a-059` (external player result handling, P2). Section 11 lists the manifest data.

### 6.13 Shell items that are dropped

| Item | Task | What the user loses | Replacement |
|---|---|---|---|
| System bar and edge-to-edge styling | `T-36a-019` (P3) | nothing on desktop | window decorations from the compositor |
| Secure screen flag | `T-36a-035` (P3) | screenshot block in the Android window | none. Documented in section 14. |
| Android widgets | `T-36a-063` (P3) | home-screen widgets | `area:50` Plasma applets |
| Assist content | `T-36a-064` (P3) | Android assistant surfacing | none |
| Android permission and intent rows | `T-36a-068` (P3) | Android permission screens | none |

## 7. Notifications (area 35 and 53)

### 7.1 Notification channels (13 Android channels, OBSERVED)

OBSERVED: `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt`.

| Android channel id | Source line | Notification ids (Android) | Kurobayomi category (proposal) |
|---|---|---|---|
| `common_channel` | `:22` | `ID_DOWNLOAD_IMAGE = 2` (`:23`) | `common` (save-image notice gets its own id, `T-35a-086`) |
| `library_progress_channel` | `:29` | `ID_LIBRARY_PROGRESS = -101`, `ID_LIBRARY_SIZE_WARNING = -103` (`:30`, `:31`) | library (`T-35a-080`) |
| `library_errors_channel` | `:32` | `ID_LIBRARY_ERROR = -102` (`:33`) | library (`T-35a-080`) |
| `downloader_progress_channel` | `:39` | `ID_DOWNLOAD_CHAPTER_PROGRESS = -201`, `ID_DOWNLOAD_EPISODE_PROGRESS = -203` (`:40`, `:41`) | downloads (`T-35a-081`) |
| `downloader_error_channel` | `:42` | `ID_DOWNLOAD_CHAPTER_ERROR = -202`, `ID_DOWNLOAD_EPISODE_ERROR = -204` (`:43`, `:44`) | downloads (`T-35a-081`) |
| `new_chapters_episodes_channel` | `:49` | `ID_NEW_CHAPTERS = -301`, `ID_NEW_EPISODES = -1301` (`:50`, `:51`); groups `eu.kanade.tachiyomi.NEW_CHAPTERS` and `eu.kanade.tachiyomi.NEW_EPISODES` (`:52`, `:53`) | library updates (`T-35a-080`) |
| `backup_restore_progress_channel` | `:59` | `ID_BACKUP_PROGRESS = -501`, `ID_RESTORE_PROGRESS = -503` (`:60`, `:61`) | backup (`T-35a-082`) |
| `backup_restore_complete_channel_v2` | `:62` | `ID_BACKUP_COMPLETE = -502`, `ID_RESTORE_COMPLETE = -504` (`:63`, `:64`) | backup (`T-35a-082`) |
| `incognito_mode_channel` | `:69` | `ID_INCOGNITO_MODE = -701` (`:70`) | incognito (`T-35a-083`) |
| `torrent_server_channel` | `:75` | `ID_TORRENT_SERVER = -801` (`:76`) | server (`T-35a-084`) |
| `http_server_channel` | `:81` | `ID_HTTP_SERVER = -901` (`:82`) | server (`T-35a-084`) |
| `app_apk_update_channel` | `:88` | `ID_APP_UPDATER = 1`, `ID_APP_UPDATE_PROMPT = 2`, `ID_APP_UPDATE_ERROR = 3` (`:89` to `:91`) | update (`T-35a-048`, `T-35a-052`) |
| `ext_apk_update_channel` | `:92` | `ID_UPDATES_TO_EXTS = -401`, `ID_EXTENSION_INSTALLER = -402` (`:93`, `:94`) | extension updates (`T-35a-085`) |

OBSERVED collision: `ID_DOWNLOAD_IMAGE = 2` (`:23`) and `ID_APP_UPDATE_PROMPT = 2` (`:90`) share the integer space. `T-35a-005` fixes it with a stable id registry.

Proposal: no Android channels. The 13 channels become notice categories (`T-53a-021`, `T-35a-004`). Each category has a per-category enable preference.

### 7.2 Notice model and router

Tasks: `T-35a-001` (Notifier trait, Notice, Category enum, P1), `T-35a-002` (GioNotifier: post and withdraw through `gtk::Application`, P1), `T-35a-003` (notice validation: button limit and content rules, P1), `T-35a-004` (category registry and per-category enable preferences, P1), `T-35a-005` (notice id registry with the Android collision fixed, P1), `T-35a-006` (AppAction codec and the `app.notice` GAction router, P1), `T-35a-007` (withdraw-only dismissal and the cancel-all guard, P1), `T-35a-008` (ProgressThrottle with the Android 200 ms and strictly-increasing rule, P1), `T-35a-009` (hide-content gate for private notices, P1), `T-35a-010` (GioOpener: open URL, open file, reveal in file manager, P1), `T-35a-011` (application id, desktop file and notification service check at startup, P1), `T-53a-017` (notice dispatcher replacing NotificationReceiver, P1), `T-53a-018` (AppAction payload codec and validation, P1), `T-53a-019` (dismiss withdraws one id only, P1), `T-53a-020` (stable notice id registry, P1), `T-53a-023` (server availability and in-app toast fallback, P1).

Proposal: the Freedesktop `org.freedesktop.Notifications` interface through `gio::Notification` on the `gtk::Application`. Action buttons map to `app.notice` with a codec payload.

### 7.3 Notification actions (OBSERVED from the Android receiver, mapped)

| Action | Task | Priority |
|---|---|---|
| Open downloads (manga and anime) | `T-35a-013` | P1 |
| Open chapter and episode, with missing-row toast | `T-35a-014` | P1 |
| Open manga and anime entry, dismissing the notice | `T-35a-015` | P1 |
| Mark chapters read | `T-35a-016` | P1 |
| Mark episodes seen | `T-35a-017` | P1 |
| Mark read and seen with optional download removal | `T-53a-043` | P1 |
| Download chapters | `T-35a-018` | P1 |
| Download episodes | `T-35a-019` | P1 |
| Queue control: resume, pause, clear (manga and anime) | `T-35a-020` | P1 |
| Cancel library update (manga and anime) | `T-35a-021` | P1 |
| Cancel restore | `T-35a-022` | P1 |
| Share image becomes reveal in file manager | `T-35a-023` | P2 |
| Backup share becomes reveal in file manager | `T-35a-024` | P2 |
| Keyboard equivalents for the five S Pen actions in reader and player | `T-35a-028` | P2 |

Dropped actions: `T-35a-012` (channel creation and deprecated-channel cleanup, P3), `T-35a-025` (unused `ACTION_OPEN_ENTRY`, P3), `T-35a-026` (group-summary dismissal logic, P3), `T-35a-027` (Samsung S Pen remote actions, P3), `T-35a-063` (runtime notification permission, P3), `T-35a-064` (accent colour tint, P3), `T-35a-065` (Android sound override for backup-complete, P3), `T-53a-028` (WAKE_LOCK and battery prompts, P3), `T-53a-030` (notification permission prompt, P3).

### 7.4 Notification categories and their switches

| Category (proposal) | Task | Priority | Android channel replaced |
|---|---|---|---|
| library | `T-35a-080` | P1 | `library_progress_channel`, `library_errors_channel`, `new_chapters_episodes_channel` |
| downloads | `T-35a-081` | P1 | `downloader_progress_channel`, `downloader_error_channel` |
| backup and restore | `T-35a-082` | P1 | `backup_restore_*` |
| incognito | `T-35a-083` | P2 | `incognito_mode_channel` |
| server status (torrent and HTTP) | `T-35a-084` | P2 | `torrent_server_channel`, `http_server_channel` |
| extension updates | `T-35a-085` | P2 | `ext_apk_update_channel` |
| update (app) | `T-35a-048`, `T-35a-052`, `T-35a-053` | P1 | `app_apk_update_channel` |
| common | `T-35a-086`, `T-35a-087` | P2 | `common_channel` |

Settings that expose these categories are under `Notification` in the Advanced page: the Kotlin row is SET-013 (`Manage notifications`, `SettingsAdvancedScreen.kt:128`), which becomes a desktop-settings link.

### 7.5 Notification and progress rules

Tasks: `T-35a-008` (ProgressThrottle, 200 ms, strictly increasing), `T-35a-051` (download progress notice, P1), `T-35a-052` (error notice with ordering: withdraw progress first, P1), `T-35a-053` (completed download notice with reveal action, P1), `T-35a-057` (download waits for network with NetworkStatus, P2), `T-53a-024` (download and update gates read one NetworkStatus, P1), `T-53a-025` (metered-connection policy, P2), `T-53a-044` (progress throttle and app-update strings, P1).

### 7.6 Tests for notifications

Tasks: `T-35a-090` (version and regression unit tests), `T-35a-091` (asset selection unit tests), `T-35a-092` (action codec and validation tests), `T-35a-093` (ProgressThrottle unit tests), `T-35a-098` (router tests with a fake notifier), `T-53a-073` (notice dispatch and payload tests), `T-53a-075` (desktop file, MIME and shortcut golden tests), `T-53a-077` (dead-code guard test for dropped actions).

## 8. Home-screen widgets (area 50)

Per `T-50a-026` to `T-50a-028`, the Android side has two Glance widgets for manga and anime, home and lock-screen receivers, and Samsung cover-screen metadata. The manifest receivers and keyguard providers are dropped.

Proposal: two Plasma 6 applets, one for manga and one for anime, that read the local database through a read-only CLI mode.

| Item | Task | Priority |
|---|---|---|
| Shared constants: cover, cell pitch, radii, caps, window, row limit | `T-50a-001` | P2 |
| Grid geometry: rows and columns from a size | `T-50a-002` | P2 |
| Per-request grid size instead of the largest-area instance | `T-50a-003` | P2 |
| Recent window: now minus three calendar months, strict bound | `T-50a-004` | P2 |
| Manga unread-updates query | `T-50a-005` | P2 |
| Anime unread-updates query | `T-50a-006` | P2 |
| Dedupe by entry and cap (first occurrence wins) | `T-50a-007` | P2 |
| Read the lock flag as Android does (fail closed) | `T-50a-008` | P1 |
| Locked payload: no query and no titles or covers leave the process | `T-50a-009` | P1 |
| Locked text uses the same message key on every surface | `T-50a-010` | P1 |
| Empty state text for both kinds | `T-50a-011` | P2 |
| Loading and error states with a timeout | `T-50a-012` | P2 |
| Thumbnail renderer and cache-only cover lookup | `T-50a-013` | P2 |
| Placeholder tile matching the Android error cover | `T-50a-014` | P2 |
| `kurobayomi --updates-json` argument parsing and schema-1 JSON envelope | `T-50a-015` | P2 |
| Read-only database access from the CLI (WAL, busy timeout, never create) | `T-50a-016` | P2 |
| Error envelope, exit codes, and stderr diagnostics | `T-50a-017` | P2 |
| Two Plasma 6 applets (manga and anime) with the grid and placeholders | `T-50a-018` | P2 |
| Applet refresh: 60 s poll, resize debounce, header refresh in error or empty states | `T-50a-019` | P2 |
| Cell activation opens the entry (`--open manga:<id>` / `anime:<id>`) | `T-50a-020` | P2 |
| Colours from the Plasma theme | `T-50a-021` | P2 |
| Applet size rules: 1x1 minimum, 4x2 default, 10x10 maximum | `T-50a-022` | P2 |
| Accessibility and pointer affordances for covers | `T-50a-023` | P2 |
| `.desktop` action `Updates` and `--open updates` | `T-50a-024` | P2 |
| Applet packaging (kpackagetool6, optional package, Flatpak note) | `T-50a-025` | P2 |
| Replace logcat error logging with the app logger and CLI stderr | `T-50a-034` | P2 |
| Test suite for the feed, CLI, and thumbnails | `T-50a-035` | P2 |

Dropped widget items: `T-50a-026` (four home and lock receivers, P3), `T-50a-027` (keyguard providers, P3), `T-50a-028` (Samsung cover-screen classes, P3), `T-50a-029` (push refresh in favour of polling, P3), `T-50a-030` (Glance modifier helpers, P3), `T-50a-031` (unused colour tokens, P3), `T-50a-032` (widget-picker preview images, P3), `T-50a-033` (Gradle module wiring and empty ProGuard files, P3). The user loses the home-screen widgets on Android. On Linux the replacement is the Plasma applet.

The widget's lock behaviour is the most important rule in this area: when the app lock is on, the applet payload carries no titles and no covers (`T-50a-009`). Proposal: this is a hard requirement, and `T-50a-008` reads the lock flag from `state.toml` and fails closed.

## 9. Internationalisation (area 51)

OBSERVED facts: the Kotlin strings use Moko resources (`MR.strings.*` and `AYMR.strings.*` in the rows above). The translated locale count is 66 (`T-51a-008`).

| Area | Tasks |
|---|---|
| Fluent catalog generated from the base XML | `T-51a-001` (P0), `T-51a-002` (P0), `T-51a-003` (P0), `T-51a-004` (P0), `T-51a-005` (P0) |
| Runtime catalog: bundle chain, lookup, fallback | `T-51a-006` (P0) |
| Locale resolution and tag normalisation | `T-51a-007` (P0) |
| Import the 66 translated locales | `T-51a-008` (P1) |
| Translation validation rules | `T-51a-009` (P1) |
| Generated message-key enum and build script | `T-51a-010` (P0) |
| Language picker data and display names | `T-51a-011` (P1), `T-51a-012` (P1) |
| Language picker screen | `T-51a-013` (P1) |
| RTL semantics for chapter swipe and reader labels | `T-51a-014` (P1) |
| Startup language, RTL direction, restart semantics | `T-51a-015` (P1) |
| Relative date and datetime functions | `T-51a-016` (P1) |
| Short date and fixed date formats | `T-51a-017` (P1) |
| Duration short formatter | `T-51a-025` (P1) |
| Missing-message policy and logging | `T-51a-026` (P2) |
| Font coverage check for non-Latin scripts | `T-51a-027` (P2) |
| Source-list display names | `T-51a-034` (P1) |
| Overall i18n acceptance | `T-51a-043` (P0) |

OBSERVED: the language picker is `app/src/main/java/eu/kanade/presentation/more/settings/screen/appearance/AppLanguageScreen.kt`, opened by SET-003 (`SettingsAppearanceScreen.kt:118`).

Dropped: `T-51a-022` (Gradle, moko and manifest plumbing, P3), `T-51a-023` (Android localisation API surface, P3), `T-51a-036` (notification channel concept, P3), `T-51a-037` (S Pen strings, P3). The user loses nothing visible; the text stays.

## 10. Theming (areas 49, 49b, 49ba and the settings rows)

### 10.1 Mode and palette rows

| Row | Task | Priority |
|---|---|---|
| Mode selector: System, Light, Dark | `T-47ba-040`, `T-49b-007`, `T-47a-011` | P0 |
| Theme preview cards | `T-47ba-041`, `T-49a-062` | P0, P1 |
| Live apply, AMOLED and deprecated-name mapping | `T-47ba-042` | P0 |
| AMOLED switch with the Light gate | `T-49b-015`, `T-49ba-003` | P1 |
| System dark detection and live updates | `T-49a-011` | P0 |
| Theme mode to GTK colour scheme | `T-36a-030` | P0 |
| Linux "System" scheme (no colour override) | `T-49a-060` | P2 |
| Optional bundled Tokyo Night scheme (addition) | `T-49a-063` | P2 |

### 10.2 Palette count conflict

Five tasks disagree on how many static palettes there are and whether they are kept.

| Task | Text | Priority |
|---|---|---|
| `T-47a-012` | "Drop the 17 named app palettes" | P3 |
| `T-48a-004` | "App colour themes (19 live palettes, dynamic replaced)" | P2 |
| `T-49a-004` | "Port the 18 static color schemes as full token tables" | P1 |
| `T-49b-002` | "Port the 18 static scheme tables from the XML (generated constants)" | P0 |
| `T-49ba-004` | "CottonCandy and Mocha palettes (light and dark)" | P2 |

Conflict C2 (section 14). Proposal: the palette list is decided by area 49 (`T-49b-002` is P0 and the owner). `T-47a-012` is rewritten to "Drop the palette names that have no Kurobayomi scheme" and its priority is changed to P2 only after the list is fixed. Per `T-49b-002`, the XML holds 18 static schemes. The GTK-following default is added to them. Open question O-1 asks the user which schemes to keep.

### 10.3 Monet and dynamic colour

OBSERVED: `UiPreferences.kt:22` to `:29` chooses `AppTheme.MONET` when dynamic colour is available. `DeviceUtil.isDynamicColorAvailable` is an Android API.

Proposal: the Linux "Dynamic" option takes the accent colour from the XDG desktop portal (`org.freedesktop.portal.Settings`, key `org.freedesktop.appearance` `accent-color`). Tasks: `T-49b-005` (P1), `T-49a-005` (P1, portal accent and KDE fallback), `T-49a-006` (P1, HCT palette generation), `T-49a-007` (P1, availability and fallback), `T-47a-013` (P2, dynamic accent replaces "Dynamic"), `T-48a-058` (P3, replacement of Monet). The default on Linux is decided in `T-49b-006` (P0), and the proposal is `DEFAULT`, which follows the GTK theme (open question Q-A in `area:02`).

### 10.4 Token, widget and layout tasks

Tasks are listed in `area:49` and `area:49b`. The ones that a settings row depends on:

| Row | Task |
|---|---|
| Settings row base and paddings | `T-49a-045` (P0) |
| Settings rows: switch, select, slider, text, tri-state, chip row | `T-49a-037` (P0), `T-49a-039` (P0), `T-49a-038` (P0), `T-49a-043` (P1), `T-49a-040` (P1), `T-49a-044` (P1) |
| Section card and list group header | `T-49a-035` (P0), `T-49a-033` (P0) |
| Dialog content layout | `T-49a-049` (P0) |
| Responsive settings widths | `T-49a-066` (P2) |
| Keyboard and focus for all widgets | `T-49a-055` (P1) |
| Window size classes and breakpoints (medium 600, expanded 840) | `T-49a-021` (P1), `T-49b-042` (P2) |

Dropped theme items: `T-49b-018` (NavigationBar, P3), `T-49b-032` (clear focus on IME hide, P3), `T-49b-036` (scrollbar, P3), `T-49b-037` (ThemePreviews annotation, P3), `T-49a-056` (haptics, P3), `T-49a-030` (paging helper, P3), `T-49ba-007` (Android dynamic colour, P3), `T-49ba-008` (window bools and API-27 overrides, P3), `T-49ba-047` (window attributes, P3). The user loses haptic feedback on the Android device only.

## 11. Intents, deep links and desktop integration (area 53, manifest)

### 11.1 Manifest intent filters (OBSERVED)

From `app/src/main/AndroidManifest.xml`:

| Component | Lines | Filter | Kurobayomi route (proposal) |
|---|---|---|---|
| `.ui.main.MainActivity` | 53 to 70 | `MAIN` and `LAUNCHER` | desktop entry launch (`T-53a-002`, P1) |
| `.ui.main.MainActivity` | 71 to 72 | scheme `tachiyomi`, host `add-repo` | `tachiyomi://add-repo` route (`T-53a-004`, P1) |
| `.ui.main.MainActivity` | 82 to 83 | scheme `aniyomi`, host `add-repo` | `aniyomi://add-repo` route (`T-53a-004`, P1) |
| `.ui.main.MainActivity` | 93 to 94 | scheme `aniyomi`, host `extension-store` | `aniyomi://extension-store` route (`T-53a-004`, P1) |
| `.ui.main.MainActivity` | 104 to 107 | schemes `file` and `content`, host `*`, MIME `*/*` | file open (`T-53a-010`, P1) |
| `.ui.main.MainActivity` | 115 to 121 | `pathPattern` `.*\\.tachibk` with 1 to 7 dot-separated segments | `.tachibk` open (`T-53a-003`, P1) |
| `.ui.deeplink.anime.DeepLinkAnimeActivity` | 137 to 160 | action `eu.kanade.tachiyomi.ANIMESEARCH`, MIME `text/plain` | anime search route (`T-53a-005`, P1) |
| `.ui.deeplink.manga.DeepLinkMangaActivity` | 164 to 190 | actions `eu.kanade.tachiyomi.SEARCH` and `eu.kanade.tachiyomi.ANIMESEARCH`, MIME `text/plain` | search route (`T-53a-005`, P1) |
| `.ui.setting.track.TrackLoginActivity` | 243 to 260 | scheme `aniyomi`, hosts `myanimelist-auth`, `anilist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth` | tracker OAuth redirect routing (`T-53a-009`, P1) |

Other OBSERVED components: `.crash.CrashActivity` (`:131`), `.ui.reader.ReaderActivity` (`:195`), `.ui.player.PlayerActivity` (`:207`), `.ui.security.UnlockActivity` (`:223`), `.ui.webview.WebViewActivity` (`:228`), `.extension.manga.util.MangaExtensionInstallActivity` (`:233`), `.extension.anime.util.AnimeExtensionInstallActivity` (`:238`), `.data.notification.NotificationReceiver` (`:263`), `.extension.manga.util.MangaExtensionInstallService` (`:267`), `.extension.anime.util.AnimeExtensionInstallService` (`:272`), `.data.torrent.service.TorrentServerService` (`:290`), `.data.player.service.HttpServerService` (`:294`).

Proposal for those components: the crash activity becomes the crash dialog (section 13). Reader and player are windows (`T-53a-055`, `T-53a-057`, `T-53a-015`). The extension install activities and services are dropped (`T-53a-036`, P3). The unlock activity becomes the unlock window (`T-36a-034`). The torrent and HTTP services become background services with notices (`T-53a-035`, P1).

### 11.2 Tasks for intents and launch

| Task | Text | Priority |
|---|---|---|
| `T-53a-001` | GTK application entry point replaces MainActivity (single window) | P0 |
| `T-53a-006` | Command-line and open handling in GApplication | P0 |
| `T-53a-007` | Single instance: a second launch is forwarded to the running window | P0 |
| `T-53a-008` | Keep the process alive while background jobs run with no window | P1 |
| `T-53a-011` | Desktop actions for the five shortcuts (replaces shortcuts.xml) | P1 |
| `T-53a-012` | Localised desktop action labels | P2 |
| `T-53a-013` | Packaging permission audit for the sandbox | P1 |
| `T-53a-015` | One reader and one player window at a time (singleTask) | P1 |
| `T-53a-037` | BackgroundHold registry | P1 |
| `T-53a-039` | Opener trait: open URL, open file, reveal | P1 |
| `T-53a-040` | Share replacement: Show in folder and Copy link | P2 |
| `T-53a-041` | Desktop packaging: MIME registration, icon names and desktop database | P1 |
| `T-53a-066` | Desktop search provider (GNOME SearchProvider2 and KRunner) | P2 |
| `T-53a-031` | Language selection replaces per-app locales | P2 |

### 11.3 Intent-specific tasks

Anime and manga deep-link loading pages: `T-53a-047` (anime loading page, P1), `T-53a-048` (anime resolver in kuro-core, P1), `T-53a-049` (manga loading page, P1), `T-53a-050` (manga resolver, P1). Route errors and kind parsing without crashes: `T-53a-045` (P1). Notification dispatch: section 7. Desktop search: `T-53a-066`.

### 11.4 Desktop entry, MIME and shortcuts

Per `T-49ba-037` and `T-49ba-038`, Android defines five launcher shortcuts. Proposal: five desktop actions in `dev.kurobayomi.Kurobayomi.desktop` (identifier is a proposal) that open the updates page, the library, the manga browse page, the anime browse page and the downloads queue.

MIME type for backups is `application/x-tachibk` (`T-53a-003`, P1). The desktop file declares it with `MimeType=application/x-tachibk;`.

### 11.5 Permissions and sandbox

| Android permission | Disposition | Replacement | Task |
|---|---|---|---|
| `ACCESS_WIFI_STATE` | REPLACE | NetworkManager state (`T-53a-026`, P1) | `T-53a-026` |
| `MANAGE_EXTERNAL_STORAGE` | REPLACE | folder chooser through the portal (`T-53a-027`, P1) | `T-53a-027` |
| `WAKE_LOCK` | DROP | `IdleInhibitor` only while playing or reading (`T-52a-038`, P2) | `T-53a-028` |
| Foreground service permissions | DROP | holds and notices (`T-53a-029`, P3; `T-53a-037`, P1) | `T-53a-029` |
| Notification permission prompt | DROP | none (`T-53a-030`, P3) | `T-53a-030` |
| Extension permissions and Shizuku provider | DROP | none (`T-53a-038`, P3) | `T-53a-038` |
| Application attributes (backup, heap, legacy storage, icons) | DROP | none (`T-53a-032`, P3) | `T-53a-032` |

Packaging permission audit: `T-53a-013` (P1).

### 11.6 Cross-platform rule

`T-req-platform-002` (P0) forbids OS-specific code outside `kuro-platform`. Every row in sections 6 to 11 that names a Linux mechanism is implemented behind a trait in `kuro-platform` (`Notifier`, `Opener`, `SecretStore`, `IdleInhibitor`, `SingleInstance`, `ExternalPlayer`, `WebViewHost`, `MediaControls`), as listed in `addenda/requirements-cross-platform.md` section "Platform traits to define".

## 12. Packaging, updates, build and release (area 54 and 35)

### 12.1 Build and workspace

Tasks: `T-54a-001` (Cargo workspace root and toolchain pin, P0), `T-54a-002` (build metadata in `build.rs`, replaces `Commands.kt`, P0), `T-54a-003` (release profile and Cargo features, replaces build types and flags, P0), `T-54a-004` (workspace dependencies replace version catalogs, P1), `T-54a-005` (rustfmt, clippy and xtask lint, replaces spotless and ktlint, P1), `T-54a-006` (`.editorconfig` and `.gitattributes`, P1), `T-54a-007` (`.gitignore` for Rust, Flatpak and dist output, P1), `T-54a-034` (xtask crate, P1), `T-54a-037` (module graph to crate graph, P1), `T-54a-038` to `T-54a-045` (crate mapping: `kb-db`, `kb-domain`, `kb-source-api`, `kb-local`, `kb-metadata`, `kb-ui`, root crate, P1).

Proposal: `T-54a-061` (architecture targets replace the ABI splits, P2) and `T-54a-029` (Devel application id, beta channel and benchmark profile, P1) cover the two Android build variants.

### 12.2 CI and release

Tasks: `T-54a-008` (CI pull request pipeline `ci.yml`, P1), `T-54a-009` (release workflow on version tags `release.yml`, P1), `T-54a-010` (release check in xtask: tag, changelog, metainfo, LINGUAS, lockfile, Flatpak source, P2), `T-54a-035` (test configuration and coverage job, P2), `T-54a-054` (cargo-deny policy, P1), `T-54a-055` (embedded dependency list, P2), `T-54a-056` (release signing and checksum signature, P1), `T-54a-057` (third-party licence list, P2).

### 12.3 Packaging, metadata and distribution

| Item | Task | Priority |
|---|---|---|
| AppStream metainfo and desktop entry (replaces fastlane metadata) | `T-54a-011` | P1 |
| Real icon files (replaces the symlinked icon.png) | `T-54a-012` | P2 |
| Flatpak manifest with pinned mpv module and cargo sources | `T-54a-013` | P1 |
| LINGUAS and AppStream languages (replaces LocalesConfigTask) | `T-54a-032` | P1 |
| gettext catalogues replace the moko resource modules | `T-54a-031` | P1 |
| Architecture targets (replaces ABI splits) | `T-54a-061` | P2 |
| Runtime requirements in README (replaces "Requires Android 8.0") | `T-54a-062` | P1 |
| User-Agent for outbound HTTP (replaces Aniyomi user-agent strings) | `T-54a-068` | P1 |
| Dev-build icon overlay (replaces debug resources) | `T-54a-028` | P2 |
| Startup timing script and in-app trace (replaces StartupBenchmark) | `T-54a-023` | P2 |

Distribution decision (OBSERVED in `RISKS-AND-DECISIONS.md`): distro packages are the update path. The in-app APK download is dropped. Packaging uses the system libarchive dependency and an optional mpv build (`RISKS-AND-DECISIONS.md`, distribution note).

### 12.4 Updater (area 35)

Per `T-35a-042` and `T-35a-043`, the release check uses the GitHub `releases/latest` endpoint, and asset selection is by architecture.

| Item | Task | Priority |
|---|---|---|
| Release and GitHub DTO models | `T-35a-040` | P1 |
| Version parsing and `is_newer` (strict semver) | `T-35a-041` | P1 |
| `fetch_latest`: GitHub `releases/latest` with status mapping | `T-35a-042` | P1 |
| `pick_asset`: architecture tiers and extension preference | `T-35a-043` | P1 |
| Release notes: mention rewrite, checksum strip, sha256 extraction | `T-35a-044` | P2 |
| Update throttle state (3 days, app state key) | `T-35a-045` | P1 |
| Updater cargo feature and `update.repo` setting | `T-35a-046` | P1 |
| `UpdateService::check` orchestration | `T-35a-047` | P1 |
| Update page and update-available notice | `T-35a-048`, `T-35a-049` | P1 |
| Downloader: single-slot task with stale part cleanup | `T-35a-050` | P1 |
| SHA-256 verification before reveal | `T-35a-054` | P2 |
| Install kind detection (package-managed, AppImage, other) | `T-35a-055` | P2 |
| Release notes renderer (Markdown subset) | `T-35a-062` | P2 |
| About "Check for updates" row | `T-35a-058` | P1 |

Dropped updater items: `T-35a-056` (APK install and in-place AppImage self-replacement, P3), `T-35a-059` (foreground service and DATA_SYNC type, P3), `T-35a-060` (OsTooOld result, P3), `T-35a-061` (preview build channel with commit-count tags, P3). The user loses in-app updates. The update path is the package manager.

### 12.5 Documentation and repository

Tasks: `T-54a-014` (drop upstream funding links, P3), `T-54a-015` (issue templates for Linux, P2), `T-54a-016` (pull request template for Linux, P2), `T-54a-017` (Code of Conduct, Contributor Covenant 2.1, P1), `T-54a-018` (CONTRIBUTING with Rust prerequisites and release runbook, P1), `T-54a-019` (README rewrite, P1), `T-54a-020` (LICENSE unchanged and NOTICE with attribution, P1), `T-54a-021` (CHANGELOG in Keep-a-Changelog form, P1), `T-54a-022` (drop Android integer versionCode, P3), `T-54a-024` (drop baseline profile, P3), `T-54a-025` (drop macrobenchmark module, P3), `T-54a-026` (drop ProGuard and consumer rules, P3), `T-54a-030` (drop dead variant filter, P3), `T-54a-036` (drop buildSrc precompiled plugins, P3), `T-54a-043` (drop the Glance home-screen widget module, P3), `T-54a-060` (record drop decisions for Android-only CI steps, P3).

### 12.6 Standing cross-platform requirement

`addenda/requirements-cross-platform.md` is a user requirement recorded 2026-10-08: keep Kurobayomi portable to Windows and macOS later. Linux is the only shipping target now.

| Task | Text | Priority |
|---|---|---|
| `T-req-platform-001` | Create the kuro-platform crate and the Paths trait | P0 |
| `T-req-platform-002` | Forbid OS-specific code outside kuro-platform | P0 |
| `T-req-platform-003` | SecretStore trait with Linux implementation | P0 |
| `T-req-platform-004` | Notifier trait with Linux implementation | P1 |
| `T-req-platform-005` | MediaControls trait for the player | P1 |
| `T-req-platform-006` | WebViewHost trait with browser-fallback path | P1 |
| `T-req-platform-007` | Opener, IdleInhibitor, SingleInstance and ExternalPlayer traits | P1 |
| `T-req-platform-008` | Portable file-name sanitiser | P0 |
| `T-req-platform-009` | Bundled icons and CSS so the look does not depend on the host theme | P1 |
| `T-req-platform-010` | CI cross-compile check for Windows | P1 |
| `T-req-platform-011` | Windows port spike (later milestone) | P3 |

## 13. Crash, debug and diagnostics

### 13.1 Crash handling

OBSERVED: `CrashActivity` is declared at `app/src/main/AndroidManifest.xml:131`. The crash log dump is `CrashLogUtil.dumpLogs` (`T-52a-017`, P1) and the debug info block is `CrashLogUtil.getDebugInfo` (`T-52a-018`, P1). Problematic extensions report: `T-52a-019` (P1).

Proposal: a panic hook writes `crashes/crash-<unix_ms>.txt` under `$XDG_STATE_HOME/kurobayomi/` (`ARCHITECTURE.md` section 11). The next launch shows a crash dialog that offers "Share crash logs" (SET-009) and "Open crash folder" (`Opener`, `T-35a-010`).

### 13.2 Logging

Tasks: `T-47a-070` (verbose logging, P1), `T-53a-014` (launch and notice logging targets and the verbose switch, P2), `T-49a-064` (logging and error policy, P0), `T-50a-034` (replace logcat with app logger and CLI stderr, P2).

Proposal: `tracing` with a rolling file `logs/kurobayomi.log` (7 files kept, `ARCHITECTURE.md` section 11). Verbose logging (SET-010) changes the filter to DEBUG.

### 13.3 Debug and worker pages

Tasks: `T-47b-040` (app info rows, P2), `T-47b-041` (device rows for Linux, P2), `T-47b-042` (drop ART profile compilation row, P3), `T-47b-043` (worker info page, P2), `T-47b-044` (job registry, P1), `T-47b-045` (backup file schema page, P2), `T-47b-046` (clipboard copy with confirmation toast, P1), `T-47b-047` (monospace text page layout, P2), `T-47b-048` (WebKitGTK version row, P2), `T-47b-049` (build metadata via `build.rs`, P2), `T-47b-050` (date format preference for info rows and job times, P2), `T-47b-051` (drop OneUI and MIUI rows, P3), `T-52a-056` (background job registry, P0).

### 13.4 Storage and database maintenance

Tasks: `T-47a-074` (reindex downloads, P1), `T-47a-075` (clear manga and anime database screens, P1), `T-47a-076` (clear cookies and WebView data, P1), `T-47a-079` (refresh library covers, P1), `T-47a-080` (reset per-series reader settings, P1).

## 14. Dropped items, conflicts and open questions

### 14.1 Register of dropped items (user loss stated)

| Setting or item | Row or task | What the user loses |
|---|---|---|
| Tablet UI mode | SET-004, `T-47a-017` | a fixed tablet layout switch; the responsive layout replaces it |
| Battery optimisation and "Don't kill my app" rows | SET-014, SET-015, `T-47a-073` | links to vendor battery pages; none is needed on Linux |
| Display profile (ICC) | SET-028, `T-47a-083` | colour-managed page rendering from an ICC profile |
| Extension installer choice | SET-029, `T-47a-085` | the choice between package installer and legacy installer; Kurobayomi has no APK extensions |
| Secure screen (screenshot block) | SET-151, `T-47a-066` | the Android screenshot block; no Linux equivalent |
| Show content in cutout area | SET-110, `T-47a-043` | cutout padding, which does not exist on desktop |
| External downloader selector (IDM, ADM) | SET-079, SET-080, `T-47a-051` | external download managers on Android |
| Player orientation | SET-155, `T-48a-019` | orientation lock on the device |
| System status bar in player | SET-166, `T-48a-020` | status bar visibility; not applicable |
| External player package allowlist | `T-48a-053` | the Android package list; MIME lookup replaces it |
| Android device gates for PiP and fullscreen SDK | `T-48a-054` | Android-only gating |
| Profile compilation status | SET-239, `T-47b-042` | ART profile information |
| OneUI and MIUI rows | SET-241, SET-242, `T-47b-051` | vendor build strings |
| Data Saver server, bandwidth fields | SET-032, `T-47a-087` | the third-party image service settings (section 14, C3) |
| Notification permission prompt, WAKE_LOCK, foreground service | `T-53a-028`, `T-53a-029`, `T-53a-030` | Android prompts |
| APK install and in-place AppImage replacement | `T-35a-056` | in-app updates; the package manager updates instead |
| Preview build channel | `T-35a-061` | the commit-count preview channel |
| Home and lock-screen widget receivers, keyguard providers, Samsung cover screen | `T-50a-026`, `T-50a-027`, `T-50a-028` | Android home and lock widgets |
| Haptics | `T-49a-056` | vibration feedback on Android |
| S Pen remote actions | `T-35a-027`, `T-53a-065` | Samsung stylus buttons; keyboard equivalents remain (`T-35a-028`) |
| Dynamic (Monet) theme | `T-48a-058`, `T-49ba-007` | Material You colours; replaced by the portal accent (section 10.3) |
| Picture-in-picture | `T-53a-033` | the Android PiP window; a compact player mode replaces it (`T-48a-017`) |
| Upstream funding links | `T-54a-014` | links in the upstream README |
| Baseline profile and macrobenchmark | `T-54a-024`, `T-54a-025` | Android startup profiling; replaced by `T-54a-023` |

### 14.2 Conflicts found while writing this document

| ID | Conflict | Sources | Proposal |
|---|---|---|---|
| C1 | Preference store: SQLite key/value (`T-47a-001`, `T-48a-001`) versus TOML (`T-02a-002`, `ARCHITECTURE.md` A8). | `47-settings-screens-general/p1-tasks.md`, `48-settings-player-and-scripts/p1-tasks.md`, `02-preferences-system/00-overview.md` | Use TOML for preferences. Amend `T-47a-001` and `T-48a-001` to say "typed `Pref<T>` over the TOML backend" and keep SQLite only for custom buttons (`T-48a-041`) and extension repos (`RISKS-AND-DECISIONS.md`, repo rows). |
| C2 | Palette count and keep-or-drop: 17 named palettes dropped (`T-47a-012`, P3) against 18 static schemes ported (`T-49b-002`, P0; `T-49a-004`, P1) against 19 live palettes (`T-48a-004`, P2). | the three task files named in 10.2 | Area 49 decides (`T-49b-002`). Rewrite `T-47a-012` and set `T-48a-004` to 18 or 19 after the decision in O-1. |
| C3 | Data Saver: `RISKS-AND-DECISIONS.md` drops the three third-party image services and keeps `NONE` only. `T-47a-086` (P2) still plans "Data Saver (image compression before load)" and `T-47a-087` (P2) plans its flags. | `RISKS-AND-DECISIONS.md` (data saver row); `47-settings-screens-general/p2-tasks.md` | Keep `T-47a-086` and `T-47a-087` only as stored-value parsing (REDUCE rows SET-031, SET-033 to SET-038). Remove the compression behaviour from the plan. |
| C4 | Notification id collision: `ID_DOWNLOAD_IMAGE = 2` and `ID_APP_UPDATE_PROMPT = 2`. | `app/src/main/java/eu/kanade/tachiyomi/data/notification/Notifications.kt:23` and `:90` | Fixed by `T-35a-005`. Already in the plan. |
| C5 | Swipe keys are crossed in Aniyomi: the start row reads `pref_chapter_swipe_end_action`, and the end row reads `pref_chapter_swipe_start_action`. | `domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:369` to `:374` | Keep the crossed keys for backup compatibility. Document it in `T-47a-028` and in the registry (`T-02a-029`). |
| C6 | Retry schedule for failed preference writes is stated differently in two documents. | `02-preferences-system/00-overview.md` (Q-H and section 8 item 5) | Resolve as Q-H in `area:02`: 1, 2, 4, 8, 16 seconds, then every 30 seconds. |
| C7 | Priority inversion for `T-02a-036` and `T-02a-038` (both S, both needed by P0 rows). | `02-preferences-system/00-overview.md` Q-I | Promote both to P0, as recommended in Q-I. |
| C8 | Hard-coded English label `"Navigation Style"` (SET-006) and the device-info literals (SET-235 to SET-243) are not string-table keys. | `SettingsAppearanceScreen.kt:144`, `DebugInfoScreen.kt:53` to `:146` | Route these through the string catalogue (`T-51a-031`, `T-47ba-073`). |
| C9 | The Kotlin swipe rows at `SettingsLibraryScreen.kt:377` and `:393` have no title key. | `SettingsLibraryScreen.kt:377`, `:393` | Use the section header `AYMR.strings.pref_behavior_episode`. |

### 14.3 Open items (unresolved in the source reading, must be filled before implementation)

| ID | Item | Owner task |
|---|---|---|
| O-1 | Which of the 18 static palettes Kurobayomi ships, and whether the GTK-following default is the only default. | `T-49b-002`, `T-49b-006` |
| O-2 | Whether the bundled Tokyo Night scheme (`T-49a-063`) is added to the palette list. | `T-49a-063` |
| O-3 | Store keys for the rows whose Kotlin binding is a local variable (SET-022, SET-029, SET-070 to SET-075, SET-080, SET-109, SET-114 to SET-116, SET-122, SET-124, SET-129, SET-131, SET-138, SET-140, SET-144, SET-157, SET-158, SET-161, SET-162, SET-163, SET-165, SET-166, SET-168, SET-169, SET-171 to SET-176, SET-183, SET-184, SET-190 to SET-197, SET-202 to SET-204, SET-205, SET-206, SET-208, SET-209, SET-212, SET-213, SET-215 to SET-217). | `T-02a-030` (binds the mockup pages) |
| O-4 | Mapping of the title key `MR.strings.library_list` (SET-055) to the export page. | `T-47a-091` |
| O-5 | Per-service keys for the nine tracker rows (SET-222 to SET-231). | `T-47a-055`, `T-47a-057` |
| O-6 | Binding of the download speed limit row (SET-057). The extractor matched the next item's argument. | `T-47a-045` |
| O-7 | Lock choice: passphrase, PAM, or no lock. | `area:02` Q-C, `T-36a-033` |
| O-8 | Whether the Kurobayomi desktop identifier is `dev.kurobayomi.Kurobayomi` (used in section 11.4 as a proposal). | `T-53a-002`, `T-54a-011` |

## 15. Coverage check

* Every settings row in section 4 has an ID, a source line, a disposition, and at least one task. The 243 rows match the 243 item constructions in the screen files.
* Rows whose binding could not be resolved are listed in O-3 and O-6 (section 14.3). Nothing is guessed in them.
* Every task ID cited in this document was checked against `TASKS.csv`: 534 distinct IDs, all present.
* Every area in section 2 is referenced.
