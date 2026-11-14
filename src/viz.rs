//! 全屏歌词右侧的 cava 风格频谱条。
//!
//! shortcut: 没有采集音频，按播放进度合成假频谱，接入真实 FFT 时只替换 step。

pub const BANDS: usize = 64;

/// 推进一帧：播放时向合成目标靠近，暂停时回落，快起慢落得到 cava 的手感。
pub fn step(levels: &mut [f32; BANDS], playing: bool, time: f64) {
    let t = time * 2.2;
    for (band, level) in levels.iter_mut().enumerate() {
        let p = band as f64 / BANDS as f64;
        // 中频峰、两端收：贴近真实频谱形状
        let shape = (1.0 - p * 0.35) * (0.55 + 0.45 * (p * std::f64::consts::PI).sin());
        let target = if playing {
            let wave = 0.40
                + 0.28 * (t * 1.17 + band as f64 * 0.41).sin()
                + 0.20 * (t * 2.53 + band as f64 * 1.13).sin()
                + 0.12 * (t * 4.61 + band as f64 * 2.71).sin();
            (wave * shape * 1.2).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // 按 ~25 帧/秒调过：起得快，落得慢
        let rate = if target > f64::from(*level) {
            0.35
        } else {
            0.12
        };
        *level += (target - f64::from(*level)) as f32 * rate;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_rise_when_playing_and_settle_when_paused() {
        let mut levels = [0.0; BANDS];
        step(&mut levels, true, 1.0);
        assert!(levels.iter().any(|level| *level > 0.05));
        for _ in 0..300 {
            step(&mut levels, false, 5.0);
        }
        assert!(levels.iter().all(|level| *level < 0.01));
        assert!(levels.iter().all(|level| (0.0..=1.0).contains(level)));
    }
}
