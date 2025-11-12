use crate::config::Config;
use crate::formatters::label::format_label;
use crate::ui::info::{
    AdditionalInfo, BestPossibleTimeInfo, PrevSegmentBestInfo, PrevSegmentDiffInfo,
};

use glib;
use gtk4::prelude::{BoxExt as _, WidgetExt as _, *};
use gtk4::{
    Align, Box as GtkBox, CenterBox, Label, ListBox, Orientation::Horizontal, Orientation::Vertical,
};

use livesplit_core::{Timer, TimerPhase};

pub struct TimerFooter {
    container: GtkBox,
    timer_container: CenterBox,
    segment_comparison: SegmentComparison,
    running_timer: RunningTimer,
    additional_info: AdditionalInfoFooter,
}

impl TimerFooter {
    pub fn new(
        timer: &Timer,
        config: &mut Config,
        primary_list: &ListBox,
        last_segment_list: &ListBox,
    ) -> Self {
        let container = GtkBox::builder()
            .orientation(Vertical)
            .halign(Align::Fill)
            .hexpand(true)
            .spacing(12)
            .build();

        let timer_container = CenterBox::builder()
            .orientation(Horizontal)
            .hexpand(true)
            .build();

        let segment_comparison =
            SegmentComparison::new(timer, config, primary_list, last_segment_list);
        let running_timer = RunningTimer::new(timer, config);

        timer_container.set_start_widget(Some(segment_comparison.container()));
        timer_container.set_end_widget(Some(running_timer.container()));

        let additional_info = AdditionalInfoFooter::new(timer, config);

        let separator = gtk4::Separator::builder().build();
        container.append(&timer_container);
        container.append(&separator);
        container.append(&additional_info.container());

        Self {
            container,
            timer_container,
            segment_comparison,
            running_timer,
            additional_info,
        }
    }

    pub fn container(&self) -> &GtkBox {
        &self.container
    }

    pub fn refresh(&mut self, timer: &Timer, config: &mut Config) {
        self.segment_comparison.update(timer, config);
        self.running_timer.update(timer, config);
        self.additional_info.update(timer, config);

        self.timer_container
            .set_start_widget(Some(self.segment_comparison.container()));
        self.timer_container
            .set_end_widget(Some(self.running_timer.container()));
    }
}

pub struct AdditionalInfoFooter {
    additional_info: Vec<Box<dyn AdditionalInfo>>,
}

impl AdditionalInfoFooter {
    pub fn new(timer: &Timer, config: &mut Config) -> Self {
        let additional_info: Vec<Box<dyn AdditionalInfo>> = vec![
            Box::new(PrevSegmentDiffInfo::new(timer, config)),
            Box::new(PrevSegmentBestInfo::new(timer, config)),
            Box::new(BestPossibleTimeInfo::new(timer, config)),
        ];

        // additional_info.push();

        Self { additional_info }
    }

    pub fn update(&mut self, timer: &Timer, config: &mut Config) {
        for info in &mut self.additional_info {
            info.update(timer, config);
        }
    }

    pub fn container(&self) -> GtkBox {
        let container = GtkBox::builder()
            .orientation(Vertical)
            .halign(Align::Fill)
            .hexpand(true)
            .spacing(6)
            .build();

        for info in &self.additional_info {
            container.append(info.container());
        }

        container
    }
}

/// Left pane in the footer:
/// - Best: <best split value>
/// - <Comparison Label>: <per-segment comparison value>
pub struct SegmentComparison {
    wrapper: GtkBox,
    primary_list_ref: glib::WeakRef<ListBox>, // Weak ref to main segments list
    last_list_ref: glib::WeakRef<ListBox>,    // Weak ref to last-segment list
    best_value: Label,
    comparison_label: Label,
    comparison_value: Label,
}

impl SegmentComparison {
    pub fn new(
        timer: &Timer,
        config: &mut Config,
        primary_list: &ListBox,
        last_list: &ListBox,
    ) -> Self {
        let build = GtkBox::builder().orientation(Vertical).build();
        let wrapper = build;

        let vbox = GtkBox::builder().orientation(Vertical).build();

        let (best_box, best_value) = SegmentComparison::build_best();

        let (comparison_box, comparison_label, comparison_value) =
            SegmentComparison::build_comparison();

        vbox.append(&best_box);
        vbox.append(&comparison_box);
        wrapper.append(&vbox);

        let mut this = Self {
            wrapper,
            primary_list_ref: glib::WeakRef::new(),
            last_list_ref: glib::WeakRef::new(),
            best_value,
            comparison_label,
            comparison_value,
        };
        this.primary_list_ref.set(Some(primary_list));
        this.last_list_ref.set(Some(last_list));
        this.rebuild(timer, config);
        this
    }
    pub fn container(&self) -> &GtkBox {
        &self.wrapper
    }

