//! Wheel picker (T-49a-048): three visible rows, the middle one selected, and an alpha falloff from
//! the selected row. The pure rules (alpha, snapped index) are tested without a display.
//!
//! The wheel steps one row per drag distance. When a drag ends, the leftover offset snaps to the row
//! with the largest alpha, after a short delay that a new drag cancels (R-49-04). The number variant
//! opens a text entry on a double-click (E-49-11, E-49-12).
//!
//! ponytail: no fling inertia or animated snap; the snap is one jump. Add a tick-driven animation if
//! the jump feels abrupt.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;

use crate::theme::style::class;

/// Visible rows.
pub const ROWS: usize = 3;
/// Default size of the wheel in px (width and height).
pub const DEFAULT_SIZE: i32 = 128;
/// Corner radius of the selected-row highlight in px.
pub const HIGHLIGHT_RADIUS: i32 = 16;

/// Alpha of a row whose centre is `distance` px from the snap line, in a viewport `viewport` px tall
/// (`calculateAnimatedAlpha`). Inside the first third it falls from 1.0 (clamped from 1.2) to 0.2;
/// beyond it the alpha is 0.2.
pub fn row_alpha(distance: f64, viewport: f64) -> f64 {
    let single = viewport / 3.0;
    let d = distance.abs();
    if d <= single.trunc() {
        (1.2 - d / single).clamp(0.0, 1.0)
    } else {
        0.2
    }
}

/// The snapped index: the row with the largest alpha. Ties pick the first row.
pub fn snapped_index(alphas: &[f64]) -> Option<usize> {
    alphas
        .iter()
        .enumerate()
        .fold(None::<(usize, f64)>, |best, (i, &a)| match best {
            Some((_, b)) if a <= b => best,
            _ => Some((i, a)),
        })
        .map(|(i, _)| i)
}

/// Delay between the end of a drag and the snap, in ms.
pub const SNAP_DELAY_MS: u64 = 120;

/// Which row to snap to after a drag: `acc` px of leftover offset (positive = content dragged down)
/// moves the rows at offsets -1, 0 and 1 off the snap line. The row with the largest alpha wins and
/// the result is its offset from the current row. Ties pick the first (upper) row.
pub fn snap_offset(acc: f64, row: f64, viewport: f64) -> isize {
    let alphas: Vec<f64> = (-1..=1).map(|k| row_alpha((f64::from(k) * row + acc).abs(), viewport)).collect();
    snapped_index(&alphas).map_or(0, |i| i as isize - 1)
}

/// The row whose text equals `text` (trimmed), for the number entry. `None` when no row matches.
pub fn match_entry(items: &[String], text: &str) -> Option<usize> {
    let text = text.trim();
    items.iter().position(|item| item == text)
}

/// Orders the snap against drags: a snap is due only after a drag ended, and any later drag
/// cancels the ones scheduled before it.
#[derive(Default)]
pub struct SnapGate {
    dragging: Cell<bool>,
    epoch: Cell<u64>,
}

impl SnapGate {
    /// A drag starts: every pending snap is cancelled.
    pub fn begin(&self) {
        self.dragging.set(true);
        self.epoch.set(self.epoch.get() + 1);
    }

    /// The drag ends. The returned token is passed to [`SnapGate::due`] when the snap timer fires.
    pub fn end(&self) -> u64 {
        self.dragging.set(false);
        self.epoch.set(self.epoch.get() + 1);
        self.epoch.get()
    }

    /// True when the snap with `token` may run: no drag is in progress and none started since.
    pub fn due(&self, token: u64) -> bool {
        !self.dragging.get() && token == self.epoch.get()
    }
}

/// The wheel over `items`, starting at `start` (read once). `on_change` fires with the new index only
/// when the user moves the wheel, never at construction.
pub fn wheel(items: Vec<String>, start: usize, on_change: impl Fn(usize) + 'static) -> gtk::Box {
    build(items, start, false, on_change)
}

/// The number variant over `first..=last`: a double-click opens a text entry that jumps to the typed
/// number. `on_change` gets the number, not the index.
pub fn number_wheel(first: i32, last: i32, start: i32, on_change: impl Fn(i32) + 'static) -> gtk::Box {
    let items: Vec<String> = (first..=last.max(first)).map(|n| n.to_string()).collect();
    let at = usize::try_from(start.saturating_sub(first)).unwrap_or(0);
    build(items, at, true, move |i| on_change(first.saturating_add(i as i32)))
}

