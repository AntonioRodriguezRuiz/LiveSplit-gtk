use crate::config::Config;
use crate::ui::timer::data_model::{CurrentSplitInfoData, SplitRowData};
use crate::utils::time::format_timer;

use adw::prelude::ActionRowExt;
use adw::ActionRow;
use gtk4::prelude::{BoxExt, WidgetExt};
use gtk4::{Align, Box as GtkBox, Label, Orientation::Horizontal};

use livesplit_core::Timer;

/// Creates an ActionRow for a split using data from the view model.
/// - Adds row/label CSS classes from the data to preserve styling.
/// - Appends a trailing label containing the split value.
pub fn split_row(data: &SplitRowData) -> ActionRow {
    let row = ActionRow::builder().title(&data.title).build();

    let label = Label::builder()
        .label(&data.value_text)
        .halign(Align::Center)
        .valign(Align::Center)
        .build();
    label.add_css_class("timer");

    for cls in &data.segment_classes {
        row.add_css_class(cls);
    }
    for cls in &data.label_classes {
        label.add_css_class(cls);
    }

    row.add_suffix(&label);
    row
}

/// Builds the right-side timer display box (HH:MM:SS.mmm).
/// - Mirrors existing styling: "timer", "greensplit", "bigtimer", "smalltimer".
/// - Splits the formatted string to show milliseconds with a smaller label.
pub fn build_timer_box(timer: &Timer, config: &mut Config) -> GtkBox {
    let timer_box = GtkBox::new(Horizontal, 0);
    timer_box.add_css_class("timer");
    if timer.current_phase() == livesplit_core::TimerPhase::Running {
        timer_box.add_css_class("active-timer");
    } else {
        timer_box.add_css_class("inactive-timer");
    }

    let formatted = format_timer(timer, config);
    let (left, right) = if let Some((l, r)) = formatted.rsplit_once('.') {
        (format!("{}.", l), r.to_string())
    } else {
        (formatted.clone(), String::new())
    };

    let hms_label = Label::builder().label(left).build();
    hms_label.add_css_class("bigtimer");

    let ms_label = Label::builder().label(right).margin_top(14).build();
    ms_label.add_css_class("smalltimer");

    timer_box.append(&hms_label);
    timer_box.append(&ms_label);

    timer_box
}

/// Builds the left-side "current split info" panel:
/// - Best: <value>
/// - <Comparison Label>: <value>
/// Mirrors existing styling for captions and timer labels.
pub fn build_current_split_info_box(data: &CurrentSplitInfoData) -> GtkBox {
    let current_split = GtkBox::builder()
        .orientation(gtk4::Orientation::Vertical)
        .build();

    // Best
    let best_box = GtkBox::builder()
        .orientation(Horizontal)
        .margin_top(6)
        .spacing(2)
        .halign(Align::Start)
        .build();
    let best_label = Label::builder().label("Best:").build();
    best_label.add_css_class("caption-heading");

    let best_value = Label::builder().label(&data.best_value_text).build();
    best_value.add_css_class("caption");
    best_value.add_css_class("timer");
    best_box.append(&best_label);
    best_box.append(&best_value);

    // Comparison
    let comparison_box = GtkBox::builder()
        .orientation(Horizontal)
        .spacing(2)
        .halign(Align::Start)
        .build();
    let comparison_label = Label::builder().label(&data.comparison_label_text).build();
    comparison_label.add_css_class("caption-heading");

    let comparison_value = Label::builder().label(&data.comparison_value_text).build();
    comparison_value.add_css_class("caption");
    comparison_value.add_css_class("timer");
    comparison_box.append(&comparison_label);
    comparison_box.append(&comparison_value);

    current_split.append(&best_box);
    current_split.append(&comparison_box);

    current_split
}

#[cfg(test)]
mod tests {
    use super::*;
    use adw::prelude::*;
    use glib::prelude::{Cast, IsA};
    use gtk4::{Box as GtkBox, Label};
    use std::sync::Once;

    static INIT: Once = Once::new();

    pub fn gtk_test_init() {
        INIT.call_once(|| {
            gtk4::init().expect("Failed to init GTK");
            let _ = adw::init();
        });
    }

    fn has_class<W: IsA<gtk4::Widget>>(w: &W, class: &str) -> bool {
        w.as_ref().css_classes().iter().any(|c| c.as_str() == class)
    }

