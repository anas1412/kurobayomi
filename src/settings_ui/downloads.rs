//! The Downloads screen rows this module binds (tasks T-47a-044 to T-47a-050 and T-47a-052): the Wi-Fi switch, the chapter
//! speed limit with its dialog, CBZ and tall-image switches, the concurrent download count, the delete-after-read rules,
//! the auto-download rules per category and the download-ahead choices. The keys and defaults come from
//! `kuro_domain::download::prefs::DownloadPrefs`; the category sets are the Android keys read as id strings.
//! The enable rules are in [`rules`]; the page is rebuilt when [`rule_state`] changes.
use std::cell::Cell;
use std::collections::BTreeSet;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_domain::download::prefs::DownloadPrefs;
use kuro_prefs::pref::Pref;
use kuro_prefs::prefs::Prefs;

use crate::pref_model::model::{Preference, PreferenceItem};
use crate::pref_model::widgets::dialog;
use crate::settings_ui::category::Category;
use crate::settings_ui::category_sets::{CategorySets, categories_row, category_multi_row};
use crate::settings_ui::rows::{edit_text, group, info, int_choice, item, ranged_choice, switch, switch_sub, text};

pub const WIFI_TITLE: &str = "Only on Wi-Fi";
pub const WIFI_KEY: &str = "pref_download_only_over_wifi_key";
pub const SPEED_TITLE: &str = "Chapter download speed limit";
pub const SPEED_HINT: &str = "Set to 0 to disable the speed limit.";
/// The label of one delete-after-read choice (`remove_after_read_slots`): -1 is Disabled, then the n-th last read chapter.
pub fn remove_after_read_label(slots: i32) -> String {
    match slots {
        -1 => "Disabled".to_owned(),
        0 => "Last read chapter".to_owned(),
        n => format!("{} to last read chapter", ["Second", "Third", "Fourth", "Fifth"].get(n as usize - 1).copied().unwrap_or("Later")),
    }
}
/// The Android keys of the two category sets of each auto-download rule, and of the excluded lists of the delete rules.
pub const ANIME_NEW_INCLUDE: &str = "download_new_anime_categories";
pub const ANIME_NEW_EXCLUDE: &str = "download_new_anime_categories_exclude";
pub const MANGA_NEW_INCLUDE: &str = "download_new_categories";
pub const MANGA_NEW_EXCLUDE: &str = "download_new_categories_exclude";
pub const ANIME_REMOVE_EXCLUDE: &str = "remove_exclude_anime_categories";
pub const MANGA_REMOVE_EXCLUDE: &str = "remove_exclude_categories";
/// Shown under both auto-download category dialogs.
pub const AUTO_DOWNLOAD_MESSAGE: &str = "Entries in excluded categories will not be downloaded even if they are also in included categories.";

/// Which auto-download rows are enabled. A row whose rule is off is hidden, as the Android rows are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rules {
    /// Skip duplicate seen episodes: needs new episodes on.
    pub unseen_only: bool,
    /// Anime categories: needs new episodes on.
    pub anime_categories: bool,
    /// Skip duplicate read chapters: needs new chapters on.
    pub unread_only: bool,
    /// Manga categories: needs new chapters on.
    pub manga_categories: bool,
}

/// The enable rules of the auto-download group (task T-47a-052, table in `p1-behavior.md` section 6.1).
pub fn rules(new_episodes: bool, new_chapters: bool) -> Rules {
    Rules { unseen_only: new_episodes, anime_categories: new_episodes, unread_only: new_chapters, manga_categories: new_chapters }
}

/// The chapter speed limit as the row shows it: `Off` for 0, else `<n> KiB/s`.
pub fn speed_subtitle(kib_per_second: i32) -> String {
    if kib_per_second <= 0 { "Off".to_owned() } else { format!("{kib_per_second} KiB/s") }
}

/// The download-ahead choices: Disabled, then `Next N <noun>s` for each step.
pub fn ahead_options(noun: &str) -> Vec<(i32, String)> {
    kuro_prefs::ranges::AUTO_DOWNLOAD_AHEAD
        .values()
        .into_iter()
        .map(|n| match n {
            0 => (0, "Disabled".to_owned()),
            n => (n as i32, format!("Next {n} {noun}s")),
        })
        .collect()
}

/// The speed limit row: the subtitle is the stored limit, a tap opens the dialog.
pub fn speed_limit_row(pref: &Pref<i32>) -> PreferenceItem {
    let mut row = text(SPEED_TITLE, Some(&speed_subtitle(pref.get())), true);
    if let PreferenceItem::Text { on_click, .. } = &mut row {
        let pref = pref.clone();
        *on_click = Some(Rc::new(move || open_speed_dialog(pref.clone())));
    }
    row
}

