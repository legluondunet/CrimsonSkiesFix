#!/usr/bin/env python3
"""Delete only old, provably merged local PR branches. Never delete unmerged work."""
import argparse
from datetime import datetime, timedelta, timezone
import json
import os
import sys
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen
import unittest

# These are never deleted even if a closed/merged PR exists.
KEEP_PREFIXES = ("release/", "hotfix/", "maintenance/", "archive/")
KEEP_NAMES = {"main", "master", "develop", "development", "gh-pages"}

def utc_date(value):
    return datetime.fromisoformat(value.replace("Z", "+00:00"))

def assess_branch(branch, prs, *, repo, default, now, min_age_days):
    """Return (reason, matching PR number) where reason 'eligible' permits deletion."""
    name = branch["name"]
    if name == default or name in KEEP_NAMES:
        return "reserved", None
    if name.startswith(KEEP_PREFIXES):
        return "reserved-prefix", None
    if branch.get("protected"):
        return "protected", None
    history = [
        pr for pr in prs
        if pr.get("head", {}).get("ref") == name
        and (pr.get("head", {}).get("repo") or {}).get("full_name") == repo
    ]
    if not history:
        return "no-pr-history", None
    if any(pr.get("state") == "open" for pr in history):
        return "open-pr", None
    latest = max(history, key=lambda pr: pr["number"])
    if not latest.get("merged_at"):
        return "closed-unmerged", latest["number"]
    if (latest.get("base", {}).get("repo") or {}).get("full_name") != repo:
        return "foreign-base-repo", latest["number"]
    if latest.get("base", {}).get("ref") != default:
        return "merged-into-nondefault", latest["number"]
    # A branch reused after a merger (even if it has its old PR) is not safe.
    if latest.get("head", {}).get("sha") != branch["commit"]["sha"]:
        return "tip-moved", latest["number"]
    if utc_date(latest["merged_at"]) > now - timedelta(days=min_age_days):
        return "recent-merge", latest["number"]
    return "eligible", latest["number"]

def request(repo, token, method, suffix):
    url = f"https://api.github.com/repos/{repo}{suffix}"
    headers = {
        "Accept": "application/vnd.github+json",
        "Authorization": f"Bearer {token}",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "CrimsonSkiesFix-safe-branch-pruner",
    }
    try:
        with urlopen(Request(url, method=method, headers=headers), timeout=50) as res:
            data = res.read()
            return json.loads(data) if data else None
    except HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")[:300]
        raise RuntimeError(f"{method} {suffix} HTTP {exc.code}: {detail}") from exc
    except (URLError, TimeoutError) as exc:
        raise RuntimeError(f"{method} {suffix}: {exc}") from exc

def pages(repo, token, resource):
    result = []
    for page in range(1, 101):
        separator = "&" if "?" in resource else "?"
        batch = request(repo, token, "GET", f"{resource}{separator}per_page=100&page={page}")
        if not isinstance(batch, list):
            raise RuntimeError(f"Unexpected GitHub collection response: {resource}")
        result.extend(batch)
        if len(batch) < 100:
            return result
    raise RuntimeError("Safety stop: more than 10,000 entries; no deletions")

def fresh_is_safe(repo, token, branch, default, matched_pr, now, min_age_days):
    """Revalidate exact branch SHA and PR state just before each DELETE."""
    name = branch["name"]
    encoded = quote(name, safe="/")
    fresh = request(repo, token, "GET", f"/branches/{encoded}")
    if fresh["commit"]["sha"] != branch["commit"]["sha"] or fresh.get("protected"):
        return False, "branch-tip-or-protection-changed"
    # Recheck ALL PRs for this head. A new open or closed-unmerged PR, or
    # newly added commits, must prevent deletion even if an older PR merged.
    owner = repo.split("/", 1)[0]
    head = quote(f"{owner}:{name}", safe="")
    history = request(repo, token, "GET", f"/pulls?state=all&head={head}&per_page=100")
    if not isinstance(history, list) or len(history) >= 100:
        return False, "pr-history-unavailable-or-over-limit"
    verdict, current_pr = assess_branch(fresh, history, repo=repo, default=default,
                                        now=now, min_age_days=min_age_days)
    if verdict != "eligible" or current_pr != matched_pr:
        return False, f"pr-changed-or-no-longer-valid:{verdict}"
    return True, "eligible"

def summarize(lines):
    output = "\n".join(lines)
    print(output, flush=True)
    path = os.getenv("GITHUB_STEP_SUMMARY")
    if path:
        with open(path, "a", encoding="utf-8") as file:
            file.write(output + "\n")

