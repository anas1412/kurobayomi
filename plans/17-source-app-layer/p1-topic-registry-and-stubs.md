# Topic: live-source registry and stub sources

Files: `app/src/main/java/eu/kanade/tachiyomi/source/manga/AndroidMangaSourceManager.kt` (125 lines), `app/src/main/java/eu/kanade/tachiyomi/source/anime/AndroidAnimeSourceManager.kt` (127 lines), `app/src/main/java/eu/kanade/tachiyomi/source/manga/MangaSourceExtensions.kt`, `app/src/main/java/eu/kanade/tachiyomi/source/anime/AnimeSourceExtensions.kt`, `domain/src/main/java/tachiyomi/domain/source/manga/model/StubMangaSource.kt`, `domain/src/main/java/tachiyomi/domain/source/anime/model/StubAnimeSource.kt`.

Behaviour details with line numbers are in `p1-behavior.md` sections 1 and 2. This topic gives the lifecycle, the state machine, and worked examples that the implementation tests must reproduce.

## 1. Concepts

| Term | Definition |
|---|---|
| Live source | An object returned by an installed extension (or the local source, id 0). Lives in `sourcesMapFlow` (manga and anime). |
| Stub | `StubMangaSource`/`StubAnimeSource`: `(id, lang, name)`. Persisted in `sources`/`animesources` once the source has been seen (registered). Network methods throw. |
| Fallback stub | `StubXSource(id, "", "")` created by `createStubSource` when neither the DB row nor the index data exists. Not persisted. Renders as the numeric id. |
| Index data | `MangaExtensionManager.getSourceData(id)`: a stub built from the AVAILABLE extension list (from the repository index, not installed). Empty until the index loads. |

Identity of a source is its 64-bit id. Two different extensions cannot own the same id (duplicates are a loader concern, areas 18-20; within this area the later duplicate replaces the earlier, `AndroidMangaSourceManager.kt:63-68`).

## 2. State machine per source id

States for one id as seen by the registry:

```
            (never seen)
                 |  getOrStub(id) and no DB row, no index data
                 v
   [FALLBACK: StubX("", "") in memory only]  ----- index loads with data ---> [INDEXED: persisted stub]
                 |                                                                  ^
                 | extension installs (live)                                        | extension installs,
                 v                                                                  | DB row present, same data
   [LIVE] <---- extension installed ---- [PERSISTED STUB (DB row)] -- uninstall --> (live removed; row remains)
     |                                        ^   |
     | extension renamed or lang changed      |   | extension installs with different (lang,name)
     +-------------------------------------> upsert row + renameSource(old,new)
```

Transitions (OBSERVED unless marked Proposal):
- `LIVE` is entered when an `installedExtensionsFlow` emission contains the source. Registration (write of the row) runs in a detached task for every live source of every emission (`AndroidMangaSourceManager.kt:66`).
- The row is written when it is absent, or when its `(lang, name)` differs. **Observed:** because `StubMangaSource` has no `equals`, the "unchanged" test never holds, so the row is rewritten on every emission. Proposal: derive `PartialEq` (`p1-rust-design.md` 2.3).
- `renameSource(old, new)` runs only when a row existed before the write (`AndroidMangaSourceManager.kt:109-111`). It is a no-op when the folder name is unchanged (`MangaDownloadManager.kt:352-368`).
- Uninstall: the live entry disappears from the map on the next emission; the row is not deleted (no delete query exists, `data/src/main/sqldelight/data/sources.sq`). From then on `get(id)` is `None` and `getOrStub(id)` returns the persisted stub.

## 3. Worked examples (test vectors)

Notation: `live(id, lang, name)`, `row(id, lang, name)`, result = what `getOrStub(id)` returns and what `display()` prints.

| # | Setup | Expected |
|---|---|---|
| S1 | extension installed with source `(10, "en", "Foo")`; DB empty | `get(10)` = live; row `(10, en, Foo)` inserted; no rename |
| S2 | extension updated; source now `(10, "en", "Foo2")`; row `(10, en, Foo)` | row updated to `Foo2`; `renameSource(Foo→Foo2)` called once; download folder follows (area 23-25) |
| S3 | same as S2 but the extension re-emits with no change | row rewritten with identical data (Kotlin) / skipped (Proposal); `renameSource` called (Kotlin) / not called (Proposal); folder unchanged in both |
| S4 | extension uninstalled; row `(10, en, Foo2)` | `get(10)` = None; `getOrStub(10)` = persisted stub `Foo2 (EN)`; calling its fetch throws `SourceNotInstalled` |
| S5 | library entry references `999`; no row, no index data | `getOrStub(999)` = fallback `("", "")`; `display()` = `"999"`; the fallback is not in the table |
| S6 | index loads with `(999, "ja", "Bar")` after S5 | Kotlin: `getOrStub(999)` stays `999` until restart (the fallback sits in `stubSourcesMap` and `getOrPut` never re-runs). Proposal: `on_index_loaded` upgrades the cached fallback to `Bar (JA)` and persists the row |
| S7 | DB row `(20, "", "Name")` (blank lang) | `display()` = `"20"` (invalid because lang is blank) |
| S8 | DB row `(21, "en", "   ")` (blank name) | `display()` = `"21"` |
| S9 | live local source `(0, "other", "Local manga source")` | `visual_name()` = `"Local manga source (OTHER)"`; not a stub |
| S10 | two extensions both claim id 30; loader keeps both | live map keeps the later one (Kotlin `mutableMap[it.id] = it`); row written for both in turn |

