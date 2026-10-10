//! The unlock method and the one modal prompt every gate uses (T-47a-063, T-47a-067, T-47a-068).
//!
//! Method (decision A of p1-rust-design section 9): an app passphrase. Its argon2id hash, as a PHC string, is the secret
//! `kurobayomi-applock` in the app's secret store; the passphrase itself is never stored. Fingerprint is not built.
//! The method exists while the Require unlock switch is on: turning the switch on sets the passphrase, turning it off
//! (after an unlock) removes it. [`confirm`] passes at once when no passphrase is set (T-47a-067), so the Hide
//! content, lock-when-idle and NSFW rows ask only while the lock is on.
use std::sync::{Arc, OnceLock};

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{Error as HashError, PasswordHasher, PasswordVerifier};
use argon2::Argon2;
use gtk::prelude::*;
use kuro_platform::secrets::SecretStore;

use crate::pref_model::widgets::dialog;

pub const CONFIRM_LOCK_CHANGE: &str = "Authenticate to confirm change";
pub const UNLOCK_BUTTON: &str = "Unlock";
pub const SET_BUTTON: &str = "Set passphrase";
pub const SET_TITLE: &str = "Set an unlock passphrase";
pub const SET_SUBTITLE: &str = "The app asks for it before changing the lock, the NSFW setting and the lock delay.";
pub const WRONG_PASSPHRASE: &str = "Wrong passphrase";
pub const TOO_SHORT: &str = "Use at least 4 characters";
pub const MISMATCH: &str = "The two passphrases differ";
pub const MIN_LENGTH: usize = 4;

const SERVICE: &str = "kurobayomi-applock";
const ACCOUNT: &str = "passphrase";

/// The passphrase method over a secret store.
pub struct Passphrase<'a>(&'a dyn SecretStore);

impl<'a> Passphrase<'a> {
    pub fn new(store: &'a dyn SecretStore) -> Self {
        Self(store)
    }

    /// Whether a passphrase is set, which is whether the method exists.
    pub fn is_set(&self) -> Result<bool, String> {
        self.0.get(SERVICE, ACCOUNT).map(|hash| hash.is_some()).map_err(|e| e.to_string())
    }

    pub fn set(&self, passphrase: &str) -> Result<(), String> {
        let hash = Argon2::default().hash_password(passphrase.as_bytes()).map_err(|e| e.to_string())?;
        self.0.set(SERVICE, ACCOUNT, &hash.to_string()).map_err(|e| e.to_string())
    }

    /// `Ok(true)` for the right passphrase, `Ok(false)` for a wrong one, `Err` when the hash cannot be read.
    pub fn verify(&self, passphrase: &str) -> Result<bool, String> {
        let stored = self.0.get(SERVICE, ACCOUNT).map_err(|e| e.to_string())?.ok_or("No passphrase is set")?;
        let hash = PasswordHash::new(&stored).map_err(|e| e.to_string())?;
        match Argon2::default().verify_password(passphrase.as_bytes(), &hash) {
            Ok(()) => Ok(true),
            Err(HashError::PasswordInvalid) => Ok(false),
            Err(e) => Err(e.to_string()),
        }
    }

    pub fn clear(&self) -> Result<(), String> {
        self.0.delete(SERVICE, ACCOUNT).map_err(|e| e.to_string())
    }
}

/// The rule for a new passphrase and its repeat.
pub fn check_new(passphrase: &str, repeat: &str) -> Result<(), &'static str> {
    if passphrase.chars().count() < MIN_LENGTH {
        Err(TOO_SHORT)
    } else if passphrase != repeat {
        Err(MISMATCH)
    } else {
        Ok(())
    }
}

/// The app's secret store, opened once. Tests get an empty in-memory one.
fn store() -> Arc<dyn SecretStore> {
    static STORE: OnceLock<Arc<dyn SecretStore>> = OnceLock::new();
    STORE.get_or_init(open).clone()
}

