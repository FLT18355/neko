use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Cache {
    pub recent: Vec<String>,
    pub last_path: Option<String>,
    pub last_position: f64,
    pub view: String,
    pub filter: String,
    pub folder: String,
    pub selected_path: Option<String>,
    pub plays: HashMap<String, u64>,
    pub ratings: HashMap<String, u8>,
    pub positions: HashMap<String, f64>,
    pub listen_seconds: HashMap<String, f64>,
    pub total_seconds: f64,
}

pub fn cache_path() -> PathBuf {
    if let Ok(xdg) = env::var("XDG_CACHE_HOME")
        && !xdg.is_empty()
    {
        return Path::new(&xdg).join("neko").join("cache.json");
    }
    #[cfg(windows)]
    if let Some(local) = env::var_os("LOCALAPPDATA").or_else(|| env::var_os("APPDATA")) {
        return PathBuf::from(local).join("neko").join("cache.json");
    }
    crate::config::home_dir()
        .join(".cache")
        .join("neko")
        .join("cache.json")
}

impl Cache {
    pub fn load() -> Self {
        let Ok(text) = fs::read_to_string(cache_path()) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = cache_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(self)?)
    }

    pub fn add_recent(&mut self, path: &Path) {
        let value = path.to_string_lossy().to_string();
        self.recent.retain(|item| item != &value);
        self.recent.insert(0, value);
        self.recent.truncate(100);
    }

    pub fn update_last(&mut self, path: &Path, position: f64) {
        self.last_path = Some(path.to_string_lossy().to_string());
        self.last_position = position.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_is_deduplicated_and_capped() {
        let mut cache = Cache::default();
        for index in 0..105 {
            cache.add_recent(Path::new(&format!("/music/{index}.mp3")));
        }
        assert_eq!(cache.recent.len(), 100);
        assert!(cache.recent[0].ends_with("104.mp3"));
        cache.add_recent(Path::new("/music/50.mp3"));
        assert!(cache.recent[0].ends_with("50.mp3"));
        assert_eq!(
            cache
                .recent
                .iter()
                .filter(|item| *item == "/music/50.mp3")
                .count(),
            1
        );
    }
}
