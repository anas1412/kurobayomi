# 52-util-app — edge cases, failure modes, tests

Upstream tests: none. `find` over the scope returns no `*Test.kt` file (checked with `find . -name "*Test*.kt" -path "*util*"`, empty). Every test below is NEW and must be written for the Rust port. Test names are `snake_case` and carry the id `TC-nn-x` so they can be linked from tasks.

Fixture directory: `/home/blackbox/kurobayomi/kuro-core/tests/fixtures/` (to create). Fixtures listed in section 7.

---

## 1. Edge cases by module

### 1.1 Preferences binding (`PreferenceMutableState.kt`)
- EC-01: Preference changed while the widget is being destroyed. Expected: the watch receiver is dropped on `destroy`; no update on a dead widget (no panic). Test TC-01-a.
- EC-02: `set` with an equal value. Upstream emits a change; Rust suppresses (`send_if_modified`). Expected: the widget still shows the value. Test TC-01-b.
- EC-03: Initial `get()` fails (corrupt store). Upstream: `preference.get()` throws, widget creation fails. Rust: `get()` returns the default and logs. Test TC-01-c.

### 1.2 Tri-state (`CheckboxState.kt`)
- EC-04: Exhaustive match; no default branch. Test TC-02-a: all three variants map to the three `(inconsistent, active)` pairs.

### 1.3 Collections (`CollectionUtils.kt`)
- EC-05: `insertSeparators` on empty input returns empty output and does NOT call the generator. Test TC-03-a (count the generator calls).
- EC-06: Single element: exactly three rows. Test TC-03-b.
- EC-07: Generator returning `None` everywhere returns the items only. Test TC-03-c.
- EC-08: `insertSeparatorsReversed` generator order is reverse index. Test TC-03-d (record call sequence).
- EC-09: `addOrRemove` on absent value with `shouldAdd = false` is a no-op. Test TC-04-a.

### 1.4 Source gate
- EC-10: The source manager never initialises (for example, the source directory is unreadable). Upstream spins forever. Rust: 30 s timeout to the empty state (ui-parity 10). Test TC-05-a (use a fake channel that never sends; expect empty-state signal after 30 s, using a paused tokio clock).

### 1.5 Chapter and episode selection
- EC-11: `next_unread` with empty list -> None. Test TC-07-a.
- EC-12: All read -> None. Test TC-07-b.
- EC-13: Descending sort: search from the end (lowest-numbered unread). Test TC-07-c (fixture S1).
- EC-14: Quirk, unread filter `EnabledNot` (show read only) with all read: result None although read chapters exist. Test TC-07-d (documents the observed behaviour).
- EC-15: Downloaded-only override (`downloadedFilter` = `EnabledIs`) with no downloaded unread -> None. Test TC-07-e.
- EC-16: Local source (`is_local = true`): downloaded filter is a no-op and every chapter counts as downloaded. Test TC-10-a.
- EC-17: `dedupe` with current chapter absent from the list: fallback to scanlator, then first. Test TC-09-b.
- EC-18: `dedupe` with `scanlator = None` on all: `None == None` matches -> the first chapter for the group that equals current scanlator None. Test TC-09-c.
- EC-19: `dedupe` chapter numbers `1.0` and `1.00` are one group. `-0.0` and `0.0` are one group. `NaN` numbers: group by bit pattern after normalising `-0.0`; `NaN` with the same bits groups together (upstream `groupBy` uses `equals`, which treats NaN as equal to NaN). Test TC-09-d.
- EC-20: Chapter numbers sorting with NaN: upstream `Double.compareTo` places NaN after all numbers; Rust `f64::total_cmp` places positive NaN after +inf: same. Test TC-08-d.
- EC-21: Sorting with an invalid `sorting` bit pattern (for example `0x400`) throws in upstream. Rust returns `SortError` and the caller shows the default sort. Test TC-08-e.
- EC-22: Source-order descending is inverted (observed). Test TC-08-b (fixture S2).

