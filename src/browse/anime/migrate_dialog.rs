//! The anime migrate dialog (T-41a-041, `MigrateAnimeDialog.kt`): what to include, and the buttons Show anime, Show seasons,
//! Copy and Migrate. `plan` says what the dialog offers for the old and the new title from `migrate_rules::offer`: when
//! the fetch types differ it shows the matching error instead of the options and offers only the two Show buttons. `show`
//! draws it over `DialogSlot`. What Copy and Migrate do is the caller's: it supplies `run`, which reports back when the
//! migration ends, and the dialog shows the running state until then (T-41a-035).
//!
//! The options are the five of `AnimeMigrationFlags` in display order (`migrate_flags::anime_options`): Episodes for an old
//! episodes title, Categories, Custom cover and Custom background when the old title has them, and Delete downloaded when
//! it has downloads. They start from the saved `anime_migrate_flags` bitmap. A run stores the bitmap first (the unused
//! bits stay as they were), and a second request while one runs is refused.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_source::anime::model::FetchType;

use super::migrate_rules::offer;
use crate::browse::dialogs::DialogSlot;
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

/// How a started run reports back, on the GTK thread: `Err` carries the reason the dialog shows.
pub type Done = Box<dyn FnOnce(Result<(), String>)>;

/// What the buttons do.
pub struct MigrateHandlers {
    pub show_anime: Rc<dyn Fn()>,
    pub show_seasons: Rc<dyn Fn()>,
    /// Stores `anime_migrate_flags`. Called before `run`, so the bitmap is written before any work starts.
    pub save_flags: Rc<dyn Fn(i32)>,
    /// Starts the migration and calls `Done` when it ends. The dialog stays in its running state until then.
    pub run: Rc<dyn Fn(Run, Done)>,
    /// A migration succeeded and the dialog closed: the caller leaves the migrate page.
    pub finished: Rc<dyn Fn()>,
}

struct Dialog {
    /// Weak: the window owns the dialog (`keep_alive`), so a strong link would be a cycle.
    window: gtk::glib::WeakRef<gtk::Window>,
    checks: Vec<(u32, gtk::CheckButton)>,
    body: gtk::Box,
    spinner: gtk::Spinner,
    error: gtk::Label,
    row: gtk::Box,
    saved: i32,
    guard: RunGuard,
    handlers: MigrateHandlers,
}

impl Dialog {
    /// The running state: the options sit under a 70 % cover (30 % opacity), a spinner is over them and the buttons are hidden.
    fn set_running(&self, running: bool) {
        self.body.set_opacity(if running { 0.3 } else { 1.0 });
        self.body.set_sensitive(!running);
        self.spinner.set_visible(running);
        self.spinner.set_spinning(running);
        self.row.set_visible(!running);
    }

    /// Copy (`replace` false) and Migrate (`replace` true). A second request while one runs is refused: the guard is
    /// finished only when the run reports back, not when `run` returns.
    fn start(self: &Rc<Self>, replace: bool) {
        if !self.guard.begin() {
            return;
        }
        self.error.set_visible(false);
        self.set_running(true);
        let selected: Vec<u32> = self.checks.iter().filter(|(_, check)| check.is_active()).map(|(flag, _)| *flag).collect();
        let flags = stored_bitmap(self.saved, &selected, ANIME_KNOWN);
        (self.handlers.save_flags)(flags);
        let this = Rc::clone(self);
        (self.handlers.run)(
            Run { replace, flags: flags as u32 },
            Box::new(move |result| {
                this.guard.finish();
                this.set_running(false);
                match result {
                    Ok(()) => {
                        if let Some(window) = this.window.upgrade() {
                            window.close();
                        }
                        (this.handlers.finished)();
                    }
                    Err(reason) => {
                        // the dialog stays open, shows why and can be used again
                        this.error.set_label(&crate::browse::texts::migration_failed(&reason));
                        this.error.set_visible(true);
                    }
                }
            }),
        );
    }
}

