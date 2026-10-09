# P1 UI parity: 48-settings-player-and-scripts

This area renders UI (Compose screens, dialogs, menus). This file lists every visible element and state of the Kotlin app, the desktop equivalent, and what the current mockup lacks. Source paths are repo-relative to `/home/blackbox/aniyomi-src`. Mockup paths are relative to `/home/blackbox/kurobayomi`.

Mockup baseline (OBSERVED): `spec/settings.json` describes the screens (`settings_player*`, `torrent_notice`) with item lists. `src/settings.rs` builds them. For this area: `settings_player_custom_buttons` is placeholder `cards(id, &["Skip opening", "Toggle shader"], true)`; `settings_player_editor` is `editor()` with three hard-coded files and a `Scripts / Script options` dropdown; `settings_player_code_editor` is `code_editor()` with a fixed sample text, no line numbers, no highlighting, no unsaved-change guard, and Save enabled only after a change. The mockup has no real data for any of these.

---

## 1. Global conventions for the desktop port

- Toolkit: GTK 4.16 without libadwaita. Use `gtk::HeaderBar`, `gtk::ListBox` (style class `boxed-list`), `gtk::Stack`, `gtk::Window` modals, `gtk::AlertDialog` for plain confirmations.
- Theme: inherit the user's GTK theme (Tokyo Night in the user's other apps). Do not hard-code colours on these pages. The highlight palette (`githubTheme`) is the only fixed colour set (see highlight topic).
- Minimum window width 360 px (phone-like narrow layout is still supported for small windows); layout must not produce horizontal scrolling except inside the code editor.
- Keyboard: Tab order follows reading order; Enter activates the default button of a dialog (OK, when enabled); Escape closes dialogs and cancels; Ctrl+S saves in the code editor; Alt+Left or the header back button pops a page.
- Mouse: every row and button is clickable; right click on a list row opens the same menu as the overflow button.
- Titles and strings: English text from `p1-behavior.md` (keys listed there). No new strings are invented.

---

## 2. Screen by screen

### 2.1 Player settings root (`PlayerSettingsMainScreen`, `PlayerSettingsScreen`)

Visible elements, top to bottom:
- Header bar: back button (if not the top-level), title "Player" (when opened from main settings) or "Player settings", search button (icon `system-search-symbolic`, tooltip "Search").
- Nine rows, each: leading icon, title (bold), subtitle (dim, up to one line), chevron (`go-next-symbolic`). Icons (freedesktop names, proposal): Internal player `media-playback-start-symbolic`, Gestures `input-touchpad-symbolic`, Decoder `video-display-symbolic`, Subtitles `media-view-subtitles-symbolic` (fallback `format-text-italic-symbolic`), Audio `audio-speakers-symbolic`, Custom buttons `utilities-terminal-symbolic`, Script editor `document-edit-symbolic`, Torrent `network-workgroup-symbolic` (fallback `folder-download-symbolic`), Advanced `applications-engineering-symbolic`.
- Subtitles: exactly the strings in `p1-behavior.md` section 1 table.

States:
- Two-pane (wide window, >= 720 px): sidebar of the nine rows on the left, page on the right, selected row highlighted with the list's selected style (`:selected`). Initial selection: Internal player.
- Narrow (< 720 px): the list is the page; selecting a row pushes the page with a back button.
- Selection persists within the session only.

Keyboard: Up/Down move selection; Enter opens (narrow) or focuses the page (wide).

Mockup gap: the spec has no item for search and no sidebar layout; there is no selection state.

### 2.2 Internal player (`PlayerSettingsPlayerScreen`)

