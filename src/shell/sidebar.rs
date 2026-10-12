//! The left panel and its narrow-window twin, the bottom bar (docs/design/app-flow.md). The panel lists Library, Browse,
//! Updates, History and, while something is queued, Downloads with a filling circle and a count. Settings is pinned at
//! the bottom. The "Kurobayomi" title is the app menu (the GTK primary menu): Incognito mode, Keyboard shortcuts, Help
//! and About, with a small icon beside the title while Incognito mode is on. Clicking the item that is already shown only
//! scrolls its page to the top.

use std::cell::Cell;
use std::rc::Rc;
use std::time::Duration;

use gtk::prelude::*;
use gtk::{Box as GBox, Orientation as O, gio, glib};
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};

use crate::ui;
use crate::widgets::{download_button, rail};

/// The tab page of the download queue. Its item shows while something is queued, and while the page is open.
pub const DOWNLOADS: &str = "download_queue";

/// How often the Downloads item reads the queues.
const POLL: Duration = Duration::from_millis(500);

/// The side of the Downloads circle in px, the size of the symbolic icons beside it.
const RING: i32 = 16;

/// What the Downloads item shows for the percent of each queued unit: the count and the mean percent done, or `None`
/// when nothing is queued (the item then hides).
pub fn downloads_summary(percents: &[u8]) -> Option<(usize, u8)> {
    if percents.is_empty() {
        return None;
    }
    let sum: usize = percents.iter().map(|p| usize::from((*p).min(100))).sum();
    Some((percents.len(), (sum / percents.len()) as u8))
}

/// The percent done of every unit in both queues that is still going, and a 0 for each saved row waiting for its source
/// (the queue page lists those too); then how many units failed. A failed unit is not "downloading": it waits for a
/// retry on the Downloads page. Empty when no download manager runs.
fn queued_percents() -> (Vec<u8>, usize) {
    let Some(queues) = crate::downloads::live::queues() else { return (Vec::new(), 0) };
    let manga = queues.manga.snapshot();
    let mut units: Vec<(EntryKind, i64)> = manga.iter().map(|d| (EntryKind::Manga, d.chapter.id)).collect();
    if let Some(anime) = &queues.anime {
        units.extend(anime.snapshot().iter().map(|d| (EntryKind::Anime, d.episode.id)));
    }
    let states: Vec<_> = units.into_iter().map(|(kind, id)| crate::downloads::live::queued(&queues, kind, id)).collect();
    let failed = states.iter().filter(|s| matches!(s, Some((kuro_downloads::DownloadState::Error, _)))).count();
    let mut out: Vec<u8> = states
        .into_iter()
        .filter(|s| !matches!(s, Some((kuro_downloads::DownloadState::Error, _))))
        .map(|s| s.map_or(0, |(_, percent)| percent))
        .collect();
    // a waiting row that was queued again is already counted above
    let waiting = queues.waiting.read().unwrap_or_else(|e| e.into_inner()).iter().filter(|w| !manga.iter().any(|d| d.chapter.id == w.row.chapter_id)).count();
    out.resize(out.len() + waiting, 0);
    (out, failed)
}

/// One panel row: the icon, the title, then `extra` (badges).
fn nav_row(icon: &str, title: &str, extra: &[&gtk::Widget]) -> gtk::ListBoxRow {
    let h = GBox::new(O::Horizontal, 12);
    ui::margins(&h, rail::ROW_PAD);
    h.append(&gtk::Image::from_icon_name(icon));
    h.append(&gtk::Label::builder().label(title).xalign(0.0).hexpand(true).build());
    for w in extra {
        h.append(*w);
    }
    let row = gtk::ListBoxRow::builder().child(&h).build();
    row.set_margin_top(rail::ITEM_SPACING / 2);
    row.set_margin_bottom(rail::ITEM_SPACING / 2);
    row
}

/// The Updates count pill. It starts empty and follows the shared counts.
fn updates_badge() -> gtk::Label {
    let badge = gtk::Label::builder().css_classes(["kb-nav-badge", "numeric"]).valign(gtk::Align::Center).opacity(0.0).build();
    if crate::data::is_installed() {
        let store = crate::data::app().base.store().clone();
        crate::misc::badge::bind(&badge, &kuro_prefs::library::LibraryPrefs::new(store));
    }
    badge
}

