// Entry details, Browse tab and everything reached from it (sources, extensions, migration,
// global search, tracking, webview).
use crate::entry_model::{Control, EntryModel, Row, CHAPTER_SHEET, EPISODE_SHEET, SEASON_SHEET};
use crate::screens::{built, open_entry, open_media, title_n, Built};
use crate::spec::Item;
use crate::ui::*;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_prefs::tristate::TriState;
use std::rc::Rc;

pub fn body(id: &str) -> Option<Built> {
    match id {
        "browse" => browse(),
        "browse_source" => browse_source(id, false),
        "migrate_source_search" => browse_source(id, true),
        "migrate_season_select" => seasons(),
        "global_search" => global_search(id, false),
        "migrate_search" => global_search(id, true),
        "migrate_entries" => migrate_entries(),
        "tracking_sheet" => tracking(),
        "tracker_search" => tracker_search(),
        "webview" => webview(),
        "extension_repos" => crate::settings::body("settings_manga_extension_repos"),
        _ => crate::settings::body(id),
    }
}

fn dialog_of(id: &str, dialog: &str) {
    if let Some(d) = screen(id).dialogs.iter().find(|d| d.id == dialog) {
        show_dialog(d);
    }
}

fn sheet_of(id: &str, sheet: &str) {
    if let Some(s) = screen(id).sheets.iter().find(|s| s.id == sheet) {
        show_sheet(s);
    }
}

fn icon_label_button(icon: &str, label: &str) -> Button {
    let v = GBox::new(O::Vertical, 4);
    v.append(&gtk::Image::from_icon_name(icon));
    v.append(&Label::builder().label(label).css_classes(["caption"]).build());
    Button::builder().child(&v).css_classes(["flat"]).hexpand(true).build()
}

fn chip_row(labels: &[&str]) -> GBox {
    let b = GBox::new(O::Horizontal, 6);
    for l in labels {
        b.append(&gtk::ToggleButton::builder().label(*l).css_classes(["pill"]).build());
    }
    b
}

/// Grid of cover cards; `on` gets the clicked title.
fn cover_grid(n: usize, w: i32, on: impl Fn(&str) + 'static) -> gtk::FlowBox {
    let grid = gtk::FlowBox::builder()
        .homogeneous(true)
        .min_children_per_line(2)
        .max_children_per_line(12)
        .column_spacing(8)
        .row_spacing(12)
        .valign(Align::Start)
        .selection_mode(gtk::SelectionMode::None)
        .activate_on_single_click(true)
        .build();
    margins(&grid, 12);
    for i in 1..=n {
        let card = GBox::new(O::Vertical, 4);
        card.append(&cover(w, w * 10 / 7));
        card.append(&Label::builder().label(title_n(i)).xalign(0.0).wrap(true).lines(2).ellipsize(gtk::pango::EllipsizeMode::End).max_width_chars(14).build());
        grid.insert(&card, -1);
    }
    grid.connect_child_activated(move |_, c| {
        if let Some(l) = c.child().and_then(|c| c.last_child()).and_downcast::<Label>() {
            on(&l.label());
        }
    });
    grid
}

// ---------- entry details ----------

