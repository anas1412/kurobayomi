//! The three player screens that edit user code: the custom button list (task T-48a-045), the script editor
//! (T-48a-037) and the code editor (T-48a-038). The buttons come from the anime database, the files from the
//! `mpv/scripts` and `mpv/script-opts` folders of the storage location. The rules are in `files.rs`.
//! Going back from the code editor with unsaved changes asks first (Escape, Alt+Left, the mouse back button and the
//! header Back button). The editor has no line numbers; the spec asks for them.
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{glib, Align, Box as GBox, Button, Label, Orientation as O};
use kuro_data::custom_button::CustomButtonRepo;
use kuro_domain::custom_buttons::service::CustomButtonService;
use kuro_domain::custom_buttons::{CustomButton, CustomButtonUpdate};
use kuro_domain::storage::StorageManager;
use kuro_platform::paths::Paths as _;

use super::files::{self, Kind};
use super::syntax::{self, Lang};
use crate::screens::{built, Built};
use crate::ui::{boxed_list, column, icon_button, margins, modal, scrolled};

/// The toast when a folder cannot be opened.
const OPEN_FAILED: &str = "Could not open link";

pub const REQUIRED: &str = "*required";
pub const BUTTON_EXISTS: &str = "Button already exists!";
/// The banner of a script file that is not UTF-8 (edge E-CE-03).
pub const NOT_UTF8: &str = "This file is not valid UTF-8. Saving writes the text back as UTF-8.";
/// Shown when the custom buttons cannot be read from the database; Retry reads them again.
pub const READ_FAILED: &str = "Could not read the custom buttons.";

/// The toast of a custom button write that fails (Kotlin `CustomButtonEvent.InternalError`).
pub const INTERNAL_ERROR: &str = "InternalError: Check crash logs for further information";
/// The supporting text under an optional field.
pub const OPTIONAL: &str = "(optional)";

/// The supporting text under the Title field and whether it is shown as an error: "*required" while blank, the
/// duplicate message in error style when another button has the title.
pub fn title_support(title: &str, taken: &[String]) -> (&'static str, bool) {
    match check_title(title, taken) {
        Ok(()) | Err(REQUIRED) => (REQUIRED, false),
        Err(message) => (message, true),
    }
}

/// Confirm of the button dialog: a title that is not blank nor used by another button, and Lua code that is not blank.
pub fn button_valid(title: &str, content: &str, taken: &[String]) -> bool {
    check_title(title, taken).is_ok() && check_content(content).is_ok()
}

/// The card preview of a button: its first three Lua lines.
pub fn preview(content: &str) -> String {
    content.lines().take(3).collect::<Vec<_>>().join("\n")
}

/// The syntax colours a script file takes from its name: `.conf` files use the conf rules, the rest Lua.
pub fn lang_for(name: &str) -> Lang {
    if name.ends_with(".conf") { Lang::Conf } else { Lang::Lua }
}

/// A save of a file that is not UTF-8 asks once before it writes (edge E-CE-03).
pub fn needs_utf8_confirm(utf8: bool, confirmed: bool) -> bool {
    !utf8 && !confirmed
}

/// Shows the internal-error toast when a button write failed.
fn toast_on_error<T, E: std::fmt::Display>(result: Result<T, E>, what: &str) {
    if let Err(e) = result {
        tracing::warn!(error = %e, "{what}");
        crate::toast::notify(INTERNAL_ERROR);
    }
}

/// Checks a button title: it must not be blank and must not be one of `taken` (the other buttons' titles).
pub fn check_title(title: &str, taken: &[String]) -> Result<(), &'static str> {
    if title.trim().is_empty() {
        Err(REQUIRED)
    } else if taken.iter().any(|t| t == title.trim()) {
        Err(BUTTON_EXISTS)
    } else {
        Ok(())
    }
}

/// A button needs Lua code (`content`): a blank one is refused with the required message.
pub fn check_content(content: &str) -> Result<(), &'static str> {
    if content.trim().is_empty() { Err(REQUIRED) } else { Ok(()) }
}

/// The dialog text of a spec template that quotes a sample name (`"Button"`, `"file"`), with the real name in it.
pub fn quoted_text(template: &str, sample: &str, name: &str) -> String {
    template.replace(&format!("\"{sample}\""), &format!("\"{name}\""))
}

/// What the button form returns on OK.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Fields {
    name: String,
    content: String,
    long_press: String,
    startup: String,
}

fn spec_dialog(screen: &str, id: &str) -> Option<crate::spec::Dialog> {
    crate::ui::screen(screen).dialogs.into_iter().find(|d| d.id == id)
}