### 1.6 Covers
- EC-23: Remote returns `thumbnail_url = None`: no change. Test TC-11-a.
- EC-24: Remote returns `""`: no change, the existing cover is kept. Test TC-11-b.
- EC-25: Same URL, `refresh_same_url = false`: no change. Test TC-11-c. With `true`: decision per section 2.5 of design (remote cover path). Test TC-11-d.
- EC-26: Custom cover exists: thumbnail file deleted, timestamp NOT bumped (custom keeps showing). Test TC-11-e.
- EC-27: `removeCovers` when no file exists: returns 0, timestamp unchanged. Test TC-12-a.
- EC-28: Local source `removeCovers`: returns self (no file operations). Test TC-12-b.
- EC-29: `editCover` on a non-favourite remote entry: silent no-op; the stream is not consumed. Test TC-12-c.
- EC-30: `editThumbnail` on a remote anime: silent no-op. Test TC-12-d.

### 1.7 Airing
- EC-31: Completed anime without manual fetch: no network, stored pair returned. Test TC-13-a.
- EC-32: Completed with manual fetch: network is used. Test TC-13-b.
- EC-33: No linked tracker: `(1, 0)` and the caller overwrites stored values. Test TC-13-c (documents the overwrite; the Rust caller must still persist, to match the observed data behaviour; the tasks flag this as a decision).
- EC-34: MAL id unknown (`0`) must not call AniList with `Media(id:0)`. Rust short-circuits to `(1, 0)`. Test TC-14-c (mock client counts requests: must be 0 for the second request).
- EC-35: AniList returns HTTP 429 with a JSON error body: parse yields `(1, 0)`. Test TC-14-d.
- EC-36: AniList `nextAiringEpisode: null`: `(1, 0)`. Test TC-14-b.
- EC-37: Simkl target `mal` is `null` and a later entry has `"mal":"..."`: the Rust port must NOT take the later entry's MAL id. Test TC-15-b (fixture S3).
- EC-38: Simkl target has no future date. Upstream returns `(episodeNumber, 0)`, where `episodeNumber` is the last value parsed from an earlier calendar type (or `1`). Rust returns `(1, 0)`. Deviation: the time is 0 in both cases, so the UI hides the line and the stored result differs only in the unused episode number. Test TC-15-c.
- EC-39: Simkl date with no offset (`2026-10-07T00:00:00`): upstream throws; Rust skips the entry. Test TC-15-d.
- EC-40: Viewer-flag episode > 65535: upstream truncates; Rust truncates identically and logs `warn`. Test TC-16-b.
- EC-41: Airing time beyond 2106: truncated. Test TC-16-c.
- EC-42: Two writes (episode, then time) are not atomic upstream. Rust writes once. Test TC-16-d (one update call).
- EC-43: Concurrent airing loads for the same anime: last writer wins. Rust serialises per anime id through `JobRegistry` tag `airing:<id>`. Test TC-13-d.
- EC-44: Very large Simkl calendar (>16 MiB) is rejected with a clear error and falls back to `(1, 0)`. Test TC-15-e.

### 1.8 Crash report
- EC-45: `logcat -f` overwrites the file (upstream hazard). Rust: writes all sections last into the same file; no external process. Test TC-17-c (assert contents include every section).
- EC-46: Extension with `available_version = None`: printed as `?`. Test TC-19-b.
- EC-47: No problematic extensions: section omitted entirely (no header). Test TC-19-c.
- EC-48: Installed version newer than available (`available.versionCode < installed`): no update flag. Test TC-19-d.
- EC-49: Crash dump while another dump is running: the file is deleted and re-created by the second dump (upstream `createFileInCacheDir` deletes); Rust serialises dumps with a `Mutex`. Test TC-17-d.

### 1.9 PKCE
- EC-50: Verifier length 67 and only `[A-Za-z0-9_-]`. Test TC-20-a.
- EC-51: Two calls are different (CSPRNG). Test TC-20-b.

