//! The Upcoming page of one kind (T-42b-027 to T-42b-035, T-42b-038): the calendar and the list of entries with an
//! update coming, under date headings with a count badge. On a content width of 720 px or more the calendar and the list
//! sit in a resizable [`gtk::Paned`]; below that the calendar is the first row of the list. A click on a day scrolls the
//! list to that day's heading. The page has no empty-state text and no toast (U§2.1, U§3).
//!
//! The grouping and the numbers come from [`model`], [`layout`] and [`calendar`]; the widgets here stay thin.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use chrono::{Local, NaiveDate, Weekday};
use gtk::gdk;
use gtk::glib;
#[cfg(test)]
use gtk::glib::translate::IntoGlib;
use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_domain::kind::EntryKind;
use kuro_i18n::{MsgKey, tr};
use kuro_images::art::{ArtKind, ArtRef, Owner};
use kuro_prefs::ui::UiPrefs;

use crate::data;
use crate::entry_core::cover_view::CoverView;
use crate::updates::download_host;
use crate::updates::prefs::week_start;
use crate::updates::upcoming::calendar::week_start_for;
use crate::updates::upcoming::calendar_view::{CalendarView, ensure_css};
use crate::updates::upcoming::layout::{self, Shape};
use crate::updates::upcoming::model::{self, Entry, Row, Upcoming};
use crate::updates::upcoming::{query, rows};

/// How often a shown page looks at the clock for a day change (E-C18).
const DAY_CHECK_SECONDS: u32 = 60;

thread_local! {
    /// The pages on the navigation stack; their widgets hold only weak references, so this keeps them alive until the
    /// page is destroyed.
    static PAGES: RefCell<Vec<Rc<UpcomingPage>>> = const { RefCell::new(Vec::new()) };
}

/// What the page reads from the app: the week start, the clock, the date preferences, and what a tap on an entry does.
/// The page takes it as a value so its layout and its scroll can run without the app data.
struct Env {
    first: Weekday,
    today: NaiveDate,
    relative: bool,
    date_pattern: String,
    locale: &'static str,
    open: Rc<dyn Fn(i64)>,
}

impl Env {
    /// The live environment of one kind. Needs the app data.
    fn current(kind: EntryKind) -> Self {
        let store = data::app().base.store().clone();
        let ui = UiPrefs::new(store.clone());
        let locale = kuro_i18n::current_locale().tag;
        Self {
            first: week_start_for(&week_start(&store).get(), locale),
            today: Local::now().date_naive(),
            relative: ui.relative_time.get(),
            date_pattern: ui.date_format.get(),
            locale,
            open: Rc::new(move |id| open_entry(kind, id)),
        }
    }
}

/// The loaded entries and the day they were loaded for.
struct Loaded {
    up: Upcoming,
    day: NaiveDate,
}

pub struct UpcomingPage {
    kind: EntryKind,
    root: GBox,
    paned: gtk::Paned,
    scroll: gtk::ScrolledWindow,
    /// The list: the calendar row on a single column, then a heading and its rows per day.
    content: GBox,
    /// Holds the calendar, or the clamp that caps its width; it moves between the paned and the list.
    calendar_slot: GBox,
    calendar: Rc<CalendarView>,
    env: Env,
    loaded: RefCell<Option<Loaded>>,
    shape: Cell<Option<Shape>>,
}

impl UpcomingPage {
    fn new(kind: EntryKind, env: Env) -> Rc<Self> {
        ensure_css();
        let page = Rc::new_cyclic(|weak: &std::rc::Weak<Self>| {
            let jump = weak.clone();
            let calendar = CalendarView::new(env.first, env.today, move |day| {
                if let Some(page) = jump.upgrade() {
                    page.jump(day);
                }
            });
            let content = GBox::new(O::Vertical, 0);
            let scroll = gtk::ScrolledWindow::builder().child(&content).vexpand(true).hexpand(true).hscrollbar_policy(gtk::PolicyType::Never).build();
            let paned = gtk::Paned::builder()
                .orientation(O::Horizontal)
                .wide_handle(true)
                .resize_start_child(true)
                .shrink_start_child(false)
                .resize_end_child(true)
                .shrink_end_child(false)
                .vexpand(true)
                .build();
            paned.set_end_child(Some(&scroll));
            let root = GBox::new(O::Vertical, 0);
            root.append(&paned);
            Self {
                kind,
                root,
                paned,
                scroll,
                content,
                calendar_slot: GBox::new(O::Vertical, 0),
                calendar,
                env,
                loaded: RefCell::new(None),
                shape: Cell::new(None),
            }
        });
        page.watch_width();
        page
    }

