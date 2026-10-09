# Topic: tracking sheet and sub-screens (navigation, state, and each contract)

Deep dive for `MangaTrackInfoDialog.kt` (MTID) and its ScreenModels. Full rules are in p2-behavior.md section 5 and 8; this file gives the navigation graph, the state each screen holds, and the exact write each confirm produces.

Sources: `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackInfoDialog.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/track/MangaTrackItem.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/manga/MangaScreenModel.kt` (tracking parts), `app/src/main/java/eu/kanade/tachiyomi/data/track/MangaTracker.kt`.

## 1. Navigation graph (upstream: navigator push/pop)

```
Details page (MangaScreenModel.Dialog.TrackSheet)
  └─ MangaTrackInfoDialogHomeScreen (home list, one row per logged-in manga tracker)
       ├─ TrackStatusSelectorScreen        confirm: setRemoteMangaStatus      -> pop
       ├─ TrackChapterSelectorScreen       confirm: setRemoteLastChapterRead  -> pop
       ├─ TrackScoreSelectorScreen         confirm: setRemoteScore            -> pop
       ├─ TrackDateSelectorScreen(start)   confirm: setRemoteStartDate        -> pop
       │     └─ TrackDateRemoverScreen     confirm: setRemoteStartDate(0)     -> popUntil(home)
       ├─ TrackDateSelectorScreen(finish)  confirm: setRemoteFinishDate       -> pop
       │     └─ TrackDateRemoverScreen     confirm: setRemoteFinishDate(0)    -> popUntil(home)
       ├─ TrackServiceSearchScreen         confirm: register(selected)        -> pop
       │     (auto-search on open when the query is non-blank)
       ├─ TrackerMangaRemoveScreen         OK: unlink locally, optional remote delete -> pop
       └─ (enhanced tracker "new search")  no screen: match + register in place
```

Home-row side actions with no screen: open in browser (`openInBrowser`), copy link (`copyToClipboard(url, url)`), toggle private (`setRemotePrivate`).

Rules common to every confirm (MTID:307-309, 327-333, 363-367, 425-430, 545-551, 644-650, 710-711, 833-835): the write runs in `launchNonCancellable` (it survives the page closing), the picker closes immediately, and there is no progress indicator and no in-picker error. Errors inside the write are logged by the tracker layer (area:27).

## 2. Home screen model

State: `trackItems: List<MangaTrackItem>` (MTID:280-283), default empty.

On open (MTID:207-225):
1. Refresh every linked track (`RefreshMangaTracks.await(mangaId)`), toasting each failure as `track_error(name, message)`.
2. Subscribe to the track flow (`GetMangaTracks.subscribe(mangaId)`), map to items, emit on change.

Mapping (MTID:268-278):
1. Start from `TrackerManager.loggedInTrackers()` filtered to `MangaTracker`.
2. For each, pair with the linked track (`find { it.trackerId == service.id }`) or `null`.
3. Keep only trackers where `(tracker as? EnhancedMangaTracker)?.accept(source) ?: true`, where `source = MangaSourceManager.getOrStub(sourceId)`.

Worked example: logged in to trackers 1 (manga), 2 (anime only), 3 (manga, enhanced, rejects this source). Linked tracks exist for 1 only.
- Result: row(1, track), row(3 is dropped by accept). Tracker 2 is not a manga tracker and never appears.
- Row for tracker 1 with no link would be `row(1, null)` if it were logged in but unlinked.

Row availability (proposal, fixes E-TRK-1; see UI parity 4.1):

| Action | Needs | Notes |
|---|---|---|
| Status, chapter, score | track != null | upstream `it.track!!` |
| Start date, end date | track != null | |
| New search | none | enhanced: auto-match (section 4) |
| Remove | track != null | |
| Private toggle | track != null and supports_private | upstream crashes with null track (E-TRK-1) |
| Open in browser, copy link | track != null and remoteUrl not blank | |

## 3. Picker contracts (exact values)

### 3.1 Status (MTID:287-340)

- Options: `getStatusListManga()` (tracker-specific Long ids, order as returned).
- Labels: `getStatusForManga(id)` (nullable StringResource).
- Initial: `track.status`.
- Confirm writes `setRemoteMangaStatus(track.toDbTrack(), selection)`.

### 3.2 Chapter (MTID:342-403)

- Range: `0 .. endRange` inclusive, `endRange = totalChapters` if > 0 else `10000`.
- Initial: `track.lastChapterRead.toInt()` (truncation; 12.9 shows 12).
- Confirm writes `setRemoteLastChapterRead(track, selection)` with an Int.
- Upstream UI: `TrackItemSelector(isManga = true)`. Port: spin button (UI parity 4.3).

