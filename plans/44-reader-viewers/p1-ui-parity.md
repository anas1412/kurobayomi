# 44-reader-viewers: UI parity

Scope renders UI (pager, webtoon, page cards, transition cards, progress indicator). This file lists every visible element, every state, every gesture and the desktop equivalent, plus what the current Kurobayomi mockup lacks. Source paths are repo-relative to `/home/blackbox/aniyomi-src`. Behaviour details are in p1-behavior.md.

Legend for states: L = loading, E = error, Q = queued, D = downloading (with %), R = ready, W = waiting (chapter not started), Ld = loaded, Em = empty.

---

## 1. Page surface (pager and webtoon)

| Element | Pager | Webtoon | Source |
|---|---|---|---|
| Page image | fills the pager; scale per `pref_image_scale_type_key` (six modes) | full width minus side padding; FIT_WIDTH always | B§8.2-8.4, §10.2 |
| Background behind the image | reader background (theme 1 black default, 2 gray, 0 white, 3 automatic from the page edge colours) | reader background | B§1.9, §9.3 |
| Gap between pages | none | 15 dp under each page in "Long strip with gaps" only | B§8.6 |
| Side padding | none | `webtoon_side_padding`% of screen width on both sides (0..25) | B§8.6 |
| Crop borders | per `crop_borders` | per `crop_borders_webtoon` | B§9.4 |
| Dual pages | split into two pages (insert page) or rotate 90° | split and stacked vertically, or rotate | B§9.1-9.2 |

### 1.1 Page states

| State | Pager visual | Webtoon visual | Source |
|---|---|---|---|
| Q (queued) | ReaderProgressIndicator spinning, centred | same, container visible | B§8.1 |
| L (loading) | same as Q | same as Q | B§8.1 |
| D (downloading) | circular indicator set to `progress` 0..100, centred | same, plus progress container | B§8.1 |
| R (ready) | image shown, indicator hidden on image loaded | image shown, progress container hidden on decode | B§8.2, §8.3 |
| E (error) | error layout: text "The image couldn't be loaded", button "Retry", button "Open in WebView" (only when the image URL is set; clickable only for http) | same layout, error height = 0.8 × viewport | B§8.5 |
| No loader | blank (nothing rendered) | blank | B§8.1 |
| No stream while READY | blank, no error | blank, no error | B§8.2 step 2 |

The spinner for Q/L/D is an always-rotating indicator so the reader does not look frozen (`ReaderProgressIndicator.kt:20-24`). Determinate progress is shown as an arc. The `CombinedCircularProgressIndicator` component is in `presentation-core` (not in scope; the visual is a Material 3 circular indicator).

### 1.2 Error layout (`reader_error.xml`)

Vertical, centred, 8 dp margin on each child:
1. Text `decode_image_error` = "The image couldn't be loaded" (`textAppearanceBodyMedium`).
2. Button `action_retry` = "Retry" (`ReaderButton`).
3. Button `action_open_in_web_view` = "Open in WebView" (`gone` by default; made visible when `imageUrl != null`).

Desktop: the same three elements in a `gtk::Box` (vertical, centred, 8 px margins), the button labels from the same keys. "Open in WebView" becomes "Open in browser" only if the string is changed in area:43's catalogue; keep the key `action_open_in_web_view` and its English text for parity in v1.

---

## 2. Chapter transition cards

Card content per B§11.1. Layout: column, `widthIn(max = 460 dp)`, `fillMaxWidth`.

Element order top to bottom for `Prev(from=curr, to=prev)`:
1. Either the previous-chapter block (label "Previous:", chapter name, scanlator) or the notice "There's no previous chapter".
2. 24 dp spacer.
3. Gap warning card (only when the destination exists and the gap is > 0), then 24 dp spacer, then
4. Current-chapter block (label "Current:", name, scanlator) or the notice "There's no previous chapter" when the destination is absent.

For `Next(from=curr, to=next)`: "Finished:" block (curr), 24 dp, optional gap card, 24 dp, "Next:" block (next) or "There's no next chapter".

Note the Kotlin order: when the destination is absent, the notice goes in the top slot for Prev and in the bottom slot for Next (`ChapterTransition.kt:103-140`). Preserve the slot positions.

Chapter block:
- Label line: titleMedium, 4 dp bottom padding.
- Name: titleLarge, 20 sp, max 5 lines, ellipsis. With an inline check icon (22 sp, "Downloaded" description) and a space before the name when downloaded.
- Scanlator: bodySmall, max 2 lines, ellipsis, secondary alpha, 2 dp top padding (omitted when null).

Notice card and gap card: outlined, transparent fill, onSurface text, padding 16 dp horizontal, 12 dp vertical, 16 dp gap between icon and text. Icons: Info (primary) for "no chapter"; Warning (error colour) for the gap.

