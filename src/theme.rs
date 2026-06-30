use ratatui::style::Color;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use terminal_colorsaurus::{QueryOptions, ThemeMode, theme_mode};

use crate::errors::{CsvlensError, CsvlensResult};

/// Visual theme used by the TUI.
///
/// Built-in themes are available via [`Theme::dark`], [`Theme::light`], and
/// [`Theme::auto`] (detect terminal light/dark mode). Custom themes can be
/// loaded from TOML files with [`Theme::from_file`] or resolved by name with
/// [`Theme::resolve`].
///
/// Prefer [`Theme::load_preferred`] so the user's config / `CSVLENS_THEME` is
/// applied without passing `--theme` every time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// Column header text color (used when `--color-columns` is off).
    pub header: Color,
    pub row_number: Color,
    pub border: Color,
    pub selected_foreground: Color,
    pub selected_background: Color,
    pub marked_foreground: Color,
    pub marked_background: Color,
    pub found: Color,
    pub found_selected_background: Color,
    pub status: Color,
    pub column_colors: Vec<Color>,
}

impl Default for Theme {
    fn default() -> Self {
        Theme::auto()
    }
}

impl Theme {
    /// Auto-detect terminal theme mode (dark/light). Falls back to dark.
    pub fn auto() -> Self {
        match theme_mode(QueryOptions::default()) {
            Ok(ThemeMode::Dark) => Theme::dark(),
            Ok(ThemeMode::Light) => Theme::light(),
            _ => Theme::dark(),
        }
    }

    /// Resolve the theme to use when no explicit CLI/library option is given.
    ///
    /// Priority:
    /// 1. `CSVLENS_THEME` environment variable
    /// 2. `theme` key in the user config file (`~/.config/csvlens/config.toml`)
    /// 3. Built-in `auto` (terminal light/dark detection)
    pub fn load_preferred() -> CsvlensResult<Self> {
        if let Ok(spec) = std::env::var("CSVLENS_THEME") {
            let trimmed = spec.trim();
            if !trimmed.is_empty() {
                return Theme::resolve(trimmed);
            }
        }
        if let Some(spec) = read_config_theme()? {
            return Theme::resolve(&spec);
        }
        Ok(Theme::auto())
    }

    pub fn dark() -> Self {
        let gutter = Color::Rgb(131, 148, 150);
        let header = Color::Rgb(253, 151, 31);
        Theme {
            header,
            row_number: gutter,
            border: gutter,
            selected_foreground: Color::Rgb(192, 192, 192),
            selected_background: Color::Rgb(62, 61, 50),
            marked_foreground: Color::Rgb(220, 230, 255),
            marked_background: Color::Rgb(40, 50, 80),
            found: Color::Rgb(200, 0, 0),
            found_selected_background: Color::LightYellow,
            status: gutter,
            column_colors: vec![
                Color::Rgb(253, 151, 31),
                Color::Rgb(102, 217, 239),
                Color::Rgb(190, 132, 255),
                Color::Rgb(249, 38, 114),
                Color::Rgb(230, 219, 116),
            ],
        }
    }

    pub fn light() -> Self {
        let gutter = Color::Rgb(131, 148, 150);
        let header = Color::Rgb(207, 112, 0);
        Theme {
            header,
            row_number: gutter,
            border: gutter,
            selected_foreground: Color::Rgb(73, 72, 62),
            selected_background: Color::Rgb(230, 227, 196),
            marked_foreground: Color::Rgb(0, 40, 80),
            marked_background: Color::Rgb(220, 235, 255),
            found: Color::Rgb(200, 0, 0),
            found_selected_background: Color::LightYellow,
            status: gutter,
            column_colors: vec![
                Color::Rgb(207, 112, 0),
                Color::Rgb(0, 137, 179),
                Color::Rgb(104, 77, 153),
                Color::Rgb(249, 0, 90),
                Color::Rgb(153, 143, 47),
            ],
        }
    }

