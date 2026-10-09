//! The three player screens that edit user code: the custom button list (task T-48a-045), the script editor
//! (T-48a-037) and the code editor (T-48a-038). The buttons come from the anime database, the files from the
//! `mpv/scripts` and `mpv/script-opts` folders of the storage location. The rules are in `files.rs`.
//! A script edit opened from the list shows no unsaved-changes prompt on back, and the editor has no line numbers or
//! syntax colours; the spec asks for both.
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
use crate::screens::{built, Built};
use crate::ui::{boxed_list, column, icon_button, margins, modal, scrolled};

pub const REQUIRED: &str = "*required";
pub const BUTTON_EXISTS: &str = "Button already exists!";

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

/// A modal with `body` above a Cancel and an OK button. OK runs `on_ok`; the window closes when it returns `true`.
/// Cancel and Escape close it with nothing done.
fn form(title: &str, body: &GBox, ok_label: &str, on_ok: impl Fn() -> bool + 'static) {
    let col = GBox::new(O::Vertical, 12);
    margins(&col, 18);
    col.append(body);
    let buttons = GBox::new(O::Horizontal, 6);
    buttons.set_halign(Align::End);
    col.append(&buttons);
    let win = modal(&Label::builder().label(title).css_classes(["title"]).build().upcast(), &col, 460, -1);
    let cancel = Button::with_label("Cancel");
    let ok = Button::builder().label(ok_label).css_classes(["suggested-action"]).build();
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
}

/// A one-line text prompt. `check` turns the typed text into the value to hand to `on_ok`, or into the error to show.
fn ask_text(
    title: &str,
    label: &str,
    initial: &str,
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
    form(title, &body, ok_label, move || match check(&entry.text()) {
        Ok(value) => {
            on_ok(value);
            true
        }
        Err(message) => {
            error.set_label(message);
            false
        }
    });
}

fn text_area(initial: &str) -> (gtk::TextView, gtk::ScrolledWindow) {
    let view = gtk::TextView::builder().monospace(true).left_margin(6).right_margin(6).top_margin(6).bottom_margin(6).build();
    view.buffer().set_text(initial);
    let frame = gtk::ScrolledWindow::builder().child(&view).min_content_height(84).has_frame(true).build();
    (view, frame)
}

fn text_of(view: &gtk::TextView) -> String {
    let buffer = view.buffer();
    buffer.text(&buffer.start_iter(), &buffer.end_iter(), false).to_string()
}

/// The Add and Edit button dialogs: a title and three Lua fields. `taken` holds the titles of the other buttons.
fn button_form(title: &str, ok_label: &str, initial: Option<&CustomButton>, taken: Vec<String>, on_ok: impl Fn(Fields) + 'static) {
    let body = GBox::new(O::Vertical, 6);
    let heading = |text: &str| Label::builder().label(text).xalign(0.0).css_classes(["caption-heading"]).build();
    let name = gtk::Entry::builder().text(initial.map_or("", |b| b.name.as_str())).build();
    let error = Label::builder().xalign(0.0).css_classes(["error"]).build();
    body.append(&heading("Title"));
    body.append(&name);
    body.append(&error);
    let (content, content_frame) = text_area(initial.map_or("", |b| b.content.as_str()));
    let (long_press, long_frame) = text_area(initial.map_or("", |b| b.long_press_content.as_str()));
    let (startup, startup_frame) = text_area(initial.map_or("", |b| b.on_startup.as_str()));
    for (label, frame) in [
        ("Lua code", &content_frame),
        ("Lua code (on long press) (optional)", &long_frame),
        ("On startup (optional)", &startup_frame),
    ] {
        body.append(&heading(label));
        body.append(frame);
    }
    form(title, &body, ok_label, move || {
        if let Err(message) = check_title(&name.text(), &taken) {
            error.set_label(message);
            return false;
        }
        on_ok(Fields {
            name: name.text().trim().to_owned(),
            content: text_of(&content),
            long_press: text_of(&long_press),
            startup: text_of(&startup),
        });
        true
    });
}

fn confirm_delete(screen: &str, dialog: &str, sample: &str, name: &str, on_ok: impl Fn() + 'static) {
    let Some(mut d) = spec_dialog(screen, dialog) else { return };
    d.text = d.text.map(|t| quoted_text(&t, sample, name));
    crate::ui::show_dialog_then(&d, on_ok);
}

// ---------- custom buttons ----------

