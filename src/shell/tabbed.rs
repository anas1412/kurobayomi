//! Tabbed screen (Kotlin `TabbedScreen`, T-36a-040): a tab row above a stack of pages. Each page binds its own search query,
//! its own search flag and its own badge. The selected page drives the app bar, so a page with search disabled hides the
//! search entry for that page only, and switching pages keeps every page's query.
//!
//! `Tabbed` is the pure model. `build` makes the GTK row and stack around it.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;

/// Edge padding of a scrollable tab row, in logical pixels (`ScrollableTabRow(edgePadding = 13.dp)`).
pub const SCROLLABLE_EDGE_PX: i32 = 13;

/// One page: its title, its badge (shown when present), whether search is enabled, and its own query (`None` when
/// search is closed).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    pub title: String,
    pub badge: Option<u32>,
    pub search_enabled: bool,
    pub query: Option<String>,
}

impl Page {
    pub fn new(title: &str, search_enabled: bool) -> Self {
        Self { title: title.to_string(), badge: None, search_enabled, query: None }
    }
}

/// The selected page and the pages.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tabbed {
    pages: Vec<Page>,
    selected: usize,
}

impl Tabbed {
    /// The first page is selected.
    pub fn new(pages: Vec<Page>) -> Self {
        Self { pages, selected: 0 }
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn pages(&self) -> &[Page] {
        &self.pages
    }

    /// Selects `index`. Returns whether the selection changed; an index out of range changes nothing.
    pub fn select(&mut self, index: usize) -> bool {
        if index >= self.pages.len() || index == self.selected {
            return false;
        }
        self.selected = index;
        true
    }

    pub fn set_badge(&mut self, index: usize, badge: Option<u32>) -> bool {
        self.pages.get_mut(index).map(|page| page.badge = badge).is_some()
    }

    /// Stores the query of page `index`. `None` closes that page's search. Returns false for an index out of range.
    pub fn set_query(&mut self, index: usize, query: Option<String>) -> bool {
        self.pages.get_mut(index).map(|page| page.query = query).is_some()
    }

    pub fn query(&self, index: usize) -> Option<&str> {
        self.pages.get(index).and_then(|page| page.query.as_deref())
    }

    /// The query the app bar shows: the selected page's own query, or `None` when that page has search disabled.
    pub fn app_bar_query(&self) -> Option<&str> {
        let page = self.pages.get(self.selected)?;
        if page.search_enabled { page.query.as_deref() } else { None }
    }
}

/// The pill text of a badge: the count as it is, never capped. `None` when the tab has no badge.
pub fn badge_text(badge: Option<u32>) -> Option<String> {
    badge.map(|count| count.to_string())
}

/// The inset at the row's ends: 13 px when the row scrolls, none when it fits.
pub fn edge_padding(scrollable: bool) -> i32 {
    if scrollable { SCROLLABLE_EDGE_PX } else { 0 }
}

fn child_name(index: usize) -> String {
    format!("tab-{index}")
}

/// The GTK tab row and stack, with the model they share.
pub struct TabbedView {
    pub widget: gtk::Box,
    model: Rc<RefCell<Tabbed>>,
    buttons: Vec<gtk::ToggleButton>,
    pills: Vec<gtk::Label>,
}

impl TabbedView {
    /// The model. The screen reads the app bar query and the page flags from it.
    pub fn model(&self) -> Rc<RefCell<Tabbed>> {
        self.model.clone()
    }

    /// Selects `index` as a tap would. Out-of-range indices do nothing.
    pub fn select(&self, index: usize) {
        if let Some(button) = self.buttons.get(index) {
            button.set_active(true);
        }
    }

