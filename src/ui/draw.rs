mod views;

use super::*;
use crate::app::App;
use crate::clock;
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
        views::draw_full_lyrics(canvas, app, hover, hits);
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

    draw_rainbow_line(canvas, separator_y);
    draw_footer(canvas, app, footer_y, width, height, hover, hits);

    if let Some(modal) = app.modal {
        views::draw_modal(canvas, app, modal, hover, hits);
    }
}

fn draw_rainbow_line(canvas: &mut Canvas, y: usize) {
    let width = canvas.width();
    for column in 0..width {
        let color = rainbow(column as f64 / width.max(1) as f64);
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
        let color = accent(index);
        let style = if active {
            Style::new(ink_on(color), color).bold()
        } else if hovered {
            Style::new(selected_fg(), surface0()).bold()
        } else {
            Style::new(color, crust()).bold()
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
    let clock_text = format!("{} {}", CLOCK, clock::now());
    let clock_width = text_width(&clock_text);
    let clock_x = width.saturating_sub(clock_width + 2);
    if clock_width + 20 < width {
        canvas.put(
            clock_x,
            0,
            &clock_text,
            Style::new(peach(), crust()).bold(),
        );
    }
    let status_width = text_width(&status);
    if status_width + x + 3 < clock_x {
        canvas.put(
            clock_x.saturating_sub(status_width + 2),
            0,
            &status,
            Style::new(if state.pause { overlay0() } else { sub1() }, crust()),
        );
    }
    draw_rainbow_line(canvas, 1);
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
            canvas.put(
                x + offset,
                start + row,
                text,
                Style::new(accent(row), mantle()).bold(),
            );
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
        let background = if app.transparent {
            mantle()
        } else if selected {
            surface0()
        } else if hovered {
            mix(mantle(), surface0(), 0.5)
        } else if index % 2 == 0 {
            mix(mantle(), surface0(), 0.22)
        } else {
            mantle()
        };
        canvas.fill(x, y, width, 1, Style::new(txt(), background));
        hits.push((Hit::Row(index), Rect::new(x, y, width, 1)));
        // 透明模式没有行底色，悬停改用左侧标记和加粗标题提示
        canvas.put(
            x,
            y,
            if selected {
                "▌"
            } else if hovered {
                "▏"
            } else {
                " "
            },
            Style::new(if selected { pink() } else { mauve() }, background),
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
        } else if selected || hovered {
            Style::new(selected_fg(), background).bold()
        } else {
            Style::new(txt(), background)
        };
        let plays = app.plays_of(&track.path);
        let play_text = format!("{plays} 次");
        let play_width = if width >= 38 {
            text_width(&play_text)
        } else {
            0
        };
        let title_x = x + 11;
        let title_width = width.saturating_sub(13 + play_width.saturating_add(2));
        canvas.put(
            title_x,
            y,
            &truncate(&track.title, title_width),
            title_style,
        );
        if play_width > 0 {
            canvas.put(
                x + width - play_width - 2,
                y,
                &play_text,
                Style::new(if playing { pink() } else { surface2() }, background),
            );
        }
        let folder = app.relative_folder(&track.path);
        if !folder.is_empty() {
            let text = format!(" {folder}");
            let text = truncate(&text, title_width / 3);
            let folder_width = text_width(&text);
            if folder_width + play_width + 12 < width {
                canvas.put(
                    x + width - play_width - folder_width - 3,
                    y,
                    &text,
                    Style::new(mix(overlay0(), accent(index + 2), 0.55), background),
                );
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
                Style::new(
                    if thumb {
                        rainbow(row as f64 / rows.max(1) as f64)
                    } else {
                        surface0()
                    },
                    mantle(),
                ),
            );
        }
        hits.push((
            Hit::Scrollbar,
            Rect::new(x + width.saturating_sub(2), top, 2, rows),
        ));
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
            canvas.put(
                x + offset,
                start + row,
                line,
                Style::new(accent(row), mantle()).bold(),
            );
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

    let bottom = top + height;
    let mut y = top;
    let title_lines = wrap(&title, width.saturating_sub(4), 2);
    let card_height = title_lines.len() + usize::from(!subtitle.is_empty());
    if card_height > 0 && card_height < height {
        let card_bg = if app.transparent {
            mantle()
        } else {
            mix(
                mantle(),
                surface0(),
                if hover == Some(Hit::TrackInfo) {
                    0.8
                } else {
                    0.55
                },
            )
        };
        canvas.fill(x, y, width, card_height, Style::new(txt(), card_bg));
        hits.push((Hit::TrackInfo, Rect::new(x, y, width, card_height)));
        for (row, line) in title_lines.iter().enumerate() {
            canvas.put(x + 1, y + row, "▎", Style::new(pink(), card_bg).bold());
            canvas.put(
                x + 3,
                y + row,
                &truncate(line, width.saturating_sub(5)),
                Style::new(txt(), card_bg).bold(),
            );
        }
        if width >= 30 {
            canvas.put(x + width - 2, y, NOTE, Style::new(pink(), card_bg));
        }
        if !subtitle.is_empty() {
            canvas.put(
                x + 3,
                y + title_lines.len(),
                &truncate(&subtitle, width.saturating_sub(5)),
                Style::new(lavender(), card_bg),
            );
        }
        y += card_height;
    }
    y += 1;

    let plays = app.plays_of(path);
    let rating = app.rating_of(path);
    let mut facts: Vec<(String, Rgb)> = vec![
        (
            format!("▶ {plays} 次"),
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
    if y < bottom {
        let mut fact_x = x;
        for (index, (text, color)) in facts.iter().enumerate() {
            if index > 0 {
                if fact_x + 3 > x + width {
                    break;
                }
                canvas.put(fact_x, y, " · ", Style::new(surface2(), mantle()));
                fact_x += 3;
            }
            let text_width = text_width(text);
            if fact_x + text_width > x + width {
                break;
            }
            canvas.put(fact_x, y, text, Style::new(*color, mantle()));
            fact_x += text_width;
        }
        y += 1;
    }

    let time = state.time_pos.unwrap_or(0.0);
    let duration = state.duration.unwrap_or(0.0);
    let ratio = if duration > 0.0 {
        (time / duration).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let bar_width = width;
    let filled = ((bar_width as f64) * ratio).round() as usize;
    if y < bottom {
        let bar_y = y;
        for column in 0..bar_width {
            let (ch, color) = if column < filled {
                ("━", rainbow(column as f64 / bar_width.max(1) as f64))
            } else {
                ("─", surface1())
            };
            canvas.put(x + column, bar_y, ch, Style::new(color, mantle()));
        }
        if bar_width > 0 {
            let knob = if filled >= bar_width {
                bar_width - 1
            } else {
                filled
            };
            canvas.put(
                x + knob,
                bar_y,
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
        }
        hits.push((Hit::Progress, Rect::new(x, bar_y, bar_width.max(1), 1)));
        y += 1;
    }

    if y < bottom {
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
        let percent_text = format!("{:.0}%", ratio * 100.0);
        let percent_x = x + width.saturating_sub(17);
        if percent_x > x + clock_width + 1 {
            canvas.put(
                percent_x,
                y,
                &percent_text,
                Style::new(surface2(), mantle()),
            );
        }

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
    }

    if y < bottom {
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
                rainbow(column as f64 / slider_width.max(1) as f64)
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
    }

    if y < bottom {
        let limit = x + width;
        let mut chip_x = x;
        chip_x = draw_chip(
            canvas,
            app,
            chip_x,
            y,
            limit,
            app.mode_label(),
            pink(),
            Some(Hit::ModeChip),
            hover,
            hits,
        );
        if (state.speed - 1.0).abs() > 0.001 {
            chip_x = draw_chip(
                canvas,
                app,
                chip_x,
                y,
                limit,
                &format!("{:.2}x", state.speed),
                peach(),
                Some(Hit::SpeedChip),
                hover,
                hits,
            );
        }
        if state.pause {
            chip_x = draw_chip(
                canvas,
                app,
                chip_x,
                y,
                limit,
                "已暂停",
                txt(),
                Some(Hit::PauseChip),
                hover,
                hits,
            );
        }
        if let Some(deadline) = app.sleep_at {
            let minutes = deadline
                .saturating_duration_since(std::time::Instant::now())
                .as_secs()
                .div_ceil(60)
                .max(1);
            chip_x = draw_chip(
                canvas,
                app,
                chip_x,
                y,
                limit,
                &format!("{} {minutes}m", CLOCK),
                yellow(),
                Some(Hit::SleepChip),
                hover,
                hits,
            );
        }
        if let (Some(position), Some(count)) = (state.playlist_pos, state.playlist_count) {
            chip_x = draw_chip(
                canvas,
                app,
                chip_x,
                y,
                limit,
                &format!("{}/{}", position + 1, count),
                lavender(),
                Some(Hit::QueueChip),
                hover,
                hits,
            );
        }
        if app.a_point.is_some() || app.b_point.is_some() {
            let label = match (app.a_point, app.b_point) {
                (Some(_), Some(_)) => "A-B",
                (Some(_), None) => "A-B A",
                _ => "A-B B",
            };
            chip_x = draw_chip(
                canvas,
                app,
                chip_x,
                y,
                limit,
                label,
                maroon(),
                Some(Hit::AbChip),
                hover,
                hits,
            );
        }
        if app.cfg.eq_on {
            let _ = draw_chip(
                canvas,
                app,
                chip_x,
                y,
                limit,
                "EQ",
                peach(),
                Some(Hit::EqChip),
                hover,
                hits,
            );
        }
        y += 1;
    }

    if y < bottom {
        if let Some(track) = app.next_track() {
            let title = truncate(&track.title, width.saturating_sub(11));
            canvas.put(
                x,
                y,
                &format!("{NEXT} 下一首  {title}"),
                Style::new(
                    if hover == Some(Hit::NextPreview) {
                        pink()
                    } else {
                        sub1()
                    },
                    mantle(),
                ),
            );
            hits.push((Hit::NextPreview, Rect::new(x, y, width, 1)));
        } else {
            canvas.put(
                x,
                y,
                &format!("{NEXT} 队列结尾"),
                Style::new(overlay0(), mantle()),
            );
        }
        y += 1;
    }

    if y < bottom {
        for column in 0..width {
            let ratio = column as f64 / width.max(1) as f64;
            canvas.put(
                x + column,
                y,
                "─",
                Style::new(mix(surface1(), mauve(), ratio * 0.6), mantle()),
            );
        }
        y += 1;
    }

    if y < bottom {
        let lyrics_header = format!("{} 歌词", NOTE);
        canvas.put(
            x,
            y,
            &lyrics_header,
            Style::new(
                if hover == Some(Hit::LyricsFull) {
                    lavender()
                } else {
                    pink()
                },
                mantle(),
            )
            .bold(),
        );
        hits.push((
            Hit::LyricsFull,
            Rect::new(x, y, text_width(&lyrics_header), 1),
        ));
        if app.lrc_offset.abs() > 0.01 {
            let offset = format!("{:+.1}s", app.lrc_offset);
            let offset_width = text_width(&offset);
            if offset_width + 4 < width {
                canvas.put(
                    x + width - offset_width,
                    y,
                    &offset,
                    Style::new(peach(), mantle()),
                );
            }
        }
        y += 1;
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
    let lyric_bottom = if info.is_empty() {
        bottom
    } else {
        bottom.saturating_sub(1)
    };
    if y < lyric_bottom {
        if app.lyrics_enabled() {
            if let Some((index, lines)) = app.current_lyric() {
                let available = lyric_bottom.saturating_sub(y);
                let (start, end) = crate::lyrics::window(index, lines.len(), available);
                let highlight_bg = if app.transparent {
                    mantle()
                } else {
                    mix(mantle(), surface0(), 0.65)
                };
                for (line_index, (_, text)) in lines.iter().enumerate().take(end).skip(start) {
                    let row_y = y + line_index - start;
                    if line_index == index {
                        canvas.fill(x, row_y, width, 1, Style::new(txt(), highlight_bg));
                        canvas.put(x, row_y, "▎", Style::new(pink(), highlight_bg).bold());
                        canvas.put(
                            x + 2,
                            row_y,
                            &format!("{} {}", NOTE, truncate(text, width.saturating_sub(4))),
                            Style::new(pink(), highlight_bg).bold(),
                        );
                    } else {
                        canvas.put(
                            x + 2,
                            row_y,
                            &truncate(text, width.saturating_sub(2)),
                            Style::new(overlay0(), mantle()),
                        );
                    }
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
    }

    if !info.is_empty() && height > 0 && y < bottom {
        canvas.put(
            x,
            bottom - 1,
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

#[allow(clippy::too_many_arguments)]
fn draw_chip(
    canvas: &mut Canvas,
    app: &App,
    x: usize,
    y: usize,
    limit: usize,
    text: &str,
    color: Rgb,
    hit: Option<Hit>,
    hover: Option<Hit>,
    hits: &mut Vec<(Hit, Rect)>,
) -> usize {
    let color = crate::theme::shade(color);
    let label = format!(" {text} ");
    let width = text_width(&label);
    if x + width > limit {
        return x;
    }
    let active = hit.is_some() && hover == hit;
    let background = if active {
        surface1()
    } else if app.transparent {
        mantle()
    } else {
        surface0()
    };
    canvas.put(
        x,
        y,
        &label,
        if active {
            Style::new(pink(), background).bold()
        } else {
            Style::new(color, background)
        },
    );
    if let Some(hit) = hit {
        hits.push((hit, Rect::new(x, y, width, 1)));
    }
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
        Style::new(
            txt(),
            if app.transparent { mantle() } else { surface0() },
        ),
    );
    canvas.put(3, y, &truncate(&line, width.saturating_sub(6)), style);
    if height <= y + 1 {
        return;
    }
    let favorite = app
        .selected_track()
        .map(|track| app.cfg.is_favorite(&track.path))
        .unwrap_or(false);
    let controls = [
        (PREV, Hit::Prev, "上一首"),
        (
            if app.mpv.state.pause { PLAY } else { PAUSE },
            Hit::PlayPause,
            if app.mpv.state.pause { "继续" } else { "暂停" },
        ),
        (NEXT, Hit::Next, "下一首"),
        (REPEAT, Hit::Mode, "模式"),
        (RANDOM, Hit::Shuffle, "随机"),
        (
            if favorite { HEART } else { HEART_O },
            Hit::Favorite,
            "收藏",
        ),
        (SEARCH, Hit::Search, "搜索"),
        (LIST, Hit::Queue, "队列"),
        (GEAR, Hit::Settings, "设置"),
        ("?", Hit::Help, "帮助"),
        (POWER, Hit::Quit, "退出"),
    ];
    let mut x = 2;
    for (index, (icon, hit, label)) in controls.into_iter().enumerate() {
        x = control(
            canvas,
            x,
            y + 1,
            width.saturating_sub(2),
            hit,
            icon,
            label,
            accent(index + 3),
            hover,
            hits,
        );
    }
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
    color: Rgb,
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
    let (fg, bg) = if active {
        (ink_on(color), color)
    } else {
        (color, mantle())
    };
    canvas.fill(x, y, button_width, 1, Style::new(fg, bg));
    canvas.put(x + 1, y, &text, Style::new(fg, bg).bold());
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
