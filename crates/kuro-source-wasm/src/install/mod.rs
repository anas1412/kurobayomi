//! The installer: download, verify and commit one `.kuroext` package per job (T-18a-034 to T-18a-037, design 9.5).
//!
//! One job per package name. A request for a package that already has a job joins it (I19, N5). Downloads share
//! [`DOWNLOAD_PERMITS`] permits, commits are serial behind one lock, and a cancel is accepted until the commit starts.
//! Steps and progress go out on an `async_channel`, which the GTK side drains with `spawn_future_local`.

mod fetch;
mod rules;
mod step;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::runtime::Handle;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use url::Url;

use self::fetch::{commit, download, or_cancel};
pub use self::rules::{InstalledInfo, check_install, read_installed};
pub use self::step::{InstallEvent, InstallStep};
use crate::manifest::{Manifest, ManifestError};
use crate::package::{read_package, with_signer};
use crate::verify::{Verified, VerifyError, verify_package};

/// Concurrent downloads (design 4). Kotlin has no cap.
pub const DOWNLOAD_PERMITS: usize = 3;
/// Largest download body, 64 MiB (data file section 6.5).
pub const MAX_DOWNLOAD_BYTES: u64 = 64 << 20;

/// Why an install ended in `Error`. The `Downgrade`, `SignerMismatch` and `CopyFailed` texts are the Kotlin log texts
/// (data file section 13).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InstallError {
    #[error("download failed: {0}")]
    Network(String),
    #[error("download failed: HTTP {0}")]
    Status(u16),
    #[error("download is larger than 64 MiB")]
    TooLarge,
    #[error("{0}")]
    Verify(#[from] VerifyError),
    #[error("{0}")]
    Manifest(#[from] ManifestError),
    #[error("Installed extension version is higher. Downgrading is not allowed.")]
    Downgrade,
    #[error("Installed extension signature is not matched.")]
    SignerMismatch,
    #[error("Failed to copy extension file.")]
    CopyFailed,
    #[error("io: {0}")]
    Io(String),
    #[error("invalid install request: {0}")]
    InvalidRequest(String),
}

/// How a job ended when it did not succeed.
pub(crate) enum Stop {
    Cancelled,
    Failed(InstallError),
}

impl From<InstallError> for Stop {
    fn from(err: InstallError) -> Self {
        Stop::Failed(err)
    }
}

/// Where the installer reads and writes, and its limits. [`InstallConfig::new`] gives the production values.
#[derive(Debug, Clone)]
pub struct InstallConfig {
    /// `$XDG_CACHE_HOME/kurobayomi/extensions/download`
    pub download_dir: PathBuf,
    /// `$XDG_DATA_HOME/kurobayomi/extensions`
    pub store_dir: PathBuf,
    pub connect_timeout: Duration,
    pub total_timeout: Duration,
    pub retry_delay: Duration,
    pub max_bytes: u64,
    pub permits: usize,
    pub user_agent: String,
}

impl InstallConfig {
    pub fn new(download_dir: PathBuf, store_dir: PathBuf) -> Self {
        InstallConfig {
            download_dir,
            store_dir,
            connect_timeout: Duration::from_secs(10),
            total_timeout: Duration::from_secs(120),
            retry_delay: Duration::from_secs(2),
            max_bytes: MAX_DOWNLOAD_BYTES,
            permits: DOWNLOAD_PERMITS,
            user_agent: concat!("Kurobayomi/", env!("CARGO_PKG_VERSION")).to_owned(),
        }
    }
}

/// One package to install, as the repo index lists it (data file sections 7.4 and 7.5).
#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub pkg: String,
    pub version_name: String,
    pub url: Url,
    pub sha256: [u8; 32],
    pub ed25519: Vec<u8>,
    pub ed25519_pub: [u8; 32],
    /// The repo's `signingKeyFingerprint`, lowercase hex.
    pub repo_fingerprint: String,
}

