//! Mounts the History and Updates tabs (T-42a-021, T-42a-025): one screen per kind under an Anime | Manga switcher,
//! the header actions, and the reselect of the sidebar item. The screens are the live ones of [`history_screen`] and
//! [`updates_screen`]; the pages call [`history`] and [`updates`], and the sidebar calls [`reselect`].

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Orientation as O};
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};

use crate::screens::Built;
use crate::updates::history_screen::HistoryScreen;
use crate::updates::nav::{Reselect, reselect_action};
use crate::updates::updates_screen::UpdatesScreen;

/// The stack page names of the kind switcher; anime comes first.
const ANIME: &str = "anime";
const MANGA: &str = "manga";

thread_local! {
    static HISTORY: RefCell<Option<(gtk::Stack, Rc<HistoryScreen>, Rc<HistoryScreen>)>> = const { RefCell::new(None) };
    /// The Updates screens; their widgets only hold weak references, so this keeps them alive.
    static UPDATES: RefCell<Vec<Rc<UpdatesScreen>>> = const { RefCell::new(Vec::new()) };
}

fn kind_of(name: Option<&str>) -> EntryKind {
    if name == Some(MANGA) { EntryKind::Manga } else { EntryKind::Anime }
}

/// An Anime | Manga switcher over two pages.
fn kind_pages(anime: &gtk::Widget, manga: &gtk::Widget) -> (GBox, gtk::Stack) {
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_titled(anime, Some(ANIME), "Anime");
    stack.add_titled(manga, Some(MANGA), "Manga");
    stack.set_vexpand(true);
    let switcher = gtk::StackSwitcher::builder().stack(&stack).halign(Align::Center).margin_top(6).build();
    let page = GBox::new(O::Vertical, 6);
    page.append(&switcher);
    page.append(&stack);
    (page, stack)
}

/// The History page: both kinds, and the Clear history button for the visible kind. `None` before the app data is open.
pub fn history() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let (anime, manga) = (HistoryScreen::new(EntryKind::Anime), HistoryScreen::new(EntryKind::Manga));
    let (page, stack) = kind_pages(anime.widget().upcast_ref(), manga.widget().upcast_ref());
    let clear = crate::ui::icon_button("edit-clear-all-symbolic", &tr(MsgKey::pref_clear_history));
    let (clear_anime, clear_manga, visible) = (anime.clone(), manga.clone(), stack.clone());
    clear.connect_clicked(move |_| match kind_of(visible.visible_child_name().as_deref()) {
        EntryKind::Anime => clear_anime.ask_clear(),
        EntryKind::Manga => clear_manga.ask_clear(),
    });
    HISTORY.with(|h| *h.borrow_mut() = Some((stack, anime, manga)));
    Some(Built { content: page.upcast(), end: vec![clear.upcast()] })
}

/// The Updates page: both kinds under the spec toolbar. There is no search field. `None` before the app data is open.
pub fn updates() -> Option<Built> {
    if !crate::data::is_installed() {
        return None;
    }
    let (anime, manga) = (UpdatesScreen::new(EntryKind::Anime), UpdatesScreen::new(EntryKind::Manga));
    let (page, _) = kind_pages(anime.widget().upcast_ref(), manga.widget().upcast_ref());
    let mut end = crate::ui::toolbar(&crate::ui::screen("updates"));
    end.reverse();
    UPDATES.with(|u| *u.borrow_mut() = vec![anime, manga]);
    Some(Built { content: page.upcast(), end })
}

/// The sidebar item `nav_id` was chosen. `already_visible` says it was the showing page: History then resumes the
/// newest row of the visible sub-tab, and Updates opens the Downloads page.
pub fn reselect(nav_id: &str, already_visible: bool) {
    match reselect_action(nav_id, already_visible) {
        Reselect::ResumeHistory => {
            let screen = HISTORY.with(|h| {
                h.borrow().as_ref().map(|(stack, anime, manga)| match kind_of(stack.visible_child_name().as_deref()) {
                    EntryKind::Anime => anime.clone(),
                    EntryKind::Manga => manga.clone(),
                })
            });
            if let Some(screen) = screen {
                screen.resume_last();
            }
        }
        Reselect::OpenDownloads => crate::ui::open("download_queue"),
        Reselect::Nothing => {}
    }
}
