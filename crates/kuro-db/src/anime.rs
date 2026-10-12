//! The anime database: constants defined once, and the flag and codec modules under their plan names.
//! Everything else in the crate reads the constants here.

/// The anime file and its schema, shared by `schema_anime` and the `Kind` of the handle.
pub mod consts {
    /// File name of the anime database inside the data directory.
    pub const FILE_NAME: &str = "anime.db";
    /// `PRAGMA user_version` of the fresh anime schema: the Aniyomi schema 139 plus the native step 139 to 140.
    pub const SCHEMA_VERSION: i64 = 140;
    /// The Aniyomi schema version the baseline SQL is compatible with; files at this version run the native steps.
    pub const BASELINE_VERSION: i64 = 139;
}

/// Flag bit layouts of the anime tables (T-04a-006): the constants, the decoders and `set_flag`.
pub mod flags {
    pub use crate::flags::set_flag;
    pub use crate::schema_anime::flags::*;
}

/// Column codecs of the anime tables (T-04a-005). `StringList` is the Aniyomi name of `GenreList`.
pub mod codec {
    pub use crate::codec::{GenreList, GenreList as StringList, UpdateStrategy};
    pub use crate::schema_anime::{FetchType, Memo, Millis};
}
