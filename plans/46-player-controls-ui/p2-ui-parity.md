# 46-player-controls-ui (part 2 of 2): UI parity

This file covers the visible UI of the 14 scope files (sheets and panels of the anime player). Every element, state, dialog, menu, gesture and animation is listed with its Linux/GTK equivalent. Behavior rules are in `p2-behavior.md`; widget mapping is in `p2-rust-design.md`. Icon names are GTK icon-theme names and must be checked against the user's theme (KDE with GTK apps uses Breeze names where available; Adwaita names are the fallback).

Key:
- **Kotlin** = Android/Compose behavior (OBSERVED).
- **Desktop** = Linux equivalent (Proposal).
- Visible text uses the English values from `p2-data-and-config.md` §6.

---

## 0. Global presentation

| Aspect | Kotlin (observed) | Desktop (proposal) |
|---|---|---|
| Container | `PlayerSheet`: bottom sheet, `extraLarge` top corners, zero bottom corners, `tonalElevation 1.dp` (`app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt`) | Card at the bottom of the player overlay: 12 px top corner radius, 0 bottom radius, background = window surface with `tonalElevation` mapped to a subtle shadow (`box-shadow: 0 -2px 8px rgba(0,0,0,.25)`) |
| Width | 420 dp portrait, 720 dp landscape | 420 px default; 720 px when the window is ≥ 900 px wide |
| Max height | 95% of screen height | 95% of player height, content scrolls inside |
| Scrim | black at 50% alpha, 350 ms | `rgba(0,0,0,0.5)` with 350 ms `Revealer` fade |
| Enter / exit | slide-up animated spring, `tween(350)` | `gtk::Revealer` `slide-up` 350 ms |
| Close paths | drag down past 56 dp or fling > 125 dp/s; scrim tap; Back; external dismiss | scrim click; Escape; close button in the header of sheets that have one; the same external dismiss call (`SheetHost::close`) |
| Safe area | `windowInsetsPadding(systemBars top + horizontal)` | not needed on desktop; respect the window decorations |
| Panels (`Panels.*`) | same container; cards aligned at 80% height, 420 dp max width | same host; card right-aligned, vertically at 80% of available height |
| Focus | Compose focus | on open, focus the first actionable control (the first chip or the header close button); restore focus to the opener on close |
| Keyboard | Back button only | Escape closes; Tab/Shift+Tab move focus; Enter/Space activate; arrow keys change sliders (see per-control notes) |
| Mouse | tap | single click activates; hover shows the tooltip where a tooltip is listed |
| Haptics | `HapticFeedback.TextHandleMove` on slider changes (`SliderItem.kt`) | none (dropped; no desktop equivalent) |

Density: Kotlin `MaterialTheme.padding` tokens (`presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:14-25`): extraSmall 4 dp, small 8 dp, mediumSmall 12 dp, medium 16 dp, large 24 dp, extraLarge 32 dp. Desktop uses the same numbers as px.

---

## 1. Track sheets (shared skeleton)

### 1.1 Visual structure (applies to Audio, Subtitles, Chapters)

1. Title row: headline text (`headlineMedium`, the desktop uses `title-2` CSS class), left-aligned; right-side actions (text buttons with icon; see per sheet).
2. Optional `AddTrackRow` (height 48 px, `+` icon 32 px, label, full-row button).
3. List rows, each 48 px high minimum; text may wrap to one line with ellipsis (chapter names are `maxLines = 1`).
4. Optional footer (subtitles only).

### 1.2 States

| State | Kotlin | Desktop |
|---|---|---|
| Empty track list (no audio tracks) | empty `LazyColumn`; header still shows (observed) | same; show the header and the add row. Proposal: no empty-state text added (parity) |
| Selected row | italic + ExtraBold text (`FontWeight.ExtraBold`, `FontStyle.Italic`) | CSS class `selected-track`: `font-style: italic; font-weight: 800` |
| Off track (id -1) | row label `Off` (from `MR.strings.off`) | row label `Off`, selectable |

### 1.3 Audio sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/AudioTracksSheet.kt`)

