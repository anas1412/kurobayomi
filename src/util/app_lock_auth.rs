//! App lock authentication through PAM (Kotlin `AuthenticatorUtil`, T-52a-033).
//!
//! Decision (behaviour 24.2): the lock uses the user's own password through PAM, service `kurobayomi`. A fingerprint reader
//! is optional and not built. The tradeoff is stated to the user: the app reads the desktop's authentication stack, and a
//! lock weaker than the desktop lock would be a regression, so the lock stays off by default.
//!
//! The rules ([`Authenticator`]) run against a [`PamBackend`] so tests use a stub. [`Libpam`] is the real backend: it loads
//! `libpam.so.0` at run time, so the app builds and starts on a machine without PAM, and `is_authentication_supported()`
//! is then false.

use std::cell::Cell;
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;
use std::rc::Rc;

/// The PAM service name. The package installs `/etc/pam.d/kurobayomi` (`src/shell/data/kurobayomi.pam`).
pub const SERVICE: &str = "kurobayomi";

/// Why a lock check did not pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// PAM refused. The text is PAM's own (for example `Authentication failure`) and is shown as it is.
    Pam(String),
    /// The password was empty. PAM was not called.
    EmptyPassword,
    /// No PAM service file or no PAM library.
    Unsupported,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pam(text) => f.write_str(text),
            Self::EmptyPassword => f.write_str("Password is empty"),
            Self::Unsupported => f.write_str("Authentication is not available"),
        }
    }
}

/// What the rules need from PAM.
pub trait PamBackend {
    /// The service file is installed and PAM can be loaded.
    fn service_available(&self) -> bool;
    /// Blocking: asks PAM to check `password` for `user`. `Err` carries PAM's text.
    fn check(&self, user: &str, password: &str) -> Result<(), String>;
}

/// The password check of the app lock.
pub struct Authenticator<B: PamBackend> {
    backend: B,
}

impl<B: PamBackend> Authenticator<B> {
    pub fn new(backend: B) -> Self {
        Self { backend }
    }

    /// False when the PAM service file is missing. The lock setting cannot be turned on then (TC-18-a).
    pub fn is_authentication_supported(&self) -> bool {
        self.backend.service_available()
    }

    /// Checks the password. Blocking, so the caller runs it on a worker thread. An empty password is refused before PAM
    /// (TC-18-c). A wrong password returns PAM's text and changes nothing (TC-18-b).
    pub fn authenticate(&self, user: &str, password: &str) -> Result<(), AuthError> {
        if !self.is_authentication_supported() {
            return Err(AuthError::Unsupported);
        }
        if password.is_empty() {
            return Err(AuthError::EmptyPassword);
        }
        self.backend.check(user, password).map_err(AuthError::Pam)
    }
}

/// Only one lock prompt is open at a time (`isAuthenticating`, TC-18-d). GTK thread only.
#[derive(Clone, Debug, Default)]
pub struct PromptGate {
    open: Rc<Cell<bool>>,
}

/// Held by the open prompt. Dropping it lets the next prompt open.
#[derive(Debug)]
pub struct PromptGuard {
    open: Rc<Cell<bool>>,
}

impl PromptGate {
    pub fn new() -> Self {
        Self::default()
    }

    /// The guard, or `None` while a prompt is already open.
    pub fn try_open(&self) -> Option<PromptGuard> {
        if self.open.replace(true) {
            return None;
        }
        Some(PromptGuard { open: Rc::clone(&self.open) })
    }

    /// Whether a prompt is open.
    pub fn is_authenticating(&self) -> bool {
        self.open.get()
    }
}

impl Drop for PromptGuard {
    fn drop(&mut self) {
        self.open.set(false);
    }
}

/// The directories PAM reads service files from.
const SERVICE_DIRS: [&str; 2] = ["/etc/pam.d", "/usr/lib/pam.d"];

fn service_file_exists(service: &str) -> bool {
    SERVICE_DIRS.iter().any(|dir| Path::new(dir).join(service).is_file())
}

// PAM constants (security/_pam_types.h)
const PAM_SUCCESS: c_int = 0;
const PAM_BUF_ERR: c_int = 5;
const PAM_CONV_ERR: c_int = 19;
const PAM_PROMPT_ECHO_OFF: c_int = 1;
const PAM_PROMPT_ECHO_ON: c_int = 2;
const PAM_ERROR_MSG: c_int = 3;
const PAM_TEXT_INFO: c_int = 4;
const MAX_MESSAGES: c_int = 32;

#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = unsafe extern "C" fn(c_int, *mut *const PamMessage, *mut *mut PamResponse, *mut c_void) -> c_int;

#[repr(C)]
struct PamConv {
    conv: Option<ConvFn>,
    appdata_ptr: *mut c_void,
}

type StartFn = unsafe extern "C" fn(*const c_char, *const c_char, *const PamConv, *mut *mut c_void) -> c_int;
type StepFn = unsafe extern "C" fn(*mut c_void, c_int) -> c_int;
type StrErrorFn = unsafe extern "C" fn(*mut c_void, c_int) -> *const c_char;

