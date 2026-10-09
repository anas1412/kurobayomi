//! The two entry kinds and the identifiers that differ between them (`p1-rust-design.md` section 2).
//! Manga and anime share one code path; every name that differs comes from this enum.

/// Which library a category, membership or history row belongs to.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum EntryKind {
    Manga,
    Anime,
}

impl EntryKind {
    pub const ALL: [EntryKind; 2] = [EntryKind::Manga, EntryKind::Anime];

    /// The category-membership table.
    pub fn membership_table(self) -> &'static str {
        match self {
            Self::Manga => "mangas_categories",
            Self::Anime => "animes_categories",
        }
    }

    /// The entry column of the membership table.
    pub fn entry_fk(self) -> &'static str {
        match self {
            Self::Manga => "manga_id",
            Self::Anime => "anime_id",
        }
    }

    /// The history table.
    pub fn history_table(self) -> &'static str {
        match self {
            Self::Manga => "history",
            Self::Anime => "animehistory",
        }
    }

    /// The item column of the history table.
    pub fn item_fk(self) -> &'static str {
        match self {
            Self::Manga => "chapter_id",
            Self::Anime => "episode_id",
        }
    }

    /// The last-activity column of the history table.
    pub fn timestamp_col(self) -> &'static str {
        match self {
            Self::Manga => "last_read",
            Self::Anime => "last_seen",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_match_the_schema_names() {
        assert_eq!(EntryKind::Manga.membership_table(), "mangas_categories");
        assert_eq!(EntryKind::Anime.membership_table(), "animes_categories");
        assert_eq!(EntryKind::Manga.entry_fk(), "manga_id");
        assert_eq!(EntryKind::Anime.entry_fk(), "anime_id");
        assert_eq!(EntryKind::Manga.history_table(), "history");
        assert_eq!(EntryKind::Anime.history_table(), "animehistory");
        assert_eq!(EntryKind::Manga.item_fk(), "chapter_id");
        assert_eq!(EntryKind::Anime.item_fk(), "episode_id");
        assert_eq!(EntryKind::Manga.timestamp_col(), "last_read");
        assert_eq!(EntryKind::Anime.timestamp_col(), "last_seen");
    }
}
