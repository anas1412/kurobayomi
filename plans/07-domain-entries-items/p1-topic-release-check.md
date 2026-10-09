# Area 07 — topic: application release check (`GetApplicationRelease`) and its test

In scope only because of `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt` (141 lines, 4 tests). The production classes (`domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`, `domain/src/main/java/tachiyomi/domain/release/model/Release.kt`, `domain/src/main/java/tachiyomi/domain/release/service/ReleaseService.kt`) belong to area:09-domain-source-track-download-release-misc, the HTTP implementation (`data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt`, `GithubRelease.kt`) to area:06-data-repos-history-track-source-misc, the Android update installer and notifier (`app/src/main/java/eu/kanade/tachiyomi/data/updater/`) to area:10-app-domain-layer and the updater (no plans folder yet). This file gives the exact logic so the test can be ported without those files.

## 1. Types

```
Release(version: String, info: String, releaseLink: String, downloadLink: String)

GetApplicationRelease.Arguments(isPreview: Boolean, commitCount: Int, versionName: String,
                                repository: String, forceCheck: Boolean = false)

sealed Result: NewUpdate(release: Release) | NoNewUpdate | OsTooOld     // OsTooOld is never returned (its only producer in AppUpdateChecker is commented out)

ReleaseService: suspend fun latest(arguments: Arguments): Release?
```

`lastChecked`: `preferenceStore.getLong(Preference.appStateKey("last_app_check"), 0)` = key `__APP_STATE_last_app_check`, default 0 (epoch ms), created lazily.

## 2. `await(arguments)`

```
now = Instant.now()
if !arguments.forceCheck && now.isBefore(Instant.ofEpochMilli(lastChecked.get()).plus(3, DAYS)):
    return NoNewUpdate                                   // service NOT called
release = service.latest(arguments) ?: return NoNewUpdate
lastChecked.set(now.toEpochMilli())                       // only after a successful lookup
return if isNewVersion(arguments.isPreview, arguments.commitCount, arguments.versionName, release.version)
       NewUpdate(release) else NoNewUpdate
```

Throttle: 3 days; a failing network call (exception from `service.latest`) propagates, does not update `lastChecked`, so the next call retries immediately. `ReleaseServiceImpl.latest` returns `null` when no asset matches (`getDownloadLink` null).

## 3. `isNewVersion`

```
newVersion = versionTag.replace("[^\d.]".toRegex(), "")        // strip everything but digits and dots, e.g. "v2.0.0" -> "2.0.0", "r2000" -> "2000"
if isPreview:                                                   // tags like "r1234"
    return newVersion.toInt() > commitCount                     // NumberFormatException if empty or contains a dot
else:                                                           // tags like "v0.1.2"
    oldVersion = versionName.replace("[^\d.]".toRegex(), "")
    newSemVer = newVersion.split(".").map { it.toInt() }        // NumberFormatException on empty segments ("1..2", trailing dot, "")
    oldSemVer = oldVersion.split(".").map { it.toInt() }
    oldSemVer.mapIndexed { index, i -> if (newSemVer[index] > i) return true }   // IndexOutOfBounds when newSemVer is shorter than oldSemVer
    return false
```

Defects (OBSERVED, all reachable with real tags): (1) the loop returns `true` on the first component where new > old WITHOUT checking that higher-order components are equal or lower: new `1.0.5` vs old `1.1.0` returns true (component 2: 5 > 0). (2) new `1.2` vs old `1.2.0`: `newSemVer[2]` throws `IndexOutOfBoundsException`. (3) new `2.0.0.1` vs old `2.0.0` returns false (extra components of the new version are ignored). (4) a tag with no digits (`"nightly"`) throws `NumberFormatException`. (5) A higher-order component that is LOWER does not stop later components from winning (defect 1 again). Proposal: compare as tuples (`Vec<u64>` lexicographic with missing components treated as 0), treat parse failure as "no new version" (never panic), keep the preview rule `tag_number > commit_count`.

## 4. The four upstream tests

Common setup (`@BeforeEach`): `preferenceStore.getLong(any(), any())` returns a mocked `Preference<Long>`; `releaseService` mock. Each test sets `preference.get()` and `preference.set(any())` stubs, stubs `releaseService.latest(any())` returning `Release(version, "info", "http://example.com/release_link", "http://example.com/release_link.apk")`, calls `await(Arguments(isPreview, commitCount, versionName, repository = "test"))`.

| # | Name | `preference.get()` | Release version | Arguments | Expected |
|---|---|---|---|---|---|
| 1 | When has update but is preview expect new update | 0 | `r2000` | isPreview = true, commitCount = 1000, versionName = "" | `NewUpdate` with the same release (2000 > 1000) |
| 2 | When has update expect new update | 0 | `v2.0.0` | isPreview = false, commitCount = 0, versionName = `v1.0.0` | `NewUpdate` (2 > 1 at index 0) |
| 3 | When has no update expect no new update | 0 | `v1.0.0` | isPreview = false, commitCount = 0, versionName = `v2.0.0` | `NoNewUpdate` (1 > 2 false, 0 > 0 false, 0 > 0 false) |
| 4 | When now is before three days expect no new update | `Instant.now().toEpochMilli()` | `v1.0.0` | isPreview = false, versionName = `v2.0.0` | `NoNewUpdate` and `releaseService.latest` called 0 times (`coVerify(exactly = 0)`) |

Rust port: `trait ReleaseService { async fn latest(&self, args: &Arguments) -> Result<Option<Release>> }` mocked by a struct with a call counter (`AtomicUsize`); preference replaced by an injected `LastChecked` trait or a `Cell<i64>`; time injected as `now_ms: i64` argument to keep tests deterministic.

Additional tests for the fixed compare (Proposal): `("1.0.5","1.1.0") -> false`, `("1.2","1.2.0") -> false`, `("1.2.1","1.2") -> true`, `("2","1.9.9") -> true`, `("nightly", "1.0.0") -> false` and does not panic, `forceCheck = true` ignores the throttle, service `None` leaves `lastChecked` untouched, service error leaves `lastChecked` untouched.

## 5. Kurobayomi decision

Android Aniyomi downloads and installs an APK. Kurobayomi is distributed by the user's own channel (AUR package or GitHub release tarball); in-place installation is out of scope and dangerous (package manager owns files). Proposal (P2): keep the throttle and a fixed compare, point `repository` at a constant naming Kurobayomi's own GitHub repository (value chosen by the maintainer; constant `KUROBAYOMI_REPO` defined in one place, value set by the maintainer (open question OQ-7 in 00-overview.md)), use the same endpoint `GET https://api.github.com/repos/{repository}/releases/latest` with `User-Agent: Kurobayomi/<version>` (GitHub requires a user agent), parse `tag_name`, `body`, `html_url`, and DROP the asset/ABI selection (`BUILD_TYPES = arm64-v8a, armeabi-v7a, x86_64, x86`, `Build.SUPPORTED_ABIS[0]`) and the Markdown mention-linking of `@user` in the release notes (regex `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))`, case-insensitive) which exists for the Android notification renderer; the UI shows "New version X available" with an "Open release page" button using `gtk::UriLauncher`/`xdg-open`. Make the check opt-in (default off) because the user did not ask for network access outside sources; the setting lives in area:02-preferences-system. The Android update notification, installer and "OsTooOld" result are P3 drops (logic: T-07a-036; drops: T-07a-037 installer/notification/ABI asset selection and T-07a-042 `OsTooOld`).
