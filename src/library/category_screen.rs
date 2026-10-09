//! The pure rules of the category management screen (T2-CATSCR-01 to T2-CATSCR-03): the name dialog, the dialog that
//! survives a refresh, and which categories the list shows. The widgets are in `src/live.rs`.

use kuro_domain::category::model::Category;

/// The Add or Rename dialog's state for a typed name, from `CategoryDialogs.kt`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NameCheck {
    /// OK is on for a non-empty name that no other category uses.
    pub confirm_enabled: bool,
    /// The helper line under the entry.
    pub helper: Helper,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Helper {
    /// "*required"
    Required,
    /// "A category with this name already exists!" (error style)
    Exists,
}

impl Helper {
    pub fn text(self) -> String {
        use kuro_i18n::{MsgKey, tr};
        tr(match self {
            Helper::Required => MsgKey::information_required_plain,
            Helper::Exists => MsgKey::error_category_exists,
        })
    }

    pub fn is_error(self) -> bool {
        self == Helper::Exists
    }
}

/// Checks `name` against the names `taken` by other categories. The match is exact: the name is not trimmed and
/// "Action" and "action" differ, as in Aniyomi (E-40).
pub fn check_name(name: &str, taken: &[&str]) -> NameCheck {
    let exists = !name.is_empty() && taken.contains(&name);
    NameCheck { confirm_enabled: !name.is_empty() && !exists, helper: if exists { Helper::Exists } else { Helper::Required } }
}

/// An open dialog of the screen: the category it works on, or none for Add.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Dialog {
    Add,
    Rename { id: i64, typed: String },
    Delete { id: i64 },
}

/// The dialog after the list refreshed (E-29). Aniyomi closes it on every emission and loses the typed text; here it
/// stays while its target category still exists.
pub fn after_refresh(dialog: Option<Dialog>, categories: &[Category]) -> Option<Dialog> {
    dialog.filter(|d| match d {
        Dialog::Add => true,
        Dialog::Rename { id, .. } | Dialog::Delete { id } => categories.iter().any(|c| c.id == *id),
    })
}

/// The categories the management list shows, in order. The system category is never listed. With the
/// "hide hidden categories" setting on, hidden ones are left out; with it off they show (E-43). The library page
/// always uses the visible ones, whatever this setting says.
pub fn management_list(categories: &[Category], hide_hidden: bool) -> Vec<&Category> {
    categories.iter().filter(|c| !c.is_system()).filter(|c| !(hide_hidden && c.hidden)).collect()
}
