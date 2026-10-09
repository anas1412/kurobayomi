#!/usr/bin/env python3
"""Partition of the Aniyomi repo into plan areas. `areas.py` prints sizes + residual (uncovered) files;
`areas.py json` prints the workflow args. A pattern is a repo-relative path prefix, or a glob (contains * or ?)."""
import fnmatch, json, os, sys
sys.path.insert(0, os.path.dirname(__file__))
import inventory as inv

E = "app/src/main/java/eu/kanade"
T = E + "/tachiyomi"
P = E + "/presentation"
D = E + "/domain"
M = "app/src/main/java/mihon"
DOM = "domain/src/main/java/tachiyomi/domain"
DAT = "data/src/main/java/tachiyomi/data"
SRCAPI = "source-api/src"
RES = "app/src/main/res"

# (id, title, patterns)
AREAS = [
    ("01-foundation-app-bootstrap", "App bootstrap, DI, crash handling, app-level plumbing",
     [T + "/App.kt", T + "/AppInfo.kt", T + "/di", T + "/crash", E + "/core", P + "/crash", "app/build.gradle.kts", "app/proguard-rules.pro", "app/shrinker-rules.pro", "app/proguard-android-optimize.txt", "app/src/main/java/eu/kanade/test", "core/common/src/main/java/eu/kanade/tachiyomi/core"]),
    ("02-preferences-system", "Preference store + every *Preferences class (all keys, types, defaults)",
     ["core/common/src/main/java/tachiyomi/core/common/preference", T + "/data/preference", D + "/base", D + "/ui", "*Preferences.kt", "*PreferenceStore*.kt"]),
    ("03-db-manga-schema-migrations", "Manga database: .sq tables/views/queries/triggers and all .sqm migrations",
     ["data/src/main/sqldelight/", "data/src/main/AndroidManifest.xml", "data/build.gradle.kts"]),
    ("04-db-anime-schema-migrations", "Anime database: .sq tables/views/queries/triggers and all .sqm migrations",
     ["data/src/main/sqldelightanime/"]),
    ("05-data-repos-entries-items", "Data layer repositories: entries, items (chapters/episodes/seasons), categories",
     [DAT + "/entries", DAT + "/items", DAT + "/category", DAT + "/DatabaseAdapter.kt"]),
    ("06-data-repos-history-track-source-misc", "Data layer repositories: history, track, updates, sources, releases, custom buttons, handlers, extension repos",
     [DAT + "/history", DAT + "/track", DAT + "/updates", DAT + "/source", DAT + "/release", DAT + "/custombutton", DAT + "/handlers", "data/src/main/java/mihon", T + "/data/database"]),
    ("07-domain-entries-items", "Domain: entries (manga/anime/season) and items (chapter/episode/season) models + interactors + tests",
     [DOM + "/entries", DOM + "/items", "domain/src/test", "domain/src/main/java/aniyomi"]),
    ("08-domain-library-category-history-updates", "Domain: library, category, history, updates",
     [DOM + "/library", DOM + "/category", DOM + "/history", DOM + "/updates"]),
    ("09-domain-source-track-download-release-misc", "Domain: source, track, download, release, storage, backup, custom buttons, extension repos/stores",
     [DOM + "/source", DOM + "/track", DOM + "/download", DOM + "/release", DOM + "/storage", DOM + "/backup", DOM + "/custombuttons", "domain/src/main/java/mihon", "domain/build.gradle.kts", "domain/src/main/AndroidManifest.xml"]),
    ("10-app-domain-layer", "App-level domain layer (eu.kanade.domain): interactors, mappers, DomainModule",
     [D + "/DomainModule.kt", D + "/SYDomainModule.kt", D + "/download", D + "/entries", D + "/extension", D + "/items", D + "/source", D + "/track"]),
    ("11-app-migrations", "Mihon/Aniyomi in-app migration framework + each migration step",
     [M + "/core/migration", "app/src/test"]),
    ("12-network-stack", "HTTP stack: OkHttp client, interceptors, Cloudflare, DoH, rate limiting, cookies, user agent",
     ["core/common/src/main/java/eu/kanade/tachiyomi/network", "core/common/src/main/java/eu/kanade/tachiyomi/util/system", T + "/network"]),
    ("13-core-util-storage-archive-torrent", "core/common utilities, storage abstraction, archive readers, torrent helpers",
     ["core/common/src/main/java/tachiyomi/core", "core/common/src/main/java/aniyomi", "core/common/src/test", "core/archive", "core/common/build.gradle.kts", "core/common/src/main/AndroidManifest.xml", "core/common/src/main/java/eu/kanade/tachiyomi/util", "core/common/src/main/java/mihon", "core/common/src/main/java/logcat"]),
    ("14-source-local-and-metadata", "Local source (manga + anime) and metadata parsing (ComicInfo, details.json)",
     ["source-local", "core-metadata"]),
    ("15-source-api-manga", "Manga source API: Source/HttpSource/CatalogueSource, models, filters, ConfigurableSource",
     [SRCAPI + "/commonMain/kotlin/eu/kanade/tachiyomi/source", SRCAPI + "/commonMain/kotlin/eu/kanade/tachiyomi/util", SRCAPI + "/androidMain", "source-api/build.gradle.kts"]),
    ("16-source-api-anime", "Anime source API: AnimeSource/AnimeHttpSource, Video/Hoster models, filters, season/episode fetch types",
     [SRCAPI + "/commonMain/kotlin/eu/kanade/tachiyomi/animesource", SRCAPI + "/commonMain/kotlin"]),
    ("17-source-app-layer", "App source layer: SourceManager, StubSource, source interactors, source pinning/language/etc.",
     [T + "/source", D + "/source", M + "/domain/source"]),
    ("18-extensions-manga", "Manga extension loader, installer, trust, update API, extension repos",
     [T + "/extension/manga", T + "/extension/*.kt", D + "/extension/manga", "*extensionrepo*"]),
    ("19-extensions-anime", "Anime extension loader, installer, trust, update API, extension stores",
     [T + "/extension/anime", D + "/extension/anime", "*ExtensionStore*", "*extension_store*", "domain/src/main/java/eu/kanade/tachiyomi/extension"]),
    ("20-extension-strategy-redesign", "Design analysis: how Kurobayomi replaces APK extensions (WASM/Lua/JS/declarative/JVM bridge), source catalog migration",
     [T + "/extension", SRCAPI, T + "/source"]),
    ("21-backup-format-and-create", "Backup file format (protobuf+gzip), models, creators, auto-backup",
     [T + "/data/backup/models", T + "/data/backup/create", T + "/data/backup/full", T + "/data/backup/*.kt", D + "/base/*Backup*"]),
    ("22-backup-restore", "Backup restore: restorers, validators, legacy handling, restore job, error reporting",
     [T + "/data/backup/restore", P + "/more/settings/screen/data", "core-metadata"]),
    ("23-downloads-manga", "Manga downloads: Downloader, DownloadManager, provider, cache, queue, job, notifications",
     [T + "/data/download/manga", T + "/ui/download/manga"]),
    ("24-downloads-anime", "Anime downloads: Downloader (HTTP/HLS/ffmpeg/torrent), manager, provider, cache, queue, job",
     [T + "/data/download/anime", T + "/ui/download/anime"]),
    ("25-downloads-shared", "Shared download infrastructure + domain download + queue UI base + download storage layout",
     [T + "/data/download/*.kt", D + "/download", DOM + "/download", T + "/ui/download/*.kt", P + "/more/*Download*"]),
    ("26-library-update-jobs", "Library update jobs, scheduling, restrictions, fetch intervals, notifications",
     [T + "/data/library", T + "/util/chapter", T + "/util/episode", "app/src/main/java/aniyomi/util"]),
    ("27-tracking-core", "Tracker framework: Tracker/TrackerManager, models, OAuth plumbing, enhanced trackers, track domain/UI",
     [T + "/data/track/*.kt", T + "/data/track/model", D + "/track", P + "/track", T + "/ui/entries/manga/track", T + "/ui/entries/anime/track", T + "/ui/setting/track", P + "/more/settings/screen/SettingsTrackingScreen.kt"]),
    ("28-tracker-anilist", "AniList tracker", [T + "/data/track/anilist"]),
    ("29-tracker-myanimelist", "MyAnimeList tracker", [T + "/data/track/myanimelist"]),
    ("30-tracker-kitsu", "Kitsu tracker", [T + "/data/track/kitsu"]),
    ("31-tracker-shikimori-bangumi", "Shikimori + Bangumi trackers", [T + "/data/track/shikimori", T + "/data/track/bangumi"]),
    ("32-tracker-mangaupdates-simkl", "MangaUpdates + Simkl trackers", [T + "/data/track/mangaupdates", T + "/data/track/simkl"]),
    ("33-tracker-enhanced-kavita-komga-suwayomi-jellyfin", "Enhanced trackers: Kavita, Komga, Suwayomi, Jellyfin", [T + "/data/track/kavita", T + "/data/track/komga", T + "/data/track/suwayomi", T + "/data/track/jellyfin"]),
    ("34-images-covers-cache", "Image pipeline: Coil fetchers/mappers/decoders, cover cache, image decoding, saving",
     [T + "/data/coil", T + "/data/cache", T + "/data/saver", T + "/util/view", E + "/presentation/util"]),
    ("35-data-misc-updater-notification-export", "Notifications, app updater, library export, misc data services",
     [T + "/data/notification", T + "/data/updater", T + "/data/export", T + "/data/*.kt", T + "/data/data", D + "/release", "app/src/main/res/xml"]),
    ("36-ui-shell-navigation-security-webview", "App shell: MainActivity, HomeScreen, navigation, deeplinks, security/unlock, webview, base activities, onboarding",
     [T + "/ui/main", T + "/ui/home", T + "/ui/base", T + "/ui/deeplink", T + "/ui/security", T + "/ui/webview", T + "/ui/*.kt", P + "/webview", P + "/more/onboarding", P + "/components", T + "/widget", T + "/ui/more/NewUpdateScreen.kt", T + "/ui/more/OnboardingScreen.kt"]),
    ("37-ui-library", "Library screens (manga + anime): grid/list, categories, filters, sorting, selection, per-category settings",
     [T + "/ui/library", P + "/library", T + "/ui/category", P + "/category"]),
    ("38-ui-entry-details-manga", "Manga details screen + ScreenModel logic (chapter list, filters, bulk actions, merge/migrate triggers)",
     [T + "/ui/entries/manga", T + "/ui/entries/*.kt", P + "/entries/manga", P + "/entries/components", P + "/entries/*.kt", T + "/util/chapter"]),
    ("39-ui-entry-details-anime", "Anime details screen + ScreenModel logic (episode list, seasons, filters, bulk actions, filler marks)",
     [T + "/ui/entries/anime", P + "/entries/anime", T + "/util/episode"]),
    ("40-ui-browse-manga", "Manga browse: sources list, browse source, global search, filters, extensions screen, migration screens",
     [T + "/ui/browse/manga", P + "/browse/manga", P + "/browse/*.kt", T + "/ui/browse/*.kt"]),
    ("41-ui-browse-anime", "Anime browse: sources list, browse source, global search, filters, extensions screen, migration screens (incl. seasons)",
     [T + "/ui/browse/anime", P + "/browse/anime"]),
    ("42-ui-updates-history-stats-storage-upcoming", "Updates, History, Stats, Storage, Upcoming calendar screens",
     [T + "/ui/updates", P + "/updates", T + "/ui/history", P + "/history", T + "/ui/stats", T + "/ui/storage", P + "/more/stats", P + "/more/storage", M + "/feature"]),
    ("43-reader-core", "Manga reader core: ReaderActivity/ViewModel, chapter loading (zip/dir/epub/http), model, preloading, progress saving, settings logic",
     [T + "/ui/reader/loader", T + "/ui/reader/model", T + "/ui/reader/setting", T + "/ui/reader/*.kt", T + "/ui/reader/viewer/*.kt", P + "/reader"]),
    ("44-reader-viewers", "Reader viewers: pager (L2R/R2L/vertical), webtoon, tap-zone navigation, zoom/gestures, image rendering",
     [T + "/ui/reader/viewer"]),
    ("45-player-core", "Anime player core: PlayerActivity/ViewModel, video loading, mpv integration, history/progress, external players, torrent/HTTP services",
     [T + "/ui/player/*.kt", T + "/ui/player/loader", T + "/ui/player/settings", T + "/ui/player/utils", T + "/data/player", T + "/data/torrent", "app/src/main/assets"]),
    ("46-player-controls-ui", "Anime player controls UI: gestures, seekbar, sheets, panels, dialogs, overlays, presentation/player",
     [T + "/ui/player/controls", P + "/player"]),
    ("47-settings-screens-general", "Settings screens (general): appearance, library, reader, browse, downloads, tracking, security, advanced, data/backup UI, about, debug",
     [P + "/more/settings/screen/*.kt", P + "/more/settings/screen/about", P + "/more/settings/screen/advanced", P + "/more/settings/screen/appearance", P + "/more/settings/screen/browse", P + "/more/settings/screen/data", P + "/more/settings/screen/debug", T + "/ui/setting/SettingsScreen.kt"]),
    ("47b-settings-widgets-search-more", "Settings widgets/search infrastructure, the More tab, settings main menu, ui/setting screens",
     [P + "/more/settings/widget", P + "/more/settings/*.kt", P + "/more/*.kt", T + "/ui/more/MoreTab.kt", T + "/ui/more"]),
    ("48-settings-player-and-scripts", "Player settings screens, custom buttons editor, mpv script/config editor, code editor",
     [P + "/more/settings/screen/player", D + "/ui", DOM + "/custombuttons", DAT + "/custombutton", T + "/ui/setting/PlayerSettingsScreen.kt"]),
    ("49-theme-and-design-system", "Themes (19 color schemes), TachiyomiTheme, presentation-core components/icons/util, mihon designsystem",
     [P + "/theme", "presentation-core", M + "/core/designsystem"]),
    ("49b-android-res-values-drawables", "Android resources: values (colors/themes/dimens/styles), drawables (icons), colors; mapping to GTK CSS/icon theme",
     [RES + "/values", RES + "/values-night", RES + "/values-night-v31", RES + "/values-sw720dp", RES + "/values-v27", RES + "/values-v31", RES + "/color", RES + "/drawable", RES + "/drawable-nodpi"]),
    ("50-android-widgets", "Android home-screen widgets (presentation-widget) and their desktop equivalents",
     ["presentation-widget"]),
    ("51-i18n", "Localization: moko resources, 67 locale directories, plurals, RTL, string key inventory, translation workflow",
     ["i18n", "i18n-aniyomi"]),
    ("52-util-app", "App utilities (eu.kanade.tachiyomi.util + core util): lang, system, storage, view helpers and their Linux equivalents",
     [T + "/util", E + "/core"]),
    ("53-platform-manifest-intents-workers", "AndroidManifest components, permissions, intents/deeplinks, shortcuts, services/receivers/workers, res layouts/menus/anim, and their Linux equivalents",
     ["app/src/main/AndroidManifest.xml", "**/AndroidManifest.xml", RES + "/xml", "app/shortcuts.xml", RES + "/layout", RES + "/menu", RES + "/anim", RES + "/anim-v33", RES + "/raw", T + "/data/notification", T + "/ui/deeplink"]),
    ("54-build-release-distribution-docs", "Gradle/build logic, CI, flavors, fastlane, docs (README/CHANGELOG/CONTRIBUTING), macrobenchmark; Cargo/Flatpak/CI equivalents",
     ["build.gradle.kts", "settings.gradle.kts", "gradle", "buildSrc", ".github", "fastlane", "macrobenchmark", "README.md", "CHANGELOG.md", "CONTRIBUTING.md", "CODE_OF_CONDUCT.md", "LICENSE", "gradlew", "gradlew.bat", "*.properties", ".gitignore", ".editorconfig", ".gitattributes", "app/proguard", "app/shrinker", "app/src/main/baseline-prof.txt", "app/src/debug", "*/build.gradle.kts", "**/consumer-rules.pro", "*.pro"]),
]


