//! Player custom buttons (`CustomButtonRepositoryImpl.kt`, `custom_buttons`). Every write here is one transaction. Which
//! buttons may change, and in what order, is decided by `kuro-domain::custom_buttons`.

use std::sync::Arc;

use kuro_db::handle::Db;
use kuro_db::statements_anime as sa;
use kuro_db::subscribe::Subscription;
use rusqlite::Connection;

pub use kuro_db::statements_anime::CustomButton;

use crate::{DataResult, ENTRY_DEBOUNCE};

/// Tables a button list reads.
const BUTTON_TABLES: &[&str] = &["custom_buttons"];

/// A partial update of one button. `None` keeps the stored column.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CustomButtonUpdate {
    pub id: i64,
    pub name: Option<String>,
    pub is_favorite: Option<bool>,
    pub sort_index: Option<i64>,
    pub content: Option<String>,
    pub long_press_content: Option<String>,
    pub on_startup: Option<String>,
}

impl CustomButtonUpdate {
    pub fn new(id: i64) -> Self {
        Self { id, ..Default::default() }
    }

    fn to_row(&self) -> sa::CustomButtonUpdate {
        sa::CustomButtonUpdate {
            name: self.name.clone(),
            is_favorite: self.is_favorite,
            sort_index: self.sort_index,
            content: self.content.clone(),
            long_press_content: self.long_press_content.clone(),
            on_startup: self.on_startup.clone(),
        }
    }
}

/// Buttons in `sortIndex` order, ties by id (the stored order is not guaranteed on ties).
fn sorted(mut rows: Vec<CustomButton>) -> Vec<CustomButton> {
    rows.sort_by_key(|b| (b.sort_index, b.id));
    rows
}

/// Ids in the current order.
fn ids_in_order(c: &Connection) -> rusqlite::Result<Vec<i64>> {
    Ok(sorted(sa::custom_buttons(c)?).iter().map(|b| b.id).collect())
}

/// Writes `sortIndex = position` for each id in `ids`.
fn write_positions(c: &Connection, ids: &[i64]) -> rusqlite::Result<()> {
    for (position, id) in ids.iter().enumerate() {
        let update = sa::CustomButtonUpdate { sort_index: Some(position as i64), ..Default::default() };
        sa::update_custom_button(c, *id, &update)?;
    }
    Ok(())
}

/// Typed access to `custom_buttons`. Shares one `Db` with the other anime repositories.
#[derive(Clone)]
pub struct CustomButtonRepo {
    db: Arc<Db>,
}

impl CustomButtonRepo {
    pub fn new(db: Arc<Db>) -> Self {
        Self { db }
    }

    /// Every button in `sortIndex` order, ties by id.
    pub fn get_all(&self) -> DataResult<Vec<CustomButton>> {
        Ok(self.db.read(|c| Ok(sorted(sa::custom_buttons(c)?)))?)
    }

    /// The button list, published again after each write to the table.
    pub fn subscribe_all(&self) -> Subscription<Vec<CustomButton>> {
        self.db.subscribe(BUTTON_TABLES, ENTRY_DEBOUNCE, |c| Ok(sorted(sa::custom_buttons(c)?)))
    }

    /// Inserts a button with `sortIndex = max + 1` (0 when none) and `isFavorite = false`. Returns its id.
    pub fn create(&self, name: &str, content: &str, long_press_content: &str, on_startup: &str) -> DataResult<i64> {
        self.db.write(|tx| Ok(sa::create_custom_button(tx, name, content, long_press_content, on_startup)?))
    }

    /// Coalesce update of one button. Returns the rows changed: `0` for an unknown id.
    pub fn update(&self, u: &CustomButtonUpdate) -> DataResult<usize> {
        self.db.write(|tx| Ok(sa::update_custom_button(tx, u.id, &u.to_row())?))
    }

    /// Every update in one transaction: one failure rolls them all back.
    pub fn update_all(&self, updates: &[CustomButtonUpdate]) -> DataResult<()> {
        self.db.write(|tx| {
            for u in updates {
                sa::update_custom_button(tx, u.id, &u.to_row())?;
            }
            Ok(())
        })
    }

    /// Deletes one button and renumbers the rest `0..n-1` in their order, in one transaction. Returns the rows deleted.
    pub fn delete(&self, id: i64) -> DataResult<usize> {
        self.db.write(|tx| {
            let removed = tx.prepare_cached("DELETE FROM custom_buttons WHERE _id = ?1")?.execute([id])?;
            let ids = ids_in_order(tx)?;
            write_positions(tx, &ids)?;
            Ok(removed)
        })
    }

    /// Writes `sortIndex = position` for each id in `ids`, in one transaction.
    pub fn set_order(&self, ids: &[i64]) -> DataResult<()> {
        self.db.write(|tx| Ok(write_positions(tx, ids)?))
    }

