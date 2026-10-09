//! Jsoup-compatible helpers over `scraper` (`JsoupExtensions.kt`; T-15a-032, T-15a-033).
//!
//! Every helper here works on an `ElementRef` of a document that is alive for the duration of a synchronous call.

use kuro_source::SourceError;
use scraper::{ElementRef, Html, Selector};
use url::Url;

/// The whitespace Jsoup's `text()` collapses: space, tab, LF, form feed, CR and NBSP.
fn is_jsoup_space(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\u{0C}' | '\r' | '\u{A0}')
}

/// Collapses every run of the Jsoup whitespace set to one space and trims both ends (EC-PH-03).
pub fn normalise_whitespace(raw: &str) -> String {
    raw.split(is_jsoup_space).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

/// `Element.text()`: the text of the element and its descendants, normalised.
// ponytail: text nodes are concatenated with no space at block boundaries; Jsoup adds one. Add it when a fixture
// shows a title glued across two block elements.
pub fn text(element: ElementRef<'_>) -> String {
    normalise_whitespace(&element.text().collect::<String>())
}

/// Parses a CSS selector. A selector the parser rejects is `Parse`, the counterpart of Jsoup's `SelectorParseException`
/// (EC-PH-15).
pub fn parse_selector(css: &str) -> Result<Selector, SourceError> {
    Selector::parse(css).map_err(|err| SourceError::Parse(format!("selector {css:?}: {err:?}")))
}

/// `selectText(css)`: the text of the first match. `None` when nothing matches (the caller applies its default), and
/// `Some("")` when the match is empty (EC-PH-01, EC-PH-02).
pub fn select_text(element: ElementRef<'_>, css: &str) -> Result<Option<String>, SourceError> {
    let selector = parse_selector(css)?;
    Ok(element.select(&selector).next().map(text))
}

/// `selectInt(css, default)`: the first match read as an integer, accepting a leading `+`. `default` when nothing
/// matches. Empty or non-numeric text is `Parse` (EC-PH-04..07).
pub fn select_int(element: ElementRef<'_>, css: &str, default: i32) -> Result<i32, SourceError> {
    match select_text(element, css)? {
        None => Ok(default),
        Some(raw) => raw.parse::<i32>().map_err(|_| SourceError::Parse(format!("not an integer: {raw:?}"))),
    }
}

/// `attrOrText(attr)`: the attribute of the element itself, or its text when `attr` is `"text"`. A missing attribute
/// is `""`. It never looks at children (EC-PH-08, EC-PH-09).
pub fn attr_or_text(element: ElementRef<'_>, attr: &str) -> String {
    if attr == "text" {
        text(element)
    } else {
        element.value().attr(attr).unwrap_or("").to_owned()
    }
}

/// The base URI of a document: the first `<base href>` resolved against the response URL, or the response URL when
/// there is none (EC-PH-16).
pub fn document_base(document: &Html, response_url: &Url) -> Url {
    let Ok(selector) = Selector::parse("base[href]") else {
        return response_url.clone();
    };
    document
        .select(&selector)
        .next()
        .and_then(|base| base.value().attr("href"))
        .and_then(|href| response_url.join(href).ok())
        .unwrap_or_else(|| response_url.clone())
}

/// Resolves `href` against `base` (Jsoup `abs:`). `None` when the result is not a URL.
pub fn resolve(base: &Url, href: &str) -> Option<String> {
    base.join(href.trim()).ok().map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(html: &str) -> Html {
        Html::parse_fragment(html)
    }

    fn first<'a>(doc: &'a Html, css: &str) -> ElementRef<'a> {
        doc.select(&Selector::parse(css).unwrap()).next().unwrap()
    }

    #[test]
    fn text_collapses_ascii_whitespace_and_nbsp_and_trims() {
        let doc = document("<p>  A \u{00A0} B \n\t C  </p>");
        assert_eq!(text(first(&doc, "p")), "A B C");
        assert_eq!(normalise_whitespace(" \u{00A0} "), "");
    }

    #[test]
    fn select_text_is_none_for_no_match_and_empty_for_an_empty_match() {
        let doc = document("<div><span></span></div>");
        let div = first(&doc, "div");
        assert_eq!(select_text(div, ".missing").unwrap(), None);
        assert_eq!(select_text(div, "span").unwrap(), Some(String::new()));
    }

    #[test]
    fn select_int_reads_plain_and_signed_numbers_and_defaults_on_no_match() {
        let doc = document("<div><b>12</b><i>+7</i><u></u><s>abc</s></div>");
        let div = first(&doc, "div");
        assert_eq!(select_int(div, "b", 0).unwrap(), 12);
        assert_eq!(select_int(div, "i", 0).unwrap(), 7);
        assert_eq!(select_int(div, ".none", 5).unwrap(), 5);
        assert!(matches!(select_int(div, "u", 0), Err(SourceError::Parse(_))));
        assert!(matches!(select_int(div, "s", 0), Err(SourceError::Parse(_))));
    }

    #[test]
    fn attr_or_text_reads_the_element_itself_and_never_a_child() {
        let doc = document(r#"<div><a href="/x">Go</a></div>"#);
        let div = first(&doc, "div");
        assert_eq!(attr_or_text(div, "href"), "");
        let link = first(&doc, "a");
        assert_eq!(attr_or_text(link, "href"), "/x");
        assert_eq!(attr_or_text(link, "text"), "Go");
    }

    #[test]
    fn an_invalid_selector_is_a_parse_error() {
        let doc = document("<p>x</p>");
        assert!(matches!(select_text(first(&doc, "p"), "p[[["), Err(SourceError::Parse(_))));
    }

    #[test]
    fn a_base_element_overrides_the_response_url_for_relative_links() {
        let doc = Html::parse_document(r#"<html><head><base href="https://cdn.test/img/"></head><body></body></html>"#);
        let response = Url::parse("https://a.test/page/1").unwrap();
        let base = document_base(&doc, &response);
        assert_eq!(resolve(&base, "1.jpg").as_deref(), Some("https://cdn.test/img/1.jpg"));
    }

    #[test]
    fn without_a_base_element_relative_links_resolve_against_the_response() {
        let doc = Html::parse_document("<html><body></body></html>");
        let response = Url::parse("https://a.test/page/1").unwrap();
        let base = document_base(&doc, &response);
        assert_eq!(resolve(&base, "../2.jpg").as_deref(), Some("https://a.test/2.jpg"));
    }
}
