# Area 47b — UI parity (what the user sees, and what the mockup lacks)

This area renders UI: settings row widgets, dialogs, the More tab, the update info screen and the onboarding wrapper. Behaviour details live in `p1-behavior.md`; this file lists every visible element, state, dialog, gesture and the desktop equivalent. Paths are repo-relative to `/home/blackbox/aniyomi-src`. Mockup references are `/home/blackbox/kurobayomi/src/*.rs` with line numbers.

## 1. Global look and feel
- Row chrome (all settings rows): min height 56 px; 16 px horizontal padding; 16 px vertical padding; title 16 px, up to 2 lines, ellipsised; subtitle (`bodySmall`) at 78% opacity, up to 10 lines; optional leading icon with 16 px start margin and 8 px gap; optional trailing control with 16 px end margin.
- Group: header label in `colorScheme.secondary`, body `bodyMedium`, top gap 14 px, bottom gap 8 px. Groups separated by a 12 px spacer, none after the last group.
- Colours: follow the active theme (area:49). Nothing is hard-coded except the tracker check green `#4CAF50` and the tri-state/radio colours which use the theme primary.
- Dialogs: 360 px wide desktop window, transient to the main window, modal, not resizable. Title bold `titleLarge`-equivalent (GTK `title-3` class). Buttons right-aligned: Cancel then OK (Compose order: dismiss on left, confirm on right in the Kotlin code; GTK puts primary action last per the GNOME/KDE HIG on Plasma, so keep confirm on the right).
- Accessibility: every icon-only control has a tooltip or accessible label (the Kotlin `contentDescription` strings listed in behavior §X.1). TriState check icons get distinct labels (see §6 defect E-04 fix).

## 2. Settings row types (visible elements and states)

### 2.1 Text row (`TextPreferenceWidget`)
- Elements: optional icon (tint = primary), title, optional subtitle, optional trailing widget.
- States: normal; pressed (GTK `:active` highlight from `ListBoxRow`); disabled (`onClick == null`) → no hover/press feedback, not focusable; disabled item (`enabled = false`) → collapsed (slides out).
- Gesture: click or Enter/Space on focused row → `on_click`.
- Keyboard: Tab focuses rows (ListBox row focus); Enter activates; Up/Down move between rows (GTK ListBox default).

### 2.2 Switch row
- Elements: title, subtitle, `gtk::Switch` trailing (non-interactive, visual).
- Gesture: click anywhere on the row toggles. The switch itself does not react to the pointer (`set_can_target(false)`), so there is one hit target, matching Kotlin.
- Keyboard: Space/Enter on the focused row toggles.
- Pointer: hover highlight on the whole row.
- State: value is always read from the preference; a write that is vetoed leaves the switch unchanged.

### 2.3 Single-choice list row
- Elements: title, subtitle (`"%s"` template with the selected label), no trailing widget.
- Click → dialog (§3.1).

### 2.4 Multi-select row
- Elements: title, subtitle = joined selected labels or "None".
- Click → dialog (§3.2).

### 2.5 Edit-text row
- Elements: title, subtitle (formatted value unless `formatSubtitle = false`).
- Click → dialog (§3.3).

### 2.6 Slider row
- Elements: label (left, `titleLarge` at 16 px), value pill (right; text = subtitle or the number), full-width `gtk::Scale`, step count from the model.
- Gesture: drag or click on the track; keyboard Left/Right step by one unit; Page Up/Down steps by 10% (GTK default for `gtk::Scale` with `step`). Mouse wheel over the scale changes the value by one step (GTK default; Proposal: disable wheel to avoid accidental changes on scroll, which Compose does not do).
- State: value is caller-owned. A dragged value is sent to the veto on each change; the pill updates immediately.
- Haptic on step: dropped.

