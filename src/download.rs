use crate::config;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver, Sender};

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub uploader: String,
    pub duration: f64,
    pub url: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Format {
    pub id: String,
    pub ext: String,
    pub audio: bool,
    pub video: bool,
    pub abr: f64,
    pub tbr: f64,
    pub height: u64,
    pub size: u64,
    pub size_approx: bool,
    pub note: String,
}

impl Format {
    /// 空 id 表示不传 `-f`，交给 yt-dlp 的 `bestaudio`。
    pub fn auto() -> Self {
        Self {
            id: String::new(),
            ext: "自动".into(),
            audio: true,
            note: "bestaudio".into(),
            ..Self::default()
        }
    }
}

pub enum Event {
    SearchDone(Result<Vec<Candidate>, String>),
    FormatsDone(Result<Vec<Format>, String>),
    Started {
        index: usize,
        total: usize,
        title: String,
    },
    Progress(f32),
    ItemDone {
        title: String,
        ok: bool,
        message: String,
    },
    AllDone {
        ok: usize,
        failed: usize,
    },
}

/// 用户直接给 BV / av / 链接时不用搜索，交给 yt-dlp 自己解析。
pub fn video_url(raw: &str) -> Option<String> {
    let text = raw.trim();
    if text.is_empty() {
        return None;
    }
    if text.starts_with("http://") || text.starts_with("https://") {
        return Some(text.to_string());
    }
    let lower = text.to_ascii_lowercase();
    if lower.starts_with("bv") && text.len() > 2 {
        return Some(format!("https://www.bilibili.com/video/{text}"));
    }
    if lower.starts_with("av") && text[2..].chars().all(|ch| ch.is_ascii_digit()) {
        return Some(format!("https://www.bilibili.com/video/{text}"));
    }
    if text.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(format!("https://www.bilibili.com/video/av{text}"));
    }
    None
}

fn id_label(raw: &str) -> String {
    raw.rsplit('/')
        .next()
        .unwrap_or(raw)
        .split(['?', '#'])
        .next()
        .unwrap_or(raw)
        .to_string()
}

/// 多个 id / 链接一起输入时给用户明确提示。
pub fn has_multiple_ids(raw: &str) -> bool {
    let parts: Vec<&str> = raw
        .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '，' | ';' | '；'))
        .filter(|part| !part.is_empty())
        .collect();
    parts.len() > 1 && parts.iter().all(|part| video_url(part).is_some())
}

/// 只接受一个 BV、av 或链接；关键词和多 id 都返回 None。
pub fn direct_candidates(raw: &str) -> Option<Vec<Candidate>> {
    let text = raw.trim();
    if text.is_empty()
        || text
            .split(|ch: char| ch.is_whitespace() || matches!(ch, ',' | '，' | ';' | '；'))
            .filter(|part| !part.is_empty())
            .count()
            != 1
    {
        return None;
    }
    let url = video_url(text)?;
    let label = {
        let label = id_label(text);
        if label.is_empty() {
            text.to_string()
        } else {
            label
        }
    };
    Some(vec![Candidate {
        id: label.clone(),
        title: label,
        uploader: String::new(),
        duration: 0.0,
        url,
    }])
}

/// cookies 文件不存在就不传参数，避免 yt-dlp 直接报错。
pub fn cookie_file(raw: &str) -> Option<PathBuf> {
    let path = config::expand_path(raw.trim());
    (!raw.trim().is_empty() && path.is_file()).then_some(path)
}

pub fn parse_progress(line: &str) -> Option<f32> {
    let line = line.trim();
    if !line.starts_with("[download]") {
        return None;
    }
    let percent = line
        .split_whitespace()
        .find(|part| part.ends_with('%'))?
        .trim_end_matches('%')
        .parse::<f32>()
        .ok()?;
    Some(percent.clamp(0.0, 100.0))
}

