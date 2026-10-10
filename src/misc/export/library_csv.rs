//! Library CSV rows (T-35a-070 model and favourites query, T-35a-071 renderer, T-35a-095 golden tests). The bytes follow
//! p1-topic-library-csv-export.md sections 3 to 6: no header row, one row per favourite, the enabled cells joined with
//! `,`, rows joined with `\r\n` and no trailing newline, and the fixed column order Title, Type, Author, Artist.
//!
//! Decision: the rows are built from `Anime` and `Manga` values the caller read from the repositories, so the
//! favourites query is an ordering and a map and needs no SQL of its own. The caller reads both lists when Save is
//! confirmed (T-35a-076).

use kuro_data::anime::Anime;
use kuro_data::manga::Manga;
use kuro_domain::kind::EntryKind;

/// Which columns the export includes. Every column is on at first (`Default`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportOptions {
    pub title: bool,
    pub kind: bool,
    pub author: bool,
    pub artist: bool,
}

impl ExportOptions {
    pub const ALL: Self = Self { title: true, kind: true, author: true, artist: true };

    /// The Title rule of the dialog (T-35a-072): unchecking Title unchecks the other three, and checking it again
    /// leaves them unchecked.
    pub fn with_title(self, title: bool) -> Self {
        if title { Self { title, ..self } } else { Self { title, kind: false, author: false, artist: false } }
    }

    /// Whether the other three checkboxes can be changed. They are disabled while Title is off (T-35a-072).
    pub fn others_enabled(self) -> bool {
        self.title
    }
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self::ALL
    }
}

/// One favourite as the export reads it: only the title, the kind, the author and the artist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportEntry {
    pub title: String,
    pub kind: EntryKind,
    pub author: Option<String>,
    pub artist: Option<String>,
}

fn kind_text(kind: EntryKind) -> &'static str {
    match kind {
        EntryKind::Manga => "manga",
        EntryKind::Anime => "anime",
    }
}

/// Rows in ascending `_id` order (the `ORDER BY _id` of the favourites query).
pub fn by_id<T: Clone>(rows: &[T], id: impl Fn(&T) -> i64) -> Vec<T> {
    let mut sorted = rows.to_vec();
    sorted.sort_by_key(|row| id(row));
    sorted
}

/// The favourites in export order (T-35a-070): anime rows first, then manga rows, each by ascending `_id`.
pub fn favourite_entries(anime: &[Anime], manga: &[Manga]) -> Vec<ExportEntry> {
    let from_anime = by_id(anime, |row| row.id).into_iter().map(|row| ExportEntry {
        title: row.title,
        kind: EntryKind::Anime,
        author: row.author,
        artist: row.artist,
    });
    let from_manga = by_id(manga, |row| row.id).into_iter().map(|row| ExportEntry {
        title: row.title,
        kind: EntryKind::Manga,
        author: row.author,
        artist: row.artist,
    });
    from_anime.chain(from_manga).collect()
}

/// One cell (section 4): a blank or missing value is empty; a value with `\r`, `\n`, `"` or `,` is quoted with each `"`
/// doubled; any other value is written as it is.
pub fn cell(value: Option<&str>) -> String {
    match value {
        None => String::new(),
        Some(text) if text.trim().is_empty() => String::new(),
        Some(text) if text.contains(['\r', '\n', '"', ',']) => format!("\"{}\"", text.replace('"', "\"\"")),
        Some(text) => text.to_owned(),
    }
}

/// The whole file (sections 5 and 6): the enabled cells of each entry joined with `,`, rows joined with `\r\n`.
pub fn render(entries: &[ExportEntry], options: ExportOptions) -> String {
    entries
        .iter()
        .map(|entry| {
            let mut cells = Vec::with_capacity(4);
            if options.title {
                cells.push(cell(Some(&entry.title)));
            }
            if options.kind {
                cells.push(cell(Some(kind_text(entry.kind))));
            }
            if options.author {
                cells.push(cell(entry.author.as_deref()));
            }
            if options.artist {
                cells.push(cell(entry.artist.as_deref()));
            }
            cells.join(",")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const NONE: ExportOptions = ExportOptions { title: false, kind: false, author: false, artist: false };

    fn entry(title: &str, kind: EntryKind, author: Option<&str>, artist: Option<&str>) -> ExportEntry {
        ExportEntry { title: title.into(), kind, author: author.map(Into::into), artist: artist.map(Into::into) }
    }

    /// Section 6 worked example (T-35a-095 golden): the bytes, not the text, are compared.
    fn worked_example() -> Vec<ExportEntry> {
        vec![
            entry("Frieren", EntryKind::Anime, Some("Kanehito Yamada"), None),
            entry("He said \"hi\", then left", EntryKind::Manga, Some("line one\nline two"), Some("")),
            entry("Padding  ", EntryKind::Manga, Some("   "), Some("Ayumi")),
        ]
    }

    #[test]
    fn the_worked_example_matches_the_golden_bytes() {
        let expected = "Frieren,anime,Kanehito Yamada,\r\n\"He said \"\"hi\"\", then left\",manga,\"line one\nline two\",\r\nPadding  ,manga,,Ayumi";
        assert_eq!(render(&worked_example(), ExportOptions::ALL).as_bytes(), expected.as_bytes());
    }

    #[test]
    fn zero_rows_give_an_empty_file_and_zero_columns_give_crlf_between_two_rows() {
        assert_eq!(render(&[], ExportOptions::ALL), "");
        assert_eq!(render(&worked_example()[..1], NONE), "");
        assert_eq!(render(&worked_example()[..2], NONE), "\r\n");
    }

    #[test]
    fn title_off_and_type_on_writes_only_the_kind_column() {
        let options = ExportOptions { title: false, ..ExportOptions::ALL };
        assert_eq!(render(&worked_example(), options), "anime\r\nmanga\r\nmanga");
    }

    #[test]
    fn a_quote_is_doubled_and_the_field_is_quoted() {
        assert_eq!(cell(Some("He said \"hi\"")), "\"He said \"\"hi\"\"\"");
    }

    #[test]
    fn a_blank_or_missing_value_is_an_empty_cell_even_with_a_comma() {
        assert_eq!(cell(Some("   ")), "");
        assert_eq!(cell(Some(",")), "\",\"");
        assert_eq!(cell(Some("")), "");
        assert_eq!(cell(None), "");
    }

    #[test]
    fn a_trailing_space_is_kept_and_not_quoted() {
        assert_eq!(cell(Some("Padding  ")), "Padding  ");
    }

    #[test]
    fn the_title_rule_clears_the_other_three_and_rechecking_keeps_them_clear() {
        let off = ExportOptions::ALL.with_title(false);
        assert_eq!(off, NONE);
        assert!(!off.others_enabled());
        let on_again = off.with_title(true);
        assert_eq!(on_again, ExportOptions { title: true, ..NONE });
        assert!(on_again.others_enabled());
    }

    #[test]
    fn rows_come_back_in_ascending_id_order_without_touching_the_input() {
        let rows = vec![(9, "late"), (2, "early"), (5, "middle")];
        let sorted = by_id(&rows, |row| row.0);
        assert_eq!(sorted, vec![(2, "early"), (5, "middle"), (9, "late")]);
        assert_eq!(rows[0], (9, "late"));
    }
}
