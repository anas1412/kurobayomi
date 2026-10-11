//! Extension repositories, the link flow of `docs/design/app-flow.md`. A repository is one link to an `index.min.json`
//! that lists its sources. Adding the link makes the sources available at once; each source's file is fetched on first
//! use and checked against the sha256 in the index. The daily check re-reads the index (see [`refresh_due`]).
//!
//! A source's file is a recipe (`.toml`, `docs/source-recipes.md`) or a plug-in component (`.wasm`, `wit/kuro-source.wit`).
//! The index is read here; the GTK side stores the links and the index text, and decides where a fetched file goes.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::anime::update::next_delay;

/// The most sources one repository may list for one kind. A user has at most this many of each.
pub const MAX_SOURCES_PER_KIND: usize = 10;
/// The largest index or source file accepted. Both are small text files.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Manga,
    Anime,
}

/// One source a repository lists. `id` names the file the source is saved under, so it is limited to lower-case letters,
/// digits, `.`, `_` and `-`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
    pub lang: String,
    pub kind: Kind,
    pub version: u32,
    /// The https link of the source file.
    pub file: String,
    /// The SHA-256 of the file, in hex.
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    pub sources: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepoError {
    #[error("the index could not be read: {0}")]
    Parse(String),
    #[error("the index lists {count} {kind:?} sources, the limit is {limit}")]
    TooMany { kind: Kind, count: usize, limit: usize },
    #[error("the source id {0:?} is listed twice or is not a plain name")]
    BadId(String),
    #[error("the source {0:?} has a sha256 that is not 64 hex digits")]
    BadChecksum(String),
    #[error("the source {0:?} has a file link that is not an https .toml or .wasm file")]
    BadLink(String),
    #[error("the repository link must be https")]
    BadRepoLink,
    #[error("the file is larger than 1 MiB")]
    TooLarge,
    #[error("the file could not be reached: {0}")]
    Network(String),
    #[error("the server answered HTTP {0}")]
    Status(u16),
    #[error("the file does not match the checksum in the index")]
    Mismatch,
    #[error("the file could not be saved: {0}")]
    Write(String),
}

/// Reads an index and checks it: each kind has at most [`MAX_SOURCES_PER_KIND`] sources, ids are unique and plain, the
/// checksums are 64 hex digits, and the file links are https `.toml` files.
pub fn parse_index(text: &str) -> Result<Index, RepoError> {
    let index: Index = serde_json::from_str(text).map_err(|e| RepoError::Parse(e.to_string()))?;
    check(&index)?;
    Ok(index)
}

fn check(index: &Index) -> Result<(), RepoError> {
    let mut ids = HashSet::new();
    let mut per_kind: BTreeMap<Kind, usize> = BTreeMap::new();
    for entry in &index.sources {
        if !is_plain_id(&entry.id) || !ids.insert(entry.id.as_str()) {
            return Err(RepoError::BadId(entry.id.clone()));
        }
        if !is_sha256_hex(&entry.sha256) {
            return Err(RepoError::BadChecksum(entry.id.clone()));
        }
        if !(entry.file.starts_with("https://") && (entry.file.ends_with(".toml") || entry.file.ends_with(".wasm"))) {
            return Err(RepoError::BadLink(entry.id.clone()));
        }
        *per_kind.entry(entry.kind).or_default() += 1;
    }
    match per_kind.into_iter().find(|(_, count)| *count > MAX_SOURCES_PER_KIND) {
        Some((kind, count)) => Err(RepoError::TooMany { kind, count, limit: MAX_SOURCES_PER_KIND }),
        None => Ok(()),
    }
}

fn is_plain_id(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'.' | b'_' | b'-'))
}

