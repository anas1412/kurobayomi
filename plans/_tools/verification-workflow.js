export const meta = {
  name: 'aniyomi-plan-verification',
  description: 'Verify and repair the Aniyomi Rust port plan: per-part critics + patchers, catalog verifiers, deterministic coverage loop, adversarial lens sweeps, final repair loop',
  phases: [
    { title: 'Area plans', detail: 'critic then patch for every source part' },
    { title: 'Catalogs', detail: 'verify and fix catalog rows' },
    { title: 'Coverage loop', detail: 'inventory check, fill gaps until nothing is unmentioned' },
    { title: 'Lens sweeps', detail: 'independent omission hunters, verified, patched until dry' },
    { title: 'Final verify', detail: 'whole-plan verification and repair' },
  ],
}

const PLANS = '/home/blackbox/kurobayomi/plans'
const SRC = '/home/blackbox/aniyomi-src'
const TOOLS = PLANS + '/_tools'

// [id, parts, design-only, title]
const AREAS = [
  ["01-foundation-app-bootstrap", 1, false, "App bootstrap, DI, crash handling, app-level plumbing"],
  ["02-preferences-system", 1, false, "Preference store + every *Preferences class (all keys, types, defaults)"],
  ["03-db-manga-schema-migrations", 1, false, "Manga database: .sq tables/views/queries/triggers and all .sqm migrations"],
  ["04-db-anime-schema-migrations", 1, false, "Anime database: .sq tables/views/queries/triggers and all .sqm migrations"],
  ["05-data-repos-entries-items", 1, false, "Data layer repositories: entries, items (chapters/episodes/seasons), categories"],
  ["06-data-repos-history-track-source-misc", 1, false, "Data layer repositories: history, track, updates, sources, releases, custom buttons, handlers, extension repos"],
  ["07-domain-entries-items", 1, false, "Domain: entries (manga/anime/season) and items (chapter/episode/season) models + interactors + tests"],
  ["08-domain-library-category-history-updates", 1, false, "Domain: library, category, history, updates"],
  ["09-domain-source-track-download-release-misc", 1, false, "Domain: source, track, download, release, storage, backup, custom buttons, extension repos/stores"],
  ["10-app-domain-layer", 1, false, "App-level domain layer (eu.kanade.domain): interactors, mappers, DomainModule"],
  ["11-app-migrations", 1, false, "Mihon/Aniyomi in-app migration framework + each migration step"],
  ["12-network-stack", 1, false, "HTTP stack: OkHttp client, interceptors, Cloudflare, DoH, rate limiting, cookies, user agent"],
  ["13-core-util-storage-archive-torrent", 1, false, "core/common utilities, storage abstraction, archive readers, torrent helpers"],
  ["14-source-local-and-metadata", 1, false, "Local source (manga + anime) and metadata parsing (ComicInfo, details.json)"],
  ["15-source-api-manga", 1, false, "Manga source API: Source/HttpSource/CatalogueSource, models, filters, ConfigurableSource"],
  ["16-source-api-anime", 1, false, "Anime source API: AnimeSource/AnimeHttpSource, Video/Hoster models, filters, season/episode fetch types"],
  ["17-source-app-layer", 1, false, "App source layer: SourceManager, StubSource, source interactors, source pinning/language/etc."],
  ["18-extensions-manga", 1, false, "Manga extension loader, installer, trust, update API, extension repos"],
  ["19-extensions-anime", 1, false, "Anime extension loader, installer, trust, update API, extension stores"],
  ["20-extension-strategy-redesign", 1, true, "Design analysis: how Kurobayomi replaces APK extensions (WASM/Lua/JS/declarative/JVM bridge), source catalog migration"],
  ["21-backup-format-and-create", 1, false, "Backup file format (protobuf+gzip), models, creators, auto-backup"],
  ["22-backup-restore", 1, false, "Backup restore: restorers, validators, legacy handling, restore job, error reporting"],
  ["23-downloads-manga", 1, false, "Manga downloads: Downloader, DownloadManager, provider, cache, queue, job, notifications"],
  ["24-downloads-anime", 1, false, "Anime downloads: Downloader (HTTP/HLS/ffmpeg/torrent), manager, provider, cache, queue, job"],
  ["25-downloads-shared", 1, false, "Shared download infrastructure + domain download + queue UI base + download storage layout"],
  ["26-library-update-jobs", 1, false, "Library update jobs, scheduling, restrictions, fetch intervals, notifications"],
  ["27-tracking-core", 2, false, "Tracker framework: Tracker/TrackerManager, models, OAuth plumbing, enhanced trackers, track domain/UI"],
  ["28-tracker-anilist", 1, false, "AniList tracker"],
  ["29-tracker-myanimelist", 1, false, "MyAnimeList tracker"],
  ["30-tracker-kitsu", 1, false, "Kitsu tracker"],
  ["31-tracker-shikimori-bangumi", 1, false, "Shikimori + Bangumi trackers"],
  ["32-tracker-mangaupdates-simkl", 1, false, "MangaUpdates + Simkl trackers"],
  ["33-tracker-enhanced-kavita-komga-suwayomi-jellyfin", 1, false, "Enhanced trackers: Kavita, Komga, Suwayomi, Jellyfin"],
  ["34-images-covers-cache", 1, false, "Image pipeline: Coil fetchers/mappers/decoders, cover cache, image decoding, saving"],
  ["35-data-misc-updater-notification-export", 1, false, "Notifications, app updater, library export, misc data services"],
  ["36-ui-shell-navigation-security-webview", 1, false, "App shell: MainActivity, HomeScreen, navigation, deeplinks, security/unlock, webview, base activities, onboarding"],
  ["37-ui-library", 1, false, "Library screens (manga + anime): grid/list, categories, filters, sorting, selection, per-category settings"],
  ["38-ui-entry-details-manga", 2, false, "Manga details screen + ScreenModel logic (chapter list, filters, bulk actions, merge/migrate triggers)"],
  ["39-ui-entry-details-anime", 2, false, "Anime details screen + ScreenModel logic (episode list, seasons, filters, bulk actions, filler marks)"],
  ["40-ui-browse-manga", 2, false, "Manga browse: sources list, browse source, global search, filters, extensions screen, migration screens"],
  ["41-ui-browse-anime", 2, false, "Anime browse: sources list, browse source, global search, filters, extensions screen, migration screens (incl. seasons)"],
  ["42-ui-updates-history-stats-storage-upcoming", 2, false, "Updates, History, Stats, Storage, Upcoming calendar screens"],
  ["43-reader-core", 2, false, "Manga reader core: ReaderActivity/ViewModel, chapter loading (zip/dir/epub/http), model, preloading, progress saving, settings logic"],
  ["44-reader-viewers", 1, false, "Reader viewers: pager (L2R/R2L/vertical), webtoon, tap-zone navigation, zoom/gestures, image rendering"],
  ["45-player-core", 2, false, "Anime player core: PlayerActivity/ViewModel, video loading, mpv integration, history/progress, external players, torrent/HTTP services"],
  ["46-player-controls-ui", 2, false, "Anime player controls UI: gestures, seekbar, sheets, panels, dialogs, overlays, presentation/player"],
  ["47-settings-screens-general", 2, false, "Settings screens (general): appearance, library, reader, browse, downloads, tracking, security, advanced, data/backup UI, about, debug"],
  ["47b-settings-widgets-search-more", 1, false, "Settings widgets/search infrastructure, the More tab, settings main menu, ui/setting screens"],
  ["48-settings-player-and-scripts", 1, false, "Player settings screens, custom buttons editor, mpv script/config editor, code editor"],
  ["49-theme-and-design-system", 2, false, "Themes (19 color schemes), TachiyomiTheme, presentation-core components/icons/util, mihon designsystem"],
  ["49b-android-res-values-drawables", 1, false, "Android resources: values (colors/themes/dimens/styles), drawables (icons), colors; mapping to GTK CSS/icon theme"],
  ["50-android-widgets", 1, false, "Android home-screen widgets (presentation-widget) and their desktop equivalents"],
  ["51-i18n", 1, false, "Localization: moko resources, 67 locale directories, plurals, RTL, string key inventory, translation workflow"],
  ["52-util-app", 1, false, "App utilities (eu.kanade.tachiyomi.util + core util): lang, system, storage, view helpers and their Linux equivalents"],
  ["53-platform-manifest-intents-workers", 1, false, "AndroidManifest components, permissions, intents/deeplinks, shortcuts, services/receivers/workers, res layouts/menus/anim, and their Linux equivalents"],
  ["54-build-release-distribution-docs", 1, false, "Gradle/build logic, CI, flavors, fastlane, docs (README/CHANGELOG/CONTRIBUTING), macrobenchmark; Cargo/Flatpak/CI equivalents"],
].map(([id, parts, design, title]) => ({ id, parts, design, title }))

