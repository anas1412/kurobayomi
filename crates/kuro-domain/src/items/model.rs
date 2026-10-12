//! Chapter and episode behaviour (`Chapter.kt`, `Episode.kt`): recognised-number test, `copy_from` for
//! source refreshes, and the full `to_update` snapshot.

use kuro_data::chapter::{Chapter, ChapterUpdate};
use kuro_data::episode::{Episode, EpisodeUpdate};
use kuro_data::item::{Item, ItemUpdate};
use kuro_platform::kind::Kind;
use kuro_source::model::normalize_scanlator;

/// `s` as `None` when it is empty or only whitespace (`ifBlank { null }`).
pub(crate) fn blank_to_none(s: Option<&str>) -> Option<String> {
    s.filter(|v| !v.trim().is_empty()).map(str::to_owned)
}

/// A source's chapter or episode number, parsed as `f32` and widened to `f64` through its shortest decimal text, so
/// `1.1f32` is stored as `1.1` and not `1.100000023841858`. This deliberately corrects Kotlin's `Float.toDouble()`
/// (EC-CH-07, T-15a-004, T-06a-034). The one widening rule of the app: `kuro_source::model::domain_chapter_number`.
pub fn widen_source_number(n: f32) -> f64 {
    kuro_source::model::domain_chapter_number(n)
}

/// Identity of a chapter or episode: its row id and url, as `ChapterImpl.equals` (T-06a-034). One definition, in kuro-data.
pub use kuro_data::chapter::ChapterKey;
pub use kuro_data::episode::EpisodeKey;

/// The scanlator of a row: chapters are trimmed (`copyFromSChapter`), episodes only blanked (`copyFromSEpisode`). The one
/// per-kind scanlator rule; the two source adapters and the copy of a refresh call it.
pub fn scanlator_of(kind: Kind, raw: Option<&str>) -> Option<String> {
    match kind {
        Kind::Manga => normalize_scanlator(raw),
        Kind::Anime => blank_to_none(raw),
    }
}

/// Behaviour of an item row of either kind (`Chapter.kt`, `Episode.kt`).
pub trait ItemExt {
    /// `number >= 0`. `-1` (unknown) and `-2` (numberless sentinel) are not recognised.
    fn is_recognized_number(&self) -> bool;
    /// Copies the source's name, url, upload date, number and scanlator (per the kind's scanlator rule), and for an
    /// anime item also the fillermark, summary and preview url (blank becomes `None`). Every other field of `self` is kept.
    fn copy_from(&self, other: &Item) -> Item;
    /// `toChapterUpdate` / `toEpisodeUpdate`: every field filled.
    fn to_update(&self) -> ItemUpdate;
}

impl ItemExt for Item {
    fn is_recognized_number(&self) -> bool {
        self.number >= 0.0
    }

    fn copy_from(&self, other: &Item) -> Item {
        let anime = self.kind.is_anime();
        Item {
            name: other.name.clone(),
            url: other.url.clone(),
            date_upload: other.date_upload,
            number: other.number,
            scanlator: scanlator_of(self.kind, other.scanlator.as_deref()),
            fillermark: if anime { other.fillermark } else { self.fillermark },
            summary: if anime { blank_to_none(other.summary.as_deref()) } else { self.summary.clone() },
            preview_url: if anime { blank_to_none(other.preview_url.as_deref()) } else { self.preview_url.clone() },
            ..self.clone()
        }
    }

    fn to_update(&self) -> ItemUpdate {
        let anime = self.kind.is_anime();
        ItemUpdate {
            id: self.id,
            entry_id: Some(self.entry_id),
            done: Some(self.done),
            bookmark: Some(self.bookmark),
            fillermark: anime.then_some(self.fillermark),
            progress: Some(self.progress),
            total_seconds: self.total_seconds,
            date_fetch: Some(self.date_fetch),
            source_order: Some(self.source_order),
            url: Some(self.url.clone()),
            name: Some(self.name.clone()),
            date_upload: Some(self.date_upload),
            number: Some(self.number),
            scanlator: self.scanlator.clone(),
            summary: self.summary.clone(),
            preview_url: self.preview_url.clone(),
            version: Some(self.version),
            memo: self.memo.clone(),
        }
    }
}

/// Behaviour of a chapter row.
pub trait ChapterExt {
    /// `chapterNumber >= 0`. `-1` (unknown) and `-2` (numberless sentinel) are not recognised.
    fn is_recognized_number(&self) -> bool;
    /// Copies the source's name, url, upload date and number, and its scanlator (blank becomes `None`, otherwise trimmed).
    /// Every other field of `self` is kept.
    fn copy_from(&self, other: &Chapter) -> Chapter;
    /// `Chapter.toChapterUpdate()`: all 12 optional fields filled.
    fn to_update(&self) -> ChapterUpdate;
}

