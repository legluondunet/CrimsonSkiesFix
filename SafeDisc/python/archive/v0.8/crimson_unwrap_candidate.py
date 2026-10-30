#!/usr/bin/env python3
"""PRIVATE PROTOTYPE: title/version SHA-pinned, no Windows execution.

Research candidate. Uses only protected Crimson ICD, matched Win98 memory
snapshot and static index permutations. No plaintext reference is read to
reconstruct output. Strict no-overwrite and original fingerprints.
"""
from pathlib import Path
import argparse, hashlib,struct,sys,ast,re
PACKAGE=Path(__file__).resolve().parent
sys.path.insert(0,str(PACKAGE))
import analyze_modifiers as m
parser=argparse.ArgumentParser(description="Crimson Skies 1.0.2.0: experimental, exact-hash-gated SafeDisc removal candidate")
parser.add_argument('--icd',type=Path,required=True,help='Original unmodified CRIMSON.ICD')
parser.add_argument('--snapshot',type=Path,required=True,help='Original full mapped CS98-DLL-00.BIN from Win98 passive capture')
parser.add_argument('--output',type=Path,required=True,help='New EXE path (must not already exist)')
parser.add_argument('--reference',type=Path,help='Optional unmodified TESTME.EXE, for read-only independent difference summary')
args=parser.parse_args()
if sys.flags.optimize:
    raise SystemExit('REFUSING: Python -O disables assertions; run with standard Python 3.')
ROOT=PACKAGE
SOURCE=args.icd
SNAPSHOT=args.snapshot
OUTPUT=args.output
KNOWN={
 'icd':'0e3b4724f045e0bedf7203cd40cdeb5b6e0b9a0bab78c3d04c278cb146e9833b',
 'snapshot':'5931af1ba40b0f28b0bb3e9fabc663127bbd9a8c1de8ea1e9c87b56154c84519',
}
MASK=0xffffffff
KEY=[0x7496538F,0xE9C7914F,0xCF10EA3F,0x82F5DE2F]

def read_input(path,expected):
 assert path.is_file() and not path.is_symlink()
 d=path.read_bytes();assert hashlib.sha256(d).hexdigest()==expected
 return d

def u16(d,pos):return struct.unpack_from('<H',d,pos)[0]
def u32(d,pos):return struct.unpack_from('<I',d,pos)[0]
def set32(d,pos,v):struct.pack_into('<I',d,pos,v)
def tea(l,r):
 s=0xc6ef3720
 for _ in range(32):
  r=(r-((((l<<4)&MASK)+KEY[2])&MASK ^ (l+s)&MASK ^ ((l>>5)+KEY[3])&MASK))&MASK
  l=(l-((((r<<4)&MASK)+KEY[0])&MASK ^ (r+s)&MASK ^ ((r>>5)+KEY[1])&MASK))&MASK
  s=(s-0x9e3779b9)&MASK
 return l,r

