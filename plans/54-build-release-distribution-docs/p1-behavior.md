# Behavior spec: build, CI, release, docs (area 54)

Source root: `/home/blackbox/aniyomi-src` (git HEAD `97414446`, tag `v0.18.2.1`). Paths are repo-relative. Each section marks OBSERVED (what the Kotlin/Gradle/YAML does) and PROPOSAL (what Kurobayomi should do). Proposals are labelled `Proposal:`.

Scope note: this area has no runtime app code and no UI. It defines how the app is built, packaged, tested in CI, released, and documented. Behaviour that runs inside the app and is only *configured* here (for example the user-agent string, the backup file name prefix, the About screen version strings) is specified here with its exact formula, and the owning area is named.

---

## 1. Build graph (OBSERVED)

`settings.gradle.kts` (lines 45-59):

- `enableFeaturePreview("TYPESAFE_PROJECT_ACCESSORS")`, `rootProject.name = "Aniyomi"`.
- Plugin repositories: `gradlePluginPortal()`, `google()`, `mavenCentral()`, `maven(url = "https://www.jitpack.io")`. A `resolutionStrategy.eachPlugin` maps `com.android.(library|application)` to `com.android.tools.build:gradle:<version>`.
- Plugin `org.gradle.toolchains.foojay-resolver-convention` version `0.9.0`.
- Dependency resolution: `repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)`; repositories `mavenCentral()`, `google()`, `maven(url = "https://www.jitpack.io")`.
- Version catalogs: `kotlinx`, `androidx`, `compose`, `aniyomilibs` (from `gradle/*.versions.toml`). The `libs` catalog is implicit (from `gradle/libs.versions.toml` via the default catalog).
- Included modules, in file order (13): `:app`, `:core-metadata`, `:core:archive`, `:core:common`, `:data`, `:domain`, `:i18n`, `:i18n-aniyomi`, `:macrobenchmark`, `:presentation-core`, `:presentation-widget`, `:source-api`, `:source-local`.

Module plugin mapping (OBSERVED):

| Module | Plugins | Namespace (build file) |
|---|---|---|
| app | `mihon.android.application`, `mihon.android.application.compose`, `com.github.zellius.shortcut-helper`, `kotlin("plugin.serialization")`, `libs.plugins.aboutLibraries` | `eu.kanade.tachiyomi` (`app/build.gradle.kts:17`) |
| core-metadata | `mihon.library`, `kotlin("android")`, `kotlin("plugin.serialization")` | `tachiyomi.core.metadata` (`core-metadata/build.gradle.kts:8`) |
| data | `mihon.library`, `kotlin("android")`, `kotlin("plugin.serialization")`, `libs.plugins.sqldelight` | `tachiyomi.data` (`data/build.gradle.kts:9`) |
| domain | `mihon.library`, `kotlin("android")`, `kotlin("plugin.serialization")` | `tachiyomi.domain` (`domain/build.gradle.kts:8`) |
| i18n | `mihon.library`, `kotlin("multiplatform")`, `libs.plugins.moko` | `tachiyomi.i18n` (`i18n/build.gradle.kts:33`) |
| i18n-aniyomi | `mihon.library`, `kotlin("multiplatform")`, `libs.plugins.moko` | `tachiyomi.i18n.aniyomi` (`i18n-aniyomi/build.gradle.kts:33`) |
| macrobenchmark | `mihon.benchmark` | `tachiyomi.macrobenchmark` (`macrobenchmark/build.gradle.kts:6`) |
| presentation-core | `mihon.library`, `mihon.library.compose`, `kotlin("android")` | `tachiyomi.presentation.core` (`presentation-core/build.gradle.kts:8`) |
| presentation-widget | `mihon.library`, `mihon.library.compose`, `kotlin("android")` | `tachiyomi.presentation.widget` (`presentation-widget/build.gradle.kts:8`) |
| source-api | `mihon.library`, `kotlin("multiplatform")`, `kotlin("plugin.serialization")` | `eu.kanade.tachiyomi.source` (`source-api/build.gradle.kts:43`) |
| source-local | `mihon.library`, `kotlin("multiplatform")` | `tachiyomi.source.local` (`source-local/build.gradle.kts:44`) |

Notes:

- `core:archive` and `core:common` are included by `settings.gradle.kts` but their build files are outside this area (owned by area 13).
- `data/build.gradle.kts:20-29` declares two SQLDelight databases: `Database` (package `tachiyomi.data`, schema dir `./src/main/sqldelight`) and `AnimeDatabase` (package `tachiyomi.mi.data`, schema dir `./src/main/sqldelightanime`). Dialect: `libs.sqldelight.dialects.sql` = `app.cash.sqldelight:sqlite-3-38-dialect`. Note the package name change (`tachiyomi.data` vs `tachiyomi.mi.data`), which is a Kotlin-level namespace only. Areas 03 and 04 own the SQL.
- Module `domain` has `api(libs.sqldelight.android.paging)` (`domain/build.gradle.kts:32`), so the paging extension leaks into the domain API surface.
- `source-api` and `source-local` are Kotlin Multiplatform with `androidTarget()` only. `source-api` exposes `api(kotlinx.serialization.json)`, `api(libs.injekt)`, `api(libs.rxjava)`, `api(libs.jsoup)`, `api(aniyomilibs.nanohttpd)` (`source-api/build.gradle.kts:13-22`). This is the public extension ABI, which area 16 and area 20 analyse.

