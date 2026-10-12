//! The markdown that sources put in descriptions (Mihon renders it), turned into Pango markup for a `gtk::Label`:
//! links, bold, italic, code, headings (bold) and `-`/`*`/`+` lists (bullets). Anything else stays as written. A text
//! the converter cannot turn into valid markup is shown escaped, never broken.

use std::sync::LazyLock;

use gtk::glib;
use regex::Regex;

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+(.*?)\s*#*\s*$").unwrap());
static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\s*)[-*+]\s+").unwrap());
static LINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[([^\]]+)\]\((https?://[^)\s]+)\)").unwrap());
static BOLD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\*\*(\S(?:.*?\S)?)\*\*|__(\S(?:.*?\S)?)__").unwrap());
static ITALIC: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(^|[\s(])[*_](\S(?:[^*_]*?\S)?)[*_]").unwrap());
static CODE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`([^`]+)`").unwrap());

/// `text` as Pango markup.
pub fn to_markup(text: &str) -> String {
    let markup = text.lines().map(line).collect::<Vec<_>>().join("\n");
    if is_valid(&markup) { markup } else { glib::markup_escape_text(text).to_string() }
}

/// Pango does not know `<a>` (only a GTK label does), so links are checked as spans.
fn is_valid(markup: &str) -> bool {
    let spans = markup.replace("</a>", "</span>").replace("<a href=", "<span font_features=");
    gtk::pango::parse_markup(&spans, '\0').is_ok()
}

fn line(raw: &str) -> String {
    let escaped = glib::markup_escape_text(raw).to_string();
    if let Some(heading) = HEADING.captures(&escaped) {
        return format!("<b>{}</b>", inline(&heading[1]));
    }
    let bulleted = BULLET.replace(&escaped, "$1• ");
    inline(&bulleted)
}

fn inline(text: &str) -> String {
    let text = CODE.replace_all(text, "<tt>$1</tt>");
    let text = LINK.replace_all(&text, r#"<a href="$2">$1</a>"#);
    let text = BOLD.replace_all(&text, "<b>$1$2</b>");
    ITALIC.replace_all(&text, "$1<i>$2</i>").into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_bold_italic_lists_and_headings_become_markup() {
        assert_eq!(to_markup("- [AniList](https://anilist.co/manga/46765)"), r#"• <a href="https://anilist.co/manga/46765">AniList</a>"#);
        assert_eq!(to_markup("**Bold** and *italic* and _this_"), "<b>Bold</b> and <i>italic</i> and <i>this</i>");
        assert_eq!(to_markup("## Synopsis"), "<b>Synopsis</b>");
        assert_eq!(to_markup("a `code` b"), "a <tt>code</tt> b");
    }

    #[test]
    fn plain_text_and_special_characters_stay_as_written() {
        assert_eq!(to_markup("Tom & Jerry <3, 2*3*4, snake_case_name"), "Tom &amp; Jerry &lt;3, 2*3*4, snake_case_name");
        assert_eq!(to_markup("a [link](https://x.org/?a=1&b=2)"), r#"a <a href="https://x.org/?a=1&amp;b=2">link</a>"#);
        assert!(is_valid(&to_markup("**unclosed *mix** of_ marks_")));
    }
}
