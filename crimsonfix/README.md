# CrimsonFix 0.1.0 — experimental DirectDraw proxy

**Status:** first x86 proof of concept. Not yet gameplay-tested. The
main Rust patcher and the native SafeDisc research CLI are unchanged.

## Why ddraw.dll?

The owner-verified unwrapped Crimson Skies 1.0.2.0 imports exactly
two named functions from DDRAW.dll: DirectDrawCreate and
DirectDrawEnumerateA. The proxy forwards both calls to the **real**
System32 ddraw.dll. It also forwards DirectDrawCreateEx, but does not
yet intercept that newer interface. Initialization is deferred from
DllMain.

## How to test

Use a **copy** of the complete game folder with the already unwrapped
CRIMSON.EXE. Extract the Windows i686 GitHub Actions artifact. Place
ddraw.dll and CrimsonFix.ini **beside CRIMSON.EXE**. Do not replace
any DLL in the Windows or Wine system directories.

Windows and Wine use **the same 32-bit Windows ddraw.dll**, not a
native Linux .so. Under Wine use the usual native/builtin override
for ddraw if needed, e.g. WINEDLLOVERRIDES="ddraw=n,b".

The DLL writes CrimsonFix.log in the game folder when DirectDraw
initializes. If the game launches but fails to draw, try
Borderless=0 and send the log.

## Configuration

[Graphics]

- Borderless=1 — experimental replacement of DirectDraw exclusive
  cooperation by DDSCL_NORMAL, and removes window decorations.
- DesktopResolution=1 — with Borderless=1, keeps the system desktop
  display mode unchanged. The screen is the one containing the mouse
  cursor when SetCooperativeLevel is called; its actual pixel bounds
  are used, including non-primary monitor coordinates.
- FPSLimit=60 — configurable 1..60, always clamped at 60 maximum.
  The cap currently hooks surface Flip and primary surface Blt.
  Paths not using those calls are **not yet guaranteed** to be capped.

**Important limitations:** Old games can require exclusive flip
chains that do not work in DDSCL_NORMAL. DirectDraw QueryInterface to
later IDirectDraw interfaces is not yet intercepted. This prototype
does not yet adapt the internal engine viewport, render target sizes,
or FOV to the desktop resolution. It is an engineering probe that
needs a Wine gameplay test, **not a fully validated borderless patch**.

Build locally with rustup target add i686-pc-windows-msvc and
cargo build --release --target i686-pc-windows-msvc in this folder.

## Repo policy

The work is on an isolated PR branch. No original game EXE, ICD,
memory capture or copyrighted DLL is included. Do not merge before
the owner validates game behavior.
