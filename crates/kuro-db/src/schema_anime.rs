//! Anime database schema: file and version constants, the open gate that creates a fresh file or
//! refuses one it must not touch, the fresh DDL, the column codecs and the bit flags of its tables.
//!
//! Sibling database contract (T-03a-035). The anime tables live only in `anime.db` (`ANIME_DB_FILE`):
//! `animes`, `episodes`, `animehistory`, `animes_categories`, `categories`, `anime_sync`,
//! `animesources`, `anime_relations`, `custom_buttons`, `extension_store`, and the eight views of
//! `views.sql`. The manga tables live only in `manga.db`. Both files have a `categories` table with
//! the same columns but separate rows. Anime SQL never names a manga table and manga SQL never names
//! an anime table. The two databases never join: no statement attaches one file to the other, no
//! transaction writes both, and a commit in one never notifies subscribers of the other.

use std::path::{Path, PathBuf};

use kuro_platform::paths::Paths;
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::Connection;

pub use crate::codec::{GenreList, UpdateStrategy};
use crate::error::DbError;
use crate::open::{create_baseline, open_and_migrate, DbResult, SchemaBundle};

/// File name of the anime database inside the data directory (`anime::consts`).
pub use crate::anime::consts::FILE_NAME as ANIME_DB_FILE;

/// `PRAGMA user_version` of the fresh anime schema (`anime::consts`).
pub use crate::anime::consts::SCHEMA_VERSION as ANIME_SCHEMA_VERSION;

/// `PRAGMA application_id` of anime files: ASCII "KUR1". Aniyomi files keep 0, and an Aniyomi file
/// at version 139 must not pass as ours.
pub const ANIME_APPLICATION_ID: i64 = 0x4B55_5231;

/// Table names of the anime file, in the order of the notification bits.
pub const ANIME_TABLES: &[&str] = &[
    "animes",
    "episodes",
    "animehistory",
    "animes_categories",
    "categories",
    "anime_sync",
    "animesources",
    "anime_relations",
    "custom_buttons",
    "extension_store",
];

/// Anime files from the Aniyomi chain: 113 to 138 need the legacy import, older ones are too old to
/// import, and 139 (the Aniyomi version of this schema) is an Aniyomi file to import.
fn refuse_foreign(found: i64) -> DbError {
    if found < 113 {
        DbError::TooOldForImport { found }
    } else if found <= 138 {
        DbError::NeedsLegacyImport { found }
    } else {
        DbError::LegacyFile { found }
    }
}

/// The anime file: the fresh schema and views at version 139. No migrations yet.
pub const ANIME_SCHEMA: SchemaBundle = SchemaBundle {
    baseline_sql: concat!(
        include_str!("../sql/anime/schema.sql"),
        include_str!("../sql/anime/views.sql"),
    ),
    baseline_version: ANIME_SCHEMA_VERSION,
    migrations: &[],
    application_id: ANIME_APPLICATION_ID,
    tables: ANIME_TABLES,
    refuse_foreign,
};

/// `<data_dir>/anime.db`.
pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join(ANIME_DB_FILE)
}

/// Opens `<data_dir>/anime.db` for the app's platform paths. See [`open`].
pub fn open_in_data_dir(paths: &dyn Paths) -> DbResult<Connection> {
    open(&db_path(&paths.data_dir()))
}

/// Opens the anime database at `path` with the connection pragmas, creating the fresh schema in an
/// empty file. A file this app must not touch (newer, or an Aniyomi file) is refused before any
/// write, so it stays byte-identical.
pub fn open(path: &Path) -> DbResult<Connection> {
    open_and_migrate(path, &ANIME_SCHEMA)
}

/// The fresh schema in a new in-memory database (tests and the test kit). WAL does not apply to
/// memory databases, so only `foreign_keys` is set.
pub fn open_in_memory() -> DbResult<Connection> {
    let mut c = Connection::open_in_memory()?;
    c.pragma_update(None, "foreign_keys", "ON")?;
    crate::open::register_functions(&c)?;
    create_baseline(&mut c, &ANIME_SCHEMA)?;
    Ok(c)
}

/// Golden text of the object inventory: one line per `sqlite_master` object, sorted by type and
/// name, then one line per column of every table and view in `PRAGMA table_info` order.
pub fn object_dump(c: &Connection) -> rusqlite::Result<String> {
    let objects: Vec<(String, String, String)> = c
        .prepare("SELECT type, name, tbl_name FROM sqlite_master ORDER BY type, name")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<rusqlite::Result<_>>()?;
    let mut out = String::new();
    for (kind, name, table) in objects {
        out.push_str(&format!("{kind} {name} on {table}\n"));
        if kind == "table" || kind == "view" {
            let mut st = c.prepare(
                "SELECT cid, name, type, \"notnull\", dflt_value, pk FROM pragma_table_info(?1)",
            )?;
            let columns = st
                .query_map([&name], |r| {
                    Ok(format!(
                        "  {}|{}|{}|{}|{}|{}\n",
                        r.get::<_, i64>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, i64>(3)?,
                        r.get::<_, Option<String>>(4)?.unwrap_or_else(|| "-".into()),
                        r.get::<_, i64>(5)?,
                    ))
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columns.iter().for_each(|line| out.push_str(line));
        }
    }
    Ok(out)
}

/// Epoch milliseconds (`AS Date` in SQLDelight): `animehistory.last_seen`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(pub i64);

impl FromSql for Millis {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self)
    }
}

impl ToSql for Millis {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        self.0.to_sql()
    }
}

/// `animes.fetch_type`: 0 Seasons, 1 Episodes. Any other value reads as Episodes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FetchType {
    Seasons = 0,
    #[default]
    Episodes = 1,
}

impl FetchType {
    pub fn from_db(v: i64) -> Self {
        if v == 0 { Self::Seasons } else { Self::Episodes }
    }
}

