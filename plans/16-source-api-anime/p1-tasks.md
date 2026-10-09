# 16-source-api-anime: implementation tasks (part 1 of 1)

Scope: anime source API (`AnimeSource`, `AnimeCatalogueSource`, `AnimeHttpSource`, `ParsedAnimeHttpSource`, `ResolvableAnimeSource`, `Video`, `Hoster`, `SAnime`, `SEpisode`, filters, season/episode fetch types, preferences helpers) plus the manga-side files and utilities that share the package (overlap with area:15-source-api-manga). Every file listed by `areas.py files 16-source-api-anime 1` is covered by at least one task (see p1-inventory.md, column "Task ids").

Target Rust layout: p1-rust-design.md sections 2 and 3. Behavior: p1-behavior.md. Edge cases: p1-edge-cases-and-tests.md. Data: p1-data-and-config.md.

Priority rule used: P0 = needed to browse, read details, get episodes, pick a stream and play it through a builtin source. P1 = parity features and helpers most users rely on. P2 = long tail (torrents, serialized codec, runtime factories). P3 = Android-only or obsolete, with a drop decision.

---

### T-16a-001 — Source module skeleton and dependency set
- Priority: P0
- Effort: S
- Depends: none
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt
- Acceptance:
  - `src/source/` exists with the files listed in p1-rust-design.md section 2 (`mod.rs`, `model.rs`, `filter.rs`, `video.rs`, `hoster.rs`, `thumbnail.rs`, `traits.rs`, `http.rs`, `scrape.rs`, `prefs.rs`, `codec.rs`, `http_server.rs`, `torrent.rs`, `registry.rs`) and `src/main.rs` declares `mod source;`.
  - `cargo build` succeeds with the crates listed in p1-rust-design.md section 1 added to `Cargo.toml`; no crate outside that list is added for this area.
  - No module outside `src/source/` imports `reqwest` or `scraper` directly (checked with `rg "use (reqwest|scraper)" src` returning only `src/source/`).

### T-16a-002 — Source identity: id generation, version id, display name
- Priority: P0
- Effort: S
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - `generate_id("Example","en",1) == 4851548778462321757` and `generate_id("Aniyomi Test","ja",2) == 7915262807009061958` (p1-topic-url-and-id-compat.md section 1).
  - Result is never negative; name is lowercased before hashing (`generate_id("EXAMPLE","en",1) == generate_id("example","en",1)`).
  - `display_name("Example","en")` is `Example (EN)`; `version_id` defaults to 1 when a source does not set it.

