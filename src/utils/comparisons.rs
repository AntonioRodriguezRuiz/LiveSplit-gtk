use crate::config::Config;
use livesplit_core::{Timer, analysis::sum_of_segments::best::calculate as calculate_sob};

pub fn current_attempt_running_duration(timer: &Timer) -> time::Duration {
    use livesplit_core::TimingMethod;
    let current_dur = timer
        .current_attempt_duration()
        .to_duration()
        .checked_add(timer.run().offset().to_duration())
        .unwrap_or_default();

    let paused_time = timer.get_pause_time().unwrap_or_default().to_duration();

    let loading_times = if timer.current_timing_method() == TimingMethod::GameTime {
        timer.loading_times().to_duration()
    } else {
        time::Duration::ZERO
    };

    current_dur
        .checked_sub(paused_time)
        .unwrap_or_default()
        .checked_sub(loading_times)
        .unwrap_or_default()
}

pub fn real_time_sob(timer: &Timer) -> time::Duration {
    let mut predictions = vec![None; timer.run().len() + 1];
    let predictions = &mut predictions[..];
    calculate_sob(
        timer.run().segments(),
        predictions,
        false,
        true,
        timer.current_timing_method(),
    )
    .unwrap_or_default()
    .to_duration()
}

pub fn best_segment_duration(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
    use livesplit_core::TimingMethod;
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

pub fn segment_split_time(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
    use livesplit_core::TimingMethod;
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

pub fn segment_best_time(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
    segment
        .comparison_timing_method("Best Segments", timer.current_timing_method())
        .unwrap_or_default()
        .to_duration()
}

pub fn segment_comparison_time(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
    segment
        .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
        .unwrap_or_default()
        .to_duration()
}

pub fn previous_comparison_values(timer: &Timer, index: usize) -> (time::Duration, time::Duration) {
    use livesplit_core::TimingMethod;
    let segments = timer.run().segments();
    if index > 0 {
        let prev = &segments[index - 1];
        let prev_comp_duration = prev
            .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
            .unwrap_or_default()
            .to_duration();
        let prev_split_time = if timer.current_timing_method() == TimingMethod::GameTime {
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
        (prev_comp_duration, prev_split_time)
    } else {
        (time::Duration::ZERO, time::Duration::ZERO)
    }
}

pub fn best_comparison_values(timer: &Timer, index: usize) -> (time::Duration, time::Duration) {
    use livesplit_core::TimingMethod;
    let segments = timer.run().segments();
    if index > 0 {
        let prev = &segments[index - 1];
        let prev_best_duration = prev
            .comparison_timing_method("Best Segments", timer.current_timing_method())
            .unwrap_or_default()
            .to_duration();
        let prev_split_time = if timer.current_timing_method() == TimingMethod::GameTime {
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
        (prev_best_duration, prev_split_time)
    } else {
        (time::Duration::ZERO, time::Duration::ZERO)
    }
}

pub fn format_signed(diff: time::Duration, config: &mut Config) -> String {
    let sign = if diff.is_positive() {
        "+"
    } else if diff.is_negative() {
        "-"
    } else {
        "~"
    };
    let abs = diff.abs();
    let formatted = config.format.split.format_segment_time(&abs);
    format!("{sign}{formatted}")
}

pub fn classify_split_label(
    comparison_duration: time::Duration,
    split_duration: time::Duration,
    diff: time::Duration,
    goldsplit_duration: time::Duration,
    running: bool,
) -> &'static str {
    if running {
        return "";
    }
    if split_duration < goldsplit_duration || goldsplit_duration == time::Duration::ZERO {
        "goldsplit"
    } else if diff.is_negative() {
        if split_duration <= comparison_duration {
            "greensplit"
        } else {
            "lostgreensplit"
        }
    } else if diff.is_positive() {
        if split_duration <= comparison_duration {
            "gainedredsplit"
        } else {
            "redsplit"
        }
    } else {
        "" // how
    }
}

pub fn previous_split_combined_gold_and_prev_comparison(
    timer: &Timer,
    index: usize,
) -> (time::Duration, time::Duration, time::Duration) {
    let segments = timer.run().segments();
    let mut last_non_skipped: Option<usize> = None;
    if index > 0 {
        for k in (0..index).rev() {
            if segment_split_time(&segments[k], timer) != time::Duration::ZERO {
                last_non_skipped = Some(k);
                break;
            }
        }
    }

    // Combined gold must include the current segment and any directly previous skipped ones
    // until the last non-skipped, or the beginning.
    let start = last_non_skipped.map_or(0, |k| k + 1);
    let mut combined_gold = time::Duration::ZERO;
    for k in start..=index {
        combined_gold = combined_gold
            .checked_add(best_segment_duration(&segments[k], timer))
            .unwrap_or_default();
    }

    // The previous split time is either the last non-skipped split time, or ZERO if none.
    let previous_split_time = last_non_skipped.map_or(time::Duration::ZERO, |k| {
        segment_split_time(&segments[k], timer)
    });

    let previous_comparison_duration = last_non_skipped.map_or(time::Duration::ZERO, |k| {
        segment_comparison_time(&segments[k], timer)
    });

    (
        previous_split_time,
        combined_gold,
        previous_comparison_duration,
    )
}

#[cfg(test)]
mod classify_split_labels_tests {
    use super::*;
    use time::Duration;

    #[test]
    fn classify_gold_when_first_split() {
        let comparison = Duration::ZERO;
        let split_duration = Duration::seconds(8);
        let diff = Duration::ZERO;
        let gold = Duration::ZERO;

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(class == "goldsplit", "Expected goldsplit: got {class:?}",);
    }

    #[test]
    fn classify_gold_when_not_running_and_new_best_and_ahead() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(8);
        let diff = Duration::seconds(-2);
        let gold = Duration::seconds(9);

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(class == "goldsplit", "Expected goldsplit: got {class:?}",);
    }

    #[test]
    fn classify_gold_when_not_running_and_zero_gold_duration() {
        // When gold duration is ZERO and not running, we treat it as gold (first split behavior)
        let comparison = Duration::ZERO;
        let split_duration = Duration::seconds(12);
        let diff = Duration::ZERO;
        let gold = Duration::ZERO;

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class == "goldsplit",
            "Expected goldsplit when gold duration is zero and not running: got {class:?}",
        );
    }

    #[test]
    fn classify_gainedred_when_not_running_and_behind_and_ahead_comparison() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(9);
        let diff = Duration::seconds(1);
        let gold = Duration::seconds(8);

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class == "gainedredsplit",
            "Expected redsplit when behind and gaining: got {class:?}",
        );
    }

    #[test]
    fn classify_red_when_not_running_and_behind_and_behind_comparison() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(11);
        let diff = Duration::seconds(1);
        let gold = Duration::seconds(9);

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class == "redsplit",
            "Expected redsplit when behind and not gaining: got {class:?}",
        );
    }

    #[test]
    fn classify_green_when_ahead_and_split_on_or_under_comparison_duration() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(9);
        let diff = Duration::seconds(-1);
        let gold = Duration::seconds(8);

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class == "greensplit",
            "Expected greensplit when ahead and not losing against comparison_duration: got {class:?}",
        );
    }

    #[test]
    fn classify_lost_green_when_ahead_but_split_exceeds_comparison_duration() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(11); // longer than comparison_duration
        let diff = Duration::seconds(-1); // still ahead overall vs segment comparison target
        let gold = Duration::seconds(8);

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class == "lostgreensplit",
            "Expected lostgreensplit when ahead (negative diff) but split exceeds comparison_duration: got {class:?}",
        );
    }

    #[test]
    fn classify_no_color_when_diff_is_zero() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(10);
        let diff = Duration::ZERO;
        let gold = Duration::seconds(5);

        let class = classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class.is_empty(),
            "Expected no red/green class when diff is zero: got {class:?}",
        );
    }
}