const CTX = `PROJECT CONTEXT
We are writing the complete plan for a Rust + GTK4 (gtk4-rs, NO libadwaita) Linux-desktop rewrite of Aniyomi (Kotlin / Jetpack Compose Android app, a Tachiyomi/Mihon fork: manga reader + anime player + extensions + trackers + downloads + backups).
- The Rust app is "Kurobayomi" at /home/blackbox/kurobayomi (a UI-only mockup already exists there: src/*.rs, plus spec/*.json that was extracted from Aniyomi's UI).
- The Aniyomi source is cloned, read-only, at ${SRC} (Apache-2.0). Every path in the plans is relative to that directory ("repo-relative", e.g. app/src/main/java/eu/kanade/tachiyomi/App.kt).
- All plan documents live in ${PLANS}/ (your output). Helper tools are in ${TOOLS}/ (inventory.py, areas.py, tasks.py, verify.py).
GOAL: make Kurobayomi usable and functional. EVERY Aniyomi capability, small or big, must end up either (a) planned for porting/rewriting in Rust, (b) replaced by a named Linux-native equivalent, or (c) explicitly listed as dropped with a reason and what the user loses. The plan must be detailed enough that an engineer who never opens the Kotlin can implement it: exact constants, defaults, enum values and bit layouts, SQL semantics, algorithms, ordering rules, edge cases, error handling, user-visible message keys, timings, limits.
TARGET: Linux (CachyOS/Arch, KDE Plasma with GTK apps, Wayland+X11), stable Rust, gtk4-rs 0.11 (GTK 4.16+, vanilla GTK so the user's GTK theme applies; no libadwaita), tokio, rusqlite, reqwest, serde, libmpv for video. The user is a solo developer who wants something he can use daily.
You may use read-only research (gh api, WebFetch, WebSearch) when the source alone cannot answer a question (e.g. what a third-party API returns).`

