//! Pure queue model for the Download queue page (T-25a-020, T-25a-031; 25-downloads-shared p1-rust-design section 2.2).
//!
//! The engine only knows the flat queue. The page groups it by source, sorts inside each group, moves rows and removes
//! them, then sends the flat order back. No GTK and no engine here.

use std::cmp::Ordering;

use crate::model::DownloadState;

/// Which queue page the item belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EntryKind {
    Anime,
    Manga,
}

/// One row of the queue. `parent_id` is the series key (anime id or manga id).
#[derive(Clone, Debug, PartialEq)]
pub struct QueueItem {
    pub kind: EntryKind,
    /// Episode id or chapter id.
    pub item_id: i64,
    pub parent_id: i64,
    pub source_id: i64,
    pub source_name: String,
    /// The entry title (anime or manga), shown as the row subtitle.
    pub title: String,
    /// The episode or chapter name, shown as the row title.
    pub item_name: String,
    /// Sort key "By upload date".
    pub date_upload: i64,
    /// Sort key "By episode number" / "By chapter number".
    pub number: f64,
    pub state: DownloadState,
    /// Pages finished (manga). Anime rows leave it at 0.
    pub pages_done: u32,
    /// Pages in the list (manga). `None` before the list is known, and always `None` for anime.
    pub pages_total: Option<u32>,
    /// Anime: percent 0..=100. Manga: the sum of page progress, each page at most 100.
    pub progress: i64,
}

/// The two sort keys of the sort menu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SortKey {
    UploadDate,
    Number,
}

/// One section: a source and its rows in queue order.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    pub source_id: i64,
    pub source_name: String,
    pub items: Vec<QueueItem>,
}

/// The queue as the page sees it. `flat` is the order the engine holds.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct QueueModel {
    flat: Vec<QueueItem>,
}

impl QueueModel {
    pub fn from_flat(flat: Vec<QueueItem>) -> Self {
        Self { flat }
    }

    pub fn flat(&self) -> &[QueueItem] {
        &self.flat
    }

    pub fn is_empty(&self) -> bool {
        self.flat.is_empty()
    }

    pub fn len(&self) -> usize {
        self.flat.len()
    }

    pub fn position(&self, item_id: i64) -> Option<usize> {
        self.flat.iter().position(|i| i.item_id == item_id)
    }

    /// Groups by source id in first-appearance order. Two sources with the same name stay two sections (E-37).
    pub fn sections(&self) -> Vec<Section> {
        let mut out: Vec<Section> = Vec::new();
        for item in &self.flat {
            match out.iter_mut().find(|s| s.source_id == item.source_id) {
                Some(section) => section.items.push(item.clone()),
                None => out.push(Section {
                    source_id: item.source_id,
                    source_name: item.source_name.clone(),
                    items: vec![item.clone()],
                }),
            }
        }
        out
    }

    fn set_sections(&mut self, sections: Vec<Section>) {
        self.flat = sections.into_iter().flat_map(|s| s.items).collect();
    }

    /// Stable sort inside each section, then reverse when asked. Reversing also flips equal keys, as Kotlin's
    /// `sortedBy { }.reverse()` does (E-07). Items never move to another section (E-08).
    pub fn sort_within_sections(&mut self, key: SortKey, reverse: bool) {
        let mut sections = self.sections();
        for section in &mut sections {
            section.items.sort_by(|a, b| compare(a, b, key));
            if reverse {
                section.items.reverse();
            }
        }
        self.set_sections(sections);
    }

    /// Moves the item to the top of its own section. Returns `false` when the id is not queued.
    pub fn move_to_top(&mut self, item_id: i64) -> bool {
        self.move_in_section(item_id, Place::Top)
    }

    /// Moves the item to the bottom of its own section. Returns `false` when the id is not queued.
    pub fn move_to_bottom(&mut self, item_id: i64) -> bool {
        self.move_in_section(item_id, Place::Bottom)
    }

    /// Moves the item one row up or down inside its section (keyboard Alt+Up / Alt+Down). `false` at the edge.
    pub fn move_one(&mut self, item_id: i64, up: bool) -> bool {
        let mut sections = self.sections();
        let Some(section) = sections.iter_mut().find(|s| s.items.iter().any(|i| i.item_id == item_id)) else {
            return false;
        };
        let Some(index) = section.items.iter().position(|i| i.item_id == item_id) else { return false };
        let target = if up { index.checked_sub(1) } else { Some(index + 1).filter(|t| *t < section.items.len()) };
        let Some(target) = target else { return false };
        section.items.swap(index, target);
        self.set_sections(sections);
        true
    }

    fn move_in_section(&mut self, item_id: i64, place: Place) -> bool {
        let mut sections = self.sections();
        let Some(section) = sections.iter_mut().find(|s| s.items.iter().any(|i| i.item_id == item_id)) else {
            return false;
        };
        let Some(index) = section.items.iter().position(|i| i.item_id == item_id) else { return false };
        let item = section.items.remove(index);
        match place {
            Place::Top => section.items.insert(0, item),
            Place::Bottom => section.items.push(item),
        }
        self.set_sections(sections);
        true
    }

