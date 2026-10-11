//! The recipe format: a TOML text of URL templates and CSS selectors, no code (`docs/source-recipes.md`).
//!
//! Every struct rejects unknown keys. A selector or a regex that does not compile is rejected while the text is read,
//! and the rest (hosts, which operations a kind needs) in [`Recipe::parse`]. After that a [`Recipe`] cannot fail on its
//! own text, so the operations in `engine` only meet page problems.

use std::collections::BTreeMap;
use std::path::Path;

use http::{HeaderName, HeaderValue};
use regex::Regex;
use scraper::Selector;
use serde::{Deserialize, Deserializer, de};
use url::Url;

/// Why a recipe was not accepted. The text names the key.
#[derive(Debug, thiserror::Error)]
pub enum RecipeError {
    #[error("cannot read {path}: {message}")]
    Read { path: String, message: String },
    #[error("{0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Manga,
    Anime,
}

/// A CSS selector, parsed while the recipe is read.
#[derive(Debug, Clone)]
pub struct Css {
    raw: String,
    selector: Selector,
}

impl Css {
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn selector(&self) -> &Selector {
        &self.selector
    }
}

impl<'de> Deserialize<'de> for Css {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let selector = Selector::parse(&raw).map_err(|e| de::Error::custom(format!("bad selector {raw:?}: {e}")))?;
        Ok(Self { raw, selector })
    }
}

/// A regular expression (the `regex` crate: no look-around), compiled while the recipe is read.
#[derive(Debug, Clone)]
pub struct Rx(pub Regex);

impl<'de> Deserialize<'de> for Rx {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Regex::new(&raw).map(Rx).map_err(|e| de::Error::custom(format!("bad regex {raw:?}: {e}")))
    }
}

/// A URL template. `{page}` and `{query}` are the only placeholders.
#[derive(Debug, Clone)]
pub struct Template(String);

impl Template {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for Template {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        let mut rest = raw.as_str();
        while let Some(open) = rest.find(['{', '}']) {
            let tail = &rest[open..];
            let name = ["{page}", "{query}"].into_iter().find(|p| tail.starts_with(p));
            match name {
                Some(name) => rest = &tail[name.len()..],
                None => return Err(de::Error::custom(format!("URL template {raw:?}: only {{page}} and {{query}} are allowed"))),
            }
        }
        if raw.is_empty() {
            return Err(de::Error::custom("URL template is empty"));
        }
        Ok(Self(raw))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PickRaw {
    select: Option<Css>,
    attr: Option<String>,
    regex: Option<Rx>,
    #[serde(default)]
    nth: usize,
    #[serde(default)]
    all: bool,
    join: Option<String>,
    format: Option<String>,
}

/// Where one value comes from: the elements `select` matches (the scope element itself when it is absent), the `nth`
/// match (0 is the first) or all of them, the text or the `attr` attribute, then the first group of `regex` (or the whole
/// match when it has no group), then `format` (`{}` is replaced by the value). An empty value counts as no match.
#[derive(Debug, Clone, Deserialize)]
#[serde(try_from = "PickRaw")]
pub struct Pick {
    pub select: Option<Css>,
    pub attr: Option<String>,
    pub regex: Option<Rx>,
    pub nth: usize,
    pub all: bool,
    pub join: Option<String>,
    pub format: Option<String>,
}

impl TryFrom<PickRaw> for Pick {
    type Error = String;