/// Answers every password prompt with the password in `data` (a NUL-terminated byte buffer) and ignores info messages.
unsafe extern "C" fn converse(count: c_int, msgs: *mut *const PamMessage, resp: *mut *mut PamResponse, data: *mut c_void) -> c_int {
    if count <= 0 || count > MAX_MESSAGES || msgs.is_null() || resp.is_null() || data.is_null() {
        return PAM_CONV_ERR;
    }
    let count = count as usize;
    // SAFETY: calloc and strdup return memory that PAM releases with free(); `msgs` holds `count` valid message pointers
    // for the duration of this call, and `data` is the password buffer owned by `Libpam::check`, alive during pam_authenticate.
    unsafe {
        let out = libc::calloc(count, size_of::<PamResponse>()) as *mut PamResponse;
        if out.is_null() {
            return PAM_BUF_ERR;
        }
        let release = |upto: usize| {
            for i in 0..upto {
                libc::free((*out.add(i)).resp as *mut c_void);
            }
            libc::free(out as *mut c_void);
        };
        for i in 0..count {
            let message = *msgs.add(i);
            if message.is_null() {
                release(i);
                return PAM_CONV_ERR;
            }
            match (*message).msg_style {
                PAM_PROMPT_ECHO_OFF | PAM_PROMPT_ECHO_ON => {
                    let copy = libc::strdup(data as *const c_char);
                    if copy.is_null() {
                        release(i);
                        return PAM_BUF_ERR;
                    }
                    (*out.add(i)).resp = copy;
                }
                PAM_ERROR_MSG | PAM_TEXT_INFO => {}
                _ => {
                    release(i);
                    return PAM_CONV_ERR;
                }
            }
        }
        *resp = out;
    }
    PAM_SUCCESS
}

/// The PAM library loaded at run time.
pub struct Libpam {
    service: String,
    start: StartFn,
    authenticate: StepFn,
    account: StepFn,
    end: StepFn,
    strerror: StrErrorFn,
}

impl Libpam {
    /// Loads `libpam.so.0` for `service`. `None` when the library is missing.
    pub fn load(service: &str) -> Option<Self> {
        // SAFETY: dlopen and dlsym are called with NUL-terminated names. The handle is never closed, so the function
        // pointers stay valid for the life of the process, and each one is cast to the signature in pam_appl.h.
        unsafe {
            let handle = libc::dlopen(c"libpam.so.0".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL);
            if handle.is_null() {
                return None;
            }
            let symbol = |name: &CStr| {
                let address = libc::dlsym(handle, name.as_ptr());
                (!address.is_null()).then_some(address)
            };
            Some(Self {
                service: service.to_owned(),
                start: std::mem::transmute::<*mut c_void, StartFn>(symbol(c"pam_start")?),
                authenticate: std::mem::transmute::<*mut c_void, StepFn>(symbol(c"pam_authenticate")?),
                account: std::mem::transmute::<*mut c_void, StepFn>(symbol(c"pam_acct_mgmt")?),
                end: std::mem::transmute::<*mut c_void, StepFn>(symbol(c"pam_end")?),
                strerror: std::mem::transmute::<*mut c_void, StrErrorFn>(symbol(c"pam_strerror")?),
            })
        }
    }

    fn text(&self, handle: *mut c_void, code: c_int) -> String {
        // SAFETY: pam_strerror returns a static NUL-terminated string (or null) for the open handle.
        unsafe {
            let text = (self.strerror)(handle, code);
            if text.is_null() { format!("PAM error {code}") } else { CStr::from_ptr(text).to_string_lossy().into_owned() }
        }
    }
}

impl PamBackend for Libpam {
    fn service_available(&self) -> bool {
        service_file_exists(&self.service)
    }

    fn check(&self, user: &str, password: &str) -> Result<(), String> {
        let (Ok(service), Ok(user)) = (CString::new(self.service.as_str()), CString::new(user)) else {
            return Err("Invalid user name".to_owned());
        };
        if password.as_bytes().contains(&0) {
            return Err("Invalid password".to_owned());
        }
        let mut secret = password.as_bytes().to_vec();
        secret.push(0);
        let conv = PamConv { conv: Some(converse), appdata_ptr: secret.as_mut_ptr() as *mut c_void };
        let mut handle: *mut c_void = std::ptr::null_mut();
        // SAFETY: every pointer passed lives through the calls; `secret` outlives pam_end, which is the last use of the
        // conversation; pam_end runs on every path after a successful pam_start.
        let result = unsafe {
            let code = (self.start)(service.as_ptr(), user.as_ptr(), &conv, &mut handle);
            if code != PAM_SUCCESS {
                Err(self.text(handle, code))
            } else {
                let mut code = (self.authenticate)(handle, 0);
                if code == PAM_SUCCESS {
                    code = (self.account)(handle, 0);
                }
                let result = if code == PAM_SUCCESS { Ok(()) } else { Err(self.text(handle, code)) };
                (self.end)(handle, code);
                result
            }
        };
        // the password does not stay in memory; volatile writes are not optimised away
        for byte in &mut secret {
            // SAFETY: `byte` is a valid, unique reference.
            unsafe { std::ptr::write_volatile(byte, 0) };
        }
        result
    }
}

