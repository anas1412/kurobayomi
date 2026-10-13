//! The dialog state of the anime page (T-39b-030). Upstream: `AnimeScreenModel.kt` (`Dialog`, 1547-1562; `dismissDialog`,
//! `showSettingsDialog`, the category-then-track flag) and `AnimeScreen.kt:232-238` (dismiss). Pure: one slot, the last
//! request wins, and the track sheet re-opens after a dismiss only when the add flow set the flag and auto-open is on.

/// Every dialog the anime page can show, with the ids it needs. The widgets read the entry for the rest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryDialog {
    ChangeCategory { entry_id: i64 },
    DuplicateAnime { entry_id: i64, duplicate_id: i64 },
    SetAnimeFetchInterval { entry_id: i64 },
    ChangeAnimeSkipIntro,
    TrackSheet,
}

/// The one open dialog, and the one-shot "track sheet after the category dialog" flag.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DialogSlot {
    current: Option<EntryDialog>,
    from_change_category: bool,
}

impl DialogSlot {
    /// Opens a dialog. It replaces the one on screen (the last request wins).
    pub fn open(&mut self, dialog: EntryDialog) {
        self.current = Some(dialog);
    }

    #[cfg(test)]
    pub fn current(&self) -> Option<&EntryDialog> {
        self.current.as_ref()
    }

    /// The choose-category branch of an add: the category dialog opens and the track sheet may follow its dismiss.
    pub fn open_change_category_from_add(&mut self, entry_id: i64) {
        self.from_change_category = true;
        self.open(EntryDialog::ChangeCategory { entry_id });
    }

    /// Dismisses the dialog on screen. When the add flow set the flag and auto-open is on, the flag is cleared and the
    /// track sheet opens; the return value says so.
    pub fn dismiss(&mut self, auto_open_track: bool) -> bool {
        self.current = None;
        if auto_open_track && self.from_change_category {
            self.from_change_category = false;
            self.open(EntryDialog::TrackSheet);
            return true;
        }
        false
    }

    /// Confirming the category dialog does not go through the dismiss, so the flag is left as it is (5.3).
    pub fn confirm_category(&mut self) {
        self.current = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_last_request_wins_and_dismiss_clears_the_slot() {
        let mut slot = DialogSlot::default();
        slot.open(EntryDialog::TrackSheet);
        slot.open(EntryDialog::ChangeAnimeSkipIntro);
        assert_eq!(slot.current(), Some(&EntryDialog::ChangeAnimeSkipIntro));
        assert!(!slot.dismiss(false));
        assert_eq!(slot.current(), None);
    }

    #[test]
    fn the_track_sheet_follows_a_dismissed_category_dialog_only_when_both_are_set() {
        let mut slot = DialogSlot::default();
        slot.open_change_category_from_add(3);
        assert!(!slot.dismiss(false), "auto-open is off");
        slot.open_change_category_from_add(3);
        assert!(slot.dismiss(true), "the add flow and auto-open are both on");
        assert_eq!(slot.current(), Some(&EntryDialog::TrackSheet));
        slot.dismiss(true);
        assert!(!slot.dismiss(true), "the flag is one-shot");
    }

    #[test]
    fn a_confirmed_category_dialog_keeps_the_flag_for_a_later_dismiss() {
        let mut slot = DialogSlot::default();
        slot.open_change_category_from_add(3);
        slot.confirm_category();
        assert_eq!(slot.current(), None);
        slot.open(EntryDialog::TrackSheet);
        assert!(slot.dismiss(true), "the flag is not cleared by the confirm");
    }
}