/// The Downloads row: the download button's circle, filled with the mean progress, and the count of queued items. The
/// row and its tab page (so the bottom bar's button too) show while something is queued and while the page is open.
fn downloads_row(icon: &str, title: &str) -> gtk::ListBoxRow {
    let percent = Rc::new(Cell::new(0u8));
    let ring = gtk::DrawingArea::builder().content_width(RING).content_height(RING).valign(gtk::Align::Center).build();
    ring.set_draw_func({
        let percent = Rc::clone(&percent);
        move |area, cr, width, height| download_button::draw(area, cr, width, height, download_button::Look::Downloading(percent.get()))
    });
    let count = gtk::Label::builder().css_classes(["kb-nav-badge", "numeric"]).valign(gtk::Align::Center).build();
    // failed downloads: a warning sign with their number, instead of a ring that would never fill
    let warning = gtk::Image::builder().icon_name("dialog-warning-symbolic").css_classes(["warning"]).valign(gtk::Align::Center).build();
    let row = nav_row(icon, title, &[ring.upcast_ref(), warning.upcast_ref(), count.upcast_ref()]);
    let weak = row.downgrade();
    let tick = move || {
        if weak.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        // the item is always there (user, 2026-10-12); only its circle and count follow the queue
        let (percents, failed) = queued_percents();
        let (queued, done) = downloads_summary(&percents).unwrap_or((0, 0));
        ring.set_visible(queued > 0);
        warning.set_visible(queued == 0 && failed > 0);
        warning.set_tooltip_text(Some(&format!("{failed} failed")));
        count.set_visible(queued > 0 || failed > 0);
        count.set_label(&(if queued > 0 { queued } else { failed }).to_string());
        if percent.replace(done) != done {
            ring.queue_draw();
        }
        glib::ControlFlow::Continue
    };
    let _ = tick();
    glib::timeout_add_local(POLL, tick);
    row
}

/// Scrolls every scrolled view under `root` back to its top.
fn scroll_to_top(root: &gtk::Widget) {
    if let Some(view) = root.downcast_ref::<gtk::ScrolledWindow>() {
        let adjustment = view.vadjustment();
        adjustment.set_value(adjustment.lower());
    }
    let mut child = root.first_child();
    while let Some(w) = child {
        scroll_to_top(&w);
        child = w.next_sibling();
    }
}

/// A click on the panel item `id`: when its page is already on show (nothing pushed over it), the page scrolls to the
/// top; otherwise the pushed pages go and the item's page shows.
fn select(tabs: &gtk::Stack, id: &str) {
    if ui::page_stack().1 == 0 && tabs.visible_child_name().as_deref() == Some(id) {
        if let Some(page) = tabs.visible_child() {
            scroll_to_top(&page);
        }
        return;
    }
    ui::pop_to_root();
    tabs.set_visible_child_name(id);
}

fn open_settings() {
    ui::pop_to_root();
    crate::pref_model::shell::open(None);
}

fn open_about() {
    ui::pop_to_root();
    crate::pref_model::shell::open(Some(crate::pref_model::router::Destination::About));
}

fn open_help() {
    crate::pref_model::stores_page::open_url(crate::pref_model::widgets::HELP_URL);
}

/// A menu button with the app menu: Incognito mode (the `app.incognito` check item), then Keyboard shortcuts, Help and
/// About, whose actions (`appmenu.*`) live on the button itself.
fn app_menu() -> gtk::MenuButton {
    let toggles = gio::Menu::new();
    toggles.append(Some(&tr(MsgKey::pref_incognito_mode)), Some(crate::incognito::TOGGLE_ACTION));
    let pages = gio::Menu::new();
    let actions = gio::SimpleActionGroup::new();
    let entries: [(String, &str, fn()); 3] = [
        ("Keyboard shortcuts".to_owned(), "shortcuts", crate::entry_core::shortcut_help_view::open),
        (tr(MsgKey::label_help), "help", open_help),
        (tr(MsgKey::pref_category_about), "about", open_about),
    ];
    for (label, name, run) in entries {
        pages.append(Some(&label), Some(&format!("appmenu.{name}")));
        let action = gio::SimpleAction::new(name, None);
        action.connect_activate(move |_, _| run());
        actions.add_action(&action);
    }
    let model = gio::Menu::new();
    model.append_section(None, &toggles);
    model.append_section(None, &pages);
    let button = ui::quiet_menu(gtk::MenuButton::builder().menu_model(&model).build());
    button.insert_action_group("appmenu", Some(&actions));
    button
}

