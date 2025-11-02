use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "kebab-case")]
#[serde(default)]
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
    pub fn get_pattern(&mut self, total_millis: Option<i64>) -> String {
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
}

#[cfg(test)]
mod tests {
    use super::TimeFormat;

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
}
