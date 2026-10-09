//! Launches an external player on a stream URL with the headers the stream needs (T-req-platform-007).

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::PathBuf;

use crate::process::spawn_detached;

/// Starts a player on a stream and does not wait for it.
pub trait ExternalPlayer: Send + Sync {
    /// `headers` are request headers the stream needs, as `(name, value)` pairs.
    fn launch(&self, url: &str, headers: &[(String, String)]) -> io::Result<()>;
}

/// Runs a player binary that takes mpv's command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandPlayer {
    program: PathBuf,
}

impl CommandPlayer {
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self { program: program.into() }
    }
}

impl ExternalPlayer for CommandPlayer {
    fn launch(&self, url: &str, headers: &[(String, String)]) -> io::Result<()> {
        let args = mpv_args(url, headers)?;
        let refs: Vec<&OsStr> = args.iter().map(OsString::as_os_str).collect();
        spawn_detached(&self.program, &refs)
    }
}

/// The mpv command line for a stream: the URL, then `--http-header-fields` when there are headers.
///
/// A URL that starts with `-` is refused, so it cannot be read as an option. Header values are joined with commas,
/// which is mpv's separator, so a value that itself contains a comma is not supported. Players other than mpv need their
/// own argument builder.
pub fn mpv_args(url: &str, headers: &[(String, String)]) -> io::Result<Vec<OsString>> {
    if url.starts_with('-') {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "stream URL starts with '-'"));
    }
    let mut args = vec![OsString::from(url)];
    if !headers.is_empty() {
        let fields: Vec<String> = headers.iter().map(|(name, value)| format!("{name}: {value}")).collect();
        args.push(OsString::from(format!("--http-header-fields={}", fields.join(","))));
    }
    Ok(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(name: &str, value: &str) -> (String, String) {
        (name.to_owned(), value.to_owned())
    }

    #[test]
    fn a_stream_without_headers_is_just_the_url() {
        assert_eq!(mpv_args("https://cdn.test/a.m3u8", &[]).unwrap(), vec![OsString::from("https://cdn.test/a.m3u8")]);
    }

    #[test]
    fn headers_become_one_http_header_fields_option() {
        let headers = [pair("Referer", "https://site.test/"), pair("User-Agent", "Mozilla/5.0")];
        assert_eq!(
            mpv_args("https://cdn.test/a.m3u8", &headers).unwrap(),
            vec![
                OsString::from("https://cdn.test/a.m3u8"),
                OsString::from("--http-header-fields=Referer: https://site.test/,User-Agent: Mozilla/5.0"),
            ]
        );
    }

    #[test]
    fn a_url_that_looks_like_an_option_is_refused() {
        let err = mpv_args("--script=/tmp/x.lua", &[]).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn a_missing_player_binary_is_an_error() {
        let player = CommandPlayer::new("/nonexistent/kuro-player");
        assert!(player.launch("https://cdn.test/a.mp4", &[]).is_err());
    }
}
