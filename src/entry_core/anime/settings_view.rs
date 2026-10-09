//! The episode settings window over `settings.rs`: a modal window with Filter, Sort and Display tabs. Filter shows four
//! tri-state rows, Sort four rows with the direction on the active one, Display two radios and two checkboxes.
//! Escape closes it. A click only reports a `Change`; the page writes it and calls `update` when the new flag word
//! arrives, so the window always shows what the store holds.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_domain::flags::TriState;
use kuro_i18n::{MsgKey, tr};

use super::settings::{
    DisplayRows, FilterRow, SortRow, display_mode_write, display_rows, filter_click, filter_rows, previews_write, sort_rows,
    summaries_write,
};
use crate::entry_core::flags::Write;
use crate::ui;

/// What a click in the window asks the page to store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// A masked write `(flag, mask)` on the episode flag word.
    Flag(Write),
    /// Sort by this key, or flip the direction when it is already the key.
    Sort(i64),
}

pub struct SettingsWindow {
    window: gtk::Window,
    filter: GBox,
    sort: GBox,
    display: GBox,
    on_change: Rc<dyn Fn(Change)>,
}

fn tri_icon(state: TriState) -> &'static str {
    match state {
        TriState::ENABLED_IS => "object-select-symbolic",
        TriState::ENABLED_NOT => "window-close-symbolic",
        _ => "",
    }
}

fn icon_row(label: &str, icon: &str) -> (Button, gtk::Image) {
    let content = GBox::new(O::Horizontal, 12);
    let image = gtk::Image::from_icon_name(if icon.is_empty() { "image-missing" } else { icon });
    image.set_opacity(if icon.is_empty() { 0.0 } else { 1.0 });
    content.append(&image);
    content.append(&Label::builder().label(label).xalign(0.0).hexpand(true).build());
    (Button::builder().child(&content).css_classes(["flat"]).build(), image)
}

fn clear(page: &GBox) {
    while let Some(child) = page.first_child() {
        page.remove(&child);
    }
}

impl SettingsWindow {
    /// Builds the window. `on_change` is called for each click.
    pub fn new(word: i64, downloaded_only: bool, on_change: Rc<dyn Fn(Change)>) -> Rc<Self> {
        let page = || {
            let b = GBox::new(O::Vertical, 2);
            ui::margins(&b, 8);
            b
        };
        let (filter, sort, display) = (page(), page(), page());
        let stack = gtk::Stack::new();
        stack.add_titled(&filter, Some("filter"), &tr(MsgKey::action_filter));
        stack.add_titled(&sort, Some("sort"), &tr(MsgKey::action_sort));
        stack.add_titled(&display, Some("display"), &tr(MsgKey::action_display));
        let switcher = gtk::StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        let window = ui::modal(switcher.upcast_ref(), &stack, 360, 380);
        let keys = gtk::EventControllerKey::new();
        keys.connect_key_pressed({
            let window = window.clone();
            move |_, key, _, _| {
                if key == gtk::gdk::Key::Escape {
                    window.close();
                    gtk::glib::Propagation::Stop
                } else {
                    gtk::glib::Propagation::Proceed
                }
            }
        });
        window.add_controller(keys);
        let this = Rc::new(Self { window, filter, sort, display, on_change });
        this.update(word, downloaded_only);
        this
    }

    pub fn present(&self) {
        self.window.present();
    }

    /// Runs `f` when the window closes (Escape, the close button or the page).
    pub fn on_close(&self, f: impl Fn() + 'static) {
        self.window.connect_close_request(move |_| {
            f();
            gtk::glib::Propagation::Proceed
        });
    }

    /// Redraws the three tabs from the flag word.
    pub fn update(&self, word: i64, downloaded_only: bool) {
        self.fill_filter(&filter_rows(word, downloaded_only), downloaded_only);
        self.fill_sort(&sort_rows(word));
        self.fill_display(display_rows(word));
    }

    fn fill_filter(&self, rows: &[FilterRow], downloaded_only: bool) {
        clear(&self.filter);
        for row in rows {
            let (button, _) = icon_row(&tr(row.label), tri_icon(row.state));
            button.set_sensitive(row.sensitive);
            let (on_change, row) = (Rc::clone(&self.on_change), *row);
            button.connect_clicked(move |_| {
                if let Some(write) = filter_click(&row, downloaded_only) {
                    on_change(Change::Flag(write));
                }
            });
            self.filter.append(&button);
        }
    }

    fn fill_sort(&self, rows: &[SortRow]) {
        clear(&self.sort);
        for row in rows {
            let icon = match row.descending {
                None => "",
                Some(true) => "pan-down-symbolic",
                Some(false) => "pan-up-symbolic",
            };
            let (button, _) = icon_row(&tr(row.label), icon);
            let (on_change, mode) = (Rc::clone(&self.on_change), row.mode);
            button.connect_clicked(move |_| on_change(Change::Sort(mode)));
            self.sort.append(&button);
        }
    }

    fn fill_display(&self, rows: DisplayRows) {
        clear(&self.display);
        let source = gtk::CheckButton::builder().label(tr(MsgKey::show_title)).active(rows.source_title).build();
        let number = gtk::CheckButton::builder().label(tr(MsgKey::show_episode_number)).active(rows.episode_number).build();
        number.set_group(Some(&source));
        let previews = gtk::CheckButton::builder().label(tr(MsgKey::show_episode_previews)).active(rows.previews).build();
        let summaries = gtk::CheckButton::builder().label(tr(MsgKey::show_episode_summaries)).active(rows.summaries).build();
        // a radio sends only when it becomes active, so the deactivation of its twin sends nothing
        for (radio, episode_number) in [(&source, false), (&number, true)] {
            let on_change = Rc::clone(&self.on_change);
            radio.connect_toggled(move |r| {
                if r.is_active() {
                    on_change(Change::Flag(display_mode_write(episode_number)));
                }
            });
        }
        let on_change = Rc::clone(&self.on_change);
        previews.connect_toggled(move |b| on_change(Change::Flag(previews_write(!b.is_active()))));
        let on_change = Rc::clone(&self.on_change);
        summaries.connect_toggled(move |b| on_change(Change::Flag(summaries_write(!b.is_active()))));
        let heading = Label::builder().label(tr(MsgKey::action_display_mode)).xalign(0.0).css_classes(["heading"]).build();
        heading.set_margin_bottom(4);
        for w in [heading.upcast_ref::<gtk::Widget>(), source.upcast_ref(), number.upcast_ref(), previews.upcast_ref(), summaries.upcast_ref()] {
            w.set_halign(Align::Start);
            self.display.append(w);
        }
    }
}
