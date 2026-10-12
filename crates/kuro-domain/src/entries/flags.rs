//! The one flag layer of both kinds: accessors over `entries.item_flags` (the unified layout of
//! `kuro_db::flags::item`), the season word, the viewer bits, the expected-update rule and the cover key.
//!
//! The logic is on three small word types ([`ItemFlags`], [`SeasonFlags`], [`ViewerFlags`]). [`EntryFlags`] answers
//! from an [`Entry`]; the per-kind `MangaExt` and `AnimeExt` wrappers answer from the same types after translating
//! their own word. Anime-only questions answer from the `Option` fields and read as zero or "off" for manga.

use kuro_data::entry::{Entry, EntryUpdate};
pub use kuro_db::flags::item;
use kuro_db::flags::season;

use super::season_display::SeasonDisplayMode;
use crate::flags::{TriState, set_flag, tri};

/// `SManga.COMPLETED` / `SAnime.COMPLETED`: a completed entry has no predicted next release.
pub const STATUS_COMPLETED: i64 = 2;

/// `None` only for a completed entry; otherwise the stored `next_update`. The one rule of both kinds.
pub fn expected_next_update(status: i64, next_update: i64) -> Option<i64> {
    (status != STATUS_COMPLETED).then_some(next_update)
}

/// A manga sorting value (`chapter_flags & SORTING_MASK`, the manga layout) in the unified layout. `None` for a value
/// outside the sorting group, which has no sorting to translate.
pub fn manga_sorting_to_unified(sorting: i64) -> Option<i64> {
    (sorting & !kuro_db::flags::chapter::SORTING_MASK == 0).then(|| kuro_db::flags::manga_item_flags_to_unified(sorting))
}

/// The cache key of a cover: a change of `last_modified` or of `favorite` yields a different key.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Cover {
    pub entry_id: i64,
    pub source_id: i64,
    pub favorite: bool,
    pub url: Option<String>,
    pub last_modified: i64,
}

/// The unified item word of an entry (`entries.item_flags`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemFlags(pub i64);

impl ItemFlags {
    pub fn sorting(self) -> i64 {
        self.0 & item::SORTING_MASK
    }
    pub fn display_mode(self) -> i64 {
        self.0 & item::DISPLAY_MASK
    }
    pub fn undone_filter_raw(self) -> i64 {
        self.0 & item::UNDONE_MASK
    }
    /// `SHOW_UNDONE` is `EnabledIs`, `SHOW_DONE` is `EnabledNot`.
    pub fn undone_filter(self) -> TriState {
        tri(self.undone_filter_raw(), item::SHOW_UNDONE, item::SHOW_DONE)
    }
    pub fn downloaded_filter_raw(self) -> i64 {
        self.0 & item::DOWNLOADED_MASK
    }
    pub fn bookmarked_filter_raw(self) -> i64 {
        self.0 & item::BOOKMARKED_MASK
    }
    pub fn bookmarked_filter(self) -> TriState {
        tri(self.bookmarked_filter_raw(), item::SHOW_BOOKMARKED, item::SHOW_NOT_BOOKMARKED)
    }
    pub fn fillermarked_filter_raw(self) -> i64 {
        self.0 & item::FILLERMARKED_MASK
    }
    pub fn fillermarked_filter(self) -> TriState {
        tri(self.fillermarked_filter_raw(), item::SHOW_FILLERMARKED, item::SHOW_NOT_FILLERMARKED)
    }
    pub fn sort_descending(self) -> bool {
        self.0 & item::SORT_DIR_MASK == item::SORT_DESC
    }
    /// Thumbnail previews are shown while the NOT bit is clear.
    pub fn show_previews(self) -> bool {
        self.0 & item::PREVIEWS_MASK == item::SHOW_PREVIEWS
    }
    /// Summaries are shown while the NOT bit is clear.
    pub fn show_summaries(self) -> bool {
        self.0 & item::SUMMARIES_MASK == item::SHOW_SUMMARIES
    }
}

/// The season word of an anime parent (`entries.season_flags`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeasonFlags(pub i64);

