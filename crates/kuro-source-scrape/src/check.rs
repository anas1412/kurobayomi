//! The recipe checker behind `examples/check_recipe.rs`: runs every operation of a recipe in the order a user meets
//! them (listings, then the first entry's details and chapters or episodes, then the first chapter's pages or the
//! first episode's videos) and says what each one returned. A [`Feed`] supplies the pages: saved fixtures, or the
//! real site.

use kuro_platform::kind::Kind;
use std::path::PathBuf;

use async_trait::async_trait;
use url::Url;

use crate::engine;
use crate::fetch::Fetcher;
use crate::recipe::Recipe;

#[async_trait]
pub trait Feed: Send {
    /// A live feed cannot continue without a first entry; a fixture feed can.
    fn live(&self) -> bool;
    /// The page of an operation. `url` is the address the app would request for it.
    async fn page(&mut self, op: &str, url: &Url) -> Result<(String, Url), String>;
}

/// The pages named in the recipe's `[tests]` table, read from `dir`.
pub struct FixtureFeed {
    pub dir: PathBuf,
    pub files: Vec<(&'static str, Option<String>)>,
}

impl FixtureFeed {
    pub fn new(recipe: &Recipe, dir: PathBuf) -> Self {
        let t = &recipe.tests;
        let files = vec![
            ("popular", t.popular.clone()),
            ("latest", t.latest.clone()),
            ("search", t.search.clone()),
            ("details", t.details.clone()),
            ("chapters", t.chapters.clone()),
            ("pages", t.pages.clone()),
            ("episodes", t.episodes.clone()),
            ("videos", t.videos.clone()),
        ];
        Self { dir, files }
    }
}

#[async_trait]
impl Feed for FixtureFeed {
    fn live(&self) -> bool {
        false
    }

    async fn page(&mut self, op: &str, url: &Url) -> Result<(String, Url), String> {
        let name = self.files.iter().find(|(key, _)| *key == op).and_then(|(_, file)| file.clone());
        let name = name.ok_or_else(|| format!("no fixture named for `{op}` in [tests]"))?;
        let path = self.dir.join(&name);
        let html = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        Ok((html, url.clone()))
    }
}

/// The real site, through the same fetcher the source uses.
pub struct LiveFeed(pub Fetcher);

#[async_trait]
impl Feed for LiveFeed {
    fn live(&self) -> bool {
        true
    }

    async fn page(&mut self, _op: &str, url: &Url) -> Result<(String, Url), String> {
        self.0.get(url).await.map_err(|e| e.to_string())
    }
}

/// The recipe as a live check runs it: at least one second between requests and a plain user agent.
pub fn polite(mut recipe: Recipe) -> Recipe {
    recipe.rate_limit_ms = recipe.rate_limit_ms.max(1000);
    recipe.headers.retain(|name, _| !name.eq_ignore_ascii_case("user-agent"));
    recipe.headers.insert("User-Agent".into(), "KurobayomiRecipeCheck/0.1 (one request at a time)".into());
    recipe
}

#[derive(Debug, Clone)]
pub struct Line {
    pub op: &'static str,
    pub ok: bool,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Report {
    pub lines: Vec<Line>,
}

impl Report {
    /// An operation errored, returned nothing, or had no page to run on.
    pub fn failed(&self) -> bool {
        self.lines.iter().any(|line| !line.ok)
    }