Groups with headers (group title as a dim label above a boxed list):
1. Ungrouped: Progress (dropdown row, 7 options), Preserve watch position (switch), Orientation (dropdown, 8 options). **Orientation row is removed on desktop** (T-48a-019).
2. Controls: Allow gesture in panels, Show loading circle, Show current chapter indicator (subtitle "Only appears if the video has chapters"), Remember and switch to the last used brightness, Remember and switch to the last used volume (**not shown on desktop**: the Kotlin switch has no reader, see T-48a-052).
3. Hosters: Display failed hosters, Display empty hosters.
4. Display: Show content in display cutout (**renamed on desktop to "Fullscreen player when opening"**, see T-48a-014), Hide player controls when opening the player, Display volume as percentage, Show system status bar with controls (**removed**), Reduce player animation, Hide player controls time (dropdown 500..5000 ms, subtitle "%1$s ms" → "4000 ms"), Panel opacity (slider 0..100, value shown as percentage "60%").
5. Intro skip: Enable intro skip; Enable auto skip (greyed when intro skip off); Enable Netflix style (greyed); Button timeout (dropdown 5..10 seconds, greyed); Enable AniSkip (greyed); Disable AniSkip if video already contains chapters (greyed unless intro skip and AniSkip on); info text "AniSkip requires the anime to be tracked with MAL or Anilist to work" (greyed when intro skip off).
6. Picture-in-Picture: shown only when the desktop probe says the feature is available (T-48a-017, T-48a-054); rows: Enable the use of PiP mode, Show episode toasts ..., Automatically switch to PiP mode on exiting the player, Replaces the "Skip 10 seconds" option with "Previous episode" (greyed when PiP off).
7. External player: Always use external player (switch); External player preference (dropdown: "None" plus detected apps; list built from MIME handlers).

States:
- Greyed (not hidden) when a parent switch disables children. Kotlin hides them (`AnimatedVisibility`). Desktop deviation, recorded here so the reviewer can decline it.
- Dropdown with a stored value not in the option list shows "null" in Kotlin; desktop shows the raw stored string in italics with "(unavailable)" appended. (Edge E-UI-03.)
- No loading state (values are synchronous).

Mockup gap: the mockup's `settings_player_internal` notes describe the groups but the rows are static text; no switch state, no greyed logic, no PiP gating, no external player list.

### 2.3 Gestures (`PlayerSettingsGesturesScreen`)

Groups: Sliders (2 switches), Seeking (horizontal seek switch with subtitle "Swipe left or right to rewind or fast-forward", show seekbar switch, Default skip intro length row with subtitle "85s", Double tap to skip length dropdown (30s, 20s, 10s, 5s, 3s, Disable), Enable precise seeking switch with subtitle), Double tap (3 dropdowns + info text), Media controls (3 dropdowns + info text).

Dialog "Skip intro length" (from the Default skip intro length row):
- Title "Skip intro length". Body: a vertical wheel-like picker of 0..255, each entry "%ds" (`0s`…`255s`), current value selected on open. Desktop: `gtk::SpinButton` (0..255, step 1, digits 0) with a suffix label "s", or a `gtk::DropDown` with 256 entries. Use SpinButton with keyboard entry (wheel and arrow keys).
- Buttons: Cancel (closes), OK (saves).
- Default focus: OK. Enter = OK. Escape = Cancel.

Dropdown labels: None, Seek, Play/Pause, Change episode, Custom (per action), Disable (skip length 0).

Info texts (multi-line dim labels, no icon): double tap info and media info (text in behaviour section 4).

States: none special. Changing a dropdown applies immediately (no OK).

Mockup gap: values are static; the intro-length dialog is absent; no per-side option filtering (the allowed lists differ by slot).

### 2.4 Decoder (`PlayerSettingsDecoderScreen`)

Rows: Try hardware decoding (switch), Enable gpu-next (switch, subtitle "A new video rendering backend"), Debanding (dropdown None/CPU/GPU, raw names), Use YUV420P pixel format (switch with two-line subtitle).

Desktop adds one note row (info, dim): "Changes apply the next time a video starts." (Proposal; the Kotlin app does not show it. Required because mpv options are set at player start.)

Mockup gap: no note; values static.

### 2.5 Subtitles (`PlayerSettingsSubtitleScreen`)

Rows: Preferred languages (edit row; subtitle is the stored value, shown as text; empty → subtitle blank); Whitelist (edit, no validation); Blacklist (edit, no validation).

Dialog (edit with dialog subtitle):
- Title: row title (e.g. "Preferred languages"). Dialog subtitle line (dim, under title) = `pref_player_subtitle_lang_info` text.
- Entry, single line, with a trailing icon: error icon (`dialog-error-symbolic`) when invalid, otherwise a clear button (`edit-clear-symbolic`) that empties the field.
- Supporting text under the entry: validation error ("Invalid code: \"xx\"") in the error colour when invalid; empty otherwise.
- Buttons: Cancel, OK. OK enabled iff the text differs from the stored value and is valid (and not blank, unless the field allows blank, which for these fields it does). Note that for the Whitelist/Blacklist fields validation always passes.
- Entry activates OK on Enter when enabled.

