use crate::app::{App, Modal};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
        app.quit = true;
        return;
    }
    if app.modal.is_some() {
        handle_modal(app, key);
        return;
    }
    let shift = key.modifiers.contains(KeyModifiers::SHIFT);
    match key.code {
        KeyCode::Char('q') => app.quit = true,
        KeyCode::Char(' ') => app.play_or_toggle(),
        KeyCode::Enter => app.play_selected(),
        KeyCode::Char('j') | KeyCode::Down => app.move_selection(1),
        KeyCode::Char('k') | KeyCode::Up => app.move_selection(-1),
        KeyCode::PageDown => app.move_selection(10),
        KeyCode::PageUp => app.move_selection(-10),
        KeyCode::Char('g') | KeyCode::Home => app.select_edge(false),
        KeyCode::Char('G') | KeyCode::End => app.select_edge(true),
        KeyCode::Char('n') => app.skip(true),
        KeyCode::Char('p') | KeyCode::Char('b') => app.skip(false),
        KeyCode::Left => app.seek_relative(if shift { -30.0 } else { -5.0 }),
        KeyCode::Right => app.seek_relative(if shift { 30.0 } else { 5.0 }),
        KeyCode::Char('-') | KeyCode::Char('_') => app.volume_delta(-5.0),
        KeyCode::Char('+') | KeyCode::Char('=') => app.volume_delta(5.0),
        KeyCode::Char('m') => app.toggle_mute(),
        KeyCode::Char('B') => app.toggle_transparent(),
        KeyCode::Char('0') => app.reset_speed(),
        KeyCode::Char('[') => app.cycle_speed(-0.05),
        KeyCode::Char(']') => app.cycle_speed(0.05),
        KeyCode::Char('L') => app.cycle_mode(),
        KeyCode::Char('S') => app.shuffle_now(),
        KeyCode::Char('f') => app.toggle_favorite(),
        KeyCode::Char('1') => app.set_view("all"),
        KeyCode::Char('2') => app.set_view("fav"),
        KeyCode::Char('3') => app.set_view("recent"),
        KeyCode::Tab => app.cycle_view(),
        KeyCode::Char('/') => app.open_modal(Modal::Search),
        KeyCode::Char('o') => app.open_modal(Modal::Directory),
        KeyCode::Char('c') => app.open_modal(Modal::Settings),
        KeyCode::Char('r') => app.rescan(),
        KeyCode::Char('s') => app.cycle_sort(),
        KeyCode::Char('e') => app.cycle_effect(),
        KeyCode::Char('t') => app.cycle_sleep(),
        KeyCode::Char('a') => app.cycle_ab(),
        KeyCode::Char('l') => app.toggle_lyrics(),
        KeyCode::Char('z') => app.lyrics_offset(-0.5),
        KeyCode::Char('x') => app.lyrics_offset(0.5),
        KeyCode::Char('y') => app.export_queue(),
        KeyCode::Char('u') => app.open_modal(Modal::Queue),
        KeyCode::Char('i') => app.open_modal(Modal::Info),
        KeyCode::Char('?') => app.open_modal(Modal::Help),
        KeyCode::Char(':') => app.open_modal(Modal::Seek),
        KeyCode::Esc if !app.filter.is_empty() => {
            app.filter.clear();
            app.rebuild_visible();
            app.notify("已清除搜索");
        }
        _ => {}
    }
}

fn handle_modal(app: &mut App, key: KeyEvent) {
    let Some(modal) = app.modal else {
        return;
    };
    if modal == Modal::Settings {
        match key.code {
            KeyCode::Esc => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => app.move_setting(-1),
            KeyCode::Down | KeyCode::Char('j') => app.move_setting(1),
            KeyCode::Left | KeyCode::Char('h') => {
                let index = app.setting_selected;
                app.adjust_setting(index, -1);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let index = app.setting_selected;
                app.adjust_setting(index, 1);
            }
            KeyCode::Enter => {
                let index = app.setting_selected;
                app.activate_setting(index);
            }
            KeyCode::Home => app.setting_selected = 0,
            KeyCode::End => app.setting_selected = app.settings_rows().len().saturating_sub(1),
            _ => {}
        }
        return;
    }
    let editing = matches!(modal, Modal::Search | Modal::Directory | Modal::Seek);
    if editing {
        match key.code {
            KeyCode::Esc => app.cancel_modal(),
            KeyCode::Enter => app.submit_modal(),
            KeyCode::Backspace => {
                app.input.pop();
                if modal == Modal::Search {
                    app.sync_search();
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.input.clear();
                if modal == Modal::Search {
                    app.sync_search();
                }
            }
            KeyCode::Char(ch)
                if !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                app.input.push(ch);
                if modal == Modal::Search {
                    app.sync_search();
                }
            }
            _ => {}
        }
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
        KeyCode::Char('?') if modal != Modal::Help => {
            app.close_modal();
            app.open_modal(Modal::Help);
        }
        KeyCode::Char('j') | KeyCode::Down => app.modal_scroll += 1,
        KeyCode::Char('k') | KeyCode::Up => app.modal_scroll = app.modal_scroll.saturating_sub(1),
        KeyCode::PageDown => app.modal_scroll += 10,
        KeyCode::PageUp => app.modal_scroll = app.modal_scroll.saturating_sub(10),
        _ => {}
    }
}
