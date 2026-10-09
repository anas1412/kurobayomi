# 16-source-api-anime: overview

Area id: `16-source-api-anime`. Kotlin root: `/home/blackbox/aniyomi-src` (all paths repo-relative to it). Rust target: `/home/blackbox/kurobayomi`. Parts: 1 of 1.

---

## 1. Purpose and scope

**What Aniyomi does here.** The `source-api` module defines the contract that every anime extension implements: listing (popular, latest, search with filters), entry details, episode and season lists, the combined update call, the hoster-then-video pipeline that turns an episode into a playable stream URL, lazy stream resolution, thumbnails for the seek bar, deep-link resolution, and per-source preferences. The HTTP base class (`AnimeHttpSource`) and the Jsoup base class (`ParsedAnimeHttpSource`) supply defaults so that an extension only writes request and parse code.

**Scope of this area.** 51 files, 3573 LOC (see [p1-inventory.md](p1-inventory.md)):
- 26 files under `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/` (6 root, 16 under `model/`, 3 under `online/`, 1 under `utils/`).
- 18 manga-side files under `.../source/` (overlaps area:15-source-api-manga; this area documents them only where they share code with the anime API).
- 3 files under `.../torrentutils/` and 4 under `.../util/`.

**Why it matters for a usable Kurobayomi.** Kurobayomi cannot play an anime without a source that produces episodes and direct stream URLs. Every anime playback and every anime download goes through the path described in [p1-topic-video-pipeline.md](p1-topic-video-pipeline.md). Stored identifiers (source id, relative entry URL) also must match what Aniyomi wrote, otherwise backups and existing rows stop matching their source ([p1-topic-url-and-id-compat.md](p1-topic-url-and-id-compat.md)).

**Out of scope here.** Player UI (area:45-player-core, area:46-player-controls-ui), anime downloader (area:24-downloads-anime), APK extension loading and trust (area:19-extensions-anime, area:20-extension-strategy-redesign), HTTP client internals (area:12-network-stack), entry and season screens (area:39-ui-entry-details-anime). Note: [p1-behavior.md](p1-behavior.md) section 16.1 and the scope reminder at the top of [p1-rust-design.md](p1-rust-design.md) point the player UI at "area:39" and "area:36"; that is a wrong pointer (see section 4.4).

**Important gap.** Task T-16a-038 makes the builtin registry empty by default. This area therefore delivers zero working sources by itself. See open question Q1 in section 4.

---

## 2. Module map

### 2.1 Kotlin packages to responsibilities

| Package (under `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/`) | Files | Responsibility |
|---|---|---|
| `animesource/` | 6 | Contracts: `AnimeSource` (root, ext-lib 17), `AnimeCatalogueSource` (listing, search, combined update), `AnimeSourceFactory` (APK-only), `ConfigurableAnimeSource` (settings), `UnmeteredSource` (marker), `PreferenceScreen` (expect). |
| `animesource/model/` | 16 | Data: `SAnime`/`SAnimeImpl`, `SEpisode`/`SEpisodeImpl`, `Video`, `Hoster`, `HttpServer`, `ThumbnailInfo`, `AnimeFilter`, `AnimeFilterList`, `FetchType`, `AnimeUpdateStrategy`, `AnimesPage`, `AnimeRelation`, `SAnimeEpisodeUpdate`, `SAnimeSeasonUpdate`. |
| `animesource/online/` | 3 | Base classes: `AnimeHttpSource` (734 LOC, request/parse defaults, hoster and video pipeline hooks, image tiles), `ParsedAnimeHttpSource` (292 LOC, Jsoup selectors), `ResolvableAnimeSource` (deep links). |
| `animesource/utils/` | 1 | `Preferences.kt`: key naming `source_<id>` and store access. |
| `source/` | 18 | Manga twin: `MangaSource`, `CatalogueSource`, `ConfigurableSource`, `SourceFactory`, `UnmeteredSource`, `PreferenceScreen`; `model/` (Filter, FilterList, MangasPage, Page, SChapter(+Impl), SManga(+Impl), UpdateStrategy); `online/` (`HttpSource` 461 LOC, `ParsedHttpSource` 201 LOC, `ResolvableSource`). Owned semantically by area:15-source-api-manga. |
| `torrentutils/` | 3 | `TorrentUtils` (magnet or `.torrent` to file list and title), `model/TorrentFile`, `model/TorrentInfo`. |
| `util/` | 4 | `JsonExtensions` (`defaultJson`), `JsoupExtensions` (`selectText`, `selectInt`, `attrOrText`, `asJsoup`), `RxExtension` (expect `awaitSingle`), `VideoInfo` (unused `sealed class Video` stub). |
| `androidMain` actuals (outside the scope folder, read for exactness) | 3 | `PreferenceScreen` typealiases to `androidx.preference.PreferenceScreen`; `awaitSingle` actual. |

