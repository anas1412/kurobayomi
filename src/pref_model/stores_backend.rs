use std::collections::BTreeSet;

use kuro_extensions::repo::{RepoError, add_link, remove_link};
use kuro_i18n::{Arg, MsgKey, tr, trf};
use kuro_prefs::pref::Pref;

use super::stores::RepoLink;

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

#[allow(dead_code)] // kept for the shape of the type; not built or read yet
/// How an add ended.
pub enum AddOutcome {
    Added,
    /// The dialog stays open and shows this text (stores).
    Failed(String),
    /// The dialog closes and the toast shows this text (repos).
    Notice(String),
}

/// The operations of one kind of list. They block, so the page calls them off the GTK thread.
pub trait Ops<T>: Send + Sync + 'static {
    fn card(&self, item: &T) -> Card;
    fn add(&self, url: &str) -> AddOutcome;
    /// Re-fetches every stored entry. A failing one is logged and skipped.
    fn refresh(&self);
    fn remove(&self, key: &str);
    /// Whether the add dialog closes as soon as Add is pressed (repos, as in Kotlin) instead of staying open with
    /// "Processing…" until the fetch ends (stores). The result then comes as a toast or the conflict dialog.
    fn closes_on_submit(&self) -> bool {
        false
    }
    /// The text of the busy card the page shows over the window while `add` runs. `None` shows none.
    fn busy_text(&self) -> Option<String> {
        None
    }
}

/// The busy card text while a repo link's index and sources download.
pub fn adding_repo() -> String {
    tr(MsgKey::repos_adding_repo)
}

/// The toast of an add that failed for a reason the domain does not name (a deliberate deviation: Kotlin is silent).
pub fn could_not_add_repo() -> String {
    tr(MsgKey::repos_could_not_add_repo)
}

/// The repository links of the Browse page (`docs/design/app-flow.md`): one https link to an `index.min.json` lists the
/// sources. The links are kept in the `[sources]` preference, so a backup carries them.
pub struct RepoOps {
    links: Pref<BTreeSet<String>>,
}

impl RepoOps {
    pub fn new(links: Pref<BTreeSet<String>>) -> Self {
        Self { links }
    }
}

/// The toast after a repo link was added: how many sources its index lists.
pub fn added_text(count: usize) -> String {
    kuro_i18n::tr_plural(MsgKey::repos_added, i64::try_from(count).unwrap_or(i64::MAX), &[])
}

/// The toast text of a repo link that could not be added.
pub fn repo_error_text(error: &RepoError) -> String {
    match error {
        RepoError::BadRepoLink => tr(MsgKey::invalid_repo_name),
        RepoError::Network(_) | RepoError::Status(_) => tr(MsgKey::ui_the_repo_could_not_be_reached).to_owned(),
        RepoError::TooMany { .. } => tr(MsgKey::ui_the_repo_lists_more_than_10_sources_of_one_k).to_owned(),
        RepoError::Mismatch => tr(MsgKey::ui_a_source_does_not_match_its_checksum_in_the).to_owned(),
        RepoError::Write(_) => could_not_add_repo().to_owned(),
        RepoError::Parse(_) | RepoError::BadId(_) | RepoError::BadChecksum(_) | RepoError::BadLink(_) | RepoError::TooLarge => {
            tr(MsgKey::ui_the_repo_index_is_not_valid).to_owned()
        }
    }
}

impl Ops<RepoLink> for RepoOps {
    fn closes_on_submit(&self) -> bool {
        true
    }

    fn busy_text(&self) -> Option<String> {
        Some(adding_repo())
    }

    fn card(&self, repo: &RepoLink) -> Card {
        repo_card(&repo.0, crate::recipe_sources::saved_index(&repo.0).as_ref())
    }

