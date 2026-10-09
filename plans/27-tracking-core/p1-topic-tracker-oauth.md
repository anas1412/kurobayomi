# Topic: tracker login and OAuth plumbing on Linux

Scope: how a user logs in to each tracker, how the redirect returns to the app, where the credentials go, and how logout works. Source: `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/BaseOAuthLoginActivity.kt`, `.../ui/setting/track/TrackLoginActivity.kt` (part 2 owns the activities; this topic designs the Linux replacement), `presentation/more/settings/screen/SettingsTrackingScreen.kt`, `domain/track/service/TrackPreferences.kt`, `data/track/BaseTracker.kt`, the `authUrl()` builders and `saveCredentials` call sites listed in `p1-behavior.md` section 17 and `p1-data-and-config.md` sections 3 and 10. Tracker-specific token exchange is areas 28 to 33.

## 1. Flows in the source (OBSERVED)

| Flow | Trackers | User action | Browser | Return | Stored |
|---|---|---|---|---|---|
| Implicit grant | AniList | tap row | `AnilistApi.authUrl()` with `response_type=token`, `client_id` | `aniyomi://anilist-auth#access_token=…&…` | token → `saveCredentials(username, accessToken)` (`Anilist.kt:352`), token JSON → `track_token_2` (`Anilist.kt:365`) |
| Authorisation code with PKCE | MyAnimeList | tap row | `MyAnimeListApi.authUrl()` with `code_challenge` (= verifier, `getPkceChallengeCode()`), `response_type=code` | `aniyomi://myanimelist-auth?code=…` | `saveCredentials(username, accessToken)` (`MyAnimeList.kt:256`) |
| Authorisation code | Shikimori | tap row | `ShikimoriApi.authUrl()`, `redirect_uri=aniyomi://shikimori-auth` | `…?code=…` | `saveCredentials(user, accessToken)` (`Shikimori.kt:228`) |
| Authorisation code | Bangumi | tap row | `BangumiApi.authUrl()`, `redirect_uri=aniyomi://bangumi-auth` | `…?code=…` | `saveCredentials(username, accessToken)` (`Bangumi.kt:198`) |
| Authorisation code | Simkl | tap row | `SimklApi.authUrl()`, `redirect_uri=aniyomi://simkl-auth` | `…?code=…` | `saveCredentials(user, accessToken)` (`Simkl.kt:127`) |
| Credential form | Kitsu | tap row, email and password dialog | none | none | `saveCredentials(username, userId)` (`Kitsu.kt:230`) |
| Credential form | MangaUpdates | tap row, username and password dialog | none | none | `saveCredentials(uid, sessionToken)` (`MangaUpdates.kt:119`) |
| No login | Komga, Kavita, Suwayomi, Jellyfin | tap row (enhanced) | none | none | constant `"user"` / `"pass"` (`kavita/Kavita.kt:90`, `komga/Komga.kt:88`, `suwayomi/Suwayomi.kt:81`, `jellyfin/Jellyfin.kt:79`) |

Observed quirks:
- The implicit-grant token is parsed from the fragment with the regex `(?:access_token=)(.*?)(?:&)` (`TrackLoginActivity.kt:20`). A fragment that ends with the token (no trailing `&`) does not match and the user is logged out (defect D18).
- Redirect routing switches on `data.host` (`TrackLoginActivity.kt:10-16`). An unknown host does nothing and leaves the loading screen up (defect D17).
- A redirect without `code` (for example the user denied access) calls `logout()` for that tracker (`TrackLoginActivity.kt:53-56`). A denied login therefore also clears any previous login. Proposal: on denial, keep the previous state and show "Login cancelled".
- No `state` parameter is sent or checked by any flow (CSRF protection is absent in the source).
- No timeout: a browser login can stay pending forever.
- MyAnimeList's PKCE challenge equals the verifier (plain method, no `code_challenge_method` in the URL). This matches the server default. Keep plain unless the server requires S256 (verify against the MAL documentation before implementing).

## 2. Requirements for Kurobayomi