impl ChapterExt for Chapter {
    fn is_recognized_number(&self) -> bool {
        Item::from(self).is_recognized_number()
    }

    fn copy_from(&self, other: &Chapter) -> Chapter {
        Chapter::from(Item::from(self).copy_from(&Item::from(other)))
    }

    fn to_update(&self) -> ChapterUpdate {
        ChapterUpdate {
            id: self.id,
            manga_id: Some(self.manga_id),
            read: Some(self.read),
            bookmark: Some(self.bookmark),
            last_page_read: Some(self.last_page_read),
            date_fetch: Some(self.date_fetch),
            source_order: Some(self.source_order),
            url: Some(self.url.clone()),
            name: Some(self.name.clone()),
            date_upload: Some(self.date_upload),
            chapter_number: Some(self.chapter_number),
            scanlator: self.scanlator.clone(),
            version: Some(self.version),
        }
    }
}

/// Behaviour of an episode row.
pub trait EpisodeExt {
    /// `episodeNumber >= 0`.
    fn is_recognized_number(&self) -> bool;
    /// Copies the source's name, url, upload date, number and fillermark, and the scanlator, summary and
    /// preview url (blank becomes `None`). Every other field of `self` is kept.
    fn copy_from(&self, other: &Episode) -> Episode;
    /// `Episode.toEpisodeUpdate()`: all 17 optional fields filled.
    fn to_update(&self) -> EpisodeUpdate;
}

impl EpisodeExt for Episode {
    fn is_recognized_number(&self) -> bool {
        Item::from(self).is_recognized_number()
    }

    fn copy_from(&self, other: &Episode) -> Episode {
        Episode::from(Item::from(self).copy_from(&Item::from(other)))
    }

