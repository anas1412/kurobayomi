// Generic building blocks: navigation stack, spec item rows, sheets, dialogs, generic screens.
use crate::spec::{value_str, Dialog, Group, Item, Screen, Sheet, Spec};
use gtk::prelude::*;
use gtk::{glib, Align, Box as GBox, Button, Label, ListBox, Orientation as O};
use std::cell::OnceCell;
use std::rc::Rc;

thread_local! {
    pub static SPEC: Spec = Spec::load();
    static NAV: OnceCell<(gtk::Stack, gtk::ApplicationWindow)> = const { OnceCell::new() };
}

pub fn with_spec<R>(f: impl FnOnce(&Spec) -> R) -> R {
    SPEC.with(f)
}

pub fn screen(id: &str) -> Screen {
    with_spec(|s| s.screen(id).cloned()).unwrap_or_else(|| Screen { id: id.into(), title: id.into(), ..Default::default() })
}

// ---------- navigation ----------

pub fn nav_init(stack: &gtk::Stack, win: &gtk::ApplicationWindow) {
    NAV.with(|n| n.set((stack.clone(), win.clone())).ok());
}

fn nav() -> (gtk::Stack, gtk::ApplicationWindow) {
    NAV.with(|n| n.get().cloned().expect("nav not initialised"))
}

pub fn window() -> gtk::ApplicationWindow {
    nav().1
}

pub fn push(page: &impl IsA<gtk::Widget>) {
    let stack = nav().0;
    stack.add_child(page);
    crate::shell::transitions::apply(&stack, crate::shell::transitions::Direction::Push);
    stack.set_visible_child(page);
}

pub fn pop() {
    let stack = nav().0;
    let Some(cur) = stack.visible_child() else { return };
    let Some(prev) = cur.prev_sibling() else { return };
    crate::shell::transitions::apply(&stack, crate::shell::transitions::Direction::Pop);
    stack.set_visible_child(&prev);
    glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || stack.remove(&cur));
}

/// The widget names of the stacked pages from the root up, and the index of the page on show. A page that was just
/// popped stays in the list for a moment, so the index, not the length, says how deep the user is.
pub fn page_stack() -> (Vec<String>, usize) {
    let stack = nav().0;
    let mut names = Vec::new();
    let mut shown = 0;
    let visible = stack.visible_child();
    let mut child = stack.first_child();
    while let Some(c) = child {
        if visible.as_ref() == Some(&c) {
            shown = names.len();
        }
        names.push(c.widget_name().to_string());
        child = c.next_sibling();
    }
    (names, shown)
}

/// Pops pages until `stop` accepts the page on top, or only the root is left (Voyager `popUntil`). `stop` is asked
/// about the visible page first, so a page that already satisfies it is not popped.
pub fn pop_until(stop: impl Fn(&gtk::Widget) -> bool) {
    while let Some(cur) = nav().0.visible_child() {
        if stop(&cur) || cur.prev_sibling().is_none() {
            break;
        }
        pop();
    }
}

/// Drop every pushed page and show the root (sidebar destination clicked).
pub fn pop_to_root() {
    let stack = nav().0;
    let Some(root) = stack.first_child() else { return };
    crate::shell::transitions::apply(&stack, crate::shell::transitions::Direction::Pop);
    stack.set_visible_child(&root);
    while let Some(w) = root.next_sibling() {
        stack.remove(&w);
    }
}

/// Split the window-button layout like GNOME split headers: sidebar gets the left part,
/// content headers the right part.
pub fn decoration(sidebar: bool) -> String {
    let layout = gtk::Settings::default().and_then(|s| s.gtk_decoration_layout()).map(|l| l.to_string()).unwrap_or_default();
    let (left, right) = layout.split_once(':').unwrap_or(("", &layout));
    if sidebar { format!("{left}:") } else { format!(":{right}") }
}

