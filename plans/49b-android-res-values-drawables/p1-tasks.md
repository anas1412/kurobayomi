# Area 49b (part 1): implementation tasks

Task ids use the prefix T-49ba-. Every scope path appears in at least one task's Sources line (coverage checked with the self-check in the area brief). Dependencies are other tasks in this file or `area:` ids. Design details: `p1-rust-design.md`. Behaviour: `p1-behavior.md`. Icon list and export: `p1-topic-icons.md`. Animation timelines: `p1-topic-animations.md`.

### T-49ba-001 — GTK-theme-following base stylesheet and palette provider
- Priority: P0
- Effort: M
- Depends: area:49-theme-and-design-system
- Sources: app/src/main/res/values/themes.xml, app/src/main/java/eu/kanade/tachiyomi/ui/base/delegate/ThemingDelegate.kt
- Acceptance:
  - `src/theme.rs` exposes `install(display, theme, dark)` and both `CssProvider`s are installed before the first window is presented (`src/main.rs` `build_ui`).
  - With the default `AppTheme::Default` no palette CSS is loaded: the provider content is empty and `@theme_bg_color` on the window equals the active GTK theme value (Tokyo Night check).
  - `data/style.css` is bundled as `/io/github/anas1412/Kurobayomi/style.css` and loads with zero `parsing-error` events (test T1).

### T-49ba-002 — AppTheme preference model and stored-value compatibility
- Priority: P0
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt, app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt, app/src/main/res/values/themes.xml
- Acceptance:
  - `enum AppTheme { Default, CottonCandy, Mocha }` with `from_pref` and `pref_name` in `src/theme.rs`.
  - Stored strings `DEFAULT`, `COTTONCANDY`, `MOCHA` map to their variants; all 19 other Android names (including `MONET`, `BLUE`, `HOT_PINK`, `DARK_BLUE`, lowercase `mocha`, the empty string) map to `Default` without panicking (test T3).
  - Unknown values are not written back to settings until the user changes the choice.

### T-49ba-003 — AMOLED overlay for dark mode
- Priority: P1
- Effort: S
- Depends: T-49ba-001, T-49ba-002
- Sources: app/src/main/res/values-night/themes.xml, app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt
- Acceptance:
  - Setting `pref_theme_dark_amoled_key` = true in dark mode makes the window background `#000000` and the card/list surfaces `#0C0C0C`, with `#131313` and `#1B1B1B` for the highest containers (values from `values-night/themes.xml`).
  - In light mode the flag has no visible effect (the `kb-amoled` class is not added).
  - The class is applied on the top-level window and removed when the flag is turned off, without restarting the app.

### T-49ba-004 — CottonCandy and Mocha palettes (light and dark)
- Priority: P2
- Effort: M
- Depends: T-49ba-001
- Sources: app/src/main/res/values/colors_cottoncandy.xml, app/src/main/res/values-night/colors_cottoncandy.xml, app/src/main/res/values/colors_mocha.xml, app/src/main/res/values-night/colors_mocha.xml, app/src/main/res/values/themes.xml, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/CottoncandyColorScheme.kt
- Acceptance:
  - `COTTONCANDY_LIGHT`, `COTTONCANDY_DARK`, `MOCHA_LIGHT`, `MOCHA_DARK` constants exist in `src/theme.rs` with all 22 `Palette` fields filled (values in p1-rust-design.md section 2.2).
  - Choosing CottonCandy in dark mode changes the window background to `#201A1B` and the selected-item colour to `#FFB1C1` (`theme_selected_bg_color`).
  - `palette_css` output for each palette contains the 22 `@define-color` names listed in the design.

### T-49ba-005 — Palette drift decisions (CottonCandy dark onTertiary, Mocha light inverseOnSurface)
- Priority: P2
- Effort: S
- Depends: T-49ba-004
- Sources: app/src/main/res/values-night/colors_cottoncandy.xml, app/src/main/res/values/colors_mocha.xml, app/src/main/java/eu/kanade/presentation/theme/colorscheme/CottoncandyColorScheme.kt, app/src/main/java/eu/kanade/presentation/theme/colorscheme/MochaColorScheme.kt
- Acceptance:
  - `COTTONCANDY_DARK.on_tertiary` = `0xFF5F112B` (Compose value; XML night file has no token).
  - `MOCHA_LIGHT.inverse_on_surface` = `0xFFFBEAEB` (Compose value; XML `#FBEEEB`).
  - The two decisions are written as comments in `src/theme.rs` and listed in the drift test T6, which fails on any other difference.