States: invalid (error icon, error text, OK disabled); valid; unchanged (OK disabled).

Mockup gap: no edit dialogs in the mockup for these rows.

### 2.6 Audio (`PlayerSettingsAudioScreen`)

Rows: Preferred languages (same dialog as above), Enable audio pitch correction (switch with two-line subtitle), Audio channels (dropdown: Auto, Auto-safe, Mono, Stereo, Reverse stereo), Volume boost cap (slider 0..200, value shown in subtitle as the number, e.g. "30").

Slider state: while dragging, the subtitle updates live; the value is saved on release (proposal; Kotlin saves on every change).

Mockup gap: slider is static; no language validation.

### 2.7 Advanced (`PlayerSettingsAdvancedScreen`)

Rows: Use user MPV files (switch, subtitle "Use files in mpv-config directory"), Edit MPV configuration file for further player settings (edit row, multi-line; subtitle = first two lines of text, plus "..." if more), Edit MPV input file for keyboard mapping configuration (same).

Multi-line editor dialog: title = row title; body = `gtk::TextView` monospace, min height 240 px, max height 60% of window; Cancel and OK (OK enabled iff changed; empty allowed).

Proposal extra row: "Open mpv-config folder" (action row; opens the directory in the file manager via `gio::AppInfo::launch_default_for_uri`). Not in Kotlin; explain in the task (T-48a-035).

Mockup gap: no editors; no folder action.

### 2.8 Torrent (`PlayerSettingsTorrentScreen`)

Rows, in order: Enable TorrServer (switch); TorrServer port (edit, numeric, subtitle "8090"); Torrent trackers (multi-line edit, subtitle first two lines + "..."); Reset default torrent trackers string (action, greyed unless changed); Proxy mode (dropdown None/Tracker/Peers/Full); Proxy url (edit, greyed unless proxy mode != None).

Dialogs:
- Legal notice (first enable): title "Legal Notice"; two paragraphs; OK. Shown once.
- Port dialog: subtitle "Enter 0 to let system choose a port"; numeric entry; error "Port must be a number between 0 and 65535"; OK disabled when invalid or unchanged.
- Trackers dialog: multi-line entry with the default list visible as text; OK; the Reset row restores it.
- Proxy URL dialog: subtitle "Supports http, https, socks4, socks4a, socks5, socks5h"; errors "Invalid uri" and "Unsupported protocol: \"%s\"".

States: greyed rows when the server is off (Kotlin hides them: same deviation as section 2.2). Server running/stopped status is not shown in Kotlin; desktop proposal: show a single dim status line under the switch ("Running on port 8090" / "Stopped" / "Failed to start: <error>"). Kotlin does not have this. Mark as addition.

Mockup gap: `torrent_notice` is in the spec but the rows are static.

### 2.9 Custom buttons (`PlayerSettingsCustomButtonScreen`, `CustomButtonScreen`, `CustomButtonListItem`, `CustomButtonDialogs`)

Header: back, title "Edit custom buttons", help button (tooltip "Custom buttons guide", opens the aniyomi docs URL in Kotlin; desktop: opens the same URL via `gtk::UriLauncher`, or a local page; decision in T-48a-049).

Body:
- Empty state: centred text "You have no custom buttons. Tap the plus button to create a custom button." No illustration.
- List: one card per button, vertical spacing 8 dp (proposal 8 px), card padding per Kotlin (small/medium).
  - Row 1: drag handle (`list-drag-handle-symbolic`), name (ellipsised, expands), star toggle (`starred-symbolic` when favourite, `non-starred-symbolic` otherwise; colour `#FDD835` for the star in both cases per Kotlin), edit (`document-edit-symbolic`, tooltip "Edit"), delete (`user-trash-symbolic`, tooltip "Delete").
  - Row 2: Lua content preview, dim (75 % opacity), at most 3 lines, ellipsis, monospace.
  - Whole card click: edit.
- FAB: `list-add-symbolic`, label "Add" for accessibility, bottom-right, suggested-action style.

