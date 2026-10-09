# Topic: every Gradle dependency in area 54 and its Rust decision

Scope: all entries in the five version catalogs (`gradle/libs.versions.toml`, `gradle/androidx.versions.toml`, `gradle/compose.versions.toml`, `gradle/kotlinx.versions.toml`, `gradle/aniyomi.versions.toml`), the plugins in `buildSrc/build.gradle.kts`, and every `implementation`, `api`, `compileOnly`, `debugImplementation`, `testImplementation`, `coreLibraryDesugaring` line in the 13 module build files in scope. Versions are the ones pinned in the TOML files (see p1-data-and-config.md section 5).

Decision vocabulary: **Replace-with-crate** (a Rust crate does the job), **Replace-native** (a Linux or GTK facility does the job), **Drop** (no need in a Linux desktop app, or Android-only), **Owned elsewhere** (the decision belongs to a feature area; this area records the crate choice only).

The crate names are the first choice for the Rust port. The Rust versions are not pinned here; they are resolved and pinned with `cargo update` at implementation time.

## 1. Catalog `gradle/libs.versions.toml`

| Gradle key | Coordinates and version | Used for (OBSERVED) | Decision | Rust / Linux replacement | Owner area |
|---|---|---|---|---|---|
| `desugar` | `com.android.tools:desugar_jdk_libs` 2.1.5 | Java 8 APIs on old Android (`configureAndroid`) | Drop | none (Rust std) | this area |
| `android-shortcut-gradle` | `com.github.zellius:android-shortcut-gradle-plugin` 0.1.2 | Build plugin for `app/shortcuts.xml` | Replace-native | `[Desktop Action]` entries in the `.desktop` file | this area (T-54a-011, T-54a-028 family) |
| `rxjava` | `io.reactivex:rxjava` 1.3.8 | Reactive streams in the app and source API (`source-api` api, `app` implementation) | Replace-with-crate | `tokio` tasks and channels, `futures` streams, `glib::MainContext` for GTK-side work | area 17 (source), area 26 (library jobs) |
| `okhttp-core` | `com.squareup.okhttp3:okhttp` 5.4.0 | HTTP client | Replace-with-crate | `reqwest` (`rustls-tls`, `cookies`, `gzip`, `brotli`, `zstd`) | area 12 |
| `okhttp-logging` | `logging-interceptor` 5.4.0 | HTTP logging | Replace-with-crate | `tracing` spans around `reqwest` middleware | area 12 |
| `okhttp-zstd` | `okhttp-zstd` 5.4.0 | zstd response decoding | Replace-with-crate | `reqwest` feature `zstd` | area 12 |
| `okhttp-brotli` | `okhttp-brotli` 5.4.0 | brotli decoding | Replace-with-crate | `reqwest` feature `brotli` | area 12 |
| `okhttp-dnsoverhttps` | `okhttp-dnsoverhttps` 5.4.0 | DNS-over-HTTPS option | Replace-with-crate | `hickory-resolver` with DoH, wired into the `reqwest` resolver (optional) | area 12 |
| `okio` | `com.squareup.okio:okio` 3.10.2 | Byte buffers and file IO | Replace-native | `std::io`, `bytes` | area 13 |
| `conscrypt-android` | `org.conscrypt:conscrypt-android` 2.5.3 | TLS 1.3 on Android < 10 (`app` comment line 248) | Drop | `rustls` (via `reqwest` `rustls-tls`) and `rustls-native-certs` if the system store is needed | area 12 |
| `quickjs-android` | `app.cash.quickjs:quickjs-android` 0.9.2 | JavaScript engine (bundle `js-engine`) | Owned elsewhere | `rquickjs` if a JS extension path is kept; area 20 decides | area 20 |
| `jsoup` | `org.jsoup:jsoup` 1.22.2 | HTML parsing for sources | Replace-with-crate | `scraper` (on `html5ever`) with CSS selectors | area 15, 17 |
| `disklrucache` | `com.jakewharton:disklrucache` 2.0.2 | Disk LRU cache (images, downloads) | Replace-with-crate | `lru` in memory plus file store under `$XDG_CACHE_HOME/kurobayomi` | area 34 |
| `unifile` | `com.github.tachiyomiorg:unifile` (commit `e0def6b3dc`) | Unified file access over SAF content URIs | Replace-native | `std::path::Path` plus the file chooser portal (`gtk::FileDialog`); no SAF on Linux | area 13 |
| `libarchive` | `me.zhanghai.android.libarchive:library` 1.1.4 | CBZ, CBR, archive reading via libarchive JNI | Replace-with-crate | `libarchive` crate bindings or `zip` and `unrar` crates, chosen by area 13 | area 13 |
| `sqlite-framework` | `androidx.sqlite:sqlite-framework` 2.4.0 | SQLite access (Android) | Replace-with-crate | `rusqlite` (`bundled`) | area 03-06 |
| `sqlite-ktx` | `androidx.sqlite:sqlite-ktx` 2.4.0 | SQLite Kotlin extensions | Drop | none | area 03-06 |
| `sqlite-android` | `com.github.requery:sqlite-android` 3.45.0 | SQLite driver fork for old Android | Drop | `rusqlite` bundled SQLite | area 03-06 |
| `preferencektx` | `androidx.preference:preference-ktx` 1.2.1 | Android SharedPreferences | Replace-native | typed prefs file (`serde_json` or `toml`) under `$XDG_CONFIG_HOME/kurobayomi`; rules in area 02 | area 02 |
| `injekt` | `com.github.mihonapp:injekt` (commit `91edab2317`) | Dependency injection (`Injekt.get`) | Drop | constructor injection; an `AppContext` struct built in `main.rs` | area 01 |
| `coil-bom` | `io.coil-kt.coil3:coil-bom` 3.1.0 | BOM for Coil | Drop | none | area 34 |
| `coil-core` | `io.coil-kt.coil3:coil` (bom) | Image loader core | Replace-with-crate | `image` crate for decoding, `gtk::gdk::Texture` for display, disk cache from `disklrucache` replacement | area 34 |
| `coil-gif` | `io.coil-kt.coil3:coil-gif` (bom) | GIF decoding | Replace-with-crate | `image` crate GIF decoder | area 34 |
| `coil-compose` | `io.coil-kt.coil3:coil-compose` (bom) | Compose image composable | Replace-native | `gtk::Picture` | area 34, 37 |
| `coil-network-okhttp` | `io.coil-kt.coil3:coil-network-okhttp` (bom) | Coil network fetch via OkHttp | Replace-with-crate | `reqwest` fetch in `kb-net` | area 34 |
| `subsamplingscaleimageview` | `com.github.tachiyomiorg:subsampling-scale-image-view` (commit `66e0db195d`) | Tiled zoomable page view | Replace-native | `gtk::Picture` with `gtk::GestureZoom` and `gtk::GestureDrag`, tiles loaded from `image` crate | area 44 |
| `image-decoder` | `com.github.tachiyomiorg:image-decoder` (commit `41c059e540`) | Native image decoder (AVIF, JXL and others) | Replace-with-crate | `image` crate with `avif` and `webp` features; JXL via `jxl-oxide` if needed | area 34, 44 |
| `natural-comparator` | `com.github.gpanther:java-nat-sort` (`natural-comparator-1.1`) | Natural string sorting for chapters | Replace-with-crate | `natord` crate | area 07, 08 |
| `richtext-commonmark` | `com.halilibo.compose-richtext:richtext-commonmark` 0.20.0 | Markdown rendering | Replace-with-crate | `pulldown-cmark` to Pango markup for `gtk::Label` | area 36, 38 |
| `richtext-m3` | `richtext-ui-material3` 0.20.0 | Material 3 rendering of the markdown | Drop | GTK label rendering | area 38 |
| `material` | `com.google.android.material:material` 1.12.0 | Android Material views | Drop | GTK4 widgets | area 49 |
| `flexible-adapter-core` | `com.github.arkon.FlexibleAdapter:flexible-adapter` (commit `c8013533`) | RecyclerView adapter with selection | Replace-native | `gtk::ListView` / `gtk::GridView` with `gtk::MultiSelection` | area 37 |
| `photoview` | `com.github.chrisbanes:PhotoView` 2.3.0 | Pinch-zoom image view | Replace-native | `gtk::Picture` with gestures | area 44 |
| `directionalviewpager` | `com.github.tachiyomiorg:DirectionalViewPager` 1.0.0 | RTL/LTR page pager | Replace-native | `gtk::Stack` or `gtk::ScrolledWindow` with direction | area 44 |
| `insetter` | `dev.chrisbanes.insetter:insetter` 0.6.1 | Window insets | Drop | GTK layout; no system bars | area 49 |
| `compose-materialmotion` | `io.github.fornewid:material-motion-compose-core` 2.0.1 | Motion transitions | Replace-native | `gtk::Revealer`, `gtk::StackTransitionType` | area 49 |
| `compose-webview` | `io.github.kevinnzou:compose-webview` 0.33.6 | WebView for source login and Cloudflare | Replace-with-crate | `webkit6` (WebKitGTK 6.0) | area 36 |
| `compose-grid` | `io.woong.compose.grid:grid` 1.2.2 | Grid layout | Replace-native | `gtk::GridView` | area 37 |
| `compose-stablemarker` | `com.github.skydoves:compose-stable-marker` 1.0.5 | Compose compiler annotation (`compileOnly` in domain) | Drop | none | area 07 |
| `reorderable` | `sh.calvin.reorderable:reorderable` 2.4.3 | Drag reordering (categories) | Replace-native | `gtk::DragSource` and `gtk::DropTarget` | area 08 |
| `swipe` | `me.saket.swipe:swipe` 1.3.0 | Swipe-to-action rows | Replace-native | `gtk::GestureSwipe` / `gtk::GestureDrag` | area 37 |
| `moko-core` | `dev.icerock.moko:resources` 0.24.5 | Multiplatform resource classes `MR`/`AYMR` | Replace-with-crate | `gettext-rs` with `po/` catalogues (section 9 of p1-rust-design.md) | this area (T-54a-031) |
| `logcat` | `com.squareup.logcat:logcat` 0.1 | Logging facade | Replace-with-crate | `tracing` and `tracing-subscriber` with file output via `tracing-appender` | area 01, 35 |
| `aboutLibraries-compose` | `com.mikepenz:aboutlibraries-compose-m3` 11.6.3 | Open-source licences screen | Replace-with-crate | `cargo about` generates the licence list at build time | this area (T-54a-057) |
| `shizuku-api` | `dev.rikka.shizuku:api` 13.1.0 | Privileged shell access on Android | Drop | none | area 53 |
| `shizuku-provider` | `dev.rikka.shizuku:provider` 13.1.0 | Shizuku content provider | Drop | none | area 53 |
| `leakcanary-android` | `com.squareup.leakcanary:leakcanary-android` 2.14 | Leak detection (commented out in `app`) | Drop | `valgrind` or `heaptrack` as dev tools | this area |
| `leakcanary-plumber` | `com.squareup.leakcanary:plumber-android` 2.14 | Leak plumbing | Drop | none | this area |
| `sqldelight-android-driver` | `app.cash.sqldelight:android-driver` 2.0.2 | SQLDelight Android driver | Replace-with-crate | `rusqlite` | area 03-06 |
| `sqldelight-coroutines` | `app.cash.sqldelight:coroutines-extensions-jvm` 2.0.2 | Flow queries | Replace-with-crate | `tokio::task::spawn_blocking` around `rusqlite` plus change notifications through `tokio::sync::broadcast` | area 05, 06 |
| `sqldelight-android-paging` | `app.cash.sqldelight:androidx-paging3-extensions` 2.0.2 | Paging for lists | Drop | SQL `LIMIT`/`OFFSET` or keyset pagination in `kb-db` | area 05, 06 |
| `sqldelight-dialects-sql` | `app.cash.sqldelight:sqlite-3-38-dialect` 2.0.2 | SQL dialect for schema checks | Replace-native | `rusqlite` with SQLite 3.38 or newer (bundled) | area 03, 04 |
| `junit` | `org.junit.jupiter:junit-jupiter` 5.11.4 | Unit test framework | Replace-native | `cargo test` | this area |
| `kotest-assertions` | `io.kotest:kotest-assertions-core` 5.9.1 | Assertions | Replace-native | `assert!`, `assert_eq!`; `insta` for snapshots | this area |
| `mockk` | `io.mockk:mockk` 1.13.17 | Mocks | Replace-with-crate | `mockall` where a trait mock is needed | this area |
| `voyager-navigator` | `cafe.adriel.voyager:voyager-navigator` 2.2.21-1.10.3 | Screen navigation | Replace-native | a `gtk::Stack` plus a navigation stack in `kb-ui` | area 36 |
| `voyager-screenmodel` | `cafe.adriel.voyager:voyager-screenmodel` (same) | ViewModel-like screen models | Replace-native | plain structs owned by each screen, `glib::Object` subclasses where GTK needs them | area 36 |
| `voyager-tab-navigator` | `cafe.adriel.voyager:voyager-tab-navigator` (same) | Bottom tabs | Replace-native | `gtk::Stack` with `gtk::StackSwitcher` or a sidebar | area 36 |
| `voyager-transitions` | `cafe.adriel.voyager:voyager-transitions` (same) | Screen transitions | Replace-native | `gtk::StackTransitionType` | area 36 |
| `spotless-gradle` | `com.diffplug.spotless:spotless-plugin-gradle` 7.0.2 | Formatting plugin | Replace-native | `rustfmt`, `clippy` | this area |
| `ktlint-core` | `com.pinterest.ktlint:ktlint-cli` 1.7.0 | Kotlin lint | Drop | none | this area |
| `aboutLibraries` plugin | `com.mikepenz.aboutlibraries.plugin` 11.6.3 | Generates licence metadata at build | Replace-with-crate | `cargo about` | this area |

