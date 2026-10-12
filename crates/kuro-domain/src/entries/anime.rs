//! Anime entry behaviour: `episode_flags`, `season_flags` and `viewer_flags` (layouts in
//! `p1-data-and-config.md` section 3), the season-list predicate, and the `to_update` / `apply_update`
//! pair (`Anime.kt`, `AnimeUpdate.kt`, `SetAnimeViewerFlags.kt`).
//!
//! The struct lives in `kuro-data`, so the behaviour is an extension trait implemented here.

use kuro_data::anime::{Anime, AnimeUpdate};
use kuro_data::entry::{Entry, EntryUpdate};

use super::cover::AnimeCover;
use super::flags::{Cover, EntryFlags, ItemFlags, SeasonFlags, ViewerFlags};
use super::season_display::SeasonDisplayMode;
use crate::flags::TriState;

/// `episode_flags` constants: the unified item layout (`kuro_db::flags::item`) under the old names of the seen/unseen group.
pub mod episode_flags {
    pub use kuro_db::flags::item::*;

    pub const SHOW_ALL: i64 = 0x0;
    pub const SHOW_UNSEEN: i64 = SHOW_UNDONE;
    pub const SHOW_SEEN: i64 = SHOW_DONE;
    pub const UNSEEN_MASK: i64 = UNDONE_MASK;
}

/// `season_flags` constants (`kuro_db::flags::season`), stored on the parent entry.
pub mod season_flags {
    pub use kuro_db::flags::season::*;

    pub const GRID_DISPLAY_MODE_BIT_OFFSET: u32 = GRID_DISPLAY_MODE_SHIFT as u32;
    pub const GRID_DISPLAY_SIZE_BIT_OFFSET: u32 = GRID_DISPLAY_SIZE_SHIFT as u32;
}

/// `viewer_flags` layout (`p1-data-and-config.md` 3.4): the shared masks of [`super::flags`].
pub mod viewer_flags {
    pub use super::super::flags::{AIRING_EPISODE_MASK, AIRING_TIME_MASK, INTRO_DISABLE_MASK, INTRO_MASK};
}

/// Accessors, setters and update helpers over an `Anime` row.
pub trait AnimeExt {
    /// `episode_flags & SORTING_MASK`.
    fn sorting(&self) -> i64;
    /// `episode_flags & DISPLAY_MASK`.
    fn display_mode(&self) -> i64;
    /// `episode_flags & UNSEEN_MASK`.
    fn unseen_filter_raw(&self) -> i64;
    /// Unseen filter: `SHOW_UNSEEN` is `EnabledIs`, `SHOW_SEEN` is `EnabledNot`.
    fn unseen_filter(&self) -> TriState;
    /// `episode_flags & DOWNLOADED_MASK`.
    fn downloaded_filter_raw(&self) -> i64;
    /// `episode_flags & BOOKMARKED_MASK`.
    fn bookmarked_filter_raw(&self) -> i64;
    fn bookmarked_filter(&self) -> TriState;
    /// `episode_flags & FILLERMARKED_MASK`.
    fn fillermarked_filter_raw(&self) -> i64;
    fn fillermarked_filter(&self) -> TriState;
    /// `(episode_flags & SORT_DIR_MASK) == SORT_DESC`.
    fn sort_descending(&self) -> bool;
    /// Thumbnail previews are shown when the NOT bit is clear.
    fn show_previews(&self) -> bool;
    /// Summaries are shown when the NOT bit is clear.
    fn show_summaries(&self) -> bool;

    fn season_downloaded_filter_raw(&self) -> i64;
    fn season_unseen_filter_raw(&self) -> i64;
    fn season_started_filter_raw(&self) -> i64;
    fn season_completed_filter_raw(&self) -> i64;
    fn season_bookmarked_filter_raw(&self) -> i64;
    fn season_fillermarked_filter_raw(&self) -> i64;
    fn season_downloaded_filter(&self) -> TriState;
    fn season_unseen_filter(&self) -> TriState;
    fn season_started_filter(&self) -> TriState;
    fn season_completed_filter(&self) -> TriState;
    fn season_bookmarked_filter(&self) -> TriState;
    fn season_fillermarked_filter(&self) -> TriState;
    /// `season_flags & SORT_MASK`.
    fn season_sorting(&self) -> i64;
    /// `(season_flags & SORT_DIR_MASK) == SORT_DESC`.
    fn season_sort_descending(&self) -> bool;
    /// Grid mode from bits 16-17. The field is two bits wide, so every value maps to a mode.
    fn season_display_grid_mode(&self) -> SeasonDisplayMode;
    /// Grid size from bits 18-21, 0 meaning automatic columns.
    fn season_display_grid_size(&self) -> i32;
    fn season_downloaded_overlay(&self) -> bool;
    fn season_unseen_overlay(&self) -> bool;
    fn season_local_overlay(&self) -> bool;
    fn season_lang_overlay(&self) -> bool;
    fn season_continue_overlay(&self) -> bool;
    /// `season_flags & DISPLAY_MODE_MASK`.
    fn season_display_mode(&self) -> i64;

