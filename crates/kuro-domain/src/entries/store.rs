//! The entry-repository operations the shared flag code needs, over the shared `EntryRepo` and, until phase E deletes
//! them, the two legacy per-kind repositories. One trait with three small impls lets every flag setter and default
//! writer be ONE function that both the new and the old callers use; the legacy impls translate the manga flag word
//! (`kuro_db::flags::manga_item_flags_*`) so the shared code only ever sees the unified layout.

use kuro_data::DataResult;
use kuro_data::anime::AnimeRepo;
use kuro_data::entry::EntryRepo;
use kuro_data::manga::MangaRepo;
use kuro_db::flags::{manga_item_flags_from_unified as from_unified, manga_item_flags_to_unified as to_unified};
use kuro_platform::kind::Kind;

/// Masked writes of the three flag words of an entry. A missing entry gives `Ok(false)`.
pub trait EntryFlagStore {
    /// The kind of the entry, or `None` when it does not exist.
    fn kind_of(&self, id: i64) -> DataResult<Option<Kind>>;
    /// `item_flags = (item_flags & !mask) | (flag & mask)` in one statement (unified layout).
    fn set_item_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool>;
    /// Reads the item word, applies `change` and stores the result in the writer transaction (unified layout).
    fn update_item_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool>;
    fn set_season_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool>;
    fn update_season_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool>;
    /// `viewer = (viewer & !mask) | (value & mask)` in one statement.
    fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool>;
}

fn kind_or_none(found: DataResult<Kind>) -> DataResult<Option<Kind>> {
    match found {
        Ok(kind) => Ok(Some(kind)),
        Err(kuro_data::DataError::NotFound) => Ok(None),
        Err(e) => Err(e),
    }
}

impl EntryFlagStore for EntryRepo {
    fn kind_of(&self, id: i64) -> DataResult<Option<Kind>> {
        kind_or_none(self.by_id(id).map(|e| e.kind))
    }
    fn set_item_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        EntryRepo::set_item_flags_masked(self, id, flag, mask)
    }
    fn update_item_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        EntryRepo::update_item_flags(self, id, change)
    }
    fn set_season_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        EntryRepo::set_season_flags_masked(self, id, flag, mask)
    }
    fn update_season_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        EntryRepo::update_season_flags(self, id, change)
    }
    fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool> {
        EntryRepo::set_viewer_bits(self, id, value, mask)
    }
}

impl EntryFlagStore for MangaRepo {
    fn kind_of(&self, id: i64) -> DataResult<Option<Kind>> {
        kind_or_none(self.by_id(id).map(|_| Kind::Manga))
    }
    fn set_item_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.set_chapter_flags_masked(id, from_unified(flag), from_unified(mask))
    }
    fn update_item_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        self.update_chapter_flags(id, |word| from_unified(change(to_unified(word))))
    }
    /// A manga has no seasons.
    fn set_season_flags_masked(&self, _id: i64, _flag: i64, _mask: i64) -> DataResult<bool> {
        Ok(false)
    }
    fn update_season_flags(&self, _id: i64, _change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        Ok(false)
    }
    fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool> {
        MangaRepo::set_viewer_bits(self, id, value, mask)
    }
}

impl EntryFlagStore for AnimeRepo {
    fn kind_of(&self, id: i64) -> DataResult<Option<Kind>> {
        kind_or_none(self.by_id(id).map(|_| Kind::Anime))
    }
    fn set_item_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        self.set_episode_flags_masked(id, flag, mask)
    }
    fn update_item_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        self.update_episode_flags(id, change)
    }
    fn set_season_flags_masked(&self, id: i64, flag: i64, mask: i64) -> DataResult<bool> {
        AnimeRepo::set_season_flags_masked(self, id, flag, mask)
    }
    fn update_season_flags(&self, id: i64, change: impl FnOnce(i64) -> i64) -> DataResult<bool> {
        AnimeRepo::update_season_flags(self, id, change)
    }
    fn set_viewer_bits(&self, id: i64, value: i64, mask: i64) -> DataResult<bool> {
        AnimeRepo::set_viewer_bits(self, id, value, mask)
    }
}
