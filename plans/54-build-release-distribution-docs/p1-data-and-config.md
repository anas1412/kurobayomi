# Data models and configuration: area 54 (build, release, docs)

This area has no runtime data model. It defines build-time configuration, generated constants, CI secrets and variables, artifact names, and external endpoints used by the build. Source paths are repo-relative under `/home/blackbox/aniyomi-src`.

## 1. Gradle properties (OBSERVED)

From `gradle.properties` (13 lines):

| Key | Value | Effect |
|---|---|---|
| `android.nonTransitiveRClass` | `false` | R class is transitive. |
| `android.useAndroidX` | `true` | AndroidX on. |
| `kotlin.code.style` | `official` | Kotlin style. |
| `kotlin.mpp.androidSourceSetLayoutVersion` | `2` | KMP source-set layout v2. |
| `org.gradle.caching` | `true` | Build cache. |
| `org.gradle.configureondemand` | `true` | Configure on demand. |
| `org.gradle.jvmargs` | `-Xmx4g -Dfile.encoding=UTF-8` | Daemon heap. |
| `org.gradle.parallel` | `true` | Parallel tasks. |
| `org.gradle.tooling.parallel` | `true` | Parallel sync (comment: "Gradle 9.4+"). |

Gradle wrapper (`gradle/wrapper/gradle-wrapper.properties`): `distributionUrl=https\://services.gradle.org/distributions/gradle-8.13-bin.zip`, `distributionBase=GRADLE_USER_HOME`, `distributionPath=wrapper/dists`, `networkTimeout=10000`, `validateDistributionUrl=true`, `zipStoreBase=GRADLE_USER_HOME`, `zipStorePath=wrapper/dists`.

Gradle toolchain provisioning (`gradle/gradle-daemon-jvm.properties`): `toolchainVendor=ADOPTIUM`, `toolchainVersion=17`. Download URLs by platform (all `https://api.foojay.io/disco/v3.0/ids/<id>/redirect`):

| Key | Id |
|---|---|
| `LINUX.X86_64`, `UNIX.X86_64`, `FREE_BSD.X86_64` | `4dfe7aab2abf71db71537e9dca36c154` |
| `LINUX.AARCH64`, `UNIX.AARCH64`, `FREE_BSD.AARCH64` | `ff8d269e2495c538cfa04b4b52d22286` |
| `MAC_OS.X86_64` | `2a0209399b0a7928a6e5fc680e1c0d35` |
| `MAC_OS.AARCH64` | `9a7f49eb8ed1ea9722ebec95f4befa0e` |
| `WINDOWS.X86_64` | `a5678544b69a5c533a76c11213c7b7ed` |

Gradle `-P` flags (see behaviour section 5): `enable-updater`, `disable-code-shrink`, `include-dependency-info`, `enableComposeCompilerMetrics`, `enableComposeCompilerReports`, `warningsAsErrors`.

## 2. Generated BuildConfig constants (OBSERVED)

| Name | Kotlin/Java type | Set in | Debug | Preview | Release | Benchmark |
|---|---|---|---|---|---|---|
| `APPLICATION_ID` | String (AGP) | `app/build.gradle.kts:20`, plus suffix | `xyz.jmir.tachiyomi.mi.dev` | `xyz.jmir.tachiyomi.mi.debug` | `xyz.jmir.tachiyomi.mi` | `xyz.jmir.tachiyomi.mi.benchmark` |
| `BUILD_TYPE` | String (AGP) | AGP | `debug` | `preview` | `release` | `benchmark` |
| `DEBUG` | boolean (AGP) | AGP | `true` | `false` | `false` | `false` |
| `VERSION_CODE` | int (AGP) | `app/build.gradle.kts:22` | 132 | 132 | 132 | 132 |
| `VERSION_NAME` | String (AGP) | `app/build.gradle.kts:23` + suffix | `0.18.2.1-<COMMIT_COUNT>` | `0.18.2.1-<COMMIT_COUNT>` | `0.18.2.1` | `0.18.2.1-benchmark` |
| `COMMIT_COUNT` | String | `Commands.kt:11-14` (`git rev-list --count HEAD`) | count | count | count | count |
| `COMMIT_SHA` | String | `Commands.kt:16-19` (`git rev-parse --short HEAD`) | sha | sha | sha | sha |
| `BUILD_TIME` | String | `app/build.gradle.kts:27` (defaultConfig, now), `:57` release (last commit), `:72` preview (now) | now (defaultConfig value) | now | last commit `%ct` | last commit `%ct` (inherited from release via `initWith(release)`; benchmark does not override) |
| `UPDATER_ENABLED` | boolean | `app/build.gradle.kts:28` (`Config.enableUpdater`) | false | false | false unless `-Penable-updater` | false |

