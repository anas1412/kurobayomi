//! Onboarding wizard (Kotlin `OnboardingScreen`, four steps): theme, storage folder, desktop status, guides and restore
//! (T-36a-024, T-36a-025, T-36a-026, T-36a-028). The step rules are pure; `page` builds the GTK page.
//!
//! Finish and restore go through `pref_model::onboarding::{finish, restore}`, so the completion flag is set in one place.

use std::cell::RefCell;
use std::rc::Rc;

use gtk::prelude::*;
use kuro_i18n::{MsgKey, tr};
use kuro_prefs::pref::Pref;
use kuro_prefs::store::PrefStore;
use kuro_prefs::ui::{AppTheme, ThemeMode, UiPrefs};

use crate::pref_model::widgets::{app_theme, theme_mode};

/// The app-state key of the base storage directory (`__APP_STATE_storage_dir`).
pub const STORAGE_KEY: &str = "__APP_STATE_storage_dir";
/// Storage help (behaviour 13.3).
pub const STORAGE_HELP_URL: &str = "https://aniyomi.org/docs/faq/storage";
/// Getting-started guide (behaviour 13.5).
pub const GETTING_STARTED_URL: &str = "https://aniyomi.org/docs/guides/getting-started";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Theme,
    Storage,
    Desktop,
    Guides,
}

impl Step {
    pub const ALL: [Step; 4] = [Step::Theme, Step::Storage, Step::Desktop, Step::Guides];

    /// Only the storage step blocks Next: it needs a folder. The other three are always complete.
    pub fn is_complete(self, storage_set: bool) -> bool {
        match self {
            Step::Storage => storage_set,
            Step::Theme | Step::Desktop | Step::Guides => true,
        }
    }
}

/// What Next did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Accept {
    Next,
    Finish,
    Blocked,
}

/// The wizard position. Back is blocked on the first step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Wizard {
    index: usize,
    storage_set: bool,
}