#[cfg(test)]
mod skipped_segments_context_tests {
    use super::*;
    use livesplit_core::{Run, Segment, Time, TimeSpan, Timer};
    use time::Duration;

    fn time_rt(seconds: i64) -> Time {
        Time::new().with_real_time(Some(TimeSpan::from_seconds(seconds as f64)))
    }

    #[test]
    fn index_1_with_prev_0_skipped_prev_time_zero_and_gold_is_prev_plus_current() {
        // Setup: 2 segments, segment 0 skipped (no split), segment 1 current.
        // Golds: s0 = 1s, s1 = 2s
        // PB Split Times (cumulative): S1 PB = 25s (S0 skipped so no PB split set for it)
        let mut run = Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");

        let mut s0 = Segment::new("S0");
        s0.set_best_segment_time(time_rt(1));
        // Skipped: leave split time empty (ZERO)

        let mut s1 = Segment::new("S1");
        s1.set_best_segment_time(time_rt(2));
        s1.set_personal_best_split_time(time_rt(25)); // PB cumulative time at segment 1

        run.push_segment(s0);
        run.push_segment(s1);

        let timer = Timer::new(run).expect("timer");

        let (prev_split_time, combined_gold, prev_comp) =
            previous_split_combined_gold_and_prev_comparison(&timer, 1);

        assert_eq!(
            prev_split_time,
            Duration::ZERO,
            "previous_split_time must be ZERO when the previous segment was skipped"
        );
        assert_eq!(
            prev_comp,
            Duration::ZERO,
            "previous comparison must be ZERO when there is no previous non-skipped segment"
        );
        assert_eq!(
            combined_gold,
            Duration::seconds(1 + 2),
            "combined_gold must equal current gold + previous skipped gold"
        );
        // Verify segment comparison duration (PB current segment cumulative since previous skipped)
        let seg_comp_time = segment_comparison_time(&timer.run().segments()[1], &timer);
        let seg_duration = seg_comp_time.checked_sub(prev_comp).unwrap_or_default();
        assert_eq!(
            seg_duration,
            Duration::seconds(25),
            "Segment comparison duration should equal PB split time of current (25s) when previous is skipped"
        );
    }

