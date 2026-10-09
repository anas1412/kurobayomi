//! The live library, categories, history and updates screens (T-08a-033, T-08a-034). Rows come from the app's
//! databases ([`crate::data`]). Each screen renders once, then again on the GTK main loop whenever its live query
//! publishes (see [`crate::watch::bind_watch`]).

use std::cell::Cell;
use std::rc::Rc;

use chrono::{Local, Months};
use gtk::glib;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, ListBox, Orientation as O};
use kuro_core::text::primary_cmp;
use kuro_data::anime::AnimeRepo;
use kuro_data::manga::MangaRepo;
use kuro_domain::category::model::Category;
use kuro_domain::dates::ListRow;
use kuro_domain::history::repo::HistoryRepo;
use kuro_domain::history::view::{HistoryLine, build_lines};
use kuro_domain::kind::EntryKind;
use kuro_domain::library::model::LibraryRow;
use kuro_domain::library::repo::LibraryRepo;
use kuro_domain::updates::repo::UpdatesRepo;
use kuro_domain::updates::tab::{UpdateLine, group};

use crate::data::{self, App};
use crate::screens::{badge, dialog_then, fab, list_row, open_media, thumb};
use crate::ui::{boxed_list, cover, dim, heading, icon_button, margins, modal, scrolled, screen};
use crate::watch::bind_watch;

/// Milliseconds in the Updates window: the last three months, as the Android Updates tab shows.
fn updates_since() -> i64 {
    Local::now().checked_sub_months(Months::new(3)).map_or(0, |t| t.timestamp_millis())
}

fn kind_of(anime: bool) -> EntryKind {
    if anime { EntryKind::Anime } else { EntryKind::Manga }
}

fn clear_box(b: &impl IsA<gtk::Widget>) {
    let b = b.upcast_ref::<gtk::Widget>();
    while let Some(child) = b.first_child() {
        child.unparent();
    }
}

fn clear_list(list: &ListBox) {
    while let Some(row) = list.first_child() {
        list.remove(&row);
    }
}

/// The library tabs of one kind: `(category id, label)`. The Default page (id 0) is kept when an entry is in it, or
/// when the library is empty; the visible categories follow in their order.
fn library_pages(app: &App, kind: EntryKind, rows: &[LibraryRow]) -> Vec<(i64, String)> {
    let mut pages = Vec::new();
    if rows.is_empty() || rows.iter().any(|r| r.category == 0) {
        pages.push((0, "Default".to_owned()));
    }
    let visible = app.categories(kind).repo().visible().unwrap_or_default();
    pages.extend(visible.into_iter().map(|c| (c.id, c.name)));
    pages
}

/// One library body: the category tabs over a grid of covers. The grid shows the page of the active tab.
struct LibraryView {
    anime: bool,
    grid: gtk::FlowBox,
    tabs: GBox,
    active: Cell<usize>,
}

impl LibraryView {
    fn render(self: &Rc<Self>) {
        let app = data::app();
        let kind = kind_of(self.anime);
        let rows = LibraryRepo::new(app.db(kind)).all(kind).unwrap_or_default();
        let pages = library_pages(&app, kind, &rows);
        let active = self.active.get().min(pages.len().saturating_sub(1));
        self.active.set(active);
        clear_box(&self.tabs);
        let mut first: Option<gtk::ToggleButton> = None;
        for (i, (_, name)) in pages.iter().enumerate() {
            let tab = gtk::ToggleButton::with_label(name);
            match &first {
                Some(f) => tab.set_group(Some(f)),
                None => first = Some(tab.clone()),
            }
            tab.set_active(i == active);
            let view = Rc::clone(self);
            tab.connect_toggled(move |b| {
                if b.is_active() {
                    view.active.set(i);
                    view.render_cards();
                }
            });
            self.tabs.append(&tab);
        }
        self.render_cards();
    }

    fn render_cards(&self) {
        let app = data::app();
        let kind = kind_of(self.anime);
        let rows = LibraryRepo::new(app.db(kind)).all(kind).unwrap_or_default();
        let pages = library_pages(&app, kind, &rows);
        while let Some(child) = self.grid.first_child() {
            self.grid.remove(&child);
        }
        let Some((page, _)) = pages.get(self.active.get()) else { return };
        let mut members: Vec<&LibraryRow> = rows.iter().filter(|r| r.category == *page).collect();
        members.sort_by(|a, b| primary_cmp(&a.title, &b.title));
        members.dedup_by_key(|r| r.id);
        if members.is_empty() {
            // the library's own empty text from the spec; "Category is empty" is the plan's text (not a spec key)
            let text = if rows.is_empty() {
                screen(if self.anime { "anime_library" } else { "manga_library" }).empty.unwrap_or_default()
            } else {
                "Category is empty".to_owned()
            };
            self.grid.insert(&dim(&text), -1);
            return;
        }
        for row in members {
            self.grid.insert(&self.card(row), -1);
        }
    }