Consumers outside this folder that define observable behavior are listed in [p1-inventory.md](p1-inventory.md) (section "Consumers outside this scope"): `app/src/main/java/eu/kanade/tachiyomi/ui/player/loader/EpisodeLoader.kt` and `HosterLoader.kt` (routing, selection, resolution), `app/src/main/java/eu/kanade/tachiyomi/data/download/anime/AnimeDownloader.kt` (ffmpeg, local server, queue thresholds), `app/src/main/java/mihon/domain/source/interactor/UpdateAnimeFromRemote.kt` (combined update).

### 2.2 Proposed Rust module map

Defined in [p1-rust-design.md](p1-rust-design.md) sections 2 to 5. Only `src/source/` imports `reqwest` or `scraper`; the GTK code never does.

| Rust path | Replaces (Kotlin) | Notes |
|---|---|---|
| `src/source/mod.rs` | error types | `SourceError { Http, Network, Parse, NotSupported, BadUrl, LocalServer, Cancelled }`. |
| `src/source/model.rs` | `SAnime`, `SEpisode`, `AnimesPage`, `FetchType`, `UpdateStrategy`, `AnimeStatus`, update carriers | `AnimeStatus` keeps integers 0 to 7 plus `Other(i32)`. |
| `src/source/filter.rs` | `AnimeFilter`, `AnimeFilterList`, manga `Filter` | Enum with explicit variants, value equality. |
| `src/source/video.rs` | `Video`, `Track`, `TimeStamp`, `ChapterType` | `VideoEntry { video, state }` carries runtime state. |
| `src/source/hoster.rs` | `Hoster`, `Hoster.State`, `HosterState` | `NO_HOSTER_LIST = "no_hoster_list"`. |
| `src/source/thumbnail.rs` | `ThumbnailInfo`, `TileInfo` | |
| `src/source/traits.rs` | `AnimeSource`, `AnimeCatalogueSource`, `ConfigurableSource`, `ResolvableAnimeSource`, `UnmeteredSource`, `LocalHandler` | `async_trait`. Defaults for `hosters` and `videos_for_hoster` return `NotSupported`. |
| `src/source/http.rs` | `AnimeHttpSource` defaults, `generateId`, `getUrlWithoutDomain` | `HttpBase` struct with `OnceLock` headers. |
| `src/source/scrape.rs` | `ParsedAnimeHttpSource`, `JsoupExtensions` | `scraper` crate. |
| `src/source/prefs.rs` | `Preferences.kt`, `ConfigurableAnimeSource` descriptors | `SourcePref` enum. |
| `src/source/codec.rs` | `defaultJson`, `SerializableVideo`, `SerializableHoster` | serde; P2 for the wire codec. |
| `src/source/http_server.rs` | `HttpServer` (NanoHTTPD), `HttpServerService` | `LocalStreamServer` on `127.0.0.1:0` with axum. |
| `src/source/torrent.rs` | `TorrentUtils`, torrent models | P2. |
| `src/source/registry.rs` | `AnimeSourceFactory` and the source manager list | Builtin list only. |
| `src/player_loader.rs` | `EpisodeLoader`, `HosterLoader` | `http_hosters`, `load_hoster`, `select_best`, `best_video`, `resolve_once`. |

---

## 3. Reading guide

All documents are in this folder.

