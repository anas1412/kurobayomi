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

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// The user-facing list of losses, as the release ships it.
    const NOTES: &str = include_str!("../RELEASE-NOTES.md");

    /// Every `.rs` file of this crate's `src` directory, as text, except this decision file.
    fn sources() -> Vec<String> {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|e| e == "rs") && !p.ends_with("dropped.rs"))
            .map(|p| std::fs::read_to_string(p).unwrap())
            .collect()
    }

    #[test]
    fn t45b083_every_loss_is_in_the_release_notes_with_its_task() {
        let notes = NOTES.to_lowercase();
        for needle in [
            "rotation lock",
            "status bar",
            "brightness",
            "software keyboard",
            "picture-in-picture",
            "file picker",
            "audio focus",
            "haptic",
            "web video caster",
            "background services",
            "volume and brightness drag",
            "t-46a-019",
            "t-46a-021",
            "t-46a-022",
            "t-45a-035",
            "t-45a-042",
            "t-45a-045",
            "t-45a-054",
            "t-45a-060",
            "t-45a-080",
            "t-45a-081",
            "t-45b-058",
            "t-45b-065",
            "t-46a-045",
            "t-46a-047",
            "t-46a-048",
            "t-46a-057",
            "t-46b-040",
        ] {
            assert!(notes.contains(needle), "release notes miss {needle}");
        }
    }

    #[test]
    fn no_android_only_api_is_left_in_the_player() {
        const FORBIDDEN: [&str; 14] = [
            "haptic",
            "PlayerOrientation",
            "BrightnessStep",
            "VerticalDrag",
            "requestAudioFocus",
            "cycle_screen_rotations",
            "START_STICKY",
            "startForeground",
            "FileProvider",
            "UniFile",
            "PictureInPictureParams",
            "setRequestedOrientation",
            "S-Pen",
            "webvideo",
        ];
        for text in sources() {
            for word in FORBIDDEN {
                assert!(!text.contains(word), "{word} is an Android-only API");
            }
        }
    }
}
