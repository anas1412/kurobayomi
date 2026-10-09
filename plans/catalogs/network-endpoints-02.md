# Network hosts and endpoints, chunk 2 of 2

Catalog chunk produced from `python3 plans/_tools/inventory.py items hosts 2 2`. It holds 38 hosts, first item `github.com`, last item `youtrack.jetbrains.com`. Chunk 1 holds the earlier hosts (alphabetically before `github.com`). Every path below is repo-relative to the Aniyomi clone at `/home/blackbox/aniyomi-src`.

How to read this file:

- OBSERVED means what the Kotlin does today. "Proposal:" means what Kurobayomi (Rust, gtk4-rs, reqwest, tokio, rusqlite, libmpv) should do.
- Several hosts here are only the "front door" of a bigger flow (AniList, Kitsu, MyAnimeList, MangaUpdates, Shikimori, Simkl, the DNS-over-HTTPS providers, the wsrv.nl image proxy, the torrent tracker list). For those, the flow is fully documented in the host's own section, including sibling hostnames that the same code path calls (for example `api.myanimelist.net`, `api.simkl.com`, `api.mangaupdates.com`, `data.simkl.in`, `api.aniskip.com`, `AWQO5J657S-dsn.algolia.net`, `api.github.com`, `api.resmush.it`). If chunk 1 also lists such a sibling host, the two sections are complementary and must agree.
- "Keep?" answers whether the Rust app keeps the host: **Keep** (feature ported), **Keep-opt** (ported but optional or off by default), **Replace** (named Linux-native or different mechanism), **Drop** (with reason and what the user loses), **Doc-only** (the host appears only in comments, docs or build files and is never contacted by the app).
- Test advice assumes `curl` and the `wiremock` crate (or `httpmock`) for unit tests. Live probes were run read-only on 2026-10-08 and are noted as "Live probe".

## 0. Summary table

| Host | Role | Contacted at runtime? | Keep? |
| --- | --- | --- | --- |
| `github.com` | repo links, release page, extension-repo "open repo" link, docs | links only (opened in browser) | Keep (links) |
| `github.githubassets.com` | image in PR template | no | Doc-only |
| `goo.gle` | Material Theme Builder credit comments | no | Doc-only |
| `graphql.anilist.co` | AniList tracker API, AniSkip id lookup, airing-time lookup | yes | Keep |
| `hosted.weblate.org` | translation platform badge | no | Doc-only |
| `imagecdn.manga.com` | example host in rate-limit KDoc | no | Doc-only |
| `img.shields.io` | README badges | no | Doc-only |
| `issuetracker.google.com` | Compose bug links in comments | no | Doc-only |
| `keepachangelog.com` | changelog format link | no | Doc-only |
| `kitsu.app` | Kitsu tracker API, OAuth, Algolia key | yes | Keep |
| `kotlinlang.org` | contributing doc link | no | Doc-only |
| `libredns.gr` | DoH provider 13 (source credit) and its `doh.libredns.gr` endpoint | yes (when selected) | Keep-opt |
| `m3.material.io` | Material 3 guideline link in comment | no | Doc-only |
| `mangaupdates.com` | MangaUpdates tracker API (`api.mangaupdates.com`) | yes | Keep |
| `material.io` | Material design layout link in KDoc | no | Doc-only |
| `mullvad.net` | DoH provider 9 (source credit) and its `dns.mullvad.net` endpoint | yes (when selected) | Keep-opt |
| `myanimelist.net` | MAL tracker OAuth plus `api.myanimelist.net` REST API | yes | Keep |
| `mysite.com` | example base URL in source-api KDoc | no | Doc-only |
| `nyaa.tracker.wf` | default BitTorrent tracker (TorrServer) | indirectly (TorrServer announces) | Keep-opt |
| `raw.githubusercontent.com` | extension repository index host and URL rewrite rules | yes (extension repo fetch) | Keep |
| `semver.org` | changelog versioning link | no | Doc-only |
| `share.camoe.cn` | default BitTorrent tracker | indirectly | Keep-opt |
| `shecan.ir` | DoH provider 12 (source credit) and its `free.shecan.ir` endpoint | yes (when selected) | Keep-opt |
| `shikimori.one` | Shikimori tracker API and OAuth | yes | Keep (domain moved, see section) |
| `shizuku.rikka.app` | Shizuku download link in an Android-only dialog | link only | Drop |
| `simkl.com` | Simkl tracker website and OAuth authorize page, plus `api.simkl.com` | yes | Keep |
| `simkl.in` | Simkl poster CDN and calendar JSON (`data.simkl.in`) | yes | Keep |
| `simpleicons.org` | icon provenance comment | no | Doc-only |
| `source.chromium.org` | header-safety list provenance comment | no | Doc-only |
| `square.github.io` | OkHttp and LeakCanary docs links | no | Doc-only |
| `stackoverflow.com` | provenance comments | no | Doc-only |
| `support.google.com` | Google device list provenance comment | no | Doc-only |
| `t.nyaatracker.com` | default BitTorrent tracker | indirectly | Keep-opt |
| `tracker.anirena.com` | default BitTorrent tracker | indirectly | Keep-opt |
| `wsrv.nl` | image-proxy "data saver" backend | yes (when selected) | Keep-opt |
| `www.contributor-covenant.org` | code of conduct attribution | no | Doc-only |
| `www.nordtheme.com` | Nord palette credit | no | Doc-only |
| `youtrack.jetbrains.com` | Kotlin compiler issue links in build files | no | Doc-only |

## 1. Shared client behaviour that every runtime row below relies on

OBSERVED, `core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkHelper.kt`:

- The shared client (`networkService.client`, used by all trackers through `BaseTracker.client` at `app/src/main/java/eu/kanade/tachiyomi/data/track/BaseTracker.kt`) has `connectTimeout(30, SECONDS)`, `readTimeout(30, SECONDS)`, `callTimeout(2, MINUTES)`, a 5 MiB on-disk HTTP cache at `<cacheDir>/network_cache`, a cookie jar, and these interceptors in order: `UncaughtExceptionInterceptor` (rethrows any non-IO exception as `IOException`), `UserAgentInterceptor` (adds `User-Agent` from preference `default_user_agent` only when the request has none; default value `Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:136.0) Gecko/20100101 Firefox/136.0`), the DoH `dns(...)` override chosen by preference `doh_provider` (default `-1` = disabled), and finally `CloudflareInterceptor`.
- Default request helper `GET/POST/PUT/DELETE` in `core/common/src/main/java/eu/kanade/tachiyomi/network/Requests.kt` attaches a default `CacheControl` of `maxAge(10, MINUTES)` and an empty header set. JSON bodies use `jsonMime = "application/json; charset=utf-8"` (`core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:22`).
- The injected `Json` is `Json { ignoreUnknownKeys = true; explicitNulls = false }` (`app/src/main/java/eu/kanade/tachiyomi/di/AppModule.kt:172-177`). `coerceInputValues` is not set, which matters for the AniSkip enum quirk below.
- Two code paths create a bare `OkHttpClient()` instead of the shared client: `AniSkipApi` and `AniChartApi` (the reSmush.it data saver uses the shared client). Those two bare clients therefore ignore DoH, the custom user agent, cookies and the cache. They use OkHttp defaults: 10 s connect, 10 s read, 10 s write, no call timeout.
- Authenticated tracker clients are `client.newBuilder().addInterceptor(<TrackerInterceptor>).build()` so they share the shared client's timeouts, DoH and cache. Exception: Kitsu and Simkl search calls use the unauthenticated `client` for the Algolia and `/search` calls (noted per host).

Proposal: one `reqwest::Client` built once (`connect_timeout(30s)`, `timeout(120s)` as the call timeout, `cookie_store(true)`, default `User-Agent` from the setting above, `hickory-dns` resolver override when a DoH provider is selected). Authenticated trackers wrap that client in a small `TrackerHttp` that injects the bearer token and per-tracker headers. The two bare-client paths (AniSkip, airing times) should use the shared client too (DoH and UA apply); this is a deliberate behaviour improvement, not a regression. Implementation is owned by area:12-network-stack.

## 2. Host sections

---

### 2.1 `github.com`

Role: project hosting. Runtime use is limited to opening links in the browser; the machine-readable GitHub endpoints live on `api.github.com` (update check) and `raw.githubusercontent.com` (extension repositories, section 2.20). `github.com` pages themselves are never fetched by the app.

OBSERVED link and URL patterns:

| URL pattern | Where | Purpose |
| --- | --- | --- |
| `https://github.com/aniyomiorg/aniyomi` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:192` | "GitHub" icon link on the About screen (opened with the system URI handler). |
| `https://github.com/$GITHUB_REPO/releases/tag/$RELEASE_TAG` (`RELEASE_URL`) | `app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt:59` | "What's new" link. `GITHUB_REPO` is `aniyomiorg/aniyomi-preview` for preview builds and `aniyomiorg/aniyomi` for stable (`AppUpdateChecker.kt:43-49`). `RELEASE_TAG` is `r${COMMIT_COUNT}` for preview and `v${VERSION_NAME}` for stable (`:51-57`). |
| `https://github.com/$user/$repo` | `app/src/main/java/eu/kanade/presentation/browse/anime/AnimeExtensionDetailsScreen.kt:81-87` and `app/src/main/java/eu/kanade/presentation/browse/manga/MangaExtensionDetailsScreen.kt:82-88` | "Open repo" action in extension details. Derived from the extension store's `indexUrl` with the regex `https://raw.githubusercontent.com/(.+?)/(.+?)/.+`; when the regex does not match, the raw `indexUrl` is opened instead. |
| `https://github.com/${mention.value.substring(1)}` | `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:34-36` | Release notes: every `@username` mention (regex `\B@([a-z0-9](?:-(?=[a-z0-9])\|[a-z0-9]){0,38}(?<=[a-z0-9]))`, case-insensitive, max 39 chars, no leading/trailing/double hyphen) is rewritten to a Markdown link to that user's profile. |
| `https://github.com/aniyomiorg/aniyomi/releases/latest`, `.../aniyomi/issues`, `.../actions/workflows/build_push.yml`, `.../graphs/contributors`, `https://github.com/aniyomiorg/aniyomi-preview/releases`, `https://github.com/aniyomiorg/aniyomi-website`, `https://github.com/aniyomiorg/aniyomi-mpv-lib` | `README.md:13,15,21,22,48,52,53,59`, `.github/ISSUE_TEMPLATE/report_issue.yml:100`, `.github/ISSUE_TEMPLATE/request_feature.yml:35` | Documentation and issue templates. Not runtime. |
| `https://github.com/aniyomiorg/aniyomi/pull/<n>`, `.../commit/<sha>`, `https://github.com/<user>` | `CHANGELOG.md` (all entries, for example `CHANGELOG.md:23-83`) | Changelog contributor and PR references. Not runtime. |
| `https://github.com/abdallahmehiz/mpvKt` | license headers of `app/src/main/java/eu/kanade/tachiyomi/ui/player/*.kt`, `app/src/main/java/eu/kanade/presentation/player/components/*.kt` | Provenance of the player code. Not runtime. |
| `https://github.com/saikou-app/saikou/blob/main/app/src/main/java/ani/saikou/others/AniSkip.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt:21` | Credit for the AniSkip client. |
| `https://github.com/MakD/AFinity/blob/master/app/src/main/java/com/makd/afinity/ui/player/components/TrickplayPreview.kt` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/controls/components/ThumbnailPreview.kt:43` | Credit for the seek-thumbnail preview. |
| `https://github.com/NeoUtils/Highlight`, `https://github.com/Qawaz/compose-code-editor` | `app/src/main/java/eu/kanade/presentation/more/settings/screen/player/editor/codeeditor/Highlight.kt:25`, `SyntaxHighlight.kt:15,26` | Credit for the mpv.conf editor syntax highlighter. |
| `https://github.com/mpv-player/mpv/issues/14651` | `app/src/main/java/eu/kanade/tachiyomi/ui/player/AniyomiMPVView.kt:151` | Workaround reference. |
| `https://github.com/square/okhttp/blob/ef5d0c83f7bbd3a0c0534e7ca23cbc4ee7550f3b/okhttp-dnsoverhttps/src/test/java/okhttp3/dnsoverhttps/DohProviders.java` | `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt:9` | Provenance of the DoH provider table (section 2.12, 2.17, 2.23). |
| `https://github.com/aniyomiorg/aniyomi/blob/main/LICENSE`, `.../app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt`, `.../app/build.gradle.kts`, `.../app/src/standard/google-services.json` | `CONTRIBUTING.md:38,45,47,49,50` | Fork guidance. Not runtime. |
| `https://github.com/mozilla/diversity` | `CODE_OF_CONDUCT.md:122` | Attribution. |
| `https://github.com/gradle/gradle/...` | `gradlew:60,63,88` | Gradle wrapper comments. |
| `https://github.com/coil-kt/coil/blob/1674d3516f061aeacbe749a435b1924f9648fd41/coil-core/src/androidMain/kotlin/coil3/util/hardwareBitmaps.kt` | `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:613` | Provenance of the hardware-bitmap blocklist. |
| `https://github.com/icerockdev/moko-resources/issues/337` | `core/common/src/main/java/tachiyomi/core/common/i18n/Localize.kt:28` | Workaround reference. |
| `https://github.com/gildor/kotlin-coroutines-okhttp`, `https://github.com/square/okhttp/blob/master/okhttp-coroutines/src/main/kotlin/okhttp3/coroutines/ExecuteAsync.kt` | `core/common/src/main/java/eu/kanade/tachiyomi/network/OkHttpExtensions.kt:72-73` | Provenance of `await()` helper. |
| `jmir1/Aniyomi/v${VERSION_NAME} (Android) (https://github.com/aniyomiorg/aniyomi)` | `app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt:38` | User-Agent string for Bangumi (chunk 1; tracker details in area:31-tracker-shikimori-bangumi). |

OBSERVED sibling endpoint, the application update check (`api.github.com`), `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:21`:

