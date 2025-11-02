use crate::config::Config;
use livesplit_core::{TimeSpan, Timer, TimingMethod};
use time::Duration as TimeDuration;

/// Default patterns used when formatting times for each context.
pub const SPLIT_TIME_FORMAT: &str = "h:m:s.dd";
pub const TIMER_TIME_FORMAT: &str = "h:m:s.dd";
pub const SEGMENT_TIME_FORMAT: &str = "h:m:s.dd";

/// Formats an optional `TimeSpan` using the provided `pattern`.
/// If the `TimeSpan` is `None`, this returns `"--"`.
///
/// Example:
/// - format_time_span_opt(Some(span), "h:m:s.dd") -> "3:25.17"
/// - format_time_span_opt(None, "h:m:s.dd") -> "--"
pub fn format_time_span_opt(span: Option<TimeSpan>, pattern: &str) -> String {
    match span {
        Some(s) => format_time_span(&s, pattern),
        None => "--".to_string(),
    }
}

/// Formats a `TimeSpan` using the provided `pattern`.
///
/// Supported tokens:
/// - h                -> hours (0+)
/// - m                -> minutes (0-59)
/// - s                -> seconds (0-59)
/// - d / dd / ddd...  -> fractional seconds (tenths/centiseconds/milliseconds). Truncated, not rounded.
///
/// Any other characters are treated as literals (e.g., ":" or ".").
///
/// Examples:
/// - "h:m:ss"       ->  "1:02:03"
/// - "m:s.dd"       ->  "2:03.45"
/// - "h:m:s.d"      ->  "1:02:03.4"
/// - "m:s.ddd"      ->  "2:03.456"
///
/// Notes:
/// - Negative values are prefixed with "-".
pub fn format_time_span(span: &TimeSpan, pattern: &str) -> String {
    // Determine sign and absolute time in milliseconds
    let total_ms = span.total_milliseconds();
    let _negative = total_ms < 0.0;
    let abs_ms = total_ms.abs() as i64;

    let hours = abs_ms / 3_600_000;
    let minutes = (abs_ms / 60_000) % 60;
    let seconds = (abs_ms / 1_000) % 60;
    let millis = abs_ms % 1_000;

    let mut out = String::new();

    // Tokenize the pattern by runs of the same character
    let mut chars = pattern.chars().peekable();
    while let Some(ch) = chars.next() {
        // Count how many consecutive identical chars we have for token width
        let mut count = 1usize;
        while let Some(&next) = chars.peek() {
            if next == ch {
                chars.next();
                count += 1;
            } else {
                break;
            }
        }

        match ch {
            'h' => append_number(&mut out, hours as i64, false),
            'm' => append_number(&mut out, minutes as i64, false),
            's' => append_number(&mut out, seconds as i64, true),
            'd' => append_fraction(&mut out, millis as i64, count),
            _ => {
                // Literal character(s)
                for _ in 0..count {
                    // Only push if there is some character before
                    if !out.is_empty() {
                        out.push(ch);
                    }
                }
            }
        }
    }

    out
}

fn append_number(out: &mut String, value: i64, always_show: bool) {
    if value <= 0 && out.is_empty() && !always_show { // Skip showing value if there is no value before, to prevent : or . at start
    } else {
        out.push_str(&format!(
            "{:0width$}",
            value,
            width = if out.is_empty() {
                value.to_string().len()
            } else {
                2 // Minutes after hours, seconds after minutes are always 2 digits
            }
        ));
    }
}

/// Appends the fractional part of the seconds, given milliseconds and desired digit count.
/// - d  -> deciseconds (e.g., "1")
/// - dd -> centiseconds (e.g., "17")
/// - ddd -> milliseconds (e.g., "178")
/// For widths > 3, pads with zeros (truncation, not rounding).
fn append_fraction(out: &mut String, millis: i64, width: usize) {
    // Always zero-pad to 3 digits for ms, then cut/pad as needed
    let base = format!("{:03}", millis); // e.g., "007", "120", "999"
    if width <= 3 {
        out.push_str(&base[..width]);
    } else {
        out.push_str(&base);
        out.push_str(&"0".repeat(width - 3));
    }
}

/// Formats a split `Time` (which may contain both Real Time and Game Time) into a string,
/// choosing the appropriate timing method based on the Config and the Timer's current method.
pub fn format_split_time(
    time: &livesplit_core::Time,
    timer: &Timer,
    config: &mut Config,
) -> String {
    let use_game_time =
        config.is_game_time() || timer.current_timing_method() == TimingMethod::GameTime;

    let span_opt = if use_game_time {
        time.game_time
    } else {
        time.real_time
    };
    let total_ms = span_opt.map(|s| s.total_milliseconds() as i64);
    let pattern = config.format.split.get_pattern(total_ms);
    format_time_span_opt(span_opt, &pattern)
}

