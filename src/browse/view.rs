//! Thin GTK widgets for the browse primitives. Every rule they show comes from the pure modules next to this file.

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Image, Label, Orientation as O, Spinner};
use kuro_prefs::library::LibraryDisplayMode;
use kuro_i18n::tr;

use std::rc::Rc;

use super::grid::{CELL_MARGIN_PX, LIST_COVER_PX, badge_visible, cell_alpha};
use super::icons::{ICON_HEIGHT_PX, SourceIcon, load_icon, placeholder_rgba};
use super::listing::ListItem;
use super::primitives::{EmptyAction, SPINNER_PADDING_PX, SPINNER_SIZE_PX, ToastAction, ToastDuration, ToastSpec};

/// A centred spinner row with the padding of the design sheet.
pub fn spinner_row() -> GBox {
    let row = GBox::new(O::Vertical, 0);
    row.set_halign(Align::Center);
    row.set_margin_top(SPINNER_PADDING_PX);
    row.set_margin_bottom(SPINNER_PADDING_PX);
    let spinner = Spinner::new();
    spinner.set_size_request(SPINNER_SIZE_PX, SPINNER_SIZE_PX);
    spinner.set_spinning(true);
    row.append(&spinner);
    row
}

/// A centred message with its action buttons, in the order given. `on_action` runs for the button pressed.
pub fn empty_state(message: &str, actions: &[EmptyAction], on_action: impl Fn(EmptyAction) + Clone + 'static) -> GBox {
    let column = GBox::new(O::Vertical, 12);
    column.set_halign(Align::Center);
    column.set_valign(Align::Center);
    column.set_vexpand(true);
    column.append(&Label::new(Some(message)));
    let row = GBox::new(O::Horizontal, 8);
    row.set_halign(Align::Center);
    for action in actions.iter().copied().take(super::primitives::EMPTY_ACTION_LIMIT) {
        let button = Button::with_label(&action.label());
        button.set_child(Some(&Image::from_icon_name(action.icon())));
        button.set_tooltip_text(Some(&action.label()));
        let callback = on_action.clone();
        button.connect_clicked(move |_| callback(action));
        row.append(&button);
    }
    column.append(&row);
    column
}

/// The bottom toast bar: the message, an optional retry action, and a dismiss button. It hides itself only when the
/// duration is not indefinite; the caller runs the timer.
pub fn toast_bar(spec: &ToastSpec, on_retry: impl Fn() + 'static, on_dismiss: impl Fn() + 'static) -> GBox {
    let bar = GBox::new(O::Horizontal, 8);
    bar.set_halign(Align::Center);
    bar.set_valign(Align::End);
    bar.add_css_class("toast");
    bar.append(&Label::new(Some(&spec.message)));
    if spec.action == Some(ToastAction::Retry) {
        let retry = Button::with_label(&tr(kuro_i18n::MsgKey::action_retry));
        retry.connect_clicked(move |_| on_retry());
        bar.append(&retry);
    }
    let dismiss = Button::from_icon_name("window-close-symbolic");
    dismiss.set_tooltip_text(Some(&tr(kuro_i18n::MsgKey::action_cancel)));
    dismiss.connect_clicked(move |_| on_dismiss());
    bar.append(&dismiss);
    bar
}

/// Whether a toast needs a timer, and for how long.
pub fn toast_timeout_ms(spec: &ToastSpec) -> Option<u32> {
    match spec.duration {
        ToastDuration::Seconds(s) => Some(s * 1000),
        ToastDuration::Indefinite => None,
    }
}

/// The icon of a row, 40 px square. A built-in glyph is tinted with the error colour for a stub or an untrusted
/// extension. The file of an installed extension shows the `PLACEHOLDER_ARGB` fill while it is read and decoded on a
/// blocking thread, then the picture; a file that cannot be read leaves the default glyph (T-41a-004).
pub fn icon_image(icon: SourceIcon, file: Option<&str>) -> Image {
    let image = match (icon.icon_name(), file) {
        (Some(name), _) => Image::from_icon_name(name),
        (None, None) => Image::from_icon_name(SourceIcon::Default.icon_name().unwrap_or("image-missing")),
        (None, Some(file)) => {
            let image = Image::new();
            let [r, g, b, a] = placeholder_rgba();
            image.set_paintable(Some(&texture_of(1, 1, &[r, g, b, a])));
            let (weak, path) = (image.downgrade(), std::path::PathBuf::from(file));
            gtk::glib::spawn_future_local(async move {
                let decoded = gtk::gio::spawn_blocking(move || load_icon(&path)).await.ok().flatten();
                let Some(image) = weak.upgrade() else { return };
                match decoded {
                    Some(icon) => image.set_paintable(Some(&texture_of(icon.width, icon.height, &icon.rgba))),
                    None => image.set_icon_name(SourceIcon::Default.icon_name()),
                }
            });
            image
        }
    };
    image.set_pixel_size(ICON_HEIGHT_PX);
    if icon.is_error() {
        image.add_css_class("error");
    }
    image
}

