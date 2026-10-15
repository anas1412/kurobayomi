//! The Downloads and storage rows this module binds (tasks T-47a-044 to T-47a-050): the concurrent download count, the
//! delete-after-read rule and the two new-item switches. Wi-Fi only, the chapter speed limit, CBZ and tall images, the
//! category rules, download-ahead and the external downloader are cut (`docs/design/settings-diet.md`). The keys and
//! defaults come from `kuro_domain::download::prefs::DownloadPrefs`.
use kuro_i18n::{MsgKey, tr};
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::Preference;
use crate::settings_ui::rows::{group, item, ranged_choice, switch};

/// The label of one delete-after-read choice (`remove_after_read_slots`): -1 is Disabled, then the n-th last read chapter.
pub fn remove_after_read_label(slots: i32) -> String {
    match slots {
        -1 => "Disabled".to_owned(),
        0 => tr(MsgKey::last_read_chapter).to_owned(),
        n => format!("{} to last read chapter", ["Second", "Third", "Fourth", "Fifth"].get(n as usize - 1).copied().unwrap_or("Later")),
    }
}

/// The typed rows of the Downloads page, in their groups.
pub fn page_rows(prefs: &Prefs) -> Vec<Preference> {
    let d = DownloadPrefs::new(prefs.store());
    vec![
        item(ranged_choice(&tr(MsgKey::pref_download_slots), &d.number_of_downloads_pref, |n| n.to_string(), true)),
        group(
            &tr(MsgKey::pref_category_delete_chapters),
            vec![ranged_choice(&tr(MsgKey::pref_remove_after_read), &d.remove_after_read_slots, remove_after_read_label, true)],
        ),
        group(
            &tr(MsgKey::pref_category_auto_download),
            vec![switch(&tr(MsgKey::pref_download_new_episodes), &d.download_new_episodes, true), switch(&tr(MsgKey::pref_download_new), &d.download_new_chapters, true)],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use crate::pref_model::model::PreferenceItem;
    use super::*;

    fn find<'a>(rows: &'a [Preference], group_title: &str) -> &'a [PreferenceItem] {
        rows.iter()
            .find_map(|entry| match entry {
                Preference::Group(g) if g.title == group_title => Some(g.items.as_slice()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("group {group_title}"))
    }

    #[test]
    fn the_page_holds_the_slots_the_delete_rule_and_the_two_new_item_switches() {
        let rows = page_rows(&Prefs::in_memory());
        let top: Vec<&str> = rows.iter().filter_map(|r| match r { Preference::Item(i) => Some(i.title()), _ => None }).collect();
        assert_eq!(top, ["Maximum downloads"]);
        let auto: Vec<&str> = find(&rows, "Auto-download").iter().map(PreferenceItem::title).collect();
        assert_eq!(auto, ["Download new episodes", "Download new chapters"]);
        let delete: Vec<&str> = find(&rows, "Delete chapters/episodes").iter().map(PreferenceItem::title).collect();
        assert_eq!(delete, ["After reading/watching automatically delete"]);
    }

    #[test]
    fn the_slot_and_delete_lists_hold_the_spec_entries() {
        let rows = page_rows(&Prefs::in_memory());
        let entries_of = |item: &PreferenceItem| match item {
            PreferenceItem::BasicList { entries, .. } => entries.clone(),
            _ => panic!("{} is not a list", item.title()),
        };
        let Preference::Item(slots) = &rows[0] else { panic!("the slots row") };
        let codes: Vec<String> = entries_of(slots).into_iter().map(|(code, _)| code).collect();
        assert_eq!(codes, ["1", "2", "3", "4", "5"]);
        let delete = entries_of(&find(&rows, "Delete chapters/episodes")[0]);
        assert_eq!(delete.len(), 6);
        assert_eq!(delete[0], ("-1".to_owned(), "Disabled".to_owned()));
        assert_eq!(delete[1].1, "Last read chapter");
        assert_eq!(delete[2].1, "Second to last read chapter");
        assert_eq!(delete[5], ("4".to_owned(), "Fifth to last read chapter".to_owned()));
    }
}