/// Formats the overall timer's current attempt duration into a string using the configured timer format.
/// This centralizes the display formatting for the main timer readout.
pub fn format_timer(timer: &Timer, config: &mut Config) -> String {
    let dur = timer
        .current_attempt_duration()
        .to_duration()
        .checked_add(timer.run().offset().to_duration())
        .unwrap_or_default()
        .checked_sub(timer.get_pause_time().unwrap_or_default().to_duration())
        .unwrap_or_default()
        .checked_sub(if timer.current_timing_method() == TimingMethod::GameTime {
            timer.loading_times().to_duration()
        } else {
            TimeDuration::ZERO
        })
        .unwrap_or_default();
    let ms_i64 = (dur.whole_nanoseconds() / 1_000_000) as i64;
    let pattern = config.format.timer.get_pattern(Some(ms_i64.abs()));
    let out = format_duration(&dur, &pattern);
    if dur < TimeDuration::ZERO {
        format!("-{}", out)
    } else {
        out
    }
}

/// Option variant for `time::Duration`.
pub fn format_segment_time(duration: &TimeDuration, config: &mut Config) -> String {
    let ms_i64 = (duration.whole_nanoseconds() / 1_000_000) as i64;
    let pattern = config.format.segment.get_pattern(Some(ms_i64.abs()));
    format_duration(duration, &pattern)
}

/// Formats a `time::Duration` using the same pattern machinery by converting to TimeSpan.
pub fn format_duration(duration: &TimeDuration, pattern: &str) -> String {
    let span = TimeSpan::from_milliseconds(duration.whole_nanoseconds() as f64 / 1_000_000.0);
    format_time_span(&span, pattern)
}

pub fn format_duration_opt(duration: Option<TimeDuration>, pattern: &str) -> String {
    match duration {
        Some(d) => format_duration(&d, pattern),
        None => "--".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span_ms(ms: i64) -> TimeSpan {
        TimeSpan::from_seconds(ms as f64 / 1000.0)
    }

    #[test]
    fn test_format_basic() {
        let t = span_ms(3_145); // 00:00:03.145
        assert_eq!(format_time_span(&t, "s"), "3");
        assert_eq!(format_time_span(&t, "s.d"), "3.1");
        assert_eq!(format_time_span(&t, "s.dd"), "3.14");
        assert_eq!(format_time_span(&t, "s.ddd"), "3.145");
    }

    #[test]
    fn test_minutes_seconds() {
        let t = span_ms(125_340); // 00:02:05.340
        assert_eq!(format_time_span(&t, "m:s"), "2:05");
        assert_eq!(format_time_span(&t, "m:s.dd"), "2:05.34");
    }

    #[test]
    fn test_hours_minutes_seconds() {
        let t = span_ms(3_845_999); // 01:04:05.999
        assert_eq!(format_time_span(&t, "h:m:s"), "1:04:05");
        assert_eq!(format_time_span(&t, "h:m:s.ddd"), "1:04:05.999");
    }

    #[test]
    fn test_negative() {
        let t = span_ms(-61_230); // -00:01:01.230
        assert_eq!(format_time_span(&t, "m:s.dd"), "1:01.23");
    }

    #[test]
    fn test_option() {
        assert_eq!(format_time_span_opt(None, SPLIT_TIME_FORMAT), "--");
        let t = span_ms(10_000);
        assert_eq!(format_time_span_opt(Some(t), "m:s"), "10");
    }

    #[test]
    fn test_format_duration_basic() {
        let d = TimeDuration::milliseconds(3_145);
        assert_eq!(format_duration(&d, SEGMENT_TIME_FORMAT), "3.14");
    }

    #[test]
    fn test_format_duration_min_sec() {
        let d = TimeDuration::milliseconds(125_340);
        assert_eq!(format_duration(&d, SEGMENT_TIME_FORMAT), "2:05.34");
    }

    #[test]
    fn test_format_duration_hours() {
        let d = TimeDuration::milliseconds(3_845_999);
        assert_eq!(format_duration(&d, SEGMENT_TIME_FORMAT), "1:04:05.99");
    }

    #[test]
    fn test_format_duration_negative() {
        let d = TimeDuration::milliseconds(-61_230);
        assert_eq!(format_duration(&d, SEGMENT_TIME_FORMAT), "1:01.23");
    }

    #[test]
    fn test_format_duration_option() {
        assert_eq!(format_duration_opt(None, SEGMENT_TIME_FORMAT), "--");
        let d = TimeDuration::seconds(10);
        assert_eq!(format_duration_opt(Some(d), SEGMENT_TIME_FORMAT), "10.00");
    }
}