## 4. The stub list (`getStubSources`)

Kotlin returns stubs for ids not currently live (`AndroidMangaSourceManager.kt:99-102`). Because the DB subscription never fills the map (`:74-82`), the list contains only stubs created in this session. Example: the user has 5 stored stubs, opens the library, touches 2 of them: the stub list shows 2 items (Kotlin). Proposal: the list shows all 5 (the registry loads the table at start, `p1-rust-design.md` 3.5). This is the T-17a-034 change.

Consumers: the stub list is shown on the source management screens (owned by the UI areas); it is the way to see and clean up sources you no longer have installed.

## 5. Lookup pipeline for UI code

```
get_or_stub(id):
  1. live map has id          -> ResolvedSource::Live
  2. memory stub map has id   -> ResolvedSource::Stub (persisted row or cached fallback)
  3. index has id             -> persist (async) and return the index stub
  4. else                     -> cache and return fallback("", "")
```

Step 1 precedes step 2 on purpose: a live source always wins, so after an install the stub is ignored for display. The stub entry stays in the cache (Kotlin never removes it); that is harmless.

## 6. Names and display (by context)

| Context | Kotlin function | Output for live `Foo (EN)` with one enabled language `en` | Output with two enabled languages |
|---|---|---|---|
| Library/info title | `getNameForMangaInfo()` | `Foo` (single language, source's language enabled) | `Foo (EN)` |
| Source list row | `visualName` (domain) | `Foo (EN)` | `Foo (EN)` |
| Stub display | `toString()` | `Foo (EN)` or id | same |

Full rule table: `p1-behavior.md` 2.3.

## 7. Startup and readiness

- `isInitialized` becomes true after the first extension emission (`AndroidMangaSourceManager.kt:70`). Until then the UI shows a loading state (`app/src/main/java/eu/kanade/core/util/MangaSourceUtil.kt`, `AnimeSourceUtil.kt`).
- The DB subscription (`:74-82`) can run before or after; the first observable live map is the extension map.
- Warm-up (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:242-251`) constructs the managers early.

Rust: `SourceSystem::start` loads the stub tables first (synchronously on a background task), then starts the live loop. `ready` flips after the first snapshot. The GTK side waits for `ready` (`p1-rust-design.md` 3.6).

## 8. Failure modes

| Failure | Kotlin result | Rust result |
|---|---|---|
| DB error during `registerStubSource` | uncaught in the scope → scope cancelled (all later registrations silently dropped) | logged; the single registration is dropped; others continue |
| Extension list fails to load | `installedExtensionsFlow` emits the last value (flow never errors); no change | same: the watch keeps the last snapshot |
| Index empty | `getSourceData` returns null → fallback | same |
| Rename fails (folder busy) | logged at ERROR, not thrown | logged; the row is already updated (upsert runs before rename, and the two are not atomic). No retry: later registrations see old == new and `findSourceDir` cannot find the old folder, so the download folder stays under the old name. Proposal: at start, log download folders that match no source name, so the user can see the orphan |
| Uninstall during a fetch | the fetch throws `SourceNotInstalledException` if the stub is used | same |

## 9. Concurrency notes

- The live map is replaced atomically per emission. Readers never see a partial map (`MutableStateFlow` set once).
- Two registration tasks for the same id may interleave (Kotlin: no lock). Proposal: `register_lock` (`p1-rust-design.md` 3.1).
- `getOrStub` may be called from any thread in Kotlin. The `runBlocking` inside `getOrPut` (`AndroidMangaSourceManager.kt:90-92`) blocks that thread on a DB query for an unknown id. In Rust there is no DB call on this path (table in memory), so the UI thread is safe.

## 10. Local source

Constructed inside the live map on every emission (`AndroidMangaSourceManager.kt:54-62`), so Kotlin creates a new object each time with the same identity (`id = 0`). Rust: construct once and keep the same `Arc` across snapshots (the same identity, and no repeated construction).

## 11. Things this topic does NOT cover

- How a source is loaded from the extension package (areas 18-20).
- The download folder naming (`provider.getSourceDirName`, area 23-25).
- The source's network behaviour (areas 15, 16).
