//! `DbError`, the one error type of the crate. Every module returns it through `DbResult`.

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("constraint violation: {0}")]
    Constraint(String),
    #[error("Database is newer than this version of Kurobayomi (found {found}, supported {supported})")]
    TooNew { found: i64, supported: i64 },
    #[error("not a Kurobayomi database (schema {found})")]
    ForeignFile { found: i64 },
    #[error("bad memo in {table} row {id}")]
    BadMemo { table: &'static str, id: i64 },
    #[error("integrity check failed: {0}")]
    Corrupt(String),
    #[error("journal_mode is {0}, expected wal")]
    JournalMode(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("background task failed: {0}")]
    Task(String),
    #[error("not found")]
    NotFound,
}

pub type DbResult<T> = Result<T, DbError>;
