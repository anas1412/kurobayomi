# 46 Player controls UI, part 1: UI parity

Every visible element, state, gesture, animation and message of the in-scope Compose files, the Linux equivalent, and what the current mockup (`src/player.rs`, `player()`) lacks. Behaviour numbers refer to `p1-behavior.md`. Task ids refer to `p1-tasks.md`.

Legend for "Mockup": OK = matches; PARTIAL = present with a different value or behaviour; MISSING = absent; WRONG = present and contradicts Aniyomi.

---

## 1. Screen anatomy (player screen, phone-sized to desktop)

Visible layers, bottom to top:

| Layer | Element | Shown when | Mockup |
|---|---|---|---|
| L0 | Video surface (black when idle) | always | PARTIAL: `surface` is a styled empty box (`src/player.rs` `player()`), no video |
| L1 | Gesture surface (invisible) | always | PARTIAL: `GestureClick` only (tap, double tap by thirds, long press) |
| L2 | Double-tap oval (white 20%, half-width) with triangles and seconds text | after a double tap, 800 ms | MISSING |
| L3 | Veil gradient (black top and bottom, 80% max) | controls shown and not locked | MISSING (controls sit on the bare video) |
| L4 | Top bar: back, title block, auto-play, subtitles, audio, quality (online only), more | controls shown and not locked | PARTIAL: quality always shown, no long-press panels distinction |
| L5 | Centre transport: previous, play/pause (96 px), next; spinner while loading; gesture seek text | controls shown or loading or gesture seek | PARTIAL: no spinner, no gesture text, no enable states |
| L6 | Volume pill (left) and brightness pill (right) | after vertical drag, 2 s | MISSING |
| L7 | Player update pill (top 20%) | after an update, 2 s | PARTIAL: `pill()` 1200 ms, no bias |
| L8 | Unlock button (top-left) | locked and controls shown | PARTIAL: unlock button shown when locked, but not gated on tap state |
| L9 | Thumbnail preview card above seekbar | seeking and an image is ready | MISSING |
| L10 | Bottom-left row: lock, rotate, speed, chapter pill | controls shown and not locked | PARTIAL: lock, speed, chapter pill present; rotate missing (Linux drop, T-46a-048) |
| L11 | Bottom-right row: primary button, PiP (if enabled), aspect | controls shown and not locked | PARTIAL: primary button hard-coded "Skip Opening", no PiP gating |
| L12 | Seekbar row: position time, seeker with ticks and read-ahead, duration/remaining time | (controls shown or seekbar shown) and not locked | PARTIAL: `gtk::Scale` with fixed marks 85 and 1330, no read-ahead, no chapter ticks, no invert toggle |
| L13 | Brightness veil (black, alpha = -brightness) | brightness < 0 | MISSING |
| L14 | Sheets (modal, bottom, 420 px / 720 px landscape, 50% scrim) | sheet open | PARTIAL: modal window with spec-driven content (part 2 bodies) |
| L15 | Panels (right card, 420 px, opacity from pref) | panel open | PARTIAL: panel IDs exist in the spec, the player does not show them yet |
| L16 | Dialogs (centred, 80% episode list, 50% integer picker) | dialog open | PARTIAL: `episode_list` dialog exists in spec; integer picker only in spec |

---

## 2. Top bar (TopLeftPlayerControls.kt, TopRightPlayerControls.kt)

### 2.1 Visual specification

| Element | Compose | Linux target | Values |
|---|---|---|---|
| Back icon | `ControlsButton(ArrowBack)` 20 dp, white | `gtk::Button` icon `go-previous-symbolic`, 20 px, white through CSS `color` | padding 16 v / 16 h (ControlsButton default `padding.medium`), circular clip |
| Anime title | 16 sp, `bodyLarge`, single line, ellipsis | `gtk::Label` xalign 0, `ellipsize End`, `max_width_chars` | white |
| Episode title | 14 sp, `bodyLarge`, italic, white 50% | `gtk::Label` `.kuro-dim-italic` | `alpha(white, 0.5)` |
| Title column spacing | -4 dp (overlap) | `gtk::Box` vertical `spacing = -4` not allowed; use margin_bottom -4 on the first label or CSS `margin-bottom: -4px` | |
| Title click | Whole column opens episode list (`showEpisodeListDialog`) | `gtk::GestureClick` on the column, `set_cursor(pointer)` | |
| Auto-play switch | Custom: track 5/6 width, 2/3 height, rounded; thumb icon PlayCircle (on, primary) or PauseCircle (off, white); track secondaryContainer (on) or LightGray (off); thumb animates 250 ms | `gtk::Switch`-like custom widget (proposal: `gtk::ToggleButton` with CSS track and `gtk::Image` thumb moved by tick animator) | size 48 x 24 dp, padding v 16 h 12 |
| Subtitles | `Subtitles` icon, tap sheet, long-press panel | `media-view-subtitles-symbolic`, long press via GestureLongPress/timer | 20 px, horizontal spacing 12 |
| Audio | `Audiotrack` icon, tap sheet, long-press panel | `audio-x-generic-symbolic` | |
| Quality | `HighQuality` icon, visible only if episode is online; tap and long press both open quality sheet | `video-display-symbolic` | |
| More | `MoreVert` icon, tap More sheet, long-press Video filters panel | `view-more-symbolic` | |