## 2. Catalog `gradle/androidx.versions.toml`

| Gradle key | Version | Used for | Decision | Replacement | Owner |
|---|---|---|---|---|---|
| `gradle` (agp) | 8.9.1 | Android Gradle Plugin | Drop | Cargo | this area |
| `annotation` | 1.9.1 | Android annotations | Drop | none | - |
| `appcompat` | 1.7.0 | AppCompat views | Drop | GTK | area 49 |
| `biometricktx` | 1.2.0-alpha05 | Biometric prompt | Drop | none (no equivalent required; a polkit prompt is not part of the port) | area 36 |
| `constraintlayout` | 2.2.1 | XML constraint layout | Drop | GTK layout | area 49 |
| `corektx` | 1.15.0 | Kotlin extensions for Android | Drop | std | - |
| `localbroadcastmanager` | 1.1.0 | In-process broadcasts | Replace-native | `glib` signals or `tokio::sync::broadcast` | area 01 |
| `splashscreen` | 1.0.1 | Launch splash | Drop | GTK window presents itself | area 01 |
| `recyclerview` | 1.4.0 | Lists | Replace-native | `gtk::ListView` | area 37 |
| `viewpager` | 1.1.0 | Pagers | Replace-native | `gtk::Stack` | area 44 |
| `profileinstaller` | 1.4.1 | AOT profile install | Drop | none | this area |
| `lifecycle-common` | 2.8.7 | Lifecycle | Drop | owned by GTK widget lifetime | - |
| `lifecycle-process` | 2.8.7 | Process lifecycle | Replace-native | `gio::Application` lifecycle | area 01 |
| `lifecycle-runtimektx` | 2.8.7 | Coroutine lifecycle scope | Replace-native | `glib::MainContext` futures tied to the widget | area 01 |
| `workmanager` | 2.10.0 | Scheduled library updates | Replace-native | `tokio` interval while the app runs; `systemd --user` timer or the Background portal for when closed | area 26 |
| `paging-runtime` | 3.3.6 | Paging | Drop | SQL pagination | area 05 |
| `paging-compose` | 3.3.6 | Paging in Compose | Drop | GTK list models | area 37 |
| `interpolator` | 1.0.0 | Easing | Drop | GTK transitions | - |
| `benchmark-macro` | 1.3.3 | Macrobenchmark | Drop | `scripts/bench-startup.sh` | this area |
| `test-ext` | 1.2.1 | JUnit extensions | Drop | none | this area |
| `test-espresso-core` | 3.6.1 | UI tests | Drop | GTK tests are not planned for v1 | this area |
| `test-uiautomator` | 2.3.0 | UI automator | Drop | none | this area |