    pub fn update(&mut self, timer: &Timer, config: &mut Config) {
        self.rebuild(timer, config);
    }

    fn rebuild(&mut self, timer: &Timer, config: &mut Config) {
        // Compute which segment to display
        let segments = timer.run().segments();
        let selected_index = if timer.current_phase().is_running() {
            timer.current_split_index().unwrap_or(0)
        } else {
            let mut idx = self
                .primary_list_ref
                .upgrade()
                .and_then(|l| l.selected_row())
                .map(|row| row.index() as usize);
            if idx.is_none()
                && let Some(last_list) = self.last_list_ref.upgrade()
                && last_list.selected_row().is_some()
            {
                idx = Some(segments.len().saturating_sub(1));
            }
            idx.unwrap_or(0)
        }
        .min(segments.len().saturating_sub(1));

        let segment = &segments[selected_index];

        // Previous segment's comparison time (under current timing method)
        let previous_comparison_time = if selected_index > 0 {
            segments[selected_index - 1]
                .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
                .unwrap_or_default()
                .to_duration()
        } else {
            time::Duration::ZERO
        };

        // Build values
        let best_value_text = config
            .format
            .segment
            .format_split_time(&segment.best_segment_time(), timer.current_timing_method());

        let comparison_label_text = format!("{}:", format_label(timer.current_comparison()));

        let comparison_value_text = {
            let segment_comparison_time = segment
                .comparison_timing_method(timer.current_comparison(), timer.current_timing_method())
                .unwrap_or_default()
                .to_duration();

            if previous_comparison_time.is_zero() || segment_comparison_time.is_zero() {
                String::from("--")
            } else {
                let per_segment_time = segment_comparison_time
                    .checked_sub(previous_comparison_time)
                    .unwrap_or_default()
                    .abs();
                config.format.comparison.format_duration(&per_segment_time)
            }
        };

        // Update stored labels in place
        if self.best_value.label().as_str() != best_value_text {
            self.best_value.set_label(&best_value_text);
        }
        if self.comparison_label.label().as_str() != comparison_label_text {
            self.comparison_label.set_label(&comparison_label_text);
        }
        if self.comparison_value.label().as_str() != comparison_value_text {
            self.comparison_value.set_label(&comparison_value_text);
        }
    }

    fn build_comparison() -> (GtkBox, Label, Label) {
        let comparison_box = GtkBox::builder()
            .orientation(Horizontal)
            .spacing(2)
            .halign(Align::Start)
            .build();

        let comparison_label = Label::builder().label("PB:").build();
        comparison_label.add_css_class("caption-heading");

        let comparison_value = Label::builder().label("--").build();
        comparison_value.add_css_class("caption");
        comparison_value.add_css_class("timer");

        comparison_box.append(&comparison_label);
        comparison_box.append(&comparison_value);
        (comparison_box, comparison_label, comparison_value)
    }

    fn build_best() -> (GtkBox, Label) {
        let best_box = GtkBox::builder()
            .orientation(Horizontal)
            .margin_top(6)
            .spacing(2)
            .halign(Align::Start)
            .build();
        let best_label = Label::builder().label("Best:").build();
        best_label.add_css_class("caption-heading");

        let best_value = Label::builder().label("--").build();
        best_value.add_css_class("caption");
        best_value.add_css_class("timer");

        best_box.append(&best_label);
        best_box.append(&best_value);
        (best_box, best_value)
    }
}

/// Right pane in the footer: the running timer display.
pub struct RunningTimer {
    wrapper: GtkBox,
    timer_box: GtkBox,
    hms_label: Label,
    ms_label: Label,
}

