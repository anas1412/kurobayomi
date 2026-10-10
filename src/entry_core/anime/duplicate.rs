//! The duplicate-add choice window (T-39a-047): shown when an anime being added has the same name as one in the library.
//! Upstream: `DuplicateAnimeDialog.kt` and p1-ui-parity 9.5. The window has three rows and a Cancel; every row closes the
//! window first and then acts (T-DU1). Nothing is written until the user picks a row.

use kuro_i18n::MsgKey;

/// The choices, in the order the rows show them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    ShowAnime,
    Migrate,
    AddAnyway,
}

/// One step the window takes after a press: close it, or run the choice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Dismiss,
    Act(Choice),
}

/// The rows in display order, with their label and icon name.
pub const ROWS: [(Choice, MsgKey, &str); 3] = [
    (Choice::ShowAnime, MsgKey::action_show_anime, "document-open-symbolic"),
    (Choice::Migrate, MsgKey::action_migrate_duplicate, "object-flip-vertical-symbolic"),
    (Choice::AddAnyway, MsgKey::action_add_anyway, "list-add-symbolic"),
];

/// The headline of the window.
pub const TITLE: MsgKey = MsgKey::are_you_sure;

/// The body text. The English is the manga duplicate text (the anime catalogue has no separate key).
pub const BODY: MsgKey = MsgKey::confirm_add_duplicate_manga;

/// The steps of a press: a row dismisses first, then acts. Cancel only dismisses.
pub fn steps(choice: Option<Choice>) -> Vec<Step> {
    match choice {
        Some(choice) => vec![Step::Dismiss, Step::Act(choice)],
        None => vec![Step::Dismiss],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn three_rows_in_order_then_cancel() {
        let order: Vec<Choice> = ROWS.iter().map(|r| r.0).collect();
        assert_eq!(order, vec![Choice::ShowAnime, Choice::Migrate, Choice::AddAnyway]);
        assert_eq!(ROWS[1].1, MsgKey::action_migrate_duplicate);
    }

    #[test]
    fn a_row_dismisses_before_it_acts() {
        // T-DU1
        assert_eq!(steps(Some(Choice::Migrate)), vec![Step::Dismiss, Step::Act(Choice::Migrate)]);
    }

    #[test]
    fn cancel_only_dismisses_and_writes_nothing() {
        assert_eq!(steps(None), vec![Step::Dismiss]);
    }
}