Layout: horizontal, spaced evenly (`Arrangement.Absolute.SpaceEvenly`), vertically centred. Gap between buttons is the 12 dp padding plus the button's own padding.

Icon names: Linux equivalents should be the freedesktop symbolic names listed above. The Material → freedesktop mapping is a design choice; the mockup already uses them (`src/player.rs` top loop). Keep.

### 2.2 States

| State | Visual |
|---|---|
| Controls shown | top bar opacity 1, slide in 100 ms (from left or right per element) |
| Controls hidden | top bar opacity 0, slide out 300 ms |
| Locked | top bar hidden, unlock button shown |
| Quality absent (offline episode) | quality button removed, row spacing unchanged (SpaceEvenly re-distributes) |
| Auto-play on / off | thumb left or right with 250 ms move; icon and track colour change |

### 2.3 Mockup comparison

| Item | Aniyomi | Mockup | Status |
|---|---|---|---|
| Quality button shown only when online | yes (TopRight:86) | always (src/player.rs) | WRONG |
| Long-press opens panel for subtitles and audio and more | yes | yes (mockup `(tap, long)` table) but long-press quality opens quality sheet, same as Aniyomi | OK |
| Auto-play text feedback "Auto-play is on/off" | yes | yes (pill text) | OK |
| Auto-play switch visual | custom track and icons | `gtk::Switch` | PARTIAL |
| Title click opens episode list | yes | yes (`titles.connect_clicked` → `dialog("player","episode_list")`) | OK |

---

## 3. Centre transport (MiddlePlayerControls.kt)

### 3.1 Visual specification

| Element | Value |
|---|---|
| Previous | `SkipPrevious` 48 px icon, circular, padding 16, disabled alpha 0.38 when no previous episode |
| Play/pause | 96 px circular touch area, animated play/pause icon (`anim_play_to_pause`; end state = pause icon when playing), padding 16 inside |
| Next | `SkipNext` 48 px, same as previous |
| Spacing | 24 dp between the three |
| Loading (show loading on) | `CircularProgressIndicator` 96 px, replaces play/pause |
| Loading (show loading off) | play/pause still shown |
| Gesture seek | previous and next hidden; centre text "+00:12\n[01:37]" headline medium, bold, black shadow blur 5 |

Linux: `gtk::Spinner` 96 px for loading (Kotlin uses a Material spinner; GTK spinner is the closest), `gtk::Image` animation for play/pause (`gtk::Image` swapping icon names `media-playback-start-symbolic` / `media-playback-pause-symbolic` with a 150 ms cross-fade; no vector morph exists in GTK, see deviation D-7).

### 3.2 States

| State | Visible |
|---|---|
| Controls shown, playing | prev, play(pause icon), next |
| Controls shown, paused | prev, play icon, next |
| Controls hidden, playing | nothing (except loading) |
| Loading, showLoading on | spinner only (prev, next hidden: they are inside the controls-shown AnimatedVisibility) |
| Loading, showLoading off, controls hidden | nothing |
| Gesture seek | text only (prev and next hidden), play/pause hidden while the text shows |
| Previous unavailable | prev greyed, not clickable |

### 3.3 Mockup comparison

| Item | Aniyomi | Mockup | Status |
|---|---|---|---|
| Play/pause morph animation | animated vector drawable `anim_play_to_pause` | icon swap | PARTIAL (D-7) |
| Prev/next disabled alpha | 0.38 | not disabled (no state) | MISSING |
| Spinner while loading | yes | none | MISSING |
| Gesture seek text | "+00:12 [01:37]" | none | MISSING |
| Prev/next hidden during gesture | yes | prev/next visible | MISSING |
| Prev/next toast "Previous Episode not found!" | via changeEpisode (buttons disabled, so unreachable from button) | none | not reachable; keep the message for keyboard shortcut path |

---

## 4. Bottom rows (BottomLeftPlayerControls.kt, BottomRightPlayerControls.kt)

### 4.1 Bottom-left

