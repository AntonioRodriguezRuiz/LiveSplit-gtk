use crate::config::Config;
use crate::ui::timer::{data_model, timer, widgets};

use std::sync::{Arc, RwLock};
use std::time::Duration;

use adw::prelude::*;
use adw::{self, AlertDialog, ApplicationWindow, Clamp, ToolbarView};
use glib::ControlFlow::Continue;
use gtk4::{
    gio, Align, Box as GtkBox, CenterBox, FileChooserDialog, FileFilter, Label, ListBox,
    Orientation::{Horizontal, Vertical},
    SelectionMode,
};

use livesplit_core::{Timer, TimerPhase};

use tracing::debug;
use tracing_subscriber::field::debug;

// Timer layout for runs
pub struct TimerUI {
    timer: Arc<RwLock<Timer>>,
    config: Arc<RwLock<Config>>,
}

impl TimerUI {
    pub fn new(timer: Arc<RwLock<Timer>>, config: Arc<RwLock<Config>>) -> Self {
        Self { timer, config }
    }

    pub fn build_ui(&self, app: &adw::Application) -> adw::ApplicationWindow {
        let mut config_ref = self.config.write().unwrap();

        // --- Root Clamp ---
        let clamp = Clamp::builder().maximum_size(300).build();

        // === Outer VBox ===
        let livesplit_gtk = GtkBox::builder()
            .orientation(Vertical)
            .valign(Align::Center)
            .halign(Align::Center)
            .margin_top(24)
            .margin_bottom(24)
            .margin_start(24)
            .margin_end(24)
            .spacing(20)
            .build();

        // =====================
        // Run Info Section
        // =====================
        let run_info = GtkBox::builder()
            .orientation(Vertical)
            .halign(Align::Center)
            .build();
        let (run_name, category) = TimerUI::build_run_info(&self.timer.read().unwrap());
        run_info.append(&run_name);
        run_info.append(&category);

        //
        // Splits List
        // =====================
        let segments_list = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .css_classes(["boxed-list"])
            .build();
        let segments_rows =
            TimerUI::build_splits_list(&self.timer.read().unwrap(), &mut config_ref);
        for row in segments_rows {
            segments_list.append(&row);
        }
        segments_list.unselect_all();

        // =====================
        // Current Split + Timer
        // =====================
        let center_box = CenterBox::builder()
            .orientation(Horizontal)
            .width_request(300)
            .build();
        center_box.set_start_widget(Some(&TimerUI::build_center_box_selected_segment_info(
            &self.timer.read().unwrap(),
            &mut config_ref,
            &segments_list,
        )));
        center_box.set_end_widget(Some(&TimerUI::build_center_box_timer(
            &self.timer.read().unwrap(),
            &mut config_ref,
        )));

        let run_info_binding = run_info.clone();

        let segments_binding = segments_list.clone();
        let center_box_binding = center_box.clone();

        let mut rendered_comparison = self.timer.read().unwrap().current_comparison().to_string();
        let mut rendered_phase = self.timer.read().unwrap().current_phase();
        let mut render_all_segments = true;
        let binding = self.config.clone();
        let mut rendered_splits = config_ref.general.splits.clone().unwrap_or_default();

        let timer_binding = self.timer.clone();
        let config_binding = self.config.clone();

        glib::timeout_add_local(Duration::from_millis(16), move || {
            let t = timer_binding.read().unwrap();
            let mut c = config_binding.write().unwrap();

            render_all_segments = (rendered_comparison != t.current_comparison().to_string())
                || (rendered_phase != t.current_phase())
                || (rendered_splits != c.general.splits.clone().unwrap_or_default());

            // Rerender run info if it changes
            if rendered_splits != c.general.splits.clone().unwrap_or_default() {
                let (run_name, category) = TimerUI::build_run_info(&t);
                loop {
                    if let Some(child) = run_info_binding.first_child() {
                        run_info_binding.remove(&child);
                    } else {
                        break;
                    }
                }
                run_info_binding.append(&run_name);
                run_info_binding.append(&category);
            }

            // =====================
            // Splits List
            // =====================
            if render_all_segments {
                render_all_segments = false;
                segments_binding.set_selection_mode(SelectionMode::Single);

                let mut selected_index: Option<i32> = None;

                // REBUILD ONCE
                for (index, _) in t.run().segments().iter().enumerate() {
                    if let Some(row) = segments_binding.row_at_index(0) {
                        if row.is_selected() {
                            selected_index = Some(index as i32);
                        }
                    }
                }

                segments_binding.unbind_model();

                let splits_rows = TimerUI::build_splits_list(&t, &mut c);
                for row in splits_rows {
                    segments_binding.append(&row);
                }

                if t.current_phase().is_ended() {
                    segments_binding.select_row(
                        segments_binding
                            .row_at_index(
                                selected_index
                                    .unwrap_or(t.run().segments().len().saturating_sub(1) as i32),
                            )
                            .as_ref(),
                    );
                } else {
                    segments_binding.unselect_all();
                }
            } else if t.current_phase().is_running() {
                render_all_segments = true;
                segments_binding.set_selection_mode(SelectionMode::None);

                let opt_current_segment_index = t.current_split_index().unwrap_or(0);
                let segments = t.run().segments();

                for (index, _) in segments.iter().enumerate() {
                    if let Some(row) = segments_binding.row_at_index(index as i32) {
                        // Set rows as not selectable to avoid interaction during update
                        if index == opt_current_segment_index
                            || index == opt_current_segment_index.saturating_sub(1)
                            || index == opt_current_segment_index.saturating_add(1)
                        {
                            segments_binding.remove(&row);
                            let row = widgets::split_row(&data_model::compute_segment_row(
                                &t,
                                &mut c,
                                Some(opt_current_segment_index),
                                index,
                                &segments[index],
                            ));
                            segments_binding.insert(&row, index as i32);
                        }
                    }
                }
            }

            // =====================
            // Current Split + Timer
            // =====================
            center_box_binding.set_start_widget(Some(
                &TimerUI::build_center_box_selected_segment_info(&t, &mut c, &segments_binding),
            ));
            center_box_binding.set_end_widget(Some(&TimerUI::build_center_box_timer(&t, &mut c)));

            rendered_comparison = t.current_comparison().to_string();
            rendered_phase = t.current_phase();
            rendered_splits = c.general.splits.clone().unwrap_or_default();

            Continue
        });

        // =====================
        // Assemble everything
        // =====================
        livesplit_gtk.append(&run_info);
        livesplit_gtk.append(&segments_list);
        livesplit_gtk.append(&center_box);

        clamp.set_child(Some(&livesplit_gtk));

        // Building the window
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title(
                Label::builder()
                    .label("TuxSplit")
                    .css_classes(["heading"])
                    .build()
                    .label(),
            )
            .resizable(false)
            .build();

        let view = ToolbarView::new();
        let header = self.build_main_header(&window);
        view.add_top_bar(&header);
        view.set_content(Some(&clamp));

        window.set_content(Some(&view));

        window
    }

