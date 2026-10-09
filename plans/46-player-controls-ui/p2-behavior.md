# 46-player-controls-ui (part 2 of 2): behavior spec

Scope: the 14 files listed in `p2-inventory.md`. Repo root `/home/blackbox/aniyomi-src`. Every path below is repo-relative to that root and written in full.

Labels: **OBSERVED** = what the Kotlin does. **Proposal:** = what the Rust port should do. Message keys are `AYMR.strings.*` (aniyomi i18n) or `MR.strings.*` (shared i18n). Their English values are in `p2-data-and-config.md` section 6.

---

## 0. Shared building blocks this part depends on (OBSERVED)

These are declared outside the scope, but every behavior below uses them, so their exact semantics are fixed here.

### 0.1 `PlayerSheet` (`app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt`)

- Signature (line 77): `PlayerSheet(onDismissRequest, modifier, tonalElevation = 1.dp, dismissEvent = false, content)`.
- Width: `maxWidth = 720.dp` when `ORIENTATION_LANDSCAPE`, else `420.dp`. Height: `maxHeight = screenHeightDp * 0.95f`.
- Scrim: full-screen `Box`, background `Color.Black.copy(alpha)`; `alpha` animates from 0 to 0.5 on open (`LaunchedEffect(true)`), `animateFloatAsState` with `tween(350)`.
- Sheet surface: `shape = MaterialTheme.shapes.extraLarge` with bottom corners zeroed (`bottomEnd = ZeroCornerSize, bottomStart = ZeroCornerSize`), anchored to the bottom (`Alignment.BottomCenter`), `windowInsetsPadding(systemBars only Top + Horizontal)`.
- Drag: `AnchoredDraggableState(initialValue = 1)`, anchors `0 at 0f` (open) and `1 at height` (closed). `positionalThreshold = 56.dp`, `velocityThreshold = 125.dp`, snap and decay animations use `tween(350)` and `rememberSplineBasedDecay`.
- Open: `LaunchedEffect(anchoredDraggableState)` calls `animateTo(0)`.
- Close paths: (a) when `currentValue` becomes 1, `onDismissRequest()` runs (collectLatest, drop(1)); (b) scrim click calls `internalOnDismissRequest`, which only acts when `currentValue == 0`: sets alpha 0 and animates to 1; (c) Android Back: `BackHandler(enabled = targetValue == 0, onBack = internalOnDismissRequest)`; (d) `dismissEvent` transitioning to true: `LaunchedEffect(dismissEvent)` sets alpha 0, `animateTo(1)`, then calls `onDismissRequest()`.
- Clicks on the sheet surface are swallowed (`clickable(onClick = {})`).

**Proposal:** a GTK overlay: bottom-anchored `gtk::Box` inside a `gtk::Overlay` above the player surface, `max-width 420px` (or 720px when the window is wider than 900px, a width breakpoint replacing orientation), `max-height 95%`, dim scrim `rgba(0,0,0,0.5)` with 350 ms opacity transition, close on scrim click, Escape, and an explicit close button. Drag-to-dismiss is Android-only (see p2-ui-parity). Keep the `dismissEvent` semantic: a Rust `close()` call animates out and then invokes the close handler exactly once.

### 0.2 `PlayerPanels` and `panelCardsColors` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerPanels.kt`)

- `val CARDS_MAX_WIDTH = 420.dp` (line 81).
- `panelCardsColors` (lines 82-92): container color = `MaterialTheme.colorScheme.surface` with alpha `panelOpacity / 100f`; disabled container = `surfaceDim` with the same alpha. `panelOpacity` is `pref_panel_opacity`, default **60** (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt:44`).
- `PlayerPanels(...)` (line 46) renders `Panels.SubtitleSettings -> SubtitleSettingsPanel(onDismissRequest)`, `Panels.SubtitleDelay -> SubtitleDelayPanel(onDismissRequest)`, `Panels.AudioDelay -> AudioDelayPanel(onDismissRequest)`, `Panels.VideoFilters -> VideoFiltersPanel(onDismissRequest)`, `Panels.None -> Box(Modifier.fillMaxHeight())`. The `when` starts at line 61.

### 0.3 `PlayerSheets` router (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt`)

OBSERVED wiring for each `Sheets` value (enum in `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:99-108`: `None, PlaybackSpeed, SubtitleTracks, AudioTracks, QualityTracks, Chapters, More, Screenshot`):

| Sheets value | Composable called | Notes from app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt |
|---|---|---|
| `None` | nothing | |
| `SubtitleTracks` | `SubtitlesSheet(...)` | `rememberLauncherForActivityResult(OpenDocument())` with `launch(arrayOf("*/*"))`; `null` result is ignored; `onOpenSubtitleSettings -> onOpenPanel(Panels.SubtitleSettings)`; `onOpenSubtitleDelay -> onOpenPanel(Panels.SubtitleDelay)` |
| `AudioTracks` | `AudioTracksSheet(...)` | same picker pattern, MIME `*/*`; `onOpenDelayPanel -> onOpenPanel(Panels.AudioDelay)` |
| `QualityTracks` | `QualitySheet(...)` | passes `dismissSheet` |
| `Chapters` | `if (chapter == null) return` then `ChaptersSheet(...)` | `onClick = { onSeekToChapter(chapters.indexOf(it)) }`; passes `dismissSheet` |
| `More` | `MoreSheet(...)` | `onSelectDecoder = onUpdateDecoder` (wired to `PlayerViewModel.updateDecoder`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt:618`); `onEnterFiltersPanel -> onOpenPanel(Panels.VideoFilters)`; no `customButtons` filter |
| `PlaybackSpeed` | `PlaybackSpeedSheet(speed, onSpeedChange, onDismissRequest)` | |
| `Screenshot` | `ScreenshotSheet(...)` | `hasSubTracks = subtitles.isNotEmpty()`; `onDismissRequest = onDismissScreenshot` |

Sheets and panels are mutually exclusive in the UI state (a panel is opened from a sheet, and the sheet is dismissed by the same callback path). The state holder is in part 1 (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt`).

### 0.4 `SliderItem` (`app/src/main/java/eu/kanade/presentation/player/components/SliderItem.kt`)

Two overloads.

- **Int overload** (line 45): `SliderItem(label: String, value: Int, valueText: String, onChange: (Int) -> Unit, max: Int, modifier, min: Int = 0, icon = {})`.
  - Row: `fillMaxWidth`, padding horizontal `padding.medium`, vertical `padding.small`, `Arrangement.spacedBy(padding.large)`, vertically centered.
  - `icon()`, then `Column(weight 0.5f)` with `Text(label, bodyMedium)` and `Text(valueText)`, then `Slider(weight 1.5f)`.
  - `Slider(value = value.toFloat(), valueRange = min.toFloat()..max.toFloat(), steps = max - min)`.
  - `onValueChange`: `newValue = it.toInt()` (truncation toward zero); if `newValue != value` then `onChange(newValue)` and haptic `TextHandleMove`.
- **Float overload** (line 93): `SliderItem(label: String, value: Float, valueText: String, onChange: (Float) -> Unit, max: Float, modifier, steps: Int = 0, min: Float = 0f, icon = {})`.
  - Same layout. `valueRange = min..max`, `steps = steps`. `onValueChange` passes the raw float if it differs from `value` (no rounding).

Important defaults: `min = 0` for both overloads unless a call site passes `min`.

### 0.5 `OutlinedNumericChooser` (`app/src/main/java/eu/kanade/presentation/player/components/OutlinedNumericChooser.kt`, Int at line 44, Float at line 98; implementation in `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:506`)

- Parameters: `value`, `onChange`, `max`, `step`, `min = 0 (Int) / 0f (Float)`, `suffix`, `label`. `assert(max > min)`.
- Minus button: `onChange(value - step)`. Plus button: `onChange(value + step)`. **No clamping.** Values outside `[min, max]` stay as given and the field shows an error.
- Text field (`KeyboardType.Number`), `isError = value > max || value < min`, supporting text `AYMR.strings.numeric_chooser_value_too_big` ("Value too big") when `value > max`, `AYMR.strings.numeric_chooser_value_too_small` ("Value too small") when `value < min`.
- Typing: blank input calls `onChange(0)` (Int and Float). Int: `"-"` is treated as `-0` (so `onChange(0)`), other input parsed with `toInt()`; Float input is parsed with `toFloat()`. Parse failure is swallowed by `runCatching` (text keeps the typed value, no `onChange`).
- Display sync: `LaunchedEffect(value)` rewrites the field text unless the text is blank and value is 0. Float text is formatted as `value.toString()` with trailing `0` and `.` removed.
- Repeat-on-hold is `RepeatingIconButton` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:463-500`). On press it calls `onClick` immediately, then repeats while held. Delay starts at `maxDelayMillis = 750`, and after each call becomes `(delay - delay * 0.25).toLong().coerceAtLeast(5)` (`minDelayMillis = 5`, `delayDecayFactor = .25f`). So holds accelerate: 750, 562, 421, … down to 5 ms.

### 0.6 `SwitchPreference` (`app/src/main/java/eu/kanade/presentation/player/components/SwitchPreference.kt:34`)

`SwitchPreference(value: Boolean, onValueChange: (Boolean) -> Unit, content: @Composable () -> Unit, modifier)`. The whole row toggles (`toggleable`), with a `Switch` at the end. Used by three scope files: `PlaybackSpeedSheet.kt` (pitch correction), `ScreenshotSheet.kt` (include subtitles), `SubtitleSettingsMiscellaneousCard.kt` (override ASS).

### 0.7 `ExpandableCard` (`app/src/main/java/eu/kanade/presentation/player/components/ExpandableCard.kt:53`)

`ExpandableCard(isExpanded: Boolean, title: @Composable (Boolean) -> Unit, onExpand: (Boolean) -> Unit, modifier, colors: CardColors = CardDefaults.cardColors(), shape, border, elevation, content)`. Every call site in scope passes `isExpanded = true` initially and toggles `isExpanded = !isExpanded` in `onExpand`.

### 0.8 `ExposedTextDropDownMenu` (`app/src/main/java/eu/kanade/presentation/player/components/ExposedTextDropDownMenu.kt:37`)

`(selectedValue: String, options: ImmutableList<String>, label: String, onValueChangedEvent: (String) -> Unit, modifier, leadingIcon: (@Composable () -> Unit)? = null)`. An outlined text field with an expandable menu. The `leadingIcon` is used for the font-loading spinner.

### 0.9 `TintedSliderItem` (`app/src/main/java/eu/kanade/presentation/player/components/TintedSliderItem.kt:43`)

`(label, value: Int, valueText, onChange: (Int) -> Unit, max: Int, tint: Color, modifier, min: Int = 0, ...)`. Used by the color card with `max = 255` and tints `Color.Red`, `Color.Green`, `Color.Blue`, `Color.White` (alpha).

### 0.10 `PlayerDialog` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/dialogs/PlayerDialog.kt:21`)

`(title: String, modifier, onConfirmRequest: (() -> Unit)?, onDismissRequest, content)`. `onConfirm` runs `onConfirmRequest?.invoke()` then `onDismissRequest()`.

### 0.11 `ActionButton` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/ActionButton.kt:17`)

`(title: String, onClick, icon, modifier)`: icon above the title in a clickable tile. Used by ScreenshotSheet.

### 0.12 Custom button execution (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2167-2173`)

- `CustomButton.execute()` runs `MPVLib.command(arrayOf("script-message", "call_button_$id"))`.
- `CustomButton.executeLongPress()` runs `MPVLib.command(arrayOf("script-message", "call_button_${id}_long"))`.