/// The small icon beside the panel title while Incognito mode is on. It follows the state of `app.incognito`.
fn incognito_icon() -> gtk::Image {
    let icon = gtk::Image::builder().icon_name(crate::incognito::ICON).tooltip_text(tr(MsgKey::pref_incognito_mode)).visible(false).build();
    let Some(action) = gio::Application::default().and_then(|app| app.lookup_action(crate::incognito::TOGGLE_NAME)) else { return icon };
    let follow = |icon: &gtk::Image, action: &gio::Action| icon.set_visible(action.state().and_then(|s| s.get::<bool>()).unwrap_or(false));
    follow(&icon, &action);
    let weak = icon.downgrade();
    action.connect_state_notify(move |action| {
        if let Some(icon) = weak.upgrade() {
            follow(&icon, action);
        }
    });
    icon
}

/// The panel title: the app's name, with the incognito icon beside it while Incognito mode is on. Not a menu: the app's
/// own items (Incognito, Keyboard shortcuts, Help, About) are rows at the bottom of the panel.
fn title() -> GBox {
    let title = GBox::new(O::Horizontal, 6);
    title.append(&gtk::Label::builder().label("Kurobayomi").css_classes(["title"]).build());
    title.append(&incognito_icon());
    title
}

/// The rows at the bottom of the panel: Incognito mode with its switch, Keyboard shortcuts, Help, About and Settings.
fn bottom_rows() -> gtk::ListBox {
    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_selection_mode(gtk::SelectionMode::None);
    list.set_margin_top(rail::PAD_V);
    list.set_margin_bottom(rail::PAD_V);
    // the switch is bound to app.incognito, so it shows the state and flips it like the menu item did
    let switch = gtk::Switch::builder().valign(gtk::Align::Center).action_name(crate::incognito::TOGGLE_ACTION).build();
    let incognito = nav_row(crate::incognito::ICON, &tr(MsgKey::pref_incognito_mode), &[switch.upcast_ref()]);
    list.append(&incognito);
    let rows: [(&str, String, fn()); 4] = [
        ("input-keyboard-symbolic", "Keyboard shortcuts".to_owned(), crate::entry_core::shortcut_help_view::open),
        ("help-browser-symbolic", tr(MsgKey::label_help), open_help),
        ("help-about-symbolic", tr(MsgKey::pref_category_about), open_about),
        ("emblem-system-symbolic", tr(MsgKey::label_settings), open_settings),
    ];
    let runs: Vec<fn()> = rows.iter().map(|(_, _, run)| *run).collect();
    for (icon, label, _) in rows {
        list.append(&nav_row(icon, &label, &[]));
    }
    list.connect_row_activated(move |_, row| {
        match usize::try_from(row.index()).ok() {
            // a click anywhere on the Incognito row flips its switch
            Some(0) => {
                switch.activate();
            }
            Some(i) => {
                if let Some(run) = runs.get(i - 1) {
                    run();
                }
            }
            None => {}
        }
    });
    list
}

/// The left panel: the app menu in a split header, one row per place from the top, and Settings at the bottom. Always
/// full width; there is no collapsed mode (docs/design/main-window.md).
pub fn build(tabs: &gtk::Stack) -> GBox {
    let header = ui::header_bar();
    header.set_decoration_layout(Some(&ui::decoration(true)));
    header.set_title_widget(Some(&title()));
    ui::no_sticky_focus(&header);

    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_valign(gtk::Align::Start);
    list.set_margin_top(rail::PAD_V);
    list.set_margin_bottom(rail::PAD_V);
    let ids: Vec<String> = ui::with_spec(|s| s.nav.iter().map(|n| n.id.clone()).collect());
    for n in ui::with_spec(|s| s.nav.clone()) {
        let row = match n.id.as_str() {
            "updates" => nav_row(&n.icon, &n.title, &[updates_badge().upcast_ref()]),
            DOWNLOADS => downloads_row(&n.icon, &n.title),
            _ => nav_row(&n.icon, &n.title, &[]),
        };
        list.append(&row);
    }
    {
        let (tabs, ids) = (tabs.clone(), ids.clone());
        list.connect_row_activated(move |_, r| {
            if let Some(id) = usize::try_from(r.index()).ok().and_then(|i| ids.get(i)) {
                select(&tabs, id);
            }
        });
    }
    // keep the selection in sync when the bottom bar (narrow mode) or a link switches tabs
    let l2 = list.clone();
    tabs.connect_visible_child_name_notify(move |t| {
        let i = t.visible_child_name().and_then(|n| ids.iter().position(|x| *x == n));
        if let Some(r) = i.and_then(|i| l2.row_at_index(i as i32)) {
            l2.select_row(Some(&r));
        }
    });
    list.select_row(list.row_at_index(0).as_ref());

    let settings = bottom_rows();

    let b = GBox::new(O::Vertical, 0);
    b.set_size_request(rail::WIDTH, -1);
    b.add_css_class("kb-sidebar");
    b.append(&header);
    b.append(&gtk::ScrolledWindow::builder().child(&list).hscrollbar_policy(gtk::PolicyType::Never).vexpand(true).build());
    b.append(&gtk::Separator::new(O::Horizontal));
    b.append(&settings);
    b
}

