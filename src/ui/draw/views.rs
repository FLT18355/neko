use super::*;
use crate::app::{App, Modal};
use crate::keymap::ACTIONS;
use crate::library;

pub fn draw_full_lyrics(canvas: &mut Canvas, app: &App) {
    let width = canvas.width();
    let height = canvas.height();
    canvas.fill(0, 0, width, height, Style::new(txt(), base()));
    let state = &app.mpv.state;
    let Some(path) = state.path.as_ref() else {
        canvas.centered(
            height / 2,
            "没有正在播放的曲目喵",
            Style::new(overlay0(), base()),
        );
        canvas.centered(
            height.saturating_sub(2),
            "F 或 Esc 返回",
            Style::new(surface2(), base()),
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
    let artist = state
        .metadata
        .get("artist")
        .filter(|value| !value.is_empty())
        .cloned();
    canvas.centered(
        1,
        &truncate(&title, width.saturating_sub(4)),
        Style::new(pink(), base()).bold(),
    );
    if let Some(artist) = artist {
        canvas.centered(
            2,
            &truncate(&artist, width.saturating_sub(4)),
            Style::new(sub0(), base()),
        );
    }
    if app.lrc_offset.abs() > 0.01 {
        canvas.put(
            width.saturating_sub(8),
            1,
            &format!("{:+.1}s", app.lrc_offset),
            Style::new(surface2(), base()),
        );
    }

    let top = 4usize;
    let footer = 4usize;
    let area = height.saturating_sub(top + footer);
    if let Some((index, lines)) = app.current_lyric() {
        if area > 0 {
            let (start, end) = crate::lyrics::window(index, lines.len(), area);
            let count = end.saturating_sub(start);
            let y0 = top + area.saturating_sub(count) / 2;
            for (line_index, (_, text)) in lines.iter().enumerate().skip(start).take(count) {
                let y = y0 + line_index - start;
                let style = if line_index == index {
                    Style::new(pink(), base()).bold()
                } else if line_index < index {
                    Style::new(surface2(), base())
                } else {
                    Style::new(overlay0(), base())
                };
                let content = if line_index == index {
                    format!("{} {}", NOTE, text)
                } else {
                    text.clone()
                };
                canvas.centered(y, &truncate(&content, width.saturating_sub(4)), style);
            }
        }
    } else {
        canvas.centered(
            top + area / 2,
            "没有找到同名的 .lrc 喵",
            Style::new(overlay0(), base()),
        );
    }

    let time = state.time_pos.unwrap_or(0.0);
    let duration = state.duration.unwrap_or(0.0);
    let ratio = if duration > 0.0 {
        (time / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bar_y = height.saturating_sub(3);
    let bar_x = 2usize;
    let bar_width = width.saturating_sub(4);
    let filled = (bar_width as f64 * ratio).round() as usize;
    for column in 0..bar_width {
        let local = column as f64 / bar_width.max(1) as f64;
        let (ch, color) = if column < filled {
            ("━", mix(pink(), mauve(), local))
        } else {
            ("─", surface0())
        };
        canvas.put(bar_x + column, bar_y, ch, Style::new(color, base()));
    }
    canvas.put(
        bar_x,
        height.saturating_sub(2),
        &format!(
            "{} / {}",
            library::duration_text(time),
            library::duration_text(duration)
        ),
        Style::new(sub0(), base()),
    );
    let hint = if state.pause {
        "已暂停 · 空格继续 · F / Esc 返回"
    } else {
        "空格暂停 · n / p 切歌 · F / Esc 返回"
    };
    let hint_width = text_width(hint);
    canvas.put(
        width.saturating_sub(hint_width + 2),
        height.saturating_sub(2),
        hint,
        Style::new(surface2(), base()),
    );
}

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
        Modal::Folders => draw_folders(canvas, app, hits),
        Modal::Playlists => draw_playlists(canvas, app, hits),
        Modal::PlaylistTracks => draw_playlist_tracks(canvas, app, hits),
        Modal::PlaylistName => draw_input(
            canvas,
            app,
            if app.playlist_name_target.is_some() {
                "重命名歌单"
            } else {
                "新建歌单"
            },
            "歌单名",
            "Enter 保存 · Esc 取消",
            hits,
        ),
        Modal::Equalizer => draw_equalizer(canvas, app, hits),
        Modal::AudioDevice => draw_devices(canvas, app, hits),
        Modal::Stats => draw_stats(canvas, app, hits),
        Modal::Keybindings => draw_keybindings(canvas, app, hits),
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
    canvas.fill(x + 2, y + 1, width, height, Style::new(txt(), crust()));
    canvas.fill(x, y, width, height, Style::new(txt(), mantle()));
    canvas.border(x, y, width, height, title, Style::new(surface1(), mantle()));
    canvas.put(
        x + 2,
        y,
        &format!(" {title} "),
        Style::new(pink(), mantle()).bold(),
    );
    hits.push((Hit::ModalPanel, Rect::new(x, y, width, height)));
    (x, y, width, height)
}

fn draw_help(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let mut entries: Vec<(String, String)> = crate::keymap::ACTIONS
        .iter()
        .map(|action| (app.key_for(*action), action.label().to_string()))
        .collect();
    entries.extend(
        [
            ("j / k", "弹窗内滚动"),
            ("Enter", "确认 / 打开"),
            ("Esc", "关闭弹窗或清除筛选"),
            ("鼠标", "悬停高亮 / 单击选择"),
            ("双击", "播放曲目"),
            ("滚轮", "滚动列表与弹窗"),
            ("音量条", "点击或拖动调音量"),
            ("时间", "点击切换已播放 / 剩余"),
        ]
        .map(|(key, text)| (key.to_string(), text.to_string())),
    );
    let columns = if canvas.width() >= 100 { 3 } else { 2 };
    let rows = entries.len().div_ceil(columns);
    let width = if columns == 3 { 118 } else { 84 };
    let height = (rows + 5).min(canvas.height().saturating_sub(2)).max(5);
    let (x, y, width, height) = frame(canvas, width, height, "快捷键", hits);
    let column_width = (width - 4) / columns;
    let visible = height.saturating_sub(4).max(1);
    let visible_count = visible * columns;
    let offset = app
        .modal_scroll
        .min(entries.len().saturating_sub(visible_count));
    for (index, (key, description)) in entries.iter().skip(offset).take(visible_count).enumerate() {
        let (column, row) = (index / visible, index % visible);
        let tx = x + 2 + column * column_width;
        let ty = y + 2 + row;
        if ty >= y + height - 2 {
            break;
        }
        canvas.put(
            tx,
            ty,
            &format!("{:<12}", truncate(key, 11)),
            Style::new(lavender(), mantle()).bold(),
        );
        canvas.put(
            tx + 13,
            ty,
            &truncate(description, column_width.saturating_sub(14)),
            Style::new(sub1(), mantle()),
        );
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "j/k 滚动 · Esc 或 q 关闭",
        Style::new(overlay0(), mantle()),
    );
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
            Style::new(overlay0(), mantle()),
        );
        return;
    }
    let offset = app
        .queue_selected
        .saturating_sub(rows / 2)
        .min(app.session.len().saturating_sub(rows));
    let current = app.current_path();
    for (row, track) in app.session.iter().skip(offset).take(rows).enumerate() {
        let index = offset + row;
        let playing = current.as_ref() == Some(&track.path);
        let row_hover = hover == Some(Hit::QueueRow(index));
        let selected = index == app.queue_selected;
        let style = if selected || row_hover {
            Style::new(selected_fg(), surface0()).bold()
        } else if playing {
            Style::new(pink(), mantle()).bold()
        } else {
            Style::new(txt(), mantle())
        };
        if selected || row_hover {
            canvas.fill(
                x + 1,
                y + 2 + row,
                width - 2,
                1,
                Style::new(txt(), surface0()),
            );
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
        "Enter 播放 · J/K 移动 · d 移出 · x 清空 · Esc 关闭",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_info(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 78;
    let height = 16;
    let (x, y, width, _height) = frame(canvas, width, height, "曲目详情", hits);
    let state = &app.mpv.state;
    let Some(path) = state.path.as_ref() else {
        canvas.put(
            x + 2,
            y + 2,
            "没有正在播放的曲目喵",
            Style::new(overlay0(), mantle()),
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
    let rating = app.rating_of(path);
    lines.push(format!(
        "评分    {}",
        if rating == 0 {
            "未评分 · 按 R".into()
        } else {
            format!("{} ({rating}/5)", "★".repeat(rating as usize))
        }
    ));
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
            Style::new(sub1(), mantle()),
        );
    }
    canvas.put(
        x + 2,
        y + _height - 2,
        "Esc 关闭",
        Style::new(overlay0(), mantle()),
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
    canvas.put(x + 2, y + 2, label, Style::new(overlay0(), mantle()));
    canvas.fill(
        x + 2,
        y + 3,
        width.saturating_sub(4),
        1,
        Style::new(txt(), surface0()),
    );
    let available = width.saturating_sub(6);
    let text = if text_width(&app.input) > available {
        tail(&app.input, available.saturating_sub(1))
    } else {
        app.input.clone()
    };
    let input_x = canvas.put(x + 2, y + 3, &text, Style::new(txt(), surface0()));
    canvas.put(input_x, y + 3, "▌", Style::new(pink(), surface0()).bold());
    canvas.put(x + 2, y + 5, hint, Style::new(overlay0(), mantle()));
}

fn draw_playlists(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 72;
    let height = canvas.height().saturating_sub(4).clamp(8, 24);
    let (x, y, width, height) = frame(canvas, width, height, "自建歌单", hits);
    let rows = height.saturating_sub(3);
    if app.playlists.is_empty() {
        canvas.put(
            x + 2,
            y + 2,
            "还没有歌单喵，按 n 新建一个",
            Style::new(overlay0(), mantle()),
        );
    } else {
        let offset = app
            .playlist_selected
            .saturating_sub(rows / 2)
            .min(app.playlists.len().saturating_sub(rows));
        for (row, list) in app.playlists.iter().skip(offset).take(rows).enumerate() {
            let index = offset + row;
            let selected = index == app.playlist_selected;
            let row_y = y + 2 + row;
            if selected {
                canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), surface0()));
            }
            canvas.put(
                x + 2,
                row_y,
                &format!(
                    "{:>3}  {}",
                    index + 1,
                    truncate(&list.name, width.saturating_sub(24))
                ),
                if selected {
                    Style::new(pink(), surface0()).bold()
                } else {
                    Style::new(txt(), mantle())
                },
            );
            canvas.put(
                x + width - 12,
                row_y,
                &format!("{} 首", list.songs.len()),
                Style::new(overlay0(), if selected { surface0() } else { mantle() }),
            );
        }
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "Enter 打开 · n 新建 · r 改名 · x 删除 · a 加入当前曲目",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_playlist_tracks(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width: usize = 72;
    let height = canvas.height().saturating_sub(4).clamp(8, 24);
    let Some(list) = app.playlists.get(app.playlist_selected) else {
        return;
    };
    let title = format!("歌单 · {}", truncate(&list.name, width.saturating_sub(16)));
    let (x, y, width, height) = frame(canvas, width, height, &title, hits);
    let rows = height.saturating_sub(3);
    if list.songs.is_empty() {
        canvas.put(
            x + 2,
            y + 2,
            "空歌单 · 按 a 加入当前选中曲目",
            Style::new(overlay0(), mantle()),
        );
    } else {
        let offset = app
            .playlist_song_selected
            .saturating_sub(rows / 2)
            .min(list.songs.len().saturating_sub(rows));
        let current = app.current_path();
        for (row, song) in list.songs.iter().skip(offset).take(rows).enumerate() {
            let index = offset + row;
            let selected = index == app.playlist_song_selected;
            let row_y = y + 2 + row;
            if selected {
                canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), surface0()));
            }
            let playing = current
                .as_ref()
                .is_some_and(|path| path.to_string_lossy() == *song);
            let marker = if playing { NOTE } else { " " };
            canvas.put(
                x + 2,
                row_y,
                &format!(
                    "{:>3}  {marker} {}",
                    index + 1,
                    truncate(&app.title_for(song), width.saturating_sub(12))
                ),
                if selected {
                    Style::new(pink(), surface0()).bold()
                } else {
                    Style::new(txt(), mantle())
                },
            );
        }
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "Enter 播放 · a 加入当前曲目 · d 移出 · Esc 返回",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_folders(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 72;
    let height = canvas.height().saturating_sub(4).clamp(8, 24);
    let (x, y, width, height) = frame(canvas, width, height, "文件夹", hits);
    let rows = height.saturating_sub(3);
    let folders = app.folder_list();
    if folders.is_empty() {
        canvas.put(
            x + 2,
            y + 2,
            "曲库是空的喵，按 o 换个目录",
            Style::new(overlay0(), mantle()),
        );
    } else {
        let offset = app
            .folder_selected
            .saturating_sub(rows / 2)
            .min(folders.len().saturating_sub(rows));
        for (row, (name, count)) in folders.iter().skip(offset).take(rows).enumerate() {
            let index = offset + row;
            let selected = index == app.folder_selected;
            let row_y = y + 2 + row;
            if selected {
                canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), surface0()));
            }
            let label = if name.is_empty() { "根目录" } else { name };
            canvas.put(
                x + 2,
                row_y,
                &format!(
                    "{:>3}  {}",
                    index + 1,
                    truncate(label, width.saturating_sub(24))
                ),
                if selected {
                    Style::new(pink(), surface0()).bold()
                } else {
                    Style::new(txt(), mantle())
                },
            );
            canvas.put(
                x + width - 12,
                row_y,
                &format!("{count} 首"),
                Style::new(overlay0(), if selected { surface0() } else { mantle() }),
            );
        }
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "Enter 进入文件夹 · Esc 关闭",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_equalizer(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 66;
    let height = app.cfg.eq_gains.len() + 6;
    let (x, y, width, height) = frame(canvas, width, height, "10 段均衡器", hits);
    let bar_x = x + 13;
    let bar_width = 28usize;
    let center = bar_width / 2;
    for row in 0..=app.cfg.eq_gains.len() {
        let row_y = y + 2 + row;
        if row_y >= y + height - 2 {
            break;
        }
        let selected = row == app.eq_selected;
        let background = if selected { surface0() } else { mantle() };
        if selected {
            canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), background));
        }
        let (label, gain) = if row == 0 {
            ("总开关".to_string(), None)
        } else {
            (
                format!("{:>5} Hz", crate::app::EQ_BANDS[row - 1] as u32),
                Some(app.cfg.eq_gains[row - 1]),
            )
        };
        canvas.put(
            x + 2,
            row_y,
            &label,
            if selected {
                Style::new(pink(), background).bold()
            } else {
                Style::new(sub1(), background)
            },
        );
        match gain {
            Some(gain) => {
                let cells = ((bar_width / 2) as f64 * gain.abs() / 12.0).round() as usize;
                for column in 0..bar_width {
                    let distance = column.abs_diff(center);
                    let (ch, color) = if distance == 0 {
                        ("┼", surface2())
                    } else if gain >= 0.0 && column > center && distance <= cells {
                        ("━", peach())
                    } else if gain < 0.0 && column < center && distance <= cells {
                        ("━", lavender())
                    } else {
                        ("─", surface0())
                    };
                    canvas.put(bar_x + column, row_y, ch, Style::new(color, background));
                }
                canvas.put(
                    x + width - 11,
                    row_y,
                    &format!("{gain:+5.0} dB"),
                    Style::new(txt(), background),
                );
            }
            None => {
                canvas.put(
                    bar_x,
                    row_y,
                    if app.cfg.eq_on { "开启" } else { "关闭" },
                    Style::new(if app.cfg.eq_on { peach() } else { overlay0() }, background),
                );
            }
        }
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "↑/↓ 选择 · ←/→ 调 ±1dB · Enter 归零/开关 · r 拉平",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_devices(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 72;
    let height = canvas.height().saturating_sub(4).clamp(8, 24);
    let (x, y, width, height) = frame(canvas, width, height, "音频输出设备", hits);
    let rows = height.saturating_sub(3);
    if app.devices.is_empty() {
        canvas.put(
            x + 2,
            y + 2,
            "没有读到音频设备 · 按 r 重试",
            Style::new(overlay0(), mantle()),
        );
    } else {
        let offset = app
            .device_selected
            .saturating_sub(rows / 2)
            .min(app.devices.len().saturating_sub(rows));
        for (row, (name, description)) in app.devices.iter().skip(offset).take(rows).enumerate() {
            let index = offset + row;
            let selected = index == app.device_selected;
            let row_y = y + 2 + row;
            if selected {
                canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), surface0()));
            }
            let current = *name == app.cfg.audio_device;
            canvas.put(
                x + 2,
                row_y,
                if current { "●" } else { " " },
                Style::new(peach(), if selected { surface0() } else { mantle() }),
            );
            canvas.put(
                x + 4,
                row_y,
                &truncate(description, width.saturating_sub(20)),
                if selected {
                    Style::new(pink(), surface0()).bold()
                } else {
                    Style::new(txt(), mantle())
                },
            );
            canvas.put(
                x + width - 14,
                row_y,
                &truncate(name, 12),
                Style::new(overlay0(), if selected { surface0() } else { mantle() }),
            );
        }
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "Enter 应用 · r 重新读取 · Esc 关闭",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_stats(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 76;
    let height = canvas.height().saturating_sub(4).clamp(12, 24);
    let (x, y, width, height) = frame(canvas, width, height, "收听统计", hits);
    let summary = app.stats_rows();
    for (row, (label, value)) in summary.iter().enumerate() {
        let row_y = y + 2 + row;
        canvas.put(x + 2, row_y, label, Style::new(overlay0(), mantle()));
        canvas.put(
            x + 18,
            row_y,
            &truncate(value, width.saturating_sub(22)),
            Style::new(txt(), mantle()).bold(),
        );
    }
    let top_y = y + 2 + summary.len() + 1;
    canvas.put(
        x + 2,
        top_y,
        "最常播放",
        Style::new(pink(), mantle()).bold(),
    );
    for column in x + 2..x + width - 2 {
        canvas.put(column, top_y + 1, "─", Style::new(surface0(), mantle()));
    }
    let top = app.stats_top(20);
    let rows = (y + height - 3).saturating_sub(top_y + 2);
    if top.is_empty() {
        canvas.put(
            x + 2,
            top_y + 2,
            "还没有收听记录喵，先放一首",
            Style::new(overlay0(), mantle()),
        );
    } else {
        let offset = app.stats_scroll.min(top.len().saturating_sub(rows.max(1)));
        for (row, (title, plays, seconds)) in top.iter().skip(offset).take(rows).enumerate() {
            let index = offset + row;
            canvas.put(
                x + 2,
                top_y + 2 + row,
                &format!(
                    "{:>2}. {}  · {plays} 次 · {}",
                    index + 1,
                    truncate(title, width.saturating_sub(28)),
                    library::human_duration(*seconds)
                ),
                Style::new(if index == 0 { peach() } else { sub1() }, mantle()),
            );
        }
    }
    canvas.put(
        x + 2,
        y + height - 2,
        "j/k 滚动 · x 清空统计 · Esc 关闭",
        Style::new(overlay0(), mantle()),
    );
}

