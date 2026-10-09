# 43-reader-core (part 2 of 2) — Implementation tasks

Task IDs: `T-43b-NNN`. Behavior references: `p2-behavior.md` §n. Data: `p2-data-and-config.md` §n. Design: `p2-rust-design.md` §n. Tests: `p2-edge-cases-and-tests.md` E-nn. Decisions D-n are in `p2-edge-cases-and-tests.md` §6.

Priority: P0 needed for daily reading. P1 parity. P2 long tail. P3 Android-only with a replacement or drop decision.

Repo-relative source root: `/home/blackbox/aniyomi-src`.

### T-43b-001 — Preference store with typed Pref handles, kv table and change subscriptions
- Priority: P0
- Effort: M
- Depends: area:02-preferences-system
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreference.kt, core/common/src/main/java/tachiyomi/core/common/preference/AndroidPreferenceStore.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt
- Acceptance:
  - `Pref<T>` provides get, set, delete, is_set, default_value and subscribe with the behavior of `AndroidPreference.changes()` (current value delivered on subscribe, then each distinct change).
  - A stored value of the wrong type returns the default and deletes the row (E-19c).
  - Unknown enum names return the default (E-19).
  - Reads never touch SQLite after startup; writes persist asynchronously and update the in-memory cache synchronously.
  - Table `kv(key TEXT PRIMARY KEY NOT NULL, value TEXT NOT NULL) STRICT` is created idempotently.

### T-43b-002 — Boolean preference catalog (36 keys) with exact keys and defaults
- Priority: P0
- Effort: S
- Depends: T-43b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - Each key in `p2-data-and-config.md` §2.1 exists with its exact string (for example `pref_enable_transitions_key` default true, `skip_filtered` default true, `reader_navigation_overlay_new_user` default true) and default.
  - A unit test asserts the 36 key strings and defaults (E-19e: keys unique).

### T-43b-003 — Integer preference catalog (14 keys) with defaults and units
- Priority: P0
- Effort: S
- Depends: T-43b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `pref_reader_flash_duration` default 100 (ms), `pref_reader_flash_interval` default 1, `pref_double_tap_anim_speed` default 500, `pref_default_reading_mode_key` default 2, `pref_default_orientation_type_key` default 8, `pref_image_scale_type_key` default 1, `pref_zoom_start_key` default 1, `pref_reader_theme_key` default 1, `webtoon_side_padding` default 0, `custom_brightness_value` default 0, `color_filter_value` default 0, `color_filter_mode` default 0, `reader_navigation_mode_pager` default 0, `reader_navigation_mode_webtoon` default 0.

### T-43b-004 — Enum-valued preference keys with name encoding
- Priority: P0
- Effort: S
- Depends: T-43b-001
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, core/common/src/main/java/tachiyomi/core/common/preference/PreferenceStore.kt
- Acceptance:
  - `pref_reader_flash_mode` (default BLACK), `reader_hide_threshold` (default LOW), `reader_tapping_inverted` and `reader_tapping_inverted_webtoon` (default NONE) store the Kotlin enum NAME exactly (for example `WHITE_BLACK`).
  - Round-trip test for every value (E-19b).

### T-43b-005 — Range rules for numeric preferences
- Priority: P0
- Effort: S
- Depends: T-43b-003
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ColorFilterPage.kt
- Acceptance:
  - Flash duration: stored value is a multiple of 100 in 100..1500; flash interval 1..10; webtoon side padding 0..25 (clamped on read, D-7); brightness -75..100 (clamped, E-36); navigation mode 0..5; scale type 1..6; zoom start 1..4; theme 0..3.
  - Out-of-range stored values never panic and never change the stored row silently except through the clamp-on-read rule.

### T-43b-006 — FlashColor, TappingInvertMode and ReaderHideThreshold enums
- Priority: P0
- Effort: S
- Depends: T-43b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - TappingInvertMode: NONE (h false, v false), HORIZONTAL (h true), VERTICAL (v true), BOTH (both true), with label keys `tapping_inverted_none`, `tapping_inverted_horizontal`, `tapping_inverted_vertical`, `tapping_inverted_both`.
  - ReaderHideThreshold: HIGHEST 5, HIGH 13, LOW 31 (default), LOWEST 47 px.
  - FlashColor: BLACK, WHITE, WHITE_BLACK.

