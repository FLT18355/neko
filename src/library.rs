use lofty::config::ParseOptions;
use lofty::prelude::*;
use lofty::probe::Probe;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "aac", "ogg", "opus", "wav", "wma", "ape", "alac", "aiff", "aif", "mka",
    "webm", "mp4",
];

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Meta {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub genre: String,
    pub year: u32,
    pub track: u32,
    pub duration: f64,
}

impl Meta {
    /// 同名专辑按歌手分开，避免不同歌手的《精选》混在一起。
    pub fn album_key(&self) -> String {
        format!("{}\u{1f}{}", self.artist, self.album)
    }

    pub fn album_label(&self) -> String {
        if self.artist == "未知歌手" || self.artist.is_empty() {
            self.album.clone()
        } else {
            format!("{} · {}", self.album, self.artist)
        }
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub modified: u64,
    pub meta: Meta,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
struct IndexEntry {
    modified: u64,
    size: u64,
    meta: Meta,
}

/// 标签读取缓存。首次扫描读文件，之后按修改时间和大小复用。
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Index {
    entries: HashMap<String, IndexEntry>,
}

pub fn index_path() -> PathBuf {
    crate::cache::cache_path().with_file_name("index.json")
}

impl Index {
    pub fn load() -> Self {
        let Ok(text) = fs::read_to_string(index_path()) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> io::Result<()> {
        let path = index_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string(self)?)
    }
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn scan_dirs(dirs: &[PathBuf], index: &mut Index) -> Vec<Track> {
    let mut tracks = Vec::new();
    let mut seen = HashSet::new();
    for dir in dirs {
        walk(dir, index, &mut tracks, &mut seen);
    }
    // 只清理本次扫描根目录内的失效条目，避免 `neko cli 某个目录` 清空整个索引。
    index.entries.retain(|path, _| {
        seen.contains(path) || !dirs.iter().any(|root| Path::new(path).starts_with(root))
    });
    tracks
}

fn walk(dir: &Path, index: &mut Index, out: &mut Vec<Track>, seen: &mut HashSet<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            walk(&path, index, out, seen);
        } else if kind.is_file() && is_audio(&path) {
            let key = path.to_string_lossy().to_string();
            if !seen.insert(key.clone()) {
                continue;
            }
            let metadata = entry.metadata().ok();
            let modified = metadata
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
            let meta = match index.entries.get(&key) {
                Some(cached) if cached.modified == modified && cached.size == size => {
                    cached.meta.clone()
                }
                _ => {
                    let meta = read_meta(&path);
                    index.entries.insert(
                        key,
                        IndexEntry {
                            modified,
                            size,
                            meta: meta.clone(),
                        },
                    );
                    meta
                }
            };
            out.push(Track {
                path,
                title: meta.title.clone(),
                modified,
                meta,
            });
        }
    }
}

fn read_meta(path: &Path) -> Meta {
    let mut meta = Meta::default();
    let parsed = Probe::open(path).and_then(|probe| {
        probe
            .options(ParseOptions::new().read_cover_art(false))
            .read()
    });
    if let Ok(tagged) = parsed {
        let props = tagged.properties();
        meta.duration = props.duration().as_secs_f64();
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            meta.title = tag
                .title()
                .map(|value| value.into_owned())
                .unwrap_or_default();
            meta.artist = tag
                .artist()
                .map(|value| value.into_owned())
                .unwrap_or_default();
            meta.album = tag
                .album()
                .map(|value| value.into_owned())
                .unwrap_or_default();
            meta.genre = tag
                .genre()
                .map(|value| value.into_owned())
                .unwrap_or_default();
            meta.year = tag.date().map(|date| u32::from(date.year)).unwrap_or(0);
            meta.track = tag.track().unwrap_or(0);
        }
    }
    if meta.title.trim().is_empty() {
        meta.title = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
    }
    if meta.artist.trim().is_empty() {
        meta.artist = "未知歌手".into();
    }
    if meta.album.trim().is_empty() {
        meta.album = path
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "未知专辑".into());
    }
    if meta.genre.trim().is_empty() {
        meta.genre = "未分类".into();
    }
    meta
}