struct ButtonsView {
    service: CustomButtonService,
    list: gtk::ListBox,
    empty: Label,
}

impl ButtonsView {
    fn render(self: &Rc<Self>) {
        while let Some(row) = self.list.first_child() {
            self.list.remove(&row);
        }
        let buttons = self.service.list().unwrap_or_else(|e| {
            tracing::warn!(error = %e, "could not read the custom buttons");
            Vec::new()
        });
        self.empty.set_visible(buttons.is_empty());
        let last = buttons.len().saturating_sub(1);
        for (at, button) in buttons.iter().enumerate() {
            self.list.append(&self.row(button, at, last, &buttons));
        }
    }

    fn row(self: &Rc<Self>, button: &CustomButton, at: usize, last: usize, all: &[CustomButton]) -> gtk::ListBoxRow {
        let h = GBox::new(O::Horizontal, 6);
        margins(&h, 8);
        h.append(&Label::builder().label(&button.name).xalign(0.0).hexpand(true).ellipsize(gtk::pango::EllipsizeMode::End).build());
        let id = button.id;
        let act = |view: &Rc<Self>, f: Box<dyn Fn(&CustomButtonService)>| {
            let view = Rc::clone(view);
            move |_: &Button| {
                f(&view.service);
                view.render();
            }
        };
        let up = icon_button("go-up-symbolic", "Move up");
        up.set_sensitive(at > 0);
        up.connect_clicked(act(self, Box::new(move |s| drop(s.reorder(id, at.saturating_sub(1))))));
        let down = icon_button("go-down-symbolic", "Move down");
        down.set_sensitive(at < last);
        down.connect_clicked(act(self, Box::new(move |s| drop(s.reorder(id, at + 1)))));
        let star = gtk::ToggleButton::builder()
            .icon_name(if button.is_favorite { "starred-symbolic" } else { "non-starred-symbolic" })
            .tooltip_text("Primary button")
            .active(button.is_favorite)
            .build();
        let view = Rc::clone(self);
        star.connect_clicked(move |_| {
            drop(view.service.toggle_favorite(id));
            view.render();
        });
        let edit = icon_button("document-edit-symbolic", "Edit");
        let (view, current) = (Rc::clone(self), button.clone());
        let taken: Vec<String> = all.iter().filter(|b| b.id != id).map(|b| b.name.clone()).collect();
        edit.connect_clicked(move |_| {
            let (view, id) = (Rc::clone(&view), current.id);
            button_form(&format!("Edit button (ID: {id})"), "Edit", Some(&current), taken.clone(), move |f| {
                let update = CustomButtonUpdate {
                    name: Some(f.name),
                    content: Some(f.content),
                    long_press_content: Some(f.long_press),
                    on_startup: Some(f.startup),
                    ..CustomButtonUpdate::new(id)
                };
                if let Err(e) = view.service.update(&update) {
                    tracing::warn!(error = %e, "the custom button was not saved");
                }
                view.render();
            });
        });
        let delete = icon_button("user-trash-symbolic", "Delete");
        let (view, name) = (Rc::clone(self), button.name.clone());
        delete.connect_clicked(move |_| {
            let view = Rc::clone(&view);
            confirm_delete("settings_player_custom_buttons", "delete_button", "Button", &name, move || {
                drop(view.service.delete(id));
                view.render();
            });
        });
        for w in [up.upcast::<gtk::Widget>(), down.upcast(), star.upcast(), edit.upcast(), delete.upcast()] {
            h.append(&w);
        }
        gtk::ListBoxRow::builder().child(&h).activatable(false).build()
    }
}

