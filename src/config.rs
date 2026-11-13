use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Playlist {
    pub name: String,
    pub songs: Vec<String>,
}

fn one_or_many<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Paths {
        One(String),
        Many(Vec<String>),
    }
    Ok(match Paths::deserialize(deserializer)? {
        Paths::One(path) => vec![path],
        Paths::Many(paths) => paths,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    // 旧版本只存单个 music_dir；别名让老配置自动迁移成列表。
    #[serde(alias = "music_dir", deserialize_with = "one_or_many")]
    pub music_dirs: Vec<String>,
    pub volume: f64,
    pub speed: f64,
    pub mode: String,
    pub sort: String,
    pub effect: String,
    pub lyrics: bool,
    pub time_remaining: bool,
    pub transparent: bool,
    pub favorites: Vec<String>,
    pub playlists: Vec<Playlist>,
    pub theme: String,
    pub eq_on: bool,
    pub eq_gains: Vec<f64>,
    pub audio_device: String,
    pub resume: bool,
    pub keys: HashMap<String, String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            music_dirs: Vec::new(),
            volume: 70.0,
            speed: 1.0,
            mode: "loop".into(),
            sort: "name".into(),
            effect: "原声".into(),
            lyrics: true,
            time_remaining: false,
            transparent: false,
            favorites: Vec::new(),
            playlists: Vec::new(),
            theme: "mocha".into(),
            eq_on: false,
            eq_gains: vec![0.0; 10],
            audio_device: String::new(),
            resume: true,
            keys: HashMap::new(),
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

impl Playlist {
    pub fn add(&mut self, path: &Path) -> bool {
        let value = path.to_string_lossy().to_string();
        if self.songs.contains(&value) {
            return false;
        }
        self.songs.push(value);
        true
    }

    pub fn remove(&mut self, index: usize) -> Option<String> {
        (index < self.songs.len()).then(|| self.songs.remove(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn music_dirs_accept_legacy_single_path() {
        let legacy: Config = serde_json::from_str(r#"{"music_dir":"/old/music"}"#).unwrap();
        assert_eq!(legacy.music_dirs, vec!["/old/music"]);
        let many: Config = serde_json::from_str(r#"{"music_dirs":["/a","/b"]}"#).unwrap();
        assert_eq!(many.music_dirs, vec!["/a", "/b"]);
        let empty: Config = serde_json::from_str("{}").unwrap();
        assert!(empty.music_dirs.is_empty());
    }

    #[test]
    fn playlist_adds_once_and_removes() {
        let mut list = Playlist::default();
        assert!(list.add(Path::new("/a.mp3")));
        assert!(!list.add(Path::new("/a.mp3")));
        assert_eq!(list.remove(0).as_deref(), Some("/a.mp3"));
        assert!(list.songs.is_empty());
        assert!(list.remove(0).is_none());
    }
}
