//! The syntax colours of the code editor and the conf dialogs as text tags (task T-48a-039). The rules and their
//! colours are in `highlight.rs`; this turns their byte spans into one colour per character and paints a
//! `gtk::TextBuffer` with them. Where two spans overlap, the one found later wins, as in the Kotlin app.
use gtk::prelude::*;

use super::highlight::{self, Span};

/// The two rule sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Lua,
    Conf,
}

/// The coloured runs of `text` as `(start, end, colour)` in characters, in text order and never overlapping. Where
/// spans overlap the later span in `spans` takes the characters.
pub fn runs(text: &str, spans: &[Span]) -> Vec<(i32, i32, &'static str)> {
    let mut char_at = vec![0usize; text.len() + 1];
    let mut count = 0;
    for (byte, ch) in text.char_indices() {
        for slot in &mut char_at[byte..byte + ch.len_utf8()] {
            *slot = count;
        }
        count += 1;
    }
    char_at[text.len()] = count;
    let mut colors: Vec<Option<&'static str>> = vec![None; count];
    for span in spans {
        let (from, to) = (char_at[span.start.min(text.len())], char_at[span.end.min(text.len())]);
        colors[from..to].fill(Some(span.color));
    }
    let mut out: Vec<(i32, i32, &'static str)> = Vec::new();
    for (at, color) in colors.iter().enumerate() {
        let Some(color) = color else { continue };
        match out.last_mut() {
            Some(last) if last.1 == at as i32 && last.2 == *color => last.1 += 1,
            _ => out.push((at as i32, at as i32 + 1, color)),
        }
    }
    out
}

fn tag(buffer: &gtk::TextBuffer, color: &str) -> gtk::TextTag {
    buffer
        .tag_table()
        .lookup(color)
        .unwrap_or_else(|| buffer.create_tag(Some(color), &[("foreground", &color)]).expect("a new tag"))
}

fn paint(buffer: &gtk::TextBuffer, lang: Lang) {
    let (start, end) = buffer.bounds();
    buffer.remove_all_tags(&start, &end);
    let text = buffer.text(&start, &end, false);
    let spans = match lang {
        Lang::Lua => highlight::lua(&text),
        Lang::Conf => highlight::conf(&text),
    };
    for (from, to, color) in runs(&text, &spans) {
        buffer.apply_tag(&tag(buffer, color), &buffer.iter_at_offset(from), &buffer.iter_at_offset(to));
    }
}

/// Colours `buffer` now and again after every edit.
pub fn attach(buffer: &gtk::TextBuffer, lang: Lang) {
    paint(buffer, lang);
    buffer.connect_changed(move |buffer| paint(buffer, lang));
}

#[cfg(test)]
mod tests {
    use super::*;
    use highlight::color;

    fn span(start: usize, end: usize, color: &'static str) -> Span {
        Span { start, end, color }
    }

    #[test]
    fn byte_spans_become_character_runs() {
        let text = "é = 1";
        let found = runs(text, &[span(0, 2, color::KEYWORD), span(5, 6, color::LITERAL)]);
        assert_eq!(found, [(0, 1, color::KEYWORD), (4, 5, color::LITERAL)]);
    }

    #[test]
    fn a_later_span_wins_where_two_overlap_and_neighbours_of_one_colour_merge() {
        let found = runs("abcdef", &[span(0, 4, color::STRING), span(2, 3, color::COMMENT), span(4, 6, color::STRING)]);
        assert_eq!(found, [(0, 2, color::STRING), (2, 3, color::COMMENT), (3, 6, color::STRING)]);
    }

    #[test]
    fn no_spans_and_empty_text_give_no_runs() {
        assert!(runs("", &[]).is_empty());
        assert!(runs("abc", &[]).is_empty());
    }

    #[test]
    fn the_runs_of_a_real_lua_line_cover_the_keyword_and_the_comment() {
        let text = "local x -- hi";
        let found = runs(text, &highlight::lua(text));
        assert!(found.contains(&(0, 5, color::KEYWORD)));
        assert!(found.contains(&(8, 13, color::COMMENT)));
    }
}