import re

_RX = {}


def _glob(pat):
    # '*' stays inside one path segment, '**' crosses segments; a slash-less glob matches the basename anywhere
    if pat not in _RX:
        r = re.escape(pat).replace(r"\*\*", "\0").replace(r"\*", "[^/]*").replace("\0", ".*").replace(r"\?", "[^/]")
        _RX[pat] = re.compile(("^" if "/" in pat else r"^(?:.*/)?") + r + "$")
    return _RX[pat]


def matches(rel, pat):
    if any(c in pat for c in "*?"):
        return _glob(pat).match(rel) is not None
    return rel == pat or rel.startswith(pat if pat.endswith("/") else pat + "/")


def area_files():
    files = inv.files_to_cover()
    out = {}
    for aid, title, pats in AREAS:
        out[aid] = [f for f in files if any(matches(f, p) for p in pats)]
    return files, out


CAP_LOC, CAP_FILES = 5000, 70
DESIGN_AREAS = {"20-extension-strategy-redesign"}


def loc_of(f):
    if f.endswith("baseline-prof.txt"):
        return 1  # Android startup profile, not worth reading
    with open(os.path.join(inv.SRC, f), errors="replace") as fh:
        return sum(1 for _ in fh)


def parts_of(aid, files):
    """Contiguous (path-sorted) bins under the loc/file caps; a design area stays in one part."""
    files = sorted(files)
    if aid in DESIGN_AREAS or not files:
        return [files]
    parts, cur, cur_loc = [], [], 0
    for f in files:
        n = loc_of(f)
        if cur and (cur_loc + n > CAP_LOC or len(cur) >= CAP_FILES):
            parts.append(cur)
            cur, cur_loc = [], 0
        cur.append(f)
        cur_loc += n
    parts.append(cur)
    if len(parts) > 1 and sum(loc_of(f) for f in parts[-1]) < CAP_LOC * 0.25:
        parts[-2].extend(parts.pop())
    return parts


