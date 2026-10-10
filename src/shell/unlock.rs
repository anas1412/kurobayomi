//! The unlock window of the app lock (Kotlin `UnlockActivity`, T-53a-070).
//!
//! The title is the `unlock_app_title` string with the app name. Success unlocks and shows the main window. A wrong
//! password shows PAM's text and keeps the window open (T-52a-033). An authentication error, or closing the window,
//! quits the application, as Android's `finishAffinity` did.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;

use gtk::prelude::*;
use kuro_i18n::{Arg, MsgKey, tr, trf};
use kuro_prefs::applock::{AppLockConfig, AppLockState, on_application_start};
use kuro_prefs::security::SecurityPrefs;

use crate::util::app_lock_auth::{AuthError, Authenticator, PamBackend, PromptGate, PromptGuard};

/// The app name in the title.
pub const APP_NAME: &str = "Kurobayomi";

/// The authenticator the window runs its checks on.
pub type SystemAuthenticator = Authenticator<Box<dyn PamBackend + Send + Sync>>;

/// "Unlock Kurobayomi".
pub fn title() -> String {
    trf(MsgKey::unlock_app_title, &[("arg1", Arg::Str(APP_NAME))])
}

/// What happened in the window.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnlockEvent {
    /// A password was checked.
    Checked(Result<(), AuthError>),
    /// The window was closed or cancelled.
    Cancelled,
}

/// What the window does next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnlockStep {
    /// Unlock and show the main window.
    Unlock,
    /// Stay locked and show this text.
    ShowError(String),
    /// Quit the application.
    Quit,
}

pub fn step(event: UnlockEvent) -> UnlockStep {
    match event {
        UnlockEvent::Checked(Ok(())) => UnlockStep::Unlock,
        UnlockEvent::Checked(Err(error @ (AuthError::Pam(_) | AuthError::EmptyPassword))) => UnlockStep::ShowError(error.to_string()),
        UnlockEvent::Checked(Err(AuthError::Unsupported)) | UnlockEvent::Cancelled => UnlockStep::Quit,
    }
}

/// Opens the unlock window, or returns `None` when one is already open (`gate`). `on_unlock` and `on_quit` run on the
/// GTK thread, once.
pub fn present(
    app: Option<&gtk::Application>,
    gate: &PromptGate,
    auth: Arc<SystemAuthenticator>,
    on_unlock: impl Fn() + 'static,
    on_quit: impl Fn() + 'static,
) -> Option<gtk::Window> {
    let guard = gate.try_open()?;
    let window = gtk::Window::builder().title(title()).modal(true).resizable(false).build();
    window.set_application(app);
    let entry = gtk::PasswordEntry::builder().show_peek_icon(true).activates_default(true).build();
    let error = gtk::Label::builder().wrap(true).visible(false).css_classes(["error"]).build();
    let ok = gtk::Button::with_label(&tr(MsgKey::action_ok));
    ok.add_css_class("suggested-action");
    let cancel = gtk::Button::with_label(&tr(MsgKey::action_cancel));
    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    buttons.append(&cancel);
    buttons.append(&ok);
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    column.set_margin_top(18);
    column.set_margin_bottom(18);
    column.set_margin_start(18);
    column.set_margin_end(18);
    column.append(&gtk::Label::new(Some(&tr(MsgKey::password))));
    column.append(&entry);
    column.append(&error);
    column.append(&buttons);
    window.set_child(Some(&column));
    window.set_default_widget(Some(&ok));

    let on_unlock: Rc<dyn Fn()> = Rc::new(on_unlock);
    let on_quit: Rc<dyn Fn()> = Rc::new(on_quit);
    let finished = Rc::new(Cell::new(false));
    let held: Rc<RefCell<Option<PromptGuard>>> = Rc::new(RefCell::new(Some(guard)));

    // closing the window ends the lock: unlocked windows were marked finished first, so only a cancel quits
    {
        let (finished, held, on_quit) = (finished.clone(), held.clone(), on_quit.clone());
        window.connect_close_request(move |_| {
            held.borrow_mut().take();
            if !finished.replace(true) {
                on_quit();
            }
            gtk::glib::Propagation::Proceed
        });
    }
    {
        let window = window.clone();
        cancel.connect_clicked(move |_| window.close());
    }

    let submit = {
        let (window, entry, error, ok) = (window.clone(), entry.clone(), error.clone(), ok.clone());
        move || {
            let password = entry.text().to_string();
            let user = gtk::glib::user_name().to_string_lossy().into_owned();
            ok.set_sensitive(false);
            let (tx, rx) = async_channel::bounded(1);
            let auth = auth.clone();
            std::thread::spawn(move || {
                let _ = tx.send_blocking(auth.authenticate(&user, &password));
            });
            let (window, entry, error, ok, finished) = (window.clone(), entry.clone(), error.clone(), ok.clone(), finished.clone());
            let (on_unlock, on_quit) = (on_unlock.clone(), on_quit.clone());
            gtk::glib::spawn_future_local(async move {
                let Ok(result) = rx.recv().await else { return };
                match step(UnlockEvent::Checked(result)) {
                    UnlockStep::Unlock => {
                        finished.set(true);
                        window.close();
                        on_unlock();
                    }
                    UnlockStep::ShowError(text) => {
                        error.set_text(&text);
                        error.set_visible(true);
                        entry.set_text("");
                        ok.set_sensitive(true);
                        entry.grab_focus();
                    }
                    UnlockStep::Quit => {
                        finished.set(true);
                        window.close();
                        on_quit();
                    }
                }
            });
        }
    };
    let submit = Rc::new(submit);
    {
        let submit = submit.clone();
        ok.connect_clicked(move |_| submit());
    }
    entry.connect_activate(move |_| submit());
    window.present();
    Some(window)
}

