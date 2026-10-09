# Area 39 part 1: edge cases, defects, race conditions and tests

IDs: `E-n` edge case or behaviour to keep, `D-n` defect in the Kotlin (observed) to fix in the port, `R-n` race or lifecycle risk, `F-n` fixture, `T-…` test to write. Each item says what the Rust port must do.

## 1. Defects observed in the Kotlin source (fix in the port)

### D-1 Episode default keys share the manga chapter keys

- Observed: `LibraryPreferences.displayEpisodeByNameOrNumber()` reads `default_chapter_display_by_name_or_number`; `sortEpisodeByAscendingOrDescending()` reads `default_chapter_sort_by_ascending_or_descending` (`domain/src/main/java/tachiyomi/domain/library/service/LibraryPreferences.kt:205-218`). `setEpisodeSettingsDefault` writes those keys (same file, the function after `showEpisodeSummaries()`).
- Effect: "Set as default" for episodes overwrites the manga chapter display and direction defaults, and the reverse.
- Fix: Rust keys `default_episode_display_by_name_or_number` and `default_episode_sort_by_ascending_or_descending`. Migration on first run: copy the chapter values into the episode keys only if the episode keys are absent and the user has saved an episode default (detect via `default_episode_filter_by_seen` presence). Test T-M1.

### D-2 Save reports success when nothing was saved

- Observed: `AnimeImageScreenModel.saveImageInternal` returns `null` when `result?.getBitmapOrNull()` is null (`return@withIOContext null`, `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeImageScreenModel.kt:126`). `saveImage` then calls `showSnackbar(saved)` with no check (lines 70-76).
- Effect: "Cover saved" is shown when there is no image.
- Fix: `save` returns `Result<PathBuf, ImageError::NoImage>`; the snackbar shows the error message. Test T-I2.

### D-3 Background save uses the cover URL

- Observed: `saveImageInternal` builds `ImageRequest.Builder(context).data(anime).size(Size.ORIGINAL).build()` (lines 117-120) with no `useBackground(true)`. `useBackground` sets the coil extra `useBackgroundKey` (`app/src/main/java/eu/kanade/tachiyomi/data/coil/Utils.kt:46-52`), which selects the background URL in the coil fetcher (`Options.useBackground`). Without it the request loads the cover.
- Effect: on the background page, "Save" saves the cover image and names it `background.<ext>`.
- Fix: pass `ImageKind::Background` to the loader. Test T-I3 (fixture: an anime with distinct `thumbnailUrl` and `backgroundUrl`; assert the saved bytes equal the background fixture).
- Caveat: the Kotlin `shareImage` has the same bug (same request), so both are fixed together.

### D-4 Saved directory is not sanitised

- Observed: `ImageSaver.Location.Pictures.directory(context)` builds `File(file, relativePath)` directly (`app/src/main/java/eu/kanade/tachiyomi/data/saver/ImageSaver.kt:199-210`); the anime title is the relative path (`AnimeImageScreenModel.kt:131`). Only the file name goes through `DiskUtil.buildValidFilename`.
- Effect: a title containing `/` (for example "Fate/Zero") creates nested directories; a title of `..` or an empty string can escape or produce an odd path.
- Fix: sanitise the title for the directory with the same rules as `buildValidFilename` (replace `/ \ : * ? " < > |` and control characters with `_`, trim `.` and space, fall back to `(invalid)`). Test T-I4.

### D-5 Quality dialog `!!` and unchecked casts

- Observed: `loadVideo` uses `_hosterState.value!!` and `as HosterState.Ready` (`EpisodeOptionsDialogScreen.kt:263, 362`); `onClickHoster` uses `_source.value!!` (line 350); `init` uses `!!` on `GetEpisode.await` and `GetAnime.await` (lines 172-173).
- Effect: a race (dialog closed before init finishes) can crash.
- Fix: all these become `Option`/`Result` checks, returning a state (`Error` or no-op). Test T-Q7.

### D-6 Season overlay defaults differ between dialog and model

