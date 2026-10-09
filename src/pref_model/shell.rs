//! The settings shell (T-47b-001 to T-47b-003): the page that `body("settings")` builds. It starts on the page the router
//! picks, lists the settings in one pane or two, and owns what the back action does while settings are on screen. Back is
//! one action: Escape, Alt+Left and the mouse back button all end in [`go_back`].

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use super::router::{self, Back, Destination, Page, Shell};
use crate::screens::Built;

thread_local! {
    static SHELL: RefCell<Shell> = RefCell::new(Shell::default());
    /// The widget name of the screen that is the shell's own page in the window's stack.
    static ROOT: RefCell<Option<String>> = const { RefCell::new(None) };
}

fn widget_name(screen_id: &str) -> String {
    format!("screen:{screen_id}")
}

/// The spec screen a router page shows.
pub fn page_screen_id(page: Page) -> &'static str {
    match page {
        Page::Main | Page::Inner => "settings",
        Page::Appearance => "settings_appearance",
        Page::Dest(d) => d.screen_id(),
    }
}

/// The destination whose screen is `id`, if it is one of the three other areas open by id.
pub fn destination_of(id: &str) -> Option<Destination> {
    [Destination::About, Destination::DataAndStorage, Destination::Tracking].into_iter().find(|d| d.screen_id() == id)
}

/// What a back action does with the window's stack.
#[derive(Debug, PartialEq, Eq)]
pub enum BackStep {
    /// The shell is not on screen; the caller does its own back.
    NotMine,
    /// Pop one page: an inner one, or the shell's own.
    Pop(Back),
}

/// Decides one back action. `names` is the window's stack from the root and `shown` the index of the page on show.
/// An inner page is any page above the shell's own, so the stack, not a counter, says how deep the user is.
pub fn plan_back(shell: &mut Shell, root: Option<&str>, names: &[String], shown: usize) -> BackStep {
    let Some(root) = root else { return BackStep::NotMine };
    let Some(at) = names.iter().rposition(|n| n == root) else {
        shell.close();
        return BackStep::NotMine;
    };
    if shown < at {
        return BackStep::NotMine;
    }
    shell.sync(shown - at);
    BackStep::Pop(shell.back())
}

/// Runs one back action for the settings shell. `false` means settings are not on screen and the caller pops as usual.
pub fn back() -> bool {
    let (names, shown) = crate::ui::page_stack();
    let step = ROOT.with(|root| SHELL.with(|shell| plan_back(&mut shell.borrow_mut(), root.borrow().as_deref(), &names, shown)));
    match step {
        BackStep::NotMine => false,
        BackStep::Pop(back) => {
            crate::ui::pop();
            if back == Back::Closed {
                ROOT.with(|root| *root.borrow_mut() = None);
            }
            true
        }
    }
}

/// The window's back action: the shell's when settings are on screen, else the plain pop.
pub fn go_back() {
    if !back() {
        crate::ui::pop();
    }
}

fn window_size() -> (i32, i32) {
    crate::window_size::size_of(&crate::ui::window())
}

/// Opens settings on `dest`, or on the default page when it is `None`. A destination shows only its own page, with no
/// main list underneath on a narrow window.
pub fn open(dest: Option<Destination>) {
    match dest {
        None => crate::ui::open("settings"),
        Some(d) => {
            SHELL.with(|s| s.borrow_mut().open(Some(d), two_pane_now()));
            ROOT.with(|r| *r.borrow_mut() = Some(widget_name(d.screen_id())));
            crate::ui::open(d.screen_id());
        }
    }
}

/// Opens settings on the destination an area asks for by id (`SettingsScreen.kt`). An unknown id opens the default page.
pub fn open_id(id: i64) {
    open(Destination::from_id(id));
}

fn two_pane_now() -> bool {
    let (width, height) = window_size();
    router::two_pane(width, height)
}

/// The body of the `settings` screen. A bare open lands on the main list (one pane) or Appearance beside the list
/// (two panes).
pub fn body() -> Option<Built> {
    let wide = two_pane_now();
    SHELL.with(|s| s.borrow_mut().open(None, wide));
    ROOT.with(|r| *r.borrow_mut() = Some(widget_name("settings")));
    let search = crate::ui::icon_button("system-search-symbolic", &tr(MsgKey::action_search_settings));
    search.connect_clicked(|_| crate::ui::open("settings_search"));
    let current = SHELL.with(|s| s.borrow().current()).unwrap_or(Page::Main);
    let content = if wide { two_pane(current) } else { main_list(Rc::new(|id| crate::ui::open(id))) };
    Some(Built { content, end: vec![search.upcast()] })
}