/// The custom button list with its Add button.
pub fn buttons_page() -> Option<Built> {
    let service = CustomButtonService::new(CustomButtonRepo::new(crate::data::app().anime_db.clone()));
    let empty_text = crate::ui::screen("settings_player_custom_buttons").empty.unwrap_or_default();
    let view = Rc::new(ButtonsView {
        service,
        list: boxed_list(),
        empty: Label::builder().label(&empty_text).wrap(true).xalign(0.0).build(),
    });
    let col = column();
    col.append(&view.list);
    margins(&view.empty, 12);
    col.append(&view.empty);
    view.render();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scrolled(&col)));
    let add = crate::screens::fab(&overlay, "Add", "list-add-symbolic");
    add.connect_clicked(move |_| {
        let taken: Vec<String> = view.service.list().unwrap_or_default().into_iter().map(|b| b.name).collect();
        let view = Rc::clone(&view);
        button_form("Add button", "Add", None, taken, move |f| {
            if let Err(e) = view.service.create(&f.name, &f.content, &f.long_press, &f.startup) {
                tracing::warn!(error = %e, "the custom button was not created");
            }
            view.render();
        });
    });
    built(overlay, crate::ui::toolbar(&crate::ui::screen("settings_player_custom_buttons")))
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
    empty: Label,
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
        let r = crate::ui::row(name, None, Some(menu.upcast()));
        if let Some(inner) = r.child().and_downcast::<GBox>() {
            inner.prepend(&gtk::Image::from_icon_name(kind.icon()));
        }
        r.set_activatable(true);
        let file = name.to_owned();
        r.connect_activate(move |_| {
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
        ask_text(&title, "Filename", &initial, "Edit", move |typed| files::checked_name(typed, kind, &existing), move |to| {
            if let Some(dir) = folder(kind) {
                if let Err(e) = files::rename(&dir, &from, &to) {
                    tracing::warn!(error = %e, "the script was not renamed");
                }
            }
            view.render();
        });
    }

    fn delete(self: Rc<Self>, name: &str) {
        let (kind, file) = (self.kind.get(), name.to_owned());
        confirm_delete("settings_player_editor", "delete_file", "file", name, move || {
            if let Some(dir) = folder(kind) {
                if let Err(e) = files::delete(&dir, &file) {
                    tracing::warn!(error = %e, "the script was not deleted");
                }
            }
            self.render();
        });
    }
}

/// The file list, a Scripts | Script options switch and the Add button.
pub fn editor_page() -> Option<Built> {
    let view = Rc::new(EditorView {
        kind: Cell::new(Kind::Scripts),
        list: boxed_list(),
        empty: Label::builder()
            .label(crate::ui::screen("settings_player_editor").empty.unwrap_or_default())
            .wrap(true)
            .xalign(0.0)
            .build(),
    });
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
    margins(&view.empty, 12);
    col.append(&view.empty);
    view.render();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scrolled(&col)));
    let add = crate::screens::fab(&overlay, "Add", "list-add-symbolic");
    add.connect_clicked(move |_| {
        let kind = view.kind.get();
        let existing = view.existing();
        let view = Rc::clone(&view);
        let title = spec_dialog("settings_player_editor", "create_file").map_or("Create file".to_owned(), |d| d.title);
        ask_text(&title, "Filename", "", "Add", move |typed| files::checked_name(typed, kind, &existing), move |name| {
            if let Some(dir) = folder(kind) {
                if let Err(e) = files::create(&dir, &name) {
                    tracing::warn!(error = %e, "the script was not created");
                }
            }
            view.render();
        });
    });
    built(overlay, vec![])
}

// ---------- code editor ----------

/// The code editor over the file the list opened. Save is enabled after an edit and writes the file back.
pub fn code_editor_page() -> Option<Built> {
    let Some((dir, name)) = OPEN_FILE.with(|f| f.borrow().clone()) else {
        return built(crate::ui::status("dialog-information-symbolic", "No file open"), vec![]);
    };
    let path = dir.join(&name);
    let text = std::fs::read(&path).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let buffer = gtk::TextBuffer::new(None);
    buffer.set_text(&text);
    let view = gtk::TextView::builder().buffer(&buffer).monospace(true).left_margin(12).top_margin(12).build();
    let save = icon_button("document-save-symbolic", "Save");
    save.set_sensitive(false);
    let on_edit = save.clone();
    buffer.connect_changed(move |_| on_edit.set_sensitive(true));
    save.connect_clicked(move |b| {
        let text = buffer.text(&buffer.start_iter(), &buffer.end_iter(), false);
        match std::fs::write(&path, text.as_str()) {
            Ok(()) => b.set_sensitive(false),
            Err(e) => tracing::warn!(error = %e, "the script was not saved"),
        }
    });
    let body = GBox::new(O::Vertical, 0);
    body.append(&Label::builder().label(&name).xalign(0.0).css_classes(["heading"]).margin_start(12).margin_top(6).build());
    body.append(&gtk::ScrolledWindow::builder().child(&view).vexpand(true).build());
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
            ] {
                let found = spec.screen(screen).is_some_and(|s| s.dialogs.iter().any(|d| d.id == dialog));
                assert!(found, "{screen}/{dialog}");
            }
        });
    }
}
