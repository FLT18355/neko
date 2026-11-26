use super::{App, EFFECTS, MODES, Modal, TRACK_MENU};
use crate::config;
use crate::keymap::{self, Action};
use crate::library;
use crate::theme;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn parse_position(text: &str) -> Option<(f64, bool)> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let relative = text.starts_with(['+', '-']);
    let mut raw = text.trim_start_matches('+');
    let negative = raw.starts_with('-');
    if negative {
        raw = &raw[1..];
    }
    let value = if raw.contains(':') {
        let parts: Vec<&str> = raw.split(':').collect();
        match parts.as_slice() {
            [minutes, seconds] => {
                minutes.parse::<f64>().ok()? * 60.0 + seconds.parse::<f64>().ok()?
            }
            [hours, minutes, seconds] => {
                hours.parse::<f64>().ok()? * 3600.0
                    + minutes.parse::<f64>().ok()? * 60.0
                    + seconds.parse::<f64>().ok()?
            }
            _ => return None,
        }
    } else {
        raw.parse::<f64>().ok()?
    };
    Some((if negative { -value } else { value }, !relative))
}

fn skip_target(position: i64, count: i64, forward: bool, wraps: bool) -> Option<i64> {
    if count <= 0 || position < 0 {
        return None;
    }
    if forward {
        if position + 1 < count {
            Some(position + 1)
        } else if wraps {
            Some(0)
        } else {
            None
        }
    } else if position > 0 {
        Some(position - 1)
    } else if wraps {
        Some(count - 1)
    } else {
        None
    }
}

impl App {
    pub(crate) fn run_action(&mut self, action: Action) {
        match action {
            Action::Quit => self.quit = true,
            Action::PlayPause => self.play_or_toggle(),
            Action::PlaySelected => self.play_selected(),
            Action::MoveDown => self.move_selection(1),
            Action::MoveUp => self.move_selection(-1),
            Action::PageDown => self.move_selection(10),
            Action::PageUp => self.move_selection(-10),
            Action::JumpFirst => self.select_edge(false),
            Action::JumpLast => self.select_edge(true),
            Action::NextTrack => self.skip(true),
            Action::PrevTrack => self.skip(false),
            Action::SeekBack5 => self.seek_relative(-5.0),
            Action::SeekForward5 => self.seek_relative(5.0),
            Action::SeekBack30 => self.seek_relative(-30.0),
            Action::SeekForward30 => self.seek_relative(30.0),
            Action::VolumeDown => self.volume_delta(-5.0),
            Action::VolumeUp => self.volume_delta(5.0),
            Action::Mute => self.toggle_mute(),
            Action::Transparent => self.toggle_transparent(),
            Action::SpeedDown => self.cycle_speed(-0.05),
            Action::SpeedUp => self.cycle_speed(0.05),
            Action::SpeedReset => self.reset_speed(),
            Action::Mode => self.cycle_mode(),
            Action::Shuffle => self.shuffle_now(),
            Action::ABLoop => self.cycle_ab(),
            Action::Favorite => self.toggle_favorite(),
            Action::Radio => self.toggle_radio(),
            Action::Browse => self.open_browse(),
            Action::ViewAll => self.set_view("all"),
            Action::ViewFav => self.set_view("fav"),
            Action::ViewRecent => self.set_view("recent"),
            Action::ViewFolder => self.open_modal(Modal::Folders),
            Action::Home => self.set_view("home"),
            Action::CycleView => self.cycle_view(),
            Action::Search => self.open_modal(Modal::Search),
            Action::Directory => self.open_modal(Modal::Directory),
            Action::Settings => self.open_modal(Modal::Settings),
            Action::Rescan => self.rescan(),
            Action::Sort => self.cycle_sort(),
            Action::Effect => self.cycle_effect(),
            Action::Equalizer => self.open_modal(Modal::Equalizer),
            Action::Sleep => self.cycle_sleep(),
            Action::Lyrics => self.toggle_lyrics(),
            Action::FullLyrics => self.toggle_full_lyrics(),
            Action::LyricsEarlier => self.lyrics_offset(-0.5),
            Action::LyricsLater => self.lyrics_offset(0.5),
            Action::Export => self.export_queue(),
            Action::SeekInput => self.open_modal(Modal::Seek),
            Action::Queue => self.open_modal(Modal::Queue),
            Action::Info => self.open_modal(Modal::Info),
            Action::Help => self.open_modal(Modal::Help),
            Action::Rating => self.rate_selected(),
            Action::Stats => self.open_modal(Modal::Stats),
            Action::Playlists => self.open_modal(Modal::Playlists),
            Action::AudioDevice => self.open_modal(Modal::AudioDevice),
            Action::Theme => self.cycle_theme(),
            Action::Keybindings => self.open_modal(Modal::Keybindings),
            Action::AddMusic => self.begin_add_music(),
            Action::RenameTrack => self.begin_rename_track(),
            Action::DeleteTrack => self.begin_delete_track(),
            Action::Download => self.open_download(),
            Action::ClearFilter => {
                if !self.filter.is_empty() {
                    self.filter.clear();
                    self.rebuild_visible();
                    self.notify("已清除搜索");
                }
            }
        }
    }
    pub(crate) fn play_selected(&mut self) {
        if self.view == "home" {
            let queue = self.home_recent(usize::MAX);
            if queue.is_empty() {
                self.notify("最近还没有播放记录喵，先挑一首吧");
                return;
            }
            let position = self.home_selected.min(queue.len() - 1);
            self.start_queue(queue, position);
            return;
        }
        if self.visible.is_empty() {
            self.notify("这里没有歌喵，换个视图或目录吧");
            return;
        }
        let queue: Vec<_> = self
            .visible
            .iter()
            .map(|&index| self.tracks[index].clone())
            .collect();
        self.start_queue(queue, self.selected);
    }

    pub(crate) fn play_home_recent(&mut self, position: usize) {
        let queue = self.home_recent(usize::MAX);
        if queue.is_empty() {
            self.notify("最近还没有播放记录喵");
            return;
        }
        let position = position.min(queue.len() - 1);
        self.home_selected = position;
        self.start_queue(queue, position);
    }

    pub(crate) fn play_home_favorite(&mut self, position: usize) {
        let queue = self.home_favorites(usize::MAX);
        if queue.is_empty() {
            self.notify("还没有收藏喵，按 f 收藏当前曲目");
            return;
        }
        let position = position.min(queue.len() - 1);
        self.start_queue(queue, position);
    }

    pub(crate) fn open_home_playlist(&mut self, position: usize) {
        if position >= self.playlists.len() {
            return;
        }
        self.playlist_selected = position;
        self.open_modal(Modal::PlaylistTracks);
    }

    pub(crate) fn start_queue(&mut self, queue: Vec<library::Track>, position: usize) {
        if queue.is_empty() {
            self.notify("队列是空的喵");
            return;
        }
        let list = Self::temp_m3u();
        if let Err(err) = library::write_m3u(&list, &queue) {
            self.notify(format!("写播放列表失败：{err}"));
            return;
        }
        if let Err(err) = self.mpv.load_m3u(&list, position, &self.mode, false) {
            self.notify(format!("mpv 播放失败：{err}"));
            return;
        }
        let title = queue[position].title.clone();
        self.order_session = queue.clone();
        self.session = queue;
        if self.mode == "shuffle" {
            let _ = self.mpv.shuffle_playlist();
            self.mpv.refresh();
            self.sync_session_from_mpv();
        }
        self.notify(format!("播放 · {title}"));
    }

    pub(crate) fn play_or_toggle(&mut self) {
        if self.mpv.state.path.is_none() {
            self.play_selected();
            return;
        }
        let paused = self.mpv.state.pause;
        if let Err(err) = self.mpv.set_prop("pause", json!(!paused)) {
            self.notify(format!("暂停/继续失败：{err}"));
            return;
        }
        self.mpv.refresh();
        if paused {
            self.notify("继续播放");
        } else {
            self.notify("已暂停");
        }
    }

    pub(crate) fn skip(&mut self, forward: bool) {
        let now = Instant::now();
        if let Some(last) = self.last_skip
            && now.duration_since(last) < Duration::from_millis(180)
        {
            return;
        }
        self.last_skip = Some(now);
        self.mpv.refresh();
        let position = self.mpv.state.playlist_pos;
        let count = self.mpv.state.playlist_count.unwrap_or(0);
        if let Some(position) = position
            && count > 0
            && position >= 0
        {
            let wraps = matches!(self.mode.as_str(), "loop" | "shuffle");
            let Some(target) = skip_target(position, count, forward, wraps) else {
                self.notify(if forward {
                    "已经是最后一首啦喵"
                } else {
                    "已经是第一首啦喵"
                });
                return;
            };
            if target == position {
                self.notify(if forward {
                    "已经是最后一首啦喵"
                } else {
                    "已经是第一首啦喵"
                });
                return;
            }
            if let Err(err) = self.mpv.set_prop("playlist-pos", json!(target)) {
                self.notify(format!("切歌失败：{err}"));
                return;
            }
            let _ = self.mpv.set_prop("pause", json!(false));
            std::thread::sleep(Duration::from_millis(80));
            self.mpv.refresh();
            return;
        }
        let result = if forward {
            self.mpv.next()
        } else {
            self.mpv.prev()
        };
        if let Err(err) = result {
            let edge = if forward { "最后一首" } else { "第一首" };
            self.notify(format!("已经是{edge}啦喵（{err}）"));
            return;
        }
        std::thread::sleep(Duration::from_millis(80));
        self.mpv.refresh();
    }

