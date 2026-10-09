# Rust design: build, packaging, CI and docs for Kurobayomi (area 54)

This replaces the Gradle build (`settings.gradle.kts`, `build.gradle.kts`, `buildSrc/`, `gradle/`, `macrobenchmark/`, `i18n*/`, `presentation-*`, `source-*`, `data/`, `domain/`, `core-metadata/`, `app/build.gradle.kts`) with Cargo, a small `xtask` crate, a Flatpak manifest, AppStream and desktop files, and GitHub Actions for Linux.

## 1. Starting point (OBSERVED in `/home/blackbox/kurobayomi`)

- Git repository with `.git`, `Cargo.toml` (package `kurobayomi` 0.1.0, edition 2024, dependencies `gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }`, `serde` 1.0.229, `serde_json` 1.0.151), `Cargo.lock`, `.gitignore` (8 bytes), `src/` (`clamp.rs`, `entry.rs`, `main.rs`, `player.rs`, `screens.rs`, `settings.rs`, `spec.rs`, `ui.rs`), `spec/` (`app.json`, `entries.json`, `reader_player.json`, `SCHEMA.md`, `settings.json`), `shots/`, `plans/`.
- The toolchain on this machine is `rustc 1.99.0` (Arch package).
- There is no `rust-toolchain.toml`, no `xtask`, no workspace, no CI, no Flatpak manifest, no metainfo, no desktop file, no `po/` directory.

Proposal: keep the root package `kurobayomi` where it is (no `src/` move). Turn the root into a workspace by adding a `[workspace]` table. New crates go under `crates/`. Crate boundaries are defined by the other areas (see section 3), so this area only creates the workspace and the build tooling.

## 2. Workspace layout (PROPOSAL)

```
kurobayomi/
  Cargo.toml                  # [package] kurobayomi + [workspace] + [workspace.dependencies] + [profile.*]
  Cargo.lock
  rust-toolchain.toml         # channel = "stable", components = ["rustfmt", "clippy"]
  rustfmt.toml
  deny.toml                   # cargo-deny: advisories, licenses, bans, sources
  build.rs                    # build metadata (KB_GIT_SHA, KB_COMMIT_COUNT, KB_BUILD_TIME, KB_APP_ID)
  .editorconfig
  .gitattributes
  .gitignore                  # /target, /.flatpak-builder, /repo, /build-dir, /dist
  README.md  CHANGELOG.md  CONTRIBUTING.md  CODE_OF_CONDUCT.md  LICENSE  NOTICE
  src/                        # kurobayomi binary (main.rs, ui.rs, screens.rs, ...)
  crates/
    kb-core/                  # paths (XDG), prefs file, logging (tracing), build info consts
    kb-db/                    # rusqlite schema, migrations, repos (areas 03-06)
    kb-domain/                # interactors and models (areas 07-09)
    kb-source-api/            # source traits and models (areas 15-16)
    kb-local/                 # local source and metadata (area 14)
    kb-metadata/              # ComicInfo and details.json parsing (area 14, core-metadata)
    kb-net/                   # reqwest client, cookies, rate limiting (area 12)
    kb-i18n/                  # gettext wrapper (area 51 strings, this area's build step)
    kb-ui/                    # GTK widget helpers (area 49, 44, 37-42)
  xtask/                      # cargo xtask: lint, i18n-check, dist, flatpak-lint, bench-report
  po/                         # gettext catalogues: LINGUAS, kurobayomi.pot, <lang>.po
  data/                       # io.github.OWNER.Kurobayomi.desktop, .metainfo.xml, icons/, schemas
  packaging/flatpak/          # io.github.OWNER.Kurobayomi.yml, cargo-sources.json (generated)
  scripts/bench-startup.sh    # hyperfine-based startup timing
  .github/workflows/ci.yml
  .github/workflows/release.yml
  .github/ISSUE_TEMPLATE/     # config.yml, report_issue.yml, request_feature.yml
  .github/pull_request_template.md
```

`OWNER`: the GitHub owner segment for the app id `io.github.OWNER.Kurobayomi`. The maintainer must choose it before the first release (Flathub requires a domain-style id). It is one constant in `xtask/src/config.rs` and in the file names under `data/` and `packaging/`. Do not hard-code it elsewhere.

## 3. Cargo workspace root (PROPOSAL)