    /// Resolve a theme by name, path, or built-in identifier.
    ///
    /// Resolution order:
    /// 1. Built-in names: `auto`, `default`, `dark`, `light` (case-insensitive)
    /// 2. Path to a `.toml` theme file (absolute, relative, or `~`-prefixed)
    /// 3. Named theme file in the config themes directory
    ///    (`$CSVLENS_CONFIG_DIR/themes/<name>.toml`, or
    ///    `$XDG_CONFIG_HOME/csvlens/themes/<name>.toml`, or
    ///    `~/.config/csvlens/themes/<name>.toml`)
    pub fn resolve(spec: &str) -> CsvlensResult<Self> {
        let trimmed = spec.trim();
        if trimmed.is_empty() {
            return Ok(Theme::auto());
        }

        match trimmed.to_ascii_lowercase().as_str() {
            "auto" | "default" => return Ok(Theme::auto()),
            "dark" => return Ok(Theme::dark()),
            "light" => return Ok(Theme::light()),
            _ => {}
        }

        let as_path = expand_tilde(trimmed);
        if as_path.exists() {
            return Theme::from_file(&as_path);
        }

        // Allow omitting the .toml extension for named config themes.
        let name = trimmed
            .strip_suffix(".toml")
            .unwrap_or(trimmed)
            .trim_end_matches('/');
        if !name.is_empty() && !name.contains('/') && !name.contains('\\') {
            let config_path = themes_dir().join(format!("{name}.toml"));
            if config_path.exists() {
                return Theme::from_file(&config_path);
            }
        }

        Err(CsvlensError::ThemeNotFound(trimmed.to_string()))
    }

    /// Load a theme from a TOML file.
    pub fn from_file(path: impl AsRef<Path>) -> CsvlensResult<Self> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path).map_err(|e| {
            CsvlensError::ThemeLoadError(format!("{}: {e}", path.display()))
        })?;
        Theme::from_toml(&contents).map_err(|e| {
            CsvlensError::ThemeLoadError(format!("{}: {e}", path.display()))
        })
    }

    /// Parse a theme from TOML text. Missing fields fall back to the dark theme.
    pub fn from_toml(toml_str: &str) -> CsvlensResult<Self> {
        let def: ThemeFile = toml::from_str(toml_str)
            .map_err(|e| CsvlensError::ThemeLoadError(e.to_string()))?;
        def.into_theme()
    }
}

/// User config directory (`~/.config/csvlens` by default).
pub fn config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CSVLENS_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        return PathBuf::from(xdg).join("csvlens");
    }
    expand_tilde("~/.config/csvlens")
}

/// Directory where user-defined theme files are looked up by name.
pub fn themes_dir() -> PathBuf {
    config_dir().join("themes")
}

/// Path to the user config file (`config.toml`).
pub fn config_file() -> PathBuf {
    config_dir().join("config.toml")
}

/// Optional user preferences from `config.toml`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct UserConfig {
    /// Default theme name, path, or built-in (`auto` / `dark` / `light`).
    theme: Option<String>,
}

fn read_config_theme() -> CsvlensResult<Option<String>> {
    let path = config_file();
    if !path.exists() {
        return Ok(None);
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| {
        CsvlensError::ThemeLoadError(format!("config {}: {e}", path.display()))
    })?;
    let cfg: UserConfig = toml::from_str(&contents).map_err(|e| {
        CsvlensError::ThemeLoadError(format!("config {}: {e}", path.display()))
    })?;
    Ok(cfg.theme.filter(|s| !s.trim().is_empty()))
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    } else if path == "~"
        && let Some(home) = home_dir()
    {
        return home;
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// TOML representation of a theme file. All fields are optional and fall back
/// to the built-in dark theme when omitted.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeFile {
    #[serde(default)]
    name: Option<String>,
    header: Option<ThemeColor>,
    row_number: Option<ThemeColor>,
    border: Option<ThemeColor>,
    selected_foreground: Option<ThemeColor>,
    selected_background: Option<ThemeColor>,
    marked_foreground: Option<ThemeColor>,
    marked_background: Option<ThemeColor>,
    found: Option<ThemeColor>,
    found_selected_background: Option<ThemeColor>,
    status: Option<ThemeColor>,
    column_colors: Option<Vec<ThemeColor>>,
}

impl ThemeFile {
    fn into_theme(self) -> CsvlensResult<Theme> {
        let base = Theme::dark();
        let column_colors = if let Some(colors) = self.column_colors {
            if colors.is_empty() {
                return Err(CsvlensError::ThemeLoadError(
                    "column_colors must not be empty".into(),
                ));
            }
            colors.into_iter().map(|c| c.0).collect()
        } else {
            base.column_colors
        };

        // Suppress unused field warning for optional display name.
        let _ = self.name;

        Ok(Theme {
            header: self.header.map(|c| c.0).unwrap_or(base.header),
            row_number: self.row_number.map(|c| c.0).unwrap_or(base.row_number),
            border: self.border.map(|c| c.0).unwrap_or(base.border),
            selected_foreground: self
                .selected_foreground
                .map(|c| c.0)
                .unwrap_or(base.selected_foreground),
            selected_background: self
                .selected_background
                .map(|c| c.0)
                .unwrap_or(base.selected_background),
            marked_foreground: self
                .marked_foreground
                .map(|c| c.0)
                .unwrap_or(base.marked_foreground),
            marked_background: self
                .marked_background
                .map(|c| c.0)
                .unwrap_or(base.marked_background),
            found: self.found.map(|c| c.0).unwrap_or(base.found),
            found_selected_background: self
                .found_selected_background
                .map(|c| c.0)
                .unwrap_or(base.found_selected_background),
            status: self.status.map(|c| c.0).unwrap_or(base.status),
            column_colors,
        })
    }
}

