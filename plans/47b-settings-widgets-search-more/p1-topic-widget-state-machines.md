# Area 47b — Topic: widget and dialog state machines

Purpose: one place where every interactive widget in scope has an explicit state machine, so the Rust implementation can be checked state by state. Source behaviour: `p1-behavior.md §B`. Rust structure: `p1-rust-design.md §3`. Test IDs: `p1-edge-cases-and-tests.md`.

Notation: `[state]` is a stable state, `(event) -> [state]` is a transition, `{guard}` is a condition, `/ action` is a side effect. Paths for the Kotlin source are repo-relative to `/home/blackbox/aniyomi-src`.

## 1. Preference write (shared by every bound widget)

Source: `app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt` (per-variant `onValueChanged` and `preference.set` calls).

```
[Idle(value=v)]
  (user commits new value n) -> [Vetoing(n)]          / start veto future with n
[Vetoing(n)]
  (veto resolves true)       -> [Idle(value=n)]       / pref.set(n); widget shows n
  (veto resolves false)      -> [Idle(value=v)]       / widget reverts to v; no write
  (veto errors)              -> [Error(v, msg)]       / keep v; show inline error (Proposal, E-18)
  (user commits m while Vetoing(n)) -> [Vetoing(m)]   / cancel the n future; start m (E-19)
[Error(v, msg)]
  (user edits or retries)    -> [Idle(value=v)]
```

- Kotlin: the veto is `suspend`, launched in `rememberCoroutineScope()`. A second commit launches a second coroutine; the last `set` wins. Kurobayomi cancels the earlier future (E-19).
- Veto errors are not modelled in Kotlin (silently cancelled). The `[Error]` state is a Proposal.
- Bound widgets re-render on `pref.watch()` changes; those re-renders must not produce a commit (E-59).

## 2. Switch row

Source: `widget/SwitchPreferenceWidget.kt:36`.

```
[Off] / [On]
  (row activated) -> commit !current   (uses the same preference write state machine as §1)
  (pref.watch changes) -> set switch to new value, no commit
```

- Visual: `Switch(onCheckedChange = null)` so only the row is interactive.
- Keyboard: Space/Enter on focused row = activation.

## 3. Text row

Source: `widget/TextPreferenceWidget.kt`, `widget/BasePreferenceWidget.kt:57`.

```
[Static]           -- no on_click: not activatable, no hover
[Activatable]      -- on_click present
  (activate) -> run on_click (for EditText/List/Multi: opens the dialog, §4-§6)
```

## 4. Single-choice list dialog

Source: `widget/ListPreferenceWidget.kt:48-80`, `DialogRow` lines 84-110.

```
[Closed]
  (row activated) -> [Open(selected=value)]
[Open(selected=s)]
  (click row k where k == s)   -> [Open(selected=s)]           / no-op (E-24)
  (click row k where k != s)   -> [Closed]                     / onValueChange(k) -> commit (§1)
  (Cancel or Escape or close)  -> [Closed]                     / no write
  (pref.watch changes while Open) -> [Open(selected=s)]        / snapshot kept (E-30 analogue)
```

- Kotlin has no OK button; the only confirm is selection itself.
- Entries are iterated in definition order.

## 5. Multi-select dialog

Source: `widget/MultiSelectListPreferenceWidget.kt:34-82`.

```
[Closed]
  (row activated) -> [Open(working = entries ∩ values)]     / snapshot at open (E-30)
[Open(working=W)]
  (toggle key k)   -> [Open(working = W xor {k})]           / no write
  (OK)             -> [Closed]                              / write set(W) through §1; unknown keys dropped (E-29)
  (Cancel/Escape)  -> [Closed]                              / no write
```

- Initial `W` keeps entries order.
- OK with an unchanged `W` still writes (Kotlin always calls `onValuesChange`), which drops unknown stored keys (E-29). Kurobayomi keeps this because it is parity; the task T-47ba-047 records it for review.

## 6. Edit-text dialog

Source: `widget/EditTextPreferenceWidget.kt:46-122`.

Variables: `orig` = stored value at open time (snapshot), `text` = current text, `canBeBlank`, `validate`.

```
derived:
  blankBad  = (text.isBlank() && !canBeBlank)
  invalid   = blankBad || !validate(text)
  canOK     = (text != orig) && (text.isNotBlank() || canBeBlank) && validate(text)

[Closed]
  (row activated) -> [Editing(text=orig)]
[Editing(text=t)]
  (typing)              -> [Editing(text=t')]             / recompute invalid, canOK
  (clear button, only when !invalid) -> [Editing(text="")] / E-31: no clear button when invalid
  (OK, canOK)           -> [Saving]                       / onConfirm(text) through §1
  (Cancel or Escape)    -> [Closed]                       / no write
[Saving]
  (veto true)           -> [Closed]                       / pref.set(text)
  (veto false)          -> [Editing(text=t)]              / dialog stays open, no message (E-03 analogue)
  (veto error)          -> [Editing(text=t)] + message    / Proposal
```

