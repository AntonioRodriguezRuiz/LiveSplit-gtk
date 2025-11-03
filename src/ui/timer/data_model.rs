use crate::config::Config;
use crate::formatters::label::format_label;

use gtk4::prelude::ListBoxRowExt as _;
use gtk4::ListBox;

use livesplit_core::{Timer, TimingMethod};
use time::Duration as TimeDuration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SplitRowData {
    pub title: String,
    pub value_text: String,
    pub segment_classes: Vec<&'static str>,
    pub label_classes: Vec<&'static str>,
}

/// Helper: Returns the current attempt duration adjusted for pause/loading/offset for the current timing method.
fn current_attempt_running_duration(timer: &Timer) -> TimeDuration {
    let current_dur = timer
        .current_attempt_duration()
        .to_duration()
        .checked_add(timer.run().offset().to_duration())
        .unwrap_or_default();
    let paused_time = timer.get_pause_time().unwrap_or_default().to_duration();
    let loading_times = if timer.current_timing_method() == TimingMethod::GameTime {
        timer.loading_times().to_duration()
    } else {
        TimeDuration::ZERO
    };

    current_dur
        .checked_sub(paused_time)
        .unwrap_or_default()
        .checked_sub(loading_times)
        .unwrap_or_default()
}

/// Helper: The best segment (gold) duration for this segment under the current timing method.
fn best_segment_duration(segment: &livesplit_core::Segment, timer: &Timer) -> TimeDuration {
    if timer.current_timing_method() == TimingMethod::GameTime {
        segment
            .best_segment_time()
            .game_time
            .unwrap_or_default()
            .to_duration()
    } else {
        segment
            .best_segment_time()
            .real_time
            .unwrap_or_default()
            .to_duration()
    }
}

/// Helper: The split time for the segment under the current timing method.
fn segment_split_time(segment: &livesplit_core::Segment, timer: &Timer) -> TimeDuration {
    if timer.current_timing_method() == TimingMethod::GameTime {
        segment
            .split_time()
            .game_time
            .unwrap_or_default()
            .to_duration()
    } else {
        segment
            .split_time()
            .real_time
            .unwrap_or_default()
            .to_duration()
    }
}

/// Helper: Previous segment's comparison duration and split time under the current method.
fn previous_comparison_values(timer: &Timer, index: usize) -> (TimeDuration, TimeDuration) {
    let segments = timer.run().segments();
    if index > 0 {
        let prev = &segments[index - 1];
        let prev_comp_duration = prev
            .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
            .unwrap_or_default()
            .to_duration();
        let prev_comp_time = if timer.current_timing_method() == TimingMethod::GameTime {
            prev.split_time()
                .game_time
                .unwrap_or_default()
                .to_duration()
        } else {
            prev.split_time()
                .real_time
                .unwrap_or_default()
                .to_duration()
        };
        (prev_comp_duration, prev_comp_time)
    } else {
        (TimeDuration::ZERO, TimeDuration::ZERO)
    }
}

/// Helper: Format a signed duration with +, -, or ~ and the configured short format.
fn format_signed(diff: TimeDuration, config: &mut Config) -> String {
    let sign = if diff.is_positive() {
        "+"
    } else if diff.is_negative() {
        "-"
    } else {
        "~"
    };
    let abs = diff.abs();
    let formatted = config.format.segment.format_segment_time(&abs);
    format!("{sign}{formatted}")
}

pub fn compute_split_rows(timer: &Timer, config: &mut Config) -> Vec<SplitRowData> {
    let segments = timer.run().segments();
    let opt_current_segment_index = timer.current_split_index();

    segments
        .iter()
        .enumerate()
        .map(|(index, segment)| {
            compute_segment_row(timer, config, opt_current_segment_index, index, segment)
        })
        .collect()
}

