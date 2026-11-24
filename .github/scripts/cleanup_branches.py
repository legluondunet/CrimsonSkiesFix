#!/usr/bin/env python3
"""Prune branches only when no open pull request depends on them.

Preserve main, active local PR head branches, and every base branch used
by an open stacked PR. The latter is essential for PR #69's diagnostic
branch chain: deleting a PR base breaks the pull request diff.
Explicitly retained research archives are exempt even after PR closure.
Never infer obsolescence merely from age or a draft PR's title.
"""
import argparse
import json
import os
import sys
from urllib.error import HTTPError, URLError
from urllib.parse import quote
from urllib.request import Request, urlopen
import unittest

REPOSITORY = "legluondunet/CrimsonSkiesFix"
# Keep the exact base of the closed cursor PR so its original diff remains usable.
ARCHIVED_BRANCHES = {"archive/smooth-menu-cursor-20261124",
                     "diagnostic/gos-menu-mouse-input-cadence-20261121"}

def local_open_heads(prs, repo):
    """Get open PR heads in this repository, ignoring unrelated fork heads."""
    heads = set()
    for pr in prs:
        if pr.get("state") != "open":
            continue
        head = pr.get("head") or {}
        name = head.get("ref")
        if not isinstance(name, str) or not name:
            raise RuntimeError("Open PR without a valid head branch: abort cleanup")
        remote = (head.get("repo") or {}).get("full_name")
        # Missing repository identity is ambiguous: protect the name rather
        # than risk deleting a local branch associated with an open PR.
        if remote is None or remote.casefold() == repo.casefold():
            heads.add(name)
    return heads

def open_pr_dependencies(prs, repo):
    """Preserve both heads and bases of all open local pull requests."""
    refs = local_open_heads(prs, repo)
    for pr in prs:
        if pr.get("state") != "open":
            continue
        base = pr.get("base") or {}
        name = base.get("ref")
        remote = (base.get("repo") or {}).get("full_name")
        if not isinstance(name, str) or not name:
            raise RuntimeError("Open PR without a valid base branch: abort cleanup")
        if remote is None or remote.casefold() == repo.casefold():
            refs.add(name)
    return refs

def decision(branch, open_heads, default):
    name = branch["name"]
    if name == default:
        return "keep-main"
    if name in ARCHIVED_BRANCHES:
        return "keep-archive"
    if name in open_heads:
        return "keep-open-pr"
    # GitHub won't allow ref deletion if branch protection is enabled.
    # Report the block rather than silently bypassing protection.
    if branch.get("protected"):
        return "blocked-protected"
    return "delete"

def github(repo, token, method, suffix):
    url = "https://api.github.com/repos/" + repo + suffix
    headers = {
        "Authorization": "Bearer " + token,
        "Accept": "application/vnd.github+json",
        "X-GitHub-Api-Version": "2022-11-28",
        "User-Agent": "CrimsonSkiesFix-branch-cleanup",
    }
    try:
        with urlopen(Request(url, headers=headers, method=method), timeout=60) as response:
            body = response.read()
            return json.loads(body) if body else None
    except HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")[:300]
        raise RuntimeError(f"{method} {suffix}: HTTP {exc.code}: {detail}") from exc
    except (URLError, TimeoutError) as exc:
        raise RuntimeError(f"{method} {suffix}: {exc}") from exc

def collect(repo, token, resource):
    """Fetch the entire collection; partial inventories never authorize deletion."""
    result = []
    for page in range(1, 101):
        sep = "&" if "?" in resource else "?"
        batch = github(repo, token, "GET",
                       f"{resource}{sep}per_page=100&page={page}")
        if not isinstance(batch, list):
            raise RuntimeError(f"Invalid GitHub list response for {resource}")
        result.extend(batch)
        if len(batch) < 100:
            return result
    raise RuntimeError("Pagination safety limit reached; no deletion")

