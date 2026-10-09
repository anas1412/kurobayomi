import re, sys, os, glob
ROOT = "/home/blackbox/aniyomi-src"
SCOPE_FILES = [l.split("\t")[0] for l in os.popen("python3 /home/blackbox/kurobayomi/plans/_tools/areas.py files 47-settings-screens-general 1").read().splitlines() if l]
SUPPORT_FILES = [
 "app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReadingMode.kt",
 "app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderOrientation.kt",
 "app/src/main/java/eu/kanade/tachiyomi/ui/reader/setting/ReaderPreferences.kt",
 "app/src/main/java/eu/kanade/domain/ui/model/AppTheme.kt",
 "app/src/main/java/eu/kanade/domain/ui/model/NavStyle.kt",
 "app/src/main/java/eu/kanade/domain/ui/model/StartScreen.kt",
 "app/src/main/java/eu/kanade/domain/ui/model/TabletUiMode.kt",
 "app/src/main/java/eu/kanade/domain/track/model/AutoTrackState.kt",
 "app/src/main/java/eu/kanade/domain/base/BasePreferences.kt",
 "core/common/src/main/java/eu/kanade/tachiyomi/core/security/SecurityPreferences.kt",
 "app/src/main/java/eu/kanade/presentation/category/CategoryExtensions.kt",
 "app/src/main/java/eu/kanade/tachiyomi/util/system/AuthenticatorUtil.kt",
 "app/src/main/java/eu/kanade/presentation/util/TimeUtils.kt",
 "app/src/main/java/eu/kanade/tachiyomi/data/export/LibraryExporter.kt",
 "app/src/main/java/eu/kanade/tachiyomi/data/updater/AppUpdateChecker.kt",
 "app/src/main/java/eu/kanade/tachiyomi/util/CrashLogUtil.kt",
 "app/src/main/java/eu/kanade/presentation/more/settings/widget/BasePreferenceWidget.kt",
 "app/src/main/java/eu/kanade/presentation/more/settings/widget/TriStateListDialog.kt",
 "app/src/main/java/eu/kanade/presentation/more/settings/widget/ListPreferenceWidget.kt",
 "app/src/main/java/eu/kanade/presentation/more/settings/PreferenceItem.kt",
 "app/src/main/java/eu/kanade/presentation/more/settings/PreferenceScreen.kt",
]
# all strings in both string files
def load(path):
    out = {}
    root = __import__("xml.etree.ElementTree", fromlist=["x"]).parse(path).getroot()
    for el in root:
        if el.tag in ("string", "plurals") and el.get("name"):  # plurals handled below
            if el.tag == "string":
                out[el.get("name")] = ("string", "".join(el.itertext()))
            else:
                items = {i.get("quantity"): "".join(i.itertext()) for i in el.findall("item")}
                out[el.get("name")] = ("plurals", items)
    return out
def loadall(d):
    out = load(d + "/strings.xml")
    out.update(load(d + "/plurals.xml"))
    return out
MR = loadall(ROOT + "/i18n/src/commonMain/moko-resources/base")
AY = loadall(ROOT + "/i18n-aniyomi/src/commonMain/moko-resources/base")
used = {}
for f in SCOPE_FILES + SUPPORT_FILES:
    src = open(os.path.join(ROOT, f), encoding="utf-8").read()
    for m in re.finditer(r"\b(MR|AYMR)\.(strings|plurals)\.(\w+)", src):
        used.setdefault((m.group(1), m.group(2), m.group(3)), set()).add(f.split("/")[-1])
rows = []
missing = []
for (ns, kind, key), files in sorted(used.items(), key=lambda x: (x[0][0], x[0][2])):
    table = MR if ns == "MR" else AY
    if key not in table:
        missing.append(f"{ns}.{kind}.{key}")
        continue
    k, v = table[key]
    if k == "string":
        val = v.replace("\n", " ").replace("|", "\\|")
    else:
        val = "; ".join(f"{q}: {t}" for q, t in v.items()).replace("|", "\\|")
    rows.append(f"| `{ns}.{kind}.{key}` | {val} | {', '.join(sorted(files))} |")
print("\n".join(rows))
sys.stderr.write("MISSING: " + ", ".join(missing) + "\n")
sys.stderr.write(f"count={len(rows)}\n")
