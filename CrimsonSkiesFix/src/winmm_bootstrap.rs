//! WinMM bootstrap that intercepts only the two authenticated game's DirectDraw IAT slots.
//! Very small loader-lock path: no file IO, no Rust allocation, no LoadLibrary,
//! no logging, and no game fix initialization during DLL_PROCESS_ATTACH.
//! Runtime verification and the existing compatibility hooks execute later,
//! when the game's DirectDrawCreate or DirectDrawEnumerateA is actually called.
use core::{ffi::{c_char,c_void},ptr};
use std::sync::atomic::{AtomicU32,Ordering};

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn GetProcAddress(module:*mut c_void,name:*const c_char)->*const c_void;
    fn LoadLibraryW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,size:usize,new_protection:u32,old_protection:*mut u32)->i32;
}

const DDRAW:[u16;10]=[100,100,114,97,119,46,100,108,108,0];
const PAGE_READWRITE:u32=0x04;
const IAT_CREATE:usize=0x61_FBA4;
const IAT_ENUMERATE:usize=0x61_FBA8;
const IMAGE_SIZE:u32=0x65_2000;

static STATUS:AtomicU32=AtomicU32::new(0);
pub(crate) fn status()->u32 {STATUS.load(Ordering::Acquire)}

// Diagnostic access point for the x86 app-local loader smoke test.
// Read-only and never used to bypass the real game executable hash gate.
#[no_mangle]
pub extern "system" fn CrimsonSkiesFixBootstrapStatus()->u32 {status()}

#[inline]
unsafe fn u16_at(base:*const u8,off:usize)->u16 {
    unsafe{ptr::read_unaligned(base.add(off).cast::<u16>())}
}
#[inline]
unsafe fn u32_at(base:*const u8,off:usize)->u32 {
    unsafe{ptr::read_unaligned(base.add(off).cast::<u32>())}
}
#[inline]
unsafe fn ascii_equals(base:*const u8, rva:usize, expect:&[u8])->bool {
    // Only invoked after validating the fixed PE image shape and its size.
    expect.iter().enumerate().all(|(i,x)| unsafe{*base.add(rva+i)==*x}) &&
        unsafe{*base.add(rva+expect.len())==0}
}

// An intentionally narrow PE preimage, derived from the exact SHA-256-approved
// 1.02 executable. This is only a loader-lock-safe *structural* filter; the
// full on-disk hash is verified by exe_gate on the first DirectDraw API call.
unsafe fn recognized_image(base:*const u8)->bool {
    if base.is_null(){return false;}
    if unsafe{u16_at(base,0)}!=0x5A4D || unsafe{u32_at(base,0x3C)}!=0xF8 {return false;}
    let pe=0xF8usize;
    if unsafe{u32_at(base,pe)}!=0x4550 ||
       unsafe{u16_at(base,pe+4)}!=0x14C ||
       unsafe{u16_at(base,pe+6)}!=6 {return false;}
    let opt=pe+24;
    if unsafe{u16_at(base,opt)}!=0x10B ||
       unsafe{u32_at(base,opt+56)}!=IMAGE_SIZE ||
       unsafe{u32_at(base,opt+104)}!=0x61_F000 {return false;}
    // Exactly the two relevant authenticated import descriptors and thunks.
    if unsafe{u32_at(base,0x61_F014)}!=0x61_FAF0 ||
       unsafe{u32_at(base,0x61_F014+12)}!=0x62_0534 ||
       unsafe{u32_at(base,0x61_F014+16)}!=0x62_0498 ||
       unsafe{u32_at(base,0x61_F118)}!=0x61_F1FC ||
       unsafe{u32_at(base,0x61_F118+12)}!=0x62_234C ||
       unsafe{u32_at(base,0x61_F118+16)}!=IAT_CREATE as u32 ||
       unsafe{u32_at(base,0x61_F1FC)}!=0x62_2320 ||
       unsafe{u32_at(base,0x61_F200)}!=0x62_2334 {return false;}
    unsafe{
        ascii_equals(base,0x62_0534,b"WINMM.dll") &&
        ascii_equals(base,0x62_234C,b"DDRAW.dll") &&
        ascii_equals(base,0x62_2322,b"DirectDrawCreate") &&
        ascii_equals(base,0x62_2336,b"DirectDrawEnumerateA")
    }
}