## 2. Build types and variants (OBSERVED)

`app/build.gradle.kts:45-86`.

| Build type | Settings | Effect |
|---|---|---|
| `debug` (`by getting`) | `applicationIdSuffix = ".dev"`, `versionNameSuffix = "-${getCommitCount()}"`, `isPseudoLocalesEnabled = true` | id `xyz.jmir.tachiyomi.mi.dev`, version like `0.18.2.1-132`. Pseudo-locales on. |
| `release` (`by getting`) | `isMinifyEnabled = Config.enableCodeShrink`, `isShrinkResources = Config.enableCodeShrink`, `proguardFiles("proguard-android-optimize.txt", "proguard-rules.pro")`, `buildConfigField BUILD_TIME` = last-commit time | id `xyz.jmir.tachiyomi.mi`. Minify and shrink on unless `-Pdisable-code-shrink`. |
| `preview` (`create`, `initWith(release)`) | `applicationIdSuffix = ".debug"`, `versionNameSuffix = debug.versionNameSuffix`, `signingConfig = debug.signingConfig`, `matchingFallbacks += release`, `BUILD_TIME` = current UTC time | id `xyz.jmir.tachiyomi.mi.debug`. `BuildConfig.BUILD_TYPE == "preview"`. Used for the Beta channel (README links `aniyomi-preview` releases). |
| `benchmark` (`create`, `initWith(release)`) | `isDebuggable = false`, `isProfileable = true`, `versionNameSuffix = "-benchmark"`, `applicationIdSuffix = ".benchmark"`, `signingConfig = debug.signingConfig`, `matchingFallbacks += release` | id `xyz.jmir.tachiyomi.mi.benchmark`. |

Resource source sets: `preview` and `benchmark` add `src/debug/res` as an extra res directory (`app/build.gradle.kts:88-91`).

Flavors: none. There is no `productFlavors` and no `flavorDimensions` anywhere in the repository (confirmed by grep, excluding the baseline profile). See section 12 for the consequence.

### 2.1 Variant filter (OBSERVED, dead configuration)

`app/build.gradle.kts:320-334`:

```kotlin
androidComponents {
    beforeVariants { variantBuilder ->
        if (variantBuilder.buildType == "benchmark") {
            variantBuilder.enable = variantBuilder.productFlavors.containsAll(
                listOf("default" to "dev"),
            )
        }
    }
    onVariants(selector().withFlavor("default" to "standard")) {
        it.packaging.resources.excludes.add("META-INF/*.version")
    }
}
```

Because no flavors exist, `productFlavors` is an empty list, so `containsAll(listOf("default" to "dev"))` is `false`, and the app `benchmark` variant is disabled (`enable = false`). The `onVariants(... withFlavor("default" to "standard"))` block never matches. The comment at line 330 says the exclusion is for Layout Inspector; that never applies.

Consequence (inference from the source, not verified by a Gradle run): `macrobenchmark/build.gradle.kts:24` sets `targetProjectPath = ":app"`, which needs an app benchmark variant. The `macrobenchmark/README.md` tells developers to select `devBenchmark`, a variant that does not exist. This is a stale remnant of a flavor setup.

## 3. Version and identity (OBSERVED)

`app/build.gradle.kts:19-23`:

- `applicationId = "xyz.jmir.tachiyomi.mi"`
- `versionCode = 132`
- `versionName = "0.18.2.1"`

The same version appears in `CHANGELOG.md:13` (`## [v0.18.2.1] - 2026-09-14`), in `.github/ISSUE_TEMPLATE/report_issue.yml:100` (`0.18.2.1`), and in `.github/ISSUE_TEMPLATE/request_feature.yml:35`. Four places are hand-synced. There is no script that checks them.

`app/build.gradle.kts:17`: `namespace = "eu.kanade.tachiyomi"`. The namespace is independent of the applicationId.

Application id in code: `BackupCreator.kt` builds the backup filename from `BuildConfig.APPLICATION_ID` (`app/src/main/java/eu/kanade/tachiyomi/data/backup/create/BackupCreator.kt:230,234`). Area 21 owns the backup format.

## 4. BuildConfig fields (OBSERVED)

Generated fields (`app/build.gradle.kts:25-28`, `57`, `72`, `137`):

| Field | Type | Source | Value |
|---|---|---|---|
| `COMMIT_COUNT` | String | `getCommitCount()` = `git rev-list --count HEAD` | e.g. `"132"` in this checkout |
| `COMMIT_SHA` | String | `getGitSha()` = `git rev-parse --short HEAD` | short SHA |
| `BUILD_TIME` | String | `getBuildTime(useLastCommitTime)` | format `yyyy-MM-dd'T'HH:mm:ss'Z'` in UTC. release: `git log -1 --format=%ct` converted to UTC. debug and preview: `LocalDateTime.now(UTC)`. |
| `UPDATER_ENABLED` | boolean | `Config.enableUpdater` = `project.hasProperty("enable-updater")` | false unless `-Penable-updater` |