```toml
[package]
name = "kurobayomi"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
default-run = "kurobayomi"

[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"
license = "Apache-2.0"

[workspace.dependencies]
gtk = { version = "0.11.5", package = "gtk4", features = ["v4_16"] }
serde = { version = "1.0.229", features = ["derive"] }
serde_json = "1.0.151"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "time", "sync", "fs", "process"] }
rusqlite = { version = "0.33", features = ["bundled", "backup", "functions"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "gzip", "brotli", "zstd", "cookies", "json", "stream"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
thiserror = "2"
anyhow = "1"
# Versions above are the proposal's starting points; resolve and pin with `cargo update` at implementation time.

[features]
default = []
updater = []      # AppUpdateChecker equivalent; never enabled for Flatpak builds
dev-tools = []    # debug-only UI, replaces isDebugBuildType branches

[profile.release]
lto = "thin"
codegen-units = 1
strip = "debuginfo"
debug = "line-tables-only"
panic = "unwind"

[profile.dist]
inherits = "release"
lto = "fat"

[profile.dev]
debug = "line-tables-only"
```

Notes:

- `rusqlite` version: the mockup does not use rusqlite yet; `0.33` is a starting point. The `bundled` feature compiles SQLite so the Flatpak build does not depend on the runtime's SQLite version (the Android side needs SQLite 3.38 for the `sqlite-3-38` dialect, data/build.gradle.kts:19).
- `panic = "unwind"` is kept deliberately. GTK callbacks run on the main loop; an unwinding panic can be caught at the `glib` boundary and logged. `panic = "abort"` would skip the log flush.
- `[profile.dist]` gives a fat-LTO build for the release tarball and Flatpak.

## 4. Crate `kurobayomi` manifest features and build script (PROPOSAL)

`Cargo.toml` features map the Gradle flags (behaviour section 5):

| Cargo feature | Off by default | Replaces | Who enables it |
|---|---|---|---|
| `updater` | yes | `-Penable-updater` (`UPDATER_ENABLED`) | release tarball build only; never Flatpak |
| `dev-tools` | yes | `isDebugBuildType` branches, pseudo locales | `cargo run` in dev |

`build.rs` (complete proposal):

```rust
use std::env;
use std::process::Command;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8(out.stdout).ok()?.trim().to_owned();
    (!s.is_empty()).then_some(s)
}

fn build_time() -> String {
    // Reproducible builds: SOURCE_DATE_EPOCH wins (Flatpak and distro builds set it).
    let epoch: Option<i64> = env::var("SOURCE_DATE_EPOCH")
        .ok()
        .and_then(|s| s.parse().ok())
        .or_else(|| git(&["log", "-1", "--format=%ct"]).and_then(|s| s.parse().ok()));
    let dt = match epoch {
        Some(e) => OffsetDateTime::from_unix_timestamp(e).unwrap_or(OffsetDateTime::now_utc()),
        None => OffsetDateTime::now_utc(),
    };
    dt.format(&Rfc3339).unwrap_or_else(|_| "unknown".into())
}

fn main() {
    println!("cargo:rerun-if-env-changed=KB_GIT_SHA");
    println!("cargo:rerun-if-env-changed=KB_COMMIT_COUNT");
    println!("cargo:rerun-if-env-changed=KB_APP_ID");
    println!("cargo:rerun-if-env-changed=SOURCE_DATE_EPOCH");
    println!("cargo:rerun-if-changed=.git/HEAD");

    let sha = env::var("KB_GIT_SHA").ok().filter(|s| !s.is_empty())
        .or_else(|| git(&["rev-parse", "--short", "HEAD"]))
        .unwrap_or_else(|| "unknown".into());
    let count = env::var("KB_COMMIT_COUNT").ok().filter(|s| !s.is_empty())
        .or_else(|| git(&["rev-list", "--count", "HEAD"]))
        .unwrap_or_else(|| "0".into());
    let app_id = env::var("KB_APP_ID").ok().filter(|s| !s.is_empty())
        .unwrap_or_else(|| "io.github.OWNER.Kurobayomi".into());

    println!("cargo:rustc-env=KB_GIT_SHA={sha}");
    println!("cargo:rustc-env=KB_COMMIT_COUNT={count}");
    println!("cargo:rustc-env=KB_BUILD_TIME={}", build_time());
    println!("cargo:rustc-env=KB_APP_ID={app_id}");
}
```