impl SeasonFlags {
    pub fn downloaded_filter_raw(self) -> i64 {
        self.0 & season::DOWNLOADED_MASK
    }
    pub fn unseen_filter_raw(self) -> i64 {
        self.0 & season::UNSEEN_MASK
    }
    pub fn started_filter_raw(self) -> i64 {
        self.0 & season::STARTED_MASK
    }
    pub fn completed_filter_raw(self) -> i64 {
        self.0 & season::COMPLETED_MASK
    }
    pub fn bookmarked_filter_raw(self) -> i64 {
        self.0 & season::BOOKMARKED_MASK
    }
    pub fn fillermarked_filter_raw(self) -> i64 {
        self.0 & season::FILLERMARKED_MASK
    }
    pub fn downloaded_filter(self) -> TriState {
        tri(self.downloaded_filter_raw(), season::SHOW_DOWNLOADED, season::SHOW_NOT_DOWNLOADED)
    }
    pub fn unseen_filter(self) -> TriState {
        tri(self.unseen_filter_raw(), season::SHOW_UNSEEN, season::SHOW_SEEN)
    }
    pub fn started_filter(self) -> TriState {
        tri(self.started_filter_raw(), season::SHOW_STARTED, season::SHOW_NOT_STARTED)
    }
    pub fn completed_filter(self) -> TriState {
        tri(self.completed_filter_raw(), season::SHOW_COMPLETED, season::SHOW_NOT_COMPLETED)
    }
    pub fn bookmarked_filter(self) -> TriState {
        tri(self.bookmarked_filter_raw(), season::SHOW_BOOKMARKED, season::SHOW_NOT_BOOKMARKED)
    }
    pub fn fillermarked_filter(self) -> TriState {
        tri(self.fillermarked_filter_raw(), season::SHOW_FILLERMARKED, season::SHOW_NOT_FILLERMARKED)
    }
    pub fn sorting(self) -> i64 {
        self.0 & season::SORT_MASK
    }
    pub fn sort_descending(self) -> bool {
        self.0 & season::SORT_DIR_MASK == season::SORT_DESC
    }
    /// Grid mode from bits 16-17. The field is two bits wide, so every value maps to a mode.
    pub fn grid_mode(self) -> SeasonDisplayMode {
        let raw = (self.0 & season::GRID_DISPLAY_MODE_MASK) >> season::GRID_DISPLAY_MODE_SHIFT;
        SeasonDisplayMode::from_i64(raw).unwrap_or(SeasonDisplayMode::CompactGrid)
    }
    /// Grid size from bits 18-21, 0 meaning automatic columns.
    pub fn grid_size(self) -> i32 {
        ((self.0 & season::GRID_DISPLAY_SIZE_MASK) >> season::GRID_DISPLAY_SIZE_SHIFT) as i32
    }
    pub fn downloaded_overlay(self) -> bool {
        self.0 & season::OVERLAY_DOWNLOADED_MASK != 0
    }
    pub fn unseen_overlay(self) -> bool {
        self.0 & season::OVERLAY_UNSEEN_MASK != 0
    }
    pub fn local_overlay(self) -> bool {
        self.0 & season::OVERLAY_LOCAL_MASK != 0
    }
    pub fn lang_overlay(self) -> bool {
        self.0 & season::OVERLAY_LANG_MASK != 0
    }
    pub fn continue_overlay(self) -> bool {
        self.0 & season::OVERLAY_CONT_MASK != 0
    }
    pub fn display_mode(self) -> i64 {
        self.0 & season::DISPLAY_MODE_MASK
    }
    /// True when any of the six season filters is on. With downloaded-only mode the downloaded filter counts as
    /// `EnabledIs` whatever is stored.
    pub fn filtered(self, downloaded_only: bool) -> bool {
        let downloaded = if downloaded_only { TriState::ENABLED_IS } else { self.downloaded_filter() };
        [
            downloaded,
            self.unseen_filter(),
            self.started_filter(),
            self.completed_filter(),
            self.bookmarked_filter(),
            self.fillermarked_filter(),
        ]
        .iter()
        .any(|f| *f != TriState::DISABLED)
    }
}

/// The anime viewer bits (`entries.viewer`): skip-intro length and the next airing. Manga's bits are reader modes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ViewerFlags(pub i64);

/// Skip-intro length in seconds, bits 0-7.
pub const INTRO_MASK: i64 = 0xFF;
/// Next episode to air, bits 8-23.
pub const AIRING_EPISODE_MASK: i64 = 0x00FF_FF00;
/// Next episode airing time in epoch seconds, bits 24-55.
pub const AIRING_TIME_MASK: i64 = 0xFFFF_FFFF << 24;
/// Skip-intro disabled, bit 56.
pub const INTRO_DISABLE_MASK: i64 = 1 << 56;
/// Manga reading mode, bits 0-2.
pub const READING_MODE_MASK: i64 = 0x7;

