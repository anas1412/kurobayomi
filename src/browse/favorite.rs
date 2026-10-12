//! The favourite toggle, the add-to-library order, the long-click decision and the remove confirm of a browsed entry
//! (T-40b-041, T-40b-042, T-40b-044, T-40b-047). Pure: the caller runs the writes and shows the toast. Every write
//! names the only columns it sets, so chapter flags written by the default-flags step survive (B-EC-12).

use kuro_domain::category::target::AddTarget;
use kuro_i18n::{Arg, MsgKey, tr, trf};
use kuro_platform::kind::Kind;

use super::primitives::{DEFAULT_TOAST, ToastSpec};

/// The columns one favourite write sets: `favorite` and `date_added` and nothing else (B§4.8.1). Adding stamps the
/// time in milliseconds; removing clears it to 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FavoriteWrite {
    pub favorite: bool,
    pub date_added: i64,
}

/// The write that flips a favourite flag. `now_ms` is read only when adding.
pub fn toggle_write(currently_favorite: bool, now_ms: i64) -> FavoriteWrite {
    if currently_favorite {
        FavoriteWrite { favorite: false, date_added: 0 }
    } else {
        FavoriteWrite { favorite: true, date_added: now_ms }
    }
}

/// `cover_last_modified` to write after a removal: set only when at least one cover file was deleted (B-EC-13).
pub fn cover_stamp_after_remove(files_deleted: usize, now_ms: i64) -> Option<i64> {
    (files_deleted > 0).then_some(now_ms)
}

/// The view flips at once. A failed write puts the previous flag back and shows `internal_error` (E-FAV-03, D-FAV-02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OptimisticFavorite {
    previous: bool,
    shown: bool,
}

impl OptimisticFavorite {
    pub fn start(previous: bool) -> Self {
        Self { previous, shown: !previous }
    }

    pub fn shown(&self) -> bool {
        self.shown
    }

    /// Restores the previous flag and returns the toast to show.
    pub fn fail(&mut self) -> ToastSpec {
        self.shown = self.previous;
        ToastSpec { message: tr(MsgKey::internal_error), action: None, duration: DEFAULT_TOAST }
    }
}

/// What an add did. Categories are written before the favourite (E-FAV-01); a category failure is logged and does not
/// block the favourite (E-FAV-02).
#[derive(Debug, PartialEq, Eq)]
pub enum AddOutcome<E> {
    /// The default category asks: open the chooser and write nothing (B§4.8.2).
    Chooser,
    /// The favourite is written. `category_error` is set when the category write failed; the caller logs it.
    Added { category_error: Option<E> },
    /// The favourite write failed. The caller restores its view and shows the toast.
    Failed(E),
}

/// Adds an entry to the library. Runs `set_categories` (always for a resolved target, empty meaning Default) before
/// `set_favorite`. An `Ask` target writes nothing.
pub fn add<E>(
    target: &AddTarget,
    now_ms: i64,
    set_categories: impl FnOnce(&[i64]) -> Result<(), E>,
    set_favorite: impl FnOnce(FavoriteWrite) -> Result<(), E>,
) -> AddOutcome<E> {
    let category_error = match target {
        AddTarget::Ask => return AddOutcome::Chooser,
        AddTarget::Categories(ids) => set_categories(ids).err(),
    };
    match set_favorite(toggle_write(false, now_ms)) {
        Ok(()) => AddOutcome::Added { category_error },
        Err(e) => AddOutcome::Failed(e),
    }
}

/// What a long press on a browse row does (B§4.8.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LongClick {
    RemoveConfirm,
    DuplicateDialog,
    Add,
}

/// A favourite asks to be removed, even when a duplicate exists (B-EC-26). A new row with a duplicate asks first.
/// Anything else is added.
pub fn long_click(favorite: bool, has_duplicate: bool) -> LongClick {
    match (favorite, has_duplicate) {
        (true, _) => LongClick::RemoveConfirm,
        (false, true) => LongClick::DuplicateDialog,
        (false, false) => LongClick::Add,
    }
}

/// The remove-from-library confirm: title "Are you sure?", a body that names the entry, and Remove or Cancel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoveConfirm {
    pub title: String,
    pub body: String,
    pub confirm: String,
    pub cancel: String,
}

pub fn remove_confirm(entry_title: &str) -> RemoveConfirm {
    RemoveConfirm {
        title: tr(MsgKey::are_you_sure),
        body: trf(MsgKey::remove_manga, &[("arg1", Arg::Str(entry_title))]),
        confirm: tr(MsgKey::action_remove),
        cancel: tr(MsgKey::action_cancel),
    }
}

/// The write the confirm answers with: Remove runs the favourite-remove write, Cancel changes nothing.
pub fn on_remove_answer(confirmed: bool) -> Option<FavoriteWrite> {
    confirmed.then(|| toggle_write(true, 0))
}

/// The duplicate dialog: "Are you sure?", the body, and Show manga, Migrate existing entry, Add anyway and Cancel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DuplicateDialog {
    pub title: String,
    pub body: String,
    pub show: String,
    pub migrate: String,
    pub add_anyway: String,
    pub cancel: String,
}

pub fn duplicate_dialog() -> DuplicateDialog {
    duplicate_dialog_for(Kind::Manga)
}

/// The dialog of an anime page: Aniyomi's `DuplicateAnimeDialog` has the manga body and its own Show anime button.
pub fn duplicate_dialog_for(kind: Kind) -> DuplicateDialog {
    DuplicateDialog {
        title: tr(MsgKey::are_you_sure),
        body: tr(MsgKey::confirm_add_duplicate_manga),
        show: tr(if kind.is_anime() { MsgKey::action_show_anime } else { MsgKey::action_show_manga }),
        migrate: tr(MsgKey::action_migrate_duplicate),
        add_anyway: tr(MsgKey::action_add_anyway),
        cancel: tr(MsgKey::action_cancel),
    }
}

