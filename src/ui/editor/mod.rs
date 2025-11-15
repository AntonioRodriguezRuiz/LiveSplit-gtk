mod context;
mod model;
mod row;
mod table;

pub use context::EditorContext;
pub use model::SegmentsModel;

use crate::config::Config;
use crate::ui::editor::table::SegmentsEditor;
use gtk4::StringList;
use livesplit_core::{TimeSpan, Timer};
use std::sync::{Arc, RwLock};

use adw::prelude::*;
use adw::{ComboRow, EntryRow, PreferencesDialog, PreferencesGroup, PreferencesPage};

pub struct SplitEditor {
    dialog: PreferencesDialog,
    timer: Arc<RwLock<Timer>>,
    config: Arc<RwLock<Config>>,
}

impl SplitEditor {
    pub fn new(timer: Arc<RwLock<Timer>>, config: Arc<RwLock<Config>>) -> Self {
        let dialog = PreferencesDialog::new();
        dialog.set_height_request(500);
        dialog.set_title("Timer Preferences");

        let this = Self {
            dialog,
            timer,
            config,
        };

        let run_info = this.build_run_info_page();
        let segment_editor = this.build_segment_editor_page();

        this.dialog.add(&run_info);
        this.dialog.add(&segment_editor);

        this
    }

    pub fn dialog(&self) -> &PreferencesDialog {
        &self.dialog
    }

    pub fn present(&self) {
        self.dialog.present(None::<&gtk4::Widget>);
    }

    fn build_run_info_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("General")
            .icon_name("gears-symbolic")
            .build();

        let run_info_group = self.build_run_info_preferences();
        let timer_group = self.build_timer_preferences();
        // let autosplit_group = self.build_autosplit_preferences();

        page.add(&run_info_group);
        page.add(&timer_group);
        // page.add(&autosplit_group);

        page
    }

    fn build_run_info_preferences(&self) -> PreferencesGroup {
        // Logic to create preferences for run information
        let group = PreferencesGroup::builder()
            .title("Run Information")
            .description("General run information details")
            .build();

        let timer = self.timer.read().unwrap();
        let name = EntryRow::builder()
            .title("Game Name")
            .text(timer.run().game_name())
            .build();
        let category = EntryRow::builder()
            .title("Category")
            .text(timer.run().category_name())
            .build();

        {
            let timer_binding = Arc::clone(&self.timer);
            name.connect_text_notify(move |entry| {
                let new_name = entry.text().to_string();

                let mut timer = timer_binding.write().unwrap();
                let mut run = timer.run().clone();

                run.set_game_name(new_name);
                assert!(timer.set_run(run).is_ok());

                drop(timer);
            });
        }
        {
            let timer_binding = Arc::clone(&self.timer);
            category.connect_text_notify(move |entry| {
                let new_category = entry.text().to_string();

                let mut timer = timer_binding.write().unwrap();
                let mut run = timer.run().clone();

                run.set_category_name(new_category);
                assert!(timer.set_run(run).is_ok());

                drop(timer);
            });
        }

        group.add(&name);
        group.add(&category);

        group
    }

    fn build_timer_preferences(&self) -> PreferencesGroup {
        let group = PreferencesGroup::builder()
            .title("Timer")
            .description("Run timing configuration")
            .build();

        let options = StringList::new(&["Real Time", "Game Time"]);
        let initial_method = {
            let timer = self.timer.read().unwrap();
            match timer.current_timing_method() {
                livesplit_core::TimingMethod::GameTime => 1,
                _ => 0,
            }
        };

        let timer = self.timer.read().unwrap();

        let offset = EntryRow::builder()
            .title("Start at")
            .text(format!("{:3}", timer.run().offset().total_seconds(),))
            .build();
        let timing_method = ComboRow::builder()
            .title("Timing Method")
            .model(&options)
            .selected(initial_method)
            .build();

        let timer_binding = Arc::clone(&self.timer);
        offset.connect_text_notify(move |entry| {
            // Offset must be a valid f64 value
            if entry.text().parse::<f64>().is_ok() {
                entry.set_title("Start at");
                entry.remove_css_class("error");
                let new_offset = entry.text().parse::<f64>().unwrap();

                let mut timer = timer_binding.write().unwrap();
                let mut run = timer.run().clone();

                run.set_offset(TimeSpan::from_seconds(new_offset));
                assert!(timer.set_run(run).is_ok());

                drop(timer);
            } else {
                entry.set_title("Start at (entry must be a valid number)");
                entry.add_css_class("error");
            }
        });

        let timer_binding = Arc::clone(&self.timer);
        timing_method.connect_selected_notify(move |r| {
            let idx = r.selected();
            let mut t = timer_binding.write().unwrap();
            match idx {
                0 => t.set_current_timing_method(livesplit_core::TimingMethod::RealTime),
                1 => t.set_current_timing_method(livesplit_core::TimingMethod::GameTime),
                _ => (),
            }
        });

        group.add(&offset);
        group.add(&timing_method);

        group
    }

    fn build_autosplit_preferences(&self) -> PreferencesGroup {
        // Logic to create autosplitter preferences UI component
        unimplemented!()
    }

    fn build_segment_editor_page(&self) -> PreferencesPage {
        let page = PreferencesPage::builder()
            .title("Segments")
            .icon_name("view-list-symbolic")
            .build();

        let segment_editor = SegmentsEditor::new(Arc::clone(&self.timer));
        let group = PreferencesGroup::builder()
            .title("Segment Editor")
            .description("Edit your run segments")
            .build();

        group.add(segment_editor.table());

        page.add(&group);

        page
    }
}
