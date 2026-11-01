# Crimson Skies v1.02 — Video Preferences research, verified binaries

**Status:** investigation only. No graphics-menu widgets were modified.
The working `CrimsonSkiesFix v0.1.69` MainMenu version watermark and the
FOV/HUD fix are preserved; PR #9 remains a draft until in-game testing.

## Owner-supplied archive: exact identification

| Source | Size | SHA-256 |
|---|---:|---|
| `ZBD/zrdr.zbd` | 3,988,394 bytes | `76b510d821edd2268040d2ccb18c462ec07ad580cdba571b3066228e2cf592dd` |
| `CRIMSON.EXE` supported SafeDisc-free 1.02 | 2,580,480 bytes | `43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75` |

The ZBD archive is **not** a simple zip or a set of Win32 dialog
resources: it is serialized Zipper reader data. Direct byte analysis
shows tagged string/value records and a large amount of asset/dialog
configuration.

Case-insensitive searches of the **entire binary** (both ASCII and
UTF-16LE) produced:

| Text searched | ASCII matches | UTF-16LE matches |
|---|---:|---:|
| `graphics` | 0 | 0 |
| `resolution` | 0 | 0 |
| `preferences` | 10 | 0 |
| `video` | 1 | 0 |
| `display` | 0 | 0 |
| `device` | 0 | 0 |
| `MainMenu` | 3 | 0 |

Examples of **actual** in-file material:
- `0x363A2` (decimal 222,114) — `MSG_BTN_PREFERENCES`
  (escape/configure dialog button resources, repeated).
- `0xA633D` (decimal 680,765) — `MM_B_Preferences2`
  followed by `MSG_BTN_PREFERENCES`, `MM_B_Preferences3`,
  `MM_B_Preferences4`, and `MAINMENU_BTN`.
- `0x6B62F` (decimal 439,855) — `video` as an `IMAGE_PATH`
  directory for an *intro*, not an option-widget name.

These facts establish that `zrdr.zbd` contains various dialog/menu
resources. They **do not establish** the custom `Preferences > Video`
list object definitions, click handlers or their enabled-state flags;
the UI may be code-driven, differently named, or backed by resources
elsewhere. Avoid pretending this file proves they are selectable via a
particular UI API.

## Stronger verified EXE leads

Cross-checked with the supported exact SHA-256 EXE and Win32 x86
disassembly (addresses below are absolute VAs for the preferred
`0x00400000` image base).

| VA | Original bytes | Observation |
|---|---|---|
| `0x004021A6` | `68 90 B4 64 00` | Pushes `0x0064B490` for registration of the `nSelectedVideoDevice` GOS script variable |
| `0x00418E82` | `A3 90 B4 64 00` | Initializes its backing value |
| `0x00419208` | `A1 90 B4 64 00` | Reads selected graphics device while applying settings |
| `0x00419235` | `8B 15 8C B4 64 00` | Reads selected **mode index** `0x0064B48C` |
| `0x00407811` | `E8 0A 1C 01 00` | Dispatches a selection event to `0x00419420` |
| `0x00419420` | `8B 44 24 04 50 E8 86 FB FF FF` | Starts mode-selection handler, which normalizes the requested mode through `0x00418FB0` |
| `0x0041942D` | `A3 8C B4 64 00` | Stores the normalized mode index, ending in `ret 4` |
| `0x004077EF` | `E8 6C 19 01 00` | Calls `0x00419160` when preferences are applied |

The original 800x600 display is a **logical game menu mode**, not
the desktop resolution. `0x0064B48C` indexes a game-owned table at
`0x0064B49C`, populated from the supported video modes. For example,
`0x0041923D` loads a 16-bit width from that table. Merely overwriting
this index with `1920` or the screen height would corrupt the
table lookup. Likewise, replacing physical DirectDraw dimensions does
**not** replace the text label displayed by a custom menu dropdown.

## Conditions for a safe final implementation

1. Identify both native GUI widget instances and exact menu callback
   owners from **runtime** Preferences > Video interaction traces.
   For the mode list, the verified `0x00407811` call is a candidate
   event boundary. `nSelectedVideoDevice` is registered as a
   writable GOS script variable and needs an independent device-list
   event/commit boundary.
2. Establish exact disabled/ignore-input behavior **without** disabling
   brightness/detail/shadows or globally intercepting mouse/keyboard.
3. Replace the **visible** resolution label with the current monitor
   geometry (for example `1920 × 1080`), **without** modifying a
   legacy mode-table index or changing the internal 4:3 menu surfaces.
4. Require the authenticated EXE hash and strict original preimages,
   preserve the original code on hook failure and restore page protections.
5. Verify live under Wine and Windows, including a secondary monitor,
   4:3 video presentation, and returning from gameplay.

The current v0.1.70 probe already logs (read-only) changes to
`nSelectedVideoDevice` and `0x0064B48C` while the Preferences
script is active. A log from opening/cycling each separate dropdown
will distinguish GOS-backed script events from real selection changes.

**Do not merge this unfinished Video Preferences functionality.**
No proprietary game resources or EXE bytes should be committed.

legluondunet assisted by ChatGPT (OpenAI)
