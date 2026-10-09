# Topic: custom player buttons (Lua), end to end

Scope files: `domain/src/main/java/tachiyomi/domain/custombuttons/**` (10 files). Read for exactness: `data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt`, `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, `data/src/main/sqldelightanime/migrations/129.sqm`, `app/src/main/assets/aniyomi.lua`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:535-580,740`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:316-345,880-960,2156-2175`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:236-249`, `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt`, the settings screens under `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/`.

## 1. What a custom button is

A user-defined button in the player's bottom-right controls. Each button has a title, a Lua snippet run on tap (`content`), an optional snippet run on long press (`longPressContent`) and an optional snippet run once when the player starts (`onStartup`). At most one button is the "primary" (favorite, star) button and only the primary button is drawn on the control bar (`BottomRightPlayerControls.kt:34-55`, label = the title state of section 8); EVERY button (primary included) also appears as a chip, labelled with its `name`, in the player's "More" sheet under the heading `AYMR.strings.player_sheets_custom_buttons_title` ("Custom buttons"), chip tap = `execute()`, chip long press = `executeLongPress()` (`MoreSheet.kt:209-238`). The Lua runs inside mpv (libmpv's Lua scripting), with the helper table `aniyomi` that lets a script drive the player UI.

## 2. Data lifecycle

| Operation | Rule (exact) | Aniyomi source |
|---|---|---|
| list | `ORDER BY sortIndex` | `custom_buttons.sq` `findAll` |
| create | `sortIndex = max(sortIndex) + 1` or `0`; `isFavorite = false` | `CreateCustomButton.kt:17-27`, `CustomButtonRepositoryImpl.kt:31` |
| edit | updates name/content/longPressContent/onStartup; keeps `sortIndex`; never touches `isFavorite` | `PlayerSettingsCustomButtonScreen.kt:212-223` |
| delete | delete row, then renumber survivors `0..n-1` in current `sortIndex` order | `DeleteCustomButton.kt:12-35` |
| reorder | move `button` to `newIndex` in the list, renumber all; mutex | `ReorderCustomButton.kt:17-44` |
| star | star toggles; starring one un-stars all others in one transaction; un-starring only that one | `ToggleFavoriteCustomButton.kt:17-44` |
| uniqueness | name unique among buttons (case-sensitive, exact) enforced only by the dialog | `CustomButtonDialogs.kt:77` |
| required | `name` and `content` non-empty (no trim) | `CustomButtonDialogs.kt:83` |

Transactions: renumbering and star updates use one SQL transaction (`updatePartialCustomButtons`); the pair "delete row" + "renumber" is two separate transactions (a crash between them leaves a gap in `sortIndex`, harmless because ordering is by value). Rust: do both in one transaction.

Edge cases:
1. Gaps or duplicates in `sortIndex` (older data, restore, concurrent edit): `list` order is by `sortIndex` then, for ties, by `_id` (Proposal: add `, _id` to the ORDER BY for determinism; SQLite otherwise returns ties in rowid/scan order).
2. `reorder` with `new_index >= len` returns `InternalError` upstream (IndexOutOfBounds); Rust clamps to `len - 1` and returns `Moved`.
3. `reorder` of an id that no longer exists returns `Unchanged` (no error).
4. A deleted primary button: the player has no primary until the user stars another (`primaryButton` null, nothing on the control bar).
5. Two favorites can exist after a restore of a backup with two favorites into a DB without any (see behavior 9.5). Proposal fix: the restore function tracks `has_favorite` while iterating; additionally on load, if more than one favorite exists keep the lowest `sortIndex` one (`SELECT ... ORDER BY sortIndex LIMIT 1` semantics of `firstOrNull { it.isFavorite }`, which is what the player already does).

## 3. Template substitution

`render(text, id, primaryId)`: `text.replace("$id", id.to_string()).replace("$isPrimary", if primaryId == id {"true"} else {"false"})`. `primaryId` is the id of the favorite button, or `0` when there is none (a button with id 0 cannot exist because SQLite rowids start at 1, so `0` never equals an id). The substitution is purely textual and happens on all three snippets. Examples with id 7, primary 7: `observe(function() if $isPrimary then ... end` -> `if true then`; `print("button $id")` -> `print("button 7")`. A literal `$id` inside a Lua string cannot be written (no escape).

## 4. The generated script `custombuttons.lua` (exact algorithm)

For `buttons` in `sortIndex` order and `primaryId`:

```
header = """
local lua_modules = mp.find_config_file('scripts')
if lua_modules then
    package.path = package.path .. ';' .. lua_modules .. '/?.lua;' .. lua_modules .. '/?/init.lua;' .. '<SCRIPTS_DIR>' .. '/?.lua'
end
local aniyomi = require 'aniyomi'
"""                                   // (indentation stripped; no trailing newline in Kotlin)
for each button b:
    <b.onStartup rendered>
    function button<b.id>()
        <b.content rendered>
    end
    mp.register_script_message('call_button_<b.id>', button<b.id>)
    function button<b.id>long()
        <b.longPressContent rendered>
    end
    mp.register_script_message('call_button_<b.id>_long', button<b.id>long)
```
`<SCRIPTS_DIR>` is the app's private `<filesDir>/mpv/scripts` on Android. OBSERVED defects of the Kotlin generator: (a) blocks are concatenated with NO newline between them and between the header and the first block, so the last line of one block and the first line of the next share a line: harmless for statements, but a trailing `-- comment` on the last line of `onStartup`... is followed by a newline from the template, so safe; a button whose `onStartup` ends without a newline and whose text begins with `(` can merge with the previous statement (Lua's ambiguity rule). (b) `$id`-style button ids are inserted into function names, safe because ids are integers. (c) the Lua `end` of the generated function sits on its own line, so a `content` ending with a `--` comment is safe, but a `content` containing unbalanced `end` breaks the whole file and all buttons stop working (mpv logs the Lua error; the app shows nothing).

