// Hand-built screens. Labels, menus, sheets and dialogs come from spec/*.json; layout follows the
// "notes" Aniyomi layouts. Anything without a builder here falls back to ui::generic.
use crate::ui::*;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};

pub struct Built {
    pub content: gtk::Widget,
    pub end: Vec<gtk::Widget>,
}

pub fn built(content: impl IsA<gtk::Widget>, end: Vec<gtk::Widget>) -> Option<Built> {
    Some(Built { content: content.upcast(), end })
}

/// Builder for a screen's body + header actions, if it has a custom one.
pub fn body(id: &str) -> Option<Built> {
    match id {
        "library" => libraries(),
        "anime_library" => Some(library(id, true)),
        "manga_library" => Some(library(id, false)),
        "updates" => crate::updates::mount::updates().or_else(|| unavailable(id)),
        "history" => crate::updates::mount::history().or_else(|| unavailable(id)),
        "more" => more(),
        "browse" => crate::browse::mount::browse().or_else(|| unavailable(id)),
        "global_search" | "migrate_search" => crate::browse::search_screen::body(id == "migrate_search").or_else(|| unavailable(id)),
        "download_queue" => downloads(),
        "categories" => categories(),
        "stats" => stats(),
        "storage" => storage(),
        "onboarding" | "new_update" | "crash" => info_screen(id),
        id if crate::settings_ui::handles(id) => crate::settings_ui::body(id),
        _ => crate::entry::body(id),
    }
}

/// A pushed page for `id`.
pub fn build(id: &str) -> GBox {
    match id {
        "upcoming" => {
            if let Some(page) = crate::updates::mount::upcoming() {
                return page;
            }
        }
        "anime_entry_details" => return crate::entry_core::anime::route::build(),
        "manga_entry_details" => return crate::entry_core::route::build(),
        "browse_source" | "migrate_source_search" => {
            if let Some(page) = crate::browse::browse_screen::page(id) {
                return page;
            }
        }
        "migrate_entries" => {
            if let Some(page) = crate::browse::migrate_screen::page() {
                return page;
            }
        }
        "source_preferences" => {
            if let Some(page) = crate::browse::source_prefs_screen::page() {
                return page;
            }
        }
        _ => {}
    }
    let s = screen(id);
    match body(id) {
        Some(b) => {
            let p = page(&s.title, &b.content, b.end, true);
            tag(&p, id);
            p
        }
        None => generic(&s),
    }
}

// ---------- shared pieces ----------

/// Whether manga row `id` belongs to the local source.
fn is_local_manga(id: i64) -> bool {
    crate::data::is_installed() && kuro_data::manga::MangaRepo::new(crate::data::app().db(kuro_domain::kind::EntryKind::Manga)).by_id(id).is_ok_and(|m| m.source == kuro_local::LOCAL_SOURCE_ID)
}

/// Opens the details page of entry `id`, by id. A local manga is opened through the local flow, which also remembers
/// the id for the reader to tell two local folders with one name apart; any other manga is a plain entry page.
pub fn open_entry(anime: bool, id: i64) {
    if anime {
        crate::entry_core::anime::route::open(id);
    } else if is_local_manga(id) {
        crate::local_flow::open::entry(id);
    } else {
        crate::entry_core::route::open(id);
    }
}

/// Opens unit `unit_id` (a chapter or an episode row id) of entry `entry_id` in the reader or the player, by id. A
/// chapter opens in the reader. An episode plays from its file, or from the online source that owns it; when neither
/// can, a toast says why.
pub fn open_unit(anime: bool, entry_id: i64, unit_id: i64) {
    if !crate::data::is_installed() {
        return;
    }
    if !anime {
        push(&if is_local_manga(entry_id) { crate::local_flow::reader::open_local_chapter(entry_id, unit_id) } else { crate::reader::launch::open_chapter(entry_id, unit_id) });
        return;
    }
    if crate::local_flow_anime::open_local_episode(unit_id) || crate::player_view::open_source_episode(&anime_sources(), unit_id) {
        return;
    }
    let source = kuro_data::episode::EpisodeRepo::new(crate::data::app().anime_db.clone())
        .by_id(unit_id)
        .ok()
        .flatten()
        .and_then(|e| kuro_data::anime::AnimeRepo::new(crate::data::app().anime_db.clone()).by_id(e.anime_id).ok())
        .map_or_else(|| unit_id.to_string(), |a| a.source.to_string());
    crate::toast::notify(crate::reader::strings::source_not_installed(&source));
}