    /// Fetches the index and every source it lists, then saves the link. Nothing is saved when any of it fails.
    fn add(&self, url: &str) -> AddOutcome {
        let link = url.trim();
        if !(link.starts_with("https://") && link.ends_with(".json")) {
            return AddOutcome::Notice(tr(MsgKey::invalid_repo_name));
        }
        let mut links: Vec<String> = self.links.get().into_iter().collect();
        if links.iter().any(|known| known.as_str() == link) {
            return AddOutcome::Notice(tr(MsgKey::error_repo_exists));
        }
        let count = match crate::recipe_sources::sync_link(link) {
            Ok(count) => count,
            Err(error) => {
                tracing::warn!(%error, link, "adding the repo failed");
                return AddOutcome::Notice(repo_error_text(&error));
            }
        };
        if let Err(error) = add_link(&mut links, link) {
            return AddOutcome::Notice(repo_error_text(&error));
        }
        if let Err(error) = self.links.set(links.into_iter().collect()) {
            tracing::error!(%error, link, "saving the repo link failed");
            return AddOutcome::Notice(could_not_add_repo().to_owned());
        }
        crate::recipe_sources::reload();
        crate::toast::notify(added_text(count));
        AddOutcome::Added
    }

    /// Fetches every saved link again. A link that fails keeps the copy it has.
    fn refresh(&self) {
        for link in self.links.get() {
            if let Err(error) = crate::recipe_sources::sync_link(&link) {
                tracing::error!(%error, link = link.as_str(), "refreshing the repo failed; its saved copy is kept");
            }
        }
        crate::recipe_sources::reload();
    }

    fn remove(&self, key: &str) {
        let mut links: Vec<String> = self.links.get().into_iter().collect();
        if !remove_link(&mut links, key) {
            return;
        }
        if let Err(error) = self.links.set(links.into_iter().collect()) {
            tracing::error!(%error, key, "removing the repo link failed");
            return;
        }
        crate::recipe_sources::forget_link(key);
        crate::recipe_sources::reload();
    }
}

/// A repo's card: the name and web page its saved index gives, or the link itself before the index is saved or when the
/// index names none. The copy button always copies the link.
fn repo_card(link: &str, index: Option<&kuro_extensions::repo::Index>) -> Card {
    let name = index.and_then(|i| i.name.clone()).filter(|n| !n.trim().is_empty()).unwrap_or_else(|| link.to_owned());
    let website = index.and_then(|i| i.website.clone()).filter(|w| w.starts_with("https://")).unwrap_or_default();
    Card { key: link.to_owned(), name, website, discord: None, copy: link.to_owned(), note: None }
}

/// The body of the delete dialog for a repo: it names the base URL only.
pub fn repo_remove_body(_name: &str, base_url: &str) -> String {
    trf(MsgKey::delete_repo_confirmation, &[("arg1", Arg::Str(base_url))])
}

#[cfg(test)]
mod tests {
    use kuro_platform::kind::Kind;
    use kuro_prefs::source::SourcePrefs;
    use kuro_prefs::store::PrefStore;

    use super::*;

    fn repo_ops(links: &[&str]) -> (Pref<BTreeSet<String>>, RepoOps) {
        let saved = SourcePrefs::new(PrefStore::in_memory()).manga_extension_repos;
        saved.set(links.iter().map(|link| (*link).to_owned()).collect()).expect("seed the links");
        let ops = RepoOps::new(saved.clone());
        (saved, ops)
    }

    // T-47b-020: a link is https and ends in .json; anything else is "Invalid repo URL" and nothing is saved
    #[test]
    fn t_47b_020_a_link_that_is_not_https_json_says_invalid_repo_url() {
        let (saved, ops) = repo_ops(&[]);
        for url in ["http://a.test/index.min.json", "https://a.test/index.min.json/", "nonsense", "https://a.test/index.min.json?x=1"] {
            let AddOutcome::Notice(text) = ops.add(url) else { panic!("{url} is a notice") };
            assert_eq!(text, "Invalid repo URL", "{url}");
        }
        assert!(saved.get().is_empty(), "nothing is saved");
    }