1. Login by browser for the five OAuth trackers, without an embedded WebView (the WebKit component is not required for this).
2. The redirect must return to the running app, or start it.
3. Pending login state per tracker, with a timeout and cancel.
4. `state` parameter generated per request and checked on return (fixes R8 and the missing CSRF check).
5. Credentials and tokens in the OS keyring, never in plain preferences (`p1-rust-design.md` section 5).
6. Logout clears credentials and the token JSON and publishes the logged-out state.
7. Credential forms (Kitsu, MangaUpdates) with the same pending and error behaviour as in `p1-ui-parity.md` section 7.1.

## 3. Redirect URI decision

The source uses `aniyomi://<tracker>-auth` as the redirect URI. The tracker developer apps (client ids in areas 28 to 33) are registered with those URIs.

Options:

| Option | How | Pros | Cons |
|---|---|---|---|
| A. Keep `aniyomi://` | register the scheme `aniyomi` with the desktop handler too | no change to tracker apps; the existing client ids work | a Linux desktop might also have Aniyomi installed on Android (not a conflict on Linux); a scheme name that does not match the app name |
| B. Own scheme `kurobayomi://` | new tracker client ids with that redirect URI | clean naming | requires registering new apps with five services; the user must own those apps |
| C. Loopback `http://127.0.0.1:<port>/callback` | a short-lived local HTTP listener | no desktop registration; works in every browser | not all trackers accept `http` redirects or a random port; needs a fixed port per tracker |

**Recommendation (Proposal):** option A for now, because it keeps the trackers working with the existing registrations. The `.desktop` file declares `MimeType=x-scheme-handler/aniyomi;` and the app accepts `aniyomi://` URIs only for the five `*-auth` hosts. Option B is a later task if the user wants a separate identity.

**Open question for the user:** may the Linux port reuse the `aniyomi://` redirect with the existing client ids? If the user wants its own identity, option B needs the user's own developer apps for each tracker.

## 4. Linux mechanism

```
user taps row ──▶ oauth::begin(tracker) ──▶ PendingLogin { tracker, state, verifier?, started_at }
                                        ──▶ Opener::open(authorise_url)        // xdg-open via gio
browser ──▶ aniyomi://anilist-auth#...   (desktop handler launches or signals the running app)
app `open` signal (gio::Application, HANDLES_OPEN) ──▶ oauth::handle_redirect(uri)
handle_redirect:
  1. parse host → tracker id; look up PendingLogin for that tracker (none → ignore, toast "Unexpected login response")
  2. check state (when the server echoes it); mismatch → error, keep old credentials
  3. parse code (query) or access_token (fragment)
  4. no code/token → "Login cancelled", clear pending, keep old credentials
  5. tracker.login(code or token)   // token exchange in areas 28-33
  6. success → toast "Logged in", clear pending, publish login state
```

Components:
- `gio::Application` with `ApplicationFlags::HANDLES_OPEN` (single instance by default; a second launch from the browser is forwarded to the running window through D-Bus).
- `.desktop` file (area 53): `MimeType=x-scheme-handler/aniyomi;`, `Exec=kurobayomi %u`.
- `kuro-platform::Opener::open(url)` → `gio::AppInfo::launch_default_for_uri`.
- `oauth::begin` returns the URL; `oauth::handle_redirect` is pure and unit-testable.

Pending state (Rust):

```rust
pub struct PendingLogin {
    pub tracker: TrackerId,
    pub state: String,          // 32 random bytes, base64url
    pub verifier: Option<String>, // MyAnimeList only
    pub started_at: std::time::Instant,
}
```

Timeout: 5 minutes (Proposal). After the timeout the pending state is cleared and the banner in settings disappears (`p1-ui-parity.md` section 7.2). Late redirects after the timeout are rejected with "Login expired, try again".

## 5. Redirect parsing rules