    /// Updates the badge of tab `index` in the model and on its pill.
    pub fn set_badge(&self, index: usize, badge: Option<u32>) {
        if self.model.borrow_mut().set_badge(index, badge) {
            let text = badge_text(badge);
            if let Some(pill) = self.pills.get(index) {
                pill.set_text(text.as_deref().unwrap_or(""));
                pill.set_visible(text.is_some());
            }
        }
    }
}

/// Builds the tab row and the stack. `contents` are the pages in order, one widget each. The row scrolls when
/// `scrollable` is set, otherwise its tabs share the width. `on_select` runs after a tap changes the selection.
pub fn build(pages: Vec<Page>, contents: Vec<gtk::Widget>, scrollable: bool, on_select: impl Fn(usize) + 'static) -> TabbedView {
    let model = Rc::new(RefCell::new(Tabbed::new(pages)));
    let stack = gtk::Stack::new();
    stack.set_vexpand(true);
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    row.set_homogeneous(!scrollable);
    row.set_margin_start(edge_padding(scrollable));
    row.set_margin_end(edge_padding(scrollable));

    let on_select: Rc<dyn Fn(usize)> = Rc::new(on_select);
    let mut buttons: Vec<gtk::ToggleButton> = Vec::new();
    let mut pills = Vec::new();
    let titles: Vec<String> = model.borrow().pages().iter().map(|page| page.title.clone()).collect();
    for (index, (title, content)) in titles.iter().zip(contents).enumerate() {
        stack.add_named(&content, Some(&child_name(index)));
        let label = gtk::Label::new(Some(title));
        label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        let pill = gtk::Label::new(None);
        pill.add_css_class("badge");
        pill.set_visible(false);
        let inner = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        inner.append(&label);
        inner.append(&pill);
        let button = gtk::ToggleButton::builder().child(&inner).hexpand(true).build();
        if let Some(first) = buttons.first() {
            button.set_group(Some(first));
        }
        {
            let model = model.clone();
            let stack = stack.clone();
            let on_select = on_select.clone();
            button.connect_toggled(move |button| {
                if !button.is_active() {
                    return;
                }
                if model.borrow_mut().select(index) {
                    stack.set_visible_child_name(&child_name(index));
                    on_select(index);
                }
            });
        }
        row.append(&button);
        buttons.push(button);
        pills.push(pill);
    }
    if let Some(first) = buttons.first() {
        first.set_active(true);
    }

    let row_host: gtk::Widget = if scrollable {
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Never);
        scroller.set_child(Some(&row));
        scroller.upcast()
    } else {
        row.upcast()
    };
    let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
    widget.append(&row_host);
    widget.append(&stack);
    let badges: Vec<Option<u32>> = model.borrow().pages().iter().map(|page| page.badge).collect();
    let view = TabbedView { widget, model, buttons, pills };
    for (index, badge) in badges.into_iter().enumerate() {
        view.set_badge(index, badge);
    }
    view
}

#[cfg(test)]
mod tests {
    use super::*;

    fn three() -> Tabbed {
        Tabbed::new(vec![Page::new("Anime", true), Page::new("Manga", true), Page::new("History", false)])
    }

    #[test]
    fn each_page_keeps_its_own_query_after_switching() {
        let mut tabs = three();
        tabs.set_query(0, Some("one piece".into()));
        tabs.select(1);
        assert_eq!(tabs.app_bar_query(), None, "the manga page has no query yet");
        tabs.set_query(1, Some("naruto".into()));
        tabs.select(0);
        assert_eq!(tabs.app_bar_query(), Some("one piece"));
        tabs.select(1);
        assert_eq!(tabs.app_bar_query(), Some("naruto"));
    }

    #[test]
    fn search_disabled_hides_the_entry_for_that_page_only() {
        let mut tabs = three();
        tabs.set_query(0, Some("x".into()));
        tabs.select(2);
        assert_eq!(tabs.app_bar_query(), None, "history has search disabled");
        tabs.select(0);
        assert_eq!(tabs.app_bar_query(), Some("x"), "anime keeps its search");
    }

    #[test]
    fn select_reports_only_real_changes_and_ignores_out_of_range() {
        let mut tabs = three();
        assert!(!tabs.select(0));
        assert!(tabs.select(2));
        assert!(!tabs.select(9));
        assert_eq!(tabs.selected(), 2);
    }

    #[test]
    fn badges_show_the_count_uncapped_and_only_when_present() {
        assert_eq!(badge_text(None), None);
        assert_eq!(badge_text(Some(3)), Some("3".into()));
        assert_eq!(badge_text(Some(1250)), Some("1250".into()));
        let mut tabs = three();
        assert!(tabs.set_badge(1, Some(7)));
        assert_eq!(tabs.pages()[1].badge, Some(7));
        assert!(!tabs.set_badge(5, Some(1)));
    }

    #[test]
    fn a_scrollable_row_has_thirteen_pixel_edges_and_a_fixed_row_has_none() {
        assert_eq!(edge_padding(true), 13);
        assert_eq!(edge_padding(false), 0);
    }
}
