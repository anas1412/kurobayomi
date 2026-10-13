# One entry page (step 6, the big merge)

The page of a title (cover, details, chapter or episode list, selection bar, menus) exists twice: `src/entry_core/*` for
manga (about 9,200 lines) and `src/entry_core/anime/*` for anime (about 7,800 lines). Slices 1 to 6 already shared the
small parts: the selection engine, the Download menu, the duplicate dialog, the library button flow, the refresh rules,
the overflow menu, the row model (`list_model`), the list view, the info views and the category picker. What is left
is the layer that uses them: the model, the store, the backend, the runner, the actions and the route.

## Decision: grow the anime page into the page of both kinds

The anime page is the base, not the manga page. It already runs on the shared `Entry` and `Item` types, the domain filter
and sort engine (`item_filters`, `apply_filters`), `SessionFlags` and the shared download state, and it covers a superset
of the features (seasons, related titles, airing, intro length, torrent sources). The manga page runs on its own
`MangaRow` and `ChapterRow` types with its own sort (`chapter.rs`), filter (`view.rs`) and flag wrapper (`flags.rs`), which
the domain engine replaces.

The merge is expand then contract, so the app works at every commit:

| Phase | What | State of the app |
|---|---|---|
| G1 | Make the anime page kind-neutral: types and files lose the word anime (`AnimeEntryModel` becomes `EntryModel`, `AnimeCmd` becomes `EntryCmd`, `AnimeStore` becomes `EntryStore`, `AnimeBackend` becomes `EntryBackend`, `entry_core/anime/` becomes `entry_core/page/`). Every place a kind matters takes the model's `Kind`: wording, the item word (episode or chapter), done (seen or read), the open-item strategy, the mark set (fillermark only for anime), what the header shows. Behaviour for anime does not change. | manga still on its own page |
| G2 | Give the generic page what only the manga page has (below), with the manga tests ported to it for `Kind::Manga`. | manga still on its own page |
| G3 | Switch: `entry_core::route::open` for a manga opens the generic page. Both kinds are checked on the test display. | one page, old manga code unused |
| G4 | Delete the manga page (`route`, `runner`, `page`, `detail`, `store`, `backend`, `chapter`, `view`, `flags`, `chapter_actions`, `dialog`, `bulk_bar`, `toolbar`, the manga half of `header`) and what only it used. | done |

## What only the manga page has (G2)

1. **Scanlators:** the available and excluded sets (store reads, a write, two watches), the sheet (`scanlator_sheet.rs`,
   already kind-neutral), the filter tint, and the list menu item. Manga entries only; an anime has no scanlator filter.
2. **Reset and defaults:** the Reset item that puts the library defaults back over a non-library entry's session copy
   (`note_flags_reset`), next to "use as default for all titles".
3. **Row second line:** the last page read ("Page 12") and the scanlator, in `list_model` (the row model is shared, the
   facts are manga's).
4. **Sort and filter parity:** the manga sort modes (source, number, upload date, alphabet with the collator) and the
   filter set (downloaded, unread, bookmarked, scanlator) must come out of the domain engine as today. `chapter.rs` and
   `view.rs` have 27-combination and ordering tests that move to the engine tests.
5. **Mark previous as read, bookmark** (anime has the mark set; bookmark is shared, fillermark is anime only).
6. **Open item:** a chapter opens the reader (`reader::launch`), an episode the player; the strategy is per kind.
7. **Download indicator rules** (`chapter_actions`): which action a row's indicator runs, which rows an action acts on,
   the local-entry rule. One rule set for both kinds.
8. **Notices:** the page's notices (refresh failed, queued prompt, delete downloads prompt) go through the one toast.

What only the anime page has stays as optional parts of the page, present only when the entry has them: seasons and
the season grid, related titles, airing time, the intro length, the torrent server, the quality sheet of the player.

## Naming

The page, its model, runner, store, backend and commands are named for what they are: `entry_core::page::{EntryModel,
EntryRunner, EntryStore, EntryBackend, EntryCmd, EntryEvent, route}`. The backends stay two implementations of one trait
(`SourceBackend` for manga, `RemoteAnimeBackend` for anime), because a manga source and an anime source are different
things; a manga backend and an anime backend share the trait and the default methods.

## Tests

The manga tests listed per file in this repo's history (page, detail, runner, store, chapter, view, list_options,
scanlator_sheet) are the checklist: each one is ported to the generic page for `Kind::Manga` before its file is deleted,
or deleted with a reason when it tested only the old type. The anime tests stay and run for `Kind::Anime`.

## Verification

Each phase: `cargo check --bin kurobayomi --tests`, the app tests, the path and platform scripts. G3 and G4 also run both
pages on the test display (a manga with chapters and a downloaded one, an anime with episodes and seasons) and compare
them to screenshots of the old pages: header, list rows, the selection bar, the filter menu, the overflow menu, a refresh,
the library button, the download indicator.

## Risks

- The GTK route is the least covered by tests. The test display checks and the existing route smoke tests are the guard;
  every visible element of the manga page is on the list above.
- Manga sort and filter order must not change. The ported ordering tests decide it.
- G1 is a large mechanical rename; it is done with word-boundary replacements and a compile, then committed alone.
