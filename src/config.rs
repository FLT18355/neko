use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub music_dir: String,
    pub volume: f64,
    pub speed: f64,
    pub mode: String,
    pub sort: String,
    pub effect: String,
    pub lyrics: bool,
    pub time_remaining: bool,
    pub transparent: bool,
    pub favorites: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_dir: String::new(),
            volume: 70.0,
            speed: 1.0,
            mode: "loop".into(),
            sort: "name".into(),
            effect: "原声".into(),
            lyrics: true,
            time_remaining: false,
            transparent: false,
            favorites: Vec::new(),
        }
    }
}

pub fn home_dir() -> PathBuf {
    env::var_os("USERPROFILE")
        .or_else(|| env::var_os("HOME"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn config_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CONFIG_HOME")
        && !xdg.is_empty()
    {
        return Path::new(&xdg).join("neko").join("config.json");
    }
    #[cfg(windows)]
    if let Some(appdata) = env::var_os("APPDATA") {
        return PathBuf::from(appdata).join("neko").join("config.json");
    }
    home_dir().join(".config").join("neko").join("config.json")
}

pub fn expand_path(raw: &str) -> PathBuf {
    if raw == "~" {
        return home_dir();
    }
    if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        return home_dir().join(rest);
    }
    PathBuf::from(raw)
}

pub fn runtime_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .filter(|path| path.is_dir())
        .unwrap_or_else(env::temp_dir)
}

pub fn default_music_dir() -> PathBuf {
    let home = home_dir();
    for name in ["音乐", "Music"] {
        let dir = home.join(name);
        if dir.is_dir() {
            return dir;
        }
    }
    home
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        let Ok(text) = fs::read_to_string(&path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)?;
        fs::write(path, text)
    }

    pub fn toggle_favorite(&mut self, path: &Path) -> bool {
        let value = path.to_string_lossy().to_string();
        if let Some(pos) = self.favorites.iter().position(|p| p == &value) {
            self.favorites.remove(pos);
            false
        } else {
            self.favorites.push(value);
            true
        }
    }

    pub fn is_favorite(&self, path: &Path) -> bool {
        let value = path.to_string_lossy();
        self.favorites.iter().any(|p| p == value.as_ref())
    }
}
