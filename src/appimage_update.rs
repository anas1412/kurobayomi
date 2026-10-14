//! The AppImage updates itself, on both channels. At start it reads the release its channel follows, and when that is
//! newer it downloads the new AppImage next to the running one, checks it and moves it into place. The next launch runs
//! the new build.
//! - A nightly build knows its commit (`channel::commit`, set by the workflow) and follows the `nightly` release: newer is
//!   a build from another commit.
//! - A stable build with the release check (`app_update::available`) follows the latest release: newer is a higher
//!   version.
//!
//! Any other build (a local one, a package) has no `APPIMAGE`, no commit or no release check, and never updates itself.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gtk::glib;
use sha2::{Digest, Sha256};

const USER_AGENT: &str = concat!("kurobayomi/", env!("CARGO_PKG_VERSION"));

/// The releases a build follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Track {
    /// The `nightly` release; the commit of the running build.
    Nightly(&'static str),
    /// The latest release; the version of the running build.
    Stable(&'static str),
}

impl Track {
    /// The track of this build, when it can update itself.
    pub fn current() -> Option<Self> {
        match crate::channel::commit() {
            Some(commit) => Some(Self::Nightly(commit)),
            None if !crate::channel::NIGHTLY && crate::app_update::available() => Some(Self::Stable(env!("CARGO_PKG_VERSION"))),
            None => None,
        }
    }

    fn release_path(self) -> &'static str {
        match self {
            Self::Nightly(_) => "releases/tags/nightly",
            Self::Stable(_) => "releases/latest",
        }
    }

    fn asset(self) -> &'static str {
        match self {
            Self::Nightly(_) => "Kurobayomi-nightly-x86_64.AppImage",
            Self::Stable(_) => "Kurobayomi-x86_64.AppImage",
        }
    }
}

/// A newer release, and the file to take.
#[derive(Debug, PartialEq, Eq)]
struct Newer {
    /// The commit of a nightly, the version of a stable release (its tag without the `v`).
    build: String,
    url: String,
    size: u64,
    sha256: Option<String>,
    /// The release's `SHA256SUMS`, read when GitHub gives no digest for the file.
    sums_url: Option<String>,
}

/// The 40-character commit in the release notes ("Built from commit <hash>.").
fn commit_in(notes: &str) -> Option<String> {
    let hash: String = notes.split("commit ").nth(1)?.chars().take_while(char::is_ascii_hexdigit).collect();
    (hash.len() == 40).then_some(hash)
}

/// `Some` when the release JSON is newer than the running build and carries the track's AppImage.
fn newer(release: &[u8], track: Track) -> Option<Newer> {
    let json: serde_json::Value = serde_json::from_slice(release).ok()?;
    let build = match track {
        Track::Nightly(current) => Some(commit_in(json["body"].as_str()?)?).filter(|commit| commit != current)?,
        Track::Stable(current) => {
            let tag = json["tag_name"].as_str()?;
            kuro_domain::release::version::is_newer(tag, current).then(|| tag.trim_start_matches('v').to_owned())?
        }
    };
    let asset = json["assets"].as_array()?.iter().find(|asset| asset["name"] == track.asset())?;
    Some(Newer {
        build,
        url: asset["browser_download_url"].as_str()?.to_owned(),
        size: asset["size"].as_u64()?,
        sha256: asset["digest"].as_str().and_then(|digest| digest.strip_prefix("sha256:")).map(str::to_owned),
        sums_url: json["assets"]
            .as_array()?
            .iter()
            .find(|asset| asset["name"] == "SHA256SUMS")
            .and_then(|asset| asset["browser_download_url"].as_str())
            .map(str::to_owned),
    })
}

/// The hash `SHA256SUMS` lists for `name` (`<hash>  <name>` lines).
fn sum_of(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(char::is_whitespace)?;
        (file.trim_start().trim_start_matches('*') == name).then(|| hash.to_owned())
    })
}

/// Downloads the update beside `target` and checks its size, its hash (when GitHub gives one) and that it is an ELF file.
fn download(client: &reqwest::blocking::Client, update: &Newer, target: &Path) -> Result<PathBuf, String> {
    let mut name = target.as_os_str().to_owned();
    name.push(".update");
    let part = PathBuf::from(name);
    let result = fetch_checked(client, update, &part);
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result.map(|()| part)
}

fn fetch_checked(client: &reqwest::blocking::Client, update: &Newer, part: &Path) -> Result<(), String> {
    let mut response = client.get(&update.url).send().and_then(|r| r.error_for_status()).map_err(|e| e.to_string())?;
    let mut file = std::fs::File::create(part).map_err(|e| e.to_string())?;
    let (mut hasher, mut total, mut buf) = (Sha256::new(), 0_u64, [0_u8; 64 * 1024]);
    loop {
        let n = response.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        total += n as u64;
    }
    file.flush().map_err(|e| e.to_string())?;
    let hash: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if total != update.size {
        return Err(format!("downloaded {total} bytes, expected {}", update.size));
    }
    if update.sha256.as_deref().is_some_and(|expected| !expected.eq_ignore_ascii_case(&hash)) {
        return Err("the downloaded file does not match its checksum".to_owned());
    }
    let mut magic = [0_u8; 4];
    std::fs::File::open(part).and_then(|mut file| file.read_exact(&mut magic)).map_err(|e| e.to_string())?;
    if magic != *b"\x7fELF" {
        return Err("the downloaded file is not an AppImage".to_owned());
    }
    Ok(())
}

