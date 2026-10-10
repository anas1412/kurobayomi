//! The History screen of one kind (T-42a-013 to T-42a-024): one implementation over [`EntryKind`] that the live
//! screen mounts for Anime and for Manga. State is `{ list, dialog, search }` ([`HistoryState`]); the query re-runs
//! on a table change with the 50 ms coalesce and on every search change, which cancels the previous query. Rows are
//! [`history_row`]; a click resumes, the cover opens the entry, the trash button asks the remove dialog.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Arc;

use chrono::Local;
use gtk::glib;
use gtk::prelude::*;
use gtk::{Box as GBox, ListBox, Orientation as O};
use crate::browse::entries::FavoriteStore;
use crate::browse::favorite::{DuplicateAnswer, duplicate_answer, duplicate_dialog};
use kuro_domain::dates::ListRow;
use kuro_domain::history::model::HistoryWithRelations;
use kuro_domain::history::repo::HistoryRepo;
use kuro_domain::history::view::{HistoryLine, remove_action};
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::player::PlayerPrefs;
use kuro_prefs::ui::UiPrefs;

use crate::data;
use crate::updates::coalesce::watch_coalesced;
use crate::updates::history::{
    COALESCE_MS, CLEAR_DONE_TOAST_MS, HistoryDialog, HistoryState, NOT_FOUND_TOAST_MS, Resume, clear_outcome, empty_message,
    history_lines, remove_dialog,
};
use crate::updates::history_add::{self, Step};
use crate::updates::history_data::{self, Unit};
use crate::updates::history_dialog::{self, ConfirmCopy};
use crate::updates::history_row::{RowActions, history_row};
use crate::updates::list::PageState;
use crate::updates::screen_toast::ScreenToast;
use crate::updates::state_page::PageStates;

/// Whether a resume plays in the external player: the preference applies to anime only (the reader has no external
/// mode).
pub fn uses_external(always_external: bool, kind: EntryKind) -> bool {
    always_external && kind == EntryKind::Anime
}

pub struct HistoryScreen {
    kind: EntryKind,
    root: gtk::Overlay,
    search: gtk::SearchEntry,
    col: GBox,
    pages: PageStates,
    toast: ScreenToast,
    state: RefCell<HistoryState<HistoryWithRelations>>,
    watch: RefCell<Option<glib::JoinHandle<()>>>,
}

impl HistoryScreen {
    /// Builds the screen and starts its query.
    pub fn new(kind: EntryKind) -> Rc<Self> {
        let screen = Rc::new_cyclic(|weak: &Weak<Self>| {
            let col = GBox::new(O::Vertical, 6);
            crate::ui::margins(&col, 12);
            let list = crate::ui::scrolled(&col);
            let retry = weak.clone();
            let pages = PageStates::new(&list, move || {
                if let Some(screen) = retry.upgrade() {
                    screen.state.borrow_mut().toast = None;
                    screen.start();
                }
            });
            let search = gtk::SearchEntry::builder().placeholder_text(tr(MsgKey::action_search)).margin_start(12).margin_end(12).margin_top(6).build();
            let body = GBox::new(O::Vertical, 0);
            body.append(&search);
            body.append(pages.widget());
            let root = gtk::Overlay::new();
            root.set_child(Some(&body));
            let toast = ScreenToast::new();
            root.add_overlay(toast.widget());
            Self { kind, root, search, col, pages, toast, state: RefCell::default(), watch: RefCell::default() }
        });
        let weak = Rc::downgrade(&screen);
        screen.search.connect_search_changed(move |entry| {
            if let Some(screen) = weak.upgrade() {
                let changed = screen.state.borrow_mut().set_search(&entry.text());
                if changed {
                    screen.start();
                }
            }
        });
        screen.start();
        screen
    }

    pub fn widget(&self) -> &gtk::Overlay {
        &self.root
    }

    fn db(&self) -> std::sync::Arc<kuro_db::handle::Db> {
        data::app().db(self.kind)
    }

    /// (Re)starts the live query for the current search text. The previous run is aborted.
    fn start(self: &Rc<Self>) {
        if let Some(previous) = self.watch.borrow_mut().take() {
            previous.abort();
        }
        let query = self.state.borrow().search.clone();
        let sub = HistoryRepo::new(self.db()).watch_list(self.kind, query);
        let weak = Rc::downgrade(self);
        let handle = watch_coalesced(sub, COALESCE_MS, move |published| {
            let Some(screen) = weak.upgrade() else { return };
            match published {
                Ok(rows) => screen.state.borrow_mut().set_rows((*rows).clone()),
                Err(message) => {
                    screen.state.borrow_mut().set_error(message);
                    screen.toast.show(&tr(MsgKey::internal_error), NOT_FOUND_TOAST_MS);
                }
            }
            screen.render();
        });
        *self.watch.borrow_mut() = Some(handle);
        self.render();
    }