---

## 1. Track sheets: shared skeleton (`GenericTracksSheet.kt`)

### 1.1 `GenericTracksSheet<T>` (lines 48-71)

- Signature: `(tracks: ImmutableList<T>, onDismissRequest, modifier, dismissEvent: Boolean = false, header: @Composable () -> Unit = {}, track: @Composable (T) -> Unit = {}, footer: @Composable () -> Unit = {})`.
- Renders `PlayerSheet(onDismissRequest, dismissEvent = dismissEvent) { Column(modifier) { header(); LazyColumn { items(tracks) { track(it) }; item { footer() } } } }`.
- The footer is always emitted as a final `LazyColumn` item. When the caller passes no footer, the item is empty.

### 1.2 `AddTrackRow` (lines 73-106)

- Outer `Row`: `fillMaxWidth`, `clickable(onClick)`, `height(48.dp)`, vertical center, spacing `padding.small`.
- Inner `Row`: `clickable(onClick)`, `fillMaxHeight`, `weight(1f)`, `padding(start = padding.medium)`, spacing `padding.small`. Contains `Icons.Default.Add` at 32 dp and the title text.
- Then `actions()` (a `RowScope` slot, unused by the callers in scope).

### 1.3 `TrackSheetTitle` (lines 131-159)

- Row `fillMaxWidth`, padding start `padding.medium`, end `padding.medium`, top `padding.small`, bottom `padding.extraSmall`, `SpaceBetween`, vertical center.
- Title `Text(headlineMedium)`; right side is a `Row` (spacing `padding.extraSmall`) holding `actions()`.

### 1.4 `getTrackTitle(track: VideoTrack): String` (lines 108-129), exact decision table

Evaluated top to bottom; first match wins. `track.language.isNullOrBlank()` treats `null` and whitespace as blank.

| Order | Condition | Result string | Resource key | Args |
|---|---|---|---|---|
| 1 | `track.id == -1` | `track.name` verbatim (no resource) | none | |
| 2 | language blank AND name not blank | `#%d: %s` | `player_sheets_track_title_wo_lang` | id, name |
| 3 | language not blank AND name not blank | `#%d: %s (%s)` | `player_sheets_track_title_w_lang` | id, name, language |
| 4 | language not blank AND name blank | `#%d: %s` | `player_sheets_track_lang_wo_title` | id, language |
| 5 | else (language blank AND name blank) | `#%d: %s` | `player_sheets_track_title_wo_lang` | id, name (empty) |

`id == -1` is the "Off" track. The name for it is produced by the `getTrackTitle` lambda in app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:400, which returns `MR.strings.off` when id is -1.

Example outputs (English): `#1: Japanese (jpn)`, `#2: Commentary`, `#3: English`, `#4: `, `Off`.

**Proposal:** pure function `track_label(id: i64, name: &str, lang: Option<&str>) -> String` in `src/player/ui/track_label.rs`, with `#[test]` cases for all five rows (see p2-edge-cases-and-tests.md).

### 1.5 `VideoTrack` data type (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:473-477)

`@Immutable data class VideoTrack(val id: Int, val name: String, val language: String?)`. Track ids are mpv `track-list` ids (`track-list/N/...`), with `-1` reserved for "Off".

---

## 2. Audio tracks sheet (`AudioTracksSheet.kt`)

### 2.1 Layout (lines 43-86)

1. `GenericTracksSheet` with `header`:
   - `TrackSheetTitle(title = AYMR.strings.pref_player_audio ("Audio"), actions = { TextButton(onOpenDelayPanel) { Icon(MoreTime) + Text(AYMR.strings.player_sheets_track_delay ("Delay")) } })`.
   - `AddTrackRow(title = AYMR.strings.player_sheets_add_ext_audio ("Add external audio tracks"), onClick = onAddAudioTrack)`.
2. Track rows: `AudioTrackRow(title = getTrackTitle(it), isSelected = selectedId == it.id, onClick = { onSelect(it.id) })`.
3. No footer.

### 2.2 `AudioTrackRow` (lines 88-113)

- Row `fillMaxWidth`, `clickable(onClick)`, padding start `padding.small`, end `padding.medium`, spacing `padding.small`, vertical center.
- `RadioButton(selected = isSelected, onClick = onClick)`.
- `Text(title, fontWeight = if selected ExtraBold else Normal, fontStyle = if selected Italic else Normal)`.

### 2.3 Behavior

- Selecting a row calls `onSelect(id)`. The selected id is the mpv track id, and `Off` (`-1`) is a selectable row produced by the view model (the list comes from PlayerViewModel audio tracks, out of scope).
- "Add external audio": opens the file picker with MIME `*/*` (app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerSheets.kt). The picked `Uri` goes to `onAddAudio(Uri)` (PlayerViewModel, part 1). A cancelled pick does nothing.
- "Delay" opens `Panels.AudioDelay`. The audio delay panel is part 1 and reuses `DelayCard` (section 11).

**Proposal:** `gtk::FileDialog` (`open`, no filter, all files) and a `gtk::ListBox` of `adw`-free rows with a `gtk::CheckButton` in radio group, or `gtk::CheckButton::set_group`, to get radio semantics.

---

## 3. Subtitle tracks sheet (`SubtitleTracksSheet.kt`)

### 3.1 Layout (lines 47-111)

- Header: `TrackSheetTitle(title = AYMR.strings.pref_player_subtitle ("Subtitles"), actions = [TextButton(onOpenSubtitleSettings){ Palette + "Palette" (player_sheets_track_palette) }, TextButton(onOpenSubtitleDelay){ MoreTime + "Delay" (player_sheets_track_delay) }])`.
- Then `AddTrackRow(title = AYMR.strings.player_sheets_add_ext_sub ("Add external subtitles"), onClick = onAddSubtitle)`.
- Track rows: `SubtitleTrackRow(title = getTrackTitle(track), selected = selectedTracks.indexOf(track.id), onClick = { onSelect(track.id) })`.
- Footer: `Column(modifier.padding(padding.medium).fillMaxWidth(), spacing padding.medium, alignment Start) { Icon(Icons.Outlined.Info); Text(AYMR.strings.player_sheets_subtitles_footer_secondary_sid_no_styles ("Secondary subtitles will have no ASS/SSA Styling.")) }`. Note the `modifier` passed to the footer is the sheet modifier (also applied to `GenericTracksSheet`); this duplicates the modifier (observed).

### 3.2 `SubtitleTrackRow` (lines 114-145)

- Parameter `selected: Int`: `-1` means not selected; `0` or `1` is the index in `selectedTracks` (primary = 0, secondary = 1). `selectedTracks` is an `ImmutableList<Int>` of track ids supplied by the view model (max two entries by design, value enforced in PlayerViewModel, out of scope).
- Row: `fillMaxWidth`, `clickable(onClick)`, padding start `padding.small`, end `padding.medium`, vertical center.
- `Checkbox(checked = selected > -1, onCheckedChange = { onClick() })`.
- `Text(title, italic + ExtraBold when selected > -1)`.
- `Spacer(weight 1f)`.
- If `selected != -1`: `Text("#${selected + 1}")` with the same italic/ExtraBold rule. Shows `#1` for primary and `#2` for secondary.

**Behavior:** the checkbox toggles through the same `onClick`, so clicking a checked row deselects it. Only one primary and one secondary slot exist (enforced by the view model).

**Proposal:** two-slot model: `selected_slots: [Option<i64>; 2]` in the Rust state, `SubtitleTrackRow` shows `#1`/`#2` labels from the slot index. Keep the `Checkbox` semantic.

---

## 4. Chapters sheet (`ChaptersSheet.kt`)

### 4.1 Layout (lines 39-68)

- `GenericTracksSheet(tracks = chapters, header = { TrackSheetTitle(title = AYMR.strings.player_sheets_chapters_title ("Chapters"), modifier = modifier.padding(top = padding.small)) }, track = { ChapterTrack(chapter = it, index = chapters.indexOf(it), selected = currentChapter == it, onClick = { onClick(it) }) }, onDismissRequest, dismissEvent = dismissSheet, modifier)`.
- The `modifier` is applied to the title row and to the sheet (observed, duplicated modifier).
- `chapters` is `ImmutableList<Segment>` from `dev.vivvvek.seeker.Segment` (external library). The concrete type is `IndexedSegment` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/SeekBar.kt:54`): `name: String`, `start: Float` (seconds), `color`, `index`, `chapterType`.

### 4.2 `ChapterTrack` (lines 70-99)

- Row `fillMaxWidth`, `clickable(onClick)`, padding vertical `padding.small`, horizontal `padding.medium`, `SpaceBetween`.
- Left: `Text(stringResource(AYMR.strings.player_sheets_track_title_wo_lang, index + 1, chapter.name), italic/ExtraBold when selected, maxLines = 1, weight(1f), TextOverflow.Ellipsis)`. The label is `#%d: %s`, so the first chapter reads `#1: Intro`.
- Right: `Text(Utils.prettyTime(chapter.start.toInt()))`, same italic/ExtraBold rule. `Utils` is `is.xyz.mpv.Utils` from the external mpv-android artifact. Its exact string format is not in the repo; see the Proposal below.

### 4.3 Data source and selection (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:479-494)

- `loadChapters()` reads `chapter-list/count` (`!!`), then for each `i`: `chapter-list/$i/title` (string, may be null), `chapter-list/$i/time` (int seconds, `!!`).
- Builds `IndexedSegment(name = title, start = time.toFloat(), index = 0)` and sorts by `start` ascending (`sortedBy { it.start }`).
- Note that `index` is always `0` in the source; the sheet uses `indexOf` and not this field.
- `selectChapter(index)` (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:500) seeks to `chapters.value[index].start`.
- `current chapter` (the `chapter` parameter in PlayerSheets) comes from part 1. If it is `null`, the sheet is not rendered at all (`if (chapter == null) return`).

### 4.4 Behavior

- Tapping a row calls `onClick(segment)`. PlayerSheets converts this to `onSeekToChapter(chapters.indexOf(segment))`, so the seek uses the sorted list index.
- Selected row: italic, ExtraBold. Equality is data-class equality (`currentChapter == it`).
- Two chapters with equal fields (name, start, index, color, type) get the same `indexOf` and show the same number (observed edge).

**Proposal:** store the ordinal at load time (`ord: usize` = position after sorting) and use it for the label. This removes the O(n²) `indexOf` in rendering and the equal-chapter collision. Keep the label `#N: name`.

---

## 5. Quality / hoster sheet (`QualitySheet.kt`)

### 5.1 Model (lines 51-72)

```kotlin
sealed class HosterState(open val name: String) {
    data class Idle(override val name: String) : HosterState(name)
    data class Loading(override val name: String) : HosterState(name)
    data class Error(override val name: String) : HosterState(name)
    data class Ready(
        override val name: String,
        val videoList: List<Video>,
        val videoState: List<Video.State>,
    ) : HosterState(name)
}
```

- `HosterState.Ready.getChangedAt(index, newVideo, newState)` returns a copy with `videoList[index]` replaced by `newVideo` and `videoState[index]` by `newState`. Indices outside range are untouched (map by index, no bounds error).
- `Video.State` (source-api `Video.kt:180`): `QUEUE, LOAD_VIDEO, READY, ERROR`.
- `Hoster.State` (source-api `Hoster.kt:22`): `IDLE, LOADING, READY, ERROR`. The sheet does not use `Hoster.State`; it uses its own `HosterState`. Mapping is part 1 (loader).
- `Hoster.NO_HOSTER_LIST = "no_hoster_list"` (source-api `Hoster.kt:57`) is the sentinel hoster name for sources that return videos without hosters.