Proposal for the Rust generator: join blocks with `"\n"`; escape `\` and `'` in the scripts-dir string; before writing the script, syntax-check each snippet individually by asking mpv to load a probe: run the generated file through `luac -p` when available, otherwise accept (no Lua dependency in the app); report load errors by watching mpv's log (`MPV_EVENT_LOG_MESSAGE` lines starting `[custombuttons]` with level `error`) and show a one-time toast "Custom buttons script has an error, see log" (key `custom_button_script_error`, new). Isolation improvement: wrap each button's three snippets in its own `do ... end` scope or `pcall(function() ... end)` so one bad button does not disable the others; wrapping with `pcall` changes error behaviour only for runtime errors, never for syntax errors (those need the per-snippet probe: load each snippet with `load(snippet)` inside the generated file itself: `local f, err = load([==[<snippet>]==]); if not f then mp.msg.error(err) else ... end`). Use level-2 long brackets and fall back to level 3+ if the snippet contains `]==]`.

## 5. Files the player must have (contract with area:45-player-core)

1. `<session config dir>/scripts/aniyomi.lua`: the bundled shim, embedded with `include_str!("../assets/aniyomi.lua")` (verbatim copy of `app/src/main/assets/aniyomi.lua`, 90 lines).
2. `<session config dir>/scripts/custombuttons.lua`: generated as above, loaded with the mpv command `load-script <path>` right after mpv is initialised (Aniyomi loads it from `PlayerActivity.setupCustomButtons` once the view model has the button list; the buttons are fetched asynchronously, so the script may load a moment after playback starts).
3. mpv requirements: libmpv built with Lua scripting (Arch's `mpv` package is built with LuaJIT) and mpv >= 0.36 for `user-data/*` properties. If mpv logs that it cannot load the script because Lua support is missing (`MPV_EVENT_LOG_MESSAGE` at `error` level mentioning `lua`), disable the feature for the session and show a banner on the Custom buttons settings page (new key `custom_button_lua_missing`: "This mpv build has no Lua support; custom buttons are disabled").

## 6. Bridge: Lua -> player (the `aniyomi` shim and its properties)

`aniyomi.lua` functions set mpv properties under `user-data/aniyomi/`; the player observes each as `MPV_FORMAT_STRING` (`AniyomiMPVView.kt:236-249`) and, on every change, calls `handleLuaInvocation(property, value)`:

1. `data = value.removePrefix("\"").removeSuffix("\"").ifEmpty { return }` (empty value = ignore; this is why every handler finishes by writing `""` back: `MPVLib.setPropertyString(property, "")`, which re-arms the property so setting the same value twice fires twice).
2. Dispatch on the last path segment:

| Lua function | Property (suffix) | Value written | Player action |
|---|---|---|---|
| `show_text(text)` | `show_text` | text | show OSD text overlay (`PlayerUpdates.ShowText`) |
| `hide_ui()` / `show_ui()` / `toggle_ui()` | `toggle_ui` | `hide` / `show` / `toggle` | hide: close sheet, panel and dialog then hide controls; show: show controls; toggle: invert controls |
| `show_subtitle_settings()` | `show_panel` | `subtitle_settings` | open the panel |
| `show_subtitle_delay()` | `show_panel` | `subtitle_delay` | open the panel |
| `show_audio_delay()` | `show_panel` | `audio_delay` | open the panel |
| `show_video_filters()` | `show_panel` | `video_filters` | open the panel |
| `show_software_keyboard()` / `hide_...` / `toggle_...` | `software_keyboard` | `show` / `hide` / `toggle` | Android IME only: DROP on desktop (no-op) |
| `set_button_title(text)` | `set_button_title` | text | primary button label text |
| `reset_button_title()` | `reset_button_title` | `unused` | restore the primary button's `name` |
| `hide_button()` / `show_button()` / `toggle_button()` | `toggle_button` | `hide` / `show` / `toggle` | hide: `primaryButton = null`; show: if null, set to the first favorite; toggle: show when null else hide |
| `previous_episode()` / `next_episode()` | `switch_episode` | `p` / `n` | `changeEpisode(previous=true / false)` |
| `pause()` / `unpause()` / `pauseunpause()` | `pause` | `pause` / `unpause` / `pauseunpause` | |
| `seek_by(v)` / `seek_to(v)` | `seek_by` / `seek_to` | integer text | `seekByWithText(v, null)` / `seekToWithText(v, null)` |
| `seek_by_with_text(v, t)` / `seek_to_with_text(v, t)` | `seek_by_with_text` / `seek_to_with_text` | `"<v>\|<t>"` | split on the first `\|` only (`split("\|", limit = 2)`) |
| `int_picker(title, fmt, start, stop, step, prop)` | `launch_int_picker` | `"<title>\|<fmt>\|<start>\|<stop>\|<step>\|<prop>"` | split on every `\|` into exactly six parts (a title containing `\|` breaks it); open an integer picker dialog with the current value `MPVLib.getPropertyInt(prop)`; on change `MPVLib.setPropertyInt(prop, v)`; `fmt` is a Java format such as `%ds` |
| legacy `left_seek_by(v)` / `right_seek_by(v)` | via `seek_by` | `-v` / `v` | |

3. Unknown suffix: nothing happens but the property is still reset to `""`.

Seek semantics (player area): `seekByWithText(value, text)`: `doubleTapSeekAmount` accumulates (`if (value < 0 && it < 0 || pos + value > duration) 0 else it + value`), `seekText = text`, `isSeekingForwards = value > 0`, `seekBy(value, preciseSeek)`, show seek bar if the preference says so. `seekToWithText(v, t)`: amount = `v - pos`, then `seekTo(v, preciseSeek)`.

Dependencies of the seed button: it observes `user-data/current-anime/intro-length` (a number set by the player when AniSkip supplies the intro length; area:45/48) and calls `aniyomi.right_seek_by(length)` on tap, `aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")` on long press; its title becomes `"+<length> s"`; length 0 hides the button. The title `+85 s` is the default `name`.

## 7. Bridge: player -> Lua (clicks)

Tap: `mpv_command(["script-message", "call_button_<id>"])`; long press: `["script-message", "call_button_<id>_long"]` (`PlayerViewModel.kt:2167-2173`). In the More sheet every custom button is a chip; tap/long-press call the same functions. Rust: `fn execute(id) / execute_long_press(id)` in `kuro_core::domain::custom_button` returning the `Vec<String>` argument list for the mpv command so the player crate (area:45) sends it with `mpv.command(&args)`.

## 8. Title state machine (primary button)

State: `primary: Option<CustomButton>`, `title: String`. Init: `primary = first favorite`; if `title.is_empty()` then `title = primary.name` (a Lua script may have already set a title because buttons load asynchronously). Events: `set_button_title(t)` -> `title = t`; `reset_button_title` -> `title = first favorite's name` (no-op when no favorite); `hide_button` -> `primary = None` (title unchanged); `show_button` -> `primary = first favorite` if currently `None`. The UI draws the primary button only when `primary.is_some()`, with `title`.

## 9. Backup / restore rules

Create: all buttons (without ids), proto numbers 1..6 (data doc 1.8). Restore (`CustomButtonRestorer`): skip if the backup list is empty; skip buttons whose NAME exists; insert others after the current max `sortIndex` in the backup's `sortIndex` order; demote `isFavorite` to false when the DB already had a favorite. Test vector: DB = [A(sort 0, fav)], backup = [B(sort 5, fav), A'(sort 1, name A), C(sort 2)]. Sorted by sortIndex: A'(skip, name exists), C, B. nextSortIndex = 1. C -> sortIndex 1, fav false; B -> sortIndex 2, fav false (DB has favorite). Result order: A, C, B.

## 10. Drop / replace list

| Android feature | Decision |
|---|---|
| `software_keyboard` bridge | Drop (no soft keyboard on desktop); the Lua function stays defined as a no-op so existing scripts keep loading |
| Lua syntax-highlighted text field (Compose `luaHighlight` + `githubTheme`) | Replace with `gtk::TextView` plus `sourceview5` if the user has it; otherwise a plain monospace `TextView` (no new hard dependency; GtkSourceView 5 is optional at runtime via `gtk` feature flag) |
| Drag handle reorder (`sh.calvin.reorderable`) | Replace with `gtk::DragSource`/`DropTarget` on rows plus keyboard `Alt+Up/Down` and an explicit "Move up/down" row menu (accessibility) |
| Help icon link to aniyomi.org docs | Keep (opens `https://aniyomi.org/docs/guides/player-settings/custom-buttons`) |

## 11. Tasks

T-09a-034, T-09a-035, T-09a-036, T-09a-037, T-09a-038, T-09a-039 in `p1-tasks.md`.