def still_safe(repo, token, old_branch, default):
    """Recheck a branch immediately before its deletion."""
    name = old_branch["name"]
    encoded = quote(name, safe="/")
    new_branch = github(repo, token, "GET", f"/branches/{encoded}")
    if new_branch["commit"]["sha"] != old_branch["commit"]["sha"]:
        return False, "branch HEAD changed since simulation"
    if new_branch.get("protected"):
        return False, "GitHub branch protection enabled"
    if name == default:
        return False, "default branch"
    if name in ARCHIVED_BRANCHES:
        return False, "explicitly retained research archive"
    owner = repo.split("/", 1)[0]
    query = quote(owner + ":" + name, safe="")
    # Fetch ALL open PRs. A branch can become a newly selected BASE
    # of an unrelated open PR without becoming its head.
    related = collect(repo, token, "/pulls?state=open")
    if name in open_pr_dependencies(related, repo):
        return False, "required by open PR as head or base"
    return True, "safe"

def publish(lines):
    report = "\n".join(lines)
    print(report, flush=True)
    target = os.getenv("GITHUB_STEP_SUMMARY")
    if target:
        with open(target, "a", encoding="utf-8") as file:
            file.write(report + "\n")

def run_cleanup(apply):
    repo = os.getenv("GITHUB_REPOSITORY", "")
    token = os.getenv("GH_TOKEN", "")
    if repo != REPOSITORY or not token:
        raise RuntimeError("Expected repository and GitHub Actions token not provided")
    meta = github(repo, token, "GET", "")
    default = meta["default_branch"]
    if default != "main":
        raise RuntimeError("The default branch is no longer main: abort")

    # Obtain a complete snapshot before any deletion.
    branches = collect(repo, token, "/branches")
    open_prs = collect(repo, token, "/pulls?state=open")
    heads = open_pr_dependencies(open_prs, repo)
    for branch in branches:
        if not branch.get("name") or not (branch.get("commit") or {}).get("sha"):
            raise RuntimeError("Incomplete branch inventory; refusing all deletion")

    categories = [(branch, decision(branch, heads, default)) for branch in branches]
    candidates = [branch for branch, verdict in categories if verdict == "delete"]
    protected = [branch["name"] for branch, verdict in categories
                 if verdict == "blocked-protected"]
    kept_pr = [branch["name"] for branch, verdict in categories
               if verdict == "keep-open-pr"]
    lines = [
        "## CrimsonSkiesFix branch cleanup",
        f"- Mode: **{'APPLY' if apply else 'DRY RUN'}**",
        f"- Branches inspected: {len(branches)}",
        f"- Branches with an open PR preserved: {len(kept_pr)}",
        f"- Research archive branches preserved: {sum(verdict == 'keep-archive' for _, verdict in categories)}",
        f"- Delete candidates: **{len(candidates)}**",
        f"- GitHub-protected branches blocking deletion: {len(protected)}",
        "- Policy: keep main, explicit research archives and every head/base referenced by an OPEN local PR.",
        "- Includes MERGED, CLOSED WITHOUT MERGE, and NO-PR branches only if unreferenced.",
        "",
        "| Candidate branch | Outcome |",
        "|---|---|",
    ]
    for branch in candidates:
        lines.append(f"| \`{branch['name']}\` | {'Delete' if apply else 'Would delete'} |")
    for name in protected:
        lines.append(f"| \`{name}\` | Blocked by GitHub protection |")
    if not apply:
        publish(lines + ["", "**Simulation only. No branches were deleted.**"])
        return

    if os.getenv("ALLOW_BRANCH_DELETION") != "YES":
        raise RuntimeError("Missing deletion authorization")
    if os.getenv("GITHUB_REF") != "refs/heads/main":
        raise RuntimeError("Apply is permitted only when running from main")

    deleted, skipped, failures = [], [], []
    for branch in candidates:
        name = branch["name"]
        try:
            ok, reason = still_safe(repo, token, branch, default)
            if not ok:
                skipped.append((name, reason))
                continue
            encoded = quote(name, safe="/")
            github(repo, token, "DELETE", f"/git/refs/heads/{encoded}")
            deleted.append(name)
            print(f"DELETED {name}", flush=True)
        except Exception as exc:
            failures.append((name, str(exc)))
            print(f"FAILED {name}: {exc}", flush=True)
    lines.extend([
        "",
        f"### Result: {len(deleted)} deleted, {len(skipped)} skipped, "
        f"{len(failures)} failed.",
    ])
    lines.extend(f"- Deleted: \`{name}\`" for name in deleted)
    lines.extend(f"- Skipped: \`{name}\`: {reason}" for name, reason in skipped)
    lines.extend(f"- Failed: \`{name}\`: {reason}" for name, reason in failures)
    publish(lines)
    if failures:
        sys.exit(1)

