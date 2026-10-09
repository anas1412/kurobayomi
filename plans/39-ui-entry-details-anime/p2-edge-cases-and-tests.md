# 39-ui-entry-details-anime (part 2): edge cases, failure modes, races, tests

Scope: the seven files listed in `p2-inventory.md`. No `*Test.kt` file lives inside the scope. Upstream tests relevant to this scope are in section 1 (they live in files owned by other areas and are listed so the port keeps their expectations).

Test IDs used here are referenced from `p2-tasks.md` acceptance criteria and from `p2-rust-design.md`.

---

## 1. Upstream tests to port

### 1.1 `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt` (area 07; episode cases only, used by the separator rule)

| Upstream test name (verbatim) | Assertions (verbatim values) | Port as |
|---|---|---|
| `calculateEpisodeGap returns difference` | `calculateEpisodeGap(episode(10.0), episode(9.0)) == 0`; `(10.0, 8.0) == 1`; `(10.0, 8.5) == 1`; `(10.0, 1.1) == 8`; the same four with plain `Double` arguments | `T-EPS-GAP-1`: `gap_numbers(10.0, 9.0) == 0`, `(10.0, 8.0) == 1`, `(10.0, 8.5) == 1`, `(10.0, 1.1) == 8`, and `episode_gap` on `Episode` values |
| `calculateEpisodeGap returns 0 if either are not valid episode numbers` | `calculateEpisodeGap(episode(-1.0), episode(10.0)) == 0`; `(episode(99.0), episode(-1.0)) == 0`; `(-1.0, 10.0) == 0`; `(99.0, -1.0) == 0` | `T-EPS-GAP-2` |

The upstream test asserts `shouldBe 0f` / `shouldBe 1f` (floats) for the `Double` overloads and `shouldBe 0` for the `Episode` overloads. The Rust port returns `i64` and asserts integers.

`missingChaptersCount` / `missingEntriesCount` cases in the same file (for example `listOf(-1.0, 1.0, 2.0, 2.2, 4.0, 6.0, 10.0, 10.0).missingChaptersCount() == 5`) are area 07 and are not part of this scope's tests. They are ported there.

### 1.2 Other upstream test files that exercise code in scope

None found. A search of `app/src/test`, `domain/src/test`, `core/*/src/test` for `AnimeScreenModel`, `EpisodeSorter`, `SeasonSorter`, `applyFilter`, `AnimeTrackInfoDialog`, `getNextUnseen`, `AniChartApi` returned no test. Coverage for this scope is therefore new (section 5).

---

## 2. Defect cross-reference

The behavior document (`p2-behavior.md` section 23) uses numbers 23.1-23.24. The track topic uses D-1..D-12. The airing topic uses A-1..A-6. The related topic uses R-1..R-3. The table maps all of them to one decision.

| Behavior 23.x | Track D-x | Airing A-x | Related R-x | Subject | Decision in Kurobayomi | Test |
|---|---|---|---|---|---|---|
| 23.1 | | | | non-library entries lose view flags on every open | Fix: apply library defaults once at add time; use an in-memory override for non-library entries | T-DEF-1 |
| 23.2 | | | | single toggle writes filtered view into the full list | Fix by construction: full list is state; views computed on read | T-SEL-6 |
| 23.3 | | A-1 | | `trackItems` never populated; manual refresh overwrites airing data | Fix: populate from the tracker observer | T-AIR-4 |
| 23.4 | | | | torrent precheck `!!` throws outside try | Fix: return false when the extension is absent | T-TRK-9 |
| 23.5 | | | | track sheet vs category dialog ordered by timing | Fix: explicit pending-sheet flag | T-LIB-5 |
| 23.6 | | | | duplicate dialog can stay visible | Fix: dismiss, then act | T-LIB-3 |
| 23.7 | | | | season child episodes never auto-downloaded | Keep for v1 (documented); P2 feature in task 022 | T-REF-6 |
| 23.8 | | | | custom interval wheel hidden in release builds | Fix: always enabled | T-REF-8 |
| 23.9 | | | | swipe key names reversed; season default writes the wrong filter | Fix: name by effect; use the downloaded filter for the downloaded default | T-DEF-3, T-FLG-9 |
| 23.10 | | | | episode defaults share keys with manga chapter defaults | Fix: separate namespaces (`p2-data-and-config.md` 4.5) | T-DEF-4 |
| 23.11 | | | | "previous" and "unseen download" use the filtered view | Keep for "previous" (documented), fix "unseen download" to use the full list | T-MRK-4, T-DL-2 |
| 23.12 | | | | success toast shown even when no tracker was updated | Fix: count accepted updates | T-MRK-6 |
| 23.13 | | | | `cancelDownload` mutates a shared object | Fix: copy | T-DL-4 |
| 23.14 | | | | search from other previous screens does nothing | Fix: fall back to global search | T-NAV-2 |
| 23.15 | | | | copy URL unguarded | Fix: guard | T-NAV-4 |
| 23.16 | | | | side effects in `update` lambda | Fix: compute then publish | T-SEL-7 |
| 23.17 | D-8 | | | `lateinit tracking_url` throws inside update | Fix: `Option<String>` | T-TRK-6 |
| 23.18 | D-3 | | | preselect matches null URL | Fix: require Some | T-TRK-5 |
| 23.19 | D-1 | | | stale track object written to remote | Fix: re-read row before write | T-TRK-11 |
| 23.20 | D-4 | | | score not in list stores -1 | Fix: reject | T-TRK-3 |
| 23.21 | D-5 | | | picker date off by a day west of UTC | Fix: `NaiveDate` | T-TRK-2 |
| 23.22 | | | | refresh log/track null (unreachable) | Not fixed; noted | none |
| 23.23 | | | | duplicate check requires favorite | Intended | T-LIB-2 |
| 23.24 | | | | empty-list refresh loop on open | Fix: refresh guard 10 min | T-REF-3 |
| | D-2 | | | `!!` on track and tracker | Fix: `Option` and disabled actions | T-TRK-7 |
| | D-6 | | | unknown service id throws | Fix: error screen | T-TRK-8 |
| | D-7 | | | actions on unbound rows throw | Fix: only bound rows expose actions | T-TRK-10 |
| | D-9 | | | log text says `mangaId` | Fix wording | none |
| | D-10 | | | two different tracker filters (home vs summary) | Fix: one function | T-TRK-12 |
| | D-11 | | | remote and local delete unordered | Fix: remote first, then local, report both | T-TRK-13 |
| | D-12 | | | date stored as UTC wall clock | Fix: local midnight | T-TRK-2 |
| | | A-2 | | Simkl "remove aired" uses list index as offset | Fix: parse dates | T-AIR-7 |
| | | A-3 | | malformed date throws out of airing load | Fix: absent entry | T-AIR-8 |
| | | A-4 | | new HTTP client per call | Fix: shared client | T-AIR-10 |
| | | A-5 | | MAL lookup ignores HTTP status | Fix: check status | T-AIR-9 |
| | | A-6 | | completed + manual overwrites airing | Fix: keep stored unless real episode | T-AIR-4 |
| | | | R-1 | missing source silently disables relations | Fix: typed result | T-REL-5 |
| | | | R-2 | related rows never garbage-collected | Fix: stub marker + GC after 30 days | T-REL-6 |
| | | | R-3 | self-link with different URL spelling kept | Fix: normalize URL | T-REL-4 |

