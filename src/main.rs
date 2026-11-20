mod app;
mod cache;
mod cli;
mod clock;
mod config;
mod keymap;
mod library;
mod lyrics;
mod mpv;
mod theme;
mod ui;
mod viz;

use app::App;
use cache::Cache;
use config::Config;
use crossterm::event::{self, Event, KeyEventKind};
use mpv::Mpv;
use std::env;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant};

fn usage() {
    println!("neko {} · 终端本地音乐播放器", env!("CARGO_PKG_VERSION"));
    println!();
    println!("用法：");
    println!("  neko [--transparent] [音乐目录 ...]");
    println!("  neko cli [音乐文件或目录 ...]");
    println!("  neko server stop");
    println!("  neko --help");
    println!("  neko --version");
    println!();
    println!("可以给多个音乐目录，会合并成一个曲库。");
    println!("启动后按 ? 看快捷键，q 退出。");
    println!("播放中退出会把音乐留在后台，用 neko server stop 停止。");
    println!("依赖 mpv；界面图标需要 Nerd Font 字体。");
    println!("配置文件：{}", config::config_path().display());
    println!("缓存文件：{}", cache::cache_path().display());
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("server") {
        match args.get(1).map(String::as_str) {
            Some("stop") => match mpv::stop_server() {
                Ok(true) => println!("已停止后台播放喵"),
                Ok(false) => println!("没有正在后台播放的音乐喵"),
                Err(err) => {
                    eprintln!("{err}");
                    std::process::exit(1);
                }
            },
            _ => println!("用法：neko server stop"),
        }
        return;
    }
    if args.first().map(String::as_str) == Some("cli") {
        if let Err(err) = cli::run(&args[1..]) {
            eprintln!("{err}");
            std::process::exit(1);
        }
        return;
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        usage();
        return;
    }
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("neko {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let mut dir_args: Vec<String> = Vec::new();
    let mut transparent = false;
    for arg in &args {
        if arg == "--transparent" {
            transparent = true;
        } else if arg.starts_with('-') {
            eprintln!("不认识的参数：{arg}（用 --help 看看用法）");
            std::process::exit(2);
        } else {
            dir_args.push(arg.clone());
        }
    }

    let mut cfg = Config::load();
    if transparent {
        cfg.transparent = true;
    }
    let mut dirs = config::resolve_dirs(&dir_args, &cfg);
    for dir in &dirs {
        if !dir.is_dir() {
            eprintln!("音乐目录不存在：{}", dir.display());
            eprintln!("用法：neko [音乐目录 ...]");
            std::process::exit(1);
        }
    }
    let dirs: Vec<PathBuf> = dirs
        .drain(..)
        .map(|dir| dir.canonicalize().unwrap_or(dir))
        .collect();

    let mpv = match Mpv::spawn(cfg.volume) {
        Ok(mpv) => mpv,
        Err(err) => {
            eprintln!("启动 mpv 失败：{err}");
            eprintln!("请先安装 mpv（Arch：sudo pacman -S mpv）");
            std::process::exit(1);
        }
    };

    let mut tracks = library::scan_dirs(&dirs);
    library::sort_tracks(&mut tracks, &cfg.sort);
    let cache = Cache::load();
    let mut app = App::new(cfg, cache, mpv, tracks, dirs);
    let mut ui_state = ui::UiState::default();
    let mut terminal = match ui::Terminal::enter() {
        Ok(terminal) => terminal,
        Err(err) => {
            eprintln!("初始化终端失败：{err}");
            std::process::exit(1);
        }
    };

    let mut last_alive = Instant::now();
    loop {
        app.tick();
        if let Err(err) = terminal.draw(&app, &mut ui_state) {
            eprintln!("绘制界面失败：{err}");
            break;
        }
        // 动画期间 30 帧/秒；播放时 25 帧/秒让频谱顺滑；空闲降回 5 帧/秒省 CPU
        let idle = app.mpv.state.path.is_none() || app.mpv.state.pause;
        let wait = if app.animating() {
            33
        } else if idle {
            200
        } else {
            40
        };
        if event::poll(Duration::from_millis(wait)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    ui::handle_key(&mut app, &mut ui_state, key);
                }
                Ok(Event::Mouse(mouse)) => {
                    ui::handle_mouse(&mut app, &mut ui_state, mouse);
                }
                // 终端可能只重排像素而格子数不变，整帧重发才能补洞。
                Ok(Event::Resize(_, _) | Event::FocusGained | Event::FocusLost) => {
                    ui_state.force_full = true;
                }
                Ok(_) => {}
                Err(err) => {
                    eprintln!("读取按键失败：{err}");
                    break;
                }
            }
        }
        if app.quit {
            break;
        }
        // mpv 可能被 `neko server stop` 关闭；每秒确认一次，避免每帧新建 IPC 连接。
        if last_alive.elapsed() >= Duration::from_secs(1) {
            last_alive = Instant::now();
            if !app.mpv.alive() {
                break;
            }
        }
    }

    terminal.leave();
    app.save();
    if app.shutdown() {
        println!("♪ 音乐在后台继续播放 · 停止：neko server stop");
    }
    let _ = io::Write::flush(&mut io::stdout());
}
