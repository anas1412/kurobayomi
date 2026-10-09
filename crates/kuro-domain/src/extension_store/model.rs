//! `AnimeExtensionStore` (`AnimeExtensionStore.kt`, T-09a-030) and its row conversion to the `kuro-db` record.

use kuro_db::statements_anime::ExtensionStore as Row;

/// The signing key of a store added from a preference entry, where no key is known (E-STO-08).
pub const NO_SIGNING_KEY: &str = "NO_SIGNING_KEY";

/// A third-party extension store the user added.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnimeExtensionStore {
    /// The URL the user entered. Also the row key.
    pub index_url: String,
    pub name: String,
    pub badge_label: String,
    pub signing_key: String,
    pub contact: Contact,
    /// `true` for a store in the legacy repo format (`repo.json`, `index.min.json`).
    pub is_legacy: bool,
    /// An explicit URL of the extension list, when the store publishes one apart from its index.
    pub extension_list_url: Option<String>,
    /// The document the store's data was read from: `index_url`, or the `indexV2` target it pointed at (Q-STORE-1).
    /// A non-legacy store's list is read from here, and a legacy store's base is derived from here.
    pub resolved_index_url: String,
}

/// The website and Discord link a store shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Contact {
    pub website: String,
    pub discord: Option<String>,
}

impl From<Row> for AnimeExtensionStore {
    fn from(row: Row) -> Self {
        Self {
            index_url: row.index_url,
            name: row.name,
            badge_label: row.badge_label,
            signing_key: row.signing_key,
            contact: Contact { website: row.contact_website, discord: row.contact_discord },
            is_legacy: row.is_legacy,
            extension_list_url: row.extension_list_url,
            resolved_index_url: row.resolved_index_url,
        }
    }
}

impl From<&AnimeExtensionStore> for Row {
    fn from(store: &AnimeExtensionStore) -> Self {
        Row {
            index_url: store.index_url.clone(),
            name: store.name.clone(),
            badge_label: store.badge_label.clone(),
            signing_key: store.signing_key.clone(),
            contact_website: store.contact.website.clone(),
            contact_discord: store.contact.discord.clone(),
            is_legacy: store.is_legacy,
            extension_list_url: store.extension_list_url.clone(),
            resolved_index_url: store.resolved_index_url.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_round_trips_every_column() {
        let store = AnimeExtensionStore {
            index_url: "https://a.test/index.min.json".into(),
            name: "A".into(),
            badge_label: "AB".into(),
            signing_key: "k".into(),
            contact: Contact { website: "https://a.test".into(), discord: Some("https://d.test".into()) },
            is_legacy: true,
            extension_list_url: Some("https://a.test/list.json".into()),
            resolved_index_url: "https://a.test/repo.json".into(),
        };
        let row = Row::from(&store);
        assert_eq!(AnimeExtensionStore::from(row), store);
    }
}
