mod draw;
mod input;

use crossterm::cursor::{Hide, Show};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use crossterm::execute;
use crossterm::terminal::{
    self, EnterAlternateScreen, LeaveAlternateScreen, SetTitle, disable_raw_mode, enable_raw_mode,
};
use std::io::{self, Stdout, Write};
use unicode_width::UnicodeWidthChar;

use crate::app::Modal;
pub use crate::theme::Rgb;
use crate::theme::*;

pub const NOTE: &str = "\u{f001}";

pub const PREV: &str = "\u{f048}";
pub const NEXT: &str = "\u{f051}";
pub const HEART_O: &str = "\u{f08a}";
pub const GEAR: &str = "\u{f013}";
pub const POWER: &str = "\u{f011}";
pub const REPEAT: &str = "\u{f01e}";
pub const RANDOM: &str = "\u{f074}";
pub const PLAY: &str = "\u{f04b}";
pub const PAUSE: &str = "\u{f04c}";
pub const HEART: &str = "\u{f004}";
pub const SEARCH: &str = "\u{f002}";
pub const VOL: &str = "\u{f028}";
pub const MUTE: &str = "\u{f026}";
pub const LIST: &str = "\u{f03a}";
pub const CLOCK: &str = "\u{f017}";
pub const PAW: &str = "\u{f1b0}";

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Style {
    pub fg: Rgb,
    pub bg: Rgb,
    pub bold: bool,
}

impl Style {
    pub const fn new(fg: Rgb, bg: Rgb) -> Self {
        Self {
            fg,
            bg,
            bold: false,
        }
    }

    pub const fn bold(mut self) -> Self {
        self.bold = true;
        self
    }
}

#[derive(Clone, Copy)]
struct Cell {
    ch: char,
    style: Style,
    continuation: bool,
}

pub struct Canvas {
    width: usize,
    height: usize,
    cells: Vec<Cell>,
    transparent: bool,
}

