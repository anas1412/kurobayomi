# Area 47b — Edge cases, failure modes, races, tests

Upstream tests: none. Searched `aniyomi-src` for `Checksums`, `DownloadQueueState`, `findHighlightedIndex`, `highlightKey`, `TriStateListDialog`, `MoreScreen`, `AppThemeModePreference` in any test or androidTest source set: no matches. Every test below is new and must be written for Kurobayomi. Each edge case has an ID (E-nn) used by the other files.

## 1. Edge cases by component

### 1.1 Settings page model and render (behavior §A)
- E-01 (highlight reset when not found): `PreferenceScreen.kt:37-40` resets `highlightKey` even when `indexOfFirst` returns -1. Kurobayomi: `highlight::take()` always clears. Test T-E01.
- E-02 (duplicate titles): two rows with the same title on one page. Kotlin flashes both (`StatusWrapper` compares per item). Kurobayomi flashes the first (rust §6.3). Test T-E02 asserts exactly one row gets `pref-flash`.
- E-05 (flash per item): flash is decided per row by title equality, so an empty title (TrackerPreference uses `""`) never matches a non-empty key. Test T-E05: a highlight key of `""` does nothing (must not highlight a tracker row).
- E-07 (index off by disabled group): Kotlin `findHighlightedIndex` counts rows of disabled groups that are not rendered (A.4). Kurobayomi never uses indices. Test T-E07: page with disabled first group and target in second group scrolls to the target row (verified by `compute_point` y > 0 and row is the one flashed).
- E-13 (disabled group with target): target inside a disabled group. Kotlin: index points to a wrong slot (scroll lands on a different row or past the end). Kurobayomi: the group is not built, so the lookup finds nothing, `take()` clears, no scroll, no flash. Test T-E13.
- E-14 (disabled standalone item): rendered collapsed, still registered in the lookup map. Flash on a collapsed (zero-height) row is invisible. Decision: register disabled items too (matches Kotlin lookup), and skip the flash when the row is not mapped (`row.is_mapped() == false`). Test T-E14.
- E-15 (group spacer on last group): Kotlin emits no spacer after the last group. Rust: no spacer rows at all (margins). Test T-E15: last group bottom margin equals 0 and gaps between groups equal 12 px.
- E-16 (`SliderPreference` steps): `steps` defaults to `(last - first) - 1`. For `valueRange` of one element (`first == last`) steps = -1 in Kotlin, which violates `IntRange(from = 0)`. Kurobayomi: `steps = max(0, (last - first) - 1)`. Test T-E16 with range `5..5` expects steps 0.
- E-17 (tracker title empty): tracker row has `title == ""`, its name shown by the tracker widget. Kurobayomi: tracker row key is the tracker name; highlight key for trackers must use the tracker name, not `""`. Test T-E17.
- E-18 (veto throws): a suspend veto that throws cancels its coroutine silently (A.5). Kurobayomi: caught, logged, dialog remains open with an error label. Test T-E18 with a veto that returns `Err`.

### 1.2 Switches and text rows (B.1-B.3, B.9-B.10)
- E-19 (switch rapid toggle): clicking the row twice quickly issues two veto calls. Kotlin launches two coroutines; the last write wins. Kurobayomi: serialize writes per preference with a single `glib::spawn_future_local` queue per `Pref` (drop the earlier pending write when a newer one arrives). Test T-E19: two clicks before the first veto returns → final value equals the second click, and the veto is invoked twice but only the last `set` is applied.
- E-20 (switch click while vetoed): veto returns false → switch unchanged. Test T-E20.
- E-21 (text row without onClick): not clickable, no hover. Test T-E21 checks `row.is_activatable() == false`.
- E-22 (subtitle with `%s` and blank value): subtitle template `"%s"` with empty string → the subtitle row is hidden (text is blank). Kotlin hides subtitles only when the total text is null or blank (`TextPreferenceWidget.kt:32`). Test T-E22: template `"%s"` with `""` → no subtitle label visible.
- E-23 (subtitle with missing key): `entries[value]` returns null, `format(null)` yields `"null"`. Kotlin: `subtitle?.format(e[v])` formats `null` as the string "null". Kurobayomi: format a missing label as the raw stored key instead (Proposal). Test T-E23: value `"x"` not in entries → subtitle equals `"x"`.

