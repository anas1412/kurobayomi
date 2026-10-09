//! Writes of the custom button list (`CreateCustomButton.kt`, `UpdateCustomButton.kt`, `DeleteCustomButton.kt`,
//! `ReorderCustomButton.kt`, `ToggleFavoriteCustomButton.kt`). Create, delete, reorder and favourite run under one ops
//! mutex, so two of them never interleave their read and their write (RC-06). The rows come from `kuro-data`.

use std::sync::Mutex;

use kuro_data::custom_button::{CustomButtonRepo, CustomButtonUpdate};

use super::CustomButton;
use crate::error::DomainError;

/// What a reorder or favourite call did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Changed,
    /// Unknown id, or the button is already where it was asked to go. Nothing was written.
    Unchanged,
}

/// Custom button writes over one repository. Holds the ops mutex.
pub struct CustomButtonService {
    repo: CustomButtonRepo,
    ops: Mutex<()>,
}

fn to_domain(b: kuro_data::custom_button::CustomButton) -> CustomButton {
    CustomButton {
        id: b.id,
        name: b.name,
        is_favorite: b.is_favorite,
        sort_index: b.sort_index,
        content: b.content,
        long_press_content: b.long_press_content,
        on_startup: b.on_startup,
    }
}

impl CustomButtonService {
    pub fn new(repo: CustomButtonRepo) -> Self {
        Self { repo, ops: Mutex::new(()) }
    }

    fn guard(&self) -> std::sync::MutexGuard<'_, ()> {
        self.ops.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Every button in `sortIndex` order.
    pub fn list(&self) -> Result<Vec<CustomButton>, DomainError> {
        Ok(self.repo.get_all()?.into_iter().map(to_domain).collect())
    }

    /// Creates a button. An empty name is rejected before anything is written. Returns the new id.
    pub fn create(
        &self,
        name: &str,
        content: &str,
        long_press_content: &str,
        on_startup: &str,
    ) -> Result<i64, DomainError> {
        if name.is_empty() {
            return Err(DomainError::Invalid("custom button name is empty"));
        }
        let _ops = self.guard();
        Ok(self.repo.create(name, content, long_press_content, on_startup)?)
    }

    /// Updates the columns named in `u`; the others keep their value. A failed write is logged at error level and
    /// returned.
    pub fn update(&self, u: &CustomButtonUpdate) -> Result<(), DomainError> {
        self.repo.update(u).map(drop).map_err(|e| {
            tracing::error!(id = u.id, error = %e, "custom button update failed");
            DomainError::from(e)
        })
    }

    /// Applies every update in one transaction.
    pub fn update_all(&self, updates: &[CustomButtonUpdate]) -> Result<(), DomainError> {
        self.repo.update_all(updates).map_err(|e| {
            tracing::error!(error = %e, "custom button batch update failed");
            DomainError::from(e)
        })
    }

    /// Deletes a button and renumbers the rest. `Unchanged` for an unknown id.
    pub fn delete(&self, id: i64) -> Result<Change, DomainError> {
        let _ops = self.guard();
        Ok(if self.repo.delete(id)? > 0 { Change::Changed } else { Change::Unchanged })
    }

    /// Moves the button `id` to `new_index` and renumbers every button to its position. `Unchanged` for an unknown id
    /// or a move to the current position. `new_index` must be within `0..=n-1`, otherwise `InvalidIndex`.
    pub fn reorder(&self, id: i64, new_index: usize) -> Result<Change, DomainError> {
        let _ops = self.guard();
        let mut ids: Vec<i64> = self.repo.get_all()?.iter().map(|b| b.id).collect();
        let Some(from) = ids.iter().position(|&x| x == id) else {
            return Ok(Change::Unchanged);
        };
        if new_index >= ids.len() {
            return Err(DomainError::Invalid("custom button index is out of range"));
        }
        if from == new_index {
            return Ok(Change::Unchanged);
        }
        ids.remove(from);
        ids.insert(new_index, id);
        self.repo.set_order(&ids)?;
        Ok(Change::Changed)
    }

