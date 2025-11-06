use crate::config::Config;

use adw::prelude::ActionRowExt as _;
use adw::ActionRow;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{
    Adjustment, Align, Box as GtkBox, Label, ListBox, Orientation, ScrolledWindow, SelectionMode,
};

use livesplit_core::{Timer, TimerPhase};

use tracing::debug;

/// The body of the Timer UI:
///
/// It owns a vertical container and a `SegmentList` that renders the splits.
pub struct TimerBody {
    container: GtkBox,
    segment_list: SegmentList,
}

impl TimerBody {
    pub fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = GtkBox::builder().orientation(Orientation::Vertical).build();

        let segment_list = SegmentList::new(timer, config);
        container.append(segment_list.container());

        Self {
            container,
            segment_list,
        }
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    pub fn list(&self) -> &ListBox {
        self.segment_list.list()
    }

    pub fn refresh(&mut self, timer: &Timer, config: &mut Config) {
        self.segment_list.update(timer, config);
    }
}

/// Component responsible of rendering, managing, and updating the list of segments/splits.
pub struct SegmentList {
    container: GtkBox,
    scroller: ScrolledWindow,
    list: ListBox,
    rows: Vec<SegmentRow>,
    last_phase: TimerPhase,
    last_comparison: String,
    last_splits_key: Option<String>,
}

impl SegmentList {
    pub fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(false)
            .vexpand(false)
            .spacing(0)
            .css_classes(["splits-container"])
            .build();

        let height_request = SegmentList::compute_scroller_height(timer, config);

        let scroller = ScrolledWindow::builder()
            .hexpand(false)
            .vexpand(false)
            .min_content_height(SegmentRow::get_natural_height())
            .height_request(height_request)
            .kinetic_scrolling(true)
            .build();

        let list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .css_classes(["split-boxed-list"])
            .build();

        container.append(&scroller);
        scroller.set_child(Some(&list));

        let mut this = Self {
            container,
            scroller,
            list,
            rows: Vec::new(),
            last_phase: timer.current_phase(),
            last_comparison: timer.current_comparison().to_owned(),
            last_splits_key: config
                .general
                .splits
                .as_ref()
                .map(|p| p.to_string_lossy().to_string()),
        };
        this.build_rows(timer, config);
        this.list.unselect_all();
        this
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    pub fn list(&self) -> &ListBox {
        &self.list
    }

    pub fn update(&mut self, timer: &Timer, config: &mut Config) {
        // Detect structural changes or comparison/splits changes that force a full rebuild.
        let phase = timer.current_phase();
        let comp_changed = self.last_comparison.as_str() != timer.current_comparison();
        let splits_key_current = config
            .general
            .splits
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
        let splits_changed = self.last_splits_key != splits_key_current;
        let count_changed = self.rows.len() != timer.run().segments().len();
        let phase_changed = self.last_phase != phase;

        let selected_index = self.get_selected_row_index(timer, phase);

        if comp_changed || splits_changed || count_changed || phase_changed {
            self.rebuild_rows(timer, config);
        } else if phase.is_running() {
            self.update_scroll_position(timer, config);
            self.update_rows_minimal(timer, config);
        }

        if phase_changed {
            if phase.is_not_running() {
                // Go to the beggining of the split list after a reset
                self.update_scroll_position(timer, config);
            }
            self.update_selection_policy(timer, phase, selected_index);
        }

        self.last_phase = phase;
        self.last_comparison = timer.current_comparison().to_string();
        self.last_splits_key = splits_key_current;
    }

    fn update_scroll_position(&mut self, timer: &Timer, config: &Config) {
        let mut adjustment = self.scroller.vadjustment();

        if let Some(cur) = timer.current_split_index() {
            let follow_from = config.style.segments_scroll_follow_from.unwrap_or(7);
            let y = SegmentRow::get_natural_height() * (cur as i32 + 1 - follow_from as i32);

            if let Some(row) = self.list.row_at_index(cur as i32) {
                adjustment.set_value(if cur >= follow_from { y as f64 } else { 0.0 });
            }
        } else {
            adjustment.set_value(0.0);
        }

        self.scroller.set_vadjustment(Some(&adjustment));
    }

    fn get_selected_row_index(&mut self, timer: &Timer, phase: TimerPhase) -> Option<i32> {
        let mut selected_index: Option<i32> = None;
        if self.last_phase != phase {
            for (index, _) in timer.run().segments().iter().enumerate() {
                if let Some(row) = self.list.row_at_index(index as i32) {
                    if row.is_selected() {
                        selected_index = Some(index as i32);
                    }
                }
            }
        }
        selected_index
    }