- Header title: `Audio` (`pref_player_audio`).
- Header actions: `TextButton` with icon `MoreTime` and label `Delay`. Opens the audio delay panel. Desktop icon: `document-open-recent-symbolic` (proposal) or text only.
- Add row: `+` and `Add external audio tracks`. Opens a file picker (`gtk::FileDialog::open`, title `Add external audio tracks`, no filter). Cancel does nothing.
- Rows: radio circle (`RadioButton`) and label `getTrackTitle`. Clicking a row selects it.
- Desktop: `gtk::CheckButton` group (radio) inside a `gtk::ListBoxRow`.

### 1.4 Subtitles sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/SubtitleTracksSheet.kt`)

- Header title: `Subtitles` (`pref_player_subtitle`).
- Header actions: `Palette` (text: `Palette`) → subtitle settings; `MoreTime` (text: `Delay`) → subtitle delay panel.
- Add row: `Add external subtitles`. File picker, no filter.
- Rows: checkbox (`Checkbox`) and label. Selected rows: italic + ExtraBold, and the `#1` / `#2` suffix at the end of the row (primary = `#1`, secondary = `#2`).
- Footer: info icon (`Icons.Outlined.Info`, desktop `dialog-information-symbolic`) and text `Secondary subtitles will have no ASS/SSA Styling.`

### 1.5 Chapters sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ChaptersSheet.kt`)

- Header title: `Chapters`. No actions.
- Rows: left label `#N: <name>` (ellipsis, one line), right timestamp `prettyTime`. Current chapter in italic + ExtraBold.
- Row click seeks.
- For long lists (> 200 chapters) use `gtk::ListView`. Scrolling to the current chapter on open is **not** in Kotlin (observed); desktop proposal: scroll to the current chapter on open (improvement, flagged).

---

## 2. Quality sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/QualitySheet.kt`)

### 2.1 Elements

- Title: `Qualities` (`player_sheets_qualities_title`), headline style, padding top 16 px, start 16 px, bottom 4 px (tokens medium / medium / extraSmall, §0).
- Loading block: centered spinner (`CircularProgressIndicator`, default size) with vertical padding. Fade + slide in/out.
- Flat list (no hoster list): video rows only.
- Hoster list (grouped): hoster rows, each with the expansion arrow and an expandable block of video rows.

### 2.2 Hoster row (`HosterTrack`)

| `HosterState` | Left | Right | Click |
|---|---|---|---|
| Idle | name (bodyMedium, ellipsis, 1 line) | `Tap to load videos` (dimmed, alpha 0.38) | loads the hoster (part 1) |
| Loading | name | 24 px spinner, 2 px stroke | none (observed: still clickable; desktop proposal: ignore clicks while loading) |
| Error | name | `Failed to load videos` (dimmed) + red `ErrorOutline` icon | click calls `onClickHoster`; the loader (part 1) decides whether to retry. No separate error view (parity) |
| Ready | name | `1 video` / `N videos` (dimmed) + `KeyboardArrowUp` when expanded, `KeyboardArrowDown` when collapsed | toggles expansion |

- Row height: 32 px (`height(32.dp)`). Desktop: 32 px min height, same text sizes.
- Selected hoster: italic + ExtraBold name.
- Expansion animation: `expandVertically` / `shrinkVertically` on the video block. Desktop: `gtk::Revealer` with `slide-down`, 200 ms (proposal; Kotlin uses default durations).

### 2.3 Video row (`VideoTrack` composable, named `VideoRow` in Rust)

- Layout order depends on `noHoster`: flat list = text then icon; hoster list = icon then text.
- Text: `video.videoTitle`, bodyMedium, `maxLines = 6`, ellipsis. Selected: italic, ExtraBold, color = primary.
- State icon (`VideoIcon`): `QUEUE` / `READY` show nothing; `LOAD_VIDEO` shows a 2 px spinner filling a 28 px (flat) / 20 px (grouped) box; `ERROR` shows red `ErrorOutline` (fill width).
- Vertical padding: 8 px (flat) / 4 px (grouped).