    /// Turns the favourite of `id` on or off. Turning one on makes it the only favourite. `Unchanged` for an unknown id.
    pub fn toggle_favorite(&self, id: i64) -> Result<Change, DomainError> {
        let _ops = self.guard();
        let Some(button) = self.repo.get_all()?.into_iter().find(|b| b.id == id) else {
            return Ok(Change::Unchanged);
        };
        if button.is_favorite {
            self.repo.update(&CustomButtonUpdate { is_favorite: Some(false), ..CustomButtonUpdate::new(id) })?;
        } else {
            self.repo.make_only_favorite(id)?;
        }
        Ok(Change::Changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::anime_db;

    fn service() -> (tempfile::TempDir, CustomButtonService) {
        let (dir, db) = anime_db();
        (dir, CustomButtonService::new(CustomButtonRepo::new(db)))
    }

    fn order(svc: &CustomButtonService) -> Vec<(String, i64)> {
        svc.list().unwrap().into_iter().map(|b| (b.name, b.sort_index)).collect()
    }

    fn favorites(svc: &CustomButtonService) -> Vec<i64> {
        svc.list().unwrap().iter().filter(|b| b.is_favorite).map(|b| b.id).collect()
    }

    #[test]
    fn create_takes_the_next_index_and_is_not_a_favourite() {
        let (_d, svc) = service();
        let id = svc.create("B", "a", "", "").unwrap();
        assert_eq!(order(&svc), vec![("+85 s".to_owned(), 0), ("B".to_owned(), 1)]);
        assert!(!svc.list().unwrap().iter().any(|b| b.id == id && b.is_favorite));
    }

    #[test]
    fn create_rejects_an_empty_name_and_writes_nothing() {
        let (_d, svc) = service();
        assert!(matches!(svc.create("", "a", "", ""), Err(DomainError::Invalid(_))));
        assert_eq!(svc.list().unwrap().len(), 1);
    }

    #[test]
    fn reorder_moves_the_button_and_renumbers_every_position() {
        let (_d, svc) = service();
        let b = svc.create("B", "", "", "").unwrap();
        let c = svc.create("C", "", "", "").unwrap();
        assert_eq!(svc.reorder(c, 0).unwrap(), Change::Changed);
        assert_eq!(order(&svc), vec![("C".to_owned(), 0), ("+85 s".to_owned(), 1), ("B".to_owned(), 2)]);
        assert_eq!(svc.reorder(c, 2).unwrap(), Change::Changed, "the last position is reachable");
        assert_eq!(svc.list().unwrap().last().unwrap().id, c);
        assert_eq!(svc.reorder(b, 0).unwrap(), Change::Changed);
    }

    #[test]
    fn reorder_rejects_an_index_past_the_last_button() {
        let (_d, svc) = service();
        svc.create("B", "", "", "").unwrap();
        let before = order(&svc);
        assert!(matches!(svc.reorder(1, 2), Err(DomainError::Invalid(_))));
        assert_eq!(order(&svc), before, "nothing was written");
    }

    #[test]
    fn reorder_of_an_unknown_id_or_to_the_same_place_writes_nothing() {
        let (_d, svc) = service();
        svc.create("B", "", "", "").unwrap();
        let before = order(&svc);
        assert_eq!(svc.reorder(999, 0).unwrap(), Change::Unchanged);
        assert_eq!(svc.reorder(1, 0).unwrap(), Change::Unchanged);
        assert_eq!(order(&svc), before);
    }

    #[test]
    fn delete_renumbers_and_an_unknown_id_is_unchanged() {
        let (_d, svc) = service();
        let b = svc.create("B", "", "", "").unwrap();
        assert_eq!(svc.delete(1).unwrap(), Change::Changed);
        assert_eq!(order(&svc), vec![("B".to_owned(), 0)]);
        assert_eq!(svc.delete(999).unwrap(), Change::Unchanged);
        assert_eq!(svc.delete(b).unwrap(), Change::Changed);
        assert!(svc.list().unwrap().is_empty());
    }

    #[test]
    fn starring_one_button_unstars_the_others_and_unstarring_clears_only_it() {
        let (_d, svc) = service();
        let b = svc.create("B", "", "", "").unwrap();
        assert_eq!(svc.toggle_favorite(b).unwrap(), Change::Changed);
        assert_eq!(favorites(&svc), vec![b]);
        assert_eq!(svc.toggle_favorite(b).unwrap(), Change::Changed);
        assert!(favorites(&svc).is_empty(), "turning the only favourite off leaves none");
    }

    #[test]
    fn toggling_an_unknown_id_is_unchanged() {
        let (_d, svc) = service();
        assert_eq!(svc.toggle_favorite(42).unwrap(), Change::Unchanged);
        assert_eq!(favorites(&svc), vec![1], "the seed favourite stays");
    }

    #[test]
    fn update_changes_only_the_named_columns() {
        let (_d, svc) = service();
        svc.update(&CustomButtonUpdate { name: Some("Renamed".into()), ..CustomButtonUpdate::new(1) }).unwrap();
        let seed = svc.list().unwrap().remove(0);
        assert_eq!(seed.name, "Renamed");
        assert!(seed.is_favorite, "isFavorite is not touched by a rename");
    }
}