/// Builds the data for a given segment given the current `Timer` and `Config`.
/// This function is pure (no GTK dependencies) and is intended to be unit-tested.
pub fn compute_segment_row(
    timer: &Timer,
    config: &mut Config,
    opt_current_segment_index: Option<usize>,
    index: usize,
    segment: &livesplit_core::Segment,
) -> SplitRowData {
    let title = segment.name().to_owned();

    // Default value is the comparison for this segment.
    let segment_comparison = segment
        .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
        .unwrap_or_default()
        .to_duration();

    let mut value_text = config.format.split.format_split_time(
        &segment.comparison(timer.current_comparison()),
        timer.current_timing_method(),
    );

    let mut segment_classes: Vec<&'static str> = Vec::new();
    let mut label_classes: Vec<&'static str> = Vec::new();

    if let Some(current_segment_index) = opt_current_segment_index {
        let goldsplit_duration = best_segment_duration(segment, timer);

        let (previous_comparison_duration, previous_comparison_time) =
            previous_comparison_values(timer, index);

        let segment_comparison_duration = segment_comparison
            .checked_sub(previous_comparison_duration)
            .unwrap_or_default()
            .abs(); // Abs because later split might be shorter than previous

        if current_segment_index == index {
            // Current segment row
            segment_classes.push("current-segment");

            let current_duration = current_attempt_running_duration(timer);

            let diff = current_duration // Represents the time difference to comparison.
                .checked_sub(segment_comparison)
                .unwrap_or_default();

            // We will calculate how long the split has been running to either show diff or comparison
            let split_running_time = if index == 0 {
                current_duration
            } else {
                // Match original behavior: assert current > previous comparison time.
                assert!(current_duration > previous_comparison_time);
                current_duration
                    .checked_sub(previous_comparison_time)
                    .unwrap_or_default()
            };

            if diff.is_positive() && segment_comparison != TimeDuration::ZERO
                || (goldsplit_duration != TimeDuration::ZERO
                    && split_running_time >= goldsplit_duration)
            {
                value_text = format_signed(diff, config);
            }
        }

        if current_segment_index > index {
            // Past split rows
            let split_time = segment_split_time(segment, timer);

            if split_time == TimeDuration::ZERO {
                // The split was skipped
                value_text = String::from("--");
            } else {
                let diff = split_time
                    .checked_sub(segment_comparison)
                    .unwrap_or_default();

                if config.general.split_format == Some(String::from("Time")) {
                    value_text = config
                        .format
                        .split
                        .format_split_time(&segment.split_time(), timer.current_timing_method());
                } else if segment_comparison != TimeDuration::ZERO {
                    // DIFF
                    value_text = format_signed(diff, config);
                }
                if segment_comparison != TimeDuration::ZERO {
                    label_classes = classify_split_label(
                        segment_comparison_duration,
                        split_time
                            .checked_sub(previous_comparison_time)
                            .unwrap_or_default(),
                        diff,
                        goldsplit_duration,
                        false, // not running
                    );
                }
            }
        }
    }
    SplitRowData {
        title,
        value_text,
        segment_classes,
        label_classes,
    }
}

/// Calculates the CSS-like classes for a split label, based on comparison and timing math.
/// This mirrors `TimerUI::calculate_split_label_classes`, but is UI-free and testable.
pub fn classify_split_label(
    comparison_duration: TimeDuration,
    split_duration: TimeDuration, // Either split duration or current attempt duration; the running duration of the split for the current attempt
    diff: TimeDuration,
    goldsplit_duration: TimeDuration,
    running: bool, // Serves to not show gold during running splits
) -> Vec<&'static str> {
    let mut classes = Vec::new();

    // Gold split check has priority
    if (split_duration < goldsplit_duration || goldsplit_duration == TimeDuration::ZERO) && !running
    {
        classes.push("goldsplit");
        return classes;
    }

    // Ahead or behind comparison (green or red families)
    if diff.is_negative() {
        // Gaining vs losing time while ahead
        if split_duration <= comparison_duration {
            classes.push("greensplit");
        } else {
            classes.push("lostgreensplit");
        }
    } else if diff.is_positive() {
        // Gaining vs losing time while behind
        if split_duration <= comparison_duration {
            classes.push("gainedredsplit");
        } else {
            classes.push("redsplit");
        }
    }

    classes
}

// New data model for current split info used in center box
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectedSegmentInfoData {
    pub best_value: String,
    pub comparison_label: String,
    pub comparison_value: String,
}