### 2.7 Tracker row
- Elements: tracker logo (left, `TrackLogoIcon`), tracker name (`titleLarge`, one line), check icon (right, 32 px, green) when logged in.
- States: logged out (no check), logged in (check), login pending (not modelled in Kotlin; the login flow of area:27 owns it).
- Gesture: click → login (when logged out) or logout (when logged in), no confirmation (behavior B.8). Proposal: confirm logout in an `AlertDialog` with Cancel/Log out, because logging out loses the session token. Document as a divergence in the tasks (T-47ba-050).

### 2.8 Info row
- Elements: info icon (`dialog-information-symbolic`) above body text (`bodySmall`), 78% opacity, 16 px horizontal padding, 16 px vertical padding, 16 px gap between icon and text.
- Not clickable, not highlighted.

### 2.9 Group header
- Label only, `secondary` colour, `bodyMedium`, top 14 px bottom 8 px, horizontal 16 px.

### 2.10 Custom row
- Whatever the caller builds. No padding, no highlight (caller-owned).

## 3. Dialogs

### 3.1 Single-choice list dialog
- Title: the row's title.
- Body: radio list of entries in definition order. Each row: radio button, label (`bodyLarge`), 24 px gap, min touch target 48 px. Scroll dividers appear top and bottom when the list can scroll in that direction.
- Selecting a different entry: applies it and closes (no OK button). Selecting the current entry: no change, dialog stays open (behavior B.4).
- Actions: Cancel only (right-aligned). Escape = Cancel.
- Keyboard: Up/Down move focus; Space selects (radio); Enter selects focused radio; Escape cancels.
- Gesture: click on a row label or radio.
- Mouse: hover highlight on row.
- Size: content height grows to the entry count up to the window height; ScrolledWindow with max height 60% of the window (Proposal, the Kotlin uses the dialog maximum).

### 3.2 Multi-select dialog
- Title: row title. Body: checkbox list (`LabeledCheckbox`: 48 px min row, 8 px gap, label), no scroll dividers (plain LazyColumn in Kotlin; Proposal: use the same scroll container as §3.1 for consistency).
- Actions: Cancel (left), OK (right). OK writes the set (order of the set is unspecified, see E-12); Cancel writes nothing.
- Keyboard: Tab through checkboxes, Space toggles, Enter activates OK (default button), Escape cancels.
- State: initial selection = intersection of current values with entries, in entries order.

### 3.3 Edit-text dialog
- Title: row title. Optional second line: `dialogSubtitle` (`bodyMedium`).
- Body: one text entry (single line for `singleLine = true`; a multi-line `gtk::TextView` for `singleLine = false` — MPV conf and multi-line rows).
- Trailing icon in the entry: error icon (`dialog-error-symbolic`, non-interactive) when the text is invalid or blank-not-allowed; otherwise a clear button (`edit-clear-symbolic`) that empties the field (not shown in the error state).
- Supporting text under the entry: the `errorMessage` text when `validate` fails and a message exists; no supporting text for the blank-only error (E-03).
- Entry style: error state shows the entry with the error colour (GTK `error` CSS class).
- Actions: Cancel (left), OK (right). OK disabled unless the text changed, is non-blank-or-canBeBlank, and validates. Enter in the single-line entry = OK when enabled.
- Keyboard: Escape cancels; Enter (single line) submits; in multi-line, Ctrl+Enter submits (Proposal, Compose has no default; document in the help).
- Input purpose: `gtk::InputPurpose` from the item (`keyboardOptions`): Kotlin `KeyboardOptions.Default` → `FreeForm`; the info variant may set another purpose (none in scope sets one; keep FreeForm).
- Failure: if the veto returns false, the dialog stays open with the typed text (no message, behavior B.6).

