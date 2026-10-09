#!/usr/bin/env python3
"""Aniyomi source inventory + plan-coverage checker.

  inventory.py list  [category]   -> print what must be covered (counts, or items of one category)
  inventory.py check [category]   -> print what the plans in ../ still do not mention

A plan "mentions" an item when its text appears anywhere in plans/**/*.md (excluding _tools/).
Files are matched by full repo-relative path; everything else by its name/key string.
Exit code is 0 only when nothing is missing.
"""
import os, re, sys, json, collections

SRC = os.environ.get("ANIYOMI_SRC", os.path.expanduser("~/aniyomi-src"))
PLANS = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))

SKIP_DIRS = {".git", "build", ".gradle", ".idea"}
BIN_EXT = {".png", ".webp", ".jpg", ".jpeg", ".gif", ".jar", ".ttf", ".otf", ".so", ".ico", ".keystore", ".bin"}


def walk():
    for root, dirs, files in os.walk(SRC):
        dirs[:] = [d for d in dirs if d not in SKIP_DIRS]
        for f in files:
            p = os.path.join(root, f)
            yield os.path.relpath(p, SRC)


def read(rel):
    try:
        with open(os.path.join(SRC, rel), encoding="utf-8", errors="replace") as fh:
            return fh.read()
    except OSError:
        return ""


def is_locale_resource(rel):
    return "/moko-resources/" in rel and "/base/" not in rel


def is_res_noise(rel):
    # launcher mipmaps and per-locale string files are covered by their directory, not file by file
    if rel.startswith("app/src/main/res/mipmap"):
        return True
    if is_locale_resource(rel):
        return True
    ext = os.path.splitext(rel)[1].lower()
    return ext in BIN_EXT


def files_to_cover():
    out = []
    for rel in sorted(walk()):
        if is_res_noise(rel):
            continue
        ext = os.path.splitext(rel)[1].lower()
        if ext in {".kt", ".kts", ".sq", ".sqm", ".lua", ".xml", ".toml", ".properties", ".pro", ".md", ".yml", ".yaml", ".json", ".txt", ".gradle", ".conf", ".cfg", ".proto", ".java", ".svg"} or rel.endswith(("gradlew", "gradlew.bat", "LICENSE", ".gitignore", ".editorconfig", ".gitattributes")):
            out.append(rel)
    return out


def pref_keys():
    keys = collections.defaultdict(set)
    pats = [
        re.compile(r'\.get(?:Boolean|String|Int|Long|Float|StringSet|Enum|Object)\(\s*"([^"]+)"'),
        re.compile(r'Preference\.(?:appStateKey|privateKey)\(\s*"([^"]+)"'),
        re.compile(r'"((?:pref|__PRIVATE|__APP_STATE)[A-Za-z0-9_]*)"'),
        re.compile(r'\b(?:preferenceKey|key)\s*=\s*"([a-z][a-z0-9_]+)"'),
    ]
    for rel in files_to_cover():
        if not rel.endswith(".kt"):
            continue
        t = read(rel)
        for pat in pats:
            for m in pat.finditer(t):
                keys[m.group(1)].add(rel)
    return keys


def sql_items():
    items = collections.defaultdict(set)
    for rel in files_to_cover():
        if not rel.endswith(".sq"):
            continue
        t = read(rel)
        for m in re.finditer(r"^([A-Za-z][A-Za-z0-9_]*):\s*$", t, re.M):
            items[m.group(1)].add(rel)
        for m in re.finditer(r"CREATE\s+(?:TEMP\s+)?(?:TABLE|VIEW|TRIGGER|INDEX)\s+(?:IF NOT EXISTS\s+)?([A-Za-z0-9_]+)", t, re.I):
            items[m.group(1)].add(rel)
    return items