| Element | Visual | Action | State |
|---|---|---|---|
| Lock | `LockOpen` icon 20 px | lock | always shown when chrome shown |
| Rotate | `ScreenRotation` icon | cycle orientation | Linux: hidden (T-46a-048) |
| Speed | text "1.00x" bodyMedium white, padding 16 | tap cycles +0.25 (wrap at 2.0 → 0.25), long press opens speed sheet | value from mpv `speed` |
| Chapter pill | pill with `Bookmarks` 16 px icon (tertiary-coloured time, ExtraBold), bullet, name bold, background alpha 0.6, radius 25% | tap opens chapters sheet | only when a chapter exists and pref on; slides up or down when chapter changes |

Speed text uses `%.2f` with `x` suffix (`player_speed`). Example: `1.25x`, `2.00x`.

### 4.2 Bottom-right

| Element | Visual | Action | State |
|---|---|---|---|
| Skip intro (filled) | `Button` (Material filled) with text `Skip <type>` (Opening, Ending, Recap, Mixed opening), or `Don't skip` during the Netflix-style countdown | skip to next chapter start | visible when skip text non-null |
| Custom primary (filled) | same style, text = button name | tap = `call_button_<id>`, long press = `_long` | when no skip text and a primary custom button exists |
| PiP | `PictureInPictureAlt` 20 px | Linux: mini-player (T-46a-046) | only if PiP enabled and supported (Linux: if mini-player implemented) |
| Aspect | `AspectRatio` 20 px | cycle Fit → Stretch → Crop → Fit with pill text | always |

Filled buttons: Aniyomi uses `tachiyomi.presentation.core.components.material.Button` (filled, rounded, padding 16 h, text bodyMedium). Linux: `gtk::Button` with `.suggested-action` and `.pill` (mockup already uses `suggested-action pill`, `src/player.rs`).

### 4.3 Mockup comparison

| Item | Aniyomi | Mockup | Status |
|---|---|---|---|
| Speed text | `%.2fx` | `format!("{:.2}x")` | OK |
| Speed cycle rule | +0.25, wrap at >= 2 | same rule | OK |
| Speed persisted | `pref_player_speed` | not persisted | MISSING |
| Chapter pill format | icon, `mm:ss`, bullet, name | "00:00 • Opening" static text | PARTIAL (no icon, no animation, static) |
| Skip button label | "Skip <type>" from chapter | "Skip Opening" static | PARTIAL |
| Skip button absent when no skip | yes | always visible | WRONG |
| PiP gated | yes | always visible | WRONG |
| Aspect cycle with pill text | yes | yes ("Fit to screen", "Stretched to screen", "Cropped to screen") | OK |
| Aspect persisted | `pref_player_aspect_state` | not persisted | MISSING |
| Rotate button | yes | none | OK for Linux (drop) |

---

## 5. Seekbar row (SeekBar.kt, PC:369-411)

### 5.1 Visual specification

Values marked (proposal) are Kurobayomi choices; unmarked values are Aniyomi values or direct equivalents.

| Element | Value |
|---|---|
| Row height | 48 px |
| Position timer | 92 px wide, centred text, white, `mm:ss` or `h:mm:ss`, no interaction |
| Seeker track | height 4 px (proposal), rounded ends, colour `background` (Android) → `@window_bg_color` shade |
| Progress fill | primary colour (Android) → `@accent_color` (GTK theme) |
| Read-ahead fill | `inversePrimary` (Android) → accent at alpha 0.5 |
| Chapter ticks | one tick per chapter (proposal: 2 px wide, 8 px tall, white at alpha 0.6), drawn over the track |
| Thumb | circle 12 px (proposal; Seeker default size not verified), primary colour |
| Duration timer | 92 px wide, centred text; plain total, or inverted `-mm:ss` / `+mm:ss`; click toggles `invert_duration` |
| Spacing | 4 px between timers and the seeker |

### 5.2 States

| State | Display |
|---|---|
| Idle | thumb at playhead, text updates each frame |
| Dragging | thumb follows pointer; position text shows drag value (internal seek position); thumbnail card visible (if image) |
| Gesture seek (horizontal drag over video) | thumb follows gesture value (seekPosition) even though the bar is not dragged; timers show gesture value |
| Seek committed | thumb returns to playhead on the next position update |
| Zero duration (unknown) | slider range 0..0, thumb at 0, duration text "00:00" |
| Chapters absent | no ticks |
| Read-ahead unknown | no read-ahead fill (value 0) |

### 5.3 Mockup comparison