    #[test]
    fn index_2_with_prev_0_and_1_skipped_prev_time_zero_and_gold_is_sum_of_all_three() {
        // Setup: 3 segments, segments 0 and 1 skipped
        // Golds: s0 = 1s, s1 = 2s, s2 = 3s
        // PB Split Times (cumulative): only S2 PB set = 55s (S0 & S1 skipped so no PB split set)
        let mut run = Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");

        let mut s0 = Segment::new("S0");
        s0.set_best_segment_time(time_rt(1));
        // skipped

        let mut s1 = Segment::new("S1");
        s1.set_best_segment_time(time_rt(2));
        // skipped

        let mut s2 = Segment::new("S2");
        s2.set_best_segment_time(time_rt(3));
        s2.set_personal_best_split_time(time_rt(55)); // cumulative PB time at segment 2

        run.push_segment(s0);
        run.push_segment(s1);
        run.push_segment(s2);

        let timer = Timer::new(run).expect("timer");

        let (prev_split_time, combined_gold, prev_comp) =
            previous_split_combined_gold_and_prev_comparison(&timer, 2);

        assert_eq!(
            prev_split_time,
            Duration::ZERO,
            "previous_split_time must be ZERO when all previous segments were skipped"
        );
        assert_eq!(
            prev_comp,
            Duration::ZERO,
            "previous comparison must be ZERO when there is no previous non-skipped segment"
        );
        assert_eq!(
            combined_gold,
            Duration::seconds(1 + 2 + 3),
            "combined_gold must equal the sum of golds for segments 0..=2"
        );
        // Verify segment comparison duration (PB cumulative up to current since all previous skipped)
        let seg_comp_time = segment_comparison_time(&timer.run().segments()[2], &timer);
        let seg_duration = seg_comp_time.checked_sub(prev_comp).unwrap_or_default();
        assert_eq!(
            seg_duration,
            Duration::seconds(55),
            "Segment comparison duration should equal PB split time of current (55s) when all previous are skipped"
        );
    }

    #[test]
    fn index_2_with_only_prev_1_skipped_prev_is_split0_and_gold_is_seg1_plus_current() {
        // Setup: 3 segments, only segment 1 skipped, segment 0 not skipped (has a split time)
        // Golds: s0 = 1s, s1 = 2s, s2 = 3s
        // Split times: s0 split = 10s (non-zero), s1 split = ZERO (skipped)
        // PB Split Times (cumulative): S0 PB = 10s, S2 PB = 55s (so duration across skipped S1 + current = 45s)
        let mut run = Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");

        let mut s0 = Segment::new("S0");
        s0.set_best_segment_time(time_rt(1));
        s0.set_personal_best_split_time(time_rt(10));
        s0.set_split_time(time_rt(10)); // non-skipped

        let mut s1 = Segment::new("S1");
        s1.set_best_segment_time(time_rt(2));
        // skipped: leave split time empty (ZERO)

        let mut s2 = Segment::new("S2");
        s2.set_best_segment_time(time_rt(3));
        s2.set_personal_best_split_time(time_rt(55));

        run.push_segment(s0);
        run.push_segment(s1);
        run.push_segment(s2);

        let timer = Timer::new(run).expect("timer");

        let (prev_split_time, combined_gold, prev_comp) =
            previous_split_combined_gold_and_prev_comparison(&timer, 2);

        assert_eq!(
            prev_split_time,
            Duration::seconds(10),
            "previous_split_time must be the last non-skipped (segment 0) split time"
        );
        assert_eq!(
            prev_comp,
            Duration::seconds(10),
            "previous comparison must equal the comparison split at the last non-skipped segment"
        );
        assert_eq!(
            combined_gold,
            Duration::seconds(2 + 3),
            "combined_gold must equal gold of skipped segment 1 plus current segment 2"
        );
        // Verify segment comparison duration:
        // PB current split (55) - PB last non-skipped split (10) = 45
        let seg_comp_time = segment_comparison_time(&timer.run().segments()[2], &timer);
        let seg_duration = seg_comp_time.checked_sub(prev_comp).unwrap_or_default();
        assert_eq!(
            seg_duration,
            Duration::seconds(45),
            "Segment comparison duration should equal PB cumulative current (55) - previous non-skipped (10) = 45s"
        );
    }
}