| Input | Result |
|---|---|
| `aniyomi://anilist-auth#access_token=abc&token_type=Bearer&expires_in=3600` | token `abc` |
| `aniyomi://anilist-auth#access_token=abc` (no `&`) | token `abc` (fixes D18) |
| `aniyomi://anilist-auth#error=access_denied` | cancelled |
| `aniyomi://bangumi-auth?code=xyz&state=…` | code `xyz`, state checked |
| `aniyomi://bangumi-auth?error=access_denied` | cancelled |
| `aniyomi://unknown-auth?code=…` | "Unexpected login response" (fixes D17) |
| `aniyomi://myanimelist-auth` (no query) | cancelled |
| `http://…` or other scheme | ignored |

Parsing uses the `url` crate (`Url::parse`, `query_pairs`, `fragment`). The fragment is parsed as a query string (`form_urlencoded`).

## 6. Token storage

- Access token → `SecretStore` account `<tracker_id>.password` (keeps `is_logged_in` semantics, `p1-behavior.md` section 2.2).
- Full OAuth object (for trackers that refresh) → account `<tracker_id>.token` as JSON. Matches `track_token_<id>` in the source.
- Username → account `<tracker_id>.username`.
- Writes are ordered: token, then username, then publish logged-in. A crash after the token and before the username leaves "logged out" at next start (correct).
- Logout: delete all three accounts (source writes empty strings; deleting is cleaner and equivalent for `is_logged_in`).

## 7. Logout

- Source: `logout()` writes empty username and password and clears the auth-expired flag (`BaseTracker.kt:27-30`, `TrackPreferences.kt:29-33`). The token JSON `track_token_<id>` is deleted in `logout` by AniList (`anilist/Anilist.kt:360`), Simkl (`simkl/Simkl.kt:147`), Bangumi (`bangumi/Bangumi.kt:218`), MyAnimeList (`myanimelist/MyAnimeList.kt:264`) and Shikimori (`shikimori/Shikimori.kt:248`). Kitsu's `logout` does not delete a token (`kitsu/Kitsu.kt:233-236`).
- Kurobayomi: one `logout` path deletes username, password and token, and clears auth-expired. Publishes `false`. Local bindings are kept (logout does not unbind; source behaviour, `p1-behavior.md` section 2.3).

## 8. Auth expiry

- MyAnimeList sets the auth-expired flag when the server refuses the token (`MyAnimeList.kt:273`). The flag is read by `MyAnimeList.kt:269`.
- Proposal: any tracker that gets HTTP 401 from `update` or `refresh` sets the flag, shows a single toast "<tracker> login expired", and the settings row shows a warning dot. Login clears the flag.

## 9. Enhanced and credential-form flows

- Credential forms: a modal dialog with username and password (or email for Kitsu), single OK button, `Loading…` while the request runs (`p1-ui-parity.md` section 7.1). No redirect.
- Enhanced trackers: no login. The row action is a no-op (`loginNoop()`), and logout is available. Their "login" state is set by the enhanced tracker's own code (area 33).

## 10. Security

- Secrets are never printed to logs. `tracing` fields for tokens are redacted (`***`).
- The redirect URL is logged at debug level with the query and fragment removed.
- The `state` value is checked before the code is used (CSRF).
- The fallback secret file (`p1-rust-design.md` section 5.1) is `0600`.
- Tokens are not included in backups (private keys are skipped, `p1-data-and-config.md` section 2). Users re-login after restoring a backup (test TT-27-402).

## 11. UI

- Row: as `p1-ui-parity.md` section 2.
- Browser waiting banner: "Waiting for <tracker> in your browser…" with Cancel.
- Errors: toasts. Success: toast "Logged in" (`MR.strings.login_success`).

## 12. Tests

See `p1-edge-cases-and-tests.md`: TT-27-D17, TT-27-D18, R8, TT-27-402, and the redirect parsing table above (add one test per row).

## 13. Open questions

1. Reuse `aniyomi://` with the existing tracker client ids, or register Kurobayomi's own (section 3)?
2. Should a denied login keep the previous login (Proposal) or clear it (source)?
3. Is a 5-minute browser login timeout acceptable?
