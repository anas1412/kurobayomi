//! Vertical drag on the video surface: volume on one half, brightness on the other, and mpv boost
//! above the device maximum (area 46 `GestureHandler.kt` drag math, T-46a-019). Pure: the GTK layer
//! feeds pointer events in and applies the returned `Write` to the device, mpv or the veil.
//!
//! Sign convention: `dy` is the change since the last event, positive when the pointer moves down.
//! Upward drags increase the value. `y` is the absolute pointer position on the surface.

/// Brightness change per pixel (`brightnessGestureSens`).
pub const BRIGHTNESS_SENS: f32 = 0.001;
/// Lowest brightness state. Values below zero only darken the veil.
pub const BRIGHTNESS_MIN: f32 = -0.75;
/// Highest brightness state.
pub const BRIGHTNESS_MAX: f32 = 1.0;
/// Device volume change per pixel per device step (`volumeGestureSens = 0.001 * maxVolume`).
pub const VOLUME_SENS_PER_PX: f32 = 0.001;
/// mpv volume at which boost starts (mpv's own 100 percent).
pub const MPV_BASE: i32 = 100;
/// How long a pill stays up after the last event, in milliseconds.
pub const PILL_MS: u64 = 2000;

/// The control a drag is changing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Channel {
    Brightness,
    Volume,
}

/// Current values the drag starts from and writes to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Levels {
    /// Device volume step, 0..=max.
    pub device: i32,
    /// mpv volume, `MPV_BASE`..=`MPV_BASE + boost_cap`.
    pub mpv: i32,
    /// Brightness state, `BRIGHTNESS_MIN`..=`BRIGHTNESS_MAX`.
    pub brightness: f32,
}

/// Drag settings, read when the pointer goes down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Config {
    /// `pref_gesture_volume_brightness`.
    pub enabled: bool,
    /// Controls locked: the drag does nothing.
    pub locked: bool,
    /// `pref_swap_volume_and_brightness`: brightness on the right half instead.
    pub swap: bool,
    /// Device volume maximum (15 on phones).
    pub max_device: i32,
    /// `pref_audio_volume_boost_cap`. Zero turns boost off.
    pub boost_cap: i32,
}

/// One write to apply. The value is already clamped to its range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Write {
    Device(i32),
    Mpv(i32),
    Brightness(f32),
}

/// Which half of the surface a pointer x is in.
pub fn channel_for(x: f32, width: f32, swap: bool) -> Channel {
    let left = x < width / 2.0;
    if left != swap {
        Channel::Brightness
    } else {
        Channel::Volume
    }
}

/// The mpv `brightness` property for a brightness state (T-45b-032). The state is clamped to
/// `BRIGHTNESS_MIN..=BRIGHTNESS_MAX` and scaled to -100..=100. This replaces the Android window
/// backlight, which Linux has no equivalent for.
pub fn mpv_brightness(state: f32) -> i64 {
    (state.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX) * 100.0).round() as i64
}

/// Window brightness for a brightness state: clamped to 0..=1. Negative states do not change the
/// hardware.
pub fn window_brightness(state: f32) -> f32 {
    state.clamp(0.0, BRIGHTNESS_MAX)
}

/// Software veil alpha for a brightness state: 0 for non-negative states, `-state` otherwise.
pub fn veil_alpha(state: f32) -> f32 {
    (-state).clamp(0.0, -BRIGHTNESS_MIN)
}

/// Port of `calc` in `GestureHandler.kt`: `original + ((start - y) * sens).toInt()`. The truncation
/// is toward zero, as Kotlin's `Float.toInt()`.
fn step_int(original: i32, start: f32, y: f32, sens: f32) -> i32 {
    original + ((start - y) * sens) as i32
}

/// The state of one vertical drag from pointer down to pointer up.
#[derive(Clone, Debug, PartialEq)]
pub struct VerticalDrag {
    config: Config,
    width: f32,
    levels: Levels,
    /// `startingY`: anchor for the device and brightness branches. `None` until the first event.
    start_y: Option<f32>,
    /// `mpvVolumeStartingY`: anchor for the boost branch.
    mpv_start_y: Option<f32>,
    original_device: i32,
    original_mpv: i32,
    original_brightness: f32,
    /// Last event: the pill it showed and when, in milliseconds.
    last_event: Option<(Channel, u64)>,
}

impl VerticalDrag {
    /// Starts a drag. Nothing is written until the first event. A disabled or locked drag ignores
    /// every event.
    pub fn start(levels: Levels, width: f32, config: Config) -> Self {
        Self {
            config,
            width,
            levels,
            start_y: None,
            mpv_start_y: None,
            original_device: levels.device,
            original_mpv: levels.mpv,
            original_brightness: levels.brightness,
            last_event: None,
        }
    }

    /// Current levels, including every write made so far.
    pub fn levels(&self) -> Levels {
        self.levels
    }

