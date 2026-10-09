# Area 45 (player core, part 1): UI parity

This file covers only the UI that the part-1 scope files render or drive: the `PlayerActivity` window chrome and lifecycle-visible behaviour, the picture-in-picture action set (`PipActions.kt`), the toasts and messages raised from scope files, the external-player hand-off, and the enums that name UI states (`PlayerEnums.kt`). The full controls, gestures, sheets, panels, and the episode list are rendered by part-2 Compose files (`app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/**`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt`) and are specified in the part-2 UI document. Where a part-1 behaviour is visible inside a part-2 control, that is said here.

Source files for this document: `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PipActions.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerEnums.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/ExternalIntents.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerObserver.kt`.

Mockup reference: `src/player.rs` (the placeholder `player()` at line 154, the `reader()` at line 49), `src/main.rs:54-58` (immersive mode for `screen:player`), `spec/reader_player.json`.

---

## 1. Window and surface

| Element | Android (OBSERVED) | Linux (Proposal) | State |
|---|---|---|---|
| Video surface | `binding.player` (`AniyomiMPVView`), full window, `FLAG_LAYOUT_NO_LIMITS` (`PlayerActivity.kt:379-410`) | `gtk::GLArea` fills the window; black background | loading: last frame kept; empty (no file): black; error: black plus error pill |
| Controls layer | `binding.controls` (`ComposeView`) over the surface, hosting `PlayerControls` | `gtk::Overlay` child (part 2) | hidden when `toggle_ui hide`; shown on `show`; `toggle` flips |
| Immersive mode | Hides status and navigation bars, sticky transient bars on swipe | `window.fullscreen()` while playing; leave with Escape or the fullscreen button | fullscreen state kept per window (not persisted) |
| Window title | none (Android task) | `"<anime title> - <episode name>"` (same string as the mpv media title minus the `E<NN>` part; see 4.2 below) | updated on each episode switch |
| Orientation lock | `requestedOrientation` from preference | Dropped (T-45a-035) | not shown |
| Brightness slider | `isBrightnessSliderShown` (part 2) fed by gesture | Dropped (T-45a-045) | not shown |
| Volume slider | `displayVolumeSlider()` on volume keys (`PlayerActivity.kt:857-858`) | Part 2 overlay; trigger is Up/Down arrow keys (T-45a-025) | hide timing is decided in part 2; the general control timeout preference is `pref_player_time_to_disappear` (default 4000 ms, `PlayerPreferences.kt:43`) |

Responsive behaviour: the window has no minimum size from part 1. The controls layer decides its own breakpoints (part 2). Part 1 requires only that the GLArea keeps the video aspect (mpv `panscan` and aspect state from `VideoAspect`, T-45a-034).

## 2. Toasts and transient messages

All toasts use `toast(message)` in Kotlin (`eu.kanade.tachiyomi.util.system.toast`). Linux: a `gtk::Overlay` pill (the mockup's `pill()` at `src/player.rs:40`), auto-dismissed. Duration: short for normal messages, long for errors (Kotlin `Toast.LENGTH_LONG` at `PlayerObserver.kt:448`; Proposal: 5 s).

| Trigger | Message text (English) | Key | Source | Duration (Kotlin) | Proposal |
|---|---|---|---|---|---|
| End of file error without message | `Error: File ended` | literal | `PlayerObserver.kt:441` | LONG | LONG pill, 5 s |
| End of file error with HTTP error | `Error: File ended: <HTTP error text>` | literal | `PlayerObserver.kt:443` | LONG | same |
| Other mpv error message | the mpv error text | | `PlayerObserver.kt:440-449` | LONG | same |
| No next episode (manual) | `Next Episode not found!` | `no_next_episode` | `PlayerActivity.kt:1021-1023` | default | short pill |
| Hoster list empty | `Hoster list is empty` (then window closes) | `no_hosters` | `PlayerActivity.kt:1030-1035` | via error path | error dialog then close |
| Initial episode error | exception message or `Unknown error`, then close | | `PlayerActivity.kt:1143-1151`, `:209-210` | default | error dialog then close |
| HTTP server start failure | `Failed to start http server` | `http_server_start_failure` | `PlayerActivity.kt:1116`, `ExternalIntents.kt:99` | default | error pill |
| Episode title in PiP on episode change | episode title | | `PlayerActivity.kt:1049-1051` | default | mini-player pill (T-45a-048) |
| Picture saved | `Picture saved` | `picture_saved` | `PlayerActivity.kt:1247` | default | short pill |
| Share message (text in share sheet) | `<anime>: <episode>, <seconds>` | `share_screenshot_info` | `PlayerActivity.kt:1235` | n/a | share dialog body |
| Cover updated | `Cover updated` | `cover_updated` | `PlayerActivity.kt:1264` | default | short pill |
| Background updated | `Background updated` | `background_updated` | `PlayerActivity.kt:1265` | default | short pill |
| Thumbnail updated | `Thumbnail updated` | `thumbnail_updated` | `PlayerActivity.kt:1266` | default | short pill |
| Add to library first | `Please add the entry to your library before doing this` | `notification_first_add_to_library` | `PlayerActivity.kt:1268` | default | short pill |
| Cover update failed | `Failed to update cover` | `notification_cover_update_failed` | `PlayerActivity.kt:1269` | default | short pill |
| External player: no URL | `Video URL is empty.` | literal | `ExternalIntents.kt:141` | default | error pill |
| External player: no episode | `Cannot open episode` (fallback text) | literal | `ExternalIntents.kt:191` | default | error pill |
| External player: video list empty | `Video list is empty` (exception, shown with `e.message`) | literal | `ExternalIntents.kt:91`, `MainActivity.kt:626` | default | error pill |

Silent (log only): screenshot save error (`PlayerActivity.kt:1250`), `Error getting links` (`PlayerActivity.kt:1046`), metadata parse failure (`PlayerActivity.kt:1333`), torrent failures (no catch: behaviour 13.2, UI gap), tracker update failures (INFO log, `ExternalIntents.kt:560`).

## 3. Loading indicator

| Signal | Source | Visual (Kotlin, part 2 renders) | Linux |
|---|---|---|---|
| `isLoading = true` | `MPV_EVENT_SEEK`, `seeking=yes`, `paused-for-cache=yes`, `changeEpisode` | spinner shown when `pref_show_loading` is true (default true, `PlayerPreferences.kt:24`); exact placement is decided in part 2 | `gtk::Spinner` on the overlay, hidden when false |
| `isLoadingEpisode` | `updateIsLoadingEpisode` | part 2 decides the visual | spinner plus title pill |
| `isLoadingHosters` | `updateIsLoadingHosters` | part 2 decides the visual | part 2 |
| `isLoadingTracks` | `setupTracks` | none (flag only) | none |

Loading state transitions (behaviour 4.1, 6.4, 7.1):

- Entering: seek event, cache pause, episode switch.
- Leaving: `pause` flag event is not a loading change; `seeking=no` and `paused-for-cache=no` clear it; `FILE_LOADED` then `PLAYBACK_RESTART` (note: `isExiting` only, not loading) normally clear the episode loading flag through part 2.

## 4. Title and placeholders

### 4.1 Mini-player and window title

Android PiP title and subtitle come from `setTitle(anime.title).setSubtitle(episode.name)` (`PlayerActivity.kt:770-772`, API 33+). Linux window title: `"<anime title> - <episode name>"`.

### 4.2 mpv on-screen title (force-media-title)

`"<anime title> - E<NN> - <episode name>"` (`AYMR.strings.mpv_media_title`, behaviour 4.4). Visible in mpv's own OSD and in `mpv`'s own window if an external player is used. The GTK window does not need to repeat it.

## 5. Picture-in-picture actions (`PipActions.kt`)

The four Android remote actions and their UI order:

| Position | Android icon | Title key | Control code | Enabled when | Linux mini-player equivalent |
|---|---|---|---|---|---|
| 1 | `ic_skip_previous_24dp` if `replaceWithPrevious`, else `ic_forward_10_24dp` | `action_previous_episode` ("Previous episode") or `pref_skip_10` ("10s") | `PIP_PREVIOUS` (3) or `PIP_SKIP` (5) | previous: `playlistPosition != 0`; skip: always | previous button (disabled at index 0) or skip +10 s button |
| 2 | `ic_play_arrow_24dp` when paused, `ic_pause_24dp` when playing | `action_play` ("Play") or `action_pause` ("Pause") | `PIP_PLAY` (2) or `PIP_PAUSE` (1) | always | play/pause toggle button, icon swaps |
| 3 | `ic_skip_next_24dp` | `action_next_episode` ("Next episode") | `PIP_NEXT` (4) | `playlistPosition != playlistCount - 1` | next button (disabled at last episode) |

Note: the Android code creates the actions with `PendingIntent.getBroadcast` and request codes equal to the control code. The Linux mini-player uses direct callbacks; the code numbers stay in the enum for the MPRIS/mini bridge and for the `pip_control` action name (`PIP_INTENTS_FILTER = "pip_control"`).

Requirements for the mini-player (P2, T-45a-048):

- Small window, fixed aspect of the video, always above the main window (Proposal: GTK transient-for hint; Wayland has no always-on-top protocol, so this is best-effort and documented).
- Shows the four controls above in the order listed; hides the rest of the player UI.
- Closing the mini-player closes playback when the main window is closed (`onPause` close path, behaviour 1.6).
- `pref_enable_pip` gates the feature; `pref_pip_on_exit` triggers the mini-player when the main window is closed while playing (behaviour 1.3 step 8, 1.9).

## 6. Keyboard and mouse (Proposal: desktop replacement)

Android input (`PlayerActivity.kt:854-884`, `AniyomiMPVView.kt:171-208`) and the Linux replacement:

| Input | Android action | Linux action (Proposal) |
|---|---|---|
| Volume Up / Down keys | volume +1 / -1, show slider | Arrow Up / Down (T-45a-025) |
| D-pad Left / Right | left / right double-tap action | Arrow Left / Right: left / right double-tap action (part 2 default: seek, `pref_left_double_tap` = `Seek`) |
| Space | `pauseUnpause()` | Space: pause or unpause |
| Media Stop | `finishAndRemoveTask()` | `XF86AudioStop` closes the window (MPRIS `Stop`) |
| Media Rewind / Fast-Forward | left / right double-tap action | `XF86AudioRewind` / `XF86AudioForward` |
| Other keys | forwarded to mpv `keydown`/`keyup` with modifiers | forwarded with the table in `p1-rust-design.md` section 8 |
| Back button | PiP entry if conditions hold, else finish | Escape: leave fullscreen if fullscreen, else close (no PiP) |
| Touch gestures (double tap, drag) | part 2 | Mouse double-click: toggles fullscreen (Proposal, part 2 owns gesture mapping) |
| Mouse wheel | none in part 1 | Proposal: wheel = volume (part 2 decides final mapping) |

Note: `KEYCODE_MEDIA_STOP` closes the window (`finishAndRemoveTask`). On Linux the window is closed through `window.close()`, which runs the same teardown (behaviour 1.8).

## 7. Media controls (MPRIS)

Visible to the desktop shell (KDE Plasma media widget, lock screen), not inside the app. Metadata: title = episode name, artist/album = anime title (Proposal), art = thumbnail (part 2 provides the image). Playback status: `Playing` / `Paused` / `Stopped`. Controls: Play, Pause, Next, Previous (Previous and Next behave per `SingleActionGesture` mapping, behaviour 7.4).

## 8. Keep-awake and screen state

- While playing: idle inhibit (no screen blank or lock). Released on pause or close (T-45a-041).
- The inhibit reason string: `Playing video` (Proposal).
- Desktop shell: KDE shows the inhibit in the power widget. No additional UI in the app.

## 9. Share and screenshot dialogs

- Screenshot: triggered from part 2 controls and the `screenshot` sheet (`spec/reader_player.json`). On success the picture is saved and a `Picture saved` pill is shown. The saved path is offered in the pill's action (Proposal: "Open folder").
- Share: `share_screenshot_info` text plus the image, through the XDG Desktop Portal (`ashpd`, `OpenURI` or `Email` share, Proposal) or a `gtk::FileLauncher` fallback. Chooser title `Share`.
- Set as cover / background / thumbnail: results only (section 2). The action itself is part 2.

## 10. Enum-driven UI states (`PlayerEnums.kt`)

| Enum | UI state it drives | Part-1 behaviour | Linux widget |
|---|---|---|---|
| `Sheets` | which bottom sheet is open | cleared on episode change (`PlayerActivity.kt:1003`) and on PiP entry (`:811`) | sheet host (part 2) |
| `Panels` | which side panel is open | cleared on episode change (`:1004`) | panel host (part 2) |
| `Dialogs.IntegerPicker` | integer picker dialog | created by `launch_int_picker` (behaviour 5) | `gtk::Window` modal dialog with `gtk::SpinButton` (min, max, step) and title; name format `%d` substitution |
| `Dialogs.EpisodeList` | episode list dialog | part 2 | part 2 |
| `PlayerUpdates.ShowText` | OSD text pill | from `show_text` bridge call | pill, duration decided by the part-2 model |
| `VideoAspect` | aspect button label (`Cropped to screen`, `Fit to screen`, `Stretched to screen`) | set on configuration change (`PlayerActivity.kt:652`) | GLArea aspect, part 2 label |
| `SingleActionGesture` | label (`None`, `Seek`, `Play/Pause`, `Change episode`, `Custom`) | media mapping (behaviour 7.4) | settings label (part 2) |
| `PlayerOrientation` | orientation picker | Dropped | not shown (T-45a-035) |
| `Decoder` | decoder picker (`Auto`, `SW`, `HW`, `HW+`) | `hwdec` change (behaviour 6.1 `hwdec`) | settings label (part 2); Linux values in `p1-rust-design.md` section 6 |
| `Debanding` | debanding picker | `vf` / `deband` (behaviour 2.3) | settings (part 2) |
| `VideoFilters` | filter sliders (`brightness`, `saturation`, `contrast`, `gamma`, `hue`) | runtime property writes | `VideoFilters` panel (part 2) |
| `AudioChannels` | channel picker | property write (behaviour 9.1) | settings (part 2) |

## 11. Custom buttons (rendered by part 2, invoked from part 1)

- Each `CustomButton` appears as a button in the controls (part 2). Tap runs `call_button_<id>` through mpv `script-message`; long press runs `call_button_<id>_long` (generated Lua, behaviour section 2.1 / task T-45a-021).
- Button label: `set_button_title` / `reset_button_title` bridge calls change it at runtime (behaviour 5).
- Show/hide: `toggle_button` bridge calls.
- Default seeded button `+85 s` (data doc 2.8): tap seeks by the intro length, long press opens an integer picker titled `Change intro length` with name format `%ds`, range 0 to 255, step 1.

## 12. Stats overlay

When `pref_player_statistics_page` is not 0, mpv's stats overlay is enabled on startup (behaviour 2.8). Visible as mpv OSD text, not as a GTK widget. Linux: same, via mpv script bindings (T-45a-053).

## 13. Desktop-only additions (Proposal)

- Window keeps its size and position across episodes.
- Close with the window manager close button runs the same teardown as Back (behaviour 1.6).
- A `Ctrl+Q` quits the app from the player (app-wide shortcut, area 36).

## 14. Gap list: what the mockup lacks (`src/player.rs`, `src/main.rs`)

The mockup `player()` (`src/player.rs:154-330`) is a UI-only placeholder with fixed values (`duration = 1420.0`, `"Title 1"`, `"Episode 1"`). Missing for this scope:

1. No libmpv surface. The `surface` GBox is a plain `osd` class box with a comment "placeholder surface; swap for an mpv/GStreamer widget" (`src/player.rs:159`).
2. No state. Position, duration and pause are not driven by any model; the progress bar is not connected.
3. No keyboard handling. There is no `EventControllerKey`, no mapping of keys to mpv, and no volume/space/arrow behaviour.
4. No loading spinner, no loading state, no error state. The error and empty states for a missing file or failed load do not exist.
5. No toasts with real messages. `pill()` exists but no message keys are wired.
6. No episode switching: the Previous and Next buttons call no logic.
7. No fullscreen behaviour (the `screen:player` immersive logic in `src/main.rs:58` only hides the sidebar).
8. No idle inhibit, no MPRIS, no mini-player.
9. No external-player hand-off or its message paths.
10. No share, screenshot, or set-as-art result messages.
11. No window title from anime/episode.
12. Dialog and sheet IDs exist in `spec/reader_player.json`, but the integer picker dialog and its name-format substitution are not implemented.

## 15. States matrix (summary)

| Screen state | Trigger | What the user sees (Linux target) |
|---|---|---|
| Loading episode | open or switch | black surface, centred spinner, title pill with episode name |
| Playing | `pause=false` | video, controls auto-hide after `pref_player_time_to_disappear` (part 2) |
| Paused | `pause=true` | video frame held, controls visible |
| Buffering | `paused-for-cache=yes` | spinner over the last frame |
| Seeking | `seeking=yes` or `MPV_EVENT_SEEK` | spinner until `seeking=no` |
| Error (load) | initial error path | error pill then window close |
| Error (mpv) | `efEvent` | error pill; the window stays open |
| Empty playlist | no next or previous | Next/Previous disabled (mini-player), `Next Episode not found!` toast on manual next |
| Hoster list empty | switch | `Hoster list is empty` error, then close |
| External player running | external launch | the GTK window closes or stays closed; no embedded playback |
| Mini-player | PiP replacement | small window with four controls |
| Fullscreen | `player_fullscreen` true, or the fullscreen button | window fullscreen |
