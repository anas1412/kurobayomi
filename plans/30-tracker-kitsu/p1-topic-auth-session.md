# Topic: Kitsu session state machine (token, credentials, refresh)

Deep dive for `p1-behavior.md` sections 4 and 5. Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/Kitsu.kt`, `KitsuInterceptor.kt`, `KitsuApi.kt`, `dto/KitsuOAuth.kt`; plus `app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`, and `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsTrackingScreen.kt`.

## 1. Two independent pieces of state

Aniyomi keeps two pieces of state that can disagree:

| State | Storage | Written by | Read by |
|---|---|---|---|
| Token (`KitsuOAuth`) | pref `track_token_3` (JSON), plus in-memory `KitsuInterceptor.oauth` | `Kitsu.saveToken` via `newAuth` | `KitsuInterceptor.intercept`, `Kitsu.restoreToken` |
| Credentials (username, "password" = user id) | prefs `pref_mangasync_username_3`, `pref_mangasync_password_3` | `BaseTracker.saveCredentials`, `setCredentials` | `isLoggedIn`, `getUserId` |

`isLoggedIn` looks only at the credentials. Requests use only the token. The two diverge in these cases:
- token present, credentials empty: possible after a login that failed at step 3 (users call) and before the UI's `checkLogin` runs `logout()`. The UI always runs `logout()` on failure, so this lasts only until the dialog closes. It is not persistent unless the app is killed between the two.
- credentials present, token missing or `null`: only if the token pref was cleared by hand or by a restore from backup that did not carry the token. Requests then fail with `Not authenticated with Kitsu` while the UI shows "logged in".

Rust (Proposal): one `Session` record holds `{ token: Option<OAuth>, user_id: Option<String>, username: Option<String> }` in one place, and `is_logged_in` = `token.is_some() && user_id.is_some()`. Writes go through one function so the two cannot diverge. T-30a-007.

## 2. States

```
            +-----------+   login OK    +-----------+
            | LoggedOut | ------------> | LoggedIn  |
            +-----------+               +-----------+
                 ^  |  login fails          |  |  token expired
                 |  +-- (rollback) ---+     |  |
                 |                    |     |  v
                 |                    |   +-----------+  refresh OK   +-----------+
                 |                    |   | Expired   | ------------> | LoggedIn  |
                 |                    |   +-----------+               +-----------+
                 |                    |     |        |
                 |                    |     |  4xx   |  network error
                 |                    |     v        v
                 |                    |  +--------+  (stale token kept;
                 |  logout            |  | Reauth |   requests fail 401)
                 +--------------------+--+--------+