### 3.4 Tri-state include/exclude dialog
- Title: `title`. Optional message above the list (`bodyMedium`, 8 px bottom gap).
- Body: list of rows. Each row: state icon (checkbox-blank / checkbox-checked / disabled-by-default icons; colours: unchecked = content colour, checked and excluded = primary), item label, 48 px min row height, 20 px gap after icon.
- Click on a row cycles unchecked → checked → excluded → unchecked. Mouse only in Kotlin; keyboard: Space cycles the focused row (Proposal, Kotlin has no keyboard handler).
- Actions: Cancel (left) and OK (right). OK returns (included, excluded) and the caller closes.
- Accessible names: each icon gets its own label: "Not included" / "Included" / "Excluded" (Proposal; Kotlin reuses "Disabled" for all three — defect E-04).

### 3.5 Update info screen (presentation `NewUpdateScreen`)
- Shown as a full page in the Kurobayomi window (not a dialog), matching the mockup `info_screen` layout (`src/screens.rs:492-535`).
- Elements: large icon (`software-update-available-symbolic`, 48 px), heading "New version available!" (title-1), subtitle = version name (dim), changelog body rendered from markdown (headings, bullets, links styled with the primary colour), "Open on GitHub" link button with an external-link icon, and two buttons at the bottom: "Download" (suggested-action, left) and "Not now" (right of Download in the mockup; Kotlin puts Accept then Reject in InfoScreen).
- Changelog: markdown → Pango markup in a wrapping `gtk::Label` with `use_markup(true)`. Links open with `gtk::UriLauncher`.
- Scroll: the body scrolls; the buttons stay pinned at the bottom (Proposal; Kotlin scrolls the whole InfoScreen).
- States: loading (none; the changelog is already in hand from the check); empty changelog → body shows nothing, buttons still work; error (none in scope).
- Keyboard: Enter → Download (suggested), Escape → Not now.
- After Download: the page closes immediately and the download appears as a notification (area:35).

### 3.6 Onboarding wrapper (presentation steps owned by area:36)
- The wrapper adds: window close blocked until `onboarding_complete` is true (Escape/back do nothing) — mockup `info_screen("onboarding")` at `src/screens.rs:506-535` does not block back; required fix (§5).
- After finish, the page is popped; if the restore path was taken, the Data-and-storage page opens with the "Backup and restore" row flashed (§C of behavior).

## 4. The More tab (page, in order)
1. Logo header: 64 px app icon, 56 px above and below, then a separator.
2. "Downloaded only" switch row. Subtitle "Filters all entries in your library". Icon: offline/cloud-off. Click toggles. Persisted immediately.
3. "Incognito mode" switch row. Subtitle "Pauses your history". Icon: glasses (bundled SVG, area:49b). Click toggles.
4. Separator.
5. Alt-nav row. Title is the moved tab label, e.g. "Updates" or "History" (default history). Icon: that tab's icon. Click opens that tab's page.
6. "Download queue" row. Subtitle: none when stopped; "Paused • N remaining" or "Paused" when paused; "N remaining" when running. Click opens the download queue page (area:23-25).
7. "Categories" row (tag icon). Click opens the categories page.
8. "Statistics" row (chart icon). Click opens stats (area:42).
9. "Data and storage" row (drive icon). Click opens the Data and storage settings page.
10. Separator.
11. "Settings" row (gear icon). Click opens the settings main menu.
12. "Player settings" row (video-display icon). Click opens the player settings page.
13. "About" row (info icon). Click opens the About page.
14. "Help" row (help icon). Click opens the troubleshooting docs URL in the browser.

- Row order and count are fixed. Missing vs mockup: none of the 14 rows is absent; the mockup's History link is removed (`src/screens.rs:344-347`), which is correct only while History is a sidebar item (see §5).
- Empty/error states: none (the More page always has rows).
- Pointer: hover highlight on rows; no context menus.
- Keyboard: Up/Down through rows; Enter activates. No page-specific shortcuts are defined here; global shortcuts belong to area:36.
- Responsive: the list is a single column with a max content width of about 720 px centred in the window (Proposal; mockup uses full width). At phone-like widths (not a target on desktop) the rows wrap title first.