/// The unlock method of the app (decision A of `p1-rust-design` section 9): the passphrase the Require unlock switch
/// sets. It stands where PAM stood in the Kotlin-to-Linux plan, so the window checks the same secret the settings gate
/// uses. `service_available` is false only when the store answers that no passphrase is set.
pub struct PassphraseBackend {
    store: Arc<dyn kuro_platform::secrets::SecretStore>,
}

impl PassphraseBackend {
    pub fn new(store: Arc<dyn kuro_platform::secrets::SecretStore>) -> Self {
        Self { store }
    }

    /// The app's secret store. Without platform paths the store is empty, so the lock stays off.
    pub fn system() -> Self {
        use kuro_platform::linux::LinuxPaths;
        match LinuxPaths::system() {
            Ok(paths) => Self::new(Arc::new(kuro_platform::linux::secrets::open(&paths))),
            Err(_) => Self::new(Arc::new(kuro_prefs::memory::MemorySecrets::default())),
        }
    }

    /// `Some(false)` when no passphrase is set; `None` when the store cannot be read.
    pub fn is_set(&self) -> Option<bool> {
        crate::settings_ui::auth::Passphrase::new(self.store.as_ref()).is_set().ok()
    }
}

impl PamBackend for PassphraseBackend {
    fn service_available(&self) -> bool {
        // an unreadable store keeps the window open and shows the error on the check, so the lock never opens by accident
        self.is_set() != Some(false)
    }

    fn check(&self, _user: &str, password: &str) -> Result<(), String> {
        match crate::settings_ui::auth::Passphrase::new(self.store.as_ref()).verify(password) {
            Ok(true) => Ok(()),
            Ok(false) => Err(crate::settings_ui::auth::WRONG_PASSPHRASE.to_owned()),
            Err(message) => Err(message),
        }
    }
}

/// Whether the main window waits behind the unlock window at start: the lock rules say so, and a passphrase exists.
/// `stored` is `__APP_STATE_last_app_closed`, `method_set` is [`PassphraseBackend::is_set`].
pub fn must_unlock(cfg: AppLockConfig, stored: Option<i64>, now: i64, method_set: Option<bool>) -> bool {
    let from = AppLockState { last_app_closed: stored, ..AppLockState::at_process_start() };
    cfg.use_authenticator && on_application_start(cfg, false, now, from).require_unlock && method_set != Some(false)
}