    pub(crate) fn seek_relative(&mut self, seconds: f64) {
        if let Err(err) = self.mpv.seek(seconds, false) {
            self.notify(format!("跳转失败：{err}"));
        }
    }

    pub(crate) fn seek_absolute(&mut self, seconds: f64) {
        if let Err(err) = self.mpv.seek(seconds, true) {
            self.notify(format!("跳转失败：{err}"));
        }
    }

    pub(crate) fn volume_delta(&mut self, delta: f64) {
        self.set_volume(self.mpv.state.volume + delta);
    }

    pub(crate) fn set_volume(&mut self, volume: f64) {
        let target = volume.clamp(0.0, 130.0);
        if let Err(err) = self.mpv.set_prop("volume", json!(target)) {
            self.notify(format!("调音量失败：{err}"));
            return;
        }
        let _ = self.mpv.set_prop("mute", json!(false));
        self.cfg.volume = target;
        self.mpv.state.volume = target;
        self.mpv.state.mute = false;
        self.notify(format!("音量 {target:.0}%"));
    }

    pub(crate) fn reset_speed(&mut self) {
        if let Err(err) = self.mpv.set_prop("speed", json!(1.0)) {
            self.notify(format!("还原速度失败：{err}"));
            return;
        }
        self.cfg.speed = 1.0;
        self.mpv.refresh();
        self.notify("速度 1.00x");
    }

    pub(crate) fn toggle_transparent(&mut self) {
        self.transparent = !self.transparent;
        self.cfg.transparent = self.transparent;
        self.notify(if self.transparent {
            "透明背景 · 开启"
        } else {
            "透明背景 · 关闭"
        });
    }

    pub(crate) fn toggle_remaining(&mut self) {
        self.remaining = !self.remaining;
        self.cfg.time_remaining = self.remaining;
        self.notify(if self.remaining {
            "时间显示 · 剩余"
        } else {
            "时间显示 · 已播放"
        });
    }

    pub(crate) fn clear_recent(&mut self) {
        self.cache.recent.clear();
        let _ = self.cache.save();
        if self.view == "recent" {
            self.rebuild_visible();
        }
        self.notify("最近播放已清空");
    }

    pub(crate) fn toggle_mute(&mut self) {
        let target = !self.mpv.state.mute;
        if let Err(err) = self.mpv.set_prop("mute", json!(target)) {
            self.notify(format!("静音失败：{err}"));
            return;
        }
        self.mpv.refresh();
        self.notify(if target { "已静音" } else { "取消静音" });
    }

    pub(crate) fn cycle_speed(&mut self, delta: f64) {
        let target = ((self.mpv.state.speed + delta).clamp(0.25, 4.0) * 100.0).round() / 100.0;
        if let Err(err) = self.mpv.set_prop("speed", json!(target)) {
            self.notify(format!("调速失败：{err}"));
            return;
        }
        self.cfg.speed = target;
        self.mpv.refresh();
        self.notify(format!("速度 {target:.2}x"));
    }

    pub(crate) fn cycle_mode(&mut self) {
        let index = MODES
            .iter()
            .position(|mode| *mode == self.mode)
            .unwrap_or(1);
        let was_shuffle = self.mode == "shuffle";
        let chosen = MODES[(index + 1) % MODES.len()].to_string();
        if let Err(err) = self.mpv.apply_mode(&chosen) {
            self.notify(format!("切换模式失败：{err}"));
            return;
        }
        self.mode = chosen.clone();
        self.cfg.mode = chosen;
        if self.mode == "shuffle" && !self.session.is_empty() {
            self.order_session = self.session.clone();
            let _ = self.mpv.shuffle_playlist();
            self.mpv.refresh();
            self.sync_session_from_mpv();
        } else if was_shuffle {
            // mpv 会记住 shuffle 前的顺序，退出随机时恢复原队列。
            let _ = self.mpv.unshuffle_playlist();
            self.mpv.refresh();
            self.sync_session_from_mpv();
            self.order_session = self.session.clone();
        }
        self.notify(format!("播放模式 · {}", self.mode_label()));
    }

    pub(crate) fn shuffle_now(&mut self) {
        if self.session.is_empty() {
            self.notify("先播放一首歌再打乱喵");
            return;
        }
        if self.mode != "shuffle" {
            self.order_session = self.session.clone();
        }
        self.mode = "shuffle".into();
        self.cfg.mode = "shuffle".into();
        let _ = self.mpv.apply_mode("shuffle");
        if let Err(err) = self.mpv.shuffle_playlist() {
            self.notify(format!("打乱失败：{err}"));
            return;
        }
        self.mpv.refresh();
        self.sync_session_from_mpv();
        self.notify("队列已打乱，下一位是缘分喵");
    }

    pub(crate) fn cycle_sort(&mut self) {
        let chosen = match self.sort.as_str() {
            "name" => "path",
            "path" => "modified",
            _ => "name",
        }
        .to_string();
        self.cfg.sort = chosen.clone();
        self.sort = chosen;
        library::sort_tracks(&mut self.tracks, &self.sort);
        let current = self.current_path();
        self.rebuild_visible();
        if let Some(path) = current
            && let Some(position) = self
                .visible
                .iter()
                .position(|&index| self.tracks[index].path == path)
        {
            self.selected = position;
        }
        let label = match self.sort.as_str() {
            "path" => "路径",
            "modified" => "修改时间",
            _ => "名称",
        };
        self.notify(format!("排序 · {label}"));
    }

    pub(crate) fn cycle_effect(&mut self) {
        let index = EFFECTS
            .iter()
            .position(|(name, _)| *name == self.cfg.effect)
            .unwrap_or(0);
        let (name, graph) = EFFECTS[(index + 1) % EFFECTS.len()];
        if let Err(err) = self.mpv.set_af(graph) {
            self.notify(format!("设置音效失败：{err}"));
            return;
        }
        self.cfg.effect = name.to_string();
        self.cfg.eq_on = false;
        self.notify(format!("音效 · {name}"));
    }

    pub(crate) fn cycle_sleep(&mut self) {
        const OPTIONS: [u64; 5] = [0, 15, 30, 60, 90];
        let next = OPTIONS
            .iter()
            .position(|&minutes| minutes == self.sleep_minutes)
            .map(|index| OPTIONS[(index + 1) % OPTIONS.len()])
            .unwrap_or(15);
        if let Some(base) = self.sleep_volume.take() {
            let _ = self.mpv.set_prop("volume", json!(base));
            self.mpv.state.volume = base;
        }
        self.sleep_minutes = next;
        if next == 0 {
            self.sleep_at = None;
            self.notify("睡眠定时器已关闭");
        } else {
            self.sleep_at = Some(Instant::now() + Duration::from_secs(next * 60));
            self.notify(format!("定时暂停 · {next} 分钟后（最后 10 秒渐弱）"));
        }
    }

    pub(crate) fn cycle_ab(&mut self) {
        self.mpv.refresh();
        let position = self.mpv.state.time_pos.unwrap_or(0.0);
        if self.a_point.is_none() {
            self.a_point = Some(position);
            self.b_point = None;
            self.notify(format!("A 点 {}", library::duration_text(position)));
        } else if self.b_point.is_none() {
            let a = self.a_point.unwrap_or(0.0);
            if position <= a {
                self.notify("B 点要排在 A 点后面喵");
                return;
            }
            self.b_point = Some(position);
            self.notify(format!(
                "A-B 循环 {} - {}",
                library::duration_text(a),
                library::duration_text(position)
            ));
        } else {
            self.a_point = None;
            self.b_point = None;
            self.notify("A-B 循环已关闭");
        }
        if let Err(err) = self.mpv.set_ab(self.a_point, self.b_point) {
            self.notify(format!("设置 A-B 失败：{err}"));
        }
    }

    pub(crate) fn toggle_favorite(&mut self) {
        let Some(track) = self.selected_track().cloned() else {
            self.notify("没有选中曲目喵");
            return;
        };
        let added = self.cfg.toggle_favorite(&track.path);
        if self.view == "fav" {
            self.rebuild_visible();
        }
        if added {
            self.notify(format!("已收藏 · {}", track.title));
        } else {
            self.notify(format!("取消收藏 · {}", track.title));
        }
    }

    pub(crate) fn toggle_full_lyrics(&mut self) {
        self.lyrics_full = !self.lyrics_full;
        self.notify(if self.lyrics_full {
            "全屏歌词 · Ctrl+F 或 Esc 返回"
        } else {
            "退出全屏歌词"
        });
    }

    pub(crate) fn toggle_lyrics(&mut self) {
        self.cfg.lyrics = !self.cfg.lyrics;
        self.notify(if self.cfg.lyrics {
            "歌词已开启"
        } else {
            "歌词已关闭"
        });
    }

    pub(crate) fn lyrics_offset(&mut self, delta: f64) {
        self.lrc_offset = (self.lrc_offset + delta).clamp(-10.0, 10.0);
        self.notify(format!("歌词偏移 {:+.1} 秒", self.lrc_offset));
    }