- Observed: `SeasonSettingsDialog.kt:134-142` uses `anime?.seasonDownloadedOverlay ?: false`, `anime?.seasonUnseenOverlay ?: true`, `anime?.seasonLocalOverlay ?: true`, `anime?.seasonLangOverlay ?: false`, `anime?.seasonContinueOverlay ?: true`. For an existing anime the getters return the stored bit (false when unset), so an anime with `season_flags` lacking those bits shows every overlay off, while the dialog's fallback values are on. See E-7.
- Fix: the Rust port stores overlay defaults at creation (from the `default_season_*_overlay` preferences) and never uses dialog fallbacks for existing anime.

### D-7 Large layout airing row has no full span

- Observed: `AnimeScreenLargeImpl` `AIRING_TIME` item (lines 910-916) has no `span` argument; the small layout has `span = { GridItemSpan(maxLineSpan) }` (lines 578-583).
- Fix: full width in both. Test T-L5 (layout snapshot).

### D-8 Countdown does not re-sync with the model

- Observed: `remember { mutableLongStateOf(state.airingTime) }` (`AnimeScreen.kt:584`, `917`) is not keyed on `state.airingTime`. A refreshed airing time does not reset the timer. The comment in the large layout ("reseting") does not match the code.
- Fix: derive remaining time from `Airing::at_ms` on each tick. Test T-A2.

### D-9 FAB visibility and label read different lists

- Observed: visibility uses `episodes` (processed, filtered), label uses `state.episodes` (unfiltered), `AnimeScreen.kt:433-444`.
- Effect: with a filter that hides every unseen episode, the FAB is hidden although "Resume" would be correct.
- Fix: decide visibility and label from the unfiltered list. Test T-F3.

### D-10 `missingEntriesCount` before `missing` is computed on the filtered list

- Observed: `missingEpisodesCount` is computed from `episodes` (processed, filtered) at `AnimeScreen.kt:544-546` and `878-880`.
- Effect: applying a filter changes the "missing" count in the header.
- Decision needed: keep (matches Kotlin) or compute from the unfiltered list. Proposal: unfiltered, because "missing" describes the source's numbering, not the user's view. Record in the Rust settings as a documented deviation. Test T-M2 covers both.

### D-11 `formatTime` and `formatEpisodeNumber` use the default locale

- Observed: `String.format("%d:%02d")` and `DecimalFormat` follow the default locale for digits (see section 6).
- Fix: ASCII. Test T-N2.

### D-12 `packRows` can return more rows than `ceil(total / 3)`

- Observed: sizes `[2, 2, 1]` produce three rows (trace in p1-behavior.md section 18.2). The row count is not a function of total alone.
- Fix (decision): port exactly (parity), add T-R1 asserting the three-row result, and mention the quirk in the release notes. A balanced packer is a later improvement (P3).

### D-13 Uncaught exception from `openInputStream`

- Observed: `AnimeImageScreenModel.editImage` calls `context.contentResolver.openInputStream(data)?.use { try {...} catch (e: Exception) {...} }` (lines 146-157). `openInputStream` itself is outside the `try`; a `SecurityException` or `FileNotFoundException` propagates out of the coroutine. A `null` stream does nothing silently (no message).
- Fix: `open` inside the `Result`; both failure paths show the "update failed" message for the kind. Test T-I5.

### D-14 Failure log happens after the snackbar is dismissed

- Observed: `notifyFailedImageUpdate` calls `snackbarHostState.showSnackbar(...)` (suspends until dismiss or timeout) and only then `logcat(LogPriority.ERROR, e)` (lines 199-205).
- Fix: log first, then show. Test T-I6 (ordering with a fake sink).

### D-15 Share has no success path and no error for an empty image

- Observed: `shareImage` returns silently when `saveImageInternal` returns `null` (line 95, `?: return@launch`). The user sees nothing.
- Fix: show the share error message ("Error sharing cover" / "Error sharing background") in that case. Test T-I7.

### D-16 Quality dialog side effects during composition
- Observed: `onError()` (toast, log, dismiss) runs inside the composable body (`EpisodeOptionsDialogScreen.kt:441-443` and `464-466`).
- Fix: the model raises a `QualityEvent::Failed` and the window reacts in an event handler, never while building widgets. Test T-Q10.

## 2. Edge cases to keep (behaviour parity)

### E-1 Empty selection gives no bottom actions
- `bottom_actions(&[])` returns empty; the bar is hidden. Test T-S1.