Format of `BUILD_TIME`: `yyyy-MM-dd'T'HH:mm:ss'Z'` (`Commands.kt:21`), UTC.

Benchmark's `BUILD_TIME`: `app/build.gradle.kts:74-85` does not override `BUILD_TIME` for `benchmark`. The benchmark build type calls `initWith(release)`, which copies the release build type's BuildConfig fields, so benchmark inherits the last-commit value. This follows from AGP's `initWith` semantics as written; it has not been confirmed with a build.

## 3. Build-type-level settings (OBSERVED)

| Setting | debug | release | preview | benchmark |
|---|---|---|---|---|
| applicationIdSuffix | `.dev` | none | `.debug` | `.benchmark` |
| versionNameSuffix | `-<COMMIT_COUNT>` | none | `-<COMMIT_COUNT>` (copied from debug) | `-benchmark` |
| isMinifyEnabled | AGP default (false) | `Config.enableCodeShrink` (true by default) | inherited from release | inherited from release |
| isShrinkResources | AGP default | `Config.enableCodeShrink` | inherited | inherited |
| isPseudoLocalesEnabled | true | AGP default false | inherited (initWith copies) | inherited |
| isDebuggable | AGP default true | AGP default false | inherited false | false |
| isProfileable | AGP default | AGP default | inherited | true |
| signingConfig | debug keystore | release (none set, so unsigned) | debug keystore | debug keystore |
| matchingFallbacks | none | none | `release` | `release` |

## 4. Packaging lists (OBSERVED)

ABI split (`app/build.gradle.kts:93-100`): `armeabi-v7a`, `arm64-v8a`, `x86`, `x86_64`, plus a universal APK. Output names in the release directory: `app-<abi>-release-unsigned.apk` (and `app-universal-release-unsigned.apk`); after signing: `app-<abi>-release-unsigned-signed.apk`.

`keepDebugSymbols` (22 patterns, see behaviour section 8). `resources.excludes` (11 patterns, see behaviour section 8).

Dependency info: `dependenciesInfo.includeInApk` and `includeInBundle` are `Config.includeDependencyInfo` (default false).

## 5. Version catalogs (OBSERVED)

Gradle version catalogs. Versions are exact as written in the TOML files.

### 5.1 `gradle/libs.versions.toml` (115 lines)

Versions: `aboutlib_version = 11.6.3`, `leakcanary = 2.14`, `moko = 0.24.5`, `okhttp_version = 5.4.0`, `richtext = 0.20.0`, `shizuku_version = 13.1.0`, `sqldelight = 2.0.2`, `sqlite = 2.4.0`, `voyager = 2.2.21-1.10.3`, `spotless = 7.0.2`, `ktlint-core = 1.7.0`.

Libraries (key = module:version):

