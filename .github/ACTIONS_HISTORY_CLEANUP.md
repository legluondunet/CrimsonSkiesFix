# GitHub Actions run-history cleanup

The [Prune old GitHub Actions runs (preserve latest build)](workflows/prune-action-runs.yml)
workflow deletes **completed GitHub Actions workflow executions**, not merely
the ZIP build artifacts attached to them.

## Preservation and scope

- Always preserve the workflow run containing the **newest unexpired**
  `CrimsonSkiesFix-UNIFIED-X86-TEST-ONLY` artifact. It must be a nonempty ZIP
  artifact with a valid workflow-run association.
- Preserve the currently executing cleanup workflow run (which cannot
  safely delete itself).
- Leave any other queued or running workflow jobs untouched until they are
  completed. They can be cleaned at the next scheduled pass.
- Delete all other **completed** workflow runs (successful, failed, canceled,
  skipped), including their old logs and associated archives.
- Do not change repository source code, `main`, branches, pull requests,
  tags, issues, releases or the preserved build ZIP.

On 17 Nov 2026 the explicitly protected Windows x86 build was artifact
`14343959078` in run `44119944628`, from commit `be4e88da`.
The initial cleanup will abort before deletion if this artifact/run pair
is no longer the newest. Later weekly runs select the newest by actual artifact
creation date, so new builds replace the old one as the protected result.

## Execution

- A one-time push to the dedicated maintenance branch initiates the initial
  purge after the policy's unit tests succeed.
- On `main`, the workflow schedules a purge every Sunday at **04:53 UTC**.
- Use **Actions → Prune old GitHub Actions runs → Run workflow** to inspect
  a `dry_run=true` (default) inventory, or `dry_run=false` to apply a
  cleanup manually.
- The deletion job has the minimum required `actions:write` token permission.
- The script inventories all runs before deleting, refuses partial/duplicated
  inventories, handles GitHub rate limits, and verifies the protected artifact
  and run again at completion. Progress and exact deletion counts are printed
  in the workflow logs and Job Summary.

**Caution:** deleting workflow runs removes their CI logs and checks from
GitHub Actions history. New builds are unaffected, and each workflow's YAML
file remains in the repository. If there are no valid unified x86 build
artifacts, the job stops instead of deleting the last known working
compilation.