    /// Skip-intro length in seconds (bits 0-7).
    fn skip_intro_length(&self) -> i32;
    /// True when the skip-intro button is disabled (bit 56).
    fn skip_intro_disable(&self) -> bool;
    /// Next episode to air (bits 8-23).
    fn next_episode_to_air(&self) -> i32;
    /// Airing time of the next episode in epoch seconds (bits 24-55).
    fn next_episode_airing_at(&self) -> i64;
    /// `SetAnimeViewerFlags.setSkipIntroLength`: sets the length (clamped to 0..=255) and the disable bit
    /// (set when the length is 0).
    fn set_skip_intro_length(&mut self, length: i32);
    /// Writes both airing groups in one change (fixes Q17). The episode is clamped to 0..=65535 and the
    /// airing time to 0..=2^32-1.
    fn set_next_episode_airing(&mut self, episode: i32, airing_at: i64);

    /// `Anime.asAnimeCover()`.
    fn as_cover(&self) -> AnimeCover;
    /// `Anime.toAnimeUpdate()`: all 28 optional fields filled from the entry.
    fn to_update(&self) -> AnimeUpdate;
    /// Applies the `Some` fields of `u` to the entry. `None` leaves the field untouched.
    fn apply_update(&mut self, u: &AnimeUpdate);
}

/// The three words of an anime, as the shared accessor types read them.
fn item(a: &Anime) -> ItemFlags {
    ItemFlags(a.episode_flags)
}

fn season(a: &Anime) -> SeasonFlags {
    SeasonFlags(a.season_flags)
}

fn viewer(a: &Anime) -> ViewerFlags {
    ViewerFlags(a.viewer_flags)
}

