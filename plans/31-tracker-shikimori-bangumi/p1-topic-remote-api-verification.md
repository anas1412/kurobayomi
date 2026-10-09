# Topic: remote API facts used by Shikimori and Bangumi, and how each was checked

Purpose: the Kotlin source alone cannot say what the servers return, so each server-side fact that the plan depends on is listed here with its source and a status. Statuses:
- **Verified (source)**: read in the server's own source or spec.
- **Verified (doc)**: read in the provider's published documentation.
- **Not verified**: needs a live account. Tracked as T-31a-046.

Read-only research only. Nothing was sent to either service. Downloaded files were read in a scratch folder and removed after use; each entry cites the upstream path so it can be re-read.

## 1. Sources

| Short name | Upstream location | Retrieved by |
|---|---|---|
| Shikimori controller | `shikimori/shikimori`, `app/controllers/api/v2/user_rates_controller.rb` | `gh api repos/shikimori/shikimori/contents/<path>` (paths as in the table) |
| Shikimori v1 controller | `shikimori/shikimori`, `app/controllers/api/v1/user_rates_controller.rb` | same |
| Shikimori model | `shikimori/shikimori`, `app/models/user_rate.rb` | same |
| Shikimori Doorkeeper config | `shikimori/shikimori`, `config/initializers/doorkeeper.rb` | same |
| Bangumi OpenAPI | `bangumi/api`, `open-api/v0.yaml` | `gh api repos/bangumi/api/contents/open-api/v0.yaml` |
| Bangumi auth doc | `bangumi/api`, `docs-raw/How-to-Auth.md` | `gh api repos/bangumi/api/contents/docs-raw/How-to-Auth.md` |
| Bangumi user-agent doc | `bangumi/api`, `docs-raw/user agent.md` | same |
| Bangumi search doc (legacy) | `bangumi/api`, `docs-raw/Search-API.md` | same (used only to note the legacy API, not the v0 one) |

## 2. Shikimori facts

| # | Fact | Source | Status | Used in |
|---|---|---|---|---|
| S-V1 | `POST /api/v2/user_rates` finds an existing row for (user_id, target_id, target_type) and updates it; otherwise creates it | Shikimori controller, `create` action: `present_rate = UserRate.find_by(user_id: @resource.user_id, target_id: @resource.target_id, target_type: @resource.target_type)`, then `update_rate` or `create_rate` | Verified (source) | p1-behavior.md 2.11, T-31a-011 |
| S-V2 | `user_rates` status enum values `planned:0, watching:1, rewatching:9, completed:2, on_hold:3, dropped:4` | `app/models/user_rate.rb`, `enum status` | Verified (source) | p1-data-and-config.md 3.1 |
| S-V3 | Score: blank becomes 0; `score > 10` or negative reverts to the previous value; no error is returned | `app/models/user_rate.rb`, `score_changed` | Verified (source) | D8, EC-S-23 |
| S-V4 | Max text length 4096; max episodes 9999; max score 10 | `app/models/user_rate.rb`, constants | Verified (source) | p1-data-and-config.md 1.1 |
| S-V5 | Uniqueness of (user_id, target_id, target_type) | `app/models/user_rate.rb`, `validates :user_id, uniqueness: { scope: %i[target_id target_type] }` | Verified (source) | S-V1 |
| S-V6 | Completing a title fills counters from the target | `app/models/user_rate.rb`, `anime_status_changed` and `manga_status_changed` | Verified (source) | p1-behavior.md 2.11 |
| S-V7 | Access token lifetime 1 day | `config/initializers/doorkeeper.rb`, `access_token_expires_in 1.day` (line 70) | Verified (source) | EC-S-05, timings |
| S-V8 | Authorisation code lifetime 30 minutes | same file, `authorization_code_expires_in 30.minutes` (line 65) | Verified (source) | p1-data-and-config.md 8 |
| S-V9 | Refresh tokens are issued | same file, `use_refresh_token` (line 175) | Verified (source) | refresh flow |
| S-V10 | Default scopes `user_rates`, `comments`, `topics` | same file, `default_scopes :user_rates, :comments, :topics` (line 191) | Verified (source) | authorise URL has no scope |
| S-V11 | `reuse_access_token` is off, so each refresh issues a new access token | same file, `reuse_access_token` commented out (line 112) | Verified (source) | refresh behaviour |
| S-V12 | Destroying a row requires the `user_rates` scope | Shikimori controller, `destroy` action with `doorkeeper_authorize! :user_rates` | Verified (source) | D delete path |
| S-V13 | Index `user_rates` needs `user_id`, `target_id` or `target_type`; `target_id` without `target_type` is an error | Shikimori controller, `index` action (`MissingApiParameter`) | Verified (source) | p1-data-and-config.md 7.1 S8 |
| S-V14 | Index `limit` clamped to 1..1000; `page` ignored when `user_id` set | Shikimori controller, `index` action (`MAX_LIMIT = 1000`) | Verified (source) | p1-data-and-config.md 7.4 |
| S-V15 | Refresh token rotation: a refresh request revokes the previous refresh token | Doorkeeper default; not stated in the config file | Not verified (live) | OQ-6, T-31a-046 |
| S-V16 | `GET /api/mangas`, `/api/animes` and `/api/mangas/{id}` need no token | not checked in source; the Kotlin sends one | Not verified (live) | EC-S-01, T-31a-045 |
| S-V17 | Shape of `/api/users/whoami` (`id` present, integer) | not checked in source | Not verified (live) | EC-S-11 |

