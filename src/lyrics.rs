use std::fs;
use std::path::Path;

#[derive(Clone, Debug, Default)]
pub struct Lrc {
    pub lines: Vec<(f64, String)>,
}

fn parse_timestamp(tag: &str) -> Option<f64> {
    let parts: Vec<&str> = tag.split(':').collect();
    match parts.len() {
        2 => {
            let minutes: f64 = parts[0].trim().parse().ok()?;
            let seconds: f64 = parts[1].trim().parse().ok()?;
            Some(minutes * 60.0 + seconds)
        }
        3 => {
            let hours: f64 = parts[0].trim().parse().ok()?;
            let minutes: f64 = parts[1].trim().parse().ok()?;
            let seconds: f64 = parts[2].trim().parse().ok()?;
            Some(hours * 3600.0 + minutes * 60.0 + seconds)
        }
        _ => None,
    }
}

impl Lrc {
    pub fn load_for(audio: &Path) -> Option<Self> {
        let mut candidates = vec![audio.with_extension("lrc")];
        candidates.push(audio.with_extension("LRC"));
        for path in candidates {
            if let Ok(text) = fs::read_to_string(&path) {
                let text = text.trim_start_matches('\u{feff}');
                return Some(Self::parse(text));
            }
        }
        None
    }

    pub fn parse(text: &str) -> Self {
        let mut lines = Vec::new();
        let mut offset = 0.0;
        for raw in text.lines() {
            let mut rest = raw.trim();
            let mut stamps = Vec::new();
            while let Some(after_open) = rest.strip_prefix('[') {
                let Some(close) = after_open.find(']') else {
                    break;
                };
                let tag = &after_open[..close];
                if let Some(value) = tag.strip_prefix("offset:") {
                    if let Ok(ms) = value.trim().parse::<f64>() {
                        offset = ms / 1000.0;
                    }
                } else if let Some(time) = parse_timestamp(tag) {
                    stamps.push(time);
                }
                rest = after_open[close + 1..].trim_start();
            }
            let content = rest.trim();
            if content.is_empty() {
                continue;
            }
            for stamp in stamps {
                lines.push((stamp, content.to_string()));
            }
        }
        for (time, _) in &mut lines {
            *time -= offset;
        }
        lines.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        Self { lines }
    }

    pub fn index_at(&self, position: f64) -> Option<usize> {
        let count = self.lines.partition_point(|(time, _)| *time <= position);
        count.checked_sub(1)
    }
}

pub fn window(index: usize, total: usize, rows: usize) -> (usize, usize) {
    if total == 0 || rows == 0 {
        return (0, 0);
    }
    let start = index
        .saturating_sub(rows / 2)
        .min(total.saturating_sub(rows));
    (start, (start + rows).min(total))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tags_and_offset() {
        let text = "[ti:示例]\n[offset:200]\n[00:01.00]第一句\n[00:03.50][00:04.50]第二句\n";
        let lrc = Lrc::parse(text);
        assert_eq!(lrc.lines.len(), 3);
        assert!((lrc.lines[0].0 - 0.8).abs() < 0.001);
        assert_eq!(lrc.lines[2].1, "第二句");
    }

    #[test]
    fn finds_current_line() {
        let lrc = Lrc::parse("[00:00.00]开始\n[00:05.00]中间\n[00:10.00]结束\n");
        assert_eq!(lrc.index_at(0.0), Some(0));
        assert_eq!(lrc.index_at(7.5), Some(1));
        assert_eq!(lrc.index_at(99.0), Some(2));
        assert_eq!(lrc.index_at(-1.0), None);
    }

    #[test]
    fn keeps_current_line_inside_window() {
        assert_eq!(window(0, 10, 5), (0, 5));
        assert_eq!(window(5, 10, 5), (3, 8));
        assert_eq!(window(9, 10, 5), (5, 10));
        assert_eq!(window(2, 3, 5), (0, 3));
        assert_eq!(window(2, 0, 5), (0, 0));
        assert_eq!(window(2, 5, 0), (0, 0));
    }
}
