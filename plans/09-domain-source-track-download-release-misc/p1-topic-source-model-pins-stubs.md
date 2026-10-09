# Topic: source model, pins, stubs and browse routing

Deep dive for area 09. Covers how a source is identified, displayed, pinned, replaced by a stub when uninstalled, and browsed. Sources of truth: `domain/src/main/java/tachiyomi/domain/source/manga/model/` (Source.kt, Pin.kt, StubMangaSource.kt), `domain/src/main/java/tachiyomi/domain/source/anime/model/` (AnimeSource.kt, Pin.kt, StubAnimeSource.kt, AnimeSourceWithIds.kt, DeletableAnime.kt), the interactors in `domain/src/main/java/tachiyomi/domain/source/*/interactor/`, and the repository and manager interfaces in `domain/src/main/java/tachiyomi/domain/source/*/repository/` and `…/service/`.

## 1. Identity

- A source is identified by a `Long` id. Installed sources use the id supplied by the extension (upstream API). The id is the join key for entries (`mangas.source`, `animes.source`), tracks do not reference sources.
- `key()` distinguishes the normal row (`"<id>"`) from the last-used pseudo-row (`"<id>-lastused"`). The UI uses the key as a list identity, so the same source may appear twice (once under "Last used", once in its language group). Observed: `Source.kt:22-27`, `AnimeSource.kt:19-24`.
- `visualName`: `"Name (LANG)"` when `lang` is non-empty; `"Name"` otherwise (`Source.kt:16-20`).

## 2. Pins

| Bit | Name | Meaning |
|---|---|---|
| 0b01 | `Pinned` | the source is pinned |
| 0b10 | `Actual` | companion bit set together with `Pinned` so that the value is never 0 when pinned |
| 0b00 | `Unpinned` | not pinned |

- The app only creates two values: `UNPINNED` (0) and `PINNED` (0b11) (`Pins.pinned = Pins(Pin.Pinned, Pin.Actual)`, `Pin.kt:28`).
- Quirks (`p1-edge-cases-and-tests.md` section 1): `contains(Unpinned)` is always true (Q-PIN-1), `minus` is XOR (Q-PIN-2), `unaryMinus` ORs (Q-PIN-3).
- Rust: `Pins(u8)` with `is_pinned()`, `with`, `without` (T-09a-003).
- Persistence of the pin state is not defined in this scope (see `p1-data-and-config.md` section 2.1); Proposal: a column or preference keyed by source id.

## 3. Stubs (uninstalled sources)

Lifecycle:
1. An installed source is seen → its id, lang and name are persisted as a stub row (`upsertStubMangaSource`, `upsertStubAnimeSource`).
2. The extension is uninstalled → `get_or_stub(id)` returns the stub. Library entries keep their source name in the UI.
3. Any remote operation on the stub fails with "source not installed" (`SourceNotInstalledException` / `AnimeSourceNotInstalledException`).
4. `display_name()`: `"Name (LANG)"` when name and lang are non-blank, else the numeric id (Q-SRC-1).

Edge: a stub with a blank name shows its id; a stub with a blank language shows its id too (the rule is `isInvalid = name.isBlank() || lang.isBlank()`, `StubMangaSource.kt:15`).

## 4. Browse routing

Three modes per source: popular, latest, search with filters.

| UI action | Rust | Kotlin (observed) |
|---|---|---|
| Open "Popular" | `BrowseQuery::Popular` → `popular(source)` | query string `"eu.kanade.domain.source.manga.interactor.POPULAR"` → `getPopularManga` |
| Open "Latest" | `BrowseQuery::Latest` → `latest(source)` | `…LATEST` → `getLatestManga` |
| Search | `BrowseQuery::Search(text)` → `search(source, text, filters)` | any other string → `searchManga` |

Paging: 1-based page numbers, `next = None` ends the list (E-SRC-10 Proposal: stop after one empty page).

Sources that do not support latest (`supports_latest = false`) must hide the Latest mode; the stub always reports false.

## 5. Listing views

| View | Source of rows | Notes |
|---|---|---|
| Source list (installed, online) | registry `online()` | sorted by pin, last-used, language group, name (area 17 owns the sort) |
| Non-library | `sources_with_non_library()` | count of non-favourite rows per source; zero rows hidden unless orphaned ids exist (E-SRC-09) |
| Migration source list | `sources_with_favorite_count()` | `Pair<Source, count>` |
| Anime deletable | `DeletableAnime` rows with `FetchType::{Seasons, Episodes}` | children via `get_deletable_children(parent)` |

## 6. Registry

- `is_initialized` turns true once the extension loader has finished; screens show a loading state until then (E-SRC-04 covers the uninstall-during-load case).
- `get_or_stub` never returns "not found": it falls back to a stub row, or to an empty stub whose display name is the numeric id (E-SRC-05).

## 7. Drop and keep decisions

| Item | Decision | Reason | User loses |
|---|---|---|---|
| `isExcludedFromDataSaver` (SY fork) | Drop (T-09a-056) | fork-only feature, data saver is area 34 | per-source exclusion from the image data saver |
| `Serializable` on pins and sources | Drop | not a file format | none |
| Upstream `CatalogueSource` / `AnimeHttpSource` types | Replaced by `SourceHandle` over trait objects | Rust trait objects loaded by area 18/19 | none |
| Paging library `PagingSource` | Replaced by `PageSource<T>` | no AndroidX | none |

## 8. Decisions for the user

1. Keep "Last used" as a separate row (Kotlin) or show a single row with a badge? Proposal: keep the Kotlin behaviour, it is what the Browse tab mockup already shows (`src/entry.rs:283-300`).
2. Hide zero-count sources in Non-library (Proposal) or show them? Proposal: hide.
