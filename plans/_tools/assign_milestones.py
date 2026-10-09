#!/usr/bin/env python3
"""Assign every task in TASKS.csv to a milestone (plans/milestones/Mxx-*.md) and emit the roadmap numbers.

Re-runnable and idempotent. Inputs:
  plans/TASKS.csv                   (run `tasks.py export` first)
  plans/milestones/rules.json       (milestone definitions, area -> home milestone, task overrides, priority floors)
Algorithm:
  1. base(t)  = task_home[t] if present, else area_home[area of t], then raised to priority_floor[p] where p is
     the effective priority of t: its own priority, or that of a more urgent explicit dependent (so a P0 task
     that needs a P2 helper pulls the helper to its area home instead of the P2 floor).
     area:X dependencies raise base(t) to area_home[X]. Task-level edges from area tasks would merge
     most areas into one cycle, so area:X is enforced at area-home granularity only.
  2. Edges pred -> t (t depends on pred) come from explicit T-id dependencies.
  3. Strongly connected components are collapsed (members must share a milestone).
  4. milestone(SCC) = max(base over members, milestone of every predecessor SCC). Never lower, so no
     task sits earlier than an explicit T-id it depends on.
Outputs:
  plans/milestones/Mxx-<slug>.md    regenerated fully each run
  plans/ROADMAP.md                  the block between the GENERATED markers is regenerated
  stdout                            per-milestone totals and the critical path estimate
"""
import csv, json, os, re, sys, collections

HERE = os.path.dirname(os.path.abspath(__file__))
PLANS = os.path.abspath(os.path.join(HERE, ".."))
MS_DIR = os.path.join(PLANS, "milestones")
RANK = {"P0": 0, "P1": 1, "P2": 2, "P3": 3}
DAYS = {"S": 1, "M": 3, "L": 7, "XL": 15}
BEGIN, END = "<!-- BEGIN GENERATED: assign_milestones.py -->", "<!-- END GENERATED: assign_milestones.py -->"


def load():
    rules = json.load(open(os.path.join(MS_DIR, "rules.json"), encoding="utf-8"))
    rows = list(csv.DictReader(open(os.path.join(PLANS, "TASKS.csv"), encoding="utf-8")))
    return rules, rows


def sccs(nodes, succ):
    """Iterative Tarjan. Returns components in reverse topological order of the edge direction."""
    index, low, on, stack, out, idx = {}, {}, set(), [], [], 0
    for root in nodes:
        if root in index:
            continue
        work = [(root, iter(succ.get(root, ())))]
        index[root] = low[root] = idx; idx += 1; stack.append(root); on.add(root)
        while work:
            v, it = work[-1]
            advanced = False
            for w in it:
                if w not in index:
                    index[w] = low[w] = idx; idx += 1; stack.append(w); on.add(w)
                    work.append((w, iter(succ.get(w, ()))))
                    advanced = True
                    break
                if w in on:
                    low[v] = min(low[v], index[w])
            if advanced:
                continue
            work.pop()
            if work:
                low[work[-1][0]] = min(low[work[-1][0]], low[v])
            if low[v] == index[v]:
                comp = []
                while True:
                    w = stack.pop(); on.discard(w); comp.append(w)
                    if w == v:
                        break
                out.append(comp)
    return out


def compute(rules, rows):
    ms_order = [m["id"] for m in rules["milestones"]]
    rank_of = {m: i for i, m in enumerate(ms_order)}
    area_home, task_home, floor = rules["area_home"], rules["task_home"], rules["priority_floor"]
    ids = [r["id"] for r in rows]
    by = {r["id"]: r for r in rows}
    area = {r["id"]: r["file"].split("/")[0] for r in rows}
    errs = []
    # effective priority: a task is as urgent as its most urgent explicit dependent (relaxed to a fixpoint)
    eff = {r["id"]: RANK[r["priority"]] for r in rows}
    deps_of = {r["id"]: [d for d in filter(None, r["depends"].split(";")) if d.startswith("T-") and d in by] for r in rows}
    changed = True
    while changed:
        changed = False
        for t, ds in deps_of.items():
            for d in ds:
                if eff[t] < eff[d]:
                    eff[d] = eff[t]
                    changed = True
    prio_of = {v: k for k, v in RANK.items()}
    base = {}
    for r in rows:
        t = r["id"]
        home = task_home.get(t) or area_home.get(area[t])
        if home is None:
            errs.append(f"no home milestone for {t} (area {area[t]})")
            continue
        fl = floor.get(prio_of[eff[t]], ms_order[0])
        base[t] = home if rank_of[home] >= rank_of[fl] else fl
    pred = collections.defaultdict(set)
    for r in rows:
        t = r["id"]
        for d in filter(None, r["depends"].split(";")):
            if d.startswith("T-"):
                if d not in by:
                    errs.append(f"dangling dependency {d} of {t}")
                else:
                    pred[t].add(d)
            elif d.startswith("area:"):
                # area dependencies are a floor at the area's home milestone (see module docstring)
                x = d[5:]
                if x not in area_home:
                    errs.append(f"unknown area {d} of {t}")
                    continue
                if rank_of[area_home[x]] > rank_of[base[t]]:
                    base[t] = area_home[x]
    succ = collections.defaultdict(set)
    for t, ps in pred.items():
        for p in ps:
            succ[p].add(t)
    comps = sccs(ids, succ)
    comp_of = {}
    for i, c in enumerate(comps):
        for t in c:
            comp_of[t] = i
    val = {}
    # reverse of Tarjan emission order is a topological order with predecessors first
    for i in reversed(range(len(comps))):
        c = comps[i]
        m = max(rank_of[base[t]] for t in c)
        for t in c:
            for p in pred.get(t, ()):
                if comp_of[p] != i:
                    m = max(m, val[p])
        for t in c:
            val[t] = m
    assign = {t: ms_order[val[t]] for t in ids}
    # critical path (days) over the condensed DAG, collapsing SCCs
    weight = [sum(DAYS[by[t]["effort"]] for t in c) for c in comps]
    longest = [0] * len(comps)
    for i in reversed(range(len(comps))):
        best = 0
        for t in comps[i]:
            for p in pred.get(t, ()):
                j = comp_of[p]
                if j != i:
                    best = max(best, longest[j])
        longest[i] = best + weight[i]
    biggest = max(len(c) for c in comps)
    stats = {"sccs_multi": sum(1 for c in comps if len(c) > 1), "largest_scc": biggest,
             "moved_by_deps": sum(1 for t in ids if rank_of[assign[t]] > rank_of[base[t]]),
             "critical_path_days": max(longest) if longest else 0, "errors": errs}
    return ms_order, assign, base, area, by, stats


