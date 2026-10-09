# Area 36 — Edge cases, failure modes, races, upstream tests, new tests, fixtures

Upstream test files in scope: none. `find` over the 42 scope files finds no `*Test.kt` in this area (checked with the areas listing). The tests below are new, and each cites the behaviour section it covers.

---

## 1. Edge cases (from the Kotlin, with the Rust expectation)

| # | Area | Input / state | Kotlin result | Rust expectation | Task |
|---|---|---|---|---|---|
| E1 | Launch | second instance started while first runs | `isTaskRoot == false` -> finish | GApplication forwards args; second process exits 0 | T-36a-001 |
| E2 | Launch | migration fails or never started | `Migrator.await()` returns false (CompletableDeferred(false)) -> no changelog | window still shown; changelog not shown | T-36a-001, T-36a-021 |
| E3 | Launch | shortcut SHOW_MANGA without `manga` extra | `return false` before `ready = true`; splash waits up to 5 s | `RouteError::MissingId`, window shown immediately | T-36a-011, T-36a-020 |
| E4 | Launch | ACTION_VIEW with unknown scheme | returns null -> ready true, nothing pushed | no-op, window shown | T-36a-014 |
| E5 | Search | `type` extra = "BOTH" | `DeepLinkScreenType.valueOf` throws IllegalArgumentException (crash) | `RouteError`, toast, no crash | T-36a-012 |
| E6 | Search | empty query | nothing pushed, ready true | no-op | T-36a-012 |
| E7 | Search | query is a URL but no source resolves it | DeepLink page -> NoResults -> replace with global search | same | T-36a-056, T-36a-057 |
| E8 | Backup | `content://...tachibk` URI | matched by `endsWith(".tachibk")` | matched by MIME + extension check; `content:` is not a Linux case, treat as file path | T-36a-014 |
| E9 | Repo deep link | `tachiyomi://add-repo` without `url` | nothing | nothing | T-36a-014 |
| E10 | Lock | cancel on biometric prompt | `finishAffinity()` (process exits) | overlay closes, app keeps running (intentional change) | T-36a-034 |
| E11 | Lock | `onResume` fires repeatedly while locked | launches UnlockActivity each time | single overlay (guard) | T-36a-034 |
| E12 | Lock | credential removed while lock on | `useAuthenticator` silently set false | Linux: passphrase cannot be "removed"; if lock file missing, setup prompt, lock off with a visible notice | T-36a-033 |
| E13 | Lock | lock_after = 0 (always) and app killed | `requireUnlock` stays true at next start | same | T-36a-032 |
| E14 | Lock | lock_after = 5, app closed 2 min ago | `last + 300000 <= now` false -> not locked | same | T-36a-032 |
| E15 | Lock | lock_after = 5, no stored close time (crash) | `0 + 300000 <= now` true -> locked | same | T-36a-032 |
| E16 | Lock | lock_after = -5 (unexpected negative) | `else` branch -> time comparison | same as Kotlin (arm `m =>`) | T-36a-032 |
| E17 | Lock | `isAuthenticating` stuck true (callback never fires) | onApplicationStopped skips timestamp; lock never re-arms | timeout 60 s on the prompt clears the flag (proposal) | T-36a-033 |
| E18 | Tabs | start_screen = BROWSE and bottom style moves Browse to More | `defaultTab == moreTab` -> goToStart goes to AnimeLibrary | `start_target` rule | T-36a-008 |
| E19 | Tabs | current = moreTab | bar hidden, BackHandler enabled | same | T-36a-005, T-36a-008 |
| E20 | Tabs | start_screen changed at runtime | not applied until restart (object init) | applied on next Home build (proposal) | T-36a-004 |
| E21 | Badges | updates count set, show flag off | badge 0 | same | T-36a-007 |
| E22 | Badges | very large count (1000+) | shows full number | same, no cap | T-36a-007 |
| E23 | Search toolbar | Enter with blank query | nothing | nothing | T-36a-039 |
| E24 | Search toolbar | reset clicked while query empty | reset button not shown | same | T-36a-039 |
| E25 | Search parity | TabbedScreen with odd tab count | page index % 2 picks wrong query | per-tab binding | T-36a-040 |
| E26 | AppBar | count > 0 and title null | `title!!` NPE | title optional; empty string shown | T-36a-037 |
| E27 | WebView | no URL extra | `url_key ?: return` -> blank window | error page with "no URL" message (proposal) | T-36a-053 |
| E28 | WebView | `source_key` missing | `getLong` returns 0 -> no source | same | T-36a-053 |
| E29 | WebView | clear cookies for invalid URL | activity: `toHttpUrl` throws; screen model: `toHttpUrlOrNull` safe | safe parse, toast | T-36a-053 |
| E30 | WebView | page HTML contains marker in a link text | banner shown | same (marker match is literal) | T-36a-050 |
| E31 | WebView | `intent://` link | blocked | blocked | T-36a-051 |
| E32 | WebView | `blob:http` link | allowed | allowed | T-36a-051 |
| E33 | WebView | navigation request also re-loads with headers | double request | single request | T-36a-051 |
| E34 | WebView | `onPageFinished` JS evaluation fails | inferred: `getHtml` (suspend over `evaluateJavascript`) would never resume if the callback is not invoked (e.g. page torn down); not observed in tests | timeout 3 s then treat as no marker | T-36a-050 |
| E35 | Deep link | source throws during resolve | launchIO exception, page stuck in Loading | Error state with Retry | T-36a-056, T-36a-057 |
| E36 | Deep link | episode in DB but source URL changed | local lookup by URL misses -> remote fetch | same | T-36a-056 |
| E37 | Deep link | composition re-runs Result branch | navigator.replace / startActivity repeated | one-shot effect | T-36a-056 |
| E38 | Onboarding | Finish clicked with storage unset | Next disabled on step 2 (isComplete false) | same | T-36a-026 |
| E39 | Onboarding | window closed before Finish | flag stays false; next launch shows onboarding | same | T-36a-024 |
| E40 | Onboarding | restore backup chosen | finish, then Settings with highlight | same | T-36a-062 |
| E41 | Sheet | nested sheet pages > 1 and Escape | BackHandler pops inner page | same | T-36a-042 |
| E42 | Banner | indexing and downloaded-only both on | three-row stack, top inset math | same stacking, no double inset | T-36a-018 |
| E43 | Theme | AMOLED on, light mode | overlay still applied (Kotlin applies amoled overlay regardless of mode) | CSS class only in dark mode (proposal, documented change) | T-36a-029 |
| E44 | Theme | appTheme = MONET stored on Linux | dynamic colour | effective() -> Default | T-36a-029 |
| E45 | Date | timestamp 0 or negative | not_applicable | same | T-36a-047 |
| E46 | Date | app_date_format invalid pattern | IllegalArgumentException | fallback to SHORT and log | T-36a-047 |
| E47 | Changelog | debug build | not shown | not shown | T-36a-021 |
| E48 | Update | release changelog contains no checksum block | regex no-op | same | T-36a-023 |
| E49 | Update | accept while download already running | job start (area 35 rules) | same, owner area decides | T-36a-023 |
| E50 | Tabs | bar tab click on selected tab | onReselect | same | T-36a-008 |
| E51 | Startup | window resized below 760 during reader page | chrome decides sidebar | immersive rule overrides | T-36a-005 |
| E52 | Keyboard | Escape with search open and non-empty | (Android back closes search via nav icon) | Escape clears search first | T-36a-039 |
| E53 | GApplication | `open` before `activate` | n/a | queued until window ready | T-36a-015 |
| E54 | Lock | window snapshot in task switcher | FLAG_SECURE prevents | no equivalent; blank content before lock overlay | T-36a-034 |
| E55 | Notification | dismiss id unknown | `dismissNotification` cancels all (`cancelAll`) | withdraw the id only (intentional tightening; note in area 35) | T-36a-010 |

