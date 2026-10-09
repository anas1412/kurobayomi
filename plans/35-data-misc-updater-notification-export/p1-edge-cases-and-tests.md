# Area 35 - edge cases, failure modes, races, tests and fixtures

IDs: N = notifications, U = app updater, X = library export, R = misc XML/resources. Behaviour references are to p1-behavior.md; Kotlin references are repo-relative to `/home/blackbox/aniyomi-src`.

---

## 1. Edge cases

### 1.1 Notifications (N)

| ID | Case | Observed Kotlin behaviour (source) | Required Kurobayomi behaviour |
|---|---|---|---|
| N1 | Notification service down (no D-Bus notification server) | Not applicable on Android; `Context.notify` only checks POST_NOTIFICATIONS (NotificationExtensions.kt:29-40) | `show` returns `NotifyError::Unavailable`; the caller shows the notice as an in-app toast once per notice id per session |
| N2 | Two notices share id 2 | `ID_DOWNLOAD_IMAGE` (Notifications.kt:23) and `ID_APP_UPDATE_PROMPT` (Notifications.kt:90) | Distinct NoticeIds (`save-image`, `app-update.install`); a unit test asserts the registry has no duplicates (`registry::ALL_IDS`) |
| N3 | Cancel on the install prompt removes every notification | `dismissNotification` instance method calls `cancelAll()` (NotificationReceiver.kt:191-194) | `Dismiss` withdraws one id; grep test forbids `withdraw_all` and `cancelAll` |
| N4 | Group summary branch | With exactly two active notifications in the group, only the summary is cancelled (NotificationReceiver.kt:578-581) | No groups on desktop; `Dismiss` withdraws the given id only (documented in T-35a-026 drop) |
| N5 | Progress notice left behind after a download error | `onDownloadError` posts id 3 without withdrawing id 1 (AppUpdateNotifier.kt:308-328, AppUpdateDownloadJob.kt:116-124) | Error path withdraws `app-update.progress` first (design 8.6 step 9) |
| N6 | Title lost on Retry or Download-from-prompt | `startDownloadAppUpdate` ignores `EXTRA_DOWNLOAD_TITLE` (NotificationReceiver.kt:283-286) | `StartAppUpdate` carries `version`; the progress title uses it |
| N7 | Progress updates faster than 200 ms | Gate `now - 200 > lastTick` (AppUpdateDownloadJob.kt:92) | `ProgressThrottle` gap strictly greater than 200 ms; percent strictly increasing |
| N8 | Progress for unknown length | `contentLength` <= 0 yields no updates (AppUpdateDownloadJob.kt:90-96) | No percent updates; body stays "Downloading…" |
| N9 | Notice click after the entry was deleted | `openChapter`/`openEpisode` toast `download_error` ("Error") when a row is missing (NotificationReceiver.kt:226-233, 246-253) | Same toast text (`notify.error.item_missing`) |
| N10 | Mark-read notice clicked while the reader is open on that chapter | Android writes the full snapshot (behaviour 1.9.5), which can overwrite `last_page_read` | Partial update `read = true` only |
| N11 | Mark-read with missing source for `removeAfterMarkedAsRead` | Chapter still marked read, file not deleted (NotificationReceiver.kt:306-312) | Same; log at info |
| N12 | Empty URL array in MARK/DOWNLOAD payloads | Early `return` after dismiss (NotificationReceiver.kt:129, 145, 161, 177) | Same: dismiss, then no-op |
| N13 | Ids missing in payload (-1) | Actions skipped (`mangaId > -1` checks) | Payload validation rejects ids <= 0 (design 6.1) |
| N14 | Oversized payload (many URLs) | Intent extra size limit (about 1 MB, Android binder) | Payload limit 1 MiB; over the limit the action is rejected with a log line |
| N15 | Malicious or malformed `app.notice` payload | Not applicable (broadcast from own app only, protected by package visibility) | JSON parse error => log and ignore; URL/path validation per design 6.1 |
| N16 | Hide-content preference on | Download and library notices hide text (`hide_notification_content`); update notices do not read it (AppUpdateNotifier.kt has no reference) | Update notices are not private (`private_content = false`) by design; library/download notices (areas 23-26) use the gate |
| N17 | Category muted | Android channel importance / user disable (system UI) | `Notifier::show` returns `Ok(false)`; the caller does not toast |
| N18 | Notification click while app is closed | Android starts MainActivity with the shortcut action | GIO notifications require the app to run; the desktop launches the app only if the desktop file's `DBusActivatable`/`Exec` supports it (documented limit; PROPOSAL: ship a desktop file with an action entry that opens the app) |
| N19 | Button label count > 3 | Android allows 3 actions | `Notice::buttons` > 3 => `NotifyError::TooManyButtons` at construction (tested) |
| N20 | Body markup characters | Android text is plain | Escape `&<>` in GIO bodies when the server renders markup (design pitfall 11) |

