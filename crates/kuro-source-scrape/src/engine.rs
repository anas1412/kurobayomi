//! The operations of a recipe as pure functions over an HTML string and the address it came from. Nothing here touches
//! the network, so the tests and `check_recipe` run them over saved pages.

use chrono::{NaiveDate, NaiveDateTime};
use kuro_source::SourceError;
use kuro_source_http::html::{document_base, normalise_whitespace, resolve};
use kuro_source_http::urls::url_without_domain;
use scraper::{ElementRef, Html, Node};
use url::Url;

use crate::recipe::{Chapters, Details, Listing, Pages, Pick, Videos};

/// One entry of a listing. `url` is the stored form (path and query, no host).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub url: String,
    pub title: String,
    pub cover: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListPage {
    pub entries: Vec<Entry>,
    pub has_next: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub title: Option<String>,
    pub author: Option<String>,
    pub artist: Option<String>,
    pub description: Option<String>,
    pub genre: Option<String>,
    pub cover: Option<String>,
    /// The `SManga.status` code, when the recipe maps one.
    pub status: Option<i32>,
}

/// A chapter or an episode. `url` is the stored form, `date` is epoch milliseconds when the recipe reads one.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub url: String,
    pub name: String,
    pub number: Option<f32>,
    pub date: Option<i64>,
    pub scanlator: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoLink {
    pub url: String,
    pub title: String,
}

fn parse_error(text: String) -> SourceError {
    SourceError::Parse(text)
}

/// The values of a pick below `scope`, in document order.
fn values(pick: &Pick, scope: ElementRef<'_>) -> Vec<String> {
    let matched: Vec<ElementRef<'_>> = match &pick.select {
        Some(css) => scope.select(css.selector()).collect(),
        None => vec![scope],
    };
    let chosen: Vec<ElementRef<'_>> = if pick.all { matched } else { matched.into_iter().nth(pick.nth).into_iter().collect() };
    chosen.into_iter().filter_map(|element| read(pick, element)).collect()
}

/// Elements that Jsoup's `text()` puts a space around, so `<p>a</p><p>b</p>` reads `a b` and not `ab`.
const BREAKS: &[&str] = &[
    "address", "article", "aside", "blockquote", "br", "dd", "details", "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form", "h1", "h2",
    "h3", "h4", "h5", "h6", "header", "hr", "li", "main", "nav", "ol", "p", "pre", "section", "table", "td", "th", "tr", "ul",
];

/// The text of an element, whitespace collapsed, with a space at block and `<br>` boundaries.
fn spaced_text(element: ElementRef<'_>) -> String {
    let mut out = String::new();
    for node in element.descendants() {
        match node.value() {
            Node::Text(text) => out.push_str(text),
            Node::Element(e) if BREAKS.contains(&e.name()) => out.push(' '),
            _ => {}
        }
    }
    normalise_whitespace(&out)
}

fn read(pick: &Pick, element: ElementRef<'_>) -> Option<String> {
    let raw = match &pick.attr {
        Some(name) => element.value().attr(name)?.to_owned(),
        None => spaced_text(element),
    };
    let value = match &pick.regex {
        Some(rx) => {
            let captures = rx.0.captures(&raw)?;
            let group = if captures.len() > 1 { 1 } else { 0 };
            captures.get(group)?.as_str().to_owned()
        }
        None => raw,
    };
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    Some(match &pick.format {
        Some(format) => format.replace("{}", value),
        None => value.to_owned(),
    })
}

/// The value of a pick: the first (or `nth`) match, or all matches joined when `all` is set.
fn one(pick: &Pick, scope: ElementRef<'_>) -> Option<String> {
    let mut found = values(pick, scope);
    if pick.all {
        (!found.is_empty()).then(|| found.join(pick.join.as_deref().unwrap_or(", ")))
    } else {
        found.drain(..).next()
    }
}

fn links(pick: &Pick, scope: ElementRef<'_>, base: &Url) -> Vec<String> {
    values(pick, scope).iter().filter_map(|value| resolve(base, value)).collect()
}

fn link(pick: &Pick, scope: ElementRef<'_>, base: &Url) -> Option<String> {
    links(pick, scope, base).into_iter().next()
}

fn need(what: &str, item: usize, value: Option<String>) -> Result<String, SourceError> {
    value.ok_or_else(|| parse_error(format!("{what}: nothing matched (item {item})")))
}

fn stored(absolute: &str) -> String {
    url_without_domain(absolute)
}

