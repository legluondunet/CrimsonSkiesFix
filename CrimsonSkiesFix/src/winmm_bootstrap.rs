//! WinMM bootstrap that intercepts only the two authenticated game's DirectDraw IAT slots.
//! Very small loader-lock path: no file IO, no Rust allocation, no LoadLibrary,
//! no logging, and no game fix initialization during DLL_PROCESS_ATTACH.
//! Runtime verification and the existing compatibility hooks execute later,
//! when the game's DirectDrawCreate or DirectDrawEnumerateA is actually called.
use core::{ffi::c_void,ptr};
use std::sync::atomic::{AtomicU32,AtomicUsize,Ordering};

#[link(name="kernel32")]
extern "system" {
    fn GetModuleHandleW(name:*const u16)->*mut c_void;
    fn LoadLibraryW(name:*const u16)->*mut c_void;
    fn VirtualProtect(address:*mut c_void,size:usize,new_protection:u32,old_protection:*mut u32)->i32;
}

const DDRAW:[u16;10]=[100,100,114,97,119,46,100,108,108,0];
const PAGE_READWRITE:u32=0x04;
// Resolved IAT targets are captured before replacement. They are the actual
// backend functions the PE loader selected (native DirectDraw or a wrapper).
static ORIGINAL_CREATE:AtomicUsize=AtomicUsize::new(0);
static ORIGINAL_ENUMERATE:AtomicUsize=AtomicUsize::new(0);
pub(crate) fn original_create()->usize {ORIGINAL_CREATE.load(Ordering::Acquire)}
pub(crate) fn original_enumerate()->usize {ORIGINAL_ENUMERATE.load(Ordering::Acquire)}
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
    // Unlike the earlier implementation, do NOT assume ddraw.dll is already
    // visible through GetModuleHandleW during WINMM's DLL_PROCESS_ATTACH.
    // The PE loader resolves these ordinary game imports before game entry.
    // Capture the actual resolved IAT addresses, then let the existing
    // DirectDraw hooks execute later, outside the loader lock.
    unsafe{install_recognized_iat(base)}
}

// Called only after the exact preimage is authenticated, either by process
// attach or by a synthetic Win32 unit test. No file IO/LoadLibrary/heap work.
unsafe fn install_recognized_iat(base:*mut u8)->u32 {
    let create=unsafe{base.add(IAT_CREATE)}.cast::<u32>();
    let enumerate=unsafe{base.add(IAT_ENUMERATE)}.cast::<u32>();
    let original_create=unsafe{ptr::read_volatile(create)} as usize;
    let original_enum=unsafe{ptr::read_volatile(enumerate)} as usize;
    // Reject missing/unresolved imports, import thunks pointing back into
    // CRIMSON.EXE, and any IAT that is already hooked by this module.
    let image_start=base as usize;
    let Some(image_end)=image_start.checked_add(IMAGE_SIZE as usize) else {return 4};
    let invalid=|p:usize|p<0x10000 || (image_start..image_end).contains(&p);
    if invalid(original_create) || invalid(original_enum) ||
       original_create==crate::DirectDrawCreate as usize ||
       original_enum==crate::DirectDrawEnumerateA as usize {return 4;}
    // Both pointers occupy a single page; no other image or DLL is modified.
    let mut previous=0u32;
    if unsafe{VirtualProtect(create.cast(),8,PAGE_READWRITE,&mut previous)}==0 {return 5;}
    // Publish the real provider pointers before activating our hooks.
    ORIGINAL_CREATE.store(original_create,Ordering::Release);
    ORIGINAL_ENUMERATE.store(original_enum,Ordering::Release);
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

    // Test the actual VirtualProtect/IAT replacement logic independently of
    // the copyrighted game's EXE, using private committed Win32 memory.
    // This is deliberately not a gameplay or Windows/Wine load-order test.
    #[test]
    fn synthetic_game_iat_patches_both_directdraw_entries() {
        #[link(name="kernel32")]
        extern "system" {
            fn VirtualAlloc(addr:*mut c_void,size:usize,allocation:u32,protect:u32)->*mut c_void;
            fn VirtualFree(addr:*mut c_void,size:usize,free_type:u32)->i32;
        }
        unsafe {
            // MEM_RESERVE | MEM_COMMIT, PAGE_READWRITE
            let memory=VirtualAlloc(ptr::null_mut(),IMAGE_SIZE as usize,0x3000,PAGE_READWRITE);
            assert!(!memory.is_null(),"cannot allocate isolated PE fixture");
            let base=memory.cast::<u8>();
            let put16=|off:usize,val:u16|ptr::write_unaligned(base.add(off).cast::<u16>(),val);
            let put32=|off:usize,val:u32|ptr::write_unaligned(base.add(off).cast::<u32>(),val);
            put16(0,0x5A4D);put32(0x3C,0xF8);
            put32(0xF8,0x4550);put16(0xF8+4,0x14C);
            put16(0xF8+6,6);put16(0xF8+24,0x10B);
            put32(0xF8+24+56,IMAGE_SIZE);
            put32(0xF8+24+104,0x61_F000);
            for (off,val) in [
                (0x61_F014,0x61_FAF0),(0x61_F020,0x62_0534),
                (0x61_F024,0x62_0498),(0x61_F118,0x61_F1FC),
                (0x61_F124,0x62_234C),(0x61_F128,IAT_CREATE as u32),
                (0x61_F1FC,0x62_2320),(0x61_F200,0x62_2334)] {
                put32(off,val);
            }
            for (off,name) in [(0x62_0534,b"WINMM.dll".as_slice()),
                               (0x62_234C,b"DDRAW.dll"),
                               (0x62_2322,b"DirectDrawCreate"),
                               (0x62_2336,b"DirectDrawEnumerateA")] {
                ptr::copy_nonoverlapping(name.as_ptr(),base.add(off),name.len());
            }
            assert!(recognized_image(base));
            // Deliberately do not load ddraw.dll. The loader may have filled
            // the import table even before GetModuleHandleW sees the provider.
            // These fake pointers model an already-resolved foreign DLL.
            const FAKE_CREATE:u32=0x7000_1234;
            const FAKE_ENUMERATE:u32=0x7000_5678;
            assert_eq!(install_recognized_iat(base),4); // unresolved thunks
            put32(IAT_CREATE,FAKE_CREATE);put32(IAT_ENUMERATE,FAKE_ENUMERATE);
            assert_eq!(install_recognized_iat(base),7);
            assert_eq!(original_create(),FAKE_CREATE as usize);
            assert_eq!(original_enumerate(),FAKE_ENUMERATE as usize);
            assert_eq!(u32_at(base,IAT_CREATE),crate::DirectDrawCreate as usize as u32);
            assert_eq!(u32_at(base,IAT_ENUMERATE),crate::DirectDrawEnumerateA as usize as u32);
            assert_eq!(u32_at(base,IAT_ENUMERATE+4),0); // Third thunk remains untouched
            // A second attempt must fail without overwriting the installed hooks.
            assert_eq!(install_recognized_iat(base),4);
            assert_ne!(VirtualFree(memory,0,0x8000),0); // MEM_RELEASE
        }
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
