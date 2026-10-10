# Kurobayomi: decisions, risks and open questions

Synthesis of the area plans in this directory. Inputs: every `plans/NN-*/00-overview.md`, the catalogs in `plans/catalogs/`, the addenda in `plans/addenda/`, and `plans/TASKS.csv` (3,801 tasks). Source paths are repo-relative to `/home/blackbox/aniyomi-src`.

Labels: **OBSERVED** = what the Kotlin does. **Proposal:** = what Kurobayomi should do.

## 0. Summary

- The plan covers 54 area directories (01 to 54, plus 47b and 49b). Every area has a `00-overview.md` and a `p1-tasks.md`. All `p1-tasks.md` files pass `tasks.py lint`.
- Task counts in `TASKS.csv`: P0 1,196, P1 1,793, P2 503, P3 309. Effort: S 2,448, M 1,234, L 118, XL 1.
- **Scope risk:** 1,196 P0 tasks is far more than the P0 definition needs (library, details, reader, player, progress, local DB, essential settings, one source mechanism). Section 3 lists the P0 demotions the area docs already propose. Apply them before scheduling.
- Eleven decisions in section 2 block the critical path. Two of them (extension strategy and video backend) change the largest areas.

## 1. Decision register (ADR style)

Each decision has options, a recommendation, and consequences. Owner is the user unless stated.

### ADR-01 Extension strategy

- **Context:** Aniyomi extensions are Android APKs loaded through `dalvik.system.PathClassLoader` and Android package APIs. They cannot run on Linux. Area 20 (`20-extension-strategy-redesign/00-overview.md`) and area 18 (`18-extensions-manga/p1-rust-design.md`) propose a three-stage path: built-in sources, then TOML scrapers, then WASM components.
- **Options:**
  1. Run the Kotlin APKs through a JVM. Rejected: Android framework and Kotlin Android libraries are needed, and the APKs are not portable.
  2. Built-in Rust sources only. Simple and safe, but a few sources only.
  3. WASM components (`wasmtime` + WIT) as the extension format, with `.kuroext` zip packages (manifest.json, source.wasm, optional icon.png) and ed25519 signatures.
- **Recommendation (documented in areas 18 and 20):** built-in sources first (D0a and Q3 in area 20), then WASM. Use `wasmtime` with the component model. Read `.kuroext` in memory, never extract it.
- **Consequences:**
  - Existing Aniyomi APK extensions stop working. Library data (ids, titles, chapters, history) carries over, and sources show as stubs (area 20 Q1).
  - Sources must be ported one by one. Rank by library entry count from a backup (area 20 Q2).
  - WASM needs `allowed_hosts` in the manifest, or any extension reaches any host (area 20 Q8).
  - Priority change needed: `T-20a-001` (WASM loader) is P0 but is not needed for daily use. Move it to P1, or split it into a P0 built-in loader and a P1 wasmtime host (area 20 Q3).
- **Open:** user accepts that Aniyomi APK extensions stop working (area 20 Q1); which sources to port first (area 20 Q2).

### ADR-02 Video backend and player

- **Context:** Aniyomi's player is mpv-based (`app/src/main/java/eu/kanade/tachiyomi/ui/player/PlayerActivity.kt`). Area 45 (`45-player-core/00-overview.md`) and area 46 cover the player.
- **Options:**
  1. libmpv through the `libmpv` C API (a thin own Rust wrapper, or an existing binding crate after review). Keeps mpv's rendering, decoding and Lua scripting, and matches Aniyomi's custom-button Lua behaviour.
  2. GStreamer. Rejected: no Lua, and a different key and property model.
  3. Spawn `mpv` as an external process with IPC. Simpler, but no embedded video in the GTK window.
- **Recommendation:** libmpv embedded in the GTK4 window with the GL render context. Keep mpv Lua scripting for custom buttons (area 48). External player is a command template with `$URL` (area 02 Q-F).
- **Consequences:** the build needs `libmpv` (system package on Arch: `mpv`). Custom button Lua is run inside mpv, so restoring a backup can run arbitrary Lua (area 06 Q6, `T-06a-061`). The restore needs a confirmation step.

### ADR-03 Torrent playback engine