const DOC_RULES = `DOCUMENT RULES (strict)
- GitHub-flavoured markdown. Be exhaustive and concrete. Never write placeholders or elisions: no TBD/TODO/FIXME, no "etc.", no "and so on", no "see source", no "similar to above". Enumerate fully.
- Cite source with FULL repo-relative paths exactly as they appear on disk (e.g. app/src/main/java/eu/kanade/tachiyomi/ui/reader/ReaderActivity.kt). Add line numbers as path:line where useful.
- Quote exact constants, defaults, enum values, regexes, URLs, JSON field names, SQL fragments as they are in the source.
- Separate OBSERVED (what the Kotlin does) from PROPOSAL (what Rust should do); label proposals "Proposal:".
- Name the Linux/Rust replacement for every Android/Kotlin library or API touched. For anything dropped, say why and what the user loses.
- Refer to other areas as area:<area-id> (e.g. area:12-network-stack).
TASK FORMAT (lint every tasks file with: python3 ${TOOLS}/tasks.py lint <file>   -> must print ok)
### T-<NN><partletter>-<3 digits> — Title            (e.g. "### T-38a-001 — Chapter list sort/filter engine")
- Priority: P0 | P1 | P2 | P3
- Effort: S | M | L | XL
- Depends: T-xxx-nnn, area:<area-id>   (or: none)
- Sources: repo-relative path, repo-relative path
- Acceptance:
  - testable, observable criterion
P0 = needed so Kurobayomi is usable daily for reading manga and watching anime from at least one working source (library, details, reader, player, progress, local DB, essential settings, a source mechanism). P1 = parity features most Aniyomi users rely on (downloads, tracking, backup/restore, library updates + notifications, categories, global search, source/extension management, history/updates, migration). P2 = long-tail features and polish. P3 = Android-only/obsolete: still gets a task that states the replacement or the drop decision.
Effort: S <= 1 day, M <= 3 days, L <= 1.5 weeks, XL > 1.5 weeks (split anything above ~3 weeks).`

const letter = (k) => String.fromCharCode(96 + k)
const filesCmd = (area, k) => `python3 ${TOOLS}/areas.py files ${area.id} ${k}`

