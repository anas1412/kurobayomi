//! The anime migrate dialog (T-41a-041, `MigrateAnimeDialog.kt`): what to include, and the buttons Show anime, Show seasons,
//! Copy and Migrate. `plan` says what the dialog offers for the old and the new title from `migrate_rules::offer`: when
//! the fetch types differ it shows the matching error instead of the options and offers only the two Show buttons. `show`
//! draws it over `DialogSlot`. What Copy and Migrate do is the caller's: this app has no anime migration engine yet, so the
//! caller supplies `run`.
//!
//! The options are the five of `AnimeMigrationFlags` in display order (`migrate_flags::anime_options`): Episodes for an old
//! episodes title, Categories, Custom cover and Custom background when the old title has them, and Delete downloaded when
//! it has downloads. They start from the saved `anime_migrate_flags` bitmap. A run stores the bitmap first (the unused
//! bits stay as they were), and a second request while one runs is ignored.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_source::anime::model::FetchType;

use super::migrate_rules::offer;
use crate::browse::dialogs::{ButtonStyle, DialogSlot, DialogSpec};
use crate::browse::migrate_flags::{self, ANIME_KNOWN, RunGuard, anime_options, preselected, stored_bitmap};

/// `AnimeMigrationFlags`: the bits the dialog can offer.
pub const FLAG_EPISODES: u32 = migrate_flags::EPISODES;
pub const FLAG_CATEGORIES: u32 = migrate_flags::CATEGORIES;

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

/// What the old title has, which decides the optional options.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Facts {
    pub custom_cover: bool,
    pub custom_background: bool,
    pub downloads: bool,
}

/// The label of one option bit.
fn option_label(bit: u32) -> String {
    tr(match bit {
        migrate_flags::EPISODES => MsgKey::episodes,
        migrate_flags::CATEGORIES => MsgKey::categories,
        migrate_flags::CUSTOM_COVER => MsgKey::custom_cover,
        migrate_flags::CUSTOM_BACKGROUND => MsgKey::custom_background,
        _ => MsgKey::delete_downloaded,
    })
}

/// What the dialog shows for an old and a new title.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogPlan {
    /// The fetch-type mismatch message, shown in place of the options.
    pub message: Option<String>,
    /// The options with their bit and label.
    pub flags: Vec<(u32, String)>,
    /// The options that start ticked: the offered ones whose bit is set in the saved bitmap.
    pub selected: Vec<u32>,
    pub actions: Vec<Action>,
}

/// The dialog for moving `old` to `new`: the options when the fetch types match, the error when they do not, Show anime
/// always, Show seasons unless the new title is an episodes title, and Copy and Migrate only when migrating is possible.
/// `saved` is the stored `anime_migrate_flags` bitmap.
pub fn plan(old: FetchType, new: FetchType, facts: Facts, saved: i32) -> DialogPlan {
    let offered = offer(old, new);
    let message = (!offered.migrate).then(|| tr(if old == FetchType::Seasons { MsgKey::label_cant_migrate_season } else { MsgKey::label_cant_migrate_episode }));
    let bits = if offered.migrate { anime_options(offered.episodes_flag, facts.custom_cover, facts.custom_background, facts.downloads) } else { Vec::new() };
    let selected = preselected(saved, &bits);
    let flags = bits.into_iter().map(|bit| (bit, option_label(bit))).collect();
    let mut actions = vec![Action::ShowAnime];
    if offered.show_seasons {
        actions.push(Action::ShowSeasons);
    }
    if offered.migrate {
        actions.extend([Action::Copy, Action::Migrate]);
    }
    DialogPlan { message, flags, selected, actions }
}

/// The bit map of the selected options (`getSelectedFlagsBitMap`).
pub fn bitmap(selected: &[u32]) -> u32 {
    selected.iter().fold(0, |map, flag| map | flag)
}

/// What Copy and Migrate ask for: whether the old entry leaves the library, and the bitmap that was stored (the selected
/// options, with the bits the catalogue does not use kept as they were saved).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Run {
    pub replace: bool,
    pub flags: u32,
}

/// What the buttons do.
pub struct MigrateHandlers {
    pub show_anime: Rc<dyn Fn()>,
    pub show_seasons: Rc<dyn Fn()>,
    /// Stores `anime_migrate_flags`. Called before `run`, so the bitmap is written before any work starts.
    pub save_flags: Rc<dyn Fn(i32)>,
    pub run: Rc<dyn Fn(Run)>,
}