- `GET https://api.github.com/repos/{GITHUB_REPO}/releases/latest` through the shared client (so DoH and the 10-minute default cache apply). No auth header, no query parameters, no pagination, no client-side rate limit.
- Response fields consumed (`data/src/main/java/tachiyomi/data/release/GithubRelease.kt`): `tag_name` (to `version`), `body` (to `info`), `html_url` (to `releaseLink`), `assets[].name`, `assets[].browser_download_url`.
- Asset choice: the code looks for an asset whose name contains `-arm64-v8a`, `-armeabi-v7a`, `-x86_64` or `-x86` (`BUILD_TYPES`, `ReleaseServiceImpl.kt:45`); it picks the one matching `Build.SUPPORTED_ABIS[0]`, else the asset with no ABI suffix (the universal APK), else `null` which yields "no update".
- Gate (`domain/src/main/java/tachiyomi/domain/release/interactor/GetApplicationRelease.kt`): unless `forceCheck`, the check is skipped when `now < last_app_check + 3 days` (preference key `Preference.appStateKey("last_app_check")`, default 0). The timestamp is written only after a successful fetch with a downloadable asset. Version comparison: strip every character that is not a digit or `.` from both versions; preview builds compare `newVersion.toInt() > commitCount`; stable builds split on `.` and return true as soon as ANY component of the new version is greater than the same-index component of the old version (a quirk: new `0.19.9` vs installed `1.0.0` returns true because `9 > 0` at index 2; a shorter new version throws `IndexOutOfBounds` if the installed one has more parts).
- Result variants: `NewUpdate(release)`, `NoNewUpdate`, `OsTooOld` (the OS-too-old branch is commented out in `AppUpdateChecker.kt:17-20`).

Keep? **Keep (links)**. Proposal: keep the About-screen GitHub link and the extension "open repo" action using `gtk::UriLauncher`/`gio::AppInfo::launch_default_for_uri`. Keep a repository-constant `KUROBAYOMI_REPO` in `src/app_info.rs`. Replace the update checker: on Linux the package manager (pacman/AUR, Flatpak) owns updates, so Proposal: a lightweight "new release available" check against `https://api.github.com/repos/<kurobayomi repo>/releases/latest` (one request per 3 days, preference `last_app_check`, correct semver comparison with the `semver` crate instead of the quirky loop, no asset selection, opens `html_url`). Default it to off until the repo is public; the user loses in-app APK download and install, which do not apply on Linux. Owned by area:35-data-misc-updater-notification-export (update flow) and area:12-network-stack.

Test advice: `curl -s https://api.github.com/repos/aniyomiorg/aniyomi/releases/latest -A x | jq '.tag_name,.html_url,[.assets[].name]'` (live probe 2026-10-08: HTTP 200 `application/json`). Unit-test the version comparison with `(installed 1.0.0, new 0.19.9) -> no update` (this deliberately differs from the Kotlin) and mention-rewrite with `"@user-name"` and `"a@b"` (the `\B` anchor must not match inside the email).

---

### 2.2 `github.githubassets.com`

Role: image host used in the pull-request template only. `.github/pull_request_template.md:11` embeds `https://github.githubassets.com/images/modules/logos_page/Octocat.png` twice in a "before / after" screenshot table.

OBSERVED: no code path. Keep? **Doc-only**. Proposal: none; the Kurobayomi repository should have its own PR template without remote images. Test: none.

---

### 2.3 `goo.gle`

Role: short link to Google's Material Theme Builder, `https://goo.gle/material-theme-builder-web`. Appears as a credit comment in the generated Material 3 colour scheme sources: `app/src/main/java/eu/kanade/presentation/theme/colorscheme/` files `CloudflareColorScheme.kt:10`, `CottoncandyColorScheme.kt:9`, `DoomColorScheme.kt:10`, `GreenAppleColorScheme.kt:10`, `MatrixColorScheme.kt:10`, `MidnightDuskColorScheme.kt:10`, `MochaColorScheme.kt:9`, `SapphireColorScheme.kt:10`, `StrawberryColorScheme.kt:10`, `TachiyomiColorScheme.kt:9`, `TakoColorScheme.kt:10`, `YotsubaColorScheme.kt:10`, and the resource files `app/src/main/res/values/colors_mocha.xml:5`, `app/src/main/res/values-night/colors_mocha.xml:5`, `presentation-core/src/main/res/values/colors_{greenapple,midnightdusk,strawberry,tachiyomi,yotsuba}.xml` and the same names under `values-night/`.

OBSERVED: never contacted. Keep? **Doc-only**. Proposal: Kurobayomi uses the desktop GTK theme (vanilla GTK, user's theme applies) so these Android Material 3 palettes are dropped (decision recorded under area:49-theme-and-design-system). The user loses the in-app Aniyomi colour themes; the GTK theme (Tokyo Night in the user's setup) replaces them. Test: none.

---

### 2.4 `graphql.anilist.co`

Role: **tracker API** (AniList), plus two non-tracker lookups used by the player (AniSkip) and the anime page (airing countdown). Three code locations call it:

| Caller | URL as written | Client |
| --- | --- | --- |
| `app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/AnilistApi.kt:571` | `https://graphql.anilist.co/` (trailing slash) | authenticated client: shared client + `AnilistInterceptor` + `rateLimit(permits = 85, period = 1.minutes)` (`AnilistApi.kt:40-43`) |
| `app/src/main/java/eu/kanade/tachiyomi/ui/player/utils/AniSkipApi.kt:55` | `https://graphql.anilist.co` | bare `OkHttpClient()`, unauthenticated |
| `app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:64` and `:93` | `https://graphql.anilist.co` | bare `OkHttpClient()`, unauthenticated |

#### 2.4.1 Authentication (implicit grant)

OBSERVED (`AnilistApi.kt:570-588`, `app/src/main/java/eu/kanade/tachiyomi/ui/setting/track/TrackLoginActivity.kt`, `AndroidManifest.xml:243-258`):

- Authorize URL: `https://anilist.co/api/v2/oauth/authorize?client_id=5338&response_type=token` (no `redirect_uri` parameter; the redirect is the one registered for the client: `aniyomi://anilist-auth`).
- The system browser returns to `aniyomi://anilist-auth#access_token=<jwt>&token_type=Bearer&expires_in=<s>`. `handleAnilist` runs the regex `(?:access_token=)(.*?)(?:&)` against `data.fragment` (the fragment must contain a `&` after the token or the match fails; AniList puts `token_type` and `expires_in` after the token so it works). On no match: `anilist.logout()`.
- `Anilist.login(token)` (`Anilist.kt:305-320`): `api.createOAuth(token)` builds `ALOAuth(accessToken = token, tokenType = "Bearer", expires = System.currentTimeMillis() + 31536000000, expiresIn = 31536000000)`; `interceptor.setAuth(oauth)` persists it as JSON under preference `track_token_<id>` (private key) and keeps it in memory; `api.getCurrentUser()` queries `Viewer`; the returned `scoreFormat` is stored in preference `anilist_score_type`; `saveCredentials(username = viewer.id.toString(), password = accessToken)` (the "username" field holds the numeric user id). Any exception during login silently calls `logout()`.
- Expiry quirk: the interceptor's `oauth` setter replaces `expires` with `value.expires * 1000 - 60 * 1000`; since `expires` is already in milliseconds the stored value is about 31.5 * 10^12 times larger, so `isExpired()` (`System.currentTimeMillis() > expires`) is effectively never true. Real AniList tokens last one year; an expired token surfaces as HTTP 401 / GraphQL error instead.
- `AnilistInterceptor` (`.../anilist/AnilistInterceptor.kt`): throws `Exception("Not authenticated with Anilist")` when the stored token is empty; when expired calls `anilist.logout()` and throws `IOException("Token expired")`; otherwise adds `Authorization: Bearer <token>` (via `addHeader`) and overrides `User-Agent` with `Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`.
- `logout()` clears credentials and the token preference and `interceptor.setAuth(null)`.
- Tracker ids: `Anilist` is `TrackerManager.ANILIST = 2L` (`app/src/main/java/eu/kanade/tachiyomi/data/track/TrackerManager.kt:20`); MyAnimeList is `1L`, Kitsu `3L`, Shikimori `4L`, MangaUpdates `7L`, Simkl `101L`.

#### 2.4.2 GraphQL operations (all `POST`, body `{"query": "<text>", "variables": {...}}`, `Content-Type: application/json; charset=utf-8`)

Variable names and selections are exact; whitespace is insignificant. "Manga" and "Anime" variants differ only in the id variable name and the selected count and credit fields.

1. `AddManga` / `AddAnime` (`AnilistApi.kt:45-80`, `:138-173`):
   `mutation AddManga($mangaId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean) { SaveMediaListEntry (mediaId: $mangaId, progress: $progress, status: $status, private: $private) { id status } }`
   Variables: `mangaId` (or `animeId`) = `track.remote_id`, `progress` = `last_chapter_read.toInt()` (or `last_episode_seen.toInt()`), `status` = status string (table 2.4.3), `private` = `track.private`. Response parsed as `ALAddEntryResult` (`data.SaveMediaListEntry.id`); the returned id is NOT written back to the track at add time (the code returns the original `track`); `library_id` is filled later by `update` through `findLibManga`.
2. `UpdateManga` / `UpdateAnime` (`:81-116`, `:174-209`):
   `mutation UpdateManga($listId: Int, $progress: Int, $status: MediaListStatus, $private: Boolean, $score: Int, $startedAt: FuzzyDateInput, $completedAt: FuzzyDateInput) { SaveMediaListEntry(id: $listId, progress: $progress, status: $status, private: $private, scoreRaw: $score, startedAt: $startedAt, completedAt: $completedAt) { id status progress } }`
   Variables: `listId` = `track.library_id`, `progress`, `status`, `score` = `track.score.toInt()` (0-100 raw), `startedAt` / `completedAt` = fuzzy dates, `private`. Fuzzy date builder (`createDate`, `:552-568`): epoch millis `0` gives `{"year":null,"month":null,"day":null}`; otherwise the local-time-zone (`ZoneId.systemDefault()`) `year`, `monthValue`, `dayOfMonth`. Response is not parsed (`awaitSuccess` only).
3. `DeleteManga` / `DeleteAnime` (`:117-137`, `:210-230`): `mutation DeleteManga($listId: Int) { DeleteMediaListEntry(id: $listId) { deleted } }` with `listId` = `track.libraryId`.
4. `Search` manga (`:231-292`): `query Search($query: String) { Page (perPage: 50) { media(search: $query, type: MANGA, format_not_in: [NOVEL]) { id staff { edges { role id node { name { full userPreferred native } } } } title { userPreferred } coverImage { large } format status chapters description startDate { year month day } averageScore } } }`, variable `query` = raw user string. Anime variant (`:293-349`): `media(search: $query, type: ANIME)` selecting `id studios { edges { isMain node { name } } } title { userPreferred } coverImage { large } format status episodes description startDate { year month day } averageScore`.
5. `findLibManga` (`:350-430`): `query ($id: Int!, $manga_id: Int!) { Page { mediaList(userId: $id, type: MANGA, mediaId: $manga_id) { id status scoreRaw: score(format: POINT_100) progress private startedAt { year month day } completedAt { year month day } media { id title { userPreferred } coverImage { large } format status chapters description startDate { year month day } staff { edges { role id node { name { full userPreferred native } } } } } } } }`. `$id` is the numeric user id taken from `getUsername().toInt()`. First element of `mediaList` or `null`. Anime variant (`:431-506`) uses `type: ANIME, mediaId: $anime_id`, selects `episodes` and `studios { edges { isMain node { name } } }`.
6. `getLibManga` / `getLibAnime` = `findLib*` or throw `Exception("Could not find manga")` / `Exception("Could not find anime")`.
7. `User` (`getCurrentUser`, `:519-551`): `query User { Viewer { id mediaListOptions { scoreFormat } } }`, no variables.

Response consumption (`app/src/main/java/eu/kanade/tachiyomi/data/track/anilist/dto/`):

- `ALSearchItem`: `id`, `title.userPreferred`, `coverImage.large`, `description` (HTML-decoded by `htmlDecode()`; null becomes empty string), `format` (underscores replaced by hyphens, null becomes empty string; e.g. `ONE_SHOT` becomes `ONE-SHOT`), `status` (kept raw, e.g. `RELEASING`; null becomes empty string), `startDate` fuzzy to epoch millis (requires year, month and day all non-null, else `0`) then rendered `yyyy-MM-dd` (`Locale.US`) only when non-zero, `chapters` / `episodes` (null becomes 0), `averageScore` (null becomes `-1`), `staff`, `studios`.
- Manga credits: for each staff edge, name = `name.userPreferred ?: name.full ?: name.native`; if `"Story" in role` add to `authors`; if `"Art" in role` add to `artists` (an edge may land in both). Anime credits: `studios.edges` filtered by `isMain`; if that list is empty, all edges; first 3; names only.
- `tracking_url` = `https://anilist.co/manga/<id>` or `https://anilist.co/anime/<id>` (`AnilistApi.kt:572-573, 576-582`).
- List entry (`ALUserListItem`): `id` to `library_id`, `status`, `scoreRaw` (0-100), `progress`, `startedAt`, `completedAt`, `private`, plus embedded `media`.

#### 2.4.3 Status mapping (`.../anilist/AnilistUtils.kt`, constants `Anilist.kt:39-47`)

| Local constant | Value | AniList `MediaListStatus` |
| --- | --- | --- |
| `READING` | 1 | `CURRENT` |
| `WATCHING` | 11 | `CURRENT` |
| `COMPLETED` | 2 | `COMPLETED` |
| `ON_HOLD` | 3 | `PAUSED` |
| `DROPPED` | 4 | `DROPPED` |
| `PLAN_TO_READ` | 5 | `PLANNING` |
| `PLAN_TO_WATCH` | 15 | `PLANNING` |
| `REREADING` | 6 | `REPEATING` |
| `REWATCHING` | 16 | `REPEATING` |

Inverse mapping (`ALUserManga.toTrackStatus`, `ALUserAnime.toTrackStatus`): any unknown string throws `NotImplementedError("Unknown status: ...")`.

#### 2.4.4 Score formats (`Anilist.kt:49-53,120-200`, `AnilistUtils.kt`)

The user's `scoreFormat` is read from `Viewer.mediaListOptions.scoreFormat` at login and stored in `anilist_score_type` (default `POINT_10`). Storage inside the app is always the 0-100 raw value; `getScoreList()` and `indexToScore(index)`:

| scoreFormat | UI choices | index to stored score |
| --- | --- | --- |
| `POINT_10` | `0..10` | `index * 10.0` |
| `POINT_100` | `0..100` | `index.toDouble()` |
| `POINT_5` | `0 ★` to `5 ★` | `0 -> 0.0`, else `index * 20.0 - 10.0` |
| `POINT_3` | `-`, smiley sad, neutral, happy | `0 -> 0.0`, else `index * 25.0 + 10.0` |
| `POINT_10_DECIMAL` | `0.0..10.0` in 0.1 steps (101 entries) | `index.toDouble()` |