#[cfg(not(test))]
fn open() -> Arc<dyn SecretStore> {
    use kuro_platform::linux::LinuxPaths;
    match LinuxPaths::system() {
        Ok(paths) => Arc::new(kuro_platform::linux::secrets::open(&paths)),
        Err(_) => Arc::new(kuro_prefs::memory::MemorySecrets::default()),
    }
}

#[cfg(test)]
fn open() -> Arc<dyn SecretStore> {
    Arc::new(kuro_prefs::memory::MemorySecrets::default())
}

/// Runs a store call off the GTK thread: it may wait on the secret service, and hashing takes a moment. Both
/// the secret store and hashing are blocking work, so the gio thread pool is used like the rest of the app.
async fn blocking<T: Send + 'static>(work: impl FnOnce(&Passphrase<'_>) -> T + Send + 'static) -> T {
    let run = move || work(&Passphrase::new(store().as_ref()));
    #[cfg(test)]
    {
        run()
    }
    #[cfg(not(test))]
    {
        gtk::gio::spawn_blocking(run).await.expect("the unlock task panicked")
    }
}

fn parent() -> Option<gtk::Window> {
    gtk::Window::list_toplevels().into_iter().find_map(|w| w.downcast::<gtk::Window>().ok())
}

fn entry() -> gtk::PasswordEntry {
    gtk::PasswordEntry::builder().show_peek_icon(true).activates_default(false).build()
}

fn body(text: &str, entries: &[&gtk::PasswordEntry]) -> gtk::Box {
    let column = gtk::Box::new(gtk::Orientation::Vertical, 12);
    let label = gtk::Label::new(Some(text));
    label.set_wrap(true);
    label.set_xalign(0.0);
    column.append(&label);
    for e in entries {
        column.append(*e);
    }
    column
}

/// `auth::confirm(title, subtitle)` (T-47a-068): true when no unlock method is set, otherwise the verdict of the one
/// modal passphrase dialog. A failing secret store refuses (the lock must not open by accident) and shows its error.
pub async fn confirm(title: &str, subtitle: &str) -> bool {
    match blocking(|p| p.is_set()).await {
        Ok(false) => return true,
        Ok(true) => {}
        Err(message) => {
            crate::toast::notify(message);
            return false;
        }
    }
    let Some(parent) = parent() else { return false };
    let field = entry();
    let (tx, rx) = async_channel::bounded::<bool>(1);
    let on_cancel = tx.clone();
    dialog::show(
        &parent,
        title,
        &body(subtitle, &[&field]),
        Some(UNLOCK_BUTTON),
        {
            let field = field.clone();
            move |dialog| {
                let typed = field.text().to_string();
                dialog.set_confirm_enabled(false);
                let (field, tx) = (field.clone(), tx.clone());
                gtk::glib::spawn_future_local(async move {
                    match blocking(move |p| p.verify(&typed)).await {
                        Ok(true) => {
                            let _ = tx.try_send(true);
                            dialog.finish();
                        }
                        Ok(false) => {
                            dialog.set_error(Some(WRONG_PASSPHRASE));
                            dialog.set_confirm_enabled(true);
                            field.select_region(0, -1);
                        }
                        Err(message) => {
                            dialog.set_error(Some(&message));
                            dialog.set_confirm_enabled(true);
                        }
                    }
                });
            }
        },
        move || {
            let _ = on_cancel.try_send(false);
        },
    );
    field.grab_focus();
    rx.recv().await.unwrap_or(false)
}

