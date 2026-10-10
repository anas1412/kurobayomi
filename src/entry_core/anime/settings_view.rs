//! The episode settings window over `settings.rs`: a modal window with Filter, Sort and Display tabs. Filter shows four
//! tri-state rows, Sort four rows with the direction on the active one, Display two radios and two checkboxes.
//! Escape closes it. A click only reports a `Change`; the page writes it and calls `update` when the new flag word
//! arrives, so the window always shows what the store holds.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_domain::flags::TriState;
use kuro_i18n::{MsgKey, tr};

use super::set_default::{self, Scope};
use super::settings::{
    DisplayRows, FilterRow, SortRow, display_mode_write, display_rows, filter_click, filter_rows, previews_write, session_note,
    sort_rows, summaries_write,
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
    /// "Set as default" was confirmed: store the flags of this entry as the library default, and with `apply_all`
    /// write them to every favorite too.
    SetDefault { apply_all: bool },
}

pub struct SettingsWindow {
    window: gtk::Window,
    filter: GBox,
    sort: GBox,
    display: GBox,
    on_change: Rc<dyn Fn(Change)>,
}

pub(super) fn tri_icon(state: TriState) -> &'static str {
    match state {
        TriState::ENABLED_IS => "object-select-symbolic",
        TriState::ENABLED_NOT => "window-close-symbolic",
        _ => "",
    }
}

pub(super) fn icon_row(label: &str, icon: &str) -> (Button, gtk::Image) {
    let content = GBox::new(O::Horizontal, 12);
    let image = gtk::Image::from_icon_name(if icon.is_empty() { "image-missing" } else { icon });
    image.set_opacity(if icon.is_empty() { 0.0 } else { 1.0 });
    content.append(&image);
    content.append(&Label::builder().label(label).xalign(0.0).hexpand(true).build());
    (Button::builder().child(&content).css_classes(["flat"]).build(), image)
}

/// The confirm dialog of "Set as default" for the window of `scope`: the text, the library checkbox (off), Cancel and
/// OK, all from `set_default`. OK reports `Change::SetDefault`; the default keys are always written, and the checkbox
/// decides whether every anime in the library takes the settings too.
pub(super) fn confirm_default(parent: &gtk::Window, scope: Scope, on_change: Rc<dyn Fn(Change)>) {
    let also = gtk::CheckButton::builder().label(tr(set_default::LIBRARY_CHECKBOX)).build();
    let ok = Button::builder().label(tr(MsgKey::action_ok)).css_classes(["suggested-action"]).build();
    let cancel = Button::with_label(&tr(MsgKey::action_cancel));
    let buttons = GBox::new(O::Horizontal, 8);
    buttons.set_halign(Align::End);
    buttons.append(&cancel);
    buttons.append(&ok);
    let col = ui::column();
    col.append(&Label::builder().label(tr(set_default::BODY)).wrap(true).xalign(0.0).build());
    col.append(&also);
    col.append(&buttons);
    let dialog = ui::modal(Label::new(Some(&tr(set_default::title(scope)))).upcast_ref(), &col, 360, 200);
    dialog.set_transient_for(Some(parent));
    cancel.connect_clicked({
        let dialog = dialog.clone();
        move |_| dialog.close()
    });
    ok.connect_clicked({
        let dialog = dialog.clone();
        move |_| {
            dialog.close();
            on_change(Change::SetDefault { apply_all: set_default::confirm(also.is_active()).apply_to_library });
        }
    });
    dialog.present();
}

pub(super) fn clear(page: &GBox) {
    while let Some(child) = page.first_child() {
        page.remove(&child);
    }
}

impl SettingsWindow {
    /// Builds the window. `on_change` is called for each click. An entry outside the library (`favorite` false) shows
    /// that its settings are kept for this session.
    pub fn new(word: i64, downloaded_only: bool, favorite: bool, on_change: Rc<dyn Fn(Change)>) -> Rc<Self> {
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
        // under the tabs: the session note of an entry outside the library, and Set as default
        let body = GBox::new(O::Vertical, 6);
        stack.set_vexpand(true);
        body.append(&stack);
        if let Some(note) = session_note(favorite) {
            body.append(&Label::builder().label(note).wrap(true).xalign(0.0).css_classes(["dim-label", "caption"]).margin_start(8).margin_end(8).build());
        }
        let set_default = Button::builder().label(tr(MsgKey::set_chapter_settings_as_default)).css_classes(["flat"]).halign(Align::End).build();
        body.append(&set_default);
        let window = ui::modal(switcher.upcast_ref(), &body, 360, 420);
        set_default.connect_clicked({
            let (window, on_change) = (window.clone(), Rc::clone(&on_change));
            move |_| confirm_default(&window, Scope::Episode, Rc::clone(&on_change))
        });
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

    /// The window, for the page's one dialog slot.
    pub fn window(&self) -> &gtk::Window {
        &self.window
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
