# Topic: layout and interaction widgets (Scaffold, FAB, rail, surface, slider, refresh, scrollbar)

Deep dive for area 49 part 2. Behaviour numbers are in `p2-behavior.md`; this file is the GTK4 design for the widgets they describe. Used by `p2-tasks.md` (T-49b-016 to T-49b-022, T-49b-025 to T-49b-036).

Root: `/home/blackbox/aniyomi-src/`. Widget sources: `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/` (Scaffold, Surface, Slider, NavigationRail, NavigationBar, PullRefresh, Tabs) and `presentation-core/src/main/java/tachiyomi/presentation/core/util/` (Scrollbar, LazyListState, Elevation, Modifier, PaddingValues).

---

## 1. Scaffold to GTK overlay

The Compose Scaffold is a `SubcomposeLayout` that measures five slots and places them on top of each other. GTK does this with a `gtk::Overlay` (one child plus overlays) and normal boxes for the bars. The table maps each slot.

| Compose slot (`ScaffoldLayoutContent`) | GTK node | Position rule |
|---|---|---|
| `MainContent` | `Overlay` child (`ScrolledWindow` of the page) | fills; its inner padding = top bar height, bottom = max(bottom bar height, FAB offset), start = max(start bar width) |
| `TopBar` | `gtk::HeaderBar` (or the page header box) in the page box | in flow, not overlay (GTK puts headers in flow); its height contributes to the content top padding only in the overlay model (see 1.2) |
| `StartBar` | sidebar box in the window `Paned` or `Box` | in flow |
| `BottomBar` | `gtk::Box` at the bottom of the page box | in flow |
| `Snackbar` | `gtk::Revealer` overlay, `halign = center`, `valign = end` | `margin-bottom = fabOffset or bottomBar height` |
| `Fab` | `gtk::Button` overlay, `halign = end`, `valign = end` | `margin-end = 16`, `margin-bottom = max(bottomBarHeight, bottomInset) + 16` |

### 1.1 Fidelity note
GTK places header bars and bottom bars in flow, so the content does not need `paddingTop = topBarHeight` the way Compose does. The Compose inner padding exists only because the bars are drawn over the content. In GTK the content is naturally under the margin of the bar box. Keep the FAB and snackbar offsets (they are overlays) and drop the inner padding for in-flow bars. The visible result is the same.

### 1.2 Expanded top bar (Compose: top bar taller than the screen)
Not needed in the mockup; the headers are fixed height. If a page needs a tall header, it becomes an in-flow box and the scroll content scrolls below it.

### 1.3 Code shape (proposal)

```rust
// src/ui/scaffold.rs
pub struct Scaffold {
    pub overlay: gtk::Overlay,           // child = page content
    pub fab: Option<gtk::Button>,
    pub snackbar: gtk::Revealer,
    bottom_bar_height: Cell<i32>,        // measured via connect_map / notify::height
}

pub const FAB_SPACING: i32 = 16;         // Scaffold.kt FabSpacing = 16.dp

impl Scaffold {
    /// Recompute overlay margins after the bottom bar or FAB changes size.
    pub fn relayout(&self, bottom_inset: i32) {
        let bb = self.bottom_bar_height.get().max(bottom_inset);
        if let Some(fab) = &self.fab {
            // fabOffsetFromBottom = max(bottomBarHeight, bottomInset) + fabHeight + 16
            fab.set_margin_bottom(bb + FAB_SPACING);
            fab.set_margin_end(FAB_SPACING);
        }
        // snackbarOffsetFromBottom = snackbarHeight + (fabOffset ?: max(bottomBarHeight, bottomInset))
        let fab_offset = self.fab.as_ref().filter(|f| f.is_visible()).map(|f| bb + f.height() + FAB_SPACING);
        self.snackbar.set_margin_bottom(fab_offset.unwrap_or(bb));
    }
}
```

Rules encoded (from `Scaffold.kt` lines 220 to 258):
- The FAB exists in the layout only when it is visible and its width and height are non-zero (`fabPlacement` is null otherwise). The `is_visible()` check mirrors this.
- Snackbar offset uses the FAB offset only when a FAB is in the layout; otherwise it uses the bottom bar height.
- Snackbar horizontal centring `(insetWidth - snackbarWidth) / 2` is GTK's `halign = center`.
- FAB at end: `layoutWidth - 16 - fabWidth - rightInset` is GTK's `halign = end` with `margin-end = 16`.
- Center FAB position (`FabPosition.Center`) is not used by the app; if needed, `halign = center`.

