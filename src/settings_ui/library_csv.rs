//! The Library CSV export (task T-47a-091): the favourites as rows of title, type, author and artist. The format is the
//! Android `LibraryExporter`: no header row, one row per favourite, rows joined with `\r\n` and no trailing newline, the
//! unchecked columns left out, a blank value written as an empty field, and a field holding `\r`, `\n`, `"` or `,`
//! wrapped in double quotes with each `"` doubled.
//!
//! Decision: the Library List dialog keeps Android's rule for Title. Unchecking it clears the other three, so the file
//! then has one empty line per favourite (the Android output).

use kuro_i18n::{MsgKey, tr};

/// The default save name (the Android `aniyomi_library.csv`, renamed for this app).
pub const FILE_NAME: &str = "kurobayomi_library.csv";
/// The toast after a successful write.
pub fn exported() -> String {
    tr(MsgKey::library_exported)
}
/// The toast after a failed write (a proposal: Android shows no error toast).
pub fn failed() -> String {
    tr(MsgKey::settings_library_csv_failed)
}

/// Which columns the export includes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Columns {
    pub title: bool,
    pub kind: bool,
    pub author: bool,
    pub artist: bool,
}

impl Columns {
    /// Every column (the dialog's starting state).
    pub const ALL: Columns = Columns { title: true, kind: true, author: true, artist: true };

    /// The dialog rule for Title: unchecking it clears the other three, and checking it leaves them as they are.
    pub fn with_title(self, title: bool) -> Columns {
        if title { Columns { title, ..self } } else { Columns { title, kind: false, author: false, artist: false } }
    }
}

/// One favourite as the export reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub title: String,
    /// `manga` or `anime`.
    pub kind: &'static str,
    pub author: Option<String>,
    pub artist: Option<String>,
}

/// One field: a blank value is empty; a value with a separator, a quote or a line break is quoted with its quotes doubled.
pub fn field(value: &str) -> String {
    if value.trim().is_empty() {
        String::new()
    } else if value.contains(['\r', '\n', '"', ',']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// The whole file for `entries` with `columns` included.
pub fn export(entries: &[Entry], columns: Columns) -> String {
    entries
        .iter()
        .map(|entry| {
            let mut fields = Vec::new();
            if columns.title {
                fields.push(field(&entry.title));
            }
            if columns.kind {
                fields.push(field(entry.kind));
            }
            if columns.author {
                fields.push(field(entry.author.as_deref().unwrap_or("")));
            }
            if columns.artist {
                fields.push(field(entry.artist.as_deref().unwrap_or("")));
            }
            fields.join(",")
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(title: &str, kind: &'static str, author: Option<&str>, artist: Option<&str>) -> Entry {
        Entry { title: title.into(), kind, author: author.map(Into::into), artist: artist.map(Into::into) }
    }

    #[test]
    fn rows_are_joined_with_crlf_and_there_is_no_trailing_newline() {
        let rows = [entry("A", "manga", None, None), entry("B", "anime", None, None)];
        assert_eq!(export(&rows, Columns::ALL), "A,manga,,\r\nB,anime,,");
    }

    #[test]
    fn the_columns_come_in_the_order_title_type_author_artist() {
        let rows = [entry("T", "anime", Some("Au"), Some("Ar"))];
        assert_eq!(export(&rows, Columns::ALL), "T,anime,Au,Ar");
    }

    #[test]
    fn an_unchecked_column_is_left_out_of_the_row() {
        let rows = [entry("T", "manga", Some("Au"), Some("Ar"))];
        let cols = Columns { title: true, kind: false, author: true, artist: false };
        assert_eq!(export(&rows, cols), "T,Au");
    }

    #[test]
    fn a_field_with_a_comma_quote_or_line_break_is_quoted_with_doubled_quotes() {
        assert_eq!(field("a,b"), "\"a,b\"");
        assert_eq!(field("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(field("line\nbreak"), "\"line\nbreak\"");
        assert_eq!(field("cr\rhere"), "\"cr\rhere\"");
    }

    #[test]
    fn a_blank_or_missing_value_is_an_empty_field() {
        assert_eq!(field("   "), "");
        assert_eq!(field(""), "");
        assert_eq!(export(&[entry("X", "manga", None, Some("  "))], Columns::ALL), "X,manga,,");
    }

    #[test]
    fn an_empty_library_gives_an_empty_file() {
        assert_eq!(export(&[], Columns::ALL), "");
    }

    #[test]
    fn unchecking_title_clears_the_other_three_and_checking_it_keeps_them() {
        let off = Columns::ALL.with_title(false);
        assert_eq!(off, Columns { title: false, kind: false, author: false, artist: false });
        let on = Columns { title: false, kind: true, author: false, artist: true }.with_title(true);
        assert_eq!(on, Columns { title: true, kind: true, author: false, artist: true });
    }

    #[test]
    fn with_title_unchecked_each_favourite_is_an_empty_line() {
        let rows = [entry("A", "manga", None, None), entry("B", "anime", None, None)];
        assert_eq!(export(&rows, Columns::ALL.with_title(false)), "\r\n");
    }
}
