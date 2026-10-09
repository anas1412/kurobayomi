# Area 47b — Behavior specification

Paths are repo-relative to `/home/blackbox/aniyomi-src`. Line numbers are `path:line` in that tree. OBSERVED sections describe the Kotlin. PROPOSAL sections describe what Kurobayomi should do. Every "Proposal:" label is explicit.

Section map: §A settings page model and dispatch · §B widgets (B.1–B.14) · §C search highlight · §T theme pickers · §M More tab · §U update dialog · §O onboarding wrapper · §L logo header · §X cross-cutting (strings, dispatch, error paths).

---

## §A. Settings page model, scaffold and dispatch (OBSERVED)

### A.1 Model (`app/src/main/java/eu/kanade/presentation/more/settings/Preference.kt`)
- `sealed class Preference` (line 26) has two abstract members: `title: String` and `enabled: Boolean`.
- `Preference.PreferenceItem<T>` (line 30) is a sealed subclass with abstract `subtitle: String?`, `icon: ImageVector?`, `onValueChanged: suspend (value: T) -> Boolean`. `onValueChanged` is a veto hook: the value is written only when it returns `true`.
- `Preference.PreferenceGroup(title, enabled = true, preferenceItems: ImmutableList<PreferenceItem<out Any>>)` (line 255). A group is not itself a `PreferenceItem`; it is a top-level entry.
- Concrete item variants and their defaults (line numbers from Preference.kt):

| Variant | Lines | Constructor parameters and defaults | Subtitle default | Value source |
|---|---|---|---|---|
| `TextPreference` | 38-46 | title, subtitle=null, icon=null, enabled=true, onClick: (() -> Unit)? = null | from param (null) | none (static) |
| `SwitchPreference` | 51-59 | preference: PreferenceData<Boolean>, title, subtitle=null, enabled=true, onValueChanged={true} | param | bound `PreferenceData<Boolean>`; icon is always null |
| `SliderPreference` | 64-74 | value: Int, title, valueRange: IntProgression = 0..1, steps: Int = (last - first) - 1 (annotated `@IntRange(from = 0)`), subtitle=null, enabled=true, onValueChanged={true} | param | NOT bound: `value` is supplied by the caller; icon always null |
| `ListPreference<T>` | 80-97 | preference: PreferenceData<T>, entries: ImmutableMap<T,String>, title, subtitle="%s", subtitleProvider = `{ v, e -> subtitle?.format(e[v]) }`, icon=null, enabled=true, onValueChanged={true} | "%s" formatted with `entries[value]` | bound |
| `BasicListPreference` | 102-112 | value: String, entries: ImmutableMap<String,String>, title, subtitle="%s", subtitleProvider (same default), icon, enabled, onValueChanged | "%s" with `entries[value]` | NOT bound (caller holds value) |
| `MultiSelectListPreference` | 118-137 | preference: PreferenceData<Set<String>>, entries: ImmutableMap<String,String>, title, subtitle="%s", subtitleProvider (see B.5), icon, enabled, onValueChanged | see B.5 | bound |
| `EditTextPreference` | 142-150 | preference: PreferenceData<String>, title, subtitle="%s", enabled, onValueChanged | "%s" with the value | bound |
| `MultiLineEditTextPreference` | 155-163 | preference: PreferenceData<String>, canBeBlank=false, title, subtitle="%s", icon, enabled, onValueChanged | "%s" | bound |
| `MPVConfPreference` | 169-201 | preference: PreferenceData<String>, scope: CoroutineScope, context: Context, fileName: String?=null, canBeBlank=true, title, subtitle = first 2 lines of the value (see B.6), icon, enabled, onValueChanged = file writer (see B.14) | computed from value | bound |
| `EditTextInfoPreference` | 207-218 | preference: PreferenceData<String>, dialogSubtitle: String?, validate: (String)->Boolean={true}, errorMessage: @Composable ((String)->String)?=null, keyboardOptions=KeyboardOptions.Default, title, subtitle="%s", icon, enabled, onValueChanged | "%s" | bound |
| `TrackerPreference` | 223-233 | tracker: Tracker, login: ()->Unit, logout: ()->Unit | title is always `""`; subtitle null | derives from `tracker.isLoggedInFlow` |
| `InfoPreference` | 235-242 | title, enabled=true | subtitle null | none |
| `CustomPreference` | 244-252 | title, content: @Composable ()->Unit | subtitle null | none |

- `enabled` defaults to `true` on every variant. `title` for `TrackerPreference` is the constant empty string (line 228).

### A.2 Scaffold (`PreferenceScaffold.kt`, lines 10-33)
- `PreferenceScaffold(titleRes: StringResource, actions: @Composable RowScope.() -> Unit = {}, onBackPressed: (() -> Unit)? = null, itemsProvider: @Composable () -> List<Preference>)`.
- Renders `Scaffold { topBar = AppBar(title = stringResource(titleRes), navigateUp = onBackPressed, actions, scrollBehavior = it); content = PreferenceScreen(items = itemsProvider(), contentPadding) }`.
- `navigateUp = null` means no back arrow is shown (the app bar decides; see area:36 for AppBar).

### A.3 Screen body (`PreferenceScreen.kt`, lines 25-83)
1. `state = rememberLazyListState()`; `highlightKey = SearchableSettings.highlightKey` read once per composition (line 32).
2. If `highlightKey != null`: `LaunchedEffect(Unit)` does `i = items.findHighlightedIndex(highlightKey)`; if `i >= 0` it waits `0.5.seconds` then `state.animateScrollToItem(i)`; then ALWAYS sets `SearchableSettings.highlightKey = null` (lines 34-41). The reset happens even when `i < 0`.
3. `ScrollbarLazyColumn(modifier, state, contentPadding)` renders the list. For each top-level `Preference` at index `i`:
   - `PreferenceGroup`: if `!enabled` the group is skipped completely (`return@fastForEachIndexed`, line 53). Otherwise emits: header item (`PreferenceGroupHeader(title)`), one `PreferenceItem` per entry in `preferenceItems`, then a spacer item of `12.dp` **only when `i < items.lastIndex`** (lines 66-70).
   - `PreferenceItem` (standalone): emits one `PreferenceItem` row.
4. Visual vertical spacing between groups is therefore the group header's `top = 14dp` plus the 12dp spacer.

### A.4 findHighlightedIndex (lines 85-97)
- Builds a flat list of titles: for each top-level entry, a group contributes `[null (header), item1.title, ..., itemN.title, null (spacer)]`; a standalone item contributes `[item.title]`. Then `indexOfFirst { it == highlightKey }`.
- It does NOT check `enabled` on groups, so when a disabled group is skipped at render time the computed index is too large by the number of rows the group would have produced (see E-07).
- It counts disabled standalone items and disabled group items, which DO occupy a LazyColumn slot (the `AnimatedVisibility` wrapper still emits a node). Those are counted correctly.
- It matches on `title` only. `TrackerPreference.title == ""`, `InfoPreference` and `CustomPreference` titles can match, but their widgets do not read the highlight flag (see §C.4).

### A.5 Dispatch (`PreferenceItem.kt`, lines 58-237)
- `StatusWrapper(item, highlightKey, content)` (lines 37-55): `highlighted = item.title == highlightKey`; wraps in `AnimatedVisibility(visible = item.enabled, enter = expandVertically() + fadeIn(), exit = shrinkVertically() + fadeOut())`; content is inside `CompositionLocalProvider(LocalPreferenceHighlighted provides highlighted)`.
  - Note: `highlighted` is evaluated per item, so duplicate titles on one screen all flash.
