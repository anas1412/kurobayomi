//! Where each chapter and episode stands in the running download queues: the one adapter every download button reads
//! (the chapter and episode lists, the Updates rows and the queue page). It reads the manga manager and the anime queue
//! and reports each change as a [`Live`] value: the queue state and percent while queued, then `Downloaded` or
//! `NotDownloaded` once the unit has left the queue.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use gtk::glib;
use gtk::prelude::*;
use kuro_domain::kind::EntryKind;
use kuro_downloads::DownloadState;
use kuro_downloads::anime::model::AnimeDownload;
use kuro_downloads::engine::{AnimeEngine, MangaEngine, QueueEngine};
use kuro_downloads::model::QueuedDownload;

use crate::downloads::Queues;

// ponytail: each follower polls both queues 4 times a second; move to the managers' event channels if many pages follow.
const TICK: Duration = Duration::from_millis(250);

/// One change of a unit, as a download button draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Live {
    pub kind: EntryKind,
    pub id: i64,
    pub state: DownloadState,
    pub percent: u8,
}

/// The percent of a chapter: the mean of its page progress (each 0 to 100; an unknown length, -1, counts as 0).
pub fn chapter_percent(pages: &[i32]) -> u8 {
    if pages.is_empty() {
        return 0;
    }
    let sum: i64 = pages.iter().map(|p| i64::from((*p).clamp(0, 100))).sum();
    (sum / pages.len() as i64) as u8
}

/// One queued unit, kept so its end state can be read once it leaves the queue.
#[derive(Clone)]
enum Entry {
    Manga(Arc<QueuedDownload>),
    Anime(Arc<AnimeDownload>),
}

impl Entry {
    /// Its queue state and percent now.
    fn now(&self) -> (DownloadState, u8) {
        match self {
            Entry::Manga(d) => {
                let pages: Vec<i32> = d.pages.as_ref().map_or_else(Vec::new, |pages| pages.iter().map(|p| p.progress).collect());
                (d.status, chapter_percent(&pages))
            }
            Entry::Anime(d) => (d.status(), d.progress.get().clamp(0, 100) as u8),
        }
    }

    /// The state it ends in once out of the queue: its files on disk are `Downloaded`, anything else (a cancel) is
    /// `NotDownloaded`.
    fn settled(&self, queues: &Queues) -> DownloadState {
        let on_disk = match self {
            Entry::Manga(d) => queues.manga.index().is_chapter_downloaded(d.manga.source_id, &d.manga.title, &d.chapter.name, d.chapter.scanlator.as_deref()),
            Entry::Anime(d) => d.status() == DownloadState::Downloaded,
        };
        if on_disk { DownloadState::Downloaded } else { DownloadState::NotDownloaded }
    }
}

/// Every queued unit of both queues.
fn entries(queues: &Queues) -> HashMap<(EntryKind, i64), Entry> {
    let mut out: HashMap<(EntryKind, i64), Entry> = queues.manga.snapshot().into_iter().map(|d| ((EntryKind::Manga, d.chapter.id), Entry::Manga(d))).collect();
    if let Some(anime) = &queues.anime {
        out.extend(anime.snapshot().iter().map(|d| ((EntryKind::Anime, d.episode.id), Entry::Anime(Arc::clone(d)))));
    }
    out
}

/// The running queues, or `None` when the app runs no download managers. Call it on the GTK thread.
pub fn queues() -> Option<Queues> {
    crate::shell::host::Shell::get()?.download_queues()
}

/// The queue state and percent of one unit now, or `None` when it is not queued.
pub fn queued(queues: &Queues, kind: EntryKind, id: i64) -> Option<(DownloadState, u8)> {
    match kind {
        EntryKind::Manga => queues.manga.queued(id).map(|d| Entry::Manga(d).now()),
        EntryKind::Anime => queues.anime.as_ref()?.queued(id).map(|d| Entry::Anime(d).now()),
    }
}

/// Takes one unit out of its queue (a cancel, with no question). Files already on disk are not touched.
pub fn cancel(kind: EntryKind, id: i64) {
    let Some(queues) = queues() else { return };
    match kind {
        EntryKind::Manga => MangaEngine::new(queues.manga.clone()).remove(&[id]),
        EntryKind::Anime => {
            if let Some(queue) = &queues.anime {
                AnimeEngine::new(queue.clone()).remove(&[id]);
            }
        }
    }
}

/// Calls `on_change` on the GTK thread for every unit whose queue state or percent changed, and once more when it left
/// the queue, for as long as `owner` lives. The first call reports every unit queued at that time. Without download
/// managers nothing is ever reported.
pub fn follow(owner: &impl IsA<gtk::Widget>, on_change: impl Fn(Live) + 'static) {
    let Some(queues) = queues() else { return };
    let owner = owner.as_ref().downgrade();
    let mut seen: HashMap<(EntryKind, i64), (Entry, (DownloadState, u8))> = HashMap::new();
    glib::timeout_add_local(TICK, move || {
        if owner.upgrade().is_none() {
            return glib::ControlFlow::Break;
        }
        let now = entries(&queues);
        for (&(kind, id), entry) in &now {
            let (state, percent) = entry.now();
            if seen.get(&(kind, id)).is_none_or(|(_, last)| *last != (state, percent)) {
                on_change(Live { kind, id, state, percent });
            }
            seen.insert((kind, id), (entry.clone(), (state, percent)));
        }
        seen.retain(|&(kind, id), (entry, _)| {
            if now.contains_key(&(kind, id)) {
                return true;
            }
            on_change(Live { kind, id, state: entry.settled(&queues), percent: 0 });
            false
        });
        glib::ControlFlow::Continue
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_chapter_percent_is_the_mean_of_its_pages() {
        assert_eq!(chapter_percent(&[]), 0, "no page list yet");
        assert_eq!(chapter_percent(&[100, 100, 0, 0]), 50);
        assert_eq!(chapter_percent(&[100, -1, 50]), 50, "an unknown length counts as nothing done");
        assert_eq!(chapter_percent(&[100, 100]), 100);
        assert_eq!(chapter_percent(&[250]), 100, "a page never counts past 100");
    }
}
