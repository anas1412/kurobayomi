# 43-reader-core: UI parity (reader chrome, dialogs, states, gestures)

Scope of UI in this area: the reader chrome and sheets that wrap the viewers. The page rendering itself (pager, webtoon, zoom, tap regions) is area:44-reader-viewers. Kotlin sources are repo-relative to `/home/blackbox/aniyomi-src`. The Kurobayomi mockup is `/home/blackbox/kurobayomi/src/player.rs` (function `reader`, lines 49-150) and `/home/blackbox/kurobayomi/spec/reader_player.json` (screen id `reader`).

Format: each element lists the Kotlin source, the visible content, the states, and the Linux behaviour (PROPOSAL). Text in quotes is the English string from `i18n/src/commonMain/moko-resources/base/strings.xml` or the key named in parentheses.

---

## 1. Screen states

| State | Trigger | Kotlin rendering | Linux behaviour (proposal) |
|---|---|---|---|
| Idle, no manga | before `init` | activity shows nothing (`ReaderActivity.kt:163-187`); `ifMangaSourcesLoaded()` gates the compose content (`:363-365`) | show the same blank page with the top bar only; spinner while the source manager initialises |
| Loading chapter (initial) | `init` in progress | no explicit spinner; viewer not yet created | centred spinner (`gtk::Spinner`) and the title line from the library entry |
| Loading adjacent chapter | `loadNext`/`loadPrevious` | blocking `AlertDialog` with `CircularProgressIndicator` and text "Loading…" (`loading`), non-dismissable (`ReaderActivity.kt:441-455`) | non-blocking: navigator buttons disabled, spinner inside the navigator centre pill; no modal |
| Loading chapter (background, swipe) | viewer crosses into next chapter | `ReaderProgressIndicator` added to container (`ReaderActivity.kt:559-560`), removed on `setChapters` (`:615`) | spinner overlay on the page area, removed on `setChapters` |
| Loaded | pages available | viewer shows pages | viewer (area:44-reader-viewers) |
| Error, initial | `init` fails | activity finishes and shows a toast with the error text (`ReaderActivity.kt:629-633`) | PROPOSAL: stay in the reader and show a card "Failed to load pages: <message>" with a "Retry" button (the mockup spec already names these strings: `reader_player.json` group "Chapter transition") and keep a Close button. Message text is `LoadError::user_message`. |
| Error, adjacent | `loadNext`/`loadPrevious` fails | silent (logged only); then page 0 of the current chapter is shown (E-NAV-1) | toast with the message; stay on the current chapter at the current page |
| Empty page list | `page_list_empty_error` | same as Error | same as Error, message "No pages found" |
| Source not installed | `source_not_installed` | same as Error (raw text) | same as Error with the source name |
| Loader not implemented | `loader_not_implemented_error` | same as Error | same |
| Chapter transition | viewer reaches an end (area:44-reader-viewers) | `ChapterTransition` card (section 6) | card in the page area |
| Dialog open | any sheet | see section 4 | see section 4 |
| Menus hidden | default when reading | page indicator visible (if pref on), no bars | same |
| Menus visible | tap centre (area:44-reader-viewers) or open a sheet | top bar, navigator, bottom bar | same; sheets keep the chrome visible |

---

## 2. Top app bar (`app/src/main/java/eu/kanade/presentation/reader/appbars/ReaderAppBars.kt:84-152`)

Visible when `menuVisible`. Slides down from the top, 200 ms (`:35`).

Elements, in order:
1. Back navigation icon (`AppBar` `navigateUp`, `:101`). Android: `onBackPressedDispatcher::onBackPressed`. Linux: back button (`go-previous-symbolic`) and Escape (after leaving fullscreen).
2. Title block: line 1 = `mangaTitle` (`state.manga?.title`), line 2 = `subtitle = chapterTitle` (`state.currentChapter?.chapter?.name`) (`:99-100`). Clicking the block opens the manga details (`openMangaScreen`, `:565-575`).
3. Bookmark action (`:106-122`):
   - Icon `Bookmark` when bookmarked else `BookmarkBorder`.
   - Title (also tooltip/content description): `action_remove_bookmark` "Unbookmark chapter" when bookmarked, else `action_bookmark` "Bookmark chapter".
   - Always shown (any source).
   - Click: `toggleChapterBookmark` (`ReaderViewModel.kt:645-664`).
4. Overflow menu, each item only when its callback exists, i.e. only for HTTP sources (`:123-146`):
   - `action_open_in_web_view` "Open in WebView".
   - `action_open_in_browser` "Open in browser".
   - `action_share` "Share".
   Local sources show only the bookmark action.

