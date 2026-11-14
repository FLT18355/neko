use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    PlayPause,
    PlaySelected,
    MoveDown,
    MoveUp,
    PageDown,
    PageUp,
    JumpFirst,
    JumpLast,
    NextTrack,
    PrevTrack,
    SeekBack5,
    SeekForward5,
    SeekBack30,
    SeekForward30,
    VolumeDown,
    VolumeUp,
    Mute,
    Transparent,
    SpeedDown,
    SpeedUp,
    SpeedReset,
    Mode,
    Shuffle,
    ABLoop,
    Favorite,
    ViewAll,
    ViewFav,
    ViewRecent,
    ViewFolder,
    CycleView,
    Search,
    Directory,
    Settings,
    Rescan,
    Sort,
    Effect,
    Equalizer,
    Sleep,
    Lyrics,
    FullLyrics,
    LyricsEarlier,
    LyricsLater,
    Export,
    SeekInput,
    Queue,
    Info,
    Help,
    Rating,
    Stats,
    Playlists,
    AudioDevice,
    Theme,
    Keybindings,
    ClearFilter,
}

pub const ACTIONS: [Action; 55] = [
    Action::Quit,
    Action::PlayPause,
    Action::PlaySelected,
    Action::MoveDown,
    Action::MoveUp,
    Action::PageDown,
    Action::PageUp,
    Action::JumpFirst,
    Action::JumpLast,
    Action::NextTrack,
    Action::PrevTrack,
    Action::SeekBack5,
    Action::SeekForward5,
    Action::SeekBack30,
    Action::SeekForward30,
    Action::VolumeDown,
    Action::VolumeUp,
    Action::Mute,
    Action::Transparent,
    Action::SpeedDown,
    Action::SpeedUp,
    Action::SpeedReset,
    Action::Mode,
    Action::Shuffle,
    Action::ABLoop,
    Action::Favorite,
    Action::ViewAll,
    Action::ViewFav,
    Action::ViewRecent,
    Action::ViewFolder,
    Action::CycleView,
    Action::Search,
    Action::Directory,
    Action::Settings,
    Action::Rescan,
    Action::Sort,
    Action::Effect,
    Action::Equalizer,
    Action::Sleep,
    Action::Lyrics,
    Action::FullLyrics,
    Action::LyricsEarlier,
    Action::LyricsLater,
    Action::Export,
    Action::SeekInput,
    Action::Queue,
    Action::Info,
    Action::Help,
    Action::Rating,
    Action::Stats,
    Action::Playlists,
    Action::AudioDevice,
    Action::Theme,
    Action::Keybindings,
    Action::ClearFilter,
];

