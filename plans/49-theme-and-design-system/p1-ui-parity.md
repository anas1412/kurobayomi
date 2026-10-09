# Area 49 (part 1): UI parity

Scope renders UI: theme preview (`AppThemePreferenceWidget.kt` consumes these components), settings rows, dialogs, sheets, scrollers, wheel pickers, FAB, badges. Screens that use them belong to other areas; this file lists what each component shows and how it behaves, and what the Kurobayomi mockup lacks.

Mockup baseline (verified in `/home/blackbox/kurobayomi/src`): no `CssProvider` or `style_context` calls exist; the app uses stock GTK style classes only (`suggested-action`, `pill`, `osd`, `toolbar`, `circular`, `card`, `boxed-list`, `navigation-sidebar`, `dim-label`, `caption-heading`, `heading`, `title-1..4`, `accent`, `monospace`, `linked`). No theme choice exists. No sheet, no fast scroller, no wheel picker, no FAB, no badges with colors, no numeric chooser.

## 1. Theme picker (consumer: `presentation/more/settings/widget/AppThemePreferenceWidget.kt`, owned by area 47)

Visible:
- A horizontally scrolling row (`LazyRow`, horizontal padding 16 dp, spacing 8 dp) of 19 entries in `AppTheme.entries` order, minus hidden entries (`titleRes == null`) and MONET when dynamic color is unavailable.
- Each card: width 114 dp, top padding 8 dp. The card is a 9:16 `AppThemePreviewItem`: border 4 dp (primary when selected, `DividerDefaults.color` otherwise) with corner radius 17 dp; inner padding 4 dp; clip 13 dp; background = theme `background`.
- Preview contents: a 40 dp app bar (a bar of `onSurface` with `small` shape at 0.7 weight, a check icon at 0.3 weight when selected), then a cover placeholder (`DividerDefaults.color`, `small` shape, 50% width, book aspect ratio), then rows of placeholder bars.
- Label below each card: `bodyMedium`, centered, max 2 lines, min 2 lines (reserves two lines), alpha 0.78 (`secondaryItemAlpha`).
- Selected card shows `CheckCircle` icon (primary) with content description `selected` ("Selected").

Behavior:
- Tap a card: `onItemClick(appTheme)`, then `ActivityCompat.recreate(activity)` (Android recreates the activity to apply the theme). Kurobayomi: apply immediately with `theme::apply` (no restart; the CSS provider swaps live).
- The AMOLED switch affects every preview (previews are rendered with `TachiyomiTheme(appTheme, amoled)`).

Empty / error states: none. Monet hidden when unavailable. If Monet is the stored value and unavailable, the picker shows no selected card (OBSERVED: the stored value is not in the list). Proposal: fall back to `System` on Linux when the stored value is hidden, and show the System card selected.

Desktop equivalents: keyboard focus ring on the cards (`gtk::Button` with focus), arrow keys move between cards (GTK default in a `FlowBox` or horizontal `ListBox`), Enter/Space selects. Mouse hover: 0.10 state layer (see section 6).

Mockup gap: no picker. The whole picker is a P1 task (T-49a-062 in `p1-tasks.md` covers the preview card rendering; the settings page placement is owned by area 47).

## 2. Settings rows (`SettingsItems.kt`)

Visible structure (all rows): 24 dp side insets, 10 dp vertical insets, body text 14/20, leading or trailing widget 24 dp, row height about 48 dp min (content driven).

