// Used only for first-run initialization. Never overwrite an existing
// CrimsonFix.ini: it contains the user's persistent preferences.
pub const DEFAULT_INI: &str = concat!(
    "; CrimsonFix — user settings; created once. Safe to edit.\n",
    "[Graphics]\n",
    "Borderless=1\n",
    "DesktopResolution=1\n",
    "FPSLimit=60\n",
    "\n",
    "[Debug]\n",
    "Log=0\n",
);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub borderless: bool,
    pub desktop_resolution: bool,
    pub fps_limit: u32,
    pub log_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self { borderless: true, desktop_resolution: true, fps_limit: 60, log_enabled: false }
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
            } else { "" };
            continue;
        }
        let Some((name, value)) = text.split_once('=') else { continue };
        match (section, name.trim().to_ascii_lowercase().as_str()) {
            ("graphics", "borderless") =>
                if let Some(v) = enabled(value) { settings.borderless = v; },
            ("graphics", "desktopresolution") =>
                if let Some(v) = enabled(value) { settings.desktop_resolution = v; },
            ("graphics", "fpslimit") =>
                if let Ok(v) = value.trim().parse::<u32>() { settings.fps_limit = v.clamp(1, 60); },
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
    fn defaults_disable_logs() {
        assert_eq!(parse(""), Config::default());
        assert!(!parse("[Graphics]\nBorderless=1").log_enabled);
    }
    #[test]
    fn scoped_keys() {
        assert_eq!(parse("[Other]\nFPSLimit=12\nLog=1\n[Graphics]\nFPSLimit=30\nBorderless=no\nDesktopResolution=off"),
                   Config { borderless:false, desktop_resolution:false, fps_limit:30, log_enabled:false });
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
}