pub fn entry(title: &str, anime: bool) -> GBox {
    let id = if anime { "anime_entry_details" } else { "manga_entry_details" };
    let s = screen(id);
    let unit = if anime { "Episode" } else { "Chapter" };
    let model = match EntryModel::open(title, anime) {
        Ok(m) => m,
        Err(e) => return page(title, &status("dialog-error-symbolic", &format!("Could not open this entry: {e}")), vec![], true),
    };
    let ov = gtk::Overlay::new();
    let col = GBox::new(O::Vertical, 12);
    margins(&col, 18);

    // info box
    let head = GBox::new(O::Horizontal, 16);
    let c = cover(130, 190);
    let full = gtk::GestureClick::new();
    let fid = if anime { "full_images" } else { "full_cover" };
    full.connect_released(move |_, _, _, _| dialog_of(id, fid));
    c.add_controller(full);
    head.append(&c);
    let info = GBox::new(O::Vertical, 4);
    info.set_valign(Align::End);
    info.append(&Label::builder().label(title).xalign(0.0).wrap(true).css_classes(["title-1"]).build());
    for (icon, text) in [
        ("avatar-default-symbolic", if anime { "Studio" } else { "Author" }),
        ("content-loading-symbolic", "Ongoing"),
        ("network-server-symbolic", "Source A (EN)"),
    ] {
        let r = GBox::new(O::Horizontal, 6);
        r.append(&gtk::Image::from_icon_name(icon));
        r.append(&dim(text));
        info.append(&r);
    }
    head.append(&info);
    col.append(&head);

    // action row
    let actions = GBox::new(O::Horizontal, 0);
    let lib = icon_label_button("emblem-favorite-symbolic", "Add to library");
    lib.connect_clicked(|b| {
        let l = b.child().and_then(|c| c.last_child()).and_downcast::<Label>().unwrap();
        l.set_label(if l.label() == "In library" { "Add to library" } else { "In library" });
    });
    let next = icon_label_button("alarm-symbolic", "2 days");
    next.connect_clicked(move |_| dialog_of(id, "set_fetch_interval"));
    let track = icon_label_button("emblem-synchronizing-symbolic", "Tracking");
    track.connect_clicked(|_| open("tracking_sheet"));
    let web = icon_label_button("web-browser-symbolic", "WebView");
    web.connect_clicked(|_| open("webview"));
    for b in [&lib, &next, &track, &web] {
        actions.append(b);
    }
    col.append(&actions);

    // description + genres
    let desc = Label::builder()
        .label("Summary of the series goes here. It is collapsed to a few lines by default and expands when you press More.")
        .wrap(true)
        .xalign(0.0)
        .lines(3)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    col.append(&desc);
    let more = Button::builder().label("More").css_classes(["flat"]).halign(Align::Center).build();
    more.connect_clicked(move |b| {
        let open = b.label().as_deref() == Some("More");
        desc.set_lines(if open { -1 } else { 3 });
        desc.set_ellipsize(if open { gtk::pango::EllipsizeMode::None } else { gtk::pango::EllipsizeMode::End });
        b.set_label(if open { "Less" } else { "More" });
    });
    col.append(&more);
    let genres = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(10).build();
    for g in ["Action", "Adventure", "Comedy", "Fantasy"] {
        let gb = menu_button("", &["Search".into(), "Copy to clipboard".into()], |_| {});
        gb.set_label(g);
        gb.set_always_show_arrow(false);
        gb.add_css_class("pill");
        genres.insert(&gb, -1);
    }
    col.append(&genres);
    if anime {
        col.append(&dim("Next episode airs in 2d 4h"));
    }

    // chapter/episode list, rebuilt from the model on every published change
    let count = heading("");
    col.append(&count);
    let l = boxed_list();
    let sel = GBox::new(O::Horizontal, 6);
    sel.add_css_class("toolbar");
    sel.set_halign(Align::Center);
    let selbar = gtk::Revealer::builder().child(&sel).build();
    col.append(&l);
    for (icon, tip) in [
        ("bookmark-new-symbolic", "Bookmark"),
        ("object-select-symbolic", if anime { "Mark as watched" } else { "Mark as read" }),
        ("edit-undo-symbolic", if anime { "Mark as unwatched" } else { "Mark as unread" }),
        ("go-bottom-symbolic", "Mark previous as read"),
        ("folder-download-symbolic", "Download"),
    ] {
        sel.append(&icon_button(icon, tip));
    }
    let del = icon_button("user-trash-symbolic", "Delete");
    del.connect_clicked(move |_| dialog_of(id, "delete_items"));
    sel.append(&del);
    let close = icon_button("window-close-symbolic", "Clear selection");
    let sb = selbar.clone();
    close.connect_clicked(move |_| sb.set_reveal_child(false));
    sel.append(&close);

    ov.set_child(Some(&scrolled(&clamp(&col, 760))));
    let fab = Button::builder().label("Resume").css_classes(["suggested-action", "pill"]).halign(Align::End).valign(Align::End).build();
    margins(&fab, 18);
    let t = title.to_string();
    fab.connect_clicked(move |_| open_media(anime, &t, &format!("{unit} 20")));
    ov.add_overlay(&fab);

    let body = GBox::new(O::Vertical, 0);
    ov.set_vexpand(true);
    body.append(&ov);
    body.append(&selbar);

    let mut end: Vec<gtk::Widget> = vec![];
    let unit_l = unit.to_lowercase();
    end.push(menu_button("folder-download-symbolic", &[format!("Next {unit_l}"), format!("Next 5 {unit_l}s"), format!("Next 10 {unit_l}s"), format!("Next 25 {unit_l}s"), (if anime { "Unwatched" } else { "Unread" }).into()], |_| {}).upcast());
    let filter = icon_button("view-filter-symbolic", "Filter");
    let sh = if anime { EPISODE_SHEET } else { CHAPTER_SHEET };
    let m = model.clone();
    filter.connect_clicked(move |_| bound_sheet(&m, id, sh));
    end.push(filter.clone().upcast());
    end.push(
        menu_button("view-more-symbolic", &s.menu, move |l| match l {
            "Edit categories" => dialog_of(id, "set_categories"),
            "Migrate" => dialog_of(id, if anime { "migrate_anime" } else { "migrate_manga" }),
            "Settings" => open("source_preferences"),
            "Change intro length" => dialog_of(id, "skip_intro_length"),
            _ => {}
        })
        .upcast(),
    );

    // the first render is synchronous; later renders come from the live query
    let render = {
        let (l, selbar, count, filter, t) = (l.clone(), selbar.clone(), count.clone(), filter.clone(), title.to_string());
        move |m: &EntryModel| {
            fill_list(m, &l, &count, &selbar, id, &t, anime);
            if m.filtered() {
                filter.add_css_class("accent");
            } else {
                filter.remove_css_class("accent");
            }
        }
    };
    render(&model);
    model.follow(render);
    let p = page(title, &body, end, true);
    tag(&p, id);
    p
}

