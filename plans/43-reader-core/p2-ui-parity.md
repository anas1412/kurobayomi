# 43-reader-core (part 2 of 2) — UI parity

Scope renders UI in four ways: (a) the page image view (`ReaderPageImageView.kt`), (b) the loading spinner (`ReaderProgressIndicator.kt`), (c) the chapter transition panel host (`ReaderTransitionView.kt`) and the retry button (`ReaderButton.kt`), (d) the e-ink flash overlay (`DisplayRefreshHost`, driven by this scope's prefs), plus (e) the settings controls bound to `ReaderPreferences` (presentation files in part 1, listed here because every control's value comes from this scope). The tap-zone overlay is drawn by part 2 navigation code (`ReaderNavigationOverlayView`, area 44) and only its data model (`ViewerNavigation`) is in scope; its geometry is summarized in `p2-topic-tap-navigation.md`.

Mockup reference: `/home/blackbox/kurobayomi/src/player.rs:49-149` (static reader), `/home/blackbox/kurobayomi/spec/reader_player.json` (spec of the reader screen and its settings sheet).

Contents
1. Page image view (per-page states and gestures)
2. Loading indicator
3. Chapter transition panel and retry button
4. E-ink flash overlay
5. Settings controls (every control, state, dependency)
6. Responsive behavior
7. Keyboard and mouse equivalents
8. Gap list versus the mockup

---

## 1. Page image view

### 1.1 Visible states of one page
| State | Kotlin trigger | What the user sees | Rust (proposal) |
|---|---|---|---|
| Empty (not yet set) | view created, no drawable | blank page area, page background color not yet applied (`pageBackground` applied on load) | `LoadState::Empty`: background color of the reader theme |
| Loading | `setImage` called, Coil/SSIV decoding | the viewer's loading spinner (see §2) over a blank page; the view itself is `isVisible` false until the image is ready (SSIV path sets visible on `onReady`/`setImage`) | `LoadState::Loading`: `LoadingIndicator` shown, page hidden |
| Ready, fit | `onReady` → `setupZoom` | image fitted per Scale type (default Fit screen), page background color applied (`onImageLoaded`) | `LoadState::Ready`, `zoom = min_scale` |
| Ready, zoom start applied | `setupZoom` zoom-start branch | Left: top-left corner of the image is the anchor; Right: top-right; Center: center. Visible only when the image is zoomed beyond fit (not at fit) | same anchors, `center` set |
| Landscape auto-zoom | `landscapeZoom(...)` 500 ms later | wide page animates to fit height, anchored to the left or right edge for the reading direction | animation 500 ms, EASE_IN_OUT_QUAD |
| Zoomed in (user) | pinch, double tap, ctrl+wheel (desktop) | image moves within bounds; pan with drag | `GestureZoom`/`GestureDrag` |
| Error | `onImageLoadError` (SSIV error, Coil error, decode failure) | error UI is drawn by the viewer (area 44 `PagerTransitionHolder`/`WebtoonPageHolder`), with a retry `ReaderButton`; page view `isVisible` remains as-is | `LoadState::Error`, emits `image-load-error`, viewer shows retry button |
| Recycled | `recycle()` | page hidden, texture released | `recycle()`: texture dropped, `set_visible(false)` |
| Animated (GIF/APNG/WebP) | `Animatable` drawable | frames play; zoom via PhotoView levels 1, 2, 5 (double tap toggles 1/2); webtoon: no zoom | `AnimatedFrames` playback in `PageView`; levels absolute |

Visual details that must match:
- No crossfade on load (`crossfade(false)` in Coil requests, `ReaderPageImageView.kt:342, 415`). Pages appear instantly when ready.
- Background: when the reader theme is "Automatic" (value 3), the page background is computed by the viewer (`pageBackground`; part 2 viewer). Otherwise the theme color (black 1, white 0, gray 2) shows behind the page.
- `minimumDpi = 1` on load means very small images are scaled up to fit the view.
- Page clipping: `MATCH_PARENT` both ways, no padding; the page fills the reader area (menus float above it).