### E-2 "Mark previous as viewed" only for exactly one selection
- Test T-S2 with 0, 1, 2 selections.

### E-3 "Mark as unviewed" is also offered for unseen-but-started episodes
- Predicate `seen || last_second_seen > 0`. Test T-S3 (episode with `seen=false`, `last_second_seen=5000` gets the unviewed action).

### E-4 Delete offers all selected, not only downloaded
- `Delete` is offered when any selected is downloaded, and it deletes every selected id. Test T-S4. Proposal: keep the Kotlin behaviour, but the confirmation dialog (part 2) must name the count.

### E-5 Swipe is not gated by selection
- Behaviour parity. Test T-W1 (swipe fires while selection is non-empty).

### E-6 Summary expansion state is per composition
- `expandSummary` is `remember`, not saveable. When a row is recycled (LazyGrid reuse) the expansion can reset. Rust: keep the expansion in the row's `EpisodeRow` model keyed by episode id, so it survives recycling. Test T-R3.

### E-7 Existing anime with no overlay bits shows overlays off
- Rust: the dialog reads the stored bit. The initial value comes from the preference (section 4 of p1-data-and-config.md) at anime creation. Test T-F5 (flags 0 → all overlays false; creation path sets bits per prefs).

### E-8 Interval label with past next update
- `next_update` in the past gives 0 days and the label "Soon". Test T-A1.

### E-9 Episode number formatting near half-way values
- `format_episode_number(2.0005)` and `format_episode_number(2.0015)`. Expected: computed from the binary value with `format!("{:.3}")`, so the test asserts the Rust output, and documents that the Java output can differ on exact decimal ties. Test T-N1 (values 1.0, 12.5, 3.14159, 2.0005, 2.0015, 100.0, 0.1+0.2).

### E-10 Locale digits
- Covered by D-11 (ASCII only). Test T-N2.

### E-11 `formatTime` boundary at one hour
- `format_time_ms(3_600_000) == "60:00"`, `format_time_ms(3_600_001) == "1:00:00"`. Test T-N3.

### E-12 Empty relation groups
- `pack_rows(&[], 3)` returns `[]`. `pack_rows(&[0], 3)` returns `[]` (guard). Test T-R2. The Kotlin would throw `ArithmeticException` on `total / rowCount` with `rowCount == 0`; the UI never sends empty groups, so the guard only protects the Rust build.

### E-13 Relation groups of size exactly 3
- Size 3 is "small" (`<= 3`); size 4 is "multi". Test T-R4.

### E-14 Missing count across a filter
- See D-10. Test T-M2.

### E-15 Missing count ignores `-1.0`
- `missing_entries_count(&[-1.0, 1.0, 3.0])` = 1. Test T-M3 (matches upstream `returns number of missing items` with -1 entries).

### E-16 Missing count with decimals
- `[1.0, 1.1, 1.5, 2.0, 2.2, 4.0]` gives 1: truncated ints `[1,1,1,2,2,4]`, distinct `[1,2,4]`, missing `(4-2-1) = 1`. Test T-M4.

### E-17 Missing count with a leading gap
- `[3.0]` gives 2 (previous starts at 0). Test T-M5.

### E-18 Season grid column rule
- `grid_size == 0` gives `max(1, width / 128)` columns; `grid_size = 4` gives 4. Test T-G1 (width 500 → 3 columns; width 100 → 1 column).

### E-19 Season list mode span
- In list mode each season spans the full row. Test T-G2.

### E-20 Season grid key
- Keys use `anime.id`, not the object. Test T-G3 (reorder keeps identity).

### E-21 Continue overlay only when the callback exists and the item flag is set
- Test T-G4 (four combinations).

### E-22 Downloaded filter is forced in Downloaded-only mode
- `downloaded_filter()` returns `EnabledIs` when `pref_downloaded_only` is true. The filter row is insensitive. Test T-F4.

### E-23 Sort direction shown only on the active sort mode
- Test T-D1 (direction indicator on the active row only).

### E-24 Preview and summary flags are "zero means shown"
- Test T-F6: flags `0` → previews and summaries shown; flags `0x800 | 0x1000` → both hidden.

