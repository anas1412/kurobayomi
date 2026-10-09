// AdwClamp without libadwaita: the child gets at most `max` px of width, centered, and can
// shrink down to its own minimum so narrow windows still work.
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use std::cell::Cell;

mod imp {
    use super::*;

    #[derive(Default)]
    pub struct Clamp {
        pub max: Cell<i32>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Clamp {
        const NAME: &'static str = "KurobayomiClamp";
        type Type = super::Clamp;
        type ParentType = gtk::Widget;
    }

    impl ObjectImpl for Clamp {
        fn dispose(&self) {
            while let Some(c) = self.obj().first_child() {
                c.unparent();
            }
        }
    }

    impl WidgetImpl for Clamp {
        fn request_mode(&self) -> gtk::SizeRequestMode {
            gtk::SizeRequestMode::HeightForWidth
        }

        fn measure(&self, o: gtk::Orientation, for_size: i32) -> (i32, i32, i32, i32) {
            let Some(c) = self.obj().first_child() else { return (0, 0, -1, -1) };
            let max = self.max.get();
            let for_size = if o == gtk::Orientation::Vertical && for_size > max { max } else { for_size };
            let (min, nat, _, _) = c.measure(o, for_size);
            let nat = if o == gtk::Orientation::Horizontal { nat.min(max).max(min) } else { nat };
            (min, nat, -1, -1)
        }

        fn size_allocate(&self, width: i32, height: i32, _baseline: i32) {
            let Some(c) = self.obj().first_child() else { return };
            let cw = width.min(self.max.get()).max(c.measure(gtk::Orientation::Horizontal, -1).0);
            c.size_allocate(&gtk::Allocation::new((width - cw) / 2, 0, cw, height), -1);
        }
    }
}

glib::wrapper! {
    pub struct Clamp(ObjectSubclass<imp::Clamp>) @extends gtk::Widget, @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

pub fn clamp(child: &impl IsA<gtk::Widget>, max: i32) -> Clamp {
    let c: Clamp = glib::Object::new();
    c.imp().max.set(max);
    child.set_parent(&c);
    c
}
