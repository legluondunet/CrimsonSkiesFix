#!/usr/bin/env python3
"""Prune completed GitHub Actions workflow runs; keep the latest usable x86 build.

This removes workflow-run history (not merely the associated artifacts).
No changes to source, PRs, releases, tags or branches.
"""
import argparse
import json
import os
import sys
import time
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen
import unittest

REPO = "legluondunet/CrimsonSkiesFix"
BUILD_NAME = "CrimsonSkiesFix-UNIFIED-X86-TEST-ONLY"
ORIGINAL_PROTECTED_ARTIFACT = 14343959078
ORIGINAL_PROTECTED_RUN = 44119944628
ONE_SHOT_BRANCH = "refs/heads/maintenance/purge-old-workflow-runs-20261117"

def choose_build(artifacts):
    """Choose the newest unexpired production-candidate ZIP, never a smoke test."""
    matches = [
        a for a in artifacts
        if a.get("name") == BUILD_NAME
        and not a.get("expired", True)
        and a.get("workflow_run", {}).get("id")
        and int(a.get("size_in_bytes", 0)) > 0
    ]
    return max(matches, key=lambda a: (a.get("created_at", ""), a["id"])) if matches else None

def deletion_targets(runs, protected_run_ids):
    """Only completed, non-protected workflow runs are deletable."""
    return [
        run for run in runs
        if run.get("status") == "completed" and
        run["id"] not in protected_run_ids
    ]

def valid_inventory(runs, expected_count, protected):
    ids = [item["id"] for item in runs]
    return len(ids) == len(set(ids)) and len(ids) >= expected_count and protected in ids

class GitHub:
    def __init__(self, token):
        self.token = token
    def call(self, method, endpoint, max_attempts=14):
        url = f"https://api.github.com/repos/{REPO}{endpoint}"
        headers = {
            "Authorization": f"Bearer {self.token}",
            "Accept": "application/vnd.github+json",
            "X-GitHub-Api-Version": "2022-11-28",
            "User-Agent": "CrimsonSkiesFix-actions-run-cleanup",
        }
        for attempt in range(max_attempts):
            try:
                with urlopen(Request(url, headers=headers, method=method),
                             timeout=80) as response:
                    body = response.read()
                    return json.loads(body) if body else None
            except HTTPError as exc:
                raw = exc.read().decode("utf-8", errors="replace")
                if method == "DELETE" and exc.code == 404:
                    # Another cleanup could have removed this run.
                    return None
                remaining = exc.headers.get("X-RateLimit-Remaining")
                reset = int(exc.headers.get("X-RateLimit-Reset", 0) or 0)
                retry_after = int(exc.headers.get("Retry-After", 0) or 0)
                limited = exc.code in (403, 429) and (
                    remaining == "0" or retry_after or "rate limit" in raw.lower()
                )
                if limited:
                    delay = max(retry_after, max(reset - int(time.time()) + 3, 0), 45)
                    delay = min(delay, 3700)
                    print(f"RATE_LIMIT: {method} {endpoint}, sleeping {delay}s", flush=True)
                    time.sleep(delay)
                    continue
                if exc.code in (500, 502, 503, 504):
                    delay = min(2 ** attempt, 75)
                    time.sleep(delay)
                    continue
                raise RuntimeError(f"{method} {endpoint}: HTTP {exc.code} {raw[:300]}") from exc
            except (URLError, TimeoutError) as exc:
                time.sleep(min(2 ** attempt, 75))
                continue
        raise RuntimeError(f"API request retry limit reached: {method} {endpoint}")

    def list_all(self, resource):
        items = []
        total = None
        for page in range(1, 151):
            response = self.call("GET", f"{resource}?per_page=100&page={page}")
            if not isinstance(response, dict):
                raise RuntimeError(f"Malformed GitHub response at {resource}: {page}")
            key = "workflow_runs" if resource == "/actions/runs" else "artifacts"
            if total is None:
                total = int(response.get("total_count", -1))
            batch = response.get(key)
            if not isinstance(batch, list):
                raise RuntimeError(f"Missing {key} at page {page}")
            items.extend(batch)
            if len(batch) < 100:
                return items, total
        raise RuntimeError(f"Pagination safety stop for {resource}: no runs deleted")

