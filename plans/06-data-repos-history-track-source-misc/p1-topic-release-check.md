# Topic: application release check (GitHub "latest release")

Scope files: `data/src/main/java/tachiyomi/data/release/GithubRelease.kt` (29 LOC), `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt` (61 LOC). Read alongside: `domain/src/main/java/tachiyomi/domain/release/{interactor/GetApplicationRelease,model/Release,service/ReleaseService}.kt`, `domain/src/test/java/tachiyomi/domain/release/interactor/GetApplicationReleaseTest.kt`, `app/src/main/java/eu/kanade/tachiyomi/data/updater/{AppUpdateChecker,AppUpdateNotifier}.kt`, `app/src/main/java/eu/kanade/tachiyomi/ui/more/NewUpdateScreen.kt`, `app/src/main/java/eu/kanade/presentation/more/NewUpdateScreen.kt`, `MainActivity.kt:369-395`, `AboutScreen.kt:200-235`.

The interactor `GetApplicationRelease` (throttle, version comparison) is area:09-domain-source-track-download-release-misc; the download/install job (`AppUpdateDownloadJob`) is area:35-data-misc-updater-notification-export. This topic covers the network service and the decisions that tie the three together.

## 1. OBSERVED data flow

```text
startup (MainActivity.CheckForUpdates, only when BuildConfig.UPDATER_ENABLED)   or   About -> "Check for updates" (forceCheck = true)
  AppUpdateChecker.checkForUpdate(forceCheck)
    GetApplicationRelease.await(Arguments(isPreview, commitCount, versionName, repository, forceCheck))
      if !forceCheck and now < lastChecked + 3 days: return NoNewUpdate          # pref __APP_STATE_last_app_check
      release = ReleaseService.latest(args) ?: return NoNewUpdate                # ReleaseServiceImpl
      lastChecked = now
      return if isNewVersion(isPreview, commitCount, versionName, release.version) NewUpdate(release) else NoNewUpdate
    on NewUpdate: AppUpdateNotifier.promptUpdate(release)                         # notification
  startup: if NewUpdate -> push NewUpdateScreen(version, changelog, releaseLink, downloadLink)
  about: NewUpdate -> same screen; NoNewUpdate -> toast "No new updates available"; exceptions -> toast(e.message)
```

`ReleaseServiceImpl.latest` (`RSI:18-35`): GET `https://api.github.com/repos/${arguments.repository}/releases/latest` (shared client, `awaitSuccess`, parse `GithubRelease`), `getDownloadLink(release) ?: return null`, then `Release(version = tag_name, info = body with mentions linkified, releaseLink = html_url, downloadLink)`.

`GithubRelease` (`GithubRelease.kt:10-19`): `tag_name` (-> `version`), `body` (-> `info`), `html_url` (-> `releaseLink`), `assets: List<GitHubAsset>`; `GitHubAsset(name, browser_download_url -> downloadLink)`. A release with no notes (GitHub serialises that as `"body": null`) fails to decode because `info` is a non-null `String` without a default (the app `Json` only tolerates missing/null for nullable properties) — a latent upstream bug; Rust uses `#[serde(default)]` on `body` and `assets`.

## 2. Mention linkification

Pattern (`RSI:58-59`, Kotlin `Regex`, `IGNORE_CASE`): `\B@([a-z0-9](?:-(?=[a-z0-9])|[a-z0-9]){0,38}(?<=[a-z0-9]))`. Replacement for every match: `[${mention.value}](https://github.com/${mention.value.substring(1)})` — the link text keeps the `@`.

Reading the pattern:
* `\B` before `@`: the preceding character must not be a word character (or start of text). `foo@bar` is not a mention; `(@bar)` and ` @bar` are.
* first character alphanumeric; then 0 to 38 units, each being either a hyphen that is *followed* by an alphanumeric (lookahead) or an alphanumeric; the match must end on an alphanumeric (lookbehind). So hyphens are single, never leading/trailing, and total length <= 39 characters (1 + 38 units, a hyphen counts as a unit).
* No trailing word boundary: `@` followed by 45 alphanumerics matches only the first 39 and leaves the rest as plain text.
* Underscores are not allowed in usernames and end the match (`@user_name` links `@user`).
Rust: the `regex` crate has no lookaround, so use `fancy-regex` (one small dependency, supports lookahead/lookbehind and `\B`): `fancy_regex::RegexBuilder::new(PATTERN).case_insensitive(true).build()`. Replacement through `Regex::replace_all` with a closure producing `format!("[{m}](https://github.com/{u})", m = &caps[0], u = &caps[0][1..])`. Test vectors in `p1-edge-cases-and-tests.md` section 5.

## 3. Download link selection

Kotlin (`RSI:37-43`): `BUILD_TYPES = ["arm64-v8a", "armeabi-v7a", "x86_64", "x86"]`; `map = assets.associate { asset -> BUILD_TYPES.find { "-$it" in asset.name } to asset.downloadLink }` (first matching type in list order; `null` key when none matches; later assets overwrite earlier ones with the same key); `map[Build.SUPPORTED_ABIS[0]] ?: map[null]`. Note the `x86` entry also matches the `-x86_64` substring only if `x86_64` were not found first — list order makes `x86_64` win, correct.