def write_milestones(rules, ms_order, assign, area, by):
    for f in os.listdir(MS_DIR):
        if re.match(r"M\d+-.*\.md$", f):
            os.remove(os.path.join(MS_DIR, f))
    for m in rules["milestones"]:
        mid = m["id"]
        mine = sorted([t for t in assign if assign[t] == mid], key=lambda t: t)
        areas = sorted({area[t] for t in mine})
        days = sum(DAYS[by[t]["effort"]] for t in mine)
        pc = collections.Counter(by[t]["priority"] for t in mine)
        lines = [f"# {mid} — {m['name']}", "",
                 f"Slug: `{mid}-{m['slug']}`", "",
                 f"Goal: {m['goal']}", "", "Exit criteria:"]
        lines += [f"- {e}" for e in m["exit"]]
        lines += ["", f"Tasks: {len(mine)} (P0 {pc.get('P0', 0)}, P1 {pc.get('P1', 0)}, P2 {pc.get('P2', 0)}, P3 {pc.get('P3', 0)}); "
                  f"effort estimate {days} days (S=1, M=3, L=7, XL=15).", "",
                  "Areas: " + ", ".join(f"area:{a}" for a in areas), "", "## Tasks", ""]
        for t in mine:
            r = by[t]
            lines.append(f"- {t} — {r['title']} ({r['priority']}, {r['effort']})")
        lines.append("")
        with open(os.path.join(MS_DIR, f"{mid}-{m['slug']}.md"), "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))


def roadmap_block(rules, assign, by):
    rows = ["| Milestone | Name | Tasks | P0 | P1 | P2 | P3 | Effort (days) | Cumulative (days) |", "|---|---|---:|---:|---:|---:|---:|---:|---:|"]
    cum = 0
    for m in rules["milestones"]:
        mine = [t for t in assign if assign[t] == m["id"]]
        pc = collections.Counter(by[t]["priority"] for t in mine)
        days = sum(DAYS[by[t]["effort"]] for t in mine)
        cum += days
        rows.append(f"| {m['id']} | {m['name']} | {len(mine)} | {pc.get('P0', 0)} | {pc.get('P1', 0)} | {pc.get('P2', 0)} | {pc.get('P3', 0)} | {days} | {cum} |")
    out = ["\n".join(rows), ""]
    for m in rules["milestones"]:
        out.append(f"**{m['id']} {m['name']}.** Goal: {m['goal']}")
        out += [f"- Exit: {e}" for e in m["exit"]]
        out.append("")
    return "\n".join(out).rstrip()


def main():
    rules, rows = load()
    ms_order, assign, base, area, by, stats = compute(rules, rows)
    if stats["errors"]:
        print("\n".join(stats["errors"]))
        sys.exit(1)
    write_milestones(rules, ms_order, assign, area, by)
    block = BEGIN + "\n" + roadmap_block(rules, assign, by) + "\n" + END
    rm = os.path.join(PLANS, "ROADMAP.md")
    if os.path.exists(rm):
        txt = open(rm, encoding="utf-8").read()
        if BEGIN in txt and END in txt:
            pre, rest = txt.split(BEGIN, 1)
            post = rest.split(END, 1)[1]
            txt = pre + block + post
            open(rm, "w", encoding="utf-8").write(txt)
    print(roadmap_block(rules, assign, by))
    print(json.dumps({k: v for k, v in stats.items() if k != "errors"}))
    print("total", len(assign))


if __name__ == "__main__":
    main()