    /// A cover card: the unread badge when entries are pending, a resume button and the title as the last label.
    fn card(&self, row: &LibraryRow) -> GBox {
        let anime = self.anime;
        let card = GBox::new(O::Vertical, 4);
        let ov = gtk::Overlay::new();
        ov.set_child(Some(&cover(140, 200)));
        let pending = row.pending();
        if pending > 0 {
            let badges = GBox::new(O::Horizontal, 0);
            badges.add_css_class("linked");
            badges.set_halign(Align::Start);
            badges.set_valign(Align::Start);
            margins(&badges, 4);
            badges.append(&badge(&format!(" {pending} "), "accent"));
            ov.add_overlay(&badges);
        }
        let resume = Button::builder()
            .icon_name("media-playback-start-symbolic")
            .css_classes(["circular", "osd"])
            .halign(Align::End)
            .valign(Align::End)
            .tooltip_text("Resume")
            .build();
        margins(&resume, 6);
        let title = row.title.clone();
        let t = title.clone();
        resume.connect_clicked(move |_| open_media(anime, &t, if anime { "Episode 2" } else { "Chapter 2" }));
        ov.add_overlay(&resume);
        card.append(&ov);
        card.append(
            &Label::builder()
                .label(&title)
                .xalign(0.0)
                .lines(2)
                .wrap(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .max_width_chars(16)
                .build(),
        );
        card
    }
}

/// Fills the library grid and the tabs of one kind, and keeps them in step with the library rows.
pub fn attach_library(grid: &gtk::FlowBox, tabs: &GBox, anime: bool) {
    let view = Rc::new(LibraryView { anime, grid: grid.clone(), tabs: tabs.clone(), active: Cell::new(0) });
    view.render();
    let app = data::app();
    let db = app.db(kind_of(anime));
    if anime {
        bind_watch(AnimeRepo::new(db).subscribe_library(), move |_| view.render());
    } else {
        bind_watch(MangaRepo::new(db).subscribe_library(), move |_| view.render());
    }
}

/// The error shown when a category name is already taken (`spec/app.json`, categories screen notes).
const DUPLICATE_CATEGORY: &str = "A category with this name already exists!";

/// The name dialog of the categories screen: the spec's title and buttons, with a text entry. The last button
/// calls `on_ok` with the trimmed name. An empty name keeps the dialog open, and so does a name that `taken`
/// reports as used, with [`DUPLICATE_CATEGORY`] shown under the entry.
fn name_dialog(dialog: &str, initial: &str, taken: impl Fn(&str) -> bool + 'static, on_ok: impl Fn(String) + 'static) {
    let Some(spec) = screen("categories").dialogs.into_iter().find(|d| d.id == dialog) else { return };
    let labels = if spec.buttons.is_empty() { vec!["OK".to_owned()] } else { spec.buttons.clone() };
    let col = GBox::new(O::Vertical, 12);
    margins(&col, 18);
    let entry = gtk::Entry::builder().text(initial).activates_default(true).build();
    col.append(&entry);
    let error = Label::builder().css_classes(["error"]).xalign(0.0).wrap(true).visible(false).build();
    col.append(&error);
    let taken = Rc::new(taken);
    let buttons = GBox::new(O::Horizontal, 6);
    buttons.set_halign(Align::End);
    col.append(&buttons);
    let win = modal(&Label::builder().label(&spec.title).css_classes(["title"]).build().upcast(), &scrolled(&col), 420, -1);
    let on_ok = Rc::new(on_ok);
    let last = labels.len().saturating_sub(1);
    for (i, label) in labels.iter().enumerate() {
        let button = Button::with_label(label);
        if i == last {
            button.add_css_class("suggested-action");
        }
        let (win, entry, on_ok, error, taken) = (win.clone(), entry.clone(), Rc::clone(&on_ok), error.clone(), Rc::clone(&taken));
        let is_ok = i == last;
        button.connect_clicked(move |_| {
            if !is_ok {
                win.close();
                return;
            }
            let name = entry.text().trim().to_owned();
            if name.is_empty() {
                return;
            }
            if taken(&name) {
                error.set_text(DUPLICATE_CATEGORY);
                error.set_visible(true);
                return;
            }
            on_ok(name);
            win.close();
        });
        buttons.append(&button);
    }
}

/// One category row list of one kind. Rename, hide and delete write through the category service.
struct CategoryList {
    kind: EntryKind,
    list: ListBox,
}

/// Whether a user category other than `except` already has the trimmed name `new` (the same exact-name rule the
/// category service applies on create and rename; the system category is never a match).
fn user_names_taken(kind: EntryKind, new: &str, except: Option<i64>) -> bool {
    let all = data::app().categories(kind).repo().all().unwrap_or_default();
    taken_by_user(&all, new, except)
}

/// The pure part of [`user_names_taken`] over a list of categories.
fn taken_by_user(all: &[Category], new: &str, except: Option<i64>) -> bool {
    all.iter().any(|c| !c.is_system() && Some(c.id) != except && c.name.trim() == new)
}

impl CategoryList {
    fn render(self: &Rc<Self>) {
        clear_list(&self.list);
        let app = data::app();
        let cats: Vec<_> = app.categories(self.kind).repo().all().unwrap_or_default().into_iter().filter(|c| !c.is_system()).collect();
        if cats.is_empty() {
            self.list.append(&dim(&screen("categories").empty.unwrap_or_default()));
        }
        for c in cats {
            let (id, name, kind) = (c.id, c.name.clone(), self.kind);
            let edit = icon_button("document-edit-symbolic", "Rename category");
            let rename_from = name.clone();
            edit.connect_clicked(move |_| {
                let app = data::app();
                let taken = move |new: &str| user_names_taken(kind, new, Some(id));
                name_dialog("category_rename", &rename_from, taken, move |new| {
                    let app = Rc::clone(&app);
                    glib::spawn_future_local(async move {
                        let _ = app.categories(kind).rename(id, &new).await;
                    });
                });
            });
            let hide = gtk::ToggleButton::builder()
                .icon_name(if c.hidden { "view-conceal-symbolic" } else { "view-reveal-symbolic" })
                .tooltip_text("Hide")
                .active(c.hidden)
                .build();
            hide.connect_toggled(move |_| {
                let app = data::app();
                glib::spawn_future_local(async move {
                    let _ = app.categories(kind).toggle_hidden(id).await;
                });
            });
            let del = icon_button("user-trash-symbolic", "Delete");
            del.connect_clicked(move |_| {
                dialog_then("categories", "category_delete", move || {
                    let app = data::app();
                    glib::spawn_future_local(async move {
                        let _ = app.categories(kind).delete(id).await;
                    });
                });
            });
            let handle = gtk::Image::from_icon_name("list-drag-handle-symbolic");
            let drag = gtk::DragSource::new();
            drag.set_actions(gtk::gdk::DragAction::MOVE);
            drag.connect_prepare(move |_, _, _| Some(gtk::gdk::ContentProvider::for_value(&id.to_value())));
            handle.add_controller(drag);
            let subtitle = if c.hidden { "Hidden" } else { "" };
            let row = list_row(Some(handle.upcast()), &name, subtitle, vec![edit.upcast(), hide.upcast(), del.upcast()]);
            // Dropping a dragged category onto a row moves it before that row, through the category service.
            let drop = gtk::DropTarget::new(<i64 as gtk::glib::prelude::StaticType>::static_type(), gtk::gdk::DragAction::MOVE);
            drop.connect_drop(move |_, value, _, _| {
                let Ok(dragged) = value.get::<i64>() else { return false };
                if dragged == id {
                    return false;
                }
                let app = data::app();
                glib::spawn_future_local(async move {
                    let _ = app.categories(kind).move_before(dragged, Some(id)).await;
                });
                true
            });
            row.add_controller(drop);
            self.list.append(&row);
        }
    }
}

/// The categories screen of one kind: a live list of the non-system categories with an Add button.
pub fn categories_content(anime: bool) -> gtk::Widget {
    let kind = kind_of(anime);
    let list = boxed_list();
    let column = GBox::new(O::Vertical, 0);
    margins(&column, 12);
    column.append(&list);
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&scrolled(&column)));
    let view = Rc::new(CategoryList { kind, list });
    view.render();
    bind_watch(data::app().categories(kind).repo().watch_all(), move |_| view.render());
    fab(&overlay, "Add", "list-add-symbolic").connect_clicked(move |_| {
        name_dialog("category_create", "", move |new| user_names_taken(kind, new, None), move |name| {
            let app = data::app();
            glib::spawn_future_local(async move {
                let _ = app.categories(kind).create(&name).await;
            });
        });
    });
    overlay.upcast()
}