- [p1-inventory.md](p1-inventory.md): every file in scope, with LOC, every top-level declaration, the port decision (Port, Rewrite, Replace-with-crate, Drop) and the task ids that cover it.
- [p1-behavior.md](p1-behavior.md): OBSERVED Kotlin behavior with `path:line` citations, in 21 sections: identity, request defaults, interface defaults, listing and details, hosters and videos, selection and resolution, sorting, Video model, thumbnails, local server, preferences, deep links, unmetered sources, filters, torrents, playback options, manga overlap, concurrency, constants, errors.
- [p1-data-and-config.md](p1-data-and-config.md): field-by-field models with Kotlin types, nullability and defaults; enum ordinals; the `Hoster` and `Video` JSON wire formats; torrent and TorrServer JSON; storage locations; defaults summary table.
- [p1-topic-url-and-id-compat.md](p1-topic-url-and-id-compat.md): source id algorithm (MD5 to 63-bit) and `url without domain` algorithm with a 50-row vector table and the generator program. Compatibility contract with stored data.
- [p1-topic-video-pipeline.md](p1-topic-video-pipeline.md): episode to playable video path: discovery decision table, hoster load, `selectBestVideo` tiers, phase A and phase B best-video search, resolution, sort rules, 12 must-pass tests.
- [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md): edge cases E-01 to E-39 with Keep, Fix or Proposal decisions; race table; upstream tests to port; 45 unit and integration tests; fixtures; user-facing failure modes.
- [p1-rust-design.md](p1-rust-design.md): Cargo additions, module layout, Rust types for every model, trait signatures, HTTP base, placeholder regex, loader algorithms, async model, Android-to-Linux mapping table, pitfalls.
- [p1-tasks.md](p1-tasks.md): 56 implementation tasks `T-16a-001` to `T-16a-056`, each with priority, effort, dependencies, sources and acceptance criteria. Lints clean (`python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint`).

---

## 4. Key design decisions and open questions

### 4.1 Decided in the plan (recommendation: keep)

| # | Decision | Kotlin reference | Rust rule | Edge |
|---|---|---|---|---|
| D1 | Hoster-versus-legacy routing is an explicit capability `provides_hosters()`, not reflection. | `EpisodeLoader.kt:60-78` (`checkHasHosters`) stops at `ParsedAnimeHttpSource` and never sees hoster selectors. | Parsed helper sets it when a hoster selector is configured. | E-01 Fix |
| D2 | No self-recursive defaults. `hosters` and `videos_for_hoster` default to `NotSupported`. | `AnimeCatalogueSource.kt:65-67` (confirmed in source: both overrides call themselves). | Required or `NotSupported`. | E-02 Fix |
| D3 | Source id and `url without domain` are frozen contracts, reproduced bit for bit, including the rejection fallback and decoded output. | `AnimeHttpSource.kt:101-105`, `:685-699`. | Vector table in the topic doc is the oracle. | E-06, E-19 to E-21 Keep or Fix (only opaque URLs change) |
| D4 | Legacy extension paths are not ported: `"null"` URL sentinel, ext-lib 15/16 constructors, `videoUrlRequest`. | `EpisodeLoader.kt:180-187`, `Video.kt:60-85`. | Dropped. | E-22, T-16a-015 |
| D5 | Parallel hoster probing: one task per hoster, first resolved preferred video wins, others aborted; no fixed concurrency cap in this layer. | `HosterLoader.kt:79-113`. | `JoinSet` with `abort_all`, bounded by the area:12 rate limiter. | E-30, E-31 |
| D6 | Filter equality is value equality, not the always-false `AnimeFilterList.equals`. | `AnimeFilterList.kt:10-12`. | Derived `PartialEq`. | E-12, E-13 Fix |
| D7 | Select index out of range is clamped to 0 with a warning. | No check in Kotlin (E-14). | Constructor clamps. | E-14 Fix |
| D8 | Local stream server never produces `http://localhost:0`. | `AnimeDownloader.kt:452-454`. | `rewrite_to_local(0)` is an error. | E-08 Fix |
| D9 | Image tile fetch is async with a status check. | `AnimeHttpSource.kt:533-537` (blocking `execute()`). | `fetch` helper, decode on `spawn_blocking`. | E-09 Fix |
| D10 | Source preferences are one JSON object per source id, key `source_<decimal id>`, stored in the settings store. | `animesource/utils/Preferences.kt:13-40`. | area:02-preferences-system table row. | Keep |
| D11 | Sorting: no global quality sort. Source order is priority. | `AnimeHttpSource.kt:591-611` (identity defaults). | Identity defaults; source overrides. | Keep |
| D12 | Kotlin `Float` episode number is kept as `f32` and round-trips through REAL. | `SEpisodeImpl` default `-1f`. | `f32` in memory. | Keep |