- **Context:** Area 45 (`45-player-core/00-overview.md`, D-2) and area 13 (`13-core-util-storage-archive-torrent/00-overview.md`) propose either a TorrServer (Go) process or `librqbit` in-process.
- **Recommendation:** `librqbit` by default, TorrServer as fallback. Both must produce the same play-link shape (area 45 `p1-data-and-config.md` 6.1).
- **Priority:** all torrent tasks (`T-45a-072` to `T-45a-079`) are P2. Not needed for daily use. Decide before `T-45a-073`.
- **User question (area 13 Q1):** is it acceptable that torrent playback needs a separately installed TorrServer binary on Linux? Recommendation: yes.
- **Open detail:** tracker list push (OQ-13a-1), multi-file index 0 vs 1-based (OQ-13a-2), proxy mode `None` mapping (OQ-13a-3).

### ADR-04 Webview, Cloudflare and user agent

- **Context:** Aniyomi uses an Android WebView for Cloudflare challenges and login (`WebViewUtil.kt`, regex `; Android .*?\)` in the user-agent transform, OQ-13a-4). Area 12 (`12-network-stack`) and area 36 (`36-ui-shell-navigation-security-webview/00-overview.md`) cover this.
- **Options:** WebKitGTK (`webkit6` crate for GTK4) for the challenge window, and `reqwest` for normal traffic. A Cloudflare clearance cookie is tied to the user agent, so both must send the same string.
- **Recommendation:** keep the Android-spoof transform for parity, and use the same string in WebKitGTK and in `reqwest` (OQ-13a-4).
- **Consequences:** WebKitGTK is a large system dependency. Cloudflare-protected sources fail with a clear message when WebKitGTK is missing. Only `reqwest` paths work without it.

### ADR-05 Database engine and layout

- **Options:** `rusqlite` with the bundled SQLite 3.53.2 (`libsqlite3-sys`), or the system `libsqlite3`.
- **Recommendation:** bundled SQLite (area 03 Q5, decision D5). Adds a C compile.
- **Layout:** `<data_dir>/anime.db` for anime, with `data_dir` from `kuro-platform::Paths` (Linux: `$XDG_DATA_HOME/kurobayomi`) (area 04 Q8). The manga database is in the same data directory (area 03).
- **Compatibility:** keep Aniyomi/Mihon backup interchange (area 03 Q1, default yes). This keeps the mixed time units (seconds for `last_modified_at` and `favorite_modified_at`, milliseconds elsewhere) (area 03 Q3).
- **Consequences:** the genre codec joins with `", "` and splits on read, which loses genres that contain `", "` (area 04 Q3, area 05 EC-19). Accept and document.

### ADR-06 Backup format

- **Context:** Aniyomi backups are protobuf-based `.tachibk` files written by `app/src/main/java/eu/kanade/tachiyomi/data/backup/`. Areas 21 and 22 cover create and restore.
- **Recommendation:** read and write the Aniyomi-compatible format. Use a `kuro_` key prefix for Kurobayomi-only settings (area 02 Q-G), so Aniyomi restores them harmlessly.
- **Format:** Aniyomi backups are protobuf-encoded (`.tachibk`; `app/src/main/java/eu/kanade/tachiyomi/data/backup/BackupDecoder.kt`, `BackupCreator.kt`).
- **Consequences:** `__APP_STATE_*` keys: exclude from backups (area 17 Q3, proposal). Trust keys are not carried into backups (area 18 D0d).

### ADR-07 Tracking services

- **Context:** Areas 28 to 33 cover AniList, MyAnimeList, Kitsu, Shikimori, Bangumi, MangaUpdates, Simkl, and the enhanced trackers (Kavita, Komga, Suwayomi, Jellyfin).
- **Recommendation:** port all trackers, at P1 (tracking is listed as a P1 parity feature). Keep the Simkl custom scheme `aniyomi://simkl-auth` and register `x-scheme-handler/aniyomi` in the `.desktop` file (area 32 O-6).
- **Credential storage:** Secret Service via `secret-service` or `oo7` crate. Store username, password and token (area 02 Q-B, `T-02a-018`).
- **Open:** MangaUpdates "not on list" detection: 404 vs 200 empty body (area 32 O-1, confirm live with TST-74). `index_to_score` returns `Result<f64, TrackError>` (area 32 O-2).

### ADR-08 Downloads

