# P1 edge cases, failure modes, races, upstream tests, new tests: 48-settings-player-and-scripts

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Each edge case has an ID. "Kotlin" = observed behaviour; "Port" = required behaviour for Kurobayomi.

## 0. Upstream tests

- No `*Test.kt` exists for any file in this area's scope (checked: no test under the custom button, player settings, editor, highlight, or torrent settings packages).
- Nearby upstream test (not in scope, owned by area:13): `core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt` (68 lines) tests `TorrentHelpers.parseTorrentDetailsFromTorrentFileContent` against two `.torrent` fixtures. Port it to area:13's Rust module. Its expected values are recorded there.

---

## 1. Edge cases

### 1.1 Player settings (E-PS)

- **E-PS-01 Remember volume is a dead switch.** Kotlin: `pref_remember_volume` has no reader. Port: do not show the row (T-48a-052). If the product wants it, implement: store the last volume (`player_volume_value`, Float, default -1) on close and apply on open when the switch is on.
- **E-PS-02 Remember brightness** (`pref_remember_brightness`) is read by `PlayerActivity` and uses `player_brightness_value` (default -1). Value -1 means "unset": do not apply. Port: brightness is an mpv video filter (see design section 8); apply only when set.
- **E-PS-03 Intro-skip children disabled while parent off.** Kotlin hides children. Port: greyed (insensitive). Test: switch off → children insensitive; state of children persists when parent re-enabled.
- **E-PS-04 Fullscreen gate.** Kotlin enables `player_fullscreen` only on API 28+. Port: always enabled (desktop). Test that the value is still read and applied.
- **E-PS-05 PiP gate.** Kotlin shows the PiP group only if `deviceHasPip()`. Port: show only when the desktop probe reports support. If not supported, do not show the group, and do not change `pref_enable_pip` stored value (it may be read by other code).
- **E-PS-06 Progress value not in list.** Stored float 0.92 is not one of the 7 entries → Kotlin shows subtitle "null". Port: show the stored value formatted as a percentage with the label "(custom)" and keep it selectable; do not overwrite the value.
- **E-PS-07 Time to disappear not in list.** Stored 4200 (from a future version) → Kotlin "null". Port: same as E-PS-06.
- **E-PS-08 Skip length 0 means disabled.** Port: double-tap seek disabled when `pref_skip_length_preference == 0`; the dropdown shows "Disable".
- **E-PS-09 Orientation key stored from Android.** Values like `SensorLandscape` remain stored in exported data. Port: ignore on desktop; do not delete.
- **E-PS-10 External player list empty.** Kotlin: entries are only "None" when no supported package is installed. Port: list = "None" plus MIME handlers for `video/*` (gio); if none, show "None" and a hint "No video player found" (desktop addition).
- **E-PS-11 External player uninstalled.** Stored package name no longer installed. Kotlin: list has no entry for it → subtitle "null". Port: show "(unavailable)" and fall back to the system chooser at launch (`external_player_preference` treated as "").
- **E-PS-12 Always external + no player selected.** Kotlin: `alwaysUseExternalPlayer` true with `""` → plain ACTION_VIEW chooser. Port: `gio::AppInfo` default for `video/*`; if none, show the chooser (`gtk::AppChooserDialog`).

### 1.2 Gestures (E-GS)

- **E-GS-01 Intro length 0.** Saving 0 disables the intro button (`update_button` hides it). Port: same; the subtitle shows "0s".
- **E-GS-02 Intro length dialog without scrolling.** Kotlin saves the start value (WheelPicker initial emission). Port: explicit initial value; OK without change keeps the value (test).
- **E-GS-03 Double-tap action not allowed for side.** A stored `Switch` for center (not in its allowed list) → Kotlin shows "null" label. Port: show the stored action's label and keep it; the dropdown lists only allowed actions plus the stored one, marked "(not available here)". Behaviour still runs the action.
- **E-GS-04 Custom with no input.conf binding.** Custom sends a key name that is unbound → nothing happens. Port: same; the info text explains the key name. Test: dispatch emits the key with no crash.
- **E-GS-05 Tap zones.** Which horizontal zone (left, centre, right) a double tap falls into is decided by the gesture handler, which is owned by area:46 and not in this area's scope. Port: use area:46's thresholds once that area is final; this area only consumes the zone name.
- **E-GS-06 Double-click vs drag.** A double click that starts a drag must not seek. Port: use `GestureClick` with `n_press == 2` and cancel on movement beyond 8 px (GTK drag threshold).