    fn update_rows_minimal(&mut self, timer: &Timer, config: &mut Config) {
        if let Some(cur) = timer.current_split_index() {
            let len = timer.run().segments().len();

            // Avoid rerendering twice
            let mut indices_vec = vec![cur.saturating_sub(1), cur, cur.saturating_add(1)];
            indices_vec.sort_unstable();
            indices_vec.dedup();
            for i in indices_vec {
                if i < len {
                    if let Some(row) = self.rows.get_mut(i) {
                        let seg = &timer.run().segments()[i];
                        row.refresh(timer, config, Some(cur), i, seg);
                    }
                }
            }
        }
    }

    fn update_selection_policy(
        &mut self,
        timer: &Timer,
        phase: TimerPhase,
        selected_index: Option<i32>,
    ) {
        match phase {
            TimerPhase::Running | TimerPhase::Paused => {
                self.list.set_selection_mode(SelectionMode::None);
                self.list.unselect_all();
            }
            TimerPhase::Ended => {
                self.list.set_selection_mode(SelectionMode::Single);
                let last_index = timer.run().segments().len().saturating_sub(1) as i32;
                let idx = selected_index.unwrap_or(last_index);
                if let Some(row) = self.list.row_at_index(idx) {
                    self.list.select_row(Some(&row));
                }
            }
            _ => {
                self.list.set_selection_mode(SelectionMode::Single);
                self.list.unselect_all();
            }
        }
    }

    fn rebuild_rows(&mut self, timer: &Timer, config: &mut Config) {
        // Clear GTK children and local row cache
        self.container.remove(&self.container.last_child().unwrap());
        self.build_rows(timer, config);
    }

    fn build_rows(&mut self, timer: &Timer, config: &mut Config) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        self.rows.clear();

        // Create new rows once and append references to the ListBox
        let opt_current_segment_index = timer.current_split_index();
        for (index, segment) in timer.run().segments().iter().enumerate() {
            let row = SegmentRow::new(timer, config, opt_current_segment_index, index, segment);
            // Last segment will always be visible, so we render it separately
            if index < timer.run().len() - 1 {
                self.list.append(row.row());
            } else {
                let last_split_list = ListBox::builder()
                    .selection_mode(SelectionMode::Single)
                    .css_classes(["last-split-boxed-list"])
                    .build();
                last_split_list.append(row.row());
                self.container.append(&last_split_list);
                row.row().set_activatable(true);

                // Handle selecting and deselecting manually
                // let row_clone = row.row().clone();
                // let list_weak = self.list.downgrade();
                // row.row().connect_activated(move |_| {
                //     if let Some(list_ref) = list_weak.upgrade() {
                //         list_ref.unselect_all();
                //     }
                //     row_clone.add_css_class("selected");
                // });
                // let row_weak = row.row().downgrade();
                // self.list.connect_row_selected(move |_, _| {
                //     if let Some(row_ref) = row_weak.upgrade() {
                //         row_ref.remove_css_class("selected");
                //     }
                // });
            }
            self.rows.push(row);
        }

        // Recompute scroller height if splits changed
        if self.last_splits_key
            != config
                .general
                .splits
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
        {
            let height_request = SegmentList::compute_scroller_height(timer, config);
            self.scroller.set_height_request(height_request);
        }

        // Refresh caches
        self.last_phase = timer.current_phase();
        self.last_comparison = timer.current_comparison().to_string();
        self.last_splits_key = config
            .general
            .splits
            .as_ref()
            .map(|p| p.to_string_lossy().to_string());
    }

    fn compute_scroller_height(timer: &Timer, config: &mut Config) -> i32 {
        let segments_requested = config.style.max_segments_displayed.unwrap_or(10);

        let height_request = if segments_requested <= timer.run().len() - 1 {
            SegmentRow::get_natural_height() * segments_requested as i32
        } else {
            SegmentRow::get_natural_height() * (timer.run().len() as i32 - 1)
        };
        height_request
    }
}

// SegmentRow: wraps a row widget and its value label so we can refresh without touching the ListBox
pub struct SegmentRow {
    row: ActionRow,
    value_label: Label,
}

impl SegmentRow {
    pub fn row(&self) -> &ActionRow {
        &self.row
    }

