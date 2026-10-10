//! The images window of an anime (T-39a-045, T-39a-046): two pages, the cover and the background, with an arrow button
//! that switches between them and says what the other page is, an Edit button (a menu with Edit and Delete when a custom
//! image exists), Save and Share. A pinch or Ctrl+scroll zooms; Escape or a click beside the picture closes the window.
//! Delete asks first. The rules are `images.rs` (pager, edit button, zoom, outcomes) and the file work is `images_io`;
//! this file only draws and calls them. A result is told in a short notice at the bottom of the window.

use std::cell::{Cell, RefCell};
use std::future::Future;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O, gdk, gio, glib};
use kuro_data::anime::Anime;
use kuro_i18n::{MsgKey, tr};
use kuro_images::share::{reveal, share_or_reveal};

use super::details_error::DetailsError;
use super::images::{
    DELETE_NEEDS_CONFIRMATION, EditButton, ImageAction, ImageError, ImageKind, Pager, ZOOM_MIN, bumped_last_modified, edit_button, hits_image,
    outcome, report_failure, zoom_after_pinch, zoom_after_scroll,
};
use super::images_io::{self, Targets, art_of};
use crate::entry_core::cover_view;
use crate::ui;

/// What the window needs from the page: the runtime for the file work, and what to call when an image was edited or
/// deleted (the page moves the last-modified of that image forward).
pub struct Env {
    pub handle: tokio::runtime::Handle,
    pub bump: Rc<dyn Fn(ImageKind)>,
}

const KINDS: [ImageKind; 2] = [ImageKind::Cover, ImageKind::Background];

/// One of the two pages: a picture, a spinner while it loads, and an error line when it cannot.
struct Page {
    stack: gtk::Stack,
    scroller: gtk::ScrolledWindow,
    picture: gtk::Picture,
    error: Label,
}

fn page() -> Page {
    let picture = gtk::Picture::builder().content_fit(gtk::ContentFit::Contain).can_shrink(true).hexpand(true).vexpand(true).build();
    let scroller = gtk::ScrolledWindow::builder().child(&picture).hexpand(true).vexpand(true).build();
    let error = Label::builder().wrap(true).justify(gtk::Justification::Center).css_classes(["title-3", "dim-label"]).build();
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
    Page { stack, scroller, picture, error }
}

struct State {
    window: gtk::Window,
    anime: RefCell<Anime>,
    pager: Cell<Pager>,
    pages: [Page; 2],
    pages_stack: gtk::Stack,
    switch_label: Label,
    edit_slot: GBox,
    notice: Label,
    /// Counts the notices, so the timeout of an older one does not hide a newer one.
    notice_id: Cell<u32>,
    zoom: Cell<f64>,
    env: Env,
}

/// Opens the images window of `anime` on the page of `start`.
pub fn open(anime: Anime, start: ImageKind, env: Env) {
    State::open(anime, start, env);
}

/// Runs `work` to the end on a blocking-pool thread and returns its result on a channel. The image service's futures are
/// not `Send`, so the future is built and driven on that thread.
fn offload<T, F>(handle: &tokio::runtime::Handle, work: impl FnOnce() -> F + Send + 'static) -> async_channel::Receiver<T>
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

/// The pictures folder of the user, `<pictures>/Kurobayomi` is made under it by the save.
fn pictures_dir() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Pictures).unwrap_or_else(|| glib::home_dir().join("Pictures"))
}

