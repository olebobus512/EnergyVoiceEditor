use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

pub const LANGUAGES: [&str; 4] = ["en", "ru", "ja", "zh"];
pub const PROJECT_EXTENSION: &str = ".eveproj";
pub const PRESETS: [&str; 4] = ["concept", "light", "ocean", "custom"];
pub const COLOR_KEYS: [&str; 8] = [
    "background",
    "panel",
    "bar",
    "barText",
    "button",
    "buttonText",
    "text",
    "accent",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Theme {
    pub preset: String,
    pub colors: BTreeMap<String, String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Workspace {
    pub track_panel: bool,
    pub piano_roll: bool,
    pub terminal: bool,
    pub px_per_sec: f64,
    pub track_panel_width: f64,
    pub bottom_height: f64,
    pub terminal_width: f64,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            track_panel: true,
            piano_roll: true,
            terminal: true,
            px_per_sec: 60.0,
            track_panel_width: 300.0,
            bottom_height: 420.0,
            terminal_width: 511.0,
        }
    }
}

pub const MAX_RECENT: usize = 10;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub language: String,
    pub theme: Theme,
    pub workspace: Workspace,
    pub recent_projects: Vec<String>,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            preset: "concept".into(),
            colors: default_colors(),
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: "en".into(),
            theme: Theme::default(),
            workspace: Workspace::default(),
            recent_projects: Vec::new(),
        }
    }
}

fn default_colors() -> BTreeMap<String, String> {
    [
        ("background", "#101010"),
        ("panel", "#181818"),
        ("bar", "#252525"),
        ("barText", "#EEEEEE"),
        ("button", "#292929"),
        ("buttonText", "#EEEEEE"),
        ("text", "#EEEEEE"),
        ("accent", "#F2CD65"),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value.to_string()))
    .collect()
}

fn is_hex_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].chars().all(|c| c.is_ascii_hexdigit())
}

impl Settings {
    /// Rejects unknown languages, presets, colour keys and malformed colours.
    /// Missing colour keys are filled from the default theme.
    pub fn validate(self) -> Result<Self, String> {
        if !LANGUAGES.contains(&self.language.as_str()) {
            return Err(format!("Unsupported language: {}", self.language));
        }
        if !PRESETS.contains(&self.theme.preset.as_str()) {
            return Err(format!("Unknown theme preset: {}", self.theme.preset));
        }
        if let Some(key) = self.theme.colors.keys().find(|key| !COLOR_KEYS.contains(&key.as_str())) {
            return Err(format!("Unknown colour key: {key}"));
        }
        let defaults = default_colors();
        let mut colors = BTreeMap::new();
        for key in COLOR_KEYS {
            let value = self
                .theme
                .colors
                .get(key)
                .cloned()
                .unwrap_or_else(|| defaults[key].clone());
            if !is_hex_color(&value) {
                return Err(format!("Invalid colour for {key}: {value}"));
            }
            colors.insert(key.to_string(), value);
        }
        if !(10.0..=400.0).contains(&self.workspace.px_per_sec) {
            return Err("Timeline zoom must be between 10 and 400 pixels per second".into());
        }
        if !(160.0..=600.0).contains(&self.workspace.track_panel_width)
            || !(160.0..=1200.0).contains(&self.workspace.bottom_height)
            || !(200.0..=1200.0).contains(&self.workspace.terminal_width)
        {
            return Err("Panel sizes are outside the allowed range".into());
        }
        let mut recent_projects: Vec<String> = Vec::new();
        for path in self.recent_projects {
            if path.ends_with(PROJECT_EXTENSION) && !recent_projects.contains(&path) {
                recent_projects.push(path);
            }
        }
        recent_projects.truncate(MAX_RECENT);
        Ok(Self {
            language: self.language,
            theme: Theme {
                preset: self.theme.preset,
                colors,
            },
            workspace: self.workspace,
            recent_projects,
        })
    }
}

pub fn load_from(path: &Path) -> Result<Settings, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => {
            let mut settings: Settings = serde_json::from_str(&text).map_err(|e| format!("Invalid settings file: {e}"))?;
            let colors = &settings.theme.colors;
            let old_concept = colors.get("panel").is_some_and(|c| c == "#000000") && colors.get("accent").is_some_and(|c| c == "#FFFFFF");
            let old_blue = colors.get("panel").is_some_and(|c| c == "#151822") && colors.get("accent").is_some_and(|c| c == "#ffd166");
            if settings.theme.preset == "concept" && (old_concept || old_blue) { settings.theme.colors = default_colors(); }
            settings.validate()
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(format!("Cannot read settings: {e}")),
    }
}

pub fn save_to(path: &Path, settings: Settings) -> Result<Settings, String> {
    let valid = settings.validate()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create settings directory: {e}"))?;
    }
    let text = serde_json::to_string_pretty(&valid).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| format!("Cannot write settings: {e}"))?;
    Ok(valid)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("eve-settings-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir.join("settings.json")
    }

    #[test]
    fn defaults_are_valid() {
        assert!(Settings::default().validate().is_ok());
    }

    #[test]
    fn rejects_bad_language_preset_and_colours() {
        let mut settings = Settings::default();
        settings.language = "fr".into();
        assert!(settings.validate().is_err());

        let mut settings = Settings::default();
        settings.theme.preset = "neon".into();
        assert!(settings.validate().is_err());

        let mut settings = Settings::default();
        settings.theme.colors.insert("panel".into(), "red".into());
        assert!(settings.validate().is_err());

        let mut settings = Settings::default();
        settings.theme.colors.insert("unknown".into(), "#000000".into());
        assert!(settings.validate().is_err());
    }

    #[test]
    fn fills_missing_colour_keys_from_defaults() {
        let mut settings = Settings::default();
        settings.theme.colors.remove("accent");
        let valid = settings.validate().unwrap();
        assert_eq!(valid.theme.colors["accent"], "#F2CD65");
    }

    #[test]
    fn missing_file_returns_defaults_and_save_round_trips() {
        let path = temp_file("roundtrip");
        assert_eq!(load_from(&path).unwrap(), Settings::default());

        let mut settings = Settings::default();
        settings.language = "ja".into();
        settings.theme.preset = "custom".into();
        settings.theme.colors.insert("bar".into(), "#123456".into());
        let saved = save_to(&path, settings).unwrap();
        assert_eq!(load_from(&path).unwrap(), saved);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn old_settings_with_track_colours_still_load() {
        let path = temp_file("old-track-colours");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(
            &path,
            r##"{"language":"ru","theme":{"preset":"concept","trackColors":["#FF0000","#1EFF00","#004AFF"]}}"##,
        )
        .unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.language, "ru");
        assert_eq!(loaded.theme.colors["accent"], "#F2CD65");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn corrupt_file_is_reported() {
        let path = temp_file("corrupt");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "{ not json").unwrap();
        assert!(load_from(&path).is_err());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