### 1.3 Single-choice list (B.4, B.13)
- E-24 (reselect current item): clicking the selected row does nothing and the dialog stays open (`ListPreferenceWidget.kt:95`). Test T-E24.
- E-25 (null key): `current.key!!` in `ListPreferenceWidget.kt:63` throws for a null key. Kurobayomi: entries are `Vec<(String, String)>`, no nulls. Test T-E25: entries cannot hold a null key (compile-time).
- E-26 (empty entries): dialog shows no rows, Cancel only. Test T-E26.
- E-27 (basic list writes): `BasicListPreference` does not write the value; the caller must. Kurobayomi keeps this rule so the caller can refuse. Test T-E27: after selection the veto runs, the bound value is unchanged unless the caller sets it.

### 1.4 Multi-select (B.5)
- E-28 (empty set): subtitle is "None" (`MR.strings.none`). Test T-E28.
- E-12 (set order): subtitle order follows set iteration order. Kurobayomi uses entries order. Test T-E12: stored `{"b","a"}` with entries `[a, b]` → subtitle "A, B" (labels) in entries order.
- E-29 (unknown stored key): a stored key not in `entries` is ignored in the subtitle (`mapNotNull`) and in the dialog selection, but it IS written back on OK (`selected.toMutableSet()` drops it, since `selected` only contains entries keys, so an unknown key is removed on OK). Kurobayomi: same (dropped on OK). Test T-E29: stored `{"zz","a"}`, OK with nothing changed → stored set becomes `{"a"}`. Note this is destructive; flagged for review in the tasks (T-47ba-047 acceptance).
- E-30 (dialog initial selection snapshot): `remember { ... }` in `MultiSelectListPreferenceWidget.kt:35-39` snapshots at open time. Concurrent external change does not update the open dialog. Test T-E30: external change while dialog open → dialog keeps the snapshot; OK writes the snapshot-based set.

### 1.5 Edit-text family (B.6)
- E-03 (blank without canBeBlank): error icon, no supporting text, OK disabled. Test T-E03 for each of: `""`, `"   "` (whitespace is blank).
- E-31 (clear button in error state): the clear button is replaced by the error icon when the text is invalid or blank-not-allowed (`EditTextPreferenceWidget.kt:76-82`). So a user cannot clear an invalid field with one click. Test T-E31: invalid field shows error icon, no clear button.
- E-32 (unchanged value): OK disabled when `text == value` (`EditTextPreferenceWidget.kt:101`). Test T-E32: open, no change → OK disabled.
- E-33 (validate true + empty + canBeBlank false): OK disabled (`isNotBlank() || canBeBlank` false). Test T-E33.
- E-34 (multi-line Enter): Enter in multi-line inserts a newline. Kurobayomi: Ctrl+Enter submits. Test T-E34.
- E-35 (original value snapshot): the OK condition compares the typed text with the value captured when the dialog opened (`EditTextPreferenceWidget.kt:58-60`, `value` captured by the composable). Kurobayomi keeps that snapshot in the dialog closure. Test T-E35: a stored value that changes while the dialog is open does not change the comparison base.
- E-06 (MPV write without permission): file not written but preference set (B.14). Kurobayomi: the write is always attempted; a failed write is reported (dialog error label) and the preference is NOT updated. Test T-E06 with an unwritable directory: preference unchanged, dialog open, error label visible.
- E-36 (MPV subtitle): first two lines of value plus `"\n..."` when more than 2 lines (`Preference.kt:176-181`). Value with exactly 2 lines → no ellipsis. Value with a trailing newline counts as a 3rd empty line → ellipsis appears. Test T-E36 for the three cases.
- E-37 (EditTextInfo always canBeBlank=true): the Kotlin info variant hard-codes `canBeBlank = true` (`PreferenceItem.kt:213`), regardless of the item. Kurobayomi: honour the item field (no hidden override). Test T-E37.