---

## 2. Failure modes and handling

| Failure | Where | Handling (Rust) |
|---|---|---|
| Migration panic / error | startup | log, show error dialog with "Quit"; do not show window with a half-migrated DB |
| Portal unavailable (folder chooser) | onboarding step 2 | fall back to `gtk::FileDialog`; if that fails, toast `file_picker_error` |
| WebKitGTK missing | WebView open | toast `information_webview_required`, do not open window |
| fprintd absent or timeout | lock | fall back to passphrase; log at info level |
| Argon2 verify error (bad lock.json) | lock | treat as corrupted: show setup dialog to re-create passphrase; do not unlock |
| Lock file unreadable | lock | fail closed: stay locked, show message, offer passphrase reset via a recovery phrase (proposal: not in scope; flag to area 02) |
| Resolve source throws | deep link | error state with Retry; log with source id |
| Resolve network timeout | deep link | error state after 30 s (proposal) |
| Cookie store locked | WebView | retry once after 200 ms, then toast |
| Share/open with no default browser | WebView | toast `information_webview_required`-like message `open_in_browser_failed` (proposal key; verify in area 51) |
| D-Bus unavailable (notifications, portals) | shell | degrade silently; GTK toast for notifications |
| Tokio runtime panic in a task | any | task is isolated (`spawn`), error logged; UI shows error state where the page waits on it |