## 3. Bangumi facts

| # | Fact | Source | Status | Used in |
|---|---|---|---|---|
| B-V1 | Collection types: 1 想看 (wish), 2 看过 (done), 3 在看 (doing), 4 搁置 (on hold), 5 抛弃 (dropped) | `open-api/v0.yaml`, schema `SubjectCollectionType` | Verified (doc) | p1-data-and-config.md 3.2 |
| B-V2 | Subject types: 1 book, 2 anime, 3 music, 4 game, 6 real (no 5) | `open-api/v0.yaml`, schema `SubjectType` | Verified (doc) | search type filters |
| B-V3 | `POST /v0/users/-/collections/{subject_id}`: create or modify; all fields optional; 204 on success in spec, 202 observed by the Kotlin comment | `open-api/v0.yaml`, `postUserCollection`; Kotlin comment at `BangumiApi.kt:54` | Verified (doc) for 204; the 202 is the Kotlin's observation | EC-B-18 |
| B-V4 | `PATCH` same URL: 204 | `open-api/v0.yaml`, `patchUserCollection` | Verified (doc) | EC-B-17 |
| B-V5 | `ep_status` is for book subjects; direct episode completion changes may have unexpected effects | `open-api/v0.yaml`, `UserSubjectCollectionModifyPayload.ep_status` description (`只能用于修改书籍条目进度`) and `postUserCollection` description | Verified (doc) | D7 |
| B-V6 | `rate` 0..10; `0` removes the rating | `open-api/v0.yaml`, `UserSubjectCollectionModifyPayload.rate` (`0 表示删除评分`) | Verified (doc) | EC-B-19 |
| B-V7 | Collection GET: 404 means the user has not collected the subject, or the collection is private | `open-api/v0.yaml`, `getUserCollection` (`用户不存在或者条目未收藏，或者条目为私有收藏`) | Verified (doc) | EC-B-06 |
| B-V8 | Collection GET: `security: OptionalHTTPBearer` (public read works without a token) | `open-api/v0.yaml`, `getUserCollection` | Verified (doc) | D9 |
| B-V9 | Search `POST /v0/search/subjects` has no `security` entry (public) | `open-api/v0.yaml`, `searchSubjects` | Verified (doc) | D9, EC-B-01 |
| B-V10 | Search `sort` values `match`, `heat`, `rank`, `score` | `open-api/v0.yaml`, `searchSubjects` | Verified (doc) | p1-data-and-config.md 7.3 |
| B-V11 | `/v0/me` returns `username`, `id`, `nickname`, `email`, `reg_time` | `open-api/v0.yaml`, `getMyself` and `User` | Verified (doc) | p1-data-and-config.md 1.2 |
| B-V12 | Episode collection types: 0 none, 1 wish, 2 done, 3 dropped | `open-api/v0.yaml`, `EpisodeCollectionType` | Verified (doc) | D7 alternative (T-31a-044) |
| B-V13 | Episode endpoints: `GET /v0/users/-/collections/{subject_id}/episodes` (limit max 1000), `PATCH` same path with `episode_id` array and `type`, `PUT /v0/users/-/collections/-/episodes/{episode_id}` | `open-api/v0.yaml` | Verified (doc) | T-31a-044 |
| B-V14 | `GET /v0/episodes?subject_id=<subject_id>` lists episodes (limit max 200) | `open-api/v0.yaml`, `getEpisodes` | Verified (doc) | T-31a-044 |
| B-V15 | Authorisation code valid 60 seconds | `docs-raw/How-to-Auth.md`, step 2 | Verified (doc) | p1-data-and-config.md 8 |
| B-V16 | Token response: `access_token`, `expires_in: 604800`, `token_type`, `scope`, `refresh_token`, `user_id` | `docs-raw/How-to-Auth.md`, step 3 example | Verified (doc) | p1-data-and-config.md 1.2 |
| B-V17 | Refresh request needs `grant_type=refresh_token`, `client_id`, `client_secret`, `refresh_token`, `redirect_uri` | `docs-raw/How-to-Auth.md`, refresh section table | Verified (doc) | EC-B-03 |
| B-V18 | Refresh response example: `access_token`, `expires_in: 604800`, `token_type`, `scope: null`, `refresh_token`; no `user_id` | `docs-raw/How-to-Auth.md`, refresh example | Verified (doc) | EC-B-12, D6 (withdrawn) |
| B-V19 | `/v0/` does not accept the token in the query string | `docs-raw/How-to-Auth.md`, section 4 note | Verified (doc) | p1-data-and-config.md 7 |
| B-V20 | User-agent must identify the developer and app; generic UAs such as `Bangumi/1.0` are disallowed | `docs-raw/user agent.md` | Verified (doc) | p1-rust-design.md 6.2 |
| B-V21 | Refresh returns a new `refresh_token` | `docs-raw/How-to-Auth.md`, refresh example | Verified (doc) | token rotation |
| B-V22 | Bangumi refresh rotation: the old refresh token stops working | not stated | Not verified (live) | OQ-7, T-31a-046 |
| B-V23 | Search results keep `platform` strings such as `"漫画"` | the Kotlin filter; spec `BGMSubject` schema not re-read for platform | Not verified (live) | EC-B-23 |
| B-V24 | Refresh response without `user_id` decodes in the app | app `Json` config (`explicitNulls = false`, `AppModule.kt:173-177`) makes absent nullable keys null | Verified (source, Kotlin config) and doc (B-V18) | D6 |

