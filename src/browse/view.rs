//! Thin GTK widgets for the browse primitives. Every rule they show comes from the pure modules next to this file.

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Image, Label, Orientation as O, Spinner};
use kuro_prefs::library::LibraryDisplayMode;
use kuro_i18n::tr;

use std::path::Path;
use std::rc::Rc;

use super::grid::{CELL_MARGIN_PX, LIST_COVER_PX, badge_visible, cell_alpha};
use super::icons::{ICON_HEIGHT_PX, IconRef, SourceIcon, fallback_glyph, fetch_icon, http_get, icon_cache_root, icon_ref, load_icon, placeholder_rgba};
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

/// The spinner row of a Sources tab with its "Loading sources…" line under the spinner.
pub fn loading_sources_row() -> GBox {
    let row = spinner_row();
    row.set_spacing(12);
    row.append(&Label::new(Some(super::texts::LOADING_SOURCES)));
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
        // icon and words: a person who does not know the icon still reads what the button does
        let content = GBox::new(O::Horizontal, 6);
        content.append(&Image::from_icon_name(action.icon()));
        content.append(&Label::new(Some(&action.label())));
        let button = Button::new();
        button.set_child(Some(&content));
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
/// extension. The file of an installed extension shows the placeholder fill while it is read and decoded on a
/// blocking thread, then the picture; a file that cannot be read leaves the default glyph (T-41a-004). The icon of an
/// available extension is a url: the same placeholder shows while it is fetched through the image cache, and a failed
/// fetch leaves the error glyph (T-40a-027).
pub fn icon_image(icon: SourceIcon, file: Option<&str>) -> Image {
    icon_image_sized(icon, file, ICON_HEIGHT_PX)
}

/// `icon_image` at `px` square: 40 in a row, 112 on the details page.
pub fn icon_image_sized(icon: SourceIcon, file: Option<&str>, px: i32) -> Image {
    let image = match (icon.icon_name(), file) {
        (Some(name), _) => Image::from_icon_name(name),
        (None, None) => Image::from_icon_name(SourceIcon::Default.icon_name().unwrap_or("image-missing")),
        (None, Some(file)) => {
            let image = Image::new();
            let [r, g, b, a] = placeholder_rgba();
            image.set_paintable(Some(&texture_of(1, 1, &[r, g, b, a])));
            let (weak, source) = (image.downgrade(), file.to_owned());
            gtk::glib::spawn_future_local(async move {
                let url = matches!(icon_ref(&source), IconRef::Url(_));
                let decoded = gtk::gio::spawn_blocking(move || match icon_ref(&source) {
                    IconRef::Url(url) => fetch_icon(icon_cache_root().as_deref(), url, http_get),
                    IconRef::File(file) => load_icon(std::path::Path::new(file)),
                })
                .await
                .ok()
                .flatten();
                let Some(image) = weak.upgrade() else { return };
                match decoded {
                    Some(icon) => image.set_paintable(Some(&texture_of(icon.width, icon.height, &icon.rgba))),
                    None => {
                        let (glyph, error) = fallback_glyph(url);
                        image.set_icon_name(Some(glyph));
                        if error {
                            image.add_css_class("error");
                        }
                    }
                }
            });
            image
        }
    };
    image.set_pixel_size(px);
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

/// The saved PNG icon of a repo source: 40 px square and cover-fitted, with the letter tile's rounded corners.
pub fn repo_picture(path: &Path) -> gtk::Image {
    // an Image at a fixed pixel size: a Picture would ask for the file's own size (192 px) and stretch the row
    let image = gtk::Image::from_file(path);
    image.set_pixel_size(ICON_HEIGHT_PX);
    image.add_css_class("kb-row-cover");
    image.set_overflow(gtk::Overflow::Hidden);
    image.set_halign(Align::Start);
    image.set_valign(Align::Center);
    image
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
    source_row_with(name, language, icon, icon_file, None, stub, false, action)
}

/// [`source_row`] for the Sources lists: `picture` is the saved icon of a repo source, shown in place of the letter tile;
/// `failing` adds the "Not working right now" line of a source that failed several times in a row
/// (`errors::is_failing`).
#[allow(clippy::too_many_arguments)]
pub fn source_row_with(
    name: &str,
    language: Option<&str>,
    icon: SourceIcon,
    icon_file: Option<&str>,
    picture: Option<&Path>,
    stub: bool,
    failing: bool,
    action: Option<&gtk::Widget>,
) -> GBox {
    // a row of a boxed list (`look`): the list draws the frame, the row keeps the 12 px inset of the other lists
    let row = GBox::new(O::Horizontal, 12);
    row.set_margin_start(12);
    row.set_margin_end(8);
    row.set_margin_top(8);
    row.set_margin_bottom(8);
    if let Some(path) = picture {
        row.append(&repo_picture(path));
    } else if icon == SourceIcon::Letter && icon_file.is_none() {
        row.append(&super::look::row_cover(name, None, ICON_HEIGHT_PX, ICON_HEIGHT_PX));
    } else {
        row.append(&icon_image(icon, icon_file));
    }
    let text = GBox::new(O::Vertical, 2);
    text.set_hexpand(true);
    text.set_valign(Align::Center);
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
    if failing && !stub {
        let note = Label::new(Some(super::errors::NOT_WORKING_BADGE));
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

/// One cover to load: its url or path, the source of the listed entry (whose image headers a remote cover is sent
/// with), and the picture that shows it (a load still running when the picture is destroyed is cancelled).
pub struct CoverAsk<'a> {
    pub url: &'a str,
    pub source_id: i64,
    pub picture: &'a gtk::Picture,
}

/// Loads the cover asked for and hands the picture to `CoverDone`. The app decides how (network, cache); a failed
/// load never calls it and leaves the placeholder.
pub type CoverLoader = Rc<dyn Fn(&CoverAsk, CoverDone)>;

/// True while the cell that asked for a cover is still bound to the item it asked for (edge R10).
pub type CoverGuard = Rc<dyn Fn() -> bool>;

/// A guard that always accepts, for cells that are never recycled.
pub fn always() -> CoverGuard {
    Rc::new(|| true)
}

/// The loader of the listing covers. A file on disk, such as the local source's, is read off the GTK thread; an http(s)
/// url goes through the app's image service (network, disk cache and decode off the GTK thread). Without the service a
/// remote cover keeps its placeholder.
pub fn file_cover_loader() -> CoverLoader {
    Rc::new(|ask, done| {
        if ask.url.starts_with("https://") || ask.url.starts_with("http://") {
            if let (Some(service), Ok(runtime)) = (crate::entry_core::cover_view::service(), tokio::runtime::Handle::try_current()) {
                remote_cover(service, &runtime, remote_art(ask.url, ask.source_id), ask.picture, done);
            }
            return;
        }
        let path = ask.url.to_owned();
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

/// A listed entry's remote cover. The entry is not a library entry yet (entry id 0, not a favourite), so the image goes to
/// the shared image cache only and no cover file is written; the source id picks the source's image headers.
pub(super) fn remote_art(url: &str, source_id: i64) -> kuro_images::art::ArtRef {
    use kuro_images::art::{ArtKind, ArtRef, Owner};
    ArtRef { owner: Owner::Manga, entry_id: 0, source_id, kind: ArtKind::Cover, url: Some(url.to_owned()), favorite: false, last_modified: 0 }
}

/// Loads `art` through the image service. The load is cancelled when `picture` is destroyed (its cell was rebound or its
/// page closed), which frees its blocking-pool thread, and a cancelled load never calls `done`.
fn remote_cover(service: std::sync::Arc<kuro_images::fetch::ImageService>, runtime: &tokio::runtime::Handle, art: kuro_images::art::ArtRef, picture: &gtk::Picture, done: CoverDone) {
    use kuro_images::fetch::LoadRequest;

    let cancel = tokio_util::sync::CancellationToken::new();
    picture.connect_destroy({
        let cancel = cancel.clone();
        move |_| cancel.cancel()
    });
    let (sender, receiver) = async_channel::bounded(1);
    // the service's future is not `Send`, so it runs to the end on a blocking-pool thread
    let driver = runtime.clone();
    let request = LoadRequest { art, options: Default::default(), cancel: cancel.clone() };
    runtime.spawn_blocking(move || {
        let _ = sender.send_blocking(driver.block_on(service.load(&request)));
    });
    gtk::glib::spawn_future_local(async move {
        if let Ok(Ok(loaded)) = receiver.recv().await
            && !cancel.is_cancelled()
        {
            let spec = kuro_images::texture::TextureSpec::from_decoded(&loaded.image);
            let bytes = gtk::glib::Bytes::from(&spec.pixels[..]);
            let texture = gtk::gdk::MemoryTexture::new(spec.width, spec.height, gtk::gdk::MemoryFormat::R8g8b8a8, &bytes, spec.stride);
            done(texture.upcast());
        }
    });
}

/// A cover: the placeholder frame with the picture over it. Only this widget carries the favourite alpha, so the
/// title and the badge next to it stay at full strength (B§2.7).
fn cover_widget(item: &ListItem, width: i32, height: i32, load: &CoverLoader, guard: &CoverGuard) -> gtk::Overlay {
    // the library card's look: rounded corners, and the title's initials until the picture draws
    let overlay = gtk::Overlay::new();
    overlay.add_css_class("kb-card-cover");
    overlay.set_overflow(gtk::Overflow::Hidden);
    overlay.set_child(Some(&super::look::cover(&item.title, None, width, height)));
    if let Some(url) = item.cover.as_deref() {
        let picture = gtk::Picture::new();
        picture.set_content_fit(gtk::ContentFit::Cover);
        picture.set_can_shrink(true);
        let (weak, guard) = (picture.downgrade(), Rc::clone(guard));
        let ask = CoverAsk { url, source_id: item.source_id, picture: &picture };
        load(&ask, Box::new(move |paintable| {
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

/// The badge of an entry already in the library, in words.
fn in_library_badge() -> Label {
    let badge = super::look::badge(&tr(kuro_i18n::MsgKey::in_library), true);
    badge.set_halign(Align::Start);
    badge.set_valign(Align::Start);
    badge.set_margin_start(4);
    badge.set_margin_top(4);
    badge
}

fn title_label(text: &str, lines: i32) -> Label {
    let label = Label::new(Some(text));
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_lines(lines);
    label.set_wrap(true);
    label.set_max_width_chars(14);
    label.set_xalign(0.0);
    label
}

/// A grid cell for the comfortable, compact or cover-only mode. A `None` slot is a placeholder and draws nothing.
/// The cell carries half the gap on every side; the container (see `listing_view`) carries the rest of the padding. The
/// cover is `cover_width` px wide and 2:3.
pub fn grid_cell(item: Option<&ListItem>, mode: LibraryDisplayMode, load: &CoverLoader, guard: &CoverGuard, cover_width: i32) -> GBox {
    grid_cell_lines(item, mode, load, guard, cover_width, 2)
}

/// `grid_cell` with `title_lines` lines of title: the listing shows two, a global-search card three (T-40a-011).
pub fn grid_cell_lines(
    item: Option<&ListItem>,
    mode: LibraryDisplayMode,
    load: &CoverLoader,
    guard: &CoverGuard,
    cover_width: i32,
    title_lines: i32,
) -> GBox {
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
            let title = title_label(&item.title, title_lines);
            title.add_css_class("kb-card-title-scrim");
            title.set_valign(Align::End);
            title.set_halign(Align::Fill);
            stage.add_overlay(&title);
            cell.append(&stage);
        }
        LibraryDisplayMode::ComfortableGrid | LibraryDisplayMode::List => {
            cell.append(&stage);
            let title = title_label(&item.title, title_lines);
            title.add_css_class("kb-card-title");
            cell.append(&title);
        }
        LibraryDisplayMode::CoverOnlyGrid => cell.append(&stage),
    }
    cell
}

/// A list row: a square cover, the title on one line, and the In library badge at the end for favourites.
pub fn list_cell(item: Option<&ListItem>, load: &CoverLoader, guard: &CoverGuard) -> GBox {
    let row = GBox::new(O::Horizontal, 12);
    row.set_margin_start(12);
    row.set_margin_end(12);
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
        row.append(&super::look::badge(&tr(kuro_i18n::MsgKey::in_library), true));
    }
    row
}

/// The long press and its desktop aliases, the secondary click and the Menu key or Shift+F10, without a primary click.
/// The cells of the listing activate through the view itself, so no primary click is added to them.
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
    secondary.connect_pressed({
        let on_long = Rc::clone(&on_long);
        move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            on_long();
        }
    });
    widget.add_controller(secondary);
    attach_menu_key(widget, on_long);
}

/// A boxed-list row around `child` with two callbacks: activating the row (a click, Enter or Space, through
/// `widgets::row_activate`) runs `on_primary`; the long press, its desktop alias the secondary click (U9), and the Menu
/// key or Shift+F10 run `on_long`. A long press claims its touch sequence, so it does not also activate the row.
pub fn list_row(child: &impl IsA<gtk::Widget>, on_primary: impl Fn() + 'static, on_long: impl Fn() + 'static) -> gtk::ListBoxRow {
    let row = gtk::ListBoxRow::builder().child(child).build();
    crate::widgets::row_activate::on_activate(&row, move |_| on_primary());
    attach_long_press(&row, on_long);
    row
}

/// The keyboard's way to a context menu: the Menu key, or Shift+F10. The widget takes focus so the keys reach it.
pub fn attach_menu_key(widget: &impl IsA<gtk::Widget>, on_menu: Rc<dyn Fn()>) {
    widget.set_focusable(true);
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(move |_, key, _, state| {
        let menu = key == gtk::gdk::Key::Menu || (key == gtk::gdk::Key::F10 && state.contains(gtk::gdk::ModifierType::SHIFT_MASK));
        if menu {
            on_menu();
        }
        if menu { gtk::glib::Propagation::Stop } else { gtk::glib::Propagation::Proceed }
    });
    widget.add_controller(keys);
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn a_source_without_an_icon_shows_a_letter_tile_and_one_with_its_own_icon_keeps_it() {
        crate::testing::with_display(|| {
            let letter = source_row("MangaDex", None, SourceIcon::Letter, None, false, None);
            let tile = letter.first_child().expect("the icon slot");
            assert!(tile.has_css_class("kb-cover-initials") && tile.has_css_class("kb-row-cover"));
            assert_eq!(tile.first_child().and_downcast::<Label>().expect("the letter").label(), "M");
            let plain = source_row("Local", None, SourceIcon::Local, None, false, None);
            assert!(plain.first_child().is_some_and(|w| w.is::<Image>()));
        });
    }

    #[test]
    fn a_repo_source_with_a_saved_icon_shows_that_picture_in_place_of_the_letter_tile() {
        crate::testing::with_display(|| {
            let dir = tempfile::tempdir().expect("dir");
            let file = dir.path().join("icon.png");
            image::RgbaImage::from_pixel(2, 2, image::Rgba([1, 2, 3, 255])).save(&file).expect("a png");
            let row = source_row_with("Repo", None, SourceIcon::Letter, None, Some(&file), false, false, None);
            let picture = row.first_child().and_downcast::<gtk::Image>().expect("the saved icon");
            assert!(picture.has_css_class("kb-row-cover"), "the letter tile's rounded corners");
            assert_eq!(picture.pixel_size(), ICON_HEIGHT_PX);
        });
    }

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
        let load: CoverLoader = Rc::new(move |ask: &CoverAsk, _| sink.borrow_mut().push((ask.url.to_owned(), ask.source_id)));
        let item = ListItem { title: "A".into(), source_id: 41, cover: Some("/covers/a.png".into()), ..ListItem::default() };
        let _ = grid_cell(Some(&item), LibraryDisplayMode::CompactGrid, &load, &always(), 136);
        let _ = list_cell(Some(&item), &load, &always());
        let want = ("/covers/a.png".to_owned(), 41);
        assert_eq!(*seen.borrow(), [want.clone(), want], "the url and the listed entry's source");
    }

    /// The http branch: the cover goes out through the image service. Destroying its picture (the cell was rebound or
    /// the page closed) cancels the request in flight, and the picture is never drawn. The server is a local socket
    /// that takes the request and never answers.
    fn a_remote_cover_is_cancelled_with_its_picture_and_never_drawn() {
        use std::io::Read;
        use std::sync::Arc;

        struct NoHeaders;
        impl kuro_images::fetch::SourceHttp for NoHeaders {
            fn http_source(&self, _: i64) -> Option<kuro_images::fetch::SourceClient> {
                None
            }
        }
        let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().expect("runtime");
        let paths = kuro_testkit::TempPaths::new().expect("paths");
        let client = crate::net_client::build(&kuro_prefs::network::NetworkPrefs::new(kuro_prefs::store::PrefStore::in_memory()), &paths).expect("client");
        let service = rt.block_on(kuro_images::fetch::ImageService::new(client, Arc::new(NoHeaders), Arc::new(paths))).expect("service");
        let server = std::net::TcpListener::bind("127.0.0.1:0").expect("a local socket");
        let url = format!("http://{}/cover.jpg", server.local_addr().expect("address"));

        let holder = GBox::new(O::Vertical, 0);
        let picture = gtk::Picture::new();
        holder.append(&picture);
        let drawn = Rc::new(Cell::new(false));
        let flag = Rc::clone(&drawn);
        remote_cover(Arc::new(service), rt.handle(), remote_art(&url, 41), &picture, Box::new(move |_| flag.set(true)));
        let (mut request, _) = server.accept().expect("the cover request reaches the service's client");
        holder.remove(&picture);
        drop(picture);
        request.set_read_timeout(Some(std::time::Duration::from_secs(5))).expect("timeout");
        let mut buffer = [0_u8; 4096];
        let closed = loop {
            match request.read(&mut buffer) {
                Ok(0) => break true,
                Ok(_) => continue,
                Err(_) => break false,
            }
        };
        assert!(closed, "the destroyed picture cancels the request in flight");
        let until = std::time::Instant::now() + std::time::Duration::from_millis(300);
        while std::time::Instant::now() < until {
            gtk::glib::MainContext::default().iteration(false);
        }
        assert!(!drawn.get(), "a cancelled cover never reaches its picture");
    }

    fn primary_click_and_long_press_call_different_callbacks() {
        let (clicks, longs) = (Rc::new(Cell::new(0)), Rc::new(Cell::new(0)));
        let (c, l) = (Rc::clone(&clicks), Rc::clone(&longs));
        let row = list_row(&GBox::new(O::Horizontal, 0), move || c.set(c.get() + 1), move || l.set(l.get() + 1));
        let list = gtk::ListBox::new();
        list.append(&row);
        crate::widgets::row_activate::click_row(&row);
        assert_eq!(clicks.get(), 1, "a click activates the row once");
        row.emit_by_name::<()>("activate", &[]);
        assert_eq!(clicks.get(), 2, "so does Enter or Space");
        let mut controllers = Vec::new();
        let all = row.observe_controllers();
        for i in 0..all.n_items() {
            controllers.push(all.item(i).expect("controller"));
        }
        assert_eq!(controllers.len(), 3, "long press, secondary click and the menu key; no primary click of its own");
        for controller in &controllers {
            if let Some(click) = controller.downcast_ref::<gtk::GestureClick>() {
                assert_eq!(click.button(), gtk::gdk::BUTTON_SECONDARY);
                click.emit_by_name::<()>("pressed", &[&1_i32, &0.0_f64, &0.0_f64]);
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
        assert_eq!(clicks.get(), 2, "the menu gestures never activate the row");
        assert_eq!(longs.get(), 4, "the long press, the right click and both menu keys reach the long-press callback");
    }

    fn a_listing_cell_opens_the_menu_from_the_keyboard_too() {
        let cell = GBox::new(O::Horizontal, 0);
        let opened = Rc::new(Cell::new(0));
        let seen = Rc::clone(&opened);
        attach_long_press(&cell, move || seen.set(seen.get() + 1));
        assert!(cell.is_focusable(), "the cell takes focus so the keys reach it");
        let list = cell.observe_controllers();
        let keys = (0..list.n_items()).filter_map(|i| list.item(i)).find_map(|c| c.downcast::<gtk::EventControllerKey>().ok()).expect("key controller");
        let shift = gtk::gdk::ModifierType::SHIFT_MASK;
        assert!(keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Menu, &0_u32, &gtk::gdk::ModifierType::empty()]));
        assert!(keys.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::F10, &0_u32, &shift]));
        assert_eq!(opened.get(), 2);
    }

    /// Run by `smoke::browse_widgets_smoke`: every widget check of this lane runs in that one test, on the shared
    /// GTK thread of `crate::testing`.
    pub(crate) fn smoke() {
        a_placeholder_slot_renders_nothing();
        a_favourite_dims_only_its_cover_and_not_the_cell_or_the_title();
        a_cell_has_half_a_gap_of_margin_on_every_side();
        cover_only_has_no_title_and_comfortable_has_one_under_the_cover();
        the_cover_loader_is_called_with_the_item_cover();
        a_remote_cover_is_cancelled_with_its_picture_and_never_drawn();
        primary_click_and_long_press_call_different_callbacks();
        a_listing_cell_opens_the_menu_from_the_keyboard_too();
    }
}