| Key | Coordinates | Version |
|---|---|---|
| `desugar` | `com.android.tools:desugar_jdk_libs` | 2.1.5 |
| `android-shortcut-gradle` | `com.github.zellius:android-shortcut-gradle-plugin` | 0.1.2 |
| `rxjava` | `io.reactivex:rxjava` | 1.3.8 |
| `okhttp-core` | `com.squareup.okhttp3:okhttp` | 5.4.0 |
| `okhttp-logging` | `com.squareup.okhttp3:logging-interceptor` | 5.4.0 |
| `okhttp-zstd` | `com.squareup.okhttp3:okhttp-zstd` | 5.4.0 |
| `okhttp-brotli` | `com.squareup.okhttp3:okhttp-brotli` | 5.4.0 |
| `okhttp-dnsoverhttps` | `com.squareup.okhttp3:okhttp-dnsoverhttps` | 5.4.0 |
| `okio` | `com.squareup.okio:okio` | 3.10.2 |
| `conscrypt-android` | `org.conscrypt:conscrypt-android` | 2.5.3 |
| `quickjs-android` | `app.cash.quickjs:quickjs-android` | 0.9.2 |
| `jsoup` | `org.jsoup:jsoup` | 1.22.2 |
| `disklrucache` | `com.jakewharton:disklrucache` | 2.0.2 |
| `unifile` | `com.github.tachiyomiorg:unifile` | commit `e0def6b3dc` |
| `libarchive` | `me.zhanghai.android.libarchive:library` | 1.1.4 |
| `sqlite-framework` | `androidx.sqlite:sqlite-framework` | 2.4.0 |
| `sqlite-ktx` | `androidx.sqlite:sqlite-ktx` | 2.4.0 |
| `sqlite-android` | `com.github.requery:sqlite-android` | 3.45.0 |
| `preferencektx` | `androidx.preference:preference-ktx` | 1.2.1 |
| `injekt` | `com.github.mihonapp:injekt` | commit `91edab2317` |
| `coil-bom` | `io.coil-kt.coil3:coil-bom` | 3.1.0 |
| `coil-core` | `io.coil-kt.coil3:coil` | (bom) |
| `coil-gif` | `io.coil-kt.coil3:coil-gif` | (bom) |
| `coil-compose` | `io.coil-kt.coil3:coil-compose` | (bom) |
| `coil-network-okhttp` | `io.coil-kt.coil3:coil-network-okhttp` | (bom) |
| `subsamplingscaleimageview` | `com.github.tachiyomiorg:subsampling-scale-image-view` | commit `66e0db195d` |
| `image-decoder` | `com.github.tachiyomiorg:image-decoder` | commit `41c059e540` |
| `natural-comparator` | `com.github.gpanther:java-nat-sort` | `natural-comparator-1.1` |
| `richtext-commonmark` | `com.halilibo.compose-richtext:richtext-commonmark` | 0.20.0 |
| `richtext-m3` | `com.halilibo.compose-richtext:richtext-ui-material3` | 0.20.0 |
| `material` | `com.google.android.material:material` | 1.12.0 |
| `flexible-adapter-core` | `com.github.arkon.FlexibleAdapter:flexible-adapter` | commit `c8013533` |
| `photoview` | `com.github.chrisbanes:PhotoView` | 2.3.0 |
| `directionalviewpager` | `com.github.tachiyomiorg:DirectionalViewPager` | 1.0.0 |
| `insetter` | `dev.chrisbanes.insetter:insetter` | 0.6.1 |
| `compose-materialmotion` | `io.github.fornewid:material-motion-compose-core` | 2.0.1 |
| `compose-webview` | `io.github.kevinnzou:compose-webview` | 0.33.6 |
| `compose-grid` | `io.woong.compose.grid:grid` | 1.2.2 |
| `compose-stablemarker` | `com.github.skydoves:compose-stable-marker` | 1.0.5 |
| `reorderable` | `sh.calvin.reorderable:reorderable` | 2.4.3 |
| `swipe` | `me.saket.swipe:swipe` | 1.3.0 |
| `moko-core` | `dev.icerock.moko:resources` | 0.24.5 |
| `logcat` | `com.squareup.logcat:logcat` | 0.1 |
| `aboutLibraries-compose` | `com.mikepenz:aboutlibraries-compose-m3` | 11.6.3 |
| `shizuku-api` | `dev.rikka.shizuku:api` | 13.1.0 |
| `shizuku-provider` | `dev.rikka.shizuku:provider` | 13.1.0 |
| `leakcanary-android` | `com.squareup.leakcanary:leakcanary-android` | 2.14 |
| `leakcanary-plumber` | `com.squareup.leakcanary:plumber-android` | 2.14 |
| `sqldelight-android-driver` | `app.cash.sqldelight:android-driver` | 2.0.2 |
| `sqldelight-coroutines` | `app.cash.sqldelight:coroutines-extensions-jvm` | 2.0.2 |
| `sqldelight-android-paging` | `app.cash.sqldelight:androidx-paging3-extensions` | 2.0.2 |
| `sqldelight-dialects-sql` | `app.cash.sqldelight:sqlite-3-38-dialect` | 2.0.2 |
| `junit` | `org.junit.jupiter:junit-jupiter` | 5.11.4 |
| `kotest-assertions` | `io.kotest:kotest-assertions-core` | 5.9.1 |
| `mockk` | `io.mockk:mockk` | 1.13.17 |
| `voyager-navigator` | `cafe.adriel.voyager:voyager-navigator` | 2.2.21-1.10.3 |
| `voyager-screenmodel` | `cafe.adriel.voyager:voyager-screenmodel` | 2.2.21-1.10.3 |
| `voyager-tab-navigator` | `cafe.adriel.voyager:voyager-tab-navigator` | 2.2.21-1.10.3 |
| `voyager-transitions` | `cafe.adriel.voyager:voyager-transitions` | 2.2.21-1.10.3 |
| `spotless-gradle` | `com.diffplug.spotless:spotless-plugin-gradle` | 7.0.2 |
| `ktlint-core` | `com.pinterest.ktlint:ktlint-cli` | 1.7.0 |

