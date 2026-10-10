//! Volume control with the mpv boost (area 45 p2 T-45b-031; behaviour 9.1, edge E-06). Pure: the
//! caller applies the returned writes, system volume through `SystemVolume` and the boost through the
//! mpv `volume` property.

use crate::system_av::VOLUME_STEPS;

/// One write the volume change asks for, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VolumeWrite {
    /// Device volume in steps, `0..=VOLUME_STEPS`.
    System(i32),
    /// The mpv `volume` property, `0..=100 + boost cap`.
    Mpv(i64),
}

/// The volume state a change starts from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VolumeState {
    /// Device volume in steps.
    pub system: i32,
    /// The mpv `volume` property at the start of the change (`mpvVolume` in behaviour 9.1).
    pub mpv: i64,
    /// `pref_audio_volume_boost_cap`. Zero or less turns the boost off.
    pub boost_cap: i64,
}

/// The result of one volume change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VolumeChange {
    pub writes: Vec<VolumeWrite>,
    /// The device volume after the change, for the next change.
    pub system: i32,
}

/// `changeVolumeBy(change)` (behaviour 9.1). With the boost on and the device at its maximum, the
/// change goes to mpv while mpv stays in `100..=100 + cap`. Otherwise the device volume changes.
/// E-06 keeps the Kotlin order: at mpv 100 a lowering step also moves the device volume, then mpv is
/// written again at 100.
pub fn change_volume_by(state: VolumeState, change: i32) -> VolumeChange {
    let max = VOLUME_STEPS;
    let mut system = state.system;
    let mut writes = Vec::new();
    if state.boost_cap > 0 && system == max {
        if state.mpv == 100 && change < 0 {
            system = (system + change).clamp(0, max);
            writes.push(VolumeWrite::System(system));
        }
        let final_mpv = (state.mpv + i64::from(change)).max(100);
        if (100..=state.boost_cap + 100).contains(&final_mpv) {
            writes.push(VolumeWrite::Mpv(final_mpv));
            return VolumeChange { writes, system };
        }
    }
    system = (system + change).clamp(0, max);
    writes.push(VolumeWrite::System(system));
    VolumeChange { writes, system }
}

/// `setMPVVolume` (behaviour 9.1): a change of the mpv volume shows the volume slider.
pub fn shows_slider_on_mpv_change(previous: Option<i64>, new: i64) -> bool {
    previous != Some(new)
}

/// One volume-key press (T-45a-046): mpv's own `volume` moves by `step` and stays in `0..=volume_max`.
pub fn key_step(volume: i64, step: i32, volume_max: i64) -> i64 {
    (volume + i64::from(step)).clamp(0, volume_max)
}

/// The mpv volume written when the window comes back from a hidden state (`onResume`,
/// `PlayerActivity.kt:634-648`): 100 when the device volume is below its maximum, else nothing.
pub fn resume_volume(device_steps: i32) -> Option<i64> {
    (device_steps < VOLUME_STEPS).then_some(100)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(system: i32, mpv: i64, cap: i64) -> VolumeState {
        VolumeState {
            system,
            mpv,
            boost_cap: cap,
        }
    }

    #[test]
    fn t46_below_the_device_maximum_only_the_device_changes() {
        let out = change_volume_by(state(5, 100, 30), 1);
        assert_eq!(out.writes, vec![VolumeWrite::System(6)]);
        assert_eq!(out.system, 6);
    }

    #[test]
    fn t47_at_the_device_maximum_the_boost_takes_the_raise() {
        let out = change_volume_by(state(VOLUME_STEPS, 100, 30), 10);
        assert_eq!(out.writes, vec![VolumeWrite::Mpv(110)]);
        assert_eq!(out.system, VOLUME_STEPS);
    }

    #[test]
    fn t48_the_boost_stops_at_the_cap() {
        let out = change_volume_by(state(VOLUME_STEPS, 125, 30), 10);
        // 135 is above 130, so the boost stays put and the device volume is clamped at its maximum.
        assert_eq!(out.writes, vec![VolumeWrite::System(VOLUME_STEPS)]);
    }

    #[test]
    fn t49_with_the_boost_off_the_device_is_the_only_path() {
        let out = change_volume_by(state(VOLUME_STEPS, 100, 0), 1);
        assert_eq!(out.writes, vec![VolumeWrite::System(VOLUME_STEPS)]);
    }

    #[test]
    fn lowering_at_the_maximum_above_one_hundred_lowers_the_boost_first() {
        let out = change_volume_by(state(VOLUME_STEPS, 120, 30), -10);
        assert_eq!(out.writes, vec![VolumeWrite::Mpv(110)]);
    }

    #[test]
    fn e06_lowering_at_mpv_100_moves_the_device_and_keeps_the_boost_at_100() {
        let out = change_volume_by(state(VOLUME_STEPS, 100, 30), -1);
        assert_eq!(
            out.writes,
            vec![VolumeWrite::System(VOLUME_STEPS - 1), VolumeWrite::Mpv(100)]
        );
        assert_eq!(out.system, VOLUME_STEPS - 1);
    }

    #[test]
    fn device_volume_is_clamped_at_zero() {
        let out = change_volume_by(state(0, 100, 30), -5);
        assert_eq!(out.writes, vec![VolumeWrite::System(0)]);
    }

    #[test]
    fn t46_a_key_step_moves_mpv_volume_by_one_and_stays_in_range() {
        assert_eq!(key_step(50, 1, 130), 51);
        assert_eq!(key_step(50, -1, 130), 49);
        assert_eq!(key_step(130, 1, 130), 130);
        assert_eq!(key_step(0, -1, 130), 0);
    }

    #[test]
    fn t46_resume_sets_100_only_below_the_device_maximum() {
        assert_eq!(resume_volume(VOLUME_STEPS - 1), Some(100));
        assert_eq!(resume_volume(VOLUME_STEPS), None);
    }

    #[test]
    fn a_mpv_change_shows_the_slider_only_when_the_value_moved() {
        assert!(shows_slider_on_mpv_change(None, 100));
        assert!(!shows_slider_on_mpv_change(Some(100), 100));
        assert!(shows_slider_on_mpv_change(Some(100), 101));
    }
}
