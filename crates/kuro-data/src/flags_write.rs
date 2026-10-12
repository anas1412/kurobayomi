//! Single-statement writes of one flag word of one row (T-07a-024 to T-07a-026). A masked write touches only the
//! bits of its mask and runs as one UPDATE, so two setters on different groups cannot overwrite each other. A
//! read-modify-write (for the sort-or-flip state machine) reads and writes inside one writer transaction.

use kuro_db::handle::Db;

use crate::DataResult;

/// Sets the bits of `mask` in the flag column to those of `flag` and keeps every other bit. `write` must be an
/// UPDATE with `?1 = flag`, `?2 = mask`, `?3 = id`. Returns whether a row changed.
pub(crate) fn masked(db: &Db, write: &'static str, id: i64, flag: i64, mask: i64) -> DataResult<bool> {
    db.write(|tx| Ok(tx.execute(write, (flag, mask, id))? > 0))
}

/// Reads one flag word, applies `change` and stores the result, all inside one writer transaction. A missing row
/// returns `Ok(false)`.
pub(crate) fn read_modify_write(
    db: &Db,
    read: &'static str,
    write: &'static str,
    id: i64,
    change: impl FnOnce(i64) -> i64,
) -> DataResult<bool> {
    db.write(|tx| {
        let current: i64 = match tx.query_row(read, [id], |r| r.get(0)) {
            Ok(value) => value,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        Ok(tx.execute(write, (change(current), id))? > 0)
    })
}

/// Manga chapter flags: `entries.item_flags` holds the anime superset layout for both kinds, while Rust keeps the
/// manga layout, so manga reads and writes translate. `tr(x)` moves the sorting bits 0x300 up to 0x600 and the read
/// expression is its inverse (`manga_item_flags_to_unified` / `_from_unified` in `kuro_db::flags`).
pub(crate) const MANGA_CHAPTER_READ: &str = "SELECT (item_flags & ~1536) | ((item_flags & 1536) >> 1) FROM entries \
     WHERE _id = ?1 AND kind = 'manga'";
pub(crate) const MANGA_CHAPTER_WRITE: &str =
    "UPDATE entries SET item_flags = (?1 & ~768) | ((?1 & 768) << 1) WHERE _id = ?2 AND kind = 'manga'";
pub(crate) const MANGA_CHAPTER_MASKED: &str = "UPDATE entries SET item_flags = \
     (item_flags & ~((?2 & ~768) | ((?2 & 768) << 1))) \
     | (((?1 & ~768) | ((?1 & 768) << 1)) & ((?2 & ~768) | ((?2 & 768) << 1))) \
     WHERE _id = ?3 AND kind = 'manga'";
/// Anime `item_flags` (the old `animes.episode_flags`).
pub(crate) const ANIME_EPISODE_READ: &str = "SELECT item_flags FROM entries WHERE _id = ?1 AND kind = 'anime'";
pub(crate) const ANIME_EPISODE_WRITE: &str = "UPDATE entries SET item_flags = ?1 WHERE _id = ?2 AND kind = 'anime'";
pub(crate) const ANIME_EPISODE_MASKED: &str =
    "UPDATE entries SET item_flags = (item_flags & ~?2) | (?1 & ?2) WHERE _id = ?3 AND kind = 'anime'";
/// `entries.season_flags`.
pub(crate) const ANIME_SEASON_READ: &str = "SELECT season_flags FROM entries WHERE _id = ?1 AND kind = 'anime'";
pub(crate) const ANIME_SEASON_WRITE: &str = "UPDATE entries SET season_flags = ?1 WHERE _id = ?2 AND kind = 'anime'";
pub(crate) const ANIME_SEASON_MASKED: &str =
    "UPDATE entries SET season_flags = (season_flags & ~?2) | (?1 & ?2) WHERE _id = ?3 AND kind = 'anime'";
