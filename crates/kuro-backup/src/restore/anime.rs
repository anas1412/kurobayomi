//! Anime entry restore (area 22, T-22a-022 to T-22a-025). Seasons are restored under their parent (D7 and D18 fixed):
//! `plan_anime` splits the sorted list into top-level rows and seasons, and each season takes the database id of its
//! parent. Rules follow `plans/22-backup-restore/p1-behavior.md` section 4.4 with the task-block fixes.

use std::collections::{BTreeSet, HashMap, HashSet};

use kuro_db::codec::GenreList;
use kuro_db::schema_anime::{Memo, Millis};
use kuro_db::statements_anime::{
    AnimeRow, AnimeTrack, AnimeTrackUpdate, AnimeUpdate, EpisodeRow, EpisodeUpdate, anime_by_id, anime_by_url_and_source,
    episodes_by_anime_id, insert_anime, insert_episode, tracks_by_anime_id, update_anime, update_episode, update_track,
    upsert_history, upsert_track,
};
use kuro_db::{DbResult, Transaction};
use kuro_domain::entries::fetch_interval::{ItemDates, Stored, to_update};
use rusqlite::OptionalExtension;

use super::categories::{differs_from_stored, membership_ids};
use super::merge::{RunClock, merge_progress, round6, sort_new_first};
use crate::schema::{BackupAnime, BackupAnimeHistory, BackupAnimeTracking, BackupEpisode};

/// A top-level anime row with the seasons that restore under it.
#[derive(Debug, Clone, PartialEq)]
pub struct AnimeRoot {
    pub entry: BackupAnime,
    pub seasons: Vec<BackupAnime>,
}

/// What the anime restore counts for the summary.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AnimeOutcome {
    /// History rows whose `readDuration` is not stored for anime (E-42). Informational.
    pub dropped_read_durations: usize,
}

/// The `(source, url)` pairs of the anime already in `entries`, read once per section (T-22a-022).
pub fn existing_pairs(tx: &Transaction<'_>) -> DbResult<HashSet<(i64, String)>> {
    let mut stmt = tx.prepare("SELECT source, url FROM entries WHERE kind = 'anime'")?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    Ok(rows.collect::<rusqlite::Result<HashSet<_>>>()?)
}

/// Entries not in the database first, newest `lastModifiedAt` first, ties in backup order (T-22a-022).
pub fn sort_anime(entries: Vec<BackupAnime>, existing: &HashSet<(i64, String)>) -> Vec<BackupAnime> {
    sort_new_first(entries, |a| (a.source, a.url.as_str(), a.last_modified_at), existing)
}

/// Splits the sorted list into roots and seasons (T-22a-023). A season is a row whose `parentId` names a row in this
/// file. Every other row is a root, including a season whose parent is missing: it restores as a top-level row (X-43).
pub fn plan_anime(sorted: Vec<BackupAnime>) -> Vec<AnimeRoot> {
    let ids: HashSet<i64> = sorted.iter().filter_map(|a| a.id).collect();
    let is_season = |a: &BackupAnime| a.parent_id.is_some_and(|p| ids.contains(&p));
    let (seasons, roots): (Vec<BackupAnime>, Vec<BackupAnime>) = sorted.into_iter().partition(|a| is_season(a));
    roots
        .into_iter()
        .map(|entry| {
            let seasons = seasons.iter().filter(|s| s.parent_id.is_some() && s.parent_id == entry.id).cloned().collect();
            AnimeRoot { entry, seasons }
        })
        .collect()
}

/// Restores one root and its seasons inside the caller's transaction. Each season takes the database id of the root.
pub fn restore_anime_root(
    tx: &Transaction<'_>,
    clock: &RunClock,
    root: &AnimeRoot,
    categories: &HashMap<i64, i64>,
) -> DbResult<AnimeOutcome> {
    let mut out = AnimeOutcome::default();
    let parent = restore_anime_entry(tx, clock, &root.entry, None, categories, &mut out)?;
    for season in &root.seasons {
        restore_anime_entry(tx, clock, season, Some(parent), categories, &mut out)?;
    }
    Ok(out)
}