### E-25 Display mode is "zero means name"
- Test T-F7: `0` → name; `0x100000` → "Episode n".

### E-26 Airing row visibility
- Hidden when `airing_time <= 0`, when the preference is off, or when the status is COMPLETED (`2`). Test T-A3 (four cases).

### E-27 Status values outside 0..7
- Unknown values show the "Unknown" label and the Block icon. Test T-St1 (values -1, 8, 99).

### E-28 Stub source shows the warning icon
- Test T-St2.

### E-29 Source name rules
- Table test for `display_source_name` (one active language in and out; two languages; `all` and `other` ignored). Test T-SN1.

### E-30 Artist shown only when different from author
- `author = Some("A")`, `artist = Some("A")` hides artist. `author = None`, `artist = Some("B")` shows artist. Test T-I8.

### E-31 Title blank shows "Unknown title" and disables search
- Test T-I9.

### E-32 Description regex
- `"a\r\n\r\nb"` becomes `"a\nb"`; `"a\nb"` stays; trailing whitespace trimmed. Test T-D2.

### E-33 Description placeholder
- Blank or null description shows "No description". Test T-D3.

### E-34 Quality dialog Copy does not close the dialog
- Test T-Q8 (UI test or state assertion).

### E-35 Quality dialog closes after the other four actions
- Test T-Q9.

### E-36 Quality dialog toast on failure
- `No available videos` shown once when the result is failure or when the hoster list is empty. Test T-Q10.

### E-37 Duplicate sheet order
- Each action dismisses before acting. Test T-DU1 (the dismiss callback fires before the action callback).

### E-38 Images dialog switch label
- On cover page: switch label "Edit background", edit label "Edit cover". On background page: reversed. Test T-I10.

### E-39 Images dialog edit menu
- Edit with no custom image on the current page runs EDIT directly; with a custom image shows a menu with Edit and Delete. Test T-I11.

## 3. Race conditions and lifecycle

### R-1 Hoster loads finishing after the dialog closed
- Kotlin: `screenModelScope` cancels `async` children; the `CancellationException` branch sets every hoster to `Idle` (lines 252-258). Rust: cancel a `CancellationToken` on close. On cancel, send `HosterState::Idle` for every hoster that has not finished, and do not send anything after the window is gone. Test T-Q11 (cancel mid-load, assert no further UI events).

### R-2 Preferred video race
- Kotlin uses `AtomicBoolean` `hasFoundPreferredVideo`. Only the first hoster with a preferred video wins. If that load fails, the flag is reset to false, so another hoster can win. Rust: `AtomicBool` with `compare_exchange`, same reset rule. Test T-Q12 (two hosters with preferred video; first fails; second becomes selected).

### R-3 Summary expansion across recycling
- See E-6. Test T-R3.

### R-4 Countdown after window destroy
- The `glib::timeout_add_local` source must be removed on destroy (pitfall 6 in p1-rust-design.md). Test T-A4 (destroy then assert no callbacks; in a GTK test use a counter).

### R-5 Selection and refresh
- A refresh that removes a selected episode must drop that id from `SelectionState`. Test T-S5.

### R-6 Dialog opened twice
- `EpisodeOptionsDialogScreen.onDismissDialog` is a global `var` (companion object). Two quality dialogs would overwrite the callback. Rust: a closure per window. Test T-Q13 (two windows, close one, the other stays usable).

### R-7 Image loads racing with edit
- The images dialog shows the old image while an edit writes a new file. Rust: after `edit` succeeds, bump the image cache key (`cover_last_modified` / `background_last_modified`) and reload the picture. Test T-I12.

### R-8 Image save while the page changes
- Kotlin reads `isCover` at call time. Rust: capture the kind when the button is pressed. Test T-I13.

### R-9 Sharing temp files
- Share writes to a temp file; delete it after the share returns (or on app exit). Test T-I14 (temp file gone after dismiss).

### R-10 Stale snapshot in loadVideo
- Observed: `loadVideo` captures `selectedHosterState` before writing `LOAD_VIDEO` and then writes `getChangedAt` on that stale copy in the failure branch (`EpisodeOptionsDialogScreen.kt:268-306`). A concurrent update to another video of the same hoster can be reverted.
- Fix: re-read the hoster state before each write. Test T-Q16 (two concurrent updates to videos of one hoster both survive).