pub fn parse_search_json(text: &str) -> Result<Vec<Candidate>, String> {
    let root: Value =
        serde_json::from_str(text).map_err(|err| format!("解析搜索结果失败：{err}"))?;
    let entries = match root.get("entries").and_then(Value::as_array) {
        Some(entries) => entries.clone(),
        None => vec![root],
    };
    let mut out = Vec::new();
    for entry in entries {
        let id = entry
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let title = entry
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string();
        if id.is_empty() || title.is_empty() {
            continue;
        }
        let uploader = entry
            .get("uploader")
            .or_else(|| entry.get("channel"))
            .or_else(|| entry.get("uploader_id"))
            .and_then(Value::as_str)
            .unwrap_or("未知 UP 主")
            .to_string();
        let duration = entry.get("duration").and_then(Value::as_f64).unwrap_or(0.0);
        let url = entry
            .get("webpage_url")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("https://www.bilibili.com/video/{id}"));
        out.push(Candidate {
            id,
            title,
            uploader,
            duration,
            url,
        });
    }
    if out.is_empty() {
        return Err("没有搜到结果喵，换个关键词或检查 cookies".into());
    }
    Ok(out)
}

pub fn parse_formats_json(text: &str) -> Result<Vec<Format>, String> {
    let root: Value =
        serde_json::from_str(text).map_err(|err| format!("解析格式列表失败：{err}"))?;
    let formats = root
        .get("formats")
        .and_then(Value::as_array)
        .ok_or_else(|| "这个视频没有可用格式喵".to_string())?;
    let mut out = Vec::new();
    for format in formats {
        let id = format
            .get("format_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if id.is_empty() {
            continue;
        }
        let vcodec = format
            .get("vcodec")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let acodec = format
            .get("acodec")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let video = !vcodec.is_empty() && vcodec != "none";
        let audio = !acodec.is_empty() && acodec != "none";
        if !audio {
            continue;
        }
        let ext = format
            .get("ext")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        if ext.eq_ignore_ascii_case("mp4") {
            continue;
        }
        let size = format.get("filesize").and_then(Value::as_u64).unwrap_or(0);
        let size_approx = size == 0;
        let size = if size == 0 {
            format
                .get("filesize_approx")
                .and_then(Value::as_u64)
                .unwrap_or(0)
        } else {
            size
        };
        out.push(Format {
            id,
            ext,
            audio,
            video,
            abr: format.get("abr").and_then(Value::as_f64).unwrap_or(0.0),
            tbr: format.get("tbr").and_then(Value::as_f64).unwrap_or(0.0),
            height: format.get("height").and_then(Value::as_u64).unwrap_or(0),
            size,
            size_approx,
            note: format
                .get("format_note")
                .or_else(|| format.get("resolution"))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string(),
        });
    }
    if out.is_empty() {
        return Err("没有可下载的格式喵".into());
    }
    // 纯音频排前面，音质高的优先；视频按分辨率从高到低。
    out.sort_by(|a, b| {
        let rank = |format: &Format| if format.video { 1 } else { 0 };
        rank(a).cmp(&rank(b)).then_with(|| {
            let a_quality = if a.video { a.height as f64 } else { a.abr };
            let b_quality = if b.video { b.height as f64 } else { b.abr };
            b_quality
                .partial_cmp(&a_quality)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    });
    Ok(out)
}

fn command(ytdlp: &str) -> Command {
    Command::new(if ytdlp.trim().is_empty() {
        "yt-dlp"
    } else {
        ytdlp.trim()
    })
}

fn add_cookies(command: &mut Command, cookies: Option<&Path>) {
    if let Some(path) = cookies {
        command.arg("--cookies").arg(path);
    }
}

/// 后台跑 `bilisearch`，返回候选列表。
pub fn spawn_search(
    ytdlp: String,
    cookies: Option<PathBuf>,
    limit: usize,
    query: String,
) -> Receiver<Event> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let result = search(&ytdlp, cookies.as_deref(), limit, &query);
        let _ = sender.send(Event::SearchDone(result));
    });
    receiver
}

