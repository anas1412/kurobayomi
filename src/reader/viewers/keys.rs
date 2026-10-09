//! Keyboard tables for the pager and the webtoon (T-44a-013, T-44a-014, T-44a-015, B§4.2, B§4.4, E-15, E-25, E-26).
//!
//! Each key is claimed on press and on release (the release is swallowed, E-26). The action runs on release, as
//! upstream does. Volume keys are claimed only when the feature is on and the menu is hidden (E-25).

use super::pager::nav::PageCmd;

/// The keys the viewers know. `Other` is never claimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputKey {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Menu,
    VolumeUp,
    VolumeDown,
    Other,
}

/// One key event. `released` is true for the release edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyEvent {
    pub key: InputKey,
    pub ctrl: bool,
    pub released: bool,
}

/// The settings and state a key decision reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyContext {
    pub volume_enabled: bool,
    pub volume_inverted: bool,
    pub menu_visible: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyAction {
    Page(PageCmd),
    ToggleMenu,
    /// `down` scrolls the webtoon down; otherwise up.
    Scroll { down: bool },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyOutcome {
    pub claimed: bool,
    /// Runs on the release edge only.
    pub action: Option<KeyAction>,
}

const UNCLAIMED: KeyOutcome = KeyOutcome { claimed: false, action: None };

fn claimed(action: Option<KeyAction>, ev: KeyEvent) -> KeyOutcome {
    KeyOutcome { claimed: true, action: if ev.released { action } else { None } }
}

/// Whether a volume key is claimed at all (E-25).
fn volume_claimed(ctx: KeyContext) -> bool {
    ctx.volume_enabled && !ctx.menu_visible
}

/// The pager table (B§4.2). Volume keys page with `Prev` for down unless inverted. Ctrl with the horizontal
/// arrows moves by reading order; plain arrows are physical.
pub fn pager_key(ev: KeyEvent, ctx: KeyContext) -> KeyOutcome {
    let action = match ev.key {
        InputKey::VolumeDown if volume_claimed(ctx) => Some(page(if ctx.volume_inverted { PageCmd::Prev } else { PageCmd::Next })),
        InputKey::VolumeUp if volume_claimed(ctx) => Some(page(if ctx.volume_inverted { PageCmd::Next } else { PageCmd::Prev })),
        InputKey::Right => Some(page(if ev.ctrl { PageCmd::Next } else { PageCmd::Right })),
        InputKey::Left => Some(page(if ev.ctrl { PageCmd::Prev } else { PageCmd::Left })),
        InputKey::Down | InputKey::PageDown => Some(page(PageCmd::Next)),
        InputKey::Up | InputKey::PageUp => Some(page(PageCmd::Prev)),
        InputKey::Menu => Some(KeyAction::ToggleMenu),
        _ => return UNCLAIMED,
    };
    claimed(action, ev)
}

/// The webtoon table (B§4.4). Ctrl is ignored. Volume keys scroll.
pub fn webtoon_key(ev: KeyEvent, ctx: KeyContext) -> KeyOutcome {
    let scroll = |down| Some(KeyAction::Scroll { down });
    let action = match ev.key {
        // `volumeKeysInverted ? scrollUp() : scrollDown()` on the volume-down release.
        InputKey::VolumeDown if volume_claimed(ctx) => scroll(!ctx.volume_inverted),
        InputKey::VolumeUp if volume_claimed(ctx) => scroll(ctx.volume_inverted),
        InputKey::Left | InputKey::Up | InputKey::PageUp => scroll(false),
        InputKey::Right | InputKey::Down | InputKey::PageDown => scroll(true),
        InputKey::Menu => Some(KeyAction::ToggleMenu),
        _ => return UNCLAIMED,
    };
    claimed(action, ev)
}

fn page(cmd: PageCmd) -> KeyAction {
    KeyAction::Page(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    const OFF: KeyContext = KeyContext { volume_enabled: false, volume_inverted: false, menu_visible: false };
    const ON: KeyContext = KeyContext { volume_enabled: true, volume_inverted: false, menu_visible: false };

    fn press(key: InputKey) -> KeyEvent {
        KeyEvent { key, ctrl: false, released: false }
    }

    fn release(key: InputKey) -> KeyEvent {
        KeyEvent { key, ctrl: false, released: true }
    }

    #[test]
    fn pager_keys_are_claimed_on_both_edges_and_act_on_release() {
        let down = pager_key(press(InputKey::Right), OFF);
        assert!(down.claimed && down.action.is_none());
        let up = pager_key(release(InputKey::Right), OFF);
        assert_eq!(up, KeyOutcome { claimed: true, action: Some(KeyAction::Page(PageCmd::Right)) });
    }

    #[test]
    fn ctrl_arrows_use_the_reading_order_and_plain_arrows_are_physical() {
        let ctrl_right = KeyEvent { key: InputKey::Right, ctrl: true, released: true };
        assert_eq!(pager_key(ctrl_right, OFF).action, Some(KeyAction::Page(PageCmd::Next)));
        let ctrl_left = KeyEvent { key: InputKey::Left, ctrl: true, released: true };
        assert_eq!(pager_key(ctrl_left, OFF).action, Some(KeyAction::Page(PageCmd::Prev)));
        assert_eq!(pager_key(release(InputKey::Left), OFF).action, Some(KeyAction::Page(PageCmd::Left)));
    }

    #[test]
    fn vertical_and_page_keys_step_through_the_reading_order() {
        assert_eq!(pager_key(release(InputKey::PageDown), OFF).action, Some(KeyAction::Page(PageCmd::Next)));
        assert_eq!(pager_key(release(InputKey::Up), OFF).action, Some(KeyAction::Page(PageCmd::Prev)));
        assert_eq!(pager_key(release(InputKey::Menu), OFF).action, Some(KeyAction::ToggleMenu));
    }

    #[test]
    fn volume_keys_are_not_claimed_when_off_or_when_the_menu_is_visible() {
        assert!(!pager_key(press(InputKey::VolumeUp), OFF).claimed);
        let menu = KeyContext { menu_visible: true, ..ON };
        assert!(!pager_key(press(InputKey::VolumeDown), menu).claimed);
        assert!(!webtoon_key(press(InputKey::VolumeDown), menu).claimed);
    }

    #[test]
    fn volume_keys_page_and_invert_when_enabled() {
        assert_eq!(pager_key(release(InputKey::VolumeDown), ON).action, Some(KeyAction::Page(PageCmd::Next)));
        assert_eq!(pager_key(release(InputKey::VolumeUp), ON).action, Some(KeyAction::Page(PageCmd::Prev)));
        let inverted = KeyContext { volume_inverted: true, ..ON };
        assert_eq!(pager_key(release(InputKey::VolumeDown), inverted).action, Some(KeyAction::Page(PageCmd::Prev)));
        assert_eq!(webtoon_key(release(InputKey::VolumeDown), inverted).action, Some(KeyAction::Scroll { down: false }));
        assert_eq!(webtoon_key(release(InputKey::VolumeUp), inverted).action, Some(KeyAction::Scroll { down: true }));
    }

    #[test]
    fn webtoon_scroll_keys_are_consumed_on_both_edges() {
        for key in [InputKey::Left, InputKey::Up, InputKey::PageUp] {
            assert_eq!(webtoon_key(release(key), OFF).action, Some(KeyAction::Scroll { down: false }));
            assert!(webtoon_key(press(key), OFF).claimed);
        }
        for key in [InputKey::Right, InputKey::Down, InputKey::PageDown] {
            assert_eq!(webtoon_key(release(key), OFF).action, Some(KeyAction::Scroll { down: true }));
        }
    }

    #[test]
    fn unlisted_keys_are_never_claimed() {
        assert_eq!(pager_key(press(InputKey::Other), OFF), UNCLAIMED);
        assert_eq!(webtoon_key(press(InputKey::Other), OFF), UNCLAIMED);
    }
}