### R-11 Lost `lazy` flag in getHosterList
- Observed: `getHosterList` copies `hosterName`, `hosterUrl`, `videoList`, `internalData`, `memo` but not `lazy` (`EpisodeOptionsDialogScreen.kt:379-385`).
- Fix: copy every field of the hoster. Test T-Q14.

## 4. Fixtures

- F-1 `anime_seasons_sample`: an anime with `fetch_type = Seasons`, 5 season entries with `season_number` 1, 2, 2.5, 3, 4. Used by T-G*.
- F-2 `anime_episodes_gap`: episode numbers `[1, 2, 4, 4.5, 7, -1]` plus one with `episode_number = 16.5`. Used by T-M*.
- F-3 `anime_episodes_50`: 50 episodes with varied flags (seen every 3rd, bookmark every 5th, fillermark every 7th, downloaded every 4th, `last_second_seen` on every 6th unseen, summaries on every 2nd, previews on every 9th). Used by selection, bottom-menu and rendering tests.
- F-4 `episodes_3000`: 3000 episodes for the list-performance check (T-P1).
- F-5 `relations_mixed`: groups of sizes `[1, 2, 3, 4, 7, 2, 2, 1]`. Used by T-R*.
- F-6 `hosters_three`: hoster A with two videos (one not ready), hoster B with one preferred video, hoster C that throws. Used by T-Q*.
- F-7 `image_pair`: two distinct PNG files (cover, background), 1x1 and 2x2 pixels, plus a non-image file. Used by T-I*.
- F-8 `airing_fixture`: `Airing { episode_number: 7.0, at_ms: now + 90_061_000 }` (1 d 1 h 1 m 1 s). Used by T-A*.
- F-9 `flags_golden`: a table of `(name, value, expected)` for every mask in p1-data-and-config.md section 2. Used by T-F*.

## 5. Tests to write (Rust)

Unit tests (no display needed) live next to the module (`#[cfg(test)] mod tests`). Names are the IDs used above.

### 5.1 Flags (`flags.rs`)
- T-F1: `ViewerFlags::AIRING_TIME_MASK == 0x0FFFFFFFF000000`; `AIRING_TIME_MASK & INTRO_DISABLE_MASK == 0`; `AIRING_EPISODE_MASK & AIRING_TIME_MASK == 0`; `INTRO_MASK & AIRING_EPISODE_MASK == 0`.
- T-F2: every `EpisodeFlags` getter over F-9 values.
- T-F3: FAB rule (D-9) uses the unfiltered list (this test lives in part 2 model but asserts the helper in this module: `fab_state(&processed, &unfiltered)`).
- T-F4: Downloaded-only forces `EnabledIs`.
- T-F5: season overlays with flags `0` are all false; with the preference-derived bits set they are true.
- T-F6: previews and summaries zero means shown.
- T-F7: display mode zero means name.
- T-F8: `SeasonDisplayMode::from_i64(4)` is `Err`; `0..=3` map to the four variants.
- T-F9: `with(mask, value)` changes only masked bits (property test with random `i64` and every mask).

### 5.2 Format (`format.rs`)
- T-N1: `format_episode_number` for `1.0 -> "1"`, `12.5 -> "12.5"`, `3.14159 -> "3.142"`, `100.0 -> "100"`, `0.1 + 0.2 -> "0.3"`, `2.0005 -> "2"` (asserts the Rust rule; document the binary value).
- T-N2: ASCII output; no Unicode digits for any input.
- T-N3: `format_time_ms` boundaries: `0 -> "0:00"`, `59_999 -> "0:59"`, `60_000 -> "1:00"`, `3_599_999 -> "59:59"`, `3_600_000 -> "60:00"`, `3_600_001 -> "1:00:00"`, `86_399_999 -> "23:59:59"`.
- T-N4: `format_airing(90_061_000) == "Airing in 01d 01h 01m 01s"`.
- T-N5: `format_airing(0) == "Airing in 00d 00h 00m 00s"` (the UI does not call it for 0, but it must not panic).