/// A resize across the tablet threshold: the shell's own page, when it is on top, is rebuilt for one pane or two.
fn rebuild_for_resize() {
    let (names, shown) = crate::ui::page_stack();
    let root = widget_name("settings");
    if names.get(shown) == Some(&root) && ROOT.with(|r| r.borrow().as_deref() == Some(root.as_str())) {
        crate::ui::pop();
        crate::ui::open("settings");
    }
}

/// Registers the resize handler once, at startup.
pub fn follow_resizes() {
    crate::window_size::on_tablet_change(|_| {
        rebuild_for_resize();
        true
    });
}

/// The settings list: one row per settings page. `on_select` gets the page's screen id.
fn main_list(on_select: Rc<dyn Fn(&str)>) -> gtk::Widget {
    let column = crate::ui::column();
    let list = crate::ui::boxed_list();
    for entry in crate::ui::with_spec(|s| s.settings_main.clone()) {
        let next = gtk::Image::from_icon_name("go-next-symbolic");
        let row = crate::ui::row(&entry.title, entry.subtitle.as_deref(), Some(next.upcast()));
        // the page's icon goes in front of its title
        if let Some(inner) = row.child().and_downcast::<gtk::Box>() {
            inner.prepend(&gtk::Image::from_icon_name(&entry.icon));
        }
        row.set_activatable(true);
        let (id, on_select) = (entry.id.clone(), on_select.clone());
        row.connect_activate(move |_| on_select(&id));
        list.append(&row);
    }
    column.append(&list);
    crate::ui::scrolled(&column).upcast()
}

/// The main list on the left and the open page on the right. Choosing a row replaces the right pane.
fn two_pane(current: Page) -> gtk::Widget {
    let right = gtk::Box::new(gtk::Orientation::Vertical, 0);
    right.set_hexpand(true);
    let show = {
        let right = right.clone();
        move |id: &str| {
            while let Some(child) = right.first_child() {
                right.remove(&child);
            }
            right.append(&pane(id));
        }
    };
    show(page_screen_id(current));
    let left = main_list(Rc::new(show));
    left.set_size_request(300, -1);
    let both = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    both.append(&left);
    both.append(&gtk::Separator::new(gtk::Orientation::Vertical));
    both.append(&right);
    both.upcast()
}