### T-49ba-006 — Remaining palettes handed to area 49 and inheritance check
- Priority: P2
- Effort: S
- Depends: area:49-theme-and-design-system
- Sources: app/src/main/res/values/themes.xml, app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt
- Acceptance:
  - The 16 other palette styles (GreenApple, Lavender, MidnightDusk, StrawberryDaiquiri, Tako, TealTurquoise, YinYang, Yotsuba, Cloudflare, Sapphire, Doom, Matrix, TidalWave, Nord, Monochrome) are implemented by area 49 and not duplicated here; this task only checks that the Kurobayomi `AppTheme` enum has no entry for them beyond what 002 states.
  - Test T9 reproduces the omitted-token table from `p1-edge-cases-and-tests.md` section 2.2 from `values/themes.xml` and passes.

### T-49ba-007 — Drop Android dynamic colour (Monet)
- Priority: P3
- Effort: S
- Depends: T-49ba-002
- Sources: app/src/main/res/values-v31/themes.xml, app/src/main/res/values-night-v31/themes.xml
- Acceptance:
  - No code path reads `m3_sys_color_dynamic_*` or `m3_dynamic_*` tokens.
  - `AppTheme::Monet` does not exist; the stored `MONET` value maps to `Default` (T3). Documented in the settings help text owned by area 47.

### T-49ba-008 — Drop Android window bools and API-27 navigation-bar overrides
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/values/bools.xml, app/src/main/res/values-night/bools.xml, app/src/main/res/values-v27/bools.xml, app/src/main/res/values-v27/themes.xml
- Acceptance:
  - No `lightStatusBar`, `lightNavigationBar`, `elevationOverlayEnabled` or `windowLightNavigationBar` concept exists in Kurobayomi code.
  - The dark-mode decision that these flags once carried is made from the GTK window background (T-49ba-001, edge case E7).

### T-49ba-009 — Icon enum, name table and GResource registration
- Priority: P0
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/drawable/ic_close_24dp.xml, app/src/main/res/drawable/ic_pause_24dp.xml, app/src/main/res/drawable/ic_play_arrow_24dp.xml, app/src/main/res/drawable/ic_skip_next_24dp.xml, app/src/main/res/drawable/ic_skip_previous_24dp.xml
- Acceptance:
  - `src/icons.rs` defines `enum Icon` with the variants in `p1-rust-design.md` section 2.3 and `name()` returns exactly the listed names.
  - `icons::register(display)` adds `/io/github/anas1412/Kurobayomi/icons` to the icon theme before the window is shown.
  - Test T2 passes: every variant resolves with `IconTheme::has_icon`.

### T-49ba-010 — Icon export pipeline (VectorDrawable to symbolic SVG)
- Priority: P0
- Effort: M
- Depends: T-49ba-009
- Sources: app/src/main/res/drawable/ic_ani.xml, app/src/main/res/drawable/ic_reader_default_24dp.xml, app/src/main/res/drawable/ic_crop_24dp.xml, app/src/main/res/drawable/ic_animelibrary_filled_24dp.xml, app/src/main/res/drawable/ic_download_item_24dp.xml
- Acceptance:
  - `tools/export_icons.py` (Python 3 stdlib) converts each Port drawable listed in `p1-topic-icons.md` to `data/icons/hicolor/scalable/actions/kb-*-symbolic.svg` with `fill="#bebebe"`; the viewBox equals the Android viewport.
  - T7 passes: every exported symbolic SVG contains `#bebebe` and no `#FFF` or `#FFFFFF` fills.
  - At 16 px and 24 px each icon is visually identical to the Android render (manual compare on a screenshot).

### T-49ba-011 — Crop on and crop off icons (reader bottom bar)
- Priority: P1
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_crop_24dp.xml, app/src/main/res/drawable/ic_crop_off_24dp.xml, app/src/main/java/eu/kanade/presentation/reader/appbars/BottomReaderBar.kt
- Acceptance:
  - `Icon::CropOn` shows when crop borders is on and `Icon::CropOff` when off, matching `BottomReaderBar.kt:59`.
  - The mockup's `edit-cut-symbolic` crop button (`src/player.rs:100`) is replaced.
  - The toggle has the tooltip "Crop borders" and toggles on click and on key `c` (Proposal).