/// A backend for a machine without the PAM library: nothing is supported.
pub struct NoPam;

impl PamBackend for NoPam {
    fn service_available(&self) -> bool {
        false
    }

    fn check(&self, _user: &str, _password: &str) -> Result<(), String> {
        Err(AuthError::Unsupported.to_string())
    }
}

/// The real authenticator for the `kurobayomi` service. Without `libpam.so.0` nothing is supported.
pub fn system_authenticator() -> Authenticator<Box<dyn PamBackend + Send + Sync>> {
    let backend: Box<dyn PamBackend + Send + Sync> = match Libpam::load(SERVICE) {
        Some(lib) => Box::new(lib),
        None => Box::new(NoPam),
    };
    Authenticator::new(backend)
}

impl PamBackend for Box<dyn PamBackend + Send + Sync> {
    fn service_available(&self) -> bool {
        (**self).service_available()
    }

    fn check(&self, user: &str, password: &str) -> Result<(), String> {
        (**self).check(user, password)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct Stub {
        available: bool,
        accept: &'static str,
        calls: RefCell<u32>,
    }

    impl Stub {
        fn new(available: bool) -> Self {
            Self { available, accept: "right", calls: RefCell::new(0) }
        }
    }

    impl PamBackend for &Stub {
        fn service_available(&self) -> bool {
            self.available
        }

        fn check(&self, _user: &str, password: &str) -> Result<(), String> {
            *self.calls.borrow_mut() += 1;
            if password == self.accept { Ok(()) } else { Err("Authentication failure".to_owned()) }
        }
    }

    #[test]
    fn a_missing_service_file_means_authentication_is_not_supported() {
        // TC-18-a
        let stub = Stub::new(false);
        let auth = Authenticator::new(&stub);
        assert!(!auth.is_authentication_supported());
        assert_eq!(auth.authenticate("u", "right"), Err(AuthError::Unsupported));
        assert_eq!(*stub.calls.borrow(), 0);
    }

    #[test]
    fn a_wrong_password_returns_the_pam_text() {
        // TC-18-b
        let stub = Stub::new(true);
        let auth = Authenticator::new(&stub);
        assert_eq!(auth.authenticate("u", "wrong"), Err(AuthError::Pam("Authentication failure".to_owned())));
        assert_eq!(auth.authenticate("u", "wrong").unwrap_err().to_string(), "Authentication failure");
        assert_eq!(auth.authenticate("u", "right"), Ok(()));
    }

    #[test]
    fn an_empty_password_is_refused_before_pam() {
        // TC-18-c
        let stub = Stub::new(true);
        let auth = Authenticator::new(&stub);
        assert_eq!(auth.authenticate("u", ""), Err(AuthError::EmptyPassword));
        assert_eq!(*stub.calls.borrow(), 0);
    }

    #[test]
    fn a_second_prompt_does_not_open_while_one_is_open() {
        // TC-18-d
        let gate = PromptGate::new();
        assert!(!gate.is_authenticating());
        let first = gate.try_open().expect("the first prompt opens");
        assert!(gate.is_authenticating());
        assert!(gate.try_open().is_none());
        assert!(gate.clone().try_open().is_none(), "a clone shares the state");
        drop(first);
        assert!(!gate.is_authenticating());
        assert!(gate.try_open().is_some());
    }

    #[test]
    fn the_shipped_pam_file_includes_the_system_stack_and_not_nologin() {
        let file = include_str!("../shell/data/kurobayomi.pam");
        assert!(file.lines().any(|l| l.starts_with("auth") && l.contains("system-auth")));
        assert!(file.lines().any(|l| l.starts_with("account") && l.contains("system-auth")));
        assert!(!file.contains("pam_nologin"));
    }

    #[test]
    fn an_unknown_service_has_no_service_file() {
        assert!(!service_file_exists("kurobayomi-no-such-service"));
    }

    #[test]
    fn without_the_library_nothing_is_supported() {
        let auth = Authenticator::new(NoPam);
        assert!(!auth.is_authentication_supported());
        assert_eq!(auth.authenticate("u", "x"), Err(AuthError::Unsupported));
    }

    #[test]
    fn libpam_refuses_a_user_for_a_service_that_does_not_exist() {
        // Loads the real library when this machine has it; a machine without it passes by returning early.
        let Some(lib) = Libpam::load("kurobayomi-no-such-service") else { return };
        assert!(!lib.service_available());
        assert!(lib.check("nobody-at-all", "x").is_err());
    }
}
