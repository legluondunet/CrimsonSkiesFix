//! In-game Video Preferences resolution LABEL override for the single
//! SHA-256-authenticated Crimson Skies 1.02 executable.
//!
//! In uiData(0x845), the game pushes (height, width, resource 0x206,
//! output buffer) and calls its original CDECL formatter at 0x4054B0.
//! Intercept ONLY that exact CALL site. Forward every other call
//! unchanged; when the verified GOS Video Preferences page is open,
//! substitute the launch monitor's dimensions as formatter arguments.
//! Never touch the GOS mode table, selection index, text buffers, 4:3
//! menu surfaces or DirectDraw mode. This changes display text only.
use std::{
    ffi::c_void,
    ptr,
    sync::atomic::{AtomicU32, Ordering},
};
use crate::log;

const IMAGE_BASE: usize = 0x0040_0000;
const FORMAT_CALL_SITE: usize = 0x0040_CF31;
const ORIGINAL_FORMATTER: usize = 0x0040_54B0;
const ORIGINAL_CALL: [u8; 5] = [0xE8, 0x7A, 0x85, 0xFF, 0xFF];
const FORMAT_ARGUMENTS_SITE: usize = 0x0040_CF20;
const FORMAT_ARGUMENTS: [u8; 17] = [
    0x52,                         // push edx: height
    0x25, 0xFF, 0xFF, 0x00, 0x00, // and eax,0xffff: width
    0x50,                         // push eax
    0x8D, 0x44, 0x24, 0x68,       // lea eax,[esp+0x68]: buffer
    0x68, 0x06, 0x02, 0x00, 0x00, // push 0x206: resource format
    0x50,                         // push eax: buffer
];
const FORMATTER_PROLOGUE: [u8; 10] = [
    0x8B, 0x4C, 0x24, 0x08, 0x81, 0xEC, 0x00, 0x04, 0x00, 0x00,
];
const AFTER_CALL_SITE: usize = 0x0040_CF36;
const AFTER_CALL: [u8; 5] = [0x8B, 0x55, 0x08, 0x6A, 0x01];
const PAGE_EXECUTE_READWRITE: u32 = 0x40;
const MAX_EVENTS: u32 = 4;
static CHANGED_LABELS: AtomicU32 = AtomicU32::new(0);

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    fn VirtualProtect(addr: *mut c_void, len: usize, new: u32, old: *mut u32) -> i32;
    fn FlushInstructionCache(process: *mut c_void, addr: *const c_void, len: usize) -> i32;
    fn GetCurrentProcess() -> *mut c_void;
}

// At this site the original function is a CDECL formatting helper.
// Its caller cleans 0x1C bytes after the formatter and following
// 3-argument text-assignment call. Keeping CDECL is essential.
type FormatFn = unsafe extern "C" fn(*mut c_void, u32, u32, u32) -> u32;

fn supported_dimensions(w: u32, h: u32) -> bool {
    (320..=16384).contains(&w) && (240..=16384).contains(&h)
}

fn replacement(
    context_valid: bool,
    format_id: u32,
    original: (u32, u32),
    monitor: Option<(u32, u32)>,
) -> Option<(u32, u32)> {
    if !context_valid || format_id != 0x206 || !supported_dimensions(original.0, original.1) {
        return None;
    }
    monitor.filter(|&(w, h)| supported_dimensions(w, h) && (w, h) != original)
}

fn active_video_page() -> bool {
    let cfg = crate::cfg();
    cfg.experimental_windowed_adapter
        && cfg.native_3d_widescreen
        && cfg.desktop_resolution
        && crate::menu_scene::main_menu_was_seen()
        && crate::menu_video_probe::legacy_ui_active()
        && crate::menu_gos_event::preferences_transaction_active()
        && crate::menu_gos_event::video_preferences_open()
}

unsafe extern "C" fn format_resolution(
    output: *mut c_void,
    format_id: u32,
    original_width: u32,
    original_height: u32,
) -> u32 {
    let original: FormatFn = std::mem::transmute(ORIGINAL_FORMATTER);
    // Monitor is frozen before the first exclusive mode switch; querying
    // current DirectDraw modes here would merely return 800x600.
    let page_active = active_video_page();
    let monitor = if page_active {
        crate::win32::capture_launch_monitor()
            .and_then(|rect| rect.dimensions())
            .or_else(crate::win32::sole_primary_monitor_dimensions)
    } else {
        None
    };
    let override_size = replacement(
        page_active,
        format_id,
        (original_width, original_height),
        monitor,
    );
    let (w, h) = override_size.unwrap_or((original_width, original_height));
    // Always reuse the game's formatter and original output pointer. No
    // new GOS buffer ownership, fabricated label or mode-table entry.
    let result = original(output, format_id, w, h);
    if override_size.is_some()
        && crate::cfg().log_enabled
        && CHANGED_LABELS.fetch_add(1, Ordering::Relaxed) < MAX_EVENTS
    {
        log(&format!(
            "MENU-RESOLUTION-LABEL: GOS formatter 0x845 native label={}x{} original={}x{} resource=0x206; mode index/table unchanged",
            w, h, original_width, original_height,
        ));
    }
    result
}