| Item | Aniyomi | Mockup | Status |
|---|---|---|---|
| Slider style | thin track with thumb | `gtk::Scale` default | PARTIAL |
| Chapter ticks | from chapter list (with the duplicate defect in Kotlin) | fixed marks 85 and 1330 | WRONG |
| Read-ahead | yes | no | MISSING |
| Position text | live | live (`fmt_time`) | OK |
| Duration toggle to remaining | yes, persisted | toggles locally, not persisted, mockup shows `-` prefix | PARTIAL |
| Drag preview of thumbnail | yes | no | MISSING |
| Seek on release only | yes (`onValueChangeFinished`) | `value_changed` updates position label only | PARTIAL |

---

## 6. Thumbnail preview (ThumbnailPreview.kt)

| Element | Value | Behaviour |
|---|---|---|
| Card | 220 x 124 dp (or 124 x aspect wide for tall images), radius 8, elevation 8, black | appears on drag, fades in/out (default Compose fade ~300 ms) |
| Image | crop to fill | `ContentScale.Crop` |
| Chapter label | bottom-left, 8 px inset, black 60% pill, 11 sp bold, single line | only when the current chapter name is non-empty |
| Position | centred over the playhead, clamped to 8 px from edges | see behaviour 7.3 |
| Tile cache | 3 tiles | see behaviour 7.6 |

Linux: `gtk::Overlay` child with `gtk::Picture` (`content-fit: cover`), `gtk::Label` for chapter, CSS classes `.kuro-thumb`, `.kuro-thumb-label`. Fade: `set_opacity` ticker over 200 ms.

Mockup: MISSING entirely.

---

## 7. Volume and brightness pills (VerticalSliders.kt, PC:201-290)

### 7.1 Volume pill

| Element | Value |
|---|---|
| Position | left edge (or right when swapped), vertically centred, 16 px margin |
| Boost text (above bar) | "" / "+N" / "-N", white with 1 px black shadow (labelSmall with shadow blur 4) |
| Bar | 120 px tall, 54 px wide box (aspect 0.45), track 50% width rounded 16 px |
| Fill | tertiary (Android) → accent; height animated |
| Overflow fill | errorContainer above the normal fill, height = boost fraction |
| Device text (below bar) | "45%" or integer (display pref) |
| Icon (below text) | VolumeOff, VolumeMute, VolumeDown, VolumeUp per rule |
| Visibility | 2 s after the last vertical drag event, enter from the edge side 100 ms, exit 300 ms |

Linux icons: `audio-volume-muted-symbolic` (Off), `audio-volume-low-symbolic` (Mute), `audio-volume-medium-symbolic` (Down), `audio-volume-high-symbolic` (Up). Mapping by behaviour 8.1 (Off at 0, Mute 1..30, Down 31..60, Up 61..100).

### 7.2 Brightness pill

| Element | Value |
|---|---|
| Position | right edge (or left when swapped), vertical centre, 16 px margin |
| Text above bar | integer percent, e.g. "45" or "-40" (no % sign) |
| Bar | positive 0..1, overflow negative 0..0.75 (errorContainer) |
| Icon | ModeNight (0), BrightnessLow (<=0.3), BrightnessMedium (<=0.6), BrightnessHigh (>0.6) |
| Visibility | 2 s after the last vertical drag event |

Linux icons: `night-light-symbolic` (Night), `display-brightness-low-symbolic`, `display-brightness-medium-symbolic`, `display-brightness-high-symbolic`.

### 7.3 Brightness veil

Full-window black, alpha = -brightness when brightness < 0 (up to 0.75). Input transparent. Always under the panels and sheets.

### 7.4 Mockup comparison

Both pills and the veil are MISSING from the mockup.

---

## 8. Player update pill (PlayerUpdates.kt)

Visual: rounded 16 px, black 40%, padding 8 v 16 h, text white default size (`bodyMedium` ~14 sp), centred, at 20% from top.

Texts that occur in scope: aspect names ("Fit to screen", "Stretched to screen", "Cropped to screen"), "Auto-play is on", "Auto-play is off".

Mockup: `pill()` (src/player.rs:40-47) shows the text at 80 px from top with 1200 ms timeout and `osd title-4` class. WRONG: timeout 2000 ms; position 20% of height, not 80 px; typography `title-4` is larger than body (Aniyomi uses bodyMedium).

---

## 9. Double-tap feedback (GestureHandler.kt, DoubleTapSeekTriangles.kt, OvalBox.kt)

### 9.1 Visual

| Element | Value |
|---|---|
| Oval | half-width (40% of surface), full height, white at alpha 0.2, side by direction (right for forward, left for backward) |
| Oval shape | cubic half-oval per behaviour 3.3 |
| Triangles | three `ic_play_seek_triangle` 16 x 20 px, white, sequential fade 150 ms each, 900 ms cycle, rotated 180 degrees for backward |
| Text | 12 sp white, centred under the triangles: "10 seconds", "-10 seconds", or custom text |
| Lifetime | visible while the accumulator is non-zero or text is set; cleared 800 ms after the last change |

