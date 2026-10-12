//! Category reads and live queries for one kind: the domain row shape and error over the shared read repository
//! (`kuro_data::category::CategoryRepo`, statements `kuro_db::tables::categories`). Every list is ordered `sort, _id`,
//! so the Default category (id 0) stays first when two rows tie on `sort`. Writes are in
//! [`super::service::CategoryService`], the only write path.

use kuro_platform::kind::Kind;
use std::sync::Arc;
use std::time::Duration;

use kuro_data::category::CategoryRepo as SharedRepo;
use kuro_db::handle::Db;
use kuro_db::subscribe::Subscription;
use kuro_db::tables::CategoryRow;
use kuro_db::tables::categories as table;

use super::model::Category;
use crate::error::DomainError;

/// Quiet time before a category query re-runs after a write (well under the 100 ms contract).
const CATEGORY_DEBOUNCE: Duration = Duration::from_millis(20);

/// Tables a category list reads, with the memberships.
const CATEGORY_TABLES: &[&str] = &["categories", "entry_categories"];

/// Read side of the categories of one kind.
#[derive(Clone)]
pub struct CategoryRepo {
    db: Arc<Db>,
    kind: Kind,
    shared: SharedRepo,
}

impl From<kuro_data::category::Category> for Category {
    fn from(c: kuro_data::category::Category) -> Self {
        Self { id: c.id, name: c.name, order: c.order, flags: c.flags, hidden: c.hidden }
    }
}

impl From<CategoryRow> for Category {
    fn from(r: CategoryRow) -> Self {
        Self { id: r.id, name: r.name, order: r.order, flags: r.flags, hidden: r.hidden }
    }
}

fn domain<T: Into<Category>>(rows: Vec<T>) -> Vec<Category> {
    rows.into_iter().map(Into::into).collect()
}

impl CategoryRepo {
    pub fn new(db: Arc<Db>, kind: Kind) -> Self {
        Self { shared: SharedRepo::new(db.clone(), kind), db, kind }
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub(crate) fn db(&self) -> &Arc<Db> {
        &self.db
    }

    /// One category by id.
    pub fn get(&self, id: i64) -> Result<Option<Category>, DomainError> {
        Ok(self.shared.get(id)?.map(Category::from))
    }

    /// Every category, the system row first.
    pub fn all(&self) -> Result<Vec<Category>, DomainError> {
        Ok(domain(self.shared.all()?))
    }

    /// Every category except the hidden ones.
    pub fn visible(&self) -> Result<Vec<Category>, DomainError> {
        Ok(domain(self.shared.all_visible()?))
    }

    /// The categories of one entry, in display order.
    pub fn by_entry(&self, entry_id: i64, visible_only: bool) -> Result<Vec<Category>, DomainError> {
        Ok(domain(if visible_only { self.shared.visible_by_entry(entry_id)? } else { self.shared.by_entry(entry_id)? }))
    }

    /// Live list of every category. The subscription publishes only when the list changes.
    pub fn watch_all(&self) -> Subscription<Vec<Category>> {
        let kind = self.kind;
        self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, move |c| Ok(domain(table::all(c, kind)?)))
    }

    /// Live list of the visible categories.
    pub fn watch_visible(&self) -> Subscription<Vec<Category>> {
        let kind = self.kind;
        self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, move |c| Ok(domain(table::visible(c, kind)?)))
    }

    /// Live list of the categories of one entry. Changes to memberships and to the categories both re-run it.
    pub fn watch_by_entry(&self, entry_id: i64, visible_only: bool) -> Subscription<Vec<Category>> {
        let kind = self.kind;
        self.db.subscribe(CATEGORY_TABLES, CATEGORY_DEBOUNCE, move |c| {
            Ok(domain(if visible_only { table::visible_by_entry(c, kind, entry_id)? } else { table::by_entry(c, kind, entry_id)? }))
        })
    }
}
