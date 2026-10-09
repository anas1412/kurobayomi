//! `MangasPage`: one page of a catalogue listing (T-15a-006).

use super::SManga;

/// The manga of one listing page and whether the source reports a page after it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MangasPage {
    pub mangas: Vec<SManga>,
    pub has_next_page: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equality_is_structural() {
        let manga = SManga::new("/m/1", "One");
        let a = MangasPage { mangas: vec![manga.clone()], has_next_page: true };
        let b = MangasPage { mangas: vec![manga.clone()], has_next_page: true };
        assert_eq!(a, b);
        let c = MangasPage { mangas: vec![manga], has_next_page: false };
        assert_ne!(a, c);
        assert_ne!(a, MangasPage { mangas: Vec::new(), has_next_page: true });
    }
}
