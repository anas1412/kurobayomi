//! The blocking work behind the extension store and repo pages (T-47b-009 to T-47b-013, T-47b-019, T-47b-020). The page
//! runs these on a worker thread and feeds each result back to the reducer in `stores.rs`.

use std::sync::Arc;

use kuro_data::extension_repo::{ExtensionRepo, ExtensionRepoRepo};
use kuro_data::extension_store::ExtensionStoreRepo;
use kuro_domain::extension_repo::{self, AddRepoOutcome, ExtensionRepoService, HttpGet};
use kuro_domain::extension_store::{self, AnimeExtensionStore, StoreHttp};
use kuro_i18n::{Arg, MsgKey, tr, trf};

use super::stores;

/// What one card shows and does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub key: String,
    pub name: String,
    /// The website; the globe button is off when it is empty.
    pub website: String,
    /// The Discord link; the button shows only when this is set.
    pub discord: Option<String>,
    /// What the copy button puts on the clipboard.
    pub copy: String,
    /// A line under the name, such as "N entries not runnable on Linux" (T-19a-066).
    pub note: Option<String>,
}

/// How an add ended.
pub enum AddOutcome {
    Added,
    /// The dialog stays open and shows this text (stores).
    Failed(String),
    /// The dialog closes and the toast shows this text (repos).
    Notice(String),
    /// Another repo owns the signing key: the conflict dialog offers to replace it (repos).
    Conflict { old_name: String, new_name: String, replace: Box<dyn FnOnce() + Send> },
}

/// The operations of one kind of list. They block, so the page calls them off the GTK thread.
pub trait Ops<T>: Send + Sync + 'static {
    fn card(&self, item: &T) -> Card;
    fn add(&self, url: &str) -> AddOutcome;
    /// Re-fetches every stored entry. A failing one is logged and skipped.
    fn refresh(&self);
    fn remove(&self, key: &str);
}

/// The anime extension stores (`AnimeExtensionStoresScreenModel`).
pub struct StoreOps {
    repo: ExtensionStoreRepo,
    http: Arc<dyn StoreHttp + Send>,
}

impl StoreOps {
    pub fn new(repo: ExtensionStoreRepo, http: Arc<dyn StoreHttp + Send>) -> Self {
        Self { repo, http }
    }
}

impl Ops<AnimeExtensionStore> for StoreOps {
    fn card(&self, store: &AnimeExtensionStore) -> Card {
        Card {
            key: store.index_url.clone(),
            name: store.name.clone(),
            website: store.contact.website.clone(),
            discord: store.contact.discord.clone().filter(|d| !d.is_empty()),
            copy: store.index_url.clone(),
            note: crate::browse::ext_badges::store_note_for(&store.index_url),
        }
    }

    fn add(&self, url: &str) -> AddOutcome {
        match extension_store::add_store(&self.repo, &*self.http, url) {
            Ok(()) => AddOutcome::Added,
            Err(e) => {
                tracing::warn!(error = %e, url, "adding the extension store failed");
                AddOutcome::Failed(e.to_string())
            }
        }
    }

    fn refresh(&self) {
        if let Err(e) = extension_store::refresh_all(&self.repo, &*self.http) {
            tracing::error!(error = %e, "refreshing the extension stores failed");
        }
    }

    fn remove(&self, key: &str) {
        if let Err(e) = self.repo.remove(key) {
            tracing::error!(error = %e, key, "removing the extension store failed");
        }
    }
}

/// The manga extension repos (`MangaExtensionReposScreenModel`).
pub struct RepoOps {
    repos: ExtensionRepoRepo,
    http: Arc<dyn HttpGet + Send>,
}

impl RepoOps {
    pub fn new(repos: ExtensionRepoRepo, http: Arc<dyn HttpGet + Send>) -> Self {
        Self { repos, http }
    }
}

