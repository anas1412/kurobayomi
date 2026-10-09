# Topic: custom buttons, primary button, and the mpv Lua bridge

Repo-relative paths are relative to `/home/blackbox/aniyomi-src`. Owner area: 48-settings-player-and-scripts. Related: area:45-player-core (player runtime), area:46-player-controls-ui (button rendering), area:22-backup-restore.

## 1. What the feature is

A user writes Lua snippets in a small editor. Each snippet set is a **custom button**: a name, a Lua body run on tap, an optional Lua body run on long press, and an optional Lua body run once when the player starts. One button can be the **favourite** (primary). The favourite's name is drawn as a pill in the player's bottom-right controls. Taps call the tap body; long presses call the long-press body. The favourite's Lua also sees `$isPrimary == true`.

The default install ships one button, `+85 s` (id 1), which is the intro-length seek button. It is favourite by default.

## 2. Domain model

`domain/src/main/java/tachiyomi/domain/custombuttons/model/CustomButton.kt`:

```kotlin
data class CustomButton(
    val id: Long,
    val name: String,
    val isFavorite: Boolean,
    val sortIndex: Long,
    val content: String,
    val longPressContent: String,
    val onStartup: String,
)
```

Placeholder substitution (`CustomButton.kt:12-25`):
- `getButtonContent(primaryId)`, `getButtonLongPressContent(primaryId)`, `getButtonOnStartup(primaryId)` each do:
  - `isPrimary = if (primaryId == id) "true" else "false"`
  - `text.replace("$id", id.toString()).replace("$isPrimary", isPrimary)` (plain string replacement, not regex, applied in this order).
- `primaryId` is the favourite button's id, or `0L` if no favourite exists (`PlayerActivity.kt:543`). Because real ids start at 1, `$isPrimary` is `false` everywhere when no favourite exists.
- Substitution is textual: `$id` inside a string literal in user Lua is also replaced.

`CustomButtonUpdate.kt`: `id: Long` plus nullable `name`, `isFavorite`, `sortIndex`, `content`, `longPressContent`, `onStartup`. `null` means "leave the column unchanged" (the SQL uses `coalesce`).

Exception `SaveCustomButtonException(throwable)` extends `IOException("Error Saving Custom Button to Database", throwable)` (`domain/src/main/java/tachiyomi/domain/custombuttons/exception/SaveCustomButtonException.kt`). It wraps `SQLiteException` so the domain does not depend on Android.

## 3. Storage

Table (from `data/src/main/sqldelightanime/dataanime/custom_buttons.sq`, created by `data/src/main/sqldelightanime/migrations/129.sqm`):

```sql
CREATE TABLE custom_buttons (
    _id INTEGER NOT NULL PRIMARY KEY,
    name TEXT NOT NULL,
    isFavorite INTEGER AS Boolean NOT NULL,
    sortIndex INTEGER NOT NULL,
    content TEXT NOT NULL,
    longPressContent TEXT NOT NULL,
    onStartup TEXT NOT NULL
);
```

No UNIQUE constraint on name, no index on sortIndex, no CHECK on isFavorite. Uniqueness of names is enforced only by the dialog and by the backup restorer.

Named queries (`custom_buttons.sq`):
- `findAll`: `SELECT * FROM custom_buttons ORDER BY sortIndex;` (ties: undefined order in SQLite; proposal below fixes this).
- `insert(name, isFavorite, sortIndex, content, longPressContent, onStartup)`: plain insert.
- `delete(_id)`: `DELETE FROM custom_buttons WHERE _id = :customButtonId;`
- `update(name?, isFavorite?, sortIndex?, content?, longPressContent?, customButtonId, onStartup?)`:
  ```sql
  UPDATE custom_buttons
  SET name = coalesce(:name, name),
      isFavorite = coalesce(:isFavorite, isFavorite),
      sortIndex = coalesce(:sortIndex, sortIndex),
      content = coalesce(:content, content),
      longPressContent = coalesce(:longPressContent, longPressContent),
      onStartup = coalesce(:onStartup, onStartup)
  WHERE _id = :customButtonId;
  ```
- `selectLastInsertedRowId`: `SELECT last_insert_rowid();`