### T-49ba-012 — Standard transport, close and warning icon replacements
- Priority: P0
- Effort: S
- Depends: T-49ba-009
- Sources: app/src/main/res/drawable/ic_close_24dp.xml, app/src/main/res/drawable/ic_pause_24dp.xml, app/src/main/res/drawable/ic_play_arrow_24dp.xml, app/src/main/res/drawable/ic_skip_next_24dp.xml, app/src/main/res/drawable/ic_skip_previous_24dp.xml, app/src/main/res/drawable/ic_warning_white_24dp.xml
- Acceptance:
  - Dialog close buttons use `Icon::Close` (`window-close-symbolic`); the player transport uses `Icon::Play`, `Icon::Pause`, `Icon::SkipNext`, `Icon::SkipPrevious`.
  - Download and update error notifications use `Icon::Warning` (`dialog-warning-symbolic`).
  - Each icon resolves (T2).

### T-49ba-013 — Notification small-icon names
- Priority: P1
- Effort: S
- Depends: T-49ba-009
- Sources: app/src/main/res/drawable/ic_book_24dp.xml, app/src/main/res/drawable/ic_done_24dp.xml, app/src/main/res/drawable/ic_dns_24dp.xml, app/src/main/res/drawable/ic_extension_24dp.xml, app/src/main/res/drawable/ic_folder_24dp.xml, app/src/main/res/drawable/ic_info_24dp.xml, app/src/main/res/drawable/ic_photo_24dp.xml, app/src/main/res/drawable/ic_refresh_24dp.xml, app/src/main/res/drawable/ic_share_24dp.xml, app/src/main/res/drawable/ic_system_update_alt_white_24dp.xml, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/manga/MangaLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/SaveImageNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt, app/src/main/java/eu/kanade/tachiyomi/extension/ExtensionUpdateNotifier.kt
- Acceptance:
  - Every notification the scope's icons appear in uses `gio::ThemedIcon` with the standard name from the inventory (for example `accessories-dictionary-symbolic` for book, `object-select-symbolic` for done).
  - Notifications default to `io.github.anas1412.Kurobayomi` (`Icon::Logo`) where Android used `ic_ani`.
  - No notification sets an accent colour (the `accent_blue` tint is dropped; see T-49ba-038 for the colour drop).

### T-49ba-014 — Entry-list calendar and drag-handle icons
- Priority: P1
- Effort: S
- Depends: T-49ba-009
- Sources: app/src/main/res/drawable/ic_calendar_clock_24dp.xml, app/src/main/res/drawable/ic_drag_handle_24dp.xml, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt, app/src/main/res/layout/download_header.xml, app/src/main/res/layout/download_item.xml
- Acceptance:
  - Anime release row uses `Icon::Calendar` (`x-office-calendar-symbolic`).
  - Download rows show `Icon::DragHandle` (`list-drag-handle-symbolic`) at the left edge with the 16 px margin.

### T-49ba-015 — Anime library two-state icon
- Priority: P0
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_animelibrary_filled_24dp.xml, app/src/main/res/drawable/ic_animelibrary_outline_24dp.xml, app/src/main/res/drawable/ic_animelibrary_selector_24dp.xml, app/src/main/res/drawable/anim_animelibrary_enter.xml, app/src/main/res/drawable/anim_animelibrary_leave.xml, app/src/main/res/menu/main_nav.xml, app/src/main/java/eu/kanade/tachiyomi/ui/library/anime/AnimeLibraryTab.kt
- Acceptance:
  - The anime library tab shows `AnimeLibraryOutline` when not selected and `AnimeLibraryFilled` when selected; the switch is immediate (the 300 ms crossfade is T-49ba-022).
  - The selected state is exposed to assistive tech as a toggle state.

