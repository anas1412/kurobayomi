# Topic: tracker OAuth callback on Linux (URI scheme, PKCE, pending attempts)

Scope: replaces `TrackLoginActivity` and `BaseOAuthLoginActivity` (`app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/`). Kotlin references: B§12 (behavior), D§5 (endpoints), E-48..E-66 (edge cases). Rust: R§8 (platform), R§10 (summary).

## 1. What the Kotlin flow does in one paragraph
Settings opens the tracker's `authUrl()` in the browser (the browser URL is built in the tracker API classes, D§5.1). The provider redirects to `aniyomi://<tracker>-auth` with either a token in the fragment (AniList implicit grant) or a `code` in the query (Bangumi, MyAnimeList with PKCE, Shikimori, Simkl). Android routes the URI to `TrackLoginActivity` through the intent filter (`app/src/main/AndroidManifest.xml:243-261`). The activity exchanges the code, stores the token, and returns to the settings screen. Nothing is verified with a `state` value; the only protection is the scheme and the host.

## 2. Keep the `aniyomi` scheme and the five hosts
Provider client registrations hold these exact redirect URIs (`aniyomi://bangumi-auth`, `aniyomi://shikimori-auth`, `aniyomi://simkl-auth`, plus the AniList and MAL registrations which the source does not spell out). Changing the scheme means new OAuth applications at each provider. Decision: keep `aniyomi` and the hosts `anilist-auth`, `myanimelist-auth`, `bangumi-auth`, `shikimori-auth`, `simkl-auth`. Trackers that do not use OAuth never receive a callback.

Rejected alternative: a loopback redirect (`http://127.0.0.1:<port>/callback`). Providers only accept the registered redirect, so this needs new client registrations. Keep as a future option if the scheme approach fails on a desktop.

## 3. Linux delivery: `.desktop` file + GApplication `open`
Files the packaging step must install (area:36-ui-shell-navigation-security-webview owns the deep-link packaging; this topic defines the content):

`dev.kurobayomi.Kurobayomi.desktop` (Proposal: application id `dev.kurobayomi.Kurobayomi`, reverse-DNS, must match the GApplication id):
```
[Desktop Entry]
Type=Application
Name=Kurobayomi
Exec=kurobayomi %U
Icon=dev.kurobayomi.Kurobayomi
Categories=AudioVideo;Video;Graphics;
MimeType=x-scheme-handler/aniyomi;
DBusActivatable=true
```
Registration on a dev machine: `xdg-mime default dev.kurobayomi.Kurobayomi.desktop x-scheme-handler/aniyomi` (a helper in `kuro-platform::linux::UriSchemeHandler::register()` runs the same thing through `gio::AppInfo` APIs, not by shelling out; addendum rule 1).

App side (`src/main.rs`, Proposal):
```rust
let app = gtk::Application::builder()
    .application_id("dev.kurobayomi.Kurobayomi")
    .flags(gio::ApplicationFlags::HANDLES_OPEN)
    .build();
app.connect_activate(|app| ensure_main_window(app).present());
app.connect_open(|app, files, _hint| {
    let win = ensure_main_window(app);        // cold start by URI: create window first
    win.present();
    for f in files {
        kuro_track::oauth::route_uri(&f.uri());   // full URI string, including #fragment
    }
});
```
- `GApplication` is unique by default: a second process started with a URI forwards the URI to the first over D-Bus and exits (single instance, addendum `SingleInstance`).
- The `open` handler receives `gio::File` objects for URI arguments. For non-file schemes `file.uri()` returns the original string. Verify the fragment is preserved in T-28 (manual test) before relying on it; if not, use `gio::File::uri()` on a `gio::File::for_uri` and fall back to reading the raw argv string.
- Kurobayomi must not log full URIs: they carry tokens and codes. Log scheme and host only (`aniyomi://anilist-auth`).