### 1.10 Size and directories
- EC-52: `format_bytes(999_999)` returns `1000.00 KB` (upstream quirk). Test TC-21-b (keep the quirk).
- EC-53: Negative size prints `-N B`. Test TC-21-c.
- EC-54: `dir_size` on a symlink loop terminates. Test TC-22-b.
- EC-55: `dir_size` on a missing path returns an error (upstream returns 0 for a missing directory because `listFiles()` is null; Rust returns `Ok(0)` for a missing path to match). Test TC-22-c.
- EC-56: `dir_size` while a download is writing: returns a partial but valid total. Test TC-22-d.

### 1.11 Stream save
- EC-57: Parent directory missing: created. Test TC-23-a.
- EC-58: Write error mid-stream: `<dest>.part` removed, `dest` not created. Test TC-23-b.
- EC-59: Destination exists: replaced atomically by rename. Test TC-23-c.

### 1.12 Cache file creation
- EC-60: Existing file is deleted, then created empty. Test TC-24-a.
- EC-61: Cache directory not writable: error returned, caller shows `Failed to get logs`. Test TC-24-b.

### 1.13 Dates
- EC-62: `relative_date` boundaries -8, -7, -1, 0, 1, 6, 7. Test TC-25-a (table in section 7).
- EC-63: `relative_datetime` 30 s ahead -> `Now`; 90 s ahead -> `In 1 minute`; 30 s behind -> `Now`; 59 min 59 s behind -> `59 minutes ago`; 1 h 0 min behind -> `1 hour ago`; 2 days 1 h behind -> `2 days ago`. Test TC-26-a (table).
- EC-64: DST transitions are ignored by naive arithmetic (upstream `LocalDateTime`). Test TC-26-c (release 02:30 on DST day, now 12:00 -> `10 hours ago`).
- EC-65: Plural `one` vs `other` in a language with more categories (for example Arabic: zero/one/two/few/many/other). Rust uses ICU plural categories; the English key table has only `one` and `other`. Expected: categories without a key fall back to `other`. Test TC-27-b.
- EC-66: `toTimestampString` with 12-hour vs 24-hour locale. Test TC-27-c (ICU short time with `en-US` and `de-DE`).
- EC-67: `convertEpochMillisZone(0, UTC, Asia/Tokyo)` = `-32400000`. Test TC-27-d.

### 1.14 Locale
- EC-68: Comparator `all` first; `compare(all, all)` returns `Equal` in Rust (upstream returns -1, invalid comparator). Test TC-28-a.
- EC-69: `zh-CN` and `zh-Hans` produce the same display name. Test TC-28-b.
- EC-70: Empty system language: enabled set has no empty string. Test TC-29-b.
- EC-71: Source display name for `null` returns empty string (no panic). Test TC-28-d.
- EC-72: Runtime locale change: cached names keyed by language code only; display names do not change until restart (documented; upstream also caches via `Locale`). Test TC-28-e.

### 1.15 Geometry
- EC-73: `invert(None)` returns the same rectangle. Test TC-31-a.
- EC-74: `invert(Both)` twice is identity. Test TC-31-b (property test over random rects in 0..1).
- EC-75: A touching-edge rectangle is not visible. Test TC-13-visibility-a (`intersects_positive_area`).

### 1.16 Clipboard, browser, toast
- EC-76: Blank content: no clipboard write, no toast. Test TC-36-a (unit on the decision function `copy_decision(content) -> Action`).
- EC-77: Clipboard unavailable (no display): toast `clipboard_copy_error`. Test TC-36-b (manual + mock trait).
- EC-78: Open URL with an invalid scheme: error text shown raw. Test TC-37-a.
- EC-79: Toast queue: a second toast replaces the first; timer of the first must not hide the second. Test TC-38-a (paused clock).
- EC-80: Toast after window closed: dropped without panic. Test TC-38-b.

### 1.17 Network
- EC-81: `PORTAL` connectivity is offline. Test TC-40-a.
- EC-82: NetworkManager absent: `watch_network` emits `gio` based state with `is_wifi = false`. Test TC-40-b (mock bus absent).
- EC-83: Flapping network: state changes 5 times in 200 ms produce at most 2 emissions after a 500 ms debounce (Proposal). Test TC-40-c.
- EC-84: The seed value is emitted before any change event. Test TC-40-d.