### 5.2 Top-level sheet (lines 74-162)

Inputs: `isLoadingHosters`, `hosterState: List<HosterState>`, `expandedState: List<Boolean>`, `selectedVideoIndex: Pair<Int, Int>` (hoster index, video index), `onClickHoster(Int)`, `onClickVideo(Int, Int)`, `displayHosters: Pair<Boolean, Boolean>` (first = show failed, second = show empty), `onDismissRequest`, `dismissSheet`.

1. `PlayerSheet(onDismissRequest = onDismissRequest(), dismissEvent = dismissSheet, modifier)`.
2. Title `Text(AYMR.strings.player_sheets_qualities_title ("Qualities"), headlineMedium, padding top medium, start medium, bottom extraSmall)`.
3. `AnimatedVisibility(visible = isLoadingHosters, enter = fadeIn() + slideInVertically(initialOffsetY = { it / 2 }), exit = fadeOut() + slideOutVertically(targetOffsetY = { it / 2 }))` containing a `Box` with padding vertical `padding.medium`, centered `CircularProgressIndicator()`.
4. `qualitySheetPadding = PaddingValues(bottom = medium, start = medium, end = medium)`.
5. `AnimatedVisibility(visible = !isLoadingHosters, same enter/exit)`:
   - If `hosterState.size == 1 && hosterState.first().name == Hoster.NO_HOSTER_LIST && hosterState.first() is HosterState.Ready`: `QualitySheetVideoContent(videoList, videoState, selectedVideoIndex.second, onClickVideo, modifier.padding(qualitySheetPadding))`.
   - Else: `QualitySheetHosterContent(hosterState, expandedState, selectedVideoIndex, onClickHoster, onClickVideo, displayHosters, modifier.padding(qualitySheetPadding))`.

Note: the loading spinner and the content are not mutually exclusive during the 2-animation transition; both use `AnimatedVisibility`.

### 5.3 Flat video list (lines 164-183)

`LazyColumn(modifier.fillMaxWidth())` with `itemsIndexed(videoList)`: `VideoTrack(video, videoState = videoState[idx], selected = selectedVideoIndex == idx, onClick = { onClickVideo(0, idx) }, noHoster = true)`. Hoster index is hard-coded `0`.

### 5.4 Hoster groups (lines 185-236)

Partition, computed on each composition from `hosterState.withIndex()`:

- `validHosters`: `state is Idle || state is Loading || (state is Ready && videoList.isNotEmpty())`.
- `failedHosters`: `state is Error`.
- `emptyHosters`: `state is Ready && videoList.isEmpty()`.

Rendering order inside one `LazyColumn`:
1. `hosterContent(validHosters, ...)` always.
2. `hosterContent(failedHosters, ...)` only if `displayHosters.first`.
3. `hosterContent(emptyHosters, ...)` only if `displayHosters.second`.

A `Ready` hoster with an empty list is therefore hidden unless `displayHosters.second` is true. Error hosters are hidden unless `displayHosters.first` is true. (Who sets these booleans is part 1; the sheet only honors them.)

### 5.5 `hosterContent` (lines 238-279)

For each `(hosterIdx, hoster)`:
- `isExpanded = expandedState.getOrNull(hosterIdx) ?: false`.
- One `item { ... }` containing:
  - `HosterTrack(hoster, selected = selectedVideoIndex.first == hosterIdx, isExpanded, onClick = { onClickHoster(hosterIdx) })`.
  - `AnimatedVisibility(visible = hoster is Ready && isExpanded, enter = expandVertically(), exit = shrinkVertically())` containing, for a `Ready` hoster, a `Column(fillMaxWidth)` of `VideoTrack(video, videoState = hoster.videoState[videoIdx], selected = selectedVideoIndex == Pair(hosterIdx, videoIdx), onClick = { onClickVideo(hosterIdx, videoIdx) }, noHoster = false)`.

Note: `expandedState` is used for the expansion of the hoster, not `hoster.videoList`. A Ready hoster that is expanded but has an empty list shows an empty column (observed).

### 5.6 `HosterTrack` (lines 281-345)

- Row `height(32.dp)`, `clickable(onClick)`, vertical center.
- Name: `Text(hoster.name, italic/ExtraBold when selected, bodyMedium, maxLines = 1, TextOverflow.Ellipsis, padding end small)`.
- Status by state:
  - `Idle`: `Text(AYMR.strings.player_hoster_tap_to_load ("Tap to load videos"), alpha DISABLED_ALPHA)`.
  - `Error`: `Text(AYMR.strings.player_hoster_failed ("Failed to load videos"), alpha DISABLED_ALPHA)`, `Spacer(weight 1f)`, `Icon(ErrorOutline, tint = MaterialTheme.colorScheme.error)`.
  - `Loading`: `Spacer(weight 1f)`, `CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)`.
  - `Ready`: `Text(pluralStringResource(AYMR.plurals.hoster_video_count, n, n), alpha DISABLED_ALPHA)` where `n = videoList.size` ("1 video" / "%d videos"), `Spacer(weight 1f)`, then `KeyboardArrowUp` if `isExpanded` else `KeyboardArrowDown`.
- `DISABLED_ALPHA = .38f` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt:9`).

Note: the `Idle` hoster is clickable and loads on tap via `onClickHoster` (part 1 handles the loading). The `Loading` row is clickable too (observed; no guard).

### 5.7 `VideoTrack` composable (lines 347-387)

- Row `fillMaxWidth`, `clickable(onClick)`, vertical center, spacing `padding.small`.
- `noHoster == true`: `VideoText(weight 1f)` first, then `VideoIcon(noHoster = true)`.
- `noHoster == false`: `VideoIcon(noHoster = false)` first, then `VideoText(weight 1f)`.

### 5.8 `VideoIcon` (private, lines 389-416)

- `Box(size = 28.dp if noHoster else 20.dp, contentAlignment = Center)`.
- `QUEUE`, `READY`: nothing.
- `LOAD_VIDEO`: `CircularProgressIndicator(fillMaxSize, strokeWidth = 2.dp)`.
- `ERROR`: `Icon(ErrorOutline, tint = error, fillMaxWidth)`.

### 5.9 `VideoText` (private, lines 418-438)

- `Text(video.videoTitle, italic + ExtraBold when selected, bodyMedium, color = primary when selected else Unspecified, maxLines = 6, TextOverflow.Ellipsis, padding vertical = small if noHoster else extraSmall)`.
- `video.videoTitle` is the source-provided label (source-api `Video`).

### 5.10 Behavior summary

- Selected video is highlighted by `selectedVideoIndex`. Clicking a video calls `onClickVideo(hosterIdx, videoIdx)`; the caller switches the quality.
- Clicking a hoster row only toggles selection or expansion through the caller; the sheet itself does not change `expandedState`.
- **Observed issue:** when `isLoadingHosters` is true the list is hidden; a sheet opened while loading shows only the spinner. The hosters appear when loading ends.

**Proposal:** model the sheet as a pure function `group_hosters(&[HosterState], display: (bool, bool)) -> Groups { valid, failed, empty }` (tested in p2-edge-cases-and-tests.md). The UI draws groups in the fixed order above.

---

## 6. Playback speed sheet (`PlaybackSpeedSheet.kt`)

### 6.1 Inputs and sources

- `speed: Float` (current playback speed, from part 1), `onSpeedChange(Float)`, `onDismissRequest`.
- `PlayerPreferences` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/PlayerPreferences.kt`): `speedPresets()` = `getStringSet("default_speed_presets", setOf("0.25","0.5","0.75","1.0","1.25","1.5","1.75","2.0","2.5","3.0","3.5","4.0"))`; `playerSpeed()` = `getFloat("pref_player_speed", 1f)`.
- `AudioPreferences`: `enablePitchCorrection()` = `getBoolean("pref_audio_pitch_correction", true)`.

### 6.2 Layout (lines 62-176)

1. `PlayerSheet(onDismissRequest)` with `Column(modifier.verticalScroll(rememberScrollState()).padding(vertical = padding.medium))`.
2. Speed slider: `SliderItem(label = AYMR.strings.player_sheets_speed_slider_label ("Speed"), value = speed, valueText = stringResource(AYMR.strings.player_speed, speed) ("%.2fx"), onChange = onSpeedChange, max = 6f, min = 0.01f)` (Float overload, continuous, steps 0).
3. Preset row: `Row(fillMaxWidth, padding horizontal medium, spacing medium, vertical center)`:
   - `FilledTonalIconButton(RestartAlt)` → `preferences.speedPresets().delete()` (restores the default set above).
   - `LazyRow(weight 1f, spacing extraSmall)` of presets: `items(presets.map { it.toFloat() }.sorted(), key = { it })`. Each item is `InputChip(selected = speed == it, onClick = { onSpeedChange(it) }, label = { Text(stringResource(AYMR.strings.player_speed, it)) }, modifier = Modifier.animateItem(), trailingIcon = { Icon(Close, modifier.clickable { presets.set(presets.minus(it.toFixed(2).toString())) }) })`.
   - `FilledTonalIconButton(Add)` → `presets.set(presets.plus(speed.toFixed(2).toString()))`.
4. Pitch correction: `SwitchPreference(value = pitchCorrection, onValueChange = { audioPrefs.enablePitchCorrection().set(it); MPVLib.setPropertyBoolean("audio-pitch-correction", it) }, content = { Column(weight 1f) { Text(AYMR.strings.pref_audio_pitch_correction_title); Text(AYMR.strings.pref_audio_pitch_correction_summary, bodyMedium) } })`.
5. Action row: `Row(padding horizontal medium, spacing small)`:
   - `Button(weight 1f, onClick = { playerSpeed().set(speed) })` text `AYMR.strings.player_sheets_speed_make_default` ("Make default speed").
   - `FilledIconButton(RestartAlt)` → `playerSpeed().delete()` then `onSpeedChange(1f)`.

### 6.3 Rules

- Preset string format: `toFixed(2).toString()` of a Float, e.g. `1.0`, `0.25`, `1.75`. Stored as strings in the set.
- Sorting: numeric (`map { toFloat() }.sorted()`), so `"0.25" < "0.5" < "1.0" < "4.0"`.
- Chip selected when `speed == preset` (Float equality). Because the slider is continuous and not rounded, the current speed `1.2345` is never selected by a rounded preset `1.23`.
- Add: no duplicate check needed (set semantics). Adding the same rounded value again is a no-op.
- Remove: computes the string from the chip's float with `toFixed(2)`. If a stored string is not canonical (for example `"1.5000"`), the remove call does not match and the chip stays (observed edge).
- `Float.toFixed(precision: Int = 1)` (line 178): `factor = 10.0f.pow(precision)`, returns `(this * factor).roundToInt() / factor`. Uses half-up for positive values (`roundToInt` = `Math.round`, which rounds .5 toward positive infinity).
- `speed` is written live by the caller through `onSpeedChange` (no debounce in this sheet). The Kotlin `Slider` callback fires on each drag change (the `SliderItem` Float overload forwards every change).
- Slider minimum `0.01f`, maximum `6f`.
- Make default writes the current exact float, not the rounded one.
- Reset: deletes the pref (default `1f`) and sets speed to `1f`.

**Proposal:** `to_fixed2(x: f32) -> String` returning the canonical string (`format!("{:.2}", x)` trimmed of trailing zeros but keeping one decimal: `1.0`, `0.25`). Parse presets with `str::parse::<f32>()`; skip invalid strings with a log, do not crash (Kotlin would throw `NumberFormatException` from `toFloat()`).