---

## 3. Edge cases by feature

Each row: ID, condition, expected outcome. Tests reference these IDs in `p2-tasks.md`.

### 3.1 Episode list and rows

| ID | Condition | Expected |
|---|---|---|
| E-LIST-1 | empty episode list | `episode_rows` returns empty; no separator; empty-state text "No episodes found" when source has none (refresh error path) |
| E-LIST-2 | single episode number 5, ascending | leading separator count = floor(5) - 1 = 4 before it; trailing: none |
| E-LIST-3 | single episode number 5, descending | leading: none; trailing separator count 4 after it |
| E-LIST-4 | numbers `[-1.0, 2.0]` | `-1.0` row has no gaps around it; separator before `2.0` ascending count `floor(2) - 1 = 1` |
| E-LIST-5 | duplicate numbers `[5, 5]` | gap = -1, no separator |
| E-LIST-6 | fractional `[5.0, 5.5, 6.0]` | gap(6, 5.5) = 0, no separator |
| E-LIST-7 | `[1.0, 4.0]` ascending | separator between with count 2; id `"<id1>-<id4>"` |
| E-LIST-8 | the same list descending | separator between the same pair, count 2; id `"<id4>-<id1>"` (lower is `after`) |
| E-LIST-9 | separator id when lower is absent | `"null-<higherId>"` (matches Kotlin string interpolation) |
| E-LIST-10 | all episodes filtered out | view empty; rows empty; FAB text unchanged |
| E-LIST-11 | episode with `episodeNumber = 0.0` | `is_recognized_number` true; gap from 0.0 to 2.0 = 1 |
| E-LIST-12 | alphabetical sort with equal names | stable order from the input list |
| E-LIST-13 | alphabetical sort with `"Ep 10"`, `"ep 2"` | PRIMARY collation: case-insensitive; `"ep 2" < "Ep 10"` |
| E-LIST-14 | date sort with `dateUpload = -1` on some items | they sort first ascending, last descending |

### 3.2 Filters and sort settings

| ID | Condition | Expected |
|---|---|---|
| E-FILTER-1 | unseen filter EnabledIs | only `!seen` items |
| E-FILTER-2 | unseen filter EnabledNot | only `seen` items |
| E-FILTER-3 | downloaded filter EnabledIs, `downloadedOnly` off, entry not local | only `isDownloaded` items (queue status is not downloaded) |
| E-FILTER-4 | downloaded filter EnabledIs, local anime | all items pass (`isLocal`) |
| E-FILTER-5 | `downloadedOnly` on, per-entry filter Disabled | downloaded filter forced EnabledIs; the per-entry flag is unchanged |
| E-FILTER-6 | setting the same sort key twice | direction flips |
| E-FILTER-7 | new sort key | ascending |
| E-FILTER-8 | filter flags word has the unused bit pattern 0x6 (both UNSEEN bits) | `tri()` returns Disabled (matches `when` with no match) |
| E-FILTER-9 | sort key bits | unreachable: `EPISODE_SORTING_MASK` (0x600) has exactly four values, all handled by `getEpisodeSort`, so the Kotlin `else -> throw NotImplementedError` cannot fire from stored data. The Rust `SortKey` enum is exhaustive; a test asserts all four masks map to a variant |

