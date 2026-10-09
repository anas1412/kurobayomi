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

/// `mangas.chapter_flags`.
pub(crate) const MANGA_CHAPTER_READ: &str = "SELECT chapter_flags FROM mangas WHERE _id = ?1";
pub(crate) const MANGA_CHAPTER_WRITE: &str = "UPDATE mangas SET chapter_flags = ?1 WHERE _id = ?2";
pub(crate) const MANGA_CHAPTER_MASKED: &str =
    "UPDATE mangas SET chapter_flags = (chapter_flags & ~?2) | (?1 & ?2) WHERE _id = ?3";
/// `animes.episode_flags`.
pub(crate) const ANIME_EPISODE_READ: &str = "SELECT episode_flags FROM animes WHERE _id = ?1";
pub(crate) const ANIME_EPISODE_WRITE: &str = "UPDATE animes SET episode_flags = ?1 WHERE _id = ?2";
pub(crate) const ANIME_EPISODE_MASKED: &str =
    "UPDATE animes SET episode_flags = (episode_flags & ~?2) | (?1 & ?2) WHERE _id = ?3";
/// `animes.season_flags`.
pub(crate) const ANIME_SEASON_READ: &str = "SELECT season_flags FROM animes WHERE _id = ?1";
pub(crate) const ANIME_SEASON_WRITE: &str = "UPDATE animes SET season_flags = ?1 WHERE _id = ?2";
pub(crate) const ANIME_SEASON_MASKED: &str =
    "UPDATE animes SET season_flags = (season_flags & ~?2) | (?1 & ?2) WHERE _id = ?3";