- `PreferenceItem(item, highlightKey)` (lines 58-237) dispatches on the concrete variant with `when`:
  - `SwitchPreference`: collects `item.preference`; `onCheckedChanged(newValue)` runs `scope.launch { if (item.onValueChanged(newValue)) item.preference.set(newValue) }` (lines 75-81).
  - `SliderPreference`: `BaseSliderItem(label=title, value=item.value, valueRange, valueText = item.subtitle.takeUnless{ it.isNullOrEmpty() } ?: item.value.toString(), steps, labelStyle = titleLarge.copy(fontSize = TitleFontSize), onChange = { scope.launch { item.onValueChanged(it) } }, modifier = padding(horizontal = 16dp, vertical = 16dp))` (lines 84-102). The slider does NOT write any preference; the caller owns state.
  - `ListPreference<*>`: collects `item.preference`; `onValueChange(newValue) = scope.launch { if (item.internalOnValueChanged(newValue!!)) item.internalSet(newValue) }` (lines 103-119). `subtitle = item.internalSubtitleProvider(value, item.entries)`.
  - `BasicListPreference`: `onValueChange = { scope.launch { item.onValueChanged(it) } }` (lines 120-129). No write.
  - `MultiSelectListPreference`: collects; `onValuesChange(newValues) -> if (item.onValueChanged(newValues)) item.preference.set(newValues.toMutableSet())` (lines 130-143).
  - `TextPreference`: `TextPreferenceWidget(title, subtitle, icon, onPreferenceClick = item.onClick)` (lines 144-151).
  - `EditTextPreference`: `EditTextPreferenceWidget(title, subtitle, icon, value, onConfirm = { accepted = item.onValueChanged(it); if (accepted) item.preference.set(it); accepted })` (lines 152-165).
  - `MultiLineEditTextPreference`: same as EditText, plus `singleLine = false`, `canBeBlank = item.canBeBlank` (lines 166-181).
  - `MPVConfPreference`: same as multi-line plus `formatSubtitle = false` (lines 182-198).
  - `EditTextInfoPreference`: `dialogSubtitle`, `singleLine = true`, `canBeBlank = true` (hard-coded, line 213), `validate`, `errorMessage`, `keyboardOptions` (lines 199-218).
  - `TrackerPreference`: `isLoggedIn by tracker.isLoggedInFlow.collectAsState(tracker.isLoggedIn)`; click: `if (isLoggedIn) item.logout() else item.login()` (lines 219-228).
  - `InfoPreference`: `InfoWidget(text = item.title)` (lines 229-231).
  - `CustomPreference`: `item.content()` (lines 232-234).
- Failure behaviour: each `scope.launch { }` runs the suspend veto in a `rememberCoroutineScope()`. An exception inside `onValueChanged` cancels that coroutine silently; no UI error is shown. Proposal: surface a toast/`adw`-free `gtk::AlertDialog` on error (see §X.3).

### A.6 Disabled items
- `enabled = false` does not remove an item from its group's `ListBox` in Compose terms; `AnimatedVisibility` animates it out. Disabled groups are removed entirely (A.3.3).
- Proposal: on GTK, use `gtk::Revealer` with `transition-type = slide-down` (150 ms) for items and do not render disabled groups at all, to match A.3.3.

---

## §B. Widgets

### B.1 Base row (`widget/BasePreferenceWidget.kt`, lines 39-87)
- Signature: `BasePreferenceWidget(modifier, title: String?, subcomponent: (@Composable ColumnScope.() -> Unit)?, icon: (@Composable () -> Unit)?, onClick: (() -> Unit)?, widget: (@Composable () -> Unit)?)`.
- Layout (a `Row`, vertically centred, `fillMaxWidth`, `sizeIn(minHeight = LocalPreferenceMinHeight)` where the default min height is `56.dp`, line 122/34):
  1. Leading icon box: `padding(start = 16dp, end = 8dp)` (line 59) if `icon != null`.
  2. Text column: `weight(1f)`, `padding(vertical = 16dp)`.
     - Title (if `!title.isNullOrBlank()`): `Text` with `horizontal = 16dp` padding, `overflow = Ellipsis`, `maxLines = 2`, style `titleLarge`, `fontSize = 16.sp` (lines 68-76).
     - Subcomponent (subtitle or custom body) directly below.
  3. Trailing widget box: `padding(end = 16dp)` if `widget != null`.
- Clickable: `Modifier.clickable(enabled = onClick != null, onClick)` on the whole row (line 53). Rows without `onClick` are not focusable/clickable.
- Highlight: `Modifier.highlightBackground(highlighted)` applied to the row (line 51). Details in §C.2.
- Constants (lines 122-125): `TrailingWidgetBuffer = 16.dp`, `PrefsHorizontalPadding = 16.dp`, `PrefsVerticalPadding = 16.dp`, `TitleFontSize = 16.sp`.

### B.2 Text row (`widget/TextPreferenceWidget.kt`, lines 20-60)
- Parameters: modifier, title, subtitle, icon, iconTint = `MaterialTheme.colorScheme.primary` (line 25), widget, onPreferenceClick.
- Subtitle: shown only if `!subtitle.isNullOrBlank()`; `Text` with horizontal 16dp padding, `secondaryItemAlpha()` (alpha 0.78, `presentation-core/src/main/java/tachiyomi/presentation/core/util/Modifier.kt:39` and constant `SECONDARY_ALPHA = .78f` in `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt`), style `bodySmall`, `maxLines = 10`.
- Icon: `Icon(imageVector, tint = iconTint, contentDescription = null)` (lines 46-56).
- Delegates to B.1 with `onClick = onPreferenceClick`.

### B.3 Switch row (`widget/SwitchPreferenceWidget.kt`, lines 16-38)
- `checked: Boolean = false`, `onCheckedChanged: (Boolean) -> Unit` (required).
- Trailing widget: `Switch(checked, onCheckedChange = null, modifier = padding(start = 16dp))`. The switch is visual only.
- Row click calls `onCheckedChanged(!checked)` (line 36). The whole row is the hit target.
- Note: the caller passes the current `checked`; the row does not keep local state.

### B.4 Single-choice list (`widget/ListPreferenceWidget.kt`, lines 31-110)
- `fun <T> ListPreferenceWidget(value: T, title, subtitle, icon, entries: Map<out T, String>, onValueChange: (T) -> Unit)`.
- Row: `TextPreferenceWidget(title, subtitle, icon, onPreferenceClick = { isDialogShown = true })`.
- Dialog (`AlertDialog`, lines 48-80):
  - title: `Text(title)`.
  - text: `Box { ScrollbarLazyColumn(state) { for each entry: DialogRow(label = entry.value, isSelected = (value == entry.key), onSelected = { onValueChange(entry.key!!); isDialogShown = false }) } ; top HorizontalDivider if state.canScrollBackward; bottom HorizontalDivider if state.canScrollForward }`.
  - `confirmButton`: `TextButton("Cancel" = MR.strings.action_cancel)` closes the dialog. There is no OK button.
  - `dismissButton`: none.
- `DialogRow` (lines 84-110): `Row` with `clip(MaterialTheme.shapes.small)`, `selectable(selected, onClick = { if (!isSelected) onSelected() })`, `fillMaxWidth`, `minimumInteractiveComponentSize()` (48dp min touch), a `RadioButton(selected, onClick = null)`, label `bodyLarge` with `padding(start = 24dp)`.
- Behaviour details:
  - Clicking the already-selected row does nothing and the dialog stays open (line 95).
  - Clicking a different row calls `onValueChange(key)` then closes (line 63-64). There is no confirm step.
  - `entries.forEach { current -> ... }` iterates the map in insertion order; `ImmutableMap` preserves the order it was built with.
