// Used only for first-run initialization. Never overwrite an existing
// CrimsonSkiesFix.ini: it contains the user's persistent preferences.
pub const DEFAULT_INI: &str = concat!(
    "; CrimsonSkiesFix — user settings; created once. Safe to edit.\n",
    "[Graphics]\n",
    "Borderless=1\n",
    "DesktopResolution=1\n",
    "FPSLimit=60\n",
    "; Experimental: pace only primary-surface modern Flip (off by default).\n",
    "ModernFPSPacing=0\n",
    "; Verified Wine/Proton native DirectDraw4 3D adapter (on by default).\n",
    "ExperimentalWindowedAdapter=1\n",
    "; One-shot repaint after focus or exposed-window return; enabled by default.\n",
    "FocusRepaint=1\n",
    "; Prevent white background erase over native DirectDraw pixels; enabled by default.\n",
    "WindowEraseGuard=1\n",
    "; Legacy first-image color fix: force RGB565 on verified game-sized offscreen source (on by default).\n",
    "LegacyOffscreenRGB565=1\n",
    "; Native FOV/HUD and startup driver/resolution prompt bypass for verified 1.02 EXE (on by default).\n",
    "Native3DWidescreen=1\n",
    "; Native GOS Graphics/Resolution control lock (on | off | restore). On by default: writes backed-up small crimptch.rof.\n",
    "GosVideoLock=on\n",
    "\n",
    "[Compatibility]\n",
    "; Wine-only ROFFILE.DLL INI parser EOF guard. Enabled by default on supported DLLs.\n",
    "RoffileTerminatorFix=1\n",
    "\n",
    "[Debug]\n",
    "Log=0\n",
    "; Diagnostic only: postpone the in-memory FOV/HUD patch until DD4 mode negotiation.\n",
    "DeferEarlyNativePatch=0\n",
    "; Native Windows uses physical monitor pixels automatically to avoid DPI scaling bugs.\n",
    "; Diagnostic only: force the same physical-pixel probe on Wine/Proton.\n",
    "EarlyNativePhysicalMode=0\n",
    "; Diagnostic ONLY: opt into system-DPI awareness before the game's first HWND.\n",
    "; Try this only under native Windows when UI/HUD is cropped at 125% DPI.\n",
    "; May alter legacy game behavior; never default-on or apply under Wine.\n",
    "NativeDpiAware=0\n",
    "; Experimental native Windows A/B: hook verified game's DD4 mode setup BEFORE first exclusive request; off by default.\n",
    "NativeDD4Bootstrap=0\n",
    "; Native Win11/DirectShow A/B: match the original 32-bit XRGB movie\n",
    "; backbuffer at 800x600 instead of the RGB565 conversion path.\n",
    "; Requires NativeDD4Bootstrap=1.\n",
    "; Experimental; disabled by default and never used on Wine/Proton.\n",
    "NativeMovieRGB32=0\n",
    "; Windows-only: bounded post-Flip game-window message dispatch, experimental; off by default.\n",
    "; Requires NativeMovieRGB32=1 and NativeDD4Bootstrap=1.\n",
    "NativeMovieMessagePump=0\n",
    "; Native Windows-only opt-in 4:3 menu mouse coordinate remap, 800x600 virtual movie graph.\n",
    "; Disabled by default: original Win32 mouse messages remain unchanged.\n",
    "NativeMenuMouseRemap=0\n",
    "; Native Windows-only A/B: repair stale ClipCursor from pre-borderless small game window.\n",
    "; Requires native DD4 virtual graph and verified game-owned HWND; disabled by default.\n",
    "NativeMouseClipRepair=0\n",
    "; Native Windows 800x600 only: test visibility using standard OS arrow in MainMenu.\n",
    "; Requires WindowEraseGuard; no virtual coordinate remap. Debug only, OFF by default.\n",
    "NativeSystemCursorProbe=0\n",
    "; Native Windows only: passive DirectInput mouse API tracing (off by default).\n",
    "; Logs device acquisition, mouse state/buttons and errors; no input modification.\n",
    "NativeDirectInputMouseTrace=0\n",
    "; Native Win11 mouse compatibility experiment: replace invalid DINPUT\n",
    "; exclusive+background (0x9) mouse mode with nonexclusive+foreground (0x6).\n",
    "; Exact authenticated GUID_SysMouse only; requires Log=1 + NativeDD4Bootstrap=1.\n",
    "; OFF by default and NEVER used on Wine/Proton.\n",
    "NativeDirectInputMouseCoopRepair=0\n",
    "; Native Windows 800x600 MainMenu only: synchronize original GOS cursor\n",
    "; with mouse position on centered 4:3 menu (DirectInput deltas only).\n",
    "; Original buttons and game renderer preserved. Experimental / OFF.\n",
    "NativeGosCursorBridge=0\n",
    "; Windows-native RGB32 800x600 videos/menus: compose full window offscreen\n",
    "; (black background + aspect-correct 4:3 frame), then one primary Blt.\n",
    "; No separate sidebar rectangles/GDI; fallback if unsupported by driver.\n",
    "; STRICTLY experimental and OFF by default, never applied to 3D or Wine.\n",
    "NativeFullFrameCompose=0\n",
    "; Optional passive audit of DirectDraw surface methods (requires Log=1).\n",
    "SurfaceUsageTrace=0\n",
    "; Crash investigation only: trace every native DirectDraw Blt enter/exit (very verbose).\n",
    "CrashBltTrace=0\n",
    "; Diagnostic only: report 120 successive successful DD4 Flip frametimes, not on-screen display timestamps. Requires Log=1.\n",
    "FrameTimeTrace=0\n",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub borderless: bool,
    pub desktop_resolution: bool,
    pub fps_limit: u32,
    pub modern_fps_pacing: bool,
    pub experimental_windowed_adapter: bool,
    pub focus_repaint: bool,
    pub window_erase_guard: bool,
    pub legacy_offscreen_rgb565: bool,
    pub native_3d_widescreen: bool,
    pub gos_video_lock: crate::gos_video_lock::Mode,
    pub roffile_terminator_fix: bool,
    pub log_enabled: bool,
    // Diagnostic-only: defer in-memory widescreen patch to native DD4 setup.
    pub defer_early_native_patch: bool,
    // Diagnostic-only: use EnumDisplaySettingsW physical mode for early EXE patch.
    pub early_native_physical_mode: bool,
    // Debug-only: early process-wide SYSTEM DPI awareness for a native Windows A/B.
    pub native_dpi_aware: bool,
    // Native Windows A/B: exact-game in-memory entrypoint bootstrap for DD4.
    pub native_dd4_bootstrap: bool,
    // Native Windows movie-only A/B: physical 32-bit XRGB backbuffer and 16-bit logical mode.
    pub native_movie_rgb32: bool,
    // Windows-only optional bounded, post-Flip WndProc pump to diagnose stalled UI.
    pub native_movie_message_pump: bool,
    // Native Windows RGB32 800x600 A/B: game HWND mouse coordinates to
    // letterboxed 800x600 logical menu coordinates. Off by default.
    pub native_menu_mouse_remap: bool,
    // Native Windows A/B only: replace a stale verified game ClipCursor rectangle
    // with exact borderless HWND client bounds, after successful setup.
    pub native_mouse_clip_repair: bool,
    // Diagnostic-only original OS arrow in native menu, never gameplay.
    pub native_system_cursor_probe: bool,
    pub native_direct_input_mouse_trace: bool,
    pub native_direct_input_mouse_coop_repair: bool,
    pub native_gos_cursor_bridge: bool,
    // Native-only exact XRGB8888 800x600 UI: offscreen background and
    // video compose, single complete window Blt to avoid white gutters.
    pub native_full_frame_compose: bool,
    // Diagnostic-only: export forwarding without any DirectDraw or game hooks.
    pub directdraw_passthrough: bool,
    pub surface_usage_trace: bool,
    pub crash_blt_trace: bool,
    pub frame_time_trace: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { borderless: true, desktop_resolution: true, fps_limit: 60, modern_fps_pacing: false, experimental_windowed_adapter: true, focus_repaint: true, window_erase_guard: true, legacy_offscreen_rgb565: true, native_3d_widescreen: true, gos_video_lock: crate::gos_video_lock::Mode::On, roffile_terminator_fix: true, log_enabled: false, defer_early_native_patch: false, early_native_physical_mode: false, native_dpi_aware: false, native_dd4_bootstrap: false, native_movie_rgb32: false, native_movie_message_pump: false, native_menu_mouse_remap:false, native_mouse_clip_repair:false, native_system_cursor_probe:false, native_direct_input_mouse_trace:false, native_direct_input_mouse_coop_repair:false, native_gos_cursor_bridge:false, native_full_frame_compose:false, directdraw_passthrough: false, surface_usage_trace: false, crash_blt_trace: false, frame_time_trace: false, }
    }
}

