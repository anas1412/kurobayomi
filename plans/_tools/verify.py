#!/usr/bin/env python3
"""Whole-plan verification: coverage inventory, task graph, required files, placeholders, links."""
import json, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
PLANS = os.path.abspath(os.path.join(HERE, ".."))
sys.path.insert(0, HERE)
import areas as A


def sh(*a):
    r = subprocess.run(a, capture_output=True, text=True)
    return r.returncode, r.stdout + r.stderr


def main():
    problems = []
    rc, out = sh("python3", os.path.join(HERE, "inventory.py"), "counts")
    counts = json.loads(out)
    for cat, c in counts.items():
        if c["missing"]:
            problems.append(f"coverage:{cat} missing {c['missing']}/{c['total']}")
    rc, out = sh("python3", os.path.join(HERE, "tasks.py"), "export")
    first = out.splitlines()[0] if out else ""
    if rc:
        problems.append("tasks: " + first)
        problems += ["  " + l for l in out.splitlines()[1:21]]
    # required files per area
    for aid, title, pats in A.AREAS:
        d = os.path.join(PLANS, aid)
        names = sorted(os.listdir(d)) if os.path.isdir(d) else []
        need = {"00-overview.md": "overview", "inventory": "inventory", "behavior": "behavior", "rust-design": "rust design", "tasks": "tasks"}
        for k, label in need.items():
            if not any(k in n for n in names):
                problems.append(f"area {aid}: no {label} file")
        for n in names:
            p = os.path.join(d, n)
            if n.endswith(".md") and os.path.getsize(p) < 600:
                problems.append(f"area {aid}: {n} is nearly empty ({os.path.getsize(p)} bytes)")
    # milestones: every task assigned once, never earlier than its dependencies
    ms_dir = os.path.join(PLANS, "milestones")
    if not os.path.isdir(ms_dir):
        problems.append("milestones: plans/milestones/ missing (every task must be assigned to a Mxx-*.md milestone)")
    else:
        import csv
        tid = re.compile(r"\bT-[A-Za-z0-9]+(?:-[A-Za-z0-9]+)*-\d{3}\b")
        where = {}
        for f in sorted(os.listdir(ms_dir)):
            m = re.match(r"M(\d+)-.*\.md$", f)
            if not m:
                continue
            for t in set(tid.findall(open(os.path.join(ms_dir, f), encoding="utf-8").read())):
                if t in where and where[t][0] != int(m.group(1)):
                    problems.append(f"milestones: {t} appears in M{where[t][0]} and M{m.group(1)}")
                where.setdefault(t, (int(m.group(1)), f))
        rows = list(csv.DictReader(open(os.path.join(PLANS, "TASKS.csv"), encoding="utf-8")))
        for r in rows:
            if r["id"] not in where:
                problems.append(f"milestones: {r['id']} not assigned")
            else:
                for d in filter(None, r["depends"].split(";")):
                    if d.startswith("T-") and d in where and where[d][0] > where[r["id"]][0]:
                        problems.append(f"milestones: {r['id']} (M{where[r['id']][0]}) scheduled before its dependency {d} (M{where[d][0]})")
    # index
    idx = os.path.join(PLANS, "00-INDEX.md")
    if not os.path.exists(idx):
        problems.append("00-INDEX.md missing")
    else:
        it = open(idx, encoding="utf-8").read()
        for aid, _, _ in A.AREAS:
            if aid not in it:
                problems.append(f"00-INDEX.md does not link area {aid}")
        for f in sorted(os.listdir(PLANS)):
            if f.endswith(".md") and f != "00-INDEX.md" and f not in it:
                problems.append(f"00-INDEX.md does not mention {f}")
        for d in ("catalogs", "addenda", "milestones"):
            if os.path.isdir(os.path.join(PLANS, d)):
                for f in sorted(os.listdir(os.path.join(PLANS, d))):
                    if f.endswith(".md") and f not in it:
                        problems.append(f"00-INDEX.md does not mention {d}/{f}")
    # placeholders + links
    ph = re.compile(r"\b(TBD|TODO|FIXME|XXX|lorem ipsum|to be determined|to be filled)\b", re.I)
    link = re.compile(r"\]\((?!https?://|#|mailto:)([^)#\s]+)")
    nfiles = 0
    for root, dirs, files in os.walk(PLANS):
        dirs[:] = [x for x in dirs if x != "_tools"]
        for f in files:
            if not f.endswith(".md"):
                continue
            nfiles += 1
            p = os.path.join(root, f)
            t = open(p, encoding="utf-8", errors="replace").read()
            for m in ph.finditer(t):
                line = t.count("\n", 0, m.start()) + 1
                problems.append(f"placeholder '{m.group(1)}' {os.path.relpath(p, PLANS)}:{line}")
            for m in link.finditer(t):
                tgt = os.path.normpath(os.path.join(root, m.group(1)))
                if not os.path.exists(tgt):
                    problems.append(f"broken link {os.path.relpath(p, PLANS)} -> {m.group(1)}")
    print(f"md files: {nfiles}")
    print(json.dumps(counts))
    print("PROBLEMS: %d" % len(problems))
    for x in problems[:300]:
        print(" -", x)
    sys.exit(1 if problems else 0)


if __name__ == "__main__":
    main()
