//! The extension stores page as data (T-19a-048). The GTK page draws what this module decides: the state, the title and
//! header buttons, the row order, and each row's buttons with their tooltips (p1-ui-parity.md sections 1 to 3, K-7, K-8).

use crate::anime::model::StoreRow;

pub const PAGE_TITLE: &str = "Extension stores";
pub const EMPTY_TEXT: &str = "You haven't added an extension store yet";
pub const REFRESH_TOOLTIP: &str = "Refresh";
pub const ADD_LABEL: &str = "Add";

/// A button of one store row, in the order the row shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowButton {
    OpenWebsite,
    Discord,
    Copy,
    Delete,
}

impl RowButton {
    /// The tooltip of the icon-only button (section 7).
    pub fn tooltip(self) -> &'static str {
        match self {
            Self::OpenWebsite => "Open in browser",
            Self::Discord => "Discord",
            Self::Copy => "Copy to clipboard",
            Self::Delete => "Delete",
        }
    }
}

/// One store row: its name, the index URL as the secondary line (K-8), and its buttons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreRowView {
    pub name: String,
    pub index_url: String,
    pub buttons: Vec<RowButton>,
}

/// What the page shows: the empty state, or the rows in store order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PageContent {
    Empty { text: &'static str },
    List(Vec<StoreRowView>),
}

/// The page for the stores of the table, in the order the table gives them.
pub fn page(stores: &[StoreRow]) -> PageContent {
    if stores.is_empty() {
        return PageContent::Empty { text: EMPTY_TEXT };
    }
    PageContent::List(
        stores
            .iter()
            .map(|store| StoreRowView {
                name: store.name.clone(),
                index_url: store.index_url.clone(),
                buttons: row_buttons(store),
            })
            .collect(),
    )
}

/// The buttons of one row. The website button is hidden when the website is blank (K-7); the discord button only
/// appears when a discord value exists (K-7). Copy and delete are always shown.
pub fn row_buttons(store: &StoreRow) -> Vec<RowButton> {
    let mut buttons = Vec::with_capacity(4);
    if !store.contact_website.trim().is_empty() {
        buttons.push(RowButton::OpenWebsite);
    }
    if store.contact_discord.as_deref().is_some_and(|discord| !discord.trim().is_empty()) {
        buttons.push(RowButton::Discord);
    }
    buttons.push(RowButton::Copy);
    buttons.push(RowButton::Delete);
    buttons
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str, website: &str, discord: Option<&str>) -> StoreRow {
        StoreRow {
            index_url: format!("https://{name}.example/index.min.json"),
            name: name.into(),
            badge_label: name.into(),
            signing_key: "00".into(),
            contact_website: website.into(),
            contact_discord: discord.map(str::to_owned),
            is_legacy: false,
            extension_list_url: None,
        }
    }

    #[test]
    fn no_stores_shows_the_empty_text() {
        assert_eq!(page(&[]), PageContent::Empty { text: "You haven't added an extension store yet" });
    }

    #[test]
    fn rows_keep_the_table_order_and_show_the_index_url_as_subtitle() {
        let PageContent::List(rows) = page(&[store("b", "https://b.example", None), store("a", "https://a.example", None)])
        else {
            panic!("expected rows");
        };
        assert_eq!(rows.iter().map(|r| r.name.as_str()).collect::<Vec<_>>(), ["b", "a"]);
        assert_eq!(rows[0].index_url, "https://b.example/index.min.json");
    }

    #[test]
    fn the_website_button_is_hidden_when_the_website_is_empty() {
        assert_eq!(row_buttons(&store("a", "", None)), vec![RowButton::Copy, RowButton::Delete]);
        assert_eq!(row_buttons(&store("a", "  ", None)), vec![RowButton::Copy, RowButton::Delete]);
    }

    #[test]
    fn the_buttons_come_in_the_fixed_order_with_discord_only_when_present() {
        assert_eq!(
            row_buttons(&store("a", "https://a.example", Some("https://discord.example/x"))),
            vec![RowButton::OpenWebsite, RowButton::Discord, RowButton::Copy, RowButton::Delete]
        );
        assert_eq!(
            row_buttons(&store("a", "https://a.example", None)),
            vec![RowButton::OpenWebsite, RowButton::Copy, RowButton::Delete]
        );
    }

    #[test]
    fn every_icon_button_has_its_tooltip() {
        assert_eq!(RowButton::OpenWebsite.tooltip(), "Open in browser");
        assert_eq!(RowButton::Copy.tooltip(), "Copy to clipboard");
        assert_eq!(RowButton::Delete.tooltip(), "Delete");
        assert_eq!((PAGE_TITLE, REFRESH_TOOLTIP, ADD_LABEL), ("Extension stores", "Refresh", "Add"));
    }
}