### 4.2 Open questions for the user (each with a recommendation)

- **Q1. Which builtin anime source ships with the first usable build?** This area provides the trait, the models and the loader, but the registry is empty (T-16a-038), so P0 "at least one working source" is not satisfied by any task here. Options: (a) the user names one site and it becomes a builtin module in a later area, (b) an in-crate test source only, which proves the pipeline but gives no content. **Recommendation:** (a). Owner of the source work is area:17-source-app-layer or area:20-extension-strategy-redesign; the user must name the site. Lazier alternative: ship only the test source until the user names a site.
- **Q2. Torrent backend (T-16a-041, P2).** Options: (a) TorrServer HTTP client only (`POST http://127.0.0.1:<port>/torrents`, default port 8090, per [p1-data-and-config.md](p1-data-and-config.md) section 8.2), which needs the user to run TorrServer; (b) embedded `librqbit`, which is self-contained but adds a large dependency. **Recommendation:** (a), defer until a torrent source is needed. Lazier alternative: drop torrents entirely and record the loss.
- **Q3. Keep Aniyomi extension (APK) compatibility?** Not possible on Linux without a JVM or a plugin ABI; the decision lives in area:20. **Recommendation:** no APK loading; builtin Rust sources only (T-16a-038, T-16a-053). User loss: existing Aniyomi third-party extensions do not work.
- **Q4. Local stream server crate.** `axum` (features `tokio`, `http1`) per [p1-rust-design.md](p1-rust-design.md) section 1, or `tiny_http`. **Recommendation:** `axum`, because `LocalHandler::serve` is async and Range handling is required (E-38). Lazier alternative: `tiny_http` with a blocking thread, if the dependency count matters more than async uniformity.
- **Q5. Selector dialect.** Jsoup `:contains()` and `:matches()` do not exist in `scraper` (E-05). Sources ported from Jsoup selectors return empty lists silently in Kotlin; Rust returns `SourceError::Parse`. **Recommendation:** accept the difference and filter by text in code. Confirm that the user does not need Jsoup text pseudo-classes in any source they plan to write.
- **Q6. Season and episode update when one side fails (E-26).** Kotlin discards a successful episode list if details fail. **Recommendation:** keep the all-or-nothing contract in this area, and let the caller retry each flag separately (area:26-library-update-jobs).

### 4.3 Known spec gaps (must be closed before implementation)

- **G1. Java `URI` acceptance rule is not fully written down.** [p1-topic-url-and-id-compat.md](p1-topic-url-and-id-compat.md) section 2.2 lists rejected characters and says that position rules (for example `[` and `]` allowed in a query, rejected in an absolute path) are only defined by the test table in section 3. Implementers need an explicit validator. **Proposal:** write `java_uri_accepts(&str) -> bool` as a per-component state check, and use the section 3 table as the test oracle. Regenerate the TSV with the section 4 generator (OpenJDK 17).
- **G2. Image tile return type conflict.** [p1-behavior.md](p1-behavior.md) section 9.3 proposes `Result<Option<Vec<u8>>, SourceError>`; [p1-rust-design.md](p1-rust-design.md) section 3.6 uses `bytes::Bytes`. **Decision:** `bytes::Bytes` (rust-design wins).
- **G3. Equality of `Video` for stored data.** Kotlin `data class` equality covers only constructor properties; `status` is excluded (E-34). The Rust struct excludes `VideoEntry.state` the same way. Confirmed consistent; no action.

### 4.4 Doc corrections found during this review (recorded, not all applied)

