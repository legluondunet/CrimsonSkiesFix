#!/usr/bin/env python3
"""Patch resolution and widescreen FOV in the two supplied Crimson Skies EXEs.

Only recognizes the precise widescreen-modified executable identified by its
SHA-256 after normalizing the four adjustable values. No third-party packages.
"""

import argparse
import hashlib
import math
from pathlib import Path
import struct
import sys

# File offsets in the two supplied widescreen-patched Crimson Skies binaries.
WIDTH_OFF = 0x20254B
HEIGHT_OFF = 0x20254F
FOV60_OFF = 0x20409C
FOV80_OFF = 0x2040A0

# The two source files differ solely in HEIGHT_OFF. Their normalized hashes match.
NORMALIZED_SHA256 = "5578a15ce072d09655be43419829a7530ed6c71fa5a8778fd9d4661770b39d67"
BASE60_BYTES = bytes.fromhex("920a863f")  # 60 degrees, radians, float32
BASE80_BYTES = bytes.fromhex("c2b8b23f")  # 80 degrees, radians, float32
REF_ASPECT = 4 / 3


def check_binary(data: bytes) -> None:
    if len(data) <= FOV80_OFF + 4 or data[:2] != b"MZ":
        raise ValueError("Fichier absent, tronqué ou non exécutable Windows (PE).")
    normalized = bytearray(data)
    struct.pack_into("<II", normalized, WIDTH_OFF, 1920, 1080)
    normalized[FOV60_OFF:FOV60_OFF + 4] = BASE60_BYTES
    normalized[FOV80_OFF:FOV80_OFF + 4] = BASE80_BYTES
    if hashlib.sha256(normalized).hexdigest() != NORMALIZED_SHA256:
        raise ValueError(
            "Version non reconnue : le patcher accepte uniquement les deux "
            "exécutables widescreen transmis (ainsi que leurs versions repatchées). "
            "Aucun fichier n'a été modifié."
        )


def horizontal_fov(base_degrees: float, width: int, height: int) -> float:
    factor = (width / height) / REF_ASPECT
    return math.degrees(2 * math.atan(math.tan(math.radians(base_degrees) / 2) * factor))


def show_current(data: bytes, width: int, height: int, prefix: str) -> None:
    base60_rad = struct.unpack_from("<f", data, FOV60_OFF)[0]
    base80_rad = struct.unpack_from("<f", data, FOV80_OFF)[0]
    base60 = math.degrees(base60_rad)
    base80 = math.degrees(base80_rad)
    print(f"{prefix} : {width} x {height} ({width / height:.5f}:1)")
    print(f"  FOV référence A : {base80:.3f}° ; FOV horizontal calculé : {horizontal_fov(base80, width, height):.3f}°")
    print(f"  FOV référence B : {base60:.3f}° ; FOV horizontal calculé : {horizontal_fov(base60, width, height):.3f}°")


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Modifier la résolution et le FOV d'un Crimson Skies déjà patché widescreen."
    )
    parser.add_argument("exe", type=Path, help="EXE original parmi les deux versions transmises")
    parser.add_argument("--width", "-W", type=int, help="Nouvelle largeur en pixels")
    parser.add_argument("--height", "-H", type=int, help="Nouvelle hauteur en pixels")
    fov_group = parser.add_mutually_exclusive_group()
    fov_group.add_argument(
        "--fov", type=float, metavar="DEGRES",
        help="FOV horizontal souhaité (en degrés) pour le mode de référence 80°"
    )
    fov_group.add_argument(
        "--reset-fov", action="store_true", help="Rétablit les deux angles de référence à 80° et 60°"
    )
    parser.add_argument("--info", action="store_true", help="Afficher les paramètres actuels, sans modifier")
    parser.add_argument("--output", "-o", type=Path, help="Chemin du nouveau fichier EXE")
    args = parser.parse_args()

    try:
        raw = args.exe.read_bytes()
        check_binary(raw)
        data = bytearray(raw)

        current_width, current_height = struct.unpack_from("<II", data, WIDTH_OFF)
        if not (320 <= current_width <= 16384 and 240 <= current_height <= 16384):
            raise ValueError("Résolution actuelle incohérente.")
        show_current(data, current_width, current_height, "Actuel")

        if args.info:
            return 0

        if all(item is None for item in (args.width, args.height, args.fov)) and not args.reset_fov:
            parser.error("Indiquer --width, --height, --fov, --reset-fov ou --info.")

        width = args.width if args.width is not None else current_width
        height = args.height if args.height is not None else current_height
        if not (320 <= width <= 16384 and 240 <= height <= 16384):
            raise ValueError("Dimensions acceptées : largeur 320–16384, hauteur 240–16384.")
        if not (0.5 <= width / height <= 4.0):
            raise ValueError("Rapport d'image hors de la plage 0,5 à 4.")

        struct.pack_into("<II", data, WIDTH_OFF, width, height)

        if args.reset_fov:
            data[FOV80_OFF:FOV80_OFF + 4] = BASE80_BYTES
            data[FOV60_OFF:FOV60_OFF + 4] = BASE60_BYTES
        elif args.fov is not None:
            if not math.isfinite(args.fov) or not (10.0 <= args.fov <= 160.0):
                raise ValueError("Le FOV horizontal doit être compris entre 10° et 160°.")
            aspect_factor = (width / height) / REF_ASPECT
            # Choose one tangent multiplier for BOTH original FOV modes so the
            # 80° branch reaches the requested horizontal FOV at this aspect.
            multiplier = math.tan(math.radians(args.fov) / 2) / (
                math.tan(math.radians(80) / 2) * aspect_factor
            )
            new80 = 2 * math.atan(math.tan(math.radians(80) / 2) * multiplier)
            new60 = 2 * math.atan(math.tan(math.radians(60) / 2) * multiplier)
            struct.pack_into("<f", data, FOV80_OFF, new80)
            struct.pack_into("<f", data, FOV60_OFF, new60)

        # Recheck the normalized SHA of the output to catch coding accidents.
        check_binary(data)
        show_current(data, width, height, "Nouveau")

        if args.output is None:
            fov_suffix = (f"_FOV{args.fov:g}" if args.fov is not None else "")
            output = args.exe.with_name(f"CRIMSON_{width}x{height}{fov_suffix}.exe")
        else:
            output = args.output
        if output.resolve() == args.exe.resolve():
            raise ValueError("La sortie doit être différente de l'original.")
        # Exclusive create: do not silently overwrite a previous test.
        with output.open("xb") as file:
            file.write(data)
        print(f"\nPatch généré : {output}")
        print(f"SHA-256 : {hashlib.sha256(data).hexdigest()}")
        print("L'exécutable n'a pas été lancé ; compatibilité à confirmer dans le jeu.")
        return 0
    except (OSError, ValueError, struct.error) as exc:
        print(f"Erreur : {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())