impl ViewerFlags {
    pub fn skip_intro_length(self) -> i32 {
        (self.0 & INTRO_MASK) as i32
    }
    pub fn skip_intro_disable(self) -> bool {
        self.0 & INTRO_DISABLE_MASK == INTRO_DISABLE_MASK
    }
    pub fn next_episode_to_air(self) -> i32 {
        ((self.0 & AIRING_EPISODE_MASK) >> 8) as i32
    }
    pub fn next_episode_airing_at(self) -> i64 {
        (self.0 & AIRING_TIME_MASK) >> 24
    }
    /// The word after storing the length in its eight bits (`length and 0xFF`, EC-21: 300 stores 44, -1 stores 255); a
    /// stored length of zero sets the disable bit.
    pub fn with_skip_intro_length(self, length: i32) -> Self {
        let (value, mask) = skip_intro_bits(i64::from(length));
        Self(set_flag(self.0, value, mask))
    }
    /// The word after storing the episode in 16 bits and the airing time in 32 bits, each truncated by its mask (EC-22).
    pub fn with_next_episode_airing(self, episode: i32, airing_at: i64) -> Self {
        let (value, mask) = airing_bits(i64::from(episode), airing_at);
        Self(set_flag(self.0, value, mask))
    }
}

/// The `(value, mask)` of a skip-intro write: the masked length and the disable bit (set when the length is 0).
pub fn skip_intro_bits(length: i64) -> (i64, i64) {
    let length = length & INTRO_MASK;
    let disable = if length == 0 { INTRO_DISABLE_MASK } else { 0 };
    (length | disable, INTRO_MASK | INTRO_DISABLE_MASK)
}

/// The `(value, mask)` of a next-airing write: both groups at once (Q17).
pub fn airing_bits(episode: i64, airing_at: i64) -> (i64, i64) {
    (((episode & 0xFFFF) << 8) | ((airing_at & 0xFFFF_FFFF) << 24), AIRING_EPISODE_MASK | AIRING_TIME_MASK)
}

/// The skip-intro length the player uses: disabled gives 0, a stored 0 gives the default, else the stored one.
pub fn effective_intro_length(viewer: i64, default_intro_length: i64) -> i64 {
    if viewer & INTRO_DISABLE_MASK == INTRO_DISABLE_MASK {
        return 0;
    }
    match viewer & INTRO_MASK {
        0 => default_intro_length,
        stored => stored,
    }
}

/// Accessors and update helpers over an [`Entry`] of either kind.
pub trait EntryFlags {
    fn item_flags(&self) -> ItemFlags;
    fn sorting(&self) -> i64;
    fn display_mode(&self) -> i64;
    fn sort_descending(&self) -> bool;
    fn undone_filter(&self) -> TriState;
    fn bookmarked_filter(&self) -> TriState;
    /// `Disabled` on a manga entry: the fillermark group never applies to it.
    fn fillermarked_filter(&self) -> TriState;
    fn downloaded_filter_raw(&self) -> i64;
    fn show_previews(&self) -> bool;
    fn show_summaries(&self) -> bool;
    fn season(&self) -> SeasonFlags;
    fn viewer(&self) -> ViewerFlags;
    /// `None` only for a completed entry; otherwise the stored `next_update`.
    fn expected_next_update(&self) -> Option<i64>;
    fn as_cover(&self) -> Cover;
    /// Every field filled from the entry (the nullable columns stay `None` when the entry has none).
    fn to_update(&self) -> EntryUpdate;
    /// Applies the `Some` fields of `u`. `None` leaves the field untouched; text is never cleared.
    fn apply_update(&mut self, u: &EntryUpdate);
    /// Stores the skip-intro length (anime; a no-op word change for manga is the caller's business).
    fn set_skip_intro_length(&mut self, length: i32);
    fn set_next_episode_airing(&mut self, episode: i32, airing_at: i64);
}