    pub fn new(
        timer: &Timer,
        config: &mut Config,
        opt_current_segment_index: Option<usize>,
        index: usize,
        segment: &livesplit_core::Segment,
    ) -> Self {
        let segment_comparison_time = Self::segment_comparison_time(segment, timer);
        let (previous_comparison_duration, previous_comparison_time) =
            Self::previous_comparison_values(timer, index);
        let segment_comparison_duration = segment_comparison_time
            .checked_sub(previous_comparison_duration)
            .unwrap_or_default()
            .abs();

        let title = segment.name().to_owned();
        let row = ActionRow::builder().title(&title).build();

        let label = Label::builder()
            .halign(Align::Center)
            .valign(Align::Center)
            .css_classes(["timer"])
            .build();

        let mut value_text = config.format.split.format_split_time(
            &segment.comparison(timer.current_comparison()),
            timer.current_timing_method(),
        );

        SegmentRow::compute_segment(
            timer,
            config,
            opt_current_segment_index,
            index,
            segment,
            segment_comparison_time,
            previous_comparison_time,
            segment_comparison_duration,
            &row,
            &label,
            &mut value_text,
        );

        label.set_label(&value_text);
        row.add_suffix(&label);

        // Add no transition for more responsive updates
        row.add_css_class("no-transition");

        Self {
            row,
            value_label: label,
        }
    }

