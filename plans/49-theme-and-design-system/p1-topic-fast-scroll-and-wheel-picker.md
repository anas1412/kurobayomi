# Area 49 (part 1) topic: fast scroller and wheel picker algorithms

Sources (repo-relative to `/home/blackbox/aniyomi-src`):
- `presentation-core/src/main/java/tachiyomi/presentation/core/components/VerticalFastScroller.kt`
- `presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyList.kt`
- `presentation-core/src/main/java/tachiyomi/presentation/core/components/LazyGrid.kt`
- `presentation-core/src/main/java/tachiyomi/presentation/core/components/WheelPicker.kt`
- `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt` (RepeatingIconButton)

## 1. Scroller structure (OBSERVED)

`VerticalFastScroller(listState, modifier, thumbAllowed = { true }, thumbColor = primary, topContentPadding = Dp.Hairline, bottomContentPadding = Dp.Hairline, endContentPadding = Dp.Hairline, content)` uses `SubcomposeLayout`:
1. Subcompose `"content"`, measure with the incoming constraints. `contentHeight` = max child height, `contentWidth` = max child width.
2. Subcompose `"scroller"` with `scrollerConstraints = constraints.copy(minWidth = 0, minHeight = 0)`.
3. Layout size is `(contentWidth, contentHeight)`. Content placed at (0, 0). Scroller placed at `(contentWidth - scrollerWidth, 0)`.

The scroller subcomposition returns early (no thumb) when `listState.layoutInfo.visibleItemsInfo.size >= totalItemsCount`. A list that fits on screen has no thumb.

## 2. Thumb geometry (OBSERVED)

- `thumbTopPadding = topContentPadding.toPx()`, `thumbBottomPadding = bottomContentPadding.toPx()`.
- `heightPx = contentHeight - thumbTopPadding - thumbBottomPadding - listState.layoutInfo.afterContentPadding` (list variant).
- `thumbHeightPx = ThumbLength.toPx()` = 48 dp.
- `trackHeightPx = heightPx - thumbHeightPx`.
- `thumbOffsetY` starts at `thumbTopPadding` (remembered per `thumbTopPadding`).
- Thumb Box: `height(48.dp)`, `padding(horizontal = 8.dp)`, `padding(end = endContentPadding)`, `width(12.dp)`, `alpha(alpha)`, `background(thumbColor, RoundedCornerShape(6.dp))`, offset `(0, thumbOffsetY rounded)`.

## 3. List scroll to thumb (OBSERVED, `LaunchedEffect(listState.firstVisibleItemScrollOffset)`)

Guard: return if `totalItemsCount == 0` or the thumb is being dragged.
1. `scrollOffset = computeScrollOffset(listState)`.
2. `scrollRange = computeScrollRange(listState)`.
3. `proportion = scrollOffset.toFloat() / (scrollRange.toFloat() - heightPx)`.
4. `thumbOffsetY = trackHeightPx * proportion + thumbTopPadding`.
5. `scrolled.tryEmit(Unit)`.

`computeScrollOffset(LazyListState)`:
- `startChild` = first visible item whose key is not a string starting with `"sticky:"` (`fastFirstOrNull { (it.key as? String)?.startsWith(STICKY_HEADER_KEY_PREFIX)?.not() ?: true }!!`). The `!!` throws if every visible item is a sticky header (edge case E-49-14).
- `endChild` = last visible item.
- `minPosition = min(startChild.index, endChild.index)`, `maxPosition = max(...)`.
- `itemsBefore = minPosition.coerceAtLeast(0)`.
- `startDecoratedTop = startChild.top` (= `offset`).
- `laidOutArea = abs(endChild.bottom - startDecoratedTop)` where `bottom = offset + size`.
- `itemRange = abs(minPosition - maxPosition) + 1`.
- `avgSizePerRow = laidOutArea.toFloat() / itemRange`.
- Return `round(itemsBefore * avgSizePerRow + (0 - startDecoratedTop))`.

`computeScrollRange(LazyListState)`:
- `laidOutArea = endChild.bottom - startChild.top`; `laidOutRange = abs(startChild.index - endChild.index) + 1`.
- Return `round(laidOutArea.toFloat() / laidOutRange * totalItemsCount)`.

## 4. Thumb drag to list (OBSERVED, `LaunchedEffect(thumbOffsetY)`)

Trigger: every change of `thumbOffsetY`. Guard: return if `totalItemsCount == 0` or not dragging (`dragInteractionSource.collectIsDraggedAsState()`).
1. `scrollRatio = (thumbOffsetY - thumbTopPadding) / trackHeightPx`.
2. `scrollItem = totalItemsCount * scrollRatio` (float index).
3. `scrollItemRounded = round(scrollItem)`.
4. `scrollItemSize = visibleItemsInfo.find { it.index == scrollItemRounded }?.size ?: 0`.
5. `scrollItemOffset = scrollItemSize * (scrollItem - scrollItemRounded)`.
6. `listState.scrollToItem(index = scrollItemRounded, scrollOffset = round(scrollItemOffset))`.
7. `scrolled.tryEmit(Unit)`.