### 1.2 App updater (U)

| ID | Case | Observed (source) | Required behaviour |
|---|---|---|---|
| U1 | Gate: check on first launch | `lastChecked` default 0 (GetApplicationRelease.kt:15-17) | `may_check(now, 0, false)` is true |
| U2 | Gate boundary | `now.isBefore(last + 3 days)` skips (GetApplicationRelease.kt:23-27) | `now >= last + 3d` proceeds; `now == last + 3d` proceeds |
| U3 | Forced check bypasses gate | `forceCheck` (GetApplicationRelease.kt:23) | `force = true` always proceeds |
| U4 | No matching asset | `latest` returns null (ReleaseServiceImpl.kt:25); no timestamp update | `NoNewUpdate`, timestamp untouched; manual toast says "No build for this platform" (UI change) |
| U5 | Timestamp for a found release | set before comparison (GetApplicationRelease.kt:33) | Set for every non-null release (parity) |
| U6 | Older tag offered (E1, E2) | digit-wise compare returns true (GetApplicationRelease.kt:68-72) | `semver` ordering; regression tests `2.5.0 vs 2.0.1` and `1.0.0 vs 0.9.9` => no update |
| U7 | Four-part tag (E6) | `1.0.0.1` vs `1.0.0` compared only to old part count | Semver rejects four parts => `BadVersion`; release tags must be semver (documented) |
| U8 | Non-numeric tag | NumberFormatException (E4) | `BadVersion` error (no panic); manual toast shows the message |
| U9 | Release body null | Parse error (GithubRelease.kt `body: String` non-null) | Missing body = empty notes |
| U10 | Concurrent checks | Both run (no guard) | `checking` mutex serialises checks; second caller waits and then sees the throttle (non-forced) |
| U11 | Rate limit | 403/429 becomes `HttpException` with code | `RateLimited` with the friendly text |
| U12 | Debug build version suffix (E5) | `0.18.2.1-123` becomes `0.18.2.1123` | Kurobayomi `CARGO_PKG_VERSION` is plain semver; debug builds compare correctly |
| U13 | Download URL not GitHub | Accepted as-is | Rejected: `StartAppUpdate` host check (design 6.1) |
| U14 | Redirect from GitHub asset to objects host | Handled by OkHttp | reqwest follows redirects (default 10) |
| U15 | Partial file on crash | Android `saveTo` deletes partial file on exception only; a hard kill leaves `update.apk` partially written | Write to `.part`; rename on success; on startup delete stale `.part` files older than 24 h (PROPOSAL) |
| U16 | Checksum mismatch | Not checked (Android) | Mismatch => delete `.part`, error notice `Checksum` text "Download error" (no installer path) |
| U17 | Cancel during write | `CancellationException` => cancel notice, no error (AppUpdateDownloadJob.kt:117-120) | Cancel token; delete `.part`; withdraw progress; no error notice |
| U18 | Start while downloading | `ExistingWorkPolicy.REPLACE` (AppUpdateDownloadJob.kt:149) | Cancel old task, await end, start new |
| U19 | Offline at start | WorkManager waits for `CONNECTED` | `is_online` false => progress notice "Waiting for network" and await `online_events` |
| U20 | Metered network | `CONNECTED` allows metered (E-U5) | Allowed; PROPOSAL: a notice button "Download later" is not added (kept parity) |
| U21 | Apply update on Linux | Installer intent (APK) | Reveal file; package-managed installs show no download |
| U22 | Update check disabled (repo empty) | Feature flag off (BuildConfig.kt:12) | `UpdateError::Disabled`; About row disabled with hint |
| U23 | Changelog with no Checksums | Unchanged text (NewUpdateScreen.kt:24-26) | Unchanged |
| U24 | Changelog Checksums section regex greed | `(\R|.)*` consumes to end (NewUpdateScreen.kt:25) | Equivalent truncation; test with text after the section (the rest is dropped too; parity) |

