use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
#[cfg(unix)]
use std::os::unix::net::UnixStream;
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

#[cfg(unix)]
type IpcStream = UnixStream;
#[cfg(windows)]
type IpcStream = std::fs::File;

fn connect_ipc(path: &Path) -> io::Result<IpcStream> {
    #[cfg(unix)]
    {
        UnixStream::connect(path)
    }
    #[cfg(windows)]
    {
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0x1 | 0x2 | 0x4)
            .open(path)
    }
}

#[derive(Clone, Debug, Default)]
pub struct AudioInfo {
    pub samplerate: u64,
    pub channels: u64,
    pub format: String,
}

#[derive(Clone, Debug)]
pub struct State {
    pub pause: bool,
    pub time_pos: Option<f64>,
    pub duration: Option<f64>,
    pub volume: f64,
    pub mute: bool,
    pub speed: f64,
    pub idle: bool,
    pub title: Option<String>,
    pub path: Option<PathBuf>,
    pub playlist_pos: Option<i64>,
    pub playlist_count: Option<i64>,
    pub metadata: HashMap<String, String>,
    pub bitrate: Option<f64>,
    pub audio: Option<AudioInfo>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            pause: false,
            time_pos: None,
            duration: None,
            volume: 70.0,
            mute: false,
            speed: 1.0,
            idle: true,
            title: None,
            path: None,
            playlist_pos: None,
            playlist_count: None,
            metadata: HashMap::new(),
            bitrate: None,
            audio: None,
        }
    }
}

pub struct Mpv {
    child: Child,
    writer: IpcStream,
    reader: BufReader<IpcStream>,
    socket: PathBuf,
    next_id: i64,
    pub state: State,
}

fn take<T: DeserializeOwned>(values: &mut HashMap<String, Value>, key: &str) -> Option<T> {
    values
        .remove(key)
        .and_then(|value| serde_json::from_value(value).ok())
}

fn parse_audio_devices(value: &Value) -> Vec<(String, String)> {
    let Some(items) = value.as_array() else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let description = item
                .get("description")
                .and_then(Value::as_str)
                .unwrap_or(&name)
                .to_string();
            Some((name, description))
        })
        .collect()
}

impl Mpv {
    pub fn spawn(volume: f64) -> io::Result<Self> {
        #[cfg(unix)]
        let socket =
            crate::config::runtime_dir().join(format!("neko-mpv-{}.sock", std::process::id()));
        #[cfg(windows)]
        let socket = PathBuf::from(format!(r"\\.\pipe\neko-mpv-{}", std::process::id()));
        let _ = fs::remove_file(&socket);

        let mut child = Command::new("mpv")
            .arg("--no-config")
            .arg("--idle=yes")
            .arg("--no-video")
            .arg("--force-window=no")
            .arg("--audio-display=no")
            .arg("--no-terminal")
            .arg("--gapless-audio=yes")
            .arg("--volume-max=130")
            .arg(format!("--volume={volume}"))
            .arg(format!("--input-ipc-server={}", socket.display()))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;

        let mut stream = None;
        for _ in 0..100 {
            if let Ok(s) = connect_ipc(&socket) {
                stream = Some(s);
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                let _ = fs::remove_file(&socket);
                return Err(io::Error::other(format!("mpv 启动后立即退出（{status}）")));
            }
            std::thread::sleep(Duration::from_millis(30));
        }
        let Some(stream) = stream else {
            let _ = child.kill();
            let _ = fs::remove_file(&socket);
            return Err(io::Error::other("等待 mpv IPC 超时"));
        };
        #[cfg(unix)]
        {
            stream.set_read_timeout(Some(Duration::from_millis(500)))?;
            stream.set_write_timeout(Some(Duration::from_millis(500)))?;
        }
        let writer = stream.try_clone()?;
        let reader = BufReader::new(stream);

        Ok(Self {
            child,
            writer,
            reader,
            socket,
            next_id: 0,
            state: State {
                volume,
                ..State::default()
            },
        })
    }