    fn active(&self) -> bool {
        self.config.enabled && !self.config.locked
    }

    fn boost_on(&self) -> bool {
        self.config.boost_cap > 0 && self.levels.device == self.config.max_device
    }

    fn increasing_boost(&self, dy: f32) -> bool {
        self.boost_on() && (self.levels.mpv - MPV_BASE) < self.config.boost_cap && dy < 0.0
    }

    fn decreasing_boost(&self, dy: f32) -> bool {
        self.boost_on()
            && (1..=self.config.boost_cap).contains(&(self.levels.mpv - MPV_BASE))
            && dy > 0.0
    }

    /// One pointer event. Returns the write to apply, or `None` when the drag is inactive. The
    /// pill of the event is shown for `PILL_MS` after `now_ms`.
    pub fn on_drag(&mut self, x: f32, y: f32, dy: f32, now_ms: u64) -> Option<Write> {
        if !self.active() {
            return None;
        }
        let channel = channel_for(x, self.width, self.config.swap);
        self.last_event = Some((channel, now_ms));
        let write = match channel {
            Channel::Brightness => self.brightness(y),
            Channel::Volume => self.volume(y, dy),
        };
        self.apply(write);
        Some(write)
    }

    fn volume(&mut self, y: f32, dy: f32) -> Write {
        let sens = VOLUME_SENS_PER_PX * self.config.max_device as f32;
        if self.increasing_boost(dy) || self.decreasing_boost(dy) {
            let anchor = *self.mpv_start_y.get_or_insert_with(|| {
                self.start_y = None;
                self.original_device = self.levels.device;
                y
            });
            let mpv_sens = VOLUME_SENS_PER_PX * self.config.boost_cap as f32;
            let value = step_int(self.original_mpv, anchor, y, mpv_sens)
                .clamp(MPV_BASE, MPV_BASE + self.config.boost_cap);
            Write::Mpv(value)
        } else {
            let anchor = *self.start_y.get_or_insert_with(|| {
                self.mpv_start_y = None;
                self.original_mpv = self.levels.mpv;
                y
            });
            let value =
                step_int(self.original_device, anchor, y, sens).clamp(0, self.config.max_device);
            Write::Device(value)
        }
    }

    fn brightness(&mut self, y: f32) -> Write {
        let anchor = *self.start_y.get_or_insert(y);
        let value = self.original_brightness + (anchor - y) * BRIGHTNESS_SENS;
        Write::Brightness(value.clamp(BRIGHTNESS_MIN, BRIGHTNESS_MAX))
    }

    fn apply(&mut self, write: Write) {
        match write {
            Write::Device(v) => self.levels.device = v,
            Write::Mpv(v) => self.levels.mpv = v,
            Write::Brightness(v) => self.levels.brightness = v,
        }
    }