## 4. Cold start and warm start
- Cold start by URI (app not running): GApplication starts, `startup` runs, then `open` (not `activate`). `ensure_main_window` builds the window once; `route_uri` then runs the login job. The settings page is not required to exist yet; the login job updates the stored token and the settings page reads it when shown.
- Warm start (app running): `open` runs in the primary instance; `route_uri` runs; the main window is presented.
- The spinner from Kotlin (`BaseOAuthLoginActivity.kt:34-36`) is not reproduced. A progress indicator sits in the tracker row of the settings page while the job runs (area:47 settings screen).

## 5. Routing (pure function, tested)
```rust
pub enum CallbackOutcome {
    AniListToken { token: String, expires_in: Option<i64> },
    Code { tracker: TrackerId, code: String },
    Denied { tracker: TrackerId },
    Malformed { tracker: TrackerId },
    UnknownHost(String),
    NotAllowed { tracker: TrackerId },   // see section 7 (login CSRF)
}

pub fn parse_callback(uri: &str) -> CallbackOutcome;
```
Rules:
1. Scheme must be `aniyomi`; else `UnknownHost`/`Malformed` (the OS would not normally deliver other schemes).
2. Host maps to tracker: `anilist-auth`→AniList(2), `bangumi-auth`→Bangumi(5), `myanimelist-auth`→MyAnimeList(1), `shikimori-auth`→Shikimori(4), `simkl-auth`→Simkl(101). Other hosts → `UnknownHost(host)`. Ids are `TrackerManager` constants (D§2.1).
3. AniList: take the raw fragment (after `#`). Find `access_token=`, anchored at the start of the fragment or after `&`; the value runs to the next `&` or the end of the fragment. No percent-decoding (Kotlin uses the raw substring, `TrackLoginActivity.kt:69-70`). `expires_in` parsed the same way when present. No token → `Denied`.
4. Other trackers: query value `code` = first occurrence, percent-decoded with `percent_encoding::percent_decode_str` (NOT `url::form_urlencoded`, which maps `+` to space; Android `Uri.getQueryParameter` does not). Empty value → `Malformed`. Missing → `Denied`. `error=...` present → `Denied`.

Expected table: `p2-edge-cases-and-tests.md` section 7.1.

## 6. What happens after parsing
| Outcome | Action | Toast (Proposal texts, ui parity 9) |
|---|---|---|
| `AniListToken` | `AniList::login_with_token(token, expires_in)`: identity call (GraphQL `Viewer`), store token JSON + username (= viewer id) + score type; on failure delete anything stored for AniList (matches Kotlin `logout()` in `catch`, but only after a failed exchange) | "AniList logged in" / "AniList login failed" |
| `Code` | `tracker.login_with_code(code, pending)` as D§5.2-5.3 | "<name> logged in" / "<name> login failed" |
| `Denied` | no change to stored state (Kotlin logs out, D-2 changed) | "<name> login cancelled" |
| `Malformed` | no network call | "Unrecognized login link" |
| `UnknownHost` | no network call | "Unrecognized login link" |
| `NotAllowed` | no network call; no change | "Unexpected login link ignored" |
| Missing client secret (Bangumi, Shikimori, Simkl) | not started | "<name> is not configured" |

Every job runs detached on the tokio runtime (R§6). The UI thread only shows the toast.

