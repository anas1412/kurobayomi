//! The info header, GTK-free: the lines it shows, what a click searches, what a long press copies, and the cover size.
//! Upstream: `MangaInfoHeader.kt:336-414` (layouts), `:424-520` (title, author, artist, source rows).

use crate::entry_core::detail::EntryState;
use crate::entry_core::text::{artist_row, display_author, display_title, status_icon, status_text};

/// The small layout never draws the cover wider than this.
pub const SMALL_COVER_MAX: i32 = 100;
/// The large layout draws the cover at this share of its column.
pub const LARGE_COVER_SHARE: f64 = 0.65;

/// A line of the header that can be clicked or copied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
    Title,
    Author,
    Artist,
    Source,
}

/// "Search for `query`". `global` searches every source, otherwise only the source of the manga.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRequest {
    pub query: String,
    pub global: bool,
}

/// The header lines, with placeholders for blank values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HeaderModel {
    pub title: String,
    pub author: String,
    /// `None` when blank or equal to the author.
    pub artist: Option<String>,
    pub status_icon: &'static str,
    pub status: String,
    pub source: String,
    /// The source extension is missing: the warning icon is drawn in the error color.
    pub stub: bool,
    raw_title: String,
    raw_author: Option<String>,
}

impl HeaderModel {
    pub fn new(page: &EntryState) -> Self {
        let m = &page.manga;
        Self::from_parts(
            &m.title,
            m.author.as_deref(),
            m.artist.as_deref(),
            (status_icon(m.status), status_text(m.status)),
            &page.source.name,
            page.source.stub,
        )
    }

    /// The header of any entry kind: the caller supplies the status icon and text, because manga and anime have
    /// different status tables.
    pub fn from_parts(
        title: &str,
        author: Option<&str>,
        artist: Option<&str>,
        status: (&'static str, String),
        source: &str,
        stub: bool,
    ) -> Self {
        Self {
            title: display_title(title),
            author: display_author(author),
            artist: artist_row(author, artist),
            status_icon: status.0,
            status: status.1,
            source: source.to_owned(),
            stub,
            raw_title: title.to_owned(),
            raw_author: author.map(str::to_owned),
        }
    }

    /// The raw text of a field when it is non-blank. Placeholders are never copied or searched.
    fn raw(&self, field: Field) -> Option<&str> {
        let text = match field {
            Field::Title => Some(self.raw_title.as_str()),
            Field::Author => self.raw_author.as_deref(),
            Field::Artist => self.artist.as_deref(),
            Field::Source => Some(self.source.as_str()),
        };
        text.filter(|t| !t.trim().is_empty())
    }

    /// A click: title, author and artist search every source, the source name searches its own source.
    pub fn search(&self, field: Field) -> Option<SearchRequest> {
        Some(SearchRequest {
            query: self.raw(field)?.to_owned(),
            global: field != Field::Source,
        })
    }

    /// A long press or right click: the text to copy.
    pub fn copy(&self, field: Field) -> Option<String> {
        self.raw(field).map(str::to_owned)
    }
}

/// True when a window of this size gets the large layout: the tablet UI (the smaller side is at least 720).
pub fn is_large(window_width: i32, window_height: i32) -> bool {
    crate::window_size::is_tablet_ui(window_width, window_height)
}

/// The cover size (width, height; 2:3) for a layout. `column_width` is the width of the column that holds it.
pub fn cover_size(large: bool, column_width: i32) -> (i32, i32) {
    let width = if large {
        (f64::from(column_width) * LARGE_COVER_SHARE).round() as i32
    } else {
        column_width.min(SMALL_COVER_MAX)
    };
    (width, width * 3 / 2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_core::detail::{LoadContext, LoadState, MangaRow, SourceInfo, finish_load};

    fn model(title: &str, author: Option<&str>, artist: Option<&str>, stub: bool) -> HeaderModel {
        let manga = MangaRow {
            id: 1,
            source: 1,
            url: String::new(),
            title: title.into(),
            author: author.map(str::to_owned),
            artist: artist.map(str::to_owned),
            description: None,
            genres: vec![],
            status: 0,
            thumbnail_url: None,
            initialized: true,
            favorite: true,
            local: false,
            flags: Default::default(),
        };
        let source = SourceInfo { name: "Source A (EN)".into(), stub };
        let LoadState::Ready(page) = finish_load(Some(manga), vec![], source, &LoadContext::default()).state else {
            unreachable!()
        };
        HeaderModel::new(&page)
    }

    #[test]
    fn blank_title_and_author_show_placeholders() {
        let m = model("", None, None, false);
        assert_eq!(m.title, "Unknown title");
        assert_eq!(m.author, "Unknown author");
        assert_eq!(m.artist, None);
    }

    #[test]
    fn artist_row_appears_only_when_it_differs_from_the_author() {
        assert_eq!(model("t", Some("A"), Some("A"), false).artist, None);
        assert_eq!(model("t", Some("A"), Some("B"), false).artist.as_deref(), Some("B"));
    }

    #[test]
    fn clicks_search_globally_except_the_source_which_stays_in_its_source() {
        let m = model("Berserk", Some("Miura"), Some("Studio"), false);
        let q = |f| m.search(f).map(|r| (r.query, r.global));
        assert_eq!(q(Field::Title), Some(("Berserk".into(), true)));
        assert_eq!(q(Field::Author), Some(("Miura".into(), true)));
        assert_eq!(q(Field::Artist), Some(("Studio".into(), true)));
        assert_eq!(q(Field::Source), Some(("Source A (EN)".into(), false)));
    }

    #[test]
    fn placeholders_are_neither_searched_nor_copied() {
        let m = model(" ", None, None, false);
        assert_eq!(m.search(Field::Title), None);
        assert_eq!(m.copy(Field::Title), None);
        assert_eq!(m.search(Field::Author), None);
        assert_eq!(m.copy(Field::Artist), None);
        assert_eq!(m.copy(Field::Source).as_deref(), Some("Source A (EN)"));
    }

    #[test]
    fn status_line_comes_from_the_manga_status() {
        let m = model("t", None, None, false);
        assert_eq!(m.status, "Unknown");
        assert_eq!(m.status_icon, "action-unavailable-symbolic");
    }

    #[test]
    fn stub_flag_reaches_the_header() {
        assert!(model("t", None, None, true).stub);
        assert!(!model("t", None, None, false).stub);
    }

    #[test]
    fn small_cover_is_capped_at_100_and_large_is_65_percent() {
        assert_eq!(cover_size(false, 400), (100, 150));
        assert_eq!(cover_size(false, 80), (80, 120));
        assert_eq!(cover_size(true, 400), (260, 390));
        assert!(!is_large(719, 900));
        assert!(is_large(720, 900));
        assert!(!is_large(1200, 719), "a short window is not a tablet");
    }
}