### 2.4 Groups and display flags

- Order: valid hosters → failed hosters (only if `displayHosters.first`) → empty hosters (only if `displayHosters.second`).
- Empty result: no text (observed). Desktop proposal: no new text.

### 2.5 Animations

- Spinner ↔ list: `fadeIn + slideInVertically(initialOffsetY = it/2)` and `fadeOut + slideOutVertically(targetOffsetY = it/2)` (Compose). Desktop: `gtk::Stack` with `crossfade` 200 ms.

---

## 3. Playback speed sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/PlaybackSpeedSheet.kt`)

### 3.1 Elements (top to bottom)

1. Speed slider row: label `Speed`, value text `%.2fx` (for example `1.00x`), slider 0.01 to 6.00 (continuous).
2. Presets row:
   - Icon button `RestartAlt` (filled tonal): resets presets to the 12 defaults.
   - Scrolling chip list (`LazyRow`, weight 1): each chip `%.2fx`, selected when equal to the current speed, close icon at the end removes the chip.
   - Icon button `Add` (filled tonal): adds the current speed (2 decimals) as a preset.
3. Pitch correction switch row: title `Enable audio pitch correction`, summary `Prevents the audio from becoming high-pitched at faster speeds and low-pitched at slower speeds`.
4. Action row: button `Make default speed` (weight 1), icon button `RestartAlt` (filled) resets the speed to 1.0 and deletes the stored default.

### 3.2 Desktop differences

- Slider: `gtk::Scale` with `set_round_digits(-1)`; arrow keys step by 0.05 (proposal, keeps precision reasonable); Shift+arrow 0.01 (proposal). Kotlin has no keyboard steps (slider only).
- Chips: `gtk::ToggleButton` for selection; remove button is a separate `gtk::Button` inside the chip so it can be focused.
- Scroll: a horizontal `gtk::ScrolledWindow` with overlay scroll bars. The mouse wheel scrolls the chip list horizontally.

### 3.3 Edge UI

- Preset chip for a speed that is not in the set: nothing shows (only presets are chips).
- Current speed `1.2345`: no chip is selected (observed; rounded preset vs exact). Desktop proposal: highlight the nearest preset within 0.005 (improvement, flagged).

---

## 4. More sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/MoreSheet.kt`)

### 4.1 Layout (top to bottom)

1. Header row: title `More` (`headlineMedium`), right side: `Sleep Timer` / `<remaining> remaining` text button with `Timer` icon, and `Filters` text button with `Tune` icon.
2. `Hardware decoding mode` label, then a chip row: `Auto`, `SW`, `HW`, `HW+` (in that order; the first chip is the `AutoCopy` value `auto-copy`, the Kotlin `Auto` value is hidden). Selected chip = current decoder.
3. `Default statistics page` label, then chips `Off`, `Page 1` … `Page 5`. Selected chip = saved page.
4. `Custom buttons` (only if at least one button exists): a wrapping flow of chips. Tap = `call_button_<id>`, long press = `call_button_<id>_long`.
5. `Audio channels` label, then chips: `Auto`, `Auto-safe`, `Mono`, `Stereo`, `Reverse stereo`. Selected = stored value (default `Auto-safe`).

### 4.2 Desktop differences

- Chips: `gtk::ToggleButton` in `gtk::FlowBox` (wrap) so the layout works at 420 px.
- Long press on custom buttons: GTK long-press gesture for touch and a secondary (right) click for mouse (proposal: both; Kotlin has long press only).
- The sleep timer label keeps the same text; the dialog is described in §6.

### 4.3 States

- No custom buttons: section hidden (Kotlin).
- Decoder change failure: Kotlin ignores failures. Desktop proposal: if mpv rejects `hwdec`, keep the previous chip selected and show nothing (no new text; parity).

---

## 5. Screenshot sheet (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/sheets/ScreenshotSheet.kt`)

### 5.1 Elements

