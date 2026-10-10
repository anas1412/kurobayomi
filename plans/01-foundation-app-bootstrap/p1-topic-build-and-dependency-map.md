# Topic: build definition, dependency map, ProGuard rules

Sources: `app/build.gradle.kts` (340 lines), `app/proguard-android-optimize.txt` (34), `app/proguard-rules.pro` (83), `gradle/libs.versions.toml`, `gradle/androidx.versions.toml`, `gradle/kotlinx.versions.toml`, `buildSrc/src/main/kotlin/mihon/buildlogic/{BuildConfig,Commands}.kt`. Facts about values are in `p1-data-and-config.md` section 1.

## 1. Cargo workspace and profiles (Proposal, T-01a-001, T-01a-040)

```toml
# Cargo.toml (workspace root; the existing package "kurobayomi" becomes a workspace member)
[workspace]
resolver = "3"
members = [".", "crates/kuro-core", "crates/kuro-platform", "crates/kuro-testkit"]

[workspace.package]
edition = "2024"
version = "0.1.0"

[profile.dev]
debug = "line-tables-only"          # readable crash backtraces in debug (Aniyomi debug: logging verbose by default)
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
debug = "line-tables-only"          # backtraces with file:line in crash reports
strip = "none"                      # keep symbol names; the crash report is the only diagnostics channel
panic = "unwind"
[profile.preview]                   # Aniyomi "preview" build type: release behaviour, debug-looking identity
inherits = "release"
```
Mapping of Gradle build types:

| Gradle | Cargo | Differences carried over |
|---|---|---|
| `debug` (`.dev` id suffix, `-<commit count>` version suffix, pseudo-locales) | `cargo build` (profile `dev`) | `verbose_logging` default true; version name gets `-<commit count>`; the application id stays `io.github.anas1412.Kurobayomi` (no suffix: the D-Bus name must not change between builds because the `.desktop` file and notifications refer to it; dev runs already use `NON_UNIQUE` through env vars) |
| `release` (minify and shrink unless `disable-code-shrink`) | `cargo build --release` | `strip` is deliberately off (section 1.1) |
| `preview` (`.debug` suffix, release behaviour, build time = now) | `cargo build --profile preview` | `BUILD_TYPE = "preview"` in `app_info` |
| `benchmark` (profileable, disabled unless flavor `default`=`dev`) | Drop | Rust has `perf`/`samply`; no profileable-APK concept |

### 1.1 Why `strip = "none"`
Aniyomi keeps debug symbols of 22 native libraries (`packaging.jniLibs.keepDebugSymbols`, `build.gradle.kts:104-128`) so native crashes are readable in reports; the equivalent need in Rust is readable panic backtraces, because the crash screen shows them verbatim.

### 1.2 Feature flags
| Gradle property / BuildConfig | Cargo feature | Effect |
|---|---|---|
| `enable-updater` → `UPDATER_ENABLED` | `updater` (off by default) | Compiles the self-update check of area:35-data-misc-updater-notification-export; distro packages leave it off exactly as F-Droid-style Aniyomi builds do |
| `disable-code-shrink` | none | R8 does not exist |
| `include-dependency-info` | none | Android APK metadata; Rust equivalent is `cargo auditable` (optional, T-01a-040 notes it) |

### 1.3 `build.rs` for `kuro-core`
Sets `cargo:rustc-env` variables: `KURO_COMMIT_COUNT` (`git rev-list --count HEAD`, else `0`), `KURO_COMMIT_SHA` (`git rev-parse --short HEAD`, else `unknown`), `KURO_BUILD_TIME` (UTC `%Y-%m-%dT%H:%M:%SZ`; release profile: `git log -1 --format=%ct` converted to UTC, else now), `KURO_BUILD_TYPE` (`debug` when `PROFILE=debug`, `release` when `PROFILE=release`, `preview` when the profile name is `preview`; read from `OUT_DIR` path component because `PROFILE` only reports debug/release). Re-run triggers: `cargo:rerun-if-changed=../../.git/HEAD` and `../../.git/refs`. No `chrono` dependency in the build script (compute with `std::time` and a small civil-from-days function) to keep build deps empty.