- Preference-bound path (`PreferenceItem.kt:103-119`): `onValueChange(newValue)` runs the veto `internalOnValueChanged`, then `internalSet` writes the preference.

### B.5 Multi-select list (`widget/MultiSelectListPreferenceWidget.kt`, lines 20-83)
- Row: `TextPreferenceWidget(title, subtitle = preference.subtitleProvider(values, preference.entries), icon, onPreferenceClick = { isDialogShown = true })`.
- Default `subtitleProvider` (`Preference.kt:124-133`): `combined = remember(v, e) { v.mapNotNull { e[it] }.joinToString().takeUnless { it.isBlank() } } ?: stringResource(MR.strings.none)`; result `subtitle?.format(combined)`.
  - `joinToString()` uses the default separator `", "`.
  - Ordering: follows the iteration order of `v: Set<String>`. The stored set has no defined order (E-12). Proposal: sort by `entries` order.
  - When `v` is empty, `combined = "None"` (`MR.strings.none`). With the default `subtitle = "%s"` the displayed text is `None`.
- Dialog (lines 34-82):
  - `selected = remember { entries.keys.filter { values.contains(it) }.toMutableStateList() }`. Initial selection order is entries order.
  - Body: plain `LazyColumn` (not `ScrollbarLazyColumn`, unlike B.4). For each entry, `LabeledCheckbox(label, checked = selected.contains(key), onCheckedChange = { add or remove })`.
  - Confirm `TextButton(MR.strings.action_ok)`: `onValuesChange(selected.toMutableSet())` then close.
  - Dismiss `TextButton(MR.strings.action_cancel)`: close, no write.
  - `DialogProperties(usePlatformDefaultWidth = true)`.
- `LabeledCheckbox` (`presentation-core/src/main/java/tachiyomi/presentation/core/components/LabeledCheckbox.kt:20-45`): row `clip(small)`, `fillMaxWidth`, `heightIn(min = 48dp)`, `clickable(role = Checkbox)` toggles `!checked`; `Checkbox(onCheckedChange = null)`; `horizontalArrangement = spacedBy(MaterialTheme.padding.small)` (8dp).

### B.6 Text-edit family (`widget/EditTextPreferenceWidget.kt`, lines 32-122)
- Parameters: title, subtitle, dialogSubtitle=null, icon, value, onConfirm: suspend (String) -> Boolean, singleLine=true, canBeBlank=false, formatSubtitle=true, validate: (String)->Boolean={true}, errorMessage: @Composable ((String)->String)?=null, keyboardOptions=KeyboardOptions.Default.
- Row: `TextPreferenceWidget(title, subtitle = if (formatSubtitle) subtitle?.format(value) else subtitle, icon, onPreferenceClick = { isDialogShown = true })` (lines 48-53).
- Dialog state: `isDialogShown` (local `remember`). Text state: `rememberSaveable(stateSaver = TextFieldValue.Saver) { TextFieldValue(value) }` (lines 58-60). The initial text is the stored value at open time.
- `AlertDialog` (lines 61-120):
  - title column: `Text(title)`; if `dialogSubtitle != null`, a second `Text(dialogSubtitle, style = bodyMedium)`.
  - text: `OutlinedTextField(value = textFieldValue, onValueChange, modifier = fillMaxWidth, singleLine, keyboardOptions, isError = errorCondition, supportingText, trailingIcon)`.
    - `errorCondition = (text.isBlank() && !canBeBlank) || !validate(text)` (lines 76, 90).
    - `trailingIcon`: if `errorCondition` → `Icon(Filled.Error)` (non-interactive); else `IconButton(onClick = { text = "" }) { Icon(Filled.Cancel) }` (lines 75-83). Note the clear button is absent in the error state.
    - `supportingText`: if `!validate(text) && errorMessage != null` → `Text(errorMessage(text))`. A blank-with-canBeBlank=false state shows the error icon but no supporting text (E-03).
  - `properties = DialogProperties(usePlatformDefaultWidth = true)`.
  - `confirmButton` `TextButton(MR.strings.action_ok)`: `enabled = text != value && (text.isNotBlank() || canBeBlank) && validate(text)` (lines 100-103). On click: `scope.launch { if (onConfirm(text)) onDismissRequest() }`.
  - `dismissButton` `TextButton(MR.strings.action_cancel)`.
- If `onConfirm` returns `false`, the dialog stays open with the typed text (no error message is shown by the widget itself).

Variants (PreferenceItem.kt dispatch):
- EditText (`EditTextPreference`): defaults above.
- MultiLine (`MultiLineEditTextPreference`): `singleLine = false`, `canBeBlank = item.canBeBlank` (default false).
- MPV config (`MPVConfPreference`): `singleLine = false`, `canBeBlank = item.canBeBlank` (default true), `formatSubtitle = false` (the subtitle is the first two lines of the value, already computed, line 176-181).
- EditTextInfo (`EditTextInfoPreference`): `dialogSubtitle`, `singleLine = true`, `canBeBlank = true`, `validate`, `errorMessage`, `keyboardOptions` from the item.

### B.7 Slider (`BaseSliderItem`, `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:237-280`, used by `PreferenceItem.kt:84-102`)
- Layout: `Column(fillMaxWidth, spacedBy(2dp))` with a `Row(spacedBy(8dp))` containing the label (weight 1, `labelStyle`) and a `Pill(text = valueText, style = bodyMedium, color = surfaceContainerHigh)`, then `Slider(value, onValueChange, valueRange, steps)`.
- `onValueChange`: `if (it == value) return; onChange(it); haptic.performHapticFeedback(TextHandleMove)` (lines 273-277). Haptics is Android-only (drop on Linux; see §X.4).
- `steps` default in the model is `(last - first) - 1` (Preference.kt:68). For `valueRange = 0..10` that is `9`, i.e. 10 discrete intervals plus the endpoints as the Compose semantics dictate.
- `valueText` on the preference path: `item.subtitle` if non-empty, else `item.value.toString()`.
- Padding applied by `PreferenceItem.kt:97-100` is `horizontal 16dp, vertical 16dp`. `SettingsItemsPaddings` (`SettingsItems.kt:75-78`, `Horizontal = 24.dp`, `Vertical = 10.dp`) is what the other slider entry point uses. The two paddings are inconsistent in the source; Proposal: use 16/16 everywhere in settings.
- Caller contract: the slider never writes a preference. The caller must re-render with a new `value` after `onValueChanged`.

### B.8 Tracker login row (`widget/TrackingPreferenceWidget.kt`, lines 26-63)
- `Box(highlightBackground)` > `Row(clickable(enabled = onClick != null), fillMaxWidth, padding(horizontal = 16dp, vertical = 8dp), verticalAlignment = Center)`.
  - `TrackLogoIcon(tracker)` (`app/src/main/java/eu/kanade/presentation/track/components/TrackLogoIcon.kt:22`).
  - Name: `Text(tracker.name, weight(1f), padding(horizontal = 16dp), maxLines = 1, titleLarge, fontSize = 16sp)`.
  - When `checked`: `Icon(Outlined.Done, padding(4dp), size(32dp), tint = Color(0xFF4CAF50), contentDescription = stringResource(MR.strings.login_success))` → English `"Logged in"`.
