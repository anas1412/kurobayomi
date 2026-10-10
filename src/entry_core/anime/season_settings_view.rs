//! The season settings window over `season_settings.rs` (T-39a-008): a modal window with Filter, Sort and Display tabs for a
//! `Seasons` entry. Filter has six tri-state rows, Sort seven rows with the direction on the active one, Display four
//! grid-mode chips, a size slider (0 is automatic, 1 to 10), five overlay checkboxes and two grid-title radios. Escape
//! closes it. A click only reports a `Change`; the page writes it through the masked-write path and calls `update`
//! when the new flag word arrives, so the window shows what the store holds. "Set as default" asks first
//! (`set_default::confirm`), like the episode window.

use std::cell::Cell;
use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Button, Label, Orientation as O};
use kuro_domain::entries::season_display::SeasonDisplayMode;
use kuro_i18n::{MsgKey, tr};

use super::season_settings::{
    GRID_MODES, OVERLAY_ORDER, Overlay, FilterRow, filter_click, filter_rows, grid_mode, grid_mode_write, grid_size,
    grid_size_write, overlay_on, overlay_write, size_label, size_value, sort_rows, title_is_number, title_write,
};
use super::set_default::Scope;
use super::settings_view::{Change, clear, confirm_default, icon_row, tri_icon};
use crate::ui;

pub struct SeasonSettingsWindow {
    window: gtk::Window,
    filter: GBox,
    sort: GBox,
    chips: Vec<(SeasonDisplayMode, gtk::ToggleButton)>,
    size_title: Label,
    size_text: Label,
    size: gtk::Scale,
    overlays: Vec<(Overlay, gtk::CheckButton)>,
    source_title: gtk::CheckButton,
    number_title: gtk::CheckButton,
    /// Set while `update` puts the stored state on the widgets, so that does not send a click back.
    updating: Cell<bool>,
    /// The slider is held: a snapshot that is one write behind must not pull it back under the pointer.
    held: Cell<bool>,
    on_change: Rc<dyn Fn(Change)>,
}