### 1.3 Decoder (E-DC)

- **E-DC-01 Debanding overwrite (Kotlin bug).** With CPU debanding and YUV420P on, Kotlin keeps only `format=yuv420p`. Port: chain both (`build_vf_chain`). Test: `build_vf_chain(Cpu, true) == "gradfun=radius=12,format=yuv420p"`.
- **E-DC-02 Debanding GPU** sets `deband=yes` and does not touch `vf`. Test: `build_vf_chain(Gpu, false) == None` plus option set.
- **E-DC-03 gpu-next unsupported.** If mpv rejects `gpu-next` (driver), Kotlin does not fall back. Port: on mpv error for `vo`, retry with `gpu` and show a toast "gpu-next unavailable, using gpu".
- **E-DC-04 Settings change during playback** do not apply until next start (Kotlin behaviour). Port: show the note (ui-parity 2.4); test that a change does not alter an open player.

### 1.4 Language lists (E-LG)

- **E-LG-01 Empty string valid.** `validate_list("") == Ok`.
- **E-LG-02 Trailing comma valid.** `"en,"` → `Ok` (empty elements dropped before trim).
- **E-LG-03 Space after comma invalid.** `"en, "` → `Err(InvalidCode(""))` (Kotlin: `Invalid code: ""`). Test that this matches; desktop may choose to trim before filtering instead (decision: keep Kotlin order, document).
- **E-LG-04 Uppercase accepted.** `"EN"` valid.
- **E-LG-05 Unknown two-letter code** `"xx"` invalid.
- **E-LG-06 Three-letter code** `"eng"`, `"jpn"`: accept (design decision, see behaviour section 6). Test both.
- **E-LG-07 Message uses the bad element.** `"en,xx,yy"` → error names `xx` (first invalid).
- **E-LG-08 Preferences with invalid stored value** (imported or migrated): page shows the value; OK requires a valid list to save again; the value is not auto-cleaned. (Kotlin migration 130 cleans on upgrade; desktop has no migration.)

### 1.5 Audio (E-AU)

- **E-AU-01 Auto/AutoSafe crossing.** Port maps "Auto" → mpv `auto`, "Auto-safe" → `auto-safe` (design section 3). Test: `AudioChannels::Auto.mpv_setting() == ("audio-channels","auto")`. Document the change.
- **E-AU-02 Boost cap 0** → `volume-max = 100` (no boost). Test.
- **E-AU-03 Boost cap 200** → `volume-max = 300`. Test.
- **E-AU-04 Reverse stereo.** Sets `af` (not `audio-channels`). Test that switching away from reverse stereo clears `af` (Kotlin does not clear; a prior `af` from another feature may be overwritten). Port: set `af` to `""` when leaving ReverseStereo only if this module set it.
- **E-AU-05 Audio delay** is not on this page (area:46) but read here for `audio-delay` at start. Test the conversion `ms/1000.0`.

### 1.6 Advanced and mpv files (E-AV)

- **E-AV-01 mpv.conf mirror not read.** Test: editing the external `mpv-config/mpv.conf` file does not change the player's config (documented; Port does not create the mirror).
- **E-AV-02 Invalid mpv.conf syntax.** mpv reports an error at start; the player still starts. Port: show the mpv log line in a toast "mpv.conf: <line>" (desktop addition) — or log only (decision: log only; toast is optional).
- **E-AV-03 User files switch off** → scripts folder content not loaded even if present. Test: with `mpv_scripts = false` the runtime `scripts/` contains only `aniyomi.lua` and `custombuttons.lua`.
- **E-AV-04 Files with spaces or Unicode names.** `my script.lua`, `脚本.lua`. Port: pass exact paths to mpv (no shell). Test load.
- **E-AV-05 Symlinked user scripts.** Port: follow symlinks when listing; do not follow directory cycles.