- Click (PreferenceItem dispatch): `if (isLoggedIn) logout() else login()`. No confirmation dialog in this widget.
- `isLoggedIn` is driven by `tracker.isLoggedInFlow` (`app/src/main/java/eu/kanade/tachiyomi/data/track/Tracker.kt:40`), initial value `tracker.isLoggedIn` (line 38).

### B.9 Info block (`widget/InfoWidget.kt`, lines 22-41)
- `Column(padding(horizontal = 16dp, vertical = MaterialTheme.padding.medium = 16dp), secondaryItemAlpha(), spacedBy(16dp))`, containing `Icon(Outlined.Info, contentDescription = null)` and `Text(text, style = bodySmall)`.
- Not interactive, not highlightable.
- Used by `PreferenceItem` for `InfoPreference` (title = the text).

### B.10 Group header (`widget/PreferenceGroupHeader.kt`, lines 14-28)
- `Box(CenterStart, fillMaxWidth, padding(bottom = 8dp, top = 14dp))` containing `Text(title, color = colorScheme.secondary, padding(horizontal = 16dp), bodyMedium)`.
- `title` is shown as given (no null case: `PreferenceGroup.title` is a non-null String).

### B.11 Tri-state list dialog (`widget/TriStateListDialog.kt`, lines 35-146)
- `private enum class State { CHECKED, INVERSED, UNCHECKED }`.
- Signature: `TriStateListDialog<T>(title, message: String?=null, items: List<T>, initialChecked: List<T>, initialInversed: List<T>, itemLabel: @Composable (T)->String, onDismissRequest, onValueChanged: (newIncluded: List<T>, newExcluded: List<T>) -> Unit)`.
- Initial state per item (lines 53-63): `in initialChecked` → CHECKED; else `in initialInversed` → INVERSED; else UNCHECKED. Checked wins if an item is in both lists. Membership uses `equals`.
- Message: if non-null, `Text(message, padding(bottom = 8dp))` above the list.
- Row (lines 81-116): `clip(small)`, `clickable { cycle }`, `defaultMinSize(minHeight = 48dp)`, `fillMaxWidth`, `Row(verticalAlignment = Center)`.
  - Cycle (lines 85-89): UNCHECKED → CHECKED; CHECKED → INVERSED; INVERSED → UNCHECKED.
  - Icon (lines 95-101), `padding(end = 20dp)`: UNCHECKED = `Icons.Rounded.CheckBoxOutlineBlank`; CHECKED = `Icons.Rounded.CheckBox`; INVERSED = `Icons.Rounded.DisabledByDefault`.
  - Tint (lines 102-106): UNCHECKED → `LocalContentColor.current`; CHECKED and INVERSED → `colorScheme.primary`.
  - contentDescription (lines 107-113): `stringResource(MR.strings.disabled)` for ALL three states. This is a source bug: CHECKED and INVERSED are mislabeled as "Disabled" for screen readers (E-04).
  - Label: `Text(itemLabel(item))`.
- `LazyColumn(state = listState)` with `HorizontalDivider` at top when `canScrollBackward` and bottom when `canScrollForward` (lines 120-121).
- Dismiss `TextButton(MR.strings.action_cancel)` → `onDismissRequest`.
- Confirm `TextButton` (no label key; the `MR.strings.action_ok` text is at line 142): `included = items where state == CHECKED (in original order)`, `excluded = items where state == INVERSED (in original order)`, then `onValueChanged(included, excluded)`. The dialog does not close itself; the caller must dismiss.
  - Note: lines 131-144 show the confirm button; `onValueChanged` is the only exit path besides dismiss.

### B.12 Custom preference
- `PreferenceItem.CustomPreference.content` is invoked as-is inside the row (`PreferenceItem.kt:232-234`). No padding or highlight is applied by this wrapper. Callers own the layout. Highlight does not apply (no `LocalPreferenceHighlighted` read).

### B.13 Basic list
- `BasicListPreference` renders through `ListPreferenceWidget` (B.4) with `subtitle = item.subtitleProvider(item.value, item.entries)` and `onValueChange = { scope.launch { item.onValueChanged(it) } }` (`PreferenceItem.kt:120-129`). The value is not written by this wrapper.

### B.14 MPV config file write (`Preference.kt:184-200`, `PreferenceItem.kt` MPV branch)
- `onValueChanged` (default for `MPVConfPreference`):
  1. If `fileName != null`: `storageManager = Injekt.get<StorageManager>()`.
  2. Only when `Build.VERSION.SDK_INT >= R && Environment.isExternalStorageManager()`: `inputFile = storageManager.getMPVConfigDirectory()?.createFile(fileName)`; open a file descriptor with mode `"rwt"` via `mihon.core.archive.openFileDescriptor(context, "rwt")`; write the entire `value` through a `bufferedWriter` (UTF-8 by default in Kotlin `writer`).
  3. After the write attempt, `preference.set(value)` (inside the same `if`).
  4. Returns `true` in all cases (line 199).
  - Consequence (E-06): on Android without all-files access, the file is NOT written, but the function still returns `true`. The outer `PreferenceItem` branch (`PreferenceItem.kt`, MPV variant, lines 189-196) then runs `if (accepted) item.preference.set(it)`, so the preference IS updated and the dialog closes. The on-disk config and the stored preference can therefore diverge silently.
- Proposal (Linux): the file lives at `$XDG_CONFIG_HOME/kurobayomi/mpv/<fileName>` (default `~/.config/kurobayomi/mpv/`). The write is unconditional. Area:48 owns the file names and the MPV path.

---

## §C. Search highlight infrastructure

### C.1 The global handoff (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SearchableSettings.kt:37-42`, owned by area:47)
- `companion object { var highlightKey: String? = null }`, documented as a hack: "The title of the target PreferenceItem; Set before showing the destination screen and reset after".
- Writers observed in scope: `app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt:47` (`SearchableSettings.highlightKey = restoreSettingKey`) and the settings search screen (area:47).
- Readers: `PreferenceScreen.kt:32-41`.

### C.2 The flash (`BasePreferenceWidget.kt:89-120`, `Modifier.highlightBackground(highlighted: Boolean)`)
- `composed { }` with `var highlightFlag by remember { mutableStateOf(false) }`.
- `LaunchedEffect(Unit)`: if `highlighted` at first composition, set `highlightFlag = true`, `delay(3.seconds)`, `highlightFlag = false`.
- Colour: `animateColorAsState(targetValue = if (highlightFlag) surfaceTint.copy(alpha = .12f) else Transparent, animationSpec = ...)`.
  - When `highlightFlag` true: `repeatable(iterations = 5, animation = tween(durationMillis = 200), repeatMode = RepeatMode.Reverse, initialStartOffset = StartOffset(offsetMillis = 600, offsetType = Delay))`.
  - When false: `tween(200)`.
- Net effect: after a 600 ms delay, the row pulses 5 times (each leg 200 ms, reversing, so about 1000 ms of pulsing), then after the 3 s timer fades back over 200 ms.
- `Modifier.background(color = highlight)` applied to the row.
- Constants used by the timeline: `600 ms` delay, `200 ms` leg, `5` iterations, `3 s` total flag.

### C.3 Target scroll timing
- Scroll target: `delay(0.5.seconds)` then `animateScrollToItem(i)` (`PreferenceScreen.kt:37-38`). The flash starts when the target row first composes, which is when it scrolls into the viewport; the highlight is therefore visible after the scroll rather than before it.