impl RunningTimer {
    pub fn new(timer: &Timer, config: &mut Config) -> Self {
        let wrapper = GtkBox::builder()
            .orientation(Horizontal)
            .halign(Align::End)
            .build();

        let timer_box = GtkBox::new(Horizontal, 0);
        timer_box.add_css_class("timer");
        if timer.current_phase() == TimerPhase::Running {
            timer_box.add_css_class("active-timer");
        } else {
            timer_box.add_css_class("inactive-timer");
        }

        let formatted = config.format.timer.format_timer(timer);
        let (left, right) = if let Some((l, r)) = formatted.rsplit_once('.') {
            (format!("{l}."), r.to_owned())
        } else {
            (formatted.clone(), String::new())
        };

        let hms_label = Label::builder().label(left).build();
        hms_label.add_css_class("bigtimer");

        let ms_label = Label::builder().label(right).margin_top(14).build();
        ms_label.add_css_class("smalltimer");

        timer_box.append(&hms_label);
        timer_box.append(&ms_label);
        wrapper.append(&timer_box);

        Self {
            wrapper,
            timer_box,
            hms_label,
            ms_label,
        }
    }

    pub fn container(&self) -> &GtkBox {
        &self.wrapper
    }

    pub fn update(&mut self, timer: &Timer, config: &mut Config) {
        self.rebuild(timer, config);
    }

    fn rebuild(&mut self, timer: &Timer, config: &mut Config) {
        self.timer_box.set_css_classes(match timer.current_phase() {
            TimerPhase::Running => &["timer", "active-timer"],
            _ => &["timer", "inactive-timer"],
        });

        // Update labels only if changed
        let formatted = config.format.timer.format_timer(timer);
        let (left, right) = if let Some((l, r)) = formatted.rsplit_once('.') {
            (format!("{l}."), r.to_owned())
        } else {
            (formatted.clone(), String::new())
        };

        if self.hms_label.label().as_str() != left {
            self.hms_label.set_label(&left);
        }
        if self.ms_label.label().as_str() != right {
            self.ms_label.set_label(&right);
        }
    }
}

#[cfg(test)]
mod footer_ui_tests {
    use super::*;
    use glib::prelude::Cast;
    use gtk4::{Box as GtkBox, Label, ListBox};
    use std::sync::Once;

    static INIT: Once = Once::new();

    fn gtk_test_init() {
        INIT.call_once(|| {
            gtk4::init().expect("Failed to init GTK");
            let _ = adw::init();
        });
    }

    #[gtk4::test]
    fn running_timer_negative_offset_displays_split_labels() {
        gtk_test_init();

        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.set_offset(livesplit_core::TimeSpan::from_seconds(-5.0));
        run.push_segment(livesplit_core::Segment::new("Split 1"));
        let timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let rt = RunningTimer::new(&timer, &mut config);
        let wrapper = rt.container();

        let timer_box_w = wrapper.first_child().expect("timer box");
        let timer_box: GtkBox = timer_box_w.downcast().expect("GtkBox");
        assert!(timer_box.has_css_class("timer"), "Expected 'timer' class");
        assert!(
            timer_box.has_css_class("inactive-timer"),
            "Expected 'inactive-timer' class"
        );

        let hms_w = timer_box.first_child().expect("hms");
        let hms: Label = hms_w.downcast().expect("Label");
        assert!(hms.has_css_class("bigtimer"), "Expected 'bigtimer' class");
        assert_eq!(
            hms.label().as_str(),
            "-5.",
            "Expected initial hms label to be '-5.'"
        );

        let ms_w = hms.next_sibling().expect("ms");
        let ms: Label = ms_w.downcast().expect("Label");
        assert!(
            ms.has_css_class("smalltimer"),
            "Expected 'smalltimer' class"
        );
        assert_eq!(
            ms.label().as_str(),
            "00",
            "Expected initial ms label to be '00'"
        );
    }