impl FromSql for FetchType {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        i64::column_result(value).map(Self::from_db)
    }
}

impl ToSql for FetchType {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok((*self as i64).into())
    }
}

/// `memo`: a JSON object stored as BLOB (or TEXT in the fresh default `'{}'`). A cell that is not a
/// JSON object reads as the empty object with a warning. `FromSql` has no table or row to report, so
/// the read is tolerant rather than an error (the decision of `p1-rust-design.md` section 5).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Memo(pub serde_json::Map<String, serde_json::Value>);

impl FromSql for Memo {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        let bytes: &[u8] = match value {
            ValueRef::Text(t) | ValueRef::Blob(t) => t,
            _ => &[],
        };
        match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(serde_json::Value::Object(map)) => Ok(Self(map)),
            _ => {
                tracing::warn!("memo is not a JSON object; reading it as {{}}");
                Ok(Self::default())
            }
        }
    }
}

impl Memo {
    /// Strict read of a `memo` cell: a TEXT or BLOB holding a JSON object, or `DbError::BadMemo` naming the
    /// table and row. The row readers are tolerant (`FromSql` above) so one corrupt cell never fails a list
    /// query; a caller that must know about the cell, such as a backup, uses this.
    pub fn decode(value: ValueRef<'_>, table: &'static str, id: i64) -> Result<Self, DbError> {
        let bytes = match value {
            ValueRef::Text(t) | ValueRef::Blob(t) => t,
            _ => return Err(DbError::BadMemo { table, id }),
        };
        match serde_json::from_slice::<serde_json::Value>(bytes) {
            Ok(serde_json::Value::Object(map)) => Ok(Self(map)),
            _ => Err(DbError::BadMemo { table, id }),
        }
    }
}

impl ToSql for Memo {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        let bytes = serde_json::to_vec(&self.0)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        Ok(ToSqlOutput::Owned(rusqlite::types::Value::Blob(bytes)))
    }
}

/// Bit layouts of `animes.episode_flags`, `animes.season_flags`, `animes.viewer` and
/// `categories.flags` of the anime database. Values are copied from Aniyomi `Anime.kt`,
/// `SeasonDisplayMode.kt` and `AnimeLibrarySortMode.kt`. The generic setter is
/// `crate::flags::set_flag`.
pub mod flags {
    pub const EPISODE_SORT_DESC: i64 = 0x0;
    pub const EPISODE_SORT_ASC: i64 = 0x1;
    pub const EPISODE_SORT_DIR_MASK: i64 = 0x1;
    pub const EPISODE_SHOW_UNSEEN: i64 = 0x2;
    pub const EPISODE_SHOW_SEEN: i64 = 0x4;
    pub const EPISODE_UNSEEN_MASK: i64 = 0x6;
    pub const EPISODE_SHOW_DOWNLOADED: i64 = 0x8;
    pub const EPISODE_SHOW_NOT_DOWNLOADED: i64 = 0x10;
    pub const EPISODE_DOWNLOADED_MASK: i64 = 0x18;
    pub const EPISODE_SHOW_BOOKMARKED: i64 = 0x20;
    pub const EPISODE_SHOW_NOT_BOOKMARKED: i64 = 0x40;
    pub const EPISODE_BOOKMARKED_MASK: i64 = 0x60;
    pub const EPISODE_SHOW_FILLERMARKED: i64 = 0x80;
    pub const EPISODE_SHOW_NOT_FILLERMARKED: i64 = 0x100;
    pub const EPISODE_FILLERMARKED_MASK: i64 = 0x180;
    pub const EPISODE_SORTING_SOURCE: i64 = 0x0;
    pub const EPISODE_SORTING_NUMBER: i64 = 0x200;
    pub const EPISODE_SORTING_UPLOAD_DATE: i64 = 0x400;
    pub const EPISODE_SORTING_ALPHABET: i64 = 0x600;
    pub const EPISODE_SORTING_MASK: i64 = 0x600;
    pub const EPISODE_SHOW_PREVIEWS: i64 = 0x0;
    pub const EPISODE_SHOW_NOT_PREVIEWS: i64 = 0x800;
    pub const EPISODE_PREVIEWS_MASK: i64 = 0x800;
    pub const EPISODE_SHOW_SUMMARIES: i64 = 0x0;
    pub const EPISODE_SHOW_NOT_SUMMARIES: i64 = 0x1000;
    pub const EPISODE_SUMMARIES_MASK: i64 = 0x1000;
    pub const EPISODE_DISPLAY_NAME: i64 = 0x0;
    pub const EPISODE_DISPLAY_NUMBER: i64 = 0x100000;
    pub const EPISODE_DISPLAY_MASK: i64 = 0x100000;

