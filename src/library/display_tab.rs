//! The display page of the library settings sheet (T-37a-039). Pure: the four mode chips, the columns slider
//! label, and which preference each overlay or tab switch writes.

use kuro_domain::kind::EntryKind;
use kuro_domain::library::columns::PORTRAIT_BREAKPOINT_PX;
use kuro_prefs::library::{LibraryDisplayMode, LibraryPrefs};
use kuro_i18n::MsgKey;
use kuro_prefs::pref::Pref;

/// Chip order of the display page (Kotlin `displayModes`), which is not the enum order.
pub const MODE_CHIPS: [LibraryDisplayMode; 4] = [
    LibraryDisplayMode::CompactGrid,
    LibraryDisplayMode::ComfortableGrid,
    LibraryDisplayMode::CoverOnlyGrid,
    LibraryDisplayMode::List,
];

/// The columns slider runs 0 to 10; 0 means automatic.
pub const SLIDER_RANGE: std::ops::RangeInclusive<i32> = 0..=10;

/// Slider caption key: list mode sets the list size, grid modes the items per row.
pub fn slider_label(mode: LibraryDisplayMode) -> MsgKey {
    match mode {
        LibraryDisplayMode::List => MsgKey::pref_library_rows,
        _ => MsgKey::pref_library_columns,
    }
}

/// Slider value text: the Auto label at 0, the number otherwise.
pub fn slider_value_text(value: i32) -> String {
    if value > 0 { value.to_string() } else { kuro_i18n::tr(MsgKey::label_auto) }
}

/// The portrait and landscape column preferences of a kind (T-08a-043).
pub fn column_prefs(prefs: &LibraryPrefs, kind: EntryKind) -> (&Pref<i32>, &Pref<i32>) {
    match kind {
        EntryKind::Anime => (&prefs.anime_portrait_columns, &prefs.anime_landscape_columns),
        EntryKind::Manga => (&prefs.manga_portrait_columns, &prefs.manga_landscape_columns),
    }
}

/// The columns preference the slider edits at a window `width` pixels wide: the portrait key below 900 px, the
/// landscape key from there on, the same split the grid reads (`columns_for_width`).
pub fn columns_pref(prefs: &LibraryPrefs, kind: EntryKind, width: i32) -> &Pref<i32> {
    let (portrait, landscape) = column_prefs(prefs, kind);
    if width < PORTRAIT_BREAKPOINT_PX { portrait } else { landscape }
}

/// The five overlay switches and the two tab switches. Each writes its preference as soon as it changes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toggle {
    DownloadBadge,
    UnreadBadge,
    LocalBadge,
    LanguageBadge,
    ContinueButton,
    CategoryTabs,
    ItemCount,
}

impl Toggle {
    /// The overlay group: the four badges and the continue button.
    pub const OVERLAY: [Toggle; 5] = [Toggle::DownloadBadge, Toggle::UnreadBadge, Toggle::LocalBadge, Toggle::LanguageBadge, Toggle::ContinueButton];
    /// The tabs group.
    pub const TABS: [Toggle; 2] = [Toggle::CategoryTabs, Toggle::ItemCount];
    #[cfg(test)]
    pub const ALL: [Toggle; 7] = [
        Toggle::DownloadBadge,
        Toggle::UnreadBadge,
        Toggle::LocalBadge,
        Toggle::LanguageBadge,
        Toggle::ContinueButton,
        Toggle::CategoryTabs,
        Toggle::ItemCount,
    ];

    pub fn pref(self, prefs: &LibraryPrefs) -> &Pref<bool> {
        match self {
            Toggle::DownloadBadge => &prefs.download_badge,
            Toggle::UnreadBadge => &prefs.unread_badge,
            Toggle::LocalBadge => &prefs.local_badge,
            Toggle::LanguageBadge => &prefs.language_badge,
            Toggle::ContinueButton => &prefs.show_continue_viewing_button,
            Toggle::CategoryTabs => &prefs.category_tabs,
            Toggle::ItemCount => &prefs.category_number_of_items,
        }
    }
}

/// The chip title from the string table.
pub fn chip_key(mode: LibraryDisplayMode) -> MsgKey {
    match mode {
        LibraryDisplayMode::CompactGrid => MsgKey::action_display_grid,
        LibraryDisplayMode::ComfortableGrid => MsgKey::action_display_comfortable_grid,
        LibraryDisplayMode::CoverOnlyGrid => MsgKey::action_display_cover_only_grid,
        LibraryDisplayMode::List => MsgKey::action_display_list,
    }
}