Plural text: "Skipping %d chapter, either the source is missing it or it has been filtered out" (count 1) and "Skipping %d chapters, either the source is missing them or they have been filtered out" (other).

### 2.1 Adjacent-chapter loading inside the card

Below the chapter blocks, a container shows:
- L: indeterminate spinner, then "Loading pages…".
- E: "Failed to load pages: {message}" then a Retry button → `requestPreloadChapter(to)`.
- W / Ld: nothing (webtoon hides the empty container).

Pager transition holder: 64 dp side padding, centred column. Webtoon transition holder: 128 dp top and bottom, 32 dp sides, 16 dp margin around the status container.

### 2.2 Text colour

Transition text uses `onBackground` (`ReaderTransitionView.kt:56-57`) with `bodySmall` as the base style. On a dark reader background the colour is the light foreground. Desktop: use the palette from `createReaderThemeContext` (B§1.4).

---

## 3. Menu and overlays owned by this area

| Element | Behaviour | Source |
|---|---|---|
| Tap zone overlay (`ReaderNavigationOverlayView`, area:43) | shown once for new users (`reader_navigation_overlay_new_user`, default true, then set false), or on start when `reader_navigation_overlay_on_start` is true; coloured zones with labels Menu / Prev / Next / Left / Right | B§1.1, §2.5 |
| Page action dialog (area:43) | opened by long press on a page (not on transitions) | B§3.2 |
| Menu toggle | centre tap, Menu key | B§2.4, §4.2 |
| Auto-hide on page change | hidden on each page change unless the slider flag is set | B§6.5 |
| Auto-hide on scroll (webtoon) | scroll larger than threshold hides it | B§7.2 |
| Auto-show at book end | pager: selecting terminal Next transition; webtoon: last item is terminal Next | B§6.4, §7.2 |

Desktop note: GTK has no "hide system bars" per viewer; the existing fullscreen logic in area:43 handles that.

---

## 4. Gestures and inputs (desktop equivalents)

| Android gesture | Desktop equivalent in Kurobayomi | Notes |
|---|---|---|
| Tap left/right zones | single click in zone (`GestureClick`, n_press 1) | zones from the active navigation mode |
| Tap centre | single click in centre zone → menu toggle | |
| Double tap (webtoon) | double click (`n_press == 2`) → zoom 2× at the point | pager double tap does nothing (B§3.2) |
| Double tap (pager image) | double click → zoom to 2× fit (SSIV rule) | B§10.5 |
| Double tap and drag (quick scale) | double click, keep button down, move vertically | B§10.4 |
| Long press | press and hold ~500 ms (`gtk-long-press-time`) | page actions; not on transitions |
| Pinch | trackpad pinch (`GestureZoom`) or ctrl + wheel | webtoon and pager image |
| Pan when zoomed | drag | pager image and webtoon strip |
| Fling (zoomed webtoon) | release drag with velocity (`GestureSwipe`) | |
| Horizontal swipe (pager L2R/R2L) | drag horizontally on the pager | commit at 50% or velocity |
| Vertical swipe (pager vertical) | drag vertically | |
| Vertical scroll (webtoon) | wheel, touchpad, drag | native scrolled window behaviour |
| Hardware keys | arrows, PageUp/PageDown, Menu, ctrl+arrows | B§4 |
| Volume keys | XF86AudioRaiseVolume / LowerVolume, only when `reader_volume_keys` is on | B§4, T-44a-015 |

Scroll wheel on the pager: wheel down = next page, wheel up = previous page (B§4.3).

Haptics: none on desktop (T-44a-017).

---

## 5. Animations

| Animation | Duration | Easing | Trigger | Source |
|---|---|---|---|---|
| Page move (pager) | 250 ms (Proposal; ViewPager smooth) | ease out | next/prev when `pref_enable_transitions_key` is on | B§6.3, §15 |
| Page move (pager, jump) | none | — | when the transitions pref is off | B§6.3 |
| Pan before page turn | 250 ms | ease out quad | `navigate_pan` and the page is wider than the view | B§10.4 |
| Double-tap zoom (pager) | `pref_double_tap_anim_speed` (500 ms default) × animator scale | none (SSIV) / ease out | double tap | B§10.5, §12.2 |
| Landscape zoom | 500 ms after a 500 ms delay | ease in-out quad | landscape page selected, `landscape_zoom` on | B§10.3 |
| Webtoon zoom in/out (double tap, reset) | 200 ms | decelerate | double tap | B§10.7 |
| Webtoon fling | 400 ms | decelerate | fling while zoomed | B§10.8 |
| Webtoon smooth scroll (tap, key) | system smooth scroll | — | `pref_enable_transitions_key` on | B§7.4 |
| Spinner | continuous | — | Q, L, D | B§8.1 |

