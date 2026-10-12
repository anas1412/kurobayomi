//! The state of one install and the events the installer emits (`InstallStep.kt`, T-18a-035).

/// The states of one install, in the order of the Kotlin ordinals (data file section 1.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallStep {
    Idle,
    Pending,
    Downloading,
    Installing,
    Installed,
    Error,
}

impl InstallStep {
    /// Kotlin `isCompleted()`: `Idle`, `Installed` and `Error` end a job. The others are still running.
    pub fn is_completed(self) -> bool {
        matches!(self, Self::Idle | Self::Installed | Self::Error)
    }
}

/// What the installer sends to its consumer. Every event names the package, which is also the state key (I19).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallEvent {
    /// The package moved to a new step. Each job ends with exactly one completed step.
    Step { pkg: String, step: InstallStep },
    /// Bytes received while `Downloading`. `total` is the `Content-Length`, when the server sent one.
    Progress { pkg: String, received: u64, total: Option<u64> },
    /// The package file is in the store. It comes just before `Step { Installed }`.
    Installed { pkg: String, version_code: i64, version_name: String },
}

#[cfg(test)]
mod tests {
    use super::InstallStep::*;
    use super::*;

    #[test]
    fn only_idle_installed_and_error_are_completed() {
        let completed: Vec<InstallStep> =
            [Idle, Pending, Downloading, Installing, Installed, Error].into_iter().filter(|s| s.is_completed()).collect();
        assert_eq!(completed, vec![Idle, Installed, Error]);
    }
}