/// Shows the dialog over `anchor`'s window. Ctrl+Enter migrates and Ctrl+Shift+Enter copies when migrating is possible.
/// `facts` says what the old title has and `saved` is the stored bitmap the options start from.
pub fn show(slot: &DialogSlot, anchor: &impl IsA<gtk::Widget>, old: FetchType, new: FetchType, facts: Facts, saved: i32, handlers: MigrateHandlers) -> gtk::Window {
    let plan = plan(old, new, facts, saved);
    let extra = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let checks: Rc<Vec<(u32, gtk::CheckButton)>> = Rc::new(
        plan.flags
            .iter()
            .map(|(flag, label)| {
                let check = gtk::CheckButton::with_label(label);
                check.set_active(plan.selected.contains(flag));
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

    // a run stores the bitmap, then starts; a second request while one runs is ignored and the buttons are disabled
    let window_slot: Rc<RefCell<Option<gtk::glib::WeakRef<gtk::Window>>>> = Rc::default();
    let guard = Rc::new(RunGuard::default());
    let start = {
        let (checks, window_slot, guard) = (Rc::clone(&checks), Rc::clone(&window_slot), Rc::clone(&guard));
        let (save, run) = (Rc::clone(&handlers.save_flags), Rc::clone(&handlers.run));
        Rc::new(move |replace: bool| {
            if !guard.begin() {
                return;
            }
            if let Some(window) = window_slot.borrow().as_ref().and_then(|weak| weak.upgrade()) {
                disable_buttons(window.upcast_ref());
            }
            let selected: Vec<u32> = checks.iter().filter(|(_, check)| check.is_active()).map(|(flag, _)| *flag).collect();
            let flags = stored_bitmap(saved, &selected, ANIME_KNOWN);
            save(flags);
            run(Run { replace, flags: flags as u32 });
            guard.finish();
        })
    };
    let actions = plan.actions.clone();
    let (show_anime, show_seasons) = (handlers.show_anime, handlers.show_seasons);
    let answered = Rc::clone(&start);
    let window = slot.show_with(anchor, spec, Some(extra.upcast()), move |answer| {
        match answer.and_then(|index| actions.get(index).copied()) {
            Some(Action::ShowAnime) => show_anime(),
            Some(Action::ShowSeasons) => show_seasons(),
            Some(Action::Copy) => answered(false),
            Some(Action::Migrate) => answered(true),
            None => {}
        }
    });
    *window_slot.borrow_mut() = Some(window.downgrade());
    if offer(old, new).migrate {
        let (migrate, copy) = (Rc::clone(&start), start);
        let (on_migrate, on_copy) = (window.clone(), window.clone());
        crate::browse::keys::attach_migrate_keys(
            &window,
            move || {
                migrate(true);
                on_migrate.close();
            },
            move || {
                copy(false);
                on_copy.close();
            },
        );
    }
    window
}

/// Disables every button of the dialog (T-40a-052).
fn disable_buttons(window: &gtk::Widget) {
    if let Some(button) = window.downcast_ref::<gtk::Button>() {
        button.set_sensitive(false);
    }
    let mut child = window.first_child();
    while let Some(next) = child {
        disable_buttons(&next);
        child = next.next_sibling();
    }
}

#[cfg(test)]
mod pure_tests {
    use super::*;

    #[test]
    fn matching_episodes_titles_offer_the_options_show_anime_copy_and_migrate() {
        let plan = plan(FetchType::Episodes, FetchType::Episodes, Facts::default(), i32::MAX);
        assert_eq!(plan.message, None);
        assert_eq!(plan.actions, [Action::ShowAnime, Action::Copy, Action::Migrate], "no Show seasons for an episodes title");
        assert_eq!(plan.flags.iter().map(|(flag, _)| *flag).collect::<Vec<_>>(), [FLAG_EPISODES, FLAG_CATEGORIES]);
    }

    #[test]
    fn matching_seasons_titles_offer_show_seasons_and_no_episodes_option() {
        let plan = plan(FetchType::Seasons, FetchType::Seasons, Facts::default(), i32::MAX);
        assert_eq!(plan.actions, [Action::ShowAnime, Action::ShowSeasons, Action::Copy, Action::Migrate]);
        assert_eq!(plan.flags.iter().map(|(flag, _)| *flag).collect::<Vec<_>>(), [FLAG_CATEGORIES], "E-flag: episodes only for an old episodes title");
    }

    #[test]
    fn a_mismatch_shows_the_error_for_the_old_kind_and_only_the_two_show_buttons() {
        let to_seasons = plan(FetchType::Episodes, FetchType::Seasons, Facts::default(), i32::MAX);
        assert_eq!(to_seasons.message, Some(tr(MsgKey::label_cant_migrate_episode)), "E41");
        assert_eq!(to_seasons.actions, [Action::ShowAnime, Action::ShowSeasons]);
        assert!(to_seasons.flags.is_empty(), "no options when migrating is off");
        let to_episodes = plan(FetchType::Seasons, FetchType::Episodes, Facts::default(), i32::MAX);
        assert_eq!(to_episodes.message, Some(tr(MsgKey::label_cant_migrate_season)), "E42");
        assert_eq!(to_episodes.actions, [Action::ShowAnime], "an episodes title has no Show seasons");
    }

    #[test]
    fn all_five_options_are_offered_from_the_titles_and_start_from_the_saved_bitmap() {
        let facts = Facts { custom_cover: true, custom_background: true, downloads: true };
        let all = plan(FetchType::Episodes, FetchType::Episodes, facts, i32::MAX);
        assert_eq!(
            all.flags.iter().map(|(flag, _)| *flag).collect::<Vec<_>>(),
            [migrate_flags::EPISODES, migrate_flags::CATEGORIES, migrate_flags::CUSTOM_COVER, migrate_flags::CUSTOM_BACKGROUND, migrate_flags::DELETE_DOWNLOADED],
            "T-41a-032: the five options in order"
        );
        assert_eq!(all.selected.len(), 5, "the saved default selects all five");
        let bare = plan(FetchType::Episodes, FetchType::Episodes, Facts::default(), i32::MAX);
        assert_eq!(bare.flags.len(), 2, "cover, background and delete only when they apply");
        let saved = plan(FetchType::Episodes, FetchType::Episodes, facts, migrate_flags::CATEGORIES as i32 | migrate_flags::DELETE_DOWNLOADED as i32);
        assert_eq!(saved.selected, [migrate_flags::CATEGORIES, migrate_flags::DELETE_DOWNLOADED], "only the saved bits start ticked");
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
        let (a, b, c, d) = (Rc::clone(log), Rc::clone(log), Rc::clone(log), Rc::clone(log));
        MigrateHandlers {
            save_flags: Rc::new(move |flags| d.borrow_mut().push(format!("save {flags}"))),
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
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Seasons, Facts::default(), i32::MAX, handlers(&log));
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
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Episodes, Facts::default(), i32::MAX, handlers(&log));
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
        assert_eq!(*log.borrow(), ["save 2147483618", "run replace=true flags=2147483618"], "T-41a-033: the bitmap is saved before the run; the unused bits stay set");
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Episodes, Facts::default(), i32::MAX, handlers(&log));
        buttons_of(&window)[1].emit_clicked();
        assert_eq!(log.borrow().last().map(String::as_str), Some("run replace=false flags=2147483619"), "Copy keeps the old entry");
    }

    fn a_run_disables_the_buttons_and_the_dialog_shows_every_offered_option() {
        let (slot, anchor, log) = (DialogSlot::new(), gtk::Label::new(None), Rc::new(RefCell::new(Vec::new())));
        let facts = Facts { custom_cover: true, custom_background: true, downloads: true };
        let window = show(&slot, &anchor, FetchType::Episodes, FetchType::Episodes, facts, migrate_flags::EPISODES as i32, handlers(&log));
        let mut checks = Vec::new();
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
        walk(window.child().expect("child").upcast_ref(), &mut checks);
        assert_eq!(checks.iter().map(|c| c.is_active()).collect::<Vec<_>>(), [true, false, false, false, false], "only the saved bit starts ticked");
        let buttons = buttons_of(&window);
        buttons[1].emit_clicked();
        assert_eq!(*log.borrow(), ["save 1", "run replace=false flags=1"]);
        assert!(buttons.iter().all(|b| !b.is_sensitive()), "T-40a-052: the buttons are disabled as soon as a migration starts");
        buttons[2].emit_clicked();
        assert_eq!(log.borrow().len(), 2, "a second request is ignored");
    }

    pub(crate) fn smoke() {
        a_run_disables_the_buttons_and_the_dialog_shows_every_offered_option();
        the_dialog_follows_the_plan_and_the_buttons_run_their_handlers();
    }
}