### T-49ba-016 — Cover placeholder for failed covers
- Priority: P0
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/drawable/cover_error.xml, app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt, app/src/main/java/eu/kanade/presentation/browse/manga/components/BrowseMangaIcons.kt, app/src/main/java/eu/kanade/presentation/browse/anime/components/BrowseAnimeIcons.kt
- Acceptance:
  - `ui::cover()` uses the CSS class `.kb-cover-placeholder` (`rgba(136,136,136,0.1216)`) and `Icon::Cover` at `width / 3` pixels.
  - Test T8 checks the alpha rounding.
  - A cover whose load fails shows the placeholder, not a broken-image icon.

### T-49ba-017 — Application icon (full colour, hicolor)
- Priority: P0
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_launcher_foreground.xml, app/src/main/res/mipmap/ic_launcher.xml, app/src/main/res/mipmap/ic_launcher_round.xml
- Acceptance:
  - `data/icons/hicolor/scalable/apps/io.github.anas1412.Kurobayomi.svg` renders with the original colours (`#000`, `#2E84BF`, `#69A3CB`, `#C92824`, `#FFFFFF`, `#FF000000`) and no background tile (the Android mipmap background is transparent).
  - The `.desktop` file (T-49ba-037) uses `Icon=io.github.anas1412.Kurobayomi`.
  - The icon appears in the KDE application menu and task switcher.

### T-49ba-018 — Logo icon (in-app symbolic) and monochrome launcher symbol
- Priority: P1
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_ani.xml, app/src/main/res/drawable/ic_ani_monochrome_launcher.xml, app/src/main/java/eu/kanade/presentation/more/LogoHeader.kt
- Acceptance:
  - `data/icons/hicolor/symbolic/apps/io.github.anas1412.Kurobayomi-symbolic.svg` exists with `#bebebe` fills and renders the Aniyomi logo.
  - The More-page logo header (UI in p1-ui-parity.md section 12) shows a 64 px symbolic logo with 56 px vertical padding and a separator below.

### T-49ba-019 — Entry expand/collapse caret (static states)
- Priority: P0
- Effort: S
- Depends: T-49ba-009
- Sources: app/src/main/res/drawable/anim_caret_down.xml, app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeInfoHeader.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/MangaInfoHeader.kt
- Acceptance:
  - The entry info header shows `Icon::CaretUp` when expanded and `Icon::CaretDown` when collapsed.
  - Activating the button toggles the state; the tooltip is the "collapse" label when expanded and the "expand" label when collapsed (keys `manga_info_collapse` / `manga_info_expand`).

### T-49ba-020 — Splash page with ready gating
- Priority: P1
- Effort: M
- Depends: T-49ba-001
- Sources: app/src/main/res/values/themes.xml, app/src/main/res/drawable/ic_tachi_splash.xml, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt
- Acceptance:
  - `src/splash.rs` shows a `splash` stack page with `.kb-splash` (`#54759E`, or `#202125` with `.kb-dark`) and the logo at 108 px.
  - The splash hides at elapsed = 500 ms if the main page is ready, otherwise at 5000 ms at the latest (test T11).
  - The transition to the main page is a 400 ms crossfade; no splash on re-activation.

### T-49ba-021 — Overflow, mark-previous and caret rotation
- Priority: P2
- Effort: S
- Depends: T-49ba-019
- Sources: app/src/main/res/drawable/ic_done_prev_24dp.xml, app/src/main/res/drawable/ic_overflow_24dp.xml, app/src/main/java/eu/kanade/presentation/entries/components/EntryBottomActionMenu.kt, app/src/main/res/layout/download_item.xml, app/src/main/res/drawable/anim_caret_down.xml
- Acceptance:
  - Download rows show `Icon::OverflowVertical` opening a `gtk::PopoverMenu`.
  - The entry bottom bar has a mark-previous-as-read button with `Icon::DoneMarkPrevious`.
  - The caret rotates 180 degrees over 300 ms when toggled (`Rotation` class, see p1-topic-animations.md).

