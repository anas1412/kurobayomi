//! Player window root (T-53a-055): the video renders in a `gtk::GLArea` with the controls overlay above it.
//!
//! The mpv handle and its render context belong to the player. This module builds the surface and passes the
//! realize and render callbacks through. Immersive mode hides the sidebar and bar (`layout::visibility`).

use gtk::prelude::*;

/// The player root: `overlay` holds the render area with the controls on top.
pub struct PlayerRoot {
    pub overlay: gtk::Overlay,
    pub area: gtk::GLArea,
}

/// The surface size in device pixels for a widget of `width` x `height` logical pixels at `scale`.
pub fn surface_size(width: i32, height: i32, scale: i32) -> (i32, i32) {
    (width.max(0) * scale.max(1), height.max(0) * scale.max(1))
}

/// Builds the root. `on_realize` runs once the GL context exists. `on_render(width, height)` runs for each frame with the
/// surface size in device pixels, and returns nothing (the frame is always handled).
pub fn build(controls: &impl IsA<gtk::Widget>, on_realize: impl Fn(&gtk::GLArea) + 'static, on_render: impl Fn(i32, i32) + 'static) -> PlayerRoot {
    let area = gtk::GLArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    area.set_auto_render(true);
    area.connect_realize(move |a| on_realize(a));
    area.connect_render(move |a, _context| {
        let (w, h) = surface_size(a.width(), a.height(), a.scale_factor());
        on_render(w, h);
        gtk::glib::Propagation::Stop
    });
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&area));
    controls.set_halign(gtk::Align::Fill);
    controls.set_valign(gtk::Align::End);
    overlay.add_overlay(controls);
    PlayerRoot { overlay, area }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_size_is_in_device_pixels() {
        assert_eq!(surface_size(640, 360, 1), (640, 360));
        assert_eq!(surface_size(640, 360, 2), (1280, 720));
        assert_eq!(surface_size(-1, 10, 0), (0, 10), "a negative size and a zero scale are clamped");
    }
}