Drag input: `Modifier.draggable(orientation = Vertical, interactionSource, state = rememberDraggableState { delta -> thumbOffsetY = (thumbOffsetY + delta).coerceIn(thumbTopPadding, thumbTopPadding + trackHeightPx) })`. The draggable is attached only when `isThumbVisible && !listState.isScrollInProgress`.

Gesture exclusion: `systemGestureExclusion()` is attached only when `isThumbVisible && !isThumbDragged && !listState.isScrollInProgress`. This stops Android's back-gesture from stealing the thumb.

## 5. Fade (OBSERVED)

- `alpha = Animatable(0f)`; `isThumbVisible = alpha.value > 0f`.
- `scrolled.sample(100)` (100 ms throttle) then `collectLatest`:
  - if `thumbAllowed()`: `alpha.snapTo(1f)` then `alpha.animateTo(0f, FadeOutAnimationSpec)` (250 ms fade after a 2000 ms delay, see section 6).
  - else `alpha.animateTo(0f, ImmediateFadeOutAnimationSpec)` (250 ms fade, no delay).
- Result: after any scroll or drag, the thumb shows for 2 s then fades out over the platform scrollbar fade duration.

Platform constant: `ViewConfiguration.getScrollBarFadeDuration()` (Android framework; 250 ms default). Kurobayomi uses the constant 250.

## 6. Constants (OBSERVED)

| Name | Value | Location |
|---|---|---|
| `ThumbLength` | 48 dp | `VerticalFastScroller.kt:440` |
| `ThumbThickness` | 12 dp | `:441` |
| `ThumbShape` | `RoundedCornerShape(ThumbThickness / 2)` = 6 dp | `:442` |
| `FadeOutAnimationSpec` | `tween(duration = scrollBarFadeDuration, delay = 2000)` | `:443-446` |
| `ImmediateFadeOutAnimationSpec` | `tween(duration = scrollBarFadeDuration)` | `:447-449` |
| sample window | 100 ms | `:137` (list), `:324` (grid) |
| `STICKY_HEADER_KEY_PREFIX` | `"sticky:"` | `:437` |

## 7. Grid variant (OBSERVED, `VerticalGridFastScroller`)

Differences from the list variant:
- `slotSizesSums = rememberColumnWidthSums(columns, arrangement, contentPadding)`:
  - `require(constraints.maxWidth != Constraints.Infinity) { "LazyVerticalGrid's width should be bound by parent" }`.
  - `horizontalPadding = contentPadding.start + end` (LTR).
  - `gridWidth = maxWidth - horizontalPadding.roundToPx()`.
  - `calculateCrossAxisCellSizes(gridWidth, arrangement.spacing.roundToPx())` then cumulative sums.
