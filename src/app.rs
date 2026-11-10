mod actions;

use crate::cache::Cache;
use crate::config::{self, Config};
use crate::library::{self, Track};
use crate::lyrics::Lrc;
use crate::mpv::Mpv;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

pub(crate) const MODES: [&str; 4] = ["order", "loop", "single", "shuffle"];
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
    Seek,
    Settings,
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
}

fn fade_volume(base: f64, remaining: f64) -> f64 {
    (base * (remaining / 10.0).clamp(0.0, 1.0)).max(0.0)
}

impl App {
    pub fn new(
        mut cfg: Config,
        cache: Cache,
        mut mpv: Mpv,
        mut tracks: Vec<Track>,
        dir: PathBuf,
    ) -> Self {
        cfg.music_dir = dir.to_string_lossy().to_string();
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
        let cached_view = if ["all", "fav", "recent"].contains(&cache.view.as_str()) {
            cache.view.clone()
        } else {
            "all".into()
        };
        let cached_filter = cache.filter.clone();
        let cached_selected = cache.selected_path.clone();
        let cached_last = cache.last_path.clone();
        let _ = mpv.apply_mode(&mode);
        // 每次启动都从 1.0x 开始，避免上次调速残留。
        cfg.speed = 1.0;
        let _ = mpv.set_prop("speed", json!(1.0));
        let effect = EFFECTS
            .iter()
            .find(|(name, _)| *name == cfg.effect)
            .copied()
            .unwrap_or(EFFECTS[0]);
        let _ = mpv.set_af(effect.1);

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
        };
        app.selected = 0;
        app.rebuild_visible();
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
                app.notify(format!(
                    "上次听到 · {} · Enter 继续",
                    app.tracks[app.visible[position]].title
                ));
            } else {
                app.notify("按 ? 看快捷键；空格播放，q 退出");
            }
        } else {
            app.notify("按 ? 看快捷键；空格播放，q 退出");
        }
        app
    }

    pub fn tick(&mut self) {
        self.mpv.refresh();
        match self.mpv.state.path.clone() {
            Some(path) => {
                if self.last_path.as_ref() != Some(&path) {
                    self.last_path = Some(path.clone());
                    self.cache.add_recent(&path);
                    *self
                        .cache
                        .plays
                        .entry(path.to_string_lossy().to_string())
                        .or_insert(0) += 1;
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
                }
            }
            None => self.last_path = None,
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
            self.cache
                .update_last(&path, self.mpv.state.time_pos.unwrap_or(0.0));
        }
        let selected_path = self
            .selected_track()
            .map(|track| track.path.to_string_lossy().to_string());
        self.cache.view = self.view.clone();
        self.cache.filter = self.filter.clone();
        self.cache.selected_path = selected_path;
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

    pub(crate) fn rebuild_visible(&mut self) {
        let needle = self.filter.to_lowercase();
        let view = self.view.clone();
        let mut visible = Vec::new();
        for (index, track) in self.tracks.iter().enumerate() {
            let in_view = match view.as_str() {
                "fav" => self.cfg.is_favorite(&track.path),
                "recent" => self
                    .cache
                    .recent
                    .iter()
                    .any(|path| path == &track.path.to_string_lossy()),
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

    pub(crate) fn selected_track(&self) -> Option<&Track> {
        self.visible
            .get(self.selected)
            .map(|&index| &self.tracks[index])
    }

    pub(crate) fn view_label(&self) -> &'static str {
        match self.view.as_str() {
            "fav" => "收藏",
            "recent" => "最近",
            _ => "全部歌曲",
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
    use super::fade_volume;

    #[test]
    fn sleep_fade_reaches_zero_at_deadline() {
        assert!((fade_volume(80.0, 10.0) - 80.0).abs() < 0.001);
        assert!((fade_volume(80.0, 5.0) - 40.0).abs() < 0.001);
        assert!((fade_volume(80.0, 0.0) - 0.0).abs() < 0.001);
        assert!((fade_volume(80.0, 30.0) - 80.0).abs() < 0.001);
    }
}