## 3. Catalog `gradle/compose.versions.toml`

| Key | Version | Decision | Replacement |
|---|---|---|---|
| `activity` (`activity-compose`) | 1.10.1 | Drop | GTK application |
| `bom` (`compose-bom`) | 2025.10.01 | Replace-native | `gtk4` 0.11 with feature `v4_16` |
| `foundation`, `animation`, `animation-graphics`, `runtime`, `ui-util`, `material3-core`, `material-icons` | BOM | Replace-native | GTK widgets; symbolic icons from the GTK icon theme (`gtk::Image::from_icon_name`) |
| `ui-tooling`, `ui-tooling-preview` | BOM | Drop | none (GTK Inspector is `GTK_DEBUG=interactive`) |
| `glance` (`glance-appwidget`) | 1.1.1 | Drop | Plasma applet is a later option (T-54a-043) |

## 4. Catalog `gradle/kotlinx.versions.toml`

| Key | Version | Decision | Replacement |
|---|---|---|---|
| `reflect` (`kotlin-reflect`) | 2.4.0 | Drop | none |
| `gradle` (`kotlin-gradle-plugin`) | 2.4.0 | Drop | Cargo |
| `compose-compiler-gradle` | 2.4.0 | Drop | none |
| `immutables` (`kotlinx-collections-immutable`) | 0.3.8 | Drop | `Arc<[T]>` and `Vec<T>` |
| `coroutines-bom`, `coroutines-core`, `coroutines-android`, `coroutines-guava`, `coroutines-test` | 1.10.2 | Replace-with-crate | `tokio` (runtime, time, sync, test macros); `glib::MainContext` for GTK-thread work |
| `serialization-json` | 1.9.0 | Replace-with-crate | `serde_json` |
| `serialization-json-okio` | 1.9.0 | Replace-with-crate | `serde_json::from_reader` on `std::io::Read` |
| `serialization-protobuf` | 1.9.0 | Replace-with-crate | `prost` (the backup `.tachibk` is protobuf; area 21) |
| `serialization-xml-core`, `serialization-xml` | 0.90.3 | Replace-with-crate | `quick-xml` with `serde` feature (ComicInfo.xml, area 14) |
| `kotlin` plugins `android`, `compose-compiler`, `serialization` | 2.4.0 | Drop | Cargo |