### C.4 Which widgets can flash
- Flash-capable (they call `BasePreferenceWidget` or read `LocalPreferenceHighlighted`): Text (B.2), Switch (B.3), List (B.4 through Text), MultiSelect (through Text), EditText family (through Text), Slider (wrapper only: NO flash because `BaseSliderItem` does not read the local; `PreferenceItem.kt:84` does not wrap with highlight), Tracker (B.8 reads it directly, line 32-33).
- NOT flash-capable: InfoWidget (B.9), CustomPreference (B.12), group header (B.10).
- Observed: the Slider path does not call `highlightBackground`. Proposal: wrap the slider row with the highlight in Rust (E-08).

### C.5 Title matching
- Match is exact string equality on `title` (`PreferenceScreen.kt:96`, `PreferenceItem.kt:43`). Locale-dependent titles mean highlight keys are localized strings.

---

## §T. Theme pickers

### T.1 Theme mode (`widget/AppThemeModePreferenceWidget.kt`, lines 15-48)
- `options = mapOf(ThemeMode.SYSTEM to MR.strings.theme_system, ThemeMode.LIGHT to MR.strings.theme_light, ThemeMode.DARK to MR.strings.theme_dark)` (insertion order SYSTEM, LIGHT, DARK).
- `MultiChoiceSegmentedButtonRow(fillMaxWidth, padding(horizontal = 16dp))` containing `SegmentedButton(checked = mode == value, onCheckedChange = { onItemClick(mode) }, shape = SegmentedButtonDefaults.itemShape(index, options.size))`.
- Labels: "System", "Light", "Dark" (English strings: `theme_system`, `theme_light`, `theme_dark`).
- Clicking an already-selected segment still calls `onItemClick(mode)` (idempotent write).
- Persisted key: `pref_theme_mode_key`, enum `ThemeMode` default `SYSTEM` (`app/src/main/java/eu/kanade/domain/ui/UiPreferences.kt:20`). Enum values: `LIGHT`, `DARK`, `SYSTEM` (`app/src/main/java/eu/kanade/domain/ui/model/ThemeMode.kt:5-9`; declaration order LIGHT, DARK, SYSTEM).

### T.2 App theme strip (`widget/AppThemePreferenceWidget.kt`, lines 60-274)
- Wrapper: `BasePreferenceWidget(subcomponent = { AppThemesList(currentTheme = value, amoled, onItemClick) })`. No title, no icon, no click.
- Theme list: `AppTheme.entries.filterNot { it.titleRes == null || (it == MONET && !DeviceUtil.isDynamicColorAvailable) }` (lines 83-86), computed once with `remember`.
- `LazyRow(contentPadding = horizontal 16dp, horizontalArrangement = spacedBy(8dp))`, `items(appThemes, key = { it.name })`.
- Each cell: `Column(width(114dp), padding(top = 8dp))`:
  - `TachiyomiTheme(appTheme, amoled) { AppThemePreviewItem(selected = currentTheme == appTheme, onClick = { onItemClick(appTheme); (context as? Activity)?.let { ActivityCompat.recreate(it) } }) }` (lines 100-110).
  - `Spacer(height = 8dp)`.
  - Label: `Text(stringResource(appTheme.titleRes!!), fillMaxWidth, secondaryItemAlpha, textAlign = Center, maxLines = 2, minLines = 2, bodyMedium)` (lines 115-124).
- Preview item (`AppThemePreviewItem`, lines 131-258):
  - Outer `Column(fillMaxWidth, aspectRatio(9f/16f), border(width = 4dp, color = primary if selected else DividerDefaults.color, shape = RoundedCornerShape(17dp)), padding(4dp), clip(RoundedCornerShape(13dp)), background(colorScheme.background), clickable(onClick))`.
  - App bar row: `height(40dp)`, `padding(8dp)`, centred: a bar `fillMaxHeight(0.8f).weight(0.7f).padding(end = 4dp)` in `onSurface` with `MaterialTheme.shapes.small`; a `Box(weight(0.3f), CenterEnd)` that shows `Icons.Filled.CheckCircle` in `primary` with contentDescription `stringResource(MR.strings.selected)` ("Selected") when selected.
  - Cover: `Box(padding(start = 8dp, top = 2dp), background(DividerDefaults.color, small), fillMaxWidth(0.5f), aspectRatio(ItemCover.Book.ratio))` with `ItemCover.Book.ratio = 2f/3f` (`app/src/main/java/eu/kanade/presentation/entries/components/ItemCover.kt:18-19`). Inside: `Row(padding(4dp), size(24dp x 16dp), clip(RoundedCornerShape(5dp)))` with two 12dp boxes: `tertiary` then `secondary`.
  - Bottom bar: `Box(fillMaxWidth, weight(1f), BottomCenter)` with `Surface(color = surfaceContainer)` containing `Row(height(32dp), fillMaxWidth, padding(horizontal = 8dp))`: a `primary` circle 17dp, then a bar `padding(start = 8dp)`, `alpha(0.6f)`, `height(17dp)`, `weight(1f)`, `onSurface` with `small` shape.
- Themes shown (declaration order, `app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt:7-32`): DEFAULT (`label_default`), MONET (`theme_monet`, Android only), CLOUDFLARE, COTTONCANDY, DOOM, GREEN_APPLE, LAVENDER, MATRIX, MIDNIGHT_DUSK, MOCHA, SAPPHIRE, NORD, STRAWBERRY_DAIQUIRI, TAKO, TEALTURQUOISE, TIDAL_WAVE, YINYANG, YOTSUBA, MONOCHROME. Deprecated with null titles and hidden: DARK_BLUE, HOT_PINK, BLUE.
  - Count: 19 with titles. Shown on Android: 19 when `isDynamicColorAvailable`, otherwise 18. On Linux `MONET` is dropped, so 18 cards.
- Persistence: key `pref_app_theme`, default `AppTheme.MONET` when dynamic colour is available, else `AppTheme.DEFAULT` (`UiPreferences.kt:22-30`). Amoled flag key `pref_theme_dark_amoled_key`, default `false` (`UiPreferences.kt:31`).
- Click behaviour: writes the theme, then recreates the Activity so the Compose theme re-applies. Linux replacement: swap the CSS provider live (no restart).

---

## §M. More tab

### M.1 Tab registration (`app/src/main/java/eu/kanade/tachiyomi/ui/more/MoreTab.kt:43-59`)
- `data object MoreTab : Tab`.
- `options`: `TabOptions(index = 4u, title = stringResource(MR.strings.label_more), icon = rememberAnimatedVectorPainter(AnimatedImageVector.animatedVectorResource(R.drawable.anim_more_enter), isSelected))` where `isSelected = LocalTabNavigator.current.current.key == key`.
- `onReselect(navigator)` pushes `SettingsScreen()` (line 58). Reselecting the More tab opens Settings.
- Label string: `label_more` = "More" (`i18n/src/commonMain/moko-resources/base/strings.xml`).

### M.2 Screen layout (`app/src/main/java/eu/kanade/presentation/more/MoreScreen.kt:34-177`)
Top to bottom, a `Scaffold { ScrollbarLazyColumn(modifier = padding(contentPadding)) { ... } }`:

