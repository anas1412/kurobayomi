# Topic: app updater (Aniyomi observed, Kurobayomi decided)

Scope files: `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt`, `.../AppUpdateDownloadJob.kt`, `.../AppUpdateNotifier.kt`. Related files outside scope: `domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`, `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt`.

## 1. What Aniyomi does, in one paragraph

Aniyomi asks the GitHub releases API for the latest release of its own repository, at most once every three days in the background (or whenever the user presses "Check for updates"). If the tag is newer than the installed version, it posts a notification and opens a changelog page. "Download" fetches the APK for the device's primary ABI into the external cache with progress notices, then hands the file to the system package installer. Nothing is verified beyond what the system installer checks.

## 2. State machine (Kurobayomi)

```
          +---------------------------+
          |          Idle             |
          +-------------+-------------+
   check(auto or manual)|
                        v
          +---------------------------+   throttled (auto)   +---------+
          |        Checking           |--------------------->|  Idle   |
          +------+------+-------------+                      +---------+
  network/HTTP err |      | release not newer  | newer + asset
          v        |      v                    v
     +--------+    |  +--------+       +-----------------+
     | Idle + |    |  | Idle   |       |  Prompted       |  notice "app-update.progress" (Download, What's new)
     | toast  |    |  +--------+       +--------+--------+  + update screen (manual or startup)
     +--------+    |                            | Download
                   |                            v
                   |                   +-----------------+   cancel -> Idle
                   |                   |  Downloading    |-----------------------+
                   |                   +--------+--------+                       |
                   |                  error     | done                           |
                   |                   v        v                                |
                   |             +---------+  +------------------+               |
                   |             | Failed  |  | ReadyToReveal    |  notice "app-update.install"
                   |             | (Retry) |  | (Open folder)    |               |
                   |             +---------+  +------------------+               |
                   |                                                             |
                   +---------------------------------------------------------------+
```

States are kept in memory. Only `last_update_check_ms` is persisted (app state key, data file section 2). Downloads are not resumed after an app restart; the `.part` file is removed on the next start if older than 24 h (T-35a-050, PROPOSAL).

## 3. Decisions

### 3.1 Version rule

Aniyomi's rule (`GetApplicationRelease.kt:48-76`) strips every non-digit and non-dot character, then compares components position by position and returns `true` as soon as one new component is greater than the old one at the same index. It never returns `false` early, so it offers older releases (for example old `2.5.0` against new `2.0.1`). It also throws on tags with fewer components or no digits.

Kurobayomi uses `semver` ordering after removing a leading `v`. Tags that are not semver are rejected with a clear error. Prerelease tags sort below the release (`1.0.0-rc.1 < 1.0.0`). Regression tests are listed in p1-edge-cases-and-tests.md section 5.1.

### 3.2 Throttle

Aniyomi: 3 days, stored in app state, set when a release is found, bypassed by a forced check (`GetApplicationRelease.kt:22-33`). Kept as is, because it prevents GitHub rate-limit problems (unauthenticated limit) and avoids nagging. The timestamp is set only when the release lookup returned a release, so a platform with no asset is re-checked on each start (parity; the cost is one request per start, which is acceptable at GitHub's limits).

### 3.3 Asset choice

Aniyomi: the asset containing `-<primary ABI>`, else the asset with no ABI token (`ReleaseServiceImpl.kt:37-43`). Kurobayomi: the same two tiers, with Linux names (`kurobayomi-<version>-<arch>.AppImage` preferred, `.tar.zst` second). Architecture comes from `platform_arch()` in kuro-platform (`x86_64` or `aarch64`).

Live evidence (read-only `gh api` on 2026-10-08): the Aniyomi latest release lists five assets, one per ABI plus one universal. Kurobayomi's release process is not defined yet; the tests use a synthetic fixture (p1-edge-cases-and-tests.md section 6).

### 3.4 Download and verification

Aniyomi: no checksum check; the system installer checks the signature. Kurobayomi: the release notes include a SHA-256 table in the live Aniyomi format (`| Variant | SHA-256 |`); the same table format is accepted for Kurobayomi releases, plus the `sha256sum` line format. When a hash exists it is checked before the file is revealed. When none exists, the install notice says "not verified" (new string `update.install_unverified`, PROPOSAL).

### 3.5 Install on Linux

Android installs an APK in place. Linux cannot do that for distro packages or for an AppImage that is running from a read-only mount. Decision:
- Package-managed installs (detected in kuro-platform `install_kind()`, T-35a-055): no download. The About row says "Update with your package manager" and "What's new" opens the release page.
- AppImage or a user-extracted tarball: download to `cache/updates`, reveal the file, the user replaces the old file themselves. In-place self-replacement is out of scope (T-35a-056, Dropped) because it needs careful permission and signature handling that this plan does not design.

### 3.6 Background work

Aniyomi uses WorkManager with `CONNECTED` and `REPLACE` (`AppUpdateDownloadJob.kt:133-154`). Kurobayomi uses a single-slot tokio task with `NetworkStatus::is_online` and `online_events` (no periodic work at all; the check happens on app start and on manual request). Foreground service semantics are not needed on a desktop process.

### 3.7 Notification ids

Aniyomi uses ids 1 (progress and prompt), 2 (install), 3 (error) (`Notifications.kt:89-91`). Kurobayomi keeps the three roles as named slots (`app-update.progress`, `app-update.install`, `app-update.error`) so the progress notice is replaced by the next state.

## 4. Failure handling (summary)

| Failure | Aniyomi | Kurobayomi |
|---|---|---|
| Network error on auto check | logged, nothing shown | logged, nothing shown |
| Network error on manual check | toast `e.message` | toast with the error text |
| No asset for platform | `NoNewUpdate` (misleading toast on manual) | `NoNewUpdate` plus toast "No build for this platform" |
| Download fails | error notice (ID 3), progress left behind | error notice with Retry, progress withdrawn |
| Checksum mismatch | not checked | error notice, file deleted |
| Cancel | progress withdrawn | progress withdrawn, file deleted |
| Second download while running | replaces | replaces (cancel and await the old task) |

## 5. Security notes

- The download URL comes from the API response and from a notification payload. Kurobayomi accepts only `https`, and only the GitHub hosts `github.com` and `objects.githubusercontent.com` (design section 6.1). A payload from another process of the same user cannot redirect the download.
- The downloaded file is never executed by the app. Reveal opens the folder; "Open" would launch the file with the default application and is not offered for executables.
- The release notes are rendered as text and a Markdown subset with links opened through `Opener::open_url` (https or http only).

## 6. Test list (summary)

Version rule (regression for E1-E6), asset selection (tiers, AppImage preference, first match), throttle boundaries (`now >= last + 3 days`), the four ported `GetApplicationReleaseTest` cases, download integration (success, 404, cancel, checksum, replace, offline). Full list: p1-edge-cases-and-tests.md sections 4-5.

## 7. Open questions (defaults chosen)

- Release channel: stable only (default). A preview channel (`aniyomi-preview` style) is Dropped (T-35a-061).
- Repository: no default; the user enters `owner/name` in settings (`update.repo`). Default empty means updates are disabled.
- Auto-check on start: on (default). A setting `update.auto_check` turns it off; manual check still works.