impl AnimeExt for Anime {
    fn sorting(&self) -> i64 {
        item(self).sorting()
    }
    fn display_mode(&self) -> i64 {
        item(self).display_mode()
    }
    fn unseen_filter_raw(&self) -> i64 {
        item(self).undone_filter_raw()
    }
    fn unseen_filter(&self) -> TriState {
        item(self).undone_filter()
    }
    fn downloaded_filter_raw(&self) -> i64 {
        item(self).downloaded_filter_raw()
    }
    fn bookmarked_filter_raw(&self) -> i64 {
        item(self).bookmarked_filter_raw()
    }
    fn bookmarked_filter(&self) -> TriState {
        item(self).bookmarked_filter()
    }
    fn fillermarked_filter_raw(&self) -> i64 {
        item(self).fillermarked_filter_raw()
    }
    fn fillermarked_filter(&self) -> TriState {
        item(self).fillermarked_filter()
    }
    fn sort_descending(&self) -> bool {
        item(self).sort_descending()
    }
    fn show_previews(&self) -> bool {
        item(self).show_previews()
    }
    fn show_summaries(&self) -> bool {
        item(self).show_summaries()
    }
    fn season_downloaded_filter_raw(&self) -> i64 {
        season(self).downloaded_filter_raw()
    }
    fn season_unseen_filter_raw(&self) -> i64 {
        season(self).unseen_filter_raw()
    }
    fn season_started_filter_raw(&self) -> i64 {
        season(self).started_filter_raw()
    }
    fn season_completed_filter_raw(&self) -> i64 {
        season(self).completed_filter_raw()
    }
    fn season_bookmarked_filter_raw(&self) -> i64 {
        season(self).bookmarked_filter_raw()
    }
    fn season_fillermarked_filter_raw(&self) -> i64 {
        season(self).fillermarked_filter_raw()
    }
    fn season_downloaded_filter(&self) -> TriState {
        season(self).downloaded_filter()
    }
    fn season_unseen_filter(&self) -> TriState {
        season(self).unseen_filter()
    }
    fn season_started_filter(&self) -> TriState {
        season(self).started_filter()
    }
    fn season_completed_filter(&self) -> TriState {
        season(self).completed_filter()
    }
    fn season_bookmarked_filter(&self) -> TriState {
        season(self).bookmarked_filter()
    }
    fn season_fillermarked_filter(&self) -> TriState {
        season(self).fillermarked_filter()
    }
    fn season_sorting(&self) -> i64 {
        season(self).sorting()
    }
    fn season_sort_descending(&self) -> bool {
        season(self).sort_descending()
    }
    fn season_display_grid_mode(&self) -> SeasonDisplayMode {
        season(self).grid_mode()
    }
    fn season_display_grid_size(&self) -> i32 {
        season(self).grid_size()
    }
    fn season_downloaded_overlay(&self) -> bool {
        season(self).downloaded_overlay()
    }
    fn season_unseen_overlay(&self) -> bool {
        season(self).unseen_overlay()
    }
    fn season_local_overlay(&self) -> bool {
        season(self).local_overlay()
    }
    fn season_lang_overlay(&self) -> bool {
        season(self).lang_overlay()
    }
    fn season_continue_overlay(&self) -> bool {
        season(self).continue_overlay()
    }
    fn season_display_mode(&self) -> i64 {
        season(self).display_mode()
    }
    fn skip_intro_length(&self) -> i32 {
        viewer(self).skip_intro_length()
    }
    fn skip_intro_disable(&self) -> bool {
        viewer(self).skip_intro_disable()
    }
    fn next_episode_to_air(&self) -> i32 {
        viewer(self).next_episode_to_air()
    }
    fn next_episode_airing_at(&self) -> i64 {
        viewer(self).next_episode_airing_at()
    }
    fn set_skip_intro_length(&mut self, length: i32) {
        self.viewer_flags = viewer(self).with_skip_intro_length(length).0;
    }
    fn set_next_episode_airing(&mut self, episode: i32, airing_at: i64) {
        self.viewer_flags = viewer(self).with_next_episode_airing(episode, airing_at).0;
    }

    fn as_cover(&self) -> AnimeCover {
        Cover { entry_id: self.id, source_id: self.source, favorite: self.favorite, url: self.thumbnail_url.clone(), last_modified: self.cover_last_modified }.into()
    }

    fn to_update(&self) -> AnimeUpdate {
        AnimeUpdate {
            id: self.id,
            source: Some(self.source),
            url: Some(self.url.clone()),
            title: Some(self.title.clone()),
            artist: self.artist.clone(),
            author: self.author.clone(),
            description: self.description.clone(),
            genre: self.genre.clone(),
            status: Some(self.status),
            thumbnail_url: self.thumbnail_url.clone(),
            background_url: self.background_url.clone(),
            favorite: Some(self.favorite),
            last_update: Some(self.last_update),
            next_update: Some(self.next_update),
            fetch_interval: Some(self.fetch_interval),
            date_added: Some(self.date_added),
            viewer_flags: Some(self.viewer_flags),
            episode_flags: Some(self.episode_flags),
            cover_last_modified: Some(self.cover_last_modified),
            background_last_modified: Some(self.background_last_modified),
            update_strategy: Some(self.update_strategy),
            initialized: Some(self.initialized),
            version: Some(self.version),
            fetch_type: Some(self.fetch_type),
            parent_id: self.parent_id,
            season_flags: Some(self.season_flags),
            season_number: Some(self.season_number),
            season_source_order: Some(self.season_source_order),
            memo: Some(self.memo.clone()),
        }
    }

    fn apply_update(&mut self, u: &AnimeUpdate) {
        let mut entry = Entry::from(&*self);
        entry.apply_update(&EntryUpdate::from(u));
        *self = Anime::from(entry);
    }
}