Display: `POINT_5` shows `((score + 10) / 20).toInt()` stars (0 shows `0`); `POINT_3` shows `0` for 0.0, sad for `<= 35`, neutral for `<= 60`, happy otherwise. `get10PointScore` = `score / 10.0`. The (unused by the API path) `toApiScore()` string converters are in `AnilistUtils.kt`; the actual update sends `score.toInt()` as `scoreRaw`, which AniList interprets according to the user's format.

#### 2.4.5 Behaviour on update/bind/refresh/delete (`Anilist.kt:204-300`)

- `update(track, didReadChapter)`: if `library_id` is null or 0 (legacy API v1 data) it first calls `findLibManga` and throws `Exception("$track not found on user library")` when absent. If the status is not `COMPLETED` and a chapter was read: when `last_chapter_read.toLong() == total_chapters && total_chapters > 0` set status `COMPLETED` and `finished_reading_date = now`; else if status is not `REREADING`, set `READING`, and when `last_chapter_read == 1.0` set `started_reading_date = now`. Then call `UpdateManga`. Anime is identical with `WATCHING`/`REWATCHING`/`last_episode_seen`/`total_episodes`.
- `bind` (`:262-300`): `findLibManga`; if found, `copyPersonalFrom(remote, copyRemotePrivate = false)` (copies `last_chapter_read`, `score`, `status`, start and finish dates; keeps the local `private` flag), `library_id` = remote, status becomes `READING` if not rereading/completed and the user has read chapters, then `update`. If not found: status = `READING` when `hasReadChapters` else `PLAN_TO_READ`/`PLAN_TO_WATCH`, `score = 0.0`, then `AddManga`.
- `refresh`: `getLibManga` then `copyPersonalFrom(remote)`, copy `title` and `total_chapters`/`total_episodes`.
- `delete`: when `libraryId` is null or 0, it looks the entry up first (`findLib*`) and returns silently if absent; otherwise `DeleteMediaListEntry`.
- Capabilities: `supportsReadingDates = true`, `supportsPrivateTracking = true`.

#### 2.4.6 AniSkip id lookup (`AniSkipApi.getMalIdFromAL`, `AniSkipApi.kt:46-69`)

`POST https://graphql.anilist.co` body `{"query":"query{\n Media(id:<anilistId>){idMal}\n }"}` (query string built with Kotlin string interpolation, no variables). The response is string-scraped: `substringAfter("idMal\":").substringBefore("}").toLongOrNull() ?: 0`. Any network exception returns `0`. Result `0` means "no MAL id", so AniSkip is not queried.

#### 2.4.7 Airing-time lookups (`AniChartApi.kt`)

- `getAlIdFromMal(idMal)` (`:51-77`): `query { Media(idMal:<idMal>,type: ANIME) { id } }`; scrape `substringAfter("id\":").substringBefore("}").toLongOrNull() ?: 0L`.
- `getAnilistAiringEpisodeData(id)` (`:78-108`): `query { Media(id:<id>) { nextAiringEpisode { episode airingAt } } }`; scrape `episode` with `substringAfter("episode\":").substringBefore(",").toIntOrNull() ?: 1` and `airingAt` with `substringAfter("airingAt\":").substringBefore("}").toLongOrNull() ?: 0L`. A finished show (`nextAiringEpisode: null`) therefore yields `(1, 0)`.
- Entry point `loadAiringTime(anime, trackItems, manualFetch)` (`:22-50`): start with the stored pair `(anime.nextEpisodeToAir, anime.nextEpisodeAiringAt)`; return it immediately when `anime.status == SAnime.COMPLETED (2)` and not a manual fetch. Otherwise take the FIRST tracker among `Anilist`, `MyAnimeList`, `Simkl` that has a linked track; none gives `(1, 0L)`. AniList uses `track.remoteId` directly; MyAnimeList converts through `getAlIdFromMal`; Simkl goes through the Simkl calendar (section 2.27).
- Triggered from `app/src/main/java/eu/kanade/tachiyomi/ui/entries/anime/AnimeScreenModel.kt:1520-1546` whenever the set of tracks or logged-in trackers changes (`collectLatest`, `manualFetch = false`); the result is saved by `setAnimeViewerFlags.awaitSetNextEpisodeAiring` and shown as "next episode airs at" in the anime header.

