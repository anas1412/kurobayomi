//! The anime migrate dialog (T-41a-041, `MigrateAnimeDialog.kt`): what to include, and the buttons Show anime, Show seasons,
//! Copy and Migrate. `plan` says what the dialog offers for the old and the new title from `migrate_rules::offer`: when
//! the fetch types differ it shows the matching error instead of the options and offers only the two Show buttons. `show`
//! draws it over `DialogSlot`. What Copy and Migrate do is the caller's: this app has no migration engine yet, so the
//! caller supplies `run`.
//!
//! Only the two options that follow from the titles are offered, Episodes (for an old episodes title) and Categories.
//! The others of `AnimeMigrationFlags` (custom cover, custom background, delete downloaded) depend on the cover cache and
//! the download index and come with them; the saved default (`migrate_flags`) is the migration task's, so every offered
//! option starts selected, which is what the saved default `Int.MAX_VALUE` gives.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_source::anime::model::FetchType;

use super::migrate_rules::offer;
use crate::browse::dialogs::{ButtonStyle, DialogSlot, DialogSpec};

/// `AnimeMigrationFlags`: the bits the dialog can offer.
pub const FLAG_EPISODES: u32 = 0b00001;
pub const FLAG_CATEGORIES: u32 = 0b00010;

/// A button of the dialog, in display order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    ShowAnime,
    ShowSeasons,
    Copy,
    Migrate,
}

impl Action {
    fn label(self) -> String {
        tr(match self {
            Action::ShowAnime => MsgKey::action_show_anime,
            Action::ShowSeasons => MsgKey::label_show_seasons,
            Action::Copy => MsgKey::copy,
            Action::Migrate => MsgKey::migrate,
        })
    }
}

/// What the dialog shows for an old and a new title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogPlan {
    /// The fetch-type mismatch message, shown in place of the options.
    pub message: Option<String>,
    /// The options with their bit and label, preselected.
    pub flags: Vec<(u32, String)>,
    pub actions: Vec<Action>,
}

/// The dialog for moving `old` to `new`: the options when the fetch types match, the error when they do not, Show anime
/// always, Show seasons unless the new title is an episodes title, and Copy and Migrate only when migrating is possible.
pub fn plan(old: FetchType, new: FetchType) -> DialogPlan {
    let offered = offer(old, new);
    let message = (!offered.migrate).then(|| tr(if old == FetchType::Seasons { MsgKey::label_cant_migrate_season } else { MsgKey::label_cant_migrate_episode }));
    let mut flags = Vec::new();
    if offered.migrate {
        if offered.episodes_flag {
            flags.push((FLAG_EPISODES, tr(MsgKey::episodes)));
        }
        flags.push((FLAG_CATEGORIES, tr(MsgKey::categories)));
    }
    let mut actions = vec![Action::ShowAnime];
    if offered.show_seasons {
        actions.push(Action::ShowSeasons);
    }
    if offered.migrate {
        actions.extend([Action::Copy, Action::Migrate]);
    }
    DialogPlan { message, flags, actions }
}

/// The bit map of the selected options (`getSelectedFlagsBitMap`).
pub fn bitmap(selected: &[u32]) -> u32 {
    selected.iter().fold(0, |map, flag| map | flag)
}

/// What Copy and Migrate ask for: whether the old entry leaves the library, and the bit map of the options.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub replace: bool,
    pub flags: u32,
}

/// What the buttons do.
pub struct MigrateHandlers {
    pub show_anime: Rc<dyn Fn()>,
    pub show_seasons: Rc<dyn Fn()>,
    pub run: Rc<dyn Fn(Run)>,
}

