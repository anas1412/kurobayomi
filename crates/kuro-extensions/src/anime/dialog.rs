//! The state of the stores page dialogs (T-19a-049, `p1-ui-parity.md` section 4, `p1-rust-design.md` section 11.2).
//! A pure value with no GTK: the window draws it and sends the button presses back. The enable rules are the ones in
//! `store::rules`.

use crate::anime::model::StoreRow;
use crate::anime::store::rules::{
    PROCESSING_LABEL, confirm_add_enabled, create_add_enabled, delete_body,
};

/// Shown on the empty stores page (E-UI-07).
pub const EMPTY_LABEL: &str = "You haven't added an extension store yet";
/// Supporting text of the Create dialog when the field needs input.
pub const REQUIRED_TEXT: &str = "*required";
/// Shown under the Create field or in the Confirm dialog when the store is already stored.
pub const ALREADY_EXISTS_TEXT: &str = crate::anime::messages::ALREADY_EXISTS;

/// The one dialog open on the stores page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DialogState {
    Closed,
    /// Add a store by typed URL.
    Create {
        text: String,
        processing: bool,
        error: Option<String>,
    },
    /// Add a store that a deep link named. The URL is read-only.
    Confirm {
        url: String,
        processing: bool,
        error: Option<String>,
    },
    /// Remove a stored store.
    Delete {
        name: String,
        url: String,
    },
}

/// The empty-state text when no store is stored (E-UI-07).
pub fn empty_text(stores: &[StoreRow]) -> Option<&'static str> {
    stores.is_empty().then_some(EMPTY_LABEL)
}

impl DialogState {
    pub fn create() -> Self {
        DialogState::Create {
            text: String::new(),
            processing: false,
            error: None,
        }
    }

    /// A deep link opens Confirm with the link's URL, trimmed (E-UI-02).
    pub fn confirm(url: &str) -> Self {
        DialogState::Confirm {
            url: url.trim().to_owned(),
            processing: false,
            error: None,
        }
    }

    pub fn delete(store: &StoreRow) -> Self {
        DialogState::Delete {
            name: store.name.clone(),
            url: store.index_url.clone(),
        }
    }

    /// The typed text changed. An error from an earlier attempt goes away.
    pub fn set_text(&mut self, typed: &str) {
        if let DialogState::Create { text, error, .. } = self {
            *text = typed.to_owned();
            *error = None;
        }
    }

    /// The URL that Add would store: the typed text or the deep link URL, trimmed (E-UI-02).
    pub fn url(&self) -> Option<&str> {
        match self {
            DialogState::Create { text, .. } => Some(text.trim()),
            DialogState::Confirm { url, .. } => Some(url.as_str()),
            DialogState::Closed | DialogState::Delete { .. } => None,
        }
    }

    /// True when the URL is already a stored store (E-UI-01, E-UI-03). The check is made on the trimmed URL, and it
    /// is made again on every call, so a store list that changes while the dialog is open is read at once (E-UI-06).
    pub fn is_duplicate(&self, stores: &[StoreRow]) -> bool {
        match self.url() {
            Some(url) if !url.is_empty() => stores.iter().any(|store| store.index_url == url),
            _ => false,
        }
    }

    /// True when the Add button is enabled.
    pub fn add_enabled(&self, stores: &[StoreRow]) -> bool {
        let duplicate = self.is_duplicate(stores);
        match self {
            DialogState::Create {
                text, processing, ..
            } => create_add_enabled(text, duplicate, *processing),
            DialogState::Confirm { processing, .. } => confirm_add_enabled(duplicate, *processing),
            DialogState::Closed | DialogState::Delete { .. } => false,
        }
    }

