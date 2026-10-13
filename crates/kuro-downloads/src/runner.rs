//! The seam between the shared queue and the two ways of downloading a unit: page fetch to `.cbz` (manga) and hoster
//! resolve to `.mkv` (anime). One `UnitRunner` per kind (docs/design/unified-downloads.md 2.3).
//!
//! The actor, not the runner, does the free-space precheck, sets `Downloading`, records the finished unit in the index and
//! emits the events.

use std::sync::Arc;

use async_trait::async_trait;
use kuro_platform::kind::Kind;
use tokio_util::sync::CancellationToken;

use crate::index::DownloadIndex;
use crate::layout::Layout;
use crate::unit::{EntryRef, SourceKey, UnitProgress, UnitRef};
use crate::error::DownloadError;
use crate::source::ChapterSource;

#[async_trait]
pub trait UnitRunner: Send + Sync {
    fn kind(&self) -> Kind;

    /// Whether the source can be reached now (installed). `false` keeps the unit queued as waiting.
    fn can_run(&self, source: &SourceKey) -> bool;

    /// Staging folder names of a unit under its entry folder, kept by the startup sweep and the cancel cleanup.
    fn staging_names(&self, unit: &UnitRef) -> Vec<String>;

    /// Runs one unit to the end. `Ok` names the finished folder or file stem, which the actor records in the index.
    /// `Err(Cancelled)` means a pause or cancel; any other error puts the unit in ERROR.
    async fn run(&self, job: UnitJob, ctx: RunCtx) -> Result<UnitDone, DownloadError>;
}

pub struct UnitJob {
    pub entry: EntryRef,
    pub unit: UnitRef,
    pub source: SourceKey,
}

pub struct UnitDone {
    pub dir_name: String,
}

pub struct RunCtx {
    pub cancel: CancellationToken,
    pub progress: Arc<UnitProgress>,
    /// This kind's layout.
    pub layout: Layout,
    /// Read only, for the "already on disk" short cut.
    pub index: DownloadIndex,
}

/// Finds the live source of a manga or novel (was `delete::ChapterSourceResolver`). `None`: not installed.
pub trait ChapterSources: Send + Sync {
    fn chapter_source(&self, source_id: i64) -> Option<Arc<dyn ChapterSource>>;
}

pub trait SourceDirectory: Send + Sync {
    /// The source of an id as the downloads need it: the live source, else the stub table's name, else `None`.
    fn source_key(&self, kind: Kind, source_id: i64) -> Option<SourceKey>;
}
