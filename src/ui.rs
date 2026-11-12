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
    QueueRow(usize),
    SettingRow(usize),
    SettingLeft(usize),
    SettingRight(usize),
    ModalPanel,
}

impl Hit {
    pub fn label(self) -> &'static str {
        match self {
            Hit::Tab(0) => "全部歌曲 · 1",
            Hit::Tab(1) => "收藏 · 2",
            Hit::Tab(2) => "最近播放 · 3",
            Hit::Tab(3) => "文件夹 · 4",
            Hit::Tab(4) => "专辑 / 艺术家 · 5",
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
            Hit::QueueRow(_) => "点击播放这首",
            Hit::SettingRow(_) => "单击选中设置项",
            Hit::SettingLeft(_) | Hit::SettingRight(_) => "点击调整",
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
}

impl UiState {
    pub fn hit_at(&self, x: u16, y: u16) -> Option<Hit> {
        self.hits
            .iter()
            .rev()
            .find(|(_, rect)| rect.contains(x, y))
            .map(|(hit, _)| *hit)
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

pub fn handle_key(app: &mut crate::app::App, key: crossterm::event::KeyEvent) {
    input::handle_key(app, key);
}
pub fn handle_mouse(app: &mut crate::app::App, ui: &mut UiState, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::Moved => {
            ui.hover = ui.hit_at(mouse.column, mouse.row);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            let hit = ui.hit_at(mouse.column, mouse.row);
            ui.hover = hit;
            if app.modal.is_some() {
                match hit {
                    Some(Hit::ModalPanel) => {}
                    Some(Hit::QueueRow(index)) => {
                        app.play_queue_index(index);
                        app.close_modal();
                    }
                    Some(Hit::SettingRow(index)) => {
                        app.setting_selected = index;
                        if index == 0 {
                            app.activate_setting(index);
                        }
                    }
                    Some(Hit::SettingLeft(index)) => {
                        app.setting_selected = index;
                        app.adjust_setting(index, -1);
                    }
                    Some(Hit::SettingRight(index)) => {
                        app.setting_selected = index;
                        app.adjust_setting(index, 1);
                    }
                    _ => app.close_modal(),
                }
                return;
            }
            let Some(hit) = hit else {
                return;
            };
            match hit {
                Hit::Tab(index) => {
                    if index == 4 {
                        app.open_modal(crate::app::Modal::Browse);
                    } else {
                        let view = ["all", "fav", "recent", "folder"][index.min(3)];
                        app.set_view(view);
                    }
                }
                Hit::Row(index) => {
                    app.selected = index;
                    let now = std::time::Instant::now();
                    let double = ui
                        .last_click
                        .map(|(at, last)| last == hit && now.duration_since(at).as_millis() < 450)
                        .unwrap_or(false);
                    if double {
                        ui.last_click = None;
                        app.play_selected();
                    } else {
                        ui.last_click = Some((now, hit));
                    }
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
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if ui.dragging_volume {
                set_volume_from_mouse(app, ui, mouse.column);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => {
            ui.dragging_volume = false;
        }
        MouseEventKind::ScrollDown => {
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
                    crate::app::Modal::Browse => app.browse_move(1),
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
                    crate::app::Modal::Browse => app.browse_move(-1),
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