---

## 7. More sheet (`MoreSheet.kt`, lines 84-264)

### 7.1 Inputs

`selectedDecoder: Decoder`, `onSelectDecoder: (Decoder) -> Unit`, `remainingTime: Int` (seconds, `0` = no timer), `onStartTimer: (Int) -> Unit` (seconds), `onDismissRequest`, `onEnterFiltersPanel`, `customButtons: ImmutableList<CustomButton>`.

Preferences read: `AdvancedPlayerPreferences.playerStatisticsPage()` (`getInt("pref_player_statistics_page", 0)`, collected as state), `AudioPreferences.audioChannels()` (`getEnum("pref_audio_config", AudioChannels.AutoSafe)`, collected).

### 7.2 Layout top to bottom

1. Header `Row(fillMaxWidth, SpaceBetween, vertical center)`:
   - `Text(AYMR.strings.player_sheets_more_title ("More"), headlineMedium)`.
   - Right `Row` (spacing extraSmall) with two `TextButton`s:
     a. **Sleep timer** (section 9): label is `AYMR.strings.timer_title` ("Sleep Timer") when `remainingTime == 0`, else `AYMR.strings.timer_remaining` ("%s remaining") with `DateUtils.formatElapsedTime(remainingTime.toLong())`. Icon `Icons.Outlined.Timer`. Opens `TimePickerDialog`.
     b. **Filters**: `Icons.Default.Tune` + `AYMR.strings.player_sheets_filters_title` ("Filters") → `onEnterFiltersPanel`.
2. `Text(AYMR.strings.player_hwdec_mode ("Hardware decoding mode"))` then `LazyRow` of `FilterChip`s: `items(Decoder.entries.minus(Decoder.Auto))`, `selected = decoder == selectedDecoder`, `onClick = { onSelectDecoder(decoder) }`, `label = { Text(decoder.title) }` (the `title` is a literal, not localized).
   - Displayed chips in enum order: `AutoCopy` (title `"Auto"`), `SW` (`"SW"`), `HW` (`"HW"`), `HWPlus` (`"HW+"`).
   - `Decoder.Auto` (title `"Auto"`, value `"auto"`) is never shown.
3. `Text(AYMR.strings.player_sheets_stats_page_title ("Default statistics page"))` then `LazyRow` of `items(6)` chips, page 0..5:
   - Label: page 0 → `AYMR.strings.player_sheets_tracks_off` ("Off"); page n>0 → `AYMR.strings.player_sheets_stats_page_chip` ("Page %d") with arg `n`.
   - `selected = statisticsPage == page`.
   - `onClick` (exact logic):
     - `if ((page == 0) xor (statisticsPage == 0))` → `MPVLib.command(arrayOf("script-binding", "stats/display-stats-toggle"))`.
     - `if (page != 0)` → `MPVLib.command(arrayOf("script-binding", "stats/display-page-$page"))`.
     - `advancedPreferences.playerStatisticsPage().set(page)`.
   - Truth table for the toggle (current → clicked):
     - 0 → 0: no toggle, no page command, pref set to 0 (no-op).
     - 0 → n (n>0): toggle on, then `display-page-n`.
     - n → 0: toggle off (stats were on).
     - n → m (both > 0): no toggle, `display-page-m` only.
4. Custom buttons (only if `customButtons.isNotEmpty()`): `Text(AYMR.strings.player_sheets_custom_buttons_title ("Custom buttons"))`, then a `FlowRow(vertical spacing mediumSmall, horizontal spacing small, maxItemsInEachRow = Int.MAX_VALUE)` of:
   - `Box { FilterChip(onClick = {}, label = Text(button.name), selected = false, interactionSource = remembered); Box(matchParentSize, combinedClickable(onClick = { button.execute() }, onLongClick = { button.executeLongPress() }, indication = null)) }`.
   - The chip itself is non-interactive; the overlay box handles tap and long press. `interactionSource` is shared so the chip still shows press feedback.
5. `Text(AYMR.strings.pref_audio_channels ("Audio channels"))` then `LazyRow` of `items(AudioChannels.entries)`: `FilterChip(selected = audioChannels == it, label = { Text(stringResource(it.titleRes)) }, onClick = { ... })` (see 7.3).

### 7.3 Audio channels click logic (exact, lines 249-257)

```
audioPreferences.audioChannels().set(it)
if (it == AudioChannels.ReverseStereo) {
    MPVLib.setPropertyString(AudioChannels.AutoSafe.property, AudioChannels.AutoSafe.value)   // "audio-channels" = "auto"
} else {
    MPVLib.setPropertyString(AudioChannels.ReverseStereo.property, "")                        // "af" = ""
}
MPVLib.setPropertyString(it.property, it.value)
```

`AudioChannels` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/AudioPreferences.kt:21`), in enum order:

| Name | titleRes key (English) | property | value |
|---|---|---|---|
| `Auto` | `pref_player_audio_channels_auto` ("Auto") | `audio-channels` | `auto-safe` |
| `AutoSafe` | `pref_player_audio_channels_auto_safe` ("Auto-safe") | `audio-channels` | `auto` |
| `Mono` | `pref_player_audio_channels_mono` ("Mono") | `audio-channels` | `mono` |
| `Stereo` | `pref_player_audio_channels_stereo` ("Stereo") | `audio-channels` | `stereo` |
| `ReverseStereo` | `pref_player_audio_channels_reverse_stereo` ("Reverse stereo") | `af` | `pan=[stereo|c0=c1|c1=c0]` |

**Trap (observed):** the enum names are swapped relative to the mpv values. The label "Auto" writes `auto-safe`; the label "Auto-safe" writes `auto`. The stored preference is the enum name (`getEnum`), so the stored value and the label stay consistent. The Rust port must key by label or by an explicit mapping table and must not reuse the Kotlin names.

Side effects:
- Choosing `ReverseStereo` sets `audio-channels=auto` and then `af=pan=[...]`.
- Choosing any other value sets `af=""`, which **clears the whole mpv audio filter chain**, including any filter the user added in `mpv.conf` or the Lua scripts (observed side effect).
- The default is `AutoSafe` (label "Auto-safe", value `auto`). At file load, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:586` applies `audioChannels().get().let { MPVLib.setPropertyString(it.property, it.value) }` (part 1). It does not clear `af`.

**Proposal:** on `ReverseStereo` set `af` to the pan string; on any other choice, remove only the pan filter from the chain (`af remove` by label, or rebuild the chain without it). This keeps other filters intact. Document this as a deliberate deviation.

### 7.4 Decoder change (observed)

- `onSelectDecoder = viewModel::updateDecoder` (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:389): `MPVLib.setPropertyString("hwdec", decoder.value)`. Only mpv is changed; **the preference is not written**. At startup `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:118` sets `hwdec` from `pref_try_hwdec` (default `true`): `"auto"` when true, `"no"` when false. A chip choice therefore lasts for the current playback session only.
- Selected chip = `decoder == selectedDecoder`. `selectedDecoder` is `getDecoderFromValue(activity.player.hwdecActive)` (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:385-386), which maps mpv's current `hwdec-current` string to the enum by `value`. `getDecoderFromValue` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt`) uses `first { it.value == value }` and throws `NoSuchElementException` for an unknown string (observed). `hwdecActive` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:79-80`) returns `getPropertyString("hwdec-current") ?: "no"`. On Linux, mpv reports values such as `vaapi-copy`, `nvdec`, `vulkan-copy` that exist in no enum entry, so the Kotlin mapping would throw on a Linux build. The Rust port must map unknown values to a `Other(String)` state instead of failing (see p2-edge-cases-and-tests.md).

### 7.5 Decoder enum (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:81-87`)

`AutoCopy("Auto", "auto-copy")`, `Auto("Auto", "auto")`, `SW("SW", "no")`, `HW("HW", "mediacodec-copy")`, `HWPlus("HW+", "mediacodec")`.

**Proposal (Linux):** the mpv `hwdec` values are the same strings, but Android-only values (`mediacodec`, `mediacodec-copy`) have no Linux meaning. Map Linux chips to `auto-copy` (Auto), `no` (SW), `vaapi-copy` or `nvdec-copy` (HW, chosen by the hardware detected at startup), and `vaapi` or `nvdec` (HW+). Keep the four chip labels. Mark `Auto` (`auto`) as hidden, as the Kotlin does.

---

## 8. Sleep timer dialog (`MoreSheet.kt:266-354`, `TimePickerDialog`)

### 8.1 Opening and params

- `isSleepTimerDialogShown` remembered state. The dialog is composed only while shown (`if (isSleepTimerDialogShown) TimePickerDialog(remainingTime, onDismissRequest, onTimeSelect = onStartTimer)`).
- `TimePickerDialog(onDismissRequest, onTimeSelect: (Int) -> Unit, modifier, remainingTime: Int = 0)`.

### 8.2 Window

- `Dialog(onDismissRequest, DialogProperties(usePlatformDefaultWidth = false))`.
- `Surface(shape = MaterialTheme.shapes.medium, color = surface, modifier.padding(padding.medium))`.
- `Column(verticalScroll, width(IntrinsicSize.Max), animateContentSize, padding(padding.medium))`.

### 8.3 Contents

- `currentLayoutType` = `rememberSaveable { mutableIntStateOf(0) }`. `0` = input mode, `1` = clock dial mode.
- Title text: `AYMR.strings.timer_picker_pick_time` ("Pick duration") when layout 1, else `AYMR.strings.timer_picker_enter_timer` ("Enter duration").
- `rememberTimePickerState(initialHour = remainingTime / 3600, initialMinute = (remainingTime % 3600) / 60, is24Hour = true)`. Note: `initialHour` is not clamped; `remainingTime` is seconds, so values above 24 h are passed as-is to the picker (observed; the Material picker is limited to 0..23 hours).
- `Box(contentAlignment = Center) { if (layout == 1) TimePicker(state) else TimeInput(state) }`.
- Row `SpaceBetween`, fillMaxWidth:
  - `IconButton` toggling layout (`0 → 1`, `1 → 0`). Icon: `Icons.Outlined.Schedule` when layout is 0, `Icons.Default.KeyboardAlt` when layout is 1.
  - Right `Row`:
    - If `remainingTime > 0`: `TextButton("AYMR.strings.timer_cancel_timer" ("Cancel current timer"))` → `onTimeSelect(0)`, `onDismissRequest()`.
    - `Spacer(width 8.dp)`.
    - `TextButton(MR.strings.action_ok ("OK"))` → `onTimeSelect(state.hour * 3600 + state.minute * 60)`, `onDismissRequest()`.

### 8.4 Rules

- Granularity is minutes. Seconds cannot be set (the picker has no seconds field). The result is seconds (`hour*3600 + minute*60`).
- Setting `00:00` and pressing OK calls `onStartTimer(0)`, which equals cancel (observed).
- Cancel is offered only when a timer is running.
- The timer itself (countdown, the action when it fires) is in part 1 (`PlayerViewModel`); this file only emits the seconds.
- Label text in the sheet uses `DateUtils.formatElapsedTime(remaining)`. `android.text.format.DateUtils.formatElapsedTime(long)` returns `MM:SS` when under one hour and `H:MM:SS` otherwise (Android API contract). Example: `05:03`, `1:05:03`.

**Proposal:** a `gtk::Window` (modal, transient for the player window) with two `gtk::SpinButton`s (hours 0..99, minutes 0..59, wrap), OK / Cancel / (Cancel timer when running). Drop the clock dial for v1 (see p2-ui-parity). The remaining label uses the same `MM:SS` / `H:MM:SS` rule implemented in Rust.