### 3.3 Selection

| ID | Condition | Expected |
|---|---|---|
| E-SEL-1 | long press first item | anchors = [i, i]; only item selected |
| E-SEL-2 | long press item 5 after 2 | range 3..4 selected too; anchors [2, 5] |
| E-SEL-3 | long press item 0 when anchors [2, 5] | range 1..1 (`(0+1) until 2`) selected; anchors [0, 5] |
| E-SEL-4 | tap (not long press) item 3 when none selected | anchors become `[-1, 3]`: the plain-tap branch moves only `pos[1]` (`3 > pos[1] = -1`), and `pos[0]` stays `-1`. Quirk, kept and tested (T-SEL-3) |
| E-SEL-5 | toggle an already selected item to selected | no change (early return) |
| E-SEL-6 | select all with a filter hiding 3 episodes | all episodes in the full list selected, hidden included (documented) |
| E-SEL-7 | invert with none selected | all selected |
| E-SEL-8 | anchor index stale after a filter change | Proposal: clear anchors on any filter or sort change (`set_all(false)` semantics on anchors only) |
| E-SEL-9 | long-press deselect | no anchor update (observed); test documents it |
| E-SEL-10 | bulk action clears selection first (`toggleAllSelection(false)`) | selection empty before the DB write completes |

### 3.4 Seasons

| ID | Condition | Expected |
|---|---|---|
| E-SEA-1 | season with `total = 0` | `seen == true`, `unseen == 0`; "unseen" filter EnabledIs hides it |
| E-SEA-2 | `seenCount > total` (corrupt) | `unseen_count` negative; overlay shows negative; Proposal: clamp to 0 in the view only |
| E-SEA-3 | season sort SEASON with `seasonNumber = -1.0` for unknown | sorts first ascending |
| E-SEA-4 | alphabetical tie-break with reversed primary | tie-break always ascending |
| E-SEA-5 | `showContinueOverlay` with `fetchType = Seasons` child | false |
| E-SEA-6 | downloaded filter using unclamped count when overlay off | filter still uses the true count |
| E-SEA-7 | `SeasonDisplayMode` value 4 stored | `TryFrom` returns Err; Kotlin throws. Rust shows the default (CompactGrid) and logs a warning |
| E-SEA-8 | grid size 15 | valid; items per row slider max 15 |
| E-SEA-9 | season refresh with `UnmeteredSource` | parallel fetch of all eligible seasons; one failure does not stop others |
| E-SEA-10 | season refresh with metered source and cancel mid-way | stops before the next season; no partial episode rows for the stopped season |

### 3.5 Library membership

| ID | Condition | Expected |
|---|---|---|
| E-LIB-1 | add with default category set and existing | favorite true, category set to the default, no dialog |
| E-LIB-2 | add with a duplicate (favorite, same lowercase title, different id) | DuplicateAnime dialog; no favorite change |
| E-LIB-3 | "Add anyway" from duplicate dialog | dialog closes first, then add proceeds (fix 23.6) |
| E-LIB-4 | add with no categories | favorite true, category set empty (Default) |
| E-LIB-5 | add with a category choice and the user dismisses the category dialog | not favorite; track sheet opens only if the auto-open flag is on (fix 23.5) |
| E-LIB-6 | add when `setAnimeCategories` fails | favorite still set (Kotlin order: category write is not awaited). Proposal: await category write, then favorite; on category failure keep favorite false and show error |
| E-LIB-7 | remove with downloads present, user chooses Delete | snackbar with action; downloads deleted; covers removed |
| E-LIB-8 | remove with downloads, user dismisses snackbar | downloads kept |
| E-LIB-9 | remove when `awaitUpdateFavorite` returns false | nothing changes; no prompt |
| E-LIB-10 | enhanced tracker match fails during add | error logged at WARN; add still succeeds (AddAnimeTracks.kt:99-104) |

### 3.6 Source refresh

| ID | Condition | Expected |
|---|---|---|
| E-REF-1 | open an entry with zero stored episodes, source returns zero | refresh runs; `NoEpisodesException` -> "No episodes found" snackbar |
| E-REF-2 | source throws generic exception | `formattedMessage` shown; state back to not refreshing |
| E-REF-3 | open an entry with zero episodes twice in 10 min | second open does not refetch (guard, proposal fix 23.24) |
| E-REF-4 | manual refresh with new episodes and auto-download on | new episodes filtered by `FilterEpisodesForDownload` then queued |
| E-REF-5 | open (automatic) with new episodes | no auto-download (manual only, as observed) |
| E-REF-6 | season child episodes new on refresh | not downloaded (23.7); episodes stored |
| E-REF-7 | cover URL empty in remote | cover kept (rule 6.5.1) |
| E-REF-8 | `fetchInterval` custom set to 0 (Default) | smart interval; `Default` label; release gating removed (23.8) |
| E-REF-9 | interval index 28 | stored -28; expected update in 28 days |
| E-REF-10 | interval change for a completed entry | stored; the presentation shows no expected update |
| E-REF-11 | refresh while the screen is closed | task finishes; results stored; no UI update (`launchNonCancellable`) |
| E-REF-12 | torrent source with torrent server disabled | no start attempt |

### 3.7 Mark seen and tracking prompt