### 3.3 Score (MTID:405-457)

- Options: `getScoreList()` (strings).
- Initial: `displayScore(track)` (string, tracker format).
- Confirm writes `setRemoteScore(track, selection)` with the string, not a number.

### 3.4 Date (MTID:459-589)

- Title: "Start date" (`track_started_reading_date`) or "Finish date" (`track_finished_reading_date`).
- Initial: `track.startDate` or `track.finishDate` if non-zero, else now. Converted: stored UTC millis from the picker are converted to the system zone before writing.
- Constraints:
  - Day: not after today; for start with a finish set, not after the finish day; for finish with a start set, not before the start day.
  - Year: not after the current year; same ordering on the year.
- Remove visible only when the stored date is non-zero.

Worked example (today = 2026-06-01): track has start 2026-03-10 and finish 0.
- Start picker: days after 2026-06-01 are disabled. No finish, so no upper bound from finish.
- Finish picker: days before 2026-03-10 are disabled; days after 2026-06-01 are disabled.
- Writing finish 2026-03-12 stores local midnight 2026-03-12 as epoch ms.

### 3.5 Remover (MTID:591-677)

- Title "Remove date?" (`track_remove_date_conf_title`).
- Text "This will remove your previously selected start date from {name}" or the finish variant.
- Remove: `setRemoteStartDate(track, 0)` or finish; then `popUntil(home)`.

### 3.6 Tracker search (MTID:679-770)

State machine:

```
Idle --(init, query not blank)--> Loading --(result)--> Done(results, selected = match on current URL)
Loading --(error)--> Failed(message)
Done/Failed --(new query)--> Loading (queryResult = null, selected = null)
Done --(select)--> Done(selected = item)
Done --(confirm, private)--> writes item.private = private, register(item, mangaId) detached, pop
```

- Search call: `searchManga(query)` inside `withIOContext`. A `Throwable` becomes `Failed`, including cancellation (E-TRK-8, upstream behavior kept in text only; the port treats cancel as no-op).
- Preselection: the first result whose `tracking_url == currentUrl` (MTID:750).
- Private option shown only when `supportsPrivateTracking` (MTID:725).
- Result identity for equality: `(manga_id, tracker_id, remote_id)` (MangaTrackSearch.kt equals) (E-TRK-9).

### 3.7 Remove tracking (MTID:772-874)

- Title: "Remove {name} tracking?" (`track_delete_title`).
- Text: "This will remove the tracking locally." (`track_delete_text`).
- Checkbox "Also remove from {name}" (`track_delete_remote_text`), shown only for `DeletableMangaTracker`, unchecked.
- OK: `deleteMangaTrack(mangaId, serviceId)` (local, detached), then if checked `delete(track)` on the deletable tracker (detached; failure logged only, E-TRK-4), then pop.

## 4. Enhanced trackers (new search without UI)

`registerEnhancedTracking(item)` (MTID:227-238):
1. Cast the tracker to `EnhancedMangaTracker` (throws if not, so the caller only routes enhanced trackers here).
2. Detached: load the manga (`GetManga.await(mangaId)`; if null return).
3. `match(manga)`; if it returns null or throws, show `error_no_match` ("No match found"), upstream for any error (E-TRK-2).
4. Otherwise `register(matchResult, mangaId)`.

Port: same flow; the real error text is logged. The banner shows the real error only when decision D-6 is accepted.

## 5. Bulk and automatic tracker interactions (for context)

- Adding a manga to the library may bind enhanced trackers automatically (`bindEnhancedTrackers`, MSM:374), before the user sees the track sheet.
- Marking chapters read may push the last chapter read to every logged-in track that is behind (`TrackChapter.await`, see behavior 6.9.9b and task 040).
- Both are in area:27 for the tracker side. The details screen only triggers them.

## 6. Acceptance checks for this topic

These map to tests in p2-edge-cases-and-tests.md section 3.8 and 3.9:
- `chapter_picker_max_default_10000`, `chapter_picker_max_from_total`, `chapter_picker_initial_truncates`.
- `start_date_not_after_finish`, `finish_date_not_before_start`, `no_future_dates`, `year_bounds_match_day_bounds`, `local_midnight_roundtrip`, `can_remove_date_only_when_set`, `initial_date_falls_back_to_now`.
- `no_track_hides_private_toggle`, `no_remote_url_hides_browser`, `enhanced_source_filter`, `map_track_items_one_row_per_logged_in_manga_tracker`.