### T-49ba-022 — Tab-enter icon animations (six AVDs)
- Priority: P2
- Effort: M
- Depends: T-49ba-015
- Sources: app/src/main/res/drawable/anim_library_enter.xml, app/src/main/res/drawable/anim_browse_enter.xml, app/src/main/res/drawable/anim_history_enter.xml, app/src/main/res/drawable/anim_updates_enter.xml, app/src/main/res/drawable/anim_animelibrary_enter.xml, app/src/main/res/drawable/anim_animelibrary_leave.xml, app/src/main/java/eu/kanade/tachiyomi/ui/library/manga/MangaLibraryTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/browse/BrowseTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/history/HistoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/download/DownloadsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/updates/UpdatesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/category/CategoriesTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/stats/StatsTab.kt, app/src/main/java/eu/kanade/tachiyomi/ui/storage/StorageTab.kt
- Acceptance:
  - Browse rotates 0 to 180 degrees over 300 ms when selected; History rotates 360 to 0 over 500 ms when selected (`src/anim.rs` `rotate_icon`).
  - Library, Updates, Categories, Stats and Storage use a 300 ms crossfade between the static icon and its end-state icon.
  - Reversal on deselect; rapid toggling does not leave an icon mid-state (edge case A1).

### T-49ba-023 — Drop the More-tab three-dot morph
- Priority: P3
- Effort: S
- Depends: T-49ba-009
- Sources: app/src/main/res/drawable/anim_more_enter.xml, app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt
- Acceptance:
  - The More tab shows a static `Icon::MoreHorizontal`; no code plays the dot morph.
  - Stated reason in the code comment: the Android `time_group` target has no children, and the morph is decorative.

### T-49ba-024 — Play/pause crossfade in the player
- Priority: P1
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/anim_play_to_pause.xml, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/MiddlePlayerControls.kt
- Acceptance:
  - The centre button shows `Icon::Pause` while playing and `Icon::Play` while paused, with a 300 ms crossfade (`crossfade_to`).
  - Rapid pause/play does not leave a half-faded state (A5).

### T-49ba-025 — Reader reading-mode icons
- Priority: P0
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_reader_default_24dp.xml, app/src/main/res/drawable/ic_reader_ltr_24dp.xml, app/src/main/res/drawable/ic_reader_rtl_24dp.xml, app/src/main/res/drawable/ic_reader_vertical_24dp.xml, app/src/main/res/drawable/ic_reader_continuous_vertical_24dp.xml, app/src/main/res/drawable/ic_reader_webtoon_24dp.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt
- Acceptance:
  - The reading-mode list shows one row per mode with `Icon::ReaderDefault`, `ReaderLtr`, `ReaderRtl`, `ReaderVertical`, `ReaderContinuousVertical`, `ReaderWebtoon`, matching the mapping in `ReadingMode.kt`.
  - LTR and RTL icons are never mirrored by GTK (edge case I3).

### T-49ba-026 — Download indicator icon
- Priority: P0
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_download_item_24dp.xml, app/src/main/java/eu/kanade/presentation/entries/anime/components/EpisodeDownloadIndicator.kt, app/src/main/java/eu/kanade/presentation/entries/manga/components/ChapterDownloadIndicator.kt
- Acceptance:
  - Downloaded chapters and episodes show `Icon::DownloadCircle` (`kb-download-circle-symbolic`) with tooltip "Downloaded"; not-downloaded rows show nothing.
  - The indicator is not clickable.

### T-49ba-027 — Download queue drag state and card foreground
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/color/draggable_card_foreground.xml, app/src/main/res/layout/download_header.xml, app/src/main/res/layout/download_item.xml
- Acceptance:
  - A dragged or selected download card has a background of `alpha(@theme_selected_bg_color, 0.08)` (`.kb-draggable.kb-dragged`, `.kb-draggable:selected`).
  - Idle cards have no overlay.
  - Keyboard reorder with Ctrl+Up / Ctrl+Down sets the dragged state while the row moves (Proposal).

### T-49ba-028 — Player double-tap seek triangles and subtitle typography icons
- Priority: P1
- Effort: M
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_play_seek_triangle.xml, app/src/main/res/drawable/ic_forward_10_24dp.xml, app/src/main/res/drawable/outline_brand_family_24.xml, app/src/main/res/drawable/sharp_shadow_24.xml, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/DoubleTapSeekTriangles.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt
- Acceptance:
  - Three `Icon::SeekTriangle` images, 16x20 px each, with opacity from `anim::seek_alpha` (test T10); backward seeks rotate 180 degrees.
  - The 900 ms cycle restarts while the seek is active and stops when the overlay is removed (edge case T-b).
  - Subtitle typography rows show `Icon::FontFamily` and `Icon::TextShadow`.
  - `Icon::Forward10` is available as an optional 10 s seek button (Proposal; the PiP action set is not ported).

