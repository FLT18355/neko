mod views;

use super::*;
use crate::app::App;
use crate::library;

pub fn render(canvas: &mut Canvas, app: &App, hover: Option<Hit>, hits: &mut Vec<(Hit, Rect)>) {
    let width = canvas.width();
    let height = canvas.height();
    canvas.fill(0, 0, width, height, Style::new(txt(), base()));
    if width < 64 || height < 14 {
        canvas.centered(
            height / 2,
            "终端太小啦喵，放大一点再玩",
            Style::new(pink(), base()).bold(),
        );
        return;
    }

    if app.lyrics_full {
        views::draw_full_lyrics(canvas, app);
        if let Some(modal) = app.modal {
            views::draw_modal(canvas, app, modal, hover, hits);
        }
        return;
    }

    draw_header(canvas, app, hover, hits);
    let footer_height = 2usize;
    let footer_y = height - footer_height;
    let separator_y = footer_y.saturating_sub(1);
    let body_y = 2usize;
    let body_height = separator_y.saturating_sub(body_y);

    let left_x = 1;
    let left_max = width.saturating_sub(38).max(32);
    let left_width = ((width * 56) / 100).clamp(32, left_max);
    let right_x = left_x + left_width + 1;
    let right_width = width.saturating_sub(right_x + 1);
    let panel_height = body_height;

    canvas.fill(
        left_x,
        body_y,
        left_width,
        panel_height,
        Style::new(txt(), mantle()),
    );
    canvas.fill(
        right_x,
        body_y,
        right_width,
        panel_height,
        Style::new(txt(), mantle()),
    );
    let library_title = format!(
        "{} {} · {} 首{}",
        LIST,
        app.view_label(),
        app.visible.len(),
        if app.filter.is_empty() {
            String::new()
        } else {
            format!(" · 「{}」", app.filter)
        }
    );
    canvas.border(
        left_x,
        body_y,
        left_width,
        panel_height,
        &library_title,
        Style::new(surface1(), mantle()),
    );
    canvas.border(
        right_x,
        body_y,
        right_width,
        panel_height,
        &format!("{} 正在播放", PAW),
        Style::new(surface1(), mantle()),
    );

    draw_library(
        canvas,
        app,
        left_x + 2,
        body_y + 1,
        left_width.saturating_sub(3),
        panel_height.saturating_sub(2),
        hover,
        hits,
    );
    draw_now_playing(
        canvas,
        app,
        right_x + 2,
        body_y + 1,
        right_width.saturating_sub(3),
        panel_height.saturating_sub(2),
        hover,
        hits,
    );

    draw_gradient_line(canvas, separator_y, pink(), mauve(), lavender());
    draw_footer(canvas, app, footer_y, width, height, hover, hits);

    if let Some(modal) = app.modal {
        views::draw_modal(canvas, app, modal, hover, hits);
    }
}

fn draw_gradient_line(canvas: &mut Canvas, y: usize, start: Rgb, middle: Rgb, end: Rgb) {
    let width = canvas.width();
    for column in 0..width {
        let ratio = column as f64 / width.max(1) as f64;
        let (from, to, local) = if ratio < 0.5 {
            (start, middle, ratio * 2.0)
        } else {
            (middle, end, (ratio - 0.5) * 2.0)
        };
        let color = (
            (from.0 as f64 + (to.0 as f64 - from.0 as f64) * local) as u8,
            (from.1 as f64 + (to.1 as f64 - from.1 as f64) * local) as u8,
            (from.2 as f64 + (to.2 as f64 - from.2 as f64) * local) as u8,
        );
        canvas.put(column, y, "─", Style::new(color, base()));
    }
}

