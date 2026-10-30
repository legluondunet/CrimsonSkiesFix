#!/usr/bin/env python3
"""Read-only, bounded PE32 memory-image x86 EAX modifier inspection.

Research-only. Input is a user-owned loaded image snapshot at RVA 0.
No execution of DLL code, Windows APIs, game writes, or output of code bytes.

Copyright (C) 2026 legluondunet
SPDX-License-Identifier: GPL-3.0-or-later
"""
from __future__ import annotations
import argparse, hashlib, struct, collections
from pathlib import Path

MASK=0xFFFFFFFF
ROUTINES=(("F1",0x65BE),("F2",0x6621))
EXPECTED_STOPS={"F1":0x6621,"F2":0x6684}
EXPECTED_SHA="5931af1ba40b0f28b0bb3e9fabc663127bbd9a8c1de8ea1e9c87b56154c84519"
KNOWN_ON_DISK_SHA="dc43dbae45a8bfc39299ab0d1d5dbc72302f3bc37faf7c3b41e8ffdf4c6838ff"


def get_u16(d,off):
    if off<0 or off+2>len(d):raise ValueError(f"u16 truncated at 0x{off:X}")
    return struct.unpack_from('<H',d,off)[0]


def get_u32(d,off):
    if off<0 or off+4>len(d):raise ValueError(f"u32 truncated at 0x{off:X}")
    return struct.unpack_from('<I',d,off)[0]


def pe_info(d):
    if len(d)<0x400 or d[:2]!=b'MZ':raise ValueError('not a mapped PE32 image')
    nt=get_u32(d,0x3c)
    if d[nt:nt+4]!=b'PE\0\0' or get_u16(d,nt+4)!=0x14c:
        raise ValueError('expected PE32 x86')
    opt=nt+24
    if get_u16(d,opt)!=0x10b:raise ValueError('expected PE32 optional header')
    count=get_u16(d,nt+6)
    sections=[]
    st=opt+get_u16(d,nt+20)
    for k in range(count):
        at=st+k*40
        sections.append((d[at:at+8].split(b'\0')[0].decode('ascii','replace'),
                         get_u32(d,at+12),get_u32(d,at+16),get_u32(d,at+20)))
    return {'preferred_base':get_u32(d,opt+28), 'size_of_image':get_u32(d,opt+56),
            'sections':sections,'reloc':(get_u32(d,opt+96+8*5),get_u32(d,opt+100+8*5))}


def decode(d,at):
    """Strictly accept EAX-only arithmetic and RET, with no branches or memory access."""
    start=at
    out=[]
    while at-start<256:
        x=d[at]
        if x==0xC3:
            return out,at+1
        if x==0x90:op,arg,size='nop',0,1
        elif x==0x40:op,arg,size='inc',0,1
        elif x==0x48:op,arg,size='dec',0,1
        elif x==0xF7 and d[at+1]==0xD8:op,arg,size='neg',0,2
        elif x==0xC1 and d[at+1] in (0xC0,0xC8):
            op='rol' if d[at+1]==0xC0 else 'ror'
            op,arg,size=op,d[at+2],3
        elif x in (0x05,0x2D,0x35):
            op={0x05:'add',0x2D:'sub',0x35:'xor'}[x]
            op,arg,size=op,get_u32(d,at+1),5
        else:
            raise ValueError(f"Unexpected opcode at RVA 0x{at:X}: 0x{x:02X}; fail closed")
        out.append((op,arg));at+=size
    raise ValueError('Missing RET within 256 bytes; fail closed')


def rol(x,n):
    n &= 31
    return ((x<<n)|(x>>(32-n)))&MASK if n else x&MASK


def ror(x,n):
    n &= 31
    return ((x>>n)|(x<<(32-n)))&MASK if n else x&MASK


def apply(ops,value):
    v=value&MASK
    for op,a in ops:
        if op=='nop':pass
        elif op=='add':v=(v+a)&MASK
        elif op=='sub':v=(v-a)&MASK
        elif op=='xor':v^=a
        elif op=='inc':v=(v+1)&MASK
        elif op=='dec':v=(v-1)&MASK
        elif op=='neg':v=(-v)&MASK
        elif op=='rol':v=rol(v,a)
        elif op=='ror':v=ror(v,a)
        else:raise AssertionError(op)
    return v


def inverse(ops):
    d={'add':'sub','sub':'add','xor':'xor','inc':'dec',
       'dec':'inc','neg':'neg','rol':'ror','ror':'rol','nop':'nop'}
    return [(d[op],a) for op,a in reversed(ops)]


def mask(index):
    return ((index<<24)|(index<<16)|(index<<8)|index)&MASK


def compose(first,second,word,index):
    m=mask(index)
    return apply(second,apply(first,word^m)^m)


