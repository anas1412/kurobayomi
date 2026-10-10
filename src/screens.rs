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
        "updates" => crate::updates::mount::updates().or_else(updates),
        "history" => crate::updates::mount::history().or_else(history),
        "more" => more(),
        "browse" => crate::browse::mount::browse().or_else(|| crate::entry::body(id)),
        "global_search" | "migrate_search" => crate::browse::search_screen::body(id == "migrate_search").or_else(|| crate::entry::body(id)),
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
        "reader" => return crate::reader::launch::page_for("Title 1", "Chapter 1"),
        "upcoming" => {
            if let Some(page) = crate::updates::mount::upcoming() {
                return page;
            }
        }
        "player" => return crate::player::player("Title 1", "Episode 1"),
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

pub fn title_n(i: usize) -> String {
    format!("Title {i}")
}

pub fn open_entry(title: &str, anime: bool) {
    if let Some(id) = anime.then(|| crate::local_flow_anime::anime_id_for_title(title)).flatten() {
        crate::entry_core::anime::route::open(id);
        return;
    }
    if !anime && crate::local_flow::open::entry_for_title(title) {
        return;
    }
    push(&crate::entry::entry(title, anime));
}

pub fn open_media(anime: bool, title: &str, unit: &str) {
    if anime && crate::local_flow_anime::open_episode(title, unit) {
        return;
    }
    push(&if anime { crate::player::player(title, unit) } else { crate::local_flow::reader::page_for(title, unit) });
}

/// Top "Anime | Manga" switcher with one page per kind (Updates, History, Downloads, …).
pub fn kind_tabs(f: impl Fn(bool) -> gtk::Widget) -> GBox {
    kind_stack(f).0
}

fn kind_stack(f: impl Fn(bool) -> gtk::Widget) -> (GBox, gtk::Stack) {
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    stack.add_titled(&f(true), Some("anime"), "Anime");
    stack.add_titled(&f(false), Some("manga"), "Manga");
    let sw = gtk::StackSwitcher::builder().stack(&stack).halign(Align::Center).margin_top(6).build();
    let b = GBox::new(O::Vertical, 6);
    b.append(&sw);
    stack.set_vexpand(true);
    b.append(&stack);
    (b, stack)
}

pub(crate) fn badge(text: &str, class: &str) -> Label {
    Label::builder().label(text).css_classes([class, "caption-heading"]).build()
}

fn dialog_of(id: &str, dialog: &str) {
    if let Some(d) = screen(id).dialogs.iter().find(|d| d.id == dialog) {
        show_dialog(d);
    }
}

/// Opens the spec dialog `dialog` of screen `id`. Its last button runs `on_ok` before closing.
pub(crate) fn dialog_then(id: &str, dialog: &str, on_ok: impl Fn() + 'static) {
    if let Some(d) = screen(id).dialogs.iter().find(|d| d.id == dialog) {
        show_dialog_then(d, on_ok);
    }
}

pub(crate) fn thumb(w: i32, h: i32) -> gtk::Frame {
    let c = cover(w, h);
    c.set_valign(Align::Center);
    c
}