    pub(crate) fn rescan(&mut self) {
        let dirs = self.roots();
        if let Some(missing) = dirs.iter().find(|dir| !dir.is_dir()) {
            self.notify(format!("目录不存在：{}", missing.display()));
            return;
        }
        let current = self.current_path();
        self.tracks = library::scan_dirs(&dirs, &mut self.index);
        if let Err(err) = self.index.save() {
            self.notify(format!("保存曲库索引失败：{err}"));
        }
        library::sort_tracks(&mut self.tracks, &self.sort);
        self.refresh_browse_groups();
        if matches!(self.view.as_str(), "album" | "artist" | "genre")
            && !self
                .browse_groups
                .iter()
                .any(|group| group.key == self.group_filter)
        {
            self.set_view("all");
        }
        self.rebuild_visible();
        if let Some(path) = current
            && let Some(position) = self
                .visible
                .iter()
                .position(|&index| self.tracks[index].path == path)
        {
            self.selected = position;
        }
        self.notify(format!(
            "扫描完成 · {} 首 · {} 个目录",
            self.tracks.len(),
            dirs.len()
        ));
    }

    pub(crate) fn dir_list(&self) -> Vec<String> {
        self.cfg.music_dirs.clone()
    }

    pub(crate) fn dir_move(&mut self, delta: isize) {
        let count = self.cfg.music_dirs.len();
        if count == 0 {
            return;
        }
        self.dir_selected = (self.dir_selected as isize + delta)
            .clamp(0, count.saturating_sub(1) as isize) as usize;
    }

    pub(crate) fn add_music_dir(&mut self, raw: &str) -> Result<(), String> {
        let path = config::expand_path(raw.trim());
        if !path.is_dir() {
            return Err(format!("不是有效目录：{}", path.display()));
        }
        let path = path.canonicalize().unwrap_or(path);
        let value = path.to_string_lossy().to_string();
        if self.cfg.music_dirs.contains(&value) {
            return Err(format!("目录已经在列表里：{}", path.display()));
        }
        self.cfg.music_dirs.push(value);
        self.cfg
            .save()
            .map_err(|err| format!("保存目录失败：{err}"))?;
        self.rescan();
        self.notify(format!("已添加目录 · {}", path.display()));
        Ok(())
    }

    pub(crate) fn remove_music_dir(&mut self, index: usize) {
        if self.cfg.music_dirs.len() <= 1 {
            self.notify("至少保留一个音乐目录喵");
            return;
        }
        if index >= self.cfg.music_dirs.len() {
            return;
        }
        let removed = self.cfg.music_dirs.remove(index);
        self.dir_selected = self.dir_selected.min(self.cfg.music_dirs.len() - 1);
        if let Err(err) = self.cfg.save() {
            self.notify(format!("保存目录失败：{err}"));
            return;
        }
        self.rescan();
        self.notify(format!("已移除目录 · {removed}"));
    }

    pub(crate) fn export_queue(&mut self) {
        let tracks: Vec<_> = if self.session.is_empty() {
            self.visible
                .iter()
                .map(|&index| self.tracks[index].clone())
                .collect()
        } else {
            self.session.clone()
        };
        if tracks.is_empty() {
            self.notify("没有可以导出的歌曲喵");
            return;
        }
        let dir = self
            .roots()
            .into_iter()
            .next()
            .unwrap_or_else(config::default_music_dir);
        let path = dir.join("neko-歌单.m3u");
        match library::write_m3u(&path, &tracks) {
            Ok(()) => self.notify(format!("已导出 {} 首 · {}", tracks.len(), path.display())),
            Err(err) => self.notify(format!("导出失败：{err}")),
        }
    }

    pub(crate) fn open_modal(&mut self, modal: Modal) {
        if modal == Modal::Search {
            self.search_original = Some(self.filter.clone());
        }
        // 切换弹窗时从底部重新滑入；若正在关闭则从当前进度接着滑回来。
        let from = if self.modal.is_some() {
            0.0
        } else {
            self.modal_anim
        };
        self.modal = Some(modal);
        self.modal_closing = None;
        self.modal_anim_from = from;
        self.modal_anim = from;
        self.modal_anim_at = Instant::now();
        self.modal_scroll = 0;
        self.capture_key = false;
        match modal {
            Modal::Search => self.set_input(self.filter.clone()),
            Modal::Directory => {
                self.dir_selected = self
                    .dir_selected
                    .min(self.cfg.music_dirs.len().saturating_sub(1));
                self.input_clear();
            }
            Modal::DirAdd => self.input_clear(),
            Modal::Settings => {
                self.setting_selected = self.setting_selected.min(self.settings_rows().len() - 1);
                self.input_clear();
            }
            Modal::Playlists => {
                self.playlist_selected = self
                    .playlist_selected
                    .min(self.playlists.len().saturating_sub(1));
                self.input_clear();
            }
            Modal::PlaylistTracks => {
                self.playlist_song_selected = 0;
                self.input_clear();
            }
            Modal::PlaylistName => {}
            Modal::Folders => {
                let folders = self.folder_list();
                self.folder_selected = folders
                    .iter()
                    .position(|(name, _)| *name == self.folder)
                    .unwrap_or(0);
                self.input_clear();
            }
            Modal::Equalizer => {
                self.eq_selected = 0;
                self.input_clear();
            }
            Modal::AudioDevice => {
                self.devices = self.mpv.audio_devices();
                self.device_selected = self
                    .devices
                    .iter()
                    .position(|(name, _)| *name == self.cfg.audio_device)
                    .unwrap_or(0);
                self.input_clear();
            }
            Modal::Stats => {
                self.stats_scroll = 0;
                self.input_clear();
            }
            Modal::Queue => {
                self.queue_selected = self
                    .queue_selected
                    .min(self.session.len().saturating_sub(1));
                self.input_clear();
            }
            Modal::Keybindings => {
                self.keybind_selected = self
                    .keybind_selected
                    .min(keymap::ACTIONS.len().saturating_sub(1));
                self.input_clear();
            }
            Modal::Download => {}
            Modal::DownloadList => {
                self.dl_selected = self
                    .dl_selected
                    .min(self.dl_candidates.len().saturating_sub(1));
            }
            Modal::TrackMenu => self.track_menu_selected = 0,
            Modal::DownloadFormats => {
                self.dl_format_selected = self
                    .dl_format_selected
                    .min(self.dl_formats.len().saturating_sub(1));
            }
            Modal::CookiesPath => self.set_input(self.cfg.cookies.clone()),
            Modal::YtdlpPath => self.set_input(self.cfg.ytdlp.clone()),
            _ => self.input_clear(),
        }
    }

    pub(crate) fn close_modal(&mut self) {
        // 保留刚关掉的弹窗一帧帧滑出，tick 里回落动画进度后丢弃。
        if let Some(modal) = self.modal.take() {
            self.modal_closing = Some(modal);
            self.modal_anim_from = self.modal_anim;
            self.modal_anim_at = Instant::now();
        }
        self.input_clear();
        self.modal_scroll = 0;
        self.search_original = None;
        self.capture_key = false;
        self.pending_delete = None;
    }

    fn input_len(&self) -> usize {
        self.input.chars().count()
    }

    pub(crate) fn set_input(&mut self, text: impl Into<String>) {
        self.input = text.into();
        self.input_cursor = self.input_len();
    }

    pub(crate) fn input_clear(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
    }

    pub(crate) fn input_insert(&mut self, ch: char) {
        let cursor = self.input_cursor.min(self.input_len());
        let byte = self
            .input
            .char_indices()
            .nth(cursor)
            .map(|(index, _)| index)
            .unwrap_or(self.input.len());
        self.input.insert(byte, ch);
        self.input_cursor = cursor + 1;
    }

    pub(crate) fn input_backspace(&mut self) {
        let cursor = self.input_cursor.min(self.input_len());
        if cursor == 0 {
            return;
        }
        let byte = self
            .input
            .char_indices()
            .nth(cursor - 1)
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.input.remove(byte);
        self.input_cursor = cursor - 1;
    }

    pub(crate) fn input_delete(&mut self) {
        let cursor = self.input_cursor.min(self.input_len());
        let Some((byte, _)) = self.input.char_indices().nth(cursor) else {
            return;
        };
        self.input.remove(byte);
    }

    pub(crate) fn input_left(&mut self) {
        self.input_cursor = self.input_cursor.min(self.input_len()).saturating_sub(1);
    }

    pub(crate) fn input_right(&mut self) {
        self.input_cursor = (self.input_cursor + 1).min(self.input_len());
    }

    pub(crate) fn input_home(&mut self) {
        self.input_cursor = 0;
    }

    pub(crate) fn input_end(&mut self) {
        self.input_cursor = self.input_len();
    }

    pub(crate) fn sync_search(&mut self) {
        self.filter = self.input.trim().to_string();
        self.selected = 0;
        self.rebuild_visible();
    }

    pub(crate) fn cancel_modal(&mut self) {
        if self.modal == Some(Modal::Search) {
            self.filter = self.search_original.take().unwrap_or_default();
            self.selected = 0;
            self.rebuild_visible();
            self.close_modal();
        } else if self.modal == Some(Modal::DirAdd) {
            self.open_modal(Modal::Directory);
        } else if matches!(self.modal, Some(Modal::CookiesPath | Modal::YtdlpPath)) {
            self.open_modal(Modal::Settings);
        } else {
            self.close_modal();
        }
    }

