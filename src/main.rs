mod applock_host;
mod app_update;
mod backup_host;
mod backup_run;
mod backup_schedule;
mod bind;
mod clamp;
mod cli;
// the thumbnail renderer is called by the cover loader, which does not exist yet
#[allow(dead_code)]
mod feed;
// No WebView host shows the window yet (T-12a-017 needs WebKitGTK).
#[allow(dead_code)]
mod cloudflare_window;
mod data;
mod entry;
#[allow(dead_code)]
mod entry_core;
mod entry_model;
mod ext_host;
mod library;
mod library_update;
mod misc;
// no screen calls the client yet; the extension host and the trackers take it from here
#[allow(dead_code)]
mod net_client;
mod local_flow;
mod local_flow_anime;
mod live;
mod gio_notifier;
mod incognito;
mod locale;
mod open_link;
mod player;
#[allow(dead_code)]
mod player_view;
mod player_scripts;
mod reader;
mod reporter;
mod screens;
mod selection;
mod settings;
mod settings_ui;
mod source_bridge;
mod source_gate;
mod toast;
mod tri_check;
mod spec;
mod startup;
mod theme;
mod widgets;
mod ui;
mod watch;
mod window_size;
#[cfg(test)]
mod testing;
mod updates;
mod pref_model;
mod browse;
mod shell;

use gtk::prelude::*;
use kuro_platform::paths::Paths as _;
use gtk::{glib, Box as GBox, Orientation as O};

fn main() -> glib::ExitCode {
    // before GTK is initialised, so this works without a display
    if std::env::args().skip(1).any(|a| a == "--version") {
        println!("{}", kuro_core::app_info::version_line());
        return glib::ExitCode::SUCCESS;
    }
    // `--updates-json`: the desktop applets read the updates feed here, before any GTK call (works without a display)
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if cli::wants_updates_json(&args) {
            return glib::ExitCode::from(cli::run(&args));
        }
    }
    // `--run-backup`: one automatic backup from the systemd timer, no window and no GTK
    if std::env::args().skip(1).any(|a| a == "--run-backup") {
        return glib::ExitCode::from(backup_run::run_backup_cli());
    }
    // the detached crash reporter started by the panic hook: a window over the report file, none of the app state below
    let mut args = std::env::args().skip(1);
    if args.any(|a| a == "--crash-report") {
        return reporter::run(args.next().unwrap_or_default().into());
    }
    // the command line is checked before GTK starts, so `--help` and a bad option work without a display; a second
    // launch hands the same arguments to the running instance through GApplication
    let args: Vec<String> = std::env::args().skip(1).collect();
    if shell::argv::wants_help(&args) {
        print!("{}", shell::argv::USAGE);
        return glib::ExitCode::SUCCESS;
    }
    if let Err(e) = shell::argv::parse_launch(&args) {
        eprintln!("kurobayomi: {e}");
        return glib::ExitCode::from(kuro_platform::deep_link::USAGE_EXIT_CODE as u8);
    }
    let paths = kuro_platform::linux::LinuxPaths::system().expect("resolve app directories");
    // the hook goes in first so a panic during the rest of startup is reported too
    kuro_core::crash::install(paths.crash_dir());
    let _log = match kuro_core::logging::init(cfg!(debug_assertions), &paths.log_dir()) {
        Ok(log) => Some(settings_ui::advanced::share_logging(log)),
        Err(e) => {
            eprintln!("kurobayomi: file logging is off: {e}");
            None
        }
    };
    if let Err(e) = kuro_core::crash::prune(&paths.crash_dir(), kuro_core::crash::now_ms()) {
        eprintln!("kurobayomi: could not prune crash reports: {e}");
    }
    let rt = kuro_core::runtime::build().expect("tokio runtime");
    // the one runtime, reachable from the shell for the whole run
    let _shell = shell::Shell::install(rt.handle().clone(), &paths);

    // a second launch is forwarded to the running instance by GApplication; dev variables run a window of their own
    let unique = kuro_platform::single_instance::is_unique(|v| std::env::var(v).is_ok());
    let app = gtk::Application::builder().application_id("io.github.blackbox.Kurobayomi").flags(shell::launch::app_flags(unique)).build();
    shell::launch::connect(&app);
    // the preferences get up to 2 s to reach the disk when the application shuts down (T-02a-038)
    app.connect_shutdown(|_| {
        if data::is_installed() {
            data::flush_on_shutdown(data::app().base.store());
        }
    });
    app.connect_activate(|app| {
        // a later launch only raises the window; the first one builds it
        if let Some(win) = app.windows().first() {
            win.present();
            return;
        }
        build_ui(app);
    });
    // live queries of the entry screens are spawned on the runtime, so the GTK thread enters it while the app runs
    let code = {
        let _entered = rt.enter();
        app.run()
    };
    // detached history flushes get up to 2 s to finish before the runtime goes away
    let unfinished = kuro_domain::history::repo::join_pending_flushes(std::time::Duration::from_secs(2));
    if unfinished > 0 {
        eprintln!("kurobayomi: {unfinished} history flush(es) were still running at exit");
    }
    // WAL checkpoint and PRAGMA optimize on both writers (T-03a-028)
    if data::is_installed() {
        let app = data::app();
        for (name, db) in [("manga", &app.manga_db), ("anime", &app.anime_db)] {
            if let Err(e) = db.close() {
                eprintln!("kurobayomi: closing the {name} database failed: {e}");
            }
        }
    }
    kuro_core::runtime::shutdown(rt);
    code
}

