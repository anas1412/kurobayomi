//! The base list row of the manga and anime migration screens (T-38a-044, T-39a-061). Upstream: `BaseMangaListItem.kt` and
//! `BaseAnimeListItem.kt`: a row of 76 px with 16 px side and 3 px vertical padding, and three slots (cover, content,
//! actions). Pure: the row widget reads these values. A click on the cover runs the cover action, which defaults to the
//! row action.

/// The vertical padding above and below the content of a row.
pub const PAD_VERTICAL: i32 = 3;