    pub(crate) fn submit_modal(&mut self) {
        let Some(modal) = self.modal else {
            return;
        };
        match modal {
            Modal::Search => {
                self.sync_search();
                if self.filter.is_empty() {
                    self.notify("已清除搜索");
                } else {
                    self.notify(format!(
                        "搜索「{}」· {} 首",
                        self.filter,
                        self.visible.len()
                    ));
                }
                self.close_modal();
            }
            Modal::DirAdd => {
                let raw = self.input.trim().to_string();
                if raw.is_empty() {
                    self.notify("目录不能为空喵");
                    return;
                }
                if let Err(err) = self.add_music_dir(&raw) {
                    self.notify(err);
                    return;
                }
                self.open_modal(Modal::Directory);
            }
            Modal::Seek => {
                let Some((seconds, absolute)) = parse_position(&self.input) else {
                    self.notify("时间格式不认识喵，例如 1:23 或 -15");
                    return;
                };
                if absolute {
                    self.seek_absolute(seconds);
                } else {
                    self.seek_relative(seconds);
                }
                self.close_modal();
            }
            Modal::PlaylistName => self.submit_playlist_name(),
            Modal::AddMusic => self.submit_add_music(),
            Modal::RenameTrack => self.submit_rename_track(),
            Modal::Download => self.start_download_search(),
            Modal::CookiesPath => {
                self.cfg.cookies = self.input.trim().to_string();
                if let Err(err) = self.cfg.save() {
                    self.notify(format!("保存配置失败：{err}"));
                    return;
                }
                self.notify(if self.cfg.cookies.is_empty() {
                    "cookies · 不发送".to_string()
                } else {
                    format!("cookies · {}", self.cfg.cookies)
                });
                self.open_modal(Modal::Settings);
            }
            Modal::YtdlpPath => {
                self.cfg.ytdlp = self.input.trim().to_string();
                if let Err(err) = self.cfg.save() {
                    self.notify(format!("保存配置失败：{err}"));
                    return;
                }
                self.notify(format!(
                    "yt-dlp · {}",
                    if self.cfg.ytdlp.is_empty() {
                        "PATH 里的 yt-dlp"
                    } else {
                        self.cfg.ytdlp.as_str()
                    }
                ));
                self.open_modal(Modal::Settings);
            }
            _ => self.close_modal(),
        }
    }

    pub(crate) fn lyrics_enabled(&self) -> bool {
        self.cfg.lyrics
    }

    pub(crate) fn current_lyric(&self) -> Option<(usize, &Vec<(f64, String)>)> {
        let lrc = self.lrc.as_ref()?;
        if lrc.lines.is_empty() {
            return None;
        }
        let position = self.mpv.state.time_pos.unwrap_or(0.0) + self.lrc_offset;
        Some((lrc.index_at(position).unwrap_or(0), &lrc.lines))
    }

    pub(crate) fn playlist_move(&mut self, delta: isize) {
        let count = self.playlists.len();
        if count == 0 {
            return;
        }
        self.playlist_selected = (self.playlist_selected as isize + delta)
            .clamp(0, count.saturating_sub(1) as isize) as usize;
    }

    pub(crate) fn open_playlist_tracks(&mut self) {
        if self.playlists.get(self.playlist_selected).is_none() {
            self.notify("还没有歌单喵，按 n 新建");
            return;
        }
        self.playlist_song_selected = 0;
        self.open_modal(Modal::PlaylistTracks);
    }

    pub(crate) fn begin_playlist_create(&mut self) {
        self.playlist_name_target = None;
        self.input_clear();
        self.open_modal(Modal::PlaylistName);
    }

    pub(crate) fn begin_playlist_rename(&mut self) {
        let Some(list) = self.playlists.get(self.playlist_selected) else {
            self.notify("没有选中的歌单喵");
            return;
        };
        self.playlist_name_target = Some(self.playlist_selected);
        self.set_input(list.name.clone());
        self.open_modal(Modal::PlaylistName);
    }

    pub(crate) fn submit_playlist_name(&mut self) {
        let name = self.input.trim().to_string();
        if name.is_empty() {
            self.notify("歌单名不能为空喵");
            return;
        }
        match self.playlist_name_target.take() {
            None => self.playlists.push(config::Playlist {
                name: name.clone(),
                songs: Vec::new(),
            }),
            Some(index) => {
                let Some(list) = self.playlists.get_mut(index) else {
                    self.notify("歌单不见了喵");
                    return;
                };
                list.name = name.clone();
            }
        }
        self.persist_playlists();
        self.notify(format!("歌单 · {name}"));
        self.close_modal();
        self.open_modal(Modal::Playlists);
    }

    pub(crate) fn delete_playlist(&mut self) {
        if self.playlists.is_empty() {
            return;
        }
        let removed = self.playlists.remove(self.playlist_selected);
        self.persist_playlists();
        self.notify(format!("已删除歌单 · {}", removed.name));
        self.playlist_selected = self
            .playlist_selected
            .min(self.playlists.len().saturating_sub(1));
    }

    pub(crate) fn add_to_playlist(&mut self) {
        let Some(track) = self.selected_track().cloned() else {
            self.notify("没有选中曲目喵");
            return;
        };
        let Some(list) = self.playlists.get_mut(self.playlist_selected) else {
            self.notify("先新建一个歌单喵");
            return;
        };
        if list.add(&track.path) {
            let name = list.name.clone();
            self.persist_playlists();
            self.notify(format!("已加入「{name}」· {}", track.title));
        } else {
            self.notify("这首已经在歌单里了喵");
        }
    }

    pub(crate) fn remove_playlist_song(&mut self) {
        let index = self.playlist_song_selected;
        let Some(list) = self.playlists.get_mut(self.playlist_selected) else {
            return;
        };
        let Some(path) = list.remove(index) else {
            return;
        };
        let remaining = list.songs.len();
        self.persist_playlists();
        self.playlist_song_selected = self.playlist_song_selected.min(remaining.saturating_sub(1));
        self.notify(format!("已移出 · {}", self.title_for(&path)));
    }

    pub(crate) fn play_playlist(&mut self, position: usize) {
        let Some(list) = self.playlists.get(self.playlist_selected).cloned() else {
            return;
        };
        let queue: Vec<library::Track> = list
            .songs
            .iter()
            .filter_map(|song| {
                self.tracks
                    .iter()
                    .find(|track| track.path.to_string_lossy() == *song)
                    .cloned()
            })
            .collect();
        if queue.is_empty() {
            self.notify("歌单里的歌都不在曲库里喵");
            return;
        }
        let position = position.min(queue.len() - 1);
        self.start_queue(queue, position);
    }

    fn persist_playlists(&mut self) {
        self.cfg.playlists = self.playlists.clone();
        if let Err(err) = self.cfg.save() {
            self.notify(format!("保存歌单失败：{err}"));
        }
    }

    pub(crate) fn folder_list(&self) -> Vec<(String, usize)> {
        library::folders(&self.tracks, &self.roots())
    }

    pub(crate) fn folder_move(&mut self, delta: isize) {
        let count = self.folder_list().len();
        if count == 0 {
            return;
        }
        self.folder_selected = (self.folder_selected as isize + delta)
            .clamp(0, count.saturating_sub(1) as isize) as usize;
    }

    pub(crate) fn select_folder(&mut self) {
        let folders = self.folder_list();
        let Some((name, _)) = folders.get(self.folder_selected) else {
            self.notify("没有文件夹喵");
            return;
        };
        self.folder = name.clone();
        self.set_view("folder");
        let count = self.visible.len();
        self.notify(format!(
            "文件夹 · {} · {count} 首",
            if self.folder.is_empty() {
                "根目录".to_string()
            } else {
                self.folder.clone()
            }
        ));
        self.close_modal();
    }

    pub(crate) fn open_browse(&mut self) {
        if matches!(self.view.as_str(), "album" | "artist" | "genre") {
            self.browse = self.view.clone();
        }
        self.refresh_browse_groups();
        self.open_modal(Modal::Browse);
    }

    pub(crate) fn select_browse_group(&mut self) {
        let Some(group) = self.browse_groups.get(self.browse_selected).cloned() else {
            self.notify("曲库是空的喵");
            return;
        };
        let kind = self.browse.clone();
        self.view = kind;
        self.group_filter = group.key;
        self.selected = 0;
        self.rebuild_visible();
        let count = self.visible.len();
        self.notify(format!("{} · {count} 首", group.label));
        self.close_modal();
    }

    pub(crate) fn effect_label(&self) -> String {
        if self.cfg.eq_on {
            "自定义 EQ".into()
        } else {
            self.cfg.effect.clone()
        }
    }

    pub(crate) fn apply_audio_graph(&mut self) {
        let graph = if self.cfg.eq_on {
            super::eq_graph(&self.cfg.eq_gains)
        } else {
            EFFECTS
                .iter()
                .find(|(name, _)| *name == self.cfg.effect)
                .map(|(_, graph)| *graph)
                .unwrap_or("")
                .to_string()
        };
        if let Err(err) = self.mpv.set_af(&graph) {
            self.notify(format!("设置音效失败：{err}"));
        }
    }

    pub(crate) fn toggle_eq(&mut self) {
        self.cfg.eq_on = !self.cfg.eq_on;
        self.apply_audio_graph();
        self.notify(if self.cfg.eq_on {
            "均衡器 · 开启"
        } else {
            "均衡器 · 关闭"
        });
    }

