//! The manga migrate dialog (`MigrateMangaDialog.kt`; T-40a-015, T-40a-045, T-40a-052): what to include, and the buttons
//! Show manga, Copy and Migrate. The options come from `migrate_flags` (chapters, categories, custom cover when the old
//! manga has one, delete downloaded when downloads exist) and start from the saved bitmap. A run saves the bitmap first,
//! disables the buttons, and shows a spinner over a dimmed body; a failure keeps the dialog open with a toast. The dim
//! line under the options says that tracking data is always transferred.

use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};

use super::migrate_engine::{MigrateError, MigrateOutcome, MigratePlan};
use super::migrate_flags::{CATEGORIES, CUSTOM_COVER, MANGA_CHAPTERS, MANGA_KNOWN, RunGuard, manga_options, preselected, stored_bitmap};

/// The label of one option bit.
pub fn option_label(bit: u32) -> String {
    tr(match bit {
        MANGA_CHAPTERS => MsgKey::chapters,
        CATEGORIES => MsgKey::categories,
        CUSTOM_COVER => MsgKey::custom_cover,
        _ => MsgKey::delete_downloaded,
    })
}

/// What the dialog offers and starts with: the options in display order with their labels, and the ones that start ticked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogPlan {
    pub options: Vec<(u32, String)>,
    pub selected: Vec<u32>,
}

/// The plan for an old manga that has (or has not) a custom cover and downloads, from the saved bitmap.
pub fn plan(has_custom_cover: bool, has_downloads: bool, saved: i32) -> DialogPlan {
    let offered = manga_options(has_custom_cover, has_downloads);
    let selected = preselected(saved, &offered);
    DialogPlan { options: offered.into_iter().map(|bit| (bit, option_label(bit))).collect(), selected }
}

/// How the engine's answer shows: a toast text for a failure, none for success.
pub fn failure_text(result: &Result<MigrateOutcome, MigrateError>) -> Option<String> {
    match result {
        Ok(outcome) => (!outcome.warnings.is_empty()).then(|| tr(MsgKey::internal_error)),
        Err(_) => Some(tr(MsgKey::internal_error)),
    }
}

/// What the dialog needs from the caller.
pub struct Handlers {
    /// Show manga: the new manga's page.
    pub show_manga: Rc<dyn Fn()>,
    /// Stores the bitmap (`migrate_flags`). Called before the run starts.
    pub save_flags: Rc<dyn Fn(i32)>,
    /// Starts the migration and reports its answer on the GTK thread.
    pub run: Rc<dyn Fn(MigratePlan, Box<dyn FnOnce(Result<MigrateOutcome, MigrateError>)>)>,
    /// A migration finished: the search screen is popped (`onPopScreen`).
    pub finished: Rc<dyn Fn()>,
    pub toast: Rc<dyn Fn(String)>,
}

struct Dialog {
    /// Weak: the window owns the dialog (`keep_alive`), so a strong link would be a cycle.
    window: gtk::glib::WeakRef<gtk::Window>,
    checks: Vec<(u32, gtk::CheckButton)>,
    buttons: Vec<gtk::Button>,
    body: gtk::Box,
    spinner: gtk::Spinner,
    saved: i32,
    guard: RunGuard,
    handlers: Handlers,
}

impl Dialog {
    fn selected(&self) -> Vec<u32> {
        self.checks.iter().filter(|(_, check)| check.is_active()).map(|(bit, _)| *bit).collect()
    }

    fn set_busy(&self, busy: bool) {
        self.body.set_opacity(if busy { 0.3 } else { 1.0 });
        self.body.set_sensitive(!busy);
        self.spinner.set_visible(busy);
        self.spinner.set_spinning(busy);
    }

    /// Copy (`replace` false) and Migrate (`replace` true). A second request while one runs is ignored; the buttons are
    /// disabled from the first one on.
    fn start(self: &Rc<Self>, replace: bool) {
        if !self.guard.begin() {
            return;
        }
        for button in &self.buttons {
            button.set_sensitive(false);
        }
        self.set_busy(true);
        let flags = stored_bitmap(self.saved, &self.selected(), MANGA_KNOWN);
        // the bitmap is stored before any work starts
        (self.handlers.save_flags)(flags);
        let this = Rc::clone(self);
        (self.handlers.run)(
            MigratePlan { replace, flags },
            Box::new(move |result| {
                this.guard.finish();
                this.set_busy(false);
                if let Some(text) = failure_text(&result) {
                    (this.handlers.toast)(text);
                }
                if result.is_ok() {
                    if let Some(window) = this.window.upgrade() {
                        window.close();
                    }
                    (this.handlers.finished)();
                } else {
                    // the dialog stays open and can be used again
                    for button in &this.buttons {
                        button.set_sensitive(true);
                    }
                }
            }),
        );
    }
}