## 5. Catalog `gradle/aniyomi.versions.toml`

| Key | Version | Coordinates | Decision | Replacement | Owner |
|---|---|---|---|---|---|
| `aniyomi-mpv` | 1.18.n | `com.github.aniyomiorg:aniyomi-mpv-lib` | Replace-with-crate | `libmpv2` (or `libmpv` bindings) against system libmpv with the render API into `gtk::GLArea` | area 45, 46 |
| `arthenica-smartexceptions` | 0.2.1 | `com.arthenica:smart-exception-java` | Drop | none (ffmpeg-kit crash helper) | area 24 |
| `constraint-layout` (compose) | 1.1.0 | `androidx.constraintlayout:constraintlayout-compose` | Drop | GTK layout | area 49 |
| `ffmpeg-kit` | 1.18 | `com.github.jmir1:ffmpeg-kit` | Replace-with-crate | `ffmpeg` CLI for HLS and muxing (`std::process::Command`) or `ffmpeg-next` bindings | area 24, 25 |
| `media` (`androidx.media`) | 1.7.0 | MediaSession | Replace-native | MPRIS via `mpris-server` crate (media keys and desktop controls) | area 45 |
| `nanohttpd` | 2.3.1 | `org.nanohttpd:nanohttpd` | Replace-with-crate | `axum` (HTTP server for extensions) | area 16 |
| `seeker` | 1.2.2 | `io.github.2307vivek:seeker` | Replace-native | custom `gtk::DrawingArea` seek bar or `gtk::Scale` | area 46 |
| `torrserver` | 0.1.1 | `io.github.secozzi:torrserver` | Owned elsewhere | `librqbit` if torrent streaming is kept; area 33/24 decides | area 24 |
| `truetypeparser` | 2.1.4 | `io.github.yubyf:truetypeparser-light` | Replace-with-crate | `ttf-parser` (font metadata for subtitles) | area 46 |

