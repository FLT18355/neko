mod actions;

use crate::cache::Cache;
use crate::config::{self, Config};
use crate::keymap::{self, Action};
use crate::library::{self, Track};
use crate::lyrics::Lrc;
use crate::mpv::Mpv;
use crate::theme;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

pub(crate) const MODES: [&str; 4] = ["order", "loop", "single", "shuffle"];
pub(crate) const EQ_BANDS: [f64; 10] = [
    31.0, 63.0, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
pub(crate) const EFFECTS: [(&str, &str); 4] = [
    ("原声", ""),
    (
        "低音增强",
        "lavfi=[equalizer=f=100:width_type=o:width=1:g=8,equalizer=f=200:width_type=o:width=1:g=4]",
    ),
    (
        "人声增强",
        "lavfi=[equalizer=f=2500:width_type=o:width=1:g=5,equalizer=f=300:width_type=o:width=1:g=-3]",
    ),
    ("柔和", "lavfi=[equalizer=f=8000:width_type=o:width=1:g=-4]"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Modal {
    Help,
    Queue,
    Info,
    Search,
    Directory,
    DirAdd,
    Seek,
    Settings,
    Folders,
    Browse,
    Playlists,
    PlaylistTracks,
    PlaylistName,
    Equalizer,
    AudioDevice,
    Stats,
    Keybindings,
}

pub struct App {
    pub(crate) mpv: Mpv,
    pub(crate) cfg: Config,
    pub(crate) cache: Cache,
    pub(crate) tracks: Vec<Track>,
    pub(crate) visible: Vec<usize>,
    pub(crate) view: String,
    pub(crate) filter: String,
    pub(crate) session: Vec<Track>,
    pub(crate) mode: String,
    pub(crate) sort: String,
    pub(crate) lrc: Option<Lrc>,
    pub(crate) lrc_path: Option<PathBuf>,
    pub(crate) lrc_offset: f64,
    pub(crate) lyrics_full: bool,
    pub(crate) a_point: Option<f64>,
    pub(crate) b_point: Option<f64>,
    pub(crate) sleep_at: Option<Instant>,
    pub(crate) sleep_minutes: u64,
    pub(crate) sleep_volume: Option<f64>,
    pub(crate) remaining: bool,
    pub(crate) transparent: bool,
    pub(crate) search_original: Option<String>,
    pub(crate) selected: usize,
    pub(crate) toast: Option<(String, Instant)>,
    pub(crate) modal: Option<Modal>,
    pub(crate) input: String,
    pub(crate) modal_scroll: usize,
    pub(crate) setting_selected: usize,
    pub(crate) quit: bool,
    pub(crate) last_path: Option<PathBuf>,
    pub(crate) last_skip: Option<Instant>,
    pub(crate) last_position: f64,
    pub(crate) last_duration: Option<f64>,
    pub(crate) stat_at: Instant,
    pub(crate) folder: String,
    pub(crate) folder_selected: usize,
    pub(crate) dir_selected: usize,
    pub(crate) group: String,
    pub(crate) browse_selected: usize,
    pub(crate) browse_artist: bool,
    pub(crate) playlists: Vec<crate::config::Playlist>,
    pub(crate) playlist_selected: usize,
    pub(crate) playlist_song_selected: usize,
    pub(crate) queue_selected: usize,
    pub(crate) playlist_name_target: Option<usize>,
    pub(crate) eq_selected: usize,
    pub(crate) device_selected: usize,
    pub(crate) devices: Vec<(String, String)>,
    pub(crate) stats_scroll: usize,
    pub(crate) keybind_selected: usize,
    pub(crate) capture_key: bool,
    pub(crate) bindings: HashMap<String, Action>,
}

fn fade_volume(base: f64, remaining: f64) -> f64 {
    (base * (remaining / 10.0).clamp(0.0, 1.0)).max(0.0)
}

pub(crate) fn eq_graph(gains: &[f64]) -> String {
    let mut parts = Vec::new();
    for (band, gain) in EQ_BANDS.iter().zip(gains.iter()) {
        let gain = gain.clamp(-12.0, 12.0);
        if gain.abs() >= 0.05 {
            parts.push(format!(
                "equalizer=f={band}:width_type=o:width=1:g={gain:.1}"
            ));
        }
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("lavfi=[{}]", parts.join(","))
    }
}

fn resume_target(position: f64, duration: Option<f64>) -> Option<f64> {
    if !position.is_finite() || position < 3.0 {
        return None;
    }
    if let Some(duration) = duration
        && duration > 0.0
        && position > duration - 3.0
    {
        return None;
    }
    Some(position)
}

pub(crate) fn cycle_rating(current: u8) -> u8 {
    if current >= 5 { 0 } else { current + 1 }
}

fn build_bindings(overrides: &HashMap<String, String>) -> HashMap<String, Action> {
    let mut map = HashMap::new();
    for action in keymap::ACTIONS {
        match overrides.get(action.id()) {
            Some(key) if !key.is_empty() => {
                map.insert(key.clone(), action);
            }
            Some(_) => {}
            None => {
                for key in keymap::default_keys(action) {
                    map.insert(key.to_string(), action);
                }
            }
        }
    }
    map
}

impl App {
    pub fn new(
        mut cfg: Config,
        cache: Cache,
        mut mpv: Mpv,
        mut tracks: Vec<Track>,
        dirs: Vec<PathBuf>,
    ) -> Self {
        cfg.music_dirs = dirs
            .iter()
            .map(|dir| dir.to_string_lossy().to_string())
            .collect();
        if !MODES.contains(&cfg.mode.as_str()) {
            cfg.mode = "loop".into();
        }
        if !matches!(cfg.sort.as_str(), "name" | "path" | "modified") {
            cfg.sort = "name".into();
        }
        library::sort_tracks(&mut tracks, &cfg.sort);
        let mode = cfg.mode.clone();
        let sort = cfg.sort.clone();
        let remaining = cfg.time_remaining;
        let transparent = cfg.transparent;
        cfg.theme = theme::set(&cfg.theme).to_string();
        cfg.eq_gains.truncate(EQ_BANDS.len());
        cfg.eq_gains.resize(EQ_BANDS.len(), 0.0);
        let cached_view = if ["all", "fav", "recent", "folder", "album", "artist"]
            .contains(&cache.view.as_str())
        {
            cache.view.clone()
        } else {
            "all".into()
        };
        let cached_filter = cache.filter.clone();
        let cached_folder = cache.folder.clone();
        let cached_group = cache.group.clone();
        let cached_queue = cache.queue.clone();
        let cached_queue_pos = cache.queue_pos;
        let cached_position = cache.last_position;
        let cached_selected = cache.selected_path.clone();
        let cached_last = cache.last_path.clone();
        let bindings = build_bindings(&cfg.keys);
        let playlists = cfg.playlists.clone();
        let _ = mpv.apply_mode(&mode);
        // 每次启动都从 1.0x 开始，避免上次调速残留。
        cfg.speed = 1.0;
        let _ = mpv.set_prop("speed", json!(1.0));
        let effect = EFFECTS
            .iter()
            .find(|(name, _)| *name == cfg.effect)
            .copied()
            .unwrap_or(EFFECTS[0]);
        let graph = if cfg.eq_on {
            eq_graph(&cfg.eq_gains)
        } else {
            effect.1.to_string()
        };
        let _ = mpv.set_af(&graph);
        if !cfg.audio_device.is_empty() {
            let _ = mpv.set_prop("audio-device", json!(cfg.audio_device));
        }

        let mut app = Self {
            mpv,
            cfg,
            cache,
            tracks,
            visible: Vec::new(),
            view: cached_view,
            filter: cached_filter,
            session: Vec::new(),
            mode,
            sort,
            lrc: None,
            lrc_path: None,
            lrc_offset: 0.0,
            lyrics_full: false,
            a_point: None,
            b_point: None,
            sleep_at: None,
            sleep_minutes: 0,
            sleep_volume: None,
            remaining,
            transparent,
            search_original: None,
            selected: 0,
            toast: None,
            modal: None,
            input: String::new(),
            modal_scroll: 0,
            setting_selected: 0,
            quit: false,
            last_path: None,
            last_skip: None,
            last_position: 0.0,
            last_duration: None,
            stat_at: Instant::now(),
            folder: cached_folder,
            folder_selected: 0,
            dir_selected: 0,
            group: cached_group,
            browse_selected: 0,
            browse_artist: false,
            playlists,
            playlist_selected: 0,
            playlist_song_selected: 0,
            queue_selected: 0,
            playlist_name_target: None,
            eq_selected: 0,
            device_selected: 0,
            devices: Vec::new(),
            stats_scroll: 0,
            keybind_selected: 0,
            capture_key: false,
            bindings,
        };
        app.selected = 0;
        app.rebuild_visible();
        let restored = app.restore_queue(
            &cached_queue,
            cached_queue_pos,
            cached_last.as_deref(),
            cached_position,
        );
        if app.tracks.is_empty() {
            app.notify("曲库是空的，按 o 选一个音乐目录喵");
        } else if let Some(last) = cached_selected.or(cached_last) {
            let path = PathBuf::from(&last);
            if let Some(position) = app
                .visible
                .iter()
                .position(|&index| app.tracks[index].path == path)
            {
                app.selected = position;
                if !restored {
                    app.notify(format!(
                        "上次听到 · {} · Enter 继续",
                        app.tracks[app.visible[position]].title
                    ));
                }
            } else if !restored {
                app.notify("按 ? 看快捷键；空格播放，q 退出");
            }
        } else if !restored {
            app.notify("按 ? 看快捷键；空格播放，q 退出");
        }
        app
    }

    fn restore_queue(
        &mut self,
        paths: &[String],
        position: usize,
        last: Option<&str>,
        last_position: f64,
    ) -> bool {
        let queue: Vec<Track> = paths
            .iter()
            .filter_map(|path| {
                let path = PathBuf::from(path);
                self.tracks.iter().find(|track| track.path == path).cloned()
            })
            .collect();
        if queue.is_empty() {
            return false;
        }
        let index = last
            .and_then(|last| {
                queue
                    .iter()
                    .position(|track| track.path.to_string_lossy() == last)
            })
            .unwrap_or(position.min(queue.len() - 1));
        let list = Self::temp_m3u();
        if library::write_m3u(&list, &queue).is_err()
            || self.mpv.load_m3u(&list, index, &self.mode, true).is_err()
        {
            return false;
        }
        let Some(path) = self.mpv.state.path.clone() else {
            return false;
        };
        self.session = queue;
        self.last_path = Some(path.clone());
        self.last_position = last_position;
        self.last_duration = self.mpv.state.duration;
        if self.cfg.resume
            && last == Some(path.to_string_lossy().as_ref())
            && let Some(target) = resume_target(last_position, self.mpv.state.duration)
        {
            let _ = self.mpv.seek(target, true);
            self.last_position = target;
        }
        self.lrc = Lrc::load_for(&path);
        self.lrc_path = Some(path);
        self.notify(format!(
            "已恢复上次队列 · {} 首 · 空格继续",
            self.session.len()
        ));
        true
    }

    pub fn tick(&mut self) {
        self.mpv.refresh();
        let now = Instant::now();
        let delta = now.duration_since(self.stat_at).as_secs_f64().min(1.0);
        self.stat_at = now;
        if let Some(path) = self.mpv.state.path.clone()
            && !self.mpv.state.pause
            && !self.mpv.state.idle
        {
            let key = path.to_string_lossy().to_string();
            self.cache.total_seconds += delta;
            *self.cache.listen_seconds.entry(key).or_insert(0.0) += delta;
        }
        match self.mpv.state.path.clone() {
            Some(path) => {
                if self.last_path.as_ref() != Some(&path) {
                    if let Some(old) = self.last_path.take() {
                        let finished = self
                            .last_duration
                            .is_some_and(|duration| self.last_position > duration - 3.0);
                        if self.last_position >= 1.0 && !finished {
                            self.cache
                                .positions
                                .insert(old.to_string_lossy().to_string(), self.last_position);
                        }
                    }
                    let key = path.to_string_lossy().to_string();
                    self.last_path = Some(path.clone());
                    self.last_position = self.mpv.state.time_pos.unwrap_or(0.0);
                    self.last_duration = self.mpv.state.duration;
                    self.cache.add_recent(&path);
                    *self.cache.plays.entry(key.clone()).or_insert(0) += 1;
                    let resume = self
                        .cfg
                        .resume
                        .then(|| self.cache.positions.get(&key).copied())
                        .flatten();
                    let _ = self.cache.save();
                    self.lrc = Lrc::load_for(&path);
                    self.lrc_path = Some(path.clone());
                    let title = self
                        .mpv
                        .state
                        .metadata
                        .get("title")
                        .cloned()
                        .or_else(|| {
                            path.file_stem()
                                .map(|stem| stem.to_string_lossy().to_string())
                        })
                        .unwrap_or_else(|| "下一首".into());
                    if let Some(position) =
                        resume.and_then(|value| resume_target(value, self.mpv.state.duration))
                    {
                        let _ = self.mpv.seek(position, true);
                        self.last_position = position;
                        self.notify(format!(
                            "继续播放 · {title} · {}",
                            library::duration_text(position)
                        ));
                    } else {
                        self.notify(format!("正在播放 · {title}"));
                    }
                } else {
                    if let Some(position) = self.mpv.state.time_pos {
                        self.last_position = position;
                    }
                    if self.mpv.state.duration.is_some() {
                        self.last_duration = self.mpv.state.duration;
                    }
                }
            }
            None => {
                if let Some(old) = self.last_path.take() {
                    let finished = self
                        .last_duration
                        .is_some_and(|duration| self.last_position > duration - 3.0);
                    if self.last_position >= 1.0 && !finished {
                        self.cache
                            .positions
                            .insert(old.to_string_lossy().to_string(), self.last_position);
                    }
                }
                self.last_position = 0.0;
                self.last_duration = None;
            }
        }
        if let Some(deadline) = self.sleep_at {
            let remaining = deadline
                .saturating_duration_since(Instant::now())
                .as_secs_f64();
            if remaining <= 10.0 && self.sleep_volume.is_none() && self.mpv.state.path.is_some() {
                self.sleep_volume = Some(self.mpv.state.volume);
                self.notify("睡眠渐弱中…");
            }
            if let Some(base) = self.sleep_volume {
                let target = fade_volume(base, remaining);
                if (self.mpv.state.volume - target).abs() > 1.0 {
                    let _ = self.mpv.set_prop("volume", json!(target));
                    self.mpv.state.volume = target;
                }
            }
            if Instant::now() >= deadline {
                self.sleep_at = None;
                self.sleep_minutes = 0;
                if let Some(base) = self.sleep_volume.take() {
                    let _ = self.mpv.set_prop("volume", json!(base));
                }
                if self.mpv.state.path.is_some() {
                    let _ = self.mpv.set_prop("pause", json!(true));
                }
                self.mpv.refresh();
                self.notify("睡眠定时器到点，已暂停喵");
            }
        }
    }

    pub fn notify(&mut self, message: impl Into<String>) {
        self.toast = Some((message.into(), Instant::now()));
    }

    pub fn save(&mut self) {
        if let Some(path) = self.mpv.state.path.clone() {
            let position = self.mpv.state.time_pos.unwrap_or(self.last_position);
            self.cache
                .positions
                .insert(path.to_string_lossy().to_string(), position);
            self.cache.update_last(&path, position);
        }
        let selected_path = self
            .selected_track()
            .map(|track| track.path.to_string_lossy().to_string());
        self.cache.view = self.view.clone();
        self.cache.filter = self.filter.clone();
        self.cache.folder = self.folder.clone();
        self.cache.group = self.group.clone();
        self.cache.queue = self
            .session
            .iter()
            .map(|track| track.path.to_string_lossy().to_string())
            .collect();
        self.cache.queue_pos = self
            .mpv
            .state
            .playlist_pos
            .and_then(|position| usize::try_from(position).ok())
            .unwrap_or(0);
        self.cache.selected_path = selected_path;
        self.cfg.playlists = self.playlists.clone();
        if let Err(err) = self.cfg.save() {
            eprintln!("保存配置失败：{err}");
        }
        if let Err(err) = self.cache.save() {
            eprintln!("保存缓存失败：{err}");
        }
    }

    pub fn shutdown(&mut self) {
        let _ = std::fs::remove_file(Self::temp_m3u());
        self.mpv.quit();
    }

    pub(crate) fn roots(&self) -> Vec<PathBuf> {
        self.cfg
            .music_dirs
            .iter()
            .map(|dir| config::expand_path(dir))
            .collect()
    }

    pub(crate) fn relative_folder(&self, path: &std::path::Path) -> String {
        library::relative_folder(path, &self.roots())
    }

    pub(crate) fn rebuild_visible(&mut self) {
        let needle = self.filter.to_lowercase();
        let view = self.view.clone();
        let roots = self.roots();
        let mut visible = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let in_view = match view.as_str() {
                "fav" => self.cfg.is_favorite(&track.path),
                "recent" => self
                    .cache
                    .recent
                    .iter()
                    .any(|path| path == &track.path.to_string_lossy()),
                "folder" => library::group_key(&track.path, &roots, false) == self.folder,
                "album" => library::group_key(&track.path, &roots, false) == self.group,
                "artist" => library::group_key(&track.path, &roots, true) == self.group,
                _ => true,
            };
            if in_view
                && (needle.is_empty()
                    || track.title.to_lowercase().contains(&needle)
                    || track
                        .path
                        .to_string_lossy()
                        .to_lowercase()
                        .contains(&needle))
            {
                visible.push(index);
            }
        }
        self.visible = visible;
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }

    pub(crate) fn set_view(&mut self, view: &str) {
        self.view = view.into();
        self.selected = 0;
        self.rebuild_visible();
    }

    pub(crate) fn cycle_view(&mut self) {
        let next = match self.view.as_str() {
            "all" => "fav",
            "fav" => "recent",
            "recent" => "folder",
            _ => "all",
        };
        self.set_view(next);
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.visible.len() - 1;
        self.selected = (self.selected as isize + delta).clamp(0, max as isize) as usize;
    }

    pub(crate) fn select_edge(&mut self, last: bool) {
        if !self.visible.is_empty() {
            self.selected = if last { self.visible.len() - 1 } else { 0 };
        }
    }

    pub(crate) fn current_path(&self) -> Option<PathBuf> {
        self.mpv.state.path.clone()
    }

    pub(crate) fn next_track(&self) -> Option<&Track> {
        let position = self.mpv.state.playlist_pos?;
        if position < 0 {
            return None;
        }
        let count = self.session.len();
        if count == 0 {
            return None;
        }
        let position = position as usize;
        let index = match self.mode.as_str() {
            "single" => position,
            "loop" | "shuffle" => (position + 1) % count,
            _ => position + 1,
        };
        self.session.get(index)
    }

    pub(crate) fn selected_track(&self) -> Option<&Track> {
        self.visible
            .get(self.selected)
            .map(|&index| &self.tracks[index])
    }

    pub(crate) fn plays_of(&self, path: &std::path::Path) -> u64 {
        self.cache
            .plays
            .get(&path.to_string_lossy().to_string())
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn view_label(&self) -> String {
        match self.view.as_str() {
            "fav" => "收藏".into(),
            "recent" => "最近".into(),
            "folder" => {
                if self.folder.is_empty() {
                    "文件夹 · 根目录".into()
                } else {
                    format!("文件夹 · {}", self.folder)
                }
            }
            "album" => {
                if self.group.is_empty() {
                    "专辑 · 根目录".into()
                } else {
                    let label = std::path::Path::new(&self.group)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_else(|| self.group.clone());
                    format!("专辑 · {label}")
                }
            }
            "artist" => {
                if self.group.is_empty() {
                    "艺术家 · 根目录".into()
                } else {
                    format!("艺术家 · {}", self.group)
                }
            }
            _ => "全部歌曲".into(),
        }
    }

    pub(crate) fn mode_label(&self) -> &'static str {
        match self.mode.as_str() {
            "order" => "顺序播放",
            "single" => "单曲循环",
            "shuffle" => "随机播放",
            _ => "列表循环",
        }
    }

    pub(crate) fn temp_m3u() -> PathBuf {
        config::runtime_dir().join(format!("neko-playlist-{}.m3u", std::process::id()))
    }

    pub(crate) fn sync_session_from_mpv(&mut self) {
        let lookup: HashMap<PathBuf, Track> = self
            .tracks
            .iter()
            .map(|track| (track.path.clone(), track.clone()))
            .collect();
        let paths = self.mpv.playlist_paths();
        if !paths.is_empty() {
            self.session = paths
                .into_iter()
                .filter_map(|path| lookup.get(&path).cloned())
                .collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cycle_rating, eq_graph, fade_volume, resume_target};

    #[test]
    fn sleep_fade_reaches_zero_at_deadline() {
        assert!((fade_volume(80.0, 10.0) - 80.0).abs() < 0.001);
        assert!((fade_volume(80.0, 5.0) - 40.0).abs() < 0.001);
        assert!((fade_volume(80.0, 0.0) - 0.0).abs() < 0.001);
        assert!((fade_volume(80.0, 30.0) - 80.0).abs() < 0.001);
    }

    #[test]
    fn eq_graph_skips_flat_bands() {
        let mut gains = [0.0; 10];
        gains[0] = 6.0;
        gains[9] = -3.5;
        let graph = eq_graph(&gains);
        assert!(graph.starts_with("lavfi=["));
        assert!(graph.contains("f=31:width_type=o:width=1:g=6.0"));
        assert!(graph.contains("f=16000:width_type=o:width=1:g=-3.5"));
        assert_eq!(graph.matches("equalizer=").count(), 2);
        assert_eq!(eq_graph(&[0.0; 10]), "");
    }

    #[test]
    fn resume_skips_start_and_tail() {
        assert_eq!(resume_target(1.0, Some(200.0)), None);
        assert_eq!(resume_target(42.0, Some(200.0)), Some(42.0));
        assert_eq!(resume_target(198.0, Some(200.0)), None);
        assert_eq!(resume_target(42.0, None), Some(42.0));
    }

    #[test]
    fn rating_cycles_back_to_zero() {
        assert_eq!(cycle_rating(0), 1);
        assert_eq!(cycle_rating(4), 5);
        assert_eq!(cycle_rating(5), 0);
    }
}