| # | Element | Source lines | Widget | Title (English) | Subtitle (English) | Icon | Action |
|---|---|---|---|---|---|---|---|
| 1 | Logo header | 57-59 | `LogoHeader()` | none | none | `R.drawable.ic_ani` | none |
| 2 | Downloaded only | 60-68 | `SwitchPreferenceWidget` | "Downloaded only" (`label_downloaded_only`) | "Filters all entries in your library" (`downloaded_only_summary`) | `Icons.Outlined.CloudOff` | toggles `downloadedOnly` |
| 3 | Incognito mode | 69-77 | `SwitchPreferenceWidget` | "Incognito mode" (`pref_incognito_mode`) | "Pauses your history" (`AYMR pref_incognito_mode_summary`) | `R.drawable.ic_glasses_24dp` | toggles `incognitoMode` |
| 4 | Divider | 79 | `HorizontalDivider()` | | | | |
| 5 | Alt-nav row | 81-87 | `TextPreferenceWidget` | `navStyle.moreTab.options.title` | none | `navStyle.moreIcon` | `onClickAlt` |
| 6 | Download queue | 89-122 | `TextPreferenceWidget` | "Download queue" (`label_download_queue`) | per queue state (M.3) | `Icons.Outlined.GetApp` | `onClickDownloadQueue` |
| 7 | Categories | 123-129 | `TextPreferenceWidget` | "Categories" (`AYMR general_categories`) | none | `Icons.AutoMirrored.Outlined.Label` | `onClickCategories` |
| 8 | Statistics | 130-136 | `TextPreferenceWidget` | "Statistics" (`label_stats`) | none | `Icons.Outlined.QueryStats` | `onClickStats` |
| 9 | Data and storage | 137-143 | `TextPreferenceWidget` | "Data and storage" (`label_data_storage`) | none | `Icons.Outlined.Storage` | `onClickDataAndStorage` |
| 10 | Divider | 145 | `HorizontalDivider()` | | | | |
| 11 | Settings | 147-153 | `TextPreferenceWidget` | "Settings" (`label_settings`) | none | `Icons.Outlined.Settings` | `onClickSettings` |
| 12 | Player settings | 154-160 | `TextPreferenceWidget` | "Player settings" (`AYMR label_player_settings`) | none | `Icons.Outlined.VideoSettings` | `onClickPlayerSettings` |
| 13 | About | 161-167 | `TextPreferenceWidget` | "About" (`pref_category_about`) | none | `Icons.Outlined.Info` | `onClickAbout` |
| 14 | Help | 168-174 | `TextPreferenceWidget` | "Help" (`label_help`) | none | `Icons.AutoMirrored.Outlined.HelpOutline` | `uriHandler.openUri(Constants.URL_HELP)` |

- Row 5 title is `navStyle.moreTab.options.title`, which resolves to the tab label of the moved tab (for example "Updates" for `MOVE_UPDATES_TO_MORE`). Row 5 has no subtitle. The `NavStyle` enum is in `app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt` (area:36) with values `MOVE_MANGA_TO_MORE` (`pref_bottom_nav_no_manga` "Move Manga to the More tab", moreIcon `CollectionsBookmark`, tab `MangaLibraryTab`), `MOVE_UPDATES_TO_MORE` (`pref_bottom_nav_no_updates`, `ic_updates_outline_24dp`, `UpdatesTab`), `MOVE_HISTORY_TO_MORE` (`pref_bottom_nav_no_history`, `Icons.Outlined.History`, `HistoriesTab`), `MOVE_BROWSE_TO_MORE` (`pref_bottom_nav_no_browse`, `Icons.Outlined.Explore`, `BrowseTab`).
- The alt-nav row is always present (line 81-87 with no condition). With the default `MOVE_HISTORY_TO_MORE`, it shows "History" (this is what `spec/app.json` removes in the mockup; see §M.5).
- Persisted key for nav style: `bottom_rail_nav_style`, default `NavStyle.MOVE_HISTORY_TO_MORE` (`UiPreferences.kt:41`).
- Divider rows use `HorizontalDivider()` (Material3) with default colour and thickness.
- Each `item { }` is a separate LazyColumn slot. Rows have no explicit padding; `TextPreferenceWidget` supplies the layout (§B.2).

