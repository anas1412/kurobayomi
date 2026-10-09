//! The two file steps of an install: the streamed download into a `.part` file, and the atomic commit into the store
//! (T-18a-034, design 9.5, data file section 5.3).

use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

use futures::StreamExt;
use reqwest::header::ACCEPT;
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

use super::{InstallConfig, InstallError, Stop};

/// What one attempt can end in. Only a network error is worth the one retry.
enum Attempt {
    Stop(Stop),
    Network(String),
}

impl From<InstallError> for Attempt {
    fn from(err: InstallError) -> Self {
        Attempt::Stop(Stop::Failed(err))
    }
}

/// Runs `future` unless the job is cancelled first.
pub(super) async fn or_cancel<T>(token: &CancellationToken, future: impl Future<Output = T>) -> Result<T, Stop> {
    tokio::select! {
        biased;
        () = token.cancelled() => Err(Stop::Cancelled),
        value = future => Ok(value),
    }
}

/// Downloads `url` into `part` and returns the bytes. A network error is retried once after `config.retry_delay`
/// (edge case I10). `progress(received, total)` runs for every chunk.
pub(super) async fn download(
    client: &reqwest::Client,
    config: &InstallConfig,
    url: &url::Url,
    part: &Path,
    token: &CancellationToken,
    progress: &(dyn Fn(u64, Option<u64>) + Sync),
) -> Result<Vec<u8>, Stop> {
    if let Some(dir) = part.parent() {
        tokio::fs::create_dir_all(dir).await.map_err(io)?;
    }
    let mut retried = false;
    loop {
        match attempt(client, config, url, part, token, progress).await {
            Ok(()) => return tokio::fs::read(part).await.map_err(io),
            Err(Attempt::Stop(stop)) => return Err(stop),
            Err(Attempt::Network(reason)) if retried => {
                return Err(Stop::Failed(InstallError::Network(reason)));
            }
            Err(Attempt::Network(reason)) => {
                tracing::warn!(%url, "download failed, retrying: {reason}");
                retried = true;
                or_cancel(token, tokio::time::sleep(config.retry_delay)).await?;
            }
        }
    }
}

async fn attempt(
    client: &reqwest::Client,
    config: &InstallConfig,
    url: &url::Url,
    part: &Path,
    token: &CancellationToken,
    progress: &(dyn Fn(u64, Option<u64>) + Sync),
) -> Result<(), Attempt> {
    let request = client.get(url.clone()).header(ACCEPT, "application/zip, application/octet-stream");
    let response = or_cancel(token, request.send())
        .await
        .map_err(Attempt::Stop)?
        .map_err(|e| Attempt::Network(e.to_string()))?;
    let status = response.status();
    if !status.is_success() {
        return Err(InstallError::Status(status.as_u16()).into());
    }
    let total = response.content_length();
    if total.is_some_and(|n| n > config.max_bytes) {
        return Err(InstallError::TooLarge.into());
    }
    let mut file = tokio::fs::File::create(part).await.map_err(|e| InstallError::Io(e.to_string()))?;
    let mut stream = response.bytes_stream();
    let mut received = 0u64;
    while let Some(chunk) = or_cancel(token, stream.next()).await.map_err(Attempt::Stop)? {
        let chunk = chunk.map_err(|e| Attempt::Network(e.to_string()))?;
        received += chunk.len() as u64;
        if received > config.max_bytes {
            return Err(InstallError::TooLarge.into());
        }
        file.write_all(&chunk).await.map_err(|e| InstallError::Io(e.to_string()))?;
        progress(received, total);
    }
    file.flush().await.map_err(|e| InstallError::Io(e.to_string()))?;
    Ok(())
}

fn io(err: std::io::Error) -> Stop {
    Stop::Failed(InstallError::Io(err.to_string()))
}

/// Writes `bytes` to `<store>/<pkg>.kuroext` the atomic way: a `.part` file next to the target, fsync, mode `0444`,
/// rename, fsync of the directory. The old file is replaced only by the rename, so a failure leaves it as it was.
pub(super) fn commit(store_dir: &Path, pkg: &str, bytes: &[u8]) -> Result<(), InstallError> {
    fs::create_dir_all(store_dir).map_err(copy_failed)?;
    let target = store_dir.join(format!("{pkg}.kuroext"));
    let part = store_dir.join(format!("{pkg}.kuroext.part"));
    let written = write_part(&part, bytes).and_then(|()| fs::rename(&part, &target)).and_then(|()| sync_dir(store_dir));
    if let Err(err) = written {
        let _ = fs::remove_file(&part);
        return Err(copy_failed(err));
    }
    Ok(())
}

fn write_part(part: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut file = File::create(part)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    // Read-only before the rename, so the target is never visible with a writable mode.
    set_read_only(part)
}

#[cfg(unix)]
fn set_read_only(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o444))
}

#[cfg(not(unix))]
fn set_read_only(path: &Path) -> std::io::Result<()> {
    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions)
}

fn sync_dir(dir: &Path) -> std::io::Result<()> {
    File::open(dir)?.sync_all()
}

fn copy_failed(err: std::io::Error) -> InstallError {
    tracing::warn!("{}: {err}", InstallError::CopyFailed);
    InstallError::CopyFailed
}