| ID | Condition | Expected |
|---|---|---|
| E-MRK-1 | mark seen empty list | selection cleared; no DB write; no prompt |
| E-MRK-2 | mark seen all already seen | DB filter yields empty; `Result::NoEpisodes`; prompt logic still runs (as observed) — Proposal: skip prompt when nothing changed |
| E-MRK-3 | mark unseen | `last_second_seen` reset to 0 for affected episodes |
| E-MRK-4 | mark previous with filter hiding some | only visible previous are marked (23.11, documented) |
| E-MRK-5 | `removeAfterMarkedAsRead` on and mark seen | downloads deleted for the marked episodes |
| E-MRK-6 | ALWAYS auto-track with one tracker offline | delayed update stored; success toast still shown (23.12); Proposal: show "queued" text when offline |
| E-MRK-7 | ASK and user confirms | `trackEpisode` called; no success toast |
| E-MRK-8 | NEVER | no tracker refresh call |
| E-MRK-9 | `maxEpisodeNumber` not greater than any `lastEpisodeSeen` | no prompt |
| E-MRK-10 | mark seen when `hasLoggedInTrackers` is false | no tracker calls |

### 3.8 Downloads

| ID | Condition | Expected |
|---|---|---|
| E-DL-1 | swipe Download on ERROR item | START_NOW |
| E-DL-2 | next-N with a filter hiding episodes | only visible unseen (23.11, documented); Proposal: full list for "download next N" |
| E-DL-3 | START_NOW with several selected | no-op |
| E-DL-4 | cancel queued download | item NOT_DOWNLOADED; queue entry removed; the shared object is not mutated (23.13 fix) |
| E-DL-5 | first download of non-library entry | "Add anime to library?" snackbar once per model instance (`hasPromptedToAddBefore`) |
| E-DL-6 | `removeAfterMarkedAsRead` with an episode whose download is in the queue | queue removal then file deletion (downloader area) |
| E-DL-7 | download progress arrives for an episode not in the list | ignored |
| E-DL-8 | delete a downloaded episode whose directory is missing | no error to the user; cache updated |
| E-DL-9 | local anime download action | no download actions shown (`isLocalOrStub`) |
| E-DL-10 | `filterDownloadedEpisodes` on a local anime | returns all episodes |

### 3.9 Tracking sheet

| ID | Condition | Expected |
|---|---|---|
| E-TRK-1 | open sheet with no logged-in trackers | `onTrackingClicked` opens Settings > Tracking instead (AnimeScreen.kt:166-171) |
| E-TRK-2 | date picker with start after finish | disallowed (`is_selectable`) |
| E-TRK-3 | date picker future date | disallowed |
| E-TRK-4 | date picker on the day boundary in a UTC-west zone | the displayed day equals the picked day (D-5 fix) |
| E-TRK-5 | status change to completed with total 24 | last-seen 24 |
| E-TRK-6 | episode set to total | status completed; finish date set to now |
| E-TRK-7 | score selection not in list | error; remote not updated (D-4) |
| E-TRK-8 | search returns zero results | "No results found" state |
| E-TRK-9 | search result missing `tracking_url` | error entry for that result, not a crash (D-8) |
| E-TRK-10 | preselect with null `currentUrl` | nothing preselected (D-3) |
| E-TRK-11 | confirm bind with private true on a tracker without private support | the UI does not offer the checkbox (`supports_private`) |
| E-TRK-12 | remove tracking with remote delete on a non-deletable tracker | checkbox hidden; local only |
| E-TRK-13 | remote delete fails | logged; local row already removed (D-11 order) |
| E-TRK-14 | refresh on open with one tracker failing | toast with service name; other tracks still updated |
| E-TRK-15 | toggle private | remote update; on failure toast; local flag unchanged |
| E-TRK-16 | status edit with stale local row (another device changed it) | re-read before write; the remote value is not overwritten by stale fields (D-1) |

### 3.10 Airing and related

| ID | Condition | Expected |
|---|---|---|
| E-AIR-1 | concurrent write of viewer flags | atomic per-field SQL; no lost update |
| E-AIR-2 | skip-intro stored 0, not disabled | dialog shows 85 (default); OK stores chosen value |
| E-AIR-3 | skip-intro set to 0 | disable bit set; button hidden |
| E-AIR-4 | manual refresh on tracked anime | airing data reloaded from the tracker track list, not overwritten with (1, 0) (A-1) |
| E-AIR-5 | AniList `nextAiringEpisode: null` | (1, 0); row hidden (no airing) |
| E-AIR-6 | MAL id with no AniList match | `getAlIdFromMal` = 0; AniList query for id 0 returns `Media: null`; (1, 0) |
| E-AIR-7 | Simkl feed with an unparseable date | that entry ignored; no crash (A-3) |
| E-AIR-8 | Simkl id not present in any calendar | (1, 0) |
| E-AIR-9 | network unavailable | (1, 0) and no crash |
| E-AIR-10 | airing time in the past | countdown negative; row hidden (Proposal: hide when <= 0) |
| E-REL-1 | source does not support related anime | no sync; empty groups |
| E-REL-2 | TTL exactly 7 days | refetch (`now - last < TTL` is false at equality) |
| E-REL-3 | group with 15 entries | 10 kept |
| E-REL-4 | related entry with the same URL as the anime | removed from its group |
| E-REL-5 | source missing in registry | `SourceMissing` result; no relations change |
| E-REL-6 | related rows older than 30 days, not favorite | GC candidates (proposal) |