/// Shows the dialog over `anchor`'s window. Ctrl+Enter migrates and Ctrl+Shift+Enter copies; Escape closes it.
pub fn show(anchor: &impl IsA<gtk::Widget>, has_custom_cover: bool, has_downloads: bool, saved: i32, handlers: Handlers) -> gtk::Window {
    let plan = plan(has_custom_cover, has_downloads, saved);
    let body = gtk::Box::new(gtk::Orientation::Vertical, 6);
    let checks: Vec<(u32, gtk::CheckButton)> = plan
        .options
        .iter()
        .map(|(bit, label)| {
            let check = gtk::CheckButton::with_label(label);
            check.set_active(plan.selected.contains(bit));
            body.append(&check);
            (*bit, check)
        })
        .collect();
    body.append(&gtk::Label::builder().label(super::texts::MIGRATE_TRACKS_ALWAYS).xalign(0.0).css_classes(["dim-label"]).build());

    let (show_button, copy, migrate) = (gtk::Button::with_label(&tr(MsgKey::action_show_manga)), gtk::Button::with_label(&tr(MsgKey::copy)), gtk::Button::with_label(&tr(MsgKey::migrate)));
    migrate.add_css_class("suggested-action");
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    show_button.set_hexpand(true);
    show_button.set_halign(gtk::Align::Start);
    for button in [&show_button, &copy, &migrate] {
        row.append(button);
    }

    let spinner = gtk::Spinner::builder().visible(false).halign(gtk::Align::Center).valign(gtk::Align::Center).width_request(48).height_request(48).build();
    let overlay = gtk::Overlay::new();
    overlay.set_child(Some(&body));
    overlay.add_overlay(&spinner);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    for set in [gtk::Box::set_margin_start, gtk::Box::set_margin_end, gtk::Box::set_margin_top, gtk::Box::set_margin_bottom] {
        set(&column, 18);
    }
    column.append(&overlay);
    column.append(&row);

    let window = gtk::Window::builder().title(tr(MsgKey::migration_dialog_what_to_include)).modal(true).resizable(false).child(&column).build();
    window.set_transient_for(anchor.upcast_ref::<gtk::Widget>().root().and_downcast::<gtk::Window>().as_ref());
    super::keys::close_on_escape(&window);

    let dialog = Rc::new(Dialog { window: window.downgrade(), checks, buttons: vec![copy.clone(), migrate.clone()], body, spinner, saved, guard: RunGuard::default(), handlers });
    let handler = Rc::clone(&dialog.handlers.show_manga);
    let closing = window.clone();
    show_button.connect_clicked(move |_| {
        closing.close();
        handler();
    });
    let (to_copy, to_migrate) = (Rc::clone(&dialog), Rc::clone(&dialog));
    copy.connect_clicked(move |_| to_copy.start(false));
    migrate.connect_clicked(move |_| to_migrate.start(true));
    let (on_migrate, on_copy) = (Rc::clone(&dialog), Rc::clone(&dialog));
    super::keys::attach_migrate_keys(&window, move || on_migrate.start(true), move || on_copy.start(false));
    // the dialog lives as long as its window
    super::glue::keep_alive(&window, &dialog);
    window.present();
    window
}

#[cfg(test)]
mod pure_tests {
    use super::*;
    use crate::browse::migrate_flags::DELETE_DOWNLOADED;

    #[test]
    fn the_options_follow_the_old_manga_and_start_from_the_saved_bitmap() {
        let all = plan(true, true, i32::MAX);
        assert_eq!(all.options.iter().map(|(bit, _)| *bit).collect::<Vec<_>>(), [MANGA_CHAPTERS, CATEGORIES, CUSTOM_COVER, DELETE_DOWNLOADED]);
        assert_eq!(all.selected, [MANGA_CHAPTERS, CATEGORIES, CUSTOM_COVER, DELETE_DOWNLOADED], "the default selects everything offered");
        let bare = plan(false, false, CATEGORIES as i32);
        assert_eq!(bare.options.iter().map(|(bit, _)| *bit).collect::<Vec<_>>(), [MANGA_CHAPTERS, CATEGORIES], "cover and delete only when they apply");
        assert_eq!(bare.selected, [CATEGORIES], "only the saved bit starts ticked");
    }

