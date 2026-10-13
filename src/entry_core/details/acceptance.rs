//! The acceptance test of the anime page (T-39b-063): open an entry that is not in the library, filter to unseen
//! episodes, mark one seen, and add the entry to the library. It drives the real runner over a migrated database and
//! applies each event to the model the way the page does (`route::View::on_event`), so what it checks is what the page
//! draws: the rows, the item header and the library state. The page's widgets are thin over these values.

use std::sync::Arc;
use std::time::Duration;

use async_channel::Receiver;
use kuro_db::flags::item as ep;
use kuro_prefs::store::PrefStore;

use super::bottom::BottomAction;
use super::dispatch::bottom_cmds;
use super::model::{EntryModel, EntryLoadContext};
use super::runner::{self, EntryBackend, EntryCmd, EntryEvent, EntryFetch};
use super::store::fixture::with_episodes;
use crate::entry_core::source_info::SourceInfo;

struct Backend;

impl EntryBackend for Backend {
    fn source_info(&self, _: i64) -> SourceInfo {
        SourceInfo { name: "Fake".into(), stub: false }
    }

    fn fetch(&self, _: i64, _: super::entry_state::RefreshPlan, _: crate::entry_core::refresh_rules::Trigger) -> EntryFetch {
        Box::pin(async { Ok(Vec::new()) })
    }
}

/// Applies events to `model` until `done` holds for it. A stuck page fails the test after five seconds.
async fn settle(rx: &Receiver<EntryEvent>, model: &mut EntryModel, done: impl Fn(&EntryModel) -> bool) {
    while !done(model) {
        match tokio::time::timeout(Duration::from_secs(5), rx.recv()).await.expect("an event in time").expect("open") {
            EntryEvent::Snapshot { entry, episodes } => model.apply_snapshot(*entry, episodes),
            EntryEvent::Downloads(on_disk) => model.downloaded = on_disk,
            _ => {}
        }
    }
}

#[test]
fn filter_to_unseen_mark_seen_and_add_to_library() {
    let rt = tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build().unwrap();
    let (_dir, store, id) = with_episodes(false, true, 3, 0);
    rt.block_on(async {
        let runner = runner::spawn(rt.handle(), store, PrefStore::in_memory(), Arc::new(Backend), EntryLoadContext::default());
        runner.send(EntryCmd::Load { entry_id: id });
        let rx = runner.events();
        let EntryEvent::Loaded(Some(model)) = rx.recv().await.unwrap() else { panic!("the model first") };
        let mut model = *model;
        assert_eq!(model.rows().len(), 3, "no filter: every episode shows");
        assert!(!model.entry.favorite);

        // the filter sheet: Unseen. The entry is not in the library, so the store holds the flags it was given.
        model.note_flag_write(ep::SHOW_UNDONE, ep::UNDONE_MASK);
        runner.send(EntryCmd::SetFlagBits { entry_id: id, flag: ep::SHOW_UNDONE, mask: ep::UNDONE_MASK });
        runner.send(EntryCmd::SetSeen { ids: vec![model.episodes[0].id], seen: true });
        settle(&rx, &mut model, |m| m.episodes.iter().filter(|e| e.done).count() == 1).await;
        assert_eq!(model.rows().len(), 2, "the filter hides the seen episode");
        assert_eq!(model.item_header().count, "2 episodes");

        // select the episode that is left and press "Mark as seen" on the bar
        let unseen_ids: Vec<i64> = model.episodes.iter().filter(|e| !e.done).map(|e| e.id).collect();
        let view = model.processed_ids();
        model.selection.toggle_row(&view, unseen_ids[0], crate::entry_core::selection::Command::LongPress);
        let dispatch = bottom_cmds(&model, BottomAction::MarkViewed.effect(), false);
        assert_eq!(dispatch.cmds, vec![EntryCmd::SetSeen { ids: vec![unseen_ids[0]], seen: true }]);
        model.selection.clear();
        for cmd in dispatch.cmds {
            runner.send(cmd);
        }
        settle(&rx, &mut model, |m| m.episodes.iter().filter(|e| e.done).count() == 2).await;
        assert_eq!(model.rows().len(), 1, "two are seen and hidden");
        assert!(model.fab().visible, "an unseen episode is left");

        // the library button
        runner.send(EntryCmd::ToggleLibrary { entry_id: id, check_duplicates: true });
        settle(&rx, &mut model, |m| m.entry.favorite).await;
        assert!(model.entry.favorite, "added to the library");
        assert_eq!(model.episodes.iter().filter(|e| e.done).count(), 2, "the seen marks stay");
    });
}