    /// "Move series to top": the selected series first, then the rest, as a flat partition (E-09, T-E09). `false` when
    /// the id is not queued.
    pub fn move_series(&mut self, item_id: i64, to_top: bool) -> bool {
        let Some(parent) = self.flat.iter().find(|i| i.item_id == item_id).map(|i| i.parent_id) else {
            return false;
        };
        let (selected, others): (Vec<QueueItem>, Vec<QueueItem>) =
            std::mem::take(&mut self.flat).into_iter().partition(|i| i.parent_id == parent);
        self.flat = if to_top {
            selected.into_iter().chain(others).collect()
        } else {
            others.into_iter().chain(selected).collect()
        };
        true
    }

    /// Removes the listed ids. Returns the ids that were queued, in queue order. An empty result means no call is needed
    /// (E-11).
    pub fn remove(&mut self, item_ids: &[i64]) -> Vec<i64> {
        self.retain_removing(|i| item_ids.contains(&i.item_id))
    }

    /// Removes every queued item of one series (`Cancel all for this series`). Returns the removed ids.
    pub fn remove_series(&mut self, parent_id: i64) -> Vec<i64> {
        self.retain_removing(|i| i.parent_id == parent_id)
    }

    fn retain_removing(&mut self, matches: impl Fn(&QueueItem) -> bool) -> Vec<i64> {
        let mut removed = Vec::new();
        self.flat.retain(|item| {
            if matches(item) {
                removed.push(item.item_id);
                false
            } else {
                true
            }
        });
        removed
    }

    pub fn item_ids(&self) -> Vec<i64> {
        self.flat.iter().map(|i| i.item_id).collect()
    }
}

#[derive(Clone, Copy)]
enum Place {
    Top,
    Bottom,
}

fn compare(a: &QueueItem, b: &QueueItem, key: SortKey) -> Ordering {
    match key {
        SortKey::UploadDate => a.date_upload.cmp(&b.date_upload),
        SortKey::Number => a.number.partial_cmp(&b.number).unwrap_or(Ordering::Equal),
    }
}

