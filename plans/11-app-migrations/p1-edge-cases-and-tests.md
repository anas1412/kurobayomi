# Area 11 app-migrations, part 1: edge cases, failure modes and tests

Task ids refer to `p1-tasks.md`. Rust test locations: framework tests in `crates/kuro-core/tests/migrate.rs` (unit tests inside `kuro-core::migrate` for private helpers), scheduler-setup tests in `crates/kuro-core/tests/migrate_steps.rs`, normaliser tests in `crates/kuro-prefs/tests/legacy_*.rs`, pruning tests in `crates/kuro-prefs/tests/category_prune.rs`.

## 1. Upstream tests to port (`app/src/test/java/mihon/core/migration/MigratorTest.kt`)

The Kotlin test builds a `MigrationContext(false)`, a spied `MigrationJobFactory`, a spied completion listener and a spied `MigrationStrategyFactory`, with `Dispatchers.Main` replaced by a single thread called "UI thread" (`MigratorTest.kt:141-157`). Six tests; each becomes a Rust test using `plan`, `select` and `run` with a counting callback. In Rust there are no spies: `select` returns a `Strategy` value (compare with `assert_eq!`), `plan` returns the filtered slice (compare lengths and versions), and `run` takes an `on_complete` closure that increments an `AtomicUsize`.

| Kotlin test | Setup | Expected (port as assertions) | Rust test name |
|---|---|---|---|
| `initialVersion` | `create(0, 1)`; list `[ALWAYS -> true, 2f -> false]` | strategy is `Initial`; `create` receives exactly 1 migration; the listener ran | `initial_version_runs_only_always` |
| `sameVersion` | `create(1, 1)`; same list | strategy is `Noop`; result `false`; no plan built; no callback | `same_version_is_noop` |
| `noMigrations` | `create(1, 2)`; empty list | strategy is `VersionRange`; result `false`; no plan built; no callback | `empty_list_returns_false_without_callback` |
| `smallMigration` | `create(1, 2)`; `[ALWAYS -> true, 2f -> true]` | `VersionRange`; plan has 2 entries; callback ran | `small_range_includes_always_and_version_two` |
| `largeMigration` | `create(1, 10)`; `[ALWAYS, 2f ... 10f]` all true | `VersionRange`; plan has 10 entries; callback ran | `large_range_includes_all` |
| `withinRangeMigration` | `create(1, 2)`; `[ALWAYS, 2f, 3f -> false]` | `VersionRange`; plan has 2 entries (3f excluded); callback ran | `range_excludes_versions_above_new` |

Note on the Kotlin assertions: `verify { migrationJobFactory.create(capture(migrations)) }` captures the list passed to the job factory, which is the already-filtered list; Rust asserts on `plan(...)`.

## 2. New framework tests (T-11a-001, T-11a-003)

| Test | Behaviour |
|---|---|
| `plan_is_stable_sorted` | two entries at `At(64)` keep their slice order; `Always` entries come first |
| `range_is_inclusive_both_ends` | `select(63, 64)` includes `At(64)`; `select(64, 65)` excludes `At(64)` and includes `At(65)` |
| `downgrade_is_noop` | `select(10, 5)` is `Noop` and the stored version is not changed by `run_blocking` |
| `fresh_profile_skips_versioned` | `old = 0, new = 99`: only `Always` entries run |
| `result_is_any_true_and_all_steps_run` | steps return `[true, false, true]`: all three bodies executed; result `true`; callback once |
| `false_steps_do_not_abort` | `[false, true]`: second step runs |
| `all_false_means_no_callback` | `[false, false]`: result `false`; callback not called; version not stored |
| `error_aborts_chain` | `[true, Err, true]`: third body not called; `run` returns `Err`; callback not called |
| `dry_run_skips_bodies` | bodies not called; each logs the `(Dry-run) Running migration: ...` line; `run` returns `Ok(true)`; `run_blocking` refuses `dry_run` |
| `log_lines_match_kotlin` | captured `tracing` events equal `Running migration: { name = X, version = -1.0 }` and `... version = 64.0 }` |
| `callback_runs_after_last_step` | callback observes that every step already finished |
| `empty_plan_stores_version_explicitly` | with an empty `MIGRATIONS` list a first run still stores `last_version_code = VERSION_CODE` (the T-01a-013 deviation) |
| `second_launch_is_noop` | run, store, run again: no step executes |
| `skip_migrations_flag_stores_version` | `--skip-migrations` stores the code without calling bodies |
| `rerun_after_failure_retries_same_range` | first run fails, second run (same `old`) executes the same steps |
| `block_on_only_in_run_blocking` | grep-style CI check, as in T-01a-012 |
| `idempotency_harness` | for every migration in `MIGRATIONS`, run twice on the same fixture store and compare the stores byte for byte (T-11a-003) |

## 3. Scheduler set-up tests (T-11a-004 to T-11a-007)

Fixtures: `FakeScheduler` recording `enqueue_unique_periodic(spec)` and `cancel_unique(name)`; in-memory typed preferences; `FakePowerNetworkState`.

