//! The download step (T-19a-031, `p1-rust-design.md` section 6.3). The body is streamed into a `.part` file, and a
//! cancel is checked between chunks. A failure of any kind removes the partial file.

use std::path::Path;
use std::time::{Duration, Instant};

use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;

/// A package larger than this is refused.
pub const MAX_PACKAGE_BYTES: u64 = 64 * 1024 * 1024;
/// Progress is reported at most this often.
pub const PROGRESS_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DownloadError {
    #[error("the server answered HTTP {0}")]
    Http(u16),
    #[error("the package is larger than 64 MiB")]
    TooLarge,
    #[error("the download failed: {0}")]
    Network(String),
    #[error("could not write the download: {0}")]
    Io(String),
    #[error("the download was cancelled")]
    Cancelled,
}

/// Streams `url` into `part`. `progress` gets the bytes so far, at most every 250 ms, and once more at the end. Any
/// failure, including a cancel, deletes `part`.
pub async fn download(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(u64),
) -> Result<(), DownloadError> {
    let result = stream_to(client, url, part, cancel, &mut progress).await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(part).await;
    }
    result
}

async fn stream_to(
    client: &reqwest::Client,
    url: &str,
    part: &Path,
    cancel: &CancellationToken,
    progress: &mut impl FnMut(u64),
) -> Result<(), DownloadError> {
    if let Some(parent) = part.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(io)?;
    }
    let mut response = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
        sent = client.get(url).send() => sent.map_err(|err| DownloadError::Network(err.to_string()))?,
    };
    if !response.status().is_success() {
        return Err(DownloadError::Http(response.status().as_u16()));
    }
    let mut file = tokio::fs::File::create(part).await.map_err(io)?;
    let mut total: u64 = 0;
    let mut last_report = Instant::now();
    loop {
        let next = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
            chunk = response.chunk() => chunk.map_err(|err| DownloadError::Network(err.to_string()))?,
        };
        let Some(bytes) = next else {
            break;
        };
        total += bytes.len() as u64;
        if total > MAX_PACKAGE_BYTES {
            return Err(DownloadError::TooLarge);
        }
        file.write_all(&bytes).await.map_err(io)?;
        if last_report.elapsed() >= PROGRESS_INTERVAL {
            progress(total);
            last_report = Instant::now();
        }
    }
    file.flush().await.map_err(io)?;
    file.sync_all().await.map_err(io)?;
    progress(total);
    Ok(())
}

fn io(err: std::io::Error) -> DownloadError {
    DownloadError::Io(err.to_string())
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::anime::testkit::test_client;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve(server: &MockServer, status: u16, body: Vec<u8>) {
        Mock::given(method("GET"))
            .and(path("/pkg.kuroext"))
            .respond_with(ResponseTemplate::new(status).set_body_bytes(body))
            .mount(server)
            .await;
    }

    #[tokio::test]
    async fn the_body_is_written_and_progress_reaches_its_total() {
        let server = MockServer::start().await;
        serve(&server, 200, b"hello package".to_vec()).await;
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("p-1.kuroext.part");
        let last = Cell::new(0);
        let url = format!("{}/pkg.kuroext", server.uri());
        download(&test_client(), &url, &part, &CancellationToken::new(), |bytes| last.set(bytes))
            .await
            .expect("downloads");
        assert_eq!(std::fs::read(&part).unwrap(), b"hello package");
        assert_eq!(last.get(), 13);
    }

    #[tokio::test]
    async fn a_non_success_status_fails_and_removes_the_partial_file() {
        let server = MockServer::start().await;
        serve(&server, 404, b"missing".to_vec()).await;
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("p-1.kuroext.part");
        let url = format!("{}/pkg.kuroext", server.uri());
        let result = download(&test_client(), &url, &part, &CancellationToken::new(), |_| {}).await;
        assert_eq!(result, Err(DownloadError::Http(404)));
        assert!(!part.exists());
    }

    #[tokio::test]
    async fn a_cancel_before_the_request_returns_cancelled_and_writes_nothing() {
        let server = MockServer::start().await;
        serve(&server, 200, b"x".to_vec()).await;
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("p-1.kuroext.part");
        let cancel = CancellationToken::new();
        cancel.cancel();
        let url = format!("{}/pkg.kuroext", server.uri());
        let result = download(&test_client(), &url, &part, &cancel, |_| {}).await;
        assert_eq!(result, Err(DownloadError::Cancelled));
        assert!(!part.exists());
    }

    #[tokio::test]
    async fn tst_19_075_a_cancel_while_the_body_streams_returns_cancelled_and_deletes_the_part() {
        use crate::anime::testkit::{has_nonempty_file, stalled_server, wait_until};
        let (url, _held) = stalled_server(vec![7u8; 4096], vec![0u8; 4096]).await;
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("p-1.kuroext.part");
        let cancel = CancellationToken::new();
        let client = test_client();
        let (result, ()) = tokio::join!(
            tokio::time::timeout(
                std::time::Duration::from_secs(10),
                download(&client, &url, &part, &cancel, |_| {})
            ),
            async {
                wait_until(|| has_nonempty_file(dir.path())).await;
                cancel.cancel();
            }
        );
        assert_eq!(result.expect("the cancel ends the download, no hang"), Err(DownloadError::Cancelled));
        assert!(!part.exists(), "the half-written part file is deleted");
    }

    #[tokio::test]
    async fn a_package_over_64_mib_is_too_large_and_leaves_no_part_file() {
        let server = MockServer::start().await;
        serve(&server, 200, vec![0u8; (MAX_PACKAGE_BYTES + 1) as usize]).await;
        let dir = tempfile::tempdir().unwrap();
        let part = dir.path().join("p-1.kuroext.part");
        let url = format!("{}/pkg.kuroext", server.uri());
        let result = download(&test_client(), &url, &part, &CancellationToken::new(), |_| {}).await;
        assert_eq!(result, Err(DownloadError::TooLarge));
        assert!(!part.exists());
    }

    #[tokio::test]
    async fn a_write_error_is_io_and_leaves_no_part_file() {
        let server = MockServer::start().await;
        serve(&server, 200, b"x".to_vec()).await;
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("not-a-dir");
        std::fs::write(&blocker, b"file").unwrap();
        let part = blocker.join("p-1.kuroext.part");
        let url = format!("{}/pkg.kuroext", server.uri());
        let result = download(&test_client(), &url, &part, &CancellationToken::new(), |_| {}).await;
        assert!(matches!(result, Err(DownloadError::Io(_))), "{result:?}");
        assert!(!part.exists());
    }
}
