# Topic: application release check (GitHub latest release)

Scope files: `domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`, `.../model/Release.kt`, `.../service/ReleaseService.kt`. Read: `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt`, `data/src/main/java/tachiyomi/data/release/GithubRelease.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateNotifier.kt`, `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:99-141,203-238`, `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt`, test `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`. Endpoint catalog: `plans/catalogs/network-endpoints-01.md` (release endpoint section) and `plans/catalogs/network-endpoints-02.md`.

## 1. OBSERVED flow, step by step

1. Triggers: (a) app start, `MainActivity.CheckForUpdates` (`MainActivity.kt:371-392`) when `updaterEnabled` (`BuildConfig.UPDATER_ENABLED`, set from `Config.enableUpdater` in `app/build.gradle.kts:28`; false in FOSS-style builds) with `forceCheck = false`; (b) Settings > About > "Check for updates" (`MR.strings.check_for_updates`; row only when `updaterEnabled`) with `forceCheck = true`, a progress ring while running, ignores a second tap while running.
2. Gate: skip when `now < last_app_check + 3 days` unless forced.
3. Request `GET https://api.github.com/repos/<repo>/releases/latest`. Non-2xx throws (rate limit 403 included).
4. Parse DTO; choose the download asset; no matching asset (`getDownloadLink == null`) returns null -> `NoNewUpdate` WITHOUT writing `last_app_check`, so the check repeats on every start until a release with an asset appears.
5. Write `last_app_check = now`.
6. Compare versions (defective upstream, section 3).
7. `NewUpdate`: start-up path pushes `NewUpdateScreen`; `AppUpdateChecker` also posts the notification (title `update_check_notification_update_available`, text = version, two actions).
8. About path toasts for `NoNewUpdate` (`update_check_no_new_updates`) / `OsTooOld` (`update_check_eol`, never produced) and, on exception, `toast(e.message)` + log.
9. "What's new" row in About (`MR.strings.whats_new`, only in non-debug builds) opens `RELEASE_URL` = `https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG`.

## 2. Wire details

Request has no `Accept` header and no token. GitHub's unauthenticated limit is 60 requests per hour per IP; a 3-day gate makes that irrelevant except for shared NAT. Response fields used: `tag_name`, `body`, `html_url`, `assets[].name`, `assets[].browser_download_url`. Assets are matched by name substrings (`-arm64-v8a`, `-armeabi-v7a`, `-x86_64`, `-x86`); the universal APK has none of them.

Mention linkifier regex (exact, Kotlin raw string): `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))`, `RegexOption.IGNORE_CASE`; replacement `"[${mention.value}](https://github.com/${mention.value.substring(1)})"` where `mention.value` includes the `@`.

Rust scanner (no regex lookaround needed):

```rust
pub fn linkify_mentions(text: &str) -> String {
    let b: Vec<char> = text.chars().collect();
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';     // Java \w (ASCII); \B uses it
    let alnum = |c: char| c.is_ascii_alphanumeric();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == '@' && (i == 0 || !is_word(b[i - 1])) && i + 1 < b.len() && alnum(b[i + 1]) {
            // longest run matching alnum ( '-'? alnum ){0,38}, total length <= 39, must end in alnum
            let mut j = i + 2;               // exclusive end of the accepted name (first alnum already counted)
            let mut k = i + 2;               // scan position
            let mut len = 1;
            while k < b.len() && len < 39 {
                if alnum(b[k]) { k += 1; len += 1; j = k; }
                else if b[k] == '-' && len + 1 < 39 && k + 1 < b.len() && alnum(b[k + 1]) { k += 2; len += 2; j = k; }
                else { break; }
            }
            let name: String = b[i + 1..j].iter().collect();
            out.push_str(&format!("[@{name}](https://github.com/{name})"));
            i = j;
        } else { out.push(b[i]); i += 1; }
    }
    out
}
```
Details that must match the regex: the first char after `@` is alnum (counts toward the 39 limit: `[a-z0-9]` + up to 38 more); a `-` is consumed only when followed by alnum (so `@a-` yields `@a`, and `@a--b` yields `@a`); the name never ends with `-`; maximum total length 39; the `\B` before `@` means the previous char must not be a word character (letter, digit, underscore), so `me@example` is not a mention and `(@bob)` is. Java's `\w` is ASCII by default; Rust `is_ascii_alphanumeric()` matches. Case: `IGNORE_CASE` means uppercase letters are valid.

Vectors:
| Input | Output |
|---|---|
| `Thanks @bob!` | `Thanks [@bob](https://github.com/bob)!` |
| `@a-b-c` | `[@a-b-c](https://github.com/a-b-c)` |
| `@a--b` | `[@a](https://github.com/a)--b` |
| `@a-` | `[@a](https://github.com/a)-` |
| `mail me@host` | unchanged |
| `x_@bob` | unchanged (`_` is a word char) |
| `@` followed by 45 letters | first 39 letters linked, remaining 6 stay plain |
| `@Bob` | `[@Bob](https://github.com/Bob)` |
| `**@bob**` | `**[@bob](https://github.com/bob)**` |

## 3. Version comparison: upstream defects and the Rust replacement