### 1.7 Runtime sync (E-SY)

- **E-SY-01 Deleted user script still loaded.** Kotlin deletes the internal copy each start, so a deleted user script is not loaded next time. Port: runtime dir regenerated at start; test deletion takes effect after restart.
- **E-SY-02 Font copy race.** Kotlin copies fonts asynchronously. Port: copy before `initialize` (synchronous) or set `sub-fonts-dir` to the user font directory directly (no copy). Test: first frame has fonts (manual test; automated check that the property is set before playback).
- **E-SY-03 Copy without close.** Kotlin does not close streams in `copyUserFiles`. Port: use `std::fs::copy`. Test: copy of a 10 MB file completes and the file handle is closed (check with `lsof` in manual test).
- **E-SY-04 Asset skip by size.** Kotlin skips `subfont.ttf` when sizes match. Port: compare size and SHA-256 of the bundled file (sizes may match with different content). Test: changed asset with same size is replaced.
- **E-SY-05 Base directory unavailable.** `getMPVConfigDirectory()` returns null (missing drive/SD card). Kotlin: silently empty lists or no user files. Port: show a banner on the script editor "Base folder not available: <path>"; player starts without user files.
- **E-SY-06 Runtime dir not writable.** Port: fall back to `$XDG_CACHE_HOME/kurobayomi/mpv` and log.

### 1.8 Script editor file list (E-ED)

- **E-ED-01 Unsorted list.** Port sorts case-insensitively. Test: order `a.lua, B.lua, c.lua`.
- **E-ED-02 Duplicate by case.** `Test.lua` exists; creating `test.lua` → "File already exists". Test.
- **E-ED-03 Case-only rename.** `test.lua` → `Test.lua`: Kotlin removes the entry equal to `initialName` before the case-insensitive duplicate check, so the rename is **allowed** and succeeds (observed by reading `PlayerSettingsEditorScreenModel.kt:90-106`). Port: keep this behaviour (exclude the file being renamed by its exact old name). Test that the case-only rename succeeds on a case-sensitive filesystem.
- **E-ED-04 Create when file exists on disk but not in list** (hidden file or race). Kotlin: `createFile` returns existing file, success path, reload. Port: `create_new` fails → error toast "Unable to create file".
- **E-ED-05 Name with leading dot.** `.hidden.lua` → invalid (buildValidFilename trims dots). Test.
- **E-ED-06 Name with trailing space.** `a.lua ` → invalid. Test.
- **E-ED-07 Name > 240 characters.** Kotlin truncates in `buildValidFilename` and compares → `name != truncated` → invalid. Test: 241-character name is invalid.
- **E-ED-08 Rename to another extension.** Allowed in Kotlin (no check). Port: keep the stored extension (decision in design section 10); test that `a.lua` → `a.txt` is rejected or extension is kept.
- **E-ED-09 Delete the file open in the editor.** Port: the editor holds the buffer; saving then recreates the file. Show a warning on save when the file no longer exists ("The file was deleted. Save as a new file?"). Desktop addition; test.
- **E-ED-10 Directory entries** are skipped. Test: a subdirectory named `sub.lua` is not listed.
- **E-ED-11 Size label** uses decimal units: 1000 B → "1.00 KB". Test: 999 → "999 B"; 1000 → "1.00 KB"; 1_500_000 → "1.50 MB".
- **E-ED-12 Modified time** unavailable (-1) → no date part. Test.

### 1.9 Code editor (E-CE)