Where `OWNER` above is replaced by the constant chosen per section 2 (the build script reads it from the same constant or env var `KB_APP_ID`; do not ship the literal `OWNER`). The `time` crate goes in `[build-dependencies]` with feature `formatting`.

Runtime use: `env!("KB_GIT_SHA")`, `env!("KB_COMMIT_COUNT")`, `env!("KB_BUILD_TIME")`, `env!("KB_APP_ID")`, and `env!("CARGO_PKG_VERSION")`. The About version string follows behaviour section 4 (`Stable`, `Preview r<count> (<sha>)`, `Debug <sha> (<time>)`), chosen by `cfg!(debug_assertions)` and the `KB_CHANNEL` env var set by the packaging script (`stable` | `beta` | `debug`).

Edge cases handled by this script: no git (Flatpak builds): sha `unknown`, count `0`, time from `SOURCE_DATE_EPOCH` or now. Empty `git log` output (no commits): falls through to now. Shallow clone (CI default fetch depth 1): count becomes `1`; CI sets `KB_COMMIT_COUNT` from `git rev-list --count HEAD` after `fetch-depth: 0` (see topic-ci-and-release).

`.git/HEAD` in a worktree is a file, not a directory; `rerun-if-changed` on a missing path only causes a rebuild each time, which is harmless.

## 5. Lint and format (PROPOSAL)

`rustfmt.toml`:

```toml
edition = "2024"
max_width = 120
use_small_heuristics = "Max"
newline_style = "Unix"
```

`max_width = 120` matches the source `.editorconfig` `max_line_length = 120` (`/home/blackbox/aniyomi-src/.editorconfig:16`).

Workspace lints in the root `Cargo.toml`:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"          # Proposal: enable per crate if a GTK FFI site needs it
unused_must_use = "deny"

