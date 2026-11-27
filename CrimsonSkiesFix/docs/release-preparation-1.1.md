# 1.1 release preparation — working checklist

Status: PR #71 remains draft. No merge, tag or public release authorized by this document.

## Technical baseline

- [x] Five recognized profiles with real installation and launch evidence across the candidate history.
- [x] Updated diagnostic UX approved by maintainer under Wine, including browser action and a successful German 1.0 fresh install on 3287914a.
- [x] Baseline CI: 45904563284, 341 runtime and 89 installer tests; Windows x86 and Wine32.
- [x] Scope decision: use separately supplied authentic official 1.02 update for three earlier CD editions; stop binary version-conversion research.
- [x] Revised workflow selects the fixed five-edition feature regardless of branch name; CI 45909737451 on 4171e88a succeeded for Windows x86 and Wine32. Test artifacts only, no automatic release.
- [x] Capture and review actual resolved installer and runtime-unified Cargo.lock files from CI 45909737451; both are tracked and build/test now requires --locked. Archive SHA-256 c440d012af97b7912b121b953a05b79a2d5ba5efeb8954365f30e29203f119f9; shared packages have identical versions and checksums.
- [x] Exact-head CI 45911197219 on 09aad277: Windows x86 and Wine32 passed with --locked and unchanged lockfiles; existing German installation smoke under Wine 11.19 succeeded (PR #71 comment 7205914407). Toolchain and runner pinning remain separate from dependency locking.
- [x] Reviewed CI-generated package-license inventory, compiler/runner versions and installed build-info lock hashes on cb3a581f (CI 45913582006; PR comments 7206174205 and 7206194656). 16 external packages in installer target metadata (15 registry + one Git), six shared with runtime; metadata is not evidence that every package is linked.
- [ ] Verify exact-package license text collection, notice embedding and installed notice hash in the next candidate. Review nested notices, toolchain runtime notices and corresponding source separately.
- [ ] Review pinned unsafedisk commit a869653e85f3786fdb82f9f9856d7b14bd25d585, currently draft PR #7 stacked on #5; ensure durable upstream reference before release. Its manifest declares GPL-3.0-or-later and no direct Rust dependencies. Do not merge or alter upstream blindly; no version change in this preparation.
- [ ] Complete dependency license/notice and corresponding-source review; verify actual resolved dependency list and required notices before distributing the linked installer. Separate review of applicable rights and anti-circumvention law is still necessary; no legal authorization inferred from open-source licensing or the owner's media. No redistribution of the Microsoft updater by this project without appropriate permission.
- [ ] Ensure version identity consistency (launcher, runtime, logs, signature and release tag), update the public README at publication, and independently verify final single-EXE package SHA-256. Stable 1.0 stays published until explicit release decision.
- [ ] Owner review of remaining native Windows fresh-media combinations / final candidate smoke as appropriate. Do not claim a complete platform matrix.
- [ ] Explicit approval for merge and public release; do not enable private embedded patch feature in public builds.

## Current design decisions

The consolidated workflow builds the five-edition candidate by explicit feature rather than branch name, and still publishes experimental CI artifacts only. Historical workflow remains manual-only. The main README is the authoritative 1.1 development guide; the inner README's old 1.0 instructions remain labeled historical. The draft release notes are separate from the historical 1.0 draft.