- **E-CE-01 Empty file.** Opens with empty buffer; gutter shows "1". Test.
- **E-CE-02 Very long single line** (no newline, 1 MB): no wrap, horizontal scroll; highlight skipped above 15000 characters. Test that the UI remains responsive (manual) and highlight is skipped.
- **E-CE-03 Non-UTF-8 file.** Kotlin reads lossy and writes U+FFFD back on save (silent change). Port: detect invalid UTF-8 on load; show a banner "This file is not UTF-8. Saving will convert it." and require confirmation on the first save. Test: bytes `0xFF` → banner.
- **E-CE-04 CRLF line endings.** Kotlin keeps `\r` in the buffer (displayed as a control char). Port: keep bytes as-is (do not normalise); test round-trip `\r\n` unchanged.
- **E-CE-05 Save failure** (read-only file): toast with error; buffer stays modified; Save stays enabled. Test.
- **E-CE-06 Save during save** (double Ctrl+S): second save is ignored while the first is in progress. Test.
- **E-CE-07 Close during save** — the window waits for save to finish; if it fails, the unsaved dialog appears. Test.
- **E-CE-08 Back with no changes** pops immediately. Test.
- **E-CE-09 External change while open.** Kotlin: no detection; save overwrites. Port: `gio::FileMonitor` detects change; on save, if the mtime differs from load, show "The file changed on disk. Overwrite?" (desktop addition). Test.
- **E-CE-10 Undo after save** keeps the buffer's undo stack. Test manually.
- **E-CE-11 Large file over 15000 chars** shows the info bar (desktop addition).

### 1.10 Highlighter (E-HL)

- **E-HL-01 Lua multi-line comment with `=` levels.** `--[==[ text ]==]` matched by rule 10 (group 1 `==`). Test the span covers all of it and `]]` inside does not close it.
- **E-HL-02 Comment marker inside a string.** `"--x"` → string colour, not comment (rule 9 is skipped inside strings via start index). Test.
- **E-HL-03 Keyword inside a string.** `"end"` → string colour (keyword span is overwritten by the later string span). Test by rendering the span list: the last span covering the index is the string colour.
- **E-HL-04 Escaped quote in string.** `"a\"b"` → one string span. Test.
- **E-HL-05 Conf value with spaces.** `key = some value here` → value colour covers `some value here` (rule 2 class `[\w ~,./;:-]`). Test.
- **E-HL-06 Conf comment after value.** `a=1 # c` → `# c` coloured comment; `1` coloured as number (rule 4) and literal (rule 2) - later rules win (comment last). Test.
- **E-HL-07 Numbers inside identifiers.** `x1 = 2` → `1` not coloured (`(?<!\w)` in Lua rule 7). Test with ASCII `\w` semantics; test non-ASCII identifier `ñ1` (ASCII `\w` does not match `ñ`, so `1` IS coloured under Java semantics). Document the result the port must produce (Java behaviour).
- **E-HL-08 Catastrophic backtracking.** Lua rule 8 with `(?<!--[^\n]{0,120})` on a 15000-char line of `-`. Test completes under 100 ms.
- **E-HL-09 Empty text.** Returns empty span list. Test.
- **E-HL-10 Full-width digits** are not numbers (ASCII rules). Test.

### 1.11 Custom buttons (E-CB)