Plugins: `aboutLibraries` = `com.mikepenz.aboutlibraries.plugin` 11.6.3; `sqldelight` = `app.cash.sqldelight` 2.0.2; `moko` = `dev.icerock.mobile.multiplatform-resources` 0.24.5.

Bundles: `okhttp` = [okhttp-core, okhttp-logging, okhttp-brotli, okhttp-dnsoverhttps, okhttp-zstd]; `js-engine` = [quickjs-android]; `sqlite` = [sqlite-framework, sqlite-ktx, sqlite-android]; `coil` = [coil-core, coil-gif, coil-compose, coil-network-okhttp]; `shizuku` = [shizuku-api, shizuku-provider]; `sqldelight` = [sqldelight-android-driver, sqldelight-coroutines, sqldelight-android-paging]; `voyager` = [4 voyager]; `richtext` = [richtext-commonmark, richtext-m3]; `test` = [junit, kotest-assertions, mockk].

Entries `coil-core`, `coil-gif`, `coil-compose`, `coil-network-okhttp` have no version of their own and take it from the `coil-bom` platform (3.1.0).

### 5.2 `gradle/androidx.versions.toml` (38 lines)

`agp_version = 8.9.1` (plugin `gradle`), `lifecycle_version = 2.8.7`, `paging_version = 3.3.6`, `interpolator_version = 1.0.0`. Libraries: `annotation` 1.9.1, `appcompat` 1.7.0, `biometricktx` 1.2.0-alpha05, `constraintlayout` 2.2.1, `corektx` 1.15.0, `localbroadcastmanager` 1.1.0, `splashscreen` 1.0.1, `recyclerview` 1.4.0, `viewpager` 1.1.0, `profileinstaller` 1.4.1, `lifecycle-common` / `lifecycle-process` / `lifecycle-runtime-ktx` 2.8.7, `workmanager` (work-runtime) 2.10.0, `paging-runtime` / `paging-compose` 3.3.6, `interpolator` 1.0.0, `benchmark-macro` (benchmark-macro-junit4) 1.3.3, `test-ext` (junit-ktx) 1.2.1, `test-espresso-core` 3.6.1, `test-uiautomator` 2.3.0. Bundle `lifecycle` = [lifecycle-common, lifecycle-process, lifecycle-runtimektx].

### 5.3 `gradle/compose.versions.toml` (18 lines)

`compose-bom = 2025.10.01`. Libraries: `activity` (`androidx.activity:activity-compose`) 1.10.1; `bom`; `foundation`, `animation`, `animation-graphics`, `runtime`, `ui-tooling`, `ui-tooling-preview`, `ui-util`, `material3-core`, `material-icons` (material-icons-extended), all BOM-managed; `glance` (`androidx.glance:glance-appwidget`) 1.1.1.