Standard AGP fields used at runtime: `BuildConfig.VERSION_NAME`, `APPLICATION_ID`, `BUILD_TYPE`, `DEBUG`. Runtime helpers in `app/src/main/java/eu/kanade/tachiyomi/util/system/BuildConfig.kt`: `updaterEnabled` (= `BuildConfig.UPDATER_ENABLED`), `isDebugBuildType` (= `BUILD_TYPE == "debug"`), `isPreviewBuildType` (= `"preview"`), `isReleaseBuildType` (= `"release"`).

Version string shown in About (OBSERVED, `app/src/main/java/eu/kanade/presentation/more/settings/screen/about/AboutScreen.kt:236-266`), function `getVersionName(withBuildDate)`:

| Condition (first match) | Output without date | Output with date |
|---|---|---|
| `BuildConfig.DEBUG` | `Debug <COMMIT_SHA>` | `Debug <COMMIT_SHA> (<formatted BUILD_TIME>)` |
| `isPreviewBuildType` | `Preview r<COMMIT_COUNT> (<COMMIT_SHA>)` | `Preview r<COMMIT_COUNT> (<COMMIT_SHA>, <formatted BUILD_TIME>)` |
| otherwise | `Stable <VERSION_NAME>` | `Stable <VERSION_NAME> (<formatted BUILD_TIME>)` |

`getFormattedBuildTime()` (`AboutScreen.kt:271-283`) parses `BUILD_TIME` with `Instant.parse` and formats it, falling back to the raw string on parse failure (the fallback branch is at line 283).

User-agent strings (OBSERVED, owned by area 12 and the tracker areas): several tracker interceptors send `"Aniyomi v${BuildConfig.VERSION_NAME} (${BuildConfig.APPLICATION_ID})"`. Bangumi sends `"jmir1/Aniyomi/v${BuildConfig.VERSION_NAME} (Android) (https://github.com/aniyomiorg/aniyomi)"` (`app/src/main/java/eu/kanade/tachiyomi/data/track/bangumi/BangumiInterceptor.kt:38`).

## 5. Build flags (OBSERVED)

`buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt` (the file name collides with the generated class; it is a Gradle helper):

| Gradle property (`-P`) | Kotlin property | Default | Effect |
|---|---|---|---|
| `enable-updater` | `enableUpdater` | off | `UPDATER_ENABLED = true`. Only `.github/workflows/build_push.yml:39` passes it. PR builds never enable the updater. |
| `disable-code-shrink` | `enableCodeShrink` | on (property absent means shrink on) | Turns R8 minify and resource shrinking off in release. |
| `include-dependency-info` | `includeDependencyInfo` | off | Sets `dependenciesInfo.includeInApk/includeInBundle` (`app/build.gradle.kts:147-150`). AGP embeds dependency metadata. |

Other gradle properties (`gradle.properties`): `enableComposeCompilerMetrics`, `enableComposeCompilerReports` (`ProjectExtensions.kt:81-82`), `warningsAsErrors` (`ProjectExtensions.kt:54-55`, default `false` via `toBoolean()`), `org.gradle.jvmargs=-Xmx4g`.

## 6. Android constants (OBSERVED)

`buildSrc/src/main/kotlin/mihon/buildlogic/AndroidConfig.kt`:

| Constant | Value |
|---|---|
| `COMPILE_SDK` | 36 |
| `TARGET_SDK` | 36 |
| `MIN_SDK` | 26 (Android 8.0; README:24 "Requires Android 8.0 or higher") |
| `NDK` | `27.1.12297006` |
| `BUILD_TOOLS` | `35.0.1` |
| `JavaVersion` | `VERSION_17` |
| `JvmTarget` | `JVM_17` |

`configureAndroid` (`ProjectExtensions.kt:26-63`) also sets `isCoreLibraryDesugaringEnabled = true` with `coreLibraryDesugaring(libs.desugar)` and `-opt-in=kotlin.RequiresOptIn`.

## 7. Kotlin and Compose compiler options (OBSERVED)

`app/build.gradle.kts:168-184`, 11 opt-ins in `freeCompilerArgs`:

`androidx.compose.animation.ExperimentalAnimationApi`, `androidx.compose.animation.graphics.ExperimentalAnimationGraphicsApi`, `androidx.compose.foundation.ExperimentalFoundationApi`, `androidx.compose.foundation.layout.ExperimentalLayoutApi`, `androidx.compose.material3.ExperimentalMaterial3Api`, `androidx.compose.ui.ExperimentalComposeUiApi`, `coil3.annotation.ExperimentalCoilApi`, `kotlinx.coroutines.ExperimentalCoroutinesApi`, `kotlinx.coroutines.FlowPreview`, `kotlinx.coroutines.InternalCoroutinesApi`, `kotlinx.serialization.ExperimentalSerializationApi`.

Compose compiler (`ProjectExtensions.kt:65-95`): feature flag `OptimizeNonSkippingGroups`; metrics to `<root>/build/compose-metrics/<module>` and reports to `<root>/build/compose-reports/<module>` when the gradle properties are set.

`allWarningsAsErrors` is `warningsAsErrors.toBoolean()`, default false (`ProjectExtensions.kt:52-55`).

## 8. Packaging (OBSERVED)

