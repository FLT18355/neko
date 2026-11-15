use crate::config::{self, Config};
use crate::library;
use crate::lyrics::{self, Lrc};
use crate::mpv::Mpv;
use crate::theme::{self, Rgb};
use crate::ui::{CLOCK, MUTE, NEXT, NOTE, PAUSE, PAW, PLAY, VOL, text_width, truncate};
use crossterm::cursor::{Hide, MoveToPreviousLine, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{
    Attribute, Color, Print, ResetColor, SetAttribute, SetBackgroundColor, SetForegroundColor,
};
use crossterm::terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size};
use crossterm::{execute, queue};
use serde_json::json;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

const LYRIC_ROWS: usize = 5;
const POLL_MS: u64 = 100;

pub fn usage() {
    println!("neko cli · 中文喵化终端播放器");
    println!();
    println!("用法：");
    println!("  neko cli [音乐文件或目录 ...]");
    println!();
    println!("不给路径时使用配置里的音乐目录。按键：");
    println!("  空格 播放/暂停    n/p 下一首/上一首");
    println!("  ←/→ 快退/快进 5 秒    Ctrl+←/→ 30 秒");
    println!("  -/= 音量    m 静音    l 歌词    r 播放模式    q 退出");
    println!();
    println!("列表播完自动退出。");
}

pub fn run(args: &[String]) -> Result<(), String> {
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        usage();
        return Ok(());
    }
    let cfg = Config::load();
    theme::set(&cfg.theme);
    let mut raw = Vec::new();
    for arg in args {
        if arg.starts_with('-') {
            return Err(format!(
                "不认识的参数：{arg}（用 neko cli --help 看看用法）"
            ));
        }
        raw.push(arg.clone());
    }
    let targets = config::resolve_dirs(&raw, &cfg);
    let mut files = collect(&targets)?;
    if files.is_empty() {
        return Err("没有找到可播放的音频文件喵".into());
    }
    files.sort();
    files.dedup();

    let list = config::runtime_dir().join(format!("neko-cli-{}.m3u", std::process::id()));
    let mut text = String::from("#EXTM3U\n");
    for file in &files {
        text.push_str(&file.to_string_lossy());
        text.push('\n');
    }
    fs::write(&list, text).map_err(|err| format!("写入播放列表失败喵：{err}"))?;
    let result = play(&cfg, &list, &files);
    let _ = fs::remove_file(&list);
    result
}

fn collect(targets: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    for target in targets {
        if target.is_dir() {
            files.extend(
                library::scan_dirs(std::slice::from_ref(target))
                    .into_iter()
                    .map(|track| track.path),
            );
        } else if library::is_audio(target) {
            files.push(target.clone());
        } else if target.is_file() {
            return Err(format!("不是支持的音频格式喵：{}", target.display()));
        } else {
            return Err(format!("文件或目录不存在喵：{}", target.display()));
        }
    }
    Ok(files)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Order,
    Loop,
    Single,
    Shuffle,
}

impl Mode {
    fn key(self) -> &'static str {
        match self {
            Self::Order => "order",
            Self::Loop => "loop",
            Self::Single => "single",
            Self::Shuffle => "shuffle",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Order => "顺序播放",
            Self::Loop => "列表循环",
            Self::Single => "单曲循环",
            Self::Shuffle => "随机播放",
        }
    }

    fn next(self) -> Self {
        match self {
            Self::Order => Self::Loop,
            Self::Loop => Self::Single,
            Self::Single => Self::Shuffle,
            Self::Shuffle => Self::Order,
        }
    }
}

struct Span {
    text: String,
    color: Rgb,
    bg: Option<Rgb>,
    bold: bool,
}

impl Span {
    fn new(text: impl Into<String>, color: Rgb) -> Self {
        Self {
            text: text.into(),
            color,
            bg: None,
            bold: false,
        }
    }

    fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    fn bg(mut self, color: Rgb) -> Self {
        self.bg = Some(color);
        self
    }
}

type Line = Vec<Span>;

fn line_width(spans: &[Span]) -> usize {
    spans.iter().map(|span| text_width(&span.text)).sum()
}

fn text_line(text: impl Into<String>, color: Rgb) -> Line {
    vec![Span::new(text, color)]
}

struct Screen {
    rendered: usize,
    lrc: Option<Lrc>,
    lrc_path: Option<PathBuf>,
    last_pos: Option<f64>,
}

impl Screen {
    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        mpv: &Mpv,
        width: usize,
        height: usize,
        files: &[PathBuf],
        mode: Mode,
        show_lyrics: bool,
    ) -> Vec<Line> {
        let state = &mpv.state;
        if self.lrc_path != state.path {
            self.lrc_path = state.path.clone();
            self.lrc = self.lrc_path.as_deref().and_then(Lrc::load_for);
        }

        let metadata = &state.metadata;
        let title = metadata
            .get("title")
            .cloned()
            .or_else(|| {
                state
                    .path
                    .as_deref()
                    .and_then(Path::file_stem)
                    .map(|stem| stem.to_string_lossy().to_string())
            })
            .unwrap_or_else(|| "本喵在等音乐喵…".into());
        let info = track_info(state);
        let time = state.time_pos.unwrap_or(0.0);
        let duration = state.duration.unwrap_or(0.0);
        let ratio = if duration > 0.0 {
            (time / duration).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let mut lines = Vec::new();
        lines.push(header_line(width, state));
        lines.push(rainbow_line(width));
        lines.push(title_line(&title, state.pause, width));
        if let Some(info) = &info {
            lines.push(text_line(
                truncate(&format!("   {info}"), width),
                theme::sub0(),
            ));
        }
        lines.push(progress_line(width, ratio));
        lines.push(status_line(width, state, mode, ratio));

        let lyrics_header = if show_lyrics {
            format!(" {NOTE} 歌词")
        } else {
            format!(" {NOTE} 歌词 · 关掉啦喵，按 l 打开")
        };
        lines.push(vec![
            Span::new(truncate(&lyrics_header, width), theme::pink()).bold(),
        ]);

        let reserved = 8 + usize::from(info.is_some());
        let rows = height.saturating_sub(reserved).min(LYRIC_ROWS);
        if show_lyrics {
            if let Some(lrc) = &self.lrc
                && let Some(position) = state.time_pos
                && let Some(index) = lrc.index_at(position)
            {
                let (start, end) = lyrics::window(index, lrc.lines.len(), rows);
                for (line_index, _) in lrc.lines.iter().enumerate().take(end).skip(start) {
                    let current = line_index == index;
                    let room = width.saturating_sub(if line_index == index { 5 } else { 4 });
                    let text = truncate(&lrc.lines[line_index].1, room);
                    let line = if current {
                        vec![
                            Span::new(" ▎ ", theme::pink()).bold(),
                            Span::new(format!("{NOTE} {text}"), theme::pink()).bold(),
                        ]
                    } else {
                        vec![Span::new(format!("   {text}"), theme::overlay0())]
                    };
                    lines.push(line);
                }
            } else if rows > 0 {
                lines.push(text_line("   没有找到同名的 .lrc 喵", theme::overlay0()));
            }
        } else if rows > 0 {
            lines.push(text_line("   歌词关掉啦喵", theme::overlay0()));
        }

        match next_track(files, state.playlist_pos, mode) {
            _ if mode == Mode::Shuffle => lines.push(text_line(
                format!(" {NEXT} 随机播放中喵"),
                theme::overlay0(),
            )),
            Some(path) => {
                let title = path
                    .file_stem()
                    .map(|stem| stem.to_string_lossy().to_string())
                    .unwrap_or_default();
                lines.push(text_line(
                    truncate(&format!(" {NEXT} 下一首 · {title}"), width),
                    theme::sub1(),
                ));
            }
            None => lines.push(text_line(
                format!(" {NEXT} 后面没有啦喵"),
                theme::overlay0(),
            )),
        }
        lines.push(text_line(
            truncate(
                " 空格 播放/暂停 · n/p 切歌 · ←/→ 快退/快进 · -/= 音量 · m 静音 · l 歌词 · r 模式 · q 退出喵",
                width,
            ),
            theme::overlay0(),
        ));
        lines
    }
}

