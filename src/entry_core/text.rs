//! Info-header text rules: title, author, artist, status, description and its expansion default.
//! Upstream: `MangaInfoHeader.kt` (`MangaContentInfo`, `ExpandableMangaDescription`), `SManga.kt:48-54`.
//! Icon names are proposals (Adwaita names); the widget layer checks them with `gtk::IconTheme::has_icon`.

use kuro_i18n::{MsgKey, tr};

/// The title line: `title.ifBlank { "Unknown title" }`.
pub fn display_title(title: &str) -> String {
    if title.trim().is_empty() {
        tr(MsgKey::unknown_title)
    } else {
        title.to_owned()
    }
}

/// The scanlator to show: nothing for a blank one or the placeholder "Unknown" some sources send.
pub fn shown_scanlator(scanlator: Option<&str>) -> Option<&str> {
    scanlator.map(str::trim).filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("unknown"))
}

/// The author line: `author` or "Unknown author" when blank.
pub fn display_author(author: Option<&str>) -> String {
    match author.filter(|a| !a.trim().is_empty()) {
        Some(a) => a.to_owned(),
        None => tr(MsgKey::unknown_author),
    }
}

/// The artist row. It is shown only when the artist is non-blank and differs from the author (null-safe).
pub fn artist_row(author: Option<&str>, artist: Option<&str>) -> Option<String> {
    let artist = artist.filter(|a| !a.trim().is_empty())?;
    if author == Some(artist) {
        None
    } else {
        Some(artist.to_owned())
    }
}

#[cfg(test)]
/// Status icon name for `SManga` status values 0..6. 0 and unknown values use the block icon.
pub fn status_icon(status: i64) -> &'static str {
    match status {
        1 => "document-open-recent-symbolic",
        2 => "emblem-ok-symbolic",
        3 => "money-symbolic",
        4 => "object-select-symbolic",
        5 => "window-close-symbolic",
        6 => "media-playback-pause-symbolic",
        _ => "action-unavailable-symbolic",
    }
}

#[cfg(test)]
/// Status text for `SManga` status values 0..6 (`ongoing`, `completed`, ...). 0 and unknown values are "Unknown".
pub fn status_text(status: i64) -> String {
    tr(match status {
        1 => MsgKey::ongoing,
        2 => MsgKey::completed,
        3 => MsgKey::licensed,
        4 => MsgKey::publishing_finished,
        5 => MsgKey::cancelled,
        6 => MsgKey::on_hiatus,
        _ => MsgKey::unknown,
    })
}

/// The description in both forms. `expanded` is the raw text. `shrunk` is the collapsed text used when collapsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Description {
    pub expanded: String,
    pub shrunk: String,
}

/// Blank descriptions show "No description". The collapsed form collapses runs of two or more CR/LF into one
/// `\n` and drops trailing whitespace (Kotlin `trimEnd`, characters up to `' '`).
pub fn description(raw: Option<&str>) -> Description {
    let expanded = match raw.filter(|d| !d.trim().is_empty()) {
        Some(d) => d.to_owned(),
        None => tr(MsgKey::description_placeholder),
    };
    let shrunk = collapse_line_breaks(&expanded)
        .trim_end_matches(|c: char| c <= ' ')
        .to_owned();
    Description { expanded, shrunk }
}

/// The collapsed description shows this many lines, with a scrim this tall over the last one.
pub const COLLAPSED_LINES: i32 = 3;
pub const SCRIM_PX: i32 = 24;

/// Whether the description is expanded. It lives as long as the page: the caret and the text both toggle it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expansion {
    expanded: bool,
}

impl Expansion {
    /// Always collapsed when a page opens (user, 2026-10-12): opening a title again never remembers an expanded text.
    pub fn new(_large_layout: bool, _from_source: bool) -> Self {
        Self { expanded: false }
    }

    pub fn is_expanded(self) -> bool {
        self.expanded
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }

    /// The scrim is drawn only while the text is collapsed.
    pub fn scrim(self) -> bool {
        !self.expanded
    }
}

/// `replace([\r\n]{2,}, "\n")`. A single CR or LF is kept as it is.
pub fn collapse_line_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut run = String::new();
    for c in text.chars() {
        if c == '\r' || c == '\n' {
            run.push(c);
            continue;
        }
        flush_run(&mut out, &run);
        run.clear();
        out.push(c);
    }
    flush_run(&mut out, &run);
    out
}

fn flush_run(out: &mut String, run: &str) {
    match run.len() {
        0 => {}
        1 => out.push_str(run),
        _ => out.push('\n'),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_or_unknown_scanlator_shows_nothing() {
        assert_eq!(shown_scanlator(None), None);
        assert_eq!(shown_scanlator(Some("  ")), None);
        assert_eq!(shown_scanlator(Some("Unknown")), None);
        assert_eq!(shown_scanlator(Some(" unknown ")), None);
        assert_eq!(shown_scanlator(Some("Official")), Some("Official"));
    }

    #[test]
    fn expansion_always_starts_collapsed_and_toggles() {
        let mut small = Expansion::new(false, false);
        assert!(!small.is_expanded() && small.scrim());
        small.toggle();
        assert!(small.is_expanded() && !small.scrim());
        assert!(!Expansion::new(false, true).is_expanded(), "opened from a source: collapsed too");
        assert!(!Expansion::new(true, false).is_expanded(), "the two-pane layout: collapsed too");
    }

    #[test]
    fn blank_title_and_author_use_placeholders() {
        assert_eq!(display_title("  "), "Unknown title");
        assert_eq!(display_title("Berserk"), "Berserk");
        assert_eq!(display_author(None), "Unknown author");
        assert_eq!(display_author(Some("")), "Unknown author");
        assert_eq!(display_author(Some("Miura")), "Miura");
    }

    #[test]
    fn artist_hidden_when_blank_or_equal_to_author() {
        assert_eq!(artist_row(Some("Miura"), Some("Miura")), None);
        assert_eq!(artist_row(Some("Miura"), Some(" ")), None);
        assert_eq!(artist_row(None, Some("Miura")), Some("Miura".to_owned()));
        assert_eq!(artist_row(Some("A"), Some("B")), Some("B".to_owned()));
    }

    #[test]
    fn status_mapping_and_unknown_fallback() {
        assert_eq!(status_icon(1), "document-open-recent-symbolic");
        assert_eq!(status_icon(0), "action-unavailable-symbolic");
        assert_eq!(status_icon(99), "action-unavailable-symbolic");
        assert_eq!(status_text(1), "Ongoing");
        assert_eq!(status_text(2), "Completed");
        assert_eq!(status_text(4), "Publishing finished");
        assert_eq!(status_text(6), "On hiatus");
        assert_eq!(status_text(0), "Unknown");
        assert_eq!(status_text(42), "Unknown");
    }

    #[test]
    fn blank_description_shows_placeholder() {
        let d = description(Some("   "));
        assert_eq!(d.expanded, "No description");
        assert_eq!(description(None), d);
    }

    #[test]
    fn whitespace_rule_collapses_runs_only_in_shrunk_text() {
        let d = description(Some("a\r\n\r\nb\n\n\n\nc\nd \n\n"));
        assert_eq!(d.expanded, "a\r\n\r\nb\n\n\n\nc\nd \n\n");
        assert_eq!(d.shrunk, "a\nb\nc\nd");
    }

    #[test]
    fn single_newlines_are_kept() {
        assert_eq!(collapse_line_breaks("x\ny\rz"), "x\ny\rz");
        assert_eq!(collapse_line_breaks("x\r\ny"), "x\ny");
    }
}