    fn render(self: &Rc<Self>) {
        let (list, searching, error) = {
            let state = self.state.borrow();
            (state.list.clone(), !state.search.is_empty(), state.toast.clone())
        };
        let page = match &list {
            None => error.map_or(PageState::Loading, PageState::Error),
            Some(rows) if rows.is_empty() => PageState::Empty(empty_message(self.kind, searching)),
            Some(_) => PageState::List,
        };
        if let Some(rows) = &list {
            self.fill(rows);
        }
        self.pages.show(&page);
    }

    fn fill(self: &Rc<Self>, rows: &[HistoryWithRelations]) {
        while let Some(child) = self.col.first_child() {
            child.unparent();
        }
        let store = data::app().base.store().clone();
        let ui = UiPrefs::new(store);
        let lines = history_lines(
            rows,
            self.kind,
            Local::now().date_naive(),
            &Local,
            ui.relative_time.get(),
            &ui.date_format.get(),
            kuro_i18n::current_locale().tag,
        );
        let mut current: Option<ListBox> = None;
        for line in lines {
            match line {
                ListRow::Header(text) => {
                    self.col.append(&crate::ui::heading(&text));
                    let list = crate::ui::boxed_list();
                    self.col.append(&list);
                    current = Some(list);
                }
                ListRow::Item(line) => {
                    let list = current.get_or_insert_with(|| {
                        let list = crate::ui::boxed_list();
                        self.col.append(&list);
                        list
                    });
                    list.append(&self.row(&line));
                }
            }
        }
    }

    fn row(self: &Rc<Self>, line: &HistoryLine) -> gtk::ListBoxRow {
        let weak = Rc::downgrade(self);
        let act = |f: fn(&Rc<Self>, &HistoryLine)| {
            let (weak, line) = (weak.clone(), line.clone());
            Rc::new(move || {
                if let Some(screen) = weak.upgrade() {
                    f(&screen, &line);
                }
            }) as Rc<dyn Fn()>
        };
        let actions = RowActions {
            open_entry: act(Self::open_entry),
            resume: act(Self::resume_line),
            remove: act(Self::ask_remove),
            add_to_library: act(Self::add_to_library),
        };
        history_row(&line.title, &line.text, &gtk::Picture::new(), line.cover.favorite, actions)
    }

    fn open_entry(self: &Rc<Self>, line: &HistoryLine) {
        if let Some(title) = history_data::entry_title(&self.db(), self.kind, line.entry_id) {
            crate::screens::open_entry(&title, self.kind == EntryKind::Anime);
        }
    }

    /// The favourite button: duplicate check, then the default category, as the Browse add does (T-42a-019).
    fn add_to_library(self: &Rc<Self>, line: &HistoryLine) {
        let store = history_add::store(self.kind);
        let title = history_data::entry_title(&self.db(), self.kind, line.entry_id).unwrap_or_default();
        let now = kuro_data::time::now_millis();
        match history_add::start(store.as_ref(), line.entry_id, &title, now) {
            Ok(Step::Added) => {}
            Ok(Step::Chooser) => self.choose_categories(store, line.entry_id, now),
            Ok(Step::Duplicate(existing)) => self.duplicate_dialog(store, line.entry_id, existing[0], now),
            Err(_) => self.toast.show(&tr(MsgKey::internal_error), NOT_FOUND_TOAST_MS),
        }
    }

    /// The category chooser: the entry's current links are checked; OK writes them, then the favourite.
    fn choose_categories(self: &Rc<Self>, store: Arc<dyn FavoriteStore>, entry_id: i64, now: i64) {
        let rows = match history_add::chooser_options(store.as_ref(), entry_id) {
            Ok(rows) => rows.into_iter().map(|(c, on)| (c.id, c.name, on)).collect(),
            Err(_) => return self.toast.show(&tr(MsgKey::internal_error), NOT_FOUND_TOAST_MS),
        };
        let labels = [tr(MsgKey::action_ok), tr(MsgKey::action_cancel), tr(MsgKey::action_edit_categories)];
        let (weak, on_store) = (Rc::downgrade(self), Arc::clone(&store));
        history_dialog::choose_categories(
            tr(MsgKey::action_move_category),
            rows,
            labels,
            move |picked| {
                if let Some(screen) = weak.upgrade() {
                    if history_add::confirm_chooser(on_store.as_ref(), entry_id, now, picked).is_err() {
                        screen.toast.show(&tr(MsgKey::internal_error), NOT_FOUND_TOAST_MS);
                    }
                }
            },
            move || crate::ui::open("categories"),
        );
    }