    /// The label of the Add button.
    pub fn add_label(&self) -> &'static str {
        match self {
            DialogState::Create {
                processing: true, ..
            }
            | DialogState::Confirm {
                processing: true, ..
            } => PROCESSING_LABEL,
            _ => "Add",
        }
    }

    /// The text under the field or the body: the duplicate text first, then the error, then `*required` for an empty
    /// Create field.
    pub fn supporting_text(&self, stores: &[StoreRow]) -> Option<String> {
        if self.is_duplicate(stores) {
            return Some(ALREADY_EXISTS_TEXT.to_owned());
        }
        match self {
            DialogState::Create { text, error, .. } => error
                .clone()
                .or_else(|| text.trim().is_empty().then(|| REQUIRED_TEXT.to_owned())),
            DialogState::Confirm { error, .. } => error.clone(),
            DialogState::Closed | DialogState::Delete { .. } => None,
        }
    }

    /// The body of the Delete dialog.
    pub fn delete_text(&self) -> Option<String> {
        match self {
            DialogState::Delete { name, url } => Some(delete_body(name, url)),
            _ => None,
        }
    }

    /// Starts an add. Returns the URL to fetch, or `None` when Add is disabled. Processing is set before the job starts,
    /// so a second press while the first runs returns `None` (E-UI-05).
    pub fn begin_add(&mut self, stores: &[StoreRow]) -> Option<String> {
        if !self.add_enabled(stores) {
            return None;
        }
        let url = self.url()?.to_owned();
        match self {
            DialogState::Create {
                processing, error, ..
            }
            | DialogState::Confirm {
                processing, error, ..
            } => {
                *processing = true;
                *error = None;
            }
            DialogState::Closed | DialogState::Delete { .. } => return None,
        }
        Some(url)
    }

    /// The add failed. Processing stops and the fixed text is shown.
    pub fn fail(&mut self, message: &str) {
        match self {
            DialogState::Create {
                processing, error, ..
            }
            | DialogState::Confirm {
                processing, error, ..
            } => {
                *processing = false;
                *error = Some(message.to_owned());
            }
            DialogState::Closed | DialogState::Delete { .. } => {}
        }
    }

    /// Closes the dialog after a successful add, a delete, or a cancel.
    pub fn close(&mut self) {
        *self = DialogState::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::anime::messages;

    fn store(index_url: &str) -> StoreRow {
        StoreRow {
            index_url: index_url.into(),
            name: "Example".into(),
            badge_label: "EX".into(),
            signing_key: "ab".repeat(32),
            contact_website: String::new(),
            contact_discord: None,
            is_legacy: false,
            extension_list_url: None,
        }
    }

    #[test]
    fn tst_19_115_a_duplicate_url_disables_add_and_says_so() {
        let stores = [store("https://example.org/index.json")];
        let mut create = DialogState::create();
        create.set_text("https://example.org/index.json");
        assert!(!create.add_enabled(&stores));
        assert_eq!(
            create.supporting_text(&stores).as_deref(),
            Some("This extension store already exists")
        );

        let confirm = DialogState::confirm("https://example.org/index.json");
        assert!(!confirm.add_enabled(&stores));
        assert_eq!(
            confirm.supporting_text(&stores).as_deref(),
            Some("This extension store already exists")
        );
    }

    #[test]
    fn the_create_add_button_needs_text() {
        let mut create = DialogState::create();
        assert!(!create.add_enabled(&[]));
        assert_eq!(create.supporting_text(&[]).as_deref(), Some("*required"));
        create.set_text("   ");
        assert!(!create.add_enabled(&[]), "blank text is not a URL");
        create.set_text("https://example.org/new.json");
        assert!(create.add_enabled(&[]));
        assert_eq!(create.supporting_text(&[]), None);
    }

    #[test]
    fn tst_19_116_the_url_is_trimmed_before_the_duplicate_check_and_the_fetch() {
        let stores = [store("https://x/repo.json")];
        let mut create = DialogState::create();
        create.set_text(" https://x/repo.json ");
        assert_eq!(create.url(), Some("https://x/repo.json"));
        assert!(create.is_duplicate(&stores));
        assert!(!create.add_enabled(&stores));

        create.set_text(" https://x/new.json ");
        assert_eq!(create.begin_add(&[]).as_deref(), Some("https://x/new.json"));
    }

    #[test]
    fn tst_19_117_a_deep_link_to_a_stored_url_shows_the_duplicate_text() {
        let link = "aniyomi://add-repo?url=https%3A%2F%2Fexample.org%2Frepo.json";
        let url = crate::anime::store::link::store_url_from_link(link).expect("a store link");
        let stores = [store("https://example.org/repo.json")];
        let confirm = DialogState::confirm(&url);
        assert!(confirm.is_duplicate(&stores));
        assert!(!confirm.add_enabled(&stores));
        assert_eq!(
            confirm.supporting_text(&stores).as_deref(),
            Some(messages::ALREADY_EXISTS)
        );
    }

    #[test]
    fn tst_19_119_a_second_confirm_press_while_processing_starts_no_second_job() {
        let stores: [StoreRow; 0] = [];
        let mut confirm = DialogState::confirm("https://example.org/new.json");
        assert_eq!(
            confirm.begin_add(&stores).as_deref(),
            Some("https://example.org/new.json")
        );
        assert_eq!(confirm.add_label(), "Processing…");
        assert_eq!(confirm.begin_add(&stores), None);
        confirm.fail("Failed to add extension store");
        assert_eq!(confirm.add_label(), "Add");
        assert!(
            confirm.begin_add(&stores).is_some(),
            "a failed add can be tried again"
        );
    }

    #[test]
    fn tst_19_120_a_store_list_change_keeps_the_dialog_open_with_its_text() {
        let mut create = DialogState::create();
        create.set_text("https://example.org/index.json");
        let before = create.clone();
        let changed = [store("https://example.org/index.json")];
        assert!(create.is_duplicate(&changed));
        assert_eq!(
            create, before,
            "the dialog state is not reset by a store change"
        );
        assert!(!create.add_enabled(&changed));
    }

    #[test]
    fn tst_19_121_an_empty_store_list_shows_the_empty_text() {
        assert_eq!(
            empty_text(&[]),
            Some("You haven't added an extension store yet")
        );
        assert_eq!(empty_text(&[store("https://x/i.json")]), None);
    }

    #[test]
    fn the_delete_dialog_quotes_the_name_and_url() {
        let delete = DialogState::delete(&store("https://x/i.json"));
        assert_eq!(
            delete.delete_text().as_deref(),
            Some("Do you wish to remove the extension store \"Example\" (https://x/i.json)?")
        );
        assert_eq!(DialogState::Closed.delete_text(), None);
    }

    #[test]
    fn closing_clears_the_state() {
        let mut create = DialogState::create();
        create.set_text("x");
        create.close();
        assert_eq!(create, DialogState::Closed);
    }
}