Drag and drop:
- Drag starts from the handle; a placeholder row shows the drop position; the list updates visually during the drag; on drop, one reorder write happens (desktop proposal; Kotlin writes on every step).
- While dragging, DB refreshes do not replace the list (Kotlin `isAnyItemDragging` rule).
- Keyboard reorder: focus a card, press Ctrl+Up / Ctrl+Down (proposal; Kotlin has no keyboard reorder).

Create/edit dialog (window, 640 px wide, modal):
- Title: "Add button" or "Edit button". Edit shows "(ID: n)" right-aligned, dimmed.
- Fields in order: Title (single line, label "Title", supporting text "*required" or "Button already exists!" in error style), Lua code (label "Lua code", "*required", 3–5 lines), Lua code (on long press) (label "Lua code (on long press)", "(optional)", 3–5 lines), On startup (label "On startup", "(optional)", 2–4 lines).
- Monospace font for the three Lua fields, Lua syntax highlight (same highlighter as the editor, no size limit in dialogs).
- Buttons: Cancel; confirm "Add" or "Edit" (enabled iff title non-empty, Lua code non-empty, and title not duplicated).
- Initial focus: Title; Enter in Title moves to Lua code (no submit); Ctrl+Enter submits when enabled.

Delete dialog: title "Delete button", body `Do you wish to delete the button "<name>"?`, OK and Cancel. Destructive style on OK (`destructive-action`).

Toast on internal error: "InternalError: Check crash logs for further information" (Kotlin text). Desktop proposal: keep the text.

States: Loading (not visible, data is synchronous from the worker; show a spinner only if the first load exceeds 300 ms); Empty; Success; Error (if the DB cannot be read: centred message with the error text and a Retry button — Kotlin shows nothing for this state in the screen).

Mockup gap: placeholder cards; no star, no reorder, no dialogs, no validation, no empty state.

### 2.10 Script editor (`PlayerSettingsEditorScreen`, `EditorScreen`, `EditorDialogs`, `EditorTypeDropdown`)

Header: back; title is a dropdown button with the current type name ("Scripts" or "Script options") and a down arrow; the popover lists the two types.

Body:
- Empty state: "Directory contains no items. Tap the plus button to add a new file."
- File rows: leading icon (Lua: `text-x-script-symbolic`; conf: `emblem-system-symbolic` or `preferences-system-symbolic`; other: `text-x-generic-symbolic`), headline = file name, supporting line = "<size> - <modified>" (for example "1.23 KB - October 8, 2026 14:05"; omitted parts are dropped with the separator); trailing overflow button (`view-more-symbolic`) opening a menu with "Rename file" and "Delete file"; row click opens the code editor.
- FAB: `list-add-symbolic`, tooltip "Add".

Dialogs:
- Create file: title "Create file"; entry labelled "Filename" with initial text `file.lua` or `file.conf`, base name pre-selected; error line under the entry ("File already exists" or "Invalid filename") in error style with an error icon; OK labelled "Add" (enabled only when valid).
- Rename: title "Edit filename"; entry initially the current name; OK labelled "Edit".
- Delete: title "Delete file"; body `Do you wish to delete the file "<name>"?`; OK, Cancel.

Toasts: "Unable to create file", "Unable to rename file", "Unable to delete file" on failure.

States: list loaded; empty; (no loading flicker; listing is quick); error on listing (desktop only: "Unable to read the folder: <error>" with Retry).

Mockup gap: hard-coded files, the rename/delete menu exists but is not backed, the dropdown is a plain `gtk::DropDown` with two strings, no empty state, no size/date labels, no validation.

### 2.11 Code editor (`CodeEditScreen`)

Header: back, title = file name (not the path), actions: Save (icon `document-save-symbolic`, tooltip "Save (Ctrl+S)", sensitive only when modified).

Body:
- Full-height text view with a line-number gutter (monospace, dim), current line number highlighted (desktop addition: `gtk::SourceView` highlights the current line; off by default in Kotlin).
- Syntax colours (Lua and conf rules).
- Horizontal scrolling for long lines; no wrapping.
- Cursor colour: theme accent (`primary`).