fn close_on_escape(win: &gtk::Window) {
    let key = gtk::EventControllerKey::new();
    let w = win.clone();
    key.connect_key_pressed(move |_, k, _, _| {
        if k == gtk::gdk::Key::Escape {
            w.close();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    win.add_controller(key);
}

/// A modal with `body` above a Cancel and an OK button (destructive style when `destructive`). OK runs `on_ok`; the
/// window closes when it returns `true`. Cancel and Escape close it with nothing done. Returns the OK button.
fn form(title: &str, body: &GBox, ok_label: &str, destructive: bool, on_ok: impl Fn() -> bool + 'static) -> Button {
    let col = GBox::new(O::Vertical, 12);
    margins(&col, 18);
    col.append(body);
    let buttons = GBox::new(O::Horizontal, 6);
    buttons.set_halign(Align::End);
    col.append(&buttons);
    let win = modal(&Label::builder().label(title).css_classes(["title"]).build().upcast(), &col, 460, -1);
    let cancel = Button::with_label("Cancel");
    let ok = Button::builder().label(ok_label).css_classes([if destructive { "destructive-action" } else { "suggested-action" }]).build();
    let w = win.clone();
    cancel.connect_clicked(move |_| w.close());
    let w = win.clone();
    ok.connect_clicked(move |_| {
        if on_ok() {
            w.close();
        }
    });
    buttons.append(&cancel);
    buttons.append(&ok);
    close_on_escape(&win);
    win.present();
    ok
}

/// A question with a text and Cancel / OK. OK runs `on_ok`.
fn confirm(title: &str, text: &str, ok_label: &str, destructive: bool, on_ok: impl Fn() + 'static) {
    let body = GBox::new(O::Vertical, 6);
    body.append(&Label::builder().label(text).wrap(true).xalign(0.0).max_width_chars(50).build());
    form(title, &body, ok_label, destructive, move || {
        on_ok();
        true
    });
}

/// A one-line text prompt. `check` turns the typed text into the value to hand to `on_ok`, or into the error to show;
/// OK is off while it errs. The first `select` characters of `initial` start selected.
fn ask_text(
    title: &str,
    label: &str,
    initial: &str,
    select: i32,
    ok_label: &str,
    check: impl Fn(&str) -> Result<String, &'static str> + 'static,
    on_ok: impl Fn(String) + 'static,
) {
    let body = GBox::new(O::Vertical, 6);
    body.append(&Label::builder().label(label).xalign(0.0).css_classes(["caption-heading"]).build());
    let entry = gtk::Entry::builder().text(initial).activates_default(true).build();
    let error = Label::builder().xalign(0.0).css_classes(["error"]).build();
    body.append(&entry);
    body.append(&error);
    let check = Rc::new(check);
    let (checked, shown, typed) = (Rc::clone(&check), error.clone(), entry.clone());
    let ok = form(title, &body, ok_label, false, move || match checked(&typed.text()) {
        Ok(value) => {
            on_ok(value);
            true
        }
        Err(message) => {
            shown.set_label(message);
            false
        }
    });
    let typed = entry.clone();
    entry.connect_changed(move |_| {
        let result = check(&typed.text());
        error.set_label(result.as_ref().err().copied().unwrap_or(""));
        ok.set_sensitive(result.is_ok());
    });
    entry.grab_focus();
    entry.select_region(0, select);
}

fn text_area(initial: &str) -> (gtk::TextView, gtk::ScrolledWindow) {
    let view = gtk::TextView::builder().monospace(true).left_margin(6).right_margin(6).top_margin(6).bottom_margin(6).build();
    view.buffer().set_text(initial);
    syntax::attach(&view.buffer(), Lang::Lua);
    let frame = gtk::ScrolledWindow::builder().child(&view).min_content_height(84).has_frame(true).build();
    (view, frame)
}

fn text_of(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
}

/// The Add and Edit button dialogs: a title and three Lua fields, each with its label and supporting text. Confirm is
/// on only while the title and the Lua code are valid. `taken` holds the titles of the other buttons.
fn button_form(title: &str, ok_label: &str, initial: Option<&CustomButton>, taken: Vec<String>, on_ok: impl Fn(Fields) + 'static) {
    let body = GBox::new(O::Vertical, 6);
    let heading = |text: &str| Label::builder().label(text).xalign(0.0).css_classes(["caption-heading"]).build();
    let support = |text: &str| Label::builder().label(text).xalign(0.0).css_classes(["dim-label", "caption"]).build();
    let name = gtk::Entry::builder().text(initial.map_or("", |b| b.name.as_str())).build();
    let name_support = support(REQUIRED);
    body.append(&heading("Title"));
    body.append(&name);
    body.append(&name_support);
    let (content, content_frame) = text_area(initial.map_or("", |b| b.content.as_str()));
    let (long_press, long_frame) = text_area(initial.map_or("", |b| b.long_press_content.as_str()));
    let (startup, startup_frame) = text_area(initial.map_or("", |b| b.on_startup.as_str()));
    for (label, note, frame) in [
        ("Lua code", REQUIRED, &content_frame),
        ("Lua code (on long press)", OPTIONAL, &long_frame),
        ("On startup", OPTIONAL, &startup_frame),
    ] {
        body.append(&heading(label));
        body.append(frame);
        body.append(&support(note));
    }
    let (n, c, t) = (name.clone(), content.clone(), taken.clone());
    let ok = form(title, &body, ok_label, false, move || {
        if !button_valid(&n.text(), &text_of(&c), &t) {
            return false;
        }
        on_ok(Fields {
            name: n.text().trim().to_owned(),
            content: text_of(&c),
            long_press: text_of(&long_press),
            startup: text_of(&startup),
        });
        true
    });
    let refresh: Rc<dyn Fn()> = {
        let (name, content, name_support, ok) = (name.clone(), content.clone(), name_support, ok);
        Rc::new(move || {
            let (note, is_error) = title_support(&name.text(), &taken);
            name_support.set_label(note);
            if is_error { name_support.add_css_class("error") } else { name_support.remove_css_class("error") }
            ok.set_sensitive(button_valid(&name.text(), &text_of(&content), &taken));
        })
    };
    let again = Rc::clone(&refresh);
    name.connect_changed(move |_| again());
    let again = Rc::clone(&refresh);
    content.buffer().connect_changed(move |_| again());
    refresh();
}

fn confirm_delete(screen: &str, dialog: &str, sample: &str, name: &str, on_ok: impl Fn() + 'static) {
    let Some(d) = spec_dialog(screen, dialog) else { return };
    let text = d.text.map(|t| quoted_text(&t, sample, name)).unwrap_or_default();
    confirm(&d.title, &text, d.buttons.last().map_or("OK", String::as_str), true, on_ok);
}

// ---------- custom buttons ----------

struct ButtonsView {
    service: CustomButtonService,
    list: gtk::ListBox,
    /// The empty state with its Add button, shown instead of the list when there are no buttons.
    empty: GBox,
    /// Shown instead of the list when the database cannot be read; its Retry button renders again.
    failed: GBox,
}

impl ButtonsView {
    fn render(self: &Rc<Self>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let (buttons, failed) = match self.service.list() {
            Ok(buttons) => (buttons, false),
            Err(e) => {
                tracing::warn!(error = %e, "could not read the custom buttons");
                (Vec::new(), true)
            }
        };
        self.failed.set_visible(failed);
        self.empty.set_visible(buttons.is_empty() && !failed);
        self.list.set_visible(!buttons.is_empty());
        let last = buttons.len().saturating_sub(1);
        for (at, button) in buttons.iter().enumerate() {
            self.list.append(&self.row(button, at, last, &buttons));
        }
    }

    /// Moves button `id` to position `to` with one write, then draws the list again once the current event is over.
    fn move_to(self: &Rc<Self>, id: i64, to: usize) {
        toast_on_error(self.service.reorder(id, to), "the custom buttons were not reordered");
        let view = Rc::clone(self);
        glib::idle_add_local_once(move || view.render());
    }

    /// One card: the drag handle, name, star, edit and delete on the first line, the first three Lua lines under them.
    fn row(self: &Rc<Self>, button: &CustomButton, at: usize, last: usize, all: &[CustomButton]) -> gtk::ListBoxRow {
        let card = GBox::new(O::Vertical, 4);
        margins(&card, 8);
        let h = GBox::new(O::Horizontal, 6);
        let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
        handle.set_tooltip_text(Some("Drag to reorder"));
        h.append(&handle);
        h.append(&Label::builder().label(&button.name).xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build());
        let id = button.id;
        let star = gtk::ToggleButton::builder()
            .icon_name(if button.is_favorite { "starred-symbolic" } else { "non-starred-symbolic" })
            .tooltip_text("Primary button")
            .active(button.is_favorite)
            .build();
        let view = Rc::clone(self);
        star.connect_clicked(move |_| {
            toast_on_error(view.service.toggle_favorite(id), "the primary button was not changed");
            view.render();
        });
        let (view, current) = (Rc::clone(self), button.clone());
        let taken: Vec<String> = all.iter().filter(|b| b.id != id).map(|b| b.name.clone()).collect();
        let open_edit: Rc<dyn Fn()> = Rc::new(move || {
            let (view, id) = (Rc::clone(&view), current.id);
            button_form(&format!("Edit button (ID: {id})"), "Edit", Some(&current), taken.clone(), move |f| {
                let update = CustomButtonUpdate {
                    name: Some(f.name),
                    content: Some(f.content),
                    long_press_content: Some(f.long_press),
                    on_startup: Some(f.startup),
                    ..CustomButtonUpdate::new(id)
                };
                toast_on_error(view.service.update(&update), "the custom button was not saved");
                view.render();
            });
        });
        let edit = icon_button("document-edit-symbolic", "Edit");
        let open = Rc::clone(&open_edit);
        edit.connect_clicked(move |_| open());
        let delete = icon_button("user-trash-symbolic", "Delete");
        let (view, name) = (Rc::clone(self), button.name.clone());
        delete.connect_clicked(move |_| {
            let view = Rc::clone(&view);
            confirm_delete("settings_player_custom_buttons", "delete_button", "Button", &name, move || {
                toast_on_error(view.service.delete(id), "the custom button was not deleted");
                view.render();
            });
        });
        for w in [star.upcast::<gtk::Widget>(), edit.upcast(), delete.upcast()] {
            h.append(&w);
        }
        card.append(&h);
        let lines = preview(&button.content);
        if !lines.is_empty() {
            card.append(&Label::builder().label(lines).xalign(0.0).wrap(true).lines(3).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["monospace", "dim-label"]).build());
        }
        let row = gtk::ListBoxRow::builder().child(&card).build();
        crate::widgets::row_activate::on_activate(&row, move |_| open_edit());
        self.drag(&row, &handle, id, at);
        let (view, keys) = (Rc::clone(self), gtk::EventControllerKey::new());
        keys.connect_key_pressed(move |_, key, _, state| {
            let to = match key {
                gtk::gdk::Key::Up if at > 0 => at - 1,
                gtk::gdk::Key::Down if at < last => at + 1,
                _ => return glib::Propagation::Proceed,
            };
            if !state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
                return glib::Propagation::Proceed;
            }
            view.move_to(id, to);
            glib::Propagation::Stop
        });
        row.add_controller(keys);
        row
    }

    /// Drag from the handle, drop on a card: the dragged button takes the place of that card, one write on the drop.
    fn drag(self: &Rc<Self>, row: &gtk::ListBoxRow, handle: &gtk::Image, id: i64, at: usize) {
        let source = gtk::DragSource::builder().actions(gtk::gdk::DragAction::MOVE).content(&gtk::gdk::ContentProvider::for_value(&id.to_value())).build();
        let shown = row.clone();
        source.connect_drag_begin(move |source, _| source.set_icon(Some(&gtk::WidgetPaintable::new(Some(&shown))), 0, 0));
        handle.add_controller(source);
        let target = gtk::DropTarget::new(i64::static_type(), gtk::gdk::DragAction::MOVE);
        let view = Rc::clone(self);
        target.connect_drop(move |_, value, _, _| match value.get::<i64>() {
            Ok(from) if from != id => {
                view.move_to(from, at);
                true
            }
            _ => false,
        });
        row.add_controller(target);
    }
}

/// Empty-state wording for a desktop (the spec's Android text says "Tap the plus button").
pub const NO_BUTTONS: &str = "You have no custom buttons";
pub const ADD_BUTTON: &str = "Add a custom button";
pub const NO_FILES: &str = "This folder has no files";
pub const ADD_FILE: &str = "Add a file";

/// The custom button list with its Add button. With no buttons the list hides and the empty state offers Add; a failed
/// read shows the error state with Retry.
pub fn buttons_page() -> Option<Built> {
    let service = CustomButtonService::new(CustomButtonRepo::new(crate::data::app().anime_db.clone()));
    let add_first = Button::with_label(ADD_BUTTON);
    let (empty, _) = crate::browse::look::empty_state("view-app-grid-symbolic", NO_BUTTONS, Some(&add_first));
    let retry = Button::with_label("Retry");
    let (failed, _) = crate::browse::look::empty_state("dialog-error-symbolic", READ_FAILED, Some(&retry));
    failed.set_visible(false);
    let view = Rc::new(ButtonsView { service, list: boxed_list(), empty: empty.clone(), failed: failed.clone() });
    let col = column();
    col.append(&view.list);
    col.append(&empty);
    col.append(&failed);
    let again = Rc::clone(&view);
    retry.connect_clicked(move |_| again.render());
    view.render();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scrolled(&col)));
    let add: Rc<dyn Fn()> = Rc::new(move || {
        let taken: Vec<String> = view.service.list().unwrap_or_default().into_iter().map(|b| b.name).collect();
        let view = Rc::clone(&view);
        button_form("Add button", "Add", None, taken, move |f| {
            toast_on_error(view.service.create(&f.name, &f.content, &f.long_press, &f.startup), "the custom button was not created");
            view.render();
        });
    });
    let again = Rc::clone(&add);
    add_first.connect_clicked(move |_| again());
    crate::screens::fab(&overlay, "Add", "list-add-symbolic").connect_clicked(move |_| add());
    built(overlay, guide_toolbar("settings_player_custom_buttons"))
}