/// Color value accepted in theme files.
///
/// Supported formats:
/// - Hex: `#rgb`, `#rrggbb`, `#rrggbbaa` (alpha ignored)
/// - `rgb(r, g, b)` / `rgba(r, g, b, a)` (alpha ignored)
/// - Named ANSI colors, e.g. `red`, `lightyellow`, `gray`
/// - Indexed: `color0` … `color255` or a bare integer `0`–`255`
#[derive(Debug, Clone)]
struct ThemeColor(Color);

impl<'de> Deserialize<'de> for ThemeColor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        ThemeColor::from_str(&s).map_err(serde::de::Error::custom)
    }
}

impl FromStr for ThemeColor {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_color(s).map(ThemeColor)
    }
}

fn parse_color(input: &str) -> Result<Color, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("empty color value".into());
    }

    if let Some(hex) = s.strip_prefix('#') {
        return parse_hex(hex);
    }

    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower
        .strip_prefix("rgba(")
        .or_else(|| lower.strip_prefix("rgb("))
        && let Some(inner) = rest.strip_suffix(')')
    {
        return parse_rgb_fn(inner);
    }

    if let Some(rest) = lower.strip_prefix("color")
        && let Ok(index) = rest.parse::<u8>()
    {
        return Ok(Color::Indexed(index));
    }

    if let Ok(index) = s.parse::<u8>() {
        return Ok(Color::Indexed(index));
    }

    named_color(&lower).ok_or_else(|| {
        format!(
            "unknown color '{s}'; use #hex, rgb(r,g,b), colorN, or a named ANSI color"
        )
    })
}

fn parse_hex(hex: &str) -> Result<Color, String> {
    let hex = hex.trim();
    let (r, g, b) = match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16)
                .map_err(|_| format!("invalid hex color '#{hex}'"))?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16)
                .map_err(|_| format!("invalid hex color '#{hex}'"))?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16)
                .map_err(|_| format!("invalid hex color '#{hex}'"))?;
            (r, g, b)
        }
        6 | 8 => {
            let r = u8::from_str_radix(&hex[0..2], 16)
                .map_err(|_| format!("invalid hex color '#{hex}'"))?;
            let g = u8::from_str_radix(&hex[2..4], 16)
                .map_err(|_| format!("invalid hex color '#{hex}'"))?;
            let b = u8::from_str_radix(&hex[4..6], 16)
                .map_err(|_| format!("invalid hex color '#{hex}'"))?;
            (r, g, b)
        }
        _ => {
            return Err(format!(
                "invalid hex color '#{hex}'; expected #rgb, #rrggbb, or #rrggbbaa"
            ));
        }
    };
    Ok(Color::Rgb(r, g, b))
}

fn parse_rgb_fn(inner: &str) -> Result<Color, String> {
    let parts: Vec<&str> = inner.split(',').map(str::trim).collect();
    if parts.len() < 3 {
        return Err(format!(
            "invalid rgb() color; expected at least 3 components, got {}",
            parts.len()
        ));
    }
    let r: u8 = parts[0]
        .parse()
        .map_err(|_| format!("invalid red component '{}'", parts[0]))?;
    let g: u8 = parts[1]
        .parse()
        .map_err(|_| format!("invalid green component '{}'", parts[1]))?;
    let b: u8 = parts[2]
        .parse()
        .map_err(|_| format!("invalid blue component '{}'", parts[2]))?;
    Ok(Color::Rgb(r, g, b))
}