fn search(
    ytdlp: &str,
    cookies: Option<&Path>,
    limit: usize,
    query: &str,
) -> Result<Vec<Candidate>, String> {
    let target = format!("bilisearch{}:{}", limit.clamp(1, 50), query.trim());
    let mut cmd = command(ytdlp);
    cmd.arg("--flat-playlist")
        .arg("-J")
        .arg("--no-warnings")
        .arg("--socket-timeout")
        .arg("15")
        .arg("--retries")
        .arg("2");
    add_cookies(&mut cmd, cookies);
    cmd.arg(&target);
    let output = cmd
        .output()
        .map_err(|err| format!("启动 yt-dlp 失败：{err}（设置里可以改路径）"))?;
    if !output.status.success() {
        let message = String::from_utf8_lossy(&output.stderr);
        let line = message
            .lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("yt-dlp 搜索失败");
        return Err(format!("搜索失败喵：{line}"));
    }
    parse_search_json(&String::from_utf8_lossy(&output.stdout))
}

/// 先用 `-J` 拿格式列表，等价于 `yt-dlp -F` 的结构化版本。
pub fn spawn_formats(ytdlp: String, cookies: Option<PathBuf>, url: String) -> Receiver<Event> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut cmd = command(&ytdlp);
        cmd.arg("--no-warnings")
            .arg("--no-playlist")
            .arg("-J")
            .arg("--socket-timeout")
            .arg("15")
            .arg("--retries")
            .arg("2");
        add_cookies(&mut cmd, cookies.as_deref());
        cmd.arg(&url);
        let result = cmd
            .output()
            .map_err(|err| format!("启动 yt-dlp 失败：{err}"));
        let result = result.and_then(|output| {
            if output.status.success() {
                parse_formats_json(&String::from_utf8_lossy(&output.stdout))
            } else {
                let message = String::from_utf8_lossy(&output.stderr);
                let line = message
                    .lines()
                    .rev()
                    .find(|line| !line.trim().is_empty())
                    .unwrap_or("yt-dlp 读取格式失败");
                Err(format!("格式列表失败喵：{line}"))
            }
        });
        let _ = sender.send(Event::FormatsDone(result));
    });
    receiver
}

/// 串行下载多个候选，每个文件的进度通过 channel 回传。
pub fn spawn_download(
    ytdlp: String,
    cookies: Option<PathBuf>,
    dir: PathBuf,
    items: Vec<Candidate>,
    format: Option<String>,
) -> Receiver<Event> {
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let total = items.len();
        let mut ok = 0usize;
        let mut failed = 0usize;
        for (index, item) in items.into_iter().enumerate() {
            let _ = sender.send(Event::Started {
                index,
                total,
                title: item.title.clone(),
            });
            let mut result = download_one(
                &ytdlp,
                cookies.as_deref(),
                &dir,
                &item,
                format.as_deref(),
                &sender,
            );
            if let Err(err) = &result
                && format.is_some()
                && (err.contains("Requested format is not available")
                    || err.contains("format is not available"))
            {
                result = download_one(&ytdlp, cookies.as_deref(), &dir, &item, None, &sender);
            }
            match result {
                Ok(()) => {
                    ok += 1;
                    let _ = sender.send(Event::ItemDone {
                        title: item.title,
                        ok: true,
                        message: "完成".into(),
                    });
                }
                Err(message) => {
                    failed += 1;
                    let _ = sender.send(Event::ItemDone {
                        title: item.title,
                        ok: false,
                        message,
                    });
                }
            }
        }
        let _ = sender.send(Event::AllDone { ok, failed });
    });
    receiver
}