/// The custom buttons guide, opened in the browser (T-48a-049). A launch that fails shows "Could not open link".
const GUIDE_URL: &str = "https://aniyomi.org/docs/guides/player-settings/custom-buttons";

fn guide_toolbar(id: &str) -> Vec<gtk::Widget> {
    let widgets = crate::ui::toolbar(&crate::ui::screen(id));
    let guide = widgets.iter().find(|w| w.tooltip_text().as_deref() == Some("Custom buttons guide"));
    if let Some(guide) = guide.and_then(|w| w.downcast_ref::<Button>()) {
        guide.connect_clicked(|_| {
            gtk::UriLauncher::new(GUIDE_URL).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
                if let Err(err) = result {
                    tracing::warn!(%err, "could not open the custom buttons guide");
                    crate::toast::notify(OPEN_FAILED);
                }
            });
        });
    }
    widgets
}

// ---------- script editor ----------

thread_local! {
    /// The file the code editor shows: its folder and its name.
    static OPEN_FILE: RefCell<Option<(PathBuf, String)>> = const { RefCell::new(None) };
}

fn folder(kind: Kind) -> Option<PathBuf> {
    let paths = kuro_platform::linux::LinuxPaths::system().ok()?;
    let storage = StorageManager::open(crate::data::app().base.store(), paths.data_dir());
    match kind {
        Kind::Scripts => storage.scripts_dir(),
        Kind::Options => storage.script_opts_dir(),
    }
    .map_err(|e| tracing::warn!(error = %e, "the script folder is not available"))
    .ok()
}