### T-43b-007 — Option lists: TapZones, ImageScaleType, ZoomStart, ColorFilterMode
- Priority: P0
- Effort: S
- Depends: T-43b-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt
- Acceptance:
  - TapZones indices 0..5 map to labels Default, L shaped, Kindle-ish, Edge, Right and Left, Disabled.
  - ImageScaleType stored 1..6 map to Fit screen, Stretch (crop-to-fill, D-1), Fit width, Fit height, Original size, Smart fit.
  - ZoomStart stored 1..4 map to Automatic, Left, Right, Center.
  - ColorFilterMode indices 0..5 always available on Linux (no API-28 gate): Default, Multiply, Screen, Overlay, Dodge / Lighten, Burn / Darken, with blend mapping SrcOver, Multiply, Screen, Overlay, Lighten, Darken.

### T-43b-008 — Cross-area reader keys wired to their consumers
- Priority: P1
- Effort: S
- Depends: T-43b-002, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderViewModel.kt
- Acceptance:
  - `skip_read` (default false), `skip_filtered` (default true), `skip_dupe` (default false), `pref_preserve_reading_position` (default false) and `always_show_chapter_transition` (default true) are read by the reader chapter navigation and the details screen with the same meaning as Kotlin (`MangaScreenModel.kt:654` uses `skipFiltered` to choose filtered or all chapters).
  - Changing a value takes effect on the next chapter transition.

### T-43b-009 — ReadingMode enum with flag values, direction and viewer kind
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt
- Acceptance:
  - Values DEFAULT 0, LEFT_TO_RIGHT 1, RIGHT_TO_LEFT 2, VERTICAL 3, WEBTOON 4, CONTINUOUS_VERTICAL 5; MASK 0x7.
  - Direction: Horizontal for 1 and 2, Vertical for 3, 4, 5; none for DEFAULT. Kind: Pager for 1..3, Webtoon for 4 and 5; none for DEFAULT.
  - Unit tests E-20.

### T-43b-010 — ReadingMode resolution and viewer factory semantics
- Priority: P0
- Effort: S
- Depends: T-43b-009
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - `from_stored` maps None, 0, 6, 7 and any unknown value to Default.
  - `resolve(global)` never returns Default (E-21, D-4).
  - The factory rejects Default with an error, mirroring the Kotlin IllegalStateException; CONTINUOUS_VERTICAL builds the webtoon viewer with `is_continuous = false`.
  - `is_pager` is false for Default and for webtoon modes.

### T-43b-011 — Per-series reading mode and orientation in manga viewer flags
- Priority: P0
- Effort: S
- Depends: area:05-data-repos-entries-items, T-43b-009
- Sources: app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt, app/src/main/java/eu/kanade/domain/entries/manga/interactor/SetMangaViewerFlags.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt
- Acceptance:
  - `manga_reading_mode(flags) = flags & 0x7`; `with_reading_mode` replaces only bits 0..2 and keeps bits 3..5 (E-20b).
  - The `mangas.viewer` column value round-trips unchanged for flags with orientation bits set.

### T-43b-012 — Long-press tracker state machine (pure Rust)
- Priority: P0
- Effort: M
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt
- Acceptance:
  - Arms on press unless within the double-tap window of the previous release (E-08, E-08b); disarms on move beyond slop, release, cancel and second pointer (E-09, E-10, E-11).
  - `fire()` returns false for stale timers (E-10).
  - Constants come from GTK settings (`gtk-long-press-time`, `gtk-double-click-time`) with fixed 8 px slop (behavior §8.1 proposal).
  - Module has no GTK imports.

### T-43b-013 — Long-press wiring to the viewer and reader_long_tap preference
- Priority: P0
- Effort: S
- Depends: T-43b-012, T-43b-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/Pager.kt
- Acceptance:
  - A confirmed long press calls the viewer long-tap handler with the press position only when `reader_long_tap` is true (default true).
  - No second long-press path is created (built-in onLongPress stays unhandled).
  - Long press is ignored while a zoom animation runs (D-10).