/// The history list of one kind, grouped under date headers, with a remove button per row.
struct HistoryView {
    kind: EntryKind,
    col: GBox,
}

impl HistoryView {
    fn render(self: &Rc<Self>) {
        clear_box(&self.col);
        let app = data::app();
        let rows = HistoryRepo::new(app.db(self.kind)).list(self.kind, "").unwrap_or_default();
        let lines = build_lines(&rows, self.kind, Local::now().date_naive(), &Local, true, None);
        let mut current: Option<ListBox> = None;
        for line in lines {
            match line {
                ListRow::Header(text) => {
                    self.col.append(&heading(&text));
                    let list = boxed_list();
                    self.col.append(&list);
                    current = Some(list);
                }
                ListRow::Item(line) => {
                    let list = current.get_or_insert_with(|| {
                        let list = boxed_list();
                        self.col.append(&list);
                        list
                    });
                    list.append(&self.row(&line));
                }
            }
        }
    }

    fn row(&self, line: &HistoryLine) -> gtk::ListBoxRow {
        let anime = self.kind == EntryKind::Anime;
        let remove = icon_button("user-trash-symbolic", "Delete");
        let (kind, history_id) = (self.kind, line.history_id);
        let dialog = if anime { "history_remove_anime" } else { "history_remove_manga" };
        remove.connect_clicked(move |_| {
            dialog_then("history", dialog, move || {
                let app = data::app();
                let _ = HistoryRepo::new(app.db(kind)).reset_one(kind, history_id);
            });
        });
        let row = list_row(Some(thumb(48, 70).upcast()), &line.title, &line.text, vec![remove.upcast()]);
        let (title, text) = (line.title.clone(), line.text.clone());
        row.connect_activate(move |_| open_media(anime, &title, &text));
        row
    }
}

