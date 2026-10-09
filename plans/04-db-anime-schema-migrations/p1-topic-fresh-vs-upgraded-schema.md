# Topic — Fresh-install schema versus upgraded schema (measured differences)

Method: the ten `dataanime/*.sq` and eight `view/*.sq` files were turned into plain SQL (annotations removed) and executed in SQLite 3.53.4 to get the FRESH shape; the 26 scripts `113.sqm` to `138.sqm` were executed in order on the reconstructed v113 baseline (`p1-topic-legacy-migration-chain.md` section 2) to get the UPGRADED shape; then `sqlite_master`, `PRAGMA table_info` and `PRAGMA foreign_key_list` of both were compared after whitespace normalisation. This extends the list in `plans/catalogs/sql-04.md` section 0.3 with two items it does not have (the leftover table and the index name).

## 1. What matches

* Column NAMES and ORDER of `animes`, `episodes`, `animehistory`, `categories`, `anime_sync` are identical (the migrations add columns in the same order the `.sq` file declares them; this is what keeps SQLDelight's positional `SELECT *` mapping valid for upgraded files).
* Types and NOT NULL flags match except the items below. Foreign keys match for all six FK-bearing tables.
* Seven views match textually after normalisation: `animelibView`, `animeseasonsView`, `animeseasonstatsView`, `animeupdatesView`, `animehistoryView`; `episodestatsView`, `animehistorystatsView` and `animedeletableView` differ only in table aliases (`e`, `A` in the scripts versus `episodes`, `M` in the `.sq` files), which has no semantic effect.
* Triggers `update_last_modified_at_animes`, `update_anime_version`, `update_episode_and_anime_version`, `update_last_modified_at_episodes`, `insert_anime_category_update_version` match.

## 2. Differences (complete list)

| # | Object | FRESH (`.sq`) | UPGRADED (`.sqm`) | Effect |
|---|---|---|---|---|
| 1 | index on `animes(favorite) WHERE favorite = 1` | named `animelib_favorite_index` (`animes.sq:43`) | named `library_favorite_index` (created by `114.sqm`) | none; only the name |
| 2 | trigger on `AFTER UPDATE OF favorite ON animes` | `update_last_favorite_at_animes` | `update_last_favorited_at_animes` (`126.sqm`) | none; same body |
| 3 | system category delete trigger | `system_category_delete_trigger` | `system_anime_category_delete_trigger` (`120.sqm`) | none; same body |
| 4 | `animes_categories.last_modified_at` and trigger `update_last_modified_at_animes_categories` | absent | column `INTEGER NOT NULL DEFAULT 0` plus the trigger (`126.sqm`) | upgraded files stamp link rows on UPDATE (links are never updated in practice) |
| 5 | table `animehistory_temp` | absent | present: `115.sqm` renames `animehistory` and never drops the old table (it still holds the pre-115 rows, with FK to `episodes`) | dead weight; cascades with episode deletes. Proposal: drop it after the upgrade (T-04a-026) |
| 6 | `animehistory.last_seen` | `INTEGER` nullable | `INTEGER NOT NULL` (`115.sqm`, NULL coalesced to 0) | the codec must accept NULL for fresh files |
| 7 | `animes.season_flags`, `season_number`, `season_source_order`, `background_last_modified` defaults | NOT NULL, no default | defaults `0`, `-1.0`, `0`, `0` | `INSERT`s that omit the columns work only on upgraded files; the Kotlin `insert` always supplies them |
| 8 | `episodes.fillermark` default | NOT NULL, no default | `DEFAULT 0` (`134.sqm`) | same as 7 |
| 9 | `animes.memo`, `episodes.memo` default | `DEFAULT '{}'` (TEXT literal in a BLOB-affinity column) | `DEFAULT x'7b7d'` (BLOB) | a defaulted row reads back as TEXT on fresh files and BLOB on upgraded; the codec accepts both |
| 10 | type annotations in script text | the annotations `AS Long`, `AS Float`, `AS Boolean` appear only in `.sqm` text and are not stored in the database | | none |

The reconstruction also confirmed the data-level consequences of the scripts (all `fetch_type` values 1 after `135.sqm`, `time_seen` gone, system category present, `extension_repos` converted).

## 3. Rules that follow for the Rust code

1. Read every row by column name; never rely on column index or on `SELECT *` width.
2. `Memo::from_sql` accepts `ValueRef::Text` and `ValueRef::Blob`; write always as Blob.
3. `last_seen` is `Option<i64>` in the row type; the history queries filter with `seenAt > 0` so NULL never surfaces.
4. Never reference a trigger or index by name (the names differ by origin).
5. Kurobayomi creates only the fresh shape; the golden schema dump (`T-04a-027`) therefore lists `animelib_favorite_index`, `update_last_favorite_at_animes`, `system_category_delete_trigger`.
6. The equivalence test (T-04a-026) encodes rows 1 to 9 as the only allowed differences, so any new drift fails the build.