fn is_sha256_hex(text: &str) -> bool {
    text.len() == 64 && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// The sources of one kind, grouped by language. The map is ordered by language, so a language filter is one lookup.
pub fn by_language(index: &Index, kind: Kind) -> BTreeMap<&str, Vec<&Entry>> {
    let mut groups: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for entry in index.sources.iter().filter(|entry| entry.kind == kind) {
        groups.entry(entry.lang.as_str()).or_default().push(entry);
    }
    groups
}

/// The SHA-256 of `bytes` as lower-case hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Adds a repository link to `links`. The link is trimmed, must be https, and is kept once: adding it twice changes nothing.
pub fn add_link(links: &mut Vec<String>, link: &str) -> Result<(), RepoError> {
    let link = link.trim();
    if !link.starts_with("https://") {
        return Err(RepoError::BadRepoLink);
    }
    if !links.iter().any(|known| known == link) {
        links.push(link.to_owned());
    }
    Ok(())
}

/// Removes a repository link. True when it was there.
pub fn remove_link(links: &mut Vec<String>, link: &str) -> bool {
    let before = links.len();
    links.retain(|known| known != link);
    links.len() != before
}

/// Whether the daily index check is due. It uses the anime extension check's rule ([`next_delay`]), so one timer serves
/// both. `last_check` and `now` are Unix seconds.
pub fn refresh_due(last_check: Option<i64>, now: i64) -> bool {
    next_delay(last_check, now) == Duration::ZERO
}

/// Fetches a repository's index and checks it. A link that is not https is refused before any request.
pub async fn fetch_index(client: &reqwest::Client, link: &str) -> Result<Index, RepoError> {
    if !link.starts_with("https://") {
        return Err(RepoError::BadRepoLink);
    }
    let bytes = get(client, link).await?;
    let text = String::from_utf8(bytes).map_err(|e| RepoError::Parse(e.to_string()))?;
    parse_index(&text)
}

/// Fetches one source file and checks it against the sha256 of its index entry. A file that does not match is an error,
/// so the caller keeps the copy it has.
pub async fn fetch_source(client: &reqwest::Client, entry: &Entry) -> Result<Vec<u8>, RepoError> {
    let bytes = get(client, &entry.file).await?;
    if sha256_hex(&bytes) != entry.sha256.to_ascii_lowercase() {
        return Err(RepoError::Mismatch);
    }
    Ok(bytes)
}

/// Where the copy of a source's file lives under `root`: `<id>.toml` or `<id>.wasm`, the extension the link has. The id
/// is checked by [`parse_index`], so it cannot leave the folder.
pub fn cached_path(root: &Path, entry: &Entry) -> PathBuf {
    let extension = if entry.file.ends_with(".wasm") { "wasm" } else { "toml" };
    root.join(format!("{}.{extension}", entry.id))
}

/// The path of the source's file under `root`, fetched first when there is no copy or the copy's bytes do not match the
/// index. A matching copy is used as it is, so a source is downloaded on its first use and then read from disk.
pub async fn ensure_cached(client: &reqwest::Client, root: &Path, entry: &Entry) -> Result<PathBuf, RepoError> {
    let path = cached_path(root, entry);
    let cached = std::fs::read(&path).ok();
    if cached.is_some_and(|bytes| sha256_hex(&bytes) == entry.sha256.to_ascii_lowercase()) {
        return Ok(path);
    }
    let bytes = fetch_source(client, entry).await?;
    write_atomic(&path, &bytes).map_err(|e| RepoError::Write(e.to_string()))?;
    Ok(path)
}

/// Writes `bytes` to `path` through a `.part` file, so a crash never leaves a half-written copy under the final name.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    // a name of its own per write, so two fetches of one entry never rename each other's file
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let part = path.with_extension(format!("{}.{n}.part", std::process::id()));
    std::fs::write(&part, bytes)?;
    std::fs::rename(&part, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&part);
    })
}