Reduced motion: when GTK's `gtk-enable-animations` is false, every duration above becomes one frame (T-44a-074). This replaces `Settings.Global.ANIMATOR_DURATION_SCALE` (`AnimationExtensions.kt:10-11`).

---

## 6. Responsive behaviour

- Phone-sized Android layouts map to any window size on desktop. Pager pages fill the pager in both axes; the scale mode decides the fit.
- Minimum window size: the viewer draws at any size, but the webtoon side padding percentage is relative to the widget width (proposal) rather than the screen width (observed; B§8.6, §15 open question 6).
- High DPI: all dp values scale with `scale_factor()`. Desktop widths between 800 and 2560 px wide must keep the transition card at its 460 dp max width (B§11.1).
- Orientation: there is no device rotation on desktop. The Kotlin `ORIENTATION_LANDSCAPE` check in `chooseBackground` (B§9.3 step 11) becomes "widget wider than tall".
- Fullscreen: handled by area:43; the viewer fills the reader area.

---

## 7. Empty and boundary states

| State | Visual |
|---|---|
| Chapter with no pages loaded yet (`pages == null`) | pager: transition card only, no page; webtoon: transition card only |
| Chapter with zero pages (edge case, B§12.4 hazard 1) | Rust: transition card only, no crash |
| First page of the book with no previous chapter | "There's no previous chapter" notice at the top of the Prev transition |
| Last page of the book | "There's no next chapter" notice in the Next transition, and the menu is shown |
| Single-page chapter | preload of the next chapter on the transition (B§6.1) |

---

## 8. Current mockup versus this design (what is missing)

Source of truth for the mockup: `src/player.rs` `reader()` (lines 49-147) and `spec/reader_player.json`.

| Area | Mockup today | Required |
|---|---|---|
| Page content | a 420×600 placeholder "cover" widget (`cover(420, 600)`) | real viewer widget (pager or webtoon) with decoded pages |
| Page navigation | `GestureClick` with fixed thirds (left third previous, right third next, centre toggle) | tap zones from the six navigation modes, direction-aware (R2L, vertical), inversion, overlay |
| Chapter boundaries | none | transition cards with "Previous:/Current:/Finished:/Next:" and notices |
| Page states | none | queued/loading/downloading/ready/error with the spinner, arc and error layout |
| Dual pages, rotate, crop, background | toggle "Crop borders" shows a toast only | real processing (B§9) |
| Reading modes | chips in `spec/reader_player.json` ("Default", "Paged (left to right)", "Paged (right to left)", "Paged (vertical)", "Long strip", "Long strip with gaps") | wired to `ViewerKind`; mode change re-creates the viewer |
| Scale type | chips with the six Kotlin labels | wired to `ScaleType`; "Stretch" maps to CENTER_CROP (B§10.2) |
| Zoom start | chips "Automatic / Left / Right / Center" | wired |
| Tap zones chips | six labels matching Kotlin | wired to `nav::layouts` |
| Invert tap zones | chips None/Horizontal/Vertical/Both | wired |
| Background colour | chips "Black, Gray, White, Auto" | matches Kotlin menu order (`SettingsReaderScreen.kt:92-97`); keep this order |
| Webtoon side padding | slider 0 | wired (0..25) |
| Webtoon double-tap zoom and disable zoom out | switches | wired |
| Long-tap page actions | switch "Show actions on long tap" | wired to `reader_long_tap` |
| Animate page transitions | switch | wired to `pref_enable_transitions_key` |
| Always show chapter transition | switch | wired to `always_show_chapter_transition` |
| Page slider (bottom) | `gtk::Scale` 1..20 (placeholder, `pages = 20.0`) | real page count; drives `move_to_page`; sets the slider flag (B§6.5) |
| Page indicator | label "1 / 20" shown when menus hidden | real page number from `PageSelected` (area:43 owns the label) |
| Tap-zone overlay | none | overlay with the five region colours and labels (B§2.5), area:43 renders |
| Volume keys | none | optional, off by default |
| Keyboard | none in the viewer | arrows, PageUp/PageDown, Menu, ctrl+arrows (B§4) |
| Scroll wheel | none | pager page turns, webtoon native scroll |
| Animated images | none | GIF and animated WebP playback (B§9.5) |
| Error page | none | error card with Retry and Open in browser |

The mock's "Background color" option order and the "Reading mode" option list are already correct and should be kept.

---

## 9. Accessibility

- Every icon-only control in area:43's chrome has a tooltip; the viewer adds none (it has no icon-only controls).
- Error and notice text uses the theme's text colours; contrast follows the reader background choice (black, gray, white, automatic).
- Keyboard: every action in §4 is reachable without a pointer (T-44a-084).
- Screen readers: the page image has the accessible label "Page {n} of {total}" (proposal; the Kotlin has none). Transition cards read in order of the slots above.