### 1.6 Slider (B.7)
- E-08 (no flash): slider row is not highlightable in Kotlin. Kurobayomi: highlight enabled for sliders too (Proposal) — test T-E08 expects the row to flash when it is a target.
- E-38 (value clamped by Slider): a value outside `valueRange` is clamped by Compose's Slider; the stored value is not clamped. Kurobayomi: clamp before display and before write. Test T-E38: value 12 in range 0..10 → displayed 10, write attempt is 10.
- E-39 (no-op drag): `if (it == value) return` (`SettingsItems.kt:274`) suppresses equal values. Test T-E39: dragging to the same step triggers no veto.
- E-40 (pill text fallback): `valueText = item.subtitle.takeUnless { isNullOrEmpty() } ?: value.toString()`. Test T-E40: subtitle `""` shows the number.

### 1.7 Tracker row (B.8)
- E-41 (login state flicker): `isLoggedIn` initial value comes from `tracker.isLoggedIn` then updates from the flow. Kurobayomi: read the flow's current value synchronously at build; Test T-E41: no transient wrong check mark after build.
- E-42 (logout without confirm): click logs out immediately. Proposal (ui-parity §2.7) adds a confirmation. Test T-E42 (proposal) expects a confirmation dialog before logout.

### 1.8 Info, header, group (B.9, B.10)
- E-43 (info text empty): `InfoPreference` with empty title renders an empty block with icon. Kurobayomi: skip the row when the text is blank. Test T-E43.
- E-44 (group title empty): `PreferenceGroupHeader` renders an empty label with margins (Kotlin has no null guard; `title: String` non-null). Kurobayomi: skip the header when the title is blank, keep the 12 px spacer. Test T-E44.

### 1.9 Tri-state (B.11)
- E-04 (accessible name): all three states labelled "Disabled". Kurobayomi: distinct labels. Test T-E04: accessible names for the three states differ.
- E-45 (both lists contain item): checked wins over inversed. Test T-E45: item in both lists → CHECKED.
- E-46 (membership by equals): duplicate items in `items` list: both show the same initial state; cycling one affects only its index. Test T-E46.
- E-47 (confirm does not close): OK calls `onValueChanged` and the dialog stays open unless the caller closes it. Kurobayomi: the caller closes. Test T-E47: after OK, dialog state is as the caller leaves it.

### 1.10 Theme pickers (T.1, T.2)
- E-48 (deprecated theme stored): stored `DARK_BLUE`, `HOT_PINK` or `BLUE` is not in the strip. The current selection is invisible. Kurobayomi: map to `DEFAULT` on read and write it back. Test T-E48.
- E-49 (MONET stored on Linux): stored `MONET` (from another device) is hidden in the strip. Kurobayomi: map to `DEFAULT` on read. Test T-E49.
- E-50 (theme mode re-click): clicking the selected segment writes the same value. Test T-E50: write count is 1 and no change event fires when the value is equal (Kurobayomi suppresses equal writes).
- E-51 (system theme): ThemeMode SYSTEM passes `None` to `set_gtk_application_prefer_dark_theme`. Test T-E51 asserts the call argument, through a small wrapper function that returns the argument.
- E-52 (amoled outside dark): amoled flag applies only when dark is in effect. Test T-E52: light theme with amoled true → background is the light background.

### 1.11 More tab (M.1-M.5)
- E-71 (storage param dead): `onClickStorage` is never rendered. Kurobayomi drops it. Test T-E71: no row routes to a Storage tab (test on built page rows).
- E-53 (queue state `Paused(0)`): reachable only in races; subtitle "Paused". Test T-E53: `subtitle(Paused{0}) == "Paused"`.
- E-54 (queue state transitions): derive() truth table. Test T-E54 table:

| manga running | manga len | anime running | anime len | expected |
|---|---|---|---|---|
| false | 0 | false | 0 | Stopped |
| true | 0 | false | 0 | Stopped |
| false | 3 | false | 0 | Paused{3} |
| true | 3 | false | 0 | Downloading{3} |
| false | 0 | true | 2 | Downloading{2} |
| false | 1 | true | 2 | Downloading{3} |
| true | 0 | false | 1 | Downloading{1} |
| false | 1 | false | 1 | Paused{2} |

  Row 2 is Stopped even though a downloader reports running, because the queue is empty (Kotlin checks `!pendingDownloadExists` first, `MoreTab.kt:123-127`). Test asserts exactly this table.
