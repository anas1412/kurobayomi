//! Anime extension stores, read side only (`extension_store` table). The app no longer writes this table: repositories
//! are links (`kuro_extensions::repo`). The rows an old database carries are still listed, so the anime extension
//! host can tell which store an installed package came from.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_anime::{ExtensionStore, extension_stores};

use crate::DataResult;

/// Repository over the `extension_store` table of the anime database (read only).
#[derive(Clone)]
pub struct ExtensionStoreRepo {
    db: Arc<Db>,
}

impl ExtensionStoreRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Every stored row, in insertion order (`ORDER BY rowid`).
    pub fn all(&self) -> DataResult<Vec<ExtensionStore>> {
        self.db.read(|c| Ok(extension_stores(c)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::anime_db;
    use kuro_db::statements_anime::upsert_extension_store;

    fn store(url: &str) -> ExtensionStore {
        ExtensionStore {
            index_url: url.into(),
            name: "S".into(),
            badge_label: "B".into(),
            signing_key: "k".into(),
            contact_website: "https://site.test".into(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
            resolved_index_url: url.into(),
        }
    }

    #[test]
    fn listing_is_in_insertion_order() {
        let (_dir, db) = anime_db();
        for url in ["https://b.test/index.min.json", "https://a.test/index.min.json"] {
            db.write(|tx| Ok(upsert_extension_store(tx, &store(url)).map(drop)?)).expect("insert");
        }
        let urls: Vec<String> = ExtensionStoreRepo::new(db).all().expect("all").into_iter().map(|s| s.index_url).collect();
        assert_eq!(urls, ["https://b.test/index.min.json", "https://a.test/index.min.json"]);
    }
}