    fn build_main_header(&self, parent: &ApplicationWindow) -> adw::HeaderBar {
        let header = adw::HeaderBar::builder()
            .title_widget(&Label::new(Some("TuxSplit")))
            .show_end_title_buttons(true)
            .build();

        // Hamburger menu with Load/Save Splits using a proper application menu
        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .build();

        // Build a MenuModel and attach actions on the application (app.*)
        let menu = gio::Menu::new();

        let splits_section = gio::Menu::new();
        splits_section.append(Some("Load Splits"), Some("app.load-splits"));
        splits_section.append(Some("Save Splits"), Some("app.save-splits"));

        let settings_section = gio::Menu::new();
        settings_section.append(Some("Settings"), Some("app.settings"));
        settings_section.append(Some("Keybindings"), Some("app.keybindings"));

        let about_section = gio::Menu::new();
        about_section.append(Some("About"), Some("app.about"));

        menu.append_section(None, &splits_section);
        menu.append_section(None, &settings_section);
        menu.append_section(None, &about_section);
        menu_button.set_menu_model(Some(&menu));

        // Load Splits action
        let load_action = gio::SimpleAction::new("load-splits", None);
        let timer_for_load = self.timer.clone();
        let config_for_load = self.config.clone();

        let parent_binding = parent.clone();

        load_action.connect_activate(move |_, _| {
            let file_chooser = FileChooserDialog::new(
                Some("Load Splits"),
                Some(&parent_binding),
                gtk4::FileChooserAction::Open,
                &[
                    ("Open", gtk4::ResponseType::Ok),
                    ("Cancel", gtk4::ResponseType::Cancel),
                ],
            );

            let lss_filter = FileFilter::new();
            let all_filter = FileFilter::new();
            lss_filter.set_name(Some("LiveSplit Splits (*.lss)"));
            all_filter.set_name(Some("All Files"));
            lss_filter.add_pattern("*.lss");
            all_filter.add_pattern("*");
            file_chooser.add_filter(&lss_filter);
            file_chooser.add_filter(&all_filter);

            let t_binding = timer_for_load.clone();
            let c_binding = config_for_load.clone();
            file_chooser.connect_response(move |dialog, response| {
                let mut c = c_binding.write().unwrap();
                let mut t = t_binding.write().unwrap();
                if response == gtk4::ResponseType::Ok {
                    if let Some(file) = dialog.file() {
                        if let Some(path) = file.path() {
                            c.set_splits_path(path);
                            let run = c.parse_run_or_default();
                            if let Some(run) = c.parse_run() {
                                t.set_run(run);
                                c.configure_timer(&mut t);
                            }
                        }
                    }
                }
                dialog.destroy(); // This hides and closes the dialog window
            });

            file_chooser.set_modal(true);
            file_chooser.present();
        });

        // Save Splits action
        let save_action = gio::SimpleAction::new("save-splits", None);
        let timer_for_save = self.timer.clone();
        let config_for_save = self.config.clone();
        save_action.connect_activate(move |_, _| {
            let t = timer_for_save.read().unwrap();
            let c = config_for_save.read().unwrap();
            c.save_splits(&t);
        });

        // TODO: Config
        let _settings_action = gio::SimpleAction::new("settings", None);
        let parent_for_settings = parent.clone();
        _settings_action.connect_activate(move |_, _| {
            let dialog = AlertDialog::builder()
                .heading("Settings")
                .body("This feature isn’t available yet. Stay tuned!")
                .default_response("ok")
                .build();
            dialog.add_response("ok", "Okay");
            dialog.present(Some(&parent_for_settings));
        });

        // Keybinds (For now only shows default keybinds)
        // TODO: Sync with config hotkeys
        let keybinds_action = gio::SimpleAction::new("keybindings", None);
        let parent_for_keybinds = parent.clone();
        keybinds_action.connect_activate(move |_, _| {
            let dialog = AlertDialog::builder()
                .heading("Keybindings")
                .body("Current keybinds are not modifiable yet.")
                .default_response("ok")
                .build();

            let keybinds_list = ListBox::new();
            keybinds_list.add_css_class("boxed-list");
            let keybinds = vec![
                ("Start / Split", "Numpad 1"),
                ("Skip Split", "Numpad 2"),
                ("Reset", "Numpad 3"),
                ("Previous Comparison", "Numpad 4"),
                ("Pause", "Numpad 5"),
                ("Next Comparison", "Numpad 6"),
                ("Undo", "Numpad 8"),
            ];
            for (action, key) in keybinds {
                let key_label = Label::new(Some(key));
                let row = adw::ActionRow::builder().title(action).build();
                row.add_suffix(&key_label);
                keybinds_list.append(&row);
            }

            dialog.set_extra_child(Some(&keybinds_list));

            dialog.add_response("ok", "Okay");
            dialog.present(Some(&parent_for_keybinds));
        });

        // About action
        let about_action = gio::SimpleAction::new("about", None);
        let parent_for_about = parent.clone();
        about_action.connect_activate(move |_, _| {
            let about_dialog = adw::AboutDialog::builder()
                .application_name("TuxSplit")
                .version("0.0.1")
                .comments("A GTK-based LiveSplit timer application.")
                .license_type(gtk4::License::MitX11)
                .website("https://github.com/AntonioRodriguezRuiz/tuxsplit")
                .build();
            about_dialog.present(Some(&parent_for_about));
        });

        let group = gio::SimpleActionGroup::new();
        group.add_action(&load_action);
        group.add_action(&save_action);
        group.add_action(&_settings_action);
        group.add_action(&keybinds_action);
        group.add_action(&about_action);

        menu_button.insert_action_group("app", Some(&group));

        header.pack_start(&menu_button);

        header
    }
}

impl TimerUI {
    fn build_run_info(timer: &Timer) -> (Label, Label) {
        let run_name = Label::builder().label(timer.run().game_name()).build();
        run_name.add_css_class("title-2");
        debug!("Run Name: {}", run_name.label());

        let category = Label::builder().label(timer.run().category_name()).build();
        category.add_css_class("heading");
        debug!("Category: {}", category.label());

        (run_name, category)
    }

    fn build_splits_list(timer: &Timer, config: &mut Config) -> Vec<adw::ActionRow> {
        data_model::compute_split_rows(timer, config)
            .into_iter()
            .map(|d| {
                let row = widgets::split_row(&d);
                row
            })
            .collect()
    }

    fn build_center_box_selected_segment_info(
        timer: &Timer,
        config: &mut Config,
        segments_list: &ListBox,
    ) -> GtkBox {
        let data = data_model::compute_selected_segment_info(timer, config, segments_list);
        widgets::build_selected_segment_info_box(&data)
    }

    fn build_center_box_timer(timer: &Timer, config: &mut Config) -> GtkBox {
        widgets::build_timer_box(timer, config)
    }
}
