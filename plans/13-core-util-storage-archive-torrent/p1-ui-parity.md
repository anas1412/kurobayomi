# Area 13 — UI parity (only the UI that this area's code drives)

This area has no Compose screens of its own. Four pieces of user-visible behaviour come from its code: the transient toast (`ToastExtensions.kt`), the torrent settings rows and dialogs whose validators and state rules live in `TorrentPreferences.kt` and the settings screen, the torrent server status shown as a notification, and the reader page background painted from `ImageUtil.chooseBackground`. Everything else in this area is headless. Screen-level layout belongs to the screen areas; this file records only what this area's code decides.

Sources: `core/common/src/main/java/eu/kanade/tachiyomi/util/system/ToastExtensions.kt`, `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/PlayerSettingsTorrentScreen.kt` (screen that reads the validators), `app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt` (notification), `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt` (background).

## 1. Toast (ToastExtensions.kt)

### 1.1 Visible elements and states

- A single short text line, no icon, no action button, no dismiss control (Android system toast).
- Durations: short = Android `LENGTH_SHORT` (about 2 s), long = `LENGTH_LONG` (about 3.5 s). The Kotlin `toast(...)` defaults to short.
- The text is either a string resource (`toast(StringResource)`, localised with `stringResource`) or a raw string (`toast(text: String?)`, `null` shows an empty toast). An empty string still shows the toast box.
- The `block: (Toast) -> Unit` customisation runs before `show()`; no caller in this area passes a custom block.
- Callers in other areas show toasts too; the one verified in this area's review is the HTTP server start failure (`http_server_start_failure`, `app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:1116`). The other callers are owned by their screens' areas.

### 1.2 Desktop equivalent (decision)

Proposal: an in-app overlay (not an OS notification, which would be noisy for routine messages):
- Placed at the bottom centre of the main window, above the bottom edge by 24 logical px, max width 480 px, text wraps at 2 lines and then ellipsises.
- Dark rounded box using the Tokyo Night colours already in the mockup theme (background surface token, foreground text token); no libadwaita widget (GTK 4 only, per the project rule).
- Short = 2000 ms, long = 3500 ms, fade out over 200 ms after the duration.
- One toast at a time: a new message replaces the visible one and restarts the timer.
- Click dismisses. Escape dismisses when the window has focus.
- Not focusable (does not take keyboard focus from the reader or player).
- Screen readers: the label has `accessible-role = status` so the message is announced.

Task T-13a-053.

### 1.3 Gaps in the mockup

The mockup has no toast. Screens show errors as inline labels. Add `src/toast.rs` and call it from the screens that currently show nothing (the torrent settings validators, the storage screen delete result).

## 2. Torrent settings (the rows whose behaviour this area defines)

Source screen: `PlayerSettingsTorrentScreen.kt` (title `pref_player_torrents`, "Torrent"). The spec row names are in `spec/settings.json` ("settings_player_torrents", "Torrent trackers", "Reset default torrent trackers string", the notice dialog `torrent_notice`).

### 2.1 Rows, in order, with their enabled state

| # | Row | Type | Enabled when | Value shown | On change |
|---|---|---|---|---|---|
| 1 | Enable TorrServer (`pref_player_torrents_enable`) | switch | always | on/off | on first enable (`pref_torrserver_shownotice` false): show legal notice and set shownotice true. On disable: stop the server. |
| 2 | TorrServer port (`pref_player_torrents_port`) | text dialog, numeric keyboard | torrents on | the port string | validated (section 5.1); summary in dialog: "Enter 0 to let system choose a port" |
| 3 | Torrent trackers (`pref_player_torrents_trackers`) | multi-line text dialog | torrents on | first two lines, then `\n...` when there are more than two lines | on change: stop the server (the new list applies at the next start) |
| 4 | Reset default torrent trackers string (`pref_player_torrents_trackers_reset`) | text action | torrents on **and** value differs from default | no value | deletes the preference (default applies) |
| 5 | Proxy mode (`pref_player_torrents_proxy_mode`) | list (None, Tracker, Peers, Full) | torrents on | label of current mode | none (applies at next start) |
| 6 | Proxy url (`pref_player_torrents_proxy_url`) | text dialog with subtitle | torrents on **and** proxy mode is not None | the URL | validated (section 5.2) |

Row 3 subtitle exact rule (`PlayerSettingsTorrentScreen.kt:113-119`): `trackers.lines().take(2).joinToString("\n", postfix = if (trackers.lines().size > 2) "\n..." else "")`.