/// Shows the dialog over `anchor`'s window. Ctrl+Enter migrates and Ctrl+Shift+Enter copies when migrating is possible.
pub fn show(slot: &DialogSlot, anchor: &impl IsA<gtk::Widget>, old: FetchType, new: FetchType, handlers: MigrateHandlers) -> gtk::Window {
    let plan = plan(old, new);
    let extra = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let checks: Rc<Vec<(u32, gtk::CheckButton)>> = Rc::new(
        plan.flags
            .iter()
            .map(|(flag, label)| {
                let check = gtk::CheckButton::with_label(label);
                check.set_active(true);
                extra.append(&check);
                (*flag, check)
            })
            .collect(),
    );
    if let Some(message) = &plan.message {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&gtk::Image::from_icon_name("dialog-error-symbolic"));
        row.append(&gtk::Label::builder().label(message).wrap(true).xalign(0.0).css_classes(["error"]).build());
        extra.append(&row);
    }
    let buttons = plan
        .actions
        .iter()
        .map(|action| (action.label(), if *action == Action::Migrate { ButtonStyle::Suggested } else { ButtonStyle::Plain }))
        .collect();
    let spec = DialogSpec { title: tr(MsgKey::migration_dialog_what_to_include), body: None, buttons };
    let selected = {
        let checks = Rc::clone(&checks);
        move || bitmap(&checks.iter().filter(|(_, check)| check.is_active()).map(|(flag, _)| *flag).collect::<Vec<_>>())
    };
    let actions = plan.actions.clone();
    let (show_anime, show_seasons, run) = (handlers.show_anime, handlers.show_seasons, Rc::clone(&handlers.run));
    let answered = selected.clone();
    let window = slot.show_with(anchor, spec, Some(extra.upcast()), move |answer| {
        match answer.and_then(|index| actions.get(index).copied()) {
            Some(Action::ShowAnime) => show_anime(),
            Some(Action::ShowSeasons) => show_seasons(),
            Some(Action::Copy) => run(Run { replace: false, flags: answered() }),
            Some(Action::Migrate) => run(Run { replace: true, flags: answered() }),
            None => {}
        }
    });
    if offer(old, new).migrate {
        let (migrate, copy) = (Rc::clone(&handlers.run), handlers.run);
        let (for_migrate, for_copy) = (selected.clone(), selected);
        let (on_migrate, on_copy) = (window.clone(), window.clone());
        crate::browse::keys::attach_migrate_keys(
            &window,
            move || {
                migrate(Run { replace: true, flags: for_migrate() });
                on_migrate.close();
            },
            move || {
                copy(Run { replace: false, flags: for_copy() });
                on_copy.close();
            },
        );
    }
    window
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn matching_episodes_titles_offer_the_options_show_anime_copy_and_migrate() {
        let plan = plan(FetchType::Episodes, FetchType::Episodes);
        assert_eq!(plan.message, None);
        assert_eq!(plan.actions, [Action::ShowAnime, Action::Copy, Action::Migrate], "no Show seasons for an episodes title");
        assert_eq!(plan.flags.iter().map(|(flag, _)| *flag).collect::<Vec<_>>(), [FLAG_EPISODES, FLAG_CATEGORIES]);
    }

    #[test]
    fn matching_seasons_titles_offer_show_seasons_and_no_episodes_option() {
        let plan = plan(FetchType::Seasons, FetchType::Seasons);
        assert_eq!(plan.actions, [Action::ShowAnime, Action::ShowSeasons, Action::Copy, Action::Migrate]);
        assert_eq!(plan.flags.iter().map(|(flag, _)| *flag).collect::<Vec<_>>(), [FLAG_CATEGORIES], "E-flag: episodes only for an old episodes title");
    }

    #[test]
    fn a_mismatch_shows_the_error_for_the_old_kind_and_only_the_two_show_buttons() {
        let to_seasons = plan(FetchType::Episodes, FetchType::Seasons);
        assert_eq!(to_seasons.message, Some(tr(MsgKey::label_cant_migrate_episode)), "E41");
        assert_eq!(to_seasons.actions, [Action::ShowAnime, Action::ShowSeasons]);
        assert!(to_seasons.flags.is_empty(), "no options when migrating is off");
        let to_episodes = plan(FetchType::Seasons, FetchType::Episodes);
        assert_eq!(to_episodes.message, Some(tr(MsgKey::label_cant_migrate_season)), "E42");
        assert_eq!(to_episodes.actions, [Action::ShowAnime], "an episodes title has no Show seasons");
    }

    #[test]
    fn the_bit_map_is_the_or_of_the_selected_options() {
        assert_eq!(bitmap(&[]), 0);
        assert_eq!(bitmap(&[FLAG_EPISODES, FLAG_CATEGORIES]), 0b11);
        assert_eq!(bitmap(&[FLAG_CATEGORIES, FLAG_CATEGORIES]), FLAG_CATEGORIES, "a repeated option is one bit");
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::browse::dialogs::buttons_of;

    fn handlers(log: &Rc<RefCell<Vec<String>>>) -> MigrateHandlers {
        let (a, b, c) = (Rc::clone(log), Rc::clone(log), Rc::clone(log));
        MigrateHandlers {
            show_anime: Rc::new(move || a.borrow_mut().push("anime".into())),
            show_seasons: Rc::new(move || b.borrow_mut().push("seasons".into())),
            run: Rc::new(move |run| c.borrow_mut().push(format!("run replace={} flags={}", run.replace, run.flags))),
        }
    }

    fn labels(window: &gtk::Window) -> Vec<String> {
        buttons_of(window).iter().filter_map(|b| b.label()).map(|l| l.to_string()).collect()
    }

    fn the_dialog_follows_the_plan_and_the_buttons_run_their_handlers() {
        let (slot, anchor, log) = (DialogSlot::new(), gtk::Label::new(None), Rc::new(RefCell::new(Vec::new())));
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Seasons, handlers(&log));
        assert_eq!(labels(&window), [Action::ShowAnime.label(), Action::ShowSeasons.label()], "a mismatch offers the two Show buttons");
        let text: Vec<String> = {
            fn walk(widget: &gtk::Widget, out: &mut Vec<String>) {
                if let Some(label) = widget.downcast_ref::<gtk::Label>() {
                    out.push(label.label().to_string());
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    walk(&next, out);
                    child = next.next_sibling();
                }
            }
            let mut out = Vec::new();
            walk(window.child().expect("child").upcast_ref(), &mut out);
            out
        };
        assert!(text.contains(&tr(MsgKey::label_cant_migrate_episode)), "the mismatch message is on the dialog: {text:?}");
        buttons_of(&window)[1].emit_clicked();
        assert_eq!(*log.borrow(), ["seasons"]);

        log.borrow_mut().clear();
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Episodes, handlers(&log));
        assert_eq!(labels(&window), [Action::ShowAnime.label(), Action::Copy.label(), Action::Migrate.label()]);
        let checks: Vec<gtk::CheckButton> = {
            fn walk(widget: &gtk::Widget, out: &mut Vec<gtk::CheckButton>) {
                if let Some(check) = widget.downcast_ref::<gtk::CheckButton>() {
                    out.push(check.clone());
                }
                let mut child = widget.first_child();
                while let Some(next) = child {
                    walk(&next, out);
                    child = next.next_sibling();
                }
            }
            let mut out = Vec::new();
            walk(window.child().expect("child").upcast_ref(), &mut out);
            out
        };
        assert_eq!(checks.len(), 2);
        assert!(checks.iter().all(|c| c.is_active()), "every offered option starts selected");
        checks[0].set_active(false);
        buttons_of(&window)[2].emit_clicked();
        assert_eq!(*log.borrow(), ["run replace=true flags=2"], "Migrate replaces and carries only the selected bits");
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Episodes, handlers(&log));
        buttons_of(&window)[1].emit_clicked();
        assert_eq!(log.borrow().last().map(String::as_str), Some("run replace=false flags=3"), "Copy keeps the old entry");
    }

    pub(crate) fn smoke() {
        the_dialog_follows_the_plan_and_the_buttons_run_their_handlers();
    }
}
