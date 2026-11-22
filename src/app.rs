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
use std::time::{Duration, Instant};

pub(crate) const MODES: [&str; 4] = ["order", "loop", "single", "shuffle"];
/// 设置项调整后的高亮脉冲时长。
pub(crate) const SETTING_PULSE: Duration = Duration::from_millis(360);
/// 底部提示淡入时长。
pub(crate) const TOAST_FADE: Duration = Duration::from_millis(280);
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
    Playlists,
    PlaylistTracks,
    PlaylistName,
    Equalizer,
    AudioDevice,
    Stats,
    Keybindings,
    Browse,
}

pub struct App {
    pub(crate) mpv: Mpv,
    pub(crate) cfg: Config,
    pub(crate) cache: Cache,
    pub(crate) index: library::Index,
    pub(crate) tracks: Vec<Track>,
    pub(crate) visible: Vec<usize>,
    pub(crate) view: String,
    pub(crate) filter: String,
    pub(crate) session: Vec<Track>,
    /// 随机模式前的原始队列顺序，退出随机时恢复。
    pub(crate) order_session: Vec<Track>,
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
    /// 首页最近播放列表的选中项。
    pub(crate) home_selected: usize,
    pub(crate) toast: Option<(String, Instant)>,
    pub(crate) modal: Option<Modal>,
    /// 正在滑出的弹窗，动画结束后清掉。
    pub(crate) modal_closing: Option<Modal>,
    /// 弹窗进度 0..1，打开时增长、关闭时回落。
    pub(crate) modal_anim: f32,
    modal_anim_from: f32,
    modal_anim_at: Instant,
    pub(crate) setting_pulse: Option<(usize, Instant)>,
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
    pub(crate) group_filter: String,
    pub(crate) browse: String,
    pub(crate) browse_groups: Vec<library::Group>,
    pub(crate) browse_selected: usize,
    pub(crate) radio_at: Option<Instant>,
    pub(crate) radio_seq: u64,
    pub(crate) folder_selected: usize,
    pub(crate) dir_selected: usize,
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
    pub(crate) viz: [f32; crate::viz::BANDS],
    pub(crate) refreshed_at: Instant,
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
        index: library::Index,
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
        // 接管后台 mpv 时不要重置速度、音效或设备，否则会打断正在播放的音乐。
        let attached = mpv.attached && mpv.state.path.is_some();
        let mode = cfg.mode.clone();
        let sort = cfg.sort.clone();
        let remaining = cfg.time_remaining;
        let transparent = cfg.transparent;
        cfg.theme = theme::set(&cfg.theme).to_string();
        cfg.eq_gains.truncate(EQ_BANDS.len());
        cfg.eq_gains.resize(EQ_BANDS.len(), 0.0);
        let cached_view = if [
            "home", "all", "fav", "recent", "folder", "album", "artist", "genre",
        ]
        .contains(&cache.view.as_str())
        {
            cache.view.clone()
        } else {
            "home".into()
        };
        let cached_filter = cache.filter.clone();
        let cached_folder = cache.folder.clone();
        let cached_group = cache.group.clone();
        let cached_queue = if cache.order_queue.is_empty() {
            cache.queue.clone()
        } else {
            cache.order_queue.clone()
        };
        let cached_queue_pos = cache.queue_pos;
        let cached_position = cache.last_position;
        let cached_selected = cache.selected_path.clone();
        let cached_last = cache.last_path.clone();
        let had_last = cached_selected.is_some() || cached_last.is_some();
        let bindings = build_bindings(&cfg.keys);
        let playlists = cfg.playlists.clone();
        if !attached {
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
        }