- Top row of five action tiles (weight 1 each, icon above label): `Set as cover` (`Photo`), `Set as background` (`Photo`), `Set as thumbnail` (`Photo`, local source only), `Share` (`Share`), `Save` (`Save`). On a non-local source there are four tiles.
- `Include Subtitles` switch (only when the video has subtitle tracks), label style: `labelLarge`, primary color.
- Confirm dialog (for the three set-as actions): title `Use this image as cover art?` (the same text for all three types, observed), buttons OK (confirms) and Cancel (dismisses), width 60% of the window.

### 5.2 Desktop differences

- Tiles become a horizontal `gtk::Box` of `gtk::Button`s with icon and label, using the same five labels.
- Share: `ashpd` OpenURI to the default image viewer or a file manager "reveal" (proposal). Kotlin shares through an Android chooser.
- Save: `gtk::FileDialog::save` with default name `<title>_<seconds>.png` (proposal; area:43 naming rules to match).
- Confirm dialog: `gtk::AlertDialog` (GTK 4.10) with the same title and buttons OK / Cancel. Proposal: change the text per type, for example `Use this image as background?`, flagged as a deviation. Needs two new message keys; not added here.

### 5.3 Error state

- Kotlin: `takeScreenshot(...)!!` throws if mpv wrote no file (crash). Desktop: show a toast-like `gtk::Label` (`Screenshot failed`) inside the sheet for 3 s. **New string required** (`player_screenshot_failed`, proposal, not in the Kotlin i18n).
- Set-as-art failure: `SetAsArt.Error` and `SetAsArt.AddToLibraryFirst` are reported by part 1 (not in this sheet).

---

## 6. Sleep timer dialog (`MoreSheet.kt`, `TimePickerDialog`)

### 6.1 Kotlin elements

- Title: `Enter duration` (input mode) or `Pick duration` (clock mode).
- Toggle icon button: `Schedule` (outlined) in input mode, `KeyboardAlt` in clock mode.
- Input mode: `TimeInput` (hour and minute fields). Clock mode: `TimePicker` dial. 24-hour format.
- Buttons row: `Cancel current timer` (only when a timer runs), then `OK`.
- Width: `IntrinsicSize.Max` (content width), `usePlatformDefaultWidth = false`.

### 6.2 Desktop

- Window: modal, transient for the player window, title `Enter duration`.
- Inputs: two `gtk::SpinButton` (hours 0 to 99 wrap = false; minutes 0 to 59 wrap = true), labelled `Hours` and `Minutes` (new labels; proposal, not in Kotlin, since the Kotlin TimeInput has no visible labels).
- Clock dial: **dropped for v1** (a dial requires custom drawing; the spin-button form covers the same range). Reason: the dial is a secondary input mode; the user loses nothing functional. Revisit if a dial is wanted.
- Buttons: `Cancel current timer` (only when running), `OK`.
- Remaining label in the More sheet: `<MM:SS or H:MM:SS> remaining`.

---

## 7. Subtitle settings container (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsPanel.kt`)

### 7.1 Kotlin

- Portrait: top app bar with back arrow (`AutoMirrored.ArrowBack`), title `Subtitles settings` (headline with shadow blur 20), transparent container. Horizontal pager of three cards (90% of screen width each), one card visible at a time, swipe to change.
- Landscape: title with a close icon (`Close`), the three cards stacked in a vertical scroll column, card width 420 dp.

### 7.2 Desktop