/// The chooser dialog: step 100, minimum 0, KiB/s suffix. OK stores the chosen value; Cancel discards it.
fn open_speed_dialog(pref: Pref<i32>) {
    let chosen = Rc::new(Cell::new(pref.get()));
    let recorded = Rc::clone(&chosen);
    let chooser = crate::widgets::numeric::numeric_chooser(pref.get(), 100, Some(0), Some("KiB/s"), Some("0"), move |value| recorded.set(value));
    let body = gtk::Box::new(gtk::Orientation::Vertical, 8);
    body.append(&gtk::Label::builder().label(SPEED_TITLE).xalign(0.0).build());
    body.append(&chooser);
    body.append(&gtk::Label::builder().label(SPEED_HINT).wrap(true).xalign(0.0).build());
    dialog::show(
        &crate::ui::window(),
        SPEED_TITLE,
        &body,
        Some("OK"),
        move |dialog| match pref.set(chosen.get()) {
            Ok(()) => dialog.finish(),
            Err(err) => dialog.set_error(Some(&err.to_string())),
        },
        || {},
    );
}

pub const EXTERNAL_SWITCH_TITLE: &str = "Always use external downloader for anime";
pub const EXTERNAL_ENDPOINT_TITLE: &str = "Downloader app preference";
/// What the endpoint row asks for: the downloader on Linux is an aria2 JSON-RPC server (`kuro-downloads`).
pub const EXTERNAL_ENDPOINT_HINT: &str = "Address of the aria2 JSON-RPC server, for example http://127.0.0.1:6800/jsonrpc. Leave empty to download with the app.";

/// The endpoint is empty (use the app) or an http(s) address.
pub fn validate_endpoint(text: &str) -> Result<(), String> {
    let text = text.trim();
    if text.is_empty() || text.starts_with("http://") || text.starts_with("https://") {
        Ok(())
    } else {
        Err("The address must start with http:// or https://".to_owned())
    }
}

/// The external downloader rows: the switch and the aria2 endpoint, over the keys the download engine reads.
pub fn external_rows(d: &DownloadPrefs) -> Vec<PreferenceItem> {
    let mut endpoint = edit_text(EXTERNAL_ENDPOINT_TITLE, &d.external_downloader_selection, true, validate_endpoint, d.use_external_downloader.get());
    if let PreferenceItem::EditText { dialog_subtitle, .. } = &mut endpoint {
        *dialog_subtitle = Some(EXTERNAL_ENDPOINT_HINT.to_owned());
    }
    vec![switch(EXTERNAL_SWITCH_TITLE, &d.use_external_downloader, true), endpoint]
}

/// The category sets the auto-download rules and the delete rules read and write.
fn sets(prefs: &Prefs, include: &'static str, exclude: &'static str) -> CategorySets {
    CategorySets { include: prefs.store().pref(include, BTreeSet::new()), exclude: prefs.store().pref(exclude, BTreeSet::new()) }
}

/// The values the Downloads rows read when built. The page is rebuilt when this changes.
pub fn rule_state(prefs: &Prefs) -> (bool, bool, i32, bool, Vec<BTreeSet<String>>) {
    let d = DownloadPrefs::new(prefs.store());
    let sets = [ANIME_NEW_INCLUDE, ANIME_NEW_EXCLUDE, MANGA_NEW_INCLUDE, MANGA_NEW_EXCLUDE, ANIME_REMOVE_EXCLUDE, MANGA_REMOVE_EXCLUDE]
        .into_iter()
        .map(|key| prefs.store().pref(key, BTreeSet::<String>::new()).get())
        .collect();
    (d.download_new_episodes.get(), d.download_new_chapters.get(), d.download_speed_limit.get(), d.use_external_downloader.get(), sets)
}

/// Borrowed `(id, label)` pairs for [`int_choice`], from owned labels.
fn borrowed(options: &[(i32, String)]) -> Vec<(i32, &str)> {
    options.iter().map(|(code, label)| (*code, label.as_str())).collect()
}

