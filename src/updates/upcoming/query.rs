//! Reads the upcoming entries of one kind (T-42b-025). The SQL and the local-midnight cut-off live in `kuro-data`;
//! this maps the rows to [`Entry`] and groups them.

use std::sync::Arc;

use chrono::{DateTime, Local};
use kuro_data::entry::EntryRepo;
use kuro_db::handle::Db;
use kuro_platform::kind::Kind;

use crate::updates::upcoming::model::{Entry, Upcoming, build};

/// The upcoming entries of `kind` from local midnight of `now`'s day, soonest first, grouped by day.
pub fn load(db: &Arc<Db>, kind: Kind, now: DateTime<Local>) -> Result<Upcoming, String> {
    let entries = EntryRepo::new(db.clone())
        .upcoming(kind, now.timestamp_millis())
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|e| Entry {
            id: e.id,
            title: e.title,
            status: e.status,
            source_id: e.source,
            next_update_ms: e.next_update,
            cover_url: e.thumbnail_url,
            cover_last_modified: e.cover_last_modified,
        })
        .collect();
    Ok(build(entries, &Local))
}

#[cfg(test)]
mod tests {
    use chrono::{Duration, TimeZone};
    use kuro_data::entry::Entry as Stored;

    use super::*;

    fn test_db() -> (tempfile::TempDir, Arc<Db>) {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("test.db");
        let writer = kuro_db::schema_unified::open_file(&path).expect("open manga db");
        (dir, Arc::new(Db::new(&path, writer).expect("manga db handle")))
    }

    #[test]
    fn upcoming_query_keeps_favourites_with_a_listed_status_from_local_midnight() {
        let (_dir, db) = test_db();
        let now = Local.with_ymd_and_hms(2026, 10, 8, 12, 0, 0).single().expect("noon");
        let midnight = Local.with_ymd_and_hms(2026, 10, 8, 0, 0, 0).single().expect("midnight");
        let yesterday_late = midnight - Duration::minutes(1);
        let far = Local.with_ymd_and_hms(2100, 1, 1, 0, 0, 0).single().expect("2100");
        let repo = EntryRepo::new(db.clone());
        let add = |title: &str, favorite: bool, status: i64, next: i64| {
            repo.insert(&Stored { favorite, status, next_update: next, url: title.into(), title: title.into(), ..Stored::create(Kind::Manga) })
                .expect("insert")
        };
        let today = add("today", true, 1, midnight.timestamp_millis() + 3 * 3_600_000);
        let completed = add("completed", true, 2, far.timestamp_millis());
        let before = add("before", true, 1, yesterday_late.timestamp_millis());
        let far_away = add("far", true, 4, far.timestamp_millis());
        add("not favourite", false, 1, far.timestamp_millis());

        let up = load(&db, Kind::Manga, now).expect("load");
        let ids: Vec<i64> = up.entries.iter().map(|e| e.id).collect();
        assert!(ids.contains(&today), "E-C1: today from local midnight is included");
        assert!(ids.contains(&far_away), "E-C2: no upper bound");
        assert!(!ids.contains(&completed), "E-C3: completed is not in the status set");
        assert!(!ids.contains(&before), "E-C1: yesterday 23:59 is excluded");
        assert_eq!(ids.len(), 2, "E-C5: the favourite flag is required");
    }
}