[workspace.lints.clippy]
all = { level = "warn", priority = -1 }
pedantic = { level = "allow", priority = -1 }
unwrap_used = "warn"
```

Each member opts in with `[lints] workspace = true`. CI runs `cargo clippy --workspace --all-targets --locked -- -D warnings`.

`.editorconfig` replacement: keep `root = true`, `charset = utf-8`, `indent_style = space`, `insert_final_newline = true`, `trim_trailing_whitespace = true`; `[*.rs] indent_size = 4`, `[*.{toml,yml,yaml,json}] indent_size = 2`, `[*.md] trim_trailing_whitespace = false`. Drop the Kotlin and ktlint keys (they have no effect on Rust).

`.gitattributes` replacement: keep `* text=auto`, `* text eol=lf`; keep the binary markers for `*.png`, `*.webp`, `*.jpg`, `*.jpeg`, `*.gif`, `*.ico`, `*.gz`, `*.zip`, `*.7z`, `*.ttf`, `*.woff`; drop `/.idea/* text eol=crlf` (Linux-only project) and `*.jar`.

## 6. `xtask` (PROPOSAL)

`xtask/src/main.rs` subcommands, mapping one-to-one from the Gradle tasks CI used:

| Subcommand | Runs | Replaces |
|---|---|---|
| `cargo xtask lint` | `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `cargo deny check` | `./gradlew spotlessCheck` |
| `cargo xtask test` | `cargo test --workspace --locked` | `./gradlew testReleaseUnitTest` |
| `cargo xtask i18n-check` | for each `po/*.po`: `msgfmt --check --statistics -o /dev/null`; verify every code in `po/LINGUAS` has a `.po`; verify `kurobayomi.pot` up to date with `xgettext` | `./gradlew generateLocalesConfig` + moko checks |
| `cargo xtask dist` | `cargo build --profile dist --locked --features updater`, then assemble `dist/kurobayomi-<version>-x86_64.tar.gz` with binary, `data/*.desktop`, `data/*.metainfo.xml`, icons, `po/*.mo`, `LICENSE`, `NOTICE`, and `SHA256SUMS` | `./gradlew assembleRelease -Penable-updater` + renames in `build_push.yml` |
| `cargo xtask flatpak-lint` | `flatpak-builder-lint manifest packaging/flatpak/*.yml`; `flatpak-builder-lint appstream data/*.metainfo.xml`; `appstreamcli validate --pedantic data/*.metainfo.xml` | (none in the Android repo) |
| `cargo xtask bench-report <csv>` | summarises `hyperfine --export-json` output into a median table | `StartupBenchmark.kt` reports |
| `cargo xtask clean` | `cargo clean` and removes `dist/`, `repo/`, `.flatpak-builder/` | root `clean` task (`build.gradle.kts:14-16`) |

`xtask` depends only on `std`, `anyhow`, and `xshell` (or `std::process::Command`). Proposal: `std::process::Command` to avoid a dependency.

## 7. Flatpak manifest (PROPOSAL, complete structure)

`packaging/flatpak/io.github.OWNER.Kurobayomi.yml`:

```yaml
id: io.github.OWNER.Kurobayomi
runtime: org.gnome.Platform
runtime-version: '48'
sdk: org.gnome.Sdk
sdk-extensions:
  - org.freedesktop.Sdk.Extension.rust-stable
build-options:
  append-path: /usr/lib/sdk/rust-stable/bin
  env:
    CARGO_HOME: /run/build/kurobayomi/cargo
    KB_CHANNEL: stable
    KB_GIT_SHA: ''                     # rewritten by the release workflow to the tag's short SHA (no .git in the sandbox)
    KB_COMMIT_COUNT: '0'               # rewritten by the release workflow to `git rev-list --count <tag>`
    SOURCE_DATE_EPOCH: '0'             # rewritten by the release workflow to the tag commit time (`git log -1 --format=%ct <tag>`)
command: kurobayomi
finish-args:
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  - --device=dri
  - --share=network
  - --socket=pulseaudio
  - --talk-name=org.freedesktop.Notifications
modules:
  - name: mpv
    buildsystem: meson
    config-opts:
      - -Dlibmpv=true
      - -Dcplayer=false
      - -Dgpl=false
    sources:
      - type: git
        url: https://github.com/mpv-player/mpv.git
        tag: v0.41.0                   # latest upstream release at the time of writing (published 2025-12-21); add the commit: field when pinning (T-54a-013)
  - name: kurobayomi
    buildsystem: simple
    build-options:
      build-args:
        - --share=network               # only for cargo fetch if cargo-sources.json is not used
    build-commands:
      - cargo --offline fetch --manifest-path Cargo.toml --locked
      - cargo --offline build --profile dist --locked --features ""
      - install -Dm755 target/dist/kurobayomi /app/bin/kurobayomi
      - install -Dm644 data/io.github.OWNER.Kurobayomi.desktop /app/share/applications/io.github.OWNER.Kurobayomi.desktop
      - install -Dm644 data/io.github.OWNER.Kurobayomi.metainfo.xml /app/share/metainfo/io.github.OWNER.Kurobayomi.metainfo.xml
    sources:
      - type: dir
        path: ../..
      - cargo-sources.json
```

Notes on this proposal:

- The mpv source is pinned by tag (`v0.41.0`, verified with `gh api repos/mpv-player/mpv/releases/latest`). Pinning by commit is added when the manifest is written (T-54a-013). The `-Dlibmpv`, `-Dcplayer`, `-Dgpl` option names must be checked against mpv's `meson_options.txt` at that version.
- `OWNER` is the one open decision for this area: the maintainer's GitHub owner segment. The repository has no git remote, so it cannot be derived from the checkout. It is decision D-1 in p1-tasks.md (see T-54a-001).
- Flatpak's `org.freedesktop.Sdk.Extension.rust-stable` is the standard Rust SDK extension. The name and the path `/usr/lib/sdk/rust-stable/bin` must be verified against the current extension release at implementation time.
- `cargo-sources.json` is generated with `flatpak-cargo-generator.py` from `Cargo.lock`; it is committed in `packaging/flatpak/`.
- `--share=network` is kept only at the runtime level so the app can reach extension and tracker endpoints. Build-time network is removed by using vendored sources.
- Filesystem access is not requested. File dialogs must use `gtk::FileDialog`, which goes through the file-chooser portal. This is a deliberate change from Android's storage model.

## 8. Desktop entry and AppStream metainfo (PROPOSAL)

`data/io.github.OWNER.Kurobayomi.desktop`:

```ini
[Desktop Entry]
Type=Application
Name=Kurobayomi
GenericName=Manga and anime reader
Comment=Read manga and watch anime from installed sources
Exec=kurobayomi %U
Icon=io.github.OWNER.Kurobayomi
Terminal=false
Categories=AudioVideo;Graphics;Network;
StartupNotify=true
StartupWMClass=io.github.OWNER.Kurobayomi
Actions=Library;Updates;History;

[Desktop Action Library]
Name=Open library
Exec=kurobayomi --open library

[Desktop Action Updates]
Name=Open updates
Exec=kurobayomi --open updates

[Desktop Action History]
Name=Open history
Exec=kurobayomi --open history
```

The three actions replace `app/shortcuts.xml` (launcher shortcuts for Manga, Updates, History, Browse; the file is outside this area). Proposal: keep the same three names plus `Browse`, so users keep the four shortcuts (Library, Updates, History, Browse). Action names are fixed by the `--open` argument parser in `src/main.rs`.

`data/io.github.OWNER.Kurobayomi.metainfo.xml` (structure):

```xml
<?xml version="1.0" encoding="UTF-8"?>
<component type="desktop-application">
  <id>io.github.OWNER.Kurobayomi</id>
  <name>Kurobayomi</name>
  <summary>Full-featured player and reader, based on Mihon</summary>
  <metadata_license>CC0-1.0</metadata_license>
  <project_license>Apache-2.0</project_license>
  <launchable type="desktop-id">io.github.OWNER.Kurobayomi.desktop</launchable>
  <description>
    <p>Discover and watch anime, cartoons, series, and more on your Linux desktop.</p>
    <ul>
      <li>Local reading and watching of content.</li>
      <li>A configurable reader with multiple viewers, reading directions and other settings.</li>
      <li>A configurable player built on libmpv with multiple options and settings.</li>
      <li>Tracker support: MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, and Bangumi.</li>
      <li>Categories to organize your library.</li>
      <li>Light and dark themes that follow your GTK theme.</li>
      <li>Scheduled library updates for new chapters and episodes.</li>
      <li>Local backups to read or watch offline, or to your own cloud storage.</li>
    </ul>
  </description>
  <url type="homepage">https://github.com/OWNER/kurobayomi</url>
  <developer_name>Kurobayomi contributors</developer_name>
  <content_rating type="oars-1.1"/>
  <releases>
    <release version="0.1.0" date="YYYY-MM-DD"/>
  </releases>
</component>
```

The `summary` and `description` texts are derived from the fastlane files (`fastlane/metadata/android/en-US/short_description.txt`, `full_description.txt`). The `<h3>` heading in `full_description.txt` is dropped because AppStream descriptions allow only `p`, `ul`, `ol`, `li`, `em`, and `code`. The `Plus much more...` item is removed because AppStream reviewers reject vague copy. `<screenshots>` is required by Flathub and is added with the first release (T-54a-011). The `<release date>` value is written by `cargo xtask dist`, not by hand.

Icon: `fastlane/metadata/android/en-US/images/icon.png` is a symlink to a 1,606-byte `logo.png`, too small for a 512 px Flathub icon. Proposal: produce `data/icons/hicolor/scalable/apps/io.github.OWNER.Kurobayomi.svg` and a 512 px PNG at `data/icons/hicolor/512x512/apps/`. The source artwork is a design decision and is not part of this area.

## 9. Localisation build (PROPOSAL)

Replaces `i18n/` and `i18n-aniyomi/` (moko 0.24.5, `MR`/`AYMR` classes, `strings.xml` and `plurals.xml` per locale folder) and `LocalesConfigTask.kt`.

- Catalogues: gettext `po/kurobayomi.pot` (template) and `po/<lang>.po`. Language codes use gettext form. Full set for `i18n` (67 folders, `base` is the template): `am ar as be bg bn ca ceb cs cv da de el eo es eu fa fi fil fr gl he hi hr hu id it ja jv ka kk km kn ko lt lv ml mr ms my nb nl nn ne pl pt pt_BR ro ru sa sah sc sdh sk sq sr sv ta te th tr uk uz vi zh_CN zh_TW` (the Android folder `in` becomes `id`; the 67th folder is `base`). `i18n-aniyomi` has one extra folder, `ur`. The Android folder names map as follows: `base` to the template (English source), `ka-rGE` to `ka_GE`, `nb-rNO` to `nb_NO`, `pt-rBR` to `pt_BR`, `zh-rCN` to `zh_CN`, `zh-rTW` to `zh_TW`, and `in` to `id` (Android's legacy Indonesian code; gettext and BCP 47 use `id`).
- Message identity: Android keys (`MR.strings.<key>`, `AYMR.strings.<key>`) become gettext `msgid` values equal to the English source text. The migration tool keeps a key-to-msgid map in `po/keys.tsv` so the Rust code can call `i18n::tr("key")` during the port. Proposal: keep the Android key names as the `msgctxt`.
- Plurals: Android quantities (`zero`, `one`, `two`, `few`, `many`, `other`) map to gettext `ngettext` with each language's `Plural-Forms` header. The number of plural forms is taken from the `.po` header, not hard-coded.
- Locale list: `po/LINGUAS` (one code per line, sorted). Generated by `cargo xtask i18n-check --write`, replacing `LocalesConfigTask`. The AppStream `<languages>` element is generated from the same list by `cargo xtask dist`.
- Runtime: `gettext-rs` crate (binds libintl). `bindtextdomain("kurobayomi", "/app/share/locale")` inside Flatpak; `$XDG_DATA_HOME/locale` or the install prefix elsewhere. `textdomain("kurobayomi")` at startup.
- Strings from `i18n-aniyomi` (`AYMR`) and `i18n` (`MR`) merge into one domain. The duplicated base-folder files are not carried over separately.

## 10. Benchmark (PROPOSAL)

Replaces `macrobenchmark/` and `app/src/main/baseline-prof.txt`.

`scripts/bench-startup.sh` (shape):

```bash
#!/usr/bin/env bash
# Usage: scripts/bench-startup.sh <binary> [runs]
set -euo pipefail
bin="${1:?binary path}"
runs="${2:-10}"
hyperfine --warmup 1 --runs "$runs" \
  --export-json "${BENCH_OUT:-target/bench-startup.json}" \
  "env KB_TRACE_STARTUP=1 $bin --exit-after-first-frame"
```

The app side: `KB_TRACE_STARTUP=1` makes `kurobayomi` write `startup main_entry`, `startup window_presented`, `startup first_library_render` timestamps (monotonic, microseconds) to stderr via `tracing`. `--exit-after-first-frame` quits after the first library frame. The three modes of the Android benchmark (cold, warm, hot) become: cold = drop the page cache with `echo 3 > /proc/sys/vm/drop_caches` under sudo (documented, not automated); warm = run the script; hot = run within the same session after the first run. Compilation-mode variants do not apply to a native binary.

## 11. CI (PROPOSAL; full design in p1-topic-ci-and-release.md)

`ci.yml` triggers: `pull_request` and `push` to `main`, with `paths-ignore: ['**.md', 'po/**/*.po', 'data/**']`... Proposal: keep the `.po` files in the path filter (translations do not change Rust code), but run `i18n-check` on any `po/` change.

Concurrency: `group: ${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true` (mirrors `build_pull_request.yml:17-19`).

Permissions: `contents: read` at the top. Release job overrides to `contents: write`.

Runner: `ubuntu-24.04` for PR and push (the Android push job used `ubuntu-latest`; pin the image for reproducibility).

System packages (Proposal): `libgtk-4-dev` (4.16+), `libmpv-dev`, `pkg-config`, `libsqlite3-dev` not required (rusqlite bundled), `gettext`, `appstream` (for `appstreamcli`), `hyperfine` (bench job only).

## 12. Android-to-Linux mapping for this area

| Android/Gradle mechanism | Linux/Rust replacement | Notes |
|---|---|---|
| Gradle + AGP 8.9.1 | Cargo, `rust-toolchain.toml` | No Gradle daemon, no JVM |
| `buildSrc` precompiled plugins | `[workspace.lints]`, `xtask` | |
| Version catalogs `gradle/*.versions.toml` | `[workspace.dependencies]` | One line per crate, see `p1-topic-dependency-mapping.md` |
| `BuildConfig` generated class | `build.rs` `rustc-env` + `env!()` | |
| `-Penable-updater` / `UPDATER_ENABLED` | Cargo feature `updater` | Off for Flatpak |
| `buildTypes` debug/preview/benchmark | Profiles + `KB_APP_ID` suffix + `KB_CHANNEL` | Preview = Flatpak `beta` branch |
| `productFlavors` (none present) | none | |
| `applicationIdSuffix` | app id `io.github.OWNER.Kurobayomi.Devel` | Devel build installs side by side |
| `versionCode` | none | Flatpak uses the release tag and metainfo `<release>` |
| `versionName` | `version` in `[workspace.package]` | Single source of truth (see behaviour section 3) |
| R8 `isMinifyEnabled`, `proguard-rules.pro` | `[profile.dist]` LTO and strip | |
| `keepDebugSymbols` (22 .so names) | `strip = "debuginfo"` in release; debuginfo kept separately in `dist` by `objcopy --only-keep-debug` | Proposal |
| APK ABI splits | `flatpak-builder --arch=x86_64 --arch=aarch64` and tarballs per arch | |
| `apksigner` via `r0adkll/sign-android-release` | `SHA256SUMS` signed with a GPG key (or minisign); Flatpak repo signed with `flatpak build-update-repo --gpg-sign` | Key material as GitHub secrets |
| `mapping/release` (R8 mapping) | not needed; Rust symbols in `dist` debug file | |
| moko `MR`/`AYMR` | gettext `po/` | Section 9 |
| `locales_config.xml` | `po/LINGUAS` + AppStream `<languages>` | |
| Fastlane `metadata/` | AppStream metainfo + desktop file | |
| `shortcutHelper` / `shortcuts.xml` | `[Desktop Action]` entries | Section 8 |
| `spotless` + ktlint | `rustfmt` + `clippy` | Section 5 |
| JUnit platform test logging | `cargo test` default output; `cargo nextest` optional | |
| `baseline-prof.txt`, macrobenchmark | `scripts/bench-startup.sh`, `KB_TRACE_STARTUP` | Section 10 |
| `google-services.json`, ACRA | none (no analytics, no crash reporter) | Crash logs go to `$XDG_STATE_HOME/kurobayomi/logs` |
| Dependency review action | `cargo deny check` | `deny.toml` |
| `include-dependency-info` | `cargo auditable build` in release CI | Embeds the dependency list in the binary |
| `aboutlibraries` (licences screen) | `cargo about generate` into `data/THIRD-PARTY.html`, shown in About | Proposal |
| Renovate (`.github/renovate.json5`) | keep Renovate; point it at `Cargo.toml` and `packaging/flatpak/` | |

## 13. `deny.toml` (PROPOSAL)

```toml
[advisories]
yanked = "deny"

[licenses]
allow = ["Apache-2.0", "MIT", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Unicode-3.0", "Zlib", "MPL-2.0", "LGPL-2.1-or-later", "LGPL-2.1"]
confidence-threshold = 0.9

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

The LGPL entries are needed because GTK 4 (LGPL-2.1+) and libmpv (LGPL-2.1+ when built with `-Dgpl=false`) are dynamically linked. The Flatpak manifest disables mpv's GPL parts for that reason. Confirm the final license of every crate that links them before the first release (T-54a-056).

## 14. Pitfalls (PROPOSAL)

1. Theme: a Flatpak app does not see the host GTK theme by default. Vanilla GTK in the runtime renders Adwaita. The user wants the host GTK theme (Tokyo Night). Options: (a) native install (`cargo install --path .` or distro package) as the daily driver; (b) Flatpak with a `--filesystem=xdg-config/gtk-4.0:ro` override. Recommend (a) for daily use and document (b) as optional. Record this in README.
2. `Flathub` requires the app id to match a domain the maintainer controls; the `OWNER` decision blocks the manifest.
3. `cargo --offline` requires `cargo-sources.json` for every crate in `Cargo.lock`. Regenerate on every lockfile change; CI checks this with `flatpak-cargo-generator.py --check` (Proposal).
4. `SOURCE_DATE_EPOCH` must be set in the Flatpak build for reproducible `KB_BUILD_TIME`.
5. `rusqlite` `bundled` builds SQLite with the C compiler from the SDK; no network needed.
6. `KB_COMMIT_COUNT` in CI requires `fetch-depth: 0`, otherwise `git rev-list --count HEAD` returns 1 (Android CI has the same issue; see behaviour section 17.8).
7. `gtk4` feature `v4_16` gates the API surface; the mockup already uses it. Keep it; it documents the minimum runtime (GTK 4.16).
8. `panic = "unwind"` with GTK: a panic inside a `glib::idle_add` callback unwinds through C frames. Wrap callbacks with `std::panic::catch_unwind` at the boundary (area 44/49 decides where).