fn draw_keybindings(canvas: &mut Canvas, app: &App, hits: &mut Vec<(Hit, Rect)>) {
    let width = 78;
    let height = canvas.height().saturating_sub(4).clamp(10, 28);
    let (x, y, width, height) = frame(canvas, width, height, "自定义快捷键", hits);
    let rows = height.saturating_sub(4);
    let offset = app
        .keybind_selected
        .saturating_sub(rows / 2)
        .min(ACTIONS.len().saturating_sub(rows));
    for row in 0..rows {
        let index = offset + row;
        let Some(action) = ACTIONS.get(index) else {
            break;
        };
        let row_y = y + 2 + row;
        let selected = index == app.keybind_selected;
        let background = if selected { surface0() } else { mantle() };
        if selected {
            canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), background));
        }
        canvas.put(
            x + 2,
            row_y,
            &truncate(action.label(), width.saturating_sub(26)),
            if selected {
                Style::new(pink(), background).bold()
            } else {
                Style::new(sub1(), background)
            },
        );
        let key = if selected && app.capture_key {
            "按下新键…".to_string()
        } else {
            truncate(&app.key_for(*action), 18)
        };
        canvas.put(
            x + width - 21,
            row_y,
            &key,
            Style::new(lavender(), background).bold(),
        );
    }
    canvas.put(
        x + 2,
        y + height - 2,
        if app.capture_key {
            "按下新按键 · Esc 取消"
        } else {
            "Enter 改键 · r 恢复默认 · Esc 关闭"
        },
        Style::new(overlay0(), mantle()),
    );
}

