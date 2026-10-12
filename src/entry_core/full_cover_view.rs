//! The full-screen cover window of a manga (T-38a-037): the cover over the whole window, a pinch or Ctrl+scroll zoom, a
//! click beside the picture or Escape closes it, and pill bars hold Close, Share and Save, and Edit (a menu with Edit and
//! Delete when a custom cover exists; the file picker is filtered to `image/*`). The picture grows in over 500 ms.
//! The rules are `cover_model`; the files are `anime::images_io`. This file only draws and calls them.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, gdk, gio, glib};
use kuro_i18n::{MsgKey, tr};
use kuro_images::share::{reveal, share_or_reveal};
use kuro_images::viewer::EditButton;

use super::anime::images::{ImageAction, ImageError, ImageKind};
use super::anime::images_io::{self, Targets};
use super::cover_model::{self, EDIT_MENU, PICKER_MIME, ZOOM_MIN};
use super::cover_view;
use super::detail::MangaRow;
use crate::ui;

/// What the window needs from the page: the runtime for the file work, and what to call when the cover was edited or
/// deleted (the page moves `cover_last_modified` forward).
pub struct Env {
    pub handle: tokio::runtime::Handle,
    pub bump: Rc<dyn Fn()>,
}

pub(super) fn offload<T, F>(handle: &tokio::runtime::Handle, work: impl FnOnce() -> F + Send + 'static) -> async_channel::Receiver<T>
where
    T: Send + 'static,
    F: Future<Output = T>,
{
    let (tx, rx) = async_channel::bounded(1);
    let driver = handle.clone();
    handle.spawn_blocking(move || {
        let _ = tx.send_blocking(driver.block_on(work()));
    });
    rx
}

fn pictures_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(|| glib::home_dir().join("Pictures"))
}

struct State {
    window: gtk::Window,
    manga: RefCell<MangaRow>,
    stack: gtk::Stack,
    picture: gtk::Picture,
    scroller: gtk::ScrolledWindow,
    error: Label,
    edit_slot: GBox,
    notice: GBox,
    notice_text: Label,
    notice_id: Cell<u32>,
    zoom: Cell<f64>,
    env: Env,
}

fn pill(content: &impl IsA<gtk::Widget>) -> GBox {
    let bar = GBox::new(O::Horizontal, 2);
    bar.add_css_class("osd");
    bar.set_margin_top(12);
    bar.set_margin_start(12);
    bar.set_margin_end(12);
    bar.set_valign(Align::Start);
    bar.append(content);
    bar
}