fn draw_header(canvas: &mut Canvas, app: &App, hover: Option<Hit>, hits: &mut Vec<(Hit, Rect)>) {
    let width = canvas.width();
    canvas.fill(0, 0, width, 1, Style::new(txt(), crust()));
    canvas.put(
        1,
        0,
        &format!("{} neko", NOTE),
        Style::new(pink(), crust()).bold(),
    );
    canvas.put(8, 0, "本地音乐", Style::new(overlay0(), crust()));
    canvas.put(17, 0, "│", Style::new(surface1(), crust()));

    let mut x = 20;
    let tabs = [
        ("全部", app.view == "all"),
        ("收藏", app.view == "fav"),
        ("最近", app.view == "recent"),
        ("文件夹", app.view == "folder"),
    ];
    for (index, (label, active)) in tabs.into_iter().enumerate() {
        let hovered = hover == Some(Hit::Tab(index));
        let style = if active {
            Style::new(crust(), pink()).bold()
        } else if hovered {
            Style::new(selected_fg(), surface0()).bold()
        } else {
            Style::new(sub0(), crust())
        };
        let text = format!(" {label} ");
        let width = text_width(&text);
        canvas.put(x, 0, &text, style);
        hits.push((Hit::Tab(index), Rect::new(x, 0, width, 1)));
        x += width + 1;
    }

    let state = &app.mpv.state;
    let volume = if state.mute {
        format!("{} 静音", MUTE)
    } else {
        format!("{} {:.0}%", VOL, state.volume)
    };
    let mut status = format!("{} · {volume}", app.mode_label());
    if (state.speed - 1.0).abs() > 0.001 {
        status.push_str(&format!(" · {:.2}x", state.speed));
    }
    if let Some(deadline) = app.sleep_at {
        let minutes = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_secs()
            .div_ceil(60);
        status.push_str(&format!(" · {} {}m", CLOCK, minutes.max(1)));
    }
    let status_width = text_width(&status);
    if status_width + x + 3 < width {
        canvas.put(
            width - status_width - 2,
            0,
            &status,
            Style::new(if state.pause { overlay0() } else { sub1() }, crust()),
        );
    }
    draw_gradient_line(canvas, 1, surface0(), surface1(), mauve());
}

