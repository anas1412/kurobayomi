# Topic: player custom buttons (repository, interactors, seeded button, Lua assembly)

Scope file: `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt` (84 LOC). Read alongside: `domain/src/main/java/tachiyomi/domain/custombuttons/**` (model, 6 interactors, exception, repository contract), `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, `data/src/main/sqldelightanime/migrations/129.sqm`, `app/src/main/assets/aniyomi.lua`, `PlayerViewModel.kt:319-345`, `PlayerActivity.kt:535-580`, `PlayerViewModel.kt:2156-2173`, `PlayerSettingsCustomButtonScreenModel.kt`, `CustomButtonRestorer.kt`, `CustomButtonBackupCreator.kt`.

Custom buttons let the user define extra player buttons whose behaviour is a Lua snippet running inside mpv. The feature is meaningful on Linux because Kurobayomi plays through libmpv (the same engine), so the Lua code works unchanged as long as the `aniyomi` Lua module and the `user-data/aniyomi/*` property protocol are provided by the player area (area:45-player-core, area:46-player-controls-ui, area:48-settings-player-and-scripts).

## 1. Data

Table (anime DB only): `custom_buttons(_id INTEGER PK, name TEXT NOT NULL, isFavorite INTEGER NOT NULL, sortIndex INTEGER NOT NULL, content TEXT NOT NULL, longPressContent TEXT NOT NULL, onStartup TEXT NOT NULL)`. No UNIQUE constraints: two buttons may share a name (the string `pref_player_custom_button_error_exists` "Button already exists!" has no producer in this area; the editor screen may check names itself, area:48). Ordering is `ORDER BY sortIndex` with no tiebreaker (Proposal: `, _id`).

Rust model:

```rust
pub struct CustomButton { pub id: i64, pub name: String, pub is_favorite: bool, pub sort_index: i64,
                          pub content: String, pub long_press_content: String, pub on_startup: String }
#[derive(Default)]
pub struct CustomButtonUpdate { pub id: i64, pub name: Option<String>, pub is_favorite: Option<bool>, pub sort_index: Option<i64>,
                                pub content: Option<String>, pub long_press_content: Option<String>, pub on_startup: Option<String> }
```

## 2. The seeded button (byte-exact)

Inserted with `INSERT OR IGNORE` at schema creation (`custom_buttons.sq:13-30`) and by migration 129. `_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`. The user can delete or edit it; it is never re-inserted afterwards (the statement only runs on creation/migration). Whitespace matters only inside Lua strings, but is reproduced exactly (the `onStartup` text contains TAB characters on four lines, written `<TAB>` below).

`content`:

```lua
local intro_length = mp.get_property_number("user-data/current-anime/intro-length")
aniyomi.right_seek_by(intro_length)
```

`longPressContent` (single line):

```lua
aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")
```

`onStartup` (lines in order; `<TAB>` = U+0009; the text has no trailing newline):

```text
function update_button(_, length)
  if length ~= nil then
    if length == 0 then
<TAB>  aniyomi.hide_button()
<TAB>  return
<TAB>else
<TAB>  aniyomi.show_button()
<TAB>end
    aniyomi.set_button_title("+" .. length .. " s")
  end
end

if $isPrimary then
  mp.observe_property("user-data/current-anime/intro-length", "number", update_button)
end
```

Implementation rule: store the three strings as Rust `const`s in `kuro_core::data::custom_button::seed` with a unit test comparing them against the SQL text in the schema file shipped by area:04 (so the two copies cannot drift). Meaning: the favourite ("primary") button's title is `+85 s` initially; `current-anime/intro-length` (seconds, set by the player per anime from the skip-intro preference, area:45) updates the title to `+<length> s` and hides the button when 0. `aniyomi.int_picker(...)` on long press opens the integer picker dialog (`launch_int_picker`).

## 3. Placeholder expansion

For every button the three scripts are passed through (`CustomButton.kt:getButtonContent/getButtonLongPressContent/getButtonOnStartup`):

```text
isPrimary = if primaryId == id { "true" } else { "false" }
out = text.replace("$id", id.to_string()).replace("$isPrimary", isPrimary)
```

`primaryId` is `viewModel.primaryButton.value?.id ?: 0L` where primary = the first button with `isFavorite` (so at most one button gets `true`; if none is favourite, `0` matches nothing). Order is significant: `$id` first. Rust: `fn expand(&self, text: &str, primary_id: i64) -> String` using `str::replace` in that order; test: `"$identity $isPrimary"` for id 7, primary 7 -> `"7entity true"`; the quirk is preserved (documented, not "fixed", because user scripts may depend on `$id` being usable inside identifiers like `button_$id`).

## 4. Script assembly and execution (consumer, for area:45)

Observed (`PlayerActivity.kt:535-580`), executed once per player start from `PlayerViewModel.init` after `getCustomButtons.getAll()` succeeds:

1. `scriptsDir = <filesDir>/mpv/scripts` (created on demand).
2. Build one Lua source: preamble `local lua_modules = mp.find_config_file('scripts')`, `if lua_modules then package.path = package.path .. ';' .. lua_modules .. '/?.lua;' .. lua_modules .. '/?/init.lua;' .. '<scriptsDir>' .. '/?.lua' end`, `local aniyomi = require 'aniyomi'` (the preamble is a raw string passed through `trimIndent()`; there is no trailing newline).
3. For each button in `sortIndex` order append (each block also `trimIndent()`ed after interpolation, no leading/trailing newline): `<onStartup expanded>`, `function button<ID>()`, `<content expanded>`, `end`, `mp.register_script_message('call_button_<ID>', button<ID>)`, `function button<ID>long()`, `<longPressContent expanded>`, `end`, `mp.register_script_message('call_button_<ID>_long', button<ID>long)`.
4. Write the text to `scripts/custombuttons.lua` (replacing the file) and `MPVLib.command(["load-script", <path>])`.
5. Pressing a button: `MPVLib.command(["script-message", "call_button_<id>"])`; long press: `"call_button_<id>_long"` (`PlayerViewModel.kt:2167-2173`).
6. The primary button's title in the control strip is `primaryButtonTitle`, initialised to the button's `name` unless a Lua script has already set it via `user-data/aniyomi/set_button_title`.
7. Failure to load buttons: `CustomButtonFetchState.Error(e.message ?: "Unable to fetch buttons")` and no script is loaded (the buttons list in the sheet shows nothing); the player keeps working.

Defects to avoid in the Rust port (task T-06a-062):
* Blocks are concatenated with no separator, so the preamble's last line runs into the first `onStartup` text (`local aniyomi = require 'aniyomi'function update_button(_, length)`). Lua tolerates this when the next token is a keyword/identifier, but not when it starts with `(` (ambiguous call) — Rust joins every block with `"\n"`.
* `trimIndent()` removes common leading whitespace across the *interpolated* result; it is not needed in Rust. Do not trim user text.
* A syntax error in any user script breaks the whole file: mpv logs the error and none of the buttons work. Proposal: write each button into its own file `custombutton_<id>.lua` (preamble + that button's blocks) and `load-script` each, so one bad button cannot disable the others; the message names `call_button_<id>` stay identical. The `call_button_<id>` message names stay identical.

The `aniyomi` Lua module (`app/src/main/assets/aniyomi.lua`, 90 lines) is an adapter that turns Lua calls into mpv `user-data` property writes that the host observes. Public functions the host must support (area:45/46 implement the property observers): UI: `show_text`, `hide_ui`, `show_ui`, `toggle_ui`, `show_subtitle_settings`, `show_subtitle_delay`, `show_audio_delay`, `show_video_filters`, `show_software_keyboard`, `hide_software_keyboard`, `toggle_software_keyboard`; buttons: `set_button_title`, `reset_button_title`, `hide_button`, `show_button`, `toggle_button`; controls: `previous_episode`, `next_episode`, `pause`, `unpause`, `pauseunpause`, `seek_by`, `seek_to`, `seek_by_with_text`, `seek_to_with_text` (value and text joined with `|`), `int_picker(title, name_format, start, stop, step, property)` (all joined with `|`); legacy: `left_seek_by(v) = seek_by(-v)`, `right_seek_by(v) = seek_by(v)`. Properties observed by the Android host (`AniyomiMPVView.kt:236-251`): `user-data/aniyomi/{show_text, toggle_ui, show_panel, software_keyboard, set_button_title, reset_button_title, toggle_button, switch_episode, pause, seek_by, seek_to, seek_by_with_text, seek_to_with_text, launch_int_picker}` (all strings) and `user-data/current-anime/intro-length` (int64). `software_keyboard` has no desktop meaning (task P3 drop in area:46).

## 5. Interactor semantics (domain, ported into `CustomButtonRepo`)

| Operation | Algorithm (OBSERVED) | Rust |
|---|---|---|
| `create(name, content, longPress, onStartup)` | `withNonCancellableContext`: `all = getAll()`; `next = all.maxOfOrNull { it.sortIndex }?.plus(1) ?: 0`; `insertCustomButton(name, next, content, longPressContent, onStartup)` (`isFavorite = false`); `Result.Success` or, on any exception, log + `InternalError(e)` | One `write` closure computing `next` and inserting (Kotlin's read-then-insert is racy; Rust is atomic) |
| `delete(id)` | delete; then `all = getAll()` (already ordered by `sortIndex`) and `updatePartial` for each survivor `sortIndex = index` in ONE transaction; errors from either step -> `InternalError` (after the first step failed nothing else runs) | one `write`: `DELETE` + renumber loop. If the deleted button was the favourite, no new favourite is chosen (the strip shows no custom button) |
| `reorder(button, newIndex)` | `Mutex`; `list = getAll()`; `current = indexOfFirst{id}`; -1 -> `Result.Unchanged`; `list.add(newIndex, list.removeAt(current))` (IndexOutOfBounds when `newIndex > size - 1` or negative -> caught -> `InternalError`); renumber all `0..n-1`; one transaction | `fn reorder(id, new_index)`: same, `new_index` clamped is NOT applied (an out-of-range index returns `Err(InvalidIndex)` -> mapped to `InternalError` for screen parity) |
| `toggleFavorite(button)` | if `button.isFavorite`: `updatePartial(id, isFavorite = false)`. else: `Mutex`; for every button set `isFavorite = (id == target)` in one transaction | same, in one `write` |
| `update(update)` | `updatePartial`; exception -> `InternalError` (NOT logged here) | same; returns `Result` (Kotlin returns `Unit` on success and `InternalError` on failure; the Rust API returns `Result<(), CustomButtonError>` uniformly) |
| `update(list)` | `updatePartialCustomButtons`, one transaction | same |
| `getAll`, `subscribeAll` | `ORDER BY sortIndex` | `all()`, `watch_all()` (table `CustomButtons`) |

Screen model (`PlayerSettingsCustomButtonScreenModel`): any `InternalError` -> one-shot event -> toast `MR.strings.internal_error`. Dialogs: Create, Edit(button), Delete(button) (`pref_player_custom_button_delete_confirm` "Do you wish to delete the button "%s"?"). Empty state `pref_player_custom_button_empty`. State is `Loading` until the first `subscribeAll` emission.

## 6. Backup and restore interplay (area:21/22)

* Create: `CustomButtonBackupCreator` maps every row with `backupCustomButtonsMapper` (name, isFavorite, sortIndex, content, longPressContent, onStartup).
* Restore (`CustomButtonRestorer.kt`): read existing buttons; index them by name; `nextSortIndex = max(existing.sortIndex) + 1 or 0`; `dbHasFavorite = existing.any { isFavorite }`; for each backup button sorted by `sortIndex`: if a DB button with the same name exists, skip it (existing wins, even if the scripts differ); else insert with `isFavorite = backup.isFavorite && !dbHasFavorite` (note: `dbHasFavorite` is not updated inside the loop, so several backup favourites can be restored as favourites when the DB had none), `sortIndex = nextSortIndex++`. Rust port lives in area:22 but must call `CustomButtonRepo::insert_raw(name, is_favorite, sort_index, ...)` (a variant of insert that accepts `isFavorite`) — included in task T-06a-060.

## 7. Security note

The scripts execute arbitrary Lua inside mpv with full `mp` API access (files, subprocess via `mp.command_native({name = "subprocess", args = {"sh", "-c", "true"}})`). They come only from the user's own database or backup. A backup from another person can carry malicious scripts; area:22 should show a confirmation listing the button names when a restore adds custom buttons (Proposal; `CustomButtonRepo::insert_raw` returns the number inserted so the restore summary can report it). mpv's own `load-script` is the trust boundary; Kurobayomi adds no sandbox.