    pub(crate) fn adjust_eq(&mut self, row: usize, delta: f64) {
        if row == 0 {
            self.toggle_eq();
            return;
        }
        let band = row - 1;
        if band >= self.cfg.eq_gains.len() {
            return;
        }
        self.cfg.eq_on = true;
        self.cfg.eq_gains[band] = (self.cfg.eq_gains[band] + delta).clamp(-12.0, 12.0);
        self.apply_audio_graph();
        self.notify(format!(
            "{}Hz · {:+.0} dB",
            super::EQ_BANDS[band],
            self.cfg.eq_gains[band]
        ));
    }

    pub(crate) fn reset_eq(&mut self) {
        self.cfg.eq_gains = vec![0.0; super::EQ_BANDS.len()];
        self.apply_audio_graph();
        self.notify("均衡器已拉平");
    }

    pub(crate) fn activate_eq_row(&mut self) {
        if self.eq_selected == 0 {
            self.toggle_eq();
            return;
        }
        let band = self.eq_selected - 1;
        let gain = self.cfg.eq_gains.get(band).copied().unwrap_or(0.0);
        self.adjust_eq(self.eq_selected, -gain);
    }

    pub(crate) fn begin_capture_binding(&mut self) {
        self.capture_key = true;
        self.notify("按下新按键 · Esc 取消");
    }

    pub(crate) fn device_move(&mut self, delta: isize) {
        if self.devices.is_empty() {
            return;
        }
        self.device_selected = (self.device_selected as isize + delta)
            .clamp(0, self.devices.len().saturating_sub(1) as isize)
            as usize;
    }

    pub(crate) fn apply_device(&mut self) {
        let Some((name, description)) = self.devices.get(self.device_selected).cloned() else {
            self.notify("没有可用的音频设备喵");
            return;
        };
        if let Err(err) = self.mpv.set_audio_device(&name) {
            self.notify(format!("切换设备失败：{err}"));
            return;
        }
        self.cfg.audio_device = name;
        self.notify(format!("音频输出 · {description}"));
        self.close_modal();
    }