### 1.4 Packaging (decision only; area:54-build-release-distribution-docs details it)
ABI splits (`armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64`, universal) → one `x86_64-unknown-linux-gnu` binary; `aarch64-unknown-linux-gnu` is optional and not tested. `shortcutHelper`/`shortcuts.xml` (static launcher shortcuts) → `.desktop` file `Actions=` entries (area:53-platform-manifest-intents-workers). `lint { abortOnError = false }` → `cargo clippy` with `-D warnings` only in CI.

## 2. Every dependency declaration of `app/build.gradle.kts` and its Rust answer

Decision column: R = Rust crate, G = GTK/GLib facility, S = std, X = external program or system library, D = dropped. "Area" = where the replacement is designed.

### 2.1 Project modules (lines 187-197)
| Gradle | Contents | Rust target | Area |
|---|---|---|---|
| `projects.i18n` | MR string resources (moko) | `kuro-i18n` with gettext-style catalogs | area:51-i18n |
| `projects.i18nAniyomi` | AYMR string resources | same | area:51-i18n |
| `projects.core.archive` | zip/rar/7z/tar readers | `kuro-core::archive` over `zip`, `unrar`/`libarchive` | area:13-core-util-storage-archive-torrent |
| `projects.core.common` | preferences, network, util | `kuro-core` | areas 02, 12, 13, 52 |
| `projects.coreMetadata` | ComicInfo, metadata models | `kuro-core::metadata` | area:14-source-local-and-metadata |
| `projects.sourceApi` | source interfaces and models | `kuro-source` | areas 15, 16 |
| `projects.sourceLocal` | local source | `kuro-source-local` | area:14 |
| `projects.data` | repositories, SQL | `kuro-data` (rusqlite) | areas 03-06 |
| `projects.domain` | models, interactors | `kuro-domain` | areas 07-09 |
| `projects.presentationCore` | shared Compose components | GTK widget library in `kurobayomi` | area:49 |
| `projects.presentationWidget` | Glance widgets | Drop (T-01a-024) | area:50 |

### 2.2 Compose (lines 199-208)
`compose.activity`, `compose.foundation`, `compose.material3.core`, `compose.material.icons`, `compose.animation`, `compose.animation.graphics`, `debugImplementation(compose.ui.tooling)`, `compose.ui.tooling.preview`, `compose.ui.util` → G: gtk4 widgets; Material icons → symbolic icon names from the icon theme with bundled SVG fallbacks (area:49); animation → `gtk::Revealer`, `gtk::Stack` transitions, CSS transitions; `ui.tooling` and `@PreviewLightDark` previews → D (replaced by the `KUROBAYOMI_SHOT=file.png` screenshot hook in `src/main.rs`); `compose.ui.util` (`fastFilter`, `fastForEach`) → S iterators.

### 2.3 AndroidX and Kotlin runtime (lines 210-237)
| Gradle | Rust answer |
|---|---|
| `androidx.interpolator` | G: plain GTK 4.16 has no animation-timeline API, so use CSS transitions where a property is animatable and otherwise a frame-clock tick callback (`add_tick_callback`) with small easing functions in `kurobayomi::anim` (area:49) |
| `androidx.paging.runtime`, `androidx.paging.compose` | G: `gio::ListStore` + `gtk::ListView`/`GridView` (virtualised) with an incremental loader (`PagedList` helper in area:40/41) |
| `libs.bundles.sqlite` (`sqlite-framework`, `sqlite-ktx`, `sqlite-android` = requery bundled SQLite) | R: `rusqlite` with feature `bundled` (same reason: a known SQLite version independent of the distro) |
| `kotlinx.reflect`, `kotlinx.immutables` | D: no reflection; Rust ownership replaces immutable collections |
| `kotlinx.coroutines.bom`, `kotlinx.bundles.coroutines` (core, android, guava) | R: `tokio` (+ `futures`, `tokio-stream`); `Dispatchers.Main` = glib main context |
| `androidx.annotation` | D |
| `androidx.appcompat` | G: `gtk::Settings` for theme mode (`AppCompatDelegate.setDefaultNightMode`) |
| `androidx.biometricktx` | X: PAM/polkit or passphrase unlock (area:36, `AppLock` trait) |
| `androidx.constraintlayout`, `aniyomilibs.compose.constraintlayout` | D: GTK containers |
| `androidx.corektx` | S |
| `androidx.localbroadcastmanager` | R: `tokio::sync::broadcast` |
| `androidx.splashscreen` | D: window appears after migrations (`p1-topic-app-context-and-startup.md` section 3) |
| `androidx.recyclerview`, `androidx.viewpager` | G: `gtk::ListView`, `gtk::Stack`, `gtk::Notebook` |
| `androidx.profileinstaller` | D |
| `aniyomilibs.mediasession` | R: `souvlaki` (MPRIS) |
| `androidx.bundles.lifecycle` (`lifecycle-common`, `lifecycle-process`, `lifecycle-runtimektx`) | `ProcessLifecycle` helper (`p1-topic-app-context-and-startup.md` section 7) |
| `androidx.workmanager` | R/X: in-process scheduler (tokio timers) with optional systemd user timer via `SchedulerBackend` (area:26, area:53) |