### T-49ba-029 — Navigation-style and extension icons
- Priority: P1
- Effort: S
- Depends: T-49ba-010
- Sources: app/src/main/res/drawable/ic_updates_outline_24dp.xml, app/src/main/res/drawable/ic_glasses_24dp.xml, app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt, app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt, app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt, app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt, app/src/main/java/eu/kanade/tachiyomi/App.kt
- Acceptance:
  - `Icon::UpdatesTab` appears wherever `NavStyle.MOVE_UPDATES_TO_MORE` puts Updates.
  - Extension detail headers and the More row use `Icon::Glasses`.
  - The app's own notification small icon (`App.kt:123`) uses `Icon::Logo`.

### T-49ba-030 — Dialog and popup surfaces
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/drawable/material_popup_background.xml, app/src/main/res/values/styles.xml
- Acceptance:
  - Dialogs use the GTK theme's `window.dialog` styling; no hard-coded corner radius (the Material value is not in the repo, see p1-ui-parity section 9).
  - Popup menus use `gtk::PopoverMenu`; the overlay tint layer of the Android background is not reproduced.
  - Dialog and popover surfaces follow the active palette (`theme_base_color`).

### T-49ba-031 — Text appearance classes
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/styles.xml, app/src/main/res/layout/download_header.xml
- Acceptance:
  - `.kb-section-header` (14 px, weight 500, secondary text colour) is used on the download-header section title.
  - `.kb-preference-secondary` (12 px, secondary colour) is used on preference summaries.
  - `.kb-button-text` (14 px, weight 500) is used on button labels that carry the Android `Widget.Button` style.

### T-49ba-032 — Dividers
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/drawable/line_divider.xml, app/src/main/res/values/themes.xml
- Acceptance:
  - Lists use `gtk::Separator` or `.kb-divider` with a 1 px `alpha(@theme_fg_color, 0.12)` line (Android `divider_default` `#1F000000`, theme-relative).
  - Dividers are visible in both light and dark themes.

### T-49ba-033 — Switch and preference widget styling
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/styles.xml
- Acceptance:
  - Settings switches use the GTK `gtk::Switch` with no elevation tint (the `ThemeOverlay.Tachiyomi.Switch` elevation flag is dropped).
  - The preference category title and summary use the classes from T-49ba-031.
  - The `PreferenceThemeOverlay.Tachiyomi` items are dropped (GTK has no preference-overlay concept), documented in the code.