## 5. Mockup gap list (exact)

| # | Area of mockup | Mockup has | Kotlin/target has | Fix task |
|---|---|---|---|---|
| G-01 | `src/ui.rs` `item_row` `Item::List` (lines 231-239) | `gtk::DropDown` inline | modal radio dialog that closes on selection (§3.1) | T-47ba-046 |
| G-02 | `src/ui.rs` `item_row` `Item::Multi` (lines 240-250) | MenuButton with "N selected" label and popover | checkbox dialog with OK/Cancel, subtitle lists labels or "None" (§3.2) | T-47ba-047 |
| G-03 | `src/ui.rs` `item_row` `Item::Slider` (lines 251-260) | `gtk::Scale` with draw_value and no pill; subtitle is `unit` | label + value pill + scale; no unit text in the pill except subtitle (§2.6) | T-47ba-052 |
| G-04 | `src/ui.rs` `item_row` `Item::Text` (lines 261-264) | single-line `gtk::Entry`, inline | text row that opens a dialog with OK/Cancel and validation (§3.3) | T-47ba-043 |
| G-05 | `src/ui.rs` `Item::Tristate` (lines 265-287) | cycles off → include → exclude with a label | matches cycle; lacks dialog OK/Cancel scoping and accessible labels (§3.4) | T-47ba-051 |
| G-06 | `src/ui.rs` `Item::Chips` (lines 301-320) | FlowBox of toggles for >4 options | theme strip = preview cards (§T.2 in behavior); chips used only for ThemeMode (3 options) | T-47ba-041 |
| G-07 | `src/ui.rs` `row` helper (lines 147-160) | title + optional subtitle + suffix, no icon | icon column, 56 px min height, title max 2 lines | T-47ba-006 |
| G-08 | `src/screens.rs` `more()` (lines 336-352) | fixed list, History removed, no dynamic subtitles | dynamic download-queue subtitle, alt-nav row, switch bindings to state | T-47ba-010, T-47ba-016 |
| G-09 | `src/screens.rs` `more()` | link to `download_queue` with spec subtitle | subtitle from live queue (§8.3 rust design) | T-47ba-016 |
| G-10 | `src/screens.rs` `more()` | no Help row action | Help opens `https://aniyomi.org/docs/guides/troubleshooting/` | T-47ba-014 |
| G-11 | `src/screens.rs` `info_screen("onboarding")` (lines 497-535) | Next / Get started; back not blocked | back blocked until complete; restore highlight | T-47ba-060, T-47ba-061 |
| G-12 | `src/screens.rs` `info_screen("new_update")` (line 498) | subtitle `v0.1.0` hard-coded; buttons Download and Not now | version from update check; changelog cleanup (strip checksums) | T-47ba-031 |
| G-13 | any settings page | no highlight/flash on search jump | 500 ms delay, scroll, 3 s flash (§C) | T-47ba-020, T-47ba-021 |
| G-14 | any settings page | no disabled-item animation | revealer slide 150 ms; disabled groups removed | T-47ba-004 |
| G-15 | theme strip | none (chips) | 18 preview cards on Linux, selected border, live apply | T-47ba-041, T-47ba-042 |
| G-16 | More tab | no logo header with 56 px padding | logo header per §L | T-47ba-012 |
| G-17 | update changelog | no markdown rendering | markdown to Pango labels with links | T-47ba-031 |
| G-18 | tracker row | none in More (tracker rows only in tracking page, area:47) | tracker row widget | T-47ba-050 |

## 6. Defects in the Kotlin UI that Kurobayomi should NOT copy
- E-04: tri-state icon accessible name is "Disabled" for all states (behavior B.11). Kurobayomi: "Not included" / "Included" / "Excluded".
- E-07: highlight index counts rows in a disabled group that are never rendered (behavior A.4). Kurobayomi uses widget lookup (rust design §6.2).
- E-12: multi-select subtitle depends on set iteration order. Kurobayomi: entries order.
- E-03: blank invalid text shows only an icon, no explanation. Kurobayomi: add the supporting text "Required" only if an app string is added in area:51; otherwise keep the icon and tooltip "This field can't be empty".
- Radio list closes only on a change; there is no OK button. Kept (parity), but the dialog must not close when the user re-selects the current item (behavior B.4).