## 6. Build-logic plugins (`buildSrc/build.gradle.kts`, `settings.gradle.kts`)

| Plugin or dependency | Decision | Replacement |
|---|---|---|
| `androidx.gradle` (AGP) | Drop | Cargo |
| `kotlinx.gradle` (Kotlin Gradle plugin) | Drop | Cargo |
| `kotlinx.compose.compiler.gradle` | Drop | none |
| `libs.spotless.gradle` | Replace-native | `rustfmt`, `clippy` (`xtask lint`) |
| `libs.android.shortcut.gradle` (root `build.gradle.kts`) | Replace-native | desktop actions |
| `com.github.zellius.shortcut-helper` (app) | Replace-native | desktop actions |
| `alias(libs.plugins.aboutLibraries)` | Replace-with-crate | `cargo about` |
| `alias(libs.plugins.moko)` | Replace-with-crate | gettext |
| `alias(libs.plugins.sqldelight)` | Replace-with-crate | `rusqlite` plus migrations in `kb-db` |
| `org.gradle.toolchains.foojay-resolver-convention` 0.9.0 | Drop | `rust-toolchain.toml` |
| `com.android.tools.build:gradle` plugin resolution (settings) | Drop | Cargo |

## 7. Module-level dependencies (OBSERVED)

From the module build files in scope:

