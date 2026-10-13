//! The downloaded lookup of a manga's chapters (T-38b-007). Upstream: `List<Chapter>.filterDownloadedChapters(manga)`. The
//! reader's skip paths ask the same question of a chapter (`Fn(&Item) -> bool`), so `downloaded_lookup` has the shape of
//! the reader's `is_downloaded` argument. A local manga counts every chapter as downloaded and is never looked up.

use std::collections::HashMap;

use kuro_data::entry::Entry;
use kuro_data::item::Item;
use kuro_downloads::model::DownloadState;
use kuro_downloads::next::index::DownloadIndex;
use kuro_platform::kind::Kind;

use crate::updates::download_host::DownloadBackend;
use crate::updates::download_state::DiskKey;

/// The lookup of one manga over the download index, with the local passthrough: it is true for every chapter of a local
/// manga and otherwise asks the index for the chapter's name and scanlator.
pub fn downloaded_lookup(index: DownloadIndex, source_id: i64, manga_title: String, local: bool) -> impl Fn(&Item) -> bool {
    move |chapter| local || index.is_downloaded(Kind::Manga, source_id, &manga_title, &chapter.name, chapter.scanlator.as_deref())
}

/// The download state of each unit of an entry that is queued or on disk, for either kind: a queued unit shows its queue
/// state, a finished one in the index shows Downloaded.
pub fn download_states(host: &dyn DownloadBackend, kind: Kind, entry: &Entry, units: &[Item]) -> HashMap<i64, DownloadState> {
    units
        .iter()
        .filter_map(|unit| {
            if let Some(queued) = host.queue_entry(kind, unit.id) {
                return Some((unit.id, queued.status));
            }
            let key = DiskKey { source_id: entry.source, entry_title: entry.title.clone(), unit_name: unit.name.clone(), scanlator: unit.scanlator.clone() };
            host.on_disk(kind, &key).then_some((unit.id, DownloadState::Downloaded))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use kuro_downloads::next::index::IndexRoots;

    use super::*;

    fn chapter(id: i64, name: &str, scanlator: Option<&str>) -> Item {
        Item { id, name: name.into(), scanlator: scanlator.map(str::to_owned), ..Item::create(kuro_platform::kind::Kind::Manga) }
    }

    #[test]
    fn the_index_lookup_matches_the_chapter_folder_and_a_local_manga_passes_everything() {
        let dir = tempfile::tempdir().unwrap();
        let index = DownloadIndex::open(IndexRoots { manga: dir.path().join("manga"), anime: dir.path().join("anime") }, dir.path());
        index.add_unit(Kind::Manga, 7, "Source (EN)", "Berserk", "Ch 1");
        let (one, two) = (chapter(1, "Ch 1", None), chapter(2, "Ch 2", None));
        let lookup = downloaded_lookup(index.clone(), 7, "Berserk".into(), false);
        assert!(lookup(&one) && !lookup(&two));
        let local = downloaded_lookup(index, 7, "Berserk".into(), true);
        assert!(local(&one) && local(&two));
    }
}