### 1.18 Notifications
- EC-85: Notification post while the desktop refuses (no portal): logged `warn`, no panic. Test TC-41-a.
- EC-86: Withdraw an id that was never posted: no error. Test TC-41-b.
- EC-87: Batch with 0 items: no call. Test TC-41-c.

### 1.19 Jobs
- EC-88: `start` while the same tag runs: returns `false`, the running job continues. Test TC-56-a.
- EC-89: `is_running` after the task panicked: `false` (the handle is finished). Test TC-56-b.
- EC-90: `cancel` a finished job: no-op. Test TC-56-c.

### 1.20 Authentication
- EC-91: PAM service file missing: `is_authentication_supported() == false`; the setting cannot be enabled. Test TC-18-a.
- EC-92: Wrong password: `AuthError::Pam(text)`; no lock state change. Test TC-18-b (needs a PAM stub; use the `pam` mock trait).
- EC-93: Empty password: rejected before PAM call. Test TC-18-c.
- EC-94: Concurrent prompts: `isAuthenticating` guard prevents a second window. Test TC-18-d.

### 1.21 Layout
- EC-95: AUTO at 719 px: compact; 720 px: wide. Test TC-53-a.
- EC-96: LANDSCAPE mode with `width == height`: compact (not landscape). Test TC-53-b.
- EC-97: ALWAYS and NEVER ignore size. Test TC-53-c.

### 1.22 Reader background
- EC-98: Theme value outside 0..3 (for example 9): treated as White (upstream `else -> false`). Test TC-54-a.
- EC-99: Auto with `themeMode = SYSTEM` and system dark unknown (portal `NoPreference`): light. Test TC-54-b.

### 1.23 Animation scale
- EC-100: GTK animations disabled: scale 0; crossfade 0 ms; page animation 1 ms. Test TC-32-a.
- EC-101: KDE factor missing: 1.0. Test TC-32-b.
- EC-102: Factor above 10 clamped to 10 (Proposal). Test TC-32-c.

### 1.24 Build flags
- EC-103: `UPDATER_ENABLED` is false in every build. Test TC-34-a.

---

## 2. Race conditions (summary)

| Race | Where | Mitigation |
|---|---|---|
| Cover invalidation vs cover render | `prep_update_cover` decision then file delete | decision and delete happen on one blocking task; UI reloads after `cover_last_modified` changes |
| Two airing loads for one anime | `load_airing` | `JobRegistry` tag `airing:<id>`; second caller awaits the first |
| Crash dump twice | `create_cache_file` deletes the file | `Mutex` around dump |
| Network watch vs job start | download jobs read state | seed value before subscribing; debounce 500 ms |
| Save stream and reader reading the same file | `save_stream` writes `.part` then renames | readers see either old or new file |
| Preference write echo | `Pref` set then watch echo | UI updates only from the watch echo |

---

## 3. Failure-mode matrix

| Dependency | Failure | Observable result (Rust) | Upstream result |
|---|---|---|---|
| AniList | timeout / DNS | `(1, 0)` | `(1, 0)` |
| AniList | invalid JSON | `(1, 0)` | `(episode or 1, time or 0)` via string parse |
| Simkl | 404 for one calendar | next calendar tried | `return (1, 0)` immediately (discards progress) |
| Simkl | calendar > 16 MiB | `(1, 0)` with `warn` | reads whole body |
| NetworkManager | not running | `gio` monitor only | `activeNetwork` null -> offline |
| Portal Settings | no session bus | `NoPreference` | dark-mode system lookup |
| Clipboard | no display | toast `clipboard_copy_error` | toast (SDK-dependent) |
| PAM | service file absent | unsupported | `isAuthenticationSupported` false |
| Save dialog | cancelled | no-op | not applicable |
| Cache dir | not writable | error -> toast `Failed to get logs` | same |

---

## 4. New tests to write (summary by file)