| Test | Behaviour |
|---|---|
| `backup_interval_zero_cancels` | `backup_interval = 0`: `cancel_unique("BackupCreator")`, no enqueue |
| `backup_default_enqueues` | default 12: spec name `BackupCreator`, interval 43 200 s, flex 600 s, `battery_not_low`, no network/ac requirements, `Backoff::Exponential(600 s)` |
| `library_interval_zero_cancels` | `pref_library_update_interval_key = 0` (default): `cancel_unique("LibraryUpdate-auto")` and `cancel_unique("AnimeLibraryUpdate-auto")` |
| `library_default_restrictions` | interval 24, restrictions default `{"wifi"}`: `wifi_only`, not `unmetered_only`, not `requires_ac`, `battery_not_low` true, `Backoff::Linear(600 s)` |
| `library_all_restrictions` | `{"wifi","network_not_metered","ac"}`: all three flags |
| `library_empty_restrictions` | empty set: only `battery_not_low` |
| `unknown_restriction_ignored` | `{"battery_not_low"}` (legacy value) sets nothing extra |
| `override_hours_wins` | `override_hours = Some(48)` with stored 12: spec interval 172 800 s |
| `setup_is_idempotent` | calling twice yields one stored job per name with identical spec |
| `step_returns_true_and_false` | returns `Ok(true)` with a scheduler; `Ok(false)` when `ctx.scheduler()` is `None` |
| `after_restore_reconfigure` | after the restore hook the three jobs reflect restored values |
| `constraint_checks` | `wifi_only` fails on `kind = Cellular`, passes on Wifi, passes on Ethernet; `unmetered_only` fails when `metered`; `requires_ac` fails on battery; `battery_not_low` fails when `battery_low()` |
| `catch_up_after_long_close` | `last_run_ms` 10 days ago, interval 24 h: job becomes due 60 s after startup, and next due is `now + 24 h` after it ran |
| `no_catch_up_burst` | 10 missed intervals produce exactly one run |
| `backoff_series` | linear: 10, 20, 30 min; exponential: 10, 20, 40 min; capped at 5 h; reset after success |
| `manual_run_blocks_auto` | auto run is retried (backoff) while the manual flag is set |
| `suspend_resume` | a wall-clock jump of +3 h while asleep triggers the due check within the 15 minute safety-net tick |

## 4. Category pruning tests (T-11a-008)

* `default_category_removed_when_unknown`: manga ids `{0,1}`, anime ids `{0,2}`; `default_category = 7` is deleted (reads -1); `default_category = 2` is also deleted because the manga default is checked against the manga ids only, while `default_anime_category = 2` stays and `default_anime_category = 1` is deleted.
* `sets_use_union`: a manga-update set `{"1","2","9"}` with manga `{0,1}` and anime `{0,2}` becomes `{"1","2"}` (union semantics: 2 is kept although it is not a manga id).
* `system_category_zero_kept`.
* `unchanged_set_not_written`: write counter stays 0.
* `all_ten_sets_covered`: each of the ten preference keys of catalogue step 129 is pruned.
* `idempotent`.
* `non_numeric_ids_removed`: ids like `"abc"` are garbage.
* `delete_manga_category_prunes_exclude_set`: regression for quirks item 12; deleting category 5 removes `"5"` from `library_update_categories_exclude`.

## 5. Normaliser fixtures (T-11a-010 to T-11a-017)

Each rule has a table-driven test `(input RawPrefs) -> (expected RawPrefs, expected report entries)`. Required rows:

Group A (T-11a-011): sort Int 0..8 for manga and anime separately, Int 5, Int 9, Int -1 (default to `ALPHABETICAL`); direction Bool true, false, absent (default ascending); Str `LAST_CHECKED` + direction Str `DESCENDING`; Str `UNSEEN` anime; Str already `UNREAD_COUNT,DESCENDING` unchanged; mode key present but direction key absent; `library_sorting_ascending` removed after processing; filters A2 `true`/`false`/absent; A3 values 0, 1, 2, 3, wrong type String, `pref_filter_library_tracked_2`, `..._tracked_x` (non-integer id is ignored and reported), `pref_filter_library_unseen` goes to the anime key; destination already present; flags A4 Int to Long for each of the 10 keys, Long untouched, `Int.MIN_VALUE`-like negative values widened unchanged.
Group B (T-11a-012): interval 0, 1, 2, 3, 4, 5, 6, 7, 8, 12, 24, 168 (1, 2, 3, 4, 6, 8 become 12; others unchanged); restriction key absent plus legacy false produces the default set minus `manga_ongoing`; legacy true changes nothing and removes the legacy key; `battery_not_low` removed; `backup_interval = 0` unchanged.
Group C (T-11a-013): rotation 0..6; viewer 0..6; `reader_tap` false/true/absent/String; destination present.
Group D (T-11a-014): `pref_progress_preference` Str `"0.9"`, `"x"`, Float; skip length Str `"5"`, `"5.5"`, Int; orientation Str `"3"`, Int 0..14, enum name; `pref_deband` 0, 1, 2, 3; `pref_player_view_mode` 0, 1, 2, 7; `gpu_next` true; `pref_sub_select_conf` valid, empty string, malformed, extra fields, missing fields, destination present.
Group E (T-11a-015): `"eng,jpn"`, `"en, ja"` (trim), `",,en,,"`, `"xx,en"`, `"EN"`, `"en-US,en"`, `"  ,en"` (blank item rejected), `"zzz"`, `""` (written back as empty); `jdk-unverified` rows: `fre`, `ger`, `und`, `mis`, `mul`, `zxx`.
Group F (T-11a-016): theme `light`, `Dark`, `SYSTEM`; `relative_time = 0`, `7`, with v2 present; navigation Int 0, 1, 2, 3 and Bool both ways; `enable_doh` true/false; `secure_screen` true/false with v2 present; `source_languages` with and without `all`.
Group G (T-11a-017): anime `https://h/index.min.json`, `https://h/index.json`, `https://h`, `https://h/` (observed result `https://h//repo.json`, kept for parity and documented), `http://h/x/index.min.json/` (suffix not at the end, so only `/repo.json` is appended); manga `user/repo`, `https://h/r`, `http://h/r`, duplicate entries, lexicographic numbering (`Repo #1` for the smallest), fingerprint uniqueness (`NOFINGERPRINT-1`, `-2`), empty set produces no rows.
Group H: every listed ignored key removed and reported; unknown keys kept untouched.