### 1.2 Gestures on a page (non-webtoon)
| Input | Result | Notes |
|---|---|---|
| Single tap | `onViewClicked` → viewer decides by tap zone (menu / prev / next) | tap zones and their rectangles are in area 44 |
| Double tap, SSIV (non-animated) | zoom to 2x fit centered on the tap; second double tap returns to fit | duration = `doubleTapAnimSpeed` (default 500 ms), scaled by system animator setting |
| Double tap, PhotoView (animated) | levels 1 → 2 → 1 (absolute scale) | centered on tap |
| Pinch | zoom between fit and 5x fit (SSIV) or 1..5 (PhotoView) | clamped; no overshoot in Kotlin (SSIV `limitedScale`) |
| Drag while zoomed | pan inside bounds | PAN_LIMIT_INSIDE |
| Drag at fit | no pan; page swipe handled by the pager (area 44) | `GestureDrag` passes through at min zoom |
| Long press | `onLongTapConfirmed` → viewer's long-tap action (area 44; setting `reader_long_tap` default true) | 500 ms on GTK default |
| Pan buttons (navigate_pan) | when the page is zoomed and the tap lands in the side zone, the viewer pans one screen width instead of turning the page (`canPanLeft/Right`, `panLeft/Right`, 250 ms) | exact trigger logic lives in area 44; this scope provides the functions |

### 1.3 Gestures on a webtoon page
- `WebtoonSubsamplingImageView` ignores all touch events (its own KDoc); the webtoon viewer handles scroll and double tap. The Rust `PageView` with `is_webtoon` disables its gesture controllers. Double tap zoom in webtoon is handled by the webtoon viewer (`webtoon_double_tap_zoom_enabled`, default true) and zoom-out can be disabled (`webtoon_disable_zoom_out`, default false), both area 44 consumers of this scope's prefs.
- Webtoon scale type is always FIT_WIDTH (`WebtoonPageHolder.kt:201`).

## 2. Loading indicator (`ReaderProgressIndicator`)
- Centered (`Gravity.CENTER`, `WRAP_CONTENT`), above the page, shown while a chapter or page is loading (the viewer calls `show()`/`hide()`).
- Spinner rotates continuously ("always rotates" by design so the app does not look stuck). Progress (0..100) is available through `setProgress`, drawn as a determinate ring when set; the Kotlin composable is `CombinedCircularProgressIndicator(progress = { progress })` (presentation-core, not in scope; its exact look is area 49).
- Rust: `gtk::Spinner` (indeterminate) for `show()`; when `set_progress` is used, show a `gtk::ProgressBar`-style ring via `DrawingArea` with `progress/100`. Default: spinner only.
- Color: follows the theme's foreground (`LocalContentColor` onBackground in Kotlin); in GTK, use the widget's inherited `color` (no hard-coded white/black) so the Tokyo Night theme applies.
- Size: default spinner size (GTK default 16 px); no spec for dp size in the Kotlin; keep the GTK default.

## 3. Chapter transition panel and retry button

### 3.1 Transition panel (`ReaderTransitionView`)
- Hosted inside a pager transition holder (area 44 `PagerTransitionHolder.kt:61`) and the webtoon chapter-gap rows. Layout: WRAP_CONTENT both ways.
- Content (Kotlin composable `ChapterTransition`, in part 1 `presentation/reader/ChapterTransition.kt`): previous/current/next chapter names, the "missing chapters" line when `chapterGap > 0` (behavior §11.1), and the download status for the current chapter (`currChapterDownloaded`) and the target chapter (`goingToChapterDownloaded`).
- Typography: `bodySmall` text style, color `onBackground` (Kotlin `CompositionLocalProvider`, `ReaderTransitionView.kt:229-237`). Rust: use GTK `caption`/`dim-label` style classes on secondary text; color inherits from the window.
- States (from the mockup copy in `spec/reader_player.json`, which mirrors Aniyomi strings; Kotlin source in part 1): "There's no previous chapter", "There's no next chapter", "Skipping N chapters, either the source is missing them or they have been filtered out" (the gap message), "Loading pages…", "Failed to load pages: %s" with "Retry" button (`ReaderButton`).
- Data is computed once per `bind` (no live update) in the Kotlin; Rust proposal updates the two booleans when a download-state event for the target chapter arrives.

