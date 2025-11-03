use livesplit_core::{TimeSpan, Timer, TimingMethod};
use serde::Deserialize;
use std::fmt::Write as _;
use time::Duration as TimeDuration;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
#[allow(clippy::struct_excessive_bools)]
pub struct TimeFormat {
    pub show_hours: bool,
    pub show_minutes: bool,
    pub show_seconds: bool,
    pub show_decimals: bool,
    pub decimal_places: u8,
    pub dynamic: bool,
    cached_pattern: Option<String>,
}

impl Default for TimeFormat {
    fn default() -> Self {
        // Default mirrors "h:m:s.dd"
        Self {
            show_hours: true,
            show_minutes: true,
            show_seconds: true,
            show_decimals: true,
            decimal_places: 2,
            dynamic: false,
            cached_pattern: None,
        }
    }
}

impl TimeFormat {
    fn get_pattern(&mut self, total_millis: Option<i64>) -> String {
        if self.dynamic || self.cached_pattern.is_none() {
            self.cached_pattern = Some(self.compute_pattern(total_millis));
        }

        self.cached_pattern.clone().unwrap()
    }

    /// Builds a pattern string (e.g., "h:m:s.dd") based on the configured flags.
    /// If `dynamic` is enabled and `total_millis` is provided, this adjusts the
    /// pattern to match the duration. For example, with minutes+seconds+decimals
    /// enabled and under a minute, this yields "s.dd"; over a minute, "m:s".
    fn compute_pattern(&self, total_millis: Option<i64>) -> String {
        // Resolve dynamic visibility for each component
        let mut show_hours = self.show_hours;
        let mut show_minutes = self.show_minutes;
        let show_seconds = self.show_seconds;
        let mut show_decimals = self.show_decimals;

        if self.dynamic {
            if let Some(ms) = total_millis {
                if ms < 60_000 {
                    // Under a minute: hide hours and minutes
                    show_hours = false;
                    show_minutes = false;
                    // Keep seconds/decimals as configured
                } else if ms < 3_600_000 {
                    // Under an hour: hide hours
                    show_hours = false;
                    // When both minutes and seconds are shown, suppress decimals (example behavior)
                    if self.show_minutes && self.show_seconds {
                        show_decimals = false;
                    }
                } else {
                    // 1 hour or more: keep hours; suppress decimals when minutes+seconds are shown
                    if self.show_minutes && self.show_seconds {
                        show_decimals = false;
                    }
                }
            }
        }

        let mut pattern = String::new();
        let push_sep = |sep: char, pat: &mut String| {
            if !pat.is_empty() {
                pat.push(sep);
            }
        };

        if show_hours {
            pattern.push('h');
        }
        if show_minutes {
            push_sep(':', &mut pattern);
            pattern.push('m');
        }
        if show_seconds {
            push_sep(':', &mut pattern);
            pattern.push('s');
        }
        if show_decimals && self.decimal_places > 0 {
            pattern.push('.');
            for _ in 0..self.decimal_places {
                pattern.push('d');
            }
        }

        // Fallback to seconds if nothing was selected
        if pattern.is_empty() {
            if self.show_seconds {
                pattern.push('s');
                if self.show_decimals && self.decimal_places > 0 {
                    pattern.push('.');
                    for _ in 0..self.decimal_places {
                        pattern.push('d');
                    }
                }
            } else {
                // Minimal sensible default
                pattern.push('s');
            }
        }

        pattern
    }

    pub fn format_time_span_opt(&mut self, span: Option<TimeSpan>) -> String {
        match span {
            Some(s) => self.format_time_span(&s),
            None => "--".to_owned(),
        }
    }

