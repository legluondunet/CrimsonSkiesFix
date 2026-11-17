# GitHub Actions — safe branch cleanup

Workflow: [`.github/workflows/prune-merged-branches.yml`](workflows/prune-merged-branches.yml).

## Policy

Runs **every Sunday at 03:31 UTC** (GitHub schedule is approximate).
It removes local branches only if **all** the following checks pass:

- The branch belongs to a pull request **merged into `main`** at least **7 full days** ago.
- The latest PR on that branch is merged, **no PR on the branch is open**, and the current branch HEAD SHA is **exactly** the merged PR's head SHA.
- The branch is not protected and is not `main`, `master`, `develop`, `gh-pages`, or any `maintenance/`, `release/`, `hotfix/`, `archive/` branch.
- Immediately before deletion, the workflow re-fetches the branch and all pull requests associated with it to reject stale data or new work.

**Closed-but-unmerged PR branches are NEVER automatically deleted.**
Branches without PR history are also preserved. Deletions cannot be undone
through the GitHub UI; the commit SHA remains available in historical PRs
but the branch reference is removed.

## Manual dry run

After this workflow has been merged into the default branch:

1. Open **Actions → Cleanup merged PR branches (safe) → Run workflow**.
2. Leave `dry_run=true` (the default). Set `min_age_days` to 7 or another
   value from 0 to 365.
3. Read the workflow's **Job summary**. It lists exactly which branches
   would be deleted and how many are being preserved.
4. If the list is correct, run again with **`dry_run=false`**.
   This option deletes the matching branches, subject to the final safety
   recheck. To disable the automatic weekly purge, remove the `schedule`
   trigger in the workflow.

## CI and safety

- Branch push tests run seven offline policy tests plus a **read-only API
  dry run**. They have **read-only repository permissions** and cannot
  delete anything.
- Scheduled and manually applied jobs use a short-lived GitHub Actions
  token with `contents:write`; no PAT or secret is needed.
- Maintenance and release branches are excluded, including any temporary
  artifact-purge workflow branch.
- This workflow **does not delete GitHub Actions runs or artifacts**.
  Branch deletion is organizational and does not itself reclaim artifact
  storage. Use the separate artifact cleanup for that purpose.

Current repo context (17 Nov 2026, before this workflow): 43 branches,
including 33 heads of merged PRs, six heads of closed-but-unmerged PRs,
two active PR branches (#49, #51), `main` and the temporary artifact
cleanup branch. Counts are just an initial inventory; the workflow always
re-queries GitHub before taking action.