`app/build.gradle.kts:93-100`, ABI splits: `isEnable = true`, `isUniversalApk = true`, `reset()`, `include("armeabi-v7a", "arm64-v8a", "x86", "x86_64")`. Output files: `app-universal-release-unsigned.apk` plus one per ABI.

`packaging.jniLibs.keepDebugSymbols` (`app/build.gradle.kts:103-128`), 22 entries mapped to `**/<name>.so`: `libandroidx.graphics.path`, `libarchive-jni`, `libavcodec`, `libavdevice`, `libavfilter`, `libavformat`, `libavutil`, `libconscrypt_jni`, `libc++_shared`, `libffmpegkit_abidetect`, `libffmpegkit`, `libimagedecoder`, `liblibrary`, `libmpv`, `libplayer`, `libpostproc`, `libquickjs`, `libsqlite3x`, `libswresample`, `libswscale`, `libtorrserver`, `libxml2`.

`packaging.resources.excludes` (`app/build.gradle.kts:131-143`), 11 patterns: `kotlin-tooling-metadata.json`, `LICENSE.txt`, `META-INF/**/*.properties`, `META-INF/**/LICENSE.txt`, `META-INF/*.properties`, `META-INF/*.version`, `META-INF/**/*.MF`, `META-INF/DEPENDENCIES`, `META-INF/LICENSE`, `META-INF/NOTICE`, `META-INF/README.md`.

`lint { abortOnError = false; checkReleaseBuilds = false }` (`app/build.gradle.kts:162-165`). `viewBinding`, `buildConfig` on; `aidl`, `renderScript`, `shaders` off (`:152-160`).

Shortcut helper: `shortcutHelper.setFilePath("./shortcuts.xml")` (`app/build.gradle.kts:14`). The file is `app/shortcuts.xml` (outside this area). The plugin generates Android launcher shortcuts from it.

## 9. Code style and lint (OBSERVED)

`buildSrc/src/main/kotlin/mihon.code.lint.gradle.kts`, applied by every module plugin:

- Spotless plugin `com.diffplug.spotless` (version `7.0.2`, catalog `spotless-gradle`).
- Kotlin: target `**/*.kt`, `**/*.kts`, exclude `**/build/**/*.kt`; `ktlint(libs.ktlint.core.get().version)` = `1.7.0`; `trimTrailingWhitespace()`, `endWithNewline()`.
- XML: target `**/*.xml`; exclude `**/build/**/*.xml` and `<module>/src/commonMain/moko-resources/**` except `base/**`; `trimTrailingWhitespace()`, `endWithNewline()`.
- `.editorconfig` (28 lines): `charset = utf-8`, `indent_size = 2` for `[*]`, `indent_size = 4` for `*.xml`, `indent_size = 4` and `max_line_length = 120` for `*.{kt,kts}`, `ktlint_code_style = intellij_idea`, and disabled rules `class-signature`, `discouraged-comment-location`, `function-expression-body`, `function-signature`. `ktlint_function_naming_ignore_when_annotated_with = Composable`.

CI runs `./gradlew spotlessCheck` (`build_pull_request.yml:46`, `build_push.yml:36`).

## 10. Tests (OBSERVED)

`buildSrc/src/main/kotlin/mihon/buildlogic/ProjectExtensions.kt:97-104`: `tasks.withType<Test> { useJUnitPlatform(); testLogging { events(PASSED, SKIPPED, FAILED) } }`.

Test dependencies: `libs.bundles.test` = `junit-jupiter 5.11.4`, `kotest-assertions-core 5.9.1`, `mockk 1.13.17` (`gradle/libs.versions.toml:89-91,115`), plus `kotlinx.coroutines.test`. Instrumentation runner `androidx.test.runner.AndroidJUnitRunner` in `app`, `core-metadata`, `domain`, `presentation-core`, `presentation-widget`, `source-local` (defaultConfig).

CI runs `./gradlew testReleaseUnitTest` (`build_pull_request.yml:52`, `build_push.yml:42`). No unit tests are in this area's scope files.

## 11. CI workflows (OBSERVED, step by step)

### 11.1 `.github/workflows/build_pull_request.yml` ("PR build check")

- Trigger: `pull_request`, `paths`: `'**'`, then excludes `'!**.md'`, `'!i18n/src/commonMain/moko-resources/**/strings-aniyomi.xml'`, `'!i18n/src/commonMain/moko-resources/**/strings.xml'`, `'!i18n/src/commonMain/moko-resources/**/plurals-aniyomi.xml'`, `'!i18n/src/commonMain/moko-resources/**/plurals.xml'`, and re-includes `base/` versions of the same four files (lines 7-14).
- `concurrency.group = ${{ github.workflow }}-${{ github.event.pull_request.number }}`, `cancel-in-progress: true`.
- `permissions: contents: read`.
- Job `build`, name "Build app", `runs-on: 'ubuntu-24.04'`.
- Steps in order:
  1. Clone repo: `actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683 # v4.2.2` (default fetch depth 1).
  2. Dependency Review: `actions/dependency-review-action@3b139cfc5fae8b618d3eae3675e383bb1769c019 # v4.5.0` (no inputs).
  3. Set up JDK: `actions/setup-java@3a4f6e1af504cf6a31855fa899c6aa5355ba6c12 # v4.7.0`, `java-version: 17`, `distribution: temurin`.
  4. Set up gradle: `gradle/actions/setup-gradle@94baf225fe0a508e581a564467443d0e2379123b # v4.3.0`.
  5. `./gradlew spotlessCheck`.
  6. `./gradlew assembleRelease` (no `-Penable-updater`).
  7. `./gradlew testReleaseUnitTest`.
  8. Upload APK: `actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02 # v4.6.2`, name `arm64-v8a-${{ github.sha }}`, path `app/build/outputs/apk/release/app-arm64-v8a-release-unsigned.apk`.
  9. Upload mapping: same action, name `mapping-${{ github.sha }}`, path `app/build/outputs/mapping/release`.

