# area:28-tracker-anilist — topic deep dive: authentication and token lifecycle

Repo root `/home/blackbox/aniyomi-src`. Full detail of each step is in `p1-behavior.md` §4; this file is the state machine and the decision table.

## 1. Stored state (Kotlin today)

| Store | Key | Value | Written by | Read by |
|---|---|---|---|---|
| preference | `pref_mangasync_username_2` | Viewer id as decimal string | `login` via `saveCredentials` (`ANI:352`) | `isLoggedIn`, `update` / `bind` / `refresh` / `delete` via `getUsername().toInt()` |
| preference | `pref_mangasync_password_2` | access token | `login` via `saveCredentials` (`ANI:352`) | `isLoggedIn`; `AnilistInterceptor` constructor (`ANI:58`) |
| preference | `track_token_2` | JSON `ALOAuth` or the text `null` | `saveOAuth` (`ANI:364-366`) | `loadOAuth` (`ANI:368-374`) |
| preference | `anilist_score_type` | POINT_* string | `login` (`ANI:351`) | score list, index, display |
| preference | `pref_tracker_auth_expired_2` | boolean | `setCredentials` resets to false (`TrackPreferences.kt:32`) | not read for AniList |

`isLoggedIn` = username non-empty AND password non-empty (`BaseTracker.kt:32-34`). Two independent stores must agree; a mismatch (e.g. username set, token missing) reads as logged in and then fails with "Not authenticated" on the first call.

## 2. States

| State | Entry condition (Kotlin) | Behaviour |
|---|---|---|
| LoggedOut | username or password empty | `isLoggedIn` false; calls throw `Not authenticated with Anilist` (`INT:26-28`) only if called directly (framework filters by `isLoggedIn` first) |
| LoggedIn | `login` success (username + token saved) | calls allowed |
| Stale | token pref set, `track_token_2` missing/`null`/corrupt | `oauth` null → `oauth!!` NPE (`INT:33`) (E-04) |
| Expired (dead in practice) | `now > expires` | `logout()` + `IOException("Token expired")` (`INT:33-36`); never reached because of the arithmetic (E-03) |
| LoggedIn-but-revoked | AniList rejects token (HTTP 401) | `HttpException(401)` → "HTTP error 401" toast; state stays LoggedIn (no handling) (E-49) |

## 3. Transitions (Kotlin, as observed)

```
LoggedOut --open browser, redirect with token--> [login(token)]
   [login]: createOAuth -> setAuth (memory + track_token_2) -> Viewer query
       ok   -> score format saved, username/token saved           => LoggedIn
       fail -> logout() (silent)                                  => LoggedOut   (E-01)
LoggedIn --logout (settings dialog)--> LoggedOut (token row set to "null", E-05)
LoggedIn --redirect with no token--> logout() then returnToSettings  => LoggedOut  (E-02 fragility)
LoggedIn --app restart--> interceptor lazy-loads oauth from track_token_2 (multiplied expiry)
```

## 4. Proposed state machine (Rust)

```
            +-----------+   redirect(token)    +-----------+
            | LoggedOut |--------------------->| Verifying |
            +-----------+                      +-----------+
                  ^                             |        |
                  |        viewer error         |        | viewer ok
                  +-----------------------------+        v
                  |                                +-----------+
                  |        logout                  | LoggedIn  |
                  +--------------------------------+-----------+
                  |                                  |      |
                  |     HTTP 401 / expires_at passed |      | restart, token valid
                  |                                  v      |
                  |                             +---------+ |
                  +-----------------------------|  Expired|-+
                     user re-logs in            +---------+
```

Rules:
- `Verifying` is visible in the settings row ("Logging in…", p1-ui-parity §2). Exits: `LoggedIn` or `LoggedOut` with a toast.
- `Expired` keeps `viewer_id` so the row can say "Session expired, log in again"; token is cleared. A redirect with a new token moves to `Verifying`.
- Restart: load `expires_at` from settings; if past → `Expired` without a network call; if future → `LoggedIn` without a network call (the Viewer query is run only on login).
- Any call returning 401 moves `LoggedIn → Expired` and cancels nothing else; in-flight calls that started before the transition are allowed to finish but their results must not write track rows (race R-4).

## 5. Decision table

| Question | Kotlin answer | Rust answer | Reason |
|---|---|---|---|
| Where is the token kept? | plaintext preference | OS keyring; file 0600 fallback with warning | secrets should not sit in app prefs (T-28a-046) |
| What lifetime is assumed? | 365 days, in ms, mis-applied (E-03) | 365 days from login (or `expires_in` from redirect if present), in seconds | correct units; real expiry check |
| What happens on 401? | HTTP error toast only | Expired state + re-login prompt | the user can act on it |
| Does login errors surface? | silent logout (E-01) | toast with the reason | failures must be visible |
| Is the score format user-editable? | no (from server) | no (from server; refreshed on each login) | matches Kotlin; avoids a second source of truth |
| Is redirect parsing strict about delimiters? | requires trailing `&` (E-02) | parses fragment as form data | robust |
| Is the viewer id a string or integer? | string, parsed with `toInt()` at each call (E-06) | i64 | removes a failure mode |
| Is logout deleting track rows? | no (E-48) | no | local history kept; user removes per entry |

## 6. Tests (cross-reference)

- State transitions: edge tests E-01, E-02, E-03, E-04, E-05, E-49; unit tests 9–16 in `p1-edge-cases-and-tests.md` §6.
- Restart behaviour: a stored expiry in the past yields `Expired` with zero HTTP calls (task T-28a-009 acceptance).
- Race R-4: start a call, log out, deliver the response; assert no track row is written (integration with area:27 repository mock).

## 7. Open decisions for the user (not blocking)

1. Whether to keep the AniList token in the keyring (recommended) or in the settings file for simplicity. Default in the plan: keyring with fallback.
2. Whether search should work without login (depends on a live check; default in the plan: keep login required).
3. Whether a 401 should auto-open the browser for re-login (default in the plan: show a row button, do not open the browser automatically).