impl Wizard {
    pub fn new(storage_set: bool) -> Self {
        Self { index: 0, storage_set }
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn step(&self) -> Step {
        Step::ALL[self.index]
    }

    pub fn is_last(&self) -> bool {
        self.index == Step::ALL.len() - 1
    }

    pub fn can_accept(&self) -> bool {
        self.step().is_complete(self.storage_set)
    }

    /// The Next or Finish label key: Finish on the last step, Next otherwise.
    pub fn accept_key(&self) -> MsgKey {
        if self.is_last() { MsgKey::onboarding_action_finish } else { MsgKey::onboarding_action_next }
    }

    pub fn set_storage_set(&mut self, set: bool) {
        self.storage_set = set;
    }

    /// Next: advances, or finishes on the last step. Blocked while the current step is incomplete.
    pub fn accept(&mut self) -> Accept {
        if !self.can_accept() {
            return Accept::Blocked;
        }
        if self.is_last() {
            return Accept::Finish;
        }
        self.index += 1;
        Accept::Next
    }

    /// Back: steps back. Returns false on the first step.
    pub fn back(&mut self) -> bool {
        if self.index == 0 {
            return false;
        }
        self.index -= 1;
        true
    }
}

/// The wizard is shown until the completion flag is set.
pub fn wizard_needed(complete_flag: bool) -> bool {
    !complete_flag
}

/// The widget name of the onboarding page. The window treats it like the reader and the player: no sidebar, no bar.
pub const PAGE_NAME: &str = "screen:onboarding";

/// The wizard shows on the first launch (the completion flag is off), and never in a dev run (smoke test, screenshot
/// or a fixed start screen).
pub fn should_show(complete_flag: bool, dev_run: bool) -> bool {
    wizard_needed(complete_flag) && !dev_run
}

/// Pushes the wizard over the window when [`should_show`] says so, and blocks closing the window until it is done.
/// Finish and Restore leave the page; Restore then opens Data and storage with the Backup row highlighted.
pub fn show_if_needed(win: &gtk::ApplicationWindow, store: &PrefStore, dev_run: bool) {
    let base = &crate::data::app().base;
    if !should_show(base.shown_onboarding_flow.get(), dev_run) {
        return;
    }
    crate::pref_model::onboarding::guard(win, base.shown_onboarding_flow.clone());
    let wizard = page(store, crate::ui::pop, || crate::ui::open(crate::pref_model::onboarding::DATA_AND_STORAGE));
    wizard.set_widget_name(PAGE_NAME);
    crate::ui::push(&wizard);
}

/// Builds the onboarding page. `on_leave` runs after the flag is set (finish); `on_restore` runs after the flag is set
/// and asks the caller to open Data and storage with the restore row highlighted.
pub fn page(store: &PrefStore, on_leave: impl Fn() + 'static, on_restore: impl Fn() + 'static) -> gtk::Box {
    let complete = crate::data::app().base.shown_onboarding_flow.clone();
    let storage: Pref<String> = Pref::new(store.clone(), STORAGE_KEY, String::new());
    let wizard = Rc::new(RefCell::new(Wizard::new(!storage.get().is_empty())));
    let ui = UiPrefs::new(store.clone());
    let on_leave: Rc<dyn Fn()> = Rc::new(on_leave);
    let on_restore: Rc<dyn Fn()> = Rc::new(on_restore);

    let pages = gtk::Stack::new();
    pages.set_transition_type(gtk::StackTransitionType::SlideLeftRight);
    pages.add_named(&theme_step(&ui), Some("theme"));
    let storage_step = storage_step(&storage);
    pages.add_named(&storage_step.widget, Some("storage"));
    pages.add_named(&desktop_step(), Some("desktop"));
    pages.add_named(&guides_step(&complete, on_leave.clone(), on_restore), Some("guides"));

    let back = gtk::Button::with_label(&tr(MsgKey::action_webview_back));
    let next = gtk::Button::builder().css_classes(["suggested-action"]).build();
    let refresh = {
        let (wizard, pages, back, next) = (wizard.clone(), pages.clone(), back.clone(), next.clone());
        Rc::new(move || {
            let w = wizard.borrow();
            pages.set_visible_child_name(["theme", "storage", "desktop", "guides"][w.index()]);
            back.set_sensitive(w.index() > 0);
            next.set_label(&tr(w.accept_key()));
            next.set_sensitive(w.can_accept());
        })
    };
    *storage_step.on_change.borrow_mut() = Some({
        let (wizard, refresh) = (wizard.clone(), refresh.clone());
        Box::new(move |set| {
            wizard.borrow_mut().set_storage_set(set);
            refresh();
        })
    });
    refresh();
    {
        let (wizard, refresh) = (wizard.clone(), refresh.clone());
        back.connect_clicked(move |_| {
            wizard.borrow_mut().back();
            refresh();
        });
    }
    {
        let (wizard, refresh, complete, on_leave) = (wizard.clone(), refresh.clone(), complete.clone(), on_leave.clone());
        next.connect_clicked(move |_| {
            let accepted = wizard.borrow_mut().accept();
            if accepted == Accept::Finish {
                crate::pref_model::onboarding::finish(&complete, || on_leave());
            }
            refresh();
        });
    }

    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    controls.set_halign(gtk::Align::End);
    controls.append(&back);
    controls.append(&next);
    let root = gtk::Box::new(gtk::Orientation::Vertical, 16);
    root.set_margin_start(16);
    root.set_margin_end(16);
    root.set_margin_top(16);
    root.set_margin_bottom(16);
    root.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_heading)).css_classes(["title-1"]).xalign(0.0).build());
    root.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_description)).wrap(true).xalign(0.0).build());
    root.append(&pages);
    root.append(&controls);
    root
}