### 9.2 States

| State | Visual |
|---|---|
| Forward accumulate | right oval, text "10 seconds", then "20 seconds" (plural, positive has no plus sign) |
| Backward accumulate | left oval, "-10 seconds" (signed plural text) |
| Reversal | accumulator reset to 0 then new direction |
| Centre (play/pause) | no oval (`handleCenterDoubleTap` does not set the accumulator) |
| Skip-intro text (aniskip) | left oval with text, amount 0 |

Note: the Compose text for forward is `10 seconds` (no plus). Parity: keep no plus sign for forward text; the horizontal drag text uses the `+`/`-` sign format (T-46a-007). This is an inconsistency in Aniyomi, preserved for parity (deviation D-3 would otherwise be needed).

### 9.3 Mockup comparison

| Item | Aniyomi | Mockup | Status |
|---|---|---|---|
| Seek amount | `skip_length_preference` (10) | fixed 10 | PARTIAL |
| Oval feedback | yes | toast "pill" text only | MISSING |
| Triangles | yes | none | MISSING |
| Zones | left below 40%, right above 60%, centre play/pause | left below 40%, right above 60%, centre play/pause | OK (zone rule matches) |
| Accumulation | yes | per-tap 10 s pill ("-10 seconds") | PARTIAL |
| Action per zone from settings | yes | fixed | MISSING |
| Repeated double tap continues accumulating | yes | not | MISSING |

---

## 10. Gestures and mouse/keyboard equivalents

### 10.1 Touch gestures and the desktop equivalent

| Aniyomi gesture | Touch behaviour | Desktop equivalent (Linux) | Notes |
|---|---|---|---|
| Single tap | toggles controls | single left click on the surface toggles controls | identical |
| Double tap left (< 40% width) | backward seek by skip length | double-click left zone | GTK double-click time (default 400 ms) |
| Double tap right (> 60%) | forward seek | double-click right zone | |
| Double tap centre | play/pause | double-click centre | |
| Long press on surface | pause + screenshot sheet | press and hold left button 500 ms | proposal: timer-based (P-3 pitfall) |
| Horizontal drag | seek by 0.15 s per px | left button drag horizontally | drag threshold: GTK default 8 px |
| Vertical drag, left half | brightness | left button drag vertically in left half | identical zones |
| Vertical drag, right half | volume (device, then boost) | left button drag vertically in right half | identical |
| (none) | | mouse wheel over surface | NEW (proposal T-46a-044): wheel up = +1 volume step, wheel down = -1 |
| (none) | | wheel left/right | NEW (proposal): seek by skip length |

Right-click: not used in Aniyomi. Proposal: right click on the surface opens the More sheet (no Aniyomi equivalent; flagged NEW).

### 10.2 Keyboard shortcuts (proposal, T-46a-043)

Aniyomi has no keyboard shortcuts for these controls (Android hardware keys only: volume keys change device volume, handled in area 45). Proposed defaults (NEW, configurable in `player.toml`):

| Key | Action | Equivalent |
|---|---|---|
| Space | play/pause | centre button |
| Left / Right | seek -skip / +skip | double-tap left/right |
| Shift+Left / Shift+Right | seek -60 s / +60 s | NEW |
| Up / Down | volume +1 / -1 step | vertical drag right half |
| Shift+Up / Shift+Down | brightness +0.05 / -0.05 | vertical drag left half |
| Comma / Period | previous / next frame | NEW (mpv `frame-back-step`/`frame-step`) |
| [ / ] | speed -0.25 / +0.25 | speed cycle (no wrap for keyboard) |
| Backspace | speed reset 1.0 | NEW |
| L | lock / unlock | lock button |
| C | toggle controls | single tap |
| A | toggle auto-play | auto-play switch |
| S | subtitles sheet | subtitles button |
| D | audio sheet | audio button |
| Q | quality sheet (online only) | quality button |
| M | more sheet | more button |
| E | episode list dialog | title click |
| P | PiP / mini-player | PiP button |
| R | aspect cycle | aspect button |
| Escape | close top-most sheet, panel, dialog; if none, leave fullscreen; if none, back | BackHandler |
| F | fullscreen toggle | NEW (window action) |
| Ctrl+S | screenshot sheet | long press |

Focus: the player window has the focus; sheets are modal windows and take Escape first.

### 10.3 Accessibility