- E-55 (subtitle pluralisation): "1 remaining" and "3 remaining" have the same form in English. Test T-E55 for 0, 1, 2 and 1000.
- E-56 (`•` separator): exact U+2022 with single spaces. Test T-E56 compares bytes.
- E-57 (download queue race): the Kotlin nested `collectLatest` restarts on each manga change. Kurobayomi's single `select!` must produce the same final state. Test T-E57: a sequence of 20 random updates to four watch channels; the final published state equals `derive()` of the final inputs.
- E-58 (switch bindings reflect external changes): if another screen changes downloaded-only, the More switch must update. Test T-E58: set via Pref, watch receiver fires, switch active matches.
- E-59 (switch feedback loop): setting `set_active` from the watch callback must not re-trigger the write. Test T-E59: count of writes stays at 1.
- E-60 (Help URL): exact `https://aniyomi.org/docs/guides/troubleshooting/`. Test T-E60 asserts the constant string.

### 1.12 Update dialog (U.1-U.4)
- E-10 (changelog cut): `---` before `Checksums` truncates from the first `---` that has `Checksums` after it. A horizontal rule above real notes and `Checksums` at the end deletes the notes after the rule. Test T-E10 (see vectors in §2).
- E-09 (button label fixed): "Open on GitHub" even for non-GitHub links. Kurobayomi: label "Open release page" (Proposal, adds a string to area:51). Test T-E09 asserts the label comes from the string table.
- E-61 (version subtitle hard-coded in mockup): `v0.1.0`. Kurobayomi: from the update check. Test T-E61.
- E-62 (accept while offline): job has `NetworkType.CONNECTED` constraint; the job waits. Kurobayomi: the download fails fast without network and reports a notification with "Could not download the update". Test T-E62: no network → notification text.
- E-63 (double click on Download): two jobs started. Kotlin: the screen pops after the first click, so the second click can't land. Kurobayomi: disable the button on click. Test T-E63: second click is ignored.
- E-64 (empty changelog): body empty; buttons still work. Test T-E64.
- E-65 (release link missing): `openInBrowser("")` opens nothing. Kurobayomi: hide the link button when the URL is empty. Test T-E65.

### 1.13 Onboarding (O)
- E-66 (back while incomplete): swallowed. Kurobayomi: close request stopped. Test T-E66.
- E-67 (finish sets flag before pop): if the pop fails, the flag is set and onboarding will not show again. Accepted (Kotlin order). Test T-E67: flag is true after finish even if the page is not on the stack.
- E-68 (restore highlight target): restore row title is `label_backup` = "Backup and restore". If area:47 renames it, the highlight silently does nothing. Test T-E68: the highlight key constant equals the Data page row title (uses the same string constant, not a copy).
- E-69 (onboarding gate race): MainActivity pushes onboarding when the flag is false and the last item is not onboarding (area:36). Test covered there.

### 1.14 Logo and layout (L)
- E-70 (no logo at narrow width): none (logo is fixed 64 px). Test T-E70: logo does not shrink below 64 px.

## 2. Changelog strip vectors (E-10)

| Input | Output |
|---|---|
| `"## Yay\nFoo\n---\nChecksums:\nabc"` | `"## Yay\nFoo\n"` |
| `"## Yay\n---\nNotes\n---\nChecksums\nx"` | `"## Yay\n"` (first `---` has Checksums later, so cut from there; Notes are removed too) |
| `"## Yay\nNo checksums here"` | unchanged |
| `"Checksums first\n---\nthing"` | unchanged (no `---` followed by Checksums) |
| `"---\r\nChecksums\r\nx"` | `""` |
| `"Plain---text Checksums"` | `"Plain"` (any `---` followed by Checksums; mid-word is also matched) |
| `""` | `""` |

Note: the Kotlin regex with `(\R|.)*` on a string with no `\n` at all behaves as the Rust version. Vector 2 documents the behaviour, not a desired result. Proposal: keep the Kotlin behaviour for parity; flag the rule in the tasks as "parity, review".