Linux replacement (Proposal, T-06a-072): `PLATFORM_TAGS = ["x86_64", "aarch64"]` (`BUILD_TYPES` equivalent), same `associate` + lookup by `std::env::consts::ARCH` then the untagged asset (key `None`). The asset naming convention is chosen by the developer's release workflow: `kurobayomi-<version>-<arch>.tar.zst` (or `.AppImage`), i.e. the arch tag preceded by `-`. If no asset matches, `latest` returns `None` => "no new update" (same as Kotlin), which is also what happens for a release that only has source archives.

## 4. What the Linux app does with a new release

| Aniyomi step | Kurobayomi decision |
|---|---|
| Notification with `action_download` and `whats_new` actions | `Notifier` trait (area:53/area:01): title `update_check_notification_update_available` ("New version available!"), body = version; click -> open the in-app update dialog; no inline actions required |
| `NewUpdateScreen`: heading, version, changelog (checksum footer stripped by the regex `---(\R|.)*Checksums(\R|.)*` -> Rust `(?s)---.*Checksums.*`), buttons `update_check_confirm` "Download", `update_check_open` "Open on GitHub", `action_not_now` | Dialog in the GTK app with the same content. "Download" opens `downloadLink` in the browser (`Opener`) |
| `AppUpdateDownloadJob` (in-app APK download with progress notification, then `ACTION_VIEW` package installer) | **Drop** (T-06a-073). Replacement: the user installs through the package manager / AppImage replacement. User loses one-tap in-app install; on Arch the AUR/pacman package is the supported channel and a release check is informational |
| Preview channel (`isPreviewBuildType`, repo `aniyomiorg/aniyomi-preview`, tags `rNNNN`, commit count comparison) | **Drop**: Kurobayomi has no preview build type. Only the stable path (`vX.Y.Z` tags) remains; `GetApplicationRelease` keeps the `is_preview` argument only if area:09 keeps it |
| `UPDATER_ENABLED` BuildConfig flag | Cargo feature `updater` (default on in release builds, off for distro packages that disable self-update) — area:01 `app_info` |
| Repository names | One build-time constant `UPDATE_REPOSITORY` (an `"owner/name"` string supplied by the developer's build, area:01); when empty the checker is disabled |
| `OsTooOld` result and `update_check_eol` toast | Not reachable on Linux; keep the enum variant out of the Rust code (task notes in area:09) |

The three-day throttle and the "last check" preference stay (`last_app_check`, ms). The startup check runs once per launch in the background after the first frame; failures are logged and silent; the manual check shows `update_check_no_new_updates` / the error message (`e.message`) in a toast-equivalent (area:36 `Toast`).

## 5. Version comparison (area:09 owns the code, behaviour recorded here because `Release.version` feeds it)

Kotlin stable branch (`GetApplicationRelease.kt`):

```kotlin
val newVersion = versionTag.replace("[^\\d.]".toRegex(), "")        // "v0.18.3" -> "0.18.3"
val oldVersion = versionName.replace("[^\\d.]".toRegex(), "")
val newSemVer = newVersion.split(".").map { it.toInt() }
val oldSemVer = oldVersion.split(".").map { it.toInt() }
oldSemVer.mapIndexed { index, i -> if (newSemVer[index] > i) { return true } }
false
```

Defects: (1) returns true as soon as ANY component of the new version is greater than the same component of the old version, regardless of more significant components — new `0.17.9` vs old `0.18.2` yields true (component 2: 9 > 2) although it is older; (2) `newSemVer[index]` throws `IndexOutOfBoundsException` when the new tag has fewer components than the installed version name (installed `0.18.2.1`, tag `v0.18.3`); (3) `toInt()` throws `NumberFormatException` for an empty component (`"1..2"` or an all-letter tag -> `""` -> `[""]`). The Aniyomi version name has four components (`0.18.2.1`), so (2) is realistic.
Proposal for Rust (a sibling task in area:09, `GetApplicationRelease`): compare component-wise from the most significant, padding the shorter list with zeros, parsing each component as `u64` (invalid -> treated as "no new update", logged). The upstream test expectations remain true (listed in `p1-edge-cases-and-tests.md` section 5).

## 6. Rust module

```rust
// kuro_core::release
pub struct Release { pub version: String, pub info: String, pub release_link: String, pub download_link: String }
pub struct ReleaseClient { http: Arc<HttpClient /* area:12 */> }
impl ReleaseClient {
    pub async fn latest(&self, repository: &str) -> Result<Option<Release>, ReleaseError>;   // None = no suitable asset
}
fn linkify_mentions(body: &str) -> String;
fn pick_download_link(assets: &[GithubAsset], arch: &str) -> Option<String>;
```

Endpoint details: `GET https://api.github.com/repos/{owner}/{name}/releases/latest`; headers: the app's default User-Agent (GitHub requires one) and `Accept: application/vnd.github+json` (Proposal; Kotlin sends none, the API still answers JSON); no authentication; rate limit 60/hour/IP unauthenticated — the 3-day throttle plus a user-initiated check cannot approach it. Non-2xx -> `ReleaseError::Http(code)` (the manual check shows its message).