### 5.4 `gradle/kotlinx.versions.toml` (33 lines)

`kotlin_version = 2.4.0`, `serialization_version = 1.9.0`, `xml_serialization_version = 0.90.3`, `coroutines_version = 1.10.2`. Libraries: `reflect` (kotlin-reflect), `gradle` (kotlin-gradle-plugin), `compose-compiler-gradle`, `immutables` (kotlinx-collections-immutable 0.3.8), coroutines bom/core/android/guava/test, serialization json / json-okio / protobuf, xml core-android and serialization-android (`io.github.pdvrieze.xmlutil`). Plugins: `android` (kotlin android), `compose-compiler` (`org.jetbrains.kotlin.plugin.compose`), `serialization`. Bundles: `coroutines` = [core, android, guava]; `serialization` = [json, json-okio, protobuf, xml-core, xml].

### 5.5 `gradle/aniyomi.versions.toml` (21 lines)

`aniyomi-mpv-lib = 1.18.n` (`com.github.aniyomiorg:aniyomi-mpv-lib`), `arthenica-smartexceptions = 0.2.1`, `constraint-layout = 1.1.0`, `ffmpeg-kit = 1.18` (`com.github.jmir1:ffmpeg-kit`), `media = 1.7.0` (`androidx.media:media`), `nanohttpd = 2.3.1` (`org.nanohttpd:nanohttpd`), `seeker = 1.2.2` (`io.github.2307vivek:seeker`), `torrserver = 0.1.1` (`io.github.secozzi:torrserver`), `truetypeparser = 2.1.4` (`io.github.yubyf:truetypeparser-light`).

## 6. Environment variables and CI variables (OBSERVED)

| Name | Set by | Used in | Notes |
|---|---|---|---|
| `GITHUB_REF` | GitHub | `build_push.yml:61` | `refs/tags/vX.Y.Z` |
| `VERSION_TAG` | step "Get tag name" | later steps | `${GITHUB_REF/refs\/tags\//}` |
| `BUILD_TOOLS_VERSION` | step env | "Sign APK" | `'35.0.1'` |
| `APK_UNIVERSAL_SHA`, `APK_ARM64_V8A_SHA`, `APK_ARMEABI_V7A_SHA`, `APK_X86_SHA`, `APK_X86_64_SHA` | "Clean up build artifacts" | "Create Release" body | sha256 hex |
| `GITHUB_TOKEN` | `secrets.GITHUB_TOKEN` | "Create Release" | |
| `JAVA_HOME` | `actions/setup-java` | Gradle | |

Secrets (OBSERVED names only): `SIGNING_KEY` (base64 keystore), `ALIAS`, `KEY_STORE_PASSWORD`, `KEY_PASSWORD`. Used only when the ref is a tag and the repository is `aniyomiorg/aniyomi`.

Gradle wrapper variables: `JAVA_HOME`, `JAVA_OPTS`, `GRADLE_OPTS`, `DEFAULT_JVM_OPTS` (`gradlew:205`: `-Xmx64m -Xms64m`).

## 7. Artifact file names (OBSERVED)

| Stage | File | Path |
|---|---|---|
| Gradle output, universal | `app-universal-release-unsigned.apk` | `app/build/outputs/apk/release/` |
| Gradle output, per ABI | `app-arm64-v8a-release-unsigned.apk`, `app-armeabi-v7a-...`, `app-x86-...`, `app-x86_64-...` | same |
| Mapping | `app/build/outputs/mapping/release` | directory |
| After signing | `app-universal-release-unsigned-signed.apk`, `app-arm64-v8a-release-unsigned-signed.apk`, `app-armeabi-v7a-release-unsigned-signed.apk`, `app-x86-release-unsigned-signed.apk`, `app-x86_64-release-unsigned-signed.apk` | same directory |
| Renamed for release | `aniyomi-${VERSION_TAG}.apk`, `aniyomi-arm64-v8a-${VERSION_TAG}.apk`, `aniyomi-armeabi-v7a-${VERSION_TAG}.apk`, `aniyomi-x86-${VERSION_TAG}.apk`, `aniyomi-x86_64-${VERSION_TAG}.apk` | repository root (in runner) |
| CI artifacts | `arm64-v8a-${{ github.sha }}`, `mapping-${{ github.sha }}` | GitHub Actions artifacts |
| Generated locales | `xml/locales_config.xml` | `<module>/build/generated/mihon/android/res/xml/locales_config.xml` (`ProjectExtensions.kt:106`) |
| Compose metrics | `<root>/build/compose-metrics/<module>` | when `enableComposeCompilerMetrics` |
| Compose reports | `<root>/build/compose-reports/<module>` | when `enableComposeCompilerReports` |