fn texture_of(width: u32, height: u32, rgba: &[u8]) -> gtk::gdk::MemoryTexture {
    gtk::gdk::MemoryTexture::new(
        width as i32,
        height as i32,
        gtk::gdk::MemoryFormat::R8g8b8a8,
        &gtk::glib::Bytes::from(rgba),
        width as usize * 4,
    )
}

/// A source row: a 40 px icon, the name on one line with ellipsis, the language when shown, "Not installed" in the
/// error colour for a stub, and an action slot.
pub fn source_row(
    name: &str,
    language: Option<&str>,
    icon: SourceIcon,
    icon_file: Option<&str>,
    stub: bool,
    action: Option<&gtk::Widget>,
) -> GBox {
    let row = GBox::new(O::Horizontal, 16);
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    row.append(&icon_image(icon, icon_file));
    let text = GBox::new(O::Vertical, 2);
    text.set_hexpand(true);
    let title = Label::new(Some(name));
    title.set_xalign(0.0);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&title);
    if let Some(language) = language {
        let lang = Label::new(Some(language));
        lang.set_xalign(0.0);
        lang.add_css_class("dim-label");
        text.append(&lang);
    }
    if stub {
        let note = Label::new(Some(&tr(kuro_i18n::MsgKey::not_installed)));
        note.set_xalign(0.0);
        note.add_css_class("error");
        text.append(&note);
    }
    row.append(&text);
    if let Some(action) = action {
        row.append(action);
    }
    row
}

/// What a cover load calls when it has the picture.
pub type CoverDone = Box<dyn FnOnce(gtk::gdk::Paintable)>;

/// Loads the cover at a url or path and hands the picture to `CoverDone`. The app decides how (network, cache); a failed
/// load never calls it and leaves the placeholder.
pub type CoverLoader = Rc<dyn Fn(&str, CoverDone)>;

/// True while the cell that asked for a cover is still bound to the item it asked for (edge R10).
pub type CoverGuard = Rc<dyn Fn() -> bool>;

/// A guard that always accepts, for cells that are never recycled.
pub fn always() -> CoverGuard {
    Rc::new(|| true)
}

/// The loader for covers that are files on disk, such as the local source's. The file is read off the GTK thread.
pub fn file_cover_loader() -> CoverLoader {
    Rc::new(|path, done| {
        let path = path.to_owned();
        gtk::glib::spawn_future_local(async move {
            let read = gtk::gio::spawn_blocking(move || std::fs::read(path)).await;
            if let Ok(Ok(bytes)) = read {
                if let Ok(texture) = gtk::gdk::Texture::from_bytes(&gtk::glib::Bytes::from_owned(bytes)) {
                    done(texture.upcast());
                }
            }
        });
    })
}

/// A cover: the placeholder frame with the picture over it. Only this widget carries the favourite alpha, so the
/// title and the badge next to it stay at full strength (B§2.7).
fn cover_widget(item: &ListItem, width: i32, height: i32, load: &CoverLoader, guard: &CoverGuard) -> gtk::Overlay {
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&crate::ui::cover(width, height)));
    if let Some(url) = item.cover.as_deref() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        let (weak, guard) = (picture.downgrade(), Rc::clone(guard));
        load(url, Box::new(move |paintable| {
            // a cover that arrives after its cell was rebound belongs to another item
            if let (true, Some(picture)) = (guard(), weak.upgrade()) {
                picture.set_paintable(Some(&paintable));
            }
        }));
        overlay.add_overlay(&picture);
    }
    overlay.set_opacity(cell_alpha(item.favorite));
    overlay
}

fn in_library_badge() -> Image {
    let badge = Image::from_icon_name("bookmark-new-symbolic");
    badge.set_halign(Align::Start);
    badge.set_valign(Align::Start);
    badge.set_margin_start(4);
    badge.set_margin_top(4);
    badge
}

fn title_label(text: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_lines(2);
    label.set_wrap(true);
    label.set_max_width_chars(14);
    label.set_xalign(0.0);
    label
}

