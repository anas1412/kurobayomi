# Player and reader: embedded or detached

The anime player and the manga reader open as a page inside the main window. The sidebar hides while the page shows,
because the page's root widget is named `screen:player` or `screen:reader` (`main.rs`). Either page can move into a
window of its own and back while it keeps playing or reading.

## The host (`src/widgets/detachable.rs`)

`Detachable` holds the page widget weakly, plus a header button whose icon and tooltip say **Detach** or **Attach**.

- **Detach**: the page leaves the navigation stack (the page below shows at once) and becomes the child of a new
  `gtk::Window`. The window has the page's title and is 960 x 540.
- **Attach**: the window lets go of the page and is destroyed, then the page is pushed on the stack again.
- **Close** (Back, Escape in the reader, the window's close button): an embedded page pops, and a detached window closes.
- `is_moving()` is true only while the page is between the two toplevels. GTK unrealizes the page when it leaves a
  toplevel, so content can use it to tell a move from a close.

The player page has a slim header: Back, the title, Detach, the quality and server menu (online episodes), and the tracks menu (audio and subtitles). The detached
window hides GTK's default title bar, so this header moves and closes the window. The reader keeps its own top bar
(Back, titles, Detach, bookmark, overflow). Its detached window keeps the default title bar, because the reader's bar
hides with the menu. Fullscreen (F11, the control bar button) acts on whichever window holds the page. The player
header hides while that window is fullscreen.

The player remembers the last choice in `[player.window] open_detached` (`config.toml`, default `false`), so the next
episode opens the same way. The reader always opens embedded, because its callers push the page themselves.

## The render-context handoff (player)

mpv keeps playing across a move. Only the GL render context belongs to a window, so it is replaced:

1. **Unrealize while moving** (`player_view::wire_gl`): read `vid` and the pause state, set `vid=no`, make the old GL
   context current, free the render context. Video goes off first because freeing the context with video on makes mpv
   drop the track as failed, and that ends a file with no audio. With video off, audio keeps playing.
2. **Realize in the new toplevel**: create the render context and its update callback, set `vid` back and restore
   the pause state. mpv brings the picture back at the current position. No seek is issued, so audio does not jump.
3. **Unrealize without a move** (the page was popped, its window was closed, the app quit): progress is saved, playback
   stops, and the context is freed. Back saves and stops at once, before the page leaves.

The first realize loads the file. The `loadfile` form has no index argument, so mpv before 0.38 accepts it.