    /// Starts the live behaviour: the query, and the look at the clock when the page is shown.
    fn start(self: &Rc<Self>) {
        self.watch_day();
        self.reload();
    }

    /// Applies the layout of the content width on the first frame and whenever it crosses a breakpoint.
    fn watch_width(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.paned.add_tick_callback(move |paned, _| {
            let Some(page) = weak.upgrade() else { return glib::ControlFlow::Break };
            let width = paned.width();
            if width > 0 {
                let shape = layout::shape(width);
                if page.shape.get() != Some(shape) {
                    page.apply_shape(shape, width);
                }
            }
            glib::ControlFlow::Continue
        });
    }

    /// Re-queries when the page is shown, or is open, on a later day than the one it was loaded for.
    fn watch_day(self: &Rc<Self>) {
        let weak = Rc::downgrade(self);
        self.root.connect_map(move |_| {
            if let Some(page) = weak.upgrade() {
                page.refresh_if_new_day();
            }
        });
        let weak = Rc::downgrade(self);
        glib::timeout_add_seconds_local(DAY_CHECK_SECONDS, move || {
            let Some(page) = weak.upgrade() else { return glib::ControlFlow::Break };
            if page.root.is_mapped() {
                page.refresh_if_new_day();
            }
            glib::ControlFlow::Continue
        });
    }

    fn refresh_if_new_day(self: &Rc<Self>) {
        let today = Local::now().date_naive();
        let stale = self.loaded.borrow().as_ref().is_some_and(|loaded| model::needs_requery(loaded.day, today));
        if stale {
            self.calendar.set_today(today);
            self.reload();
        }
    }