function authorPrompt(area, k, n) {
  const pfx = `p${k}-`
  const dir = `${PLANS}/${area.id}`
  const tid = `T-${area.id.split('-')[0]}${letter(k)}`
  const design = area.design
  return `${CTX}

${DOC_RULES}

YOU ARE THE AUTHOR for plan area "${area.id}" — ${area.title} (part ${k} of ${n}).
SCOPE: run  ${filesCmd(area, k)}  -> prints "<repo-relative path><TAB><loc>" for every file in your scope (cwd-independent; files live under ${SRC}).
${design
    ? `This is a DESIGN area: read all the extension/source-api files that matter (loader, installer, trust, ExtensionManager/SourceManager, source-api models) plus gh/WebFetch research on the real extension ecosystem (keiyoushi / aniyomi extension repos, index.min.json format, multisrc, source id generation) and write a decisive design: option matrix (WASM component model/wasmtime, Lua via mlua, JS via rquickjs, declarative CSS-selector scrapers, JVM bridge running dex2jar'd Aniyomi extensions with an Android shim, sidecar processes, statically linked Rust sources), security/sandbox, performance, authoring ergonomics, ecosystem reuse, legal; a recommended path with phases; the exact Rust Source trait/types derived from source-api (manga + anime); source-id mapping so Aniyomi backups still resolve; behaviour when a source is missing; how repos/stores/updates/trust work in the new world. Every file in your scope must still appear in your inventory with its port decision.`
    : `Read EVERY file in your scope IN FULL (use offset/limit chunks for long files; never skim, never rely on filenames). Also read callers/callees outside the scope whenever you need them to be exact. The documents cover your scope files.`}
OUTPUT FOLDER: ${dir}/ — create it if missing. File prefix: "${pfx}". Write these files with the Write tool, each substantial and exhaustive:
1. ${pfx}inventory.md — a table with ONE ROW PER SCOPE FILE: full repo-relative path | LOC | ALL top-level classes/objects/interfaces/enums/typealiases/top-level functions/composables declared (every name) | purpose | port decision (Port / Rewrite / Replace-with-crate / Drop) with reason | target Rust module/crate | priority | task ids. Every path printed by the scope command MUST appear verbatim.
2. ${pfx}behavior.md — the behavior spec: per feature/class, step-by-step algorithms, state machines, defaults, constants with values, ordering/sorting rules, edge cases, error paths and the user-visible message keys (MR.strings.* / AYMR.strings.*), side effects (notifications, work scheduling, file writes, DB writes), concurrency/dispatchers, timings/debounces/limits, interactions with other areas.
3. ${pfx}data-and-config.md — data models (every field, type, nullability, default; enums with their stored numeric/string values; bit-flag layouts), preference keys used here with their defaults, files/folders/formats on disk, network endpoints (URL, method, params, headers, auth, response shape), DB tables/columns touched.
4. ${pfx}rust-design.md — concrete Rust design: crate/module layout, key types and trait signatures in code blocks, async/threading model (tokio vs the GTK main loop), crates to use and why, error handling, how this plugs into the existing Kurobayomi mockup code (src/ui.rs, src/screens.rs, src/spec.rs, spec/*.json), Android-API -> Linux mapping, pitfalls.
${design ? '' : `5. ${pfx}ui-parity.md — ONLY if scope files render UI (Compose screens/Activities/dialogs): every visible element, state (loading/empty/error/selection), dialog, snackbar/toast, menu, gesture, animation, keyboard/mouse equivalent on desktop, responsive behaviour, plus precisely what the existing mockup lacks. Skip this file if there is no UI.
`}6. ${pfx}edge-cases-and-tests.md — edge cases, failure modes, race conditions, upstream test cases to port (from any *Test.kt in scope), new tests to write, fixtures needed.
7. ${pfx}tasks.md — the implementation task list (format above). IDs must start with ${tid}- (e.g. ${tid}-001). Cover EVERYTHING in the scope: no feature, setting, edge case or file may be without a task (or an explicit P3 drop task). Lint it.
8. Extra topic files ${pfx}topic-<slug>.md for any subsystem that deserves its own deep dive (encouraged whenever the scope has more than one subsystem).
SELF-CHECK before you finish (fix until it prints nothing):
  for f in $(${filesCmd(area, k)} | cut -f1); do grep -qF "$f" ${dir}/${pfx}inventory.md || echo MISSING $f; done
  python3 ${TOOLS}/tasks.py lint ${dir}/${pfx}tasks.md
Finish with a 5-line summary of what you wrote (file names + counts). Do not write anything outside ${dir}/.`
}

const CRITIC = {
  type: 'object',
  properties: {
    files_read: { type: 'integer' },
    omissions: { type: 'array', items: { type: 'object', properties: {
      file: { type: 'string' }, what: { type: 'string' }, severity: { type: 'string', enum: ['critical', 'major', 'minor'] },
      evidence: { type: 'string' }, put_in: { type: 'string' } }, required: ['file', 'what', 'severity', 'evidence'] } },
    wrong_claims: { type: 'array', items: { type: 'object', properties: {
      claim: { type: 'string' }, where: { type: 'string' }, truth: { type: 'string' }, evidence: { type: 'string' } }, required: ['claim', 'where', 'truth', 'evidence'] } },
  },
  required: ['omissions', 'wrong_claims'],
}

function criticPrompt(area, k, n, round) {
  const dir = `${PLANS}/${area.id}`
  const pfx = `p${k}-`
  const focus = round === 1
    ? 'ROUND 1 = BREADTH: for every scope file, check every class, public function, branch, constant, default, enum value, preference key, query, string key, side effect and UI state against the plan docs.'
    : 'ROUND 2 = DEPTH: line by line numbers/limits/timeouts, ordering, null/empty/error handling, concurrency, off-by-one and rounding rules, regexes, SQL semantics, migration steps; also challenge the Rust design (impossible crate choices, wrong threading model, missing GTK equivalents) and the task list (features without a task, missing acceptance criteria, wrong priorities/dependencies).'
  return `${CTX}

YOU ARE THE ADVERSARIAL CRITIC for area "${area.id}" part ${k}/${n}. The author wrote the docs ${dir}/${pfx}*.md. Find what they MISS or got WRONG.
${focus}
Method: run ${filesCmd(area, k)} for the scope; Read every scope file again in full; Read every ${dir}/${pfx}*.md; compare. Verify claims against the source rather than trusting the docs. Do not report style preferences. Each item needs evidence (source path:line, or doc path + quote). 'put_in' = which plan file should receive the fix.
Also report format problems as wrong_claims (e.g. an inventory row missing, a task without acceptance criteria).
Return the structured result.`
}

function patchPrompt(area, k, n, findings) {
  const dir = `${PLANS}/${area.id}`
  return `${CTX}

${DOC_RULES}

YOU ARE THE PATCHER for area "${area.id}" part ${k}/${n}. A critic returned the findings below. For EACH finding: verify it against the Aniyomi source (${SRC}); if real, fix the plan docs in ${dir}/ in place (Edit/append; add new sections or new files p${k}-topic-*.md as needed; add tasks to p${k}-tasks.md with unique IDs continuing the numbering); if not real, skip it. Keep every existing path in the inventory. Re-run python3 ${TOOLS}/tasks.py lint ${dir}/p${k}-tasks.md (must print ok).
FINDINGS (JSON):
${JSON.stringify(findings)}
Finish with: applied N, rejected M (one line each reason for rejections).`
}

async function runPart(area, k, n) {
  const label = `${area.id}${n > 1 ? '/p' + k : ''}`
  for (let round = 1; round <= 2; round++) {
    const c = await agent(criticPrompt(area, k, n, round), { label: `critic${round} ${label}`, phase: 'Area plans', schema: CRITIC, effort: 'high' })
    if (!c) break
    const total = c.omissions.length + c.wrong_claims.length
    log(`${label} critic${round}: ${c.omissions.length} omissions, ${c.wrong_claims.length} wrong`)
    if (total === 0) break
    await agent(patchPrompt(area, k, n, c), { label: `patch${round} ${label}`, phase: 'Area plans', effort: 'high' })
  }
}

function leadPrompt(area) {
  const dir = `${PLANS}/${area.id}`
  return `${CTX}

${DOC_RULES}

YOU ARE THE AREA LEAD for "${area.id}" — ${area.title}. The part authors have written the docs in ${dir}/. Read all of them (and spot-check the source for anything unclear), then write ${dir}/00-overview.md containing:
1. Purpose and scope (what Aniyomi does here, why it matters for a usable Kurobayomi).
2. Module map of the Kotlin code (packages -> responsibilities) and the proposed Rust module/crate map.
3. A reading guide: every doc in the folder with a one-line description (relative markdown links).
4. Key design decisions and open questions that need the user's call (with a recommendation each).
5. Cross-area dependencies: what this area needs from others (area:<id>) and what others need from it.
6. Task summary: counts per priority/effort (run python3 ${TOOLS}/tasks.py export and read TASKS.csv rows whose file starts with ${area.id}/), the P0 critical path in order, and risks.
7. "Definition of done" for this area in Kurobayomi (observable behaviors).
Also: run python3 ${TOOLS}/tasks.py lint on every *tasks*.md in the folder; fix any lint errors and fix 'area:' dependency tokens so they use real area ids from ${PLANS}. Do not change task IDs.`
}

async function runArea(area) {
  const n = area.parts
  await parallel(Array.from({ length: n }, (_, i) => () => runPart(area, i + 1, n)))
}

// ---------------- catalogs (source-driven, independent of area docs) ----------------
const CATS = [
  { cat: 'prefs', n: 8, dir: 'preferences', title: 'Preference keys',
    how: `For EVERY key: find its definition in the source (rg -n for the key string), then write a table row: key | store (app-state/private/normal) | Kotlin type | default value | exact enum/string values if enum-valued | where defined (path:line) | where read/used (paths) | what it changes behaviorally | settings screen/label that exposes it | proposed Rust config field name + type + default | keep/drop decision. Keys that are not real preferences (false positives of the extractor, e.g. string templates) must still get a row saying exactly what they are.` },
  { cat: 'sql', n: 4, dir: 'sql', title: 'SQL tables/views/triggers/indexes/queries',
    how: `For EVERY item: copy the exact SQL (trimmed), then describe columns/params/returned fields, ordering, joins, filters, NULL handling, trigger side effects, which Kotlin repository/interactor calls it (path:line), perf notes, and give the proposed rusqlite equivalent (statement text + Rust signature). For tables list every column with type/default/constraints and which migration (.sqm) introduced it.` },
  { cat: 'imports', n: 8, dir: 'android-api-mapping', title: 'Android/Java/third-party API classes -> Linux/Rust replacements',
    how: `For EVERY class name: say what it does in Aniyomi (with the representative usage path:line and usage count: rg -c), the Linux/Rust replacement (std / crate name / GTK API / custom code / drop), behavior that must be preserved, porting difficulty (trivial/moderate/hard) and risks.` },
  { cat: 'hosts', n: 2, dir: 'network-endpoints', title: 'Network hosts and endpoints',
    how: `For EVERY host: its role (tracker API, DoH provider, image CDN, docs link, update check, etc.), every URL/endpoint pattern used in the source (method, path, query/body params, headers, auth scheme, pagination, rate limits, response fields consumed), where used (path:line), whether the Rust app keeps it, and test/verification advice. Doc-only links are listed with what they are for.` },
]

const CAT_VERIFY = { type: 'object', properties: { rows_checked: { type: 'integer' }, errors: { type: 'array', items: { type: 'object', properties: { item: { type: 'string' }, problem: { type: 'string' }, fix: { type: 'string' } }, required: ['item', 'problem', 'fix'] } } }, required: ['rows_checked', 'errors'] }

async function runCatalog(c, i) {
  const file = `${PLANS}/catalogs/${c.dir}-${String(i).padStart(2, '0')}.md`
  const items = `python3 ${TOOLS}/inventory.py items ${c.cat} ${i} ${c.n}`
  const v = await agent(`${CTX}

YOU ARE THE VERIFIER of ${file}. Re-derive every row yourself from the Aniyomi source (${SRC}) — types, defaults, SQL text, call sites, endpoints — and report every row that is wrong, incomplete or unverifiable. Items of this chunk: run ${items}. Return the structured result.`,
    { label: `verify ${c.cat} ${i}/${c.n}`, phase: 'Catalogs', schema: CAT_VERIFY, effort: 'high' })
  if (v && v.errors.length) {
    log(`catalog ${c.cat} ${i}: ${v.errors.length} errors`)
    await agent(`${CTX}

Fix these verified errors in ${file} (re-check each against the source first; skip any that are not real):
${JSON.stringify(v.errors)}`, { label: `fix ${c.cat} ${i}/${c.n}`, phase: 'Catalogs', effort: 'high' })
  }
}

// ---------------- stage 1+2: areas and catalogs in parallel ----------------
phase('Area plans')
await Promise.all([
  parallel(AREAS.map((a) => () => runArea(a))),
  parallel(CATS.flatMap((c) => Array.from({ length: c.n }, (_, i) => () => runCatalog(c, i + 1)))),
])

// ---------------- stage 3: coverage loop (deterministic) ----------------
const CAT = { type: 'object', properties: { total: { type: 'integer' }, missing: { type: 'integer' } }, required: ['total', 'missing'] }
const COUNTS = { type: 'object', properties: { files: CAT, prefs: CAT, sql: CAT, manifest: CAT, decls: CAT, imports: CAT, hosts: CAT }, required: ['files', 'prefs', 'sql', 'manifest', 'decls', 'imports', 'hosts'] }
const PER_CHUNK = { files: 10, decls: 50, prefs: 35, sql: 20, manifest: 25, imports: 35, hosts: 12 }
const GAP_HOW = {
  files: 'Treat each file exactly like a part author would: read it in full and document it (inventory row incl. ALL declared names, behavior, data/config, Rust design, edge cases) and add tasks.',
  decls: 'For each top-level declaration: open its file, document what it is (fields, behavior, who uses it) and its Rust equivalent or drop reason; mention its file path.',
  prefs: 'For each key: find the definition and write the same row as the preference catalog (store, type, default, usage, behavior, Rust field).',
  sql: 'For each item: exact SQL, semantics, callers, rusqlite equivalent.',
  manifest: 'For each manifest/intent/permission/component item: what it is in Aniyomi (path:line), what triggers it, behavior, and the Linux equivalent (or drop reason); add tasks.',
  imports: 'For each class: what it does in Aniyomi (usage path:line), the Linux/Rust replacement, behavior to preserve, difficulty.',
  hosts: 'For each host: its role, every endpoint pattern used (method/path/params/auth), where used, keep/drop.',
}

phase('Coverage loop')
for (let round = 1; round <= 4; round++) {
  const counts = await agent(`Run exactly:  python3 ${TOOLS}/inventory.py counts  and return its JSON output as the structured result (it also writes snapshot files).`, { schema: COUNTS, effort: 'low', phase: 'Coverage loop', label: `counts r${round}` })
  if (!counts) { log('counts agent failed'); break }
  const todo = Object.keys(PER_CHUNK).filter((c) => counts[c].missing > 0)
  log(`coverage round ${round}: ` + Object.keys(PER_CHUNK).map((c) => `${c} ${counts[c].missing}/${counts[c].total}`).join(', '))
  if (todo.length === 0) break
  const jobs = []
  for (const cat of todo) {
    const n = Math.ceil(counts[cat].missing / PER_CHUNK[cat])
    for (let i = 1; i <= n; i++) jobs.push([cat, i, n])
  }
  await parallel(jobs.map(([cat, i, n]) => () => agent(`${CTX}

${DOC_RULES}

COVERAGE GAP FILL (round ${round}). The automatic inventory check says the plans never mention these ${cat} items. Run  python3 ${TOOLS}/inventory.py snap ${cat} ${i} ${n}  -> "<item><TAB><source files>" lines = YOUR items. ${GAP_HOW[cat]}
Write ${PLANS}/addenda/gap-r${round}-${cat}-${i}.md (every item name/path verbatim, with full detail, not one-liners). ${['files', 'manifest'].includes(cat) ? `Also write ${PLANS}/addenda/gap-r${round}-${cat}-${i}-tasks.md with tasks whose IDs start T-gap-r${round}${cat}${i}- (e.g. T-gap-r${round}${cat}${i}-001), lint it, and cover every item.` : 'If an item implies work, add tasks in a sibling file gap-r' + round + '-' + cat + '-' + i + '-tasks.md (IDs T-gap-r' + round + cat + i + '-001...), lint it.'}
Verify: for each item name, grep -rF in ${PLANS} (excluding _tools) must now match.`, { label: `gap r${round} ${cat} ${i}/${n}`, phase: 'Coverage loop', effort: 'high' })))
}

// ---------------- stage 4: adversarial lens sweeps ----------------
const FOUND = {
  type: 'object',
  properties: {
    checked: { type: 'integer' },
    omissions: { type: 'array', items: { type: 'object', properties: {
      title: { type: 'string' }, evidence: { type: 'string' }, detail: { type: 'string' },
      severity: { type: 'string', enum: ['critical', 'major', 'minor'] }, put_in: { type: 'string' } }, required: ['title', 'evidence', 'detail', 'severity'] } },
  },
  required: ['checked', 'omissions'],
}

const LENSES = [
  ...Array.from({ length: 6 }, (_, i) => ({ key: `strings${i + 1}`, title: `user-visible strings ${i + 1}/6`,
    brief: `Run  python3 ${TOOLS}/inventory.py items strings ${i + 1} 6  -> string/plural resource keys (moko resources in i18n*/…/strings.xml, plurals.xml). For each key read its text and find where it is used in the source (rg for the key name, e.g. MR.strings.<key> or AYMR.strings.<key>); the text implies a feature, dialog, toast, snackbar, notification, error or setting. Check the plans describe that feature/message (its trigger and conditions).` })),
  { key: 'docs', title: 'README/CHANGELOG/fastlane claims', brief: `Read README.md, CHANGELOG.md, CONTRIBUTING.md and fastlane/metadata/**; every feature, fix or behaviour mentioned must be traceable in the plans (search for it).` },
  ...[1, 2].map((i) => ({ key: `models${i}`, title: `ScreenModels/ViewModels ${i}/2`,
    brief: `List files with:  rg --files ${SRC} -g '*ScreenModel.kt' -g '*ViewModel.kt' -g '*StateScreenModel*' | sort | awk 'NR%2==${i % 2}'  . For each, enumerate every state field, user action/event, side effect, dialog type and navigation event; check the plans capture each (search by name and by behavior).` })),
  { key: 'workers', title: 'background work and notifications', brief: `Find every worker/job/service/receiver/alarm/foreground service/notification channel/wakelock/periodic schedule (CoroutineWorker, WorkManager, Notifications.kt, NotificationReceiver, Service classes, AlarmManager, BroadcastReceiver). Check trigger conditions, constraints, backoff, intervals, notification contents/actions/ids and the Linux replacement are all planned.` },
  { key: 'errors', title: 'error handling and user-visible failures', brief: `Sample broadly: every try/catch, runCatching, throw, require/check, toast/snackbar/dialog error path, retry/backoff and logged error in app/, data/, domain/, core/, source-api/. Check the plans describe each failure mode, message key and recovery behaviour.` },
  { key: 'compat', title: 'data migration and compatibility', brief: `DB migrations (.sqm), in-app migrations (mihon/core/migration), backup versions and legacy formats, preference key migrations, folder layout for downloads/covers/backups/local source, file name sanitising. Check the plans give an exact compatibility story so an existing Aniyomi install/backup/folder can be adopted.` },
  { key: 'intents', title: 'intents, deep links, sharing, file handling', brief: `Every Intent action/extra, deep link scheme/host/path, share target, clipboard use, URI parsing, file/MIME association (.tachibk, .cbz, .zip, .epub, video types), shortcut, search intent, external-player intent and ContentProvider usage. Check the Linux equivalent (xdg-open, .desktop, MIME, D-Bus, CLI args) is planned for each.` },
  { key: 'input', title: 'input, gestures, display and system integration', brief: `Hardware/volume keys, tap zones, gestures, haptics, brightness, orientation, fullscreen/insets, keep-screen-on, audio focus/media session/notification controls, PiP, tablet layout, accessibility labels. Check each has a desktop equivalent or a drop decision (MPRIS, idle inhibit, keyboard shortcuts, window fullscreen).` },
  ...[1, 2, 3].map((i) => ({ key: `consts${i}`, title: `constants/limits/regexes ${i}/3`,
    brief: `List files with:  rg -l 'const val|Regex\\(|toRegex|delay\\(|debounce|timeout|TimeUnit|\\.seconds|\\.minutes|\\.days|MAX_|MIN_' ${SRC} --glob '*.kt' | sort | awk 'NR%3==${i % 3}'  . Extract every numeric constant, timeout, threshold, size/cache limit, interval, regex pattern and magic string; check the plans contain the exact value and meaning.` })),
  { key: 'tests', title: 'tests and fixtures', brief: `Every test file (rg --files ${SRC} -g '*Test*.kt' and src/test dirs): each test case's input/expected output must be reproduced in the plans' test sections.` },
  { key: 'mockup', title: 'mockup spec vs plans', brief: `Read /home/blackbox/kurobayomi/spec/*.json (settings items, screens, sheets, dialogs, toolbar/menu entries). For EVERY settings item and every screen/sheet/dialog, find the behaviour that backs it in the plans (search by label text or key). Report items with no backing behaviour or task.` },
  { key: 'locale', title: 'locale, formatting, RTL', brief: `Date/time/number/size/duration formatting, relative timestamps, plurals, locale lists, RTL handling in reader/UI, fonts, text sorting/natural sort, language codes. Check each is planned with the Rust/GTK equivalent (chrono/icu/gettext…).` },
  { key: 'security', title: 'security and privacy', brief: `Incognito mode, secure screen, app lock/biometrics, cookie and credential/token storage, TLS/trust settings, extension trust/signature checks, DoH, user agent handling, log redaction, permissions. Check each is planned with a Linux equivalent (libsecret, portals, etc.).` },
  { key: 'perf', title: 'performance and resource limits', brief: `Caches (disk/memory), image decode sizes, paging, DB indexes/pagination, preloading, concurrency limits, dispatcher choices, throttling/rate limiting, battery/data-saver features. Check plans carry the exact numbers and a Rust approach.` },
  { key: 'ecosystem', title: 'source/extension ecosystem details', brief: `Source id generation, language codes, extension metadata (pkg name, version, nsfw flag, signature), repo index formats, multisrc, trust flow, update checks, source preferences (ConfigurableSource), latest/popular/search/filter protocols, stub sources. Check exact algorithms and formats are in the plans.` },
  { key: 'integrations', title: 'external integrations', brief: `mpv (config, scripts, fonts, assets/aniyomi.lua), ffmpeg-kit, external players, torrent server (TorrServer, trackers), NanoHTTPD player server, AniSkip, Discord links, Shizuku, Cloudflare/WebView bypass, image libraries (decoders/subsampling), archive libs. Check each is planned with a Linux equivalent.` },
]

const VERIFIED = { type: 'object', properties: { confirmed: { type: 'array', items: { type: 'object', properties: {
  title: { type: 'string' }, evidence: { type: 'string' }, detail: { type: 'string' }, severity: { type: 'string', enum: ['critical', 'major', 'minor'] }, put_in: { type: 'string' } }, required: ['title', 'evidence', 'detail', 'severity'] } }, rejected: { type: 'integer' } }, required: ['confirmed', 'rejected'] }

function lensPrompt(l, round, seen) {
  return `${CTX}

YOU ARE AN ADVERSARIAL COMPLETENESS AUDITOR, lens "${l.title}", round ${round}. The plans in ${PLANS}/ (area folders NN-*/, catalogs/, addenda/) claim to capture everything needed to port Aniyomi to Rust. Find what they MISS or get WRONG, starting from the SOURCE, not from the plans.
LENS: ${l.brief}
Method: enumerate the relevant source items with shell tools (rg/grep/find); for each, search the plans (rg -F -i in ${PLANS}, excluding _tools) for the name or a distinctive detail — try synonyms before concluding it is missing. Report only genuine omissions (behaviour/detail not documented) or documented-incorrectly items. No style opinions. Every report needs evidence (source path:line) and enough detail that a patcher can document it without re-discovering it. 'put_in' = best area folder for the fix.
${seen.length ? `Already confirmed in earlier rounds (do not re-report): ${seen.slice(-80).join(' | ')}` : ''}
Return how many source items you checked and the omissions.`
}

phase('Lens sweeps')
const seenTitles = []
for (let round = 1; round <= 3; round++) {
  const results = await pipeline(
    LENSES,
    (l) => agent(lensPrompt(l, round, seenTitles), { label: `lens ${l.key} r${round}`, phase: 'Lens sweeps', schema: FOUND, effort: 'high' }),
    (found, l) => {
      if (!found || found.omissions.length === 0) return { confirmed: [] }
      return agent(`${CTX}