pub fn sort_tracks(tracks: &mut [Track], mode: &str) {
    match mode {
        "path" => tracks.sort_by_key(|t| t.path.to_string_lossy().to_lowercase()),
        "modified" => tracks.sort_by_key(|track| std::cmp::Reverse(track.modified)),
        _ => tracks.sort_by_key(|t| t.title.to_lowercase()),
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    pub key: String,
    pub label: String,
    pub count: usize,
    pub duration: f64,
}

/// 按专辑 / 歌手 / 流派分组，按名称排序。
pub fn groups(tracks: &[Track], by: &str) -> Vec<Group> {
    let mut map: BTreeMap<String, (String, usize, f64)> = BTreeMap::new();
    for track in tracks {
        let (key, label) = match by {
            "artist" => (track.meta.artist.clone(), track.meta.artist.clone()),
            "genre" => (track.meta.genre.clone(), track.meta.genre.clone()),
            _ => (track.meta.album_key(), track.meta.album_label()),
        };
        let entry = map.entry(key).or_insert((label, 0, 0.0));
        entry.1 += 1;
        entry.2 += track.meta.duration;
    }
    map.into_iter()
        .map(|(key, (label, count, duration))| Group {
            key,
            label,
            count,
            duration,
        })
        .collect()
}

/// 智能电台的相似度：同歌手、同专辑、同流派、年代和时长越接近越高。
pub fn similarity(a: &Track, b: &Track) -> f64 {
    let mut score = 0.0;
    if a.meta.artist == b.meta.artist && a.meta.artist != "未知歌手" {
        score += 5.0;
    }
    if a.meta.album_key() == b.meta.album_key() && a.meta.album != "未知专辑" {
        score += 3.0;
    }
    if a.meta.genre == b.meta.genre && a.meta.genre != "未分类" {
        score += 1.5;
    }
    if a.meta.year > 0 && b.meta.year > 0 {
        let diff = a.meta.year.abs_diff(b.meta.year) as f64;
        score += (2.0 - diff * 0.4).max(0.0);
    }
    if a.meta.duration > 0.0 && b.meta.duration > 0.0 {
        let diff = (a.meta.duration - b.meta.duration).abs();
        if diff <= 30.0 {
            score += 0.6;
        } else if diff <= 90.0 {
            score += 0.2;
        }
    }
    score
}

pub fn write_m3u(path: &Path, tracks: &[Track]) -> io::Result<()> {
    let mut text = String::from("#EXTM3U\n");
    for track in tracks {
        text.push_str(&track.path.to_string_lossy());
        text.push('\n');
    }
    fs::write(path, text)
}

pub fn duration_text(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "--:--".into();
    }
    let total = seconds as u64;
    let (h, m, s) = (total / 3600, total % 3600 / 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    }
}

pub fn human_duration(seconds: f64) -> String {
    let total = seconds.max(0.0) as u64;
    if total >= 3600 {
        format!("{} 小时 {} 分", total / 3600, total % 3600 / 60)
    } else if total >= 60 {
        format!("{} 分 {} 秒", total / 60, total % 60)
    } else {
        format!("{total} 秒")
    }
}

/// 目录相对于哪个音乐根目录：取最长的匹配根，避免嵌套目录归组到父级。
pub fn relative_folder(path: &Path, roots: &[PathBuf]) -> String {
    let Some(parent) = path.parent() else {
        return String::new();
    };
    roots
        .iter()
        .filter(|root| parent.starts_with(root))
        .max_by_key(|root| root.as_os_str().len())
        .and_then(|root| parent.strip_prefix(root).ok())
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default()
}