#[allow(clippy::too_many_arguments)]
fn draw_library(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let total = app.visible.len();
    if height == 0 || width == 0 {
        return;
    }
    if total == 0 {
        let art = ["   /\\_/\\   ", "  ( o.o )  ", "   > ^ <   "];
        let start = top + height.saturating_sub(art.len() + 3) / 2;
        for (row, line) in art.iter().enumerate() {
            let text = line.trim_end();
            let offset = (width.saturating_sub(text_width(text))) / 2;
            canvas.put(x + offset, start + row, text, Style::new(mauve(), mantle()));
        }
        let hint = "空空的喵…按 o 换个目录";
        let offset = (width.saturating_sub(text_width(hint))) / 2;
        canvas.put(
            x + offset,
            start + art.len() + 1,
            hint,
            Style::new(overlay0(), mantle()),
        );
        return;
    }

    let rows = height;
    let offset = app
        .selected
        .saturating_sub(rows / 2)
        .min(total.saturating_sub(rows));
    let current = app.current_path();
    for (row, index) in (offset..total.min(offset + rows)).enumerate() {
        let track = &app.tracks[app.visible[index]];
        let y = top + row;
        let selected = index == app.selected;
        let hovered = hover == Some(Hit::Row(index));
        let playing = current.as_ref() == Some(&track.path);
        let background = if selected {
            surface0()
        } else if hovered {
            mix(mantle(), surface0(), 0.5)
        } else {
            mantle()
        };
        canvas.fill(x, y, width, 1, Style::new(txt(), background));
        hits.push((Hit::Row(index), Rect::new(x, y, width, 1)));
        canvas.put(
            x,
            y,
            if selected { "▌" } else { " " },
            Style::new(pink(), background),
        );
        canvas.put(
            x + 1,
            y,
            if playing { NOTE } else { " " },
            Style::new(pink(), background).bold(),
        );
        canvas.put(
            x + 3,
            y,
            &format!("{:>3}", index + 1),
            Style::new(overlay0(), background),
        );
        if app.cfg.is_favorite(&track.path) {
            canvas.put(x + 7, y, HEART, Style::new(maroon(), background));
        }
        let rating = app.rating_of(&track.path);
        if rating > 0 {
            canvas.put(
                x + 8,
                y,
                &format!("★{rating}"),
                Style::new(yellow(), background),
            );
        }
        let title_style = if playing {
            Style::new(pink(), background).bold()
        } else if selected {
            Style::new(selected_fg(), background).bold()
        } else {
            Style::new(txt(), background)
        };
        let title_x = x + 11;
        let title_width = width.saturating_sub(13);
        canvas.put(
            title_x,
            y,
            &truncate(&track.title, title_width),
            title_style,
        );
        if let Some(parent) = track.path.parent()
            && let Ok(relative) = parent.strip_prefix(&app.cfg.music_dir)
        {
            let folder = relative.to_string_lossy();
            if !folder.is_empty() {
                let text = format!(" {folder}");
                let text = truncate(&text, title_width / 3);
                let folder_width = text_width(&text);
                if folder_width + 12 < width {
                    canvas.put(
                        x + width - folder_width - 2,
                        y,
                        &text,
                        Style::new(overlay0(), background),
                    );
                }
            }
        }
    }

    let right = x + width - 1;
    if total > rows {
        let thumb_height = (rows * rows / total).max(1);
        let max_offset = total - rows;
        let thumb_top = offset * (rows - thumb_height) / max_offset.max(1);
        for row in 0..rows {
            let thumb = row >= thumb_top && row < thumb_top + thumb_height;
            canvas.put(
                right,
                top + row,
                if thumb { "▐" } else { "│" },
                Style::new(if thumb { lavender() } else { surface0() }, mantle()),
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn draw_now_playing(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    if width < 20 || height < 6 {
        return;
    }
    let state = &app.mpv.state;
    let Some(path) = state.path.as_ref() else {
        let art = ["  /\\_/\\  ", " ( o.o ) ", "  > ^ <  "];
        let start = top + height.saturating_sub(art.len() + 3) / 2;
        for (row, line) in art.iter().enumerate() {
            let offset = (width.saturating_sub(text_width(line))) / 2;
            canvas.put(x + offset, start + row, line, Style::new(mauve(), mantle()));
        }
        let hint = "选一首歌，按 Enter 开始喵";
        let offset = (width.saturating_sub(text_width(hint))) / 2;
        canvas.put(
            x + offset,
            start + art.len() + 1,
            hint,
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
    let mut y = top;
    for line in wrap(&title, width, 2) {
        canvas.put(x, y, &line, Style::new(txt(), mantle()).bold());
        y += 1;
    }
    let artist = state
        .metadata
        .get("artist")
        .filter(|value| !value.is_empty())
        .cloned();
    let album = state
        .metadata
        .get("album")
        .filter(|value| !value.is_empty())
        .cloned();
    let subtitle = match (artist, album) {
        (Some(artist), Some(album)) => format!("{artist} · {album}"),
        (Some(artist), None) => artist,
        (None, Some(album)) => album,
        (None, None) => path
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default(),
    };
    if !subtitle.is_empty() {
        canvas.put(
            x,
            y,
            &truncate(&subtitle, width),
            Style::new(sub1(), mantle()),
        );
    }
    y += 2;

    let time = state.time_pos.unwrap_or(0.0);
    let duration = state.duration.unwrap_or(0.0);
    let ratio = if duration > 0.0 {
        (time / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bar_width = width;
    let filled = ((bar_width as f64) * ratio).round() as usize;
    let bar_y = y;
    for column in 0..bar_width {
        let (ch, color) = if column < filled {
            let local = column as f64 / bar_width.max(1) as f64;
            ("━", mix(pink(), mauve(), local))
        } else {
            ("─", surface1())
        };
        canvas.put(x + column, bar_y, ch, Style::new(color, mantle()));
    }
    hits.push((Hit::Progress, Rect::new(x, bar_y, bar_width.max(1), 1)));
    y += 1;

    let time_label = if app.remaining {
        format!("-{}", library::duration_text((duration - time).max(0.0)))
    } else {
        library::duration_text(time)
    };
    let total = library::duration_text(duration);
    let clock_text = format!("{time_label} / {total}");
    let clock_width = text_width(&clock_text);
    let clock_style = if hover == Some(Hit::Time) {
        Style::new(pink(), mantle()).bold()
    } else {
        Style::new(sub0(), mantle())
    };
    canvas.put(x, y, &clock_text, clock_style);
    hits.push((Hit::Time, Rect::new(x, y, clock_width, 1)));

    let back_x = x + width.saturating_sub(11);
    let forward_x = x + width.saturating_sub(5);
    let button_style = |hit: Hit| {
        if hover == Some(hit) {
            Style::new(pink(), surface0()).bold()
        } else {
            Style::new(overlay0(), mantle())
        }
    };
    canvas.put(back_x, y, "-5s", button_style(Hit::SeekBack));
    hits.push((Hit::SeekBack, Rect::new(back_x, y, 3, 1)));
    canvas.put(forward_x, y, "+5s", button_style(Hit::SeekForward));
    hits.push((Hit::SeekForward, Rect::new(forward_x, y, 3, 1)));
    y += 1;

    let mute_x = x;
    let mute_icon = if state.mute { MUTE } else { VOL };
    let mute_style = if hover == Some(Hit::MuteButton) {
        Style::new(pink(), surface0()).bold()
    } else {
        Style::new(sub0(), mantle())
    };
    canvas.put(mute_x, y, mute_icon, mute_style);
    hits.push((Hit::MuteButton, Rect::new(mute_x, y, 1, 1)));
    let percent = if state.mute {
        "静音".to_string()
    } else {
        format!("{:.0}%", state.volume)
    };
    let percent_width = text_width(&percent);
    let slider_x = x + 3;
    let slider_width = width.saturating_sub(percent_width + 6);
    let volume_ratio = (state.volume / 100.0).clamp(0.0, 1.0);
    let volume_filled = (slider_width as f64 * volume_ratio).round() as usize;
    for column in 0..slider_width {
        let ch = if column < volume_filled { "━" } else { "─" };
        let color = if column < volume_filled {
            lavender()
        } else {
            surface1()
        };
        canvas.put(slider_x + column, y, ch, Style::new(color, mantle()));
    }
    if !state.mute && volume_filled < slider_width {
        canvas.put(
            slider_x + volume_filled,
            y,
            "●",
            Style::new(
                if hover == Some(Hit::Volume) {
                    pink()
                } else {
                    mauve()
                },
                mantle(),
            ),
        );
    }
    hits.push((Hit::Volume, Rect::new(slider_x, y, slider_width.max(1), 1)));
    canvas.put(
        x + width.saturating_sub(percent_width),
        y,
        &percent,
        Style::new(overlay0(), mantle()),
    );
    y += 1;

    let mut chip_x = x;
    chip_x = draw_chip(canvas, chip_x, y, app.mode_label(), pink());
    if (state.speed - 1.0).abs() > 0.001 {
        chip_x = draw_chip(canvas, chip_x, y, &format!("{:.2}x", state.speed), peach());
    }
    if state.pause {
        chip_x = draw_chip(canvas, chip_x, y, "已暂停", txt());
    }
    if let Some(deadline) = app.sleep_at {
        let minutes = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_secs()
            .div_ceil(60)
            .max(1);
        chip_x = draw_chip(
            canvas,
            chip_x,
            y,
            &format!("{} {minutes}m", CLOCK),
            yellow(),
        );
    }
    if let (Some(position), Some(count)) = (state.playlist_pos, state.playlist_count) {
        let _ = draw_chip(
            canvas,
            chip_x,
            y,
            &format!("{}/{}", position + 1, count),
            lavender(),
        );
    }
    y += 1;

    for column in 0..width {
        canvas.put(x + column, y, "─", Style::new(surface0(), mantle()));
    }
    y += 1;

    let lyrics_title = if app.lrc_offset.abs() > 0.01 {
        format!("歌词  {:+.1}s", app.lrc_offset)
    } else {
        "歌词".into()
    };
    canvas.put(x, y, &lyrics_title, Style::new(pink(), mantle()).bold());
    y += 1;
    if app.lyrics_enabled() {
        if let Some((index, lines)) = app.current_lyric() {
            let available = (top + height).saturating_sub(y).saturating_sub(1);
            let end = (index + available).min(lines.len());
            for (line_index, (_, text)) in lines.iter().enumerate().take(end).skip(index) {
                if line_index == index {
                    canvas.put(
                        x,
                        y,
                        &format!("{} {}", NOTE, truncate(text, width.saturating_sub(2))),
                        Style::new(pink(), mantle()).bold(),
                    );
                } else {
                    canvas.put(
                        x,
                        y,
                        &truncate(text, width),
                        Style::new(overlay0(), mantle()),
                    );
                }
                y += 1;
            }
        } else {
            canvas.put(
                x,
                y,
                "没有找到同名的 .lrc 喵",
                Style::new(overlay0(), mantle()),
            );
        }
    } else {
        canvas.put(
            x,
            y,
            "歌词已关闭 · 按 l 打开",
            Style::new(overlay0(), mantle()),
        );
    }

    let info = match (state.audio.as_ref(), state.bitrate) {
        (Some(audio), Some(bitrate)) => format!(
            "{} · {}Hz · {}声道 · {:.0}kbps",
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("音频"),
            audio.samplerate,
            audio.channels,
            bitrate / 1000.0
        ),
        (Some(audio), None) => format!(
            "{} · {}Hz · {}声道",
            path.extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("音频"),
            audio.samplerate,
            audio.channels
        ),
        _ => String::new(),
    };
    if !info.is_empty() {
        canvas.put(
            x,
            top + height - 1,
            &truncate(&info, width),
            Style::new(surface2(), mantle()),
        );
    }
}

fn mix(from: Rgb, to: Rgb, ratio: f64) -> Rgb {
    let ratio = ratio.clamp(0.0, 1.0);
    (
        (from.0 as f64 + (to.0 as f64 - from.0 as f64) * ratio) as u8,
        (from.1 as f64 + (to.1 as f64 - from.1 as f64) * ratio) as u8,
        (from.2 as f64 + (to.2 as f64 - from.2 as f64) * ratio) as u8,
    )
}

fn draw_chip(canvas: &mut Canvas, x: usize, y: usize, text: &str, color: Rgb) -> usize {
    let color = crate::theme::shade(color);
    let label = format!(" {text} ");
    let width = text_width(&label);
    canvas.put(x, y, &label, Style::new(color, surface0()));
    x + width + 1
}

#[allow(clippy::too_many_arguments)]
fn draw_footer(
    canvas: &mut Canvas,
    app: &App,
    y: usize,
    width: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    canvas.fill(0, y, width, height - y, Style::new(txt(), crust()));
    let message = hover
        .map(|hit| hit.label().to_string())
        .or_else(|| {
            app.toast
                .as_ref()
                .filter(|(_, at)| at.elapsed().as_secs() < 4)
                .map(|(text, _)| text.clone())
        })
        .unwrap_or_else(|| {
            format!(
                "{} {} · {} · 音量 {:.0}%",
                if app.mpv.state.pause { PAUSE } else { PLAY },
                app.mode_label(),
                app.effect_label(),
                app.mpv.state.volume
            )
        });
    let style = if hover.is_some() {
        Style::new(lavender(), mantle()).bold()
    } else {
        Style::new(pink(), mantle())
    };
    let line = format!("{} {message}", PAW);
    canvas.fill(
        2,
        y,
        text_width(&line).min(width.saturating_sub(4)) + 2,
        1,
        Style::new(txt(), surface0()),
    );
    canvas.put(3, y, &truncate(&line, width.saturating_sub(6)), style);
    if height <= y + 1 {
        return;
    }
    let mut x = 2;
    let play_label = if app.mpv.state.pause {
        "继续"
    } else {
        "暂停"
    };
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Prev,
        PREV,
        "上一首",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::PlayPause,
        if app.mpv.state.pause { PLAY } else { PAUSE },
        play_label,
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Next,
        NEXT,
        "下一首",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Mode,
        REPEAT,
        "模式",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Shuffle,
        RANDOM,
        "随机",
        hover,
        hits,
    );
    let favorite = app
        .selected_track()
        .map(|track| app.cfg.is_favorite(&track.path))
        .unwrap_or(false);
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Favorite,
        if favorite { HEART } else { HEART_O },
        "收藏",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Search,
        SEARCH,
        "搜索",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Queue,
        LIST,
        "队列",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Settings,
        GEAR,
        "设置",
        hover,
        hits,
    );
    x = control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Help,
        "?",
        "帮助",
        hover,
        hits,
    );
    control(
        canvas,
        x,
        y + 1,
        width.saturating_sub(2),
        Hit::Quit,
        POWER,
        "退出",
        hover,
        hits,
    );
}

#[allow(clippy::too_many_arguments)]
fn control(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    limit: usize,
    hit: Hit,
    icon: &str,
    label: &str,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) -> usize {
    let text = if label.is_empty() || canvas.width() < 100 {
        icon.to_string()
    } else {
        format!("{icon} {label}")
    };
    let button_width = text_width(&text) + 2;
    if x + button_width > limit {
        return x;
    }
    let active = hover == Some(hit);
    let style = if active {
        Style::new(pink(), surface0()).bold()
    } else {
        Style::new(sub0(), mantle())
    };
    canvas.fill(x, y, button_width, 1, Style::new(style.fg, style.bg));
    canvas.put(x + 1, y, &text, style);
    hits.push((hit, Rect::new(x, y, button_width, 1)));
    x + button_width + 1
}

fn wrap(text: &str, width: usize, max_lines: usize) -> Vec<String> {
    let mut lines = Vec::new();
    let mut current = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let char_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + char_width > width && !current.is_empty() {
            lines.push(current);
            current = String::new();
            used = 0;
            if lines.len() == max_lines {
                break;
            }
        }
        current.push(ch);
        used += char_width;
    }
    if lines.len() < max_lines && !current.is_empty() {
        lines.push(current);
    }
    lines
}
