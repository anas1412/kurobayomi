//! The player's popover menus: quality and server, speed, chapters, and the audio and subtitle menu (`menu.rs`).
//! Every menu is a `gtk::MenuButton` whose popover is rebuilt each time it opens, so its rows show what the player
//! has now. [`menu`] and [`section`] are the one way to build such a list: an optional heading, then rows with a check
//! on the current choice, each row going through `widgets::row_activate`. Main thread only.

pub(super) mod chapters;
pub(super) mod quality;
pub(super) mod speed;

use gtk::prelude::*;

/// One row of a menu.
pub struct Choice {
    pub title: String,
    pub subtitle: Option<String>,
    /// The row that carries the check mark.
    pub current: bool,
    /// A row that is not enabled is shown but does nothing when clicked.
    pub enabled: bool,
    /// Runs when the row is chosen; the popover closes first.
    pub pick: Box<dyn Fn()>,
}

/// A menu button, labelled by `tooltip`, whose popover `fill` rebuilds on each open. `fill` gets the popover's column
/// (already empty) and the popover itself.
pub fn menu(tooltip: &str, fill: impl Fn(&gtk::Box, &gtk::Popover) + 'static) -> gtk::MenuButton {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 6);
    crate::ui::margins(&column, 6);
    let popover = gtk::Popover::builder().child(&column).build();
    {
        let column = column.clone();
        popover.connect_show(move |popover| {
            clear(&column);
            fill(&column, popover);
        });
    }
    let button = gtk::MenuButton::builder().popover(&popover).tooltip_text(tooltip).build();
    button.update_property(&[gtk::accessible::Property::Label(tooltip)]);
    button
}

/// Removes every child of a menu column.
pub fn clear(column: &gtk::Box) {
    while let Some(child) = column.first_child() {
        column.remove(&child);
    }
}

/// Appends an optional heading and one boxed list of `choices`. A chosen row closes `popover` and then runs its pick.
pub fn section(column: &gtk::Box, heading: Option<&str>, choices: Vec<Choice>, popover: &gtk::Popover) {
    if let Some(heading) = heading {
        column.append(&gtk::Label::builder().label(heading).xalign(0.0).css_classes(["heading"]).build());
    }
    let list = crate::ui::boxed_list();
    for choice in choices {
        let check = gtk::Image::from_icon_name("object-select-symbolic");
        check.set_opacity(if choice.current { 1.0 } else { 0.0 });
        let row = crate::ui::row(&choice.title, choice.subtitle.as_deref(), Some(check.upcast()));
        row.set_activatable(choice.enabled);
        let (popover, pick) = (popover.clone(), choice.pick);
        crate::widgets::row_activate::on_activate(&row, move |_| {
            popover.popdown();
            pick();
        });
        list.append(&row);
    }
    column.append(&list);
}