impl InstallRequest {
    /// `pkg` and `version_name` become part of a file name, so only plain characters pass.
    fn validate(&self) -> Result<(), InstallError> {
        let plain = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
        if !plain(&self.pkg) || !plain(&self.version_name) || self.pkg.starts_with('.') {
            return Err(InstallError::InvalidRequest(format!("{} {}", self.pkg, self.version_name)));
        }
        Ok(())
    }
}

/// What a request did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Enqueued {
    Started,
    /// The package already had a job; this request is that job.
    Joined,
    /// An update was asked for a package with no available entry. Nothing was emitted (I20).
    NoEntry,
}

struct Job {
    token: CancellationToken,
    step: InstallStep,
    /// Set under the `jobs` lock when the commit starts. From then on a cancel is ignored (I18).
    committing: bool,
}

pub struct Installer {
    config: InstallConfig,
    client: reqwest::Client,
    handle: Handle,
    events: async_channel::Sender<InstallEvent>,
    downloads: Arc<Semaphore>,
    commit_lock: tokio::sync::Mutex<()>,
    /// Active jobs by package name. A job leaves the map when it emits its completed step.
    jobs: Mutex<HashMap<String, Job>>,
}

impl Installer {
    /// `handle` is the tokio runtime the jobs run on, so the caller may be any thread.
    pub fn new(
        config: InstallConfig,
        handle: Handle,
        events: async_channel::Sender<InstallEvent>,
    ) -> Result<Arc<Installer>, InstallError> {
        let client = reqwest::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.total_timeout)
            .user_agent(config.user_agent.clone())
            .build()
            .map_err(|e| InstallError::Network(e.to_string()))?;
        Ok(Arc::new(Installer {
            downloads: Arc::new(Semaphore::new(config.permits)),
            config,
            client,
            handle,
            events,
            commit_lock: tokio::sync::Mutex::new(()),
            jobs: Mutex::new(HashMap::new()),
        }))
    }

    /// The step of a package, `Idle` when it has no job.
    pub fn step(&self, pkg: &str) -> InstallStep {
        self.jobs().get(pkg).map_or(InstallStep::Idle, |job| job.step)
    }

    /// Queues an install. A package that already has a job is joined, not downloaded twice.
    pub fn install(self: &Arc<Self>, request: InstallRequest) -> Result<Enqueued, InstallError> {
        request.validate()?;
        let token = CancellationToken::new();
        {
            let mut jobs = self.jobs();
            if jobs.contains_key(&request.pkg) {
                return Ok(Enqueued::Joined);
            }
            jobs.insert(
                request.pkg.clone(),
                Job { token: token.clone(), step: InstallStep::Pending, committing: false },
            );
            let _ = self.events.try_send(InstallEvent::Step { pkg: request.pkg.clone(), step: InstallStep::Pending });
        }
        let installer = Arc::clone(self);
        self.handle.spawn(async move { installer.job(request, token).await });
        Ok(Enqueued::Started)
    }

    /// An update is an install of the available entry. Without one nothing is emitted (I20).
    pub fn update(self: &Arc<Self>, pkg: &str, available: Option<InstallRequest>) -> Result<Enqueued, InstallError> {
        match available {
            Some(request) => self.install(request),
            None => {
                tracing::warn!(pkg, "update requested for a package with no available entry");
                Ok(Enqueued::NoEntry)
            }
        }
    }

    /// Cancels a queued or downloading job; the job then ends in `Idle`. Once the commit has started the cancel is
    /// ignored and the job ends in `Installed`. Returns whether the job was signalled.
    pub fn cancel(&self, pkg: &str) -> bool {
        let jobs = self.jobs();
        match jobs.get(pkg) {
            Some(job) if !job.committing => {
                job.token.cancel();
                true
            }
            _ => false,
        }
    }

    fn jobs(&self) -> MutexGuard<'_, HashMap<String, Job>> {
        self.jobs.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Records the step and sends it, under the lock, so a joiner never sees a finished job and events keep order.
    fn emit_step(&self, pkg: &str, step: InstallStep) {
        let mut jobs = self.jobs();
        if step.is_completed() {
            jobs.remove(pkg);
        } else if let Some(job) = jobs.get_mut(pkg) {
            job.step = step;
        }
        let _ = self.events.try_send(InstallEvent::Step { pkg: pkg.to_owned(), step });
    }

    /// Marks the commit as started unless the job was cancelled first. Both happen under the lock `cancel` takes.
    fn begin_commit(&self, pkg: &str) -> Result<(), Stop> {
        let mut jobs = self.jobs();
        let job = jobs.get_mut(pkg).ok_or(Stop::Cancelled)?;
        if job.token.is_cancelled() {
            return Err(Stop::Cancelled);
        }
        job.committing = true;
        Ok(())
    }

    async fn job(self: Arc<Self>, request: InstallRequest, token: CancellationToken) {
        let part = self.config.download_dir.join(format!("{}-{}.kuroext.part", request.pkg, request.version_name));
        let result = self.run(&request, &token, &part).await;
        let _ = tokio::fs::remove_file(&part).await;
        match result {
            Ok(()) => self.emit_step(&request.pkg, InstallStep::Installed),
            Err(Stop::Cancelled) => self.emit_step(&request.pkg, InstallStep::Idle),
            Err(Stop::Failed(err)) => {
                tracing::warn!(pkg = %request.pkg, "{err}");
                self.emit_step(&request.pkg, InstallStep::Error);
            }
        }
    }

    async fn run(&self, request: &InstallRequest, token: &CancellationToken, part: &std::path::Path) -> Result<(), Stop> {
        let pkg = request.pkg.as_str();
        let permit = or_cancel(token, Arc::clone(&self.downloads).acquire_owned())
            .await?
            .map_err(|_| InstallError::Io("download queue closed".to_owned()))?;
        let started = AtomicBool::new(false);
        let progress = |received: u64, total: Option<u64>| {
            if !started.swap(true, Ordering::SeqCst) {
                self.emit_step(pkg, InstallStep::Downloading);
            }
            if received > 0 {
                let _ = self.events.try_send(InstallEvent::Progress { pkg: pkg.to_owned(), received, total });
            }
        };
        let bytes = download(&self.client, &self.config, &request.url, part, token, &progress).await?;
        drop(permit);

        let req = request.clone();
        let (sealed, manifest, verified) = blocking(move || verify_and_seal(&bytes, &req)).await??;

        let _commit = or_cancel(token, self.commit_lock.lock()).await?;
        self.emit_step(pkg, InstallStep::Installing);
        let store = self.config.store_dir.clone();
        let name = pkg.to_owned();
        let installed = blocking(move || read_installed(&store, &name)).await?;
        check_install(installed.as_ref(), manifest.version_code, verified.public_key())?;
        self.begin_commit(pkg)?;
        let store = self.config.store_dir.clone();
        let name = pkg.to_owned();
        blocking(move || commit(&store, &name, &sealed)).await??;
        let _ = self.events.try_send(InstallEvent::Installed {
            pkg: pkg.to_owned(),
            version_code: manifest.version_code,
            version_name: manifest.version_name,
        });
        Ok(())
    }
}

/// Digest, fingerprint and signature first, then the manifest, then the archive with `signerPub` written. Nothing
/// reaches the store before this returns `Ok`.
fn verify_and_seal(bytes: &[u8], request: &InstallRequest) -> Result<(Vec<u8>, Manifest, Verified), InstallError> {
    let verified =
        verify_package(bytes, &request.sha256, &request.ed25519_pub, &request.repo_fingerprint, &request.ed25519)?;
    let manifest = read_package(bytes)?.manifest;
    manifest.check_index_pkg(&request.pkg)?;
    let sealed = with_signer(bytes, &verified)?;
    Ok((sealed, manifest, verified))
}

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T, InstallError> {
    tokio::task::spawn_blocking(work).await.map_err(|e| InstallError::Io(format!("install task failed: {e}")))
}
