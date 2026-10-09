# Topic: OAuth token lifecycle for Shikimori and Bangumi

Scope: how a token is created, stored, restored, refreshed, failed and deleted, for both trackers. Behaviour citations are in p1-behavior.md (sections 2.4 to 2.7 and 3.4 to 3.7). Storage names are in p1-data-and-config.md section 4 and 10. The Rust types are in p1-rust-design.md section 5.

Contents: 1 states; 2 timelines; 3 refresh under concurrency; 4 failure table; 5 persistence and restore; 6 logout; 7 security notes; 8 test hooks.

---

## 1. States

Each tracker has one of these states. The state is derived from the secret store and the in-memory slot. There is no separate state field in the Kotlin.

```
            login ok                  token expired,           refresh ok
  LoggedOut ----------> Fresh ----------------------> Expiring ----------------> Fresh
      ^                  |  ^                           |
      |                  |  | restore at start          | refresh rejected (400/401)
      |     logout       |  | (token + username +       v
      +------------------+  |  password present)     AuthExpired
      ^                     |                           |
      |                     |                           | login again (new code)
      +---------------------+---------------------------+--> Fresh
                 logout from any state
```

Definitions:
- **LoggedOut**: username or password missing, or no token in the secret store. `is_logged_in()` is false (`BaseTracker.kt:32-34`). Authenticated calls return `NotAuthenticated` before I/O.
- **Fresh**: token present and `now <= created_at + expires_in - 3600`. Requests use the access token directly.
- **Expiring**: token present and `now > created_at + expires_in - 3600`. The next authenticated call refreshes first.
- **AuthExpired**: refresh rejected with 400 or 401, or the token is expired and has no refresh token, or a fresh token got a 401. The credentials are kept (so the account is still shown), the auth-expired flag is set (`pref_tracker_auth_expired_N`), and every authenticated call returns `AuthExpired` until the user logs in again.

Kotlin today: AuthExpired does not exist. A rejected refresh is ignored and the request is sent with the old token (D1). The flag exists in `TrackPreferences.kt:24-27` but nothing in this area sets it.

## 2. Timelines

### 2.1 Shikimori (access token 86400 s, refresh threshold 82800 s)

| Time since `created_at` | State | Event |
|---|---|---|
| 0 s | Fresh | login; token stored |
| 0 to 82800 s | Fresh | requests use the token |
| 82801 s | Expiring | next request refreshes (strict `>`, `SMOAuth.kt:21`) |
| 82801 s | Fresh | refresh ok: new access token, new refresh token, both stored |
| each day | repeats | refresh about once per day while the app is used |

### 2.2 Bangumi (access token 604800 s, refresh threshold 601200 s)

| Time since `created_at` | State | Event |
|---|---|---|
| 0 s | Fresh | login; token stored |
| 0 to 601200 s | Fresh | requests use the token |
| 601201 s (6 days 23 h) | Expiring | next request refreshes (`BGMOAuth.kt:25`) |
| 601201 s | Fresh | refresh ok, stored |
| each week | repeats | refresh about once per week |

A user who does not open the app for more than 7 days hits Expiring with no chance to refresh before the server expires the token. The next start then gets a 401 on the first request, which the new code reports as AuthExpired instead of a generic HTTP error.

## 3. Refresh under concurrency

Kotlin: `oauth` is a plain `var` with no lock (`ShikimoriInterceptor.kt:18`, `BangumiInterceptor.kt:18`). Two requests that both see Expiring each send a refresh with the same refresh token. Doorkeeper rotates refresh tokens (p1-topic-remote-api-verification.md S-V9, S-V15), so the second refresh can fail, and the second response then decides the stored token.

Rust design (p1-rust-design.md 5.2):
1. A single `tokio::sync::Mutex<Option<StoredToken>>` guards the slot.
2. `bearer()` takes the lock before reading the token.
3. If the token is Expiring, the lock is held across the refresh request.
4. Other callers wait on the lock. When they get it, the token is already Fresh, so they return it without a second refresh.
5. The refresh result is stored before the lock is released.

Sequence with two callers A and B:

```
A: lock ------------------------------------- unlock
A:   read token (Expiring) -> refresh request -> store new -> set slot
B:   wait for lock ..........................  lock -> read token (Fresh) -> return -> unlock
```

Edge: a caller that is cancelled while waiting drops its future; it does not hold the lock, so there is no deadlock (RC-01).

Edge: logout while a refresh is in flight. The refresh result must not be stored after logout. The rule (RC-02, TS-S-15): before storing, check `base.is_logged_in()`; if false, discard the new token and return `NotAuthenticated`.

Edge: two refresh attempts in quick succession, if the first failed with a transport error. The second attempt is allowed (the token is still Expiring). The OQ-4 guard (at most one refresh per 60 seconds) applies only to the case `expires_in <= 3600`, where every call would otherwise refresh.

## 4. Failure table

