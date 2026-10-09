//! Manga extension repo interactors (`mihon/domain/extensionrepo/manga/interactor/*`) and the `repo.json` service.
//! Rules: `p1-behavior.md` section 6 (area 09) and section 8 (area 06).
//!
//! The one network call is a GET, behind [`HttpGet`]. The HTTP client lives in `kuro-net` (area:12), which is not
//! in the workspace yet, so the flow is tested with a fake.

use std::sync::LazyLock;

use kuro_data::DataResult;
use kuro_data::extension_repo::{ExtensionRepo, ExtensionRepoRepo, InsertConflict};
use regex::Regex;
use serde::Deserialize;

use crate::error::RepoStoreError;

/// Repos refreshed at the same time (`UpdateMangaExtensionRepo`, a `JoinSet` capped at 4 fetches).
const REFRESH_CONCURRENCY: usize = 4;

/// The one network call the repo flow makes: a GET whose body is returned as text.
pub trait HttpGet: Sync {
    fn get_text(&self, url: &str) -> Result<String, String>;
}

/// `ExtensionRepoMetaDto`: the `repo.json` document, with its details under `meta`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRepoMetaDto {
    pub meta: ExtensionRepoDto,
}

/// `ExtensionRepoDto`: the details of a repo. `short_name` may be absent.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRepoDto {
    pub name: String,
    pub short_name: Option<String>,
    pub website: String,
    pub signing_key_fingerprint: String,
}

/// `ExtensionRepoService`: fetches a repo's details with `GET {base}/repo.json`.
pub struct ExtensionRepoService<'a> {
    http: &'a dyn HttpGet,
}

impl<'a> ExtensionRepoService<'a> {
    pub fn new(http: &'a dyn HttpGet) -> Self {
        Self { http }
    }

    /// `fetchRepoDetails(baseUrl)`: `None` when the GET or the parse fails. The failure is logged.
    pub fn fetch_repo_details(&self, base_url: &str) -> Option<ExtensionRepo> {
        let url = format!("{base_url}/repo.json");
        let body = match self.http.get_text(&url) {
            Ok(body) => body,
            Err(e) => {
                tracing::error!(error = %e, repo = base_url, "Failed to fetch repo details");
                return None;
            }
        };
        match serde_json::from_str::<ExtensionRepoMetaDto>(&body) {
            Ok(dto) => Some(to_extension_repo(dto, base_url)),
            Err(e) => {
                tracing::error!(error = %e, repo = base_url, "Failed to fetch repo details");
                None
            }
        }
    }
}

/// `ExtensionRepoMetaDto.toExtensionRepo(baseUrl)`.
fn to_extension_repo(dto: ExtensionRepoMetaDto, base_url: &str) -> ExtensionRepo {
    ExtensionRepo {
        base_url: base_url.to_owned(),
        name: dto.meta.name,
        short_name: dto.meta.short_name,
        website: dto.meta.website,
        signing_key_fingerprint: dto.meta.signing_key_fingerprint,
    }
}

/// `^https://.*/index\.min\.json$`, the check on the normalised index URL.
static REPO_INDEX_URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^https://.*/index\.min\.json$").expect("valid regex"));

/// `validate_index_url`: the URL is parsed and normalised (`toHttpUrlOrNull().toString()`: lowercase host, default
/// port dropped, dot segments resolved), then must match `^https://.*/index\.min\.json$`. Returns the normalised
/// index URL and the base URL (the index URL without `/index.min.json`). `None` is `InvalidUrl`.
pub fn validate_index_url(input: &str) -> Option<(String, String)> {
    let url = url::Url::parse(input).ok()?;
    let formatted = url.as_str().to_owned();
    if !REPO_INDEX_URL.is_match(&formatted) {
        return None;
    }
    let base = formatted.strip_suffix("/index.min.json")?.to_owned();
    Some((formatted, base))
}

/// `AddRepoOutcome`: what adding a manga repo produced.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)] // two repo payloads, matched once per add call
pub enum AddRepoOutcome {
    Success,
    /// The index URL is not an acceptable repo URL, or its `repo.json` could not be read (E-REPO-04).
    InvalidUrl,
    /// A repo with this base URL already exists (E-REPO-05).
    RepoAlreadyExists,
    /// Another repo owns the signing-key fingerprint. Carries the stored repo (`old`) and the new one (`new`).
    DuplicateFingerprint {
        old: ExtensionRepo,
        new: ExtensionRepo,
    },
    /// Any other insert failure.
    Error,
}

