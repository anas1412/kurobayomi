//! The frame the webtoon strip sits in (T-44a-038, B§10.7). Upstream scales and moves the whole list view. Here the
//! strip's scrolled window is the frame's only child: the frame gives it the viewport height `original / scale` below a
//! scale of 1 (so the strip still fills the frame once it is scaled down) and draws it scaled about its centre and moved
//! by the pan. GTK applies the same transform to the pointer, so a tap lands where it is drawn.

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::ObjectSubclassIsExt;

/// The size the strip is allocated for a frame of `width` by `height` at `scale`: taller below 1 (E-31).
pub fn strip_size(width: i32, height: i32, scale: f64) -> (i32, i32) {
    if scale > 0.0 && scale < 1.0 { (width, (f64::from(height) / scale) as i32) } else { (width, height) }
}

mod imp {
    use std::cell::Cell;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{glib, graphene, gsk};

    pub struct ZoomFrame {
        pub scale: Cell<f64>,
        pub pan: Cell<(f64, f64)>,
    }

    impl Default for ZoomFrame {
        fn default() -> Self {
            Self { scale: Cell::new(1.0), pan: Cell::new((0.0, 0.0)) }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for ZoomFrame {
        const NAME: &'static str = "KuroWebtoonZoomFrame";
        type Type = super::ZoomFrame;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for ZoomFrame {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.set_hexpand(true);
            obj.set_vexpand(true);
            // a strip scaled above 1 reaches past the frame
            obj.set_overflow(gtk::Overflow::Hidden);
        }

        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for ZoomFrame {
        fn measure(&self, orientation: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            match self.obj().first_child() {
                Some(child) => child.measure(orientation, for_size),
                None => (0, 0, -1, -1),
            }
        }

        fn size_allocate(&self, width: i32, height: i32, baseline: i32) {
            let Some(child) = self.obj().first_child() else { return };
            let (scale, (px, py)) = (self.scale.get(), self.pan.get());
            let (w, h) = super::strip_size(width, height, scale);
            let transform = (scale != 1.0 || px != 0.0 || py != 0.0).then(|| {
                let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
                gsk::Transform::new()
                    .translate(&graphene::Point::new(px as f32, py as f32))
                    .translate(&graphene::Point::new(cx, cy))
                    .scale(scale as f32, scale as f32)
                    .translate(&graphene::Point::new(-cx, -cy))
            });
            child.allocate(w, h, baseline, transform);
        }
    }
}

glib::wrapper! {
    pub struct ZoomFrame(ObjectSubclass<imp::ZoomFrame>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl ZoomFrame {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let this: Self = glib::Object::new();
        child.set_parent(&this);
        this
    }

    /// Shows the strip at `scale`, moved by `pan` (pixels, after the scale).
    pub fn set_view(&self, scale: f64, pan: (f64, f64)) {
        let state = self.imp();
        let (old_scale, old_pan) = (state.scale.replace(scale), state.pan.replace(pan));
        if old_scale != scale || old_pan != pan {
            self.queue_allocate();
        }
    }

    pub fn view(&self) -> (f64, (f64, f64)) {
        let state = self.imp();
        (state.scale.get(), state.pan.get())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_one_the_strip_is_taller_by_the_inverse_scale() {
        assert_eq!(strip_size(800, 600, 1.0), (800, 600));
        assert_eq!(strip_size(800, 600, 0.5), (800, 1200));
        assert_eq!(strip_size(800, 600, 2.0), (800, 600), "above one the strip keeps the frame's height");
        assert_eq!(strip_size(800, 600, 0.0), (800, 600), "a zero scale never divides");
    }

    #[test]
    fn the_strip_is_allocated_taller_when_zoomed_out_and_back_when_reset() {
        crate::testing::with_display(|| {
            let child = gtk::Box::new(gtk::Orientation::Vertical, 0);
            child.set_hexpand(true);
            child.set_vexpand(true);
            let frame = ZoomFrame::new(&child);
            let window = gtk::Window::new();
            window.set_default_size(400, 300);
            window.set_child(Some(&frame));
            window.present();
            let ctx = glib::MainContext::default();
            let pump = || {
                for _ in 0..50 {
                    while ctx.iteration(false) {}
                    std::thread::sleep(std::time::Duration::from_millis(4));
                }
            };
            pump();
            assert_eq!(child.height(), frame.height());
            frame.set_view(0.5, (0.0, -150.0));
            pump();
            assert_eq!(child.height(), frame.height() * 2);
            frame.set_view(1.0, (0.0, 0.0));
            pump();
            assert_eq!(child.height(), frame.height());
            window.destroy();
        });
    }
}