    /// Reads the entries off the GTK thread and shows them. A failure leaves the page as it is: there is no toast.
    fn reload(self: &Rc<Self>) {
        let (db, kind) = (data::app().db(self.kind), self.kind);
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let now = Local::now();
            let result = download_host::blocking(move || query::load(&db, kind, now)).await;
            let Some(page) = weak.upgrade() else { return };
            match result {
                Some(Ok(up)) => page.show(up, now.date_naive()),
                Some(Err(error)) => tracing::warn!("the upcoming entries could not be read: {error}"),
                None => tracing::warn!("the upcoming query task was dropped"),
            }
        });
    }

    fn show(self: &Rc<Self>, up: Upcoming, day: NaiveDate) {
        self.calendar.set_today(day);
        self.calendar.set_events(up.index.events.clone());
        *self.loaded.borrow_mut() = Some(Loaded { up, day });
        self.fill();
    }

    /// Moves the calendar between the paned and the list and caps its width in the medium band.
    fn apply_shape(self: &Rc<Self>, shape: Shape, width: i32) {
        self.shape.set(Some(shape));
        // the slot leaves whichever container holds it: the paned (two panels) or the list (single column)
        if self.paned.start_child().as_ref() == Some(self.calendar_slot.upcast_ref()) {
            self.paned.set_start_child(None::<&gtk::Widget>);
        } else if self.calendar_slot.parent().is_some() {
            self.content.remove(&self.calendar_slot);
        }
        while let Some(child) = self.calendar_slot.first_child() {
            self.calendar_slot.remove(&child);
        }
        let calendar = self.calendar.widget();
        if calendar.parent().is_some() {
            calendar.unparent();
        }
        match shape.calendar_cap {
            Some(max) => self.calendar_slot.append(&crate::clamp::clamp(calendar, max)),
            None => self.calendar_slot.append(calendar),
        }
        self.calendar_slot.set_valign(if shape.two_panels { Align::Start } else { Align::Fill });
        if shape.two_panels {
            self.paned.set_start_child(Some(&self.calendar_slot));
            self.paned.set_position(layout::divider_start(width));
        }
        self.fill();
    }

    fn two_panels(&self) -> bool {
        self.shape.get().is_some_and(|shape| shape.two_panels)
    }

    /// Rebuilds the list: the calendar row first on a single column, then a heading per day with its rows.
    fn fill(self: &Rc<Self>) {
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }
        if self.shape.get().is_none() {
            return;
        }
        if !self.two_panels() {
            self.content.append(&self.calendar_slot);
        }
        let loaded = self.loaded.borrow();
        let Some(loaded) = loaded.as_ref() else { return };
        let env = &self.env;
        for row in &loaded.up.index.rows {
            match row {
                Row::Header { date, count } => {
                    self.content.append(&heading(&rows::header_text(*date, loaded.day, env.relative, &env.date_pattern, env.locale), *count));
                }
                Row::Item(i) => self.content.append(&self.item(&loaded.up.entries[*i])),
            }
        }
    }

    /// One entry: the book cover and the title (two lines at most); a click opens the entry by id.
    fn item(&self, entry: &Entry) -> Button {
        let (cover_width, cover_height) = layout::cover_size();
        let line = GBox::new(O::Horizontal, 16);
        line.set_size_request(-1, layout::ROW_HEIGHT);
        line.set_margin_start(16);
        line.set_margin_end(16);
        line.set_margin_top(layout::ROW_PADDING);
        line.set_margin_bottom(layout::ROW_PADDING);
        let cover = CoverView::new(cover_width, cover_height);
        cover.frame.set_valign(Align::Center);
        cover.load(ArtRef {
            owner: if self.kind == EntryKind::Anime { Owner::Anime } else { Owner::Manga },
            entry_id: entry.id,
            source_id: entry.source_id,
            kind: ArtKind::Cover,
            url: entry.cover_url.clone(),
            favorite: true,
            last_modified: entry.cover_last_modified,
        });
        line.append(&cover.frame);
        let title = Label::builder()
            .label(&entry.title)
            .xalign(0.0)
            .wrap(true)
            .lines(2)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .hexpand(true)
            .valign(Align::Center)
            .css_classes(["heading"])
            .build();
        line.append(&title);
        let button = Button::builder().child(&line).css_classes(["flat"]).build();
        let (open, id) = (Rc::clone(&self.env.open), entry.id);
        button.connect_clicked(move |_| open(id));
        button
    }

    /// A click on a day: scrolls to its heading, one row further down on a single column where the calendar row comes
    /// first. A day with no heading does nothing.
    fn jump(&self, day: NaiveDate) {
        let target = self.loaded.borrow().as_ref().and_then(|loaded| model::day_jump(&loaded.up.index, day, !self.two_panels()));
        let Some(position) = target else { return };
        let mut widget = self.content.first_child();
        for _ in 0..position {
            widget = widget.and_then(|w| w.next_sibling());
        }
        if let Some(bounds) = widget.and_then(|w| w.compute_bounds(&self.content)) {
            self.scroll.vadjustment().set_value(f64::from(bounds.y()));
        }
    }
}

/// The heading of a day: its text and a badge with the number of entries.
fn heading(text: &str, count: u32) -> GBox {
    let line = GBox::new(O::Horizontal, 8);
    line.set_margin_start(16);
    line.set_margin_end(16);
    line.set_margin_top(8);
    line.set_margin_bottom(4);
    line.append(&Label::builder().label(text).xalign(0.0).css_classes(["heading", "dim-label"]).build());
    line.append(&Label::builder().label(rows::badge_text(count)).valign(Align::Center).css_classes(["kuro-badge"]).build());
    line
}

/// Pushes the entry screen of the entry by its id.
fn open_entry(kind: EntryKind, id: i64) {
    match kind {
        EntryKind::Anime => crate::entry_core::anime::route::open(id),
        EntryKind::Manga => crate::entry_core::route::open(id),
    }
}

/// The help button: opens the Upcoming guide in the default browser.
fn help_button() -> Button {
    let button = crate::ui::icon_button("help-browser-symbolic", &tr(MsgKey::upcoming_guide));
    button.connect_clicked(|_| {
        gtk::UriLauncher::new(layout::help_url()).launch(Some(&crate::ui::window()), None::<&gtk::gio::Cancellable>, |result| {
            if let Err(error) = result {
                tracing::warn!("the Upcoming guide did not open: {error}");
            }
        });
    });
    button
}

/// The navigation page of one kind: the title Upcoming, the back arrow, the help button, and Escape that goes back. `None`
/// before the app data is open.
pub fn build(kind: EntryKind) -> Option<GBox> {
    if !data::is_installed() {
        return None;
    }
    let screen = UpcomingPage::new(kind, Env::current(kind));
    screen.start();
    Some(assemble(screen))
}