/// The order the engine receives for a `reorder` call (E-15). Unknown ids are dropped, repeats keep their first place,
/// and live ids that were not listed follow in their current order. An empty request is `None`: it is a no-op, and it
/// never clears the queue.
pub fn normalize_order(current: &[i64], requested: &[i64]) -> Option<Vec<i64>> {
    if requested.is_empty() {
        return None;
    }
    let mut out: Vec<i64> = Vec::with_capacity(current.len());
    for id in requested {
        if current.contains(id) && !out.contains(id) {
            out.push(*id);
        }
    }
    for id in current {
        if !out.contains(id) {
            out.push(*id);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(item_id: i64, parent_id: i64, source_id: i64, date: i64, number: f64) -> QueueItem {
        QueueItem {
            kind: EntryKind::Anime,
            item_id,
            parent_id,
            source_id,
            source_name: format!("Source {source_id}"),
            title: format!("Title {parent_id}"),
            item_name: format!("Episode {item_id}"),
            date_upload: date,
            number,
            state: DownloadState::Queue,
            pages_done: 0,
            pages_total: None,
            progress: 0,
        }
    }

    fn ids(model: &QueueModel) -> Vec<i64> {
        model.item_ids()
    }

    #[test]
    fn sections_follow_first_appearance_not_source_id() {
        // Source 9 appears first, so it is the first section even though its id is larger.
        let model = QueueModel::from_flat(vec![
            item(1, 10, 9, 0, 1.0),
            item(2, 20, 3, 0, 1.0),
            item(3, 10, 9, 0, 2.0),
        ]);
        let sections = model.sections();
        assert_eq!(sections.iter().map(|s| s.source_id).collect::<Vec<_>>(), vec![9, 3]);
        assert_eq!(sections[0].items.iter().map(|i| i.item_id).collect::<Vec<_>>(), vec![1, 3]);
    }

    #[test]
    fn two_sources_with_the_same_name_are_two_sections() {
        let mut second = item(2, 20, 4, 0, 1.0);
        second.source_name = "Source 3".to_owned();
        let mut first = item(1, 10, 3, 0, 1.0);
        first.source_name = "Source 3".to_owned();
        let model = QueueModel::from_flat(vec![first, second]);
        assert_eq!(model.sections().len(), 2, "E-37: the id decides the section, not the name");
    }

    #[test]
    fn flat_order_across_sections_is_kept_on_a_drop() {
        let flat = vec![item(1, 10, 1, 0, 1.0), item(2, 20, 2, 0, 1.0), item(3, 10, 1, 0, 2.0)];
        let model = QueueModel::from_flat(flat.clone());
        assert_eq!(model.flat(), flat.as_slice(), "T-E34: the model keeps the order it was given");
    }

    #[test]
    fn sort_is_stable_then_reversed_so_equal_keys_flip() {
        let mut model = QueueModel::from_flat(vec![
            item(1, 10, 1, 5, 1.0),
            item(2, 10, 1, 5, 1.0),
            item(3, 10, 1, 9, 1.0),
        ]);
        model.sort_within_sections(SortKey::UploadDate, false);
        assert_eq!(ids(&model), vec![1, 2, 3]);
        model.sort_within_sections(SortKey::UploadDate, true);
        assert_eq!(ids(&model), vec![3, 2, 1], "E-07: equal keys are reversed too");
    }

    #[test]
    fn sort_never_moves_items_across_sections() {
        let mut model = QueueModel::from_flat(vec![
            item(1, 10, 1, 1, 3.0),
            item(2, 20, 2, 9, 1.0),
            item(3, 10, 1, 2, 1.0),
        ]);
        model.sort_within_sections(SortKey::Number, false);
        let sections = model.sections();
        assert_eq!(sections[0].items.iter().map(|i| i.item_id).collect::<Vec<_>>(), vec![3, 1]);
        assert_eq!(sections[1].items.iter().map(|i| i.item_id).collect::<Vec<_>>(), vec![2]);
        assert_eq!(sections.len(), 2, "E-08: no section is created or merged by a sort");
    }

    #[test]
    fn number_sort_orders_by_episode_number() {
        let mut model = QueueModel::from_flat(vec![item(1, 10, 1, 0, 10.0), item(2, 10, 1, 0, 2.5)]);
        model.sort_within_sections(SortKey::Number, false);
        assert_eq!(ids(&model), vec![2, 1]);
        model.sort_within_sections(SortKey::Number, true);
        assert_eq!(ids(&model), vec![1, 2]);
    }

    #[test]
    fn move_to_top_and_bottom_act_inside_the_items_own_section() {
        let mut model = QueueModel::from_flat(vec![
            item(1, 10, 1, 0, 1.0),
            item(2, 20, 2, 0, 1.0),
            item(3, 10, 1, 0, 1.0),
            item(4, 10, 1, 0, 1.0),
        ]);
        // The flat list is rebuilt section by section, as Kotlin rebuilds it from the headers.
        assert!(model.move_to_top(3));
        assert_eq!(ids(&model), vec![3, 1, 4, 2]);
        assert!(model.move_to_bottom(3));
        assert_eq!(ids(&model), vec![1, 4, 3, 2]);
        assert!(!model.move_to_top(99), "unknown id is a no-op");
    }

    #[test]
    fn move_one_swaps_with_the_neighbour_in_its_section_only() {
        let mut model = QueueModel::from_flat(vec![
            item(1, 10, 1, 0, 1.0),
            item(2, 20, 2, 0, 1.0),
            item(3, 10, 1, 0, 1.0),
        ]);
        assert!(!model.move_one(1, true), "first row cannot move up");
        assert!(model.move_one(3, true));
        assert_eq!(ids(&model), vec![3, 1, 2]);
        assert!(!model.move_one(1, false), "the last row of its section cannot move down");
    }

    #[test]
    fn move_series_is_a_flat_partition() {
        let mut model = QueueModel::from_flat(vec![
            item(1, 10, 1, 0, 1.0),
            item(2, 20, 2, 0, 1.0),
            item(3, 10, 1, 0, 1.0),
        ]);
        assert!(model.move_series(2, true));
        assert_eq!(ids(&model), vec![2, 1, 3]);
        assert!(model.move_series(2, false));
        assert_eq!(ids(&model), vec![1, 3, 2]);
        assert!(model.move_series(1, true));
        assert_eq!(ids(&model), vec![1, 3, 2], "E-09: the selected series already leads");
        assert!(!model.move_series(99, true));
    }

    #[test]
    fn remove_returns_the_removed_ids_and_no_match_is_empty() {
        let mut model = QueueModel::from_flat(vec![item(1, 10, 1, 0, 1.0), item(2, 20, 1, 0, 1.0)]);
        assert_eq!(model.remove(&[99]), Vec::<i64>::new(), "E-11: no match, nothing removed");
        assert_eq!(model.remove(&[2, 1]), vec![1, 2], "removed ids come back in queue order");
        assert!(model.is_empty());
    }

    #[test]
    fn remove_series_takes_every_item_of_the_parent() {
        let mut model = QueueModel::from_flat(vec![
            item(1, 10, 1, 0, 1.0),
            item(2, 20, 1, 0, 1.0),
            item(3, 10, 1, 0, 1.0),
        ]);
        assert_eq!(model.remove_series(10), vec![1, 3]);
        assert_eq!(ids(&model), vec![2]);
        assert_eq!(model.remove_series(10), Vec::<i64>::new());
    }

    #[test]
    fn reorder_ids_drops_unknown_keeps_unlisted_and_ignores_empty() {
        assert_eq!(normalize_order(&[1, 2, 3], &[]), None, "an empty list never clears the queue");
        assert_eq!(normalize_order(&[1, 2, 3], &[3, 99, 1]), Some(vec![3, 1, 2]), "E-15");
        assert_eq!(normalize_order(&[1, 2, 3], &[2, 2]), Some(vec![2, 1, 3]), "repeats keep their first place");
    }
}