Row 4 enabled rule (`:128-130`): `torrentEnable && trackersPref.get() != trackersPref.defaultValue()`.

### 2.2 Legal notice dialog

- Shown once, the first time the switch goes on (`shownNotice` false).
- Title: "Legal Notice" (`pref_player_torrents_notice`).
- Body: two paragraphs: "TorrServer relies on peer to peer file sharing to access content. By doing so, you also upload files to others. Any content you share is your sole responsibility." then "No further notices will be issued." (`pref_player_torrents_notice_text`, `pref_player_torrents_notice_footer`).
- One button: "OK" (`MR.strings.action_ok`); dismisses; no cancel.
- Observed: the flag is set to `true` when the dialog is opened, not when it is dismissed (`PlayerSettingsTorrentScreen.kt:80-83`). Closing the app with the dialog open still counts as shown. The port keeps this order (it is the legal-notice contract that the user accepted the disclosure by enabling).

### 2.3 Desktop layout (proposal)

- The rows are `AdwActionRow`-like rows built with plain GTK `gtk::ListBox` rows (no libadwaita): title label, subtitle label, trailing switch or entry icon.
- Dialogs: `gtk::Window` modal with an `gtk::Entry` (single line) or `gtk::TextView` (trackers), Save and Cancel buttons. Save is disabled while the validator fails; the error text appears below the entry in the error colour (`pref_player_torrents_port_error` for the port; `pref_player_torrents_proxy_url_invalid_uri` or `pref_player_torrents_proxy_url_invalid_protocol` for the proxy URL).
- The port entry uses `gtk::Entry` with `input-purpose = digits`; the validator still runs (pasted text can contain letters).
- The notice dialog is a `gtk::MessageDialog`-style modal (or `gtk::AlertDialog` in GTK 4.10+) with one OK button.

### 2.4 Gaps in the mockup

- `spec/settings.json` has the rows and the notice; it does not encode the enabled rules in section 2.1 (rows 2 to 6 disabled when torrents are off; row 4 disabled at default; row 6 disabled when proxy mode is None). The mockup must implement these rules in code, not as static spec.
- The notice is in `spec/settings.json` as `torrent_notice`; its English text must match section 2.2 exactly.
- The mockup has no port validator, no proxy validator and no "reset" behaviour.

Tasks: T-13a-042 (keys, defaults, reset rule), T-13a-043 (validators, proxy-mode mapping).

## 3. Torrent server status (notification replacement)

### 3.1 Android behaviour (for parity of meaning)

A foreground service notification while the server runs: small icon, title "Aniyomi" (app name), text "Torrent Server is running", a running timer (chronometer), not dismissible (ongoing), and one action "Stop" (hard-coded English in `TorrentServerService.kt:124`). Dismissing it restarts the service (Android 14 workaround).

### 3.2 Desktop replacement (proposal)

- Linux has no ongoing notification without an app indicator. Use the in-app status instead: a status row in Settings > Torrent showing `Stopped`, `Starting…`, `Running on port N (uptime mm:ss)` and a "Stop server" button. The state comes from `TorrServerSupervisor::status()` (`ServerStatus`).
- Optional: a desktop notification (through `kuro-platform::Notifier`) only when the server fails to start (`ServerStatus::Failed`), with the text of the failure. Not sent when the user stops it.
- The "Stop" label is a localisable string `torrent_server_stop` ("Stop"), not a literal.
- A tray icon is not part of this area (no tray in the mockup).

Task T-13a-041 (status), T-13a-044 (routing errors show a toast with the mapped message).

### 3.3 Status texts

| State | Text (key proposal) | Shown in |
|---|---|---|
| `Stopped` | "Stopped" | settings status row |
| `Starting` | "Starting…" | settings status row |
| `Running { port }` | "Running on port {port}" | settings status row and the playback error area |
| `Failed(reason)` | "Failed to start: {reason}" | settings status row, plus desktop notification once |
| `DisabledTorrServer` (playback while off) | "TorrServer is not running" (`torrserver_disabled`) | toast |

## 4. Reader page background (ImageUtil.chooseBackground)

### 4.1 Where it shows

The background is painted behind a single manga page in the paged reader when automatic background is on and the page is not animated (`PagerPageHolder.kt:152-156`, `viewer.config.automaticBackground`). It sits behind the page image; the rest of the reader uses the reader theme background.

### 4.2 Visible outcomes