---

## 9. Statistics page (see §7.2 step 3)

Covered in §7.2. Additional observed facts:
- `AniyomiMPVView.postInitOptions()` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:163-169`) re-applies the saved page at startup: if `playerStatisticsPage != 0` it sends `stats/display-stats-toggle` then `stats/display-page-$it`.
- The sheet reads the preference, not the mpv state, so a statistics overlay toggled from the keyboard (mpv default bindings) desynchronizes the chip selection (observed).

**Proposal:** on sheet open, query mpv for the statistics overlay state (script property if the stats script exposes one, otherwise keep the preference as truth and document the desync). Priority P2.

---

## 10. Screenshot sheet (`ScreenshotSheet.kt`)

### 10.1 Inputs

`isLocalSource: Boolean`, `hasSubTracks: Boolean`, `showSubtitles: Boolean`, `onToggleShowSubtitles(Boolean)`, `cachePath: String`, `onSetAsArt(ArtType, () -> InputStream)`, `onShare(() -> InputStream)`, `onSave(() -> InputStream)`, `takeScreenshot(String, Boolean): InputStream?`, `onDismissRequest`.

### 10.2 Layout (lines 47-127)

- `var setArtTypeAs: ArtType? by remember { mutableStateOf(null) }`.
- `PlayerSheet(onDismissRequest, modifier)` with `Column`:
  - `Row(padding top medium, spacing small)` of `ActionButton(weight 1f, icon = Icons.Outlined.Photo | Share | Save)`:
    1. `title = MR.strings.set_as_cover` ("Set as cover") → `setArtTypeAs = ArtType.Cover`.
    2. `title = AYMR.strings.set_as_background` ("Set as background") → `ArtType.Background`.
    3. only if `isLocalSource`: `title = AYMR.strings.set_as_thumbnail` ("Set as thumbnail") → `ArtType.Thumbnail`.
    4. `title = MR.strings.action_share` ("Share"), `Icons.Outlined.Share` → `onShare { takeScreenshot(cachePath, showSubtitles)!! }`.
    5. `title = MR.strings.action_save` ("Save"), `Icons.Outlined.Save` → `onSave { takeScreenshot(cachePath, showSubtitles)!! }`.
  - If `hasSubTracks`: `SwitchPreference(value = showSubtitles, onValueChange = onToggleShowSubtitles, modifier padding bottom medium, content = Text(AYMR.strings.screenshot_show_subs ("Include Subtitles"), labelLarge, primary))`.
- If `setArtTypeAs != null`: `PlayerDialog(title = MR.strings.confirm_set_image_as_cover ("Use this image as cover art?"), modifier = fillMaxWidth(fraction = 0.6F).padding(medium), onConfirmRequest = { onSetAsArt(setArtTypeAs!!) { takeScreenshot(cachePath, showSubtitles)!! } }, onDismissRequest = { setArtTypeAs = null })`.

Observed: the confirm dialog text is the same for Cover, Background and Thumbnail (the string says "cover art"). The text is misleading for non-cover types.

### 10.3 Screenshot capture (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1830-1845, called through the lambda)

- `filename = "$cachePath/${System.currentTimeMillis()}_mpv_screenshot_tmp.png"`.
- `subtitleFlag = if (showSubtitles) "subtitles" else "video"`.
- `MPVLib.command(arrayOf("screenshot-to-file", filename, subtitleFlag))`.
- `tempFile = File(filename).takeIf { exists } ?: return null`.
- `newFile = "$cachePath/mpv_screenshot.png"`; `newFile.delete()`; `tempFile.renameTo(newFile)`; returns `newFile.inputStream()` if it exists, else null.
- **Observed crash path:** the sheet calls `takeScreenshot(...)!!`. If mpv did not write the file (no video frame, paused in an invalid state, or cache dir missing), the `!!` throws `NullPointerException`.
- Each call writes the same `mpv_screenshot.png` path, so Share and Save while another share is pending can race (observed).

### 10.4 Save, share and set-as-art (app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1847-1930, summarized here, owned by part 1)

- `saveImage(imageStream, timePos)`, `shareImage(imageStream, timePos)`, `setAsArt(artType, imageStream)`. `setAsArt` runs `anime.editCover(...)`, `anime.editBackground(...)` or `episode.editThumbnail(...)`, then returns `SetAsArt.Success` when the anime is local or favorite, else `SetAsArt.AddToLibraryFirst`; any exception gives `SetAsArt.Error`. The result is an `Event.SetArtResult(result, artType)`.
- `SetAsArt` enum (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:29-33`): `Success, AddToLibraryFirst, Error`.
- `ArtType` enum (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:35-39`): `Cover, Background, Thumbnail`.

**Proposal:** Share on Linux: the XDG Desktop Portal `OpenURI` / file save via `gtk::FileDialog`. "Save" default location: the existing image save directory from area:43/13, file name `<title>_<seconds>.png` (owned by part 1 for the exact naming). Set-as-art writes through area:34 covers cache.

---

## 11. Subtitle delay panel and the tap-to-sync timer (`SubtitleDelayPanel.kt`, `DelayCard`)

### 11.1 Enums

- `SubtitleDelayType` (lines 194-200): `Primary(AYMR.strings.player_sheets_sub_delay_subtitle_type_primary)` ("Primary"), `Secondary(...secondary)` ("Secondary"), `Both(...primary_and_secondary)` ("Both").
- `DelayType` (lines 370-373): `Audio`, `Subtitle`.

### 11.2 `SubtitleDelayPanel` state and effects (lines 75-146)

Container: `ConstraintLayout(Modifier.fillMaxSize().padding(padding.medium))`; the card is `linkTo(parent.top, parent.bottom, bias = 0.8f)` and `end.linkTo(parent.end)`. (No `BackHandler`; back is handled by the panel host.)

State:
- `affectedSubtitle` = `Primary` (remembered).
- `delay: Int` = `(MPVLib.getPropertyDouble("sub-delay") * 1000).roundToInt()` at composition.
- `secondaryDelay: Int` = `(MPVLib.getPropertyDouble("secondary-sub-delay") * 1000).roundToInt()`.
- `speed: Float` = `MPVLib.getPropertyDouble("sub-speed").toFloat()` (no rounding).

Effects:
1. `LaunchedEffect(speed)`: `if (speed in 0.1f..1f) MPVLib.setPropertyDouble("sub-speed", speed.toDouble())`. **Observed:** speeds above `1.0` are not sent live (only `0.1..1` is). They are sent at startup from the saved preference (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:263`) and when "Set as default" saves them (range `0.1..10`).
2. `LaunchedEffect(delay, secondaryDelay)`: `finalDelay = (if (affected == Secondary) secondaryDelay else delay) / 1000.0`, then:
   - `Primary` → `sub-delay = finalDelay`.
   - `Secondary` → `secondary-sub-delay = finalDelay`.
   - `Both` (`else`) → `sub-delay = finalDelay` and `secondary-sub-delay = finalDelay`.
   - Note: in `Both` mode the value written is `delay` (primary state); `secondaryDelay` is ignored.
3. `LaunchedEffect(affectedSubtitle)`:
   - `secondaryDelay = (getPropertyDouble(if (affected == Both) "sub-delay" else "secondary-sub-delay") * 1000).toInt()` (truncation, unlike step 1).
   - `delay = (getPropertyDouble("sub-delay") * 1000).toInt()`.
   - Observed: on `Both`, `secondaryDelay` gets the primary value. Rounding differs from the initial composition (`roundToInt` vs `toInt`), so a value like `0.0999999` yields `99` at startup and `99` vs `100` depending on the path (observed edge).

`DelayCard` call (lines 117-144): `delay = if (affected == Secondary) secondaryDelay else delay`; `onDelayChange` writes `secondaryDelay` for Secondary, else `delay`; `speed`; `onSpeedChange = { speed = round(it * 1000) / 1000f }` (`kotlin.math.round` = round-half-even, then 3 decimals); `onApply` saves `preferences.subtitlesDelay().set(delay)` and, only if `speed in 0.1f..10f`, `preferences.subtitlesSpeed().set(speed)`; `onReset` sets `delay = 0`, `secondaryDelay = 0`, `speed = 1f`; `onClose = onDismissRequest`.

**Observed gap:** `onApply` never saves `secondaryDelay`. The preference `pref_subtitles_secondary_delay` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/SubtitlePreferences.kt:53`) is read at startup (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:264-267`) but nothing in this panel writes it, so the secondary delay is lost on restart. Also "Reset" zeroes `secondaryDelay` but does not save it.

### 11.3 `SubtitleDelayCard` (lines 149-192)

- Calls `DelayCard(delay, onDelayChange, onApply, onReset, title = { SubtitleDelayTitle(affectedSubtitle, onClose, onTypeChange) }, extraSettings = { if (affected == Primary) OutlinedNumericChooser(label = AYMR.strings.player_sheets_sub_delay_speed ("Speed"), value = speed, onChange = onSpeedChange, max = 10f, step = .01f, min = .1f) }, delayType = DelayType.Subtitle, modifier)`.
- Speed control is shown **only** when `affectedSubtitle == Primary`. For Secondary and Both there is no speed control, but the value is still applied to `sub-speed` through the shared effect (observed).

### 11.4 `DelayCard` (lines 204-318)

`DelayCard(delay: Int, onDelayChange: (Int)->Unit, onApply, onReset, title: @Composable, delayType: DelayType, modifier, extraSettings: ColumnScope.() -> Unit = {})`.

Layout: `Card(widthIn(max = CARDS_MAX_WIDTH), animateContentSize, colors = panelCardsColors())`, `Column(verticalScroll, padding horizontal medium / vertical small, spacing small)`:
1. `title()`.
2. `OutlinedNumericChooser(label = AYMR.strings.player_sheets_sub_delay_delay ("Delay"), value = delay, onChange = onDelayChange, step = 50, min = Int.MIN_VALUE, max = Int.MAX_VALUE, suffix = { Text(AYMR.strings.player_generic_unit_ms ("ms")) })`.
3. `Column(animateContentSize) { extraSettings() }`.
4. Tap-sync row (state below).
5. Apply/reset row.

### 11.5 Tap-to-sync state machine (exact)

State: `isDirectionPositive: Boolean?` (null = idle, true = positive run, false = negative run), `timerStart: Long?` (epoch millis), `finalDelay: Int`.

Button A (label: Audio → `AYMR.strings.player_sheets_sub_delay_audio_sound_heard` "Sound heard"; Subtitle → `player_sheets_sub_delay_subtitle_voice_heard` "Voice heard"):
- onClick: if idle, state := `delayType == Audio` (Audio: true / Subtitle: false); else state := null (stop).
- enabled iff `state != (delayType == Audio)`.

Button B (label: Audio → `AYMR.strings.player_sheets_sub_delay_sound_sound_spotted` "Sound spotted"; Subtitle → `player_sheets_sub_delay_subtitle_text_seen` "Text seen"):
- onClick: if idle, state := `delayType != Audio` (Audio: false / Subtitle: true); else state := null (stop).
- enabled iff `state != (delayType == Subtitle)`.

Resulting behavior (derived from the two rules above; this is the exact behavior):

| Delay type | Press A from idle | Press B from idle | A enabled while running | B enabled while running |
|---|---|---|---|---|
| Audio | start **positive** (Sound heard) | start **negative** (Sound spotted) | only when the run is negative (A stops it) | only when the run is positive (B stops it) |
| Subtitle | start **negative** (Voice heard) | start **positive** (Text seen) | only when the run is positive (A stops it) | only when the run is negative (B stops it) |