Backup file name pattern (runtime, owned by area 21, derived from the applicationId): `${APPLICATION_ID}_yyyy-MM-dd_HH-mm.tachibk` (`BackupCreator.kt:230-234`), regex `${APPLICATION_ID}_\d{4}-\d{2}-\d{2}_\d{2}-\d{2}.tachibk`.

## 8. External endpoints used by the build and CI (OBSERVED)

| Endpoint | Method | Used by | Purpose |
|---|---|---|---|
| `https://services.gradle.org/distributions/gradle-8.13-bin.zip` | GET | gradle wrapper | Gradle distribution (checksum not pinned; `validateDistributionUrl=true` only validates the URL) |
| `https://plugins.gradle.org/m2/` (gradlePluginPortal) | GET | settings and buildSrc | plugins |
| `https://dl.google.com` / `google()` | GET | settings | AndroidX, AGP |
| `https://repo.maven.apache.org/maven2` (`mavenCentral()`) | GET | settings | libraries |
| `https://www.jitpack.io` | GET | settings and libs | `com.github.*` artifacts: unifile, injekt, subsampling-scale-image-view, image-decoder, DirectionalViewPager, FlexibleAdapter, PhotoView, aniyomi-mpv-lib, ffmpeg-kit, java-nat-sort, aboutlibraries (indirect) |
| `https://api.foojay.io/disco/v3.0/ids/<id>/redirect` | GET | Gradle toolchain provisioning (`gradle-daemon-jvm.properties`) | JDK 17 Adoptium |
| `https://github.com/*` actions (`actions/checkout`, `actions/setup-java`, `gradle/actions/setup-gradle`, `actions/upload-artifact`, `actions/dependency-review-action`, `r0adkll/sign-android-release`, `softprops/action-gh-release`) | — | workflows | pinned SHAs (behaviour 11) |
| `https://hosted.weblate.org/engage/aniyomi/` | — | README badge | translation status |
| `https://discord.gg/F32UjdJZrR` | — | README, CONTRIBUTING, config.yml | upstream community |
| `https://aniyomi.org/docs/faq/general`, `…/faq/browse/extensions`, `…/guides/troubleshooting/`, `…/changelogs/`, `…/docs/contribute#translation` | — | templates, README, CONTRIBUTING | upstream documentation |
| `https://github.com/aniyomiorg/aniyomi/releases/latest` | — | report_issue.yml, request_feature.yml | version reference |
| `https://github-readme-stats.vercel.app/api/pin/...` | GET | README | repo cards |
| `https://contrib.rocks/image?repo=aniyomiorg/aniyomi` | GET | README | contributor graph |
| `https://img.shields.io/...` | GET | README | badges |

Runtime endpoints (AppUpdateChecker and track APIs) are not in this area; see area 35 and the tracker areas.

## 9. Fastlane store metadata (OBSERVED)

Directory `fastlane/metadata/android/en-US/` (Fastlane supply convention; no Fastfile in repo):

- `short_description.txt`: 1 line, 49 bytes: `Full-featured player and reader, based on Mihon.`
- `full_description.txt`: 724 bytes (13 lines), first line: `Discover and watch anime, cartoons, series, and more – easier than ever on your Android device.` (en dash), then HTML `<h3>Features</h3>`, `<ul>`, nine `<li>` items, `</ul>`.
- `images/icon.png`: symlink to `../../../../../.github/assets/logo.png` (1,606 bytes).

## 10. Repository configuration files (OBSERVED)

