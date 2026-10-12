//! Manga entry behaviour: `chapter_flags` accessors, the expected-update rule, cover descriptors and
//! the `to_update` / `apply_update` pair (`Manga.kt`, `MangaUpdate.kt`).
//!
//! The struct lives in `kuro-data`, so the behaviour is an extension trait implemented here.

use kuro_data::entry::{Entry, EntryUpdate};
use kuro_data::manga::{Manga, MangaUpdate};
use kuro_db::codecs::UpdateStrategy;
use kuro_db::flags::{manga_item_flags_from_unified as from_unified, manga_item_flags_to_unified as to_unified};

pub use kuro_db::flags::chapter as chapter_flags;

use super::cover::MangaCover;
use super::flags::{Cover, EntryFlags, ItemFlags, expected_next_update};
pub use super::flags::STATUS_COMPLETED;
use crate::flags::TriState;

/// What a source reports for a manga, in domain types. The app layer converts the source model into this, so this
/// crate does not depend on `kuro-source`.
#[derive(Debug, Clone, PartialEq)]
pub struct SourceMetadata {
    pub author: Option<String>,
    pub artist: Option<String>,
    pub description: Option<String>,
    /// `None` when the source sent no genre text. `Some(None)` when it sent text with no genre in it (a blank string),
    /// which clears the stored genres on the metadata path (EC-MR-02).
    pub genre: Option<Option<Vec<String>>>,
    pub thumbnail_url: Option<String>,
    pub status: i64,
    pub update_strategy: UpdateStrategy,
    pub initialized: bool,
}

/// Accessors and update helpers over a `Manga` row.
pub trait MangaExt {
    /// `chapter_flags & SORTING_MASK`.
    fn sorting(&self) -> i64;
    /// `chapter_flags & DISPLAY_MASK`.
    fn display_mode(&self) -> i64;
    /// `chapter_flags & UNREAD_MASK`.
    fn unread_filter_raw(&self) -> i64;
    /// Unread filter as a tri-state: `SHOW_UNREAD` is `EnabledIs`, `SHOW_READ` is `EnabledNot`.
    fn unread_filter(&self) -> TriState;
    /// `chapter_flags & DOWNLOADED_MASK`. The app layer adds the downloaded-only override.
    fn downloaded_filter_raw(&self) -> i64;
    /// `chapter_flags & BOOKMARKED_MASK`.
    fn bookmarked_filter_raw(&self) -> i64;
    /// Bookmarked filter: `SHOW_BOOKMARKED` is `EnabledIs`, `SHOW_NOT_BOOKMARKED` is `EnabledNot`.
    fn bookmarked_filter(&self) -> TriState;
    /// `(chapter_flags & SORT_DIR_MASK) == SORT_DESC`.
    fn sort_descending(&self) -> bool;
    /// `None` only for a completed manga; otherwise the stored `next_update`.
    fn expected_next_update(&self) -> Option<i64>;
    /// `Manga.asMangaCover()`.
    fn as_cover(&self) -> MangaCover;
    /// `Manga.toMangaUpdate()`: all 20 optional fields filled from the entry.
    fn to_update(&self) -> MangaUpdate;
    /// Applies the `Some` fields of `u` to the entry. `None` leaves the field untouched.
    fn apply_update(&mut self, u: &MangaUpdate);
    /// The metadata path (`Manga.copyFrom`). Text the source sent replaces the stored text and `None` keeps it. A sent
    /// genre replaces the stored genres, and a blank one clears them. Status and update strategy always come from the
    /// source. `initialized` is the AND of both values.
    fn copy_from(&mut self, remote: &SourceMetadata);
}

/// The unified word of a manga: the accessors below are the shared [`ItemFlags`] ones, read through the translation.
fn words(m: &Manga) -> ItemFlags {
    ItemFlags(to_unified(m.chapter_flags))
}

impl MangaExt for Manga {
    fn sorting(&self) -> i64 {
        from_unified(words(self).sorting())
    }

    fn display_mode(&self) -> i64 {
        words(self).display_mode()
    }

    fn unread_filter_raw(&self) -> i64 {
        words(self).undone_filter_raw()
    }

    fn unread_filter(&self) -> TriState {
        words(self).undone_filter()
    }

    fn downloaded_filter_raw(&self) -> i64 {
        words(self).downloaded_filter_raw()
    }

