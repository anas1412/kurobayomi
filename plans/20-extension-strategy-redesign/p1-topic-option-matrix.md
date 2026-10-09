# Topic: option matrix for replacing APK extensions

Scored 1 (poor) to 5 (good). "Reuse" means reuse of the Aniyomi/keiyoushi ecosystem.

| Option | Security / sandbox | Performance | Authoring ergonomics | Ecosystem reuse | Legal / distribution | Linux fit | Verdict |
|---|---|---|---|---|---|---|---|
| **WASM component model (wasmtime + WIT)** | 5: no ambient access, fuel and epoch limits, memory cap | 4: AOT-compiled, call overhead small vs network | 3: Rust, Go, JS (via componentize) guests; WIT is typed | 1: no existing extensions, every source is a rewrite | Good: no vendor SDK, MIT/Apache toolchain | 5 | **Primary extension format** |
| **Lua (mlua)** | 3: sandbox by removing globals; no memory cap by default | 4 | 3: dynamic, weak typing, no shared types with Rust | 1 | Good (MIT) | 4 | Rejected: no type boundary, sandbox weaker |
| **JS (rquickjs)** | 3: QuickJS, memory limit available | 3 | 4: familiar, large authoring base | 2: some fetch-style code is portable | Good (MIT) | 4 | Rejected as primary; kept as optional guest behind the same WIT |
| **Declarative CSS / JSON scraper (TOML)** | 5: no code | 5 | 4 for simple sites, 2 for logic-heavy sites | 2: a port of ParsedHttpSource selectors is direct | Good | 5 | **Second source kind** |
| **JVM bridge (dex2jar + Android shim)** | 2: runs arbitrary bytecode in a JVM with Android API stubs | 3: JVM start cost and memory | 3 for the authors, 1 for us | 5 in theory: runs existing APK logic | Risky: APKs are third-party, and shimming Android APIs is a long-term maintenance cost; Aniyomi itself is Apache-2.0 but extension APKs carry their own licences | 2 | Rejected: too large a surface and not Linux-native |
| **Sidecar processes (any language over stdio/JSON)** | 3: process isolation, no memory cap | 3: IPC per call | 4 | 1 | Good | 4 | Rejected: WASM gives isolation without per-extension processes |
| **Statically linked Rust sources** | 4: compiled, but full process access | 5 | 2: a new release per source | 0 | Good | 5 | **First source kind (built-in)** |

## Measured facts used in the scoring

- Aniyomi's published index (`aniyomiorg/aniyomi-extensions`, branch `repo`) lists 3 entries on 2026-10-08, last commit 2024-07-11. Reuse of that index is near zero.
- keiyoushi (`keiyoushi/extensions-source`, branch `main`) has 407 directories in `src/en` and 134 in `src/all` (GitHub contents API, 2026-10-08). Every one is Kotlin against OkHttp, Jsoup, RxJava and Android APIs.
- Aniyomi's loader requires `tachiyomi.extension` (manga) or `tachiyomi.xextension` (anime) features and an Android package (MangaExtensionLoader.kt).

## Recommended path with phases

**Phase 0 (MVP, weeks):** `kb-source` types and traits, built-in Rust sources for the manga and anime sources the author uses, registry, stubs, backup id resolution (T-20a-001..012, T-20a-094/095).
**Phase 1 (weeks):** declarative TOML scrapers and the scraper engine (T-20a-052), repo index and trust (T-20a-020, T-20a-060), update flags and notification (T-20a-050, T-20a-098/099).
**Phase 2 (weeks):** WASM host, WIT world, install pipeline and watcher (T-20a-001 WASM branch, T-20a-030, T-20a-040), anime hoster and video flow (T-20a-096/097/100).
**Phase 3 (optional):** torrent backend (T-20a-070), resolvable URLs (T-20a-031), Unmetered (T-20a-013).

## Why not just keep Aniyomi APKs

Kurobayomi is a Linux app. Aniyomi extensions are signed Android APKs whose code calls Android framework classes. Running them needs an Android runtime or a shim. Both are large, and the shim would have to be updated for each extension's API use. The WASM path needs a one-time port per source, which the author controls.

## Known limits of the chosen design

- Users lose the ability to install existing Android APK extensions. Stubs keep their library entries, but the sources do not work until someone ports them.
- WASM components cannot use arbitrary native libraries. A source that needs a native JavaScript engine (for example to run a site's obfuscated player script) must call a host function; that function must be added to the WIT world first.
- The anime video path depends on libmpv. Sources that depend on Android-specific players do not map.