- `columnCount = remember { slotSizesSums(constraints).size }`. The `remember` has no keys, so the count is computed once per slot and not recomputed when the grid's column setting changes (edge E-49-15).
- Scroll-to-thumb (drag) uses column arithmetic:
  - `scrollItemWhole = scrollItem.toInt()`.
  - `columnNum = ((scrollItemWhole + 1) % columnCount).takeIf { it != 0 } ?: columnCount`.
  - `scrollItemFraction = if (scrollItemWhole == 0) scrollItem else scrollItem % scrollItemWhole` (note: `% scrollItemWhole` is the source's expression; it is not `scrollItem - scrollItemWhole`; keep it as a known oddity and test it, E-49-16).
  - `offsetPerItem = 1f / columnCount`; `offsetRatio = offsetPerItem * scrollItemFraction + offsetPerItem * (columnNum - 1)`.
  - `scrollItemSize = max over num in 1..columnCount of (visible item at (num != columnNum ? whole + num - columnCount : whole)).size.height ?: 0`. Source TODO: "Sometimes item height is not available when scrolling up".
  - `scrollToItem(index = scrollItemWhole, scrollOffset = round(scrollItemSize * offsetRatio))`.
- Scroll-from-list uses `computeScrollOffset/Range(LazyGridState)`:
  - Offset: `startChild = visible.first()`, `endChild = visible.last()`, `itemsBefore = min(index).coerceAtLeast(0)`, `laidOutArea = abs((endChild.offset.y + endChild.height) - startChild.offset.y)`, `itemRange = abs(min - max) + 1`, `avg = laidOutArea / itemRange`, return `round(itemsBefore * avg - startChild.offset.y)`.
  - Range: `laidOutArea = (endChild.offset.y + endChild.height) - startChild.offset.y`; `laidOutRange = abs(startChild.index - endChild.index) + 1`; return `round(laidOutArea / laidOutRange * totalItemsCount)`.
- No sticky-header skipping in the grid variant.

## 8. WheelPicker algorithm (OBSERVED)

Constants: `ROW_COUNT = 3`; default `DpSize(128.dp, 128.dp)`.

Layout:
- Outer `Box(modifier.height(size.height).width(size.width), contentAlignment = Center)`.
- Background (`backgroundContent`, default `WheelPickerDefaults.Background(size)`): `Surface(Modifier.size(width, height / 3), shape = RoundedCornerShape(16.dp), color = primary.copy(alpha = 0.2f), border = BorderStroke(1.dp, primary))`.
- `LazyColumn(state = lazyListState, contentPadding = PaddingValues(vertical = height / 3 * ((3 - 1) / 2)), flingBehavior = rememberSnapFlingBehavior(lazyListState))`.
- Each item: `Box(height(height / 3), width(width), alpha(calculateAnimatedAlpha(state, index)), Center) { itemContent(item) }`.

Index tracking:
- `lazyListState = rememberLazyListState(startIndex)` (startIndex used only at creation).
- `internalIndex` starts at `startIndex`.
- `LaunchedEffect(lazyListState, onSelectionChanged)`: `snapshotFlow { firstVisibleItemScrollOffset }.map { calculateSnappedItemIndex(state) }.distinctUntilChanged().collectLatest { haptic(TextHandleMove); internalIndex = it; onSelectionChanged(it) }`. The first emission happens at composition, so the callback fires once with the currently snapped index.

Snap and alpha:
- `snapOffsetForItem(itemInfo) = 0 + (viewportEndOffset - afterContentPadding - 0 - itemInfo.size) / 2` (integer division).
- `distanceToSnapForIndex(index)`: if the item is visible, `itemInfo.offset - snapOffsetForItem(itemInfo)`; else 0.
- `calculateAnimatedAlpha(state, index)`: `d = |distanceToSnapForIndex(index)|`; `viewport = viewportSize.height`; `single = viewport / 3f`; if `d` in `0..single.toInt()` return `1.2f - d / single`, else `0.2f`.
- `calculateSnappedItemIndex(state) = visibleItemsInfo.maxBy { calculateAnimatedAlpha(state, it.index) }.index`.

Manual input (only when `manualInputType != null`, set by `WheelNumberPicker` to `KeyboardType.Number`; `WheelTextPicker` passes null, so it has no manual input):
1. Tap on the LazyColumn (`clickableNoIndication { showManualInput = true }`) replaces it with a `BasicTextField`.
2. `value = rememberSaveable(saver = TextFieldState.Saver) { TextFieldState(initialText = items[internalIndex].toString(), initialSelection = TextRange(length)) }`.
3. `showSoftKeyboard(true)` requests focus and opens the keyboard once.
4. `lineLimits = SingleLine`; `keyboardOptions = KeyboardOptions(keyboardType = Number, imeAction = Done)`; `textStyle = titleMedium + (color onSurface, TextAlign.Center)`; `cursorBrush = SolidColor(primary)`.
5. `processManualInput()`: launch a coroutine: `items.indexOfFirst { it.toString() == value.text }`; if `>= 0`: `internalOnSelectionChanged(index)` then `lazyListState.scrollToItem(index)`; then `showManualInput = false`. Runs on IME Done (`onKeyboardAction`) and when the keyboard is hidden (`clearFocusOnSoftKeyboardHide(processManualInput)`).
6. No match: the field closes and the selection is unchanged.

## 9. RepeatingIconButton timing (OBSERVED, computed)

Parameters: `maxDelayMillis = 750`, `minDelayMillis = 5`, `delayDecayFactor = 0.25f`.

Delay recurrence: `next = max(5, toLong(current - current * 0.25f))`.

Schedule of delays between successive clicks, starting with the first click at press time:

`750, 562, 421, 315, 236, 177, 132, 99, 74, 55, 41, 30, 22, 16, 12, 9, 6, 5, 5, 5, ...`

Pseudo code:

```
on ACTION_DOWN: pressed = true     (other actions: pressed = false)
LaunchedEffect(pressed, enabled):
  current = 750
  while enabled && pressed:
      onClick()
      delay(current)
      current = max(5, (current - current*0.25) truncated to Long)
```

## 10. OutlinedNumericChooser arithmetic (OBSERVED)

- Step: `+step` on the plus button, `-step` on minus, then clamp to `min` only when `min != null`.
- Text: `trim()`, remove every char matching `[^-\d.]`, `toIntOrNull()`, fallback to the previous value. No min clamp for typed input.
- Display: `"%d".format(currentValue)`.