    fn bookmarked_filter_raw(&self) -> i64 {
        words(self).bookmarked_filter_raw()
    }

    fn bookmarked_filter(&self) -> TriState {
        words(self).bookmarked_filter()
    }

    fn sort_descending(&self) -> bool {
        words(self).sort_descending()
    }

    fn expected_next_update(&self) -> Option<i64> {
        expected_next_update(self.status, self.next_update)
    }

    fn as_cover(&self) -> MangaCover {
        Cover { entry_id: self.id, source_id: self.source, favorite: self.favorite, url: self.thumbnail_url.clone(), last_modified: self.cover_last_modified }.into()
    }

    fn to_update(&self) -> MangaUpdate {
        MangaUpdate {
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
            favorite: Some(self.favorite),
            last_update: Some(self.last_update),
            next_update: Some(self.next_update),
            fetch_interval: Some(self.fetch_interval),
            date_added: Some(self.date_added),
            viewer_flags: Some(self.viewer_flags),
            chapter_flags: Some(self.chapter_flags),
            cover_last_modified: Some(self.cover_last_modified),
            update_strategy: Some(self.update_strategy),
            initialized: Some(self.initialized),
            version: Some(self.version),
        }
    }

    fn apply_update(&mut self, u: &MangaUpdate) {
        let mut entry = Entry::from(&*self);
        entry.apply_update(&EntryUpdate::from(u));
        *self = Manga::from(entry);
    }