/// Rebuilds the item list from the model's rows: a row per item (activation opens the reader, right-click reveals the
/// selection bar, long-press on an episode opens its options), and a dimmed line per missing-count separator.
fn fill_list(m: &EntryModel, list: &gtk::ListBox, count: &Label, selbar: &gtk::Revealer, id: &'static str, title: &str, anime: bool) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let unit = if anime { "episode" } else { "chapter" };
    let mut items = 0;
    for r in m.rows() {
        match r {
            Row::Item { name, sub, read } => {
                items += 1;
                let dl = menu_button("folder-download-symbolic", &["Download".into(), "Delete".into()], |_| {});
                let r = row(&name, Some(&sub), Some(dl.upcast()));
                if read {
                    r.set_opacity(0.6);
                }
                r.set_activatable(true);
                let (t, n) = (title.to_string(), name.clone());
                r.connect_activate(move |_| open_media(anime, &t, &n));
                let rc = gtk::GestureClick::builder().button(3).build();
                let sb = selbar.clone();
                rc.connect_pressed(move |_, _, _, _| sb.set_reveal_child(true));
                r.add_controller(rc);
                if anime {
                    let lp = gtk::GestureLongPress::new();
                    lp.connect_pressed(move |_, _, _| dialog_of(id, "episode_options"));
                    r.add_controller(lp);
                }
                list.append(&r);
            }
            Row::Missing(n) => {
                let r = gtk::ListBoxRow::builder().activatable(false).child(&dim(&format!("Missing {n} {unit}s"))).build();
                list.append(&r);
            }
        }
    }
    count.set_label(&format!("{items} {unit}s"));
}

/// A settings sheet whose controls are bound to the entry model (T-07a-040). An item without a binding renders as in
/// the mockup. The season `Grid display mode` item (and its group heading) is shown as `Display`.
fn bound_sheet(model: &Rc<EntryModel>, screen_id: &str, sheet_id: &str) {
    let s = screen(screen_id);
    let Some(sheet) = s.sheets.iter().find(|x| x.id == sheet_id) else {
        return;
    };
    let stack = gtk::Stack::new();
    for t in &sheet.tabs {
        let col = column();
        for g in &t.groups {
            if let Some(h) = g.title.as_deref().filter(|h| !h.is_empty()) {
                col.append(&heading(relabel(sheet_id, h)));
            }
            let l = boxed_list();
            for it in &g.items {
                if let Some(r) = bound_row(model, sheet_id, it).or_else(|| item_row(it)) {
                    l.append(&r);
                }
            }
            if l.first_child().is_some() {
                col.append(&l);
            }
        }
        stack.add_titled(&scrolled(&col), None, &t.title);
    }
    let title: gtk::Widget = if sheet.tabs.len() > 1 {
        gtk::StackSwitcher::builder().stack(&stack).build().upcast()
    } else {
        Label::builder().label(&sheet.title).css_classes(["title"]).build().upcast()
    };
    modal(&title, &stack, 520, 620).present();
}

/// The label of a bound control. The season `Grid display mode` list is `Display` (`SEASON_DISPLAY_MODE`).
fn relabel<'a>(sheet: &str, text: &'a str) -> &'a str {
    if sheet == SEASON_SHEET && text == "Grid display mode" { "Display" } else { text }
}

