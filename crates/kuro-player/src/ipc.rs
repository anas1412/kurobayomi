//! mpv JSON IPC for external sessions (area 45 `ExternalIntents.kt` progress, T-45a-061). mpv listens
//! on a Unix socket given by `--input-ipc-server`. Each query is one JSON line with `get_property`,
//! and mpv answers with one JSON line. Progress is polled every few seconds until the process exits.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Child;
use std::time::{Duration, Instant};

/// How often `time-pos` and `duration` are read (T-45a-061).
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(1);
const EXIT_CHECK: Duration = Duration::from_millis(100);

/// The request line for one property.
pub fn request_line(property: &str) -> String {
    let request = serde_json::json!({ "command": ["get_property", property] });
    format!("{request}\n")
}

/// The number in a reply, or `None` when the reply is an error or has no number.
pub fn parse_reply(line: &str) -> Option<f64> {
    let reply: serde_json::Value = serde_json::from_str(line).ok()?;
    if reply.get("error")?.as_str()? != "success" {
        return None;
    }
    reply.get("data")?.as_f64()
}

/// Reads one property over the socket. Any failure (no socket, timeout, bad reply) gives `None`.
pub fn query(socket: &Path, property: &str) -> Option<f64> {
    let mut stream = UnixStream::connect(socket).ok()?;
    stream.set_read_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok()?;
    stream.write_all(request_line(property).as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line).ok()?;
    parse_reply(&line)
}

/// The last values read before the player exited. `None` means never read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Last {
    /// Seconds.
    pub time_pos: Option<f64>,
    /// Seconds.
    pub duration: Option<f64>,
}

/// Waits for `child` to exit. While it runs, `time-pos` and `duration` are read every `interval`.
/// A failed read keeps the previous value. The player is not killed.
pub fn watch_until_exit(
    child: &mut Child,
    socket: &Path,
    interval: Duration,
) -> std::io::Result<Last> {
    let mut last = Last::default();
    let mut next_poll = Instant::now();
    loop {
        if Instant::now() >= next_poll {
            if let Some(pos) = query(socket, "time-pos") {
                last.time_pos = Some(pos);
            }
            if let Some(duration) = query(socket, "duration") {
                last.duration = Some(duration);
            }
            next_poll = Instant::now() + interval;
        }
        if child.try_wait()?.is_some() {
            return Ok(last);
        }
        std::thread::sleep(EXIT_CHECK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::process::Command;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn request_is_one_get_property_line() {
        let line = request_line("time-pos");
        assert!(line.ends_with('\n'));
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["command"][0], "get_property");
        assert_eq!(value["command"][1], "time-pos");
    }

    #[test]
    fn reply_number_needs_success() {
        assert_eq!(
            parse_reply(r#"{"data":12.5,"error":"success"}"#),
            Some(12.5)
        );
        assert_eq!(parse_reply(r#"{"error":"property unavailable"}"#), None);
        assert_eq!(parse_reply(r#"{"data":"x","error":"success"}"#), None);
        assert_eq!(parse_reply("not json"), None);
    }

    /// Answers every connection on `socket` with `time-pos` 42.5 and `duration` 100.0 until stopped.
    fn fake_mpv(socket: &Path) -> (Arc<AtomicBool>, std::thread::JoinHandle<()>) {
        let listener = UnixListener::bind(socket).unwrap();
        listener.set_nonblocking(true).unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        stream.set_nonblocking(false).unwrap();
                        let mut reader = BufReader::new(stream.try_clone().unwrap());
                        let mut request = String::new();
                        reader.read_line(&mut request).unwrap();
                        let value: serde_json::Value = serde_json::from_str(&request).unwrap();
                        let reply = match value["command"][1].as_str() {
                            Some("time-pos") => r#"{"data":42.5,"error":"success"}"#,
                            Some("duration") => r#"{"data":100.0,"error":"success"}"#,
                            _ => r#"{"error":"property unavailable"}"#,
                        };
                        let mut stream = stream;
                        writeln!(stream, "{reply}").unwrap();
                    }
                    Err(_) => std::thread::sleep(Duration::from_millis(5)),
                }
            }
        });
        (stop, handle)
    }

    #[test]
    fn query_reads_a_live_socket() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("mpv.sock");
        let (stop, handle) = fake_mpv(&socket);
        assert_eq!(query(&socket, "time-pos"), Some(42.5));
        assert_eq!(query(&socket, "duration"), Some(100.0));
        assert_eq!(query(&socket, "pause"), None);
        stop.store(true, Ordering::SeqCst);
        handle.join().unwrap();
    }

    #[test]
    fn query_without_a_socket_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(query(&dir.path().join("absent.sock"), "time-pos"), None);
    }

    #[test]
    fn watch_collects_values_until_the_process_exits() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("mpv.sock");
        let (stop, handle) = fake_mpv(&socket);
        let mut child = Command::new("sleep").arg("0.4").spawn().unwrap();
        let last = watch_until_exit(&mut child, &socket, Duration::from_millis(50)).unwrap();
        stop.store(true, Ordering::SeqCst);
        handle.join().unwrap();
        assert_eq!(last.time_pos, Some(42.5));
        assert_eq!(last.duration, Some(100.0));
    }

    #[test]
    fn watch_without_a_socket_reports_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = Command::new("true").spawn().unwrap();
        let last =
            watch_until_exit(&mut child, &dir.path().join("none.sock"), POLL_INTERVAL).unwrap();
        assert_eq!(last, Last::default());
    }
}