- **E-CB-01 One bad Lua body disables all buttons.** Kotlin: `custombuttons.lua` fails to load; no buttons work and no error is shown in the UI. Port: per-button `pcall` (design section 4.4); test: one invalid button leaves the others working; error logged with the button name.
- **E-CB-02 Empty long press.** `longPressContent = ""` → empty function; long press does nothing. Test.
- **E-CB-03 Button ID 0.** Not possible (ids start at 1); `primaryId = 0` means no favourite. Test that `$isPrimary` is `false` for all when no favourite.
- **E-CB-04 Concurrent create.** Two creates race: Kotlin reads max then inserts without lock → possible duplicate sortIndex. Port: serialised worker reads and inserts in one job. Test: 100 concurrent `Create` commands yield sort indices 0..99 with no duplicates.
- **E-CB-05 Edit after reorder writes stale sortIndex.** Kotlin: edit dialog snapshot sends old `sortIndex`. Port: edit never writes sortIndex. Test: open edit, reorder, save edit → order preserved.
- **E-CB-06 Stale favourite snapshot.** Kotlin: `ToggleFavorite` uses the UI snapshot's `isFavorite`; a button favourited elsewhere in the meantime can be wrongly toggled. Port: read fresh row in the worker (plan_favorite takes fresh state). Test: two favourite toggles from stale UI leave exactly one favourite.
- **E-CB-07 Dialog closes on list update.** Kotlin: any DB emission resets `dialog` to null (the `Success` is rebuilt). Port: keep dialog state separate from data. Test: a DB change while Create dialog open keeps the dialog.
- **E-CB-08 Bad integer in bridge.** `seek_by` with `"abc"` throws in Kotlin (crash of the observer). Port: log and ignore. Test.
- **E-CB-09 Restore with two favourites.** Kotlin inserts two favourites. Port: at most one favourite after restore (first in backup sort order wins; others become non-favourite). Test.
- **E-CB-10 Duplicate names.** No DB constraint; dialog blocks duplicates; restore dedups by name. Port: same; a name that already exists in the DB is rejected by the dialog. Test: duplicate name in DB (inserted directly) does not crash the list or the restore.
- **E-CB-11 Delete favourite.** No favourite remains; the bottom-right control shows nothing. Port: same. Test.
- **E-CB-12 Reorder to the same position.** `newIndex == currentIndex`: Kotlin writes all rows again (no-op). Port: return `Unchanged` without writing. Test.
- **E-CB-13 Reorder index out of range.** Kotlin: `IndexOutOfBounds` → InternalError toast. Port: `PlanError::IndexOutOfRange` → toast "InternalError…". Test.
- **E-CB-14 Empty list** and delete of the last button: renumbering of zero rows is a no-op. Test.
- **E-CB-15 Lua with `$id` in strings.** `print("$id")` → `print("1")` (textual). Document; test.
- **E-CB-16 Very long Lua body** (100 KB). Port: write file; mpv load works. Test.
- **E-CB-17 Titles with spaces or symbols** (`+85 s`). Not used in Lua; the title is only for the button label. Test: label renders with special characters.
- **E-CB-18 Seed row deleted by user.** Restore of seed on reinstall: `INSERT OR IGNORE` with `_id = 1` only runs in migration. Port: seed inserted only at migration; if deleted, it stays deleted. Test.
- **E-CB-19 Primary title from Lua persisted?** `set_button_title` is in-memory only (per player session). Port: same. Test that a new session resets title to favourite name.

### 1.12 Lua bridge (E-LB)

- **E-LB-01 Property cleared after handling.** Kotlin sets property to `""` after each call, even ignored ones. Port: same. Test: repeated identical `seek_by "5"` calls both seek.
- **E-LB-02 Empty value.** `""` → ignored (no clear loop). Test.
- **E-LB-03 Quoted value.** Value `"\"5\""` (with quotes) → data `5`. Test.
- **E-LB-04 Pipes in text.** `seek_by_with_text` with `"5|a|b"` → seek 5, text `a|b`. Test.
- **E-LB-05 int_picker with pipe in title** → arity error, logged, no dialog. Test.
- **E-LB-06 Unknown property** under user-data/aniyomi → ignored, property cleared. Test.
- **E-LB-07 Play/pause during seek** ordering preserved in one queue. Test order.
- **E-LB-08 software_keyboard on desktop** → no-op with debug log. Test.
- **E-LB-09 toggle_ui hide while sheet open** closes sheets and panels and dialogs (Kotlin). Test state transitions.

### 1.13 TorrServer settings (E-TS)

- **E-TS-01 First enable shows notice once.** Enable → notice; disable → enable → no notice. Test (shownNotice persists).
- **E-TS-02 Port 0** valid (system chooses). Test. Port `65536` invalid; `-1` invalid; `""` invalid; `" 80"` → toIntOrNull(" 80") is null → invalid (Kotlin); port `08090` → 8090 valid. Port: parse with `u16` after trim? Decision: Kotlin does not trim; keep, but accept leading zeros. Test.
- **E-TS-03 Proxy URL cannot be cleared.** Kotlin: empty invalid. Port: empty allowed when mode is None; with a non-None mode, empty is invalid (shows "Invalid uri"). Test.
- **E-TS-04 Proxy mode None with URL set.** Kotlin: URL ignored, server gets `tracker` with empty URL. Port: no proxy arguments. Test.
- **E-TS-05 Trackers multi-line with blank lines.** Kotlin: blank lines kept. Port: keep (server ignores blanks). Test.
- **E-TS-06 Trackers change while running** → service stops (Kotlin); Port: restart the process automatically. Test.
- **E-TS-07 Port change while running** → Kotlin: no restart. Port: restart. Test.
- **E-TS-08 Credentials in proxy URL** not logged. Test log output.
- **E-TS-09 Reset trackers** when already default → row greyed. Test.
- **E-TS-10 Server binary missing** → status "Failed to start: torrserver not found". Test.

