//! Category reads and live queries for one kind. Every list is ordered `sort, _id`, so the Default category
//! (id 0) stays first when two rows tie on `sort`. The by-entry lists are ordered in Rust with the same key
//! (the upstream queries have no ORDER BY).

use kuro_platform::kind::Kind;
use std::sync::Arc;
use std::time::Duration;

use kuro_db::handle::Db;
use kuro_db::statements_anime as anime;
use kuro_db::statements_manga::categories as manga;
use kuro_db::subscribe::Subscription;

use super::model::Category;
use crate::error::DomainError;

/// Quiet time before a category query re-runs after a write (well under the 100 ms contract).
const CATEGORY_DEBOUNCE: Duration = Duration::from_millis(20);

/// Tables a category list reads.
const CATEGORY_TABLES: &[&str] = &["categories"];

/// Tables a by-entry category list reads.
const MEMBERSHIP_TABLES: &[&str] = &["categories", "entry_categories"];

/// Read side of the categories of one kind. Writes are in [`super::service::CategoryService`].
#[derive(Clone)]
pub struct CategoryRepo {
    db: Arc<Db>,
    kind: Kind,
}

fn by_order(mut rows: Vec<Category>) -> Vec<Category> {
    rows.sort_by_key(|c| (c.order, c.id));
    rows
}

impl CategoryRepo {
    pub fn new(db: Arc<Db>, kind: Kind) -> Self {
        Self { db, kind }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn db(&self) -> &Arc<Db> {
        &self.db
    }

    /// One category by id.
    pub fn get(&self, id: i64) -> Result<Option<Category>, DomainError> {
        Ok(match self.kind {
            Kind::Manga => self
                .db
                .read(|c| Ok(manga::get_category(c, id)?.map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })))?,
            Kind::Anime => self
                .db
                .read(|c| Ok(anime::category_by_id(c, id)?.map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })))?,
        })
    }

    /// Every category, Default included.
    pub fn all(&self) -> Result<Vec<Category>, DomainError> {
        Ok(match self.kind {
            Kind::Manga => self.db.read(|c| {
                Ok(manga::get_categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            })?,
            Kind::Anime => self.db.read(|c| {
                Ok(anime::categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            })?,
        })
    }

    /// Categories with `hidden = 0`, Default included.
    pub fn visible(&self) -> Result<Vec<Category>, DomainError> {
        Ok(match self.kind {
            Kind::Manga => self.db.read(|c| {
                Ok(manga::get_visible_categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            })?,
            Kind::Anime => self.db.read(|c| {
                Ok(anime::visible_categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            })?,
        })
    }

    /// The categories of one entry, in `sort, _id` order. `visible_only` drops hidden ones.
    pub fn by_entry(&self, entry_id: i64, visible_only: bool) -> Result<Vec<Category>, DomainError> {
        let rows = match (self.kind, visible_only) {
            (Kind::Manga, false) => self.db.read(|c| {
                Ok(manga::get_categories_by_manga_id(c, entry_id)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect::<Vec<_>>())
            })?,
            (Kind::Manga, true) => self.db.read(|c| {
                Ok(manga::get_visible_categories_by_manga_id(c, entry_id)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect::<Vec<_>>())
            })?,
            (Kind::Anime, false) => self.db.read(|c| {
                Ok(anime::categories_by_anime_id(c, entry_id)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect::<Vec<_>>())
            })?,
            (Kind::Anime, true) => self.db.read(|c| {
                Ok(anime::visible_categories_by_anime_id(c, entry_id)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect::<Vec<_>>())
            })?,
        };
        Ok(by_order(rows))
    }

    /// Live list of every category. The subscription publishes only when the list changes.
    pub fn watch_all(&self) -> Subscription<Vec<Category>> {
        match self.kind {
            Kind::Manga => self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, |c| {
                Ok(manga::get_categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            }),
            Kind::Anime => self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, |c| {
                Ok(anime::categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            }),
        }
    }

    /// Live list of the visible categories.
    pub fn watch_visible(&self) -> Subscription<Vec<Category>> {
        match self.kind {
            Kind::Manga => self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, |c| {
                Ok(manga::get_visible_categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            }),
            Kind::Anime => self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, |c| {
                Ok(anime::visible_categories(c)?
                    .into_iter()
                    .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                    .collect())
            }),
        }
    }

    /// Live list of the categories of one entry. Changes to memberships and to the categories both re-run it.
    pub fn watch_by_entry(&self, entry_id: i64, visible_only: bool) -> Subscription<Vec<Category>> {
        match (self.kind, visible_only) {
            (Kind::Manga, false) => self.db.subscribe(MEMBERSHIP_TABLES, CATEGORY_DEBOUNCE, move |c| {
                Ok(by_order(
                    manga::get_categories_by_manga_id(c, entry_id)?
                        .into_iter()
                        .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                        .collect(),
                ))
            }),
            (Kind::Manga, true) => self.db.subscribe(MEMBERSHIP_TABLES, CATEGORY_DEBOUNCE, move |c| {
                Ok(by_order(
                    manga::get_visible_categories_by_manga_id(c, entry_id)?
                        .into_iter()
                        .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                        .collect(),
                ))
            }),
            (Kind::Anime, false) => self.db.subscribe(MEMBERSHIP_TABLES, CATEGORY_DEBOUNCE, move |c| {
                Ok(by_order(
                    anime::categories_by_anime_id(c, entry_id)?
                        .into_iter()
                        .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                        .collect(),
                ))
            }),
            (Kind::Anime, true) => self.db.subscribe(MEMBERSHIP_TABLES, CATEGORY_DEBOUNCE, move |c| {
                Ok(by_order(
                    anime::visible_categories_by_anime_id(c, entry_id)?
                        .into_iter()
                        .map(|r| Category { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden })
                        .collect(),
                ))
            }),
        }
    }
}