    /// Formats a `TimeSpan` using the class `pattern`.
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
    pub fn format_time_span(&mut self, span: &TimeSpan) -> String {
        // Determine sign and absolute time in milliseconds
        let total_ms = span.total_milliseconds();
        let abs_ms = total_ms.abs() as i64;

        let hours = abs_ms / 3_600_000;
        let minutes = (abs_ms / 60_000) % 60;
        let seconds = (abs_ms / 1_000) % 60;
        let millis = abs_ms % 1_000;

        let pattern = self.get_pattern(Some(abs_ms));

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
                'h' => Self::append_number(&mut out, hours, false),
                'm' => Self::append_number(&mut out, minutes, false),
                's' => Self::append_number(&mut out, seconds, true),
                'd' => Self::append_fraction(&mut out, millis, count),
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

    /// Formats a split `Time` (which may contain both Real Time and Game Time) into a string.
    /// The caller decides whether to use game time or real time via `use_game_time`.
    pub fn format_split_time(
        &mut self,
        time: &livesplit_core::Time,
        timing_method: TimingMethod,
    ) -> String {
        let span_opt = if timing_method == TimingMethod::GameTime {
            time.game_time
        } else {
            time.real_time
        };

        self.format_time_span_opt(span_opt)
    }

    /// Formats the overall timer's current attempt duration into a string using this format.
    pub fn format_timer(&mut self, timer: &Timer) -> String {
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
        let out = self.format_duration(&dur);
        if dur < TimeDuration::ZERO {
            format!("-{out}")
        } else {
            out
        }
    }

    /// Formats a segment duration.
    pub fn format_segment_time(&mut self, duration: &TimeDuration) -> String {
        self.format_duration(duration)
    }

    /// Formats a `time::Duration` using the same pattern machinery by converting to `TimeSpan`.
    pub fn format_duration(&mut self, duration: &TimeDuration) -> String {
        let span = TimeSpan::from_milliseconds(duration.whole_nanoseconds() as f64 / 1_000_000.0);
        self.format_time_span(&span)
    }

    pub fn format_duration_opt(&mut self, duration: Option<TimeDuration>) -> String {
        match duration {
            Some(d) => self.format_duration(&d),
            None => "--".to_owned(),
        }
    }

    fn append_number(out: &mut String, value: i64, always_show: bool) {
        if value <= 0 && out.is_empty() && !always_show {
        } else {
            let _ = write!(
                out,
                "{:0width$}",
                value,
                width = if out.is_empty() {
                    value.to_string().len()
                } else {
                    2 // Minutes after hours, seconds after minutes are always 2 digits
                }
            );
        }
    }

    /// Appends the fractional part of the seconds, given milliseconds and desired digit count.
    /// - d  -> deciseconds (e.g., "1")
    /// - dd -> centiseconds (e.g., "17")
    /// - ddd -> milliseconds (e.g., "178")
    ///
    /// For widths > 3, pads with zeros (truncation, not rounding).
    fn append_fraction(out: &mut String, millis: i64, width: usize) {
        // Always zero-pad to 3 digits for ms, then cut/pad as needed
        let base = format!("{millis:03}"); // e.g., "007", "120", "999"
        if width <= 3 {
            out.push_str(&base[..width]);
        } else {
            out.push_str(&base);
            out.push_str(&"0".repeat(width - 3));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::TimeFormat;
    use livesplit_core::TimeSpan;

    fn make_tf(hours: bool, minutes: bool, seconds: bool, decimals: u8) -> TimeFormat {
        TimeFormat {
            show_hours: hours,
            show_minutes: minutes,
            show_seconds: seconds,
            show_decimals: decimals > 0,
            decimal_places: decimals,
            dynamic: false,
            cached_pattern: None,
        }
    }

    #[test]
    fn non_dynamic_full_hms_decimals() {
        let tf = TimeFormat {
            show_hours: true,
            show_minutes: true,
            show_seconds: true,
            show_decimals: true,
            decimal_places: 2,
            dynamic: false,
            cached_pattern: None,
        };
        assert_eq!(tf.compute_pattern(None), "h:m:s.dd");
        assert_eq!(tf.compute_pattern(Some(500)), "h:m:s.dd");
        assert_eq!(tf.compute_pattern(Some(65_000)), "h:m:s.dd");
        assert_eq!(tf.compute_pattern(Some(3_700_000)), "h:m:s.dd");
    }

    #[test]
    fn non_dynamic_no_decimals_min_sec() {
        let tf = TimeFormat {
            show_hours: false,
            show_minutes: true,
            show_seconds: true,
            show_decimals: false,
            decimal_places: 3,
            dynamic: false,
            cached_pattern: None,
        };
        assert_eq!(tf.compute_pattern(None), "m:s");
        assert_eq!(tf.compute_pattern(Some(59_999)), "m:s");
    }

    #[test]
    fn dynamic_under_minute_prefers_seconds_with_decimals() {
        let tf = TimeFormat {
            show_hours: false,
            show_minutes: true,
            show_seconds: true,
            show_decimals: true,
            decimal_places: 2,
            dynamic: true,
            cached_pattern: None,
        };
        // under 1 minute -> hide minutes, keep s.dd
        assert_eq!(tf.compute_pattern(Some(59_500)), "s.dd");
    }

    #[test]
    fn dynamic_over_minute_suppresses_decimals_with_min_sec() {
        let tf = TimeFormat {
            show_hours: false,
            show_minutes: true,
            show_seconds: true,
            show_decimals: true,
            decimal_places: 3,
            dynamic: true,
            cached_pattern: None,
        };
        // >= 1 minute and < 1 hour -> m:s (no decimals)
        assert_eq!(tf.compute_pattern(Some(60_000)), "m:s");
        assert_eq!(tf.compute_pattern(Some(3_599_999)), "m:s");
    }

    #[test]
    fn dynamic_over_hour_includes_hours_and_suppresses_decimals() {
        let tf = TimeFormat {
            show_hours: true,
            show_minutes: true,
            show_seconds: true,
            show_decimals: true,
            decimal_places: 2,
            dynamic: true,
            cached_pattern: None,
        };
        // >= 1 hour -> h:m:s (no decimals)
        assert_eq!(tf.compute_pattern(Some(3_600_000)), "h:m:s");
        assert_eq!(tf.compute_pattern(Some(3_700_000)), "h:m:s");
    }

    #[test]
    fn decimal_places_width_applied_when_decimals_visible() {
        let tf = TimeFormat {
            show_hours: false,
            show_minutes: false,
            show_seconds: true,
            show_decimals: true,
            decimal_places: 4,
            dynamic: false,
            cached_pattern: None,
        };
        assert_eq!(tf.compute_pattern(None), "s.dddd");
    }

    #[test]
    fn fallback_to_seconds_when_all_hidden() {
        let tf = TimeFormat {
            show_hours: false,
            show_minutes: false,
            show_seconds: false,
            show_decimals: false,
            decimal_places: 0,
            dynamic: false,
            cached_pattern: None,
        };
        assert_eq!(tf.compute_pattern(None), "s");
    }

    #[test]
    fn format_time_span_basic() {
        let t = TimeSpan::from_milliseconds(3_145.0); // 00:00:03.145
        let mut tf_s = make_tf(false, false, true, 0); // "s"
        assert_eq!(tf_s.format_time_span(&t), "3");
        let mut tf_sd = make_tf(false, false, true, 1); // "s.d"
        assert_eq!(tf_sd.format_time_span(&t), "3.1");
        let mut tf_sdd = make_tf(false, false, true, 2); // "s.dd"
        assert_eq!(tf_sdd.format_time_span(&t), "3.14");
        let mut tf_sddd = make_tf(false, false, true, 3); // "s.ddd"
        assert_eq!(tf_sddd.format_time_span(&t), "3.145");
    }

    #[test]
    fn format_time_span_minutes_seconds() {
        let t = TimeSpan::from_milliseconds(125_340.0); // 00:02:05.340
        let mut tf_ms = make_tf(false, true, true, 0); // "m:s"
        assert_eq!(tf_ms.format_time_span(&t), "2:05");
        let mut tf_msdd = make_tf(false, true, true, 2); // "m:s.dd"
        assert_eq!(tf_msdd.format_time_span(&t), "2:05.34");
    }

    #[test]
    fn format_time_span_hours_minutes_seconds() {
        let t = TimeSpan::from_milliseconds(3_845_999.0); // 01:04:05.999
        let mut tf_hms = make_tf(true, true, true, 0); // "h:m:s"
        assert_eq!(tf_hms.format_time_span(&t), "1:04:05");
        let mut tf_hmsddd = make_tf(true, true, true, 3); // "h:m:s.ddd"
        assert_eq!(tf_hmsddd.format_time_span(&t), "1:04:05.999");
    }

    #[test]
    fn format_time_span_negative() {
        let t = TimeSpan::from_milliseconds(-61_230.0); // -00:01:01.230
        let mut tf_msdd = make_tf(false, true, true, 2); // "m:s.dd"
        assert_eq!(tf_msdd.format_time_span(&t), "1:01.23");
    }

    #[test]
    fn format_time_span_option() {
        let mut tf_ms = make_tf(false, true, true, 0); // "m:s"
        assert_eq!(tf_ms.format_time_span_opt(None), "--");
        let t = TimeSpan::from_milliseconds(10_000.0);
        assert_eq!(tf_ms.format_time_span(&t), "10");
    }

    #[test]
    fn format_duration_basic() {
        let d = time::Duration::milliseconds(3_145);
        let mut tf = make_tf(false, false, true, 2); // "s.dd"
        assert_eq!(tf.format_duration(&d), "3.14");
    }

    #[test]
    fn format_duration_min_sec() {
        let d = time::Duration::milliseconds(125_340);
        let mut tf = make_tf(false, true, true, 2); // "m:s.dd"
        assert_eq!(tf.format_duration(&d), "2:05.34");
    }

    #[test]
    fn format_duration_hours() {
        let d = time::Duration::milliseconds(3_845_999);
        let mut tf = make_tf(true, true, true, 2); // "h:m:s.dd"
        assert_eq!(tf.format_duration(&d), "1:04:05.99");
    }

    #[test]
    fn format_duration_negative() {
        let d = time::Duration::milliseconds(-61_230);
        let mut tf = make_tf(false, true, true, 2); // "m:s.dd"
        assert_eq!(tf.format_duration(&d), "1:01.23");
    }

    #[test]
    fn format_duration_option() {
        let mut tf = make_tf(false, false, true, 2); // "s.dd"
        assert_eq!(tf.format_duration_opt(None), "--");
        let d = time::Duration::seconds(10);
        assert_eq!(tf.format_duration_opt(Some(d)), "10.00");
    }
}