### 11.2 `.github/workflows/build_push.yml` ("CI")

- Trigger: `push` with `branches: ['*']` and `tags: [v*]`.
- `permissions: contents: write` (for the whole workflow).
- `concurrency.group = ${{ github.workflow }}-${{ github.ref }}`, `cancel-in-progress: true`.
- Job `build`, name "Build app", `runs-on: ubuntu-latest` (PR job uses `ubuntu-24.04`).
- Steps:
  1. Clone repo (`actions/checkout` pinned as above).
  2. Set up JDK 17 temurin (pinned as above).
  3. Set up gradle (pinned as above).
  4. `./gradlew spotlessCheck`.
  5. `./gradlew assembleRelease -Penable-updater`.
  6. `./gradlew testReleaseUnitTest`.
  7. Upload APK, name `arm64-v8a-${{ github.sha }}`, path `app/build/outputs/apk/release/app-arm64-v8a-release-unsigned.apk` (action `ea165f8d...`).
  8. Upload mapping, name `mapping-${{ github.sha }}`.
  9. "Get tag name" (if tag ref AND `github.repository == 'aniyomiorg/aniyomi'`): `echo "VERSION_TAG=${GITHUB_REF/refs\/tags\//}" >> $GITHUB_ENV` with `set -x`.
  10. "Sign APK" (same condition): `r0adkll/sign-android-release@f30bdd30588842ac76044ecdbd4b6d0e3e813478`, `releaseDirectory: app/build/outputs/apk/release`, `signingKeyBase64: ${{ secrets.SIGNING_KEY }}`, `alias: ${{ secrets.ALIAS }}`, `keyStorePassword: ${{ secrets.KEY_STORE_PASSWORD }}`, `keyPassword: ${{ secrets.KEY_PASSWORD }}`, env `BUILD_TOOLS_VERSION: '35.0.1'`. The action is not version-tagged (commit SHA only).
  11. "Clean up build artifacts" (same condition, `set -e`): renames `app-universal-release-unsigned-signed.apk` to `aniyomi-${VERSION_TAG}.apk`; `app-arm64-v8a-release-unsigned-signed.apk` to `aniyomi-arm64-v8a-${VERSION_TAG}.apk`; `app-armeabi-v7a-...` to `aniyomi-armeabi-v7a-...`; `app-x86-...` to `aniyomi-x86-...`; `app-x86_64-...` to `aniyomi-x86_64-...`. After each rename it computes `sha256sum` and writes `APK_UNIVERSAL_SHA`, `APK_ARM64_V8A_SHA`, `APK_ARMEABI_V7A_SHA`, `APK_X86_SHA`, `APK_X86_64_SHA` to `$GITHUB_ENV`.
  12. "Create Release" (same condition): `softprops/action-gh-release@c95fe1489396fe8a9eb87c0abf8aa5b2ef267fda # v2.2.1`, `tag_name: ${{ env.VERSION_TAG }}`, `name: Aniyomi ${{ env.VERSION_TAG }}`, body with a "### Checksums" table (rows Universal, arm64-v8a, armeabi-v7a, x86, x86_64; the Universal and x86_64 rows have a formatting defect: the Universal row lacks a trailing `|`, and x86_64 has an extra trailing `|`), `files` = the five renamed APKs, `draft: true`, `prerelease: false`, env `GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}`.

Observed consequences:

- A single command that pushes a branch and a tag together starts two runs (one per ref). Each run builds the full app.
- Signing happens only for the upstream repository. Forks build unsigned artifacts and produce no release.
- The release is a draft. A maintainer must publish it by hand.
- The workflow keeps `contents: write` for the whole job, including the PR-style build steps, which is broader than needed.

### 11.3 Action pinning

