# Area 49b: Android resources (values, drawables, colour selector): overview

Area id: `49b-android-res-values-drawables`. Upstream root for all repo-relative paths: `/home/blackbox/aniyomi-src`. Target: `/home/blackbox/kurobayomi` (gtk4-rs 0.11.5 with feature `v4_16`, no libadwaita, no tokio in this area).

Scope counts (from `p1-inventory.md`): 92 files, 2470 lines = 1 colour selector (`color/`), 73 drawables (`drawable/`), 18 values files (`values/` 9, `values-night/` 4, `values-night-v31/` 1, `values-v31/` 1, `values-v27/` 2, `values-sw720dp/` 1).

Status: plan only. Nothing in this area is implemented in the Kurobayomi source yet. `grep -rn CssProvider src` is empty and the mockup has no `theme.rs` or `icons.rs`.

---

## 1. Purpose and scope

### 1.1 What Aniyomi does here (OBSERVED)

Android resources are the static look of the app: colours, theme styles, dimensions, integers, booleans, vector icons, animated vectors, the splash theme, adaptive launcher icons, and the launcher shortcut list. Resources have no behaviour of their own. The behaviour belongs to the code that resolves them, so each part of this area describes both the resource and the Kotlin that uses it. Examples:

- `app/src/main/res/values/themes.xml` (678 lines) defines 23 styles, including `Base.Theme.Tachiyomi`, `Theme.Tachiyomi` and one style per palette. The style chosen is decided by the `AppTheme` preference (`app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt`) through `app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt`.
- The splash screen is `Theme.Tachiyomi.SplashScreen` (`values/themes.xml:660-668`). The keep-on-screen timing is in `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:316-322`, with constants at `MainActivity.kt:665-667` (`SPLASH_MIN_DURATION = 500`, `SPLASH_MAX_DURATION = 5000`, `SPLASH_EXIT_ANIM_DURATION = 400L`).
- Reader and player icons (`ic_reader_*`, `ic_crop_*`, `ic_play_seek_triangle`) are used by `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt`, `BottomReaderBar.kt` and the player controls.
- Nine animated vector drawables (`anim_*.xml`) drive the tab-enter morphs and the caret rotation.
- Notification small icons (`ic_book_24dp`, `ic_done_24dp`, `ic_warning_white_24dp`, and others) are used by the download, library-update, backup and extension notifiers.

### 1.2 Why it matters for a usable Kurobayomi

Without this area Kurobayomi looks like a bare GTK prototype. The mockup currently has:

1. No splash page or ready gating (`p1-ui-parity.md` section 1).
2. Generic tab icons, not per-tab icons, and no selected-state animation (section 2).
3. No download indicator, no overflow icon and no drag highlight on download rows (section 5).
4. A crop button that uses the wrong icon, `edit-cut-symbolic` (`p1-rust-design.md` section 11, `src/player.rs:100`).
5. No CSS provider at all, so no theme choice, no AMOLED switch and no scrim (`p1-ui-parity.md` section 16).

What matters most for daily use is the reader and player icons (reading modes, crop, transport, download indicator), the GTK theme following behaviour, and a working set of symbolic icons. The palette and animation work is polish.

### 1.3 Out of scope for this area

- The Compose colour schemes and the base palette tables for 16 palettes: owned by `area:49-theme-and-design-system`.
- The theme picker screen and preference widgets: `area:47-settings-screens-general`, `area:47b-settings-widgets-search-more`.
- Cover loading and the cover image widget: `area:34-images-covers-cache`.
- The reader and player screens themselves: `area:43-reader-core`, `area:44-reader-viewers`, `area:45-player-core`, `area:46-player-controls-ui`.

---

## 2. Module map

### 2.1 Kotlin module map (where the resources are consumed)

