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
    "; Experimental Wine-only ROFFILE.DLL INI parser EOF guard. Disabled by default.\n",
    "RoffileTerminatorFix=0\n",
    "\n",
    "[Debug]\n",
    "Log=0\n",
    "; Optional passive audit of DirectDraw surface methods (requires Log=1).\n",
    "SurfaceUsageTrace=0\n",
    "; Crash investigation only: trace every native DirectDraw Blt enter/exit (very verbose).\n",
    "CrashBltTrace=0\n",
    "; Optional diagnostic: read 9 RGB32/RGB565 pixel pairs after selected native Blts.\n",
    "ColorProbe=0\n",
    "; One-shot BMP captures of native RGB32 source and RGB565 output; off by default.\n",
    "ColorCapture=0\n",
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
    pub surface_usage_trace: bool,
    pub crash_blt_trace: bool,
    pub color_probe: bool,
    pub color_capture: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { borderless: true, desktop_resolution: true, fps_limit: 60, modern_fps_pacing: false, experimental_windowed_adapter: true, focus_repaint: true, window_erase_guard: true, legacy_offscreen_rgb565: true, native_3d_widescreen: true, gos_video_lock: crate::gos_video_lock::Mode::On, roffile_terminator_fix: false, log_enabled: false, surface_usage_trace: false, crash_blt_trace: false, color_probe: false, color_capture: false }
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
            ("debug", "colorcapture") =>
                if let Some(v) = enabled(value) { settings.color_capture = v; },
            ("debug", "colorprobe") =>
                if let Some(v) = enabled(value) { settings.color_probe = v; },
            ("debug", "crashblttrace") =>
                if let Some(v) = enabled(value) { settings.crash_blt_trace = v; },
            ("debug", "surfaceusagetrace") =>
                if let Some(v) = enabled(value) { settings.surface_usage_trace = v; },
            ("debug", "log") =>
                if let Some(v) = enabled(value) { settings.log_enabled = v; },
            _ => {},
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roffile_terminator_fix_is_opt_in_and_scoped() {
        assert!(!Config::default().roffile_terminator_fix);
        assert!(!parse("[Graphics]\nRoffileTerminatorFix=1").roffile_terminator_fix);
        assert!(!parse("[Debug]\nRoffileTerminatorFix=1").roffile_terminator_fix);
        assert!(parse("[Compatibility]\nRoffileTerminatorFix=1").roffile_terminator_fix);
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
                   Config { borderless:false, desktop_resolution:false, fps_limit:30, modern_fps_pacing:false, experimental_windowed_adapter:true, focus_repaint:true, window_erase_guard:true, legacy_offscreen_rgb565:true, native_3d_widescreen:true, gos_video_lock: crate::gos_video_lock::Mode::On, roffile_terminator_fix:false, log_enabled:false, surface_usage_trace:false, crash_blt_trace:false, color_probe:false, color_capture:false });
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
    fn color_capture_is_debug_only_and_default_off(){
        assert!(!Config::default().color_capture);
        assert!(!parse("[Graphics]\nColorCapture=1").color_capture);
        assert!(parse("[Debug]\nColorCapture=1\nLog=1").color_capture);
        assert!(!parse("[Debug]\nColorCapture=1\nColorCapture=0").color_capture);
    }
    #[test]
    fn color_probe_is_debug_only_default_off(){
        assert!(!Config::default().color_probe);
        assert!(!parse("[Graphics]\nColorProbe=1").color_probe);
        assert!(parse("[Debug]\nColorProbe=1\nLog=1").color_probe);
        assert!(!parse("[Debug]\nColorProbe=1\nColorProbe=0").color_probe);
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
        assert_eq!(parse(shipped).gos_video_lock, crate::gos_video_lock::Mode::On);
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