Repository implementation (`data/src/main/java/tachiyomi/data/custombutton/CustomButtonRepositoryImpl.kt`):
- `subscribeAll()` → `handler.subscribeToList { findAll }` (live query, re-emits on table change).
- `getAll()` → `handler.awaitList { findAll }`.
- `insertCustomButton(...)`: `insert(name, false, sortIndex, content, longPressContent, onStartup)` (always non-favourite). Wraps `SQLiteException` as `SaveCustomButtonException`.
- `updatePartialCustomButton(update)`: single `update` statement.
- `updatePartialCustomButtons(updates)`: one transaction (`handler.await(inTransaction = true)`), loops `update`.
- `deleteCustomButton(id)`: single `delete`.

Seed row (migration 129, `INSERT OR IGNORE`; the same statement also appears in the `.sq` file). Columns: `_id = 1`, `name = '+85 s'`, `isFavorite = 1`, `sortIndex = 0`. The `content`, `longPressContent`, `onStartup` values are below, copied exactly. The `onStartup` block contains literal TAB characters before `aniyomi.hide_button()` and `aniyomi.show_button()` (the source has a tab, then two spaces). Keep them as tabs in the port, or the output is identical in Lua anyway.

content:
```lua
local intro_length = mp.get_property_number("user-data/current-anime/intro-length")
aniyomi.right_seek_by(intro_length)
```

longPressContent:
```lua
aniyomi.int_picker("Change intro length", "%ds", 0, 255, 1, "user-data/current-anime/intro-length")
```

onStartup:
```lua
function update_button(_, length)
  if length ~= nil then
    if length == 0 then
	  aniyomi.hide_button()
	  return
	else
	  aniyomi.show_button()
	end
    aniyomi.set_button_title("+" .. length .. " s")
  end
end

if $isPrimary then
  mp.observe_property("user-data/current-anime/intro-length", "number", update_button)
end
```

Note: `user-data/current-anime/intro-length` is written by the player (area:45). Its value comes from the per-anime intro-length value, default `85` from `pref_default_intro_length`.

## 4. Operations (exact algorithms)

### 4.1 Create (`CreateCustomButton.await(name, content, longPressContent, onStartup)`)
`domain/src/main/java/tachiyomi/domain/custombuttons/interactor/CreateCustomButton.kt`
1. Inside `withNonCancellableContext`: `all = getAll()`.
2. `nextSortIndex = (all.maxOfOrNull { it.sortIndex } + 1) ?: 0`. Note: `maxOfOrNull` returns the max, `+1`, and if the list is empty the result is `0`.
3. `insertCustomButton(name, nextSortIndex, content, longPressContent, onStartup)`. Not favourite.
4. Result: `Success`, or `InternalError(e)` for **any** `Exception` (logged at ERROR).
- No mutex. Two concurrent creates can read the same max and insert equal sortIndex values (edge E-CB-04).

### 4.2 Update (`UpdateCustomButton.await(update)`)
1. `updatePartialCustomButton(update)`.
2. Any `Exception` → `InternalError(e)`, **not logged** (unlike other interactors).
- Screen always sends `sortIndex = dialog.customButton.sortIndex` (`PlayerSettingsCustomButtonScreen.kt:72-81`) from the snapshot taken when the dialog opened. Consequence: if the list was reordered while the edit dialog was open, saving writes the stale sortIndex back and two buttons can end up with the same sortIndex (edge E-CB-05). Proposal: edits never write sortIndex.
- `isFavorite` is never sent from the edit dialog, so editing does not change favourite state.

### 4.3 Delete (`DeleteCustomButton.await(id)`)
`domain/src/main/java/tachiyomi/domain/custombuttons/interactor/DeleteCustomButton.kt`
1. `deleteCustomButton(id)`. Exception → `InternalError` (logged).
2. `all = getAll()` (ordered by sortIndex).
3. Build updates `CustomButtonUpdate(id, sortIndex = index)` for each element in order (0-based, contiguous).
4. `updatePartialCustomButtons(updates)` in one transaction. Exception → `InternalError` (logged).
5. `Success`.
- Deleting the favourite leaves **no favourite**. Nothing promotes another button (observed). The player then shows no primary button (see section 6).
- Note step 1 and step 4 are separate transactions: a crash between them leaves gaps in sortIndex (harmless, since ordering is relative).