### 3.11 UI state and dialogs

| ID | Condition | Expected |
|---|---|---|
| E-UI-1 | dialog open, a new refresh finishes | dialog remains; the list behind updates |
| E-UI-2 | two dialogs requested in one dispatch | the last one wins (single slot) |
| E-UI-3 | two snackbars requested in a row | the second replaces the first; the first's action is dropped (Compose would queue; documented difference) |
| E-UI-4 | page closed with a snackbar visible | the revealer is destroyed with the page |
| E-UI-5 | dispatch while Loading | ignored (`updateSuccessState` semantics) |
| E-UI-6 | window resized narrow (< 360 px) | the action row wraps; episode rows remain readable (minimum width 280 px) |
| E-UI-7 | keyboard: Enter on an episode row | opens the episode (internal player unless the external setting is on) |
| E-UI-8 | keyboard: Ctrl+A | select all (full list) |
| E-UI-9 | keyboard: Escape with selection active | clears selection before leaving the page |

### 3.12 Concurrency and storage

| ID | Condition | Expected |
|---|---|---|
| E-CONC-1 | refresh started twice quickly | second call joins the first (single-flight per anime id); Proposal |
| E-CONC-2 | DB change during refresh | model reloads after refresh completes; no stale overwrite |
| E-CONC-3 | page opened for an id that was deleted | load fails with NotFound; page shows "Entry not found" |
| E-CONC-4 | two windows on the same anime | both render; writes are last-write-wins at the flag level |
| E-STORE-1 | flags word with unknown high bits | preserved on write (`set_flag` masks) |
| E-STORE-2 | `calculate_interval` -28 | custom interval; `expectedNextUpdate` computed by the interval area |
| E-STORE-3 | episode number stored as NaN | treated as unknown (`-1`) by the importer |

---

## 4. Failure modes

| Mode | Where it arises | Visible result | Recovery |
|---|---|---|---|
| Source HTTP 500 | `getAnimeEpisodeUpdate` | snackbar with `formattedMessage` | none automatic; refresh button retries |
| Source returns zero episodes | sync | "No episodes found" | refresh on next open (guard 10 min) |
| Source returns zero seasons | sync | "No seasons found" | same |
| Source missing (uninstalled extension) | `sourceManager.getOrStub` | `StubAnimeSource`: no refresh, no related | user reinstalls extension |
| DB write failure in `updateAllEpisodes` | `SetSeenStatus` | `Result.InternalError`, ignored by the model (behavior 1.2) | Proposal: show a snackbar "Could not mark episodes" |
| DB locked (WAL busy) | any write | error from rusqlite `SQLITE_BUSY` | busy timeout 5000 ms, then error snackbar |
| Tracker token expired | `refresh`, `update` | toast "<service> error: <message>" | user re-logs in |
| Tracker search timeout | `searchAnime` | `Result.failure` shown as error text | retry by editing query |
| AniList rate limit (HTTP 429) | airing | `(1, 0)` (A-5 proposal: distinguish) | next refresh |
| Simkl unreachable | airing | `(1, 0)` | next refresh |
| Torrent server binary missing | `ensure_torrent_server` | "Torrent server not installed" toast | install server |
| External player not configured | `open_episode(external=true)` | toast "No external player configured" | configure player |
| Download directory unwritable | downloader | download state ERROR per item | user fixes permissions; START retries |
| Corrupt flags word (bit 63 set) | read | unchanged behavior; unknown bit preserved | none |
| Clock skew (system time in the past) | related TTL, airing | TTL check passes early or late | none needed |
| Out-of-memory with 10k episodes | list | not expected (ListView virtualization) | none |

---

## 5. Race conditions

| ID | Race | Kotlin behavior | Rust behavior |
|---|---|---|---|
| RC-1 | Init subscription emits before the first `Success` state (line 212 vs 253) | dropped by `updateSuccessState` | the Rust model applies the subscription only after `load` returns (single owner) |
| RC-2 | `toggleFavorite` add path, category dialog vs track sheet | timing decides (23.5) | explicit pending flag |
| RC-3 | `markEpisodesSeen` reads `hasLoggedInTrackers` from state while another write runs | reads a possibly stale flag | read inside the same task after the DB write |
| RC-4 | selection mutation inside `MutableStateFlow.update` retried | side effects duplicated (23.16) | pure transform then publish |
| RC-5 | `isFromChangeCategory` read on UI and written on IO | no synchronization | owned by the GTK thread |
| RC-6 | download event for an item removed by refresh | index lookup fails, ignored | same |
| RC-7 | two `setRemote*` writes for the same track in quick succession | last write wins with stale fields (D-1) | re-read before each write, serialize writes per track id with a `tokio::sync::Mutex` keyed by track id |
| RC-8 | `launchNonCancellable` write after page close | completes | completes; UI update dropped via weak handle |
| RC-9 | `updateAiringTime` from observer and manual refresh simultaneously | both write | serialize airing writes per anime id |
| RC-10 | `refreshTrackers` on home sheet open during an auto-track | two refreshes of the same track | single-flight per track id |

---

## 6. New tests to write (kuro-core unless noted)

### 6.1 Flags (`kuro-core/tests/flags.rs`)