fn header_line(width: usize, state: &crate::mpv::State) -> Line {
    let left = format!(" {NOTE} neko");
    let right = match (state.playlist_pos, state.playlist_count) {
        (Some(position), Some(count)) if position >= 0 => format!("{}/{} ", position + 1, count),
        _ => String::new(),
    };
    let mut spans = vec![
        Span::new(left.clone(), theme::pink())
            .bold()
            .bg(theme::crust()),
        Span::new(" · 本地音乐", theme::overlay0()).bg(theme::crust()),
    ];
    let used = line_width(&spans);
    let right_width = text_width(&right);
    if used + right_width < width {
        spans.push(
            Span::new(" ".repeat(width - used - right_width), theme::overlay0()).bg(theme::crust()),
        );
        spans.push(Span::new(right, theme::lavender()).bg(theme::crust()));
    } else {
        let plain = format!("{left} · 本地音乐");
        return vec![
            Span::new(truncate(&plain, width), theme::pink())
                .bold()
                .bg(theme::crust()),
        ];
    }
    spans
}

fn rainbow_line(width: usize) -> Line {
    let mut spans = Vec::with_capacity(width);
    for column in 0..width {
        let color = theme::rainbow(column as f64 / width.max(1) as f64);
        spans.push(Span::new("─", color).bg(theme::crust()));
    }
    spans
}

fn title_line(title: &str, paused: bool, width: usize) -> Line {
    let icon = if paused { PAUSE } else { PLAY };
    let color = if paused {
        theme::overlay0()
    } else {
        theme::pink()
    };
    vec![
        Span::new(format!(" {icon} "), color).bold(),
        Span::new(truncate(title, width.saturating_sub(3)), theme::txt()).bold(),
    ]
}

fn track_info(state: &crate::mpv::State) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(artist) = state.metadata.get("artist") {
        parts.push(artist.clone());
    }
    if let Some(album) = state.metadata.get("album") {
        parts.push(album.clone());
    }
    if let Some(path) = &state.path
        && let Some(ext) = path.extension().and_then(|ext| ext.to_str())
    {
        parts.push(ext.to_string());
    }
    if let Some(audio) = &state.audio {
        parts.push(format!("{}Hz", audio.samplerate));
        parts.push(format!("{}声道", audio.channels));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

fn progress_line(width: usize, ratio: f64) -> Line {
    let filled = (width as f64 * ratio).round() as usize;
    let mut spans = Vec::with_capacity(width);
    for column in 0..width {
        let (glyph, color) = if column < filled {
            ("━", theme::rainbow(column as f64 / width.max(1) as f64))
        } else {
            ("─", theme::surface1())
        };
        spans.push(Span::new(glyph, color));
    }
    if width > 0 {
        let knob = filled.min(width - 1);
        spans[knob] = Span::new("●", theme::mauve());
    }
    spans
}

fn status_line(width: usize, state: &crate::mpv::State, mode: Mode, ratio: f64) -> Line {
    let time = state.time_pos.unwrap_or(0.0);
    let duration = state.duration.unwrap_or(0.0);
    let left = format!(
        " {CLOCK} {} / {}",
        library::duration_text(time),
        if duration > 0.0 {
            library::duration_text(duration)
        } else {
            "--:--".into()
        }
    );
    let volume = if state.mute {
        format!("{MUTE} 静音")
    } else {
        format!("{VOL} {:.0}%", state.volume)
    };
    let mut segments = vec![
        (volume, theme::sub0(), false),
        (format!("{:.0}%", ratio * 100.0), theme::surface2(), false),
        (mode.label().to_string(), theme::lavender(), false),
    ];
    if state.pause {
        segments.push(("已暂停".into(), theme::peach(), true));
    }
    let left_width = text_width(&left);
    let right_width: usize = segments
        .iter()
        .map(|(text, _, _)| text_width(text))
        .sum::<usize>()
        + 3 * segments.len().saturating_sub(1);
    if left_width + right_width + 2 > width {
        return text_line(truncate(&left, width), theme::sub0());
    }
    let mut spans = vec![Span::new(left, theme::sub0())];
    spans.push(Span::new(
        " ".repeat(width - left_width - right_width),
        theme::overlay0(),
    ));
    for (index, (text, color, bold)) in segments.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::new(" · ", theme::overlay0()));
        }
        let span = Span::new(text, color);
        spans.push(if bold { span.bold() } else { span });
    }
    spans
}