def summary(lines):
    message = "\n".join(lines)
    print(message, flush=True)
    destination = os.getenv("GITHUB_STEP_SUMMARY")
    if destination:
        with open(destination, "a", encoding="utf-8") as f:
            f.write(message + "\n")

def prune(apply):
    if os.getenv("GITHUB_REPOSITORY") != REPO:
        raise RuntimeError("Repository identity mismatch")
    token = os.getenv("GH_TOKEN")
    if not token:
        raise RuntimeError("GitHub Actions token required")
    this_id = int(os.getenv("GITHUB_RUN_ID", 0))
    if not this_id:
        raise RuntimeError("This workflow run ID is unknown")
    if apply:
        if os.getenv("ALLOW_RUN_DELETION") != "YES":
            raise RuntimeError("Explicit workflow authorization missing")
        if os.getenv("GITHUB_REF") not in ("refs/heads/main", ONE_SHOT_BRANCH):
            raise RuntimeError("Deletion is only permitted from main or approved one-shot branch")

    api = GitHub(token)
    artifact_list, artifact_total = api.list_all("/actions/artifacts")
    protected_artifact = choose_build(artifact_list)
    if protected_artifact is None:
        raise RuntimeError("No unexpired unified Windows x86 build to preserve; no deletion")
    build_run_id = int(protected_artifact["workflow_run"]["id"])
    if os.getenv("GITHUB_REF") == ONE_SHOT_BRANCH:
        if (protected_artifact["id"] != ORIGINAL_PROTECTED_ARTIFACT or
                build_run_id != ORIGINAL_PROTECTED_RUN):
            raise RuntimeError("The originally requested build is not the latest; "
                               "no one-shot deletion without a new inventory")

    all_runs, total = api.list_all("/actions/runs")
    if not valid_inventory(all_runs, total, build_run_id):
        raise RuntimeError("Incomplete/duplicated run inventory or missing build run; no deletion")
    protected_ids = {build_run_id, this_id}
    targets = deletion_targets(all_runs, protected_ids)
    active_count = sum(r.get("status") != "completed" for r in all_runs)
    lines = [
        "## GitHub Actions run-history cleanup",
        f"- Mode: **{'DELETE' if apply else 'DRY RUN'}**",
        f"- Workflow runs inventoried: **{len(all_runs)}**",
        f"- Old completed runs eligible for deletion: **{len(targets)}**",
        f"- Still-active runs skipped: **{active_count}**",
        f"- Protected build artifact: \u0060{protected_artifact['id']}\u0060, "
        f"run \u0060{build_run_id}\u0060 (\u0060{BUILD_NAME}\u0060)",
        f"- Current cleanup run: \u0060{this_id}\u0060 (cannot delete itself)",
        "",
    ]
    if not apply:
        summary(lines + ["**Dry run complete. Zero workflow runs deleted.**"])
        return
    summary(lines + ["Deletion started; progress is recorded in the run log."])

    deleted = 0
    failed = []
    for index, run in enumerate(targets, start=1):
        try:
            api.call("DELETE", f"/actions/runs/{run['id']}")
            deleted += 1
            if deleted % 100 == 0:
                print(f"RUN_CLEANUP_PROGRESS deleted={deleted}/{len(targets)} "
                      f"last={run['id']}", flush=True)
            time.sleep(0.08)
        except Exception as exc:
            failed.append((run["id"], str(exc)))
            print(f"RUN_CLEANUP_FAILURE run={run['id']}: {exc}", flush=True)
            if len(failed) >= 25:
                print("RUN_CLEANUP_STOP: too many failures", flush=True)
                break

    # Recheck the most important guarantee before reporting success.
    kept_artifact = api.call("GET", f"/actions/artifacts/{protected_artifact['id']}")
    if kept_artifact is None or kept_artifact.get("expired"):
        raise RuntimeError("Protected compilation could not be verified after cleanup")
    if kept_artifact.get("workflow_run", {}).get("id") != build_run_id:
        raise RuntimeError("Protected build's workflow-run identity changed")
    api.call("GET", f"/actions/runs/{build_run_id}")

    final = [
        "## GitHub Actions cleanup result",
        f"- Deleted runs: **{deleted}**",
        f"- Errors: **{len(failed)}**",
        f"- Protected build run preserved: \u0060{build_run_id}\u0060",
        f"- Protected build artifact preserved: \u0060{protected_artifact['id']}\u0060",
    ]
    if failed:
        final.append("Failures (first 25): " +
                     ", ".join(f"{item}: {reason}" for item, reason in failed[:25]))
    summary(final)
    print(f"RUN_CLEANUP_RESULT deleted={deleted} failed={len(failed)} "
          f"protected_run={build_run_id} protected_artifact={protected_artifact['id']}",
          flush=True)
    if failed:
        sys.exit(1)