| Row | Visible | States | Interaction | Keyboard/mouse on desktop |
|---|---|---|---|---|
| Heading | header text, SemiBold, onSurfaceVariant | none | none | non-focusable |
| Icon row | label + primary icon | normal | click row | row focus, Enter |
| Sort row | label + arrow (down/up) or blank spacer | sorted or not | click cycles the owner's sort (owner logic) | same |
| Checkbox row | label + checkbox | checked, unchecked, disabled (not used by callers) | click toggles | Space toggles on focus |
| Radio row | label + radio | selected/unselected | click selects | Space selects |
| Slider row | label + value pill on top, slider below (2 dp gap) | value, min..max | drag; haptic removed on Linux | arrows +/-1, Home/End, PageUp/Down step 10 (GTK default for `gtk::Scale`) |
| Select row | outlined text field with label and value, trailing arrow; menu list | closed, open, selected item | click opens dropdown; item click selects and closes | Alt+Down or Enter opens; arrows; Enter selects |
| TriState row | state icon (blank / check / disabled-by-default) + label | DISABLED, ENABLED_IS, ENABLED_NOT; disabled row alpha 0.38 | click cycles | Space cycles |
| Numeric chooser | minus circle, outlined field (min 140 dp, suffix), plus circle | value; min clamp on buttons | press and hold repeats (section 9.8 of rust design) | arrows Up/Down on the field (Proposal) |
| Text row | outlined text field with label, margins 24/4 | editing, empty | typing | Tab to move |
| Chip row | heading + wrapping chips (FlowRow), spacing 8 | selected chip | click chip | Tab, Space |
| Icon grid | heading + grid of icon tiles, adaptive 128 dp columns | selected tile | click tile | arrow keys in `GridView` |

Empty states: none in the component; callers decide.

## 3. Badges (`Badges.kt`)

Visible: small rounded label (corner 4 dp for group, rectangle for single badge), 3 dp horizontal and 1 dp vertical padding, text 12 sp Medium, one line. Color roles: `secondary` (unread) and `tertiary` (downloaded) per scheme comments, `onSecondary` / `onTertiary` text. Icon badges: 12 sp icon inline.

States: none (static). Overflow: `maxLines = 1` (text is clipped, no ellipsis specified in source; Kurobayomi: ellipsize end).

Mockup gap: badges exist in the mockup as `accent` label text (`main.rs:184`), not as filled badges.

## 4. Dialog content (`AlertDialog.kt`)

