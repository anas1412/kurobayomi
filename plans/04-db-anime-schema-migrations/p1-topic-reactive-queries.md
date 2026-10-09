# Topic — Reactive queries (replacement of SQLDelight `asFlow()` and Kotlin Flows)

## 1. What Aniyomi does

`AnimeDatabaseHandler.subscribeToList/One/OneOrNull` wrap `Query.asFlow()` from `app.cash.sqldelight:coroutines-extensions`. A generated query registers a listener on the set of tables it reads; each generated mutating statement calls `notifyQueries` for the tables it writes, which invalidates listeners; the flow re-runs the query on `queryDispatcher` (`Dispatchers.IO`) and emits. The first emission happens when collection starts. (`data/src/main/java/tachiyomi/data/handlers/anime/AndroidAnimeDatabaseHandler.kt:63-77`.)

Observable consequences the UI relies on: every screen showing library, history, updates, categories or episodes refreshes by itself after any write made anywhere in the app; flows never complete; an exception inside the query terminates the flow (for `mapToOne` when the row vanished).

## 2. Rust design

* Writer connection registers three hooks (rusqlite feature `hooks`): `update_hook(|action, db, table, rowid| pending.insert(table))` collects table names (all changes, including trigger and cascade writes); `commit_hook(|| { bus.send(take(pending)); false })` publishes after commit; `rollback_hook(|| pending.clear())` discards.
* The bus is `tokio::sync::broadcast::Sender<Arc<TableSet>>` (capacity 256). `TableSet` is a `u32` bit mask over the ten tables (`animes = 1<<0`, `episodes`, `animehistory`, `animes_categories`, `categories`, `anime_sync`, `animesources`, `anime_relations`, `custom_buttons`, `extension_store`).
* `Db::subscribe<T>(tables: TableSet, f: impl Fn(&Connection) -> Result<T> + Send + 'static) -> impl Stream<Item = Result<T>>`: run `f` on a reader connection immediately; then loop: wait for a bus message whose mask intersects `tables`; drain further messages for 50 ms (coalescing); run `f` again; yield. A lagged receiver (`RecvError::Lagged`) triggers one extra run (no data is lost because the query re-reads the database).
* Distinct-until-changed: the stream compares the new value with the last emitted one (`T: PartialEq`) and skips equal values, which prevents UI redraws when a write touched a row outside the result.
* GTK bridge: `ui::db_bridge::spawn_stream(stream, |value| ...)` forwards items through an `async_channel` to `glib::MainContext::default().spawn_local`, so widgets are only touched on the main thread; dropping the returned handle cancels the stream.

## 3. Dependency sets per stream (what each query reads)

| Stream | Tables (bit mask members) |
|---|---|
| `anime_by_id`, `anime_by_url_and_source` | `animes` |
| `library` | `animes`, `episodes`, `animehistory`, `animes_categories` |
| `favorites_by_source`, `source_favorite_counts` | `animes` |
| `seasons_of(parent)` | `animes`, `episodes`, `animehistory` |
| `deletable_parents` | `animes` |
| `episodes_by_anime` | `episodes` |
| `history(query)` | `animes`, `episodes`, `animehistory` |
| `updates_*` | `animes`, `episodes` |
| `upcoming` | `animes` |
| `categories_*` | `categories` (and `animes_categories` for the by-anime variants) |
| `tracks_*` | `anime_sync` |
| `stub_sources` | `animesources` |
| `related(anime)` | `anime_relations`, `animes` |
| `custom_buttons` | `custom_buttons` |
| `extension_store_all`, `extension_store_count` | `extension_store` |

## 4. Tests (T-04a-007)

* delete an anime row: the bus reports `animes` plus every cascaded table (`episodes`, `animehistory`, `animes_categories`, `anime_sync`, `anime_relations`);
* a rolled-back transaction publishes nothing;
* ten commits within 50 ms produce one re-run;
* a stream on `anime_by_id` yields `None` after the row is deleted and keeps running;
* dropping the stream drops its receiver (`bus.receiver_count()` decreases).