So a run always starts at one button and is ended by the other button. The sign is fixed by the starting button.

Written precisely per button:
- **Audio**
  - A "Sound heard": idle → positive. Enabled when state ∈ {idle, negative}. Pressing while negative stops the run.
  - B "Sound spotted": idle → negative. Enabled when state ∈ {idle, positive}. Pressing while positive stops the run.
  - Net: a run starts at one button and ends at the other. Starting at A gives positive (delay increases); starting at B gives negative.
- **Subtitle**
  - A "Voice heard": idle → negative. Enabled when state ∈ {idle, positive}. Pressing while positive stops the run.
  - B "Text seen": idle → positive. Enabled when state ∈ {idle, negative}. Pressing while negative stops the run.
  - Net: starting at A gives negative (delay decreases); starting at B gives positive.

Run effect (`LaunchedEffect(isDirectionPositive)`, lines 249-263):
- On `null`: `onDelayChange(finalDelay)` and return. This also runs once on first composition with `finalDelay = delay` (no change).
- On non-null: `finalDelay = delay` (the current parent value at start); `timerStart = System.currentTimeMillis()`; `startingDelay = finalDelay`; loop `while (state != null && timerStart != null)`: `elapsed = now - timerStart`; `finalDelay = startingDelay + (if positive) elapsed else -elapsed`; `delay(20)`.
- So the delta is `±elapsed ms` where `elapsed` is wall-clock milliseconds since start, sampled every 20 ms. The final value is applied only on stop.

**Observed:** while a run is in progress the numeric field keeps showing the starting `delay`; the live value is held in `finalDelay` and is shown only after stop. The `Set as default` and `Reset` buttons are disabled while a run is active (`enabled = isDirectionPositive == null`).

**Proposal:** keep the truth table exactly. Show the live value during the run (improvement, flagged; default to the Kotlin behavior if the parity owner objects). Tick with a 20 ms `glib::timeout_add_local` that recomputes `start + sign * elapsed` from `Instant` (not from a tick counter).

### 11.6 Apply and reset row (lines 299-315)

- `Button(onApply, weight 1f, enabled = state == null)` text `AYMR.strings.player_sheets_delay_set_as_default` ("Set as default").
- `FilledIconButton(onReset, enabled = state == null) { Icon(Refresh) }`.

### 11.7 `SubtitleDelayTitle` (lines 321-368)

- Row `fillMaxWidth`, vertical alignment Bottom, spacing `extraSmall`.
- `Text(AYMR.strings.player_sheets_sub_delay_title ("Subtitle delay"), headlineMedium)`.
- Clickable `Row` (`clickable { showDropDownMenu = true }`) with `Text(stringResource(affectedSubtitle.title), color = onSurfaceVariant, bodyMedium)`, `Icon(ArrowDropDown)`, and a `DropdownMenu` listing `SubtitleDelayType.entries` (Primary, Secondary, Both). Selecting an item calls `onTypeChange(it)` and closes the menu.
- `Spacer(weight 1f)`, then `IconButton(onClose) { Icon(Close, size 32.dp) }`.

### 11.8 Persisted values for this panel

- `SubtitlePreferences.subtitlesDelay()` = `getInt("pref_subtitles_delay", 0)` (ms).
- `SubtitlePreferences.subtitlesSpeed()` = `getFloat("pref_subtitles_speed", 1f)`.
- `SubtitlePreferences.subtitlesSecondaryDelay()` = `getInt("pref_subtitles_secondary_delay", 0)` (read only at startup, never written by this panel; see gap above).

---

## 12. Subtitle settings container (`SubtitleSettingsPanel.kt`)

- `BackHandler(onBack = onDismissRequest)` (line 63). Back closes the panel.
- `orientation = LocalConfiguration.current.orientation`.
- Cards: `0 -> SubtitleSettingsTypographyCard`, `1 -> SubtitleSettingsColorsCard`, `2 -> SubtitlesMiscellaneousCard`, `else -> {}`.
- `pagerState = rememberPagerState { 3 }`.

**Portrait** (`orientation == ORIENTATION_PORTRAIT`, lines 78-110): `Column` constrained to `top.linkTo(parent.top, 32.dp)`, `start.linkTo(parent.start)`, spacing `extraSmall`:
- `TopAppBar(title = Text(AYMR.strings.player_sheets_subtitles_settings_title ("Subtitles settings"), headlineMedium with Shadow(blurRadius = 20f)), navigationIcon = IconButton(onDismissRequest) { AutoMirrored ArrowBack }, colors = topAppBarColors().copy(containerColor = Color.Transparent))`.
- `HorizontalPager(state, pageSize = PageSize.Fixed(screenWidthDp.dp * 0.9f), verticalAlignment = Top, pageSpacing = padding.small, contentPadding = PaddingValues(horizontal = padding.small), beyondViewportPageCount = 1) { page -> cards(page, Modifier.fillMaxWidth()) }`.

**Landscape** (lines 111-141): `Column(horizontalAlignment = Start, spacing small, constrainAs top.linkTo(parent.top), end.linkTo(parent.end, 32.dp), verticalScroll)`:
- `Spacer(16.dp)`, then `Row(width(CARDS_MAX_WIDTH), SpaceBetween)` with the title (same shadow) and `IconButton(onDismissRequest) { Icon(Close) }`.
- `repeat(3) { cards(it, Modifier) }`, then `Spacer(16.dp)`.

Observed: the portrait top bar has a back arrow; landscape has a close icon. Both call `onDismissRequest`.

**Proposal:** one layout for desktop: a `gtk::Notebook`-like `gtk::StackSwitcher` with three tabs (Typography, Colors, Miscellaneous), each a `ScrolledWindow`. Width breakpoint is not needed.

---

## 13. Typography card (`SubtitleSettingsTypographyCard.kt`, lines 85-352)

### 13.1 Fonts list (lines 88-117)

- `fontsDir = storageManager.getFontsDirectory()` (`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:77`) = `<baseDir>/mpv-config/fonts` (created if missing: `createDirectory` has a side effect, run on every composition). Base directory constants: `MPV_CONFIG_PATH = "mpv-config"`, `FONTS_PATH = "fonts"` (`domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt:102-103`).
- `fonts` starts as `mutableListOf(preferences.subtitleFont().defaultValue())` = `["Sans Serif"]`.
- `fontsLoadingIndicator` starts as a spinner `CircularProgressIndicator(Modifier.size(32.dp))`.
- `LaunchedEffect(Unit)`:
  - If `fontsDir == null`: indicator := null, return.
  - `withContext(Dispatchers.IO)`: `fontsDir.listFiles()?.filter { file.name?.lowercase()?.matches(FONT_EXTENSION_REGEX) == true }?.mapNotNull { runCatching { TTFFile.open(it.openInputStream()).families.values.first() }.getOrNull() }?.let { fonts.addAll(it.distinct()) }`; then indicator := null.
- `FONT_EXTENSION_REGEX = Regex(""".*\.[ot]tf${'$'}""")`: matches names ending in `.ttf` or `.otf` (case-insensitive because the name is lowercased first). `.ttc`, `.woff`, `.otc` are ignored.
- Family name: `TTFFile.open(stream).families.values.first()` (external library `com.yubyf.truetypeparser`). Files that fail to parse are skipped silently.
- `distinct()` removes duplicates among the discovered families only. A discovered family equal to `"Sans Serif"` appears twice (observed).
- **Observed concurrency issue:** `fonts` is a `MutableList` captured by `remember { mutableStateOf(...) }` and mutated in place. The UI does not recompose on `addAll`; it updates only when another state changes (the indicator becomes null, which does trigger recomposition, so in practice the list appears together with the indicator disappearing).

### 13.2 State init (lines 134-161)

From mpv (not from the preference):
- `isBold = getPropertyBoolean("sub-bold")`, `isItalic = getPropertyBoolean("sub-italic")`.
- `justify = SubtitleJustification.entries.first { it.value == getPropertyString("sub-justify") }`. Throws `NoSuchElementException` if mpv returns a value not in `{left, center, right, auto}` (observed).
- `font = getPropertyString("sub-font")`, `fontSize = getPropertyInt("sub-font-size")`.
- `borderStyle = SubtitlesBorderStyle.entries.first { it.value == getPropertyString("sub-border-style") }` (same throw on unknown value).
- `borderSize = getPropertyInt("sub-border-size")`, `shadowOffset = getPropertyInt("sub-shadow-offset")`.

### 13.3 Toolbar row (lines 162-237)

`Row(fillMaxWidth, horizontalScroll, padding start extraSmall end medium, vertical center)`:

1. **Bold** `IconToggleButton(checked = isBold, onCheckedChange = { isBold = it; preferences.boldSubtitles().set(it); MPVLib.setPropertyBoolean("sub-bold", it) })`, `Icon(FormatBold, size 32.dp)`.
2. **Italic** same pattern with `italicSubtitles` and `sub-italic`, `Icon(FormatItalic, 32.dp)`.
3. **Justification**: `SubtitleJustification.entries.minus(Auto).forEach { justification -> IconToggleButton(checked = justify == justification, onCheckedChange = { justify = justification; MPVLib.setPropertyBoolean("sub-ass-justify", it); if (it) { prefs.subtitleJustification().set(justification); MPVLib.setPropertyString("sub-justify", justification.value) } else { prefs.subtitleJustification().set(Auto); MPVLib.setPropertyString("sub-justify", "auto") } }) { Icon(justification.icon) } }`.
   - Three toggles in order: Left (`FormatAlignLeft`), Center (`FormatAlignCenter`), Right (`FormatAlignRight`).
   - **Observed inconsistency:** `justify = justification` is set regardless of `it`. Unchecking a checked toggle (`it == false`) writes `auto` to mpv and the preference, but `justify` still equals the toggle, so the button stays visually checked until the sheet is reopened.
4. `Spacer(weight 1f)`.
5. **Reset** `TextButton(onClick = {...}) { Row { Icon(FormatClear); Text(MR.strings.action_reset) } }`: calls `resetTypography(preferences)`, then re-reads `isBold`, `isItalic`, `justify`, `font`, `fontSize`, `borderStyle`, `borderSize`, `shadowOffset` from mpv.

### 13.4 Font dropdown (lines 238-259)

`Row(padding horizontal medium, spacing medium, vertical center)`:
- `Icon(painterResource(R.drawable.outline_brand_family_24), size 32.dp)`.
- `ExposedTextDropDownMenu(selectedValue = font, options = fonts.toImmutableList(), label = AYMR.strings.player_sheets_sub_typography_font ("Font"), onValueChangedEvent = { font = it; prefs.subtitleFont().set(it); MPVLib.setPropertyString("sub-font", it) }, leadingIcon = fontsLoadingIndicator)`.
- `options` is a snapshot at composition time (`toImmutableList()`), so families discovered after the list snapshot appear on the next recomposition.

### 13.5 Font size (lines 260-273)

`SliderItem(label = AYMR.strings.player_sheets_sub_typography_font_size ("Font size"), max = 100, min = 1, value = fontSize, valueText = fontSize.toString(), onChange = { fontSize = it; prefs.subtitleFontSize().set(it); MPVLib.setPropertyInt("sub-font-size", it) }) { Icon(FormatSize) }`.

### 13.6 Border style (lines 275-325)

