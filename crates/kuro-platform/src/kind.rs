//! The one "manga or anime" of the whole app (docs/design/unify-map.md, step 2). Every crate uses this type instead of
//! its own two-variant enum; what differs between the kinds is a method here or an extension trait in the crate that
//! owns the difference. It lives in the lowest crate so every other one can use it.

/// Manga or anime. Serialised in lowercase (`"manga"`, `"anime"`), as repository indexes and feeds write it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Manga,
    Anime,
}

impl Kind {
    /// Both kinds, manga first.
    pub const ALL: [Kind; 2] = [Kind::Manga, Kind::Anime];

    /// `"manga"` or `"anime"`: the lowercase name used in files, links and preference keys.
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Manga => "manga",
            Kind::Anime => "anime",
        }
    }

    /// The kind named by `"manga"` or `"anime"` (any case).
    pub fn parse(text: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|kind| kind.as_str().eq_ignore_ascii_case(text.trim()))
    }

    pub fn is_anime(self) -> bool {
        self == Kind::Anime
    }

    pub fn is_manga(self) -> bool {
        self == Kind::Manga
    }

    /// Upcoming statuses of the library's Upcoming view: manga `[1, 4]`, anime `[1, 4, 7]`.
    pub const fn upcoming_statuses(self) -> &'static [i64] {
        match self {
            Kind::Manga => &[1, 4],
            Kind::Anime => &[1, 4, 7],
        }
    }

    /// `Anime` when `anime` is true: for the places that still carry a flag.
    pub fn from_anime(anime: bool) -> Kind {
        if anime { Kind::Anime } else { Kind::Manga }
    }
}

impl std::fmt::Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_name_round_trips_and_unknown_text_is_none() {
        for kind in Kind::ALL {
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(Kind::parse(" Anime "), Some(Kind::Anime));
        assert_eq!(Kind::parse("novel"), None);
        assert_eq!(Kind::from_anime(true), Kind::Anime);
        assert_eq!(Kind::Manga.to_string(), "manga");
        assert_eq!(Kind::Manga.upcoming_statuses(), &[1, 4]);
        assert_eq!(Kind::Anime.upcoming_statuses(), &[1, 4, 7]);
        assert_eq!(serde_json::to_string(&Kind::Anime).unwrap(), "\"anime\"");
    }
}