Cross-cutting: `normalise_is_idempotent` (second pass changes nothing, empty converted list); `normalise_never_panics` (proptest over random maps of random kinds with keys drawn from all legacy and new key names plus junk); `no_legacy_key_left` (after normalising a map containing every legacy key, none remains); `report_counts_match` (converted + skipped + ignored equals number of legacy keys present).

Golden files: `tests/fixtures/legacy/*.json` store input maps and expected output for three whole-profile snapshots reconstructed from the code: a pre-52 profile (booleans, Int sort), a build-64 to 81 profile (string sort with separate direction), a build-99 profile (Int tri-state filters, `pref_sub_select_conf`).

## 6. Edge cases and failure modes

| Case | Behaviour |
|---|---|
| Stored version higher than the running build (downgrade) | `Noop`, nothing written; unknown newer settings stay in `config.toml` extras (area:02-preferences-system) |
| `state.toml` missing or unreadable | `old = 0`: only always-run steps; log a warning; the file is recreated with the new code |
| `last_version_code` stored with a wrong type | typed read falls back to `0` (Kotlin deletes and defaults) |
| Two Kurobayomi processes start together | the preference backend's advisory lock (area:02-preferences-system) makes the second wait or exit; migrations run only in the process that holds it, SingleInstance forwards the second launch |
| Process killed in the middle of a run | version not stored; rerun on next start; every step idempotent (harness) |
| Disk full while storing the version | `Err`; startup does not continue; the crash dialog shows the disk error |
| Scheduler preference values out of range (negative interval, huge interval) | interval `<= 0` cancels; values above 8 760 hours are clamped to 8 760 with a warning (Kotlin passes the long straight to WorkManager) |
| Clock changes (manual change, NTP step, suspend) | due time is computed from wall-clock `last_run_ms`; a re-check tick every 15 minutes; a backwards step larger than one interval re-bases `last_run_ms = now` |
| Constraint state unavailable (no D-Bus, no NetworkManager) | `network()` falls back to `gio::NetworkMonitor`; if even that fails constraints are treated as satisfied and a warning is logged once |
| Machine without battery | `on_ac() == true`, `battery_low() == false` |
| Backup import with both legacy and new keys | the new key wins (rule G2) and the report says `DestinationPresent` |
| Backup import with wrong-typed values | rule skipped (`WrongType`), the key kept in the raw map for area:02-preferences-system's extra table |
| Language list with invalid codes | invalid codes dropped, valid ones keep order; result may be empty and is still written |
| Repo set with hundreds of entries | no limit in the normaliser; area:20-extension-strategy-redesign decides a cap |
| Locale-dependent defaults | not touched by the normaliser (it never writes defaults) |
| Windows later | no `cfg(target_os)` in area-11 code; `PowerNetworkState` is the only platform seam |

## 7. Race conditions

* Settings callback vs scheduler task: callbacks only send a message to the scheduler task (`mpsc`), which applies `enqueue_unique_periodic` serially; two quick interval changes cannot interleave.
* Restore vs scheduler: the scheduler holds off any due job while `restore_in_progress` is set and re-evaluates when it clears.
* Manual update vs automatic update: the automatic job checks the manual flag at start and backs off (Kotlin `Result.retry()` parity).
* Startup `reconfigure` vs settings page open: both go through the same function and are idempotent.

## 8. Fixtures needed

`FakeScheduler`, `FakePowerNetworkState`, `MemoryRawPrefStore` (BTreeMap), `TempPaths` (tempfile), `tests/fixtures/legacy/*.json`, `tests/fixtures/lang_codes.json` (list of `(code, jdk_valid)` pairs produced once by a JDK `jshell` snippet that runs the exact Kotlin predicate; rows not yet produced are tagged `jdk-unverified`), a captured-logs helper (`tracing-subscriber` with a `Vec<String>` writer).
