//! Seek primitives, the double-tap accumulator, the tap dispatch tables and the gesture seek
//! indicator text (area 45 `PlayerViewModel` seek functions, behaviour 8.2 and 8.3; area 46
//! `MiddlePlayerControls.kt`, T-45b-023, T-45b-024, T-46a-007). Pure: the caller writes the returned
//! commands to mpv. Times are whole seconds.

use crate::controls::Zone;
use crate::enums::SingleActionGesture;
use crate::time::pretty_time;

/// Flag for a relative seek. `precise` is `pref_player_smooth_seek`.
pub fn relative_flag(precise: bool) -> &'static str {
    if precise {
        "relative+exact"
    } else {
        "relative"
    }
}

/// Flag for an absolute seek. `precise = false` uses keyframes.
pub fn absolute_flag(precise: bool) -> &'static str {
    if precise {
        "absolute"
    } else {
        "absolute+keyframes"
    }
}

/// A seek to an absolute position, waiting until the duration is known (E-11).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Seeker {
    pending: Option<(i64, bool)>,
}

impl Seeker {
    /// `seekTo`. Returns the command `(position, flag)` to send. An unknown duration (zero) queues
    /// the seek, and a position outside `0..=duration` is ignored.
    pub fn seek_to(
        &mut self,
        position: i64,
        precise: bool,
        duration: i64,
    ) -> Option<(i64, &'static str)> {
        if duration <= 0 {
            self.pending = Some((position, precise));
            return None;
        }
        if !(0..=duration).contains(&position) {
            return None;
        }
        Some((position, absolute_flag(precise)))
    }

    /// Runs the queued seek once the duration is known.
    pub fn duration_known(&mut self, duration: i64) -> Option<(i64, &'static str)> {
        let (position, precise) = self.pending.take()?;
        self.seek_to(position, precise, duration)
    }
}

/// The back-tap accumulator after one more step of `value` (E-19 fix). It adds the step, and resets
/// to zero only when the target passes the duration. The Kotlin reset on a second backward step is
/// removed, so repeated back-taps keep adding.
pub fn accumulate(amount: i64, value: i64, pos: i64, duration: i64) -> i64 {
    if pos + value > duration {
        0
    } else {
        amount + value
    }
}

/// Which side of the video a double tap landed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapSide {
    Left,
    Center,
    Right,
}

/// What a double tap or single action does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TapCommand {
    /// Relative seek by `offset` seconds.
    SeekBy(i64),
    PlayPause,
    /// Key code sent to mpv with `keypress`, one of `0x10001`, `0x10002`, `0x10003`.
    Keypress(&'static str),
    /// Previous episode when `previous` is set, otherwise next.
    Switch {
        previous: bool,
    },
    Nothing,
}

/// The dispatch tables of behaviour 8.3. `skip` is `pref_skip_length_preference`. The centre has
/// no seek or switch.
pub fn tap_command(side: TapSide, action: SingleActionGesture, skip: i64) -> TapCommand {
    use SingleActionGesture as A;
    use TapSide::*;
    match (side, action) {
        (Left, A::Seek) => TapCommand::SeekBy(-skip),
        (Right, A::Seek) => TapCommand::SeekBy(skip),
        (Left, A::Switch) => TapCommand::Switch { previous: true },
        (Right, A::Switch) => TapCommand::Switch { previous: false },
        (_, A::PlayPause) => TapCommand::PlayPause,
        (Left, A::Custom) => TapCommand::Keypress("0x10001"),
        (Center, A::Custom) => TapCommand::Keypress("0x10002"),
        (Right, A::Custom) => TapCommand::Keypress("0x10003"),
        _ => TapCommand::Nothing,
    }
}

/// The command for a double tap in `zone` with the zone's action. While the controls are locked
/// a double tap does nothing (GH:128, T-46a-015).
pub fn double_tap_command(
    zone: Zone,
    action: SingleActionGesture,
    skip: i64,
    locked: bool,
) -> TapCommand {
    if locked {
        return TapCommand::Nothing;
    }
    let side = match zone {
        Zone::Left => TapSide::Left,
        Zone::Center => TapSide::Center,
        Zone::Right => TapSide::Right,
    };
    tap_command(side, action, skip)
}