- Every icon button gets an accessible label (Aniyomi uses `contentDescription = null` everywhere; parity is not the goal here). Labels: "Back", "Auto-play", "Subtitles", "Audio", "Quality", "More", "Lock controls", "Rotate" (hidden), "Playback speed", "Current chapter", "Skip intro", "Picture-in-picture", "Aspect ratio", "Previous episode", "Play", "Pause", "Next episode", "Seek", "Volume", "Brightness".
- Keyboard focus ring on every focusable control (GTK default).

---

## 11. Animations (behaviour section 1.6 and others)

| Animation | Duration | Easing | Linux implementation |
|---|---|---|---|
| Controls enter (slide + fade) | 100 ms | LinearOutSlowIn (cubic-bezier 0, 0, 0.2, 1) | tick animator on `set_margin_*` and `set_opacity` |
| Controls exit (slide + fade) | 300 ms | FastOutSlowIn (cubic-bezier 0.4, 0, 0.2, 1) | same |
| Veil alpha | 300 ms | FastOutSlowIn | opacity ticker |
| Player update pill fade | 100 / 300 ms | as above | opacity ticker |
| Oval alpha | Compose default (tween ~300 ms, verify) | FastOutSlowIn-like | opacity ticker |
| Triangles | 150 ms per step, 900 ms cycle | linear | `timeout_add_local(150)` phase |
| Brightness and volume pill fills | Compose default `animateFloatAsState` (~300 ms) | FastOutSlowIn | tick animator on height |
| Sheet scrim | 350 ms | tween linear | opacity ticker |
| Sheet slide | snap 350 ms | tween linear | revealer 350 ms |
| Panel switch | fade 300 ms plus slide | default | `gtk::Stack` transition 300 ms slide-left |
| Auto-play thumb | 250 ms | tween linear | tick animator on the thumb alignment |
| Chapter pill slide | Compose default | | `gtk::Stack` slide-up 200 ms |
| Thumbnail fade | 200 ms | tween | opacity ticker |
| Expandable card | animate content size | default | `gtk::Revealer` 200 ms |
| Play/pause icon morph | AVD `anim_play_to_pause` | | cross-fade 150 ms (D-7) |

Reduce motion (`pref_reduce_motion`, default false): every slide becomes a fade (behaviour 1.5). Linux: when reduce-motion is set, set transition durations to 0 and keep opacity changes. Proposal: also honour `gtk-enable-animations` (GTK setting) as the system-level equivalent. Deviation D-5.

---

## 12. Dialogs

### 12.1 Episode list

| Element | Visual | State |
|---|---|---|
| Title | "Episodes", titleLarge (~22 sp) | always |
| Dialog size | 80% width, 80% height | |
| Row | bookmark icon (primary when on), fillermark icon (tertiary when on), spacer 2 dp, title (bodyMedium, bold italic if current, dimmed 38% if seen), subtitle (labelMedium: date, dot, scanlator) | |
| Scroll | fast scroller at right (VerticalFastScroller) | |
| Initial scroll | current episode at top | |
| Empty playlist | not defined in Aniyomi (the dialog only opens when anime exists, VM:1209-1215; the playlist can be empty) | Proposal: show "No episodes" label (new key) |
| Current episode missing | Kotlin crashes (E-46-017) | Proposal: no current highlight, scroll to top |

Mockup: `dialog("player", "episode_list")` opens the spec dialog (`spec/reader_player.json` dialog "Episodes"). Its content is spec-driven, not the Aniyomi row design. PARTIAL.

### 12.2 Integer picker

Visual: 50% width, title, a 128 px wheel with the items (formatted by `nameFormat`), no OK/Cancel. Dismiss by Escape or clicking outside commits the selected value (behaviour 10.3).

Linux: `gtk::ListView` in a 50% width modal with 5 visible rows, centred selection, scroll wheel. Commit on close. Deviation D-6: GTK list instead of the Compose wheel (no kinetic wheel).

Mockup: spec dialog `integer_picker` ("Change intro length") exists, content spec-driven. PARTIAL.

### 12.3 Shell

Padding 16 px, large radius, tonal 1 dp, title titleLarge, optional Cancel (left) and OK (right) with SpaceBetween.

---

## 13. Panels and sheets (chrome only; bodies are part 2)

| Element | Visual |
|---|---|
| Sheet container | bottom sheet, 420 px wide (portrait) or 720 (landscape), up to 95% height, top corners rounded (extraLarge), bottom corners square, scrim 50% black |
| Drag handle | none in Aniyomi (the sheet is draggable from anywhere on its surface) |
| Dismiss | scrim tap, Escape, back, drag down past 56 px or fling with 125 px/s |
| Panel card | 420 px max width, `surface` at alpha `panel_opacity / 100`, right side, 16 px inner padding |
| Panel layout | `ConstraintLayout` bias 0.8 (audio/subtitle delay), at bottom-right |