/// Opens a unit by its entry's title and its own name, which finds only a local entry's file. Kept for
/// `entry_core::anime::route::open_player`, whose fallback should call [`open_unit`] with the episode id instead.
// ponytail: delete when entry_core opens its fallback by id.
pub fn open_media(anime: bool, title: &str, unit: &str) {
    if anime {
        if !crate::local_flow_anime::open_episode(title, unit) {
            crate::toast::notify(crate::reader::strings::source_not_installed(title));
        }
    } else {
        push(&crate::local_flow::reader::page_for(title, unit));
    }
}

/// The anime sources this build runs: the builtin ones, the recipe sources and the shipped source crates.
pub fn anime_sources() -> Vec<std::sync::Arc<dyn kuro_source::anime::AnimeSource>> {
    let mut sources = kuro_source::anime::registry::builtin_anime_sources();
    sources.extend(crate::recipe_sources::anime_sources());
    sources.extend(crate::browse::builtin::anime_sources());
    sources
}

/// What a page shows when its live builder cannot start (no app data, or nothing was parked for it): the screen's
/// empty text, never made-up rows.
pub(crate) fn unavailable(id: &str) -> Option<Built> {
    Some(empty_built(id))
}

fn empty_built(id: &str) -> Built {
    let text = screen(id).empty.unwrap_or_else(|| kuro_i18n::tr(kuro_i18n::MsgKey::no_results_found));
    Built { content: crate::browse::look::empty_state("system-search-symbolic", &text, None).0.upcast(), end: vec![] }
}

/// A pushed page with a view per kind: the Anime | Manga switch goes in the page's one header row (as an `end` widget
/// of `ui::page`), not on a row of its own under it. The stack pages are named `anime` and `manga`.
fn kind_switch(f: impl Fn(bool) -> gtk::Widget) -> (gtk::Stack, gtk::StackSwitcher) {
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_titled(&f(true), Some("anime"), "Anime");
    stack.add_titled(&f(false), Some("manga"), "Manga");
    let switch = gtk::StackSwitcher::builder().stack(&stack).build();
    (stack, switch)
}

/// Opens the spec dialog `dialog` of screen `id`, its text naming `name` where it has `%s` (the delete-category
/// question). The last button runs `on_ok` before closing.
pub(crate) fn dialog_then_named(id: &str, dialog: &str, name: &str, on_ok: impl Fn() + 'static) {
    if let Some(d) = screen(id).dialogs.iter().find(|d| d.id == dialog) {
        let mut d = d.clone();
        d.text = d.text.map(|text| text.replace("%s", name));
        show_dialog_then(&d, on_ok);
    }
}