pub const DEFAULT_BINDINGS: &[(Action, &str)] = &[
    (Action::Quit, "q"),
    (Action::PlayPause, "space"),
    (Action::PlaySelected, "enter"),
    (Action::MoveDown, "down"),
    (Action::MoveDown, "j"),
    (Action::MoveUp, "up"),
    (Action::MoveUp, "k"),
    (Action::PageDown, "pagedown"),
    (Action::PageUp, "pageup"),
    (Action::JumpFirst, "home"),
    (Action::JumpFirst, "g"),
    (Action::JumpLast, "end"),
    (Action::JumpLast, "shift+g"),
    (Action::NextTrack, "n"),
    (Action::PrevTrack, "p"),
    (Action::PrevTrack, "b"),
    (Action::SeekBack5, "left"),
    (Action::SeekForward5, "right"),
    (Action::SeekBack30, "shift+left"),
    (Action::SeekForward30, "shift+right"),
    (Action::VolumeDown, "-"),
    (Action::VolumeUp, "="),
    (Action::Mute, "m"),
    (Action::Transparent, "shift+b"),
    (Action::SpeedDown, "["),
    (Action::SpeedUp, "]"),
    (Action::SpeedReset, "0"),
    (Action::Mode, "shift+l"),
    (Action::Shuffle, "shift+s"),
    (Action::ABLoop, "a"),
    (Action::Favorite, "f"),
    (Action::ViewAll, "1"),
    (Action::ViewFav, "2"),
    (Action::ViewRecent, "3"),
    (Action::ViewFolder, "4"),
    (Action::CycleView, "tab"),
    (Action::Search, "/"),
    (Action::Directory, "o"),
    (Action::Settings, "c"),
    (Action::Rescan, "r"),
    (Action::Sort, "s"),
    (Action::Effect, "e"),
    (Action::Equalizer, "shift+e"),
    (Action::Sleep, "t"),
    (Action::Lyrics, "l"),
    (Action::FullLyrics, "shift+f"),
    (Action::LyricsEarlier, "z"),
    (Action::LyricsLater, "x"),
    (Action::Export, "y"),
    (Action::SeekInput, ":"),
    (Action::Queue, "u"),
    (Action::Info, "i"),
    (Action::Help, "?"),
    (Action::Rating, "shift+r"),
    (Action::Stats, "v"),
    (Action::Playlists, "shift+p"),
    (Action::AudioDevice, "shift+d"),
    (Action::Theme, "shift+t"),
    (Action::Keybindings, "shift+k"),
    (Action::ClearFilter, "esc"),
];

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Action::Quit => "quit",
            Action::PlayPause => "play_pause",
            Action::PlaySelected => "play_selected",
            Action::MoveDown => "move_down",
            Action::MoveUp => "move_up",
            Action::PageDown => "page_down",
            Action::PageUp => "page_up",
            Action::JumpFirst => "jump_first",
            Action::JumpLast => "jump_last",
            Action::NextTrack => "next_track",
            Action::PrevTrack => "prev_track",
            Action::SeekBack5 => "seek_back_5",
            Action::SeekForward5 => "seek_forward_5",
            Action::SeekBack30 => "seek_back_30",
            Action::SeekForward30 => "seek_forward_30",
            Action::VolumeDown => "volume_down",
            Action::VolumeUp => "volume_up",
            Action::Mute => "mute",
            Action::Transparent => "transparent",
            Action::SpeedDown => "speed_down",
            Action::SpeedUp => "speed_up",
            Action::SpeedReset => "speed_reset",
            Action::Mode => "mode",
            Action::Shuffle => "shuffle",
            Action::ABLoop => "ab_loop",
            Action::Favorite => "favorite",
            Action::ViewAll => "view_all",
            Action::ViewFav => "view_fav",
            Action::ViewRecent => "view_recent",
            Action::ViewFolder => "view_folder",
            Action::CycleView => "cycle_view",
            Action::Search => "search",
            Action::Directory => "directory",
            Action::Settings => "settings",
            Action::Rescan => "rescan",
            Action::Sort => "sort",
            Action::Effect => "effect",
            Action::Equalizer => "equalizer",
            Action::Sleep => "sleep",
            Action::Lyrics => "lyrics",
            Action::FullLyrics => "full_lyrics",
            Action::LyricsEarlier => "lyrics_earlier",
            Action::LyricsLater => "lyrics_later",
            Action::Export => "export",
            Action::SeekInput => "seek_input",
            Action::Queue => "queue",
            Action::Info => "info",
            Action::Help => "help",
            Action::Rating => "rating",
            Action::Stats => "stats",
            Action::Playlists => "playlists",
            Action::AudioDevice => "audio_device",
            Action::Theme => "theme",
            Action::Keybindings => "keybindings",
            Action::ClearFilter => "clear_filter",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Quit => "退出 neko",
            Action::PlayPause => "播放 / 暂停",
            Action::PlaySelected => "播放选中曲目",
            Action::MoveDown => "列表下移",
            Action::MoveUp => "列表上移",
            Action::PageDown => "下翻十行",
            Action::PageUp => "上翻十行",
            Action::JumpFirst => "跳到列表头",
            Action::JumpLast => "跳到列表尾",
            Action::NextTrack => "下一首",
            Action::PrevTrack => "上一首",
            Action::SeekBack5 => "快退 5 秒",
            Action::SeekForward5 => "快进 5 秒",
            Action::SeekBack30 => "快退 30 秒",
            Action::SeekForward30 => "快进 30 秒",
            Action::VolumeDown => "音量 -5%",
            Action::VolumeUp => "音量 +5%",
            Action::Mute => "静音开关",
            Action::Transparent => "透明背景",
            Action::SpeedDown => "速度 -0.05x",
            Action::SpeedUp => "速度 +0.05x",
            Action::SpeedReset => "速度恢复 1.00x",
            Action::Mode => "切换播放模式",
            Action::Shuffle => "打乱队列",
            Action::ABLoop => "A-B 循环",
            Action::Favorite => "收藏 / 取消收藏",
            Action::ViewAll => "全部歌曲视图",
            Action::ViewFav => "收藏视图",
            Action::ViewRecent => "最近播放视图",
            Action::ViewFolder => "文件夹视图",
            Action::CycleView => "循环切换视图",
            Action::Search => "搜索曲目",
            Action::Directory => "音乐目录列表",
            Action::Settings => "打开设置",
            Action::Rescan => "重新扫描目录",
            Action::Sort => "切换排序",
            Action::Effect => "切换音效预设",
            Action::Equalizer => "自定义均衡器",
            Action::Sleep => "睡眠定时",
            Action::Lyrics => "歌词开关",
            Action::FullLyrics => "全屏歌词",
            Action::LyricsEarlier => "歌词提前 0.5 秒",
            Action::LyricsLater => "歌词延后 0.5 秒",
            Action::Export => "导出队列 M3U",
            Action::SeekInput => "输入时间跳转",
            Action::Queue => "播放队列",
            Action::Info => "曲目详情",
            Action::Help => "快捷键帮助",
            Action::Rating => "星级评分",
            Action::Stats => "收听统计",
            Action::Playlists => "自建歌单",
            Action::AudioDevice => "音频输出设备",
            Action::Theme => "切换主题",
            Action::Keybindings => "自定义快捷键",
            Action::ClearFilter => "清除搜索筛选",
        }
    }
}

