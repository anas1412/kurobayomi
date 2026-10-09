//! Window-level key bindings and custom key codes (area 46, `PlayerActivity.kt` key handling,
//! T-45a-025). Pure: the GTK window maps its key event to a keyval and acts on the result.
//! Keys that are not listed here are forwarded to `keys::map_key`.

use crate::enums::CustomKeyCode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowKey {
    /// Up arrow: volume +1 and show the volume slider.
    VolumeUp,
    /// Down arrow: volume -1 and show the volume slider.
    VolumeDown,
    /// Left arrow and the rewind media key: the left double-tap action.
    DoubleTapLeft,
    /// Right arrow and the fast-forward media key: the right double-tap action.
    DoubleTapRight,
    /// Space.
    TogglePause,
    /// Media stop: closes the window.
    CloseWindow,
    /// Custom gesture code (`0x10001`..`0x10006`), sent through `keypress`.
    Custom(CustomKeyCode),
    /// Not bound here; the event goes to `keys::map_key` and is consumed.
    Forward,
}

// X11 keysyms (`X11/keysymdef.h`, `X11/XF86keysym.h`).
const KS_LEFT: u32 = 0xff51;
const KS_UP: u32 = 0xff52;
const KS_RIGHT: u32 = 0xff53;
const KS_DOWN: u32 = 0xff54;
const KS_SPACE: u32 = 0x0020;
const KS_XF86_AUDIO_STOP: u32 = 0x1008ff15;
const KS_XF86_AUDIO_REWIND: u32 = 0x1008ff3e;
const KS_XF86_AUDIO_FORWARD: u32 = 0x1008ff97;

pub fn window_key(keyval: u32) -> WindowKey {
    match keyval {
        KS_UP => WindowKey::VolumeUp,
        KS_DOWN => WindowKey::VolumeDown,
        KS_LEFT | KS_XF86_AUDIO_REWIND => WindowKey::DoubleTapLeft,
        KS_RIGHT | KS_XF86_AUDIO_FORWARD => WindowKey::DoubleTapRight,
        KS_SPACE => WindowKey::TogglePause,
        KS_XF86_AUDIO_STOP => WindowKey::CloseWindow,
        code => CustomKeyCode::from_code(code).map_or(WindowKey::Forward, WindowKey::Custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_arrows_and_space() {
        assert_eq!(window_key(KS_UP), WindowKey::VolumeUp);
        assert_eq!(window_key(KS_DOWN), WindowKey::VolumeDown);
        assert_eq!(window_key(KS_SPACE), WindowKey::TogglePause);
    }

    #[test]
    fn horizontal_keys_and_media_rewind_forward_run_double_taps() {
        assert_eq!(window_key(KS_LEFT), WindowKey::DoubleTapLeft);
        assert_eq!(window_key(KS_XF86_AUDIO_REWIND), WindowKey::DoubleTapLeft);
        assert_eq!(window_key(KS_RIGHT), WindowKey::DoubleTapRight);
        assert_eq!(window_key(KS_XF86_AUDIO_FORWARD), WindowKey::DoubleTapRight);
    }

    #[test]
    fn media_stop_closes_window() {
        assert_eq!(window_key(KS_XF86_AUDIO_STOP), WindowKey::CloseWindow);
    }

    #[test]
    fn custom_codes_are_sent() {
        assert_eq!(
            window_key(0x10001),
            WindowKey::Custom(CustomKeyCode::DoubleTapLeft)
        );
        assert_eq!(
            window_key(0x10006),
            WindowKey::Custom(CustomKeyCode::MediaNext)
        );
        assert_eq!(window_key(0x10007), WindowKey::Forward);
    }

    #[test]
    fn other_keys_are_forwarded() {
        assert_eq!(window_key(u32::from('a')), WindowKey::Forward);
        assert_eq!(window_key(0xff1b), WindowKey::Forward);
    }
}