thread_local! {
    /// True while the databases open, so a second activation does not start a second open.
    static OPENING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Opens the data off the GTK thread (a splash shows if that takes over 300 ms), then builds and presents the window.
fn build_ui(app: &gtk::Application) {
    if data::is_installed() {
        build_window(app);
        return;
    }
    if OPENING.with(|o| o.replace(true)) {
        return;
    }
    // the window does not exist yet, so the application is held until the data is open
    let hold = app.hold();
    let (shown_app, app) = (app.clone(), app.clone());
    startup::run_with_splash(
        data::open_blocking,
        startup::SPLASH_DELAY,
        move || startup::splash_window(&shown_app),
        move |opened, splash| {
            match opened.and_then(data::install) {
                Ok(()) => build_window(&app),
                Err(e) => {
                    eprintln!("kurobayomi: cannot open the library databases: {e}");
                    app.quit();
                }
            }
            // the splash goes after the main window is presented, so focus passes straight to the main window
            if let Some(splash) = splash {
                splash.destroy();
            }
            OPENING.with(|o| o.set(false));
            drop(hold);
        },
    );
}

fn build_window(app: &gtk::Application) {
    // the theme is applied before the first widget is built (T-36a-061)
    shell::Shell::build(theme::startup).build_widget().expect("the theme was applied by Shell::build");
    let win = gtk::ApplicationWindow::builder()
        .application(app)
        .title("Kurobayomi")
        .default_width(1180)
        .default_height(780)
        .build();
    // each page brings its own header bar, so the window titlebar stays empty
    win.set_titlebar(Some(&GBox::new(O::Horizontal, 0)));

    // [sidebar | content nav stack], plus a bottom tab bar that replaces the sidebar when narrow
    let nav = gtk::Stack::new();
    nav.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    nav.set_vexpand(true);
    nav.set_hexpand(true);
    nav.set_hhomogeneous(false);
    ui::nav_init(&nav, &win);
    let tabs = tabs();
    nav.add_child(&tabs);
    let sidebar = sidebar(&tabs);
    let prefs = settings_ui::route::prefs();
    settings_ui::appearance::open_start_tab(&tabs, &prefs);
    settings_ui::appearance::apply_theme_mode(prefs.ui.theme_mode.get());
    settings_ui::appearance::apply_saved_pure_black(prefs.ui.theme_dark_amoled.get(), prefs.ui.theme_mode.get());
    // one selection for every layout: the sidebar and the bottom bar show the same tab (T-36a-004)
    let host = shell::Shell::get().expect("the shell is installed in main");
    let home = host.home_tabs(prefs.ui.start_screen.get(), prefs.ui.nav_style.get());
    shell::tabs::bind(&tabs, &home);
    let sep = gtk::Separator::new(O::Vertical);
    let row = GBox::new(O::Horizontal, 0);
    row.append(&sidebar);
    row.append(&sep);
    row.append(&nav);
    let bottom = gtk::StackSwitcher::builder().stack(&tabs).halign(gtk::Align::Center).margin_top(6).margin_bottom(6).build();
    let outer = GBox::new(O::Vertical, 0);
    row.set_vexpand(true);
    outer.append(&row);
    outer.append(&bottom);
    // the toast layer sits over the whole window (T-12a-014)
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&outer));
    let _toaster = toast::attach(&overlay);
    win.set_child(Some(&overlay));
    // the app-state banners above everything: indexing, downloaded only, incognito (T-36a-018). The incognito module
    // keeps the action, the notification and the More switch; its own banner is not placed.
    let base = &data::app().base;
    let _ = incognito::attach(app, base.incognito_mode.clone(), base.downloaded_only.clone());
    let banners = std::rc::Rc::new(shell::banners::Banners::new());
    outer.prepend(&banners.widget);
    {
        let (index, downloaded_only, incognito) = (host.downloads().clone(), base.downloaded_only.clone(), base.incognito_mode.clone());
        shell::banners::watch(banners, move || shell::banners::BannerState {
            indexing: index.checking_indicator_visible(),
            downloaded_only: downloaded_only.get(),
            incognito: incognito.get(),
        });
    }
    // the extension host: package scans, icons, load problems and the first-run languages
    ext_host::start(base.store(), data::app().anime_db.clone());
    // the library update runner behind the refresh button and the automatic schedule
    misc::start();
    // the restore banner, the Create and Restore hosts, and the close dialog of a running restore
    outer.prepend(&backup_host::attach(app, &win));
    // the automatic backup schedule follows the interval setting (T-02a-024)
    backup_schedule::start(app);
    open_link::register(app, &win);

    // reader/player are immersive: no sidebar or tab bar while they're on screen
    let refresh = {
        let (win, nav, tabs, home, mode) = (win.clone(), nav.clone(), tabs.clone(), home.clone(), prefs.ui.tablet_ui_mode.clone());
        move || {
            let immersive = nav.visible_child().is_some_and(|c| matches!(c.widget_name().as_str(), "screen:reader" | "screen:player" | shell::onboarding::PAGE_NAME));
            let (width, height) = window_size::size_of(&win);
            let chrome = shell::layout::chrome_for(mode.get(), width, height, win.is_maximized());
            // the bottom bar belongs to the tab pages, and it hides on the tab that moved into More
            let on_tabs = nav.visible_child().as_ref() == Some(tabs.upcast_ref());
            let bar_allowed = on_tabs && shell::tabs::bar_visible(home.borrow().current(), home.borrow().more_tab(), true);
            let shown = shell::layout::visibility(chrome, immersive, bar_allowed);
            // the sidebar also hides while library entries are selected (B-1.4)
            let selecting = library::mount::selecting();
            sidebar.set_visible(shown.sidebar && !selecting);
            sep.set_visible(shown.sidebar && !selecting);
            bottom.set_visible(shown.bottom_bar);
        }
    };
    let r = refresh.clone();
    nav.connect_visible_child_notify(move |_| r());
    let r = refresh.clone();
    tabs.connect_visible_child_name_notify(move |_| r());
    let r = refresh.clone();
    pref_model::bind::follow(&prefs.ui.tablet_ui_mode, || true, move |_| r());
    let r = refresh.clone();
    win.connect_default_width_notify(move |_| r());
    let r = refresh.clone();
    win.connect_default_height_notify(move |_| r());
    let r = refresh.clone();
    win.connect_maximized_notify(move |_| r());
    // the entry header and the settings shell follow the same breakpoint
    pref_model::shell::follow_resizes();
    window_size::watch(&win);
    theme::attach_window(&win);
    local_flow::ui::attach(&win);
    library::mount::on_selection_change(refresh.clone());
    refresh();
    applock_host::attach(&win, data::app().security.clone());
    win.present();
    schedule_crash_test();
    startup::publish_launcher_badge(&win, &app.application_id().unwrap_or_default());
    // background work that waits for the window: the chapter cache clear and the opt-in release check
    if let Ok(paths) = kuro_platform::linux::LinuxPaths::system() {
        startup::clear_chapter_cache_on_launch(data::app().library.auto_clear_item_cache.get(), kuro_images::platform::chapter_cache_dir(&paths));
    }
    app_update::startup(&data::app().base);
    // requests that arrived before the window: --action, --search, --open and opened files
    shell::launch::window_built(app, &win);
    // the first launch starts the onboarding wizard (T-36a-024)
    shell::onboarding::show_if_needed(&win, base.store(), !kuro_platform::single_instance::is_unique(|v| std::env::var(v).is_ok()));
    // no keyboard focus ring on the first header button at startup
    GtkWindowExt::set_focus(&win, None::<&gtk::Widget>);

    // Escape / Alt+Left goes back, like Aniyomi's back gesture. On the root page, back returns to the start tab.
    let go_back: std::rc::Rc<dyn Fn()> = {
        let (tabs, home) = (tabs.clone(), home.clone());
        std::rc::Rc::new(move || {
            let (_, shown) = ui::page_stack();
            if shown == 0 && shell::tabs::go_back(&tabs, &home) {
                return;
            }
            pref_model::shell::go_back();
        })
    };
    let keys = gtk::EventControllerKey::new();
    let back_key = go_back.clone();
    keys.connect_key_pressed(move |_, key, _, state| {
        let back = key == gtk::gdk::Key::Escape || (key == gtk::gdk::Key::Left && state.contains(gtk::gdk::ModifierType::ALT_MASK));
        if back {
            back_key();
        }
        if back { glib::Propagation::Stop } else { glib::Propagation::Proceed }
    });
    win.add_controller(keys);
    // the mouse back button (button 8) is the same action
    let mouse_back = gtk::GestureClick::builder().button(8).build();
    mouse_back.connect_pressed(move |_, _, _, _| go_back());
    win.add_controller(mouse_back);

    shell::layout::apply_minimum_size(&win);
    // dev hooks: KUROBAYOMI_OPEN=<screen id> starts on that screen; KUROBAYOMI_SMOKE=1 opens every
    // screen, sheet and dialog once and quits (a panic or GTK critical shows up in the output)
    if let Some((w, h)) = std::env::var("KUROBAYOMI_SIZE").ok().and_then(|v| v.split_once('x').map(|(w, h)| (w.parse().unwrap_or(1180), h.parse().unwrap_or(780)))) {
        win.set_default_size(w, h);
    }
    if let Ok(id) = std::env::var("KUROBAYOMI_OPEN") {
        ui::open(&id);
        if let Some(sh) = std::env::var("KUROBAYOMI_SHEET").ok().and_then(|n| ui::screen(&id).sheets.into_iter().find(|s| s.id == n)) {
            ui::show_sheet(&sh);
        }
    }
    // KUROBAYOMI_SHOT=file.png renders the window to a PNG and quits (no focus-dependent screenshots)
    if let Ok(path) = std::env::var("KUROBAYOMI_SHOT") {
        let (w, app) = (win.clone(), app.clone());
        glib::timeout_add_local_once(std::time::Duration::from_millis(1200), move || {
            let snap = gtk::Snapshot::new();
            gtk::WidgetPaintable::new(Some(&w)).snapshot(&snap, w.width() as f64, w.height() as f64);
            if let (Some(node), Some(r)) = (snap.to_node(), w.renderer()) {
                r.render_texture(&node, None).save_to_png(&path).expect("save png");
            }
            app.quit();
        });
    }
    if std::env::var("KUROBAYOMI_SMOKE").is_ok() {
        let mut ids: Vec<String> = ui::with_spec(|s| s.screens.keys().cloned().collect());
        ids.sort();
        let (mut sheets, mut dialogs) = (0, 0);
        for id in &ids {
            ui::open(id);
            let s = ui::screen(id);
            for sh in &s.sheets {
                ui::show_sheet(sh);
                sheets += 1;
            }
            for d in &s.dialogs {
                ui::show_dialog(d);
                dialogs += 1;
            }
            for w in gtk::Window::list_toplevels() {
                if let Ok(w) = w.downcast::<gtk::Window>() {
                    if w != *win.upcast_ref::<gtk::Window>() {
                        w.destroy();
                    }
                }
            }
        }
        screens::open_entry("Title 1", true);
        screens::open_entry("Title 1", false);
        println!("smoke: {} screens, {sheets} sheets, {dialogs} dialogs opened", ids.len());
        app.quit();
    }
}