/// A clicked header button keeps keyboard focus, and some GTK themes paint a focused header button like a pressed one, so it looks stuck.
pub fn no_sticky_focus(header: &impl IsA<gtk::Widget>) {
    let mut child = header.as_ref().first_child();
    while let Some(w) = child {
        if w.is::<Button>() || w.is::<gtk::MenuButton>() {
            w.set_focus_on_click(false);
        }
        no_sticky_focus(&w);
        child = w.next_sibling();
    }
}

/// A navigation page: its own header bar (title, optional back button, end widgets) above `content`.
pub fn page(title: &str, content: &impl IsA<gtk::Widget>, end: Vec<gtk::Widget>, back: bool) -> GBox {
    let header = gtk::HeaderBar::new();
    header.set_decoration_layout(Some(&decoration(false)));
    header.set_title_widget(Some(&Label::builder().label(title).css_classes(["title"]).build()));
    if back {
        let b = Button::from_icon_name("go-previous-symbolic");
        b.set_tooltip_text(Some("Back"));
        b.connect_clicked(|_| pop());
        header.pack_start(&b);
    }
    for w in end.iter().rev() {
        header.pack_end(w);
    }
    no_sticky_focus(&header);
    let b = GBox::new(O::Vertical, 0);
    b.append(&header);
    content.set_vexpand(true);
    b.append(content);
    b
}

/// Open a screen by id: hand-built if one exists, otherwise rendered from its spec.
pub fn open(id: &str) {
    if with_spec(|s| s.nav.iter().any(|n| n.id == id)) {
        // sidebar destinations switch tabs instead of stacking a second copy
        pop_to_root();
        if let Some(tabs) = nav().0.first_child().and_downcast::<gtk::Stack>() {
            tabs.set_visible_child_name(id);
        }
        return;
    }
    push(&crate::screens::build(id));
}

// ---------- layout helpers ----------

pub fn scrolled(child: &impl IsA<gtk::Widget>) -> gtk::ScrolledWindow {
    let child = child.as_ref();
    let child = if child.has_css_class("column") { clamp(child, 640).upcast() } else { child.clone() };
    gtk::ScrolledWindow::builder().child(&child).vexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build()
}

pub use crate::clamp::clamp;

pub fn margins(w: &impl IsA<gtk::Widget>, m: i32) {
    w.set_margin_top(m);
    w.set_margin_bottom(m);
    w.set_margin_start(m);
    w.set_margin_end(m);
}

/// Centered column with a max-ish width, like AdwClamp.
pub fn column() -> GBox {
    let b = GBox::new(O::Vertical, 12);
    margins(&b, 18);
    b.add_css_class("column");
    b
}

pub fn heading(text: &str) -> Label {
    Label::builder().label(text).xalign(0.0).css_classes(["heading"]).margin_top(6).build()
}

pub fn dim(text: &str) -> Label {
    Label::builder().label(text).xalign(0.0).wrap(true).css_classes(["dim-label", "caption"]).build()
}

pub fn boxed_list() -> ListBox {
    let l = ListBox::new();
    l.set_selection_mode(gtk::SelectionMode::None);
    l.add_css_class("boxed-list");
    l
}

pub fn icon_button(icon: &str, tip: &str) -> Button {
    let b = Button::from_icon_name(if icon.is_empty() { "applications-system-symbolic" } else { icon });
    b.set_tooltip_text(Some(tip));
    b.update_property(&[gtk::accessible::Property::Label(tip)]);
    b
}

/// Title/subtitle row with an optional trailing widget.
pub fn row(title: &str, subtitle: Option<&str>, suffix: Option<gtk::Widget>) -> gtk::ListBoxRow {
    let h = GBox::new(O::Horizontal, 12);
    margins(&h, 10);
    h.set_margin_start(12);
    let v = GBox::new(O::Vertical, 2);
    v.set_hexpand(true);
    v.set_valign(Align::Center);
    v.append(&Label::builder().label(title).xalign(0.0).wrap(true).build());
    if let Some(s) = subtitle.filter(|s| !s.is_empty()) {
        v.append(&dim(s));
    }
    h.append(&v);
    if let Some(s) = suffix {
        s.set_valign(Align::Center);
        h.append(&s);
    }
    gtk::ListBoxRow::builder().child(&h).build()
}