### 1.3 Library export (X)

| ID | Case | Observed (source) | Required behaviour |
|---|---|---|---|
| X1 | No columns selected | Rows are empty strings joined by CRLF (LibraryExporter.kt:85-94) | Same bytes (test) |
| X2 | Zero favourites | Empty file, success toast (LibraryExporter.kt:47-61) | Empty file, success toast (parity); PROPOSAL note in toast |
| X3 | Null author/artist | Empty cell (`isNullOrBlank`) | Empty cell |
| X4 | Whitespace-only value | Empty cell (`isNullOrBlank` true) | Empty cell (`trim().is_empty()`) |
| X5 | Leading or trailing spaces | Not quoted, kept (LibraryExporter.kt:88-92) | Kept, not quoted |
| X6 | Quote, comma, CR, LF in value | Quoted, quotes doubled | Quoted, quotes doubled |
| X7 | Output encoding | UTF-8 without BOM | UTF-8 without BOM |
| X8 | Output file exists | `openOutputStream` truncates (mode default "w") | Write temp then rename (replaces) |
| X9 | Stream unavailable (null from ContentResolver) | Silent success (LibraryExporter.kt:55-59) | Not applicable on Linux; any IO error => error toast (defect fixed) |
| X10 | Read-only target | IOException propagates to an unhandled coroutine (SettingsDataScreen.kt:376-388) | Error toast with the OS message |
| X11 | Favourites change while dialog open | Stale snapshot (SettingsDataScreen.kt:366-370) | Snapshot taken at Save |
| X12 | Anime and manga with same title | Both rows, type column distinguishes | Same |
| X13 | Very large library (100 000 rows) | Whole string in memory | Acceptable at this size (PROPOSAL: stream rows if above 100 000; measured later) |
| X14 | Spreadsheet formula injection (`=HYPERLINK(...)`) | Not escaped | Not escaped (parity); documented |
| X15 | Options combinations with Title off | UI forces others off; renderer does not enforce | Renderer follows options as given (parity); UI enforces |

### 1.4 Misc XML and resources (R)

| ID | Case | Observed | Required behaviour |
|---|---|---|---|
| R1 | Cleartext HTTP to sources | Allowed (network_security_config.xml:4-6) | reqwest allows http by default; no config needed (T-35a-031) |
| R2 | User-installed CA certificates | Trusted (network_security_config.xml:12-14) | Native roots feature loads the system store; user CAs installed into the system store are trusted (documented) |
| R3 | FileProvider authority | `${applicationId}.provider` (AndroidManifest.xml:299) | Not applicable |
| R4 | S Pen remote actions | Registered on Reader and Player (AndroidManifest.xml:202-204, 219-220) | Dropped; keyboard equivalents (T-35a-028) |
| R5 | Search hint | "Global search…" (searchable.xml:3) | In-app search placeholder text uses the same string (owned by area:40-ui-browse-manga/41) |

---

## 2. Failure modes and handling (table)