### 3.2 Retry button (`ReaderButton`)
- Standard button, label "Retry" (mockup) — Kotlin uses `MaterialButton` with the `materialButtonStyle` default.
- Behavior: clicking retries the page load. Kotlin pager coupling (disabling the pager gesture detector while touched) is not needed in GTK.
- Focus: must be keyboard-activatable (Enter/Space) and show focus ring (GTK default).

## 4. E-ink flash overlay (`DisplayRefreshHost`)
- Only exists when `flash_on_page_change` is true (default false). When off, no overlay widget.
- A full-size overlay draws a solid color over the whole reader for the flash duration. Colors: BLACK (full black), WHITE (full white), WHITE_BLACK (white for half the duration, then black for half).
- Duration: `flash_duration_millis` (default 100 ms, settings slider 100..1500 ms in 100 ms steps). Interval: `flash_page_interval` (default 1 = every page; settings 1..10).
- The overlay is pointer-transparent (the Kotlin Canvas does not consume touch; in GTK set `can-target = false`).
- Rapid page turns during a flash do not restart it (behavior §14); a new flash starts after the current one ends.
- Desktop note: flash is for e-ink panels; on LCD the overlay is a visible black/white blink. Keep it off by default (as Kotlin).

## 5. Settings controls bound to ReaderPreferences

Layout source: Kotlin composables in `app/src/main/java/eu/kanade/presentation/reader/settings/*.kt` (part 1 presentation) and `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsReaderScreen.kt` (app settings screen; area 47). Every control below is one row in the reader settings sheet; the mockup renders them from `spec/reader_player.json` (`src/ui.rs item_row`).

### 5.1 General page (`GeneralSettingsPage.kt`)
| Control | Type | Pref | Default | Enabled when | Options / range |
|---|---|---|---|---|---|
| Background color | chip row (single select) | `reader_theme` (`pref_reader_theme_key`) | 1 (black) | always | 1 Black, 2 Gray, 0 White, 3 Automatic (labels `black_background`, `gray_background`, `white_background`, `automatic_background`) |
| Show page number | checkbox | `pref_show_page_number_key` | true | always | — |
| Fullscreen | checkbox | `fullscreen` | true | always | — |
| Show content in cutout area | checkbox | `cutout_short` | true | only when the device has a display cutout AND fullscreen is on | Linux: never shown (P3 drop) |
| Keep screen on | checkbox | `pref_keep_screen_on_key` | true | always | — |
| Show actions on long tap | checkbox | `reader_long_tap` | true | always | — |
| Always show chapter transition | checkbox | `always_show_chapter_transition` | true | always | — |
| Animate page transitions | checkbox | `pref_enable_transitions_key` | true | always | — |
| Flash on page change (E-ink group) | checkbox | `pref_reader_flash` | false | always | see §5.6 |
| Skip read chapters / Skip filtered / Skip duplicates | checkboxes | `skip_read` (false), `skip_filtered` (true), `skip_dupe` (false) | — | always | consumed by reader chapter navigation (part 1) |
| Preserve reading position on read chapters | checkbox | `pref_preserve_reading_position` | false | always | — |
| Read with volume keys / inverted | checkboxes | `reader_volume_keys`, `reader_volume_keys_inverted` | false | always | desktop: see behavior of volume keys in p2-rust-design §8 |
| Double tap animation speed | slider | `pref_double_tap_anim_speed` | 500 ms | always | slider range is defined in the settings screen (area 47, not read in this part) |

### 5.2 Reading mode page (`ReadingModePage.kt`)
- Section "For this series": chips for all six reading modes (`ReadingMode.entries`), selected per manga (`manga.readingMode`), plus "use default" when not DEFAULT (`ReadingModeSelectDialog.kt:64`). Labels: "Default", "Paged (left to right)", "Paged (right to left)", "Paged (vertical)", "Long strip", "Long strip with gaps".
- Reader orientation chips for the series (part 1 `ReaderOrientation`, entries DEFAULT 0x00, FREE 0x08, PORTRAIT 0x10, LANDSCAPE 0x18, LOCKED_PORTRAIT 0x20, LOCKED_LANDSCAPE 0x28, REVERSE_PORTRAIT 0x30; mask 0x38).
- Then, by current viewer: **pager** settings or **webtoon** settings.