    pub(crate) fn title_for(&self, raw: &str) -> String {
        let path = Path::new(raw);
        self.tracks
            .iter()
            .find(|track| track.path == path)
            .map(|track| track.title.clone())
            .or_else(|| {
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| raw.to_string())
    }

    pub(crate) fn stats_rows(&self) -> Vec<(String, String)> {
        let plays: u64 = self.cache.plays.values().sum();
        let tracks = self.cache.plays.len().max(self.cache.listen_seconds.len());
        vec![
            (
                "总收听".into(),
                library::human_duration(self.cache.total_seconds),
            ),
            ("总播放".into(), format!("{plays} 次")),
            ("曲目".into(), format!("{tracks} 首")),
            (
                "平均每首".into(),
                library::human_duration(if tracks == 0 {
                    0.0
                } else {
                    self.cache.total_seconds / tracks as f64
                }),
            ),
        ]
    }

    pub(crate) fn stats_top(&self, limit: usize) -> Vec<(String, u64, f64)> {
        let mut items: Vec<(String, u64, f64)> = self
            .cache
            .plays
            .iter()
            .map(|(path, plays)| {
                (
                    self.title_for(path),
                    *plays,
                    self.cache.listen_seconds.get(path).copied().unwrap_or(0.0),
                )
            })
            .collect();
        items.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then(b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal))
        });
        items.truncate(limit);
        items
    }

    pub(crate) fn clear_stats(&mut self) {
        self.cache.plays.clear();
        self.cache.listen_seconds.clear();
        self.cache.total_seconds = 0.0;
        let _ = self.cache.save();
        self.notify("收听统计已清空");
    }

    pub(crate) fn rating_of(&self, path: &Path) -> u8 {
        self.cache
            .ratings
            .get(&path.to_string_lossy().to_string())
            .copied()
            .unwrap_or(0)
    }

    pub(crate) fn rate_selected(&mut self) {
        let Some(track) = self.selected_track().cloned() else {
            self.notify("没有选中曲目喵");
            return;
        };
        let key = track.path.to_string_lossy().to_string();
        let current = self.cache.ratings.get(&key).copied().unwrap_or(0);
        let next = super::cycle_rating(current);
        if next == 0 {
            self.cache.ratings.remove(&key);
            self.notify(format!("清除评分 · {}", track.title));
        } else {
            self.cache.ratings.insert(key, next);
            self.notify(format!("{} · {}", "★".repeat(next as usize), track.title));
        }
        let _ = self.cache.save();
    }

    pub(crate) fn toggle_resume(&mut self) {
        self.cfg.resume = !self.cfg.resume;
        self.notify(if self.cfg.resume {
            "断点续播 · 开启"
        } else {
            "断点续播 · 关闭"
        });
    }

    pub(crate) fn toggle_autoplay(&mut self) {
        self.cfg.autoplay = !self.cfg.autoplay;
        self.notify(if self.cfg.autoplay {
            "启动自动播放 · 开启"
        } else {
            "启动自动播放 · 关闭"
        });
    }

    pub(crate) fn toggle_radio(&mut self) {
        self.cfg.radio = !self.cfg.radio;
        if self.cfg.radio {
            self.radio_at = None;
            self.notify("智能电台 · 开启 · 队列快播完时自动续播");
        } else {
            self.notify("智能电台 · 关闭");
        }
    }

    /// 队列只剩两首时按标签相似度补一首，保持智能电台不断播。
    pub(crate) fn radio_fill(&mut self) {
        if !self.cfg.radio || self.mpv.state.pause {
            return;
        }
        let now = Instant::now();
        if self
            .radio_at
            .is_some_and(|at| now.duration_since(at) < Duration::from_secs(1))
        {
            return;
        }
        let Some(position) = self.mpv.state.playlist_pos else {
            return;
        };
        if position < 0 {
            return;
        }
        let count = self.mpv.state.playlist_count.unwrap_or(0);
        if count <= 0 || count - position > 2 || self.session.is_empty() {
            return;
        }
        let current = self.session.get(position as usize).cloned().or_else(|| {
            let path = self.mpv.state.path.as_ref()?;
            self.tracks
                .iter()
                .find(|track| &track.path == path)
                .cloned()
        });
        let Some(current) = current else {
            return;
        };
        let played: std::collections::HashSet<String> = self
            .session
            .iter()
            .map(|track| track.path.to_string_lossy().to_string())
            .collect();
        let recent: std::collections::HashSet<String> =
            self.cache.recent.iter().take(20).cloned().collect();
        let mut candidates: Vec<(f64, &library::Track)> = self
            .tracks
            .iter()
            .filter(|track| track.path != current.path)
            .filter(|track| {
                let key = track.path.to_string_lossy();
                !played.contains(key.as_ref()) && !recent.contains(key.as_ref())
            })
            .map(|track| (library::similarity(&current, track), track))
            .collect();
        if candidates.is_empty() {
            // 曲库都被排过时，至少避开队列尾部，别把上一首立刻重复一遍。
            let tail: std::collections::HashSet<String> = self
                .session
                .iter()
                .rev()
                .take(3)
                .map(|track| track.path.to_string_lossy().to_string())
                .collect();
            candidates = self
                .tracks
                .iter()
                .filter(|track| track.path != current.path)
                .filter(|track| !tail.contains(track.path.to_string_lossy().as_ref()))
                .map(|track| (library::similarity(&current, track), track))
                .collect();
        }
        if candidates.is_empty() {
            candidates = self
                .tracks
                .iter()
                .filter(|track| track.path != current.path)
                .map(|track| (library::similarity(&current, track), track))
                .collect();
        }
        if candidates.is_empty() {
            return;
        }
        candidates.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        // 只有同歌手、同专辑或同流派才算强相关；年代和时长只用于排序。
        let strong = candidates.iter().any(|(score, _)| *score >= 3.0);
        if strong {
            candidates.retain(|(score, _)| *score >= 3.0);
        }
        let top = candidates.len().min(3);
        self.radio_seq = self.radio_seq.wrapping_add(1);
        let pick = (self.radio_seq as usize + count as usize) % top;
        let track = candidates[pick].1.clone();
        self.radio_at = Some(now);
        if self.mpv.append_track(&track.path).is_ok() {
            self.session.push(track.clone());
            self.order_session.push(track);
            self.mpv.refresh();
            self.sync_session_from_mpv();
        }
    }

    pub(crate) fn cycle_theme(&mut self) {
        let key = theme::next();
        self.cfg.theme = key.to_string();
        self.notify(format!("主题 · {}", theme::label()));
    }

    pub(crate) fn key_for(&self, action: Action) -> String {
        self.cfg
            .keys
            .get(action.id())
            .cloned()
            .unwrap_or_else(|| keymap::default_keys(action).join(" / "))
    }

    fn rebuild_bindings(&mut self) {
        self.bindings = super::build_bindings(&self.cfg.keys);
    }

    pub(crate) fn set_binding(&mut self, action: Action, key: String) {
        if let Some(other) = self.bindings.get(&key).copied()
            && other != action
        {
            self.notify(format!("{key} 已被「{}」占用", other.label()));
            return;
        }
        self.cfg.keys.insert(action.id().to_string(), key.clone());
        self.rebuild_bindings();
        self.notify(format!("{} → {key}", action.label()));
    }

    pub(crate) fn reset_binding(&mut self, action: Action) {
        self.cfg.keys.remove(action.id());
        self.rebuild_bindings();
        self.notify(format!(
            "{} 恢复默认 · {}",
            action.label(),
            keymap::default_keys(action).join(" / ")
        ));
    }

    pub(crate) fn settings_rows(&self) -> Vec<(String, String)> {
        let state = &self.mpv.state;
        let mode = match self.mode.as_str() {
            "order" => "顺序播放",
            "single" => "单曲循环",
            "shuffle" => "随机播放",
            _ => "列表循环",
        };
        let sort = match self.sort.as_str() {
            "path" => "路径",
            "modified" => "修改时间",
            _ => "名称",
        };
        let sleep = match self.sleep_at {
            Some(deadline) => format!(
                "{} 分钟",
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_secs()
                    .div_ceil(60)
                    .max(1)
            ),
            None => "关闭".into(),
        };
        let ab = match (self.a_point, self.b_point) {
            (Some(a), Some(b)) => format!(
                "{} - {}",
                library::duration_text(a),
                library::duration_text(b)
            ),
            (Some(a), None) => format!("A {}", library::duration_text(a)),
            _ => "关闭".into(),
        };
        let download_dir = self
            .roots()
            .into_iter()
            .next()
            .unwrap_or_else(config::default_music_dir)
            .display()
            .to_string();
        vec![
            (
                "音乐目录".into(),
                if self.cfg.music_dirs.len() == 1 {
                    self.cfg.music_dirs[0].clone()
                } else {
                    format!("{} 个目录", self.cfg.music_dirs.len())
                },
            ),
            (
                "音量".into(),
                if state.mute {
                    "静音".into()
                } else {
                    format!("{:.0}%", state.volume)
                },
            ),
            ("播放速度".into(), format!("{:.2}x", state.speed)),
            ("播放模式".into(), mode.into()),
            ("音效".into(), self.effect_label()),
            (
                "均衡器".into(),
                if self.cfg.eq_on {
                    "自定义"
                } else {
                    "跟随预设"
                }
                .into(),
            ),
            (
                "歌词".into(),
                if self.cfg.lyrics { "开启" } else { "关闭" }.into(),
            ),
            ("歌词偏移".into(), format!("{:+.1}s", self.lrc_offset)),
            ("排序".into(), sort.into()),
            ("睡眠定时".into(), sleep),
            ("A-B 循环".into(), ab),
            (
                "透明背景".into(),
                if self.transparent { "开启" } else { "关闭" }.into(),
            ),
            (
                "时间显示".into(),
                if self.remaining {
                    "剩余"
                } else {
                    "已播放"
                }
                .into(),
            ),
            ("主题".into(), theme::label().into()),
            (
                "断点续播".into(),
                if self.cfg.resume { "开启" } else { "关闭" }.into(),
            ),
            (
                "启动自动播放".into(),
                if self.cfg.autoplay {
                    "开启"
                } else {
                    "关闭"
                }
                .into(),
            ),
            (
                "智能电台".into(),
                if self.cfg.radio { "开启" } else { "关闭" }.into(),
            ),
            ("快捷键".into(), "自定义".into()),
            (
                "音频设备".into(),
                if self.cfg.audio_device.is_empty() {
                    "自动".into()
                } else {
                    self.cfg.audio_device.clone()
                },
            ),
            (
                "清空最近".into(),
                if self.cache.recent.is_empty() {
                    "已经是空的".into()
                } else {
                    format!("{} 条", self.cache.recent.len())
                },
            ),
            (
                "配置文件".into(),
                config::config_path().display().to_string(),
            ),
            (
                "缓存文件".into(),
                crate::cache::cache_path().display().to_string(),
            ),
            (
                "yt-dlp 路径".into(),
                if self.cfg.ytdlp.is_empty() {
                    "PATH 里的 yt-dlp".into()
                } else {
                    self.cfg.ytdlp.clone()
                },
            ),
            (
                "cookies 文件".into(),
                if self.cfg.cookies.is_empty() {
                    "不发送".into()
                } else {
                    self.cfg.cookies.clone()
                },
            ),
            ("搜索数量".into(), format!("{} 个", self.cfg.download_limit)),
            ("下载目录".into(), download_dir),
        ]
    }

    pub(crate) fn move_setting(&mut self, delta: isize) {
        let count = self.settings_rows().len();
        let previous = self.setting_selected;
        self.setting_selected = (self.setting_selected as isize + delta)
            .clamp(0, count.saturating_sub(1) as isize) as usize;
        if self.setting_selected != previous {
            self.setting_pulse = Some((self.setting_selected, Instant::now()));
        }
    }

    pub(crate) fn adjust_setting(&mut self, index: usize, delta: isize) {
        if delta != 0 {
            self.setting_pulse = Some((index, Instant::now()));
        }
        match index {
            0 => {
                if delta != 0 {
                    self.open_modal(Modal::Directory);
                }
            }
            1 => self.volume_delta(5.0 * delta as f64),
            2 => self.cycle_speed(0.05 * delta as f64),
            3 => self.cycle_mode(),
            4 => self.cycle_effect(),
            5 => {
                if delta != 0 {
                    self.open_modal(Modal::Equalizer);
                }
            }
            6 => {
                if delta != 0 {
                    self.toggle_lyrics();
                }
            }
            7 => self.lyrics_offset(0.5 * delta as f64),
            8 => self.cycle_sort(),
            9 => self.cycle_sleep(),
            10 => self.cycle_ab(),
            11 => {
                if delta != 0 {
                    self.toggle_transparent();
                }
            }
            12 => {
                if delta != 0 {
                    self.toggle_remaining();
                }
            }
            13 => {
                if delta != 0 {
                    self.cycle_theme();
                }
            }
            14 => {
                if delta != 0 {
                    self.toggle_resume();
                }
            }
            15 => {
                if delta != 0 {
                    self.toggle_autoplay();
                }
            }
            16 => {
                if delta != 0 {
                    self.toggle_radio();
                }
            }
            17 => {
                if delta != 0 {
                    self.open_modal(Modal::Keybindings);
                }
            }
            18 => {
                if delta != 0 {
                    self.open_modal(Modal::AudioDevice);
                }
            }
            19 => {
                if delta != 0 {
                    self.clear_recent();
                }
            }
            22 => {
                if delta != 0 {
                    self.open_modal(Modal::YtdlpPath);
                }
            }
            23 => {
                if delta != 0 {
                    self.open_modal(Modal::CookiesPath);
                }
            }
            24 => {
                self.cfg.download_limit =
                    (self.cfg.download_limit as isize + delta).clamp(3, 30) as usize;
                self.notify(format!("搜索数量 · {} 个", self.cfg.download_limit));
            }
            20 | 21 | 25 => {}
            _ => {}
        }
    }

    pub(crate) fn activate_setting(&mut self, index: usize) {
        match index {
            0 => self.open_modal(Modal::Directory),
            22 => self.open_modal(Modal::YtdlpPath),
            23 => self.open_modal(Modal::CookiesPath),
            25 => {}
            _ => self.adjust_setting(index, 1),
        }
    }

    pub(crate) fn play_queue_index(&mut self, index: usize) {
        if index >= self.session.len() {
            return;
        }
        let track = self.session[index].clone();
        if let Err(err) = self.mpv.set_prop("playlist-pos", serde_json::json!(index)) {
            self.notify(format!("播放失败：{err}"));
            return;
        }
        let _ = self.mpv.set_prop("pause", serde_json::json!(false));
        self.mpv.refresh();
        self.notify(format!("播放 · {}", track.title));
    }

    pub(crate) fn queue_move(&mut self, delta: isize) {
        if self.session.is_empty() {
            return;
        }
        let from = self.queue_selected as i64;
        let to = from + delta as i64;
        if to < 0 || to >= self.session.len() as i64 {
            return;
        }
        if let Err(err) = self.mpv.playlist_move(from, to) {
            self.notify(format!("移动失败：{err}"));
            return;
        }
        self.queue_selected = to as usize;
        self.mpv.refresh();
        self.sync_session_from_mpv();
        if self.mode != "shuffle" && (from as usize) < self.order_session.len() {
            let track = self.order_session.remove(from as usize);
            let to = (to as usize).min(self.order_session.len());
            self.order_session.insert(to, track);
        }
        self.notify("队列顺序已更新");
    }

    pub(crate) fn queue_remove(&mut self) {
        if self.session.is_empty() {
            return;
        }
        let index = self.queue_selected;
        let removed = self.session[index].clone();
        if let Err(err) = self.mpv.playlist_remove(index as i64) {
            self.notify(format!("移除失败：{err}"));
            return;
        }
        self.mpv.refresh();
        self.sync_session_from_mpv();
        self.order_session
            .retain(|track| track.path != removed.path);
        self.queue_selected = self
            .queue_selected
            .min(self.session.len().saturating_sub(1));
        self.notify(format!("已移出队列 · {}", removed.title));
    }

    pub(crate) fn queue_clear(&mut self) {
        if self.session.is_empty() {
            return;
        }
        let _ = self.mpv.stop();
        let _ = self.mpv.playlist_clear();
        self.mpv.refresh();
        self.session.clear();
        self.order_session.clear();
        self.queue_selected = 0;
        self.last_path = None;
        self.notify("队列已清空");
    }

    pub(crate) fn open_track_menu(&mut self) {
        if self.selected_track().is_none() {
            self.notify("没有选中曲目喵");
            return;
        }
        self.track_menu_selected = 0;
        self.open_modal(Modal::TrackMenu);
    }

    pub(crate) fn track_menu_move(&mut self, delta: isize) {
        self.track_menu_selected = (self.track_menu_selected as isize + delta)
            .clamp(0, TRACK_MENU.len() as isize - 1) as usize;
    }

    pub(crate) fn run_track_menu(&mut self, index: usize) {
        match index {
            0 => {
                self.close_modal();
                self.play_selected();
            }
            1 => {
                self.close_modal();
                self.toggle_favorite();
            }
            2 => {
                self.close_modal();
                self.open_modal(Modal::Playlists);
                self.notify("选一个歌单，按 a 加入当前曲目喵");
            }
            3 => {
                self.close_modal();
                self.begin_rename_track();
            }
            4 => {
                self.close_modal();
                self.begin_delete_track();
            }
            _ => {}
        }
    }

    pub(crate) fn begin_add_music(&mut self) {
        self.open_modal(Modal::AddMusic);
        self.input_clear();
    }

    pub(crate) fn submit_add_music(&mut self) {
        let raw = self.input.trim().to_string();
        if raw.is_empty() {
            self.notify("路径不能为空喵");
            return;
        }
        let source = config::expand_path(&raw);
        if !source.exists() {
            self.notify(format!("找不到：{}", source.display()));
            return;
        }
        let root = self
            .roots()
            .into_iter()
            .next()
            .unwrap_or_else(config::default_music_dir);
        if source.starts_with(&root) {
            self.notify("已经在音乐目录里喵");
            return;
        }
        let result = if source.is_dir() {
            copy_audio_tree(&source, &root)
        } else if library::is_audio(&source) {
            copy_audio_file(&source, &root).map(|_| 1)
        } else {
            Err("不是音频文件喵".into())
        };
        match result {
            Ok(count) => {
                self.rescan();
                self.notify(format!("已添加 {count} 首 · {}", root.display()));
                self.close_modal();
            }
            Err(err) => self.notify(err),
        }
    }

    pub(crate) fn begin_rename_track(&mut self) {
        let Some(track) = self.selected_track().cloned() else {
            self.notify("没有选中曲目喵");
            return;
        };
        if self.mpv.state.path.as_ref() == Some(&track.path) {
            self.notify("正在播放这首，先切歌再改名喵");
            return;
        }
        let stem = track
            .path
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or(track.title);
        self.open_modal(Modal::RenameTrack);
        self.set_input(stem);
    }

    pub(crate) fn submit_rename_track(&mut self) {
        let raw = self.input.trim().to_string();
        if raw.is_empty() {
            self.notify("名字不能为空喵");
            return;
        }
        let Some(track) = self.selected_track().cloned() else {
            self.notify("没有选中曲目喵");
            return;
        };
        let old = track.path;
        let ext = old.extension().map(|ext| ext.to_string_lossy().to_string());
        let mut stem = raw;
        if let Some(ext) = &ext {
            let suffix = format!(".{ext}");
            if stem.len() > suffix.len() && stem.to_lowercase().ends_with(&suffix.to_lowercase()) {
                stem.truncate(stem.len() - suffix.len());
            }
        }
        if stem.is_empty() || stem.contains(['/', '\\']) {
            self.notify("名字里不能有斜杠喵");
            return;
        }
        let file_name = match &ext {
            Some(ext) => format!("{stem}.{ext}"),
            None => stem.clone(),
        };
        let target = old.with_file_name(file_name);
        if target == old {
            self.close_modal();
            self.notify("名字没有变喵");
            return;
        }
        if target.exists() {
            self.notify("同名文件已经存在喵");
            return;
        }
        if let Err(err) = fs::rename(&old, &target) {
            self.notify(format!("重命名失败：{err}"));
            return;
        }
        let tag_error = library::write_title(&target, &stem).err();
        self.remap_path(&old, &target);
        self.rescan();
        self.close_modal();
        if let Some(err) = tag_error {
            self.notify(format!("已改文件名 · 标签写入失败：{err}"));
        } else {
            self.notify(format!("已重命名 · {stem}"));
        }
    }

    pub(crate) fn begin_delete_track(&mut self) {
        let Some(track) = self.selected_track().cloned() else {
            self.notify("没有选中曲目喵");
            return;
        };
        if self.mpv.state.path.as_ref() == Some(&track.path) {
            self.notify("正在播放这首，先切歌再删喵");
            return;
        }
        self.pending_delete = Some(track.path);
        self.open_modal(Modal::DeleteTrack);
    }

    pub(crate) fn confirm_delete_track(&mut self) {
        let Some(path) = self.pending_delete.take() else {
            self.close_modal();
            return;
        };
        if let Err(err) = fs::remove_file(&path) {
            self.pending_delete = Some(path);
            self.notify(format!("删除失败：{err}"));
            return;
        }
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        if let Some(index) = self.session.iter().position(|track| track.path == path) {
            let _ = self.mpv.playlist_remove(index as i64);
            self.mpv.refresh();
            self.sync_session_from_mpv();
        }
        self.forget_path(&path);
        self.rescan();
        self.close_modal();
        self.notify(format!("已删除 · {name}"));
    }

    fn remap_path(&mut self, old: &Path, new: &Path) {
        let old_key = old.to_string_lossy().to_string();
        let new_key = new.to_string_lossy().to_string();
        for item in &mut self.cfg.favorites {
            if *item == old_key {
                *item = new_key.clone();
            }
        }
        for list in &mut self.playlists {
            for song in &mut list.songs {
                if *song == old_key {
                    *song = new_key.clone();
                }
            }
        }
        for item in &mut self.cache.recent {
            if *item == old_key {
                *item = new_key.clone();
            }
        }
        if self.cache.last_path.as_deref() == Some(old_key.as_str()) {
            self.cache.last_path = Some(new_key.clone());
        }
        if let Some(value) = self.cache.plays.remove(&old_key) {
            self.cache.plays.insert(new_key.clone(), value);
        }
        if let Some(value) = self.cache.ratings.remove(&old_key) {
            self.cache.ratings.insert(new_key.clone(), value);
        }
        if let Some(value) = self.cache.listen_seconds.remove(&old_key) {
            self.cache.listen_seconds.insert(new_key.clone(), value);
        }
        for track in self.session.iter_mut().chain(self.order_session.iter_mut()) {
            if track.path == old {
                track.path = new.to_path_buf();
            }
        }
        if self.last_path.as_deref() == Some(old) {
            self.last_path = Some(new.to_path_buf());
        }
        self.cfg.playlists = self.playlists.clone();
        let _ = self.cfg.save();
        let _ = self.cache.save();
    }

    fn forget_path(&mut self, path: &Path) {
        let key = path.to_string_lossy().to_string();
        self.cfg.favorites.retain(|item| item != &key);
        for list in &mut self.playlists {
            list.songs.retain(|song| song != &key);
        }
        self.cache.recent.retain(|item| item != &key);
        self.cache.plays.remove(&key);
        self.cache.ratings.remove(&key);
        self.cache.listen_seconds.remove(&key);
        if self.cache.last_path.as_deref() == Some(key.as_str()) {
            self.cache.last_path = None;
        }
        self.session.retain(|track| track.path != path);
        self.order_session.retain(|track| track.path != path);
        if self.last_path.as_deref() == Some(path) {
            self.last_path = None;
        }
        self.cfg.playlists = self.playlists.clone();
        let _ = self.cfg.save();
        let _ = self.cache.save();
    }

    pub(crate) fn open_download(&mut self) {
        if self.dl_formatting || (!self.dl_formats.is_empty() && !self.dl_pending.is_empty()) {
            self.open_modal(Modal::DownloadFormats);
            return;
        }
        if self.dl_busy || self.dl_searching || !self.dl_candidates.is_empty() {
            self.open_modal(Modal::DownloadList);
            return;
        }
        self.open_modal(Modal::Download);
        self.input_clear();
    }

    pub(crate) fn start_download_search(&mut self) {
        if self.dl_busy || self.dl_searching || self.dl_formatting {
            self.notify("还有任务在跑喵，等它结束");
            return;
        }
        let raw = self.input.trim().to_string();
        if raw.is_empty() {
            self.notify("输入关键词、BV 号或链接喵");
            return;
        }
        if crate::download::has_multiple_ids(&raw) {
            self.notify("一次只能输入一个 id 喵");
            return;
        }
        if let Some(items) = crate::download::direct_candidates(&raw) {
            self.dl_candidates.clear();
            self.dl_checked.clear();
            self.dl_selected = 0;
            self.dl_searching = false;
            self.dl_from_search = false;
            self.begin_format_choice(items);
            return;
        }
        self.dl_candidates.clear();
        self.dl_checked.clear();
        self.dl_selected = 0;
        self.dl_from_search = true;
        self.dl_searching = true;
        self.dl_status = "正在搜索 B 站…".into();
        let ytdlp = self.cfg.ytdlp.clone();
        let cookies = crate::download::cookie_file(&self.cfg.cookies);
        let limit = self.cfg.download_limit;
        self.dl_rx = Some(crate::download::spawn_search(ytdlp, cookies, limit, raw));
        self.open_modal(Modal::DownloadList);
    }

    pub(crate) fn start_download(
        &mut self,
        items: Vec<crate::download::Candidate>,
        format: Option<String>,
    ) {
        if items.is_empty() {
            self.notify("没有选中要下载的曲目喵");
            return;
        }
        if self.dl_busy || self.dl_searching || self.dl_formatting {
            self.notify("还有任务在跑喵，等它结束");
            return;
        }
        let dir = self
            .roots()
            .into_iter()
            .next()
            .unwrap_or_else(config::default_music_dir);
        if !dir.is_dir() {
            self.notify(format!("下载目录不存在：{}", dir.display()));
            return;
        }
        let ytdlp = self.cfg.ytdlp.clone();
        let cookies = crate::download::cookie_file(&self.cfg.cookies);
        self.dl_formats.clear();
        self.dl_pending.clear();
        self.dl_busy = true;
        self.dl_progress = 0.0;
        self.dl_status = match &format {
            Some(format) => format!("准备下载 {} 首 · 格式 {format}…", items.len()),
            None => format!("准备下载 {} 首 · 自动格式…", items.len()),
        };
        self.dl_rx = Some(crate::download::spawn_download(
            ytdlp, cookies, dir, items, format,
        ));
        self.notify("开始下载 · 关掉弹窗也会继续喵");
    }

    pub(crate) fn download_selected(&mut self) {
        if self.dl_busy || self.dl_searching || self.dl_formatting {
            self.notify("等当前任务结束喵");
            return;
        }
        let picked: Vec<_> = self
            .dl_candidates
            .iter()
            .enumerate()
            .filter(|(index, _)| self.dl_checked.get(*index).copied().unwrap_or(false))
            .map(|(_, candidate)| candidate.clone())
            .collect();
        let items = if picked.is_empty() {
            self.dl_candidates
                .get(self.dl_selected)
                .cloned()
                .into_iter()
                .collect()
        } else {
            picked
        };
        if items.is_empty() {
            self.notify("先搜索，再选一首喵");
            return;
        }
        self.begin_format_choice(items);
    }

    fn begin_format_choice(&mut self, items: Vec<crate::download::Candidate>) {
        if items.is_empty() {
            self.notify("没有选中要下载的曲目喵");
            return;
        }
        self.dl_pending = items;
        self.dl_formats.clear();
        self.dl_format_selected = 0;
        self.dl_formatting = true;
        self.dl_status = "读取格式列表…".into();
        let ytdlp = self.cfg.ytdlp.clone();
        let cookies = crate::download::cookie_file(&self.cfg.cookies);
        let url = self.dl_pending[0].url.clone();
        self.dl_rx = Some(crate::download::spawn_formats(ytdlp, cookies, url));
        self.open_modal(Modal::DownloadFormats);
    }

    pub(crate) fn back_from_formats(&mut self) {
        if self.dl_from_search && !self.dl_candidates.is_empty() {
            self.open_modal(Modal::DownloadList);
        } else {
            self.dl_formats.clear();
            self.dl_pending.clear();
            self.dl_formatting = false;
            self.dl_rx = None;
            self.open_modal(Modal::Download);
        }
    }

    pub(crate) fn format_move(&mut self, delta: isize) {
        if self.dl_formats.is_empty() {
            return;
        }
        self.dl_format_selected = (self.dl_format_selected as isize + delta)
            .clamp(0, self.dl_formats.len().saturating_sub(1) as isize)
            as usize;
    }

    pub(crate) fn confirm_format_download(&mut self) {
        if self.dl_pending.is_empty() {
            self.notify("先选要下载的曲目喵");
            return;
        }
        if self.dl_busy || self.dl_searching || self.dl_formatting {
            self.notify("等格式列表读完喵");
            return;
        }
        let format = self
            .dl_formats
            .get(self.dl_format_selected)
            .filter(|format| !format.id.is_empty())
            .map(|format| format.id.clone());
        let items = std::mem::take(&mut self.dl_pending);
        self.start_download(items, format);
        self.open_modal(Modal::DownloadList);
    }

    pub(crate) fn toggle_download_check(&mut self) {
        if let Some(flag) = self.dl_checked.get_mut(self.dl_selected) {
            *flag = !*flag;
        }
    }

    pub(crate) fn toggle_all_download_checks(&mut self) {
        if self.dl_candidates.is_empty() {
            return;
        }
        let all = self.dl_checked.iter().all(|flag| *flag);
        self.dl_checked = vec![!all; self.dl_candidates.len()];
    }

    pub(crate) fn download_move(&mut self, delta: isize) {
        if self.dl_candidates.is_empty() {
            return;
        }
        self.dl_selected = (self.dl_selected as isize + delta)
            .clamp(0, self.dl_candidates.len().saturating_sub(1) as isize)
            as usize;
    }

    pub(crate) fn poll_download(&mut self) {
        let Some(rx) = self.dl_rx.take() else {
            return;
        };
        let mut keep = true;
        loop {
            match rx.try_recv() {
                Ok(crate::download::Event::SearchDone(result)) => {
                    self.dl_searching = false;
                    match result {
                        Ok(list) => {
                            self.dl_from_search = true;
                            self.dl_checked = vec![false; list.len()];
                            self.dl_selected = 0;
                            self.dl_status =
                                format!("找到 {} 个结果 · 空格勾选 · Enter 选格式", list.len());
                            self.dl_candidates = list;
                        }
                        Err(err) => {
                            self.dl_status = err.clone();
                            self.notify(err);
                        }
                    }
                }
                Ok(crate::download::Event::FormatsDone(result)) => {
                    self.dl_formatting = false;
                    let count = self.dl_pending.len();
                    let mut formats = vec![crate::download::Format::auto()];
                    match result {
                        Ok(list) => {
                            formats.extend(list);
                            self.dl_status = format!("选择格式 · 将下载 {count} 首 · Enter 确认");
                        }
                        Err(err) => {
                            self.dl_status = format!("{err} · 可先用自动格式");
                            self.notify(err);
                        }
                    }
                    self.dl_formats = formats;
                    self.dl_format_selected = self
                        .dl_format_selected
                        .min(self.dl_formats.len().saturating_sub(1));
                }
                Ok(crate::download::Event::Started {
                    index,
                    total,
                    title,
                }) => {
                    self.dl_progress = 0.0;
                    self.dl_status = format!("下载 {}/{} · {title}", index + 1, total);
                }
                Ok(crate::download::Event::Progress(percent)) => {
                    self.dl_progress = percent;
                }
                Ok(crate::download::Event::ItemDone { title, ok, message }) => {
                    self.dl_progress = 100.0;
                    self.notify(if ok {
                        format!("下载完成 · {title}")
                    } else {
                        format!("下载失败 · {title} · {message}")
                    });
                }
                Ok(crate::download::Event::AllDone { ok, failed }) => {
                    self.dl_busy = false;
                    self.dl_progress = 0.0;
                    self.dl_status = if failed == 0 {
                        format!("全部完成 · {ok} 首")
                    } else {
                        format!("完成 {ok} 首 · 失败 {failed} 首")
                    };
                    self.rescan();
                    self.notify(self.dl_status.clone());
                    keep = false;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    self.dl_searching = false;
                    self.dl_busy = false;
                    self.dl_formatting = false;
                    keep = false;
                    break;
                }
            }
        }
        if keep {
            self.dl_rx = Some(rx);
        }
    }
}