### T-43b-014 — PageView widget: decode, display, load state, GTK snapshot
- Priority: P0
- Effort: L
- Depends: T-43b-015, T-43b-059
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Loads a page from bytes via `spawn_blocking` decode and shows it as a `gdk::Texture` in a `gtk::Widget` snapshot, fitted by the selected scale type.
  - States Empty, Loading, Ready, Error are observable; Loading hides the page and shows the loading indicator; Error emits `image-load-error`.
  - No crossfade on load (Coil `crossfade(false)` parity).
  - Pages at 1000x2000 and 2000x1000 display at the fitted scale in an 800x600 view (E-01).

### T-43b-015 — Scale type parity with SSIV minScale rules (stored 1..6)
- Priority: P0
- Effort: M
- Depends: T-43b-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerPageHolder.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - `min_scale` matches the table in p2-behavior §3.5 and topic image view §3 for FitScreen, Stretch (max of ratios), FitWidth, FitHeight, OriginalSize and SmartFit (E-01, E-01b).
  - Webtoon always uses FitWidth.
  - Stored values 0, 99 fall back to FitScreen (D-2, E-18); value 7 (CUSTOM) falls back to FitScreen.

### T-43b-016 — Zoom limits and double-tap zoom
- Priority: P0
- Effort: M
- Depends: T-43b-014, T-43b-015
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - User zoom is clamped to [min, 5 x min] for non-animated pages (E-02).
  - Double tap toggles between min and 2 x min, focused on the tap point, with duration = `pref_double_tap_anim_speed` (default 500) scaled by T-43b-026 (E-14).
  - Animated pages use absolute levels 1, 2, 5 with double tap toggling 1 and 2 (E-41).
  - Pinch uses GestureZoom; ctrl+wheel zooms by 1.1 per notch (desktop equivalent).

### T-43b-017 — Zoom start position (Automatic, Left, Right, Center)
- Priority: P0
- Effort: S
- Depends: T-43b-014, T-43b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Automatic maps to Left for L2R, Right for R2L, Center otherwise; 2 Left, 3 Right, any other value Center (behavior §3.6).
  - At ready, Left anchors center at (0,0), Right at (src_w, 0), Center at the middle (E-03).

### T-43b-018 — Landscape auto-zoom for wide pages
- Priority: P1
- Effort: M
- Depends: T-43b-016, T-43b-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Runs only when all five preconditions hold: `landscape_zoom` true, scale type FitScreen, wide page, zoom equal to min (E-04).
  - Starts 500 ms after the trigger, animates 500 ms EASE_IN_OUT_QUAD to fit height, anchored per zoom start and direction.
  - Cancelled by any user zoom or pan gesture (D-3, E-04b).

### T-43b-019 — Pan wide images (navigate_pan), can_pan and pan steps
- Priority: P1
- Effort: M
- Depends: T-43b-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - `can_pan_left` and `can_pan_right` are true only when remaining pan exceeds 1 px (E-13).
  - pan_left and pan_right move the center by one view width in source px, 250 ms EASE_OUT_QUAD, interruptible; no-op when not ready (no panic).
  - `navigate_pan` default true; the viewer uses these functions for side-zone taps (area 44 consumes).

### T-43b-020 — Crop borders for paged and webtoon pages
- Priority: P1
- Effort: M
- Depends: T-43b-014, area:34-images-covers-cache
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt
- Acceptance:
  - `crop_borders` (default false) and `crop_borders_webtoon` (default false) crop uniform non-filled borders at decode time; tolerance is a documented constant.
  - Cropped and uncropped page sizes are both tested with a fixture that has a 20 px white border.
  - Pages with no uniform border are unchanged.

### T-43b-021 — Tiling for large pages with hardware threshold parity
- Priority: P1
- Effort: L
- Depends: T-43b-014, T-43b-059
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, core/common/src/main/java/eu/kanade/tachiyomi/util/system/GLUtil.kt
- Acceptance:
  - `needs_tiling(w, h, 2048)` is true only when max(w,h) > 2048 (E-06).
  - Tiles are created from the decoded buffer and uploaded lazily for the visible region at the current zoom.
  - Pages up to 16384 px per side upload without downscale; above that, downscale per D-9 (E-07 covers decode errors).
  - Peak texture memory for the window (current ±2 pages) is bounded by a documented budget.

