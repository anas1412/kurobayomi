//! The `[library]` preferences: display, sort, columns, filters, badges, categories and counters.
//!
//! Keys and defaults come from the registry. Per-entry chapter, episode and season templates are
//! not here. Enum-valued keys have typed codecs: an unknown stored value reads as the default and the
//! stored text is kept, as Aniyomi's `getEnum` and `getObject` do.

use std::borrow::Cow;
use std::collections::BTreeSet;

use crate::PrefError;
use crate::pref::Pref;
use crate::registry;
use crate::flags::{
    CHAPTER_BOOKMARKED_MASK, CHAPTER_DISPLAY_MASK, CHAPTER_DOWNLOADED_MASK, CHAPTER_SORTING_MASK,
    CHAPTER_SORT_DIR_MASK, CHAPTER_UNREAD_MASK, EPISODE_BOOKMARKED_MASK, EPISODE_DISPLAY_MASK,
    EPISODE_DOWNLOADED_MASK, EPISODE_FILLERMARKED_MASK, EPISODE_PREVIEWS_MASK, EPISODE_SORTING_MASK,
    EPISODE_SORT_DIR_MASK, EPISODE_SUMMARIES_MASK, EPISODE_UNSEEN_MASK,
};
use crate::flags::{
    SEASON_BOOKMARKED_MASK, SEASON_COMPLETED_MASK, SEASON_DISPLAY_MODE_MASK, SEASON_DOWNLOADED_MASK,
    SEASON_FILLERMARKED_MASK, SEASON_GRID_DISPLAY_MODE_MASK, SEASON_GRID_DISPLAY_MODE_SHIFT,
    SEASON_GRID_DISPLAY_SIZE_MASK, SEASON_GRID_DISPLAY_SIZE_SHIFT, SEASON_OVERLAY_CONT_MASK,
    SEASON_OVERLAY_DOWNLOADED_MASK, SEASON_OVERLAY_LANG_MASK, SEASON_OVERLAY_LOCAL_MASK,
    SEASON_OVERLAY_UNSEEN_MASK, SEASON_SORT_DIR_MASK, SEASON_SORT_MASK, SEASON_STARTED_MASK, SEASON_UNSEEN_MASK,
};
use crate::registry::define_prefs;
use crate::sort::{AnimeLibrarySort, MangaLibrarySort};
use crate::store::PrefStore;
use crate::tristate::TriState;
use crate::values::{Decoded, PrefType, PrefValue};