/// The dialog that sets the passphrase. True once it is stored, false when cancelled.
async fn create() -> Result<bool, String> {
    let Some(parent) = parent() else { return Ok(false) };
    let (first, repeat) = (entry(), entry());
    repeat.set_placeholder_text(Some("Repeat the passphrase"));
    let (tx, rx) = async_channel::bounded::<Result<bool, String>>(1);
    let on_cancel = tx.clone();
    dialog::show(
        &parent,
        SET_TITLE,
        &body(SET_SUBTITLE, &[&first, &repeat]),
        Some(SET_BUTTON),
        {
            let (first, repeat) = (first.clone(), repeat.clone());
            move |dialog| {
                let typed = first.text().to_string();
                if let Err(message) = check_new(&typed, &repeat.text()) {
                    dialog.set_error(Some(message));
                    return;
                }
                dialog.set_confirm_enabled(false);
                let tx = tx.clone();
                gtk::glib::spawn_future_local(async move {
                    match blocking(move |p| p.set(&typed)).await {
                        Ok(()) => {
                            let _ = tx.try_send(Ok(true));
                            dialog.finish();
                        }
                        Err(message) => {
                            dialog.set_error(Some(&message));
                            dialog.set_confirm_enabled(true);
                        }
                    }
                });
            }
        },
        move || {
            let _ = on_cancel.try_send(Ok(false));
        },
    );
    first.grab_focus();
    rx.recv().await.unwrap_or(Ok(false))
}

/// The veto of the Require unlock switch: on sets the passphrase (or unlocks, when one is already set), off needs an
/// unlock and then removes the passphrase. `Ok(false)` keeps the switch where it was.
pub async fn set_lock(title: &str, on: bool) -> Result<bool, String> {
    if on && !blocking(|p| p.is_set()).await? {
        return create().await;
    }
    if !confirm(title, CONFIRM_LOCK_CHANGE).await {
        return Ok(false);
    }
    if !on {
        blocking(|p| p.clear()).await?;
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use kuro_prefs::memory::MemorySecrets;

    use super::*;

    /// The futures here resolve at once (no dialog opens), so one poll is enough.
    fn block_on<F: std::future::Future>(future: F) -> F::Output {
        let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
        match std::pin::pin!(future).as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => v,
            std::task::Poll::Pending => panic!("a gate that needs no dialog stayed pending"),
        }
    }

    #[test]
    fn no_method_set_means_confirm_passes_without_a_dialog() {
        assert!(block_on(confirm("Title", CONFIRM_LOCK_CHANGE)));
    }

    #[test]
    fn turning_the_lock_off_without_a_passphrase_passes() {
        assert_eq!(block_on(set_lock("Require unlock", false)), Ok(true));
    }

    #[test]
    fn a_passphrase_is_stored_as_an_argon2id_hash_and_verifies_only_itself() {
        let secrets = MemorySecrets::default();
        let lock = Passphrase::new(&secrets);
        assert_eq!(lock.is_set(), Ok(false));
        lock.set("correct horse").unwrap();
        assert_eq!(lock.is_set(), Ok(true));
        let stored = secrets.get(SERVICE, ACCOUNT).unwrap().unwrap();
        assert!(stored.starts_with("$argon2id$"), "a PHC string, never the passphrase: {stored}");
        assert!(!stored.contains("correct horse"));
        assert_eq!(lock.verify("correct horse"), Ok(true));
        assert_eq!(lock.verify("wrong"), Ok(false));
    }

    #[test]
    fn clearing_removes_the_method_and_an_unreadable_hash_is_an_error_not_a_pass() {
        let secrets = MemorySecrets::default();
        let lock = Passphrase::new(&secrets);
        assert!(lock.verify("x").is_err(), "no passphrase to check against");
        secrets.set(SERVICE, ACCOUNT, "not a hash").unwrap();
        assert!(lock.verify("x").is_err());
        lock.clear().unwrap();
        assert_eq!(lock.is_set(), Ok(false));
    }

    #[test]
    fn a_new_passphrase_needs_four_characters_and_a_matching_repeat() {
        assert_eq!(check_new("abc", "abc"), Err(TOO_SHORT));
        assert_eq!(check_new("abcd", "abce"), Err(MISMATCH));
        assert_eq!(check_new("abcd", "abcd"), Ok(()));
        assert_eq!(check_new("äöüß", "äöüß"), Ok(()), "length counts characters");
    }
}