/// The row of a bound item, or `None` when the item has no binding for this entry.
fn bound_row(model: &Rc<EntryModel>, sheet: &str, item: &Item) -> Option<gtk::ListBoxRow> {
    match item {
        Item::Tristate { title } => match model.control(sheet, title)? {
            Control::Tri(state) => Some(tri_row(model, sheet, title, state)),
            _ => None,
        },
        Item::Switch { title, .. } => match model.control(sheet, title)? {
            Control::Toggle(on) => Some(switch_row(model, sheet, title, on)),
            _ => None,
        },
        Item::Sort { title, options } => match model.control(sheet, title)? {
            Control::Choice(i) => Some(sort_row(model, sheet, title, options, i)),
            _ => None,
        },
        Item::List { title, options, .. } => match model.control(sheet, title)? {
            Control::Choice(i) => Some(list_row(model, sheet, title, options, i)),
            _ => None,
        },
        Item::Chips { title, options, .. } => match model.control(sheet, title)? {
            Control::Choice(i) => Some(chips_row(model, sheet, title, options, i)),
            _ => None,
        },
        Item::Slider { title, min, max, step, unit, .. } => match model.control(sheet, title)? {
            Control::Value(v) => {
                Some(slider_row(model, sheet, title, (min.unwrap_or(0.0), max.unwrap_or(100.0), step.unwrap_or(1.0)), unit.as_deref(), v))
            }
            _ => None,
        },
        _ => None,
    }
}

/// Tri-state control: disabled (empty), include (checked), exclude (dash). A click cycles the states through
/// `TriState::next` and writes the new state; the row keeps its old state when the write fails.
fn tri_row(model: &Rc<EntryModel>, sheet: &str, title: &str, state: TriState) -> gtk::ListBoxRow {
    let c = gtk::CheckButton::new();
    show_tri(&c, state);
    let r = row(relabel(sheet, title), None, Some(c.clone().upcast()));
    r.set_activatable(true);
    let (m, sheet, title) = (model.clone(), sheet.to_owned(), title.to_owned());
    let current = std::cell::Cell::new(state);
    r.connect_activate(move |_| {
        let next = current.get().next();
        if m.apply(&sheet, &title, Control::Tri(next)) {
            current.set(next);
            show_tri(&c, next);
        }
    });
    r
}

fn show_tri(c: &gtk::CheckButton, state: TriState) {
    c.set_active(state != TriState::DISABLED);
    c.set_inconsistent(state == TriState::ENABLED_NOT);
}

fn switch_row(model: &Rc<EntryModel>, sheet: &str, title: &str, on: bool) -> gtk::ListBoxRow {
    let s = gtk::Switch::new();
    s.set_active(on);
    let (m, sh, t) = (model.clone(), sheet.to_owned(), title.to_owned());
    s.connect_active_notify(move |s| {
        m.apply(&sh, &t, Control::Toggle(s.is_active()));
    });
    let r = row(relabel(sheet, title), None, Some(s.clone().upcast()));
    r.set_activatable(true);
    r.connect_activate(move |_| s.set_active(!s.is_active()));
    r
}

/// Sort control: the dropdown picks the sort (choosing the sort in use flips the direction); the arrow button flips
/// the direction of the sort in use.
fn sort_row(model: &Rc<EntryModel>, sheet: &str, title: &str, options: &[String], index: usize) -> gtk::ListBoxRow {
    let opts: Vec<&str> = options.iter().map(String::as_str).collect();
    let dd = gtk::DropDown::from_strings(&opts);
    dd.set_selected(index as u32);
    let dir = Button::from_icon_name(if model.sort_descending(sheet) { "view-sort-descending-symbolic" } else { "view-sort-ascending-symbolic" });
    dir.set_tooltip_text(Some("Ascending / descending"));
    let h = GBox::new(O::Horizontal, 6);
    h.append(&dd);
    h.append(&dir);
    let (m, s, t) = (model.clone(), sheet.to_owned(), title.to_owned());
    dd.connect_selected_notify(move |d| {
        m.apply(&s, &t, Control::Choice(d.selected() as usize));
    });
    let (m, s, t, d) = (model.clone(), sheet.to_owned(), title.to_owned(), dd.clone());
    dir.connect_clicked(move |b| {
        if m.apply(&s, &t, Control::Choice(d.selected() as usize)) {
            let up = b.icon_name().as_deref() == Some("view-sort-ascending-symbolic");
            b.set_icon_name(if up { "view-sort-descending-symbolic" } else { "view-sort-ascending-symbolic" });
        }
    });
    row(relabel(sheet, title), None, Some(h.upcast()))
}