### 1.14 UI preferences (E-UP)

- **E-UP-01 Date format invalid pattern** (`app_date_format = "yyyy-QQ-zz"`): Kotlin throws IllegalArgumentException on use. Port: validate with chrono; if invalid, fall back to the localised short date and show the settings help line "Invalid format". Test.
- **E-UP-02 Date empty** → localised short date. Test.
- **E-UP-03 Deprecated AppTheme values** (`DARK_BLUE`, `HOT_PINK`, `BLUE`) stored → Port: map to `DEFAULT`, keep stored value. Test.
- **E-UP-04 Monet on non-dynamic systems**: stored `MONET` on desktop → map to "System colours" (follow GTK theme). Test.
- **E-UP-05 Nav style "More" tab removed**: bottom bar always has five tabs. Test for each of the four styles: count = 5 and `MoreTab` present.
- **E-UP-06 Start screen Browse** when extensions missing: opens Browse (empty state). Test (owned by area:40/41 for content).
- **E-UP-07 Tablet mode stored** `ALWAYS` on desktop: ignored. Test.

---

## 2. Race conditions (summary)

| ID | Race | Mitigation |
|---|---|---|
| R-01 | Two creates read the same max sortIndex (E-CB-04) | single DB worker; read+insert in one job |
| R-02 | Reorder during drag and DB refresh (Kotlin `isAnyItemDragging`) | do not replace list model while dragging |
| R-03 | Edit dialog open during reorder (E-CB-05) | edit never writes sortIndex |
| R-04 | Favourite toggle from stale UI (E-CB-06) | fresh read in worker |
| R-05 | Player start while editing a script | scripts copied at start; edits apply next start (documented) |
| R-06 | Font copy vs first frame (E-SY-02) | set property without copy; or copy synchronously before init |
| R-07 | Two saves at once (E-CE-06) | save button disabled while saving |
| R-08 | Torrent start/stop while toggling switch | serialise through the child-process supervisor; latest request wins |
| R-09 | Lua property set from mpv thread during GTK modal dialog | queue; handled on GTK thread in order |
| R-10 | Rename and delete from two windows | operations re-read directory listing before acting; errors toast |

---

## 3. Failure modes and expected behaviour

| Failure | Expected |
|---|---|
| SQLite locked/full during create | toast "Could not save the custom button"; dialog closes but nothing is added; log error |
| DB read fails at startup for custom buttons | screen shows Error state with Retry; player starts with no buttons; log |
| Base folder removed while app runs | script editor shows banner; create/rename/delete show "Unable to …" toasts |
| Script file unreadable (permission) | code editor shows error state with message |
| mpv fails to load `custombuttons.lua` | log error with mpv message; buttons unavailable; primary button hidden |
| mpv refuses `vo=gpu-next` | fallback to `gpu` (E-DC-03) |
| torrserver child exits unexpectedly | status "Stopped (exit code N)"; no auto restart; log |
| Clipboard/URI launch fails (help link) | toast "Could not open link" |
| Invalid language list stored | page shows value, validation shown on edit (E-LG-08) |

---

## 4. Test plan (new tests to write)

Unit tests (`cargo test`, no GTK, no mpv):

1. `prefs::tests`
   - `Pref::get` returns default for missing, undecodable, and wrong-type values.
   - `NamedEnum` round-trip for every variant of `PlayerOrientation`, `SingleAction`, `Debanding`, `AudioChannels`, `ProxyMode`, `ThemeMode`; names equal the Kotlin constant names (table-driven).
   - `f32` round-trip for `0.85`, `0.70`, `1.0`.
   - Unknown enum name → default.