    #[test]
    fn a_failure_or_a_warning_toasts_internal_error_and_a_clean_success_is_silent() {
        let toast = tr(MsgKey::internal_error);
        assert_eq!(failure_text(&Err(MigrateError::NewSourceMissing)), Some(toast.clone()));
        assert_eq!(failure_text(&Err(MigrateError::FetchChapters("x".into()))), Some(toast.clone()));
        assert_eq!(failure_text(&Ok(MigrateOutcome { warnings: vec!["busy".into()] })), Some(toast));
        assert_eq!(failure_text(&Ok(MigrateOutcome::default())), None);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::{Cell, RefCell};

    use super::*;
    use crate::browse::dialogs::buttons_of;

    type Done = Box<dyn FnOnce(Result<MigrateOutcome, MigrateError>)>;

    struct Rig {
        log: Rc<RefCell<Vec<String>>>,
        pending: Rc<RefCell<Vec<Done>>>,
        window: gtk::Window,
        /// How many times the window was asked to close.
        closes: Rc<Cell<u32>>,
    }

    fn rig(has_cover: bool, has_downloads: bool, saved: i32) -> Rig {
        let (log, pending): (Rc<RefCell<Vec<String>>>, Rc<RefCell<Vec<Done>>>) = Default::default();
        let (show, save, run, finished, toast) = (Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log), Rc::clone(&log));
        let queue = Rc::clone(&pending);
        let handlers = Handlers {
            show_manga: Rc::new(move || show.borrow_mut().push("show".into())),
            save_flags: Rc::new(move |flags| save.borrow_mut().push(format!("save {flags}"))),
            run: Rc::new(move |plan, done| {
                run.borrow_mut().push(format!("run replace={} flags={}", plan.replace, plan.flags));
                queue.borrow_mut().push(done);
            }),
            finished: Rc::new(move || finished.borrow_mut().push("finished".into())),
            toast: Rc::new(move |text| toast.borrow_mut().push(format!("toast {text}"))),
        };
        let window = show_dialog(has_cover, has_downloads, saved, handlers);
        let closes = Rc::new(Cell::new(0));
        let counted = Rc::clone(&closes);
        window.connect_close_request(move |_| {
            counted.set(counted.get() + 1);
            gtk::glib::Propagation::Proceed
        });
        Rig { log, pending, window, closes }
    }

    fn show_dialog(has_cover: bool, has_downloads: bool, saved: i32, handlers: Handlers) -> gtk::Window {
        show(&gtk::Label::new(None), has_cover, has_downloads, saved, handlers)
    }

    fn walk<T: IsA<gtk::Widget>>(root: &gtk::Widget, out: &mut Vec<T>) {
        if let Some(found) = root.downcast_ref::<T>() {
            out.push(found.clone());
        }
        let mut child = root.first_child();
        while let Some(next) = child {
            walk(&next, out);
            child = next.next_sibling();
        }
    }

    fn all<T: IsA<gtk::Widget>>(window: &gtk::Window) -> Vec<T> {
        let mut out = Vec::new();
        walk(window.child().expect("child").upcast_ref(), &mut out);
        out
    }

    fn the_dialog_offers_the_options_the_three_buttons_and_the_dim_disclosure() {
        let rig = rig(true, true, i32::MAX);
        let checks = all::<gtk::CheckButton>(&rig.window);
        assert_eq!(checks.len(), 4, "chapters, categories, custom cover, delete downloaded");
        assert!(checks.iter().all(|c| c.is_active()), "the saved default selects all");
        let buttons: Vec<String> = buttons_of(&rig.window).iter().filter_map(|b| b.label()).map(|l| l.to_string()).collect();
        assert_eq!(buttons, [tr(MsgKey::action_show_manga), tr(MsgKey::copy), tr(MsgKey::migrate)]);
        let disclosure = all::<gtk::Label>(&rig.window).into_iter().find(|l| l.label() == crate::browse::texts::MIGRATE_TRACKS_ALWAYS).expect("disclosure");
        assert!(disclosure.has_css_class("dim-label"), "the line is dim");
        assert!(!checks.iter().any(|c| c.label().as_deref() == Some(crate::browse::texts::MIGRATE_TRACKS_ALWAYS)), "and it is not a checkbox");
        let bare = rig_labels_only(false, false);
        assert_eq!(bare.len(), 2, "no custom cover and no downloads: two options");
        rig.window.close();
    }

    fn rig_labels_only(has_cover: bool, has_downloads: bool) -> Vec<gtk::CheckButton> {
        let rig = rig(has_cover, has_downloads, i32::MAX);
        let checks = all::<gtk::CheckButton>(&rig.window);
        rig.window.close();
        checks
    }