async fn get(client: &reqwest::Client, url: &str) -> Result<Vec<u8>, RepoError> {
    let mut response = client.get(url).send().await.map_err(|e| RepoError::Network(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(RepoError::Status(status.as_u16()));
    }
    if response.content_length().is_some_and(|n| n > MAX_FILE_BYTES as u64) {
        return Err(RepoError::TooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|e| RepoError::Network(e.to_string()))? {
        if bytes.len() + chunk.len() > MAX_FILE_BYTES {
            return Err(RepoError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA_ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    fn entry(id: &str, kind: &str, lang: &str) -> String {
        format!(
            r#"{{"id":"{id}","name":"Name {id}","lang":"{lang}","kind":"{kind}","version":1,"file":"https://raw.example/{id}.toml","sha256":"{SHA_ABC}"}}"#
        )
    }

    fn index_of(entries: &[String]) -> String {
        format!(r#"{{"version":1,"sources":[{}]}}"#, entries.join(","))
    }

    #[test]
    fn a_valid_index_is_read_and_grouped_by_language() {
        let text = index_of(&[entry("a.one", "manga", "en"), entry("b.two", "manga", "ja"), entry("c.three", "anime", "en")]);
        let index = parse_index(&text).expect("valid");
        let manga = by_language(&index, Kind::Manga);
        assert_eq!(manga.keys().copied().collect::<Vec<_>>(), vec!["en", "ja"]);
        assert_eq!(manga["en"][0].id, "a.one");
        assert_eq!(by_language(&index, Kind::Anime).len(), 1);
    }

    #[test]
    fn ten_sources_of_one_kind_are_the_limit_and_eleven_are_refused() {
        let ten: Vec<String> = (0..10).map(|i| entry(&format!("m{i}"), "manga", "en")).collect();
        let anime: Vec<String> = (0..10).map(|i| entry(&format!("a{i}"), "anime", "en")).collect();
        let mut both = ten.clone();
        both.extend(anime);
        assert!(parse_index(&index_of(&both)).is_ok(), "ten of each kind is fine");
        let mut eleven = ten;
        eleven.push(entry("m10", "manga", "en"));
        assert_eq!(
            parse_index(&index_of(&eleven)),
            Err(RepoError::TooMany { kind: Kind::Manga, count: 11, limit: 10 })
        );
    }

    #[test]
    fn a_duplicate_or_unsafe_id_is_refused() {
        let twice = index_of(&[entry("same", "manga", "en"), entry("same", "anime", "en")]);
        assert_eq!(parse_index(&twice), Err(RepoError::BadId("same".into())));
        let slash = index_of(&[entry("../escape", "manga", "en")]);
        assert_eq!(parse_index(&slash), Err(RepoError::BadId("../escape".into())));
    }

    #[test]
    fn a_bad_checksum_or_link_is_refused() {
        let short = index_of(&[entry("x", "manga", "en").replace(SHA_ABC, "abc")]);
        assert_eq!(parse_index(&short), Err(RepoError::BadChecksum("x".into())));
        let plain = index_of(&[entry("x", "manga", "en").replace("https://", "http://")]);
        assert_eq!(parse_index(&plain), Err(RepoError::BadLink("x".into())));
        let not_source = index_of(&[entry("x", "manga", "en").replace(".toml", ".exe")]);
        assert_eq!(parse_index(&not_source), Err(RepoError::BadLink("x".into())));
    }

    #[test]
    fn a_plug_in_link_is_accepted_and_its_copy_is_named_by_the_id_and_the_extension() {
        let text = index_of(&[entry("x", "anime", "en").replace(".toml", ".wasm")]);
        let index = parse_index(&text).expect("a .wasm link is a source too");
        assert_eq!(cached_path(Path::new("/c"), &index.sources[0]), PathBuf::from("/c/x.wasm"));
        let recipe = parse_index(&index_of(&[entry("y", "manga", "en")])).expect("valid");
        assert_eq!(cached_path(Path::new("/c"), &recipe.sources[0]), PathBuf::from("/c/y.toml"));
    }

    #[test]
    fn sha256_matches_the_standard_vector() {
        assert_eq!(sha256_hex(b"abc"), SHA_ABC);
    }

    #[test]
    fn links_must_be_https_and_are_kept_once() {
        let mut links = Vec::new();
        add_link(&mut links, "  https://host/index.min.json ").expect("added");
        add_link(&mut links, "https://host/index.min.json").expect("same link again");
        assert_eq!(links, vec!["https://host/index.min.json".to_owned()]);
        assert_eq!(add_link(&mut links, "http://host/index.min.json"), Err(RepoError::BadRepoLink));
        assert!(remove_link(&mut links, "https://host/index.min.json"));
        assert!(!remove_link(&mut links, "https://host/index.min.json"));
    }

    #[test]
    fn the_daily_check_is_due_when_none_is_stored_or_a_day_has_passed() {
        let day = 24 * 60 * 60;
        assert!(refresh_due(None, 1_000));
        assert!(!refresh_due(Some(1_000), 1_000 + day - 1));
        assert!(refresh_due(Some(1_000), 1_000 + day));
    }

    #[tokio::test]
    async fn a_source_file_is_returned_only_when_its_checksum_matches() {
        use wiremock::matchers::path;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        Mock::given(path("/ok.toml")).respond_with(ResponseTemplate::new(200).set_body_bytes(b"abc".to_vec())).mount(&server).await;
        Mock::given(path("/bad.toml")).respond_with(ResponseTemplate::new(200).set_body_bytes(b"xyz".to_vec())).mount(&server).await;
        let client = reqwest::Client::new();
        let mut good = parse_index(&index_of(&[entry("ok", "manga", "en")])).expect("valid").sources.remove(0);
        good.file = format!("{}/ok.toml", server.uri());
        assert_eq!(fetch_source(&client, &good).await.expect("matches"), b"abc".to_vec());
        let mut bad = good.clone();
        bad.file = format!("{}/bad.toml", server.uri());
        assert_eq!(fetch_source(&client, &bad).await, Err(RepoError::Mismatch));
    }

    #[tokio::test]
    async fn an_index_over_plain_http_is_refused_before_any_request() {
        let client = reqwest::Client::new();
        assert_eq!(fetch_index(&client, "http://host/index.min.json").await, Err(RepoError::BadRepoLink));
    }

    #[tokio::test]
    async fn a_source_is_fetched_when_it_has_no_copy_and_read_from_disk_after_that() {
        use wiremock::matchers::path;
        use wiremock::{Mock, MockServer, ResponseTemplate};
        let server = MockServer::start().await;
        // two fetches: the first use, and the one after the copy is changed on disk
        Mock::given(path("/ok.toml"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(b"abc".to_vec()))
            .expect(2)
            .mount(&server)
            .await;
        let client = reqwest::Client::new();
        let mut source = parse_index(&index_of(&[entry("ok", "manga", "en")])).expect("valid").sources.remove(0);
        source.file = format!("{}/ok.toml", server.uri());
        let dir = tempfile::tempdir().expect("temp dir");
        let root = dir.path().join("sources");

        let first = ensure_cached(&client, &root, &source).await.expect("fetched");
        let second = ensure_cached(&client, &root, &source).await.expect("read from the copy");
        assert_eq!(first, second);
        assert_eq!(std::fs::read(&first).expect("the copy"), b"abc");

        std::fs::write(&first, b"changed").expect("overwrite the copy");
        ensure_cached(&client, &root, &source).await.expect("fetched again");
        assert_eq!(std::fs::read(&first).expect("the copy"), b"abc");
    }
}
