//! Single instance: a second launch forwards its `--open` command to the running app over a Unix socket and
//! exits. The socket lives under the state directory; a socket file left by a crashed run is replaced.

use std::io::{self, BufRead, BufReader, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};

/// Dev environment variables that force a non-unique instance (several windows can run side by side).
pub const NON_UNIQUE_ENV_VARS: [&str; 6] = [
    "KUROBAYOMI_SMOKE",
    "KUROBAYOMI_SHOT",
    "KUROBAYOMI_SHEET",
    "KUROBAYOMI_SIZE",
    "KUROBAYOMI_RAIL",
    "KUROBAYOMI_OPEN",
];

/// File name of the socket inside the state directory.
pub const SOCKET_FILE: &str = "instance.sock";

/// The socket path for a state directory.
pub fn socket_path(state_dir: &Path) -> PathBuf {
    state_dir.join(SOCKET_FILE)
}

/// True when no dev variable forces a second instance. `lookup` reads one variable (`std::env::var_os` in the app).
pub fn is_unique(lookup: impl Fn(&str) -> bool) -> bool {
    !NON_UNIQUE_ENV_VARS.iter().any(|name| lookup(name))
}

/// Result of [`acquire`].
pub enum Acquired {
    /// This process is the running instance. Commands from later launches arrive on `commands`.
    Primary(Listener),
    /// A running instance took the command. The caller exits with code 0.
    Forwarded,
}

/// The running instance's end of the socket. Dropping it removes the socket file.
pub struct Listener {
    pub commands: async_channel::Receiver<String>,
    socket: PathBuf,
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

/// Connects to a running instance and sends `command` (one line; empty means "just show the window"). If nothing
/// answers, binds the socket and becomes the primary instance.
pub fn acquire(socket: &Path, command: &str) -> io::Result<Acquired> {
    if let Ok(mut stream) = UnixStream::connect(socket) {
        writeln!(stream, "{command}")?;
        stream.flush()?;
        return Ok(Acquired::Forwarded);
    }
    match std::fs::remove_file(socket) {
        Ok(()) => {}
        Err(err) if err.kind() == io::ErrorKind::NotFound => {}
        Err(err) => return Err(err),
    }
    let listener = UnixListener::bind(socket)?;
    let (tx, rx) = async_channel::unbounded();
    std::thread::Builder::new().name("single-instance".into()).spawn(move || {
        for stream in listener.incoming().flatten() {
            for line in BufReader::new(stream).lines().map_while(Result::ok) {
                if tx.send_blocking(line).is_err() {
                    return;
                }
            }
        }
    })?;
    Ok(Acquired::Primary(Listener { commands: rx, socket: socket.to_path_buf() }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_launch_forwards_its_command_to_the_first() {
        let dir = tempfile::tempdir().unwrap();
        let socket = socket_path(dir.path());
        let first = match acquire(&socket, "").unwrap() {
            Acquired::Primary(listener) => listener,
            Acquired::Forwarded => panic!("first launch must be primary"),
        };
        assert!(matches!(acquire(&socket, "updates").unwrap(), Acquired::Forwarded));
        assert_eq!(first.commands.recv_blocking().unwrap(), "updates");
    }

    #[test]
    fn a_stale_socket_file_is_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let socket = socket_path(dir.path());
        std::fs::write(&socket, b"left behind").unwrap();
        assert!(matches!(acquire(&socket, "").unwrap(), Acquired::Primary(_)));
    }

    #[test]
    fn dropping_the_primary_removes_the_socket() {
        let dir = tempfile::tempdir().unwrap();
        let socket = socket_path(dir.path());
        let listener = match acquire(&socket, "").unwrap() {
            Acquired::Primary(l) => l,
            Acquired::Forwarded => unreachable!(),
        };
        assert!(socket.exists());
        drop(listener);
        assert!(!socket.exists());
    }

    #[test]
    fn dev_variables_disable_uniqueness() {
        assert!(is_unique(|_| false));
        assert!(!is_unique(|name| name == "KUROBAYOMI_SHOT"));
    }
}