### T-43b-022 — Animated pages (GIF, APNG, animated WebP)
- Priority: P2
- Effort: M
- Depends: T-43b-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Frames decode with their delays; playback loops; the page shows frame changes on the timer.
  - Webtoon animated pages have no zoom (E-42).

### T-43b-023 — Webtoon page variant and alwaysDecodeLongStripWithSSIV
- Priority: P1
- Effort: M
- Depends: T-43b-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/domain/base/BasePreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonPageHolder.kt
- Acceptance:
  - Webtoon pages ignore touch (gesture controllers off) and use FitWidth.
  - `pref_always_decode_long_strip_with_ssiv` (default false) selects the full-decode path for long strips; otherwise the webtoon decode path with the same options as Kotlin.
  - Non-bitmap decode results produce an error state, not a panic (E-07).

### T-43b-024 — Load callbacks, recycle, page background and re-apply on settings change
- Priority: P0
- Effort: S
- Depends: T-43b-014
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Signals `image-loaded`, `image-load-error`, `scale-changed`, `view-clicked` fire at the points listed in behavior §9.1.
  - `recycle()` drops the texture and hides the page; a later `set_image` works (E-15).
  - Page background applies only after a successful load (`onImageLoaded`).
  - Changing scale type or zoom start re-applies to the visible page without reloading the texture (D-11).

### T-43b-025 — Page background color for the automatic reader theme
- Priority: P1
- Effort: S
- Depends: T-43b-024, T-43b-042
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt
- Acceptance:
  - When reader theme is 3 (automatic), the page background becomes the viewer's computed color after load; otherwise the theme color (0 white, 1 black, 2 gray) shows.

### T-43b-026 — System animation factor for zoom durations
- Priority: P1
- Effort: S
- Depends: T-43b-016
- Sources: app/src/main/java/eu/kanade/tachiyomi/util/system/AnimationExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt
- Acceptance:
  - Zoom durations scale by the GTK animation setting (`gtk-enable-animations`): enabled → unchanged; disabled → 1 ms minimum (E-16).
  - Landscape zoom (500 ms) and pan (250 ms) are not scaled (behavior §9.8).

### T-43b-027 — calculate_chapter_gap with upstream tests ported
- Priority: P0
- Effort: S
- Depends: area:07-domain-entries-items
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/MissingChapters.kt, domain/src/main/java/tachiyomi/domain/items/chapter/service/MissingChapters.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - Ported cases from MissingItemsTest pass: (10,9)=0, (10,8)=1, (10,8.5)=1, (10,1.1)=8; null, unrecognized and negative inputs = 0 (E-28).
  - (5.0, 5.5) returns -1 and callers hide values not greater than 0 (E-29).

### T-43b-028 — Overlay button without pager gesture coupling
- Priority: P2
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderButton.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt
- Acceptance:
  - A retry button inside the page overlay receives clicks and keyboard activation; page-turn gestures do not fire on the button (E-28b).
  - No pager gesture toggle code exists in Rust.

### T-43b-029 — Loading indicator with spinner and clamped percentage
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderProgressIndicator.kt
- Acceptance:
  - show and hide toggle visibility; spinner keeps spinning while visible.
  - set_progress clamps to 0..=100 (E-31) and draws a determinate ring only after set_progress is called.

### T-43b-030 — Reader settings model state
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderSettingsScreenModel.kt
- Acceptance:
  - The model exposes the current viewer kind and the current manga, updated on change, and callbacks for reading mode and orientation.
  - Equality uses manga id and flags, not object identity (see edge cases §3.2).
  - `has_display_cutout` is false on Linux.

### T-43b-031 — Reader settings sheet bound to ReaderPrefs (General and Reading mode pages)
- Priority: P0
- Effort: M
- Depends: T-43b-001, T-43b-030, area:47-settings-screens-general
- Sources: app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReaderSettingsDialog.kt, app/src/main/java/eu/kanade/presentation/reader/ReadingModeSelectDialog.kt
- Acceptance:
  - Every General row and every Pager and Webtoon row listed in p2-ui-parity §5.1 and §5.2 reads its default, writes its key, and enables or disables as listed.
  - The spec item type in `src/spec.rs` carries a `pref` key; `src/ui.rs item_row` binds switch, list, slider and chip rows to `ReaderPrefs`.
  - Reading mode chips update the manga flags (T-43b-011).