    pub const SEASON_SORT_DESC: i64 = 0x0;
    pub const SEASON_SORT_ASC: i64 = 0x1;
    pub const SEASON_SORT_DIR_MASK: i64 = 0x1;
    pub const SEASON_SHOW_DOWNLOADED: i64 = 0x2;
    pub const SEASON_SHOW_NOT_DOWNLOADED: i64 = 0x4;
    pub const SEASON_DOWNLOADED_MASK: i64 = 0x6;
    pub const SEASON_SHOW_UNSEEN: i64 = 0x8;
    pub const SEASON_SHOW_SEEN: i64 = 0x10;
    pub const SEASON_UNSEEN_MASK: i64 = 0x18;
    pub const SEASON_SHOW_STARTED: i64 = 0x20;
    pub const SEASON_SHOW_NOT_STARTED: i64 = 0x40;
    pub const SEASON_STARTED_MASK: i64 = 0x60;
    pub const SEASON_SHOW_COMPLETED: i64 = 0x80;
    pub const SEASON_SHOW_NOT_COMPLETED: i64 = 0x100;
    pub const SEASON_COMPLETED_MASK: i64 = 0x180;
    pub const SEASON_SHOW_BOOKMARKED: i64 = 0x200;
    pub const SEASON_SHOW_NOT_BOOKMARKED: i64 = 0x400;
    pub const SEASON_BOOKMARKED_MASK: i64 = 0x600;
    pub const SEASON_SHOW_FILLERMARKED: i64 = 0x800;
    pub const SEASON_SHOW_NOT_FILLERMARKED: i64 = 0x1000;
    pub const SEASON_FILLERMARKED_MASK: i64 = 0x1800;
    pub const SEASON_SORT_SOURCE: i64 = 0x0;
    pub const SEASON_SORT_SEASON: i64 = 0x2000;
    pub const SEASON_SORT_UPLOAD: i64 = 0x4000;
    pub const SEASON_SORT_ALPHABET: i64 = 0x6000;
    pub const SEASON_SORT_COUNT: i64 = 0x8000;
    pub const SEASON_SORT_LAST_SEEN: i64 = 0xA000;
    pub const SEASON_SORT_FETCHED: i64 = 0xC000;
    pub const SEASON_SORT_MASK: i64 = 0xE000;
    pub const SEASON_GRID_DISPLAY_MODE_SHIFT: i64 = 16;
    pub const SEASON_GRID_DISPLAY_MODE_MASK: i64 = 0x30000;
    pub const SEASON_GRID_DISPLAY_SIZE_SHIFT: i64 = 18;
    pub const SEASON_GRID_DISPLAY_SIZE_MASK: i64 = 0x3C0000;
    pub const SEASON_OVERLAY_DOWNLOADED_MASK: i64 = 0x400000;
    pub const SEASON_OVERLAY_UNSEEN_MASK: i64 = 0x800000;
    pub const SEASON_OVERLAY_LOCAL_MASK: i64 = 0x1000000;
    pub const SEASON_OVERLAY_LANG_MASK: i64 = 0x2000000;
    pub const SEASON_OVERLAY_CONT_MASK: i64 = 0x4000000;
    pub const SEASON_DISPLAY_MODE_SOURCE: i64 = 0x0;
    pub const SEASON_DISPLAY_MODE_NUMBER: i64 = 0x8000000;
    pub const SEASON_DISPLAY_MODE_MASK: i64 = 0x8000000;

    pub const ANIME_INTRO_MASK: i64 = 0xFF;
    pub const ANIME_AIRING_EPISODE_MASK: i64 = 0xFFFF00;
    /// Bits 24 to 55 (Kotlin `0x0FFFFFFFF000000L`).
    pub const ANIME_AIRING_TIME_MASK: i64 = 0xFFFF_FFFF << 24;
    pub const ANIME_INTRO_DISABLE_MASK: i64 = 0x0100_0000_0000_0000;

    pub const CATEGORY_SORT_TYPE_MASK: i64 = 0x3C;
    pub const CATEGORY_SORT_DIRECTION_MASK: i64 = 0x40;
    pub const CATEGORY_SORT_ALPHABETICAL: i64 = 0x00;
    pub const CATEGORY_SORT_LAST_SEEN: i64 = 0x04;
    pub const CATEGORY_SORT_LAST_UPDATE: i64 = 0x08;
    pub const CATEGORY_SORT_UNSEEN_COUNT: i64 = 0x0C;
    pub const CATEGORY_SORT_TOTAL_EPISODES: i64 = 0x10;
    pub const CATEGORY_SORT_LATEST_EPISODE: i64 = 0x14;
    pub const CATEGORY_SORT_EPISODE_FETCH_DATE: i64 = 0x18;
    pub const CATEGORY_SORT_DATE_ADDED: i64 = 0x1C;
    pub const CATEGORY_SORT_TRACKER_MEAN: i64 = 0x20;
    pub const CATEGORY_SORT_AIRING_TIME: i64 = 0x30;
    pub const CATEGORY_SORT_RANDOM: i64 = 0x3C;
    pub const CATEGORY_SORT_ASCENDING: i64 = 0x40;
    pub const CATEGORY_SORT_DESCENDING: i64 = 0x00;

    /// Skip-intro length in seconds (`viewer & 0xFF`).
    pub fn skip_intro_length(viewer: i64) -> i64 {
        viewer & ANIME_INTRO_MASK
    }

    /// Episode number of the next episode to air (`(viewer & 0xFFFF00) / 16^2`).
    pub fn next_episode_to_air(viewer: i64) -> i64 {
        (viewer & ANIME_AIRING_EPISODE_MASK) >> 8
    }

    /// Airing time of the next episode, Unix seconds (`(viewer & 0x0FFFFFFFF000000) / 16^6`).
    pub fn next_episode_airing_at(viewer: i64) -> i64 {
        (viewer & ANIME_AIRING_TIME_MASK) >> 24
    }

    pub fn skip_intro_disabled(viewer: i64) -> bool {
        viewer & ANIME_INTRO_DISABLE_MASK == ANIME_INTRO_DISABLE_MASK
    }

    /// Sort type of a `categories.flags` value. Unknown type bits fall back to Alphabetical.
    pub fn category_sort_type(flags: i64) -> i64 {
        const TYPES: [i64; 11] = [
            CATEGORY_SORT_ALPHABETICAL,
            CATEGORY_SORT_LAST_SEEN,
            CATEGORY_SORT_LAST_UPDATE,
            CATEGORY_SORT_UNSEEN_COUNT,
            CATEGORY_SORT_TOTAL_EPISODES,
            CATEGORY_SORT_LATEST_EPISODE,
            CATEGORY_SORT_EPISODE_FETCH_DATE,
            CATEGORY_SORT_DATE_ADDED,
            CATEGORY_SORT_TRACKER_MEAN,
            CATEGORY_SORT_AIRING_TIME,
            CATEGORY_SORT_RANDOM,
        ];
        let t = flags & CATEGORY_SORT_TYPE_MASK;
        if TYPES.contains(&t) { t } else { CATEGORY_SORT_ALPHABETICAL }
    }