```

State definitions (Aniyomi):

| State | Token | Credentials | `isLoggedIn` | Requests |
|---|---|---|---|---|
| LoggedOut | null / `"null"` / `""` | `""`, `""` | false | `Not authenticated with Kitsu` (no network) |
| LoggedIn | valid, not expired | email, user id | true | bearer injected |
| Expired | expired (now > created + expires - 3600) | unchanged | true | refresh first, then bearer |
| Reauth (proposal) | refresh rejected with 4xx | unchanged | true (UI shows "log in again") | `ReauthRequired` (no network) |
| PartialLogin (transient) | valid | empty | false | bearer injected (until logout) |

Aniyomi has no explicit `Reauth` state. An expired token whose refresh fails (any status) continues as `Expired`, and each request is sent with the stale bearer and fails with 401 (`HTTP error 401`). The `pref_tracker_auth_expired_3` flag exists but is never set to true by Kitsu code.

## 3. Transitions (exact)

### 3.1 LoggedOut → LoggedIn
1. `POST /api/oauth/token` (password grant). Success → token T.
2. Store T (memory + `track_token_3`).
3. `GET /api/edge/users?filter[self]=true` with bearer T. Success → user id U.
4. Store username = email, password slot = U (`pref_mangasync_password_3`), auth-expired = false.

Failure at 1: no state change (token unchanged, credentials unchanged). The UI calls `logout()`, which clears everything (already empty, so a no-op in effect).
Failure at 3: token T is stored, credentials are not. The UI calls `logout()` → LoggedOut.

### 3.2 LoggedIn → Expired
Time passes; `now > created_at + expires_in - 3600`. Detected on the next request (`intercept`), not by a timer.

### 3.3 Expired → LoggedIn (refresh)
`POST /api/oauth/token` with `grant_type=refresh_token`, `refresh_token=R`. On 2xx: the whole token is replaced with the response. Aniyomi does not merge; if the response lacks `refresh_token`, the stored token has `refreshToken = null` and the next expiry is fatal (NPE, E-02/E-04 in the edge-case file).

### 3.4 Expired → Stale (network error or 5xx)
Response closed, stale token used, request sent. Server returns 401 → `HTTP error 401` surfaces to the caller. The track update fails and the caller (area 27) handles retry.

### 3.5 Any → LoggedOut (logout)
1. Credentials cleared (`setCredentials("", "")`, auth-expired false).
2. Token set to null in memory; `track_token_3` = `"null"`.
No server call.

## 4. Concurrency rules

Aniyomi: none. `KitsuInterceptor.oauth` is a plain `var`. Multiple requests can observe Expired at the same time and each run a refresh. If the server rotates refresh tokens (not visible in source), the second refresh fails and the stale token is used.

Rust (Proposal, T-30a-027):
- `AuthSession.token: tokio::sync::Mutex<Option<OAuth>>`.
- `bearer()` holds the mutex across the refresh (single flight). Pseudocode in `p1-rust-design.md` section 4.
- A generation counter increments on every `set`. A refresh result is written only if the generation is unchanged when the refresh returns (so a concurrent logout wins). Test KT-97.

## 5. Persistence and restore

- Save: every `set(Some(T))` writes JSON to the secret store, including after refresh.
- Restore: on `AuthSession::restore`, read the store once. `""`, `"null"`, and malformed JSON all become `None`. No error is shown; the user sees "logged out" and must log in again.
- Keyring first, settings table fallback (see `p1-data-and-config.md` section 7). The fallback key is `track_token_3` so a backup import (T-30a-032) can find it.

## 6. What the user sees (mapped to states)

Observed in Aniyomi: the login dialog is reached from settings (SettingsTrackingScreen.kt:183), the logout dialog from the same row, and the error texts below. The track-sheet column is a Proposal.

| State | Track sheet (Kitsu row) (Proposal) | Search | Login dialog |
|---|---|---|---|
| LoggedOut | "Add tracking" is not offered for Kitsu; row shows "Log in" action | not reachable | open from settings |
| LoggedIn | row bound/bindable | works | "Log out" in settings |
| Expired | same as LoggedIn (refresh is invisible) | refresh then works | same |
| Reauth (proposal) | row shows a warning "Kitsu session expired. Log in again." | blocked with same message | login dialog opened from the warning |
| PartialLogin | not visible (transient) | — | error shown; logout runs |

Error texts shown (Aniyomi, English): `Not authenticated with Kitsu`, `HTTP error 401`, `Logged in`, `You are now logged out`. Proposal for Reauth: `Kitsu session expired. Log in again.` (new key, flagged).

## 7. Test list (session only)

KT-20 expiry boundary; KT-21 token restore variants; KT-22 login form order; KT-23 login success; KT-24 rollback on no user; KT-25 wrong password; KT-26 refresh happy path; KT-27 refresh keeps refresh token; KT-28 4xx → Reauth; KT-29 network error keeps stale; KT-30 expired without refresh token; KT-31 single flight; KT-32 logout; KT-33 no token; KT-97 logout wins over refresh. All in `p1-edge-cases-and-tests.md` section 3.3 and 5.

## 8. Open questions (cannot be answered from source)

1. Does Kitsu rotate the refresh token on each refresh? Answer needed before enabling the single-flight rule as the only protection. (Test plan section 6, step 6.)
2. Does Kitsu invalidate the whole session when a refresh token is reused? Unknown.
3. Typical `expires_in` for password-grant tokens. Not a constant in source. Do not hard-code; read from the response.
