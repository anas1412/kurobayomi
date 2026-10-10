//! The bulk delete confirm of the Updates bar (T-42a-032): no title, the chapter or episode text, OK and Cancel. The
//! bulk bar's delete always asks; the per-row delete in the download indicator does not (parity, EC-42-U-08).

use kuro_i18n::{MsgKey, tr};

use crate::updates::copy;
use crate::updates::history_dialog::{ConfirmCopy, show};

/// The confirm text of a bulk delete: "...chapters?" for manga, "...episodes?" for anime.
pub fn delete_confirm_text(anime: bool) -> &'static str {
    if anime { copy::CONFIRM_DELETE_EPISODES } else { copy::CONFIRM_DELETE_CHAPTERS }
}

/// Shows the confirm. `on_ok` runs only when OK is pressed.
pub fn confirm_bulk_delete(anime: bool, on_ok: impl Fn() + 'static) {
    show(
        ConfirmCopy {
            title: String::new(),
            text: delete_confirm_text(anime).to_owned(),
            checkbox: None,
            confirm: tr(MsgKey::action_ok),
            cancel: tr(MsgKey::action_cancel),
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
}