def main():
    if len(sys.argv) > 1 and sys.argv[1] in ("parts", "files"):
        _, by = area_files()
        if sys.argv[1] == "parts":
            out = []
            for aid, title, pats in AREAS:
                ps = parts_of(aid, by[aid])
                out.append({"id": aid, "title": title, "parts": len(ps), "design": aid in DESIGN_AREAS, "files": len(by[aid])})
            print(json.dumps(out))
        else:
            aid, k = sys.argv[2], int(sys.argv[3])
            ps = parts_of(aid, by[aid])
            for f in ps[k - 1]:
                print(f"{f}\t{loc_of(f)}")
        return
    files, by_area = area_files()
    covered = set()
    for fs in by_area.values():
        covered.update(fs)
    resid = [f for f in files if f not in covered]
    if len(sys.argv) > 1 and sys.argv[1] == "json":
        print(json.dumps({"areas": [{"id": a, "title": t, "patterns": p, "files": by_area[a]} for a, t, p in AREAS], "residual": resid}))
        return
    for aid, title, pats in AREAS:
        loc = sum(sum(1 for _ in open(os.path.join(inv.SRC, f), errors="replace")) for f in by_area[aid])
        print(f"{aid:58s} files={len(by_area[aid]):4d} loc={loc:6d}")
    print(f"\nresidual={len(resid)} of {len(files)}")
    for f in resid[:200]:
        print("  ", f)


if __name__ == "__main__":
    main()