### 1.4 Reserved space rule
When a FAB is shown, the scrollable content needs bottom space equal to its offset so the last item can scroll above it. Compose does this through the inner padding. In GTK, set the scrolled child's `margin-bottom` to `fabOffset` only while a FAB is visible. Document this in the page builder (`ui::page`), not in each screen.

---

## 2. FAB expansion (`LazyListState.shouldExpandFAB`)

Compose logic (for both `LazyListState` and `LazyGridState`):

```
expanded = (firstVisibleItemIndex == 0 && firstVisibleItemScrollOffset == 0)
        || lastScrolledBackward
        || !canScrollForward
```

GTK equivalent over `gtk::Adjustment` of a `ScrolledWindow` vertical axis:

```rust
// src/ui/scroll.rs
pub struct ScrollSnapshot { pub value: f64, pub upper: f64, pub page: f64 }

/// Compose: lastScrolledBackward is true when the last scroll moved the list up.
pub fn should_expand_fab(now: ScrollSnapshot, prev_value: f64) -> bool {
    let at_top = now.value <= 0.0;
    let backward = now.value < prev_value;
    let cannot_forward = now.value + now.page >= now.upper - 0.5;
    at_top || backward || cannot_forward
}
```

Notes:
- `prev_value` is the value on the previous `value-changed`; store it on the page.
- `0.5` tolerance is a proposal to absorb sub-pixel rounding (Compose compares integer offsets).
- Expand and collapse animate the label width over 120 ms (proposal; same as the hover timing).
- `lastScrolledBackward` is a `LazyListState` property that reports the direction of the last scroll; the Rust version derives the same flag from the previous adjustment value, which matches it for every scroll that moves the list.

---

## 3. Navigation rail and bar

- Compose `NavigationRail`: min width 80 dp, vertical padding 4 dp, item spacing 4 dp, header then 8 dp spacer, tonal elevation 3 dp, `selectableGroup()`.
- Compose `NavigationBar`: height 80 dp, row, tonal elevation default.
- Neither has a call site in this repo.

Desktop: the mockup sidebar (`src/main.rs::sidebar`, line 154) is the navigation. It keeps its own geometry. The rules to carry over are the minimum width of 80 px and the label rule: below a 600 px window width the labels hide and icons remain (proposal, `p2-ui-parity.md` section 3; task T-49b-017).

---

## 4. Surface

Compose `Surface` is a `Box` with: minimum touch size 48 dp, shadow (`clip = false`), border, background with tint, clip to shape, `combinedClickable` with ripple, long press and role Button.

GTK (proposal):
- Base element: `gtk::Box` (or `gtk::Button` with `flat` when it is a button) with CSS class `kb-surface`.
- Hit target: `min-width: 48px; min-height: 48px` on the child, so the visible shape can be smaller.
- Click: `gtk::GestureClick` (primary button). Long press: `gtk::GestureLongPress` (GTK default delay). Right click: optional `GestureClick` on button 3 when the surface declares a context action.
- Tint: the background colour is computed by `theme::elevation::tint_at` and set in the CSS class for each elevation level used (for example `.kb-elev-1`, `.kb-elev-3`). Only levels used by the app are generated (1 and 3 are the ones in the NavigationRail and bottom bars; add others when a screen needs them).
- Border: `border: 4px solid @color` for the theme card (CSS, not a separate widget).
- Shape: `border-radius` from the CSS class (17 px outer, 13 px inner for the theme card; 8 px for small shapes).

Ripple replacement: the CSS `:hover` background (120 ms in), `:active` (150 ms out). The Compose ripple is a circular expansion; the desktop look is a flat state layer (the M3 "state layer" approach). This is a deliberate look difference; note it in the UI parity file (done).

---

## 5. Slider

Compose `Slider(value: Int, valueRange: IntProgression, steps = last - first - 1)` snaps to integers.

GTK (proposal) `ui::slider::IntSlider`:
```rust
pub struct IntSlider { pub scale: gtk::Scale }
impl IntSlider {
    pub fn new(first: i32, last: i32, value: i32) -> Self {
        let steps = (last - first - 1).max(0);            // S1/S2: clamp steps
        let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, first as f64, last as f64, 1.0);
        scale.set_round_digits(0);
        scale.set_draw_value(true);
        scale.set_value_pos(gtk::PositionType::Right);
        scale.set_digits(0);
        scale.set_value(value.clamp(first, last) as f64);
        let _ = steps; // GTK has no discrete-steps property; step 1 with round-digits 0 gives the same stops
        IntSlider { scale }
    }
}
/// Kotlin Math.round semantics: half rounds toward +infinity (S3).
pub fn round_half_up(x: f64) -> i32 { (x + 0.5).floor() as i32 }
```

