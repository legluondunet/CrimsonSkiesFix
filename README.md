# CrimsonSkiesFix

Experimental **Windows x86 DirectDraw 3/4 compatibility proxy** for *Crimson Skies (2000)* on modern Windows and Wine/Proton.

This repository now focuses exclusively on the **`ddraw.dll` graphics compatibility project**.

The historical SafeDisc/unSafeDisc work (native Rust analysis, Python v0.8/v0.9 unwrapper, legacy compatibility patcher, research archives, documentation and dedicated CI) has moved to the independent repository: **[legluondunet/unsafedisk](https://github.com/legluondunet/unsafedisk)**, currently proposed for review in [unsafedisk PR #1](https://github.com/legluondunet/unsafedisk/pull/1).

## Build the DirectDraw DLL

```sh
cd CrimsonSkiesFix
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

GitHub Actions: `.github/workflows/crimsonskiesfix-x86.yml`. It compiles **only** `CrimsonSkiesFix/**`.

## Installed game layout

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonSkiesFix.log                # only if [Debug] Log=1; beside ddraw.dll
  CrimsonSkiesFix/
    CrimsonSkiesFix.ini              # persistent personal settings
    CrimsonSkiesFix.ini.example      # only the template is packaged
    README.md
```

Place `ddraw.dll` beside `CRIMSON.EXE` for proxy loading. Logging is disabled by default. The log, if enabled, is written next to the loaded DLL; configuration remains in `CrimsonSkiesFix/`.

Older `CrimsonFix/CrimsonFix.ini` and root `CrimsonFix.ini` settings can be migrated automatically at first launch **without overwriting or deleting the originals**. Old logs are unchanged.

Full installation, troubleshooting and limitations: [DLL README](CrimsonSkiesFix/README.md). This is an experimental compatibility layer: true borderless presentation and correct in-game graphics/performance still require gameplay verification.

## License

GPL-3.0-or-later — [LICENSE](LICENSE). Original SafeDisc research and its third-party notices are retained in [unsafedisk](https://github.com/legluondunet/unsafedisk).
