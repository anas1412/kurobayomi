//! Local source for folders, archives and video files, with ComicInfo and EPUB metadata (area:14).

use std::io;

use error::LocalSourceError;

pub mod anime;
pub mod bridge;
pub mod browse;
pub mod epub;
pub mod error;
pub mod format;
pub mod fs;
pub mod ids;
pub mod images;
pub mod json_side;
pub mod manga;
pub mod media;
pub mod names;
pub mod natsort;
pub mod recognition;
pub mod roots;
pub mod sort;

pub use anime::LocalAnimeSource;
pub use ids::{LOCAL_SOURCE_ID, is_local_source};
pub use manga::LocalMangaSource;

/// Runs `work` on a blocking thread. Local listings and reads are synchronous file-system calls.
async fn blocking<T, F>(work: F) -> Result<T, LocalSourceError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, LocalSourceError> + Send + 'static,
{
    tokio::task::spawn_blocking(work).await.map_err(io::Error::other)?
}