define_prefs! {
    /// `[library]` section: every non-template, non-season key of Aniyomi `LibraryPreferences`.
    /// The per-tracker filter families are methods on this type.
    pub struct LibraryPrefs in "library" {
        display_mode: LibraryDisplayMode = "pref_display_mode_library" => LibraryDisplayMode::CompactGrid;
        manga_sorting_mode: SortSetting<false> = "library_sorting_mode" => SortSetting("ALPHABETICAL,ASCENDING".to_owned());
        anime_sorting_mode: SortSetting<true> = "animelib_sorting_mode" => SortSetting("ALPHABETICAL,ASCENDING".to_owned());
        last_updated_timestamp: i64 = "__APP_STATE_library_update_last_timestamp" => 0;
        auto_update_interval: i32 = "pref_library_update_interval_key" => 0;
        auto_update_device_restrictions: BTreeSet<String> = "library_update_restriction" => BTreeSet::from(["wifi".to_owned()]);
        auto_update_item_restrictions: BTreeSet<String> = "library_update_manga_restriction" => BTreeSet::from([
            "manga_fully_read".to_owned(),
            "manga_ongoing".to_owned(),
            "manga_started".to_owned(),
            "manga_outside_release_period".to_owned(),
        ]);
        auto_update_metadata: bool = "auto_update_metadata" => false;
        show_continue_viewing_button: bool = "display_continue_reading_button" => false;
        category_tabs: bool = "display_category_tabs" => true;
        category_number_of_items: bool = "display_number_of_items" => false;
        categorized_display_settings: bool = "categorized_display" => false;
        hide_hidden_categories_settings: bool = "hidden_categories" => false;
        filter_interval_custom: TriState = "pref_filter_library_interval_custom" => TriState::DISABLED;
        download_badge: bool = "display_download_badge" => false;
        unread_badge: bool = "display_unread_badge" => true;
        local_badge: bool = "display_local_badge" => true;
        language_badge: bool = "display_language_badge" => false;
        new_show_updates_count: bool = "library_show_updates_count" => true;
        auto_clear_item_cache: bool = "auto_clear_chapter_cache" => false;
        random_anime_sort_seed: i32 = "library_random_anime_sort_seed" => 0;
        random_manga_sort_seed: i32 = "library_random_manga_sort_seed" => 0;
        anime_portrait_columns: i32 = "pref_animelib_columns_portrait_key" => 0;
        manga_portrait_columns: i32 = "pref_library_columns_portrait_key" => 0;
        anime_landscape_columns: i32 = "pref_animelib_columns_landscape_key" => 0;
        manga_landscape_columns: i32 = "pref_library_columns_landscape_key" => 0;
        filter_downloaded_anime: TriState = "pref_filter_animelib_downloaded_v2" => TriState::DISABLED;
        filter_downloaded_manga: TriState = "pref_filter_library_downloaded_v2" => TriState::DISABLED;
        filter_unseen: TriState = "pref_filter_animelib_unread_v2" => TriState::DISABLED;
        filter_unread: TriState = "pref_filter_library_unread_v2" => TriState::DISABLED;
        filter_started_anime: TriState = "pref_filter_animelib_started_v2" => TriState::DISABLED;
        filter_started_manga: TriState = "pref_filter_library_started_v2" => TriState::DISABLED;
        filter_bookmarked_anime: TriState = "pref_filter_animelib_bookmarked_v2" => TriState::DISABLED;
        filter_bookmarked_manga: TriState = "pref_filter_library_bookmarked_v2" => TriState::DISABLED;
        filter_completed_anime: TriState = "pref_filter_animelib_completed_v2" => TriState::DISABLED;
        filter_completed_manga: TriState = "pref_filter_library_completed_v2" => TriState::DISABLED;
        new_manga_updates_count: i32 = "library_unread_updates_count" => 0;
        new_anime_updates_count: i32 = "library_unseen_updates_count" => 0;
        default_anime_category: i32 = "default_anime_category" => -1;
        default_manga_category: i32 = "default_category" => -1;
        last_used_anime_category: i32 = "__APP_STATE_last_used_anime_category" => 0;
        last_used_manga_category: i32 = "__APP_STATE_last_used_category" => 0;
        anime_update_categories: BTreeSet<String> = "animelib_update_categories" => BTreeSet::new();
        manga_update_categories: BTreeSet<String> = "library_update_categories" => BTreeSet::new();
        anime_update_categories_exclude: BTreeSet<String> = "animelib_update_categories_exclude" => BTreeSet::new();
        manga_update_categories_exclude: BTreeSet<String> = "library_update_categories_exclude" => BTreeSet::new();
        mark_duplicate_read_chapter_as_read: BTreeSet<String> = "mark_duplicate_read_chapter_read" => BTreeSet::new();
        mark_duplicate_seen_episode_as_seen: BTreeSet<String> = "mark_duplicate_seen_episode_seen" => BTreeSet::new();
        filter_episode_by_seen: i64 = "default_episode_filter_by_seen" => 0;
        filter_chapter_by_read: i64 = "default_chapter_filter_by_read" => 0;
        filter_episode_by_downloaded: i64 = "default_episode_filter_by_downloaded" => 0;
        filter_chapter_by_downloaded: i64 = "default_chapter_filter_by_downloaded" => 0;
        filter_episode_by_bookmarked: i64 = "default_episode_filter_by_bookmarked" => 0;
        filter_chapter_by_bookmarked: i64 = "default_chapter_filter_by_bookmarked" => 0;
        filter_episode_by_fillermarked: i64 = "default_episode_filter_by_fillermarked" => 0;
        sort_episode_by_source_or_number: i64 = "default_episode_sort_by_source_or_number" => 0;
        sort_chapter_by_source_or_number: i64 = "default_chapter_sort_by_source_or_number" => 0;
        /// Shares the key of `display_chapter_by_name_or_number` (Kotlin twin).
        display_episode_by_name_or_number: i64 = "default_chapter_display_by_name_or_number" => 0;
        /// Shares the key of `display_episode_by_name_or_number` (Kotlin twin).
        display_chapter_by_name_or_number: i64 = "default_chapter_display_by_name_or_number" => 0;
        /// Shares the key of `sort_chapter_by_ascending_or_descending` (Kotlin twin).
        sort_episode_by_ascending_or_descending: i64 = "default_chapter_sort_by_ascending_or_descending" => 0;
        /// Shares the key of `sort_episode_by_ascending_or_descending` (Kotlin twin).
        sort_chapter_by_ascending_or_descending: i64 = "default_chapter_sort_by_ascending_or_descending" => 0;
        show_episode_thumbnail_previews: i64 = "default_episode_show_thumbnail_previews" => 0;
        show_episode_summaries: i64 = "default_episode_show_summaries" => 0;
        filter_season_by_download: i64 = "default_season_filter_by_downloaded" => 0;
        filter_season_by_unseen: i64 = "default_season_filter_by_unseen" => 0;
        filter_season_by_started: i64 = "default_season_filter_by_started" => 0;
        filter_season_by_completed: i64 = "default_season_filter_by_completed" => 0;
        filter_season_by_bookmarked: i64 = "default_season_filter_by_bookmarked" => 0;
        filter_season_by_fillermarked: i64 = "default_season_filter_by_fillermarked" => 0;
        sort_season_by_source_or_number: i64 = "default_season_sort_by_source_or_number" => 0;
        sort_season_by_ascending_or_descending: i64 = "default_season_sort_by_ascending_or_descending" => 0;
        season_display_grid_mode: i64 = "default_season_grid_display_mode" => 0;
        season_display_grid_size: i32 = "default_season_grid_display_size" => 0;
        season_download_overlay: bool = "default_season_download_overlay" => false;
        season_unseen_overlay: bool = "default_season_unseen_overlay" => true;
        season_local_overlay: bool = "default_season_local_overlay" => true;
        season_lang_overlay: bool = "default_season_lang_overlay" => false;
        season_continue_overlay: bool = "default_season_continue_overlay" => true;
        season_display_mode: i64 = "default_season_display_mode" => 0;
        update_season_on_refresh: bool = "pref_update_season_on_refresh" => false;
        update_season_on_library_update: bool = "pref_update_season_on_library_update" => false;
    }
}

