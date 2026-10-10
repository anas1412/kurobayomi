//! The bulk delete confirm of the Updates bar (T-42a-032): no title, the chapter or episode text, OK and Cancel. The
//! bulk bar's delete always asks; the per-row delete in the download indicator does not (parity, EC-42-U-08).

use kuro_i18n::{MsgKey, tr};

use crate::updates::copy;
use crate::updates::history_dialog::{ConfirmCopy, show};

/// The spec dialog of a bulk delete (`updates_delete_episodes`, `updates_delete_chapters`).
pub fn dialog_id(anime: bool) -> &'static str {
    if anime { "updates_delete_episodes" } else { "updates_delete_chapters" }
}

/// The spec dialog of a bulk delete, when the Updates screen of the spec has it.
fn spec_dialog(anime: bool) -> Option<crate::spec::Dialog> {
    crate::ui::with_spec(|spec| spec.screen("updates").and_then(|s| s.dialogs.iter().find(|d| d.id == dialog_id(anime)).cloned()))
}

/// The confirm text of a bulk delete: "...chapters?" for manga, "...episodes?" for anime. It is the title of the spec
/// dialog, and the catalogue text when the spec has none.
pub fn delete_confirm_text(anime: bool) -> String {
    match spec_dialog(anime) {
        Some(dialog) if !dialog.title.is_empty() => dialog.title,
        _ if anime => copy::confirm_delete_episodes(),
        _ => copy::confirm_delete_chapters(),
    }
}

/// The Cancel and the confirm label: the first and the last button of the spec dialog, else the catalogue's.
fn buttons(anime: bool) -> (String, String) {
    let spec = spec_dialog(anime).map(|d| d.buttons).unwrap_or_default();
    match (spec.first(), spec.last()) {
        (Some(cancel), Some(ok)) if spec.len() > 1 => (cancel.clone(), ok.clone()),
        _ => (tr(MsgKey::action_cancel), tr(MsgKey::action_ok)),
    }
}

/// Shows the confirm. `on_ok` runs only when OK is pressed.
pub fn confirm_bulk_delete(anime: bool, on_ok: impl Fn() + 'static) {
    let (cancel, confirm) = buttons(anime);
    show(
        ConfirmCopy {
            title: String::new(),
            text: delete_confirm_text(anime),
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
        assert_eq!(delete_confirm_text(false), "Are you sure you want to delete the selected chapters?");
        assert_eq!(delete_confirm_text(true), "Are you sure you want to delete the selected episodes?");
    }

    #[test]
    fn the_delete_dialogs_are_the_spec_dialogs() {
        assert_eq!(dialog_id(true), "updates_delete_episodes");
        assert_eq!(dialog_id(false), "updates_delete_chapters");
        for anime in [true, false] {
            let dialog = spec_dialog(anime).expect("the spec has the dialog");
            assert_eq!(delete_confirm_text(anime), dialog.title);
            assert_eq!(buttons(anime), ("Cancel".to_owned(), "OK".to_owned()));
        }
    }
}