fn memberships_differ(tx: &Transaction<'_>, anime_id: i64, ids: &[i64]) -> DbResult<bool> {
    let stored: Vec<i64> = tx
        .prepare("SELECT category_id FROM entry_categories WHERE entry_id = ?1 AND kind = 'anime'")?
        .query_map([anime_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(differs_from_stored(stored, ids))
}

/// True when applying `p` would leave `d` as it is. The write is skipped then, so a repeat restore writes nothing.
fn update_is_noop(p: &AnimeUpdate, d: &AnimeRow) -> bool {
    fn same<T: PartialEq>(p: &Option<T>, cur: &T) -> bool {
        p.as_ref().is_none_or(|v| v == cur)
    }
    fn same_opt<T: PartialEq>(p: &Option<T>, cur: &Option<T>) -> bool {
        p.as_ref().is_none_or(|v| Some(v) == cur.as_ref())
    }
    same(&p.favorite, &d.favorite)
        && same(&p.initialized, &d.initialized)
        && same(&p.version, &d.version)
        && same(&p.status, &d.status)
        && same(&p.title, &d.title)
        && same(&p.viewer_flags, &d.viewer)
        && same(&p.episode_flags, &d.episode_flags)
        && same(&p.cover_last_modified, &d.cover_last_modified)
        && same(&p.date_added, &d.date_added)
        && same(&p.update_strategy, &d.update_strategy)
        && same(&p.fetch_type, &d.fetch_type)
        && same(&p.season_flags, &d.season_flags)
        && same(&p.season_number, &d.season_number)
        && same(&p.season_source_order, &d.season_source_order)
        && same(&p.background_last_modified, &d.background_last_modified)
        && same(&p.memo, &d.memo)
        && same_opt(&p.artist, &d.artist)
        && same_opt(&p.author, &d.author)
        && same_opt(&p.description, &d.description)
        && same_opt(&p.genre, &d.genre)
        && same_opt(&p.thumbnail_url, &d.thumbnail_url)
        && same_opt(&p.last_update, &d.last_update)
        && same_opt(&p.parent_id, &d.parent_id)
        && same_opt(&p.background_url, &d.background_url)
}

/// Replaces the category memberships of one anime (T-22a-025). Written here because the `kuro-db` helper opens its own
/// transaction, which SQLite refuses inside the entry transaction.
fn set_categories(tx: &Transaction<'_>, anime_id: i64, ids: &[i64]) -> DbResult<()> {
    tx.execute("DELETE FROM entry_categories WHERE entry_id = ?1 AND kind = 'anime'", [anime_id])?;
    let unique: BTreeSet<i64> = ids.iter().copied().collect();
    for id in unique {
        tx.execute("INSERT OR IGNORE INTO entry_categories(entry_id, kind, category_id) VALUES (?1, 'anime', ?2)", [anime_id, id])?;
    }
    Ok(())
}

/// Decodes the backup memo bytes. Empty or invalid JSON becomes `{}`.
pub fn memo_of(bytes: &[u8]) -> Memo {
    serde_json::from_slice::<serde_json::Map<String, serde_json::Value>>(bytes).map(Memo).unwrap_or_default()
}

/// Restores one row (a root, or a season under `parent`) inside the caller's transaction. Returns its database id.
pub fn restore_anime_entry(
    tx: &Transaction<'_>,
    clock: &RunClock,
    b: &BackupAnime,
    parent: Option<i64>,
    categories: &HashMap<i64, i64>,
    out: &mut AnimeOutcome,
) -> DbResult<i64> {
    let id = match anime_by_url_and_source(tx, &b.url, b.source)? {
        None => insert_new(tx, b, parent)?,
        Some(d) => {
            update_existing(tx, b, &d, parent)?;
            d.id
        }
    };
    if let Some(ids) = membership_ids(categories, &b.categories) {
        if memberships_differ(tx, id, &ids)? {
            // The membership insert trigger bumps `version` unless the row is syncing; the restore keeps the version it set.
            let version: i64 = tx.query_row("SELECT version FROM entries WHERE _id = ?1 AND kind = 'anime'", [id], |r| r.get(0))?;
            set_categories(tx, id, &ids)?;
            tx.execute("UPDATE entries SET version = ?1 WHERE _id = ?2 AND kind = 'anime'", [version, id])?;
        }
    }
    restore_episodes(tx, id, &b.episodes)?;
    restore_tracks(tx, id, &b.tracking)?;
    restore_history(tx, id, &b.history, out)?;
    refresh_interval(tx, clock, id)?;
    Ok(id)
}

/// A new row (T-22a-022). `initialized` is `description != null`. `parent` is the database id of the root for a season,
/// and `None` for a root (the backup's raw `parentId` is never written, D7).
fn insert_new(tx: &Transaction<'_>, b: &BackupAnime, parent: Option<i64>) -> DbResult<i64> {
    let row = AnimeRow {
        id: 0,
        source: b.source,
        url: b.url.clone(),
        artist: b.artist.clone(),
        author: b.author.clone(),
        description: b.description.clone(),
        genre: Some(GenreList(b.genre.clone())),
        title: b.title.clone(),
        status: i64::from(b.status),
        thumbnail_url: b.thumbnail_url.clone(),
        favorite: b.favorite,
        last_update: Some(0),
        next_update: Some(0),
        initialized: b.description.is_some(),
        viewer: i64::from(b.viewer_flags),
        episode_flags: i64::from(b.episode_flags),
        cover_last_modified: 0,
        date_added: b.date_added,
        update_strategy: b.update_strategy,
        calculate_interval: 0,
        last_modified_at: 0,
        favorite_modified_at: None,
        version: b.version,
        is_syncing: 0,
        fetch_type: b.fetch_type,
        parent_id: parent,
        season_flags: b.season_flags,
        season_number: b.season_number,
        season_source_order: b.season_source_order,
        background_url: b.background_url.clone(),
        background_last_modified: 0,
        memo: memo_of(&b.memo),
    };
    Ok(insert_anime(tx, &row)?)
}

/// The merge of an existing row (T-22a-022). Same ownership as manga (D5, parity). Fetch type follows the newer side.
/// The parent is written only for a season (`parent`). A root leaves it as the row has it.
fn update_existing(tx: &Transaction<'_>, b: &BackupAnime, d: &AnimeRow, parent: Option<i64>) -> DbResult<()> {
    let mut p = AnimeUpdate {
        id: d.id,
        favorite: Some(d.favorite || b.favorite),
        initialized: Some(d.initialized || b.description.is_some()),
        parent_id: parent,
        ..Default::default()
    };
    if b.version > d.version {
        p.artist = b.artist.clone();
        p.author = b.author.clone();
        p.description = b.description.clone();
        p.genre = Some(GenreList(b.genre.clone()));
        p.thumbnail_url = b.thumbnail_url.clone();
        p.status = Some(i64::from(b.status));
        p.version = Some(b.version);
        p.fetch_type = Some(b.fetch_type);
    } else {
        p.title = Some(b.title.clone());
        p.viewer_flags = Some(i64::from(b.viewer_flags));
        p.episode_flags = Some(i64::from(b.episode_flags));
        p.update_strategy = Some(b.update_strategy);
        p.date_added = Some(b.date_added);
        p.last_update = Some(0);
        p.cover_last_modified = Some(0);
        p.season_flags = Some(b.season_flags);
        p.season_number = Some(b.season_number);
        p.season_source_order = Some(b.season_source_order);
        p.background_url = b.background_url.clone();
        p.background_last_modified = Some(0);
        p.memo = Some(memo_of(&b.memo));
    }
    if !update_is_noop(&p, d) {
        update_anime(tx, &p)?;
    }
    Ok(())
}

/// Episodes (T-22a-024). Lookup is by url within this anime. The same-state skip and the progress rule follow chapters.
fn restore_episodes(tx: &Transaction<'_>, anime_id: i64, backup: &[BackupEpisode]) -> DbResult<()> {
    let db = episodes_by_anime_id(tx, anime_id)?;
    let by_url: HashMap<&str, &EpisodeRow> = db.iter().map(|r| (r.url.as_str(), r)).collect();
    for b in backup {
        let row = EpisodeRow {
            id: 0,
            anime_id,
            url: b.url.clone(),
            name: b.name.clone(),
            scanlator: b.scanlator.clone(),
            seen: b.seen,
            bookmark: b.bookmark,
            last_second_seen: b.last_second_seen,
            total_seconds: b.total_seconds,
            episode_number: round6(b.episode_number),
            source_order: b.source_order,
            date_fetch: b.date_fetch,
            date_upload: b.date_upload,
            last_modified_at: 0,
            version: b.version,
            is_syncing: 0,
            summary: b.summary.clone(),
            preview_url: b.preview_url.clone(),
            fillermark: b.fillermark,
            memo: memo_of(&b.memo),
        };
        match by_url.get(row.url.as_str()) {
            None => {
                insert_episode(tx, &row)?;
            }
            Some(d) if same_episode_state(&row, d) => {}
            Some(d) => {
                let (seen, last_second_seen) =
                    merge_progress(row.seen, row.last_second_seen, d.seen, d.last_second_seen);
                update_episode(
                    tx,
                    &EpisodeUpdate {
                        id: d.id,
                        seen: Some(seen),
                        bookmark: Some(row.bookmark || d.bookmark),
                        fillermark: Some(row.fillermark || d.fillermark),
                        last_second_seen: Some(last_second_seen),
                        total_seconds: Some(row.total_seconds),
                        version: Some(row.version),
                        memo: Some(row.memo.clone()),
                        ..Default::default()
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// `forComparison` for an episode: every field but the ids, the upload and fetch dates, `lastModifiedAt` and `version`.
fn same_episode_state(a: &EpisodeRow, d: &EpisodeRow) -> bool {
    EpisodeRow {
        id: d.id,
        anime_id: d.anime_id,
        date_fetch: d.date_fetch,
        date_upload: d.date_upload,
        last_modified_at: d.last_modified_at,
        version: d.version,
        is_syncing: d.is_syncing,
        ..a.clone()
    } == *d
}

/// Tracks (T-22a-025). Same rules as manga tracks, with `last_episode_seen`.
fn restore_tracks(tx: &Transaction<'_>, anime_id: i64, backup: &[BackupAnimeTracking]) -> DbResult<()> {
    let db = tracks_by_anime_id(tx, anime_id)?;
    let by_tracker: HashMap<i64, &AnimeTrack> = db.iter().map(|r| (r.sync_id, r)).collect();
    for b in backup {
        let row = AnimeTrack {
            id: 0,
            anime_id,
            sync_id: i64::from(b.sync_id),
            remote_id: b.remote_id(),
            library_id: Some(b.library_id),
            title: b.title.clone(),
            last_episode_seen: f64::from(b.last_episode_seen),
            total_episodes: i64::from(b.total_episodes),
            status: i64::from(b.status),
            score: f64::from(b.score),
            remote_url: b.tracking_url.clone(),
            start_date: b.started_watching_date,
            finish_date: b.finished_watching_date,
            private: b.private,
        };
        match by_tracker.get(&row.sync_id) {
            None => {
                upsert_track(tx, &row)?;
            }
            Some(d) if same_track_state(&row, d) => {}
            Some(d) => {
                update_track(
                    tx,
                    &AnimeTrackUpdate {
                        id: d.id,
                        remote_id: Some(row.remote_id),
                        library_id: row.library_id,
                        last_episode_seen: Some(d.last_episode_seen.max(row.last_episode_seen)),
                        ..Default::default()
                    },
                )?;
            }
        }
    }
    Ok(())
}

/// `forComparison` for an anime track: every field but the id and the anime id.
fn same_track_state(a: &AnimeTrack, d: &AnimeTrack) -> bool {
    AnimeTrack { id: d.id, anime_id: d.anime_id, ..a.clone() } == *d
}

/// Anime history (T-22a-025). The episode is looked up in this anime (D9 fixed). `last_seen` ends at the later of the
/// two timestamps. `readDuration` is not stored and is counted in `out` (E-42).
fn restore_history(
    tx: &Transaction<'_>,
    anime_id: i64,
    backup: &[BackupAnimeHistory],
    out: &mut AnimeOutcome,
) -> DbResult<()> {
    let episode_ids: HashMap<String, i64> =
        episodes_by_anime_id(tx, anime_id)?.into_iter().map(|r| (r.url, r.id)).collect();
    for h in backup {
        if h.read_duration != 0 {
            out.dropped_read_durations += 1;
        }
        let Some(&episode_id) = episode_ids.get(&h.url) else {
            continue;
        };
        let stored: Option<i64> = tx
            .query_row("SELECT last_at FROM history WHERE item_id = ?1", [episode_id], |r| r.get(0))
            .optional()?;
        let seen_at = stored.map_or(h.last_read, |db| h.last_read.max(db));
        if stored == Some(seen_at) {
            continue;
        }
        upsert_history(tx, episode_id, Millis(seen_at))?;
    }
    Ok(())
}

/// Recomputes `next_update` and `fetch_interval` for one anime from its episodes and the run clock (T-22a-021).
pub fn refresh_interval(tx: &Transaction<'_>, clock: &RunClock, anime_id: i64) -> DbResult<()> {
    let row = anime_by_id(tx, anime_id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)?;
    let items: Vec<ItemDates> = episodes_by_anime_id(tx, anime_id)?
        .iter()
        .map(|r| ItemDates { date_upload: r.date_upload, date_fetch: r.date_fetch })
        .collect();
    let stored = Stored {
        fetch_interval: row.calculate_interval as i32,
        next_update: row.next_update.unwrap_or(0),
        last_update: row.last_update.unwrap_or(0),
    };
    let out = to_update(stored, &items, &clock.now, clock.window, clock.clock_now_ms);
    if row.next_update == Some(out.next_update) && row.calculate_interval == i64::from(out.fetch_interval) {
        return Ok(());
    }
    update_anime(
        tx,
        &AnimeUpdate {
            id: anime_id,
            next_update: Some(out.next_update),
            fetch_interval: Some(i64::from(out.fetch_interval)),
            ..Default::default()
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use chrono::{Local, TimeZone};
    use kuro_db::schema_unified;
    use kuro_db::statements_anime::{categories_by_anime_id, episodes_by_url_and_anime_id, insert_category};
    use rusqlite::Connection;

    use super::*;

    fn clock() -> RunClock {
        RunClock::new(Local.with_ymd_and_hms(2026, 10, 9, 12, 0, 0).single().unwrap())
    }

    fn db() -> (tempfile::TempDir, Connection) {
        let dir = tempfile::tempdir().unwrap();
        let c = schema_unified::open_file(&dir.path().join("kurobayomi.db")).unwrap();
        (dir, c)
    }

    fn anime(url: &str) -> BackupAnime {
        BackupAnime { source: 3, url: url.into(), title: format!("A {url}"), ..Default::default() }
    }

    fn episode(url: &str, seen: bool, last: i64) -> BackupEpisode {
        BackupEpisode { url: url.into(), name: format!("ep {url}"), seen, last_second_seen: last, episode_number: 2.0, version: 1, ..Default::default() }
    }

    fn by_url(c: &Connection, url: &str) -> AnimeRow {
        anime_by_url_and_source(c, url, 3).unwrap().unwrap()
    }

    fn restore(c: &mut Connection, root: &AnimeRoot) -> AnimeOutcome {
        let tx = c.transaction().unwrap();
        let out = restore_anime_root(&tx, &clock(), root, &HashMap::new()).unwrap();
        tx.commit().unwrap();
        out
    }

    #[test]
    fn new_row_with_mapped_categories_keeps_the_backup_version() {
        let (_d, mut c) = db();
        let cat = insert_category(&c, "A", 0, 0).unwrap();
        let mut a = anime("ver");
        a.version = 6;
        a.categories = vec![0];
        let tx = c.transaction().unwrap();
        restore_anime_root(&tx, &clock(), &AnimeRoot { entry: a, seasons: vec![] }, &HashMap::from([(0, cat)])).unwrap();
        tx.commit().unwrap();
        assert_eq!(by_url(&c, "ver").version, 6);
    }

    #[test]
    fn season_rows_are_not_top_level_and_take_the_root_db_id() {
        let (_d, mut c) = db();
        let mut root = anime("root");
        root.id = Some(10);
        let mut season = anime("season-1");
        season.id = Some(11);
        season.parent_id = Some(10);
        season.season_number = 1.0;
        let roots = plan_anime(vec![root.clone(), season.clone()]);
        assert_eq!(roots.len(), 1, "season is not a root");
        assert_eq!(roots[0].seasons.len(), 1);
        restore(&mut c, &roots[0]);
        let parent = by_url(&c, "root");
        let child = by_url(&c, "season-1");
        assert_eq!(child.parent_id, Some(parent.id), "D7: parent is the db id, not the backup id");
        assert!(parent.parent_id.is_none());
    }

    #[test]
    fn season_with_missing_parent_becomes_a_top_level_row() {
        let (_d, mut c) = db();
        let mut orphan = anime("orphan");
        orphan.id = Some(5);
        orphan.parent_id = Some(999);
        let roots = plan_anime(vec![orphan]);
        assert_eq!(roots.len(), 1);
        assert!(roots[0].seasons.is_empty());
        restore(&mut c, &roots[0]);
        assert!(by_url(&c, "orphan").parent_id.is_none(), "X-43: parent_id stays NULL");
    }

    #[test]
    fn existing_root_keeps_its_parent_link() {
        let (_d, mut c) = db();
        let mut root = anime("keep");
        root.id = Some(1);
        restore(&mut c, &AnimeRoot { entry: root.clone(), seasons: vec![] });
        let db_id = by_url(&c, "keep").id;
        c.execute("UPDATE entries SET parent_id = 42 WHERE _id = ?1", [db_id]).unwrap();
        root.parent_id = Some(7);
        restore(&mut c, &AnimeRoot { entry: root, seasons: vec![] });
        assert_eq!(by_url(&c, "keep").parent_id, Some(42), "a root never writes the raw backup parent");
    }

    #[test]
    fn episodes_merge_seen_and_position_and_bookmark_or() {
        let (_d, mut c) = db();
        let mut a = anime("eps");
        a.episodes = vec![episode("e1", true, 0), episode("e2", false, 300)];
        restore(&mut c, &AnimeRoot { entry: a.clone(), seasons: vec![] });
        let id = by_url(&c, "eps").id;

        let mut a2 = anime("eps");
        a2.episodes = vec![
            BackupEpisode { bookmark: true, ..episode("e1", false, 0) },
            BackupEpisode { seen: true, last_second_seen: 120, ..episode("e2", true, 120) },
            episode("e3", false, 0),
        ];
        restore(&mut c, &AnimeRoot { entry: a2, seasons: vec![] });
        let rows = episodes_by_anime_id(&c, id).unwrap();
        let get = |u: &str| rows.iter().find(|r| r.url == u).unwrap().clone();
        assert_eq!(rows.len(), 3);
        assert!(get("e1").seen, "seen never reverts");
        assert!(get("e1").bookmark, "bookmark is OR");
        assert_eq!(get("e2").last_second_seen, 120);
        assert!((get("e2").episode_number - 2.0).abs() < f64::EPSILON);
        assert!(!get("e3").seen);
    }

    #[test]
    fn season_episodes_are_restored_under_the_season() {
        let (_d, mut c) = db();
        let mut root = anime("r2");
        root.id = Some(1);
        let mut season = anime("s2");
        season.id = Some(2);
        season.parent_id = Some(1);
        season.episodes = vec![episode("only-in-season", false, 0)];
        let roots = plan_anime(vec![root, season]);
        restore(&mut c, &roots[0]);
        let s = by_url(&c, "s2");
        let eps = episodes_by_url_and_anime_id(&c, "only-in-season", s.id).unwrap();
        assert_eq!(eps.len(), 1);
    }

    #[test]
    fn history_uses_the_later_timestamp_and_counts_dropped_durations() {
        let (_d, mut c) = db();
        let mut a = anime("hist");
        a.episodes = vec![episode("h1", true, 0)];
        a.history = vec![BackupAnimeHistory { url: "h1".into(), last_read: 500, read_duration: 77 }];
        let out = restore(&mut c, &AnimeRoot { entry: a.clone(), seasons: vec![] });
        assert_eq!(out.dropped_read_durations, 1, "E-42: readDuration not stored");
        let id = by_url(&c, "hist").id;
        let ep = episodes_by_anime_id(&c, id).unwrap()[0].id;
        let seen: i64 = c.query_row("SELECT last_at FROM history WHERE item_id = ?1", [ep], |r| r.get(0)).unwrap();
        assert_eq!(seen, 500);

        a.history = vec![BackupAnimeHistory { url: "h1".into(), last_read: 100, read_duration: 0 }];
        restore(&mut c, &AnimeRoot { entry: a, seasons: vec![] });
        let seen: i64 = c.query_row("SELECT last_at FROM history WHERE item_id = ?1", [ep], |r| r.get(0)).unwrap();
        assert_eq!(seen, 500, "max keeps the later DB value");
    }

    #[test]
    fn categories_are_replaced_when_an_order_maps() {
        let (_d, mut c) = db();
        let cat = insert_category(&c, "Watching", 0, 0).unwrap();
        let mut a = anime("cat-a");
        a.categories = vec![0];
        let tx = c.transaction().unwrap();
        restore_anime_root(&tx, &clock(), &AnimeRoot { entry: a, seasons: vec![] }, &HashMap::from([(0, cat)])).unwrap();
        tx.commit().unwrap();
        let id = by_url(&c, "cat-a").id;
        let got: Vec<i64> = categories_by_anime_id(&c, id).unwrap().into_iter().map(|r| r.id).collect();
        assert_eq!(got, vec![cat]);
    }

    #[test]
    fn sort_and_tracks_follow_the_manga_rules() {
        let (_d, mut c) = db();
        let existing: HashSet<(i64, String)> = [(3, "old".to_string())].into_iter().collect();
        let mut fresh = anime("fresh");
        fresh.last_modified_at = 1;
        let sorted = sort_anime(vec![anime("old"), fresh], &existing);
        assert_eq!(sorted[0].url, "fresh");

        let mut a = anime("trk");
        a.tracking = vec![BackupAnimeTracking { sync_id: 4, media_id: 8, library_id: 1, last_episode_seen: 2.0, ..Default::default() }];
        restore(&mut c, &AnimeRoot { entry: a.clone(), seasons: vec![] });
        a.tracking[0].last_episode_seen = 1.0;
        a.tracking[0].media_id = 9;
        restore(&mut c, &AnimeRoot { entry: a, seasons: vec![] });
        let id = by_url(&c, "trk").id;
        let t = &tracks_by_anime_id(&c, id).unwrap()[0];
        assert_eq!((t.remote_id, t.last_episode_seen), (9, 2.0));
    }

    #[test]
    fn memo_bytes_decode_and_empty_is_an_empty_object() {
        assert!(memo_of(b"").0.is_empty());
        assert_eq!(memo_of(br#"{"k":1}"#).0.get("k"), Some(&serde_json::json!(1)));
    }
}
