use crate::config::Config;
use crate::utils::comparisons::*;

use adw::ActionRow;
use adw::prelude::ActionRowExt as _;
use glib::Propagation;
use gtk4::{
    Align, Box as GtkBox, EventControllerKey, Label, ListBox, Orientation, ScrolledWindow,
    SelectionMode, gdk,
};
use gtk4::{CenterBox, prelude::*};

use livesplit_core::{Timer, TimerPhase, analysis::delta::calculate as calculate_delta};

/// The body of the Timer UI:
///
/// It owns a vertical container and a `SegmentList` that renders the splits.
pub struct TimerBody {
    container: GtkBox,
    segment_list: SegmentList,
}

impl TimerBody {
    pub fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .build();

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

    pub fn last_segment_list(&self) -> &ListBox {
        self.segment_list.last_segment_list()
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
    last_segment_list: ListBox,
    rows: Vec<SegmentRow>,
    last_phase: TimerPhase,
    last_comparison: String,
    last_splits_key: Option<String>,
}

impl SegmentList {
    pub fn new(timer: &Timer, config: &mut Config) -> Self {
        let container = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .hexpand(true)
            .vexpand(false)
            .spacing(0)
            .css_classes(["splits-container", "no-background"])
            .build();

        let height_request = SegmentList::compute_scroller_height(timer, config);

        let scroller = ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(false)
            .min_content_height(SegmentRow::get_natural_height())
            .height_request(height_request)
            .css_classes(["no-background"])
            .kinetic_scrolling(true)
            .build();

        let list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .hexpand(true)
            .css_classes(["split-boxed-list", "no-background"])
            .build();
        let last_segment_list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .hexpand(true)
            .css_classes(["last-split-boxed-list", "no-background"])
            .build();

        container.append(&scroller);
        container.append(&last_segment_list);
        scroller.set_child(Some(&list));

        let mut this = Self {
            container,
            scroller,
            list,
            last_segment_list,
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
        this.enable_multilateral_selection();
        this
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    pub fn list(&self) -> &ListBox {
        &self.list
    }

    pub fn last_segment_list(&self) -> &ListBox {
        &self.last_segment_list
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

        let selected_index = self.get_selected_row_index();

        if comp_changed || splits_changed || count_changed || phase_changed {
            self.rebuild_rows(timer, config);
        } else if phase.is_running() {
            self.update_scroll_position(timer, config);
            self.update_rows_minimal(timer, config);
        }

        if comp_changed
            && let Some(index) = selected_index
            && let Some(row) = self.list.row_at_index(index)
        {
            self.list.grab_focus();
            self.list.select_row(Some(&row));
        }

        if phase_changed {
            if phase.is_not_running() {
                // Go to the beggining of the split list after a reset
                self.update_scroll_position(timer, config);
            } else if phase.is_ended() {
                self.last_segment_list.grab_focus();
                self.last_segment_list
                    .select_row(Some(&self.last_segment_list.row_at_index(0).unwrap()));
            }
            self.update_selection_policy(phase);
        }

        self.last_phase = phase;
        self.last_comparison = timer.current_comparison().to_string();
        self.last_splits_key = splits_key_current;

        // Update scroller height request
        let height_request = SegmentList::compute_scroller_height(timer, config);
        self.scroller.set_height_request(height_request);
    }

    fn update_scroll_position(&mut self, timer: &Timer, config: &Config) {
        let adjustment = self.scroller.vadjustment();

        if let Some(cur) = timer.current_split_index() {
            let follow_from = config.style.segments_scroll_follow_from.unwrap_or(7);
            let y = SegmentRow::get_natural_height() * (cur as i32 + 1 - follow_from as i32);

            if self.list.row_at_index(cur as i32).is_some() {
                adjustment.set_value(if cur >= follow_from {
                    f64::from(y)
                } else {
                    0.0
                });
            }
        } else {
            adjustment.set_value(0.0);
        }

        self.scroller.set_vadjustment(Some(&adjustment));
    }

    fn get_selected_row_index(&mut self) -> Option<i32> {
        self.list.selected_row().map(|row| row.index())
    }

    fn update_rows_minimal(&mut self, timer: &Timer, config: &mut Config) {
        if let Some(cur) = timer.current_split_index() {
            let len = timer.run().segments().len();

            // Avoid rerendering twice
            let mut indices_vec = vec![cur.saturating_sub(1), cur, cur.saturating_add(1)];
            indices_vec.sort_unstable();
            indices_vec.dedup();
            for i in indices_vec {
                if i < len
                    && let Some(row) = self.rows.get_mut(i)
                {
                    let seg = &timer.run().segments()[i];
                    row.refresh(timer, config, Some(cur), i, seg);
                }
            }
        }
    }

    fn enable_multilateral_selection(&self) {
        // Click navigation
        let list_weak = self.list.downgrade();

        self.last_segment_list
            .connect_row_selected(move |_, row_opt| {
                if row_opt.is_some()
                    && let Some(list_ref) = list_weak.upgrade()
                {
                    list_ref.unselect_all();
                }
            });

        let last_segment_list_weak = self.last_segment_list.downgrade();
        self.list.connect_row_selected(move |_, row_opt| {
            if row_opt.is_some()
                && let Some(list_ref) = last_segment_list_weak.upgrade()
            {
                list_ref.unselect_all();
            }
        });

        // Keyboard navigation
        let list_for_down = self.list.clone();
        let last_list_for_down = self.last_segment_list.clone();
        let down_ctrl = EventControllerKey::new();
        down_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Down
                && let Some(selected) = list_for_down.selected_row()
                && selected.next_sibling().is_none()
                && let Some(row) = last_list_for_down.row_at_index(0)
            {
                last_list_for_down.grab_focus();
                last_list_for_down.select_row(Some(&row));
                return Propagation::Stop;
            }
            Propagation::Proceed
        });
        self.list.add_controller(down_ctrl);

        let list_for_up = self.list.clone();
        let last_list_for_up = self.last_segment_list.clone();
        let scroller_for_up = self.scroller.clone();
        let up_ctrl = EventControllerKey::new();
        up_ctrl.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gdk::Key::Up
                && let Some(selected) = last_list_for_up.selected_row()
                && selected.index() == 0
                && let Some(last) = list_for_up.last_child()
                && let Ok(row) = last.downcast::<gtk4::ListBoxRow>()
            {
                list_for_up.grab_focus();
                list_for_up.select_row(Some(&row));
                scroller_for_up
                    .vadjustment()
                    .set_value(scroller_for_up.vadjustment().upper());
                return Propagation::Stop;
            }
            Propagation::Proceed
        });
        self.last_segment_list.add_controller(up_ctrl);
    }

    fn update_selection_policy(&mut self, phase: TimerPhase) {
        match phase {
            TimerPhase::Running | TimerPhase::Paused => {
                self.list.set_selection_mode(SelectionMode::None);
                self.list.unselect_all();
                self.last_segment_list
                    .set_selection_mode(SelectionMode::Single);
                self.last_segment_list.unselect_all();
            }
            TimerPhase::Ended => {
                self.list.set_selection_mode(SelectionMode::Single);
                self.last_segment_list
                    .set_selection_mode(SelectionMode::Single);
                if let Some(row) = self.last_segment_list.row_at_index(0) {
                    self.last_segment_list.select_row(Some(&row));
                }
            }
            _ => {
                self.list.set_selection_mode(SelectionMode::Single);
                self.list.unselect_all();
                self.last_segment_list
                    .set_selection_mode(SelectionMode::Single);
                self.last_segment_list.unselect_all();
            }
        }
    }

    fn rebuild_rows(&mut self, timer: &Timer, config: &mut Config) {
        self.build_rows(timer, config);
    }

    fn build_rows(&mut self, timer: &Timer, config: &mut Config) {
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
        while let Some(child) = self.last_segment_list.first_child() {
            self.last_segment_list.remove(&child);
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
                self.last_segment_list.append(row.row());
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

        if segments_requested < timer.run().len() - 1 {
            SegmentRow::get_natural_height() * segments_requested as i32
        } else {
            SegmentRow::get_natural_height() * (timer.run().len() as i32 - 1)
        }
    }
}