- Applied in [p1-tasks.md](p1-tasks.md): T-16a-034 referred to a non-existent `E-40`; now points at area:02-preferences-system. All short `area:` tokens in that file were expanded to full area ids.
- Not applied (owners to fix): [p1-behavior.md](p1-behavior.md) section 19 and [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) E-32 cite the default cache-control constant at `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt:13`. The constant is at line 16 (`private val DEFAULT_CACHE_CONTROL = CacheControl.Builder().maxAge(10, MINUTES).build()`).
- Not applied: [p1-inventory.md](p1-inventory.md) line 69 says "14 model files" under `animesource/`; the table lists 16 and the directory has 16.
- Not applied: [p1-rust-design.md](p1-rust-design.md) section 3.3 says the Select clamp is "E-12 family"; the correct edge id is E-14.
- Not applied: [p1-behavior.md](p1-behavior.md) section 16.1 and the scope reminder at the top of [p1-rust-design.md](p1-rust-design.md) point player UI at area:39 and area:36. Player UI is area:45-player-core and area:46-player-controls-ui.

### 4.5 Spot-checks against the Kotlin source (done in this review)

Confirmed: `AnimeCatalogueSource.kt:65-67` self-recursion; `AnimeHttpSource.kt:60` (`versionId = 1`), `:72` (lazy id), `:101-105` (`generateId`); `HttpSource.kt:412` (space replacement); `EpisodeLoader.kt:60-78` (`checkHasHosters` walk, names `getHosterList`, `hosterListRequest`, `hosterListParse`); `Video.kt:189` (`localUrl` regex `http:\/\/localhost:1(?!\d)`), `Video.kt:188` (`MPV_ARGS_TAG`); `SAnime.kt:72` and `:79` (`UNKNOWN`, `UPCOMING`); `TorrentUtils.kt:24` (case-sensitive `startsWith("magnet")`); `HttpServer.kt:7` and `:33`; `AnimeDownloader.kt:940-942` (30000 ms, 10, 20); `AnimeLibraryUpdateNotifier.kt:413` (60); `JsonExtensions.kt:11-12` (doc comment `ignoreUnknownKeys = true`, `explicitNulls = false`).

Not verified in this review: the deprecated `Video.copy` overload behavior (E-07) and the exact `Hoster` state constants; both are marked "probable" in the edge doc.

---

## 5. Cross-area dependencies

### 5.1 What this area needs from others

| Area | Needed | Used by |
|---|---|---|
| area:02-preferences-system | Settings store row `scope="source"`, `key="source_<decimal id>"`, typed JSON values; read with default; no row written on read. | T-16a-034, T-16a-035 |
| area:04-db-anime-schema-migrations | Columns `animes.fetch_type` (ordinal, default 1), `update_strategy` (ordinal, default 0), `season_number` (REAL), `memo`, `initialized`; `episodes.memo`, `fillermark`, `date_upload` (epoch ms). Unknown ordinal decodes to the default. | T-16a-009, T-16a-010 |
| area:05-data-repos-entries-items | Repository mapping of the same columns to these models. | T-16a-006, T-16a-007 |
| area:07-domain-entries-items | Episode sync, `source_order`, consumption of `SAnimeEpisodeUpdate`. | T-16a-008 |
| area:09-domain-source-track-download-release-misc | Source manager lookup by id (`sourceManager.get`, `getOrStub`) that maps to the Rust registry. | T-16a-038 |
| area:12-network-stack | `reqwest` client; default `User-Agent` from setting `default_user_agent` (default Firefox 136 string); cookies and Cloudflare handling; rate limiter that skips `UnmeteredSource`. | T-16a-025, T-16a-026, T-16a-037 |
| area:13-core-util-storage-archive-torrent | Bencode parser for `.torrent` files (`TorrentHelpers`) and the two upstream torrent fixtures. | T-16a-040 |
| area:14-source-local-and-metadata | `LocalAnimeSource` implements the anime traits for the `Local source: <episode.url>` hoster. | T-16a-019 |
| area:15-source-api-manga | Manga trait shapes, `HttpSource` URL rules, `Page` progress. | T-16a-042, T-16a-044, T-16a-054 |
| area:19-extensions-anime, area:20-extension-strategy-redesign | Decision that no APK loading exists; builtin registry only. | T-16a-038, T-16a-053 |
| area:22-backup-restore | Backups carry relative urls and source ids written by Aniyomi. Restore must not renormalise them. | T-16a-027 (consumer note) |
| area:24-downloads-anime | ffmpeg argument vector and local stream lifetime. | T-16a-033 |
| area:26-library-update-jobs | Skip rule for `OnlyFetchOnce`; per-flag combined update calls. | T-16a-010, T-16a-004 |
| area:51-i18n | Message keys: `http_server_start_failure` ("Failed to start http server"), `http_server_is_running` ("Http server is running"), `torrserver_disabled` ("TorrServer is not running"), `download_queue_size_warning`. | T-16a-033, T-16a-040 |
| area:53-platform-manifest-intents-workers | Lifetime of the local stream server tied to playback (replaces the Android foreground service). | T-16a-033 |