- `T-FLG-1` episode flags: each setter round trip (unseen, downloaded, bookmarked, fillermarked, sort, direction, display, previews, summaries) on a word with all other bits set; other bits unchanged.
- `T-FLG-2` `set_flag` equals Kotlin `setFlag` for 1000 random (word, flag, mask) triples where `flag & !mask == 0` (generated with a fixed seed); reference values computed by a tiny Kotlin-equivalent function in the test.
- `T-FLG-3` tri-state readers for all 27 combinations of the three-bit groups that are valid, and the invalid pattern 0x6 -> Disabled.
- `T-FLG-4` sort flip: same key flips direction; new key sets ascending (`set_sort` table).
- `T-FLG-5` season grid mode values 0..3 and rejection of 4..7 (`TryFrom` error).
- `T-FLG-6` season grid size 0..15 round trip.
- `T-FLG-7` season overlay booleans: each bit toggled independently.
- `T-FLG-8` season filter values: each of the six filters set to each of three states.
- `T-FLG-9` season default downloaded filter copies `seasonDownloadedFilterRaw` (fix 23.9 regression).
- `V-1` viewer masks: `AIRING_TIME_MASK == 0x0FFFFFFFF000000`, `AIRING_EPISODE_MASK == 0xFFFF00`, `INTRO_DISABLE_BIT == 1 << 56`, `INTRO_MASK == 0xFF` (values from `Anime.kt:310-313`).
- `V-2` viewer round trip: skip intro 0..255 and airing (episode 0..65535, time 0..2^32-1) survive a write of each field alone.
- `V-3` skip intro 0 sets the disable bit; nonzero clears it.

### 6.2 Episodes (`kuro-core/tests/episodes.rs`)

- `T-EPS-GAP-1` and `T-EPS-GAP-2`: upstream cases from section 1.1.
- `T-EPS-SORT-1`: each sort key x direction against the table in topic episode-list 2.2 (oracle values written out in the test).
- `T-EPS-SORT-2`: source inversion regression (desc source = ascending `source_order`).
- `T-EPS-SORT-3`: alphabetical PRIMARY collation (E-LIST-13).
- `T-EPS-FILT-1`: each tri-state x predicate; the four-filter order (sequential filter; result independent of order because each is a pure predicate; assert the same result for permutations).
- `T-EPS-FILT-2`: downloaded-only override and local override (E-FILTER-4, E-FILTER-5).
- `T-EPS-SEP-1`: separator generator for ascending, descending, leading, trailing, `-1.0`, duplicates, fractional, empty, single (E-LIST-1 to E-LIST-9).
- `T-EPS-NEXT-1`: `next_unseen` ascending and descending; all seen; none seen; filtered view.
- `T-EPS-NEXT-2`: both overloads give the same result for the same inputs.
- `T-EPS-DL-1`: `filter_downloaded` for local and non-local.

### 6.3 Selection (`kuro-core/tests/selection.rs`)

- `T-SEL-1` table test: for each row of section 5.2's algorithm, the (anchors before, action, view before) -> (anchors after, selected set after). At least 16 rows covering each branch.
- `T-SEL-2` range selection in both directions and the EMPTY range inside the span.
- `T-SEL-3` tap-select and tap-deselect anchor moves.
- `T-SEL-4` select all and invert on the full list (E-SEL-6, E-SEL-7).
- `T-SEL-5` ids and flags stay consistent after every operation (property test with proptest, 1000 cases).
- `T-SEL-6` regression for 23.2: after a toggle the full list still has every episode.
- `T-SEL-7` regression for 23.16: the transform is pure (calling it twice on the same input gives the same output).

### 6.4 Seasons (`kuro-core/tests/seasons.rs`)

- `T-SEA-1` filter truth table (six filters x three states) on fixtures.
- `T-SEA-2` sort keys and direction; default SOURCE + desc order (E-SEA-*).
- `T-SEA-3` overlay gating: hidden values are -1 or empty; filters use true counts (E-SEA-6).
- `T-SEA-4` `show_continue` conditions (E-SEA-5).
- `T-SEA-5` `seen` for zero totals (E-SEA-1).

### 6.5 Entry model (`kuro-core/tests/entry_model.rs`, with fake `Services`)

- `T-MDL-1` load: non-library entry applies defaults once per open (fixed in Kurobayomi: applied at add time only; test asserts zero default writes on open of a non-library entry).
- `T-MDL-2` init refresh decision: `needRefreshInfo`, `needRefreshEpisode`, `needRefreshSeason` for combinations of `initialized`, empty lists, fetch type.
- `T-MDL-3` `isRefreshingData` true until the refresh completes, including on error.
- `T-MDL-4` dialog single-slot behavior (E-UI-2) and dismiss.
- `T-MDL-5` Loading state ignores dispatches (E-UI-5).
- `T-DEF-1` opening a non-library entry writes no library defaults to its row (fix 23.1; in Kurobayomi defaults are applied only at add time, or as an in-memory override for the session).
- `T-DEF-3` `save_as_default` for seasons copies the downloaded filter from the downloaded field (fix 23.9); same test as T-FLG-9 at the model level.
- `T-DEF-4` episode and manga default keys do not collide: writing the episode display or sort default leaves the manga keys unchanged (fix 23.10; `settings` table namespaces in `p2-data-and-config.md` section 4.5).