| Failure | Where | Handling (Kurobayomi) | User-visible result |
|---|---|---|---|
| GitHub unreachable | check | `UpdateError::Network` | Auto: log only. Manual: toast with error text |
| GitHub 404 | check | `NoReleases` | Auto: log. Manual: toast "No releases published yet" |
| GitHub 403/429 | check | `RateLimited` | Auto: log. Manual: toast rate-limit text |
| Malformed JSON | check | `Network(msg)` (parse error) | Same as unreachable |
| Asset missing | check | `Ok(None)` | Manual: "No build for this platform in the latest release" |
| Download HTTP error | download | `Http(code)` | Error notice with Retry, `.part` deleted |
| Disk full mid-write | download | `Io` | Error notice, `.part` deleted |
| Checksum mismatch | download | `Checksum` | Error notice, `.part` deleted |
| Cancel | download | token | Progress withdrawn, `.part` deleted, no error notice |
| Notification service down | any | `NotifyError::Unavailable` | In-app toast fallback |
| Reveal fails (no file manager) | reveal | `OpenError` fallback to parent dir; then toast | Toast "Could not open: <error>" |
| Export write fails | export | `ExportError::Write` | Error toast "Export failed: <error>" |
| Payload invalid | router | `ActionError` | Log warn, no-op |
| Entity missing (chapter deleted) | router | `None` from lookup | Toast "Error" (`notify.error.item_missing`) |
| DB error during mark-read | router | `Err` | Log error; toast "Could not update entries" (PROPOSAL; Android swallows) |

---

## 3. Race conditions and ordering

1. Mark-read vs reader progress save (N10): the Android full-row update can overwrite `lastPageRead`. Mitigation: partial update (`read` only). Test: read `last_page_read = 7`, run mark-read action, assert it is still 7.
2. Two download starts in a row (U18): the second start must wait for the first to stop before writing `.part`. Test: start A, start B immediately; assert A's `.part` is removed and only B's final file exists.
3. Check during check (U10): mutex. Test: two concurrent `check(false)` calls; the HTTP mock sees one request.
4. Download completes while the window is closing: the install notice is posted from the tokio task via the main context; if the application has quit, `glib::MainContext::invoke` runs nothing and the file remains in `updates/` (acceptable; next start does not re-prompt). Test: drop the app context before completion; no panic.
5. Notice click arrives during download replace (N6 + U18): the router runs on the main thread; `StartAppUpdate` awaits the old task's join handle in a tokio task, so the UI never blocks.
6. Export snapshot vs library change (X11): the snapshot runs on tokio at Save. A change committed after Save starts is not included; acceptable.
7. Progress notice withdraw racing with a new progress post: both go through `invoke` in order, so the last call wins. Test: withdraw after the final post; assert the notice is absent (fake notifier records order).
8. Startup check while the window is not yet shown: the update screen is pushed only after the main window is mapped (area:36-ui-shell-navigation-security-webview). The service returns the result; the UI event is queued until the window exists.

---

## 4. Upstream tests to port (from *Test.kt in scope or adjacent)

Scope files contain no `*Test.kt`. The adjacent test is `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`, which tests the throttle and version rule used by the checker. Port all four cases:

| Upstream test name (exact) | Setup | Expected | Rust test name |
|---|---|---|---|
| `When has update but is preview expect new update` | preview, commitCount 1000, tag `r2000`, versionName "", lastChecked 0 | `NewUpdate` with the same release | `update::check::tests::preview_update_offered_when_tag_number_is_higher` |
| `When has update expect new update` | release, tag `v2.0.0`, current `v1.0.0`, lastChecked 0 | `NewUpdate` | `update::check::tests::release_update_offered_for_higher_semver` |
| `When has no update expect no new update` | release, tag `v1.0.0`, current `v2.0.0` | `NoNewUpdate` | `update::check::tests::no_update_for_older_tag` |
| `When now is before three days expect no new update` | lastChecked = now, not forced | `NoNewUpdate`, service not called | `update::check::tests::throttle_blocks_within_three_days_without_request` |

Rust translation notes: the `Preference<Long>` mock becomes an `AppStateStore` fake; `releaseService.latest` becomes a fake `fetch` closure; "service not called" becomes a counter assertion.

---

## 5. New tests to write (Rust)

### 5.1 Version and asset (update)