struct EditorView {
    kind: Cell<Kind>,
    list: gtk::ListBox,
    /// The empty state with its Add button, shown instead of the list when the folder has no files.
    empty: GBox,
}

impl EditorView {
    fn render(self: &Rc<Self>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let kind = self.kind.get();
        let dir = folder(kind);
        let names = dir.as_deref().map(|d| files::list(d, kind)).unwrap_or_default();
        self.empty.set_visible(names.is_empty());
        self.list.set_visible(!names.is_empty());
        for name in names {
            self.list.append(&self.row(&name));
        }
    }

    fn row(self: &Rc<Self>, name: &str) -> gtk::ListBoxRow {
        let kind = self.kind.get();
        let (view, file) = (Rc::clone(self), name.to_owned());
        let menu = crate::ui::menu_button("view-more-symbolic", &["Rename file".into(), "Delete file".into()], move |choice| {
            let (view, file) = (Rc::clone(&view), file.clone());
            if choice.starts_with("Rename") { view.rename(&file) } else { view.delete(&file) }
        });
        let detail = folder(kind).map(|dir| files::detail(&dir.join(name))).unwrap_or_default();
        let r = crate::ui::row(name, Some(&detail), Some(menu.upcast()));
        if let Some(inner) = r.child().and_downcast::<GBox>() {
            inner.prepend(&gtk::Image::from_icon_name(kind.icon()));
        }
        r.set_activatable(true);
        let file = name.to_owned();
        crate::widgets::row_activate::on_activate(&r, move |_| {
            let Some(dir) = folder(kind) else { return };
            OPEN_FILE.with(|f| *f.borrow_mut() = Some((dir, file.clone())));
            crate::ui::open("settings_player_code_editor");
        });
        r
    }

