#!/usr/bin/env python3
"""Task-file linter / exporter for the Kurobayomi plans.

Task block format (markdown):

### T-38a-001 — Short title
- Priority: P0 | P1 | P2 | P3
- Effort: S | M | L | XL
- Depends: T-12a-004, T-27a-010, area:12-network-stack   (or: none)
- Sources: repo/relative/path.kt, other/path.kt
- Acceptance:
  - observable criterion
  - another criterion

  tasks.py lint FILE...   -> format errors in those files (exit 1 on any)
  tasks.py export         -> validates every *tasks*.md under plans/, writes plans/TASKS.csv, prints stats
"""
import csv, glob, os, re, sys, collections

PLANS = os.path.abspath(os.path.join(os.path.dirname(__file__), ".."))
ID = re.compile(r"^T-\d{2}[a-z]?[a-z]?-\d{3}$|^T-[a-z]+(?:-[a-z0-9]+)*-\d{3}$")
HEAD = re.compile(r"^###\s+(T-[A-Za-z0-9-]+)\s+[—-]\s+(.+?)\s*$")
DEP = re.compile(r"^(T-[A-Za-z0-9-]+|area:[A-Za-z0-9-]+|none)$")


def parse(path):
    tasks, errs, cur = [], [], None
    with open(path, encoding="utf-8") as fh:
        lines = fh.read().splitlines()
    for n, line in enumerate(lines, 1):
        m = HEAD.match(line)
        if m:
            cur = {"id": m.group(1), "title": m.group(2), "file": path, "line": n, "accept": 0}
            tasks.append(cur)
            if not ID.match(cur["id"]):
                errs.append(f"{path}:{n}: bad task id {cur['id']}")
            continue
        if line.startswith("### T-"):
            errs.append(f"{path}:{n}: malformed task heading (need '### T-ID — title')")
        if cur is None:
            continue
        m = re.match(r"^- (Priority|Effort|Depends|Sources|Acceptance):\s*(.*)$", line)
        if m:
            k, v = m.group(1).lower(), m.group(2).strip()
            cur[k] = v
        elif re.match(r"^\s+- \S", line) and "acceptance" in cur:
            cur["accept"] += 1
    for t in tasks:
        loc = f"{t['file']}:{t['line']} {t['id']}"
        if t.get("priority") not in {"P0", "P1", "P2", "P3"}:
            errs.append(f"{loc}: Priority must be P0|P1|P2|P3")
        if t.get("effort") not in {"S", "M", "L", "XL"}:
            errs.append(f"{loc}: Effort must be S|M|L|XL")
        deps = [d.strip() for d in t.get("depends", "").split(",") if d.strip()]
        if not deps:
            errs.append(f"{loc}: Depends missing (use 'none')")
        for d in deps:
            if not DEP.match(d):
                errs.append(f"{loc}: bad dependency token '{d}'")
        if not t.get("sources"):
            errs.append(f"{loc}: Sources missing")
        if t["accept"] == 0 and not t.get("acceptance"):
            errs.append(f"{loc}: Acceptance criteria missing")
        t["deps"] = deps
    return tasks, errs


def all_task_files():
    out = []
    for root, dirs, files in os.walk(PLANS):
        dirs[:] = [d for d in dirs if d != "_tools"]
        for f in files:
            if f.endswith(".md") and "tasks" in f.lower():
                out.append(os.path.join(root, f))
    return sorted(out)


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "export"
    if cmd == "lint":
        errs = []
        for f in sys.argv[2:]:
            t, e = parse(f)
            if not t:
                e.append(f"{f}: no tasks found")
            errs += e
        print("\n".join(errs) if errs else "ok")
        sys.exit(1 if errs else 0)
    tasks, errs = [], []
    for f in all_task_files():
        t, e = parse(f)
        tasks += t
        errs += e
    ids = collections.Counter(t["id"] for t in tasks)
    errs += [f"duplicate task id {i} x{c}" for i, c in ids.items() if c > 1]
    areas = {d for d in os.listdir(PLANS) if os.path.isdir(os.path.join(PLANS, d)) and d[:2].isdigit()}
    for t in tasks:
        for d in t["deps"]:
            if d.startswith("T-") and d not in ids:
                errs.append(f"{t['file']}:{t['line']} {t['id']}: dangling dependency {d}")
            if d.startswith("area:") and d[5:] not in areas:
                errs.append(f"{t['file']}:{t['line']} {t['id']}: unknown area {d}")
    with open(os.path.join(PLANS, "TASKS.csv"), "w", newline="", encoding="utf-8") as fh:
        w = csv.writer(fh)
        w.writerow(["id", "title", "priority", "effort", "depends", "sources", "file"])
        for t in tasks:
            w.writerow([t["id"], t["title"], t.get("priority", ""), t.get("effort", ""), ";".join(t["deps"]), t.get("sources", ""), os.path.relpath(t["file"], PLANS)])
    by_p = collections.Counter(t.get("priority", "?") for t in tasks)
    by_e = collections.Counter(t.get("effort", "?") for t in tasks)
    print(f"tasks={len(tasks)} errors={len(errs)} priority={dict(by_p)} effort={dict(by_e)}")
    for e in errs[:200]:
        print(e)
    sys.exit(1 if errs else 0)


if __name__ == "__main__":
    main()
