//! Decisions for the Android-only player pieces (area 45 and 46). Each dropped feature has no Linux
//! code path, and the user-visible losses are listed in `RELEASE-NOTES.md`.
//!
//! * Orientation lock (T-45a-035, T-45b-058, T-46a-048). Windows have no device orientation. The
//!   `pref_default_player_orientation_type_key` preference is dropped (it is in `DROPPED_KEYS` of the
//!   preference registry), so an old settings file or backup that still has it loads and the value is
//!   ignored. The replacement is the rotation control `VideoRotate` (mpv `video-rotate`, T-45a-035).
//!   The rotate button and the cycle of screen rotations are not ported.
//! * Audio focus (T-45a-042). Linux has no global audio focus API, so there is no automatic pause and
//!   no volume duck.
//! * Screen brightness (T-45a-045) and the volume and brightness drag (T-46a-019, T-46a-021,
//!   T-46a-022). Desktop monitors have no standard brightness API, so the screen brightness shortcuts,
//!   the `pref_remember_brightness` and `player_brightness_value` settings and the vertical drag on the
//!   video (with `pref_gesture_volume_brightness` and `pref_swap_volume_and_brightness`) are dropped.
//!   Old settings files and backups that hold these keys load and the values are ignored. The mpv video
//!   filter `brightness` (T-45a-032) is the only brightness control. Volume stays on the slider and the
//!   keys.
//! * Uncaught exception handler (T-45a-054). No process-wide handler closes the window. Handled
//!   errors show a GTK dialog, and panics in worker threads go to the global logger.
//! * Web Video Caster intent (T-45a-060). The package path and its extras are dropped. No Linux
//!   equivalent is planned.
//! * Services (T-45a-080). The local servers are owned by the application state and stop on exit
//!   (T-45a-039).
//! * Android file plumbing (T-45a-081). `FileProvider` URIs, `UniFile`, file descriptor paths, the
//!   S-Pen remote action, the system screenshot directory, brightness settings and content display
//!   names are dropped. A file's title is its basename on disk.
//! * Haptics (T-46a-045). No vibration on desktops. Visual feedback (thumb) stays.
//! * Picture-in-picture parameters and remote actions (T-46a-047). The `PictureInPicture` pause keeps
//!   playback going. No Android picture-in-picture params are built.
//! * Ripple configuration (T-46a-057). GTK hover and pressed states replace the Compose ripple.
//! * Software keyboard (T-45b-065). `software_keyboard` commands are accepted and ignored with a debug
//!   log in `bridge`, because the desktop has no soft keyboard.
//! * Android-only behaviour (T-46b-040). Drag-to-dismiss sheets are replaced by a click on the scrim
//!   and Escape. Haptics on sliders are dropped. The clock-dial time picker is replaced by two spin
//!   buttons (T-46b-018). Android Back is Escape.
