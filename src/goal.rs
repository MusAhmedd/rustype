//! Word counts, the daily writing goal and the writing-session timer.

/// Gaps between keystrokes longer than this don't count as writing time.
const IDLE_GAP: f64 = 60.0;

/// How long the "goal reached" banner and glow stay up, in seconds.
pub const CELEBRATE_SECS: f64 = 4.5;

pub fn words(text: &str) -> usize {
    text.split_whitespace().count()
}

pub fn chars(text: &str) -> usize {
    text.chars().filter(|c| *c != '\n').count()
}

/// Today's local date as days since 1970-01-01.
pub fn today() -> u32 {
    let (y, m, d) = local_ymd();
    days_from_civil(y, m, d)
}

#[cfg(windows)]
fn local_ymd() -> (i32, u32, u32) {
    use windows_sys::Win32::System::SystemInformation::GetLocalTime;
    // SAFETY: GetLocalTime only writes into the struct we hand it.
    let st = unsafe {
        let mut st = std::mem::zeroed();
        GetLocalTime(&mut st);
        st
    };
    (st.wYear as i32, st.wMonth as u32, st.wDay as u32)
}

#[cfg(not(windows))]
fn local_ymd() -> (i32, u32, u32) {
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
    civil_from_days((secs / 86_400) as i64)
}

/// Howard Hinnant's days-from-civil algorithm.
fn days_from_civil(y: i32, m: u32, d: u32) -> u32 {
    let y = if m <= 2 { y - 1 } else { y } as i64;
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    (era * 146_097 + doe - 719_468).max(0) as u32
}

#[cfg(not(windows))]
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = (yoe + era * 400 + if m <= 2 { 1 } else { 0 }) as i32;
    (y, m, d)
}

/// Writing activity since the app started.
#[derive(Default)]
pub struct Session {
    /// Seconds spent actually typing (short pauses included).
    pub active_secs: f64,
    /// Net words typed this session.
    pub words: i64,
    last_key: Option<f64>,
    /// When the goal was reached (egui seconds), for the banner.
    pub reached_at: Option<f64>,
}

impl Session {
    pub fn keystroke(&mut self, now: f64) {
        if let Some(last) = self.last_key {
            let gap = now - last;
            if (0.0..IDLE_GAP).contains(&gap) {
                self.active_secs += gap;
            }
        }
        self.last_key = Some(now);
    }

    /// 0..1 strength of the goal celebration, fading out at the end.
    pub fn celebration(&self, now: f64) -> f32 {
        let Some(t) = self.reached_at else { return 0.0 };
        let age = now - t;
        if !(0.0..CELEBRATE_SECS).contains(&age) {
            return 0.0;
        }
        let fade_in = (age / 0.25).min(1.0);
        let fade_out = ((CELEBRATE_SECS - age) / 1.0).min(1.0);
        (fade_in * fade_out) as f32
    }
}

/// "1 word" / "5 words".
pub fn n_words(n: i64) -> String {
    if n == 1 {
        "1 word".into()
    } else {
        format!("{n} words")
    }
}

/// "12 min" / "1 h 05 min".
pub fn duration(secs: f64) -> String {
    let mins = (secs / 60.0) as u64;
    if mins < 60 {
        format!("{mins} min")
    } else {
        format!("{} h {:02} min", mins / 60, mins % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_words() {
        assert_eq!(words(""), 0);
        assert_eq!(words("  hello   world\nagain "), 3);
        assert_eq!(chars("ab\ncd"), 4);
    }

    #[test]
    fn civil_dates() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(days_from_civil(2000, 3, 1), 11_017);
        assert_eq!(days_from_civil(2026, 11, 19), 20_776);
    }

    #[test]
    fn session_ignores_long_gaps() {
        let mut s = Session::default();
        s.keystroke(0.0);
        s.keystroke(10.0);
        s.keystroke(200.0);
        s.keystroke(205.0);
        assert_eq!(s.active_secs, 15.0);
    }

    #[test]
    fn formats_duration() {
        assert_eq!(duration(59.0), "0 min");
        assert_eq!(duration(12.0 * 60.0), "12 min");
        assert_eq!(duration(65.0 * 60.0), "1 h 05 min");
    }
}