- **Recommendation:** honour `download_slots` (default 1, UI 1 to 5) as the parallel download count (area 25 Q1). Apply `download_speed_limit` to anime downloads as well (area 25 Q2, the Kotlin anime downloader ignores it).
- **Storage:** permanent deletion, no trash (area 25 Q4).
- **Orphans:** keep files of uninstalled sources, log a warning, add a P2 "Clean orphaned downloads" action (area 25 Q3).

### ADR-09 App lock and authentication

- **Options:** no lock, a passphrase (argon2id hash in the keyring), or fprintd biometric.
- **Recommendation:** lock is opt-in, default off, passphrase only, P1. fprintd later (area 36 Q2). The area 02 recommendation is P2 with a passphrase (area 02 Q-C).
- **Consequence:** a forgotten passphrase has no recovery path. Settings help text must say so (risk R-11).

### ADR-10 Update checks and distribution

- **Recommendation:** stable channel only. Preview channel dropped (area 35 Q3, `T-35a-061`). Update source is setting `update.repo` (`owner/name`), empty by default, which disables checks (area 35 Q4). `update.auto_check` defaults to `true` (area 35 Q5). Update button hidden for package-managed installs (area 35 Q9, `T-35a-055`). Release check throttled to 3 days (area 06 Q4).
- **Do not point at Aniyomi.** `T-36a-022` and `T-36a-023` hard-code `https://github.com/aniyomiorg/aniyomi/releases/tag/v<version>`. Change to the Kurobayomi repository (area 36 Q3).
- **Distribution:** distro packages are the update path. In-app APK download dropped (area 06 Q5). Packaging decisions (`54-build-release-distribution-docs`): libarchive system dependency, optional `libheif`, TorrServer binary path `<data>/bin/torrserver` (area 13 section 5).

### ADR-11 Settings scope on Linux