    pub fn refresh(
        &mut self,
        timer: &Timer,
        config: &mut Config,
        opt_current_segment_index: Option<usize>,
        index: usize,
        segment: &livesplit_core::Segment,
    ) {
        // Recompute values
        let mut value_text = config.format.split.format_split_time(
            &segment.comparison(timer.current_comparison()),
            timer.current_timing_method(),
        );

        // Reset dynamic classes
        self.value_label.set_css_classes(&["timer"]);
        self.row.remove_css_class("current-segment");

        let segment_comparison_time = Self::segment_comparison_time(segment, timer);
        let (previous_comparison_duration, previous_comparison_time) =
            Self::previous_comparison_values(timer, index);
        let segment_comparison_duration = segment_comparison_time
            .checked_sub(previous_comparison_duration)
            .unwrap_or_default()
            .abs();

        SegmentRow::compute_segment(
            timer,
            config,
            opt_current_segment_index,
            index,
            segment,
            segment_comparison_time,
            previous_comparison_time,
            segment_comparison_duration,
            &self.row,
            &self.value_label,
            &mut value_text,
        );

        // Apply recomputed label (avoid churn if unchanged)
        if self.value_label.label().as_str() != value_text {
            self.value_label.set_label(&value_text);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_segment(
        timer: &Timer,
        config: &mut Config,
        opt_current_segment_index: Option<usize>,
        index: usize,
        segment: &livesplit_core::Segment,
        segment_comparison_time: time::Duration,
        previous_comparison_time: time::Duration,
        segment_comparison_duration: time::Duration,
        row: &ActionRow,
        label: &Label,
        value_text: &mut String,
    ) {
        if let Some(current_segment_index) = opt_current_segment_index {
            if current_segment_index > index {
                SegmentRow::compute_passed_segment(
                    timer,
                    config,
                    segment,
                    segment_comparison_time,
                    previous_comparison_time,
                    segment_comparison_duration,
                    label,
                    value_text,
                );
            }

            if current_segment_index == index {
                SegmentRow::compute_current_segment(
                    timer,
                    config,
                    index,
                    segment,
                    segment_comparison_time,
                    previous_comparison_time,
                    row,
                    value_text,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_passed_segment(
        timer: &Timer,
        config: &mut Config,
        segment: &livesplit_core::Segment,
        segment_comparison_time: time::Duration,
        previous_comparison_time: time::Duration,
        segment_comparison_duration: time::Duration,
        label: &Label,
        value_text: &mut String,
    ) {
        let split_time = Self::segment_split_time(segment, timer);

        if split_time == time::Duration::ZERO {
            *value_text = String::from("--");
        } else {
            let diff = split_time
                .checked_sub(segment_comparison_time)
                .unwrap_or_default();

            if config.style.split_format == Some(String::from("Time")) {
                *value_text = config
                    .format
                    .split
                    .format_split_time(&segment.split_time(), timer.current_timing_method());
            } else if segment_comparison_time != time::Duration::ZERO {
                *value_text = Self::format_signed(diff, config);
            }

            if segment_comparison_time != time::Duration::ZERO {
                let gold_duration = Self::best_segment_duration(segment, timer);
                let split_duration = split_time
                    .checked_sub(previous_comparison_time)
                    .unwrap_or_default();

                label.add_css_class(Self::classify_split_label(
                    segment_comparison_duration,
                    split_duration,
                    diff,
                    gold_duration,
                    false,
                ));
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_current_segment(
        timer: &Timer,
        config: &mut Config,
        index: usize,
        segment: &livesplit_core::Segment,
        segment_comparison_time: time::Duration,
        previous_comparison_time: time::Duration,
        row: &ActionRow,
        value_text: &mut String,
    ) {
        row.add_css_class("current-segment");

        let current_duration = Self::current_attempt_running_duration(timer);
        let diff = current_duration
            .checked_sub(segment_comparison_time)
            .unwrap_or_default();

        let split_running_time = if index == 0 {
            current_duration
        } else if current_duration > previous_comparison_time {
            current_duration
                .checked_sub(previous_comparison_time)
                .unwrap_or_default()
        } else {
            time::Duration::ZERO
        };

        let gold_duration = Self::best_segment_duration(segment, timer);
        if segment_comparison_time != time::Duration::ZERO
            && (diff.is_positive()
                || (gold_duration != time::Duration::ZERO && split_running_time >= gold_duration))
        {
            *value_text = Self::format_signed(diff, config);
        }
    }

    fn current_attempt_running_duration(timer: &Timer) -> time::Duration {
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

    fn best_segment_duration(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
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

    fn get_natural_height() -> i32 {
        // We create an action row and measure its natural height
        let row = ActionRow::builder().title("Test").build();
        let monospace_label = Label::builder()
            .label("00:00:00")
            .css_classes(["timer", "monospace"])
            .build();
        row.add_suffix(&monospace_label);
        row.measure(gtk4::Orientation::Vertical, -1).0 + 5 // Account for padding
    }

    fn segment_split_time(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
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

    fn segment_comparison_time(segment: &livesplit_core::Segment, timer: &Timer) -> time::Duration {
        segment
            .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
            .unwrap_or_default()
            .to_duration()
    }

    fn previous_comparison_values(timer: &Timer, index: usize) -> (time::Duration, time::Duration) {
        use livesplit_core::TimingMethod;
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
            (time::Duration::ZERO, time::Duration::ZERO)
        }
    }

    fn format_signed(diff: time::Duration, config: &mut Config) -> String {
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

    fn classify_split_label(
        comparison_duration: time::Duration,
        split_duration: time::Duration,
        diff: time::Duration,
        goldsplit_duration: time::Duration,
        running: bool,
    ) -> &'static str {
        if (split_duration < goldsplit_duration || goldsplit_duration == time::Duration::ZERO)
            && !running
        {
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
}

#[cfg(test)]
mod segment_row_ui_tests {
    use super::*;
    use adw::prelude::*;
    use gtk4;
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn gtk_test_init() {
        INIT.call_once(|| {
            gtk4::init().expect("Failed to init GTK");
            let _ = adw::init();
        });
    }

    #[gtk4::test]
    fn segment_row_sets_title_and_no_current_class_when_none() {
        gtk_test_init();

        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.push_segment(livesplit_core::Segment::new("Split A"));
        let timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let segment = &timer.run().segments()[0];
        let row = SegmentRow::new(&timer, &mut config, None, 0, segment);

        assert_eq!(row.row().title().as_str(), "Split A");
        assert!(
            !row.row().has_css_class("current-segment"),
            "Expected no current-segment class"
        );
    }

    #[gtk4::test]
    fn segment_row_applies_current_segment_class_when_current() {
        gtk_test_init();

        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.push_segment(livesplit_core::Segment::new("Split A"));
        let timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let segment = &timer.run().segments()[0];
        let row = SegmentRow::new(&timer, &mut config, Some(0), 0, segment);

        assert_eq!(row.row().title().as_str(), "Split A");
        assert!(
            row.row().has_css_class("current-segment"),
            "Expected current-segment class"
        );
    }
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

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(class == "goldsplit", "Expected goldsplit: got {class:?}",);
    }

    #[test]
    fn classify_gold_when_not_running_and_new_best_and_ahead() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(8);
        let diff = Duration::seconds(-2);
        let gold = Duration::seconds(9);

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(class == "goldsplit", "Expected goldsplit: got {class:?}",);
    }

    #[test]
    fn classify_gold_when_not_running_and_zero_gold_duration() {
        // When gold duration is ZERO and not running, we treat it as gold (first split behavior)
        let comparison = Duration::ZERO;
        let split_duration = Duration::seconds(12);
        let diff = Duration::ZERO;
        let gold = Duration::ZERO;

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
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

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
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

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
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

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
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

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class=="lostgreensplit",
            "Expected lostgreensplit when ahead (negative diff) but split exceeds comparison_duration: got {class:?}",

        );
    }

    #[test]
    fn classify_no_color_when_diff_is_zero() {
        let comparison = Duration::seconds(10);
        let split_duration = Duration::seconds(10);
        let diff = Duration::ZERO;
        let gold = Duration::seconds(5);

        let class = SegmentRow::classify_split_label(comparison, split_duration, diff, gold, false);
        assert!(
            class.is_empty(),
            "Expected no red/green class when diff is zero: got {class:?}",
        );
    }
}