pub(crate) fn list_row(prefix: Option<gtk::Widget>, title: &str, sub: &str, suffix: Vec<gtk::Widget>) -> gtk::ListBoxRow {
    let h = GBox::new(O::Horizontal, 12);
    margins(&h, 6);
    h.set_margin_start(12);
    if let Some(p) = prefix {
        h.append(&p);
    }
    let v = GBox::new(O::Vertical, 2);
    v.set_hexpand(true);
    v.set_valign(Align::Center);
    v.append(&Label::builder().label(title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).build());
    v.append(&dim(sub));
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

/// Anime and Manga libraries behind one destination; the header actions follow the active kind.
fn libraries() -> Option<Built> {
    let end = gtk::Stack::new();
    let (content, kinds) = kind_stack(|anime| {
        let b = library(if anime { "anime_library" } else { "manga_library" }, anime);
        let actions = GBox::new(O::Horizontal, 6);
        b.end.iter().for_each(|w| actions.append(w));
        end.add_named(&actions, Some(if anime { "anime" } else { "manga" }));
        b.content
    });
    kinds.bind_property("visible-child-name", &end, "visible-child-name").sync_create().build();
    built(content, vec![end.upcast()])
}

fn library(id: &str, anime: bool) -> Built {
    // the live library view; the prototype below stays as the fallback when its worker cannot start
    if let Some(mut b) = crate::library::mount::build(anime) {
        if anime {
            b.end.insert(0, crate::local_flow_anime::header_actions().upcast());
        } else {
            b.end.insert(0, crate::local_flow::ui::header_actions().upcast());
        }
        return b;
    }
    let s = screen(id);
    let body = GBox::new(O::Vertical, 0);

    let search = gtk::SearchBar::new();
    let entry = gtk::SearchEntry::builder().placeholder_text("Search…").build();
    search.set_child(Some(&entry));
    search.connect_entry(&entry);
    body.append(&search);

    // category tabs, filled from the category pages once the grid is attached below
    let tabs = GBox::new(O::Horizontal, 0);
    tabs.add_css_class("linked");
    tabs.set_halign(Align::Center);
    body.append(&tabs);

    let grid = gtk::FlowBox::builder()
        .homogeneous(true)
        .min_children_per_line(2)
        .max_children_per_line(12)
        .column_spacing(8)
        .row_spacing(12)
        .valign(Align::Start)
        .selection_mode(gtk::SelectionMode::None)
        .build();
    margins(&grid, 12);
    crate::live::attach_library(&grid, &tabs, anime);
    {
        let entry = entry.clone();
        grid.set_filter_func(move |child| {
            let q = entry.text().to_lowercase();
            let label = child.child().and_then(|c| c.last_child()).and_downcast::<Label>();
            label.is_none_or(|l| l.label().to_lowercase().contains(&q))
        });
    }
    let g2 = grid.clone();
    entry.connect_search_changed(move |_| g2.invalidate_filter());

    // selection mode: right click / long press a cover shows Aniyomi's bottom action bar
    let actions = GBox::new(O::Horizontal, 6);
    actions.add_css_class("toolbar");
    actions.set_halign(Align::Center);
    let bar = gtk::Revealer::builder().child(&actions).build();
    let (cc, del) = (format!("{}_change_category", if anime { "anime" } else { "manga" }), format!("{}_delete_from_library", if anime { "anime" } else { "manga" }));
    let (idc, idd) = (id.to_string(), id.to_string());
    let setcat = icon_button("tag-symbolic", "Set categories");
    setcat.connect_clicked(move |_| dialog_of(&idc, &cc));
    let seen = icon_button("object-select-symbolic", if anime { "Mark as watched" } else { "Mark as read" });
    let unseen = icon_button("edit-undo-symbolic", if anime { "Mark as unwatched" } else { "Mark as unread" });
    let unit = if anime { "episode" } else { "chapter" };
    let dl = menu_button("folder-download-symbolic", &[format!("Next {unit}"), format!("Next 5 {unit}s"), format!("Next 10 {unit}s"), format!("Next 25 {unit}s"), (if anime { "Unwatched" } else { "Unread" }).into()], |_| {});
    let delete = icon_button("user-trash-symbolic", "Delete");
    delete.connect_clicked(move |_| dialog_of(&idd, &del));
    let close = icon_button("window-close-symbolic", "Clear selection");
    for w in [setcat.upcast_ref::<gtk::Widget>(), seen.upcast_ref(), unseen.upcast_ref(), dl.upcast_ref(), delete.upcast_ref(), close.upcast_ref()] {
        actions.append(w);
    }
    {
        let bar = bar.clone();
        close.connect_clicked(move |_| bar.set_reveal_child(false));
    }

    grid.set_activate_on_single_click(true);
    grid.connect_child_activated(move |_, child| {
        // the empty-page placeholder is a bare label without a title, so it opens nothing
        if let Some(l) = child.child().and_then(|c| c.last_child()).and_downcast::<Label>() {
            open_entry(&l.label(), anime);
        }
    });
    let press = gtk::GestureClick::builder().button(3).build();
    let b2 = bar.clone();
    press.connect_pressed(move |_, _, _, _| b2.set_reveal_child(true));
    grid.add_controller(press);
    let lp = gtk::GestureLongPress::new();
    let b3 = bar.clone();
    lp.connect_pressed(move |_, _, _| b3.set_reveal_child(true));
    grid.add_controller(lp);

    body.append(&scrolled(&grid));
    body.append(&bar);
    tag(&body, id);

    let search_btn = gtk::ToggleButton::builder().icon_name("system-search-symbolic").tooltip_text("Search").build();
    search_btn.bind_property("active", &search, "search-mode-enabled").bidirectional().build();
    let filter = icon_button("view-filter-symbolic", "Filter");
    let sheets = s.sheets.clone();
    filter.connect_clicked(move |_| {
        if let Some(sh) = sheets.first() {
            show_sheet(sh);
        }
    });
    let id2 = id.to_string();
    let menu = menu_button("view-more-symbolic", &s.menu, move |l| {
        if l == "Open random entry" {
            open_entry(&title_n(7), id2 == "anime_library");
        }
    });
    Built { content: body.upcast(), end: vec![search_btn.upcast(), filter.upcast(), menu.upcast()] }
}

// ---------- updates / history ----------

fn updates() -> Option<Built> {
    let s = screen("updates");
    let content = kind_tabs(crate::live::updates_content);
    let mut end = toolbar(&s);
    end.reverse();
    built(content, end)
}

fn history() -> Option<Built> {
    let (content, kinds) = kind_stack(crate::live::history_content);
    let clear = icon_button("edit-clear-all-symbolic", "Clear history");
    clear.connect_clicked(move |_| crate::live::confirm_clear_history(kinds.visible_child_name().as_deref() == Some("anime")));
    built(content, vec![clear.upcast()])
}

// ---------- more & friends ----------

fn more() -> Option<Built> {
    let s = screen("more");
    let col = column();
    let logo = GBox::new(O::Vertical, 6);
    logo.set_margin_bottom(12);
    logo.append(&gtk::Image::builder().icon_name("applications-multimedia-symbolic").pixel_size(64).build());
    logo.append(&Label::builder().label("Kurobayomi").css_classes(["title-2"]).build());
    col.append(&logo);
    // History has its own sidebar entry, so More drops the "moved tab" row (Aniyomi's nav-style rule)
    let mut groups = s.groups.clone();
    for g in &mut groups {
        g.items.retain(|it| !matches!(it, crate::spec::Item::Link { target, .. } if target == "history"));
    }
    groups_into(&col, &groups);
    tag(&col, "more");
    built(scrolled(&col), vec![])
}

fn downloads() -> Option<Built> {
    let s = screen("download_queue");
    let content = kind_tabs(|anime| {
        let ov = gtk::Overlay::new();
        let col = column();
        for (src, n) in [("Source A", 3), ("Source B", 2)] {
            col.append(&heading(&format!("{src} ({n})")));
            let l = boxed_list();
            for i in 1..=n {
                let unit = if anime { format!("Episode {i}") } else { format!("Chapter {i}") };
                let v = GBox::new(O::Vertical, 4);
                let r = list_row(
                    Some(gtk::Image::from_icon_name("list-drag-handle-symbolic").upcast()),
                    &title_n(i),
                    &unit,
                    vec![
                        Label::new(Some(if anime { "45%" } else { "12/30" })).upcast(),
                        menu_button("view-more-symbolic", &["Move to top", "Move series to top", "Move to bottom", "Move series to bottom", "Cancel", "Cancel all for this series"].map(String::from), |_| {}).upcast(),
                    ],
                );
                let inner = r.child().unwrap();
                r.set_child(None::<&gtk::Widget>);
                v.append(&inner);
                let pb = gtk::ProgressBar::new();
                pb.set_fraction(0.45);
                margins(&pb, 0);
                pb.set_margin_start(12);
                pb.set_margin_end(12);
                pb.set_margin_bottom(6);
                v.append(&pb);
                r.set_child(Some(&v));
                l.append(&r);
            }
            col.append(&l);
        }
        ov.set_child(Some(&scrolled(&col)));
        let (a, b) = if anime { ("Stop", "Continue") } else { ("Pause", "Resume") };
        let f = fab(&ov, a, "media-playback-pause-symbolic");
        f.connect_clicked(move |f| {
            let lbl = f.child().and_then(|c| c.last_child()).and_downcast::<Label>().unwrap();
            let paused = lbl.label() == b;
            lbl.set_label(if paused { a } else { b });
        });
        ov.upcast()
    });
    let sort = menu_button("view-sort-ascending-symbolic", &["By upload date: Newest", "By upload date: Oldest", "By number: Ascending", "By number: Descending"].map(String::from), |_| {});
    let menu = menu_button("view-more-symbolic", &s.menu, |_| {});
    built(content, vec![sort.upcast(), menu.upcast()])
}

fn categories() -> Option<Built> {
    let content = kind_tabs(crate::live::categories_content);
    built(content, vec![])
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
        "new_update" => ("software-update-available-symbolic", "v0.1.0", &["Download", "Not now"]),
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
        let text = if id == "crash" { "java.lang.RuntimeException: something went wrong" } else { "## Changelog\n- Improvements" };
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