All actions are pinned to full commit SHAs with a version comment (except `r0adkll/sign-android-release`, which has no version comment). Renovate is configured separately at `.github/renovate.json5` (outside this area's file list; it includes only `gradle/aniyomi.versions.toml`).

## 12. Benchmark and baseline profile (OBSERVED)

- `macrobenchmark/build.gradle.kts`: plugin `mihon.benchmark` (`com.android.test`); `targetProjectPath = ":app"`; `experimentalProperties["android.experimental.self-instrumenting"] = true`; `defaultConfig.testInstrumentationRunnerArguments["androidx.benchmark.enabledRules"] = "BaselineProfile"`; build type `benchmark` with `isDebuggable = true`, debug signing, `matchingFallbacks.add("release")`; `androidComponents.beforeVariants(selector().all()) { it.enable = it.buildType == "benchmark" }`.
- `StartupBenchmark.kt`: `ColdStartupBenchmark` (`StartupMode.COLD`), `WarmStartupBenchmark` (`WARM`), `HotStartupBenchmark` (`HOT`). Each runs four `@Test`s over `AbstractStartupBenchmark`: `startupNoCompilation` (`CompilationMode.None()`), `startupBaselineProfileDisabled` (`CompilationMode.Partial(baselineProfileMode = BaselineProfileMode.Disable, warmupIterations = 1)`), `startupBaselineProfile` (`CompilationMode.Partial(baselineProfileMode = BaselineProfileMode.Require)`), `startupFullCompilation` (`CompilationMode.Full()`). Metric `StartupTimingMetric()`, `iterations = 10`, `setupBlock = { pressHome() }`, body `startActivityAndWait()`.
  - Observed defect: `packageName = "xyz.jmir.tachiyomi.mibenchmark"` (`StartupBenchmark.kt:78`). The benchmark variant's id is `xyz.jmir.tachiyomi.mi.benchmark`. The string does not match, so the measured package would not exist.
- `BaselineProfileGenerator.kt`: `BaselineProfileRule().collect(packageName = "xyz.jmir.tachiyomi.mi.benchmark")`. Block: `pressHome()`, `startActivityAndWait()`, click text `Manga`, `Updates`, `History`, `More`, `Settings`. Commented-out steps: `Browse`, `Extensions`, a swipe; a source comment notes that storage permissions are not automated.
- `app/src/main/baseline-prof.txt`: 37,790 lines, 3,932,838 bytes. Rule prefixes by first letter: `H` 20,063; `P` 12,242; `L` 5,485. It is the ART AOT list of method signatures used at launch. The README (`macrobenchmark/README.md`) says it must be regenerated for release builds that change startup.

## 13. Localisation build (OBSERVED)

- `i18n/build.gradle.kts` and `i18n-aniyomi/build.gradle.kts` are identical in structure: `androidTarget()`, `applyDefaultHierarchyTemplate()`, `api(libs.moko.core)` (`dev.icerock.moko:resources` 0.24.5), `-Xexpect-actual-classes`, `preBuild.dependsOn(generateLocalesConfig)`, `lint.disable += MissingTranslation, ExtraTranslation`.
- `i18n`: `multiplatformResources { resourcesPackage.set("tachiyomi.i18n") }`, class `MR` (default).
- `i18n-aniyomi`: `resourcesClassName.set("AYMR")`, `resourcesPackage.set("tachiyomi.i18n.aniyomi")`.
- Resource folders: `src/commonMain/moko-resources/<locale>/strings.xml` and `plurals.xml`. Base strings: `i18n` has 857 `<string>` and 19 `<plurals>`; `i18n-aniyomi` has 570 `<string>` and 15 `<plurals>`. Directory counts under `moko-resources` (from `ls`): `i18n` has 67 entries and `i18n-aniyomi` has 68 entries; both include `base`. The `i18n-aniyomi` module has one extra folder, `ur`, compared with `i18n`; the list above is for `i18n` only. Confirm the exact set with `ls` before writing the locale importer (T-54a-031).
- Locale folder names observed (base plus 66 others, `i18n`): `am ar as base be bg bn ca ceb cs cv da de el eo es eu fa fi fil fr gl he hi hr hu in it ja jv ka-rGE kk km kn ko lt lv ml mr ms my nb-rNO ne nl nn pl pt pt-rBR ro ru sa sah sc sdh sk sq sr sv ta te th tr uk uz vi zh-rCN zh-rTW`.
- `buildSrc/src/main/kotlin/mihon/buildlogic/tasks/LocalesConfigTask.kt`: 
  - For each `**/strings.xml` under `src/commonMain/moko-resources/` whose text does not match `<resources>\s*</resources>|<resources\s*/>`, take `parentFile.name`, replace `base` with `en`, replace `-r` with `-`, replace `+` with `-`. Sort lexicographically. Join as `|   <locale android:name="X"/>` lines. Write `xml/locales_config.xml` with `<locale-config xmlns:android="http://schemas.android.com/apk/res/android">` into `generated/mihon/android/res`. Registered as task `generateLocalesConfig`, a dependency of `preBuild`.
  - The task reads `strings.xml` only (not `plurals.xml`), so a locale with only plurals is omitted.
  - Manifest: `app/src/main/AndroidManifest.xml:44` `android:localeConfig="@xml/locales_config"`.
- Translations are done externally via Weblate (`CONTRIBUTING.md:33`, `README.md:17` badge `hosted.weblate.org/engage/aniyomi/`).

## 14. Release process (OBSERVED)

1. Maintainer bumps `versionCode` and `versionName` in `app/build.gradle.kts`, adds a `CHANGELOG.md` section, and tags `vX.Y.Z[.W]`. The git tag `v0.18.2.1` exists in this checkout.
2. Pushing a tag triggers `build_push.yml`. On the upstream repository the steps above sign, rename, hash, and create a draft release with `files` and checksums.
3. A maintainer edits and publishes the draft.
4. The README points users to the GitHub Releases page; the Beta channel is a separate repository `aniyomi-preview` (README:21-22).
5. The fastlane store metadata is not referenced by any workflow in this repository; it is used by external tooling (Fastlane's supply/metadata convention). Observed: no `Fastfile` exists in the repository.

## 15. Documentation (OBSERVED)

| File | Content | Notes |
|---|---|---|
| `README.md` | Logo at `./.github/assets/logo.png` (80px); badges (Discord, downloads, CI, Apache-2.0, Weblate); download links to `aniyomiorg/aniyomi` releases (Stable) and `aniyomi-preview` (Beta); "Requires Android 8.0 or higher"; features list (7 bullets); contributing links; repositories (`aniyomi-website`, `aniyomi-mpv-lib`); credits (contrib.rocks); disclaimer ("hosts zero content"); license block with copyright lines `Copyright © 2015 Javier Tomás`, `Copyright © 2024 Mihon Open Source Project`, `Copyright © 2024 Aniyomi Open Source Project` | Rewrite for Kurobayomi. Keep the attribution lines in NOTICE. |
| `CHANGELOG.md` | Keep-a-Changelog variant. Legend lines 6-11: `Added`, `Changed `, `Improved`, `Removed`, `Fixed`, `Other`. Sections are `## [vX.Y.Z[.W]] - YYYY-MM-DD`. 22 release sections from `v0.16.4.0` (2024-07-01) to `v0.18.2.1` (2026-09-14). | Legend item `Changed ` has a stray trailing space. |
| `CONTRIBUTING.md` | Prerequisites (Android development, Kotlin), tools (Android Studio, emulator), Discord, Weblate, fork rules (rename app, change icon, disable updater checker `AppUpdateChecker.kt`, change `applicationId`, replace `google-services.json`, replace `ACRA_URI`). | `google-services.json` and ACRA no longer exist in this tree (ACRA removed in 0.18.0.0 per CHANGELOG:151). The file is stale on that point. |
| `CODE_OF_CONDUCT.md` | Contributor Covenant v2.1 text. Enforcement contact: Tachiyomi Discord (`https://discord.gg/tachiyomi`). | Contact is upstream-specific. |
| `LICENSE` | Apache License 2.0, unmodified text, 176 lines. | Keep. |
| `.github/ISSUE_TEMPLATE/config.yml` | `blank_issues_enabled: false`; three contact links (FAQ, Discord, website). | Upstream URLs. |
| `.github/ISSUE_TEMPLATE/report_issue.yml` | Form fields: reproduce-steps (required), expected-behavior (required), actual-behavior (required), crash-logs (optional; instructs `More → Settings → Advanced → Dump crash logs`), aniyomi-version (required), android-version (required), device (required), other-details, acknowledgements (8 checkboxes, 7 required, including the extension-issues clause). | Android-specific fields. |
| `.github/ISSUE_TEMPLATE/request_feature.yml` | Fields: feature-description (required), other-details, acknowledgements (4 required checkboxes). | |
| `.github/pull_request_template.md` | HTML comment template with a self-review reminder, "check all base themes and tablet mode", and an Images table with two placeholder Octocat images. | Tablet and base-theme wording is Android-specific. |
| `.github/FUNDING.yml` | `github: jmir1`, `ko_fi: jmir1`. | Upstream owner. |
| `fastlane/metadata/android/en-US/short_description.txt` | `Full-featured player and reader, based on Mihon.` (49 bytes incl. newline). | Android store metadata. |
| `fastlane/metadata/android/en-US/full_description.txt` | 724 bytes: one paragraph, then `<h3>Features</h3>` and `<ul>` with 9 `<li>` items: local reading and watching; configurable reader with multiple viewers; configurable player on mpv-android; tracker support MyAnimeList, AniList, Kitsu, MangaUpdates, Shikimori, Simkl, Bangumi; categories; light and dark themes; scheduled library updates; local backups for offline or cloud; "Plus much more...". | Converted to AppStream (T-54a-011). |
| `fastlane/metadata/android/en-US/images/icon.png` | Symlink to `../../../../../.github/assets/logo.png` (target is 1606 bytes). | |

## 16. Drop decisions and user impact (PROPOSAL summary; full list in p1-tasks.md)

| Dropped item | Reason | What the user loses |
|---|---|---|
| Android APK signing, APK ABI splits, universal APK (`build_push.yml` steps 10-11) | No APK. | Nothing; Linux packages are Flatpak bundles plus a source tarball. |
| ProGuard/R8 keep rules (`app/proguard-rules.pro` and 14 module files) | Rust has no reflection-based keep list. | Nothing. Extension keep rules are obsolete with the Rust extension strategy (area 20). |
| Baseline profile and macrobenchmark module (`macrobenchmark/*`, `app/src/main/baseline-prof.txt`) | ART-only AOT. | Nothing user-facing; startup measurement is kept as a script (T-54a-023). |
| Glance app widget (`presentation-widget/*`) | Android home-screen widget. | Home-screen widget of the Android app. A KDE Plasma applet is a later option (T-54a-043). |
| Shizuku, leakcanary, biometric, splash screen, profile installer, RxJava 1, desugar, Compose stable marker, shortcut-helper | Android-only libraries. | Android-only features, such as "Shizuku" device-level permissions. |
| `gradle.properties` JVM options, Gradle wrapper, `gradle-daemon-jvm.properties`, `buildSrc` | Replaced by Cargo. | Nothing. |
| `FUNDING.yml` | Upstream owner's links. | Nothing for users. |
| Dead flavor filter in `app/build.gradle.kts:320-334` | Refers to flavors that do not exist. | Nothing; it never ran. |
| `include-dependency-info` flag | AGP-only. | Replaced by `cargo auditable` (T-54a-055). |

## 17. Defects observed in the source (for the port team to avoid)

1. Dead benchmark variant filter and dead flavor-based resource exclusion (`app/build.gradle.kts:320-334`). Effect: app benchmark variant disabled while macrobenchmark expects it. Not verified by running Gradle.
2. Wrong package in `StartupBenchmark.kt:78` (`mibenchmark` vs `mi.benchmark`).
3. Stale path filters in `build_pull_request.yml:7-14` and `build_push.yml` (the latter has none) referencing `strings-aniyomi.xml` and `plurals-aniyomi.xml`. The files under `i18n/src/commonMain/moko-resources/base/` are `strings.xml` and `plurals.xml` only, so the `-aniyomi` filter entries never match. The `i18n-aniyomi` module's files are also named `strings.xml` and `plurals.xml`.
4. `CONTRIBUTING.md` describes `google-services.json` and ACRA, neither of which exists in this tree (CHANGELOG:151 says ACRA was removed).
5. `README.md` and `CONTRIBUTING.md` link to the upstream Discord and issue tracker; these would mislead users of a fork.
6. `macrobenchmark/README.md` says to select `devBenchmark`, which does not exist.
7. Release body checksum table has malformed rows (Universal and x86_64).
8. `COMMIT_COUNT` is wrong in shallow clones: `actions/checkout` at its default fetch depth of 1 yields `git rev-list --count HEAD` = 1 in CI. The PR and push builds therefore report `Preview r1` if a preview build is produced from CI (inference from the default; not verified with a CI run).
9. `getBuildTime` calls `.toLong()` on the output of `git log -1 --format=%ct`. If git is missing or the repository has no commits, the empty string throws `NumberFormatException` at configuration time for release builds (`Commands.kt:30`).
10. PR builds never pass `-Penable-updater`, so updater code paths are not compiled into PR artifacts. This is intentional but undocumented.
11. Gradle `buildSrc/src/main/kotlin/mihon/buildlogic/BuildConfig.kt` shares its file name and the `BuildConfig` interface name with the generated `BuildConfig` class. Not a bug at compile time, but confusing.

## 18. Proposal: how Kurobayomi should behave

Proposal: each OBSERVED behaviour maps to one of these Kurobayomi mechanisms.

- Version: `Cargo.toml` `[workspace.package] version = "0.1.0"` is the only version source. `build.rs` exports `KB_VERSION` from `CARGO_PKG_VERSION`. The About dialog shows `Stable <version>` when built with the release profile, `Preview r<count> (<sha>)` for the nightly Flatpak branch, and `Debug <sha> (<time>)` for debug builds. The same three-way logic as `AboutScreen.getVersionName` is kept.
- Build metadata (`build.rs`): `KB_GIT_SHA` from `git rev-parse --short HEAD`, falling back to env var `KB_GIT_SHA` (set by the Flatpak manifest), then to `unknown`. `KB_COMMIT_COUNT` from `git rev-list --count HEAD`, falling back to `0`. `KB_BUILD_TIME` from `SOURCE_DATE_EPOCH` if set (UTC, `%Y-%m-%dT%H:%M:%SZ`), else `git log -1 --format=%ct`, else the current time. Never panic when git is missing.
- Updater: a Cargo feature `updater` that is off by default. Flathub policy asks apps not to self-update inside the Flatpak sandbox, so the Flatpak build never enables it. The feature is enabled in the non-Flatpak release tarball only.
- Build types: `cargo build` (dev, app id `io.github.<owner>.Kurobayomi.Devel` via a `KB_APP_ID` env var read at build time), `cargo build --release` (app id `io.github.<owner>.Kurobayomi`), and a nightly Flatpak branch `beta` with the Devel id retained for side-by-side install.
- Release profile: `lto = "thin"`, `codegen-units = 1`, `strip = "debuginfo"`, `debug = "line-tables-only"`. Replaces R8 and resource shrinking.
- Lint: `rustfmt.toml` with `max_width = 120`, `edition = "2024"`, `use_small_heuristics = "Max"` (to match the 120 column rule of the source `.editorconfig`). `cargo clippy --workspace --all-targets -- -D warnings`. Workspace-level `[workspace.lints.rust]` and `[workspace.lints.clippy]`.
- CI: see topic file `p1-topic-ci-and-release.md`.
- Docs: README, CHANGELOG (Keep-a-Changelog legend kept), CONTRIBUTING (Rust prerequisites, Flatpak build), CODE_OF_CONDUCT (Contributor Covenant 2.1, contact changed), issue and PR templates adapted for Linux.
- Store metadata: AppStream `io.github.<owner>.Kurobayomi.metainfo.xml`, desktop entry, and icons, generated from the current fastlane texts converted to AppStream markup.
