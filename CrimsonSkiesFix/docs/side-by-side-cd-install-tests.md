# Safe side-by-side original CD installations for testing

Historical test note: the original USA disc is no longer available to the owner,
so these exact commands are archival examples, **not a remaining release gate**.
The owner attempted `--install-cd "Crimson Skies (USA, Europe).cue"` after
the German installation and supplied `install-cd-USA.log` plus
`verify-install-USA.log`. The installation command **correctly refused**
because `CrimsonSkiesFix/` already existed. The verification command
**succeeded on the existing German files**, reporting
`Edition=C_SKIES_DE_001`. **Neither is evidence of a successful USA install.**

The original anti-overwrite guarantee remains: `CrimsonSkiesFix/` is
the production default, and an existing installation is *never* deleted,
replaced or renamed automatically.

## Optional test-only side-by-side mode

The launcher now supports **explicit edition-specific sibling directories**
without needing to move images, patches, launchers or game saves into
different test workspaces. The user must opt into a named edition.

```sh
WINEDEBUG=-all wine CrimsonSkiesFix.exe --install-cd "Crimson Skies (USA, Europe).cue" "Crimson_Skies_Patch_102.exe" --into USA
cp CrimsonSkiesFix_installer.log install-cd-USA.log

WINEDEBUG=-all wine CrimsonSkiesFix.exe --verify-install USA
# Successful --verify-install reports to the terminal and keeps the previous
# installer log untouched; capture this success explicitly if desired:
# WINEDEBUG=-all wine CrimsonSkiesFix.exe --verify-install USA > verify-install-USA.log 2>&1
```

These commands create/verify `CrimsonSkiesFix.USA/` while leaving
the existing German `CrimsonSkiesFix/` intact. A successful log must
explicitly state `Edition=C_SKIES_DISC_001`, the destination, and
`INSTALL_RESULT=GAME_FILES_READY_EXECUTABLE_PENDING`.
The separate verification, captured from standard output on success, must
explicitly state the **USA edition** and the selected directory. A successful
`--verify-install` intentionally **does not overwrite** an existing
`CrimsonSkiesFix_installer.log`; an error does update it.

Other selectors are `FR` and `DE`. The new install-mode forms are:

```text
CrimsonSkiesFix.exe --install-cd [IMAGE] [PATCH]
CrimsonSkiesFix.exe --install-cd IMAGE PATCH --into USA
CrimsonSkiesFix.exe --install-cd IMAGE PATCH --into FR
CrimsonSkiesFix.exe --install-cd IMAGE PATCH --into DE
CrimsonSkiesFix.exe --verify-install
CrimsonSkiesFix.exe --verify-install USA|FR|DE
```

Safety properties:

- `--into` accepts **only** known selectors; no arbitrary filesystem
  path, traversal or special Windows device names.
- Its destination is deterministically
  `CrimsonSkiesFix.USA/`, `CrimsonSkiesFix.FR/` or
  `CrimsonSkiesFix.DE/`, with independent
  `<directory>.installing/` transactional staging.
- The **entire original media-file manifest** determines the actual edition.
  The specified `--into` edition must match it; a mismatch is refused.
  An interrupted mismatch can leave incomplete staging and no game
  directory is published.
- Existing selected directories and abandoned stage directories are
  refused, never overwritten. A separate `--verify-install ED`
  validates the requested edition against the installed receipt and
  original hashed CD/game assets.
- Ordinary launcher starts and `--verify` still work only on the
  default `CrimsonSkiesFix/` directory. Named installations are
  **testing artifacts**, not multiple simultaneous running game
  configurations.
- These commands **do not** unwrap, repair, replace or run the protected
  original `CRIMSON.EXE`. SafeDisc remains the final stage.

The original owner's error therefore needs no deletion of a good
German installation. With the new build, rerun the USA test using
`--into USA` and then `--verify-install USA`.

Keep PR #26 as a draft until genuine-USA installation results and full
retail-equivalence / approved executable testing are complete.