/// Window start (T-53a-070): when the lock is on, opens the unlock window and shows `win` only after it succeeds; an
/// authentication error or a cancel quits. Returns false when no unlock is needed, and the caller presents `win` itself.
pub fn gate(app: &gtk::Application, win: &gtk::ApplicationWindow, sec: &SecurityPrefs) -> bool {
    let cfg = AppLockConfig { use_authenticator: sec.use_authenticator.get(), lock_app_after: sec.lock_app_after.get() };
    // the secret store is read only while the lock is on
    let backend = cfg.use_authenticator.then(PassphraseBackend::system);
    let stored = sec.last_app_closed.is_set().then(|| sec.last_app_closed.get());
    let now = i64::try_from(kuro_core::crash::now_ms()).unwrap_or(i64::MAX);
    let method_set = backend.as_ref().map(PassphraseBackend::is_set);
    if !must_unlock(cfg, stored, now, method_set.unwrap_or(None)) {
        return false;
    }
    let Some(backend) = backend else { return false };
    let auth: Arc<SystemAuthenticator> = Arc::new(Authenticator::new(Box::new(backend) as Box<dyn PamBackend + Send + Sync>));
    let (show, quit_app) = (win.clone(), app.clone());
    let opened = present(Some(app), &PromptGate::new(), auth, move || show.present(), move || quit_app.quit());
    opened.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_unlocks() {
        assert_eq!(step(UnlockEvent::Checked(Ok(()))), UnlockStep::Unlock);
    }

    #[test]
    fn a_wrong_password_stays_locked_with_pam_text() {
        assert_eq!(
            step(UnlockEvent::Checked(Err(AuthError::Pam("Authentication failure".into())))),
            UnlockStep::ShowError("Authentication failure".into())
        );
        assert!(matches!(step(UnlockEvent::Checked(Err(AuthError::EmptyPassword))), UnlockStep::ShowError(_)));
    }

    #[test]
    fn an_authentication_error_or_a_cancel_quits() {
        assert_eq!(step(UnlockEvent::Checked(Err(AuthError::Unsupported))), UnlockStep::Quit);
        assert_eq!(step(UnlockEvent::Cancelled), UnlockStep::Quit);
    }

    #[test]
    fn the_title_uses_the_unlock_key_with_the_app_name() {
        assert_eq!(title(), "Unlock Kurobayomi");
    }

    #[test]
    fn a_second_window_does_not_open_while_the_first_is_open() {
        crate::testing::with_display(|| {
            let gate = PromptGate::new();
            let auth = Arc::new(crate::util::app_lock_auth::system_authenticator());
            let first = present(None, &gate, auth.clone(), || {}, || {});
            assert!(first.is_some());
            assert!(present(None, &gate, auth, || {}, || {}).is_none());
            first.unwrap().close();
            assert!(!gate.is_authenticating());
        });
    }

    #[test]
    fn the_main_window_waits_only_when_the_lock_is_on_and_a_passphrase_exists() {
        let on = AppLockConfig { use_authenticator: true, lock_app_after: 5 };
        let off = AppLockConfig { use_authenticator: false, lock_app_after: 5 };
        assert!(must_unlock(on, None, 1_000, Some(true)));
        assert!(must_unlock(on, Some(1), 1_000, None), "an unreadable store does not open the lock");
        assert!(!must_unlock(on, None, 1_000, Some(false)), "no passphrase, no lock");
        assert!(!must_unlock(off, None, 1_000, Some(true)));
    }

    #[test]
    fn the_passphrase_backend_checks_the_stored_passphrase() {
        let store: Arc<dyn kuro_platform::secrets::SecretStore> = Arc::new(kuro_prefs::memory::MemorySecrets::default());
        let backend = PassphraseBackend::new(store.clone());
        let auth = Authenticator::new(backend);
        assert_eq!(auth.authenticate("u", "abcd"), Err(AuthError::Unsupported), "no passphrase is set");
        crate::settings_ui::auth::Passphrase::new(store.as_ref()).set("abcd").unwrap();
        assert_eq!(auth.authenticate("u", "abcd"), Ok(()));
        assert_eq!(
            auth.authenticate("u", "nope"),
            Err(AuthError::Pam(crate::settings_ui::auth::WRONG_PASSPHRASE.to_owned()))
        );
        assert_eq!(auth.authenticate("u", ""), Err(AuthError::EmptyPassword));
    }
}
