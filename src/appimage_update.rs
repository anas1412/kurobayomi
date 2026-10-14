//! The nightly AppImage updates itself. A nightly build knows its commit (`channel::commit`, set by the workflow); at start it
//! reads the `nightly` release, and when that was built from another commit it downloads the new AppImage next to the
//! running one, checks it and moves it into place. The next launch runs the new build. Any other build (a local one, a
//! package) has no `APPIMAGE` or no commit and never checks. Stable builds follow tagged releases and are not handled here.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use gtk::glib;
use sha2::{Digest, Sha256};

const ASSET: &str = "Kurobayomi-nightly-x86_64.AppImage";
const USER_AGENT: &str = concat!("kurobayomi/", env!("CARGO_PKG_VERSION"));

/// A nightly release built from another commit than ours, and the file to take.
#[derive(Debug, PartialEq, Eq)]
struct Newer {
    commit: String,
    url: String,
    size: u64,
    sha256: Option<String>,
}

/// The 40-character commit in the release notes ("Built from commit <hash>.").
fn commit_in(notes: &str) -> Option<String> {
    let hash: String = notes.split("commit ").nth(1)?.chars().take_while(char::is_ascii_hexdigit).collect();
    (hash.len() == 40).then_some(hash)
}

/// `Some` when the release JSON names a commit other than `current` and carries the AppImage.
fn newer(release: &[u8], current: &str) -> Option<Newer> {
    let json: serde_json::Value = serde_json::from_slice(release).ok()?;
    let commit = commit_in(json["body"].as_str()?)?;
    if commit == current {
        return None;
    }
    let asset = json["assets"].as_array()?.iter().find(|asset| asset["name"] == ASSET)?;
    Some(Newer {
        commit,
        url: asset["browser_download_url"].as_str()?.to_owned(),
        size: asset["size"].as_u64()?,
        sha256: asset["digest"].as_str().and_then(|digest| digest.strip_prefix("sha256:")).map(str::to_owned),
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

/// One check and, when there is a newer nightly, the replacement. `Some(commit)` when the file was replaced.
fn update(file: &Path, current: &str) -> Result<Option<String>, String> {
    let http = |timeout| reqwest::blocking::Client::builder().timeout(Duration::from_secs(timeout)).user_agent(USER_AGENT).build().map_err(|e| e.to_string());
    let response = http(30)?.get(format!("https://api.github.com/repos/{}/releases/tags/nightly", crate::channel::REPOSITORY)).send().map_err(|e| e.to_string())?;
    // no release while the workflow is replacing it
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    let body = response.error_for_status().and_then(|r| r.bytes()).map_err(|e| e.to_string())?;
    let Some(newer) = newer(&body, current) else { return Ok(None) };
    let part = download(&http(900)?, &newer, file)?;
    kuro_platform::self_update::replace_file(&part, file).map_err(|e| e.to_string())?;
    Ok(Some(newer.commit))
}

/// The startup check, on a worker thread. A failure is logged and otherwise silent; an update shows one toast.
pub fn startup() {
    let (Some(file), Some(current)) = (kuro_platform::self_update::appimage_file(), crate::channel::commit()) else { return };
    let (tx, rx) = async_channel::bounded(1);
    std::thread::spawn(move || {
        let _ = tx.send_blocking(update(&file, current));
    });
    glib::spawn_future_local(async move {
        match rx.recv().await {
            Ok(Ok(Some(commit))) => crate::whats_new::nightly_installed(commit),
            Ok(Err(error)) => tracing::debug!(%error, "the nightly update failed"),
            _ => {}
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "1111111111111111111111111111111111111111";
    const NEW: &str = "2222222222222222222222222222222222222222";

    fn release(commit: &str, asset: &str) -> Vec<u8> {
        serde_json::json!({
            "body": format!("Built from commit {commit}."),
            "assets": [{ "name": asset, "browser_download_url": "https://example.test/a", "size": 7, "digest": "sha256:abc" }]
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn a_release_from_another_commit_is_an_update() {
        let found = newer(&release(NEW, ASSET), OLD).expect("an update");
        assert_eq!(found, Newer { commit: NEW.into(), url: "https://example.test/a".into(), size: 7, sha256: Some("abc".into()) });
    }

    #[test]
    fn the_same_commit_a_missing_asset_or_odd_notes_are_no_update() {
        assert_eq!(newer(&release(OLD, ASSET), OLD), None);
        assert_eq!(newer(&release(NEW, "Other.AppImage"), OLD), None);
        assert_eq!(newer(br#"{"body":"nothing here","assets":[]}"#, OLD), None);
        assert_eq!(newer(b"not json", OLD), None);
    }
}