    /// The duplicate dialog: show the existing entry, migrate it, add anyway, or cancel (nothing is written).
    fn duplicate_dialog(self: &Rc<Self>, store: Arc<dyn FavoriteStore>, entry_id: i64, existing: i64, now: i64) {
        let copy = duplicate_dialog();
        let labels = vec![copy.show, copy.migrate, copy.add_anyway, copy.cancel];
        let (weak, anime) = (Rc::downgrade(self), self.kind == EntryKind::Anime);
        let db = self.db();
        let kind = self.kind;
        history_dialog::pick(copy.title, copy.body, labels, move |index| {
            let Some(screen) = weak.upgrade() else { return };
            match duplicate_answer(Some(index)) {
                DuplicateAnswer::ShowExisting => {
                    if let Some(title) = history_data::entry_title(&db, kind, existing) {
                        crate::screens::open_entry(&title, anime);
                    }
                }
                DuplicateAnswer::MigrateExisting => {
                    let title = history_data::entry_title(&db, kind, existing).unwrap_or_default();
                    crate::entry_core::handoff::open_migrate(crate::entry_core::handoff::MigrateRequest { entry_id: existing, anime, title });
                }
                DuplicateAnswer::AddAnyway => match history_add::add_anyway(store.as_ref(), entry_id, now) {
                    Ok(Step::Chooser) => screen.choose_categories(Arc::clone(&store), entry_id, now),
                    Ok(_) => {}
                    Err(_) => screen.toast.show(&tr(MsgKey::internal_error), NOT_FOUND_TOAST_MS),
                },
                DuplicateAnswer::Nothing => {}
            }
        });
    }

    /// A click on a row: the first next unit of its entry opens, or the not-found toast shows.
    fn resume_line(self: &Rc<Self>, line: &HistoryLine) {
        let outcome = history_data::resume_row(&self.db(), self.kind, line.entry_id, line.item_id);
        self.finish_resume(line.entry_id, outcome);
    }

    /// Resume-last: the newest history row of this kind. With no history nothing happens.
    pub fn resume_last(self: &Rc<Self>) {
        if let Some((entry_id, outcome)) = history_data::resume_last(&self.db(), self.kind) {
            self.finish_resume(entry_id, outcome);
        }
    }

    fn finish_resume(&self, entry_id: i64, outcome: Resume<Unit>) {
        match outcome {
            Resume::Open(unit) => open_unit(&self.db(), self.kind, entry_id, &unit),
            Resume::NotFound { message, toast_ms } => self.toast.show(&message, toast_ms),
        }
    }

    fn ask_remove(self: &Rc<Self>, line: &HistoryLine) {
        self.state.borrow_mut().open_dialog(HistoryDialog::Remove { history_id: line.history_id, entry_id: line.entry_id });
        let copy = remove_dialog(self.kind);
        let (confirm, close, line) = (Rc::downgrade(self), Rc::downgrade(self), line.clone());
        history_dialog::show(
            ConfirmCopy { title: copy.title, text: copy.text, checkbox: Some(copy.reset_label), confirm: copy.confirm, cancel: copy.cancel },
            true,
            move |reset_all| {
                if let Some(screen) = confirm.upgrade() {
                    screen.confirm_remove(&line, reset_all);
                }
            },
            move || {
                if let Some(screen) = close.upgrade() {
                    screen.state.borrow_mut().dismiss_dialog();
                }
            },
        );
    }

    /// The checkbox on resets the whole entry; off resets this row.
    fn confirm_remove(self: &Rc<Self>, line: &HistoryLine, reset_all: bool) {
        if history_data::apply_remove(&self.db(), self.kind, remove_action(line, reset_all)).is_err() {
            self.toast.show(&tr(MsgKey::internal_error), NOT_FOUND_TOAST_MS);
        }
    }

    /// "Remove everything" for this kind.
    pub fn ask_clear(self: &Rc<Self>) {
        self.state.borrow_mut().open_dialog(HistoryDialog::ClearAll);
        let (confirm, close) = (Rc::downgrade(self), Rc::downgrade(self));
        history_dialog::show(
            ConfirmCopy {
                title: tr(MsgKey::action_remove_everything),
                text: tr(MsgKey::clear_history_confirmation),
                checkbox: None,
                confirm: tr(MsgKey::action_ok),
                cancel: tr(MsgKey::action_cancel),
            },
            true,
            move |_| {
                if let Some(screen) = confirm.upgrade() {
                    let cleared = HistoryRepo::new(screen.db()).clear_all(screen.kind);
                    screen.toast.show(&clear_outcome(cleared), CLEAR_DONE_TOAST_MS);
                }
            },
            move || {
                if let Some(screen) = close.upgrade() {
                    screen.state.borrow_mut().dismiss_dialog();
                }
            },
        );
    }
}

/// Opens a unit in the reader or the player. The external player is chosen by `pref_always_use_external_player`; the
/// app has no external launcher yet, so the internal player is the fallback, as the entry screen does.
fn open_unit(db: &std::sync::Arc<kuro_db::handle::Db>, kind: EntryKind, entry_id: i64, unit: &Unit) {
    let Some(title) = history_data::entry_title(db, kind, entry_id) else { return };
    let always = PlayerPrefs::new(data::app().base.store().clone()).always_use_external_player.get();
    if uses_external(always, kind) {
        tracing::debug!(entry_id, unit = unit.id, "no external player launcher; using the internal player");
    }
    crate::screens::open_media(kind == EntryKind::Anime, &title, &unit.name);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_external_player_applies_to_anime_only() {
        assert!(uses_external(true, EntryKind::Anime));
        assert!(!uses_external(true, EntryKind::Manga));
        assert!(!uses_external(false, EntryKind::Anime));
    }
}