    fn send(&mut self, command: Vec<Value>) -> Result<Option<Value>, String> {
        self.next_id += 1;
        let id = self.next_id;
        let message = json!({ "command": command, "request_id": id }).to_string() + "\n";
        self.writer
            .write_all(message.as_bytes())
            .map_err(|e| format!("写入 mpv 失败：{e}"))?;

        loop {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) => return Err("mpv 已退出".into()),
                Ok(_) => {}
                Err(e) => return Err(format!("读取 mpv 失败：{e}")),
            }
            let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            if value.get("request_id").and_then(Value::as_i64) != Some(id) {
                continue;
            }
            let error = value
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("未知错误");
            if error == "success" {
                return Ok(value.get("data").cloned());
            }
            return Err(error.to_string());
        }
    }

    pub fn refresh(&mut self) {
        const PROPS: [&str; 14] = [
            "pause",
            "time-pos",
            "duration",
            "volume",
            "mute",
            "speed",
            "idle-active",
            "media-title",
            "path",
            "playlist-pos",
            "playlist-count",
            "metadata",
            "audio-bitrate",
            "audio-params",
        ];
        let start_id = self.next_id;
        let mut sent = Vec::new();
        for (offset, prop) in PROPS.iter().enumerate() {
            let id = start_id + offset as i64 + 1;
            let message =
                json!({ "command": ["get_property", prop], "request_id": id }).to_string() + "\n";
            if self.writer.write_all(message.as_bytes()).is_err() {
                return;
            }
            sent.push((id, *prop));
        }
        self.next_id = start_id + PROPS.len() as i64;

        let mut values = HashMap::new();
        let deadline = Instant::now() + Duration::from_millis(350);
        while values.len() < PROPS.len() && Instant::now() < deadline {
            let mut line = String::new();
            match self.reader.read_line(&mut line) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let Ok(value) = serde_json::from_str::<Value>(line.trim()) else {
                continue;
            };
            let Some(id) = value.get("request_id").and_then(Value::as_i64) else {
                continue;
            };
            for (wanted, prop) in &sent {
                if *wanted == id {
                    values.insert(
                        prop.to_string(),
                        value.get("data").cloned().unwrap_or(Value::Null),
                    );
                }
            }
        }

        if let Some(v) = take::<bool>(&mut values, "pause") {
            self.state.pause = v;
        }
        if let Some(v) = take::<Option<f64>>(&mut values, "time-pos") {
            self.state.time_pos = v;
        }
        if let Some(v) = take::<Option<f64>>(&mut values, "duration") {
            self.state.duration = v;
        }
        if let Some(v) = take::<f64>(&mut values, "volume") {
            self.state.volume = v;
        }
        if let Some(v) = take::<bool>(&mut values, "mute") {
            self.state.mute = v;
        }
        if let Some(v) = take::<f64>(&mut values, "speed") {
            self.state.speed = v;
        }
        if let Some(v) = take::<bool>(&mut values, "idle-active") {
            self.state.idle = v;
        }
        if let Some(v) = take::<Option<String>>(&mut values, "media-title") {
            self.state.title = v;
        }
        if let Some(v) = take::<Option<PathBuf>>(&mut values, "path") {
            self.state.path = v;
        }
        if let Some(v) = take::<Option<i64>>(&mut values, "playlist-pos") {
            self.state.playlist_pos = v;
        }
        if let Some(v) = take::<Option<i64>>(&mut values, "playlist-count") {
            self.state.playlist_count = v;
        }
        if let Some(v) = take::<HashMap<String, String>>(&mut values, "metadata") {
            self.state.metadata = v;
        }
        if let Some(v) = take::<Option<f64>>(&mut values, "audio-bitrate") {
            self.state.bitrate = v;
        }
        if let Some(Value::Object(map)) = values.remove("audio-params") {
            let samplerate = map.get("samplerate").and_then(Value::as_u64).unwrap_or(0);
            let channels = map
                .get("channel-count")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let format = map
                .get("format")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string();
            self.state.audio = Some(AudioInfo {
                samplerate,
                channels,
                format,
            });
        }
    }

    pub fn set_prop(&mut self, name: &str, value: Value) -> Result<(), String> {
        self.send(vec![json!("set_property"), json!(name), value])
            .map(|_| ())
    }

    pub fn apply_mode(&mut self, mode: &str) -> Result<(), String> {
        let (file, playlist) = match mode {
            "single" => ("inf", "no"),
            "loop" => ("no", "inf"),
            "shuffle" => ("no", "inf"),
            _ => ("no", "no"),
        };
        self.set_prop("loop-file", json!(file))?;
        self.set_prop("loop-playlist", json!(playlist))
    }

    pub fn load_m3u(
        &mut self,
        list: &Path,
        index: usize,
        mode: &str,
        paused: bool,
    ) -> Result<(), String> {
        if paused {
            self.set_prop("pause", json!(true))?;
        }
        self.send(vec![
            json!("loadlist"),
            json!(list.to_string_lossy()),
            json!("replace"),
        ])?;
        self.set_prop("playlist-pos", json!(index))?;
        self.apply_mode(mode)?;
        self.set_prop("pause", json!(paused))?;
        std::thread::sleep(Duration::from_millis(120));
        self.refresh();
        Ok(())
    }

    pub fn next(&mut self) -> Result<(), String> {
        self.send(vec![json!("playlist-next")]).map(|_| ())
    }

    pub fn prev(&mut self) -> Result<(), String> {
        self.send(vec![json!("playlist-prev")]).map(|_| ())
    }

    pub fn shuffle_playlist(&mut self) -> Result<(), String> {
        self.send(vec![json!("playlist-shuffle")]).map(|_| ())
    }

    pub fn seek(&mut self, seconds: f64, absolute: bool) -> Result<(), String> {
        let mode = if absolute { "absolute" } else { "relative" };
        self.send(vec![json!("seek"), json!(seconds), json!(mode)])
            .map(|_| ())
    }

    pub fn set_af(&mut self, graph: &str) -> Result<(), String> {
        self.set_prop("af", json!(graph))
    }

    pub fn set_ab(&mut self, a: Option<f64>, b: Option<f64>) -> Result<(), String> {
        self.set_prop(
            "ab-loop-a",
            a.map(|v| json!(v)).unwrap_or_else(|| json!("no")),
        )?;
        self.set_prop(
            "ab-loop-b",
            b.map(|v| json!(v)).unwrap_or_else(|| json!("no")),
        )
    }

    pub fn playlist_paths(&mut self) -> Vec<PathBuf> {
        let Ok(Some(Value::Array(items))) =
            self.send(vec![json!("get_property"), json!("playlist")])
        else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| item.get("filename").and_then(Value::as_str))
            .map(PathBuf::from)
            .collect()
    }

    pub fn audio_devices(&mut self) -> Vec<(String, String)> {
        let Ok(Some(value)) = self.send(vec![json!("get_property"), json!("audio-device-list")])
        else {
            return Vec::new();
        };
        parse_audio_devices(&value)
    }

    pub fn set_audio_device(&mut self, name: &str) -> Result<(), String> {
        self.set_prop("audio-device", json!(name))
    }

    pub fn playlist_move(&mut self, from: i64, to: i64) -> Result<(), String> {
        self.send(vec![json!("playlist-move"), json!(from), json!(to)])
            .map(|_| ())
    }

    pub fn playlist_remove(&mut self, index: i64) -> Result<(), String> {
        self.send(vec![json!("playlist-remove"), json!(index)])
            .map(|_| ())
    }

    pub fn playlist_clear(&mut self) -> Result<(), String> {
        self.send(vec![json!("playlist-clear")]).map(|_| ())
    }

    pub fn stop(&mut self) -> Result<(), String> {
        self.send(vec![json!("stop")]).map(|_| ())
    }

    pub fn quit(&mut self) {
        let _ = self.writer.write_all(b"{\"command\":[\"quit\"]}\n");
        for _ in 0..20 {
            if let Ok(Some(_)) = self.child.try_wait() {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_file(&self.socket);
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        self.quit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_audio_devices_and_skips_bad_entries() {
        let value = json!([
            {"name": "auto", "description": "自动选择"},
            {"description": "没有名字"},
            {"name": "pulse/1", "description": "扬声器"}
        ]);
        assert_eq!(
            parse_audio_devices(&value),
            vec![
                ("auto".to_string(), "自动选择".to_string()),
                ("pulse/1".to_string(), "扬声器".to_string())
            ]
        );
        assert!(parse_audio_devices(&json!(null)).is_empty());
    }
}