pub fn status(icon: &str, text: &str) -> GBox {
    let b = GBox::new(O::Vertical, 12);
    b.set_valign(Align::Center);
    b.set_halign(Align::Center);
    b.set_vexpand(true);
    b.append(&gtk::Image::builder().icon_name(icon).pixel_size(96).css_classes(["dim-label"]).build());
    b.append(&Label::builder().label(text).wrap(true).justify(gtk::Justification::Center).css_classes(["title-3", "dim-label"]).build());
    b
}

// ponytail: grey frame stands in for cover art until there is an image loader
pub fn cover(w: i32, h: i32) -> gtk::Frame {
    let f = gtk::Frame::new(None);
    f.set_size_request(w, h);
    f.set_child(Some(&gtk::Image::builder().icon_name("image-x-generic-symbolic").pixel_size(w / 3).css_classes(["dim-label"]).build()));
    f
}

pub fn linked(labels: &[String], active: usize) -> GBox {
    let b = GBox::new(O::Horizontal, 0);
    b.add_css_class("linked");
    let mut first: Option<gtk::ToggleButton> = None;
    for (i, l) in labels.iter().enumerate() {
        let t = gtk::ToggleButton::with_label(l);
        if let Some(f) = &first {
            t.set_group(Some(f));
        } else {
            first = Some(t.clone());
        }
        t.set_active(i == active);
        b.append(&t);
    }
    b
}

/// Popover menu of plain buttons; clicking one runs `on(label)`.
pub fn menu_button(icon: &str, entries: &[String], on: impl Fn(&str) + Clone + 'static) -> gtk::MenuButton {
    let pop = gtk::Popover::new();
    let col = GBox::new(O::Vertical, 0);
    for e in entries {
        let b = Button::builder().label(e).css_classes(["flat"]).build();
        b.child().and_downcast::<Label>().map(|l| l.set_xalign(0.0));
        let (on, e, pop) = (on.clone(), e.clone(), pop.clone());
        b.connect_clicked(move |_| {
            pop.popdown();
            on(&e);
        });
        col.append(&b);
    }
    pop.set_child(Some(&col));
    gtk::MenuButton::builder().icon_name(icon).popover(&pop).tooltip_text("More").build()
}

// ---------- spec items ----------