/// The text of the gesture seek indicator: `"%c%s\n[%s]"`. The sign is `+` for a delta of zero or
/// more, the second line is the target time `start + delta`.
pub fn gesture_seek_text(start: i64, delta: i64) -> String {
    let sign = if delta >= 0 { '+' } else { '-' };
    format!(
        "{sign}{}\n[{}]",
        pretty_time(delta.abs()),
        pretty_time(start + delta)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_strings_match_the_spec() {
        assert_eq!(relative_flag(false), "relative");
        assert_eq!(relative_flag(true), "relative+exact");
        assert_eq!(absolute_flag(true), "absolute");
        assert_eq!(absolute_flag(false), "absolute+keyframes");
    }

    #[test]
    fn t43_seek_to_out_of_range_is_ignored() {
        let mut s = Seeker::default();
        assert_eq!(s.seek_to(-1, true, 100), None);
        assert_eq!(s.seek_to(101, true, 100), None);
        assert_eq!(s.seek_to(100, true, 100), Some((100, "absolute")));
        assert_eq!(s.seek_to(0, false, 100), Some((0, "absolute+keyframes")));
    }

    #[test]
    fn seek_to_waits_for_the_duration() {
        let mut s = Seeker::default();
        assert_eq!(s.seek_to(50, true, 0), None);
        assert_eq!(s.duration_known(100), Some((50, "absolute")));
        assert_eq!(s.duration_known(100), None);
    }

    #[test]
    fn t44_back_taps_keep_adding_and_reset_past_the_duration() {
        let a = accumulate(0, -10, 60, 100);
        assert_eq!(a, -10);
        let b = accumulate(a, -10, 50, 100);
        assert_eq!(b, -20);
        assert_eq!(accumulate(-20, 10, 99, 100), 0);
    }

    #[test]
    fn t45_dispatch_tables_cover_every_action_per_side() {
        use SingleActionGesture as A;
        use TapSide::*;
        assert_eq!(tap_command(Left, A::Seek, 10), TapCommand::SeekBy(-10));
        assert_eq!(tap_command(Right, A::Seek, 10), TapCommand::SeekBy(10));
        assert_eq!(tap_command(Center, A::Seek, 10), TapCommand::Nothing);
        assert_eq!(tap_command(Center, A::Switch, 10), TapCommand::Nothing);
        assert_eq!(
            tap_command(Left, A::Switch, 10),
            TapCommand::Switch { previous: true }
        );
        assert_eq!(
            tap_command(Right, A::Switch, 10),
            TapCommand::Switch { previous: false }
        );
        assert_eq!(tap_command(Center, A::PlayPause, 10), TapCommand::PlayPause);
        assert_eq!(tap_command(Left, A::None, 10), TapCommand::Nothing);
        assert_eq!(
            tap_command(Right, A::Custom, 10),
            TapCommand::Keypress("0x10003")
        );
        for side in [Left, Center, Right] {
            assert_eq!(tap_command(side, A::None, 10), TapCommand::Nothing);
        }
        assert_eq!(
            tap_command(Center, A::Custom, 10),
            TapCommand::Keypress("0x10002")
        );
        assert_eq!(tap_command(Right, A::PlayPause, 10), TapCommand::PlayPause);
    }

    #[test]
    fn a_locked_double_tap_does_nothing_in_any_zone() {
        use SingleActionGesture as A;
        assert_eq!(
            double_tap_command(Zone::Right, A::Seek, 10, true),
            TapCommand::Nothing
        );
        assert_eq!(
            double_tap_command(Zone::Right, A::Seek, 10, false),
            TapCommand::SeekBy(10)
        );
        assert_eq!(
            double_tap_command(Zone::Center, A::PlayPause, 10, false),
            TapCommand::PlayPause
        );
    }

    #[test]
    fn indicator_text_matches_the_spec_example() {
        assert_eq!(gesture_seek_text(85, 12), "+00:12\n[01:37]");
        assert_eq!(gesture_seek_text(85, -12), "-00:12\n[01:13]");
        assert_eq!(gesture_seek_text(0, 0), "+00:00\n[00:00]");
    }
}