- One layout: header row (title + close button), then a `gtk::StackSwitcher` with three pages: `Typography`, `Colors`, `Miscellaneous`. The switcher replaces the swipe pager (swipe on touch screens still works through the stack's gesture).
- Title: `Subtitles settings`. Close: header button, Escape.
- Pages are scrollable (`gtk::ScrolledWindow`).

---

## 8. Typography card (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsTypographyCard.kt`)

### 8.1 Elements (top to bottom)

1. Expander header: icon `FormatColorText`, title `Typography`.
2. Toolbar row (horizontal scroll): toggle `FormatBold` (bold), toggle `FormatItalic` (italic), three toggles `FormatAlignLeft` / `FormatAlignCenter` / `FormatAlignRight` (justification), spacer, text button `FormatClear` + `Reset`.
3. Font row: icon `brand family` (drawable `outline_brand_family_24`), dropdown labelled `Font`, value = current font, options = `Sans Serif` plus discovered families; leading spinner while the scan runs.
4. Font size slider: label `Font size`, value text, range 1 to 100, icon `FormatSize`.
5. Border style row: clickable row with icon `BorderStyle`, label `Border style`, current style name below; opens a menu of three styles with a check mark on the current one.
6. Border size slider: label `Border size`, range 0 to 100, icon `BorderColor`.
7. Shadow offset slider: label `Shadow offset`, range 0 to 100, icon = shadow drawable (`sharp_shadow_24`).

### 8.2 Desktop

- Toolbar: `gtk::ToggleButton` for bold, italic, the three justify buttons; they form a `linked` box. Icon names (proposal): `format-text-bold-symbolic`, `format-text-italic-symbolic`, `format-justify-left-symbolic`, `format-justify-center-symbolic`, `format-justify-right-symbolic`. Note the Kotlin "unchecking" bug (behavior §13.3): the desktop toggle must reflect the actual state after a click.
- Font dropdown: `gtk::DropDown` with `gtk::StringList`; spinner in a `gtk::Box` suffix while loading.
- Sliders: `gtk::Scale` with integer digits.
- Border style menu: `gtk::DropDown` (three entries) instead of a popover menu.

### 8.3 States

- Font scan in progress: spinner visible; list shows `Sans Serif` only (observed: the list grows when the scan ends).
- Font directory missing: no spinner, list shows `Sans Serif` (observed).

---

## 9. Colors card (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsColorsCard.kt`)

### 9.1 Elements

1. Expander header: icon `Palette`, title `Colors`.
2. Toggle row (horizontal scroll): three toggles `FormatColorText` (Text), `BorderColor` (Border), `FormatColorFill` (Background); label of the current target; reset text button with `FormatColorReset` icon and `Reset`.
3. Four sliders: `Red` (tint red), `Green` (tint green), `Blue` (tint blue), `Alpha` (tint white); values 0 to 255; value text next to the label.

### 9.2 Desktop

- Option A (parity): sliders only. Option B (proposal): add `gtk::ColorDialogButton` above the sliders; choosing a color updates the four sliders. Recommended: both, sliders stay the source of truth.
- Tint colors: red, green, blue, and white for alpha (CSS classes on the scale highlight).
- Reset: restores the default color for the chosen target (white / black / transparent).

### 9.3 Behavior notes for the UI

- Each slider change writes preferences and mpv immediately (observed). Desktop: writes coalesced at most every 50 ms (proposal).
- Alpha `0` (transparent text) is valid and hides text (observed; no guard).

---

## 10. Miscellaneous card (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleSettingsMiscellaneousCard.kt`)

### 10.1 Elements

1. Expander header: icon `Tune`, title `Miscellaneous`.
2. Switch row: `Override ASS/SSA subtitles`.
3. Slider `Scale`: icon `FormatSize`, value text with 2 decimals, range 0.00 to 5.00 (continuous).
4. Slider `Position`: icon `AlignVerticalCenter`, value text integer, range 0 to 150.
5. Reset text button with `EditOff` icon and `Reset`, right-aligned.

### 10.2 Desktop

- Scale slider: `gtk::Scale` with 2 digits. Value 0 hides subtitles: show a dim hint label `Subtitles are hidden at scale 0` (proposal; new string).
- Position: integer slider with `set_round_digits(0)`.

---

## 11. Subtitle delay panel (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/SubtitleDelayPanel.kt`)

### 11.1 Elements

1. Header row: title `Subtitle delay` (headline), then a clickable target selector showing `Primary` / `Secondary` / `Both` with a drop-down arrow, spacer, close icon (32 px).
2. Delay control (`OutlinedNumericChooser`): minus icon button (`RemoveCircle`), text field with label `Delay` and suffix `ms`, plus icon button (`AddCircle`). Step 50 ms. No clamping; out-of-range shows `Value too big` / `Value too small`.
3. Speed control (only when target = Primary): numeric chooser labelled `Speed`, step 0.01, range 0.10 to 10.00. Error text as above.
4. Tap-sync group (`DelayCard`):
   - Button `Voice heard` and button `Text seen` (subtitle), side by side, each weight 1.
   - While a run is active, only the other button is enabled (behavior §11.5).
5. Action row: button `Set as default` (weight 1) and filled icon button `Refresh` (reset). Both disabled while a run is active.

### 11.2 Desktop

- Delay field: `gtk::SpinButton` with step 50, digits 0, unclamped (see p2-rust-design §4).
- Keyboard: Up/Down on the field change by the step (GTK default for SpinButton).
- Tap-sync buttons: `gtk::Button`; while running, the active run's button label shows `Stop` (proposal; Kotlin keeps the label; flagged).
- Target selector: `gtk::DropDown` with `Primary`, `Secondary`, `Both`.

### 11.3 States

- Idle: both tap-sync buttons enabled; `Set as default` and `Refresh` enabled.
- Running: one tap-sync button disabled; `Set as default` and `Refresh` disabled; the numeric field keeps the value from the start of the run (Kotlin) — the desktop proposal shows the live value (flagged in p2-behavior.md §11.5).
- Secondary target: speed control hidden (Kotlin).

---

## 12. Audio delay panel (part 1 file; this part supplies `DelayCard`)

`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt` reuses `DelayCard` with `DelayType.Audio`. Its buttons are `Sound heard` and `Sound spotted`. The title is `Audio delay` (`player_sheets_audio_delay_title`) and it has no target or speed control. Behavior of the shared card is in §11.5 of the behavior spec.

---

## 13. Video filters panel (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/VideoFiltersPanel.kt`)

### 13.1 Elements

- Header: title `Filters` (headline), right side: text button `Reset` and a close control (`ControlsButton` with `Close` icon).
- Five sliders in order: `Brightness`, `Saturation`, `Contrast`, `Gamma`, `Hue`, each with label, value text (integer), range -100 to 100.
- Warning block (if `gpu-next` is off): info icon and text `Some filters may not work on your current video driver`.

### 13.2 Desktop

- Sliders: `gtk::Scale` range -100..100 with digits 0; keyboard arrows step by 1.
- Warning: shown when the active video output is not `gpu-next` (desktop proposal: check the `vo` property; Kotlin checks a preference). Text unchanged.

---

## 14. Panel and sheet common controls

| Control | Kotlin visual | Desktop |
|---|---|---|
| Close icon button | `Close` icon (32 px in the subtitle delay title, default size elsewhere) | `window-close-symbolic`, flat button |
| Expander card header | icon + title row, chevron to expand/collapse | `gtk::Expander` with icon + label as the title widget |
| Text button with icon | `TextButton` + `Row(icon, text)` | `gtk::Button` with `gtk::Box` (icon + label), flat |
| Filled icon button | `FilledIconButton` / `FilledTonalIconButton` | `gtk::Button` with the default style and the `circular` class (proposal) |
| Chip | `FilterChip` (selected, check) / `InputChip` (trailing close) | `gtk::ToggleButton` + class `chip`; input chip = box with close button |
| Switch row | `SwitchPreference` (`toggleable` whole row) | `gtk::Box` row with `gtk::Switch` at the end; whole row clickable |
| Dropdown | `DropdownMenu` / `ExposedTextDropDownMenu` | `gtk::DropDown` |
| Number field | `OutlinedNumericChooser` | `gtk::SpinButton` (unclamped via range) with +/- buttons provided by SpinButton |

---

## 15. Screen-size behavior

| Width | Behavior |
|---|---|
| < 420 px | Sheet and panel cards take the full width minus 16 px gutters on each side |
| 420 to 899 px | Card width 420 px, centered on the bottom edge (sheets) or right edge (panels) |
| ≥ 900 px | Card width 720 px (sheets, matches Kotlin landscape), panels still 420 px |
| Height < 600 px | Sheet max height 95% of player height; content scrolls |

The Kotlin orientation check (`ORIENTATION_LANDSCAPE` → 720 dp) becomes a width breakpoint. The subtitle settings panel's portrait pager vs landscape stack is replaced by a single stack layout (§7.2).

---

## 16. What the existing mockup lacks (gap list)

Source of the mockup: `spec/reader_player.json` (screen `player`), `src/player.rs`, `src/ui.rs`.

### 16.1 Structure

1. Sheets are modal windows (`src/ui.rs:391-404`, `modal(&title, &stack, 520, 620)`), not an overlay inside the player. Kotlin: in-player bottom sheet with scrim. Gap: need `SheetHost`.
2. Panels (`panel_subtitle_settings`, `panel_subtitle_delay`, `panel_audio_delay`, `panel_video_filters`) are spec sheets. Kotlin: cards positioned at 80% height, no tabs for delay/video filters. Gap: `PanelKind` placement.
3. No Escape handling or scrim click (mockup uses window close).

### 16.2 Controls and content

4. `subtitle_tracks` has `multi:Tracks=[]` (a multi-select list). Kotlin: checkbox rows with `#1`/`#2` slot semantics (two slots, behavior §3.2). Gap: slot model and `#N` suffix.
5. `subtitle_tracks` action labels `Palette` and `Delay` match. The footer text matches.
6. `audio_tracks` uses `list:Tracks` with `#1: Japanese (jpn)`. Kotlin: radio rows, `Off` row, label rules (behavior §1.4). Gap: radio semantics and `Off`.
7. `quality_tracks` uses `info:3 videos` / `list:Videos=1080p` / `info:Tap to load videos` / `info:Failed to load videos`. Kotlin: hoster rows with state icons, expansion arrows, per-video state icons (spinner, error), flat list when no hoster list. Gap: `HosterState` model, expansion, flat mode.
8. `chapters` shows one list row `#1: Intro 00:00`. Kotlin: rows for all chapters, current chapter highlighted, click seeks. Gap: data binding and selection.
9. `playback_speed` labels: slider `Speed`, chips `1.00x`, actions `Reset presets`, `Add current speed as preset`, `Make default speed`, `Reset speed`. Kotlin: the reset-presets and add buttons are **icon-only** buttons (`RestartAlt`, `Add`), not text. Gap: icon semantics and tooltips (proposal: tooltips with the same labels).
10. `playback_speed` slider range is not specified in the mockup. Kotlin: 0.01 to 6.00. Gap: range.
11. `more` sheet shows `action:Sleep Timer`, `action:Filters`, `chips:Hardware decoding mode=Auto`, `chips:Default statistics page=Off`, `chips:Custom buttons`, `chips:Audio channels=Auto-safe`. Kotlin: decoder chips are `Auto-copy` (titled "Auto"), `SW`, `HW`, `HW+` and **no** `Auto` chip. The mockup default `Auto` cannot be selected in Kotlin. Gap: decoder chip list and the Linux mapping.
12. `more` statistics chips: Kotlin has six chips (`Off`, `Page 1` … `Page 5`). Mockup shows `Off` only. Gap: chips 1..5.
13. `more` audio channel chips: Kotlin has five chips (`Auto`, `Auto-safe`, `Mono`, `Stereo`, `Reverse stereo`); mockup shows the default only. Gap: full list and the swapped mpv mapping (behavior §7.3).
14. `screenshot` labels match: `Set as cover`, `Set as background`, `Set as thumbnail`, `Share`, `Save`, `Include Subtitles`. Gap: mockup always shows the thumbnail tile (Kotlin shows it only for local sources).
15. `panel_subtitle_settings` Typography: `chips:Justification` (Kotlin: three toggles, not chips). `list:Font=Sans Serif`, `slider:Font size=55`, `list:Border style=Outline and shadow`, `slider:Border size=3`, `slider:Shadow offset=0`. Gap: font list from fonts directory; justify toggle semantics.
16. `panel_subtitle_settings` Colors: `chips:Color target=Text`, `slider:Red=255`, `slider:Green=255`, `slider:Blue=255`, `slider:Alpha=255` Kotlin: toggles (not chips) for the target. Gap: toggle semantics.
17. `panel_subtitle_settings` Misc: `slider:Scale=1`, `slider:Position=100`. Kotlin: Scale range 0..5 and Position 0..150. Gap: ranges.
18. `panel_subtitle_delay`: `slider:Delay=0` and `slider:Speed=1`. Kotlin: `OutlinedNumericChooser` (text field with +/- buttons, step 50 ms, unclamped), not sliders. Gap: control type.
19. `panel_subtitle_delay`: `list:Subtitle=Primary` (Kotlin: `Primary` / `Secondary` / `Both`). Gap: `Both` option, verify in spec.
20. `panel_subtitle_delay`: buttons `Voice heard` and `Text seen`, `Set as default`, `Reset`. Matches. Gap: the enabled/disabled truth table (behavior §11.5).
21. `panel_audio_delay`: `slider:Delay=0` vs numeric chooser (same as 18). Buttons `Sound heard`, `Sound spotted` match.
22. `panel_video_filters`: `info:Some filters may not work on your current video driver` is always shown. Kotlin: only when `gpu-next` is off. Gap: conditional warning.
23. `sleep_timer` dialog: text `Hours : Minutes=00:00`, toggle text. Kotlin: `Enter duration` / `Pick duration` titles, `TimeInput` and `TimePicker` toggle; cancel button only when a timer runs. Gap: cancel-only-when-running rule; clock dial dropped (§6).
24. `set_art_confirm` title: mockup `Use this image as cover art?` for all types matches Kotlin. Gap: none (same text).

### 16.3 Behavior not in the mockup

25. Sheet close on scrim click and Escape (Kotlin: scrim and Back). Gap: wiring.
26. Selection persistence: Kotlin writes preferences for the speed presets, statistics page, audio channels; the mockup stores nothing. Gap: `Store` wiring (p2-data-and-config.md §3).
27. Long press on top bar icons (mockup) has no Kotlin counterpart for sheets; it is a desktop convenience introduced by the mockup. Decision: keep as a desktop shortcut and document it (not parity).
28. The mockup spec has no concept of a live `PlayerModel`; all values are static. Gap: data binding (area:45).
29. Speed chip long press (`src/player.rs:227`): mockup-only.

### 16.4 Missing Kotlin states

30. Loading state for quality sheet (spinner) — mockup has none.
31. Font scan loading — mockup has none.
32. Empty track list, empty hoster result — mockup has none (Kotlin shows nothing).
33. Screenshot failure — Kotlin crashes; desktop needs a message (new string, §5.3).

---

## 17. Proposed new strings (not in Kotlin; need i18n keys before shipping)

| Key (proposal) | English | Used in |
|---|---|---|
| `player_screenshot_failed` | Screenshot failed | §5.3 |
| `player_sub_scale_zero_hint` | Subtitles are hidden at scale 0 | §10.2 |
| `player_sheets_set_as_background_confirm` | Use this image as background? | §5.2 (optional per-type confirm) |
| `player_sheets_set_as_thumbnail_confirm` | Use this image as thumbnail? | §5.2 (optional per-type confirm) |
| `player_timer_hours` | Hours | §6.2 |
| `player_timer_minutes` | Minutes | §6.2 |
| `player_delay_stop` | Stop | §11.2 |

These are proposals. The Kotlin app keeps the shared confirm text; the desktop change is optional.

---

## 18. Accessibility

- Every icon-only button has a tooltip with the same text as the Kotlin label (for example `Reset presets`, `Add current speed`, `Close`).
- Selected state is conveyed by text style (italic + bold) and by checkbox/radio state; keep both on desktop so the state is not color-only.
- Dimmed status text uses opacity 0.38 (`DISABLED_ALPHA`). Desktop proposal: keep the text but ensure a contrast ratio of at least 4.5:1 against the card background; if not, raise opacity to 0.6 (flagged).
- Focus outline: keep GTK's default focus ring.