Mockup: sheets exist (`show_sheet`, src/ui.rs:391-405) as spec modal windows; they do not have the drag or the scrim. PARTIAL. Panels: MISSING from player, present in spec list.

---

## 14. Delay card (shared, used by audio delay)

| Element | Visual | State |
|---|---|---|
| Title | "Audio delay" headlineMedium + close icon 32 px | |
| Delay field | "Delay" label, value field, "ms" suffix, - and + repeat buttons (50 ms step) | error text "Value too big/small" only for bounded fields (not used here) |
| Sync buttons | two equal buttons: "Sound heard" / "Sound spotted" | the active one disabled; other one stops |
| Apply / Reset | "Set as default" button and refresh icon (filled) | disabled while syncing |

Syncing visual: while running, value field is the parent value (stale) in Kotlin (E-46-029). Proposal: show live value; parity note.

Mockup: MISSING (no audio delay panel in the player).

---

## 15. Messages and toasts (visible text)

| Message | Trigger | Visual | Mockup |
|---|---|---|---|
| "Auto-play is on" / "Auto-play is off" | auto-play toggle | update pill 2 s | OK (text), timeout differs |
| "Fit to screen" / "Stretched to screen" / "Cropped to screen" | aspect cycle | update pill | OK text |
| "Previous Episode not found!" / "Next Episode not found!" | changeEpisode without neighbour (keyboard path) | toast (Android Toast ~2 s) | MISSING |
| "Skip Intro: Skipping <name> in N seconds" | Netflix-style skip countdown | toast + button "Don't skip" | MISSING |
| "<name> skipped" | auto skip | seek text in oval (amount 0) | MISSING |
| "Skip <Opening|Ending|Recap|Mixed opening>" | skip intro button | filled button | PARTIAL (static "Skip Opening") |
| "Sleep timer ended" | sleep timer end | toast | part 2 |
| "-10 seconds" / "10 seconds" | double-tap accumulator | oval text | PARTIAL ("-10 seconds" pill, no oval) |
| "+00:12 [01:37]" | horizontal drag | centre text | MISSING |

Toast placement: centred near the top (Android Toast default bottom; Aniyomi `showToast` is the system toast). Linux proposal: in-window pill at bottom-centre, 2 s, same class as the update pill, to avoid OS notification stacking. Deviation D-8.

---

## 16. Responsive behaviour

| Window size | Aniyomi behaviour | Linux behaviour |
|---|---|---|
| Phone portrait (~360 dp) | top row fits 5 buttons; bottom-left row fits lock, rotate, speed, chapter; bottom-right fits skip plus PiP plus aspect; seekbar takes remaining width | Desktop minimum window 640 x 360 (proposal): all rows fit; the chapter pill truncates the name with ellipsis before hiding |
| Landscape (phone, ~800 dp) | sheets 720 dp wide; panels 420 dp | Desktop: sheets 720 px; panels 420 px |
| Tablet / desktop | not designed | Desktop default window 1280 x 720; controls keep fixed sizes (no scaling), so text remains 14-16 sp equivalent |
| Narrow window (< 640 px) | n/a | Chapter pill hidden at widths below 560 px; top title block shrinks with ellipsis; volume and brightness pills remain (they are vertical) |
| Fullscreen | Android immersive | GTK fullscreen; overlay chrome unchanged |

Proposal (T-46a-055): use `gtk::Overlay` child size requests and a `gtk::LayoutManager` for the bottom row to collapse the chapter pill first, then the aspect label (none; aspect is an icon).

---

## 17. Mockup gap summary (what `src/player.rs` lacks)

Items MISSING (must be implemented; tasks in brackets):
1. Real video surface (area 45).
2. Veil gradient and opacity animation (T-46a-001, T-46a-022).
3. Spinner while loading and the show-loading preference (T-46a-006).
4. Previous / next disabled state (T-46a-006).
5. Gesture seek centre text (T-46a-007, T-46a-018).
6. Horizontal drag seek (T-46a-018).
7. Vertical drag volume and brightness with pills (T-46a-019, T-46a-021, T-46a-022).
8. Brightness veil for negative values (T-46a-022).
9. Double-tap oval, triangles, accumulator and per-zone actions (T-46a-015, T-46a-016, T-46a-017).
10. Long press pauses (T-46a-020).
11. Thumbnail preview (T-46a-013).
12. Read-ahead and chapter ticks on the seekbar (T-46a-010).
13. Duration/remaining toggle persisted (T-46a-011).
14. Auto-hide timer 4000 ms with pause and seek rules (T-46a-001).
15. Lock state with tap-to-reveal unlock (T-46a-002).
16. Skip-intro logic and labels (T-46a-050).
17. Custom primary button (T-46a-051).
18. Chapter pill animation (T-46a-052).
19. Panels: subtitle settings, subtitle delay, audio delay, video filters host (T-46a-028, T-46a-030).
20. Episode list row design (T-46a-024).
21. Integer picker (T-46a-026).
22. Sheet drag-dismiss and scrim (T-46a-029).
23. Keyboard shortcuts (T-46a-043) and wheel/right-click (T-46a-044).
24. Accessibility labels (T-46a-054).
25. Responsive rules (T-46a-055).
26. Settings persistence for speed and aspect (T-46a-042).