    /// The pill that should be up at `now_ms`: the channel of the last event, for `PILL_MS` after it.
    pub fn pill_at(&self, now_ms: u64) -> Option<Channel> {
        self.last_event
            .filter(|(_, at)| now_ms.saturating_sub(*at) < PILL_MS)
            .map(|(channel, _)| channel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mpv_brightness_is_clamped_and_scaled_to_a_hundred() {
        assert_eq!(mpv_brightness(-1.0), -75);
        assert_eq!(mpv_brightness(0.0), 0);
        assert_eq!(mpv_brightness(0.33), 33);
        assert_eq!(mpv_brightness(2.0), 100);
    }

    fn config() -> Config {
        Config {
            enabled: true,
            locked: false,
            swap: false,
            max_device: 15,
            boost_cap: 30,
        }
    }

    fn levels(device: i32, mpv: i32, brightness: f32) -> Levels {
        Levels {
            device,
            mpv,
            brightness,
        }
    }

    const W: f32 = 1000.0;

    #[test]
    fn left_half_is_brightness_and_right_half_is_volume() {
        assert_eq!(channel_for(100.0, W, false), Channel::Brightness);
        assert_eq!(channel_for(900.0, W, false), Channel::Volume);
        assert_eq!(channel_for(900.0, W, true), Channel::Brightness);
        assert_eq!(channel_for(100.0, W, true), Channel::Volume);
    }

    fn brightness_of(write: Option<Write>) -> f32 {
        match write {
            Some(Write::Brightness(b)) => b,
            other => panic!("expected a brightness write, got {other:?}"),
        }
    }

    #[test]
    fn brightness_moves_a_thousandth_per_pixel_and_clamps_to_range() {
        let mut drag = VerticalDrag::start(levels(7, 100, 0.5), W, config());
        assert!((brightness_of(drag.on_drag(100.0, 400.0, 0.0, 0)) - 0.5).abs() < 1e-6);
        assert!((brightness_of(drag.on_drag(100.0, 300.0, -100.0, 10)) - 0.6).abs() < 1e-5);
        let down = brightness_of(drag.on_drag(100.0, 1400.0, 1100.0, 20));
        assert!((down + 0.5).abs() < 1e-5);
        assert_eq!(window_brightness(down), 0.0);
        assert!((veil_alpha(down) - 0.5).abs() < 1e-5);
        let mut far = VerticalDrag::start(levels(7, 100, 0.0), W, config());
        far.on_drag(100.0, 0.0, 0.0, 0);
        assert_eq!(brightness_of(far.on_drag(100.0, 5000.0, 5000.0, 1)), -0.75);
        assert_eq!(brightness_of(far.on_drag(100.0, -5000.0, -10000.0, 2)), 1.0);
    }

    #[test]
    fn volume_is_truncated_steps_of_max_times_a_thousandth() {
        let mut drag = VerticalDrag::start(levels(3, 100, 0.5), W, config());
        assert_eq!(drag.on_drag(900.0, 500.0, 0.0, 0), Some(Write::Device(3)));
        // 67 px is one device step at max 15 (67 * 0.015 = 1.005).
        assert_eq!(drag.on_drag(900.0, 433.0, -67.0, 1), Some(Write::Device(4)));
        assert_eq!(
            drag.on_drag(900.0, 200.0, -233.0, 2),
            Some(Write::Device(7))
        );
        assert_eq!(
            drag.on_drag(900.0, 9000.0, 8800.0, 3),
            Some(Write::Device(0))
        );
    }

    #[test]
    fn swap_moves_brightness_to_the_right() {
        let mut swapped = config();
        swapped.swap = true;
        let mut drag = VerticalDrag::start(levels(3, 100, 0.5), W, swapped);
        assert_eq!(
            drag.on_drag(900.0, 500.0, 0.0, 0),
            Some(Write::Brightness(0.5))
        );
    }

    #[test]
    fn disabled_or_locked_drags_do_nothing() {
        let mut off = config();
        off.enabled = false;
        let mut drag = VerticalDrag::start(levels(3, 100, 0.5), W, off);
        assert_eq!(drag.on_drag(900.0, 500.0, 0.0, 0), None);
        let mut locked = config();
        locked.locked = true;
        let mut drag = VerticalDrag::start(levels(3, 100, 0.5), W, locked);
        assert_eq!(drag.on_drag(100.0, 500.0, 0.0, 0), None);
        assert_eq!(drag.pill_at(0), None);
    }

    #[test]
    fn upward_drag_at_device_max_boosts_and_stops_at_the_cap() {
        let mut drag = VerticalDrag::start(levels(15, 100, 0.5), W, config());
        assert_eq!(drag.on_drag(900.0, 500.0, -1.0, 0), Some(Write::Mpv(100)));
        // 0.03 per pixel of boost sensitivity: 100 px up is +3.
        assert_eq!(drag.on_drag(900.0, 400.0, -100.0, 1), Some(Write::Mpv(103)));
        assert_eq!(
            drag.on_drag(900.0, -10000.0, -10000.0, 2),
            Some(Write::Mpv(130))
        );
        assert_eq!(drag.levels().device, 15);
    }

    #[test]
    fn dragging_down_reverses_boost_back_to_the_base() {
        let mut drag = VerticalDrag::start(levels(15, 110, 0.5), W, config());
        // The first event anchors and writes the starting value.
        assert_eq!(drag.on_drag(900.0, 500.0, 1.0, 0), Some(Write::Mpv(110)));
        assert_eq!(drag.on_drag(900.0, 600.0, 100.0, 1), Some(Write::Mpv(107)));
        assert_eq!(
            drag.on_drag(900.0, 9000.0, 9000.0, 2),
            Some(Write::Mpv(100))
        );
    }

    #[test]
    fn no_boost_without_a_cap_or_below_device_max() {
        let mut none = config();
        none.boost_cap = 0;
        let mut drag = VerticalDrag::start(levels(15, 100, 0.5), W, none);
        assert_eq!(
            drag.on_drag(900.0, 500.0, -50.0, 0),
            Some(Write::Device(15))
        );
        let mut drag = VerticalDrag::start(levels(14, 100, 0.5), W, config());
        assert_eq!(
            drag.on_drag(900.0, 500.0, -50.0, 0),
            Some(Write::Device(14))
        );
    }

    #[test]
    fn pill_shows_for_two_seconds_after_the_last_event() {
        let mut drag = VerticalDrag::start(levels(3, 100, 0.5), W, config());
        drag.on_drag(100.0, 500.0, 0.0, 1000);
        assert_eq!(drag.pill_at(1000), Some(Channel::Brightness));
        assert_eq!(drag.pill_at(2999), Some(Channel::Brightness));
        assert_eq!(drag.pill_at(3000), None);
        drag.on_drag(900.0, 500.0, 0.0, 2500);
        assert_eq!(drag.pill_at(4000), Some(Channel::Volume));
    }
}
