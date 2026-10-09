# Topic — The 26 legacy migration scripts (versions 113 to 139) and the import runner

Paths are repo-relative to `/home/blackbox/aniyomi-src`; scripts live in `data/src/main/sqldelightanime/migrations/`. Purpose of this document: an engineer can implement `kuro-db::anime::legacy` (task T-04a-025, P2) and its equivalence tests (T-04a-026) without reading the `.sqm` files, and understands exactly what each step does to data.

Why this exists at all: a fresh Kurobayomi database is built directly from the final schema (T-04a-003). The chain is needed only to open an Aniyomi `tachiyomi.animedb` that the user copied from an Android device (or an Android backup of the app's databases folder). The primary migration path for user data is the backup restore of area:22-backup-restore; the raw-file import is a convenience (P2).

## 1. Conventions

* File `N.sqm` upgrades `PRAGMA user_version` N to N+1 (SQLDelight rule: migration `N` moves from schema version N). Latest file `138.sqm`; schema version 139. A database whose stored version is V is upgraded by running `V.sqm` to `138.sqm` in order. The first file is `113.sqm`, so versions below 113 cannot be upgraded (Aniyomi itself has no scripts for them).
* The scripts are plain SQLite. SQLDelight-only syntax inside them is `import ...;` lines and the type annotations `AS Long`, `AS Float`, `AS Boolean`, `AS Date`; both are removed before execution (the Rust copies are stored already stripped).
* Android runs `onUpgrade` before `onOpen`, i.e. with `foreign_keys` still OFF (the pragma is set in `onOpen`). The Rust runner must also run with `PRAGMA foreign_keys = OFF` (set before `BEGIN`, since the pragma is ignored inside a transaction), then run `PRAGMA foreign_key_check` after the last step and fail the import if it returns rows, then restore `foreign_keys = ON`. (Verified: the chain also runs and passes `integrity_check` and `foreign_key_check` with `foreign_keys = ON` on a SQLite 3.53 build.)
* Views: on SQLite 3.26 and later `ALTER TABLE ... RENAME` rewrites references to the renamed table inside views and triggers. Steps 114 and 115 rename tables that views reference. To be independent of that, the runner drops EVERY view before step 113 and recreates all eight views from `views.sql` after step 138 (idempotent: the step scripts' own `DROP VIEW IF EXISTS` / `CREATE VIEW` statements still run in between and are harmless).
* Each step runs in its own transaction and writes `PRAGMA user_version = N+1` inside it, so an interrupted import resumes at the failed step. Before the first step the file is copied to `anime.db.bak-v<V>` (T-04a-031).

## 2. Reconstructed baseline (version 113)

The repository has no script that creates version 113; its shape is implied by the `INSERT ... SELECT` column lists of `114.sqm`. The fixture used for the equivalence tests (and in the harness that produced this document) is:

```sql
CREATE TABLE animes(_id INTEGER NOT NULL PRIMARY KEY, source INTEGER NOT NULL, url TEXT NOT NULL, artist TEXT, author TEXT, description TEXT, genre TEXT, title TEXT NOT NULL, status INTEGER NOT NULL, thumbnail_url TEXT, favorite INTEGER NOT NULL, last_update INTEGER, next_update INTEGER, initialized INTEGER NOT NULL, viewer INTEGER NOT NULL, episode_flags INTEGER NOT NULL, cover_last_modified INTEGER NOT NULL, date_added INTEGER NOT NULL);
CREATE TABLE categories(_id INTEGER NOT NULL PRIMARY KEY, name TEXT NOT NULL, sort INTEGER NOT NULL, flags INTEGER NOT NULL);
CREATE TABLE episodes(_id INTEGER NOT NULL PRIMARY KEY, anime_id INTEGER NOT NULL, url TEXT NOT NULL, name TEXT NOT NULL, scanlator TEXT, seen INTEGER NOT NULL, bookmark INTEGER NOT NULL, last_second_seen INTEGER NOT NULL, total_seconds INTEGER NOT NULL, episode_number REAL NOT NULL, source_order INTEGER NOT NULL, date_fetch INTEGER NOT NULL, date_upload INTEGER NOT NULL, FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE);
CREATE TABLE animehistory(animehistory_id INTEGER NOT NULL PRIMARY KEY, animehistory_episode_id INTEGER NOT NULL UNIQUE, animehistory_last_seen INTEGER, animehistory_time_seen INTEGER, FOREIGN KEY(animehistory_episode_id) REFERENCES episodes (_id) ON DELETE CASCADE);
CREATE TABLE animes_categories(_id INTEGER NOT NULL PRIMARY KEY, anime_id INTEGER NOT NULL, category_id INTEGER NOT NULL, FOREIGN KEY(category_id) REFERENCES categories (_id) ON DELETE CASCADE, FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE);
CREATE TABLE anime_sync(_id INTEGER NOT NULL PRIMARY KEY, anime_id INTEGER NOT NULL, sync_id INTEGER NOT NULL, remote_id INTEGER NOT NULL, library_id INTEGER, title TEXT NOT NULL, last_episode_seen REAL NOT NULL, total_episodes INTEGER NOT NULL, status INTEGER NOT NULL, score REAL NOT NULL, remote_url TEXT NOT NULL, start_date INTEGER NOT NULL, finish_date INTEGER NOT NULL, UNIQUE (anime_id, sync_id) ON CONFLICT REPLACE, FOREIGN KEY(anime_id) REFERENCES animes (_id) ON DELETE CASCADE);
```

Caveat: a real v113 file may contain more columns (those are discarded by step 114) and a pre-existing `animehistoryView`; the runner's "drop every view first" rule covers the latter.

## 3. Step-by-step

For each step: what it does, what changes for data, what the Rust runner must preserve.

| Step (file) | Exact effect | Data consequences |
|---|---|---|
| 113 (`113.sqm`) | `UPDATE episodes SET date_upload = date_fetch WHERE date_upload = 0` | episodes without a source date show their fetch date in date sorting |
| 114 (`114.sqm`) | Drops 5 indexes; renames `animes`, `categories`, `episodes`, `animehistory`, `animes_categories`, `anime_sync` to `*_temp`; creates the new tables (typed columns, same names) and copies the listed columns with `INSERT INTO t SELECT cols FROM t_temp`; recreates 5 indexes (`episodes_anime_id_index`, `episodes_unseen_by_anime_index`, `animehistory_history_episode_id_index` on `animehistory_episode_id`, `library_favorite_index`, `animes_url_index`); creates `animehistoryView` if absent; drops the six `*_temp` tables | all rows keep ids; columns not named in the copy lists are discarded; `animes.genre` stays plain TEXT |
| 115 (`115.sqm`) | Drops index and view; renames `animehistory` to `animehistory_temp`; creates `animehistory(_id, episode_id UNIQUE, last_seen NOT NULL, FK)`; copies `animehistory_id, animehistory_episode_id, coalesce(animehistory_last_seen, 0)`; creates `animehistoryView`; recreates index on `episode_id`. The script never drops `animehistory_temp` | `last_seen` NULL becomes 0 (reset); `time_seen` (accumulated watch time) is LOST; a leftover table `animehistory_temp` remains (verified: it keeps the old rows) |
| 116 (`116.sqm`) | drop and recreate `animehistoryView` (same query) | none |
| 117 (`117.sqm`) | `CREATE TABLE animesources` | empty table |
| 118 (`118.sqm`) | drop/recreate `animehistoryView` adding `animes.source`, `animes.favorite`, `animes.cover_last_modified` | none |
| 119 (`119.sqm`) | `CREATE VIEW animeupdatesView` | none |
| 120 (`120.sqm`) | `INSERT OR IGNORE INTO categories(_id, name, sort, flags) VALUES (0, "", -1, 0)`; `CREATE TRIGGER IF NOT EXISTS system_anime_category_delete_trigger` (same body as the fresh trigger but a different NAME) | system category appears |
| 121 (`121.sqm`) | `ALTER TABLE animes ADD COLUMN update_strategy INTEGER NOT NULL DEFAULT 0` | all entries ALWAYS_UPDATE |
| 122 (`122.sqm`) | recreate `animelibView` with an inline per-anime episode-stats subquery (total, seen, latest upload, last seen via LEFT JOIN history, fetched-at, bookmark count) and the category join | none |
| 123 (`123.sqm`) | recreate `animeupdatesView` adding `last_second_seen`, `total_seconds AS totalSeconds` | none |
| 124 (`124.sqm`) | `ALTER TABLE animes ADD COLUMN calculate_interval INTEGER DEFAULT 0 NOT NULL` | all 0 (recomputed at the next library update) |
| 125 (`125.sqm`) | `ALTER TABLE categories ADD COLUMN hidden INTEGER DEFAULT 0 NOT NULL` | no hidden categories |
| 126 (`126.sqm`) | adds `animes.last_modified_at` (NOT NULL DEFAULT 0), `animes.favorite_modified_at`, `animes_categories.last_modified_at`, `episodes.last_modified_at`; sets all `last_modified_at` to `strftime('%s','now')` and `favorite_modified_at` to now for favorites; drops and creates 4 triggers: `update_last_modified_at_animes`, `update_last_favorited_at_animes` (after `update of favorite`; NOTE the name differs from the fresh `update_last_favorite_at_animes`; the preceding `DROP TRIGGER IF EXISTS update_favorite_modified_at_animes` drops a name that never existed), `update_last_modified_at_episodes`, `update_last_modified_at_animes_categories` | every row is stamped with the upgrade time (seconds); an upgraded database keeps an extra column and trigger on `animes_categories` |
| 127 (`127.sqm`) | adds `version` and `is_syncing` to `animes` and `episodes` (NOT NULL DEFAULT 0); recreates triggers `update_anime_version`, `update_episode_and_anime_version`, `insert_anime_category_update_version` | versions start at 0 |
| 128 (`128.sqm`) | `CREATE TABLE extension_repos(base_url PK, name, short_name, website, signing_key_fingerprint UNIQUE NOT NULL)` | empty |
| 129 (`129.sqm`) | `CREATE TABLE custom_buttons`; `INSERT OR IGNORE` of button 1 with the same bytes as the fresh seed | skip-intro button exists |
| 130 (`130.sqm`) | `ALTER TABLE anime_sync ADD COLUMN private INTEGER DEFAULT 0 NOT NULL` | not private |
| 131 (`131.sqm`) | adds `animes.fetch_type` (NOT NULL DEFAULT 1), `parent_id`, `season_flags` (DEFAULT 0), `season_number` (REAL DEFAULT -1.0), `season_source_order` (DEFAULT 0); recreates `episodestatsView`, `animehistorystatsView`, `animelibView` (correlated subqueries for season aggregates), `animeseasonsView`, `animedeletableView` | all entries Episodes-type; view texts of this step are replaced by 133 and 134 |
| 132 (`132.sqm`) | `UPDATE animes SET fetch_type = CASE fetch_type WHEN 2 THEN 1 WHEN 1 THEN 0 WHEN 0 THEN 1 END` | after step 131 every row has `fetch_type = 1`, which this maps to 0 (Seasons); step 135 repairs it. The mapping exists for databases that saw a pre-release numbering |
| 133 (`133.sqm`) | recreates indexes `animes_parent_id`, `animes_fetch_type`; recreates `animeseasonstatsView` and `animelibView` with the aggregate join | none |
| 134 (`134.sqm`) | adds `episodes.summary`, `preview_url`, `fillermark` (NOT NULL DEFAULT 0), `animes.background_url`, `background_last_modified` (NOT NULL DEFAULT 0); recreates `animeupdatesView`, `episodestatsView` (+`fillermarkCount`), `animeseasonstatsView` (+`total_fillermarks`), `animelibView`, `animeseasonsView` | |
| 135 (`135.sqm`) | `UPDATE animes SET fetch_type = 1` | EVERY entry becomes Episodes-type. A database that had real season parents (fetch_type 0) loses that: parents appear as ordinary entries without episodes until the next refresh; children keep `parent_id`. Documented user loss |
| 136 (`136.sqm`) | `ALTER TABLE animes ADD COLUMN memo BLOB NOT NULL DEFAULT x'7b7d'`; same for `episodes` | empty JSON object |
| 137 (`137.sqm`) | creates `extension_store`; `INSERT INTO extension_store(index_url, name, badge_label, signing_key, contact_website, contact_discord, is_legacy, extension_list_url) SELECT base_url \|\| '/repo.json', name, coalesce(short_name, name), signing_key_fingerprint, website, NULL, 1, NULL FROM extension_repos`; `DROP TABLE extension_repos` | every old repo becomes a legacy store |
| 138 (`138.sqm`) | creates `anime_relations` and its two indexes | empty; version becomes 139 |

## 4. Runner design (summary; code in `p1-rust-design.md` section 7)

```text
fn upgrade_legacy(path):                                   // T-04a-025
  v = user_version(path)
  if v >= 139: return Ok                                   // nothing to do (139 is current)
  if v < 113: return Err(TooOldForImport)
  copy path -> path.bak-v{v}                               // T-04a-031 keeps the newest 3
  conn = open(path); PRAGMA foreign_keys=OFF; PRAGMA legacy_alter_table=OFF
  drop all views
  for n in v..=138: BEGIN; run include_str!("legacy/{n}.sql"); PRAGMA user_version = n+1; COMMIT
  recreate views from views.sql
  DROP TABLE IF EXISTS animehistory_temp                   // Proposal: Aniyomi leaves it
  PRAGMA foreign_key_check must be empty; PRAGMA integrity_check must be "ok"
  PRAGMA foreign_keys=ON
```

`user_version` is written with `PRAGMA user_version = N` inside the step transaction (SQLite stores it in the database header, so it rolls back with the transaction).

## 5. Tests (T-04a-025, T-04a-026)

1. Build the baseline of section 2, insert the seed rows listed in `p1-edge-cases-and-tests.md` section 7, run the runner, assert the row-level results in that section (history `last_seen` NULL becomes 0, `time_seen` gone, all `fetch_type = 1`, system category present).
2. Compare `sqlite_master` of the upgraded file with a fresh one and allow only the differences in `p1-topic-fresh-vs-upgraded-schema.md` section 2.
3. For every version V from 113 to 138, build a baseline, run steps `113..V-1` with the Rust runner stopped there (set `user_version = V`), then run the runner from V and compare with the all-at-once result.
4. A 139 database is untouched; a 140 database yields `NewerThanApp`; a 112 database yields `TooOldForImport`.