/// A settings page inside the right pane: its title and header actions on a bar, then its content.
fn pane(id: &str) -> gtk::Widget {
    let screen = crate::ui::screen(id);
    let (content, end): (gtk::Widget, Vec<gtk::Widget>) = match crate::screens::body(id) {
        Some(b) => (b.content, b.end),
        None => {
            let column = crate::ui::column();
            crate::ui::groups_into(&column, &screen.groups);
            (crate::ui::scrolled(&column).upcast(), crate::ui::toolbar(&screen))
        }
    };
    let bar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    crate::ui::margins(&bar, 6);
    bar.append(&gtk::Label::builder().label(&screen.title).xalign(0.0).hexpand(true).css_classes(["title-4"]).build());
    for widget in end {
        bar.append(&widget);
    }
    let page = gtk::Box::new(gtk::Orientation::Vertical, 0);
    page.append(&bar);
    content.set_vexpand(true);
    page.append(&content);
    page.upcast()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|n| (*n).to_owned()).collect()
    }

    fn opened(wide: bool) -> Shell {
        let mut shell = Shell::default();
        shell.open(None, wide);
        shell
    }

    #[test]
    fn t_47b_003_back_pops_the_inner_page_first_then_closes_the_shell() {
        let stack = names(&["screen:tabs", "screen:settings", "screen:settings_advanced", "screen:settings_debug_info"]);
        let mut shell = opened(false);
        assert_eq!(plan_back(&mut shell, Some("screen:settings"), &stack, 3), BackStep::Pop(Back::Popped));
        // the page above the shell is gone; the next back is the shell's own page
        assert_eq!(plan_back(&mut shell, Some("screen:settings"), &stack, 2), BackStep::Pop(Back::Popped));
        assert_eq!(plan_back(&mut shell, Some("screen:settings"), &stack, 1), BackStep::Pop(Back::Closed));
        assert!(!shell.is_open());
    }

    #[test]
    fn t_47b_003_back_is_not_the_shells_when_settings_are_not_on_screen() {
        let mut shell = opened(false);
        assert_eq!(plan_back(&mut shell, None, &names(&["screen:tabs"]), 0), BackStep::NotMine);
        let gone = names(&["screen:tabs", "screen:browse"]);
        assert_eq!(plan_back(&mut shell, Some("screen:settings"), &gone, 1), BackStep::NotMine);
        assert!(!shell.is_open(), "a shell whose page left the stack is closed");
        let mut shell = opened(false);
        let below = names(&["screen:tabs", "screen:settings"]);
        assert_eq!(plan_back(&mut shell, Some("screen:settings"), &below, 0), BackStep::NotMine, "a page below the shell is showing");
    }

    #[test]
    fn t_47b_001_a_destination_opens_alone_and_unknown_ids_open_the_default_page() {
        let mut shell = Shell::default();
        shell.open(Destination::from_id(0), false);
        assert_eq!(shell.current(), Some(Page::Dest(Destination::About)));
        assert_eq!(page_screen_id(Page::Dest(Destination::About)), "settings_about");
        shell.open(Destination::from_id(9), false);
        assert_eq!(shell.current(), Some(Page::Main), "an unknown id opens the main list on a narrow window");
        shell.open(Destination::from_id(-4), true);
        assert_eq!(shell.current(), Some(Page::Appearance), "and Appearance on a wide one");
        assert_eq!(page_screen_id(Page::Appearance), "settings_appearance");
        assert_eq!(page_screen_id(Page::Main), "settings");
    }

    #[test]
    fn t_47b_001_each_destination_is_found_by_its_screen_id() {
        assert_eq!(destination_of("settings_data_storage"), Some(Destination::DataAndStorage));
        assert_eq!(destination_of("settings_about"), Some(Destination::About));
        assert_eq!(destination_of("settings_player"), None);
    }

    #[test]
    fn t_47b_001_every_router_page_is_a_spec_screen() {
        crate::ui::with_spec(|spec| {
            for page in [Page::Main, Page::Appearance, Page::Inner] {
                assert!(spec.screen(page_screen_id(page)).is_some(), "{page:?}");
            }
        });
    }

    /// Needs a display; run alone: `cargo test -p kurobayomi pref_model::shell -- --ignored`.
    #[test]
    #[ignore = "needs a display; run alone: cargo test -p kurobayomi pref_model::shell -- --ignored"]
    fn t_47b_003_back_walks_the_real_window_stack() {
        if gtk::init().is_err() {
            eprintln!("skipped: no display for the shell check");
            return;
        }
        let app = gtk::Application::builder().application_id("io.github.blackbox.KurobayomiShellTest").flags(gtk::gio::ApplicationFlags::NON_UNIQUE).build();
        app.register(None::<&gtk::gio::Cancellable>).expect("register");
        let window = gtk::ApplicationWindow::new(&app);
        let stack = gtk::Stack::new();
        let tabs = gtk::Box::new(gtk::Orientation::Vertical, 0);
        tabs.set_widget_name("screen:tabs");
        stack.add_child(&tabs);
        crate::ui::nav_init(&stack, &window);

        assert!(!back(), "no shell yet: the caller pops as usual");
        let page = |name: &str| {
            let w = gtk::Box::new(gtk::Orientation::Vertical, 0);
            w.set_widget_name(&format!("screen:{name}"));
            crate::ui::push(&w);
            w
        };
        SHELL.with(|s| s.borrow_mut().open(None, false));
        ROOT.with(|r| *r.borrow_mut() = Some("screen:settings".to_owned()));
        let settings = page("settings");
        let inner = page("settings_advanced");
        assert!(back(), "an inner page is popped first");
        assert_eq!(stack.visible_child().as_ref(), Some(settings.upcast_ref()));
        assert!(SHELL.with(|s| s.borrow().is_open()), "the shell stays open under it");
        assert!(back(), "then the shell closes");
        assert_eq!(stack.visible_child().as_ref(), Some(tabs.upcast_ref()));
        assert!(!SHELL.with(|s| s.borrow().is_open()));
        assert!(!back(), "and the next back is the caller's");
        drop(inner);
    }
}