/// Dropdown of options; choosing one writes its index.
fn list_row(model: &Rc<EntryModel>, sheet: &str, title: &str, options: &[String], index: usize) -> gtk::ListBoxRow {
    let opts: Vec<&str> = options.iter().map(String::as_str).collect();
    let dd = gtk::DropDown::from_strings(&opts);
    dd.set_selected(index as u32);
    let (m, s, t) = (model.clone(), sheet.to_owned(), title.to_owned());
    dd.connect_selected_notify(move |d| {
        m.apply(&s, &t, Control::Choice(d.selected() as usize));
    });
    row(relabel(sheet, title), None, Some(dd.upcast()))
}

/// Toggle group of options (grouped toggle buttons); the active one is the stored choice.
fn chips_row(model: &Rc<EntryModel>, sheet: &str, title: &str, options: &[String], index: usize) -> gtk::ListBoxRow {
    let b = GBox::new(O::Horizontal, 0);
    b.add_css_class("linked");
    let mut first: Option<gtk::ToggleButton> = None;
    for (i, o) in options.iter().enumerate() {
        let t = gtk::ToggleButton::with_label(o);
        match &first {
            Some(f) => t.set_group(Some(f)),
            None => first = Some(t.clone()),
        }
        t.set_active(i == index);
        let (m, s, name) = (model.clone(), sheet.to_owned(), title.to_owned());
        t.connect_toggled(move |t| {
            if t.is_active() {
                m.apply(&s, &name, Control::Choice(i));
            }
        });
        b.append(&t);
    }
    row(relabel(sheet, title), None, Some(b.upcast()))
}

fn slider_row(model: &Rc<EntryModel>, sheet: &str, title: &str, (min, max, step): (f64, f64, f64), unit: Option<&str>, value: i64) -> gtk::ListBoxRow {
    let sc = gtk::Scale::with_range(O::Horizontal, min, max, step.max(f64::EPSILON));
    sc.set_value(value as f64);
    sc.set_width_request(220);
    sc.set_draw_value(true);
    sc.set_digits(0);
    let (m, s, t) = (model.clone(), sheet.to_owned(), title.to_owned());
    sc.connect_value_changed(move |sc| {
        m.apply(&s, &t, Control::Value(sc.value().round() as i64));
    });
    row(relabel(sheet, title), unit, Some(sc.upcast()))
}

// ---------- browse ----------

pub fn browse() -> Option<Built> {
    let s = screen("browse");
    let stack = gtk::Stack::new();
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);
    for (i, t) in s.tabs.iter().enumerate() {
        let anime = i % 2 == 0;
        let w: gtk::Widget = match i {
            0 | 1 => sources(anime).upcast(),
            2 | 3 => extensions(anime).upcast(),
            _ => migrate_sources(anime).upcast(),
        };
        stack.add_titled(&w, None, t);
    }
    let b = GBox::new(O::Vertical, 6);
    let sw = gtk::StackSwitcher::builder().stack(&stack).halign(Align::Center).margin_top(6).margin_start(12).margin_end(12).build();
    // 6 tabs are wider than a narrow window: scroll sideways instead of forcing the window wide
    b.append(&gtk::ScrolledWindow::builder().child(&sw).vscrollbar_policy(gtk::PolicyType::Never).propagate_natural_width(true).build());
    stack.set_vexpand(true);
    b.append(&stack);
    tag(&b, "browse");

    let gs = icon_button("system-search-symbolic", "Global search");
    gs.connect_clicked(|_| open("global_search"));
    let filter = icon_button("view-filter-symbolic", "Filter");
    let st = stack.clone();
    filter.connect_clicked(move |_| {
        let ext = st.pages().n_items() > 0 && st.visible_child_name().is_some_and(|_| false);
        let _ = ext;
        open("sources_filter")
    });
    let menu = menu_button("view-more-symbolic", &s.menu, |l| match l {
        "Filter" => open("extension_filter"),
        "Extension repos" => open("settings_manga_extension_repos"),
        "Extension stores" => open("settings_anime_extension_stores"),
        _ => {}
    });
    built(b, vec![gs.upcast(), filter.upcast(), menu.upcast()])
}