class PolicyTests(unittest.TestCase):
    def setUp(self):
        self.repo = REPOSITORY
        self.b = {"name": "feature/sample", "protected": False,
                  "commit": {"sha": "headsha"}}
        self.pr = {"number": 123, "state": "open",
                   "head": {"ref": "feature/sample",
                            "repo": {"full_name": self.repo}}}

    def test_main_is_always_kept(self):
        self.assertEqual(decision(dict(self.b, name="main"), set(), "main"),
                         "keep-main")

    def test_explicit_research_archive_is_kept_after_pr_closure(self):
        self.assertEqual(decision(dict(self.b, name="archive/smooth-menu-cursor-20261124"), set(), "main"),
                         "keep-archive")

    def test_archived_cursor_pr_base_is_kept(self):
        self.assertEqual(decision(dict(self.b, name="diagnostic/gos-menu-mouse-input-cadence-20261121"), set(), "main"),
                         "keep-archive")

    def test_open_local_pr_is_kept(self):
        heads = local_open_heads([self.pr], self.repo)
        self.assertEqual(decision(self.b, heads, "main"), "keep-open-pr")

    def test_open_pr_to_other_base_still_kept(self):
        pr = dict(self.pr, base={"ref": "release"})
        self.assertIn(self.b["name"], local_open_heads([pr], self.repo))

    def test_open_pr_base_is_preserved_when_head_is_different(self):
        pr = dict(self.pr, base={"ref":"diagnostic/previous",
                                 "repo":{"full_name":self.repo}})
        deps = open_pr_dependencies([pr], self.repo)
        self.assertIn("diagnostic/previous", deps)
        self.assertIn("feature/sample", deps)
        self.assertEqual(decision(dict(self.b,name="diagnostic/previous"),
                                  deps, "main"), "keep-open-pr")

    def test_closed_pr_does_not_protect_old_base_or_head(self):
        old = dict(self.pr, state="closed",
                   base={"ref":"old-base","repo":{"full_name":self.repo}})
        self.assertEqual(open_pr_dependencies([old], self.repo), set())

    def test_different_repo_base_is_not_protected(self):
        pr = dict(self.pr, base={"ref":"foreign-base",
                                 "repo":{"full_name":"other/repo"}})
        self.assertNotIn("foreign-base", open_pr_dependencies([pr], self.repo))

    def test_fork_pr_same_name_does_not_protect_local_branch(self):
        pr = dict(self.pr, head={"ref": "feature/sample",
                                  "repo": {"full_name": "other/project"}})
        self.assertEqual(decision(self.b, local_open_heads([pr], self.repo),
                                  "main"), "delete")

    def test_merged_or_closed_unmerged_branches_are_deleted(self):
        # Only OPEN PRs are supplied by the API, so closed/merged branches
        # have no exemption regardless of their merge history.
        self.assertEqual(decision(self.b, set(), "main"), "delete")

    def test_branch_with_no_pr_is_deleted(self):
        self.assertEqual(decision(dict(self.b, name="untracked"),
                                  set(), "main"), "delete")

    def test_maintenance_release_and_hotfix_branches_are_deleted(self):
        for name in ("maintenance/purge", "release/2.0", "hotfix/urgent",
                     "master", "develop"):
            self.assertEqual(decision(dict(self.b, name=name),
                                      set(), "main"), "delete")

    def test_protected_branch_is_blocked(self):
        self.assertEqual(decision(dict(self.b, protected=True),
                                  set(), "main"), "blocked-protected")

    def test_ambiguous_open_pr_head_is_conservatively_kept(self):
        pr = dict(self.pr, head={"ref": "feature/sample", "repo": None})
        self.assertEqual(decision(self.b, local_open_heads([pr], self.repo),
                                  "main"), "keep-open-pr")

    def test_all_missing_or_closed_prs_yield_no_heads(self):
        closed = dict(self.pr, state="closed")
        self.assertEqual(local_open_heads([closed], self.repo), set())

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--dry-run", action="store_true")
    mode.add_argument("--apply", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        result = unittest.main(argv=[sys.argv[0]], exit=False).result
        sys.exit(0 if result.wasSuccessful() else 1)
    run_cleanup(apply=args.apply)