/// Opens the full-cover window of `manga`.
pub fn open(manga: MangaRow, env: Env) -> gtk::Window {
    let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Contain).can_shrink(true).hexpand(true).vexpand(true).build();
    let scroller = gtk::ScrolledWindow::builder().child(&picture).hexpand(true).vexpand(true).build();
    let error = Label::builder().wrap(true).css_classes(["title-3", "dim-label"]).build();
    let status = GBox::new(O::Vertical, 12);
    status.set_valign(Align::Center);
    status.set_halign(Align::Center);
    status.append(&gtk::Image::builder().icon_name("image-missing").pixel_size(64).css_classes(["dim-label"]).build());
    status.append(&error);
    let spinner = gtk::Spinner::builder().spinning(true).width_request(32).height_request(32).halign(Align::Center).valign(Align::Center).build();
    let stack = gtk::Stack::new();
    stack.add_named(&spinner, Some("loading"));
    stack.add_named(&scroller, Some("picture"));
    stack.add_named(&status, Some("error"));

    let notice_text = Label::builder().css_classes(["caption"]).build();
    let notice = GBox::new(O::Horizontal, 6);
    notice.add_css_class("osd");
    notice.set_halign(Align::Center);
    notice.set_valign(Align::End);
    notice.set_margin_bottom(16);
    notice.set_visible(false);
    notice.append(&notice_text);

    let close = ui::icon_button("window-close-symbolic", &tr(MsgKey::action_close));
    close.add_css_class("flat");
    let share = ui::icon_button("emblem-shared-symbolic", &tr(MsgKey::action_share));
    let save = ui::icon_button("document-save-symbolic", &tr(MsgKey::action_save));
    for b in [&share, &save] {
        b.add_css_class("flat");
    }
    let edit_slot = GBox::new(O::Horizontal, 0);
    let left = pill(&close);
    left.set_halign(Align::Start);
    let actions = GBox::new(O::Horizontal, 0);
    actions.append(&share);
    actions.append(&save);
    let right = pill(&actions);
    right.set_halign(Align::End);
    let edit_pill = pill(&edit_slot);
    edit_pill.set_halign(Align::End);
    edit_pill.set_margin_top(64);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&stack));
    for w in [&left, &right, &edit_pill, &notice] {
        overlay.add_overlay(w);
    }
    // a header row with the window buttons, so the window can be dragged, minimised and closed like every other screen
    let header = gtk::HeaderBar::new();
    header.set_decoration_layout(Some(&ui::decoration(false)));
    header.set_title_widget(Some(&GBox::new(O::Horizontal, 0)));
    ui::no_sticky_focus(&header);
    let window = gtk::Window::builder().transient_for(&ui::window()).modal(true).default_width(900).default_height(700).titlebar(&header).child(&overlay).build();
    window.maximize();

    let this = Rc::new(State { window, manga: RefCell::new(manga), stack, picture, scroller, error, edit_slot, notice, notice_text, notice_id: Cell::new(0), zoom: Cell::new(ZOOM_MIN), env });
    this.connect(&close, &save, &share);
    this.refresh_edit();
    this.load();
    this.window.present();
    this.grow_in();
    this.window.clone()
}