### 2.4 Networking, serialization, parsing (lines 242-261)
| Gradle | Rust answer |
|---|---|
| `libs.rxjava` | D (T-01a-038) |
| `libs.bundles.okhttp` (`okhttp-core`, `okhttp-logging`, `okhttp-brotli`, `okhttp-dnsoverhttps`, `okhttp-zstd`) | R: `reqwest` with features `rustls-tls`, `brotli`, `zstd`, `gzip`, `cookies`; DoH via `hickory-resolver` (area:12) |
| `libs.okio` | S/R: `bytes` |
| `libs.conscrypt.android` | D: rustls (T-01a-027) |
| `kotlinx.bundles.serialization` (`serialization-json`, `serialization-json-okio`, `serialization-protobuf`, `serialization-xml-core`, `serialization-xml`) | R: `serde`, `serde_json`, `prost` (backups are protobuf), `quick-xml` with serde |
| `libs.jsoup` | R: `scraper` (html5ever) for built-in parsers; third-party code per area:20 |
| `libs.disklrucache` | R: custom LRU in `kuro-core::cache` (area:34) |
| `libs.unifile` | S/G: `std::fs`, `gio::File` (no SAF) |
| `libs.preferencektx` | R: `toml` + `serde` (area:02) |
| `libs.injekt` | `AppContext` (this area) |

### 2.5 Images and UI libraries (lines 266-289)
| Gradle | Rust answer |
|---|---|
| `platform(libs.coil.bom)`, `libs.bundles.coil` (`coil-core`, `coil-gif`, `coil-compose`, `coil-network-okhttp`) | `ImagePipeline` over `reqwest` + `image` + `gdk::Texture` (area:34) |
| `libs.subsamplingscaleimageview` (excluding module `image-decoder`) | tiled `GtkSnapshot` region drawing of large pages (area:43, area:44) |
| `libs.image.decoder` (tachiyomi image-decoder: AVIF, HEIF, JXL, WebP...) | R: `image` (JPEG, PNG, GIF, WebP, AVIF via `dav1d` feature), `jxl-oxide` for JXL, `libheif-rs` for HEIF; `AppInfo::supported_image_mime_types` reflects the compiled set |
| `libs.material` | D |
| `libs.flexible.adapter.core` | G: `gtk::ListView` |
| `libs.photoview` | G: `gtk::GestureZoom`, `GestureDrag` |
| `libs.directionalviewpager` (excluding `androidx.viewpager`) | custom pager widget in area:44 |
| `libs.insetter` | D: no system bars |
| `libs.bundles.richtext` (`richtext-commonmark`, `richtext-m3`) | R: `pulldown-cmark` rendered into `gtk::TextBuffer` tags (changelog, descriptions) |
| `libs.aboutLibraries.compose` and plugin `aboutLibraries` | R/X: `cargo about` generated HTML/JSON of licenses (area:47b-settings-widgets-search-more) |
| `libs.bundles.voyager` (`voyager-navigator`, `voyager-screenmodel`, `voyager-tab-navigator`, `voyager-transitions`) | existing `ui::push/pop/pop_to_root` navigation stack in `src/ui.rs` plus per-screen model structs |
| `libs.compose.materialmotion` | G: `gtk::StackTransitionType` |
| `libs.swipe` | G: `gtk::GestureSwipe`, `GestureDrag` |
| `libs.compose.webview` | R: `webkit6` (WebKitGTK 6) via `WebViewHost` |
| `libs.compose.grid` | G: `gtk::GridView` |
| `libs.reorderable` | G: `gtk::DragSource` / `DropTarget` |