/// `LibraryDisplayMode` (`LibraryDisplayMode.kt`). One key serves both libraries and is stored as the
/// spelling of [`LibraryDisplayMode::serialize`].
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum LibraryDisplayMode {
    CompactGrid,
    ComfortableGrid,
    List,
    CoverOnlyGrid,
}

impl LibraryDisplayMode {
    /// Menu order of the four modes.
    pub const ALL: [LibraryDisplayMode; 4] = [Self::CompactGrid, Self::ComfortableGrid, Self::List, Self::CoverOnlyGrid];
    pub const DEFAULT: Self = Self::CompactGrid;

    /// The stored spelling.
    pub const fn serialize(self) -> &'static str {
        match self {
            Self::CompactGrid => "COMPACT_GRID",
            Self::ComfortableGrid => "COMFORTABLE_GRID",
            Self::List => "LIST",
            Self::CoverOnlyGrid => "COVER_ONLY_GRID",
        }
    }

    /// The mode stored as `stored`, or `None` for any other text (lower case and empty included).
    pub fn from_serialized(stored: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.serialize() == stored)
    }

    /// Kotlin `deserialize`: the mode stored as `stored`, and [`LibraryDisplayMode::DEFAULT`] for any other text.
    pub fn deserialize(stored: &str) -> Self {
        Self::from_serialized(stored).unwrap_or(Self::DEFAULT)
    }
}

impl PrefType for LibraryDisplayMode {
    fn encode(&self) -> PrefValue {
        PrefValue::Str(self.serialize().to_owned())
    }

    fn decode(v: &PrefValue) -> Decoded<Self> {
        match v {
            PrefValue::Str(s) => Self::from_serialized(s).map_or(Decoded::Invalid, Decoded::Ok),
            _ => Decoded::WrongType,
        }
    }
}

/// Type tokens of the manga library sort (`MangaLibrarySort.Serializer`).
pub const MANGA_SORT_TOKENS: &[&str] = &[
    "ALPHABETICAL",
    "LAST_READ",
    "LAST_MANGA_UPDATE",
    "UNREAD_COUNT",
    "TOTAL_CHAPTERS",
    "LATEST_CHAPTER",
    "CHAPTER_FETCH_DATE",
    "DATE_ADDED",
    "TRACKER_MEAN",
    "RANDOM",
];

/// Type tokens of the anime library sort (`AnimeLibrarySort.Serializer`).
pub const ANIME_SORT_TOKENS: &[&str] = &[
    "ALPHABETICAL",
    "LAST_SEEN",
    "LAST_ANIME_UPDATE",
    "UNSEEN_COUNT",
    "TOTAL_EPISODES",
    "LATEST_EPISODE",
    "EPISODE_FETCH_DATE",
    "DATE_ADDED",
    "TRACKER_MEAN",
    "AIRING_TIME",
    "RANDOM",
];

/// A stored `<TYPE>,<DIRECTION>` library sort such as `LAST_READ,DESCENDING`. `ANIME` picks the type tokens of
/// that kind. Text with an unknown type, an unknown direction or no comma is `Invalid`: the default is read and
/// the stored text stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SortSetting<const ANIME: bool>(pub String);

impl<const ANIME: bool> PrefType for SortSetting<ANIME> {
    fn encode(&self) -> PrefValue {
        PrefValue::Str(self.0.clone())
    }

    /// Read like Kotlin's `getObject`: the text is parsed and the value comes back in canonical
    /// form (an unknown type reads as the default type, see [`crate::sort::LibrarySort::deserialize`]).
    /// The stored text is not rewritten by a read. Empty text or text without a comma is `Invalid`.
    fn decode(v: &PrefValue) -> Decoded<Self> {
        let PrefValue::Str(s) = v else {
            return Decoded::WrongType;
        };
        let canonical = if ANIME {
            AnimeLibrarySort::deserialize(s).map(|sort| sort.serialize())
        } else {
            MangaLibrarySort::deserialize(s).map(|sort| sort.serialize())
        };
        canonical.map_or(Decoded::Invalid, |text| Decoded::Ok(Self(text)))
    }
}

impl SortSetting<false> {
    /// The typed manga sort. Text without a comma gives the default sort.
    pub fn sort(&self) -> MangaLibrarySort {
        MangaLibrarySort::deserialize(&self.0).unwrap_or_else(MangaLibrarySort::default_sort)
    }
}

impl SortSetting<true> {
    /// The typed anime sort. Text without a comma gives the default sort.
    pub fn sort(&self) -> AnimeLibrarySort {
        AnimeLibrarySort::deserialize(&self.0).unwrap_or_else(AnimeLibrarySort::default_sort)
    }
}

/// `SeasonDisplayMode` (`aniyomi.domain.anime.SeasonDisplayMode`). Stored as a Long; the numbers
/// are not the order of [`LibraryDisplayMode`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeasonDisplayMode {
    CompactGrid,
    ComfortableGrid,
    CoverOnlyGrid,
    List,
}

/// A season display number outside 0 to 3 (Kotlin `IllegalArgumentException("Invalid sorting value")`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("Invalid sorting value: {0}")]
pub struct InvalidSortingValue(pub i64);

