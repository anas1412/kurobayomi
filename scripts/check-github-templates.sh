#!/usr/bin/env bash
# Checks the GitHub issue forms and the pull request template (T-54a-015, T-54a-016).
# Usage: scripts/check-github-templates.sh [dir]    (default: packaging/github; use .github once copied)
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."
exec python3 -I - "${1:-packaging/github}" <<'PY'
import os, re, sys

try:
    import yaml
except ImportError:
    sys.exit("python3 yaml module missing (package python-yaml)")

root = sys.argv[1]
errors = []

def load(name):
    with open(os.path.join(root, "ISSUE_TEMPLATE", name), encoding="utf-8") as f:
        text = f.read()
    return text, yaml.safe_load(text)

def ids(form):
    return {b["id"]: b for b in form["body"] if "id" in b.get("attributes", {}) or "id" in b}

def check_forbidden(name, text):
    for word in ("aniyomi.org", "discord", "Android", "More > Settings", "More → Settings"):
        if word.lower() in text.lower():
            errors.append(f"{name}: still mentions '{word}'")

text, cfg = load("config.yml")
if cfg.get("blank_issues_enabled") is not False:
    errors.append("config.yml: blank_issues_enabled must be false")
if not any("kurobayomi" in l["url"] for l in cfg.get("contact_links", [])):
    errors.append("config.yml: no link to the Kurobayomi repository")
check_forbidden("config.yml", text)

text, report = load("report_issue.yml")
found = {b["id"] for b in report["body"] if "id" in b}
need = {"reproduce-steps", "expected-behavior", "actual-behavior", "log-file", "kurobayomi-version",
        "linux-distribution", "desktop-environment", "session-type", "gtk-version", "libmpv-version",
        "acknowledgements"}
if need - found:
    errors.append(f"report_issue.yml: missing fields {sorted(need - found)}")
check_forbidden("report_issue.yml", text)
if "kurobayomi/logs" not in text:
    errors.append("report_issue.yml: log directory not named")

text, feat = load("request_feature.yml")
found = {b["id"] for b in feat["body"] if "id" in b}
if not {"feature-description", "kurobayomi-version", "acknowledgements"} <= found:
    errors.append("request_feature.yml: missing fields")
check_forbidden("request_feature.yml", text)

for name, form in (("report_issue.yml", report), ("request_feature.yml", feat)):
    for key in ("name", "description", "body"):
        if key not in form:
            errors.append(f"{name}: missing '{key}'")

with open(os.path.join(root, "pull_request_template.md"), encoding="utf-8") as f:
    pr = f.read()
for needle in ("cargo xtask lint", "cargo test", "light and a dark GTK theme", "| Before | After |"):
    if needle not in pr:
        errors.append(f"pull_request_template.md: missing '{needle}'")
if re.search(r"tablet|base themes", pr, re.I):
    errors.append("pull_request_template.md: tablet or base themes wording remains")

if errors:
    print("\n".join("FAIL: " + e for e in errors), file=sys.stderr)
    sys.exit(1)
print("OK: GitHub templates")
PY
