#!/usr/bin/env python3
"""Fail closed if the compiled CSFIX.dll gains WinMM/DDraw/core loader imports.

No third-party Python packages required. Works on PE32 disk images. This gate
checks only static and delay dependencies / exported names; behavioral tests
remain mandatory on native Windows and Wine.
"""
from __future__ import annotations
import pathlib
import struct
import sys


class PeError(ValueError):
    pass


def u16(data: bytes, offset: int) -> int:
    if offset < 0 or offset + 2 > len(data):
        raise PeError(f"truncated PE u16 at {offset:#x}")
    return struct.unpack_from('<H', data, offset)[0]


def u32(data: bytes, offset: int) -> int:
    if offset < 0 or offset + 4 > len(data):
        raise PeError(f"truncated PE u32 at {offset:#x}")
    return struct.unpack_from('<I', data, offset)[0]


def inspect(data: bytes) -> tuple[list[str], list[str]]:
    if data[:2] != b'MZ':
        raise PeError('missing DOS MZ signature')
    pe = u32(data, 0x3c)
    if data[pe:pe + 4] != b'PE\0\0' or u16(data, pe + 4) != 0x14c:
        raise PeError('expected Windows PE32 i386')
    section_count = u16(data, pe + 6)
    optional_size = u16(data, pe + 20)
    optional = pe + 24
    if not 1 <= section_count <= 96 or optional_size < 208 or u16(data, optional) != 0x10b:
        raise PeError('invalid or truncated PE32 optional header')
    if optional + optional_size > len(data):
        raise PeError('truncated optional header')
    size_of_headers = u32(data, optional + 60)
    sections = []
    for i in range(section_count):
        s = optional + optional_size + i * 40
        sections.append((u32(data, s + 12), u32(data, s + 16), u32(data, s + 20)))

    def rva_to_file(rva: int, length: int) -> int:
        if not rva or length < 0:
            raise PeError(f'invalid RVA/length {rva:#x}/{length}')
        for virtual, raw_size, pointer in sections:
            if virtual <= rva and rva - virtual + length <= raw_size:
                offset = pointer + rva - virtual
                if offset + length <= len(data):
                    return offset
        if rva + length <= size_of_headers and rva + length <= len(data):
            return rva
        raise PeError(f'unmapped RVA {rva:#x} ({length} bytes)')

    def text_at(rva: int) -> str:
        off = rva_to_file(rva, 1)
        end = data.find(b'\0', off, min(len(data), off + 261))
        if end < 0:
            raise PeError(f'unterminated imported/exported name at RVA {rva:#x}')
        return data[off:end].decode('ascii', 'strict')

    def directory(index: int) -> tuple[int, int]:
        if u32(data, optional + 92) <= index:
            return (0, 0)
        return u32(data, optional + 96 + index * 8), u32(data, optional + 100 + index * 8)

    if directory(13) != (0, 0):
        raise PeError('CSFIX.dll unexpectedly contains delay imports')

    imports = []
    import_rva, import_size = directory(1)
    if not import_rva or import_size < 20 or import_size > 0x10000:
        raise PeError('missing or excessive import descriptor directory')
    terminated = False
    for i in range(min(1024, import_size // 20)):
        p = rva_to_file(import_rva + 20 * i, 20)
        if data[p:p + 20] == b'\0' * 20:
            terminated = True
            break
        imports.append(text_at(u32(data, p + 12)))
    if not terminated:
        raise PeError('unterminated import descriptors')

    export_rva, export_size = directory(0)
    if not export_rva or export_size < 40:
        raise PeError('CSFIX.dll has no export directory')
    exp = rva_to_file(export_rva, 40)
    count = u32(data, exp + 24)
    names_rva = u32(data, exp + 32)
    if count > 1024 or count == 0:
        raise PeError('invalid alias export count')
    exports = [text_at(u32(data, rva_to_file(names_rva + i * 4, 4))) for i in range(count)]
    return imports, exports


def verify(data: bytes, *, unified: bool = False) -> tuple[list[str], list[str]]:
    imported, exported = inspect(data)
    forbidden = {'winmm.dll', 'ddraw.dll', 'csfix.dll', 'crimsonskiesfix.dll'}
    if unified:
        # One project DLL; SDL3 remains optional and must be loaded dynamically.
        forbidden.update({'fix.dll', 'sdl3.dll'})
    bad = [s for s in imported if s.casefold() in forbidden]
    if bad:
        raise PeError(f'CSFIX.dll reintroduced a forbidden static provider/import: {bad}')
    if not any(s.casefold() == 'kernel32.dll' for s in imported):
        raise PeError('unexpected import table: kernel32.dll missing')
    needed = {'DirectDrawCreate', 'DirectDrawEnumerateA'}
    if not needed.issubset(set(exported)):
        raise PeError(f'missing DirectDraw alias exports: {sorted(needed - set(exported))}')
    multimedia_exports = [s for s in exported if s.lower().startswith(('mci', 'mixer', 'wave', 'midi', 'time', 'joy', 'aux'))]
    if multimedia_exports:
        raise PeError(f'CSFIX.dll unexpectedly exports WinMM-like names: {multimedia_exports}')
    if unified:
        exact = needed | {'DllMain'}
        if len(exported) != len(exact) or set(exported) != exact:
            raise PeError(f'unified fix.dll must export exactly {sorted(exact)}; got {exported}')
    return imported, exported


def main(argv: list[str]) -> int:
    unified = len(argv) == 3 and argv[1] == '--unified'
    if not (len(argv) == 2 or unified):
        print('Usage: verify_alias_pe.py [--unified] path/to/alias.dll', file=sys.stderr)
        return 2
    try:
        imported, exported = verify(pathlib.Path(argv[-1]).read_bytes(), unified=unified)
    except (OSError, PeError, UnicodeDecodeError) as exc:
        print(f'ALIAS-PE-GATE FAIL: {exc}', file=sys.stderr)
        return 1
    print(f'ALIAS-PE-GATE PASS: PE32; imports={imported}; exports={exported};'
          ' no app-local WinMM, DDRAW, core imports or delay imports')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv))