def manifest_items():
    items = collections.defaultdict(set)
    for rel in files_to_cover():
        if not rel.endswith("AndroidManifest.xml"):
            continue
        t = read(rel)
        for m in re.finditer(r'android:name="([^"]+)"', t):
            items[m.group(1)].add(rel)
        for m in re.finditer(r'android:(?:scheme|host|mimeType|pathPattern|pathPrefix)="([^"]+)"', t):
            items[m.group(1)].add(rel)
    return items


def decls():
    items = collections.defaultdict(set)
    pat = re.compile(r"^(?!\s*(?:private|protected))\s*(?:(?:public|internal|data|sealed|abstract|open|enum|annotation|value|inline|fun|expect|actual)\s+)*(?:class|object|interface)\s+([A-Z][A-Za-z0-9_]*)", re.M)
    for rel in files_to_cover():
        if not rel.endswith(".kt") or "/test/" in rel:
            continue
        # top-level only: declarations that start at column 0
        for m in re.finditer(r"^(?:(?:public|internal|data|sealed|abstract|open|enum|annotation|value|inline|fun|expect|actual)\s+)*(?:class|object|interface)\s+([A-Z][A-Za-z0-9_]*)", read(rel), re.M):
            items[m.group(1)].add(rel)
    return items


EXT_IMPORT = re.compile(r"^import\s+((?:android|androidx|okhttp3|okio|coil3|com|org|io|rx|uy|nl|app|dev|logcat|is|me|net)\.[\w.]+)", re.M)
IGNORE_IMPORT = ("androidx.compose.", "androidx.annotation.", "android.R", "com.google.accompanist", "org.jetbrains.annotations", "org.jetbrains.compose", "androidx.core.graphics.drawable.toBitmap")


def imports():
    items = collections.defaultdict(set)
    for rel in files_to_cover():
        if not rel.endswith(".kt"):
            continue
        for m in EXT_IMPORT.finditer(read(rel)):
            fq = m.group(1)
            if fq.startswith(IGNORE_IMPORT):
                continue
            name = fq.rsplit(".", 1)[-1]
            if name[0].isupper():
                items[name].add(fq)
    return items


def hosts():
    items = collections.defaultdict(set)
    for rel in files_to_cover():
        if not rel.endswith((".kt", ".xml", ".md", ".lua")):
            continue
        if "/test/" in rel:
            continue
        for m in re.finditer(r"https?://([A-Za-z0-9.-]+\.[A-Za-z]{2,})", read(rel)):
            h = m.group(1).lower()
            if h.endswith((".png", ".jpg")) or h in {"www.w3.org", "schemas.android.com", "xmlpull.org", "www.apache.org", "apache.org", "example.com", "localhost"}:
                continue
            items[h].add(rel)
    return items


def string_keys():
    items = collections.defaultdict(set)
    for rel in walk():
        if "/moko-resources/base/" in rel and rel.endswith(("strings.xml", "plurals.xml")):
            for m in re.finditer(r'<(?:string|plurals)\s+name="([^"]+)"', read(rel)):
                items[m.group(1)].add(rel)
    return items


def locale_dirs():
    out = set()
    for rel in walk():
        m = re.match(r"(i18n(?:-aniyomi)?)/src/commonMain/moko-resources/([^/]+)/", rel)
        if m and m.group(2) != "base":
            out.add(m.group(2))
    return sorted(out)


def plan_text():
    chunks = []
    for root, dirs, files in os.walk(PLANS):
        dirs[:] = [d for d in dirs if d != "_tools"]
        for f in files:
            if f.endswith(".md"):
                with open(os.path.join(root, f), encoding="utf-8", errors="replace") as fh:
                    chunks.append(fh.read())
    return "\n".join(chunks)


CATEGORIES = {
    "files": lambda: {f: {f} for f in files_to_cover()},
    "prefs": pref_keys,
    "sql": sql_items,
    "manifest": manifest_items,
    "decls": decls,
    "imports": imports,
    "hosts": hosts,
    "strings": string_keys,
}
# whole-word match for names, substring for paths/keys
WORD = {"decls", "imports", "sql"}