Visible: a rounded container (28 dp corners, `surfaceContainerHigh`, the caller's M3 dialog frame), width clamped to 280..560 dp. Top: optional icon (secondary color, centered, 16 dp bottom gap), title (headline small, centered if icon else start), text body (on_surface_variant, 14/20, 24 dp bottom gap), then buttons end-aligned, 24 dp padding, label-large, primary color.

States: icon-only and title-only variants exist; a text-only dialog has no header; a dialog with no buttons is possible (text block only).

Dismiss: outside tap and Escape (Compose: back; Proposal: Escape plus close button). Focus trap: modal window (GTK).

Mockup gap: no dialogs in this area's code path. The mockup uses `gtk::Window` placeholders only where present in `settings.rs`; none of them use the dialog frame.

## 5. Adaptive sheet (`AdaptiveSheet.kt`)

Visible:
- Phone: bottom sheet, rounded top corners 28 dp, `surfaceContainerHigh`, width up to 460 dp (portrait) or 600 dp (landscape), padded by the status and navigation bars (Android). Linux: no inset padding (the window decorations are the desktop's).
- Tablet: centered card, max width as above, vertical margin 16 dp, fully rounded 28 dp corners, fades in and out over 350 ms. No visible scrim (observed).

States: closed, opening, open, dragging (with the sheet following the finger), settling, closing. Swipe dismiss off: sheet cannot be dragged; only outside tap or back dismiss it.

Gestures: drag down from the sheet, fling down, outside tap (phone: when open; tablet: any time), back.

Desktop equivalents: Escape closes; outside click closes (Proposal: a 0.32 black scrim is NOT added because the source has none; decision recorded in `p1-tasks.md` T-49a-023 as "match source: no scrim"). Pointer drag on the top handle (Proposal: a 32 px wide, 4 px tall handle bar at the top of the sheet, which the source does not have; add as a P2 polish, since on desktops dragging a sheet is unusual).

Mockup gap: no sheet.

## 6. Ripple and state layers (`TachiyomiTheme.kt`, `playerRippleConfiguration`)

Visible: a translucent overlay on press, hover, focus and drag, alpha 0.10, color white in system dark, black in system light (player only). Other Compose widgets use the M3 default ripple of the scheme's content color (not defined here).

Kurobayomi: CSS on `button:hover`, `button:focus-visible`, `button:active`, `row:hover` add `background-image: linear-gradient(alpha(@kuro_on_surface, 0.10), alpha(@kuro_on_surface, 0.10))` (state layer). Player overlay controls use white/black per the system dark flag (see `p1-behavior.md` 1.4). Keyboard focus shows a 2 px outline in `@kuro_primary` (GTK focus ring, already present in the user theme).

## 7. Fast scroller (`VerticalFastScroller.kt`)

Visible: a 12 dp wide, 48 dp tall rounded thumb (6 dp corner) in primary color, 8 dp from the end edge (plus `endContentPadding`). Appears on scroll, stays 2 s, fades over 250 ms. Shown only when the list overflows the viewport. The thumb is hidden when `thumbAllowed()` is false (callers use it to hide while, for example, a selection mode is active).

States: hidden, visible (idle), visible (dragged), fading.

Gestures: drag the thumb to scroll (proportional); scroll the list (thumb follows). Thumb drag is blocked while the list is scrolling (`!isScrollInProgress`).

Desktop equivalents: thumb is always reachable with the mouse (the GTK overlay scrollbar would also work; the fast scroller is kept because the request is parity). Mouse wheel and touchpad scroll work natively.

Mockup gap: none of the scrollers exist. The mockup uses `gtk::ScrolledWindow` defaults (GTK overlay scrollbars that fade on their own).

## 8. Wheel picker (`WheelPicker.kt`)

Visible: a 128 x 128 dp box with three visible rows; the center row is selected and highlighted by a rounded 16 dp rectangle (primary at 20% alpha, 1 dp primary border). Non-center rows are faded (alpha 0.2 at distance, 1.0 at center). Text is `titleMedium`, one line.

States: idle, scrolling (snap pending), selected, manual input (number variant only, a text field replaces the list with the number keyboard and a Done action).

Gestures: drag or fling to scroll, snap to nearest; tap opens manual input (number variant).

Desktop equivalents: scroll wheel and drag; Up/Down keys change selection by one (Proposal); double-click or Enter opens manual entry (Proposal, Compose tap-to-edit is mapped to double-click so single click on a list row remains selection).

Mockup gap: no wheel picker. The picker is used by reader and settings, owned by other areas.

## 9. Extended FAB (`FloatingActionButton.kt`)

Visible: a pill-ish rounded button, container `primaryContainer`, content `onPrimaryContainer`, with icon and optional label. Collapsed: 56 dp wide, icon only. Expanded: at least 80 dp, label visible with 12 dp / 20 dp inner paddings. Expand animation: fade in (200 ms after 100 ms delay) plus expand from the start edge (500 ms emphasized). Collapse: fade out (100 ms) plus shrink (500 ms). Width change 500 ms, start padding 300 ms (expand) / 900 ms (collapse).

States: expanded, collapsed, pressed, hovered, focused, disabled (disabled elevation 0).

Desktop equivalents: keyboard focus ring; Tab order; Enter activates. Hover shows the state layer.

Mockup gap: no FAB (the mockup's "Resume" button in `entry.rs:209` is a `suggested-action` pill, not an extended FAB).

## 10. Progress ring (`CircularProgressIndicator.kt`)

Visible: indeterminate spinner (M3 spinner) when progress is exactly 0; determinate arc with a 2 s rotation otherwise. Crossfade between them (fade in / fade out).

States: indeterminate, determinate at p in (0, 1].

Desktop: `gtk::Spinner` plus a drawn arc. Reduced motion: the GTK `gtk-enable-animations` setting disables the spin; the arc then stays static (Proposal, honors the setting).

Mockup gap: none; mockup uses `gtk::Spinner`-less placeholders.

## 11. Collapsible box (`CollapsibleBox.kt`)

Visible: a header row (header style, 24/12 padding, chevron at the end). Expanded body below. Collapsed by default. Chevron swaps between expand-less and expand-more.

Desktop: `gtk::Expander` with a chevron, keyboard toggling with Enter or Space.

## 12. Section card (`SectionCard.kt`)

Visible: optional title above (title small, 32 dp side inset), then a card with a 16 dp inner padding, 16 dp side margins, 8 dp vertical gaps, 28 dp corners, elevated (tonal shadow per M3).

Used inside `LazyColumn` items (LazyItemScope receiver). On Linux, a `gtk::Box` section in a `ScrolledWindow`.

## 13. Two-panel layout (`TwoPanelBox.kt`)

Visible: start panel up to 450 dp (or half the width), end panel fills the rest, both top-aligned.

Desktop: `gtk::Paned` (user-resizable). Proposal: keep the 450 dp cap as the initial position and a minimum for the start panel of 280 dp. This is a desktop change; record in the task.

## 14. Link icon, action button, list group header, pill, toggle button, labeled checkbox

- Link icon: flat icon button, primary tint, tooltip = label (content description). Click opens the URL in the default browser.
- Action button: vertical icon over label, text-button style, centered. Used in action bars.
- List group header: body medium SemiBold, on_surface_variant, 16/8 padding.
- Pill: rounded value chip, 6/1 padding, 4 dp start margin, one line.
- Icon toggle: 48 dp tall filled toggle, icon + up to 2 lines of title, ellipsis.
- Labeled checkbox: full-width row, min height 48 dp, label to the right of the checkbox; the whole row toggles.

## 15. Buttons and dialogs elements

- Filled button: primary container, on-primary text, label large, min 58x40 dp (M3 library defaults; Kurobayomi uses CSS min-width 58 px and min-height 40 px), full pill radius. Disabled: 12% on_surface container, 38% text.
- Text button: transparent, primary text, 12 dp side padding. Disabled: 38% text.
- Long-press variant on both (Compose `onLongClick`): Kurobayomi uses `gtk::GestureLongPress` (about 500 ms hold per GTK default).

## 16. Numeric chooser (`SettingsItems.kt:505-553`)

Visible: a row of two circle icon buttons with an outlined field between, field min width 140 dp, label above, suffix text at the end, placeholder when empty.

States: value, min reached (minus button does not go below min, because clamp applies), typed invalid text (reverted to the last valid integer).

Desktop: spin buttons with keyboard Up/Down. Proposal fixes the reversion bug (E-49-20) by allowing intermediate "-" and clamping typed values to min on commit.

## 17. Icon grid and chip rows

Chip row: wrapping chips in a `FlowRow` under a heading (16 px gap grid: 8 dp). Icon grid: adaptive tiles, min 128 dp columns, 8/4 dp gaps, under a heading.

## 18. Empty, loading, error states

None in this area. Loading and empty states are owned by screens (area 37-41, `EmptyScreen` in part 2 of this area). The progress ring is the only loading indicator defined here.

## 19. Text and localization

All strings are supplied by callers via `StringResource` (Compose) or Rust strings. The only strings defined by this area are the scheme names in `p1-behavior.md` section 12 and the `selected` content description.

Kurobayomi: scheme names are English-only for the first release; translations follow the Aniyomi `i18n` keys (`MR` and `AYMR` base strings) via the area that owns localization (Proposal: a `kuro-i18n` string table keyed by the same identifiers).

## 20. Responsiveness

- Window class (section 7 of `p1-rust-design.md`): compact (<= 600), medium (601..840), expanded (> 840) by content width in dp.
- Sheet width and dialog width clamp as in section 2/4 above.
- Settings rows scale with text; no fixed heights.
- Two-panel becomes single panel only if the caller decides; this component does not switch by itself.

## 21. What the mockup lacks (summary)

1. No theme engine, no scheme choice, no dark/light/AMOLED settings (the mockup is always the stock GTK look).
2. No CSS provider; no `@define-color` tokens; no `.kuro-*` classes.
3. No theme picker card preview.
4. No adaptive sheet, no tablet dialog mode, no swipe-to-dismiss.
5. No fast scroller, no wheel picker, no numeric chooser with repeat.
6. No extended FAB; no badges with semantic colors; no pill/section card classes.
7. No window size class logic.
8. Settings rows exist only as stock `ListBox` rows (no slider, select, tri-state, or numeric variants).