// SegmentRow: wraps a row widget and its value label so we can refresh without touching the ListBox
pub struct SegmentRow {
    row: ActionRow,
    suffix: SegmentSuffix,
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
        let title = segment.name().to_owned();
        let row = ActionRow::builder().title(&title).hexpand(true).build();

        if Some(index) == opt_current_segment_index {
            row.add_css_class("current-segment");
        }
        let suffix = SegmentSuffix::new(timer, config, opt_current_segment_index, index, segment);

        row.add_suffix(suffix.container());

        // Add no transition for more responsive updates
        row.add_css_class("no-transition");

        Self { row, suffix }
    }

    pub fn refresh(
        &mut self,
        timer: &Timer,
        config: &mut Config,
        opt_current_segment_index: Option<usize>,
        index: usize,
        segment: &livesplit_core::Segment,
    ) {
        // Reset dynamic classes
        self.row.remove_css_class("current-segment");
        if Some(index) == opt_current_segment_index {
            self.row.add_css_class("current-segment");
        }

        self.suffix
            .compute_segment(timer, config, opt_current_segment_index, index, segment);
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
}

// A segment suffix contains both the delta and the comparison labels, and renders them in a box, that is meant to be attached to a SegmentRow
pub struct SegmentSuffix {
    container: CenterBox,
    delta_label: Label,
    comparison_label: Label,
}

