//! Progress ring (T-49a-025): indeterminate spinner at progress 0, determinate arc otherwise.
//!
//! Input rules (E-49-36, TEST-49-25): NaN and exactly 0 are indeterminate; other values clamp to
//! [0, 1]. The two modes crossfade in a `gtk::Stack`. The rotation stops when the ring is unmapped
//! (R-49-07) and the spinner is not started in the determinate mode.

use std::cell::{Cell, RefCell};
use std::f64::consts::FRAC_PI_2;
use std::rc::Rc;

use gtk::prelude::*;

use super::classes;
use crate::theme::motion;
use crate::theme::tokens::PROGRESS_ROTATION_MS;

/// Crossfade between the two modes, in ms.
const CROSSFADE_MS: u32 = 200;
/// Edge length of the ring in px.
const RING_SIZE: i32 = 24;
/// Arc stroke width in px.
const STROKE: f64 = 4.0;
/// The static arc of the indeterminate ring when reduced motion is on: a quarter turn.
const STATIC_SWEEP: f64 = std::f64::consts::FRAC_PI_2;

/// What the ring shows for a progress value.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RingState {
    Indeterminate,
    Determinate(f32),
}

/// Maps a raw progress value to the ring state (E-49-36).
pub fn ring_state(progress: f32) -> RingState {
    if progress.is_nan() || progress == 0.0 {
        RingState::Indeterminate
    } else {
        RingState::Determinate(progress.clamp(0.0, 1.0))
    }
}

/// The rotation in degrees after `elapsed_ms`, one turn per `PROGRESS_ROTATION_MS`.
pub fn rotation_deg(elapsed_ms: u64) -> f64 {
    let period = PROGRESS_ROTATION_MS as u64;
    (elapsed_ms % period) as f64 / period as f64 * 360.0
}

/// A progress ring widget and its state. `set_progress` updates the visible mode.
pub struct ProgressRing {
    pub widget: gtk::Stack,
    progress: Rc<Cell<f32>>,
    spinner: gtk::Spinner,
    area: gtk::DrawingArea,
    /// True when reduced motion is on: no spinner, no rotation, a static arc (T-49a-057).
    still: bool,
}

impl ProgressRing {
    /// Creates the ring at `progress` (see `ring_state` for the rules).
    pub fn new(initial: f32) -> Self {
        let progress = Rc::new(Cell::new(0.0_f32));
        let still = !motion::ring_spins(motion::enabled());
        let still_flag = Rc::new(Cell::new(still));
        let spinner = gtk::Spinner::new();
        spinner.set_size_request(RING_SIZE, RING_SIZE);
        let area = gtk::DrawingArea::new();
        area.set_content_width(RING_SIZE);
        area.set_content_height(RING_SIZE);
        area.add_css_class(classes::PROGRESS_RING);

        let widget = gtk::Stack::new();
        widget.set_transition_type(gtk::StackTransitionType::Crossfade);
        widget.set_transition_duration(motion::duration_ms(CROSSFADE_MS, !still));
        widget.add_named(&spinner, Some("indeterminate"));
        widget.add_named(&area, Some("determinate"));

        let rotation = Rc::new(Cell::new(0.0_f64));
        let start: Rc<Cell<Option<i64>>> = Rc::new(Cell::new(None));
        {
            let progress = progress.clone();
            let rotation = rotation.clone();
            let still_flag = still_flag.clone();
            area.set_draw_func(move |area, cr, w, h| {
                let color = area.color();
                let (cx, cy) = (w as f64 / 2.0, h as f64 / 2.0);
                let radius = (w.min(h) as f64) / 2.0 - STROKE;
                let sweep = match ring_state(progress.get()) {
                    RingState::Determinate(p) => p as f64 * std::f64::consts::TAU,
                    RingState::Indeterminate if still_flag.get() => STATIC_SWEEP,
                    RingState::Indeterminate => 0.0,
                };
                cr.translate(cx, cy);
                cr.rotate(rotation.get().to_radians());
                cr.translate(-cx, -cy);
                cr.set_source_rgba(
                    color.red() as f64,
                    color.green() as f64,
                    color.blue() as f64,
                    color.alpha() as f64,
                );
                cr.set_line_width(STROKE);
                cr.new_path();
                cr.arc(cx, cy, radius, -FRAC_PI_2, -FRAC_PI_2 + sweep);
                let _ = cr.stroke();
            });
        }

        // The tick callback runs while the ring is mapped (R-49-07).
        let tick: Rc<RefCell<Option<gtk::TickCallbackId>>> = Rc::new(RefCell::new(None));
        {
            let tick = tick.clone();
            let rotation = rotation.clone();
            let start = start.clone();
            let still_flag = still_flag.clone();
            area.connect_map(move |area| {
                if tick.borrow().is_some() || still_flag.get() {
                    return;
                }
                let rotation = rotation.clone();
                let start = start.clone();
                let id = area.add_tick_callback(move |w, clock| {
                    let now = clock.frame_time();
                    let origin = match start.get() {
                        Some(t) => t,
                        None => {
                            start.set(Some(now));
                            now
                        }
                    };
                    let elapsed_ms = ((now - origin).max(0) / 1000) as u64;
                    rotation.set(rotation_deg(elapsed_ms));
                    w.queue_draw();
                    gtk::glib::ControlFlow::Continue
                });
                *tick.borrow_mut() = Some(id);
            });
        }
        {
            let tick = tick.clone();
            area.connect_unmap(move |_| {
                if let Some(id) = tick.borrow_mut().take() {
                    id.remove();
                }
            });
        }

        let ring = ProgressRing {
            widget,
            progress,
            spinner,
            area,
            still,
        };
        ring.set_progress(initial);
        ring
    }

    /// Sets the progress value and switches the visible mode.
    pub fn set_progress(&self, value: f32) {
        self.progress.set(value);
        match ring_state(value) {
            RingState::Indeterminate if !self.still => {
                self.spinner.start();
                self.widget.set_visible_child_name("indeterminate");
            }
            RingState::Indeterminate => {
                self.spinner.stop();
                self.widget.set_visible_child_name("determinate");
                self.area.queue_draw();
            }
            RingState::Determinate(_) => {
                self.spinner.stop();
                self.widget.set_visible_child_name("determinate");
                self.area.queue_draw();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_state_matches_the_spec_table() {
        // TEST-49-25: 0, 0.5, 1.0, 1.2, -0.1, NaN -> indeterminate, 0.5, 1.0, 1.0, 0.0, indeterminate.
        assert_eq!(ring_state(0.0), RingState::Indeterminate);
        assert_eq!(ring_state(0.5), RingState::Determinate(0.5));
        assert_eq!(ring_state(1.0), RingState::Determinate(1.0));
        assert_eq!(ring_state(1.2), RingState::Determinate(1.0));
        assert_eq!(ring_state(-0.1), RingState::Determinate(0.0));
        assert_eq!(ring_state(f32::NAN), RingState::Indeterminate);
    }

    #[test]
    fn rotation_is_one_turn_per_period() {
        assert_eq!(rotation_deg(0), 0.0);
        assert_eq!(rotation_deg(1000), 180.0);
        assert_eq!(rotation_deg(2000), 0.0);
        assert_eq!(rotation_deg(2500), 90.0);
    }
}