States:
- Loading (first open): spinner centred.
- Error: centred text with the exception message (Kotlin: "Unknown exception" if no message). Plus a Back button.
- Modified: Save enabled; header subtitle "Unsaved changes" (desktop addition; Kotlin only enables Save).
- Saved: toast "File saved".
- Save failure: toast with the error message (Kotlin: exception message or "Unable to save file").
- Large file (> 15000 characters): no colours; a dim info bar "Syntax highlighting is off for files over 15000 characters." (desktop addition; Kotlin has no message).

Dialog on back with unsaved changes: title "Warning" (`label_warning`), body "You have unsaved changes. Do you still wish to go back?", OK (leave), Cancel (stay). Window close (X) and Alt+Left trigger the same guard.

Keyboard: Ctrl+S saves; Ctrl+Z / Ctrl+Shift+Z undo/redo (GtkSourceView undo manager; Kotlin BasicTextField has undo on Android IME only — add).

Mockup gap: no save state logic, no unsaved guard, no line-number gutter alignment, no highlight, text static, no error state.

---

## 3. Snackbars, toasts and messages

Kotlin uses `Context.toast` (Android Toast, short duration). Desktop: an in-window toast at the bottom centre, shown for 2 seconds (Android LENGTH_SHORT), one at a time (newer replaces older). Messages used in this area:

| Key | Text |
|---|---|
| `editor_create_error` | Unable to create file |
| `editor_rename_error` | Unable to rename file |
| `editor_delete_error` | Unable to delete file |
| `editor_save_error` | Unable to save file |
| `editor_save_success` | File saved |
| `internal_error` | InternalError: Check crash logs for further information |

Custom buttons emit only `internal_error`. Torrent and language validation use inline errors, not toasts.

---

## 4. Responsive behaviour

- Width >= 720 px: two-pane root (section 2.1).
- Width < 720 px: single pane; back button in header.
- Dialogs: fixed max width 640 px; full width below 480 px.
- Code editor: fills the window; gutter width is 12 px per digit of the largest line number (Kotlin: 12 dp per digit).
- Min window size 360 x 480 px.

---

## 5. Animations

- Kotlin: `expandVertically + fadeIn` / `shrinkVertically + fadeOut` when an item becomes enabled/disabled; `animateItem()` in lists; reorder animates.
- Desktop: use `gtk::Revealer` with `RevealerTransitionType::SlideDown` (duration 200 ms) for dependent groups that are hidden (none in desktop default since we grey instead); list reorder has no animation (GTK ListBox does not animate); page changes use `gtk::Stack` `SlideLeftRight` 200 ms.
- Respect reduced motion: when `gtk-enable-animations` is false, set transitions to `None`. This is the desktop equivalent of the `pref_reduce_motion` concept for the app UI; `pref_reduce_motion` (player animation) itself is an mpv/OSD setting handled in area:46.

---

## 6. Mockup gap summary

| Area | Mockup has | Missing |
|---|---|---|
| Root | nine rows in spec | sidebar layout, selection, search action |
| Internal | static rows with notes | switch state, greyed logic, PiP gate, external list, dropdown values, slider |
| Gestures | static rows | intro-length dialog, per-side option filter, live values |
| Decoder | static rows | note row, live values |
| Subtitles | static rows | edit dialogs, validation |
| Audio | static rows | edit dialog, slider live, validation |
| Advanced | static rows | multi-line editors, folder action |
| Torrent | spec entry `torrent_notice` | all rows, port/trackers/proxy dialogs, status line |
| Custom buttons | placeholder cards | real list, star, reorder, dialogs, empty state, error state |
| Script editor | `editor()` with fixed files | real listing, create/rename/delete, validation, labels, empty state |
| Code editor | `code_editor()` sample text | load/save, modified state, unsaved guard, highlight, gutter, error state, large-file message |

---

## 7. Accessibility

- All icon-only buttons have tooltips and accessible labels (GTK `set_tooltip_text` plus `update_property(Label)`).
- Star toggle exposes its state (`pressed` toggle button semantics).
- Validation errors are announced: the error label is linked with `set_accessible_description` on the entry.
- Colour is never the only cue: invalid entries show an icon and text; greyed rows keep their text readable (opacity no lower than 0.55 of foreground).
- Highlight colours are only decoration; text content is complete without them.