impl State {
    fn open(anime: Anime, start: ImageKind, env: Env) {
        let pages = [page(), page()];
        let pages_stack = gtk::Stack::builder().transition_type(gtk::StackTransitionType::SlideLeftRight).hexpand(true).vexpand(true).build();
        for (kind, page) in KINDS.iter().zip(&pages) {
            pages_stack.add_named(&page.stack, Some(kind.stem()));
        }
        let notice = Label::builder().css_classes(["osd", "caption"]).halign(Align::Center).valign(Align::End).margin_bottom(16).visible(false).build();
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&pages_stack));
        overlay.add_overlay(&notice);

        let switch_label = Label::new(None);
        let switch_content = GBox::new(O::Horizontal, 6);
        switch_content.append(&switch_label);
        switch_content.append(&gtk::Image::from_icon_name("go-next-symbolic"));
        let switch = Button::builder().child(&switch_content).css_classes(["flat"]).build();
        let save = ui::icon_button("document-save-symbolic", &tr(MsgKey::action_save));
        let share = ui::icon_button("emblem-shared-symbolic", &tr(MsgKey::action_share));
        let edit_slot = GBox::new(O::Horizontal, 0);

        let window = ui::modal(Label::new(None).upcast_ref(), &overlay, 900, 700);
        if let Some(header) = window.titlebar().and_downcast::<gtk::HeaderBar>() {
            header.pack_start(&switch);
            header.pack_end(&share);
            header.pack_end(&save);
            header.pack_end(&edit_slot);
        }
        let this = Rc::new(Self {
            window,
            anime: RefCell::new(anime),
            pager: Cell::new(Pager::new(start)),
            pages,
            pages_stack,
            switch_label,
            edit_slot,
            notice,
            notice_id: Cell::new(0),
            zoom: Cell::new(ZOOM_MIN),
            env,
        });
        this.connect(&switch, &save, &share);
        this.show_pager();
        for kind in KINDS {
            this.load(kind);
        }
        this.window.present();
    }

    fn connect(self: &Rc<Self>, switch: &Button, save: &Button, share: &Button) {
        let weak = Rc::downgrade(self);
        let on = |f: fn(&Rc<State>)| {
            let weak = weak.clone();
            move |_: &Button| {
                if let Some(this) = weak.upgrade() {
                    f(&this);
                }
            }
        };
        switch.connect_clicked(on(|s| {
            s.pager.set(s.pager.get().switch());
            s.show_pager();
        }));
        save.connect_clicked(on(|s| s.run(ImageAction::Save)));
        share.connect_clicked(on(|s| s.run(ImageAction::Share)));

        // Escape closes the window
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

        // a pinch zooms the page on screen
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
                    this.set_zoom(zoom_after_pinch(start.get(), scale));
                }
            }
        });
        self.pages_stack.add_controller(pinch);

        // Ctrl+scroll zooms; a plain scroll pans the zoomed picture as usual
        let scroll = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
        scroll.set_propagation_phase(gtk::PropagationPhase::Capture);
        scroll.connect_scroll({
            let weak = weak.clone();
            move |controller, _, dy| match weak.upgrade() {
                Some(this) if controller.current_event_state().contains(gdk::ModifierType::CONTROL_MASK) => {
                    this.set_zoom(zoom_after_scroll(this.zoom.get(), dy));
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.pages_stack.add_controller(scroll);

        // a click beside the picture closes the window
        let click = gtk::GestureClick::builder().button(1).build();
        let weak = Rc::downgrade(self);
        click.connect_released(move |_, _, x, y| {
            if let Some(this) = weak.upgrade()
                && !this.click_hits_picture(x, y)
            {
                this.window.close();
            }
        });
        self.pages_stack.add_controller(click);
    }

    fn page(&self) -> &Page {
        &self.pages[usize::from(self.pager.get().page == ImageKind::Background)]
    }

    fn page_of(&self, kind: ImageKind) -> &Page {
        &self.pages[usize::from(kind == ImageKind::Background)]
    }

    /// Whether a click at (x, y) of the pages lands on the picture on screen (D: a click on the background closes).
    fn click_hits_picture(&self, x: f64, y: f64) -> bool {
        let page = self.page();
        if page.stack.visible_child_name().as_deref() != Some("picture") {
            return false;
        }
        let Some(paintable) = page.picture.paintable() else { return false };
        let Some(point) = self.pages_stack.compute_point(&page.picture, &gtk::graphene::Point::new(x as f32, y as f32)) else { return false };
        let (px, py) = (f64::from(point.x()), f64::from(point.y()));
        let widget = (f64::from(page.picture.width()), f64::from(page.picture.height()));
        hits_image(widget, (f64::from(paintable.intrinsic_width()), f64::from(paintable.intrinsic_height())), (px, py))
    }

    /// The page of the pager on screen, with the labels of its two buttons. The zoom starts again at fit.
    fn show_pager(self: &Rc<Self>) {
        let pager = self.pager.get();
        self.pages_stack.set_visible_child_name(pager.page.stem());
        self.switch_label.set_label(&tr(pager.switch_label()));
        self.set_zoom(ZOOM_MIN);
        self.refresh_edit();
    }

    fn set_zoom(&self, zoom: f64) {
        self.zoom.set(zoom);
        let page = self.page();
        if zoom <= ZOOM_MIN {
            page.picture.set_size_request(-1, -1);
        } else {
            let (w, h) = (f64::from(page.scroller.width()), f64::from(page.scroller.height()));
            page.picture.set_size_request((w * zoom) as i32, (h * zoom) as i32);
        }
    }

    /// The files of the image of `kind`: its custom file, the save folder and the share folder.
    fn targets(&self, kind: ImageKind) -> Option<Targets> {
        let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
        Some(Targets {
            custom: kuro_images::cover_paths::custom_file_for(&paths, &art_of(&self.anime.borrow(), kind)),
            pictures: pictures_dir().join(kuro_images::save::PICTURES_FOLDER),
            staging: kuro_images::share::staging_dir(&paths),
        })
    }

    /// Reads the picture of `kind` and puts it on its page; no image or an unreadable one says why on the page.
    fn load(self: &Rc<Self>, kind: ImageKind) {
        let anime = self.anime.borrow().clone();
        let Some(targets) = self.targets(kind) else { return self.show_error(kind, ImageError::Io) };
        let art = art_of(&anime, kind);
        let service = cover_view::service();
        let rx = offload(&self.env.handle, move || async move { images_io::current_bytes(service.as_deref(), &art, &targets.custom).await });
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            match result.and_then(|bytes| gdk::Texture::from_bytes(&glib::Bytes::from_owned(bytes)).map_err(|_| ImageError::OpenFailed)) {
                Ok(texture) => {
                    let page = this.page_of(kind);
                    page.picture.set_paintable(Some(&texture));
                    page.stack.set_visible_child_name("picture");
                }
                Err(error) => this.show_error(kind, error),
            }
        });
    }

    fn show_error(&self, kind: ImageKind, error: ImageError) {
        let error = DetailsError::from(error).record();
        let page = self.page_of(kind);
        page.error.set_label(&error.user_message());
        page.stack.set_visible_child_name("error");
    }

    /// A short message at the bottom of the window, for four seconds.
    fn notify(self: &Rc<Self>, text: &str) {
        let id = self.notice_id.get().wrapping_add(1);
        self.notice_id.set(id);
        self.notice.set_label(text);
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

    /// Tells the result of an action: the success text, or the failure text after the log line (D-14).
    fn finish(self: &Rc<Self>, kind: ImageKind, action: ImageAction, result: Result<(), ImageError>) {
        match outcome(kind, action, result.err()) {
            Ok(Some(key)) => self.notify(&tr(key)),
            Ok(None) => {}
            Err(_) => {
                let error = result.err().unwrap_or(ImageError::Io);
                let key = report_failure(kind, action, error, &mut |line| tracing::error!("{line}"));
                self.notify(&tr(key));
            }
        }
    }

    /// Save and Share on the page on screen.
    fn run(self: &Rc<Self>, action: ImageAction) {
        let kind = self.pager.get().page;
        let anime = self.anime.borrow().clone();
        let Some(targets) = self.targets(kind) else { return self.finish(kind, action, Err(ImageError::Io)) };
        let art = art_of(&anime, kind);
        let service = cover_view::service();
        let title = anime.title.clone();
        let rx = offload(&self.env.handle, move || async move {
            let bytes = images_io::current_bytes(service.as_deref(), &art, &targets.custom).await?;
            match action {
                ImageAction::Share => images_io::stage_for_share(&bytes, kind, &targets).await,
                _ => images_io::save(&bytes, kind, &title, &targets).await,
            }
        });
        let this = Rc::clone(self);
        glib::spawn_future_local(async move {
            let Ok(result) = rx.recv().await else { return };
            match (action, result) {
                (ImageAction::Share, Ok(path)) => {
                    // no system share sheet on Linux: the staged file opens in its default app, and when that fails
                    // its folder opens instead, with no error (T-34a-047)
                    let _ = share_or_reveal(&path, |p| open_file(p), reveal);
                }
                (_, result) => this.finish(kind, action, result.map(|_| ())),
            }
        });
    }

    /// The Edit button: Edit at once, or a menu with Edit and Delete when a custom image exists (T-I11).
    fn refresh_edit(self: &Rc<Self>) {
        while let Some(child) = self.edit_slot.first_child() {
            self.edit_slot.remove(&child);
        }
        let pager = self.pager.get();
        let has_custom = self.targets(pager.page).is_some_and(|t| t.custom.is_file());
        let label = tr(pager.edit_label());
        let weak = Rc::downgrade(self);
        let on_edit = {
            let weak = weak.clone();
            move || {
                if let Some(this) = weak.upgrade() {
                    this.pick_image();
                }
            }
        };
        match edit_button(has_custom) {
            EditButton::Edit => {
                let button = Button::builder().label(&label).css_classes(["flat"]).build();
                button.connect_clicked(move |_| on_edit());
                self.edit_slot.append(&button);
            }
            EditButton::Menu => {
                let popover = gtk::Popover::new();
                let col = GBox::new(O::Vertical, 2);
                let item = |text: String, run: Box<dyn Fn()>| {
                    let b = Button::builder().label(text).css_classes(["flat"]).build();
                    let popover = popover.clone();
                    b.connect_clicked(move |_| {
                        popover.popdown();
                        run();
                    });
                    col.append(&b);
                };
                item(tr(MsgKey::action_edit), Box::new(on_edit));
                item(
                    tr(MsgKey::action_delete),
                    Box::new(move || {
                        if let Some(this) = weak.upgrade() {
                            this.ask_delete();
                        }
                    }),
                );
                popover.set_child(Some(&col));
                self.edit_slot.append(&gtk::MenuButton::builder().label(&label).popover(&popover).css_classes(["flat"]).build());
            }
        }
    }

    /// Edit: picks an image file and copies it in as the custom image.
    fn pick_image(self: &Rc<Self>) {
        let kind = self.pager.get().page;
        let dialog = gtk::FileDialog::builder().title(tr(self.pager.get().edit_label())).build();
        let filter = gtk::FileFilter::new();
        filter.add_mime_type("image/*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        dialog.set_filters(Some(&filters));
        let this = Rc::clone(self);
        dialog.open(Some(&self.window), None::<&gio::Cancellable>, move |picked| {
            // cancelling the chooser is not an error
            let Some(path) = picked.ok().and_then(|f| f.path()) else { return };
            let Some(targets) = this.targets(kind) else { return this.finish(kind, ImageAction::Edit, Err(ImageError::Io)) };
            let rx = offload(&this.env.handle, move || async move { images_io::set_custom(&path, &targets).await });
            let this = Rc::clone(&this);
            glib::spawn_future_local(async move {
                if let Ok(result) = rx.recv().await {
                    this.changed(kind, ImageAction::Edit, result);
                }
            });
        });
    }

    /// Delete asks first (OD-3), then removes the custom image.
    fn ask_delete(self: &Rc<Self>) {
        let kind = self.pager.get().page;
        let this = Rc::clone(self);
        let delete = move || {
            let Some(targets) = this.targets(kind) else { return this.finish(kind, ImageAction::Delete, Err(ImageError::Io)) };
            let rx = offload(&this.env.handle, move || async move { images_io::delete_custom(&targets).await });
            let this = Rc::clone(&this);
            glib::spawn_future_local(async move {
                if let Ok(result) = rx.recv().await {
                    this.changed(kind, ImageAction::Delete, result);
                }
            });
        };
        if !DELETE_NEEDS_CONFIRMATION {
            return delete();
        }
        let ok = Button::builder().label(tr(MsgKey::action_delete)).css_classes(["destructive-action"]).build();
        let cancel = Button::with_label(&tr(MsgKey::action_cancel));
        let buttons = GBox::new(O::Horizontal, 8);
        buttons.set_halign(Align::End);
        buttons.append(&cancel);
        buttons.append(&ok);
        let col = ui::column();
        col.append(&buttons);
        let dialog = ui::modal(Label::new(Some(&tr(MsgKey::are_you_sure))).upcast_ref(), &col, 320, 120);
        dialog.set_transient_for(Some(&self.window));
        cancel.connect_clicked({
            let dialog = dialog.clone();
            move |_| dialog.close()
        });
        let delete = Rc::new(delete);
        ok.connect_clicked({
            let dialog = dialog.clone();
            move |_| {
                dialog.close();
                delete();
            }
        });
        dialog.present();
    }

    /// An edit or a delete ended: on success the last-modified of that image moves forward (here and in the store), the
    /// page loads the picture again and the Edit button follows the new state of the custom image (T-I12).
    fn changed(self: &Rc<Self>, kind: ImageKind, action: ImageAction, result: Result<(), ImageError>) {
        if result.is_ok() {
            let now = chrono::Utc::now().timestamp_millis();
            {
                let mut anime = self.anime.borrow_mut();
                match kind {
                    ImageKind::Cover => anime.cover_last_modified = bumped_last_modified(anime.cover_last_modified, now),
                    ImageKind::Background => anime.background_last_modified = bumped_last_modified(anime.background_last_modified, now),
                }
            }
            (self.env.bump)(kind);
            self.page_of(kind).stack.set_visible_child_name("loading");
            self.load(kind);
            self.refresh_edit();
        }
        self.finish(kind, action, result);
    }
}

/// Hands the file to the desktop's default app for it.
fn open_file(path: &std::path::Path) -> std::io::Result<()> {
    let uri = gio::File::for_path(path).uri();
    gio::AppInfo::launch_default_for_uri(&uri, None::<&gio::AppLaunchContext>).map_err(std::io::Error::other)
}