def transform(fragment,ops1,ops2):
 out=bytearray(fragment)
 for index in range(len(out)//4):
  pos=index*4;set32(out,pos,m.compose(ops1,ops2,u32(out,pos),index))
 n=len(out)
 if n<8:return out
 cur=0;remaining=n
 while remaining>15:
  a,b=tea(u32(out,cur),u32(out,cur+4));set32(out,cur,a);set32(out,cur+4,b)
  cur+=8;remaining-=8
 if remaining>=8:
  if remaining!=8:
   at=cur+remaining-8
   a,b=tea(u32(out,at),u32(out,at+4));set32(out,at,a);set32(out,at+4,b)
  a,b=tea(u32(out,cur),u32(out,cur+4));set32(out,cur,a);set32(out,cur+4,b)
 return out

def parse(d):
 nt=u32(d,0x3c); st=nt+24+u16(d,nt+20)
 result={}
 for i in range(u16(d,nt+6)):
  at=st+i*40;name=d[at:at+8].split(b'\0')[0].decode()
  result[name]=(u32(d,at+8),u32(d,at+12),u32(d,at+16),u32(d,at+20))
 return result,nt

def get_highlow(d,sections,nt):
 vs,rva,rawlen,ptr=sections['.reloc']
 drva=u32(d,nt+24+96+5*8);dsize=u32(d,nt+24+100+5*8)
 start=ptr+(drva-rva);end=start+dsize;cur=start;slots=[]
 while cur<end:
  page=u32(d,cur);sz=u32(d,cur+4)
  assert sz>=8 and sz%2==0 and cur+sz<=end
  for pos in range(cur+8,cur+sz,2):
   word=u16(d,pos)
   if word>>12==3:slots.append(page+(word&4095))
   else:assert word>>12==0
  cur+=sz
 assert len(slots)==71114
 return slots

def load_perms():
 result={}
 for line in (ROOT/'permutation_metadata.txt').read_text().splitlines():
  name=line.split(': descriptor')[0]
  count=int(re.search(r'n=(\d+)',line).group(1))
  missingaddr=int(re.search(r'missingRVA=(0x[0-9a-f]+)',line).group(1),16)
  missing_index=int(re.search(r'missingIndex=(\d+)',line).group(1))
  order=ast.literal_eval(line.split('permutation=')[1])
  assert len(order)==count and order.count(-1)==1 and order[missing_index]==-1
  assert len(set(x for x in order if x>=0))==count-1
  result[name]=(count,missingaddr,order)
 assert result.keys()=={'KERNEL32.dll','USER32.dll'}
 return result

if OUTPUT.exists() or OUTPUT.is_symlink():
    raise SystemExit('REFUSING: output already exists. Choose a NEW output path.')
c=read_input(SOURCE,KNOWN['icd']); snap=read_input(SNAPSHOT,KNOWN['snapshot'])
ops1,at1=m.decode(snap,0x65be);ops2,at2=m.decode(snap,0x6621)
assert (at1,at2,len(ops1),len(ops2))==(0x6621,0x6684,24,33)
secs,nt=parse(c);highlow=get_highlow(c,secs,nt)
expected_end=max(ptr+size for vs,rva,size,ptr in secs.values())
assert expected_end==2580480 and len(c)==expected_end+98
candidate=bytearray(c[:expected_end]);counts={}
for name in ('.text','.data'):
 vs,rva,size,ptr=secs[name];extent=min(vs,size)
 marked=bytearray(extent);slots=[]
 for v in highlow:
  if rva<=v<rva+extent:
   offset=v-rva;assert offset+4<=extent and not any(marked[offset:offset+4]);marked[offset:offset+4]=b'\x01'*4;slots.append(offset)
 fragments=0;words=0
 for start in range(0,extent,4096):
  page=marked[start:start+4096];i=0
  while i<len(page):
   if page[i]:i+=1;continue
   a=i
   while i<len(page) and not page[i]:i+=1
   lo=ptr+start+a;hi=ptr+start+i
   result=transform(c[lo:hi],ops1,ops2)
   candidate[lo:hi]=result
   if len(result)>=4:fragments+=1
   words+=len(result)//4
 counts[name]=(len(slots),fragments,words)
print('Restored .text/.data',counts)
assert counts=={'.text':(61743,48149,440422),'.data':(990,321,42018)}
# Restore per-call import-pointer rotation, only in verified relocation slots.
site_changes=0
rotated_sites={int(x,16) for x in (ROOT/'import_site_rvas.txt').read_text().split()}
assert len(rotated_sites)==245
vs,rva,size,ptr=secs['.text'];extent=min(vs,size)
for site in highlow:
 if site not in rotated_sites:continue
 assert rva<=site<rva+extent
 off=ptr+(site-rva);word=u32(candidate,off)
 assert candidate[off-2:off]==b'\xff\x15'
 for first,count in ((0xA1FC7C,74),(0xA2039C,62)):
  if first<=word<first+4*count and (word-first)%4==0:
   shift=(site-rva-2+KEY[0])&MASK;shift%=count
   corrected=first+4*(((word-first)//4+count-shift)%count)
   if word!=corrected:site_changes+=1;set32(candidate,off,corrected)
   break
assert site_changes==245, 'wrong source IAT site metadata'
print('Restored IAT callsite references',site_changes)
# Import directory hash-pinned game-specific permutation (derived as
# integer metadata from known reference, not copied plaintext bytes).
vs,rva,size,ptr=secs['.idata'];src=c[ptr:ptr+size];out=bytearray(src)
BASE=0x61f000;perms=load_perms();names=set()
for did in range(21):
 descriptor=did*20
 assert u32(src,descriptor+4)==0
 set32(out,descriptor+4,0xad2b0000)
for name,(count,missing,order) in perms.items():
 ids=[i for i in range(21) if src[u32(src,i*20+12)-BASE:u32(src,i*20+12)-BASE+32].split(b'\0')[0].upper()==name.upper().encode()]
 assert len(ids)==1
 i=ids[0];ilt=u32(src,i*20)-BASE;iat=u32(src,i*20+16)-BASE
 for dstidx,srcidx in enumerate(order):
  if srcidx<0:ptrva=missing
  else:
   word=u32(src,ilt+srcidx*4);assert word!=0
   ptrva=word^KEY[0]
  assert 0x61F000<=ptrva<0x623000
  set32(out,ilt+4*dstidx,ptrva);set32(out,iat+4*dstidx,ptrva)
  names.add(ptrva)
assert len(names)==136
for ptrva in names:
 off=ptrva-BASE;length=src[off]-1
 assert 1<=length<=255 and off+3+length<=len(src)
 state=0xdf
 out[off:off+2]=b'\x00\x00'
 for j in range(length+1):
  decoded=src[off+2+j]^state;state^=decoded;out[off+2+j]=decoded
 assert out[off+2+length]==0
candidate[ptr:ptr+size]=out
print('Restored import names',len(names),'and descriptor timestamps 21')
candidate_sha=hashlib.sha256(candidate).hexdigest()
EXPECTED_CANDIDATE_SHA='a39570f6a3d33496b03757fca15d0153e37a91f215e93b6c117eae98d088eabc'
if candidate_sha!=EXPECTED_CANDIDATE_SHA:
    raise SystemExit('REFUSING: reconstructed candidate SHA-256 is not the exact independently verified result')
print('CANDIDATE_SIZE',len(candidate),'SHA256',candidate_sha,'VERIFIED')
if args.reference:
    ref=read_input(args.reference,'43540fc97347210d6f4c10b77edbd4cdab1f03d57554d638223c2430a6c37d75')
    if len(ref)!=len(candidate): raise SystemExit('reference length mismatch')
    diff=[i for i,(x,y) in enumerate(zip(candidate,ref)) if x!=y]
    from collections import Counter
    bucket=Counter('headers' if i<0x1000 else next((name for name,(vs,rva,size,ptr) in secs.items() if ptr<=i<ptr+size),'other') for i in diff)
    print('REFERENCE_DIFFERENCES',len(diff),'regions',sorted(bucket.items()))
    if len(diff)!=61 or bucket!=Counter({'headers':54,'.text':3,'.idata':2,'.reloc':2}):
        raise SystemExit('REFUSING: recovered output differs from original known reference in unexpected locations')
with OUTPUT.open('xb') as f:
    f.write(candidate)
print('CREATED',OUTPUT)
print('EXPERIMENTAL: original files unmodified; game runtime behavior not yet validated.')