---

## 3. Races and concurrency

| Race | Description | Mitigation |
|---|---|---|
| R1 | Incognito toggled while Browse source page is open | `watch` on incognito pref; pop on off (behaviour §1.2). Debounce: apply on the GTK thread only. |
| R2 | Two route requests arrive in quick succession (open + search) | queue applied in arrival order on GTK thread (T-36a-015) |
| R3 | Deep-link resolve finishes after the user navigated away | `CancellationToken` cancelled on page drop; late result discarded |
| R4 | Lock timestamp written on stop and read on start in the same second | `on_app_stopped` runs before app exit; `on_app_started` runs after window build; no concurrent access (both on GTK thread) |
| R5 | `isAuthenticating` set true, prompt window destroyed | prompt callback on destroy clears the flag; timeout 60 s (E17) |
| R6 | Lock overlay raced by resume | single overlay guard (E11) |
| R7 | Tab channel send blocks a tokio task | bounded channel with drop-oldest (T-36a-009) |
| R8 | Badge watch receivers updated from many sources | `watch` keeps last value only; no lost-update issue |
| R9 | WebView `decide-policy` called from a non-GTK thread | never: WebKit signals are GTK-thread only |
| R10 | Migration completes after the window is shown | window is not shown until migration future resolves (T-36a-001) |
| R11 | Changelog state read before migration | derived from the awaited result only |
| R12 | Onboarding Finish and Settings restore race | finish writes flag first, then navigates |

---

## 4. Upstream tests to port
None in this area. Kotlin tests for the callees (e.g. `SecurityPreferences`, `NavStyle`) are not present in this scope. Do not invent upstream test names.

---

## 5. New tests to write (Rust)

Unit tests (no GTK, `cargo test`):

| Test | Asserts | Covers |
|---|---|---|
| `nav_style_lists` | for each NavStyle, `bar_tabs()` equals the expected 5 entries in order; more tab correct | T-36a-006 |
| `back_rule_truth_table` | `back_enabled` equals `(A||B)&&(C||D)` for all 16 combinations | T-36a-008 |
| `start_target_rule` | default==more gives AnimeLibrary; otherwise default | T-36a-008 |
| `bar_visibility` | hidden when current == more; hidden when requested false | T-36a-005 |
| `route_shortcut_*` | each ShortcutAction gives the NavAction list from behaviour §4.1 table; missing id gives MissingId | T-36a-011 |
| `route_search_default_anime` | missing kind gives Anime; empty query gives no actions | T-36a-012 |
| `route_search_unknown_kind` | `"BOTH"` gives RouteError, not panic | T-36a-012 |
| `route_ext_search` | pushes GlobalSearch with filter; empty query no-op | T-36a-013 |
| `route_file_tachibk` | PopToRoot then RestoreBackup; `.zip` ignored | T-36a-014 |
| `route_uri_repo` | tachiyomi add-repo, aniyomi add-repo, aniyomi extension-store; missing url no-op | T-36a-014 |
| `startup_gate_times` | keep_hidden at 0, 499, 501, 4999, 5001 ms with ready false/true (table) | T-36a-020 |
| `changelog_rule` | true only when migrated and not debug | T-36a-021 |
| `lock_table` | for enabled x authenticating x require_unlock x lock_after in {-1,0,5,30,-5} with last in {none, now-1min, now-10min}: result equals the Kotlin table generated by a reference implementation in the test | T-36a-032 |
| `lock_stop_writes_only_when_delayed` | write only when lock_after > 0 and not requireUnlock and not authenticating | T-36a-032 |
| `lock_start_deletes_timestamp_when_enabled` | key removed when enabled, kept when disabled | T-36a-032 |
| `theme_parse_all_names` | 22 stored names parse; unknown gives Default | T-36a-029 |
| `theme_effective_monet` | Monet -> Default | T-36a-029 |
| `chrome_for_matrix` | modes x widths {500, 759, 760, 900} x orientations gives Chrome per behaviour §8.2 | T-36a-031 |
| `updates_badge_rule` | show off gives 0; sum otherwise | T-36a-007 |
| `date_text_zero` | timestamp 0 gives not_applicable key | T-36a-047 |
| `date_format_fallback` | invalid pattern gives SHORT style | T-36a-047 |
| `cloudflare_markers` | both markers detected; false positives like "Ray ID" alone not detected | T-36a-050 |
| `navigation_policy` | blob:http Allow; intent:// Block; https Allow | T-36a-051 |
| `changelog_regex` | fixture with checksum section removed, text before intact | T-36a-023 |
| `parse_kind` | ANIME, MANGA parse; other fails | T-36a-055 |
| `resolve_*` (with fake sources) | source selection, NoResults, Entry with and without child, error path | T-36a-056, T-36a-057 |
| `argon2_roundtrip` | set then check true; wrong passphrase false; file mode 0600 | T-36a-033 |
| `onboarding_accept_flow` | step 4 accept gives Finished; back from step 1 is no-op | T-36a-024 |