### 5.2 What others need from this area

| Consumer area | Needs | Provided by |
|---|---|---|
| area:07-domain-entries-items, area:26-library-update-jobs | `SAnimeEpisodeUpdate`, `SAnimeSeasonUpdate`, `supports_details()`, `supports_episode_list()`, `UpdateStrategy::skips_library_update()` | T-16a-004, T-16a-008, T-16a-010 |
| area:45-player-core, area:46-player-controls-ui | `best_video`, `load_hoster`, `select_best`, `resolve_once`, `VideoEntry`, `HosterState`, `ThumbnailInfo`, `LocalStreamServer` | T-16a-020, T-16a-021, T-16a-022, T-16a-032, T-16a-033 |
| area:24-downloads-anime | `Video` model with `headers: Option<Headers>`, `uses_local_server()`, `rewrite_to_local(port)`, mpv and ffmpeg argument vectors | T-16a-015, T-16a-016 |
| area:22-backup-restore, area:04-db-anime-schema-migrations | `generate_id`, `url_without_domain`, `AnimeStatus` integers, `FetchType` ordinals | T-16a-002, T-16a-027, T-16a-006, T-16a-009 |
| area:39-ui-entry-details-anime, area:41-ui-browse-anime | `AnimeStatus`, `genres()`, `FetchType`, `AnimesPage`, `FilterList` (values and equality), `AnimeRelation` | T-16a-006, T-16a-011, T-16a-012, T-16a-013, T-16a-046 |
| area:02-preferences-system and settings UI | `SourcePref` descriptors (`Switch`, `List`, `MultiSelect`, `EditText`, `Category`) | T-16a-035 |
| area:17-source-app-layer | Trait objects `Arc<dyn AnimeSource>`, `ResolvableAnimeSource` for deep links | T-16a-003, T-16a-036 |
| area:15-source-api-manga | Shared `FilterList`, `HttpBase`, `generate_id`, header helpers | T-16a-025, T-16a-042 |

---

## 6. Task summary

Source: `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py export`, then the `TASKS.csv` rows whose `file` column starts with `16-source-api-anime/`.

**Totals: 56 tasks** (`T-16a-001` to `T-16a-056`, no gaps, dependency graph acyclic, longest chain 10 tasks, no dangling dependencies).

| Priority | Count | S | M | L | XL |
|---|---|---|---|---|---|
| P0 | 31 | 18 | 12 | 1 | 0 |
| P1 | 15 | 7 | 7 | 1 | 0 |
| P2 | 5 | 2 | 2 | 1 | 0 |
| P3 | 5 | 5 | 0 | 0 | 0 |
| **All** | **56** | **32** | **21** | **3** | **0** |

P3 tasks are drop decisions with reasons: T-16a-048 (RxJava bridge), T-16a-049 (util `Video` stub), T-16a-050 (Android actuals and manifest), T-16a-053 (deprecated hooks), T-16a-056 (manga `fetch_search` Rx wrapper).

**P0 critical path** (longest chain by effort weight, S = 1 day, M = 3 days, L = 7.5 days, midpoint estimates from the effort definitions; total about 26.5 engineer-days, serial):

