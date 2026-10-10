//! One library cover card, built for the active display mode (T-37a-018 to T-37a-020). Thin: every number and
//! string comes from `layout`, and the click handling lives in the view.

use gtk::prelude::*;
use gtk::{Align, Orientation as O};
use kuro_domain::library::columns::COLUMN_WIDTH_PX;
use kuro_domain::library::page::LibraryItem;
use kuro_prefs::library::LibraryDisplayMode;

use crate::library::layout;

/// The widgets of one card. `continue_button` is present when the card shows one.
pub struct Card {
    pub root: gtk::Widget,
    pub continue_button: Option<gtk::Button>,
}

/// Builds the card of `item`. `selected` styles it as selected; `show_continue` adds the continue button.
pub fn build(item: &LibraryItem, mode: LibraryDisplayMode, selected: bool, show_continue: bool) -> Card {
    let width = COLUMN_WIDTH_PX;
    let cover = crate::ui::cover(width, layout::cover_height(width));
    cover.add_css_class("library-cover");
    if selected {
        cover.set_opacity(0.76);
    }
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&cover));

    let badges = badge_group(item);
    badges.set_halign(Align::Start);
    badges.set_valign(Align::Start);
    overlay.add_overlay(&badges);

    let language = language_badge(item);
    language.set_halign(Align::End);
    language.set_valign(Align::Start);
    overlay.add_overlay(&language);

    let continue_button = show_continue.then(|| {
        let size = layout::continue_size(mode);
        let button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        button.set_css_classes(&["circular", "osd"]);
        button.set_size_request(size, size);
        button.set_tooltip_text(Some("Resume"));
        button.set_halign(Align::End);
        button.set_valign(Align::End);
        button.set_margin_end(6);
        button.set_margin_bottom(6);
        overlay.add_overlay(&button);
        button
    });

    let root = gtk::Box::new(O::Vertical, 4);
    root.set_size_request(width, -1);
    root.add_css_class("library-card");
    if selected {
        root.add_css_class("selected");
    }
    root.append(&overlay);

    match mode {
        LibraryDisplayMode::CompactGrid => {
            if layout::shows_title_overlay(mode) {
                let title = title_label(&item.row.title, 2, 8);
                title.add_css_class("title-scrim");
                title.set_valign(Align::End);
                overlay.add_overlay(&title);
            }
        }
        LibraryDisplayMode::ComfortableGrid => root.append(&title_label(&item.row.title, 2, 4)),
        LibraryDisplayMode::CoverOnlyGrid | LibraryDisplayMode::List => {}
    }
    Card { root: root.upcast(), continue_button }
}

/// Builds the row of `item` for List mode (T-37a-021): a square cover, the title, the badges, then the continue
/// button. `row_height` comes from `layout::list_row_height`.
pub fn build_list(item: &LibraryItem, selected: bool, show_continue: bool, row_height: f64) -> Card {
    let height = row_height.round() as i32;
    let edge = (height - 8).max(40);
    let cover = crate::ui::cover(edge, edge);
    cover.add_css_class("library-cover");
    if selected {
        cover.set_opacity(0.76);
    }
    let root = gtk::Box::new(O::Horizontal, 8);
    root.set_size_request(-1, height);
    root.set_margin_start(8);
    root.set_margin_end(8);
    root.add_css_class("library-card");
    if selected {
        root.add_css_class("selected");
    }
    root.append(&cover);

    let title = gtk::Label::new(Some(&item.row.title));
    title.set_hexpand(true);
    title.set_xalign(0.0);
    title.set_valign(Align::Center);
    title.set_wrap(true);
    title.set_lines(2);
    title.set_ellipsize(gtk::pango::EllipsizeMode::End);
    root.append(&title);

    let badges = badge_group(item);
    badges.set_valign(Align::Center);
    root.append(&badges);
    let language = language_badge(item);
    language.set_valign(Align::Center);
    root.append(&language);

    let continue_button = show_continue.then(|| {
        let size = layout::continue_size(LibraryDisplayMode::List);
        let button = gtk::Button::from_icon_name("media-playback-start-symbolic");
        button.set_css_classes(&["circular", "osd"]);
        button.set_size_request(size, size);
        button.set_tooltip_text(Some("Resume"));
        button.set_valign(Align::Center);
        root.append(&button);
        button
    });
    Card { root: root.upcast(), continue_button }
}

fn title_label(text: &str, lines: i32, inset: i32) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_lines(lines);
    label.set_ellipsize(gtk::pango::EllipsizeMode::End);
    label.set_max_width_chars(14);
    label.set_xalign(0.0);
    label.set_margin_start(inset);
    label.set_margin_end(inset);
    label.set_margin_bottom(inset / 2);
    label
}

/// Download count and unread count, top start. Hidden when both are zero or not computed.
fn badge_group(item: &LibraryItem) -> gtk::Box {
    let group = gtk::Box::new(O::Horizontal, 2);
    group.set_margin_start(4);
    group.set_margin_top(4);
    if let Some(text) = layout::count_badge(item.download_count) {
        group.append(&badge(&text, "badge-download"));
    }
    if let Some(text) = layout::count_badge(item.unread_count) {
        group.append(&badge(&text, "badge-unread"));
    }
    group
}

/// Local folder icon, or the source language, top end.
fn language_badge(item: &LibraryItem) -> gtk::Box {
    let group = gtk::Box::new(O::Horizontal, 2);
    group.set_margin_end(4);
    group.set_margin_top(4);
    if item.is_local {
        let icon = gtk::Image::from_icon_name("folder-symbolic");
        icon.add_css_class("badge-language");
        group.append(&icon);
    } else if let Some(text) = layout::language_badge(&item.source_language) {
        group.append(&badge(&text, "badge-language"));
    }
    group
}

fn badge(text: &str, class: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("badge");
    label.add_css_class(class);
    label
}
