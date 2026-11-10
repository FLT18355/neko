use super::*;
use crate::app::{App, Modal};
use crate::library;

pub fn draw_modal(
    canvas: &mut Canvas,
    app: &App,
    modal: Modal,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    match modal {
        Modal::Help => draw_help(canvas, app, hits),
        Modal::Queue => draw_queue(canvas, app, hover, hits),
        Modal::Info => draw_info(canvas, app, hits),
        Modal::Search => draw_input(
            canvas,
            app,
            "搜索曲目",
            "关键词",
            "Enter 应用 · Esc 取消",
            hits,
        ),
        Modal::Directory => draw_input(
            canvas,
            app,
            "默认音乐目录",
            "目录路径（Ctrl+U 清空）",
            "Enter 设为默认目录 · Esc 取消",
            hits,
        ),
        Modal::Seek => draw_input(
            canvas,
            app,
            "跳转时间",
            "秒 / 分:秒 / ±秒",
            "Enter 跳转 · 例 1:23 或 -15 · Esc 取消",
            hits,
        ),
        Modal::Settings => draw_settings(canvas, app, hover, hits),
    }
}

fn frame(
    canvas: &mut Canvas,
    width: usize,
    height: usize,
    title: &str,
    hits: &mut Vec<(Hit, Rect)>,
) -> (usize, usize, usize, usize) {
    let screen_width = canvas.width();
    let screen_height = canvas.height();
    let width = width.min(screen_width.saturating_sub(4)).max(10);
    let height = height.min(screen_height.saturating_sub(2)).max(5);
    let x = (screen_width - width) / 2;
    let y = (screen_height - height) / 2;
    canvas.fill(x + 2, y + 1, width, height, Style::new(TEXT, CRUST));
    canvas.fill(x, y, width, height, Style::new(TEXT, MANTLE));
    canvas.border(x, y, width, height, title, Style::new(SURFACE1, MANTLE));
    canvas.put(
        x + 2,
        y,
        &format!(" {title} "),
        Style::new(PINK, MANTLE).bold(),
    );
    hits.push((Hit::ModalPanel, Rect::new(x, y, width, height)));
    (x, y, width, height)
}

fn draw_help(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let entries = [
        ("空格", "播放 / 暂停"),
        ("Enter", "播放选中曲目"),
        ("n / p", "下一首 / 上一首"),
        ("j / k", "上下移动"),
        ("g / G", "跳到列表首尾"),
        ("← / →", "快退 / 快进 5 秒"),
        ("Shift+←/→", "快退 / 快进 30 秒"),
        ("- / =", "音量 -5 / +5"),
        ("m", "静音开关"),
        ("B", "透明背景开关"),
        ("[ / ]", "速度 -0.05 / +0.05"),
        ("0", "速度恢复到 1.00x"),
        ("L", "切换播放模式"),
        ("S", "立刻打乱队列"),
        ("a", "A-B 循环（三次取消）"),
        ("f", "收藏选中曲目"),
        ("1 / 2 / 3", "全部 / 收藏 / 最近"),
        ("Tab", "循环切换视图"),
        ("/", "搜索标题或路径"),
        ("o", "更换默认音乐目录"),
        ("s", "切换排序方式"),
        ("e", "切换音效预设"),
        ("t", "睡眠定时，最后 10 秒渐弱"),
        ("l", "歌词开关"),
        ("z / x", "歌词提前 / 延后 0.5 秒"),
        ("y", "导出当前队列 M3U"),
        (":", "跳转到指定时间"),
        ("u", "查看播放队列"),
        ("c", "打开设置"),
        ("i", "查看曲目详情"),
        ("鼠标", "悬停高亮 / 单击选择"),
        ("音量条", "点击或拖动调音量"),
        ("时间", "点击切换已播放 / 剩余"),
        ("双击", "播放曲目"),
        ("滚轮", "滚动列表与弹窗"),
        ("r", "重新扫描目录"),
        ("?", "打开这个帮助"),
        ("q", "退出 neko"),
    ];
    let rows = entries.len().div_ceil(2);
    let width = 84;
    let height = rows + 5;
    let (x, y, width, height) = frame(canvas, width, height, "快捷键", hits);
    let split = rows;
    let column_width = (width - 4) / 2;
    for (index, (key, description)) in entries.iter().enumerate() {
        let (column, row) = if index < split {
            (0, index)
        } else {
            (1, index - split)
        };
        let tx = x + 2 + column * column_width;
        let ty = y + 2 + row;
        if ty >= y + height - 2 {
            break;
        }
        canvas.put(
            tx,
            ty,
            &format!("{key:<12}"),
            Style::new(LAVENDER, MANTLE).bold(),
        );
        canvas.put(
            tx + 13,
            ty,
            &truncate(description, column_width.saturating_sub(14)),
            Style::new(SUBTEXT1, MANTLE),
        );
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "Esc 或 q 关闭",
        Style::new(OVERLAY0, MANTLE),
    );
    let _ = app;
}