fn next_track(files: &[PathBuf], position: Option<i64>, mode: Mode) -> Option<&PathBuf> {
    let position = usize::try_from(position?).ok()?;
    if position >= files.len() {
        return None;
    }
    let index = match mode {
        Mode::Single => position,
        Mode::Order => position + 1,
        Mode::Loop | Mode::Shuffle => (position + 1) % files.len(),
    };
    files.get(index)
}

fn emit(out: &mut impl Write, lines: &[Line], rendered: &mut usize) -> io::Result<()> {
    if *rendered > 0 {
        queue!(
            out,
            MoveToPreviousLine(*rendered as u16),
            Clear(ClearType::FromCursorDown)
        )?;
    }
    for line in lines {
        for span in line {
            queue!(
                out,
                SetForegroundColor(Color::Rgb {
                    r: span.color.0,
                    g: span.color.1,
                    b: span.color.2,
                }),
                SetBackgroundColor(match span.bg {
                    Some(color) => Color::Rgb {
                        r: color.0,
                        g: color.1,
                        b: color.2,
                    },
                    None => Color::Reset,
                }),
                SetAttribute(if span.bold {
                    Attribute::Bold
                } else {
                    Attribute::NormalIntensity
                }),
                Print(&span.text)
            )?;
        }
        queue!(out, ResetColor, Print("\r\n"))?;
    }
    out.flush()?;
    *rendered = lines.len();
    Ok(())
}

fn handle_key(
    mpv: &mut Mpv,
    key: KeyEvent,
    mode: &mut Mode,
    show_lyrics: &mut bool,
    quit: &mut bool,
) {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    match key.code {
        KeyCode::Char('q') => *quit = true,
        KeyCode::Char('c') if control => *quit = true,
        KeyCode::Char(' ') | KeyCode::Enter => {
            let _ = mpv.set_prop("pause", json!(!mpv.state.pause));
        }
        KeyCode::Char('n') => {
            let _ = mpv.next();
        }
        KeyCode::Char('p') => {
            let _ = mpv.prev();
        }
        KeyCode::Left => {
            let _ = mpv.seek(if control { -30.0 } else { -5.0 }, false);
        }
        KeyCode::Right => {
            let _ = mpv.seek(if control { 30.0 } else { 5.0 }, false);
        }
        KeyCode::Char('-') => {
            let volume = (mpv.state.volume - 5.0).clamp(0.0, 130.0);
            let _ = mpv.set_prop("volume", json!(volume));
        }
        KeyCode::Char('=' | '+') => {
            let volume = (mpv.state.volume + 5.0).clamp(0.0, 130.0);
            let _ = mpv.set_prop("volume", json!(volume));
        }
        KeyCode::Char('m') => {
            let _ = mpv.set_prop("mute", json!(!mpv.state.mute));
        }
        KeyCode::Char('l') => *show_lyrics = !*show_lyrics,
        KeyCode::Char('r') => {
            *mode = mode.next();
            let _ = mpv.apply_mode(mode.key());
            if *mode == Mode::Shuffle {
                let _ = mpv.shuffle_playlist();
            }
        }
        _ => {}
    }
}

