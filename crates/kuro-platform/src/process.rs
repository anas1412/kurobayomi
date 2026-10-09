//! Child processes that outlive the caller.

use std::ffi::OsStr;
use std::io;
use std::path::Path;
use std::process::{Command, Stdio};

/// Starts `program` with `args` in its own process group, with stdio closed. The child is not waited for.
pub fn spawn_detached(program: &Path, args: &[&OsStr]) -> io::Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    cmd.spawn().map(drop)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn spawns_and_returns_without_waiting() {
        spawn_detached(Path::new("true"), &[]).unwrap();
    }

    #[test]
    fn missing_program_is_an_error() {
        assert!(spawn_detached(Path::new("/nonexistent/kuro-missing"), &[]).is_err());
    }
}