/// The history screen of one kind. The rows follow the history table.
pub fn history_content(anime: bool) -> gtk::Widget {
    let kind = kind_of(anime);
    let col = GBox::new(O::Vertical, 6);
    margins(&col, 12);
    let view = Rc::new(HistoryView { kind, col: col.clone() });
    view.render();
    bind_watch(HistoryRepo::new(data::app().db(kind)).watch_list(kind, String::new()), move |_| view.render());
    scrolled(&col).upcast()
}

/// `history_remove_everything` for the history of one kind, then clears it.
pub fn confirm_clear_history(anime: bool) {
    let kind = kind_of(anime);
    dialog_then("history", "history_remove_everything", move || {
        let app = data::app();
        HistoryRepo::new(app.db(kind)).clear_all(kind);
    });
}

/// The updates feed of one kind, grouped by day over the last three months.
struct UpdatesView {
    kind: EntryKind,
    since: i64,
    col: GBox,
}

impl UpdatesView {
    fn render(self: &Rc<Self>) {
        clear_box(&self.col);
        let app = data::app();
        let rows = UpdatesRepo::new(app.db(self.kind)).recent(self.kind, self.since).unwrap_or_default();
        for item in group(&rows, Local::now().date_naive(), &Local, true, None) {
            match item {
                ListRow::Header(text) => self.col.append(&heading(&text)),
                ListRow::Item(line) => {
                    let list = boxed_list();
                    list.append(&self.row(&line));
                    self.col.append(&list);
                }
            }
        }
    }

    fn row(&self, line: &UpdateLine) -> gtk::ListBoxRow {
        let anime = self.kind == EntryKind::Anime;
        let pre = thumb(40, 40);
        let row = list_row(Some(pre.upcast()), &line.entry_title, &line.text, Vec::new());
        let (title, text) = (line.entry_title.clone(), line.text.clone());
        row.connect_activate(move |_| open_media(anime, &title, &text));
        row
    }
}

/// The updates screen of one kind. The feed follows the updates table.
pub fn updates_content(anime: bool) -> gtk::Widget {
    let kind = kind_of(anime);
    let since = updates_since();
    let col = GBox::new(O::Vertical, 6);
    margins(&col, 12);
    let view = Rc::new(UpdatesView { kind, since, col: col.clone() });
    view.render();
    bind_watch(UpdatesRepo::new(data::app().db(kind)).watch_recent(kind, since), move |_| view.render());
    scrolled(&col).upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cat(id: i64, name: &str) -> Category {
        Category { id, name: name.to_owned(), order: id, flags: 0, hidden: false }
    }

    #[test]
    fn a_name_used_by_another_user_category_is_taken_but_its_own_name_is_not() {
        let all = [cat(0, "Default"), cat(1, "Reading"), cat(2, "Later ")];
        assert!(taken_by_user(&all, "Reading", None));
        assert!(taken_by_user(&all, "Later", None), "stored names are compared trimmed");
        assert!(!taken_by_user(&all, "Reading", Some(1)), "renaming to the current name is not a duplicate");
        assert!(!taken_by_user(&all, "Default", None), "the system category never blocks a name");
        assert!(!taken_by_user(&all, "reading", None), "names are exact, not case-folded, as in the service");
    }
}
