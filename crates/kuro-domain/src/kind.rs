//! The two entry kinds and the identifiers that differ between them (`p1-rust-design.md` section 2).
//! Manga and anime share one code path; every name that differs comes from this enum.

use kuro_platform::kind::Kind;

/// The table and column names that differ between the two kinds.
pub trait EntryKindExt {
    /// The category-membership table.
    fn membership_table(self) -> &'static str;
    /// The entry column of the membership table.
    fn entry_fk(self) -> &'static str;
    /// The history table.
    fn history_table(self) -> &'static str;
    /// The item column of the history table.
    fn item_fk(self) -> &'static str;
    /// The last-activity column of the history table.
    fn timestamp_col(self) -> &'static str;
}

impl EntryKindExt for Kind {
    fn membership_table(self) -> &'static str {
        match self {
            Self::Manga => "entry_categories",
            Self::Anime => "entry_categories",
        }
    }
    fn entry_fk(self) -> &'static str {
        match self {
            Self::Manga => "entry_id",
            Self::Anime => "entry_id",
        }
    }
    fn history_table(self) -> &'static str {
        match self {
            Self::Manga => "history",
            Self::Anime => "history",
        }
    }
    fn item_fk(self) -> &'static str {
        match self {
            Self::Manga => "item_id",
            Self::Anime => "item_id",
        }
    }
    fn timestamp_col(self) -> &'static str {
        match self {
            Self::Manga => "last_at",
            Self::Anime => "last_at",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_match_the_schema_names() {
        assert_eq!(Kind::Manga.membership_table(), "entry_categories");
        assert_eq!(Kind::Anime.membership_table(), "entry_categories");
        assert_eq!(Kind::Manga.entry_fk(), "entry_id");
        assert_eq!(Kind::Anime.entry_fk(), "entry_id");
        assert_eq!(Kind::Manga.history_table(), "history");
        assert_eq!(Kind::Anime.history_table(), "history");
        assert_eq!(Kind::Manga.item_fk(), "item_id");
        assert_eq!(Kind::Anime.item_fk(), "item_id");
        assert_eq!(Kind::Manga.timestamp_col(), "last_at");
        assert_eq!(Kind::Anime.timestamp_col(), "last_at");
    }
}
