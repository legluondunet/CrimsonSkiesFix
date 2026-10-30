"""Exact-hash-gated historical-output cosmetics, NOT part of decryption.

This module intentionally separates legacy output formatting from
cryptographic F1/F2 -> TEA decryption. It does not take a reference EXE
as input. Two fixed legacy style values (ASCII timestamp and module-name
case) are observed in historical output, and not yet individually
recovered from unSafeDisc 1.5.5 disassembly.

Copyright (C) 2026 legluondunet
SPDX-License-Identifier: GPL-3.0-or-later
"""

import struct

# Historical author text also independently described on public period sites;
# it is not original Crimson Skies executable content.
LEGACY_SIGNATURE = b' unwrapped with    unSafedisc   - csir.cjb.net -'
LEGACY_TIMESTAMP = b'1911'
HISTORICAL_SHA256 = '43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75'


def _require_zero(blob, start, n, reason):
    if blob[start:start+n] != bytes(n):
        raise ValueError(f'{reason} unexpectedly contains nonzero bytes')


def apply_historical_output_profile(blob, loader_size, sections, nt):
    """Return a copy with exact-title unSafeDisc 1.5.5 output formatting.

    Does not inspect the historical plaintext file. All source-dependent
    fill bytes are computed from loader size, reconstructed PE size, or
    recovered descriptor TimeDateStamp; legacy stamp/case are pinned
    style observations, not a general unSafeDisc output rule.
    """
    result = bytearray(blob)
    if len(result) != 0x276000 or nt != 0xF8 or loader_size != 344_851:
        raise ValueError('historical output style only supports exact Crimson 1.0.2.0')
    if len(LEGACY_SIGNATURE) != 48:
        raise ValueError('unexpected historical signature length')

    # Observed 0xAD2B0000 duplicated in import descriptors and in the
    # DWORD immediately preceding PE signature. Obtain it from *our
    # reconstructed descriptor*, not from any plaintext-reference file.
    idata = sections['.idata']
    descriptor_offset = idata[3]
    descriptor_stamp = result[descriptor_offset+4:descriptor_offset+8]
    if descriptor_stamp != bytes.fromhex('00002bad'):
        raise ValueError('restored import timestamp anchor mismatch')
    _require_zero(result, nt-4, 4, 'pre-PE timestamp mirror')
    result[nt-4:nt] = descriptor_stamp

    # Historical signature/metadata, unrelated to decrypted game code.
    source_timestamp = result[nt+8:nt+12]
    if source_timestamp != bytes.fromhex('dedb153a'):
        raise ValueError('original PE timestamp anchor mismatch')
    result[nt+8:nt+12] = LEGACY_TIMESTAMP
    # Ensure literal is strictly within section-table-to-first-raw gap.
    sections_end = nt+24+0xE0+len(sections)*40
    first_raw = min(s[3] for s in sections.values())
    if not (sections_end <= 0x300 < 0x300+48 <= first_raw):
        raise ValueError('author tag must be in PE header slack')
    _require_zero(result, 0x300, 48, 'historical author tag gap')
    result[0x300:0x330] = LEGACY_SIGNATURE

    # The source module uses KERNEL32.dll, but historical unSafeDisc output
    # contains KeRNeL32.dll. This casing is based on a pinned observation,
    # not on the importer arithmetic. Both spellings resolve the same DLL.
    # Discover its name by walking the canonical import descriptors,
    # rather than writing a magic file offset into the rebuilt image.
    original_name = b'KERNEL32.dll\x00'
    restored_name = b'KeRNeL32.dll\x00'
    _,idata_rva,_,idata_raw = idata
    first = result[idata_raw:idata_raw+idata[2]]
    found = []
    for did in range(21):
        name_va = struct.unpack_from('<I', first, did*20+12)[0]
        name_off = idata_raw+name_va-idata_rva # IMAGE_IMPORT_DESCRIPTOR.Name is an RVA
        if result[name_off:name_off+len(original_name)] == original_name:
            found.append(name_off)
    if len(found) != 1:
        raise ValueError(f'expected one KERNEL32.dll import descriptor, found {len(found)}')
    result[found[0]:found[0]+len(restored_name)] = restored_name

    # The 5 remaining raw-section padding bytes happen to encode input
    # loader size and output file size as 24-bit big-endian numbers.
    # This is a *measured exact-title relationship*, not proof of a
    # generic old unSafeDisc padding algorithm.
    text_vs,_,text_raw_len,text_raw_off = sections['.text']
    reloc_vs,_,reloc_raw_len,reloc_raw_off = sections['.reloc']
    for section_label, end, versus, encoded in (
        ('.text',text_raw_off+text_raw_len,text_vs,loader_size.to_bytes(3,'big')),
        ('.reloc',reloc_raw_off+reloc_raw_len,reloc_vs,len(result).to_bytes(3,'big')),
    ):
        if not end-3 >= (text_raw_off if section_label=='.text' else reloc_raw_off)+versus:
            raise ValueError(f'{section_label} last 3 bytes are not section raw padding')
        _require_zero(result, end-3, 3, f'{section_label} raw-end trailer')
        result[end-3:end] = encoded
    return result