- Visual flags: `isError = invalid`; trailing icon = Error when `invalid`, else clear; supporting text only when `!validate(text) && errorMessage != null` (a blank-only error has no supporting text, E-03).
- Multi-line variant: Enter inserts newline; Ctrl+Enter = OK (E-34).
- MPV variant: `formatSubtitle = false`; the veto writes the file first, then the preference (behaviour B.14; Kurobayomi requires the write to succeed before `pref.set`, E-06).

## 7. Tri-state dialog

Source: `widget/TriStateListDialog.kt:53-144`.

Per item: `s ∈ {Unchecked, Checked, Inversed}`.

```
init: s = Checked if item in initialChecked
      else Inversed if item in initialInversed
      else Unchecked                       (E-45: checked wins)
click on item: Unchecked -> Checked -> Inversed -> Unchecked   (cycle, no other transitions)
OK:  onValueChanged(included = [items with Checked, original order],
                     excluded = [items with Inversed, original order])
     dialog stays open until the caller closes it (E-47)
Cancel / Escape: onDismissRequest(); no value emitted
```

- Accessible name per state: Kotlin uses `MR.strings.disabled` for all three (E-04). Kurobayomi: "Not included", "Included", "Excluded" (Proposal strings, T-47ba-074).
- Keyboard: Space cycles focused row (Proposal).

## 8. Slider row

Source: `presentation-core/src/main/java/tachiyomi/presentation/core/components/SettingsItems.kt:237-280`, `PreferenceItem.kt:84-102`.

```
[Showing(v)]   v is caller-owned, clamped to range (E-38)
  (drag/keys to w, w != v) -> [Showing(v)] + veto(w)      / caller re-renders with new v when it wants
  (drag to same step w == v) -> [Showing(v)]               / no veto (E-39)
```

- The slider never writes a preference itself. The item's `onValueChanged` is the only side effect.
- Pill text: `subtitle` if non-empty else the number (E-40).
- Highlight: not applied in Kotlin; Kurobayomi applies it (T-47ba-022).

## 9. Tracker row

Source: `widget/TrackingPreferenceWidget.kt`, `PreferenceItem.kt:219-228`.

```
[LoggedOut]  (initial from tracker.isLoggedIn, then flow updates)  -- no check icon
[LoggedIn]   -- green check (accessible "Logged in")
  (click in LoggedOut) -> login()             (area:27 flow; may leave the row LoggedOut on failure)
  (click in LoggedIn)  -> logout()            (Proposal: confirm first, E-42)
  (flow emits)         -> [LoggedIn] or [LoggedOut]
```

- Initial state must not flicker (E-41): read the flow's current value before the first frame.

## 10. Theme mode segmented control

Source: `widget/AppThemeModePreferenceWidget.kt:21-47`.

```
[Mode(m)]  m ∈ {System, Light, Dark}
  (segment k clicked, k == m) -> [Mode(m)]          / no write (E-50)
  (segment k clicked, k != m) -> [Mode(k)] / pref set, apply GTK colour scheme (E-51)
```

## 11. Theme card strip

Source: `widget/AppThemePreferenceWidget.kt:60-128`.

```
[Strip(selected=s)]   cards = AppTheme minus deprecated minus MONET (Linux)
  (card t clicked, t != s) -> [Strip(selected=t)] / pref set, live CSS swap (no Activity recreate)
  (card s clicked)         -> [Strip(selected=s)] / no write
```

## 12. Info row, group header, custom row

- Static: no states. Info row with blank text is skipped (E-43). Group with blank title keeps spacer and no header (E-44). Custom row: caller decides.

## 13. Update info screen

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt:13-43` and `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt`.

```
[Showing]
  (Open on GitHub) -> [Showing]                               / open releaseLink in browser (link hidden when empty, E-65)
  (Not now)        -> [Closed]                                / pop, nothing persisted
  (Download)       -> [Closed]                                / start AppUpdateDownloadJob(downloadLink, versionName), pop
                      (Proposal: first click disables the button, E-63)
```

## 14. Onboarding gate

Source: `app/src/main/java/eu/kanade/tachiyomi/ui/more/OnboardingScreen.kt:20-52`.

```
[Incomplete]   close request -> stopped (E-66)
               (Finish)      -> [Complete]  / set __APP_STATE_onboarding_complete=true; pop
               (Restore)     -> [Complete]  / same, then highlight "Backup and restore" and open Data and storage
[Complete]     close request -> allowed
```

## 15. Acceptance for this topic
- Every state above has a test in `p1-edge-cases-and-tests.md §5` (or is marked manual).
- Each transition above has exactly one side effect; no transition writes twice.