Items WRONG (must change):
1. Quality button always visible: gate on online episode (T-46a-004).
2. PiP always visible: gate on feature (T-46a-009, T-46a-046).
3. Skip button always visible with fixed text: show only when skip text exists (T-46a-050).
4. Pill timeout 1200 ms and position: 2000 ms, 20% from top (T-46a-023).
5. Double-tap fixed 10 s: use `pref_skip_length_preference` and zone actions (T-46a-015).
6. Seekbar fixed marks at 85 and 1330 s: use chapters (T-46a-010).

Items PARTIAL: see the tables above.

---

## 18. Deviations from Aniyomi (intentional, listed for review)

| ID | Deviation | Reason | Task |
|---|---|---|---|
| D-1 | Gesture drag starts at press point (no lost touch-slop distance) | GTK drag semantics | T-46a-018 |
| D-2 | Drag `unpause` no-op removed | Kotlin no-op, quirk | T-46a-018 |
| D-3 | Forward double-tap text has no plus sign (kept); drag text uses sign (kept) | Parity (Aniyomi inconsistency) | T-46a-016 |
| D-4 | Long-press release does not restore speed | Aniyomi quirk (E-46-011) | T-46a-020 |
| D-5 | Reduce motion also honours GTK `gtk-enable-animations` | Linux system preference | T-46a-041 |
| D-6 | Integer picker uses a list, not a wheel | GTK has no wheel picker | T-46a-026 |
| D-7 | Play/pause icon cross-fades instead of morphing | No AVD equivalent in GTK | T-46a-053 |
| D-8 | Toasts are in-window pills, not OS toasts | Avoid desktop notification clutter; consistent with mockup | T-46a-023 |
| D-9 | Percent formatting uses Rust `format!`, not locale-aware `NumberFormat` | No locale NumberFormat in std; difference only for locales with space before % | T-46a-021 |
| D-10 | Episode number `0.5` shows `0.5`, not `.5` | Aniyomi DecimalFormat quirk | T-46a-024 |
| D-11 | `IntegerPicker` commits the displayed item | Fixes E-46-027 | T-46a-026 |
| D-12 | Repeat button minimum 5 ms with GLib jitter | Platform timer granularity | T-46a-033 |
| D-13 | Speed/aspect/auto-play values persisted | Mockup gap (the Kotlin persists them too) | T-46a-042 |
| D-14 | Keyboard shortcuts and wheel/right-click are new | Desktop input | T-46a-043, T-46a-044 |
| D-15 | Rotate button removed on Linux | No orientation API | T-46a-048 |
| D-16 | PiP replaced by mini-player window | No system PiP on Linux desktops in GTK | T-46a-046 |
| D-17 | Haptic feedback removed | No haptics on desktop | T-46a-045 |
| D-18 | Ripple effect replaced by CSS hover/active states | GTK style system | T-46a-057 |
| D-19 | Sheets and dialogs are modal windows; scrim in the player window | Stacking and focus on Wayland/X11 | T-46a-029 |
| D-20 | Brightness: hardware where writable, software veil otherwise | No per-window brightness | T-46a-049 |

---

## 19. Verification checklist (manual, on the target machine)

1. Single tap toggles chrome; the chrome hides 4 s after the last control click while playing; stays visible while paused.
2. Locking hides chrome; tapping reveals the unlock button; unlock restores chrome.
3. Double-click left/right seeks by 10 s and accumulates; reversing direction resets.
4. Horizontal drag shows "+mm:ss [mm:ss]" text; release seeks to the target.
5. Vertical drag on the left changes brightness; at 0 further upward drag shows the veil (negative values).
6. Vertical drag on the right changes volume; with boost cap 30 and volume at max, continuing up shows "+N" and the mpv volume goes to 130.
7. Speed cycle 1.00 → 1.25 → … → 2.00 → 0.25.
8. Aspect cycle Fit → Stretch → Crop → Fit with the matching pill text.
9. Episode title opens the list; current episode is bold italic and scrolled into view; bookmark toggles persist.
10. Seekbar shows chapter ticks once each; drag shows thumbnail when the source provides tiles.