    fn existing(&self) -> Vec<String> {
        folder(self.kind.get()).map(|d| files::list(&d, self.kind.get())).unwrap_or_default()
    }

    fn rename(self: Rc<Self>, from: &str) {
        let (kind, from) = (self.kind.get(), from.to_owned());
        let title = spec_dialog("settings_player_editor", "rename_file").map_or("Edit filename".to_owned(), |d| d.title);
        let existing = self.existing();
        let view = Rc::clone(&self);
        let initial = from.clone();
        let select = files::base_len(&initial);
        ask_text(&title, "Filename", &initial, select, "Edit", move |typed| files::checked_name(typed, kind, &existing), move |to| {
            match folder(kind) {
                Some(dir) => {
                    if let Err(e) = files::rename(&dir, &from, &to) {
                        tracing::warn!(error = %e, "the script was not renamed");
                        crate::toast::notify(files::RENAME_FAILED);
                    }
                }
                None => crate::toast::notify(files::RENAME_FAILED),
            }
            view.render();
        });
    }

    fn delete(self: Rc<Self>, name: &str) {
        let (kind, file) = (self.kind.get(), name.to_owned());
        confirm_delete("settings_player_editor", "delete_file", "file", name, move || {
            match folder(kind) {
                Some(dir) => {
                    if let Err(e) = files::delete(&dir, &file) {
                        tracing::warn!(error = %e, "the script was not deleted");
                        crate::toast::notify(files::DELETE_FAILED);
                    }
                }
                None => crate::toast::notify(files::DELETE_FAILED),
            }
            self.render();
        });
    }
}