    fn copy_from(&mut self, remote: &SourceMetadata) {
        if let Some(v) = &remote.author {
            self.author = Some(v.clone());
        }
        if let Some(v) = &remote.artist {
            self.artist = Some(v.clone());
        }
        if let Some(v) = &remote.description {
            self.description = Some(v.clone());
        }
        if let Some(genres) = &remote.genre {
            self.genre = genres.clone();
        }
        if let Some(v) = &remote.thumbnail_url {
            self.thumbnail_url = Some(v.clone());
        }
        self.status = remote.status;
        self.update_strategy = remote.update_strategy;
        self.initialized = remote.initialized && self.initialized;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kuro_db::codecs::UpdateStrategy;
    use kuro_db::flags::set_flag;

    fn with_flags(flags: i64) -> Manga {
        Manga {
            chapter_flags: flags,
            ..Manga::create()
        }
    }

    fn sample() -> Manga {
        Manga {
            id: 11,
            source: 22,
            url: "/series/1".into(),
            title: "Title".into(),
            artist: Some("A".into()),
            author: Some("B".into()),
            description: Some("D".into()),
            genre: Some(vec!["g1".into(), "g2".into()]),
            status: 1,
            thumbnail_url: Some("https://img/1.jpg".into()),
            favorite: true,
            last_update: 5,
            next_update: 6,
            fetch_interval: -3,
            date_added: 7,
            viewer_flags: 8,
            chapter_flags: 0x1B2,
            cover_last_modified: 9,
            update_strategy: UpdateStrategy::OnlyFetchOnce,
            initialized: true,
            last_modified_at: 10,
            favorite_modified_at: Some(12),
            version: 13,
        }
    }

    #[test]
    fn create_matches_the_documented_defaults() {
        let m = Manga::create();
        assert_eq!((m.id, m.source, m.status, m.favorite), (-1, -1, 0, false));
        assert_eq!((m.last_update, m.next_update, m.fetch_interval), (0, 0, 0));
        assert_eq!(m.update_strategy, UpdateStrategy::AlwaysUpdate);
    }

    #[test]
    fn unread_filter_named_vectors() {
        assert_eq!(with_flags(0x0).unread_filter(), TriState::DISABLED);
        assert_eq!(with_flags(0x2).unread_filter(), TriState::ENABLED_IS);
        assert_eq!(with_flags(0x4).unread_filter(), TriState::ENABLED_NOT);
        assert_eq!(
            with_flags(0x6).unread_filter(),
            TriState::DISABLED,
            "invalid group value"
        );
    }

    #[test]
    fn downloaded_and_bookmarked_named_vectors() {
        assert_eq!(with_flags(0x18).downloaded_filter_raw(), 0x18);
        assert_eq!(with_flags(0x8).downloaded_filter_raw(), 0x8);
        assert_eq!(with_flags(0x60).bookmarked_filter(), TriState::DISABLED);
        assert_eq!(with_flags(0x20).bookmarked_filter(), TriState::ENABLED_IS);
        assert_eq!(with_flags(0x40).bookmarked_filter(), TriState::ENABLED_NOT);
    }

    #[test]
    fn sorting_display_and_direction_named_vectors() {
        assert_eq!(with_flags(0x300).sorting(), 0x300);
        assert_eq!(with_flags(0x100).sorting(), 0x100);
        assert_eq!(with_flags(0x100000).display_mode(), 0x100000);
        assert!(with_flags(0x0).sort_descending());
        assert!(!with_flags(0x1).sort_descending());
    }

    #[test]
    fn accessors_agree_with_the_masks_over_30_words() {
        let words: [i64; 30] = [
            0x0,
            0x1,
            0x2,
            0x3,
            0x4,
            0x5,
            0x6,
            0x7,
            0x8,
            0x10,
            0x18,
            0x20,
            0x40,
            0x60,
            0x7F,
            0x100,
            0x200,
            0x300,
            0x3FF,
            0x100000,
            0x100001,
            0x100006,
            0x100018,
            0x100060,
            0x100300,
            0x1FFFFF,
            0x2,
            0x40,
            0x100000 | 0x6,
            0x300 | 0x40,
        ];
        for w in words {
            let m = with_flags(w);
            assert_eq!(m.sorting(), w & 0x300, "sorting {w:#x}");
            assert_eq!(m.display_mode(), w & 0x100000, "display {w:#x}");
            assert_eq!(m.unread_filter_raw(), w & 0x6, "unread {w:#x}");
            assert_eq!(m.bookmarked_filter_raw(), w & 0x60, "bookmarked {w:#x}");
            assert_eq!(m.sort_descending(), w & 0x1 == 0, "direction {w:#x}");
            let expected_unread = match w & 0x6 {
                0x2 => TriState::ENABLED_IS,
                0x4 => TriState::ENABLED_NOT,
                _ => TriState::DISABLED,
            };
            assert_eq!(m.unread_filter(), expected_unread, "unread filter {w:#x}");
        }
    }

    #[test]
    fn expected_next_update_is_none_only_when_completed() {
        let mut m = sample();
        m.status = STATUS_COMPLETED;
        assert_eq!(m.expected_next_update(), None);
        for status in [0, 1, 3, 4, 5, 6] {
            m.status = status;
            assert_eq!(m.expected_next_update(), Some(6), "status {status}");
        }
    }

    #[test]
    fn round_trip_each_group_leaves_other_bits_unchanged() {
        let groups: [(i64, &[i64]); 4] = [
            (
                chapter_flags::SORT_DIR_MASK,
                &[chapter_flags::SORT_DESC, chapter_flags::SORT_ASC],
            ),
            (
                chapter_flags::UNREAD_MASK,
                &[
                    chapter_flags::SHOW_ALL,
                    chapter_flags::SHOW_UNREAD,
                    chapter_flags::SHOW_READ,
                ],
            ),
            (
                chapter_flags::SORTING_MASK,
                &[
                    chapter_flags::SORTING_SOURCE,
                    chapter_flags::SORTING_NUMBER,
                    chapter_flags::SORTING_UPLOAD_DATE,
                    chapter_flags::SORTING_ALPHABET,
                ],
            ),
            (
                chapter_flags::DISPLAY_MASK,
                &[chapter_flags::DISPLAY_NAME, chapter_flags::DISPLAY_NUMBER],
            ),
        ];
        let background = 0x7FFF_FFFF_FFF0_0000_i64 | 0x0A;
        for (mask, values) in groups {
            for &v in values {
                let before = background & !mask;
                let word = set_flag(background, v, mask);
                assert_eq!(
                    with_flags(word).chapter_flags & mask,
                    v,
                    "group {mask:#x} value {v:#x}"
                );
                assert_eq!(
                    word & !mask,
                    before,
                    "other bits changed for group {mask:#x}"
                );
            }
        }
    }

    #[test]
    fn to_update_fills_all_twenty_fields() {
        let u = sample().to_update();
        assert_eq!(u.id, 11);
        let filled = [
            u.source.is_some(),
            u.url.is_some(),
            u.title.is_some(),
            u.artist.is_some(),
            u.author.is_some(),
            u.description.is_some(),
            u.genre.is_some(),
            u.status.is_some(),
            u.thumbnail_url.is_some(),
            u.favorite.is_some(),
            u.last_update.is_some(),
            u.next_update.is_some(),
            u.fetch_interval.is_some(),
            u.date_added.is_some(),
            u.viewer_flags.is_some(),
            u.chapter_flags.is_some(),
            u.cover_last_modified.is_some(),
            u.update_strategy.is_some(),
            u.initialized.is_some(),
            u.version.is_some(),
        ];
        assert_eq!(filled.iter().filter(|b| **b).count(), 20);
    }

    #[test]
    fn apply_of_to_update_restores_every_covered_field() {
        let original = sample();
        let mut changed = original.clone();
        changed.source = 0;
        changed.url = "/other".into();
        changed.title = "Other".into();
        changed.artist = None;
        changed.genre = None;
        changed.status = 5;
        changed.favorite = false;
        changed.chapter_flags = 0;
        changed.version = 0;
        changed.apply_update(&original.to_update());
        assert_eq!(changed, original);
    }

    #[test]
    fn apply_leaves_none_fields_and_never_clears_text() {
        let mut m = sample();
        let mut u = MangaUpdate::new(11);
        u.title = Some("New".into());
        m.apply_update(&u);
        assert_eq!(m.title, "New");
        assert_eq!(
            m.artist.as_deref(),
            Some("A"),
            "None artist keeps the stored text"
        );
        assert_eq!(m.chapter_flags, 0x1B2);
    }

    #[test]
    fn manga_update_default_has_every_optional_field_none() {
        let d = MangaUpdate::default();
        assert_eq!(d.id, 0);
        assert!(
            d.source.is_none() && d.title.is_none() && d.genre.is_none() && d.version.is_none()
        );
        assert_eq!(MangaUpdate::new(3).id, 3);
    }

    /// The stored row of the metadata vector (`p1-topic-models-and-sync.md` section 11, vector 1).
    fn stored() -> Manga {
        Manga {
            author: Some("A".into()),
            artist: Some("B".into()),
            description: Some("D".into()),
            genre: Some(vec!["x".into()]),
            thumbnail_url: Some("t".into()),
            initialized: true,
            ..Manga::create()
        }
    }

    /// The source report of the same vector: no author, a blank artist, no description, a blank genre, no thumbnail.
    fn remote() -> SourceMetadata {
        SourceMetadata {
            author: None,
            artist: Some(String::new()),
            description: None,
            genre: Some(None),
            thumbnail_url: None,
            status: 2,
            update_strategy: UpdateStrategy::OnlyFetchOnce,
            initialized: false,
        }
    }

    #[test]
    fn copy_from_matches_the_metadata_vector() {
        let mut m = stored();
        m.copy_from(&remote());
        assert_eq!(m.author.as_deref(), Some("A"), "EC-MR-01: a None author keeps the stored one");
        assert_eq!(m.artist.as_deref(), Some(""), "a sent blank artist is written");
        assert_eq!(m.description.as_deref(), Some("D"));
        assert_eq!(m.genre, None, "EC-MR-02: a blank genre clears the stored genres");
        assert_eq!(m.thumbnail_url.as_deref(), Some("t"));
        assert_eq!((m.status, m.update_strategy), (2, UpdateStrategy::OnlyFetchOnce));
        assert!(!m.initialized, "EC-MR-05: true && false is false");
    }

    #[test]
    fn copy_from_keeps_the_stored_genres_when_the_source_sent_no_genre_text() {
        let mut m = stored();
        m.copy_from(&SourceMetadata { genre: None, ..remote() });
        assert_eq!(m.genre, Some(vec!["x".to_owned()]));
    }

    #[test]
    fn copy_from_keeps_initialized_true_only_when_both_sides_are_true() {
        let mut m = stored();
        m.copy_from(&SourceMetadata { initialized: true, ..remote() });
        assert!(m.initialized, "true && true is true");
    }

    #[test]
    fn refresh_coalesce_keeps_the_genres_and_writes_an_empty_artist() {
        let mut m = stored();
        let meta = remote();
        let update = MangaUpdate {
            genre: meta.genre.clone().flatten(),
            artist: meta.artist.clone(),
            ..MangaUpdate::new(m.id)
        };
        m.apply_update(&update);
        assert_eq!(m.genre, Some(vec!["x".to_owned()]), "EC-MR-03: a blank genre is None on refresh, so it keeps");
        assert_eq!(m.artist.as_deref(), Some(""), "EC-MR-04: an empty artist is written");
    }
}