    #[test]
    fn t_47b_020_a_link_already_added_says_so() {
        let (saved, ops) = repo_ops(&["https://a.test/index.min.json"]);
        let AddOutcome::Notice(text) = ops.add("https://a.test/index.min.json") else { panic!("a repeat is a notice") };
        assert_eq!(text, "This repo already exists!");
        assert_eq!(saved.get().len(), 1);
    }

    #[test]
    fn t_47b_020_an_unreachable_index_is_a_notice_and_nothing_is_saved() {
        let (saved, ops) = repo_ops(&[]);
        assert!(matches!(ops.add("https://127.0.0.1:9/index.min.json"), AddOutcome::Notice(_)));
        assert!(saved.get().is_empty());
    }

    #[test]
    fn t_47b_020_each_failure_has_its_toast_text_and_the_dialog_closes_on_submit() {
        assert_eq!(repo_error_text(&RepoError::Network("refused".into())), "The repo could not be reached");
        assert_eq!(repo_error_text(&RepoError::Status(404)), "The repo could not be reached");
        assert_eq!(repo_error_text(&RepoError::TooMany { kind: Kind::Manga, count: 11, limit: 10 }), "The repo lists more than 10 sources of one kind");
        assert!(repo_error_text(&RepoError::Parse("x".into())).starts_with("This is not a valid Kurobayomi repository"));
        assert_eq!(repo_error_text(&RepoError::TooLarge), repo_error_text(&RepoError::Parse("x".into())), "a Mihon index.json is too large: same message");
        assert_eq!(repo_error_text(&RepoError::Mismatch), "A source does not match its checksum in the repo index");
        assert_eq!(repo_error_text(&RepoError::Write("disk".into())), could_not_add_repo());
        let (_saved, ops) = repo_ops(&[]);
        assert!(ops.closes_on_submit(), "T-47b-024: Add closes the dialog at once");
        assert_eq!(ops.busy_text(), Some("Adding repository…".to_owned()));
        assert_eq!((added_text(1), added_text(3)), ("Added 1 source".to_owned(), "Added 3 sources".to_owned()));
    }

    #[test]
    fn t_47b_019_the_repo_card_copies_the_link_and_has_no_discord() {
        let (_saved, ops) = repo_ops(&[]);
        let card = ops.card(&RepoLink("https://a.test/index.min.json".into()));
        assert_eq!(card.copy, "https://a.test/index.min.json");
        assert_eq!(card.discord, None);
        assert_eq!(repo_remove_body("A", "https://a.test/index.min.json"), "Do you wish to delete the repo \"https://a.test/index.min.json\"?");
    }

    #[test]
    fn the_repo_card_shows_the_index_name_and_website() {
        let link = "https://a.test/index.min.json";
        let named = kuro_extensions::repo::parse_index(r#"{"version":1,"name":"My Repo","website":"https://github.com/a/b","sources":[]}"#).expect("valid");
        let card = repo_card(link, Some(&named));
        assert_eq!((card.name.as_str(), card.website.as_str(), card.copy.as_str()), ("My Repo", "https://github.com/a/b", link));
        let bare = kuro_extensions::repo::parse_index(r#"{"version":1,"website":"javascript:x","sources":[]}"#).expect("valid");
        let card = repo_card(link, Some(&bare));
        assert_eq!((card.name.as_str(), card.website.as_str()), (link, ""), "no name: the link; a non-https page: no globe");
        assert_eq!(repo_card(link, None).name, link);
    }

    #[test]
    fn t_47b_019_remove_deletes_the_link() {
        let (saved, ops) = repo_ops(&["https://a.test/index.min.json", "https://b.test/index.min.json"]);
        ops.remove("https://a.test/index.min.json");
        assert_eq!(saved.get().into_iter().collect::<Vec<_>>(), vec!["https://b.test/index.min.json".to_owned()]);
    }
}