| File | Key content |
|---|---|
| `.editorconfig` | root=true; `[*]` utf-8, indent 2, spaces, final newline, trim trailing whitespace; `[*.xml]` indent 4; `[*.{kt,kts}]` indent 4, max_line_length 120, trailing commas allowed (call site too), `ktlint_code_style = intellij_idea`, star-import thresholds 2147483647, disabled ktlint rules: class-signature, discouraged-comment-location, function-expression-body, function-signature. |
| `.gitattributes` | `* text=auto`, `* text eol=lf`, `/.idea/* text eol=crlf`, `*.jar binary`; binary: webp, png, jpg, jpeg, gif, ico, gz, zip, 7z, ttf, eot, woff, pyc, swp. |
| `.gitignore` | `.gradle`, `.kotlin`, `build`, `*.iml`, `.idea/*`, `!.idea/icon.png`, `/captures`, `local.properties`, `.DS_Store`. |
| `.github/renovate.json5` | outside this scope (not listed by `areas.py`); includes only `gradle/aniyomi.versions.toml`. |

## 11. Proposed Kurobayomi configuration (PROPOSAL)

Proposal: the Rust equivalents of the above, each owned by this area unless noted.

Workspace root `Cargo.toml` (excerpt, final form in p1-rust-design.md):

```toml
[workspace]
resolver = "3"
members = ["crates/*", "xtask"]

[workspace.package]
version = "0.1.0"
edition = "2024"
rust-version = "1.85"            # Proposal: confirm MSRV against gtk4 0.11 and rusqlite at implementation time
license = "Apache-2.0"
repository = "https://github.com/<owner>/kurobayomi"
```

Values for the Linux build (Proposal):

| Setting | Value | Replaces |
|---|---|---|
| App id (release) | `io.github.<owner>.Kurobayomi` | `xyz.jmir.tachiyomi.mi` |
| App id (devel) | `io.github.<owner>.Kurobayomi.Devel` | `xyz.jmir.tachiyomi.mi.dev` |
| App id (beta/nightly branch) | `io.github.<owner>.Kurobayomi.Devel` with branch `beta` | `.debug` preview |
| Runtime | `org.gnome.Platform` branch `48` (GTK 4.18; GTK 4.16 is the minimum) | Android minSdk 26 |
| GTK minimum | 4.16, feature `v4_16` on `gtk4` | MIN_SDK 26 |
| Rust toolchain | `rust-toolchain.toml` with `channel = "stable"`, components `rustfmt`, `clippy` | Gradle wrapper 8.13 |
| Target arches | `x86_64`, `aarch64` | ABI list (`armeabi-v7a`, `x86`, ...) |
| Build metadata env | `KB_GIT_SHA`, `KB_COMMIT_COUNT`, `KB_BUILD_TIME`, `KB_APP_ID`, `KB_VARIANT` | BuildConfig constants |
| Feature `updater` | off by default | `UPDATER_ENABLED` / `-Penable-updater` |
| Feature `dev-tools` | off by default | `isPseudoLocalesEnabled`, debug-only UI |
| Log directory | `$XDG_STATE_HOME/kurobayomi/logs` (default `~/.local/state/kurobayomi/logs`) | "Dump crash logs" |
| Data directory | `$XDG_DATA_HOME/kurobayomi` (default `~/.local/share/kurobayomi`) | app private storage |
| Config directory | `$XDG_CONFIG_HOME/kurobayomi` (default `~/.config/kurobayomi`) | SharedPreferences |
| Cache directory | `$XDG_CACHE_HOME/kurobayomi` (default `~/.cache/kurobayomi`) | Coil disk cache |
| Backup file name | `io.github.<owner>.Kurobayomi_yyyy-MM-dd_HH-mm.tachibk` (format kept, owner per area 21) | `${APPLICATION_ID}_...` |
| User-Agent | `Kurobayomi/<version> (Linux; io.github.<owner>.Kurobayomi)` | `Aniyomi v<version> (<applicationId>)` |
| Locale config | `po/LINGUAS` plus AppStream `<languages>` | `locales_config.xml` |
