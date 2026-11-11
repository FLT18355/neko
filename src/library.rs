use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "m4a", "aac", "ogg", "opus", "wav", "wma", "ape", "alac", "aiff", "aif", "mka",
    "webm", "mp4",
];

#[derive(Clone, Debug)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub modified: u64,
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| AUDIO_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn scan(dir: &Path) -> Vec<Track> {
    let mut tracks = Vec::new();
    walk(dir, &mut tracks);
    tracks
}

fn walk(dir: &Path, out: &mut Vec<Track>) {
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
            walk(&path, out);
        } else if kind.is_file() && is_audio(&path) {
            let modified = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            out.push(Track {
                title: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.to_string_lossy().to_string()),
                path,
                modified,
            });
        }
    }
}

pub fn sort_tracks(tracks: &mut [Track], mode: &str) {
    match mode {
        "path" => tracks.sort_by_key(|t| t.path.to_string_lossy().to_lowercase()),
        "modified" => tracks.sort_by_key(|track| std::cmp::Reverse(track.modified)),
        _ => tracks.sort_by_key(|t| t.title.to_lowercase()),
    }
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

pub fn folders(tracks: &[Track], root: &Path) -> Vec<(String, usize)> {
    let mut map: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for track in tracks {
        let folder = track
            .path
            .parent()
            .and_then(|parent| parent.strip_prefix(root).ok())
            .map(|path| path.to_string_lossy().to_string())
            .unwrap_or_default();
        *map.entry(folder).or_insert(0) += 1;
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

    #[test]
    fn scans_nested_audio_and_ignores_other_files() {
        let dir = temp_dir("scan");
        fs::create_dir_all(dir.join("子目录")).unwrap();
        fs::write(dir.join("a.mp3"), b"x").unwrap();
        fs::write(dir.join("子目录/b.FLAC"), b"x").unwrap();
        fs::write(dir.join("cover.jpg"), b"x").unwrap();
        fs::write(dir.join(".hidden.mp3"), b"x").unwrap();
        let tracks = scan(&dir);
        assert_eq!(tracks.len(), 2);
        assert!(tracks.iter().any(|t| t.title == "b"));
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn writes_utf8_m3u() {
        let dir = temp_dir("m3u");
        let track = Track {
            path: dir.join("夜空中最亮的星.m4a"),
            title: "夜空中最亮的星".into(),
            modified: 0,
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
        let root = Path::new("/music");
        let track = |path: &str| Track {
            path: PathBuf::from(path),
            title: String::new(),
            modified: 0,
        };
        let tracks = [
            track("/music/a.mp3"),
            track("/music/rock/b.mp3"),
            track("/music/rock/c.mp3"),
            track("/music/jazz/d.mp3"),
        ];
        let folders = folders(&tracks, root);
        assert_eq!(
            folders,
            vec![
                ("".to_string(), 1),
                ("jazz".to_string(), 1),
                ("rock".to_string(), 2)
            ]
        );
    }
}