pub fn default_keys(action: Action) -> Vec<&'static str> {
    DEFAULT_BINDINGS
        .iter()
        .filter(|(item, _)| *item == action)
        .map(|(_, key)| *key)
        .collect()
}

/// SHIFT+符号按 US 布局换算，兼容只上报基础键码的终端。
fn shifted_symbol(ch: char, modifiers: KeyModifiers) -> char {
    if !modifiers.contains(KeyModifiers::SHIFT) {
        return ch;
    }
    match ch {
        '`' => '~',
        '1' => '!',
        '2' => '@',
        '3' => '#',
        '4' => '$',
        '5' => '%',
        '6' => '^',
        '7' => '&',
        '8' => '*',
        '9' => '(',
        '0' => ')',
        '-' => '_',
        '=' => '+',
        '[' => '{',
        ']' => '}',
        '\\' => '|',
        ';' => ':',
        '\'' => '"',
        ',' => '<',
        '.' => '>',
        '/' => '?',
        _ => ch,
    }
}

pub fn key_name(key: KeyEvent) -> Option<String> {
    let mut mods = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        mods.push_str("ctrl+");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        mods.push_str("alt+");
    }
    let base = match key.code {
        KeyCode::Char(' ') => "space".to_string(),
        KeyCode::Char(ch) if ch.is_alphabetic() => {
            // 部分终端（kitty 键盘协议）上报小写字母 + SHIFT，不能只看大小写。
            if ch.is_uppercase() || key.modifiers.contains(KeyModifiers::SHIFT) {
                mods.push_str("shift+");
            }
            ch.to_lowercase().to_string()
        }
        KeyCode::Char(ch) => shifted_symbol(ch, key.modifiers).to_string(),
        KeyCode::Enter => "enter".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => {
            if !mods.contains("shift+") {
                mods.push_str("shift+");
            }
            "tab".into()
        }
        KeyCode::Backspace => "backspace".into(),
        KeyCode::Delete => "delete".into(),
        KeyCode::Insert => "insert".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        KeyCode::Home => "home".into(),
        KeyCode::End => "end".into(),
        KeyCode::PageUp => "pageup".into(),
        KeyCode::PageDown => "pagedown".into(),
        KeyCode::F(number) => format!("f{number}"),
        _ => return None,
    };
    if !matches!(key.code, KeyCode::Char(_))
        && key.modifiers.contains(KeyModifiers::SHIFT)
        && !mods.contains("shift+")
    {
        mods.push_str("shift+");
    }
    Some(format!("{mods}{base}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn name(code: KeyCode, modifiers: KeyModifiers) -> String {
        key_name(KeyEvent::new(code, modifiers)).unwrap()
    }

    #[test]
    fn names_special_and_shifted_keys() {
        assert_eq!(name(KeyCode::Char(' '), KeyModifiers::NONE), "space");
        assert_eq!(name(KeyCode::Char('G'), KeyModifiers::SHIFT), "shift+g");
        assert_eq!(name(KeyCode::Char('F'), KeyModifiers::NONE), "shift+f");
        assert_eq!(name(KeyCode::Char('l'), KeyModifiers::SHIFT), "shift+l");
        assert_eq!(name(KeyCode::Char('?'), KeyModifiers::SHIFT), "?");
        assert_eq!(name(KeyCode::Char('/'), KeyModifiers::SHIFT), "?");
        assert_eq!(name(KeyCode::Char('1'), KeyModifiers::SHIFT), "!");
        assert_eq!(name(KeyCode::Left, KeyModifiers::SHIFT), "shift+left");
        assert_eq!(name(KeyCode::PageDown, KeyModifiers::NONE), "pagedown");
    }

    #[test]
    fn default_bindings_are_unique_and_cover_every_action() {
        for action in ACTIONS {
            assert!(
                DEFAULT_BINDINGS.iter().any(|(item, _)| *item == action),
                "{} has no default key",
                action.id()
            );
        }
        let mut keys: Vec<&str> = DEFAULT_BINDINGS.iter().map(|(_, key)| *key).collect();
        keys.sort_unstable();
        let before = keys.len();
        keys.dedup();
        assert_eq!(before, keys.len(), "duplicate default binding");
    }
}
