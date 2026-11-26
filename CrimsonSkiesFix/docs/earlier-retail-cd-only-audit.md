# CD-only installation: earlier retail editions (26 November 2026)

Research baseline: validated candidate 771b566e, unsafedisk a869653e; issue #46. No new runtime or executable support in this audit.

| Profile | Protected ICD bytes | 797-byte official patch asset on CD | Currently validated route |
|---|---:|---|---|
| C_SKIES_DISC_001 | 2560095 | absent | original CD + authentic local 1.02 updater |
| C_SKIES_DE_001 | 2560095 | absent | original CD + authentic local 1.02 updater |
| C_SKIES_FR_002 | 2560095 | absent | original CD + authentic local 1.02 updater |
| C_SKIES_FR_001 | 2580578 | present | original CD alone |
| C_SKIES_DISC_002 | 2580578 | present | original CD alone |

Source: exact five inventories in installer/profiles. English DISC_001/DISC_002 share identical CRIMSON.ROF, LANGUAGE.DLL and STRINGS.DLL. FR_001/FR_002 also share those three corresponding localized assets. This does not imply matching executable code: the ICD hashes differ and all three earlier profiles lack CRIMPTCH.ROF.

The official updater contains complete protected 1.02 EXE/ICD inputs and the 797-byte CRIMPTCH.ROF (update102.rs EXPECTED_MEMBERS), not a demonstrated 1.00-to-1.02 transformation. No verified algorithm deriving these missing 1.02 contents solely from earlier media is available. The runtime and installer independently accept only the existing exact 2580480-byte 1.02 executable (exe_gate.rs and installer/src/main.rs). Widescreen and menu hooks are version-specific. The current GOS overlay verifier requires the official asset or its authenticated managed derivative. Do not remove any of these gates.

Unsafedisk draft PR #6 reports a separately tested native 1.00 reconstruction for the US/DE inputs, with a different output size and hash. This is not a 1.02 result and does not establish support for FR_002 or compatibility with this runtime. A 1.00 compatibility port, if explicitly chosen, needs a distinct executable profile, independent hook and asset validation, version-specific regressions, and legal review. It changes the currently agreed 1.02-only product requirement; do not silently substitute it for issue #46.

Possible paths under current constraints:
- Continue accepting the authentic locally supplied official update for three earlier editions.
- Seek permission from the rights holder to distribute the official update; only enable existing private embedding route in public builds after that permission and review.
- Investigate a genuinely independent, legally reviewed upgrade implementation without copying proprietary 1.02 code or distributing game data. No demonstrated exact upgrade currently exists. Do not introduce a binary delta or bundle an executable as a shortcut.

Keep the five tested media paths and the exact SHA gates intact. No additional owner file/test needed for this audit. No merge or release authorization inferred.