Mockup (`player.rs:55-63`): back button (tooltip "Back"), two-line title (`two_line(title, chapter)`), bookmark ToggleButton (tooltip "Bookmark chapter"/"Unbookmark chapter", local state), overflow menu from `screen("reader").menu` (`spec`: "Open in WebView", "Open in browser", "Share").

Gaps: title and chapter are fixed strings (`screens.rs:37` passes "Title 1", "Chapter 1"); bookmark state is not loaded from or saved to the chapter; overflow items are not wired to actions; the menu is not filtered by source type; title click does not open the manga.

---

## 3. Chapter navigator and bottom bar (`P/components/ChapterNavigator.kt`, `P/appbars/BottomReaderBar.kt`)

### 3.1 Chapter navigator (`ChapterNavigator.kt:45-149`)

Layout, left to right (LTR screen; reversed for R2L mode, section 3.1.1):
1. Filled icon button, icon `SkipPrevious` (always), enabled when `enabledPrevious` (`:78-89`). Description `action_previous_chapter` "Previous chapter". Click: previous chapter.
2. Pill (only when `totalPages > 1`, else an empty spacer), rounded 24 dp, background `surfaceColorAtElevation(3.dp)` at 0.9 (dark) or 0.95 (light) alpha, horizontal padding 16 dp (`:91-130`):
   - Current page number text (left).
   - Slider, range `1..totalPages`, step 1, value `currentPage`, horizontal padding 8 dp, weight 1 (`:115-126`). On change to a different value: `onPageIndexChange(value - 1)`.
   - Total pages text (right). A transparent copy of the total is laid out behind the current page number so the slider does not move when the number's width changes (`:101-105`).
   - Haptic tick while dragging (no Linux equivalent, section 9).
3. Filled icon button, icon `SkipNext` (always), enabled when `enabledNext`. Description `action_next_chapter` "Next chapter".

#### 3.1.1 Right-to-left (`isRtl`, `ReaderAppBars.kt:69`)
- Whole navigator is laid out LTR (`:71`) but the pill row is RTL (`:92`).
- Left button: enabled by `enabledNext`, click = next chapter, description `action_next_chapter`.
- Right button: enabled by `enabledPrevious`, click = previous chapter, description `action_previous_chapter`.

Exact mapping (`ChapterNavigator.kt:78-89`, `:135-146`):

| Position | Mode | Enabled by | Click | Description |
|---|---|---|---|---|
| Left | LTR / vertical | `enabledPrevious` | previous | `action_previous_chapter` |
| Left | RTL | `enabledNext` | next | `action_next_chapter` |
| Right | LTR / vertical | `enabledNext` | next | `action_next_chapter` |
| Right | RTL | `enabledPrevious` | previous | `action_previous_chapter` |

Icons never change (left is always `SkipPrevious`, right always `SkipNext`). Observed in RTL, the left button shows a "skip previous" glyph while it means next chapter. PROPOSAL: keep the Kotlin positions but mirror the icons in RTL so the glyph matches the action (a deliberate improvement; record it in the tasks as a visible change).

Linux layout: GTK `gtk::Box` horizontal with two `gtk::Button` (icon names `media-skip-backward-symbolic` / `media-skip-forward-symbolic`, mockup already uses these, `player.rs:94-98`) and a `gtk::Scale` (`gtk::Scale::with_range(Orientation::Horizontal, 1.0, n, 1.0)`, `draw_value = false`, `digits = 0`). The current and total labels are `gtk::Label` with the same invisible-sizing trick (`set_width_chars` of the total).

Behaviour parity checks:
- Slider value equal to current page: no action (`ChapterNavigator.kt:122`).
- `currentPage` may be -1 before the first page event (`ReaderViewModel.kt:962`). Show the slider at 1 and the label as "-" until the first page; PROPOSAL, because the Kotlin Slider receives an out-of-range value there.
- Slider is hidden when total is 1 or less (`:131-133`), leaving a spacer.

### 3.2 Bottom bar (`BottomReaderBar.kt:24-71`)

Row of four `IconButton`, space-evenly, padding 8 dp, background = the same bar colour as the navigator (`:35-42`):

| # | Icon | Content description | Click | Notes |
|---|---|---|---|---|
| 1 | `readingMode.iconRes` (painter), raw manga value | `viewer` "Reading mode" | opens Reading mode sheet (`openReadingModeSelectDialog`) | raw value: a Default manga shows the Default icon (`ReaderActivity.kt:416-418`) |
| 2 | `orientation.icon` (vector), raw | `rotation_type` "Rotation" | opens Rotation sheet | same raw rule (`:420-422`) |
| 3 | `R.drawable.ic_crop_24dp` when crop on, else `ic_crop_off_24dp` | `pref_crop_borders` "Crop borders" | toggles crop, toast "On"/"Off" | crop pref depends on viewer type (pager or webtoon, `ReaderViewModel.kt:742-749`) |
| 4 | `Icons.Outlined.Settings` | `action_settings` "Settings" | opens settings dialog | |