    #[gtk4::test]
    fn running_timer_has_two_labels_and_expected_classes_during_phase_changes() {
        gtk_test_init();

        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.push_segment(livesplit_core::Segment::new("Split 1"));
        let mut timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let mut rt = RunningTimer::new(&timer, &mut config);

        // Initially not running
        let wrapper = rt.container();
        let timer_box_w = wrapper.first_child().expect("timer box");
        let timer_box: GtkBox = timer_box_w.downcast().expect("GtkBox");
        assert!(timer_box.has_css_class("timer"), "Expected 'timer' class");
        assert!(
            timer_box.has_css_class("inactive-timer"),
            "Expected 'inactive-timer' class"
        );

        // Check label defaults
        let hms_w = timer_box.first_child().expect("hms");
        let hms: Label = hms_w.downcast().expect("Label");
        assert!(hms.has_css_class("bigtimer"), "Expected 'bigtimer' class");
        assert_eq!(
            hms.label().as_str(),
            "0.",
            "Expected initial hms label to be '0.'"
        );

        let ms_w = hms.next_sibling().expect("ms");
        let ms: Label = ms_w.downcast().expect("Label");
        assert!(
            ms.has_css_class("smalltimer"),
            "Expected 'smalltimer' class"
        );
        assert_eq!(
            ms.label().as_str(),
            "00",
            "Expected initial ms label to be '00'"
        );

        // Start timer -> active
        timer.start();
        rt.update(&timer, &mut config);
        let wrapper = rt.container();
        let timer_box_w = wrapper.first_child().expect("timer box");
        let timer_box: GtkBox = timer_box_w.downcast().expect("GtkBox");
        assert!(
            timer_box.has_css_class("active-timer"),
            "Expected 'active-timer' class"
        );

        // Pause -> inactive
        timer.pause();
        rt.update(&timer, &mut config);
        let wrapper = rt.container();
        let timer_box_w = wrapper.first_child().expect("timer box");
        let timer_box: GtkBox = timer_box_w.downcast().expect("GtkBox");
        assert!(
            timer_box.has_css_class("inactive-timer"),
            "Expected 'inactive-timer' class"
        );

        // Reset -> inactive
        timer.reset(false);
        rt.update(&timer, &mut config);
        let wrapper = rt.container();
        let timer_box_w = wrapper.first_child().expect("timer box");
        let timer_box: GtkBox = timer_box_w.downcast().expect("GtkBox");
        assert!(
            timer_box.has_css_class("inactive-timer"),
            "Expected 'inactive-timer' class"
        );
    }

    #[gtk4::test]
    fn segment_comparison_structure_and_texts() {
        gtk_test_init();

        // Build list for selection
        let list = ListBox::new();

        // Minimal timer and config
        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.push_segment(livesplit_core::Segment::new("Split 1"));
        let timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let last_list = ListBox::new();
        let mut sc = SegmentComparison::new(&timer, &mut config, &list, &last_list);
        let wrapper = sc.container();

        // vbox inside wrapper
        let vbox_w = wrapper.first_child().expect("vbox");
        let vbox: GtkBox = vbox_w.downcast().expect("GtkBox");

        // Best row
        let best_box_w = vbox.first_child().expect("best box");
        let best_box: GtkBox = best_box_w.downcast().expect("GtkBox");

        let best_label_w = best_box.first_child().expect("best label");
        let best_label: Label = best_label_w.downcast().expect("Label");
        assert_eq!(best_label.label().as_str(), "Best:",);
        assert!(
            best_label.has_css_class("caption-heading"),
            "Expected 'caption-heading' class"
        );

        let best_value_w = best_label.next_sibling().expect("best value");
        let best_value: Label = best_value_w.downcast().expect("Label");
        assert!(
            best_value.has_css_class("caption"),
            "Expected 'caption' class"
        );
        assert!(best_value.has_css_class("timer"), "Expected 'timer' class");
        // No best set -> "--"
        assert_eq!(best_value.label().as_str(), "--");

        // Comparison row
        let comparison_box_w = best_box.next_sibling().expect("comparison box");
        let comparison_box: GtkBox = comparison_box_w.downcast().expect("GtkBox");

        let comp_label_w = comparison_box.first_child().expect("comparison label");
        let comp_label: Label = comp_label_w.downcast().expect("Label");
        assert_eq!(comp_label.label().as_str(), "PB:");
        assert!(
            best_label.has_css_class("caption-heading"),
            "Expected 'caption-heading' class"
        );

        let comp_value_w = comp_label.next_sibling().expect("comparison value");
        let comp_value: Label = comp_value_w.downcast().expect("Label");
        assert!(
            comp_value.has_css_class("caption"),
            "Expected 'caption' class"
        );
        assert!(comp_value.has_css_class("timer"), "Expected 'timer' class");
        // No comparison times yet -> "0.00"
        assert_eq!(comp_value.label().as_str(), "0.00");

        // Ensure update works without panics and keeps structure
        sc.update(&timer, &mut config);
    }
}