/// Shows the dialog over `anchor`'s window. Ctrl+Enter migrates and Ctrl+Shift+Enter copies when migrating is possible.
/// `facts` says what the old title has and `saved` is the stored bitmap the options start from. While a run is going the
/// window refuses to close, so Escape does nothing.
pub fn show(slot: &DialogSlot, anchor: &impl IsA<gtk::Widget>, old: FetchType, new: FetchType, facts: Facts, saved: i32, handlers: MigrateHandlers) -> gtk::Window {
    let plan = plan(old, new, facts, saved);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let checks: Vec<(u32, gtk::CheckButton)> = plan
        .flags
        .iter()
        .map(|(flag, label)| {
            let check = gtk::CheckButton::with_label(label);
            check.set_active(plan.selected.contains(flag));
            body.append(&check);
            (*flag, check)
        })
        .collect();
    if let Some(message) = &plan.message {
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        row.append(&gtk::Image::from_icon_name("dialog-error-symbolic"));
        row.append(&gtk::Label::builder().label(message).wrap(true).xalign(0.0).css_classes(["error"]).build());
        body.append(&row);
    }
    let error = gtk::Label::builder().visible(false).wrap(true).xalign(0.0).max_width_chars(50).css_classes(["error"]).build();
    body.append(&error);
    let spinner = gtk::Spinner::builder().visible(false).halign(gtk::Align::Center).valign(gtk::Align::Center).width_request(48).height_request(48).build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&body));
    overlay.add_overlay(&spinner);

    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    row.set_halign(gtk::Align::End);
    let buttons: Vec<gtk::Button> = plan
        .actions
        .iter()
        .map(|action| {
            let button = gtk::Button::with_label(&action.label());
            if *action == Action::Migrate {
                button.add_css_class("suggested-action");
            }
            row.append(&button);
            button
        })
        .collect();
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    for set in [gtk::Box::set_margin_start, gtk::Box::set_margin_end, gtk::Box::set_margin_top, gtk::Box::set_margin_bottom] {
        set(&column, 18);
    }
    column.append(&overlay);
    column.append(&row);
    let window = gtk::Window::builder().title(tr(MsgKey::migration_dialog_what_to_include)).modal(true).resizable(false).child(&column).build();
    window.set_transient_for(anchor.upcast_ref::<gtk::Widget>().root().and_downcast::<gtk::Window>().as_ref());
    crate::browse::keys::close_on_escape(&window);

    let dialog = Rc::new(Dialog { window: window.downgrade(), checks, body, spinner, error, row, saved, guard: RunGuard::default(), handlers });
    // a running migration cannot be dismissed: Escape and the close button end up here
    let running = Rc::clone(&dialog);
    window.connect_close_request(move |_| if running.guard.is_running() { gtk::glib::Propagation::Stop } else { gtk::glib::Propagation::Proceed });
    slot.adopt(&window);

    for (action, button) in plan.actions.iter().zip(&buttons) {
        let (this, closing) = (Rc::clone(&dialog), window.clone());
        match action {
            Action::ShowAnime => button.connect_clicked(move |_| {
                closing.close();
                (this.handlers.show_anime)();
            }),
            Action::ShowSeasons => button.connect_clicked(move |_| {
                closing.close();
                (this.handlers.show_seasons)();
            }),
            Action::Copy => button.connect_clicked(move |_| this.start(false)),
            Action::Migrate => button.connect_clicked(move |_| this.start(true)),
        };
    }
    if offer(old, new).migrate {
        let (to_migrate, to_copy) = (Rc::clone(&dialog), Rc::clone(&dialog));
        crate::browse::keys::attach_migrate_keys(&window, move || to_migrate.start(true), move || to_copy.start(false));
    }
    // the dialog lives as long as its window
    crate::browse::glue::keep_alive(&window, &dialog);
    window.present();
    window
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
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::browse::dialogs::buttons_of;

    type Pending = Rc<RefCell<Vec<Done>>>;

    fn handlers(log: &Rc<RefCell<Vec<String>>>, pending: &Pending) -> MigrateHandlers {
        let (a, b, c, d, e) = (Rc::clone(log), Rc::clone(log), Rc::clone(log), Rc::clone(log), Rc::clone(log));
        let queue = Rc::clone(pending);
        MigrateHandlers {
            save_flags: Rc::new(move |flags| d.borrow_mut().push(format!("save {flags}"))),
            show_anime: Rc::new(move || a.borrow_mut().push("anime".into())),
            show_seasons: Rc::new(move || b.borrow_mut().push("seasons".into())),
            run: Rc::new(move |run, done| {
                c.borrow_mut().push(format!("run replace={} flags={}", run.replace, run.flags));
                queue.borrow_mut().push(done);
            }),
            finished: Rc::new(move || e.borrow_mut().push("finished".into())),
        }
    }

    fn labels(window: &gtk::Window) -> Vec<String> {
        buttons_of(window).iter().filter_map(|b| b.label()).map(|l| l.to_string()).collect()
    }

    fn all<T: IsA<gtk::Widget>>(window: &gtk::Window) -> Vec<T> {
        fn walk<T: IsA<gtk::Widget>>(widget: &gtk::Widget, out: &mut Vec<T>) {
            if let Some(found) = widget.downcast_ref::<T>() {
                out.push(found.clone());
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
    }

    struct Rig {
        slot: DialogSlot,
        anchor: gtk::Label,
        log: Rc<RefCell<Vec<String>>>,
        pending: Pending,
    }

    fn rig() -> Rig {
        Rig { slot: DialogSlot::new(), anchor: gtk::Label::new(None), log: Rc::default(), pending: Rc::default() }
    }

    impl Rig {
        fn open(&self, old: FetchType, new: FetchType, facts: Facts, saved: i32) -> gtk::Window {
            show(&self.slot, &self.anchor, old, new, facts, saved, handlers(&self.log, &self.pending))
        }

        fn episodes(&self) -> gtk::Window {
            self.open(FetchType::Episodes, FetchType::Episodes, Facts::default(), i32::MAX)
        }

        /// The run the engine has not answered yet.
        fn answer(&self, result: Result<(), String>) {
            let done = self.pending.borrow_mut().pop().expect("a run is pending");
            done(result);
        }
    }

    fn the_dialog_follows_the_plan_and_the_buttons_run_their_handlers() {
        let rig = rig();
        let window = rig.open(FetchType::Episodes, FetchType::Seasons, Facts::default(), i32::MAX);
        assert_eq!(labels(&window), [Action::ShowAnime.label(), Action::ShowSeasons.label()], "a mismatch offers the two Show buttons");
        let text: Vec<String> = all::<gtk::Label>(&window).iter().map(|l| l.label().to_string()).collect();
        assert!(text.contains(&tr(MsgKey::label_cant_migrate_episode)), "the mismatch message is on the dialog: {text:?}");
        buttons_of(&window)[1].emit_clicked();
        assert_eq!(*rig.log.borrow(), ["seasons"]);
        assert!(rig.slot.current().is_none(), "Show seasons closes the dialog");

        rig.log.borrow_mut().clear();
        let window = rig.episodes();
        assert_eq!(labels(&window), [Action::ShowAnime.label(), Action::Copy.label(), Action::Migrate.label()]);
        let checks = all::<gtk::CheckButton>(&window);
        assert_eq!(checks.len(), 2);
        assert!(checks.iter().all(|c| c.is_active()), "every offered option starts selected");
        checks[0].set_active(false);
        buttons_of(&window)[2].emit_clicked();
        assert_eq!(*rig.log.borrow(), ["save 2147483618", "run replace=true flags=2147483618"], "T-41a-033: the bitmap is saved before the run; the unused bits stay set");
        rig.answer(Ok(()));
        let window = rig.episodes();
        buttons_of(&window)[1].emit_clicked();
        assert_eq!(rig.log.borrow().last().map(String::as_str), Some("run replace=false flags=2147483619"), "Copy keeps the old entry");
        rig.answer(Ok(()));
    }

    fn a_run_shows_the_spinner_over_a_dimmed_body_hides_the_buttons_and_refuses_a_second_start() {
        let rig = rig();
        let facts = Facts { custom_cover: true, custom_background: true, downloads: true };
        let window = rig.open(FetchType::Episodes, FetchType::Episodes, facts, migrate_flags::EPISODES as i32);
        let checks = all::<gtk::CheckButton>(&window);
        assert_eq!(checks.iter().map(|c| c.is_active()).collect::<Vec<_>>(), [true, false, false, false, false], "only the saved bit starts ticked");
        let buttons = buttons_of(&window);
        let spinner = all::<gtk::Spinner>(&window).remove(0);
        assert!(!spinner.is_visible(), "no spinner before a run");
        buttons[1].emit_clicked();
        assert_eq!(*rig.log.borrow(), ["save 1", "run replace=false flags=1"]);
        assert!(spinner.is_visible() && spinner.is_spinning(), "T-41a-035: the spinner shows while the run goes");
        assert!(buttons.iter().all(|b| !b.is_visible()), "and the buttons are hidden");
        let body = checks[0].parent().expect("body");
        assert!((body.opacity() - 0.3).abs() < 0.01, "the options sit under a 70 % cover");
        buttons[2].emit_clicked();
        assert_eq!(rig.log.borrow().len(), 2, "a second request during the run is refused");
        assert_eq!(rig.pending.borrow().len(), 1);
        rig.answer(Ok(()));
        assert!(rig.slot.current().is_none(), "a success closes the dialog");
        assert_eq!(rig.log.borrow().last().map(String::as_str), Some("finished"));
    }

    fn escape_and_the_close_button_do_nothing_while_a_run_goes() {
        let rig = rig();
        let window = rig.episodes();
        let closes = Rc::new(Cell::new(0));
        let counted = Rc::clone(&closes);
        window.connect_close_request(move |_| {
            counted.set(counted.get() + 1);
            gtk::glib::Propagation::Proceed
        });
        buttons_of(&window)[2].emit_clicked();
        window.close();
        assert!(rig.slot.current().is_some(), "the dialog is still the slot's while it runs");
        assert_eq!(closes.get(), 0, "the close request was refused before it reached anything else");
        rig.answer(Ok(()));
        assert!(rig.slot.current().is_none());
    }

    fn a_failure_shows_the_reason_in_the_dialog_and_lets_it_run_again() {
        let rig = rig();
        let window = rig.episodes();
        let buttons = buttons_of(&window);
        buttons[2].emit_clicked();
        rig.answer(Err("disk full".into()));
        let text: Vec<String> = all::<gtk::Label>(&window).iter().filter(|l| l.is_visible()).map(|l| l.label().to_string()).collect();
        assert!(text.contains(&"Migration failed: disk full".to_owned()), "{text:?}");
        assert!(buttons.iter().all(|b| b.is_visible() && b.is_sensitive()), "the buttons come back");
        assert!(!rig.log.borrow().contains(&"finished".to_owned()), "the page is not left");
        assert!(rig.slot.current().is_some(), "the dialog stays open");
        buttons[1].emit_clicked();
        assert_eq!(rig.pending.borrow().len(), 1, "a new run starts");
        assert!(!all::<gtk::Label>(&window).iter().any(|l| l.is_visible() && l.label().starts_with("Migration failed")), "the old error is cleared");
        rig.answer(Ok(()));
    }

    pub(crate) fn smoke() {
        the_dialog_follows_the_plan_and_the_buttons_run_their_handlers();
        a_run_shows_the_spinner_over_a_dimmed_body_hides_the_buttons_and_refuses_a_second_start();
        escape_and_the_close_button_do_nothing_while_a_run_goes();
        a_failure_shows_the_reason_in_the_dialog_and_lets_it_run_again();
    }
}