## 3. Race conditions
- R-01: rapid switch toggles (E-19). Serialize per Pref.
- R-02: download queue inputs changing during combine (E-57). Single select over watch channels.
- R-03: highlight request set by onboarding while the destination page is not yet built: `take()` happens at page build; the request must be set before pushing the page (write order in §13 of rust design). Test T-R03: request set, page built, flash happens once.
- R-04: highlight request left set after navigation away: the next settings page must not consume a stale request. Kotlin resets only when PreferenceScreen composes; a stale value survives if the page is never composed. Kurobayomi: `take()` also runs on navigation to any page (clears it). Test T-R04: request set, navigate to another page, request is cleared.
- R-05: dialog open while the underlying page is rebuilt (theme change, preference change from elsewhere): the dialog keeps its snapshot (E-30). Test T-R05.
- R-06: Escape pressed while a veto is in flight. Decision: an in-flight veto is allowed to finish and its result is applied (the user already confirmed); if the veto never started, nothing is written. Test T-R06: Escape after OK click with veto in flight → value written if the veto returns true; Escape before OK → no veto call and no write.
- R-07: theme change applied while a theme card is being drawn: CSS swap is atomic on the GTK thread. No test.
- R-08: flash timers on a destroyed row: `glib` callbacks hold a strong ref to the row. Test T-R08: destroying the page while a flash is running does not panic (run the main loop for 3 s in a test harness, or assert the removed SourceIds).

## 4. Failure modes
- F-01 Preference store write fails (disk full): `Pref::set` returns error through area:02. This area shows an error in the dialog (E-18) and keeps the old value shown.
- F-02 Icon name missing in the theme: broken image. Release-gate check T-47ba-049.
- F-03 Download job cannot start (no tokio runtime): log and show a notification (area:35). Test T-F03.
- F-04 `UriLauncher` fails (no handler): show a toast-equivalent `gtk::AlertDialog` with the URL and a Copy button (Proposal). Test T-F04: launch error path shows the dialog.
- F-05 MPV directory missing: created on first write (`create_dir_all`); permission error → E-06.

## 5. Test catalogue (to write)

Unit tests (pure Rust, no GTK, run with `cargo test`):

| Test ID | Subject | Assertion |
|---|---|---|
| T-E01 | `highlight::take` clears | after `take()` second call returns None even when no row matched |
| T-E07 | lookup order with disabled group | target after a disabled group is found by title, not by index |
| T-E10 | `strip_checksums` vectors | all 7 vectors of §2 |
| T-E12 | multi-select subtitle order | labels in entries order |
| T-E16 | slider steps | `(5..5)` → steps 0, `(0..10)` → 9, `(0..1)` → 0 |
| T-E22 | subtitle blank hidden | `"%s"` + `""` → None |
| T-E23 | subtitle missing key | `"x"` shown raw |
| T-E28 | empty set subtitle | "None" |
| T-E29 | unknown key dropped on OK | stored `{zz, a}` → `{a}` |
| T-E36 | MPV subtitle | 2 lines, 3 lines, trailing newline cases |
| T-E38 | slider clamp | 12 in 0..10 → 10 |
| T-E40 | pill fallback | `""` subtitle → number |
| T-E45 | tri-state precedence | item in both lists → CHECKED |
| T-E46 | tri-state duplicates | duplicate items cycle independently |
| T-E48 | deprecated theme read | DARK_BLUE/HOT_PINK/BLUE → DEFAULT |
| T-E49 | MONET on Linux | MONET → DEFAULT |
| T-E50 | theme mode equal write | no write when equal |
| T-E53 | Paused(0) subtitle | "Paused" |
| T-E54 | derive() truth table | 7 rows of §1.11 |
| T-E55 | "N remaining" | 0, 1, 2, 1000 |
| T-E56 | bullet bytes | `"Paused • 3 remaining"` bytes match U+2022 |
| T-E57 | queue select convergence | 20 random updates → final state = derive(final inputs) |
| T-E60 | Help URL constant | exact string |
| T-E61 | version subtitle | from argument, not literal |
| T-E63 | double click guard | second click ignored |
| T-E64 | empty changelog | returns empty |
| T-E65 | empty release link | link hidden |
| T-E68 | highlight key equals Data row title | same constant |
| T-E51 | system theme argument | `None` for SYSTEM |

GTK integration tests (run under `xvfb-run` or a Wayland headless compositor, using `gtk::init()` in a test binary; mark `#[ignore]` unless the env var `KUROBAYOMI_GTK_TESTS=1` is set):

