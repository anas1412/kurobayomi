//! Pointer equivalents of the touch gestures (area 46 `GestureHandler.kt`; T-46a-020 and T-46a-044).
//! Pure: the GTK gesture handlers feed in positions, buttons and times in milliseconds, and act on
//! the results. Double-click zones reuse `gesture::zone_at`.

use crate::enums::Sheets;

/// Press and hold time that opens the screenshot sheet (`P-3`, E-46-073).
pub const LONG_PRESS_MS: u64 = 500;

/// State of a long press that paused playback. The screenshot sheet is open while it is held.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LongPress {
    /// Playback was running before the press, so dismissal resumes it.
    pub was_playing: bool,
}

/// A long press pauses and opens the screenshot sheet, unless the controls are locked. `paused` is
/// `None` until mpv reports it, which counts as not playing. No speed is restored on release
/// (E-46-011).
pub fn begin_long_press(locked: bool, paused: Option<bool>) -> Option<LongPress> {
    if locked {
        None
    } else {
        Some(LongPress {
            was_playing: paused == Some(false),
        })
    }
}

/// The sheet a secondary (right) click opens on the video.
pub const SECONDARY_CLICK_SHEET: Sheets = Sheets::More;

/// Press-and-hold timer. `due` is true once, after the hold reaches `LONG_PRESS_MS`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hold {
    started_ms: Option<u64>,
    fired: bool,
}

impl Hold {
    pub fn press(&mut self, now_ms: u64) {
        self.started_ms = Some(now_ms);
        self.fired = false;
    }

    pub fn release(&mut self) {
        self.started_ms = None;
    }

    /// True on the first call at or after the long-press time, and false afterwards.
    pub fn due(&mut self, now_ms: u64) -> bool {
        match self.started_ms {
            Some(start) if !self.fired && now_ms.saturating_sub(start) >= LONG_PRESS_MS => {
                self.fired = true;
                true
            }
            _ => false,
        }
    }
}

/// What a wheel scroll over the video does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wheel {
    /// Device volume by this many steps. Scrolling up is louder.
    Volume(i32),
    /// Seek by this many seconds. Scrolling right is forward.
    Seek(i64),
}

/// The action of one wheel event. The dominant axis wins. Ignored while locked (E-46-075).
/// `skip_secs` is the skip length from the preferences.
pub fn wheel(dx: f64, dy: f64, locked: bool, skip_secs: i64) -> Option<Wheel> {
    if locked || (dx == 0.0 && dy == 0.0) {
        None
    } else if dx.abs() > dy.abs() {
        Some(Wheel::Seek(if dx > 0.0 { skip_secs } else { -skip_secs }))
    } else if dy < 0.0 {
        Some(Wheel::Volume(1))
    } else {
        Some(Wheel::Volume(-1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Zone;
    use crate::gesture::zone_at;

    #[test]
    fn long_press_remembers_playback_and_is_ignored_when_locked() {
        assert_eq!(begin_long_press(true, Some(false)), None);
        assert_eq!(
            begin_long_press(false, Some(false)),
            Some(LongPress { was_playing: true })
        );
        assert_eq!(
            begin_long_press(false, Some(true)),
            Some(LongPress { was_playing: false })
        );
        assert_eq!(
            begin_long_press(false, None),
            Some(LongPress { was_playing: false })
        );
    }

    #[test]
    fn hold_fires_once_at_five_hundred_ms() {
        let mut hold = Hold::default();
        hold.press(1000);
        assert!(!hold.due(1499));
        assert!(hold.due(1500));
        assert!(!hold.due(2000));
        hold.release();
        hold.press(3000);
        hold.release();
        assert!(!hold.due(4000));
    }

    #[test]
    fn wheel_steps_volume_and_seeks_on_the_dominant_axis() {
        assert_eq!(wheel(0.0, -1.0, false, 10), Some(Wheel::Volume(1)));
        assert_eq!(wheel(0.0, 1.0, false, 10), Some(Wheel::Volume(-1)));
        assert_eq!(wheel(1.0, 0.2, false, 10), Some(Wheel::Seek(10)));
        assert_eq!(wheel(-1.0, 0.0, false, 5), Some(Wheel::Seek(-5)));
        assert_eq!(wheel(0.0, 0.0, false, 10), None);
    }

    #[test]
    fn wheel_is_ignored_while_locked() {
        assert_eq!(wheel(0.0, -1.0, true, 10), None);
    }

    #[test]
    fn double_click_zones_match_the_double_tap_zones() {
        assert_eq!(zone_at(900.0, 1000.0), Zone::Right);
        assert_eq!(zone_at(500.0, 1000.0), Zone::Center);
        assert_eq!(zone_at(100.0, 1000.0), Zone::Left);
    }
}
