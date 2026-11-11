use super::{App, EFFECTS, MODES, Modal};
use crate::config;
use crate::keymap::{self, Action};
use crate::library;
use crate::theme;
use serde_json::json;
use std::path::Path;
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
            Action::ViewAll => self.set_view("all"),
            Action::ViewFav => self.set_view("fav"),
            Action::ViewRecent => self.set_view("recent"),
            Action::ViewFolder => self.open_modal(Modal::Folders),
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
        if let Err(err) = self.mpv.load_m3u(&list, position, &self.mode) {
            self.notify(format!("mpv 播放失败：{err}"));
            return;
        }
        let title = queue[position].title.clone();
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
        let chosen = MODES[(index + 1) % MODES.len()].to_string();
        if let Err(err) = self.mpv.apply_mode(&chosen) {
            self.notify(format!("切换模式失败：{err}"));
            return;
        }
        self.mode = chosen.clone();
        self.cfg.mode = chosen;
        if self.mode == "shuffle" && !self.session.is_empty() {
            let _ = self.mpv.shuffle_playlist();
            self.mpv.refresh();
            self.sync_session_from_mpv();
        }
        self.notify(format!("播放模式 · {}", self.mode_label()));
    }

    pub(crate) fn shuffle_now(&mut self) {
        if self.session.is_empty() {
            self.notify("先播放一首歌再打乱喵");
            return;
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
            "全屏歌词 · F 或 Esc 返回"
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
        let dir = config::expand_path(&self.cfg.music_dir);
        if !dir.is_dir() {
            self.notify(format!("目录不存在：{}", dir.display()));
            return;
        }
        let current = self.current_path();
        self.tracks = library::scan(&dir);
        library::sort_tracks(&mut self.tracks, &self.sort);
        self.rebuild_visible();
        if let Some(path) = current
            && let Some(position) = self
                .visible
                .iter()
                .position(|&index| self.tracks[index].path == path)
        {
            self.selected = position;
        }
        self.notify(format!("扫描完成 · {} 首", self.tracks.len()));
    }

    pub(crate) fn set_music_dir(&mut self, raw: &str) -> Result<(), String> {
        let path = config::expand_path(raw.trim());
        if !path.is_dir() {
            return Err(format!("不是有效目录：{}", path.display()));
        }
        let path = path.canonicalize().unwrap_or(path);
        self.cfg.music_dir = path.to_string_lossy().to_string();
        self.cfg
            .save()
            .map_err(|err| format!("保存目录失败：{err}"))?;
        self.tracks = library::scan(&path);
        library::sort_tracks(&mut self.tracks, &self.sort);
        self.selected = 0;
        self.rebuild_visible();
        self.notify(format!(
            "音乐目录 · {}（{} 首）",
            path.display(),
            self.tracks.len()
        ));
        Ok(())
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
        let path = std::path::PathBuf::from(&self.cfg.music_dir).join("neko-歌单.m3u");
        match library::write_m3u(&path, &tracks) {
            Ok(()) => self.notify(format!("已导出 {} 首 · {}", tracks.len(), path.display())),
            Err(err) => self.notify(format!("导出失败：{err}")),
        }
    }

    pub(crate) fn open_modal(&mut self, modal: Modal) {
        if modal == Modal::Search {
            self.search_original = Some(self.filter.clone());
        }
        self.modal = Some(modal);
        self.modal_scroll = 0;
        self.capture_key = false;
        match modal {
            Modal::Search => self.input = self.filter.clone(),
            Modal::Directory => self.input = self.cfg.music_dir.clone(),
            Modal::Settings => {
                self.setting_selected = self.setting_selected.min(self.settings_rows().len() - 1);
                self.input.clear();
            }
            Modal::Playlists => {
                self.playlist_selected = self
                    .playlist_selected
                    .min(self.playlists.len().saturating_sub(1));
                self.input.clear();
            }
            Modal::PlaylistTracks => {
                self.playlist_song_selected = 0;
                self.input.clear();
            }
            Modal::PlaylistName => {}
            Modal::Folders => {
                let folders = self.folder_list();
                self.folder_selected = folders
                    .iter()
                    .position(|(name, _)| *name == self.folder)
                    .unwrap_or(0);
                self.input.clear();
            }
            Modal::Equalizer => {
                self.eq_selected = 0;
                self.input.clear();
            }
            Modal::AudioDevice => {
                self.devices = self.mpv.audio_devices();
                self.device_selected = self
                    .devices
                    .iter()
                    .position(|(name, _)| *name == self.cfg.audio_device)
                    .unwrap_or(0);
                self.input.clear();
            }
            Modal::Stats => {
                self.stats_scroll = 0;
                self.input.clear();
            }
            Modal::Queue => {
                self.queue_selected = self
                    .queue_selected
                    .min(self.session.len().saturating_sub(1));
                self.input.clear();
            }
            Modal::Keybindings => {
                self.keybind_selected = self
                    .keybind_selected
                    .min(keymap::ACTIONS.len().saturating_sub(1));
                self.input.clear();
            }
            _ => self.input.clear(),
        }
    }

    pub(crate) fn close_modal(&mut self) {
        self.modal = None;
        self.input.clear();
        self.modal_scroll = 0;
        self.search_original = None;
        self.capture_key = false;
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
            self.modal = None;
            self.input.clear();
            self.modal_scroll = 0;
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
            Modal::Directory => {
                let raw = self.input.trim().to_string();
                if raw.is_empty() {
                    self.notify("目录不能为空喵");
                    return;
                }
                if let Err(err) = self.set_music_dir(&raw) {
                    self.notify(err);
                    return;
                }
                self.close_modal();
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
        self.input.clear();
        self.open_modal(Modal::PlaylistName);
    }

    pub(crate) fn begin_playlist_rename(&mut self) {
        let Some(list) = self.playlists.get(self.playlist_selected) else {
            self.notify("没有选中的歌单喵");
            return;
        };
        self.playlist_name_target = Some(self.playlist_selected);
        self.input = list.name.clone();
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
        library::folders(&self.tracks, Path::new(&self.cfg.music_dir))
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
        vec![
            ("音乐目录".into(), self.cfg.music_dir.clone()),
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
        ]
    }

    pub(crate) fn move_setting(&mut self, delta: isize) {
        let count = self.settings_rows().len();
        self.setting_selected = (self.setting_selected as isize + delta)
            .clamp(0, count.saturating_sub(1) as isize) as usize;
    }

    pub(crate) fn adjust_setting(&mut self, index: usize, delta: isize) {
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
                    self.open_modal(Modal::Keybindings);
                }
            }
            16 => {
                if delta != 0 {
                    self.open_modal(Modal::AudioDevice);
                }
            }
            17 => {
                if delta != 0 {
                    self.clear_recent();
                }
            }
            18 | 19 => {}
            _ => {}
        }
    }

    pub(crate) fn activate_setting(&mut self, index: usize) {
        if index == 0 {
            self.open_modal(Modal::Directory);
        } else {
            self.adjust_setting(index, 1);
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
        self.notify("队列顺序已更新");
    }

    pub(crate) fn queue_remove(&mut self) {
        if self.session.is_empty() {
            return;
        }
        let index = self.queue_selected;
        let removed = self.session[index].title.clone();
        if let Err(err) = self.mpv.playlist_remove(index as i64) {
            self.notify(format!("移除失败：{err}"));
            return;
        }
        self.mpv.refresh();
        self.sync_session_from_mpv();
        self.queue_selected = self
            .queue_selected
            .min(self.session.len().saturating_sub(1));
        self.notify(format!("已移出队列 · {removed}"));
    }

    pub(crate) fn queue_clear(&mut self) {
        if self.session.is_empty() {
            return;
        }
        let _ = self.mpv.stop();
        let _ = self.mpv.playlist_clear();
        self.mpv.refresh();
        self.session.clear();
        self.queue_selected = 0;
        self.last_path = None;
        self.notify("队列已清空");
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