impl SegmentSuffix {
    pub fn new(
        timer: &Timer,
        config: &mut Config,
        opt_current_segment_index: Option<usize>,
        index: usize,
        segment: &livesplit_core::Segment,
    ) -> Self {
        let container = CenterBox::builder()
            .orientation(Orientation::Horizontal)
            .width_request(150)
            .build();
        let delta_label = Label::builder()
            .halign(Align::Center)
            .valign(Align::Center)
            .css_classes(["timer", "monospace"])
            .build();
        let comparison_label = Label::builder()
            .halign(Align::Center)
            .valign(Align::Center)
            .css_classes(["timer", "monospace", "comparison"])
            .build();
        container.set_start_widget(Some(&delta_label));
        container.set_end_widget(Some(&comparison_label));

        let suffix = Self {
            container,
            delta_label,
            comparison_label,
        };
        suffix.compute_segment(timer, config, opt_current_segment_index, index, segment);

        suffix
    }

    pub fn container(&self) -> &CenterBox {
        &self.container
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_segment(
        &self,
        timer: &Timer,
        config: &mut Config,
        opt_current_segment_index: Option<usize>,
        index: usize,
        segment: &livesplit_core::Segment,
    ) {
        let segment_comparison_time = segment_comparison_time(segment, timer);
        let (previous_comparison_duration, previous_comparison_time) =
            previous_comparison_values(timer, index);
        let segment_comparison_duration = segment_comparison_time
            .checked_sub(previous_comparison_duration)
            .unwrap_or_default()
            .abs();

        self.comparison_label.set_label(
            config
                .format
                .segment
                .format_split_time(
                    &segment.comparison(timer.current_comparison()),
                    timer.current_timing_method(),
                )
                .as_str(),
        );
        self.delta_label.set_label("");
        if let Some(current_segment_index) = opt_current_segment_index {
            if current_segment_index > index {
                self.compute_passed_segment(
                    timer,
                    config,
                    segment,
                    segment_comparison_time,
                    previous_comparison_time,
                    segment_comparison_duration,
                );
            }

            if current_segment_index == index {
                self.compute_current_segment(
                    timer,
                    config,
                    index,
                    segment,
                    segment_comparison_time,
                    previous_comparison_time,
                );
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn compute_passed_segment(
        &self,
        timer: &Timer,
        config: &mut Config,
        segment: &livesplit_core::Segment,
        segment_comparison_time: time::Duration,
        previous_comparison_time: time::Duration,
        segment_comparison_duration: time::Duration,
    ) {
        let split_time = segment_split_time(segment, timer);

        if split_time == time::Duration::ZERO {
            self.comparison_label.set_label("--");
            self.delta_label.set_label("");
        } else {
            let diff = split_time
                .checked_sub(segment_comparison_time)
                .unwrap_or_default();

            self.comparison_label.set_label(
                config
                    .format
                    .segment
                    .format_split_time(&segment.split_time(), timer.current_timing_method())
                    .as_str(),
            );
            if segment_comparison_time != time::Duration::ZERO {
                self.delta_label
                    .set_label(format_signed(diff, config).as_str());

                let gold_duration = best_segment_duration(segment, timer);
                let split_duration = split_time
                    .checked_sub(previous_comparison_time)
                    .unwrap_or_default();

                self.delta_label.add_css_class(classify_split_label(
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
        &self,
        timer: &Timer,
        config: &mut Config,
        index: usize,
        segment: &livesplit_core::Segment,
        segment_comparison_time: time::Duration,
        previous_comparison_time: time::Duration,
    ) {
        let current_duration = current_attempt_running_duration(timer);
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

        let gold_duration = best_segment_duration(segment, timer);
        if segment_comparison_time != time::Duration::ZERO
            && (diff.is_positive()
                || (gold_duration != time::Duration::ZERO && split_running_time >= gold_duration))
        {
            self.delta_label
                .set_label(format_signed(diff, config).as_str());
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
