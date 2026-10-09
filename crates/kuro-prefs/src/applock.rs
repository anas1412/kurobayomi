//! App lock state machine (T-01a-032): the pure rules of `SecureActivityDelegate` (`p1-behavior.md` 3.1).
//! The unlock dialog is in the GUI and the timer and window activity come from `lifecycle`; this module only
//! decides whether a lock is required.

/// `lock_app_after` value that never locks after a stop.
pub const NEVER: i32 = -1;
/// `lock_app_after` value that locks on every start.
pub const ALWAYS: i32 = 0;

/// The preference inputs of the state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppLockConfig {
    /// `use_biometric_lock`. False means the lock is off.
    pub use_authenticator: bool,
    /// `lock_app_after`: [`NEVER`], [`ALWAYS`], or a number of minutes.
    pub lock_app_after: i32,
}

/// The runtime state of the lock. `last_app_closed` is `None` when `__APP_STATE_last_app_closed` is absent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppLockState {
    pub require_unlock: bool,
    pub last_app_closed: Option<i64>,
}

impl AppLockState {
    /// The state at process start: unlock is required ("always require unlock if app is killed").
    pub fn at_process_start() -> Self {
        Self { require_unlock: true, last_app_closed: None }
    }

    /// The unlock dialog succeeded.
    pub fn unlock(self) -> Self {
        Self { require_unlock: false, ..self }
    }
}

/// `onApplicationStopped`. `now` is epoch milliseconds.
pub fn on_application_stopped(cfg: AppLockConfig, authenticating: bool, now: i64, state: AppLockState) -> AppLockState {
    if !cfg.use_authenticator || authenticating || state.require_unlock {
        return state;
    }
    if cfg.lock_app_after > 0 {
        return AppLockState { last_app_closed: Some(now), ..state };
    }
    state
}

/// `require_unlock(lock_after, last_closed, now)`: whether the app must be unlocked when it starts. `NEVER` (-1)
/// never locks, `ALWAYS` (0) always locks, and a number of minutes locks once `last_closed + minutes * 60_000 <= now`.
/// Deviation from that formula: a clock that moved back before `last_closed` also locks (the safe side).
pub fn require_unlock(lock_after: i32, last_closed: i64, now: i64) -> bool {
    match lock_after {
        NEVER => false,
        ALWAYS => true,
        minutes => {
            let window = i64::from(minutes) * 60_000;
            now < last_closed || last_closed.saturating_add(window) <= now
        }
    }
}

/// `onApplicationStart`. `now` is epoch milliseconds. Returns the new state, and the `use_authenticator`
/// value that the caller must store (the lock turns itself off when no unlock method exists; see
/// [`disable_when_unsupported`]).
pub fn on_application_start(cfg: AppLockConfig, authenticating: bool, now: i64, state: AppLockState) -> AppLockState {
    if !cfg.use_authenticator {
        return state;
    }
    let mut next = state;
    if !authenticating && !state.require_unlock {
        next.require_unlock = require_unlock(cfg.lock_app_after, state.last_app_closed.unwrap_or(0), now);
    }
    next.last_app_closed = None;
    next
}

/// When the lock is enabled but no unlock method is configured, the lock switches itself off silently.
/// Returns the new value of `use_authenticator`.
pub fn disable_when_unsupported(use_authenticator: bool, unlock_supported: bool) -> bool {
    use_authenticator && unlock_supported
}

#[cfg(test)]
mod tests {
    use super::*;

    const T: i64 = 1_000_000_000;
    const ON: AppLockConfig = AppLockConfig { use_authenticator: true, lock_app_after: 5 };

    fn st(require_unlock: bool, last: Option<i64>) -> AppLockState {
        AppLockState { require_unlock, last_app_closed: last }
    }