| Test ID | Assertion |
|---|---|
| T-E02 | duplicate titles: only one row gets `pref-flash` |
| T-E05 | empty highlight key does not flash the tracker row |
| T-E13 | target in disabled group: no scroll, no flash, request cleared |
| T-E14 | disabled standalone item: not flashed when unmapped |
| T-E15 | spacing: 12 px between groups, none after last group |
| T-E17 | tracker highlight by tracker name |
| T-E18 | veto error keeps dialog open and shows label |
| T-E19 | rapid toggles: last write wins |
| T-E20 | vetoed switch unchanged |
| T-E21 | text row without onClick not activatable |
| T-E24 | list re-select keeps dialog open |
| T-E26 | empty list dialog shows Cancel only |
| T-E27 | basic list does not write |
| T-E30 | dialog snapshot is stable under external change |
| T-E31 | invalid field shows error icon, no clear button |
| T-E32 | OK disabled when unchanged |
| T-E33 | OK disabled for blank non-canBeBlank |
| T-E34 | multi-line Enter inserts newline; Ctrl+Enter submits |
| T-E37 | info variant honours canBeBlank |
| T-E39 | equal slider value suppresses veto |
| T-E41 | tracker initial state correct without flicker |
| T-E43 | blank info row skipped |
| T-E44 | blank group title: no header, spacer kept |
| T-E47 | tri-state OK does not auto-close |
| T-E52 | amoled ignored in light |
| T-E58 | switch reflects external change |
| T-E59 | no feedback write |
| T-E62 | offline update reports failure |
| T-E66 | close request stopped while onboarding incomplete |
| T-E67 | flag set on finish |
| T-E70 | logo 64 px |
| T-R03 | request before push flashes once |
| T-R04 | stale request cleared on navigate |
| T-R06 | Escape during veto: no write unless veto started |
| T-R08 | destroy during flash: no panic |

Manual/visual checks (screenshots via `KUROBAYOMI_SHOT`):
- More page at 1180x780 and 720x900: all 14 rows, logo spacing.
- Settings row states: normal, pressed, disabled (insensitive), error (edit dialog), highlight mid-pulse (capture at 900 ms).
- Theme strip with 18 cards; selected border; scroll.
- Tri-state dialog with 3 states present.
- Update screen with a changelog containing a fenced code block and a link.

## 6. Fixtures
- `fixtures/settings/more_page.json`: the 14 rows of §M.2 with the English strings.
- `fixtures/settings/disabled_group.json`: two groups, first disabled, target in second.
- `fixtures/settings/tri_state.json`: 5 items, two in each list plus one overlap.
- `fixtures/update/changelog_*.md`: the seven vectors of §2 as files.
- `fixtures/queue/derive_table.csv`: the table in T-E54.
- `fixtures/theme/stored_names.txt`: each `AppTheme` name plus `DARK_BLUE`, `HOT_PINK`, `BLUE`, `MONET`.

Placement: `tests/` for unit tests in the crate (`tests/settings_model.rs`, `tests/queue_state.rs`, `tests/changelog.rs`); GTK tests in `tests/gtk_settings.rs` with `#[ignore]` guard.

## 7. Coverage map (behavior section to test)
- §A: T-E01, T-E02, T-E05, T-E07, T-E13, T-E14, T-E15, T-E16, T-E18, T-R03, T-R04
- §B.1-B.3: T-E19, T-E20, T-E21, T-E22
- §B.4: T-E24, T-E25, T-E26, T-E27
- §B.5: T-E12, T-E28, T-E29, T-E30
- §B.6: T-E03, T-E06, T-E31-T-E37
- §B.7: T-E08, T-E38-T-E40
- §B.8: T-E41, T-E42
- §B.9-B.10: T-E43, T-E44
- §B.11: T-E04, T-E45-T-E47
- §B.14: T-E06
- §C: T-E01, T-E02, T-E13, T-E14, T-R08
- §T: T-E48-T-E52
- §M: T-E71, T-E53-T-E60
- §U: T-E09, T-E10, T-E61-T-E65
- §O: T-E66-T-E68
- §L: T-E70

## 8. Known gaps (explicit)
- No upstream test exists for the update-check/changelog flow outside the strip regex; the regex vectors in §2 are the only oracle, derived by reading the regex semantics. Confidence: high for the vectors that involve one `---` and one `Checksums`; medium for multi-match inputs because Java regex engines are not run here.
- GTK behaviour (focus order, Revealer timing) is verified by the manual checks in §5, not by automated tests, because there is no headless GTK harness in the repo yet (T-47ba-071 adds one).
