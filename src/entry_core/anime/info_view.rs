//! The info box widget: the shared header (`header_view`: cover, title, author, artist, status, source, and the
//! warning icon in the error colour for a stub source) over a backdrop. The small layout puts a 100 px cover beside
//! the text; the large layout puts a cover at 65% of its column above centred text (`header::cover_size`).
//! The backdrop is the cover placeholder at 0.2 opacity fading out downward; the image loader is not in the tree yet.

use gtk::prelude::*;

use crate::entry_core::header::HeaderModel;
use crate::entry_core::header_view::{self, Handlers};

/// Opacity of the backdrop at its top edge.
const BACKDROP_ALPHA: f64 = 0.2;

fn backdrop() -> gtk::DrawingArea {
    let area = gtk::DrawingArea::new();
    area.set_can_target(false);
    area.set_draw_func(|area, cr, w, h| {
        let color = area.color();
        let (r, g, b) = (f64::from(color.red()), f64::from(color.green()), f64::from(color.blue()));
        let fade = gtk::cairo::LinearGradient::new(0.0, 0.0, 0.0, f64::from(h));
        fade.add_color_stop_rgba(0.0, r, g, b, BACKDROP_ALPHA);
        fade.add_color_stop_rgba(1.0, r, g, b, 0.0);
        let _ = cr.set_source(&fade);
        cr.rectangle(0.0, 0.0, f64::from(w), f64::from(h));
        let _ = cr.fill();
    });
    area
}

/// Builds the info box. `large` and `column_width` pick the cover size.
pub fn build(model: &HeaderModel, large: bool, column_width: i32, handlers: &Handlers) -> gtk::Overlay {
    let content = header_view::build(model, large, column_width, handlers);
    content.set_margin_start(16);
    content.set_margin_end(16);
    content.set_margin_top(16);
    content.set_margin_bottom(8);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&backdrop()));
    overlay.add_overlay(&content);
    overlay.set_measure_overlay(&content, true);
    overlay
}