### T-43b-032 — Chapter transition downloaded flags with off-thread download check
- Priority: P0
- Effort: M
- Depends: area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderTransitionView.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerTransitionHolder.kt
- Acceptance:
  - `bind(None)` clears the view; local manga sets goingToChapterDownloaded true without a download lookup; a non-local manga runs the lookup with skip-cache semantics (behavior §11.2).
  - The lookup runs off the GTK thread; the view updates when it returns; a download completion for the target chapter updates the flag (Proposal improvement).

### T-43b-033 — Chapter transition rendering host
- Priority: P0
- Effort: S
- Depends: T-43b-032
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderTransitionView.kt, app/src/main/java/eu/kanade/presentation/reader/ChapterTransition.kt
- Acceptance:
  - Renders previous, current, finished and next labels, the gap line only when gap > 0, and the downloaded state, with secondary text style from the GTK theme.
  - Layout is WRAP_CONTENT equivalent (natural size).

### T-43b-034 — Viewer trait (contract for pager, webtoon and future viewers)
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/Viewer.kt
- Acceptance:
  - Trait methods: widget, destroy (default no-op), set_chapters, move_to_page, handle_key, handle_scroll, mirroring the six Kotlin methods.
  - A test viewer implementation receives a key event and returns handled.

### T-43b-035 — ViewerConfig with live preference bindings
- Priority: P0
- Effort: M
- Depends: T-43b-001, T-43b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt
- Acceptance:
  - Each bound field updates when its preference changes; each change callback runs once per distinct value (E-23).
  - `use_page_transitions` starts false and becomes the stored value after the first emission (E-22).
  - Dropping the viewer drops all subscriptions.

### T-43b-036 — One-shot new-user navigation overlay and on-start overlay
- Priority: P1
- Effort: S
- Depends: T-43b-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderNavigationOverlayView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerViewer.kt
- Acceptance:
  - On the first viewer construction, `reader_navigation_overlay_new_user` (default true) forces the overlay once and is written false (E-24).
  - `reader_navigation_overlay_on_start` (default false) shows the overlay on every start.

### T-43b-037 — NavRegion model, colors, labels and half-open hit geometry
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt
- Acceptance:
  - Five regions MENU, PREV, NEXT, LEFT, RIGHT with labels `action_menu`, `nav_zone_prev`, `nav_zone_next`, `nav_zone_left`, `nav_zone_right` and ARGB colors from the topic file §4.
  - `RectN::contains` is half-open and false for empty rects (E-05).

### T-43b-038 — Tap hit test with Menu fallback and DisabledNavigation behavior
- Priority: P0
- Effort: S
- Depends: T-43b-037
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/navigation/DisabledNavigation.kt
- Acceptance:
  - First region in list order wins; no region matches → Menu (E-27, E-26).
  - `constantMenuRegion` is not ported (dead logic).

### T-43b-039 — Tap-zone inversion for paged and webtoon viewers
- Priority: P0
- Effort: S
- Depends: T-43b-038, T-43b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/util/lang/RectFExtensions.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt
- Acceptance:
  - Horizontal, vertical and both inversions use the exact rect transforms in behavior §7.3 (E-25).
  - Changing `reader_tapping_inverted` (paged) or `reader_tapping_inverted_webtoon` (webtoon) changes the hit test without restarting the viewer.

### T-43b-040 — Navigation overlay drawing data (colors, labels, inverted rects)
- Priority: P1
- Effort: S
- Depends: T-43b-037, T-43b-039
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderNavigationOverlayView.kt
- Acceptance:
  - Overlay draws each region's rect (inverted) in its color with its label; the overlay can be dismissed by any tap.

