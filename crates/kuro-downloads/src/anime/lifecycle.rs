//! Runner lifecycle and window-close rule (T-24a-030; behaviour 2.5, 2.6, 4.2; E-N06).
//!
//! The queue actor owns the runner. It publishes a [`RunnerState`] on a watch channel, and the UI reads it. `Start`
//! and `Pause` turn the runner on and off. A failure stop (for example a network warning) leaves the items in ERROR, so
//! that a later start requeues them.

/// What the runner is doing. `Paused` means the run is off while items are still queued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RunnerState {
    Stopped,
    Running,
    Paused,
}

/// The state the queue shows: running when it is on, paused when it is off with items queued, else stopped.
pub fn runner_state(runner_on: bool, queued: usize) -> RunnerState {
    if runner_on {
        RunnerState::Running
    } else if queued > 0 {
        RunnerState::Paused
    } else {
        RunnerState::Stopped
    }
}

/// The question shown when the window closes while downloads run (E-N06, proposal).
pub const CLOSE_PROMPT: &str = "Downloads will pause. Quit?";

/// The window may close at once, unless downloads are running and the user has not turned the prompt off.
pub fn close_needs_confirmation(state: RunnerState, prompt_enabled: bool) -> bool {
    state == RunnerState::Running && prompt_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_follows_the_flag_and_the_queue() {
        assert_eq!(runner_state(true, 0), RunnerState::Running);
        assert_eq!(runner_state(true, 3), RunnerState::Running);
        assert_eq!(runner_state(false, 2), RunnerState::Paused);
        assert_eq!(runner_state(false, 0), RunnerState::Stopped);
    }

    #[test]
    fn closing_while_running_asks_unless_the_prompt_is_off() {
        assert!(close_needs_confirmation(RunnerState::Running, true));
        assert!(!close_needs_confirmation(RunnerState::Running, false));
        assert!(!close_needs_confirmation(RunnerState::Paused, true));
        assert!(!close_needs_confirmation(RunnerState::Stopped, true));
        assert_eq!(CLOSE_PROMPT, "Downloads will pause. Quit?");
    }
}