| Rust file | Tests |
|---|---|
| `kuro-core/src/util/size.rs` | TC-21-a, TC-21-b, TC-21-c |
| `kuro-core/src/util/collections.rs` | TC-03-a .. TC-03-d, TC-04-a |
| `kuro-core/src/selection.rs` | TC-07-a .. TC-07-e, TC-09-a .. TC-09-d, TC-10-a |
| `kuro-core/src/sort.rs` | TC-08-a .. TC-08-e |
| `kuro-core/src/covers.rs` | TC-11-a .. TC-11-e, TC-12-a .. TC-12-d |
| `kuro-core/src/airing.rs` | TC-16-a .. TC-16-d |
| `kuro-net/src/airing.rs` (mock HTTP) | TC-13-a .. TC-13-d, TC-14-a .. TC-14-d, TC-15-a .. TC-15-e |
| `kuro-core/src/time/relative.rs` | TC-25-a, TC-26-a .. TC-26-c |
| `kuro-core/src/time/format.rs` | TC-27-a .. TC-27-d |
| `kuro-core/src/locale.rs` | TC-28-a .. TC-28-e, TC-29-a, TC-29-b |
| `kuro-core/src/geometry.rs` | TC-31-a, TC-31-b, TC-13-visibility-a |
| `kuro-core/src/pkce.rs` | TC-20-a, TC-20-b |
| `kuro-core/src/crashlog.rs` | TC-17-a, TC-17-b, TC-19-a .. TC-19-d |
| `kuro-core/src/reader_theme.rs` | TC-54-a, TC-54-b |
| `kuro-core/src/layout.rs` | TC-53-a .. TC-53-c |
| `kuro-platform/src/files.rs` (tempdir) | TC-22-a .. TC-22-d, TC-23-a .. TC-23-c, TC-24-a, TC-24-b |
| `kuro-platform/src/net.rs` (mock monitor/bus) | TC-40-a .. TC-40-d |
| `kuro-platform/src/jobs.rs` (tokio paused clock) | TC-56-a .. TC-56-c |
| `kuro-platform/src/auth.rs` (mock PAM) | TC-18-a .. TC-18-d |
| `kuro-platform/src/appearance.rs` | TC-32-a .. TC-32-c |
| `kuro-platform/src/notify.rs` (mock app) | TC-41-a .. TC-41-c |
| `kurobayomi/src/ui/toast.rs` (glib main context test) | TC-38-a, TC-38-b |
| `kurobayomi/src/ui/checkbox.rs` | TC-02-a |
| `kurobayomi/src/ui/bind.rs` | TC-01-a .. TC-01-c |
| `kuro-core/src/build_info.rs` | TC-34-a |

---

## 5. Detailed test vectors

### 5.1 `format_bytes` (TC-21-a/b/c)
| Input (i64) | Output |
|---|---|
| 0 | `0 B` |
| 999 | `999 B` |
| 1000 | `1.00 KB` |
| 999999 | `1000.00 KB` (quirk, keep) |
| 1000000 | `1.00 MB` |
| 1500000 | `1.50 MB` |
| 2000000000 | `2.00 GB` |
| -5 | `-5 B` |
| i64::MAX | `9223372036.85 GB` |

### 5.2 `insert_separators` (TC-03-a/b/c/d)
Input `[a, b]`, generator `g(before, after) = Some(Sep(before, after))` where `None` renders as `_`:
Output: `[Sep(_,a), a, Sep(a,b), b, Sep(b,_)]`, generator calls: 3 (`(_,a)`, `(a,b)`, `(b,_)`) in forward mode; reverse mode calls `(b,_)`, `(a,b)`, `(_,a)` in that order, final output identical.
Input `[]`: output `[]`, calls 0.
Input `[a]`: output `[Sep(_,a), a, Sep(a,_)]`.

### 5.3 `next_unread` (TC-07-a..e)
| Name | Units (number:read) | Filter | Sort desc | Downloaded | Expected |
|---|---|---|---|---|---|
| a | (none) | Disabled | false | — | None |
| b | 1:r 2:r 3:r | Disabled | false | — | None |
| c | 1:r 2:u 3:u | Disabled | true | — | 2 (sorted desc = [3u, 2u, 1r]; last unread = 2) |
| d | 1:r 2:r | unread = EnabledNot | false | — | None (quirk) |
| e | 1:u 2:u | downloaded = EnabledIs | false | none downloaded | None |

