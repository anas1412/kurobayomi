//! Settings page scaffold: header bar, a scrolled column with one ListBox per group, 12 px between groups,
//! and the highlight target. Groups that are disabled are not built.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, HeaderBar, Label, ListBox, ListBoxRow, Orientation, PolicyType, ScrolledWindow, SelectionMode};

use super::highlight;
use super::model::{Preference, PreferenceItem};
use super::render;
use super::style;
use super::widgets::{group, GROUP_SPACER, PAGE_MAX_WIDTH};

/// A built page. `targets` maps each highlightable row title to its row (first registration wins).
pub struct Built {
    pub widget: gtk::Widget,
    pub scrolled: ScrolledWindow,
    pub targets: Vec<(String, ListBoxRow)>,
}

/// Builds `tree` as a page titled `title`. Takes the pending highlight request (cleared even when no row matches),
/// and reveals the matching row after the reveal delay.
pub fn build(title: &str, on_back: Option<Rc<dyn Fn()>>, tree: &[Preference]) -> Built {
    let body = body(tree);
    let page = GBox::new(Orientation::Vertical, 0);
    page.append(&header_bar(title, on_back));
    page.append(&body.scrolled);
    Built { widget: page.upcast(), scrolled: body.scrolled, targets: body.targets }
}

/// Builds `tree` without a header: the scrolled column of groups, for a caller that has a header of its own. The
/// pending highlight request is taken here, so the page that is built next shows it.
pub fn body(tree: &[Preference]) -> Built {
    style::install();
    let pending = highlight::take();

    let content = GBox::new(Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_margin_top(8);
    content.set_margin_bottom(16);
    let mut targets: Vec<(String, ListBoxRow)> = Vec::new();

    let sections = tree.iter().filter(|entry| match entry {
        Preference::Group(g) => g.enabled,
        Preference::Item(_) => true,
    });
    let entries: Vec<&Preference> = sections.collect();
    let last = entries.len().saturating_sub(1);
    let mut loose: Option<ListBox> = None;
    for (index, entry) in entries.iter().enumerate() {
        match entry {
            Preference::Item(item) => {
                let list = loose.get_or_insert_with(|| new_list(&content));
                append_item(list, item, &mut targets);
            }
            Preference::Group(group_entry) => {
                loose = None;
                let block = GBox::new(Orientation::Vertical, 0);
                if let Some(header) = group::header(&group_entry.title) {
                    block.append(&header);
                }
                let list = new_list(&block);
                for item in &group_entry.items {
                    append_item(&list, item, &mut targets);
                }
                // a request for the group's title (the onboarding restore asks for "Backup and restore") flashes its
                // first row, because a group header is not a row
                if let Some(first) = list.row_at_index(0) {
                    if !group_entry.title.trim().is_empty() {
                        targets.push((group_entry.title.clone(), first));
                    }
                }
                if index != last {
                    block.set_margin_bottom(GROUP_SPACER);
                }
                content.append(&block);
            }
        }
    }

    let scrolled = ScrolledWindow::new();
    scrolled.set_hscrollbar_policy(PolicyType::Never);
    scrolled.set_propagate_natural_width(true);
    scrolled.set_max_content_width(PAGE_MAX_WIDTH);
    scrolled.set_vexpand(true);
    scrolled.set_child(Some(&content));

    if let Some(key) = pending {
        let titles: Vec<&str> = targets.iter().map(|(t, _)| t.as_str()).collect();
        if let Some(index) = highlight::first_match(titles, &key) {
            highlight::reveal(&scrolled, &targets[index].1);
        }
    }

    Built { widget: scrolled.clone().upcast(), scrolled, targets }
}

fn new_list(parent: &GBox) -> ListBox {
    let list = ListBox::new();
    list.add_css_class("boxed-list");
    list.set_selection_mode(SelectionMode::None);
    list.set_margin_start(16);
    list.set_margin_end(16);
    parent.append(&list);
    list
}

/// Adds the item's row. Its title is registered for highlight unless it is an info block or custom widget.
fn append_item(list: &ListBox, item: &PreferenceItem, targets: &mut Vec<(String, ListBoxRow)>) {
    let Some(row) = render::item_row(item) else {
        return;
    };
    if !item.enabled() {
        row.set_sensitive(false);
    }
    let highlightable = !matches!(item, PreferenceItem::Info { .. } | PreferenceItem::Custom { .. });
    if highlightable {
        targets.push((item.title().to_owned(), row.clone()));
    }
    list.append(&row);
}

fn header_bar(title: &str, on_back: Option<Rc<dyn Fn()>>) -> HeaderBar {
    let bar = HeaderBar::new();
    let label = Label::new(Some(title));
    label.add_css_class("title");
    label.set_halign(Align::Center);
    bar.set_title_widget(Some(&label));
    if let Some(back) = on_back {
        let button = Button::from_icon_name("go-previous-symbolic");
        button.set_tooltip_text(Some(&super::strings::back()));
        button.connect_clicked(move |_| back());
        bar.pack_start(&button);
    }
    bar
}
