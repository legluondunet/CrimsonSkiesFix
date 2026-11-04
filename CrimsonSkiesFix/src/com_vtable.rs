//! Shared native COM vtable patch registry for the verified 32-bit game DLL.
//! This is a mechanical extraction of the original lib.rs code: keying,
//! publication-before-patch, idempotence and rollback behavior are unchanged.
//! Never dereference an arbitrary COM object or install from DllMain.
use std::{collections::HashMap,ffi::c_void,sync::{Mutex,OnceLock}};
use crate::win32;

static ORIGINALS:OnceLock<Mutex<HashMap<(usize,usize),usize>>>=OnceLock::new();

fn originals()->&'static Mutex<HashMap<(usize,usize),usize>> {
    ORIGINALS.get_or_init(||Mutex::new(HashMap::new()))
}

pub(crate) unsafe fn method(object:*mut c_void,index:usize)->usize {
    if object.is_null(){return 0;}
    let table=*(object as *const *const usize);
    if table.is_null(){return 0;}
    *table.add(index)
}

pub(crate) unsafe fn orig(object:*mut c_void,index:usize)->Option<usize> {
    if object.is_null(){return None;}
    let table=*(object as *const *const usize);
    if table.is_null(){return None;}
    originals().lock().ok()?.get(&(table as usize,index)).copied()
}

pub(crate) unsafe fn replace_method(object:*mut c_void,index:usize,hook:usize)->bool {
    if object.is_null(){return false;}
    let table=*(object as *const *mut usize);
    if table.is_null(){return false;}
    let slot=table.add(index);
    let Ok(mut map)=originals().lock() else {return false};
    if map.contains_key(&(table as usize,index)){return true;}
    let original=*slot;
    if original==0 || original==hook{return false;}
    // Publish the original before changing the pointer to avoid a hook
    // seeing an uninitialized trampoline on another thread.
    map.insert((table as usize,index),original);
    if !win32::patch_vtable_slot(slot,hook) {
        map.remove(&(table as usize,index));
        return false;
    }
    true
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_and_missing_vtable_are_rejected_without_patch() {
        unsafe {
            assert_eq!(method(std::ptr::null_mut(),5),0);
            assert_eq!(orig(std::ptr::null_mut(),5),None);
            assert!(!replace_method(std::ptr::null_mut(),5,0x1234));
            let mut missing:*const usize=std::ptr::null();
            let object=(&mut missing as *mut *const usize).cast::<c_void>();
            assert_eq!(method(object,5),0);
            assert_eq!(orig(object,5),None);
            assert!(!replace_method(object,5,0x1234));
        }
    }

    #[test]
    fn reads_existing_vtable_without_rewriting_it() {
        let slots=[0x1234usize,0x5678usize];
        let mut table=slots.as_ptr();
        let object=(&mut table as *mut *const usize).cast::<c_void>();
        unsafe {
            assert_eq!(method(object,0),0x1234);
            assert_eq!(method(object,1),0x5678);
            assert_eq!(orig(object,1),None);
        }
        assert_eq!(slots,[0x1234,0x5678]);
    }

    #[test]
    fn lookup_is_keyed_by_shared_vtable_address_and_slot() {
        let slots=[0x1234usize,0x5678usize];
        let mut table=slots.as_ptr();
        let object=(&mut table as *mut *const usize).cast::<c_void>();
        let key=(slots.as_ptr() as usize,1usize);
        // Registration is synthetic here: do not call VirtualProtect in tests.
        assert!(originals().lock().unwrap().insert(key,0xabcd).is_none());
        assert_eq!(unsafe { orig(object,1) },Some(0xabcd));
        assert_eq!(unsafe { orig(object,0) },None);
        assert_eq!(originals().lock().unwrap().remove(&key),Some(0xabcd));
        assert_eq!(unsafe { orig(object,1) },None);
        assert_eq!(slots,[0x1234,0x5678]);
    }

    #[test]
    fn self_hook_fails_closed_without_patching_native_memory() {
        let slots=[0x1234usize];
        let mut table=slots.as_ptr();
        let object=(&mut table as *mut *const usize).cast::<c_void>();
        unsafe { assert!(!replace_method(object,0,0x1234)); }
        assert_eq!(slots,[0x1234]);
        assert_eq!(unsafe { orig(object,0) },None);
    }

    #[test]
    fn zero_slot_fails_closed_without_touching_registry() {
        let slots=[0usize;2];
        let mut table=slots.as_ptr();
        let object=(&mut table as *mut *const usize).cast::<c_void>();
        unsafe { assert!(!replace_method(object,1,0x1234)); }
        assert_eq!(slots,[0,0]);
        assert_eq!(unsafe { orig(object,1) },None);
    }
}