/// `insert` of a repo, with a failure reported as `RepoStoreError::Save` (`SaveExtensionRepoException`).
pub fn insert_repo(repos: &ExtensionRepoRepo, r: &ExtensionRepo) -> Result<(), RepoStoreError> {
    repos.insert(r).map_err(|e| RepoStoreError::Save(e.0))
}

/// `upsert` of a repo, with a failure reported as `RepoStoreError::Save`.
pub fn upsert_repo(repos: &ExtensionRepoRepo, r: &ExtensionRepo) -> Result<(), RepoStoreError> {
    repos.upsert(r).map_err(RepoStoreError::Save)
}

/// `CreateMangaExtensionRepo.await(indexUrl)`.
pub fn add_manga_repo(repos: &ExtensionRepoRepo, service: &ExtensionRepoService, index_url: &str) -> AddRepoOutcome {
    let Some((_, base_url)) = validate_index_url(index_url) else {
        return AddRepoOutcome::InvalidUrl;
    };
    let Some(repo) = service.fetch_repo_details(&base_url) else {
        return AddRepoOutcome::InvalidUrl;
    };
    let Err(e) = insert_repo(repos, &repo) else {
        return AddRepoOutcome::Success;
    };
    tracing::warn!(error = %e, repo = %repo.base_url, "SQL Conflict attempting to add new manga repository");
    match repos.classify_conflict(&repo.base_url, &repo.signing_key_fingerprint) {
        Ok(InsertConflict::RepoAlreadyExists) => AddRepoOutcome::RepoAlreadyExists,
        Ok(InsertConflict::DuplicateFingerprint(old)) => AddRepoOutcome::DuplicateFingerprint { old, new: repo },
        Ok(InsertConflict::Other) => AddRepoOutcome::Error,
        Err(e) => {
            tracing::error!(error = %e, "cannot classify a failed manga repo insert");
            AddRepoOutcome::Error
        }
    }
}

/// `UpdateMangaExtensionRepo.await(repo)`: the fetched details are written only when the stored fingerprint is a
/// `NOFINGERPRINT` placeholder or equals the fetched one. Otherwise nothing is written.
pub fn update(repos: &ExtensionRepoRepo, service: &ExtensionRepoService, repo: &ExtensionRepo) -> DataResult<()> {
    let Some(fetched) = service.fetch_repo_details(&repo.base_url) else {
        return Ok(());
    };
    match accept_fetched(repo, fetched) {
        Some(next) => repos.upsert(&next),
        None => Ok(()),
    }
}

/// `UpdateMangaExtensionRepo.awaitAll()`: the repos are fetched in parallel, at most [`REFRESH_CONCURRENCY`] at a time,
/// and the writes follow in store order. A failing fetch or write leaves that repo alone and does not stop the others.
pub fn update_all(repos: &ExtensionRepoRepo, service: &ExtensionRepoService) -> DataResult<()> {
    let stored = repos.all()?;
    let fetched = fetch_all(service, &stored);
    for (old, new) in stored.iter().zip(fetched) {
        let Some(next) = new.and_then(|n| accept_fetched(old, n)) else {
            continue;
        };
        if let Err(e) = upsert_repo(repos, &next) {
            tracing::error!(error = %e, repo = %old.base_url, "cannot refresh manga repo");
        }
    }
    Ok(())
}

/// Fetches the repos in batches of [`REFRESH_CONCURRENCY`], each batch on its own threads, and returns the results in
/// input order. No batch starts before the previous one has finished, so no more than four fetches run at once.
fn fetch_all(service: &ExtensionRepoService, repos: &[ExtensionRepo]) -> Vec<Option<ExtensionRepo>> {
    repos
        .chunks(REFRESH_CONCURRENCY)
        .flat_map(|batch| {
            std::thread::scope(|scope| {
                let handles: Vec<_> =
                    batch.iter().map(|r| scope.spawn(move || service.fetch_repo_details(&r.base_url))).collect();
                handles
                    .into_iter()
                    .map(|h| match h.join() {
                        Ok(fetched) => fetched,
                        Err(panic) => std::panic::resume_unwind(panic),
                    })
                    .collect::<Vec<_>>()
            })
        })
        .collect()
}