impl SeasonSettingsWindow {
    /// Builds the window. `on_change` is called for each click. An entry outside the library (`favorite` false) shows
    /// that its settings are kept for this session.
    pub fn new(word: i64, downloaded_only: bool, favorite: bool, on_change: Rc<dyn Fn(Change)>) -> Rc<Self> {
        let page = || {
            let b = GBox::new(O::Vertical, 2);
            ui::margins(&b, 8);
            b
        };
        let (filter, sort, display) = (page(), page(), page());

        // Display: chips, slider, overlays, title radios. They are built once and set from the word by `update`.
        let chip_row = GBox::new(O::Horizontal, 0);
        chip_row.add_css_class("linked");
        let mut chips: Vec<(SeasonDisplayMode, gtk::ToggleButton)> = Vec::new();
        for (label, mode) in GRID_MODES {
            let chip = gtk::ToggleButton::with_label(&tr(label));
            if let Some((_, first)) = chips.first() {
                chip.set_group(Some(first));
            }
            chip_row.append(&chip);
            chips.push((mode, chip));
        }
        let size_title = Label::builder().xalign(0.0).hexpand(true).build();
        let size_text = Label::builder().xalign(1.0).css_classes(["dim-label"]).build();
        let size_head = GBox::new(O::Horizontal, 8);
        size_head.append(&size_title);
        size_head.append(&size_text);
        let size = gtk::Scale::with_range(O::Horizontal, 0.0, 10.0, 1.0);
        size.set_digits(0);
        size.set_draw_value(false);
        size.set_round_digits(0);
        let overlays: Vec<(Overlay, gtk::CheckButton)> =
            OVERLAY_ORDER.iter().map(|&o| (o, gtk::CheckButton::builder().label(tr(o.label())).build())).collect();
        let source_title = gtk::CheckButton::builder().label(tr(MsgKey::show_title)).build();
        let number_title = gtk::CheckButton::builder().label(tr(MsgKey::show_season_number)).build();
        number_title.set_group(Some(&source_title));
        let heading = |text: MsgKey| {
            let l = Label::builder().label(tr(text)).xalign(0.0).css_classes(["heading"]).margin_top(8).margin_bottom(4).build();
            l.set_halign(Align::Start);
            l
        };
        display.append(&heading(MsgKey::action_display_mode));
        display.append(&chip_row);
        display.append(&size_head);
        display.append(&size);
        display.append(&heading(MsgKey::action_display));
        for (_, check) in &overlays {
            check.set_halign(Align::Start);
            display.append(check);
        }
        display.append(&heading(MsgKey::action_display_mode));
        for radio in [&source_title, &number_title] {
            radio.set_halign(Align::Start);
            display.append(radio);
        }

        let stack = gtk::Stack::new();
        stack.add_titled(&filter, Some("filter"), &tr(MsgKey::action_filter));
        stack.add_titled(&sort, Some("sort"), &tr(MsgKey::action_sort));
        stack.add_titled(&display, Some("display"), &tr(MsgKey::action_display));
        let switcher = gtk::StackSwitcher::new();
        switcher.set_stack(Some(&stack));
        let body = GBox::new(O::Vertical, 6);
        stack.set_vexpand(true);
        body.append(&stack);
        if let Some(note) = super::settings::session_note(favorite) {
            body.append(&Label::builder().label(note).wrap(true).xalign(0.0).css_classes(["dim-label", "caption"]).margin_start(8).margin_end(8).build());
        }
        let set_default = Button::builder().label(tr(MsgKey::set_chapter_settings_as_default)).css_classes(["flat"]).halign(Align::End).build();
        body.append(&set_default);
        let window = ui::modal(switcher.upcast_ref(), &body, 380, 520);
        set_default.connect_clicked({
            let (window, on_change) = (window.clone(), Rc::clone(&on_change));
            move |_| confirm_default(&window, Scope::Season, Rc::clone(&on_change))
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

        let this = Rc::new(Self { window, filter, sort, chips, size_title, size_text, size, overlays, source_title, number_title, updating: Cell::new(false), held: Cell::new(false), on_change });
        this.connect_display();
        this.update(word, downloaded_only);
        this
    }

    /// Wires the Display widgets once. A handler sends nothing while `update` is setting the widgets.
    fn connect_display(self: &Rc<Self>) {
        for (mode, chip) in &self.chips {
            let (weak, mode) = (Rc::downgrade(self), *mode);
            chip.connect_toggled(move |c| {
                if let (true, Some(this)) = (c.is_active(), weak.upgrade()) {
                    this.send(Change::Flag(grid_mode_write(mode)));
                }
            });
        }
        let weak = Rc::downgrade(self);
        self.size.connect_value_changed(move |scale| {
            if let Some(this) = weak.upgrade() {
                let size = scale.value().round() as i64;
                this.size_text.set_label(&size_value(size));
                this.send(Change::Flag(grid_size_write(size)));
            }
        });
        let hold = gtk::GestureClick::new();
        hold.set_propagation_phase(gtk::PropagationPhase::Capture);
        hold.connect_pressed({
            let weak = Rc::downgrade(self);
            move |_, _, _, _| {
                if let Some(this) = weak.upgrade() {
                    this.held.set(true);
                }
            }
        });
        hold.connect_released({
            let weak = Rc::downgrade(self);
            move |_, _, _, _| {
                if let Some(this) = weak.upgrade() {
                    this.held.set(false);
                }
            }
        });
        hold.connect_stopped({
            let weak = Rc::downgrade(self);
            move |_| {
                if let Some(this) = weak.upgrade() {
                    this.held.set(false);
                }
            }
        });
        self.size.add_controller(hold);
        for (overlay, check) in &self.overlays {
            let (weak, overlay) = (Rc::downgrade(self), *overlay);
            check.connect_toggled(move |c| {
                if let Some(this) = weak.upgrade() {
                    this.send(Change::Flag(overlay_write(overlay, c.is_active())));
                }
            });
        }
        // a radio sends only when it becomes active, so the deactivation of its twin sends nothing
        for (radio, number) in [(&self.source_title, false), (&self.number_title, true)] {
            let weak = Rc::downgrade(self);
            radio.connect_toggled(move |r| {
                if let (true, Some(this)) = (r.is_active(), weak.upgrade()) {
                    this.send(Change::Flag(title_write(number)));
                }
            });
        }
    }

    fn send(&self, change: Change) {
        if !self.updating.get() {
            (self.on_change)(change);
        }
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

    /// Redraws the three tabs from the season flag word.
    pub fn update(&self, word: i64, downloaded_only: bool) {
        self.updating.set(true);
        self.fill_filter(&filter_rows(word, downloaded_only), downloaded_only);
        self.fill_sort(word);
        self.fill_display(word);
        self.updating.set(false);
    }

    fn fill_filter(&self, rows: &[FilterRow], downloaded_only: bool) {
        clear(&self.filter);
        for row in rows {
            let (button, _) = icon_row(&tr(row.label), tri_icon(row.state));
            button.set_sensitive(row.sensitive);
            let (on_change, row) = (Rc::clone(&self.on_change), *row);
            button.connect_clicked(move |_| {
                if let Some(write) = filter_click(row.kind, row.state, downloaded_only) {
                    on_change(Change::Flag(write));
                }
            });
            self.filter.append(&button);
        }
    }

    fn fill_sort(&self, word: i64) {
        clear(&self.sort);
        for row in sort_rows(word) {
            let icon = match row.descending {
                None => "",
                Some(true) => "pan-down-symbolic",
                Some(false) => "pan-up-symbolic",
            };
            let (button, _) = icon_row(&tr(row.label), icon);
            let (on_change, code) = (Rc::clone(&self.on_change), row.code);
            button.connect_clicked(move |_| on_change(Change::Sort(code)));
            self.sort.append(&button);
        }
    }

    fn fill_display(&self, word: i64) {
        let mode = grid_mode(word);
        for (chip_mode, chip) in &self.chips {
            chip.set_active(*chip_mode == mode);
        }
        self.size_title.set_label(&tr(size_label(mode)));
        let size = grid_size(word);
        self.size_text.set_label(&size_value(size));
        if !self.held.get() {
            self.size.set_value(size as f64);
        }
        for (overlay, check) in &self.overlays {
            check.set_active(overlay_on(word, *overlay));
        }
        let number = title_is_number(word);
        self.source_title.set_active(!number);
        self.number_title.set_active(number);
    }
}