### 2.6 Logging, system, tests, media (lines 291-317)
| Gradle | Rust answer |
|---|---|
| `libs.logcat` | R: `tracing`, `tracing-subscriber`, `tracing-appender` (rotation) |
| `libs.bundles.shizuku` (`shizuku-api`, `shizuku-provider`) | D: Android privileged installer |
| `libs.bundles.test` (`junit`, `kotest-assertions`, `mockk`) | S: `#[test]`; `proptest` only for parsers; hand-written fakes instead of mocks |
| `libs.leakcanary.plumber` (and the commented `leakcanary.android`) | D: Rust has no GC leaks; use `heaptrack` when needed |
| `kotlinx.coroutines.test` | R: `tokio::test` with `start_paused = true` |
| `aniyomilibs.aniyomi.mpv` (mpv-android fork) | X/R: system `libmpv` through `libmpv2` or `libmpv-sys` with the render API in `GtkGLArea` (area:45) |
| `aniyomilibs.ffmpeg.kit`, `aniyomilibs.arthenica.smartexceptions` | X: `ffmpeg` executable for muxing/remux, or `ffmpeg-next` (area:24-downloads-anime) |
| `aniyomilibs.torrserver` | X: TorrServer as an external process (area:13) |
| `aniyomilibs.seeker` | G: `gtk::Scale` with custom chapter markers (area:46) |
| `aniyomilibs.truetypeparser` | R: `ttf-parser` (subtitle font family names, area:48) |
| `buildscript { classpath(kotlinx.gradle) }` | D |
| plugins `mihon.android.application`, `mihon.android.application.compose`, `com.github.zellius.shortcut-helper`, `kotlin("plugin.serialization")`, `libs.plugins.aboutLibraries` | D (Gradle plugins); see 1.4 for shortcuts and licenses |

## 3. Native libraries kept with debug symbols (`build.gradle.kts:104-128`) and their Linux counterparts
| Library | Role | Linux counterpart |
|---|---|---|
| `libandroidx.graphics.path` | Compose path ops | none needed |
| `libarchive-jni` | libarchive JNI (archive reading) | `libarchive` via crate, or pure-Rust `zip`/`sevenz-rust`/`unrar` (area:13) |
| `libavcodec`, `libavdevice`, `libavfilter`, `libavformat`, `libavutil`, `libpostproc`, `libswresample`, `libswscale` | FFmpeg shared libraries bundled for mpv and ffmpeg-kit | system FFmpeg used by system libmpv; `ffmpeg` binary for downloads |
| `libconscrypt_jni` | Conscrypt TLS | rustls |
| `libc++_shared` | C++ runtime for the Android native libs | none |
| `libffmpegkit_abidetect`, `libffmpegkit` | FFmpeg-kit JNI | `ffmpeg` process |
| `libimagedecoder` | tachiyomi image-decoder JNI | `image`, `jxl-oxide`, `libheif-rs` crates |
| `liblibrary`, `libplayer` | JNI glue shipped inside bundled AARs (mpv-android and related; the AAR contents are not in this repository) | none: libmpv is used through its C API |
| `libmpv` | mpv | system `libmpv.so.2` |
| `libquickjs` | QuickJS for source scripts | `rquickjs` (area:12) |
| `libsqlite3x` | requery SQLite | `rusqlite` feature `bundled` |
| `libtorrserver` | TorrServer (Go) | external TorrServer binary |
| `libxml2` | XML | `quick-xml` |

## 4. Kotlin compiler opt-ins (`build.gradle.kts:170-182`)
All eleven are `-opt-in` flags for experimental APIs (Compose animation/foundation/layout/material3/ui, Coil, coroutines `ExperimentalCoroutinesApi`/`FlowPreview`/`InternalCoroutinesApi`, serialization). No runtime behaviour. Drop.