struct TermGuard {
    raw: bool,
    cursor: bool,
}

impl TermGuard {
    fn enter() -> io::Result<Self> {
        let raw = io::stdin().is_terminal();
        if raw {
            enable_raw_mode()?;
        }
        let mut guard = Self { raw, cursor: false };
        if io::stdout().is_terminal() {
            execute!(io::stdout(), Hide)?;
            guard.cursor = true;
        }
        Ok(guard)
    }
}

impl Drop for TermGuard {
    fn drop(&mut self) {
        if self.cursor {
            let _ = execute!(io::stdout(), Show);
        }
        if self.raw {
            let _ = disable_raw_mode();
        }
    }
}

fn play(cfg: &Config, list: &Path, files: &[PathBuf]) -> Result<(), String> {
    let mut mpv = Mpv::spawn(cfg.volume).map_err(|err| {
        format!("启动 mpv 失败喵：{err}\n请先安装 mpv（Arch：sudo pacman -S mpv）")
    })?;
    mpv.load_m3u(list, 0, Mode::Order.key(), false)?;

    let guard = TermGuard::enter().map_err(|err| format!("操作终端失败喵：{err}"))?;
    let result = draw_loop(&mut mpv, files);
    drop(guard);

    let ended = result?;
    println!(
        " {PAW} {}",
        if ended {
            "播完啦喵~"
        } else {
            "本喵收工喵~"
        }
    );
    Ok(())
}

fn draw_loop(mpv: &mut Mpv, files: &[PathBuf]) -> Result<bool, String> {
    let mut screen = Screen {
        rendered: 0,
        lrc: None,
        lrc_path: None,
        last_pos: None,
    };
    let mut mode = Mode::Order;
    let mut show_lyrics = true;
    let mut out = io::stdout();
    let interactive = io::stdin().is_terminal();
    let mut started = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        mpv.refresh();
        if !mpv.alive() {
            return Err("mpv 意外退出喵".into());
        }
        if started {
            if mpv.state.idle {
                return Ok(true);
            }
            // 播完后 mpv 会先把 time-pos 归零、下一拍才置 idle，别把这帧画出来。
            let reset = screen.lrc_path == mpv.state.path
                && screen.last_pos.is_some_and(|pos| pos > 1.0)
                && mpv.state.time_pos.is_some_and(|pos| pos < 0.5);
            if reset {
                return Ok(true);
            }
        } else if Instant::now() > deadline {
            return Err("mpv 没有开始播放喵".into());
        }
        if mpv.state.path.is_some() {
            started = true;
        }

        let (width, height) = size()
            .map(|(width, height)| (width as usize, height as usize))
            .unwrap_or((80, 24));
        let width = width.max(20);
        let lines = screen.render(mpv, width, height, files, mode, show_lyrics);
        emit(&mut out, &lines, &mut screen.rendered)
            .map_err(|err| format!("操作终端失败喵：{err}"))?;
        screen.last_pos = mpv.state.time_pos;

        let mut quit = false;
        if interactive {
            if event::poll(Duration::from_millis(POLL_MS))
                .map_err(|err| format!("读取按键失败喵：{err}"))?
            {
                match event::read() {
                    Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                        handle_key(mpv, key, &mut mode, &mut show_lyrics, &mut quit);
                    }
                    Ok(_) => {}
                    Err(err) => return Err(format!("读取按键失败喵：{err}")),
                }
            }
        } else {
            std::thread::sleep(Duration::from_millis(POLL_MS));
        }
        if quit {
            return Ok(false);
        }
    }
}