/// The file list, a Scripts | Script options switch and the Add button. With no files the list hides and the empty
/// state offers Add.
pub fn editor_page() -> Option<Built> {
    let add_first = Button::with_label(ADD_FILE);
    let (empty, _) = crate::browse::look::empty_state("folder-symbolic", NO_FILES, Some(&add_first));
    let view = Rc::new(EditorView { kind: Cell::new(Kind::Scripts), list: boxed_list(), empty: empty.clone() });
    let kinds = gtk::DropDown::from_strings(&[Kind::Scripts.label(), Kind::Options.label()]);
    kinds.set_halign(Align::Start);
    let v = Rc::clone(&view);
    kinds.connect_selected_notify(move |d| {
        v.kind.set(if d.selected() == 0 { Kind::Scripts } else { Kind::Options });
        v.render();
    });
    let col = column();
    col.append(&kinds);
    col.append(&view.list);
    col.append(&empty);
    view.render();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scrolled(&col)));
    let add: Rc<dyn Fn()> = Rc::new(move || {
        let kind = view.kind.get();
        let existing = view.existing();
        let view = Rc::clone(&view);
        let title = spec_dialog("settings_player_editor", "create_file").map_or("Create file".to_owned(), |d| d.title);
        let initial = files::default_name(kind);
        let select = files::base_len(&initial);
        ask_text(&title, "Filename", &initial, select, "Add", move |typed| files::checked_name(typed, kind, &existing), move |name| {
            match folder(kind) {
                Some(dir) => {
                    if let Err(e) = files::create(&dir, &name) {
                        tracing::warn!(error = %e, "the script was not created");
                        crate::toast::notify(files::CREATE_FAILED);
                    }
                }
                None => crate::toast::notify(files::CREATE_FAILED),
            }
            view.render();
        });
    });
    let again = Rc::clone(&add);
    add_first.connect_clicked(move |_| again());
    crate::screens::fab(&overlay, "Add", "list-add-symbolic").connect_clicked(move |_| add());
    built(overlay, vec![])
}

// ---------- code editor ----------

/// The Back button in the header of the page that holds `content`.
fn back_button(content: &impl IsA<gtk::Widget>) -> Option<Button> {
    fn find(w: &gtk::Widget) -> Option<Button> {
        if let Some(b) = w.downcast_ref::<Button>().filter(|b| b.tooltip_text().is_some_and(|t| t == "Back")) {
            return Some(b.clone());
        }
        let mut child = w.first_child();
        while let Some(c) = child {
            if let Some(found) = find(&c) {
                return Some(found);
            }
            child = c.next_sibling();
        }
        None
    }
    find(&content.parent()?.first_child()?)
}