/// The toast text and dialog result of each add outcome (`MangaExtensionReposScreenModel.createRepo`).
pub fn repo_outcome(outcome: AddRepoOutcome, repos: &ExtensionRepoRepo) -> AddOutcome {
    match outcome {
        AddRepoOutcome::Success => AddOutcome::Added,
        AddRepoOutcome::InvalidUrl => AddOutcome::Notice(tr(MsgKey::invalid_repo_name)),
        AddRepoOutcome::RepoAlreadyExists => AddOutcome::Notice(tr(MsgKey::error_repo_exists)),
        // Kotlin is silent here; a toast is the one deliberate deviation (T-47b-020). The string table has no
        // "Could not add the repo" text, so it says "Unknown error".
        AddRepoOutcome::Error => AddOutcome::Notice(stores::unknown_error()),
        AddRepoOutcome::DuplicateFingerprint { old, new } => {
            let repos = repos.clone();
            let (old_name, new_name) = (old.name.clone(), new.name.clone());
            AddOutcome::Conflict {
                old_name,
                new_name,
                replace: Box::new(move || {
                    if let Err(e) = extension_repo::replace(&repos, &new) {
                        tracing::error!(error = %e, repo = %new.base_url, "replacing the repo failed");
                    }
                }),
            }
        }
    }
}

impl Ops<ExtensionRepo> for RepoOps {
    fn card(&self, repo: &ExtensionRepo) -> Card {
        Card {
            key: repo.base_url.clone(),
            name: repo.name.clone(),
            website: repo.website.clone(),
            discord: None,
            copy: format!("{}/index.min.json", repo.base_url),
            note: None,
        }
    }

    fn add(&self, url: &str) -> AddOutcome {
        let service = ExtensionRepoService::new(&*self.http);
        repo_outcome(extension_repo::add_manga_repo(&self.repos, &service, url), &self.repos)
    }

    fn refresh(&self) {
        let service = ExtensionRepoService::new(&*self.http);
        if let Err(e) = extension_repo::update_all(&self.repos, &service) {
            tracing::error!(error = %e, "refreshing the extension repos failed");
        }
    }

    fn remove(&self, key: &str) {
        if let Err(e) = extension_repo::delete(&self.repos, key) {
            tracing::error!(error = %e, key, "removing the extension repo failed");
        }
    }
}

/// The body of the remove dialog for a store.
pub fn store_remove_body(name: &str, url: &str) -> String {
    stores::remove_body(name, url)
}