- `version::parse_tag` accepts `v0.2.0`, `0.2.0`; rejects `latest`, `1.2`, `` (empty).
- `is_newer("2.5.0", "v2.0.1") == false` (E1 regression).
- `is_newer("1.0.0", "v0.9.9") == false` (E2 regression).
- `is_newer("1.0.0", "v1.0.0-rc.1") == false` (prerelease lower than release).
- `is_newer("1.0.0", "v1.0.0") == false`; `is_newer("1.0.0", "v1.0.1") == true`.
- `is_newer` on `1.2.3` vs `1.2` returns `Err(BadVersion)` and does not panic (E3 regression).
- `pick_asset` with fixture A (the captured release): arch `x86_64` returns `aniyomi-x86_64-v0.18.2.1.apk`-style asset (for Kurobayomi tests use an equivalent fixture named `kurobayomi-0.2.0-x86_64.AppImage`).
- `pick_asset` with only a universal asset returns the universal asset (tier 2).
- `pick_asset` with `x86` asset and arch `x86_64` returns None (the `x86` asset is not universal and does not match arch).
- `pick_asset` prefers AppImage over tar.zst within tier 1.
- `pick_asset` first match wins when two tier-1 assets exist (documented deviation from Kotlin `associate`).

### 5.2 Notes

- `rewrite_mentions("thanks @user-name")` == `thanks [@user-name](https://github.com/user-name)`.
- `rewrite_mentions("@-bad")` unchanged; `rewrite_mentions("foo@bar")` unchanged (word char before @).
- `rewrite_mentions("@a-")` == `[@a](https://github.com/a)-`.
- 40 characters after `@`: only the first 39 are linked; the 40th stays as text.
- `strip_checksums_section`: removes from the `---` line before `### Checksums` to the end; no Checksums => identical text.
- `sha256_for`: table form (`| Universal | <hex>`) mapped to the universal asset; `sha256sum` form (`<hex>  <name>`) mapped by name; no match => None; uppercase hex accepted and lowercased.

### 5.3 Throttle

- Offer sequence on `total = 100` (each call is `offer(t, bytes, Some(100))`): (t=0, bytes=10) -> Some(10); (t=100 ms, bytes=20) -> None (gap 100 ms is not more than 200 ms); (t=200 ms, bytes=20) -> None (gap exactly 200 ms is not strictly more than 200 ms); (t=201 ms, bytes=20) -> Some(20); (t=300 ms, bytes=20) -> None (percent not greater than 20); (t=450 ms, bytes=21) -> Some(21) (gap 249 ms).
- `total = None` and `total = Some(0)` => None always.

### 5.4 Actions

- Round-trip `to_target`/`from_target` for every AppAction variant (fixture list in the test).
- Payload > 1 MiB => `ActionError::TooLarge`.
- Unknown `kind` => `ActionError::Parse`.
- Validation (design 6.1): `StartAppUpdate` with `http://` => rejected; host `evil.example` => rejected; host `objects.githubusercontent.com` => accepted; `RevealFile` outside allowed roots => rejected; ids <= 0 => rejected.

### 5.5 Registry and mute

- `ALL_IDS` has no duplicates; count equals 21 entries plus `app-update.*` three and `save-image`: the test asserts the number of distinct ids equals `ALL_IDS.len()`.
- `Notifier::show` with a muted category returns `Ok(false)` and records no call (fake notifier).
- Private content + hide setting on => `Ok(false)`.
- Three buttons ok; four buttons => `TooManyButtons`.

### 5.6 Export

- Byte-exact cases from behaviour 3.4: the Frieren row; the quoted-title row; two-row CRLF join; zero favourites => `""`.
- Zero columns with two rows => `"\r\n"`; with one row => `""`.
- Whitespace-only author => empty cell; leading space kept unquoted.
- Quote doubling: `He said "hi"` => `"He said ""hi"""`.
- `write_csv` writes via temp and renames; a failure leaves no temp file (test with a read-only directory).

### 5.7 Downloader (integration)