fn rel_call(site: usize, target: usize) -> [u8; 5] {
    let rel = (target as u32)
        .wrapping_sub((site as u32).wrapping_add(5))
        .to_le_bytes();
    [0xE8, rel[0], rel[1], rel[2], rel[3]]
}

/// Call once from the first authenticated DirectDrawCreate export,
/// before the game's Video Preferences scripts are executed.
pub unsafe fn install() -> bool {
    if GetModuleHandleW(ptr::null()) as usize != IMAGE_BASE {
        log("MENU-RESOLUTION-LABEL: image base mismatch; hook disabled");
        return false;
    }
    let preimages: &[(usize, &[u8])] = &[
        (FORMAT_CALL_SITE, &ORIGINAL_CALL),
        (FORMAT_ARGUMENTS_SITE, &FORMAT_ARGUMENTS),
        (ORIGINAL_FORMATTER, &FORMATTER_PROLOGUE),
        (AFTER_CALL_SITE, &AFTER_CALL),
    ];
    for &(site, bytes) in preimages {
        if std::slice::from_raw_parts(site as *const u8, bytes.len()) != bytes {
            log(&format!(
                "MENU-RESOLUTION-LABEL: verified formatter preimage mismatch RVA={:#x}; hook disabled",
                site - IMAGE_BASE
            ));
            return false;
        }
    }
    let mut old = 0u32;
    if VirtualProtect(
        FORMAT_CALL_SITE as *mut c_void, ORIGINAL_CALL.len(), PAGE_EXECUTE_READWRITE, &mut old
    ) == 0 {
        log("MENU-RESOLUTION-LABEL: memory protection denied; hook disabled");
        return false;
    }
    let new_call = rel_call(FORMAT_CALL_SITE, format_resolution as usize);
    ptr::copy_nonoverlapping(new_call.as_ptr(), FORMAT_CALL_SITE as *mut u8, new_call.len());
    let flushed = FlushInstructionCache(
        GetCurrentProcess(), FORMAT_CALL_SITE as *const c_void, new_call.len()
    );
    let mut ignored = 0u32;
    let restored = VirtualProtect(
        FORMAT_CALL_SITE as *mut c_void, ORIGINAL_CALL.len(), old, &mut ignored
    );
    if flushed == 0 || restored == 0 {
        let mut temp = 0u32;
        if VirtualProtect(
            FORMAT_CALL_SITE as *mut c_void, ORIGINAL_CALL.len(), PAGE_EXECUTE_READWRITE, &mut temp
        ) != 0 {
            ptr::copy_nonoverlapping(
                ORIGINAL_CALL.as_ptr(), FORMAT_CALL_SITE as *mut u8, ORIGINAL_CALL.len()
            );
            let _ = FlushInstructionCache(
                GetCurrentProcess(), FORMAT_CALL_SITE as *const c_void, ORIGINAL_CALL.len()
            );
            let mut ignored = 0u32;
            let _ = VirtualProtect(
                FORMAT_CALL_SITE as *mut c_void, ORIGINAL_CALL.len(), old, &mut ignored
            );
        }
        log("MENU-RESOLUTION-LABEL: hook install failed; original CALL restored where possible");
        return false;
    }
    log("MENU-RESOLUTION-LABEL: verified GOS 0x845 formatter hook installed (text only)");
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_original_cdecl_call() {
        assert_eq!(rel_call(FORMAT_CALL_SITE, ORIGINAL_FORMATTER), ORIGINAL_CALL);
        assert_eq!(FORMAT_ARGUMENTS.len(), FORMAT_CALL_SITE - FORMAT_ARGUMENTS_SITE);
    }
    #[test]
    fn native_dimensions_only_when_video_page_and_resource_match() {
        assert_eq!(replacement(true, 0x206, (640,480), Some((1920,1080))), Some((1920,1080)));
        assert_eq!(replacement(true, 0x206, (800,600), Some((2560,1440))), Some((2560,1440)));
        assert_eq!(replacement(true, 0x206, (640,480), Some((1920,1200))), Some((1920,1200)));
        assert_eq!(replacement(false, 0x206, (640,480), Some((1920,1080))), None);
        assert_eq!(replacement(true, 0x207, (640,480), Some((1920,1080))), None);
        assert_eq!(replacement(true, 0x206, (640,480), Some((640,480))), None);
        assert_eq!(replacement(true, 0x206, (640,480), None), None);
        assert_eq!(replacement(true, 0x206, (0,0), Some((1920,1080))), None);
        assert_eq!(replacement(true, 0x206, (640,480), Some((0,0))), None);
    }
}