impl SeasonDisplayMode {
    pub fn from_long(v: i64) -> Result<Self, InvalidSortingValue> {
        match v {
            0 => Ok(Self::CompactGrid),
            1 => Ok(Self::ComfortableGrid),
            2 => Ok(Self::CoverOnlyGrid),
            3 => Ok(Self::List),
            other => Err(InvalidSortingValue(other)),
        }
    }

    pub const fn to_long(self) -> i64 {
        match self {
            Self::CompactGrid => 0,
            Self::ComfortableGrid => 1,
            Self::CoverOnlyGrid => 2,
            Self::List => 3,
        }
    }

    /// The stored number read as a mode: any invalid number gives [`SeasonDisplayMode::CompactGrid`].
    pub fn from_stored(v: i64) -> Self {
        Self::from_long(v).unwrap_or(Self::CompactGrid)
    }
}

/// A handle for a family key, with its default taken from the registry row for that key.
/// Panics when the key is not registered or its default does not decode as `T`. Both are
/// programming errors, and the test that builds [`LibraryPrefs`] catches them.
fn reg<T: PrefType>(store: &PrefStore, key: impl Into<Cow<'static, str>>) -> Pref<T> {
    let key = key.into();
    let (meta, _) =
        registry::lookup(&key).unwrap_or_else(|| panic!("unregistered preference key {key}"));
    let default = match T::decode(&meta.default.value()) {
        Decoded::Ok(v) => v,
        _ => panic!("registry default of {key} does not match the handle type"),
    };
    store.pref(key, default)
}

impl LibraryPrefs {
    pub fn filter_tracked_anime(&self, tracker_id: i32) -> Pref<TriState> {
        reg(
            &self.store,
            format!("pref_filter_animelib_tracked_{tracker_id}_v2"),
        )
    }

    pub fn filter_tracked_manga(&self, tracker_id: i32) -> Pref<TriState> {
        reg(
            &self.store,
            format!("pref_filter_library_tracked_{tracker_id}_v2"),
        )
    }
}

impl LibraryPrefs {
    /// `setChapterSettingsDefault`: copies the filters, sort and display bits of a manga's `chapter_flags`
    /// into the default templates. The display key is shared with the episode template (Kotlin twin keys).
    pub fn set_chapter_settings_default(&self, chapter_flags: i64) -> Result<(), PrefError> {
        self.filter_chapter_by_read.set(chapter_flags & CHAPTER_UNREAD_MASK)?;
        self.filter_chapter_by_downloaded.set(chapter_flags & CHAPTER_DOWNLOADED_MASK)?;
        self.filter_chapter_by_bookmarked.set(chapter_flags & CHAPTER_BOOKMARKED_MASK)?;
        self.sort_chapter_by_source_or_number.set(chapter_flags & CHAPTER_SORTING_MASK)?;
        self.display_chapter_by_name_or_number.set(chapter_flags & CHAPTER_DISPLAY_MASK)?;
        self.sort_chapter_by_ascending_or_descending.set(chapter_flags & CHAPTER_SORT_DIR_MASK)?;
        Ok(())
    }

    /// `setEpisodeSettingsDefault`: the episode counterpart of [`LibraryPrefs::set_chapter_settings_default`],
    /// with the thumbnail and summary bits too.
    pub fn set_episode_settings_default(&self, episode_flags: i64) -> Result<(), PrefError> {
        self.filter_episode_by_seen.set(episode_flags & EPISODE_UNSEEN_MASK)?;
        self.filter_episode_by_downloaded.set(episode_flags & EPISODE_DOWNLOADED_MASK)?;
        self.filter_episode_by_bookmarked.set(episode_flags & EPISODE_BOOKMARKED_MASK)?;
        self.filter_episode_by_fillermarked.set(episode_flags & EPISODE_FILLERMARKED_MASK)?;
        self.sort_episode_by_source_or_number.set(episode_flags & EPISODE_SORTING_MASK)?;
        self.display_episode_by_name_or_number.set(episode_flags & EPISODE_DISPLAY_MASK)?;
        self.sort_episode_by_ascending_or_descending.set(episode_flags & EPISODE_SORT_DIR_MASK)?;
        self.show_episode_thumbnail_previews.set(episode_flags & EPISODE_PREVIEWS_MASK)?;
        self.show_episode_summaries.set(episode_flags & EPISODE_SUMMARIES_MASK)?;
        Ok(())
    }
}

impl LibraryPrefs {
    /// The season grid mode as a typed value. An invalid stored number reads as `CompactGrid`.
    pub fn season_grid_mode(&self) -> SeasonDisplayMode {
        SeasonDisplayMode::from_stored(self.season_display_grid_mode.get())
    }