2. `lang::tests` — E-LG-01..07 table.
3. `player::custom_buttons::plan::tests`
   - `plan_reorder` for: move to front, move to end, same position (returns `Unchanged` at the service layer), index = len-1, out of range (E-CB-13).
   - `plan_after_delete` contiguous renumbering (E-CB-14).
   - `plan_favorite` on: turning on one (others off), turning off (only that), empty list.
4. `player::custom_buttons::model::tests` — `render` with `$id`, `$isPrimary`, no favourite (primary 0), `$id` inside strings (E-CB-15).
5. `player::custom_buttons::lua::tests`
   - Output contains exactly one `register_script_message('call_button_<id>'` per button and one `_long`.
   - Empty bodies produce valid Lua: run the output through an embedded Lua 5.1 parser (`mlua` crate, `Lua::new().load(...).into_function()`), expecting success; one bad button produces a per-button error (E-CB-01).
6. `player::lua_bridge::tests` — every row of the table in topic section 8; E-LB-01..08.
7. `player::vf::tests` — E-DC-01, E-DC-02, and the four combinations.
8. `editor::files::tests`
   - `build_valid_filename`: `.x` → invalid after trim (E-ED-05), `a/b` → `a_b`, 241 chars (E-ED-07), empty → `(invalid)`.
   - `validate_name` with case-insensitive duplicate (E-ED-02), case-only rename (E-ED-03).
   - `list`: skips directories (E-ED-10), sorts case-insensitively (E-ED-01), size labels (E-ED-11).
   - `save_text` atomicity: simulate failure after temp write; original unchanged.
9. `editor::highlight::tests` — E-HL-01..10 plus a golden test per rule (input → span list), golden files under `tests/fixtures/highlight/`.
10. `torrent::settings::tests` — E-TS-02, E-TS-03, E-TS-04; default tracker list has exactly 26 entries and no spaces.
11. `prefs::ui::tests` — E-UP-01..07.

Integration tests (headless, use a temp directory and in-memory SQLite):

12. `db::custom_buttons` — migration creates table; seed row present once after two migrations (E-CB-18); create/update/delete/reorder/favourite through the worker, including E-CB-04 (100 concurrent creates) and E-CB-06.
13. `player::mpv_dir` — runtime directory contents with `mpv_scripts` on/off (E-AV-03), deleted user script disappears (E-SY-01), changed asset with same size is replaced (E-SY-04).
14. Bridge end-to-end with a fake `MpvHandle` trait implementation: send `seek_by` twice and assert two seeks and two property clears (E-LB-01).

Manual test checklist (requires a running app and mpv):

15. Play a video: default `+85 s` button visible; tap seeks; long press opens the int picker; change to 30 s; button title updates.
16. Create button with Lua error in content; play; verify other buttons still work (after the E-CB-01 fix) and error appears in log.
17. Script editor: create, rename (case-only), delete, save with Ctrl+S, unsaved guard, large file (> 15000 chars) plain mode, non-UTF-8 file banner (E-CE-03).
18. Torrent: enable (notice once), port change restarts server, trackers change restarts, proxy mode None sends no proxy.
19. Decoder: CPU debanding + YUV420P both active (check with `mpv --show-property vf` via IPC).
20. Gestures: double-click left seeks back; drag does not seek; Custom key names trigger `input.conf` bindings.
21. Audio: "Auto" and "Auto-safe" produce the documented mpv values (IPC check).
22. Accessibility: keyboard-only pass over every page; screen-reader labels on icon buttons.

Fixtures needed:
- `tests/fixtures/custom_buttons/seed.sql` (migration 129 statements).
- `tests/fixtures/highlight/lua_sample.lua` and `conf_sample.conf` (cover all rules).
- `tests/fixtures/mpv_config/` with `scripts/`, `script-opts/`, `shaders/`, `fonts/` (one file each, one with spaces, one non-ASCII).
- `tests/fixtures/files/non_utf8.lua` (bytes `0xFF 0x0A`).
- `tests/fixtures/files/large_15001.lua` (15001 characters).
- Lua 5.1 snippets for syntax-error case.
