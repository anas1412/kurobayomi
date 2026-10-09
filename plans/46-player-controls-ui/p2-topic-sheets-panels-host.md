# Topic: the sheet and panel host (open, close, sizing, focus)

Deep dive for `p2-behavior.md` §0.1-§0.3 and `p2-tasks.md` T-46b-001, T-46b-046. Repo root `/home/blackbox/aniyomi-src`.

## 1. What the Kotlin host does

- `PlayerSheet` (`app/src/main/java/eu/kanade/presentation/player/components/PlayerSheet.kt`) is the only container. Every sheet in scope passes its content into it. Panels (`SubtitleSettingsPanel`, `SubtitleDelayPanel`, `VideoFiltersPanel`, `AudioDelayPanel`) are not wrapped in `PlayerSheet`. They are placed by `PlayerPanels` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerPanels.kt:46-80`) in a `ConstraintLayout`-style container aligned to 80% height.
- One sheet is shown at a time: `PlayerSheets` is a `when (sheetShown)` over the `Sheets` enum (`PlayerSheets.kt`). Panels are chosen by `PlayerPanels` `when (currentPanel)`.
- Sheet opening is driven by state (`sheetShown: Sheets`, `currentPanel: Panels`), not by navigation. A sheet opens a panel by calling `onOpenPanel(Panels.X)`, which sets the panel state; the sheet closes through the same dismiss path.

## 2. Lifecycle

| Phase | Kotlin | Port |
|---|---|---|
| Open | `LaunchedEffect(true)` raises the scrim to 0.5 over 350 ms; `animateTo(0)` moves the sheet up | `Revealer` reveal, scrim opacity 0.5, 350 ms |
| Interact | sheet surface swallows clicks; drag gesture moves the sheet | clicks are claimed by the card; no drag |
| Close by scrim | `internalOnDismissRequest` only when `currentValue == 0`: alpha 0 then `animateTo(1)` | scrim click → `close()` |
| Close by Back | `BackHandler(enabled = targetValue == 0)` → same path | Escape → `close()` |
| Close by drag | `currentValue == 1` after drag → `onDismissRequest()` | not provided (see parity list) |
| Close by external signal | `dismissEvent` changes to true → alpha 0, `animateTo(1)`, `onDismissRequest()` | `close()` called by the owner |

Two rules to keep:
- The dismiss callback runs **once**, after the closing animation. The port guards with a flag (T-46b-001 acceptance).
- A closed sheet does not keep its state. Sheets rebuild from the model on open (the Kotlin `remember` values die with composition).

## 3. Sizing

| Input | Kotlin | Port |
|---|---|---|
| Max width, portrait | 420 dp | 420 px |
| Max width, landscape | 720 dp | 720 px when window width ≥ 900 px |
| Max height | 95% of screen height | 95% of player height |
| Panel width | `CARDS_MAX_WIDTH = 420.dp` (`PlayerPanels.kt:81`) | 420 px |
| Panel vertical position | `linkTo(parent.top, parent.bottom, bias = 0.8f)` | card top at 80% of available height minus card height |
| Subtitle settings layout | pager (portrait) or stack (landscape) | single stack with tabs |

Breakpoint note: the Kotlin switch is `LocalConfiguration.orientation`. The desktop has no orientation concept, so the width breakpoint replaces it. The 900 px value is a proposal; it is the smallest width at which two 420 px cards can sit side by side next to the player controls.

## 4. Focus and keyboard

- On open: focus the first actionable control (for sheets with a header action, the first header button; otherwise the first list row).
- Tab and Shift+Tab move through controls inside the card only (focus trap).
- Escape closes. Enter and Space activate buttons and toggles.
- Slider keys: arrow keys step by the slider step; Page Up and Page Down step by ten steps (proposal).
- On close: focus returns to the control that opened the sheet. The host stores a weak reference to the opener when `open` is called.

## 5. Stacking with other overlays

- The player's own control overlay (area:45) is below the sheet. Observed: the auto-hide `LaunchedEffect` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/PlayerControls.kt:134-146`) hides controls after `playerTimeToDisappear` without checking whether a sheet or panel is open. The sheet itself stays visible because it is a separate layer, but the controls underneath can disappear while the user is in the middle of a sheet. Proposal (flagged, not parity): keep controls visible while a sheet or panel is open. The timer owner is area:45; this part only requires the host to expose `is_open()`.
- Dialogs opened from a sheet (sleep timer, set-as-art confirm, file pickers) are separate windows or `gtk::AlertDialog`s transient for the player window. The sheet stays open underneath. Kotlin: the `TimePickerDialog` and `PlayerDialog` are composed inside the sheet, so they close with it only when the sheet is removed.

## 6. What is intentionally not reproduced

- Drag to dismiss and fling physics (Android gesture). Users lose the flick gesture; scrim, Escape, and close buttons remain.
- Haptic feedback on slider changes.
- Swipe paging for the three subtitle cards (replaced by a tab stack).
- The Android system-bar insets logic (no equivalent on desktop; window decorations are handled by GTK).

## 7. Desktop additions (proposals, flagged)

- Escape closes the top-most sheet first if a dialog is open over it (dialog closes first).
- Mouse wheel over a chip row scrolls it horizontally.
- Long press is also a secondary (right) click where a long-press action exists (custom buttons).

## 8. Rust surface

`SheetHost::{new, open, open_panel, close, is_open}` as in `p2-rust-design.md` §3. Sheet builders are plain functions `fn build_*(model: &…, on: impl Fn(Event)) -> gtk::Widget`, so the host does not know sheet contents. The host holds at most one sheet and one panel at a time (matches Kotlin `when` semantics: sheets and panels are independent in state but the UI shows one of each).

Test hooks: the host's open/close state machine can be tested with a fake card widget only if the GTK init is allowed in tests; keep the state machine (`HostState { sheet, panel, closing }`) as a pure struct and test that. Parity tests: E36, E37 in `p2-edge-cases-and-tests.md`.
