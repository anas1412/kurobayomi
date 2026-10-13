//! The download side of the anime selection bar (T-39b-063): which selected episodes the bar's Download row queues and
//! its Delete row removes. Pure. The Download menu itself is `download_menu`.

use std::collections::HashSet;

use kuro_data::item::Item;

/// The ids of the selected episodes the bar's Download row queues: those not on disk.
pub fn selection_to_download(selected: &[Item], downloaded: &HashSet<i64>) -> Vec<i64> {
    selected.iter().filter(|e| !downloaded.contains(&e.id)).map(|e| e.id).collect()
}

/// The ids of the selected episodes the bar's Delete row removes: those on disk.
pub fn selection_to_delete(selected: &[Item], downloaded: &HashSet<i64>) -> Vec<i64> {
    selected.iter().filter(|e| downloaded.contains(&e.id)).map(|e| e.id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(id: i64, seen: bool) -> Item {
        Item { id, done: seen, ..Item::create(kuro_platform::kind::Kind::Anime) }
    }

    fn set(ids: &[i64]) -> HashSet<i64> {
        ids.iter().copied().collect()
    }

    #[test]
    fn the_bar_downloads_what_is_missing_and_deletes_what_is_on_disk() {
        let selected = [ep(1, false), ep(2, false), ep(3, true)];
        let on_disk = set(&[2]);
        assert_eq!(selection_to_download(&selected, &on_disk), vec![1, 3]);
        assert_eq!(selection_to_delete(&selected, &on_disk), vec![2]);
    }
}
