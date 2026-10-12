//! The repository list page (T-47b-009 to T-47b-019). Pure: the page state machine, the add and deep-link
//! entry rules, and what each card offers. Storage reads and network fetches are kuro-domain's; the GTK page runs
//! the returned [`Effect`] and feeds the result back as an [`Event`].

use kuro_i18n::{MsgKey, tr};

/// Store or repo identity: the key the database holds it by (`index_url`, or the repo link).
pub trait Keyed: Clone {
    fn key(&self) -> &str;
}

/// One repository link (an `index.min.json` address), keyed by the link itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RepoLink(pub String);

impl Keyed for RepoLink {
    fn key(&self) -> &str {
        &self.0
    }
}

/// Shown when a failed add has no message of its own.
pub fn unknown_error() -> String {
    tr(MsgKey::unknown_error)
}

/// The URL entry of the add and deep-link dialogs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub input: String,
    /// Set synchronously when Add is pressed; a second press is ignored until the fetch ends.
    pub processing: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    Add(Entry),
    /// Read-only URL entry, opened from a deep link.
    DeepLink(Entry),
    Remove { key: String, name: String, url: String },
}

#[derive(Clone, Debug)]
pub enum Page<T> {
    Loading,
    Ready { items: Vec<T>, dialog: Option<Dialog>, refreshing: bool },
}

#[derive(Clone, Debug)]
pub enum Event<T> {
    /// The database emitted the current list (the first one ends Loading).
    Loaded(Vec<T>),
    Open(Dialog),
    Dismiss,
    Type(String),
    /// Add or deep-link OK.
    Submit,
    AddSucceeded,
    AddFailed(String),
    /// Remove OK.
    ConfirmRemove,
    Refresh,
    RefreshDone,
}

/// What the GTK page must do after a transition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    None,
    /// Fetch the store at this URL, then send `AddSucceeded` or `AddFailed`.
    Fetch(String),
    /// Delete the row, then reload available extensions.
    Delete(String),
    /// Re-fetch every stored index; each failing store is logged and skipped.
    RefreshAll,
}

/// The helper line under the URL entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Helper {
    Required,
    Duplicate,
    Error(String),
    Plain,
}

/// Add is enabled only when the entry is non-empty, not a duplicate, and not processing.
pub fn add_enabled(entry: &Entry, existing_keys: &[&str]) -> bool {
    !entry.processing && !entry.input.trim().is_empty() && !existing_keys.contains(&entry.input.trim())
}

pub fn add_label(entry: &Entry) -> String {
    tr(if entry.processing { MsgKey::extensionStoresScreen_addStore_processing } else { MsgKey::action_add })
}

pub fn helper(entry: &Entry, existing_keys: &[&str]) -> Helper {
    if entry.input.trim().is_empty() {
        Helper::Required
    } else if existing_keys.contains(&entry.input.trim()) {
        Helper::Duplicate
    } else if let Some(error) = &entry.error {
        Helper::Error(error.clone())
    } else {
        Helper::Plain
    }
}

/// The text under the URL entry: empty normally; a duplicate or an error fills it and is drawn as an
/// error ([`Helper::is_error`]).
pub fn helper_text(helper: &Helper) -> String {
    match helper {
        // nothing under an empty or valid link: the disabled Add button already says a link is needed
        Helper::Required | Helper::Plain => String::new(),
        Helper::Duplicate => tr(MsgKey::extensionStoresScreen_addStore_alreadyExists),
        Helper::Error(text) => text.clone(),
    }
}

impl Helper {
    pub fn is_error(&self) -> bool {
        matches!(self, Helper::Duplicate | Helper::Error(_))
    }
}

/// What a store card offers, in order: globe (website), Discord (only with a Discord link), copy, delete.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardActions {
    pub globe_enabled: bool,
    pub discord: Option<String>,
}

pub fn card_actions(website: &str, discord: Option<&str>) -> CardActions {
    CardActions {
        globe_enabled: !website.is_empty(),
        discord: discord.filter(|d| !d.is_empty()).map(str::to_owned),
    }
}