/// A grid cell for the comfortable, compact or cover-only mode. A `None` slot is a placeholder and draws nothing.
/// The cell carries half the gap on every side; the container (see `listing_view`) carries the rest of the padding. The
/// cover is `cover_width` px wide and 2:3.
pub fn grid_cell(item: Option<&ListItem>, mode: LibraryDisplayMode, load: &CoverLoader, guard: &CoverGuard, cover_width: i32) -> GBox {
    let cell = GBox::new(O::Vertical, 4);
    for margin in [GBox::set_margin_start, GBox::set_margin_end, GBox::set_margin_top, GBox::set_margin_bottom] {
        margin(&cell, CELL_MARGIN_PX);
    }
    let Some(item) = item else { return cell };
    let stage = gtk::Overlay::new();
    stage.set_child(Some(&cover_widget(item, cover_width, cover_width * 3 / 2, load, guard)));
    if badge_visible(item.favorite) {
        stage.add_overlay(&in_library_badge());
    }
    match mode {
        LibraryDisplayMode::CompactGrid => {
            let title = title_label(&item.title);
            title.add_css_class("osd");
            title.set_valign(Align::End);
            stage.add_overlay(&title);
            cell.append(&stage);
        }
        LibraryDisplayMode::ComfortableGrid | LibraryDisplayMode::List => {
            cell.append(&stage);
            cell.append(&title_label(&item.title));
        }
        LibraryDisplayMode::CoverOnlyGrid => cell.append(&stage),
    }
    cell
}

/// A list row: a square cover, the title on one line, and the bookmark badge at the end for favourites.
pub fn list_cell(item: Option<&ListItem>, load: &CoverLoader, guard: &CoverGuard) -> GBox {
    let row = GBox::new(O::Horizontal, 12);
    row.set_margin_start(16);
    row.set_margin_end(16);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    let Some(item) = item else { return row };
    row.append(&cover_widget(item, LIST_COVER_PX, LIST_COVER_PX, load, guard));
    let title = Label::new(Some(&item.title));
    title.set_xalign(0.0);
    title.set_hexpand(true);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&title);
    if badge_visible(item.favorite) {
        row.append(&Image::from_icon_name("bookmark-new-symbolic"));
    }
    row
}

/// The long press and its desktop alias, the secondary click, without a primary click. The cells of the listing
/// activate through the view itself, so only these two gestures are added to them.
pub fn attach_long_press(widget: &impl IsA<gtk::Widget>, on_long: impl Fn() + 'static) {
    let on_long: Rc<dyn Fn()> = Rc::new(on_long);
    let long = gtk::GestureLongPress::new();
    long.connect_pressed({
        let on_long = Rc::clone(&on_long);
        move |gesture, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            on_long();
        }
    });
    widget.add_controller(long);
    let secondary = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_SECONDARY).build();
    secondary.connect_pressed(move |gesture, _, _, _| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        on_long();
    });
    widget.add_controller(secondary);
}