def prune(args):
    token = os.environ.get("GH_TOKEN")
    repo = os.environ.get("GITHUB_REPOSITORY", "")
    if not token or repo != "legluondunet/CrimsonSkiesFix":
        raise RuntimeError("Requires the authorized CrimsonSkiesFix repository token")
    if not 0 <= args.min_age_days <= 365:
        raise ValueError("min_age_days must be 0..365")
    info = request(repo, token, "GET", "")
    default = info["default_branch"]
    if default != "main":
        raise RuntimeError("Unexpected default branch: stop without deleting")
    # Gather everything before DELETE. Refuse partial inventories.
    branches = pages(repo, token, "/branches")
    pulls = pages(repo, token, "/pulls?state=all")
    now = datetime.now(timezone.utc)
    results = [
        (b, *assess_branch(b, pulls, repo=repo, default=default,
                           now=now, min_age_days=args.min_age_days))
        for b in branches
    ]
    candidates = [(b, pr) for b, reason, pr in results if reason == "eligible"]
    counts = {}
    for _, reason, _ in results:
        counts[reason] = counts.get(reason, 0) + 1
    lines = [
        "## CrimsonSkiesFix branch cleanup",
        f"- Mode: **{'APPLY' if args.apply else 'DRY RUN'}**",
        f"- Repository: \u0060{repo}\u0060",
        f"- Minimum merged-PR age: **{args.min_age_days} days**",
        f"- Branches checked: {len(branches)}",
        f"- Candidates: **{len(candidates)}**",
        "- Other branches (open PRs, unmerged, protected, recent, "
        "maintenance/release) are preserved.",
        "",
        "| Branch to delete | Merged PR |",
        "|---|---|",
    ]
    for b, number in candidates:
        lines.append(f"| \u0060{b['name']}\u0060 | #{number} |")
    lines += ["", "Reason counts: " + ", ".join(f"{k}={v}" for k,v in sorted(counts.items()))]
    if not args.apply:
        summarize(lines)
        return
    if os.getenv("ALLOW_BRANCH_DELETION") != "YES":
        raise RuntimeError("Explicit workflow authorization required for --apply")
    deleted, skipped, errors = [], [], []
    for b, number in candidates:
        try:
            ok, reason = fresh_is_safe(repo, token, b, default, number, now,
                                       args.min_age_days)
            if not ok:
                skipped.append((b["name"], reason))
                continue
            encoded = quote(b["name"], safe="/")
            request(repo, token, "DELETE", f"/git/refs/heads/{encoded}")
            deleted.append(b["name"])
            print(f"DELETED: {b['name']} (PR #{number})", flush=True)
        except Exception as exc:
            errors.append((b["name"], str(exc)))
            print(f"ERROR: {b['name']}: {exc}", flush=True)
    lines += ["", f"### Applied: {len(deleted)} deleted; "
              f"{len(skipped)} skipped after recheck; {len(errors)} errors."]
    lines += [f"- Deleted \u0060{x}\u0060" for x in deleted]
    lines += [f"- Skipped \u0060{x}\u0060: {why}" for x, why in skipped]
    lines += [f"- Error \u0060{x}\u0060: {why}" for x, why in errors]
    summarize(lines)
    if errors:
        sys.exit(1)

class PolicyTests(unittest.TestCase):
    def setUp(self):
        self.now = utc_date("2026-11-17T18:00:00Z")
        self.repo = "legluondunet/CrimsonSkiesFix"
        self.branch = {"name":"feature/completed", "protected":False,
                       "commit":{"sha":"abcdef"}}
        self.pr = {"number":17,"state":"closed","merged_at":"2026-11-01T12:00:00Z",
                   "head":{"ref":"feature/completed","sha":"abcdef",
                           "repo":{"full_name":self.repo}},
                   "base":{"ref":"main","repo":{"full_name":self.repo}}}
    def check(self, branch=None, prs=None, age=7):
        return assess_branch(branch or self.branch,
                             [self.pr] if prs is None else prs,
                             repo=self.repo, default="main", now=self.now,
                             min_age_days=age)[0]
    def test_merged_matches_tip_and_age(self):
        self.assertEqual(self.check(), "eligible")
        self.assertEqual(self.check(age=30), "recent-merge")
    def test_open_pr_never_deleted(self):
        pr=dict(self.pr, state="open", merged_at=None)
        self.assertEqual(self.check(prs=[pr]), "open-pr")
        self.assertEqual(self.check(prs=[self.pr,pr]), "open-pr")
    def test_unmerged_never_deleted(self):
        self.assertEqual(self.check(prs=[dict(self.pr, merged_at=None)]),
                         "closed-unmerged")
    def test_moved_branch_never_deleted(self):
        self.assertEqual(self.check(branch=dict(self.branch,
                            commit={"sha":"changed"})), "tip-moved")
    def test_special_and_protected(self):
        for name in ("main","release/2.0","maintenance/cleanup","hotfix/urgent"):
            self.assertTrue(self.check(branch=dict(self.branch,name=name))
                            .startswith(("reserved", "protected")))
        self.assertEqual(self.check(branch=dict(self.branch,protected=True)),
                         "protected")
    def test_no_history_is_preserved(self):
        self.assertEqual(self.check(prs=[]), "no-pr-history")
    def test_nondefault_merge_is_preserved(self):
        pr=dict(self.pr,base={"ref":"release","repo":{"full_name":self.repo}})
        self.assertEqual(self.check(prs=[pr]),"merged-into-nondefault")

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--apply", action="store_true")
    mode.add_argument("--dry-run", action="store_true")
    parser.add_argument("--min-age-days", type=int, default=7)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        # Do not contact GitHub or require a token during validation.
        result = unittest.main(argv=[sys.argv[0]], exit=False).result
        sys.exit(0 if result.wasSuccessful() else 1)
    prune(args)