pub fn item_row(item: &Item) -> Option<gtk::ListBoxRow> {
    Some(match item {
        Item::Switch { title, subtitle, default } => {
            let s = gtk::Switch::new();
            s.set_active(default.as_bool().unwrap_or(false));
            // the More screen's incognito switch is the real preference (T-01a-020)
            if title == "Incognito mode" && subtitle.as_deref() == Some("Pauses your history") {
                crate::incognito::bind_switch(&s);
            }
            let r = row(title, subtitle.as_deref(), Some(s.clone().upcast()));
            r.set_activatable(true);
            r.connect_activate(move |_| s.set_active(!s.is_active()));
            r
        }
        Item::List { title, subtitle, options, default } => {
            let opts: Vec<&str> = options.iter().map(String::as_str).collect();
            let d = gtk::DropDown::from_strings(&opts);
            let def = value_str(default);
            if let Some(i) = options.iter().position(|o| *o == def) {
                d.set_selected(i as u32);
            }
            row(title, subtitle.as_deref(), Some(d.upcast()))
        }
        Item::Multi { title, subtitle, options, default } => {
            let defaults: Vec<String> = default.as_array().map(|a| a.iter().map(value_str).collect()).unwrap_or_default();
            let col = GBox::new(O::Vertical, 2);
            margins(&col, 6);
            for o in options {
                col.append(&gtk::CheckButton::builder().label(o).active(defaults.contains(o)).build());
            }
            let pop = gtk::Popover::builder().child(&scrolled_max(&col, 360)).build();
            let mb = gtk::MenuButton::builder().label(format!("{} selected", defaults.len())).popover(&pop).build();
            row(title, subtitle.as_deref(), Some(mb.upcast()))
        }
        Item::Slider { title, min, max, step, default, unit } => {
            let (min, max) = (min.unwrap_or(0.0), max.unwrap_or(100.0));
            let step = step.unwrap_or(1.0).max(f64::EPSILON);
            // whole-number ranges use the integer slider (T-49b-020)
            let whole = step == 1.0 && min.fract() == 0.0 && max.fract() == 0.0;
            let sc = if whole {
                crate::widgets::int_slider::IntSlider::new(min as i64, max as i64, default.as_f64().unwrap_or(min) as i64).scale
            } else {
                gtk::Scale::with_range(O::Horizontal, min, max, step)
            };
            sc.set_value(default.as_f64().unwrap_or(min));
            sc.set_width_request(220);
            sc.set_draw_value(true);
            sc.set_digits(if step < 1.0 { 2 } else { 0 });
            row(title, unit.as_deref(), Some(sc.upcast()))
        }
        Item::Text { title, subtitle, default } => {
            let e = gtk::Entry::builder().text(value_str(default)).width_chars(18).build();
            row(title, subtitle.as_deref(), Some(e.upcast()))
        }
        Item::Tristate { title } => {
            // off → include (checked) → exclude (dash), like Aniyomi's TriStateItem
            let t = crate::tri_check::tri_check(kuro_prefs::checkbox::TriState::None(()), |_| {});
            let r = row(title, None, Some(t.widget().clone().upcast()));
            r.set_activatable(true);
            r.connect_activate(move |_| t.activate());
            r
        }
        Item::Sort { title, options } => {
            let opts: Vec<&str> = options.iter().map(String::as_str).collect();
            let h = GBox::new(O::Horizontal, 6);
            h.append(&gtk::DropDown::from_strings(&opts));
            let dir = Button::from_icon_name("view-sort-ascending-symbolic");
            dir.set_tooltip_text(Some("Ascending / descending"));
            dir.connect_clicked(|b| {
                let up = b.icon_name().as_deref() == Some("view-sort-ascending-symbolic");
                b.set_icon_name(if up { "view-sort-descending-symbolic" } else { "view-sort-ascending-symbolic" });
            });
            h.append(&dir);
            row(title, None, Some(h.upcast()))
        }
        Item::Chips { title, options, default } => {
            let def = value_str(default);
            let active = options.iter().position(|o| *o == def).unwrap_or(0);
            if options.len() <= 4 {
                return Some(row(title, None, Some(linked(options, active).upcast())));
            }
            // long chip lists (e.g. the 19 app themes) wrap under the title instead of widening the window
            let flow = gtk::FlowBox::builder().selection_mode(gtk::SelectionMode::None).max_children_per_line(8).column_spacing(4).row_spacing(4).build();
            let mut first: Option<gtk::ToggleButton> = None;
            for (i, o) in options.iter().enumerate() {
                let t = gtk::ToggleButton::builder().label(o).active(i == active).build();
                match &first { Some(f) => t.set_group(Some(f)), None => first = Some(t.clone()) }
                flow.insert(&t, -1);
            }
            let v = GBox::new(O::Vertical, 8);
            margins(&v, 10);
            v.append(&Label::builder().label(title).xalign(0.0).build());
            v.append(&flow);
            gtk::ListBoxRow::builder().child(&v).activatable(false).build()
        }
        Item::Link { title, subtitle, target } => {
            let r = row(title, subtitle.as_deref(), Some(gtk::Image::from_icon_name("go-next-symbolic").upcast()));
            r.set_activatable(true);
            let t = target.clone();
            r.connect_activate(move |_| open(&t));
            r
        }
        Item::Action { title, subtitle } => {
            if crate::app_update::hides_row(title) {
                return None;
            }
            let r = row(title, subtitle.as_deref(), None);
            r.set_activatable(true);
            let t = title.clone();
            r.connect_activate(move |r| {
                if t == "Clear WebView data" {
                    crate::settings::clear_webview_data();
                    return;
                }
                // an action whose title names a dialog/sheet/screen of the current spec opens it
                let owner = owner_screen(r.upcast_ref());
                if let Some(id) = owner {
                    // the onboarding's Restore backup finishes onboarding and opens Data and storage
                    if crate::pref_model::onboarding::on_action(&id, &t) || crate::app_update::on_action(&id, &t) {
                        return;
                    }
                    resolve(&id, &t);
                }
            });
            r
        }
        Item::Info { title } => {
            let h = GBox::new(O::Horizontal, 8);
            margins(&h, 10);
            h.append(&gtk::Image::from_icon_name("dialog-information-symbolic"));
            let l = dim(title);
            l.set_hexpand(true);
            h.append(&l);
            gtk::ListBoxRow::builder().child(&h).activatable(false).build()
        }
        Item::Unknown => return None,
    })
}

