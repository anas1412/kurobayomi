# Fix plan v2 (2026-10-11): the audit's open tasks against the app flow and the settings diet

Written by hand from `docs/audit/P0-AUDIT.md` (section 4), `docs/design/app-flow.md` and `docs/design/settings-diet.md`. Each open task of
the lanes E, D, K, G2, H, F, G2b, G2c and Q gets one verdict:

- **KEEP**: still needed for the flow, unchanged.
- **CHANGE**: needed, but the work differs because of the flow or the diet (the change is named).
- **DROP**: not needed any more (feature cut, install step gone, other trackers, touch gestures, settings the diet cuts).

Ids start with `T-`. Lanes G1, L, C and O (unfinished parts) are re-cut at the end of this file.

## Summary

| Lane | Tasks | KEEP | CHANGE | DROP |
|---|---:|---:|---:|---:|
| E source registry, local source, library update | 50 | 38 | 10 | 2 |
| D downloads | 27 | 21 | 6 | 0 |
| K browse | 29 | 20 | 8 | 1 |
| H tracking (AniList only) | 30 | 30 | 0 | 0 |
| F extensions (no install step) | 51 | 22 | 17 | 12 |
| G2 player session | 43 | 37 | 3 | 3 |
| G2b player controls | 30 | 7 | 9 | 14 |
| G2c player sheets | 15 | 9 | 3 | 3 |
| Q duplicates and shared components | 60 | 12 | 42 | 6 |
| **All** | **335** | **196** | **100** | **41** |

(Counted by script from the lists below.) About 296 tasks remain to build or change after dropping 41. The biggest cuts are in the extension install and trust machinery (F), the touch
gestures (G2b) and the player sheets for decoder and channel chips (G2c).

## Lane E: source registry, local source, library update

- **KEEP:** 01a-037, 06a-040, 06a-041, 07a-032, 09a-004, 09a-005, 14a-001, 14a-003, 14a-012, 14a-013, 14a-014, 14a-016, 14a-017, 14a-018, 14a-019,
  14a-025, 14a-043, 14a-051, 15a-035, 17a-004, 17a-005, 17a-007, 17a-008, 17a-009, 17a-010, 17a-011, 17a-012, 17a-013, 17a-036, 17a-037, 20a-003,
  20a-094 (backup source-id resolution: key for Aniyomi import), 26a-004, 26a-006, 26a-008, 26a-009, 26a-025, 26a-032.
- **CHANGE:** 17a-006 (the loop watches the repository source set, not installed packages), 10a-001 (one small container, no extra layers), 26a-033,
  26a-129, 26a-130, 26a-131 (keep, but fold into the one runtime and job registry), 40b-003, 40b-040, 41b-010, 52a-056 (gates and job registry:
  one gate, one registry).
- **DROP:** 26a-024 and 26a-092 (auto-download of new episodes: the diet cuts auto-download), none else.

## Lane D: downloads (basic only: 4 settings)

- **KEEP:** 08a-022, 15a-028, 23a-005, 23a-006, 23a-007, 23a-008, 23a-009, 23a-011, 23a-012, 23a-013, 23a-014, 23a-019, 23a-020, 23a-024, 23a-027,
  23a-030, 23a-035, 23a-036, 37a-026, 39a-039, 39b-002.
- **CHANGE:** 09a-021 (two settings: slots and speed; Wi-Fi-only off by default), 23a-010 (scheduler per source, at most 10 sources), 23a-023 (background
  running without Wi-Fi gating by default), 47ba-015, 47ba-016 (one simple queue page), 52a-010 (the downloaded-only filter stays as a library filter).