    fn try_from(raw: PickRaw) -> Result<Self, String> {
        if raw.all && raw.nth > 0 {
            return Err("`all` and `nth` cannot be used together".into());
        }
        if raw.nth > 0 && raw.select.is_none() {
            return Err("`nth` needs a `select`".into());
        }
        if raw.join.is_some() && !raw.all {
            return Err("`join` needs `all = true`".into());
        }
        if raw.format.as_deref().is_some_and(|f| !f.contains("{}")) {
            return Err("`format` needs a `{}` for the value".into());
        }
        Ok(Self { select: raw.select, attr: raw.attr, regex: raw.regex, nth: raw.nth, all: raw.all, join: raw.join, format: raw.format })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Listing {
    pub items: Css,
    pub title: Pick,
    pub url: Pick,
    pub cover: Option<Pick>,
    /// The next page exists when this matches anything.
    pub next_page: Option<Css>,
    /// The last page number: the next page exists when the current page is below it.
    pub last_page: Option<Pick>,
}

/// The order of the variants is the tie-break when several `status_map` entries match (`docs/source-recipes.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Unknown,
    Ongoing,
    Completed,
    Licensed,
    PublishingFinished,
    Cancelled,
    OnHiatus,
}

impl Status {
    /// The value the source API stores (`SManga.status`).
    pub fn code(self) -> i32 {
        match self {
            Self::Unknown => 0,
            Self::Ongoing => 1,
            Self::Completed => 2,
            Self::Licensed => 3,
            Self::PublishingFinished => 4,
            Self::Cancelled => 5,
            Self::OnHiatus => 6,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Details {
    pub title: Option<Pick>,
    pub author: Option<Pick>,
    pub artist: Option<Pick>,
    pub description: Option<Pick>,
    pub genre: Option<Pick>,
    pub cover: Option<Pick>,
    pub status: Option<Pick>,
    /// Status name to the words that mean it (lower case, matched anywhere in the status text).
    #[serde(default)]
    pub status_map: BTreeMap<Status, Vec<String>>,
}

/// The chapters of a manga or the episodes of an anime.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chapters {
    pub items: Css,
    pub name: Pick,
    pub url: Pick,
    pub number: Option<Pick>,
    pub date: Option<Pick>,
    /// A chrono format (`%B %d, %Y`), or `unix` (seconds) or `unix_ms`.
    pub date_format: Option<String>,
    pub scanlator: Option<Pick>,
    /// The site lists the oldest first. The source API wants the newest first.
    #[serde(default)]
    pub reverse: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pages {
    pub items: Css,
    pub image: Pick,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Videos {
    /// One video per element. Without it `url` is read from the whole page, and `all = true` gives several videos.
    pub items: Option<Css>,
    pub url: Pick,
    pub title: Option<Pick>,
    pub default_title: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Urls {
    pub popular: Template,
    pub latest: Option<Template>,
    pub search: Option<Template>,
}

/// Fixture file of each operation, for `check_recipe`, and the text its live search uses.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tests {
    pub popular: Option<String>,
    pub latest: Option<String>,
    pub search: Option<String>,
    pub details: Option<String>,
    pub chapters: Option<String>,
    pub pages: Option<String>,
    pub episodes: Option<String>,
    pub videos: Option<String>,
    pub query: Option<String>,
    /// The checker fails when the first chapter or episode address does not match this regex (the expected shape).
    pub item_url: Option<Rx>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    pub id: String,
    pub name: String,
    pub lang: String,
    pub kind: Kind,
    /// Part of the source id (`generate_id`). Raise it to make the app treat the source as a new one.
    #[serde(default = "one")]
    pub version: i32,
    /// The only hosts the recipe may request. The first one is the site root that stored links are joined to.
    pub base_urls: Vec<String>,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    /// The least time between two requests of this source, in milliseconds.
    #[serde(default)]
    pub rate_limit_ms: u64,
    pub urls: Urls,
    pub popular: Listing,
    pub latest: Option<Listing>,
    pub search: Option<Listing>,
    pub details: Details,
    pub chapters: Option<Chapters>,
    pub pages: Option<Pages>,
    pub episodes: Option<Chapters>,
    pub videos: Option<Videos>,
    #[serde(default)]
    pub tests: Tests,
}

fn one() -> i32 {
    1
}

fn invalid<T>(text: impl Into<String>) -> Result<T, RecipeError> {
    Err(RecipeError::Invalid(text.into()))
}

impl Recipe {
    /// Reads and checks a recipe text.
    pub fn parse(text: &str) -> Result<Self, RecipeError> {
        let recipe: Recipe = toml_edit::de::from_str(text).map_err(|e| RecipeError::Invalid(e.to_string()))?;
        recipe.check()?;
        Ok(recipe)
    }

    pub fn load(path: &Path) -> Result<Self, RecipeError> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| RecipeError::Read { path: path.display().to_string(), message: e.to_string() })?;
        Self::parse(&text).map_err(|e| match e {
            RecipeError::Invalid(message) => RecipeError::Invalid(format!("{}: {message}", path.display())),
            other => other,
        })
    }

    fn check(&self) -> Result<(), RecipeError> {
        let word = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !word(&self.id) {
            return invalid("id: use letters, digits, `-` and `_`");
        }
        if self.name.trim().is_empty() {
            return invalid("name is empty");
        }
        if !word(&self.lang) {
            return invalid("lang: use a language code such as `en`, or `all`");
        }
        if self.base_urls.is_empty() {
            return invalid("base_urls: list at least one site");
        }
        for base in &self.base_urls {
            let url = Url::parse(base).map_err(|e| RecipeError::Invalid(format!("base_urls: {base:?}: {e}")))?;
            if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
                return invalid(format!("base_urls: {base:?} is not an http or https address"));
            }
        }
        for (name, value) in &self.headers {
            let parsed = (HeaderName::from_bytes(name.as_bytes()), HeaderValue::from_str(value));
            if parsed.0.is_err() || parsed.1.is_err() {
                return invalid(format!("headers: {name:?} is not a valid header"));
            }
            if name.eq_ignore_ascii_case("host") {
                return invalid("headers: `Host` cannot be set");
            }
        }
        if self.rate_limit_ms > 60_000 {
            return invalid("rate_limit_ms: at most 60000");
        }
        if self.urls.latest.is_some() != self.latest.is_some() {
            return invalid("`latest` and `urls.latest` go together");
        }
        if self.urls.search.is_some() != self.search.is_some() {
            return invalid("`search` and `urls.search` go together");
        }
        self.check_templates()?;
        for (name, listing) in [("popular", Some(&self.popular)), ("latest", self.latest.as_ref()), ("search", self.search.as_ref())] {
            if let Some(listing) = listing {
                if listing.next_page.is_some() && listing.last_page.is_some() {
                    return invalid(format!("{name}: use `next_page` or `last_page`, not both"));
                }
            }
        }
        if self.details.status.is_some() == self.details.status_map.is_empty() {
            return invalid("details: `status` and `status_map` go together");
        }
        for (what, chapters) in [("chapters", &self.chapters), ("episodes", &self.episodes)] {
            if let Some(chapters) = chapters {
                if chapters.date.is_some() != chapters.date_format.is_some() {
                    return invalid(format!("{what}: `date` and `date_format` go together"));
                }
            }
        }
        let (needs, forbids): (&[(&str, bool)], &[(&str, bool)]) = match self.kind {
            Kind::Manga => (
                &[("chapters", self.chapters.is_some()), ("pages", self.pages.is_some())],
                &[("episodes", self.episodes.is_some()), ("videos", self.videos.is_some())],
            ),
            Kind::Anime => (
                &[("episodes", self.episodes.is_some()), ("videos", self.videos.is_some())],
                &[("chapters", self.chapters.is_some()), ("pages", self.pages.is_some())],
            ),
        };
        if let Some((name, _)) = needs.iter().find(|(_, present)| !present) {
            return invalid(format!("a {:?} recipe needs `{name}`", self.kind));
        }
        if let Some((name, _)) = forbids.iter().find(|(_, present)| *present) {
            return invalid(format!("`{name}` does not belong in a {:?} recipe", self.kind));
        }
        Ok(())
    }

    /// An absolute template must name a host of `base_urls`. A relative one is joined to the first base.
    fn check_templates(&self) -> Result<(), RecipeError> {
        let templates = [Some(&self.urls.popular), self.urls.latest.as_ref(), self.urls.search.as_ref()];
        for template in templates.into_iter().flatten() {
            let text = template.as_str();
            if text.starts_with('/') {
                continue;
            }
            let probe = text.replace("{page}", "1").replace("{query}", "x");
            let url = Url::parse(&probe).map_err(|_| RecipeError::Invalid(format!("urls: {text:?} must start with `/` or be a full address")))?;
            self.check_host(&url).map_err(|e| RecipeError::Invalid(format!("urls: {e}")))?;
        }
        Ok(())
    }

    /// The site root, without a trailing slash.
    pub fn base(&self) -> &str {
        self.base_urls[0].trim_end_matches('/')
    }

    /// Whether the recipe may request this address: http or https, and a host listed in `base_urls`.
    pub fn check_host(&self, url: &Url) -> Result<(), RecipeError> {
        let host = url.host_str().unwrap_or("");
        let listed = self.base_urls.iter().filter_map(|b| Url::parse(b).ok()).any(|b| b.host_str() == Some(host));
        if matches!(url.scheme(), "http" | "https") && listed {
            Ok(())
        } else {
            Err(RecipeError::Invalid(format!("{url} is outside base_urls")))
        }
    }

    /// The request address of a listing template.
    pub fn listing_url(&self, template: &Template, page: i32, query: &str) -> Result<Url, RecipeError> {
        let encoded = url::form_urlencoded::byte_serialize(query.as_bytes()).collect::<String>().replace('+', "%20");
        let text = template.as_str().replace("{page}", &page.to_string()).replace("{query}", &encoded);
        self.address(&text)
    }

    /// The request address of a stored link (`SManga.url`, `SChapter.url`): joined to the first base unless it is
    /// already a full address.
    pub fn address(&self, stored: &str) -> Result<Url, RecipeError> {
        let full = if stored.starts_with("http://") || stored.starts_with("https://") {
            stored.to_owned()
        } else if stored.starts_with('/') {
            format!("{}{stored}", self.base())
        } else {
            format!("{}/{stored}", self.base())
        };
        let url = Url::parse(&full).map_err(|e| RecipeError::Invalid(format!("{full:?}: {e}")))?;
        self.check_host(&url)?;
        Ok(url)
    }
}
