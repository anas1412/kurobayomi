//! What a press on the bottom bar, a menu item or a mark seen sends to the runner (T-39b-013, T-39b-063). Pure: the page
//! hands in the model and the preferences it read, and gets the commands back. Upstream: `AnimeScreenModel.kt`
//! (`markEpisodesSeen`, `runEpisodeDownloadActions`, `markPreviousEpisodeSeen`).

use kuro_data::item::Item;

use crate::entry_core::list_model::RowAction;

use super::actions::{MenuEffect, bulk_mark_targets, menu_effect};
use super::bottom::Effect;
use super::downloads::{selection_to_delete, selection_to_download};
use super::model::EntryModel;
use super::runner::EntryCmd;
use super::seen::{previous_to_mark, seen_effects};

/// The commands of marking `episodes` seen or unseen: the write, and with `remove_after_seen`
/// (`pref_remove_after_marked_as_read`) the delete of the downloads of the episodes it changed.
pub fn seen_cmds(m: &EntryModel, episodes: &[Item], seen: bool, remove_after_seen: bool) -> Vec<EntryCmd> {
    let effects = seen_effects(seen, episodes, remove_after_seen, &m.downloaded);
    let mut cmds = Vec::new();
    if !effects.write_ids.is_empty() {
        cmds.push(EntryCmd::SetSeen { ids: effects.write_ids, seen });
    }
    if !effects.delete_ids.is_empty() {
        cmds.push(EntryCmd::DeleteEpisodeDownloads { anime_id: m.anime.id, ids: effects.delete_ids });
    }
    cmds
}

/// What a row of the bottom bar sends for the selected episodes, and the episode it opens (`(id, external)`).
#[derive(Debug, Default, PartialEq, Eq)]
pub struct BarDispatch {
    pub cmds: Vec<EntryCmd>,
    pub open: Option<(i64, bool)>,
}

pub fn bottom_cmds(m: &EntryModel, effect: Effect, remove_after_seen: bool) -> BarDispatch {
    let selected = m.selected_episodes();
    let anime_id = m.anime.id;
    let mut out = BarDispatch::default();
    match effect {
        Effect::Mark(mark, on) => out.cmds.push(EntryCmd::SetMark { ids: bulk_mark_targets(&selected, mark, on), mark, on }),
        Effect::Seen(seen) => out.cmds = seen_cmds(m, &selected, seen, remove_after_seen),
        Effect::PreviousSeen => {
            if let Some(pointer) = selected.first() {
                out.cmds = previous_seen_cmds(m, pointer.id, remove_after_seen);
            }
        }
        Effect::Open { external } => out.open = selected.first().map(|e| (e.id, external)),
        Effect::Download => {
            let ids = selection_to_download(&selected, &m.downloaded);
            if !ids.is_empty() {
                out.cmds.push(EntryCmd::Download { anime_id, ids, start_now: false });
            }
        }
        Effect::Delete => {
            let ids = selection_to_delete(&selected, &m.downloaded);
            if !ids.is_empty() {
                out.cmds.push(EntryCmd::DeleteEpisodeDownloads { anime_id, ids });
            }
        }
    }
    out
}

/// Marks every episode before `pointer`, in list order, as seen. Only the unseen ones change.
fn previous_seen_cmds(m: &EntryModel, pointer: i64, remove_after_seen: bool) -> Vec<EntryCmd> {
    let ids = previous_to_mark(&m.processed_ids(), pointer, m.filters().is_some_and(|f| f.descending));
    let before: Vec<Item> = m.episodes.iter().filter(|e| ids.contains(&e.id) && !e.done).cloned().collect();
    seen_cmds(m, &before, true, remove_after_seen)
}