    /// `setSeasonSettingsDefault` for an anime's `season_flags`: copies the filters, sort, grid, overlay and title
    /// bits into the default templates. Deviation from Kotlin (quirk Q3): the downloaded filter is taken from the
    /// downloaded mask, not from the unseen mask.
    pub fn set_season_settings_default(&self, season_flags: i64) -> Result<(), PrefError> {
        self.filter_season_by_download.set(season_flags & SEASON_DOWNLOADED_MASK)?;
        self.filter_season_by_unseen.set(season_flags & SEASON_UNSEEN_MASK)?;
        self.filter_season_by_started.set(season_flags & SEASON_STARTED_MASK)?;
        self.filter_season_by_completed.set(season_flags & SEASON_COMPLETED_MASK)?;
        self.filter_season_by_bookmarked.set(season_flags & SEASON_BOOKMARKED_MASK)?;
        self.filter_season_by_fillermarked.set(season_flags & SEASON_FILLERMARKED_MASK)?;
        self.sort_season_by_source_or_number.set(season_flags & SEASON_SORT_MASK)?;
        self.sort_season_by_ascending_or_descending.set(season_flags & SEASON_SORT_DIR_MASK)?;
        self.season_display_grid_mode
            .set((season_flags & SEASON_GRID_DISPLAY_MODE_MASK) >> SEASON_GRID_DISPLAY_MODE_SHIFT)?;
        self.season_display_grid_size
            .set(((season_flags & SEASON_GRID_DISPLAY_SIZE_MASK) >> SEASON_GRID_DISPLAY_SIZE_SHIFT) as i32)?;
        self.season_download_overlay.set(season_flags & SEASON_OVERLAY_DOWNLOADED_MASK != 0)?;
        self.season_unseen_overlay.set(season_flags & SEASON_OVERLAY_UNSEEN_MASK != 0)?;
        self.season_local_overlay.set(season_flags & SEASON_OVERLAY_LOCAL_MASK != 0)?;
        self.season_lang_overlay.set(season_flags & SEASON_OVERLAY_LANG_MASK != 0)?;
        self.season_continue_overlay.set(season_flags & SEASON_OVERLAY_CONT_MASK != 0)?;
        self.season_display_mode.set(season_flags & SEASON_DISPLAY_MODE_MASK)?;
        Ok(())
    }
}

/// `LibraryPreferences` companion constants: the device restrictions of library updates.
pub const DEVICE_ONLY_ON_WIFI: &str = "wifi";
pub const DEVICE_NETWORK_NOT_METERED: &str = "network_not_metered";
pub const DEVICE_CHARGING: &str = "ac";

/// Entry restrictions of library updates (manga and anime share the key).
pub const ENTRY_NON_COMPLETED: &str = "manga_ongoing";
pub const ENTRY_HAS_UNVIEWED: &str = "manga_fully_read";
pub const ENTRY_NON_VIEWED: &str = "manga_started";
pub const ENTRY_OUTSIDE_RELEASE_PERIOD: &str = "manga_outside_release_period";

/// Values of the "mark duplicate as read" multi-select preferences.
pub const MARK_DUPLICATE_CHAPTER_READ_NEW: &str = "new";
pub const MARK_DUPLICATE_CHAPTER_READ_EXISTING: &str = "existing";
pub const MARK_DUPLICATE_EPISODE_SEEN_NEW: &str = "new_episode";
pub const MARK_DUPLICATE_EPISODE_SEEN_EXISTING: &str = "existing_episode";

pub const DEFAULT_MANGA_CATEGORY_PREF_KEY: &str = "default_category";
pub const DEFAULT_ANIME_CATEGORY_PREF_KEY: &str = "default_anime_category";
pub const LIBRARY_UPDATE_MANGA_CATEGORIES_PREF_KEY: &str = "library_update_categories";
pub const LIBRARY_UPDATE_ANIME_CATEGORIES_PREF_KEY: &str = "animelib_update_categories";
pub const LIBRARY_UPDATE_MANGA_CATEGORIES_EXCLUDE_PREF_KEY: &str = "library_update_categories_exclude";
pub const LIBRARY_UPDATE_ANIME_CATEGORIES_EXCLUDE_PREF_KEY: &str = "animelib_update_categories_exclude";

