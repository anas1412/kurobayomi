# Player controls and menus

The control bar, the header and the menus of the player page (`src/player_view/`). The flow it serves is in
`docs/design/app-flow.md` ("Watching an anime": pause, seek, volume, fullscreen, previous and next, a quality and
server menu, a subtitle and audio menu, speed). Cut from the Android player on purpose: touch gestures, brightness,
aspect modes, the lock, decoder and audio channel chips, statistics.

## Layout

- **Header** (`widgets::page_header::header`, the helper shared with the reader): Back on the left, the title in the
  middle. On the right: the quality and server menu, the audio and subtitle menu. The header never hides.
- **Video**: the GL area. A click toggles pause. A double click toggles fullscreen. One press waits out the double
  click time (400 ms) before it pauses, so a double click does not also pause.
- **Control bar** (bottom, over the video, `osd toolbar`): previous, play or pause, next, seek bar, time, speed,
  chapters (only when the file has chapters), volume, fullscreen.

## Behaviour

- **Auto-hide**: the bar hides after 3 seconds without mouse movement or key press (`controls::autohide`). It never
  hides while one of its menus is open. Mouse movement and key presses show it again. Pausing does not keep it shown.
- **Previous and next**: enabled only when the playlist has a neighbour (`neighbours::adjacent`, the episode number
  order of the entry). They are checked when the episode changes, and they stay disabled at the ends.
- **Quality and server**: shown only for an online episode. When it opens, every server of the episode and its videos
  load on the source's runtime (a spinner until they arrive; the list is kept for that episode). The current video
  has the check mark. Choosing a video resolves its stream (`resolve_once`), then reloads it at the playing position
  with that video's headers, subtitles and audio (`source::media_of`). The pause state is kept. While it resolves and
  loads, the spinner shows and a second switch or a previous or next episode waits.
- **Session memory**: the last choice (server name and video title) is kept for the whole session. The next episode
  plays the same server and title when its list has them (`quality::choose`, which replaces `best_video` in
  `source.rs`), else the source's best video. The check mark marks what plays now.
- **Speed**: six presets, 0.5, 0.75, 1, 1.25, 1.5 and 2. The button shows the speed that plays now (`1.00x`). The
  speed is written to mpv's `speed` property.
- **Chapters**: the button shows only when mpv reports chapters. A row jumps to the chapter's start and plays on.
- **Audio and subtitles**: the menu lists every track by name (`track_label`), with Off first in both lists. The
  current track carries the check. The default selection is the one made at load (`source::select_defaults`).

## Shared pieces

- `sheets::menu` builds every menu button: its popover is rebuilt on each open. `sheets::section` builds the list
  (an optional heading, then a boxed list of rows with a check mark). Rows go through `widgets::row_activate`, so a
  click and Enter do the same thing. A row whose `enabled` is false is shown but does nothing.
- Pure rules are tested without GTK: `controls::autohide::hidden`, `sheets::speed::PRESETS` and its check,
  `sheets::quality::identify`. Label text comes from `kuro_player::speed::speed_label` and
  `kuro_player::chapters::{row_label, row_time}`.

## Not done

- The quality list shows videos by their title only. Resolution and bitrate are not shown separately.
- A server whose list fails to load shows one row, "Could not load", that cannot be chosen.
- Servers load one after another when the menu opens. Parallel loading would be faster for many servers.