/// Step 1: the theme mode, the app theme and AMOLED. Each choice is saved at once; the mode also applies to GTK at once.
fn theme_step(ui: &UiPrefs) -> gtk::Box {
    let modes = theme_mode::SEGMENTS;
    let mode_labels: Vec<String> = modes.iter().map(|m| theme_mode::segment_label(*m)).collect();
    let mode_refs: Vec<&str> = mode_labels.iter().map(String::as_str).collect();
    let mode_drop = gtk::DropDown::from_strings(&mode_refs);
    mode_drop.set_selected(modes.iter().position(|m| *m == ui.theme_mode.get()).unwrap_or(0) as u32);
    {
        let (ui, modes) = (ui.clone(), modes);
        mode_drop.connect_selected_notify(move |d| {
            let mode: ThemeMode = modes[d.selected() as usize];
            if theme_mode::should_write(ui.theme_mode.get(), mode) {
                let _ = ui.theme_mode.set(mode);
                theme_mode::apply(mode);
            }
        });
    }
    let themes = app_theme::strip_themes();
    let theme_labels: Vec<String> = themes.iter().map(|t| app_theme::card_title(*t)).collect();
    let theme_refs: Vec<&str> = theme_labels.iter().map(String::as_str).collect();
    let theme_drop = gtk::DropDown::from_strings(&theme_refs);
    let current = app_theme::normalise(ui.app_theme.get());
    theme_drop.set_selected(themes.iter().position(|t| *t == current).unwrap_or(0) as u32);
    {
        let (ui, themes) = (ui.clone(), themes.clone());
        theme_drop.connect_selected_notify(move |d| {
            let theme: AppTheme = themes[d.selected() as usize];
            let _ = ui.app_theme.set(theme);
        });
    }
    let amoled = gtk::Switch::builder().active(ui.theme_dark_amoled.get()).valign(gtk::Align::Center).build();
    {
        let ui = ui.clone();
        amoled.connect_active_notify(move |s| {
            let _ = ui.theme_dark_amoled.set(s.is_active());
        });
    }
    let amoled_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    amoled_row.append(&gtk::Label::builder().label(tr(MsgKey::pref_dark_theme_pure_black)).hexpand(true).xalign(0.0).build());
    amoled_row.append(&amoled);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.append(&mode_drop);
    column.append(&theme_drop);
    column.append(&amoled_row);
    column
}

/// Step 2 result: the step widget and the callback the wizard uses to learn whether a folder is set.
struct StorageStep {
    widget: gtk::Box,
    on_change: RefCell<Option<Box<dyn Fn(bool)>>>,
}

/// Step 2: the storage folder. Select opens the folder chooser (the portal when present, GTK otherwise). Cancel leaves the
/// step incomplete with no error.
fn storage_step(storage: &Pref<String>) -> Rc<StorageStep> {
    let step = Rc::new(StorageStep { widget: gtk::Box::new(gtk::Orientation::Vertical, 8), on_change: RefCell::new(None) });
    let shown = gtk::Label::builder().label(storage.get()).wrap(true).selectable(true).xalign(0.0).build();
    let select = gtk::Button::with_label(&tr(MsgKey::onboarding_storage_action_select));
    let help = gtk::Button::with_label(&tr(MsgKey::onboarding_storage_help_action));
    {
        let (storage, shown, step) = (storage.clone(), shown.clone(), step.clone());
        select.connect_clicked(move |button| {
            let dialog = gtk::FileDialog::new();
            let window = button.root().and_then(|r| r.downcast::<gtk::Window>().ok());
            let (storage, shown, step) = (storage.clone(), shown.clone(), step.clone());
            dialog.select_folder(window.as_ref(), None::<&gtk::gio::Cancellable>, move |result| {
                // a cancelled chooser is not an error: the step stays incomplete
                let Ok(folder) = result else { return };
                let Some(path) = folder.path() else { return };
                let text = path.to_string_lossy().into_owned();
                if storage.set(text.clone()).is_ok() {
                    shown.set_label(&text);
                    if let Some(cb) = step.on_change.borrow().as_ref() {
                        cb(true);
                    }
                }
            });
        });
    }
    help.connect_clicked(|button| {
        let window = button.root().and_then(|r| r.downcast::<gtk::Window>().ok());
        gtk::UriLauncher::new(STORAGE_HELP_URL).launch(window.as_ref(), None::<&gtk::gio::Cancellable>, |_| {});
    });
    step.widget.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_storage_info)).wrap(true).xalign(0.0).build());
    step.widget.append(&shown);
    step.widget.append(&select);
    step.widget.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    step.widget.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_storage_help_info)).wrap(true).xalign(0.0).build());
    step.widget.append(&help);
    step
}