Fixture S1 = the table row c data.

### 5.4 `dedupe` (TC-09-a..d)
Units: `(id, number, scanlator)`: `(1, 1.0, "A")`, `(2, 1.0, "B")`, `(3, 2.0, "A")`.
- a: current = id 2 -> `[id2 (1.0), id3 (2.0)]`.
- b: current = id 9 (absent), scanlator "A" -> `[id1, id3]`.
- c: current scanlator None, no None present -> group keeps first: `[id1, id3]`; with `(4, 2.0, None)` present and current None -> `[id1, id4]`.
- d: numbers `1.0` and `1.00` (same f64) -> one group; `-0.0` and `0.0` -> one group; NaN (bit pattern equal) -> one group.

### 5.5 Sort (TC-08-a..e)
- a: number desc `[1, 3, 2]` -> `[3, 2, 1]`.
- b: source desc with sourceOrder values `[0, 1, 2]` -> ascending `[0, 1, 2]` (inverted, fixture S2 documents).
- c: alphabet `"Chapter 10"` and `"Chapter 2"` with primary collator `en-US` -> `"Chapter 2"` < `"Chapter 10"`? With the PRIMARY strength on digits, collation compares "1" < "2" first, so `"Chapter 10"` < `"Chapter 2"`. Expected order (asc): `Chapter 10`, `Chapter 2`. Asserted as the test.
- d: NaN number sorts after finite numbers (`total_cmp`).
- e: bits `0x400` -> `SortError::Invalid(0x400)`.

### 5.6 Airing (TC-13..16)
- TC-16-a: `encode_airing(0, 12, 1765000000)` then `decode_airing` = `(12, 1765000000)`.
- TC-16-b: episode 70000 -> decoded `4464` (70000 mod 65536).
- TC-16-c: time `2^32` -> decoded `0`.
- TC-16-d: one `update` call with both fields (mock repository counts writes).
- TC-14-a: AniList ok body -> `(12, 1765000000)`.
- TC-14-b: AniList null body -> `(1, 0)`.
- TC-14-c: MAL null id -> no AniList call.
- TC-14-d: transport error -> `(1, 0)`.
- TC-15-a: Simkl target with `mal` "62248" -> AniList path.
- TC-15-b: Simkl target `mal: null`, later entry `mal: "999"` -> must not use 999.
- TC-15-c: Simkl only past entries -> `(1, 0)`.
- TC-15-d: Simkl entry date `2026-10-07T00:00:00` (no offset) -> skipped.
- TC-15-e: body 16 MiB + 1 -> error, `(1, 0)`.
- TC-13-a: completed, not manual -> no network, stored pair.
- TC-13-b: completed, manual -> network used.
- TC-13-c: no track -> `(1, 0)`.
- TC-13-d: two concurrent loads for one anime -> one network sequence.

### 5.7 Relative date (TC-25-a) — today = Friday 2026-10-09
| Date | Difference (today - date) | Output |
|---|---|---|
| 2026-10-17 | -8 | absolute |
| 2026-10-16 | -7 | `In 7 days` |
| 2026-10-10 | -1 | `Tomorrow` |
| 2026-10-09 | 0 | `Today` |
| 2026-10-08 | 1 | `Yesterday` |
| 2026-10-03 | 6 | `6 days ago` |
| 2026-10-02 | 7 | absolute |

### 5.8 Relative datetime (TC-26-a) — now = 2026-10-09 12:00:00
| Release | Output |
|---|---|
| 12:00:30 | `Now` |
| 12:01:30 | `In 1 minute` |
| 11:59:30 | `Now` |
| 11:00:59 | `59 minutes ago` (elapsed 59 min 1 s, minute difference 59, hour difference 0) |
| 11:00:00 | `1 hour ago` |
| 09:55:00 | `2 hours ago` |
| 2026-10-07 10:00 | `2 days ago` (time difference 2 days 2 h -> td = 2; dd = 2) |
| 2026-10-01 12:00 | absolute |