impl Canvas {
    pub fn new(width: usize, height: usize, style: Style, transparent: bool) -> Self {
        Self {
            width,
            height,
            cells: vec![
                Cell {
                    ch: ' ',
                    style,
                    continuation: false,
                };
                width * height
            ],
            transparent,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    fn set(&mut self, x: usize, y: usize, ch: char, style: Style) {
        if x < self.width && y < self.height {
            let index = y * self.width + x;
            self.cells[index] = Cell {
                ch,
                style,
                continuation: false,
            };
        }
    }

    pub fn put(&mut self, x: usize, y: usize, text: &str, style: Style) -> usize {
        if y >= self.height {
            return x;
        }
        let mut column = x;
        for ch in text.chars() {
            let width = UnicodeWidthChar::width(ch).unwrap_or(0);
            if width == 0 {
                if column > 0 {
                    let previous = y * self.width + column - 1;
                    if !self.cells[previous].continuation {
                        self.cells[previous].ch = ch;
                    }
                }
                continue;
            }
            if column + width > self.width {
                break;
            }
            self.set(column, y, ch, style);
            for extra in 1..width {
                let index = y * self.width + column + extra;
                self.cells[index] = Cell {
                    ch: ' ',
                    style,
                    continuation: true,
                };
            }
            column += width;
        }
        column
    }

    pub fn fill(&mut self, x: usize, y: usize, width: usize, height: usize, style: Style) {
        for row in y..(y + height).min(self.height) {
            for column in x..(x + width).min(self.width) {
                self.set(column, row, ' ', style);
            }
        }
    }

    pub fn border(
        &mut self,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
        title: &str,
        style: Style,
    ) {
        if width < 2 || height < 2 {
            return;
        }
        let right = x + width - 1;
        let bottom = y + height - 1;
        self.put(x, y, "╭", style);
        self.put(right, y, "╮", style);
        self.put(x, bottom, "╰", style);
        self.put(right, bottom, "╯", style);
        for column in x + 1..right {
            self.put(column, y, "─", style);
            self.put(column, bottom, "─", style);
        }
        for row in y + 1..bottom {
            self.put(x, row, "│", style);
            self.put(right, row, "│", style);
        }
        if !title.is_empty() {
            self.put(x + 2, y, &format!(" {title} "), style);
        }
    }

    pub fn centered(&mut self, row: usize, text: &str, style: Style) {
        let width = text_width(text);
        let x = self.width.saturating_sub(width) / 2;
        self.put(x, row, text, style);
    }

    fn render(&self) -> String {
        let emit_style = |output: &mut String, style: Style| {
            output.push_str(&format!(
                "\x1b[38;2;{};{};{}m",
                style.fg.0, style.fg.1, style.fg.2
            ));
            if self.transparent
                && (style.bg == base() || style.bg == mantle() || style.bg == crust())
            {
                output.push_str("\x1b[49m");
            } else {
                output.push_str(&format!(
                    "\x1b[48;2;{};{};{}m",
                    style.bg.0, style.bg.1, style.bg.2
                ));
            }
        };
        let mut output = String::with_capacity(self.width * self.height * 2);
        for row in 0..self.height {
            output.push_str(&format!("\x1b[{};1H\x1b[0m", row + 1));
            let mut style = Style::new(txt(), base());
            let mut active = false;
            for column in 0..self.width {
                let cell = self.cells[row * self.width + column];
                if cell.continuation {
                    continue;
                }
                if cell.style != style {
                    style = cell.style;
                    output.push_str("\x1b[0m");
                    if style.bold {
                        output.push_str("\x1b[1m");
                    }
                    emit_style(&mut output, style);
                    active = true;
                }
                if !active {
                    emit_style(&mut output, style);
                    active = true;
                }
                output.push(cell.ch);
            }
            output.push_str("\x1b[0m\x1b[K");
        }
        output
    }
}

pub fn text_width(text: &str) -> usize {
    text.chars()
        .map(|ch| UnicodeWidthChar::width(ch).unwrap_or(0))
        .sum()
}

pub fn truncate(text: &str, width: usize) -> String {
    let mut result = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > width {
            break;
        }
        result.push(ch);
        used += char_width;
    }
    result
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Rect {
    pub const fn new(x: usize, y: usize, width: usize, height: usize) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, x: u16, y: u16) -> bool {
        let (x, y) = (x as usize, y as usize);
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalButton {
    Close,
    Confirm,
    Cancel,
    New,
    Rename,
    Delete,
    Add,
    Play,
    Reset,
    Clear,
    Apply,
    Refresh,
    Capture,
    MoveUp,
    MoveDown,
    Remove,
    Open,
}

impl ModalButton {
    pub fn label(self) -> &'static str {
        match self {
            Self::Close => "关闭 Esc",
            Self::Confirm => "确定 Enter",
            Self::Cancel => "取消 Esc",
            Self::New => "新建 n",
            Self::Rename => "改名 r",
            Self::Delete => "删除 x",
            Self::Add => "加入 a",
            Self::Play => "播放 Enter",
            Self::Reset => "恢复 r",
            Self::Clear => "清空 x",
            Self::Apply => "应用 Enter",
            Self::Refresh => "重读 r",
            Self::Capture => "改键 Enter",
            Self::MoveUp => "上移 K",
            Self::MoveDown => "下移 J",
            Self::Remove => "移出 d",
            Self::Open => "打开 Enter",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit {
    Tab(usize),
    Row(usize),
    PlayPause,
    Prev,
    Next,
    Mode,
    Shuffle,
    Favorite,
    Search,
    Settings,
    Help,
    Queue,
    Quit,
    Progress,
    Volume,
    MuteButton,
    SeekBack,
    SeekForward,
    Time,
    Scrollbar,
    ModeChip,
    SpeedChip,
    PauseChip,
    SleepChip,
    QueueChip,
    AbChip,
    EqChip,
    NextPreview,
    TrackInfo,
    LyricsFull,
    LyricsFullBack,
    QueueRow(usize),
    SettingRow(usize),
    SettingLeft(usize),
    SettingRight(usize),
    ModalRow(usize),
    EqualizerRow(usize),
    EqualizerLeft(usize),
    EqualizerRight(usize),
    ModalButton(ModalButton),
    ModalPanel,
}

impl Hit {
    pub fn label(self) -> &'static str {
        match self {
            Hit::Tab(0) => "全部歌曲 · 1",
            Hit::Tab(1) => "收藏 · 2",
            Hit::Tab(2) => "最近播放 · 3",
            Hit::Tab(3) => "文件夹 · 4",
            Hit::Tab(_) => "视图",
            Hit::Row(_) => "单击选中 · 再点一次播放",
            Hit::PlayPause => "播放 / 暂停 · 空格",
            Hit::Prev => "上一首 · p",
            Hit::Next => "下一首 · n",
            Hit::Mode => "循环模式 · L",
            Hit::Shuffle => "打乱队列 · S",
            Hit::Favorite => "收藏选中曲目 · f",
            Hit::Search => "搜索 · /",
            Hit::Settings => "设置 · c",
            Hit::Help => "快捷键 · ?",
            Hit::Queue => "播放队列 · u",
            Hit::Quit => "退出 neko · q",
            Hit::Progress => "点击跳转进度",
            Hit::Volume => "点击或拖动调节音量",
            Hit::MuteButton => "静音开关 · m",
            Hit::SeekBack => "后退 5 秒 · ←",
            Hit::SeekForward => "前进 5 秒 · →",
            Hit::Time => "点击切换已播放 / 剩余时间",
            Hit::Scrollbar => "点击或拖动跳转列表",
            Hit::ModeChip => "切换播放模式 · L",
            Hit::SpeedChip => "恢复 1.00x · 0",
            Hit::PauseChip => "播放 / 暂停 · 空格",
            Hit::SleepChip => "睡眠定时 · t",
            Hit::QueueChip => "打开播放队列 · u",
            Hit::AbChip => "A-B 循环 · a",
            Hit::EqChip => "打开均衡器 · E",
            Hit::NextPreview => "下一首 · n",
            Hit::TrackInfo => "曲目详情 · i",
            Hit::LyricsFull => "全屏歌词 · F",
            Hit::LyricsFullBack => "返回 · Esc",
            Hit::QueueRow(_) => "单击选中 · 再点一次播放",
            Hit::SettingRow(_) => "单击选中 · 双击调整",
            Hit::SettingLeft(_) | Hit::SettingRight(_) => "点击调整",
            Hit::ModalRow(_) => "单击选中 · 双击确认",
            Hit::EqualizerRow(_) => "单击选中 · 双击归零",
            Hit::EqualizerLeft(_) => "降低 1 dB",
            Hit::EqualizerRight(_) => "提高 1 dB",
            Hit::ModalButton(button) => button.label(),
            Hit::ModalPanel => "窗口",
        }
    }
}

#[derive(Default)]
pub struct UiState {
    pub hover: Option<Hit>,
    pub hits: Vec<(Hit, Rect)>,
    pub last_click: Option<(std::time::Instant, Hit)>,
    pub dragging_volume: bool,
    pub dragging_scrollbar: bool,
}

impl UiState {
    pub fn hit_at(&self, x: u16, y: u16) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, rect)| rect.contains(x, y))
            .map(|(hit, _)| *hit)
    }

    fn double_click(&mut self, hit: Hit) -> bool {
        let now = std::time::Instant::now();
        let double = self
            .last_click
            .map(|(at, last)| last == hit && now.duration_since(at).as_millis() < 450)
            .unwrap_or(false);
        self.last_click = if double { None } else { Some((now, hit)) };
        double
    }
}

pub struct Terminal {
    stdout: Stdout,
    title: String,
}

impl Terminal {
    pub fn enter() -> io::Result<Self> {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let _ = disable_raw_mode();
            let mut stdout = io::stdout();
            let _ = execute!(stdout, Show, DisableMouseCapture, LeaveAlternateScreen);
            previous(info);
        }));
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, Hide, EnableMouseCapture)?;
        Ok(Self {
            stdout,
            title: String::new(),
        })
    }

    pub fn draw(&mut self, app: &crate::app::App, ui: &mut UiState) -> io::Result<()> {
        let title = match app.current_path() {
            Some(path) => format!(
                "▶ {} · neko",
                path.file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_else(|| "neko".into())
            ),
            None => "neko · 本地音乐".to_string(),
        };
        if title != self.title {
            execute!(self.stdout, SetTitle(&title))?;
            self.title = title;
        }
        let (width, height) = terminal::size()?;
        let mut canvas = Canvas::new(
            width as usize,
            height as usize,
            Style::new(txt(), base()),
            app.transparent,
        );
        let hover = ui.hover;
        let mut hits = Vec::new();
        draw::render(&mut canvas, app, hover, &mut hits);
        ui.hits = hits;
        self.stdout.write_all(canvas.render().as_bytes())?;
        self.stdout.flush()
    }

    pub fn leave(&mut self) {
        let _ = execute!(self.stdout, Show, DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        self.leave();
    }
}