const GITHUB_API: &str = "https://api.github.com";

/// One check against the releases at `api` and, when there is a newer release, the replacement. `Some(build)` when the
/// file was replaced.
fn update(file: &Path, track: Track, api: &str) -> Result<Option<String>, String> {
    let http = |timeout| reqwest::blocking::Client::builder().timeout(Duration::from_secs(timeout)).user_agent(USER_AGENT).build().map_err(|e| e.to_string());
    let url = format!("{api}/repos/{}/{}", crate::channel::REPOSITORY, track.release_path());
    let response = http(30)?.get(url).send().map_err(|e| e.to_string())?;
    // no release while the workflow is replacing the nightly, or before the first stable one
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let body = response.error_for_status().and_then(|r| r.bytes()).map_err(|e| e.to_string())?;
    let Some(mut newer) = newer(&body, track) else { return Ok(None) };
    if newer.sha256.is_none()
        && let Some(url) = &newer.sums_url
    {
        let sums = http(30)?.get(url).send().and_then(|r| r.error_for_status()).and_then(|r| r.text()).map_err(|e| e.to_string())?;
        newer.sha256 = Some(sum_of(&sums, track.asset()).ok_or("SHA256SUMS does not list the AppImage")?);
    }
    let part = download(&http(900)?, &newer, file)?;
    kuro_platform::self_update::replace_file(&part, file).map_err(|e| e.to_string())?;
    Ok(Some(newer.build))
}

/// Whether this run can update itself: an AppImage of a build that follows releases.
pub fn can_update() -> bool {
    kuro_platform::self_update::appimage_file().is_some() && Track::current().is_some()
}

/// Checks and updates on a worker thread, then hands the result to `done` on the main loop: `Ok(Some(build))` when the
/// AppImage was replaced. Does nothing when this run cannot update itself.
pub fn run(done: impl FnOnce(Result<Option<String>, String>) + 'static) {
    let (Some(file), Some(track)) = (kuro_platform::self_update::appimage_file(), Track::current()) else { return };
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(update(&file, track, GITHUB_API));
    });
    glib::spawn_future_local(async move {
        if let Ok(result) = rx.recv().await {
            done(result);
        }
    });
}

/// The installed toast of a replaced AppImage.
pub fn announce(build: String) {
    match Track::current() {
        Some(Track::Nightly(_)) => crate::whats_new::nightly_installed(build),
        _ => crate::whats_new::stable_installed(build),
    }
}