pub fn folders(tracks: &[Track], roots: &[PathBuf]) -> Vec<(String, usize)> {
    let mut map: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for track in tracks {
        *map.entry(relative_folder(&track.path, roots)).or_insert(0) += 1;
    }
    map.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("neko-test-{name}-{}-{stamp}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn track(path: &str, artist: &str, album: &str, genre: &str, duration: f64) -> Track {
        let meta = Meta {
            title: path.to_string(),
            artist: artist.into(),
            album: album.into(),
            genre: genre.into(),
            duration,
            ..Meta::default()
        };
        Track {
            path: PathBuf::from(path),
            title: meta.title.clone(),
            modified: 0,
            meta,
        }
    }

    #[test]
    fn scans_nested_audio_and_ignores_other_files() {
        let dir = temp_dir("scan");
        fs::create_dir_all(dir.join("子目录")).unwrap();
        fs::write(dir.join("a.mp3"), b"x").unwrap();
        fs::write(dir.join("子目录/b.FLAC"), b"x").unwrap();
        fs::write(dir.join("cover.jpg"), b"x").unwrap();
        fs::write(dir.join(".hidden.mp3"), b"x").unwrap();
        let mut index = Index::default();
        let tracks = scan_dirs(std::slice::from_ref(&dir), &mut index);
        assert_eq!(tracks.len(), 2);
        let nested_track = tracks.iter().find(|t| t.title == "b").unwrap();
        assert_eq!(nested_track.meta.album, "子目录");
        let nested = scan_dirs(&[dir.clone(), dir.join("子目录")], &mut index);
        assert_eq!(nested.len(), 2, "重叠目录不应重复收录");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn writes_utf8_m3u() {
        let dir = temp_dir("m3u");
        let track = Track {
            path: dir.join("夜空中最亮的星.m4a"),
            title: "夜空中最亮的星".into(),
            modified: 0,
            meta: Meta {
                title: "夜空中最亮的星".into(),
                ..Meta::default()
            },
        };
        let list = dir.join("list.m3u");
        write_m3u(&list, &[track]).unwrap();
        let text = fs::read_to_string(&list).unwrap();
        assert!(text.starts_with("#EXTM3U\n"));
        assert!(text.contains("夜空中最亮的星.m4a"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn formats_durations() {
        assert_eq!(duration_text(83.2), "01:23");
        assert_eq!(duration_text(3723.0), "1:02:03");
        assert_eq!(duration_text(f64::NAN), "--:--");
        assert_eq!(human_duration(45.0), "45 秒");
        assert_eq!(human_duration(125.0), "2 分 5 秒");
        assert_eq!(human_duration(7325.0), "2 小时 2 分");
    }

    #[test]
    fn groups_tracks_by_relative_folder() {
        let roots = [PathBuf::from("/music")];
        let track = |path: &str| Track {
            path: PathBuf::from(path),
            title: String::new(),
            modified: 0,
            meta: Meta::default(),
        };
        let tracks = [
            track("/music/a.mp3"),
            track("/music/rock/b.mp3"),
            track("/music/rock/c.mp3"),
            track("/music/jazz/d.mp3"),
        ];
        let folders = folders(&tracks, &roots);
        assert_eq!(
            folders,
            vec![
                ("".to_string(), 1),
                ("jazz".to_string(), 1),
                ("rock".to_string(), 2)
            ]
        );
    }

    #[test]
    fn groups_albums_by_artist_and_album() {
        let tracks = [
            track("/a/1.mp3", "A", "同名专辑", "rock", 180.0),
            track("/a/2.mp3", "A", "同名专辑", "rock", 200.0),
            track("/b/3.mp3", "B", "同名专辑", "pop", 120.0),
            track("/b/4.mp3", "B", "另一张", "pop", 100.0),
        ];
        let albums = groups(&tracks, "album");
        assert_eq!(albums.len(), 3);
        assert_eq!(albums[0].label, "同名专辑 · A");
        assert_eq!(albums[0].count, 2);
        assert_eq!(albums[0].duration, 380.0);
        assert_eq!(groups(&tracks, "artist").len(), 2);
        assert_eq!(groups(&tracks, "genre").len(), 2);
    }

    #[test]
    fn similarity_prefers_same_artist_and_genre() {
        let current = track("/a/1.mp3", "A", "专辑", "rock", 180.0);
        let same = track("/a/2.mp3", "A", "专辑", "rock", 185.0);
        let other = track("/b/1.mp3", "B", "别的", "pop", 300.0);
        assert!(similarity(&current, &same) > similarity(&current, &other));
        assert_eq!(similarity(&current, &other), 0.0);
    }

    #[test]
    fn relative_folder_prefers_nested_root() {
        let roots = [PathBuf::from("/music"), PathBuf::from("/music/rock")];
        let path = PathBuf::from("/music/rock/album/a.mp3");
        assert_eq!(relative_folder(&path, &roots), "album");
    }
}