Rule used for every row: `MINUTES.between(release, now)` and `HOURS.between(release, now)` are the elapsed time truncated toward zero, and `DAYS.between` is the elapsed 24 h periods truncated toward zero.

### 5.9 Locale (TC-28, TC-29)
- `comparator(all, en) < 0`, `comparator(en, all) > 0`.
- `display(zh-CN) == display(zh-Hans)`.
- system `de` -> enabled `{all, de, en}`.
- system `""` -> `{all, en}`.

### 5.10 Geometry (TC-31)
Rect `(left 0.1, top 0.2, right 0.4, bottom 0.6)`:
- None -> same.
- Horizontal -> `(0.6, 0.2, 0.9, 0.6)`.
- Vertical -> `(0.1, 0.4, 0.4, 0.8)`.
- Both -> `(0.6, 0.4, 0.9, 0.8)`.

### 5.11 Crash report (TC-17, TC-19)
Labels in order: `App ID:`, `App version:`, `Android version:` (Linux: `OS:`), ... as recorded in behaviour 8.2. Test asserts the first 4 labels and the trailing `FFmpeg version:`.
Extension block (manga form, TC-19-a): name `Foo`, installed `1.2` / available `1.3` -> lines `- Foo`, `  Installed: 1.2 / Available: 1.3`, `  Orphaned: false`.

### 5.12 Reader background (TC-54)
| theme | themeMode | system dark | result |
|---|---|---|---|
| 0 | any | any | Light |
| 1 | any | any | Dark |
| 2 | any | any | Dark |
| 3 | System | true | Dark |
| 3 | Light | true | Light |
| 9 | any | any | Light |

### 5.13 Layout (TC-53)
| mode | width x height | result |
|---|---|---|
| AUTO | 719 x 800 | Compact |
| AUTO | 720 x 800 | Wide |
| LANDSCAPE | 1000 x 800 | Wide |
| LANDSCAPE | 800 x 800 | Compact |
| ALWAYS | 300 x 300 | Wide |
| NEVER | 2000 x 1000 | Compact |

---

## 6. Test infrastructure

- `kuro-testkit` (Proposal, area 01): in-memory fakes for `Pref`, `DownloadCache`, `Clock` (tokio paused time), `HttpMock` (serves fixture bodies by URL), `PamMock`, `NetMock`.
- Property tests with `proptest` for `invert` (involution for Both, identity for None) and `format_bytes` (monotone thresholds).
- GTK tests (toast, checkbox, bind) need `gtk::init()` and a display. Run under `xvfb-run` in CI (Proposal); mark `#[ignore]` locally when no display.
- `cargo test -p kuro-core` must not require network.

---

## 7. Fixtures (to create under `kuro-core/tests/fixtures/`)

| File | Content |
|---|---|
| `anilist_airing_ok.json` | `{"data":{"Media":{"nextAiringEpisode":{"episode":12,"airingAt":1765000000}}}}` |
| `anilist_airing_null.json` | `{"data":{"Media":{"nextAiringEpisode":null}}}` |
| `anilist_mal_ok.json` | `{"data":{"Media":{"id":1535}}}` |
| `anilist_mal_null.json` | `{"data":{"Media":null}}` |
| `simkl_s3_mal_null_then_mal.json` | two entries: target `simkl_id: 3194924`, `mal: null`, `date` in the future; second entry with `mal: "999"` |
| `simkl_s1_head.json` | the first 700 bytes of `https://data.simkl.in/calendar/anime.json` as observed on 2026-10-08 (kept as a fixture, truncated to complete entries) |
| `chapters_s1.json` | the table row c data (3 units) |
| `chapters_s2_source.json` | three units with `sourceOrder` 0,1,2 |

The Simkl sample in `simkl_s1_head.json` must be re-captured whenever the calendar schema changes (note in tests README).
