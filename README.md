# CrimsonSkiesFix

Rust research and compatibility improvements for **Crimson Skies
(2000)** on modern Windows and Wine/Proton.

Two **independent** projects live in this repository.

## [SafeDisc](SafeDisc/)

Includes the original Python v0.9 Crimson Skies SafeDisc unwrapper
in `SafeDisc/python/unwrapper/` and restored offline SafeDisc/Wine
Python research scripts in `SafeDisc/scripts/`. See
[SafeDisc documentation](SafeDisc/README.md).

Native Rust SafeDisc research and an experimental Windows EXE / Wine
compatibility patcher. Includes a read-only native unSafeDisc 1.x
analysis tool, source code, historical research, and documentation.

```sh
cargo test --manifest-path SafeDisc/Cargo.toml
cargo build --release --manifest-path SafeDisc/Cargo.toml
```

All SafeDisc research is in `SafeDisc/` (source in `SafeDisc/src`,
notes in `SafeDisc/docs`). This work remains independent of the
DirectDraw graphics DLL.

## [CrimsonSkiesFix](CrimsonSkiesFix/)

Experimental x86 DirectDraw proxy DLL named **`ddraw.dll`**.
It supports privacy-filtered diagnostics, optional modern DirectDraw
primary-`Flip` FPS pacing, and ongoing legacy Direct3D3/windowed
presentation development.

```sh
cd CrimsonSkiesFix
cargo test --target i686-pc-windows-msvc
cargo build --release --target i686-pc-windows-msvc
```

**Installed game layout:**

```text
Crimson Skies/
  CRIMSON.EXE
  ddraw.dll
  CrimsonSkiesFix.log                # only with Log=1; beside ddraw.dll
  CrimsonSkiesFix/
    CrimsonSkiesFix.ini              # persistent personal settings
    CrimsonSkiesFix.ini.example      # only this template is packaged
    README.md
```

`ddraw.dll` stays beside `CRIMSON.EXE` for normal proxy loading.
The log is placed beside the **loaded DLL**, while configuration
and instructions are in the `CrimsonSkiesFix/` subfolder.

The prior `CrimsonFix/CrimsonFix.ini` (or older root
`CrimsonFix.ini`) is automatically copied to the new location at
first launch when needed, **without overwriting or deleting it**.
Old logs are not altered.

See [DLL installation and troubleshooting](CrimsonSkiesFix/README.md).
The exclusive DirectDraw4/Direct3D3 borderless renderer remains
experimental and is not yet enabled in-game.

## License

GPL-3.0-or-later — [LICENSE](LICENSE). Third-party attributions for
the patcher are in [SafeDisc/THIRD_PARTY_NOTICES.md](SafeDisc/THIRD_PARTY_NOTICES.md).