| Package or file (repo-relative) | Responsibility | Resources used |
|---|---|---|
| `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt` | Enum of 22 theme constants and their label keys | style names in `values/themes.xml` |
| `app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt` | `LIGHT`, `DARK`, `SYSTEM` mapping to `AppCompatDelegate` modes (`ThemeMode.kt:11-18`) | none |
| `app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt` | Preference keys `pref_app_theme`, `pref_theme_dark_amoled_key`, `pref_theme_mode_key` | none |
| `app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt` | Picks the style resource id per `AppTheme`, applies the AMOLED overlay | `values/themes.xml`, `values-night/themes.xml` |
| `app/src/main/java/eu/kanade/presentation/theme/TachiyomiTheme.kt` | Compose theme root, colour scheme selection, ripple configuration | `presentation/theme/colorscheme/*.kt` |
| `app/src/main/java/eu/kanade/presentation/theme/colorscheme/` | Compose `ColorScheme` objects per palette (the Compose source of truth, `p1-rust-design.md` section 6) | colour values (cross-checked against XML) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt` | Splash lifecycle, ready gating, pre-Android-12 exit animation | `Theme.Tachiyomi.SplashScreen`, `ic_tachi_splash`, `splash` colour |
| `app/src/main/java/eu/kanade/tachiyomi/util/system/ContextExtensions.kt` | Reader theme context (`createReaderThemeContext`, lines 122-141) | `pref_reader_theme_key` |
| `app/src/main/java/eu/kanade/presentation/entries/**`, `.../presentation/entries/components/EntryBottomActionMenu.kt` | Entry list rows, download indicator, mark-previous action, caret | `ic_download_item_24dp`, `ic_done_prev_24dp`, `ic_overflow_24dp` |
| `app/src/main/java/eu/kanade/presentation/more/LogoHeader.kt` | More-tab logo header | `ic_ani` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt` | Reading-mode icons | `ic_reader_default/ltr/rtl/vertical/webtoon/continuous_vertical_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/BottomReaderBar.kt` | Crop toggle in the reader bottom bar | `ic_crop_24dp`, `ic_crop_off_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt` | Double-tap seek triangles | `ic_play_seek_triangle` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt` | Subtitle typography rows | `outline_brand_family_24`, `sharp_shadow_24` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt` | Android picture-in-picture actions (dropped on Linux) | `ic_forward_10_24dp`, `ic_pause_24dp`, `ic_play_arrow_24dp`, `ic_skip_next_24dp`, `ic_skip_previous_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt`, `.../download/anime/AnimeDownloadNotifier.kt` | Download notification small icons and actions | `ic_book_24dp`, `ic_play_arrow_24dp`, `ic_pause_24dp`, `ic_warning_white_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/data/library/{anime,manga}/*LibraryUpdateNotifier.kt` | Library-update notifications | `ic_done_24dp`, `ic_refresh_24dp`, `ic_close_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt` | App-update notification | `ic_info_24dp`, `ic_refresh_24dp`, `ic_system_update_alt_white_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt` | Backup notification | `ic_folder_24dp`, `ic_share_24dp`, `ic_close_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt` | Extension-update notification | `ic_extension_24dp` |
| `app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt` | Save-image notification | `ic_photo_24dp`, `ic_share_24dp` |
| `app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt` | Navigation-style icons | `ic_updates_outline_24dp` |
| `app/shortcuts.xml` | Launcher shortcut list (five entries) | `sc_*_48dp.xml` |
| `app/src/main/res/layout/download_item.xml`, `download_header.xml` | View-based download rows (overflow, drag handle) | `ic_overflow_24dp`, `ic_drag_handle_24dp` |

Other Kotlin references are listed per resource in `p1-inventory.md`, which was checked repo-wide.

### 2.2 Proposed Rust module map (Kurobayomi)

From `p1-rust-design.md` section 1. Files marked (new) do not exist yet. Files marked (existing) exist in the mockup.

```
kurobayomi/
  build.rs                      (new) compiles data/kurobayomi.gresource.xml -> $OUT_DIR/kurobayomi.gresource
  data/
    kurobayomi.gresource.xml    (new) bundles style.css, icons/, splash resources
    style.css                   (new) all CSS rules for this area
    icons/hicolor/scalable/actions/kb-*-symbolic.svg               (new, about 30 files)
    icons/hicolor/scalable/apps/io.github.blackbox.Kurobayomi.svg  (new)
    icons/hicolor/symbolic/apps/io.github.blackbox.Kurobayomi-symbolic.svg (new)
    io.github.blackbox.Kurobayomi.desktop   (new, five [Desktop Action] groups)
  tools/
    export_icons.py             (new) one-time VectorDrawable -> symbolic SVG export (stdlib only)
    check_palette.py            (new) XML vs Rust palette drift check
    check_svg.py                (new) symbolic SVG fill check
    check_themes.py             (new) inheritance table check
  tests/
    css_parses.rs               (new) Xvfb test
    icons_resolve.rs            (new) Xvfb test
    desktop_file.rs             (new)
  src/
    main.rs        (existing) calls theme::install and icons::register in build_ui before the window is shown
    theme.rs       (new) AppTheme enum, Palette struct, palette constants, CSS string builders, install()
    icons.rs       (new) Icon enum -> icon name, register(), resolution test helper
    anim.rs        (new) crossfade / rotate / timeline helpers (GTK main loop only)
    splash.rs      (new) splash page and the 500/5000/400 ms state machine
    ui.rs          (existing) cover(), menu_button(), linked(), row(), status() switch to icons::Icon
    player.rs      (existing) reader/player controls switch to icons::Icon and the seek-triangle overlay
    entry.rs       (existing) download/entry icons and the caret expander
    settings.rs    (existing) theme picker uses theme::AppTheme
```

Key types (from `p1-rust-design.md` sections 2.1 to 2.3):

- `theme::AppTheme { Default, CottonCandy, Mocha }`, with `from_pref(&str)` and `pref_name(self)`. Unknown names, `MONET`, `DARK_BLUE`, `HOT_PINK` and `BLUE` all map to `Default`.
- `theme::Palette`, with constants `COTTONCANDY_LIGHT`, `COTTONCANDY_DARK`, `MOCHA_LIGHT`, `MOCHA_DARK`.
- `icons::Icon`, an enum with one variant per icon, `name() -> &'static str`, and `register(display)`.
- `anim` helpers for crossfade and rotate. Driven by the GTK main loop only.

Threading (`p1-rust-design.md` section 3): everything runs on the GTK main loop. No tokio, no worker threads, no runtime file IO. CSS and icon bytes come from the GResource bundle compiled into the binary.

---

## 3. Reading guide

All documents are in this folder. Read in this order for implementation.

1. [p1-inventory.md](p1-inventory.md): every one of the 92 files, with its decision (Port, Rewrite, Replace, Drop), target module and task ids. Start here to find a file.
2. [p1-behavior.md](p1-behavior.md): what the Kotlin and XML do, with exact timings, the theme resolution rules, the splash lifecycle, the icon usage and the double-tap seek schedule. Also lists the proposed Linux behaviour for each item.
3. [p1-data-and-config.md](p1-data-and-config.md): value formats (colours with alpha, dimensions, path data grammar), the `AppTheme` and `ThemeMode` enums, preference keys and defaults, and what is explicitly not in this area.
4. [p1-rust-design.md](p1-rust-design.md): crate and module layout, the Rust types, the icon export pipeline, the stylesheet, the desktop actions, the Android-to-Linux mapping table and the pitfalls.
5. [p1-ui-parity.md](p1-ui-parity.md): what each screen should look like on the desktop for each element in scope (splash, tabs, entry header, covers, queue rows, reader bar, player, dialogs, switches, logo header, shortcuts, notifications), and the consolidated list of what the mockup lacks.
6. [p1-topic-icons.md](p1-topic-icons.md): the icon catalogue, the export procedure step by step, the list of exported `kb-*` symbolic icons, the standard freedesktop names used instead of exporting, the logo and launcher icons, and the pixel checks.
7. [p1-topic-animations.md](p1-topic-animations.md): the nine `anim_*.xml` files, their exact timelines, Compose driver semantics, GTK CSS replacements, the interpolator mapping and the timing summary.
8. [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases per feature, failure modes, races and ordering, fixtures and the new tests T1 to T14 (automated and manual).
9. [p1-tasks.md](p1-tasks.md): the 50 implementation tasks (`T-49ba-001` to `T-49ba-050`), with priority, effort, dependencies, sources and acceptance criteria. Lint passes (`python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint`).

Related area documents owned elsewhere: [area 49 folder](../49-theme-and-design-system/) (palette tables, theme mode, scheme resolution).

---

## 4. Key design decisions and open questions

Each item needs the user's call unless marked "decided". The recommendation is what this plan assumes until the user says otherwise.

### 4.1 Decided in the plan (no action needed unless the user objects)

1. **Follow the GTK theme by default.** `AppTheme.Default` loads no palette CSS. The window uses the active GTK theme (on the user's machine, Tokyo Night). Source: `p1-rust-design.md` section 2.1; task T-49ba-001. Reason: the user's rule is to match the current theme and never reset to Breeze or stock looks.
2. **Drop Android dynamic colour (Monet).** Wallpaper-derived palettes (`values-v31/themes.xml`, `values-night-v31/themes.xml`) have no Linux source of truth here. Task T-49ba-007. See conflict in 4.3.
3. **Drop Android window flags and API-27 navigation-bar overrides.** `values/bools.xml`, `values-night/bools.xml`, `values-v27/bools.xml`, the window attributes in `values/themes.xml` and the `windowLightStatusBar` family. Tasks T-49ba-008, T-49ba-047.
4. **Icons are symbolic SVGs exported from VectorDrawable, recoloured by GTK.** Every exported symbolic file uses `fill="#bebebe"` so GTK can recolour it. Full-colour files (app icon) keep their colours. Task T-49ba-010.
5. **Use freedesktop icon names wherever a standard name exists.** Examples: `media-playback-start-symbolic`, `media-skip-forward-symbolic`, `view-refresh-symbolic`, `folder-symbolic`. No export is made for these. Table in `p1-topic-icons.md` section 3.
6. **Replace the 48 dp adaptive shortcut bitmaps with `.desktop` `[Desktop Action]` entries and icon names.** Task T-49ba-037, T-49ba-038.
7. **Resources are compiled into the binary (GResource), and no runtime XML parsing is done.** `build.rs` plus `kurobayomi.gresource.xml`.
8. **Drop unused drawables (14 files), unused style items, unused attributes, the unused NumberPicker and widget corner radii.** Tasks T-49ba-039 to T-49ba-042, T-49ba-040, T-49ba-041.
9. **Drop the More-tab three-dot morph.** `anim_more_enter.xml` (201 lines, used by `app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt`) is decorative. Its `time_group` target has no children. The More tab shows a static `Icon::MoreHorizontal` instead. Task T-49ba-023.
10. **Only the More-tab morph is dropped from the animations.** The other eight `anim_*.xml` files stay in scope (T-49ba-022 for the six tab-enter files, T-49ba-021 for the caret, T-49ba-024 for play/pause).
11. **Drop picture-in-picture actions** (`PipActions.kt`). There is no Linux PiP equivalent in the target. The icons they use are ported or replaced in tasks T-49ba-012 and T-49ba-028.

### 4.2 Open questions for the user

Q1. **Which palettes to ship.** The enum has 22 constants. Three are deprecated (`DARK_BLUE`, `HOT_PINK`, `BLUE`, all hidden). This area implements only CottonCandy and Mocha (plus Default). The other 16 are in area 49 (`plans/49-theme-and-design-system/p1-topic-color-token-tables.md`).
- Options: (a) ship Default, CottonCandy and Mocha only; (b) port all 18 palettes through area 49; (c) ship Default only.
- Recommendation: (a) now. It covers the two palettes the user can compare against Android. Port the rest only if the user asks for them. Tasks T-49ba-004, T-49ba-006.

Q2. **Aniyomi branding in the app icon and logo.** The launcher foreground (`ic_launcher_foreground.xml`) and the logo (`ic_ani.xml`) are Aniyomi artwork. T-49ba-017 and T-49ba-018 ship them as Kurobayomi's app icon and in-app logo. The repo's license is Apache-2.0 (per the task brief), but using another project's logo as this app's identity may confuse users and needs attribution.
- Options: (a) keep the Aniyomi artwork with an attribution note in the About page and a NOTICE file; (b) replace the app icon and logo with a new Kurobayomi design and keep the exported artwork only for the reader and player icons; (c) use a generic icon name for the app.
- Recommendation: (b). Decide before T-49ba-017 is done. The exported reader and player icons are Material-style glyphs that need the same attribution in either case.

Q3. **Attribution for exported icons.** Every exported `kb-*` SVG comes from Aniyomi's Android drawables. No task in this area writes an attribution file. Confirm that a NOTICE entry is wanted for the exported icons (Apache-2.0 requires keeping the notice for derived works, see the repo LICENSE).
- Recommendation: add a `data/icons/NOTICE` file in T-49ba-010 that lists the source paths. Costs nothing.

Q4. **Animations in the GTK UI.** The nine animated vectors become CSS transitions or cairo drawing. The plan keeps six tab-enter animations (T-49ba-022, P2) and drops the More-tab morph. `-gtk-icon-transform` on `gtk::Image` is not verified on GTK 4.16.
- Options: (a) implement the animations as planned, with a `DrawingArea` fallback; (b) skip them and use static icons only.
- Recommendation: (b) for the first release. Tasks T-49ba-021, T-49ba-022, T-49ba-046 are P2 and do not block daily use.

Q5. **Respect the GTK animations setting.** `gtk-enable-animations` should disable the caret rotation and the crossfades (T-49ba-046). Confirm this matches the user's expectation on KDE. Recommendation: yes.

Q6. **AMOLED pure black.** The AMOLED overlay (`values-night/themes.xml`, `ThemingDelegate.kt:28`) is a preference, default `false`. Recommendation: keep the preference, default `false`, and show it only in dark mode (T-49ba-003).

Q7. **Splash page timing.** The Android splash waits for the first browse-tab ready signal, with a 500 ms minimum and a 5000 ms maximum. Recommendation: keep both values exactly (T-49ba-020), because they are the contract the user already knows.

### 4.3 Conflicts with area 49 (must be resolved before implementation)

Area 49 has its own task list (`plans/49-theme-and-design-system/p2-tasks.md`, tasks T-49b-xxx). It overlaps this area in five places.

| Topic | Area 49b task | Area 49 task | Conflict | Recommendation |
|---|---|---|---|---|
| CSS provider lifecycle | T-49ba-001 (install both providers in `theme.rs`) | T-49b-008 (generated GTK CSS and a single `CssProvider` lifecycle) | Two owners for one provider | Area 49 owns the provider and the generated CSS. Area 49b provides the static `style.css` and the palette strings. Make T-49ba-001 depend on T-49b-008 or merge them. |
| Dynamic colour | T-49ba-007 (drop Monet) | T-49b-005 (Dynamic (Monet) option on Linux from the XDG portal) | Opposite decisions | Ask the user (Q8 below). |
| AMOLED | T-49ba-003 | T-49b-003 (scheme resolution, dark flag, AMOLED override) and T-49b-015 (pure-black switch) | Duplicate | Keep the logic in area 49. T-49ba-003 only adds the CSS class `kb-amoled`. |
| Scheme tables | T-49ba-004, T-49ba-005, T-49ba-006 | T-49b-002 (port the 18 static scheme tables from the XML) and T-49b-039 (drift checker) | Two drift checkers | Keep one checker. Recommend area 49 (`T-49b-039`) and drop `tools/check_palette.py` (T6 in `p1-edge-cases-and-tests.md`). |
| AppTheme enum | T-49ba-002 | T-49b-043 (AppTheme enum port, titles from gettext, deprecated values) | Two enum definitions | Area 49 owns the enum. Area 49b keeps only `from_pref` and the Default fallback. |

Q8. **Dynamic colour on Linux.** Area 49's T-49b-005 proposes an XDG-portal-based dynamic palette, which is the Linux analogue of Monet. Area 49b drops Monet. Recommendation: keep the drop for now (P3, not in P0 or P1). Revisit only if the user wants the Plasma accent colour to drive the palette.

### 4.4 Dependencies on the whole of area 49

T-49ba-001 depends on `area:49-theme-and-design-system` as a whole, which is too broad. The GTK CSS provider in area 49 (T-49b-008) is the real dependency. Narrow the dependency to T-49b-008 once area 49's tasks are fixed.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from other areas

| Needs | From | Task that depends | Why |
|---|---|---|---|
| Palette tables (18), theme mode, dark flag, CSS provider lifecycle | `area:49-theme-and-design-system` (`T-49b-002`, `T-49b-003`, `T-49b-008`, `T-49b-039`, `T-49b-043`) | T-49ba-001, T-49ba-006 | Colour source of truth and provider lifecycle |
| Reader pager widget name and a stable reader view | `area:43-reader-core` | T-49ba-036 (P0) | The `reader_pager` id (`values/ids.xml`, `PagerViewer.kt:88`) must map to a widget |
| Preference store (for `pref_app_theme`, AMOLED, theme mode) | `area:02-preferences-system` | T-47ba-040 depends on it; this area's T-49ba-002 needs the key names | Stored enum values and defaults |
| Splash ready signal (first browse-tab ready) | `area:36-ui-shell-navigation-security-webview` or the browse area (`area:40-ui-browse-manga`, `area:41-ui-browse-anime`) | T-49ba-020 | `BrowseTab` sets `ready` (`BrowseTab.kt:103-104`) |
| Entry list, caret, download row, library surfaces | `area:37-ui-library`, `area:38-ui-entry-details-manga`, `area:39-ui-entry-details-anime`, `area:42-ui-updates-history-stats-storage-upcoming` | T-49ba-019, T-49ba-021, T-49ba-026, T-49ba-027 | The views that consume the icons |
| Reader and player controls | `area:44-reader-viewers`, `area:45-player-core`, `area:46-player-controls-ui` | T-49ba-024, T-49ba-025, T-49ba-028 | The views that consume reader and player icons |
| Notifications (desktop, `gio::Notification`) | `area:35-data-misc-updater-notification-export` | T-49ba-013 | The names are used in `gio::ThemedIcon` |
| Desktop entry and packaging | `area:53-platform-manifest-intents-workers`, `area:54-build-release-distribution-docs` | T-49ba-037, T-49ba-038, T-49ba-044 | `.desktop` file and build tasks |
| i18n of theme labels | `area:51-i18n` | T-49ba-002 (titles from gettext) | Label keys such as `theme_cottoncandy` |

### 5.2 What other areas need from this area

Taken from the existing task files that depend on this area (`TASKS.csv`, rows whose depends cite `area:49-theme-and-design-system` or `T-49ba-*`):

| Consumer task | Needs from 49b |
|---|---|
| `T-36a-029` (App theme application, `area:36`) | `theme::install`, `AppTheme`, the AMOLED class |
| `T-47ba-040`, `T-47ba-041` (theme mode selector and theme strip, `area:47b`) | `AppTheme::from_pref`, `pref_name` and the palette preview data |
| `T-34a-052` (`CoverImage` widget, `area:34`) | the cover placeholder (`Icon::Cover`, `.kb-cover-placeholder`, T-49ba-016) |
| `T-27a-050` (logo tile, `area:27`) | the logo icon (`kb-logo` symbolic, T-49ba-018) |
| `T-39b-034` (season display modes, `area:39`) | the grid margins (T-49ba-034) |
| `T-35a-062` (release notes renderer, `area:35`) | the CSS text classes (T-49ba-031) |
| `T-50a-021` (widget colours from the Plasma theme, `area:50`) | the palette override mechanism (T-49ba-001) |
| `T-51a-027` (font coverage, `area:51`) | none directly; font families are GTK defaults |
| `T-53a-032` (drop Android application attributes, `area:53`) | the drop decisions (T-49ba-047) |
| `T-54a-044` (presentation-core module to kb-ui, `area:54`) | the icon and stylesheet layout (`data/`) |
| `T-req-platform-009` (bundled icons and CSS, `addenda`) | GResource bundling (T-49ba-009) |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the rows of `TASKS.csv` whose file starts with `49b-android-res-values-drawables/`. Lint: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint p1-tasks.md` prints `ok`.

Total: 50 tasks (`T-49ba-001` to `T-49ba-050`).

| Priority | S | M | L | XL | Total |
|---|---|---|---|---|---|
| P0 | 11 | 2 | 0 | 0 | 13 |
| P1 | 17 | 2 | 0 | 0 | 19 |
| P2 | 7 | 3 | 0 | 0 | 10 |
| P3 | 8 | 0 | 0 | 0 | 8 |
| **Total** | **43** | **7** | **0** | **0** | **50** |

### 6.1 P0 critical path (in order)

The P0 tasks are the ones needed for daily reading and watching: the stylesheet, the icon pipeline, reader and player icons, the download indicator, the caret, the cover placeholder, and the reader pager name.

1. `area:49-theme-and-design-system` (external). T-49ba-001 cannot start until the CSS provider in area 49 exists (see 4.3 and 4.4).
2. **T-49ba-001** (P0, M): GTK-theme-following base stylesheet and palette provider.
3. **T-49ba-009** (P0, S): icon enum, name table and GResource registration. Needs T-49ba-001 (the bundle lives under the same `build.rs`).
4. **T-49ba-010** (P0, M): icon export pipeline (VectorDrawable to symbolic SVG). Needs T-49ba-009.
5. In parallel after step 4:
   - **T-49ba-015** (P0, S): anime library two-state icon.
   - **T-49ba-017** (P0, S): application icon. Blocked on the decision in Q2.
   - **T-49ba-025** (P0, S): reader reading-mode icons.
   - **T-49ba-026** (P0, S): download indicator icon.
6. In parallel after T-49ba-009: **T-49ba-012** (P0, S, standard transport, close, warning icon replacements) and **T-49ba-019** (P0, S, entry expand/collapse caret static states).
7. In parallel after T-49ba-001: **T-49ba-002** (P0, S, AppTheme preference model), **T-49ba-016** (P0, S, cover placeholder), **T-49ba-045** (P0, S, stylesheet and CSS-provider failure handling).
8. External, in parallel: **T-49ba-036** (P0, S, reader pager widget name) depends on `area:43-reader-core`.

Longest chain: T-49ba-001 (M) to T-49ba-009 (S) to T-49ba-010 (M) to T-49ba-025 (S). Using the effort bounds (S up to 1 day, M up to 3 days), that is roughly 3 to 8 days of work for one person, excluding the area 49 dependency.

### 6.2 Risks

1. **Area 49 coupling.** The provider lifecycle and the scheme tables are owned by area 49. A slip there blocks T-49ba-001 and every P0 task after it. Mitigation: narrow the dependency (4.4), and build T-49ba-009 to T-49ba-012 without the palette override.
2. **Aniyomi artwork and licence.** See Q2 and Q3. A late decision delays T-49ba-017 and T-49ba-018 only.
3. **Icon export fidelity.** Exported SVGs must render like the Android drawables at 16 px and 24 px. Mitigation: pixel checks in `p1-topic-icons.md` section 6, and T7 (`tools/check_svg.py`).
4. **GTK CSS named colours.** The stylesheet uses `@theme_*` names. These are legacy GTK 4 names. A future GTK release may change or deprecate them. Mitigation: T1 (`tests/css_parses.rs`) fails on any undefined name, and the names are kept in one file.
5. **`-gtk-icon-transform` on GTK 4.16.** Not verified. Only the P2 animation tasks depend on it (T-49ba-022, T-49ba-046). Mitigation: the `DrawingArea` fallback in `p1-topic-animations.md` section 4.
6. **Palette drift.** The Compose and XML colours differ in two tokens (`p1-rust-design.md` section 6). Mitigation: both corrections are documented, and T6 checks the rest.
7. **Scope creep from the 92 files.** Most of the scope is dropped or replaced. Mitigation: the decision column in `p1-inventory.md` is authoritative. Do not port a drawable that the table marks Drop.

---

## 7. Definition of done for this area in Kurobayomi

Each item is observable on the user's machine (CachyOS, KDE Plasma, Tokyo Night GTK theme) unless noted.

1. **Default theme follows GTK.** On first launch with `AppTheme.Default`, the window background equals the active GTK theme background, no palette CSS is loaded, and the bundled `style.css` parses without errors (T1, T14).
2. **Theme choice works and persists.** Choosing CottonCandy or Mocha in the settings theme picker changes the window colours and the choice survives a restart. An unknown stored value (for example `MONET`, `HOT_PINK`, or a typo) falls back to Default without a crash (T3).
3. **AMOLED.** With dark mode and the AMOLED switch on, the window background is pure black. The switch is hidden or disabled in light mode.
4. **Splash.** At launch, a splash page with the logo at 108 px shows on the splash background colour. It hides once the library is ready, no sooner than 500 ms and no later than 5000 ms (T11, and a stopwatch check in `p1-edge-cases-and-tests.md` section 7).
5. **Every icon resolves.** Every variant of `Icon` resolves in the GTK icon theme after `icons::register` (T2). A missing export fails the test.
6. **Reader icons.** The reader bottom bar shows the crop and crop-off toggle with the correct icons (not `edit-cut-symbolic`). The reading-mode dialog shows an icon for each of the six reading modes: default, left to right, right to left, vertical, webtoon, and continuous vertical (`ic_reader_*_24dp.xml`).
7. **Player.** The double-tap seek triangles (three copies of `ic_play_seek_triangle`, 16 x 20 px, rotated 180 degrees for backward seeks) cycle through six 150 ms steps in a 750 ms loop, matching `p1-behavior.md` section 4.5 (T10). The centre play/pause button crossfades between play and pause.
8. **Download rows.** Downloaded episodes and chapters show the download indicator. Download rows have an overflow icon (vertical three dots) and a drag handle. A card being dragged shows the drag tint (alpha 0.08 of the primary colour).
9. **Notifications.** Every notifier that used a small icon sends a notification with a named symbolic icon from `p1-rust-design.md` section 11 (for example `view-refresh-symbolic`, `folder-symbolic`). No notification shows an empty icon.
10. **Desktop actions.** The `.desktop` file validates with `desktop-file-validate` and has five unique `[Desktop Action]` entries (T13). Each action opens the matching screen (library, manga library, browse, recent manga, recent updates).
11. **Launcher icon.** The application icon appears in the KDE application menu and task switcher with its full colours (T-49ba-017, subject to Q2).
12. **Stylesheet failures are contained.** A CSS parse error is logged and the app still starts with the GTK theme (T-49ba-045).
13. **Tests.** `cargo test` (under `xvfb-run -a`) passes T1, T2, T3, T4, T8, T10, T11, T12, T13. The Python checks T6, T7 and T9 pass. Manual checks T14 and the screenshot checks in `p1-edge-cases-and-tests.md` section 7 are recorded in the release notes.
14. **No runtime resource IO.** The binary reads no Android XML, PNG or SVG from disk at runtime. All resources come from the GResource bundle.