Pager settings (`PagerViewerSettings` at `ReadingModePage.kt:71`):
| Control | Type | Pref | Default | Enabled when |
|---|---|---|---|---|
| Scale type | chip row | `pref_image_scale_type_key` | 1 Fit screen | always; options 1..6 (Fit screen, Stretch, Fit width, Fit height, Original size, Smart fit) |
| Zoom start position | chip row | `pref_zoom_start_key` | 1 Automatic | always; options Automatic, Left, Right, Center |
| Crop borders | checkbox | `crop_borders` | false | always |
| Automatically zoom into wide images | checkbox | `landscape_zoom` | true | scale type == 1 (`SettingsReaderScreen.kt:268`) |
| Pan wide images | checkbox | `navigate_pan` | true | scale type == 1 |
| Tap zones | list | `reader_navigation_mode_pager` | 0 Default | always; options Default, L shaped, Kindle-ish, Edge, Right and Left, Disabled |
| Invert tap zones | list | `reader_tapping_inverted` | None | always; None, Horizontal, Vertical, Both |
| Split wide pages | checkbox | `pref_dual_page_split` | false | always |
| Invert split page placement | checkbox | `pref_dual_page_invert` | false | split on |
| Rotate wide pages to fit | checkbox | `pref_dual_page_rotate` | false | always (turning it on turns split off, `SettingsReaderScreen.kt:293`) |
| Flip orientation of rotated wide pages | checkbox | `pref_dual_page_rotate_invert` | false | rotate on |

Webtoon settings (`WebtoonViewerSettings` at `ReadingModePage.kt:148`):
| Control | Type | Pref | Default | Enabled when |
|---|---|---|---|---|
| Tap zones | list | `reader_navigation_mode_webtoon` | 0 | always |
| Invert tap zones | list | `reader_tapping_inverted_webtoon` | None | tap zones != 5 (Disabled) (`SettingsReaderScreen.kt` webtoon group: `enabled = navMode != 5`) |
| Side padding | slider 0..25 | `webtoon_side_padding` | 0 | always; subtitle as percent (value/100, e.g. "10%") |
| Crop borders | checkbox | `crop_borders_webtoon` | false | always |
| Split wide pages | checkbox | `pref_dual_page_split_webtoon` | false | always |
| Invert split page placement | checkbox | `pref_dual_page_invert_webtoon` | false | split on |
| Rotate wide pages to fit | checkbox | `pref_dual_page_rotate_webtoon` | false | always |
| Flip orientation of rotated wide pages | checkbox | `pref_dual_page_rotate_invert_webtoon` | false | rotate on |
| Double tap to zoom | checkbox | `pref_enable_double_tap_zoom_webtoon` | true | always |
| Disable zoom out | checkbox | `webtoon_disable_zoom_out` | false | always |
| Hide menu threshold (list) | list | `reader_hide_threshold` | Low | always; Highest (5 px), High (13), Low (31), Lowest (47) |

Note: the paged "Invert tap zones" list and the webtoon one use the same four labels.

### 5.3 Color filter page (`ColorFilterPage.kt`)
| Control | Type | Pref | Default | Enabled when |
|---|---|---|---|---|
| Custom brightness | checkbox | `pref_custom_brightness_key` | false | always |
| Brightness | slider | `custom_brightness_value` | 0 | custom brightness on; range -75..100 |
| Custom color filter | checkbox | `pref_color_filter_key` | false | always |
| Color | color picker (red, green, blue, alpha channels as sliders) | `color_filter_value` | 0 | color filter on |
| Blend mode | chip row | `color_filter_mode` | 0 Default | color filter on; Default, Multiply, Screen (+ Overlay, Dodge/Lighten, Burn/Darken on API 28+ → always on Linux) |
| Grayscale | checkbox | `pref_grayscale` | false | always |
| Inverted colors | checkbox | `pref_inverted_colors` | false | always |