pub(crate) fn list_row(prefix: Option<gtk::Widget>, title: &str, sub: &str, suffix: Vec<gtk::Widget>) -> gtk::ListBoxRow {
    // the shared row metrics (`ui::row`)
    let h = GBox::new(O::Horizontal, 12);
    h.set_margin_start(ROW_PAD_H);
    h.set_margin_end(ROW_PAD_H);
    h.set_margin_top(ROW_PAD_V);
    h.set_margin_bottom(ROW_PAD_V);
    h.set_size_request(-1, ROW_MIN_HEIGHT - 2 * ROW_PAD_V);
    if let Some(p) = prefix {
        h.append(&p);
    }
    let v = GBox::new(O::Vertical, 2);
    v.set_hexpand(true);
    v.set_valign(Align::Center);
    v.append(&Label::builder().label(title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
    if !sub.is_empty() {
        v.append(&dim(sub));
    }
    h.append(&v);
    for s in suffix {
        s.set_valign(Align::Center);
        h.append(&s);
    }
    gtk::ListBoxRow::builder().child(&h).activatable(true).build()
}

pub(crate) fn fab(overlay: &gtk::Overlay, label: &str, icon: &str) -> Button {
    let h = GBox::new(O::Horizontal, 6);
    h.append(&gtk::Image::from_icon_name(icon));
    h.append(&Label::new(Some(label)));
    let b = Button::builder().child(&h).css_classes(["suggested-action", "pill"]).halign(Align::End).valign(Align::End).build();
    margins(&b, 18);
    overlay.add_overlay(&b);
    b
}

// ---------- library ----------

/// The Library destination as a whole page: one header row (Anime | Manga switch, the visible kind's search, its
/// refresh and menu) over the two library views (docs/design/main-window.md). The page has no title of its own.
fn libraries() -> Option<Built> {
    built(library_page([true, false].map(|anime| library(if anime { "anime_library" } else { "manga_library" }, anime))), vec![])
}

/// The Library page from the Anime and the Manga views, in that order. Updates and History use the same header
/// (`updates::mount`).
pub(crate) fn library_page(views: [Built; 2]) -> GBox {
    let kinds = gtk::Stack::new();
    kinds.set_transition_type(gtk::StackTransitionType::Crossfade);
    kinds.set_vexpand(true);
    let (centre, end) = (gtk::Stack::new(), gtk::Stack::new());
    for (b, anime) in views.into_iter().zip([true, false]) {
        let name = if anime { "anime" } else { "manga" };
        kinds.add_titled(&b.content, Some(name), if anime { "Anime" } else { "Manga" });
        let mut rest = b.end.into_iter().peekable();
        // the live view hands its search entry first; it takes the middle of the header
        let search = rest.next_if(|w| w.is::<gtk::SearchEntry>()).unwrap_or_else(|| GBox::new(O::Horizontal, 0).upcast());
        centre.add_named(&search, Some(name));
        let actions = GBox::new(O::Horizontal, 4);
        rest.for_each(|w| actions.append(&w));
        end.add_named(&actions, Some(name));
    }
    for follower in [&centre, &end] {
        kinds.bind_property("visible-child-name", follower, "visible-child-name").sync_create().build();
    }
    let header = gtk::HeaderBar::new();
    header.add_css_class("kb-library-header");
    header.set_decoration_layout(Some(&decoration(false)));
    header.pack_start(&gtk::StackSwitcher::builder().stack(&kinds).build());
    header.set_title_widget(Some(&centre));
    header.pack_end(&end);
    no_sticky_focus(&header);
    let page = GBox::new(O::Vertical, 0);
    page.append(&header);
    page.append(&kinds);
    page
}

fn library(id: &str, anime: bool) -> Built {
    crate::library::mount::build(anime).unwrap_or_else(|| empty_built(id))
}

// ---------- more & friends ----------

/// The leading icon of a More row, by its spec title (the icons Aniyomi's `MoreScreen` uses, in GTK names).
fn more_icon(title: &str) -> Option<&'static str> {
    Some(match title {
        "Downloaded only" => "network-offline-symbolic",
        "Incognito mode" => "view-conceal-symbolic",
        "Download queue" => "folder-download-symbolic",
        "Categories" => "tag-symbolic",
        "Statistics" => "x-office-spreadsheet-symbolic",
        "Data and storage" => "drive-harddisk-symbolic",
        "Settings" => "emblem-system-symbolic",
        "Player settings" => "video-display-symbolic",
        "About" => "help-about-symbolic",
        "Help" => "help-browser-symbolic",
        _ => return None,
    })
}

/// More: three boxed lists with icons (switches, library pages, settings and help) in the page column. No logo and no
/// title: the sidebar names the tab and its header names the app (docs/design/other-tabs.md).
fn more() -> Option<Built> {
    let s = screen("more");
    let col = column();
    // the three lists have no headings, so the column itself leaves the 24 px between groups
    col.set_spacing(24);
    // History has its own sidebar entry, so More drops the "moved tab" row (Aniyomi's nav-style rule)
    let mut groups = s.groups.clone();
    for g in &mut groups {
        g.items.retain(|it| !matches!(it, crate::spec::Item::Link { target, .. } if target == "history"));
        // The queue row shows the derived download state, not the mockup text; no manager runs in the app yet, so Stopped.
        for it in &mut g.items {
            if let crate::spec::Item::Link { subtitle, target, .. } = it {
                if target == "download_queue" {
                    *subtitle = crate::pref_model::more::queue_subtitle(crate::pref_model::queue_state::QueueState::Stopped);
                }
            }
        }
    }
    for (at, g) in groups.iter().enumerate() {
        let list = boxed_list();
        // the tab the nav style moves into More leads the second list, when the sidebar does not carry it
        if at == 1 {
            if let Some(row) = moved_row() {
                list.append(&row);
            }
        }
        for it in &g.items {
            let Some(r) = item_row(it) else { continue };
            let title = crate::pref_model::spec_convert::item_title(it);
            if let (Some(icon), Some(inner)) = (more_icon(&title), r.child().and_downcast::<GBox>()) {
                inner.prepend(&gtk::Image::from_icon_name(icon));
            }
            if title == "Help" {
                crate::widgets::row_activate::on_activate(&r, |_| open_help());
            }
            list.append(&r);
        }
        if list.first_child().is_some() {
            col.append(&list);
        }
    }
    tag(&col, "more");
    built(scrolled(&col), vec![])
}

/// What the More row of the moved tab shows (title, icon, the page it opens): the tab `style` moves out of the
/// navigation, when `nav_ids` (the sidebar's pages) no longer carry it. Both libraries are one sidebar page, so a
/// moved manga library has no row of its own.
fn moved_target(style: kuro_prefs::ui::NavStyle, nav_ids: &[&str]) -> Option<(String, &'static str, &'static str)> {
    use kuro_prefs::ui::Tab;
    let tab = style.more_tab();
    let page = match tab {
        Tab::Updates => "updates",
        Tab::History => "history",
        Tab::Browse => "browse",
        _ => return None,
    };
    (!nav_ids.contains(&crate::shell::tabs::nav_id(tab))).then(|| (crate::pref_model::more::moved_title(tab), crate::pref_model::more::moved_icon(style), page))
}

// ponytail: dead until the sidebar (`main.rs`) lists only the tabs the nav style keeps; it lists every spec entry now.
fn moved_row() -> Option<gtk::ListBoxRow> {
    if !crate::data::is_installed() {
        return None;
    }
    let style = crate::settings_ui::route::prefs().ui.nav_style.get();
    with_spec(|s| moved_row_for(style, &s.nav.iter().map(|n| n.id.as_str()).collect::<Vec<_>>()))
}

/// The More row of the tab `style` moves out of the sidebar pages `nav_ids`, when it moves one.
fn moved_row_for(style: kuro_prefs::ui::NavStyle, nav_ids: &[&str]) -> Option<gtk::ListBoxRow> {
    let (title, icon, page) = moved_target(style, nav_ids)?;
    let row = item_row(&crate::spec::Item::Link { title, subtitle: None, target: page.to_owned() })?;
    if let Some(inner) = row.child().and_downcast::<GBox>() {
        inner.prepend(&gtk::Image::from_icon_name(icon));
    }
    Some(row)
}

#[cfg(test)]
thread_local! {
    /// Test hook: how many times [`open_help`] ran; tests count instead of starting the browser.
    static HELP_OPENED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Opens the help site in the browser (More, Help).
fn open_help() {
    #[cfg(test)]
    HELP_OPENED.with(|n| n.set(n.get() + 1));
    #[cfg(not(test))]
    gtk::UriLauncher::new(crate::pref_model::widgets::HELP_URL).launch(None::<&gtk::Window>, None::<&gtk::gio::Cancellable>, |result| {
        if let Err(err) = result {
            tracing::warn!(%err, "could not open the help page");
        }
    });
}

/// Download queue. No download manager runs in the app yet, so the queue is always empty: the Library's empty state
/// with one way forward. The Anime | Manga switch, sort, Cancel all and the pause button come back with the manager.
fn downloads() -> Option<Built> {
    let library = Button::with_label("Library");
    library.connect_clicked(|_| open("library"));
    let (page, _) = crate::browse::look::empty_state("folder-download-symbolic", &screen("download_queue").empty.unwrap_or_default(), Some(&library));
    built(page, vec![])
}

/// Categories: the Anime | Manga switch and Add (for the visible kind) in the header row, a page per kind.
fn categories() -> Option<Built> {
    let (kinds, switch) = kind_switch(crate::live::categories_content);
    let add = icon_button("list-add-symbolic", "Add category");
    let visible = kinds.clone();
    add.connect_clicked(move |_| crate::live::add_category(visible.visible_child_name().as_deref() == Some("anime")));
    built(kinds, vec![switch.upcast(), add.upcast()])
}

fn stats() -> Option<Built> {
    crate::updates::mount::stats()
}

fn storage() -> Option<Built> {
    crate::updates::mount::storage()
}

// Onboarding / New update / Crash share Aniyomi's InfoScreen layout.
fn info_screen(id: &str) -> Option<Built> {
    let s = screen(id);
    let col = column();
    let (icon, subtitle, buttons): (&str, &str, &[&str]) = match id {
        "onboarding" => ("starred-symbolic", "Let's set some things up first. You can always change these in the settings later too.", &["Next"]),
        "new_update" => ("software-update-available-symbolic", concat!("v", env!("CARGO_PKG_VERSION")), &["Download", "Not now"]),
        _ => ("dialog-error-symbolic", "Kurobayomi ran into an unexpected error. We suggest you share the crash logs in our support channel on Discord.", &["Share crash logs", "Restart the application"]),
    };
    col.append(&gtk::Image::builder().icon_name(icon).pixel_size(48).halign(Align::Start).build());
    col.append(&Label::builder().label(&s.title).xalign(0.0).css_classes(["title-1"]).build());
    col.append(&dim(subtitle));
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    if id == "onboarding" {
        for g in &s.groups {
            let c = GBox::new(O::Vertical, 12);
            groups_into(&c, std::slice::from_ref(g));
            stack.add_child(&c);
        }
    } else {
        // no crash log or release notes are recorded yet: say so rather than show made-up text
        let text = if id == "crash" { "No crash log was recorded." } else { "No release notes available." };
        stack.add_child(&gtk::Frame::builder().child(&Label::builder().label(text).wrap(true).xalign(0.0).margin_start(12).margin_top(12).margin_end(12).margin_bottom(12).build()).build());
        groups_into(&col, &s.groups);
    }
    col.append(&stack);
    let row = GBox::new(O::Vertical, 6);
    for (i, b) in buttons.iter().enumerate() {
        let btn = Button::with_label(b);
        if i == 0 {
            btn.add_css_class("suggested-action");
        }
        row.append(&btn);
        if id == "onboarding" {
            let st = stack.clone();
            btn.connect_clicked(move |btn| {
                match st.visible_child().and_then(|c| c.next_sibling()) {
                    Some(n) => {
                        st.set_visible_child(&n);
                        if n.next_sibling().is_none() {
                            btn.set_label("Get started");
                        }
                    }
                    None => pop(),
                }
            });
        } else if i == 1 {
            btn.connect_clicked(|_| pop());
        }
    }
    col.append(&row);
    tag(&col, id);
    built(scrolled(&col), vec![])
}

/// Harness for tests that need the app data installed. `data::install` binds the data to the GTK thread for good, so
/// the normal run starts this test binary again for the one test, with a temporary home and fresh databases, and the
/// child runs the body. Without a display the test skips itself.
#[cfg(test)]
pub(crate) mod alone {
    use gtk::glib;
    use gtk::prelude::*;

    const ALONE: &str = "KUROBAYOMI_TEST_ALONE";

    /// Runs `body` on the GTK thread of a child process: the data installed, a main context owned, a runtime entered.
    /// Call it as `alone::run(module_path!(), "this_test_name", |ctx| { .. })`.
    pub fn run(module: &str, name: &str, body: impl FnOnce(&glib::MainContext) + Send + 'static) {
        if std::env::var_os(ALONE).is_none() {
            if !crate::testing::has_display() {
                eprintln!("skipped: no display for this GTK test");
                return;
            }
            let home = tempfile::tempdir().expect("a temporary home");
            let path = module.split_once("::").map_or(module, |(_, rest)| rest);
            let test = format!("{path}::{name}");
            let mut child = std::process::Command::new(std::env::current_exe().expect("the test binary"));
            // the platform's base-directory variables would point past the temporary home
            for (key, _) in std::env::vars().filter(|(k, _)| ["_DATA_HOME", "_CONFIG_HOME", "_CACHE_HOME", "_STATE_HOME"].iter().any(|s| k.ends_with(s))) {
                child.env_remove(key);
            }
            let out = child.args([test.as_str(), "--exact", "--test-threads=1", "--nocapture"]).env(ALONE, "1").env("HOME", home.path()).output().expect("the test binary runs");
            let (stdout, stderr) = (String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
            assert!(out.status.success() && stdout.contains("1 passed"), "the child run of {test} failed:\n{stdout}\n{stderr}");
            assert!(!stderr.contains("skipped: no display"), "the child lost the display:\n{stderr}");
            return;
        }
        crate::testing::with_display(move || {
            let ctx = glib::MainContext::default();
            let _owner = ctx.acquire().expect("main context");
            let runtime = tokio::runtime::Runtime::new().expect("a runtime");
            let _inside = runtime.enter();
            let dir = tempfile::tempdir().expect("a temporary folder");
            crate::data::install(crate::data::open_in(dir.path()).expect("fresh data")).expect("installed");
            body(&ctx);
        });
    }

    /// Pumps `ctx` until `done` holds or three seconds pass.
    pub fn pump(ctx: &glib::MainContext, done: impl Fn() -> bool) -> bool {
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(3) {
            while ctx.iteration(false) {}
            if done() {
                return true;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        done()
    }

    /// `root` and every widget below it, in tree order.
    pub fn descendants(root: &gtk::Widget) -> Vec<gtk::Widget> {
        let mut all = vec![root.clone()];
        let mut child = root.first_child();
        while let Some(c) = child {
            all.extend(descendants(&c));
            child = c.next_sibling();
        }
        all
    }

    /// The toast overlay of a test: the label the toast presenter writes to.
    pub fn toast_label() -> gtk::Label {
        let overlay = gtk::Overlay::new();
        crate::toast::attach(&overlay);
        let revealer = overlay.last_child().and_downcast::<gtk::Revealer>().expect("the toast revealer");
        revealer.child().and_downcast::<gtk::Label>().expect("the toast label")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The widgets a user can see: a stack contributes only its visible page.
    fn shown(root: &gtk::Widget, out: &mut Vec<gtk::Widget>) {
        if root.is::<gtk::StackSwitcher>() {
            return; // its button labels are the switch, not titles
        }
        if let Some(stack) = root.downcast_ref::<gtk::Stack>() {
            if let Some(page) = stack.visible_child() {
                out.push(page.clone());
                shown(&page, out);
            }
            return;
        }
        let mut child = root.first_child();
        while let Some(w) = child {
            if w.is_visible() {
                out.push(w.clone());
                shown(&w, out);
            }
            child = w.next_sibling();
        }
    }

    fn view(word: &str) -> Built {
        let search = gtk::SearchEntry::new();
        let refresh = icon_button("view-refresh-symbolic", "Update category");
        let menu = gtk::MenuButton::builder().icon_name("open-menu-symbolic").build();
        Built { content: Label::new(Some(word)).upcast(), end: vec![search.upcast(), refresh.upcast(), menu.upcast()] }
    }

    #[test]
    fn the_moved_tab_gets_a_more_row_only_when_the_sidebar_lacks_it() {
        use kuro_prefs::ui::NavStyle;
        let full = ["library", "updates", "history", "browse", "more"];
        assert_eq!(moved_target(NavStyle::MOVE_HISTORY_TO_MORE, &full), None, "the sidebar still has History");
        let (title, icon, page) = moved_target(NavStyle::MOVE_HISTORY_TO_MORE, &["library", "updates", "browse", "more"]).expect("History moved");
        assert_eq!((title.as_str(), icon, page), ("History", "document-open-recent-symbolic", "history"));
        let (title, icon, page) = moved_target(NavStyle::MOVE_BROWSE_TO_MORE, &["library", "updates", "history", "more"]).expect("Browse moved");
        assert_eq!((title.as_str(), icon, page), ("Browse", "compass-symbolic", "browse"));
        assert_eq!(moved_target(NavStyle::MOVE_MANGA_TO_MORE, &["updates", "more"]), None, "the manga library has no page of its own");
    }

    /// Needs a display, and skips itself without one.
    #[test]
    fn the_moved_tab_row_is_on_the_more_page_and_opens_the_moved_tab() {
        use kuro_prefs::ui::NavStyle;
        crate::testing::with_display(|| {
            let (_stack, _window) = crate::testing::nav();
            let kept = ["library", "updates", "browse", "more"];
            let list = boxed_list();
            let row = moved_row_for(NavStyle::MOVE_HISTORY_TO_MORE, &kept).expect("History moved into More");
            list.append(&row);
            assert_eq!(crate::widgets::row_activate::row_titled(&list, "History"), row, "the row names the moved tab");
            assert_eq!(crate::ui::opened_during(|| crate::widgets::row_activate::click_row(&row)), ["history"], "a click opens it");
            assert!(moved_row_for(NavStyle::MOVE_HISTORY_TO_MORE, &["library", "updates", "history", "browse", "more"]).is_none(), "no row while the sidebar has the tab");
        });
    }

    /// Needs a display and runs in a child process with fresh app data (see [`alone`]).
    #[test]
    fn opening_by_id_reaches_the_entry_page_the_reader_and_the_source_not_installed_toast() {
        use kuro_data::anime::{Anime, AnimeRepo};
        use kuro_data::chapter::{Chapter, ChapterRepo};
        use kuro_data::episode::{Episode, EpisodeRepo};
        use kuro_data::manga::{Manga, MangaRepo};
        use kuro_domain::kind::EntryKind;
        alone::run(module_path!(), "opening_by_id_reaches_the_entry_page_the_reader_and_the_source_not_installed_toast", |ctx| {
            let (nav, _window) = crate::testing::nav();
            let toast = alone::toast_label();
            let app = crate::data::app();
            let top = || nav.visible_child().map(|c| c.widget_name().to_string());
            let (mangas, chapters) = (MangaRepo::new(app.db(EntryKind::Manga)), ChapterRepo::new(app.db(EntryKind::Manga)));
            let manga = |source: i64, url: &str| {
                let id = mangas.insert(&Manga { source, url: url.into(), title: url.into(), ..Manga::create() }).expect("manga");
                let added = chapters.add_all(&[Chapter { manga_id: id, url: "/c1".into(), name: "Chapter 1".into(), chapter_number: 1.0, ..Chapter::create() }]).expect("chapter");
                (id, added[0].id)
            };
            let (remote, remote_chapter) = manga(7, "/remote");
            let (local, local_chapter) = manga(kuro_local::LOCAL_SOURCE_ID, "/local");
            assert!(!is_local_manga(remote) && is_local_manga(local), "only the local source's rows are local");
            assert!(!is_local_manga(99_999), "a missing row is not local");

            // a remote manga's page leaves the reader's local-entry memory alone; a local one sets it
            assert_eq!(crate::local_flow::open::current(), None);
            open_entry(false, remote);
            assert!(alone::pump(ctx, || top().as_deref() == Some("screen:manga_entry_details")), "a remote manga opens its entry page");
            assert_eq!(crate::local_flow::open::current(), None, "a remote manga is not the local entry the reader disambiguates");
            open_entry(false, local);
            assert!(alone::pump(ctx, || crate::local_flow::open::current() == Some(local)), "a local manga is remembered for the reader");
            assert_eq!(top().as_deref(), Some("screen:manga_entry_details"));

            // a chapter opens in the reader, from the online or the local path
            open_unit(false, remote, remote_chapter);
            assert!(alone::pump(ctx, || top().as_deref() == Some("screen:reader")), "a remote chapter opens the reader");
            crate::testing::nav(); // an empty stack, so the second reader is the one that is seen
            open_unit(false, local, local_chapter);
            assert!(alone::pump(ctx, || top().as_deref() == Some("screen:reader")), "a local chapter opens the reader");

            // an episode of a source that is not installed says so, and opens nothing
            let anime = AnimeRepo::new(app.anime_db.clone()).insert(&Anime { source: 7, url: "/a".into(), title: "A".into(), ..Anime::create() }).expect("anime");
            let episodes = EpisodeRepo::new(app.anime_db.clone());
            let episode = episodes.add_all(&[Episode { anime_id: anime, url: "/e1".into(), name: "Episode 1".into(), episode_number: 1.0, ..Episode::create() }]).expect("episode")[0].id;
            let before = top();
            open_unit(true, anime, episode);
            assert!(alone::pump(ctx, || toast.text() == crate::reader::strings::source_not_installed("7")), "the toast names the missing source, got {:?}", toast.text());
            assert_eq!(top(), before, "no page was opened");
            open_entry(true, anime);
            assert!(alone::pump(ctx, || top().as_deref() == Some("screen:anime_entry_details")), "an anime opens its entry page by id");
        });
    }

    #[test]
    fn the_delete_category_question_names_the_category() {
        let text = screen("categories").dialogs.iter().find(|d| d.id == "category_delete").and_then(|d| d.text.clone()).expect("the dialog");
        assert_eq!(text.replace("%s", "Reading"), "Do you wish to delete the category \"Reading\"?");
    }

    /// Needs a display, and skips itself without one.
    #[test]
    fn more_help_opens_the_help_site_once_per_click_and_once_per_key() {
        crate::testing::with_display(|| {
            let page = build("more");
            let row = crate::widgets::row_activate::row_titled(&page, "Help");
            HELP_OPENED.with(|n| n.set(0));
            crate::widgets::row_activate::click_row(&row);
            assert_eq!(HELP_OPENED.with(|n| n.get()), 1, "a click");
            row.emit_by_name::<()>("activate", &[]);
            assert_eq!(HELP_OPENED.with(|n| n.get()), 2, "Enter or Space");
        });
    }

    /// Needs a display, and skips itself without one.
    #[test]
    fn the_download_queue_is_an_empty_state_with_one_way_forward() {
        crate::testing::with_display(|| {
            let page = build("download_queue");
            let mut all = vec![page.clone().upcast::<gtk::Widget>()];
            shown(page.upcast_ref(), &mut all);
            let count = |f: &dyn Fn(&gtk::Widget) -> bool| all.iter().filter(|w| f(w)).count();
            assert_eq!(count(&|w| w.is::<gtk::HeaderBar>()), 1, "one header row");
            assert_eq!(count(&|w| w.is::<gtk::ListBoxRow>()), 0, "no made-up queue rows");
            assert_eq!(count(&|w| w.downcast_ref::<Label>().is_some_and(|l| l.label() == "No downloads")), 1);
            assert_eq!(count(&|w| w.downcast_ref::<Button>().is_some_and(|b| b.label().as_deref() == Some("Library"))), 1, "one action");
            assert_eq!(count(&|w| w.is::<gtk::MenuButton>() || w.is::<gtk::StackSwitcher>()), 0, "no menus that do nothing");
        });
    }

    #[test]
    fn the_library_header_is_one_row_with_each_control_once() {
        crate::testing::with_display(|| {
            let page = library_page([view("anime grid"), view("manga grid")]);
            assert!(page.first_child().is_some_and(|w| w.is::<gtk::HeaderBar>()), "the header row comes first");
            assert!(page.first_child().and_then(|w| w.next_sibling()).is_some_and(|w| !w.is::<gtk::HeaderBar>() && w.next_sibling().is_none()), "then only the views");
            let mut all = vec![];
            shown(page.upcast_ref(), &mut all);
            let count = |f: &dyn Fn(&gtk::Widget) -> bool| all.iter().filter(|w| f(w)).count();
            assert_eq!(count(&|w| w.is::<gtk::StackSwitcher>()), 1, "the Anime | Manga switch");
            assert_eq!(count(&|w| w.is::<gtk::SearchEntry>()), 1, "one search");
            assert_eq!(count(&|w| w.is::<gtk::MenuButton>()), 1, "one menu");
            let refresh = |w: &gtk::Widget| w.downcast_ref::<Button>().is_some_and(|b| b.icon_name().as_deref() == Some("view-refresh-symbolic"));
            assert_eq!(count(&refresh), 1, "one refresh");
            assert_eq!(count(&|w| w.downcast_ref::<Label>().is_some_and(|l| matches!(l.label().as_str(), "Library" | "Anime" | "Manga"))), 0, "no page title");
            assert_eq!(count(&|w| w.downcast_ref::<Label>().is_some_and(|l| l.label() == "anime grid")), 1, "Anime shows first");
        });
    }
}