/// Primary click and long press are two gestures with two callbacks. A secondary click is the desktop alias of a long
/// press (U9). A long press claims its touch sequence, so it does not also count as a click when released.
pub fn attach_row_gestures(widget: &impl IsA<gtk::Widget>, on_primary: impl Fn() + 'static, on_long: impl Fn() + 'static) {
    let on_long: Rc<dyn Fn()> = Rc::new(on_long);

    let click = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_PRIMARY).build();
    click.connect_released(move |_, _, _, _| on_primary());
    widget.add_controller(click);

    let long = gtk::GestureLongPress::new();
    long.connect_pressed({
        let on_long = Rc::clone(&on_long);
        move |gesture, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            on_long();
        }
    });
    widget.add_controller(long);

    let secondary = gtk::GestureClick::builder().button(gtk::gdk::BUTTON_SECONDARY).build();
    secondary.connect_pressed({
        let on_long = Rc::clone(&on_long);
        move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            on_long();
        }
    });
    widget.add_controller(secondary);

    // the keyboard's way to a context menu: the Menu key, or Shift+F10
    widget.set_focusable(true);
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, state| {
        let menu = key == gtk::gdk::Key::Menu || (key == gtk::gdk::Key::F10 && state.contains(gtk::gdk::ModifierType::SHIFT_MASK));
        if menu {
            on_long();
        }
        if menu { gtk::glib::Propagation::Stop } else { gtk::glib::Propagation::Proceed }
    });
    widget.add_controller(keys);
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::Cell;

    use super::*;

    fn favorite(title: &str) -> ListItem {
        ListItem { title: title.into(), favorite: true, ..ListItem::default() }
    }

    fn loader() -> CoverLoader {
        Rc::new(|_, _| {})
    }

    fn a_placeholder_slot_renders_nothing() {
        for mode in LibraryDisplayMode::ALL {
            assert!(grid_cell(None, mode, &loader(), &always(), 136).first_child().is_none(), "{mode:?}");
        }
        assert!(list_cell(None, &loader(), &always()).first_child().is_none());
    }

    fn a_favourite_dims_only_its_cover_and_not_the_cell_or_the_title() {
        let cell = grid_cell(Some(&favorite("Berserk")), LibraryDisplayMode::ComfortableGrid, &loader(), &always(), 136);
        assert_eq!(cell.opacity(), 1.0, "the cell itself is not dimmed");
        let stage = cell.first_child().and_downcast::<gtk::Overlay>().expect("stage");
        let cover = stage.child().expect("cover");
        assert!((cover.opacity() - 0.34).abs() < 0.005, "GTK keeps opacity in 8 bits: {}", cover.opacity());
        let title = stage.next_sibling().and_downcast::<Label>().expect("title under the cover");
        assert_eq!(title.opacity(), 1.0, "the title keeps full strength");
    }

    fn a_cell_has_half_a_gap_of_margin_on_every_side() {
        let cell = grid_cell(None, LibraryDisplayMode::CompactGrid, &loader(), &always(), 136);
        assert_eq!((cell.margin_start(), cell.margin_end(), cell.margin_top(), cell.margin_bottom()), (2, 2, 2, 2));
    }

    fn cover_only_has_no_title_and_comfortable_has_one_under_the_cover() {
        let item = ListItem { title: "A".into(), ..ListItem::default() };
        let only = grid_cell(Some(&item), LibraryDisplayMode::CoverOnlyGrid, &loader(), &always(), 136);
        assert!(only.first_child().and_then(|c| c.next_sibling()).is_none());
        let comfortable = grid_cell(Some(&item), LibraryDisplayMode::ComfortableGrid, &loader(), &always(), 136);
        assert!(comfortable.last_child().is_some_and(|c| c.is::<Label>()));
    }

    fn the_cover_loader_is_called_with_the_item_cover() {
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let sink = Rc::clone(&seen);
        let load: CoverLoader = Rc::new(move |url, _| sink.borrow_mut().push(url.to_owned()));
        let item = ListItem { title: "A".into(), cover: Some("/covers/a.png".into()), ..ListItem::default() };
        let _ = grid_cell(Some(&item), LibraryDisplayMode::CompactGrid, &load, &always(), 136);
        let _ = list_cell(Some(&item), &load, &always());
        assert_eq!(*seen.borrow(), ["/covers/a.png", "/covers/a.png"]);
    }

    fn primary_click_and_long_press_call_different_callbacks() {
        let row = GBox::new(O::Horizontal, 0);
        let (clicks, longs) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let (c, l) = (Rc::clone(&clicks), Rc::clone(&longs));
        attach_row_gestures(&row, move || c.set(c.get() + 1), move || l.set(l.get() + 1));
        let mut controllers = Vec::new();
        let list = row.observe_controllers();
        for i in 0..list.n_items() {
            controllers.push(list.item(i).expect("controller"));
        }
        assert_eq!(controllers.len(), 4, "primary click, long press, secondary click and the menu key");
        for controller in &controllers {
            if let Some(click) = controller.downcast_ref::<gtk::GestureClick>() {
                if click.button() == gtk::gdk::BUTTON_PRIMARY {
                    click.emit_by_name::<()>("released", &[&1_i32, &0.0_f64, &0.0_f64]);
                } else {
                    click.emit_by_name::<()>("pressed", &[&1_i32, &0.0_f64, &0.0_f64]);
                }
            } else if let Some(long) = controller.downcast_ref::<gtk::GestureLongPress>() {
                long.emit_by_name::<()>("pressed", &[&0.0_f64, &0.0_f64]);
            } else if let Some(keys) = controller.downcast_ref::<gtk::EventControllerKey>() {
                let shift = gtk::gdk::ModifierType::SHIFT_MASK;
                let none = gtk::gdk::ModifierType::empty();
                assert!(keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Menu, &0_u32, &none]), "the Menu key is handled");
                assert!(keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::F10, &0_u32, &shift]), "so is Shift+F10");
                assert!(!keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::F10, &0_u32, &none]), "F10 alone is not");
            }
        }
        assert_eq!(clicks.get(), 1, "one primary click");
        assert_eq!(longs.get(), 4, "the long press, the right click and both menu keys reach the long-press callback");
    }

    /// Run by `smoke::browse_widgets_smoke`: GTK binds to the first thread that starts it, so every widget check
    /// of this lane runs in that one test.
    pub(crate) fn smoke() {
        a_placeholder_slot_renders_nothing();
        a_favourite_dims_only_its_cover_and_not_the_cell_or_the_title();
        a_cell_has_half_a_gap_of_margin_on_every_side();
        cover_only_has_no_title_and_comfortable_has_one_under_the_cover();
        the_cover_loader_is_called_with_the_item_cover();
        primary_click_and_long_press_call_different_callbacks();
    }
}