/// What the duplicate dialog's answer asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DuplicateAnswer {
    ShowExisting,
    MigrateExisting,
    AddAnyway,
    Nothing,
}

/// `chosen` is the index of the pressed button in dialog order, or `None` for a dismissal or Cancel.
pub fn duplicate_answer(chosen: Option<usize>) -> DuplicateAnswer {
    match chosen {
        Some(0) => DuplicateAnswer::ShowExisting,
        Some(1) => DuplicateAnswer::MigrateExisting,
        Some(2) => DuplicateAnswer::AddAnyway,
        _ => DuplicateAnswer::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    #[test]
    fn the_duplicate_dialog_has_its_four_answers() {
        let dialog = duplicate_dialog();
        assert_eq!(dialog.title, tr(MsgKey::are_you_sure));
        assert!(!dialog.body.is_empty() && !dialog.show.is_empty() && !dialog.migrate.is_empty() && !dialog.add_anyway.is_empty());
        assert_eq!(duplicate_answer(Some(0)), DuplicateAnswer::ShowExisting);
        assert_eq!(duplicate_answer(Some(1)), DuplicateAnswer::MigrateExisting);
        assert_eq!(duplicate_answer(Some(2)), DuplicateAnswer::AddAnyway);
        assert_eq!(duplicate_answer(Some(3)), DuplicateAnswer::Nothing, "Cancel changes nothing");
        assert_eq!(duplicate_answer(None), DuplicateAnswer::Nothing, "so does a dismissal");
    }

    #[test]
    fn the_anime_duplicate_dialog_says_show_anime_and_the_manga_one_says_show_manga() {
        assert_eq!(duplicate_dialog_for(Kind::Anime).show, tr(MsgKey::action_show_anime));
        assert_eq!(duplicate_dialog_for(Kind::Manga).show, tr(MsgKey::action_show_manga));
        assert_eq!(duplicate_dialog_for(Kind::Anime).body, duplicate_dialog_for(Kind::Manga).body);
    }

    #[test]
    fn adding_stamps_the_time_and_removing_clears_it() {
        assert_eq!(toggle_write(false, 42), FavoriteWrite { favorite: true, date_added: 42 });
        assert_eq!(toggle_write(true, 42), FavoriteWrite { favorite: false, date_added: 0 });
    }

    #[test]
    fn cover_stamp_is_written_only_when_a_file_was_deleted() {
        assert_eq!(cover_stamp_after_remove(0, 9), None);
        assert_eq!(cover_stamp_after_remove(2, 9), Some(9));
    }

    #[test]
    fn a_failed_favourite_write_restores_the_previous_flag_and_shows_the_toast() {
        let mut state = OptimisticFavorite::start(false);
        assert!(state.shown(), "the view flips at once");
        let toast = state.fail();
        assert!(!state.shown(), "the previous flag is back");
        assert_eq!(toast.message, tr(MsgKey::internal_error));
        assert_eq!(toast.duration, DEFAULT_TOAST);
        assert!(toast.action.is_none());
    }

    #[test]
    fn add_writes_the_categories_before_the_favourite() {
        let order = RefCell::new(Vec::new());
        let outcome = add(
            &AddTarget::Categories(vec![3]),
            7,
            |ids| {
                assert_eq!(ids, &[3]);
                order.borrow_mut().push("categories");
                Ok::<(), &str>(())
            },
            |write| {
                assert_eq!(write, FavoriteWrite { favorite: true, date_added: 7 }, "only favorite and date_added");
                order.borrow_mut().push("favorite");
                Ok(())
            },
        );
        assert_eq!(outcome, AddOutcome::Added { category_error: None });
        assert_eq!(*order.borrow(), ["categories", "favorite"]);
    }

    #[test]
    fn a_category_failure_is_reported_but_does_not_block_the_favourite() {
        let outcome = add(&AddTarget::Categories(vec![]), 1, |_| Err("cat"), |_| Ok(()));
        assert_eq!(outcome, AddOutcome::Added { category_error: Some("cat") });
    }

    #[test]
    fn a_favourite_failure_is_reported_as_failed() {
        let outcome = add(&AddTarget::Categories(vec![]), 1, |_| Ok(()), |_| Err("db"));
        assert_eq!(outcome, AddOutcome::Failed("db"));
    }

    #[test]
    fn an_ask_target_opens_the_chooser_and_writes_nothing() {
        let touched = RefCell::new(false);
        let outcome = add::<&str>(
            &AddTarget::Ask,
            1,
            |_| {
                *touched.borrow_mut() = true;
                Ok(())
            },
            |_| {
                *touched.borrow_mut() = true;
                Ok(())
            },
        );
        assert_eq!(outcome, AddOutcome::Chooser);
        assert!(!*touched.borrow());
    }

    #[test]
    fn the_long_click_table() {
        assert_eq!(long_click(true, false), LongClick::RemoveConfirm);
        assert_eq!(long_click(true, true), LongClick::RemoveConfirm, "a favourite with a duplicate asks to remove only");
        assert_eq!(long_click(false, true), LongClick::DuplicateDialog);
        assert_eq!(long_click(false, false), LongClick::Add);
    }

    #[test]
    fn the_remove_confirm_names_the_entry_and_remove_writes_only_the_favourite_columns() {
        let confirm = remove_confirm("Berserk");
        assert_eq!(confirm.title, tr(MsgKey::are_you_sure));
        assert!(confirm.body.contains("Berserk"));
        assert_eq!(on_remove_answer(true), Some(FavoriteWrite { favorite: false, date_added: 0 }));
        assert_eq!(on_remove_answer(false), None);
    }
}