- `Solid(colour)`: the page area is filled with one colour (for example white for a white page, black for a dark page, or the border colour when all six border samples agree).
- `VerticalBands { top, bottom }`: two colour bands painted top to bottom. In the Android GradientDrawable with four colours, the top band covers the upper portion from the top edge to one third of the height, the middle third is a transition from `top` to `bottom`, and the lower third is `bottom`. Desktop: the same four stops at 0, 1/3, 2/3 and 1 (cairo `LinearGradient` or CSS `linear-gradient`).
- Fallback: any decode failure gives `Solid(white)`. The page itself still shows its error state if the page cannot decode (other area).

### 4.3 Landscape rule

The Kotlin returns the solid colour (`darkBG ? blackColor : white`) when the device is in landscape. Desktop proposal: landscape means the reader viewport is wider than it is tall, evaluated when the page is set. A window resize after that does not recompute the background (matches Android, which computes it when the page is set).

### 4.4 Gaps

- The mockup's reader has no background or page fill. The reader area adds the page-fill widget; this area supplies `choose_background` returning the enum from section 3.8 of p1-rust-design.md.
- Toggle "automatic background" is a reader setting (other area); this area does not own the switch.

Task T-13a-029.

## 5. Double-page, split and merge (no direct UI in this area)

The reader's double-page rules (`PagerPageHolder.kt:193-234`, `WebtoonPageHolder.kt:210-225`) call `split_in_half`, `rotate_quarter` and `split_and_merge`. Their user-visible effect is the page layout chosen by the reader area. This area guarantees:
- `split_in_half(img, Side::Right)` returns the right half of the spread, `split_in_half(img, Side::Left)` the left half (for odd widths the middle column is dropped; behaviour 5.7).
- `rotate_quarter(img, Cw90 | Ccw90)` returns a `height x width` image.
- `split_and_merge(img, upper)` returns a `width/2 x 2*height` image with the side named by `upper` on top.

Tasks T-13a-025, T-13a-026, T-13a-027.

## 6. Tall-image splitting (downloads, no UI)

Shown only in the Download settings row "Split tall images" with summary "Improves reader performance" (`split_tall_images`, default on). The effect is visible when a downloaded page appears as several images in the chapter folder. No progress UI is involved. Task T-13a-028.

## 7. Storage and folder chooser (first-run, storage step)

The first-run storage step in `spec/app.json` requires a folder before "Next" is enabled. The folder comes from the GTK folder chooser (`gtk::FileDialog::select_folder`, portal-backed under Flatpak). The Android-only "create folder" button on TV boxes is dropped. Area 13 supplies `FolderProvider` (default folder) and the write check used to validate the chosen folder: create a temporary file in it, remove it, and report failure text "Selected folder is not writable" (proposal key `storage_folder_not_writable`). Task T-13a-015.

## 8. Things this area does not show (explicit)

- The download progress and its free-space check result: the screen reads `free_bytes` (T-13a-016) and decides the message (other area).
- The `.nomedia` marker and the cover cache sizes: shown by the storage screen as numbers only (`dir_size`).
- The WebView version and user agent: shown in the debug info screen (area 12 and debug area). This area provides `parse_chrome_major` and `infer_user_agent` only.
- Localisation of plural strings: area i18n.

## 9. Responsive behaviour (desktop)

- Toast: fixed bottom centre; on windows narrower than 520 px it spans the width minus 24 px margins.
- Torrent settings rows: full width of the settings column; dialogs are centred on the main window and have a minimum width of 360 px.
- Status row: wraps the text at the row width; never truncates the port number.
- No horizontal scrolling is introduced by any element in this area.

## 10. Keyboard and mouse

- Toast: clickable to dismiss; Escape dismisses when the window has focus; never takes focus.
- Dialogs: Enter activates the default (Save/OK) when the entry is valid; Escape cancels. Tab moves between entry and buttons.
- Switch rows: Space toggles; the row activates on click (the whole row is a target).
- The "Reset default torrent trackers" action: a button with a label, not an icon-only control.

## 11. Mockup gap summary for this area

| Missing in the mockup | Needed for | Task |
|---|---|---|
| Toast overlay | all transient messages | T-13a-053 |
| Torrent rows with enabled rules and validators | torrent settings | T-13a-042, T-13a-043 |
| Legal notice dialog (once) | first torrent enable | T-13a-042 |
| Server status row and stop button | torrent server visibility | T-13a-041 |
| Reader background fill | automatic page background | T-13a-029 |
| Folder writability check in storage step | first run | T-13a-015 |