    #[gtk4::test]
    fn split_row_applies_title_and_segment_classes() {
        gtk_test_init();

        let data = SplitRowData {
            title: "Split A".to_string(),
            value_text: "1:23.45".to_string(),
            segment_classes: vec!["current-segment", "foo"],
            label_classes: vec!["greensplit", "timer"],
        };

        let row = split_row(&data);

        assert_eq!(row.title().as_str(), "Split A");
        assert!(has_class(&row, "current-segment"));
        assert!(has_class(&row, "foo"));
    }

    #[gtk4::test]
    fn build_timer_box_negative_offset() {
        gtk_test_init();

        // Minimal timer and config
        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.set_offset(livesplit_core::TimeSpan::from_seconds(-5.0));
        run.push_segment(livesplit_core::Segment::new("Split 1"));
        let timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let timer_box = build_timer_box(&timer, &mut config);

        // Children: first is bigtimer label with "0.", second is smalltimer label with "00"
        let first = timer_box.first_child().expect("first child");
        let hms: Label = first.downcast().expect("Label");
        assert!(has_class(&hms, "bigtimer"));
        assert_eq!(hms.label().as_str(), "-5.");

        let second = hms.next_sibling().expect("second child");
        let ms: Label = second.downcast().expect("Label");
        assert!(has_class(&ms, "smalltimer"));
        assert_eq!(ms.label().as_str(), "00");
    }

    #[gtk4::test]
    fn build_timer_box_has_two_labels_and_expected_classes() {
        gtk_test_init();

        // Minimal timer and config
        let mut run = livesplit_core::Run::new();
        run.set_game_name("Game");
        run.set_category_name("Any%");
        run.push_segment(livesplit_core::Segment::new("Split 1"));
        let mut timer = livesplit_core::Timer::new(run).expect("timer");
        let mut config = Config::default();

        let mut timer_box = build_timer_box(&timer, &mut config);

        // Box classes
        assert!(has_class(&timer_box, "timer"));
        assert!(has_class(&timer_box, "inactive-timer"));

        timer.start();

        timer_box = build_timer_box(&timer, &mut config);
        assert!(has_class(&timer_box, "active-timer"));

        timer.pause();

        timer_box = build_timer_box(&timer, &mut config);
        assert!(has_class(&timer_box, "inactive-timer"));

        timer.reset(false);

        timer_box = build_timer_box(&timer, &mut config);
        assert!(has_class(&timer_box, "inactive-timer"));

        // Children: first is bigtimer label with "0.", second is smalltimer label with "00"
        let first = timer_box.first_child().expect("first child");
        let hms: Label = first.downcast().expect("Label");
        assert!(has_class(&hms, "bigtimer"));
        assert_eq!(hms.label().as_str(), "0.");

        let second = hms.next_sibling().expect("second child");
        let ms: Label = second.downcast().expect("Label");
        assert!(has_class(&ms, "smalltimer"));
        assert_eq!(ms.label().as_str(), "00");
    }

    #[gtk4::test]
    fn build_current_split_info_box_structure_and_texts() {
        gtk_test_init();

        let data = CurrentSplitInfoData {
            best_value_text: "1:23.45".to_string(),
            comparison_label_text: "PB:".to_string(),
            comparison_value_text: "0:45.67".to_string(),
        };

        let vbox = build_current_split_info_box(&data);

        // First row: Best
        let best_box_w = vbox.first_child().expect("best box");
        let best_box: GtkBox = best_box_w.downcast().expect("GtkBox");
        let best_label_w = best_box.first_child().expect("best label");
        let best_label: Label = best_label_w.downcast().expect("Label");
        assert_eq!(best_label.label().as_str(), "Best:");
        assert!(has_class(&best_label, "caption-heading"));

        let best_value_w = best_label.next_sibling().expect("best value");
        let best_value: Label = best_value_w.downcast().expect("Label");
        assert_eq!(best_value.label().as_str(), "1:23.45");
        assert!(has_class(&best_value, "caption"));
        assert!(has_class(&best_value, "timer"));

        // Second row: Comparison
        let comparison_box_w = best_box.next_sibling().expect("comparison box");
        let comparison_box: GtkBox = comparison_box_w.downcast().expect("GtkBox");

        let comp_label_w = comparison_box.first_child().expect("comparison label");
        let comp_label: Label = comp_label_w.downcast().expect("Label");
        assert_eq!(comp_label.label().as_str(), "PB:");
        assert!(has_class(&comp_label, "caption-heading"));

        let comp_value_w = comp_label.next_sibling().expect("comparison value");
        let comp_value: Label = comp_value_w.downcast().expect("Label");
        assert_eq!(comp_value.label().as_str(), "0:45.67");
        assert!(has_class(&comp_value, "caption"));
        assert!(has_class(&comp_value, "timer"));
    }
}