fn download_one(
    ytdlp: &str,
    cookies: Option<&Path>,
    dir: &Path,
    item: &Candidate,
    format: Option<&str>,
    sender: &Sender<Event>,
) -> Result<(), String> {
    let mut cmd = command(ytdlp);
    cmd.arg("--no-playlist")
        .arg("--newline")
        .arg("--no-warnings")
        .arg("--socket-timeout")
        .arg("15")
        .arg("--retries")
        .arg("3")
        .arg("-f")
        .arg(format.unwrap_or("bestaudio"))
        .arg("-P")
        .arg(dir)
        .arg("-o")
        .arg("%(title)s [%(id)s].%(ext)s");
    add_cookies(&mut cmd, cookies);
    cmd.arg(&item.url);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|err| format!("启动 yt-dlp 失败：{err}"))?;
    let stderr = child.stderr.take();
    let errors = stderr.map(|stderr| {
        std::thread::spawn(move || {
            let mut last = String::new();
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                if !line.trim().is_empty() {
                    last = line;
                }
            }
            last
        })
    });
    if let Some(stdout) = child.stdout.take() {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(percent) = parse_progress(&line) {
                let _ = sender.send(Event::Progress(percent));
            }
        }
    }
    let status = child
        .wait()
        .map_err(|err| format!("等待 yt-dlp 失败：{err}"))?;
    let last_error = errors
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();
    if status.success() {
        Ok(())
    } else if last_error.is_empty() {
        Err(format!("yt-dlp 退出：{status}"))
    } else {
        Err(last_error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_ids_and_urls() {
        assert_eq!(
            video_url("BV1xx411c7mD").as_deref(),
            Some("https://www.bilibili.com/video/BV1xx411c7mD")
        );
        assert_eq!(
            video_url("av170001").as_deref(),
            Some("https://www.bilibili.com/video/av170001")
        );
        assert_eq!(
            video_url("170001").as_deref(),
            Some("https://www.bilibili.com/video/av170001")
        );
        assert_eq!(
            video_url("https://www.bilibili.com/video/BV1xx411c7mD").as_deref(),
            Some("https://www.bilibili.com/video/BV1xx411c7mD")
        );
        assert_eq!(video_url("周杰伦 晴天"), None);
    }

    #[test]
    fn accepts_only_one_id() {
        let list = direct_candidates("BV1xx411c7mD").unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "BV1xx411c7mD");
        assert!(direct_candidates("BV1xx411c7mD,BV1ab411c7mD").is_none());
        assert!(direct_candidates("BV1xx411c7mD BV1ab411c7mD").is_none());
        assert!(direct_candidates("周杰伦 晴天").is_none());
        assert!(has_multiple_ids("BV1xx411c7mD,BV1ab411c7mD"));
        assert!(!has_multiple_ids("周杰伦 晴天"));
    }

    #[test]
    fn parses_progress_and_search_entries() {
        assert_eq!(parse_progress("[download]  42.3% of 10MiB"), Some(42.3));
        assert_eq!(parse_progress("[download] Destination: a.m4a"), None);
        let json = r#"{"entries":[{"id":"BV1","title":"歌","uploader":"UP","duration":61,"webpage_url":"https://x"}]}"#;
        let list = parse_search_json(json).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].title, "歌");
        assert_eq!(list[0].uploader, "UP");
        assert_eq!(list[0].duration, 61.0);
        assert_eq!(list[0].url, "https://x");
        assert!(parse_search_json(r#"{"entries":[]}"#).is_err());
    }

    #[test]
    fn parses_and_sorts_formats() {
        let json = r#"{"formats":[
            {"format_id":"30011","ext":"mp4","vcodec":"avc1","acodec":"none","height":360,"tbr":300},
            {"format_id":"30280","ext":"m4a","vcodec":"none","acodec":"mp4a","abr":128,"filesize_approx":33010000},
            {"format_id":"30216","ext":"m4a","vcodec":"none","acodec":"mp4a","abr":64,"filesize":16820000}
        ]}"#;
        let list = parse_formats_json(json).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, "30280");
        assert_eq!(list[1].id, "30216");
        assert!(list.iter().all(|format| !format.video));
        assert_eq!(list[0].size, 33010000);
        assert!(list[0].size_approx);
        assert_eq!(list[1].size, 16820000);
        assert!(!list[1].size_approx);
        assert!(parse_formats_json(r#"{"formats":[]}"#).is_err());
    }
}
