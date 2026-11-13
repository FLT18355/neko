mod app;
mod cache;
mod clock;
mod config;
mod keymap;
mod library;
mod lyrics;
mod mpv;
mod theme;
mod ui;

use app::App;
use cache::Cache;
use config::Config;
use crossterm::event::{self, Event, KeyEventKind};
use mpv::Mpv;
use std::env;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

fn usage() {
    println!("neko {} · 终端本地音乐播放器", env!("CARGO_PKG_VERSION"));
    println!();
    println!("用法：");
    println!("  neko [--transparent] [音乐目录 ...]");
    println!("  neko --help");
    println!("  neko --version");
    println!();
    println!("可以给多个音乐目录，会合并成一个曲库。");
    println!("启动后按 ? 看快捷键，q 退出。");
    println!("依赖 mpv；界面图标需要 Nerd Font 字体。");
    println!("配置文件：{}", config::config_path().display());
    println!("缓存文件：{}", cache::cache_path().display());
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
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
    let mut dirs: Vec<PathBuf> = if !dir_args.is_empty() {
        dir_args.iter().map(|arg| config::expand_path(arg)).collect()
    } else if !cfg.music_dirs.is_empty() {
        cfg.music_dirs
            .iter()
            .map(|dir| config::expand_path(dir))
            .collect()
    } else {
        vec![config::default_music_dir()]
    };
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

    loop {
        app.tick();
        if let Err(err) = terminal.draw(&app, &mut ui_state) {
            eprintln!("绘制界面失败：{err}");
            break;
        }
        if event::poll(Duration::from_millis(120)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => {
                    ui::handle_key(&mut app, &mut ui_state, key);
                }
                Ok(Event::Mouse(mouse)) => {
                    ui::handle_mouse(&mut app, &mut ui_state, mouse);
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
    }

    terminal.leave();
    app.save();
    app.shutdown();
    let _ = io::Write::flush(&mut io::stdout());
}
