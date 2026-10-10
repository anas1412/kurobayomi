//! The Statistics page of one kind (T-42a-043): four section cards in order, each a heading and a row of equal tiles.
//! A spinner shows while the snapshot loads. The values and captions come from [`stats::cards`]; this file only lays
//! them out. The data is the caller's (the aggregation queries live with the data layer).

use gtk::prelude::*;
use gtk::{Align, Box as GBox, Image, Label, Orientation as O, Spinner, Stack};

use crate::updates::stats::{self, Card, Failed, StatsState, Tile};

pub struct StatsScreen {
    root: Stack,
    content: GBox,
}

impl StatsScreen {
    /// The page, showing its spinner until the first [`StatsScreen::set_state`].
    pub fn new() -> Self {
        let root = Stack::new();
        let spinner = Spinner::builder().spinning(true).halign(Align::Center).valign(Align::Center).vexpand(true).build();
        root.add_named(&spinner, Some("loading"));
        let content = GBox::new(O::Vertical, 12);
        crate::ui::margins(&content, 12);
        root.add_named(&crate::ui::scrolled(&content), Some("content"));
        root.set_visible_child_name("loading");
        Self { root, content }
    }

    pub fn widget(&self) -> &Stack {
        &self.root
    }

    /// Shows `state`: the spinner while loading, else the cards in order.
    pub fn set_state(&self, state: &StatsState) {
        self.set_state_with(state, Failed::default());
    }

    /// Shows an aggregation result. `None` (the task was dropped) leaves the page as it is.
    pub fn set_loaded(&self, loaded: Option<crate::updates::stats_data::Loaded>) {
        if let Some(loaded) = loaded {
            self.set_state_with(&loaded.state, loaded.failed);
        }
    }

    /// Like [`StatsScreen::set_state`], with the failed sections showing "N/A".
    pub fn set_state_with(&self, state: &StatsState, failed: Failed) {
        while let Some(child) = self.content.first_child() {
            self.content.remove(&child);
        }
        let cards = stats::cards_with(state, failed);
        if cards.is_empty() {
            self.root.set_visible_child_name("loading");
            return;
        }
        for card in &cards {
            self.content.append(&card_widget(card));
        }
        self.root.set_visible_child_name("content");
    }
}

impl Default for StatsScreen {
    fn default() -> Self {
        Self::new()
    }
}

/// One section card: the heading, then the tiles in one equal-width row.
fn card_widget(card: &Card) -> GBox {
    let col = GBox::new(O::Vertical, 6);
    col.append(&crate::ui::heading(&card.title));
    let row = GBox::new(O::Horizontal, 6);
    row.set_homogeneous(true);
    for tile in &card.tiles {
        row.append(&tile_widget(tile));
    }
    col.append(&row);
    col
}

/// One tile: the icon when it has one, the value in bold, and the caption dimmed.
fn tile_widget(tile: &Tile) -> GBox {
    let col = GBox::new(O::Vertical, 4);
    col.add_css_class("card");
    crate::ui::margins(&col, 10);
    if let Some(icon) = tile.icon {
        col.append(&Image::from_icon_name(icon));
    }
    col.append(&Label::builder().label(&tile.value).css_classes(["title-4"]).build());
    col.append(&crate::ui::dim(&tile.caption));
    col
}