Rate limit and pagination: authenticated calls are limited client-side to 85 requests per minute (AniList's documented limit is 90/min, degraded mode 30/min); `Page` queries use `perPage: 50` for search and no page argument elsewhere (one page). The bare-client calls are unlimited.

Keep? **Keep** (tracker, AniSkip id lookup, airing times). Proposal for Rust (`src/track/anilist.rs`):

- Use `serde_json` structs, never string scraping: `Media(id:) { idMal }` parsed as `data.Media.idMal: Option<i64>`; `nextAiringEpisode: Option<{episode:i32, airingAt:i64}>`.
- Register a Kurobayomi AniList API client (AniList lets each client set its redirect URL). Preferred redirect for a desktop app: implicit grant to a loopback page is awkward because the token is in the URL fragment which a server never receives; Proposal: register redirect `http://127.0.0.1:<fixed port>/anilist-auth` that serves a tiny HTML page whose JS posts `location.hash` back to the loopback server (`POST /anilist-auth/token`), then closes. Alternative that needs no registration: reuse client `5338` with the custom scheme `aniyomi://anilist-auth` by installing a desktop file declaring `MimeType=x-scheme-handler/aniyomi;` (works because Aniyomi has no Linux build, but it ties Kurobayomi to someone else's client id; use only as a fallback).
- Keep 85 req/min with a token-bucket (`governor` crate). Store tokens in the Secret Service via the `oo7` crate (libsecret) instead of plain preferences (Android keystore equivalent).
- Fix the expiry bug: store `expires_at = now + expires_in` (seconds) and treat HTTP 401 or a GraphQL `errors[].message` of `Invalid token` as logout.
- Owned by area:27-tracking-core (tracker contract) and area:12-network-stack (client, rate limiting).

Test advice: unauthenticated live checks (all verified 2026-10-08): `curl -s -X POST https://graphql.anilist.co/ -H 'Content-Type: application/json' -d '{"query":"query{Media(idMal:1,type:ANIME){id}}"}'` returns HTTP 200 JSON. The `Search` query also works without a token. Authenticated mutations need a real token; test them with `wiremock` recording expected bodies, asserting the exact variable names above and the fuzzy-date `null` encoding for a zero date. Table-driven unit tests for score conversion (`POINT_5`: 70 shows 4 stars is `((70+10)/20)=4`; `POINT_3`: 35 is sad, 36 neutral, 61 happy).

---

### 2.5 `hosted.weblate.org`

Role: translation platform. `README.md:17` links the Weblate "engage" page `https://hosted.weblate.org/engage/aniyomi/` behind a progress badge. OBSERVED: no code path; strings live in `i18n-aniyomi/src/commonMain/moko-resources/<locale>/strings.xml`. Keep? **Doc-only**. Proposal: Kurobayomi uses gettext (`.po` via the `gettext-rs` or `fluent` crate; decision in area:51-i18n); Weblate can host those later. Test: none.

---

### 2.6 `imagecdn.manga.com`

Role: fictional example host in the KDoc of the per-host rate limiter, `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/SpecificHostRateLimitInterceptor.kt:21,46,67` ("10 requests per 2 minutes to imagecdn.manga.com"). OBSERVED: never contacted.

The real behaviour behind the example (for extensions-lib compatibility, `rateLimitHost(...)` and `rateLimit(...)` are public API that extensions call on their own `OkHttpClient.Builder`; see `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/RateLimitInterceptor.kt`):

- Signatures: `rateLimitHost(httpUrl: HttpUrl, permits: Int, period: Duration = 1.seconds)`, `rateLimitHost(url: String, permits: Int, period: Duration = 1.seconds)` (extension-lib 1.5), and the deprecated `rateLimitHost(httpUrl, permits, period: Long = 1, unit: TimeUnit = SECONDS)` (extension-lib 1.3). `rateLimit(permits, period)` is the same without a host filter.
- Algorithm (`RateLimitInterceptor`): a fair `Semaphore(1, true)` serialises callers; a deque of request timestamps (`SystemClock.elapsedRealtime()`); when the deque holds `permits` entries, expired entries (older than `period`) are removed, and if none expired the thread waits until the oldest expires, re-checking cancellation (`IOException("Canceled")`). Requests to other hosts pass through untouched when a host filter is set. A cached response notifies waiters (`requestQueue.notifyAll()` on cache hit).

Keep? **Replace** (the example host is Doc-only; the mechanism is kept). Proposal: implement the same semantics in Rust as a `tower`/`reqwest-middleware` layer with `governor` (sliding window, per-host keyed limiter) for the native sources; extension (Kotlin) sources run outside this process in the chosen extension mechanism, whose HTTP calls go through the host app's client, so the limiter must live there (owned by area:12-network-stack and area:20-extension-strategy-redesign). Test: deterministic unit test with `tokio::time::pause()`: with 2 permits per 10 s, the third call completes at t = 10 s.

---

### 2.7 `img.shields.io`

Role: README badge images: Discord (`README.md:12`), downloads (`:13`), CI (`:15`), license (`:16`), Weblate progress (`:17`), stable release (`:21`), preview release (`:22`). Example URL: `https://img.shields.io/github/license/aniyomiorg/aniyomi?labelColor=27303D&color=818cf8`. OBSERVED: documentation only. Keep? **Doc-only**. Test: none.

---

### 2.8 `issuetracker.google.com`

Role: links to Jetpack Compose bugs inside code comments that explain workarounds: `https://issuetracker.google.com/issues/204502668` (a to-do comment in `app/src/main/java/eu/kanade/presentation/category/components/CategoryDialogs.kt:96,160`, `.../browse/components/anime/AnimeExtensionStoresDialogs.kt:97`, `.../browse/components/ExtensionReposDialogs.kt:88`, `.../player/editor/components/EditorDialogs.kt:103`, `.../player/custombutton/components/CustomButtonDialogs.kt:178`; the bug is about auto-focusing a text field inside a dialog), `issues/246909281` (`decorFitsSystemWindows = false` does not work: `app/src/main/java/eu/kanade/presentation/entries/anime/components/AnimeImagesDialog.kt:102`, `.../manga/components/MangaCoverDialog.kt:71`), `issues/242879624` (`app/src/main/java/eu/kanade/presentation/library/components/LibraryTabs.kt:32`), `issues/352584409` (`app/src/main/java/eu/kanade/presentation/util/FastScrollAnimateItem.kt:6`), `u/0/issues/316327367` (`app/src/main/java/eu/kanade/tachiyomi/ui/home/HomeScreen.kt:317`), `issues/238793260` (`presentation-widget/src/main/java/tachiyomi/presentation/widget/components/{anime/UpdatesAnimeWidget.kt,manga/UpdatesMangaWidget.kt}:83`).

OBSERVED: never contacted. Keep? **Doc-only** (Android Compose bugs are irrelevant to GTK4). Proposal: when implementing the matching GTK dialogs, remember the user-visible intent: in text-entry dialogs (new category, add extension repo, mpv editor) the entry grabs focus when the dialog opens (`Entry::grab_focus()` in the dialog's `map` handler). Test: GTK UI test asserts focus after `present()`.

---

### 2.9 `keepachangelog.com`

Role: `https://keepachangelog.com/en/1.1.0/`, referenced by `CHANGELOG.md:5` ("modified version of Keep a Changelog"). Doc-only. Proposal: Kurobayomi can adopt the same changelog format; the in-app "What's new" would then render the GitHub release body. Test: none.

---

### 2.10 `kitsu.app`

Role: **tracker API** (Kitsu), manga and anime. Also reached: `AWQO5J657S-dsn.algolia.net` (Algolia search backend used by Kitsu for search). Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/kitsu/KitsuApi.kt`, `KitsuInterceptor.kt`, `KitsuUtils.kt`, `KitsuDateHelper.kt`, `Kitsu.kt`, `dto/*.kt`. Tracker id `3L`.

Constants (`KitsuApi.kt:428-441`): `BASE_URL = "https://kitsu.app/api/edge/"`, `LOGIN_URL = "https://kitsu.app/api/oauth/token"`, `BASE_MANGA_URL = "https://kitsu.app/manga/"`, `BASE_ANIME_URL = "https://kitsu.app/anime/"`, `ALGOLIA_KEY_URL = "https://kitsu.app/api/edge/algolia-keys/media/"`, `ALGOLIA_URL = "https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/"`, `ALGOLIA_APP_ID = "AWQO5J657S"`, `CLIENT_ID = "dd031b32d2f56c990b1425efe6c42ad847e7fe3ab46bf1299f05ecd856bdb7dd"` and a `CLIENT_SECRET` (the well-known public Kitsu API client; `KitsuApi.kt:430-431`; not reproduced here, copy from the file if you reuse it). `VND_API_JSON = "application/vnd.api+json"`.

#### 2.10.1 Authentication (resource-owner password grant)

- Login (`KitsuApi.login`, `:395-411`): `POST https://kitsu.app/api/oauth/token`, form body `username`, `password`, `grant_type=password`, `client_id`, `client_secret`, via the unauthenticated shared client. Response `KitsuOAuth`: `access_token`, `token_type`, `created_at` (seconds), `expires_in` (seconds), `refresh_token` (nullable).
- `Kitsu.login(username, password)` (`Kitsu.kt:226-231`): store token (`track_token_3`), `GET /users?filter[self]=true` for the numeric id (`data[0].id`), `saveCredentials(username, userId)` (the "password" slot holds the Kitsu user id string).
- Interceptor (`KitsuInterceptor.kt`): throws `Exception("Not authenticated with Kitsu")` without a token. Expired is `now_s > created_at + expires_in - 3600` (one-hour margin); then it calls `POST LOGIN_URL` form `grant_type=refresh_token`, `refresh_token`, `client_id`, `client_secret` straight on the chain; on success it stores the new token, on failure it just closes the response and continues with the old token. Headers added on every call: `Authorization: Bearer <access>`, `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`, `Accept: application/vnd.api+json`, `Content-Type: application/vnd.api+json`.
- No client-side rate limit, no pagination (all calls fetch one entry or one search page).

#### 2.10.2 Endpoints

| Operation | Method and URL | Body / query | Response used |
| --- | --- | --- | --- |
| add manga/anime (`addLibManga`/`addLibAnime`, `:45-134`) | `POST https://kitsu.app/api/edge/library-entries`, `Content-Type: application/vnd.api+json` | `{"data":{"type":"libraryEntries","attributes":{"status":<s>,"progress":<int>,"private":<bool>},"relationships":{"user":{"data":{"id":"<userId>","type":"users"}},"media":{"data":{"id":<remoteId>,"type":"manga"\|"anime"}}}}}` | `data.id` (Long): OVERWRITES `track.remote_id` with the library-entry id (so after add, `remote_id` is the entry id, not the media id). |
| update (`updateLibManga`/`updateLibAnime`, `:135-198`) | `PATCH https://kitsu.app/api/edge/library-entries/<remote_id>` | `{"data":{"type":"libraryEntries","id":<remote_id>,"attributes":{"status","progress","ratingTwenty","startedAt","finishedAt","private"}}}` | not parsed |
| remove (`:199-222`) | `DELETE https://kitsu.app/api/edge/library-entries/<remoteId>` | header `Content-Type: application/vnd.api+json` | none |
| search key (`search`/`searchAnime`, `:223-248`) | `GET https://kitsu.app/api/edge/algolia-keys/media/` (authenticated client) | none | `media.key` (search-only Algolia API key) |
| Algolia search (`:249-310`) | `POST https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/` (shared UNauthenticated client), headers `X-Algolia-Application-Id: AWQO5J657S`, `X-Algolia-API-Key: <media.key>` | `{"params":"query=<url-encoded query>&facetFilters=%5B%22kind%3Amanga%22%5D&attributesToRetrieve=%5B%22synopsis%22%2C%22averageRating%22%2C%22canonicalTitle%22%2C%22chapterCount%22%2C%22posterImage%22%2C%22startDate%22%2C%22subtype%22%2C%22endDate%22%2C%20%22id%22%5D"}` (anime: `kind%3Aanime` and `episodeCount` instead of `chapterCount`) | `hits[]` filtered `subtype != "novel"` |
| find entry (`findLibManga`/`findLibAnime`, `:311-352`) | `GET https://kitsu.app/api/edge/library-entries?filter[manga_id]=<remote_id>&filter[user_id]=<userId>&include=manga` (anime: `filter[anime_id]`, `include=anime`) | none | `data[0]` and `included[0]`; `null` unless both are non-empty |
| get entry (`getLibManga`/`getLibAnime`, `:353-394`) | `GET .../library-entries?filter[id]=<remote_id>&include=manga` (or `anime`) | none | same; throws `Exception("Could not find manga")` when empty (the anime variant reuses the word "manga") |
| current user (`:412-426`) | `GET https://kitsu.app/api/edge/users?filter[self]=true` | none | `data[0].id` |

Response fields consumed: Algolia hit `id`, `canonicalTitle`, `chapterCount`, `episodeCount`, `subtype`, `posterImage.original`, `synopsis`, `averageRating`, `startDate` (epoch SECONDS, rendered `yyyy-MM-dd`), `endDate` (null means "Publishing", else "Finished"); `publishing_type = subtype`; `score = averageRating ?: -1.0`. Library entry: `attributes.status`, `startedAt`, `finishedAt`, `ratingTwenty`, `progress`, `private`; media attributes `canonicalTitle`, `chapterCount`, `episodeCount`, `mangaType`, `showType`, `posterImage.original`, `synopsis`, `startDate`, `status`.

Status mapping (`KitsuUtils.kt`; constants `Kitsu.kt:38-44`): `READING 1` and `WATCHING 11` to `current`; `COMPLETED 2` to `completed`; `ON_HOLD 3` to `on_hold`; `DROPPED 4` to `dropped`; `PLAN_TO_READ 5` and `PLAN_TO_WATCH 15` to `planned`. Unknown throws `Exception("Unknown status")`.

Score: stored 0-10 with half steps (`score = ratingTwenty / 2.0`); sent as `ratingTwenty = (score * 2).toInt()` as a string, or JSON `null` when `score <= 0`. UI list `["0"] + (2..20).map { df.format(it/2f) }` with `DecimalFormat("0.#")` (so `1`, `1.5`, `2`, ... `10`); `indexToScore(index) = if (index > 0) (index + 1) / 2.0 else 0.0`.

Dates: `KitsuDateHelper` formats with `SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss.SSS'Z'", Locale.ENGLISH)` using the device default time zone while appending a literal `Z` (so the instant is wrong by the local UTC offset; observed bug). A zero epoch becomes JSON `null`.

Behaviour: `update` (`Kitsu.kt:124-158`) sets `COMPLETED` + finished date when `last == total && total > 0`, else sets `READING`/`WATCHING` (it has no rereading state: `getRereadingStatus() = -1`) and the started date when `last == 1.0`. `bind` finds the entry; if found copies personal fields, sets `remote_id` = entry id, upgrades status as for AniList then `update`; if not found sets status `READING`/`PLAN_TO_READ` (anime `WATCHING`/`PLAN_TO_WATCH`), score 0, `add`. `refresh` copies personal fields and totals. Capabilities: reading dates and private tracking.

Keep? **Keep**. Proposal (`src/track/kitsu.rs`): `reqwest` with `.header(ACCEPT, "application/vnd.api+json")`; JSON:API parsing with typed structs; do NOT overwrite the media id: keep `remote_id` (media id) and a separate `library_id` (entry id) in the `tracks` table (area:03-db-manga-schema-migrations) to avoid the dual meaning. Format dates in UTC (`chrono::Utc`) with milliseconds and `Z`. Store the password-grant token and refresh token in the Secret Service; do not store the user's password. Reusing the shared public client id/secret from the Aniyomi source is acceptable for Kitsu because Kitsu documents these as public values; verify they still work first (see test advice).

Test advice: `curl -s https://kitsu.app/api/edge/algolia-keys/media/` (live probe 2026-10-08: HTTP 200, body `{"media":{"key":"..."}}`). With that key, `curl -s -X POST 'https://AWQO5J657S-dsn.algolia.net/1/indexes/production_media/query/' -H 'X-Algolia-Application-Id: AWQO5J657S' -H "X-Algolia-API-Key: $KEY" -d '{"params":"query=berserk&facetFilters=%5B%22kind%3Amanga%22%5D"}'`. A bare `POST /api/oauth/token` without credentials returned HTTP 403 from the edge on 2026-10-08, so verify the password grant with a real (throwaway) account before relying on it. Unit-test: JSON:API body construction for add, the `ratingTwenty` null/number rule, and the UTC date format.

---

### 2.11 `kotlinlang.org`

Role: `https://kotlinlang.org/`, link in `CONTRIBUTING.md:20` ("Kotlin" under the tech list). Doc-only. Proposal: the Kurobayomi contributing guide links `https://www.rust-lang.org/` and `https://gtk-rs.org/` instead. Test: none.

---

### 2.12 `libredns.gr`

Role: credit comment for DoH provider 13 (`core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt:189`, "Source: https://libredns.gr/"). Endpoint: `https://doh.libredns.gr/dns-query` (`:193`). Part of the DoH table documented in full in section 2.17 (provider constant `PREF_DOH_LIBREDNS = 13`). Bootstrap addresses: `116.202.176.26`, `2a01:4f8:1c0c:8274::1`. Keep? **Keep-opt**. Test: live probe 2026-10-08 `GET https://doh.libredns.gr/dns-query?dns=AAABAAABAAAAAAAAB2V4YW1wbGUDY29tAAABAAE` with `Accept: application/dns-message` returned HTTP 200 `application/dns-message`.

---

### 2.13 `m3.material.io`

Role: `https://m3.material.io/components/navigation-bar/guidelines#90615a71-607e-485e-9e09-778bfc080563`, a comment in `app/src/main/java/eu/kanade/presentation/theme/colorscheme/BaseColorScheme.kt:12` explaining how the navigation-bar indicator colour is derived (secondary container). Doc-only. Proposal: not applicable (GTK theme). Test: none.

---

### 2.14 `mangaupdates.com`

Role: **tracker API** (MangaUpdates, manga only, no anime). Website `https://mangaupdates.com` (README); API host `https://api.mangaupdates.com` (`app/src/main/java/eu/kanade/tachiyomi/data/track/mangaupdates/MangaUpdatesApi.kt:194`). Tracker id `7L`. Files: `MangaUpdatesApi.kt`, `MangaUpdatesInterceptor.kt`, `MangaUpdates.kt`, `dto/*.kt`.

Auth: session token. `PUT https://api.mangaupdates.com/v1/account/login` with body `{"username":"...","password":"..."}` (content type `application/vnd.api+json`, from `CONTENT_TYPE`), unauthenticated client; response `{"context":{"session_token":"...","uid":<long>}}`. `MangaUpdates.login` (`MangaUpdates.kt:104-114`) throws `Throwable("Unable to login")` on a null context, then `saveCredentials(uid.toString(), sessionToken)` (token stored in the "password" slot, restored with `trackPreferences.trackPassword(this).get().ifBlank { null }`) and `interceptor.newAuth(token)`. Every authenticated call adds `Authorization: Bearer <session_token>` and `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`; without a token it throws `IOException("Not authenticated with MangaUpdates")`. There is no refresh flow and no client-side rate limit.

Endpoints (all JSON bodies use `application/vnd.api+json`):

| Operation | Method and path | Body | Notes |
| --- | --- | --- | --- |
| get list item (`getSeriesListItem`, `:43-54`) | `GET /v1/lists/series/{remote_id}` | none | response `MUListItem {series{id,title}, list_id, status{volume,chapter}, priority}`; then `getSeriesRating` |
| get rating (`:116-127`) | `GET /v1/series/{remote_id}/rating` | none | response `{"rating": <double>}`; any exception returns null |
| add to list (`addSeriesToList`, `:55-79`) | `POST /v1/lists/series` | `[{"series":{"id":<remote_id>},"list_id":<status>}]` | status = `READING_LIST (0)` when the user has read chapters else `WISH_LIST (1)`; on HTTP 200 sets `track.status` and `track.last_chapter_read = 1.0` |
| update (`updateSeriesListItem`, `:80-102`) | `POST /v1/lists/series/update` | `[{"series":{"id":<remote_id>},"list_id":<track.status>,"status":{"chapter":<last_chapter_read.toInt()>}}]` | then `updateSeriesRating` |
| delete (`:103-115`) | `POST /v1/lists/series/delete` | `[<remoteId>]` | JSON array of ids |
| set rating (`updateSeriesRating`, `:128-148`) | `PUT /v1/series/{remote_id}/rating` body `{"rating":<score double>}`; `DELETE /v1/series/{remote_id}/rating` when score is exactly `0.0`; nothing when score `< 0.0` | | |
| search (`:149-174`) | `POST /v1/series/search` (UNauthenticated shared client) | `{"search":"<query>","filter_types":["drama cd","novel"]}` | response `{"results":[{"record":{...}}]}`; no `page`/`perpage` sent, so the server default applies |

Search record fields consumed (`dto/MURecord.kt`): `series_id`, `title` (HTML-decoded), `url`, `description` (HTML-decoded), `image.url.original`, `type`, `year`, `bayesian_rating`, `rating_votes`, `latest_chapter`. `total_chapters` is always `0`, `publishing_status` is empty, `publishing_type = type.toString()`, `start_date = year.toString()` (so a null year becomes the literal string `"null"`; observed quirk).

Status model (`MangaUpdates.kt:29-34`): `READING_LIST = 0`, `WISH_LIST = 1`, `COMPLETE_LIST = 2`, `UNFINISHED_LIST = 3`, `ON_HOLD_LIST = 4` (these are the MangaUpdates list ids, used verbatim as track status). Status order shown in the UI: reading, complete, on hold, unfinished, wish list. `getRereadingStatus() = -1`. Behaviour: `update` sets `READING_LIST` when the list is not `COMPLETE_LIST` and a chapter was read, then calls the update endpoint; completion is never auto-set. `bind`: try `getSeriesListItem`; on any exception fall back to `score = 0.0` + `addSeriesToList`. Score UI list: `"-"`, then `0.0..9.9` step 0.1 grouped by whole number (`0.0`..`0.9`, `1.0`..`9.9`), `"10.0"`; `indexToScore(0) = 0.0`, otherwise the parsed list entry. There are no reading dates and no private flag.

Keep? **Keep**. Proposal (`src/track/mangaupdates.rs`): same endpoints; store the session token in the Secret Service; never persist the password; map HTTP 401 to a "log in again" error. Fix `start_date` to be empty when `year` is null. Verify the `/v1/lists/series/update` semantics against the live API docs (https://api.mangaupdates.com/) before shipping.

Test advice: live probe 2026-10-08: `curl -s -X POST https://api.mangaupdates.com/v1/series/search -H 'Content-Type: application/vnd.api+json' -d '{"search":"berserk","filter_types":["drama cd","novel"]}'` returned HTTP 200 JSON. Authenticated endpoints need an account: use `wiremock` to assert the array-shaped bodies above.

---

### 2.15 `material.io`

Role: `https://material.io/design/layout/understanding-layout.html` in the KDoc of `presentation-core/src/main/java/tachiyomi/presentation/core/components/material/Scaffold.kt:53`; the same host also appears through `m3.material.io` (section 2.13). Doc-only, never contacted. Proposal: n/a. Test: none.

---

### 2.16 `mullvad.net`

Role: credit comment for DoH provider 9 (`DohProviders.kt:132`, "Source: https://mullvad.net/en/help/dns-over-https-and-dns-over-tls"). Endpoint `https://dns.mullvad.net/dns-query` written as `" https://dns.mullvad.net/dns-query"` with a leading space (`:136`; OkHttp's `toHttpUrl()` trims it, a Rust port must not copy the space). Bootstrap `194.242.2.2`, `2a07:e340::2`. Documented in section 2.17. Keep? **Keep-opt**. Test: live probe 2026-10-08 HTTP 200 `application/dns-message`.

---

### 2.17 DoH provider table (shared by `libredns.gr`, `mullvad.net`, `shecan.ir`)

OBSERVED, `core/common/src/main/java/eu/kanade/tachiyomi/network/DohProviders.kt`, wired in `NetworkHelper.kt:42-57` and the settings dialog `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:261-282` (title string `pref_dns_over_https`; changing it shows a toast `requires_app_restart`, the client is built once at start). Preference key `doh_provider` (`core/common/src/main/java/eu/kanade/tachiyomi/network/NetworkPreferences.kt:15-17`), type `Int`, default `-1` (UI label `disabled`); any value outside the table behaves as disabled. Migration `DOHMigration` (`app/src/main/java/mihon/core/migration/migrations/DOHMigration.kt`, version `57f`) turned the old boolean `enable_doh` into `PREF_DOH_CLOUDFLARE` (1) and removed the old key.

Mechanism: OkHttp `DnsOverHttps` with `bootstrapDnsHosts` (so the DoH host itself is resolved from fixed IPs, avoiding a chicken-and-egg lookup), RFC 8484 over HTTPS.

| Value | UI label | DoH URL | Bootstrap IPs |
| --- | --- | --- | --- |
| -1 | Disabled | none | none |
| 1 `PREF_DOH_CLOUDFLARE` | Cloudflare | `https://cloudflare-dns.com/dns-query` | `162.159.36.1`, `162.159.46.1`, `1.1.1.1`, `1.0.0.1`, `162.159.132.53`, `2606:4700:4700::1111`, `2606:4700:4700::1001`, `2606:4700:4700::0064`, `2606:4700:4700::6400` |
| 2 `PREF_DOH_GOOGLE` | Google | `https://dns.google/dns-query` | `8.8.4.4`, `8.8.8.8`, `2001:4860:4860::8888`, `2001:4860:4860::8844` |
| 3 `PREF_DOH_ADGUARD` | AdGuard | `https://dns-unfiltered.adguard.com/dns-query` (the "Unfiltered" resolver, chosen so no site is blocked) | `94.140.14.140`, `94.140.14.141`, `2a10:50c0::1:ff`, `2a10:50c0::2:ff` |
| 4 `PREF_DOH_QUAD9` | Quad9 | `https://dns.quad9.net/dns-query` | `9.9.9.9`, `149.112.112.112`, `2620:fe::fe`, `2620:fe::9` |
| 5 `PREF_DOH_ALIDNS` | AliDNS | `https://dns.alidns.com/dns-query` | `223.5.5.5`, `223.6.6.6`, `2400:3200::1`, `2400:3200:baba::1` |
| 6 `PREF_DOH_DNSPOD` | DNSPod | `https://doh.pub/dns-query` | `1.12.12.12`, `120.53.53.53` |
| 7 `PREF_DOH_360` | 360 | `https://doh.360.cn/dns-query` | `101.226.4.6`, `218.30.118.6`, `123.125.81.6`, `140.207.198.6`, `180.163.249.75`, `101.199.113.208`, `36.99.170.86` |
| 8 `PREF_DOH_QUAD101` | Quad 101 | `https://dns.twnic.tw/dns-query` | `101.101.101.101`, `2001:de4::101`, `2001:de4::102` |
| 9 `PREF_DOH_MULLVAD` | Mullvad | `https://dns.mullvad.net/dns-query` (no ad blocking variant) | `194.242.2.2`, `2a07:e340::2` |
| 10 `PREF_DOH_CONTROLD` | Control D | `https://freedns.controld.com/p0` (unfiltered, source `https://controld.com/free-dns/?`) | `76.76.2.0`, `76.76.10.0`, `2606:1a40::`, `2606:1a40:1::` |
| 11 `PREF_DOH_NJALLA` | Njalla | `https://dns.njal.la/dns-query` (non-logging, uncensored) | `95.215.19.53`, `2001:67c:2354:2::53` |
| 12 `PREF_DOH_SHECAN` | Shecan | `https://free.shecan.ir/dns-query` (credit `https://shecan.ir/`, comment at `DohProviders.kt:176`) | `178.22.122.100`, `185.51.200.2` |
| 13 `PREF_DOH_LIBREDNS` | LibreDNS | `https://doh.libredns.gr/dns-query` | `116.202.176.26`, `2a01:4f8:1c0c:8274::1` |

Keep? **Keep-opt** (default Disabled). Proposal: add `hickory-resolver` (feature `dns-over-https-rustls`) and install it as the `reqwest` DNS resolver via `ClientBuilder::dns_resolver`, with `NameServerConfigGroup::from_ips_https(&ips, 443, "<hostname>", true)` built from the table above, so the bootstrap IPs behave exactly like OkHttp's. Store the choice in the settings key `doh_provider` (same integers so backups restore, area:22-backup-restore). Replace the "requires restart" toast with rebuilding the client on change. Drop none; the user loses nothing. Concern: Quad 101 (`dns.twnic.tw`) and Shecan (`free.shecan.ir`) did not answer from the probe machine on 2026-10-08 (connection failed), which is expected for geo-restricted resolvers; keep them selectable anyway. On Linux also document the alternative of enabling DoH system-wide with `systemd-resolved` `DNSOverTLS=` or the browser; the app option remains useful because it covers only Kurobayomi's traffic.

Test advice: for each provider, `curl -s -m 12 -o /dev/null -w '%{http_code} %{content_type}\n' -H 'accept: application/dns-message' 'https://<host>/<path>?dns=AAABAAABAAAAAAAAB2V4YW1wbGUDY29tAAABAAE'` (the base64url payload is a query for `example.com A`); expect `200 application/dns-message`. Results on 2026-10-08: 200 for Cloudflare, Google, AdGuard, Quad9, AliDNS, DNSPod, 360, Mullvad, Control D, Njalla, LibreDNS; connection failure for Quad 101 and Shecan. Unit-test that every provider entry parses to a URL with no whitespace and a non-empty IP list.

---

### 2.18 `myanimelist.net`

Role: **tracker API** (MyAnimeList). Hosts: OAuth `https://myanimelist.net/v1/oauth2` and REST `https://api.myanimelist.net/v2`; website pages `https://myanimelist.net/manga/<id>` and `https://myanimelist.net/anime/<id>`. Tracker id `1L`. Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/myanimelist/MyAnimeListApi.kt`, `MyAnimeListInterceptor.kt`, `MyAnimeList.kt`, `MyAnimeListUtils.kt`, `dto/*.kt`, `app/src/main/java/eu/kanade/tachiyomi/util/PkceUtil.kt`.

Constants (`MyAnimeListApi.kt`): `CLIENT_ID = "686b980ff4240fccce7f6a654cea07ce"`, `BASE_OAUTH_URL = "https://myanimelist.net/v1/oauth2"` (`:385`), `BASE_API_URL = "https://api.myanimelist.net/v2"` (`:386`), `LIST_PAGINATION_AMOUNT = 250`. Tracking URL: `https://myanimelist.net/manga/$remote_id` (`:140`) and `https://myanimelist.net/anime/$remote_id` (`:171`).

#### 2.18.1 Authentication (authorization code + PKCE "plain")

- `authUrl()` = `https://myanimelist.net/v1/oauth2/authorize?client_id=686b980ff4240fccce7f6a654cea07ce&code_challenge=<verifier>&response_type=code`. The code verifier is `PkceUtil.generateCodeVerifier()`: 50 random bytes (`SecureRandom`) encoded with URL-safe Base64 without padding (67 characters), and the same string is sent as `code_challenge` (PKCE method `plain`, which MAL requires). The verifier is held in a process-global `codeVerifier` variable (lost on app restart, so the login must complete in one session).
- Callback `aniyomi://myanimelist-auth?code=<code>`; `TrackLoginActivity.handleMyAnimeList` calls `MyAnimeList.login(code)`; no `code` means `logout()`.
- Token exchange (`getAccessToken`, `:50-65`): `POST https://myanimelist.net/v1/oauth2/token`, form `client_id`, `code`, `code_verifier`, `grant_type=authorization_code`. Response `MALOAuth`: `token_type`, `refresh_token`, `access_token`, `expires_in` (s), `created_at` (defaults to `now/1000` at decode time and is encoded because of `@EncodeDefault`). `isExpired()` = `created_at + (expires_in - 60) < now_s`.
- `login(authCode)` (`MyAnimeList.kt:251-260`): store token, `GET /users/@me` for `name`, `saveCredentials(name, accessToken)`; failure silently logs out.
- Interceptor (`MyAnimeListInterceptor.kt`): if the preference `pref_tracker_auth_expired_1` is true throw `MALTokenExpired` (`IOException("MAL: Login has expired")`); if the token is expired do a `synchronized` refresh: `POST https://myanimelist.net/v1/oauth2/token` form `client_id`, `refresh_token`, `grant_type=refresh_token` with header `Authorization: Bearer <old access token>`; HTTP 401 sets the expired flag and throws `MALTokenExpired`; other failures throw `MALTokenRefreshFailed` (`IOException("MAL: Failed to refresh account token")`); success stores the new token. Requests then carry `Authorization: Bearer <access>` only (the custom User-Agent line is commented out, so the shared client's default Firefox UA is sent).

#### 2.18.2 REST endpoints (`BASE_API_URL = https://api.myanimelist.net/v2`)

| Operation | Method and URL | Params / body | Fields used |
| --- | --- | --- | --- |
| current user | `GET /users/@me` | none | `name` |
| search manga | `GET /manga?q=<first 64 chars>&nsfw=true` | MAL answers 400 when `q` exceeds 64 chars | `data[].node.id`; then one `getMangaDetails` per hit in parallel (`async`/`awaitAll`); results with `media_type` containing `novel` are filtered out |
| search anime | `GET /anime?q=<first 64 chars>&q=<full query>&nsfw=true` | the code appends `q` twice (bug: duplicate parameter; the full-length value can still trigger the 400) | `data[].node.id`; one `getAnimeDetails` per hit; no novel filter |
| manga details | `GET /manga/<id>?fields=id,title,synopsis,num_chapters,mean,main_picture,status,media_type,start_date` | | `id`, `title`, `synopsis` (default `""`), `num_chapters`, `mean` (default `-1.0`), `main_picture.large`, `status` (underscores to spaces), `media_type` (underscores to spaces), `start_date` |
| anime details | `GET /anime/<id>?fields=id,title,synopsis,num_episodes,mean,main_picture,status,media_type,start_date` | | analogous with `num_episodes` |
| update/add item | `PUT /manga/<id>/my_list_status` (anime `/anime/<id>/my_list_status`), form body | manga: `status`, `is_rereading` (`true`/`false`), `score` (`track.score.toString()`, e.g. `7.0`), `num_chapters_read` (int), optional `start_date`, `finish_date` (`yyyy-MM-dd`, `Locale.US`); anime: `status`, `is_rewatching`, `score`, `num_watched_episodes`, same dates | response `MALListMangaItemStatus` (`is_rereading`, `status`, `num_chapters_read`, `score`, `start_date`, `finish_date`) |
| delete item | `DELETE /manga/<id>/my_list_status` (or anime) | | none |
| find item | `GET /manga/<id>?fields=num_chapters,my_list_status{start_date,finish_date}` (anime: `num_episodes`) | | `num_chapters`/`num_episodes` and `my_list_status`; `null` list status means "not in list" |
| list page (for `my:` search) | `GET /users/@me/mangalist?fields=list_status{start_date,finish_date}&limit=250[&offset=<n>]` (the SAME mangalist path is used for the anime variant, `findListItemsAnime`, which is a bug: anime search over the manga list) | `offset` omitted when 0 | `data[].node.{id,title}`, `paging.next`; recurses with `offset += 250` while `paging.next` is non-blank |

Date quirk: `convertToIsoDate(0)` returns `""` (not null), so a zero date is sent as an empty `start_date=` / `finish_date=` field, which clears the date on MAL; `null` is only returned on a formatting exception.

Status mapping (`MyAnimeListUtils.kt`; constants `MyAnimeList.kt:36-44`): `READING 1` to `reading`; `WATCHING 11` to `watching`; `COMPLETED 2` to `completed`; `ON_HOLD 3` to `on_hold`; `DROPPED 4` to `dropped`; `PLAN_TO_READ 6` to `plan_to_read`; `PLAN_TO_WATCH 16` to `plan_to_watch`; `REREADING 7` to `reading` + `is_rereading=true`; `REWATCHING 17` to `watching` + `is_rewatching=true`. The reverse `getStatus(String?)` defaults to `READING` for unknown values (for anime too).

Search shortcuts (`MyAnimeList.searchManga`, `:209-243`): query `id:<digits>` fetches that id directly; `my:<title>` filters the user's own list by case-insensitive substring and fetches details for each match; everything else is a normal search.

Behaviour (`MyAnimeList.kt:125-206`): same completion/started-date rules as AniList. `refresh` = `findListItem` or `add` (re-add when missing). Scores 0-10 integers (`SCORE_LIST = 0..10`). Supports reading dates; no private flag.

Keep? **Keep**. Proposal (`src/track/myanimelist.rs`): PKCE with the `oauth2` crate (`PkceCodeVerifier` plain/S256: MAL accepts only plain, so build the challenge manually), redirect through the loopback server `http://127.0.0.1:<port>/myanimelist-auth` registered in a Kurobayomi-owned MAL API client (type "other" app; MAL lets you set any redirect URL), or the `aniyomi://` scheme fallback with Aniyomi's client id. Persist the verifier in memory for the session only, as the original. Fix the double `q`, use the anime list path `/users/@me/animelist` for `my:` anime search, and do the parallel detail fetches with `futures::stream::buffer_unordered(8)`. Serialize `score` as an integer. Token refresh is serialized with a `tokio::sync::Mutex`.

Test advice: public read endpoints need only the client id header: `curl -s 'https://api.myanimelist.net/v2/manga?q=berserk' -H 'X-MAL-CLIENT-ID: 686b980ff4240fccce7f6a654cea07ce'` (quote the URL in fish; live probe 2026-10-08 returned HTTP 200). Mock-test the refresh path: first request gets 401 expiry handling (`MALTokenExpired`), concurrent calls trigger exactly one refresh.

---

### 2.19 `mysite.com`

Role: placeholder example in documentation of the extension API: `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/source/online/HttpSource.kt:37` and `source-api/src/commonMain/kotlin/eu/kanade/tachiyomi/animesource/online/AnimeHttpSource.kt:39` ("Base url of the website without the trailing slash, like: http://mysite.com"). Contract it documents: `baseUrl` has no trailing slash. OBSERVED: never contacted. Keep? **Doc-only**; the contract (no trailing slash on `baseUrl`) must be enforced by the Rust source host when it loads a source (strip one trailing `/`). Test: unit test `normalize_base_url("http://mysite.com/") == "http://mysite.com"`.

---

### 2.20 `raw.githubusercontent.com`

Role: **extension repository host** (GitHub raw content). Extension stores are plain HTTP directories with an `index.min.json`; Aniyomi/Mihon community repositories live at `https://raw.githubusercontent.com/<user>/<repo>/<branch>/...`.

OBSERVED:

- `app/src/main/java/mihon/core/migration/migrations/ExternalRepoMigration.kt:16` (migration version `114f`): rewrites every stored manga extension repo entry `<user>/<repo>` into `https://raw.githubusercontent.com/<user>/<repo>/repo` (a short form, from before repos were full URLs, becomes the branch named `repo`) using `sourcePreferences.mangaExtensionRepos().getAndSet { ... }`.
- Details screens (`AnimeExtensionDetailsScreen.kt:81-87`, `MangaExtensionDetailsScreen.kt:82-88`): regex `https://raw.githubusercontent.com/(.+?)/(.+?)/.+` turns a store `indexUrl` into the "Open repo" link `https://github.com/<user>/<repo>`; otherwise the raw `indexUrl` is opened.
- Index fetching (not host-specific, shown because this host is its main use): `GET <repoBaseUrl>/index.min.json` (`app/src/main/java/eu/kanade/tachiyomi/extension/manga/api/MangaExtensionApi.kt:55`, `data/src/main/java/mihon/data/extension/anime/service/AnimeExtensionStoreService.kt:97`); store trust metadata at `<repoBaseUrl>/repo.json` (`AnimeExtensionStoreService.kt:36-39,96`; migration `TrustExtensionRepositoryMigration.kt:23` rewrites `.../index.min.json` or `.../index.json` into `.../repo.json`); repo creation strips the `/index.min.json` suffix (`domain/src/main/java/mihon/domain/extensionrepo/manga/interactor/CreateMangaExtensionRepo.kt:23`); the settings list shows `"${repo.baseUrl}/index.min.json"` (`.../browse/components/manga/MangaExtensionReposContent.kt:102`).
- The request goes through the shared client, so DoH and the 10-minute default cache apply. No auth. GitHub raw has unauthenticated rate limits per IP; on HTTP errors the repo is reported as failed.

Keep? **Keep**. Proposal: the extension repository protocol is kept (area:20-extension-strategy-redesign decides what an "extension" is on Linux; the index and `repo.json` handling must remain compatible so the user can reuse existing repos). Implement `fn repo_web_url(index_url: &str) -> String` with the same regex (`regex` crate) and the same fallback. Port `ExternalRepoMigration`'s rule only inside the Aniyomi-backup import path (area:22-backup-restore), because a fresh Kurobayomi database never holds the short form. Use conditional requests (`ETag` / `If-None-Match`) to be gentler than the Kotlin client. Test advice: `curl -sI https://raw.githubusercontent.com/<user>/<repo>/repo/index.min.json` for an existing repo; unit-test `repo_web_url("https://raw.githubusercontent.com/a/b/repo/index.min.json") == "https://github.com/a/b"` and a non-GitHub URL falling back to itself.

---

### 2.21 `semver.org`

Role: `https://semver.org/spec/v2.0.0.html`, referenced by `CHANGELOG.md:5` ("adheres to Semantic Versioning"). Doc-only. Proposal: Kurobayomi follows semver; the update comparison in section 2.1 uses the `semver` crate. Test: none.

---

### 2.22 `share.camoe.cn`

Role: default BitTorrent tracker, `http://share.camoe.cn:8080/announce`, element 5 of the default tracker list in `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt:19`. See the combined tracker list section 2.33 below (all four default-list hosts `nyaa.tracker.wf`, `tracker.anirena.com`, `share.camoe.cn`, `t.nyaatracker.com` share one mechanism). Keep? **Keep-opt**. Test: tracker liveness is not part of tests (trackers die often); see 2.33.

---

### 2.23 `shecan.ir`

Role: credit comment for DoH provider 12 (`DohProviders.kt:176`, "Source: https://shecan.ir/"). Endpoint `https://free.shecan.ir/dns-query` (`:180`), bootstrap `178.22.122.100`, `185.51.200.2`. Documented in section 2.17. Keep? **Keep-opt**. Test: live probe on 2026-10-08 failed to connect from the probe machine (the service is regional); do not make CI depend on it.

---

### 2.24 `shikimori.one`

Role: **tracker API** (Shikimori, anime and manga). Source: `app/src/main/java/eu/kanade/tachiyomi/data/track/shikimori/ShikimoriApi.kt`, `ShikimoriInterceptor.kt`, `Shikimori.kt`, `ShikimoriUtils.kt`, `dto/*.kt`. Tracker id `4L`.

Constants (`ShikimoriApi.kt:250-266`): `BASE_URL = "https://shikimori.one"`, `API_URL = "$BASE_URL/api"`, `OAUTH_URL = "$BASE_URL/oauth/token"`, `LOGIN_URL = "$BASE_URL/oauth/authorize"`, `REDIRECT_URL = "aniyomi://shikimori-auth"`, `CLIENT_ID = "aOAYRqOLwxpA8skpcQIXetNy4cw2rn2fRzScawlcQ5U"`, plus an embedded `CLIENT_SECRET` (`ShikimoriApi.kt:259`; not reproduced here).

IMPORTANT live finding (probe 2026-10-08): `shikimori.one` now answers with HTTP 301 (GET) or 308 (POST) redirecting to `https://shikimori.io/<same path>` (for `/api/animes`, `/oauth/token`, `/oauth/authorize` and `/api/users/whoami`), served behind DDoS-Guard. OkHttp (and `reqwest`) strip the `Authorization` header on a cross-host redirect, so authenticated Aniyomi requests would arrive unauthenticated at the new host. The Kotlin app is not updated for this. Proposal: make the Shikimori base URL a constant `SHIKIMORI_BASE` in Rust set to `https://shikimori.io`, with a settings-free fallback to `https://shikimori.one` only if `.io` fails at login; confirm the OAuth app's registered redirect and host with the Shikimori docs before release. Cover pictures and tracking URLs are built as `BASE_URL + path`, so they follow the base constant.

#### 2.24.1 Authentication (authorization code, with refresh token)

- Authorize: `https://shikimori.one/oauth/authorize?client_id=<id>&redirect_uri=aniyomi%3A%2F%2Fshikimori-auth&response_type=code`; callback `aniyomi://shikimori-auth?code=<code>` (`TrackLoginActivity.handleShikimori`).
- Token: `POST https://shikimori.one/oauth/token` form `grant_type=authorization_code`, `client_id`, `client_secret`, `code`, `redirect_uri`. Response `SMOAuth`: `access_token`, `token_type`, `created_at` (s), `expires_in` (s), `refresh_token` (nullable). `isExpired()` = `now_s > created_at + expires_in - 3600` (comment: "Access token lives 1 day").
- `Shikimori.login(code)` (`Shikimori.kt:222-232`): stores the token (`track_token_4`), `GET /api/users/whoami` for `id`, `saveCredentials(id.toString(), accessToken)`; failures log out.
- Interceptor: throws `Exception("Not authenticated with Shikimori")` without a token; when expired it posts `grant_type=refresh_token`, `client_id`, `client_secret`, `refresh_token` to `OAUTH_URL` and stores the result when the response is successful (otherwise continues with the old token); adds `Authorization: Bearer <access>` and `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`.

#### 2.24.2 Endpoints

| Operation | Method and URL | Params / body | Fields |
| --- | --- | --- | --- |
| add / update (`addLibManga`/`updateLibManga` are the same call, `:42-82`) | `POST https://shikimori.one/api/v2/user_rates` | `{"user_rate":{"user_id":"<userId>","target_id":<remote_id>,"target_type":"Manga","chapters":<int>,"score":<int>,"status":"<s>"}}` (anime: `"target_type":"Anime"`, `"episodes"`) | response `id` becomes `track.library_id` |
| delete | `DELETE https://shikimori.one/api/v2/user_rates/<libraryId>` | | none |
| search | `GET /api/mangas` or `/api/animes` with `order=popularity`, `search=<query>`, `limit=20` | | list of `SMEntry` |
| find | `GET /api/mangas/<remote_id>` (or `/api/animes/<id>`) then `GET /api/v2/user_rates?user_id=<userId>&target_id=<remote_id>&target_type=Manga` (or `Anime`) | | more than one result throws `Exception("Too many manga in response")` (also for anime); none returns null |
| current user | `GET /api/users/whoami` | | `id` |

Entry fields consumed (`dto/SMEntry.kt`): `id`, `name`, `chapters`, `episodes` (non-null asserted `!!` on the relevant one: a null total crashes the mapping), `image.preview` (prefixed with the base URL), `score` (Double), `url` (prefixed with the base URL for `tracking_url`), `status`, `kind`, `aired_on`. `summary` is always empty. User-rate fields (`dto/SMUserListEntry.kt`): `id`, `chapters`, `episodes`, `score` (Int), `status`; note `remote_id = user_rate.id` in the mapping (the user-rate id replaces the media id), `library_id` also the user-rate id.

Status mapping (`ShikimoriUtils.kt`; constants `Shikimori.kt:37-42`): `READING 1` (also used for anime) to `watching`; `COMPLETED 2` to `completed`; `ON_HOLD 3` to `on_hold`; `DROPPED 4` to `dropped`; `PLAN_TO_READ 5` to `planned`; `REREADING 6` to `rewatching`. Unknown throws `NotImplementedError`. Scores 0-10 integers. Anime and manga share the same constants; the anime branch of `update` and `bind` uses `READING`/`REREADING` (observed shortcut). No reading dates, no private flag, no client-side rate limit (the site limits roughly 5 requests per second and 90 per minute).

Keep? **Keep** (domain updated, see the IMPORTANT note). Proposal (`src/track/shikimori.rs`): one `POST /api/v2/user_rates` for create and `PATCH /api/v2/user_rates/<id>` for update (the Shikimori v2 API supports both; verify), use a Kurobayomi-owned OAuth app with a loopback redirect. Replace the string-templated JSON with `serde_json::json!`. Handle the 301/308 by pinning the base to the final host.

Test advice: live probes 2026-10-08: `curl -si 'https://shikimori.one/api/animes?search=naruto&limit=1' -A kurobayomi` shows HTTP 301 with `location: https://shikimori.io/api/animes?...`; `curl -s -L 'https://shikimori.one/api/animes?search=naruto&limit=1&order=popularity' -A kurobayomi` follows to `shikimori.io` and returns JSON (a `User-Agent` is mandatory). Mock-test the three-step flow add, find (including the "too many" error), delete.

---

### 2.25 `shizuku.rikka.app`

Role: download page for Shizuku (an Android privileged-shell service used as one of the APK installer back-ends for extensions). `app/src/main/java/eu/kanade/presentation/more/settings/screen/SettingsAdvancedScreen.kt:431` opens `https://shizuku.rikka.app/download` from the "Shizuku is not installed" dialog (title string `ext_installer_shizuku`, body `ext_installer_shizuku_unavailable_dialog`, confirm button `action_ok`, cancel `action_cancel`) shown when the user picks the Shizuku installer (the installer picker is the `Preference.PreferenceGroup` titled `label_extensions` in the same file). OBSERVED: the link is only opened in the browser.

Keep? **Drop** (Android-only). Reason: Shizuku grants `adb`-level permission so the app can install APKs silently; Linux has no APK installation and no such permission model. Replacement: whatever extension mechanism area:20-extension-strategy-redesign chooses installs files as the normal user (no elevated service needed), so the installer-choice setting disappears. What the user loses: nothing on Linux. Test: none.

---

### 2.26 `simkl.com`

Role: **tracker API** (Simkl, anime and movies/shows, no manga). Hosts: website and OAuth authorize `https://simkl.com`, API `https://api.simkl.com`, posters `https://simkl.in/posters/` (section 2.27). Tracker id `101L` (`TrackerManager.SIMKL`). Files: `app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt`, `SimklInterceptor.kt`, `Simkl.kt`, `SimklUtils.kt`, `dto/*.kt`.

Constants (`SimklApi.kt:237-246`): `CLIENT_ID = "aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e"` (public, also sent as the `simkl-api-key` header), embedded `CLIENT_SECRET` (`SimklApi.kt:238`; not reproduced here), `BASE_URL = "https://simkl.com"`, `API_URL = "https://api.simkl.com"`, `OAUTH_URL = "$API_URL/oauth/token"`, `LOGIN_URL = "$BASE_URL/oauth/authorize"`, `POSTERS_URL = "https://simkl.in/posters/"`, `REDIRECT_URL = "aniyomi://simkl-auth"`.

Authentication: authorize `https://simkl.com/oauth/authorize?response_type=code&client_id=<id>&redirect_uri=aniyomi%3A%2F%2Fsimkl-auth`; callback `aniyomi://simkl-auth?code=<code>`; `POST https://api.simkl.com/oauth/token` JSON body `{"code","client_id","client_secret","redirect_uri","grant_type":"authorization_code"}`; response `SimklOAuth {access_token}` (no refresh token, no expiry; Simkl tokens do not expire). `Simkl.login(code)`: store token (`track_token_101`), `GET /users/settings` (called with `runBlocking`) for `account.id`, `saveCredentials(id.toString(), accessToken)`; failures log out. Interceptor adds `Authorization: Bearer <token>`, `simkl-api-key: <CLIENT_ID>` and `User-Agent: Aniyomi v<VERSION_NAME> (<APPLICATION_ID>)`; throws `Exception("Not authenticated with Simkl")` without a token.

Endpoints (all JSON, `application/json; charset=utf-8`, authenticated client unless stated):

| Operation | Method and path | Body | Notes |
| --- | --- | --- | --- |
| add to list (`addToList`, `:48-63`) | `POST /sync/add-to-list` | `{"<shows\|movies>":[{"ids":{"simkl":<remote_id>},"to":"<status>"}]}` | `<media>` = `movies` when the first path segment of `tracking_url` is `movies`, otherwise `shows` (anime and tv) |
| rating (`updateRating`, `:64-86`) | `POST /sync/ratings` (or `POST /sync/ratings/remove` when score is exactly `0.0`) | `{"<media>":[{"ids":{"simkl":<id>},"rating":<score.toInt()>}]}` | |
| progress (`updateProgress`, `:87-121`) | `POST /sync/history/remove` then `POST /sync/history` | `{"shows":[{"ids":{"simkl":<id>},"seasons":[{"number":1,"episodes":[{"number":1}, ... {"number":<last_episode_seen>}]}]}]}`; the remove call omits `episodes` (removes the whole season 1) | only for non-movies; it re-uploads every episode number from 1 to `last_episode_seen` on every update |
| update (`updateLibAnime`, `:122-141`) | sequence: progress (non-movies), add-to-list, rating | | |
| search (`:142-161`) | `GET https://api.simkl.com/search/<type>?q=<query>&extended=full&client_id=<CLIENT_ID>` (UNauthenticated shared client) | none | `<type>` is called three times: `anime`, `tv`, `movie`, results concatenated in that order |
| find (`findLibAnime`, `:162-203`) | `POST /sync/watched` body `[{"simkl":<remote_id>}]`; then `GET /sync/all-items/<anime\|shows\|movies>/<status>?date_from=<last_watched>` | | first response element must have `result == true`, `last_watched` and `list` non-null; then scans the returned list for `ids.simkl == remote_id` |
| user (`getCurrentUser`, `:204-214`) | `GET /users/settings` | | `account.id` |

Search result fields (`dto/SimklSearch.kt`): `ids.simkl_id`, `title_romaji` (preferred) else `title`, `ep_count` (default 1), `poster` (cover = `https://simkl.in/posters/<poster>_m.webp`), `all_titles` (joined with newline under the prefix `All titles:\n` as the summary), `url` (relative like `/anime/39508/naruto`, stored as `tracking_url`), `status` (default `ended`), `type` (fallback is the queried type), `year`. Live probe 2026-10-08 confirmed `url` is relative (`"/anime/39508/naruto"`) and that movie results carry `endpoint_type: "movies"`. Library entries (`dto/SimklSyncItem.kt`): `show`/`movie` `{title, ids.simkl}`, `total_episodes_count`, `watched_episodes_count`, `user_rating`; movies are modelled as 1 episode watched when the status is `completed`; `tracking_url = "/$type/$simklId"`.

Status mapping (`SimklUtils.kt`; constants `Simkl.kt:23-27`): `WATCHING 1` to `watching`; `COMPLETED 2` to `completed`; `ON_HOLD 3` to `hold`; `NOT_INTERESTING 4` to `notinteresting` (the reverse maps both `dropped` and `notinteresting` to 4); `PLAN_TO_WATCH 5` to `plantowatch`. Scores `0..10` integers. `getRewatchingStatus() = 0` (no rewatch).

Behaviour: `update` sets `COMPLETED` when `last == total && total > 0`, else `WATCHING`. `bind`: `findLibAnime`; found copies personal fields and runs `update`; else status `WATCHING` or `PLAN_TO_WATCH`, score 0, `add`. `refresh` copies personal fields and total; when not found it returns the track unchanged. No rate limit is applied by the client (Simkl documents about 1 write per second as polite).

Keep? **Keep**. Proposal (`src/track/simkl.rs`): same endpoints; stop deleting and re-adding full history each update: call `/sync/history` only with the delta (episodes greater than the previously synced `last_episode_seen`) and use `/sync/history/remove` only when progress decreases. Use a Kurobayomi-owned client id with a loopback redirect. The user loses nothing.

Test advice: live probes 2026-10-08: `curl -s 'https://api.simkl.com/search/anime?q=naruto&extended=full&client_id=aa62a7da32518aae5d5049a658b87fa4837c3b739e06ed250b315aab6af82b0e'` returned HTTP 200 JSON with the fields above. Mock-test the three-call update sequence and the movie-versus-show body difference.

---

### 2.27 `simkl.in`

Role: Simkl static CDN with two uses.

1. Posters: `https://simkl.in/posters/<poster>_m.webp` (`app/src/main/java/eu/kanade/tachiyomi/data/track/simkl/SimklApi.kt:244`; used for search result covers in `dto/SimklSearch.kt`). `poster` values look like `11/11151378cbbf3e93e9`. Plain image GET through the image loader.
2. Airing calendar JSON: `GET https://data.simkl.in/calendar/<anime|tv|movie_release>.json` (`app/src/main/java/eu/kanade/tachiyomi/util/AniChartApi.kt:116`), bare client, no auth. Live probe 2026-10-08 returned HTTP 200 JSON: an array of `{"title","poster","date":"2026-10-07T00:00:00+09:00","release_date","rank","ratings":{...},"url","ids":{"simkl_id":<n>,"slug","tmdb","mal":"<id>"},"episode":{"episode":14,"url":...}}`.

OBSERVED consumption (`getSimklAiringEpisodeData`, `AniChartApi.kt:109-160`, with `removeAiredSimkl` and `toUnixTimestamp`): for each calendar type in the order `anime`, `tv`, `movie_release` the whole body is fetched as a string. `removeAiredSimkl` splits the string on the literal `"date":"`, finds the index of the first element whose ISO offset date-time (`OffsetDateTime.parse`) is in the future, and then calls `body.substring(index)` (the index counts elements, not characters, so it cuts the text at an arbitrary position: observed bug). The code then looks for the substring `"simkl_id":<id>,`. If that fragment is followed by `"mal":"<digits>"`, it converts the MAL id to an AniList id and returns AniList's `nextAiringEpisode` data. Otherwise it reads `"episode":<n>` and the preceding `"date":"..."` and returns `(episode, epochSecond)` when the date is non-blank. Network failure returns `(1, 0L)`.

Keep? **Keep**. Proposal: fetch each calendar once per app run (cache 30 minutes in memory), parse with `serde_json` into `Vec<CalendarItem>`, filter `ids.simkl_id == id`, pick the first item with `date > now`; prefer the AniList path when `ids.mal` is present (same behaviour, but correct parsing). Show posters through the shared image cache with the `_m.webp` suffix (`gdk-pixbuf` webp loader, `webp-pixbuf-loader` package on Arch; area:34-images-covers-cache). Test advice: `curl -s https://data.simkl.in/calendar/anime.json | jq '.[0]'`; unit-test the parser against a stored fixture saved from that call.

---

### 2.28 `simpleicons.org`

Role: provenance comment, `presentation-core/src/main/java/tachiyomi/presentation/core/icons/CustomIcons.kt:4` ("Icons imported from https://simpleicons.org using ..."), for the Discord/GitHub/Reddit/Twitter style icons on the About screen. Doc-only. Proposal: ship the needed brand glyphs as SVG assets in `data/icons/` (license CC0 per Simple Icons) and load them with `gtk::Image::from_resource`. Test: none.

---

### 2.29 `source.chromium.org`

Role: provenance comment, `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/WebViewInterceptor.kt:94` ("Based on IsRequestHeaderSafe in https://source.chromium.org/chromium/chromium/src/+/main:services/network/public/cpp/header_util.cc"). It explains `isRequestHeaderSafe(name, value)` (`:95-103`): lowercase name and value; unsafe when the name is in `unsafeHeaderNames` or starts with `proxy-`, and when `connection: upgrade`. `unsafeHeaderNames` (`:102-112`) is exactly `content-length`, `host`, `trailer`, `te`, `upgrade`, `cookie2`, `keep-alive`, `transfer-encoding`, `set-cookie`, plus the `proxy-` name prefix. Used to decide which request headers are copied into the Android WebView when solving a Cloudflare challenge.

Keep? **Replace**. Proposal: the Cloudflare challenge solver on Linux uses a `webkit6`/WebKitGTK `WebView` (area:12-network-stack, "cloudflare bypass"); the same header filter is needed when replaying request headers into WebKit. Re-implement the filter from the Chromium list verbatim (copy the nine `unsafeHeaderNames` entries above into a Rust `const`). Test: unit-test that `Host`, `Content-Length`, `Proxy-Authorization` and `Connection: Upgrade` are rejected and `Referer` is allowed.

---

### 2.30 `square.github.io`

Role: documentation links: `https://square.github.io/okhttp/4.x/okhttp/okhttp3/-interceptor/` in `core/common/src/main/java/eu/kanade/tachiyomi/network/interceptor/UncaughtExceptionInterceptor.kt:13`, and `https://square.github.io/leakcanary/` in `app/build.gradle.kts:300` (memory-leak detector dependency). The interceptor documented there is described in section 1 (rethrows exceptions as `IOException`, must be first in the chain). Doc-only. Proposal: Rust has no equivalent need (errors are `Result`); LeakCanary is dropped (Android debug tool); the user loses nothing. Test: none.

---

### 2.31 `stackoverflow.com`

Role: provenance comments, never contacted:

- `app/src/main/AndroidManifest.xml:113` (`https://stackoverflow.com/a/31028507`, about an activity configuration choice),
- `app/src/main/java/eu/kanade/tachiyomi/ui/browse/anime/extension/details/AnimeSourcePreferencesScreen.kt:81` and `.../manga/extension/details/MangaSourcePreferencesScreen.kt:81` (`https://stackoverflow.com/questions/60520145/fragment-container-in-jetpack-compose/70817794#70817794`, hosting an Android `PreferenceFragment` inside Compose for extension-provided preference screens),
- `app/src/main/java/eu/kanade/tachiyomi/ui/main/MainActivity.kt:168` (`http://stackoverflow.com/questions/16283079`, launcher intent duplicate-activity workaround),
- `core/common/src/main/java/eu/kanade/tachiyomi/util/system/WebViewUtil.kt:104` (`https://stackoverflow.com/a/29218966`, WebView version parsing),
- `data/src/main/java/tachiyomi/data/release/ReleaseServiceImpl.kt:56` (`https://stackoverflow.com/a/30281147`, the GitHub username regex described in section 2.1).

Keep? **Doc-only**. The only behaviour worth carrying over is "extension-provided preference screens": Proposal: render source preferences natively as GTK widgets from a declarative preference schema (switch, list, multi-select, edit text) in area:20-extension-strategy-redesign. Test: none.

---

### 2.32 `support.google.com`

Role: `https://support.google.com/googleplay/answer/1727131?hl=en` ("Google's official device list"), `core/common/src/main/java/tachiyomi/core/common/util/system/ImageUtil.kt:618`, the source of the model names in the `HARDWARE_BITMAP_UNSUPPORTED` blocklist (Android API 26 devices with broken hardware bitmaps). Doc-only; Android-only workaround. Keep? **Drop**: GTK image decoding uses `gdk-pixbuf`/`image` crate on the CPU or GPU texture upload; no device blocklist exists. The user loses nothing. Test: none.

---

### 2.33 Default BitTorrent trackers: `nyaa.tracker.wf`, `tracker.anirena.com`, `share.camoe.cn`, `t.nyaatracker.com`

Role: default announce URLs for the embedded torrent streaming server (TorrServer). OBSERVED, `core/common/src/main/java/aniyomi/core/common/torrent/TorrentPreferences.kt:11-34`:

- Preference `pref_torrserver_tackers` (sic, typo in the key), type String, default is the newline-joined list below with all spaces removed (`.replace(" ", "")`), in this order:
  1. `http://nyaa.tracker.wf:7777/announce` (host `nyaa.tracker.wf`)
  2. `http://anidex.moe:6969/announce`
  3. `http://tracker.anirena.com:80/announce` (host `tracker.anirena.com`)
  4. `udp://tracker.uw0.xyz:6969/announce`
  5. `http://share.camoe.cn:8080/announce` (host `share.camoe.cn`)
  6. `http://t.nyaatracker.com:80/announce` (host `t.nyaatracker.com`)
  7. `udp://47.ip-51-68-199.eu:6969/announce`
  8. `udp://9.rarbg.me:2940`
  9. `udp://9.rarbg.to:2820`
  10. `udp://exodus.desync.com:6969/announce`
  11. `udp://explodie.org:6969/announce`
  12. `udp://ipv4.tracker.harry.lu:80/announce`
  13. `udp://open.stealth.si:80/announce`
  14. `udp://opentor.org:2710/announce`
  15. `udp://opentracker.i2p.rocks:6969/announce`
  16. `udp://retracker.lanta-net.ru:2710/announce`
  17. `udp://tracker.cyberia.is:6969/announce`
  18. `udp://tracker.dler.org:6969/announce`
  19. `udp://tracker.ds.is:6969/announce`
  20. `udp://tracker.internetwarriors.net:1337`
  21. `udp://tracker.openbittorrent.com:6969/announce`
  22. `udp://tracker.opentrackr.org:1337/announce`
  23. `udp://tracker.tiny-vps.com:6969/announce`
  24. `udp://tracker.torrent.eu.org:451/announce`
  25. `udp://valakas.rollo.dnsabr.com:2710/announce`
  26. `udp://www.torrent.eu.org:451/announce`
- Related preferences: `pref_torrserver_enable` (Boolean, default false), `pref_torrserver_shownotice` (Boolean, false), `pref_torrserver_port` (String, `"8090"`), `pref_torrserver_proxymode` (enum `ProxyMode`, default `None`), `pref_torrserver_proxyurl` (String, empty). `ProxyMode` values: `None("tracker")` (quirk: the value string for `None` is `tracker`, same as `Tracker`), `Tracker("tracker")`, `Peers("peers")`, `Full("full")`.
- Use (`core/common/src/main/java/aniyomi/core/common/torrent/TorrentServerUtils.kt:11-19`): at start of the server service (`app/src/main/java/eu/kanade/tachiyomi/data/torrent/service/TorrentServerService.kt:71-80`) the list is split on `\n`, re-joined with `,\n` and passed to `TorrServer.addTrackers(...)` (library `xyz.secozzi.torrserver`). The player then streams from `http://127.0.0.1:<port>/stream/<urlencoded file name>?link=<info hash>&index=<file index>&play` (`TorrentServerUtils.getTorrentPlayLink`). The app never contacts the tracker hosts itself; TorrServer announces to them (UDP/HTTP) when a torrent is added.
- Several of these are long dead (for example `9.rarbg.*`, `tracker.uw0.xyz`); the app does not prune.

Keep? **Keep-opt** (anime torrent streaming is a P2 feature; the user's GTK build may skip it). Proposal: Linux replacement for the Android-embedded TorrServer is a managed external process: run the `TorrServer` Go binary (AUR `torrserver`) as a child process on `127.0.0.1:<port>` and POST the tracker list via its settings API, or use `libtorrent-rasterbar` through `librqbit` (pure Rust BitTorrent client crate with an HTTP streaming API) which needs no external binary; area:13-core-util-storage-archive-torrent decides. Ship the same 26-entry list as the default but filter hostnames that fail to resolve at first run and surface the editable text setting. The user loses nothing; the Android foreground-service and notification parts (area:53-platform-manifest-intents-workers) are dropped.

Test advice: `curl -I http://nyaa.tracker.wf:7777/announce` (HTTP trackers answer with a bencoded failure message to an announce without parameters; any response proves liveness; UDP trackers need a UDP connect packet, test them with `librqbit` or skip). Unit-test that the default string has 26 non-empty lines with no spaces.

---

### 2.34 `t.nyaatracker.com`

Role: default BitTorrent tracker `http://t.nyaatracker.com:80/announce`, element 6 of the default tracker list (`TorrentPreferences.kt:20`). Full mechanism, list and decision are in section 2.33. Keep? **Keep-opt**. Test: see 2.33.

---

### 2.35 `tracker.anirena.com`

Role: default BitTorrent tracker `http://tracker.anirena.com:80/announce`, element 3 of the default tracker list (`TorrentPreferences.kt:17`). Full mechanism, list and decision are in section 2.33. Keep? **Keep-opt**. Test: see 2.33.

---

### 2.36 `wsrv.nl`

Role: **image proxy for the "data saver"** (one of three back-ends). OBSERVED, `app/src/main/java/aniyomi/util/DataSaver.kt` (the whole algorithm is documented here because `wsrv.nl` is the host that the setting is named after; label string `wsrv`, text `wsrv.nl` in every locale's `i18n-aniyomi/src/commonMain/moko-resources/<locale>/strings.xml`).

Preferences (`app/src/main/java/eu/kanade/domain/source/service/SourcePreferences.kt:97-122`): `data_saver` enum `DataSaver {NONE, BANDWIDTH_HERO, WSRV_NL, RESMUSH_IT}` default `NONE`; `ignore_jpeg` Boolean default `false`; `ignore_gif` Boolean default `true`; `data_saver_image_quality` Int default `80`; `data_saver_image_format_jpeg` Boolean (JPEG output instead of WebP; default is defined at `SourcePreferences.kt:105-108`); `data_saver_server` String default `""` (Bandwidth Hero only); `data_saver_color_bw` Boolean default `false` (Bandwidth Hero only); `data_saver_excluded` String set of source ids, default empty; `data_saver_downloader` Boolean default `true` (apply the saver to chapter downloads too).

Selection (`DataSaver(source, preferences)`): if the preference is not `NONE` and the source id is in `data_saver_excluded`, no saver; else the chosen implementation. Usage: `HttpSource.getImage(page, dataSaver)` temporarily replaces `page.imageUrl` with the compressed URL for the request, then restores it (`DataSaver.kt:21-30`); called from the reader (`app/src/main/java/eu/kanade/tachiyomi/ui/reader/loader/HttpPageLoader.kt:51,196`) and the downloader (`app/src/main/java/eu/kanade/tachiyomi/data/download/manga/MangaDownloader.kt:374-377,402,455,486,516,521`, only when `data_saver_downloader` is true). Anime has no data saver.

The wsrv.nl URL builder (`WsrvNlDataSaver.getUrl`, `DataSaver.kt:115-136`): `GET https://wsrv.nl/?url=<imageUrl>` (note: the image URL is concatenated WITHOUT URL-encoding, so an image URL with its own query string or `&` breaks the parameters), followed by:

- if the image URL contains `.webp` or `.gif` (case-insensitive): when output is not JPEG `&q=<quality>&n=-1`; when JPEG is requested `&output=jpg&q=<quality>&n=-1` (`n=-1` keeps all animation frames);
- otherwise: JPEG requested `&output=jpg&q=<quality>`; else `&output=webp&q=<quality>`.

Skip rules shared by all three back-ends: `.jpeg`/`.jpg` (case-insensitive substring) is left unchanged when `ignore_jpeg`; `.gif` is left unchanged when `ignore_gif`; everything else is proxied.

Sibling back-ends, for completeness (same file):

- Bandwidth Hero (`BandwidthHeroDataSaver`): requires a non-blank `data_saver_server` (trailing `/` trimmed) and skips URLs that already contain that server; URL = `<server>/?jpg=<0|1>&l=<quality>&bw=<0|1>&url=<url-encoded imageUrl>` (`jpg` = JPEG format flag, `bw` = grayscale flag).
- reSmush.it (`ReSmushItDataSaver`): performs a synchronous `GET http://api.resmush.it/ws.php?img=<imageUrl>&qlty=<quality>` through the shared client and string-scrapes `substringAfter("\"dest\":\"").substringBefore("\",")` as the new URL (note plain HTTP). It ignores the JPEG/WebP format switch (the UI disables that switch for reSmush.it, `SettingsAdvancedScreen.kt:545`). Note: JSON escapes in `dest` such as `\/` are not unescaped by the scraper (observed bug).

UI (`SettingsAdvancedScreen.kt:480-555`): list `data_saver` (entries labelled `none`/`bandwidth_hero`/`wsrv`/`resmush` strings), edit text `data_saver_server` enabled only for Bandwidth Hero, switches enabled whenever the saver is not `NONE`, image-quality slider, format switch (disabled for `NONE` and `RESMUSH_IT`), colour B/W switch enabled only for Bandwidth Hero. Per-source exclusion is toggled from the source list (`app/src/main/java/eu/kanade/domain/source/manga/interactor/ToggleExcludeFromMangaDataSaver.kt`, `.../browse/manga/MangaSourcesScreen.kt`).

Keep? **Keep-opt** (Manga reader only; default off). Proposal: implement `DataSaver` as a trait with `NoOp`, `Wsrv`, `BandwidthHero`, `ReSmush`; build wsrv URLs with `url::Url::query_pairs_mut()` so the image URL IS percent-encoded (deliberate fix; wsrv.nl accepts encoded `url`), keep `n=-1` for animated images; use `serde_json` for reSmush responses and prefer `https://api.resmush.it/ws.php`. This is a privacy trade-off (third party sees the images you read); show that in the setting description. The user keeps all functionality. Local-network alternative: none needed.

Test advice: live probes 2026-10-08: `curl -s 'https://wsrv.nl/?url=https://example.com/a.jpg&output=webp&q=80'` returns the JSON error `{"status":"error","code":404,...}` for a non-existent origin image, which confirms the URL shape is accepted (use a real public image to verify a 200 image response). `curl -s 'http://api.resmush.it/ws.php?img=<url>&qlty=80'` returned HTTP 200 JSON with `error`/`error_long` fields for an unfetchable origin, and the `dest` field on success. Unit-test the four wsrv query cases (webp input, gif input, jpg-forced output, default webp) and the three skip rules.

---

### 2.37 `www.contributor-covenant.org`

Role: attribution in `CODE_OF_CONDUCT.md:117,119,125,126` (`https://www.contributor-covenant.org/`, `/version/2/1/code_of_conduct.html`, `/faq`, `/translations`). Doc-only. Proposal: Kurobayomi may reuse the Contributor Covenant 2.1 text; no runtime. Test: none.

---

### 2.38 `www.nordtheme.com`

Role: credit for the Nord palette: `https://www.nordtheme.com/docs/colors-and-palettes` in `app/src/main/java/eu/kanade/presentation/theme/colorscheme/NordColorScheme.kt:9`, `presentation-core/src/main/res/values/colors_nord.xml:2` and `presentation-core/src/main/res/values-night/colors_nord.xml:2`. Doc-only, never contacted. Keep? **Doc-only** (the GTK theme replaces app palettes; if a Nord CSS is desired it is a user theme, not app code). Test: none.

---

### 2.39 `youtrack.jetbrains.com`

Role: Kotlin compiler issue links in build files: `https://youtrack.jetbrains.com/issue/KT-66995/JvmTarget-and-JavaVersion-compatibility-for-easier-JVM-version-setup` (`buildSrc/src/main/kotlin/mihon/buildlogic/AndroidConfig.kt:13`) and `https://youtrack.jetbrains.com/issue/KT-57605` (`source-api/build.gradle.kts:29`, "Workaround for ..."). Doc-only; Gradle/Kotlin build concerns that do not exist in a Cargo workspace. Test: none.

## 3. Cross-cutting facts that belong to several rows

### 3.1 Tracker local data model implied by the HTTP layer

Every tracker row above maps onto one `tracks` row per (entry, tracker) with fields (`app/src/main/java/eu/kanade/tachiyomi/data/database/models/{manga,anime}/*Track.kt`, copy rules in `copyPersonalFrom`): `remote_id`, `library_id` (nullable), `title`, `last_chapter_read` / `last_episode_seen` (Double), `total_chapters` / `total_episodes` (Long), `status` (Long, the tracker's own constant), `score` (Double, tracker's native scale; AniList stores 0-100), `tracking_url`, `started_*_date`, `finished_*_date` (epoch millis, 0 = unset), `private` (Boolean). `copyPersonalFrom(other, copyRemotePrivate)` copies progress, score, status, both dates and (unless told not to) the private flag. AniList, Kitsu and Shikimori `bind` use `copyRemotePrivate = false` where they pass it. Owned by area:03-db-manga-schema-migrations and area:27-tracking-core.

### 3.2 Custom URL scheme and OAuth redirect on Linux

`AndroidManifest.xml:243-258` registers `aniyomi://myanimelist-auth`, `aniyomi://anilist-auth`, `aniyomi://bangumi-auth`, `aniyomi://shikimori-auth`, `aniyomi://simkl-auth` for `TrackLoginActivity` (`BROWSABLE`). Proposal for Kurobayomi: a single `LoginCallbackServer` in Rust (`tokio` + `hyper`/`axum`, binds `127.0.0.1:0`, serves `/<tracker>-auth`, shuts down after one request or 5 minutes), opens the authorize URL with `gtk::UriLauncher`, and for AniList's fragment token serves a one-line HTML page that re-posts `location.hash`. A `.desktop` entry with `MimeType=x-scheme-handler/aniyomi;` is the fallback for clients already registered with `aniyomi://` redirects. Owned by area:27-tracking-core.

### 3.3 Credential storage

OBSERVED: tokens and credentials sit in private preferences: `pref_mangasync_username_<id>`, `pref_mangasync_password_<id>`, `pref_tracker_auth_expired_<id>`, `track_token_<id>` (`app/src/main/java/eu/kanade/domain/track/service/TrackPreferences.kt:14-35`), plus `anilist_score_type` (default `POINT_10`) and `pref_auto_update_manga_sync_key` (default `true`). Proposal: Secret Service (`oo7`) for the two secret slots and `track_token_*`; the rest in the settings database.

### 3.4 Verification checklist for this chunk (run before enabling each tracker)

1. AniList: unauthenticated `Media(idMal:1,type:ANIME){id}` returns HTTP 200 (verified 2026-10-08).
2. AniSkip: `GET https://api.aniskip.com/v2/skip-times/21/1?types[]=op&episodeLength=0` returned HTTP 200 `{"found":true,"results":[{"interval":{"startTime":28.783,"endTime":118.783},"skipType":"op","skipId":"...","episodeLength":1500.04}],"message":"Successfully found skip times","statusCode":200}` (verified 2026-10-08). The request in the app also asks for `types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=recap` and passes `episodeLength=<player duration seconds, integer>`; AniSkip returns HTTP 404 when no matching skip data exists (that is a normal "not found").
3. Kitsu: `GET https://kitsu.app/api/edge/algolia-keys/media/` returns HTTP 200 (verified).
4. Simkl: `GET https://api.simkl.com/search/anime?...&client_id=<id>` and `GET https://data.simkl.in/calendar/anime.json` return HTTP 200 (verified).
5. MangaUpdates: `POST https://api.mangaupdates.com/v1/series/search` returns HTTP 200 (verified).
6. Shikimori: base host redirects to `shikimori.io` (verified, section 2.24); pick the final host before coding.
7. GitHub: `GET https://api.github.com/repos/aniyomiorg/aniyomi/releases/latest` returns HTTP 200 (verified).

### 3.5 AniSkip (player skip-intro) full flow, because it spans `graphql.anilist.co` and a sibling host

OBSERVED:

- Trigger (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt:1290-1307`): when intro skipping is enabled (`introSkipEnabled`), the preference `aniSkipEnabled()` is on, and not (`disableAniSkipOnChapters()` is on AND the video already has chapters), the app calls `viewModel.aniSkipResponse(player.duration)` inside `runBlocking`, merges the returned timestamps into the chapter list with `ChapterUtils.mergeChapters(currentChapters, stamps, duration)` and re-selects the chapter for the current position.
- `aniSkipResponse` (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerViewModel.kt:2027-2058`): returns null when there is no current anime, no episode number, or the anime has no tracks (log text `AniSkip: No tracks found for anime <id>`); it walks the tracks but returns on the FIRST track (the `return` inside `map` ends the loop on iteration one): MAL track gives `track.remoteId` as the MAL id; AniList track resolves it with `getMalIdFromAL(track.remoteId)`; any other tracker gives null; it needs a non-null `playerDuration`. Episode number is `episode_number.toInt()` (so 12.5 becomes 12).
- `AniSkipApi.getResult` (`AniSkipApi.kt:22-44`): `GET https://api.aniskip.com/v2/skip-times/<malId>/<episodeNumber>?types[]=ed&types[]=mixed-ed&types[]=mixed-op&types[]=op&types[]=recap&episodeLength=<seconds>` using the bare client, body decoded as `AniSkipResponse { found: Boolean, results: List<Stamp>? }`, `Stamp { interval { startTime: Double, endTime: Double }, skipType }`. When `found` is true, each result becomes `TimeStamp(start, end, name, type)`; names `Opening`, `Ending`, `Recap`, `Mixed-op`; chapter types `Opening`, `Ending`, `Recap`, `MixedOp`. Any exception returns null.
- Enum quirk: `SkipType` has only `op`, `ed`, `recap`, `mixed-op`. The request asks for `mixed-ed` as well, and the Json has no `coerceInputValues`; a response containing `skipType":"mixed-ed"` fails to decode, the `catch` returns null and ALL skip data for that episode is lost. Proposal: Rust enum with `#[serde(rename = "mixed-ed")] MixedEd` mapped to a chapter type `MixedEd` (add it to the player's chapter-type enum and the "skip" button label) and `#[serde(other)] Other` to survive future types. Sibling host: `api.aniskip.com` (chunk 1 or `plans/catalogs/network-endpoints-01.md` covers the host row; this section is the behavioural spec).
- Keep? **Keep** (P2). Test: wiremock fixture with `mixed-ed` present; assert that all other stamps are still returned.