fn build(items: Vec<String>, start: usize, manual_entry: bool, on_change: impl Fn(usize) + 'static) -> gtk::Box {
    let last = items.len().saturating_sub(1);
    let index = Rc::new(Cell::new(start.min(last)));
    let items = Rc::new(items);
    let on_change: Rc<dyn Fn(usize)> = Rc::new(on_change);
    let row_h = DEFAULT_SIZE / ROWS as i32;

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.set_size_request(DEFAULT_SIZE, DEFAULT_SIZE);
    root.add_css_class(class::WHEEL);
    let labels: Rc<Vec<gtk::Label>> = Rc::new(
        (0..ROWS)
            .map(|row| {
                let label = gtk::Label::new(None);
                label.set_height_request(row_h);
                label.set_hexpand(true);
                if row == ROWS / 2 {
                    label.add_css_class(class::WHEEL_SELECTED);
                }
                root.append(&label);
                label
            })
            .collect(),
    );
    render(&labels, &items, index.get());

    let last_dy = Rc::new(Cell::new(0.0_f64));
    let accum = Rc::new(RefCell::new(0.0_f64));
    let gate = Rc::new(SnapGate::default());
    let drag = gtk::GestureDrag::new();
    {
        let gate = gate.clone();
        drag.connect_drag_begin(move |_, _, _| gate.begin());
    }
    {
        let (labels, items, index, last_dy, accum, on_change) =
            (labels.clone(), items.clone(), index.clone(), last_dy.clone(), accum.clone(), on_change.clone());
        drag.connect_drag_update(move |_, _, dy| {
            let delta = dy - last_dy.get();
            last_dy.set(dy);
            let mut acc = accum.borrow_mut();
            *acc += delta;
            let mut moved = false;
            let row = f64::from(row_h);
            while acc.abs() >= row {
                // dragging up (negative) moves to the next row; the accumulator shrinks toward zero
                let step: isize = if *acc < 0.0 { 1 } else { -1 };
                let next = (index.get() as isize + step).clamp(0, last as isize) as usize;
                if next == index.get() {
                    *acc = 0.0;
                    break;
                }
                index.set(next);
                moved = true;
                *acc += if step > 0 { row } else { -row };
            }
            drop(acc);
            if moved {
                render(&labels, &items, index.get());
                on_change(index.get());
            }
        });
    }
    {
        let (labels, items, index, last_dy, accum, on_change, gate) =
            (labels.clone(), items.clone(), index.clone(), last_dy.clone(), accum.clone(), on_change.clone(), gate.clone());
        drag.connect_drag_end(move |_, _, _| {
            last_dy.set(0.0);
            let token = gate.end();
            let (labels, items, index, accum, on_change, gate) =
                (labels.clone(), items.clone(), index.clone(), accum.clone(), on_change.clone(), gate.clone());
            gtk::glib::timeout_add_local_once(Duration::from_millis(SNAP_DELAY_MS), move || {
                if !gate.due(token) {
                    return;
                }
                let acc = accum.replace(0.0);
                let offset = snap_offset(acc, f64::from(row_h), f64::from(DEFAULT_SIZE));
                let next = (index.get() as isize + offset).clamp(0, last as isize) as usize;
                if next != index.get() {
                    index.set(next);
                    render(&labels, &items, next);
                    on_change(next);
                }
            });
        });
    }
    root.add_controller(drag);
    if manual_entry {
        add_manual_entry(&root, &labels, &items, &index, &on_change);
    }
    root
}