### T-43b-041 — Volume-key page turns (XF86 keys) with inversion
- Priority: P1
- Effort: S
- Depends: T-43b-035
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - With `reader_volume_keys` on: XF86AudioRaiseVolume → previous page, XF86AudioLowerVolume → next page; `reader_volume_keys_inverted` swaps them (E-39).
  - Documented caveat that some desktop shells swallow media keys; PageUp/PageDown always work.

### T-43b-042 — Window options: fullscreen, keep screen on, page number, reader theme background
- Priority: P0
- Effort: S
- Depends: T-43b-002, T-43b-004
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - `fullscreen` (default true) toggles window fullscreen while the reader is open.
  - `pref_keep_screen_on_key` (default true) holds an idle inhibit only while the reader is open and the pref is on; released on close (E-38).
  - `pref_show_page_number_key` (default true) shows page number; `pref_reader_theme_key` sets background 1 black by default.

### T-43b-043 — E-ink flash host (counter, colors, duration)
- Priority: P2
- Effort: M
- Depends: T-43b-002, T-43b-003, T-43b-004
- Sources: app/src/main/java/eu/kanade/presentation/reader/DisplayRefreshHost.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Flash occurs on page change when `pref_reader_flash` is on and the counter rule matches (E-29c, E-30).
  - BLACK: black for the full duration; WHITE: white; WHITE_BLACK: white then black at half each (E-30b).
  - Overlay is pointer transparent; no restart while active.

### T-43b-044 — Flash settings controls (switch, duration, interval, color list)
- Priority: P2
- Effort: S
- Depends: T-43b-043, T-43b-031
- Sources: app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt, app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt
- Acceptance:
  - Duration slider 1..15 stores value × 100; subtitle "%1$s ms"; interval slider 1..10 with plural subtitle; color list Black, White, White and Black; all controls disabled when the switch is off.

### T-43b-045 — Page transitions toggle and double-tap animation speed
- Priority: P1
- Effort: S
- Depends: T-43b-035, T-43b-002
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt
- Acceptance:
  - `pref_enable_transitions_key` (default true) enables page transition animations; `pref_double_tap_anim_speed` (default 500 ms) feeds `double_tap_anim_duration`.

### T-43b-046 — Webtoon hide-menu scroll threshold
- Priority: P1
- Effort: S
- Depends: T-43b-006
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/webtoon/WebtoonViewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - The menu hides when `|dy| > threshold` (strict) while the menu is visible; thresholds 5, 13, 31 (default), 47 (E-33).
  - Changing the setting takes effect when the webtoon viewer is next created (matches Kotlin read-once).

### T-43b-047 — Webtoon zoom options and side padding
- Priority: P1
- Effort: M
- Depends: T-43b-003, T-43b-005
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt
- Acceptance:
  - `pref_enable_double_tap_zoom_webtoon` (default true) enables double-tap zoom in webtoon; `webtoon_disable_zoom_out` (default false) prevents zooming below the fit scale.
  - `webtoon_side_padding` 0..25 renders as percent (value / 100) and pads both sides; out-of-range values are clamped on read (D-7, E-37).

### T-43b-048 — Dual-page split, invert and rotate-to-fit (paged and webtoon)
- Priority: P1
- Effort: L
- Depends: T-43b-014, T-43b-031
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerConfig.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/pager/PagerConfig.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ReadingModePage.kt
- Acceptance:
  - Eight keys (split, invert, rotate, rotate-invert for paged and webtoon) default false; turning rotate on turns split off in the settings UI.
  - Split divides a wide page into two halves (invert swaps placement); rotate-to-fit rotates a wide page; flip swaps rotation direction.
  - Split and rotate decisions are unit tested with a 2000x1000 fixture.

### T-43b-049 — Color filter: brightness overlay, color filter, blend modes, grayscale, invert
- Priority: P1
- Effort: M
- Depends: T-43b-003, T-43b-007
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/reader/settings/ColorFilterPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Custom brightness on with value -75..-1 draws a semi-transparent black overlay at the top of the screen; 1..100 sets no overlay on Linux and is recorded as a documented no-op; 0 hides the overlay.
  - Color filter draws the stored ARGB with the selected blend mode (SrcOver, Multiply, Screen, Overlay, Lighten, Darken) via `gsk::BlendMode`.
  - Grayscale and invert use GSK color matrix nodes.

