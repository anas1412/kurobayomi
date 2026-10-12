//! Repeating button schedule (area 46 `RepeatingIconButton.kt`, T-46a-033). Pure: the GTK layer
//! fires the click at each press and at each timer tick, then arms the next timer with the delay
//! returned here. Timer jitter is accepted (E-46-021).

/// First wait after a press, in milliseconds.
pub const MAX_DELAY_MS: u64 = 750;
/// Shortest wait the schedule reaches.
pub const MIN_DELAY_MS: u64 = 5;
/// Each wait is this fraction shorter than the last.
pub const DECAY: f64 = 0.25;

/// The waits between clicks: 750, 562, 421, 315, ... down to 5 and then stays at 5.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RepeatSchedule {
    next: u64,
}

impl Default for RepeatSchedule {
    fn default() -> Self {
        Self { next: MAX_DELAY_MS }
    }
}

impl RepeatSchedule {
    /// The wait before the next click. Advances the schedule.
    pub fn next_delay(&mut self) -> u64 {
        let wait = self.next;
        let shorter = (wait as f64 - wait as f64 * DECAY) as u64;
        self.next = shorter.max(MIN_DELAY_MS);
        wait
    }
}

/// Press state of one repeating button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Repeater {
    active: bool,
    schedule: RepeatSchedule,
}

impl Repeater {
    /// Pressed: the caller clicks now and arms a timer for the returned delay.
    pub fn press(&mut self) -> u64 {
        self.active = true;
        self.schedule = RepeatSchedule::default();
        self.schedule.next_delay()
    }

    /// A timer fired. While still pressed the caller clicks and arms the returned delay. `None`
    /// after release or disable, so no click fires.
    pub fn tick(&mut self) -> Option<u64> {
        if self.active {
            Some(self.schedule.next_delay())
        } else {
            None
        }
    }

    /// Release, disable or destroy: cancels the repeat.
    pub fn release(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_times_start_at_the_press() {
        let mut r = Repeater::default();
        let mut time = 0;
        let mut fires = vec![time];
        time += r.press();
        fires.push(time);
        for _ in 0..3 {
            time += r.tick().unwrap();
            fires.push(time);
        }
        assert_eq!(fires, vec![0, 750, 1312, 1733, 2048]);
    }

    #[test]
    fn release_cancels_the_next_click() {
        let mut r = Repeater::default();
        r.press();
        r.release();
        assert_eq!(r.tick(), None);
    }
}
