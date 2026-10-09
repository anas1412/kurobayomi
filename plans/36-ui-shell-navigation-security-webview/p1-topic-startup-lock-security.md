# Topic — Startup sequence, app lock and secure-screen decisions

Deep dive for area 36. Covers: MainActivity startup order, SecureActivityDelegate lock timing, UnlockActivity, FLAG_SECURE. Full behaviour: `p1-behavior.md` §1, §11. Rust types: `p1-rust-design.md` §2.5, §2.6, §2.7.

## 1. Startup order (Kotlin, observed)

```
process start
  └─ SecureActivityDelegate.onApplicationStart()            (called by app lifecycle; caller outside scope)
       requireUnlock stays true (initial)                  -> first window will lock if lock enabled
MainActivity.onCreate (isLaunch = savedInstanceState == null)
  ├─ installSplashScreen()  [launch only]
  ├─ BaseActivity: attachBaseContext(prepareTabletUiContext); onCreate applies theme
  ├─ Migrator.awaitAndRelease()   (blocks; didMigration)
  ├─ !isTaskRoot -> finish()
  ├─ setComposeContent
  │    └─ Navigator(HomeScreen)
  │         LaunchedEffect(navigator): handleIntentAction(intent) [launch only]; incognito := false
  │         CheckForUpdates(); ShowOnboarding()
  ├─ splash keep-on-screen: elapsed <= 500 || (!ready && elapsed <= 5000)
  └─ chapterCache.clear() if auto_clear_chapter_cache [launch only]
MainActivity.onResume (every foreground)
  └─ SecureActivityDelegate.setAppLock(): lock if enabled && requireUnlock; else nothing
```

Order matters: the lock check (`onResume`) runs after the first composition, so content may be composed before the lock overlay appears. Kurobayomi must build the lock overlay before the content page is shown (design §7 pitfall 1).

## 2. Lock timing as a state machine

State: `require_unlock` (in memory), `authenticating` (in memory), `last_closed` (persisted, `__APP_STATE_last_app_closed`), config `enabled`, `lock_after` in minutes (-1 never, 0 always, n minutes).

### 2.1 On application stop (`onApplicationStopped`)
| enabled | authenticating | require_unlock | lock_after | Action |
|---|---|---|---|---|
| no | — | — | — | nothing |
| yes | yes | — | — | nothing |
| yes | no | yes | — | nothing (still locked; stays locked on next start) |
| yes | no | no | > 0 | write `last_closed = now_ms` |
| yes | no | no | 0 or -1 | nothing |

### 2.2 On application start (`onApplicationStart`)
| enabled | authenticating | require_unlock (before) | lock_after | Result for require_unlock | Note |
|---|---|---|---|---|---|
| no | — | — | — | unchanged | timestamp not deleted (return happens first) |
| yes | yes | — | — | unchanged | timestamp deleted |
| yes | no | yes | — | stays yes | timestamp deleted |
| yes | no | no | -1 | no | timestamp deleted |
| yes | no | no | 0 | yes | timestamp deleted |
| yes | no | no | n > 0 | yes iff `last_closed + n*60000 <= now` | timestamp deleted |

Implementation note from the Kotlin: `lastClosedPref.delete()` is inside the function after the `if` block, and the earlier `return` for `!useAuthenticator` skips it. Rust `on_app_started` reproduces this with `if cfg.enabled { kv.delete(...) }`.

### 2.3 Unlock
- `unlock()`: `require_unlock = false`. Called only by a successful authentication.
- Failure or cancel: Kotlin exits the app (`finishAffinity`). Kurobayomi: overlay stays, app stays (intentional change, behaviour §11.8).

### 2.4 On resume (`setAppLock`)
- If lock not enabled: return.
- If the authenticator is available: if `require_unlock` is false, return; else show the overlay.
- If not available: Kotlin silently sets the preference to false. Kurobayomi: if the passphrase file is missing, show a setup prompt and keep the lock off, with a visible notice (edge E12).

## 3. Authenticator decision (Kurobayomi)

```
available = passphrase_file_exists()                    # the lock is only meaningful if a secret exists
fprint    = fprintd_available() && user_has_enrolled()
if fprint: try fprintd verify (60 s timeout); on Success -> unlock; on NoMatch -> count++; after 5 -> passphrase
passphrase entry: Argon2id check; success -> unlock; failure -> count++, message "Incorrect passphrase"
cancel -> close overlay, keep locked (window still shows overlay; no content)
```
Attempts: no lockout in Kotlin (Android enforces its own). Proposal: 5 wrong passphrases -> 30 s delay before the next try. Stored count is in memory only (resets on restart; the lock timestamp rule still applies).

## 4. Secure screen (FLAG_SECURE) decision

Kotlin: `secure_screen_v2` (ALWAYS | INCOGNITO | NEVER, default INCOGNITO) sets FLAG_SECURE on the window; effective when ALWAYS or (INCOGNITO and incognito on). Linux: no API. Decision: drop (task T-36a-035). What the user loses: protection from screenshots and screen recording by other apps, and from the Android recents thumbnail. Kurobayomi mitigates the recents thumbnail only partly (blank content on lock, pitfall 1); a screenshot tool can still capture the window.

## 5. Incognito interplay
- Incognito is not a lock. It resets on every launch (`__APP_STATE_incognito_mode` := false).
- Incognito-off pops source pages (behaviour §1.2) so no source screen stays open after leaving incognito.
- The lock does not depend on incognito.

## 6. Data at rest
- Passphrase: Argon2id PHC only (never plaintext). File `lock.json` mode 0600.
- The lock timestamp is an integer in the app-state store (area 02); not sensitive.
- The lock does not encrypt the database (out of scope; flagged to area 02 as a separate decision).

## 7. Test checklist (maps to T-36a-032..035)
1. Table test for §2.1 and §2.2 (both functions).
2. Restart with lock_after = 0 always locks.
3. Restart with lock_after = 5 within 5 minutes does not lock; after 5 minutes locks.
4. Crash (no stop event) with lock_after = 5 locks.
5. Cancel on the overlay keeps the app open and locked.
6. Overlay shown before any content is visible (screenshot test).
7. Passphrase file corrupt: unlock refused, setup prompt offered.
