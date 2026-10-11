//! The library settings sheet (T-37a-038, T-37a-039): the Sort and Display tabs are live; the Filter tab is the
//! spec's. Each control writes its preference as soon as it changes, and the pure rules are in `sort_tab` and
//! `display_tab`.

use std::cell::Cell;
use std::rc::Rc;

use gtk::Orientation as O;
use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_domain::library::sort::{LibrarySort, SortType};
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::library::{LibraryDisplayMode, LibraryPrefs};

use crate::library::display_tab;
use crate::library::sort_tab;

/// What the sheet reads and writes.
pub struct Ctx {
    pub kind: EntryKind,
    pub prefs: Rc<LibraryPrefs>,
    pub has_tracker: bool,
    /// The sort the active page shows now.
    pub current_sort: LibrarySort,
    /// Writes a sort through `CategoryService::set_sort_mode`; a Random sort draws a new seed there.
    pub set_sort: Rc<dyn Fn(LibrarySort)>,
    /// The spec's Filter tab, built by the caller.
    pub filter_tab: gtk::Widget,
    /// The window width in pixels; it picks which of the portrait and landscape keys the columns slider edits.
    pub window_width: i32,
}

/// The tab the sheet opens on; the header menu has one entry per tab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    Filter,
    Sort,
    Display,
}

/// Opens the sheet over the main window, on `tab`.
pub fn show(ctx: Ctx, tab: Tab) {
    let stack = gtk::Stack::new();
    let filter = ctx.filter_tab.clone();
    stack.add_titled(&filter, Some("filter"), &tr(MsgKey::action_filter));
    stack.add_titled(&sort_tab(&ctx), Some("sort"), &tr(MsgKey::action_sort));
    stack.add_titled(&display_tab(&ctx), Some("display"), &tr(MsgKey::action_display));
    stack.set_visible_child_name(match tab {
        Tab::Filter => "filter",
        Tab::Sort => "sort",
        Tab::Display => "display",
    });
    let switcher = gtk::StackSwitcher::builder().stack(&stack).build();
    crate::ui::modal(switcher.upcast_ref(), &stack, 520, 620).present();
}

fn page(content: &impl IsA<gtk::Widget>) -> gtk::Widget {
    let col = crate::ui::column();
    col.append(content);
    crate::ui::scrolled(&col).upcast()
}

/// One row per sort type; the active row shows its direction arrow. A tap flips or moves the sort (`sort_tab::tap`)
/// and writes it at once.
pub fn sort_tab(ctx: &Ctx) -> gtk::Widget {
    let list = crate::ui::boxed_list();
    let current = Rc::new(Cell::new(ctx.current_sort));
    let kind = ctx.kind;
    let mut marks: Vec<(SortType, gtk::Image)> = Vec::new();
    for ty in sort_tab::rows(kind, ctx.has_tracker) {
        let mark = gtk::Image::new();
        let row = crate::ui::row(&tr(sort_tab::label_key(kind, ty)), None, Some(mark.clone().upcast()));
        row.set_activatable(true);
        list.append(&row);
        marks.push((ty, mark));
    }
    let marks = Rc::new(marks);
    let refresh = {
        let (current, marks) = (current.clone(), marks.clone());
        move || {
            let now = current.get();
            for (ty, mark) in marks.iter() {
                let icon = (now.ty == *ty).then(|| sort_tab::direction_icon(now)).flatten();
                mark.set_icon_name(icon);
                mark.set_visible(icon.is_some());
            }
        }
    };
    refresh();
    let set_sort = ctx.set_sort.clone();
    let types: Vec<SortType> = marks.iter().map(|(ty, _)| *ty).collect();
    list.connect_row_activated(move |_, row| {
        let Some(tapped) = types.get(row.index() as usize).copied() else { return };
        let (next, _reseed) = sort_tab::tap(current.get(), tapped);
        current.set(next);
        refresh();
        set_sort(next);
    });
    page(&list)
}

/// Four mode chips, the columns slider (0 is Auto), the five overlay switches and the two tab switches.
pub fn display_tab(ctx: &Ctx) -> gtk::Widget {
    let col = gtk::Box::new(O::Vertical, 12);
    let prefs = ctx.prefs.clone();

    let chips = gtk::Box::new(O::Horizontal, 0);
    chips.add_css_class("linked");
    chips.set_halign(gtk::Align::Start);
    let mut first: Option<gtk::ToggleButton> = None;
    let active = prefs.display_mode.get();
    // the slider's title follows the mode: list size in list mode, items per row in the grids (T-37a-039)
    let slider_title = crate::ui::heading(&tr(display_tab::slider_label(active)));
    for mode in display_tab::MODE_CHIPS {
        let chip = gtk::ToggleButton::with_label(&tr(display_tab::chip_key(mode)));
        match &first {
            Some(f) => chip.set_group(Some(f)),
            None => first = Some(chip.clone()),
        }
        chip.set_active(mode == active);
        let (prefs, slider_title) = (prefs.clone(), slider_title.clone());
        chip.connect_toggled(move |c| {
            write_mode(&prefs, c.is_active().then_some(mode));
            if c.is_active() {
                slider_title.set_text(&tr(display_tab::slider_label(mode)));
            }
        });
        chips.append(&chip);
    }
    col.append(&crate::ui::heading(&tr(MsgKey::action_display_mode)));
    col.append(&chips);

    col.append(&slider_title);
    let pref = display_tab::columns_pref(&ctx.prefs, ctx.kind, ctx.window_width).clone();
    let range = display_tab::SLIDER_RANGE;
    let slider = gtk::Scale::with_range(O::Horizontal, f64::from(*range.start()), f64::from(*range.end()), 1.0);
    slider.set_digits(0);
    slider.set_draw_value(true);
    slider.set_value_pos(gtk::PositionType::Right);
    slider.set_format_value_func(|_, value| display_tab::slider_value_text(value as i32));
    slider.set_value(f64::from(pref.get()));
    slider.connect_value_changed(move |s| {
        if let Err(e) = pref.set(s.value().round() as i32) {
            tracing::warn!(error = %e, "saving the library columns failed");
        }
    });
    col.append(&slider);

    for (heading, toggles) in [(MsgKey::overlay_header, &display_tab::Toggle::OVERLAY[..]), (MsgKey::tabs_header, &display_tab::Toggle::TABS[..])] {
        col.append(&crate::ui::heading(&tr(heading)));
        let list = crate::ui::boxed_list();
        for toggle in toggles.iter().copied() {
            let switch = gtk::Switch::new();
            switch.set_active(toggle.pref(&ctx.prefs).get());
            let prefs = ctx.prefs.clone();
            switch.connect_active_notify(move |s| {
                if let Err(e) = toggle.pref(&prefs).set(s.is_active()) {
                    tracing::warn!(error = %e, ?toggle, "saving a library display switch failed");
                }
            });
            list.append(&crate::ui::row(&tr(toggle.label_key(ctx.kind)), None, Some(switch.upcast())));
        }
        col.append(&list);
    }
    page(&col)
}

