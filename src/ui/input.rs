use crate::app::{App, Modal};
use crate::keymap::{self, ACTIONS};
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
    if app.lyrics_full && key.code == KeyCode::Esc {
        app.lyrics_full = false;
        app.notify("退出全屏歌词");
        return;
    }
    let Some(name) = keymap::key_name(key) else {
        return;
    };
    if let Some(action) = app.bindings.get(&name).copied() {
        app.run_action(action);
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
    if modal == Modal::Keybindings {
        handle_keybindings(app, key);
        return;
    }
    if modal == Modal::Directory {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => app.dir_move(-1),
            KeyCode::Down | KeyCode::Char('j') => app.dir_move(1),
            KeyCode::Char('a') => app.open_modal(Modal::DirAdd),
            KeyCode::Char('x') | KeyCode::Char('d') => {
                let index = app.dir_selected;
                app.remove_music_dir(index);
            }
            KeyCode::Char('r') | KeyCode::Enter => app.rescan(),
            _ => {}
        }
        return;
    }
    let editing = matches!(
        modal,
        Modal::Search | Modal::DirAdd | Modal::Seek | Modal::PlaylistName
    );
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

    match modal {
        Modal::Playlists => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => app.playlist_move(-1),
            KeyCode::Down | KeyCode::Char('j') => app.playlist_move(1),
            KeyCode::Enter => app.open_playlist_tracks(),
            KeyCode::Char('n') => app.begin_playlist_create(),
            KeyCode::Char('r') => app.begin_playlist_rename(),
            KeyCode::Char('x') => app.delete_playlist(),
            KeyCode::Char('a') => app.add_to_playlist(),
            _ => {}
        },
        Modal::PlaylistTracks => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => {
                app.open_modal(Modal::Playlists);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                app.playlist_song_selected = app.playlist_song_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let count = app
                    .playlists
                    .get(app.playlist_selected)
                    .map(|list| list.songs.len())
                    .unwrap_or(0);
                app.playlist_song_selected =
                    (app.playlist_song_selected + 1).min(count.saturating_sub(1));
            }
            KeyCode::Enter => app.play_playlist(app.playlist_song_selected),
            KeyCode::Char('a') => app.add_to_playlist(),
            KeyCode::Char('d') => app.remove_playlist_song(),
            _ => {}
        },
        Modal::Folders => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => app.folder_move(-1),
            KeyCode::Down | KeyCode::Char('j') => app.folder_move(1),
            KeyCode::Enter => app.select_folder(),
            _ => {}
        },
        Modal::Equalizer => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => {
                app.eq_selected = app.eq_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.eq_selected = (app.eq_selected + 1).min(crate::app::EQ_BANDS.len());
            }
            KeyCode::Left | KeyCode::Char('h') => {
                let row = app.eq_selected;
                app.adjust_eq(row, -1.0);
            }
            KeyCode::Right | KeyCode::Char('l') => {
                let row = app.eq_selected;
                app.adjust_eq(row, 1.0);
            }
            KeyCode::Enter => {
                app.activate_eq_row();
            }
            KeyCode::Char('r') => app.reset_eq(),
            _ => {}
        },
        Modal::AudioDevice => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => app.device_move(-1),
            KeyCode::Down | KeyCode::Char('j') => app.device_move(1),
            KeyCode::Enter => app.apply_device(),
            KeyCode::Char('r') => app.open_modal(Modal::AudioDevice),
            _ => {}
        },
        Modal::Stats => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => {
                app.stats_scroll = app.stats_scroll.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => app.stats_scroll += 1,
            KeyCode::Char('x') => app.clear_stats(),
            _ => {}
        },
        Modal::Queue => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Up | KeyCode::Char('k') => {
                app.queue_selected = app.queue_selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                app.queue_selected =
                    (app.queue_selected + 1).min(app.session.len().saturating_sub(1));
            }
            KeyCode::Char('K') => app.queue_move(-1),
            KeyCode::Char('J') => app.queue_move(1),
            KeyCode::Char('d') => app.queue_remove(),
            KeyCode::Char('x') => app.queue_clear(),
            KeyCode::Enter => app.play_queue_index(app.queue_selected),
            _ => {}
        },
        _ => match key.code {
            KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
            KeyCode::Char('?') if modal != Modal::Help => {
                app.close_modal();
                app.open_modal(Modal::Help);
            }
            KeyCode::Char('j') | KeyCode::Down => app.modal_scroll += 1,
            KeyCode::Char('k') | KeyCode::Up => {
                app.modal_scroll = app.modal_scroll.saturating_sub(1);
            }
            KeyCode::PageDown => app.modal_scroll += 10,
            KeyCode::PageUp => app.modal_scroll = app.modal_scroll.saturating_sub(10),
            _ => {}
        },
    }
}

fn handle_keybindings(app: &mut App, key: KeyEvent) {
    if app.capture_key {
        if key.code == KeyCode::Esc {
            app.capture_key = false;
            app.notify("已取消改键");
            return;
        }
        let Some(name) = keymap::key_name(key) else {
            return;
        };
        let action = ACTIONS[app.keybind_selected.min(ACTIONS.len() - 1)];
        app.set_binding(action, name);
        app.capture_key = false;
        return;
    }
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => app.close_modal(),
        KeyCode::Up | KeyCode::Char('k') => {
            app.keybind_selected = app.keybind_selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            app.keybind_selected = (app.keybind_selected + 1).min(ACTIONS.len() - 1);
        }
        KeyCode::PageUp => app.keybind_selected = app.keybind_selected.saturating_sub(10),
        KeyCode::PageDown => {
            app.keybind_selected = (app.keybind_selected + 10).min(ACTIONS.len() - 1);
        }
        KeyCode::Char('r') => {
            let action = ACTIONS[app.keybind_selected];
            app.reset_binding(action);
        }
        KeyCode::Enter => {
            app.begin_capture_binding();
        }
        _ => {}
    }
}