### T-43b-050 — Drop: cutout (cutout_short) on Linux
- Priority: P3
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/presentation/reader/settings/GeneralSettingsPage.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - The "Show content in cutout area" item is never shown on Linux; the key stays in the store for backup compatibility and has no effect.
  - Recorded as dropped in the settings sheet help text.

### T-43b-051 — Drop: Android hardware bitmap, BitmapRegionDecoder and SDK gating
- Priority: P3
- Effort: S
- Depends: T-43b-021
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt
- Acceptance:
  - No Rust code path depends on hardware-bitmap config, BitmapRegionDecoder or `Build.VERSION` gates; the color filter list is always complete (T-43b-007).
  - The `pref_hardware_bitmap_threshold` value is still read and used as the tile threshold (T-43b-021).

### T-43b-052 — Replacement record for Android view plumbing in scope
- Priority: P3
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/GestureDetectorWithLongTap.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderProgressIndicator.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderTransitionView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderButton.kt
- Acceptance:
  - Each Android API has a named Linux replacement in p2-rust-design §12 (AbstractComposeView, Handler, postDelayed, ViewConfiguration, MaterialButton, Canvas.drawRect, FLAG_KEEP_SCREEN_ON).
  - No Rust module imports an Android-specific concept.

### T-43b-053 — Default orientation pass-through and orientation flags
- Priority: P1
- Effort: S
- Depends: T-43b-003, area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt
- Acceptance:
  - `pref_default_orientation_type_key` default 8 (FREE) is stored and read; orientation bits (0x38) are preserved when the reading mode changes (E-20b).
  - On Linux the orientation choice is kept as data; no display rotation is applied (P3 note).

### T-43b-054 — Unit test suite for the scope's pure logic
- Priority: P0
- Effort: M
- Depends: T-43b-012, T-43b-015, T-43b-016, T-43b-027, T-43b-037, T-43b-038
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ViewerNavigation.kt, domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt
- Acceptance:
  - `cargo test reader::` runs E-01..E-05, E-08..E-11, E-14, E-16..E-21b, E-22..E-30b, E-31, E-33, E-36, E-37, E-19..E-19e, E-28, E-29.
  - Fixtures per p2-edge-cases-and-tests.md §7 are generated by the test helper (no binary fixtures committed).

### T-43b-055 — Backup and restore of reader preferences and viewer flags
- Priority: P1
- Effort: S
- Depends: T-43b-001, area:21-backup-format-and-create, area:22-backup-restore
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/domain/entries/manga/model/Manga.kt
- Acceptance:
  - Every key in p2-data-and-config.md §2 round-trips through backup with the same name and encoding.
  - Manga `viewer` values (reading mode and orientation bits) round-trip unchanged.

### T-43b-056 — Create folder per manga preference (downloads)
- Priority: P1
- Effort: S
- Depends: T-43b-002, area:23-downloads-manga
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt
- Acceptance:
  - `create_folder_per_manga` (default false) is read by the download path (area 23) and set from the reader settings screen.

### T-43b-057 — Localization keys for scope strings
- Priority: P1
- Effort: S
- Depends: area:51-i18n
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt, i18n/src/commonMain/moko-resources/base/strings.xml
- Acceptance:
  - Every key in p2-behavior §15 resolves in the Rust i18n table; the plural `pref_pages` has all locale forms that the English catalog uses.
  - A test asserts all keys exist in the base catalog.

### T-43b-058 — Keyboard page turns and zoom shortcuts in the reader
- Priority: P1
- Effort: S
- Depends: T-43b-034
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/Viewer.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Left, Right, PageUp, PageDown, Space, Shift+Space, Home, End turn pages (direction follows reading mode); plus, minus, 0 and Ctrl+0 zoom (Proposal, additive).
  - Keys are ignored while a dialog has focus.

### T-43b-059 — Decode session guard and stale-result discard
- Priority: P0
- Effort: S
- Depends: area:43-reader-core
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/reader/viewer/ReaderPageImageView.kt, app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt
- Acceptance:
  - Each PageView has a generation counter; a decode result whose generation or reader session is stale is dropped without touching widgets (E-40).
  - A new `set_image` cancels the visual of the previous load immediately.