        let mut app = Self {
            mpv,
            cfg,
            cache,
            index,
            tracks,
            visible: Vec::new(),
            view: cached_view,
            filter: cached_filter,
            session: Vec::new(),
            order_session: Vec::new(),
            mode,
            sort,
            lrc: None,
            lrc_path: None,
            lrc_offset: 0.0,
            lyrics_full: false,
            viz: [0.0; crate::viz::BANDS],
            refreshed_at: Instant::now(),
            a_point: None,
            b_point: None,
            sleep_at: None,
            sleep_minutes: 0,
            sleep_volume: None,
            remaining,
            transparent,
            search_original: None,
            selected: 0,
            home_selected: 0,
            toast: None,
            modal: None,
            modal_closing: None,
            modal_anim: 1.0,
            modal_anim_from: 1.0,
            modal_anim_at: Instant::now(),
            setting_pulse: None,
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
            group_filter: cached_group,
            browse: "album".into(),
            browse_groups: Vec::new(),
            browse_selected: 0,
            radio_at: None,
            radio_seq: 0,
            folder_selected: 0,
            dir_selected: 0,
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
        if matches!(app.view.as_str(), "album" | "artist" | "genre") {
            app.browse = app.view.clone();
        }
        app.refresh_browse_groups();
        if matches!(app.view.as_str(), "album" | "artist" | "genre")
            && !app
                .browse_groups
                .iter()
                .any(|group| group.key == app.group_filter)
        {
            app.set_view("all");
        }
        let mut restored = false;
        if attached {
            app.sync_session_from_mpv();
            app.order_session = app
                .cache
                .order_queue
                .iter()
                .filter_map(|path| {
                    let path = PathBuf::from(path);
                    app.tracks.iter().find(|track| track.path == path).cloned()
                })
                .collect();
            if app.order_session.is_empty() {
                app.order_session = app.session.clone();
            }
            if let Some(path) = app.mpv.state.path.clone() {
                app.last_path = Some(path.clone());
                app.last_position = app.mpv.state.time_pos.unwrap_or(0.0);
                app.last_duration = app.mpv.state.duration;
                app.lrc = Lrc::load_for(&path);
                app.lrc_path = Some(path.clone());
                if let Some(position) = app
                    .visible
                    .iter()
                    .position(|&index| app.tracks[index].path == path)
                {
                    app.selected = position;
                }
                app.notify("已连接到后台播放喵");
                restored = true;
            }
        }
        if !restored {
            restored = app.restore_queue(
                &cached_queue,
                cached_queue_pos,
                cached_last.as_deref(),
                cached_position,
            );
        }
        if !attached {
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
        }
        if !attached && app.cfg.autoplay && !restored && app.mpv.state.path.is_none() && had_last {
            app.play_selected();
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
        self.order_session = self.session.clone();
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
        if self.mode == "shuffle" {
            let _ = self.mpv.shuffle_playlist();
            self.mpv.refresh();
            self.sync_session_from_mpv();
        }
        let autoplay = self.cfg.autoplay;
        if autoplay {
            let _ = self.mpv.set_prop("pause", json!(false));
            self.mpv.refresh();
        }
        self.lrc = Lrc::load_for(&path);
        self.lrc_path = Some(path);
        self.notify(if autoplay {
            format!("继续上次播放 · {} 首", self.session.len())
        } else {
            format!("已恢复上次队列 · {} 首 · 空格继续", self.session.len())
        });
        true
    }

    pub fn tick(&mut self) {
        let now = Instant::now();
        let delta = now.duration_since(self.stat_at).as_secs_f64().min(1.0);
        self.stat_at = now;
        // 弹窗滑入/滑出约 160ms；按起始时间算进度，避免空闲后的第一次 tick 跳帧。
        let progress = (self.modal_anim_at.elapsed().as_secs_f32() / 0.16).min(1.0);
        if self.modal.is_some() {
            if self.modal_anim < 1.0 {
                self.modal_anim = self.modal_anim_from + (1.0 - self.modal_anim_from) * progress;
            }
        } else if self.modal_closing.is_some() {
            self.modal_anim = self.modal_anim_from * (1.0 - progress);
            if progress >= 1.0 {
                self.modal_anim = 0.0;
                self.modal_closing = None;
            }
        }
        if self
            .setting_pulse
            .is_some_and(|(_, at)| at.elapsed() >= SETTING_PULSE)
        {
            self.setting_pulse = None;
        }
        // 界面按帧刷新，mpv 属性读取仍限在 ~8 次/秒，避免 IPC 拖慢动画
        if now.duration_since(self.refreshed_at).as_millis() >= 120 {
            self.refreshed_at = now;
            self.mpv.refresh();
            self.radio_fill();
        }
        let playing =
            !self.mpv.state.pause && !self.mpv.state.idle && self.mpv.state.path.is_some();
        crate::viz::step(&mut self.viz, playing, self.cache.total_seconds);
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
                    let key = path.to_string_lossy().to_string();
                    self.last_path = Some(path.clone());
                    self.last_position = self.mpv.state.time_pos.unwrap_or(0.0);
                    self.last_duration = self.mpv.state.duration;
                    self.cache.add_recent(&path);
                    *self.cache.plays.entry(key.clone()).or_insert(0) += 1;
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
                    self.notify(format!("正在播放 · {title}"));
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
                self.last_path = None;
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

    /// 需要高频重绘的状态：弹窗动画、设置项脉冲、吐司淡入。
    pub fn animating(&self) -> bool {
        (self.modal.is_some() && self.modal_anim < 1.0)
            || self.modal_closing.is_some()
            || self.setting_pulse.is_some()
            || self
                .toast
                .as_ref()
                .is_some_and(|(_, at)| at.elapsed() < TOAST_FADE)
    }

    /// 打开中的弹窗优先，关闭动画期间继续画即将消失的那个。
    pub fn drawn_modal(&self) -> Option<Modal> {
        self.modal.or(self.modal_closing)
    }

    pub fn save(&mut self) {
        if let Some(path) = self.mpv.state.path.clone() {
            let position = self.mpv.state.time_pos.unwrap_or(self.last_position);
            self.cache.update_last(&path, position);
        }
        let selected_path = self
            .selected_track()
            .map(|track| track.path.to_string_lossy().to_string());
        self.cache.view = self.view.clone();
        self.cache.filter = self.filter.clone();
        self.cache.folder = self.folder.clone();
        self.cache.group = self.group_filter.clone();
        self.cache.queue = self
            .session
            .iter()
            .map(|track| track.path.to_string_lossy().to_string())
            .collect();
        self.cache.order_queue = self
            .order_session
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

    /// 播放中退出时把 mpv 留在后台，返回是否仍在播放。
    pub fn shutdown(&mut self) -> bool {
        let _ = std::fs::remove_file(Self::temp_m3u());
        self.mpv.refresh();
        if self.mpv.alive() && self.mpv.state.playing() {
            self.mpv.detach();
            return true;
        }
        self.mpv.quit();
        false
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
                "folder" => library::relative_folder(&track.path, &roots) == self.folder,
                "album" => track.meta.album_key() == self.group_filter,
                "artist" => track.meta.artist == self.group_filter,
                "genre" => track.meta.genre == self.group_filter,
                _ => true,
            };
            if in_view
                && (needle.is_empty()
                    || track.title.to_lowercase().contains(&needle)
                    || track.meta.artist.to_lowercase().contains(&needle)
                    || track.meta.album.to_lowercase().contains(&needle)
                    || track.meta.genre.to_lowercase().contains(&needle)
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
        if !matches!(view, "album" | "artist" | "genre") {
            self.group_filter.clear();
        }
        self.selected = 0;
        if view == "home" {
            self.home_selected = 0;
        }
        self.rebuild_visible();
    }

    pub(crate) fn refresh_browse_groups(&mut self) {
        self.browse_groups = library::groups(&self.tracks, &self.browse);
        self.browse_selected = self
            .browse_selected
            .min(self.browse_groups.len().saturating_sub(1));
    }

    pub(crate) fn set_browse(&mut self, browse: &str) {
        if !matches!(browse, "album" | "artist" | "genre") {
            return;
        }
        self.browse = browse.into();
        self.browse_selected = 0;
        self.refresh_browse_groups();
    }

    pub(crate) fn browse_move(&mut self, delta: isize) {
        if self.browse_groups.is_empty() {
            self.browse_selected = 0;
            return;
        }
        self.browse_selected = (self.browse_selected as isize + delta)
            .clamp(0, self.browse_groups.len().saturating_sub(1) as isize)
            as usize;
    }

    pub(crate) fn browse_cycle(&mut self, delta: isize) {
        let kinds = ["album", "artist", "genre"];
        let current = kinds
            .iter()
            .position(|kind| *kind == self.browse)
            .unwrap_or(0);
        let next = (current as isize + delta).rem_euclid(kinds.len() as isize) as usize;
        self.set_browse(kinds[next]);
    }

    pub(crate) fn cycle_view(&mut self) {
        let next = match self.view.as_str() {
            "home" => "all",
            "all" => "fav",
            "fav" => "recent",
            "recent" => "folder",
            _ => "home",
        };
        self.set_view(next);
    }

    pub(crate) fn move_selection(&mut self, delta: isize) {
        if self.view == "home" {
            let count = self.home_recent_count();
            if count == 0 {
                self.home_selected = 0;
                return;
            }
            self.home_selected =
                (self.home_selected as isize + delta).clamp(0, count as isize - 1) as usize;
            return;
        }
        if self.visible.is_empty() {
            self.selected = 0;
            return;
        }
        let max = self.visible.len() - 1;
        self.selected = (self.selected as isize + delta).clamp(0, max as isize) as usize;
    }

    pub(crate) fn select_edge(&mut self, last: bool) {
        if self.view == "home" {
            let count = self.home_recent_count();
            if count > 0 {
                self.home_selected = if last { count - 1 } else { 0 };
            }
            return;
        }
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

    pub(crate) fn home_recent(&self, limit: usize) -> Vec<Track> {
        self.cache
            .recent
            .iter()
            .filter_map(|path| {
                let path = PathBuf::from(path);
                self.tracks.iter().find(|track| track.path == path)
            })
            .take(limit)
            .cloned()
            .collect()
    }

    pub(crate) fn home_recent_track(&self, index: usize) -> Option<&Track> {
        self.cache
            .recent
            .iter()
            .filter_map(|path| {
                let path = PathBuf::from(path);
                self.tracks.iter().find(|track| track.path == path)
            })
            .nth(index)
    }

    pub(crate) fn home_recent_count(&self) -> usize {
        self.cache
            .recent
            .iter()
            .filter(|path| {
                let path = PathBuf::from(path.as_str());
                self.tracks.iter().any(|track| track.path == path)
            })
            .count()
    }

    pub(crate) fn home_favorites(&self, limit: usize) -> Vec<Track> {
        self.cfg
            .favorites
            .iter()
            .filter_map(|path| {
                let path = PathBuf::from(path);
                self.tracks.iter().find(|track| track.path == path)
            })
            .take(limit)
            .cloned()
            .collect()
    }

    pub(crate) fn selected_track(&self) -> Option<&Track> {
        if self.view == "home" {
            return self.home_recent_track(self.home_selected);
        }
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
            "home" => "首页".into(),
            "fav" => "收藏".into(),
            "recent" => "最近".into(),
            "folder" => {
                if self.folder.is_empty() {
                    "文件夹 · 根目录".into()
                } else {
                    format!("文件夹 · {}", self.folder)
                }
            }
            "album" | "artist" | "genre" => {
                let kind = match self.view.as_str() {
                    "album" => "专辑",
                    "artist" => "歌手",
                    _ => "流派",
                };
                let label = self
                    .browse_groups
                    .iter()
                    .find(|group| group.key == self.group_filter)
                    .map(|group| group.label.clone())
                    .unwrap_or_default();
                if label.is_empty() {
                    kind.into()
                } else {
                    format!("{kind} · {label}")
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