Notes:
- GTK `Scale` has no `steps` property; `set_round_digits(0)` plus increment 1.0 gives the same integer stops as Compose `steps = last - first - 1`.
- The value is always an integer in the callback: `round_half_up(scale.value())`.
- `value-changed` fires on every drag step; the "finished" event is emitted on `GestureDrag` end and on keyboard release (proposal). Use it to write preferences (matches `onValueChangeFinished`).
- Checked: every `valueRange =` call in the app is an `IntProgression` with step 1: `0..10` (library and season settings dialogs, `MangaLibrarySettingsDialog.kt` line 262, `AnimeLibrarySettingsDialog.kt` line 262, `SeasonSettingsDialog.kt` line 262), `1..15` and `1..10` (`SettingsReaderScreen.kt` lines 145, 156), `0..200` (`PlayerSettingsAudioScreen.kt` line 92), `0..100` (`PlayerSettingsPlayerScreen.kt` line 187). The 0..200 slider gets `steps = 199` in Compose (201 stops).

---

## 6. Pull to refresh

Compose: gesture-driven indicator at the top centre.

GTK (proposal, T-49b-019): no gesture. A refresh button in the page header (`view-refresh-symbolic`) and the `F5` and `Ctrl+R` accelerators (`gtk::Application::set_accels_for_action("win.refresh", &["F5", "<Control>r"])`). While `refreshing`, the button is insensitive and shows a `gtk::Spinner` in place of the icon.

The `enabled` flag maps to the button's sensitivity. The `indicatorPadding` parameter is not needed (the button has its own margin).

---

## 7. Scrollbar

Dropped. GTK `ScrolledWindow` with `set_overlay_scrolling(true)` (default in GTK 4 on Linux, confirm with the theme; the user's theme may set `gtk-overlay-scrolling`). The Compose sticky-header aware thumb is not reproduced.

Edge handled by GTK: empty list (no thumb), RTL, reverse scrolling.

---

## 8. Elevation and interaction timings

| State change | Compose spec | CSS (proposal) |
|---|---|---|
| Rest to pressed, focused, hovered (incoming) | 120 ms, FastOutSlowIn | `transition: background-color 120ms cubic-bezier(0.4, 0.0, 0.2, 1)` |
| Pressed or focused back to rest (outgoing) | 150 ms, cubic-bezier(0.40, 0.00, 0.60, 1.00) | `transition: background-color 150ms cubic-bezier(0.4, 0.0, 0.6, 1.0)` on `:not(:hover)` |
| Hover back to rest | 120 ms, same curve | `transition-duration: 120ms` on `:not(:hover)` when leaving hover |

GTK CSS transitions apply to `background-color`, `box-shadow`, `opacity`, `color` and `border-color`. Elevation shadows are not used (the M3 tonal elevation is the tint, not a shadow).

---

## 9. Layout constants to copy

| Constant | Value | Used by |
|---|---|---|
| FAB spacing | 16 px (dp at scale 1) | Scaffold |
| Nav rail min width | 80 | sidebar reference |
| Nav bar height | 80 | not used |
| Surface min touch | 48 | Surface, buttons |
| Empty screen horizontal padding | 24 | EmptyScreen |
| Info screen top gap | 48 | InfoScreen |
| Info screen horizontal padding | 16 | InfoScreen |
| Info screen icon | 48 | InfoScreen |
| Padding scale | 32, 24, 16, 12, 8, 4 | every component |
| Theme card width | 114 | theme picker |
| Theme card aspect | 9:16 | theme picker |
| Theme card border | 4 (selected or divider) | theme picker |
| Theme card radius outer, inner | 17, 13 | theme picker |
| Theme card label lines | 2 (min and max) | theme picker |

Scale note: Compose dp at density 1 equals CSS px. The desktop build uses GTK's scale factor, so all sizes stay in px in CSS and are scaled by the compositor.

---

## 10. Things to test in the running app
- A page with a FAB and a snackbar: the snackbar sits above the FAB (16 px gap), not over it.
- Resize the window below 600 px: the sidebar keeps the 80 px minimum and the content does not overlap it.
- Slider at the ends: `value == first` and `value == last` both reachable with the mouse and the keyboard.
- Pressing and holding a surface for the GTK long-press delay fires the long-press action once and does not also fire click.
