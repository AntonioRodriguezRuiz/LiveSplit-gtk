use crate::config::Config;
use crate::utils::comparisons::{segment_comparison_time, previous_comparison_values, segment_split_time, format_signed, best_segment_duration, classify_split_label, segment_best_time, best_comparison_values, current_attempt_running_duration, real_time_sob};

use gtk4::{CenterBox, Label, Orientation::Horizontal, prelude::WidgetExt};

use livesplit_core::Timer;

pub trait AdditionalInfo {
    fn new(timer: &Timer, config: &mut Config) -> Self
    where
        Self: Sized;
    fn update(&mut self, timer: &Timer, config: &mut Config);
    fn container(&self) -> &CenterBox;
}

pub struct PrevSegmentDiffInfo {
    container: CenterBox,
    label: Label,
    value: Label,
}

pub struct PrevSegmentBestInfo {
    container: CenterBox,
    label: Label,
    value: Label,
}

pub struct BestPossibleTimeInfo {
    container: CenterBox,
    label: Label,
    value: Label,
}

impl AdditionalInfo for PrevSegmentDiffInfo {
    fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = CenterBox::builder().orientation(Horizontal).build();

        let label = Label::builder()
            .label("Previous Segment:")
            .css_classes(["heading"])
            .build();
        let value = Label::builder().label("").css_classes(["timer"]).build();

        container.set_start_widget(Some(&label));
        container.set_end_widget(Some(&value));

        let mut res = Self {
            container,
            label,
            value,
        };

        res.update(timer, config); // Initialize with default timer state

        res
    }

    fn update(&mut self, timer: &Timer, config: &mut Config) {
        self.value.set_css_classes(&[]);
        self.value.set_label("");
        if let Some(mut index) = timer.current_split_index()
            && index > 0
        {
            index -= 1; // Previous segment index

            let segment = &timer.run().segments()[index];

            let segment_comparison_time = segment_comparison_time(segment, timer);
            let (previous_comparison_duration, previous_split_time) =
                previous_comparison_values(timer, index);
            let segment_comparison_duration = segment_comparison_time
                .checked_sub(previous_comparison_duration)
                .unwrap_or_default()
                .abs();

            let split_time = segment_split_time(segment, timer);

            if split_time == time::Duration::ZERO {
                self.value.set_label("");
            } else {
                let diff = split_time
                    .checked_sub(previous_split_time)
                    .unwrap_or_default()
                    .checked_sub(segment_comparison_duration)
                    .unwrap_or_default();

                if segment_comparison_time != time::Duration::ZERO {
                    self.value.set_label(format_signed(diff, config).as_str());

                    let gold_duration = best_segment_duration(segment, timer);
                    let split_duration = split_time
                        .checked_sub(previous_split_time)
                        .unwrap_or_default();

                    self.value.add_css_class(classify_split_label(
                        segment_comparison_duration,
                        split_duration,
                        diff,
                        gold_duration,
                        false,
                    ));
                }
            }
        } else {
            self.value.set_label("");
        }
    }

    fn container(&self) -> &CenterBox {
        &self.container
    }
}

impl AdditionalInfo for PrevSegmentBestInfo {
    fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = CenterBox::builder().orientation(Horizontal).build();

        let label = Label::builder()
            .label("Previous Segment (Best):")
            .css_classes(["heading"])
            .build();
        let value = Label::builder().label("").css_classes(["timer"]).build();

        container.set_start_widget(Some(&label));
        container.set_end_widget(Some(&value));

        let mut res = Self {
            container,
            label,
            value,
        };

        res.update(timer, config); // Initialize with default timer state

        res
    }

    fn update(&mut self, timer: &Timer, config: &mut Config) {
        self.value.set_css_classes(&[]);
        self.value.set_label("");
        if let Some(mut index) = timer.current_split_index()
            && index > 0
        {
            index -= 1; // Previous segment index

            let segment = &timer.run().segments()[index];

            let segment_best_time = segment_best_time(segment, timer);
            let (_, previous_split_time) = previous_comparison_values(timer, index);
            let (previous_best_duration, previous_best_time) = best_comparison_values(timer, index);
            let segment_best_duration = segment_best_time
                .checked_sub(previous_best_duration)
                .unwrap_or_default()
                .abs();

            let split_time = segment_split_time(segment, timer);

            if split_time == time::Duration::ZERO {
                self.value.set_label("");
            } else {
                let diff = split_time
                    .checked_sub(previous_split_time)
                    .unwrap_or_default()
                    .checked_sub(segment_best_duration)
                    .unwrap_or_default();

                if segment_best_time != time::Duration::ZERO {
                    self.value.set_label(format_signed(diff, config).as_str());

                    let gold_duration = best_segment_duration(segment, timer);
                    let split_duration = split_time
                        .checked_sub(previous_best_time)
                        .unwrap_or_default();

                    self.value.add_css_class(classify_split_label(
                        segment_best_duration,
                        split_duration,
                        diff,
                        gold_duration,
                        false,
                    ));
                }
            }
        } else {
            self.value.set_label("");
        }
    }

    fn container(&self) -> &CenterBox {
        &self.container
    }
}

impl AdditionalInfo for BestPossibleTimeInfo {
    fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = CenterBox::builder().orientation(Horizontal).build();

        let label = Label::builder()
            .label("Best Possible Time:")
            .css_classes(["heading"])
            .build();
        let value = Label::builder().label("").css_classes(["timer"]).build();

        container.set_start_widget(Some(&label));
        container.set_end_widget(Some(&value));

        let mut res = Self {
            container,
            label,
            value,
        };

        res.update(timer, config); // Initialize with default timer state

        res
    }

    fn update(&mut self, timer: &Timer, config: &mut Config) {
        if timer.current_phase().is_not_running() {
            self.value.set_label("");
        } else if timer.current_phase().is_running() || timer.current_phase().is_paused() {
            let segment = timer.current_split().unwrap_or(timer.run().segment(0));

            let segment_best_duration = segment_best_time(segment, timer);

            // Diff to SOB
            let diff = current_attempt_running_duration(timer)
                .checked_sub(segment_best_duration)
                .unwrap_or_default();

            // We will be adding only diff time to the best possible time when we are behind
            let live_addition = if diff.is_positive() {
                diff
            } else {
                time::Duration::ZERO
            };

            let best_possible_time = real_time_sob(timer)
                .checked_add(live_addition)
                .unwrap_or_default();
            if best_possible_time == time::Duration::ZERO {
                self.value.set_label("");
            } else {
                self.value.set_label(
                    config
                        .format
                        .segment
                        .format_duration(&best_possible_time)
                        .as_str(),
                );
            }
        } else if timer.current_phase().is_ended() {
            self.value.set_label(
                config
                    .format
                    .segment
                    .format_duration(&current_attempt_running_duration(timer))
                    .as_str(),
            );
        }
    }

    fn container(&self) -> &CenterBox {
        &self.container
    }
}