/// The typed rows of the Downloads screen, in their groups. `anime` and `manga` are the user categories of each kind.
pub fn page_rows(prefs: &Prefs, anime: &[Category], manga: &[Category]) -> Vec<Preference> {
    let d = DownloadPrefs::new(prefs.store());
    let rules = rules(d.download_new_episodes.get(), d.download_new_chapters.get());
    let (watch, read) = (ahead_options("unseen episode"), ahead_options("unread chapter"));
    vec![
        item(switch(WIFI_TITLE, &prefs.store().pref(WIFI_KEY, true), true)),
        item(speed_limit_row(&d.download_speed_limit)),
        item(switch("Save as CBZ archive", &d.save_chapters_as_cbz, true)),
        item(switch_sub("Split tall images", "Improves reader performance", &d.split_tall_images, true)),
        item(ranged_choice("Maximum downloads", &d.number_of_downloads_pref, |n| n.to_string(), true)),
        item(info("Will only download concurrently from self-hosted or unmetered sources")),
        group(
            "Delete chapters/episodes",
            vec![
                switch("After manually marked as read/watched", &d.remove_after_marked_as_read, true),
                ranged_choice("After reading/watching automatically delete", &d.remove_after_read_slots, remove_after_read_label, true),
                switch("Allow deleting bookmarked chapters/episodes", &d.remove_bookmarked, true),
                switch("Allow downloading of fillermarked episodes", &d.download_fillermarked, true),
                category_multi_row("Excluded anime categories", &prefs.store().pref(ANIME_REMOVE_EXCLUDE, BTreeSet::new()), anime, true),
                category_multi_row("Excluded manga categories", &prefs.store().pref(MANGA_REMOVE_EXCLUDE, BTreeSet::new()), manga, true),
            ],
        ),
        group(
            "Auto-download",
            vec![
                switch("Download new episodes", &d.download_new_episodes, true),
                switch("Skip downloading duplicate seen episodes", &d.download_new_unseen_episodes_only, rules.unseen_only),
                categories_row(
                    "Anime Categories",
                    &sets(prefs, ANIME_NEW_INCLUDE, ANIME_NEW_EXCLUDE),
                    anime,
                    AUTO_DOWNLOAD_MESSAGE,
                    rules.anime_categories,
                ),
                switch("Download new chapters", &d.download_new_chapters, true),
                switch("Skip downloading duplicate read chapters", &d.download_new_unread_chapters_only, rules.unread_only),
                categories_row(
                    "Manga Categories",
                    &sets(prefs, MANGA_NEW_INCLUDE, MANGA_NEW_EXCLUDE),
                    manga,
                    AUTO_DOWNLOAD_MESSAGE,
                    rules.manga_categories,
                ),
            ],
        ),
        group(
            "Download ahead",
            vec![
                int_choice("Auto download while watching", &d.auto_download_while_watching, &borrowed(&watch), true),
                int_choice("Auto download while reading", &d.auto_download_while_reading, &borrowed(&read), true),
                info("Only works if the current chapter/episode + the next one are already downloaded."),
            ],
        ),
        group("External downloader", external_rows(&d)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_prefs::prefs::Prefs;

    fn find<'a>(rows: &'a [Preference], group_title: &str) -> &'a [PreferenceItem] {
        rows.iter()
            .find_map(|entry| match entry {
                Preference::Group(g) if g.title == group_title => Some(g.items.as_slice()),
                _ => None,
            })
            .unwrap_or_else(|| panic!("group {group_title}"))
    }

    fn titles(items: &[PreferenceItem]) -> Vec<&str> {
        items.iter().map(PreferenceItem::title).collect()
    }

    #[test]
    fn the_switch_is_on_by_default_and_writes_the_downloader_key() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs, &[], &[]);
        let Preference::Item(PreferenceItem::Switch { common, pref, .. }) = &rows[0] else {
            panic!("a switch")
        };
        assert!(pref.get());
        assert_eq!(pref.key(), WIFI_KEY);
        assert_eq!(common.title, WIFI_TITLE);
    }

    #[test]
    fn the_speed_limit_subtitle_is_off_for_zero_and_kib_per_second_otherwise() {
        assert_eq!(speed_subtitle(0), "Off");
        assert_eq!(speed_subtitle(-3), "Off");
        assert_eq!(speed_subtitle(500), "500 KiB/s");
        let prefs = Prefs::in_memory();
        let PreferenceItem::Text { common, on_click, .. } = speed_limit_row(&prefs.store().pref("download_speed_limit", 0)) else {
            panic!("a text row")
        };
        assert_eq!(common.title, SPEED_TITLE);
        assert_eq!(common.subtitle.as_deref(), Some("Off"));
        assert!(on_click.is_some(), "a tap opens the dialog");
    }

    #[test]
    fn the_enable_rules_follow_the_table_of_section_6_1() {
        assert_eq!(rules(false, false), Rules { unseen_only: false, anime_categories: false, unread_only: false, manga_categories: false });
        assert_eq!(rules(true, false), Rules { unseen_only: true, anime_categories: true, unread_only: false, manga_categories: false });
        assert_eq!(rules(false, true), Rules { unseen_only: false, anime_categories: false, unread_only: true, manga_categories: true });
    }

    #[test]
    fn the_auto_download_rows_are_hidden_while_their_switch_is_off() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs, &[], &[]);
        let off = titles(find(&rows, "Auto-download"));
        assert!(off.contains(&"Download new episodes") && off.contains(&"Download new chapters"));
        let enabled = |rows: &[PreferenceItem], title: &str| rows.iter().find(|i| i.title() == title).map(PreferenceItem::enabled);
        let group_rows = find(&rows, "Auto-download");
        assert_eq!(enabled(group_rows, "Skip downloading duplicate seen episodes"), Some(false));
        assert_eq!(enabled(group_rows, "Anime Categories"), Some(false));
        prefs.store().pref("download_new", false).set(true).unwrap();
        prefs.store().pref("download_new_episode", false).set(true).unwrap();
        let rows = page_rows(&prefs, &[], &[]);
        let group_rows = find(&rows, "Auto-download");
        assert_eq!(enabled(group_rows, "Skip downloading duplicate seen episodes"), Some(true));
        assert_eq!(enabled(group_rows, "Anime Categories"), Some(true));
        assert_eq!(enabled(group_rows, "Skip downloading duplicate read chapters"), Some(true));
        assert_eq!(enabled(group_rows, "Manga Categories"), Some(true));
    }

    #[test]
    fn the_download_ahead_choices_read_as_the_spec_lists_them() {
        assert_eq!(
            ahead_options("unseen episode"),
            [
                (0, "Disabled".to_owned()),
                (2, "Next 2 unseen episodes".to_owned()),
                (3, "Next 3 unseen episodes".to_owned()),
                (5, "Next 5 unseen episodes".to_owned()),
                (10, "Next 10 unseen episodes".to_owned()),
            ]
        );
        assert_eq!(ahead_options("unread chapter")[1].1, "Next 2 unread chapters");
    }

    #[test]
    fn the_delete_and_slot_lists_hold_the_spec_entries() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs, &[], &[]);
        let entries_of = |items: &[&PreferenceItem], title: &str| match items.iter().find(|i| i.title() == title) {
            Some(PreferenceItem::BasicList { entries, .. }) => entries.clone(),
            _ => panic!("{title} is not a list"),
        };
        let top: Vec<&PreferenceItem> = rows.iter().filter_map(|r| match r { Preference::Item(i) => Some(i), _ => None }).collect();
        let slots: Vec<String> = entries_of(&top, "Maximum downloads").into_iter().map(|(code, _)| code).collect();
        assert_eq!(slots, ["1", "2", "3", "4", "5"]);
        let delete = entries_of(&find(&rows, "Delete chapters/episodes").iter().collect::<Vec<_>>(), "After reading/watching automatically delete");
        assert_eq!(delete.len(), 6);
        assert_eq!(delete[0], ("-1".to_owned(), "Disabled".to_owned()));
        assert_eq!(delete[1].1, "Last read chapter");
        assert_eq!(delete[2].1, "Second to last read chapter");
        assert_eq!(delete[5], ("4".to_owned(), "Fifth to last read chapter".to_owned()));
    }

    #[test]
    fn the_excluded_lists_offer_the_categories_of_their_kind() {
        let prefs = Prefs::in_memory();
        let anime = vec![Category { id: 2, name: "Series".into(), order: 0 }];
        let rows = page_rows(&prefs, &anime, &[]);
        let delete = find(&rows, "Delete chapters/episodes");
        let PreferenceItem::MultiSelect { entries, .. } = delete.iter().find(|i| i.title() == "Excluded anime categories").unwrap() else {
            panic!("a multi-select")
        };
        assert_eq!(entries, &[("2".to_owned(), "Series".to_owned())]);
    }

    #[test]
    fn the_external_downloader_rows_write_the_keys_the_download_engine_reads() {
        let prefs = Prefs::in_memory();
        let rows = page_rows(&prefs, &[], &[]);
        let group_rows = find(&rows, "External downloader");
        assert_eq!(titles(group_rows), [EXTERNAL_SWITCH_TITLE, EXTERNAL_ENDPOINT_TITLE]);
        let PreferenceItem::Switch { pref, .. } = &group_rows[0] else { panic!("a switch") };
        assert_eq!(pref.key(), "use_external_downloader");
        let PreferenceItem::EditText { pref, .. } = &group_rows[1] else { panic!("an edit row") };
        assert_eq!(pref.key(), "external_downloader_selection");
        assert!(!group_rows[1].enabled(), "the endpoint waits for the switch");
        assert!(validate_endpoint("").is_ok() && validate_endpoint(" http://127.0.0.1:6800/jsonrpc ").is_ok());
        assert!(validate_endpoint("com.example.app").is_err());
    }

    #[test]
    fn the_rule_state_changes_with_a_switch_or_the_speed_limit() {
        let prefs = Prefs::in_memory();
        let before = rule_state(&prefs);
        prefs.store().pref("download_speed_limit", 0).set(400).unwrap();
        assert_ne!(before, rule_state(&prefs));
    }
}