### 5.3 Missing (`missing.rs`)
- T-M1: migration of the default keys (D-1) is in `settings` tests (cross-reference), here assert the key names.
- T-M2: filter-aware count (D-10) with F-2 and a filter that drops 4.5 and 7.0.
- T-M3: `[-1.0, 1.0, 3.0] -> 1`.
- T-M4: `[1.0, 1.1, 1.5, 2.0, 2.2, 4.0] -> 1`.
- T-M5: `[3.0] -> 2`.
- T-M6: upstream cases from `domain/src/test/java/tachiyomi/domain/items/service/MissingItemsTest.kt`: `[]` gives 0; `[-1,-1,-1]` gives 0; `[1,1,1.1,1.5,1.6,1.99]` gives 0; `[-1,1,2,2.2,4,6,10,10]` gives 5. Port these four as-is.

### 5.4 Selection (`selection.rs`)
- T-S1: empty selection gives no actions.
- T-S2: "mark previous" only for one.
- T-S3: unviewed for `seen=false, last_second_seen>0`.
- T-S4: delete offered when any downloaded; deletes all selected ids.
- T-S5: refresh drops removed ids from `SelectionState`.
- T-S6: `invert` over `[1,2,3]` with `{2}` selected gives `{1,3}`.
- T-S7: `select_all(false)` clears; `select_all(true)` selects every id.
- T-S8: the full predicate table from p1-behavior.md section 1.4 on F-3 (every row, both player modes).

### 5.5 Download (`download_action.rs`)
- T-DL1: the full `(state, click)` matrix from p1-rust-design.md section 3.3 and the Kotlin lines cited there.
- T-DL2: `Downloaded` primary click returns `None` (opens menu), `MenuDelete` returns `Delete`.

### 5.6 Related packing (`related_pack.rs`)
- T-R1: `[2, 2, 1] -> 3 rows` (D-12).
- T-R2: `[]` and `[0]` give `[]`.
- T-R3: lives in part 2 model; here the helper `expansion_key(episode_id)` is stable across rebuilds (E-6).
- T-R4: sizes `[3]` small; `[4]` multi (the partition is in the view module; test the partition helper).
- T-R5: sizes `[1,1,1,1]` give `[[0,1],[2,3]]`. Trace: total 4, rowCount 2, base 2, remainder 0, cap 2. Groups 0 and 1 fill row 1 (filled 2). Group 2 overflows (2+1 > 2), so row 1 closes and cap stays 2. Groups 2 and 3 fill row 2.

### 5.7 Status and source name
- T-St1: unknown status values give the unknown label and `action-unavailable-symbolic`.
- T-St2: stub warning shown only for stub sources (view test, optional).
- T-SN1: `display_source_name` table: `("Src", "en", ["en"]) -> "Src"`; `("Src", "ja", ["en"]) -> "Src (JA)"`; `("Src", "en", ["en", "ja"]) -> "Src (EN)"`; `("Src", "en", ["all", "other"]) -> "Src (EN)"`.

### 5.8 Image model (`images/model.rs`)
- T-I1: save writes `cover.<ext>` for cover and `background.<ext>` for background; ext from magic bytes (PNG, JPEG, WebP).
- T-I2: D-2 (no image gives error, not success).
- T-I3: D-3 (background fetch uses background URL).
- T-I4: D-4 (title sanitising, table: `"Fate/Zero" -> "Fate_Zero"`, `"..." -> "(invalid)"`, `""` -> `"(invalid)"`, `"a:b*c"` -> `"a_b_c"`).
- T-I5: D-13 (openInputStream failure shows the update failed message).
- T-I6: D-14 (log before snackbar).
- T-I7: D-15 (share empty image shows error).
- T-I8: artist shown rule (E-30).
- T-I9: blank title (E-31).
- T-I10: switch label rule (E-38).
- T-I11: edit menu rule (E-39).
- T-I12: image cache bump after edit (R-7).
- T-I13: kind captured at press time (R-8).
- T-I14: temp share file removed (R-9).