Linux: `gtk::Box` horizontal, `homogeneous = true`, four flat `gtk::Button`s with symbolic icons. Icon names: reading mode `view-paged-symbolic` (mockup), rotation `object-rotate-right-symbolic` (mockup), crop `edit-cut-symbolic` (mockup, PROPOSAL: a crop glyph is not standard in Adwaita; keep the mockup name and make the toggle state visible with a `toggled` CSS class), settings `emblem-system-symbolic` (mockup).

Gaps vs mockup (`player.rs:117-127`): the crop control is a `ToggleButton` that toggles local state and shows "On"/"Off" toast (matches Kotlin's toast) but is not persisted and is not per viewer type; reading mode and rotation open the spec sheets without data; settings opens the spec sheet without data.

---

## 4. Sheets and dialogs

### 4.1 Reading mode sheet (`P/ReadingModeSelectDialog.kt:31-81`)

- Container: bottom sheet (`AdaptiveSheet`), padding 16 dp vertical (`ModeSelectionDialog.kt:34`).
- Title grid: `SettingsIconGrid` titled `pref_category_reading_mode` "Reading mode" (`:67`).
- Grid items: `LEFT_TO_RIGHT`, `RIGHT_TO_LEFT`, `VERTICAL`, `WEBTOON`, `CONTINUOUS_VERTICAL` (no DEFAULT). Each is an `IconToggleButton` with the mode icon and label (`:68-78`). Labels: "Paged (left to right)" (`left_to_right_viewer`), `right_to_left_viewer`, `vertical_viewer`, `webtoon_viewer` "Long strip", `vertical_plus_viewer`.
- Selection: tapping an item only selects it; nothing is saved until "Apply" (local `selected`, `:61-65`).
- Footer row (`ModeSelectionDialog.kt:38-65`): left "Revert to default" (`action_revert_to_default`, OutlinedButton, shown only when the current manga mode is not DEFAULT); right "Apply" (`action_apply`, FilledTonalButton with check icon).
- Apply: writes the mode, toast with the mode label (`onChange`, `ReaderActivity.kt:468-473` shows the toast only when `showReadingMode` is false; see behaviour section 12.11), dismiss.
- Revert: writes DEFAULT, dismiss.

Linux: `gtk::Revealer` anchored to the bottom, containing a `gtk::FlowBox` of five toggle buttons (icon + label), and a footer `gtk::Box`. Selected item uses a `checked` style. Keyboard: arrow keys move focus, Space selects, Enter applies.

Mockup: `reading_mode_select` dialog exists in `reader_player.json` (not re-read here); not wired.

### 4.2 Rotation sheet (`P/OrientationSelectDialog.kt:29-83`)

Same structure. Items: `FREE`, `PORTRAIT`, `LANDSCAPE`, `LOCKED_PORTRAIT`, `LOCKED_LANDSCAPE`, `REVERSE_PORTRAIT` (no DEFAULT). Labels: "Free", "Portrait", "Landscape", "Locked portrait", "Locked landscape", "Reverse portrait". Title `rotation_type` "Rotation". Apply always toasts (`:45-48`). Revert shown when not DEFAULT.

Linux: same widget structure. PROPOSAL: because orientation locking is dropped on Linux (`p1-rust-design.md` section 15), the sheet still lets the user set and store the value (for sync and backup compatibility), with a one-line note "Rotation has no effect on this platform". This is a visible change and is listed in the tasks.

### 4.3 Page actions sheet (`P/ReaderPageActionsDialog.kt:28-110`)

- Opened by long press on a page (`ReaderActivity.onPageLongTap`, `:694-696`). Only when `page.status == READY` for the action to do anything (`ReaderViewModel.kt:806`, `:854`, `:884`). The sheet itself opens for any page.
- Four equal `ActionButton`s in a row, each with icon and label, weight 1, spacing `padding.small`, vertical padding 16 dp (`:37-76`):
  1. `set_as_cover` "Set as cover" (icon `Photo`). Opens a confirm dialog (4.4). The sheet stays open after confirming (observed; the mockup should close it, see section 7 gap list).
  2. `action_copy_to_clipboard` "Copy to clipboard" (icon `ContentCopy`). Copies the image; sheet closes.
  3. `action_share` "Share" (icon `Share`). Share; sheet closes.
  4. `action_save` "Save" (icon `Save`). Saves; sheet closes.

Linux: `gtk::Revealer` sheet with four `gtk::Button`s with `Adw`-free styling; each button shows an icon and label. The sheet also opens from a right-click (context menu on the page) and from the keyboard shortcut Shift+F10 (PROPOSAL, desktop convention; not in Kotlin).

### 4.4 Set-as-cover confirm (`ReaderPageActionsDialog.kt:89-110`)

- `AlertDialog` with text `confirm_set_image_as_cover` "Use this image as cover art?", confirm `action_ok` ("OK"), dismiss `action_cancel` ("Cancel"). Dismiss on outside tap.
- Linux: `gtk::AlertDialog` (GTK 4.10+) with the same texts.

### 4.5 Reader settings dialog (`P/settings/ReaderSettingsDialog.kt:22-71`)

- Tabbed dialog, max height 75 percent of the window (`:38`). Three tabs: `pref_category_reading_mode` "Reading mode", `pref_category_general` "General", `custom_filter` "Custom filter".
- Tab 3 (Custom filter) dims the background 0 percent and hides the reader chrome (`:49-51`). Other tabs dim 50 percent and show the chrome (`:52-55`). Linux: the dialog is a modal `gtk::Window` with `gtk::Notebook`; for tab 3 the window becomes non-modal and the page view stays fully visible (PROPOSAL, mirrors the intent: the user previews the colour filter).
- Each tab scrolls (`verticalScroll`, `:58-61`).

Tab 1 (`ReadingModePage.kt`), in order:
1. Heading `pref_category_for_this_series` "For this series".
2. Chips `pref_category_reading_mode` "Reading mode": ALL six modes including Default (`ReadingMode.entries`). Applies immediately (`:37-45`).
3. Chips `rotation_type` "Rotation": all seven incl. Default (`:52-60`). Applies immediately.
4. If the viewer is webtoon (`WEBTOON` or `CONTINUOUS_VERTICAL`) -> webtoon block, else pager block (`:62-67`).

Pager block (`PagerViewerSettings`, `:70-145`):
- Heading `pager_viewer` "Paged".
- Tap zone chips `pref_viewer_nav` "Tap zones": "Default", "L shaped", "Kindle-ish", "Edge", "Right and Left", "Disabled". If not "Disabled": chips `pref_read_with_tapping_inverted` "Invert tap zones": "None", "Horizontal", "Vertical", "Both" (`TapZonesItems`, `:215-243`).
- Chips `pref_image_scale_type` "Scale type": "Fit screen", "Stretch", "Fit width", "Fit height", "Original size", "Smart fit". Default "Fit screen".
- Chips `pref_zoom_start` "Zoom start position": "Automatic", "Left", "Right", "Center". Default "Automatic".
- Checkbox `pref_crop_borders` "Crop borders".
- Checkbox `pref_landscape_zoom` "Automatically zoom into wide images" (default on).
- Checkbox `pref_navigate_pan` "Pan wide images" (default on).
- Checkbox `pref_dual_page_split` "Split wide pages". OBSERVED BUG: bound to `dualPageInvertPaged` (`ReadingModePage.kt:120-124`). Linux must bind it to `dual_page_split_paged` (PROPOSAL, fix; see E-UI-3).
- If split is on: checkbox `pref_dual_page_invert` "Invert split page placement", bound to `dualPageRotateToFitInvert` (`:126-131`). Also a mis-binding (its key is the rotation-invert key). Linux: bind to `dual_page_invert_paged` (E-UI-3).
- Checkbox `pref_page_rotate` "Rotate wide pages to fit" (`dualPageRotateToFit`).
- If rotate is on: checkbox `pref_page_rotate_invert` "Flip orientation of rotated wide pages" (`dualPageRotateToFitInvert`).

Webtoon block (`WebtoonViewerSettings`, `:147-213`):
- Heading `webtoon_viewer` "Long strip".
- Tap zones and inversion (same as pager, webtoon keys).
- Slider `pref_webtoon_side_padding` "Side padding", range 0..25, value text as a percent of value/100 (`NumberFormat.getPercentInstance`). Pill colour `surfaceContainerHighest`.
- Checkbox `pref_crop_borders` "Crop borders" (webtoon key `crop_borders_webtoon`).
- Checkbox `pref_dual_page_split` "Split wide pages" (webtoon key). If on: `pref_dual_page_invert` "Invert split page placement" (webtoon invert key).
- Checkbox `pref_page_rotate` (webtoon key). If on: `pref_page_rotate_invert` (webtoon key).
- Checkbox `pref_double_tap_zoom` "Double tap to zoom" (default on).
- Checkbox `pref_webtoon_disable_zoom_out` "Disable zoom out" (default off).

Tab 2 (`GeneralSettingsPage.kt:33-127`), in order:
1. Chips `pref_reader_theme` "Background color": "Black", "Gray", "White", "Auto" (default Black).
2. Checkbox `pref_show_page_number` "Show page number" (default on).
3. Checkbox `pref_fullscreen` "Fullscreen" (default on).
4. Checkbox `pref_cutout_short` "Show content in cutout area" (only when a cutout exists and fullscreen is on; Linux: hidden, section 9 of rust design).
5. Checkbox `pref_keep_screen_on` "Keep screen on" (default on).
6. Checkbox `pref_read_with_long_tap` "Show actions on long tap" (default on).
7. Checkbox `pref_always_show_chapter_transition` "Always show chapter transition" (default on).
8. Checkbox `pref_page_transitions` "Animate page transitions" (default on).
9. Checkbox `pref_flash_page` "Flash on page change" (default off). When on:
   - Slider `pref_flash_duration` "Flash duration", steps of 100 ms from 100 to 1500, value text `pref_flash_duration_summary` "%1$s ms".
   - Slider `pref_flash_page_interval` "Flash every", 1..10, value text `pref_pages` ("1 page" / "%1$s pages").
   - Chips `pref_flash_with` "Flash with": "Black", "White", "White and Black".

Tab 3 (`ColorFilterPage.kt:24-125`), in order:
1. Checkbox `pref_custom_brightness` "Custom brightness". When on: slider `-75..100` labelled `pref_custom_brightness` (no value text).
2. Checkbox `pref_custom_color_filter` "Custom color filter". When on: sliders "Red" (`color_filter_r_value`), Green, Blue, Alpha (0..255 each); chips `pref_color_filter_mode` "Color filter blend mode": "Default", "Multiply", "Screen", "Overlay", "Dodge / Lighten", "Burn / Darken".
3. Checkbox `pref_grayscale` "Grayscale".
4. Checkbox `pref_inverted_colors` "Inverted".

Linux: all controls are `gtk::CheckButton`/`gtk::Switch` rows, `gtk::Scale` for sliders with the value label, and toggle-button groups for chips. Persistence is immediate (Kotlin writes on each change).

### 4.6 Loading dialog (Kotlin only)

See section 1: on Linux the adjacent-chapter loading is non-modal (spinner in navigator). Only the mockup spec "Loading pages…" is shown on the initial load card.

---

## 5. Page indicator (`P/PageIndicatorText.kt:17-50`)

- Text `"<n> / <total>"` (1-based page, total pages).
- Visible only when menus are hidden AND `show_page_number` is on (`ReaderActivity.kt:340-349`). Hidden when `n <= 0` or `total <= 0`.
- Style: fill `#EBEBEB` bold body-small with 1 sp letter spacing and a 4 px `#2D2D2D` stroke outline underneath (`:26-44`). Position: bottom centre (mockup: `valign End`, `margin_bottom 8`, `caption-heading` css class, `player.rs:130-136`).
- Linux: `gtk::Label` with CSS `text-shadow` for the outline (no stroke API). Colours from the theme variables (section 8).

---

## 6. Chapter transition card (`P/ChapterTransition.kt`)

Shown by the viewer at chapter edges (area:44-reader-viewers). Content and rules are in `p1-behavior.md` section 14. Visual:
- Column, max width 460 dp, centred.
- Top block and bottom block. Each block: header (title-medium text: "Previous:", "Current:", "Finished:", "Next:"), then the chapter name (title-large, 20 sp, max 5 lines, ellipsis), with an inline check-circle before the name when downloaded (accessible name "Downloaded"), then the scanlator (body-small, max 2 lines, ellipsis, dimmed).
- Gap warning card between the two blocks (only when `gap > 0`): error-coloured warning icon and the plural message.
- "No chapter" card (info icon, primary colour) for the missing side: "There's no previous chapter" / "There's no next chapter".
- Outlined cards with transparent background and `onSurface` content colour.
- Spacing: 24 dp between blocks.

Linux: `gtk::Box` vertical, `max-width` 460 px (CSS), labels with `ellipsize` and `lines`, `gtk::Image` for icons. The card colours come from the GTK theme.

Mockup spec: "Chapter transition" group lists exactly these strings (`reader_player.json`). The mockup itself has no transition card (gap).

---

## 7. Navigation overlay (`R/ReaderNavigationOverlayView.kt:19-116`)

- Shows region labels over the viewer (for example "Menu", "Prev", "Next"; the exact labels come from the active tap-zone layout in area:44-reader-viewers). Mockup spec "Tap zone overlay" lists "Menu", "Prev", "Next", "Left", "Right".
- Appears on first launch only when `show_navigation_overlay_on_start` is on (default off), and fades in over 1000 ms (`:28-47`). Hidden for the "Disabled" tap zones (`DisabledNavigation`).
- Any touch fades it out over 1000 ms (`:92-113`).
- Each region: translucent fill and label with black stroke and white text, font 64 px scaled to the view (`:49-89`).

Linux: `gtk::DrawingArea` above the viewer, `set_can_target(false)` so it never eats clicks (PROPOSAL; Kotlin consumes touches via `performClick` but the viewer still handles them). Fade with `gtk::Widget` opacity transition 1000 ms (CSS `transition` or `glib::timeout`).

---

## 8. Colours, theme and alpha

- Reader background (`pref_reader_theme`): Black (default), Gray `#202125`, White, Auto (gray when the system is dark, else white). Source: `ReaderActivity.kt:840-850`, `:834`.
- Chrome background: `surfaceColorAtElevation(3.dp)` with alpha 0.9 dark / 0.95 light (`ReaderAppBars.kt:70-72`, `ChapterNavigator.kt:62-64`).
- Toolbar window colour: `SurfaceColors.SURFACE_2` at alpha 230 (dark) / 242 (light) (`ReaderActivity.kt:498-507`). On Linux there is no system bar colour, so this is dropped.
- Page indicator: fill `#EBEBEB`, outline `#2D2D2D`.
- Brightness overlay: black with alpha `|v|/100`.
- Colour filter: user-defined ARGB with blend modes (behaviour section 18).

Project rule (from the user's memory, `match-current-theme`): the user runs Tokyo Night everywhere; the reader chrome MUST take colours from the active GTK theme (CSS named colours such as `@theme_bg_color`, `@headerbar_bg_color`), not hard-coded Android colours. Only the reader background choices (black, gray, white) and the overlays are fixed values, because they are content colours, not chrome.

---

## 9. Gestures, keyboard, mouse, pointer

| Input | Kotlin | Linux (PROPOSAL) |
|---|---|---|
| Tap (centre) | toggles menus (viewer, area:44-reader-viewers; `toggleMenu`, `ReaderActivity.kt:710-712`) | same (area:44-reader-viewers) |
| Tap (edges) | page turn per tap zone (area:44-reader-viewers) | same (area:44-reader-viewers) |
| Long press on a page | opens page actions (`onPageLongTap`, `ReaderActivity.kt:694-696`) | long press (500 ms) or right-click opens page actions |
| Drag on slider | changes page, haptic tick | changes page; no haptic |
| Key `N` | next chapter (`ReaderActivity.kt:309-311`) | same |
| Key `P` | previous chapter (`:312-314`) | same |
| Arrow keys / Page Up/Down | viewer (area:44-reader-viewers, `dispatchKeyEvent`, `:322-325`) | same (area:44-reader-viewers) |
| Mouse wheel | viewer (area:44-reader-viewers, `dispatchGenericMotionEvent`, `:331-334`) | same (area:44-reader-viewers) |
| Volume keys | viewer option `reader_volume_keys` (area:44-reader-viewers; default off) | not supported (P3, stored only) |
| Back | `finish()` (`:293-306`) | Escape (leaves fullscreen first) and a back button; Backspace not used |
| Fullscreen toggle | via `setMenuVisibility` and immersive flags (`:516-528`) | `F11` toggles window fullscreen (PROPOSAL, desktop convention) |
| Bookmark | button only | `Ctrl+D` (PROPOSAL) |
| Share/copy page | page actions | `Ctrl+C` copies the current page image (PROPOSAL) |
| Save page | page actions | `Ctrl+S` saves the current page (PROPOSAL) |

Shortcut rule: every new shortcut must be listed in the in-app shortcut help (area:36-ui-shell-navigation-security-webview owns the help screen, if one exists). PROPOSAL only; no Kotlin counterpart.

---

## 10. Animations and timing (visible)

| Element | Duration | Easing | Source |
|---|---|---|---|
| Top bar and bottom column slide | 200 ms | tween (default) | `ReaderAppBars.kt:35`, `:84-94`, `:156-166` |
| Navigation overlay fade in/out | 1000 ms | linear (default ViewPropertyAnimator) | `ReaderNavigationOverlayView.kt:116` |
| Flash on page change | total `flash_duration` (default 100 ms), half per colour in WHITE_BLACK | none (instant) | `DisplayRefreshHost.kt:61-79` |
| Page transitions | pref `pref_enable_transitions_key` (default on) | area:44-reader-viewers | |
| Activity open/close | shared axis X push/pop | Material | `ReaderActivity.kt:145-156`, `:293-305` (P3 replacement: Stack slide) |
| Loading dialog | none | | |
| Brightness sample | 100 ms | sampling | `ReaderActivity.kt:940` |

---

## 11. Toasts and messages (complete for the reader chrome)

Toast duration on Linux: 2000 ms (PROPOSAL, matches Android `LENGTH_SHORT`; the Kotlin constant is not in this scope). A new toast replaces the previous one of the same kind (`menuToggleToast?.cancel()` / `readingModeToast?.cancel()`, `ReaderActivity.kt:427`, `:469`, `:481`, `:601`).

| Toast | Trigger | Text key | English text |
|---|---|---|---|
| Crop on/off | crop button | `on` / `off` | "On" / "Off" |
| Reading mode name | viewer created when `show_reading_mode` on | reading-mode string | e.g. "Paged (left to right)" |
| Reading mode name | sheet applied when `show_reading_mode` OFF | reading-mode string | same |
| Rotation name | rotation sheet applied | `rotation_*` | e.g. "Locked portrait" |
| Picture saved | page saved | `picture_saved` | "Picture saved" |
| Cover updated | set as cover ok | `cover_updated` | "Cover updated" |
| Add to library first | set as cover, not in library | `notification_first_add_to_library` | "Please add the entry to your library before doing this" |
| Cover failed | set as cover error | `notification_cover_update_failed` | "Failed to update cover" |
| Initial load failure | init failed | error message | "No pages found", "Source not installed: %s", "Source not found" |
| Unknown err | manga missing | literal | "Unknown err" (not localised; PROPOSAL: localise) |
| Custom brightness unsupported | positive brightness, no writable backlight | new key | PROPOSAL text: "Custom brightness is not available on this display" (E-BRIGHT-2) |

Notification (desktop, not a toast): "Picture saved" with the thumbnail and a "Share" button (`SaveImageNotifier.kt:69-90`); error variant "Error" with the message or "Unknown error" (`:59-67`).

---

## 12. Mockup gap list (`/home/blackbox/kurobayomi/src/player.rs:49-150`, `spec/reader_player.json`)

| # | Element | Mockup | Required (Kotlin) | Gap / action |
|---|---|---|---|---|
| G-01 | Page total | fixed `let pages = 20.0;` (`player.rs:52`) | real page count from the loader | replace with `ReaderSession` data |
| G-02 | Page content | placeholder `cover(420, 600)` (`player.rs:58`) | decoded page textures | replace with pages; no placeholder art (user rule: no invented scaffolding) |
| G-03 | Title/subtitle | `screens.rs:37` passes "Title 1", "Chapter 1" | manga title, chapter name | pass real values |
| G-04 | Back | `pop()` (`player.rs:61`) | navigate up | keep, add Escape |
| G-05 | Title tap | none | opens manga (`openMangaScreen`) | add (area:38-ui-entry-details-manga route) |
| G-06 | Bookmark | local ToggleButton (`player.rs:65-67`) | persisted, initial state from chapter | wire to `ProgressWriter` |
| G-07 | Overflow menu | spec-driven, not filtered | HTTP-only items | filter by source kind; wire actions |
| G-08 | Prev/Next buttons | created, no click handler (`player.rs:94-98`, `:113-116`) | load adjacent chapter, disabled at ends | wire, enable/disable |
| G-09 | Page slider | `Scale 1..20`, updates label only (`player.rs:99-111`, `:118-125`) | moves the viewer to page index | call viewer `move_to_page` |
| G-10 | Page label | float formatted `s.value()` | integer page number | format as integer |
| G-11 | Current page -1 | not modelled | slider hidden or "-" before first page | add |
| G-12 | Slider hidden at 1 page | not modelled | hidden when total <= 1 | add |
| G-13 | RTL mirror | not modelled | navigator and pill mirror for R2L | add (area:44-reader-viewers provides mode) |
| G-14 | Reading mode button | opens spec sheet | sheet applies per-manga mode | wire |
| G-15 | Rotation button | opens spec sheet | sheet applies per-manga rotation (stored) | wire, add "no effect here" note |
| G-16 | Crop button | local state, toast | per-viewer-type persisted pref | wire |
| G-17 | Settings button | opens spec sheet | real settings values | wire to `ReaderSettings` |
| G-18 | Chrome toggle | center tap in mockup (`player.rs:139-146`) | area:44-reader-viewers decides tap zones | align with area:44-reader-viewers |
| G-19 | Tap side zones | fixed thirds `scale ±1` (`player.rs:141-146`) | area:44-reader-viewers tap zones (configurable) | replace |
| G-20 | Page indicator | updated from slider; visible when chrome hidden (`player.rs:129-137`) | from state; hidden at 0 | keep position, use state |
| G-21 | Page actions | `on_long_press` opens spec dialog (`player.rs:147`) | save, share, copy, set cover | implement |
| G-22 | Loading state | none | spinner (initial and adjacent) | add |
| G-23 | Error state | none | card with message and Retry | add (spec strings exist) |
| G-24 | Chapter transition | spec text only | card at chapter edges (area:44-reader-viewers triggers) | add |
| G-25 | Flash overlay | none | optional flash | add (P2) |
| G-26 | Brightness/colour overlays | none | optional overlays | add (P1 brightness, P1 colour) |
| G-27 | Navigation overlay labels | none | labels with fade | add (P2) |
| G-28 | Keyboard N/P | none | next/previous chapter | add |
| G-29 | Fullscreen | visibility only (`player.rs:139-146`) | window fullscreen with menus | add |
| G-30 | Keep screen on | none | inhibit idle while reading | add |
| G-31 | Reader background | fixed dark (`screens` / css) | pref: black, gray, white, auto | wire |
| G-32 | Settings sheet, webtoon branch | spec only | depends on viewer type | wire (area:44-reader-viewers viewer type) |
| G-33 | Split wide pages checkbox | spec "Split wide pages" with "Invert" subtitle | Kotlin binding bug; Linux must bind correctly | implement correct binding (E-UI-3) |
| G-34 | Tap zone label "Long strip with gaps" | spec text for continuous vertical | Kotlin string for `vertical_plus_viewer` not re-read here | verify text when porting strings |
| G-35 | Open-in-browser / WebView / share | spec menu strings only | real actions | wire (area:36-ui-shell-navigation-security-webview webkit6, area:12-network-stack helpers) |

Summary for the implementer: the mockup is a layout sketch with fixed data and no state. Everything in gaps G-01 to G-35 is required for parity with the reader core. Viewer-specific gaps (tap zones, zoom, scale, webtoon view) are owned by area:44-reader-viewers.

---

## 13. Accessibility

- Every icon-only button has a tooltip and an accessible name equal to its Kotlin content description (sections 2, 3.2, and the page-actions list). Keys: `action_previous_chapter`, `action_next_chapter`, `viewer`, `rotation_type`, `pref_crop_borders`, `action_settings`, `action_bookmark`, `action_remove_bookmark`, `label_downloaded`.
- Checked/toggled state exposed (GTK `toggled` state for bookmark and crop toggles; `checked` for chips and toggle buttons).
- Focus order: top bar -> navigator -> bottom bar; sheets trap focus while open.
- Text sizes follow the GTK font scale. Chapter names wrap, never clipped below 5 lines in the transition card.
- Colour is never the only signal: crop uses icon AND label; bookmark uses icon AND label change.
- Flash overlay is opt-in and documented as a photosensitivity risk in the settings label; PROPOSAL: add a warning line under "Flash on page change".

---

## 14. Responsive behaviour

- Kotlin switches horizontal padding of the navigator between 24 dp (tablet) and 8 dp (phone) (`ChapterNavigator.kt:56-57`). Linux PROPOSAL: use 24 px when the window is at least 720 px wide, else 8 px, re-evaluated on size change.
- Transition card max width 460 dp -> 460 px.
- Settings dialog max height 75 percent of the window (`ReaderSettingsDialog.kt:38`); on narrow windows the settings sheet fills the width.
- Bottom bar buttons are homogeneous; they scale with the window width and never wrap.
- Page indicator and navigator stay inside the safe area (no cutouts on desktop).

---

## 15. Dialog and sheet dismissal rules

- Reading mode, rotation, page actions sheets: dismiss on Escape, on click outside, and after the action (Kotlin `onDismissRequest` after apply, `ReadingModeSelectDialog.kt:44-53`).
- Settings dialog: dismiss restores the chrome (`onShowMenus`), `ReaderSettingsDialog.kt:39-42`.
- Set-as-cover confirm: Cancel or outside click dismisses only the confirm (`ReaderPageActionsDialog.kt:108`).
- Loading dialog: not dismissable (Kotlin). Not applicable on Linux (no modal loading).
- `closeDialog()` closes whatever dialog is open (`ReaderViewModel.kt:792-794`). A chapter load finishing while the page-actions sheet is open closes the sheet (observed). PROPOSAL: keep the sheet open unless the page it refers to is gone.