### M.3 Download queue state (`MoreTab.kt:88-132`, `MoreScreenModel`)
- Inputs: `MangaDownloadManager.isDownloaderRunning` and `.queueState` (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloadManager.kt:64,71`, a `StateFlow`-like exposure of `MangaDownloader.queueState`), and the same two members on `AnimeDownloadManager` (`app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloadManager.kt:62,69`).
- Algorithm (`MoreTab.kt:102-130`, inside `screenModelScope.launchIO`):
  1. `combine(manga.isDownloaderRunning, manga.queueState) { isRunningManga, mangaQueue -> Pair(isRunningManga, mangaQueue.size) }.collectLatest { (isDownloadingManga, mangaSize) -> ... }`.
  2. Inside, `combine(anime.isDownloaderRunning, anime.queueState) { isRunningAnime, animeQueue -> Pair(isRunningAnime, animeQueue.size) }.collectLatest { (isDownloadingAnime, animeSize) -> ... }`.
  3. `isDownloading = isDownloadingAnime || isDownloadingManga`.
  4. `downloadQueueSize = mangaSize + animeSize`.
  5. `pendingDownloadExists = downloadQueueSize != 0`.
  6. `_downloadQueueState.value = when { !pendingDownloadExists -> Stopped; !isDownloading -> Paused(downloadQueueSize); else -> Downloading(downloadQueueSize) }`.
- Initial value: `DownloadQueueState.Stopped` (line 97-99).
- Subtitle rendering (`MoreScreen.kt:93-118`):
  - `Stopped` → `null` (no subtitle line).
  - `Paused(pending)`: if `pending == 0` → `"Paused"` (`MR.strings.paused`); else `"Paused • " + pluralStringResource(MR.plurals.download_queue_summary, count = pending, pending)`. Separator is space, U+2022 BULLET, space.
  - `Downloading(pending)` → `pluralStringResource(download_queue_summary, count = pending, pending)`.
  - `download_queue_summary` plurals (`i18n/src/commonMain/moko-resources/base/plurals.xml:33-36`): `one` = `%1$s remaining`, `other` = `%1$s remaining`. So English is always "N remaining" (e.g. "3 remaining", "1 remaining", "0 remaining").
  - Example strings: "Paused • 3 remaining", "3 remaining", "Paused".
- Notes:
  - `Paused(0)` is reachable only if the queue was non-empty and then emptied while paused; the code still prints "Paused".
  - The state is computed on the Kotlin side from manager flows, not from preferences.

### M.4 Observed defects and dead code in MoreScreen/MoreTab (OBSERVED)
- `MoreScreen` has a parameter `onClickStorage: () -> Unit` (`MoreScreen.kt:45`) that is never used by any row. `MoreTab.kt:79` supplies `navigator.push(StorageTab)`. The "Data and storage" row (line 137-143) calls `onClickDataAndStorage`, which pushes `SettingsScreen(SettingsScreen.Destination.DataAndStorage)` (`MoreTab.kt:80`). So `StorageTab` is unreachable from this screen. Proposal: drop the parameter; Rust has no equivalent (storage page is `settings_data_storage` in the spec).
- Help row (line 168-174) calls `uriHandler.openUri`; no error handling if no browser.
- The "Downloaded only" and "Incognito" switches use `onCheckedChange` directly (no veto). Their state is in `MoreScreenModel` via `asState(screenModelScope)`.

### M.5 Navigation pushes (`MoreTab.kt:75-83`)
| Row | Push | Notes |
|---|---|---|
| Alt-nav row | `navStyle.moreTab` (one of MangaLibraryTab / UpdatesTab / HistoriesTab / BrowseTab) | pushes a Tab object as a screen |
| Download queue | `DownloadsTab` | |
| Categories | `CategoriesTab` | |
| Statistics | `StatsTab` | |
| Data and storage | `SettingsScreen(SettingsScreen.Destination.DataAndStorage)` | |
| Player settings | `PlayerSettingsScreen(mainSettings = false)` | |
| Settings | `SettingsScreen()` | |
| About | `SettingsScreen(SettingsScreen.Destination.About)` | |
- Mockup comparison: `src/screens.rs:336-349` removes the `History` link (`Item::Link { target == "history" }`) from the More groups because History is a sidebar entry in Kurobayomi. Proposal: keep that removal but make it conditional on the nav style pref (`bottom_rail_nav_style`), so that the alt-nav row reappears if the user moves History.

### M.6 Preview/tooling
- `@PreviewLightDark` is absent in `MoreScreen.kt` (no preview). `LogoHeader.kt` has no preview. Preview infrastructure is dropped.

---

## §U. Update dialog

### U.1 Presentation (`app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt:29-67`)
- Signature: `NewUpdateScreen(versionName: String, changelogInfo: String, onOpenInBrowser, onRejectUpdate, onAcceptUpdate)`.
- `InfoScreen(icon = Icons.Outlined.NewReleases, headingText = stringResource(MR.strings.update_check_notification_update_available), subtitleText = versionName, acceptText = stringResource(MR.strings.update_check_confirm), onAcceptClick = onAcceptUpdate, rejectText = stringResource(MR.strings.action_not_now), onRejectClick = onRejectUpdate) { body }`.
- Strings: heading "New version available!", accept "Download", reject "Not now".
- Body: `RichText(fillMaxWidth, padding(vertical = MaterialTheme.padding.large))` (24dp) with `RichTextStyle(stringStyle = RichTextStringStyle(linkStyle = SpanStyle(color = primary)))`, containing `Markdown(content = changelogInfo)` then a `TextButton(onOpenInBrowser, padding(top = small))` with label "Open on GitHub" (`update_check_open`) and an `Icons.AutoMirrored.Outlined.OpenInNew` icon after `Spacer(width = extraSmall)`.
- Padding tokens (`presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Constants.kt`): extraLarge 32dp, large 24dp, medium 16dp, mediumSmall 12dp, small 8dp, extraSmall 4dp. This screen uses `large` (24dp) for the body, `small` (8dp) above the link button, `extraSmall` (4dp) before its icon.
- Label "Open on GitHub" is hard-coded in the English string even though the URL may point to a non-GitHub host (E-09).

### U.2 Wrapper (`app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt:13-43`)
- Constructor: `(versionName: String, changelogInfo: String, releaseLink: String, downloadLink: String)`.
- Changelog cleanup: `remember { changelogInfo.replace("""---(\R|.)*Checksums(\R|.)*""".toRegex(), "") }` (line 24-26).
  - Regex semantics (Java regex, no DOTALL, but `(\R|.)*` covers newlines): the match starts at the first `---` that is followed somewhere later by `Checksums`, and consumes greedily to the end of the string (including every later line). Everything from that `---` to the end is removed. If no `Checksums` appears after any `---`, nothing is removed.
  - Example: body `"## Yay\n...\n---\nChecksums:\nabc"` → `"## Yay\n...\n"`.
  - Edge: a changelog with `---` used as a horizontal rule above real notes, and `Checksums` only at the very end, loses everything after the first rule (E-10).
- `onOpenInBrowser = { context.openInBrowser(releaseLink) }` (line 31).
- `onRejectUpdate = navigator::pop`.
- `onAcceptUpdate`: `AppUpdateDownloadJob.start(context, url = downloadLink, title = versionName)` then `navigator.pop()` (lines 33-40).

### U.3 Download start (`app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateDownloadJob.kt:133-146`, owned by area:35)
- `OneTimeWorkRequestBuilder<AppUpdateDownloadJob>()` with `Constraints(requiredNetworkType = CONNECTED)`, tag `"AppUpdateDownload"` (`TAG`, line 128), input data `EXTRA_DOWNLOAD_URL = "DOWNLOAD_URL"` and `EXTRA_DOWNLOAD_TITLE = "DOWNLOAD_TITLE"` (lines 130-131). Title falls back to `MR.strings.app_name` (line 42).
- The actual download and install are area:35's responsibility. This area only triggers it. Linux (PROPOSAL): spawn a tokio task that downloads to `$XDG_DOWNLOAD_DIR` (fallback `~/Downloads`), shows progress through `gio::Notification`, then reveals the file with `gtk::FileLauncher`. Install is not performed automatically (Linux packages are not self-installable).

### U.4 Reject / accept / open
- Not now: pop the screen; nothing persisted (the updater has its own "skip version" logic in area:35, not here).
- Download: start job, then pop immediately. The dialog does not wait for progress.
- Open on GitHub: `context.openInBrowser(releaseLink)`; `releaseLink` is the release page URL from the update check.

---

## §O. Onboarding wrapper (`app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt:20-52`)

- `class OnboardingScreen : Screen()`.
- `basePreferences.shownOnboardingFlow()` = `preferenceStore.getBoolean(Preference.appStateKey("onboarding_complete"), false)` (`app/src/main/java/eu/kanade/domain/base/BasePreferences.kt:29`). Stored key: `__APP_STATE_onboarding_complete` (prefix from `core/common/src/main/java/tachiyomi/core/common/preference/Preference.kt:47`).
- `BackHandler(enabled = !shownOnboardingFlow, onBack = { })`: back is swallowed until onboarding is complete (lines 36-41).
- `finishOnboarding = { basePreferences.shownOnboardingFlow().set(true); navigator.pop() }` (lines 29-32).
- `restoreSettingKey = stringResource(SettingsDataScreen.restorePreferenceKeyString)` = `MR.strings.label_backup` = "Backup and restore" (`app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsDataScreen.kt:88`, `i18n/src/commonMain/moko-resources/base/strings.xml`).
- `onRestoreBackup`: `finishOnboarding(); SearchableSettings.highlightKey = restoreSettingKey; navigator.push(SettingsScreen(SettingsScreen.Destination.DataAndStorage))` (lines 45-49). The restore row in the Data page is then highlighted by §C.
- The step UI (`app/src/main/java/eu/kanade/presentation/more/onboarding/OnboardingScreen.kt`, `ThemeStep.kt`, `StorageStep.kt`, `PermissionStep.kt`, `GuidesStep.kt`) is area:36 scope; it is passed `onComplete` and `onRestoreBackup`.
- Gate that pushes it: `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:407-412` pushes `OnboardingScreen()` when `!preferences.shownOnboardingFlow().get() && navigator.lastItem !is OnboardingScreen` (area:36).

---

## §L. Logo header (`app/src/main/java/eu/kanade/presentation/more/LogoHeader.kt:18-34`)
- `Column(fillMaxWidth, CenterHorizontally) { Icon(painterResource(R.drawable.ic_ani), contentDescription = null, tint = onSurface, Modifier.padding(vertical = 56.dp).size(64.dp)); HorizontalDivider() }`.
- The icon is decorative (`contentDescription = null`).
- Vertical space above and below the icon is 56dp each (total 112dp of padding plus the 64dp icon).

---

## §X. Cross-cutting

### X.1 Visible strings used in this scope (English, from the base string files)
Strings used by scope files, resolved from `i18n/src/commonMain/moko-resources/base/strings.xml` and `i18n-aniyomi/src/commonMain/moko-resources/base/strings.xml`:

| Key | English | Used in |
|---|---|---|
| `label_downloaded_only` | Downloaded only | MoreScreen |
| `downloaded_only_summary` | Filters all entries in your library | MoreScreen |
| `pref_incognito_mode` | Incognito mode | MoreScreen |
| `pref_incognito_mode_summary` (AYMR) | Pauses your history | MoreScreen |
| `label_download_queue` | Download queue | MoreScreen |
| `paused` | Paused | MoreScreen |
| `download_queue_summary` (plural) | %1$s remaining | MoreScreen |
| `general_categories` (AYMR) | Categories | MoreScreen |
| `label_stats` | Statistics | MoreScreen |
| `label_data_storage` | Data and storage | MoreScreen |
| `label_settings` | Settings | MoreScreen |
| `label_player_settings` (AYMR) | Player settings | MoreScreen |
| `pref_category_about` | About | MoreScreen |
| `label_help` | Help | MoreScreen |
| `label_more` | More | MoreTab |
| `none` | None | MultiSelect subtitle |
| `selected` | Selected | AppTheme preview |
| `login_success` | Logged in | Tracker row |
| `action_ok` | OK | EditText, MultiSelect, TriState |
| `action_cancel` | Cancel | EditText, List, MultiSelect, TriState |
| `action_not_now` | Not now | NewUpdate |
| `update_check_notification_update_available` | New version available! | NewUpdate |
| `update_check_confirm` | Download | NewUpdate |
| `update_check_open` | Open on GitHub | NewUpdate |
| `theme_system` / `theme_light` / `theme_dark` | System / Light / Dark | ThemeMode |
| `label_default` | Default | AppTheme DEFAULT |
| `theme_monet` | Dynamic | AppTheme MONET |
| `disabled` | Disabled | TriState (all states, bug E-04) |
| `label_backup` | Backup and restore | Onboarding restore highlight |
| `pref_bottom_nav_no_manga` / `_updates` / `_history` / `_browse` (AYMR) | Move Manga / Updates / History / Browse to the More tab | NavStyle titles |
| `theme_cloudflare`, `theme_cottoncandy`, `theme_doom`, `theme_matrix`, `theme_mocha`, `theme_sapphire` (AYMR) | Cloudflare, Cotton Candy, Doom, Matrix, Mocha, Sapphire | AppTheme labels |
| `download_ahead_info` (AYMR) | Only works if the current chapter/episode + the next one are already downloaded. | InfoWidget preview only |

Other AppTheme labels (`theme_greenapple`, `theme_lavender`, `theme_midnightdusk`, `theme_nord`, `theme_strawberrydaiquiri`, `theme_tako`, `theme_tealturquoise`, `theme_tidalwave`, `theme_yinyang`, `theme_yotsuba`, `theme_monochrome`) are in `i18n/src/commonMain/moko-resources/base/strings.xml` and are resolved the same way. The Kurobayomi spec must use the exact English text from that file (per `spec/SCHEMA.md`).

### X.2 Dialog and widget inventory (what a user can see)
- AlertDialog variants: List (B.4, Cancel only), MultiSelect (B.5, OK+Cancel), EditText (B.6, OK+Cancel, clear button and error icon), TriState (B.11, Cancel + OK; message text optional).
- Non-dialog inline widgets: Switch, Text, Info, Tracker, Slider, Segmented theme mode, Theme card strip, Group header, Logo.

### X.3 Errors and messages (OBSERVED)
- No error toast or snackbar exists in any scope file. The only user-visible error text is `supportingText` of `EditTextPreferenceWidget` (from the caller's `errorMessage`).
- Exceptions inside `onValueChanged` are not caught in scope (A.5). Proposal: wrap each Rust veto in a `Result`, log with `tracing`, and show `gtk::AlertDialog` with message "Something went wrong" only if an app-level string exists in area:51; otherwise log only.

### X.4 Android-only mechanisms observed
- `ActivityCompat.recreate(activity)` (AppThemePreference, line 108): Linux replacement is live CSS reload.
- `HapticFeedback` (`SettingsItems.kt:273-277`): dropped on Linux (no haptic device). Replacement: none; note loss in the user loss table.
- `Environment.isExternalStorageManager()` and `openFileDescriptor` (`Preference.kt:187-195`): replaced by plain file I/O under XDG config.
- `DeviceUtil.isDynamicColorAvailable` (`DeviceUtilExtensions.kt:6-8`): always false on Linux, so MONET is hidden.
- `ImageVector` and `painterResource` / `AnimatedImageVector`: replaced by GTK symbolic icon names (see rust-design §8).
- `BackHandler` (OnboardingScreen wrapper): replaced by `gtk::Window::connect_close_request` returning `glib::Propagation::Stop` while onboarding is incomplete. Also the Escape key path in GTK.
- `UriHandler.openUri` (MoreScreen Help row) → `gtk::UriLauncher`.
- `context.openInBrowser(url)` (NewUpdateScreen) → `gtk::UriLauncher::new(url).launch(...)`.

### X.5 Concurrency (OBSERVED)
- `rememberCoroutineScope()` for every write (`scope.launch`) runs on the Compose main dispatcher; `screenModelScope.launchIO` for MoreScreenModel's combine.
- `preference.collectAsState()` drives recomposition from the preference store (`app/src/main/java/eu/kanade/core/preference/asState` and `tachiyomi.presentation.core.util.collectAsState`).
- Rust mapping in rust-design §5.

### X.6 Timings (all OBSERVED)
| Constant | Value | Location |
|---|---|---|
| Highlight target scroll delay | 0.5 s | PreferenceScreen.kt:37 |
| Highlight flag duration | 3 s | BasePreferenceWidget.kt:94 |
| Highlight start offset | 600 ms | BasePreferenceWidget.kt:110 |
| Highlight leg | 200 ms | BasePreferenceWidget.kt:107 |
| Highlight iterations | 5 (Reverse) | BasePreferenceWidget.kt:106 |
| Highlight fade out | 200 ms | BasePreferenceWidget.kt:115 |
| Group spacer | 12 dp | PreferenceScreen.kt:68 |
| Group header padding | top 14 dp, bottom 8 dp | PreferenceGroupHeader.kt:19 |
| Row min height | 56 dp | PreferenceItem.kt:34 |
| Row horizontal padding | 16 dp | BasePreferenceWidget.kt:123 |
| Row vertical padding | 16 dp | BasePreferenceWidget.kt:124 |
| Theme card width | 114 dp | AppThemePreferenceWidget.kt:97 |
| Theme card label lines | 2 min / 2 max | AppThemePreferenceWidget.kt:121 |
| Tri-state row min height | 48 dp | TriStateListDialog.kt:91 |
| Checkbox row min height | 48 dp | LabeledCheckbox.kt (heightIn min 48) |
| Logo icon | 64 dp, 56 dp padding | LogoHeader.kt:28-29 |

---

## §E. Edge-case behaviour summary (cross-referenced in p1-edge-cases-and-tests.md)
- Highlight of a row inside a disabled group: not reached (group not rendered) but index still counted (E-07).
- Highlight of a title shared by two rows: both flash (A.5, C.5).
- Multi-select subtitle ordering: set order (B.5).
- Tri-state contentDescription: always "Disabled" (B.11).
- EditText with `canBeBlank=false` and blank text: error icon, no supporting text, confirm disabled (B.6).
- EditText with `validate` false and no `errorMessage`: error icon, confirm disabled, no text (B.6).
- MPV write when permission absent: still returns true (B.14).
- Changelog with `---` before `Checksums`: truncated from first `---` (U.2).
- Onboarding back: swallowed until complete (§O).
