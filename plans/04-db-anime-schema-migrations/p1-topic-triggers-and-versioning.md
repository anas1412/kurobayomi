# Topic — Triggers, version counters and the is_syncing flag

Source: `data/src/main/sqldelightanime/dataanime/animes.sq:48-73`, `episodes.sq:32-55`, `animes_categories.sq:11-16`, `categories.sq:12-18`. Behaviour below was measured in SQLite 3.53.4 with `recursive_triggers = OFF` (the default) and `foreign_keys = ON`.

## 1. Purpose of the columns

| Column | Written by | Meaning | Unit |
|---|---|---|---|
| `animes.last_modified_at`, `episodes.last_modified_at` | trigger on every UPDATE of the row | last local modification | Unix seconds |
| `animes.favorite_modified_at` | trigger when `favorite` is in the SET list | last time the statement touched the favorite column (NULL = never) | Unix seconds |
| `animes.version`, `episodes.version` | triggers (and explicit assignment by restore) | monotonically increasing change counter used to decide whose copy is newer during backup restore (`AnimeRestorer.restoreExistingAnime`: higher `version` wins) | integer |
| `animes.is_syncing`, `episodes.is_syncing` | restore (`animes.update isSyncing = 1`) and every normal partial update (`isSyncing = 0`) | "bulk write in progress": suppresses version bumps | 0 / 1 |

`last_modified_at` is exported in backups (`BackupAnime.lastModifiedAt`, `BackupEpisode.lastModifiedAt`) and used by `AnimeRestorer.sortByNew` (restore order: entries not yet in the DB first, then most recently modified first).

## 2. The seven triggers

1. `update_last_favorite_at_animes` — `AFTER UPDATE OF favorite ON animes`: `UPDATE animes SET favorite_modified_at = strftime('%s','now') WHERE _id = new._id`. `UPDATE OF col` fires when `col` is named in the SET clause, whether or not its value changes.
2. `update_last_modified_at_animes` — `AFTER UPDATE ON animes FOR EACH ROW`: sets `last_modified_at = strftime('%s','now')`.
3. `update_anime_version` — `AFTER UPDATE ON animes` (no WHEN): `UPDATE animes SET version = version + 1 WHERE _id = new._id AND new.is_syncing = 0 AND (new.url != old.url OR new.description != old.description OR new.favorite != old.favorite)`. The three comparisons propagate NULL: `description` changing from or to NULL does not bump.
4. `update_last_modified_at_episodes` — same as 2 for `episodes`.
5. `update_episode_and_anime_version` — `AFTER UPDATE ON episodes WHEN new.is_syncing = 0 AND (new.seen != old.seen OR new.bookmark != old.bookmark OR new.last_second_seen != old.last_second_seen)`: bump `episodes.version`, and bump `animes.version` of the parent when the parent's `is_syncing` is 0. Not covered: `fillermark`, `total_seconds`, `summary`, `preview_url`, `memo`, `name`, `url`.
6. `insert_anime_category_update_version` — `AFTER INSERT ON animes_categories`: bump `animes.version` of the linked anime unless it is syncing. Deleting links never bumps.
7. `system_category_delete_trigger` — `BEFORE DELETE ON categories`: `RAISE(ABORT, 'System category can''t be deleted')` when `old._id <= 0`.

## 3. How the triggers compose (recursive_triggers OFF)

A trigger's body that updates the same table fires the OTHER AFTER UPDATE triggers of that table, but a trigger never re-enters itself. So one user `UPDATE animes` that changes the description runs: trigger 2 (stamp), trigger 3 (inner `UPDATE ... version + 1`, which fires trigger 2 again for the stamp, trigger 1 only if `favorite` is in its SET list: it is not, and trigger 3 not at all). Net: `last_modified_at` = now, `version` + 1, exactly once. Final values are independent of the (unspecified) firing order.

Measured table (fresh schema, anime id 1, episode id 1):

| Operation | anime `version` | episode `version` | notes |
|---|---|---|---|
| `animes.update` with every parameter NULL | 0 -> 0 | | `last_modified_at` and `favorite_modified_at` stamped |
| `favorite` 0 -> 1 | +1 | | |
| `description` NULL -> `'x'` | +0 | | |
| `description` `'x'` -> `'y'` | +1 | | |
| `url` change | +1 | | |
| update with `is_syncing = 1` and `version = 50` | becomes 50 | | no bump because `new.is_syncing = 1` |
| next normal update changing description | 51 | | `isSyncing = 0` is rebound by the repository |
| `episodes.update seen = 1` | +1 | +1 | |
| `episodes.update fillermark = 1` | +0 | +0 | |
| `episodes.update total_seconds = 1400` | +0 | +0 | |
| `episodes.update last_second_seen = 30` | +1 | +1 | |
| `episodes.update bookmark = 1` while anime `is_syncing = 1` | +0 | +1 | |
| one `animes_categories` insert | +1 | | per inserted row |

## 4. is_syncing lifecycle (OBSERVED)

1. Only `AnimeRestorer.updateAnime` ever writes `is_syncing = 1` (existing entries during a restore).
2. `AnimeRepositoryImpl.partialUpdateAnime` and `EpisodeRepositoryImpl.partialUpdate` always bind `isSyncing = 0`, so the first normal update afterwards clears it; the restore itself ends with `UpdateAnime.awaitUpdateFetchInterval`, which is such an update.
3. The two statements `resetIsSyncing` (animes, episodes) have no caller. If the process dies between steps 1 and 2 inside a transaction the flag rolls back; a flag could only stay at 1 if a future code path committed it. Proposal (T-04a-024): run both statements at open as a safety net (they do not alter `version`, `last_modified_at` is stamped, which is acceptable).
4. Restore writes episode `version` explicitly; because the episode `update` is issued with `isSyncing = 0`, a changed `seen`/`bookmark`/`last_second_seen` adds one more version on top of the restored value (the bump comes AFTER the explicit assignment within the same statement, since triggers run after the row update). Reproduce, do not "fix": backups compare versions only between devices that follow the same rule.

## 5. Rust implementation notes

* Keep the triggers in the SQL schema file verbatim (T-04a-003); do not re-implement them in Rust. Do not enable `recursive_triggers`.
* Do not write `last_modified_at` or `favorite_modified_at` from Rust (the `update` statement does not list them; `insert` writes literal 0 for `last_modified_at`).
* Tests (T-04a-027) assert the measured table above row by row; the clock is real (`strftime('%s','now')`), so tests compare "within 2 seconds of now" instead of exact stamps.