OBSERVED code (`GetApplicationRelease.isNewVersion`):
```
newVersion = versionTag.replace("[^\\d.]".toRegex(), "")
preview: newVersion.toInt() > commitCount
stable:  oldVersion = versionName.replace(same, "")
         newSemVer = newVersion.split(".").map { it.toInt() }
         oldSemVer = oldVersion.split(".").map { it.toInt() }
         oldSemVer.mapIndexed { index, i -> if (newSemVer[index] > i) return true }
         false
```
Behaviors (test with old = `0.17.1`):
| New tag | Upstream result | Correct |
|---|---|---|
| `v0.17.2` | true | true |
| `v0.18.0` | true | true |
| `v1.0.0` | true | true |
| `v0.16.9` | true (9 > 1 at index 2) | false |
| `v0.17.1` | false | false |
| `v0.17` | throws `IndexOutOfBoundsException` (index 2) | false (0.17.0 < 0.17.1) |
| `v0.17.1.1` | false (extra component ignored) | true |
| `v0.17.1-beta` | `-beta` stripped to `0.17.1`; false | false (prerelease of the same version is lower) |
| `v1..2` | `NumberFormatException` (empty string) | invalid tag, ignored with a log |
| ``(empty) | `NumberFormatException` | ignored |

Rust: `parse_tag(tag)`: trim, strip one leading `v`/`V`/`r`, split off a `-prerelease` / `+build` per semver, pad missing minor/patch with zeros (`1.2` -> `1.2.0`), reject anything else -> `None`. `is_newer(candidate, current)` = `candidate > current` using semver precedence (`1.0.0-beta.1 < 1.0.0`). A tag that fails to parse never raises an update. `current` comes from `CARGO_PKG_VERSION` via `AppInfo` (area:01).

Preview channel: Aniyomi uses a separate repo (`aniyomiorg/aniyomi-preview`) with tags `r<commit count>` compared with the build's commit count. Kurobayomi drops the preview channel (single solo-developer repo); if added later it is a second repository constant plus an integer compare. The user loses nothing.

## 4. Port of the upstream tests (also in `p1-edge-cases-and-tests.md`)

Mapping of `GetApplicationReleaseTest.kt` (mock `ReleaseService`, mock preference `get() = 0` unless stated):
1. `When has update but is preview expect new update`: release tag `r2000`, commitCount 1000 -> NewUpdate. (Rust: dropped with the preview channel; keep as an integer-compare unit test only if preview is added.)
2. `When has update expect new update`: tag `v2.0.0`, current `v1.0.0` -> NewUpdate with the same release object.
3. `When has no update expect no new update`: tag `v1.0.0`, current `v2.0.0` -> `NoNewUpdate`.
4. `When now is before three days expect no new update`: `last_app_check = now` -> `NoNewUpdate` and `latest()` never called (`coVerify(exactly = 0)`).
New tests: forced check ignores the gate; gate boundary (`last = now - 3 days` exactly is NOT before -> check runs; `now - 3d + 1ms` skip); `latest()` error does not write `last_app_check`; `None` from `latest()` does not write; the defect rows of the table in section 3; `linkify_mentions` vectors.

## 5. Linux decisions

| Aniyomi behavior | Kurobayomi |
|---|---|
| Check on start + manual button | Same; setting `updates.check_for_app_updates: bool = true` (Proposal), hidden and forced false when `FLATPAK_ID` is set or the binary lives under `/usr` (package-managed install detection by `std::env::current_exe()`), because the package manager owns updates |
| Download the APK and launch the Android installer (`NotificationReceiver` download + `PackageInstaller`) | Dropped. The dialog offers "Open on GitHub" (`gtk::UriLauncher` on `releaseLink`) and "Download" which opens `downloadLink` in the browser. The app never replaces its own binary. User loses one-click self-update; gains safety (no unsigned binary swap) |
| Notification with actions Download / What's new | Desktop notification via the platform `Notifier` (area:01) with the same two actions; shown only when the window is not focused, otherwise the in-app `NewUpdateScreen` page is pushed |
| Asset selection by ABI | `pick_asset(assets)`: choose the first asset whose lowercase name contains the Rust target pair, in priority: `linux` and `x86_64`/`aarch64` per `std::env::consts::ARCH`; prefer `.AppImage`, then `.tar.zst`/`.tar.gz`, then `.pkg.tar.zst`; else the release page link as `downloadLink`. If no asset matches return `Release` with `downloadLink = releaseLink` (Aniyomi returns null and shows nothing; for a source-only distribution a notice with just the release page is more useful) |
| Markdown changelog (RichText) | `pulldown-cmark` events rendered into a `gtk::TextBuffer` with tags (bold, italic, code, link, list bullets) or a plain `gtk::Label` with `use_markup` after converting a safe subset; links open through `UriLauncher` |

## 6. Message keys

`check_for_updates` ("Check for updates"), `update_check_no_new_updates`, `update_check_notification_update_available`, `update_check_confirm`, `update_check_open`, `action_not_now`, `whats_new`, `update_check_eol` (dropped), `update_check_notification_download_complete` ("Tap to install update", dropped with the installer). New: `update_check_failed` ("Could not check for updates: {reason}") replaces toasting a raw exception message.

## 7. Tasks

T-09a-024, T-09a-025, T-09a-026, T-09a-043 in `p1-tasks.md`.