- `app`: `projects.i18n`, `projects.i18nAniyomi`, `projects.core.archive`, `projects.core.common`, `projects.coreMetadata`, `projects.sourceApi`, `projects.sourceLocal`, `projects.data`, `projects.domain`, `projects.presentationCore`, `projects.presentationWidget`, plus about 60 external libraries (section 1-5).
- `core-metadata`: `projects.sourceApi`, `kotlinx.bundles.serialization`.
- `data`: `projects.sourceApi`, `projects.domain`, `projects.core.common`, `kotlinx.serialization.json`, `.json.okio`, `.protobuf`; `api libs.bundles.sqldelight`.
- `domain`: `projects.sourceApi`, `projects.core.common`, coroutines BOM and bundle, serialization bundle, `libs.unifile`; `api libs.sqldelight.android.paging`; `compileOnly libs.compose.stablemarker`; tests `libs.bundles.test` and `kotlinx.coroutines.test`.
- `i18n`, `i18n-aniyomi`: `api libs.moko.core`.
- `presentation-core`: `api projects.core.common`, `api projects.i18n`; Compose BOM and components; `androidx.paging.runtime`, `androidx.paging.compose`, `kotlinx.immutables`.
- `presentation-widget`: `projects.core.common`, `projects.domain`, `projects.presentationCore`, `api projects.i18n`, `api projects.i18nAniyomi`, `compose.glance`, `libs.material`, `kotlinx.immutables`, `coil.bom`, `coil.core`, `api libs.injekt`.
- `source-api` (commonMain): `api kotlinx.serialization.json`, `api libs.injekt`, `api libs.rxjava`, `api libs.jsoup`, `api aniyomilibs.nanohttpd`; (androidMain) `implementation projects.core.common`, `api libs.preferencektx`, coroutines Android.
- `source-local` (commonMain): `implementation projects.sourceApi`, `api projects.i18n`, `api projects.i18nAniyomi`, `implementation libs.unifile`; (androidMain) `projects.core.archive`, `projects.core.common`, `projects.coreMetadata`, `projects.domain`, serialization bundle, `aniyomilibs.ffmpeg.kit`.
- `macrobenchmark`: `androidx.test.ext`, `androidx.test.espresso.core`, `androidx.test.uiautomator`, `androidx.benchmark.macro`.

Every module-level dependency maps to a row above. The Rust crate boundaries (`kb-*`) are defined in p1-rust-design.md section 2.

## 8. Summary counts

- Catalog entries in scope (all five TOML files, library and plugin keys): see the sections above; every key has exactly one decision row.
- Decisions: Replace-with-crate, Replace-native, Drop, Owned elsewhere. Drop is the majority for the Android UI and test stack; Replace-with-crate covers networking, storage, parsing, image, media, and i18n.
- Crates with the most weight in the port (cross-area): `reqwest`, `rusqlite`, `serde`/`serde_json`, `tokio`, `scraper`, `image`, `gtk4` 0.11, `webkit6`, `libmpv2`, `gettext-rs`, `quick-xml`, `prost`, `natord`, `pulldown-cmark`, `mpris-server`, `axum`, `tracing`.
