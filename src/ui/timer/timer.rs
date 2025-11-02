use crate::config::Config;
use crate::ui::timer::{data_model, widgets};

use std::sync::{Arc, RwLock};
use std::time::Duration;

use adw::prelude::*;
use adw::{self, ApplicationWindow, Clamp, ToolbarView};
use glib::ControlFlow::Continue;
use gtk4::{
    Align, Box as GtkBox, CenterBox, Label, ListBox,
    Orientation::{Horizontal, Vertical},
};

use livesplit_core::{Timer, TimerPhase};

use tracing::debug;

// Timer layout for runs
pub struct TimerUI {
    timer: Arc<RwLock<Timer>>,
    config: Arc<RwLock<Config>>,
}

impl TimerUI {
    pub fn new(timer: Arc<RwLock<Timer>>, config: Arc<RwLock<Config>>) -> Self {
        Self { timer, config }
    }

    pub fn build_ui(&self) -> ToolbarView {
        // --- Root Clamp ---
        let clamp = Clamp::builder().maximum_size(300).build();

        // === Outer VBox ===
        let livesplit_gtk = GtkBox::builder()
            .orientation(Vertical)
            .valign(Align::Center)
            .halign(Align::Center)
            .width_request(300)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .spacing(20)
            .build();

        // =====================
        // Run Info Section
        // =====================
        let run_info = TimerUI::build_run_info(&self.timer.read().unwrap());

        //
        // Splits List
        // =====================
        let splits = ListBox::new();
        splits.add_css_class("boxed-list");
        let mut config_ref = self.config.write().unwrap();
        let splits_rows = TimerUI::build_splits_list(&self.timer.read().unwrap(), &mut config_ref);
        for row in splits_rows {
            splits.append(&row);
        }

        // =====================
        // Current Split + Timer
        // =====================
        let center_box = CenterBox::builder()
            .orientation(Horizontal)
            .margin_start(18)
            .margin_end(18)
            .build();
        center_box.set_start_widget(Some(&TimerUI::build_center_box_current_split_info(
            &self.timer.read().unwrap(),
            &mut config_ref,
        )));
        center_box.set_end_widget(Some(&TimerUI::build_center_box_timer(
            &self.timer.read().unwrap(),
            &mut config_ref,
        )));

        let splits_binding = splits.clone();
        let center_box_binding = center_box.clone();

        let timer_binding = self.timer.clone();
        let config_binding = self.config.clone();

        glib::timeout_add_local(Duration::from_millis(16), move || {
            let t = timer_binding.read().unwrap();
            let mut c = config_binding.write().unwrap();
            // =====================
            // Splits List
            // =====================
            // Remove all existing rows
            for _ in t.run().segments().iter() {
                if let Some(row) = splits_binding.row_at_index(0) {
                    splits_binding.remove(&row);
                }
            }
            // Now rebuild
            let splits_rows = TimerUI::build_splits_list(&t, &mut c);
            for row in splits_rows {
                splits_binding.append(&row);
            }

            // =====================
            // Current Split + Timer
            // =====================
            center_box_binding.set_start_widget(Some(
                &TimerUI::build_center_box_current_split_info(&t, &mut c),
            ));
            center_box_binding.set_end_widget(Some(&TimerUI::build_center_box_timer(&t, &mut c)));

            Continue
        });

        // =====================
        // Assemble everything
        // =====================
        livesplit_gtk.append(&run_info);
        livesplit_gtk.append(&splits);
        livesplit_gtk.append(&center_box);

        clamp.set_child(Some(&livesplit_gtk));

        // Building the window
        let view = ToolbarView::new();
        let header = adw::HeaderBar::builder()
            .title_widget(&Label::new(Some("LiveSplit GTK")))
            .show_end_title_buttons(true)
            .build();
        view.add_top_bar(&header);
        view.set_content(Some(&clamp));

        view
    }
}

impl TimerUI {
    fn build_run_info(timer: &Timer) -> GtkBox {
        let run_info = GtkBox::builder()
            .orientation(Vertical)
            .halign(Align::Center)
            .build();

        let run_name = Label::builder().label(timer.run().game_name()).build();
        run_name.add_css_class("title-2");
        debug!("Run Name: {}", run_name.label());

        let category = Label::builder().label(timer.run().category_name()).build();
        category.add_css_class("heading");
        debug!("Category: {}", category.label());

        run_info.append(&run_name);
        run_info.append(&category);
        run_info
    }

    fn build_splits_list(timer: &Timer, config: &mut Config) -> Vec<adw::ActionRow> {
        data_model::compute_split_rows(timer, config)
            .into_iter()
            .map(|d| widgets::split_row(&d))
            .collect()
    }

    fn build_center_box_current_split_info(timer: &Timer, config: &mut Config) -> GtkBox {
        let data = data_model::compute_current_split_info(timer, config);
        widgets::build_current_split_info_box(&data)
    }

    fn build_center_box_timer(timer: &Timer, config: &mut Config) -> GtkBox {
        widgets::build_timer_box(timer, config)
    }
}