/// What an item of one row's right-click menu sends. Mark previous is the bar's, for this row. The Download item
/// deletes a downloaded episode and queues any other one, and only when the app can queue downloads. Select is the
/// page's own (it changes the selection, not the store).
pub fn menu_cmds(m: &EntryModel, id: i64, action: RowAction, has_download_handler: bool, remove_after_seen: bool) -> Vec<EntryCmd> {
    let Some(episode) = m.episodes.iter().find(|e| e.id == id) else { return Vec::new() };
    match (menu_effect(action, episode), action) {
        (Some(MenuEffect::Seen { seen, .. }), _) => seen_cmds(m, std::slice::from_ref(episode), seen, remove_after_seen),
        (Some(MenuEffect::Mark { ids, mark, on }), _) => vec![EntryCmd::SetMark { ids, mark, on }],
        (None, RowAction::MarkPrevious) => previous_seen_cmds(m, id, remove_after_seen),
        (None, RowAction::Download) if has_download_handler => {
            let (anime_id, ids) = (m.anime.id, vec![id]);
            vec![if m.downloaded.contains(&id) { EntryCmd::DeleteEpisodeDownloads { anime_id, ids } } else { EntryCmd::Download { anime_id, ids, start_now: false } }]
        }
        (None, _) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use kuro_prefs::store::PrefStore;

    use super::super::bottom::BottomAction;
    use super::super::model::EntryLoadContext;
    use super::super::store::fixture::with_episodes;
    use super::*;
    use crate::entry_core::detail::SourceInfo;

    fn model(episodes: usize, seen: usize) -> EntryModel {
        let (_dir, store, id) = with_episodes(true, true, episodes, seen);
        EntryModel::load(&store, &PrefStore::in_memory(), id, EntryLoadContext::default(), |_| SourceInfo::default()).unwrap().unwrap()
    }

    fn select(m: &mut EntryModel, numbers: &[f64]) {
        let view = m.processed_ids();
        for n in numbers {
            let id = m.episodes.iter().find(|e| e.number == *n).unwrap().id;
            m.selection.toggle_row(&view, id, crate::entry_core::selection::Command::LongPress);
        }
    }

    fn id_of(m: &EntryModel, n: f64) -> i64 {
        m.episodes.iter().find(|e| e.number == n).unwrap().id
    }

    #[test]
    fn mark_seen_writes_only_the_unseen_and_deletes_their_downloads_with_the_preference_on() {
        let mut m = model(3, 1);
        m.downloaded.extend([id_of(&m, 1.0), id_of(&m, 2.0)]);
        select(&mut m, &[1.0, 2.0]);
        let on = bottom_cmds(&m, BottomAction::MarkViewed.effect(), true);
        assert_eq!(
            on.cmds,
            vec![
                EntryCmd::SetSeen { ids: vec![id_of(&m, 2.0)], seen: true },
                EntryCmd::DeleteEpisodeDownloads { anime_id: m.anime.id, ids: vec![id_of(&m, 2.0)] },
            ],
            "episode 1 was already seen, so its download stays"
        );
        let off = bottom_cmds(&m, BottomAction::MarkViewed.effect(), false);
        assert_eq!(off.cmds, vec![EntryCmd::SetSeen { ids: vec![id_of(&m, 2.0)], seen: true }]);
    }

    #[test]
    fn mark_unseen_never_deletes_a_download() {
        let mut m = model(2, 2);
        m.downloaded.extend(m.episodes.iter().map(|e| e.id));
        select(&mut m, &[1.0]);
        let out = bottom_cmds(&m, BottomAction::MarkUnviewed.effect(), true);
        assert!(matches!(out.cmds.as_slice(), [EntryCmd::SetSeen { seen: false, .. }]));
    }

    #[test]
    fn mark_previous_seen_touches_the_unseen_episodes_before_the_pointer() {
        let mut m = model(4, 0);
        m.downloaded.insert(id_of(&m, 1.0));
        select(&mut m, &[3.0]);
        let out = bottom_cmds(&m, BottomAction::MarkPreviousViewed.effect(), true);
        let mut written = match &out.cmds[0] {
            EntryCmd::SetSeen { ids, seen: true } => ids.clone(),
            other => panic!("a seen write first, got {other:?}"),
        };
        written.sort_unstable();
        let mut want = vec![id_of(&m, 1.0), id_of(&m, 2.0)];
        want.sort_unstable();
        assert_eq!(written, want);
        assert_eq!(out.cmds[1], EntryCmd::DeleteEpisodeDownloads { anime_id: m.anime.id, ids: vec![id_of(&m, 1.0)] });
    }

    #[test]
    fn the_download_and_delete_rows_send_the_missing_and_the_downloaded_episodes() {
        let mut m = model(3, 0);
        m.downloaded.insert(id_of(&m, 2.0));
        select(&mut m, &[1.0, 2.0]);
        let download = bottom_cmds(&m, Effect::Download, false);
        assert_eq!(download.cmds, vec![EntryCmd::Download { anime_id: m.anime.id, ids: vec![id_of(&m, 1.0)], start_now: false }]);
        let delete = bottom_cmds(&m, Effect::Delete, false);
        assert_eq!(delete.cmds, vec![EntryCmd::DeleteEpisodeDownloads { anime_id: m.anime.id, ids: vec![id_of(&m, 2.0)] }]);
    }

    #[test]
    fn the_open_row_names_the_episode_and_the_player() {
        let mut m = model(2, 0);
        select(&mut m, &[2.0]);
        let out = bottom_cmds(&m, Effect::Open { external: true }, false);
        assert_eq!((out.cmds.is_empty(), out.open), (true, Some((id_of(&m, 2.0), true))));
    }

    #[test]
    fn a_seen_menu_item_follows_the_preference_and_the_download_item_toggles_the_download() {
        let mut m = model(2, 0);
        m.downloaded.insert(id_of(&m, 1.0));
        let one = id_of(&m, 1.0);
        let menu = |m: &EntryModel, id, action, handler, remove| menu_cmds(m, id, action, handler, remove);
        let seen = menu(&m, one, RowAction::ToggleRead, false, true);
        assert_eq!(seen.len(), 2, "the write and the delete");
        assert_eq!(menu(&m, one, RowAction::Download, true, false), vec![EntryCmd::DeleteEpisodeDownloads { anime_id: m.anime.id, ids: vec![one] }]);
        let two = id_of(&m, 2.0);
        assert_eq!(menu(&m, two, RowAction::Download, true, false), vec![EntryCmd::Download { anime_id: m.anime.id, ids: vec![two], start_now: false }]);
        assert!(menu(&m, two, RowAction::Download, false, false).is_empty(), "no download handler");
        assert!(menu(&m, two, RowAction::Select, true, false).is_empty(), "Select writes nothing");
        assert!(menu(&m, 9999, RowAction::ToggleRead, true, false).is_empty(), "a row that is gone");
    }

    #[test]
    fn the_menu_mark_previous_marks_the_unseen_episodes_before_its_row_as_the_bar_does() {
        let m = model(4, 0);
        let mut written = match menu_cmds(&m, id_of(&m, 3.0), RowAction::MarkPrevious, true, false).as_slice() {
            [EntryCmd::SetSeen { ids, seen: true }] => ids.clone(),
            other => panic!("one seen write, got {other:?}"),
        };
        written.sort_unstable();
        let mut want = vec![id_of(&m, 1.0), id_of(&m, 2.0)];
        want.sort_unstable();
        assert_eq!(written, want);
    }
}