// All status codes are safely readable AFTER loader initialization through the
// WinMM first-use diagnostic or the regular CrimsonSkiesFix game log.
unsafe fn install_exe_iat()->u32 {
    let base=unsafe{GetModuleHandleW(ptr::null())}.cast::<u8>();
    if !unsafe{recognized_image(base)} {return 1;} // Not our exact PE layout.
    let ddraw=unsafe{GetModuleHandleW(DDRAW.as_ptr())};
    if ddraw.is_null(){return 2;} // DDRAW may still be awaiting loader initialization.
    let original_create=unsafe{GetProcAddress(ddraw,c"DirectDrawCreate".as_ptr())} as usize;
    let original_enum=unsafe{GetProcAddress(ddraw,c"DirectDrawEnumerateA".as_ptr())} as usize;
    if original_create==0 || original_enum==0 {return 3;}
    let create=unsafe{base.add(IAT_CREATE)}.cast::<u32>();
    let enumerate=unsafe{base.add(IAT_ENUMERATE)}.cast::<u32>();
    if unsafe{ptr::read_volatile(create)}!=original_create as u32 ||
       unsafe{ptr::read_volatile(enumerate)}!=original_enum as u32 {return 4;}
    // Both pointers occupy a single page; no other image or DLL is modified.
    let mut previous=0u32;
    if unsafe{VirtualProtect(create.cast(),8,PAGE_READWRITE,&mut previous)}==0 {return 5;}
    unsafe{
        ptr::write_volatile(create,crate::DirectDrawCreate as usize as u32);
        ptr::write_volatile(enumerate,crate::DirectDrawEnumerateA as usize as u32);
    }
    let mut discarded=0u32;
    let restore=unsafe{VirtualProtect(create.cast(),8,previous,&mut discarded)};
    if restore==0 {return 6;}
    7 // Two game IAT slots now target the same original CrimsonSkiesFix hooks.
}

pub(crate) unsafe fn loaded_ddraw()->Option<*mut c_void> {
    // Called after loader lock by normal runtime code, not from DllMain.
    let mut module=unsafe{GetModuleHandleW(DDRAW.as_ptr())};
    if module.is_null(){module=unsafe{LoadLibraryW(DDRAW.as_ptr())};}
    (!module.is_null()).then_some(module)
}

#[no_mangle]
pub unsafe extern "system" fn DllMain(_: *mut c_void,reason:u32,_reserved:*mut c_void)->i32 {
    if reason==1 /* DLL_PROCESS_ATTACH */ {
        STATUS.store(unsafe{install_exe_iat()},Ordering::Release);
    }
    1 // A failed compatibility hook must never abort DLL loading/multimedia.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_non_game_images() {
        let data=vec![0u8;0x200];
        assert!(!unsafe{recognized_image(data.as_ptr())});
    }
    #[test]
    fn approved_image_descriptor_shape() {
        let mut data=vec![0u8;IMAGE_SIZE as usize];
        let put16=|d:&mut Vec<u8>,o:usize,v:u16|d[o..o+2].copy_from_slice(&v.to_le_bytes());
        let put32=|d:&mut Vec<u8>,o:usize,v:u32|d[o..o+4].copy_from_slice(&v.to_le_bytes());
        put16(&mut data,0,0x5A4D);put32(&mut data,0x3C,0xF8);
        put32(&mut data,0xF8,0x4550);put16(&mut data,0xF8+4,0x14C);
        put16(&mut data,0xF8+6,6);put16(&mut data,0xF8+24,0x10B);
        put32(&mut data,0xF8+24+56,IMAGE_SIZE);
        put32(&mut data,0xF8+24+104,0x61_F000);
        for (offset,val) in [
          (0x61_F014,0x61_FAF0),(0x61_F014+12,0x62_0534),
          (0x61_F014+16,0x62_0498),(0x61_F118,0x61_F1FC),
          (0x61_F118+12,0x62_234C),(0x61_F118+16,IAT_CREATE as u32),
          (0x61_F1FC,0x62_2320),(0x61_F200,0x62_2334)] {
            put32(&mut data,offset,val);
        }
        for (offset,name) in [(0x62_0534,b"WINMM.dll".as_slice()),
                              (0x62_234C,b"DDRAW.dll"),
                              (0x62_2322,b"DirectDrawCreate"),
                              (0x62_2336,b"DirectDrawEnumerateA")] {
            data[offset..offset+name.len()].copy_from_slice(name);
        }
        assert!(unsafe{recognized_image(data.as_ptr())});
        data[0x62_0534]=b'X';
        assert!(!unsafe{recognized_image(data.as_ptr())});
    }
}