fn scrolled_max(child: &impl IsA<gtk::Widget>, h: i32) -> gtk::ScrolledWindow {
    gtk::ScrolledWindow::builder().child(child).max_content_height(h).propagate_natural_height(true).hscrollbar_policy(gtk::PolicyType::Never).build()
}

pub fn groups_into(col: &GBox, groups: &[Group]) {
    for g in groups {
        if let Some(t) = g.title.as_deref().filter(|t| !t.is_empty()) {
            col.append(&heading(t));
        }
        let l = boxed_list();
        for it in &g.items {
            if let Some(r) = item_row(it) {
                l.append(&r);
            }
        }
        if l.first_child().is_some() {
            col.append(&l);
        }
    }
}

// ---------- sheets & dialogs ----------

pub fn modal(title: &gtk::Widget, content: &impl IsA<gtk::Widget>, w: i32, h: i32) -> gtk::Window {
    let header = gtk::HeaderBar::new();
    header.set_title_widget(Some(title));
    let win = gtk::Window::builder()
        .transient_for(&window())
        .modal(true)
        .default_width(w)
        .default_height(h)
        .titlebar(&header)
        .child(content)
        .build();
    win
}

pub fn show_sheet(sheet: &Sheet) {
    let stack = gtk::Stack::new();
    for t in &sheet.tabs {
        let col = column();
        groups_into(&col, &t.groups);
        stack.add_titled(&scrolled(&col), None, &t.title);
    }
    let title: gtk::Widget = if sheet.tabs.len() > 1 {
        gtk::StackSwitcher::builder().stack(&stack).build().upcast()
    } else {
        Label::builder().label(&sheet.title).css_classes(["title"]).build().upcast()
    };
    modal(&title, &stack, 520, 620).present();
}

pub fn show_dialog(d: &Dialog) {
    show_dialog_then(d, || {});
}

/// Shows a spec dialog. Its last button runs `on_ok` and then closes the dialog; the other buttons only close it.
pub fn show_dialog_then(d: &Dialog, on_ok: impl Fn() + 'static) {
    let col = GBox::new(O::Vertical, 12);
    margins(&col, 18);
    if let Some(t) = &d.text {
        col.append(&Label::builder().label(t).wrap(true).xalign(0.0).max_width_chars(50).build());
    }
    groups_into(&col, &d.groups);
    let buttons = GBox::new(O::Horizontal, 6);
    buttons.set_halign(Align::End);
    buttons.set_margin_top(6);
    col.append(&buttons);
    let win = modal(&Label::builder().label(&d.title).css_classes(["title"]).build().upcast(), &scrolled_max(&col, 520), 420, -1);
    let labels = if d.buttons.is_empty() { vec!["OK".to_string()] } else { d.buttons.clone() };
    let n = labels.len();
    let on_ok = Rc::new(on_ok);
    for (i, l) in labels.iter().enumerate() {
        let b = Button::with_label(l);
        let is_last = i + 1 == n;
        if is_last {
            b.add_css_class(if l.to_lowercase().contains("delete") || l.to_lowercase().contains("remove") { "destructive-action" } else { "suggested-action" });
        }
        let (w, on_ok) = (win.clone(), Rc::clone(&on_ok));
        b.connect_clicked(move |_| {
            if is_last {
                on_ok();
            }
            w.close();
        });
        buttons.append(&b);
    }
    win.present();
}

