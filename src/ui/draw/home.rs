use super::*;
use crate::app::App;
use crate::clock;
use crate::library;

/// 首页：一整页的播放面板、概览统计和三栏快捷列表。
pub(super) fn draw_home(
    canvas: &mut Canvas,
    app: &App,
    top: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    if height == 0 || canvas.width() < 2 {
        return;
    }
    let x = 1;
    let width = canvas.width().saturating_sub(2);
    let bottom = top + height;
    let hero_height = if height >= 22 {
        10
    } else if height >= 12 {
        9
    } else {
        height
    }
    .min(height);
    draw_hero(canvas, app, x, top, width, hero_height, hover, hits);

    let mut y = top + hero_height;
    if y >= bottom {
        return;
    }
    if bottom - y >= 5 && width >= 50 {
        y += 1;
        let stats_height = 4.min(bottom - y);
        draw_stats(canvas, app, x, y, width, stats_height, hover, hits);
        y += stats_height;
    }
    if y >= bottom {
        return;
    }
    y += 1;
    if y < bottom {
        draw_lists(canvas, app, x, y, width, bottom - y, hover, hits);
    }
}

fn draw_hero(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    if width < 24 || height < 3 {
        return;
    }
    canvas.fill(x, top, width, height, Style::new(txt(), mantle()));
    let title = if app.current_path().is_some() {
        format!("{PAW} 正在播放")
    } else {
        format!("{PAW} 欢迎回来")
    };
    canvas.border(x, top, width, height, "", Style::new(surface1(), mantle()));
    canvas.put(
        x + 2,
        top,
        &truncate(&format!(" {title} "), width.saturating_sub(4)),
        Style::new(pink(), mantle()).bold(),
    );

    let inner_x = x + 2;
    let inner_y = top + 1;
    let inner_w = width.saturating_sub(4);
    let inner_h = height.saturating_sub(2);
    if inner_w == 0 || inner_h == 0 {
        return;
    }
    let chips_y = inner_y + inner_h - 1;
    let art_w = if inner_w >= 62 {
        14
    } else if inner_w >= 42 {
        10
    } else {
        0
    };
    let art_h = inner_h.saturating_sub(1);
    let content_x = if art_w > 0 && art_h >= 3 {
        draw_art(canvas, app, inner_x, inner_y, art_w, art_h);
        inner_x + art_w + 2
    } else {
        inner_x
    };
    let content_w = (inner_x + inner_w).saturating_sub(content_x);
    if content_w < 12 {
        return;
    }

    let state = &app.mpv.state;
    let content_bottom = chips_y;
    let mut y = inner_y;
    if let Some(path) = &state.path {
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

        let title_width = content_w.saturating_sub(2);
        for line in wrap(&title, title_width, 2) {
            if y >= content_bottom {
                break;
            }
            canvas.put(content_x, y, "▎", Style::new(pink(), mantle()).bold());
            canvas.put(
                content_x + 2,
                y,
                &truncate(&line, title_width),
                Style::new(txt(), mantle()).bold(),
            );
            y += 1;
        }
        if !subtitle.is_empty() && y < content_bottom {
            canvas.put(
                content_x + 2,
                y,
                &truncate(&subtitle, content_w.saturating_sub(2)),
                Style::new(lavender(), mantle()),
            );
            y += 1;
        }
        if y < content_bottom {
            let plays = app.plays_of(path);
            let rating = app.rating_of(path);
            let mut facts = vec![
                (
                    format!("{NOTE} {plays} 次"),
                    if plays > 0 { lavender() } else { overlay0() },
                ),
                (
                    path.extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("音频")
                        .to_uppercase(),
                    sub0(),
                ),
            ];
            if rating > 0 {
                facts.push((format!("★{rating}"), yellow()));
            }
            if let Some(audio) = &state.audio
                && audio.samplerate > 0
            {
                facts.push((format!("{} kHz", audio.samplerate / 1000), sub0()));
            }
            let mut fact_x = content_x + 2;
            for (index, (text, color)) in facts.iter().enumerate() {
                let text_width = text_width(text);
                let gap = if index > 0 { 3 } else { 0 };
                if fact_x + gap + text_width > content_x + content_w {
                    break;
                }
                if index > 0 {
                    canvas.put(fact_x, y, " · ", Style::new(surface2(), mantle()));
                    fact_x += 3;
                }
                canvas.put(fact_x, y, text, Style::new(*color, mantle()));
                fact_x += text_width;
            }
            y += 1;
        }
        if y < content_bottom {
            let time = state.time_pos.unwrap_or(0.0);
            let duration = state.duration.unwrap_or(0.0);
            let ratio = if duration > 0.0 {
                (time / duration).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let clock_text = format!(
                "{} / {}",
                library::duration_text(time),
                library::duration_text(duration)
            );
            let clock_w = text_width(&clock_text).min(content_w / 2);
            let bar_x = content_x + 2;
            let bar_w = content_w.saturating_sub(clock_w + 6);
            if bar_w >= 6 {
                let filled = ((bar_w as f64) * ratio).round() as usize;
                for column in 0..bar_w {
                    let (ch, color) = if column < filled {
                        ("━", rainbow(column as f64 / bar_w.max(1) as f64))
                    } else {
                        ("─", surface1())
                    };
                    canvas.put(bar_x + column, y, ch, Style::new(color, mantle()));
                }
                let knob = filled.min(bar_w.saturating_sub(1));
                canvas.put(
                    bar_x + knob,
                    y,
                    "●",
                    Style::new(
                        if hover == Some(Hit::Progress) {
                            pink()
                        } else {
                            mauve()
                        },
                        mantle(),
                    ),
                );
                hits.push((Hit::Progress, Rect::new(bar_x, y, bar_w.max(1), 1)));
                canvas.put(
                    content_x + content_w - clock_w,
                    y,
                    &truncate(&clock_text, clock_w),
                    Style::new(sub0(), mantle()),
                );
            } else {
                canvas.put(
                    content_x + 2,
                    y,
                    &truncate(&clock_text, content_w.saturating_sub(2)),
                    Style::new(sub0(), mantle()),
                );
            }
        }
    } else {
        if y < content_bottom {
            canvas.put(
                content_x,
                y,
                greeting(),
                Style::new(pink(), mantle()).bold(),
            );
            y += 1;
        }
        if y < content_bottom {
            let hint = format!("曲库里 {} 首歌在等你喵", app.tracks.len());
            canvas.put(
                content_x,
                y,
                &truncate(&hint, content_w),
                Style::new(lavender(), mantle()),
            );
            y += 1;
        }
        if y < content_bottom {
            let hint = match app.home_recent_track(app.home_selected) {
                Some(track) => format!("最近在听 · {}", track.title),
                None => "空格播放，Enter 从最近开始".into(),
            };
            canvas.put(
                content_x,
                y,
                &truncate(&hint, content_w),
                Style::new(sub0(), mantle()),
            );
        }
    }

    draw_home_chips(canvas, app, content_x, chips_y, content_w, hover, hits);
}

fn draw_art(canvas: &mut Canvas, app: &App, x: usize, y: usize, width: usize, height: usize) {
    let path = app.current_path();
    let seed = path
        .as_deref()
        .map(|path| {
            path.to_string_lossy().bytes().fold(0usize, |hash, byte| {
                hash.wrapping_mul(31).wrapping_add(byte as usize)
            })
        })
        .unwrap_or(7);
    let playing = app.current_path().is_some();
    let from = if playing { accent(seed) } else { surface0() };
    let to = if playing {
        accent(seed + 2)
    } else {
        surface1()
    };
    for row in 0..height {
        for column in 0..width {
            let ratio =
                (column as f64 / width.max(1) as f64 + row as f64 / height.max(1) as f64) / 2.0;
            let color = mix(from, to, ratio);
            let background = if app.transparent { mantle() } else { color };
            canvas.put(x + column, y + row, "█", Style::new(color, background));
        }
    }
    if !app.transparent && height >= 3 && width >= 5 {
        let color = mix(from, to, 0.5);
        let icon = if playing { NOTE } else { PAW };
        canvas.put(
            x + (width - 1) / 2,
            y + height / 2,
            icon,
            Style::new(ink_on(color), color).bold(),
        );
    }
}

fn draw_home_chips(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    y: usize,
    width: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let limit = x + width;
    let mut chip_x = x;
    let state = &app.mpv.state;
    let (play_icon, play_label) = if app.current_path().is_none() {
        (PLAY, "播放")
    } else if state.pause {
        (PLAY, "继续")
    } else {
        (PAUSE, "暂停")
    };
    chip_x = draw_chip(
        canvas,
        app,
        chip_x,
        y,
        limit,
        &format!("{play_icon} {play_label}"),
        pink(),
        Some(Hit::PlayPause),
        hover,
        hits,
    );
    chip_x = draw_chip(
        canvas,
        app,
        chip_x,
        y,
        limit,
        &format!("{PREV} 上一首"),
        mauve(),
        Some(Hit::Prev),
        hover,
        hits,
    );
    chip_x = draw_chip(
        canvas,
        app,
        chip_x,
        y,
        limit,
        &format!("{NEXT} 下一首"),
        lavender(),
        Some(Hit::Next),
        hover,
        hits,
    );
    if app.current_path().is_some() {
        let favorite = app
            .selected_track()
            .map(|track| app.cfg.is_favorite(&track.path))
            .unwrap_or(false);
        chip_x = draw_chip(
            canvas,
            app,
            chip_x,
            y,
            limit,
            &format!("{} 收藏", if favorite { HEART } else { HEART_O }),
            maroon(),
            Some(Hit::Favorite),
            hover,
            hits,
        );
        chip_x = draw_chip(
            canvas,
            app,
            chip_x,
            y,
            limit,
            &format!("{NOTE} 歌词"),
            peach(),
            Some(Hit::LyricsFull),
            hover,
            hits,
        );
    }
    chip_x = draw_chip(
        canvas,
        app,
        chip_x,
        y,
        limit,
        &format!("{LIST} 歌单"),
        yellow(),
        Some(Hit::HomePlaylists),
        hover,
        hits,
    );
    chip_x = draw_chip(
        canvas,
        app,
        chip_x,
        y,
        limit,
        &format!("{CLOCK} 统计"),
        accent(5),
        Some(Hit::HomeStats),
        hover,
        hits,
    );
    let _ = draw_chip(
        canvas,
        app,
        chip_x,
        y,
        limit,
        &format!("{RANDOM} 随机"),
        accent(3),
        Some(Hit::Shuffle),
        hover,
        hits,
    );
}

fn draw_stats(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    _hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    if width < 20 || height < 3 {
        return;
    }
    canvas.fill(x, top, width, height, Style::new(txt(), mantle()));
    canvas.border(x, top, width, height, "", Style::new(surface1(), mantle()));
    let title = format!("{LIST} 音乐概览");
    canvas.put(
        x + 2,
        top,
        &truncate(&format!(" {title} "), width.saturating_sub(4)),
        Style::new(pink(), mantle()).bold(),
    );
    let inner_x = x + 2;
    let inner_w = width.saturating_sub(4);
    let stats = [
        ("曲库", format!("{} 首", app.tracks.len()), lavender()),
        ("收藏", format!("{} 首", app.cfg.favorites.len()), maroon()),
        ("最近", format!("{} 首", app.cache.recent.len()), peach()),
        ("歌单", format!("{} 个", app.playlists.len()), yellow()),
        ("收听", short_duration(app.cache.total_seconds), pink()),
    ];
    let columns = if inner_w >= 76 {
        5
    } else if inner_w >= 56 {
        4
    } else {
        3
    };
    let column_w = inner_w / columns;
    if column_w == 0 {
        return;
    }
    for (index, (label, value, color)) in stats.iter().take(columns).enumerate() {
        let sx = inner_x + index * column_w;
        if index > 0 {
            for row in top + 1..top + height - 1 {
                canvas.put(sx - 1, row, "│", Style::new(surface0(), mantle()));
            }
        }
        let content_x = sx + usize::from(index > 0);
        let content_w = column_w.saturating_sub(usize::from(index > 0));
        if height >= 4 {
            canvas.put(content_x, top + 1, label, Style::new(overlay0(), mantle()));
            canvas.put(
                content_x,
                top + 2,
                &truncate(value, content_w.saturating_sub(2)),
                Style::new(*color, mantle()).bold(),
            );
        } else {
            canvas.put(
                content_x,
                top + 1,
                &truncate(&format!("{label} {value}"), content_w.saturating_sub(1)),
                Style::new(*color, mantle()),
            );
        }
    }
    hits.push((Hit::HomeStats, Rect::new(x, top, width, height)));
}

fn draw_lists(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    if width < 24 || height < 3 {
        return;
    }
    let columns = if width >= 96 {
        3
    } else if width >= 52 {
        2
    } else {
        1
    };
    let gap = 1;
    let column_w = (width.saturating_sub(gap * (columns - 1))) / columns;
    if column_w < 18 {
        return;
    }
    let recent = app.home_recent(8);
    let favorites = app.home_favorites(8);
    for index in 0..columns {
        let cx = x + index * (column_w + gap);
        match index {
            0 => draw_recent_panel(canvas, app, cx, top, column_w, height, &recent, hover, hits),
            1 => draw_favorites_panel(
                canvas, app, cx, top, column_w, height, &favorites, hover, hits,
            ),
            _ => draw_playlists_panel(canvas, app, cx, top, column_w, height, hover, hits),
        }
    }
}

fn panel(
    canvas: &mut Canvas,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    title: &str,
    color: Rgb,
    hit: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) -> (usize, usize) {
    canvas.fill(x, top, width, height, Style::new(txt(), mantle()));
    canvas.border(x, top, width, height, "", Style::new(surface1(), mantle()));
    let label = format!(" {title} ");
    let label = truncate(&label, width.saturating_sub(4));
    canvas.put(x + 2, top, &label, Style::new(color, mantle()).bold());
    if let Some(hit) = hit {
        hits.push((hit, Rect::new(x + 2, top, text_width(&label), 1)));
    }
    (x + 2, top + 1)
}

fn draw_recent_panel(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    recent: &[library::Track],
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let title = format!("{CLOCK} 最近播放");
    let (inner_x, inner_y) = panel(
        canvas,
        x,
        top,
        width,
        height,
        &title,
        peach(),
        Some(Hit::Tab(3)),
        hits,
    );
    let inner_w = width.saturating_sub(4);
    let rows = height.saturating_sub(2);
    if recent.is_empty() {
        centered_hint(
            canvas,
            inner_x,
            inner_y,
            inner_w,
            rows,
            "还没有播放记录",
            "按 1 挑一首开始",
            overlay0(),
        );
        return;
    }
    for (index, track) in recent.iter().take(rows).enumerate() {
        let y = inner_y + index;
        let selected = app.home_selected == index;
        let hovered = hover == Some(Hit::HomeRecent(index));
        let active = selected || hovered;
        let background = if app.transparent {
            mantle()
        } else if selected {
            surface0()
        } else if hovered {
            mix(mantle(), surface0(), 0.8)
        } else if index % 2 == 1 {
            mix(mantle(), surface0(), 0.18)
        } else {
            mantle()
        };
        canvas.fill(inner_x, y, inner_w, 1, Style::new(txt(), background));
        canvas.put(
            inner_x,
            y,
            if selected {
                "▌"
            } else if hovered {
                "▏"
            } else {
                " "
            },
            Style::new(if active { pink() } else { mauve() }, background),
        );
        canvas.put(
            inner_x + 1,
            y,
            &format!("{:>2}", index + 1),
            Style::new(if active { pink() } else { accent(index) }, background).bold(),
        );
        let duration = library::duration_text(track.meta.duration);
        let duration_w = if inner_w >= 20 {
            text_width(&duration)
        } else {
            0
        };
        let title_w = inner_w.saturating_sub(4 + duration_w + usize::from(duration_w > 0) * 2);
        canvas.put(
            inner_x + 4,
            y,
            &truncate(&track.title, title_w),
            Style::new(if active { selected_fg() } else { sub1() }, background).bold(),
        );
        if duration_w > 0 {
            canvas.put(
                inner_x + inner_w - duration_w,
                y,
                &duration,
                Style::new(if hovered { sub0() } else { surface2() }, background),
            );
        }
        hits.push((Hit::HomeRecent(index), Rect::new(inner_x, y, inner_w, 1)));
    }
}

fn draw_favorites_panel(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    favorites: &[library::Track],
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let title = format!("{HEART} 我的收藏");
    let (inner_x, inner_y) = panel(
        canvas,
        x,
        top,
        width,
        height,
        &title,
        maroon(),
        Some(Hit::Tab(2)),
        hits,
    );
    let inner_w = width.saturating_sub(4);
    let rows = height.saturating_sub(2);
    if favorites.is_empty() {
        centered_hint(
            canvas,
            inner_x,
            inner_y,
            inner_w,
            rows,
            "还没有收藏",
            "按 f 收藏当前曲目",
            overlay0(),
        );
        return;
    }
    for (index, track) in favorites.iter().take(rows).enumerate() {
        let y = inner_y + index;
        let hovered = hover == Some(Hit::HomeFavorite(index));
        let background = if app.transparent {
            mantle()
        } else if hovered {
            mix(mantle(), surface0(), 0.6)
        } else if index % 2 == 1 {
            mix(mantle(), surface0(), 0.18)
        } else {
            mantle()
        };
        canvas.fill(inner_x, y, inner_w, 1, Style::new(txt(), background));
        canvas.put(inner_x, y, HEART, Style::new(maroon(), background).bold());
        let artist = if track.meta.artist.is_empty() {
            String::new()
        } else {
            format!(" · {}", track.meta.artist)
        };
        let text = format!("{}{}", track.title, artist);
        canvas.put(
            inner_x + 3,
            y,
            &truncate(&text, inner_w.saturating_sub(4)),
            Style::new(if hovered { txt() } else { sub1() }, background),
        );
        hits.push((Hit::HomeFavorite(index), Rect::new(inner_x, y, inner_w, 1)));
    }
}

fn draw_playlists_panel(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    top: usize,
    width: usize,
    height: usize,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) {
    let title = format!("{LIST} 自建歌单");
    let (inner_x, inner_y) = panel(
        canvas,
        x,
        top,
        width,
        height,
        &title,
        yellow(),
        Some(Hit::HomePlaylists),
        hits,
    );
    let inner_w = width.saturating_sub(4);
    let rows = height.saturating_sub(2);
    if app.playlists.is_empty() {
        centered_hint(
            canvas,
            inner_x,
            inner_y,
            inner_w,
            rows,
            "还没有歌单",
            "Ctrl+P 新建一个",
            overlay0(),
        );
        return;
    }
    for (index, playlist) in app.playlists.iter().take(rows).enumerate() {
        let y = inner_y + index;
        let hovered = hover == Some(Hit::HomePlaylist(index));
        let background = if app.transparent {
            mantle()
        } else if hovered {
            mix(mantle(), surface0(), 0.6)
        } else if index % 2 == 1 {
            mix(mantle(), surface0(), 0.18)
        } else {
            mantle()
        };
        canvas.fill(inner_x, y, inner_w, 1, Style::new(txt(), background));
        canvas.put(
            inner_x,
            y,
            NOTE,
            Style::new(accent(index + 1), background).bold(),
        );
        let count = format!("{} 首", playlist.songs.len());
        let count_w = text_width(&count);
        let name_w = inner_w.saturating_sub(3 + count_w + 2);
        canvas.put(
            inner_x + 3,
            y,
            &truncate(&playlist.name, name_w),
            Style::new(if hovered { txt() } else { sub1() }, background).bold(),
        );
        if count_w + 3 < inner_w {
            canvas.put(
                inner_x + inner_w - count_w,
                y,
                &count,
                Style::new(surface2(), background),
            );
        }
        hits.push((Hit::HomePlaylist(index), Rect::new(inner_x, y, inner_w, 1)));
    }
}

fn centered_hint(
    canvas: &mut Canvas,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    text: &str,
    sub: &str,
    color: Rgb,
) {
    if height == 0 {
        return;
    }
    let row = y + height / 2;
    let tx = x + width.saturating_sub(text_width(text)) / 2;
    canvas.put(tx, row, &truncate(text, width), Style::new(color, mantle()));
    if height >= 2 && row + 1 < y + height {
        let sx = x + width.saturating_sub(text_width(sub)) / 2;
        canvas.put(
            sx,
            row + 1,
            &truncate(sub, width),
            Style::new(surface2(), mantle()),
        );
    }
}

fn greeting() -> &'static str {
    let hour = clock::now()
        .get(..2)
        .and_then(|hour| hour.parse::<u32>().ok())
        .unwrap_or(0);
    match hour {
        5..=10 => "早上好，主人",
        11..=13 => "中午好，主人",
        14..=17 => "下午好，主人",
        18..=22 => "晚上好，主人",
        _ => "夜深了，主人",
    }
}

fn short_duration(seconds: f64) -> String {
    if seconds >= 3600.0 {
        format!("{:.1} 小时", seconds / 3600.0)
    } else if seconds >= 60.0 {
        format!("{:.0} 分", seconds / 60.0)
    } else {
        format!("{:.0} 秒", seconds)
    }
}