    /// Makes `id` the only favourite, in one transaction.
    pub fn make_only_favorite(&self, id: i64) -> DataResult<()> {
        self.db.write(|tx| {
            tx.execute("UPDATE custom_buttons SET isFavorite = 0 WHERE _id != ?1", [id])?;
            tx.execute("UPDATE custom_buttons SET isFavorite = 1 WHERE _id = ?1", [id])?;
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::support::anime_db;

    fn repo() -> (tempfile::TempDir, CustomButtonRepo) {
        let (dir, db) = anime_db();
        (dir, CustomButtonRepo::new(db))
    }

    fn names(rows: &[CustomButton]) -> Vec<String> {
        rows.iter().map(|b| b.name.clone()).collect()
    }

    #[test]
    fn a_fresh_database_holds_the_seed_button_with_its_three_snippets() {
        let (_d, repo) = repo();
        let all = repo.get_all().unwrap();
        assert_eq!(all.len(), 1);
        let seed = &all[0];
        assert_eq!((seed.id, seed.name.as_str(), seed.is_favorite, seed.sort_index), (1, "+85 s", true, 0));
        assert!(seed.on_startup.contains("\n\t"), "tab characters are kept byte for byte");
        assert!(!seed.content.is_empty() && !seed.long_press_content.is_empty());
    }

    #[test]
    fn create_appends_after_the_highest_index_and_is_never_a_favourite() {
        let (_d, repo) = repo();
        let id = repo.create("B", "mp.pause()", "", "").unwrap();
        let all = repo.get_all().unwrap();
        let created = all.iter().find(|b| b.id == id).unwrap();
        assert_eq!((created.sort_index, created.is_favorite), (1, false));
        assert_eq!(names(&all), vec!["+85 s", "B"]);
    }

    #[test]
    fn update_keeps_the_columns_it_does_not_name_and_reports_unknown_ids() {
        let (_d, repo) = repo();
        let id = repo.create("B", "a", "b", "c").unwrap();
        let changed = repo.update(&CustomButtonUpdate { content: Some("new".into()), ..CustomButtonUpdate::new(id) }).unwrap();
        assert_eq!(changed, 1);
        let got = repo.get_all().unwrap().into_iter().find(|b| b.id == id).unwrap();
        assert_eq!((got.name.as_str(), got.content.as_str(), got.long_press_content.as_str()), ("B", "new", "b"));
        assert_eq!(repo.update(&CustomButtonUpdate::new(999)).unwrap(), 0);
    }

    #[test]
    fn update_all_applies_every_change_or_none() {
        let (_d, repo) = repo();
        let id = repo.create("B", "", "", "").unwrap();
        repo.update_all(&[
            CustomButtonUpdate { name: Some("X".into()), ..CustomButtonUpdate::new(id) },
            CustomButtonUpdate { name: Some("Y".into()), ..CustomButtonUpdate::new(1) },
        ])
        .unwrap();
        assert_eq!(names(&repo.get_all().unwrap()), vec!["Y", "X"]);
    }

    #[test]
    fn delete_renumbers_the_survivors_from_zero() {
        let (_d, repo) = repo();
        let b = repo.create("B", "", "", "").unwrap();
        let c = repo.create("C", "", "", "").unwrap();
        assert_eq!(repo.delete(1).unwrap(), 1);
        let all = repo.get_all().unwrap();
        assert_eq!(all.iter().map(|x| (x.id, x.sort_index)).collect::<Vec<_>>(), vec![(b, 0), (c, 1)]);
        assert_eq!(repo.delete(b).unwrap(), 1);
        assert_eq!(repo.get_all().unwrap()[0].sort_index, 0);
        assert_eq!(repo.delete(c).unwrap(), 1);
        assert!(repo.get_all().unwrap().is_empty());
    }

    #[test]
    fn set_order_writes_positions_and_get_all_follows_them() {
        let (_d, repo) = repo();
        let b = repo.create("B", "", "", "").unwrap();
        let c = repo.create("C", "", "", "").unwrap();
        repo.set_order(&[c, 1, b]).unwrap();
        assert_eq!(names(&repo.get_all().unwrap()), vec!["C", "+85 s", "B"]);
    }

    #[test]
    fn make_only_favorite_clears_every_other_favourite() {
        let (_d, repo) = repo();
        let b = repo.create("B", "", "", "").unwrap();
        repo.make_only_favorite(b).unwrap();
        let favs: Vec<i64> = repo.get_all().unwrap().iter().filter(|x| x.is_favorite).map(|x| x.id).collect();
        assert_eq!(favs, vec![b]);
    }

    /// The next publication of the list, or `None` when none comes within half a second.
    async fn next(sub: &mut Subscription<Vec<CustomButton>>) -> Option<Vec<String>> {
        let published = tokio::time::timeout(std::time::Duration::from_millis(500), sub.changed()).await.ok()??;
        Some(names(&published.expect("the query succeeded")))
    }

    #[tokio::test]
    async fn the_list_is_published_at_start_and_after_each_write() {
        let (_d, repo) = repo();
        let mut sub = repo.subscribe_all();
        assert_eq!(next(&mut sub).await, Some(vec!["+85 s".to_owned()]));
        repo.create("B", "", "", "").unwrap();
        assert_eq!(next(&mut sub).await, Some(vec!["+85 s".to_owned(), "B".to_owned()]));
    }
}