fn sources(anime: bool) -> gtk::ScrolledWindow {
    let col = column();
    let local = if anime { "Local anime source" } else { "Local source" };
    for (section, names) in [("Last used", vec!["Source A"]), ("Pinned", vec!["Source B"]), ("English", vec!["Source A", "Source C", "Source D"]), ("Other", vec![local])] {
        col.append(&heading(section));
        let l = boxed_list();
        for n in names {
            let latest = Button::builder().label("Latest").css_classes(["flat"]).build();
            latest.connect_clicked(|_| open("browse_source"));
            let pin = gtk::ToggleButton::builder().icon_name("view-pin-symbolic").active(section == "Pinned").css_classes(["flat"]).build();
            let h = GBox::new(O::Horizontal, 6);
            h.append(&latest);
            h.append(&pin);
            let r = row(n, Some(if section == "Other" { "Other" } else { "English" }), Some(h.upcast()));
            r.child().and_downcast::<GBox>().unwrap().prepend(&gtk::Image::builder().icon_name("application-x-addon-symbolic").pixel_size(32).build());
            r.set_activatable(true);
            r.connect_activate(|_| open("browse_source"));
            let lp = gtk::GestureClick::builder().button(3).build();
            lp.connect_pressed(|_, _, _, _| dialog_of("browse", "source_options"));
            r.add_controller(lp);
            l.append(&r);
        }
        col.append(&l);
    }
    let _ = anime;
    scrolled(&col)
}

fn extensions(anime: bool) -> gtk::ScrolledWindow {
    let col = column();
    let search = gtk::SearchEntry::builder().placeholder_text("Search…").build();
    col.append(&search);
    let hdr = GBox::new(O::Horizontal, 6);
    let h = heading("Updates pending");
    h.set_hexpand(true);
    hdr.append(&h);
    hdr.append(&Button::builder().label("Update all").css_classes(["flat"]).build());
    col.append(&hdr);
    let kind = if anime { "Anime" } else { "Manga" };
    for (section, items) in [
        ("", vec![("Extension A", "1.4.13", "Update")]),
        ("Installed", vec![("Extension B", "1.4.2", "Settings"), ("Extension C", "1.4.7", "Untrusted")]),
        ("English", vec![("Extension D", "1.4.1", "Install"), ("Extension E", "1.4.3", "Install")]),
        ("Japanese", vec![("Extension F", "1.4.5", "Install")]),
    ] {
        if !section.is_empty() {
            col.append(&heading(section));
        }
        let l = boxed_list();
        for (n, v, act) in items {
            let b = Button::builder().label(act).build();
            match act {
                "Untrusted" => {
                    b.add_css_class("destructive-action");
                    b.connect_clicked(|_| dialog_of("browse", "extension_trust"));
                }
                "Settings" => {
                    b.connect_clicked(|_| open("extension_details"));
                }
                _ => {
                    b.connect_clicked(|b| {
                        let next = match b.label().as_deref() { Some("Install") | Some("Update") => "Installing", _ => "Settings" };
                        b.set_label(next);
                    });
                }
            }
            let r = row(&format!("{kind} {n}"), Some(&format!("{v} • EN")), Some(b.upcast()));
            r.child().and_downcast::<GBox>().unwrap().prepend(&gtk::Image::builder().icon_name("application-x-addon-symbolic").pixel_size(32).build());
            r.set_activatable(true);
            r.connect_activate(|_| open("extension_details"));
            l.append(&r);
        }
        col.append(&l);
    }
    scrolled(&col)
}

fn migrate_sources(anime: bool) -> gtk::ScrolledWindow {
    let col = column();
    let hdr = GBox::new(O::Horizontal, 6);
    let h = heading("Select a source to migrate from");
    h.set_hexpand(true);
    hdr.append(&h);
    hdr.append(&menu_button("view-sort-ascending-symbolic", &["Alphabetically".into(), "Total".into()], |_| {}));
    col.append(&hdr);
    let l = boxed_list();
    for (n, c) in [("Source A", 12), ("Source B", 4), ("Source C", 1)] {
        let r = row(n, Some("English"), Some(Label::builder().label(c.to_string()).css_classes(["accent"]).build().upcast()));
        r.set_activatable(true);
        r.connect_activate(|_| open("migrate_entries"));
        l.append(&r);
    }
    col.append(&l);
    let _ = anime;
    scrolled(&col)
}