    fn to_update(&self) -> EpisodeUpdate {
        EpisodeUpdate {
            id: self.id,
            anime_id: Some(self.anime_id),
            seen: Some(self.seen),
            bookmark: Some(self.bookmark),
            fillermark: Some(self.fillermark),
            last_second_seen: Some(self.last_second_seen),
            total_seconds: Some(self.total_seconds),
            date_fetch: Some(self.date_fetch),
            source_order: Some(self.source_order),
            url: Some(self.url.clone()),
            name: Some(self.name.clone()),
            date_upload: Some(self.date_upload),
            episode_number: Some(self.episode_number),
            scanlator: self.scanlator.clone(),
            summary: self.summary.clone(),
            preview_url: self.preview_url.clone(),
            version: Some(self.version),
            memo: Some(self.memo.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_numbers_widen_through_their_decimal_form() {
        assert_eq!(widen_source_number(1.5), 1.5);
        assert_eq!(widen_source_number(1.1), 1.1);
        let parsed: f32 = "0.1".parse().unwrap();
        assert_eq!(widen_source_number(parsed), 0.1f64);
        assert_eq!(widen_source_number(-1.0), -1.0);
    }

    fn chapter() -> Chapter {
        Chapter {
            id: 4,
            manga_id: 2,
            read: true,
            bookmark: true,
            last_page_read: 9,
            date_fetch: 100,
            source_order: 3,
            url: "/c/4".into(),
            name: "Chapter 4".into(),
            date_upload: 1_700_000_000_000,
            chapter_number: 4.0,
            scanlator: Some("Team".into()),
            last_modified_at: 55,
            version: 6,
        }
    }

    fn episode() -> Episode {
        Episode {
            id: 8,
            anime_id: 2,
            seen: true,
            bookmark: false,
            fillermark: false,
            last_second_seen: 30,
            total_seconds: 1400,
            date_fetch: 100,
            source_order: 1,
            url: "/e/8".into(),
            name: "Ep 8".into(),
            date_upload: 42,
            episode_number: 8.0,
            scanlator: Some("Sub".into()),
            summary: Some("Summary".into()),
            preview_url: Some("https://p/8.jpg".into()),
            last_modified_at: 7,
            version: 2,
            memo: serde_json::Map::new(),
        }
    }

    #[test]
    fn create_defaults() {
        let c = Chapter::create();
        assert_eq!(
            (c.id, c.manga_id, c.date_upload, c.version),
            (-1, -1, -1, 1)
        );
        assert_eq!(c.chapter_number, -1.0);
        let e = Episode::create();
        assert_eq!(
            (e.id, e.anime_id, e.date_upload, e.version),
            (-1, -1, -1, 1)
        );
        assert_eq!(e.episode_number, -1.0);
        assert!(e.memo.is_empty());
    }

    #[test]
    fn recognised_number_boundaries() {
        let mut c = chapter();
        for (n, expected) in [
            (0.0, true),
            (-1.0, false),
            (-2.0, false),
            (-0.0001, false),
            (1.5, true),
        ] {
            c.chapter_number = n;
            assert_eq!(c.is_recognized_number(), expected, "chapter {n}");
            let mut e = episode();
            e.episode_number = n;
            assert_eq!(e.is_recognized_number(), expected, "episode {n}");
        }
    }

    #[test]
    fn chapter_copy_from_takes_source_fields_and_blank_scanlator_becomes_none() {
        let mine = chapter();
        let mut source = Chapter::create();
        source.name = "New name".into();
        source.url = "/new".into();
        source.date_upload = 99;
        source.chapter_number = 4.5;
        source.scanlator = Some("   ".into());
        let out = mine.copy_from(&source);
        source.scanlator = Some(" Team X ".into());
        assert_eq!(mine.copy_from(&source).scanlator.as_deref(), Some("Team X"), "T-15a-004: trimmed");
        assert_eq!(out.name, "New name");
        assert_eq!(out.url, "/new");
        assert_eq!(out.date_upload, 99);
        assert_eq!(out.chapter_number, 4.5);
        assert_eq!(out.scanlator, None);
        assert_eq!(
            (out.id, out.read, out.bookmark, out.last_page_read),
            (4, true, true, 9)
        );
        assert_eq!(
            (
                out.date_fetch,
                out.source_order,
                out.last_modified_at,
                out.version
            ),
            (100, 3, 55, 6)
        );
    }

    #[test]
    fn episode_copy_from_copies_fillermark_and_clears_blank_text() {
        let mine = episode();
        let mut source = Episode::create();
        source.name = "Renamed".into();
        source.fillermark = true;
        source.scanlator = Some("".into());
        source.summary = Some("  ".into());
        source.preview_url = Some("https://p/new.jpg".into());
        let out = mine.copy_from(&source);
        assert_eq!(out.name, "Renamed");
        assert!(out.fillermark);
        assert_eq!(out.scanlator, None);
        assert_eq!(out.summary, None);
        assert_eq!(out.preview_url.as_deref(), Some("https://p/new.jpg"));
        assert_eq!(
            (out.id, out.seen, out.last_second_seen, out.total_seconds),
            (8, true, 30, 1400)
        );
    }

    #[test]
    fn to_update_fills_all_optional_fields() {
        let u = chapter().to_update();
        assert_eq!((u.id, u.manga_id, u.version), (4, Some(2), Some(6)));
        assert_eq!(u.scanlator.as_deref(), Some("Team"));
        assert_eq!(u.chapter_number, Some(4.0));
        let e = episode().to_update();
        assert_eq!(e.summary.as_deref(), Some("Summary"));
        assert_eq!(e.memo, Some(serde_json::Map::new()));
        assert_eq!(
            (e.anime_id, e.seen, e.fillermark, e.version),
            (Some(2), Some(true), Some(false), Some(2))
        );
    }

    #[test]
    fn the_item_rules_serve_both_kinds() {
        let manga = Item { name: "mine".into(), done: true, progress: 9, scanlator: Some("old".into()), ..Item::create(Kind::Manga) };
        let source = Item { name: "new".into(), number: 4.5, scanlator: Some("  Team  ".into()), ..Item::create(Kind::Manga) };
        let out = manga.copy_from(&source);
        assert_eq!((out.name.as_str(), out.number, out.scanlator.as_deref(), out.done, out.progress), ("new", 4.5, Some("Team"), true, 9));
        let anime = Item { done: true, progress: 30, total_seconds: Some(1400), ..Item::create(Kind::Anime) };
        let from_source = Item { fillermark: true, scanlator: Some("  Sub ".into()), summary: Some(" ".into()), preview_url: Some("p".into()), ..Item::create(Kind::Anime) };
        let out = anime.copy_from(&from_source);
        assert_eq!((out.fillermark, out.scanlator.as_deref(), out.summary, out.preview_url.as_deref()), (true, Some("  Sub "), None, Some("p")));
        assert_eq!((out.done, out.progress, out.total_seconds), (true, 30, Some(1400)), "the user's state is kept");
        assert!(Item { number: 0.0, ..Item::create(Kind::Manga) }.is_recognized_number());
        assert!(!Item { number: -2.0, ..Item::create(Kind::Anime) }.is_recognized_number());
        let update = anime.to_update();
        assert_eq!((update.fillermark, update.total_seconds, update.memo.is_some()), (Some(false), Some(1400), true));
        assert_eq!(manga.to_update().fillermark, None, "a manga item has no fillermark column to write");
    }
}