/// A listing page. One item without a title or a link fails the page, so a bad selector is loud.
pub fn listing(op: &Listing, html: &str, page_url: &Url, page: i32) -> Result<ListPage, SourceError> {
    let document = Html::parse_document(html);
    let base = document_base(&document, page_url);
    let mut entries = Vec::new();
    for (index, item) in document.select(op.items.selector()).enumerate() {
        let title = need("title", index, one(&op.title, item))?;
        let url = need("url", index, link(&op.url, item, &base))?;
        let cover = op.cover.as_ref().and_then(|pick| link(pick, item, &base));
        entries.push(Entry { url: stored(&url), title, cover });
    }
    let root = document.root_element();
    let has_next = if let Some(next) = &op.next_page {
        document.select(next.selector()).next().is_some()
    } else if let Some(last) = &op.last_page {
        match one(last, root) {
            None => false,
            Some(raw) => {
                let last: i32 = raw.parse().map_err(|_| parse_error(format!("last_page: {raw:?} is not a number")))?;
                page < last
            }
        }
    } else {
        false
    };
    Ok(ListPage { entries, has_next })
}

/// The details of a manga or an anime. Whatever the recipe does not name stays `None`.
pub fn details(op: &Details, html: &str, page_url: &Url) -> Result<Found, SourceError> {
    let document = Html::parse_document(html);
    let base = document_base(&document, page_url);
    let root = document.root_element();
    let text_of = |pick: &Option<Pick>| pick.as_ref().and_then(|p| one(p, root));
    let status = op.status.as_ref().and_then(|pick| one(pick, root)).map(|raw| status_of(op, &raw));
    Ok(Found {
        title: text_of(&op.title),
        author: text_of(&op.author),
        artist: text_of(&op.artist),
        description: text_of(&op.description),
        genre: text_of(&op.genre),
        cover: op.cover.as_ref().and_then(|pick| link(pick, root, &base)),
        status,
    })
}

/// The first status (in the order of the table's keys) with a word that occurs in the text. No word: unknown.
fn status_of(op: &Details, raw: &str) -> i32 {
    let raw = raw.to_lowercase();
    op.status_map
        .iter()
        .find(|(_, words)| words.iter().any(|word| raw.contains(&word.to_lowercase())))
        .map_or(0, |(status, _)| status.code())
}

/// The chapters of a manga or the episodes of an anime, newest first.
pub fn chapters(op: &Chapters, html: &str, page_url: &Url) -> Result<Vec<Item>, SourceError> {
    let document = Html::parse_document(html);
    let base = document_base(&document, page_url);
    let mut found = Vec::new();
    for (index, element) in document.select(op.items.selector()).enumerate() {
        let name = need("name", index, one(&op.name, element))?;
        let url = need("url", index, link(&op.url, element, &base))?;
        let number = op.number.as_ref().and_then(|pick| one(pick, element)).and_then(|raw| raw.parse::<f32>().ok());
        let date = match (&op.date, &op.date_format) {
            (Some(pick), Some(format)) => one(pick, element).and_then(|raw| parse_date(&raw, format)),
            _ => None,
        };
        let scanlator = op.scanlator.as_ref().and_then(|pick| one(pick, element));
        found.push(Item { url: stored(&url), name, number, date, scanlator });
    }
    if op.reverse {
        found.reverse();
    }
    Ok(found)
}

/// Epoch milliseconds of a date text in UTC. `None` when the text does not fit the format.
pub fn parse_date(raw: &str, format: &str) -> Option<i64> {
    let raw = raw.trim();
    match format {
        "unix" => raw.parse::<i64>().ok().map(|s| s.saturating_mul(1000)),
        "unix_ms" => raw.parse().ok(),
        _ => NaiveDateTime::parse_from_str(raw, format)
            .ok()
            .or_else(|| NaiveDate::parse_from_str(raw, format).ok().and_then(|d| d.and_hms_opt(0, 0, 0)))
            .map(|d| d.and_utc().timestamp_millis()),
    }
}

/// The image addresses of a chapter, in order.
pub fn pages(op: &Pages, html: &str, page_url: &Url) -> Result<Vec<String>, SourceError> {
    let document = Html::parse_document(html);
    let base = document_base(&document, page_url);
    let mut found = Vec::new();
    for (index, element) in document.select(op.items.selector()).enumerate() {
        found.push(need("image", index, link(&op.image, element, &base))?);
    }
    if found.is_empty() {
        return Err(SourceError::EmptyPageList);
    }
    Ok(found)
}

/// The direct video addresses of an episode page (a file or an HLS playlist, as the page gives them).
pub fn videos(op: &Videos, html: &str, page_url: &Url) -> Result<Vec<VideoLink>, SourceError> {
    let document = Html::parse_document(html);
    let base = document_base(&document, page_url);
    let default_title = op.default_title.as_deref().unwrap_or("Video");
    let mut found = Vec::new();
    let mut take = |scope: ElementRef<'_>| {
        let title = op.title.as_ref().and_then(|pick| one(pick, scope)).unwrap_or_else(|| default_title.to_owned());
        found.extend(links(&op.url, scope, &base).into_iter().map(|url| VideoLink { url, title: title.clone() }));
    };
    match &op.items {
        Some(items) => document.select(items.selector()).for_each(&mut take),
        None => take(document.root_element()),
    }
    if found.is_empty() {
        return Err(parse_error("no video address found".into()));
    }
    Ok(found)
}
