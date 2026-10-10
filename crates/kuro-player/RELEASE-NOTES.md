# Player: what the desktop build does not do

Kurobayomi keeps the player features of Aniyomi that have a desktop equivalent. The items below are Android-only
and are not in this release. Each one names its replacement, or "none".

## Losses

- **Rotation lock is no longer available.** Windows have no device orientation. The rotation control sets the
  picture rotation instead (0, 90, 180 or 270 degrees). The old default orientation setting is kept but has no
  effect. Replacement: the rotation control (T-45a-035). The rotate button and the rotation cycle are gone
  (T-45b-058, T-46a-048).
- **Android status bar setting does nothing.** There is no status bar to hide. Replacement: fullscreen (T-45b-059).
- **Window brightness control is gone.** The monitor brightness cannot be set from the player. Replacement: the mpv
  brightness filter, which changes the picture only (T-45a-032, T-45a-045).
- **Software keyboard is not shown.** Text fields use the normal desktop keyboard. Replacement: none (T-45b-065).
- **Android picture-in-picture is gone.** The player does not shrink to a floating window when you leave the app,
  and the picture-in-picture buttons are not there. Replacement: none (T-46a-047).
- **Content URI picker is not used.** Local files are read from their path on disk through the file picker of
  the desktop. Replacement: the file picker (T-45a-081).
- **Automatic pause on audio focus is gone.** The player does not pause when a call or another app takes the sound,
  and the volume is not lowered for other apps. Replacement: none (T-45a-042).
- **The haptic tick on sliders is gone.** The pill and the moving thumb still show the change. Replacement: none
  (T-46a-045, T-46b-040).
- **Ripple tint on buttons is not the same.** Buttons show a hover and a pressed state. Replacement: GTK states
  (T-46a-057).
- **Drag-to-close for the sheets is gone.** Close a sheet with a click on the dark area or with Escape. Replacement:
  scrim click and Escape (T-46b-040).
- **Clock-dial time picker is gone.** Use the two spin buttons. No function is lost (T-46b-040, T-46b-018).
- **Android Back is Escape.** Replacement: Escape (T-46b-040).
- **Casting through Web Video Caster is gone.** Replacement: none (T-45a-060).
- **Servers do not survive closing the app.** The streaming and torrent servers stop when the app exits, because
  the app no longer runs them as background services. Replacement: shutdown on exit (T-45a-080, T-45a-039).
- **Uncaught exception handler is not installed.** Handled errors show a dialog. Replacement: GTK error dialog
  (T-45a-054).