### 5.9 Quality model (`quality/model.rs`)
- T-Q1: `select_best_video` pass 1 (preferred READY in a later hoster beats a non-preferred in an earlier one).
- T-Q2: pass 2 (no preferred; first non-empty URL in READY or QUEUE).
- T-Q3: no candidates gives `None`.
- T-Q4: `QUEUE` counts as usable (`READY || QUEUE`).
- T-Q5: `ERROR` and `LOAD_VIDEO` are not usable.
- T-Q6: `load_video` failure of the current video with `current == None` picks the next best; with `current == Some` restores the previous selection.
- T-Q7: D-5 (no panic when the hoster list is missing; returns an error state).
- T-Q8: E-34 (copy keeps the window open).
- T-Q9: E-35 (other four actions close).
- T-Q10: E-36 (toast once).
- T-Q11: R-1 (cancellation: Idle for unfinished hosters; no later events).
- T-Q12: R-2 (preferred race).
- T-Q13: R-6 (two windows).
- T-Q14: `getHosterList` returns `None` before the hosters load, and substitutes Ready videos into the hoster list (mirror of `getHosterList`, EpisodeOptionsDialogScreen.kt:375-390).
- T-Q15: `alt` parameter table for both downloader preference values (quality topic section 6): internal row passes `alt = use_external_downloader`, external row passes `alt = !use_external_downloader`.
- T-Q16: R-10 regression (two concurrent video updates in one hoster both survive).

### 5.10 Layout and view (GTK, optional display test)
- T-L1: wide layout at 1000 px gives two panes; 800 px gives one column.
- T-L2: season grid columns: adaptive and fixed (T-G1 as widget test).
- T-L3: selection bar revealed exactly when count > 0.
- T-L4: FAB visible rule and label (T-F3 as widget test).
- T-L5: airing row spans full width in both layouts (D-7).
- T-L6: summary toggle does not fire while selecting (E-5 neighbour; click on summary when selected does nothing).
- T-W1: swipe fires while selected (E-5).
- T-D1: sort direction indicator only on active row.
- T-D2: description regex (E-32).
- T-D3: description placeholder (E-33).
- T-G1..T-G4: season grid rules (E-18..E-21).
- T-P1: 3000-episode list: scroll to the end in under 2 s on the CI display; memory stays under 200 MB (measure with `/proc/self/status` VmRSS in the test). Pass criteria are part of the acceptance of T-39a-010.

### 5.11 Airing (`view/airing_row.rs`)
- T-A1: E-8 (past next update gives "Soon", future gives "N days").
- T-A2: D-8 (after a model update the remaining time is re-derived).
- T-A3: E-26 (four visibility cases).
- T-A4: R-4 (source removed on destroy).

### 5.12 Preferences (`settings` module, cross-reference to area 02)
- T-PR1: keys and defaults in p1-data-and-config.md section 4 are read by the names listed (golden test over the key strings).
- T-PR2: swipe key mapping: `start action` reads `pref_episode_swipe_end_action` (default `ToggleSeen`), `end action` reads `pref_episode_swipe_start_action` (default `ToggleBookmark`). Asserting the key names, not the UI meaning (see p1-behavior.md section 11 verification note).

## 6. Manual test checklist (desktop)

1. Open an anime with 50 episodes: header count "50 episodes", missing count matches F-2 logic.
2. Filter Unseen: list shrinks; FAB still shows "Resume" when any episode is seen (D-9 fixed).
3. Long press a row: selection bar appears with the predicates of section 1.4.
4. Swipe a row right and left: each side performs the configured action.
5. Open the quality dialog on a not-downloaded episode; choose "Use internal downloader": dialog closes and the download is queued.
6. Open cover, switch to background page, save: file is named `background.<ext>` and shows the background image (D-3).
7. Save with a title containing `/`: directory is sanitised (D-4).
8. Resize from 1200 px to 700 px: layout switches to one column; no horizontal scroll.
9. Airing anime: countdown decreases each second; reopening after a refresh shows the new time (D-8).
10. Related section: a group of size 2 and a group of size 2 and a group of size 1 render as three rows (D-12).

## 7. Open decisions (need the user's answer)

- OD-1: keep the Kotlin "missing count over filtered list" (D-10) or change it.
- OD-2: keep the packing quirk (D-12) or balance rows.
- OD-3: confirm before deleting an image on Linux (9.8 in p1-ui-parity.md).
- OD-4: episode thumbnails and summaries on desktop: default on or off (Kotlin defaults are on, `EPISODE_SHOW_PREVIEWS = 0`).
