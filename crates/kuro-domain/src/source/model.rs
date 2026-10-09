//! The source value model: visual name, the stable key and the last-used key (`Source.kt`, `AnimeSource.kt`).
//! Manga and anime share this shape. The data-saver exclusion is a manga flag; an anime source leaves it false.

use super::pin::Pins;

pub type Id = i64;

/// The name the source area uses for [`Source`] (`SourceEntry`, area 17 section 2.2). One type serves both kinds.
pub type SourceEntry = Source;

/// Which kind of source a stub belongs to. The kind decides the stub table (`sources` or `animesources`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceKind {
    Manga,
    Anime,
}

impl SourceKind {
    /// The table that holds this kind's stub rows (T-17a-004).
    pub const fn stub_table(self) -> &'static str {
        match self {
            Self::Manga => "sources",
            Self::Anime => "animesources",
        }
    }
}

/// An installed or stub source as the lists show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    pub id: Id,
    pub lang: String,
    pub name: String,
    pub supports_latest: bool,
    pub is_stub: bool,
    pub pin: Pins,
    pub is_used_last: bool,
    /// Manga only: the source is left out of data saver. Always false for anime.
    pub data_saver_excluded: bool,
}

impl Source {
    /// A source with the defaults of both constructors: unpinned, not used last, data saver not excluded.
    pub fn new(id: Id, name: &str, lang: &str) -> Self {
        Self {
            id,
            lang: lang.to_owned(),
            name: name.to_owned(),
            supports_latest: false,
            is_stub: false,
            pin: Pins::UNPINNED,
            is_used_last: false,
            data_saver_excluded: false,
        }
    }

    /// `name`, or `name (LANG)` when the language is set.
    pub fn visual_name(&self) -> String {
        if self.lang.is_empty() { self.name.clone() } else { format!("{} ({})", self.name, self.lang.to_uppercase()) }
    }

    /// `{id}`, or `{id}-lastused` for the source used last.
    pub fn key(&self) -> String {
        if self.is_used_last { format!("{}-lastused", self.id) } else { self.id.to_string() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_names_its_own_stub_table() {
        assert_eq!(SourceKind::Manga.stub_table(), "sources");
        assert_eq!(SourceKind::Anime.stub_table(), "animesources");
        let entry: SourceEntry = Source::new(3, "Named", "en");
        assert_eq!(entry.visual_name(), "Named (EN)");
    }

    #[test]
    fn visual_name_adds_the_upper_case_language_when_present() {
        assert_eq!(Source::new(1, "Example", "").visual_name(), "Example");
        assert_eq!(Source::new(1, "Example", "en").visual_name(), "Example (EN)");
        assert_eq!(Source::new(1, "Example", "pt-BR").visual_name(), "Example (PT-BR)");
    }

    #[test]
    fn key_adds_the_last_used_suffix_only_when_used_last() {
        let mut s = Source::new(42, "Example", "en");
        assert_eq!(s.key(), "42");
        s.is_used_last = true;
        assert_eq!(s.key(), "42-lastused");
    }

    #[test]
    fn constructor_defaults_match_the_upstream_constructors() {
        let s = Source::new(7, "n", "");
        assert_eq!(s.pin, Pins::UNPINNED);
        assert!(!s.is_used_last);
        assert!(!s.is_stub && !s.supports_latest);
    }

    #[test]
    fn data_saver_exclusion_defaults_to_off_and_keeps_the_value_it_is_given() {
        let mut source = Source::new(3, "Named", "en");
        assert!(!source.data_saver_excluded);
        source.data_saver_excluded = true;
        let copy = source.clone();
        assert!(copy.data_saver_excluded);
        assert_eq!(copy, source);
    }
}