/// The six keys that hold category ids (`categoryPreferenceKeys`). Backup restore remaps them.
pub fn category_preference_keys() -> [&'static str; 6] {
    [
        DEFAULT_MANGA_CATEGORY_PREF_KEY,
        DEFAULT_ANIME_CATEGORY_PREF_KEY,
        LIBRARY_UPDATE_MANGA_CATEGORIES_PREF_KEY,
        LIBRARY_UPDATE_ANIME_CATEGORIES_PREF_KEY,
        LIBRARY_UPDATE_MANGA_CATEGORIES_EXCLUDE_PREF_KEY,
        LIBRARY_UPDATE_ANIME_CATEGORIES_EXCLUDE_PREF_KEY,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::assert_section_registered;
    use crate::values::PrefValue;

    #[test]
    fn constructing_resolves_every_key_in_the_registry() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        assert_eq!(
            lib.show_continue_viewing_button.key(),
            "display_continue_reading_button"
        );
        assert_eq!(lib.display_mode.key(), "pref_display_mode_library");
        assert_eq!(lib.manga_sorting_mode.key(), "library_sorting_mode");
        assert_eq!(lib.anime_sorting_mode.key(), "animelib_sorting_mode");
        assert_eq!(
            lib.last_updated_timestamp.key(),
            "__APP_STATE_library_update_last_timestamp"
        );
    }

    #[test]
    fn every_field_is_registered_under_the_library_table() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        assert_eq!(LibraryPrefs::TABLE, "library");
        assert_section_registered(LibraryPrefs::TABLE, LibraryPrefs::FIELDS, &lib.defaults());
    }

    #[test]
    fn defaults_match_the_registry() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        assert!(!lib.show_continue_viewing_button.get());
        assert!(lib.category_tabs.get());
        assert!(!lib.category_number_of_items.get());
        assert!(!lib.categorized_display_settings.get());
        assert!(!lib.hide_hidden_categories_settings.get());
        assert!(!lib.download_badge.get());
        assert!(lib.unread_badge.get());
        assert!(lib.local_badge.get());
        assert!(!lib.language_badge.get());
        assert!(lib.new_show_updates_count.get());
        assert!(!lib.auto_clear_item_cache.get());
        assert_eq!(lib.random_manga_sort_seed.get(), 0);
        assert_eq!(lib.random_anime_sort_seed.get(), 0);
        assert_eq!(lib.manga_portrait_columns.get(), 0);
        assert_eq!(lib.anime_landscape_columns.get(), 0);
        assert_eq!(lib.new_manga_updates_count.get(), 0);
        assert_eq!(lib.new_anime_updates_count.get(), 0);
        assert_eq!(lib.default_manga_category.get(), -1);
        assert_eq!(lib.default_anime_category.get(), -1);
        assert_eq!(lib.last_used_manga_category.get(), 0);
        assert_eq!(lib.display_mode.get(), LibraryDisplayMode::CompactGrid);
        assert_eq!(lib.manga_sorting_mode.get().0, "ALPHABETICAL,ASCENDING");
        assert_eq!(
            lib.auto_update_device_restrictions.get(),
            BTreeSet::from(["wifi".to_owned()])
        );
        assert_eq!(lib.filter_downloaded_manga.get(), TriState::DISABLED);
        assert_eq!(lib.filter_interval_custom.get(), TriState::DISABLED);
    }

    #[test]
    fn tracked_filters_use_the_tracker_id_in_the_key() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        let p = lib.filter_tracked_manga(7);
        assert_eq!(p.key(), "pref_filter_library_tracked_7_v2");
        assert_eq!(p.get(), TriState::DISABLED);
        p.set(TriState::ENABLED_NOT).unwrap();
        assert_eq!(
            store.get_raw("pref_filter_library_tracked_7_v2"),
            Some(PrefValue::Str("ENABLED_NOT".into()))
        );
        assert_eq!(
            lib.filter_tracked_anime(7).key(),
            "pref_filter_animelib_tracked_7_v2"
        );
    }

    #[test]
    fn a_filter_stored_as_another_kind_is_reset() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.set_raw("pref_filter_library_downloaded_v2", PrefValue::Int(1));
        assert_eq!(lib.filter_downloaded_manga.get(), TriState::DISABLED);
        assert!(!lib.filter_downloaded_manga.is_set());
    }

    /// Table of `p1-data-and-config.md` section 5 (83 rows, the four swipe rows are dropped). Columns: index, key, namespace, Rust field, kind
    /// (`bool`, `int`, `long`, `str`, `set`) and the default in stored form (a set is comma-separated, empty for `{}`).
    const SECTION_5: &str = include_str!("../tests/fixtures/library_prefs_table.tsv");

    fn section_5_rows() -> Vec<Vec<&'static str>> {
        SECTION_5
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split('\t').collect())
            .collect()
    }

    /// The stored key of a section 5 row: app-state keys carry the `__APP_STATE_` prefix.
    fn stored_key(row: &[&str]) -> String {
        if row[2] == "app-state" {
            format!("__APP_STATE_{}", row[1])
        } else {
            row[1].to_owned()
        }
    }

    fn fixture_value(kind: &str, text: &str) -> PrefValue {
        match kind {
            "bool" => PrefValue::Bool(text.parse().unwrap()),
            "int" => PrefValue::Int(text.parse().unwrap()),
            "long" => PrefValue::Long(text.parse().unwrap()),
            "str" => PrefValue::Str(text.to_owned()),
            "set" => PrefValue::Set(text.split(',').filter(|s| !s.is_empty()).map(str::to_owned).collect()),
            other => panic!("unknown fixture kind {other}"),
        }
    }

    fn kind_name(kind: registry::Kind) -> &'static str {
        match kind {
            registry::Kind::Bool => "bool",
            registry::Kind::Int => "int",
            registry::Kind::Long => "long",
            registry::Kind::Float => "float",
            registry::Kind::Str => "str",
            registry::Kind::Set => "set",
        }
    }

    #[test]
    fn section_5_table_has_83_accessors_registered_with_their_namespace() {
        let rows = section_5_rows();
        assert_eq!(rows.len(), 83);
        let fields: Vec<&str> = LibraryPrefs::FIELDS.iter().map(|(_, key)| *key).collect();
        for row in &rows {
            let stored = stored_key(row);
            let (meta, _) = registry::lookup(&stored).unwrap_or_else(|| panic!("{stored} not registered"));
            let expected = if row[2] == "app-state" { "AppState" } else { "Normal" };
            assert_eq!(format!("{:?}", meta.ns), expected, "namespace of {stored}");
            assert!(fields.contains(&stored.as_str()) || row[1].contains("{id}"), "{stored} has no LibraryPrefs field");
        }
        let app_state: Vec<&str> = rows.iter().filter(|r| r[2] == "app-state").map(|r| r[1]).collect();
        assert_eq!(app_state, ["library_update_last_timestamp", "last_used_anime_category", "last_used_category"]);
    }

    #[test]
    fn section_5_rows_match_key_kind_and_default() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        let handle_defaults = lib.defaults();
        for row in section_5_rows() {
            let stored = stored_key(&row);
            let (meta, _) = registry::lookup(&stored).unwrap_or_else(|| panic!("{stored} not registered"));
            let expected = fixture_value(row[4], row[5]);
            assert_eq!(kind_name(meta.kind), row[4], "registry kind of {stored}");
            assert_eq!(meta.default.value(), expected, "registry default of {stored}");
            if !stored.contains("{id}") {
                let (_, handle) = handle_defaults
                    .iter()
                    .find(|(key, _)| *key == stored)
                    .unwrap_or_else(|| panic!("{stored} has no LibraryPrefs handle"));
                assert_eq!(handle, &expected, "LibraryPrefs default and kind of {stored}");
            }
        }
    }

    #[test]
    fn shared_keys_and_category_key_list_follow_the_table() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        lib.display_chapter_by_name_or_number.set(2).unwrap();
        assert_eq!(lib.display_episode_by_name_or_number.get(), 2, "Q1: one stored key");
        lib.sort_chapter_by_ascending_or_descending.set(1).unwrap();
        assert_eq!(lib.sort_episode_by_ascending_or_descending.get(), 1);
        assert_eq!(category_preference_keys().len(), 6);
        assert!(category_preference_keys().contains(&"default_category"));
    }

    #[test]
    fn the_dropped_swipe_keys_have_no_accessor_and_no_registry_row() {
        let fields: Vec<&str> = LibraryPrefs::FIELDS.iter().map(|(_, key)| *key).collect();
        for key in [
            "pref_chapter_swipe_end_action",
            "pref_chapter_swipe_start_action",
            "pref_episode_swipe_end_action",
            "pref_episode_swipe_start_action",
        ] {
            assert!(!fields.contains(&key), "{key} has a LibraryPrefs field");
            assert!(registry::lookup(key).is_none(), "{key} is registered");
            assert!(registry::is_dropped(key), "{key} is not in DROPPED_KEYS");
        }
    }

    #[test]
    fn known_enum_names_decode_to_their_variants() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.set_raw("pref_filter_library_downloaded_v2", PrefValue::Str("ENABLED_NOT".into()));
        assert_eq!(lib.filter_downloaded_manga.get(), TriState::ENABLED_NOT);
        store.set_raw("pref_display_mode_library", PrefValue::Str("COVER_ONLY_GRID".into()));
        assert_eq!(lib.display_mode.get(), LibraryDisplayMode::CoverOnlyGrid);
    }

    #[test]
    fn an_unknown_tristate_name_reads_the_default_and_stay_stored() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.set_raw("pref_filter_library_downloaded_v2", PrefValue::Str("BANANA".into()));
        assert_eq!(lib.filter_downloaded_manga.get(), TriState::DISABLED, "TriState BANANA reads as DISABLED");
        assert_eq!(store.get_raw("pref_filter_library_downloaded_v2"), Some(PrefValue::Str("BANANA".into())));
    }

    #[test]
    fn unknown_display_mode_and_sort_strings_read_the_default_and_stay_stored() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        store.set_raw("pref_display_mode_library", PrefValue::Str("list".into()));
        assert_eq!(lib.display_mode.get(), LibraryDisplayMode::CompactGrid, "lower case reads as compact");
        assert_eq!(store.get_raw("pref_display_mode_library"), Some(PrefValue::Str("list".into())));

        // Edge cases E3 to E5: an unknown type keeps the direction, only the exact word ASCENDING is ascending,
        // and text without a comma or empty text reads as the default.
        let cases = [
            ("Banana,ASCENDING", "ALPHABETICAL,ASCENDING"),
            ("LAST_READ,SIDEWAYS", "LAST_READ,DESCENDING"),
            ("LAST_READ", "ALPHABETICAL,ASCENDING"),
            ("AIRING_TIME,DESCENDING", "ALPHABETICAL,DESCENDING"),
            ("", "ALPHABETICAL,ASCENDING"),
        ];
        for (stored, expected) in cases {
            store.set_raw("library_sorting_mode", PrefValue::Str(stored.into()));
            assert_eq!(lib.manga_sorting_mode.get().0, expected, "manga sort {stored:?}");
            assert_eq!(store.get_raw("library_sorting_mode"), Some(PrefValue::Str(stored.into())));
        }
        store.set_raw("animelib_sorting_mode", PrefValue::Str("AIRING_TIME,DESCENDING".into()));
        assert_eq!(lib.anime_sorting_mode.get().0, "AIRING_TIME,DESCENDING", "airing time is an anime sort");
        store.set_raw("library_sorting_mode", PrefValue::Str("LAST_READ,DESCENDING".into()));
        assert_eq!(lib.manga_sorting_mode.get().0, "LAST_READ,DESCENDING");
    }

    #[test]
    fn display_mode_text_that_is_unknown_reads_compact_grid() {
        assert_eq!(LibraryDisplayMode::deserialize("COVER_ONLY_GRID"), LibraryDisplayMode::CoverOnlyGrid);
        assert_eq!(LibraryDisplayMode::deserialize("cover"), LibraryDisplayMode::CompactGrid);
        assert_eq!(LibraryDisplayMode::deserialize(""), LibraryDisplayMode::CompactGrid);
    }

    #[test]
    fn season_display_mode_numbers_run_from_0_to_3() {
        for (number, mode) in [
            (0, SeasonDisplayMode::CompactGrid),
            (1, SeasonDisplayMode::ComfortableGrid),
            (2, SeasonDisplayMode::CoverOnlyGrid),
            (3, SeasonDisplayMode::List),
        ] {
            assert_eq!(SeasonDisplayMode::from_long(number), Ok(mode));
            assert_eq!(mode.to_long(), number);
        }
        assert_eq!(SeasonDisplayMode::from_long(7), Err(InvalidSortingValue(7)));
        assert_eq!(SeasonDisplayMode::from_stored(7), SeasonDisplayMode::CompactGrid);
    }

    #[test]
    fn the_typed_sort_reads_the_stored_text() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store);
        lib.manga_sorting_mode.set(SortSetting("LAST_READ,DESCENDING".into())).unwrap();
        assert_eq!(lib.manga_sorting_mode.get().sort(), crate::sort::MangaLibrarySort::deserialize("LAST_READ,DESCENDING").unwrap());
        lib.anime_sorting_mode.set(SortSetting("AIRING_TIME,ASCENDING".into())).unwrap();
        assert_eq!(lib.anime_sorting_mode.get().sort().to_flag(), 0x30 | 0x40);
    }

    #[test]
    fn set_chapter_default_copies_the_manga_flag_fields() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        // read filter ENABLED_NOT (0x4), downloaded (0x8), bookmarked (0x20), number sort (0x100), number display, descending (0x0)
        lib.set_chapter_settings_default(0x4 | 0x8 | 0x20 | 0x100 | 0x100000).unwrap();
        assert_eq!(lib.filter_chapter_by_read.get(), 0x4);
        assert_eq!(lib.filter_chapter_by_downloaded.get(), 0x8);
        assert_eq!(lib.filter_chapter_by_bookmarked.get(), 0x20);
        assert_eq!(lib.sort_chapter_by_source_or_number.get(), 0x100);
        assert_eq!(lib.sort_chapter_by_ascending_or_descending.get(), 0);
        assert_eq!(lib.display_chapter_by_name_or_number.get(), 0x100000);
        // the display key is shared with the episode template
        assert_eq!(lib.display_episode_by_name_or_number.get(), 0x100000);
    }

    #[test]
    fn set_episode_default_maps_the_seen_mask_and_the_preview_bit() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        // seen filter SHOW_SEEN (0x4), filler marked (0x80), number sort (0x200), ascending (0x1), previews off (0x800)
        lib.set_episode_settings_default(0x4 | 0x80 | 0x200 | 0x1 | 0x800).unwrap();
        assert_eq!(lib.filter_episode_by_seen.get(), 0x4);
        assert_eq!(lib.filter_episode_by_fillermarked.get(), 0x80);
        assert_eq!(lib.sort_episode_by_source_or_number.get(), 0x200);
        assert_eq!(lib.sort_episode_by_ascending_or_descending.get(), 1);
        assert_eq!(lib.show_episode_thumbnail_previews.get(), 0x800);
        assert_eq!(lib.show_episode_summaries.get(), 0);
    }
}