### T-16a-003 — AnimeSource async trait (listing, search, filters, supportsLatest)
- Priority: P0
- Effort: M
- Depends: T-16a-001, T-16a-011, T-16a-012, T-16a-013
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeCatalogueSource.kt
- Acceptance:
  - Trait `AnimeSource` declares `id`, `name`, `lang` (default `""`), `supports_latest` (required, no default), `filter_list` (default empty), `popular`, `latest`, `search` as `async` methods returning `Result<AnimesPage, SourceError>`.
  - A mock source compiles with only the required methods; omitting `supports_latest` is a compile error.
  - Calling `popular(page)` on a mock returns the page unchanged (identity on the trait's default path).

### T-16a-004 — Combined episode and season update defaults (supervisor semantics)
- Priority: P0
- Effort: S
- Depends: T-16a-003, T-16a-008, T-16a-052
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeCatalogueSource.kt
- Acceptance:
  - `anime_episode_update(anime, episodes, false, false)` returns the same `anime` and `episodes` values with no source call (verified by a call counter of 0).
  - With `fetch_details = true` and `fetch_episodes = true`, both calls run concurrently (test: two 500 ms delays finish in under 900 ms total).
  - A failure in either child fails the whole call (E-26); the failure is returned, not swallowed.
  - `supports_details()` and `supports_episode_list()` default to `true`; the default update path skips a call whose capability is `false` (E-27, E-28).

### T-16a-005 — Hoster versus legacy video routing capability
- Priority: P0
- Effort: S
- Depends: T-16a-003, T-16a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt
- Acceptance:
  - `provides_hosters()` is an explicit trait method; no reflection is used anywhere in `src/`.
  - A mock source with `provides_hosters() == false` yields exactly one hoster named `no_hoster_list` containing the videos in source order (test 10 in p1-topic-video-pipeline.md section 10).
  - A Parsed-helper source with a configured hoster selector reports `provides_hosters() == true` (fixes E-01).

### T-16a-006 — SAnime model, status values, genres, copy
- Priority: P0
- Effort: M
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnime.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnimeImpl.kt
- Acceptance:
  - `SAnime::default()` has `status = Unknown (0)`, `update_strategy = AlwaysUpdate`, `fetch_type = Episodes`, `season_number = -1.0`, `initialized = false`, empty memo (p1-data-and-config.md section 11).
  - `AnimeStatus::from_i32` and `to_i32` round-trip 0..7; 99 becomes `Other(99)` and back to 99.
  - `genres("Action, Drama, Action,  ,Comedy")` is `["Action","Drama","Comedy"]`; `"a,b"` is `["a,b"]`; `"   "` and `None` give `None` (behavior 4.10).

### T-16a-007 — SEpisode model, defaults, copy-from
- Priority: P0
- Effort: S
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SEpisode.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SEpisodeImpl.kt
- Acceptance:
  - Defaults: `episode_number == -1.0f32`, `fillermark == false`, `date_upload == 0`, `memo` empty.
  - `copy_from(other)` copies exactly the nine fields listed in behavior 4.12 and nothing else.
  - `episode_number` survives a SQLite REAL round trip for all values in `[-1.0, 10000.0]` in 0.5 steps (exact f32 round trip).

### T-16a-008 — Combined update result carriers
- Priority: P0
- Effort: S
- Depends: T-16a-006, T-16a-007
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnimeEpisodeUpdate.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/SAnimeSeasonUpdate.kt
- Acceptance:
  - `SAnimeEpisodeUpdate { anime, episodes }` and `SAnimeSeasonUpdate { anime, seasons }` exist with no defaults and are `Clone`.
  - The app's update path (area:07-domain-entries-items and area:26-library-update-jobs) consumes them without conversion; a compile-time test constructs each.

### T-16a-009 — FetchType routing contract
- Priority: P0
- Effort: S
- Depends: T-16a-006
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/FetchType.kt
- Acceptance:
  - `FetchType` has exactly two variants `Seasons = 0` and `Episodes = 1` with `Episodes` as default.
  - Stored value is the ordinal; an unknown stored value decodes to `Episodes` (matches `FetchTypeColumnAdapter`, `data/src/main/java/tachiyomi/data/DatabaseAdapter.kt:43-47`).
  - A season-typed entry never calls the episode path and an episode-typed entry never calls the season path (test with call counters).

### T-16a-010 — UpdateStrategy (anime and manga) and library-update skip rule
- Priority: P0
- Effort: S
- Depends: T-16a-006
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeUpdateStrategy.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt
- Acceptance:
  - `AlwaysUpdate = 0` and `OnlyFetchOnce = 1`; default `AlwaysUpdate`; unknown stored value decodes to `AlwaysUpdate` (`DatabaseAdapter.kt:36-41`).
  - Helper `should_skip_in_library_update(strategy, has_only_one_episode_known)` is not added here; the decision is exported as `UpdateStrategy::skips_library_update()` returning `true` only for `OnlyFetchOnce`.

### T-16a-011 — Filter model (shared shape for anime and manga)
- Priority: P0
- Effort: M
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Filter.kt
- Acceptance:
  - Enum variants `Header`, `Separator`, `Select`, `Text`, `CheckBox`, `TriState`, `Group`, `Sort` with the defaults in p1-data-and-config.md section 2.
  - `TriStateValue` discriminants are `Ignore=0`, `Include=1`, `Exclude=2`.
  - `Select` with an out-of-range index is clamped to 0 and logs a warning (E-14); `Header{name:"A"} != Select{name:"A", values:["x"], index:0}` (E-13).

### T-16a-012 — FilterList container semantics
- Priority: P0
- Effort: S
- Depends: T-16a-011
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeFilterList.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/FilterList.kt
- Acceptance:
  - Order is preserved; `FilterList::default()` is empty.
  - Two lists with equal contents compare equal (deliberate deviation from the always-false Kotlin `equals`, E-12); a list compares equal to itself.
  - Lists are shareable as `Arc<FilterList>` without copying.

### T-16a-013 — AnimesPage listing result
- Priority: P0
- Effort: S
- Depends: T-16a-006
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimesPage.kt
- Acceptance:
  - `AnimesPage { animes, has_next_page }` with no default; deprecated Kotlin component and copy helpers are not ported (drop note in p1-inventory.md).
  - An empty page with `has_next_page = true` is representable and passes through unchanged.

### T-16a-014 — Track, ChapterType, TimeStamp models
- Priority: P0
- Effort: S
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt
- Acceptance:
  - `ChapterType` variants `Opening, Ending, Recap, MixedOp, Other` (default `Other`).
  - `TimeStamp::new(start, end, name, kind)` rejects non-finite values and `end < start` by returning `None` and logging (E-25).
  - `Track { url, lang }` has no defaults.

### T-16a-015 — Video model: fields, defaults, equality, legacy drops
- Priority: P0
- Effort: M
- Depends: T-16a-014
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt
- Acceptance:
  - `Video` has the 15 persisted fields with the defaults in p1-data-and-config.md section 1.8 and no runtime `state` field (state lives in `VideoEntry`).
  - Equality compares all persisted fields; two videos differing only in runtime state compare equal.
  - Legacy members are not ported: `quality`, `url`/`videoPageUrl`, ext-lib-15/16 constructors and the deprecated `copy`, and the `"null"` URL sentinel (E-22). Drop reason and user loss recorded in p1-inventory.md (native sources only; APK extensions are not loaded, see area:19-extensions-anime and area:20-extension-strategy-redesign).
  - `headers: None` and `headers: Some(Headers(vec![]))` are distinct values (E-23).

### T-16a-016 — Local-server placeholder detection and rewrite
- Priority: P1
- Effort: M
- Depends: T-16a-015, T-16a-033
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/HttpServer.kt
- Acceptance:
  - `is_placeholder` matches `http://localhost:1` followed by end or a non-digit; does not match `http://localhost:12/x` or `https://localhost:1/x` (test 26 in p1-edge-cases-and-tests.md).
  - `uses_local_server()` is true when the placeholder appears in `video_url` or any audio or subtitle track URL (test 27).
  - `rewrite_to_local(0)` returns `SourceError::LocalServer` (E-08); a valid port rewrites every placeholder and keeps `memo`, `headers`, `timestamps` unchanged (E-07).

### T-16a-017 — Hoster model and synthetic hoster wrap
- Priority: P0
- Effort: S
- Depends: T-16a-015
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt
- Acceptance:
  - `Hoster` fields and defaults per p1-data-and-config.md section 1.7; `lazy` defaults to `false`.
  - `Hoster::synthetic(videos)` has `hoster_name == "no_hoster_list"`, empty `hoster_url`, `video_list == Some(videos)`, `lazy == false`.
  - `HosterState` variants `Idle`, `Loading`, `Ready`, `Error` exist; `Ready` carries `Vec<VideoEntry>`.

### T-16a-018 — Video and hoster serialized codec (Aniyomi wire format)
- Priority: P2
- Effort: M
- Depends: T-16a-015, T-16a-017, T-16a-047
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Video.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/Hoster.kt
- Acceptance:
  - Reading a serialized `SerializableVideo` list with an unknown extra key succeeds (superset tolerance).
  - `headers` encodes as an array of two-element arrays `[["k","v"]]` and decodes back to the same ordered list (test 39).
  - `SerializableHoster.internalData` is required on read (missing key is an error) and `videoList` is a JSON string containing an array (p1-data-and-config.md section 5).
  - Round trip of a list containing one hoster with two videos is byte-equal after decode and re-encode of the same field set.

### T-16a-019 — Hoster discovery for HTTP, downloaded and local episodes
- Priority: P0
- Effort: M
- Depends: T-16a-005, T-16a-017
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt
- Acceptance:
  - `http_hosters` calls `hosters()` then `sort_hosters()` when `provides_hosters()`, else `videos_for_episode()`, `sort_videos()` and wraps it synthetically (p1-topic-video-pipeline.md section 2).
  - A downloaded episode yields one hoster and makes no source call; any error yields an empty list (not an error).
  - An episode URL `dir/file.mkv` with the local source resolves to one hoster named as `Local source: dir/file.mkv`; a missing file yields an empty list.

### T-16a-020 — Hoster load, video fetch and sort order
- Priority: P0
- Effort: M
- Depends: T-16a-019, T-16a-024
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt
- Acceptance:
  - A lazy hoster with `force = false` returns `Idle` without any source call; with `force = true` it loads.
  - A hoster with `video_list == Some(v)` uses `v` without calling `videos_for_hoster`; with `None` it calls it exactly once.
  - Videos are sorted with `sort_videos` in every case; each video starts as `Queue`; an error other than cancellation becomes `HosterState::Error` (topic video pipeline test 11).

### T-16a-021 — Preferred-then-first-URL selection (selectBestVideo)
- Priority: P0
- Effort: S
- Depends: T-16a-020
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt
- Acceptance:
  - The worked example in p1-topic-video-pipeline.md section 4 returns `(0, 1)`; with the preferred video in `Queue` it returns `(1, 0)`.
  - Videos in `Error` or `LoadVideo` state are never returned; no ready hosters returns `None`.
  - Tests 1 to 3 of p1-topic-video-pipeline.md section 10 and tests 20 to 23 of p1-edge-cases-and-tests.md section G pass.

### T-16a-022 — Best-video search with parallel preferred probing and early return
- Priority: P0
- Effort: L
- Depends: T-16a-021, T-16a-023
- Sources: app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt
- Acceptance:
  - Phase A runs one task per hoster and returns the first resolved preferred video; the remaining tasks are aborted (`JoinSet::abort_all`), verified by a test where the slow hoster's completion counter stays 0 (test 31).
  - Phase B loops `select_best`, marking each failed candidate `Error`, and returns `None` when none remain (tests 32, 33).
  - Hoster states are owned by a single collector (no shared mutable vector); `Cancelled` from any task propagates and aborts the call (test 29).

### T-16a-023 — Video resolution and the initialized flag
- Priority: P0
- Effort: S
- Depends: T-16a-003, T-16a-015
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/HosterLoader.kt
- Acceptance:
  - `resolve_video` default returns the input unchanged; an error other than `Cancelled` becomes `None` (test 28 for empty URL, plus error to `None`).
  - A video with `initialized == true` is not sent to `resolve_video` (test 30).
  - Successful resolution sets `initialized = true` on the returned value; cancellation is returned as `Err(Cancelled)`.

### T-16a-024 — sortHosters and sortVideos contract
- Priority: P0
- Effort: S
- Depends: T-16a-003
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt
- Acceptance:
  - Default `sort_hosters` and `sort_videos` return their input order unchanged (verified with a three-element input).
  - A source override is applied before selection in every path (http-hosters, http-legacy, hoster with explicit list).
  - No global quality sort exists in the Rust code for anime videos.

### T-16a-025 — Request defaults: User-Agent header and Cache-Control max-age
- Priority: P0
- Effort: S
- Depends: T-16a-002, T-16a-026
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt
- Acceptance:
  - `headers()` returns exactly one pair `User-Agent` with the trimmed `default_user_agent` value (default `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0`).
  - Every `get_request` sends `Cache-Control: max-age=600` (test 5).
  - `headers()` is computed once per source instance (E-33); a second call returns the same allocation.

### T-16a-026 — Request execution: non-2xx as error, cancellation, IO errors
- Priority: P0
- Effort: M
- Depends: T-16a-001
- Sources: core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - `HttpBase::fetch` returns `SourceError::Http{status}` for 404 and 500, and `Ok` for 200 and 206 (test 6).
  - Dropping the future cancels the in-flight request (test: server sees connection closed within 1 s).
  - Transport errors map to `SourceError::Network`; `fetch_text` returns the final URL after redirects (used as Jsoup base URI).

### T-16a-027 — Relative URL storage (setUrlWithoutDomain) with Java URI parity
- Priority: P0
- Effort: M
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt
- Acceptance:
  - Every row of the table in p1-topic-url-and-id-compat.md section 3 (ANI column) passes as a unit test in `url_without_domain`.
  - Rejected inputs return the original string including the domain (E-06, E-20); opaque URLs return the original string (E-19 fix).
  - Decoded path, query and fragment: `%26` yields `&`, `%20` yields a space, `%2F` yields `/` (E-21).

### T-16a-028 — Source URL helpers (getAnimeUrl, getEpisodeUrl) and prepareNewEpisode drop
- Priority: P1
- Effort: S
- Depends: T-16a-027, T-16a-025
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt
- Acceptance:
  - `anime_url(anime)` returns the absolute `base + anime.url` as a normalised URL; `episode_url(episode)` returns `episode.url` unchanged (E-23 consumer note: callers must not mix forms).
  - Deprecated `prepareNewEpisode` hook is not ported; the drop is recorded in p1-inventory.md with reason (the app applies fields at construction, behavior 4.5).
  - An invalid base URL yields `SourceError::BadUrl`, never a panic.

### T-16a-029 — Parsed listing helper (popular, search, latest, hasNextPage)
- Priority: P1
- Effort: M
- Depends: T-16a-013, T-16a-031
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ParsedAnimeHttpSource.kt
- Acceptance:
  - `parse_listing` keeps document order and does no dedupe (behavior 4.2).
  - `has_next_page` is `false` for a `None` selector and `true` when the selector matches at least one element.
  - A selector that fails to parse returns `SourceError::Parse`, never an empty list (E-05; test 15).

### T-16a-030 — Parsed details, episode, season, hoster, video and video-URL helpers
- Priority: P1
- Effort: L
- Depends: T-16a-029, T-16a-005
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ParsedAnimeHttpSource.kt
- Acceptance:
  - Episode, season, hoster and video lists keep document order; each uses the base URI of the final response URL for relative links.
  - A Parsed source that configures only hoster selectors reports `provides_hosters() == true` (fixes E-01).
  - Recorded fixture HTML for episode list returns episodes in document order (test scrape_episode_list_document_order).

### T-16a-031 — Jsoup-equivalent helpers (selectText, selectInt, attrOrText, asJsoup)
- Priority: P1
- Effort: S
- Depends: T-16a-026
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsoupExtensions.kt
- Acceptance:
  - `select_int` returns the default for a missing element, parses `"12"` to 12, and returns `Err(Parse)` for `""` or non-numeric text (test 16).
  - `select_text` returns `Some("")` for an empty matched element and the default only for a missing element (test 17).
  - `attr_or_text("text")` returns text content; any other key returns the attribute, or `""` when absent (test 18).

### T-16a-032 — Thumbnail (seek preview) model, tile sorting and preload
- Priority: P1
- Effort: M
- Depends: T-16a-023, T-16a-054
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/ThumbnailInfo.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt
- Acceptance:
  - `ThumbnailInfo { tile_info, image_tile_urls }` and `TileInfo { image_index, time_ms (i64), x, y, width, height }` match p1-data-and-config.md section 1.10.
  - Tiles are sorted ascending by `time_ms` before use; only the first two tile images are preloaded (behavior 9.2).
  - Image tile fetch is async with status check and returns bytes; it never blocks the GTK thread (E-09; test: a 404 tile returns `None`).

### T-16a-033 — Local stream server (HttpServer replacement) with Range support
- Priority: P1
- Effort: M
- Depends: T-16a-001, T-16a-026
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/HttpServer.kt, app/src/main/java/eu/kanade/tachiyomi/data/player/service/HttpServerService.kt
- Acceptance:
  - `LocalStreamServer::start(handler)` binds `127.0.0.1:0`, returns a non-zero port, and `url()` equals `http://localhost:<port>` (test 42).
  - A `Range: bytes=100-199` request yields 206, `Content-Range: bytes 100-199/<total>` and 100 bytes (E-38).
  - A bind failure returns `SourceError::LocalServer`, shown with `AYMR.strings.http_server_start_failure` (E-37); `stop()` completes within 2 s (test: stop while a download reads).

### T-16a-034 — Per-source preference namespace and key naming
- Priority: P0
- Effort: M
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/utils/Preferences.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/ConfigurableAnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt
- Acceptance:
  - The namespace key for a source is `source_<decimal id>` (e.g. `source_4851548778462321757`).
  - Reading an unset key returns the caller's default; writing a value persists it in the settings store (area:02-preferences-system) under `scope = "source"`.
  - Reads and writes never create a row for a value that was never written (store-level rule owned by area:02-preferences-system; the test asserts no row after a read).

### T-16a-035 — Source settings descriptors (replaces setupPreferenceScreen and androidx types)
- Priority: P1
- Effort: M
- Depends: T-16a-034, T-16a-003
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/ConfigurableAnimeSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/ConfigurableSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/PreferenceScreen.kt
- Acceptance:
  - `ConfigurableSource::preferences()` returns `Vec<SourcePref>` with variants `Switch`, `List`, `MultiSelect`, `EditText`, `Category`, each with `key`, `title`, `summary` and `default`.
  - The androidx `PreferenceScreen` is not referenced in Rust code (both `expect` classes drop).
  - A `Category` nests children; a descriptor with a duplicate key within one source is rejected at construction.

### T-16a-036 — Deep-link resolution for anime URIs
- Priority: P1
- Effort: S
- Depends: T-16a-003
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ResolvableAnimeSource.kt, app/src/main/java/eu/kanade/tachiyomi/ui/deeplink/anime/DeepLinkAnimeScreenModel.kt
- Acceptance:
  - `uri_type` returns `Anime`, `Episode` or `Unknown`; `anime_for_uri` and `episode_for_uri` return `Option` and are only called for the matching type (test with call counters).
  - A URI the source does not recognise yields `Unknown` and no network call.

### T-16a-037 — UnmeteredSource marker and its scheduling effects
- Priority: P1
- Effort: S
- Depends: T-16a-003
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/UnmeteredSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/UnmeteredSource.kt, app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt, app/src/main/java/eu/kanade/tachiyomi/data/library/anime/AnimeLibraryUpdateNotifier.kt, app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt
- Acceptance:
  - `UnmeteredSource` is a marker trait; a source implementing it is excluded from the per-source queue warning counts (threshold 10 episodes, 60 library updates, 20 queued downloads).
  - Unmetered sources fetch seasons in parallel; others run sequentially with a cancellation check before each (test with timing).
  - The rate limiter (area:12-network-stack) skips a source that implements the marker (cross-area test).

### T-16a-038 — Runtime source factories dropped; builtin registry
- Priority: P2
- Effort: S
- Depends: T-16a-003
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSourceFactory.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/SourceFactory.kt
- Acceptance:
  - `registry.rs` exposes `fn builtin_anime_sources() -> Vec<Arc<dyn AnimeSource>>` as the only source of sources in this build.
  - No APK, classloader or `createSources` factory is implemented; the drop with user impact (existing Aniyomi extensions cannot be loaded) is recorded in p1-inventory.md and the mechanism decision lives in area:20-extension-strategy-redesign.
  - Builtin list is empty by default and extended only by adding a module (test: list length equals the number of registered modules).

### T-16a-039 — Torrent file model and magnet builder
- Priority: P2
- Effort: S
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/model/TorrentFile.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/model/TorrentInfo.kt
- Acceptance:
  - `TorrentFile::magnet_uri()` for hash `abc`, one tracker `udp://tracker.example:80/announce?x=a b`, index 2 equals `magnet:?xt=urn:btih:abc&tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce%3Fx%3Da+b&index=2`.
  - With no trackers the output has no `&tr=`.
  - `TorrentInfo::set_trackers` returns a copy with the new list; `DeadTorrentException` is an error variant.

### T-16a-040 — Torrent info resolution: magnet versus .torrent file, title and error mapping
- Priority: P2
- Effort: M
- Depends: T-16a-039, T-16a-026, T-16a-041
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/torrentutils/TorrentUtils.kt
- Acceptance:
  - Magnet detection is case-insensitive (`MAGNET:?xt=urn:btih:abc` routes to magnet; test 34) (E-17).
  - Empty title falls back to the torrent's own title (test 35) (E-15).
  - Missing hash is `SourceError::Parse` (test 36) (E-16); connection refused maps to "TorrServer is not running", timeouts to dead torrent, other HTTP and parse errors keep their own messages (E-18).
  - Parsing the two upstream fixtures yields the expected hash, sizes and trackers listed in p1-edge-cases-and-tests.md section F.

### T-16a-041 — Torrent server backend (TorrServer HTTP API client or librqbit)
- Priority: P2
- Effort: L
- Depends: T-16a-033, T-16a-026
- Sources: core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerApi.kt, core/common/src/main/java/aniyomi/core/common/torrent/model/TorrentRequest.kt, core/common/src/main/java/aniyomi/core/common/torrent/model/Torrent.kt, core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt
- Acceptance:
  - Proposal decision recorded: either a TorrServer-compatible client (POST `http://127.0.0.1:<pref_torrserver_port>/torrents` with the JSON body in p1-data-and-config.md section 8.2, default port 8090) or an embedded `librqbit` backend; the chosen option is named in p1-rust-design.md before implementation.
  - `add_magnet` returns a `Torrent` whose JSON fields `torrent_size`, `file_stats`, `hash`, `trackers` are read with the serde names in section 4 of p1-data-and-config.md.
  - With the backend disabled (`pref_torrserver_enable == false`) the player never calls it (test with call counter).

### T-16a-042 — Manga-side source interfaces (MangaSource, CatalogueSource, ResolvableSource skeletons)
- Priority: P0
- Effort: M
- Depends: T-16a-001, T-16a-043, T-16a-011
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/MangaSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/CatalogueSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - Traits `MangaSource` (`get_manga_details`, `get_chapter_list`, `get_page_list`) and `CatalogueSource` (`supports_latest`, `popular`, `search`, `latest`, `filter_list`) compile with the same async conventions as the anime trait.
  - Manga semantics (URL rules, page download) are owned by area:15-source-api-manga; this task only creates the trait shape and the id and header helpers shared with `HttpBase`.
  - Deprecated Rx methods are not ported (drop recorded in p1-inventory.md).

### T-16a-043 — Manga models (SManga, SChapter, Page, MangasPage, UpdateStrategy)
- Priority: P0
- Effort: M
- Depends: T-16a-001, T-16a-010
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SManga.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SMangaImpl.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapter.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/SChapterImpl.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/MangasPage.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/UpdateStrategy.kt
- Acceptance:
  - `SManga` status constants are `0..6` with no `Upcoming`; `SChapter` defaults `chapter_number = -1.0`, `date_upload = 0`.
  - `Page` state variants `Queue, LoadPage, DownloadImage, Ready, Error` in that order (ordinals 0..4).
  - `MangasPage { mangas, has_next_page }` has no defaults; `UpdateStrategy` matches the anime enum values.

### T-16a-044 — Manga HTTP URL rules (space replacement) and parity with anime
- Priority: P1
- Effort: S
- Depends: T-16a-027, T-16a-042
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - Manga `url_without_domain` replaces spaces with `%20` before parsing: `https://x.org/a b/c` gives `/a b/c` (vector table, MANG column).
  - Shared code path with the anime helper; the only difference is the space replacement.
  - Rejected inputs still return the original string.

### T-16a-045 — Manga deep-link resolution (ResolvableSource)
- Priority: P1
- Effort: S
- Depends: T-16a-042
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ResolvableSource.kt
- Acceptance:
  - `uri_type` returns `Manga`, `Chapter` or `Unknown`; the lookup functions run only for the matching type.
  - Same test shape as T-16a-036.

### T-16a-046 — Related anime list (AnimeRelation and getRelatedAnimeList contract)
- Priority: P1
- Effort: S
- Depends: T-16a-003, T-16a-006
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/model/AnimeRelation.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt
- Acceptance:
  - `AnimeRelation { name, animes }` exists; `related_anime` is only called when `supports_related_anime()` is true (default false).
  - The default `related_anime` returns `NotSupported`; the Kotlin `Exception("Stub!")` is not reproduced.
  - Relation groups keep the source order of groups and of animes within each group.

### T-16a-047 — Shared JSON configuration (defaultJson) mapped to serde
- Priority: P0
- Effort: S
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/JsonExtensions.kt
- Acceptance:
  - `codec.rs` provides one `serde_json` configuration: unknown keys ignored on read, `None` fields omitted on write, nulls treated as absent on read.
  - A source response with an unknown key still parses into the model struct.
  - Test: round trip of a `SAnime` with `memo` containing a nested object keeps every key.

### T-16a-048 — RxJava bridge (awaitSingle) dropped
- Priority: P3
- Effort: S
- Depends: T-16a-003
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt
- Acceptance:
  - No RxJava dependency exists in `Cargo.toml`; `grep -ri rx src/` finds nothing in `src/source/`.
  - The drop is recorded in p1-inventory.md: RxJava is an Android-era bridge with no user-visible behavior; async Rust replaces it with no loss.

### T-16a-049 — Unused util Video stub dropped
- Priority: P3
- Effort: S
- Depends: T-16a-001
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/util/VideoInfo.kt
- Acceptance:
  - No `VideoUrl` or util-package `Video` type exists in Rust; `grep -rn "VideoUrl" src/` is empty.
  - Drop reason recorded: no consumer in app, domain or data (grep result in p1-inventory.md); no user loss.

### T-16a-050 — Android actuals (PreferenceScreen typealias, awaitSingle actual) and manifest dropped
- Priority: P3
- Effort: S
- Depends: T-16a-035
- Sources: source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/animesource/PreferenceScreen.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/source/PreferenceScreen.kt, source-api/src/androidMain/kotlin/eu/kanade/tachiyomi/util/RxExtension.kt, source-api/src/androidMain/AndroidManifest.xml
- Acceptance:
  - No `androidx.preference` type is referenced by the Rust project.
  - `source-api/src/androidMain/AndroidManifest.xml` has no Linux equivalent (no manifest exists in Kurobayomi); recorded as a P3 drop.
  - The Linux settings screen is generated from `SourcePref` (T-16a-035); nothing is lost for users because Kotlin's androidx screen is replaced one to one.

### T-16a-051 — Test corpus for this scope
- Priority: P0
- Effort: M
- Depends: T-16a-002, T-16a-011, T-16a-015, T-16a-021, T-16a-022, T-16a-027
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, core/common/src/test/java/aniyomi/core/common/torrent/TorrentHelpersTest.kt
- Acceptance:
  - All tests numbered 1 to 45 in p1-edge-cases-and-tests.md section G compile and run under `cargo test` with no network access.
  - Fixtures in section H exist under `tests/fixtures/` (source id vectors, URL vectors TSV, two torrent files copied from upstream resources, two HTML fixtures).
  - `cargo test -p kurobayomi source::` passes in CI and the URL vector TSV is regenerated by the Java generator with zero diff.

### T-16a-052 — Remove self-recursion in default hoster and video list overrides
- Priority: P0
- Effort: S
- Depends: T-16a-003, T-16a-017
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeCatalogueSource.kt
- Acceptance:
  - In Rust, `hosters` and `videos_for_hoster` have no default body that calls themselves; defaults return `NotSupported` (E-02).
  - A mock catalogue source that implements neither method returns `NotSupported` on the first call instead of overflowing the stack (test with a 1000-call loop).
  - Documented in p1-behavior.md section 3.3 and closed as a Fix.

### T-16a-053 — Deprecated Rx and request/parse hooks dropped
- Priority: P3
- Effort: S
- Depends: T-16a-003, T-16a-029, T-16a-030
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/ParsedAnimeHttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/AnimeSource.kt
- Acceptance:
  - The list of dropped members (all `fetch*` Rx methods, deprecated `*Request`/`*Parse` helpers, `prepareNewEpisode`, `episodeVideoParse`, legacy `getSeasonList`/`getVideoList(episode)` entry points except the one used by the legacy routing path in T-16a-005) appears in p1-inventory.md with reason and user impact.
  - No Rust trait method is named after a deprecated Kotlin hook.
  - Extensions built against this ABI cannot load (user impact recorded: none for builtin sources).

### T-16a-054 — Image fetch with progress and cache control (manga page download)
- Priority: P1
- Effort: M
- Depends: T-16a-026, T-16a-043
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt, source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/model/Page.kt, core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt
- Acceptance:
  - Image fetch sends the request with caching disabled and reports `progress = 100 * read / length` as an integer, or `-1` when length is unknown (behavior 17).
  - Progress is published through a `tokio::sync::watch` channel; the reader sees values in order.
  - The per-call timeout for progress-reported requests is 30 hours (matches `callTimeout(30, HOURS)`), other requests keep the client default.

### T-16a-055 — Parsed manga listing and detail helpers (ParsedHttpSource parity)
- Priority: P1
- Effort: M
- Depends: T-16a-029, T-16a-031, T-16a-042
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/ParsedHttpSource.kt
- Acceptance:
  - Popular, search and latest listing helpers reuse `parse_listing` (T-16a-029) with manga types.
  - Chapter list, page list and image URL helpers keep document order and resolve relative URLs against the final response URL.
  - A selector that fails to parse returns `SourceError::Parse`.

### T-16a-056 — Chapter-level and page-level manga sources share the same error policy
- Priority: P3
- Effort: S
- Depends: T-16a-042, T-16a-026
- Sources: source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt
- Acceptance:
  - Manga `fetch_search` (Rx `Observable.defer` wrapper that rethrew `NoClassDefFoundError` as `RuntimeException`) is dropped: Rust has no classloader and no equivalent error; recorded in p1-inventory.md.
  - Search errors return `SourceError` values with no panic path (test: a 500 from search yields `Http{500}`).
