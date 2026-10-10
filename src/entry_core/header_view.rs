//! The info header widget over `header.rs`: cover, title, author, artist, status and source. A click on a line runs
//! its search, a right click or a long press copies it, and a stub source shows a warning icon in the error color.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Label, Orientation as O};

use crate::entry_core::action_row::{self, Tone, tone_alpha};
use crate::entry_core::header::{Field, HeaderModel, SearchRequest, cover_size};
use crate::entry_core::cover_view::CoverView;
use crate::entry_core::list_view::ensure_css;

/// Draws a button of the action row in `tone`: the primary colour at full opacity, or the foreground at 0.38 alpha.
pub fn apply_tone(widget: &impl IsA<gtk::Widget>, tone: Tone) {
    widget.set_opacity(tone_alpha(tone));
    if tone == Tone::Primary {
        widget.add_css_class("chapter-primary");
    } else {
        widget.remove_css_class("chapter-primary");
    }
}

/// The library button of an entry page: a heart and "In library" for a favorite, an empty star and "Add to library"
/// otherwise. It is a plain button with an icon and a label as its child, because a button holds either its own
/// label or its own icon, not both. The primary colour marks a favorite, the foreground at 0.38 alpha the rest
/// (`action_row::library`), and a long press opens the category editor, for a favorite only (E-13).
#[derive(Clone)]
pub struct LibraryButton {
    pub button: gtk::Button,
    icon: gtk::Image,
    label: Label,
    content: GBox,
    favorite: Rc<Cell<bool>>,
}

impl LibraryButton {
    pub fn new() -> Self {
        ensure_css();
        let icon = gtk::Image::new();
        let label = Label::new(None);
        let content = GBox::new(O::Horizontal, 8);
        content.append(&icon);
        content.append(&label);
        let button = gtk::Button::builder().child(&content).css_classes(["flat"]).halign(Align::Start).margin_start(8).margin_end(8).focus_on_click(false).build();
        let this = Self { button, icon, label, content, favorite: Rc::new(Cell::new(false)) };
        this.set(false);
        this
    }

    pub fn set(&self, favorite: bool) {
        self.favorite.set(favorite);
        let shown = action_row::library(favorite);
        self.label.set_label(&kuro_i18n::tr(shown.label));
        self.icon.set_icon_name(Some(if shown.filled { "emblem-favorite-symbolic" } else { "non-starred-symbolic" }));
        apply_tone(&self.content, shown.tone);
    }

    /// Runs `on_click` on a click and `on_long_press` on a long press. The long press counts only while the entry is a
    /// favorite; then the release that follows it is not a click. Otherwise the press is a plain click.
    pub fn connect(&self, on_click: impl Fn() + 'static, on_long_press: impl Fn() + 'static) {
        let long_fired = Rc::new(Cell::new(false));
        let press = gtk::GestureClick::builder().propagation_phase(gtk::PropagationPhase::Capture).build();
        press.connect_pressed({
            let long_fired = Rc::clone(&long_fired);
            move |_, _, _, _| long_fired.set(false)
        });
        self.button.add_controller(press);
        let long = gtk::GestureLongPress::new();
        long.connect_pressed({
            let (favorite, long_fired) = (Rc::clone(&self.favorite), Rc::clone(&long_fired));
            move |_, _, _| {
                if action_row::library_long_press_opens_categories(favorite.get()) {
                    long_fired.set(true);
                    on_long_press();
                }
            }
        });
        self.button.add_controller(long);
        self.button.connect_clicked(move |_| {
            if !long_fired.replace(false) {
                on_click();
            }
        });
    }
}

/// What the header reports back to the page.
#[derive(Clone)]
pub struct Handlers {
    pub on_search: Rc<dyn Fn(SearchRequest)>,
    pub on_copy: Rc<dyn Fn(String)>,
    pub on_cover: Rc<dyn Fn()>,
}

/// A label that searches on click and copies on right click or long press, for the fields that have text.
fn field_label(model: &HeaderModel, field: Field, text: &str, css: &[&str], xalign: f32, h: &Handlers) -> Label {
    let label = Label::builder().label(text).xalign(xalign).wrap(true).css_classes(css.to_vec()).build();
    if let Some(request) = model.search(field) {
        label.set_cursor_from_name(Some("pointer"));
        let click = gtk::GestureClick::builder().button(1).build();
        let (on_search, long_fired) = (Rc::clone(&h.on_search), Rc::new(Cell::new(false)));
        let skip = Rc::clone(&long_fired);
        click.connect_released(move |_, _, _, _| {
            if !skip.replace(false) {
                on_search(request.clone());
            }
        });
        label.add_controller(click);
        if let Some(copy) = model.copy(field) {
            let long = gtk::GestureLongPress::new();
            let on_copy = Rc::clone(&h.on_copy);
            let text = copy.clone();
            long.connect_pressed(move |_, _, _| {
                long_fired.set(true);
                on_copy(text.clone());
            });
            label.add_controller(long);
        }
    }
    if let Some(copy) = model.copy(field) {
        let right = gtk::GestureClick::builder().button(3).build();
        let on_copy = Rc::clone(&h.on_copy);
        right.connect_pressed(move |_, _, _, _| on_copy(copy.clone()));
        label.add_controller(right);
    }
    label
}

