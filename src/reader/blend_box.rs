//! A container that blends a colour over its child with a `gsk` blend mode. GTK's CSS has no `mix-blend-mode`, and a
//! plain overlay widget cannot see what is under it, so the blend is drawn here: the child is the bottom image of the
//! blend and the colour is the top image. Used for the colour filter layer (T-043a-066).

use gtk::glib;
use gtk::prelude::*;

mod imp {
    use std::cell::Cell;

    use gtk::prelude::*;
    use gtk::subclass::prelude::*;
    use gtk::{gdk, glib, graphene, gsk};

    #[derive(Default)]
    pub struct BlendBox {
        pub colour: Cell<Option<(f32, f32, f32, f32)>>,
        pub mode: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BlendBox {
        const NAME: &'static str = "KuroReaderBlendBox";
        type Type = super::BlendBox;
        type ParentType = gtk::Widget;

        fn class_init(class: &mut Self::Class) {
            class.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for BlendBox {
        fn dispose(&self) {
            while let Some(child) = self.obj().first_child() {
                child.unparent();
            }
        }
    }

    impl WidgetImpl for BlendBox {
        fn snapshot(&self, snapshot: &gtk::Snapshot) {
            let obj = self.obj();
            let Some(child) = obj.first_child() else { return };
            let Some((r, g, b, a)) = self.colour.get() else {
                obj.snapshot_child(&child, snapshot);
                return;
            };
            snapshot.push_blend(super::blend_mode(self.mode.get()));
            obj.snapshot_child(&child, snapshot);
            snapshot.pop();
            snapshot.append_color(&gdk::RGBA::new(r, g, b, a), &graphene::Rect::new(0.0, 0.0, obj.width() as f32, obj.height() as f32));
            snapshot.pop();
        }
    }
}

glib::wrapper! {
    pub struct BlendBox(ObjectSubclass<imp::BlendBox>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

/// The `gsk` blend mode of a colour filter mode index (`0..=5`: default, multiply, screen, overlay, lighten, darken).
/// An index outside the list is the default mode.
pub fn blend_mode(mode: i32) -> gtk::gsk::BlendMode {
    use gtk::gsk::BlendMode;
    match mode {
        1 => BlendMode::Multiply,
        2 => BlendMode::Screen,
        3 => BlendMode::Overlay,
        4 => BlendMode::Lighten,
        5 => BlendMode::Darken,
        _ => BlendMode::Default,
    }
}

impl BlendBox {
    pub fn new(child: &impl IsA<gtk::Widget>) -> Self {
        let this: Self = glib::Object::new();
        // a custom container does not forward its child's expand flags
        this.set_hexpand(true);
        this.set_vexpand(true);
        child.set_parent(&this);
        this
    }

    /// Sets the colour (red, green, blue, alpha in `0..=1`) and the mode index; `None` shows the child as is.
    pub fn set_filter(&self, colour: Option<(f64, f64, f64, f64)>, mode: i32) {
        use gtk::subclass::prelude::ObjectSubclassIsExt;
        let state = self.imp();
        state.colour.set(colour.map(|(r, g, b, a)| (r as f32, g as f32, b as f32, a as f32)));
        state.mode.set(mode);
        self.queue_draw();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gtk::gsk::BlendMode;

    #[test]
    fn the_six_modes_map_in_order_and_unknown_is_default() {
        let modes: Vec<BlendMode> = (0..6).map(blend_mode).collect();
        assert_eq!(
            modes,
            [BlendMode::Default, BlendMode::Multiply, BlendMode::Screen, BlendMode::Overlay, BlendMode::Lighten, BlendMode::Darken]
        );
        assert_eq!(blend_mode(9), BlendMode::Default);
        assert_eq!(blend_mode(-1), BlendMode::Default);
    }
}
