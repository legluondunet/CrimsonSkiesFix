# Branch cleanup — preserve only main and open pull-request branches

The GitHub Actions workflow [Cleanup branches - keep main and open PRs](workflows/cleanup-branches.yml)
runs every **Sunday at 03:31 UTC** and is also manually executable.

## Exact policy

**Keep:**
- The repository's main/default branch (`main`).
- Every branch **in this repository** associated with at least one **open pull request**,
  even when the PR targets another base branch.
- Branches covered by GitHub branch protection **cannot be deleted by this
  workflow**. They are visibly reported as blocked; protection is never removed.

**Delete:**
- All other local branches, including branches whose PRs were merged,
  branches whose PRs were closed without merging, branches without any PR,
  and even branches named `release/*`, `maintenance/*`, `hotfix/*` or
  `master` if they are not `main` and have no open PR.

**Warning:** This is more aggressive than a "merged branches only" cleanup.
Unmerged changes on a deleted branch can become difficult to recover. Check
the dry-run list before enabling the first manual deletion. Deleting branch
references does not remove the corresponding commits immediately, but does
remove the convenient branch pointers.

## Manual use

After this PR is merged:
1. Actions → **Cleanup branches - keep main and open PRs** → Run workflow.
2. Keep **dry_run=true** (default) and read the complete list in the job summary.
3. Run again with **dry_run=false** to delete the candidates.
4. The weekly scheduled execution applies the same policy automatically,
   without a minimum age or waiting period.

## Safety and validation

- The policy test job has no permission to delete branches. Its push-trigger
  runs Python unit tests followed by a read-only inventory.
- Before applying any deletion, the script refuses to run from any ref other
  than `refs/heads/main`, verifies the repository default is still `main`,
  and requires an explicit short-lived GitHub token authorization.
- It fetches **all** current local branches and **all** open PRs (paginated),
  and checks each candidate's SHA, protection and open-PR status once more
  immediately before the deletion API call.
- GitHub branch protection takes precedence over the cleanup; these blocked
  branches are listed in the workflow summary.
- Does **not** remove GitHub Actions workflow executions, build artifacts,
  releases, commits, or tags. Artifact storage cleanup is a separate process.