fn icon_row(icon: &str, content: &impl IsA<gtk::Widget>, centered: bool) -> GBox {
    let row = GBox::new(O::Horizontal, 6);
    row.set_halign(if centered { Align::Center } else { Align::Start });
    row.append(&gtk::Image::from_icon_name(icon));
    row.append(content);
    row
}

/// Builds the header. The small layout puts the cover beside the text, the large layout puts a wider cover above
/// centered text. `column_width` is the width of the column that holds the header.
pub fn build(model: &HeaderModel, large: bool, column_width: i32, h: &Handlers) -> GBox {
    let (w, ht) = cover_size(large, column_width);
    let cover_view = CoverView::new(w, ht);
    if let Some(art) = &model.cover {
        cover_view.load(art.clone());
    }
    let cover = cover_view.frame.clone();
    cover.set_valign(Align::Start);
    let on_cover = Rc::clone(&h.on_cover);
    let tap = gtk::GestureClick::new();
    tap.connect_released(move |_, _, _, _| on_cover());
    cover.add_controller(tap);

    let xalign = if large { 0.5 } else { 0.0 };
    let info = GBox::new(O::Vertical, 4);
    info.set_hexpand(true);
    info.set_valign(Align::End);
    info.append(&field_label(model, Field::Title, &model.title, &["title-2"], xalign, h));
    let author = field_label(model, Field::Author, &model.author, &[], xalign, h);
    info.append(&icon_row("avatar-default-symbolic", &author, large));
    if let Some(artist) = &model.artist {
        let artist = field_label(model, Field::Artist, artist, &[], xalign, h);
        info.append(&icon_row("document-edit-symbolic", &artist, large));
    }
    info.append(&icon_row(model.status_icon, &Label::builder().label(&model.status).css_classes(["dim-label"]).build(), large));
    let source_row = GBox::new(O::Horizontal, 6);
    source_row.set_halign(if large { Align::Center } else { Align::Start });
    source_row.append(&gtk::Image::from_icon_name("network-server-symbolic"));
    source_row.append(&field_label(model, Field::Source, &model.source, &["dim-label"], xalign, h));
    if model.stub {
        // the stock `error` class colors the icon like an error label
        source_row.append(&gtk::Image::builder().icon_name("dialog-warning-symbolic").css_classes(["error"]).build());
    }
    info.append(&source_row);

    let root = GBox::new(if large { O::Vertical } else { O::Horizontal }, 16);
    if large {
        cover.set_halign(Align::Center);
    }
    root.append(&cover);
    root.append(&info);
    root
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// One display test: the button draws a favorite in the primary colour at full opacity and anything else at 0.38,
    /// a long press runs only for a favorite and then swallows the click that follows, and a plain click always runs.
    #[test]
    fn library_button_tone_and_long_press() {
        crate::testing::with_display(|| {
            let button = LibraryButton::new();
            let content = button.button.child().expect("the content box");
            assert!((content.opacity() - 0.38).abs() < 0.01, "not in the library: dimmed (GTK keeps opacity in 8 bits)");
            assert!(!content.has_css_class("chapter-primary"));
            let clicks = Rc::new(Cell::new(0));
            let longs = Rc::new(Cell::new(0));
            button.connect(
                {
                    let clicks = Rc::clone(&clicks);
                    move || clicks.set(clicks.get() + 1)
                },
                {
                    let longs = Rc::clone(&longs);
                    move || longs.set(longs.get() + 1)
                },
            );
            let long_press = || {
                let controllers = button.button.observe_controllers();
                let gesture = (0..controllers.n_items())
                    .filter_map(|i| controllers.item(i).and_downcast::<gtk::GestureLongPress>())
                    .next()
                    .expect("the long press gesture");
                gesture.emit_by_name::<()>("pressed", &[&0.0f64, &0.0f64]);
            };
            long_press();
            assert_eq!(longs.get(), 0, "no long press for an entry outside the library (E-13)");
            button.set(true);
            assert_eq!(content.opacity(), 1.0, "a favorite: full opacity");
            assert!(content.has_css_class("chapter-primary"), "and the primary colour");
            long_press();
            assert_eq!(longs.get(), 1);
            button.button.emit_clicked();
            assert_eq!(clicks.get(), 0, "the release after a long press is not a click");
            button.button.emit_clicked();
            assert_eq!(clicks.get(), 1, "a plain click runs");
        });
    }
}