    /// The 13 rows of `p1-edge-cases-and-tests.md` 3.9, in order.
    #[test]
    fn transition_table() {
        let off = AppLockConfig { use_authenticator: false, lock_app_after: 5 };
        let never = AppLockConfig { use_authenticator: true, lock_app_after: NEVER };
        let always = AppLockConfig { use_authenticator: true, lock_app_after: ALWAYS };

        // stop rows
        assert_eq!(on_application_stopped(off, false, T, st(false, None)), st(false, None), "stop, use_auth=false");
        assert_eq!(on_application_stopped(ON, true, T, st(false, None)), st(false, None), "stop, authenticating");
        assert_eq!(on_application_stopped(ON, false, T, st(true, None)), st(true, None), "stop, require_unlock");
        assert_eq!(on_application_stopped(always, false, T, st(false, None)), st(false, None), "stop, lock_after=0");
        assert_eq!(on_application_stopped(never, false, T, st(false, None)), st(false, None), "stop, lock_after=-1");
        assert_eq!(on_application_stopped(ON, false, T, st(false, None)), st(false, Some(T)), "stop, lock_after=5 stores T");

        // start rows
        assert_eq!(on_application_start(never, false, T, st(false, Some(T))), st(false, None), "start, lock_after=-1");
        assert_eq!(on_application_start(always, false, T, st(false, Some(T))), st(true, None), "start, lock_after=0");
        assert_eq!(on_application_start(ON, false, T + 299_999, st(false, Some(T))), st(false, None), "start, 5 min minus 1 ms");
        assert_eq!(on_application_start(ON, false, T + 300_000, st(false, Some(T))), st(true, None), "start, exactly 5 min");
        assert_eq!(on_application_start(ON, false, T - 1, st(false, Some(T))), st(true, None), "start, clock moved back");
        assert_eq!(on_application_start(ON, false, T, st(true, Some(T))), st(true, None), "start, already locked");
        assert_eq!(on_application_start(ON, true, T, st(false, Some(T))), st(false, None), "start, authenticating");
    }

    #[test]
    fn start_without_a_stored_time_locks_after_any_positive_setting() {
        assert!(on_application_start(ON, false, T, st(false, None)).require_unlock);
    }

    #[test]
    fn minutes_use_64_bit_arithmetic() {
        let big = AppLockConfig { use_authenticator: true, lock_app_after: i32::MAX };
        let window = i64::from(i32::MAX) * 60_000;
        let last = 1_000;
        assert!(!on_application_start(big, false, last + window - 1, st(false, Some(last))).require_unlock);
        assert!(on_application_start(big, false, last + window, st(false, Some(last))).require_unlock);
        assert!(on_application_start(big, false, i64::MAX, st(false, Some(i64::MAX))).require_unlock, "saturates, no overflow");
    }

    /// The table of `T-02a-022`: -1, 0, 1, 2, 5, 10 minutes, and a clock that moved back.
    #[test]
    fn require_unlock_table() {
        let last = T;
        let minute = 60_000;
        assert!(!require_unlock(-1, last, last + 10 * minute), "-1 never locks");
        assert!(require_unlock(0, last, last), "0 always locks");
        for (minutes, after_ms) in [(1, minute), (2, 2 * minute), (5, 5 * minute), (10, 10 * minute)] {
            assert!(!require_unlock(minutes, last, last + after_ms - 1), "{minutes} min, one ms early");
            assert!(require_unlock(minutes, last, last + after_ms), "{minutes} min, exactly due");
        }
        assert!(require_unlock(5, last, last - 1), "clock moved back locks");
        assert!(require_unlock(i32::MAX, i64::MAX - 1, i64::MAX), "saturating add, no overflow");
    }

    #[test]
    fn unlock_clears_the_requirement_and_start_begins_locked() {
        assert!(AppLockState::at_process_start().require_unlock);
        assert!(!AppLockState::at_process_start().unlock().require_unlock);
    }

    #[test]
    fn unsupported_unlock_switches_the_lock_off() {
        assert!(!disable_when_unsupported(true, false));
        assert!(disable_when_unsupported(true, true));
        assert!(!disable_when_unsupported(false, true));
    }
}