def test_ops(first,second):
    tests=[0,1,2,0x12345678,0xDEADBEEF,0x80000000,0xFFFFFFFF]
    state=0x51C0A71E
    for _ in range(512):
        state=(1664525*state+1013904223)&MASK
        tests.append(state)
    for fn in [first,second]:
        inv=inverse(fn)
        for value in tests:
            if apply(inv,apply(fn,value))!=value:
                raise AssertionError('inverse check failed')
    for value in tests:
        for i in [0,1,255,256,257,0xABCD1234]:
            m=mask(i)
            decoded=compose(first,second,value,i)
            # Invert F2, then reapply index XOR and inverse F1.
            actual=apply(inverse(first),apply(inverse(second),decoded)^m)^m
            if actual!=value:raise AssertionError('composite inverse failed')
    return len(tests)


def reloc_info(d,pe):
    at,size=pe['reloc'];end=at+size
    if end>len(d):raise ValueError('reloc directory outside mapped image')
    types=collections.Counter();addresses=[];blocks=0
    while at<end:
        page,sz=get_u32(d,at),get_u32(d,at+4)
        if sz<8 or sz%2 or at+sz>end:raise ValueError('invalid relocation block')
        for z in range(at+8,at+sz,2):
            word=get_u16(d,z);kind=word>>12
            types[kind]+=1
            if kind==3:addresses.append(page+(word&0xFFF))
        at+=sz;blocks+=1
    return blocks,types,addresses


def orig_code_comparison(original,memory,pe):
    """Optional exact user-owned original file comparison; never exports file bytes."""
    if len(original)!=138752 or hashlib.sha256(original).hexdigest()!=KNOWN_ON_DISK_SHA:
        raise ValueError('Original DPLAYERX.DLL does not match the previously verified original hash and length')
    chunks=[]
    for name,va,rawlen,rawptr in pe['sections']:
        chunks.append((name,va,rawlen,rawptr))
    results=[]
    for name,start in ROUTINES:
        end=EXPECTED_STOPS[name]
        matches=[]
        for sn,va,n,pos in chunks:
            if va<=start and end<=va+n:
                matches.append(original[pos+start-va:pos+end-va]==memory[start:end])
        if not matches:raise ValueError('Modifier lacks file-backed section')
        results.append((name,all(matches)))
    return results


def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('snapshot',type=Path,help='CS98-DLL-00.BIN (mapped PE image)')
    ap.add_argument('--original-dll',type=Path,help='optional verified 138752-byte DPLAYERX.DLL')
    ap.add_argument('--load-base',type=lambda x:int(x,0),default=0x00F90000)
    args=ap.parse_args()
    d=args.snapshot.read_bytes()
    h=hashlib.sha256(d).hexdigest()
    print(f'Snapshot size=0x{len(d):X} sha256={h}')
    if h==EXPECTED_SHA:print('Known captured Win98 sample: yes')
    else:print('Known captured Win98 sample: no (inspect independently)')
    pe=pe_info(d)
    if len(d)!=pe['size_of_image']:raise ValueError('snapshot is incomplete')
    print(f'PE32 base preferred=0x{pe["preferred_base"]:08X} actual=0x{args.load_base:08X} delta=0x{(args.load_base-pe["preferred_base"])&MASK:X}')
    print(f'PE32 sections={[(n,hex(v)) for n,v,_,_ in pe["sections"]]}')
    blocks,types,relocs=reloc_info(d,pe)
    print(f'Relocations: {blocks} blocks, types={dict(sorted(types.items()))}')
    decoded=[]
    for name,rva in ROUTINES:
        ops,ret=decode(d,rva)
        if ret!=EXPECTED_STOPS[name]:raise ValueError(f'{name} unexpected end RVA 0x{ret:X}')
        # Any 4-byte relocation overlapping the instruction window would require caution.
        overlaps=[v for v in relocs if v<ret and v+4>rva]
        if overlaps:raise ValueError(f'{name} overlaps relocation fields {overlaps}')
        decoded.append(ops)
        print(f'{name} starts RVA 0x{rva:X}, RET RVA 0x{ret-1:X}, 0x{ret-rva:X} bytes, {len(ops)} EAX-only ops, reloc-overlap=0')
        print('  Opcode categories: '+str(dict(sorted(collections.Counter(op for op,_ in ops).items()))))
    f1,f2=decoded
    n=test_ops(f1,f2)
    print(f'Forward/inverse validation: PASS on {n} synthetic words per function + composite indices')
    for src in [0,1,0x12345678,0xFFFFFFFF]:
        print(f'Synthetic input 0x{src:08X}: F1=0x{apply(f1,src):08X} F2=0x{apply(f2,src):08X} composite(index 0)=0x{compose(f1,f2,src,0):08X} composite(index 256)=0x{compose(f1,f2,src,256):08X}')
    if args.original_dll:
        original=args.original_dll.read_bytes()
        print('Memory-to-original-disk function comparison:',orig_code_comparison(original,d,pe))
    print('Scope: these are observed EAX-only modifiers, NOT a full SafeDisc ICD decryption implementation.')

if __name__=='__main__':
    try:main()
    except (ValueError,AssertionError,OSError,IndexError) as exc:
        raise SystemExit('ANALYSIS REFUSED: '+str(exc))