pub fn handle_key(app: &mut crate::app::App, ui: &mut UiState, key: crossterm::event::KeyEvent) {
    let modal = app.modal;
    let lyrics_full = app.lyrics_full;
    ui.last_click = None;
    input::handle_key(app, key);
    if app.modal != modal || app.lyrics_full != lyrics_full {
        ui.hover = None;
    }
}
pub fn handle_mouse(app: &mut crate::app::App, ui: &mut UiState, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Moved => {
            ui.hover = ui.hit_at(mouse.column, mouse.row);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let hit = ui.hit_at(mouse.column, mouse.row);
            let modal = app.modal;
            let lyrics_full = app.lyrics_full;
            let modal_button = matches!(hit, Some(Hit::ModalButton(_)));
            let double_clickable = matches!(
                hit,
                Some(
                    Hit::Row(_)
                        | Hit::QueueRow(_)
                        | Hit::SettingRow(_)
                        | Hit::ModalRow(_)
                        | Hit::EqualizerRow(_)
                )
            );
            ui.hover = hit;
            if app.modal.is_some() {
                match hit {
                    Some(hit) => handle_modal_click(app, ui, hit),
                    None => app.close_modal(),
                }
                if modal_button || app.modal != modal {
                    ui.hover = None;
                }
                if !double_clickable {
                    ui.last_click = None;
                }
                return;
            }
            let Some(hit) = hit else {
                ui.last_click = None;
                return;
            };
            match hit {
                Hit::Tab(index) => {
                    if index == 3 {
                        app.open_modal(Modal::Folders);
                    } else {
                        let view = ["all", "fav", "recent"][index.min(2)];
                        app.set_view(view);
                    }
                }
                Hit::Row(index) => {
                    app.selected = index;
                    if ui.double_click(hit) {
                        app.play_selected();
                    }
                }
                Hit::Scrollbar => {
                    ui.dragging_scrollbar = true;
                    jump_scrollbar(app, ui, mouse.row);
                }
                Hit::ModeChip => app.cycle_mode(),
                Hit::SpeedChip => app.reset_speed(),
                Hit::PauseChip => app.play_or_toggle(),
                Hit::SleepChip => app.cycle_sleep(),
                Hit::QueueChip => app.open_modal(Modal::Queue),
                Hit::AbChip => app.cycle_ab(),
                Hit::EqChip => app.open_modal(Modal::Equalizer),
                Hit::NextPreview => app.skip(true),
                Hit::TrackInfo => app.open_modal(Modal::Info),
                Hit::LyricsFull => app.toggle_full_lyrics(),
                Hit::LyricsFullBack => {
                    app.lyrics_full = false;
                    app.notify("退出全屏歌词");
                }
                Hit::PlayPause => app.play_or_toggle(),
                Hit::Prev => app.skip(false),
                Hit::Next => app.skip(true),
                Hit::Mode => app.cycle_mode(),
                Hit::Shuffle => app.shuffle_now(),
                Hit::Favorite => app.toggle_favorite(),
                Hit::Search => app.open_modal(crate::app::Modal::Search),
                Hit::Settings => app.open_modal(crate::app::Modal::Settings),
                Hit::Help => app.open_modal(crate::app::Modal::Help),
                Hit::Queue => app.open_modal(crate::app::Modal::Queue),
                Hit::Quit => app.quit = true,
                Hit::Progress => {
                    if let Some((_, rect)) = ui.hits.iter().rev().find(|(h, _)| *h == Hit::Progress)
                    {
                        let fraction = if rect.width == 0 {
                            0.0
                        } else {
                            ((mouse.column as usize).saturating_sub(rect.x) as f64)
                                / rect.width as f64
                        };
                        let duration = app.mpv.state.duration.unwrap_or(0.0);
                        if duration > 0.0 {
                            app.seek_absolute(fraction.clamp(0.0, 1.0) * duration);
                        }
                    }
                }
                Hit::Volume => {
                    ui.dragging_volume = true;
                    set_volume_from_mouse(app, ui, mouse.column);
                }
                Hit::MuteButton => app.toggle_mute(),
                Hit::SeekBack => app.seek_relative(-5.0),
                Hit::SeekForward => app.seek_relative(5.0),
                Hit::Time => app.toggle_remaining(),
                _ => {}
            }
            // 点击可能让控件消失，清掉悬停避免底部提示残留。
            if app.modal != modal || app.lyrics_full != lyrics_full {
                ui.hover = None;
            }
            if !double_clickable {
                ui.last_click = None;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if ui.dragging_volume {
                set_volume_from_mouse(app, ui, mouse.column);
            } else if ui.dragging_scrollbar {
                jump_scrollbar(app, ui, mouse.row);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            ui.dragging_volume = false;
            ui.dragging_scrollbar = false;
        }
        MouseEventKind::ScrollDown => {
            ui.last_click = None;
            if app.lyrics_full {
                return;
            }
            if ui.hover == Some(Hit::Volume) {
                app.volume_delta(-5.0);
                return;
            }
            if let Some(modal) = app.modal {
                match modal {
                    crate::app::Modal::Settings => {
                        app.setting_selected =
                            (app.setting_selected + 1).min(app.settings_rows().len() - 1);
                    }
                    crate::app::Modal::Queue => {
                        app.queue_selected =
                            (app.queue_selected + 1).min(app.session.len().saturating_sub(1));
                    }
                    crate::app::Modal::Playlists => app.playlist_move(1),
                    crate::app::Modal::PlaylistTracks => {
                        let count = app
                            .playlists
                            .get(app.playlist_selected)
                            .map(|list| list.songs.len())
                            .unwrap_or(0);
                        app.playlist_song_selected =
                            (app.playlist_song_selected + 1).min(count.saturating_sub(1));
                    }
                    crate::app::Modal::Folders => app.folder_move(1),
                    crate::app::Modal::Directory => app.dir_move(1),
                    crate::app::Modal::Equalizer => {
                        app.eq_selected = (app.eq_selected + 1).min(crate::app::EQ_BANDS.len());
                    }
                    crate::app::Modal::AudioDevice => app.device_move(1),
                    crate::app::Modal::Stats => app.stats_scroll += 1,
                    crate::app::Modal::Keybindings => {
                        app.keybind_selected =
                            (app.keybind_selected + 1).min(crate::keymap::ACTIONS.len() - 1);
                    }
                    _ => app.modal_scroll += 3,
                }
            } else {
                app.move_selection(3);
                ui.hover = ui.hit_at(mouse.column, mouse.row);
            }
        }
        MouseEventKind::ScrollUp => {
            ui.last_click = None;
            if app.lyrics_full {
                return;
            }
            if ui.hover == Some(Hit::Volume) {
                app.volume_delta(5.0);
                return;
            }
            if let Some(modal) = app.modal {
                match modal {
                    crate::app::Modal::Settings => {
                        app.setting_selected = app.setting_selected.saturating_sub(1);
                    }
                    crate::app::Modal::Queue => {
                        app.queue_selected = app.queue_selected.saturating_sub(1);
                    }
                    crate::app::Modal::Playlists => app.playlist_move(-1),
                    crate::app::Modal::PlaylistTracks => {
                        app.playlist_song_selected = app.playlist_song_selected.saturating_sub(1);
                    }
                    crate::app::Modal::Folders => app.folder_move(-1),
                    crate::app::Modal::Directory => app.dir_move(-1),
                    crate::app::Modal::Equalizer => {
                        app.eq_selected = app.eq_selected.saturating_sub(1);
                    }
                    crate::app::Modal::AudioDevice => app.device_move(-1),
                    crate::app::Modal::Stats => {
                        app.stats_scroll = app.stats_scroll.saturating_sub(1);
                    }
                    crate::app::Modal::Keybindings => {
                        app.keybind_selected = app.keybind_selected.saturating_sub(1);
                    }
                    _ => app.modal_scroll = app.modal_scroll.saturating_sub(3),
                }
            } else {
                app.move_selection(-3);
                ui.hover = ui.hit_at(mouse.column, mouse.row);
            }
        }
        _ => {}
    }
}

fn handle_modal_click(app: &mut crate::app::App, ui: &mut UiState, hit: Hit) {
    match hit {
        Hit::ModalPanel => {}
        Hit::QueueRow(index) => {
            app.queue_selected = index;
            if ui.double_click(hit) {
                app.play_queue_index(index);
                app.close_modal();
            }
        }
        Hit::SettingRow(index) => {
            app.setting_selected = index;
            if ui.double_click(hit) {
                app.activate_setting(index);
            }
        }
        Hit::SettingLeft(index) => {
            app.setting_selected = index;
            app.adjust_setting(index, -1);
        }
        Hit::SettingRight(index) => {
            app.setting_selected = index;
            app.adjust_setting(index, 1);
        }
        Hit::ModalRow(index) => {
            select_modal_row(app, index);
            if ui.double_click(hit) {
                activate_modal_row(app);
            }
        }
        Hit::EqualizerRow(index) => {
            app.eq_selected = index.min(crate::app::EQ_BANDS.len());
            if ui.double_click(hit) {
                app.activate_eq_row();
            }
        }
        Hit::EqualizerLeft(index) => {
            app.eq_selected = index.min(crate::app::EQ_BANDS.len());
            app.adjust_eq(index, -1.0);
        }
        Hit::EqualizerRight(index) => {
            app.eq_selected = index.min(crate::app::EQ_BANDS.len());
            app.adjust_eq(index, 1.0);
        }
        Hit::ModalButton(button) => handle_modal_button(app, button),
        _ => app.close_modal(),
    }
}

fn select_modal_row(app: &mut crate::app::App, index: usize) {
    match app.modal {
        Some(Modal::Folders) => {
            app.folder_selected = index.min(app.folder_list().len().saturating_sub(1));
        }
        Some(Modal::Directory) => {
            app.dir_selected = index.min(app.cfg.music_dirs.len().saturating_sub(1));
        }
        Some(Modal::Playlists) => {
            app.playlist_selected = index.min(app.playlists.len().saturating_sub(1));
        }
        Some(Modal::PlaylistTracks) => {
            let count = app
                .playlists
                .get(app.playlist_selected)
                .map(|list| list.songs.len())
                .unwrap_or(0);
            app.playlist_song_selected = index.min(count.saturating_sub(1));
        }
        Some(Modal::AudioDevice) => {
            app.device_selected = index.min(app.devices.len().saturating_sub(1));
        }
        Some(Modal::Keybindings) => {
            app.keybind_selected = index.min(crate::keymap::ACTIONS.len().saturating_sub(1));
        }
        _ => {}
    }
}

fn activate_modal_row(app: &mut crate::app::App) {
    match app.modal {
        Some(Modal::Folders) => app.select_folder(),
        Some(Modal::Directory) => app.rescan(),
        Some(Modal::Playlists) => app.open_playlist_tracks(),
        Some(Modal::PlaylistTracks) => app.play_playlist(app.playlist_song_selected),
        Some(Modal::AudioDevice) => app.apply_device(),
        Some(Modal::Keybindings) => app.begin_capture_binding(),
        Some(Modal::Equalizer) => app.activate_eq_row(),
        Some(Modal::Queue) => app.play_queue_index(app.queue_selected),
        Some(Modal::Settings) => app.activate_setting(app.setting_selected),
        _ => {}
    }
}

fn handle_modal_button(app: &mut crate::app::App, button: ModalButton) {
    let Some(modal) = app.modal else {
        return;
    };
    match button {
        ModalButton::Close => app.close_modal(),
        ModalButton::Confirm => app.submit_modal(),
        ModalButton::Cancel => app.cancel_modal(),
        ModalButton::New => app.begin_playlist_create(),
        ModalButton::Rename => app.begin_playlist_rename(),
        ModalButton::Delete => app.delete_playlist(),
        ModalButton::Add => app.add_to_playlist(),
        ModalButton::Play => {
            if modal == Modal::PlaylistTracks {
                app.play_playlist(app.playlist_song_selected);
            }
        }
        ModalButton::Reset => match modal {
            Modal::Equalizer => app.reset_eq(),
            Modal::Keybindings => {
                if let Some(action) = crate::keymap::ACTIONS.get(app.keybind_selected) {
                    app.reset_binding(*action);
                }
            }
            _ => {}
        },
        ModalButton::Clear => match modal {
            Modal::Stats => app.clear_stats(),
            Modal::Queue => app.queue_clear(),
            _ => {}
        },
        ModalButton::Apply => {
            if modal == Modal::AudioDevice {
                app.apply_device();
            }
        }
        ModalButton::Refresh => {
            if modal == Modal::AudioDevice {
                app.open_modal(Modal::AudioDevice);
            }
        }
        ModalButton::Capture => app.begin_capture_binding(),
        ModalButton::MoveUp => {
            if modal == Modal::Queue {
                app.queue_move(-1);
            }
        }
        ModalButton::MoveDown => {
            if modal == Modal::Queue {
                app.queue_move(1);
            }
        }
        ModalButton::Remove => match modal {
            Modal::Queue => app.queue_remove(),
            Modal::PlaylistTracks => app.remove_playlist_song(),
            _ => {}
        },
        ModalButton::Open => activate_modal_row(app),
    }
}

fn jump_scrollbar(app: &mut crate::app::App, ui: &UiState, row: u16) {
    let Some((_, rect)) = ui.hits.iter().rev().find(|(hit, _)| *hit == Hit::Scrollbar) else {
        return;
    };
    if rect.height == 0 {
        return;
    }
    let total = app.visible.len();
    let rows = rect.height;
    let max_offset = total.saturating_sub(rows);
    let fraction =
        ((row as usize).saturating_sub(rect.y) as f64 / rect.height as f64).clamp(0.0, 1.0);
    let top = (fraction * max_offset as f64).round() as usize;
    app.selected = (top + rows / 2).min(total.saturating_sub(1));
}

fn set_volume_from_mouse(app: &mut crate::app::App, ui: &UiState, column: u16) {
    if let Some((_, rect)) = ui.hits.iter().rev().find(|(hit, _)| *hit == Hit::Volume) {
        let fraction = if rect.width == 0 {
            0.0
        } else {
            ((column as usize).saturating_sub(rect.x) as f64) / rect.width as f64
        };
        app.set_volume(fraction.clamp(0.0, 1.0) * 100.0);
    }
}