/// Applies one event. Refresh runs only in Ready; a list emission keeps the open dialog, except a Remove dialog
/// whose row is gone.
pub fn reduce<T: Keyed>(state: &mut Page<T>, event: Event<T>) -> Effect {
    match (&mut *state, event) {
        (Page::Loading, Event::Loaded(items)) => {
            *state = Page::Ready { items, dialog: None, refreshing: false };
            Effect::None
        }
        (Page::Ready { items, dialog, .. }, Event::Loaded(next)) => {
            *items = next;
            if let Some(Dialog::Remove { key, .. }) = dialog {
                if !items.iter().any(|i| i.key() == key.as_str()) {
                    *dialog = None;
                }
            }
            Effect::None
        }
        (Page::Ready { dialog, .. }, Event::Open(d)) => {
            *dialog = Some(d);
            Effect::None
        }
        (Page::Ready { dialog, .. }, Event::Dismiss) => {
            *dialog = None;
            Effect::None
        }
        (Page::Ready { dialog: Some(Dialog::Add(e) | Dialog::DeepLink(e)), .. }, Event::Type(text)) => {
            e.input = text;
            e.error = None;
            Effect::None
        }
        (Page::Ready { items, dialog: Some(Dialog::Add(e) | Dialog::DeepLink(e)), .. }, Event::Submit) => {
            let keys: Vec<&str> = items.iter().map(Keyed::key).collect();
            if !add_enabled(e, &keys) {
                return Effect::None;
            }
            e.processing = true;
            e.error = None;
            Effect::Fetch(e.input.trim().to_owned())
        }
        (Page::Ready { dialog, .. }, Event::AddSucceeded) => {
            *dialog = None;
            Effect::None
        }
        (Page::Ready { dialog: Some(Dialog::Add(e) | Dialog::DeepLink(e)), .. }, Event::AddFailed(msg)) => {
            e.processing = false;
            e.error = Some(if msg.is_empty() { unknown_error() } else { msg });
            Effect::None
        }
        (Page::Ready { items, dialog, .. }, Event::ConfirmRemove) => {
            let key = match dialog {
                Some(Dialog::Remove { key, .. }) => key.clone(),
                _ => return Effect::None,
            };
            *dialog = None;
            items.retain(|i| i.key() != key);
            Effect::Delete(key)
        }
        (Page::Ready { items: _, dialog: _, refreshing }, Event::Refresh) if !*refreshing => {
            *refreshing = true;
            Effect::RefreshAll
        }
        (Page::Ready { refreshing, .. }, Event::RefreshDone) => {
            *refreshing = false;
            Effect::None
        }
        _ => Effect::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug)]
    struct Repo(String);

    impl Keyed for Repo {
        fn key(&self) -> &str {
            &self.0
        }
    }

    fn ready(items: Vec<Repo>) -> Page<Repo> {
        Page::Ready { items, dialog: None, refreshing: false }
    }

    #[test]
    fn loading_ends_at_the_first_emission_and_empty_is_ready() {
        let mut page: Page<Repo> = Page::Loading;
        reduce(&mut page, Event::Loaded(vec![]));
        assert!(matches!(page, Page::Ready { ref items, .. } if items.is_empty()));
    }

    #[test]
    fn an_emission_keeps_the_open_dialog() {
        let mut page = ready(vec![Repo("a".into())]);
        reduce(&mut page, Event::Open(Dialog::Add(Entry::default())));
        reduce(&mut page, Event::Loaded(vec![Repo("a".into()), Repo("b".into())]));
        assert!(matches!(page, Page::Ready { dialog: Some(Dialog::Add(_)), .. }));
    }

    #[test]
    fn an_emission_closes_a_remove_dialog_whose_row_is_gone() {
        let mut page = ready(vec![Repo("a".into())]);
        reduce(&mut page, Event::Open(Dialog::Remove { key: "a".into(), name: "A".into(), url: "a".into() }));
        reduce(&mut page, Event::Loaded(vec![]));
        assert!(matches!(page, Page::Ready { dialog: None, .. }));
    }

    #[test]
    fn add_sets_processing_once_and_a_second_press_is_ignored() {
        let mut page = ready(vec![]);
        reduce(&mut page, Event::Open(Dialog::Add(Entry::default())));
        reduce(&mut page, Event::Type("https://x/index.min.json".into()));
        assert_eq!(reduce(&mut page, Event::Submit), Effect::Fetch("https://x/index.min.json".into()));
        assert_eq!(reduce(&mut page, Event::Submit), Effect::None);
    }

    #[test]
    fn failure_stores_the_text_and_reenables_add() {
        let mut page = ready(vec![]);
        reduce(&mut page, Event::Open(Dialog::Add(Entry { input: "u".into(), ..Entry::default() })));
        reduce(&mut page, Event::Submit);
        reduce(&mut page, Event::AddFailed(String::new()));
        let Page::Ready { dialog: Some(Dialog::Add(e)), .. } = &page else { panic!("dialog stays open") };
        assert!(!e.processing);
        assert_eq!(e.error.as_deref(), Some("Unknown error"));
    }

    #[test]
    fn success_closes_the_add_dialog() {
        let mut page = ready(vec![]);
        reduce(&mut page, Event::Open(Dialog::Add(Entry { input: "u".into(), ..Entry::default() })));
        reduce(&mut page, Event::Submit);
        reduce(&mut page, Event::AddSucceeded);
        assert!(matches!(page, Page::Ready { dialog: None, .. }));
    }

    #[test]
    fn duplicate_and_empty_entries_cannot_be_added() {
        let existing = ["https://x/index.min.json"];
        let dup = Entry { input: "https://x/index.min.json".into(), ..Entry::default() };
        assert!(!add_enabled(&dup, &existing));
        assert_eq!(helper(&dup, &existing), Helper::Duplicate);
        assert_eq!(helper(&Entry::default(), &existing), Helper::Required);
        let busy = Entry { input: "u".into(), processing: true, error: None };
        assert_eq!(add_label(&busy), "Processing…");
        assert!(!add_enabled(&busy, &existing));
    }

    #[test]
    fn remove_drops_the_row_and_closes_at_once() {
        let mut page = ready(vec![Repo("a".into()), Repo("b".into())]);
        reduce(&mut page, Event::Open(Dialog::Remove { key: "a".into(), name: "A".into(), url: "a".into() }));
        assert_eq!(reduce(&mut page, Event::ConfirmRemove), Effect::Delete("a".into()));
        let Page::Ready { items, dialog, .. } = &page else { panic!() };
        assert_eq!(items.iter().map(Keyed::key).collect::<Vec<_>>(), vec!["b"]);
        assert!(dialog.is_none());
    }

    #[test]
    fn refresh_runs_only_in_ready_and_not_twice() {
        let mut loading: Page<Repo> = Page::Loading;
        assert_eq!(reduce(&mut loading, Event::Refresh), Effect::None);
        let mut page = ready(vec![]);
        assert_eq!(reduce(&mut page, Event::Refresh), Effect::RefreshAll);
        assert_eq!(reduce(&mut page, Event::Refresh), Effect::None);
        reduce(&mut page, Event::RefreshDone);
        assert!(matches!(page, Page::Ready { refreshing: false, .. }));
    }

    #[test]
    fn card_offers_discord_only_when_set_and_globe_only_with_a_website() {
        let none = card_actions("", None);
        assert!(!none.globe_enabled);
        assert_eq!(none.discord, None);
        let full = card_actions("https://site", Some("https://discord.gg/x"));
        assert!(full.globe_enabled);
        assert_eq!(full.discord.as_deref(), Some("https://discord.gg/x"));
        assert_eq!(card_actions("s", Some("")).discord, None);
    }

    #[test]
    fn t_47b_016_the_helper_line_is_required_duplicate_or_the_error_text() {
        let existing = ["https://x/index.min.json"];
        let required = helper(&Entry::default(), &existing);
        assert_eq!(helper_text(&required), "");
        assert!(!required.is_error());
        let typed = Entry { input: "https://y/index.min.json".into(), ..Entry::default() };
        assert_eq!(helper_text(&helper(&typed, &existing)), "", "a fresh link shows nothing under it");
        let dup = helper(&Entry { input: "https://x/index.min.json".into(), ..Entry::default() }, &existing);
        assert_eq!(helper_text(&dup), "This extension store already exists");
        assert!(dup.is_error());
        let failed = helper(&Entry { input: "u".into(), processing: false, error: Some("HTTP 404".into()) }, &existing);
        assert_eq!(helper_text(&failed), "HTTP 404");
        assert!(failed.is_error());
        assert_eq!(add_label(&Entry::default()), "Add");
    }

    fn repo(base: &str) -> RepoLink {
        RepoLink(base.to_owned())
    }

    #[test]
    fn t_47b_019_a_repo_emission_keeps_the_open_dialog_unless_its_row_was_deleted() {
        let mut page = Page::Ready { items: vec![repo("https://a"), repo("https://b")], dialog: None, refreshing: false };
        reduce(&mut page, Event::Open(Dialog::Add(Entry { input: "typed".into(), ..Entry::default() })));
        reduce(&mut page, Event::Loaded(vec![repo("https://a"), repo("https://b"), repo("https://c")]));
        let Page::Ready { dialog: Some(Dialog::Add(e)), .. } = &page else { panic!("RACE-02: the add dialog stays open") };
        assert_eq!(e.input, "typed", "the typed text survives the emission");
        reduce(&mut page, Event::Open(Dialog::Remove { key: "https://b".into(), name: "b".into(), url: "https://b".into() }));
        reduce(&mut page, Event::Loaded(vec![repo("https://a")]));
        assert!(matches!(page, Page::Ready { dialog: None, .. }), "the deleted row's dialog closes");
    }

    #[test]
    fn t_47b_019_a_repo_page_is_loading_until_the_first_emission() {
        let mut page: Page<RepoLink> = Page::Loading;
        assert_eq!(reduce(&mut page, Event::Open(Dialog::Add(Entry::default()))), Effect::None);
        assert!(matches!(page, Page::Loading), "no dialog opens while loading");
        reduce(&mut page, Event::Loaded(vec![repo("https://a")]));
        assert!(matches!(page, Page::Ready { ref items, .. } if items.len() == 1));
    }

    #[test]
    fn t_47b_019_adding_a_repo_blocks_a_stored_base_url_and_remove_deletes_by_base_url() {
        let mut page = Page::Ready { items: vec![repo("https://a")], dialog: None, refreshing: false };
        reduce(&mut page, Event::Open(Dialog::Add(Entry { input: "https://a".into(), ..Entry::default() })));
        assert_eq!(reduce(&mut page, Event::Submit), Effect::None, "a stored URL is a duplicate");
        reduce(&mut page, Event::Open(Dialog::Remove { key: "https://a".into(), name: "a".into(), url: "https://a".into() }));
        assert_eq!(reduce(&mut page, Event::ConfirmRemove), Effect::Delete("https://a".into()));
    }

}