/// A chip going off does nothing: its group neighbour going on carries the new mode.
fn write_mode(prefs: &LibraryPrefs, mode: Option<LibraryDisplayMode>) {
    if let Some(mode) = mode {
        kuro_domain::category::service::set_display_mode(prefs, mode);
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    fn ctx(kind: EntryKind, written: Rc<Cell<Option<LibrarySort>>>) -> Ctx {
        Ctx {
            kind,
            prefs: Rc::new(LibraryPrefs::new(PrefStore::in_memory())),
            has_tracker: false,
            current_sort: LibrarySort::DEFAULT,
            set_sort: Rc::new(move |s| written.set(Some(s))),
            filter_tab: gtk::Box::new(O::Vertical, 0).upcast(),
            window_width: 1180,
        }
    }

    fn rows_of(widget: &gtk::Widget) -> Vec<gtk::ListBox> {
        fn walk(w: &gtk::Widget, out: &mut Vec<gtk::ListBox>) {
            if let Some(l) = w.downcast_ref::<gtk::ListBox>() {
                out.push(l.clone());
            }
            let mut c = w.first_child();
            while let Some(child) = c {
                walk(&child, out);
                c = child.next_sibling();
            }
        }
        let mut out = Vec::new();
        walk(widget, &mut out);
        out
    }

    /// Both widget checks run in this one test, on the shared GTK thread of `crate::testing`.
    #[test]
    fn sheet_widgets_smoke() {
        crate::testing::with_display(|| {
            a_sort_tap_flips_the_same_row_and_writes_at_once();
            the_display_tab_writes_every_control_immediately();
        });
    }

    fn a_sort_tap_flips_the_same_row_and_writes_at_once() {
        let written = Rc::new(Cell::new(None));
        let tab = sort_tab(&ctx(EntryKind::Manga, written.clone()));
        let list = rows_of(&tab).into_iter().next().expect("the sort list");
        let first = list.row_at_index(0).expect("Alphabetically row");
        list.emit_by_name::<()>("row-activated", &[&first]);
        let flipped = written.get().expect("the tap wrote a sort");
        assert_eq!(flipped.ty, SortType::Alphabetical);
        assert!(!flipped.is_ascending(), "the same row flips the direction");
        let date = list.row_at_index(7).expect("Date added row");
        list.emit_by_name::<()>("row-activated", &[&date]);
        let moved = written.get().expect("second write");
        assert_eq!((moved.ty, moved.is_ascending()), (SortType::DateAdded, false), "another row keeps the direction");
    }

    fn the_display_tab_writes_every_control_immediately() {
        let c = ctx(EntryKind::Anime, Rc::new(Cell::new(None)));
        let prefs = c.prefs.clone();
        let tab = display_tab(&c);
        let switches = rows_of(&tab).into_iter().next().expect("switch list");
        let before = prefs.download_badge.get();
        let row = switches.row_at_index(0).expect("first switch row");
        let switch = find::<gtk::Switch>(row.upcast_ref()).expect("a switch");
        switch.set_active(!before);
        assert_eq!(prefs.download_badge.get(), !before);
        let slider = find::<gtk::Scale>(&tab).expect("columns slider");
        slider.set_value(4.0);
        assert_eq!(prefs.anime_landscape_columns.get(), 4);
        let chips: Vec<gtk::ToggleButton> = all::<gtk::ToggleButton>(&tab);
        assert_eq!(chips.len(), 4, "four mode chips");
        chips[3].set_active(true);
        assert_eq!(prefs.display_mode.get(), LibraryDisplayMode::List);
    }

    fn all<T: IsA<gtk::Widget>>(w: &gtk::Widget) -> Vec<T> {
        let mut out = Vec::new();
        if let Some(t) = w.downcast_ref::<T>() {
            out.push(t.clone());
        }
        let mut c = w.first_child();
        while let Some(child) = c {
            out.extend(all::<T>(&child));
            c = child.next_sibling();
        }
        out
    }

    fn find<T: IsA<gtk::Widget>>(w: &gtk::Widget) -> Option<T> {
        all::<T>(w).into_iter().next()
    }
}