    /// `animes.season_flags` grid display mode (Kotlin `SeasonDisplayMode`).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum SeasonDisplayMode {
        CompactGrid,
        ComfortableGrid,
        CoverOnlyGrid,
        List,
    }

    impl SeasonDisplayMode {
        /// Maps 0 to 3. Any other value is returned as `Err` (Kotlin throws there).
        pub fn from_i64(v: i64) -> Result<Self, i64> {
            match v {
                0 => Ok(Self::CompactGrid),
                1 => Ok(Self::ComfortableGrid),
                2 => Ok(Self::CoverOnlyGrid),
                3 => Ok(Self::List),
                other => Err(other),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::set_flag;
    use crate::open::DbError;
    use crate::schema_anime::flags as f;

    const SKIP_INTRO_CONTENT: &str = "local intro_length = mp.get_property_number(\"user-data/current-anime/intro-length\")\naniyomi.right_seek_by(intro_length)";
    const SKIP_INTRO_LONG_PRESS: &str = "aniyomi.int_picker(\"Change intro length\", \"%ds\", 0, 255, 1, \"user-data/current-anime/intro-length\")";
    const SKIP_INTRO_ON_STARTUP: &str = "function update_button(_, length)\n  if length ~= nil then\n    if length == 0 then\n\t  aniyomi.hide_button()\n\t  return\n\telse\n\t  aniyomi.show_button()\n\tend\n    aniyomi.set_button_title(\"+\" .. length .. \" s\")\n  end\nend\n\nif $isPrimary then\n  mp.observe_property(\"user-data/current-anime/intro-length\", \"number\", update_button)\nend";

    fn pragma_i64(c: &Connection, name: &str) -> i64 {
        c.pragma_query_value(None, name, |r| r.get(0)).unwrap()
    }

    fn count(c: &Connection, kind: &str) -> i64 {
        c.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%'",
            [kind],
            |r| r.get(0),
        )
        .unwrap()
    }

    /// One anime (`_id` 1, not a favorite, `description` NULL) and one episode (`_id` 1) of it.
    fn with_anime_and_episode() -> Connection {
        let c = open_in_memory().unwrap();
        c.execute_batch(
            "INSERT INTO animes(_id, source, url, title, status, favorite, initialized, viewer,
                episode_flags, cover_last_modified, date_added, season_flags, season_number,
                season_source_order, background_last_modified)
             VALUES (1, 1, 'u1', 'Alpha', 1, 0, 1, 0, 0, 0, 0, 0, -1.0, 0, 0);
             INSERT INTO episodes(_id, anime_id, url, name, seen, bookmark, last_second_seen,
                total_seconds, episode_number, source_order, date_fetch, date_upload, fillermark)
             VALUES (1, 1, 'e1', 'Ep 1', 0, 0, 0, 0, 1.0, 0, 0, 0, 0);",
        )
        .unwrap();
        c
    }

    fn anime_version(c: &Connection) -> i64 {
        c.query_row("SELECT version FROM animes WHERE _id = 1", [], |r| r.get(0)).unwrap()
    }

    fn episode_version(c: &Connection) -> i64 {
        c.query_row("SELECT version FROM episodes WHERE _id = 1", [], |r| r.get(0)).unwrap()
    }

    /// Asserts that a stamp column was written within 2 seconds of the database clock.
    fn assert_stamped_now(c: &Connection, sql: &str) {
        let (stamp, now): (i64, i64) = c
            .query_row(&format!("SELECT ({sql}), CAST(strftime('%s', 'now') AS INTEGER)"), [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert!((now - stamp).abs() <= 2, "{sql} = {stamp}, now = {now}");
    }

    #[test]
    fn file_name_and_versions_are_the_contract() {
        assert_eq!(ANIME_DB_FILE, "anime.db");
        assert_eq!(ANIME_SCHEMA_VERSION, 139);
        assert_eq!(db_path(Path::new("/data")), Path::new("/data/anime.db"));
    }

    #[test]
    fn open_in_data_dir_creates_anime_db_in_the_data_dir() {
        let paths = kuro_testkit::TempPaths::new().unwrap();
        drop(open_in_data_dir(&paths).unwrap());
        assert!(db_path(&paths.data_dir()).is_file());
    }

    #[test]
    fn open_sets_the_pragmas_on_a_file_database() {
        let dir = tempfile::tempdir().unwrap();
        let c = open(&db_path(dir.path())).unwrap();
        assert_eq!(pragma_i64(&c, "foreign_keys"), 1);
        assert_eq!(pragma_i64(&c, "synchronous"), 1);
        assert_eq!(pragma_i64(&c, "busy_timeout"), 5000);
        assert_eq!(pragma_i64(&c, "recursive_triggers"), 0);
        assert_eq!(pragma_i64(&c, "application_id"), ANIME_APPLICATION_ID);
        assert_eq!(pragma_i64(&c, "user_version"), 139);
        let mode: String = c.pragma_query_value(None, "journal_mode", |r| r.get(0)).unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn fresh_schema_has_ten_tables_nine_indexes_seven_triggers_and_eight_views() {
        let c = open_in_memory().unwrap();
        assert_eq!(count(&c, "table"), 10);
        assert_eq!(count(&c, "index"), 9);
        assert_eq!(count(&c, "trigger"), 7);
        assert_eq!(count(&c, "view"), 8);
    }

    #[test]
    fn empty_file_gets_the_fresh_schema_and_reopening_changes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = db_path(dir.path());
        let first = object_dump(&open(&path).unwrap()).unwrap();
        let second = object_dump(&open(&path).unwrap()).unwrap();
        assert_eq!(first, second);
        assert_eq!(count(&open_in_memory().unwrap(), "table"), 10);
    }

    #[test]
    fn newer_file_is_refused_and_left_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        let path = db_path(dir.path());
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE t(x); PRAGMA user_version = 9999;")
            .unwrap();
        let before = std::fs::read(&path).unwrap();
        let err = open(&path).unwrap_err();
        assert!(matches!(err, DbError::TooNew { found: 9999, supported: 139 }), "{err:?}");
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }

    #[test]
    fn aniyomi_file_is_refused_by_version_and_left_byte_identical() {
        let dir = tempfile::tempdir().unwrap();
        // application_id 0 is the Aniyomi value. 139 is the Aniyomi version of this schema, 120 is
        // in the legacy chain range, 50 is below it.
        let cases = [
            (139, DbError::LegacyFile { found: 139 }),
            (120, DbError::NeedsLegacyImport { found: 120 }),
            (50, DbError::TooOldForImport { found: 50 }),
        ];
        for (version, want) in cases {
            let path = dir.path().join(format!("aniyomi-{version}.db"));
            Connection::open(&path)
                .unwrap()
                .execute_batch(&format!("CREATE TABLE animes(x); PRAGMA user_version = {version};"))
                .unwrap();
            let before = std::fs::read(&path).unwrap();
            let err = open(&path).unwrap_err();
            assert_eq!(format!("{err:?}"), format!("{want:?}"));
            assert_eq!(std::fs::read(&path).unwrap(), before);
        }
    }

    #[test]
    fn failed_creation_rolls_back_to_an_empty_file() {
        const BROKEN: SchemaBundle = SchemaBundle {
            baseline_sql: "CREATE TABLE t(x); CREATE TABLE t(x);",
            baseline_version: ANIME_SCHEMA_VERSION,
            migrations: &[],
            application_id: ANIME_APPLICATION_ID,
            tables: &[],
            refuse_foreign,
        };
        let mut c = Connection::open_in_memory().unwrap();
        // The second statement fails, after the first one ran.
        assert!(create_baseline(&mut c, &BROKEN).is_err());
        assert_eq!(count(&c, "table"), 0);
        assert_eq!(pragma_i64(&c, "user_version"), 0);
        assert_eq!(pragma_i64(&c, "application_id"), 0);
    }

    #[test]
    fn seed_rows_exist_and_the_system_category_cannot_be_deleted() {
        let c = open_in_memory().unwrap();
        let cat: (i64, String, i64, i64, i64) = c
            .query_row("SELECT _id, name, sort, flags, hidden FROM categories", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })
            .unwrap();
        assert_eq!(cat, (0, String::new(), -1, 64, 0), "fresh seed row takes Alphabetical Ascending (Q28)");
        let button: (String, bool, i64) = c
            .query_row(
                "SELECT name, isFavorite, sortIndex FROM custom_buttons WHERE _id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(button, ("+85 s".to_owned(), true, 0));
        let err = c.execute("DELETE FROM categories WHERE _id = 0", []).unwrap_err();
        assert!(err.to_string().contains("System category can't be deleted"), "{err}");
    }

    #[test]
    fn replaying_a_seed_insert_or_ignore_changes_nothing() {
        let c = open_in_memory().unwrap();
        c.execute_batch(
            "INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, 'x', 9, 9);
             INSERT OR IGNORE INTO custom_buttons(_id, name, isFavorite, sortIndex, content,
                longPressContent, onStartup) VALUES (1, 'x', 0, 9, '', '', '');",
        )
        .unwrap();
        let name: String = c.query_row("SELECT name FROM categories WHERE _id = 0", [], |r| r.get(0)).unwrap();
        assert_eq!(name, "");
        let button_name: String =
            c.query_row("SELECT name FROM custom_buttons WHERE _id = 1", [], |r| r.get(0)).unwrap();
        assert_eq!(button_name, "+85 s");
    }

    #[test]
    fn skip_intro_seed_lua_is_byte_exact() {
        let c = open_in_memory().unwrap();
        let (content, long_press, on_startup): (String, String, String) = c
            .query_row(
                "SELECT content, longPressContent, onStartup FROM custom_buttons WHERE _id = 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(content, SKIP_INTRO_CONTENT);
        assert_eq!(long_press, SKIP_INTRO_LONG_PRESS);
        assert_eq!(on_startup, SKIP_INTRO_ON_STARTUP);
    }

    #[test]
    fn anime_update_with_every_value_kept_stamps_but_keeps_version() {
        let c = with_anime_and_episode();
        c.execute("UPDATE animes SET favorite = favorite, url = url WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 0);
        assert_stamped_now(&c, "SELECT last_modified_at FROM animes WHERE _id = 1");
        assert_stamped_now(&c, "SELECT favorite_modified_at FROM animes WHERE _id = 1");
    }

    #[test]
    fn anime_version_follows_the_measured_table() {
        let c = with_anime_and_episode();
        // NULL to text is not a change (NULL comparisons do not fire the trigger).
        c.execute("UPDATE animes SET description = 'a' WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 0);
        c.execute("UPDATE animes SET description = 'b' WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 1);
        c.execute("UPDATE animes SET url = 'u2' WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 2);
        c.execute("UPDATE animes SET favorite = 1 WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 3);
        assert_stamped_now(&c, "SELECT favorite_modified_at FROM animes WHERE _id = 1");
    }

    #[test]
    fn anime_version_is_frozen_while_syncing_and_resumes_after() {
        let c = with_anime_and_episode();
        c.execute("UPDATE animes SET description = 'a' WHERE _id = 1", []).unwrap();
        c.execute("UPDATE animes SET is_syncing = 1, version = 50 WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 50);
        c.execute("UPDATE animes SET is_syncing = 0, description = 'b' WHERE _id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 51);
    }

    #[test]
    fn category_link_insert_bumps_version_and_delete_does_not() {
        let c = with_anime_and_episode();
        c.execute_batch(
            "INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'a', 0, 0), (2, 'b', 1, 0);",
        )
        .unwrap();
        c.execute_batch(
            "INSERT INTO animes_categories(anime_id, category_id) VALUES (1, 1);
             INSERT INTO animes_categories(anime_id, category_id) VALUES (1, 2);",
        )
        .unwrap();
        assert_eq!(anime_version(&c), 2);
        c.execute("DELETE FROM animes_categories WHERE anime_id = 1", []).unwrap();
        assert_eq!(anime_version(&c), 2);
    }

    #[test]
    fn deleting_a_normal_category_removes_its_links() {
        let c = with_anime_and_episode();
        c.execute_batch(
            "INSERT INTO categories(_id, name, sort, flags) VALUES (1, 'a', 0, 0);
             INSERT INTO animes_categories(anime_id, category_id) VALUES (1, 1);",
        )
        .unwrap();
        c.execute("DELETE FROM categories WHERE _id = 1", []).unwrap();
        let links: i64 = c.query_row("SELECT count(*) FROM animes_categories", [], |r| r.get(0)).unwrap();
        assert_eq!(links, 0);
    }

    #[test]
    fn episode_seen_bumps_both_versions_and_other_columns_follow_the_table() {
        let c = with_anime_and_episode();
        c.execute("UPDATE episodes SET seen = 1 WHERE _id = 1", []).unwrap();
        assert_eq!((episode_version(&c), anime_version(&c)), (1, 1));
        c.execute("UPDATE episodes SET fillermark = 1 WHERE _id = 1", []).unwrap();
        assert_eq!((episode_version(&c), anime_version(&c)), (1, 1));
        c.execute("UPDATE episodes SET total_seconds = 1400 WHERE _id = 1", []).unwrap();
        assert_eq!((episode_version(&c), anime_version(&c)), (1, 1));
        c.execute("UPDATE episodes SET last_second_seen = 30 WHERE _id = 1", []).unwrap();
        assert_eq!((episode_version(&c), anime_version(&c)), (2, 2));
        assert_stamped_now(&c, "SELECT last_modified_at FROM episodes WHERE _id = 1");
    }

    #[test]
    fn explicit_episode_version_gets_one_more_from_the_trigger() {
        let c = with_anime_and_episode();
        c.execute("UPDATE episodes SET seen = 1, version = 5, is_syncing = 0 WHERE _id = 1", [])
            .unwrap();
        assert_eq!(episode_version(&c), 6);
        assert_eq!(anime_version(&c), 1);
    }

    #[test]
    fn episode_change_while_anime_syncs_bumps_only_the_episode() {
        let c = with_anime_and_episode();
        c.execute("UPDATE episodes SET version = 9 WHERE _id = 1", []).unwrap();
        c.execute("UPDATE animes SET is_syncing = 1 WHERE _id = 1", []).unwrap();
        c.execute("UPDATE episodes SET bookmark = 1 WHERE _id = 1", []).unwrap();
        assert_eq!(episode_version(&c), 10);
        assert_eq!(anime_version(&c), 0);
    }

    #[test]
    fn column_codecs_round_trip_through_sqlite() {
        let c = Connection::open_in_memory().unwrap();
        // Millis: INTEGER milliseconds, NULL reads as None.
        let v: Option<Millis> = c.query_row("SELECT 1234", [], |r| r.get(0)).unwrap();
        assert_eq!(v, Some(Millis(1234)));
        let v: Option<Millis> = c.query_row("SELECT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(v, None);
        // FetchType: 0 Seasons, 1 Episodes, anything else Episodes.
        for (stored, want) in [(0, FetchType::Seasons), (1, FetchType::Episodes), (2, FetchType::Episodes), (-1, FetchType::Episodes)] {
            let v: FetchType = c.query_row("SELECT ?1", [stored], |r| r.get(0)).unwrap();
            assert_eq!(v, want, "stored {stored}");
        }
        let n: i64 = c.query_row("SELECT ?1", [FetchType::Seasons], |r| r.get(0)).unwrap();
        assert_eq!(n, 0);
        // Booleans: any non-zero integer is true, writes are 0 or 1.
        let b: bool = c.query_row("SELECT 2", [], |r| r.get(0)).unwrap();
        assert!(b);
        let n: i64 = c.query_row("SELECT ?1", [true], |r| r.get(0)).unwrap();
        assert_eq!(n, 1);
        // UpdateStrategy: the shared codec, 0 and 1 known, 99 reads as AlwaysUpdate.
        let s: UpdateStrategy = c.query_row("SELECT 99", [], |r| r.get(0)).unwrap();
        assert_eq!(s, UpdateStrategy::AlwaysUpdate);
    }

    #[test]
    fn memo_reads_text_and_blob_and_falls_back_to_empty_object() {
        let c = Connection::open_in_memory().unwrap();
        let read = |sql: &str| -> Memo { c.query_row(sql, [], |r| r.get(0)).unwrap() };
        assert_eq!(read("SELECT '{}'"), Memo::default());
        assert_eq!(read("SELECT x'7b7d'"), Memo::default());
        let mut want = serde_json::Map::new();
        want.insert("a".into(), serde_json::json!([1, 2]));
        assert_eq!(read(r#"SELECT CAST('{"a":[1,2]}' AS BLOB)"#), Memo(want));
        assert_eq!(read("SELECT 'not json'"), Memo::default());
        assert_eq!(read("SELECT '[1,2]'"), Memo::default());
    }

    #[test]
    fn strict_memo_decode_names_the_table_and_row_of_a_bad_cell() {
        let ok = Memo::decode(ValueRef::Blob(b"{\"a\":[1,2]}"), "animes", 7).unwrap();
        assert_eq!(ok.0["a"], serde_json::json!([1, 2]));
        assert_eq!(Memo::decode(ValueRef::Text(b"{}"), "animes", 7).unwrap(), Memo::default());
        for bad in [ValueRef::Blob(b"[1]"), ValueRef::Text(b"not json"), ValueRef::Blob(&[0xFF, 0xFE]), ValueRef::Null] {
            let err = Memo::decode(bad, "episodes", 9).unwrap_err();
            assert!(matches!(err, DbError::BadMemo { table: "episodes", id: 9 }), "{err:?}");
        }
    }

    #[test]
    fn memo_writes_compact_utf8_json_as_blob() {
        let c = Connection::open_in_memory().unwrap();
        let mut map = serde_json::Map::new();
        map.insert("k".into(), serde_json::json!(1));
        let stored: Vec<u8> = c
            .query_row("SELECT ?1", [Memo(map)], |r| r.get(0))
            .unwrap();
        assert_eq!(stored, br#"{"k":1}"#);
        let stored: Vec<u8> = c.query_row("SELECT ?1", [Memo::default()], |r| r.get(0)).unwrap();
        assert_eq!(stored, b"{}");
    }

    #[test]
    fn genre_list_round_trip_uses_the_shared_codec() {
        let c = Connection::open_in_memory().unwrap();
        let v: GenreList = c.query_row("SELECT 'Action, Sci-Fi'", [], |r| r.get(0)).unwrap();
        assert_eq!(v, GenreList(vec!["Action".into(), "Sci-Fi".into()]));
    }

    #[test]
    fn episode_flag_constants_match_the_kotlin_values() {
        let want = [
            (f::EPISODE_SORT_ASC, 0x1), (f::EPISODE_SORT_DIR_MASK, 0x1), (f::EPISODE_UNSEEN_MASK, 0x6),
            (f::EPISODE_SHOW_SEEN, 0x4), (f::EPISODE_DOWNLOADED_MASK, 0x18), (f::EPISODE_SHOW_NOT_DOWNLOADED, 0x10),
            (f::EPISODE_BOOKMARKED_MASK, 0x60), (f::EPISODE_SHOW_NOT_BOOKMARKED, 0x40),
            (f::EPISODE_FILLERMARKED_MASK, 0x180), (f::EPISODE_SHOW_NOT_FILLERMARKED, 0x100),
            (f::EPISODE_SORTING_ALPHABET, 0x600), (f::EPISODE_SORTING_UPLOAD_DATE, 0x400),
            (f::EPISODE_SORTING_NUMBER, 0x200), (f::EPISODE_SORTING_MASK, 0x600),
            (f::EPISODE_SHOW_NOT_PREVIEWS, 0x800), (f::EPISODE_SHOW_NOT_SUMMARIES, 0x1000),
            (f::EPISODE_DISPLAY_NUMBER, 0x100000), (f::EPISODE_DISPLAY_MASK, 0x100000),
        ];
        for (got, expected) in want {
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn season_flag_constants_match_the_kotlin_values() {
        let want = [
            (f::SEASON_SORT_ASC, 0x1), (f::SEASON_DOWNLOADED_MASK, 0x6), (f::SEASON_SHOW_NOT_DOWNLOADED, 0x4),
            (f::SEASON_UNSEEN_MASK, 0x18), (f::SEASON_SHOW_SEEN, 0x10), (f::SEASON_STARTED_MASK, 0x60),
            (f::SEASON_SHOW_NOT_STARTED, 0x40), (f::SEASON_COMPLETED_MASK, 0x180),
            (f::SEASON_SHOW_NOT_COMPLETED, 0x100), (f::SEASON_BOOKMARKED_MASK, 0x600),
            (f::SEASON_SHOW_NOT_BOOKMARKED, 0x400), (f::SEASON_FILLERMARKED_MASK, 0x1800),
            (f::SEASON_SHOW_NOT_FILLERMARKED, 0x1000), (f::SEASON_SORT_MASK, 0xE000),
            (f::SEASON_SORT_LAST_SEEN, 0xA000), (f::SEASON_SORT_FETCHED, 0xC000),
            (f::SEASON_SORT_COUNT, 0x8000), (f::SEASON_GRID_DISPLAY_MODE_MASK, 0x30000),
            (f::SEASON_GRID_DISPLAY_SIZE_MASK, 0x3C0000), (f::SEASON_OVERLAY_DOWNLOADED_MASK, 0x400000),
            (f::SEASON_OVERLAY_CONT_MASK, 0x4000000), (f::SEASON_DISPLAY_MODE_NUMBER, 0x8000000),
            (f::SEASON_DISPLAY_MODE_MASK, 0x8000000), (f::SEASON_GRID_DISPLAY_MODE_SHIFT, 16),
            (f::SEASON_GRID_DISPLAY_SIZE_SHIFT, 18),
        ];
        for (got, expected) in want {
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn library_sort_flags_match_the_kotlin_values() {
        assert_eq!(f::CATEGORY_SORT_DATE_ADDED | f::CATEGORY_SORT_ASCENDING, 0b0101_1100);
        assert_eq!(f::CATEGORY_SORT_UNSEEN_COUNT | f::CATEGORY_SORT_DESCENDING, 0b0000_1100);
        assert_eq!(f::CATEGORY_SORT_ALPHABETICAL | f::CATEGORY_SORT_ASCENDING, 0b0100_0000);
        // Replacing the sort on UnseenCount + Descending with DateAdded + Ascending.
        let mask = f::CATEGORY_SORT_TYPE_MASK | f::CATEGORY_SORT_DIRECTION_MASK;
        let composed = set_flag(
            f::CATEGORY_SORT_UNSEEN_COUNT | f::CATEGORY_SORT_DESCENDING,
            f::CATEGORY_SORT_DATE_ADDED | f::CATEGORY_SORT_ASCENDING,
            mask,
        );
        assert_eq!(composed, 0b0101_1100);
        assert_ne!(composed, 0b0000_1100);
        let last_seen_then_date_added = set_flag(
            f::CATEGORY_SORT_LAST_SEEN | f::CATEGORY_SORT_ASCENDING,
            f::CATEGORY_SORT_DATE_ADDED | f::CATEGORY_SORT_ASCENDING,
            mask,
        );
        assert_eq!(last_seen_then_date_added, 0b0101_1100);
    }

    #[test]
    fn category_sort_constants_and_fallback() {
        assert_eq!(f::CATEGORY_SORT_TYPE_MASK, 0x3C);
        assert_eq!(f::CATEGORY_SORT_DIRECTION_MASK, 0x40);
        assert_eq!(f::CATEGORY_SORT_AIRING_TIME, 0x30);
        assert_eq!(f::CATEGORY_SORT_RANDOM, 0x3C);
        assert_eq!(f::category_sort_type(0x44), f::CATEGORY_SORT_LAST_SEEN);
        assert_eq!(f::category_sort_type(0x40), f::CATEGORY_SORT_ALPHABETICAL);
        // 0x24 masks to 0x24, which is not one of the eleven types.
        assert_eq!(f::category_sort_type(0x24), f::CATEGORY_SORT_ALPHABETICAL);
    }

    #[test]
    fn set_flag_reproduces_the_kotlin_setter() {
        assert_eq!(set_flag(0, 0x2, 0x6), 2);
        assert_eq!(set_flag(0x7, 0x4, 0x6), 0x5);
    }

    #[test]
    fn set_all_episode_flags_matches_the_kotlin_results() {
        // SetAnimeEpisodeFlags.awaitSetAllFlags builds from 0 with one setFlag per mask. Each row is
        // (unseen, downloaded, bookmarked, fillermarked, sorting, direction, display, previews,
        // summaries) and the expected value was computed with the Kotlin formula.
        let vectors: [([i64; 9], i64); 10] = [
            ([0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0], 0x0),
            ([0x2, 0x8, 0x20, 0x80, 0x200, 0x1, 0x100000, 0x800, 0x1000], 0x101aab),
            ([0x4, 0x10, 0x40, 0x100, 0x400, 0x0, 0x0, 0x0, 0x1000], 0x1554),
            ([0x2, 0x18, 0x60, 0x180, 0x600, 0x1, 0x0, 0x800, 0x0], 0xffb),
            ([0x0, 0x8, 0x0, 0x100, 0x200, 0x0, 0x100000, 0x0, 0x0], 0x100308),
            ([0x4, 0x0, 0x20, 0x0, 0x0, 0x1, 0x0, 0x800, 0x1000], 0x1825),
            ([0x2, 0x10, 0x0, 0x80, 0x400, 0x0, 0x100000, 0x0, 0x1000], 0x101492),
            ([0x0, 0x0, 0x40, 0x180, 0x600, 0x1, 0x0, 0x0, 0x0], 0x7c1),
            ([0x4, 0x18, 0x0, 0x0, 0x200, 0x1, 0x100000, 0x800, 0x0], 0x100a1d),
            ([0x2, 0x0, 0x60, 0x100, 0x0, 0x0, 0x100000, 0x0, 0x1000], 0x101162),
        ];
        let masks = [
            f::EPISODE_UNSEEN_MASK,
            f::EPISODE_DOWNLOADED_MASK,
            f::EPISODE_BOOKMARKED_MASK,
            f::EPISODE_FILLERMARKED_MASK,
            f::EPISODE_SORTING_MASK,
            f::EPISODE_SORT_DIR_MASK,
            f::EPISODE_DISPLAY_MASK,
            f::EPISODE_PREVIEWS_MASK,
            f::EPISODE_SUMMARIES_MASK,
        ];
        for (inputs, expected) in vectors {
            let flags = inputs.into_iter().zip(masks).fold(0, |acc, (flag, mask)| set_flag(acc, flag, mask));
            assert_eq!(flags, expected, "inputs {inputs:x?}");
        }
    }

    #[test]
    fn viewer_accessors_match_the_kotlin_getters() {
        let viewer = (1_700_000_000_i64 << 24) | (12 << 8) | 85;
        assert_eq!(f::skip_intro_length(viewer), 85);
        assert_eq!(f::next_episode_to_air(viewer), 12);
        assert_eq!(f::next_episode_airing_at(viewer), 1_700_000_000);
        assert!(!f::skip_intro_disabled(viewer));
        let disabled = viewer | (1 << 56);
        assert!(f::skip_intro_disabled(disabled));
        assert_eq!(f::skip_intro_length(disabled), 85);
        assert_eq!(f::next_episode_to_air(disabled), 12);
        assert_eq!(f::next_episode_airing_at(disabled), 1_700_000_000);
        assert_eq!((f::skip_intro_length(0), f::next_episode_to_air(0), f::next_episode_airing_at(0)), (0, 0, 0));
        assert!(!f::skip_intro_disabled(0));
    }

    #[test]
    fn season_display_mode_maps_zero_to_three_and_rejects_the_rest() {
        use f::SeasonDisplayMode as Mode;
        assert_eq!(
            (0..4).map(Mode::from_i64).collect::<Result<Vec<_>, _>>().unwrap(),
            [Mode::CompactGrid, Mode::ComfortableGrid, Mode::CoverOnlyGrid, Mode::List]
        );
        assert_eq!(Mode::from_i64(4), Err(4));
        assert_eq!(Mode::from_i64(-1), Err(-1));
    }

    #[test]
    fn name_and_column_dump_is_stable_for_the_golden_file() {
        let c = open_in_memory().unwrap();
        let dump = object_dump(&c).unwrap();
        assert!(dump.contains("table animes on animes\n"));
        assert!(dump.contains("  0|_id|INTEGER|1|-|1\n"));
        assert!(dump.contains("view animelibView on animelibView\n"));
    }

    #[test]
    fn anime_schema_matches_the_golden_object_dump() {
        let c = open_in_memory().unwrap();
        let dump = object_dump(&c).unwrap();
        kuro_testkit::assert_golden(
            concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden/anime_schema_v139.txt"),
            dump,
        );
    }
}