/// The row to write after a refresh: a `NOFINGERPRINT` placeholder adopts the fetched key, any other row must match.
/// A changed key is skipped and logged (E-REPO-07).
fn accept_fetched(stored: &ExtensionRepo, fetched: ExtensionRepo) -> Option<ExtensionRepo> {
    let placeholder = stored.signing_key_fingerprint.starts_with("NOFINGERPRINT");
    if placeholder || stored.signing_key_fingerprint == fetched.signing_key_fingerprint {
        return Some(fetched);
    }
    tracing::warn!(repo = %stored.base_url, "signing key fingerprint changed upstream; repo not updated");
    None
}

/// `ReplaceMangaExtensionRepo.await(repo)`.
pub fn replace(repos: &ExtensionRepoRepo, repo: &ExtensionRepo) -> DataResult<()> {
    repos.replace(repo)
}

/// `DeleteMangaExtensionRepo.await(baseUrl)`.
pub fn delete(repos: &ExtensionRepoRepo, base_url: &str) -> DataResult<()> {
    repos.delete(base_url)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use kuro_data::extension_repo::ExtensionRepoRepo;

    use super::*;
    use crate::testdb::manga_db;

    /// Serves fixed bodies by URL. Records calls and the most fetches in flight at once.
    #[derive(Default)]
    struct FakeHttp {
        bodies: HashMap<String, String>,
        calls: AtomicUsize,
        in_flight: AtomicUsize,
        peak: AtomicUsize,
        delay: Duration,
    }

    impl HttpGet for FakeHttp {
        fn get_text(&self, url: &str) -> Result<String, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let now = self.in_flight.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            std::thread::sleep(self.delay);
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            self.bodies.get(url).cloned().ok_or_else(|| format!("404 {url}"))
        }
    }

    fn meta_json(name: &str, fingerprint: &str) -> String {
        format!(
            r#"{{"meta":{{"name":"{name}","website":"https://{name}.test","signingKeyFingerprint":"{fingerprint}"}}}}"#
        )
    }

    /// A fake whose `https://{host}.test/repo.json` serves the given name and fingerprint.
    fn http_with(repos: &[(&str, &str, &str)]) -> FakeHttp {
        let bodies = repos
            .iter()
            .map(|(host, name, fp)| (format!("https://{host}.test/repo.json"), meta_json(name, fp)))
            .collect();
        FakeHttp { bodies, ..Default::default() }
    }

    fn repo(base: &str, name: &str, fingerprint: &str) -> ExtensionRepo {
        ExtensionRepo {
            base_url: base.into(),
            name: name.into(),
            short_name: None,
            website: format!("https://{name}.test"),
            signing_key_fingerprint: fingerprint.into(),
        }
    }

    fn repos() -> (tempfile::TempDir, ExtensionRepoRepo) {
        let (dir, db) = manga_db();
        (dir, ExtensionRepoRepo::new(db))
    }

    #[test]
    fn repo_json_parses_into_the_repo_and_short_name_is_optional() {
        let http = http_with(&[("a", "Alpha", "fp-a")]);
        let got = ExtensionRepoService::new(&http).fetch_repo_details("https://a.test");
        assert_eq!(got, Some(repo("https://a.test", "Alpha", "fp-a")));
        assert_eq!(http.calls.load(Ordering::SeqCst), 1, "one GET of the repo.json URL");
    }

    #[test]
    fn a_failed_get_or_a_bad_document_gives_no_details() {
        let mut http = http_with(&[]);
        http.bodies.insert("https://bad.test/repo.json".into(), "not json".into());
        let service = ExtensionRepoService::new(&http);
        assert_eq!(service.fetch_repo_details("https://missing.test"), None);
        assert_eq!(service.fetch_repo_details("https://bad.test"), None);
    }

    #[test]
    fn url_normalisation_vector_gives_the_base_url() {
        let (formatted, base) = validate_index_url("https://Example.COM:443/a/../index.min.json").expect("valid");
        assert_eq!(formatted, "https://example.com/index.min.json");
        assert_eq!(base, "https://example.com");
    }

    #[test]
    fn a_valid_index_url_gives_the_normalised_url_and_the_base() {
        assert_eq!(
            validate_index_url("https://example.com/a/index.min.json"),
            Some(("https://example.com/a/index.min.json".into(), "https://example.com/a".into())),
            "TC-REPO-01"
        );
        assert_eq!(validate_index_url("http://example.com/index.min.json"), None, "TC-REPO-02: http");
        assert_eq!(validate_index_url("https://example.com/index.json"), None, "TC-REPO-02: wrong file");
        assert_eq!(validate_index_url("https://example.com/index.min.json?x"), None, "TC-REPO-02: query");
    }

    #[test]
    fn the_repo_json_fixture_decodes_with_a_null_short_name() {
        let body = r#"{"meta":{"name":"Test Repo","shortName":null,"website":"https://example.com","signingKeyFingerprint":"abc123"}}"#;
        let http = FakeHttp {
            bodies: HashMap::from([("https://fixture.test/repo.json".to_owned(), body.to_owned())]),
            ..Default::default()
        };
        let got = ExtensionRepoService::new(&http).fetch_repo_details("https://fixture.test");
        assert_eq!(
            got,
            Some(ExtensionRepo {
                base_url: "https://fixture.test".into(),
                name: "Test Repo".into(),
                short_name: None,
                website: "https://example.com".into(),
                signing_key_fingerprint: "abc123".into(),
            })
        );
    }

    #[test]
    fn plain_http_is_invalid_and_never_fetches() {
        let (_d, repos) = repos();
        let http = http_with(&[("x", "X", "fp")]);
        let service = ExtensionRepoService::new(&http);
        assert_eq!(add_manga_repo(&repos, &service, "http://x.test/index.min.json"), AddRepoOutcome::InvalidUrl);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0, "rejected before the GET");
    }

    #[test]
    fn a_query_string_fails_the_suffix_check() {
        let (_d, repos) = repos();
        let http = http_with(&[("x", "X", "fp")]);
        let service = ExtensionRepoService::new(&http);
        assert_eq!(add_manga_repo(&repos, &service, "https://x.test/index.min.json?x=1"), AddRepoOutcome::InvalidUrl);
        assert_eq!(http.calls.load(Ordering::SeqCst), 0);
        assert_eq!(add_manga_repo(&repos, &service, "not a url"), AddRepoOutcome::InvalidUrl);
    }

    #[test]
    fn unreadable_repo_json_is_invalid_and_stores_nothing() {
        let (_d, repos) = repos();
        let http = http_with(&[]);
        let service = ExtensionRepoService::new(&http);
        assert_eq!(add_manga_repo(&repos, &service, "https://gone.test/index.min.json"), AddRepoOutcome::InvalidUrl);
        assert!(repos.all().expect("all").is_empty());
    }

    #[test]
    fn create_success_then_repo_already_exists() {
        let (_d, repos) = repos();
        let http = http_with(&[("a", "Alpha", "fp-a")]);
        let service = ExtensionRepoService::new(&http);
        assert_eq!(add_manga_repo(&repos, &service, "https://a.test/index.min.json"), AddRepoOutcome::Success);
        assert_eq!(repos.get("https://a.test").expect("get"), Some(repo("https://a.test", "Alpha", "fp-a")));
        assert_eq!(
            add_manga_repo(&repos, &service, "https://a.test/index.min.json"),
            AddRepoOutcome::RepoAlreadyExists
        );
    }

    #[test]
    fn duplicate_fingerprint_names_both_repos_and_replace_moves_the_row() {
        let (_d, repos) = repos();
        let http = http_with(&[("a", "Alpha", "shared"), ("b", "Beta", "shared")]);
        let service = ExtensionRepoService::new(&http);
        assert_eq!(add_manga_repo(&repos, &service, "https://a.test/index.min.json"), AddRepoOutcome::Success);
        assert_eq!(
            add_manga_repo(&repos, &service, "https://b.test/index.min.json"),
            AddRepoOutcome::DuplicateFingerprint {
                old: repo("https://a.test", "Alpha", "shared"),
                new: repo("https://b.test", "Beta", "shared"),
            }
        );
        replace(&repos, &repo("https://b.test", "Beta", "shared")).expect("replace");
        assert_eq!(repos.all().expect("all"), vec![repo("https://b.test", "Beta", "shared")]);
    }

    #[test]
    fn replace_onto_a_third_repos_base_url_is_an_error() {
        let (_d, repos) = repos();
        repos.insert(&repo("https://a.test", "A", "fp-a")).expect("insert");
        repos.insert(&repo("https://c.test", "C", "fp-c")).expect("insert");
        assert!(replace(&repos, &repo("https://c.test", "A", "fp-a")).is_err());
    }

    #[test]
    fn delete_removes_the_repo() {
        let (_d, repos) = repos();
        repos.insert(&repo("https://a.test", "A", "fp-a")).expect("insert");
        delete(&repos, "https://a.test").expect("delete");
        assert!(repos.all().expect("all").is_empty());
    }

    #[test]
    fn delete_of_an_absent_base_url_is_ok() {
        let (_d, repos) = repos();
        assert!(delete(&repos, "https://absent.test").is_ok(), "TC-REPO-07");
    }

    #[test]
    fn update_adopts_the_fingerprint_of_a_placeholder_row() {
        let (_d, repos) = repos();
        let stored = repo("https://a.test", "A", "NOFINGERPRINT-1");
        repos.insert(&stored).expect("insert");
        let http = http_with(&[("a", "Alpha", "real")]);
        update(&repos, &ExtensionRepoService::new(&http), &stored).expect("update");
        assert_eq!(repos.get("https://a.test").expect("get").expect("row").signing_key_fingerprint, "real");
    }

    #[test]
    fn update_all_adopts_placeholders_ignores_a_changed_key_and_keeps_going_past_a_failure() {
        let (_d, repos) = repos();
        repos.insert(&repo("https://a.test", "A", "NOFINGERPRINT-1")).expect("insert");
        repos.insert(&repo("https://b.test", "B", "fp-b")).expect("insert");
        repos.insert(&repo("https://c.test", "C", "fp-c")).expect("insert");
        // a adopts its key, b's key changed so it stays, c has no repo.json at all.
        let http = http_with(&[("a", "Alpha", "real-a"), ("b", "Beta", "fp-b-new")]);
        update_all(&repos, &ExtensionRepoService::new(&http)).expect("update all");
        let row = |base: &str| repos.get(base).expect("get").expect("row");
        assert_eq!(row("https://a.test"), repo("https://a.test", "Alpha", "real-a"));
        assert_eq!(row("https://b.test"), repo("https://b.test", "B", "fp-b"), "a changed key is ignored");
        assert_eq!(row("https://c.test"), repo("https://c.test", "C", "fp-c"), "a failed fetch leaves c alone");
    }

    #[test]
    fn update_all_never_runs_more_than_four_fetches_at_once() {
        let (_d, repos) = repos();
        for host in ["a", "b", "c", "d", "e", "f"] {
            repos.insert(&repo(&format!("https://{host}.test"), host, &format!("fp-{host}"))).expect("insert");
        }
        let mut http = http_with(&[
            ("a", "A", "fp-a"),
            ("b", "B", "fp-b"),
            ("c", "C", "fp-c"),
            ("d", "D", "fp-d"),
            ("e", "E", "fp-e"),
            ("f", "F", "fp-f"),
        ]);
        http.delay = Duration::from_millis(100);
        update_all(&repos, &ExtensionRepoService::new(&http)).expect("update all");
        assert_eq!(http.peak.load(Ordering::SeqCst), 4, "TC-REPO-08: capped at 4");
        assert_eq!(http.calls.load(Ordering::SeqCst), 6, "every repo is fetched");
    }

    #[test]
    fn update_all_fetches_every_repo_at_the_same_time() {
        let (_d, repos) = repos();
        for host in ["a", "b", "c"] {
            repos.insert(&repo(&format!("https://{host}.test"), host, &format!("fp-{host}"))).expect("insert");
        }
        let mut http = http_with(&[("a", "A", "fp-a"), ("b", "B", "fp-b"), ("c", "C", "fp-c")]);
        http.delay = Duration::from_millis(150);
        update_all(&repos, &ExtensionRepoService::new(&http)).expect("update all");
        assert_eq!(http.peak.load(Ordering::SeqCst), 3, "three GETs in flight at once");
    }
}
