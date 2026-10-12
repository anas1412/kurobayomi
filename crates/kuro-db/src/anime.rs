//! The anime flag and codec modules under their plan names.

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
