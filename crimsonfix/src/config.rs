#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub borderless: bool,
    pub desktop_resolution: bool,
    pub fps_limit: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self { borderless: true, desktop_resolution: true, fps_limit: 60 }
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
    let mut in_graphics = false;
    for line in content.lines() {
        let text = line.split([';', '#']).next().unwrap_or("").trim();
        if text.is_empty() { continue; }
        if text.starts_with('[') && text.ends_with(']') {
            in_graphics = text[1..text.len() - 1].eq_ignore_ascii_case("Graphics");
            continue;
        }
        if !in_graphics { continue; }
        let Some((name, value)) = text.split_once('=') else { continue };
        match name.trim().to_ascii_lowercase().as_str() {
            "borderless" => if let Some(v) = enabled(value) { settings.borderless = v; },
            "desktopresolution" => if let Some(v) = enabled(value) { settings.desktop_resolution = v; },
            "fpslimit" => if let Ok(v) = value.trim().parse::<u32>() { settings.fps_limit = v.clamp(1, 60); },
            _ => {},
        }
    }
    settings
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults() { assert_eq!(parse(""), Config::default()); }
    #[test]
    fn scoped_keys() {
        assert_eq!(parse("[Other]\nFPSLimit=12\n[Graphics]\nFPSLimit=30\nBorderless=no\nDesktopResolution=off"),
                   Config { borderless:false, desktop_resolution:false, fps_limit:30 });
    }
    #[test]
    fn cannot_exceed_sixty() {
        assert_eq!(parse("[graphics]\nFPSLimit=1000").fps_limit,60);
        assert_eq!(parse("[graphics]\nFPSLimit=0").fps_limit,1);
    }
    #[test]
    fn invalid_options_keep_defaults() {
        assert_eq!(parse("[Graphics]\nFPSLimit=-7\nBorderless=maybe"),Config::default());
    }
}
