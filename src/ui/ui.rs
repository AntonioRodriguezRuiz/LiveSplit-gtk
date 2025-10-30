// This file defines the user interfaces for the application

use super::super::config::Config;

use std::cell::{Ref, RefMut};

use adw::prelude::*;
use adw::ApplicationWindow;
use adw::{self, Clamp};
use gtk4::Orientation::{Horizontal, Vertical};
use gtk4::{Align, Box as GtkBox, CenterBox, Label, ListBox};

use livesplit_core::{Run, Segment, Timer, TimerPhase};

use tracing::debug;

// Main screen for load / create splits
pub struct MainUI {}

// Timer layout for runs
pub struct TimerUI<'a> {
    timer: RefMut<'a, Timer>,
    config: Ref<'a, Config>,
}

// Splits editor/Creator
pub struct EditorUI {}

pub struct SettingsUI {}

pub struct AboutUI {}

pub struct HelpUI {}

impl<'a> TimerUI<'a> {
    pub fn new(timer: RefMut<'a, Timer>, config: Ref<'a, Config>) -> Self {
        Self { timer, config }
    }

    pub fn build_ui(&self) -> adw::Clamp {
        // --- Root Clamp ---
        let clamp = Clamp::builder().maximum_size(300).build();

        // === Outer VBox ===
        let livesplit_gtk = GtkBox::builder()
            .orientation(Vertical)
            .valign(Align::Center)
            .halign(Align::Center)
            .width_request(300)
            .spacing(20)
            .build();

        // =====================
        // Run Info Section
        // =====================
        let run_info = GtkBox::builder()
            .orientation(Vertical)
            .halign(Align::Center)
            .build();

        let run_name = Label::builder().label(self.timer.run().game_name()).build();
        run_name.add_css_class("title-2");
        debug!("Run Name: {}", run_name.label());

        let category = Label::builder()
            .label(self.timer.run().category_name())
            .build();
        category.add_css_class("heading");
        debug!("Category: {}", category.label());

        run_info.append(&run_name);
        run_info.append(&category);

        // =====================
        // Splits List
        // =====================
        let splits = ListBox::new();
        splits.add_css_class("boxed-list");

        // Helper to create rows
        fn make_split_row(title: &str, value: &str, classes: &[&str]) -> adw::ActionRow {
            let row = adw::ActionRow::builder().title(title).build();
            let label = Label::builder()
                .label(value)
                .halign(Align::Center)
                .valign(Align::Center)
                .build();
            label.add_css_class("timer");
            for cls in classes {
                label.add_css_class(cls);
            }
            row.add_suffix(&label);
            row
        }

        let segments = self.timer.run().segments();
        let mut rows = Vec::with_capacity(segments.len());

        let opt_current_segment_index = self.timer.current_split_index();

        for (index, segment) in segments.iter().enumerate() {
            let title = segment.name();
            let mut value = String::from("--");

            // Configure the value based on the current segment index
            if let Some(current_segment_index) = opt_current_segment_index {
                if current_segment_index > index {
                    value = format!(
                        "{:.2}",
                        segment
                            .split_time() // TODO: Implement custom css based on if ahead or behind
                            .real_time
                            .unwrap_or_default()
                            .to_duration()
                    ); // TODO: Allow for time instead of comparison | Allow for gametime/realtime comparison
                }
                if current_segment_index == index {
                    value = String::from("WIP") // TODO: Allow for time instead of comparison | Allow for gametime/realtime comparison
                }
            }
            let classes = if index == segments.len() - 1 {
                &["finalsplit"][..]
            } else {
                &["split"][..]
            };
            rows.push((title, value, classes));
        }

        for (title, value, classes) in rows {
            splits.append(&make_split_row(&title, &value, classes));
        }

        // =====================
        // Current Split + Timer
        // =====================
        let center_box = CenterBox::builder()
            .orientation(Horizontal)
            .margin_start(18)
            .margin_end(18)
            .build();

        // --- Left side: current split info ---
        let current_split = GtkBox::builder().orientation(Vertical).build();

        // Best
        let best_box = GtkBox::builder()
            .orientation(Horizontal)
            .margin_top(6)
            .spacing(2)
            .halign(Align::Start)
            .build();
        let best_label = Label::builder().label("Best:").build();
        best_label.add_css_class("caption-heading");

        let best_comparison_split = self
            .timer
            .current_split()
            .unwrap_or(segments.get(0).unwrap());
        let best_comparison_time = best_comparison_split
            .best_segment_time()
            .real_time
            .unwrap_or_default();

        let best_minutes = best_comparison_time.total_seconds() as i32 / 60 % 60;
        let best_seconds = best_comparison_time.total_seconds() as i32 % 60;
        let best_milliseconds = best_comparison_time.total_milliseconds() as i32 % 1000;
        let best_value = Label::builder()
            .label(format!(
                "{}:{:02}.{:02}",
                best_minutes, best_seconds, best_milliseconds
            ))
            .build();
        best_value.add_css_class("caption");
        best_value.add_css_class("timer");
        best_box.append(&best_label);
        best_box.append(&best_value);

        // comparison
        let comparison_box = GtkBox::builder()
            .orientation(Horizontal)
            .spacing(2)
            .halign(Align::Start)
            .build();
        let comparison_label = Label::builder() // TODO: Map comparisons to simpler string representations
            .label(format!(
                "{}:",
                self.config
                    .general
                    .comparison
                    .as_ref()
                    .unwrap_or(&String::from("PB"))
            ))
            .build();
        comparison_label.add_css_class("caption-heading");

        let comparison_time = self
            .timer
            .current_split()
            .unwrap_or(segments.get(0).unwrap())
            .comparison(
                &self
                    .config
                    .general
                    .comparison
                    .as_ref()
                    .unwrap_or(&String::from("")),
            ) // TODO: Implement custom css based on if ahead or behind
            .real_time
            .unwrap_or_default();

        let comparison_minutes = comparison_time.total_seconds() as i32 / 60 % 60;
        let comparison_seconds = comparison_time.total_seconds() as i32 % 60;
        let comparison_milliseconds = comparison_time.total_milliseconds() as i32 % 1000;
        let comparison_value = Label::builder()
            .label(format!(
                "{}:{:02}.{:02}",
                comparison_minutes, comparison_seconds, comparison_milliseconds
            ))
            .build();

        comparison_value.add_css_class("caption");
        comparison_value.add_css_class("timer");
        comparison_box.append(&comparison_label);
        comparison_box.append(&comparison_value);

        current_split.append(&best_box);
        current_split.append(&comparison_box);

        // --- Right side: timer display ---
        let timer_box = GtkBox::new(Horizontal, 0);
        timer_box.add_css_class("timer");
        timer_box.add_css_class("greensplit");

        let time = self.timer.current_attempt_duration();
        let minutes = time.total_seconds() as i32 / 60 % 60;
        let seconds = time.total_seconds() as i32 % 60;
        let hour_minutes_seconds_timer = Label::builder()
            .label(format!("{:02}.{:02}", minutes, seconds))
            .build();
        hour_minutes_seconds_timer.add_css_class("bigtimer");

        let milliseconds = time.total_milliseconds() as i32 % 1000;
        let milis_timer = Label::builder()
            .label(format!("{:02}", milliseconds))
            .margin_top(14)
            .build();
        milis_timer.add_css_class("smalltimer");

        timer_box.append(&hour_minutes_seconds_timer);
        timer_box.append(&milis_timer);

        center_box.set_start_widget(Some(&current_split));
        center_box.set_end_widget(Some(&timer_box));

        // =====================
        // Assemble everything
        // =====================
        livesplit_gtk.append(&run_info);
        livesplit_gtk.append(&splits);
        livesplit_gtk.append(&center_box);

        clamp.set_child(Some(&livesplit_gtk));

        clamp
    }
}