/// The startup check. A failure is logged and otherwise silent; an update shows one toast.
pub fn startup() {
    run(|result| match result {
        Ok(Some(build)) => announce(build),
        Ok(None) => {}
        Err(error) => tracing::debug!(%error, "the AppImage update failed"),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";

    const NIGHTLY: &str = "Kurobayomi-nightly-x86_64.AppImage";
    const STABLE: &str = "Kurobayomi-x86_64.AppImage";

    fn release(commit: &str, asset: &str) -> Vec<u8> {
        serde_json::json!({
            "tag_name": "nightly",
            "body": format!("Built from commit {commit}."),
            "assets": [{ "name": asset, "browser_download_url": "https://example.test/a", "size": 7, "digest": "sha256:abc" }]
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn a_release_from_another_commit_is_an_update() {
        let found = newer(&release(NEW, NIGHTLY), Track::Nightly(OLD)).expect("an update");
        assert_eq!(found, Newer { build: NEW.into(), url: "https://example.test/a".into(), size: 7, sha256: Some("abc".into()), sums_url: None });
    }

    #[test]
    fn the_same_commit_a_missing_asset_or_odd_notes_are_no_update() {
        assert_eq!(newer(&release(OLD, NIGHTLY), Track::Nightly(OLD)), None);
        assert_eq!(newer(&release(NEW, "Other.AppImage"), Track::Nightly(OLD)), None);
        assert_eq!(newer(&release(NEW, STABLE), Track::Nightly(OLD)), None, "a nightly takes only the nightly file");
        assert_eq!(newer(br#"{"body":"nothing here","assets":[]}"#, Track::Nightly(OLD)), None);
        assert_eq!(newer(b"not json", Track::Nightly(OLD)), None);
    }

    fn stable(tag: &str) -> Vec<u8> {
        let asset = |name: &str| serde_json::json!({ "name": name, "browser_download_url": format!("https://example.test/{name}"), "size": 9 });
        serde_json::json!({
            "tag_name": tag,
            "body": "## Added",
            "assets": [asset("Kurobayomi-x86_64.AppImage.zsync"), asset(STABLE), asset("SHA256SUMS")]
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn a_higher_stable_version_is_an_update_and_takes_the_appimage_not_its_zsync() {
        let found = newer(&stable("v0.2.0"), Track::Stable("0.1.0")).expect("an update");
        assert_eq!(found.build, "0.2.0");
        assert_eq!(found.url, format!("https://example.test/{STABLE}"));
        assert_eq!((found.sha256, found.sums_url.as_deref()), (None, Some("https://example.test/SHA256SUMS")), "no digest: the sums file is read");
    }

    #[test]
    fn the_sums_file_names_the_hash_of_one_file() {
        let sums = "aaa  Kurobayomi-x86_64.AppImage.zsync\nbbb  Kurobayomi-x86_64.AppImage\nccc *Other\n";
        assert_eq!(sum_of(sums, STABLE).as_deref(), Some("bbb"));
        assert_eq!(sum_of(sums, "Other").as_deref(), Some("ccc"));
        assert_eq!(sum_of(sums, "Missing"), None);
    }

    #[test]
    fn the_same_or_an_older_stable_version_is_no_update() {
        assert_eq!(newer(&stable("v0.1.0"), Track::Stable("0.1.0")), None);
        assert_eq!(newer(&stable("v0.0.9"), Track::Stable("0.1.0")), None);
        assert_eq!(newer(&stable("nightly"), Track::Stable("0.1.0")), None, "a tag that is no version is never newer");
    }

    #[test]
    fn a_track_reads_its_own_release_and_file() {
        assert_eq!((Track::Nightly(OLD).release_path(), Track::Nightly(OLD).asset()), ("releases/tags/nightly", NIGHTLY));
        assert_eq!((Track::Stable("0.1.0").release_path(), Track::Stable("0.1.0").asset()), ("releases/latest", STABLE));
    }

    /// Serves the files `files` builds from the base url (path, body) over HTTP on a local port, one request per
    /// connection, and gives the base url.
    fn serve(files: impl FnOnce(&str) -> Vec<(String, Vec<u8>)>) -> String {
        use std::io::{BufRead, BufReader};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let files = files(&base);
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut line = String::new();
                BufReader::new(&stream).read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or_default();
                let (status, body) = files.iter().find(|(p, _)| p == path).map_or(("404 Not Found", &[][..]), |(_, b)| ("200 OK", &b[..]));
                let _ = write!(stream, "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
                let _ = stream.write_all(body);
            }
        });
        base
    }

    /// A latest release `tag` with the AppImage `app` and a SHA256SUMS that gives `app_hash` for it.
    fn stable_release(tag: &'static str, app: Vec<u8>, app_hash: String) -> impl FnOnce(&str) -> Vec<(String, Vec<u8>)> {
        move |base| {
            let sums = format!("{app_hash}  {STABLE}\n");
            let release = serde_json::json!({
                "tag_name": tag,
                "assets": [
                    { "name": STABLE, "browser_download_url": format!("{base}/dl/app"), "size": app.len() },
                    { "name": "SHA256SUMS", "browser_download_url": format!("{base}/dl/sums"), "size": sums.len() }
                ]
            });
            vec![
                (format!("/repos/{}/releases/latest", crate::channel::REPOSITORY), release.to_string().into_bytes()),
                ("/dl/app".to_owned(), app),
                ("/dl/sums".to_owned(), sums.into_bytes()),
            ]
        }
    }

    fn hex(bytes: &[u8]) -> String {
        Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
    }

    /// The whole stable update over HTTP: the release, its SHA256SUMS, the download, the checks and the swap.
    #[test]
    fn a_stable_appimage_replaces_itself_with_a_newer_release_and_refuses_a_bad_one() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join(STABLE);
        std::fs::write(&file, b"\x7fELF old").unwrap();
        let new = b"\x7fELF new build".to_vec();
        let track = Track::Stable("0.1.0");

        assert_eq!(update(&file, track, &serve(|_| Vec::new())), Ok(None), "no stable release yet");
        assert_eq!(update(&file, track, &serve(stable_release("v0.1.0", new.clone(), hex(&new)))), Ok(None), "the same version");

        let error = update(&file, track, &serve(stable_release("v0.2.0", new.clone(), hex(b"other")))).unwrap_err();
        assert!(error.contains("checksum"), "{error}");
        assert_eq!(std::fs::read(&file).unwrap(), b"\x7fELF old", "a file that fails its sum leaves the AppImage alone");
        assert!(!dir.path().join(format!("{STABLE}.update")).exists(), "and is removed");

        let bad_file = b"not an elf".to_vec();
        let error = update(&file, track, &serve(stable_release("v0.2.0", bad_file.clone(), hex(&bad_file)))).unwrap_err();
        assert!(error.contains("not an AppImage"), "{error}");

        assert_eq!(update(&file, track, &serve(stable_release("v0.2.0", new.clone(), hex(&new)))), Ok(Some("0.2.0".to_owned())));
        assert_eq!(std::fs::read(&file).unwrap(), new);
    }
}