- **Recommendation:** grey out disabled settings, do not hide them (area 48 D-01). Keep `AppTheme` and AMOLED settings stored, with a note that the GTK theme sets colours (area 36 Q7). Keep the screenshot-block setting with text "Not available on Linux" (area 34 Q-1). Keep the four NavStyle variants because the names are in backups (area 36 Q6).
- **Theme:** default `DEFAULT` (inherit the user's GTK theme). Confirm the GTK theme is Tokyo Night (area 02 Q-A).

## 2. Blocking questions for the user

Answer these before the listed tasks start. The recommendation applies if there is no answer.

| # | Question | Blocks | Recommendation |
|---|---|---|---|
| U-1 | Do you have an Aniyomi library (`tachiyomi.db` or a backup) to bring over? (area 03 Q2) | Promotes `T-03a-022`, `T-03a-023` from P2 to P1 | Yes, if you have one. Schedule before daily use. |
| U-2 | Accept that Aniyomi APK extensions stop working? (area 20 Q1) | ADR-01, area 18 and 19 | Yes. |
| U-3 | Which sources do you use? Export a backup and count library entries per source (area 20 Q2). | Source port order in areas 15, 16, 18, 19 | Port the top sources by entry count. |
| U-4 | Is WASM needed for the first usable release? (area 20 Q3) | `T-20a-001` priority | No. Move to P1. |
| U-5 | Do you watch torrent-sourced anime? (area 39 Q-5) | `T-39b-052`, area 45 torrent tasks | No. Skip for v1. |
| U-6 | Is the app lock in v1? (area 36 Q2) | `T-36a-032`, `T-36a-033`, `T-36a-034` (all large) | Opt-in, default off, passphrase, P1. |
| U-7 | Cancel on the lock overlay: stays or closes the app? (area 36 Q1) | Lock state machine | The overlay stays; the app stays. The two docs disagree, so confirm. |
| U-8 | Do you want Kurobayomi to check for updates, and against which GitHub repository? (area 35 Q4) | `T-36a-022`, `T-36a-023`, `T-35a-*` | Set `update.repo` to the Kurobayomi repository when it exists. Empty until then. |
| U-9 | Keep Kurobayomi-only settings in backups with `kuro_` prefix? (area 02 Q-G) | Backup key set | Yes. |
| U-10 | Keep the Simkl client secret in the binary? (area 32 O-7) | Simkl build config | Yes. |
| U-11 | Keep Unicode lowercasing in `source_id`? (area 20 Q6) | Source id derivation, frozen ids in backups | Yes (`str::to_lowercase`). Non-ASCII ids exist. |
| U-12 | Use Sunday or Saturday as week start for Upcoming? (area 42 D-24) | Upcoming screen | Locale table in area 42 D-24. |
| U-13 | Should the "Delete downloaded" box in the migrate dialog be pre-checked? (area 41 Q2) | Migrate dialog | OFF, because it deletes data. |
| U-14 | Should clear-all history keep `time_read`? (area 42 D-18, `DELETE FROM history` drops "Read duration") | History clear | Keep Aniyomi semantics; confirm. |

Owner notes:
- Questions on chip filters, data saver and migrate (area 40 Q1 to Q3) are UI-level. Defaults are in the area docs.
- Data saver (`BANDWIDTH_HERO`, `WSRV_NL`, `RESMUSH_IT`) sends image URLs to third-party services. Area 17 Q2 and area 40 Q2 recommend dropping the three services and keeping the `NONE` value so stored settings still parse.

## 3. Priority corrections

Priority problems to fix before scheduling. The area docs proposed each one.

| Task | Current | Proposed | Source |
|---|---|---|---|
| `T-20a-001` WASM loader | P0 | P1 (or split: built-in loader P0, wasmtime host P1) | area 20 Q3 |
| `T-02a-036` value ranges | P1 | P0 (S effort; widgets need it) | area 02 Q-I |
| `T-02a-038` shutdown flush | P1 | P0 (a quit within 250 ms otherwise loses the change) | area 02 Q-I |
| `T-05a-025` category create/rename/hide/delete | P1 | P0, or ship P0 with the ui-only subset (`T-05a-026` needs the logic) | area 05 Q3 |
| `T-03a-022`, `T-03a-023` library import | P2 | P1 if U-1 is yes | area 03 Q2 |
| `T-34a-028` AVIF and JPEG XL | P0 | P0 only if the user's sources serve AVIF or JXL, otherwise P1 | area 34 Q-4 |
| `T-34a-029` HEIF | P2 | P2, feature `heif` off by default | area 34 Q-3 |
| `T-39b-052` torrent server start | P2 | P2, skip for v1 unless U-5 is yes | area 39 Q-5 |
| `T-35a-*` notifier chain | P1 | P0 only if download, library-update and backup notices must work on the first daily-use build | area 35 Q12 |
| `T-06a-070`, `T-06a-073` release check | P1 | keep, informational, behind build constant `UPDATE_REPOSITORY` | area 06 Q4 |
| `T-08a-*` Glance widgets | in plan | drop (area 08 Q-J); replace later with KDE Plasma widget or tray menu | area 08 Q-J |
| `T-25a-026` queue keyboard shortcuts | P2 | P1 (desktop-only, Alt+Up, Alt+Down, Delete, Space) | area 25 Q6 |

Also: area 04 `T-04a-025`, `T-04a-026` (raw `tachiyomi.animedb` import) are P2. Backup restore is enough for most users (area 04 Q1).

## 4. Dropped or replaced Android features

Each row states the replacement or the drop, and what the user loses. Source: area overviews.

| Aniyomi feature | Decision | What the user loses | Source |
|---|---|---|---|
| Android APK extension install and installer service | Dropped. Replaced by WASM/built-in sources. | Existing APK extensions | area 18 D0g, area 20 Q1 |
| Extension installer preference (`extension_installer`), MIUI workaround | Dropped | None (Android-only) | area 18 D0g |
| Local HTTP server for sources, `ffmpegStreamArgs`, `ffmpegVideoArgs` | Dropped | Source-specified ffmpeg tweaks and sources needing the local proxy | area 20 Q10 |
| Home-screen widgets (Glance) | Dropped for now. Later: KDE Plasma widget or tray menu. `UpdatesRepo::by_status` stays available. | Home-screen updates widget | area 08 Q-J |
| Android notification channels and deep links | Replaced by desktop notifications (freedesktop `org.freedesktop.Notifications` over D-Bus, via `notify-rust`). Notice click does not start the app (area 35 Q1). | Starting the app from a notice | area 35 Q1 |
| In-app APK update download and preview channel | Dropped. Distro packages update the app. | In-app update, preview builds | area 35 Q3, area 06 Q5 |
| Display cutout handling (`reader` D-6) | Hidden. No Linux equivalent. | Cutout padding | area 43 D-6 |
| Biometric app lock (Android BiometricPrompt) | Replaced by passphrase (ADR-09). fprintd later. | Biometric unlock | area 36 Q2 |
| `FLAG_SECURE` screenshot block | Setting kept, shows "Not available on Linux" | Screenshot blocking | area 34 Q-1 |
| ICC display profile | Dropped for P0 and P1. Optional `icc` feature at P2 (`T-34a-031`). | Colour-managed display | area 34 Q-2 |
| Android WebView user-agent transform | Kept (parity), via WebKitGTK (ADR-04) | None | area 13 OQ-13a-4 |
| Data saver (`bandwidth-hero`, `wsrv.nl`, `resmush.it`) | Dropped. `NONE` kept so settings parse. | Image proxying | area 40 Q2, area 17 Q2 |
| Migrate: pre-checked delete of downloads | Keep, default OFF (U-13) | None if default OFF | area 41 Q2 |
| Clear database (anime reset) `reset_viewer_flags` UI | No UI. Method kept, tested as destructive. | UI for reset flags | area 05 Q4 |
| Random sort exact shuffle | Accepted different order (ChaCha8). | Same order as Aniyomi for the same seed | area 08 Q-H |
| Platform `ImageUtil` and `ImageDecoder` JVM paths | Replaced by `image` and `avif-decode`, `jxl-oxide` crates | None beyond format list in area 34 | area 34 |

## 5. Risk register

Likelihood (L) and impact (I) are H, M or L. Each risk names the task or area that carries the mitigation.

| ID | Risk | L | I | Mitigation | Tasks / area |
|---|---|---|---|---|---|
| R-01 | P0 scope inflation: 1,196 P0 tasks delays daily use | H | H | Apply section 3 corrections; ship a P0-only milestone and measure | TASKS.csv, section 3 |
| R-02 | Two authors drift on module names. Area 45 D-1 names two layouts that clash (`state.rs`, `model.rs`, `progress.rs`). | H | M | Pick one layout (45 D-1) before the first P0 task of that area | area 45 section 3 |
| R-03 | Repository overlap: areas 04, 05 and 06 each specify Rust repository functions | H | M | Area 04 owns the SQL and codecs. Areas 05 and 06 own only their own functions. | area 04 Q7 |
| R-04 | Scale-type "Stretch" label maps to crop-to-fill (`CENTER_CROP`) | H | L | Keep behaviour; rename help text "Fill (crop)" (area 44 Q2) | area 43 D-1, area 44 Q2 |
| R-05 | Source id changes break backups (ids are frozen) | M | H | Keep Unicode lowercasing; add Turkish dotted-I and non-ASCII golden tests | area 20 Q6, area 18 |
| R-06 | Custom button Lua restored from backup runs arbitrary code in mpv | M | H | Restore confirmation listing button names; per-button `pcall` isolation (area 48 D-04) | `T-06a-061`, area 22 |
| R-07 | Cloudflare clearance tied to user agent; WebKitGTK missing or outdated | M | M | Same UA string in WebKitGTK and `reqwest`; clear failure message | ADR-04, area 36, area 12 |
| R-08 | Sources need WASM sandbox escape or unbounded network | M | H | `allowed_hosts` manifest field; ed25519 signatures on repos and packages; WASM limits | area 20 Q8, area 18 D0c |
| R-09 | Repo signing key compromise or user installs untrusted repo | L | H | Two-tier rule (repo sha256 plus repo key) as in Aniyomi (area 20 Q7); trust keys `pkg:versionCode:digest` (area 18 D0d) | area 18 |
| R-10 | Data loss from destructive actions: clear history, delete downloads, delete custom images, delete category | M | H | Confirmation where Aniyomi has none (area 42 D-6 Updates "Delete" needs confirm, area 39 OD-3); permanent delete documented | area 25 Q4, area 39 OD-3, area 42 D-6 |
| R-11 | Forgotten app-lock passphrase with no recovery | M | H | Opt-in, default off; help text; no biometric fallback | ADR-09, `T-36a-033` |
| R-12 | Library database corruption | L | H | `quick_check` on start with quarantine dialog (area 04 Q9, P1); busy_timeout 5000 (area 05 Q7) | `T-04a-031` |
| R-13 | Aniyomi backup restore changes unexpected data (e.g. `time_read` reset by clear) | M | M | Keep Aniyomi semantics; document; confirm (U-14) | area 42 D-18 |
| R-14 | Anime history has no duration column; reading time counts are unbounded | M | L | Clamp reading duration to 0..12 h per flush (area 08 Q-G, `T-08a-019`) | area 06 Q7, area 08 |
| R-15 | Image memory overrun on very large pages | M | M | Decoded budget 64 MiB; tiling decision after measuring the largest real page (area 34 Q-8, area 44 Q7) | `T-44a-*`, area 34 |
| R-16 | Browse crash on `TriState` unknown integers (Kotlin crash E-FD-01, E-FD-03) | L | M | Map unknown to `Ignore` with a warning (area 41 D-4) | area 41 |
| R-17 | Stale search results overwrite newer ones | M | L | Generation guard (area 41 D-5); shared semaphore of 5 permits (area 41 D-6) | area 41 |
| R-18 | Torrent-server dependency missing on Linux | M | L | P2 only; TorrServer fallback; clear error (area 13 section 5) | area 45, area 13 |
| R-19 | Upstream tracker APIs change shape (Simkl, MangaUpdates) | M | M | Record fixtures on first live run (area 32 O-8, TST-74..TST-78); DTOs follow Kotlin | area 32 |
| R-20 | MangaUpdates "not on list" detection wrong (404 vs 200) | M | M | Treat 404 as not on list, other errors as errors (area 32 O-1) | `TST-74` |
| R-21 | Simkl redirect needs a desktop URL scheme handler | M | L | Register `x-scheme-handler/aniyomi`; loopback redirect as fallback (area 32 O-6) | area 32 |
| R-22 | Locale collation: wrong sort for non-Latin titles | M | L | Byte order first (area 41 Q-3); ICU4X only if testing shows bad sorts (area 17 Q1) | area 17 Q1, area 41 |
| R-23 | Random sort and seeded shuffle cross-check with Aniyomi fails | L | L | Accept different order (area 08 Q-H) | area 08 |
| R-24 | Clock or timestamp unit confusion (seconds vs milliseconds) | M | M | Helper names `secs` and `millis` (area 03 Q3) | area 03 |
| R-25 | Lost Android-only features users expect (widgets, app-lock biometrics, in-app update) | H | L | Documented drops (section 4) | section 4 |
| R-26 | WebKitGTK version dependency (GTK 4.16+, `webkit6`) on the user's CachyOS | L | M | Pin minimum; check at build | `54-build-release-distribution-docs` |
| R-27 | libmpv API version differs from the bindings used | M | M | Pin the libmpv version in build docs; smoke test in CI | area 45 |
| R-28 | Migration path from an Aniyomi library may import data Kurobayomi cannot validate (old schema) | M | M | Legacy migration chain tested only on reconstructed v113 baseline; real file needed (area 04 Q1) | area 04 |
| R-29 | Image format coverage gaps (HEIF, AVIF) | L | L | HEIF feature off by default; AVIF per U-decision (area 34 Q-4) | area 34, area 13 OQ-13a-5 |
| R-30 | Keeping Aniyomi's "Auto" audio mapping reverses channels | M | L | Fix the mapping and note in release notes (area 48 D-02) | area 48 |

## 6. Legal and licensing

- **Source license:** Apache License 2.0 (`LICENSE` at the root of `/home/blackbox/aniyomi-src`, confirmed). The repository root has no `NOTICE` file, so Apache-2.0 section 4(d) does not require one, but section 4(a) to (c) still apply to any derived source: keep the license text, keep copyright notices, and state changes in modified files.
- **Proposal:** add `THIRD_PARTY_NOTICES.md` to the Kurobayomi repository listing Aniyomi (Apache-2.0, `https://github.com/aniyomiorg/aniyomi`), Mihon/Tachiyomi lineage notices as they appear in the source headers, and each Rust dependency's license (generate with `cargo-about`).
- **Proposal:** each file ported from Aniyomi keeps its original copyright header. Files rewritten from scratch need only a Kurobayomi header. Record provenance per file in the area inventories (`p1-inventory.md`, which already lists "Port" vs "Rewrite").
- **Trademark:** do not use "Aniyomi" in the product name, icons, or update URLs as a brand. Area 36 Q3 already rejects pointing update checks at Aniyomi releases.
- **Extension ecosystem terms:** third-party extension repositories and sources are not licensed by Aniyomi. Kurobayomi must not bundle extension code. Show a first-run notice that sources are third-party and that the user is responsible for the content they access.
- **Copyrighted content:** manga and anime sources can serve unlicensed copies. The plan does not add content, does not host it, and does not ship any default repository (area 20 Q4: ship none). Keep it that way in documentation.
- **Simkl and MangaUpdates APIs:** API terms are not in the repository. Check each provider's current terms before public distribution. The Simkl client secret is public-equivalent for a desktop client (area 32 O-7).
- **Data saver services:** the three third-party image services (`bandwidth-hero`, `wsrv.nl`, `resmush.it`) are dropped (section 4), so no user data goes to them.
- **GPL and LGPL components:** `libmpv` (LGPL/GPL depending on build), `libarchive` (BSD), `SQLite` (public domain), `wasmtime` (Apache-2.0 with LLVM exception). Check the libmpv build flags before distributing a binary; a GPL build changes licensing of the combined binary.

## 7. Cross-area ownership conflicts

From the overview "conflict" tables. Each row gives the owner.

| Item | Areas | Owner |
|---|---|---|
| Module layout (`crates/kuro-domain`, `kuro-data`, `src/ui/...`) vs flat `src/updates/` | 01, 42 | Area 01 layout (42 D-2) |
| Relative time formatting | 01, 42 | Area 01 `format::relative_time_span` (42 D-3) |
| Row list widget: `gtk::ListView` with `SignalListItemFactory` vs `gtk::ListBox` | 01, 42 | `gtk::ListView` everywhere (42 D-15) |
| Transient messages: `gtk::Overlay` toast vs `gtk::Revealer` | 01, 42 | One shared toast, owned by `T-42b-039`, 4 s (42 D-16) |
| Repo rows: SQLite `extension_repos` vs `repos.toml` | 03, 18, 20 | SQLite `extension_repos` (area 20 Q5) |
| Repository functions for history and anime | 04, 05, 06 | Area 04 owns SQL and codecs |
| Local source constructor `local_source(paths)` | 17, 14 | Area 14 exposes it (area 17 Q5) |
| Stub list visibility | 17, 14 | Show all persisted stubs (area 17 Q6) |
| Local source language `"other"` (code) vs `""` (docs) | 09, 17 | Follow the code: `"other"` (area 17 Q4) |
| `__APP_STATE_*` backup inclusion | 02, 17 | Exclude from backups (area 17 Q3) |

## 8. Verification checklist before first release

Each item is observable. Run before tagging a release.

1. `python3 /home/blackbox/kurobayomi/plans/_tools/tasks.py lint` passes for every `plans/NN-*/p1-tasks.md` (currently passing).
2. Every area directory has `00-overview.md`, and every overview has section 4 with decisions and open questions. Confirm with a directory scan.
3. Every P0 task ID in `TASKS.csv` maps to an area that has a P0 milestone. After the section 3 corrections, the P0 count is recalculated and recorded in this file.
4. Every open question in section 2 has an answer recorded in the owning area's overview, or its recommendation is applied.
5. A backup exported from Aniyomi, restored into Kurobayomi, keeps library, chapters, history, categories and tracks (areas 21 and 22 test vectors).
6. A custom mpv button list restored from a backup shows the confirmation before the Lua runs (R-06).
7. Lock passphrase flow: wrong passphrase, forgotten passphrase help text (R-11).
8. Downloads queue: `download_slots` 1 to 5, speed limit applies to anime downloads (ADR-08).
9. Update check disabled while `update.repo` is empty; enabled and throttled to 3 days once set (ADR-10).
10. Placeholder scan: run a grep over `/home/blackbox/kurobayomi/plans` for the banned markers listed in the document rules (open-task markers, filler phrases) and confirm zero matches in the files this document covers.

## 9. What this document does not decide

- Visual theme beyond "inherit the GTK theme" (area 02 Q-A, area 49 owns the theme).
- Packaging format. Decided by the user on 2026-10-10: Flatpak first (primary), an AppImage built by CI later. No `.deb`, RPM, AUR or any other OS-specific package from this project. The in-app updater only announces a new version; Flatpak updates itself and the plan already drops AppImage self-replacement. Area 54 owns the details. This document also records the constraints in ADR-10.
- Exact `wasmtime` version pin and WIT package version. Area 20 owns the contract (`p1-topic-wasm-component-design.md`).