impl Toggle {
    /// The switch title from the string table; the item words follow the kind.
    pub fn label_key(self, kind: EntryKind) -> MsgKey {
        match (self, kind) {
            (Toggle::DownloadBadge, EntryKind::Manga) => MsgKey::action_display_download_badge,
            (Toggle::DownloadBadge, EntryKind::Anime) => MsgKey::action_display_download_badge_anime,
            (Toggle::UnreadBadge, EntryKind::Manga) => MsgKey::action_display_unread_badge,
            (Toggle::UnreadBadge, EntryKind::Anime) => MsgKey::action_display_unseen_badge,
            (Toggle::LocalBadge, _) => MsgKey::action_display_local_badge,
            (Toggle::LanguageBadge, _) => MsgKey::action_display_language_badge,
            (Toggle::ContinueButton, _) => MsgKey::action_display_show_continue_reading_button,
            (Toggle::CategoryTabs, _) => MsgKey::action_display_show_tabs,
            (Toggle::ItemCount, _) => MsgKey::action_display_show_number_of_items,
        }
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::*;

    #[test]
    fn chips_cover_all_four_modes_in_menu_order() {
        assert_eq!(MODE_CHIPS.len(), 4);
        for mode in LibraryDisplayMode::ALL {
            assert!(MODE_CHIPS.contains(&mode));
        }
        assert_eq!(MODE_CHIPS[0], LibraryDisplayMode::CompactGrid);
        assert_eq!(MODE_CHIPS[3], LibraryDisplayMode::List);
    }

    #[test]
    fn slider_label_follows_the_mode_and_zero_reads_auto() {
        assert_eq!(kuro_i18n::tr(slider_label(LibraryDisplayMode::List)), "List size");
        for mode in [LibraryDisplayMode::CompactGrid, LibraryDisplayMode::ComfortableGrid, LibraryDisplayMode::CoverOnlyGrid] {
            assert_eq!(kuro_i18n::tr(slider_label(mode)), "Items per row");
        }
        assert_eq!(slider_value_text(0), "Auto");
        assert_eq!(slider_value_text(7), "7");
        assert_eq!(*SLIDER_RANGE.end(), 10);
    }

    #[test]
    fn the_overlay_and_tabs_groups_cover_every_switch_once_under_their_headings() {
        let mut both: Vec<Toggle> = Toggle::OVERLAY.iter().chain(Toggle::TABS.iter()).copied().collect();
        both.sort_by_key(|t| *t as u8);
        let mut all = Toggle::ALL.to_vec();
        all.sort_by_key(|t| *t as u8);
        assert_eq!(both, all);
        assert_eq!(kuro_i18n::tr(MsgKey::overlay_header), "Overlay");
        assert_eq!(kuro_i18n::tr(MsgKey::tabs_header), "Tabs");
    }

    #[test]
    fn each_toggle_writes_its_own_key_immediately() {
        let prefs = LibraryPrefs::new(PrefStore::in_memory());
        for toggle in Toggle::ALL {
            let pref = toggle.pref(&prefs);
            let before = pref.get();
            pref.set(!before).unwrap();
            assert_eq!(toggle.pref(&prefs).get(), !before, "{toggle:?}");
        }
    }

    #[test]
    fn columns_preference_is_one_per_kind() {
        let prefs = LibraryPrefs::new(PrefStore::in_memory());
        columns_pref(&prefs, EntryKind::Anime, 800).set(4).unwrap();
        assert_eq!(prefs.anime_portrait_columns.get(), 4);
        assert_eq!(prefs.manga_portrait_columns.get(), 0);
    }

    #[test]
    fn the_slider_edits_the_key_the_grid_reads_at_that_width() {
        let prefs = LibraryPrefs::new(PrefStore::in_memory());
        columns_pref(&prefs, EntryKind::Manga, 899).set(3).unwrap();
        columns_pref(&prefs, EntryKind::Manga, 900).set(7).unwrap();
        assert_eq!((prefs.manga_portrait_columns.get(), prefs.manga_landscape_columns.get()), (3, 7));
    }

    #[test]
    fn chips_and_switches_read_from_the_string_table() {
        assert_eq!(kuro_i18n::tr(chip_key(LibraryDisplayMode::CompactGrid)), "Compact grid");
        assert_eq!(kuro_i18n::tr(chip_key(LibraryDisplayMode::List)), "List");
        for toggle in Toggle::ALL {
            for kind in [EntryKind::Manga, EntryKind::Anime] {
                assert!(!kuro_i18n::tr(toggle.label_key(kind)).is_empty(), "{toggle:?} {kind:?}");
            }
        }
        assert_eq!(kuro_i18n::tr(Toggle::UnreadBadge.label_key(EntryKind::Anime)), "Unseen episodes");
    }
}