/// Step 3: informational only. Notifications are available on the desktop; no grant buttons.
fn desktop_step() -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    column.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_permission_notifications)).wrap(true).xalign(0.0).build());
    column.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_permission_notifications_description)).wrap(true).xalign(0.0).build());
    column
}

/// Step 4: the getting-started guide, and the restore entry that finishes onboarding and opens Data and storage.
fn guides_step(complete: &Pref<bool>, on_leave: Rc<dyn Fn()>, on_restore: Rc<dyn Fn()>) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 8);
    let guide = gtk::Button::with_label(&tr(MsgKey::getting_started_guide));
    guide.connect_clicked(|button| {
        let window = button.root().and_then(|r| r.downcast::<gtk::Window>().ok());
        gtk::UriLauncher::new(GETTING_STARTED_URL).launch(window.as_ref(), None::<&gtk::gio::Cancellable>, |_| {});
    });
    let restore = gtk::Button::with_label(&tr(MsgKey::pref_restore_backup));
    {
        let complete = complete.clone();
        restore.connect_clicked(move |_| {
            crate::pref_model::onboarding::restore(&complete, || on_leave(), || on_restore());
        });
    }
    column.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_guides_new_user)).wrap(true).xalign(0.0).build());
    column.append(&guide);
    column.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    column.append(&gtk::Label::builder().label(tr(MsgKey::onboarding_guides_returning_user)).wrap(true).xalign(0.0).build());
    column.append(&restore);
    column
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_is_disabled_on_storage_until_a_folder_is_set() {
        let mut w = Wizard::new(false);
        assert_eq!(w.accept(), Accept::Next, "theme step is complete");
        assert_eq!(w.step(), Step::Storage);
        assert!(!w.can_accept());
        assert_eq!(w.accept(), Accept::Blocked);
        w.set_storage_set(true);
        assert!(w.can_accept());
        assert_eq!(w.accept(), Accept::Next);
    }

    #[test]
    fn finish_label_and_action_on_the_last_step_only() {
        let mut w = Wizard::new(true);
        assert_eq!(w.accept_key(), MsgKey::onboarding_action_next);
        w.accept();
        w.accept();
        w.accept();
        assert_eq!(w.step(), Step::Guides);
        assert_eq!(w.accept_key(), MsgKey::onboarding_action_finish);
        assert_eq!(w.accept(), Accept::Finish);
    }

    #[test]
    fn back_is_blocked_on_the_first_step_and_steps_back_otherwise() {
        let mut w = Wizard::new(true);
        assert!(!w.back());
        assert_eq!(w.index(), 0);
        w.accept();
        assert!(w.back());
        assert_eq!(w.step(), Step::Theme);
    }

    #[test]
    fn wizard_is_needed_until_the_flag_is_set() {
        assert!(wizard_needed(false));
        assert!(!wizard_needed(true));
    }

    #[test]
    fn the_wizard_shows_on_first_launch_only_and_not_in_a_dev_run() {
        assert!(should_show(false, false));
        assert!(!should_show(true, false));
        assert!(!should_show(false, true));
    }

    #[test]
    fn desktop_and_guides_never_block() {
        assert!(Step::Theme.is_complete(false));
        assert!(Step::Desktop.is_complete(false));
        assert!(Step::Guides.is_complete(false));
    }

    #[test]
    fn help_and_guide_urls_are_the_documented_ones() {
        assert_eq!(STORAGE_HELP_URL, "https://aniyomi.org/docs/faq/storage");
        assert_eq!(GETTING_STARTED_URL, "https://aniyomi.org/docs/guides/getting-started");
    }
}