def mentioned(cat, key, text, text_l):
    if cat in WORD:
        return re.search(r"(?<![A-Za-z0-9_])" + re.escape(key) + r"(?![A-Za-z0-9_])", text) is not None
    if cat == "hosts":
        return key in text_l
    return key in text


def chunk(seq, i, n):
    size = -(-len(seq) // n)
    return seq[(i - 1) * size : i * size]


def cmd_items(cat, i, n, only_missing):
    items = CATEGORIES[cat]()
    keys = sorted(items)
    if only_missing:
        text = plan_text()
        text_l = text.lower()
        keys = [k for k in keys if not mentioned(cat, k, text, text_l)]
    for k in chunk(keys, i, n):
        v = items[k]
        print(k + "\t" + ",".join(sorted(v))[:300])


SNAP = os.path.join(os.path.dirname(os.path.abspath(__file__)), "snap")


def cmd_counts():
    """Counts of still-unmentioned items per category; also snapshots them so chunked gap-fill agents see a stable list."""
    text = plan_text()
    text_l = text.lower()
    out = {}
    os.makedirs(SNAP, exist_ok=True)
    for cat in CATEGORIES:
        if cat == "strings":
            continue
        items = CATEGORIES[cat]()
        miss = sorted(k for k in items if not mentioned(cat, k, text, text_l))
        out[cat] = {"total": len(items), "missing": len(miss)}
        with open(os.path.join(SNAP, cat + ".tsv"), "w") as fh:
            for k in miss:
                fh.write(k + "\t" + ",".join(sorted(items[k]))[:300] + "\n")
    print(json.dumps(out))


def cmd_snap(cat, i, n):
    with open(os.path.join(SNAP, cat + ".tsv")) as fh:
        rows = fh.read().splitlines()
    print("\n".join(chunk(rows, i, n)))


def main():
    if len(sys.argv) > 1 and sys.argv[1] == "counts":
        return cmd_counts()
    if len(sys.argv) > 1 and sys.argv[1] == "snap":
        return cmd_snap(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]))
    if len(sys.argv) > 1 and sys.argv[1] in ("items", "missing"):
        return cmd_items(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]), sys.argv[1] == "missing")
    mode = sys.argv[1] if len(sys.argv) > 1 else "list"
    only = sys.argv[2:] or list(CATEGORIES)
    text = plan_text() if mode == "check" else ""
    text_l = text.lower()
    missing_total = 0
    report = {}
    for cat in only:
        items = CATEGORIES[cat]()
        if mode == "list":
            report[cat] = {"count": len(items)}
            if len(only) == 1:
                for k in sorted(items):
                    print(k)
            continue
        miss = sorted(k for k in items if not mentioned(cat, k, text, text_l))
        if cat == "strings":
            # string keys are covered by pattern, not one by one: report only the coverage ratio
            report[cat] = {"total": len(items), "mentioned": len(items) - len(miss)}
            continue
        report[cat] = {"total": len(items), "missing": len(miss), "items": miss}
        missing_total += len(miss)
    if mode == "list" and len(only) != 1:
        print(json.dumps(report, indent=1))
    elif mode == "check":
        loc = locale_dirs()
        if not any(l in text for l in loc[:3]):
            report["locales"] = {"note": f"{len(loc)} locale dirs, none mentioned yet"}
        if len(only) == 1 and only[0] != "strings":
            print(f"{only[0]}: {report[only[0]]['missing']} missing of {report[only[0]]['total']}")
            for k in report[only[0]]["items"]:
                print(k)
        else:
            print(json.dumps({c: ({k: v for k, v in r.items() if k != 'items'}) for c, r in report.items()}, indent=1))
        sys.exit(0 if missing_total == 0 else 1)


if __name__ == "__main__":
    main()