1. T-16a-001 (S) skeleton and crates
2. T-16a-011 (M) filter model
3. T-16a-012 (S) filter list
4. T-16a-003 (M) `AnimeSource` trait
5. T-16a-005 (S) hoster routing capability
6. T-16a-019 (M) hoster discovery
7. T-16a-020 (M) hoster load and sort
8. T-16a-021 (S) preferred-then-URL selection
9. T-16a-022 (L) best-video search with early return
10. T-16a-051 (M) test corpus

Parallel P0 work that feeds the same chain: T-16a-014 to T-16a-017 (Video, Hoster models), T-16a-023 and T-16a-024 (resolution, sort), T-16a-026 and T-16a-027 (request execution, URL compatibility), T-16a-034 (preference namespace).

**Risks**

1. **No working source** (Q1). P0 playback depends on a concrete source that is not in this area. Probability high, impact high. Mitigation: user names the first site before implementation starts.
2. **Silent selector failures** after porting Jsoup-based sources (E-05). Mitigation: explicit `SourceError::Parse` on selector parse failure; test 15.
3. **Java URI parity** (G1). A mismatch changes stored urls for unusual inputs, which breaks backup matching for those entries. Mitigation: vector TSV regenerated by the OpenJDK generator, zero diff required in CI.
4. **Local stream server seeking** (E-38). mpv and ffmpeg need `Range` support; a wrong 200 instead of 206 breaks seeking silently. Mitigation: test 42 (range) and the stop-during-download test in the race table (5 s limit), plus the 2 s `stop()` bound in T-16a-033.
5. **Unbounded parallel probes** (E-31). Mitigation: the area:12 rate limiter must apply to hoster probes; cross-area test in T-16a-037.
6. **Extension loss for users** (Q3). Existing Aniyomi APK extensions stop working. User must accept.
7. **Source id stability** (D3). Changing `generate_id` orphans every source. Mitigation: frozen vector tests (test 1 and test 2 in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section G).
8. **Torrent backend undecided** (Q2). P2, no P0 impact.

---

## 7. Definition of done for this area in Kurobayomi

Observable behaviors, each checkable without reading code:

1. A builtin source (Q1 choice) returns popular, latest and search results; a page with a next-page element shows a "next" control and a page without one does not.
2. Opening an entry with `fetch_type = Episodes` lists episodes in the order the source returned them; an entry with `fetch_type = Seasons` lists seasons and never fetches episodes.
3. Choosing an episode from a source that provides hosters shows a server list; a source that does not provides shows one unnamed server (displayed as "no hoster", never as the raw key `no_hoster_list`).
4. Playback starts from the first preferred video that resolves to a non-empty URL; if none is preferred, from the first valid URL in source order; a video that fails to resolve is skipped and the next candidate is used.
5. Closing the player during resolution stops every pending hoster and resolve request; no further network requests for that episode appear in the log after close.
6. A stream whose URL starts with `http://localhost:1` plays through the local stream server on `127.0.0.1` with an ephemeral port; seeking works (a `Range` request gets `206` with a correct `Content-Range`).
7. If the local server cannot bind, the user sees "Failed to start http server" and playback is not attempted with port `0`.
8. Source id and stored relative url computed for the vector inputs in [p1-topic-url-and-id-compat.md](p1-topic-url-and-id-compat.md) section 3 match the Aniyomi values; an Aniyomi backup entry restored into Kurobayomi matches its source (area:22).
9. Each `SourcePref` kind (`Switch`, `List`, `MultiSelect`, `EditText`, `Category`) renders in the source settings screen and its value survives an application restart under `source_<id>`.
10. Magnet links with TorrServer disabled are not sent to any backend; with TorrServer enabled but not running, the user sees "TorrServer is not running".
11. `cargo test -p kurobayomi source::` passes offline (no network), including the 45 tests in [p1-edge-cases-and-tests.md](p1-edge-cases-and-tests.md) section G and the URL vector TSV with zero diff against the Java generator.
12. No Kurobayomi module outside `src/source/` imports `reqwest` or `scraper` (checked with `rg "use (reqwest|scraper)" src`).

Dropped with user impact recorded: APK extensions (Q3), RxJava-era helpers (no user impact), Android foreground-service notification for the local server (no user impact; GTK shows playback status), `"null"` URL legacy fix-up (no impact for builtin sources).