/// The narrow window's bar: the same items as the panel. The places switch the tab pages, then Settings and the app menu.
pub fn bottom_bar(tabs: &gtk::Stack) -> GBox {
    let bar = GBox::new(O::Horizontal, 6);
    bar.set_halign(gtk::Align::Center);
    bar.set_margin_top(6);
    bar.set_margin_bottom(6);
    bar.append(&gtk::StackSwitcher::builder().stack(tabs).build());
    let settings = ui::icon_button("emblem-system-symbolic", &tr(MsgKey::label_settings));
    settings.add_css_class("flat");
    settings.connect_clicked(|_| open_settings());
    bar.append(&settings);
    let menu = app_menu();
    menu.set_icon_name("open-menu-symbolic");
    menu.set_tooltip_text(Some(&tr(MsgKey::action_menu)));
    bar.append(&menu);
    ui::no_sticky_focus(&bar);
    bar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels(root: &gtk::Widget) -> Vec<String> {
        crate::screens::alone::descendants(root).iter().filter_map(|w| w.downcast_ref::<gtk::Label>().map(|l| l.label().to_string())).collect()
    }

    #[test]
    fn the_downloads_item_counts_the_queued_units_and_averages_their_progress() {
        assert_eq!(downloads_summary(&[]), None, "nothing queued hides the item");
        assert_eq!(downloads_summary(&[0]), Some((1, 0)), "a queued unit shows the empty circle");
        assert_eq!(downloads_summary(&[100, 0, 50, 50]), Some((4, 50)));
        assert_eq!(downloads_summary(&[250, 100]), Some((2, 100)), "a percent never counts past 100");
    }

    #[test]
    fn scrolling_to_the_top_reaches_every_scrolled_view_below() {
        crate::testing::with_display(|| {
            let adjustment = gtk::Adjustment::new(500.0, 0.0, 1000.0, 1.0, 10.0, 10.0);
            let view = gtk::ScrolledWindow::builder().vadjustment(&adjustment).build();
            let page = GBox::new(O::Vertical, 0);
            page.append(&view);
            scroll_to_top(page.upcast_ref());
            assert_eq!(adjustment.value(), 0.0);
        });
    }

    #[test]
    fn the_panel_has_the_places_and_the_app_rows_at_the_bottom_and_no_menu() {
        crate::testing::with_display(|| {
            let panel = build(&gtk::Stack::new());
            let texts = labels(panel.upcast_ref());
            let wanted = [tr(MsgKey::pref_incognito_mode), "Keyboard shortcuts".to_owned(), tr(MsgKey::label_help), tr(MsgKey::pref_category_about)];
            for title in ["Library", "Browse", "Updates", "History", "Downloads", "Settings", "Kurobayomi"].map(str::to_owned).iter().chain(&wanted) {
                assert!(texts.iter().any(|l| l == title), "{title}");
            }
            assert!(!texts.iter().any(|l| l == "More"), "More is gone");
            let all = crate::screens::alone::descendants(panel.upcast_ref());
            assert!(!all.iter().any(|w| w.is::<gtk::MenuButton>()), "the title is plain, not a drop-down");
            assert!(all.iter().any(|w| w.downcast_ref::<gtk::Switch>().is_some_and(|s| s.action_name().as_deref() == Some(crate::incognito::TOGGLE_ACTION))), "Incognito has a switch");
            let downloads = all
                .iter()
                .filter_map(|w| w.downcast_ref::<gtk::ListBoxRow>())
                .find(|r| labels(r.upcast_ref()).iter().any(|l| l == "Downloads"))
                .expect("the Downloads row");
            assert!(downloads.is_visible(), "Downloads is always in the panel, also with an empty queue");
        });
    }

    #[test]
    fn the_bottom_bar_has_the_places_settings_and_the_app_menu() {
        crate::testing::with_display(|| {
            let bar = bottom_bar(&gtk::Stack::new());
            let all = crate::screens::alone::descendants(bar.upcast_ref());
            assert_eq!(all.iter().filter(|w| w.is::<gtk::StackSwitcher>()).count(), 1);
            assert!(all.iter().any(|w| w.downcast_ref::<gtk::Button>().is_some_and(|b| b.icon_name().as_deref() == Some("emblem-system-symbolic"))), "Settings");
            assert_eq!(all.iter().filter(|w| w.is::<gtk::MenuButton>()).count(), 1, "the app menu");
        });
    }
}