/// The navigation page around a screen, and the owner that keeps the screen alive until the page is destroyed.
fn assemble(screen: Rc<UpcomingPage>) -> GBox {
    let page = crate::ui::page(&tr(MsgKey::label_upcoming), &screen.root, vec![help_button().upcast()], true);
    crate::ui::tag(&page, "upcoming");
    let keys = gtk::EventControllerKey::new();
    keys.connect_key_pressed(|_, key, _, _| {
        if key == gdk::Key::Escape {
            crate::ui::pop();
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    page.add_controller(keys);
    let owner = Rc::clone(&screen);
    PAGES.with(|pages| pages.borrow_mut().push(owner));
    let weak = Rc::downgrade(&screen);
    page.connect_destroy(move |_| {
        if let Some(screen) = weak.upgrade() {
            PAGES.with(|pages| pages.borrow_mut().retain(|p| !Rc::ptr_eq(p, &screen)));
        }
    });
    page
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).expect("a date")
    }

    fn at(day: u32, hour: u32) -> i64 {
        Utc.with_ymd_and_hms(2026, 10, day, hour, 0, 0).single().expect("a time").timestamp_millis()
    }

    fn entry(id: i64, next_update_ms: i64) -> Entry {
        Entry { id, title: format!("Entry {id}"), status: 1, source_id: 7, next_update_ms, cover_url: None, cover_last_modified: 0 }
    }

    /// Entries 1 and 2 on the 8th, 3 on the 9th, 4 on the 15th.
    fn upcoming() -> Upcoming {
        model::build(vec![entry(1, at(8, 1)), entry(2, at(8, 20)), entry(3, at(9, 5)), entry(4, at(15, 12))], &Utc)
    }

    fn page(opened: &Rc<RefCell<Vec<i64>>>) -> Rc<UpcomingPage> {
        let log = Rc::clone(opened);
        let env = Env {
            first: Weekday::Mon,
            today: d(8),
            relative: true,
            date_pattern: String::new(),
            locale: "en",
            open: Rc::new(move |id| log.borrow_mut().push(id)),
        };
        let page = UpcomingPage::new(EntryKind::Anime, env);
        page.show(upcoming(), d(8));
        page
    }

    fn children(parent: &GBox) -> Vec<gtk::Widget> {
        let mut all = Vec::new();
        let mut child = parent.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            all.push(widget);
        }
        all
    }

    const SINGLE: Shape = Shape { two_panels: false, calendar_cap: None };
    const TWO: Shape = Shape { two_panels: true, calendar_cap: Some(360) };

    #[test]
    fn upcoming_page_single_column_starts_with_the_calendar_then_headings_and_rows() {
        crate::testing::with_display(|| {
            let page = page(&Rc::default());
            page.apply_shape(SINGLE, 500);
            let rows = children(&page.content);
            assert_eq!(rows.len(), 1 + 3 + 4, "the calendar row, 3 headings and 4 entries");
            assert_eq!(rows[0], page.calendar_slot.clone().upcast::<gtk::Widget>(), "the calendar is the first row");
            assert_eq!(page.calendar_slot.first_child().as_ref(), Some(page.calendar.widget().upcast_ref()), "no cap below 600 px");
            let heading = rows[1].first_child().and_downcast::<Label>().expect("the heading text");
            assert_eq!(heading.label(), "Today");
            let badge = rows[1].last_child().and_downcast::<Label>().expect("the badge");
            assert_eq!(badge.label(), "2");
            assert!(badge.has_css_class("kuro-badge"));
            let row = rows[2].clone().downcast::<Button>().expect("an entry row is a button");
            assert_eq!(row.child().expect("the row line").height_request(), 96, "rows are 96 px");
        });
    }

    #[test]
    fn upcoming_page_two_panels_put_the_calendar_in_the_paned_and_cap_it() {
        crate::testing::with_display(|| {
            let page = page(&Rc::default());
            page.apply_shape(SINGLE, 500);
            page.apply_shape(TWO, 800);
            assert_eq!(page.paned.start_child().as_ref(), Some(page.calendar_slot.upcast_ref()), "the calendar is the start panel");
            assert_eq!(children(&page.content).len(), 3 + 4, "the list has no calendar row");
            let cap = page.calendar_slot.first_child().expect("the capped calendar");
            assert_eq!(cap.type_().name(), "KurobayomiClamp", "the width is capped in the medium band");
            assert_eq!(cap.first_child().as_ref(), Some(page.calendar.widget().upcast_ref()));
            page.apply_shape(Shape { two_panels: true, calendar_cap: None }, 1000);
            assert_eq!(page.calendar_slot.first_child().as_ref(), Some(page.calendar.widget().upcast_ref()), "uncapped above 840 px");
            page.apply_shape(SINGLE, 500);
            assert!(page.paned.start_child().is_none(), "back to one column empties the start panel");
            assert_eq!(children(&page.content).len(), 1 + 3 + 4);
        });
    }

    #[test]
    fn upcoming_page_a_row_click_opens_the_entry_by_id() {
        crate::testing::with_display(|| {
            let opened: Rc<RefCell<Vec<i64>>> = Rc::default();
            let page = page(&opened);
            page.apply_shape(TWO, 800);
            let rows = children(&page.content);
            // [heading, 1, 2, heading, 3, heading, 4]
            rows[2].clone().downcast::<Button>().expect("a row button").emit_clicked();
            rows[6].clone().downcast::<Button>().expect("a row button").emit_clicked();
            assert_eq!(opened.borrow().as_slice(), [2, 4]);
        });
    }

    #[test]
    fn upcoming_page_day_jump_scrolls_to_the_heading_and_ignores_empty_days() {
        crate::testing::with_display(|| {
            let page = page(&Rc::default());
            page.apply_shape(SINGLE, 500);
            page.root.size_allocate(&gtk::Allocation::new(0, 0, 500, 200), -1);
            let adjustment = page.scroll.vadjustment();
            assert_eq!(adjustment.value(), 0.0);
            page.jump(d(20));
            assert_eq!(adjustment.value(), 0.0, "a day with no heading does nothing");
            page.jump(d(15));
            let heading = children(&page.content)[1 + 5].clone();
            let top = heading.compute_bounds(&page.content).expect("bounds").y();
            assert!(adjustment.value() > 0.0, "the list scrolled");
            assert!((adjustment.value() - f64::from(top).min(adjustment.upper() - adjustment.page_size())).abs() < 1.0, "to the heading of the 15th, one row down for the calendar");
        });
    }

    #[test]
    fn upcoming_page_has_no_empty_state_text() {
        crate::testing::with_display(|| {
            let opened: Rc<RefCell<Vec<i64>>> = Rc::default();
            let page = page(&opened);
            page.show(model::build(Vec::new(), &Utc), d(8));
            page.apply_shape(SINGLE, 500);
            assert_eq!(children(&page.content).len(), 1, "only the calendar row; no message");
        });
    }

    #[test]
    fn upcoming_page_is_titled_upcoming_and_escape_pops_it() {
        crate::testing::with_display(|| {
            let (stack, _window) = crate::testing::nav();
            let base = GBox::new(O::Vertical, 0);
            stack.add_child(&base);
            stack.set_visible_child(&base);
            let page = assemble(page(&Rc::default()));
            crate::ui::push(&page);
            let title = page.first_child().and_downcast::<gtk::HeaderBar>().and_then(|bar| bar.title_widget()).and_downcast::<Label>().expect("the title");
            assert_eq!(title.label(), "Upcoming");
            assert_eq!(page.widget_name(), "screen:upcoming");
            assert_eq!(crate::ui::page_stack().1, 1, "the page is on show");
            let keys = page
                .observe_controllers()
                .into_iter()
                .filter_map(|c| c.ok())
                .find_map(|c| c.downcast::<gtk::EventControllerKey>().ok())
                .expect("the key controller");
            let _ = keys.emit_by_name::<bool>("key-pressed", &[&gdk::Key::Escape.into_glib(), &0u32, &gdk::ModifierType::empty()]);
            assert_eq!(crate::ui::page_stack().1, 0, "Escape went back to the page below");
        });
    }

    #[test]
    fn upcoming_page_help_button_has_the_guide_tooltip() {
        crate::testing::with_display(|| {
            assert_eq!(help_button().tooltip_text().as_deref(), Some("Upcoming Guide"));
            assert_eq!(layout::help_url(), "https://aniyomi.org/docs/faq/updates/upcoming");
        });
    }
}
