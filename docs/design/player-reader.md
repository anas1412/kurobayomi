# Player and reader: embedded in the main window

The anime player and the manga reader open as a page inside the main window. The sidebar hides while the page shows,
because the page's root widget is named `screen:player` or `screen:reader` (`main.rs`). Each page is pushed on the
main window's navigation stack by its caller (`crate::ui::push`) and leaves it with `crate::ui::pop`.

There is no pop-out window: the Detach / Attach button and the `Detachable` host were cut (user, 2026-10-11).

## Leaving a page

- **Back** in the page header (player) or the top bar (reader) leaves the page at once.
- **Escape, Alt+Left and the mouse back button** pop the page through `ui::pop`. The player watches the main stack
  (`player_view::watch_left`) and stops playback and saves progress at that call, not when the stack removes the page
  after its 400 ms transition. The handler is disconnected when the GL area unrealizes.
- The player also stops when its GL area unrealizes (`player_view::wire_gl`), so the close is saved however the page left.

The player remembers nothing about its placement; it always opens in the main window.

## The render context (player)

The GL render context belongs to the page's window, so it is created on realize and destroyed on unrealize while the
GL context is still current. The first realize loads the file. The `loadfile` form has no index argument, so mpv
before 0.38 accepts it.

## The top bar (reader)

The reader's top bar (back, title, bookmark, overflow menu) is always shown; only the bottom bar follows the menu
toggle (guard test in `src/reader/topbar_guard.rs`).