/// `seasonsFiltered`: true when any of the six season filters is on (see [`SeasonFlags::filtered`]).
pub fn seasons_filtered(anime: &Anime, downloaded_only: bool) -> bool {
    season(anime).filtered(downloaded_only)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_data::anime::Anime;
    use kuro_db::codecs::UpdateStrategy;
    use kuro_db::flags::set_flag;
    use kuro_db::schema_anime::FetchType;

    fn with_episode_flags(flags: i64) -> Anime {
        Anime {
            episode_flags: flags,
            ..Anime::create()
        }
    }

    fn with_season_flags(flags: i64) -> Anime {
        Anime {
            season_flags: flags,
            ..Anime::create()
        }
    }

    fn with_viewer_flags(flags: i64) -> Anime {
        Anime {
            viewer_flags: flags,
            ..Anime::create()
        }
    }

    fn sample() -> Anime {
        Anime {
            id: 21,
            source: 31,
            url: "/anime/9".into(),
            title: "Show".into(),
            artist: None,
            author: Some("Studio".into()),
            description: Some("desc".into()),
            genre: Some(vec!["action".into()]),
            status: 1,
            thumbnail_url: Some("https://img/9.jpg".into()),
            background_url: Some("https://img/bg9.jpg".into()),
            favorite: true,
            last_update: 3,
            next_update: 4,
            fetch_interval: 7,
            date_added: 5,
            viewer_flags: 0x0068_E778_0000_0D5A,
            episode_flags: 0x101B42,
            cover_last_modified: 6,
            background_last_modified: 8,
            update_strategy: UpdateStrategy::OnlyFetchOnce,
            initialized: true,
            last_modified_at: 9,
            favorite_modified_at: Some(10),
            version: 11,
            fetch_type: FetchType::Seasons,
            parent_id: Some(2),
            season_flags: 0x5970000,
            season_number: 2.0,
            season_source_order: 4,
            memo: serde_json::Map::new(),
        }
    }

    #[test]
    fn create_matches_the_documented_defaults() {
        let a = Anime::create();
        assert_eq!((a.id, a.source), (-1, -1));
        assert_eq!(a.fetch_type, FetchType::Episodes);
        assert_eq!(a.season_number, -1.0);
        assert!(a.memo.is_empty());
    }

    #[test]
    fn episode_flag_vectors_from_the_topic_table() {
        let mut flags = 0x0;
        flags = set_flag(
            flags,
            episode_flags::SORTING_NUMBER,
            episode_flags::SORTING_MASK,
        );
        assert_eq!(flags, 0x200);
        flags = set_flag(flags, episode_flags::SORT_ASC, episode_flags::SORT_DIR_MASK);
        assert_eq!(flags, 0x201);
        flags = set_flag(
            flags,
            episode_flags::SORT_DESC,
            episode_flags::SORT_DIR_MASK,
        );
        assert_eq!(flags, 0x200, "flip to descending");
        flags = set_flag(
            flags,
            episode_flags::SHOW_UNSEEN,
            episode_flags::UNSEEN_MASK,
        );
        assert_eq!(flags, 0x202);
        flags = set_flag(
            flags,
            episode_flags::SHOW_NOT_BOOKMARKED,
            episode_flags::BOOKMARKED_MASK,
        );
        assert_eq!(flags, 0x242);
        flags = set_flag(
            flags,
            episode_flags::SHOW_NOT_FILLERMARKED,
            episode_flags::FILLERMARKED_MASK,
        );
        assert_eq!(flags, 0x342);
        flags = set_flag(
            flags,
            episode_flags::SHOW_NOT_PREVIEWS,
            episode_flags::PREVIEWS_MASK,
        );
        flags = set_flag(
            flags,
            episode_flags::SHOW_NOT_SUMMARIES,
            episode_flags::SUMMARIES_MASK,
        );
        flags = set_flag(
            flags,
            episode_flags::DISPLAY_NUMBER,
            episode_flags::DISPLAY_MASK,
        );
        assert_eq!(flags, 0x101B42);
        let a = with_episode_flags(flags);
        assert!(!a.show_previews() && !a.show_summaries());
        assert_eq!(a.display_mode(), episode_flags::DISPLAY_NUMBER);
    }

    #[test]
    fn episode_filters_named_vectors() {
        assert_eq!(
            with_episode_flags(0x2).unseen_filter(),
            TriState::ENABLED_IS
        );
        assert_eq!(
            with_episode_flags(0x4).unseen_filter(),
            TriState::ENABLED_NOT
        );
        assert_eq!(with_episode_flags(0x6).unseen_filter(), TriState::DISABLED);
        assert_eq!(
            with_episode_flags(0x80).fillermarked_filter(),
            TriState::ENABLED_IS
        );
        assert_eq!(
            with_episode_flags(0x100).fillermarked_filter(),
            TriState::ENABLED_NOT
        );
        assert_eq!(with_episode_flags(0x18).downloaded_filter_raw(), 0x18);
        assert_eq!(
            with_episode_flags(0x400).sorting(),
            episode_flags::SORTING_UPLOAD_DATE
        );
        assert!(with_episode_flags(0x0).show_previews());
        assert!(with_episode_flags(0x0).show_summaries());
    }

    #[test]
    fn episode_accessors_agree_with_the_masks_over_30_words() {
        let words: [i64; 30] = [
            0x0, 0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x8, 0x10, 0x18, 0x20, 0x40, 0x60, 0x80, 0x100,
            0x180, 0x200, 0x400, 0x600, 0x800, 0x1000, 0x1800, 0x100000, 0x201, 0x202, 0x242,
            0x342, 0x101B42, 0x7FF, 0x3FFF,
        ];
        for w in words {
            let a = with_episode_flags(w);
            assert_eq!(a.sorting(), w & 0x600, "sorting {w:#x}");
            assert_eq!(a.sort_descending(), w & 0x1 == 0, "direction {w:#x}");
            assert_eq!(a.show_previews(), w & 0x800 == 0, "previews {w:#x}");
            assert_eq!(a.show_summaries(), w & 0x1000 == 0, "summaries {w:#x}");
            assert_eq!(a.bookmarked_filter_raw(), w & 0x60, "bookmarked {w:#x}");
            let expected_seen = match w & 0x6 {
                0x2 => TriState::ENABLED_IS,
                0x4 => TriState::ENABLED_NOT,
                _ => TriState::DISABLED,
            };
            assert_eq!(a.unseen_filter(), expected_seen, "unseen {w:#x}");
        }
    }

    #[test]
    fn season_flag_vectors_and_grid_fields() {
        let a = with_season_flags(0x5970000);
        assert_eq!(a.season_display_grid_mode(), SeasonDisplayMode::List);
        assert_eq!(a.season_display_grid_size(), 5);
        assert!(
            a.season_unseen_overlay() && a.season_local_overlay() && a.season_continue_overlay()
        );
        assert!(!a.season_downloaded_overlay() && !a.season_lang_overlay());
        assert_eq!(a.season_sorting(), season_flags::SORT_SOURCE);
        assert!(a.season_sort_descending());

        let sorted = with_season_flags(0x2001);
        assert_eq!(sorted.season_sorting(), season_flags::SORT_SEASON);
        assert!(!sorted.season_sort_descending());
    }

    #[test]
    fn default_season_word_has_three_overlays() {
        let a = with_season_flags(0x5800000);
        assert_eq!(a.season_display_grid_mode(), SeasonDisplayMode::CompactGrid);
        assert_eq!(a.season_display_grid_size(), 0);
        assert!(
            a.season_unseen_overlay() && a.season_local_overlay() && a.season_continue_overlay()
        );
        assert!(!a.season_downloaded_overlay() && !a.season_lang_overlay());
        assert_eq!(a.season_display_mode(), season_flags::DISPLAY_MODE_SOURCE);
    }

    #[test]
    fn season_filters_map_is_and_not_per_group() {
        type Getter = fn(&Anime) -> TriState;
        let pairs: [(i64, Getter); 6] = [
            (0x2, |a| a.season_downloaded_filter()),
            (0x8, |a| a.season_unseen_filter()),
            (0x20, |a| a.season_started_filter()),
            (0x80, |a| a.season_completed_filter()),
            (0x200, |a| a.season_bookmarked_filter()),
            (0x800, |a| a.season_fillermarked_filter()),
        ];
        for (is_bit, read) in pairs {
            assert_eq!(
                read(&with_season_flags(is_bit)),
                TriState::ENABLED_IS,
                "is {is_bit:#x}"
            );
            assert_eq!(
                read(&with_season_flags(is_bit << 1)),
                TriState::ENABLED_NOT,
                "not {is_bit:#x}"
            );
            assert_eq!(read(&with_season_flags(0)), TriState::DISABLED);
        }
    }

    #[test]
    fn seasons_filtered_considers_six_filters_and_downloaded_only() {
        assert!(!seasons_filtered(&with_season_flags(0), false));
        assert!(
            !seasons_filtered(&with_season_flags(0x5800000), false),
            "overlays are not filters"
        );
        assert!(
            seasons_filtered(&with_season_flags(0x1000), false),
            "fillermarked NOT"
        );
        assert!(
            seasons_filtered(&with_season_flags(0x8), false),
            "unseen IS"
        );
        assert!(
            seasons_filtered(&with_season_flags(0), true),
            "downloaded only forces a filter"
        );
    }

    #[test]
    fn season_sort_code_0xe000_is_source_and_not_a_named_code() {
        let a = with_season_flags(0xE000);
        assert_eq!(a.season_sorting(), 0xE000);
        assert_ne!(a.season_sorting(), season_flags::SORT_SOURCE);
    }

    #[test]
    fn viewer_vector_decodes_and_round_trips() {
        let a = with_viewer_flags(0x0068_E778_0000_0D5A);
        assert_eq!(a.skip_intro_length(), 90);
        assert!(!a.skip_intro_disable());
        assert_eq!(a.next_episode_to_air(), 13);
        assert_eq!(a.next_episode_airing_at(), 1_760_000_000);
    }

    #[test]
    fn skip_intro_zero_sets_disable_bit_and_clears_length() {
        let mut a = with_viewer_flags(0x0068_E778_0000_0D5A);
        a.set_skip_intro_length(0);
        assert_eq!(a.viewer_flags, 0x0168_E778_0000_0D00);
        assert!(a.skip_intro_disable());
        a.set_skip_intro_length(5);
        assert!(!a.skip_intro_disable());
        assert_eq!(a.skip_intro_length(), 5);
    }

    #[test]
    fn skip_intro_length_is_masked_to_eight_bits_like_the_kotlin_setter() {
        let mut a = with_viewer_flags(0);
        a.set_skip_intro_length(300);
        assert_eq!(a.skip_intro_length(), 44, "EC-21");
        assert!(!a.skip_intro_disable());
        a.set_skip_intro_length(-4);
        assert_eq!(a.skip_intro_length(), 252);
        a.set_skip_intro_length(256);
        assert_eq!(a.skip_intro_length(), 0, "a masked zero is a disabled intro");
        assert!(a.skip_intro_disable());
    }

    #[test]
    fn set_next_episode_airing_writes_both_groups_and_keeps_intro() {
        let mut a = with_viewer_flags(0x5A);
        a.set_next_episode_airing(13, 1_760_000_000);
        assert_eq!(a.viewer_flags, 0x0068_E778_0000_0D5A);
        assert_eq!(a.next_episode_to_air(), 13);
        assert_eq!(a.next_episode_airing_at(), 1_760_000_000);
        assert_eq!(a.skip_intro_length(), 90);
    }

    #[test]
    fn to_update_fills_all_twenty_eight_fields() {
        let u = sample().to_update();
        let filled = [
            u.source.is_some(),
            u.url.is_some(),
            u.title.is_some(),
            u.status.is_some(),
            u.favorite.is_some(),
            u.last_update.is_some(),
            u.next_update.is_some(),
            u.fetch_interval.is_some(),
            u.date_added.is_some(),
            u.viewer_flags.is_some(),
            u.episode_flags.is_some(),
            u.cover_last_modified.is_some(),
            u.background_last_modified.is_some(),
            u.thumbnail_url.is_some(),
            u.background_url.is_some(),
            u.update_strategy.is_some(),
            u.initialized.is_some(),
            u.version.is_some(),
            u.fetch_type.is_some(),
            u.parent_id.is_some(),
            u.season_flags.is_some(),
            u.season_number.is_some(),
            u.season_source_order.is_some(),
            u.memo.is_some(),
            u.artist.is_some(),
            u.author.is_some(),
            u.description.is_some(),
            u.genre.is_some(),
        ];
        assert_eq!(
            filled.iter().filter(|b| **b).count(),
            27,
            "artist is None in the sample"
        );
        assert_eq!(u.id, 21);
    }

    #[test]
    fn apply_of_to_update_restores_every_covered_field() {
        let original = sample();
        let mut changed = original.clone();
        changed.title = "Changed".into();
        changed.viewer_flags = 0;
        changed.episode_flags = 0;
        changed.season_flags = 0;
        changed.parent_id = None;
        changed
            .memo
            .insert("k".into(), serde_json::Value::Bool(true));
        changed.apply_update(&original.to_update());
        assert_eq!(changed, original);
    }

    #[test]
    fn apply_none_fields_keep_the_stored_text() {
        let mut a = sample();
        let mut u = AnimeUpdate::new(21);
        u.title = Some("New".into());
        a.apply_update(&u);
        assert_eq!(a.title, "New");
        assert_eq!(a.author.as_deref(), Some("Studio"));
        assert_eq!(a.parent_id, Some(2));
    }
}