fn enabled(text: &str) -> Option<bool> {
    match text.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

pub fn parse(content: &str) -> Config {
    let mut settings = Config::default();
    let mut section = "";
    for line in content.lines() {
        let text = line.split([';', '#']).next().unwrap_or("").trim();
        if text.is_empty() { continue; }
        if text.starts_with('[') && text.ends_with(']') {
            section = if text[1..text.len() - 1].eq_ignore_ascii_case("Graphics") {
                "graphics"
            } else if text[1..text.len() - 1].eq_ignore_ascii_case("Debug") {
                "debug"
            } else if text[1..text.len() - 1].eq_ignore_ascii_case("Compatibility") {
                "compatibility"
            } else { "" };
            continue;
        }
        let Some((name, value)) = text.split_once('=') else { continue };
        match (section, name.trim().to_ascii_lowercase().as_str()) {
            ("graphics", "borderless") =>
                if let Some(v) = enabled(value) { settings.borderless = v; },
            ("graphics", "desktopresolution") =>
                if let Some(v) = enabled(value) { settings.desktop_resolution = v; },
            ("graphics", "experimentalwindowedadapter") =>
                if let Some(v) = enabled(value) { settings.experimental_windowed_adapter = v; },
            ("graphics", "native3dwidescreen") =>
                if let Some(v) = enabled(value) { settings.native_3d_widescreen = v; },
            ("graphics", "gosvideolock") =>
                match value.trim().to_ascii_lowercase().as_str() {
                    "on" | "1" | "true" => settings.gos_video_lock = crate::gos_video_lock::Mode::On,
                    "off" | "0" | "false" => settings.gos_video_lock = crate::gos_video_lock::Mode::Off,
                    "restore" => settings.gos_video_lock = crate::gos_video_lock::Mode::Restore,
                    _ => {},
                },
            ("graphics", "legacyoffscreenrgb565") =>
                if let Some(v) = enabled(value) { settings.legacy_offscreen_rgb565 = v; },
            ("graphics", "windoweraseguard") =>
                if let Some(v) = enabled(value) { settings.window_erase_guard = v; },
            ("graphics", "focusrepaint") =>
                if let Some(v) = enabled(value) { settings.focus_repaint = v; },
            ("graphics", "modernfpspacing") =>
                if let Some(v) = enabled(value) { settings.modern_fps_pacing = v; },
            ("graphics", "fpslimit") =>
                if let Ok(v) = value.trim().parse::<u32>() { settings.fps_limit = v.clamp(1, 60); },
            ("compatibility", "roffileterminatorfix") =>
                if let Some(v) = enabled(value) { settings.roffile_terminator_fix = v; },
            ("debug", "crashblttrace") =>
                if let Some(v) = enabled(value) { settings.crash_blt_trace = v; },
            ("debug", "frametimetrace") =>
                if let Some(v) = enabled(value) { settings.frame_time_trace = v; },
            ("debug", "surfaceusagetrace") =>
                if let Some(v) = enabled(value) { settings.surface_usage_trace = v; },
            ("debug", "log") =>
                if let Some(v) = enabled(value) { settings.log_enabled = v; },
            ("debug", "nativedd4bootstrap") =>
                if let Some(v) = enabled(value) { settings.native_dd4_bootstrap = v; },
            ("debug", "nativemoviergb32") =>
                if let Some(v) = enabled(value) { settings.native_movie_rgb32 = v; },
            ("debug", "nativemoviemessagepump") =>
                if let Some(v) = enabled(value) { settings.native_movie_message_pump = v; },
            ("debug", "nativemenumouseremap") =>
                if let Some(v) = enabled(value) { settings.native_menu_mouse_remap = v; },
            ("debug", "nativemousecliprepair") =>
                if let Some(v) = enabled(value) { settings.native_mouse_clip_repair = v; },
            ("debug", "nativesystemcursorprobe") =>
                if let Some(v) = enabled(value) { settings.native_system_cursor_probe = v; },
            ("debug", "nativedirectinputmousetrace") =>
                if let Some(v) = enabled(value) { settings.native_direct_input_mouse_trace = v; },
            ("debug", "nativedirectinputmousecooprepair") =>
                if let Some(v) = enabled(value) { settings.native_direct_input_mouse_coop_repair = v; },
            ("debug", "nativegoscursorbridge") =>
                if let Some(v) = enabled(value) { settings.native_gos_cursor_bridge = v; },
            ("debug", "nativefullframecompose") =>
                if let Some(v) = enabled(value) { settings.native_full_frame_compose = v; },
            ("debug", "nativedpiaware") =>
                if let Some(v) = enabled(value) { settings.native_dpi_aware = v; },
            ("debug", "earlynativephysicalmode") =>
                if let Some(v) = enabled(value) { settings.early_native_physical_mode = v; },
            ("debug", "deferearlynativepatch") =>
                if let Some(v) = enabled(value) { settings.defer_early_native_patch = v; },
            ("debug", "directdrawpassthrough") =>
                if let Some(v) = enabled(value) { settings.directdraw_passthrough = v; },
            _ => {},
        }
    }
    settings
}

/// Return the canonical [section]/key of each INI line, preserving line
/// indexes. Only known sections are considered; unknown/custom keys can
/// never be removed or migrated by our deduplication step.
fn ini_line_keys(content:&str)->Vec<Option<String>>{
    let mut section=String::new();
    content.split_inclusive('\n').map(|line|{
        let clean=line.split([';', '#']).next().unwrap_or("").trim();
        if clean.starts_with('[') && clean.ends_with(']') && clean.len()>2{
            let candidate=clean[1..clean.len()-1].trim().to_ascii_lowercase();
            section=if matches!(candidate.as_str(),
                "graphics"|"debug"|"compatibility"){candidate}else{String::new()};
            return None;
        }
        if section.is_empty(){return None;}
        let (key,_)=clean.split_once('=')?;
        let key=key.trim();
        if key.is_empty(){return None;}
        Some(format!("{section}/{}",key.to_ascii_lowercase()))
    }).collect()
}

/// Normalize repeated known INI keys while preserving the EXACT effective
/// parsed configuration, comments, unknown options and original line endings.
/// Example: if an old valid value is followed by an invalid duplicate, the
/// valid value wins as before. This function only returns text, never writes
/// user files or changes any default; a separate guarded file operation
/// backs up the original before committing any cleanup.
pub fn deduplicate_known_ini(content:&str)->Option<(String,usize)>{
    use std::collections::{BTreeMap,BTreeSet};
    let lines:Vec<&str>=content.split_inclusive('\n').collect();
    let keys=ini_line_keys(content);
    if keys.len()!=lines.len(){return None;}
    let recognized:BTreeSet<String>=ini_line_keys(DEFAULT_INI)
        .into_iter().flatten().collect();
    let mut groups:BTreeMap<String,Vec<usize>>=BTreeMap::new();
    for (index,key) in keys.iter().enumerate(){
        if let Some(key)=key{
            if recognized.contains(key){groups.entry(key.clone()).or_default().push(index);}
        }
    }
    let original=parse(content);
    let mut keep=vec![true;lines.len()];
    let mut removed=0usize;
    for indexes in groups.values().filter(|v|v.len()>1){
        let saved=keep.clone();
        // Prefer the last VALID/effective occurrence, never blindly the
        // last text line: invalid trailing values are ignored by parse().
        let mut accepted=false;
        for &candidate in indexes.iter().rev(){
            for &i in indexes{keep[i]=i==candidate;}
            let trial:String=lines.iter().enumerate()
                .filter_map(|(i,line)|keep[i].then_some(*line)).collect();
            if parse(&trial)==original{
                removed+=indexes.len()-1;
                accepted=true;
                break;
            }
            keep.clone_from(&saved);
        }
        if !accepted{keep.clone_from(&saved);}
    }
    if removed==0{return None;}
    let output:String=lines.iter().enumerate()
        .filter_map(|(i,line)|keep[i].then_some(*line)).collect();
    if parse(&output)!=original{return None;}
    Some((output,removed))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_duplicates_in_generated_or_shipped_ini(){
        assert!(deduplicate_known_ini(DEFAULT_INI).is_none());
        assert!(deduplicate_known_ini(include_str!(
            "../CrimsonSkiesFix/CrimsonSkiesFix.ini.example")).is_none());
    }
    #[test]
    fn clean_known_repeated_keys_across_duplicate_sections_without_changing_values(){
        let ini="; keep intro\r\n[Graphics]\r\nGosVideoLock=on\r\nBorderless=0\r\n; preserve comment\r\n[Debug]\r\nNativeGosCursorBridge=0\r\n[Graphics]\r\nGosVideoLock=off\r\nBorderless=1\r\n[Debug]\r\nNativeGosCursorBridge=1\r\nUnknownPluginOption=first\r\nUnknownPluginOption=second\r\n";
        let (output,removed)=deduplicate_known_ini(ini).unwrap();
        assert_eq!(removed,3);
        assert_eq!(parse(&output),parse(ini));
        assert!(output.contains("GosVideoLock=off\r\n"));
        assert!(!output.contains("GosVideoLock=on\r\n"));
        assert!(output.contains("NativeGosCursorBridge=1\r\n"));
        assert!(!output.contains("NativeGosCursorBridge=0\r\n"));
        assert!(output.contains("; preserve comment\r\n"));
        assert!(output.contains("UnknownPluginOption=first\r\nUnknownPluginOption=second\r\n"));
    }
    #[test]
    fn invalid_later_value_never_overrides_previous_valid_value(){
        let ini="[Graphics]\nFPSLimit=50\nFPSLimit=oops\n[Debug]\nNativeDD4Bootstrap=1\nNativeDD4Bootstrap=invalid\n";
        let (cleaned,removed)=deduplicate_known_ini(ini).unwrap();
        assert_eq!(removed,2);
        assert_eq!(parse(&cleaned),parse(ini));
        assert!(cleaned.contains("FPSLimit=50\n"));
        assert!(!cleaned.contains("FPSLimit=oops\n"));
        assert!(cleaned.contains("NativeDD4Bootstrap=1\n"));
        assert!(!cleaned.contains("NativeDD4Bootstrap=invalid\n"));
    }
    #[test]
    fn identical_and_mixed_case_keys_only_cleaned_inside_known_sections(){
        let ini="[Debug]\nLog=1\nLOG=1\n[Modded]\nLOG=0\nLOG=1\n[Graphics]\nBorderless=1\nBORDERLESS=1";
        let (cleaned,count)=deduplicate_known_ini(ini).unwrap();
        assert_eq!(count,2);
        assert_eq!(parse(&cleaned),parse(ini));
        assert_eq!(cleaned.matches("LOG=1").count(),2);
        assert!(cleaned.contains("[Modded]\nLOG=0\nLOG=1"));
    }
    #[test]
    fn roffile_terminator_fix_is_default_on_and_compatibility_scoped() {
        assert!(Config::default().roffile_terminator_fix);
        assert!(parse("[Graphics]\nRoffileTerminatorFix=0").roffile_terminator_fix);
        assert!(parse("[Debug]\nRoffileTerminatorFix=0").roffile_terminator_fix);
        assert!(parse("[Compatibility]\nRoffileTerminatorFix=1").roffile_terminator_fix);
        assert!(!parse("[Compatibility]\nRoffileTerminatorFix=0").roffile_terminator_fix);
        assert!(!parse("[Compatibility]\nRoffileTerminatorFix=1\nRoffileTerminatorFix=0").roffile_terminator_fix);
    }
    #[test]
    fn defaults_disable_logs() {
        assert_eq!(parse(""), Config::default());
        assert!(!parse("[Graphics]\nBorderless=1").log_enabled);
    }
    #[test]
    fn scoped_keys() {
        assert_eq!(parse("[Other]\nFPSLimit=12\nLog=1\n[Graphics]\nFPSLimit=30\nBorderless=no\nDesktopResolution=off"),
                   Config { borderless:false, desktop_resolution:false, fps_limit:30, modern_fps_pacing:false, experimental_windowed_adapter:true, focus_repaint:true, window_erase_guard:true, legacy_offscreen_rgb565:true, native_3d_widescreen:true, gos_video_lock: crate::gos_video_lock::Mode::On, roffile_terminator_fix:true, log_enabled:false, defer_early_native_patch:false, early_native_physical_mode:false, native_dpi_aware:false, native_dd4_bootstrap:false, native_movie_rgb32:false, native_movie_message_pump:false, native_menu_mouse_remap:false, native_mouse_clip_repair:false, native_system_cursor_probe:false, native_direct_input_mouse_trace:false, native_direct_input_mouse_coop_repair:false, native_gos_cursor_bridge:false, native_full_frame_compose:false, directdraw_passthrough:false, surface_usage_trace:false, crash_blt_trace:false, frame_time_trace:false, });
    }
    #[test]
    fn directdraw_passthrough_is_explicit_debug_only_and_default_off(){
        assert!(!Config::default().directdraw_passthrough);
        assert!(parse("[Debug]\nDirectDrawPassThrough=1").directdraw_passthrough);
        assert!(parse("[DEBUG]\ndirectdrawpassthrough=on").directdraw_passthrough);
        assert!(!parse("[Debug]\nDirectDrawPassThrough=0").directdraw_passthrough);
        assert!(!parse("[Graphics]\nDirectDrawPassThrough=1").directdraw_passthrough);
        assert!(!parse("[Compatibility]\nDirectDrawPassThrough=1").directdraw_passthrough);
    }
    #[test]
    fn defer_early_native_patch_is_explicit_debug_only_and_default_off(){
        assert!(!Config::default().defer_early_native_patch);
        assert!(parse("[Debug]\nDeferEarlyNativePatch=1").defer_early_native_patch);
        assert!(parse("[DEBUG]\ndeferearlynativepatch=on").defer_early_native_patch);
        assert!(!parse("[Debug]\nDeferEarlyNativePatch=0").defer_early_native_patch);
        assert!(!parse("[Graphics]\nDeferEarlyNativePatch=1").defer_early_native_patch);
    }
    #[test]
    fn physical_mode_debug_probe_is_off_by_default_and_scoped_to_debug(){
        assert!(!Config::default().early_native_physical_mode);
        assert!(parse("[Debug]\nEarlyNativePhysicalMode=1").early_native_physical_mode);
        assert!(!parse("[Graphics]\nEarlyNativePhysicalMode=1").early_native_physical_mode);
    }
    #[test]
    fn native_full_frame_compose_is_strictly_opt_in_and_debug_only(){
        assert!(!Config::default().native_full_frame_compose);
        assert!(parse("[Debug]\nNativeFullFrameCompose=1").native_full_frame_compose);
        assert!(!parse("[Debug]\nNativeFullFrameCompose=0").native_full_frame_compose);
        assert!(!parse("[Graphics]\nNativeFullFrameCompose=1").native_full_frame_compose);
        assert!(!parse("[Compatibility]\nNativeFullFrameCompose=1").native_full_frame_compose);
    }
    #[test]
    fn native_gos_cursor_bridge_is_debug_opt_in_only(){
        assert!(!Config::default().native_gos_cursor_bridge);
        assert!(parse("[Debug]\nNativeGosCursorBridge=1").native_gos_cursor_bridge);
        assert!(!parse("[Graphics]\nNativeGosCursorBridge=1").native_gos_cursor_bridge);
        assert!(!parse("[Debug]\nNativeGosCursorBridge=0").native_gos_cursor_bridge);
    }
    #[test]
    fn native_directinput_mouse_coop_repair_is_strictly_opt_in(){
        assert!(!Config::default().native_direct_input_mouse_coop_repair);
        assert!(parse("[Debug]\nNativeDirectInputMouseCoopRepair=1").native_direct_input_mouse_coop_repair);
        assert!(!parse("[Graphics]\nNativeDirectInputMouseCoopRepair=1").native_direct_input_mouse_coop_repair);
        assert!(!parse("[Debug]\nNativeDirectInputMouseCoopRepair=0").native_direct_input_mouse_coop_repair);
    }
    #[test]
    fn native_directinput_mouse_trace_is_native_debug_opt_in_only(){
        assert!(!Config::default().native_direct_input_mouse_trace);
        assert!(parse("[Debug]\nNativeDirectInputMouseTrace=1").native_direct_input_mouse_trace);
        assert!(!parse("[Graphics]\nNativeDirectInputMouseTrace=1").native_direct_input_mouse_trace);
        assert!(!parse("[Debug]\nNativeDirectInputMouseTrace=0").native_direct_input_mouse_trace);
    }
    #[test]
    fn native_system_cursor_probe_is_strictly_debug_only(){
        assert!(!Config::default().native_system_cursor_probe);
        assert!(parse("[Debug]\nNativeSystemCursorProbe=1").native_system_cursor_probe);
        assert!(!parse("[Debug]\nNativeSystemCursorProbe=0").native_system_cursor_probe);
        assert!(!parse("[Graphics]\nNativeSystemCursorProbe=1").native_system_cursor_probe);
    }
    #[test]
    fn native_mouse_clip_repair_is_debug_only_and_off_by_default(){
        assert!(!Config::default().native_mouse_clip_repair);
        assert!(parse("[Debug]\nNativeMouseClipRepair=1").native_mouse_clip_repair);
        assert!(!parse("[Debug]\nNativeMouseClipRepair=0").native_mouse_clip_repair);
        assert!(!parse("[Graphics]\nNativeMouseClipRepair=1").native_mouse_clip_repair);
    }
    #[test]
    fn native_menu_mouse_remap_is_debug_only_and_off_by_default(){
        assert!(!Config::default().native_menu_mouse_remap);
        assert!(parse("[Debug]\nNativeMenuMouseRemap=1").native_menu_mouse_remap);
        assert!(!parse("[Debug]\nNativeMenuMouseRemap=0").native_menu_mouse_remap);
        assert!(!parse("[Graphics]\nNativeMenuMouseRemap=1").native_menu_mouse_remap);
    }
    #[test]
    fn native_movie_message_pump_is_strictly_opt_in(){
        assert!(!Config::default().native_movie_message_pump);
        assert!(parse("[Debug]\nNativeMovieMessagePump=1").native_movie_message_pump);
        assert!(!parse("[Debug]\nNativeMovieMessagePump=0").native_movie_message_pump);
        assert!(!parse("[Graphics]\nNativeMovieMessagePump=1").native_movie_message_pump);
    }
    #[test]
    fn native_movie_rgb32_is_debug_only_and_off_by_default(){
        assert!(!Config::default().native_movie_rgb32);
        assert!(parse("[Debug]\nNativeMovieRGB32=1").native_movie_rgb32);
        assert!(!parse("[Debug]\nNativeMovieRGB32=0").native_movie_rgb32);
        assert!(!parse("[Graphics]\nNativeMovieRGB32=1").native_movie_rgb32);
        assert!(!parse("[Compatibility]\nNativeMovieRGB32=1").native_movie_rgb32);
    }
    #[test]
    fn native_dd4_bootstrap_is_debug_only_opt_in(){
        assert!(!Config::default().native_dd4_bootstrap);
        assert!(parse("[Debug]\nNativeDD4Bootstrap=1").native_dd4_bootstrap);
        assert!(!parse("[Graphics]\nNativeDD4Bootstrap=1").native_dd4_bootstrap);
        assert!(!parse("[Debug]\nNativeDD4Bootstrap=0").native_dd4_bootstrap);
    }
    #[test]
    fn native_dpi_aware_is_debug_only_and_disabled_by_default(){
        assert!(!Config::default().native_dpi_aware);
        assert!(parse("[Debug]\nNativeDpiAware=1").native_dpi_aware);
        assert!(!parse("[Graphics]\nNativeDpiAware=1").native_dpi_aware);
        assert!(!parse("[Compatibility]\nNativeDpiAware=1").native_dpi_aware);
        assert!(!parse("[Debug]\nNativeDpiAware=0").native_dpi_aware);
    }
    #[test]
    fn experimental_adapter_is_on_by_default_and_graphics_scoped(){
        assert!(Config::default().experimental_windowed_adapter);
        assert!(parse("[Debug]\nExperimentalWindowedAdapter=0").experimental_windowed_adapter);
        assert!(parse("[Graphics]\nExperimentalWindowedAdapter=1").experimental_windowed_adapter);
        assert!(!parse("[Graphics]\nExperimentalWindowedAdapter=0").experimental_windowed_adapter);
    }
    #[test]
    fn native_widescreen_and_startup_dialog_bypass_default_on(){
        // The in-memory AskForVDev bypass is gated by Native3DWidescreen.
        assert!(Config::default().native_3d_widescreen);
        assert!(parse("[Debug]\nNative3DWidescreen=0").native_3d_widescreen);
        assert!(parse("[Graphics]\nNative3DWidescreen=1").native_3d_widescreen);
        assert!(!parse("[Graphics]\nNative3DWidescreen=0").native_3d_widescreen);
    }
    #[test]
    fn legacy_offscreen_rgb565_is_default_on_and_graphics_only(){
        assert!(Config::default().legacy_offscreen_rgb565);
        assert!(parse("[Debug]\nLegacyOffscreenRGB565=0").legacy_offscreen_rgb565);
        assert!(parse("[Graphics]\nLegacyOffscreenRGB565=1").legacy_offscreen_rgb565);
        assert!(!parse("[Graphics]\nLegacyOffscreenRGB565=0").legacy_offscreen_rgb565);
        assert!(!parse("[Graphics]\nLegacyOffscreenRGB565=1\nLegacyOffscreenRGB565=0").legacy_offscreen_rgb565);
    }
    #[test]
    fn window_erase_guard_is_on_by_default_and_graphics_scoped(){
        assert!(Config::default().window_erase_guard);
        assert!(parse("[Debug]\nWindowEraseGuard=0").window_erase_guard);
        assert!(parse("[Graphics]\nWindowEraseGuard=1").window_erase_guard);
        assert!(!parse("[Graphics]\nWindowEraseGuard=0").window_erase_guard);
        assert!(!parse("[Graphics]\nWindowEraseGuard=1\nWindowEraseGuard=0").window_erase_guard);
    }
    #[test]
    fn focus_repaint_is_on_by_default_and_graphics_scoped(){
        assert!(Config::default().focus_repaint);
        assert!(parse("[Debug]\nFocusRepaint=0").focus_repaint);
        assert!(parse("[Graphics]\nFocusRepaint=1").focus_repaint);
        assert!(!parse("[Graphics]\nFocusRepaint=0").focus_repaint);
        assert!(!parse("[Graphics]\nFocusRepaint=1\nFocusRepaint=0").focus_repaint);
    }
    #[test]
    fn retired_pixel_diagnostics_are_ignored_without_changing_defaults(){
        // Old user INIs are not rewritten on upgrade. Ignoring these two
        // retired debug keys must never enable a side effect.
        assert_eq!(
            parse("[Debug]\nColorCapture=1\nColorProbe=1"),
            Config::default()
        );
    }
    #[test]
    fn frametime_trace_is_debug_only_off_by_default_and_requires_logging(){
        assert!(!Config::default().frame_time_trace);
        assert!(parse("[Debug]\nFrameTimeTrace=1\nLog=1").frame_time_trace);
        assert!(!parse("[Graphics]\nFrameTimeTrace=1").frame_time_trace);
        assert!(!parse("[Debug]\nFrameTimeTrace=1\nFrameTimeTrace=0").frame_time_trace);
        assert!(!parse("[Debug]\nFrameTimeTrace=bad").frame_time_trace);
    }
    #[test]
    fn crash_blt_trace_is_debug_only_and_off_by_default(){
        assert!(!Config::default().crash_blt_trace);
        assert!(!parse("[Graphics]\nCrashBltTrace=1").crash_blt_trace);
        assert!(parse("[Debug]\nCrashBltTrace=1\nLog=1").crash_blt_trace);
        assert!(!parse("[Debug]\nCrashBltTrace=1\nCrashBltTrace=0").crash_blt_trace);
    }
    #[test]
    fn modern_pacing_is_explicit_opt_in_and_graphics_only(){
        assert!(!Config::default().modern_fps_pacing);
        assert!(!parse("[Debug]\nModernFPSPacing=1").modern_fps_pacing);
        assert!(parse("[Graphics]\nModernFPSPacing=1").modern_fps_pacing);
        assert!(!parse("[Graphics]\nModernFPSPacing=1\nModernFPSPacing=0").modern_fps_pacing);
    }
    #[test]
    fn surface_usage_trace_is_opt_in_and_debug_scoped(){
        assert!(!Config::default().surface_usage_trace);
        assert!(!parse("[Graphics]\nSurfaceUsageTrace=1").surface_usage_trace);
        assert!(parse("[Debug]\nSurfaceUsageTrace=1\nLog=1").surface_usage_trace);
        assert!(!parse("[Debug]\nSurfaceUsageTrace=1\nSurfaceUsageTrace=0").surface_usage_trace);
    }
    #[test]
    fn cannot_exceed_sixty() {
        assert_eq!(parse("[graphics]\nFPSLimit=1000").fps_limit,60);
        assert_eq!(parse("[graphics]\nFPSLimit=0").fps_limit,1);
    }
    #[test]
    fn invalid_options_keep_defaults() {
        assert_eq!(parse("[Graphics]\nFPSLimit=-7\nBorderless=maybe\n[Debug]\nLog=invalid"),Config::default());
    }
    #[test]
    fn debug_logging_explicit_opt_in() {
        assert!(parse("[Debug]\nLog=1").log_enabled);
        assert!(parse("[DEBUG]\nlog=on").log_enabled);
        assert!(!parse("[Debug]\nLog=0").log_enabled);
        assert!(!parse("[Other]\nLog=1").log_enabled);
        assert!(!parse("[Graphics]\nLog=1").log_enabled);
    }
    #[test]
    fn debug_key_does_not_reinterpret_graphics_keys() {
        let c=parse("[Debug]\nFPSLimit=10\nLog=1\n[Graphics]\nFPSLimit=59");
        assert_eq!(c.fps_limit,59);
        assert!(c.log_enabled);
    }
    #[test]
    fn shipped_ini_template_matches_actual_generated_config_defaults() {
        // The shipped template and generated first-run defaults must match
        // the explicitly selected project policy, including enabled recovery.
        let shipped = include_str!("../CrimsonSkiesFix/CrimsonSkiesFix.ini.example");
        assert_eq!(parse(shipped), parse(DEFAULT_INI));
        // Parsing alone is insufficient: unknown/retired keys are silently
        // ignored and can linger in the published example indefinitely.
        // Compare the complete normalized key multiset (not a set), which
        // also catches accidental duplicate entries in either INI.
        let mut generated:Vec<String>=ini_line_keys(DEFAULT_INI)
            .into_iter().flatten().collect();
        let mut template:Vec<String>=ini_line_keys(shipped)
            .into_iter().flatten().collect();
        generated.sort_unstable();
        template.sort_unstable();
        assert_eq!(template,generated,
            "INI example must contain exactly the current generated keys");
        assert!(deduplicate_known_ini(shipped).is_none());

        assert_eq!(parse(shipped).gos_video_lock, crate::gos_video_lock::Mode::On);
        assert!(parse(shipped).roffile_terminator_fix);
        assert!(parse(shipped).experimental_windowed_adapter);
        assert!(parse(shipped).native_3d_widescreen);
        assert!(parse(shipped).legacy_offscreen_rgb565);
        assert!(parse(shipped).focus_repaint);
        assert!(parse(shipped).window_erase_guard);
        assert!(parse(shipped).desktop_resolution);
    }
    #[test]
    fn first_run_enables_native_video_lock_and_startup_bypass_dependencies() {
        let initial = parse(DEFAULT_INI);
        assert!(initial.desktop_resolution);
        assert!(initial.experimental_windowed_adapter);
        assert!(initial.native_3d_widescreen);
        assert!(initial.legacy_offscreen_rgb565);
        assert!(initial.focus_repaint);
        assert!(initial.window_erase_guard);
        assert_eq!(initial.gos_video_lock, crate::gos_video_lock::Mode::On);
        assert!(initial.roffile_terminator_fix);
        assert!(!initial.log_enabled);
        assert!(!initial.modern_fps_pacing);

        // Owner can opt out of persistent GOS override independently.
        let opt_out = parse("[Graphics]\nGosVideoLock=off\nNative3DWidescreen=0");
        assert_eq!(opt_out.gos_video_lock, crate::gos_video_lock::Mode::Off);
        assert!(!opt_out.native_3d_widescreen);
        // Explicit rollback is independent of native-wide prerequisites.
        let rollback = parse("[Graphics]\nNative3DWidescreen=0\nGosVideoLock=restore");
        assert_eq!(rollback.gos_video_lock, crate::gos_video_lock::Mode::Restore);
    }
    #[test]
    fn native_gos_video_lock_is_explicit_and_graphics_scoped(){
        use crate::gos_video_lock::Mode;
        assert_eq!(parse("").gos_video_lock,Mode::On);
        assert_eq!(parse("[Graphics]\nGosVideoLock=on").gos_video_lock,Mode::On);
        assert_eq!(parse("[Graphics]\nGosVideoLock=1").gos_video_lock,Mode::On);
        assert_eq!(parse("[Graphics]\nGosVideoLock=restore").gos_video_lock,Mode::Restore);
        assert_eq!(parse("[Graphics]\nGosVideoLock=OFF").gos_video_lock,Mode::Off);
        assert_eq!(parse("[Debug]\nGosVideoLock=off").gos_video_lock,Mode::On);
        assert_eq!(parse("[Graphics]\nGosVideoLock=maybe").gos_video_lock,Mode::On);
    }

}