## 5. ProGuard/R8 files and the "host API surface" checklist for area:20

R8 mechanics (Drop): `-dontusemixedcaseclassnames`, `-ignorewarnings`, `-verbose`, `-keepattributes *Annotation*`, native-method keep, enum `values()/valueOf()` keep, Parcelable `CREATOR` keep, `androidx.annotation.Keep` keep rules, `-dontobfuscate`, the `-keep,allowoptimization` rules for the app's own packages (`eu.kanade.**`, `tachiyomi.**`, `mihon.**`), RxJava 1.x unsafe-queue member rules, okhttp `MultipartBody$Builder` rule, kotlinx.serialization companion/serializer rules, XmlUtil `EventType` rule.

What the rules prove about the contract with third-party Kotlin extensions (to be honoured or replaced by the plugin ABI of area:20-extension-strategy-redesign). The extension code is loaded into the host and links against these host classes and libraries, so they are "kept public":

| Keep rule (`proguard-rules.pro` line) | Consumer-visible API |
|---|---|
| 41 `androidx.preference.**` | `PreferenceScreen` and preference widgets for `ConfigurableSource.setupPreferenceScreen` (per-source settings screen; area:15/16 and area:17 `SourcePreferencesScreen`) |
| 42 `android.content.**` | `Context`, `SharedPreferences`, `Intent` |
| 43, 53 `uy.kohesive.injekt.**` | Injekt `Injekt.get<NetworkHelper>()` as the way extensions obtain host services |
| 44 `android.test.base.**` | test base classes |
| 45-47 `kotlin.**`, `kotlinx.coroutines.**`, `kotlinx.serialization.**` | language and runtime |
| 48-49 `okhttp3.**`, `okio.**` | HTTP client types in the `HttpSource` signatures |
| 50 `org.jsoup.**` | HTML parsing used by nearly every extension |
| 51 `rx.**` | RxJava 1 `Observable` in legacy source APIs |
| 52 `app.cash.quickjs.**` | QuickJS for JS-in-extension evaluation |
| 54 `is.xyz.mpv.**` | mpv types for video sources |
| 55 `com.arthenica.**` | FFmpeg-kit types |
| 58 `eu.kanade.tachiyomi.network.interceptor.RateLimitInterceptorKt` | `rateLimit(...)` interceptors |
| 59 `...SpecificHostRateLimitInterceptorKt` | `rateLimitHost(...)` |
| 60 `eu.kanade.tachiyomi.network.NetworkHelper` | `cookieJar`, `client`, `cloudflareClient`, `defaultUserAgentProvider()` |
| 61 `eu.kanade.tachiyomi.network.OkHttpExtensionsKt` | `Call.await()`, `Call.awaitSuccess()`, `Call.asObservable()`, `Call.asObservableSuccess()`, `OkHttpClient.newCachelessCallWithProgress()` |
| 62 `eu.kanade.tachiyomi.network.RequestsKt` | `GET`, `POST`, `PUT`, `DELETE` request builders and `OkHttpClient.get`/`post` suspend helpers |
| 63 `eu.kanade.tachiyomi.AppInfo` | this area's `AppInfo` (version code, version name, image MIME types) |
| 64 `eu.kanade.tachiyomi.torrentutils.**` | torrent URL/metadata helpers for torrent-based anime sources |
| 66-83 RxJava rules | reflection on `rx.internal.util.unsafe` queue fields |
| 86 `okhttp3.MultipartBody$Builder` | multipart POST bodies |
| 90-113 serialization rules | `@Serializable` models in extensions are looked up reflectively |
| 116 `nl.adaptivity.xmlutil.EventType` | XML serialization enum |

Rust consequence: whichever plugin mechanism area:20 chooses must expose, at minimum, the semantic equivalents of: an HTTP client with rate-limit helpers and a Cloudflare-solving client, a default user-agent provider, request builders, HTML parsing, JSON parsing, a per-source preference screen, a way to read `AppInfo`, QuickJS evaluation, and (for video sources) mpv/ffmpeg-level helpers and torrent helpers. This list is the checklist handed over by T-01a-041.