fn browse_source(id: &str, migrating: bool) -> Option<Built> {
    let s = screen(id);
    let b = GBox::new(O::Vertical, 6);
    let search = gtk::SearchBar::new();
    let entry = gtk::SearchEntry::builder().placeholder_text("Search…").build();
    search.set_child(Some(&entry));
    search.connect_entry(&entry);
    b.append(&search);
    let chips = chip_row(&["Popular", "Latest", "Filter"]);
    margins(&chips, 8);
    if let Some(first) = chips.first_child().and_downcast::<gtk::ToggleButton>() {
        first.set_active(true);
    }
    if let Some(f) = chips.last_child().and_downcast::<gtk::ToggleButton>() {
        f.connect_clicked(|_| sheet_of("browse_source", "filter_sheet"));
    }
    b.append(&chips);
    b.append(&gtk::Separator::new(O::Horizontal));
    let grid = cover_grid(24, 130, move |t| {
        if migrating {
            dialog_of("migrate_search", "migrate_manga")
        } else {
            open_entry(t, false)
        }
    });
    b.append(&scrolled(&grid));
    tag(&b, id);

    let st = gtk::ToggleButton::builder().icon_name("system-search-symbolic").tooltip_text("Search").build();
    st.bind_property("active", &search, "search-mode-enabled").bidirectional().build();
    let mut end: Vec<gtk::Widget> = vec![st.upcast()];
    if !migrating {
        let dm = icon_button("view-grid-symbolic", "Display mode");
        dm.connect_clicked(|_| dialog_of("browse_source", "display_mode"));
        end.push(dm.upcast());
        end.push(
            menu_button("view-more-symbolic", &s.menu, |l| match l {
                "Open in WebView" => open("webview"),
                "Settings" => open("source_preferences"),
                _ => {}
            })
            .upcast(),
        );
    }
    built(b, end)
}

fn seasons() -> Option<Built> {
    let grid = cover_grid(6, 130, |_| dialog_of("migrate_search", "migrate_anime"));
    built(scrolled(&grid), vec![])
}

fn global_search(id: &str, migrating: bool) -> Option<Built> {
    let b = GBox::new(O::Vertical, 6);
    let entry = gtk::SearchEntry::builder().placeholder_text("Search…").margin_start(12).margin_end(12).margin_top(6).build();
    b.append(&entry);
    let progress = gtk::ProgressBar::new();
    progress.set_fraction(0.6);
    b.append(&progress);
    let chips = chip_row(&["Pinned", "All", "Has results"]);
    margins(&chips, 8);
    b.append(&chips);
    let col = GBox::new(O::Vertical, 12);
    margins(&col, 12);
    for (i, src) in ["Source A", "Source B", "Source C"].iter().enumerate() {
        let hdr = GBox::new(O::Horizontal, 6);
        let t = GBox::new(O::Vertical, 0);
        t.set_hexpand(true);
        let name = if migrating && i == 0 { format!("▶ {src}") } else { src.to_string() };
        t.append(&Label::builder().label(&name).xalign(0.0).css_classes(["heading"]).build());
        t.append(&dim("English"));
        hdr.append(&t);
        let go = icon_button("go-next-symbolic", "Open source");
        go.connect_clicked(move |_| open(if migrating { "migrate_source_search" } else { "browse_source" }));
        hdr.append(&go);
        col.append(&hdr);
        if i == 2 {
            col.append(&dim("No results found"));
            continue;
        }
        let row = GBox::new(O::Horizontal, 8);
        for n in 1..=8 {
            let card = GBox::new(O::Vertical, 4);
            card.append(&cover(96, 137));
            card.append(&Label::builder().label(title_n(n)).max_width_chars(12).ellipsize(gtk::pango::EllipsizeMode::End).build());
            let btn = Button::builder().child(&card).css_classes(["flat"]).build();
            btn.connect_clicked(move |_| {
                if migrating {
                    dialog_of("migrate_search", "migrate_manga")
                } else {
                    open_entry(&title_n(n), false)
                }
            });
            row.append(&btn);
        }
        col.append(&gtk::ScrolledWindow::builder().child(&row).vscrollbar_policy(gtk::PolicyType::Never).build());
    }
    b.append(&scrolled(&col));
    tag(&b, id);
    built(b, vec![])
}

fn migrate_entries() -> Option<Built> {
    let col = column();
    let l = boxed_list();
    for i in 1..=8 {
        let r = row(&title_n(i), None, None);
        r.child().and_downcast::<GBox>().unwrap().prepend(&cover(40, 57));
        r.set_activatable(true);
        r.connect_activate(|_| open("migrate_search"));
        l.append(&r);
    }
    col.append(&l);
    built(scrolled(&col), vec![])
}

// ---------- tracking ----------