fn copy_audio_file(source: &Path, dir: &Path) -> Result<PathBuf, String> {
    let name = source
        .file_name()
        .ok_or_else(|| "文件名读不出来喵".to_string())?;
    let mut target = dir.join(name);
    if target.exists() {
        let stem = source
            .file_stem()
            .map(|stem| stem.to_string_lossy().to_string())
            .unwrap_or_else(|| "track".into());
        let ext = source
            .extension()
            .map(|ext| format!(".{}", ext.to_string_lossy()))
            .unwrap_or_default();
        for index in 2..1000 {
            target = dir.join(format!("{stem} ({index}){ext}"));
            if !target.exists() {
                break;
            }
        }
    }
    if target.exists() {
        return Err("同名文件太多喵，换个名字再添加".into());
    }
    fs::copy(source, &target).map_err(|err| format!("复制 {} 失败：{err}", source.display()))?;
    Ok(target)
}

fn copy_audio_tree(source: &Path, dir: &Path) -> Result<usize, String> {
    let mut count = 0;
    let mut stack = vec![source.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if library::is_audio(&path) {
                copy_audio_file(&path, dir)?;
                count += 1;
            }
        }
    }
    if count == 0 {
        Err("目录里没有音频文件喵".into())
    } else {
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::skip_target;

    #[test]
    fn skip_moves_exactly_one_entry() {
        assert_eq!(skip_target(1, 5, true, false), Some(2));
        assert_eq!(skip_target(1, 5, false, false), Some(0));
        assert_eq!(skip_target(4, 5, true, false), None);
        assert_eq!(skip_target(0, 5, false, false), None);
        assert_eq!(skip_target(4, 5, true, true), Some(0));
        assert_eq!(skip_target(0, 5, false, true), Some(4));
        assert_eq!(skip_target(0, 0, true, true), None);
    }
}