- Clickable `Row` (`fillMaxWidth`, `clickable { selectingBorderStyle = !selectingBorderStyle }`, padding horizontal medium, vertical small, spacing large) with `Icon(BorderStyle)` and `Column` of `Text(AYMR.strings.player_sheets_sub_typography_border_style ("Border style"), bodyLarge)` and `Text(stringResource(borderStyle.titleRes), bodyMedium)`.
- `DropdownMenu(expanded, onDismissRequest = false)` of `SubtitlesBorderStyle.entries`: `DropdownMenuItem(text, onClick = { borderStyle = it; prefs.borderStyleSubtitles().set(it); MPVLib.setPropertyString("sub-border-style", it.value); close }, trailingIcon = Check when selected)`.
- `SubtitlesBorderStyle` (lines 368-375): `OutlineAndShadow("outline-and-shadow", AYMR.strings.player_sheets_subtitles_border_style_outline_and_shadow)` ("Outline and shadow"), `OpaqueBox("opaque-box", ...opaque_box)` ("Opaque box"), `BackgroundBox("background-box", ...background_box)` ("Background box").

### 13.7 Border size and shadow (lines 326-349)

- `SliderItem(label = AYMR.strings.player_sheets_sub_typography_border_size ("Border size"), value = borderSize, valueText, onChange = { borderSize = it; prefs.subtitleBorderSize().set(it); MPVLib.setPropertyInt("sub-border-size", it) }, max = 100, icon = { Icon(BorderColor) })`. Min is `0` (default).
- `SliderItem(label = AYMR.strings.player_sheets_subtitles_shadow_offset ("Shadow offset"), value = shadowOffset, ..., onChange = { shadowOffset = it; prefs.shadowOffsetSubtitles().set(it); MPVLib.setPropertyInt("sub-shadow-offset", it) }, max = 100, icon = { Icon(painterResource(R.drawable.sharp_shadow_24)) })`. Min `0`.

### 13.8 `resetTypography(preferences)` (lines 356-366), exact

```
sub-bold              <- boldSubtitles().deleteAndGet()           (bool, default false)
sub-italic            <- italicSubtitles().deleteAndGet()         (bool, default false)
sub-ass-justify       <- overrideSubsASS().deleteAndGet()         (bool, default false)   <- NOTE: uses the ASS-override pref
sub-justify           <- subtitleJustification().deleteAndGet().value   (default Auto -> "auto")
sub-font              <- subtitleFont().deleteAndGet()            (default "Sans Serif")
sub-font-size         <- subtitleFontSize().deleteAndGet()        (default 55)
sub-border-size       <- subtitleBorderSize().deleteAndGet()      (default 3)
sub-shadow-offset     <- shadowOffsetSubtitles().deleteAndGet()   (default 0)
sub-border-style      <- borderStyleSubtitles().deleteAndGet().value    (default "outline-and-shadow")
```

**Observed bug:** `sub-ass-justify` is set from `overrideSubsASS` in reset, but the justify toggles set `sub-ass-justify` from the toggle state. Reset therefore changes `sub-ass-justify` to the ASS-override value, not to the justify default. Proposal: reset `sub-ass-justify` to the justify default (`false`) and keep ASS override under its own key.

### 13.9 Behavior summary

- Every write goes to both the preference (persistence) and mpv (live). Reset deletes preferences and re-applies the defaults.
- Defaults (see `p2-data-and-config.md`): font `"Sans Serif"`, font size `55`, border size `3`, shadow offset `0`, border style `outline-and-shadow`, bold/italic `false`, justify `auto`.

---

## 14. Colors card (`SubtitleSettingsColorsCard.kt`, lines 71-251)

### 14.1 Color encoding (lines 148-156)

- `Int.copyAsArgb(alpha = this.alpha, red = this.red, green = this.green, blue = this.blue) = (alpha shl 24) or (red shl 16) or (green shl 8) or blue`. Uses `androidx.core.graphics` extension properties (`Color.alpha` etc., each `(c >>> n) & 0xFF`).
- `Int.toColorHexString() = "#" + this.toHexString().uppercase()`. `Int.toHexString()` (Kotlin stdlib, `ExperimentalStdlibApi`) emits exactly 8 hex digits of the two's-complement int (for example `-1` → `ffffffff`). Result is `#AARRGGBB`, uppercase. Examples: white opaque `#FFFFFFFF`, black opaque `#FF000000`, transparent `#00000000`.
- mpv reads `#AARRGGBB` for `sub-color`, `sub-border-color`, `sub-back-color` (as written by the app).
- `getCurrentMPVColor: (SubColorType) -> Int = { getPropertyString(property)?.let { android.graphics.Color.parseColor(it.uppercase()) }!! }`. `Color.parseColor` accepts `#RRGGBB` and `#AARRGGBB` (and some named colors). mpv may return a format parseColor rejects, which throws `IllegalArgumentException`. The `!!` throws `NullPointerException` when mpv returns null (observed).

### 14.2 `SubColorType` (lines 158-178)

| Name | titleRes (English) | mpv property | preference (default) |
|---|---|---|---|
| `Text` | `player_sheets_subtitles_color_text` ("Text") | `sub-color` | `textColorSubtitles()` = `getInt("pref_text_color_subtitles", Color.White.toArgb())` → `-1` (`0xFFFFFFFF`) |
| `Border` | `player_sheets_subtitles_color_border` ("Border") | `sub-border-color` | `borderColorSubtitles()` = `getInt("pref_border_color_subtitles", Color.Black.toArgb())` → `-16777216` (`0xFF000000`) |
| `Background` | `player_sheets_subtitles_color_background` ("Background") | `sub-back-color` | `backgroundColorSubtitles()` = `getInt("pref_background_color_subtitles", Color.Transparent.toArgb())` → `0` (`0x00000000`) |

### 14.3 Card layout (lines 71-146)

- `isExpanded = true`. `ExpandableCard(isExpanded, title = Row { Icon(Palette); Text(AYMR.strings.player_sheets_sub_colors_title ("Colors")) }, modifier.widthIn(max = CARDS_MAX_WIDTH), colors = panelCardsColors())`.
- `currentColorType = SubColorType.Text`, `currentColor = getCurrentMPVColor(currentColorType)`; `LaunchedEffect(currentColorType)` re-reads `currentColor`.
- Toggle row `Row(fillMaxWidth, horizontalScroll, center, padding start extraSmall end medium)`:
  - Three `IconToggleButton`s (`checked = currentColorType == type`): Text → `Icons.Default.FormatColorText`, Border → `BorderColor`, Background → `FormatColorFill`.
  - `Text(stringResource(currentColorType.titleRes))`.
  - `Spacer(weight 1f)`.
  - `TextButton { Row { Icon(FormatColorReset); Text(MR.strings.action_reset) } }` → `resetColors(preferences, currentColorType)` then `currentColor = getCurrentMPVColor(type)`.
- `SubtitlesColorPicker(currentColor, onColorChange = { currentColor = it; currentColorType.preference(preferences).set(it); MPVLib.setPropertyString(currentColorType.property, it.toColorHexString()) })`.

### 14.4 `resetColors(preferences, type)` (lines 180-200)

`MPVLib.setPropertyString(<property>, preferences.<pref>().deleteAndGet().toColorHexString())`. Deletes the preference (value becomes its default) and writes the default color to mpv. Does not re-read the preference afterwards.

### 14.5 `SubtitlesColorPicker` (lines 208-251)

Four `TintedSliderItem`s (Int, `max = 255`, `min = 0`):
1. `AYMR.strings.player_sheets_sub_color_red` ("Red"), value `color.red`, tint `Color.Red`, `onChange = { onColorChange(color.copyAsArgb(red = it)) }`.
2. Green (`color.green`, `Color.Green`).
3. Blue (`color.blue`, `Color.Blue`).
4. Alpha (`AYMR.strings.player_sheets_sub_color_alpha` "Alpha", `color.alpha`, tint `Color.White`).

Each change re-encodes the full ARGB and calls `onColorChange`, which writes preference and mpv on every slider change (no debounce).

### 14.6 Persisted-value notes

- Alpha is honored by mpv via the `#AA` byte. Startup applies the three colors (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:278-283`) from the preferences.

**Proposal:** Linux color selection can use `gtk::ColorDialogButton` (GTK 4.10+) for the picker, with RGBA ↔ ARGB conversion at the boundary. Keep the three-target selector and the alpha channel. The Kotlin writes on every drag step; the port should coalesce writes to at most one every 50 ms (proposal, not parity).

---

## 15. Miscellaneous card (`SubtitleSettingsMiscellaneousCard.kt`, lines 57-156)

### 15.1 State and controls

- `ExpandableCard(isExpanded, title = Row { Icon(Tune); Text(AYMR.strings.player_sheets_sub_misc_title ("Miscellaneous")) }, ...)`.
- `overrideAssSubs` = `getPropertyString("sub-ass-override").also { println(it) } == "force"` (lines 74-76). **Observed:** a stray `println` writes the mpv value to stdout on every composition start. Port: drop the print.
- **Override ASS/SSA** `SwitchPreference(overrideAssSubs, onValueChange = { overrideAssSubs = it; prefs.overrideSubsASS().set(it); MPVLib.setPropertyString("sub-ass-override", if (it) "force" else "scale") }, content = Text(AYMR.strings.player_sheets_sub_override_ass ("Override ASS/SSA subtitles")))`.
- **Scale** `SliderItem(Float overload)`: label `AYMR.strings.player_sheets_sub_scale` ("Scale"), value `subScale` (initial `getPropertyDouble("sub-scale").toFloat()`), `valueText = subScale.toFixed(2).toString()` (`toFixed` from `PlaybackSpeedSheet.kt:178`), `max = 5f` (min default `0f`), icon `FormatSize`, `onChange = { subScale = it; prefs.subtitleFontScale().set(it); MPVLib.setPropertyDouble("sub-scale", it.toDouble()) }`.
- **Position** `SliderItem(Int overload)`: label `AYMR.strings.player_sheets_sub_position` ("Position"), initial `getPropertyInt("sub-pos")`, `valueText = subPos.toString()`, `max = 150` (min default `0`), icon `AlignVerticalCenter`, `onChange = { subPos = it; prefs.subtitlePos().set(it); MPVLib.setPropertyInt("sub-pos", it) }`.
- **Reset** `TextButton` (Row end, padding end medium, bottom medium) with `Icon(EditOff)` and `Text(MR.strings.action_reset)`:
  1. `prefs.subtitlePos().deleteAndGet()` → `subPos` and `sub-pos` (default `100`).
  2. `prefs.subtitleFontScale().deleteAndGet()` → `subScale` and `sub-scale` (default `1f`).
  3. `prefs.overrideSubsASS().deleteAndGet()` → `overrideAssSubs` (default `false`).
  4. `MPVLib.setPropertyString("sub-ass-override", "scale")` (comment: mpv's default is "scale").

### 15.2 Observed notes

- `sub-scale` slider allows `0`, which hides subtitles in mpv (observed range; not guarded).
- Reset re-applies `sub-ass-override` as `"scale"` regardless of the preference value (consistent with default `false`).
- `SubtitlesMiscellaneousCard` is the only subtitle card with an `ExpandableCard` argument order where `modifier` is positional after named arguments (style note, no behavior change).

---

## 16. Video filters panel (`VideoFiltersPanel.kt`)

### 16.1 Container (lines 59-79)

`ConstraintLayout(modifier.fillMaxSize().padding(padding.medium))` with the card constrained to `linkTo(parent.top, parent.bottom, bias = 0.8f)` and `end.linkTo(parent.end)`. No `BackHandler`.

### 16.2 `FiltersCard` (lines 81-149)

- `decoderPreferences = Injekt.get<DecoderPreferences>()` (remembered).
- `Card(colors = panelCardsColors(), widthIn(max = CARDS_MAX_WIDTH))`.
- Header `Row(fillMaxWidth, padding start medium, SpaceBetween, vertical center)`:
  - `Text(AYMR.strings.player_sheets_filters_title ("Filters"), headlineMedium)`.
  - Right `Row` (spacing extraSmall): `TextButton(onClick = { VideoFilters.entries.forEach { MPVLib.setPropertyInt(it.mpvProperty, it.preference(prefs).deleteAndGet()) } }) { Text(MR.strings.action_reset) }` and `ControlsButton(Icons.Default.Close, onClose)`.
- `LazyColumn`:
  - `items(VideoFilters.entries)`: `value by filter.preference(prefs).collectAsState()`; `SliderItem(Int overload, label = stringResource(filter.titleRes), value, valueText = value.toString(), onChange = { prefs.set(it); MPVLib.setPropertyInt(filter.mpvProperty, it) }, max = 100, min = -100)`.
  - `item { if (decoderPreferences.gpuNext().get()) return@item; Column(padding medium, fillMaxWidth, spacing medium, Start) { Icon(Icons.Outlined.Info); Text(AYMR.strings.player_sheets_filters_warning) } }`.

### 16.3 `VideoFilters` enum (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:141-170`)