fn draw_queue(canvas: &mut Canvas, app: &App, hover: Option<Hit>, hits: &mut Vec<(Hit, Rect)>) {
    let width = 72;
    let height = canvas.height().saturating_sub(4).clamp(8, 24);
    let (x, y, width, height) = frame(canvas, width, height, "播放队列", hits);
    let rows = height.saturating_sub(3);
    if app.session.is_empty() {
        canvas.put(
            x + 2,
            y + 2,
            "队列是空的喵，先挑一首播放",
            Style::new(OVERLAY0, MANTLE),
        );
        return;
    }
    let offset = app.modal_scroll.min(app.session.len().saturating_sub(rows));
    let current = app.current_path();
    for (row, track) in app.session.iter().skip(offset).take(rows).enumerate() {
        let index = offset + row;
        let playing = current.as_ref() == Some(&track.path);
        let row_hover = hover == Some(Hit::QueueRow(index));
        let style = if playing {
            Style::new(PINK, MANTLE).bold()
        } else if row_hover {
            Style::new(ROSEWATER, SURFACE0).bold()
        } else {
            Style::new(TEXT, MANTLE)
        };
        if row_hover {
            canvas.fill(x + 1, y + 2 + row, width - 2, 1, Style::new(TEXT, SURFACE0));
        }
        let marker = if playing { NOTE } else { " " };
        canvas.put(
            x + 2,
            y + 2 + row,
            &format!(
                "{:>3}  {} {}",
                index + 1,
                marker,
                truncate(&track.title, width - 12)
            ),
            style,
        );
        hits.push((
            Hit::QueueRow(index),
            Rect::new(x + 1, y + 2 + row, width - 2, 1),
        ));
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "j/k 滚动 · Esc 关闭",
        Style::new(OVERLAY0, MANTLE),
    );
}