## 7. PKCE and pending attempts; login CSRF
### 7.1 PKCE (MyAnimeList only)
- Verifier: 50 random bytes from `getrandom`, base64url without padding → 67 characters (`PkceUtil.kt:9-13`). Challenge: the verifier itself (`plain` method; no `code_challenge_method` in the URL, D§5.1).
- Token request: POST `https://myanimelist.net/v1/oauth2/token` with `client_id`, `code`, `code_verifier`, `grant_type=authorization_code` (D§5.2).
- Storage: `PendingLogins` (in memory). One entry per tracker: `{ verifier, created_at }`. A new login start for the same tracker replaces the entry (last attempt wins; MAL's callback does not echo a state value in the source).
- TTL: 5 minutes, the same `PendingLogin` lifetime as part 1 (`p1-topic-tracker-oauth.md` section 4, Proposal shared by both parts). After TTL, or after an app restart, a MAL callback finds no pending attempt → `NotAllowed`/`LoginExpired` message ("Login expired, start again"). Kotlin would exchange with an empty verifier and fail (E-57).

### 7.2 Other trackers
No PKCE. The source sends no `state` value; part 1 adds a per-request `state` and checks it when the provider echoes it (`p1-topic-tracker-oauth.md` section 4). Without an echoed state the callback is only the code.

### 7.3 Login CSRF (Proposal, security)
Problem: any web page can open `aniyomi://bangumi-auth?code=<attacker code>` (the OS hands it to Kurobayomi). Kotlin accepts it and logs the user into the attacker's account. The source has no mitigation.
Proposal: accept a `code` callback for a tracker only if the user started a login for that tracker within the TTL (the `PendingLogins` entry exists for that tracker; the entry is created when the authorize URL is opened). Otherwise → `NotAllowed` and no network call. For AniList the token callback is treated the same way.
Trade-off: a user who starts the login in a different app session (after restart) must restart the login. Accepted.
Open question OQ-3 (below) asks the user to confirm this default.

## 8. Manual paste fallback
If the OS does not route `aniyomi://` (no `.desktop` installed, dev run, Flatpak without portal), the browser shows an unknown-protocol page with the URL in the address bar.
- Settings row "Finish login by pasting the link" (area:47) opens a dialog with a `gtk::Entry`, placeholder `aniyomi://...`, buttons "Cancel" and "OK".
- The pasted text goes through the same `parse_callback`. Errors show the same toasts as section 6.
- Same CSRF rule applies (section 7.3).

## 9. Security notes
- Tokens: stored only through `SecretStore` (R§8). Never in the config file, never in logs, never in backups (B§12; D§3 `__PRIVATE_` keys are excluded from Kotlin backups; the Rust backup format must also exclude them, area:21).
- Client secrets: never committed. Loaded at runtime (R§7.3).
- URIs: logged as scheme+host only (section 3).
- HTTP: all token exchanges over HTTPS (the URLs in D§5 are all `https://`).
- AniList fragment is never sent to a server by the browser, so the token stays client-side until the GraphQL call (`Authorization: Bearer`).

## 10. Open questions (user decisions)
- OQ-1: Client secrets for Bangumi, Shikimori and Simkl are embedded upstream (`bangumi/BangumiApi.kt:269`, `shikimori/ShikimoriApi.kt:259`, `simkl/SimklApi.kt:238`). Keep the upstream values (works immediately, but they are shared and public in the source), or register Kurobayomi's own OAuth applications with those providers and configure the secrets locally? Blocks T-27b-022 acceptance for these three trackers only.
- OQ-2: Does MyAnimeList echo `state` if it is sent? Not documented in the source. Check the provider documentation before adding a `state` value; until then section 7.1 applies.
- OQ-3: Confirm the login-CSRF default (section 7.3): only accept codes for logins started in the same session within 5 minutes. Default: yes.
- OQ-4: Application id `dev.kurobayomi.Kurobayomi` (section 3). Confirm before packaging.

## 11. Tests
- Pure: T-01, T-25, T-26, T-30 (`p2-edge-cases-and-tests.md` section 6).
- Login flow with fake tracker: T-24, T-27, T-29.
- Platform: T-28 (manual, cold start by `xdg-open` with the app closed and open). Checklist: (1) `xdg-open 'aniyomi://bangumi-auth?code=test'` with app closed → window opens, login job runs once; (2) same with app open → same window, no second window; (3) URI with `#access_token=` for AniList → token stored, username = viewer id; (4) a second process started with a URI forwards to the first and exits.