fn tracking() -> Option<Built> {
    let id = "tracking_sheet";
    let s = screen(id);
    let col = column();
    for (name, bound) in [("AniList", true), ("MyAnimeList", false), ("Kitsu", false)] {
        let head = GBox::new(O::Horizontal, 12);
        let logo = gtk::Frame::new(None);
        logo.set_size_request(48, 48);
        logo.set_child(Some(&Label::builder().label(&name[..1]).css_classes(["title-2"]).build()));
        head.append(&logo);
        if bound {
            let t = Button::builder().label(title_n(1)).css_classes(["flat"]).hexpand(true).build();
            t.connect_clicked(|_| open("tracker_search"));
            head.append(&t);
            head.append(&menu_button("view-more-symbolic", &s.menu, move |l| {
                if l == "Remove" {
                    dialog_of(id, "remove_tracking")
                }
            }));
            col.append(&head);
            let grid = gtk::Grid::builder().column_homogeneous(true).css_classes(["card"]).build();
            for (i, (label, d)) in [("Watching", "status"), ("12 / 24", "chapters"), ("8.0", "score"), ("Start date", "start_date"), ("Finish date", "finish_date")].into_iter().enumerate() {
                let b = Button::builder().label(label).css_classes(["flat"]).build();
                b.connect_clicked(move |_| dialog_of(id, d));
                if i < 3 {
                    grid.attach(&b, i as i32 * 2, 0, 2, 1);
                } else {
                    grid.attach(&b, (i as i32 - 3) * 3, 1, 3, 1);
                }
            }
            col.append(&grid);
        } else {
            let add = Button::builder().label("Add tracking").css_classes(["flat"]).hexpand(true).build();
            add.connect_clicked(|_| open("tracker_search"));
            head.append(&add);
            col.append(&head);
        }
    }
    built(scrolled(&col), vec![])
}

fn tracker_search() -> Option<Built> {
    let b = GBox::new(O::Vertical, 6);
    let entry = gtk::SearchEntry::builder().text(title_n(1)).margin_start(12).margin_end(12).margin_top(6).build();
    b.append(&entry);
    let col = column();
    let l = gtk::ListBox::new();
    l.add_css_class("boxed-list");
    for i in 1..=4 {
        let h = GBox::new(O::Horizontal, 12);
        margins(&h, 12);
        h.append(&cover(67, 96));
        let v = GBox::new(O::Vertical, 2);
        v.append(&Label::builder().label(title_n(i)).xalign(0.0).css_classes(["heading"]).build());
        v.append(&dim("Author name"));
        for (k, val) in [("Type", "Manga"), ("Started", "2020-01-01"), ("Status", "Publishing"), ("Score", "8.4")] {
            v.append(&Label::builder().label(format!("{k}: {val}")).xalign(0.0).build());
        }
        h.append(&v);
        l.append(&h);
    }
    col.append(&l);
    b.append(&scrolled(&col));
    let bar = GBox::new(O::Horizontal, 6);
    margins(&bar, 8);
    bar.append(&Button::builder().label("Track").css_classes(["suggested-action"]).hexpand(true).build());
    bar.append(&icon_button("view-conceal-symbolic", "Track privately"));
    let rev = gtk::Revealer::builder().child(&bar).build();
    let r2 = rev.clone();
    l.connect_row_selected(move |_, r| r2.set_reveal_child(r.is_some()));
    b.append(&rev);
    built(b, vec![])
}

fn webview() -> Option<Built> {
    let s = screen("webview");
    let b = GBox::new(O::Vertical, 0);
    b.append(&dim("https://example.org/"));
    b.last_child().unwrap().set_margin_start(12);
    let p = gtk::ProgressBar::new();
    p.set_fraction(0.3);
    b.append(&p);
    // ponytail: no web engine linked; webkitgtk-6.0 would go here
    b.append(&status("web-browser-symbolic", "Web content"));
    let mut end = vec![icon_button("go-previous-symbolic", "Back").upcast::<gtk::Widget>(), icon_button("go-next-symbolic", "Forward").upcast()];
    end.push(menu_button("view-more-symbolic", &s.menu, |_| {}).upcast());
    built(b, end)
}

#[cfg(test)]
mod tests {
    use super::relabel;

    #[test]
    fn the_season_grid_list_is_labelled_display_and_other_labels_are_kept() {
        assert_eq!(relabel("season_settings", "Grid display mode"), "Display");
        assert_eq!(relabel("chapter_settings", "Grid display mode"), "Grid display mode");
        assert_eq!(relabel("season_settings", "Sort"), "Sort");
    }
}