class UnitTests(unittest.TestCase):
    def setUp(self):
        self.last = {"id": ORIGINAL_PROTECTED_ARTIFACT,
                     "name": BUILD_NAME, "expired": False,
                     "size_in_bytes": 739449,
                     "created_at": "2026-11-17T15:57:52Z",
                     "workflow_run": {"id": ORIGINAL_PROTECTED_RUN}}
    def test_protects_latest_unified_build(self):
        old = dict(self.last, id=1, created_at="2026-10-01T00:00:00Z")
        self.assertEqual(choose_build([old, self.last])["id"], self.last["id"])
    def test_skips_smoke_and_expired(self):
        smoke = dict(self.last, id=10, name="CrimsonSkiesFix-UNIFIED-SYNTHETIC-WINE-SMOKE")
        expired = dict(self.last, id=20, expired=True)
        self.assertEqual(choose_build([smoke, expired, self.last])["id"], self.last["id"])
    def test_missing_protected_build_blocks_prune(self):
        self.assertIsNone(choose_build([]))
    def test_protected_run_excluded(self):
        runs = [{"id": ORIGINAL_PROTECTED_RUN, "status": "completed"},
                {"id": 2, "status": "completed"}]
        self.assertEqual([r["id"] for r in deletion_targets(runs, {ORIGINAL_PROTECTED_RUN})], [2])
    def test_current_or_active_run_not_deleted(self):
        runs = [{"id": 9, "status": "in_progress"}, {"id": 8, "status": "queued"},
                {"id": 7, "status": "completed"}]
        self.assertEqual(deletion_targets(runs, {7}), [])
    def test_inventory_must_be_complete(self):
        runs = [{"id": 10}, {"id": 11}]
        self.assertFalse(valid_inventory(runs, 3, 10))
        self.assertFalse(valid_inventory(runs, 2, 50))
        self.assertTrue(valid_inventory(runs, 2, 10))
    def test_duplicate_inventory_blocked(self):
        self.assertFalse(valid_inventory([{"id":1},{"id":1}], 2, 1))
    def test_newer_build_wins(self):
        future = dict(self.last, id=99999, created_at="2026-12-10T08:10:00Z")
        self.assertEqual(choose_build([self.last, future])["id"], 99999)

if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    group = parser.add_mutually_exclusive_group()
    group.add_argument("--dry-run", action="store_true")
    group.add_argument("--apply", action="store_true")
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        output = unittest.main(argv=[sys.argv[0]], exit=False).result
        sys.exit(0 if output.wasSuccessful() else 1)
    prune(args.apply)