impl EntryFlags for Entry {
    fn item_flags(&self) -> ItemFlags {
        ItemFlags(self.item_flags)
    }
    fn sorting(&self) -> i64 {
        self.item_flags().sorting()
    }
    fn display_mode(&self) -> i64 {
        self.item_flags().display_mode()
    }
    fn sort_descending(&self) -> bool {
        self.item_flags().sort_descending()
    }
    fn undone_filter(&self) -> TriState {
        self.item_flags().undone_filter()
    }
    fn bookmarked_filter(&self) -> TriState {
        self.item_flags().bookmarked_filter()
    }
    fn fillermarked_filter(&self) -> TriState {
        if self.kind.is_anime() { self.item_flags().fillermarked_filter() } else { TriState::DISABLED }
    }
    fn downloaded_filter_raw(&self) -> i64 {
        self.item_flags().downloaded_filter_raw()
    }
    fn show_previews(&self) -> bool {
        self.item_flags().show_previews()
    }
    fn show_summaries(&self) -> bool {
        self.item_flags().show_summaries()
    }
    fn season(&self) -> SeasonFlags {
        SeasonFlags(self.season_flags())
    }
    fn viewer(&self) -> ViewerFlags {
        ViewerFlags(self.viewer_flags)
    }
    fn expected_next_update(&self) -> Option<i64> {
        expected_next_update(self.status, self.next_update)
    }
    fn as_cover(&self) -> Cover {
        Cover {
            entry_id: self.id,
            source_id: self.source,
            favorite: self.favorite,
            url: self.thumbnail_url.clone(),
            last_modified: self.cover_last_modified,
        }
    }
    fn to_update(&self) -> EntryUpdate {
        EntryUpdate {
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
            item_flags: Some(self.item_flags),
            cover_last_modified: Some(self.cover_last_modified),
            background_last_modified: self.background_last_modified,
            update_strategy: Some(self.update_strategy),
            initialized: Some(self.initialized),
            version: Some(self.version),
            fetch_type: self.fetch_type,
            parent_id: self.parent_id,
            season_flags: self.season_flags,
            season_number: self.season_number,
            season_source_order: self.season_source_order,
            memo: self.memo.clone(),
        }
    }
    fn apply_update(&mut self, u: &EntryUpdate) {
        macro_rules! set {
            ($($f:ident),*) => {$( if let Some(v) = &u.$f { self.$f = v.clone(); } )*};
        }
        macro_rules! set_some {
            ($($f:ident),*) => {$( if let Some(v) = &u.$f { self.$f = Some(v.clone()); } )*};
        }
        set!(source, url, title, status, favorite, last_update, next_update, fetch_interval, date_added, viewer_flags,
            item_flags, cover_last_modified, update_strategy, initialized, version);
        set_some!(artist, author, description, genre, thumbnail_url, background_url, background_last_modified, fetch_type,
            parent_id, season_flags, season_number, season_source_order, memo);
    }
    fn set_skip_intro_length(&mut self, length: i32) {
        self.viewer_flags = self.viewer().with_skip_intro_length(length).0;
    }
    fn set_next_episode_airing(&mut self, episode: i32, airing_at: i64) {
        self.viewer_flags = self.viewer().with_next_episode_airing(episode, airing_at).0;
    }
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;

    use super::*;

    fn entry(kind: Kind, word: i64) -> Entry {
        Entry { item_flags: word, ..Entry::create(kind) }
    }

    #[test]
    fn accessors_agree_with_the_masks_for_both_kinds() {
        for kind in [Kind::Manga, Kind::Anime] {
            for w in [0x0, 0x1, 0x2, 0x4, 0x6, 0x18, 0x20, 0x40, 0x60, 0x80, 0x100, 0x200, 0x400, 0x600, 0x800, 0x1000, 0x100000, 0x101B42] {
                let e = entry(kind, w);
                assert_eq!(e.sorting(), w & 0x600, "{kind} sorting {w:#x}");
                assert_eq!(e.display_mode(), w & 0x100000);
                assert_eq!(e.sort_descending(), w & 1 == 0);
                assert_eq!(e.downloaded_filter_raw(), w & 0x18);
                let expected = match w & 0x6 {
                    0x2 => TriState::ENABLED_IS,
                    0x4 => TriState::ENABLED_NOT,
                    _ => TriState::DISABLED,
                };
                assert_eq!(e.undone_filter(), expected);
            }
        }
    }