fn draw_info(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 78;
    let height = 14;
    let (x, y, width, _height) = frame(canvas, width, height, "曲目详情", hits);
    let state = &app.mpv.state;
    let Some(path) = state.path.as_ref() else {
        canvas.put(
            x + 2,
            y + 2,
            "没有正在播放的曲目喵",
            Style::new(OVERLAY0, MANTLE),
        );
        return;
    };
    let title = state
        .metadata
        .get("title")
        .filter(|value| !value.is_empty())
        .cloned()
        .or_else(|| {
            path.file_stem()
                .map(|stem| stem.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "未知曲目".into());
    let mut lines = vec![
        format!("标题    {title}"),
        format!("路径    {}", path.display()),
    ];
    for (key, label) in [
        ("artist", "艺术家"),
        ("album", "专辑"),
        ("date", "年份"),
        ("genre", "流派"),
    ] {
        if let Some(value) = state.metadata.get(key).filter(|value| !value.is_empty()) {
            lines.push(format!("{label}    {value}"));
        }
    }
    if let Some(bitrate) = state.bitrate {
        lines.push(format!("码率    {:.0} kbps", bitrate / 1000.0));
    }
    let plays = app
        .cache
        .plays
        .get(&path.to_string_lossy().to_string())
        .copied()
        .unwrap_or(0);
    if plays > 0 {
        lines.push(format!("播放    {plays} 次"));
    }
    if let Some(audio) = &state.audio {
        lines.push(format!(
            "音频    {} Hz · {} 声道 · {}",
            audio.samplerate, audio.channels, audio.format
        ));
    }
    lines.push(format!(
        "时长    {} / {}",
        library::duration_text(state.time_pos.unwrap_or(0.0)),
        library::duration_text(state.duration.unwrap_or(0.0))
    ));
    lines.push(format!(
        "状态    {} · {:.2}x · 音量 {:.0}%",
        app.mode_label(),
        state.speed,
        state.volume
    ));
    if let (Some(position), Some(count)) = (state.playlist_pos, state.playlist_count) {
        lines.push(format!("队列    {}/{}", position + 1, count));
    }
    for (row, line) in lines.iter().enumerate() {
        if row >= _height.saturating_sub(4) {
            break;
        }
        canvas.put(
            x + 2,
            y + 2 + row,
            &truncate(line, width.saturating_sub(4)),
            Style::new(SUBTEXT1, MANTLE),
        );
    }
    canvas.put(
        x + 2,
        y + _height - 2,
        "Esc 关闭",
        Style::new(OVERLAY0, MANTLE),
    );
}

fn draw_input(
    canvas: &mut Canvas,
    app: &App,
    title: &str,
    label: &str,
    hint: &str,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let width = 70;
    let height = 8;
    let (x, y, width, _height) = frame(canvas, width, height, title, hits);
    canvas.put(x + 2, y + 2, label, Style::new(OVERLAY0, MANTLE));
    canvas.fill(
        x + 2,
        y + 3,
        width.saturating_sub(4),
        1,
        Style::new(TEXT, SURFACE0),
    );
    let available = width.saturating_sub(6);
    let text = if text_width(&app.input) > available {
        tail(&app.input, available.saturating_sub(1))
    } else {
        app.input.clone()
    };
    let input_x = canvas.put(x + 2, y + 3, &text, Style::new(TEXT, SURFACE0));
    canvas.put(input_x, y + 3, "▌", Style::new(PINK, SURFACE0).bold());
    canvas.put(x + 2, y + 5, hint, Style::new(OVERLAY0, MANTLE));
}

fn draw_settings(canvas: &mut Canvas, app: &App, hover: Option<Hit>, hits: &mut Vec<(Hit, Rect)>) {
    let rows = app.settings_rows();
    let width = 78;
    let height = rows.len() + 5;
    let (x, y, width, height) = frame(canvas, width, height, "设置", hits);
    for (index, (label, value)) in rows.iter().enumerate() {
        if index >= height.saturating_sub(4) {
            break;
        }
        let row_y = y + 2 + index;
        let selected = app.setting_selected == index;
        let row_hover = hover == Some(Hit::SettingRow(index));
        let background = if selected {
            SURFACE1
        } else if row_hover {
            SURFACE0
        } else {
            MANTLE
        };
        if selected || row_hover {
            canvas.fill(x + 1, row_y, width - 2, 1, Style::new(TEXT, background));
        }
        hits.push((
            Hit::SettingRow(index),
            Rect::new(x + 1, row_y, width - 2, 1),
        ));
        let label_style = if selected {
            Style::new(PINK, background).bold()
        } else {
            Style::new(SUBTEXT1, background)
        };
        canvas.put(x + 2, row_y, label, label_style);
        let arrows_width = 8;
        let value_width = width.saturating_sub(label_width() + arrows_width + 4);
        canvas.put(
            x + 16,
            row_y,
            &truncate(value, value_width),
            Style::new(TEXT, background),
        );
        if index >= 13 {
            canvas.put(x + width - 7, row_y, "—", Style::new(SURFACE2, background));
            continue;
        }
        let left_hover = hover == Some(Hit::SettingLeft(index));
        let right_hover = hover == Some(Hit::SettingRight(index));
        let left_style = if left_hover {
            Style::new(PINK, SURFACE0).bold()
        } else {
            Style::new(OVERLAY0, background)
        };
        let right_style = if right_hover {
            Style::new(PINK, SURFACE0).bold()
        } else {
            Style::new(OVERLAY0, background)
        };
        canvas.put(x + width - 7, row_y, "‹", left_style);
        canvas.put(x + width - 4, row_y, "›", right_style);
        hits.push((
            Hit::SettingLeft(index),
            Rect::new(x + width - 7, row_y, 1, 1),
        ));
        hits.push((
            Hit::SettingRight(index),
            Rect::new(x + width - 4, row_y, 1, 1),
        ));
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "↑/↓ 选择 · ←/→ 调整 · Enter 修改 · Esc 关闭",
        Style::new(OVERLAY0, MANTLE),
    );
}

fn label_width() -> usize {
    12
}

fn tail(text: &str, width: usize) -> String {
    let mut result = Vec::new();
    let mut used = 0;
    for ch in text.chars().rev() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > width {
            break;
        }
        result.push(ch);
        used += char_width;
    }
    result.reverse();
    format!("…{}", result.into_iter().collect::<String>())
}