/// The crash-reporter test hooks (T-01a-048): `KUROBAYOMI_CRASH_TEST=1` panics on the main thread 500 ms after the
/// window is presented, `=worker` panics inside a tokio task. Release builds ignore both unless
/// `KUROBAYOMI_ALLOW_TEST_HOOKS=1`.
fn schedule_crash_test() {
    use kuro_core::crash::{self, CrashTest};
    let env = |name: &str| std::env::var(name).ok();
    match crash::crash_test_hook(env(crash::CRASH_TEST_ENV).as_deref(), env(crash::ALLOW_TEST_HOOKS_ENV).as_deref(), crash::RELEASE_BUILD) {
        Some(CrashTest::MainWindow) => {
            glib::timeout_add_local_once(std::time::Duration::from_millis(crash::CRASH_TEST_DELAY_MS), || crash::crash_test_now());
        }
        // the GTK thread is inside the runtime while the app runs, see `main`
        Some(CrashTest::Worker) => {
            tokio::spawn(crash::crash_test_worker());
        }
        None => {}
    }
}

/// One page per Aniyomi nav destination, each with its own header (title + that tab's actions).
fn tabs() -> gtk::Stack {
    let tabs = gtk::Stack::new();
    tabs.set_transition_type(gtk::StackTransitionType::Crossfade);
    tabs.set_hhomogeneous(false);
    for n in ui::with_spec(|s| s.nav.clone()) {
        let Some(b) = screens::body(&n.id) else { continue };
        let title = if n.id == "library" { n.title.clone() } else { ui::screen(&n.id).title };
        tabs.add_titled(&ui::page(&title, &b.content, b.end, false), Some(&n.id), &n.title).set_icon_name(&n.icon);
    }
    tabs
}