| Enum | titleRes (English) | Preference getter | Preference key | Default | mpv property |
|---|---|---|---|---|---|
| `BRIGHTNESS` | `player_sheets_filters_brightness` ("Brightness") | `brightnessFilter()` | `pref_player_filter_brightness` | `0` (no default arg → Int zero) | `brightness` |
| `SATURATION` | `player_sheets_filters_Saturation` ("Saturation") | `saturationFilter()` | `pref_player_filter_saturation` | `0` | `saturation` |
| `CONTRAST` | `player_sheets_filters_contrast` ("Contrast") | `contrastFilter()` | `pref_player_filter_contrast` | `0` | `contrast` |
| `GAMMA` | `player_sheets_filters_gamma` ("Gamma") | `gammaFilter()` | `pref_player_filter_gamma` | `0` | `gamma` |
| `HUE` | `player_sheets_filters_hue` ("Hue") | `hueFilter()` | `pref_player_filter_hue` | `0` | `hue` |

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/player/settings/DecoderPreferences.kt:17-21` (`getInt("pref_player_filter_<name>")` with no default, so the default is `0`).

Observed: `getInt(key)` without a default uses the store's default for Int (0). Slider range is `-100..100`, steps 200 (Int overload: `steps = max - min`).

### 16.4 Warning

The warning is hidden when `DecoderPreferences.gpuNext()` is `true` (`getBoolean("pref_gpu_next", false)`). It is not collected as state, so toggling `gpu-next` while the panel is open does not update it (observed).

**Proposal:** the Linux renderer is `gpu-next` in most builds; keep the warning text and show it only when the selected video output is not `gpu-next`.

---

## 17. Startup application and persistence cross-check (OBSERVED, for parity testing)

The sheets write preferences that are re-applied at file load. The table lists which keys this part writes and where they are read at startup.

| Preference key (default) | Written by | Applied at startup by | mpv write at startup |
|---|---|---|---|
| `pref_subtitles_delay` (0) | SubtitleDelayPanel `onApply` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:262` | `sub-delay` (ms / 1000.0) |
| `pref_subtitles_speed` (1f) | SubtitleDelayPanel `onApply` (0.1..10 only) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:263` | `sub-speed` |
| `pref_subtitles_secondary_delay` (0) | **nobody in scope** | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:264-267` | `secondary-sub-delay` |
| `pref_sub_pos` (100) | Misc card | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:287` | `sub-pos` |
| `pref_sub_scale` (1f) | Misc card | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:288` | `sub-scale` |
| `pref_override_subtitles_ass` (false) | Misc card | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:270-272` (only `force` + `sub-ass-justify yes` when true) | `sub-ass-override`, `sub-ass-justify` |
| `pref_subtitle_font` ("Sans Serif") | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:269` | `sub-font` |
| `pref_subtitles_font_size` (55) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:274` | `sub-font-size` |
| `pref_bold_subtitles` (false) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:275` | `sub-bold` (`yes`/`no`) |
| `pref_italic_subtitles` (false) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:276` | `sub-italic` (`yes`/`no`) |
| `pref_sub_justify` (auto) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:277` | `sub-justify` |
| `pref_sub_border_size` (3) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:284` | `sub-border-size` |
| `sub_shadow_offset` (0) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:286` | `sub-shadow-offset` |
| `pref_border_style_subtitles` (outline-and-shadow) | Typography | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:285` | `sub-border-style` |
| `pref_text_color_subtitles` (-1) | Colors | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:278` | `sub-color` |
| `pref_border_color_subtitles` (-16777216) | Colors | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:283` | `sub-border-color` |
| `pref_background_color_subtitles` (0) | Colors | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:279-282` | `sub-back-color` |
| `pref_try_hwdec` (true) (not in this part's Kotlin, read by AniyomiMPVView) | More sheet decoder chips do **not** write it | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:118` (`hwdec` = `auto` or `no`) | `hwdec` |
| `pref_audio_pitch_correction` (true) | Speed sheet | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:257` | `audio-pitch-correction` |
| `pref_player_speed` (1f) | Speed sheet "Make default" | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:150` | `speed` |
| `default_speed_presets` (12 values) | Speed sheet | not applied at startup (UI only) | none |
| `pref_audio_config` (AutoSafe) | More sheet | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:586` | `audio-channels` |
| `pref_player_statistics_page` (0) | More sheet | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:163-169` | `script-binding` stats commands |
| `pref_player_filter_brightness/saturation/contrast/gamma/hue` (0) | Video filters | not in the listed range (verify in AniyomiMPVView setup, out of scope) | `brightness`, `saturation`, `contrast`, `gamma`, `hue` |
| `pref_gpu_next` (false) | (outside scope) | (part 1) | (renderer) |
| `pref_panel_opacity` (60) | (part 1 settings) | read live by `panelCardsColors` | none |

Observed notes:
- The hwdec decoder chip writes **no** preference (section 7.4), so there is no startup key for the decoder in this part.
- The subtitle `sub-ass-justify` at startup is set to `yes` only when `pref_override_subtitles_ass` is true, while the justify toggles set it independently. A user who picked a justification but kept ASS override off gets `sub-justify` applied at startup without `sub-ass-justify` (observed).

---

## 18. Message keys (summary; English values in p2-data-and-config.md §6)

All keys used by the 14 files (78 distinct, including `MR.strings.*`, `AYMR.strings.*`, `AYMR.plurals.*`) are listed in `p2-data-and-config.md` section 6 with their English text.

---

## 19. Interactions with other areas

| Other area | What this part needs | Where it is defined |
|---|---|---|
| area:45-player-core (part 1) | `PlayerViewModel` state flows (`VideoTrack`, chapters, hosters, speed, sleep timer), `Sheets`/`Panels` state, `takeScreenshot`, `setAsArt`, `updateDecoder`, `CustomButton` execution, `AudioDelayPanel` (reuses `DelayCard`) | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/panels/AudioDelayPanel.kt` |
| area:02-preferences-system | `PreferenceStore` getters, `deleteAndGet`, `defaultValue`, `getEnum` | `core/common/src/main/java/tachiyomi/core/common/preference` |
| area:16-source-api-anime | `Video`, `Video.State`, `Hoster`, `Hoster.NO_HOSTER_LIST` | `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/` |
| area:13-core-util-storage-archive-torrent | `StorageManager.getFontsDirectory`, `UniFile` | `domain/src/main/java/tachiyomi/domain/storage/service/StorageManager.kt` |
| area:34-images-covers-cache | cover/background/thumbnail edit used by `setAsArt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:1915` and callees |
| area:43-reader-core | image save notification and file naming shared with manga | `SaveImageNotifier` (per 45 inventory) |
| area:14-source-local-and-metadata | `isLocalSource` gating of thumbnail art | `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt` |
| area:07-domain-entries-items | episode sort, `EpisodeSorter` | not used directly in scope |

---

## 20. Concurrency and timing (OBSERVED)

- Composition runs on the main thread. `MPVLib` calls are synchronous JNI calls on the main thread in every composable in scope.
- `LaunchedEffect` coroutines run on the main dispatcher; the only background work is the font scan (`Dispatchers.IO`, `SubtitleSettingsTypographyCard.kt:105`).
- `DelayCard` tap-sync ticks every `20 ms` (`delay(20)`, line 261). The elapsed time is sampled with `System.currentTimeMillis()`.
- `PlayerSheet` animations: 350 ms (`sheetAnimationSpec = tween<Float>(350)`); scrim alpha 0.5.
- Slider change writes: unthrottled (each `onValueChange` call writes preferences and mpv).
- `PlaybackSpeedSheet` `speed` changes are forwarded directly to the caller (no debounce).
- `Sheets` `dismissEvent` and `PlayerSheet` close: a close animation of 350 ms precedes `onDismissRequest`.

---

## 21. Edge cases and error paths (OBSERVED, consolidated)

1. `takeScreenshot(...)!!` in ScreenshotSheet: NPE when mpv does not write the file (sheet lines 83, 91, 118-122).
2. `getCurrentMPVColor` `!!` and `parseColor` throw on unexpected mpv values (Colors card lines 202-206).
3. `Subtitle justification` and `border style` initial state use `first { ... }`: `NoSuchElementException` on unknown mpv strings (Typography lines 136-151, 221-226).
4. `getTrackTitle` with null `language` and blank name yields `#id: ` (rule 5).
5. Chapters: `chapter.name` may be null from mpv (`chapter-list/N/title` can be absent) → formatted as `null` (observed; `IndexedSegment.name` is non-null Kotlin, so the platform type hides a null until formatting).
6. `Decoder` unknown `hwdec-current` string → `getDecoderFromValue` throws (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt:84-86`).
7. `QualitySheet` `hoster.videoState[videoIdx]` index mismatch (videoList and videoState have different lengths) → `IndexOutOfBoundsException` (observed; the sheet assumes equal lengths).
8. `SpeedSheet` `playbackSpeedPresets.map { it.toFloat() }`: invalid stored string → `NumberFormatException` (observed).
9. `OutlinedNumericChooser` text input `"-"` → `-0` → `onChange(0)` (observed; the field then shows `0`).
10. Subtitle delay `Secondary` with a run in progress and a type change: the run's `finalDelay` belongs to the previous target; switching type is disabled only through the buttons, not via the dropdown (observed: the dropdown is not disabled while a run is active).
11. Typography `fonts` snapshot race (section 13.1).
12. More sheet audio channel `af=""` clears all audio filters (section 7.3).
13. Sleep timer: `remainingTime` > 24 h passed to `rememberTimePickerState` without clamping.
14. Duplicate chapter equality → shared index (section 4.4).
15. Speed presets remove by canonical string only (section 6.3).

---

## 22. Behavior equivalence checks to carry into Rust

- Subtitle delay ms ↔ seconds conversions use `/ 1000.0` (double) and `* 1000` on read; the port must use `f64` seconds in mpv and integer ms in the model, with the same rounding choice on read (`round`, not `trunc`) everywhere. Kotlin is inconsistent (section 11.2); the port picks `round`.
- Speed rounding: `round(x * 1000) / 1000` with round-half-even (`kotlin.math.round`); the port uses `f32::round_ties_even`.
- Presets: `toFixed(2)` uses half-up; the port uses the same half-up rule to keep preset strings identical with existing settings files.
- Colors: `#AARRGGBB` uppercase, 8 digits.