/// Makes every way back from the page ask `ask` instead while `dirty` is set: Escape, Alt+Left, the mouse back button
/// and the header Back button (click, Enter and Space).
fn guard_back(body: &GBox, dirty: &Rc<Cell<bool>>, ask: Rc<dyn Fn()>) {
    let keys = gtk::EventControllerKey::new();
    keys.set_propagation_phase(gtk::PropagationPhase::Capture);
    let (d, a) = (Rc::clone(dirty), Rc::clone(&ask));
    keys.connect_key_pressed(move |_, key, _, state| {
        let back = key == gtk::gdk::Key::Escape || (key == gtk::gdk::Key::Left && state.contains(gtk::gdk::ModifierType::ALT_MASK));
        if back && d.get() {
            a();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    body.add_controller(keys);
    let mouse = gtk::GestureClick::builder().button(8).propagation_phase(gtk::PropagationPhase::Capture).build();
    let (d, a) = (Rc::clone(dirty), Rc::clone(&ask));
    mouse.connect_pressed(move |gesture, _, _, _| {
        if d.get() {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            a();
        }
    });
    body.add_controller(mouse);
    let (d, armed) = (Rc::clone(dirty), Cell::new(false));
    body.connect_map(move |body| {
        let Some(back) = back_button(body).filter(|_| !armed.replace(true)) else { return };
        let click = gtk::GestureClick::builder().propagation_phase(gtk::PropagationPhase::Capture).build();
        let (d1, a1) = (Rc::clone(&d), Rc::clone(&ask));
        click.connect_pressed(move |gesture, _, _, _| {
            if d1.get() {
                gesture.set_state(gtk::EventSequenceState::Claimed);
                a1();
            }
        });
        back.add_controller(click);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        let (d2, a2) = (Rc::clone(&d), Rc::clone(&ask));
        keys.connect_key_pressed(move |_, key, _, _| {
            if d2.get() && (matches!(key, gtk::gdk::Key::Return | gtk::gdk::Key::KP_Enter | gtk::gdk::Key::ISO_Enter) || key == gtk::gdk::Key::space) {
                a2();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        back.add_controller(keys);
    });
}

/// The code editor over the file the list opened. Save is enabled after an edit and writes the file back. Leaving
/// with unsaved changes asks first; the first save of a file that is not UTF-8 asks too.
pub fn code_editor_page() -> Option<Built> {
    let Some((dir, name)) = OPEN_FILE.with(|f| f.borrow().clone()) else {
        return built(crate::ui::status("dialog-information-symbolic", "No file open"), vec![]);
    };
    let path = dir.join(&name);
    let doc = match files::read_document(&path) {
        Ok(doc) => doc,
        Err(err) => return built(crate::ui::status("dialog-error-symbolic", &err.to_string()), vec![]),
    };
    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(&doc.text);
    syntax::attach(&buffer, lang_for(&name));
    let view = gtk::TextView::builder().buffer(&buffer).monospace(true).left_margin(12).top_margin(12).build();
    let save = icon_button("document-save-symbolic", "Save");
    save.set_sensitive(false);
    let dirty = Rc::new(Cell::new(false));
    let (on_edit, edited) = (save.clone(), Rc::clone(&dirty));
    buffer.connect_changed(move |_| {
        edited.set(true);
        on_edit.set_sensitive(true);
    });
    let confirmed = Rc::new(Cell::new(false));
    let write: Rc<dyn Fn()> = {
        let (buffer, save, dirty, crlf) = (buffer.clone(), save.clone(), Rc::clone(&dirty), doc.crlf);
        Rc::new(move || {
            let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
            match files::save_atomic(&path, &files::encode(text.as_str(), crlf)) {
                Ok(()) => {
                    dirty.set(false);
                    save.set_sensitive(false);
                    crate::toast::notify("File saved");
                }
                Err(err) => crate::toast::notify(format!("Could not save the file: {err}")),
            }
        })
    };
    let save_now: Rc<dyn Fn()> = {
        let (write, confirmed, utf8) = (Rc::clone(&write), Rc::clone(&confirmed), doc.utf8);
        Rc::new(move || {
            if !needs_utf8_confirm(utf8, confirmed.get()) {
                write();
                return;
            }
            let (write, confirmed) = (Rc::clone(&write), Rc::clone(&confirmed));
            confirm("Save as UTF-8", NOT_UTF8, "Save", false, move || {
                confirmed.set(true);
                write();
            });
        })
    };
    let on_save = Rc::clone(&save_now);
    save.connect_clicked(move |_| on_save());
    let keys = gtk::EventControllerKey::new();
    let (on_ctrl_s, save_key) = (Rc::clone(&save_now), save.clone());
    keys.connect_key_pressed(move |_, key, _, state| {
        if key == gtk::gdk::Key::s && state.contains(gtk::gdk::ModifierType::CONTROL_MASK) {
            if save_key.is_sensitive() {
                on_ctrl_s();
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    view.add_controller(keys);
    let body = GBox::new(O::Vertical, 0);
    body.append(&Label::builder().label(&name).xalign(0.0).css_classes(["heading"]).margin_start(12).margin_top(6).build());
    if !doc.utf8 {
        body.append(&Label::builder().label(NOT_UTF8).wrap(true).xalign(0.0).css_classes(["warning"]).margin_start(12).margin_end(12).build());
    }
    body.append(&gtk::ScrolledWindow::builder().child(&view).vexpand(true).build());
    let leave = Rc::clone(&dirty);
    let ask: Rc<dyn Fn()> = Rc::new(move || {
        let Some(d) = spec_dialog("settings_player_code_editor", "unsaved") else { return };
        let leave = Rc::clone(&leave);
        confirm(&d.title, d.text.as_deref().unwrap_or_default(), d.buttons.last().map_or("OK", String::as_str), false, move || {
            leave.set(false);
            crate::pref_model::shell::go_back();
        });
    });
    guard_back(&body, &dirty, ask);
    view.connect_map(|view| {
        view.grab_focus();
    });
    built(body, vec![save.upcast()])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_button_title_is_required_and_unique() {
        let taken = vec!["Skip opening".to_owned()];
        assert_eq!(check_title("  ", &taken), Err(REQUIRED));
        assert_eq!(check_title("Skip opening", &taken), Err(BUTTON_EXISTS));
        assert_eq!(check_title(" Skip opening ", &taken), Err(BUTTON_EXISTS));
        assert_eq!(check_title("Toggle shader", &taken), Ok(()));
    }

    #[test]
    fn a_blank_lua_code_is_refused_and_a_filled_one_passes() {
        assert_eq!(check_content("   \n"), Err(REQUIRED));
        assert_eq!(check_content("print(1)"), Ok(()));
    }

    #[test]
    fn the_title_support_is_required_until_a_duplicate_turns_it_into_an_error() {
        let taken = vec!["Skip".to_owned()];
        assert_eq!(title_support("", &taken), (REQUIRED, false));
        assert_eq!(title_support("Other", &taken), (REQUIRED, false));
        assert_eq!(title_support("Skip", &taken), (BUTTON_EXISTS, true));
    }

    #[test]
    fn confirm_needs_a_unique_title_and_lua_code() {
        let taken = vec!["Skip".to_owned()];
        assert!(button_valid("New", "print(1)", &taken));
        assert!(!button_valid("", "print(1)", &taken));
        assert!(!button_valid("Skip", "print(1)", &taken));
        assert!(!button_valid("New", "  ", &taken));
    }

    #[test]
    fn the_card_preview_is_the_first_three_lines() {
        assert_eq!(preview("a\nb\nc\nd"), "a\nb\nc");
        assert_eq!(preview("one"), "one");
        assert_eq!(preview(""), "");
    }

    #[test]
    fn a_conf_file_gets_the_conf_colours_and_anything_else_lua() {
        assert_eq!(lang_for("a.conf"), Lang::Conf);
        assert_eq!(lang_for("a.lua"), Lang::Lua);
    }

    #[test]
    fn a_file_that_is_not_utf8_asks_once_before_its_first_save() {
        assert!(needs_utf8_confirm(false, false));
        assert!(!needs_utf8_confirm(false, true));
        assert!(!needs_utf8_confirm(true, false));
    }

    #[test]
    fn the_internal_error_toast_is_the_kotlin_text() {
        assert_eq!(INTERNAL_ERROR, "InternalError: Check crash logs for further information");
    }

    #[test]
    fn the_delete_text_names_the_real_button_or_file() {
        assert_eq!(
            quoted_text("Do you wish to delete the button \"Button\"?", "Button", "Skip"),
            "Do you wish to delete the button \"Skip\"?"
        );
        assert_eq!(quoted_text("Delete \"file\"?", "file", "a.lua"), "Delete \"a.lua\"?");
    }

    #[test]
    fn the_spec_dialogs_these_screens_open_exist() {
        crate::ui::with_spec(|spec| {
            for (screen, dialog) in [
                ("settings_player_custom_buttons", "delete_button"),
                ("settings_player_editor", "create_file"),
                ("settings_player_editor", "rename_file"),
                ("settings_player_editor", "delete_file"),
                ("settings_player_code_editor", "unsaved"),
            ] {
                let found = spec.screen(screen).is_some_and(|s| s.dialogs.iter().any(|d| d.id == dialog));
                assert!(found, "{screen}/{dialog}");
            }
        });
    }
}