fn draw_settings(canvas: &mut Canvas, app: &App, hover: Option<Hit>, hits: &mut Vec<(Hit, Rect)>) {
    let rows = app.settings_rows();
    let width = 78;
    let height = (rows.len() + 5)
        .min(canvas.height().saturating_sub(2))
        .max(5);
    let (x, y, width, height) = frame(canvas, width, height, "设置", hits);
    let visible = height.saturating_sub(4);
    let offset = app
        .setting_selected
        .saturating_sub(visible / 2)
        .min(rows.len().saturating_sub(visible));
    for (index, (label, value)) in rows.iter().enumerate().skip(offset).take(visible) {
        let row_y = y + 2 + (index - offset);
        let selected = app.setting_selected == index;
        let row_hover = hover == Some(Hit::SettingRow(index));
        let background = if selected {
            surface1()
        } else if row_hover {
            surface0()
        } else {
            mantle()
        };
        if selected || row_hover {
            canvas.fill(x + 1, row_y, width - 2, 1, Style::new(txt(), background));
        }
        hits.push((
            Hit::SettingRow(index),
            Rect::new(x + 1, row_y, width - 2, 1),
        ));
        let label_style = if selected {
            Style::new(pink(), background).bold()
        } else {
            Style::new(sub1(), background)
        };
        canvas.put(x + 2, row_y, label, label_style);
        let arrows_width = 8;
        let value_width = width.saturating_sub(label_width() + arrows_width + 4);
        canvas.put(
            x + 16,
            row_y,
            &truncate(value, value_width),
            Style::new(txt(), background),
        );
        if index + 2 >= rows.len() {
            canvas.put(
                x + width - 7,
                row_y,
                "—",
                Style::new(surface2(), background),
            );
            continue;
        }
        let left_hover = hover == Some(Hit::SettingLeft(index));
        let right_hover = hover == Some(Hit::SettingRight(index));
        let left_style = if left_hover {
            Style::new(pink(), surface0()).bold()
        } else {
            Style::new(overlay0(), background)
        };
        let right_style = if right_hover {
            Style::new(pink(), surface0()).bold()
        } else {
            Style::new(overlay0(), background)
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
        Style::new(overlay0(), mantle()),
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