impl State {
    fn connect(self: &Rc<Self>, close: &Button, save: &Button, share: &Button) {
        let weak = Rc::downgrade(self);
        let on = |f: fn(&Rc<State>)| {
            let weak = weak.clone();
            move |_: &Button| {
                if let Some(this) = weak.upgrade() {
                    f(&this);
                }
            }
        };
        close.connect_clicked(on(|s| s.window.close()));
        save.connect_clicked(on(|s| s.run(ImageAction::Save)));
        share.connect_clicked(on(|s| s.run(ImageAction::Share)));

        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let window = self.window.clone();
            move |_, key, _, _| {
                if key == gdk::Key::Escape {
                    window.close();
                    glib::Propagation::Stop
                } else {
                    glib::Propagation::Proceed
                }
            }
        });
        self.window.add_controller(keys);

        let pinch = gtk::GestureZoom::new();
        let start = Rc::new(Cell::new(ZOOM_MIN));
        pinch.connect_begin({
            let (weak, start) = (weak.clone(), Rc::clone(&start));
            move |_, _| {
                if let Some(this) = weak.upgrade() {
                    start.set(this.zoom.get());
                }
            }
        });
        pinch.connect_scale_changed({
            let weak = weak.clone();
            move |_, scale| {
                if let Some(this) = weak.upgrade() {
                    this.set_zoom(cover_model::zoom_after_pinch(start.get(), scale));
                }
            }
        });
        self.stack.add_controller(pinch);

        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        scroll.connect_scroll({
            let weak = weak.clone();
            move |controller, _, dy| match weak.upgrade() {
                Some(this) if controller.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) => {
                    this.set_zoom(cover_model::zoom_after_scroll(this.zoom.get(), dy));
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.stack.add_controller(scroll);

        let click = gtk::GestureClick::builder().button(1).build();
        let weak = Rc::downgrade(self);
        click.connect_released(move |_, _, x, y| {
            if let Some(this) = weak.upgrade()
                && this.click_closes(x, y)
            {
                this.window.close();
            }
        });
        self.stack.add_controller(click);
    }

    fn click_closes(&self, x: f64, y: f64) -> bool {
        let Some(point) = self.stack.compute_point(&self.picture, &gtk::graphene::Point::new(x as f32, y as f32)) else { return false };
        let widget = (f64::from(self.picture.width()), f64::from(self.picture.height()));
        let image = self.picture.paintable().map(|p| (f64::from(p.intrinsic_width()), f64::from(p.intrinsic_height())));
        let shown = self.stack.visible_child_name().as_deref() == Some("picture");
        cover_model::click_closes(widget, if shown { image } else { None }, (f64::from(point.x()), f64::from(point.y())))
    }

    /// The picture grows to full size over 500 ms by shrinking its margins to nothing.
    fn grow_in(self: &Rc<Self>) {
        let started = Instant::now();
        let weak = Rc::downgrade(self);
        glib::timeout_add_local(Duration::from_millis(16), move || {
            let Some(this) = weak.upgrade() else { return glib::ControlFlow::Break };
            let elapsed = started.elapsed().as_millis() as u64;
            let scale = cover_model::open_scale(elapsed);
            let (w, h) = (f64::from(this.scroller.width()), f64::from(this.scroller.height()));
            this.picture.set_margin_start(((1.0 - scale) * w / 2.0) as i32);
            this.picture.set_margin_end(((1.0 - scale) * w / 2.0) as i32);
            this.picture.set_margin_top(((1.0 - scale) * h / 2.0) as i32);
            this.picture.set_margin_bottom(((1.0 - scale) * h / 2.0) as i32);
            if elapsed >= cover_model::OPEN_TRANSITION_MS { glib::ControlFlow::Break } else { glib::ControlFlow::Continue }
        });
    }

    fn set_zoom(&self, zoom: f64) {
        self.zoom.set(zoom);
        if zoom <= ZOOM_MIN {
            self.picture.set_size_request(-1, -1);
        } else {
            let (w, h) = (f64::from(self.scroller.width()), f64::from(self.scroller.height()));
            self.picture.set_size_request((w * zoom) as i32, (h * zoom) as i32);
        }
    }

    fn targets(&self) -> Option<Targets> {
        let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
        Some(Targets {
            custom: kuro_images::cover_paths::custom_file_for(&paths, &cover_model::art_of(&self.manga.borrow())),
            pictures: pictures_dir().join(kuro_images::save::PICTURES_FOLDER),
            staging: kuro_images::share::staging_dir(&paths),
        })
    }

    fn load(self: &Rc<Self>) {
        let art = cover_model::art_of(&self.manga.borrow());
        let Some(targets) = self.targets() else { return self.show_error(ImageError::Io) };
        let service = cover_view::service();
        let rx = offload(&self.env.handle, move || async move { images_io::current_bytes(service.as_deref(), &art, &targets.custom).await });
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            match result.and_then(|bytes| gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)).map_err(|_| ImageError::OpenFailed)) {
                Ok(texture) => {
                    this.picture.set_paintable(Some(&texture));
                    this.stack.set_visible_child_name("picture");
                }
                Err(error) => this.show_error(error),
            }
        });
    }

    fn show_error(&self, error: ImageError) {
        tracing::error!("cover load failed: {error:?}");
        self.error.set_label(&tr(MsgKey::error_saving_cover));
        self.stack.set_visible_child_name("error");
    }

    fn notify(self: &Rc<Self>, text: &str) {
        let id = self.notice_id.get().wrapping_add(1);
        self.notice_id.set(id);
        self.notice_text.set_label(text);
        self.notice.set_visible(true);
        let weak = Rc::downgrade(self);
        glib::timeout_add_local_once(Duration::from_secs(4), move || {
            if let Some(this) = weak.upgrade()
                && this.notice_id.get() == id
            {
                this.notice.set_visible(false);
            }
        });
    }

    fn finish(self: &Rc<Self>, action: ImageAction, result: Result<(), ImageError>) {
        if let Err(error) = result {
            tracing::error!("cover {action:?} failed: {error:?}");
        }
        match cover_model::notice(action, result.err()) {
            Ok(Some(key)) | Err(key) => self.notify(&tr(key)),
            Ok(None) => {}
        }
        if cover_model::bumps_cover(action, result.is_ok()) {
            let now = chrono::Utc::now().timestamp_millis();
            {
                let mut manga = self.manga.borrow_mut();
                manga.cover_last_modified = cover_model::bumped_last_modified(manga.cover_last_modified, now);
            }
            (self.env.bump)();
            self.stack.set_visible_child_name("loading");
            self.load();
            self.refresh_edit();
        }
    }

    /// Save and Share.
    fn run(self: &Rc<Self>, action: ImageAction) {
        let Some(targets) = self.targets() else { return self.finish(action, Err(ImageError::Io)) };
        let manga = self.manga.borrow().clone();
        let art = cover_model::art_of(&manga);
        let service = cover_view::service();
        let rx = offload(&self.env.handle, move || async move {
            let bytes = images_io::current_bytes(service.as_deref(), &art, &targets.custom).await?;
            match action {
                ImageAction::Share => images_io::stage_for_share(&bytes, ImageKind::Cover, &targets).await,
                _ => images_io::save(&bytes, ImageKind::Cover, &manga.title, &targets).await,
            }
        });
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            match (action, result) {
                (ImageAction::Share, Ok(path)) => {
                    let _ = share_or_reveal(&path, open_file, reveal);
                }
                (_, result) => this.finish(action, result.map(|_| ())),
            }
        });
    }

    /// The Edit pill: the picker at once, or a menu with Edit and Delete when a custom cover exists.
    fn refresh_edit(self: &Rc<Self>) {
        while let Some(child) = self.edit_slot.first_child() {
            self.edit_slot.remove(&child);
        }
        let has_custom = self.targets().is_some_and(|t| t.custom.is_file());
        let label = tr(MsgKey::action_edit);
        let weak = Rc::downgrade(self);
        match cover_model::edit_pill(has_custom) {
            EditButton::Picker => {
                let button = Button::builder().label(&label).css_classes(["flat"]).build();
                button.connect_clicked(move |_| {
                    if let Some(this) = weak.upgrade() {
                        this.pick_image();
                    }
                });
                self.edit_slot.append(&button);
            }
            EditButton::Menu => {
                let popover = gtk::Popover::new();
                let col = GBox::new(O::Vertical, 2);
                for action in EDIT_MENU {
                    let text = tr(if action == ImageAction::Edit { MsgKey::action_edit } else { MsgKey::action_delete });
                    let b = Button::builder().label(text).css_classes(["flat"]).build();
                    let (popover, weak) = (popover.clone(), weak.clone());
                    b.connect_clicked(move |_| {
                        popover.popdown();
                        if let Some(this) = weak.upgrade() {
                            if action == ImageAction::Edit { this.pick_image() } else { this.delete() }
                        }
                    });
                    col.append(&b);
                }
                popover.set_child(Some(&col));
                self.edit_slot.append(&crate::ui::quiet_menu(gtk::MenuButton::builder().label(&label).popover(&popover).css_classes(["flat"]).build()));
            }
        }
    }

    fn pick_image(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder().title(tr(MsgKey::action_edit)).build();
        let filter = gtk::FileFilter::new();
        filter.add_mime_type(PICKER_MIME);
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let this = Rc::clone(self);
        dialog.open(Some(&self.window), None::<&gio::Cancellable>, move |picked| {
            let Some(path) = picked.ok().and_then(|f| f.path()) else { return };
            let Some(targets) = this.targets() else { return this.finish(ImageAction::Edit, Err(ImageError::Io)) };
            let rx = offload(&this.env.handle, move || async move { images_io::set_custom(&path, &targets).await });
            let this = Rc::clone(&this);
            glib::spawn_future_local(async move {
                if let Ok(result) = rx.recv().await {
                    this.finish(ImageAction::Edit, result);
                }
            });
        });
    }

    fn delete(self: &Rc<Self>) {
        let Some(targets) = self.targets() else { return self.finish(ImageAction::Delete, Err(ImageError::Io)) };
        let rx = offload(&self.env.handle, move || async move { images_io::delete_custom(&targets).await });
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            if let Ok(result) = rx.recv().await {
                this.finish(ImageAction::Delete, result);
            }
        });
    }
}

fn open_file(path: &std::path::Path) -> std::io::Result<()> {
    let uri = gio::File::for_path(path).uri();
    gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>).map_err(std::io::Error::other)
}