### 4.4 Reorder (`ReorderCustomButton.changeOrder(customButton, newIndex)`)
`domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ReorderCustomButton.kt`
1. Under `Mutex` (instance-level, shared by all callers of this interactor instance): `list = getAll().toMutableList()`.
2. `currentIndex = list.indexOfFirst { it.id == customButton.id }`; if `-1` → `Unchanged`.
3. Inside try: `list.add(newIndex, list.removeAt(currentIndex))`. If `newIndex` is out of range after removal, `IndexOutOfBoundsException` is raised and caught → `InternalError`.
4. `updates = list.mapIndexed { i, b -> CustomButtonUpdate(id = b.id, sortIndex = i) }`; `updatePartialCustomButtons(updates)` in one transaction.
5. `Success`. Exception → `InternalError` (logged).
- The UI calls this **on every drag step** (`CustomButtonScreen.kt:101-105`, the reorderable callback fires per position change). Each call writes every row. Proposal: call once on drop.

### 4.5 Toggle favourite (`ToggleFavoriteCustomButton.await(customButton)`)
`domain/src/main/java/tachiyomi/domain/custombuttons/interactor/ToggleFavoriteCustomButton.kt`
- If `customButton.isFavorite == true`: `updatePartialCustomButton(id, isFavorite = false)`; `Success`. (Not under the mutex.) Result: no favourite.
- Else, under the mutex: `list = getAll()`; updates = for each button `isFavorite = (btn.id == customButton.id)`; `updatePartialCustomButtons(updates)` in one transaction; `Success`. Exactly one favourite results.
- Exception → `InternalError` (logged).
- Note: the favourite check uses the **snapshot** passed in, not a fresh read. A stale snapshot can flip a button off when the UI thinks it is off (edge E-CB-06).

### 4.6 Read (`GetCustomButtons`)
`subscribeAll()` (live Flow) and `getAll()` (one-shot). `PlayerViewModel` uses `getAll()` at init and `setupCustomButtons` (section 5).

## 5. Screen behaviour

Files: `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/custombutton/` (`PlayerSettingsCustomButtonScreen.kt`, `PlayerSettingsCustomButtonScreenModel.kt`, `components/CustomButtonScreen.kt`, `components/CustomButtonListItem.kt`, `components/CustomButtonDialogs.kt`).