#[cfg(test)]
mod season_tests {
    use super::*;
    use crate::flags::{SEASON_GRID_DISPLAY_MODE_SHIFT, SEASON_GRID_DISPLAY_SIZE_SHIFT, SEASON_OVERLAY_LOCAL_MASK, SEASON_SHOW_DOWNLOADED, SEASON_SHOW_SEEN};
    use crate::store::PrefStore;
    use crate::values::PrefValue;

    #[test]
    fn the_downloaded_filter_comes_from_the_downloaded_mask_not_the_unseen_mask() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        // Downloaded bits 0x2 and unseen bits 0x10 (quirk Q3 regression guard).
        lib.set_season_settings_default(SEASON_SHOW_DOWNLOADED | SEASON_SHOW_SEEN).unwrap();
        assert_eq!(lib.filter_season_by_download.get(), SEASON_SHOW_DOWNLOADED);
        assert_eq!(lib.filter_season_by_unseen.get(), SEASON_SHOW_SEEN);
    }

    #[test]
    fn grid_mode_and_size_are_read_from_their_shifted_fields() {
        let store = PrefStore::in_memory();
        let lib = LibraryPrefs::new(store.clone());
        lib.set_season_settings_default((2 << SEASON_GRID_DISPLAY_MODE_SHIFT) | (5 << SEASON_GRID_DISPLAY_SIZE_SHIFT)).unwrap();
        assert_eq!(lib.season_display_grid_mode.get(), 2);
        assert_eq!(lib.season_display_grid_size.get(), 5);
        assert_eq!(lib.season_grid_mode(), SeasonDisplayMode::CoverOnlyGrid);
    }

    #[test]
    fn overlays_are_on_when_their_bit_is_set() {
        let lib = LibraryPrefs::new(PrefStore::in_memory());
        lib.set_season_settings_default(SEASON_OVERLAY_LOCAL_MASK).unwrap();
        assert!(lib.season_local_overlay.get());
        assert!(!lib.season_unseen_overlay.get(), "unset bit means off, the default of true is overwritten");
        assert!(!lib.season_continue_overlay.get());
    }

    #[test]
    fn an_invalid_stored_grid_number_reads_as_compact_grid() {
        let store = PrefStore::in_memory();
        store.set_raw("default_season_grid_display_mode", PrefValue::Long(7));
        let lib = LibraryPrefs::new(store);
        assert_eq!(lib.season_grid_mode(), SeasonDisplayMode::CompactGrid);
    }
}