fn named_color(name: &str) -> Option<Color> {
    Some(match name {
        "reset" | "default" => Color::Reset,
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn parse_hex_colors() {
        assert_eq!(parse_color("#f00").unwrap(), Color::Rgb(255, 0, 0));
        assert_eq!(parse_color("#00ff00").unwrap(), Color::Rgb(0, 255, 0));
        assert_eq!(
            parse_color("#0000ff80").unwrap(),
            Color::Rgb(0, 0, 255)
        );
    }

    #[test]
    fn parse_rgb_and_named() {
        assert_eq!(
            parse_color("rgb(1, 2, 3)").unwrap(),
            Color::Rgb(1, 2, 3)
        );
        assert_eq!(parse_color("LightYellow").unwrap(), Color::LightYellow);
        assert_eq!(parse_color("color42").unwrap(), Color::Indexed(42));
        assert_eq!(parse_color("7").unwrap(), Color::Indexed(7));
    }

    #[test]
    fn resolve_builtins() {
        assert_eq!(Theme::resolve("dark").unwrap(), Theme::dark());
        assert_eq!(Theme::resolve("LIGHT").unwrap(), Theme::light());
        Theme::resolve("auto").unwrap();
        Theme::resolve("").unwrap();
    }

    #[test]
    fn load_partial_toml_falls_back() {
        let theme = Theme::from_toml(
            r##"
            header = "#abcdef"
            found = "#ff0000"
            column_colors = ["#111111", "#222222"]
            "##,
        )
        .unwrap();
        assert_eq!(theme.header, Color::Rgb(0xab, 0xcd, 0xef));
        assert_eq!(theme.found, Color::Rgb(255, 0, 0));
        assert_eq!(theme.column_colors.len(), 2);
        assert_eq!(theme.border, Theme::dark().border);
    }

    #[test]
    fn load_from_file_and_resolve_path() {
        let mut file = NamedTempFile::with_suffix(".toml").unwrap();
        writeln!(
            file,
            r#"
            name = "test"
            status = "cyan"
            selected_background = "rgb(10, 20, 30)"
            "#
        )
        .unwrap();
        let theme = Theme::from_file(file.path()).unwrap();
        assert_eq!(theme.status, Color::Cyan);
        assert_eq!(theme.selected_background, Color::Rgb(10, 20, 30));
        // header falls back to dark default
        assert_eq!(theme.header, Theme::dark().header);

        let resolved = Theme::resolve(file.path().to_str().unwrap()).unwrap();
        assert_eq!(resolved, theme);
    }

    #[test]
    fn resolve_named_theme_from_config_dir() {
        let dir = tempfile::tempdir().unwrap();
        let themes = dir.path().join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        let theme_path = themes.join("nord.toml");
        std::fs::write(
            &theme_path,
            r##"
            header = "#88c0d0"
            border = "#2e3440"
            status = "#88c0d0"
            "##,
        )
        .unwrap();

        // SAFETY: test-only env mutation.
        unsafe {
            std::env::set_var("CSVLENS_CONFIG_DIR", dir.path());
            std::env::remove_var("CSVLENS_THEME");
        }
        let theme = Theme::resolve("nord").unwrap();
        assert_eq!(theme.header, Color::Rgb(0x88, 0xc0, 0xd0));
        assert_eq!(theme.border, Color::Rgb(0x2e, 0x34, 0x40));
        assert_eq!(theme.status, Color::Rgb(0x88, 0xc0, 0xd0));
        unsafe {
            std::env::remove_var("CSVLENS_CONFIG_DIR");
        }
    }

    #[test]
    fn load_preferred_from_config_file() {
        let dir = tempfile::tempdir().unwrap();
        let themes = dir.path().join("themes");
        std::fs::create_dir_all(&themes).unwrap();
        std::fs::write(
            themes.join("mine.toml"),
            r##"header = "#112233""##,
        )
        .unwrap();
        std::fs::write(dir.path().join("config.toml"), "theme = \"mine\"\n").unwrap();

        unsafe {
            std::env::set_var("CSVLENS_CONFIG_DIR", dir.path());
            std::env::remove_var("CSVLENS_THEME");
        }
        let theme = Theme::load_preferred().unwrap();
        assert_eq!(theme.header, Color::Rgb(0x11, 0x22, 0x33));
        unsafe {
            std::env::remove_var("CSVLENS_CONFIG_DIR");
        }
    }

    #[test]
    fn load_preferred_env_overrides_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("config.toml"), "theme = \"dark\"\n").unwrap();

        unsafe {
            std::env::set_var("CSVLENS_CONFIG_DIR", dir.path());
            std::env::set_var("CSVLENS_THEME", "light");
        }
        let theme = Theme::load_preferred().unwrap();
        assert_eq!(theme, Theme::light());
        unsafe {
            std::env::remove_var("CSVLENS_CONFIG_DIR");
            std::env::remove_var("CSVLENS_THEME");
        }
    }

    #[test]
    fn reject_unknown_fields_and_empty_columns() {
        assert!(Theme::from_toml("unknown = true").is_err());
        assert!(Theme::from_toml("column_colors = []").is_err());
        assert!(Theme::resolve("definitely-not-a-theme-xyz").is_err());
    }

    #[test]
    fn column_color_indexing_works_with_vec() {
        let theme = Theme::dark();
        let idx = 7 % theme.column_colors.len();
        let _ = theme.column_colors[idx];
    }
}
