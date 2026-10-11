//! Library update restrictions and the candidate selection of a library update (behaviour 1.11 and the skip rules
//! of `MangaLibraryUpdateJob`). Pure: the job supplies the rows, the preference sets and the fetch-window bound.

use std::collections::BTreeSet;

use super::filter::COMPLETED_STATUS;
use super::model::LibraryRow;
use kuro_platform::device::DeviceState;
use kuro_prefs::library::{
    DEVICE_CHARGING, DEVICE_NETWORK_NOT_METERED, DEVICE_ONLY_ON_WIFI, ENTRY_HAS_UNVIEWED, ENTRY_NON_COMPLETED,
    ENTRY_NON_VIEWED, ENTRY_OUTSIDE_RELEASE_PERIOD,
};

/// Why a library update skips an entry. The order of [`skip_reason`] is the order of these checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkipReason {
    NotAlwaysUpdate,
    Completed,
    HasUnread,
    NotStarted,
    OutsideReleasePeriod,
}

impl SkipReason {
    /// The English text shown in the update log.
    pub fn text(self) -> &'static str {
        match self {
            Self::NotAlwaysUpdate => "Skipped because series does not require updates",
            Self::Completed => "Skipped because series is complete",
            Self::HasUnread => "Skipped because there are unread chapters",
            Self::NotStarted => "Skipped because no chapters are read",
            Self::OutsideReleasePeriod => "Skipped because no release was expected today",
        }
    }
}

/// The first reason an entry is skipped, or `None` when the update refreshes it. `fetch_window_upper` is the upper
/// bound of today's release window in epoch ms (`getWindow(now).second`).
pub fn skip_reason(row: &LibraryRow, restrictions: &BTreeSet<String>, fetch_window_upper: i64) -> Option<SkipReason> {
    let has = |key: &str| restrictions.contains(key);
    if !row.always_update {
        return Some(SkipReason::NotAlwaysUpdate);
    }
    if has(ENTRY_NON_COMPLETED) && row.status == COMPLETED_STATUS {
        return Some(SkipReason::Completed);
    }
    if has(ENTRY_HAS_UNVIEWED) && row.pending() != 0 {
        return Some(SkipReason::HasUnread);
    }
    if has(ENTRY_NON_VIEWED) && row.total > 0 && !row.has_started() {
        return Some(SkipReason::NotStarted);
    }
    if has(ENTRY_OUTSIDE_RELEASE_PERIOD) && row.next_update > fetch_window_upper {
        return Some(SkipReason::OutsideReleasePeriod);
    }
    None
}

/// The entries a library update may refresh, distinct by entry id, first row wins.
///
/// A single-category refresh (`only_category = Some(id)`) takes that category and ignores both lists. Otherwise the
/// include set selects categories (empty means all), and an entry in any excluded category is out entirely.
pub fn update_candidates<'a>(
    rows: &'a [LibraryRow],
    include: &BTreeSet<i64>,
    exclude: &BTreeSet<i64>,
    only_category: Option<i64>,
) -> Vec<&'a LibraryRow> {
    let mut seen = BTreeSet::new();
    if let Some(category) = only_category {
        return rows.iter().filter(|r| r.category == category && seen.insert(r.id)).collect();
    }
    let excluded_entries: BTreeSet<i64> = rows.iter().filter(|r| exclude.contains(&r.category)).map(|r| r.id).collect();
    rows.iter()
        .filter(|r| (include.is_empty() || include.contains(&r.category)) && !excluded_entries.contains(&r.id))
        .filter(|r| seen.insert(r.id))
        .collect()
}

/// The entries that a global update would refresh: candidates that no restriction skips. The statistics screen
/// shows this count.
pub fn global_update_items<'a>(
    rows: &'a [LibraryRow],
    include: &BTreeSet<i64>,
    exclude: &BTreeSet<i64>,
    restrictions: &BTreeSet<String>,
    fetch_window_upper: i64,
) -> Vec<&'a LibraryRow> {
    update_candidates(rows, include, exclude, None)
        .into_iter()
        .filter(|r| skip_reason(r, restrictions, fetch_window_upper).is_none())
        .collect()
}

/// Interval choices of the library update setting, in hours: `(hours, label)`.
pub const INTERVAL_CHOICES: [(i32, &str); 6] =
    [(0, "Off"), (12, "Every 12 hours"), (24, "Daily"), (48, "Every 2 days"), (72, "Every 3 days"), (168, "Weekly")];

/// Device restrictions of the library update setting: `(stored value, label)`. The default is no restriction, so a wired desktop updates.
pub const DEVICE_RESTRICTIONS: [(&str, &str); 3] = [
    (kuro_prefs::library::DEVICE_ONLY_ON_WIFI, "Only on Wi-Fi"),
    (kuro_prefs::library::DEVICE_NETWORK_NOT_METERED, "Only on unmetered network"),
    (kuro_prefs::library::DEVICE_CHARGING, "When charging"),
];

/// Whether every device condition the setting selects holds now. A library update needs all of the selected
/// conditions, as the Android job's network and charging constraints do. An empty selection always holds.
pub fn device_conditions_met(selected: &BTreeSet<String>, state: DeviceState) -> bool {
    (!selected.contains(DEVICE_ONLY_ON_WIFI) || state.on_wifi)
        && (!selected.contains(DEVICE_NETWORK_NOT_METERED) || state.network_not_metered)
        && (!selected.contains(DEVICE_CHARGING) || state.charging)
}