## 7. Keyboard and mouse summary (desktop)

| Control | Mouse | Keyboard |
|---|---|---|
| Switch row | click anywhere | Tab to row, Space/Enter toggles |
| Text row | click | Tab, Enter |
| List dialog | click a radio row (closes) | Up/Down, Enter/Space selects, Escape cancels |
| Multi-select dialog | click checkbox row | Tab, Space toggles, Enter = OK, Escape = Cancel |
| Edit-text dialog | click in entry, clear button | Enter submits when valid (single-line), Escape cancels |
| Slider | drag, click track, wheel | Left/Right = 1 step, PgUp/PgDn = 10% |
| Tri-state | click row cycles | Space cycles focused row (Proposal) |
| Theme mode | click segment | Tab, Space selects |
| Theme strip | click card (applies immediately) | Tab, Enter applies |
| Tracker row | click login/logout | Enter |
| More rows | click | Up/Down, Enter |
| Update screen | buttons, link | Enter = Download, Escape = Not now |
| Onboarding | buttons | Escape/close does nothing until finished |

## 8. Transitions and animation
- Disabled item collapse/expand: 150 ms slide (`gtk::Revealer` `slide-down`/`slide-up`) plus crossfade. Kotlin uses `expandVertically`+`fadeIn` and `shrinkVertically`+`fadeOut` (default durations).
- Highlight pulse: 5 cycles of 200 ms, starting after 600 ms, total 3 s (behavior §C.2; rust design §6.4).
- Dialog open/close: GTK default window transition (no custom animation).
- Slider: no animation (Compose default none).
- Theme switch: immediate CSS swap (Kotlin recreates the Activity; Linux has no restart).
- Animated More icon (`anim_more_enter`): dropped; static icon. Note in the user loss table of the tasks.

## 9. Responsive behaviour
- Settings pages: max content width 720 px centred; below 720 px the content fills the window with the 16 px gutter.
- Theme strip: horizontal scroll; cards are 114 px wide, so 6 visible at 720 px and the rest reachable by scroll (not wrapped). Mockup's FlowBox wrap is replaced.
- Dialogs: fixed width 360 px; height grows with content up to 80% of the window.
- Update screen: body scrolls; buttons pinned.
- Minimum window size (area:36) must keep one settings row readable: 360 px minimum width.

## 10. Empty, loading and error states
- Settings pages: never empty (groups always have rows). If a page has zero rows (e.g. all groups disabled), show the page title and no body (Kotlin shows nothing; no empty state).
- More tab: always full.
- Download queue row: "Stopped" state shows no subtitle (not "0 remaining").
- Theme strip: never empty on Linux (18 cards). If the palette lookup fails for one theme, skip that card and log.
- Tracker row: logged-out state shows no check; no loading spinner (the login flow of area:27 shows its own progress).
- Update screen: no changelog → body empty but buttons work.

## 11. Icon and symbol map (summary; full list in data-and-config §8)
Use GTK symbolic icons. Verify presence with `gtk4-icon-browser`. Icons not available in the user's icon theme fall back to a generic missing icon, so the verification is a release gate (T-47ba-049 checks the list).

## 12. Strings (English, shown in the UI)
All strings listed in behavior §X.1 are required verbatim. Strings that Kurobayomi adds (proposals only): "Not included", "Included", "Excluded", "This field can't be empty", "Could not save this setting", "Log out of {tracker}?" (only if the logout confirmation proposal is accepted). These are NOT in the Kotlin tree and must be marked as proposals in the tasks file and translated through area:51 when adopted.