fn norm(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase()
}

/// Route a toolbar/menu/action label of screen `id` to the matching sheet, dialog or screen.
pub fn resolve(id: &str, label: &str) -> bool {
    let s = screen(id);
    let n = norm(label);
    if n.is_empty() {
        return false;
    }
    let hit = |a: &str, b: &str| !a.is_empty() && (norm(a) == n || norm(b) == n);
    if let Some(sh) = s.sheets.iter().find(|x| hit(&x.title, &x.id) || x.tabs.iter().any(|t| norm(&t.title) == n)) {
        show_sheet(sh);
        return true;
    }
    if let Some(d) = s.dialogs.iter().find(|x| hit(&x.title, &x.id)) {
        show_dialog(d);
        return true;
    }
    if n.contains("filter") || n.contains("settings") {
        if let Some(sh) = s.sheets.first() {
            show_sheet(sh);
            return true;
        }
    }
    let target = with_spec(|sp| {
        sp.screens.values().find(|x| x.id != id && (norm(&x.title) == n || norm(&x.id) == n)).map(|x| x.id.clone())
    });
    if let Some(t) = target {
        open(&t);
        return true;
    }
    false
}

/// Toolbar buttons + overflow menu for a screen, wired through `resolve`.
pub fn toolbar(s: &Screen) -> Vec<gtk::Widget> {
    let mut out: Vec<gtk::Widget> = vec![];
    for t in &s.toolbar {
        let b = icon_button(&t.icon, &t.label);
        let (id, l) = (s.id.clone(), t.label.clone());
        b.connect_clicked(move |_| {
            resolve(&id, &l);
        });
        out.push(b.upcast());
    }
    if !s.menu.is_empty() {
        let id = s.id.clone();
        out.push(menu_button("view-more-symbolic", &s.menu, move |l| {
            resolve(&id, l);
        }).upcast());
    }
    out
}

// Screen pages carry their id as the widget name so Action rows can find their owner.
pub fn owner_screen(w: &gtk::Widget) -> Option<String> {
    let mut cur = Some(w.clone());
    while let Some(c) = cur {
        let n = c.widget_name();
        if let Some(id) = n.strip_prefix("screen:") {
            return Some(id.to_string());
        }
        cur = c.parent();
    }
    None
}

pub fn tag(w: &impl IsA<gtk::Widget>, id: &str) {
    w.set_widget_name(&format!("screen:{id}"));
}

/// Render any spec screen: optional tab chips, groups, or empty state.
pub fn generic(s: &Screen) -> GBox {
    let col = column();
    if !s.tabs.is_empty() {
        let t = linked(&s.tabs, 0);
        t.set_halign(Align::Center);
        col.append(&t);
    }
    groups_into(&col, &s.groups);
    let content: gtk::Widget = if s.groups.is_empty() {
        let b = GBox::new(O::Vertical, 0);
        b.append(&col);
        b.append(&status("dialog-information-symbolic", s.empty.as_deref().unwrap_or(&s.title)));
        b.upcast()
    } else {
        scrolled(&col).upcast()
    };
    let p = page(&s.title, &content, toolbar(s), true);
    tag(&p, &s.id);
    p
}
