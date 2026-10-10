//! The season grid of a `Seasons` entry (T-39b-064, T-39a-032 to T-39a-034): one tile per child season in a wrapping grid
//! whose minimum column count follows the entry's grid size setting. The tile rules (title, badges, continue button) and
//! the column rule come from `season_tile`; this widget only draws them. The widget is a `gtk::FlowBox`, so a handful of
//! seasons needs no virtualised list. Upstream: `AnimeScreen.kt` season cells, `AnimeSeasonListItem.kt`.

use std::rc::Rc;

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Label, Orientation as O};
use kuro_domain::entries::season_display::SeasonDisplayMode;

use super::season_tile::{self, SeasonItem, Tile, edge_badges, list_badges};
use crate::ui;

/// What decides how the grid draws and how many columns it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Layout {
    pub mode: SeasonDisplayMode,
    /// The season titles are "Season {n}" (`SEASON_DISPLAY_MODE_NUMBER`), else the source titles.
    pub number_mode: bool,
    /// The page has a continue callback, so a tile may show the continue button.
    pub has_continue: bool,
    pub columns: u32,
    /// A set size is also the maximum per line; an automatic one is only a minimum.
    pub fixed: bool,
    pub width: i32,
}

impl Layout {
    /// The columns of a grid of this width and size setting (list mode is one column, see `season_tile`).
    pub fn columns_for(width: i32, size: i64, mode: SeasonDisplayMode) -> u32 {
        season_tile::grid_columns(width, size, mode == SeasonDisplayMode::List)
    }
}

pub struct SeasonGrid {
    pub widget: gtk::FlowBox,
    on_open: Rc<dyn Fn(i64)>,
    on_continue: Option<Rc<dyn Fn(i64)>>,
}

impl SeasonGrid {
    pub fn new(on_open: Rc<dyn Fn(i64)>, on_continue: Option<Rc<dyn Fn(i64)>>) -> Self {
        let widget = gtk::FlowBox::builder()
            .selection_mode(gtk::SelectionMode::None)
            .homogeneous(true)
            .row_spacing(8)
            .column_spacing(8)
            .margin_start(16)
            .margin_end(16)
            .margin_bottom(8)
            .visible(false)
            .build();
        Self { widget, on_open, on_continue }
    }

    /// Replaces the tiles and applies the layout. List mode gives one full-width row per season.
    pub fn set(&self, items: &[SeasonItem], layout: &Layout) {
        while let Some(child) = self.widget.first_child() {
            self.widget.remove(&child);
        }
        self.widget.set_visible(!items.is_empty());
        self.widget.set_min_children_per_line(layout.columns);
        let max = if layout.fixed { layout.columns } else { (2 * season_tile::MAX_GRID_SIZE as u32).max(layout.columns) };
        self.widget.set_max_children_per_line(max);
        let list = layout.mode == SeasonDisplayMode::List;
        let cover_width = if list { 96 } else { ((layout.width - 32 - 8 * (layout.columns as i32 - 1)) / layout.columns.max(1) as i32).clamp(48, 220) };
        for item in items {
            let tile = season_tile::tile(item, layout.mode, layout.number_mode, layout.has_continue);
            let content = if list { self.list_row(&tile, cover_width) } else { self.grid_tile(&tile, cover_width) };
            let button = gtk::Button::builder().child(&content).css_classes(["flat"]).build();
            let (on_open, id) = (Rc::clone(&self.on_open), item.id);
            button.connect_clicked(move |_| on_open(id));
            if let (true, Some(on_continue)) = (tile.continue_button, &self.on_continue) {
                let (on_continue, id) = (Rc::clone(on_continue), item.id);
                let overlay = gtk::Overlay::new();
                overlay.set_child(Some(&button));
                let play = gtk::Button::builder().icon_name("media-playback-start-symbolic").css_classes(["circular", "osd"]).valign(Align::End).halign(Align::End).margin_end(8).margin_bottom(8).build();
                play.connect_clicked(move |_| on_continue(id));
                overlay.add_overlay(&play);
                self.widget.insert(&overlay, -1);
            } else {
                self.widget.insert(&button, -1);
            }
        }
    }

    fn grid_tile(&self, tile: &Tile, cover_width: i32) -> GBox {
        let content = GBox::new(O::Vertical, 4);
        // downloads and unseen sit on the start edge of the cover, the language on its end edge (T-39a-033)
        let cover = gtk::Overlay::new();
        cover.set_child(Some(&ui::cover(cover_width, cover_width * 3 / 2)));
        let edges = edge_badges(&tile.badges);
        for (texts, align) in [(edges.start, Align::Start), (edges.end.into_iter().collect(), Align::End)] {
            if texts.is_empty() {
                continue;
            }
            let row = GBox::new(O::Horizontal, 2);
            row.set_halign(align);
            row.set_valign(Align::Start);
            row.set_margin_top(4);
            row.set_margin_start(4);
            row.set_margin_end(4);
            for text in texts {
                row.append(&Label::builder().label(text).css_classes(["caption", "osd"]).build());
            }
            cover.add_overlay(&row);
        }
        content.append(&cover);
        if let Some(title) = &tile.title {
            content.append(&Label::builder().label(title).max_width_chars(14).wrap(true).lines(2).ellipsize(gtk::pango::EllipsizeMode::End).build());
        }
        content
    }

    fn list_row(&self, tile: &Tile, cover_width: i32) -> GBox {
        let row = GBox::new(O::Horizontal, 12);
        row.append(&ui::cover(cover_width, cover_width * 3 / 2));
        let text = GBox::new(O::Vertical, 4);
        text.set_hexpand(true);
        if let Some(title) = &tile.title {
            text.append(&Label::builder().label(title).xalign(0.0).ellipsize(gtk::pango::EllipsizeMode::End).css_classes(["heading"]).build());
        }
        let badges = list_badges(&tile.badges);
        if !badges.is_empty() {
            text.append(&Label::builder().label(badges.join("  ")).xalign(0.0).css_classes(["caption", "dim-label"]).build());
        }
        row.append(&text);
        row
    }
}