## 4. Corrections found while checking

These were first assumed and then disproved. They are recorded so that nobody re-introduces them.

| Earlier assumption | Correct fact | Effect on plan |
|---|---|---|
| The app `Json` only sets `ignoreUnknownKeys` | It also sets `explicitNulls = false` (`AppModule.kt:173-177`) | D6 withdrawn; D20 and D21 reworded (absent nullable keys are fine) |
| Shikimori POST for update is a different request from add | Both are the same upsert (S-V1) | p1-behavior.md 2.11 states one request |
| Bangumi POST returns the new id | It returns 202 with no body (B-V3) | `library_id` stays null for Bangumi |

## 5. What is still to verify on a live account (T-31a-046)

| Item | Question | Why it matters |
|---|---|---|
| S-V15 | Does Shikimori invalidate the old refresh token on refresh? | If yes, concurrent refreshes must be serialised (D2). The single-flight lock covers it either way. |
| S-V16 | Are the Shikimori catalogue GETs usable without a token? | Decides P-anon (T-31a-045). |
| S-V17 | Exact `whoami` shape | Confirms the `id` field name and integer type. |
| B-V22 | Does Bangumi invalidate the old refresh token? | Same as S-V15 for Bangumi. |
| B-V23 | Platform values in search results | Confirms the manga filter (EC-B-23). |
| B-V18 / B-V24 | Refresh response keys on a real token | Confirms that the refresh decodes without `user_id`. |
| B-V5 / D7 | `PATCH ... ep_status` on an anime subject: accepted, ignored, or rejected | Decides whether anime progress on Bangumi is working at all (T-31a-044). |

## 6. Live verification results

Not yet run. T-31a-046 needs the user's own test accounts. Results go here with the date, the account type, and the response shape observed.