YOU ARE THE SKEPTIC. The auditor (lens "${l.title}") claims the plans miss the items below. Try to REFUTE each one: re-open the source evidence, search the plans again with synonyms and different keywords (rg -F -i in ${PLANS}); an item is confirmed only if it is a real behaviour/detail of Aniyomi that the plans do not (correctly) document. Correct the detail text where the auditor was imprecise. Default to rejecting when unsure.
ITEMS:
${JSON.stringify(found.omissions)}`, { label: `verify ${l.key} r${round}`, phase: 'Lens sweeps', schema: VERIFIED, effort: 'high' })
    },
  )
  const confirmed = results.filter(Boolean).flatMap((r) => (r && r.confirmed) || [])
  log(`lens round ${round}: ${confirmed.length} confirmed omissions`)
  if (confirmed.length === 0) break
  confirmed.forEach((c) => seenTitles.push(c.title))
  const groups = []
  for (let i = 0; i < confirmed.length; i += 10) groups.push(confirmed.slice(i, i + 10))
  await parallel(groups.map((g, gi) => () => agent(`${CTX}

${DOC_RULES}

YOU ARE A PATCHER (lens round ${round}, group ${gi + 1}/${groups.length}). These omissions were found and confirmed. For EACH: open the source evidence, then document it fully (behaviour, constants, edge cases, Rust equivalent) in ${PLANS}/addenda/lens-r${round}-${gi + 1}.md, add implementation tasks in ${PLANS}/addenda/lens-r${round}-${gi + 1}-tasks.md (IDs T-lens-r${round}g${gi + 1}-001, ...; lint it with python3 ${TOOLS}/tasks.py lint), and where the item clearly belongs to an existing area doc also fix/extend that doc in place (and append one pointer line to that area's 00-overview.md under a heading "Addenda").
ITEMS:
${JSON.stringify(g)}`, { label: `lens-patch r${round} g${gi + 1}`, phase: 'Lens sweeps', effort: 'high' })))
}

// ---------------- stage 6: final verification and repair ----------------
const VERIFYR = { type: 'object', properties: { problems: { type: 'integer' }, report: { type: 'string' } }, required: ['problems', 'report'] }
phase('Final verify')
for (let round = 1; round <= 4; round++) {
  const v = await agent(`Run: python3 ${TOOLS}/inventory.py counts >/dev/null; python3 ${TOOLS}/verify.py  . Return the number after "PROBLEMS:" as problems and the complete output as report.`, { schema: VERIFYR, effort: 'low', phase: 'Final verify', label: `verify r${round}` })
  if (!v) break
  log(`final verify round ${round}: ${v.problems} problems`)
  if (v.problems === 0) break
  await agent(`${CTX}

${DOC_RULES}

FINAL REPAIR (round ${round}). verify.py reported these problems about the plans in ${PLANS}. Fix every one at its root cause: missing coverage (document the items properly in ${PLANS}/addenda/ — real content, not mentions), task lint/dangling dependencies, missing/short files, placeholders, broken links, milestone assignment (re-run ${TOOLS}/assign_milestones.py after task changes), index gaps (update 00-INDEX.md). Re-run python3 ${TOOLS}/verify.py until it prints PROBLEMS: 0 or you cannot fix more; report what remains.
REPORT:
${v.report}`, { label: `repair r${round}`, phase: 'Final verify', effort: 'high' })
}
return 'plan workflow finished; run python3 ' + TOOLS + '/verify.py to see the final state'