Brightness text in Kotlin KDoc: -75..-1 darkens (black overlay with minimum brightness), 1..100 sets screen brightness, 0 system brightness (no overlay). Linux: no screen brightness API for the desktop session; the -75..-1 darkening overlay is kept; 1..100 maps to no-op with a note (T-43b-049).

### 5.4 E-ink group (in the general settings screen, `SettingsReaderScreen.kt` `getEInkGroup`)
Group title "E-Ink" (literal in Kotlin). All controls disabled when flash is off.
| Control | Type | Pref | Default | Range |
|---|---|---|---|---|
| Flash on page change | switch | `pref_reader_flash` | false | — |
| Flash duration | slider | `pref_reader_flash_duration` | 100 ms | 1..15 (×100 ms), subtitle "%1$s ms" |
| Flash every | slider | `pref_reader_flash_interval` | 1 | 1..10, subtitle plural `pref_pages` |
| Flash with | list | `pref_reader_flash_mode` | Black | Black, White, White and Black |

### 5.5 Other settings-screen items of this scope's keys (for completeness)
- Preserve reading position, skip read, skip filtered, skip duplicates: under the "Chapters" section of the reader settings screen (area 47 renders the screen; keys here).
- Folder per manga (`create_folder_per_manga`): downloads (area 23).

## 6. Responsive behavior

- Reader is full-window and immersive (`src/main.rs:54-58`: sidebar and tab bar hidden for `screen:reader`).
- Narrow windows (< 600 px): page fills the width; fit scale recalculated on size change (`min_scale` uses the current view size). Settings sheet becomes a full-height modal (existing `src/ui.rs:377 modal` sizing).
- Wide windows: page is centered; no max width (Kotlin fills the window). Keep this.
- Window resize while zoomed: keep the center in source coordinates (proportional), clamp to bounds; do not reset zoom.
- Rotation (phone-like) is not applicable on desktop; "Rotation" orientation settings are kept for the data model and become a no-op with a visible note in settings (P3).
- Small heights: the OSD top bar wraps labels with ellipsis (`two_line` in `src/player.rs:32`).

## 7. Keyboard and mouse equivalents

| Android | Desktop (proposal) |
|---|---|
| Tap zone (touch) | left click on zone (same zones) |
| Double tap | double click |
| Pinch | ctrl + wheel (zoom step 1.1x per notch), or touchpad pinch (GestureZoom) |
| Drag | left button drag |
| Long press | press and hold ≥ long-press time (GTK setting); also right click (new) |
| Volume keys | XF86Audio keys (may be swallowed), plus PageUp/PageDown and arrows |
| Back button | Escape closes the settings sheet; Backspace/Alt+Left = back to library (new, documented) |
| Scroll (webtoon) | mouse wheel / touchpad scroll |
| Settings sheet | opened from the OSD menu button; Escape closes |

## 8. Gap list versus the mockup (`/home/blackbox/kurobayomi`)

| Element | Mockup now | Needed |
|---|---|---|
| Page display | `src/player.rs:55` placeholder `cover(420, 600)` | real `PageView` with decode |
| Tap zones | comment only (`src/player.rs:122`) | `nav::action_at` + area 44 layouts |
| Zoom, pan, double tap, pinch | none | `PageView` gestures |
| Long press | none (the OSD has `on_long_press` for other things) | `LongPressTracker` |
| Loading spinner | none | `LoadingIndicator` |
| Chapter transition panel | none | `ChapterTransitionView` |
| Retry button | none | retry in error state |
| Flash overlay | none | `FlashHost` (P2) |
| Reader settings values | labels only; no values stored | `ReaderPrefs` binding through `pref` keys in `spec/reader_player.json` |
| Color filter / brightness overlay | none | `snapshot` overlay |
| Keep screen on | none | `Application::inhibit` |
| Fullscreen toggle | none | window fullscreen |
| Bookmark chapter | toggle (`src/player.rs:44`) | out of scope (part 1 / area 44) |
| Reading mode and orientation per series | sheet labels only | per-manga flags (area 05/03) |
| Keyboard page turns | none | `Viewer::handle_key` |
| Volume-key page turns | none | XF86 mapping (P1) |
