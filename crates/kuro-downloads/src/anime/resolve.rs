//! Best-video resolution through the hoster loader (T-24a-014, behaviour 5.2).

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use crate::anime::model::{AnimeDownload, AnimeRef, AnimeSource, EpisodeRef, Video};
use crate::error::DownloadError;

/// Chooses the playable video for an episode: `EpisodeLoader.getHosters`, then `HosterLoader.getBestVideo`. Lives in the
/// player and source areas. `Ok(None)` means no hoster or no playable video.
#[async_trait]
pub trait HosterResolver: Send + Sync {
    async fn best_video(
        &self,
        anime: &AnimeRef,
        episode: &EpisodeRef,
        source: &AnimeSource,
    ) -> Result<Option<Video>, DownloadError>;
}

/// Sets `download.video` if it is unset, and returns it. The resolver runs only while the video is `None`. An empty
/// hoster list or no best video is `NoVideo`. Cancellation leaves the video unset.
pub async fn resolve_video(
    download: &AnimeDownload,
    resolver: &dyn HosterResolver,
    cancel: &CancellationToken,
) -> Result<Video, DownloadError> {
    if let Some(video) = download.video() {
        return Ok(video);
    }
    let found = tokio::select! {
        _ = cancel.cancelled() => return Err(DownloadError::Cancelled),
        result = resolver.best_video(&download.anime, &download.episode, &download.source) => result?,
    };
    let video = found.ok_or(DownloadError::NoVideo)?;
    download.set_video(video.clone());
    Ok(video)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::DownloadState;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Fake {
        answer: Option<Video>,
        calls: AtomicUsize,
        pending: bool,
    }

    #[async_trait]
    impl HosterResolver for Fake {
        async fn best_video(
            &self,
            _: &AnimeRef,
            _: &EpisodeRef,
            _: &AnimeSource,
        ) -> Result<Option<Video>, DownloadError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.pending {
                std::future::pending::<()>().await;
            }
            Ok(self.answer.clone())
        }
    }

    fn download(video: Option<Video>) -> AnimeDownload {
        let source = AnimeSource { id: 1, name: "S".into(), lang: "en".into(), is_http: true, unmetered: false };
        let anime = AnimeRef { id: 1, source_id: 1, url: "/a".into(), title: "A".into() };
        let episode = EpisodeRef {
            id: 2,
            anime_id: 1,
            url: "/e".into(),
            name: "E".into(),
            scanlator: None,
            seen: false,
            bookmark: false,
            fillermark: false,
            source_order: 1,
            upload_date: 0,
            episode_number: 0.0,
        };
        AnimeDownload::new(source, anime, episode, false, video)
    }

    fn video(url: &str) -> Video {
        Video { url: url.into(), title: "best".into(), ..Default::default() }
    }

    #[tokio::test]
    async fn a_known_video_skips_the_resolver() {
        let d = download(Some(video("cached")));
        let fake = Fake { answer: Some(video("other")), calls: AtomicUsize::new(0), pending: false };
        let got = resolve_video(&d, &fake, &CancellationToken::new()).await.unwrap();
        assert_eq!(got.url, "cached");
        assert_eq!(fake.calls.load(Ordering::SeqCst), 0, "only when video is None");
    }

    #[tokio::test]
    async fn the_resolved_video_is_stored_on_the_download() {
        let d = download(None);
        let fake = Fake { answer: Some(video("best")), calls: AtomicUsize::new(0), pending: false };
        let got = resolve_video(&d, &fake, &CancellationToken::new()).await.unwrap();
        assert_eq!(got.url, "best");
        assert_eq!(d.video().map(|v| v.url), Some("best".into()));
        assert_eq!(d.status(), DownloadState::Queue, "resolution does not change status");
    }

    #[tokio::test]
    async fn no_playable_video_is_no_video() {
        let d = download(None);
        let fake = Fake { answer: None, calls: AtomicUsize::new(0), pending: false };
        let err = resolve_video(&d, &fake, &CancellationToken::new()).await.unwrap_err();
        assert!(matches!(err, DownloadError::NoVideo));
        assert_eq!(err.to_string(), "No video found");
        assert!(d.video().is_none());
    }

    #[tokio::test]
    async fn cancellation_leaves_the_video_unset() {
        let d = download(None);
        let fake = Fake { answer: Some(video("late")), calls: AtomicUsize::new(0), pending: true };
        let cancel = CancellationToken::new();
        let stopper = cancel.clone();
        let (result, _) = tokio::join!(resolve_video(&d, &fake, &cancel), async move { stopper.cancel() });
        assert!(matches!(result, Err(DownloadError::Cancelled)));
        assert!(d.video().is_none());
    }
}