    fn a_run_saves_the_flags_first_disables_the_buttons_and_ignores_a_second_request() {
        let rig = rig(true, true, i32::MAX);
        let checks = all::<gtk::CheckButton>(&rig.window);
        checks[1].set_active(false);
        let buttons = buttons_of(&rig.window);
        buttons[2].emit_clicked();
        assert_eq!(
            *rig.log.borrow(),
            ["save 2147483645", "run replace=true flags=2147483645"],
            "the stored bitmap keeps the unused bits and drops the unticked one, and it is saved before the run"
        );
        assert!(!buttons[1].is_sensitive() && !buttons[2].is_sensitive(), "Copy and Migrate are disabled while it runs");
        buttons[1].emit_clicked();
        buttons[2].emit_clicked();
        assert_eq!(rig.log.borrow().len(), 2, "a second request is ignored");
        assert_eq!(rig.pending.borrow().len(), 1);
        rig.window.close();
    }

    fn a_failure_keeps_the_dialog_open_with_a_toast_and_lets_it_run_again() {
        let rig = rig(false, false, i32::MAX);
        let buttons = buttons_of(&rig.window);
        buttons[1].emit_clicked();
        let done = rig.pending.borrow_mut().pop().expect("run");
        done(Err(MigrateError::NewSourceMissing));
        assert_eq!(rig.log.borrow().last(), Some(&format!("toast {}", tr(MsgKey::internal_error))), "D-UI-4");
        assert!(!rig.log.borrow().contains(&"finished".to_owned()), "the screen is not popped");
        assert!(buttons[1].is_sensitive() && buttons[2].is_sensitive(), "the buttons come back");
        assert_eq!(rig.closes.get(), 0, "the dialog stays open");
        buttons[2].emit_clicked();
        assert_eq!(rig.pending.borrow().len(), 1, "a new run starts");
        rig.window.close();
    }

    fn a_success_closes_the_dialog_and_finishes_the_screen() {
        let rig = rig(false, false, i32::MAX);
        buttons_of(&rig.window)[2].emit_clicked();
        let done = rig.pending.borrow_mut().pop().expect("run");
        done(Ok(MigrateOutcome::default()));
        assert_eq!(rig.log.borrow().last(), Some(&"finished".to_owned()));
        assert_eq!(rig.closes.get(), 1, "the dialog closed");
    }

    fn the_keys_migrate_and_copy_and_show_manga_closes_the_dialog() {
        let rig = rig(false, false, i32::MAX);
        let controllers = rig.window.observe_controllers();
        let keys: Vec<gtk::EventControllerKey> = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).filter_map(|c| c.downcast().ok()).collect();
        let (ctrl, shift) = (gtk::gdk::ModifierType::CONTROL_MASK, gtk::gdk::ModifierType::CONTROL_MASK | gtk::gdk::ModifierType::SHIFT_MASK);
        let press = |modifiers: gtk::gdk::ModifierType| keys.iter().any(|k| k.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Return, &0_u32, &modifiers]));
        assert!(press(ctrl), "Ctrl+Enter is taken");
        assert_eq!(rig.log.borrow().last().map(String::as_str), Some("run replace=true flags=2147483623"));
        rig.pending.borrow_mut().clear();
        rig.window.close();

        let rig = rig_again();
        let controllers = rig.window.observe_controllers();
        let keys: Vec<gtk::EventControllerKey> = (0..controllers.n_items()).filter_map(|i| controllers.item(i)).filter_map(|c| c.downcast().ok()).collect();
        assert!(keys.iter().any(|k| k.emit_by_name::<bool>("key-pressed", &[&gtk::gdk::Key::Return, &0_u32, &shift])), "Ctrl+Shift+Enter is taken");
        assert_eq!(rig.log.borrow().last().map(String::as_str), Some("run replace=false flags=2147483623"));
        rig.window.close();

        let rig = rig_again();
        buttons_of(&rig.window)[0].emit_clicked();
        assert_eq!(*rig.log.borrow(), ["show"]);
        assert_eq!(rig.closes.get(), 1, "Show manga closes the dialog");
    }

    fn rig_again() -> Rig {
        rig(false, false, i32::MAX)
    }

    pub(crate) fn smoke() {
        the_dialog_offers_the_options_the_three_buttons_and_the_dim_disclosure();
        a_run_saves_the_flags_first_disables_the_buttons_and_ignores_a_second_request();
        a_failure_keeps_the_dialog_open_with_a_toast_and_lets_it_run_again();
        a_success_closes_the_dialog_and_finishes_the_screen();
        the_keys_migrate_and_copy_and_show_manga_closes_the_dialog();
    }
}