- **DROP:** none. Downloads stay a rare feature, but the download manager must be started in the app (the audit's gap 4).

## Lane K: browse

- **KEEP:** 06a-042, 06a-043, 06a-081, 09a-060, 10a-019, 17a-017, 17a-045, 20a-005 (small per-source settings kept for plug-in sources), 40a-038, 40b-041,
  41a-005, 41a-007, 41b-015, 41b-016, 41b-018, 41b-020, 41b-042, 41b-053, 47a-060, 41a-049.
- **CHANGE:** 10a-018 and 17a-016 (no enable or disable toggles: every source of a repository is enabled; a language filter decides), 40a-001 (the
  Browse tab is sources plus global search only, no extensions or migrate sub-pages), 40a-002, 41a-006, 41b-023 (two display modes), 40b-014, 41b-004.
- **DROP:** 41a-042 (hidden and pinned source sets; with at most 10 sources they are not needed).

## Lane H: tracking, AniList only

- **KEEP all 30:** 28a-003 to 28a-057 and req-platform-003 (the secret store). The other ten trackers' code stays on disk but switched off.

## Lane F: extensions, with no per-source install step

The new flow: add a repository link, its sources appear, each source's file is fetched and checked on first use, a daily refresh updates them.

- **KEEP:** 15a-001, 15a-020, 15a-024, 15a-030, 15a-031, 15a-032, 15a-048, 16a-011, 16a-051, 18a-003, 18a-006, 18a-008, 18a-009, 18a-029, 18a-048,
  19a-024, 19a-030, 19a-031, 20a-001, 20a-011, 20a-012, 40b-051.
- **CHANGE:** 18a-001, 18a-002 (the ABI gate guards the plug-in contract), 18a-005 (a cache of fetched files, not a package store), 18a-020,
  18a-021, 18a-022, 19a-013 (repository index `index.min.json`, one fetch per repository), 18a-028, 18a-030, 18a-034 (lazy fetch and checksum), 18a-047,
  19a-011 (digest check only, no signer chain), 19a-033, 19a-061, 19a-069, 20a-002 (registry state: repositories and their sources), 41a-013 (refresh
  repositories).
- **DROP:** 18a-004 (no `.kuroext` zip), 18a-012, 18a-013, 41a-012 (trust rules and dialog: adding the link is the trust decision), 18a-033 (uninstall:
  removing a repository removes its sources), 18a-036, 18a-037, 19a-032, 40a-017, 40a-018, 41a-011, 41a-015 (the Extensions screen, install queue, install rules and
  lifecycle).
- **NEW lane R (repository link):** the add-link field, the index fetch, language grouping, lazy fetch with checksum, daily refresh, remove repository; the
  anime operations in the plug-in contract; move the three built-in source crates to `kurobayomi-extensions` as plug-ins; delete the built-in registration.

## Lane G2: player session

- **KEEP:** 16a-019, 45a-001, 45a-006, 45a-007, 45a-011, 45a-014, 45a-015, 45a-018, 45a-019, 45a-024, 45a-036, 45a-037, 45a-039, 45a-084, 45b-001 to 45b-010,
  45b-012 to 45b-018, 45b-023, 45b-027, 45b-064, 45b-066, 45b-073, 45b-075.
- **CHANGE:** 45a-010 (local and http addresses only), 45a-038 (one player page, repeated clicks open it once: already done), 45b-061 (messages in the
  one text table).
- **DROP:** 45a-025 (media-key gesture codes), 45b-030 (process-restart snapshot), 48a-040 (the Lua bridge: scripts and custom buttons are a pending decision;
  cut unless you keep them).

## Lane G2b: player controls (only what the flow lists: pause, seek, volume, fullscreen, previous and next, speed)

- **KEEP:** 45b-038, 46a-010, 46a-011, 46a-058, 46a-027, 46a-029, 46a-059.
- **CHANGE:** 45b-031 (volume only, no boost), 45b-051, 46a-001 (controls auto-hide; the top bar never hides), 46a-003 (the same header as the reader),
  46a-004, 46a-014 (a click toggles pause), 46a-008 (speed button only), 46a-006 (no centre transport: bottom bar), 46a-042 (the five kept preferences).
- **DROP:** 45a-034, 45b-032, 45b-033, 46a-009 (brightness and aspect), 45b-024, 46a-015, 46a-016, 46a-018, 46a-007, 46a-039, 48a-025 (touch gestures),
  46a-002 (lock controls), 48a-043, 48a-044 (custom buttons).

## Lane G2c: player sheets (quality and server, audio, subtitles, speed, chapters)

- **KEEP:** 20a-096, 45b-078, 46b-001, 46b-004, 46b-007, 46b-010, 46b-011, 46b-014, 46b-015.
- **CHANGE:** 46b-002 (GTK stock widgets), 46b-012 (one subtitle slot), 46b-013 (chapters sheet kept, small).
- **DROP:** 46b-009, 46b-016, 46b-017 (decoder chips, audio channel chips, statistics page).

## Lane Q: duplicates and shared components

Principle: one way to do each thing. Run last, after the settings removal, and decide each pair once.

- **KEEP (small utilities):** 52a-023, 52a-024, 52a-028, 52a-053, req-platform-008, 13a-014 (merge these two sanitisers into one), 52a-064,
  01a-035, 52a-003 (merge), 05a-023, 06a-005 (one test harness), 51a-043 (English plus a few locales).
- **CHANGE (pick one implementation, delete the other):** the database access layers 01a-014, 03a-033, 04a-008, 05a-001, 06a-002, 06a-003, 06a-004, 05a-022,
  history recording 06a-014 and 08a-019, reactive helpers 10a-002, read and seen status 10a-013 and 10a-014, module skeletons 08a-001, 09a-001, 16a-001,
  42a-001, covers 07a-020 and 07a-007, image loader 01a-029, 34a-023, 34a-055, shared UI components 36a-037 to 36a-040, 36a-047, 36a-048, 36a-069,
  49a-024, 49a-025, 49a-033, 49a-035, 49a-049, 49b-025, 49b-027, 49b-028, 49ba-012, 49ba-016, 49ba-019, 49ba-025, 49ba-026 (adopt the app's existing
  component, delete unused ones; the dead-code lane's list decides).
- **DROP:** 02a-014 (player gesture and decoder preferences), 49a-036, 49a-037, 49a-038, 49a-039, 49a-045 (the settings row primitives; the 8-page settings
  use one row helper), 49b-028, 49a-049 (if unused after the redesign).

## Order of the new lanes

1. **Settings removal** (after your seven decisions): cut 248 preferences, 8 pages; add every cut key to `DROPPED_KEYS`.
2. **Lane R (repository link) and lane E (source registry)** together: restore, library update and browse depend on them.
3. **Player (G1 reduced to five settings, G2, G2b, G2c) and reader (L)**: one after the other in the player files; the reader in parallel.
4. **Browse (K) and downloads (D) and tracking (H)**: in parallel (disjoint folders).
5. **Dedupe (Q)**, then the README and progress table.
6. **Very last:** the Aniyomi to Kurobayomi rename pass, then `PRIVACY.md` and the disclaimer.

## Unfinished parts of the earlier lanes

- **G1 (player settings):** keep only wiring the five kept preferences (volume, decoder choice, subtitle language, audio language, the window choice).
- **L (reader):** the themed top bar like the player (never hidden), left-to-right default, previous on the left, loading spinner, data saver cut.
- **C (entry page):** WebView and share buttons stay, source Settings item stays, external player is a pending decision.
- **O (startup):** verbose logging, migration path, storage tree: keep; sidebar bars from the nav style: drop (tablet UI is gone).