/// The manga include/exclude dialog note (`pref_manga_library_update_categories_details`).
pub const MANGA_UPDATE_CATEGORIES_DETAILS: &str =
    "Manga in excluded categories will not be updated even if they are also in included categories.";

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, category: i64) -> LibraryRow {
        LibraryRow {
            id,
            source: 7,
            title: format!("t{id}"),
            category,
            total: 4,
            done: 0,
            bookmarks: 0,
            fillermarks: 0,
            latest_upload: 0,
            item_fetched_at: 0,
            last_activity: 0,
            last_update: 0,
            date_added: 0,
            status: 1,
            fetch_interval: 0,
            viewer_flags: 0,
            always_update: true,
            next_update: 0,
        }
    }

    fn set(values: &[i64]) -> BTreeSet<i64> {
        values.iter().copied().collect()
    }

    fn restrictions(keys: &[&str]) -> BTreeSet<String> {
        keys.iter().map(|k| (*k).to_owned()).collect()
    }

    #[test]
    fn skip_reasons_follow_the_upstream_order_and_texts() {
        let all =
            restrictions(&[ENTRY_NON_COMPLETED, ENTRY_HAS_UNVIEWED, ENTRY_NON_VIEWED, ENTRY_OUTSIDE_RELEASE_PERIOD]);
        let mut r = row(1, 0);
        r.always_update = false;
        r.status = 2;
        assert_eq!(skip_reason(&r, &all, 0), Some(SkipReason::NotAlwaysUpdate), "strategy comes first");
        r.always_update = true;
        assert_eq!(skip_reason(&r, &all, 0), Some(SkipReason::Completed));
        r.status = 1;
        r.done = 1;
        assert_eq!(skip_reason(&r, &all, 0), Some(SkipReason::HasUnread));
        r.done = 4;
        r.total = 4;
        assert_eq!(skip_reason(&r, &all, 0), None, "fully read, ongoing, not outside the window");
        r.done = 0;
        r.total = 4;
        assert_eq!(skip_reason(&r, &restrictions(&[ENTRY_NON_VIEWED]), 0), Some(SkipReason::NotStarted));
        r.next_update = 100;
        assert_eq!(
            skip_reason(&r, &restrictions(&[ENTRY_OUTSIDE_RELEASE_PERIOD]), 50),
            Some(SkipReason::OutsideReleasePeriod)
        );
        assert_eq!(SkipReason::Completed.text(), "Skipped because series is complete");
        assert_eq!(SkipReason::OutsideReleasePeriod.text(), "Skipped because no release was expected today");
    }

    #[test]
    fn excluded_category_removes_the_entry_entirely_and_include_empty_means_all() {
        let rows = vec![row(1, 5), row(1, 6), row(2, 5), row(3, 7)];
        let none = BTreeSet::new();
        let ids: Vec<i64> = update_candidates(&rows, &none, &none, None).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 2, 3], "distinct by id");
        let ids: Vec<i64> = update_candidates(&rows, &none, &set(&[6]), None).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2, 3], "entry 1 is in excluded category 6");
        let ids: Vec<i64> = update_candidates(&rows, &set(&[5]), &none, None).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn a_single_category_refresh_ignores_both_lists() {
        let rows = vec![row(1, 5), row(2, 6)];
        let ids: Vec<i64> = update_candidates(&rows, &set(&[9]), &set(&[6]), Some(6)).iter().map(|r| r.id).collect();
        assert_eq!(ids, vec![2]);
    }

    #[test]
    fn global_count_skips_restricted_entries() {
        let mut read_all = row(1, 0);
        read_all.done = 4;
        let mut partly = row(2, 0);
        partly.done = 1;
        let rows = vec![read_all, partly];
        let count =
            global_update_items(&rows, &BTreeSet::new(), &BTreeSet::new(), &restrictions(&[ENTRY_HAS_UNVIEWED]), 0)
                .len();
        assert_eq!(count, 1);
    }

    #[test]
    fn interval_and_device_choices_match_the_settings_screen() {
        assert_eq!(INTERVAL_CHOICES.map(|c| c.0), [0, 12, 24, 48, 72, 168]);
        assert_eq!(INTERVAL_CHOICES[2].1, "Daily");
        assert_eq!(DEVICE_RESTRICTIONS[0], ("wifi", "Only on Wi-Fi"));
    }

    #[test]
    fn every_selected_device_condition_must_hold() {
        let wifi = BTreeSet::from(["wifi".to_owned()]);
        let all: BTreeSet<String> = ["wifi", "network_not_metered", "ac"].into_iter().map(str::to_owned).collect();
        let on_wifi = DeviceState { on_wifi: true, network_not_metered: false, charging: false };
        assert!(device_conditions_met(&wifi, on_wifi));
        assert!(!device_conditions_met(&wifi, DeviceState { on_wifi: false, network_not_metered: true, charging: true }));
        assert!(device_conditions_met(&BTreeSet::new(), DeviceState::default()), "no condition selected");
        let everything = DeviceState { on_wifi: true, network_not_metered: true, charging: true };
        assert!(device_conditions_met(&all, everything));
        assert!(!device_conditions_met(&all, DeviceState { charging: false, ..everything }));
    }
}