    fn add(&mut self, op: &'static str, outcome: Result<String, String>) {
        let (ok, text) = match outcome {
            Ok(text) => (true, text),
            Err(text) => (false, text),
        };
        self.lines.push(Line { op, ok, text });
    }
}

impl std::fmt::Display for Report {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for line in &self.lines {
            writeln!(f, "{} {}: {}", if line.ok { "ok  " } else { "FAIL" }, line.op, line.text)?;
        }
        Ok(())
    }
}

fn clip(text: &str) -> String {
    let flat = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 70 { format!("{}...", flat.chars().take(70).collect::<String>()) } else { flat }
}

fn shown<T>(heading: String, rows: &[T], line: impl Fn(&T) -> String) -> Result<String, String> {
    shown_up_to(3, heading, rows, line)
}

fn shown_up_to<T>(limit: usize, heading: String, rows: &[T], line: impl Fn(&T) -> String) -> Result<String, String> {
    if rows.is_empty() {
        return Err("returned nothing".into());
    }
    let mut text = heading;
    for row in rows.iter().take(limit) {
        text.push_str(&format!("\n       {}", line(row)));
    }
    if rows.len() > limit {
        text.push_str(&format!("\n       ... and {} more", rows.len() - limit));
    }
    Ok(text)
}

/// Fails the line when a configured part of the recipe (`(name, configured, seen)`) found nothing, so a selector, a
/// regex or a date format that matches nothing cannot hide behind an operation that otherwise returned something.
fn dead(text: String, parts: &[(&str, bool, bool)], what: &str) -> Result<String, String> {
    let names: Vec<&str> = parts.iter().filter(|(_, configured, seen)| *configured && !*seen).map(|(name, ..)| *name).collect();
    if names.is_empty() { Ok(text) } else { Err(format!("{text}\n       {what}: {}", names.join(", "))) }
}

/// Runs every operation of the recipe. The first entry of `popular` and the first chapter or episode are followed on.
pub async fn run(recipe: &Recipe, feed: &mut dyn Feed) -> Report {
    let mut report = Report::default();
    let mut first_entry: Option<String> = None;
    let query = recipe.tests.query.as_deref().unwrap_or("a");
    let listings = [
        ("popular", Some(&recipe.popular), Some(&recipe.urls.popular)),
        ("latest", recipe.latest.as_ref(), recipe.urls.latest.as_ref()),
        ("search", recipe.search.as_ref(), recipe.urls.search.as_ref()),
    ];
    for (op, listing, template) in listings {
        let (Some(listing), Some(template)) = (listing, template) else { continue };
        let outcome = async {
            let url = recipe.listing_url(template, 1, query).map_err(|e| e.to_string())?;
            let (html, at) = feed.page(op, &url).await?;
            let page = engine::listing(listing, &html, &at, 1).map_err(|e| e.to_string())?;
            if op == "popular" {
                first_entry = page.entries.first().map(|e| e.url.clone());
            }
            let heading = format!("{} entries, next page: {}", page.entries.len(), if page.has_next { "yes" } else { "no" });
            shown(heading, &page.entries, |e| format!("{} -> {}{}", clip(&e.title), e.url, if e.cover.is_some() { " (cover)" } else { "" }))
        }
        .await;
        report.add(op, outcome);
    }

    let Some(entry) = entry_address(recipe, feed, &first_entry, &mut report) else { return report };
    let outcome = async {
        let (html, at) = feed.page("details", &entry).await?;
        let found = engine::details(&recipe.details, &html, &at).map_err(|e| e.to_string())?;
        let fields = [
            ("title", &found.title),
            ("author", &found.author),
            ("artist", &found.artist),
            ("description", &found.description),
            ("genre", &found.genre),
            ("cover", &found.cover),
        ];
        let mut rows: Vec<String> = fields.iter().filter_map(|(k, v)| v.as_ref().map(|v| format!("{k}: {}", clip(v)))).collect();
        rows.extend(found.status.map(|s| format!("status: {s}")));
        let d = &recipe.details;
        // a status text that matches no word of the map is 0 (unknown), which is as good as nothing found
        let status_found = found.status.is_some_and(|s| s != 0);
        let seen = [
            ("title", d.title.is_some(), found.title.is_some()),
            ("author", d.author.is_some(), found.author.is_some()),
            ("artist", d.artist.is_some(), found.artist.is_some()),
            ("description", d.description.is_some(), found.description.is_some()),
            ("genre", d.genre.is_some(), found.genre.is_some()),
            ("cover", d.cover.is_some(), found.cover.is_some()),
            ("status", d.status.is_some(), status_found),
        ];
        let text = shown_up_to(8, "found:".into(), &rows, |row| row.clone())?;
        dead(text, &seen, "found nothing")
    }
    .await;
    report.add("details", outcome);

    let (list_op, follow_op, items_op) = match recipe.kind {
        Kind::Manga => ("chapters", "pages", recipe.chapters.as_ref()),
        Kind::Anime => ("episodes", "videos", recipe.episodes.as_ref()),
    };
    let mut first_item: Option<String> = None;
    if let Some(items_op) = items_op {
        let outcome = async {
            let (html, at) = feed.page(list_op, &entry).await?;
            let items = engine::chapters(items_op, &html, &at).map_err(|e| e.to_string())?;
            first_item = items.first().map(|i| i.url.clone());
            if let (Some(rx), Some(first)) = (&recipe.tests.item_url, &first_item) {
                if !rx.0.is_match(first) {
                    return Err(format!("first address {first} does not match tests.item_url {}", rx.0.as_str()));
                }
            }
            let seen = [
                ("number", items_op.number.is_some(), items.iter().any(|i| i.number.is_some())),
                ("date", items_op.date.is_some(), items.iter().any(|i| i.date.is_some())),
                ("scanlator", items_op.scanlator.is_some(), items.iter().any(|i| i.scanlator.is_some())),
            ];
            let text = shown(format!("{} found", items.len()), &items, |i| {
                let date = i.date.map_or(String::new(), |d| format!(", date {d}"));
                let number = i.number.map_or(String::new(), |n| format!(", number {n}"));
                format!("{} -> {}{number}{date}", clip(&i.name), i.url)
            })?;
            dead(text, &seen, "found on none of the items")
        }
        .await;
        report.add(list_op, outcome);
    }

    let Some(item) = entry_address(recipe, feed, &first_item, &mut report) else { return report };
    let outcome = async {
        let (html, at) = feed.page(follow_op, &item).await?;
        match recipe.kind {
            Kind::Manga => {
                let op = recipe.pages.as_ref().ok_or("no pages operation")?;
                let pages = engine::pages(op, &html, &at).map_err(|e| e.to_string())?;
                shown(format!("{} images", pages.len()), &pages, |p| p.clone())
            }
            Kind::Anime => {
                let op = recipe.videos.as_ref().ok_or("no videos operation")?;
                let videos = engine::videos(op, &html, &at).map_err(|e| e.to_string())?;
                shown(format!("{} videos", videos.len()), &videos, |v| format!("{}: {}", clip(&v.title), v.url))
            }
        }
    }
    .await;
    report.add(follow_op, outcome);
    report
}

/// The address to run the next operation on: the followed entry. Without one a live check stops (and says so), and a
/// fixture check uses the site root, since the fixture does not depend on the address.
fn entry_address(recipe: &Recipe, feed: &dyn Feed, followed: &Option<String>, report: &mut Report) -> Option<Url> {
    match followed {
        Some(stored) => recipe.address(stored).ok(),
        None if feed.live() => {
            report.add("follow-up", Err("skipped: the step before returned nothing to follow".into()));
            None
        }
        None => recipe.address("/").ok(),
    }
}