### T-49ba-034 — Layout margins 16 px and 24 px
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/dimens.xml, app/src/main/res/values-sw720dp/dimens.xml
- Acceptance:
  - `theme::screen_margin(window_width)` returns 16 for widths below 720 px and 24 for widths of 720 px and above.
  - The margin is applied on resize (`notify::default-width` and the window's `width` property).
  - A 720 px window shows 24 px margins; a 719 px window shows 16 px.

### T-49ba-035 — Player transition duration constant
- Priority: P1
- Effort: S
- Depends: none
- Sources: app/src/main/res/values/integers.xml, app/src/main/res/anim/player_fade_in.xml
- Acceptance:
  - `PLAYER_ANIM_MS: u32 = 200` is defined in `src/player.rs` and used for all player overlay show and hide transitions.
  - The 10 `player_*` transitions in `app/src/main/res/anim/` (outside this scope) are the reference; their durations are all 200 ms.

### T-49ba-036 — Reader pager widget name
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/res/values/ids.xml, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - The reader pager widget has the name `reader_pager` (`widget_name()` in GTK, used by `src/main.rs:58` to detect the immersive reader).
  - Immersive mode test: navigating to the reader hides the sidebar and tab bar.

### T-49ba-037 — Desktop actions and routing for the five launcher shortcuts
- Priority: P1
- Effort: S
- Depends: T-49ba-009
- Sources: app/shortcuts.xml, core/common/src/main/java/eu/kanade/tachiyomi/core/Constants.kt, app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt, app/src/main/res/drawable/sc_collections_bookmark_48dp.xml, app/src/main/res/drawable/sc_collections_baseline_48dp.xml, app/src/main/res/drawable/sc_new_releases_48dp.xml, app/src/main/res/drawable/sc_history_48dp.xml, app/src/main/res/drawable/sc_explore_48dp.xml
- Acceptance:
  - `data/io.github.anas1412.Kurobayomi.desktop` has five `[Desktop Action]` groups with unique names: `library`, `animelib`, `updates`, `history`, `browse`.
  - `kurobayomi --action=<name>` routes to the same screens as MainActivity (`Library`, `AnimeLib`, `Updates`, `History`, `Browse`).
  - Test T13 passes (`desktop-file-validate`).
  - The duplicate `show_recently_updated` id bug is not reproduced.

### T-49ba-038 — Shortcut icon names (replace the 48 dp adaptive bitmaps)
- Priority: P1
- Effort: S
- Depends: T-49ba-037
- Sources: app/src/main/res/drawable/sc_collections_bookmark_48dp.xml, app/src/main/res/drawable/sc_collections_baseline_48dp.xml, app/src/main/res/drawable/sc_new_releases_48dp.xml, app/src/main/res/drawable/sc_history_48dp.xml, app/src/main/res/drawable/sc_explore_48dp.xml, app/src/main/java/eu/kanade/tachiyomi/util/system/NotificationExtensions.kt
- Acceptance:
  - The `.desktop` actions use icon names: library `kb-library-symbolic`, anime library `kb-animelibrary-filled-symbolic`, updates `kb-updates-outline-symbolic`, history `document-open-recent-symbolic`, browse `system-search-symbolic`.
  - The `accent_blue` background of the adaptive icons and the notification `setColor(accent_blue)` (`NotificationExtensions.kt:70-71`) are dropped; no Kurobayomi code sets an accent colour.

### T-49ba-039 — Drop dead attributes and the unused NumberPicker
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/values/attrs.xml, app/src/main/java/eu/kanade/tachiyomi/widget/MinMaxNumberPicker.kt
- Acceptance:
  - No Kurobayomi code defines `MinMaxNumberPicker` or `MaterialSpinnerView`; numeric settings use `gtk::SpinButton` (area 47).
  - The `min`/`max` semantics (default 0 when unset) are recorded in the design notes for area 47.

### T-49ba-040 — Drop the widget corner radii
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/values/dimens.xml
- Acceptance:
  - No `appwidget_*` radius exists in Kurobayomi; the widget module is out of Linux scope (no home-screen widget on Linux).
  - The duplicate definitions in `presentation-widget` are not ported.

### T-49ba-041 — Drop unused drawables (14 files)
- Priority: P3
- Effort: S
- Depends: none
- Sources: app/src/main/res/drawable/ic_arrow_back_24dp.xml, app/src/main/res/drawable/ic_baseline_collections_24.xml, app/src/main/res/drawable/ic_baseline_input_24.xml, app/src/main/res/drawable/ic_baseline_open_in_new_24.xml, app/src/main/res/drawable/ic_brightness_negative_20dp.xml, app/src/main/res/drawable/ic_chrome_player_mode_24dp.xml, app/src/main/res/drawable/ic_circle_200dp.xml, app/src/main/res/drawable/ic_circle_right_200dp.xml, app/src/main/res/drawable/ic_collections_baseline_state.xml, app/src/main/res/drawable/ic_launcher_background.xml, app/src/main/res/drawable/ic_pause_circle_filled_24.xml, app/src/main/res/drawable/ic_play_circle_filled_24.xml, app/src/main/res/drawable/ic_video_chapter_20dp.xml, app/src/main/res/drawable/ic_volume_off_24dp.xml
- Acceptance:
  - None of these files is exported; the zero-reference check (edge case I6) is repeated in the export script and passes.
  - Back navigation uses GTK's `go-previous-symbolic` (`Icon::GoBack`), not the dropped arrow.

### T-49ba-042 — Drop unused style items
- Priority: P3
- Effort: S
- Depends: T-49ba-031
- Sources: app/src/main/res/values/styles.xml, app/src/main/res/values/themes.xml
- Acceptance:
  - `Theme.Tachiyomi.ActionButton.Overflow` (overflow tint) and `Theme.AppCompat.Light.NoActionBar.FullScreen` (no reference outside `themes.xml`) are not ported.
  - The overflow icon colour follows the GTK foreground automatically.

### T-49ba-043 — Drop unused CottonCandy elevation token
- Priority: P2
- Effort: S
- Depends: T-49ba-004
- Sources: app/src/main/res/values/colors_cottoncandy.xml, app/src/main/res/values-night/colors_cottoncandy.xml
- Acceptance:
  - `cottoncandy_elevationOverlay` is not in `Palette` and not in the drift test expectations.
  - GTK has no elevation overlay; the drop is recorded in the palette module doc comment.

### T-49ba-044 — Resource validation scripts and tests
- Priority: P2
- Effort: M
- Depends: T-49ba-004, T-49ba-010
- Sources: app/src/main/res/values/colors_cottoncandy.xml, app/src/main/res/values/colors_mocha.xml, app/src/main/res/values/themes.xml
- Acceptance:
  - `tools/check_palette.py` (T6), `tools/check_svg.py` (T7) and `tools/check_themes.py` (T9) run with Python 3 stdlib and exit 0 on the repo.
  - Rust tests T1, T2, T3, T4, T5, T8, T10, T11, T12 exist and pass under `xvfb-run -a cargo test`.
  - Fixtures live under `tests/fixtures/android/` as listed in `p1-edge-cases-and-tests.md` section 5.

### T-49ba-045 — Stylesheet and CSS-provider failure handling
- Priority: P0
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/themes.xml
- Acceptance:
  - Each `CssProvider` connects `parsing-error` and logs the location to stderr; the app continues with the rest of the rules (edge case section 3).
  - Only the GTK default named colours (`@theme_bg_color`, `@theme_fg_color`, `@theme_base_color`, `@theme_selected_bg_color`, `@theme_selected_fg_color`) are referenced by `style.css` unless test T1 proves another name exists in the active theme.
  - If the GResource path is missing the app starts unstyled and logs one error line.

### T-49ba-046 — Honour the GTK animations setting
- Priority: P2
- Effort: S
- Depends: T-49ba-019
- Sources: app/src/main/res/drawable/anim_caret_down.xml, app/src/main/res/drawable/anim_play_to_pause.xml
- Acceptance:
  - When `gtk-enable-animations` is false, every transition duration in `src/anim.rs` and `style.css` for these icons is 0 ms and the icon state still changes.
  - The setting is read from `gtk::Settings::default()` and reacts to changes at runtime.

### T-49ba-047 — Drop Android window attributes
- Priority: P3
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/themes.xml
- Acceptance:
  - Not ported: `android:forceDarkAllowed`, `android:statusBarColor`, `android:navigationBarColor`, `android:navigationBarDividerColor`, `android:enforceNavigationBarContrast`, `android:enforceStatusBarContrast`, `windowActionModeOverlay`, `elevationOverlayColor`, `android:windowLightStatusBar`.
  - Each drop has a one-line reason in the module doc comment (GTK has no system bars, no forced dark, no elevation tint, no action-mode overlay).

### T-49ba-048 — Ripple and highlight mapping
- Priority: P2
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/themes.xml
- Acceptance:
  - `colorControlHighlight` (= `colorSurfaceVariant`) is represented by `.kb-highlight:hover` (alpha 0.1 of the foreground colour, labelled PROPOSAL in the stylesheet).
  - Hover and focus states of list rows and buttons use the class; keyboard focus is visible (no focus is hidden).

### T-49ba-049 — Modal scrim (backgroundDimAmount 0.32)
- Priority: P2
- Effort: S
- Depends: T-49ba-030
- Sources: app/src/main/res/values/themes.xml
- Acceptance:
  - While a modal dialog or sheet is open, the window overlay has class `.kb-scrim` (`rgba(0,0,0,0.32)`).
  - The scrim is removed when the modal closes.

### T-49ba-050 — Text alignment and direction (RTL)
- Priority: P1
- Effort: S
- Depends: T-49ba-001
- Sources: app/src/main/res/values/themes.xml
- Acceptance:
  - `android:textAlignment=gravity` and `android:textDirection=locale` are satisfied by GTK's default (labels follow the locale direction); no label in Kurobayomi forces LTR.
  - With an Arabic locale the library list and entry titles align to the right; reader LTR/RTL icons keep their semantics (edge case I3).