    #[test]
    fn the_fillermark_group_never_applies_to_manga() {
        assert_eq!(entry(Kind::Manga, item::SHOW_FILLERMARKED).fillermarked_filter(), TriState::DISABLED);
        assert_eq!(entry(Kind::Anime, item::SHOW_FILLERMARKED).fillermarked_filter(), TriState::ENABLED_IS);
        assert_eq!(entry(Kind::Anime, item::SHOW_NOT_FILLERMARKED).fillermarked_filter(), TriState::ENABLED_NOT);
    }

    #[test]
    fn expected_next_update_is_none_only_when_completed_for_both_kinds() {
        for kind in [Kind::Manga, Kind::Anime] {
            let mut e = Entry { next_update: 6, ..Entry::create(kind) };
            e.status = STATUS_COMPLETED;
            assert_eq!(e.expected_next_update(), None);
            for status in [0, 1, 3, 4, 5, 6] {
                e.status = status;
                assert_eq!(e.expected_next_update(), Some(6), "{kind} status {status}");
            }
        }
    }

    #[test]
    fn one_cover_shape_serves_both_kinds() {
        for kind in [Kind::Manga, Kind::Anime] {
            let e = Entry { id: 7, source: 3, favorite: true, thumbnail_url: Some("u".into()), cover_last_modified: 9, ..Entry::create(kind) };
            assert_eq!(e.as_cover(), Cover { entry_id: 7, source_id: 3, favorite: true, url: Some("u".into()), last_modified: 9 });
        }
    }

    #[test]
    fn apply_of_to_update_restores_every_covered_field_for_both_kinds() {
        for kind in [Kind::Manga, Kind::Anime] {
            let original = Entry {
                id: 11,
                source: 22,
                url: "/u".into(),
                title: "T".into(),
                artist: Some("A".into()),
                genre: Some(vec!["g".into()]),
                status: 1,
                favorite: true,
                item_flags: 0x101B42 & if kind.is_anime() { !0 } else { item::MANGA_MASK },
                version: 13,
                ..Entry::create(kind)
            };
            let mut changed = Entry { source: 0, url: "/x".into(), title: "X".into(), artist: None, genre: None, status: 5, favorite: false, item_flags: 0, version: 0, ..original.clone() };
            changed.apply_update(&original.to_update());
            assert_eq!(changed, original, "{kind}");
        }
    }

    #[test]
    fn apply_leaves_none_fields_and_never_clears_text() {
        let mut e = Entry { artist: Some("A".into()), item_flags: 0x1B2, ..Entry::create(Kind::Manga) };
        let mut u = EntryUpdate::new(e.id);
        u.title = Some("New".into());
        e.apply_update(&u);
        assert_eq!((e.title.as_str(), e.artist.as_deref(), e.item_flags), ("New", Some("A"), 0x1B2));
    }

    #[test]
    fn the_viewer_bits_round_trip_and_truncate() {
        let mut e = Entry::create(Kind::Anime);
        e.set_skip_intro_length(300);
        assert_eq!(e.viewer().skip_intro_length(), 44, "EC-21");
        e.set_skip_intro_length(0);
        assert!(e.viewer().skip_intro_disable());
        e.set_skip_intro_length(30);
        assert!(!e.viewer().skip_intro_disable());
        e.set_next_episode_airing(70_000, 1 << 33);
        assert_eq!(e.viewer().next_episode_to_air(), 70_000 & 0xFFFF, "EC-22");
        assert_eq!(e.viewer().next_episode_airing_at(), 0);
        e.set_next_episode_airing(12, 1_700_000_000);
        assert_eq!((e.viewer().next_episode_to_air(), e.viewer().next_episode_airing_at()), (12, 1_700_000_000));
        assert_eq!(e.viewer().skip_intro_length(), 30, "the intro group is untouched");
    }

    #[test]
    fn season_filters_and_grid_read_from_the_word() {
        let s = SeasonFlags(0x5970000);
        assert_eq!(s.grid_mode(), SeasonDisplayMode::List);
        assert_eq!(s.grid_size(), 5);
        assert!(s.unseen_overlay() && s.local_overlay() && s.continue_overlay());
        assert!(!s.downloaded_overlay() && !s.lang_overlay());
        assert!(!s.filtered(false));
        assert!(s.filtered(true), "downloaded-only counts as an enabled filter");
        assert!(SeasonFlags(season::SHOW_UNSEEN).filtered(false));
    }
}