/// The body of the delete dialog for a repo: it names the base URL only.
pub fn repo_remove_body(_name: &str, base_url: &str) -> String {
    trf(MsgKey::delete_repo_confirmation, &[("arg1", Arg::Str(base_url))])
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use kuro_db::handle::Db;
    use kuro_domain::extension_store::Contact;

    use super::*;

    struct Canned {
        routes: HashMap<String, Result<Vec<u8>, String>>,
        calls: Mutex<Vec<String>>,
    }

    impl Canned {
        fn new(routes: &[(&str, Result<&str, &str>)]) -> Arc<Self> {
            Arc::new(Self {
                routes: routes.iter().map(|(u, r)| ((*u).to_owned(), r.map(|b| b.as_bytes().to_vec()).map_err(str::to_owned))).collect(),
                calls: Mutex::new(Vec::new()),
            })
        }
    }

    impl StoreHttp for Canned {
        fn get(&self, url: &str) -> Result<Vec<u8>, String> {
            self.calls.lock().unwrap().push(url.to_owned());
            self.routes.get(url).cloned().unwrap_or_else(|| Err("HTTP 404".into()))
        }
    }

    impl HttpGet for Canned {
        fn get_text(&self, url: &str) -> Result<String, String> {
            self.calls.lock().unwrap().push(url.to_owned());
            match self.routes.get(url).cloned().unwrap_or_else(|| Err("HTTP 404".into())) {
                Ok(bytes) => String::from_utf8(bytes).map_err(|e| e.to_string()),
                Err(e) => Err(e),
            }
        }
    }

    const STORE: &str = r#"{"name":"New Store","badgeLabel":"N","signingKey":"key","contact":{"website":"https://n.test","discord":"https://discord.gg/n"}}"#;
    const REPO: &str = r#"{"meta":{"name":"Repo A","shortName":"A","website":"https://a.test","signingKeyFingerprint":"fp-a"}}"#;
    const REPO_B_SAME_KEY: &str = r#"{"meta":{"name":"Repo B","shortName":"B","website":"https://b.test","signingKeyFingerprint":"fp-a"}}"#;

    fn anime_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("anime.db");
        let writer = kuro_db::schema_anime::open(&path).expect("anime schema");
        (dir, Arc::new(Db::new(&path, writer).expect("anime handle")))
    }

    fn manga_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("manga.db");
        let writer = kuro_db::schema_manga::open(&path).expect("manga schema");
        (dir, Arc::new(Db::new(&path, writer).expect("manga handle")))
    }

    fn store_ops(routes: &[(&str, Result<&str, &str>)]) -> (tempfile::TempDir, ExtensionStoreRepo, StoreOps) {
        let (dir, db) = anime_db();
        let repo = ExtensionStoreRepo::new(db);
        let ops = StoreOps::new(repo.clone(), Canned::new(routes));
        (dir, repo, ops)
    }

    #[test]
    fn t_47b_011_a_good_url_upserts_the_row() {
        let (_dir, repo, ops) = store_ops(&[("https://n.test/index.json", Ok(STORE))]);
        assert!(matches!(ops.add("https://n.test/index.json"), AddOutcome::Added));
        let rows = repo.all().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "New Store");
        assert_eq!(rows[0].index_url, "https://n.test/index.json", "keyed by the entered URL");
    }

    #[test]
    fn t_47b_011_garbage_and_unreachable_urls_fail_with_text_and_save_nothing() {
        let (_dir, repo, ops) = store_ops(&[("https://bad.test/x.json", Ok("not json at all")), ("https://down.test/x.json", Err("HTTP 500"))]);
        let AddOutcome::Failed(text) = ops.add("https://bad.test/x.json") else { panic!("garbage fails") };
        assert!(!text.is_empty());
        let AddOutcome::Failed(text) = ops.add("https://down.test/x.json") else { panic!("a failing GET fails") };
        assert!(text.contains("HTTP 500"), "{text}");
        assert_eq!(repo.count().expect("count"), 0);
    }

    #[test]
    fn t_47b_012_refresh_updates_what_it_can_and_skips_a_failing_store() {
        let (_dir, repo, ops) = store_ops(&[("https://ok.test/a.json", Ok(STORE))]);
        for (url, name) in [("https://ok.test/a.json", "Old A"), ("https://gone.test/b.json", "Old B")] {
            repo.add(&kuro_db::statements_anime::ExtensionStore {
                index_url: url.into(),
                name: name.into(),
                badge_label: String::new(),
                signing_key: "key".into(),
                contact_website: String::new(),
                contact_discord: None,
                is_legacy: false,
                extension_list_url: None,
                resolved_index_url: url.into(),
            })
            .expect("seed");
        }
        ops.refresh();
        assert_eq!(repo.get("https://ok.test/a.json").expect("get").expect("row").name, "New Store");
        assert_eq!(repo.get("https://gone.test/b.json").expect("get").expect("row").name, "Old B", "the failing store is kept");
    }

    #[test]
    fn t_47b_013_remove_deletes_by_index_url() {
        let (_dir, repo, ops) = store_ops(&[("https://n.test/index.json", Ok(STORE))]);
        let _ = ops.add("https://n.test/index.json");
        ops.remove("https://n.test/index.json");
        assert_eq!(repo.count().expect("count"), 0);
    }

    #[test]
    fn t_47b_015_a_card_lists_discord_only_when_set() {
        let (_dir, _repo, ops) = store_ops(&[]);
        let mut store = AnimeExtensionStore {
            index_url: "https://s/index.json".into(),
            name: "S".into(),
            badge_label: String::new(),
            signing_key: String::new(),
            contact: Contact { website: "https://s".into(), discord: Some("https://discord.gg/s".into()) },
            is_legacy: false,
            extension_list_url: None,
            resolved_index_url: "https://s/index.json".into(),
        };
        let card = ops.card(&store);
        assert_eq!(card.discord.as_deref(), Some("https://discord.gg/s"));
        assert_eq!(card.copy, "https://s/index.json");
        store.contact.discord = Some(String::new());
        assert_eq!(ops.card(&store).discord, None, "an empty link is no link");
        store.contact = Contact { website: String::new(), discord: None };
        let none = ops.card(&store);
        assert!(!stores::card_actions(&none.website, none.discord.as_deref()).globe_enabled);
    }

    fn repo_ops(routes: &[(&str, Result<&str, &str>)]) -> (tempfile::TempDir, ExtensionRepoRepo, RepoOps) {
        let (dir, db) = manga_db();
        let repos = ExtensionRepoRepo::new(db);
        let ops = RepoOps::new(repos.clone(), Canned::new(routes));
        (dir, repos, ops)
    }

    #[test]
    fn t_47b_020_a_valid_index_url_adds_the_repo_and_a_second_add_says_it_exists() {
        let (_dir, repos, ops) = repo_ops(&[("https://a.test/repo.json", Ok(REPO))]);
        assert!(matches!(ops.add("https://a.test/index.min.json"), AddOutcome::Added));
        assert_eq!(repos.all().expect("rows")[0].name, "Repo A");
        let AddOutcome::Notice(text) = ops.add("https://a.test/index.min.json") else { panic!("a repeat is a notice") };
        assert_eq!(text, "This repo already exists!");
    }

    #[test]
    fn t_47b_020_bad_urls_and_unreachable_repos_say_invalid_url() {
        let (_dir, _repos, ops) = repo_ops(&[]);
        for url in ["http://a.test/index.min.json", "https://a.test/index.min.json/", "nonsense", "https://a.test/index.min.json"] {
            let AddOutcome::Notice(text) = ops.add(url) else { panic!("{url} is a notice") };
            assert_eq!(text, "Invalid repo URL", "{url}");
        }
    }

    #[test]
    fn t_47b_021_a_second_repo_with_the_same_key_offers_a_replace_that_swaps_the_rows() {
        let (_dir, repos, ops) = repo_ops(&[("https://a.test/repo.json", Ok(REPO)), ("https://b.test/repo.json", Ok(REPO_B_SAME_KEY))]);
        assert!(matches!(ops.add("https://a.test/index.min.json"), AddOutcome::Added));
        let AddOutcome::Conflict { old_name, new_name, replace } = ops.add("https://b.test/index.min.json") else { panic!("same key conflicts") };
        assert_eq!((old_name.as_str(), new_name.as_str()), ("Repo A", "Repo B"));
        assert_eq!(repos.all().expect("rows").len(), 1, "nothing changes until Replace");
        replace();
        let rows = repos.all().expect("rows");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].base_url, "https://b.test");
    }

    #[test]
    fn t_47b_019_the_repo_card_copies_the_index_url_and_has_no_discord() {
        let (_dir, _repos, ops) = repo_ops(&[]);
        let card = ops.card(&ExtensionRepo {
            base_url: "https://a.test".into(),
            name: "A".into(),
            short_name: None,
            website: "https://a.test".into(),
            signing_key_fingerprint: "fp".into(),
        });
        assert_eq!(card.copy, "https://a.test/index.min.json");
        assert_eq!(card.discord, None);
        assert_eq!(repo_remove_body("A", "https://a.test"), "Do you wish to delete the repo \"https://a.test\"?");
    }

    #[test]
    fn t_47b_019_remove_deletes_by_base_url() {
        let (_dir, repos, ops) = repo_ops(&[("https://a.test/repo.json", Ok(REPO))]);
        let _ = ops.add("https://a.test/index.min.json");
        ops.remove("https://a.test");
        assert!(repos.all().expect("rows").is_empty());
    }
}
