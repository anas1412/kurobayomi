//! The bulk delete confirm of the Updates bar (T-42a-032): no title, the chapter or episode text, OK and Cancel. The
//! bulk bar's delete always asks; the per-row delete in the download indicator does not (parity, EC-42-U-08).

use kuro_platform::kind::Kind;
use kuro_i18n::{MsgKey, tr};

use crate::updates::copy;
use crate::updates::history_dialog::{ConfirmCopy, show};

/// The spec dialog of a bulk delete (`updates_delete_episodes`, `updates_delete_chapters`).
pub fn dialog_id(kind: Kind) -> &'static str {
    if kind.is_anime() { "updates_delete_episodes" } else { "updates_delete_chapters" }
}

/// The spec dialog of a bulk delete, when the Updates screen of the spec has it.
fn spec_dialog(kind: Kind) -> Option<crate::spec::Dialog> {
    crate::ui::with_spec(|spec| spec.screen("updates").and_then(|s| s.dialogs.iter().find(|d| d.id == dialog_id(kind)).cloned()))
}

/// The confirm text of a bulk delete: "...chapters?" for manga, "...episodes?" for anime. It is the title of the spec
/// dialog, and the catalogue text when the spec has none.
pub fn delete_confirm_text(kind: Kind) -> String {
    match spec_dialog(kind) {
        Some(dialog) if !dialog.title.is_empty() => dialog.title,
        _ if kind.is_anime() => copy::confirm_delete_episodes(),
        _ => copy::confirm_delete_chapters(),
    }
}

/// The Cancel and the confirm label: the first and the last button of the spec dialog, else the catalogue's.
fn buttons(kind: Kind) -> (String, String) {
    let spec = spec_dialog(kind).map(|d| d.buttons).unwrap_or_default();
    match (spec.first(), spec.last()) {
        (Some(cancel), Some(ok)) if spec.len() > 1 => (cancel.clone(), ok.clone()),
        _ => (tr(MsgKey::action_cancel), tr(MsgKey::action_ok)),
    }
}

/// Shows the confirm. `on_ok` runs only when OK is pressed.
pub fn confirm_bulk_delete(kind: Kind, on_ok: impl Fn() + 'static) {
    let (cancel, confirm) = buttons(kind);
    show(
        ConfirmCopy {
            title: String::new(),
            text: delete_confirm_text(kind),
            checkbox: None,
            confirm,
            cancel,
        },
        true,
        move |_| on_ok(),
        || {},
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_delete_confirm_text_per_kind() {
        assert_eq!(delete_confirm_text(Kind::Manga), "Are you sure you want to delete the selected chapters?");
        assert_eq!(delete_confirm_text(Kind::Anime), "Are you sure you want to delete the selected episodes?");
    }

    #[test]
    fn the_delete_dialogs_are_the_spec_dialogs() {
        assert_eq!(dialog_id(Kind::Anime), "updates_delete_episodes");
        assert_eq!(dialog_id(Kind::Manga), "updates_delete_chapters");
        for kind in Kind::ALL {
            let dialog = spec_dialog(kind).expect("the spec has the dialog");
            assert_eq!(delete_confirm_text(kind), dialog.title);
            assert_eq!(buttons(kind), ("Cancel".to_owned(), "OK".to_owned()));
        }
    }
}