### 6.5a Navigation and search routing (`kuro-ui` routing layer, testable through a fake page stack)

- `T-NAV-1` search from Home pops and calls the library search (AnimeScreen.kt:487-490 behavior).
- `T-NAV-2` search from any other previous screen falls back to global search instead of doing nothing (fix 23.14, E-search fallback).
- `T-NAV-3` genre search from a Browse screen with an HTTP source pops and calls `searchGenre` (AnimeScreen.kt:513-515).
- `T-NAV-4` copy URL with a failing `getAnimeUrl` shows an error toast and copies nothing (fix 23.15).

### 6.6 Library, refresh, mark seen, downloads (`kuro-core/tests/library.rs`, `refresh.rs`, `marks.rs`, `downloads.rs`)

- `T-LIB-1` add with each category branch (default category, no category, choose) calls the expected services in order.
- `T-LIB-2` duplicate detection requires favorite and lowercase equality, excludes self.
- `T-LIB-3` "Add anyway" closes the dialog before the add (23.6).
- `T-LIB-4` remove with downloads prompts; Delete triggers deletion; dismiss keeps downloads.
- `T-LIB-5` pending track sheet opens exactly once after category dismiss when the auto flag is on (23.5).
- `T-REF-1` `NoEpisodesException` -> "No episodes found"; `NoSeasonsException` -> "No seasons found".
- `T-REF-2` other errors -> formatted text; `isRefreshingData` reset.
- `T-REF-3` refresh guard for zero-episode entries (E-REF-3).
- `T-REF-4` auto-download only on manual refresh and only for new episodes (E-REF-4, E-REF-5).
- `T-REF-5` cover rules (6.5 of behavior): all five branches for `coverLastModified`, with custom cover kept.
- `T-REF-6` season child episodes are stored and not downloaded (23.7 documented behavior).
- `T-REF-7` season list refresh fetches only `Episodes` children that are not completed or never updated (E-SEA-*).
- `T-REF-8` interval: index 0 gives 0; index 28 gives -28; release gating absent (23.8).
- `T-MRK-1` mark seen clears the selection even for an empty list (E-MRK-1).
- `T-MRK-2` unseen resets `last_second_seen` (E-MRK-3).
- `T-MRK-3` removal of downloads after read (E-MRK-5).
- `T-MRK-4` mark previous uses the visible view (E-MRK-4).
- `T-MRK-5` prompt conditions (E-MRK-7..10).
- `T-MRK-6` success toast only when at least one tracker accepted (23.12, E-MRK-6).
- `T-DL-1` action mapping for swipe Download (E-DL-1).
- `T-DL-2` next-N and unseen selection (E-DL-2 with fix).
- `T-DL-3` START_NOW with multi-select is a no-op (E-DL-3).
- `T-DL-4` cancel does not mutate the shared download (23.13).
- `T-DL-5` first download prompt once (E-DL-5).
- `T-DL-6` download state precedence: queue beats downloaded (13.5 of behavior).

### 6.7 Tracking (`kuro-core/tests/tracking.rs`)

- `T-TRK-1` `set_last_episode_seen` table (0 -> n, rewatching, total reached, zero).
- `T-TRK-2` date rules and wall-clock conversion; UTC-west regression (23.21 / D-5 / E-TRK-4).
- `T-TRK-3` score: index lookup; missing string rejected (D-4 / E-TRK-7).
- `T-TRK-4` episode range `0..=total` and `0..=10000` (section 2.5 of the track topic).
- `T-TRK-5` preselect requires Some URL (D-3 / E-TRK-10).
- `T-TRK-6` result without URL is a per-row error (D-8 / E-TRK-9).
- `T-TRK-7` unbound row actions are unavailable (D-2).
- `T-TRK-8` unknown service renders an error page (D-6).
- `T-TRK-9` torrent precheck with missing extension returns Ok (23.4 / E-REF-12 family).
- `T-TRK-10` actions only on bound rows (D-7).
- `T-TRK-11` status write re-reads the local row (D-1 / E-TRK-16) with a fake repository that changes the row between calls.
- `T-TRK-12` one tracker filter function for home and summary (D-10).
- `T-TRK-13` remote delete before local delete; both attempted; errors reported (D-11 / E-TRK-13).
- `T-TRK-14` bind: leading-run rule (gap stops the run); start date only when unset (7.4 of the track topic).
- `T-TRK-15` `set_private` error keeps the local flag (E-TRK-15).

### 6.8 Airing and related (`kuro-core/tests/airing.rs`, `related.rs`)