/// GNOME-style navigation sidebar with Aniyomi's badges (new updates, extension updates).
fn sidebar(tabs: &gtk::Stack) -> GBox {
    let header = gtk::HeaderBar::new();
    header.set_decoration_layout(Some(&ui::decoration(true)));
    let title = gtk::Label::builder().label("Kurobayomi").css_classes(["title"]).build();
    header.set_title_widget(Some(&title));
    let toggle = gtk::ToggleButton::builder().icon_name("sidebar-show-symbolic").tooltip_text("Collapse sidebar").build();
    header.pack_start(&toggle);
    ui::no_sticky_focus(&header);

    let list = gtk::ListBox::new();
    list.add_css_class("navigation-sidebar");
    list.set_valign(gtk::Align::Center);
    list.set_margin_top(widgets::rail::PAD_V);
    list.set_margin_bottom(widgets::rail::PAD_V);
    let ids: Vec<String> = ui::with_spec(|s| s.nav.iter().map(|n| n.id.clone()).collect());
    // widgets that hide in icon-only mode, and the dots that replace badges there
    let (mut expanded, mut dots, mut rows): (Vec<gtk::Widget>, Vec<gtk::Widget>, Vec<GBox>) = (vec![], vec![], vec![]);
    let mut squares: Vec<(gtk::Image, gtk::ListBoxRow)> = vec![]; // bigger icon + square selection in icon-only mode
    for n in ui::with_spec(|s| s.nav.clone()) {
        let h = GBox::new(O::Horizontal, 8);
        h.set_margin_start(6);
        h.set_margin_end(6);
        h.set_margin_top(6);
        h.set_margin_bottom(6);
        // both badges start empty and are bound to the shared counts below; no count is written here
        let badge = if matches!(n.id.as_str(), "updates" | "browse") { " " } else { "" };
        let icon = gtk::Overlay::new();
        let img = gtk::Image::from_icon_name(&n.icon);
        icon.set_child(Some(&img));
        h.append(&icon);
        let label = gtk::Label::builder().label(&n.title).xalign(0.0).hexpand(true).build();
        h.append(&label);
        expanded.push(label.upcast());
        if !badge.is_empty() {
            let b = gtk::Label::builder().label(badge).css_classes(["accent", "caption-heading"]).width_chars(3).xalign(1.0).build();
            h.append(&b);
            b.set_label("");
            let updates_label = (n.id == "updates").then(|| b.clone());
            let browse_label = (n.id == "browse").then(|| b.clone());
            expanded.push(b.upcast());
            let dot = gtk::Label::builder().use_markup(true).label("<span size=\"5pt\">●</span>").css_classes(["accent"]).halign(gtk::Align::End).valign(gtk::Align::Start).visible(false).build();
            icon.add_overlay(&dot);
            if let Some(label) = updates_label.as_ref().filter(|_| data::is_installed()) {
                misc::badge::bind(label, &dot, &kuro_prefs::library::LibraryPrefs::new(data::app().base.store().clone()));
            }
            if let Some(label) = browse_label.as_ref().filter(|_| data::is_installed()) {
                misc::badge::bind_extension_updates(label, &dot, &kuro_prefs::source::SourcePrefs::new(data::app().base.store().clone()));
            }
            dots.push(dot.upcast());
        }
        let tip = n.title.clone();
        let row = gtk::ListBoxRow::builder().child(&h).tooltip_text(tip).build();
        row.set_margin_top(widgets::rail::ITEM_SPACING / 2);
        row.set_margin_bottom(widgets::rail::ITEM_SPACING / 2);
        list.append(&row);
        squares.push((img, row));
        rows.push(h);
    }
    {
        let (tabs, ids) = (tabs.clone(), ids.clone());
        list.connect_row_activated(move |_, r| {
            ui::pop_to_root();
            let id = &ids[r.index() as usize];
            // choosing the item that is already showing opens the library settings sheet (B-1.2)
            if id == "library" && tabs.visible_child_name().as_deref() == Some("library") {
                library::mount::reselect();
            }
            updates::mount::reselect(id, tabs.visible_child_name().as_deref() == Some(id.as_str()));
            // choosing More while it shows opens Settings (HomeScreen reselect)
            let showing = tabs.visible_child_name().as_deref() == Some(id.as_str());
            if showing && shell::tabs::tab_for_id(id, kuro_prefs::ui::Tab::AnimeLibrary).is_some_and(|t| shell::tabs::on_reselect(t) == shell::tabs::Reselect::OpenSettings) {
                pref_model::shell::open(None);
            }
            tabs.set_visible_child_name(id);
        });
    }
    // keep the selection in sync when the bottom bar (narrow mode) switches tabs
    let l2 = list.clone();
    tabs.connect_visible_child_name_notify(move |t| {
        let i = t.visible_child_name().and_then(|n| ids.iter().position(|x| *x == n));
        if let Some(r) = i.and_then(|i| l2.row_at_index(i as i32)) {
            l2.select_row(Some(&r));
        }
    });
    list.select_row(list.row_at_index(0).as_ref());

    let b = GBox::new(O::Vertical, widgets::rail::HEADER_GAP);
    b.set_size_request(widgets::rail::MIN_WIDTH, -1);
    b.append(&header);
    b.append(&gtk::ScrolledWindow::builder().child(&list).hscrollbar_policy(gtk::PolicyType::Never).propagate_natural_width(true).vexpand(true).build());

    // icon-only rail: hide labels/badges/title, show badge dots, drop the app-icon decoration
    let narrow = std::rc::Rc::new(std::cell::Cell::new(false));
    let narrow_now = narrow.clone();
    toggle.connect_toggled(move |t| {
        let rail = t.is_active() || narrow_now.get();
        expanded.iter().for_each(|w| w.set_visible(!rail));
        dots.iter().for_each(|w| w.set_visible(rail));
        rows.iter().for_each(|h| h.set_halign(if rail { gtk::Align::Center } else { gtk::Align::Fill }));
        for (img, row) in &squares {
            img.set_pixel_size(if rail { 24 } else { -1 });
            row.set_halign(if rail { gtk::Align::Center } else { gtk::Align::Fill });
            row.set_size_request(if rail { 48 } else { -1 }, if rail { 48 } else { -1 });
        }
        title.set_visible(!rail);
        header.set_decoration_layout(Some(if rail { ":".to_string() } else { ui::decoration(true) }.as_str()));
        t.set_tooltip_text(Some(if t.is_active() { "Expand sidebar" } else { "Collapse sidebar" }));
    });
    // below 600 px the labels hide and the icons remain, whatever the toggle says
    let follower = toggle.clone();
    widgets::rail::follow_width(&b, move |now| {
        narrow.set(now);
        follower.emit_by_name::<()>("toggled", &[]);
    });
    if std::env::var("KUROBAYOMI_RAIL").is_ok() {
        toggle.set_active(true);
    }
    b
}