/// Computes the textual data for the "current split info" panel:
/// - Best split value for the current segment
/// - Comparison label (e.g., "PB:")
/// - Comparison value (per-segment), adjusted by the previous segment's comparison time
pub fn compute_selected_segment_info(
    timer: &Timer,
    config: &mut Config,
    segments_list: &ListBox,
) -> SelectedSegmentInfoData {
    let segments = timer.run().segments();

    let selected_index = if timer.current_phase().is_running() {
        timer.current_split_index().unwrap_or(0)
    } else {
        // When not running, use the selected segment in the segments list if available
        segments_list
            .selected_row()
            .and_then(|row| row.index().try_into().ok())
            .unwrap_or(0)
    };

    let selected_segment = &segments[selected_index];

    let previous_comparison_time = if selected_index > 0 {
        segments[selected_index - 1]
            .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
            .unwrap_or_default()
            .to_duration()
    } else {
        TimeDuration::ZERO
    };

    let best_value_text = config.format.split.format_split_time(
        &selected_segment.best_segment_time(),
        timer.current_timing_method(),
    );

    let comparison_label_text = format!("{}:", format_label(timer.current_comparison()));

    let comparison_value_text = config.format.segment.format_segment_time(
        &selected_segment
            .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
            .unwrap_or_default()
            .to_duration()
            .checked_sub(previous_comparison_time)
            .unwrap_or_default()
            .abs(), // Abs because later split might be shorter than previous
    );

    SelectedSegmentInfoData {
        best_value: best_value_text,
        comparison_label: comparison_label_text,
        comparison_value: comparison_value_text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_gold_when_not_running_and_new_best_and_ahead() {
        let comparison = TimeDuration::seconds(10);
        let split_duration = TimeDuration::seconds(8);
        let diff = TimeDuration::seconds(-2);
        let gold = TimeDuration::seconds(9);

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            classes.contains(&"goldsplit"),
            "Expected goldsplit: got {classes:?}",
        );
    }

    #[test]
    fn classify_gold_when_not_running_and_zero_gold_duration() {
        // When gold duration is ZERO and not running, we treat it as gold (first split behavior)
        let comparison = TimeDuration::ZERO;
        let split_duration = TimeDuration::seconds(12);
        let diff = TimeDuration::ZERO;
        let gold = TimeDuration::ZERO;

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            classes.contains(&"goldsplit"),
            "Expected goldsplit when gold duration is zero and not running: got {classes:?}",
        );
    }

    #[test]
    fn classify_gainedred_when_not_running_and_behind_and_ahead_comparison() {
        let comparison = TimeDuration::seconds(10);
        let split_duration = TimeDuration::seconds(9);
        let diff = TimeDuration::seconds(1);
        let gold = TimeDuration::seconds(8);

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            classes.contains(&"gainedredsplit"),
            "Expected redsplit when behind and gaining: got {classes:?}",
        );
    }

    #[test]
    fn classify_red_when_not_running_and_behind_and_behind_comparison() {
        let comparison = TimeDuration::seconds(10);
        let split_duration = TimeDuration::seconds(11);
        let diff = TimeDuration::seconds(1);
        let gold = TimeDuration::seconds(9);

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            classes.contains(&"redsplit"),
            "Expected redsplit when behind and not gaining: got {classes:?}",
        );
    }

    #[test]
    fn classify_green_when_ahead_and_split_on_or_under_comparison_duration() {
        let comparison = TimeDuration::seconds(10);
        let split_duration = TimeDuration::seconds(9);
        let diff = TimeDuration::seconds(-1);
        let gold = TimeDuration::seconds(8);

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            classes.contains(&"greensplit"),
            "Expected greensplit when ahead and not losing against comparison_duration: got {classes:?}",
        );
    }

    #[test]
    fn classify_lost_green_when_ahead_but_split_exceeds_comparison_duration() {
        let comparison = TimeDuration::seconds(10);
        let split_duration = TimeDuration::seconds(11); // longer than comparison_duration
        let diff = TimeDuration::seconds(-1); // still ahead overall vs segment comparison target
        let gold = TimeDuration::seconds(8);

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            classes.contains(&"lostgreensplit"),
            "Expected lostgreensplit when ahead (negative diff) but split exceeds comparison_duration: got {classes:?}",

        );
    }

    #[test]
    fn classify_no_color_when_diff_is_zero() {
        let comparison = TimeDuration::seconds(10);
        let split_duration = TimeDuration::seconds(10);
        let diff = TimeDuration::ZERO;
        let gold = TimeDuration::seconds(50);

        let classes = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            !classes.iter().any(
                |c| ["greensplit", "lostgreensplit", "gainedredsplit", "redsplit"].contains(c)
            ),
            "Expected no red/green class when diff is zero: got {classes:?}",
        );
    }
}