**Screen model**
- Initial state `Loading`. `init` subscribes to `subscribeAll()` and sets `Success(customButtons, dialog = null)` on each emission (the dialog state is preserved by `copy`? Observed: each emission creates a **new** `Success` with `dialog` defaulting to `null`, which closes any open dialog when the list changes underneath (edge E-CB-07).
- Dialog state: `CustomButtonDialog { Create; Edit(customButton); Delete(customButton) }`. `showDialog` / `dismissDialog` update `Success.dialog`.
- Events: `CustomButtonEvent.InternalError` (message `MR.strings.internal_error` = "InternalError: Check crash logs for further information") is sent on any `InternalError` from create, toggle, edit, delete, reorder. Shown as a toast (`context.toast(stringRes)`).

**Screen**
- App bar title: `pref_player_custom_button_header` = "Edit custom buttons". Action: help icon (`Icons.AutoMirrored.Outlined.HelpOutline`, description `pref_player_custom_button_guide` = "Custom buttons guide") opens `https://aniyomi.org/docs/guides/player-settings/custom-buttons`.
- FAB: `FloatingActionAddButton` (shared component, given the list state), opens Create.
- Empty state: `EmptyScreen(stringRes = pref_player_custom_button_empty)` = "You have no custom buttons. Tap the plus button to create a custom button."
- List: `LazyColumn`, `contentPadding = paddingValues + topSmallPaddingValues + PaddingValues(horizontal = padding.medium)`, `verticalArrangement = spacedBy(padding.small)`. Item key `"custombutton-$id"`. Items are `animateItem()`.
- Local list copy: drag mutates a local `SnapshotStateList`; while **not** dragging, the local list is replaced by the DB list on each DB emission (`LaunchedEffect(customButtons)`).

**Card** (`CustomButtonListItem`)
- `ElevatedCard`, whole card clickable → Edit.
- Row: drag handle (`Icons.Outlined.DragHandle`), name (weight 1), star button, edit button (`Icons.Outlined.Edit`, description `action_edit`), delete button (`Icons.Outlined.Delete`, description `action_delete`).
- Star: `Icons.Outlined.Star` when favourite, else `Icons.Outlined.StarOutline`, tint `Color(0xFFFDD835)`.
- Below the row: `content` text, `bodySmall`, 75% alpha, `maxLines = 3`, ellipsis, start padding `padding.medium`, bottom padding `padding.medium`.

**Create / Edit dialog** (`CustomButtonButtonDialog`)
- Title (create): `pref_player_custom_button_add` = "Add button"; confirm `action_add` = "Add". Title (edit): `pref_player_custom_button_edit` = "Edit button"; confirm `action_edit` = "Edit". Edit title row also shows `pref_player_custom_button_id` = "(ID: %d)" at alpha `DISABLED_ALPHA`, right-aligned via a spacer.
- Fields, in order:
  1. Title: `OutlinedTextField`, single line, label `pref_player_custom_button_title` = "Title", supporting text `MR.strings.information_required_plain` = "*required" or, when the title duplicates another button, `pref_player_custom_button_error_exists` = "Button already exists!" (error state when duplicate and non-empty).
  2. Lua code: label `pref_player_custom_button_content` = "Lua code", supporting "*required", min 3 lines, max 5 lines.
  3. Long press: label `pref_player_custom_button_content_long` = "Lua code (on long press)", supporting `pref_player_custom_button_optional` = "(optional)", min 3, max 5 lines.
  4. Startup: label `pref_player_custom_button_startup` = "On startup", supporting "(optional)", min 2, max 4 lines.
- Duplicate check: `buttonNames` for Create = all names; for Edit = all names except the edited button's own name.
- Confirm enabled iff `title.isNotEmpty() && content.text.isNotEmpty() && !titleAlreadyExists`. Long press and startup may be empty.
- On confirm: `onAction(title, content, longPress, startup)` then close. In the Edit case the update carries `id`, `name`, `sortIndex` (stale snapshot, see 4.2), `content`, `longPressContent`, `onStartup`. `isFavorite` is not sent.
- Field values are `TextFieldValue` with a Lua syntax-highlight `AnnotatedString` (`githubTheme` + `luaHighlight`). Highlight is recomputed on every edit (no length limit here, unlike the code editor).
- Initial focus on the title after `0.1 s`.
- The Create dialog closes on confirm even when the DB write later fails; the error appears as a toast afterwards.

**Delete dialog**: title `pref_player_custom_button_delete` = "Delete button"; body `pref_player_custom_button_delete_confirm` = `Do you wish to delete the button "%s"?`; OK `action_ok`; Cancel `action_cancel`.

## 6. Runtime: primary button in the player

Files: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`, `PlayerActivity.kt`, `controls/BottomRightPlayerControls.kt`.

**State** (`PlayerViewModel.kt:319-330`):
- `customButtons: StateFlow<CustomButtonFetchState>` (`Loading`, `Success(list)`, `Error(message)`).
- `primaryButton: StateFlow<CustomButton?>`, initially `null`.
- `primaryButtonTitle: StateFlow<String>`, initially `""`.

**Init** (`PlayerViewModel.kt:332-345`, `launchIO`):
1. `buttons = getCustomButtons.getAll()`.
2. `buttons.firstOrNull { it.isFavorite }` → set `primaryButton`. If `primaryButtonTitle` is empty, set it to the favourite's `name`. (If a Lua script already set a title, it is kept.)
3. `activity.setupCustomButtons(buttons)` (writes the Lua file, section 7).
4. `customButtons = Success(buttons)`. Exception → `Error(e.message ?: "Unable to fetch buttons")`.

**Bridge handlers** that touch this state (section 8):
- `set_button_title` → `primaryButtonTitle = data`.
- `reset_button_title` → `primaryButtonTitle = favourite.name` (if a favourite exists in `customButtons`).
- `toggle_button`: `show` → if `primaryButton == null`, set it to the favourite from `customButtons` (may remain `null` if none). `hide` → `primaryButton = null`. `toggle` → if `primaryButton == null` show, else hide.

**Rendering** (`BottomRightPlayerControls.kt:33-56`):
- If `skipIntroButton != null` (text from the intro-skip feature, area:45) → show it as a `FilledControlsButton`; long press does nothing. **The custom button is not shown.**
- Else if `customButton != null` → `FilledControlsButton(text = customButtonTitle, onClick = customButton::execute, onLongClick = customButton::executeLongPress)`.
- Else nothing.

**Execution**: `CustomButton.execute()` sends `script-message call_button_<id>`; `executeLongPress()` sends `script-message call_button_<id>_long` (`PlayerViewModel.kt:2167-2173`). These names are registered by the generated Lua (section 7).

## 7. Generated `custombuttons.lua`

`PlayerActivity.setupCustomButtons(buttons)` (`PlayerActivity.kt:535-600`), run on `Dispatchers.IO`:

1. `primaryButtonId = viewModel.primaryButton.value?.id ?: 0L`.
2. Header (`trimIndent`ed):
```lua
local lua_modules = mp.find_config_file('scripts')
if lua_modules then
    package.path = package.path .. ';' .. lua_modules .. '/?.lua;' .. lua_modules .. '/?/init.lua;' .. '<scriptsDir>' .. '/?.lua'
end
local aniyomi = require 'aniyomi'
```
   `<scriptsDir>` is `filesDir/mpv/scripts` (`scriptsDir()!!.filePath`; the `!!` throws if the directory cannot be created).
3. For **each** button in list order (all buttons, not only favourite), append (trimIndent'd template, then substitutions):
```lua
<onStartup with $id/$isPrimary replaced>
function button<id>()
    <content with $id/$isPrimary replaced>
end
mp.register_script_message('call_button_<id>', button<id>)
function button<id>long()
    <longPressContent with $id/$isPrimary replaced>
end
mp.register_script_message('call_button_<id>_long', button<id>long)
```
   Note: the function name for long press is `button<id>long` (no underscore), while the message name is `call_button_<id>_long`.
4. Write to `filesDir/mpv/scripts/custombuttons.lua` (overwrite), then `MPVLib.command(["load-script", <path>])`.

Consequences (observed):
- Any Lua syntax error in any button's `onStartup`, `content`, or `longPressContent` makes the whole file fail to load, which disables every button. There is no per-button isolation.
- `onStartup` runs at load time, in global scope, for every button, not only the favourite. Only the favourite's `$isPrimary` is `true`.
- Empty `longPressContent` produces an empty function body (valid Lua).
- Button names are never written into Lua (they are only UI). Ids are interpolated as integers.
- The file is rewritten only at player start, not when the list changes. Edits made while the player is open take effect next time.

**Proposal:** generate per-button chunks inside `pcall`-guarded blocks, for example `local ok, err = pcall(function() <content> end)`, so one bad button cannot disable the others. Log the error with the button name through mpv's `msg` (`mp.msg.error`). Regenerate on change while the player is open and `load-script` again (or `script-message` a reload). Mark this as a deliberate deviation.

## 8. Bridge protocol (`aniyomi.lua` ↔ player)

Bundled file: `app/src/main/assets/aniyomi.lua` (90 lines). Every function sets an mpv user-data property; the player observes them.

Observed properties (`app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:236-249`, all `MPV_FORMAT_STRING`): `user-data/aniyomi/show_text`, `toggle_ui`, `show_panel`, `software_keyboard`, `set_button_title`, `reset_button_title`, `toggle_button`, `switch_episode`, `pause`, `seek_by`, `seek_to`, `seek_by_with_text`, `seek_to_with_text`, `launch_int_picker`.

Dispatch (`PlayerActivity.kt:740`: a property whose parent path is `user-data/aniyomi` → `PlayerViewModel.handleLuaInvocation(property, value)`).

Handler (`PlayerViewModel.kt:856-914`):
1. `data = value.removePrefix("\"").removeSuffix("\"")`; if empty → return without action (but see step 3).
2. Switch on the last path segment:

| Property | Accepted `data` | Effect |
|---|---|---|
| `show_text` | any | `playerUpdate = ShowText(data)` (OSD text) |
| `toggle_ui` | `show` → `showControls()`; `toggle` → show if hidden else hide; `hide` → clear `sheetShown`, `panelShown`, `dialogShown` to none, then `hideControls()` | |
| `show_panel` | `subtitle_settings`, `subtitle_delay`, `audio_delay`, `video_filters` | `showPanel(Panels.*)` |
| `software_keyboard` | `show` → force show IME; `hide` → force hide; `toggle` → hide if `inputMethodManager.isActive` else show | Android-only (area:46 desktop equivalent: none) |
| `set_button_title` | any | primary title = `data` |
| `reset_button_title` | any | primary title = favourite name |
| `toggle_button` | `show`, `hide`, `toggle` (see section 6) | |
| `switch_episode` | `n` → next episode, `p` → previous | `changeEpisode(false)` for n; `changeEpisode(true)` for p. (Passes `true` for previous.) |
| `pause` | `pause`, `unpause`, `pauseunpause` | |
| `seek_by` | integer string | `seekByWithText(data.toInt(), null)` |
| `seek_to` | integer string | `seekToWithText(data.toInt(), null)` |
| `seek_by_with_text` | `"<int>|<text>"` split limit 2 | `seekByWithText(int, text)` |
| `seek_to_with_text` | `"<int>|<text>"` split limit 2 | `seekToWithText(int, text)` |
| `launch_int_picker` | `"<title>|<nameFormat>|<min>|<max>|<step>|<property>"` split on `|` (exactly 6 parts) | opens integer picker dialog |

3. After the `when`, always: `MPVLib.setPropertyString(property, "")` (clears the property so the same value re-triggers the observer).

Parsing notes (observed):
- `seek_by` / `seek_to` use `toInt()`; invalid input throws `NumberFormatException` in the observer (uncaught; edge E-CB-08).
- `launch_int_picker` destructures six parts; a `|` in the title or name format shifts the fields and the `toInt()` calls throw.
- The `IntegerPicker` dialog: `defaultValue = MPVLib.getPropertyInt(property)`, `onChange` sets the property with `setPropertyInt`, dismiss → `Dialogs.None`.
- `seekByWithText` and `seekToWithText` also update `_doubleTapSeekAmount` and `_seekText` (OSD), and show the seek bar when `showSeekBar` is on (`PlayerViewModel.kt:945-961`).

Lua API (from `aniyomi.lua`):

| Function | Property set | Value |
|---|---|---|
| `aniyomi.show_text(text)` | `show_text` | text |
| `aniyomi.hide_ui()` / `show_ui()` / `toggle_ui()` | `toggle_ui` | `hide` / `show` / `toggle` |
| `aniyomi.show_subtitle_settings()` | `show_panel` | `subtitle_settings` |
| `aniyomi.show_subtitle_delay()` | `show_panel` | `subtitle_delay` |
| `aniyomi.show_audio_delay()` | `show_panel` | `audio_delay` |
| `aniyomi.show_video_filters()` | `show_panel` | `video_filters` |
| `aniyomi.show_software_keyboard()` / `hide_software_keyboard()` / `toggle_software_keyboard()` | `software_keyboard` | `show` / `hide` / `toggle` |
| `aniyomi.set_button_title(text)` | `set_button_title` | text |
| `aniyomi.reset_button_title()` | `reset_button_title` | `unused` |
| `aniyomi.hide_button()` / `show_button()` / `toggle_button()` | `toggle_button` | `hide` / `show` / `toggle` |
| `aniyomi.previous_episode()` / `next_episode()` | `switch_episode` | `p` / `n` |
| `aniyomi.pause()` / `unpause()` / `pauseunpause()` | `pause` | `pause` / `unpause` / `pauseunpause` |
| `aniyomi.seek_by(v)` / `seek_to(v)` | `seek_by` / `seek_to` | v |
| `aniyomi.seek_by_with_text(v, t)` / `seek_to_with_text(v, t)` | `..._with_text` | `v .. "|" .. t` |
| `aniyomi.int_picker(title, fmt, start, stop, step, property)` | `launch_int_picker` | `title|fmt|start|stop|step|property` |
| `aniyomi.left_seek_by(v)` / `right_seek_by(v)` (legacy) | calls `seek_by(-v)` / `seek_by(v)` | |

Note: `aniyomi.set_button_title` writes the raw title even when no button is shown.

## 9. Gesture dispatch for `SingleActionGesture` (cross-reference)

`PlayerSettingsGesturesScreen` stores one `SingleActionGesture` per double-tap side and media control. Runtime dispatch is in area:45 / area:46. Semantics this area depends on:
- `None`: no action.
- `Seek`: seek by `skip_length_preference` seconds (`pref_skip_length_preference`, default 10; 0 means disabled). Double tap left seeks back, right forward.
- `PlayPause`: toggle pause.
- `Switch`: change episode (previous for left/previous-media, next for right/next-media).
- `Custom`: does not run Lua. It sends a key code that `input.conf` may bind. Codes (`PlayerEnums.kt` `CustomKeyCodes`): DoubleTapLeft `0x10001`, DoubleTapCenter `0x10002`, DoubleTapRight `0x10003`, MediaPrevious `0x10004`, MediaPlay `0x10005`, MediaNext `0x10006`.

Proposal for Linux: `Custom` sends the mpv key name `kurobayomi-double-tap-left` (and -center, -right, -media-previous, -media-play, -media-next). Users bind these in `input.conf` (for example `kurobayomi-double-tap-left script-message my-action`). The key names are documented in the settings page info text (replacing the Android key-code text).

## 10. Backup and restore

- Backup creator: `app/src/main/java/eu/kanade/tachiyomi/data/backup/create/creators/CustomButtonBackupCreator.kt` → `getAll()` → `BackupCustomButtons` (`data/backup/models/BackupCustomButtons.kt`), field numbers in `p1-data-and-config.md`.
- Restorer: `app/src/main/java/eu/kanade/tachiyomi/data/backup/restore/restorers/CustomButtonRestorer.kt`:
  1. If the backup list is empty → nothing.
  2. `dbCustomButtons = getAll()`; `byName = associateBy { name }` (later duplicates win in the map); `nextSortIndex = max+1 or 0`; `dbHasFavorite = any isFavorite`.
  3. For each backup button, sorted by backup `sortIndex`:
     - If a DB button with the same **name** exists → skip (no change, not updated).
     - Else insert with `sortIndex = nextSortIndex++`, `isFavorite = backup.isFavorite && !dbHasFavorite` (so at most one favourite is created, and only when the DB has none), content fields copied.
  4. Insert runs in `handler.awaitOneExecutable`, return value discarded.
- Restored buttons take the backup's order, not the backup's raw `sortIndex` values.
- Observed: `dbHasFavorite` is computed once before the loop and never updated, so restoring a backup with two favourite buttons into a database without a favourite inserts **two** favourites (edge E-CB-09). The player then uses the first one in `sortIndex` order.

## 11. Limits and performance

- No limit on the number of buttons, the length of Lua bodies, or the title length.
- Lua is compiled once per player start, all buttons together.
- The screen re-renders the full list on every DB emission.
- Syntax highlight in the button dialog runs on each keystroke, without a size limit.

## 12. Rust port summary (details in `p1-rust-design.md`)

- One `custom_buttons` table, identical columns and seed row. Add a UNIQUE index on `name` only if the product decides names must be unique; the current app enforces it in the dialog only, so keep the dialog check and do not add the index (edge E-CB-10).
- Writes go through a single worker. The mutex in the Kotlin code becomes "the worker runs one job at a time", so reorder and favourite toggle need no lock.
- Delete promotes nothing (keep observed behaviour; add a P2 option "promote the next button").
- Edit never writes sortIndex. Reorder writes once per drop.
- Generated Lua uses per-button `pcall` isolation (proposal, see section 7).
- Bridge: parse the 14 properties into a `LuaCall` enum; invalid numeric input is logged and ignored instead of crashing.