Integration / smoke tests (GTK, run under Xvfb or a Wayland session, `KUROBAYOMI_SMOKE=1`):

| Test | Steps | Expected |
|---|---|---|
| `smoke_start_tab` | launch with start_screen=MANGA | manga library page selected |
| `smoke_badges` | set counts 3 and 4 with show on | updates badge shows 7 |
| `smoke_back` | open a page, press Escape | previous page; Escape on start tab does nothing |
| `smoke_onboarding_gate` | fresh config dir | onboarding page shown, Next disabled on storage step until folder chosen (portal stubbed) |
| `smoke_lock_overlay` | lock enabled, stop and start with lock_after=0 | overlay shown, content not visible |
| `smoke_webview_cookie` | open webview to a local test server that sets a cookie | cookie appears in the HTTP jar (area 12 test helper) |
| `smoke_deeplink_tachibk` | `kurobayomi file.tachibk` | restore page opened |
| `smoke_sheet_nested` | open sheet, push inner page, Escape | inner page closed, sheet still open |

Property / fuzz (optional, P2): `route_request` never panics for arbitrary query strings and kinds.

---

## 6. Fixtures needed

| Fixture | Content | Used by |
|---|---|---|
| `tests/fixtures/changelog_with_checksums.md` | release notes with a `---` line followed by `Checksums` block | changelog regex test |
| `tests/fixtures/cloudflare_challenge.html` | HTML containing `window._cf_chl_opt` | cloudflare test |
| `tests/fixtures/cloudflare_ray.html` | HTML containing `Ray ID is` only | cloudflare test |
| `tests/fixtures/plain_page.html` | page without markers | cloudflare negative test |
| `tests/fixtures/lock_v1.json` | valid lock file with a known Argon2id PHC for passphrase `test-passphrase` | argon2 test (test-only passphrase) |
| `tests/fixtures/app_state_kv.rs` (in-memory KV) | implements `AppStateKv` | lock tests |
| `tests/fixtures/fake_sources.rs` | `ResolvableSource` fakes with URL types Unknown, Anime, Episode, Manga, Chapter | deep-link tests |
| `tests/fixtures/sample.tachibk` | a tiny backup file (zero entries) | file route test (no parse; only extension check) |
| Smoke config dir | `XDG_CONFIG_HOME` and `XDG_DATA_HOME` pointing to a temp directory per test | smoke tests |
| Local HTTP test server | serves a page with `Set-Cookie` and a `window._cf_chl_opt` page | webview tests |

---

## 7. Accessibility checks (manual and automated)
- Every icon-only button has an accessible name (`gtk::Accessible::set_accessible_name`), using the string keys from behaviour §16.
- Focus ring visible on every interactive control.
- Colour contrast of banner text vs. banner colour at least 4.5:1 for the default GTK theme (manual check).
- Reduced-motion: with `gtk-enable-animations = false`, transitions are instant (test by toggling the setting).

---

## 8. Known gaps this area will not close (recorded)
1. Swipe-to-dismiss on sheets: not implemented (desktop has no swipe).
2. Marquee subtitle: replaced by ellipsis plus tooltip.
3. Haptic feedback: dropped.
4. Screen capture protection: dropped (no Linux API).
5. Dynamic colour (Monet): dropped (no Linux equivalent); the GTK theme is the palette.
6. Android share sheet: replaced by clipboard copy.
7. Battery optimisation and unknown-app-source rows: dropped.
8. Notification permission row: replaced by status row.
9. Assist content: dropped.
10. Trampoline activities for deep links: replaced by GApplication routing.