Use a local test server (tokio `TcpListener` with a fixed HTTP/1.1 response) and a temp directory from `Paths` fake:
- 200 with body of 10 chunks: final file exists with the exact bytes; `.part` gone; progress notices are increasing and at most one per 200 ms (fake clock via injected `Instant` source or real sleeps of 250 ms in the server).
- 404: error notice `app-update.error` posted; `.part` gone; progress notice withdrawn before the error notice (order recorded by the fake notifier).
- Cancel mid-way: no error notice; `.part` gone; progress withdrawn.
- Checksum mismatch: error notice; file not renamed.
- Start while running: the first task is cancelled; only one final file.
- Offline at start: no request sent until the fake `online_events` emits `true`.

### 5.8 Router (unit, with fakes)

- `OpenChapter` with missing chapter => toast `Error`, no navigation.
- `MarkChaptersRead` with `notice: Some(id)` => withdraw called before the DB call (order recorded).
- `Dismiss` => only that id withdrawn.

---

## 6. Fixtures

| Fixture (planned path) | Content | Source |
|---|---|---|
| `crates/kuro-platform/tests/fixtures/github_latest_aniyomi.json` | Captured `releases/latest` response for `aniyomiorg/aniyomi`: `tag_name` v0.18.2.1, `html_url`, `body` (562 characters including the `### Checksums` table), five assets | captured with `gh api repos/aniyomiorg/aniyomi/releases/latest` on 2026-10-08 (read-only) |
| `tests/fixtures/github_latest_kurobayomi.json` | Synthetic release for Kurobayomi: tag `v0.2.0`, assets `kurobayomi-0.2.0-x86_64.AppImage`, `kurobayomi-0.2.0-aarch64.AppImage`, `kurobayomi-0.2.0.tar.zst` (universal), `kurobayomi-0.2.0-x86.tar.zst` (32-bit, must not match) | authored for tests |
| `tests/fixtures/library_favorites.json` | 3 anime and 3 manga rows including: null author, whitespace author, comma/quote/newline title, leading-space title | authored for tests |
| `tests/fixtures/body_with_checksums.md` | release body containing a `---` line, `### Checksums` table with 5 rows | from the captured body |
| `tests/fixtures/sha256sum_format.txt` | three lines `<hex>  <asset>` | authored |

Fixtures are data only; no secrets. The captured JSON contains public GitHub URLs only.

---

## 7. Manual verification checklist (desktop)

1. KDE Plasma (Wayland): trigger a fake update via the test build flag; the notice appears, body click opens the release page, "Download" starts the download, progress text updates, install notice appears and "Open folder" opens Dolphin with the file selected (`FileManager1.ShowItems`).
2. GNOME (Mutter) with Xwayland: same flow; notifications use the same backend.
3. Stop the notification daemon (`systemctl --user stop plasma-plasmashell` is not needed; use `gdbus` to confirm the name is absent): the in-app toast appears instead.
4. Export: choose a path on an external disk; check that the file is CRLF with the expected bytes (`file` and `xxd`).
5. Export to a read-only folder: error toast, no `.kuro-tmp` left behind.
6. About "Check for updates" with `update.repo` empty: row disabled with hint.
7. About "Check for updates" while offline: error toast (network text), spinner stops.
8. Click a manga-download notice after deleting that manga: toast "Error", no crash.

---

## 8. Test infrastructure

- `cargo test -p kuro-platform` and `cargo test` (kurobayomi crate).
- Fake implementations live in `src/testing.rs` behind `#[cfg(test)]`: `FakeNotifier` (records `Vec<Event>` where `Event` is `Show(Notice)`, `Withdraw(NoticeId)`), `FakePaths` (temp dir), `FakeNetwork` (flag plus a channel), `FakeAppState` (`Mutex<i64>`).
- The HTTP tests use `tokio::net::TcpListener` on `127.0.0.1:0`; no external network in unit tests (CI safe).
- The GitHub fixture test calls `fetch_latest` against a local server replaying the fixture (base URL injected for tests; the production base URL is a constant).
- Clippy: `-D warnings`; no `unwrap` in non-test code (grep check).