/// A double-click opens an entry in a popover. Enter commits a matching row; no match closes without
/// a change.
fn add_manual_entry(
    root: &gtk::Box,
    labels: &Rc<Vec<gtk::Label>>,
    items: &Rc<Vec<String>>,
    index: &Rc<Cell<usize>>,
    on_change: &Rc<dyn Fn(usize)>,
) {
    let entry = gtk::Entry::new();
    entry.set_input_purpose(gtk::InputPurpose::Digits);
    EditableExt::set_alignment(&entry, 0.5);
    let popover = gtk::Popover::new();
    popover.set_child(Some(&entry));
    popover.set_parent(root);
    {
        let (labels, items, index, on_change, popover) =
            (labels.clone(), items.clone(), index.clone(), on_change.clone(), popover.clone());
        entry.connect_activate(move |e| {
            if let Some(i) = match_entry(&items, &e.text()) {
                if i != index.get() {
                    index.set(i);
                    render(&labels, &items, i);
                    on_change(i);
                }
            }
            popover.popdown();
        });
    }
    let click = gtk::GestureClick::new();
    {
        let (items, index, entry, popover) = (items.clone(), index.clone(), entry.clone(), popover.clone());
        click.connect_pressed(move |_, presses, _, _| {
            if presses == 2 {
                entry.set_text(items.get(index.get()).map_or("", String::as_str));
                popover.popup();
                entry.grab_focus();
            }
        });
    }
    root.add_controller(click);
    root.connect_destroy(move |_| popover.unparent());
}

/// Shows the three rows around `index`, with the alpha of each row.
fn render(labels: &[gtk::Label], items: &[String], index: usize) {
    for (row, label) in labels.iter().enumerate() {
        let offset = row as isize - (ROWS / 2) as isize;
        let at = index as isize + offset;
        let text = usize::try_from(at).ok().and_then(|i| items.get(i)).map_or("", String::as_str);
        label.set_text(text);
        let distance = offset.unsigned_abs() as f64 * f64::from(DEFAULT_SIZE / ROWS as i32);
        label.set_opacity(row_alpha(distance, f64::from(DEFAULT_SIZE)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // TEST-49-17, viewport 300 px (single = 100).
    #[test]
    fn alpha_falls_from_the_centre_to_the_boundary() {
        assert_eq!(row_alpha(0.0, 300.0), 1.0);
        assert!((row_alpha(50.0, 300.0) - 0.7).abs() < 1e-9);
        assert!((row_alpha(100.0, 300.0) - 0.2).abs() < 1e-9);
        assert_eq!(row_alpha(150.0, 300.0), 0.2);
    }

    // TEST-49-18: the snapped row is the max-alpha row; ties pick the first.
    #[test]
    fn snapped_row_is_the_largest_alpha_and_ties_pick_the_first() {
        assert_eq!(snapped_index(&[0.2, 1.0, 0.2]), Some(1));
        assert_eq!(snapped_index(&[0.5, 0.5, 0.2]), Some(0));
        assert_eq!(snapped_index(&[]), None);
    }

    #[test]
    fn snap_goes_to_the_nearest_row() {
        // 128 px viewport, 42 px rows; a small leftover returns to the current row
        assert_eq!(snap_offset(0.0, 42.0, 128.0), 0);
        assert_eq!(snap_offset(10.0, 42.0, 128.0), 0);
        assert_eq!(snap_offset(-10.0, 42.0, 128.0), 0);
        // dragged down past half a row: the previous row is nearer the snap line
        assert_eq!(snap_offset(30.0, 42.0, 128.0), -1);
        assert_eq!(snap_offset(-30.0, 42.0, 128.0), 1);
    }

    // R-49-04: a drag in progress cancels a pending snap; a snap starts only after drag end.
    #[test]
    fn a_new_drag_cancels_a_pending_snap() {
        let gate = SnapGate::default();
        gate.begin();
        assert!(!gate.due(gate.epoch.get()), "no snap during a drag");
        let first = gate.end();
        assert!(gate.due(first));
        gate.begin();
        assert!(!gate.due(first), "a drag in progress cancels the pending snap");
        let second = gate.end();
        assert!(!gate.due(first), "an older snap stays cancelled");
        assert!(gate.due(second));
    }

    // E-49-11, E-49-12
    #[test]
    fn typed_text_picks_the_matching_row_only() {
        let items: Vec<String> = (0..60).map(|n| n.to_string()).collect();
        assert_eq!(match_entry(&items, "17"), Some(17));
        assert_eq!(match_entry(&items, " 5 "), Some(5));
        assert_eq!(match_entry(&items, "99"), None);
        assert_eq!(match_entry(&items, ""), None);
        assert_eq!(match_entry(&items, "x"), None);
    }

    #[test]
    fn highlight_geometry() {
        assert_eq!((DEFAULT_SIZE, HIGHLIGHT_RADIUS, ROWS), (128, 16, 3));
    }
}
