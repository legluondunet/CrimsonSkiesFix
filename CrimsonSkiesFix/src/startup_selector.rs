//! Skip Crimson Skies' obsolete AskForVDev prompt without replacing HAL.
//! Two call sites consult the same user setting via the getter at 0x4404F0.
//! This is a per-process, fail-closed and strictly fingerprinted modification.
use super::{IMAGE_BASE,IMAGE_SIZE,GetModuleHandleW,VirtualProtect,GetCurrentProcess,FlushInstructionCache,PAGE_EXECUTE_READWRITE,get32};
use std::{ptr,ffi::c_void};
const SITES:[(usize,[u8;5]);2]=[
    (0x44bfb,[0xe8,0xf0,0xb8,0xff,0xff]),
    (0xa780a,[0xe8,0xe1,0x8c,0xf9,0xff]),
];
const REPLACEMENT:[u8;5]=[0xb8,0,0,0,0]; // mov eax, 0
fn matches_image(image:&[u8])->bool {
    if image.get(0..2)!=Some(b"MZ"){return false;}
    let Some(pe)=get32(image,0x3c).map(|v|v as usize) else{return false};
    pe>=0x40 && pe<=0x400 && image.get(pe..pe+4)==Some(b"PE\0\0") &&
    image.get(pe+24..pe+26)==Some(&[0x0b,0x01][..]) &&
    get32(image,pe+24+28)==Some(IMAGE_BASE as u32) &&
    get32(image,pe+24+56)==Some(IMAGE_SIZE as u32) &&
    SITES.iter().all(|(rva,expected)|image.get(*rva..*rva+5)==Some(expected.as_slice()))
}
pub unsafe fn apply()->bool {
    let base=GetModuleHandleW(ptr::null());
    if base.is_null() || base as usize!=IMAGE_BASE{return false;}
    let image=std::slice::from_raw_parts(base.cast::<u8>(),IMAGE_SIZE);
    if !matches_image(image){return false;}
    let mut modified=Vec::new();
    for &(rva,original) in &SITES{
        let address=(base as usize+rva) as *mut c_void;
        let mut old=0;
        if VirtualProtect(address,5,PAGE_EXECUTE_READWRITE,&mut old)==0{break;}
        ptr::copy_nonoverlapping(REPLACEMENT.as_ptr(),address.cast::<u8>(),5);
        modified.push((rva,original));
        let flush=FlushInstructionCache(GetCurrentProcess(),address.cast_const(),5);
        let mut unused=0;
        if VirtualProtect(address,5,old,&mut unused)==0 || flush==0 {break;}
    }
    if modified.len()==SITES.len(){return true;}
    for (rva,original) in modified.into_iter().rev(){
        let address=(base as usize+rva) as *mut c_void;
        let mut old=0;
        if VirtualProtect(address,5,PAGE_EXECUTE_READWRITE,&mut old)!=0{
            ptr::copy_nonoverlapping(original.as_ptr(),address.cast::<u8>(),5);
            let _=FlushInstructionCache(GetCurrentProcess(),address.cast_const(),5);
            let mut unused=0;
            let _=VirtualProtect(address,5,old,&mut unused);
        }
    }
    false
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn both_preimages_required(){
        let mut image=vec![0;IMAGE_SIZE];
        image[0..2].copy_from_slice(b"MZ");
        image[0x3c..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        image[0x80..0x84].copy_from_slice(b"PE\0\0");
        image[0x98..0x9a].copy_from_slice(&0x10bu16.to_le_bytes());
        image[0x98+28..0x98+32].copy_from_slice(&(IMAGE_BASE as u32).to_le_bytes());
        image[0x98+56..0x98+60].copy_from_slice(&(IMAGE_SIZE as u32).to_le_bytes());
        for (rva,expected) in SITES {image[rva..rva+5].copy_from_slice(&expected);}
        assert!(matches_image(&image));
        image[SITES[1].0]=0x90;
        assert!(!matches_image(&image));
    }
}
