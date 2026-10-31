//! Byte-exact delta from owner-supplied CRIMSON-UNWRAPPED.EXE to the
//! owner-supplied 1920x1200 FOV/HUD-fixed reference.
//! Every changed byte is checked before any writes. No proprietary EXE included.
//! No code generation/branch relocation: both images use IMAGE_BASE 0x400000.
use super::{PatchResult,IMAGE_BASE,IMAGE_SIZE,WIDTH_RVA,HEIGHT_RVA,PAGE_EXECUTE_READWRITE,get32,valid_landscape_dimensions,GetModuleHandleW,VirtualProtect,GetCurrentProcess,FlushInstructionCache};
use std::{ffi::c_void,ptr};
const DELTA:&str=r#"2b678:d905a0406000:e95c6e1d0090
2b67f:d9059c406000:e9846e1d0090
41100:ff2485801344:83f8030f84b8
41107:8b0d0cf764:0000e90601
4110d:6a00:0090
41229:68000300006800040000e8e8fbffff6a00:ff354f256000ff354b256000e8e6fbffff
4123c:e88ffeffff68000300006800040000e860feffff6a006a00e817fdffff680003000068000400:6a00e88dfeffffff354f256000ff354b256000e85cfeffff6a006a00e813fdffffff354f2560
41263:e9f30000008b150cf764:ff354b256000e9ed0000
55c01:9c4060:f0efa1
55c0e:904060:f4efa1
19ea39:0f8da3:e9a400
19ea3e:00:90
1a190e:3a:00
1a8eb1:ba01000000ff24858c905a00b880020000b99001000089150803a100c705bc03a10040010000c705c003a100c8000000a30404a100a3e003a100890d0804a100:83f8040f84c50000008b0d4b256000e9e70000009090909090909090909090909090909090909090909090909090909090909090909090909090909090
1a8fc9:b900030000eb6db880040000:8b0d4f256000eb6c90909090
1a9053:0f9ec1:909090
1aa08b:8d54241852:e92d840500
1c159f:06:00
2024bd:0000000000:db054f2560
2024c3:0000000000:da354b2560
2024c9:0000000000:d91df8b371
2024cf:000000000000000000000000000000:8d54241852e9b77bfaffd905a04060
2024df:0000000000:d80de03260
2024e5:000000000000000000:d9f2ddd8db054b2560
2024ef:0000000000:da354f2560
2024f5:0000000000:d835d07b60
2024fb:0000000000000000000000:dec9d9e8d9f3d835e03260
202507:000000000000:c3d9059c4060
20250e:0000000000:d80de03260
202514:000000000000000000:d9f2ddd8db054b2560
20251e:0000000000:da354f2560
202524:0000000000:d835d07b60
20252a:0000000000000000000000:dec9d9e8d9f3d835e03260
202536:0000000000:d915f0efa1
20253c:00000000000000:d9c0d80de03260
202544:0000000000:dd1df4efa1
20254a:000000:c38007
20254f:0000:b004"#;
fn decode_hex(s:&str)->Option<Vec<u8>>{
    if s.len()%2!=0{return None;}
    (0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).ok()).collect()
}
fn parse()->Option<Vec<(usize,Vec<u8>,Vec<u8>)>>{
    let mut out=Vec::new();
    for row in DELTA.lines(){
        let mut fields=row.split(':');
        let rva=usize::from_str_radix(fields.next()?,16).ok()?;
        let old=decode_hex(fields.next()?)?;
        let new=decode_hex(fields.next()?)?;
        if fields.next().is_some()||old.is_empty()||old.len()!=new.len(){return None;}
        out.push((rva,old,new));
    }
    if out.len()!=38{return None;}
    Some(out)
}
pub(super) unsafe fn apply(width:u32,height:u32)->PatchResult{
    if !valid_landscape_dimensions(width,height){return PatchResult::InvalidMonitor;}
    let base=GetModuleHandleW(ptr::null());
    if base.is_null()||base as usize!=IMAGE_BASE{return PatchResult::UnsupportedExecutable;}
    let bytes=std::slice::from_raw_parts(base.cast::<u8>(),IMAGE_SIZE);
    let Some(pe)=get32(bytes,0x3c).map(|v|v as usize) else{return PatchResult::UnsupportedExecutable;};
    if pe<0x40||pe>0x400||bytes.get(0..2)!=Some(b"MZ")||
        bytes.get(pe..pe+4)!=Some(b"PE\0\0")||
        bytes.get(pe+24..pe+26)!=Some(&[0x0b,0x01][..])||
        get32(bytes,pe+24+28)!=Some(IMAGE_BASE as u32)||
        get32(bytes,pe+24+56)!=Some(IMAGE_SIZE as u32){
        return PatchResult::UnsupportedExecutable;
    }
    let Some(mut changes)=parse() else{return PatchResult::UnsupportedExecutable;};
    // Replace the two patch-specific dimensions; all 29 reference EXEs
    // have identical code and differ only in these two little-endian DWORDs.
    // Their first bytes overlap the final two short diff-runs.
    for (rva,_,replacement) in changes.iter_mut(){
        if *rva==0x20254a {replacement[1..3].copy_from_slice(&width.to_le_bytes()[..2]);}
        if *rva==0x20254f {replacement.copy_from_slice(&height.to_le_bytes()[..2]);}
    }
    // Width and height greater than 65535 are disallowed by valid_dimensions.
    // Complete high bytes were zero in the original reference and remain zero.
    // Check every original byte before even one page is made writable.
    if changes.iter().any(|(rva,old,_)|
        bytes.get(*rva..(*rva).saturating_add(old.len()))!=Some(old.as_slice())){
        crate::log("WINDOWED-WIDE: full FOV/HUD delta mismatch; executable untouched");
        return PatchResult::UnsupportedExecutable;
    }
    let mut done:Vec<(usize,Vec<u8>)>=Vec::new();
    let mut failure=None;
    for (rva,old,new) in changes {
        let address=(base as usize+rva) as *mut c_void;
        let mut previous=0u32;
        if VirtualProtect(address,new.len(),PAGE_EXECUTE_READWRITE,&mut previous)==0{
            failure=Some(PatchResult::ProtectFailed);break;
        }
        ptr::copy_nonoverlapping(new.as_ptr(),address.cast::<u8>(),new.len());
        done.push((rva,old));
        let flushed=FlushInstructionCache(GetCurrentProcess(),address.cast_const(),new.len());
        let mut ignored=0u32;
        let restored=VirtualProtect(address,new.len(),previous,&mut ignored);
        if flushed==0||restored==0 {failure=Some(PatchResult::RestoreProtectionFailed);break;}
    }
    if let Some(reason)=failure {
        let mut ok=true;
        for (rva,old) in done.into_iter().rev(){
            let address=(base as usize+rva) as *mut c_void;
            let mut prev=0;
            if VirtualProtect(address,old.len(),PAGE_EXECUTE_READWRITE,&mut prev)==0{ok=false;continue;}
            ptr::copy_nonoverlapping(old.as_ptr(),address.cast::<u8>(),old.len());
            let flushed=FlushInstructionCache(GetCurrentProcess(),address.cast_const(),old.len());
            let mut ignored=0;
            if VirtualProtect(address,old.len(),prev,&mut ignored)==0||flushed==0{ok=false;}
        }
        crate::log(if ok{"WINDOWED-WIDE: full patch failed; rollback complete"}
             else{"WINDOWED-WIDE: CRITICAL full patch rollback incomplete"});
        return reason;
    }
    crate::log(&format!("WINDOWED-WIDE: full unwrapped FOV/HUD reference delta applied; dimensions={}x{}",width,height));
    PatchResult::Patched
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn reference_delta_valid_and_non_overlapping(){
        let d=parse().unwrap();assert_eq!(d.len(),38);
        let mut prev=0;
        for (rva,old,new) in d {
            assert!(rva>=prev);assert_eq!(old.len(),new.len());
            prev=rva+old.len();
        }
        assert!(prev<IMAGE_SIZE);
    }
}
