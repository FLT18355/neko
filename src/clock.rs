use std::time::{SystemTime, UNIX_EPOCH};

/// 本地时间 `HH:MM:SS`；系统调用失败时显示占位符。
pub fn now() -> String {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0) as libc::time_t;
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    #[cfg(unix)]
    let valid = !unsafe { libc::localtime_r(&seconds, &mut tm) }.is_null();
    #[cfg(windows)]
    let valid = unsafe { libc::localtime_s(&mut tm, &seconds) } == 0;
    #[cfg(not(any(unix, windows)))]
    let valid = false;
    if !valid {
        return "--:--:--".into();
    }
    format!("{:02}:{:02}:{:02}", tm.tm_hour, tm.tm_min, tm.tm_sec)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_has_fixed_shape() {
        let text = now();
        assert_eq!(text.len(), 8);
        assert!(text.chars().enumerate().all(|(index, ch)| {
            if index == 2 || index == 5 {
                ch == ':'
            } else {
                ch.is_ascii_digit()
            }
        }));
    }
}