| Failure | Kotlin | Kurobayomi | Stored state after |
|---|---|---|---|
| Token endpoint 400 on login (bad or used code) | caught, `logout()`, nothing shown | error returned, toast, nothing stored | LoggedOut |
| Token endpoint 200 but `whoami` fails | caught, `logout()` | error returned; token not stored | LoggedOut |
| Refresh 400 or 401 | ignored; request sent with old token; server 401 → `HTTP error 401` | AuthExpired, flag set | AuthExpired (credentials kept) |
| Refresh 5xx | ignored; request sent with old token | `Http(5xx)`; token kept; next call retries | Expiring |
| Refresh transport error (no network) | exception from `chain.proceed` propagates | `Offline` / `Http`; token kept | Expiring |
| Refresh 200 but body does not decode | exception propagates from interceptor | `Decode`; token kept | Expiring |
| Refresh token absent and expired | NPE (Shikimori on every request) | AuthExpired | AuthExpired |
| Token secret unreadable at start | restore returns null → LoggedOut | restore returns none → LoggedOut; username and password still present → shown as logged in but calls fail with NotAuthenticated | see 5.2 |
| Token write fails after refresh | exception on `saveToken` (not caught) | warning logged; in-memory token kept for this session | Fresh in memory, lost on restart |
| Revoked on the provider's site | 401 on a fresh token → `HTTP error 401` | 401 on a fresh token → AuthExpired | AuthExpired |

## 5. Persistence and restore

### 5.1 What is stored

| Secret (service `kurobayomi.track`) | Account | Content | Written | Deleted |
|---|---|---|---|---|
| token JSON | `<id>.token` | `StoredToken` JSON (p1-data-and-config.md 1) | after login success and after each successful refresh | logout |
| username | `<id>.username` | Shikimori numeric id as string; Bangumi login name | after login success | logout (set to empty) |
| password | `<id>.password` | access token (duplicate, kept for `is_logged_in`) | after login success and each refresh | logout (set to empty) |

Kotlin stores the same three values in preferences (`TrackPreferences.kt:14-35`). Only the location changes.

Kotlin order on login: the token is saved first (`newAuth`, `ShikimoriInterceptor.kt:45-48`), then `whoami` runs, then username and password are saved (`Shikimori.kt:226-228`). Rust order: the token stays in memory while `whoami` runs; after `whoami` succeeds, the token JSON, username and password are written in that order (EC-S-10: no token is stored when `whoami` fails). Login is complete only when all three are present. A crash between writes is detected at start as LoggedOut, because `is_logged_in()` needs username and password and the token is required for calls. Proposal: write the three values in one batch and publish login state after all three succeed (p1-rust-design.md 5.5).

### 5.2 Restore at start

1. Read username and password. If either is empty: LoggedOut.
2. Read the token JSON. If absent or not decodable: LoggedOut; the username and password are cleared so the UI matches (proposal; Kotlin keeps them, which shows a logged-in account whose calls fail).
3. Otherwise: Fresh or Expiring, decided by the clock, not by the stored state.

### 5.3 Why the token is restored in the constructor

Kotlin restores lazily (`ShikimoriInterceptor.kt:18`, `private var oauth: SMOAuth? = shikimori.restoreToken()`). Restoring eagerly in Rust keeps `is_logged_in()` and the first request consistent, and makes the restore failure visible at start. There is no behavioural difference for a working install.

## 6. Logout

Order (p1-rust-design.md 5.4):
1. Clear username and password, clear the auth-expired flag, publish LoggedOut to the watch channel. The UI updates immediately.
2. Delete the token secret and clear the in-memory slot.

Kotlin writes `"null"` into the token preference after deleting it (`Shikimori.kt:248-249`, `Bangumi.kt:218-219`). Kurobayomi deletes the secret and never writes a null value (D10). A test checks that no secret with the token account holds the text `null` after logout (TS-S-15).

Logout while a request is in flight: the in-flight request may still complete with the old token. That is acceptable (the server still accepts it until it expires). The refresh-store rule in section 3 prevents a refreshed token from being stored after logout.

## 7. Security notes

- Tokens are never placed in URLs. Bangumi v0 rejects query-string tokens (B-V19), and Kurobayomi never builds such URLs.
- Tokens are never logged. Test TS-X-12 checks the log output of a failing refresh.
- The access token is stored twice (token JSON and password). This is the Kotlin's design, kept for `is_logged_in()`. Both entries live in the same secret service item namespace and are deleted together on logout. Proposal for a later change: derive the password from the token JSON and store only the JSON.
- The OAuth client secret is required by both token endpoints (both servers use confidential clients). A desktop app cannot keep it secret. This is accepted (both trackers' own apps do the same) but it is why each deployment should use its own registration (D-OPEN-1), not the Aniyomi one.
- The redirect can carry a code from any local process that opens the URI. The code is one-use and short-lived (B-V15, S-V8), so the exposure is small. Area 27 owns the redirect handler and its checks (`plans/27-tracking-core/p1-topic-tracker-oauth.md`, section 5).

## 8. Test hooks

| Hook | Used by | Purpose |
|---|---|---|
| `Clock` trait with `now_unix()` | TS-S-02, TS-B-04, TS-X-02 | exact expiry boundaries without sleeping |
| `SecretStore` in-memory implementation | all session tests | no keyring in CI |
| `AuthExpiredFlag` trait | TS-S-05, TS-S-07, TS-X-06 | assert the flag is set without the preference store |
| wiremock token endpoint with call counter | TS-S-04 | proves single flight (one refresh for ten callers) |
| wiremock slow response | TS-S-15 | holds the lock to exercise logout during refresh |
| tracing capture | TS-X-12 | proves no token text is logged |