- `T-AIR-1` decision table (completed + automatic = no network; completed + manual = network; first AniList/MAL/Simkl with a track wins; no track = (1, 0)).
- `T-AIR-2` AniList parse with `nextAiringEpisode: null` gives (1, 0) (fixture: the live response from section 2.5 of the airing topic).
- `T-AIR-3` AniList parse with values gives (episode, airingAt seconds).
- `T-AIR-4` manual refresh does not overwrite airing data when the tracker list is non-empty; fixture with a tracked AniList entry (A-1 / A-6 / E-AIR-4).
- `T-AIR-5` `airing_time_ms` may be negative (E-AIR-10).
- `T-AIR-6` MAL id absent gives 0 and the AniList query for 0 yields (1, 0) (E-AIR-6).
- `T-AIR-7` Simkl offset regression: an id occurring before the first upcoming entry is still found (A-2).
- `T-AIR-8` malformed date is ignored, others still parsed (A-3 / E-AIR-7).
- `T-AIR-9` HTTP non-200 on MAL lookup gives 0 (A-5).
- `T-AIR-10` one `reqwest::Client` for the process (A-4), asserted by a counter in a fake.
- `T-REL-1` TTL boundary (E-REL-2).
- `T-REL-2` dedupe, self-exclusion, max 10, empty groups dropped (E-REL-3, E-REL-4).
- `T-REL-3` unsupported source: no network (E-REL-1).
- `T-REL-4` URL normalization for self-exclusion (R-3).
- `T-REL-5` missing source typed result (E-REL-5).
- `T-REL-6` GC candidates after 30 days, not favorite, not in use (R-2).

### 6.9 UI (`kuro-ui`, optional headless with `xvfb-run -a cargo test -p kuro-ui`)

- `T-UI-1` snackbar replaces the previous message and auto-hides after 4000 ms (E-UI-3).
- `T-UI-2` episode row binding shows bookmark and download icons per state (behavior 7 and 13).
- `T-UI-3` season grid min-columns follows the size setting.
- `T-UI-4` keyboard: Enter opens, Ctrl+A selects all, Escape clears (E-UI-7..9).
- `T-UI-5` no `GtkWidget` is touched from a non-main thread (checked by a debug assertion in `apply`).

---

## 7. Fixtures

All fixtures are plain JSON or Rust builders in `kuro-core/tests/fixtures/`.

| Fixture | Content | Used by |
|---|---|---|
| `episodes_gap.json` | episodes numbered `[1, 2, 2.5, 4, 4, 5.5, 6, 9, -1]`, `source_order` 0..8, mixed seen/bookmark/fillermark, two downloaded | T-EPS-SEP-1, T-EPS-FILT-1, T-LIST-* |
| `episodes_upload.json` | 6 episodes with `date_upload` values including `-1` | T-EPS-SORT-1 |
| `episodes_names.json` | names `["Ep 10", "ep 2", "Ep 2", "Special", "éclair"]` | T-EPS-SORT-3 |
| `seasons_mixed.json` | 5 seasons with totals `[0, 3, 12, 12, 24]`, seen counts, status values `[0, 1, 2]`, `season_number` `[-1, 1, 2, 2.5, 3]` | T-SEA-* |
| `tracks_anilist.json` | one AniList track, total 24, last seen 12, status watching | T-TRK-*, T-AIR-4 |
| `tracks_none.json` | empty track list | T-AIR-1 |
| `anilist_airing_null.json` | `{"data":{"Media":{"nextAiringEpisode":null}}}` | T-AIR-2 |
| `anilist_airing_value.json` | `{"data":{"Media":{"nextAiringEpisode":{"episode":14,"airingAt":1791000000}}}}` (value chosen for the test) | T-AIR-3 |
| `anilist_mal_lookup.json` | `{"data":{"Media":{"id":1}}}` and `{"data":{"Media":null}}` | T-AIR-6, T-AIR-9 |
| `simkl_anime_sample.json` | 4-element array in the live schema (section 2.6 of the airing topic): two entries for simkl id 3194924 (one past, one upcoming), one malformed date entry, one entry with `ids.mal` null | T-AIR-7, T-AIR-8 |
| `related_groups.json` | 3 groups, one with 12 entries, one containing the anime itself by URL | T-REL-2 |
| `duplicate_titles.json` | anime rows with the same lowercase title, one not favorite, one with a different title | T-LIB-2 |

Fixture rules:
- Times are fixed (`now_ms = 1_790_000_000_000`) and passed explicitly to pure functions.
- Dates in fixtures use RFC 3339 with explicit offsets.
- No fixture uses a real user's data.

---

## 8. Test data builders (Rust)

```rust
pub fn ep(id: i64, number: f64) -> Episode { /* defaults as Episode.create() in Kotlin */ }
pub fn item(id: i64, number: f64) -> EpisodeItem { EpisodeItem { episode: ep(id, number), download_state: DownloadState::NotDownloaded, download_progress: 0, selected: false } }
pub fn anime_with(episode_flags: u64, season_flags: u64, viewer: u64) -> AnimeRef { /* ... */ }
pub struct FakeServices { pub calls: std::sync::Mutex<Vec<String>>, /* scripted responses */ }
```

`FakeServices` records calls as strings in order (for example `"set_favorite(1,true)"`, `"set_categories(1,[3])"`) so that ordering tests (T-LIB-1, T-LIB-5) can assert exact sequences.

---

## 9. Acceptance summary

A change in this scope is accepted when:
1. Every test ID in sections 6.1-6.9 that is marked as P0 in `p2-tasks.md` passes on `cargo test -p kuro-core`.
2. The upstream tests in section 1 pass under their Rust names.
3. Every defect in section 2 marked "Fix" has a regression test that fails on the Kotlin-equivalent behavior and passes on the Rust behavior.
4. `cargo clippy -p kuro-core -- -D warnings` passes, including `await_holding_refcell_ref`.
5. The GTK page opens a real anime from a fixture database, shows episodes with separators, toggles a filter, marks an episode seen, and adds the entry to the library, with no console warnings from GTK.
